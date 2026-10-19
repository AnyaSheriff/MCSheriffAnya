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
    let packet = PacketInfo { id, length: body.len() as i32, body, teleport: None, foreign_inventory: false };
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
    assert!(!player.creative_flying); assert_eq!(player.game_mode, GAME_MODE_SURVIVAL);
    vitals::change_game_mode(&mut player, GAME_MODE_CREATIVE);
    apply(&packet, &mut player, &shared);
    assert!(player.creative_flying);
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
