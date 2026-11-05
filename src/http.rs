//! Общие настройки HTTPS для самостоятельного процесса сервера.

pub(crate) fn builder() -> reqwest::ClientBuilder {
    let builder = reqwest::Client::builder();
    #[cfg(target_os = "android")]
    {
        // Termux не предоставляет JavaVM/Context, требуемые системным
        // rustls-platform-verifier. «Use only the provided certificate roots»:
        // https://docs.rs/reqwest/0.13.5/reqwest/struct.ClientBuilder.html#method.tls_certs_only
        // Корни Mozilla встроены в файл; проверка цепочки, имени и срока остаётся.
        let roots = webpki_root_certs::TLS_SERVER_ROOT_CERTS.iter().map(|root|
            reqwest::Certificate::from_der(root.as_ref())
                .expect("встроенный корневой сертификат Mozilla должен быть DER"));
        builder.tls_certs_only(roots)
    }
    #[cfg(not(target_os = "android"))]
    { builder }
}

#[cfg(test)]
mod tests {
    #[test]
    fn common_http_client_builds_without_android_java_context() {
        super::builder().build().expect("HTTPS-клиент без JavaVM");
    }

    #[tokio::test]
    async fn caller_https_only_policy_is_preserved() {
        let client = super::builder().https_only(true).no_proxy()
            .timeout(std::time::Duration::from_secs(1)).build().unwrap();
        let error = client.get("http://127.0.0.1:9/").send().await.unwrap_err();
        assert!(error.is_builder(), "HTTP должен отклоняться до соединения: {error}");
        assert!(!error.is_connect());
    }
}
