//! Соломенная кровать, подушки, карты строений и пластинка Bounce 26.3
//! (наблюдение `probe_furnish_777.py`, данные — tools/protocol-bridge/testdata/furnish-observations-777.json).
use super::*;
use crate::blocks;
use crate::inventory::Stack;

const PREVIEW: Catalog = Catalog::new(Profile::Preview26_3);
const COLORS: [&str; 16] = ["white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray", "light_gray", "cyan", "purple", "blue", "brown", "green", "red", "black"];

fn observed() -> serde_json::Value {
    serde_json::from_str(include_str!("../../tools/protocol-bridge/testdata/furnish-observations-777.json")).unwrap()
}

#[test]
fn ids_continue_after_poplar_boats_and_match_the_pinned_777_tables() {
    let boats_end = POPLAR.iter().filter(|row| row.form == BlockForm::Boat).map(|row| row.item).max().unwrap();
    let rows: Vec<_> = POPLAR.iter().filter(|row| row.item > boats_end).collect();
    // кровать + 16 подушек + 16 карт + пластинка + ведро и яйцо куба серы
    assert_eq!(rows.len(), 36);
    for (offset, row) in rows.iter().enumerate() { assert_eq!(row.item, boats_end + 1 + offset as i32, "{}", row.name); }
    let straw = rows[0];
    assert_eq!((straw.name, straw.form), ("straw_bed", BlockForm::Wood));
    assert_eq!(straw.last_state - straw.first_state, 15);
    // Те же свойства, что у белой кровати; по умолчанию facing=north, occupied=false, part=foot.
    assert_eq!(blocks::state_by_name("straw_bed"), Some(straw.default_state));
    let state = straw.default_state;
    let rule = blocks::orientation_of(state).unwrap();
    assert_eq!((rule.value(state, "part"), rule.value(state, "occupied"), rule.value(state, "facing")), (Some("foot"), Some("false"), Some("north")));
    // Сверка с pinned JSON 777 (если лежит на машине).
    let candidate = std::path::Path::new(env!("HOME")).join(".local/share/mcsa-java-26.3-probe/candidate-json");
    let (Ok(blocks_json), Ok(items_json)) = (std::fs::read_to_string(candidate.join("blocks.json")), std::fs::read_to_string(candidate.join("items.json"))) else { return; };
    let blocks_json: Vec<serde_json::Value> = serde_json::from_str(&blocks_json).unwrap();
    let items_json: Vec<serde_json::Value> = serde_json::from_str(&items_json).unwrap();
    let block = blocks_json.iter().find(|b| b["name"] == "straw_bed").unwrap();
    assert_eq!((block["id"].as_u64().unwrap() as u32, block["minStateId"].as_u64().unwrap() as u32), (straw.wire_block, straw.wire_first_state));
    assert_eq!((block["defaultState"].as_i64().unwrap() as i32 - straw.wire_first_state as i32), straw.default_state - straw.first_state);
    for row in &rows {
        let item = items_json.iter().find(|i| i["name"] == row.name).unwrap();
        assert_eq!(item["id"].as_u64().unwrap() as u32, row.wire_item, "{}", row.name);
        assert_eq!(PREVIEW.item_named(row.name), Ok(Some(row.item)));
    }
}

#[test]
fn stacks_and_recipes_match_the_official_server() {
    let observed = observed();
    for (name, sizes) in observed["stacks"].as_object().unwrap() {
        let item = blocks::item_named(name).unwrap_or_else(|| panic!("нет предмета {name}"));
        let want = sizes.as_array().unwrap()[0].as_i64().unwrap() as i32;
        // Пластинка 1, кровать и подушки 16, карты 64; у сервера «1» для пластинки — все строки по одному.
        assert_eq!(blocks::stack_size(item), want, "{name}");
    }
    let mut checked = 0;
    for entry in observed["crafting"].as_array().unwrap() {
        let result = entry["result"].as_str().unwrap();
        let ours = crate::crafting::poplar_recipes().iter().find(|e| blocks::item_name(e.recipe.result) == Some(result)).unwrap_or_else(|| panic!("нет рецепта {result}"));
        assert_eq!(ours.recipe.count, entry["count"].as_i64().unwrap() as i32, "{result}");
        assert_eq!((ours.recipe.width, ours.recipe.height), (entry["width"].as_u64().unwrap() as u8, entry["height"].as_u64().unwrap() as u8), "{result}");
        assert_eq!(ours.category as u64, entry["category"].as_u64().unwrap(), "{result}");
        let cells: Vec<&str> = entry["cells"].as_array().unwrap().iter().map(|c| c.as_str().unwrap()).collect();
        assert_eq!(cells.len(), ours.recipe.cells.len());
        for (cell, want) in ours.recipe.cells.iter().zip(&cells) {
            assert_eq!(cell.iter().map(|id| PREVIEW.name(Kind::Item, *id).unwrap()).collect::<Vec<_>>(), vec![*want], "{result}");
        }
        checked += 1;
    }
    assert_eq!(checked, 17);
}

#[test]
fn straw_bed_drops_itself_and_is_crafted_from_hay() {
    let straw = blocks::item_named("straw_bed").unwrap();
    let head = blocks::state_from_text("straw_bed[part=head,facing=south]").unwrap();
    let drops = crate::mining::block_drops_with_catalog(PREVIEW, head, None, || 0.0);
    assert_eq!(drops.iter().map(|s| (s.item, s.count)).collect::<Vec<_>>(), vec![(straw, 1)]);
    assert!(crate::bed::is_straw(head) && crate::bed::is_bed(head));
    assert!(!crate::bed::is_straw(blocks::state_by_name("red_bed").unwrap()));
    // Не подпрыгивает (вики, Straw Bed), в отличие от обычной.
    assert_eq!(crate::survival::landing_multiplier("straw_bed"), 1.0);
    assert_eq!(crate::survival::landing_multiplier("red_bed"), 0.5);
    let hay = blocks::item_named("hay_block").unwrap();
    let grid = [Some(Stack::new(hay, 1)), Some(Stack::new(hay, 1)), Some(Stack::new(hay, 1)), None, None, None, None, None, None];
    let result = crate::crafting::craft_for_catalog(PREVIEW, &grid).expect("рецепт");
    assert_eq!((result.item, result.count), (straw, 4));
}

#[test]
fn cushions_carry_a_colour_break_into_the_item_and_need_support() {
    use crate::mob::{cushion, Kind as MobKind, Mob};
    for (index, color) in COLORS.iter().enumerate() {
        assert_eq!(cushion::color_of_item(&format!("{color}_cushion")), Some(index as u8));
        assert_eq!(cushion::item_name(index as u8), format!("{color}_cushion"));
    }
    assert_eq!(MobKind::from_name("cushion"), Some(MobKind::Cushion));
    let mut mob = Mob::new(5, MobKind::Cushion, (0.5, 64.0, 0.5));
    mob.color = 14;
    assert_eq!(mob.size(), (0.999, 0.249));
    // Любой удар ломает сразу, предмет — по цвету.
    assert!(mob.hurt(1.0, crate::survival::Source::PlayerAttack, None).is_some_and(|hit| hit.died));
    assert!(mob.is_dead() && mob.gone());
    let loot = crate::mob::loot(&mob, &mut crate::mob::Rng::new(1));
    assert_eq!(loot.iter().map(|s| (blocks::item_name(s.item), s.count)).collect::<Vec<_>>(), vec![(Some("red_cushion"), 1)]);
}

#[test]
fn structure_maps_are_items_that_copy_but_do_not_zoom() {
    use crate::maps::{Maps, STRUCTURE_MAPS};
    assert_eq!(STRUCTURE_MAPS.len(), 16);
    for row in STRUCTURE_MAPS { assert!(blocks::item_named(row.0).is_some(), "{}", row.0); assert!(crate::maps::is_structure_map(row.0)); }
    let mut maps = Maps::default();
    let id = maps.create(crate::dimension::Dimension::Overworld, 0, 0, 2);
    maps.maps.get_mut(&id).unwrap().destination = Some((100, 100, 9));
    let data = crate::stackdata::Data { map_id: Some(id), ..Default::default() };
    let map = Stack::new(blocks::item_named("ocean_monument_map").unwrap(), 1).with_extras(data);
    let paper = Stack::new(blocks::item_named("paper").unwrap(), 1);
    let empty = Stack::new(blocks::item_named("map").unwrap(), 1);
    assert!(maps.cartography(map, paper, false).is_none(), "увеличение запрещено");
    let copy = maps.cartography(map, empty, false).expect("копия");
    assert_eq!((copy.item, copy.count), (map.item, 2));
    // Верстак: карта + пустая карта → две карты того же предмета.
    let crafted = crate::crafting::craft_for_catalog(PREVIEW, &[Some(map), Some(empty), None, None, None, None, None, None, None]).expect("копирование");
    assert_eq!((crafted.item, crafted.count), (map.item, 2));
}

#[test]
fn bounce_plays_in_a_jukebox_for_3_54_with_the_registry_number_after_the_old_songs() {
    let bounce = blocks::item_named("music_disc_bounce").unwrap();
    let (number, ticks, signal) = crate::redstone::jukebox_song(bounce).expect("песня");
    assert_eq!((ticks, signal), (234 * 20, 7));
    let old = crate::network::configuration::REQUIRED_REGISTRIES.iter().find(|(r, _)| *r == "minecraft:jukebox_song").unwrap().1.len();
    assert_eq!(number as usize, old);
}
