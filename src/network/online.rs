// Java online-mode: вики, Java Edition protocol/Encryption,
// «1024-bit RSA keypair», «PKCS#1 v1.5», «two's complement».
use std::io;
use std::net::IpAddr;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use rsa::{Pkcs1v15Encrypt, RsaPrivateKey, pkcs8::EncodePublicKey};
use rsa::rand_core::{OsRng, RngCore};
use sha1::{Digest, Sha1};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};

use super::connection::CryptoStream;
use super::login::valid_name;
use super::packet::{decode_varint, write_bytes};
use super::varint::{encode_string, encode_varint, read_varint};

const SESSION_URL: &str = "https://sessionserver.mojang.com/session/minecraft/hasJoined";
const AUTH_TIMEOUT: Duration = Duration::from_secs(10);
static AUTH: OnceLock<Authenticator> = OnceLock::new();

pub struct Authenticator {
    key: Arc<RsaPrivateKey>,
    public_key: Vec<u8>,
}

#[derive(Debug)]
pub struct Profile {
    pub uuid: [u8; 16],
    pub name: String,
    pub skin: Option<crate::skins::Skin>,
}

#[derive(Debug)]
pub enum Failure {
    Unverified,
    Unavailable,
    Protocol(io::Error),
}

impl Failure {
    pub fn translation(&self) -> &'static str {
        match self {
            Self::Unavailable => "multiplayer.disconnect.authservers_down",
            _ => "multiplayer.disconnect.unverified_username",
        }
    }
}

impl From<io::Error> for Failure {
    fn from(error: io::Error) -> Self { Self::Protocol(error) }
}

impl Authenticator {
    fn generate() -> io::Result<Self> {
        let key = RsaPrivateKey::new(&mut OsRng, 1024).map_err(io::Error::other)?;
        let public_key = key.to_public_key().to_public_key_der().map_err(io::Error::other)?.as_bytes().to_vec();
        Ok(Self { key: Arc::new(key), public_key })
    }

    pub fn get() -> io::Result<&'static Self> {
        AUTH.get().ok_or_else(|| io::Error::other("ключ online-mode не создан"))
    }

    pub async fn authenticate<S: AsyncRead + AsyncWrite + Unpin>(
        &self, stream: &mut CryptoStream<S>, name: &str, ip: Option<IpAddr>,
    ) -> Result<Profile, Failure> {
        self.authenticate_with(stream, |hash| has_joined(name, hash, ip)).await
    }

    async fn authenticate_with<S, F, Fut>(
        &self, stream: &mut CryptoStream<S>, lookup: F,
    ) -> Result<Profile, Failure>
    where
        S: AsyncRead + AsyncWrite + Unpin,
        F: FnOnce(String) -> Fut,
        Fut: std::future::Future<Output = Result<Profile, Failure>>,
    {
        let mut token = [0; 4];
        OsRng.try_fill_bytes(&mut token).map_err(io::Error::other)?;
        let mut request = encode_varint(1);
        request.extend(encode_string(""));
        put_array(&mut request, &self.public_key);
        put_array(&mut request, &token);
        if stream.client_version() != mcsa_protocol::JavaVersion::V1_12_2 {
            request.push(1); // should_authenticate=true, поля нет в340
        }
        let mut frame = encode_varint(request.len() as i32);
        frame.extend(request);
        write_bytes(stream, &frame).await?;

        let response = tokio::time::timeout(Duration::from_secs(30), read_response(stream)).await
            .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "Encryption Response не получен"))??;
        let key = self.key.clone();
        // RSA не занимает рабочий поток Tokio; операция с закрытым ключом
        // использует ослепление из библиотеки rsa.
        let secret = tokio::task::spawn_blocking(move || decrypt_response(&key, response, &token)).await
            .map_err(io::Error::other)??;
        stream.enable_encryption(&secret);
        let hash = server_hash("", &secret, &self.public_key);
        lookup(hash).await
    }
}

pub async fn initialize() -> io::Result<()> {
    let auth = tokio::task::spawn_blocking(Authenticator::generate).await.map_err(io::Error::other)??;
    AUTH.set(auth).map_err(|_| io::Error::other("ключ online-mode уже создан"))
}

fn put_array(out: &mut Vec<u8>, value: &[u8]) {
    out.extend(encode_varint(value.len() as i32));
    out.extend_from_slice(value);
}

async fn read_response<R: AsyncRead + Unpin>(stream: &mut R) -> io::Result<(Vec<u8>, Vec<u8>)> {
    let length = read_varint(stream).await?;
    if !(1..=512).contains(&length) {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "длина Encryption Response"));
    }
    let mut body = vec![0; length as usize];
    stream.read_exact(&mut body).await?;
    parse_response(&body)
}

fn parse_response(body: &[u8]) -> io::Result<(Vec<u8>, Vec<u8>)> {
    let invalid = || io::Error::new(io::ErrorKind::InvalidData, "неверный Encryption Response");
    let (id, mut offset) = decode_varint(body)?;
    if id != 1 { return Err(invalid()); }
    let mut array = || -> io::Result<Vec<u8>> {
        let (length, size) = decode_varint(&body[offset..])?;
        offset += size;
        if length != 128 { return Err(invalid()); }
        let end = offset + length as usize;
        let bytes = body.get(offset..end).ok_or_else(invalid)?.to_vec();
        offset = end;
        Ok(bytes)
    };
    let secret = array()?;
    let token = array()?;
    if offset != body.len() { return Err(invalid()); }
    Ok((secret, token))
}

fn decrypt_response(key: &RsaPrivateKey, (secret, token): (Vec<u8>, Vec<u8>), expected: &[u8; 4]) -> Result<[u8; 16], Failure> {
    let secret = key.decrypt_blinded(&mut OsRng, Pkcs1v15Encrypt, &secret);
    let token = key.decrypt_blinded(&mut OsRng, Pkcs1v15Encrypt, &token);
    let (Ok(secret), Ok(token)) = (secret, token) else { return Err(Failure::Unverified); };
    if token.len() != expected.len() || token.iter().zip(expected).fold(0, |diff, (a, b)| diff | (a ^ b)) != 0 {
        return Err(Failure::Unverified);
    }
    secret.try_into().map_err(|_| Failure::Unverified)
}

fn server_hash(id: &str, secret: &[u8], public_key: &[u8]) -> String {
    let mut hash = Sha1::new();
    hash.update(id.as_bytes());
    hash.update(secret);
    hash.update(public_key);
    signed_hex(hash.finalize().into())
}

fn signed_hex(mut digest: [u8; 20]) -> String {
    let negative = digest[0] & 0x80 != 0;
    if negative {
        let mut carry = true;
        for byte in digest.iter_mut().rev() {
            let (value, overflow) = (!*byte).overflowing_add(u8::from(carry));
            *byte = value;
            carry = overflow;
        }
    }
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    let hex = hex.trim_start_matches('0');
    if hex.is_empty() { return "0".into(); }
    format!("{}{hex}", if negative { "-" } else { "" })
}

fn session_request(client: &reqwest::Client, name: &str, hash: &str, ip: Option<IpAddr>) -> Result<reqwest::Request, Failure> {
    let mut url = reqwest::Url::parse(SESSION_URL).expect("постоянный URL");
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("username", name).append_pair("serverId", hash);
        if let Some(ip) = ip { query.append_pair("ip", &ip.to_string()); }
    }
    client.get(url).timeout(AUTH_TIMEOUT).build().map_err(|_| Failure::Unavailable)
}

async fn has_joined(name: &str, hash: String, ip: Option<IpAddr>) -> Result<Profile, Failure> {
    let client = crate::skins::client().ok_or(Failure::Unavailable)?;
    let request = session_request(client, name, &hash, ip)?;
    let response = client.execute(request).await.map_err(|_| Failure::Unavailable)?;
    session_profile(response, name).await
}

async fn session_profile(mut response: reqwest::Response, name: &str) -> Result<Profile, Failure> {
    if response.status() != reqwest::StatusCode::OK { return Err(Failure::Unverified); }
    // Ограничение ответа защищает память при ошибке удалённой службы.
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| Failure::Unavailable)? {
        if body.len() + chunk.len() > 1024 * 1024 { return Err(Failure::Unverified); }
        body.extend_from_slice(&chunk);
    }
    parse_profile(&body, name)
}

fn parse_profile(body: &[u8], requested: &str) -> Result<Profile, Failure> {
    #[derive(serde::Deserialize)]
    struct Response {
        id: String,
        name: String,
        #[serde(default)]
        properties: Vec<Property>,
    }
    #[derive(serde::Deserialize)]
    struct Property {
        name: String,
        value: String,
        #[serde(default)]
        signature: String,
    }
    let response: Response = serde_json::from_slice(body).map_err(|_| Failure::Unverified)?;
    if !valid_name(&response.name) || !response.name.eq_ignore_ascii_case(requested) {
        return Err(Failure::Unverified);
    }
    let id = response.id.as_bytes();
    if id.len() != 32 || !id.iter().all(u8::is_ascii_hexdigit) { return Err(Failure::Unverified); }
    let mut uuid = [0; 16];
    for (index, byte) in uuid.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&response.id[index * 2..index * 2 + 2], 16).map_err(|_| Failure::Unverified)?;
    }
    let skin = response.properties.into_iter().find(|p| p.name == "textures")
        .map(|p| crate::skins::Skin { value: p.value, signature: p.signature });
    Ok(Profile { uuid, name: response.name, skin })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsa::{RsaPublicKey, pkcs8::DecodePublicKey, traits::PublicKeyParts};
    use tokio::io::AsyncWriteExt;

    const PROFILE: &[u8] = br#"{"id":"069a79f444e94726a5befca90e38aaf5","name":"Notch","properties":[{"name":"textures","value":"skin","signature":"signed"}]}"#;

    fn auth() -> &'static Authenticator {
        static KEY: OnceLock<Authenticator> = OnceLock::new();
        KEY.get_or_init(|| Authenticator::generate().unwrap())
    }

    #[test]
    fn minecraft_signed_sha1_vectors() {
        for (input, expected) in [
            ("Notch", "4ed1f46bbe04bc756bcb17c0c7ce3e4632f06a48"),
            ("jeb_", "-7c9d5b0044c130109a5d7b5fb5c317c02b4e28c1"),
            ("simon", "88e16a1019277b15d58faf0541e11910eb756f6"),
        ] {
            assert_eq!(server_hash(input, &[], &[]), expected);
        }
        assert_eq!(signed_hex([0; 20]), "0");
        assert_eq!(signed_hex([255; 20]), "-1");
        let mut min = [0; 20];
        min[0] = 0x80;
        assert_eq!(signed_hex(min), "-8000000000000000000000000000000000000000");
        assert_eq!(server_hash("a", b"b", b"c"), server_hash("abc", &[], &[]));
    }

    #[test]
    fn rsa_rejects_wrong_token_bad_padding_and_wrong_secret_length() {
        let auth = auth();
        let encrypt = |bytes: &[u8]| auth.key.to_public_key().encrypt(&mut OsRng, Pkcs1v15Encrypt, bytes).unwrap();
        let token = [1, 2, 3, 4];
        assert_eq!(decrypt_response(&auth.key, (encrypt(&[9; 16]), encrypt(&token)), &token).unwrap(), [9; 16]);
        for response in [
            (encrypt(&[9; 16]), encrypt(&[1, 2, 3, 5])),
            (encrypt(&[9; 15]), encrypt(&token)),
            (encrypt(&[9; 17]), encrypt(&token)),
            (vec![0; 128], encrypt(&token)),
            (encrypt(&[9; 16]), vec![0; 128]),
        ] {
            assert!(matches!(decrypt_response(&auth.key, response, &token), Err(Failure::Unverified)));
        }
    }

    #[test]
    fn response_packet_is_bounded_and_exact() {
        let mut body = vec![1];
        put_array(&mut body, &[7; 128]);
        put_array(&mut body, &[8; 128]);
        assert_eq!(parse_response(&body).unwrap(), (vec![7; 128], vec![8; 128]));
        for len in 0..body.len() { assert!(parse_response(&body[..len]).is_err()); }
        body.push(0);
        assert!(parse_response(&body).is_err());
        assert!(parse_response(&[1, 255, 255, 255, 255, 15]).is_err());
        assert!(parse_response(&[2]).is_err());
    }

    #[test]
    fn session_profile_supplies_identity_and_signed_skin() {
        let profile = parse_profile(PROFILE, "notch").unwrap();
        assert_eq!(profile.name, "Notch");
        assert_eq!(super::super::login::format_uuid(&profile.uuid), "069a79f4-44e9-4726-a5be-fca90e38aaf5");
        let skin = profile.skin.unwrap();
        assert_eq!((skin.value.as_str(), skin.signature.as_str()), ("skin", "signed"));
        assert!(parse_profile(PROFILE, "jeb_").is_err());
        for body in [b"{}".as_slice(), b"null", b"invalid", br#"{"id":"../","name":"Notch"}"#,
            br#"{"id":"zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz","name":"Notch"}"#] {
            assert!(parse_profile(body, "Notch").is_err());
        }
        assert!(parse_profile(br#"{"id":"069a79f444e94726a5befca90e38aaf5","name":"Notch"}"#, "Notch").unwrap().skin.is_none());
    }

    #[test]
    fn session_query_and_failure_translations() {
        let client = reqwest::Client::new();
        for ip in [None, Some("2001:db8::1234".parse().unwrap())] {
            let request = session_request(&client, "Notch", "-abc", ip).unwrap();
            assert_eq!(request.method(), reqwest::Method::GET);
            assert_eq!(request.url().scheme(), "https");
            assert_eq!(request.url().host_str(), Some("sessionserver.mojang.com"));
            let pairs: std::collections::HashMap<_, _> = request.url().query_pairs().collect();
            assert_eq!(pairs["username"], "Notch");
            assert_eq!(pairs["serverId"], "-abc");
            assert_eq!(pairs.get("ip").map(|s| s.to_string()), ip.map(|ip| ip.to_string()));
            assert_eq!(request.timeout(), Some(&AUTH_TIMEOUT));
        }
        assert_eq!(Failure::Unverified.translation(), "multiplayer.disconnect.unverified_username");
        assert_eq!(Failure::Unavailable.translation(), "multiplayer.disconnect.authservers_down");
    }

    #[tokio::test]
    async fn session_http_status_and_json_fail_closed() {
        for status in [204, 301, 302, 307, 400, 403, 429, 500, 503] {
            let response = http::Response::builder().status(status).body(PROFILE.to_vec()).unwrap();
            assert!(matches!(session_profile(response.into(), "notch").await, Err(Failure::Unverified)));
        }
        for bytes in [b"invalid".as_slice(), b"{}", b""] {
            let response = http::Response::builder().status(200).body(bytes.to_vec()).unwrap();
            assert!(matches!(session_profile(response.into(), "notch").await, Err(Failure::Unverified)));
        }
    }

    async fn raw_packet<R: AsyncRead + Unpin>(stream: &mut R) -> Vec<u8> {
        let length = read_varint(stream).await.unwrap();
        let mut body = vec![0; length as usize];
        stream.read_exact(&mut body).await.unwrap();
        body
    }

    async fn client_exchange<S: AsyncRead + AsyncWrite + Unpin>(client: &mut CryptoStream<S>) {
        let request = raw_packet(client).await;
        assert_eq!(&request[..2], &[1, 0]); // Encryption Request, пустой server id
        let (length, size) = decode_varint(&request[2..]).unwrap();
        let end = 2 + size + length as usize;
        let public = RsaPublicKey::from_public_key_der(&request[2 + size..end]).unwrap();
        assert_eq!(public.n().bits(), 1024);
        assert_eq!(request[end], 4);
        if client.client_version() == mcsa_protocol::JavaVersion::V1_12_2 {
            assert!(request[end + 5..].is_empty());
        } else { assert_eq!(request[end + 5..], [1]); }
        let mut body = vec![1];
        put_array(&mut body, &public.encrypt(&mut OsRng, Pkcs1v15Encrypt, &[9; 16]).unwrap());
        put_array(&mut body, &public.encrypt(&mut OsRng, Pkcs1v15Encrypt, &request[end + 1..end + 5]).unwrap());
        let mut packet = encode_varint(body.len() as i32);
        packet.extend(body);
        client.write_all(&packet).await.unwrap();
        client.enable_encryption(&[9; 16]);
    }

    #[tokio::test]
    async fn login_with_in_memory_session_service() {
        use mcsa_protocol::{JavaVersion, Negotiated};
        let auth = auth();
        // Один и тот же RSA/session-service путь для775/776/777, без Mojang аккаунта.
        for version in [JavaVersion::V26_1_2, JavaVersion::V26_2, JavaVersion::V26_3] {
            let (server, client) = tokio::io::duplex(7);
            let mut server = CryptoStream::new(server);
            let mut client = CryptoStream::new(client);
            let route = Negotiated { client: version, native: JavaVersion::V26_1_2 };
            server.protocol = Some(route); client.protocol = Some(route);
            let serving = async {
                let login = super::super::login::read_login_start(&mut server).await.unwrap();
                assert_eq!(login.username, "notch");
                assert_eq!(login.uuid, [99; 16]);
                let profile = auth.authenticate_with(&mut server, |hash| async move {
                    assert_eq!(hash, server_hash("", &[9; 16], &auth.public_key));
                    let response = http::Response::builder().status(200).body(PROFILE.to_vec()).unwrap();
                    session_profile(response.into(), "notch").await
                }).await.unwrap();
                super::super::login::send_set_compression(&mut server, 32).await.unwrap();
                super::super::login::send_login_success(&mut server, &profile.name, &profile.uuid, profile.skin.as_ref(), 32).await.unwrap();
                assert_eq!(raw_packet(&mut server).await, [0, 3]); // Login Acknowledged, уже шифрован
            };
            let joining = async {
                let mut start = vec![0];
                start.extend(encode_string("notch"));
                start.extend([99; 16]);
                let mut frame = encode_varint(start.len() as i32);
                frame.extend(start);
                client.write_all(&frame).await.unwrap();
                client_exchange(&mut client).await;
                assert_eq!(raw_packet(&mut client).await, [3, 32]);
                let packed = raw_packet(&mut client).await;
                let (length, offset) = decode_varint(&packed).unwrap();
                let mut body = Vec::new();
                std::io::Read::read_to_end(&mut flate2::read::ZlibDecoder::new(&packed[offset..]), &mut body).unwrap();
                assert_eq!(length as usize, body.len());
                assert_eq!(body[0], 2);
                assert_eq!(&body[1..17], &parse_profile(PROFILE, "Notch").unwrap().uuid);
                let mut offset = 17;
                assert_eq!(super::super::packet::decode_string(&body, &mut offset).unwrap(), "Notch");
                assert_eq!(body[offset], 1);
                offset += 1;
                assert_eq!(super::super::packet::decode_string(&body, &mut offset).unwrap(), "textures");
                assert_eq!(super::super::packet::decode_string(&body, &mut offset).unwrap(), "skin");
                assert_eq!(body[offset], 1);
                offset += 1;
                assert_eq!(super::super::packet::decode_string(&body, &mut offset).unwrap(), "signed");
                if version != JavaVersion::V26_1_2 {
                    assert_eq!(body.len() - offset, 16);
                    let session: [u8; 16] = body[offset..].try_into().unwrap();
                    assert_ne!(session, [0; 16]);
                    assert_ne!(session, parse_profile(PROFILE, "Notch").unwrap().uuid);
                    offset += 16;
                }
                assert_eq!(offset, body.len());
                client.write_all(&[2, 0, 3]).await.unwrap();
            };
            tokio::join!(serving, joining);
        }
    }

    #[tokio::test]
    async fn legacy_login_encrypts_compresses_and_uses_verified_identity() {
        use mcsa_protocol::{JavaVersion, Negotiated, State, legacy_control::Packet};
        let auth = auth();
        let (server, client) = tokio::io::duplex(8192);
        let mut server = CryptoStream::new(server);
        let mut client = CryptoStream::new(client);
        let protocol = Negotiated { client: JavaVersion::V1_12_2, native: JavaVersion::V26_1_2 };
        server.protocol = Some(protocol); client.protocol = Some(protocol);
        let serving = async {
            let profile = auth.authenticate_with(&mut server, |_| async {
                Ok(parse_profile(PROFILE, "Notch").unwrap())
            }).await.unwrap();
            super::super::login::send_set_compression(&mut server, 32).await.unwrap();
            super::super::login::send_login_success(&mut server, &profile.name, &profile.uuid, profile.skin.as_ref(), 32).await.unwrap();
            assert_eq!(server.login_session_id, None);
        };
        let joining = async {
            client_exchange(&mut client).await;
            assert_eq!(raw_packet(&mut client).await, [3, 32]);
            let packed = raw_packet(&mut client).await;
            let (length, offset) = decode_varint(&packed).unwrap();
            let mut body = Vec::new();
            std::io::Read::read_to_end(&mut flate2::read::ZlibDecoder::new(&packed[offset..]), &mut body).unwrap();
            assert_eq!(length as usize, body.len());
            assert_eq!(body[0], 2);
            assert_eq!(Packet::decode(State::Login, 2, &body[1..]).unwrap(), Packet::LoginSuccess {
                uuid: parse_profile(PROFILE, "Notch").unwrap().uuid, username: "Notch".into(),
            });
        };
        tokio::join!(serving, joining);
    }

    #[tokio::test]
    async fn authentication_failures_disconnect_over_encrypted_stream() {
        for failure in [Failure::Unverified, Failure::Unavailable] {
            let expected = failure.translation();
            let (server, client) = tokio::io::duplex(1024);
            let mut server = CryptoStream::new(server);
            let mut client = CryptoStream::new(client);
            let serving = async {
                let failure = auth().authenticate_with(&mut server, |_| async { Err(failure) }).await.unwrap_err();
                let json = serde_json::json!({"translate": failure.translation()}).to_string();
                super::super::login::send_login_disconnect_json(&mut server, &json).await.unwrap();
            };
            let joining = async {
                client_exchange(&mut client).await;
                let body = raw_packet(&mut client).await;
                assert_eq!(body[0], 0);
                let mut offset = 1;
                let json = super::super::packet::decode_string(&body, &mut offset).unwrap();
                assert_eq!(serde_json::from_str::<serde_json::Value>(&json).unwrap()["translate"], expected);
            };
            tokio::join!(serving, joining);
        }
    }
}
