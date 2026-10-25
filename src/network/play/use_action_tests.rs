//! Полный пакет отпускания/рывка должен предшествовать любому изменению.
use super::*;

fn setup() -> (Shared, PlayerState) {
    let shared = Shared::new(World::in_memory(), World::in_memory(), Default::default(), Default::default(), Default::default(),
        crate::ops::Ops::open(Path::new("tmp/use-action/ops.json")), crate::skins::Settings::default());
    shared.ticks.send_replace(100);
    let mut p = PlayerState::new([219;16], "UseAction".into(), None, None, 0, 219, (1000.5,64.,1000.5));
    p.game_mode = GAME_MODE_SURVIVAL;
    p.on_ground = true;
    (shared, p)
}
fn stack(name: &str, count: i32) -> Stack { Stack::new(blocks::item_named(name).unwrap(), count) }
fn payload(status: i32) -> Vec<u8> {
    let mut bytes = encode_varint(status);
    push_packed_position(&mut bytes, 0, 0, 0);
    bytes.push(0);
    bytes.extend(encode_varint(0));
    bytes
}
fn action(bytes: &[u8], p: &mut PlayerState, shared: &Shared) -> Reaction {
    let mut body = encode_varint(PLAYER_ACTION);
    body.extend_from_slice(bytes);
    handle_client_packet(&PacketInfo { id:PLAYER_ACTION, length:body.len() as i32, body, teleport:None,
        punch: false, foreign_inventory:false, legacy_inventory:None }, p, shared)
}
fn arm(p: &mut PlayerState, hand: i32, kind: vitals::UseKind) {
    let slot = if hand == 0 { p.inventory.selected_slot() } else { inventory::OFFHAND };
    let name = match kind {
        vitals::UseKind::Bow => "bow",
        vitals::UseKind::Shield => "shield",
        vitals::UseKind::Spyglass => "spyglass",
        vitals::UseKind::Crossbow { .. } => "crossbow",
        vitals::UseKind::Trident => "trident",
        _ => panic!("неподдерживаемая проба"),
    };
    p.inventory.set(slot, Some(stack(name,1)));
    p.inventory.set(9, Some(stack("arrow",4)));
    p.net.using = Some(vitals::Using { hand, since:80, kind });
}
fn malformed(status: i32) -> Vec<Vec<u8>> {
    let bytes = payload(status);
    let mut cases: Vec<_> = (0..bytes.len()).map(|n| bytes[..n].to_vec()).collect();
    let mut trailing = bytes;
    trailing.push(0);
    cases.push(trailing);
    cases
}

#[test]
fn release_full_packet_fires_once_in_both_hands_far_from_block_origin() {
    for hand in [0,1] {
        let (shared, mut p) = setup();
        arm(&mut p, hand, vitals::UseKind::Bow);
        let slot = if hand == 0 { p.inventory.selected_slot() } else { inventory::OFFHAND };
        let result = action(&payload(5), &mut p, &shared);
        assert!(result.acknowledge.is_none());
        assert!(p.net.using.is_none());
        assert_eq!(p.inventory.slot(9).unwrap().count, 3);
        assert_eq!(p.inventory.slot(slot).unwrap().extras().damage, 1);
        assert_eq!(shared.arrows.lock().unwrap().list.len(), 1);
        let before = p.inventory.slots().to_vec();
        action(&payload(5), &mut p, &shared);
        assert_eq!(p.inventory.slots(), before);
        assert_eq!(shared.arrows.lock().unwrap().list.len(), 1);
    }
}

#[test]
fn release_rejects_every_truncated_prefix_and_trailing_byte_before_cancelling_or_firing() {
    for bytes in malformed(5) {
        for kind in [vitals::UseKind::Bow, vitals::UseKind::Trident, vitals::UseKind::Shield, vitals::UseKind::Spyglass, vitals::UseKind::Crossbow { quick_charge:0 }] {
            let (shared, mut p) = setup();
            arm(&mut p, 0, kind);
            let using = p.net.using;
            let before = p.inventory.slots().to_vec();
            let from = shared.world_in(p.dimension).lock().unwrap().watch_events(219);
            action(&bytes, &mut p, &shared);
            assert!(shared.world_in(p.dimension).lock().unwrap().events_since(from,219).0.is_empty());
            assert_eq!(p.net.using, using, "{bytes:?}, {kind:?}");
            assert_eq!(p.inventory.slots(), before);
            assert!(shared.arrows.lock().unwrap().list.is_empty());
        }
    }
}

#[test]
fn release_cancels_food_only_for_a_complete_packet_without_spending_it() {
    let mut cases = malformed(5);
    cases.push(payload(5));
    for (index, bytes) in cases.iter().enumerate() {
        let (shared, mut p) = setup();
        let food = stack("apple",3);
        let slot = p.inventory.selected_slot();
        p.inventory.set(slot, Some(food));
        p.net.eating = Some(vitals::Eating { slot, stack:food, ticks_left:1, nutrition:4, saturation:2.4, effects:vec![], drink:None });
        action(bytes, &mut p, &shared);
        assert_eq!(p.net.eating.is_none(), index == cases.len()-1);
        assert_eq!(p.inventory.slot(slot), Some(food));
        assert_eq!(p.vit.food, 20);
        assert!(shared.arrows.lock().unwrap().list.is_empty());
    }
}

#[test]
fn release_after_real_spectator_transition_and_injected_dead_state_has_no_side_effects() {
    for dead in [false,true] {
        let (shared, mut p) = setup();
        arm(&mut p, 0, vitals::UseKind::Bow);
        if dead {
            // Защитная проверка: обычная смерть сама снимает использование.
            p.vit.dead = true;
        } else {
            vitals::change_game_mode(&mut p, GAME_MODE_SPECTATOR);
        }
        let using = p.net.using;
        let before = p.inventory.slots().to_vec();
        action(&payload(5), &mut p, &shared);
        assert_eq!(p.net.using, using);
        assert_eq!(p.inventory.slots(), before);
        assert!(shared.arrows.lock().unwrap().list.is_empty());
    }
}

#[test]
fn jab_rejects_every_truncated_prefix_trailing_byte_and_inactive_player() {
    let mut candidates: Vec<_> = malformed(7).into_iter().map(|bytes| (bytes,0)).collect();
    candidates.extend([(payload(7),1),(payload(7),2)]);
    for (bytes, mode) in candidates {
        let (shared, mut p) = setup();
        let mut data = crate::stackdata::Data::default();
        data.set_enchant(crate::stackdata::enchantment_number("lunge").unwrap(),3);
        p.inventory.set(p.inventory.selected_slot(), Some(stack("iron_spear",1).with_extras(data)));
        if mode == 1 { p.vit.dead = true; }
        if mode == 2 { vitals::change_game_mode(&mut p, GAME_MODE_SPECTATOR); }
        let before = p.inventory.slots().to_vec();
        let exhaustion = p.vit.exhaustion;
        let last_attack = p.last_attack;
        let from = shared.world_in(p.dimension).lock().unwrap().watch_events(219);
        action(&bytes, &mut p, &shared);
        assert!(shared.world_in(p.dimension).lock().unwrap().events_since(from,219).0.is_empty());
        assert_eq!(p.inventory.slots(), before);
        assert_eq!(p.vit.exhaustion, exhaustion);
        assert_eq!(p.last_attack, last_attack);
        assert!(p.net.velocity.is_none());
        assert!(shared.arrows.lock().unwrap().list.is_empty());
    }
}

#[test]
fn complete_release_throws_trident_once_in_both_hands() {
    for hand in [0,1] {
        let (shared, mut p) = setup();
        arm(&mut p, hand, vitals::UseKind::Trident);
        let slot = if hand == 0 { p.inventory.selected_slot() } else { inventory::OFFHAND };
        action(&payload(5), &mut p, &shared);
        assert!(p.net.using.is_none());
        assert!(p.inventory.slot(slot).is_none());
        assert_eq!(p.inventory.slot(9).unwrap().count,4);
        assert_eq!(shared.arrows.lock().unwrap().list.len(),1);
        action(&payload(5), &mut p, &shared);
        assert_eq!(shared.arrows.lock().unwrap().list.len(),1);
    }
}

#[test]
fn complete_release_stops_crossbow_shield_and_spyglass_without_ammo_or_projectile() {
    for kind in [vitals::UseKind::Crossbow { quick_charge:0 }, vitals::UseKind::Shield, vitals::UseKind::Spyglass] {
        let (shared, mut p) = setup();
        arm(&mut p, 0, kind);
        let before = p.inventory.slots().to_vec();
        action(&payload(5), &mut p, &shared);
        assert!(p.net.using.is_none());
        assert_eq!(p.inventory.slots(),before);
        assert!(shared.arrows.lock().unwrap().list.is_empty());
    }
}
