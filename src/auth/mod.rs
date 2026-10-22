// Парольная авторизация Java до входа в мир. Доверяем UUID учётной записи,
// а не Login Start и не одному лишь совпадению ника с ops.json.
pub mod dialog;
pub mod premium;
mod store;
mod hash;
#[cfg(test)] mod tests;

use std::{collections::HashMap, io, net::IpAddr, num::NonZeroU32, path::Path, sync::{Arc, Mutex}, time::{SystemTime, UNIX_EPOCH}};
use aws_lc_rs::pbkdf2;
use store::{Account, Database, FileStore, Store};

pub const OP_PROTECTED: &str = "Аккаунт оператора: пароль задаётся из консоли";
pub const BLOCKED: &str = "Слишком много ошибок входа. Попробуй через 10 минут.";
const ITERATIONS: NonZeroU32 = NonZeroU32::new(100_000).unwrap();

pub fn now() -> u64 { SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs() }
fn valid_password(password: &str) -> bool { (4..=64).contains(&password.chars().count()) }
// Старый (до Argon2id) способ: только для проверки записей v1 и тестов миграции.
#[cfg(test)]
fn legacy_hash(password: &str) -> io::Result<([u8; 16], [u8; 32])> {
    let mut salt = [0; 16];
    aws_lc_rs::rand::fill(&mut salt).map_err(|_| io::Error::other("не удалось получить случайную соль"))?;
    let mut result = [0; 32];
    pbkdf2::derive(pbkdf2::PBKDF2_HMAC_SHA256, ITERATIONS, &salt, password.as_bytes(), &mut result);
    Ok((salt, result))
}
fn legacy_verify(account: &Account, password: &str) -> bool {
    pbkdf2::verify(pbkdf2::PBKDF2_HMAC_SHA256, ITERATIONS, &account.salt,
        password.as_bytes(), &account.password_hash).is_ok()
}

#[derive(Default)]
struct Limits { failures: HashMap<IpAddr, Vec<u64>> }
impl Limits {
    fn blocked(&mut self, ip: IpAddr, now: u64) -> bool {
        self.failures.retain(|_, times| { times.retain(|t| now.saturating_sub(*t) < 600); !times.is_empty() });
        self.failures.get(&ip).is_some_and(|v| v.len() >= 5)
    }
    fn fail(&mut self, ip: IpAddr, now: u64) { self.blocked(ip, now); self.failures.entry(ip).or_default().push(now); }
}
struct State {
    store: Box<dyn Store>,
    db: Database,
    limits: Limits,
    sessions: HashMap<(String, [u8; 16]), usize>,
}
pub struct Service {
    state: Mutex<State>,
    operation: Mutex<()>,
    pepper: hash::Pepper,
}

#[derive(Clone)]
pub struct Identity {
    pub name: String,
    pub uuid: [u8; 16],
    pub verified: bool,
    pub registered: bool,
}
impl Service {
    /// Секрет лежит рядом с базой: `mcsa-guard/pepper`. Бэкап — auth.db и mcsa-guard/ вместе.
    pub fn open(path: &Path) -> io::Result<Self> { Self::open_in(path, &path.with_file_name("mcsa-guard")) }
    #[cfg(test)]
    fn open_test(path: &Path) -> io::Result<Self> { Self::open_in(path, &path.with_extension("guard")) }
    fn open_in(path: &Path, secret: &Path) -> io::Result<Self> {
        let store = Box::new(FileStore::new(path));
        let db = store.load()?;
        // Пока в базе есть записи с секретом, молча создавать новый секрет нельзя.
        let pepper = hash::Pepper::open(secret, !db.accounts.values().any(|a| a.protected.is_some()))?;
        // Создание файла проверяет доступность записи до первого подключения.
        store.save(&db)?;
        Ok(Self { state: Mutex::new(State { store, db, limits: Limits::default(), sessions: HashMap::new() }), operation:Mutex::new(()), pepper })
    }
    pub fn identity(&self, name: &str, verified_uuid: Option<[u8; 16]>) -> Identity {
        let state = self.state.lock().expect("auth");
        let account = state.db.accounts.get(&name.to_ascii_lowercase());
        Identity {
            name: account.map_or_else(|| name.into(), |a| a.display_name.clone()),
            uuid: account.map_or_else(|| verified_uuid.unwrap_or_else(|| crate::network::login::offline_uuid(name)), |a| a.uuid),
            verified: verified_uuid.is_some(), registered: account.is_some(),
        }
    }
    pub fn blocked(&self, ip: IpAddr) -> bool { self.state.lock().expect("auth").limits.blocked(ip, now()) }
    pub fn registration_allowed(ops: &crate::ops::Ops, name: &str, verified: Option<[u8; 16]>) -> bool {
        ops.uuid_for_name(name).is_none_or(|uuid| Some(uuid) == verified)
    }
    #[cfg(test)]
    fn submit(&self, identity: &Identity, ip: IpAddr, password: &str, repeat: Option<&str>, permitted: bool) -> Result<(), &'static str> {
        self.submit_before(identity, ip, password, repeat, permitted, None)
    }
    fn submit_before(&self, identity: &Identity, ip: IpAddr, password: &str, repeat: Option<&str>, permitted: bool, deadline: Option<tokio::time::Instant>) -> Result<(), &'static str> {
        // Ограничение памяти/процессов: только одна проверка, без неограниченной очереди.
        let _operation=self.operation.try_lock().map_err(|_| "Авторизация занята. Попробуй ещё раз.")?;
        let mut state = self.state.lock().expect("auth");
        if deadline.is_some_and(|d| tokio::time::Instant::now() >= d) { return Err(dialog::TIMEOUT); }
        let time = now();
        if state.limits.blocked(ip, time) { return Err(BLOCKED); }
        let key = identity.name.to_ascii_lowercase();
        let existing = state.db.accounts.get(&key);
        // Login Success уже объявил UUID. Изменение БД в другом подключении
        // или консоли требует нового входа, но никогда не смены UUID молча.
        if existing.is_some() != identity.registered || existing.is_some_and(|a| a.uuid != identity.uuid) {
            return Err("Аккаунт изменился. Зайди ещё раз.");
        }
        if existing.is_none() {
            if !permitted { return Err(OP_PROTECTED); }
            if !valid_password(password) { return Err("Пароль должен содержать 4–64 символа"); }
            if repeat != Some(password) { return Err("Пароли не совпадают"); }
            if state.db.registrations.iter().filter(|(address, t)| address == &ip.to_string() && time.saturating_sub(*t) < 86400).count() >= 3 {
                return Err("С этого IP уже зарегистрировано 3 аккаунта за сутки");
            }
        }
        let mut db = state.db.clone();
        drop(state); // Дорогая проверка не держит общий замок сессий/identity.
        if let Some(account)=db.accounts.get(&key)
            && (!valid_password(password) || !self.check_password(account,password)?) {
            self.state.lock().expect("auth").limits.fail(ip,time);
            return Err("Неверный пароль");
        }
        if !identity.registered {
            let protected=Some(self.new_password(&key,password)?);
            db.accounts.insert(key.clone(), Account {
                name: key.clone(), display_name: identity.name.clone(), password_hash: [0;32], salt: [0;16], protected,
                created: time, last_ip: ip.to_string(), last_login: time, uuid: identity.uuid, premium: identity.verified,
            });
            db.registrations.retain(|(_, t)| time.saturating_sub(*t) < 86400);
            db.registrations.push((ip.to_string(), time));
        }
        let account = db.accounts.get_mut(&key).expect("аккаунт");
        // Старый хеш (v1) после успешного входа заменяется на Argon2id.
        if account.protected.is_none() {
            account.protected=Some(self.new_password(&key,password)?);
            account.salt=[0;16]; account.password_hash=[0;32];
        }
        account.last_ip = ip.to_string(); account.last_login = time;
        if identity.verified && !account.premium {
            crate::log_info!("Auth: {} подтвердил лицензию; сохранён UUID cracked-аккаунта {}", identity.name, crate::network::login::format_uuid(&account.uuid));
        }
        if deadline.is_some_and(|d| tokio::time::Instant::now() >= d) { return Err(dialog::TIMEOUT); }
        db.version=2;
        let mut state=self.state.lock().expect("auth");
        state.store.save(&db).map_err(|_| "Не удалось сохранить аккаунт")?;
        state.db = db;
        Ok(())
    }
    fn check_password(&self,account:&Account,password:&str)->Result<bool,&'static str> {
        match &account.protected {
            Some(record)=>self.pepper.verify(&account.name,password,record)
                .map_err(|_|"Запись пароля повреждена или не совпадает с секретом mcsa-guard/pepper"),
            None=>Ok(legacy_verify(account,password)),
        }
    }
    fn new_password(&self,name:&str,password:&str)->Result<String,&'static str> {
        self.pepper.hash(name,password).map_err(|_|"Не удалось сохранить пароль")
    }
    pub fn session(self: &Arc<Self>, identity: &Identity) -> Session {
        let key = (identity.name.to_ascii_lowercase(), identity.uuid);
        *self.state.lock().expect("auth").sessions.entry(key.clone()).or_default() += 1;
        Session { service: self.clone(), key }
    }
}
pub struct Session { service: Arc<Service>, key: (String, [u8; 16]) }
impl Drop for Session {
    fn drop(&mut self) {
        let mut state = self.service.state.lock().expect("auth");
        if let Some(count) = state.sessions.get_mut(&self.key) {
            *count -= 1;
            if *count == 0 { state.sessions.remove(&self.key); }
        }
    }
}

pub fn sensitive_command(command: &str) -> bool {
    matches!(command.trim_start().trim_start_matches('/').split_whitespace().next(), Some("auth" | "changepassword"))
}

pub fn command(command: &str, source: &crate::commands::Source, shared: &crate::shared::Shared) -> Option<crate::commands::Answer> {
    if !sensitive_command(command) { return None; }
    let result = command_inner(command, source, shared).unwrap_or_else(|s| s.into());
    Some(crate::commands::Answer::saying(result))
}
// Кавычки позволяют сменить пароль с пробелами; ошибки никогда не цитируют
// исходную строку. Внутри кавычек — обычные JSON escape-последовательности.
fn arguments(mut line: &str) -> Result<Vec<String>, &'static str> {
    let mut words = Vec::new();
    loop {
        line = line.trim_start();
        if line.is_empty() { return Ok(words); }
        if line.starts_with('"') {
            let mut values = serde_json::Deserializer::from_str(line).into_iter::<String>();
            let value = values.next().ok_or("Неверные кавычки")?.map_err(|_| "Неверные кавычки")?;
            let offset = values.byte_offset();
            line = &line[offset..];
            if !line.is_empty() && !line.starts_with(char::is_whitespace) { return Err("Неверные кавычки"); }
            words.push(value);
        } else {
            let end = line.find(char::is_whitespace).unwrap_or(line.len());
            words.push(line[..end].into()); line = &line[end..];
        }
        if words.len() > 4 { return Err("Слишком много аргументов"); }
    }
}

fn command_inner(command: &str, source: &crate::commands::Source, shared: &crate::shared::Shared) -> Result<String, &'static str> {
    use crate::commands::Source;
    let service = shared.auth.as_ref().ok_or("Парольная авторизация выключена")?;
    let parsed = arguments(command)?;
    let words: Vec<_> = parsed.iter().map(String::as_str).collect();
    let _operation=service.operation.try_lock().map_err(|_|"Авторизация занята. Попробуй ещё раз.")?;
    let state = service.state.lock().expect("auth");
    let mut db = state.db.clone();
    let sessions=state.sessions.clone();
    drop(state);
    match words.as_slice() {
        ["auth", ..] if !matches!(source, Source::Console) => return Err("Команда auth доступна только из консоли"),
        ["auth", "list"] => return Ok(format!("Аккаунтов: {}. {}", db.accounts.len(), db.accounts.values().map(|a| a.display_name.as_str()).collect::<Vec<_>>().join(", "))),
        ["auth", "unregister", name] => {
            if db.accounts.remove(&name.to_ascii_lowercase()).is_none() { return Err("Аккаунт не найден"); }
        }
        ["auth", "setpassword", name, password] => {
            if !crate::network::login::valid_name(name) { return Err("Недопустимый ник"); }
            if !valid_password(password) { return Err("Пароль должен содержать 4–64 символа"); }
            let key = name.to_ascii_lowercase();
            let protected=Some(service.new_password(&key,password)?);
            let op_uuid = shared.ops.lock().expect("ops").uuid_for_name(name);
            let account = db.accounts.entry(key.clone()).or_insert_with(|| Account {
                name: key, display_name: (*name).into(), password_hash: [0;32], salt: [0;16], protected:None, created: now(), last_ip: String::new(), last_login: 0,
                uuid: op_uuid.unwrap_or_else(|| crate::network::login::offline_uuid(name)), premium: op_uuid.is_some(),
            });
            account.salt = [0;16]; account.password_hash = [0;32]; account.protected=protected;
        }
        ["changepassword", old, new] => {
            let Source::Player { uuid, name } = source else { return Err("Команда доступна вошедшему игроку"); };
            if !sessions.contains_key(&(name.to_ascii_lowercase(), *uuid)) { return Err("Сначала войди с паролем"); }
            if !valid_password(new) { return Err("Пароль должен содержать 4–64 символа"); }
            let account = db.accounts.get_mut(&name.to_ascii_lowercase()).ok_or("Аккаунт не найден")?;
            if account.uuid != *uuid || !valid_password(old) || !service.check_password(account, old)? { return Err("Неверный пароль"); }
            account.protected = Some(service.new_password(&account.name,new)?);
            account.salt = [0;16]; account.password_hash = [0;32];
        }
        _ => return Err("Использование: auth setpassword <name> <password> | auth unregister <name> | auth list; /changepassword <old> <new>"),
    }
    db.version=2;
    let mut state=service.state.lock().expect("auth");
    state.store.save(&db).map_err(|_| "Не удалось сохранить auth.db")?;
    state.db = db;
    Ok("Готово".into())
}
