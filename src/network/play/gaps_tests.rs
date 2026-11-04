// Регрессии игровых пробелов: настоящие обработчики без сети и серверов.
use super::*;
fn shared() -> Shared {
    Shared::new(World::in_memory(),World::in_memory(),Default::default(),Default::default(),Default::default(),
        crate::ops::Ops::open(std::path::Path::new("tmp/gaps-no-ops.json")),crate::skins::Settings::default())
}
fn stack(name: &str) -> Stack { Stack::new(blocks::item_named(name).unwrap(),1) }
fn player(shared: &Shared) -> PlayerState {
    let mut p=PlayerState::new([29;16],"Gaps".into(),None,None,0x7f,29,(0.5,64.0,0.5));
    vitals::change_game_mode(&mut p,GAME_MODE_SURVIVAL);p.awaiting_teleport=false;
    shared.players.lock().unwrap().join(p.member(),p.entity_id).unwrap();p
}
fn use_packet(hand:i32) -> Vec<u8> {
    let mut p=encode_varint(hand);p.extend(encode_varint(1));p.extend(0f32.to_be_bytes());p.extend(0f32.to_be_bytes());p
}

#[test]
fn all_thrown_gravity_kinds_respect_summoned_nbt() {
    for kind in ["snowball","egg","ender_pearl","experience_bottle","splash_potion","lingering_potion","llama_spit"] {
        let shared=shared();
        let dim=Dimension::Overworld;
        shared.world_in(dim).lock().unwrap().set_block_silently(0,63,0,blocks::state_by_name("stone").unwrap());
        crate::commands::run(&format!("summon {kind} 0.5 65 0.5 {{NoGravity:1b,Motion:[0.1d,0d,0d]}}"),&crate::commands::Source::Console,&shared);
        for _ in 0..5 {crate::thrown::step(&shared,dim,&mut shared.world_in(dim).lock().unwrap());}
        let thrown=shared.thrown.lock().unwrap();assert_eq!(thrown.flying.len(),1,"{kind}");
        let f=&thrown.flying[0];assert!(f.no_gravity);assert_eq!((f.y,f.vy),(65.0,0.0),"{kind}");assert!(f.vx>0.0&&f.vx<0.1);
    }
}

#[test]
fn armor_use_swaps_both_hands_and_emits_one_sound() {
    for hand in [0,1] {
        let shared=shared();let mut p=player(&shared);
        let held=if hand==1 {inventory::OFFHAND} else {p.inventory.selected_slot()};
        let old=stack("diamond_chestplate");
        let new=old.with_extras(crate::stackdata::Data {damage:10,..Default::default()});
        p.inventory.set(6,Some(old));p.inventory.set(held,Some(new));publish_player(&shared,&p);
        let start=shared.world_in(p.dimension).lock().unwrap().watch_events(77);
        let result=use_item(&use_packet(hand),&mut p,&shared).unwrap();
        assert_eq!(p.inventory.slot(6),Some(new));assert_eq!(p.inventory.slot(held),Some(old));
        assert!(result.inventory);publish_player(&shared,&p);
        let mut w=shared.world_in(p.dimension).lock().unwrap();
        let events=w.events_since(start,77).0;
        assert_eq!(events.iter().filter(|e|matches!(e,WorldEvent::Sound(s) if s.name=="minecraft:item.armor.equip_diamond")).count(),1);
    }
}

#[test]
fn binding_blocks_use_swap_and_elytra_and_turtle_can_equip() {
    let shared=shared();let mut p=player(&shared);
    let held=p.inventory.selected_slot();
    let mut data=crate::stackdata::Data::default();data.set_enchant(crate::stackdata::enchantment_number("binding_curse").unwrap(),1);
    let bound=stack("diamond_chestplate").with_extras(data);
    p.inventory.set(6,Some(bound));p.inventory.set(held,Some(stack("elytra")));
    use_item(&use_packet(0),&mut p,&shared).unwrap();assert_eq!(p.inventory.slot(6),Some(bound));
    p.inventory.set(6,None);
    use_item(&use_packet(0),&mut p,&shared).unwrap();assert_eq!(p.inventory.slot(6),Some(stack("elytra")));
    p.inventory.set(held,Some(stack("turtle_helmet")));
    use_item(&use_packet(0),&mut p,&shared).unwrap();assert_eq!(p.inventory.slot(5),Some(stack("turtle_helmet")));
}

#[test]
fn bucket_use_respects_reach_and_cauldron_uses_block_packet() {
    let shared=shared();let mut p=player(&shared);p.z=0.25;let slot=p.inventory.selected_slot();
    p.inventory.set(slot,Some(stack("water_bucket")));
    shared.world_in(p.dimension).lock().unwrap().set_block_silently(0,65,5,blocks::state_by_name("stone").unwrap());
    use_item(&use_packet(0),&mut p,&shared).unwrap();assert_eq!(p.inventory.slot(slot),Some(stack("water_bucket")));
    shared.world_in(p.dimension).lock().unwrap().set_block_silently(0,64,2,blocks::state_by_name("cauldron").unwrap());
    let mut click=encode_varint(0);push_packed_position(&mut click,0,64,2);click.extend(encode_varint(1));
    for _ in 0..3 {click.extend(0.5f32.to_be_bytes());}click.push(0);click.extend(encode_varint(1));
    place_block(&click,&mut p,&shared).unwrap();
    assert_eq!(blocks::block_at_state(shared.world_in(p.dimension).lock().unwrap().get_block(0,64,2)),Some("water_cauldron"));
    assert_eq!(p.inventory.slot(slot),Some(stack("bucket")));
}

#[test]
fn ominous_bottle_is_dropped_instead_of_becoming_a_potion() {
    let shared=shared();let dim=Dimension::Overworld;
    crate::commands::run("summon ominous_item_spawner 0.5 65 0.5 {item:{id:\"minecraft:ominous_bottle\",count:1},spawn_item_after_ticks:0L}",&crate::commands::Source::Console,&shared);
    {
        let mut world=shared.world_in(dim).lock().unwrap();let mut items=shared.items_in(dim).lock().unwrap();
        crate::ominous::step(&shared,&mut world,&mut items,dim,0,&mut crate::mob::Rng::new(5));
    }
    assert!(shared.thrown.lock().unwrap().flying.is_empty());
    assert_eq!(shared.items_in(dim).lock().unwrap().items()[0].stack,stack("ominous_bottle"));
    assert!((shared.items_in(dim).lock().unwrap().items()[0].vy-0.2).abs()<1e-9,"предмет подбрасывается на 0,2 (запись gaps2-ominous-drops)");
}

#[test]
fn fishing_retrieval_pulls_mob_and_charges_rod_durability() {
    let shared=shared();let mut p=player(&shared);let slot=p.inventory.selected_slot();p.inventory.set(slot,Some(stack("fishing_rod")));
    let mob=crate::mob::Mob::new(90,crate::mob::Kind::Cow,(0.5,64.0,4.5));
    shared.items_in(p.dimension).lock().unwrap().add_mob(mob);
    let mut bob=crate::fishing::Bobber::new(91,p.entity_id,p.dimension,(0.5,64.0,4.5),(0.0,0.0,0.0),0,0.0);bob.hooked=Some(90);
    shared.fishing.lock().unwrap().push(bob);
    assert_eq!(fishing::use_rod(&shared,&mut p,0),Some(true));
    assert_eq!(p.inventory.slot(slot).unwrap().extras().damage,5);
    assert!(shared.items_in(p.dimension).lock().unwrap().mobs()[0].vz<0.0);
    assert!(shared.fishing.lock().unwrap().is_empty());
}

#[test]
fn fishing_catch_grants_one_to_six_xp_and_one_point_of_wear() {
    let shared=shared();let mut p=player(&shared);let slot=p.inventory.selected_slot();p.inventory.set(slot,Some(stack("fishing_rod")));
    let mut bob=crate::fishing::Bobber::new(91,p.entity_id,p.dimension,(0.5,64.0,4.5),(0.0,0.0,0.0),0,0.0);bob.bite=20;bob.open=false;
    shared.fishing.lock().unwrap().push(bob);
    fishing::use_rod(&shared,&mut p,0).unwrap();
    assert_eq!(p.inventory.slot(slot).unwrap().extras().damage,1);
    let items=shared.items_in(p.dimension).lock().unwrap();assert_eq!(items.items().len(),1);
    assert!((1..=6).contains(&items.orbs().iter().map(|o|o.value).sum::<i32>()));
    assert!(items.items()[0].vz<0.0);
}

#[test]
fn commanded_nested_items_match_official_inventory_slots() {
    let commands:serde_json::Value=serde_json::from_str(include_str!("../../../tools/protocol-bridge/testdata/nested-item-commands.json")).unwrap();
    let samples:serde_json::Value=serde_json::from_str(include_str!("../../../tools/protocol-bridge/testdata/nested-item-observations.json")).unwrap();
    for name in ["bundle_basic","bundle_nested","bundle_book","container_sparse","container_empty","crossbow_arrow","crossbow_tipped","crossbow_intangible"] {
        let command=commands.as_array().unwrap().iter().find(|c|c["name"]==name).unwrap()["command"].as_str().unwrap();
        let expected=samples["old"]["cases"].as_array().unwrap().iter().find(|c|c["name"]==name).unwrap()["slot_hex"].as_str().unwrap();
        let expected=expected.as_bytes().as_chunks::<2>().0.iter().map(|s|u8::from_str_radix(std::str::from_utf8(s).unwrap(),16).unwrap()).collect::<Vec<_>>();
        let stack=crate::commands::stack_from_spec(command,1).unwrap().unwrap();let mut actual=Vec::new();push_slot(&mut actual,Some(stack));
        assert_eq!(actual,expected,"{name}: содержимое команды не должно теряться");
    }
}

#[test]
fn creative_shulker_keeps_official_contents_through_file_placement_and_repacking() {
    use mcsa_protocol::{JavaVersion, items};
    let samples:serde_json::Value=serde_json::from_str(include_str!("../../../tools/protocol-bridge/testdata/nested-item-observations.json")).unwrap();
    let hex=samples["old"]["cases"].as_array().unwrap().iter().find(|c|c["name"]=="container_sparse").unwrap()["slot_hex"].as_str().unwrap();
    let bytes=hex.as_bytes().as_chunks::<2>().0.iter().map(|s|u8::from_str_radix(std::str::from_utf8(s).unwrap(),16).unwrap()).collect::<Vec<_>>();
    let (canonical,used)=items::decode(JavaVersion::V26_1_2,&bytes,false,items::Context::default()).unwrap();assert_eq!(used,bytes.len());
    let untrusted=items::encode(JavaVersion::V26_1_2,canonical.as_ref(),true,items::Context::default()).unwrap();
    let received=read_untrusted_slot(&mut Cursor::new(&untrusted)).unwrap().unwrap();
    for n in 1..untrusted.len() {assert!(read_untrusted_slot(&mut Cursor::new(&untrusted[..n])).is_none());}
    let saved=crate::stackdata::to_bytes(&received.extras());
    let restored=stack("shulker_box").with_extras(crate::stackdata::from_bytes(&saved).unwrap());
    assert_eq!(received,restored);
    let shared=shared();let mut p=player(&shared);let held=p.inventory.selected_slot();p.inventory.set(held,Some(restored));
    shared.world_in(p.dimension).lock().unwrap().set_block_silently(0,63,2,blocks::state_by_name("stone").unwrap());
    let mut click=encode_varint(0);push_packed_position(&mut click,0,63,2);click.extend(encode_varint(1));
    for _ in 0..3 {click.extend(0.5f32.to_be_bytes());}click.push(0);click.extend(encode_varint(1));
    place_block(&click,&mut p,&shared).unwrap();assert!(p.inventory.slot(held).is_none());
    let world=shared.world_in(p.dimension).lock().unwrap();let contents=&world.container(0,64,2).unwrap().slots;
    assert_eq!(contents[0],Some(stack("stone").with_count(3)));assert!(contents[1..26].iter().all(Option::is_none));
    assert_eq!(contents[26].unwrap().extras().potion,Some(13));
    let repacked=containers::with_contents(stack("shulker_box"),contents);
    let mut actual=Vec::new();push_slot(&mut actual,Some(repacked));assert_eq!(actual,bytes);
}

#[test]
fn creative_crossbow_preserves_official_charges_through_storage() {
    use mcsa_protocol::{JavaVersion, items};
    let samples: serde_json::Value = serde_json::from_str(include_str!("../../../tools/protocol-bridge/testdata/nested-item-observations.json")).unwrap();
    for case in samples["old"]["cases"].as_array().unwrap().iter().filter(|c| c["name"].as_str().unwrap().starts_with("crossbow_")) {
        let hex = case["slot_hex"].as_str().unwrap();
        let bytes = hex.as_bytes().as_chunks::<2>().0.iter().map(|s| u8::from_str_radix(std::str::from_utf8(s).unwrap(),16).unwrap()).collect::<Vec<_>>();
        let (canonical, used) = items::decode(JavaVersion::V26_1_2, &bytes, false, items::Context::default()).unwrap();
        assert_eq!(used, bytes.len());
        let untrusted = items::encode(JavaVersion::V26_1_2, canonical.as_ref(), true, items::Context::default()).unwrap();
        let received = read_untrusted_slot(&mut Cursor::new(&untrusted)).unwrap().unwrap();
        for n in 1..untrusted.len() { assert!(read_untrusted_slot(&mut Cursor::new(&untrusted[..n])).is_none()); }
        let restored = stack("crossbow").with_extras(crate::stackdata::from_bytes(&crate::stackdata::to_bytes(&received.extras())).unwrap());
        assert_eq!(restored, received);
        let mut actual = Vec::new(); push_slot(&mut actual, Some(restored)); assert_eq!(actual, bytes);
    }
}
