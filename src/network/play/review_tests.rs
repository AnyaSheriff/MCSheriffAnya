use super::*;
fn shared() -> Shared {
    Shared::new(World::in_memory(),World::in_memory(),Default::default(),Default::default(),Default::default(),
        crate::ops::Ops::open(std::path::Path::new("tmp/review-test-ops.json")),crate::skins::Settings::default())
}
fn player() -> PlayerState { PlayerState::new([27;16],"Review".into(),None,None,0x7f,0,(0.5,70.0,2.5)) }
fn item(name: &str) -> Stack { Stack::new(blocks::item_named(name).unwrap(),1) }
#[test]
fn creative_bundle_moves_keep_contents() {
    let (bundle,_) = crate::bundle::insert(item("bundle"),item("stone"));
    let (_,components) = crate::stackdata::encode_components(&bundle.extras());
    let mut payload = Vec::new();
    for value in [1,bundle.item,1,0,50,(components.len()-1) as i32] { payload.extend(encode_varint(value)); }
    payload.extend_from_slice(&components[1..]);
    assert_eq!(read_untrusted_slot(&mut Cursor::new(&payload)),Some(Some(bundle)));
    payload.pop(); assert!(read_untrusted_slot(&mut Cursor::new(&payload)).is_none());
}
#[test]
fn bundle_selection_persists_in_chest_and_ender_storage() {
    for ender in [false,true] {
        let shared = shared(); let mut p = player();
        let (bundle,_) = crate::bundle::insert(item("bundle"),item("stone"));
        let (bundle,_) = crate::bundle::insert(bundle,item("dirt"));
        let mut shown = vec![None;27]; shown[0] = Some(bundle);
        if ender { p.ender = shown.clone(); }
        else {
            let mut world = shared.world_in(p.dimension).lock().unwrap();
            world.set_block(0,70,0,blocks::state_by_name("chest").unwrap());
            world.with_container(0,70,0,|c| c.slots.clone_from(&shown));
        }
        p.window = Some(OpenWindow { id:1,halves:vec![(0,70,0)],cart:None,shown,lectern:false,crafter:false,
            kind:Some(if ender { containers::Kind::EnderChest } else { containers::Kind::Chest }),props:Vec::new(),merchant:None,station:None,plugin_chest:None,ender });
        select_bundle(&shared,&mut p,0,1);
        let mut payload = vec![1,0]; payload.extend_from_slice(&0i16.to_be_bytes()); payload.extend([1,0]);
        click_window(&shared,&mut p,&payload,&mut Reaction::default());
        assert_eq!(p.inventory.cursor(),Some(item("stone")));
    }
}
#[test]
fn cartography_cursor_gets_final_map_components() {
    let shared = shared(); let mut p = player();
    shared.world_in(p.dimension).lock().unwrap().set_block(0,70,0,blocks::state_by_name("cartography_table").unwrap());
    let id = shared.maps.lock().unwrap().create(p.dimension,0,0,0);
    let input = item("filled_map").with_extras(crate::stackdata::Data {map_id:Some(id),..Default::default()});
    p.window = Some(OpenWindow { id:1,halves:Vec::new(),cart:None,shown:vec![Some(input),Some(item("paper")),Some(input)],lectern:false,crafter:false,
        kind:None,props:Vec::new(),merchant:None,plugin_chest: None, ender: false,station:Some(station::StationWindow {kind:crate::enchanting::Station::Cartography,pos:(0,70,0),props:Vec::new(),name:None,cost:0,repair_items:0,selected:-1,input_item:None,sound_tick:None}) });
    let mut payload = vec![1,0]; payload.extend_from_slice(&2i16.to_be_bytes()); payload.extend([0,0,0]);
    // HashedSlot: предмет и число верны, но map_id клиент ещё не знает.
    payload.push(1); payload.extend(encode_varint(input.item)); payload.extend([1,0,0]);
    let mut reaction = Reaction::default(); station::click(&shared,&mut p,&payload,&mut reaction);
    assert_eq!(p.inventory.cursor().unwrap().extras().map_id,Some(id+1));
    assert_eq!(reaction.cursor,Some(p.inventory.cursor()));
}

#[test]
fn plugin_chest_drag_conserves_items_and_skips_foreign_stacks() {
    let mut p = player();
    let mut slots = vec![None; 9];
    p.inventory.set_cursor(Some(item("stone").with_count(8)));
    assert!(prepare_plugin_drag(&mut p, 1, -999, 0, &slots).is_none());
    for slot in [0, 1, 2, 3] { assert!(prepare_plugin_drag(&mut p, 1, slot, 1, &slots).is_none()); }
    assert!(slots.iter().all(Option::is_none));
    let (kind, selected) = prepare_plugin_drag(&mut p, 1, -999, 2, &slots).unwrap();
    // Обработчик мог изменить ячейку до применения транзакции.
    slots[1] = Some(item("dirt"));
    distribute_plugin_drag(&mut p.inventory, &mut slots, kind, &selected);
    assert_eq!(slots[1], Some(item("dirt")));
    assert_eq!(slots[0], Some(item("stone").with_count(2)));
    assert_eq!(slots.iter().flatten().filter(|s| s.item == item("stone").item).map(|s| s.count).sum::<i32>() + p.inventory.cursor().map_or(0, |s| s.count), 8);
}

#[test]
fn plugin_chest_without_live_handler_cannot_give_out_items_and_close_keeps_cursor() {
    let shared = shared(); let mut p = player();
    let chest = std::sync::Arc::new(std::sync::Mutex::new(crate::players::PluginChest { owner: "absent".into(), handle: 1, title: "{}".into(), slots: vec![Some(item("diamond")); 9], active: true }));
    p.window = Some(OpenWindow { id: 1, halves: Vec::new(), cart: None, shown: vec![Some(item("diamond")); 9], lectern: false, crafter: false, kind: None, props: Vec::new(), merchant: None, station: None, ender: false, plugin_chest: Some(chest.clone()) });
    let mut payload = vec![1, 0]; payload.extend(0i16.to_be_bytes()); payload.extend([0, 0, 0, 0]);
    let mut reaction = Reaction::default(); click_window(&shared, &mut p, &payload, &mut reaction);
    assert!(p.inventory.cursor().is_none()); assert_eq!(chest.lock().unwrap().slots[0], Some(item("diamond")));
    assert!(reaction.window_content && reaction.window_slots.is_empty());
    p.inventory.set_cursor(Some(item("stone").with_count(3)));
    close_own_window(&shared, &mut p);
    assert!(p.window.is_none() && p.inventory.cursor().is_none());
    assert_eq!(p.inventory.slots().iter().flatten().filter(|s| s.item == item("stone").item).map(|s| s.count).sum::<i32>(), 3);
}
