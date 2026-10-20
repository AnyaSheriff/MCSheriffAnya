use super::*;
use JavaVersion::{V1_12_2, V26_1_2 as OLD, V26_2 as MID, V26_3 as NEW};

fn report() -> serde_json::Value {
    serde_json::from_str(include_str!("../metadata-776-report.json")).unwrap()
}
fn hex(text: &str) -> Vec<u8> {
    text.as_bytes().as_chunks::<2>().0.iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap()).collect()
}
fn meta(from: JavaVersion, to: JavaVersion, bytes: &[u8], kind: u32) -> Result<Vec<u8>, wire::Error> {
    let mut input = Cursor::new(bytes);
    let mut out = Vec::new();
    metadata(&mut input, &mut out, from, to, wire::Context {
        entity_kind: Some(kind), ..Default::default()
    })?;
    if input.remaining() != 0 { return Err(wire::Error::Invalid); }
    Ok(out)
}

#[test]
fn observed_776_cow_slime_magma_item_metadata_keeps_values_and_exact_size_indices() {
    let data = report();
    for entity in data["observed_entities"].as_array().unwrap() {
        let layout = data["layouts"].as_array().unwrap().iter().find(|row| row["name"] == entity["type"]).unwrap();
        let kind = layout["native_id"].as_u64().unwrap() as u32;
        assert_eq!(metadata_layout(MID, kind).unwrap().len(), layout["fields"].as_array().unwrap().len());
        for packet in entity["packets"].as_array().unwrap() {
            let body = hex(packet["body"].as_str().unwrap());
            assert_eq!(body[0], 0x63);
            let payload = &body[1..];
            let mut old = payload.to_vec();
            if matches!(entity["type"].as_str(), Some("slime" | "magma_cube")) {
                let tail = old.len() - 4;
                assert_eq!(&old[tail..tail + 2], &[18, 1]);
                old[tail] = 16;
            }
            assert_eq!(meta(MID, OLD, payload, kind).unwrap(), old);
            assert_eq!(meta(MID, MID, payload, kind).unwrap(), payload);
            assert_eq!(meta(MID, NEW, payload, kind).unwrap(), payload);
            assert_eq!(meta(OLD, MID, &old, kind).unwrap(), payload);
            assert_eq!(meta(NEW, MID, payload, kind).unwrap(), payload);
            for length in 0..payload.len() { assert!(meta(MID, OLD, &payload[..length], kind).is_err()); }
            let mut tail = payload.to_vec(); tail.push(0);
            assert_eq!(meta(MID, OLD, &tail, kind), Err(wire::Error::Invalid));
        }
    }
}

#[test]
fn middle_metadata_layout_is_explicit_and_added_age_fields_are_not_dropped() {
    for kind in [80, 117] {
        assert_eq!(metadata_layout(OLD, kind).unwrap()[16], "size");
        assert_eq!(metadata_layout(MID, kind).unwrap()[16..19], ["baby", "age_locked", "size"]);
        assert_eq!(metadata_layout(NEW, kind).unwrap()[16..19], ["baby", "age_locked", "size"]);
        for (index, field) in [(16, "baby"), (17, "age_locked")] {
            let bytes = [2, index, 8, 1, 255];
            assert_eq!(meta(MID, NEW, &bytes, kind).unwrap(), bytes);
            assert_eq!(meta(NEW, MID, &bytes, kind).unwrap(), bytes);
            assert_eq!(meta(MID, OLD, &bytes, kind), Err(failure(Error::MissingField(field))));
        }
    }
    assert!(matches!(metadata_layout(MID, 0), Err(wire::Error::PayloadUnavailable(_))));
    assert_eq!(metadata_layout(MID, u32::MAX), Err(failure(Error::UnknownKind(u32::MAX))));
    assert_eq!(metadata_layout(V1_12_2, 30), Err(wire::Error::UnsupportedVersion(V1_12_2)));
}

#[test]
fn player_layout_776_uses_native_kind_and_synthetic_first_play_fields() {
    // Только pinned layout + наш775 writer, НЕ отдельная запись GUI/official776.
    let kind = entity_layouts::PLAYER;
    assert_eq!(kind, 155);
    assert_eq!(ids::translate(RegistryKind::Entity, MID, OLD, 156), Ok(kind));
    assert_eq!(metadata_layout(MID, kind).unwrap(), metadata_layout(OLD, kind).unwrap());
    let mut body = vec![1, 0, 0, 2, 6, 20, 0, 9, 3];
    body.extend(20.0f32.to_be_bytes());
    body.extend([10, 17, 0, 15, 42, 1, 16, 0, 127, 17, 3]);
    body.extend(0.0f32.to_be_bytes());
    body.extend([18, 1, 10, 19, 19, 0, 20, 19, 0, 255]);
    for version in [OLD, MID, NEW] {
        assert_eq!(meta(MID, version, &body, kind).unwrap(), body);
        assert_eq!(meta(version, MID, &body, kind).unwrap(), body);
    }
    assert_eq!(meta(MID, NEW, &[1, 15, 0, 1, 255], kind), Err(failure(Error::Serializer(0))));
    assert_eq!(meta(MID, NEW, &[1, 15, 42, 2, 255], kind), Err(wire::Error::TooLarge));
}

#[test]
fn bounded_776_metadata_refuses_unknown_duplicate_and_complex_payloads() {
    let cow = 30;
    assert_eq!(meta(MID, NEW, &[2, 254, 0, 0, 255], cow), Err(failure(Error::UnknownField(254))));
    assert_eq!(meta(MID, NEW, &[2, 9, 7, 0, 255], cow), Err(failure(Error::Serializer(7))));
    assert_eq!(meta(MID, NEW, &[2, 9, 44, 255], cow), Err(failure(Error::Serializer(44))));
    assert_eq!(meta(MID, NEW, &[2, 5, 8, 1, 5, 8, 1, 255], cow), Err(wire::Error::Invalid));
    assert_eq!(meta(MID, NEW, &[2, 5, 8, 2, 255], cow), Err(wire::Error::Invalid));
    let mut invalid_float = vec![2, 9, 3]; invalid_float.extend(f32::NAN.to_be_bytes()); invalid_float.push(255);
    assert_eq!(meta(MID, NEW, &invalid_float, cow), Err(wire::Error::Invalid));
    assert_eq!(meta(MID, NEW, &[2, 10, 17, 0, 255], cow).unwrap(), [2, 10, 17, 0, 255]);
    assert_eq!(meta(MID, NEW, &[2, 10, 17, 1, 0, 255], cow),
        Err(wire::Error::PayloadUnavailable("unverified 26.2 metadata particles")));
    // Динамический вариант не проходит без согласованного caller registry context.
    assert_eq!(meta(MID, NEW, &[2, 18, 23, 0, 255], cow),
        Err(failure(Error::MissingRegistry("minecraft:cow_variant"))));
}

#[test]
fn layouts_do_not_open_public_wire_or_tracking_for_776() {
    for (from, to) in [(OLD, MID), (MID, OLD), (NEW, MID), (MID, NEW), (MID, MID)] {
        let mut tracker = Tracker::default();
        let body = [0x63, 2, 5, 8, 1, 255];
        assert_eq!(tracker.translate(from, to, &body, wire::Context::default()),
            Err(wire::Error::UnsupportedVersion(MID)));
        assert_eq!(tracker.translate_packets(from, to, &body, wire::Context::default()),
            Err(wire::Error::UnsupportedVersion(MID)));
        assert!(tracker.entities.is_empty());
        assert_eq!(wire::translate(State::Play, Direction::Clientbound, from, to, &body),
            Err(wire::Error::UnsupportedVersion(MID)));
    }
}
