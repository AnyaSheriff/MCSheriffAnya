//! Clean-room implementation of the public HAProxy PROXY protocol specification.
//! Only TCP/IPv4 and TCP/IPv6 addresses are accepted; LOCAL/UNKNOWN keep the peer.
use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use tokio::io::{AsyncRead, AsyncReadExt};

const SIGNATURE: &[u8] = b"\r\n\r\n\0\r\nQUIT\n";
fn invalid() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "invalid PROXY header")
}

/// In enabled mode every connection must have a header from a trusted TCP peer.
/// Check trust before reading any bytes; bound both memory and total read time.
pub async fn read<R: AsyncRead + Unpin>(stream: &mut R, peer: SocketAddr, trusted: &[IpAddr]) -> io::Result<SocketAddr> {
    if !trusted.contains(&peer.ip()) {
        return Err(io::Error::new(io::ErrorKind::PermissionDenied, "untrusted PROXY source"));
    }
    tokio::time::timeout(std::time::Duration::from_secs(3), read_header(stream, peer))
        .await.map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "PROXY header timeout"))?
}

async fn read_header<R: AsyncRead + Unpin>(stream: &mut R, peer: SocketAddr) -> io::Result<SocketAddr> {
    let first = stream.read_u8().await?;
    if first == b'P' {
        let mut line = vec![first];
        while !line.ends_with(b"\r\n") {
            if line.len() >= 107 { return Err(invalid()); }
            line.push(stream.read_u8().await?);
        }
        let line = std::str::from_utf8(&line[..line.len() - 2]).map_err(|_| invalid())?;
        let fields: Vec<_> = line.split(' ').collect();
        if fields.first() != Some(&"PROXY") { return Err(invalid()); }
        if fields.get(1) == Some(&"UNKNOWN") { return Ok(peer); }
        if fields.len() != 6 { return Err(invalid()); }
        let source: IpAddr = fields[2].parse().map_err(|_| invalid())?;
        let dest: IpAddr = fields[3].parse().map_err(|_| invalid())?;
        match (fields[1], source, dest) {
            ("TCP4", IpAddr::V4(_), IpAddr::V4(_)) | ("TCP6", IpAddr::V6(_), IpAddr::V6(_)) => {}
            _ => return Err(invalid()),
        }
        let port = |s: &str| -> io::Result<u16> {
            if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) || (s.len() > 1 && s.starts_with('0')) { return Err(invalid()); }
            s.parse().map_err(|_| invalid())
        };
        let source_port = port(fields[4])?;
        port(fields[5])?;
        return Ok(SocketAddr::new(source, source_port));
    }
    let mut header = [0; 16];
    header[0] = first;
    stream.read_exact(&mut header[1..]).await?;
    if &header[..12] != SIGNATURE || header[12] >> 4 != 2 { return Err(invalid()); }
    let length = u16::from_be_bytes([header[14], header[15]]) as usize;
    // Read no more than the protocol's u16 length, including optional TLVs.
    let mut body = vec![0; length];
    stream.read_exact(&mut body).await?;
    match header[12] & 15 {
        0 => Ok(peer), // LOCAL: address and transport information must be ignored.
        1 => {
            let (ip, offset) = match header[13] {
                0x11 if length >= 12 => (IpAddr::V4(Ipv4Addr::new(body[0], body[1], body[2], body[3])), 8),
                0x21 if length >= 36 => {
                    let bytes: [u8; 16] = body[..16].try_into().map_err(|_| invalid())?;
                    (IpAddr::V6(Ipv6Addr::from(bytes)), 32)
                }
                _ => return Err(invalid()),
            };
            Ok(SocketAddr::new(ip, u16::from_be_bytes([body[offset], body[offset + 1]])))
        }
        _ => Err(invalid()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn peer() -> SocketAddr { "127.0.0.1:1234".parse().unwrap() }
    async fn parse(bytes: &[u8]) -> io::Result<SocketAddr> { read(&mut &bytes[..], peer(), &[peer().ip()]).await }
    fn v2(family: u8, body: &[u8]) -> Vec<u8> {
        let mut bytes = SIGNATURE.to_vec();
        bytes.extend([0x21, family]);
        bytes.extend((body.len() as u16).to_be_bytes());
        bytes.extend(body);
        bytes
    }
    #[tokio::test]
    async fn v1_addresses_and_trailing_game_bytes() {
        let mut data = &b"PROXY TCP4 198.51.100.7 127.0.0.1 4567 26100\r\nGAME"[..];
        assert_eq!(read(&mut data, peer(), &[peer().ip()]).await.unwrap(), "198.51.100.7:4567".parse().unwrap());
        assert_eq!(data, b"GAME");
        assert_eq!(parse(b"PROXY TCP6 2001:db8::1 ::1 42 26100\r\n").await.unwrap(), "[2001:db8::1]:42".parse().unwrap());
        assert_eq!(parse(b"PROXY UNKNOWN ignored\r\n").await.unwrap(), peer());
    }
    #[tokio::test]
    async fn v2_addresses_and_local() {
        let mut body = vec![198, 51, 100, 8, 127, 0, 0, 1, 0x12, 0x34, 0x65, 0xf4];
        assert_eq!(parse(&v2(0x11, &body)).await.unwrap(), "198.51.100.8:4660".parse().unwrap());
        body = "2001:db8::2".parse::<Ipv6Addr>().unwrap().octets().to_vec();
        body.extend(Ipv6Addr::LOCALHOST.octets());
        body.extend([0, 42, 0x65, 0xf4]);
        assert_eq!(parse(&v2(0x21, &body)).await.unwrap(), "[2001:db8::2]:42".parse().unwrap());
        let mut local = v2(0, b"ignored"); local[12] = 0x20;
        assert_eq!(parse(&local).await.unwrap(), peer());
    }
    #[tokio::test]
    async fn malformed_and_untrusted() {
        for line in [&b"PROXY TCP4 ::1 127.0.0.1 1 2\r\n"[..], b"PROXY TCP4 1.2.3.4 1.2.3.5 65536 2\r\n", b"PROXY  TCP4 1.2.3.4 1.2.3.5 1 2\r\n", b"PROXY TCP4 1.2.3.4 1.2.3.5 +1 2\r\n", b"PROXY", b"BROKEN\r\n"] {
            assert!(parse(line).await.is_err());
        }
        assert!(parse(&[b'P'; 108]).await.is_err());
        assert!(parse(&v2(0x11, &[0; 11])).await.is_err());
        assert!(parse(&v2(0x12, &[0; 12])).await.is_err());
        let mut bad = v2(0x11, &[0; 12]); bad[12] = 0x31;
        assert!(parse(&bad).await.is_err());
        let mut data = &b"PROXY TCP4 198.51.100.7 127.0.0.1 42 26100\r\n"[..];
        let original = data;
        assert_eq!(read(&mut data, peer(), &[]).await.unwrap_err().kind(), io::ErrorKind::PermissionDenied);
        assert_eq!(data, original);
    }
    #[tokio::test(start_paused = true)]
    async fn slow_header_times_out() {
        let (mut reader, _writer) = tokio::io::duplex(128);
        assert_eq!(read(&mut reader, peer(), &[peer().ip()]).await.unwrap_err().kind(), io::ErrorKind::TimedOut);
    }
}
