//! Расширение по закреплённым схемам, не новая запись official776.
use crate::{Direction, JavaVersion, State, entities::Tracker, items, wire::{self, Context}};
use JavaVersion::{V26_1_2 as OLD, V26_2 as MID};

fn hex(value: &serde_json::Value) -> Vec<u8> {
    value.as_str().unwrap().as_bytes().as_chunks::<2>().0.iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap()).collect()
}
fn cb(from: JavaVersion, to: JavaVersion, body: &[u8], context: Context) -> Result<Vec<u8>, wire::Error> {
    wire::translate_inner(State::Play, Direction::Clientbound, from, to, body, context, true).map(|b| b.into_owned())
}

#[test]
fn typed_776_complex_slots_preserve_54_existing_cases_and_untrusted_lengths() {
    let data: serde_json::Value = serde_json::from_str(include_str!("../testdata/complex-item-observations.json")).unwrap();
    let cases = data["old"]["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 54);
    for case in cases {
        let raw = hex(&case["slot_hex"]);
        let (stack, size) = items::decode(OLD, &raw, false, items::Context::default()).unwrap();
        assert_eq!(size, raw.len());
        for untrusted in [false, true] {
            let encoded = items::encode(MID, stack.as_ref(), untrusted, items::Context::default()).unwrap();
            assert_eq!(items::decode(MID, &encoded, untrusted, items::Context::default()).unwrap(), (stack.clone(), encoded.len()), "{}", case["name"]);
            for end in 0..encoded.len() {
                assert!(items::decode(MID, &encoded[..end], untrusted, items::Context::default()).is_err(), "{} prefix{end}", case["name"]);
            }
        }
        assert_eq!(items::encode(OLD, stack.as_ref(), false, items::Context::default()).unwrap(), raw);
    }
    // Damage7: длина относится к компоненту, а не ко всему Slot.
    let correct = [1, 1, 1, 0, 3, 1, 7];
    assert_eq!(items::decode(MID, &correct, true, items::Context::default()).unwrap().1, correct.len());
    for raw in [&[1, 1, 1, 0, 3, 0][..], &[1, 1, 1, 0, 3, 2, 7], &[1, 1, 1, 0, 3, 2, 7, 0]] {
        assert!(items::decode(MID, raw, true, items::Context::default()).is_err());
    }
}

#[test]
fn typed_776_enchanted_named_equipment_preserves_all_ten_existing_samples() {
    let data: serde_json::Value = serde_json::from_str(include_str!("../testdata/item-observations.json")).unwrap();
    let cases = data.as_array().unwrap().iter().find(|r| r["protocol"] == 775).unwrap()["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 10);
    let context = items::Context { shared_enchantment_count: Some(43), ..Default::default() };
    for case in cases {
        let (stack, _) = items::decode(OLD, &hex(&case["slot_hex"]), false, context).unwrap();
        for untrusted in [false, true] {
            let encoded = items::encode(MID, stack.as_ref(), untrusted, context).unwrap();
            assert_eq!(items::decode(MID, &encoded, untrusted, context).unwrap(), (stack.clone(), encoded.len()));
        }
    }
}

#[test]
fn typed_776_world_particles_keep_old_envelope_and_all_23_existing_effects() {
    let data: serde_json::Value = serde_json::from_str(include_str!("../testdata/particle-observations.json")).unwrap();
    let cases = data["old"]["particles"].as_array().unwrap();
    assert_eq!(cases.len(), 23);
    for case in cases {
        let source = hex(&case["body"]);
        let encoded = cb(OLD, MID, &source, Context::default()).unwrap();
        assert_eq!(cb(MID, OLD, &encoded, Context::default()).unwrap(), source, "{}", case["case"]);
        for end in 0..encoded.len() { assert!(cb(MID, OLD, &encoded[..end], Context::default()).is_err()); }
        let mut extra = encoded; extra.push(0);
        assert!(cb(MID, OLD, &extra, Context::default()).is_err());
    }
}

#[test]
fn pinned_776_layouts_and_eleven_recorded_entity_payloads_preserve_fields() {
    assert_eq!(crate::generated_payload_776::LAYOUTS.len(), 158);
    for &(kind, _) in crate::generated_payload_776::LAYOUTS {
        // Общий shared_flags; проверка индексов всех раскладок, не поведение моба.
        let body = [0x63, 2, 0, 0, 3, 255];
        let context = Context { entity_kind: Some(kind), ..Default::default() };
        assert_eq!(cb(OLD, MID, &body, context).unwrap(), body);
        assert_eq!(cb(MID, OLD, &body, context).unwrap(), body);
    }
    let data: serde_json::Value = serde_json::from_str(include_str!("../testdata/entity-observations.json")).unwrap();
    let cases = data["old"]["entities"].as_array().unwrap();
    assert_eq!(cases.len(), 11);
    let context = Context { metadata_registry_counts: &[("minecraft:painting_variant", 51), ("minecraft:wolf_variant", 9)], ..Default::default() };
    for case in cases {
        let mut tracker = Tracker::default();
        let kind = crate::entity_layouts::LAYOUTS.iter().position(|r| Some(r.0) == case["type"].as_str()).unwrap() as u32;
        for packet in case["packets"].as_array().unwrap() {
            if !matches!(packet["name"].as_str(), Some("spawn_entity" | "entity_metadata")) { continue; }
            let source = hex(&packet["body"]);
            let encoded = crate::probe_776::clientbound(&mut tracker, &source, context).unwrap();
            let context = Context { entity_kind: Some(kind), ..context };
            assert_eq!(cb(MID, OLD, &encoded, context).unwrap(), source, "{}", case["type"]);
        }
    }
}

#[test]
fn schema_776_particle_metadata_preserves_nonempty_lists() {
    let data: serde_json::Value = serde_json::from_str(include_str!("../testdata/particle-observations.json")).unwrap();
    for entry in data["old"]["metadata"].as_array().unwrap() {
        let kind = crate::entity_layouts::LAYOUTS.iter().position(|r| Some(r.0) == entry["type"].as_str()).unwrap() as u32;
        let context = Context { entity_kind: Some(kind), ..Default::default() };
        for packet in entry["packets"].as_array().unwrap().iter().filter(|p| p["name"] == "entity_metadata") {
            let source = hex(&packet["body"]);
            let middle = cb(OLD, MID, &source, context).unwrap();
            assert_eq!(cb(MID, OLD, &middle, context).unwrap(), source);
        }
    }
}

#[test]
fn primary_777_extensions_keep_sulfur_cube_and_project_cushion_metadata_for_776() {
    let context = Context { items: items::Context {
        content: crate::content::ContentCodec::white_slab_preview(), ..Default::default()
    }, ..Default::default() };
    for canonical in [157, 158, 159, 160] {
        let mut source = vec![1, 2]; source.extend([0; 16]);
        wire::uint(&mut source, canonical);
        for coordinate in [0.5f64, 65.0, 0.5] { source.extend(coordinate.to_be_bytes()); }
        source.extend([0, 0, 0, 0, 0]);
        let mut tracker = Tracker::default();
        let spawn = crate::probe_776::clientbound(&mut tracker, &source, context).unwrap();
        let mut input = wire::Cursor::new(&spawn);
        assert_eq!(input.uint().unwrap(), 1); assert_eq!(input.uint().unwrap(), 2);
        input.take(16).unwrap();
        let expected = context.items.content.packet_id(crate::ids::RegistryKind::Entity, OLD, MID, canonical).unwrap();
        assert_eq!(input.uint().unwrap(), expected);
        let source: &[u8] = match canonical {
            159 => &[0x63, 2, 8, 1, 14, 255],
            160 => &[0x63, 2, 18, 1, 2, 255],
            _ => &[0x63, 2, 0, 0, 1, 255],
        };
        let metadata = crate::probe_776::clientbound(&mut tracker, source, context).unwrap();
        assert_eq!(metadata.as_ref(), if canonical == 159 { &[0x63, 2, 255] } else { source });
    }
}
