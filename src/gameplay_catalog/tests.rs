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
        for id in [-1, i32::MIN, i32::MAX, POPLAR.last().unwrap().item + 1] {
            assert!(matches!(catalog.stack_size(id), Err(Error::UnknownInternal { .. })));
        }
        for id in [-1, i32::MIN, i32::MAX, POPLAR.last().unwrap().last_state + 1, 65535] {
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

#[test]
fn runtime_resolvers_preserve_profile_boundary_and_validate_properties() {
    assert_eq!(PREVIEW.block_for_item(WHITE_SLAB_ITEM), Ok(Some((WHITE_SLAB_NAME, WHITE_SLAB_DEFAULT_STATE))));
    assert_eq!(PREVIEW.item_for_block(WHITE_SLAB_NAME), Ok(Some(WHITE_SLAB_ITEM)));
    assert_eq!(PREVIEW.block_id(WHITE_SLAB_NAME), Ok(Some(WHITE_SLAB_BLOCK)));
    assert_eq!(PREVIEW.states_of(WHITE_SLAB_NAME), Ok(Some((WHITE_SLAB_FIRST_STATE, WHITE_SLAB_LAST_STATE))));
    for text in ["white_concrete_slab", "minecraft:white_concrete_slab[type=bottom,waterlogged=false]"] {
        assert_eq!(PREVIEW.state_from_text(text), Ok(Some(WHITE_SLAB_DEFAULT_STATE)));
        assert!(matches!(OLD_CATALOG.state_from_text(text), Err(Error::Disabled { .. })));
    }
    for text in ["white_concrete_slab[type=side]", "white_concrete_slab[type=top,type=bottom]",
        "white_concrete_slab[unknown=true]", "white_concrete_slab[type=top"] {
        assert_eq!(PREVIEW.state_from_text(text), Ok(None));
    }
    for state in WHITE_SLAB_FIRST_STATE..=WHITE_SLAB_LAST_STATE {
        let orientation = PREVIEW.orientation_of(state).unwrap().unwrap();
        assert_eq!(orientation.family, crate::blocks::Family::Slab);
        let (half, wet) = PREVIEW.slab_properties(state).unwrap().unwrap();
        assert_eq!(orientation.value(state, "waterlogged"), Some(if wet { "true" } else { "false" }));
        assert_eq!(orientation.value(state, "type"), Some(match half { SlabHalf::Top => "top", SlabHalf::Bottom => "bottom", SlabHalf::Double => "double" }));
        assert!(matches!(OLD_CATALOG.orientation_of(state), Err(Error::Disabled { .. })));
        assert!(matches!(OLD_CATALOG.mining_properties(state), Err(Error::Disabled { .. })));
        assert!(matches!(OLD_CATALOG.light_properties(state), Err(Error::Disabled { .. })));
    }
}

#[test]
fn runtime_mining_metadata_uses_pinned_values_and_internal_tool_identity() {
    let selected: serde_json::Value = serde_json::from_str(include_str!("white-concrete-slab.json")).unwrap();
    let (hardness, material, tools) = PREVIEW.mining_properties(WHITE_SLAB_DEFAULT_STATE).unwrap();
    assert_eq!(f64::from(hardness), f64::from(selected["block"]["hardness"].as_f64().unwrap() as f32));
    assert_eq!(material, selected["block"]["material"].as_str().unwrap());
    assert_eq!(PREVIEW.blast_resistance(WHITE_SLAB_DEFAULT_STATE), Ok(1.8));
    let mut translated = selected["block"]["harvestTools"].as_object().unwrap().keys()
        .map(|id| PREVIEW.decode_client_id(Kind::Item, id.parse().unwrap(), NEW).unwrap()).collect::<Vec<_>>();
    translated.sort_unstable();
    assert_eq!(tools, translated);
    for state in WHITE_SLAB_FIRST_STATE..=WHITE_SLAB_LAST_STATE {
        let (emit, filter, _) = PREVIEW.light_properties(state).unwrap();
        assert_eq!((emit, filter), (0, 0));
        assert_eq!(PREVIEW.drops_by_luck(state, None, 0.5), Ok(None));
        assert_eq!(PREVIEW.drops_by_luck(state, crate::blocks::item_named("iron_shovel"), 0.5), Ok(None));
        for &tool in tools {
            let half = PREVIEW.slab_properties(state).unwrap().unwrap().0;
            assert_eq!(PREVIEW.drops_by_luck(state, Some(tool), 0.5),
                Ok(Some(crate::inventory::Stack::new(WHITE_SLAB_ITEM, if half == SlabHalf::Double { 2 } else { 1 }))));
        }
    }
    let stone = crate::blocks::state_by_name("stone").unwrap();
    assert_eq!(PREVIEW.mining_properties(stone).unwrap(), crate::mining::properties(stone));
    assert_eq!(PREVIEW.drops_by_luck(stone, Some(tools[0]), 0.5).unwrap(), crate::blocks::drops_by_luck(stone, Some(tools[0]), 0.5));
    assert!(PREVIEW.light_properties(-1).is_err());
    assert!(PREVIEW.block_for_item(-1).is_err());
}

#[test]
fn concrete_family_selected_metadata_and_all_96_states_stay_distinct() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!("concrete-slabs-26.3.json")).unwrap();
    let entries = fixture["entries"].as_array().unwrap();
    assert_eq!(entries.len(), CONCRETE_SLABS.len());
    let mut states = std::collections::HashSet::new();
    for (index, row) in CONCRETE_SLABS.iter().enumerate() {
        let selected = &entries[index];
        assert_eq!(selected["name"], row.name);
        assert_eq!(selected["block777"]["id"], row.wire_block);
        assert_eq!(selected["item777"]["id"], row.wire_item);
        assert_eq!(selected["block777"]["minStateId"], row.wire_first_state);
        assert_eq!(selected["block777"]["maxStateId"], row.wire_first_state + 5);
        assert_eq!(selected["block777"]["defaultState"], row.wire_first_state + 3);
        assert_eq!(row.default_state, row.first_state + 3);
        assert_eq!(row.last_state, row.first_state + 5);
        assert_eq!(row.item, WHITE_SLAB_ITEM + index as i32);
        assert_eq!(row.block, WHITE_SLAB_BLOCK + index as i32);
        assert_eq!(row.first_state, WHITE_SLAB_FIRST_STATE + 6 * index as i32);
        assert_eq!(selected["block777"]["hardness"], 1.8);
        assert_eq!(selected["block777"]["resistance"], 1.8);
        assert_eq!(selected["block777"]["material"], "mineable/pickaxe");
        assert_eq!(selected["item777"]["stackSize"], 64);
        assert_eq!(selected["concrete_input"]["item775"], row.input_item);
        assert_eq!(crate::blocks::item_named(row.input_name), Some(row.input_item));
        assert_eq!(PREVIEW.item_named(row.name), Ok(Some(row.item)));
        assert_eq!(PREVIEW.block_for_item(row.item), Ok(Some((row.name,row.default_state))));
        assert_eq!(PREVIEW.item_for_block(row.name), Ok(Some(row.item)));
        assert_eq!(PREVIEW.block_id(row.name), Ok(Some(row.block)));
        for (kind,id,wire) in [(Kind::Item,row.item,row.wire_item),(Kind::Block,row.block,row.wire_block)] {
            assert_eq!(PREVIEW.to_client(kind,id,NEW),Ok(wire));
            assert_eq!(PREVIEW.decode_client_id(kind,wire,NEW),Ok(id));
            assert!(matches!(OLD_CATALOG.name(kind,id),Err(Error::Disabled { .. })));
            assert!(PREVIEW.to_client(kind,id,OLD).is_err());
        }
        for half in [SlabHalf::Top,SlabHalf::Bottom,SlabHalf::Double] {
            for wet in [true,false] {
                let state = PREVIEW.slab_state_for(row,half,wet).unwrap();
                assert!(states.insert(state));
                assert_eq!(PREVIEW.slab_entry(state),Ok(Some(row)));
                assert_eq!(PREVIEW.slab_properties(state),Ok(Some((half,wet))));
                let wire = row.wire_first_state + (state-row.first_state) as u32;
                assert_eq!(PREVIEW.to_client(Kind::State,state,NEW),Ok(wire));
                assert_eq!(PREVIEW.decode_client_id(Kind::State,wire,NEW),Ok(state));
                assert!(OLD_CATALOG.slab_entry(state).is_err());
                assert!(OLD_CATALOG.decode_client_id(Kind::State,wire,NEW).is_err());
                assert!(PREVIEW.to_client(Kind::State,state,MIDDLE).is_err());
                let orientation = PREVIEW.orientation_of(state).unwrap().unwrap();
                assert_eq!(orientation.name,row.name);
                assert_eq!(orientation.state(&[("type",orientation.value(state,"type").unwrap()),("waterlogged",if wet {"true"} else {"false"})]),Some(state));
                let tool = crate::blocks::item_named("diamond_pickaxe");
                assert_eq!(PREVIEW.drops_by_luck(state,tool,0.0),Ok(Some(crate::inventory::Stack::new(row.item,if half==SlabHalf::Double {2}else{1}))));
            }
        }
    }
    assert_eq!(states.len(),96);
    assert_eq!(PREVIEW.slab_entry(WHITE_SLAB_FIRST_STATE-1),Ok(None));
    assert!(PREVIEW.slab_entry(POPLAR.last().unwrap().last_state+1).is_err());
    assert_eq!(PREVIEW.slab_entry(WOOL_SLABS.last().unwrap().last_state+1),Ok(None), "дальше идут ступени, не плиты");
}

#[test]
fn wool_metadata_allocates_only_append_ids_and_maps_all_96_selected_states() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!("wool-slabs-26.3.json")).unwrap();
    assert_eq!(fixture["candidate_commit"], "924e26d6b3bb8f97a5d1e73d6190b09879c2e1e2");
    let selected = fixture["entries"].as_array().unwrap();
    assert_eq!(selected.len(), 16);
    assert_eq!(slabs().count(), 32);
    let mut states = std::collections::HashSet::new();
    for (index, row) in WOOL_SLABS.iter().enumerate() {
        let entry = &selected[index];
        assert_eq!(row.family, SlabFamily::Wool);
        assert_eq!(entry["name"], row.name);
        assert_eq!((row.item, row.block, row.first_state, row.last_state, row.default_state),
            (1522 + index as i32, 1184 + index as i32, 29969 + 6 * index as i32, 29974 + 6 * index as i32, 29972 + 6 * index as i32));
        assert_eq!(entry["canonical"]["item"], row.item);
        assert_eq!(entry["canonical"]["block"], row.block);
        assert_eq!(entry["canonical"]["minStateId"], row.first_state);
        assert_eq!(entry["canonical"]["maxStateId"], row.last_state);
        assert_eq!(entry["canonical"]["defaultState"], row.default_state);
        assert_eq!(entry["item777"]["id"], row.wire_item);
        assert_eq!(entry["item777"]["stackSize"], 64);
        assert_eq!(entry["block777"]["id"], row.wire_block);
        assert_eq!(entry["block777"]["minStateId"], row.wire_first_state);
        assert_eq!(entry["block777"]["maxStateId"], row.wire_first_state + 5);
        assert_eq!(entry["block777"]["defaultState"], row.wire_first_state + 3);
        assert_eq!(entry["block777"]["hardness"], 0.8);
        assert_eq!(entry["block777"]["resistance"], 0.8);
        assert_eq!(entry["block777"]["material"], "wool");
        assert_eq!(entry["block777"]["emitLight"], 0);
        assert_eq!(entry["block777"]["filterLight"], 0);
        assert_eq!(entry["block777"]["harvestTools"], serde_json::Value::Null);
        assert_eq!(entry["wool_input"]["item775"], row.input_item);
        assert_eq!(crate::blocks::item_named(row.input_name), Some(row.input_item));
        assert_eq!(ids::translate(RegistryKind::Item, OLD, NEW, row.input_item as u32).unwrap(), entry["wool_input"]["item777"].as_u64().unwrap() as u32);
        assert_eq!(PREVIEW.item_named(row.name), Ok(Some(row.item)));
        assert_eq!(PREVIEW.default_state(row.name), Ok(Some(row.default_state)));
        assert_eq!(PREVIEW.stack_size(row.item), Ok(64));
        assert!(matches!(OLD_CATALOG.item_named(row.name), Err(Error::Disabled { .. })));
        for (kind, id, wire) in [(Kind::Item, row.item, row.wire_item), (Kind::Block, row.block, row.wire_block)] {
            assert_eq!(PREVIEW.name(kind, id), Ok(row.name));
            assert_eq!(PREVIEW.to_client(kind, id, NEW), Ok(wire));
            assert_eq!(PREVIEW.decode_client_id(kind, wire, NEW), Ok(id));
            assert!(matches!(OLD_CATALOG.decode_client_id(kind, wire, NEW), Err(Error::Disabled { .. })));
            for version in [OLD, MIDDLE, JavaVersion::V1_12_2] { assert!(PREVIEW.to_client(kind, id, version).is_err()); }
        }
        for (half_index, half) in [SlabHalf::Top, SlabHalf::Bottom, SlabHalf::Double].into_iter().enumerate() {
            for wet in [true, false] {
                let offset = 2 * half_index + usize::from(!wet);
                let state = PREVIEW.slab_state_for(row, half, wet).unwrap();
                let wire = row.wire_first_state + offset as u32;
                assert_eq!(state, row.first_state + offset as i32);
                assert!(states.insert(state));
                assert_eq!(entry["enumerated_states777"][offset]["id"], wire);
                assert_eq!(entry["enumerated_states777"][offset]["properties"]["type"], ["top", "bottom", "double"][half_index]);
                assert_eq!(entry["enumerated_states777"][offset]["properties"]["waterlogged"], if wet { "true" } else { "false" });
                assert_eq!(PREVIEW.slab_entry(state), Ok(Some(row)));
                assert_eq!(PREVIEW.slab_properties(state), Ok(Some((half, wet))));
                assert_eq!(PREVIEW.to_client(Kind::State, state, NEW), Ok(wire));
                assert_eq!(PREVIEW.decode_client_id(Kind::State, wire, NEW), Ok(state));
                assert!(matches!(OLD_CATALOG.slab_entry(state), Err(Error::Disabled { .. })));
                assert!(OLD_CATALOG.decode_client_id(Kind::State, wire, NEW).is_err());
                assert!(PREVIEW.to_client(Kind::State, state, OLD).is_err());
            }
        }
    }
    assert_eq!(states.len(), 96);
    assert_eq!(WOOL_SLABS[0].item, CONCRETE_SLABS[15].item + 1);
    assert_eq!(WOOL_SLABS[0].block, CONCRETE_SLABS[15].block + 1);
    assert_eq!(WOOL_SLABS[0].first_state, CONCRETE_SLABS[15].last_state + 1);
    assert_eq!(PREVIEW.slab_entry(WOOL_SLABS[0].first_state - 1), Ok(Some(&CONCRETE_SLABS[15])));
    assert_eq!(PREVIEW.slab_entry(WOOL_SLABS[15].last_state + 1), Ok(None));
    assert_eq!(crate::world::profile::current_profile().gameplay_version, "26.1.2");
}

#[test]
fn shared_descriptor_keeps_concrete_identity_and_wool_changes_only_preview_fingerprint() {
    let concrete: Vec<_> = slabs().take(16).collect();
    assert_eq!(concrete, CONCRETE_SLABS.iter().collect::<Vec<_>>());
    for (index, row) in CONCRETE_SLABS.iter().enumerate() {
        assert_eq!(row.family, SlabFamily::Concrete);
        assert_eq!((row.item, row.block, row.first_state, row.last_state, row.default_state),
            (1506 + index as i32, 1168 + index as i32, 29873 + 6 * index as i32, 29878 + 6 * index as i32, 29876 + 6 * index as i32));
    }
    // Замороженный алгоритм прежнего preview с одним concrete-семейством.
    use aws_lc_rs::digest;
    let base = crate::world::profile::block_item_registry_id();
    let mut old = digest::Context::new(&digest::SHA256);
    for value in ["mcsa-catalog-v1", base, "type:top,bottom,double", "waterlogged:true,false"] {
        old.update(&(value.len() as u64).to_be_bytes()); old.update(value.as_bytes());
    }
    for row in &CONCRETE_SLABS {
        old.update(&(row.name.len() as u64).to_be_bytes()); old.update(row.name.as_bytes());
        for id in [row.item, row.block, row.first_state, row.last_state, row.default_state] { old.update(&id.to_be_bytes()); }
    }
    let hex: String = old.finish().as_ref().iter().map(|byte| format!("{byte:02x}")).collect();
    assert_ne!(PREVIEW.registry_id(), format!("mcsa-catalog-v1:sha256:{hex}"));
    assert_eq!(OLD_CATALOG.registry_id(), base);
    assert_eq!(crate::world::profile::current_profile().registry_id, base);
    let mut forged = WOOL_SLABS[0]; forged.family = SlabFamily::Concrete;
    assert!(PREVIEW.slab_state_for(&forged, SlabHalf::Bottom, false).is_err());
}
