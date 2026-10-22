// Хеширование паролей: Argon2id + HMAC-SHA256 с секретом установки («pepper»).
// Это бывший закрытый модуль MCSA Guard, теперь встроенный в ядро. Формат записи
// `g1.<128 hex>` байт-в-байт тот же, поэтому старые записи auth.db v2 проверяются.
// Запись (64 байта): соль 16 | идентификатор секрета 16 | метка 32.
// RFC 9106, второй профиль: m=64 МиБ, t=3, p=4; секрет добавляется после хеширования
// (OWASP, «peppering»).
use std::{fs, io::{self, Read, Write}, path::Path};
use argon2::{Algorithm, Argon2, Params, Version};
use aws_lc_rs::{constant_time, digest, hmac, rand};
use zeroize::Zeroizing;

const PREFIX: &str = "g1.";
const CONTEXT: &[u8] = b"MCSA-Guard/password/v1\0";

fn error(message: &'static str) -> io::Error { io::Error::other(message) }
fn hex(bytes: &[u8]) -> String { bytes.iter().map(|b| format!("{b:02x}")).collect() }
fn decode(record: &str) -> Option<[u8; 64]> {
    if record.len() != 131 || !record.starts_with(PREFIX) { return None; }
    let mut out = [0; 64];
    for (i, pair) in record.as_bytes()[3..].as_chunks::<2>().0.iter().enumerate() {
        let digit = |v: u8| match v { b'0'..=b'9' => Some(v - b'0'), b'a'..=b'f' => Some(v - b'a' + 10), _ => None };
        out[i] = digit(pair[0])? * 16 + digit(pair[1])?;
    }
    Some(out)
}

/// Секрет установки: 32 случайных байта в `mcsa-guard/pepper` (права только владельцу).
pub struct Pepper { key: Zeroizing<Vec<u8>>, id: [u8; 16] }
impl Pepper {
    /// Открывает секрет. Если файла нет, создаёт его только при `create`; иначе
    /// (в базе уже есть записи с секретом) — ошибка, а не тихая замена секрета.
    pub fn open(directory: &Path, create: bool) -> io::Result<Self> {
        if !directory.exists() { fs::create_dir_all(directory)?; }
        if !fs::symlink_metadata(directory)?.is_dir() { return Err(error("Auth: каталог mcsa-guard не должен быть ссылкой")); }
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(directory, fs::Permissions::from_mode(0o700))?;
        }
        let path = directory.join("pepper");
        if create && !path.exists() {
            let mut bytes = Zeroizing::new(vec![0; 32]);
            rand::fill(&mut bytes).map_err(|_| error("Auth: не удалось получить случайные байты"))?;
            let mut options = fs::OpenOptions::new();
            options.create_new(true).write(true);
            #[cfg(unix)] {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&path)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
        }
        let metadata = fs::symlink_metadata(&path)
            .map_err(|_| error("Auth: нет файла mcsa-guard/pepper (секрет установки потерян)"))?;
        if !metadata.is_file() { return Err(error("Auth: mcsa-guard/pepper должен быть обычным файлом")); }
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            if metadata.permissions().mode() & 0o077 != 0 {
                return Err(error("Auth: у mcsa-guard/pepper должны быть права только владельцу (chmod 600)"));
            }
        }
        let mut key = Zeroizing::new(Vec::new());
        fs::File::open(&path)?.take(33).read_to_end(&mut key)?;
        if key.len() != 32 { return Err(error("Auth: mcsa-guard/pepper повреждён (нужно 32 байта)")); }
        let mut id = [0; 16];
        id.copy_from_slice(&digest::digest(&digest::SHA256, &key).as_ref()[..16]);
        Ok(Self { key, id })
    }

    fn tag(&self, password: &str, name: &str, salt: &[u8]) -> io::Result<[u8; 32]> {
        let mut derived = Zeroizing::new([0u8; 32]);
        let params = Params::new(65536, 3, 4, Some(32)).map_err(|_| error("Auth: параметры Argon2"))?;
        Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
            .hash_password_into(password.as_bytes(), salt, &mut *derived)
            .map_err(|_| error("Auth: ошибка Argon2"))?;
        let mut data = Vec::with_capacity(CONTEXT.len() + 4 + name.len() + salt.len() + 32);
        data.extend_from_slice(CONTEXT);
        data.extend_from_slice(&(name.len() as u32).to_be_bytes());
        data.extend_from_slice(name.as_bytes());
        data.extend_from_slice(salt);
        data.extend_from_slice(&*derived);
        let mac = hmac::sign(&hmac::Key::new(hmac::HMAC_SHA256, &self.key), &data);
        let mut out = [0; 32];
        out.copy_from_slice(mac.as_ref());
        Ok(out)
    }

    /// Новая запись для пароля. Ник привязан к записи (в нижнем регистре, как в auth.db).
    pub fn hash(&self, name: &str, password: &str) -> io::Result<String> {
        let mut record = [0; 64];
        rand::fill(&mut record[..16]).map_err(|_| error("Auth: не удалось получить соль"))?;
        record[16..32].copy_from_slice(&self.id);
        let tag = self.tag(password, name, &record[..16])?;
        record[32..].copy_from_slice(&tag);
        Ok(format!("{PREFIX}{}", hex(&record)))
    }

    /// Ok(true) — пароль подходит; Err — запись чужая/повреждена (другой секрет).
    pub fn verify(&self, name: &str, password: &str, record: &str) -> io::Result<bool> {
        let record = decode(record).ok_or_else(|| error("Auth: неверная запись пароля"))?;
        if record[16..32] != self.id { return Err(error("Auth: запись создана с другим секретом (mcsa-guard/pepper)")); }
        let tag = self.tag(password, name, &record[..16])?;
        Ok(constant_time::verify_slices_are_equal(&tag, &record[32..]).is_ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hmac_ref::Mac;

    fn dir(name: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!("auth-hash-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&p); p
    }

    // Запись ровно так, как её собирал закрытый модуль: независимая реализация
    // на hmac/sha2 из RustCrypto, чтобы поймать любой дрейф формата.
    fn reference(key: &[u8], name: &str, password: &str, salt: [u8; 16]) -> String {
        use sha2::Digest;
        let id = sha2::Sha256::digest(key);
        let mut derived = [0u8; 32];
        Argon2::new(Algorithm::Argon2id, Version::V0x13, Params::new(65536, 3, 4, Some(32)).unwrap())
            .hash_password_into(password.as_bytes(), &salt, &mut derived).unwrap();
        let mut mac = hmac_ref::Hmac::<sha2::Sha256>::new_from_slice(key).unwrap();
        mac.update(b"MCSA-Guard/password/v1\0");
        mac.update(&(name.len() as u32).to_be_bytes());
        mac.update(name.as_bytes());
        mac.update(&salt);
        mac.update(&derived);
        let mut record = [0u8; 64];
        record[..16].copy_from_slice(&salt);
        record[16..32].copy_from_slice(&id[..16]);
        record[32..].copy_from_slice(&mac.finalize().into_bytes());
        format!("g1.{}", hex(&record))
    }

    #[test]
    fn compatible_with_module_records() {
        let d = dir("compat");
        let pepper = Pepper::open(&d, true).unwrap();
        let key = fs::read(d.join("pepper")).unwrap();
        // Запись модуля проверяется ядром.
        let old = reference(&key, "alice", "correct", [7; 16]);
        assert!(pepper.verify("alice", "correct", &old).unwrap());
        assert!(!pepper.verify("alice", "wrong", &old).unwrap());
        assert!(!pepper.verify("bob", "correct", &old).unwrap());
        // И наоборот: запись ядра совпадает с эталоном при той же соли.
        let mine = pepper.hash("alice", "correct").unwrap();
        let salt: [u8; 16] = decode(&mine).unwrap()[..16].try_into().unwrap();
        assert_eq!(mine, reference(&key, "alice", "correct", salt));
        assert_ne!(mine, pepper.hash("alice", "correct").unwrap());
        fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn known_answer_with_fixed_pepper() {
        // Фиксированный секрет 0..32 и соль: защита от случайной смены параметров.
        let d = dir("kat");
        fs::create_dir_all(&d).unwrap();
        let key: Vec<u8> = (0..32).collect();
        let mut options = fs::OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; options.mode(0o600); }
        options.open(d.join("pepper")).unwrap().write_all(&key).unwrap();
        let pepper = Pepper::open(&d, false).unwrap();
        let record = reference(&key, "alice", "пароль😀", [1; 16]);
        assert!(pepper.verify("alice", "пароль😀", &record).unwrap());
        assert_eq!(record, KNOWN);
        fs::remove_dir_all(d).unwrap();
    }
    const KNOWN: &str = "g1.01010101010101010101010101010101630dcd2966c4336691125448bbb25b4f9c275591e8da7e3ee9a960e599b7bfe4c6a2438e7ae7c415d476e0f223621878";

    #[test]
    fn rejects_foreign_bad_records_and_missing_or_open_pepper() {
        let d = dir("bad");
        let pepper = Pepper::open(&d, true).unwrap();
        let record = pepper.hash("alice", "correct").unwrap();
        assert!(pepper.verify("alice", "correct", "g1.bad").is_err());
        let mut other = record.clone();
        other.replace_range(35..37, if &record[35..37] == "00" { "01" } else { "00" }); // чужой id секрета
        assert!(pepper.verify("alice", "correct", &other).is_err());
        let mut flipped = record.clone();
        flipped.replace_range(129..131, if &record[129..131] == "00" { "01" } else { "00" });
        assert!(!pepper.verify("alice", "correct", &flipped).unwrap());
        // Потеря секрета не создаёт новый, если есть записи.
        fs::remove_file(d.join("pepper")).unwrap();
        assert!(Pepper::open(&d, false).is_err());
        let fresh = Pepper::open(&d, true).unwrap();
        assert!(fresh.verify("alice", "correct", &record).is_err());
        #[cfg(unix)] {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(d.join("pepper"), fs::Permissions::from_mode(0o644)).unwrap();
            assert!(Pepper::open(&d, false).is_err());
        }
        fs::remove_dir_all(d).unwrap();
    }
}
