// Проверка входа Xbox Live у игроков Bedrock.
//
// Клиенты 26.x кладут в пакет Login жетон `Token` — JWT, который выдала
// служба входа Minecraft (в официальной документации Mojang он зовётся
// GameServerToken: bedrock-protocol-docs, additional_docs/
// NetherNetOnboardingGuide.md, раздел 5.1). В нём имя (`xname`), XUID
// (`xid`) и открытый ключ клиента (`cpk`). По той же документации сервер
// проверяет подпись жетона открытыми ключами службы входа, а данными из
// жетона уже распоряжается сам.
//
// Где ключи: издатель жетона (`iss`) — сервис OpenID, его описание лежит
// по адресу `iss` + `.well-known/openid-configuration` (OpenID Connect
// Discovery 1.0, раздел 4), в описании — `jwks_uri`, набор ключей JWK
// (RFC 7517). Подпись — RS256 (RFC 7518, 3.3; заявлено и в описании:
// `id_token_signing_alg_values_supported`), ключ ищется по `kid` из
// заголовка (RFC 7515, 4.1.4).
//
// Второй JWT пакета Login — данные клиента (ClientData, там скин) — клиент
// подписывает сам своим ключом, тем, что указан в жетоне как `cpk` (ES384).
// Проверка этой подписи связывает данные клиента с подтверждённым жетоном.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use aws_lc_rs::signature::{self, RsaPublicKeyComponents, UnparsedPublicKey};
use serde_json::Value;

use super::session::base64_url;
use crate::{log_debug, log_info, log_warn};

/// Издатель жетонов входа — служба входа Minecraft.
pub const ISSUER: &str = "https://authorization.franchise.minecraft-services.net/";

/// Какое расхождение часов с клиентом и службой входа прощается.
const LEEWAY: u64 = 60;

/// Как часто ключи службы входа обновляются сами.
const REFRESH: Duration = Duration::from_secs(60 * 60);

/// Не чаще этого — внеочередное обновление (незнакомый `kid`, сбой сети):
/// поддельными жетонами службу входа не задёргать.
const RETRY: Duration = Duration::from_secs(60);

/// Сколько ждать службу входа на каждый запрос.
const WAIT: Duration = Duration::from_secs(5);

/// Почему вход не подтверждён.
#[derive(Debug, PartialEq)]
pub enum Failure {
    /// Жетона нет: прежний вид входа (цепочка `chain`) или клиент без
    /// учётной записи Xbox.
    NoToken,
    /// JWT не разбирается.
    Malformed(&'static str),
    /// Подпись не тем способом, какой ждём (не RS256 / не ES384).
    WrongAlgorithm(String),
    /// Ключа с таким `kid` у службы входа нет.
    UnknownKey(String),
    /// Подпись жетона не сошлась.
    BadSignature,
    /// Жетон выдан не службой входа Minecraft.
    WrongIssuer(String),
    /// Срок жетона вышел.
    Expired,
    /// Жетон ещё не действует.
    NotYetValid,
    /// В жетоне нет годного ключа клиента `cpk`.
    NoClientKey,
    /// Данные клиента не подписаны ключом из жетона.
    BadClientData,
    /// Ключи службы входа не получены: сеть, служба недоступна.
    Network(String),
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Failure::NoToken => write!(f, "нет жетона Xbox Live (вход без учётной записи или прежнего вида)"),
            Failure::Malformed(what) => write!(f, "жетон не разбирается: {}", what),
            Failure::WrongAlgorithm(alg) => write!(f, "неожиданный способ подписи «{}»", alg),
            Failure::UnknownKey(kid) => write!(f, "у службы входа нет ключа «{}»", kid),
            Failure::BadSignature => write!(f, "подпись жетона не сошлась"),
            Failure::WrongIssuer(iss) => write!(f, "жетон выдан не службой входа Minecraft, а «{}»", iss),
            Failure::Expired => write!(f, "срок жетона вышел"),
            Failure::NotYetValid => write!(f, "жетон ещё не действует"),
            Failure::NoClientKey => write!(f, "в жетоне нет ключа клиента"),
            Failure::BadClientData => write!(f, "данные клиента подписаны не ключом из жетона"),
            Failure::Network(why) => write!(f, "ключи службы входа не получены: {}", why),
        }
    }
}

impl Failure {
    /// Сбой на нашей стороне (сеть), а не негодный вход.
    pub fn is_network(&self) -> bool {
        matches!(self, Failure::Network(_))
    }
}

/// Разобранный JWT (RFC 7519): заголовок, поля, подписанная часть и подпись.
struct Jwt<'a> {
    header: Value,
    claims: Value,
    signed: &'a str,
    signature: Vec<u8>,
}

fn parse_jwt(token: &str) -> Result<Jwt<'_>, Failure> {
    let (signed, signature) = token.rsplit_once('.').ok_or(Failure::Malformed("нет подписи"))?;
    let (header, claims) = signed.split_once('.').ok_or(Failure::Malformed("нет полей"))?;

    let json = |part: &str, what| {
        base64_url(part)
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .filter(Value::is_object)
            .ok_or(Failure::Malformed(what))
    };

    Ok(Jwt {
        header: json(header, "заголовок")?,
        claims: json(claims, "поля")?,
        signed,
        signature: base64_url(signature).filter(|s| !s.is_empty()).ok_or(Failure::Malformed("подпись"))?,
    })
}

/// Ключи службы входа: `kid` → модуль и показатель RSA.
#[derive(Default)]
pub struct KeySet {
    keys: HashMap<String, RsaPublicKeyComponents<Vec<u8>>>,
}

impl KeySet {
    /// Из набора JWK (RFC 7517, раздел 5). Берутся только ключи RSA для
    /// подписи (`use` = `sig` или без `use`), с `kid`.
    pub fn from_jwks(jwks: &Value) -> KeySet {
        let mut keys = HashMap::new();

        for key in jwks.get("keys").and_then(Value::as_array).into_iter().flatten() {
            let text = |name| key.get(name).and_then(Value::as_str);

            if text("kty") != Some("RSA") || text("use").is_some_and(|u| u != "sig") {
                continue;
            }

            if let (Some(kid), Some(n), Some(e)) = (text("kid"), text("n").and_then(base64_url), text("e").and_then(base64_url)) {
                keys.insert(kid.to_string(), RsaPublicKeyComponents { n, e });
            }
        }

        KeySet { keys }
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    pub fn contains(&self, kid: &str) -> bool {
        self.keys.contains_key(kid)
    }
}

/// Секунды от начала эпохи Unix.
fn now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// `kid` из заголовка жетона — какой ключ нужен.
fn key_id(token: &str) -> Result<String, Failure> {
    let jwt = parse_jwt(token)?;
    jwt.header.get("kid").and_then(Value::as_str).map(str::to_string).ok_or(Failure::Malformed("нет kid"))
}

/// Проверяет жетон входа: подпись RS256 ключом с его `kid`, издателя,
/// срок (`exp` обязателен, `nbf` — если есть). Возвращает поля жетона.
///
/// Поле `aud` не сверяется: его значение для входа на сервер в открытых
/// источниках не описано, а ошибка в нём отказала бы всем честным игрокам.
pub fn verify_token(token: &str, keys: &KeySet, now: u64) -> Result<Value, Failure> {
    let jwt = parse_jwt(token)?;

    match jwt.header.get("alg").and_then(Value::as_str) {
        Some("RS256") => {}
        other => return Err(Failure::WrongAlgorithm(other.unwrap_or("").to_string())),
    }

    let kid = jwt.header.get("kid").and_then(Value::as_str).ok_or(Failure::Malformed("нет kid"))?;
    let key = keys.keys.get(kid).ok_or_else(|| Failure::UnknownKey(kid.to_string()))?;

    key.verify(&signature::RSA_PKCS1_2048_8192_SHA256, jwt.signed.as_bytes(), &jwt.signature)
        .map_err(|_| Failure::BadSignature)?;

    // Подпись верна — полям можно верить, остаётся проверить, те ли они.
    let claims = jwt.claims;

    match claims.get("iss").and_then(Value::as_str) {
        Some(ISSUER) => {}
        other => return Err(Failure::WrongIssuer(other.unwrap_or("").to_string())),
    }

    let time = |name| claims.get(name).and_then(Value::as_f64).map(|t| t.max(0.0) as u64);

    match time("exp") {
        Some(exp) if now <= exp.saturating_add(LEEWAY) => {}
        _ => return Err(Failure::Expired),
    }

    if time("nbf").is_some_and(|nbf| now.saturating_add(LEEWAY) < nbf) {
        return Err(Failure::NotYetValid);
    }

    Ok(claims)
}

/// Открытый ключ клиента из поля `cpk`: строкой — Base64 от DER
/// SubjectPublicKeyInfo (как `identityPublicKey` прежних цепочек), или
/// объектом JWK (RFC 7518, 6.2: `kty` EC, `crv` P-384, `x`, `y`) — так
/// `cpk` описан в документации Mojang для NetherNet.
fn client_key(cpk: &Value) -> Result<Vec<u8>, Failure> {
    match cpk {
        Value::String(text) => base64_url(text).filter(|k| !k.is_empty()).ok_or(Failure::NoClientKey),
        Value::Object(jwk) => {
            let text = |name| jwk.get(name).and_then(Value::as_str);

            if text("kty") != Some("EC") || text("crv") != Some("P-384") {
                return Err(Failure::NoClientKey);
            }

            let (x, y) = (text("x").and_then(base64_url), text("y").and_then(base64_url));
            match (x, y) {
                (Some(x), Some(y)) if x.len() == 48 && y.len() == 48 => Ok([&[4u8][..], &x, &y].concat()),
                _ => Err(Failure::NoClientKey),
            }
        }
        _ => Err(Failure::NoClientKey),
    }
}

/// Проверяет подпись данных клиента (второй JWT пакета Login) ключом `cpk`
/// из подтверждённого жетона: ES384, подпись — r‖s по 48 байт (RFC 7518,
/// 3.4); на всякий случай принимается и DER.
pub fn verify_client_data(client_data: &str, cpk: &Value) -> Result<(), Failure> {
    let key = client_key(cpk)?;
    let jwt = parse_jwt(client_data).map_err(|_| Failure::BadClientData)?;

    match jwt.header.get("alg").and_then(Value::as_str) {
        Some("ES384") => {}
        other => return Err(Failure::WrongAlgorithm(other.unwrap_or("").to_string())),
    }

    let algorithm: &dyn signature::VerificationAlgorithm = if jwt.signature.len() == 96 {
        &signature::ECDSA_P384_SHA384_FIXED
    } else {
        &signature::ECDSA_P384_SHA384_ASN1
    };

    UnparsedPublicKey::new(algorithm, &key)
        .verify(jwt.signed.as_bytes(), &jwt.signature)
        .map_err(|_| Failure::BadClientData)
}

/// Проверяет вход целиком: жетон — ключами службы входа, данные клиента —
/// ключом из жетона. Возвращает поля жетона.
pub fn verify_login(token: &str, client_data: &str, keys: &KeySet, now: u64) -> Result<Value, Failure> {
    let claims = verify_token(token, keys, now)?;
    verify_client_data(client_data, claims.get("cpk").ok_or(Failure::NoClientKey)?)?;
    Ok(claims)
}

/// Ключи службы входа, скачанные раньше.
struct Cache {
    keys: Arc<KeySet>,
    /// Когда ключи получены.
    fetched: Option<Instant>,
    /// Когда за ними ходили в последний раз (удачно или нет).
    tried: Option<Instant>,
}

/// Идти ли за ключами: раз в `REFRESH`, а за незнакомым `kid` — сразу,
/// но не чаще раза в `RETRY`.
fn should_fetch(fetched: Option<Instant>, tried: Option<Instant>, known: bool, now: Instant) -> bool {
    let stale = fetched.is_none_or(|at| now.duration_since(at) >= REFRESH);
    let resting = tried.is_some_and(|at| now.duration_since(at) < RETRY);

    (stale || !known) && !resting
}

fn cache() -> &'static tokio::sync::Mutex<Cache> {
    static CACHE: std::sync::OnceLock<tokio::sync::Mutex<Cache>> = std::sync::OnceLock::new();
    CACHE.get_or_init(|| tokio::sync::Mutex::new(Cache { keys: Arc::new(KeySet::default()), fetched: None, tried: None }))
}

/// Ключи для жетона с таким `kid`: из памяти или от службы входа.
///
/// Замок — только свой: пока одни вход ждёт службу, другие ждут его, а не
/// ходят за теми же ключами сами. Миру и игрокам это не мешает.
async fn keys_for(kid: &str) -> Result<Arc<KeySet>, Failure> {
    let mut cache = cache().lock().await;
    let now = Instant::now();

    if should_fetch(cache.fetched, cache.tried, cache.keys.contains(kid), now) {
        cache.tried = Some(now);

        match fetch_keys().await {
            Ok(keys) => {
                log_debug!("Bedrock: ключи службы входа получены ({} шт.)", keys.len());
                cache.keys = Arc::new(keys);
                cache.fetched = Some(now);
            }
            // Прежние ключи остаются в ходу: служба входа меняет их не
            // каждый час, а вход не должен зависеть от её минутной заминки.
            Err(why) => {
                log_warn!("Bedrock: {}", why);

                if cache.fetched.is_none() {
                    return Err(why);
                }
            }
        }
    }

    if cache.fetched.is_none() {
        return Err(Failure::Network("служба входа недавно не ответила".to_string()));
    }

    Ok(Arc::clone(&cache.keys))
}

/// Скачивает ключи: описание OpenID издателя → `jwks_uri` → набор JWK.
async fn fetch_keys() -> Result<KeySet, Failure> {
    let failed = |what: &str, error: String| Failure::Network(format!("{}: {}", what, error));

    let client = client().ok_or_else(|| Failure::Network("HTTP-клиент не создан".to_string()))?;
    let get = |url: String| async move {
        let answer = client.get(&url).send().await.map_err(|e| failed(&url, e.to_string()))?;

        if !answer.status().is_success() {
            return Err(failed(&url, answer.status().to_string()));
        }

        answer.json::<Value>().await.map_err(|e| failed(&url, e.to_string()))
    };

    let discovery = get(format!("{}.well-known/openid-configuration", ISSUER)).await?;

    // Описание должно быть того самого издателя (OpenID Connect Discovery,
    // 4.3), а ключи — лежать по HTTPS.
    if discovery.get("issuer").and_then(Value::as_str) != Some(ISSUER) {
        return Err(Failure::Network("описание OpenID чужого издателя".to_string()));
    }

    let jwks_uri = discovery
        .get("jwks_uri")
        .and_then(Value::as_str)
        .filter(|uri| uri.starts_with("https://"))
        .ok_or_else(|| Failure::Network("в описании OpenID нет jwks_uri".to_string()))?;

    let keys = KeySet::from_jwks(&get(jwks_uri.to_string()).await?);

    if keys.is_empty() {
        return Err(Failure::Network("в наборе ключей нет ни одного ключа RSA".to_string()));
    }

    Ok(keys)
}

fn client() -> Option<&'static reqwest::Client> {
    static CLIENT: std::sync::OnceLock<Option<reqwest::Client>> = std::sync::OnceLock::new();
    CLIENT.get_or_init(|| reqwest::Client::builder().timeout(WAIT).build().ok()).as_ref()
}

/// Проверяет вход игрока Bedrock. `token` — жетон `Token` (нет — вход без
/// Xbox Live), `client_data` — второй JWT пакета Login.
pub async fn check(token: Option<&str>, client_data: &str) -> Result<Value, Failure> {
    let token = token.filter(|t| !t.is_empty()).ok_or(Failure::NoToken)?;
    let keys = keys_for(&key_id(token)?).await?;
    let result = verify_login(token, client_data, &keys, now());

    if let Err(why) = &result {
        log_info!("Bedrock: вход Xbox Live не подтверждён — {}", why);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_lc_rs::rand::SystemRandom;
    use aws_lc_rs::signature::{EcdsaKeyPair, KeyPair, RsaKeyPair};

    fn b64(bytes: &[u8]) -> String {
        const ABC: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        let mut out = String::new();

        for chunk in bytes.chunks(3) {
            let n = chunk.iter().enumerate().fold(0u32, |n, (i, b)| n | ((*b as u32) << (16 - 8 * i)));
            for i in 0..=chunk.len() {
                out.push(ABC[((n >> (18 - 6 * i)) & 63) as usize] as char);
            }
        }

        out
    }

    /// Модуль и показатель из DER RSAPublicKey (RFC 8017, A.1.1):
    /// SEQUENCE { INTEGER n, INTEGER e }.
    fn rsa_components(der: &[u8]) -> (Vec<u8>, Vec<u8>) {
        fn item(der: &[u8]) -> (u8, &[u8], &[u8]) {
            let (tag, first) = (der[0], der[1] as usize);
            let (length, start) = if first < 0x80 {
                (first, 2)
            } else {
                let size = first & 0x7f;
                (der[2..2 + size].iter().fold(0usize, |l, b| (l << 8) | *b as usize), 2 + size)
            };
            (tag, &der[start..start + length], &der[start + length..])
        }
        let strip = |v: &[u8]| v.iter().skip_while(|b| **b == 0).copied().collect::<Vec<u8>>();

        let (_, body, _) = item(der);
        let (_, n, rest) = item(body);
        let (_, e, _) = item(rest);
        (strip(n), strip(e))
    }

    struct Service {
        key: RsaKeyPair,
        keys: KeySet,
    }

    fn service() -> Service {
        let key = RsaKeyPair::generate(aws_lc_rs::rsa::KeySize::Rsa2048).expect("ключ RSA");
        let (n, e) = rsa_components(key.public_key().as_ref());
        let jwks = serde_json::json!({ "keys": [
            { "kty": "RSA", "use": "sig", "kid": "ПРОБНЫЙ", "n": b64(&n), "e": b64(&e) },
            { "kty": "EC", "kid": "не тот", "crv": "P-256", "x": "AA", "y": "AA" },
        ]});
        Service { key, keys: KeySet::from_jwks(&jwks) }
    }

    fn sign_rs256(key: &RsaKeyPair, kid: &str, claims: &Value) -> String {
        let header = serde_json::json!({ "alg": "RS256", "kid": kid, "typ": "JWT" });
        let signed = format!("{}.{}", b64(header.to_string().as_bytes()), b64(claims.to_string().as_bytes()));
        let mut signature = vec![0; key.public_modulus_len()];
        key.sign(&signature::RSA_PKCS1_SHA256, &SystemRandom::new(), signed.as_bytes(), &mut signature).expect("подпись");
        format!("{}.{}", signed, b64(&signature))
    }

    fn client_key_pair() -> EcdsaKeyPair {
        EcdsaKeyPair::generate(&signature::ECDSA_P384_SHA384_FIXED_SIGNING).expect("ключ P-384")
    }

    /// Открытый ключ клиента так, как его кладут в `cpk`: Base64 от DER
    /// SubjectPublicKeyInfo (RFC 5480) кривой P-384.
    fn cpk_of(key: &EcdsaKeyPair) -> String {
        const SPKI_P384: [u8; 23] = [
            0x30, 0x76, 0x30, 0x10, 0x06, 0x07, 0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01, 0x06, 0x05, 0x2b, 0x81, 0x04, 0x00,
            0x22, 0x03, 0x62, 0x00,
        ];
        b64(&[&SPKI_P384[..], key.public_key().as_ref()].concat())
    }

    fn client_data(key: &EcdsaKeyPair) -> String {
        let header = serde_json::json!({ "alg": "ES384", "x5u": cpk_of(key) });
        let claims = serde_json::json!({ "SkinId": "проба" });
        let signed = format!("{}.{}", b64(header.to_string().as_bytes()), b64(claims.to_string().as_bytes()));
        let signature = key.sign(&SystemRandom::new(), signed.as_bytes()).expect("подпись");
        format!("{}.{}", signed, b64(signature.as_ref()))
    }

    const NOW: u64 = 1_790_000_000;

    fn claims(cpk: &str) -> Value {
        serde_json::json!({
            "iss": ISSUER, "sub": "проба", "xid": "2535425101461547", "xname": "SheriffAnya",
            "cpk": cpk, "iat": NOW - 10, "nbf": NOW - 10, "exp": NOW + 3600,
        })
    }

    #[test]
    fn honest_login_passes() {
        let service = service();
        let player = client_key_pair();
        let token = sign_rs256(&service.key, "ПРОБНЫЙ", &claims(&cpk_of(&player)));

        let verified = verify_login(&token, &client_data(&player), &service.keys, NOW).expect("честный вход");
        assert_eq!(verified["xname"], "SheriffAnya");
        assert_eq!(verified["xid"], "2535425101461547");

        // Расхождение часов в пределах минуты прощается.
        assert!(verify_token(&token, &service.keys, NOW + 3600 + LEEWAY).is_ok());
        assert!(verify_token(&token, &service.keys, NOW - 10 - LEEWAY).is_ok());
    }

    #[test]
    fn expired_token_fails() {
        let service = service();
        let token = sign_rs256(&service.key, "ПРОБНЫЙ", &claims("x"));
        assert_eq!(verify_token(&token, &service.keys, NOW + 3600 + LEEWAY + 1).err(), Some(Failure::Expired));
        assert_eq!(verify_token(&token, &service.keys, NOW - 11 - LEEWAY).err(), Some(Failure::NotYetValid));

        // Без срока вовсе — тоже нет.
        let mut endless = claims("x");
        endless.as_object_mut().expect("объект").remove("exp");
        let token = sign_rs256(&service.key, "ПРОБНЫЙ", &endless);
        assert_eq!(verify_token(&token, &service.keys, NOW).err(), Some(Failure::Expired));
    }

    #[test]
    fn unknown_kid_fails() {
        let service = service();
        let token = sign_rs256(&service.key, "ЧУЖОЙ", &claims("x"));
        assert_eq!(verify_token(&token, &service.keys, NOW).err(), Some(Failure::UnknownKey("ЧУЖОЙ".to_string())));

        // Ключ не RSA с тем же kid в набор не попадает.
        assert!(!service.keys.contains("не тот"));
    }

    #[test]
    fn broken_signature_fails() {
        let service = service();
        let token = sign_rs256(&service.key, "ПРОБНЫЙ", &claims("x"));

        // Подменённые поля при прежней подписи.
        let (header, rest) = token.split_once('.').expect("заголовок");
        let (_, signature) = rest.split_once('.').expect("подпись");
        let mut forged = claims("x");
        forged["xname"] = "Владелец".into();
        let forged = format!("{}.{}.{}", header, b64(forged.to_string().as_bytes()), signature);
        assert_eq!(verify_token(&forged, &service.keys, NOW).err(), Some(Failure::BadSignature));

        // Испорченный байт подписи.
        let mut bytes = base64_url(signature).expect("подпись");
        bytes[10] ^= 1;
        let spoiled = format!("{}.{}", token.rsplit_once('.').expect("точка").0, b64(&bytes));
        assert_eq!(verify_token(&spoiled, &service.keys, NOW).err(), Some(Failure::BadSignature));

        // Подписан чужим ключом с тем же kid.
        let stranger = RsaKeyPair::generate(aws_lc_rs::rsa::KeySize::Rsa2048).expect("ключ RSA");
        let token = sign_rs256(&stranger, "ПРОБНЫЙ", &claims("x"));
        assert_eq!(verify_token(&token, &service.keys, NOW).err(), Some(Failure::BadSignature));

        // «Без подписи» (alg none) не принимается.
        let none = format!("{}.{}.", b64(r#"{"alg":"none","kid":"ПРОБНЫЙ"}"#.as_bytes()), b64(claims("x").to_string().as_bytes()));
        assert!(verify_token(&none, &service.keys, NOW).is_err());
    }

    #[test]
    fn wrong_issuer_fails() {
        let service = service();
        let mut other = claims("x");
        other["iss"] = "https://example.com/".into();
        let token = sign_rs256(&service.key, "ПРОБНЫЙ", &other);
        assert_eq!(
            verify_token(&token, &service.keys, NOW).err(),
            Some(Failure::WrongIssuer("https://example.com/".to_string()))
        );
    }

    #[test]
    fn client_data_must_be_signed_by_token_key() {
        let service = service();
        let (player, stranger) = (client_key_pair(), client_key_pair());
        let token = sign_rs256(&service.key, "ПРОБНЫЙ", &claims(&cpk_of(&player)));

        // Чужие данные клиента (подписаны не ключом из жетона).
        assert_eq!(verify_login(&token, &client_data(&stranger), &service.keys, NOW).err(), Some(Failure::BadClientData));

        // Ключ клиента объектом JWK тоже понимается.
        let point = player.public_key().as_ref();
        let jwk = serde_json::json!({ "kty": "EC", "crv": "P-384", "x": b64(&point[1..49]), "y": b64(&point[49..]) });
        assert!(verify_client_data(&client_data(&player), &jwk).is_ok());
        assert_eq!(verify_client_data(&client_data(&stranger), &jwk).err(), Some(Failure::BadClientData));

        // Жетон без cpk — не подтверждён.
        let mut keyless = claims("x");
        keyless.as_object_mut().expect("объект").remove("cpk");
        let token = sign_rs256(&service.key, "ПРОБНЫЙ", &keyless);
        assert_eq!(verify_login(&token, &client_data(&player), &service.keys, NOW).err(), Some(Failure::NoClientKey));
    }

    /// Ключи обновляются раз в час, за незнакомым kid — сразу, но не чаще
    /// раза в минуту.
    #[test]
    fn keys_are_refreshed_sparingly() {
        let now = Instant::now() + Duration::from_secs(3 * 3600);
        let ago = |seconds| Some(now - Duration::from_secs(seconds));

        assert!(should_fetch(None, None, false, now), "ключей ещё нет");
        assert!(!should_fetch(ago(600), ago(600), true, now), "свежие ключи");
        assert!(should_fetch(ago(3600), ago(3600), true, now), "час прошёл");
        assert!(should_fetch(ago(600), ago(600), false, now), "незнакомый kid");
        assert!(!should_fetch(ago(600), ago(30), false, now), "незнакомый kid, но только что ходили");
        assert!(!should_fetch(None, ago(30), false, now), "сеть только что подвела");
        assert!(should_fetch(None, ago(61), false, now), "минута прошла");
    }

    #[test]
    fn no_token_is_not_verified() {
        let runtime = tokio::runtime::Builder::new_current_thread().build().expect("исполнитель");
        assert_eq!(runtime.block_on(check(None, "")).err(), Some(Failure::NoToken));
        assert_eq!(runtime.block_on(check(Some(""), "")).err(), Some(Failure::NoToken));
        assert!(matches!(runtime.block_on(check(Some("мусор"), "")), Err(Failure::Malformed(_))));
    }
}
