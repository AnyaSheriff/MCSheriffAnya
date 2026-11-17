// Подключается root одной строкой в src/plugins/tests.rs; production API не меняет.
// Настоящие компоненты обязательны. Отсутствие fixture — ошибка, не skip.
mod own_mod_source_tests {
    use super::*;

    fn install_mod(root: &Path, name: &str) {
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tmp/mod-test/plugins").join(name);
        let folder = root.join("plugins").join(name);
        std::fs::create_dir_all(folder.join("data")).unwrap();
        for file in ["plugin.toml", "plugin.wasm", "conversion.json"] {
            std::fs::copy(source.join(file), folder.join(file)).unwrap();
        }
    }

    fn messages(shared: &Shared) -> Vec<Vec<u8>> {
        shared.players.lock().unwrap().take_orders(&[1; 16]).into_iter().filter_map(|order| {
            if let crate::players::Order::Message { component } = order { Some(component) } else { None }
        }).collect()
    }

    fn expect_messages(shared: &Shared, expected: &[String]) {
        let expected: Vec<_> = expected.iter().map(|text| crate::network::nbt::text_component(text)).collect();
        assert_eq!(messages(shared), expected, "точные байты NBT, включая порядок обработчиков");
    }

    fn chat(shared: &Shared, from: usize) -> Vec<String> {
        shared.chat.lock().unwrap().since(from).to_vec()
    }

    #[test]
    #[ignore = "нужны настоящие WASM-компоненты build_fixtures.py, CI-only"]
    fn translated_own_mod_source_lifecycle_events_and_isolation() {
        let root = server_root("own-mod-source");
        for name in ["mod-welcome", "mod-welcome-b"] { install_mod(&root, name); }
        let shared = shared();
        let mut host = Host::new(Arc::clone(&shared), root).unwrap();
        host.tick(0);
        host.load("mod-welcome").unwrap();
        host.load("mod-welcome-b").unwrap();
        assert_eq!(chat(&shared, 0), ["mod:start", "mod-b:start"]);
        assert!(host.load("mod-welcome").is_err());
        assert_eq!(chat(&shared, 0).len(), 2, "двойная загрузка не вызывает onStart");
        assert_eq!(host.loaded.len(), 2);
        for plugin in &host.loaded {
            assert!(!plugin.trusted, "свой ModSDK не получает privileged");
            assert_eq!(plugin.store.data().tasks.len(), 1);
            let task = &plugin.store.data().tasks[0];
            assert_eq!(task.task, 5);
            let expected = if plugin.manifest.name == "mod-welcome" { 2 } else { 3 };
            assert_eq!(task.period, Some(expected));
            assert_eq!(task.due, u64::from(expected));
        }
        let uuid = crate::network::login::format_uuid(&[1; 16]);
        host.player_join(&view());
        expect_messages(&shared, &[
            "mod:join:1:Первый".into(), format!("mod:join:2:{uuid}"),
            "mod-b:join:1:Первый".into(), format!("mod-b:join:2:{uuid}"),
        ]);
        host.player_quit(&view());
        assert_eq!(chat(&shared, 2), ["mod:quit:Первый", "mod-b:quit:Первый"]);
        host.tick(1);
        assert!(chat(&shared, 4).is_empty(), "every не вызывается раньше периода");
        host.tick(2);
        assert_eq!(chat(&shared, 4), ["mod:periodic"]);
        host.tick(3);
        assert_eq!(chat(&shared, 5), ["mod-b:periodic"]);
        host.tick(4);
        assert_eq!(chat(&shared, 6), ["mod:periodic"]);
        let other_tasks = host.loaded[1].store.data().tasks.clone();
        host.unload("mod-welcome").unwrap();
        assert_eq!(chat(&shared, 7), ["mod:stop"]);
        assert!(host.unload("mod-welcome").is_err());
        assert_eq!(host.loaded.len(), 1);
        assert_eq!(host.loaded[0].store.data().tasks, other_tasks, "выгрузка не отменяет чужой handle");
        host.tick(6);
        assert_eq!(chat(&shared, 8), ["mod-b:periodic"], "у выгруженного нет задач");
        host.load("mod-welcome").unwrap();
        assert_eq!(chat(&shared, 9), ["mod:start"]);
        assert_eq!(host.loaded[1].store.data().tasks.len(), 1, "перезагрузка без повторной регистрации старых задач");
        assert_eq!(host.loaded[1].store.data().tasks[0].due, 8);
        host.player_join(&view());
        expect_messages(&shared, &[
            "mod-b:join:1:Первый".into(), format!("mod-b:join:2:{uuid}"),
            "mod:join:1:Первый".into(), format!("mod:join:2:{uuid}"),
        ]);
        host.tick(8);
        assert_eq!(chat(&shared, 10), ["mod:periodic"]);
        host.tick(9);
        assert_eq!(chat(&shared, 11), ["mod-b:periodic"]);
        host.unload_all();
        assert!(host.is_empty());
        let mut stops = chat(&shared, 12);
        stops.sort();
        assert_eq!(stops, ["mod-b:stop", "mod:stop"]);
        let end = shared.chat.lock().unwrap().count();
        host.tick(20);
        host.player_join(&view());
        assert!(chat(&shared, end).is_empty());
        assert!(messages(&shared).is_empty());
    }
}
