//! Проверяем только низкоуровневые payload helpers по собственным ordinary776.
//! GUI-client ACK и полный client/admission маршрут этим тестом не доказаны.
use super::*;
use crate::{chunk_sections, light::Light, teleport::{Confirmation, Pose}};
use JavaVersion::{V1_12_2, V26_1_2 as OLD, V26_2 as MID, V26_3 as NEW};

fn fixture() -> serde_json::Value {
    serde_json::from_str(include_str!("../testdata/ordinary-payloads-26.2.json")).unwrap()
}
fn hex(value: &serde_json::Value) -> Vec<u8> {
    value.as_str().unwrap().as_bytes().as_chunks::<2>().0.iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap()).collect()
}
fn mask_value(mask: &[u8]) -> u64 {
    mask.iter().enumerate().map(|(i, value)| u64::from(*value) << (i * 8)).sum()
}
fn section_bytes(entry: &serde_json::Value) -> Vec<u8> {
    let mut bytes = hex(&entry["blocks_hex"]);
    // Это наш синтетический biome0, НЕ записанная палитра official776.
    bytes.extend([0, 0]);
    bytes
}
fn move_payload(from: JavaVersion, to: JavaVersion, bytes: &[u8], rotation: bool) -> Result<Vec<u8>, Error> {
    let mut cursor = Cursor::new(bytes);
    let mut out = Vec::new();
    entity_move(&mut cursor, &mut out, from, to, rotation)?;
    if cursor.remaining() != 0 { return Err(Error::Invalid); }
    Ok(out)
}

#[test]
fn observed_776_long_light_masks_and_torch_cells_round_trip_all_three_layouts() {
    let records = fixture();
    for entry in records["light"].as_array().unwrap() {
        let bytes = hex(&entry["light_hex"]);
        let decoded = Light::decode(MID, &bytes).unwrap();
        for (mask, expected) in [&decoded.sky, &decoded.block, &decoded.empty_sky, &decoded.empty_block]
            .into_iter().zip(entry["masks"].as_array().unwrap()) {
            assert_eq!(mask_value(mask), expected.as_u64().unwrap());
        }
        // Факел x=-16,y=65,z=-16: section bit9, локальная ячейка y1,z0,x0.
        let mask = mask_value(&decoded.block);
        let level = if mask & (1 << 9) != 0 {
            let index = (mask & ((1 << 9) - 1)).count_ones() as usize;
            decoded.block_arrays[index][128] & 15
        } else { 0 };
        assert_eq!(u64::from(level), entry["torch_cell_level"].as_u64().unwrap());
        assert_eq!(decoded.encode(MID).unwrap(), bytes);
        assert_eq!(decoded.encode(OLD).unwrap(), bytes);
        for version in [OLD, MID, NEW] {
            let projected = decoded.encode(version).unwrap();
            assert_eq!(Light::decode(version, &projected).unwrap(), decoded);
            let mut extra = projected.clone(); extra.push(0);
            assert_eq!(Light::decode(version, &extra), Err(crate::light::Error::Invalid));
            assert!(Light::decode(version, &projected[..projected.len() - 1]).is_err());
        }
    }
}

#[test]
fn observed_authored_776_block_palette_keeps_all_4096_states_across_versions() {
    let records = fixture();
    for entry in records["sections"].as_array().unwrap() {
        let bytes = section_bytes(entry);
        let decoded = chunk_sections::decode(MID, &bytes, 1, 1).unwrap();
        let mut expected: Vec<u32> = entry["expected_prefix"].as_array().unwrap().iter()
            .map(|value| value.as_u64().unwrap() as u32).collect();
        expected.resize(4096, entry["remaining_state"].as_u64().unwrap() as u32);
        assert_eq!(decoded[0].blocks, expected);
        assert_eq!(decoded[0].blocks_present, 4096);
        assert_eq!(decoded[0].fluids_present, 0);
        assert_eq!(decoded[0].biomes, vec![0; 64]);
        assert_eq!(chunk_sections::encode(MID, &decoded, 1).unwrap(), bytes);
        for version in [OLD, MID, NEW] {
            let translated = chunk_sections::translate(MID, version, &bytes, 1, 1).unwrap();
            assert_eq!(translated[4], if version == NEW { 16 } else { 15 });
            let target = chunk_sections::decode(version, &translated, 1, 1).unwrap();
            for (&source, &target) in expected.iter().zip(&target[0].blocks) {
                assert_eq!(ids::translate(RegistryKind::BlockState, MID, version, source), Ok(target));
            }
            assert_eq!(chunk_sections::translate(version, MID, &translated, 1, 1).unwrap(), bytes);
        }
    }
}

#[test]
fn section_776_direct_mapping_keeps_new_content_and_distinguishes_unknown_from_missing() {
    let new_shared = (0..32_366).find(|id|
        matches!(ids::translate(RegistryKind::BlockState, MID, OLD, *id),
            Err(ids::IdTranslationError::MissingTarget(_)))
        && ids::translate(RegistryKind::BlockState, MID, NEW, *id).is_ok()).unwrap();
    let section = chunk_sections::Section {
        blocks_present: 4096, fluids_present: 0, blocks: vec![new_shared; 4096], biomes: vec![0; 64],
    };
    let bytes = chunk_sections::encode(MID, &[section], 1).unwrap();
    let newest = chunk_sections::translate(MID, NEW, &bytes, 1, 1).unwrap();
    assert_eq!(chunk_sections::translate(NEW, MID, &newest, 1, 1).unwrap(), bytes);
    assert!(matches!(chunk_sections::translate(MID, OLD, &bytes, 1, 1),
        Err(chunk_sections::Error::Id(ids::IdTranslationError::MissingTarget(_)))));

    let records = fixture();
    let bytes = section_bytes(&records["sections"][0]);
    let mut wrong_width = bytes.clone(); wrong_width[4] = 16;
    assert_eq!(chunk_sections::decode(MID, &wrong_width, 1, 1), Err(chunk_sections::Error::Invalid));
    let mut unknown = bytes.clone();
    let word = u64::from_be_bytes(unknown[5..13].try_into().unwrap());
    unknown[5..13].copy_from_slice(&((word & !0x7fff) | 0x7fff).to_be_bytes());
    assert!(matches!(chunk_sections::decode(MID, &unknown, 1, 1),
        Err(chunk_sections::Error::Id(ids::IdTranslationError::UnknownId(32_767)))));
    for length in [0, 4, 5, bytes.len() - 1] {
        assert!(chunk_sections::decode(MID, &bytes[..length], 1, 1).is_err());
    }
    let mut extra = bytes.clone(); extra.push(0);
    assert_eq!(chunk_sections::decode(MID, &extra, 1, 1), Err(chunk_sections::Error::Invalid));
    assert_eq!(chunk_sections::decode(MID, &bytes, 0, 1), Err(chunk_sections::Error::Invalid));
    assert_eq!(chunk_sections::decode(MID, &bytes, 1, 0), Err(chunk_sections::Error::Invalid));
    assert_eq!(chunk_sections::decode(V1_12_2, &bytes, 1, 1), Err(chunk_sections::Error::UnsupportedVersion));
}

#[test]
fn observed_776_move_look_preserves_angles_ground_and_commanded_positions() {
    let records = fixture();
    let mut position = [0.0; 3];
    for (axis, value) in position.iter_mut().zip(records["cow"]["spawn_position"].as_array().unwrap()) {
        *axis = value.as_f64().unwrap();
    }
    let mut rotated = 0;
    for command in records["motions"].as_array().unwrap() {
        for entry in command["packets"].as_array().unwrap() {
            let body = hex(&entry["body"]);
            let rotation = entry["name"] == "entity_move_look";
            let mut cursor = Cursor::new(&body);
            assert_eq!(cursor.uint().unwrap(), if rotation { 0x36 } else { 0x35 });
            let payload = cursor.rest;
            let entity = cursor.uint().unwrap();
            assert_eq!(u64::from(entity), records["cow"]["id"].as_u64().unwrap());
            let deltas: Vec<i16> = entry["decoded"]["delta"].as_array().unwrap().iter()
                .map(|value| value.as_i64().unwrap() as i16).collect();
            for (axis, delta) in position.iter_mut().zip(&deltas) { *axis += f64::from(*delta) / 4096.0; }
            let ground = entry["decoded"]["on_ground"].as_u64().unwrap() as u8;
            let mut expected = Vec::new(); uint(&mut expected, entity);
            expected.push(ground); // 777 linear: properties=onGround, step count0.
            for delta in &deltas { expected.extend(delta.to_be_bytes()); }
            if rotation {
                rotated += 1;
                expected.extend(entry["decoded"]["angles"].as_array().unwrap().iter()
                    .map(|value| value.as_u64().unwrap() as u8));
            }
            assert_eq!(move_payload(MID, NEW, payload, rotation).unwrap(), expected);
            assert_eq!(move_payload(MID, OLD, payload, rotation).unwrap(), payload);
            assert_eq!(move_payload(MID, MID, payload, rotation).unwrap(), payload);
            for (version, bytes) in [(OLD, payload), (MID, payload), (NEW, expected.as_slice())] {
                assert_eq!(move_payload(version, MID, bytes, rotation).unwrap(), payload);
                for length in 0..bytes.len() { assert!(move_payload(version, MID, &bytes[..length], rotation).is_err()); }
                let mut extra = bytes.to_vec(); extra.push(0);
                assert_eq!(move_payload(version, MID, &extra, rotation), Err(Error::Invalid));
            }
        }
        for (actual, expected) in position.iter().zip(command["expected_position"].as_array().unwrap()) {
            assert_eq!(*actual, expected.as_f64().unwrap());
        }
    }
    assert_eq!(rotated, 3);
}

#[test]
fn movement_776_refuses_step_timing_loss_and_invalid_ground() {
    let timed = Delta { on_ground: true, motion: Motion::Stepped(vec![
        crate::movement::Step { tick_offset: 1, delta: [1, 2, 3] },
    ]) };
    assert_eq!(timed.encode(MID), Err(crate::movement::Error::RequiresTimedDelivery));
    let mut packet = vec![2]; packet.extend(timed.encode(NEW).unwrap());
    assert_eq!(move_payload(NEW, MID, &packet, false),
        Err(Error::Movement(crate::movement::Error::RequiresTimedDelivery)));
    let mut bad = vec![0; 7]; bad[6] = 2;
    assert_eq!(Delta::decode_prefix(MID, &bad), Err(crate::movement::Error::Invalid));
    let mut entity_bad = vec![2]; entity_bad.extend(bad);
    assert_eq!(move_payload(MID, NEW, &entity_bad, false), Err(Error::Invalid));
    assert_eq!(move_payload(V1_12_2, MID, &[], false), Err(Error::UnsupportedVersion(V1_12_2)));
    assert_eq!(move_payload(MID, V1_12_2, &[], false), Err(Error::UnsupportedVersion(V1_12_2)));
}

#[test]
fn short_776_ack_matches_pinned_schema_and_own_probe_without_inventing_pose() {
    // Синтетические ID для границ; НЕ запись исходящего настоящего GUI ACK.
    for bytes in [&[0][..], &[1], &[0x80, 1], &[0xff, 0xff, 0xff, 0xff, 0x0f]] {
        let confirmation = Confirmation::decode(MID, bytes).unwrap();
        assert_eq!(confirmation.pose, None);
        assert_eq!(confirmation.encode(MID).unwrap(), bytes);
        assert_eq!(confirmation.encode(OLD).unwrap(), bytes);
        assert_eq!(confirmation.encode(NEW), Err(crate::teleport::Error::MissingPose));
        for length in 0..bytes.len() { assert!(Confirmation::decode(MID, &bytes[..length]).is_err()); }
        let mut extra = bytes.to_vec(); extra.push(0);
        assert_eq!(Confirmation::decode(MID, &extra), Err(crate::teleport::Error::Invalid));
    }
    let confirmation = Confirmation { id: 1, pose: Some(Pose {
        x: 0.0, y: 80.0, z: 0.0, yaw: 0.0, pitch: 0.0,
    }) };
    assert_eq!(confirmation.encode(MID), Err(crate::teleport::Error::UnexpectedPose));
    assert_eq!(Confirmation::decode(MID, &confirmation.encode(NEW).unwrap()), Err(crate::teleport::Error::Invalid));
    for bytes in [&[0xff; 6][..], &[0xff, 0xff, 0xff, 0xff, 0x1f]] {
        assert_eq!(Confirmation::decode(MID, bytes), Err(crate::teleport::Error::Invalid));
    }
}

#[test]
fn low_level_776_payloads_do_not_open_full_chunk_wire_tracker_or_admission() {
    let context = crate::chunk::Context { min_y: -64, sections: 24, shared_biome_count: 59 };
    for (from, to) in [(OLD, MID), (MID, OLD), (NEW, MID), (MID, NEW), (MID, MID)] {
        assert_eq!(crate::chunk::translate(from, to, &[], context),
            Err(crate::chunk::Error::UnsupportedVersion(MID)));
        for state in [State::Login, State::Configuration, State::Play] {
            for direction in [Direction::Clientbound, Direction::Serverbound] {
                assert_eq!(translate(state, direction, from, to, &[0]), Err(Error::UnsupportedVersion(MID)));
            }
        }
        let mut tracker = crate::entities::Tracker::default();
        assert_eq!(tracker.translate(from, to, &[0], Context::default()), Err(Error::UnsupportedVersion(MID)));
        assert_eq!(tracker.translate_packets(from, to, &[0], Context::default()), Err(Error::UnsupportedVersion(MID)));
    }
    assert_eq!(crate::Negotiated::native_only(776, OLD),
        Err(crate::AdmissionError::AdapterUnavailable { client: MID, native: OLD }));
}
