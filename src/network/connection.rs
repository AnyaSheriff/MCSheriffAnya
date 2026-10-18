// Шифрование всего потока, включая длины и сжатые данные.
// Вики, Java Edition protocol/Encryption: «updated continuously».
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use aes::cipher::{BlockDecryptMut, BlockEncryptMut, KeyIvInit};
use tokio::io::{AsyncRead, AsyncWrite, ReadBuf};

pub type Connection = CryptoStream<tokio::net::TcpStream>;
type Encryptor = cfb8::Encryptor<aes::Aes128>;
type Decryptor = cfb8::Decryptor<aes::Aes128>;

pub struct CryptoStream<S> {
    inner: S,
    encryptor: Option<Encryptor>,
    decryptor: Option<Decryptor>,
    read_taken: bool,
    pub protocol: Option<mcsa_protocol::Negotiated>,
    pub login_session_id: Option<[u8; 16]>,
}

impl<S> CryptoStream<S> {
    pub fn new(inner: S) -> Self {
        Self { inner, encryptor: None, decryptor: None, read_taken: false, protocol: None, login_session_id: None }
    }

    pub fn client_version(&self) -> mcsa_protocol::JavaVersion {
        self.protocol.map_or(mcsa_protocol::wire::INTERNAL, |profile| profile.client)
    }

    pub fn enable_encryption(&mut self, secret: &[u8; 16]) {
        self.encryptor = Some(Encryptor::new(secret.into(), secret.into()));
        self.decryptor = Some(Decryptor::new(secret.into(), secret.into()));
    }
}

impl Connection {
    /// Передаёт состояние чтения отдельной задаче Play ровно один раз.
    pub fn take_reader(&mut self) -> io::Result<Self> {
        if self.read_taken {
            return Err(io::Error::other("читатель уже передан"));
        }
        #[cfg(unix)]
        let own = {
            use std::os::fd::AsFd;
            std::net::TcpStream::from(self.inner.as_fd().try_clone_to_owned()?)
        };
        #[cfg(windows)]
        let own = {
            use std::os::windows::io::AsSocket;
            std::net::TcpStream::from(self.inner.as_socket().try_clone_to_owned()?)
        };
        own.set_nonblocking(true)?;
        let mut reader = Self::new(tokio::net::TcpStream::from_std(own)?);
        reader.decryptor = self.decryptor.take();
        reader.protocol = self.protocol;
        reader.login_session_id = self.login_session_id;
        self.read_taken = true;
        Ok(reader)
    }
}

fn encrypt(cipher: &mut Encryptor, bytes: &mut [u8]) {
    for byte in bytes {
        cipher.encrypt_block_mut(std::array::from_mut(byte).into());
    }
}

fn decrypt(cipher: &mut Decryptor, bytes: &mut [u8]) {
    for byte in bytes {
        cipher.decrypt_block_mut(std::array::from_mut(byte).into());
    }
}

impl<S: AsyncRead + Unpin> AsyncRead for CryptoStream<S> {
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        let this = self.get_mut();
        if this.read_taken {
            return Poll::Ready(Err(io::Error::other("читатель уже передан")));
        }
        let start = buf.filled().len();
        ready!(Pin::new(&mut this.inner).poll_read(cx, buf))?;
        if let Some(cipher) = &mut this.decryptor {
            decrypt(cipher, &mut buf.filled_mut()[start..]);
        }
        Poll::Ready(Ok(()))
    }
}

impl<S: AsyncWrite + Unpin> AsyncWrite for CryptoStream<S> {
    fn poll_write(self: Pin<&mut Self>, cx: &mut Context<'_>, bytes: &[u8]) -> Poll<io::Result<usize>> {
        let this = self.get_mut();
        let Some(cipher) = &mut this.encryptor else {
            return Pin::new(&mut this.inner).poll_write(cx, bytes);
        };
        // Pending и короткая запись не должны продвигать шифр дальше
        // реально отправленных байтов. Буфер ограничен независимо от пакета.
        let len = bytes.len().min(8192);
        let mut buffer = [0; 8192];
        buffer[..len].copy_from_slice(&bytes[..len]);
        let mut next = cipher.clone();
        encrypt(&mut next, &mut buffer[..len]);
        let written = ready!(Pin::new(&mut this.inner).poll_write(cx, &buffer[..len]))?;
        if written == len {
            *cipher = next;
        } else {
            buffer[..written].copy_from_slice(&bytes[..written]);
            encrypt(cipher, &mut buffer[..written]);
        }
        Poll::Ready(Ok(written))
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.get_mut().inner).poll_shutdown(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn hex<const N: usize>(text: &str) -> [u8; N] {
        std::array::from_fn(|i| u8::from_str_radix(&text[i * 2..i * 2 + 2], 16).unwrap())
    }

    #[test]
    fn cfb8_nist_sp800_38a_vector() {
        // NIST SP 800-38A, F.3.7: AES-128-CFB8, первый блок.
        let key = hex::<16>("2b7e151628aed2a6abf7158809cf4f3c");
        let iv = hex::<16>("000102030405060708090a0b0c0d0e0f");
        let plain = hex::<16>("6bc1bee22e409f96e93d7e117393172a");
        let expected = hex::<16>("3b79424c9c0dd436bace9e0ed4586a4f");
        let mut cipher = Encryptor::new(&key.into(), &iv.into());
        let mut data = plain;
        encrypt(&mut cipher, &mut data[..3]);
        encrypt(&mut cipher, &mut data[3..]);
        assert_eq!(data, expected);
        let mut cipher = Decryptor::new(&key.into(), &iv.into());
        for chunk in data.chunks_mut(5) { decrypt(&mut cipher, chunk); }
        assert_eq!(data, plain);
    }

    #[tokio::test]
    async fn short_writes_pending_and_bidirectional_cipher_continuity() {
        let (left, right) = tokio::io::duplex(3);
        let mut left = CryptoStream::new(left);
        let mut right = CryptoStream::new(right);
        let secret = *b"0123456789abcdef";
        left.enable_encryption(&secret);
        right.enable_encryption(&secret);
        let data: Vec<u8> = (0..17000).map(|i| (i % 251) as u8).collect();
        let sender = async {
            left.write_all(&data[..17]).await.unwrap();
            left.write_all(&data[17..]).await.unwrap();
            let mut answer = [0; 16];
            left.read_exact(&mut answer).await.unwrap();
            assert_eq!(answer, secret);
        };
        let receiver = async {
            let mut received = vec![0; data.len()];
            right.read_exact(&mut received).await.unwrap();
            assert_eq!(received, data);
            right.write_all(&secret).await.unwrap();
        };
        tokio::join!(sender, receiver);
    }

    #[tokio::test]
    async fn play_reader_keeps_negotiated_versions_and_cannot_be_taken_twice() {
        let listener=tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let peer=tokio::net::TcpStream::connect(listener.local_addr().unwrap());
        let (peer,accepted)=tokio::join!(peer,listener.accept());
        let mut peer=peer.unwrap();
        let mut writer=Connection::new(accepted.unwrap().0);
        let protocol=mcsa_protocol::Negotiated::native_only(775,super::super::NATIVE_VERSION).unwrap();
        writer.protocol=Some(protocol);
        writer.login_session_id=Some([23; 16]);
        let mut reader=writer.take_reader().unwrap();
        assert_eq!(reader.protocol,Some(protocol));
        assert_eq!(reader.login_session_id,Some([23; 16]));
        assert_eq!(writer.protocol,Some(protocol));
        assert!(writer.take_reader().is_err());
        peer.write_all(b"play").await.unwrap();
        let mut bytes=[0;4];
        reader.read_exact(&mut bytes).await.unwrap();
        assert_eq!(&bytes,b"play");
    }

    #[tokio::test]
    async fn offline_stream_is_plaintext() {
        let (raw, mut peer) = tokio::io::duplex(32);
        let mut connection = CryptoStream::new(raw);
        connection.write_all(b"plaintext").await.unwrap();
        let mut data = [0; 9];
        peer.read_exact(&mut data).await.unwrap();
        assert_eq!(&data, b"plaintext");
        peer.write_all(b"unchanged").await.unwrap();
        connection.read_exact(&mut data).await.unwrap();
        assert_eq!(&data, b"unchanged");
    }

    #[tokio::test]
    async fn read_does_not_decrypt_previously_filled_bytes() {
        let (raw, peer) = tokio::io::duplex(32);
        let mut reader = CryptoStream::new(raw);
        let mut writer = CryptoStream::new(peer);
        reader.enable_encryption(&[7; 16]);
        writer.enable_encryption(&[7; 16]);
        writer.write_all(b"new").await.unwrap();
        let mut data = [0; 6];
        let mut buf = ReadBuf::new(&mut data);
        buf.put_slice(b"old");
        std::future::poll_fn(|cx| Pin::new(&mut reader).poll_read(cx, &mut buf)).await.unwrap();
        assert_eq!(buf.filled(), b"oldnew");
    }
}
