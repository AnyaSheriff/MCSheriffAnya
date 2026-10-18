// Маленькая БД с атомарной заменой: rusqlite отсутствует в offline-кэше.
// Пароли и хеши не попадают ни в Debug, ни в сообщения ошибок.
use std::{collections::BTreeMap, fs, io, io::Write, path::{Path, PathBuf}};
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize, Deserialize)]
pub struct Account {
    pub name: String,
    pub display_name: String,
    pub password_hash: [u8; 32],
    pub salt: [u8; 16],
    #[serde(default, skip_serializing_if="Option::is_none")]
    pub protected: Option<String>,
    pub created: u64,
    pub last_ip: String,
    pub last_login: u64,
    pub uuid: [u8; 16],
    pub premium: bool,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Database {
    pub version: u32,
    pub accounts: BTreeMap<String, Account>,
    // Отдельный журнал: unregister не сбрасывает дневную квоту.
    pub registrations: Vec<(String, u64)>,
}

impl Default for Database {
    fn default() -> Self { Self { version: 1, accounts: BTreeMap::new(), registrations: Vec::new() } }
}

pub trait Store: Send {
    fn load(&self) -> io::Result<Database>;
    fn save(&self, database: &Database) -> io::Result<()>;
}

pub struct FileStore { path: PathBuf }
impl FileStore {
    pub fn new(path: &Path) -> Self { Self { path: path.into() } }
}
impl Store for FileStore {
    fn load(&self) -> io::Result<Database> {
        let bytes = match fs::read(&self.path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Database::default()),
            Err(error) => return Err(error),
        };
        let db: Database = serde_json::from_slice(&bytes)
            .map_err(|_| io::Error::other("auth.db: неверный формат"))?;
        if !matches!(db.version, 1 | 2) || db.accounts.iter().any(|(name, a)| {
            (a.protected.is_some() && db.version != 2)
                || a.protected.as_ref().is_some_and(|v| v.is_empty() || v.len()>1024 || a.password_hash != [0;32] || a.salt != [0;16])
                || name != &a.name || name != &name.to_ascii_lowercase()
                || !crate::network::login::valid_name(name)
                || !a.display_name.eq_ignore_ascii_case(name)
        }) { return Err(io::Error::other("auth.db: неверные записи или версия")); }
        Ok(db)
    }
    fn save(&self, db: &Database) -> io::Result<()> {
        let bytes = serde_json::to_vec(db).map_err(|_| io::Error::other("auth.db: ошибка записи"))?;
        let temporary = self.path.with_extension("db.tmp");
        let mut options = fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)] {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(&bytes)?;
        file.sync_all()?;
        fs::rename(&temporary, &self.path)?;
        Ok(())
    }
}
