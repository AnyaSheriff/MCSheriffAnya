// Приёмник отчётов об ошибках MCSheriffAnya.
//
// Серверы, чьи владельцы включили отправку отчётов, шлют сюда
// `POST /v1/report` с JSON (формат — в README.md рядом). Приёмник ничего не
// знает об отправителе: IP не пишется ни на диск, ни в журнал. Он живёт
// только в памяти — чтобы один адрес не завалил приёмник отчётами — и
// забывается через час.
//
// Отчёты складываются по «подписи»: виду ошибки и первой строке её текста
// без чисел. Одинаковые падения лежат в одной папке и считаются вместе;
// от каждого вида хранятся последние KEEP отчётов.
//
// Чтение — `GET /v1/summary` и `GET /v1/report/<подпись>` с ключом
// в заголовке `Authorization: Bearer <ключ>`; ключ — в файле, путь к нему
// в MCSA_TOKEN_FILE. Без файла ключа чтение закрыто совсем.
//
// Настройки — переменными окружения:
// MCSA_LISTEN     — где слушать (по умолчанию 127.0.0.1:8787, за Caddy);
// MCSA_DATA       — папка отчётов (по умолчанию /var/lib/mcsa-logreceiver);
// MCSA_TOKEN_FILE — файл с ключом для чтения.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::{Read, Write};
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};
use tiny_http::{Header, Method, Request, Response, Server};

/// Самый большой принимаемый отчёт.
const MAX_BODY: usize = 256 * 1024;

/// Пределы полей отчёта: длиннее — обрезается.
const MAX_LINES: usize = 400;
const MAX_LINE: usize = 1000;
const MAX_MESSAGE: usize = 2000;
const MAX_SHORT: usize = 64;

/// Виды отчётов, которые принимаются.
const KINDS: [&str; 3] = ["panic", "error", "bedrock-crash"];

/// Сколько отчётов одного вида хранить: старые удаляются.
const KEEP: usize = 20;

/// Сколько разных серверов помнить у одного вида ошибки.
const MAX_INSTALLS: usize = 1000;

/// Пределы хранилища: больше — новые виды и отчёты не принимаются.
const MAX_SIGNATURES: usize = 10_000;
const MAX_BYTES: u64 = 1 << 30;

/// Сколько отчётов в час принимается с одного адреса и со всех вместе.
/// Настоящему серверу больше и не нужно: он сам шлёт одну и ту же ошибку не
/// чаще раза в час и не больше 30 отчётов в сутки. Всё сверх — шум.
const PER_ADDRESS_HOURLY: u32 = 20;
const PER_INSTALL_HOURLY: u32 = 20;
const TOTAL_HOURLY: u32 = 500;

/// Сколько запросов разбирается одновременно.
const WORKERS: usize = 4;

const HOUR: u64 = 3600;

fn main() {
    let listen = std::env::var("MCSA_LISTEN").unwrap_or_else(|_| "127.0.0.1:8787".to_string());
    let data = PathBuf::from(std::env::var("MCSA_DATA").unwrap_or_else(|_| "/var/lib/mcsa-logreceiver".to_string()));
    let token = std::env::var("MCSA_TOKEN_FILE")
        .ok()
        .and_then(|path| fs::read_to_string(path).ok())
        .map(|token| token.trim().to_string())
        .filter(|token| token.len() >= 16);

    if token.is_none() {
        eprintln!("Ключа для чтения нет (MCSA_TOKEN_FILE, не короче 16 знаков) — чтение отчётов закрыто");
    }

    let store = match Store::open(&data) {
        Ok(store) => store,
        Err(error) => {
            eprintln!("Не открылась папка отчётов {}: {}", data.display(), error);
            std::process::exit(1);
        }
    };
    eprintln!("Отчётов: {} видов, {} байт; слушаю {}", store.signatures.len(), store.bytes, listen);

    let server = match Server::http(&listen) {
        Ok(server) => Arc::new(server),
        Err(error) => {
            eprintln!("Не слушается {}: {}", listen, error);
            std::process::exit(1);
        }
    };
    let state = Arc::new(State {
        store: Mutex::new(store),
        limits: Mutex::new(Limits::default()),
        token,
    });

    let workers: Vec<_> = (0..WORKERS)
        .map(|_| {
            let (server, state) = (Arc::clone(&server), Arc::clone(&state));

            std::thread::spawn(move || {
                while let Ok(request) = server.recv() {
                    handle(&state, request);
                }
            })
        })
        .collect();

    for worker in workers {
        let _ = worker.join();
    }
}

/// Общее для всех потоков.
struct State {
    store: Mutex<Store>,
    limits: Mutex<Limits>,
    token: Option<String>,
}

/// Разбирает один запрос. Ответы — без тела, кроме чтения.
fn handle(state: &State, mut request: Request) {
    let url = request.url().split('?').next().unwrap_or("").to_string();

    let (status, body, kind) = match (request.method(), url.as_str()) {
        (Method::Get, "/health") => (200, "ok\n".to_string(), "text/plain; charset=utf-8"),
        (Method::Post, "/v1/report") => (receive(state, &mut request), String::new(), "text/plain"),
        (Method::Get, "/v1/summary") => read_summary(state, &request),
        (Method::Get, path) if path.starts_with("/v1/report/") => read_reports(state, &request, &path["/v1/report/".len()..]),
        _ => (404, String::new(), "text/plain"),
    };

    let header = Header::from_bytes(&b"Content-Type"[..], kind.as_bytes()).expect("заголовок из ASCII");
    let _ = request.respond(Response::from_string(body).with_status_code(status).with_header(header));
}

/// Принимает отчёт; ответ — код HTTP.
fn receive(state: &State, request: &mut Request) -> u16 {
    let now = unix_now();

    if !state.limits.lock().expect("пределы захвачены").allow(sender(request), now) {
        return 429;
    }

    let mut body = Vec::new();
    if request.as_reader().take(MAX_BODY as u64 + 1).read_to_end(&mut body).is_err() {
        return 400;
    }
    if body.len() > MAX_BODY {
        return 413;
    }

    let Some(report) = Report::parse(&body) else {
        return 400;
    };

    if !state.limits.lock().expect("пределы захвачены").allow_install(&report.install, now) {
        return 429;
    }

    match state.store.lock().expect("хранилище захвачено").add(&report, now) {
        Ok(true) => 204,
        Ok(false) => 507,
        Err(error) => {
            eprintln!("Отчёт не записался: {}", error);
            500
        }
    }
}

/// Адрес отправителя — только для пределов частоты, в памяти. За Caddy
/// или nginx настоящий адрес в X-Forwarded-For (прокси ставит его сам и не
/// верит присланному клиентом); напрямую — адрес соединения. Если и там
/// местный адрес (nginx за stream без proxy_protocol видит только себя),
/// отправитель неизвестен: тогда работает лишь общий предел.
fn sender(request: &Request) -> Option<IpAddr> {
    let remote = request.remote_addr()?.ip();

    if !remote.is_loopback() {
        return Some(remote);
    }

    request
        .headers()
        .iter()
        .find(|header| header.field.equiv("X-Forwarded-For"))
        .and_then(|header| header.value.as_str().split(',').next())
        .and_then(|first| first.trim().parse::<IpAddr>().ok())
        .filter(|address| !address.is_loopback())
}

/// Есть ли у запроса верный ключ для чтения.
fn authorized(state: &State, request: &Request) -> bool {
    let Some(token) = &state.token else {
        return false;
    };

    request
        .headers()
        .iter()
        .find(|header| header.field.equiv("Authorization"))
        .and_then(|header| header.value.as_str().strip_prefix("Bearer "))
        .is_some_and(|given| same(given.trim().as_bytes(), token.as_bytes()))
}

/// Сравнение ключей за одинаковое время, чем бы они ни отличались.
fn same(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

/// Сводка: по строке на вид ошибки, свежие сверху.
fn read_summary(state: &State, request: &Request) -> (u16, String, &'static str) {
    if !authorized(state, request) {
        return (401, String::new(), "text/plain");
    }

    let store = state.store.lock().expect("хранилище захвачено");
    let mut rows: Vec<(&String, &Meta)> = store.signatures.iter().collect();
    rows.sort_by_key(|row| std::cmp::Reverse(row.1.last));

    let mut out = String::from("подпись\tраз\tсерверов\tпоследний\tвид\tверсии\tсообщение\n");
    for (signature, meta) in rows {
        let versions: Vec<String> = meta.versions.iter().map(|(version, count)| format!("{}×{}", version, count)).collect();
        out.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            signature,
            meta.count,
            meta.installs.len(),
            date(meta.last),
            meta.kind,
            versions.join(","),
            meta.message.replace(['\t', '\n'], " ")
        ));
    }

    (200, out, "text/tab-separated-values; charset=utf-8")
}

/// Отчёты одного вида: сведения о нём и сохранённые отчёты, свежие первыми.
fn read_reports(state: &State, request: &Request, signature: &str) -> (u16, String, &'static str) {
    if !authorized(state, request) {
        return (401, String::new(), "text/plain");
    }

    let store = state.store.lock().expect("хранилище захвачено");
    let Some(meta) = store.signatures.get(signature) else {
        return (404, String::new(), "text/plain");
    };

    let reports: Vec<Value> = store
        .files(signature)
        .iter()
        .rev()
        .filter_map(|path| fs::read(path).ok())
        .filter_map(|bytes| serde_json::from_slice(&bytes).ok())
        .collect();

    let out = json!({ "signature": signature, "meta": meta.to_json(), "reports": reports });
    (200, format!("{:#}\n", out), "application/json; charset=utf-8")
}

// --- Отчёт -----------------------------------------------------------------

/// Отчёт, как его прислал сервер, — уже проверенный и обрезанный.
struct Report {
    install: String,
    version: String,
    os: String,
    kind: String,
    message: String,
    lines: Vec<String>,
}

impl Report {
    /// Разбирает JSON отчёта. Неизвестный вид или формат — отказ; длинные
    /// поля обрезаются; адреса и домашние папки ещё раз вычищаются — на
    /// случай, если сервер что-то пропустил.
    fn parse(body: &[u8]) -> Option<Report> {
        let value: Value = serde_json::from_slice(body).ok()?;

        if value.get("format")?.as_u64()? != 1 {
            return None;
        }

        let text = |name: &str, limit: usize| value.get(name).and_then(Value::as_str).map(|text| clean(text, limit));
        let kind = text("kind", MAX_SHORT)?;

        if !KINDS.contains(&kind.as_str()) {
            return None;
        }

        let install = text("install", MAX_SHORT)?;
        if install.len() != 16 || !install.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }

        let lines = value
            .get("lines")
            .and_then(Value::as_array)
            .map(|lines| lines.iter().filter_map(Value::as_str).take(MAX_LINES).map(|line| clean(line, MAX_LINE)).collect())
            .unwrap_or_default();

        // Правдоподобие: версия — как у выпусков («0.0.2-alpha»), система —
        // как её называет Rust («linux x86_64»), сообщение не пустое.
        // Прочее — не от нашего сервера.
        let version = text("version", MAX_SHORT)?;
        let os = text("os", MAX_SHORT).unwrap_or_default();
        let message = text("message", MAX_MESSAGE)?;

        let version_ok = version.starts_with(|char: char| char.is_ascii_digit())
            && version.len() <= 32
            && version.chars().all(|char| char.is_ascii_alphanumeric() || matches!(char, '.' | '-' | '+'));
        let os_ok = os.len() <= 32 && os.chars().all(|char| char.is_ascii_lowercase() || char.is_ascii_digit() || matches!(char, '_' | ' '));

        if !version_ok || !os_ok || message.trim().is_empty() {
            return None;
        }

        Some(Report {
            install: install.to_ascii_lowercase(),
            version,
            os,
            kind,
            message,
            lines,
        })
    }

    /// Подпись вида ошибки: вид и первая строка сообщения без чисел —
    /// одна и та же ошибка на разных местах и версиях даёт одну подпись.
    fn signature(&self) -> String {
        let first = self.message.lines().next().unwrap_or("").trim();
        let mut shape = String::with_capacity(first.len() + self.kind.len() + 1);
        shape.push_str(&self.kind);
        shape.push(':');

        let mut in_number = false;
        for char in first.chars() {
            if char.is_ascii_digit() {
                if !in_number {
                    shape.push('#');
                }
                in_number = true;
            } else {
                shape.push(char);
                in_number = false;
            }
        }

        format!("{:016x}", fnv(shape.as_bytes()))
    }
}

/// Хеш FNV-1a: один и тот же на любой машине и версии.
fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3))
}

/// Обрезает строку до `limit` знаков, убирает управляющие знаки (кроме
/// перевода строки), прячет адреса IPv4 и имена в домашних папках.
fn clean(text: &str, limit: usize) -> String {
    let cut: String = text.chars().filter(|char| *char == '\n' || !char.is_control()).take(limit).collect();
    hide_home(&hide_ipv4(&cut))
}

/// Заменяет адреса вида 1.2.3.4 (с портом или без) на `<ip>`.
fn hide_ipv4(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut at = 0;

    while at < bytes.len() {
        let starts_word = at == 0 || !(bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'.');

        if starts_word {
            if let Some(length) = ipv4_at(&bytes[at..]) {
                out.push_str("<ip>");
                at += length;
                continue;
            }
        }

        let char = text[at..].chars().next().expect("внутри строки");
        out.push(char);
        at += char.len_utf8();
    }

    out
}

/// Длина адреса IPv4 в начале `bytes`, если он там есть.
fn ipv4_at(bytes: &[u8]) -> Option<usize> {
    let mut at = 0;

    for part in 0..4 {
        if part > 0 {
            if bytes.get(at) != Some(&b'.') {
                return None;
            }
            at += 1;
        }

        let digits = bytes[at..].iter().take_while(|byte| byte.is_ascii_digit()).count();
        if digits == 0 || digits > 3 {
            return None;
        }

        let number: u32 = std::str::from_utf8(&bytes[at..at + digits]).ok()?.parse().ok()?;
        if number > 255 {
            return None;
        }
        at += digits;
    }

    // Дальше не должно идти ещё цифр или точки с цифрой: это уже не адрес.
    match bytes.get(at) {
        Some(byte) if byte.is_ascii_alphanumeric() => None,
        Some(b'.') if bytes.get(at + 1).is_some_and(u8::is_ascii_digit) => None,
        _ => Some(at),
    }
}

/// Заменяет имя пользователя в путях `/home/имя/` и `C:\Users\имя\`.
fn hide_home(text: &str) -> String {
    let mut out = text.to_string();

    for (prefix, separator) in [("/home/", '/'), ("\\Users\\", '\\'), ("/Users/", '/')] {
        let mut from = 0;

        while let Some(found) = out[from..].find(prefix) {
            let start = from + found + prefix.len();
            let end = out[start..].find(|char: char| char == separator || char.is_whitespace()).map_or(out.len(), |end| start + end);

            if end > start {
                if &out[start..end] != "<user>" {
                    out.replace_range(start..end, "<user>");
                }
                from = start + "<user>".len();
            } else {
                from = start;
            }
        }
    }

    out
}

// --- Хранилище -------------------------------------------------------------

/// Сведения об одном виде ошибки.
struct Meta {
    kind: String,
    message: String,
    count: u64,
    first: u64,
    last: u64,
    versions: BTreeMap<String, u64>,
    installs: HashSet<String>,
}

impl Meta {
    fn to_json(&self) -> Value {
        let mut installs: Vec<&String> = self.installs.iter().collect();
        installs.sort();

        json!({
            "kind": self.kind,
            "message": self.message,
            "count": self.count,
            "first": self.first,
            "last": self.last,
            "versions": self.versions,
            "installs": installs,
        })
    }

    fn from_json(value: &Value) -> Option<Meta> {
        Some(Meta {
            kind: value.get("kind")?.as_str()?.to_string(),
            message: value.get("message")?.as_str()?.to_string(),
            count: value.get("count")?.as_u64()?,
            first: value.get("first")?.as_u64()?,
            last: value.get("last")?.as_u64()?,
            versions: value
                .get("versions")?
                .as_object()?
                .iter()
                .filter_map(|(version, count)| Some((version.clone(), count.as_u64()?)))
                .collect(),
            installs: value.get("installs")?.as_array()?.iter().filter_map(Value::as_str).map(str::to_string).collect(),
        })
    }
}

/// Папка отчётов: `<подпись>/meta.json` и `<подпись>/<время>-<случайное>.json`.
struct Store {
    root: PathBuf,
    signatures: HashMap<String, Meta>,
    /// Сколько байтов занимают отчёты — примерно, для предела.
    bytes: u64,
}

impl Store {
    fn open(root: &Path) -> std::io::Result<Store> {
        fs::create_dir_all(root)?;

        let mut store = Store {
            root: root.to_path_buf(),
            signatures: HashMap::new(),
            bytes: 0,
        };

        for entry in fs::read_dir(root)? {
            let entry = entry?;
            let name = entry.file_name().to_string_lossy().to_string();

            if !entry.file_type()?.is_dir() || name.len() != 16 {
                continue;
            }

            let meta = fs::read(entry.path().join("meta.json"))
                .ok()
                .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
                .and_then(|value| Meta::from_json(&value));

            if let Some(meta) = meta {
                for file in fs::read_dir(entry.path())? {
                    store.bytes += file?.metadata()?.len();
                }
                store.signatures.insert(name, meta);
            }
        }

        Ok(store)
    }

    /// Сохранённые отчёты вида по порядку: старые первыми.
    fn files(&self, signature: &str) -> Vec<PathBuf> {
        let mut files: Vec<PathBuf> = fs::read_dir(self.root.join(signature))
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .map(|entry| entry.path())
                    .filter(|path| path.file_name().is_some_and(|name| name != "meta.json"))
                    .filter(|path| path.extension().is_some_and(|extension| extension == "json"))
                    .collect()
            })
            .unwrap_or_default();

        files.sort();
        files
    }

    /// Добавляет отчёт. `Ok(false)` — хранилище полно.
    fn add(&mut self, report: &Report, now: u64) -> std::io::Result<bool> {
        let signature = report.signature();
        let known = self.signatures.contains_key(&signature);

        if self.bytes > MAX_BYTES || (!known && self.signatures.len() >= MAX_SIGNATURES) {
            return Ok(false);
        }

        let folder = self.root.join(&signature);
        fs::create_dir_all(&folder)?;

        let body = format!(
            "{:#}\n",
            json!({
                "received": now,
                "install": report.install,
                "version": report.version,
                "os": report.os,
                "kind": report.kind,
                "message": report.message,
                "lines": report.lines,
            })
        );
        write_atomically(&folder.join(format!("{:010}-{:016x}.json", now, random())), body.as_bytes())?;
        self.bytes += body.len() as u64;

        let meta = self.signatures.entry(signature.clone()).or_insert_with(|| Meta {
            kind: report.kind.clone(),
            message: report.message.lines().next().unwrap_or("").to_string(),
            count: 0,
            first: now,
            last: now,
            versions: BTreeMap::new(),
            installs: HashSet::new(),
        });
        meta.count += 1;
        meta.last = now;
        *meta.versions.entry(report.version.clone()).or_insert(0) += 1;
        if meta.installs.len() < MAX_INSTALLS {
            meta.installs.insert(report.install.clone());
        }
        let meta_body = format!("{:#}\n", meta.to_json());
        write_atomically(&folder.join("meta.json"), meta_body.as_bytes())?;

        // Лишние старые отчёты — прочь.
        let files = self.files(&signature);
        for old in files.iter().take(files.len().saturating_sub(KEEP)) {
            let size = fs::metadata(old).map(|meta| meta.len()).unwrap_or(0);
            if fs::remove_file(old).is_ok() {
                self.bytes = self.bytes.saturating_sub(size);
            }
        }

        Ok(true)
    }
}

/// Пишет файл целиком или никак: сначала во временный, потом переименование.
fn write_atomically(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let temporary = path.with_extension("tmp");
    let mut file = fs::File::create(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(temporary, path)
}

/// Случайное число — для имён файлов отчётов.
fn random() -> u64 {
    let mut bytes = [0u8; 8];
    if fs::File::open("/dev/urandom").and_then(|mut file| file.read_exact(&mut bytes)).is_err() {
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|time| time.subsec_nanos()).unwrap_or(0);
        return fnv(&nanos.to_le_bytes());
    }
    u64::from_le_bytes(bytes)
}

// --- Пределы частоты ---------------------------------------------------------

/// Сколько отчётов принято за текущий час — с каждого адреса и всего.
/// Только в памяти: адреса нигде не записываются.
#[derive(Default)]
struct Limits {
    hour: u64,
    total: u32,
    by_address: HashMap<IpAddr, u32>,
    by_install: HashMap<String, u32>,
}

impl Limits {
    fn allow(&mut self, address: Option<IpAddr>, now: u64) -> bool {
        // Новый час — счёт с нуля, прошлые адреса забываются.
        if now / HOUR != self.hour {
            self.hour = now / HOUR;
            self.total = 0;
            self.by_address.clear();
            self.by_install.clear();
        }

        if self.total >= TOTAL_HOURLY {
            return false;
        }

        if let Some(address) = address {
            let count = self.by_address.entry(address).or_insert(0);
            if *count >= PER_ADDRESS_HOURLY {
                return false;
            }
            *count += 1;
        }

        self.total += 1;
        true
    }

    /// Предел на номер установки — второй, после разбора отчёта: номер
    /// можно выдумать, но адресов у тролля всё равно немного.
    fn allow_install(&mut self, install: &str, now: u64) -> bool {
        if now / HOUR != self.hour {
            return true;
        }

        let count = self.by_install.entry(install.to_string()).or_insert(0);
        *count += 1;
        *count <= PER_INSTALL_HOURLY
    }
}

// --- Время -------------------------------------------------------------------

fn unix_now() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|time| time.as_secs()).unwrap_or(0)
}

/// Время UNIX как «ГГГГ-ММ-ДД ЧЧ:ММ» по UTC.
fn date(unix: u64) -> String {
    let days = (unix / 86_400) as i64;
    let (hours, minutes) = ((unix % 86_400) / 3600, (unix % 3600) / 60);

    // Дни от 1970-01-01 в григорианскую дату.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era = (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = if shifted_month < 10 { shifted_month + 3 } else { shifted_month - 9 };
    let year = year_of_era + era * 400 + i64::from(month <= 2);

    format!("{:04}-{:02}-{:02} {:02}:{:02}", year, month, day, hours, minutes)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(message: &str) -> Report {
        Report::parse(
            json!({
                "format": 1,
                "install": "0123456789abcdef",
                "version": "0.0.3-alpha",
                "os": "linux x86_64",
                "kind": "panic",
                "message": message,
                "lines": ["строка"],
            })
            .to_string()
            .as_bytes(),
        )
        .expect("отчёт разбирается")
    }

    #[test]
    fn addresses_and_home_folders_are_hidden() {
        assert_eq!(hide_ipv4("от 192.168.1.20:19132 и 10.0.0.1."), "от <ip>:19132 и <ip>.");
        assert_eq!(hide_ipv4("версия 1.2.3.4.5 и 300.1.1.1"), "версия 1.2.3.4.5 и 300.1.1.1");
        assert_eq!(hide_home("/home/lenox/server/world и C:\\Users\\Anya\\x"), "/home/<user>/server/world и C:\\Users\\<user>\\x");
    }

    #[test]
    fn the_same_error_in_other_places_has_one_signature() {
        let one = report("панику вызвал чанк 12 -4 на такте 551\nподробности");
        let two = report("панику вызвал чанк 7 -8 на такте 90");
        let other = report("мир захвачен другим потоком");

        assert_eq!(one.signature(), two.signature());
        assert_ne!(one.signature(), other.signature());
    }

    #[test]
    fn bad_reports_are_refused() {
        assert!(Report::parse(b"not json").is_none());
        assert!(Report::parse(json!({"format": 1, "install": "0123456789abcdef", "version": "x", "kind": "hack", "message": "m"}).to_string().as_bytes()).is_none());
        assert!(Report::parse(json!({"format": 1, "install": "short", "version": "x", "kind": "panic", "message": "m"}).to_string().as_bytes()).is_none());
        // Неправдоподобное: версия не выпуск, система с чужими знаками, пустое сообщение.
        let with = |version: &str, os: &str, message: &str| {
            Report::parse(json!({"format": 1, "install": "0123456789abcdef", "version": version, "os": os, "kind": "error", "message": message}).to_string().as_bytes())
        };
        assert!(with("0.0.2-alpha", "linux x86_64", "ошибка").is_some());
        assert!(with("<script>", "linux x86_64", "ошибка").is_none());
        assert!(with("0.0.2", "LINUX; DROP TABLE", "ошибка").is_none());
        assert!(with("0.0.2", "linux x86_64", "   ").is_none());
    }

    #[test]
    fn a_store_keeps_the_newest_reports_and_survives_a_restart() {
        let root = std::env::temp_dir().join(format!("mcsa-logreceiver-test-{}", random()));
        let mut store = Store::open(&root).unwrap();

        for step in 0..(KEEP as u64 + 5) {
            assert!(store.add(&report(&format!("паника {}", step)), 1_800_000_000 + step).unwrap());
        }

        let signature = report("паника 0").signature();
        assert_eq!(store.files(&signature).len(), KEEP);

        let reopened = Store::open(&root).unwrap();
        assert_eq!(reopened.signatures[&signature].count, KEEP as u64 + 5);
        assert_eq!(reopened.signatures[&signature].installs.len(), 1);

        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn limits_count_per_address_and_forget_after_an_hour() {
        let mut limits = Limits::default();
        let address = Some("203.0.113.5".parse().unwrap());

        for _ in 0..PER_ADDRESS_HOURLY {
            assert!(limits.allow(address, 7200));
        }
        assert!(!limits.allow(address, 7200));
        assert!(limits.allow(address, 7200 + HOUR));

        // Неизвестный отправитель упирается только в общий предел.
        assert!(limits.allow(None, 7200 + HOUR));

        // Номер установки — свой предел.
        for _ in 0..PER_INSTALL_HOURLY {
            assert!(limits.allow_install("0123456789abcdef", 7200 + HOUR));
        }
        assert!(!limits.allow_install("0123456789abcdef", 7200 + HOUR));
        assert!(limits.allow_install("fedcba9876543210", 7200 + HOUR));
    }

    #[test]
    fn dates_are_utc() {
        assert_eq!(date(0), "1970-01-01 00:00");
        assert_eq!(date(1_790_553_600), "2026-09-28 00:00");
    }
}
