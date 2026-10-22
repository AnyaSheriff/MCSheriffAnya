// Неудачная проверка лицензии разрешает повторный cracked-вход на пять
// минут. Результат поиска имени — только подсказка, не доказательство прав.
use std::{collections::HashMap, net::IpAddr, sync::Mutex, time::Duration};

pub const RETRY: &str = "Не удалось проверить лицензию. Зайди ещё раз — войдёшь без лицензии.";
#[derive(Default)]
pub struct Detector { state: Mutex<Cache> }
#[derive(Default)]
struct Cache {
    names: HashMap<String, (u64, bool)>,
    retry: HashMap<(String, IpAddr), u64>,
}
impl Detector {
    pub fn failed(&self, name: &str, ip: IpAddr, now: u64) {
        let mut state = self.state.lock().expect("premium");
        state.retry.retain(|_, until| *until > now);
        state.retry.insert((name.to_ascii_lowercase(), ip), now + 300);
    }
    pub async fn should_verify(&self, name: &str, ip: IpAddr) -> bool {
        self.decide(name, ip, super::now(), || lookup(name)).await
    }
    async fn decide<F, Fut>(&self, name: &str, ip: IpAddr, now: u64, lookup: F) -> bool
    where F: FnOnce() -> Fut, Fut: std::future::Future<Output = Option<bool>> {
        let key = name.to_ascii_lowercase();
        {
            let mut state = self.state.lock().expect("premium");
            state.retry.retain(|_, until| *until > now);
            state.names.retain(|_, (until, _)| *until > now);
            if state.retry.contains_key(&(key.clone(), ip)) { return false; }
            if let Some((_, premium)) = state.names.get(&key) { return *premium; }
        }
        let result = lookup().await;
        if let Some(premium) = result {
            self.state.lock().expect("premium").names.insert(key, (now + 1800, premium));
        }
        result.unwrap_or(false)
    }
}
async fn lookup(name: &str) -> Option<bool> {
    let client = crate::skins::client()?;
    // Один общий reqwest Client; общий предел включает чтение тела ответа.
    tokio::time::timeout(Duration::from_secs(3), async {
        let response = client.get(format!("https://api.mojang.com/users/profiles/minecraft/{name}"))
            .timeout(Duration::from_secs(3)).send().await.ok()?;
        profile_response(response, name).await
    }).await.ok().flatten()
}
async fn profile_response(mut response: reqwest::Response, name: &str) -> Option<bool> {
    match response.status().as_u16() {
        204 | 404 => return Some(false),
        200 => {}
        _ => return None,
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.ok()? {
        if bytes.len() + chunk.len() > 4096 { return None; }
        bytes.extend_from_slice(&chunk);
    }
    let value: serde_json::Value = serde_json::from_slice(&bytes).ok()?;
    let id = value.get("id")?.as_str()?;
    let returned = value.get("name")?.as_str()?;
    if id.len() != 32 || !id.bytes().all(|b| b.is_ascii_hexdigit()) || !returned.eq_ignore_ascii_case(name) { return None; }
    Some(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn http_decisions_cache_and_retry_expire() {
        let ip = "127.0.0.1".parse().unwrap();
        for (status, body, expected) in [
            (200, r#"{"id":"16d3c22147764fe380af6f2b651b501b","name":"SheriffAnya"}"#, Some(true)),
            (204, "", Some(false)), (404, "", Some(false)), (500, "", None),
            (200, "broken", None), (200, r#"{"id":"bad","name":"SheriffAnya"}"#, None),
        ] {
            let response: reqwest::Response = http::Response::builder().status(status).body(body).unwrap().into();
            assert_eq!(profile_response(response, "SheriffAnya").await, expected);
        }
        let detector = Detector::default();
        assert!(detector.decide("SheriffAnya", ip, 100, || async { Some(true) }).await);
        assert!(detector.decide("sheriffanya", ip, 1899, || async { panic!("cached") }).await);
        detector.failed("SheriffAnya", ip, 100);
        assert!(!detector.decide("sheriffanya", ip, 399, || async { panic!("retry") }).await);
        assert!(detector.decide("sheriffanya", "127.0.0.2".parse().unwrap(), 399, || async { panic!("cached") }).await);
        assert!(detector.decide("sheriffanya", ip, 400, || async { panic!("cached") }).await);
        assert!(!detector.decide("sheriffanya", ip, 1900, || async { None }).await);
        assert!(!detector.decide("sheriffanya", ip, 1901, || async { Some(false) }).await);
    }
    #[tokio::test]
    async fn license_account_op_decision_table() {
        use super::super::{Service, store::Account, legacy_hash};
        let owner = [0x16, 0xd3, 0xc2, 0x21, 0x47, 0x76, 0x4f, 0xe3, 0x80, 0xaf, 0x6f, 0x2b, 0x65, 0x1b, 0x50, 0x1b];
        let (salt, password_hash) = legacy_hash("correct").unwrap();
        for license in ["ok", "failed-retry", "not-premium", "api-down"] {
            for account in ["new", "licensed", "cracked"] {
                for op in [false, true] {
                    let root = std::env::temp_dir();
                    let path = root.join(format!("matrix-{}-{license}-{account}-{op}.db", std::process::id()));
                    let _ = std::fs::remove_file(&path);
                    let auth = Service::open_test(&path).unwrap();
                    let op_path = path.with_extension("ops.json"); let _ = std::fs::remove_file(&op_path);
                    let mut ops = crate::ops::Ops::open(&op_path);
                    if op { ops.give(&owner, "SheriffAnya", 4); }
                    let offline = crate::network::login::offline_uuid("SheriffAnya");
                    if account != "new" {
                        auth.state.lock().unwrap().db.accounts.insert("sheriffanya".into(), Account {
                            name: "sheriffanya".into(), display_name: "SheriffAnya".into(), salt, password_hash, protected: None,
                            created: 1, last_ip: String::new(), last_login: 1,
                            uuid: if account == "licensed" { owner } else { offline }, premium: account == "licensed",
                        });
                    }
                    let ip = "192.0.2.10".parse().unwrap();
                    let status = match license { "not-premium" => 404, "api-down" => 503, _ => 200 };
                    let response: reqwest::Response = http::Response::builder().status(status)
                        .body(r#"{"id":"16d3c22147764fe380af6f2b651b501b","name":"SheriffAnya"}"#).unwrap().into();
                    let try_license = auth.premium.decide("SheriffAnya", ip, 100, || profile_response(response, "SheriffAnya")).await;
                    assert_eq!(try_license, matches!(license, "ok" | "failed-retry"));
                    if license == "failed-retry" {
                        // Тот же переход вызывается после disconnect/hasJoined failure.
                        auth.premium.failed("SheriffAnya", ip, 100);
                        assert!(!auth.premium.decide("SheriffAnya", ip, 101, || async { panic!("retry must skip HTTP") }).await);
                    }
                    let verified = (license == "ok").then_some(owner);
                    let identity = auth.identity("SheriffAnya", verified);
                    let permitted = Service::registration_allowed(&ops, &identity.name, verified);
                    let expected_uuid = if account == "licensed" || (account == "new" && verified.is_some()) { owner } else { offline };
                    assert_eq!(identity.uuid, expected_uuid);
                    let result = auth.submit(&identity, ip, "correct", Some("correct"), permitted);
                    assert_eq!(result.is_ok(), account != "new" || !op || verified.is_some(), "{license}/{account}/{op}");
                    if result.is_ok() {
                        assert_eq!(ops.level_of(&identity.uuid), if op && expected_uuid == owner { 4 } else { 0 });
                        let reload = Service::open_test(&path).unwrap();
                        assert_eq!(reload.identity("sheriffanya", None).uuid, expected_uuid);
                    }
                    std::fs::remove_file(path).unwrap(); let _ = std::fs::remove_file(op_path);
                }
            }
        }
    }

}
