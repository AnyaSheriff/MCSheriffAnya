use super::*;
use crate::{enchanting::Station, gameplay_catalog::{Catalog, Profile}};

fn window(kind: Option<Station>, shown: Vec<Option<Stack>>) -> OpenWindow {
    OpenWindow { id: 1, halves: Vec::new(), cart: None, shown, lectern: false,
        crafter: false, kind: None, props: Vec::new(), merchant: None,
        station: kind.map(|kind| station::StationWindow { kind, pos: (0, 64, 0),
            props: Vec::new(), name: None, cost: 0, repair_items: 0, selected: -1,
            input_item: None, sound_tick: None }), ender: false, plugin_chest: None }
}

fn input() -> Stack {
    Stack::new(blocks::item_named("diamond_sword").unwrap(), 1)
        .with_extras(crate::stackdata::Data { name: Some("Не потерять".into()),
            damage: 17, repair_cost: 4, enchants: vec![(0, 2)], ..Default::default() })
}

#[test]
fn snapshot_owns_inputs_but_never_workstation_previews_or_shared_contents() {
    let mut player = PlayerState::new([231; 16], "Snapshot".into(), None, None, 0, 1, (0.5, 64.0, 0.5));
    let stack = input();
    for kind in [Station::Crafting, Station::Enchanting, Station::Stonecutter,
        Station::Anvil, Station::Grindstone, Station::Cartography, Station::Smithing, Station::Loom] {
        let mut shown = vec![Some(stack); kind.slots()];
        let preview = Stack::new(blocks::item_named("diamond").unwrap(), 7);
        if kind != Station::Enchanting {
            shown[if kind == Station::Crafting { 0 } else { kind.inputs() }] = Some(preview);
        }
        player.window = Some(window(Some(kind), shown));
        let original = player.inventory.clone();
        let mut snapshot = player.inventory_snapshot().into_owned();
        assert_eq!(snapshot.take_retained(), vec![stack; kind.inputs()], "{kind:?}");
        assert_eq!(player.inventory, original);
    }
    player.window = Some(window(None, vec![Some(stack); 3]));
    assert!(player.inventory_snapshot().into_owned().take_retained().is_empty());
    player.window.as_mut().unwrap().kind = Some(containers::Kind::Beacon);
    assert_eq!(player.inventory_snapshot().into_owned().take_retained(), vec![stack]);
    player.window.as_mut().unwrap().kind = None;
    let mut trade = crate::villager::Merchant::new(1);
    trade.slots = [Some(stack), Some(stack), Some(Stack::new(blocks::item_named("diamond").unwrap(), 7))];
    player.window.as_mut().unwrap().merchant = Some(trade);
    assert_eq!(player.inventory_snapshot().into_owned().take_retained(), vec![stack; 2]);
    player.window.as_mut().unwrap().ender = true;
    assert!(player.inventory_snapshot().into_owned().take_retained().is_empty());
    player.window.as_mut().unwrap().ender = false;
    player.window.as_mut().unwrap().cart = Some(1);
    assert!(player.inventory_snapshot().into_owned().take_retained().is_empty());
}

#[test]
fn window_snapshot_survives_disk_reload_once_and_normal_close_has_no_phantom_inputs() {
    let test = "window_snapshot_survives_disk_reload_once_and_normal_close_has_no_phantom_inputs";
    if std::env::var("MCSA_WINDOW_SAVE_WORKER").as_deref() != Ok(test) {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tmp")
            .join(format!("window-save-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", &format!("network::play::window_persistence_tests::{test}"), "--nocapture"])
            .env("MCSA_WINDOW_SAVE_WORKER", test).current_dir(&root).output().unwrap();
        std::fs::remove_dir_all(root).unwrap();
        assert!(result.status.success(), "{}\n{}", String::from_utf8_lossy(&result.stdout), String::from_utf8_lossy(&result.stderr));
        return;
    }
    for profile in [Profile::Java26_1_2, Profile::Preview26_3] {
        let catalog = Catalog::new(profile);
        for full in [false, true] {
            let directory = Path::new(if full { "full" } else { "free" });
            std::fs::create_dir_all(directory).unwrap();
            let uuid = [232; 16];
            let mut player = PlayerState::new_for_catalog(uuid, "WindowSave".into(), None, None, 0, 1, (0.5, 64.0, 0.5), catalog);
            if full { for slot in 9..=44 { player.inventory.set(slot, Some(Stack::new(blocks::item_named("stone").unwrap(), 64))); } }
            let stack = input();
            player.window = Some(window(Some(Station::Anvil), vec![Some(stack), None, Some(stack)]));
            let live = player.inventory.clone();
            player.save_if_needed(directory, catalog);
            assert_eq!(player.inventory, live);
            assert!(!player.needs_save(), "снимок не должен оставаться вечно грязным");
            player.window.as_mut().unwrap().shown[0] = Some(stack.with_extras(crate::stackdata::Data { name: Some("Новое имя".into()), ..stack.extras() }));
            assert!(player.needs_save(), "изменение только входа должно сохраняться");
            player.window.as_mut().unwrap().shown[0] = Some(stack);
            let saved = playerdata::load_checked_for_catalog(directory, &uuid, catalog).unwrap().unwrap();
            let mut restored = PlayerState::new_for_catalog(uuid, "WindowSave".into(), Some(saved), None, 0, 2, (0.5, 64.0, 0.5), catalog);
            let mut pending = restored.inventory.clone();
            assert_eq!(pending.take_retained(), vec![stack], "итог наковальни не является вторым предметом");
            if full {
                assert!(!restored.inventory.restore_retained());
                restored.save_with_catalog(directory, catalog);
                restored.save_with_catalog(directory, catalog);
                let still_pending = playerdata::load_checked_for_catalog(directory, &uuid, catalog).unwrap().unwrap();
                assert_eq!(still_pending.inventory, restored.inventory);
                let mut pending = still_pending.inventory;
                assert_eq!(pending.take_retained(), vec![stack]);
                restored.inventory.set(36, None);
            }
            assert!(restored.inventory.restore_retained());
            assert!(!restored.inventory.restore_retained());
            let recovered = restored.inventory.clone();
            restored.save_with_catalog(directory, catalog);
            restored.save_with_catalog(directory, catalog);
            let saved = playerdata::load_checked_for_catalog(directory, &uuid, catalog).unwrap().unwrap();
            assert_eq!(saved.inventory, recovered);
            assert!(saved.inventory.clone().take_retained().is_empty());
            assert_eq!(saved.inventory.slots().iter().flatten().filter(|s| s.item == stack.item).copied().collect::<Vec<_>>(), vec![stack]);

            // После прежнего autosave обычное закрытие возвращает живой вход;
            // следующий файл заменяет очередь снимка, не добавляет её повторно.
            if full { player.inventory.set(36, None); }
            let shared = Shared::new(World::in_memory_with_catalog(catalog), World::in_memory_with_catalog(catalog),
                directory.to_path_buf(), directory.join("skins"), Default::default(),
                crate::ops::Ops::open(&directory.join("ops.json")), Default::default());
            close_own_window(&shared, &mut player);
            assert!(player.needs_save());
            player.save_if_needed(directory, catalog);
            assert!(!player.needs_save());
            let saved = playerdata::load_checked_for_catalog(directory, &uuid, catalog).unwrap().unwrap();
            assert!(saved.inventory.clone().take_retained().is_empty());
            assert_eq!(saved.inventory.slots().iter().flatten().filter(|s| s.item == stack.item).copied().collect::<Vec<_>>(), vec![stack]);
            let path = playerdata::path_for(directory, &uuid);
            let original = std::fs::read(&path).unwrap();
            let mut invalid = vec![
                stack.with_extras(crate::stackdata::Data { name: Some("я".repeat(70_000)), ..Default::default() }),
                stack.with_extras(crate::stackdata::Data { contents: vec![crate::stackdata::Content {
                    slot: 0, item: 999_999, count: 1, data: None }], ..Default::default() }),
            ];
            if profile == Profile::Java26_1_2 {
                invalid.push(Stack::new(crate::gameplay_catalog::WHITE_SLAB_ITEM, 1));
            }
            for invalid in invalid {
                let mut wrong = saved.clone();
                wrong.inventory.retain_window_inputs(&[Some(invalid)]);
                assert_eq!(playerdata::save_for_catalog(directory, &uuid, &wrong, catalog).unwrap_err().kind(), io::ErrorKind::InvalidData);
                assert_eq!(std::fs::read(&path).unwrap(), original, "отказ не заменяет старый файл");
            }
        }
    }
}
