// Включается из tests.rs: настоящий собственный Java fixture -> TeaVM -> WIT -> Host.
// Без готового компонента проверка обязана завершиться ошибкой, не пропуском.

fn bossbar_fixture(root: &Path) {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tmp/java-test/plugins/java-bossbar");
    let folder = root.join("plugins/java-bossbar");
    std::fs::create_dir_all(folder.join("data")).unwrap();
    for file in ["plugin.toml", "plugin.wasm"] {
        std::fs::copy(source.join(file), folder.join(file)).unwrap();
    }
}

fn bossbar_sender(n: u8) -> wit::Sender {
    wit::Sender { kind: wit::SenderKind::Player, uuid: Some(crate::network::login::format_uuid(&[n;16])), name: format!("Игрок{n}") }
}

fn bossbar_second_player(shared: &Shared) {
    let mut players = shared.players.lock().unwrap();
    let mut member = players.members()[0].clone();
    member.uuid = [2;16];
    member.name = "Второй".into();
    member.entity_id = 2;
    players.join(member, 2);
    players.ui.info.insert([2;16], crate::presentation::Info { health: 20.0, max_health: 20.0, ..Default::default() });
}

fn bossbar_command(host: &mut Host, shared: &Shared, n: u8, phase: &str, count: usize) -> Vec<Vec<u8>> {
    let result = host.run_command(bossbar_sender(n), "barprobe", &[phase.into()]);
    assert!(host.is_loaded("java-bossbar"), "BossBar, этап {phase}: компонент выгружен, ответ {result:?}");
    let (handled, message) = result.unwrap_or_else(|| panic!("BossBar, этап {phase}: команда не найдена"));
    assert!(handled, "BossBar, этап {phase}: команда отклонена, ответ {message:?}");
    let orders = shared.players.lock().unwrap().take_orders(&[n;16]);
    let mut packets = Vec::new();
    let mut messages = Vec::new();
    for order in orders {
        match order {
            crate::players::Order::Packet(packet) => {
                assert_eq!(packet[0], crate::scoreboard::BOSS_PACKET as u8);
                assert!(packet.len() >= 18);
                packets.push(packet);
            }
            crate::players::Order::Message { component } => messages.push(component),
            _ => panic!("неожиданный побочный эффект полосы босса"),
        }
    }
    assert_eq!(messages, [crate::network::nbt::text_component(&format!("BossBar {phase} {count}"))]);
    packets
}

fn bossbar_other_packets(shared: &Shared, n: u8) -> Vec<Vec<u8>> {
    shared.players.lock().unwrap().take_orders(&[n;16]).into_iter().map(|order| {
        let crate::players::Order::Packet(packet) = order else { panic!("чужой игрок получил сообщение или иной эффект"); };
        assert_eq!(packet[0], crate::scoreboard::BOSS_PACKET as u8);
        packet
    }).collect()
}

fn bossbar_actions(packets: &[Vec<u8>]) -> Vec<u8> { packets.iter().map(|p| p[17]).collect() }

fn assert_tab_presentation(host: &mut Host, shared: &Shared) {
    use crate::{network::nbt::text_component, players::Order, presentation::{header_packet, list_packet}};
    use serde_json::json;
    use super::api::mcsa::api::presentation::Host as _;
    let player = [1; 16];
    let viewer = [2; 16];

    assert_eq!(host.run_command(bossbar_sender(1), "barprobe", &["tab".into()]), Some((true, String::new())));
    let update = list_packet(&player, Some(&json!({"text":"TabAlias"})));
    let header = header_packet(
        &json!({"text":"TabHead","extra":[{"text":"\n"},{"text":"LineTwo"}]}),
        &json!({"text":"TabFoot"}),
    );
    {
        let mut players = shared.players.lock().unwrap();
        assert_eq!(players.ui.displays[&player].0, "java-bossbar");
        assert!(matches!(players.take_orders(&player).as_slice(),
            [Order::Packet(list), Order::Packet(tab), Order::Message { component }]
            if *list == update && *tab == header && *component == text_component("Tab updated")));
        assert!(matches!(players.take_orders(&viewer).as_slice(), [Order::Packet(list)] if *list == update));
        assert!(!players.ui.displays.contains_key(&viewer));
    }

    assert_eq!(host.run_command(bossbar_sender(1), "barprobe", &["tab-reset".into()]), Some((true, String::new())));
    let reset = list_packet(&player, None);
    let mut players = shared.players.lock().unwrap();
    assert!(matches!(players.take_orders(&player).as_slice(), [Order::Packet(list), Order::Message { component }]
        if *list == reset && *component == text_component("Tab reset")));
    assert!(matches!(players.take_orders(&viewer).as_slice(), [Order::Packet(list)] if *list == reset));
    assert!(!players.ui.displays.contains_key(&player));
    assert!(!players.ui.list_names.contains_key(&player));
    drop(players);
    let ctx = host.loaded[0].store.data_mut();
    assert!(ctx.reset_tab_header_footer(crate::network::login::format_uuid(&player)));
    let orders = shared.players.lock().unwrap().take_orders(&player);
    assert!(matches!(orders.as_slice(), [Order::Packet(packet)] if *packet == header_packet(&json!(""), &json!(""))));
}

fn assert_race_presentation(host: &mut Host, shared: &Shared) {
    use crate::{network::nbt::text_component, players::Order, survival::{Effect, EffectKind}, world::WorldEvent};
    let player = [1; 16];
    {
        let mut players = shared.players.lock().unwrap();
        let info = players.ui.info.get_mut(&player).unwrap();
        info.selected = 4;
        info.plugin_state.effects.push(Effect {
            kind: EffectKind::Speed, amplifier: 0, ticks_left: 80, ambient: false, visible: true, show_icon: true,
        });
    }
    let world = shared.world_in(crate::dimension::Dimension::Overworld);
    let from = world.lock().unwrap().watch_events(0);
    assert_eq!(host.run_command(bossbar_sender(1), "barprobe", &["race-presentation".into()]), Some((true, String::new())));
    {
        let mut players = shared.players.lock().unwrap();
        let inventory = &players.ui.info[&player].inventory;
        for (slot, name) in [(5, "iron_helmet"), (6, "iron_chestplate"), (8, "iron_boots"), (40, "iron_sword")] {
            assert_eq!(crate::blocks::item_name(inventory[slot].unwrap().item), Some(name));
        }
        assert!(inventory[7].is_none() && inventory[36].is_none());
        let orders = players.take_orders(&player);
        assert!(orders.iter().any(|order| matches!(order, Order::Message { component } if *component == text_component("Race presentation checked"))));
        assert!(orders.iter().any(|order| matches!(order, Order::Packet(packet)
            if packet.windows(b"#55ff55".len()).any(|value| value == b"#55ff55")
                && packet.windows(b"Race ready".len()).any(|value| value == b"Race ready"))));
    }
    let (events, _) = world.lock().unwrap().events_since(from, 0);
    assert!(matches!(events.as_slice(), [WorldEvent::Sound(sound), WorldEvent::Particle {kind, count, ..}]
        if sound.name == "entity.warden.sonic_charge" && crate::commands_admin::PARTICLES[*kind as usize] == "totem_of_undying" && *count == 3));
}

fn assert_bukkit_scoreboard(host: &mut Host, shared: &Shared) {
    use crate::{network::nbt::text_component, players::Order, scoreboard};
    use serde_json::json;
    let owner = "java-bossbar".to_string();
    let run = |host: &mut Host, phase: &str, reply: &str| {
        assert_eq!(host.run_command(bossbar_sender(1), "barprobe", &[phase.into()]), Some((true, String::new())));
        let mut players = shared.players.lock().unwrap();
        assert!(players.take_orders(&[1;16]).iter().any(|order| {
            matches!(order, Order::Message { component } if *component == text_component(reply))
        }), "{phase}");
        players.take_orders(&[2;16])
    };

    assert!(!run(host, "board-main", "Board main").is_empty());
    {
        let players = shared.players.lock().unwrap();
        for viewer in [[1;16], [2;16]] {
            let ui = &players.ui.owned[&(owner.clone(), viewer)];
            let objective = ui.objectives.values().next().unwrap();
            assert_eq!(ui.objectives.len(), 1);
            assert_eq!(scoreboard::plain_text(&objective.display), "MainTitle");
            assert_eq!(ui.teams.values().next().unwrap().nametag, 1);
            assert_eq!(players.ui.slots[&(viewer, 2)].1, objective.name);
            let fixed = scoreboard::Score { value: -7, locked: true, display: None,
                format: Some(scoreboard::NumberFormat::Fixed(json!({"text":"FixedName"}))) };
            let score = scoreboard::score_packet("Первый", &objective.name, &fixed);
            assert!(ui.cache.values().any(|packet| *packet == score));
        }
    }

    assert!(run(host, "board-private", "Board private").is_empty());
    {
        let players = shared.players.lock().unwrap();
        let personal = &players.ui.owned[&(owner.clone(), [1;16])];
        let shared_board = &players.ui.owned[&(owner.clone(), [2;16])];
        let private_objective = personal.objectives.values().next().unwrap();
        let main_objective = shared_board.objectives.values().next().unwrap();
        assert!(personal.teams.is_empty());
        assert_eq!(shared_board.teams.len(), 1);
        assert_eq!(private_objective.name, main_objective.name);
        assert_eq!(scoreboard::plain_text(&private_objective.display), "ChangedTitle");
        assert_eq!(scoreboard::plain_text(&main_objective.display), "MainTitle");
        assert!(!players.ui.slots.contains_key(&([1;16], 2)));
        assert_eq!(players.ui.slots[&([1;16], 1)].1, private_objective.name);
        assert_eq!(players.ui.slots[&([2;16], 2)].1, main_objective.name);
        let points = scoreboard::Score { value: 6, locked: true, display: None, format: None };
        let score = scoreboard::score_packet("PersonalEntry", &private_objective.name, &points);
        assert!(personal.cache.values().any(|packet| *packet == score));
        assert!(!shared_board.cache.values().any(|packet| *packet == score));
    }

    assert!(!run(host, "board-clear", "Board cleared").is_empty());
    let players = shared.players.lock().unwrap();
    assert!(players.ui.slots.is_empty());
    for viewer in [[1;16], [2;16]] {
        let ui = &players.ui.owned[&(owner.clone(), viewer)];
        assert!(ui.teams.is_empty() && ui.objectives.is_empty() && ui.cache.is_empty());
    }
}

fn assert_bukkit_world_change(host: &mut Host, shared: &Shared) {
    use crate::{dimension::Dimension, network::nbt::text_component, players::Order};
    let mut who = view();
    host.player_changed_world(&who, Dimension::Overworld);
    assert!(shared.players.lock().unwrap().take_orders(&who.uuid).is_empty());
    for (from, to, message) in [
        (Dimension::Overworld, Dimension::Nether, "ChangedWorld world world_nether"),
        (Dimension::Nether, Dimension::Overworld, "ChangedWorld world_nether world"),
    ] {
        who.dimension = to;
        shared.players.lock().unwrap().set_dimension(&who.uuid, to);
        host.player_changed_world(&who, from);
        let mut players = shared.players.lock().unwrap();
        assert!(matches!(players.take_orders(&who.uuid).as_slice(), [Order::Message { component }]
            if *component == text_component(message)));
        assert!(players.take_orders(&[2;16]).is_empty());
    }
}

#[test]
#[ignore = "нужен переведённый fixtures/bossbar в tmp/java-test/plugins/java-bossbar"]
fn translated_bukkit_player_presentation_and_world_change() {
    let root = server_root("java-player-presentation");
    bossbar_fixture(&root);
    let shared = shared();
    java_info(&shared);
    bossbar_second_player(&shared);
    let mut host = Host::new(Arc::clone(&shared), root).unwrap();
    host.load("java-bossbar").unwrap();
    {
        let mut players = shared.players.lock().unwrap();
        players.ui.info.entry([1; 16]).or_default().ping = 42;
        players.ui.info.entry([2; 16]).or_default().ping = u32::MAX;
    }
    for (n, ping) in [(1, 42), (2, i32::MAX)] {
        assert_eq!(host.run_command(bossbar_sender(n), "barprobe", &["info".into()]), Some((true, String::new())));
        let orders = shared.players.lock().unwrap().take_orders(&[n; 16]);
        let max = shared.properties.max_players.max(0);
        assert!(matches!(orders.as_slice(), [crate::players::Order::Message { component }]
            if *component == crate::network::nbt::text_component(&format!("PlayerInfo {ping} {max} {max}"))));
    }
    assert_eq!(host.run_command(bossbar_sender(1), "barprobe", &["metadata".into()]), Some((true, String::new())));
    assert!(matches!(shared.players.lock().unwrap().take_orders(&[1;16]).as_slice(),
        [crate::players::Order::Message { component }] if *component == crate::network::nbt::text_component("Metadata checked")));
    assert_eq!(host.run_command(bossbar_sender(1), "barprobe", &["minitext".into()]), Some((true, String::new())));
    let decoded = serde_json::json!({"text":"","extra":[
        {"text":"Head ","color":"#55ff55"},{"text":"bold","color":"#55ff55","bold":true},{"text":"\n"},
        {"text":"A","color":"#ff0000"},{"text":"☃","color":"#800080"},{"text":"Z","color":"#0000ff"},{"text":"<red>"},
    ]});
    assert!(matches!(shared.players.lock().unwrap().take_orders(&[1;16]).as_slice(),
        [crate::players::Order::Message { component }] if *component == crate::scoreboard::encode_component(&decoded)));
    assert_tab_presentation(&mut host, &shared);
    assert_bukkit_scoreboard(&mut host, &shared);
    assert_bukkit_world_change(&mut host, &shared);
    assert_race_presentation(&mut host, &shared);
    assert!(host.is_loaded("java-bossbar"));
    host.unload("java-bossbar").unwrap();
}

#[test]
#[ignore = "нужен переведённый fixtures/bossbar в tmp/java-test/plugins/java-bossbar"]
fn translated_bukkit_bossbar_multiplayer_updates_and_cleanup() {
    let root = server_root("java-bossbar");
    bossbar_fixture(&root);
    assert!(install(&root, "plugins", "hello", "hello", ""));
    let shared = shared();
    java_info(&shared);
    bossbar_second_player(&shared);
    let mut host = Host::new(Arc::clone(&shared), root).unwrap();
    host.load("java-bossbar").unwrap();
    assert!(shared.players.lock().unwrap().ui.owned.is_empty(), "создание без зрителей не шлёт оформление");
    let first = bossbar_command(&mut host, &shared, 1, "add", 1);
    assert_eq!(bossbar_actions(&first), [0]);
    let first_uuid = first[0][1..17].to_vec();
    assert_eq!(&first[0][first[0].len()-7..], &[0x3f, 0x80, 0, 0, 1, 2, 0], "1.0 / blue / notched_10 / no flags");
    assert!(bossbar_other_packets(&shared, 2).is_empty());
    let bounds = bossbar_command(&mut host, &shared, 1, "bounds", 1);
    assert_eq!(bossbar_actions(&bounds), [2, 2]);
    assert_eq!(&bounds[0][18..], &0.0f32.to_be_bytes());
    assert_eq!(&bounds[1][18..], &1.0f32.to_be_bytes());
    assert!(bossbar_command(&mut host, &shared, 1, "add", 1).is_empty(), "один UUID добавляется один раз");
    let second_viewer = bossbar_command(&mut host, &shared, 2, "add", 2);
    assert_eq!(second_viewer, first, "зрители видят одну полосу с одним UUID");
    assert!(bossbar_other_packets(&shared, 1).is_empty());
    let update = bossbar_command(&mut host, &shared, 1, "update", 2);
    assert_eq!(bossbar_actions(&update), [2, 3, 4, 4]);
    assert_eq!(&update[0][18..], &0.25f32.to_be_bytes());
    assert_eq!(&update[2][18..], &[3, 2]);
    assert_eq!(&update[3][18..], &[3, 1]);
    assert_eq!(bossbar_other_packets(&shared, 2), update);
    {
        let players = shared.players.lock().unwrap();
        let bar = players.ui.owned[&("java-bossbar".into(), [1;16])].bars.values().next().unwrap();
        assert_eq!((bar.value, bar.color, bar.style), (250000, 3, 1));
        assert_eq!(crate::scoreboard::plain_text(&bar.name), "Новая");
    }
    assert!(bossbar_command(&mut host, &shared, 1, "invalid", 2).is_empty());
    assert!(bossbar_other_packets(&shared, 2).is_empty());
    assert_eq!(bossbar_actions(&bossbar_command(&mut host, &shared, 1, "hide", 2)), [1]);
    assert_eq!(bossbar_actions(&bossbar_other_packets(&shared, 2)), [1]);
    assert!(bossbar_command(&mut host, &shared, 1, "hidden", 2).is_empty());
    assert!(bossbar_other_packets(&shared, 2).is_empty());
    assert!(bossbar_command(&mut host, &shared, 2, "remove", 1).is_empty());
    let shown = bossbar_command(&mut host, &shared, 1, "show", 1);
    assert_eq!(bossbar_actions(&shown), [0]);
    assert_eq!(&shown[0][1..17], first_uuid.as_slice());
    assert_eq!(&shown[0][shown[0].len()-7..], &[0x3f, 0x40, 0, 0, 3, 1, 0], "скрытые изменения сохранились: 0.75 / green / notched_6");
    assert!(bossbar_other_packets(&shared, 2).is_empty(), "удалённый зритель не получает show");
    let other_bar = bossbar_command(&mut host, &shared, 1, "second", 1);
    assert_eq!(bossbar_actions(&other_bar), [0]);
    assert_ne!(&other_bar[0][1..17], first_uuid.as_slice(), "две Java-полосы имеют разные id");

    // Тот же локальный id другого компонента остаётся отдельным ресурсом хозяина.
    host.load("hello").unwrap();
    {
        use super::api::mcsa::api::presentation::{BossBar, Host as _};
        let ctx = host.loaded.iter_mut().find(|p| p.manifest.name == "hello").unwrap().store.data_mut();
        assert!(ctx.set_boss_bar(crate::network::login::format_uuid(&[1;16]), BossBar {
            id: "bukkit-bar-1".into(), title: "\"Чужая\"".into(), progress: 0.5, color: "red".into(), style: "progress".into(),
        }));
    }
    let foreign = bossbar_other_packets(&shared, 1);
    assert_eq!(bossbar_actions(&foreign), [0]);
    assert_ne!(&foreign[0][1..17], first_uuid.as_slice());
    host.unload("java-bossbar").unwrap();
    let removed = bossbar_other_packets(&shared, 1);
    assert_eq!(bossbar_actions(&removed), [1, 1]);
    let mut removed_ids: Vec<_> = removed.iter().map(|p| p[1..17].to_vec()).collect();
    removed_ids.sort();
    let mut own_ids = vec![first_uuid, other_bar[0][1..17].to_vec()];
    own_ids.sort();
    assert_eq!(removed_ids, own_ids);
    assert!(bossbar_other_packets(&shared, 2).is_empty());
    assert_eq!(shared.players.lock().unwrap().ui.owned[&("hello".into(), [1;16])].bars.len(), 1);
    assert!(shared.players.lock().unwrap().ui.owned.keys().all(|(name, _)| name != "java-bossbar"));
    host.unload("hello").unwrap();
    assert_eq!(bossbar_actions(&bossbar_other_packets(&shared, 1)), [1]);
    assert!(host.is_empty());
    assert!(host.command_level("barprobe").is_none());
}

#[test]
#[ignore = "нужен переведённый fixtures/bossbar в tmp/java-test/plugins/java-bossbar"]
fn translated_bukkit_bossbar_remove_all_and_disconnected_viewers() {
    let root = server_root("java-bossbar-disconnect");
    bossbar_fixture(&root);
    let shared = shared();
    java_info(&shared);
    bossbar_second_player(&shared);
    let mut host = Host::new(Arc::clone(&shared), root).unwrap();
    host.load("java-bossbar").unwrap();
    let limit = bossbar_command(&mut host, &shared, 1, "limit", 0);
    assert_eq!(bossbar_actions(&limit), [vec![0;32], vec![1;32]].concat(), "33-я полоса явно отвергнута хозяином");
    assert!(shared.players.lock().unwrap().ui.owned[&("java-bossbar".into(), [1;16])].bars.is_empty());
    assert!(bossbar_other_packets(&shared, 2).is_empty());
    assert_eq!(bossbar_actions(&bossbar_command(&mut host, &shared, 1, "add", 1)), [0]);
    assert_eq!(bossbar_actions(&bossbar_command(&mut host, &shared, 2, "add", 2)), [0]);
    assert_eq!(bossbar_actions(&bossbar_command(&mut host, &shared, 1, "remove", 1)), [1]);
    assert!(bossbar_other_packets(&shared, 2).is_empty());
    assert_eq!(bossbar_actions(&bossbar_command(&mut host, &shared, 1, "add", 2)), [0]);
    assert_eq!(bossbar_actions(&bossbar_command(&mut host, &shared, 1, "all", 0)), [1]);
    assert_eq!(bossbar_actions(&bossbar_other_packets(&shared, 2)), [1]);
    assert_eq!(bossbar_actions(&bossbar_command(&mut host, &shared, 1, "add", 1)), [0]);
    assert_eq!(bossbar_actions(&bossbar_command(&mut host, &shared, 2, "add", 2)), [0]);
    let original = shared.players.lock().unwrap().members().iter().find(|m| m.uuid == [1;16]).unwrap().clone();
    host.player_quit(&view());
    shared.players.lock().unwrap().leave(&[1;16]);
    assert_eq!(bossbar_actions(&bossbar_command(&mut host, &shared, 2, "update", 1)), [2, 3, 4, 4]);
    assert!(shared.players.lock().unwrap().ui.owned.keys().all(|(_, uuid)| *uuid != [1;16]));
    shared.players.lock().unwrap().join(original, 1);
    java_info(&shared);
    host.player_join(&view());
    assert!(bossbar_command(&mut host, &shared, 2, "update", 1).is_empty(), "переподключение не возвращает зрителя автоматически");
    assert!(bossbar_other_packets(&shared, 1).is_empty());
    assert_eq!(bossbar_actions(&bossbar_command(&mut host, &shared, 1, "add", 2)), [0]);
    host.unload("java-bossbar").unwrap();
    assert_eq!(bossbar_actions(&bossbar_other_packets(&shared, 1)), [1]);
    assert_eq!(bossbar_actions(&bossbar_other_packets(&shared, 2)), [1]);
    assert!(shared.players.lock().unwrap().ui.owned.is_empty());
}

#[test]
#[ignore = "нужен переведённый fixtures/bossbar в tmp/java-test/plugins/java-bossbar"]
fn translated_bukkit_bossbar_visibility_failure_rolls_back_all_viewers() {
    let root = server_root("java-bossbar-visibility-rollback");
    bossbar_fixture(&root);
    let shared = shared();
    java_info(&shared);
    bossbar_second_player(&shared);
    let mut host = Host::new(Arc::clone(&shared), root).unwrap();
    host.load("java-bossbar").unwrap();
    let first = bossbar_command(&mut host, &shared, 1, "add", 1);
    assert_eq!(bossbar_actions(&first), [0]);
    let bar_uuid = first[0][1..17].to_vec();
    assert_eq!(bossbar_actions(&bossbar_command(&mut host, &shared, 2, "add", 2)), [0]);
    assert_eq!(bossbar_actions(&bossbar_command(&mut host, &shared, 1, "hide", 2)), [1]);
    assert_eq!(bossbar_actions(&bossbar_other_packets(&shared, 2)), [1]);
    let capacity = bossbar_command(&mut host, &shared, 2, "fill-hidden-viewer", 2);
    assert_eq!(bossbar_actions(&capacity), vec![0;32]);
    assert!(bossbar_other_packets(&shared, 1).is_empty());
    let full_ids: std::collections::BTreeSet<_> = capacity.iter().map(|p| p[1..17].to_vec()).collect();
    assert_eq!(full_ids.len(), 32);
    assert!(!full_ids.contains(&bar_uuid));
    // Две попытки: первая не должна оставить состояние, влияющее на вторую.
    for _ in 0..2 {
        let attempted = bossbar_command(&mut host, &shared, 1, "show-rejected", 2);
        assert_eq!(bossbar_actions(&attempted), [0, 1], "после отказа второму первый получает явное удаление");
        assert!(attempted.iter().all(|p| &p[1..17] == bar_uuid.as_slice()));
        assert!(bossbar_other_packets(&shared, 2).is_empty(), "у заполненного зрителя не меняются его 32 полосы");
        let players = shared.players.lock().unwrap();
        assert!(players.ui.owned[&("java-bossbar".into(), [1;16])].bars.is_empty(), "нет призрака у первого при Java visible=false");
        let actual_ids: std::collections::BTreeSet<_> = players.ui.owned[&("java-bossbar".into(), [2;16])].bars.values().map(|bar| bar.uuid.to_vec()).collect();
        assert_eq!(actual_ids, full_ids, "отказ не удаляет прежние полосы второго");
    }
    let freed = bossbar_command(&mut host, &shared, 2, "free-hidden-viewer", 2);
    assert_eq!(bossbar_actions(&freed), vec![1;32]);
    let freed_ids: std::collections::BTreeSet<_> = freed.iter().map(|p| p[1..17].to_vec()).collect();
    assert_eq!(freed_ids, full_ids);
    assert!(bossbar_other_packets(&shared, 1).is_empty());
    let shown = bossbar_command(&mut host, &shared, 1, "show", 2);
    assert_eq!(bossbar_actions(&shown), [0]);
    assert_eq!(&shown[0][1..17], bar_uuid.as_slice());
    assert_eq!(bossbar_other_packets(&shared, 2), shown, "после освобождения места повторный показ успешен обоим");
    for uuid in [[1;16], [2;16]] {
        assert_eq!(shared.players.lock().unwrap().ui.owned[&("java-bossbar".into(), uuid)].bars.len(), 1);
    }
    host.unload("java-bossbar").unwrap();
    for n in [1, 2] {
        let cleanup = bossbar_other_packets(&shared, n);
        assert_eq!(bossbar_actions(&cleanup), [1]);
        assert_eq!(&cleanup[0][1..17], bar_uuid.as_slice());
    }
    assert!(shared.players.lock().unwrap().ui.owned.is_empty());
}
