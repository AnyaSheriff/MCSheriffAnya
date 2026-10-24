//! Старые действия проходят настоящие проверки ядра, без второго набора правил.
use super::*;
use mcsa_protocol::{legacy_actions, legacy_chunk::HeightWindow, teleport::Pose};

fn setup(mode: i32) -> (Shared, PlayerState) {
    let mut world = World::in_memory();
    for x in -3..=3 { for z in -3..=3 { world.set_block(x, 63, z, blocks::state_by_name("stone").unwrap()); } }
    let shared = Shared::new(world, World::in_memory(), Default::default(), Default::default(), Default::default(),
        crate::ops::Ops::open(std::path::Path::new("tmp/legacy-actions-no-ops.json")), crate::skins::Settings::default());
    let mut player = PlayerState::new([131; 16], "OldClient".into(), None, None, 0x7f, 131, (0.5, 64.0, 0.5));
    vitals::change_game_mode(&mut player, mode); player.awaiting_teleport = false;
    shared.players.lock().unwrap().join(player.member(), player.entity_id).unwrap();
    (shared, player)
}

fn apply(body: &[u8], player: &mut PlayerState, shared: &Shared) -> Reaction {
    let context = legacy_actions::Context { window: HeightWindow::new(-64).unwrap(),
        pose: Pose { x: player.x, y: player.y, z: player.z, yaw: player.yaw, pitch: player.pitch } };
    let body = legacy_actions::translate(body, context).unwrap();
    let (id, _) = decode_varint(&body).unwrap();
    let packet = PacketInfo { id, length: body.len() as i32, body, teleport: None, foreign_inventory: false, legacy_inventory: None };
    handle_client_packet(&packet, player, shared)
}

fn position(x: i32, y: u8, z: i32) -> [u8; 8] {
    (((x as u64 & 0x3ffffff) << 38) | (u64::from(y) << 26) | (z as u64 & 0x3ffffff)).to_be_bytes()
}

#[test]
fn legacy_creative_flags_do_not_grant_survival_flight() {
    let (shared, mut player) = setup(GAME_MODE_SURVIVAL);
    let mut packet = vec![0x13, 15];
    packet.extend(99f32.to_be_bytes()); packet.extend(99f32.to_be_bytes());
    apply(&packet, &mut player, &shared);
    assert!(!player.abilities.flying); assert_eq!(player.game_mode, GAME_MODE_SURVIVAL);
    vitals::change_game_mode(&mut player, GAME_MODE_CREATIVE);
    apply(&packet, &mut player, &shared);
    assert!(player.abilities.flying);
}

#[test]
fn legacy_placement_consumes_server_inventory_and_uses_translated_height() {
    let (shared, mut player) = setup(GAME_MODE_SURVIVAL);
    let slot = player.inventory.selected_slot();
    player.inventory.set(slot, Some(Stack::new(blocks::item_named("stone").unwrap(), 2)));
    let mut packet = vec![0x1f]; packet.extend(position(1, 127, 0)); packet.extend([1, 0]);
    for cursor in [0.5f32, 1.0, 0.5] { packet.extend(cursor.to_be_bytes()); }
    apply(&packet, &mut player, &shared);
    assert_eq!(shared.world_in(Dimension::Overworld).lock().unwrap().get_block(1, 64, 0), blocks::state_by_name("stone").unwrap());
    assert_eq!(player.inventory.slot(slot).unwrap().count, 1);
    assert_eq!(shared.world_in(Dimension::Overworld).lock().unwrap().get_block(1, 128, 0), 0);
}

#[test]
fn legacy_breaking_respects_spectator_and_reach() {
    let (shared, mut player) = setup(GAME_MODE_SPECTATOR);
    let stone = blocks::state_by_name("stone").unwrap();
    let mut packet = vec![0x14, 0]; packet.extend(position(1, 127, 0)); packet.push(1);
    apply(&packet, &mut player, &shared);
    assert_eq!(shared.world_in(Dimension::Overworld).lock().unwrap().get_block(1, 63, 0), stone);
    vitals::change_game_mode(&mut player, GAME_MODE_CREATIVE);
    apply(&packet, &mut player, &shared);
    assert_eq!(shared.world_in(Dimension::Overworld).lock().unwrap().get_block(1, 63, 0), 0);
    shared.world_in(Dimension::Overworld).lock().unwrap().set_block(30, 63, 0, stone);
    packet[2..10].copy_from_slice(&position(30, 127, 0));
    apply(&packet, &mut player, &shared);
    assert_eq!(shared.world_in(Dimension::Overworld).lock().unwrap().get_block(30, 63, 0), stone);
}


fn inventory_action(body: &[u8], player: &mut PlayerState, shared: &Shared) -> Reaction {
    let (body, action) = mcsa_protocol::legacy_inventory::decode(body, mcsa_protocol::legacy_items::Context {
        enchantments: super::super::packet::legacy_enchantments(),
    }).unwrap();
    let id = if body.is_empty() { -1 } else { decode_varint(&body).unwrap().0 };
    handle_client_packet(&PacketInfo { id, length: body.len() as i32, body, teleport: None,
        foreign_inventory: false, legacy_inventory: action }, player, shared)
}

fn click_packet(transaction: i16, slot: i16, expected: &[u8]) -> Vec<u8> {
    let mut body = vec![7, 0]; body.extend(slot.to_be_bytes()); body.push(0);
    body.extend(transaction.to_be_bytes()); body.push(0); body.extend(expected); body
}

#[test]
fn legacy_inventory_replays_official_rejection_and_matching_ack() {
    let (shared, mut player) = setup(GAME_MODE_SURVIVAL);
    let stone = Stack::new(blocks::item_named("stone").unwrap(), 8);
    player.inventory.set(36, Some(stone));
    let stone_wire = [0, 1, 8, 0, 0, 0];
    for (transaction, slot, expected) in [(1, 36, stone_wire.as_slice()), (2, 37, &[255,255])] {
        let reaction = inventory_action(&click_packet(transaction, slot, expected), &mut player, &shared);
        assert!(reaction.legacy_confirmation.unwrap().accepted);
    }
    assert_eq!(player.inventory.slot(36), None); assert_eq!(player.inventory.slot(37), Some(stone));
    let reaction = inventory_action(&click_packet(3, 37, &[1,8,64,0,0,0]), &mut player, &shared);
    assert!(!reaction.legacy_confirmation.unwrap().accepted); assert!(reaction.inventory);
    assert_eq!(player.inventory.slot(37), None); assert_eq!(player.inventory.cursor(), Some(stone));
    for acknowledgement in [None, Some([5,0,0,2,1]), Some([5,1,0,3,1])] {
        if let Some(body) = acknowledgement { inventory_action(&body, &mut player, &shared); }
        let reaction = inventory_action(&click_packet(4, 38, &[255,255]), &mut player, &shared);
        assert!(reaction.legacy_confirmation.is_none()); assert_eq!(player.inventory.slot(38), None);
        assert_eq!(player.inventory.cursor(), Some(stone));
    }
    inventory_action(&[5,0,0,3,0], &mut player, &shared);
    let reaction = inventory_action(&click_packet(6, 38, &[255,255]), &mut player, &shared);
    assert!(reaction.legacy_confirmation.unwrap().accepted);
    assert_eq!(player.inventory.slot(38), Some(stone)); assert_eq!(player.inventory.cursor(), None);
}

#[test]
fn legacy_rejection_survives_close_but_not_death_and_respawn() {
    let (shared, mut player) = setup(GAME_MODE_SURVIVAL);
    let wrong = click_packet(3, 5, &[1,8,64,0,0,0]);
    assert!(!inventory_action(&wrong, &mut player, &shared).legacy_confirmation.unwrap().accepted);
    let close = PacketInfo { id: CLOSE_CONTAINER, length: 2, body: vec![CLOSE_CONTAINER as u8, 0],
        teleport: None, foreign_inventory: false, legacy_inventory: None };
    handle_client_packet(&close, &mut player, &shared);
    let empty = click_packet(4, 37, &[255,255]);
    assert!(inventory_action(&empty, &mut player, &shared).legacy_confirmation.is_none());
    player.vit.dead = true;
    vitals::respawned(&mut player);
    assert!(inventory_action(&empty, &mut player, &shared).legacy_confirmation.unwrap().accepted);
    // Запоздавший ответ прежней жизни не снимает новый отказ с другим номером.
    let wrong = click_packet(5, 5, &[1,8,64,0,0,0]);
    assert!(!inventory_action(&wrong, &mut player, &shared).legacy_confirmation.unwrap().accepted);
    inventory_action(&[5,0,0,3,1], &mut player, &shared);
    assert!(inventory_action(&empty, &mut player, &shared).legacy_confirmation.is_none());
    inventory_action(&[5,0,0,5,1], &mut player, &shared);
    assert!(inventory_action(&empty, &mut player, &shared).legacy_confirmation.unwrap().accepted);
}

#[test]
fn legacy_creative_slot_cannot_create_items_in_survival() {
    let (shared, mut player) = setup(GAME_MODE_SURVIVAL);
    let packet = [0x1b, 0,36, 1,8,64,0,0,0];
    inventory_action(&packet, &mut player, &shared);
    assert_eq!(player.inventory.slot(36), None);
    vitals::change_game_mode(&mut player, GAME_MODE_CREATIVE);
    inventory_action(&packet, &mut player, &shared);
    assert_eq!(player.inventory.slot(36), Some(Stack::new(blocks::item_named("diamond").unwrap(),64)));
    let invalid_count = [0x1b, 0,37, 1,8,65,0,0,0];
    inventory_action(&invalid_count, &mut player, &shared);
    assert_eq!(player.inventory.slot(37), None);
}


#[test]
fn legacy_inventory_modes_use_shared_handlers_with_recorded_inputs() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!("../../../tools/protocol-bridge/testdata/legacy-inventory.json")).unwrap();
    let stack = |v: &serde_json::Value| {
        if v.is_null() { None } else {
            assert_eq!(v["item"], 7); assert_eq!(v["damage"], 0);
            Some(Stack::new(blocks::item_named("bedrock").unwrap(), v["count"].as_i64().unwrap() as i32))
        }
    };
    for snapshot in fixture["snapshots"].as_array().unwrap() {
        let stage = snapshot["stage"].as_str().unwrap().strip_suffix("-observe").unwrap();
        let (shared, mut player) = setup(if stage.contains("creative") { GAME_MODE_CREATIVE } else { GAME_MODE_SURVIVAL });
        player.inventory.set(36, Some(Stack::new(blocks::item_named("bedrock").unwrap(),8)));
        let (native_shared, mut native) = setup(player.game_mode);
        native.inventory = player.inventory.clone();
        for action in fixture["actions"].as_array().unwrap().iter().filter(|a| a["stage"] == stage || a["stage"] == format!("{stage}-pickup")) {
            let hex = action["payload"].as_str().unwrap(); let mut body = vec![7];
            for pair in hex.as_bytes().as_chunks::<2>().0 { body.push(u8::from_str_radix(std::str::from_utf8(pair).unwrap(),16).unwrap()); }
            let (native_body, _) = mcsa_protocol::legacy_inventory::decode(&body, mcsa_protocol::legacy_items::Context {
                enchantments: super::super::packet::legacy_enchantments(),
            }).unwrap();
            let native_packet = PacketInfo { id: CLICK_CONTAINER, length: native_body.len() as i32, body: native_body,
                teleport: None, foreign_inventory: false, legacy_inventory: None };
            handle_client_packet(&native_packet, &mut native, &native_shared);
            let reaction = inventory_action(&body, &mut player, &shared);
            assert!(reaction.legacy_confirmation.unwrap().accepted, "{stage}");
            assert_eq!(player.inventory.slots(), native.inventory.slots(), "{stage} primary rules");
            assert_eq!(player.inventory.cursor(), native.inventory.cursor(), "{stage} primary cursor");
        }
        let correction = inventory_action(&click_packet(100, 5, &[1,8,64,0,0,0]), &mut player, &shared);
        assert!(!correction.legacy_confirmation.unwrap().accepted); assert!(correction.inventory);
        for (index, value) in snapshot["slots"].as_array().unwrap().iter().enumerate() {
            assert_eq!(player.inventory.slot(index as i32), stack(value), "{stage} slot {index}");
        }
        assert_eq!(player.inventory.cursor(), stack(&snapshot["cursor"]), "{stage} cursor");
    }
}
