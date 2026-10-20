//! Ограниченный native775 fixture для отдельной проверки клиента776.
//! Только наши builders; экспорт не открывает обычный Play776.
use super::*;
use serde_json::{Value, json};

const NAMES: [&str; 21] = [
    "login", "abilities", "game_state_change", "update_view_position",
    "map_chunk", "map_chunk", "map_chunk", "map_chunk", "map_chunk",
    "map_chunk", "map_chunk", "map_chunk", "map_chunk", "position",
    "entity_metadata", "window_items", "set_slot", "set_cursor_item",
    "set_player_inventory", "held_item_slot", "update_health",
];
const IDS: [i32; 21] = [
    0x31, 0x40, 0x26, 0x5e, 0x2d, 0x2d, 0x2d, 0x2d, 0x2d, 0x2d,
    0x2d, 0x2d, 0x2d, 0x48, 0x63, 0x12, 0x14, 0x60, 0x6c, 0x69, 0x68,
];

async fn fixture() -> Value {
    let mut world = World::in_memory();
    let stone = blocks::state_by_name("stone").unwrap();
    // Все девять чанков имеют пол: Y64, ноги игрока Y65. Нет генератора/диска.
    for z in -16..32 { for x in -16..32 {
        assert!(world.set_block_silently(x, 64, z, stone));
    } }
    let world = Mutex::new(world);
    let mut out = Outgoing::capture();
    send_login(&mut out, Dimension::Overworld, GAME_MODE_SURVIVAL, 1, 2, 2, false).await.unwrap();
    send_abilities(&mut out, GAME_MODE_SURVIVAL).await.unwrap();
    send_game_event_start_waiting(&mut out).await.unwrap();
    send_center_chunk(&mut out, (0, 0)).await.unwrap();
    for z in -1..=1 { for x in -1..=1 {
        send_chunk(&mut out, &world, x, z, &ChunkLight::open_sky()).await.unwrap();
    } }
    let mut player = PlayerState::new([72; 16], "MCSA probe".into(), None, None, 0x7f, 0, (0.5, 65.0, 0.5));
    player.entity_id = 1;
    player.game_mode = GAME_MODE_SURVIVAL;
    player.yaw = 0.0;
    player.pitch = 0.0;
    player.teleport = 0;
    send_player_position(&mut out, &mut player).await.unwrap();
    assert_eq!(player.teleport, 1);
    send_look(&mut out, 1, 0x7f, 1, false).await.unwrap();

    player.inventory.set(36, Some(Stack::new(blocks::item_named("stone").unwrap(), 3)));
    send_container_content(&mut out, &mut player).await.unwrap();
    let paper = Stack::new(blocks::item_named("paper").unwrap(), 1).with_extras(crate::stackdata::Data {
        name: Some("MCSA probe".into()), ..Default::default()
    });
    player.inventory.set(37, Some(paper));
    send_container_slot(&mut out, &mut player, 37).await.unwrap();
    // Для этих двух самостоятельных пакетов используется тот же push_slot,
    // что и в основном инвентаре; отдельного игрового handler здесь нет.
    let mut cursor = encode_varint(SET_CURSOR_ITEM);
    push_slot(&mut cursor, None);
    out.send(&cursor).await.unwrap();
    let sword = Stack::new(blocks::item_named("diamond_sword").unwrap(), 1).with_extras(crate::stackdata::Data {
        damage: 7, ..Default::default()
    });
    player.inventory.set(38, Some(sword));
    let mut slot = encode_varint(0x6c);
    slot.extend(encode_varint(2)); // Set Player Inventory: индекс hotbar2, не window-slot38.
    push_slot(&mut slot, Some(sword));
    out.send(&slot).await.unwrap();
    send_held_slot(&mut out, 0).await.unwrap();
    vitals::send_health(&mut out, &player.vit).await.unwrap();
    out.flush().await.unwrap();

    let mut raw = out.queued();
    let mut packets = Vec::new();
    while !raw.is_empty() {
        let packet = read_packet(&mut raw).await.unwrap();
        let index = packets.len();
        assert!(index < NAMES.len());
        assert_eq!(packet.id, IDS[index], "{}", NAMES[index]);
        if (4..13).contains(&index) {
            let payload = packet.payload();
            let x = i32::from_be_bytes(payload[..4].try_into().unwrap());
            let z = i32::from_be_bytes(payload[4..8].try_into().unwrap());
            assert_eq!((x, z), (((index - 4) % 3) as i32 - 1, ((index - 4) / 3) as i32 - 1));
        }
        if index == 1 { assert_eq!(packet.payload()[0], 0, "Survival без полёта"); }
        if index == 13 {
            let mut position = Cursor::new(packet.payload());
            assert_eq!(position.varint(), Some(1));
            for expected in [0.5, 65.0, 0.5, 0.0, 0.0, 0.0] {
                assert_eq!(f64::from_be_bytes(position.bytes(8).unwrap().try_into().unwrap()), expected);
            }
            assert_eq!(position.f32(), Some(0.0));
            assert_eq!(position.f32(), Some(0.0));
            assert_eq!(position.u32(), Some(0));
            assert_eq!(position.offset, position.bytes.len());
        }
        if index == 20 {
            let mut health = Cursor::new(packet.payload());
            assert_eq!(health.f32(), Some(20.0));
            assert_eq!(health.varint(), Some(20));
            assert_eq!(health.f32(), Some(5.0));
            assert_eq!(health.offset, health.bytes.len());
        }
        packets.push(json!({"name":NAMES[index], "body_hex":hex(&packet.body)}));
    }
    assert_eq!(packets.len(), NAMES.len());
    let biomes = configuration::REQUIRED_REGISTRIES.iter()
        .find(|(name, _)| *name == "minecraft:worldgen/biome").unwrap().1.len();
    json!({
        "schema":1, "native_protocol":775, "target_protocol":776,
        "online_mode":false, "entity_id":1, "teleport_id":1,
        "expected_pose":{"x":0.5,"y":65.0,"z":0.5,"yaw":0.0,"pitch":0.0},
        "chunk_context":{"min_y":world::MIN_Y,"sections":world::SECTIONS,"shared_biome_count":biomes},
        "item_context":{"shared_enchantment_count":null}, "metadata_registry_counts":[],
        "packets":packets
    })
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes { write!(&mut text, "{byte:02x}").unwrap(); }
    text
}

#[tokio::test]
async fn initial_776_native_fixture_has_exact_order_and_grounded_pose() {
    let value = fixture().await;
    assert_eq!(value["packets"].as_array().unwrap().len(), 21);
    assert_eq!(value["expected_pose"]["y"], 65.0);
    assert_eq!(value["chunk_context"]["sections"], 24);
    assert_eq!(value["chunk_context"]["min_y"], -64);
    assert!(value["chunk_context"]["shared_biome_count"].as_u64().unwrap() > 0);
}

#[tokio::test]
#[ignore = "native775 экспорт для отдельного закрытого Play776 fixture"]
async fn export_initial_play_native_776_probe() {
    let value = fixture().await;
    std::fs::create_dir_all("tmp").unwrap();
    std::fs::write("tmp/initial-play-native-26.2.json", serde_json::to_vec_pretty(&value).unwrap()).unwrap();
}
