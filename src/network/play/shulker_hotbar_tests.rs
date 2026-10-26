//! Регрессии переноса хранилище→панель через настоящий обработчик пакета.
use super::*;

fn stack(name: &str, count: i32) -> Stack { Stack::new(blocks::item_named(name).unwrap(), count) }
fn setup(kind: &str) -> (Shared, PlayerState) {
    let shared = Shared::new(World::in_memory(), World::in_memory(), Default::default(), Default::default(), Default::default(),
        crate::ops::Ops::open(Path::new("tmp/shulker-hotbar-ops.json")), crate::skins::Settings::default());
    let mut player = PlayerState::new([217;16], "ShulkerHotbar".into(), None, None, 0x7f, 217, (0.5,64.0,0.5));
    player.game_mode = GAME_MODE_SURVIVAL;
    player.awaiting_teleport = false;
    {
        let mut world = shared.world_in(player.dimension).lock().unwrap();
        world.ensure(0,0);
        world.set_block(2,64,0,blocks::state_by_name(kind).unwrap());
        world.with_container(2,64,0,|c| c.slots[0] = Some(stack("stone",12)));
    }
    reopen(&shared, &mut player);
    (shared, player)
}
fn reopen(shared: &Shared, p: &mut PlayerState) {
    let w = shared.world_in(p.dimension).lock().unwrap();
    let shown = containers::window_slots(&w, &[(2,64,0)]);
    p.window = Some(OpenWindow { id: 7, halves: vec![(2,64,0)], cart: None, shown, lectern: false, crafter: false,
        kind: containers::Kind::of(w.get_block(2,64,0)), props: Vec::new(), merchant: None, station: None, ender: false });
}
fn click(shared: &Shared, p: &mut PlayerState, slot: i16, button: u8, mode: i32, state: i32) -> Reaction {
    let mut body = encode_varint(CLICK_CONTAINER);
    body.extend(encode_varint(p.window.as_ref().unwrap().id));
    body.extend(encode_varint(state)); body.extend(slot.to_be_bytes()); body.push(button);
    body.extend(encode_varint(mode)); body.extend([0,0]);
    handle_client_packet(&PacketInfo { id: CLICK_CONTAINER, length: body.len() as i32, body, teleport: None,
        punch: false, foreign_inventory: false, legacy_inventory: None }, p, shared)
}
fn close(shared: &Shared, p: &mut PlayerState) {
    let mut body = encode_varint(CLOSE_CONTAINER);
    body.extend(encode_varint(p.window.as_ref().unwrap().id));
    handle_client_packet(&PacketInfo { id: CLOSE_CONTAINER, length: body.len() as i32, body, teleport: None,
        punch: false, foreign_inventory: false, legacy_inventory: None }, p, shared);
}
fn contents(shared: &Shared, p: &PlayerState) -> Vec<Option<Stack>> {
    containers::window_slots(&shared.world_in(p.dimension).lock().unwrap(), &[(2,64,0)])
}

#[test]
fn container_pickup_shift_and_all_number_keys_keep_authoritative_hotbar() {
    for kind in ["shulker_box", "chest", "barrel"] {
        for reopened in [false,true] {
            for mode in [0,1,2] {
                for button in 0..9 {
                    for occupied in [false,true] {
                        let (shared, mut p) = setup(kind);
                        if reopened { close(&shared,&mut p); reopen(&shared,&mut p); }
                        let destination = if mode == 1 { 44 } else { 36 + i32::from(button) };
                        if occupied && mode == 2 { p.inventory.set(destination,Some(stack("dirt",5))); }
                        let response = click(&shared,&mut p,0,if mode == 2 { button } else { 0 },mode,0);
                        if mode == 0 {
                            assert_eq!(p.inventory.cursor(),Some(stack("stone",12)));
                            click(&shared,&mut p,54 + i16::from(button),0,0,0);
                        } else { assert!(response.window_slots.iter().any(|(i,s)| *i == (destination + 18) as usize && *s == Some(stack("stone",12)))); }
                        assert_eq!(p.inventory.slot(destination),Some(stack("stone",12)),"{kind},mode={mode},button={button}");
                        assert_eq!(contents(&shared,&p)[0],if occupied && mode == 2 { Some(stack("dirt",5)) } else { None });
                        assert!(p.inventory.cursor().is_none());
                        close(&shared,&mut p); reopen(&shared,&mut p);
                        assert_eq!(p.window.as_ref().unwrap().shown,contents(&shared,&p));
                        assert_eq!(p.inventory.slot(destination),Some(stack("stone",12)));
                        assert!(p.needs_save());
                    }
                }
            }
        }
    }
}

#[test]
fn container_drag_places_real_stacks_in_all_hotbar_edges() {
    for kind in ["shulker_box", "chest", "barrel"] {
        for offset in [0,4] {
            let (shared,mut p) = setup(kind);
            click(&shared,&mut p,0,0,0,0);
            click(&shared,&mut p,-999,offset,5,0);
            for slot in [54,55,62] { click(&shared,&mut p,slot,offset+1,5,0); }
            click(&shared,&mut p,-999,offset+2,5,0);
            for slot in [36,37,44] { assert_eq!(p.inventory.slot(slot),Some(stack("stone",if offset == 0 { 4 } else { 1 })),"{kind}/{offset}"); }
            assert_eq!(p.inventory.cursor(),if offset == 0 { None } else { Some(stack("stone",9)) });
            assert!(contents(&shared,&p).iter().all(Option::is_none));
        }
    }
}

#[test]
fn container_drag_rejects_nested_shulkers_and_resets_on_other_clicks_and_close() {
    let (shared,mut p) = setup("shulker_box");
    let nested = containers::with_contents(stack("red_shulker_box",1),&[Some(stack("diamond",3))]);
    p.inventory.set_cursor(Some(nested));
    click(&shared,&mut p,-999,0,5,0);
    click(&shared,&mut p,1,1,5,0);
    click(&shared,&mut p,54,1,5,0);
    click(&shared,&mut p,-999,2,5,0);
    assert!(contents(&shared,&p)[1].is_none());
    assert_eq!(p.inventory.slot(36),Some(nested));
    p.inventory.set_cursor(Some(stack("stone",8)));
    click(&shared,&mut p,-999,0,5,0); click(&shared,&mut p,55,1,5,0);
    click(&shared,&mut p,56,1,0,0);
    click(&shared,&mut p,-999,2,5,0);
    assert!(p.inventory.slot(37).is_none());
    click(&shared,&mut p,-999,0,5,0);
    close(&shared,&mut p); reopen(&shared,&mut p);
    assert!(p.crafting_drag.is_none());
    click(&shared,&mut p,-999,2,5,0);
    assert!(p.inventory.cursor().is_none());
}

#[test]
fn stale_container_state_executes_then_requests_full_authoritative_contents() {
    let (shared,mut p) = setup("shulker_box");
    p.state_id = 10;
    let response = click(&shared,&mut p,0,0,0,-1);
    assert_eq!(p.inventory.cursor(),Some(stack("stone",12)));
    assert!(response.inventory);
    assert!(contents(&shared,&p)[0].is_none());
}

#[test]
fn offhand_swap_is_saved_and_synchronized_when_the_container_closes() {
    let (shared,mut p) = setup("shulker_box");
    p.inventory.set(inventory::OFFHAND,Some(stack("dirt",5)));
    let response = click(&shared,&mut p,0,40,2,0);
    assert_eq!(p.inventory.slot(inventory::OFFHAND),Some(stack("stone",12)));
    assert_eq!(contents(&shared,&p)[0],Some(stack("dirt",5)));
    assert!(response.slots.contains(&inventory::OFFHAND) || p.inventory_after_window);
}

#[test]
fn shulker_item_components_survive_container_hotbar_and_player_disk_roundtrip() {
    let (shared,mut p) = setup("chest");
    let potion = stack("potion",1).with_extras(crate::stackdata::Data { potion: Some(13), ..Default::default() });
    let named = stack("red_shulker_box",1).with_extras(crate::stackdata::Data { name: Some("Кладовая".into()), ..Default::default() });
    let nested = containers::with_contents(named,&[Some(stack("diamond",3)),None,Some(potion)]);
    shared.world_in(p.dimension).lock().unwrap().with_container(2,64,0,|c| c.slots[0] = Some(nested));
    reopen(&shared,&mut p);
    click(&shared,&mut p,0,0,0,0);
    click(&shared,&mut p,54,0,0,0);
    assert_eq!(p.inventory.slot(36),Some(nested));
    let directory = std::env::temp_dir().join(format!("mcsa-shulker-hotbar-{}",std::process::id()));
    std::fs::create_dir_all(&directory).unwrap();
    p.save_with_catalog(&directory,shared.catalog());
    let restored = playerdata::load(&directory,&p.uuid).unwrap();
    assert_eq!(restored.inventory.slot(36),Some(nested));
    assert_eq!(containers::contents_of(&restored.inventory.slot(36).unwrap())[2],Some(potion));
    assert_eq!(restored.inventory.slot(36).unwrap().extras().name.as_deref(),Some("Кладовая"));
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn collect_and_drag_recheck_slot_limits_and_keep_every_item() {
    for kind in ["shulker_box", "chest", "barrel"] {
        let (shared,mut p) = setup(kind);
        shared.world_in(p.dimension).lock().unwrap().with_container(2,64,0,|c| c.slots[1] = Some(stack("stone",7)));
        p.inventory.set(44,Some(stack("stone",2)));
        click(&shared,&mut p,0,0,0,0);
        click(&shared,&mut p,54,0,6,0);
        click(&shared,&mut p,54,0,0,0);
        assert_eq!(p.inventory.slot(36),Some(stack("stone",21)));
        assert!(p.inventory.slot(44).is_none());
        assert!(contents(&shared,&p).iter().all(Option::is_none));
        p.inventory.set_cursor(Some(stack("stone",12)));
        p.inventory.set(37,Some(stack("stone",63)));
        click(&shared,&mut p,-999,0,5,0);
        for slot in [55,56,62] { click(&shared,&mut p,slot,1,5,0); }
        click(&shared,&mut p,-999,2,5,0);
        assert_eq!(p.inventory.slot(37),Some(stack("stone",64)));
        assert_eq!(p.inventory.slot(38),Some(stack("stone",4)));
        assert_eq!(p.inventory.slot(44),Some(stack("stone",4)));
        assert_eq!(p.inventory.cursor(),Some(stack("stone",3)));
    }
}

#[test]
fn placed_shulker_and_player_inventory_survive_actual_file_reload_after_transfer() {
    let catalog = crate::gameplay_catalog::Catalog::new(crate::gameplay_catalog::Profile::Preview26_3);
    let directory = std::env::temp_dir().join(format!("mcsa-shulker-world-{}",std::process::id()));
    let (shared,mut p) = setup("shulker_box");
    {
        let mut world = World::open_with_catalog(directory.join("world"),Some(1),crate::config::server_properties::WorldKind::Flat,catalog).unwrap();
        world.ensure(0,0);
        world.set_block(2,64,0,blocks::state_by_name("shulker_box").unwrap());
        world.with_container(2,64,0,|c| {
            c.slots[0] = Some(stack("stone",12));
            c.slots[26] = Some(stack("diamond",7));
        });
        *shared.world_in(p.dimension).lock().unwrap() = world;
    }
    reopen(&shared,&mut p);
    let players = directory.join("players");
    std::fs::create_dir_all(&players).unwrap();
    p.save_with_catalog(&players,catalog);
    assert!(!p.needs_save());
    click(&shared,&mut p,0,0,1,0);
    assert!(p.needs_save());
    p.save_if_needed(&players,catalog);
    let saved = playerdata::load_checked_for_catalog(&players,&p.uuid,catalog).unwrap().unwrap();
    assert_eq!(saved.inventory.slot(44),Some(stack("stone",12)));
    shared.world_in(p.dimension).lock().unwrap().save_all();
    *shared.world_in(p.dimension).lock().unwrap() = World::in_memory();
    let mut restored = World::open_with_catalog(directory.join("world"),Some(1),crate::config::server_properties::WorldKind::Flat,catalog).unwrap();
    restored.ensure(0,0);
    let contents = &restored.container(2,64,0).unwrap().slots;
    assert!(contents[0].is_none());
    assert_eq!(contents[26],Some(stack("diamond",7)));
    drop(restored);
    drop(shared);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn matching_item_count_prediction_cannot_hide_shulker_components() {
    let (shared,mut p) = setup("chest");
    let nested = containers::with_contents(stack("red_shulker_box",1),&[Some(stack("diamond",3))]);
    p.inventory.set_cursor(Some(nested));
    // Клиент предсказывает тот же предмет и число, но без содержимого.
    // Без досылки полного Slot получилось бы пустое изображение шалкера.
    let mut payload = vec![7,0,0,54,0,0,1,0,54,1];
    payload.extend(encode_varint(nested.item)); payload.extend([1,0,0,0]);
    let mut reaction = Reaction::default();
    click_window(&shared,&mut p,&payload,&mut reaction);
    assert_eq!(p.inventory.slot(36),Some(nested));
    assert!(reaction.window_slots.contains(&(54,Some(nested))));
    assert!(p.inventory.cursor().is_none());
    let mut payload = vec![7,0,0,54,0,0,1,0,54,0,1];
    payload.extend(encode_varint(nested.item)); payload.extend([1,0,0]);
    let mut reaction = Reaction::default();
    click_window(&shared,&mut p,&payload,&mut reaction);
    assert!(p.inventory.slot(36).is_none());
    assert_eq!(p.inventory.cursor(),Some(nested));
    assert_eq!(reaction.cursor,Some(Some(nested)));
}

#[test]
fn emptying_a_bundle_still_sends_the_slot_when_its_last_component_disappears() {
    let (shared,mut p) = setup("chest");
    let empty = stack("bundle",1);
    let filled = crate::bundle::insert(empty,stack("stone",24)).0;
    p.inventory.set(36,Some(filled));
    let reaction = click(&shared,&mut p,54,1,0,0);
    assert_eq!(p.inventory.slot(36),Some(empty));
    assert_eq!(p.inventory.cursor(),Some(stack("stone",24)));
    assert!(reaction.window_slots.contains(&(54,Some(empty))));
}

#[test]
fn quick_move_burst_merges_into_hotbar_then_main_without_losing_contents() {
    for kind in ["shulker_box", "chest", "barrel"] {
        let (shared,mut p) = setup(kind);
        shared.world_in(p.dimension).lock().unwrap().with_container(2,64,0,|c| {
            c.slots.fill(Some(stack("stone",64)));
        });
        reopen(&shared,&mut p);
        // Все щелчки приходят с одним state ID, без ожидания сетевого ответа.
        for source in 0..27 { click(&shared,&mut p,source,0,1,0); }
        assert!(contents(&shared,&p).iter().all(Option::is_none));
        for own in 18..=44 { assert_eq!(p.inventory.slot(own),Some(stack("stone",64)),"{kind}/{own}"); }
        for own in 9..18 { assert!(p.inventory.slot(own).is_none()); }
        assert!(p.inventory.cursor().is_none());
        close(&shared,&mut p); reopen(&shared,&mut p);
        assert!(p.window.as_ref().unwrap().shown.iter().all(Option::is_none));
    }
}

#[test]
fn quick_move_full_inventory_and_partial_merge_leave_the_exact_remainder() {
    for kind in ["shulker_box", "chest", "barrel"] {
        let (shared,mut p) = setup(kind);
        for own in 9..=44 { p.inventory.set(own,Some(stack("dirt",64))); }
        click(&shared,&mut p,0,0,1,0);
        assert_eq!(contents(&shared,&p)[0],Some(stack("stone",12)));
        p.inventory.set(44,Some(stack("stone",60)));
        let response = click(&shared,&mut p,0,1,1,-1);
        assert_eq!(p.inventory.slot(44),Some(stack("stone",64)));
        assert_eq!(contents(&shared,&p)[0],Some(stack("stone",8)));
        assert!(response.inventory);
        assert!(p.inventory.cursor().is_none());
    }
}

#[test]
fn quick_move_preserves_shulker_item_components_even_with_matching_prediction() {
    for kind in ["chest", "barrel"] {
        let (shared,mut p) = setup(kind);
        let nested = containers::with_contents(stack("red_shulker_box",1),&[None,Some(stack("diamond",3))]);
        shared.world_in(p.dimension).lock().unwrap().with_container(2,64,0,|c| c.slots[26] = Some(nested));
        reopen(&shared,&mut p);
        // Предсказание совпадает по ID и числу, но не доказывает компоненты.
        let mut payload = vec![7,0,0,26,0,1,2,0,26,0,0,62,1];
        payload.extend(encode_varint(nested.item)); payload.extend([1,0,0,0]);
        let mut response = Reaction::default();
        click_window(&shared,&mut p,&payload,&mut response);
        assert_eq!(p.inventory.slot(44),Some(nested));
        assert!(contents(&shared,&p)[26].is_none());
        assert!(response.window_slots.contains(&(62,Some(nested))));
        assert!(p.inventory.cursor().is_none());
    }
}
