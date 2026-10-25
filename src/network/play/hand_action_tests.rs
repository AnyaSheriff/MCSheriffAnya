//! Смена рук через Player Action: оба слота и компоненты сохраняются.
use super::*;

fn setup() -> (Shared, PlayerState) {
    let shared=Shared::new(World::in_memory(),World::in_memory(),Default::default(),Default::default(),Default::default(),
        crate::ops::Ops::open(Path::new("tmp/hand-action-ops.json")),crate::skins::Settings::default());
    let mut player=PlayerState::new([218;16],"HandAction".into(),None,None,0x7f,218,(1000.5,64.,1000.5));
    player.game_mode=GAME_MODE_SURVIVAL;
    player.inventory.select(4);
    (shared,player)
}
fn named(name:&str,count:i32,label:&str)->Stack {
    Stack::new(blocks::item_named(name).unwrap(),count).with_extras(crate::stackdata::Data { name:Some(label.into()),..Default::default() })
}
fn action(payload:&[u8],p:&mut PlayerState,shared:&Shared)->Reaction {
    let mut body=encode_varint(PLAYER_ACTION);body.extend_from_slice(payload);
    handle_client_packet(&PacketInfo {id:PLAYER_ACTION,length:body.len() as i32,body,teleport:None,punch: false, foreign_inventory:false,legacy_inventory:None},p,shared)
}
fn payload()->Vec<u8>{let mut bytes=encode_varint(6);push_packed_position(&mut bytes,0,0,0);bytes.push(0);bytes.extend(encode_varint(1));bytes}

#[test]
fn hand_action_swaps_selected_slot_and_preserves_components_without_drops() {
    let (shared,mut p)=setup();let slot=p.inventory.selected_slot();
    let main=named("iron_ingot",3,"main");let off=named("gold_ingot",5,"off");
    p.inventory.set(slot,Some(main));p.inventory.set(inventory::OFFHAND,Some(off));
    p.inventory.set(inventory::FIRST_HOTBAR,Some(named("stone",7,"untouched")));
    let before=p.inventory.slots().to_vec();
    for _ in 0..10 {
        let result=action(&payload(),&mut p,&shared);
        assert_eq!(p.inventory.slot(slot),Some(off));assert_eq!(p.inventory.slot(inventory::OFFHAND),Some(main));
        assert_eq!(result.slots,vec![slot,inventory::OFFHAND]);assert!(result.acknowledge.is_none());
        assert!(shared.items_in(p.dimension).lock().unwrap().items().is_empty());
        action(&payload(),&mut p,&shared);
        assert_eq!(p.inventory.slots(),before);
    }
}
#[test]
fn hand_action_preserves_empty_hands_in_all_playing_modes() {
    for mode in [GAME_MODE_SURVIVAL,GAME_MODE_CREATIVE,GAME_MODE_ADVENTURE] {
        for (main,off) in [(None,None),(Some(named("iron_ingot",3,"main")),None),(None,Some(named("gold_ingot",5,"off")))] {
            let (shared,mut p)=setup();p.game_mode=mode;let slot=p.inventory.selected_slot();
            p.inventory.set(slot,main);p.inventory.set(inventory::OFFHAND,off);
            action(&payload(),&mut p,&shared);
            assert_eq!(p.inventory.slot(slot),off);assert_eq!(p.inventory.slot(inventory::OFFHAND),main);
        }
    }
}
#[test]
fn hand_action_rejects_incomplete_extra_spectator_and_dead_without_mutation() {
    let bytes=payload();let mut candidates:Vec<Vec<u8>>=(0..bytes.len()).map(|end|bytes[..end].to_vec()).collect();
    let mut extra=bytes.clone();extra.push(0);candidates.push(extra);
    for candidate in candidates {
        let (shared,mut p)=setup();let slot=p.inventory.selected_slot();p.inventory.set(slot,Some(named("stone",5,"keep")));
        let before=p.inventory.slots().to_vec();action(&candidate,&mut p,&shared);assert_eq!(p.inventory.slots(),before);
    }
    for dead in [false,true] {
        let (shared,mut p)=setup();p.vit.dead=dead;if !dead{p.game_mode=GAME_MODE_SPECTATOR;}
        let slot=p.inventory.selected_slot();p.inventory.set(slot,Some(named("stone",5,"keep")));
        let before=p.inventory.slots().to_vec();let result=action(&bytes,&mut p,&shared);
        assert_eq!(p.inventory.slots(),before);assert!(result.slots.is_empty());
    }
}
#[test]
fn hand_action_cancels_use_without_releasing_a_projectile() {
    for hand in [0,1] {
        let (shared,mut p)=setup();let slot=p.inventory.selected_slot();
        let used_slot=if hand==0 {slot} else {inventory::OFFHAND};
        p.inventory.set(used_slot,Some(named("bow",1,"bow")));p.inventory.set(9,Some(named("arrow",4,"arrows")));
        p.net.using=Some(vitals::Using {hand,since:0,kind:vitals::UseKind::Bow});
        action(&payload(),&mut p,&shared);
        assert!(p.net.using.is_none());assert!(p.net.eating.is_none());
        assert_eq!(p.inventory.slot(9).unwrap().count,4);
        let destination=if hand==0 {inventory::OFFHAND} else {slot};
        assert_eq!(p.inventory.slot(destination).unwrap().item,blocks::item_named("bow").unwrap());
    }
}

#[test]
fn hand_action_cancels_pending_food_without_spending_it() {
    let (shared,mut p)=setup();let slot=p.inventory.selected_slot();
    let food=named("apple",3,"food");p.inventory.set(slot,Some(food));
    p.net.eating=Some(vitals::Eating {slot,stack:food,ticks_left:1,nutrition:4,saturation:2.4,effects:vec![],drink:None});
    action(&payload(),&mut p,&shared);
    assert!(p.net.eating.is_none());assert_eq!(p.inventory.slot(slot),None);
    assert_eq!(p.inventory.slot(inventory::OFFHAND),Some(food));
}
