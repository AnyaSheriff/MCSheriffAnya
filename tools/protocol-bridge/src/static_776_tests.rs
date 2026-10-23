use crate::{Direction, JavaVersion::{V1_12_2, V26_1_2 as OLD, V26_2 as MID, V26_3 as NEW},
    State, generated_static_776, ids::{self, IdTranslationError as Error, RegistryKind as Kind}};

const KINDS: &[Kind] = &[Kind::Item, Kind::Block, Kind::BlockState, Kind::Entity, Kind::Component, Kind::Particle];

#[test]
fn every_direct_776_mapping_has_a_real_source_and_exact_inverse() {
    for &kind in KINDS {
        for (from, to) in [(OLD, MID), (MID, OLD), (MID, NEW), (NEW, MID)] {
            let count = generated_static_776::source_count(kind, from).unwrap();
            for id in 0..count {
                match ids::translate(kind, from, to, id) {
                    Ok(target) => assert_eq!(ids::translate(kind, to, from, target), Ok(id)),
                    Err(Error::MissingTarget(source)) => assert_eq!(source, id),
                    other => panic!("Известный исходный ID{kind:?}/{from:?}/{id}: {other:?}"),
                }
            }
            for id in [count, 65535, u32::MAX] {
                assert_eq!(ids::translate(kind, from, to, id), Err(Error::UnknownId(id)));
            }
        }
        let count = generated_static_776::source_count(kind, MID).unwrap();
        for id in 0..count { assert_eq!(ids::translate(kind, MID, MID, id), Ok(id)); }
        assert_eq!(ids::translate(kind, MID, MID, count), Err(Error::UnknownId(count)));
        assert_eq!(ids::translate(kind, V1_12_2, MID, 0), Err(Error::UnsupportedVersion(V1_12_2)));
        assert_eq!(ids::translate(kind, MID, V1_12_2, 0), Err(Error::UnsupportedVersion(V1_12_2)));
    }
}

#[test]
fn direct_776_777_preserves_shared_new_objects_without_a_775_detour() {
    // Имена/ID — закреплённые JSON, exact examples в static-776-report.json.
    for (kind, middle, modern, name) in [
        (Kind::Block, 998, 1088, "sulfur"),
        (Kind::BlockState, 25926, 29283, "cinnabar"),
        (Kind::Item, 1052, 1140, "sulfur_cube_bucket"),
        (Kind::Item, 1342, 1463, "music_disc_bounce"),
        (Kind::Entity, 130, 133, "sulfur_cube"),
        (Kind::Component, 78, 80, "sulfur_cube_content"),
        (Kind::Particle, 124, 127, "sulfur_cube_goo"),
        (Kind::Particle, 115, 118, "trial_spawner_detection"),
    ] {
        assert_eq!(ids::translate(kind, MID, NEW, middle), Ok(modern), "{name}");
        assert_eq!(ids::translate(kind, NEW, MID, modern), Ok(middle), "{name}");
        assert_eq!(ids::translate(kind, MID, OLD, middle), Err(Error::MissingTarget(middle)), "{name}");
    }
}

#[test]
fn old_components_shared_with_776_do_not_disappear_through_777() {
    for (id, name) in [(40, "swing_animation"), (45, "map_color")] {
        assert_eq!(ids::translate(Kind::Component, OLD, MID, id), Ok(id), "{name}");
        assert_eq!(ids::translate(Kind::Component, MID, OLD, id), Ok(id), "{name}");
        assert_eq!(ids::translate(Kind::Component, MID, NEW, id), Err(Error::MissingTarget(id)), "{name}");
    }
    // Похожие названия частиц не являются доказанным соответствием.
    assert_eq!(ids::translate(Kind::Particle, OLD, MID, 108), Err(Error::MissingTarget(108)));
}

#[test]
fn slot_and_template_776_validate_untrusted_lengths_and_unknown_components() {
    use crate::items::{self, Context, Stack};
    let ctx = Context::default();
    assert_eq!(items::decode(MID, &[0], false, ctx), Ok((None, 1)));
    assert_eq!(items::encode(MID, None, false, ctx), Ok(vec![0]));
    assert_eq!(items::decode(MID, &[0], true, ctx), Ok((None, 1)));
    assert_eq!(items::encode(MID, None, true, ctx), Ok(vec![0]));
    let stack = Stack { item: 1, count: 1, added: vec![], removed: vec![] };
    assert_eq!(items::decode_template(MID, &[1, 1, 0, 0], ctx), Ok((stack.clone(), 4)));
    assert_eq!(items::encode_template(MID, &stack, ctx), Ok(vec![1, 1, 0, 0]));
    for (from, to) in [(MID, OLD), (OLD, MID), (MID, NEW), (NEW, MID)] {
        let mut input = items::Reader(&[]);
        // damage_type ещё требует отдельного typed codec.
        assert_eq!(items::component_bytes(8, &mut input, from, to, ctx),
            Err(items::Error::UnsupportedComponent(8)));
    }
}

#[test]
fn static_tables_do_not_enable_chunk_entity_wire_or_admission_routes() {
    use crate::{chunk, legacy_blocks, legacy_chunk, wire};
    let context = chunk::Context { min_y: -64, sections: 24, shared_biome_count: 1 };
    for (from, to) in [(MID, OLD), (OLD, MID), (MID, NEW), (NEW, MID), (MID, MID)] {
        assert_eq!(chunk::translate(from, to, &[], context), Err(chunk::Error::UnsupportedVersion(MID)));
        for state in [State::Login, State::Configuration, State::Play] {
            for direction in [Direction::Clientbound, Direction::Serverbound] {
                assert_eq!(wire::translate(state, direction, from, to, &[0]), Err(wire::Error::UnsupportedVersion(MID)));
            }
        }
        assert!(crate::entities::Tracker::default().translate(from, to, &[0], wire::Context::default()).is_err());
    }
    let error = legacy_blocks::Error::Id(Error::UnsupportedVersion(MID));
    assert_eq!(legacy_blocks::to_modern(0, MID), Err(error.clone()));
    assert_eq!(legacy_blocks::to_legacy(0, MID), Err(error.clone()));
    let view = legacy_blocks::View { version: MID, x: 0, z: 0, min_y: 0,
        window: legacy_chunk::HeightWindow::new(0).unwrap(), sections: &[],
        biomes: [0; 256], block_entities: vec![] };
    assert_eq!(legacy_blocks::project(view), Err(error));
    for &kind in crate::generated_ids::ALL {
        if KINDS.contains(&kind) || kind == Kind::Attribute { continue; }
        for (from, to) in [(MID, OLD), (OLD, MID), (MID, NEW), (NEW, MID), (MID, MID)] {
            assert_eq!(ids::translate(kind, from, to, 0), Err(Error::UnsupportedVersion(MID)));
        }
    }
    assert!(crate::Negotiated::native_only(776, OLD).is_err());
    assert!(crate::registry::Registry::decode(MID, &[]).is_err());
    assert!(crate::light::Light::decode(MID, &[]).is_err());
    // Короткий ACK уже проверяется отдельным codec; общий wire остаётся закрыт.
    assert_eq!(crate::teleport::Confirmation::decode(MID, &[0]),
        Ok(crate::teleport::Confirmation { id: 0, pose: None }));
    assert!(crate::teleport::Confirmation::decode(MID, &[]).is_err());
    assert!(crate::chunk_sections::decode(MID, &[], 1, 1).is_err());
}
