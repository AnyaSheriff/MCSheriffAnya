use super::*;

#[test]
fn discarded_packets_release_tick_counts_even_after_receiving() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let waiting = Arc::new([AtomicUsize::new(0), AtomicUsize::new(0)]);
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    sender.send(TickPermit::new(&waiting, 3, true)).ok().unwrap();
    sender.send(TickPermit::new(&waiting, 3, true)).ok().unwrap();
    let received = receiver.try_recv().unwrap();
    assert_eq!(waiting[1].load(Ordering::Acquire), 2);
    drop(receiver);
    assert_eq!(waiting[1].load(Ordering::Acquire), 1);
    drop(received);
    assert_eq!(waiting[1].load(Ordering::Acquire), 0);
    assert!(sender.send(TickPermit::new(&waiting, 3, true)).is_err());
    assert_eq!(waiting[1].load(Ordering::Acquire), 0);
}

fn server(label: &str) -> Shared {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tmp").join(format!("ghost-{label}-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    Shared::new(World::in_memory(), World::in_memory(), root.clone(), root.join("skins"),
        Default::default(), crate::ops::Ops::open(&root.join("ops.json")), Default::default())
}

// Progress хранит статистику относительно cwd. Отдельный процесс держит
// и эти файлы в tmp, не меняя cwd параллельных проверок всего сервера.
fn isolated(test: &str) -> bool {
    if std::env::var("GHOST_TEST_WORKER").as_deref() == Ok(test) { return false; }
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tmp")
        .join(format!("ghost-worker-{test}-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &format!("network::play::disconnect_tests::{test}"), "--nocapture"])
        .env("GHOST_TEST_WORKER", test).current_dir(&root).output().unwrap();
    std::fs::remove_dir_all(root).unwrap();
    assert!(output.status.success(), "{}\n{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    true
}

#[tokio::test(start_paused = true)]
async fn duplicate_waits_for_save_and_restores_inventory_position_and_cursor_once() {
    if isolated("duplicate_waits_for_save_and_restores_inventory_position_and_cursor_once") { return; }
    let shared = server("replace");
    let uuid = [241; 16];
    let first = shared.sessions.acquire("SheriffAnya", uuid).await;
    let control = first.control();
    let mut player = PlayerState::new(uuid, "SheriffAnya".into(), None, None, 0x7f,
        shared.next_reader(), (12.5, 70.0, -8.25));
    let diamond = blocks::item_named("diamond").unwrap();
    player.inventory.set(36, Some(Stack::new(diamond, 7)));
    player.inventory.set_cursor(Some(Stack::new(diamond, 3)));
    {
        let mut players = shared.players.lock().unwrap();
        player.entity_id = players.join(player.member(), shared.next_entity_id()).unwrap().0.entity_id;
        players.connection(uuid, control.clone());
    }
    let (mut writer, _peer) = tokio::io::duplex(1);
    let mut out = Outgoing::duplex(&mut writer);
    out.control(control.clone());
    out.send(&[1, 2, 3]).await.unwrap();
    let replacement = shared.sessions.acquire("SheriffAnya", uuid);
    tokio::pin!(replacement);
    tokio::select! {
        biased;
        _ = &mut replacement => panic!("данные прежней сессии ещё не сохранены"),
        _ = control.cancelled() => {}
    }
    assert_eq!(out.flush().await.unwrap_err().kind(), io::ErrorKind::ConnectionAborted);
    assert_eq!(shared.players.lock().unwrap().online(), 1);
    finish_play(&shared, &mut player);
    assert_eq!(shared.players.lock().unwrap().online(), 0);
    drop(first);
    let _new_session = replacement.await;
    let saved = playerdata::load(&shared.playerdata, &uuid).unwrap();
    assert_eq!((saved.x, saved.y, saved.z), (12.5, 70.0, -8.25));
    assert_eq!(saved.inventory.cursor(), None);
    let count: i32 = saved.inventory.slots().iter().flatten()
        .filter(|stack| stack.item == diamond).map(|stack| stack.count).sum();
    assert_eq!(count, 7);
    let items = shared.items_in(player.dimension).lock().unwrap();
    let ground: i32 = items.items().iter().filter(|item| item.stack.item == diamond).map(|item| item.stack.count).sum();
    assert_eq!(ground, 3);
    assert_eq!(count + ground, 10);
    drop(items);
    std::fs::remove_dir_all(&shared.playerdata).unwrap();
}

#[tokio::test(start_paused = true)]
async fn command_kick_reaches_the_writer_and_normal_cleanup_removes_the_player() {
    if isolated("command_kick_reaches_the_writer_and_normal_cleanup_removes_the_player") { return; }
    let shared = server("kick");
    let uuid = [242; 16];
    let lease = shared.sessions.acquire("KickPlayer", uuid).await;
    let control = lease.control();
    let mut player = PlayerState::new(uuid, "KickPlayer".into(), None, None, 0x7f,
        shared.next_reader(), (4.0, 72.0, 6.0));
    {
        let mut players = shared.players.lock().unwrap();
        player.entity_id = players.join(player.member(), shared.next_entity_id()).unwrap().0.entity_id;
        players.connection(uuid, control.clone());
    }
    let (mut writer, _peer) = tokio::io::duplex(1);
    let mut out = Outgoing::duplex(&mut writer);
    out.control(control.clone());
    out.send(&[1, 2, 3]).await.unwrap();
    let (result, ()) = tokio::join!(out.flush(), async {
        tokio::time::sleep(Duration::from_millis(1)).await;
        shared.players.lock().unwrap().order(&uuid, Order::Kick { reason: "за дело".into() });
    });
    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::ConnectionAborted);
    assert_eq!(control.reason().unwrap().plain, "за дело");
    finish_play(&shared, &mut player);
    assert_eq!(shared.players.lock().unwrap().online(), 0);
    assert_eq!(playerdata::load(&shared.playerdata, &uuid).unwrap().x, 4.0);
    std::fs::remove_dir_all(&shared.playerdata).unwrap();
}

#[test]
fn remaining_personal_window_items_drop_once_on_disconnect_without_previews() {
    if isolated("remaining_personal_window_items_drop_once_on_disconnect_without_previews") { return; }
    use crate::{enchanting::Station, gameplay_catalog::{Catalog, Profile}};
    for profile in [Profile::Java26_1_2, Profile::Preview26_3] {
        for kind in [None, Some(Station::Crafting), Some(Station::Anvil)] {
            for full in [false, true] {
                let catalog = Catalog::new(profile);
                let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tmp")
                    .join(format!("ghost-drop-{profile:?}-{kind:?}-{full}-{}", std::process::id()));
                std::fs::create_dir_all(&root).unwrap();
                let shared = Shared::new(World::in_memory_with_catalog(catalog), World::in_memory_with_catalog(catalog),
                    root.clone(), root.join("skins"), Default::default(),
                    crate::ops::Ops::open(&root.join("ops.json")), Default::default());
                let uuid = [243; 16];
                let mut player = PlayerState::new_for_catalog(uuid, "DropCleanup".into(), None, None, 0, shared.next_reader(), (0.5, 70.0, 0.5), catalog);
                let sword = Stack::new(blocks::item_named("diamond_sword").unwrap(), 1).with_extras(crate::stackdata::Data {
                    name: Some("Меч на курсоре".into()), damage: 23, enchants: vec![(0, 2)], ..Default::default()
                });
                let stone = Stack::new(blocks::item_named("stone").unwrap(), 3);
                let planks = Stack::new(blocks::item_named("oak_planks").unwrap(), 2);
                let preview = Stack::new(blocks::item_named("diamond").unwrap(), 7);
                if full { for slot in 9..=44 { player.inventory.set(slot, Some(Stack::new(blocks::item_named("dirt").unwrap(), 64))); } }
                let bag = player.inventory.slots()[9..=44].to_vec();
                player.inventory.set_cursor(Some(sword));
                player.inventory.set(1, Some(planks));
                player.inventory.set(0, Some(preview));
                if let Some(kind) = kind {
                    let mut shown = vec![None; kind.slots()];
                    shown[if kind == Station::Crafting { 1 } else { 0 }] = Some(stone);
                    shown[if kind == Station::Crafting { 0 } else { kind.inputs() }] = Some(preview);
                    player.window = Some(OpenWindow { id: 1, halves: Vec::new(), cart: None, shown,
                        lectern: false, crafter: false, kind: None, props: Vec::new(), merchant: None,
                        station: Some(station::StationWindow { kind, pos: (0, 70, 0), props: Vec::new(),
                            name: None, cost: 0, repair_items: 0, selected: -1, input_item: None, sound_tick: None }),
                        ender: false, plugin_chest: None });
                } else { player.inventory.set(2, Some(stone)); }
                player.entity_id = shared.players.lock().unwrap().join(player.member(), shared.next_entity_id()).unwrap().0.entity_id;
                // Прежний снимок содержал входы в очереди, живые ячейки — только в окне.
                player.save_with_catalog(&shared.playerdata, catalog);
                finish_play(&shared, &mut player);
                let saved = playerdata::load_checked_for_catalog(&shared.playerdata, &uuid, catalog).unwrap().unwrap();
                assert_eq!(&saved.inventory.slots()[9..=44], bag.as_slice());
                assert_eq!(saved.inventory.cursor(), None);
                assert!(saved.inventory.slots()[0..=4].iter().all(Option::is_none));
                assert!(saved.inventory.retained_stacks().is_empty());
                let dimension = player.dimension;
                let ground = || shared.items_in(dimension).lock().unwrap().items().iter().map(|item| item.stack).collect::<Vec<_>>();
                let stacks = ground();
                assert_eq!(stacks.len(), 3);
                for stack in [sword, stone, planks] { assert_eq!(stacks.iter().filter(|other| **other == stack).count(), 1); }
                assert!(!stacks.contains(&preview));
                finish_play(&shared, &mut player);
                assert_eq!(ground(), stacks, "повторная уборка не создаёт новых предметов");
                std::fs::remove_dir_all(&shared.playerdata).unwrap();
            }
        }
    }
}
