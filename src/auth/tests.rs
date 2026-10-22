use super::*;
use crate::network::{nbt::Compound, packet, varint::encode_string};

fn path(name: &str) -> std::path::PathBuf {
    let p = std::env::temp_dir().join(format!("auth-{}-{name}.db", std::process::id()));
    let _ = std::fs::remove_file(&p); let _ = std::fs::remove_dir_all(p.with_extension("guard")); p
}
fn service(name: &str) -> Arc<Service> { Arc::new(Service::open_test(&path(name)).unwrap()) }
fn ip() -> IpAddr { "192.0.2.1".parse().unwrap() }

#[test]
fn hashing_roundtrip_identity_and_limits() {
    let p = path("roundtrip");
    let auth = Service::open_test(&p).unwrap();
    let identity = auth.identity("Alice", Some([42; 16]));
    assert!(auth.submit(&identity, ip(), "correct", Some("correct"), true).is_ok());
    let auth = Service::open_test(&p).unwrap();
    let identity = auth.identity("alice", None);
    assert_eq!(identity.uuid, [42; 16]); assert_eq!(identity.name, "Alice");
    assert!(auth.submit(&identity, ip(), "correct", None, true).is_ok());
    assert_eq!(auth.submit(&identity, ip(), "wrong", None, true), Err("Неверный пароль"));
    let state = auth.state.lock().unwrap();
    let a = state.db.accounts.get("alice").unwrap();
    assert!(a.premium); assert!(auth.check_password(a, "correct").unwrap()); assert!(!auth.check_password(a, "Correct").unwrap());
    assert_eq!(a.salt, [0; 16]); assert_eq!(a.password_hash, [0; 32]);
    assert_ne!(a.protected, Some(auth.new_password("alice", "correct").unwrap()));
    drop(state);
    let mut limits = Limits::default();
    for _ in 0..5 { assert!(!limits.blocked(ip(), 100)); limits.fail(ip(), 100); }
    assert!(limits.blocked(ip(), 699)); assert!(!limits.blocked(ip(), 700));
    std::fs::remove_file(p).unwrap();
}

#[test]
fn registration_rules_daily_quota_and_protected_op() {
    let auth = service("rules");
    let id = auth.identity("Alice", None);
    assert_eq!(auth.submit(&id, ip(), "abc", Some("abc"), true), Err("Пароль должен содержать 4–64 символа"));
    assert_eq!(auth.submit(&id, ip(), "abcd", Some("abcd!"), true), Err("Пароли не совпадают"));
    assert_eq!(auth.submit(&id, ip(), "abcd", Some("abcd"), false), Err(OP_PROTECTED));
    for name in ["Alice", "Bob", "Eve"] {
        assert!(auth.submit(&auth.identity(name, None), ip(), "abcd", Some("abcd"), true).is_ok());
    }
    assert!(auth.submit(&auth.identity("Fourth", None), ip(), "abcd", Some("abcd"), true).is_err());
    let op_path = path("op"); let mut ops = crate::ops::Ops::open(&op_path);
    ops.give(&[7; 16], "SheriffAnya", 4);
    assert!(!Service::registration_allowed(&ops, "sheriffanya", None));
    assert!(!Service::registration_allowed(&ops, "SheriffAnya", Some([8; 16])));
    assert!(Service::registration_allowed(&ops, "sheriffanya", Some([7; 16])));
    assert!(Service::registration_allowed(&ops, "Alice", None));
}

#[test]
fn custom_click_strict_nbt_and_unicode() {
    let mut payload = encode_string("mcsheriffanya:register");
    payload.extend(Compound::new().string("password", "пароль😀").string("repeat", "пароль😀").encode_network());
    let click = dialog::parse(&payload).unwrap();
    assert_eq!(click.password, "пароль😀"); assert_eq!(click.repeat.as_deref(), Some("пароль😀"));
    for end in 0..payload.len() { assert!(dialog::parse(&payload[..end]).is_err()); }
    payload.push(0); assert!(dialog::parse(&payload).is_err());
    let mut duplicate = encode_string("mcsheriffanya:login");
    duplicate.extend(Compound::new().string("password", "abcd").string("password", "abcd").encode_network());
    assert!(dialog::parse(&duplicate).is_err());
    let mut nested = encode_string("mcsheriffanya:login");
    nested.extend(Compound::new().compound("password", Compound::new()).encode_network());
    assert!(dialog::parse(&nested).is_err());
}

#[test]
fn custom_click_with_payload_size_like_the_real_client() {
    // Настоящий клиент 26.1.2 ставит перед NBT его длину (живой вход на альфе).
    let nbt = Compound::new().string("password", "abcd").encode_network();
    let mut payload = encode_string("mcsheriffanya:login");
    payload.extend(crate::network::varint::encode_varint(nbt.len() as i32));
    payload.extend(&nbt);
    assert_eq!(dialog::parse(&payload).unwrap().password, "abcd");
    let mut exit = encode_string("mcsheriffanya:exit");
    exit.extend([1, 0]);
    assert_eq!(dialog::parse(&exit).unwrap().id, "mcsheriffanya:exit");
}

async fn click<S: tokio::io::AsyncWrite + Unpin>(stream: &mut S, action: &str, password: &str, register: bool) {
    let mut body = vec![dialog::CLICK as u8]; body.extend(encode_string(action));
    let mut nbt = Compound::new().string("password", password);
    if register { nbt = nbt.string("repeat", password); }
    body.extend(nbt.encode_network());
    let mut frame = Vec::new(); packet::frame(&mut frame, &body, packet::compression_threshold());
    packet::write_bytes(stream, &frame).await.unwrap();
}

#[tokio::test]
async fn configuration_dialog_gate_success_and_three_failures() {
    for registered in [false, true] {
        let auth = service(if registered { "flow-login" } else { "flow-register" });
        if registered {
            auth.submit(&auth.identity("Alice", None), ip(), "correct", Some("correct"), true).unwrap();
        }
        let identity = auth.identity("Alice", None);
        let ops = Mutex::new(crate::ops::Ops::open(&path(if registered { "ops-login" } else { "ops-register" })));
        let (mut server, mut client) = tokio::io::duplex(8192);
        let serving = dialog::authenticate(&mut server, &auth, &identity, ip(), &ops, None, tokio::time::Instant::now() + std::time::Duration::from_secs(90));
        let joining = async {
            assert_eq!(packet::read_packet(&mut client).await.unwrap().id, dialog::SHOW);
            if registered {
                for attempt in 1..=3 {
                    click(&mut client, "mcsheriffanya:login", "wrong", false).await;
                    assert_eq!(packet::read_packet(&mut client).await.unwrap().id, if attempt == 3 { 2 } else { dialog::SHOW });
                }
            } else {
                click(&mut client, "mcsheriffanya:register", "correct", true).await;
                assert_eq!(packet::read_packet(&mut client).await.unwrap().id, dialog::CLEAR);
            }
        };
        let (result, _) = tokio::join!(serving, joining);
        assert_eq!(result.unwrap(), !registered);
    }
}

#[tokio::test(start_paused = true)]
async fn configuration_timeout_is_total() {
    let auth = service("timeout"); let identity = auth.identity("Alice", None);
    let ops = Mutex::new(crate::ops::Ops::open(&path("ops-timeout")));
    let (mut server, mut client) = tokio::io::duplex(8192);
    let serving = dialog::authenticate(&mut server, &auth, &identity, ip(), &ops, None, tokio::time::Instant::now() + std::time::Duration::from_secs(90));
    let joining = async {
        assert_eq!(packet::read_packet(&mut client).await.unwrap().id, dialog::SHOW);
        tokio::time::sleep(std::time::Duration::from_secs(91)).await;
        assert_eq!(packet::read_packet(&mut client).await.unwrap().id, 2);
    };
    let (result, _) = tokio::join!(serving, joining); assert!(!result.unwrap());
}

// Независимый разбор небольшого подмножества NBT нужен тесту схемы: он
// проверяет типы и вложенность полей, а не просто наличие слов в байтах.
fn nbt_value(kind: u8, bytes: &mut &[u8]) -> serde_json::Value {
    use serde_json::{Value, json};
    fn take<const N: usize>(bytes: &mut &[u8]) -> [u8; N] {
        let (head, tail) = bytes.split_at(N); *bytes = tail; head.try_into().unwrap()
    }
    fn string(bytes: &mut &[u8]) -> String {
        let len = u16::from_be_bytes(take(bytes)) as usize;
        let (text, tail) = bytes.split_at(len); *bytes = tail;
        String::from_utf8(text.to_vec()).unwrap()
    }
    match kind {
        1 => json!(take::<1>(bytes)[0]),
        3 => json!(i32::from_be_bytes(take(bytes))),
        8 => json!(string(bytes)),
        9 => {
            let kind = take::<1>(bytes)[0]; let count = i32::from_be_bytes(take(bytes));
            Value::Array((0..count).map(|_| nbt_value(kind, bytes)).collect())
        }
        10 => {
            let mut map = serde_json::Map::new();
            loop {
                let kind = take::<1>(bytes)[0]; if kind == 0 { break; }
                let key = string(bytes); assert!(map.insert(key, nbt_value(kind, bytes)).is_none());
            }
            Value::Object(map)
        }
        _ => panic!("unexpected NBT tag {kind}"),
    }
}
#[test]
fn dialog_matches_pinned_wiki_schema() {
    for registered in [false, true] {
        let bytes = dialog::encode(registered, Some("Ошибка"));
        assert_eq!(bytes[0], 10);
        let mut rest = &bytes[1..]; let v = nbt_value(10, &mut rest); assert!(rest.is_empty());
        assert_eq!(v["type"], "minecraft:confirmation");
        assert_eq!(v["can_close_with_escape"], 0); assert_eq!(v["pause"], 0);
        assert_eq!(v["after_action"], "wait_for_response");
        assert!(v["title"]["text"].as_str().unwrap().contains("MCSheriffAnya"));
        assert_eq!(v["body"][1]["contents"]["color"], "red");
        assert_eq!(v["body"][0]["type"], "minecraft:plain_message");
        let inputs = v["inputs"].as_array().unwrap(); assert_eq!(inputs.len(), if registered { 1 } else { 2 });
        for input in inputs { assert_eq!(input["type"], "minecraft:text"); assert_eq!(input["max_length"], 64); }
        assert_eq!(inputs[0]["key"], "password");
        if !registered { assert_eq!(inputs[1]["key"], "repeat"); }
        assert_eq!(v["yes"]["action"]["type"], "minecraft:dynamic/custom");
        assert_eq!(v["yes"]["action"]["id"], if registered { "mcsheriffanya:login" } else { "mcsheriffanya:register" });
        assert_eq!(v["no"]["action"]["id"], "mcsheriffanya:exit");
        assert_eq!(v["no"]["label"]["text"], "Выйти");
    }
}

#[test]
fn corrupt_database_is_never_reset_and_failed_save_does_not_commit() {
    let p = path("corrupt"); std::fs::write(&p, "broken").unwrap();
    assert!(Service::open_test(&p).is_err()); assert_eq!(std::fs::read_to_string(&p).unwrap(), "broken");
    struct Broken;
    impl Store for Broken {
        fn load(&self) -> io::Result<Database> { Ok(Database::default()) }
        fn save(&self, _: &Database) -> io::Result<()> { Err(io::Error::other("failure")) }
    }
    let auth = service("save-failure");
    auth.state.lock().unwrap().store = Box::new(Broken);
    assert!(auth.submit(&auth.identity("Alice", None), ip(), "abcd", Some("abcd"), true).is_err());
    assert!(!auth.identity("Alice", None).registered);
}

#[tokio::test]
async fn configuration_success_existing_account_exit_and_early_finish() {
    for action in ["mcsheriffanya:login", "mcsheriffanya:exit", "finish"] {
        let auth = service(action.rsplit(':').next().unwrap());
        auth.submit(&auth.identity("Alice", Some([9; 16])), ip(), "correct", Some("correct"), true).unwrap();
        let identity = auth.identity("Alice", None);
        assert_eq!(identity.uuid, [9; 16]);
        let ops = Mutex::new(crate::ops::Ops::open(&path(&format!("ops-{action}"))));
        let (mut server, mut client) = tokio::io::duplex(8192);
        let serving = dialog::authenticate(&mut server, &auth, &identity, ip(), &ops, None,
            tokio::time::Instant::now() + std::time::Duration::from_secs(90));
        let joining = async {
            assert_eq!(packet::read_packet(&mut client).await.unwrap().id, dialog::SHOW);
            if action == "finish" {
                let mut frame = Vec::new(); packet::frame(&mut frame, &[3], packet::compression_threshold());
                packet::write_bytes(&mut client, &frame).await.unwrap();
            } else { click(&mut client, action, "correct", false).await; }
            assert_eq!(packet::read_packet(&mut client).await.unwrap().id, if action == "mcsheriffanya:login" { dialog::CLEAR } else { 2 });
        };
        let (result, _) = tokio::join!(serving, joining);
        assert_eq!(result.unwrap(), action == "mcsheriffanya:login");
    }
}

#[test]
fn concurrent_registration_cannot_overwrite_or_change_advertised_uuid() {
    let auth = service("race");
    let first = auth.identity("Alice", Some([1; 16]));
    let second = auth.identity("alice", None);
    auth.submit(&first, ip(), "aaaa", Some("aaaa"), true).unwrap();
    assert_eq!(auth.submit(&second, ip(), "bbbb", Some("bbbb"), true), Err("Аккаунт изменился. Зайди ещё раз."));
    let identity = auth.identity("ALICE", None);
    assert_eq!(identity.uuid, [1; 16]);
    assert!(auth.submit(&identity, ip(), "aaaa", None, true).is_ok());
}

#[test]
fn console_commands_password_change_and_session_requirement() {
    use crate::{commands::{run, Source}, shared::Shared, world::World};
    let auth = service("commands");
    let mut shared = Shared::new(World::in_memory(), World::in_memory(), path("players"), path("skins"),
        Default::default(), crate::ops::Ops::open(&path("commands-ops")), Default::default());
    shared.auth = Some(auth.clone());
    shared.ops.lock().unwrap().give(&[7; 16], "SheriffAnya", 4);
    assert_eq!(run("auth setpassword SheriffAnya correct", &Source::Console, &shared).reply, "Готово");
    let identity = auth.identity("SheriffAnya", None); assert_eq!(identity.uuid, [7; 16]);
    let player = Source::Player { name: identity.name.clone(), uuid: identity.uuid };
    assert!(run("auth setpassword Other hacked", &player, &shared).reply.contains("только из консоли"));
    assert_eq!(run("changepassword correct newpass", &player, &shared).reply, "Сначала войди с паролем");
    auth.submit(&identity, ip(), "correct", None, false).unwrap();
    let session = auth.session(&identity);
    assert_eq!(run("changepassword wrong newpass", &player, &shared).reply, "Неверный пароль");
    assert_eq!(run("changepassword correct \"new pass\"", &player, &shared).reply, "Готово");
    assert!(auth.submit(&identity, ip(), "new pass", None, false).is_ok());
    assert!(auth.submit(&identity, ip(), "correct", None, false).is_err());
    drop(session);
    assert_eq!(run("changepassword \"new pass\" final", &player, &shared).reply, "Сначала войди с паролем");
    let list = run("auth list", &Source::Console, &shared).reply;
    assert!(list.contains("1")); assert!(list.contains("SheriffAnya")); assert!(!list.contains("new pass"));
    assert_eq!(run("auth unregister sheriffanya", &Source::Console, &shared).reply, "Готово");
    assert!(!auth.identity("SheriffAnya", None).registered);
    for line in ["auth setpassword A secret", " /changepassword a b", "changepassword"] { assert!(sensitive_command(line)); }
    assert!(!sensitive_command("say auth"));
}

#[test]
fn daily_registration_limit_survives_unregister_and_restart() {
    let p = path("quota");
    let auth = Service::open_test(&p).unwrap();
    for name in ["Alice", "Bob", "Eve"] {
        auth.submit(&auth.identity(name, None), ip(), "abcd", Some("abcd"), true).unwrap();
    }
    {
        let mut state = auth.state.lock().unwrap();
        state.db.accounts.clear(); state.store.save(&state.db).unwrap();
    }
    let auth = Service::open_test(&p).unwrap();
    let id = auth.identity("Fourth", None);
    assert!(auth.submit(&id, ip(), "abcd", Some("abcd"), true).is_err());
    auth.state.lock().unwrap().db.registrations.iter_mut().for_each(|(_, t)| *t = now() - 86400);
    assert!(auth.submit(&id, ip(), "abcd", Some("abcd"), true).is_ok());
}

#[tokio::test]
async fn login_success_uuid_and_configuration_finish_are_password_gated() {
    use crate::network::login::send_login_success;
    let auth = service("login-config");
    auth.submit(&auth.identity("Alice", Some([42; 16])), ip(), "correct", Some("correct"), true).unwrap();
    let id = auth.identity("Alice", None);
    let ops = Mutex::new(crate::ops::Ops::open(&path("login-config-ops")));
    let (mut server, mut client) = tokio::io::duplex(8192);
    let serving = async {
        send_login_success(&mut server, &id.name, &id.uuid, None, -1).await.unwrap();
        assert_eq!(packet::read_packet(&mut server).await.unwrap().id, 3);
        assert!(dialog::authenticate(&mut server, &auth, &id, ip(), &ops, None,
            tokio::time::Instant::now() + std::time::Duration::from_secs(90)).await.unwrap());
        let mut finish = Vec::new(); packet::frame(&mut finish, &[3], -1);
        packet::write_bytes(&mut server, &finish).await.unwrap();
    };
    let joining = async {
        let success = packet::read_packet(&mut client).await.unwrap();
        assert_eq!(success.id, 2); assert_eq!(&success.payload()[..16], &[42; 16]);
        packet::write_bytes(&mut client, &[1, 3]).await.unwrap();
        assert_eq!(packet::read_packet(&mut client).await.unwrap().id, dialog::SHOW);
        // Даже после неправильного пароля клиент видит только новый диалог.
        click(&mut client, "mcsheriffanya:login", "wrong", false).await;
        assert_eq!(packet::read_packet(&mut client).await.unwrap().id, dialog::SHOW);
        click(&mut client, "mcsheriffanya:login", "correct", false).await;
        assert_eq!(packet::read_packet(&mut client).await.unwrap().id, dialog::CLEAR);
        assert_eq!(packet::read_packet(&mut client).await.unwrap().id, 3);
    };
    tokio::join!(serving, joining);
}

#[test]
fn legacy_hash_upgrades_on_login_and_secret_is_required() {
    let p=path("migration");
    let auth=Service::open_test(&p).unwrap();
    let identity=auth.identity("Alice",Some([42;16]));
    // Запись «до Guard»: PBKDF2 без секрета, версия 1.
    let (salt,password_hash)=legacy_hash("correct").unwrap();
    let mut db=FileStore::new(&p).load().unwrap();
    db.accounts.insert("alice".into(),Account { name:"alice".into(),display_name:"Alice".into(),password_hash,salt,protected:None,
        created:1,last_ip:String::new(),last_login:0,uuid:[42;16],premium:true });
    FileStore::new(&p).save(&db).unwrap();
    drop(auth);
    let auth=Service::open_test(&p).unwrap();
    let identity_registered=auth.identity("Alice",None);
    assert_eq!(identity_registered.uuid,[42;16]); let _=identity;
    let original=std::fs::read(&p).unwrap();
    assert!(auth.submit(&identity_registered,ip(),"wrong",None,true).is_err());
    assert_eq!(std::fs::read(&p).unwrap(),original); // Ошибка ничего не переписывает.
    auth.submit(&identity_registered,ip(),"correct",None,true).unwrap();
    {
        let state=auth.state.lock().unwrap();
        let account=&state.db.accounts["alice"];
        assert_eq!(state.db.version,2);
        assert_eq!((account.salt,account.password_hash),([0;16],[0;32]));
        assert!(account.protected.as_deref().unwrap().starts_with("g1."));
    }
    assert!(auth.submit(&identity_registered,ip(),"correct",None,true).is_ok());
    assert!(auth.submit(&identity_registered,ip(),"wrong",None,true).is_err());
    // Новый аккаунт сразу получает Argon2id-запись.
    let bob=auth.identity("Bob",None);
    auth.submit(&bob,ip(),"длинный😀пароль",Some("длинный😀пароль"),true).unwrap();
    assert!(auth.state.lock().unwrap().db.accounts["bob"].protected.is_some());
    // Потеря секрета не пускает и не создаёт новый секрет при наличии записей.
    drop(auth);
    let secret=p.with_extension("guard").join("pepper");
    std::fs::remove_file(&secret).unwrap();
    assert!(Service::open_test(&p).is_err());
    assert!(!secret.exists());
}

#[test]
fn changepassword_and_setpassword_use_new_scheme() {
    use crate::{commands::{run,Source}, shared::Shared,world::World};
    let p=path("commands-scheme");
    let auth=Arc::new(Service::open_test(&p).unwrap());
    let identity=auth.identity("Alice",Some([42;16]));
    auth.submit(&identity,ip(),"correct",Some("correct"),true).unwrap();
    let identity=auth.identity("Alice",None);
    let dir=std::env::temp_dir().join(format!("auth-commands-{}",std::process::id()));
    let mut shared=Shared::new(World::in_memory(),World::in_memory(),dir.join("players"),dir.join("skins"),
        Default::default(),crate::ops::Ops::open(&dir.join("ops")),Default::default());
    shared.auth=Some(auth.clone());
    let player=Source::Player { name:identity.name.clone(),uuid:identity.uuid };
    let _session=auth.session(&identity);
    assert_eq!(run("changepassword correct newpass",&player,&shared).reply,"Готово");
    auth.submit(&identity,ip(),"newpass",None,true).unwrap();
    assert!(auth.submit(&identity,ip(),"correct",None,true).is_err());
    assert_eq!(run("auth setpassword Alice correct",&Source::Console,&shared).reply,"Готово");
    auth.submit(&identity,ip(),"correct",None,true).unwrap();
    assert!(auth.state.lock().unwrap().db.accounts["alice"].protected.is_some());
    let _=std::fs::remove_dir_all(dir);
}
