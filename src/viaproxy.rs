//! Official ViaProxy binary only; no translator source is linked into the core.
use crate::config::{mcsheriffanya::Settings, server_properties::ServerProperties};
use std::{io, path::Path, process::Stdio, time::Duration};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
    sync::oneshot,
};
const JAR: &str = "ViaProxy-3.4.14.jar";
const URL: &str =
    "https://github.com/ViaVersion/ViaProxy/releases/download/v3.4.14/ViaProxy-3.4.14.jar";
const SHA256: &str = "2894bbfb2f4342f8fde887af6c689313efd5b0e46a8006775eea0e21e2948ab5";
fn error(message: impl Into<String>) -> io::Error {
    io::Error::other(message.into())
}
pub fn configure(settings: &mut Settings, properties: &mut ServerProperties) -> io::Result<()> {
    if !matches!(settings.auth_mode.as_str(), "dialog" | "chat") {
        return Err(error("auth-mode: ожидается dialog или chat"));
    }
    if !settings.viaproxy {
        return Ok(());
    }
    if settings.viaproxy_port == 0 || settings.viaproxy_port == properties.server_port {
        return Err(error(
            "ViaProxy: публичный порт должен быть ненулевым и отличаться от server-port",
        ));
    }
    let memory = &settings.viaproxy_memory;
    if memory.len() < 2
        || !matches!(memory.as_bytes().last(), Some(b'M' | b'm' | b'G' | b'g'))
        || memory[..memory.len() - 1]
            .parse::<u32>()
            .ok()
            .is_none_or(|n| n == 0)
    {
        return Err(error(
            "viaproxy-memory: ожидается размер, например 512M или 1G",
        ));
    }
    properties.server_ip = "127.0.0.1".into();
    settings.proxy_protocol = true;
    settings.proxy_protocol_trusted_sources = vec![std::net::Ipv4Addr::LOCALHOST.into()];
    Ok(())
}
fn verified(bytes: &[u8]) -> bool {
    let hash = aws_lc_rs::digest::digest(&aws_lc_rs::digest::SHA256, bytes);
    hash.as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>()
        == SHA256
}
async fn prepare(settings: &Settings, port: u16) -> io::Result<()> {
    let status = Command::new(&settings.viaproxy_java).arg("-version").stdout(Stdio::null()).stderr(Stdio::null()).status().await
        .map_err(|e| error(format!("ViaProxy: Java не найдена или не запускается ({}): {e}. Установите Java 17+ и укажите viaproxy-java.", settings.viaproxy_java)))?;
    if !status.success() {
        return Err(error(
            "ViaProxy: Java не работает. Установите Java 17+; проверьте viaproxy-java.",
        ));
    }
    tokio::fs::create_dir_all("viaproxy").await?;
    let path = Path::new("viaproxy").join(JAR);
    let bytes = match tokio::fs::read(&path).await {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            crate::log_info!("ViaProxy: загрузка официального релиза 3.4.14 по HTTPS");
            let client = reqwest::Client::builder()
                .https_only(true)
                .timeout(Duration::from_secs(180))
                .build()
                .map_err(error_from)?;
            let bytes = client
                .get(URL)
                .send()
                .await
                .map_err(error_from)?
                .error_for_status()
                .map_err(error_from)?
                .bytes()
                .await
                .map_err(error_from)?
                .to_vec();
            if !verified(&bytes) {
                return Err(error(
                    "ViaProxy: SHA-256 загрузки не совпадает; запуск отменён",
                ));
            }
            let temporary = path.with_extension("jar.part");
            tokio::fs::write(&temporary, &bytes).await?;
            tokio::fs::rename(temporary, &path).await?;
            bytes
        }
        Err(e) => return Err(e),
    };
    if !verified(&bytes) {
        return Err(error(
            "ViaProxy: неверный SHA-256 JAR; удалите файл для повторной загрузки",
        ));
    }
    let config = format!(
        "bind-address: 0.0.0.0:{}\ntarget-address: 127.0.0.1:{port}\ntarget-version: '26.1.2'\nproxy-online-mode: false\nauth-method: NONE\nbackend-haproxy: true\nfrontend-haproxy: false\nwildcard-domain-handling: NONE\nallow-legacy-client-passthrough: false\nignore-protocol-translation-errors: false\nlog-ips: true\n",
        settings.viaproxy_port
    );
    tokio::fs::write("viaproxy/viaproxy.yml", config).await
}
fn error_from(e: impl std::fmt::Display) -> io::Error {
    error(format!("ViaProxy: {e}"))
}
fn spawn(settings: &Settings) -> io::Result<Child> {
    let mut child = Command::new(&settings.viaproxy_java)
        .arg(format!("-Xmx{}", settings.viaproxy_memory))
        .args(["-jar", JAR, "config", "viaproxy.yml"])
        .current_dir("viaproxy")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(error_from)?;
    let stdout = child.stdout.take().expect("stdout");
    let stderr = child.stderr.take().expect("stderr");
    for reader in [
        Box::new(stdout) as Box<dyn tokio::io::AsyncRead + Unpin + Send>,
        Box::new(stderr),
    ] {
        tokio::spawn(async move {
            let mut lines = BufReader::new(reader).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                crate::log_info!("[ViaProxy] {}", line);
            }
        });
    }
    Ok(child)
}
pub struct Supervisor {
    stop: Option<oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<()>,
}
impl Supervisor {
    pub async fn start(settings: &Settings, port: u16) -> io::Result<Option<Self>> {
        if !settings.viaproxy {
            return Ok(None);
        }
        prepare(settings, port).await?;
        let mut child = spawn(settings)?;
        let settings = settings.clone();
        let (stop, mut stopped) = oneshot::channel();
        let task = tokio::spawn(async move {
            for attempt in 0..=3 {
                tokio::select! {
                    _ = &mut stopped => { let _ = child.kill().await; let _ = child.wait().await; return; }
                    result = child.wait() => crate::log_warn!("ViaProxy завершился: {:?}", result),
                }
                if attempt == 3 {
                    crate::log_error!("ViaProxy: исчерпаны 3 попытки перезапуска");
                    return;
                }
                tokio::select! {
                    _ = &mut stopped => return,
                    _ = tokio::time::sleep(Duration::from_secs(2 << attempt)) => {}
                }
                match spawn(&settings) {
                    Ok(next) => child = next,
                    Err(e) => {
                        crate::log_error!("{}", e);
                        return;
                    }
                }
            }
        });
        Ok(Some(Self {
            stop: Some(stop),
            task,
        }))
    }
    pub async fn stop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        let _ = (&mut self.task).await;
    }
}
impl Drop for Supervisor {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn missing_java_has_russian_actionable_error() {
        let settings = Settings {
            viaproxy_java: "/nonexistent/mcsa-test-java".into(),
            ..Settings::default()
        };
        let error = prepare(&settings, 26100).await.unwrap_err().to_string();
        assert!(error.contains("Java не найдена"));
        assert!(error.contains("viaproxy-java"));
    }
    #[test]
    fn pin_rejects_corruption() {
        assert!(!verified(b"not a jar"));
    }
    #[test]
    fn isolates_backend() {
        let mut s = Settings {
            viaproxy: true,
            ..Settings::default()
        };
        let mut p = ServerProperties {
            server_port: 26100,
            ..ServerProperties::default()
        };
        configure(&mut s, &mut p).unwrap();
        assert_eq!(p.server_ip, "127.0.0.1");
        assert!(s.proxy_protocol);
        assert_eq!(
            s.proxy_protocol_trusted_sources,
            vec!["127.0.0.1".parse::<std::net::IpAddr>().unwrap()]
        );
        s.viaproxy_memory = "bad".into();
        assert!(configure(&mut s, &mut p).is_err());
    }
}
