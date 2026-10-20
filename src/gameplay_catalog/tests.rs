use super::*;
use JavaVersion::{V26_1_2 as OLD, V26_2 as MIDDLE, V26_3 as NEW};

const OLD_CATALOG: Catalog = Catalog::new(Profile::Java26_1_2);
const PREVIEW: Catalog = Catalog::new(Profile::Preview26_3);

#[test]
fn reservations_follow_frozen_base_and_fit_existing_storage() {
    let old_item = crate::blocks_table::ITEM_NAMES.iter().map(|(id, _)| *id).max().unwrap();
    let old_block = crate::blocks_table::BLOCK_IDS.iter().map(|(_, id)| *id).max().unwrap();
    let old_state = crate::blocks_table::BLOCK_NAMES.iter().map(|(_, high, _)| *high).max().unwrap();
    assert_eq!((old_item, old_block, old_state), (1505, 1167, 29872));
    assert_eq!(WHITE_SLAB_ITEM, old_item + 1);
    assert_eq!(WHITE_SLAB_BLOCK, old_block + 1);
    assert_eq!(WHITE_SLAB_FIRST_STATE, old_state + 1);
    assert!(old_item + 1 < 1 << 24); // Stack::stored_item сохраняет24младших бита.
    assert!(u16::try_from(WHITE_SLAB_LAST_STATE).is_ok()); // World::Packed.
    assert_eq!(crate::blocks::item_named(WHITE_SLAB_NAME), None);
    assert_eq!(crate::blocks::state_by_name(WHITE_SLAB_NAME), None);
    assert_eq!(crate::world::profile::current_profile().gameplay_version, "26.1.2");
}

#[test]
fn preview_does_not_change_any_base_identity_or_existing_mapping() {
    let mut rows = Vec::new();
    rows.extend(crate::blocks_table::ITEM_NAMES.iter().map(|(id, name)| (Kind::Item, *id, *name)));
    rows.extend(crate::blocks_table::BLOCK_IDS.iter().map(|(name, id)| (Kind::Block, *id, *name)));
    for &(low, high, name) in crate::blocks_table::BLOCK_NAMES {
        rows.extend((low..=high).map(|id| (Kind::State, id, name)));
    }
    for (kind, id, name) in rows {
        for catalog in [OLD_CATALOG, PREVIEW] {
            assert_eq!(catalog.name(kind, id), Ok(name));
            assert_eq!(catalog.to_client(kind, id, OLD), Ok(id as u32));
            for client in [MIDDLE, NEW] {
                let expected = ids::translate(kind.registry(), OLD, client, id as u32).map_err(Error::Wire);
                if client == NEW { assert!(expected.is_ok(), "базовый объект отсутствует в777: {kind:?} {name} {id}"); }
                assert_eq!(catalog.to_client(kind, id, client), expected);
            }
        }
    }
}

#[test]
fn server_profile_not_client_number_controls_new_content() {
    assert!(matches!(OLD_CATALOG.item_named(WHITE_SLAB_NAME), Err(Error::Disabled { .. })));
    assert!(matches!(OLD_CATALOG.default_state(WHITE_SLAB_NAME), Err(Error::Disabled { .. })));
    for (kind, internal, wire) in [(Kind::Item, WHITE_SLAB_ITEM, WHITE_SLAB_WIRE_ITEM),
        (Kind::Block, WHITE_SLAB_BLOCK, WHITE_SLAB_WIRE_BLOCK),
        (Kind::State, WHITE_SLAB_DEFAULT_STATE, WHITE_SLAB_WIRE_FIRST_STATE + 3)] {
        assert!(matches!(OLD_CATALOG.name(kind, internal), Err(Error::Disabled { .. })));
        assert!(matches!(OLD_CATALOG.to_client(kind, internal, NEW), Err(Error::Disabled { .. })));
        assert!(matches!(OLD_CATALOG.decode_client_id(kind, wire, NEW), Err(Error::Disabled { .. })));
        assert_eq!(PREVIEW.to_client(kind, internal, NEW), Ok(wire));
        assert_eq!(PREVIEW.decode_client_id(kind, wire, NEW), Ok(internal));
        for unsupported in [OLD, MIDDLE, JavaVersion::V1_12_2] {
            assert!(matches!(PREVIEW.to_client(kind, internal, unsupported), Err(Error::Unrepresentable { .. })));
        }
    }
    // 721 в775 — иной старый предмет, не наш append-only item1506.
    assert_eq!(PREVIEW.decode_client_id(Kind::Item, WHITE_SLAB_WIRE_ITEM, OLD), Ok(WHITE_SLAB_WIRE_ITEM as i32));
    assert_ne!(WHITE_SLAB_WIRE_ITEM as i32, WHITE_SLAB_ITEM);
}

#[test]
fn every_slab_property_pair_has_exact_selected_candidate_state() {
    let selected: serde_json::Value = serde_json::from_str(include_str!("white-concrete-slab.json")).unwrap();
    assert_eq!(selected["item"]["id"], WHITE_SLAB_WIRE_ITEM);
    assert_eq!(selected["block"]["id"], WHITE_SLAB_WIRE_BLOCK);
    assert_eq!(selected["block"]["minStateId"], WHITE_SLAB_WIRE_FIRST_STATE);
    assert_eq!(selected["block"]["maxStateId"], WHITE_SLAB_WIRE_FIRST_STATE + 5);
    assert_eq!(selected["block"]["defaultState"], WHITE_SLAB_WIRE_FIRST_STATE + 3);
    assert_eq!(selected["block"]["states"][0]["values"], serde_json::json!(["top", "bottom", "double"]));
    assert_eq!(selected["block"]["states"][1]["name"], "waterlogged");
    assert_eq!(selected["block"]["states"][1]["num_values"], 2);
    for (half_index, half) in [SlabHalf::Top, SlabHalf::Bottom, SlabHalf::Double].into_iter().enumerate() {
        for waterlogged in [true, false] {
            let offset = half_index as u32 * 2 + u32::from(!waterlogged);
            let state = PREVIEW.slab_state(half, waterlogged).unwrap();
            assert_eq!(state, WHITE_SLAB_FIRST_STATE + offset as i32);
            assert_eq!(PREVIEW.slab_properties(state), Ok(Some((half, waterlogged))));
            assert_eq!(PREVIEW.to_client(Kind::State, state, NEW), Ok(WHITE_SLAB_WIRE_FIRST_STATE + offset));
            assert_eq!(PREVIEW.decode_client_id(Kind::State, WHITE_SLAB_WIRE_FIRST_STATE + offset, NEW), Ok(state));
            assert!(matches!(OLD_CATALOG.slab_properties(state), Err(Error::Disabled { .. })));
        }
    }
    assert_eq!(PREVIEW.default_state(WHITE_SLAB_NAME), Ok(Some(PREVIEW.slab_state(SlabHalf::Bottom, false).unwrap())));
    assert_eq!(PREVIEW.stack_size(WHITE_SLAB_ITEM), Ok(64));
    assert_eq!(selected["recipe"], serde_json::Value::Null); // Нельзя выдумывать рецепт из названия.
}

#[test]
fn unknown_ids_never_inherit_default_stack_size_or_wrap_into_valid_state() {
    for catalog in [OLD_CATALOG, PREVIEW] {
        for id in [-1, i32::MIN, i32::MAX, WHITE_SLAB_ITEM + 1] {
            assert!(matches!(catalog.stack_size(id), Err(Error::UnknownInternal { .. })));
        }
        for id in [-1, i32::MIN, i32::MAX, WHITE_SLAB_LAST_STATE + 1, 65535] {
            assert!(matches!(catalog.name(Kind::State, id), Err(Error::UnknownInternal { .. })));
        }
        assert!(catalog.decode_client_id(Kind::Item, u32::MAX, NEW).is_err());
        assert!(catalog.decode_client_id(Kind::State, u32::MAX, NEW).is_err());
        assert_eq!(catalog.item_named("not_a_real_item"), Ok(None));
    }
    assert!(matches!(OLD_CATALOG.stack_size(WHITE_SLAB_ITEM), Err(Error::Disabled { .. })));
}

#[test]
fn preview_fingerprint_never_relabels_or_changes_native_profile() {
    let before = crate::world::profile::current_profile();
    assert_eq!(OLD_CATALOG.registry_id(), before.registry_id);
    assert_ne!(PREVIEW.registry_id(), OLD_CATALOG.registry_id());
    let preview = PREVIEW.registry_id();
    let hex = preview.strip_prefix("mcsa-catalog-v1:sha256:").unwrap();
    assert_eq!(hex.len(), 64);
    assert!(hex.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert_eq!(Catalog::new(Profile::Preview26_3).registry_id(), preview);
    assert_eq!(crate::world::profile::current_profile(), before);
}
