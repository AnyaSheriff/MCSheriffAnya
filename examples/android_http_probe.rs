//! Узкая проверка настоящего Android-процесса без JNI, не запуск сервера.
#[path = "../src/http.rs"]
mod http;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let untrusted = std::env::args().nth(1).ok_or("нужен URL самоподписанного стенда")?;
    let resolved = std::env::args().nth(2).ok_or("нужны IPv4-адреса example.com со стенда")?;
    let addresses = resolved.split(',').map(str::parse::<std::net::SocketAddr>)
        .collect::<Result<Vec<_>, _>>()?;
    if addresses.is_empty() || addresses.len() > 16
        || addresses.iter().any(|address| !address.is_ipv4() || address.port() != 443) {
        return Err("нужны от одного до шестнадцати IPv4-адресов с портом 443".into());
    }
    let client = http::builder().https_only(true).no_proxy()
        // DNS эмулятора не участвует; URL/SNI и проверяемое имя остаются example.com.
        .resolve_to_addrs("example.com", &addresses)
        .timeout(std::time::Duration::from_secs(30)).build()?;
    println!("PASS: standalone Android HTTP client built without JavaVM");
    let response = client.get("https://example.com/").send().await?.error_for_status()?;
    println!("PASS: trusted HTTPS status={}", response.status());
    let error = client.get(&untrusted).send().await.expect_err("самоподписанный сертификат должен отклоняться");
    let mut messages = error.to_string();
    let mut cause = std::error::Error::source(&error);
    while let Some(error) = cause {
        messages.push_str(&format!("; {error}"));
        cause = error.source();
    }
    // Отказ сети не засчитывается как проверка сертификата.
    if !messages.contains("UnknownIssuer") {
        return Err(format!("ожидался UnknownIssuer, получено: {messages}").into());
    }
    println!("PASS: untrusted self-signed TLS rejected with UnknownIssuer");
    Ok(())
}
