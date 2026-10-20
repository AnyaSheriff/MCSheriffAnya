//! Только ignored-test export: реальные native775 builder-ы → ограниченный Play776.
//! Модуль отсутствует в обычной сборке; публичные wire/Tracker guards не меняются.
use super::*;
use JavaVersion::{V26_1_2 as OLD, V26_2 as MID};
use serde_json::{Value, json};

const NAMES: [&str; 21] = [
    "login", "abilities", "game_state_change", "update_view_position",
    "map_chunk", "map_chunk", "map_chunk", "map_chunk", "map_chunk",
    "map_chunk", "map_chunk", "map_chunk", "map_chunk",
    "position", "entity_metadata", "window_items", "set_slot",
    "set_cursor_item", "set_player_inventory", "held_item_slot", "update_health",
];

#[derive(Clone, Copy)]
struct Contract {
    chunk: crate::chunk::Context,
    entity: u32,
    teleport: u32,
    pose: crate::teleport::Pose,
}

fn u32_value(value: &Value) -> Result<u32, Error> {
    value.as_u64().and_then(|n| u32::try_from(n).ok()).ok_or(Error::Invalid)
}
fn contract(value: &Value) -> Result<Contract, Error> {
    if value["schema"] != 1 || value["native_protocol"] != 775 || value["target_protocol"] != 776
        || value["online_mode"] != false || value["entity_id"] != 1 || value["teleport_id"] != 1
        || value.get("payload_protocol").is_some()
        || !value["item_context"].is_object()
        || value["item_context"].get("shared_enchantment_count").is_none()
        || value["item_context"]["shared_enchantment_count"] != Value::Null
        || value["metadata_registry_counts"] != json!([]) {
        return Err(Error::Invalid);
    }
    let chunk = &value["chunk_context"];
    if chunk["min_y"] != -64 || chunk["sections"] != 24 { return Err(Error::Invalid); }
    let biomes = u32_value(&chunk["shared_biome_count"])?;
    if biomes == 0 || biomes > 65_536 { return Err(Error::Invalid); }
    let p = &value["expected_pose"];
    for (field, expected) in [("x", 0.5), ("y", 65.0), ("z", 0.5), ("yaw", 0.0), ("pitch", 0.0)] {
        if p[field].as_f64() != Some(expected) { return Err(Error::Invalid); }
    }
    Ok(Contract {
        chunk: crate::chunk::Context { min_y: -64, sections: 24, shared_biome_count: biomes },
        entity: 1, teleport: 1, pose: crate::teleport::Pose { x: 0.5, y: 65.0, z: 0.5, yaw: 0.0, pitch: 0.0 },
    })
}
fn unhex(value: &Value) -> Result<Vec<u8>, Error> {
    let text = value.as_str().ok_or(Error::Invalid)?;
    if text.len() > 2 * LIMIT { return Err(Error::TooLarge); }
    if !text.len().is_multiple_of(2) { return Err(Error::Invalid); }
    text.as_bytes().as_chunks::<2>().0.iter().map(|pair| {
        let text = std::str::from_utf8(pair).map_err(|_| Error::Invalid)?;
        u8::from_str_radix(text, 16).map_err(|_| Error::Invalid)
    }).collect()
}
fn f32_value(input: &mut Cursor<'_>) -> Result<f32, Error> {
    let value = f32::from_be_bytes(input.take(4)?.try_into().map_err(|_| Error::Invalid)?);
    if !value.is_finite() { return Err(Error::Invalid); }
    Ok(value)
}
fn f64_value(input: &mut Cursor<'_>) -> Result<f64, Error> {
    let value = f64::from_be_bytes(input.take(8)?.try_into().map_err(|_| Error::Invalid)?);
    if !value.is_finite() { return Err(Error::Invalid); }
    Ok(value)
}
fn ordinary_slot(input: &mut Cursor<'_>, out: &mut Vec<u8>) -> Result<(), Error> {
    let context = crate::items::Context::default();
    let (stack, used) = crate::items::decode(OLD, input.rest, false, context).map_err(Error::Item)?;
    if let Some(stack) = &stack
        && (stack.count > 64 || !stack.removed.is_empty()
            || stack.added.iter().any(|component| !matches!(component.kind, 3 | 6))) {
        return Err(Error::PayloadUnavailable("bounded 776 fixture item"));
    }
    input.take(used)?;
    out.extend(crate::items::encode(MID, stack.as_ref(), false, context).map_err(Error::Item)?);
    Ok(())
}

/// Имя и ID сверяются явно. Равенство JSON не служит общим разрешением rawcopy.
fn packet(name: &str, body: &[u8], c: Contract) -> Result<Vec<u8>, Error> {
    if body.len() > LIMIT { return Err(Error::TooLarge); }
    let mut input = Cursor::new(body);
    let source = input.uint()?;
    let info = crate::packet_layouts::packet_info(RegistryKind::PlayClientbound, OLD, MID, source as usize)
        .ok_or(Error::Invalid)?;
    if info.name != name { return Err(Error::Invalid); }
    let mut out = Vec::new();
    uint(&mut out, info.target_id.ok_or(Error::PayloadUnavailable("missing 776 fixture packet"))?);
    match name {
        "login" => {
            let mut header = Cursor::new(input.rest);
            if i32::from_be_bytes(header.take(4)?.try_into().map_err(|_| Error::Invalid)?) != c.entity as i32 {
                return Err(Error::Invalid);
            }
            play_login(&mut input, &mut out, OLD, MID, Context { online_mode: Some(false), ..Default::default() })?;
        }
        "map_chunk" => {
            out.extend(crate::chunk::translate_776_fixture(input.take(input.remaining())?, c.chunk).map_err(Error::Chunk)?);
        }
        "entity_metadata" => {
            if Cursor::new(input.rest).uint()? != c.entity { return Err(Error::MissingEntityContext); }
            crate::entities::metadata(&mut input, &mut out, OLD, MID, Context {
                entity_kind: Some(crate::entity_layouts::PLAYER), ..Default::default()
            })?;
        }
        "window_items" | "set_slot" => {
            let window = input.uint()?;
            if window != 0 { return Err(Error::Invalid); }
            uint(&mut out, window); uint(&mut out, input.uint()?);
            if name == "window_items" {
                let count = input.count(46)?;
                if count != 46 { return Err(Error::Invalid); }
                uint(&mut out, count as u32);
                for _ in 0..count { ordinary_slot(&mut input, &mut out)?; }
                ordinary_slot(&mut input, &mut out)?;
            } else {
                let index = i16::from_be_bytes(input.take(2)?.try_into().map_err(|_| Error::Invalid)?);
                if !(0..46).contains(&index) { return Err(Error::Invalid); }
                out.extend(index.to_be_bytes()); ordinary_slot(&mut input, &mut out)?;
            }
        }
        "set_cursor_item" | "set_player_inventory" => {
            if name == "set_player_inventory" { uint(&mut out, input.count(45)? as u32); }
            ordinary_slot(&mut input, &mut out)?;
        }
        // Следующие формы не содержат ID контента; копируются только после
        // полного типизированного разбора и ограничений этого fixture.
        "abilities" | "game_state_change" | "update_view_position" | "position" | "held_item_slot" | "update_health" => {
            if !info.schema_equal { return Err(Error::PayloadUnavailable("changed 776 fixture control")); }
            let original = input.rest;
            match name {
                "abilities" => {
                    if input.take(1)?[0] & !15 != 0 { return Err(Error::Invalid); }
                    for _ in 0..2 { if f32_value(&mut input)? < 0.0 { return Err(Error::Invalid); } }
                }
                "game_state_change" => {
                    if input.take(1)?[0] != 13 || f32_value(&mut input)? != 0.0 { return Err(Error::Invalid); }
                }
                "update_view_position" => {
                    if input.varint()? != 0 || input.varint()? != 0 { return Err(Error::Invalid); }
                }
                "position" => {
                    if input.uint()? != c.teleport { return Err(Error::Invalid); }
                    for expected in [c.pose.x, c.pose.y, c.pose.z, 0.0, 0.0, 0.0] {
                        if f64_value(&mut input)? != expected { return Err(Error::Invalid); }
                    }
                    for expected in [c.pose.yaw, c.pose.pitch] {
                        if f32_value(&mut input)? != expected { return Err(Error::Invalid); }
                    }
                    if input.take(4)? != [0, 0, 0, 0] { return Err(Error::Invalid); }
                }
                "held_item_slot" => { input.count(8)?; }
                "update_health" => {
                    if !(0.0..=1024.0).contains(&f32_value(&mut input)?) { return Err(Error::Invalid); }
                    input.count(20)?;
                    if !(0.0..=20.0).contains(&f32_value(&mut input)?) { return Err(Error::Invalid); }
                }
                _ => unreachable!(),
            }
            if input.remaining() != 0 { return Err(Error::Invalid); }
            out.extend(original);
        }
        _ => return Err(Error::PayloadUnavailable("packet outside bounded 776 fixture")),
    }
    if input.remaining() != 0 { return Err(Error::Invalid); }
    if out.len() > LIMIT { return Err(Error::TooLarge); }
    Ok(out)
}

fn translate_fixture(value: &Value) -> Result<Vec<Vec<u8>>, Error> {
    let c = contract(value)?;
    let packets = value["packets"].as_array().ok_or(Error::Invalid)?;
    if packets.len() != NAMES.len() { return Err(Error::Invalid); }
    let mut output = Vec::with_capacity(packets.len());
    for (index, (entry, expected)) in packets.iter().zip(NAMES).enumerate() {
        if entry["name"] != expected { return Err(Error::Invalid); }
        let body = unhex(&entry["body_hex"])?;
        if expected == "map_chunk" {
            let mut input = Cursor::new(&body); input.uint()?;
            let x = i32::from_be_bytes(input.take(4)?.try_into().map_err(|_| Error::Invalid)?);
            let z = i32::from_be_bytes(input.take(4)?.try_into().map_err(|_| Error::Invalid)?);
            let chunk_index = (index - 4) as i32;
            if (x, z) != (chunk_index % 3 - 1, chunk_index / 3 - 1) { return Err(Error::Invalid); }
        }
        // Exact order выше гарантирует: metadata self только после успешного Join.
        output.push(packet(expected, &body, c)?);
    }
    Ok(output)
}

#[test]
#[ignore = "экспорт bounded776 только после native own-builder export; не запускает Minecraft"]
fn export_initial_play_776_fixture() {
    let root = std::env::var_os("MCSA_776_FIXTURE_DIR").map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tmp"));
    assert!(root.is_absolute(), "MCSA_776_FIXTURE_DIR должен быть абсолютным каталогом артефактов");
    let input = std::fs::read(root.join("initial-play-native-26.2.json")).unwrap();
    assert!(input.len() <= 32 * 1024 * 1024);
    let fixture: Value = serde_json::from_slice(&input).unwrap();
    let packets = translate_fixture(&fixture).unwrap();
    let entries: Vec<Value> = NAMES.iter().zip(&packets).map(|(name, body)| {
        let hex: String = body.iter().map(|byte| format!("{byte:02x}")).collect();
        json!({"name": name, "body_hex": hex})
    }).collect();
    let mut translated = fixture.clone();
    translated["payload_protocol"] = json!(776);
    translated["packets"] = json!(entries);
    translated["native_fixture"] = json!("initial-play-native-26.2.json");
    std::fs::write(root.join("initial-play-26.2.json"), serde_json::to_vec_pretty(&translated).unwrap()).unwrap();
    let report = json!({
        "schema": 1, "native_protocol": 775, "target_protocol": 776, "payload_protocol": 776,
        "native_fixture": "initial-play-native-26.2.json", "source_bytes": input.len(),
        "packets": NAMES, "packet_count": packets.len(),
        "expected_pose": fixture["expected_pose"], "teleport_id": 1, "entity_id": 1,
        "chunk_context": fixture["chunk_context"],
        "limits": ["stone/noBE/no effects; bounded control/player/items only",
                   "private test export, public wire/Tracker/admission776 remains closed",
                   "actual client acceptance is a separate runtime step"]
    });
    std::fs::write(root.join("initial-play-26.2-report.json"), serde_json::to_vec_pretty(&report).unwrap()).unwrap();
}

fn sample_contract() -> Contract {
    Contract {
        chunk: crate::chunk::Context { min_y: -64, sections: 24, shared_biome_count: 59 },
        entity: 1, teleport: 1,
        pose: crate::teleport::Pose { x: 0.5, y: 65.0, z: 0.5, yaw: 0.0, pitch: 0.0 },
    }
}

#[test]
fn bounded_controls_use_explicit_scalar_shapes_and_refuse_trailing_or_wrong_names() {
    let mut abilities = vec![0x40, 11]; abilities.extend(0.05f32.to_be_bytes()); abilities.extend(0.1f32.to_be_bytes());
    let mut position = vec![0x48, 1];
    for value in [0.5f64, 65.0, 0.5, 0.0, 0.0, 0.0] { position.extend(value.to_be_bytes()); }
    position.extend([0; 12]);
    for (name, body) in [("abilities", abilities), ("position", position),
        ("update_view_position", vec![0x5e, 0, 0]), ("held_item_slot", vec![0x69, 0])] {
        assert_eq!(packet(name, &body, sample_contract()).unwrap(), body);
        for length in 0..body.len() { assert!(packet(name, &body[..length], sample_contract()).is_err()); }
        let mut extra = body.clone(); extra.push(0);
        assert_eq!(packet(name, &extra, sample_contract()), Err(Error::Invalid));
        assert_eq!(packet("update_health", &body, sample_contract()), Err(Error::Invalid));
    }
    assert_eq!(packet("held_item_slot", &[0x69, 9], sample_contract()), Err(Error::TooLarge));
    assert!(packet("world_particles", &[0x2f], sample_contract()).is_err());
}

#[test]
fn bounded_player_metadata_and_items_do_not_enable_other_entities_or_components() {
    let body = [0x63, 1, 15, 42, 1, 16, 0, 127, 255];
    assert_eq!(packet("entity_metadata", &body, sample_contract()).unwrap(), body);
    let mut foreign = body; foreign[1] = 2;
    assert_eq!(packet("entity_metadata", &foreign, sample_contract()), Err(Error::MissingEntityContext));
    let mut payload = vec![0x60];
    payload.extend(crate::items::encode(OLD, Some(&crate::items::Stack {
        item: 1, count: 1, added: vec![crate::items::Component { kind: 4, data: vec![] }], removed: vec![],
    }), false, Default::default()).unwrap());
    assert_eq!(packet("set_cursor_item", &payload, sample_contract()),
        Err(Error::PayloadUnavailable("bounded 776 fixture item")));
}

#[test]
fn test_only_chunk_helper_rejects_block_entities_and_preserves_public_guard() {
    let section = crate::chunk_sections::Section {
        blocks_present: 4096, fluids_present: 0, blocks: vec![1; 4096], biomes: vec![0; 64],
    };
    let sections = crate::chunk_sections::encode(OLD, &[section], 1).unwrap();
    let mut payload = vec![0; 9]; // x0,z0, heightmaps0.
    uint(&mut payload, sections.len() as u32); payload.extend(sections);
    let entity_count = payload.len(); payload.push(0);
    payload.extend([0; 6]); // четыре пустых long masks, два пустых arrays.
    let context = crate::chunk::Context { min_y: -64, sections: 1, shared_biome_count: 1 };
    assert!(crate::chunk::translate_776_fixture(&payload, context).is_ok());
    assert_eq!(crate::chunk::translate(OLD, MID, &payload, context),
        Err(crate::chunk::Error::UnsupportedVersion(MID)));
    let mut with_entity = payload; with_entity[entity_count] = 1;
    assert_eq!(crate::chunk::translate_776_fixture(&with_entity, context),
        Err(crate::chunk::Error::Entity(crate::block_entities::Error::UnsupportedVersion)));
}

#[test]
fn fixture_contract_refuses_ambiguous_versions_order_context_and_hex() {
    assert!(translate_fixture(&json!({})).is_err());
    for text in ["0", "gg", "💥"] { assert!(unhex(&json!(text)).is_err()); }
    let mut fixture = json!({
        "schema": 1, "native_protocol": 775, "target_protocol": 776,
        "online_mode": false, "entity_id": 1, "teleport_id": 1,
        "expected_pose": {"x": 0.5, "y": 65.0, "z": 0.5, "yaw": 0, "pitch": 0},
        "chunk_context": {"min_y": -64, "sections": 24, "shared_biome_count": 59},
        "item_context": {"shared_enchantment_count": null}, "metadata_registry_counts": [],
        "packets": []
    });
    assert!(contract(&fixture).is_ok());
    assert!(translate_fixture(&fixture).is_err());
    fixture["payload_protocol"] = json!(776); assert!(contract(&fixture).is_err());
    fixture.as_object_mut().unwrap().remove("payload_protocol");
    fixture["target_protocol"] = json!(777); assert!(contract(&fixture).is_err());
    fixture["target_protocol"] = json!(776);
    fixture["chunk_context"]["shared_biome_count"] = json!(0); assert!(contract(&fixture).is_err());
    fixture["chunk_context"]["shared_biome_count"] = json!(59);
    fixture["metadata_registry_counts"] = json!([["minecraft:cow_variant", 3]]);
    assert!(contract(&fixture).is_err());
}
