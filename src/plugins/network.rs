// Сеть доступна только через явное пересечение декларации и разрешения ядра.
use std::collections::{BTreeSet, HashMap};
use std::net::{IpAddr, SocketAddr};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

use super::api::{PluginCtx, mcsa::api::https};

type Reply = Result<https::Response, String>;
const DEADLINE: Duration = Duration::from_secs(10);
const MAX_BODY: usize = 32 * 1024;
const MAX_RESPONSE: usize = 64 * 1024;

pub fn valid_host(host: &str) -> bool {
    host.len() <= 253 && host.contains('.') && host.parse::<IpAddr>().is_err() && host.split('.').all(|part| {
        !part.is_empty() && part.len() <= 63 && !part.starts_with('-') && !part.ends_with('-')
            && part.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == b'-')
    })
}

pub struct Access {
    allowed: BTreeSet<String>,
    pending: HashMap<u32, Receiver<Reply>>,
    next: u32,
    last: Option<Instant>,
}

impl Access {
    pub fn new(declared: &[String], approved: &[String]) -> Self {
        Self { allowed: declared.iter().filter(|host| valid_host(host) && approved.contains(host)).cloned().collect(), pending: HashMap::new(), next: 0, last: None }
    }

    pub fn description(&self) -> String {
        if self.allowed.is_empty() { "запрещён (нет разрешённых хостов)".into() } else { self.allowed.iter().cloned().collect::<Vec<_>>().join(", ") }
    }

    fn validate(&self, method: &str, url: &str, body: &[u8]) -> Result<reqwest::Url, String> {
        if !matches!(method, "GET" | "POST") || body.len() > MAX_BODY || (method == "GET" && !body.is_empty()) || url.len() > 2048 {
            return Err("HTTPS: неверный метод или превышен размер запроса".into());
        }
        let url = reqwest::Url::parse(url).map_err(|_| "HTTPS: неверный URL")?;
        if url.scheme() != "https" || !url.username().is_empty() || url.password().is_some() || url.fragment().is_some()
            || url.port_or_known_default() != Some(443) || !url.host_str().is_some_and(|host| self.allowed.contains(host)) {
            return Err("HTTPS: хост не объявлен или не разрешён администратором".into());
        }
        Ok(url)
    }
}

// Не допускаем обращение к внутренним службам даже через DNS разрешённого имени.
fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            !ip.is_private() && !ip.is_loopback() && !ip.is_link_local() && !ip.is_broadcast() && !ip.is_documentation()
                && a != 0 && a < 224 && !(a == 100 && (64..=127).contains(&b)) && !(a == 198 && (18..=19).contains(&b))
                && !(a == 192 && b == 0 && c == 0)
        }
        IpAddr::V6(ip) => {
            let segments = ip.segments();
            segments[0] & 0xe000 == 0x2000 && !(segments[0] == 0x2001 && (segments[1] < 0x200 || segments[1] == 0xdb8))
                && segments[0] != 0x2002 && !(segments[0] == 0x3fff && segments[1] < 0x1000)
        }
    }
}

async fn request(method: String, url: reqwest::Url, body: Vec<u8>) -> Reply {
    let host = url.host_str().ok_or("HTTPS: нет хоста")?;
    let addresses: Vec<SocketAddr> = tokio::net::lookup_host((host, 443)).await.map_err(|_| "HTTPS: ошибка DNS")?.take(33).collect();
    if addresses.is_empty() || addresses.len() > 32 || addresses.iter().any(|addr| !public_ip(addr.ip())) {
        return Err("HTTPS: DNS вернул недопустимый адрес".into());
    }
    let client = reqwest::Client::builder().https_only(true).no_proxy().redirect(reqwest::redirect::Policy::none()).referer(false)
        .timeout(DEADLINE).connect_timeout(Duration::from_secs(3)).resolve_to_addrs(host, &addresses)
        .no_gzip().no_brotli().no_deflate().no_zstd().build().map_err(|_| "HTTPS: клиент не создан")?;
    let mut response = if method == "GET" { client.get(url) } else { client.post(url).body(body) }.send().await.map_err(|_| "HTTPS: запрос не выполнен")?;
    let status = response.status().as_u16();
    if response.content_length().is_some_and(|size| size > MAX_RESPONSE as u64) { return Err("HTTPS: ответ слишком большой".into()); }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|_| "HTTPS: ответ не прочитан")? {
        if body.len() + chunk.len() > MAX_RESPONSE { return Err("HTTPS: ответ слишком большой".into()); }
        body.extend_from_slice(&chunk);
    }
    Ok(https::Response { status, body })
}

// Один исполнитель и не больше четырёх потоков DNS. Ожидание системного DNS
// не удерживает поток хозяина и не заставляет деструктор Runtime ждать после таймаута.
fn network_runtime() -> Result<&'static tokio::runtime::Runtime,String> {
    static RUNTIME: std::sync::OnceLock<Result<tokio::runtime::Runtime,String>> = std::sync::OnceLock::new();
    RUNTIME.get_or_init(|| tokio::runtime::Builder::new_multi_thread().worker_threads(1).max_blocking_threads(4).thread_name("plugin https").enable_all().build().map_err(|_| "HTTPS: исполнитель не запущен".into())).as_ref().map_err(Clone::clone)
}
static ACTIVE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
struct Budget;
impl Budget {
    fn acquire() -> Result<Self,String> {
        ACTIVE.fetch_update(std::sync::atomic::Ordering::Relaxed,std::sync::atomic::Ordering::Relaxed,|value| (value<64).then_some(value+1)).map(|_| Self).map_err(|_| "HTTPS: общий лимит запросов".into())
    }
}
impl Drop for Budget {fn drop(&mut self){ACTIVE.fetch_sub(1,std::sync::atomic::Ordering::Relaxed);}}

impl https::Host for PluginCtx {
    fn start(&mut self, method: String, url: String, body: Vec<u8>) -> Result<u32, String> {
        let url = self.network.validate(&method, &url, &body)?;
        if self.network.pending.len() >= 4 || self.network.last.is_some_and(|last| last.elapsed() < Duration::from_secs(1)) {
            return Err("HTTPS: лимит 4 незабранных запросов и 1 новый запрос в секунду".into());
        }
        let (sender, receiver) = mpsc::sync_channel(1);
        let budget = Budget::acquire()?;
        network_runtime()?.spawn(async move {
            let _budget = budget;
            let result = tokio::time::timeout(DEADLINE, request(method,url,body)).await.map_err(|_| "HTTPS: время истекло".to_string()).and_then(|reply| reply);
            let _ = sender.send(result);
        });
        self.network.next = self.network.next.wrapping_add(1).max(1);
        let handle = self.network.next;
        self.network.pending.insert(handle, receiver);
        self.network.last = Some(Instant::now());
        Ok(handle)
    }

    fn poll(&mut self, handle: u32) -> Result<Option<https::Response>, String> {
        let receiver = self.network.pending.get(&handle).ok_or("HTTPS: неизвестный запрос")?;
        match receiver.try_recv() {
            Ok(reply) => { self.network.pending.remove(&handle); reply.map(Some) }
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => { self.network.pending.remove(&handle); Err("HTTPS: исполнитель завершился".into()) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn global_request_budget_is_bounded_and_released() {
        let budgets: Vec<_> = (0..64).map(|_| Budget::acquire().unwrap()).collect();assert!(Budget::acquire().is_err());drop(budgets);assert!(Budget::acquire().is_ok());
    }
    #[test]
    fn declaration_needs_explicit_approval_and_exact_host() {
        let declared = vec!["api.example.org".into()];
        assert!(Access::new(&declared, &[]).validate("GET", "https://api.example.org/", &[]).is_err());
        let access = Access::new(&declared, &declared);
        assert!(access.validate("GET", "https://api.example.org/path?q=x", &[]).is_ok());
        for url in ["http://api.example.org/", "https://sub.api.example.org/", "https://api.example.org.evil.test/", "https://user:password@api.example.org/", "https://api.example.org:444/", "https://api.example.org/#x", "https://127.0.0.1/"] {
            assert!(access.validate("GET", url, &[]).is_err(), "{url}");
        }
        assert!(access.validate("DELETE", "https://api.example.org/", &[]).is_err());
        assert!(access.validate("POST", "https://api.example.org/", &vec![0; MAX_BODY + 1]).is_err());
        assert!(access.validate("GET", "https://api.example.org/", &[1]).is_err());
    }
    #[test]
    fn dns_must_resolve_to_public_addresses() {
        for ip in ["127.0.0.1", "10.1.2.3", "169.254.169.254", "100.64.0.1", "192.0.2.1", "198.18.0.1", "224.0.0.1", "::1", "::ffff:127.0.0.1", "fc00::1", "2001:db8::1", "2002:7f00:1::1"] { assert!(!public_ip(ip.parse().unwrap()), "{ip}"); }
        for ip in ["1.1.1.1", "8.8.8.8", "2606:4700:4700::1111"] { assert!(public_ip(ip.parse().unwrap()), "{ip}"); }
        for host in ["*.example.org", "https://example.org", "example.org:443", "localhost", "EXAMPLE.org", "example.org.", "127.0.0.1"] { assert!(!valid_host(host), "{host}"); }
    }
}
