//! Ступени бетона и шерсти 26.3: идентичности, 80 состояний, добыча, рецепты.
use super::*;
use crate::inventory::Stack;

const PREVIEW: Catalog = Catalog::new(Profile::Preview26_3);
const NATIVE: Catalog = Catalog::new(Profile::Java26_1_2);

fn selected() -> serde_json::Value { serde_json::from_str(include_str!("stairs-26.3.json")).unwrap() }

#[test]
fn identities_follow_slabs_and_match_selected_json() {
    let json = selected();
    let rows: Vec<_> = stairs().collect();
    assert_eq!(rows.len(), 32);
    assert_eq!(CONCRETE_STAIRS[0].item, WOOL_SLABS[15].item + 1);
    assert_eq!(CONCRETE_STAIRS[0].block, WOOL_SLABS[15].block + 1);
    assert_eq!(CONCRETE_STAIRS[0].first_state, WOOL_SLABS[15].last_state + 1);
    for (index, row) in rows.iter().enumerate() {
        let entry = &json["entries"][index];
        assert_eq!(entry["name"], row.name);
        assert_eq!(entry["item777"].as_u64(), Some(u64::from(row.wire_item)));
        let block = &entry["block777"];
        assert_eq!(block["id"].as_u64(), Some(u64::from(row.wire_block)));
        assert_eq!(block["minStateId"].as_u64(), Some(u64::from(row.wire_first_state)));
        assert_eq!(block["maxStateId"].as_u64(), Some(u64::from(row.wire_first_state) + 79));
        assert_eq!(row.last_state - row.first_state, 79);
        assert_eq!(block["defaultState"].as_u64().unwrap() - block["minStateId"].as_u64().unwrap(), (row.default_state - row.first_state) as u64);
        let (hardness, material) = match row.family { SlabFamily::Concrete => (1.8, "mineable/pickaxe"), SlabFamily::Wool => (0.8, "wool"), SlabFamily::Mineral | SlabFamily::Poplar => unreachable!("сера и тополь проверяются в своих тестах") };
        assert_eq!(block["hardness"], hardness);
        assert_eq!(block["resistance"], hardness);
        assert_eq!(block["material"], material);
        if index > 0 { assert_eq!(row.first_state, rows[index - 1].last_state + 1); }
    }
}

#[test]
fn all_2560_states_have_properties_mining_drops_wire_and_native_refusal() {
    let water = crate::blocks::blast_resistance(crate::blocks::state_by_name("water").unwrap());
    let pickaxe = crate::blocks::item_named("diamond_pickaxe");
    let mut count = 0;
    for row in stairs() {
        for state in row.first_state..=row.last_state {
            count += 1;
            let stair = PREVIEW.stair_properties(state).unwrap().unwrap();
            assert_eq!(PREVIEW.stair_state_for(row, stair), Ok(state));
            let rule = PREVIEW.orientation_of(state).unwrap().unwrap();
            assert_eq!(rule.family, crate::blocks::Family::Stairs);
            assert_eq!(rule.value(state, "facing"), Some(stair.facing_name()));
            assert_eq!(rule.value(state, "half"), Some(if stair.top { "top" } else { "bottom" }));
            assert_eq!(rule.value(state, "shape"), Some(stair.shape_name()));
            assert_eq!(rule.value(state, "waterlogged"), Some(if stair.waterlogged { "true" } else { "false" }));
            assert_eq!(PREVIEW.waterlogged(state), Ok(Some(stair.waterlogged)));
            assert_eq!(PREVIEW.slab_entry(state), Ok(None));
            let wet = stair.waterlogged;
            let (hardness, own) = match row.family {
                SlabFamily::Concrete => (1.8, "mineable/pickaxe"),
                SlabFamily::Wool => (0.8, "wool"),
                SlabFamily::Mineral | SlabFamily::Poplar => unreachable!("сера и тополь проверяются в своих тестах"),
            };
            let (h, tool, _) = PREVIEW.mining_properties(state).unwrap();
            assert_eq!((h, tool), (hardness, own));
            assert_eq!(PREVIEW.blast_resistance(state), Ok(if wet { water.max(hardness) } else { hardness }));
            let hand = PREVIEW.drops_by_luck(state, None, 0.5).unwrap();
            let tooled = PREVIEW.drops_by_luck(state, pickaxe, 0.5).unwrap();
            assert_eq!(tooled, Some(Stack::new(row.item, 1)));
            assert_eq!(hand, if row.family == SlabFamily::Wool { Some(Stack::new(row.item, 1)) } else { None });
            assert!(PREVIEW.explosion_drops(state, 0.0).unwrap().is_some() || row.family == SlabFamily::Concrete);
            let wire = PREVIEW.to_client(Kind::State, state, JavaVersion::V26_3).unwrap();
            assert_eq!(wire, row.wire_first_state + (state - row.first_state) as u32);
            assert_eq!(PREVIEW.decode_client_id(Kind::State, wire, JavaVersion::V26_3), Ok(state));
            assert!(PREVIEW.to_client(Kind::State, state, JavaVersion::V26_1_2).is_err());
            assert!(NATIVE.orientation_of(state).is_err());
            assert!(NATIVE.mining_properties(state).is_err());
            assert!(NATIVE.drops_by_luck(state, None, 0.5).is_err());
        }
        assert_eq!(PREVIEW.to_client(Kind::Item, row.item, JavaVersion::V26_3), Ok(row.wire_item));
        assert_eq!(PREVIEW.to_client(Kind::Block, row.block, JavaVersion::V26_3), Ok(row.wire_block));
        assert_eq!(PREVIEW.item_named(row.name), Ok(Some(row.item)));
        assert_eq!(PREVIEW.block_for_item(row.item), Ok(Some((row.name, row.default_state))));
        assert_eq!(PREVIEW.stack_size(row.item), Ok(64));
        assert!(NATIVE.item_named(row.name).is_err());
        let text = format!("minecraft:{}[facing=east,half=top,shape=inner_left,waterlogged=true]", row.name);
        let parsed = PREVIEW.state_from_text(&text).unwrap().unwrap();
        assert_eq!(PREVIEW.stair_properties(parsed), Ok(Some(StairState { facing: 3, top: true, shape: 1, waterlogged: true })));
        // Состояние по умолчанию: north, bottom, straight, не налитая вода.
        assert_eq!(PREVIEW.stair_properties(row.default_state), Ok(Some(StairState { facing: 0, top: false, shape: 0, waterlogged: false })));
    }
    assert_eq!(count, 2560);
}

#[test]
fn manual_crafting_gives_four_in_both_mirrors_and_refuses_wrong_shapes() {
    let grid = |cells: [Option<i32>; 9]| cells.map(|cell| cell.map(|item| Stack::new(item, 1)));
    for (index, row) in stairs().enumerate() {
        let (x, other) = (Some(row.input_item), Some(stairs().nth((index + 1) % 16 + 16 * (index / 16)).unwrap().input_item));
        let left = grid([x, None, None, x, x, None, x, x, x]);
        let right = grid([None, None, x, None, x, x, x, x, x]);
        for slots in [left, right] {
            assert_eq!(crate::crafting::craft_for_catalog(PREVIEW, &slots), Some(Stack::new(row.item, 4)), "{}", row.name);
            let id = crate::crafting::recipe_identity_for_catalog(PREVIEW, &slots).unwrap();
            let recipe = crate::crafting::recipe_for_catalog(PREVIEW, id).unwrap();
            assert_eq!((recipe.result, recipe.count), (row.item, 4));
            assert_eq!(crate::crafting::craft_for_catalog(NATIVE, &slots), None);
        }
        assert_eq!(crate::crafting::craft_for_catalog(PREVIEW, &grid([x, None, None, x, other, None, x, x, x])), None);
        assert_eq!(crate::crafting::craft_for_catalog(PREVIEW, &grid([x, x, x, x, x, x, None, None, None])), None);
        assert_eq!(crate::crafting::craft_for_catalog(PREVIEW, &grid([x, None, None, x, x, None, x, x, None])), None);
    }
}

#[test]
fn only_concrete_stairs_have_one_to_one_stonecutting() {
    for row in stairs() {
        let choices = crate::stonecutter::choices_for_catalog(PREVIEW, row.input_item);
        let own = choices.iter().find(|recipe| recipe.output == row.item);
        assert_eq!(own.map(|recipe| recipe.count), (row.family == SlabFamily::Concrete).then_some(1), "{}", row.name);
        assert!(crate::stonecutter::choices_for_catalog(NATIVE, row.input_item).iter().all(|recipe| recipe.output != row.item));
    }
}

#[test]
fn stair_collision_boxes_cover_expected_volume() {
    let volume = |state: StairState| -> f64 {
        state.boxes().iter().map(|(from, to)| (to[0] - from[0]) * (to[1] - from[1]) * (to[2] - from[2])).sum()
    };
    for facing in 0..4 {
        for top in [false, true] {
            // Нижний уровень — полблока; ступень сверху: половина, четверть или три четверти от верхнего.
            for (shape, expected) in [(0, 0.75), (1, 0.875), (2, 0.875), (3, 0.625), (4, 0.625)] {
                assert_eq!(volume(StairState { facing, top, shape, waterlogged: false }), expected);
            }
        }
    }
    // Нижние ступени на север: ступенька у северной стороны, на высоте 0.5..1.
    let north = StairState { facing: 0, top: false, shape: 0, waterlogged: false }.boxes();
    assert_eq!(north[1], ([0.0, 0.5, 0.0], [1.0, 1.0, 0.5]));
}
