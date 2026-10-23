//! Тополь 26.3 и сверка серы/киновари с официальным сервером (наблюдение `probe_poplar_777.py`,
//! данные — tools/protocol-bridge/testdata/poplar-observations-777.json).
use super::*;
use crate::blocks;
use crate::inventory::Stack;

const PREVIEW: Catalog = Catalog::new(Profile::Preview26_3);
const NATIVE: Catalog = Catalog::new(Profile::Java26_1_2);

fn observed() -> serde_json::Value {
    serde_json::from_str(include_str!("../../tools/protocol-bridge/testdata/poplar-observations-777.json")).unwrap()
}

fn item(name: &str) -> i32 { PREVIEW.item_named(name).unwrap().unwrap_or_else(|| panic!("нет предмета {name}")) }
fn iname(id: i32) -> &'static str { PREVIEW.name(Kind::Item, id).unwrap() }

#[test]
fn ids_are_appended_after_minerals_and_match_the_pinned_777_tables() {
    // 23 блока дерева + полочный гриб, красный куст и две лодки (plants_tests ниже).
    assert_eq!(POPLAR.len(), 63); // 27 + соломенная кровать, 16 подушек, 16 карт, пластинка (furnish_tests)
    let last = &MINERALS[MINERALS.len() - 1];
    assert_eq!(POPLAR[0].block, last.block + 1);
    assert_eq!(POPLAR[0].first_state, last.last_state + 1);
    let mut next_item = last.item + 1;
    for (index, row) in POPLAR.iter().enumerate().filter(|(_, row)| !matches!(row.form, BlockForm::Plant | BlockForm::Boat | BlockForm::Item) && row.name != "straw_bed") {
        assert_eq!(row.family, SlabFamily::Poplar);
        if index > 0 { assert_eq!((row.block, row.first_state), (POPLAR[index - 1].block + 1, POPLAR[index - 1].last_state + 1)); }
        if row.has_item() { assert_eq!(row.item, next_item); next_item += 1; } else { assert_eq!(row.item, -1); }
        assert_eq!(PREVIEW.item_named(row.name), Ok(row.has_item().then_some(row.item)));
        assert!(NATIVE.item_named(row.name).is_err());
        assert!(NATIVE.name(Kind::State, row.first_state).is_err());
        // Свойства и число состояний совпадают с дубовым двойником.
        let (first, last) = blocks::states_of(row.input_name).unwrap();
        assert_eq!(last - first, row.last_state - row.first_state, "{}", row.name);
        assert_eq!(blocks::state_by_name(row.name), Some(row.default_state));
        assert_eq!(row.default_state - row.first_state, blocks::state_by_name(row.input_name).unwrap() - first, "{}", row.name);
    }
    let candidate = std::path::Path::new(env!("HOME")).join(".local/share/mcsa-java-26.3-probe/candidate-json");
    let (Ok(blocks_json), Ok(items_json)) = (std::fs::read_to_string(candidate.join("blocks.json")), std::fs::read_to_string(candidate.join("items.json"))) else { return; };
    let blocks_json: Vec<serde_json::Value> = serde_json::from_str(&blocks_json).unwrap();
    let items_json: Vec<serde_json::Value> = serde_json::from_str(&items_json).unwrap();
    for row in POPLAR.iter().filter(|row| row.has_block()) {
        let block = blocks_json.iter().find(|b| b["name"] == row.name).unwrap();
        assert_eq!(block["id"].as_u64(), Some(u64::from(row.wire_block)), "{}", row.name);
        assert_eq!(block["minStateId"].as_u64(), Some(u64::from(row.wire_first_state)), "{}", row.name);
        assert_eq!(block["maxStateId"].as_u64(), Some(u64::from(row.wire_first_state + row.state_count() - 1)), "{}", row.name);
        assert_eq!(block["defaultState"].as_u64().unwrap() - block["minStateId"].as_u64().unwrap(), (row.default_state - row.first_state) as u64, "{}", row.name);
        if row.has_item() {
            assert_eq!(items_json.iter().find(|i| i["name"] == row.name).unwrap()["id"].as_u64(), Some(u64::from(row.wire_item)));
        }
    }
    for row in POPLAR.iter().filter(|row| row.form == BlockForm::Boat) {
        assert_eq!(items_json.iter().find(|i| i["name"] == row.name).unwrap()["id"].as_u64(), Some(u64::from(row.wire_item)));
    }
}

#[test]
fn every_state_translates_to_the_wire_and_back_only_in_the_preview_profile() {
    for row in &POPLAR {
        for state in row.first_state..=row.last_state {
            let wire = PREVIEW.to_client(Kind::State, state, JavaVersion::V26_3).unwrap();
            assert_eq!(wire, row.wire_first_state + (state - row.first_state) as u32);
            assert_eq!(PREVIEW.decode_client_id(Kind::State, wire, JavaVersion::V26_3), Ok(state));
            assert!(PREVIEW.to_client(Kind::State, state, JavaVersion::V26_1_2).is_err());
            assert!(NATIVE.mining_properties(state).is_err());
            assert!(NATIVE.drops_by_luck(state, None, 0.5).is_err());
        }
        if row.has_item() {
            assert_eq!(PREVIEW.to_client(Kind::Item, row.item, JavaVersion::V26_3), Ok(row.wire_item));
            assert_eq!(PREVIEW.decode_client_id(Kind::Item, row.wire_item, JavaVersion::V26_3), Ok(row.item));
        }
    }
}

#[test]
fn poplar_behaves_like_its_oak_twin_in_every_table() {
    let pickaxe = item("diamond_pickaxe");
    let axe = item("diamond_axe");
    for row in POPLAR.iter().filter(|row| !matches!(row.form, BlockForm::Plant | BlockForm::Boat | BlockForm::Item)) {
        for state in row.first_state..=row.last_state {
            let twin = crate::poplar::twin_state(state);
            assert_eq!(PREVIEW.mining_properties(state), Ok(crate::mining::properties(twin)), "{}", row.name);
            assert_eq!(blocks::conductive(state), blocks::conductive(twin), "{}", row.name);
            assert_eq!(blocks::full_cube(state), blocks::full_cube(twin), "{}", row.name);
            assert_eq!(blocks::has_box(state), blocks::has_box(twin), "{}", row.name);
            assert_eq!(blocks::blast_resistance(state), blocks::blast_resistance(twin), "{}", row.name);
            let (emit, filter, _) = PREVIEW.light_properties(state).unwrap();
            let (twin_emit, twin_filter, _) = PREVIEW.light_properties(twin).unwrap_or((emit, filter, 0));
            assert_eq!((emit, filter), (twin_emit, twin_filter), "{}", row.name);
            // Годный инструмент даёт то же, что дубовый двойник, только предмет — тополя.
            for tool in [None, Some(pickaxe), Some(axe)] {
                let ours = blocks::drops_by_luck(state, tool, 0.5);
                let theirs = blocks::drops_by_luck(twin, tool, 0.5);
                assert_eq!(ours.map(|stack| stack.count), theirs.map(|stack| stack.count), "{}", row.name);
            }
        }
    }
}

fn state_for(label: &str) -> i32 {
    let spec = match label {
        l if l.ends_with("_double") => format!("{}[type=double]", l.trim_end_matches("_double")),
        "poplar_door" => "poplar_door[half=lower]".to_owned(),
        "poplar_door_upper" => "poplar_door[half=upper]".to_owned(),
        l => l.to_owned(),
    };
    PREVIEW.state_from_text(&spec).unwrap().unwrap_or_else(|| panic!("нет состояния {spec}"))
}

#[test]
fn drops_match_the_official_loot_tables() {
    let observed = observed();
    for (label, expected) in observed["drops"].as_object().unwrap() {
        // Верхняя половина двери предмета не даёт: пару разбирает обработчик двери (прежний путь дубовых).
        if label == "poplar_door_upper" { continue; }
        let state = state_for(label);
        let (_, _, tools) = PREVIEW.mining_properties(state).unwrap();
        // Таблица добычи не смотрит на инструмент; берём годный (кирка для камня, топор для дерева).
        let tool = tools.iter().copied().next();
        let got: Vec<(String, i64)> = PREVIEW.drops_by_luck(state, tool, 0.5).unwrap().into_iter()
            .map(|stack| (iname(stack.item).to_owned(), i64::from(stack.count))).collect();
        let mut want: Vec<(String, i64)> = expected.as_array().unwrap().iter().map(|pair| (pair[0].as_str().unwrap().to_owned(), pair[1].as_i64().unwrap())).collect();
        if label.starts_with("potted_") {
            // Горшок: цветочный горшок выпадает таблицей, саженец возвращает обработчик горшков.
            want.retain(|(name, _)| name == "flower_pot");
        }
        assert_eq!(got, want, "{label}");
    }
}

#[test]
fn stack_sizes_match_the_official_server() {
    let observed = observed();
    for (name, size) in observed["stacks_not_64"].as_object().unwrap() {
        if name.ends_with("boat") { continue; } // лодки вне каталога [проверить]
        assert_eq!(PREVIEW.stack_size(item(name)), Ok(size.as_i64().unwrap() as i32), "{name}");
    }
    assert_eq!(PREVIEW.stack_size(item("poplar_planks")), Ok(64));
    assert_eq!(PREVIEW.stack_size(item("sulfur")), Ok(64));
}

fn names_of(cell: &serde_json::Value) -> Option<Vec<String>> {
    cell.as_array().map(|items| items.iter().map(|name| name.as_str().unwrap().to_owned()).collect())
}

#[test]
fn crafting_recipes_match_the_official_server() {
    let observed = observed();
    let mut checked = 0;
    for entry in observed["crafting"].as_array().unwrap() {
        let result = entry["result"].as_str().unwrap();
        let wanted_cells: Vec<Option<Vec<String>>> = entry["cells"].as_array().unwrap().iter().map(|cell| if cell.is_null() { None } else { names_of(cell) }).collect();
        let (shaped, width, height) = (entry["shaped"].as_bool().unwrap(), entry["width"].as_u64().unwrap_or(0) as u8, entry["height"].as_u64().unwrap_or(0) as u8);
        let (ours, count): (&crate::crafting::Recipe, i32) = if let Some(found) = crate::crafting::poplar_recipes().iter().find(|e| iname(e.recipe.result) == result) {
            (&found.recipe, found.recipe.count)
        } else if let Some((_, recipe)) = crate::crafting::mineral_recipes().iter().find(|(_, r)| iname(r.result) == result) {
            (recipe, recipe.count)
        } else { panic!("нет рецепта {result}") };
        assert_eq!(count, entry["count"].as_i64().unwrap() as i32, "{result}");
        assert_eq!(ours.shaped, shaped, "{result}");
        if shaped { assert_eq!((ours.width, ours.height), (width, height), "{result}"); }
        // Тег poplar_logs в наблюдении записан именем тега: проверяем состав вручную.
        if result == "poplar_planks" {
            let mut got: Vec<_> = ours.cells[0].iter().map(|id| iname(*id)).collect();
            got.sort_unstable();
            assert_eq!(got, ["poplar_log", "poplar_wood", "stripped_poplar_log", "stripped_poplar_wood"]);
            checked += 1;
            continue;
        }
        assert_eq!(ours.cells.len(), wanted_cells.len(), "{result}");
        for (cell, wanted) in ours.cells.iter().zip(&wanted_cells) {
            let got: Option<Vec<String>> = (!cell.is_empty()).then(|| cell.iter().map(|id| iname(*id).to_owned()).collect());
            assert_eq!(&got, wanted, "{result}");
        }
        checked += 1;
    }
    // 14 рецептов дерева тополя, две лодки и 27 рецептов серы/киновари.
    // + соломенная кровать и 16 подушек (furnish_tests).
    assert_eq!(crate::crafting::poplar_recipes().len(), 16 + 17);
    assert_eq!(checked, 16 + crate::crafting::mineral_recipes().len());
}

#[test]
fn stonecutter_recipes_match_the_official_server_in_order() {
    let observed = observed();
    let wanted: Vec<(String, String, i64)> = observed["stonecutting"].as_array().unwrap().iter()
        .map(|row| (row[0].as_str().unwrap().to_owned(), row[1].as_str().unwrap().to_owned(), row[2].as_i64().unwrap())).collect();
    let ours: Vec<(String, String, i64)> = crate::stonecutter::mineral_recipes().iter()
        .map(|r| (iname(r.input).to_owned(), iname(r.output).to_owned(), i64::from(r.count))).collect();
    assert_eq!(ours, wanted);
}

#[test]
fn leaf_drops_are_saplings_and_sticks_without_apples() {
    let leaf = PREVIEW.state_from_text("yellow_poplar_leaves[persistent=false]").unwrap().unwrap();
    let sapling = item("poplar_sapling");
    let stick = item("stick");
    // Первые броски ниже порогов: саженец 5 %, палки 2 %, яблока нет вовсе.
    let all = crate::mining::block_drops_with_catalog(PREVIEW, leaf, None, || 0.0);
    assert!(all.iter().any(|stack| stack.item == sapling));
    assert!(all.iter().any(|stack| stack.item == stick));
    assert!(all.iter().all(|stack| iname(stack.item) != "apple"));
    assert!(crate::mining::block_drops_with_catalog(PREVIEW, leaf, None, || 0.99).is_empty());
    // Ножницы дают саму листву своего цвета.
    let shears = Stack::new(item("shears"), 1);
    assert_eq!(crate::mining::block_drops_with_catalog(PREVIEW, leaf, Some(shears), || 0.5), vec![Stack::new(item("yellow_poplar_leaves"), 1)]);
}

#[test]
fn fire_fuel_and_smelting_treat_poplar_like_other_woods() {
    use crate::fire::flammability;
    assert_eq!(flammability("poplar_planks"), (5, 20));
    assert_eq!(flammability("poplar_slab"), (5, 20));
    assert_eq!(flammability("poplar_fence_gate"), (5, 20));
    assert_eq!(flammability("poplar_log"), (5, 5));
    assert_eq!(flammability("stripped_poplar_wood"), (5, 5));
    assert_eq!(flammability("red_poplar_leaves"), (30, 60));
    assert_eq!(crate::furnace::burn_time(item("poplar_planks")), Some(300));
    assert_eq!(crate::furnace::burn_time(item("poplar_slab")), Some(150));
    assert_eq!(crate::furnace::burn_time(item("poplar_door")), Some(200));
    assert_eq!(crate::furnace::burn_time(item("poplar_button")), Some(100));
    assert_eq!(crate::furnace::burn_time(item("poplar_log")), Some(300));
    assert_eq!(crate::furnace::smelt(crate::containers::Kind::Furnace, item("poplar_log")), Some((item("charcoal"), 0.15)));
}

#[test]
fn crafting_grid_makes_poplar_items_and_accepts_poplar_planks_for_any_planks_recipe() {
    let grid = |items: &[(usize, &str)]| {
        let mut slots = vec![None; 9];
        for (index, name) in items { slots[*index] = Some(Stack::new(item(name), 1)); }
        slots
    };
    let craft = |slots: &[Option<Stack>]| crate::crafting::craft_for_catalog(PREVIEW, slots).map(|stack| (iname(stack.item).to_owned(), stack.count));
    assert_eq!(craft(&grid(&[(0, "poplar_log")])), Some(("poplar_planks".to_owned(), 4)));
    assert_eq!(craft(&grid(&[(4, "stripped_poplar_wood")])), Some(("poplar_planks".to_owned(), 4)));
    assert_eq!(craft(&grid(&[(3, "poplar_planks"), (4, "poplar_planks"), (5, "poplar_planks")])), Some(("poplar_slab".to_owned(), 6)));
    assert_eq!(craft(&grid(&[(0, "poplar_log"), (1, "poplar_log"), (3, "poplar_log"), (4, "poplar_log")])), Some(("poplar_wood".to_owned(), 3)));
    // Тег planks: палки, верстак, сундук принимают доски тополя.
    assert_eq!(craft(&grid(&[(0, "poplar_planks"), (3, "poplar_planks")])), Some(("stick".to_owned(), 4)));
    assert_eq!(craft(&grid(&[(0, "poplar_planks"), (1, "poplar_planks"), (3, "poplar_planks"), (4, "poplar_planks")])), Some(("crafting_table".to_owned(), 1)));
    // Но не там, где доски названы точно: дубовая плита из досок тополя не получается.
    assert!(craft(&grid(&[(0, "poplar_planks"), (1, "oak_planks"), (2, "poplar_planks")])).is_none_or(|(name, _)| name != "oak_slab"));
    // Обычный профиль не знает тополя.
    assert_eq!(crate::crafting::craft_for_catalog(NATIVE, &grid(&[(0, "poplar_planks"), (3, "poplar_planks")])), None);
}

fn preview_world() -> crate::world::World {
    let mut world = crate::world::World::in_memory_with_catalog(PREVIEW);
    let dirt = blocks::state_by_name("dirt").unwrap();
    for x in -4..=4 { for z in -4..=4 { world.set_block_silently(x, 63, z, dirt); world.set_block_silently(x, 66, z, 1); } }
    world
}

fn click(face: i32) -> crate::placing::Click { crate::placing::Click { face, cursor_y: 0.5, yaw: 0.0, pitch: 0.0 } }

#[test]
fn every_poplar_item_is_placed_by_the_ordinary_path() {
    let player = (30.5, 64.0, 30.5);
    for row in POPLAR.iter().filter(|row| row.has_item() && !matches!(row.form, BlockForm::Plant | BlockForm::Boat | BlockForm::Item)) {
        let mut world = preview_world();
        let (name, default_state) = PREVIEW.block_for_item(row.item).unwrap().unwrap();
        assert_eq!(name, row.name);
        // Подвесная табличка вешается под потолок, остальное ставится сверху на землю.
        let (clicked, face, at) = if row.name == "poplar_hanging_sign" { ((0, 66, 0), 0, (0, 65, 0)) } else { ((0, 63, 0), 1, (0, 64, 0)) };
        let result = crate::placing::place(&mut world, name, default_state, clicked, &click(face), player);
        assert!(matches!(result, crate::placing::Placement::Placed), "{}: {result:?}", row.name);
        let state = world.get_block(at.0, at.1, at.2);
        assert_eq!(blocks::block_at_state(state), Some(row.name), "{}", row.name);
        assert_eq!(world.catalog().name(Kind::State, state), Ok(row.name));
        if row.name == "poplar_door" {
            let top = world.get_block(0, 65, 0);
            assert_eq!(blocks::orientation_of(top).and_then(|rule| rule.value(top, "half")), Some("upper"));
        }
    }
}

#[test]
fn signs_on_walls_fences_and_logs_follow_the_oak_rules() {
    let player = (30.5, 64.0, 30.5);
    let mut world = preview_world();
    // Настенная табличка (её выбирает сетевой обработчик по грани щелчка) устроена как дубовая.
    let wall = blocks::state_by_name("poplar_wall_sign").unwrap();
    assert_eq!(blocks::orientation_of(wall).unwrap().name, "poplar_wall_sign");
    assert_eq!(crate::redstone::block_entity_type(wall), crate::redstone::block_entity_type(blocks::state_by_name("oak_wall_sign").unwrap()));
    assert_eq!(crate::redstone::block_entity_type(blocks::state_by_name("poplar_hanging_sign").unwrap()), crate::redstone::block_entity_type(blocks::state_by_name("oak_hanging_sign").unwrap()));
    // Бревно ложится осью по грани щелчка.
    let (name, default_state) = PREVIEW.block_for_item(item("poplar_log")).unwrap().unwrap();
    crate::placing::place(&mut world, name, default_state, (0, 63, 0), &click(1), player);
    let log = world.get_block(0, 64, 0);
    assert_eq!(blocks::orientation_of(log).and_then(|rule| rule.value(log, "axis")), Some("y"));
    // Забор срастается с соседним забором тополя и с калиткой.
    let (name, default_state) = PREVIEW.block_for_item(item("poplar_fence")).unwrap().unwrap();
    crate::placing::place(&mut world, name, default_state, (2, 63, 0), &click(1), player);
    crate::placing::place(&mut world, name, default_state, (3, 63, 0), &click(1), player);
    let fence = world.get_block(2, 64, 0);
    assert_eq!(blocks::orientation_of(fence).and_then(|rule| rule.value(fence, "east")), Some("true"));
    // Плита сливается в двойную, ступени ложатся по взгляду.
    let (name, default_state) = PREVIEW.block_for_item(item("poplar_slab")).unwrap().unwrap();
    crate::placing::place(&mut world, name, default_state, (-2, 63, 0), &click(1), player);
    let merged = crate::placing::place(&mut world, name, default_state, (-2, 64, 0), &click(1), (30.5, 64.0, 30.5));
    assert!(matches!(merged, crate::placing::Placement::Merged), "{merged:?}");
    let double = world.get_block(-2, 64, 0);
    assert_eq!(blocks::orientation_of(double).and_then(|rule| rule.value(double, "type")), Some("double"));
}

#[test]
fn poplar_wood_strips_with_an_axe_keeping_the_axis() {
    let log = PREVIEW.state_from_text("poplar_log[axis=x]").unwrap().unwrap();
    let stripped = crate::copper::corresponding(log, "stripped_poplar_log").unwrap();
    assert_eq!(blocks::block_at_state(stripped), Some("stripped_poplar_log"));
    assert_eq!(blocks::orientation_of(stripped).and_then(|rule| rule.value(stripped, "axis")), Some("x"));
}

#[test]
fn tnt_and_mining_speed_follow_the_oak_twin() {
    let axe = Stack::new(item("iron_axe"), 1);
    let env = crate::mining::Conditions { haste: 0, fatigue: 0, underwater: false, on_ground: true, efficiency: 0.0, block_speed: 1.0, submerged_speed: 0.2 };
    for (poplar, oak) in [("poplar_planks", "oak_planks"), ("poplar_log", "oak_log"), ("orange_poplar_leaves", "oak_leaves"), ("poplar_door", "oak_door"), ("poplar_sapling", "oak_sapling")] {
        let (p, o) = (PREVIEW.default_state(poplar).unwrap().unwrap(), blocks::state_by_name(oak).unwrap());
        assert_eq!(crate::mining::progress_with_catalog(PREVIEW, p, Some(axe), env), crate::mining::progress_with_catalog(PREVIEW, o, Some(axe), env), "{poplar}");
        assert_eq!(PREVIEW.blast_resistance(p), Ok(blocks::blast_resistance(o)));
    }
    // Динамит ломает доску тополя и роняет её саму.
    let planks = PREVIEW.default_state("poplar_planks").unwrap().unwrap();
    assert_eq!(PREVIEW.explosion_drops(planks, 0.5), Ok(Some(Stack::new(item("poplar_planks"), 1))));
    let slab_double = state_for("poplar_slab_double");
    assert_eq!(PREVIEW.explosion_drops(slab_double, 0.5), Ok(Some(Stack::new(item("poplar_slab"), 2))));
}

#[test]
fn poplar_leaves_decay_without_a_log_and_stay_next_to_a_poplar_log() {
    let mut world = preview_world();
    let leaf = PREVIEW.state_from_text("red_poplar_leaves[distance=7,persistent=false]").unwrap().unwrap();
    world.set_block_silently(0, 64, 0, leaf);
    crate::farming::random_tick(&mut world, (0, 64, 0), leaf);
    assert_eq!(world.get_block(0, 64, 0), crate::world::AIR, "листва без ствола опала");

    let mut world = preview_world();
    world.set_block_silently(0, 64, 0, PREVIEW.state_from_text("poplar_log[axis=y]").unwrap().unwrap());
    let near = PREVIEW.state_from_text("red_poplar_leaves[distance=1,persistent=false]").unwrap().unwrap();
    world.set_block_silently(1, 64, 0, near);
    crate::farming::random_tick(&mut world, (1, 64, 0), near);
    assert_eq!(blocks::block_at_state(world.get_block(1, 64, 0)), Some("red_poplar_leaves"), "у ствола листва держится");
}

// ---------------------------------------------------------------------------
// Растения 26.3 (полочный гриб, красный куст), лодки и дерево из саженца тополя.
// Наблюдение официального сервера 26.3: RCON-команды, крафтер, раздатчик с костной мукой, регионы мира.
// ---------------------------------------------------------------------------

fn plant_world() -> crate::world::World {
    let mut world = crate::world::World::in_memory_with_catalog(PREVIEW);
    let (dirt, grass) = (blocks::state_by_name("dirt").unwrap(), blocks::state_by_name("grass_block").unwrap());
    for x in -6..=6 { for z in -6..=6 { world.set_block_silently(x, 63, z, if (x + z) % 2 == 0 { grass } else { dirt }); } }
    world
}

#[test]
fn plants_and_boats_have_the_pinned_777_ids_and_no_native_identity() {
    let shelf = POPLAR.iter().find(|row| row.name == "shelf_mushroom").unwrap();
    let shrub = POPLAR.iter().find(|row| row.name == "red_shrub").unwrap();
    assert_eq!((shelf.item, shelf.block, shelf.first_state, shelf.last_state), (1618, 1283, 35698, 35705));
    assert_eq!((shrub.item, shrub.block, shrub.first_state, shrub.last_state), (1619, 1284, 35706, 35706));
    assert_eq!((shelf.wire_item, shelf.wire_block, shelf.wire_first_state), (319, 447, 11227));
    assert_eq!((shrub.wire_item, shrub.wire_block, shrub.wire_first_state), (241, 144, 2367));
    let boats: Vec<_> = POPLAR.iter().filter(|row| row.form == BlockForm::Boat).collect();
    assert_eq!(boats.iter().map(|row| (row.name, row.item, row.wire_item)).collect::<Vec<_>>(), [("poplar_boat", 1620, 993), ("poplar_chest_boat", 1621, 994)]);
    for row in &boats {
        assert!(!row.has_block());
        assert_eq!(PREVIEW.item_named(row.name), Ok(Some(row.item)));
        assert_eq!(PREVIEW.to_client(Kind::Item, row.item, JavaVersion::V26_3), Ok(row.wire_item));
        assert_eq!(PREVIEW.decode_client_id(Kind::Item, row.wire_item, JavaVersion::V26_3), Ok(row.item));
        assert!(NATIVE.item_named(row.name).is_err());
        assert!(PREVIEW.to_client(Kind::Item, row.item, JavaVersion::V26_1_2).is_err());
        assert_eq!(PREVIEW.stack_size(row.item), Ok(1));
        assert_eq!(crate::blocks::block_id(row.name), None);
        assert_eq!(crate::blocks::state_by_name(row.name), None);
    }
    for (name, wire) in [("shelf_mushroom", 11227), ("red_shrub", 2367)] {
        let state = blocks::state_by_name(name).unwrap();
        assert_eq!(PREVIEW.to_client(Kind::State, state, JavaVersion::V26_3), Ok(wire));
        assert_eq!(PREVIEW.stack_size(blocks::item_named(name).unwrap()), Ok(64));
    }
    // Свойства полочного гриба: age × facing; состояние 777 — смещение от 11227.
    for state in shelf.first_state..=shelf.last_state {
        let rule = blocks::orientation_of(state).unwrap();
        let (age, facing) = (rule.value(state, "age").unwrap(), rule.value(state, "facing").unwrap());
        assert_eq!(rule.state(&[("age", age), ("facing", facing)]), Some(state));
        assert_eq!(PREVIEW.to_client(Kind::State, state, JavaVersion::V26_3), Ok(11227 + (state - shelf.first_state) as u32));
    }
    // Сущности лодок: внутренние 157/158 ↔ 106/107 в 777, только в профиле Preview.
    use mcsa_protocol::{content::{ContentCodec, IdSpace}, ids::RegistryKind};
    let codec = ContentCodec::white_slab_preview();
    for (internal, wire) in [(157, 106), (158, 107)] {
        assert_eq!(codec.map(RegistryKind::Entity, IdSpace::Canonical, IdSpace::Wire(JavaVersion::V26_3), internal), Ok(wire));
        assert_eq!(codec.map(RegistryKind::Entity, IdSpace::Wire(JavaVersion::V26_3), IdSpace::Canonical, wire), Ok(internal));
        assert!(codec.map(RegistryKind::Entity, IdSpace::Canonical, IdSpace::Wire(JavaVersion::V26_1_2), internal).is_err());
        assert!(ContentCodec::default().map(RegistryKind::Entity, IdSpace::Canonical, IdSpace::Wire(JavaVersion::V26_3), internal).is_err());
    }
    assert_eq!(crate::minecart::Kind::from_name("poplar_boat").map(|kind| kind.java_type()), Some(157));
    assert_eq!(crate::minecart::Kind::from_name("poplar_chest_boat").map(|kind| (kind.java_type(), kind.slots())), Some((158, 27)));
}

#[test]
fn poplar_boats_are_crafted_and_come_back_as_their_own_items() {
    let planks = item("poplar_planks");
    let grid = |cells: [Option<Stack>; 9]| cells.to_vec();
    let p = Some(Stack::new(planks, 1));
    let boat = crate::crafting::craft_for_catalog(PREVIEW, &grid([p, None, p, p, p, p, None, None, None]));
    assert_eq!(boat.map(|stack| (iname(stack.item), stack.count)), Some(("poplar_boat", 1)));
    let chest = Some(Stack::new(item("chest"), 1));
    let chest_boat = crate::crafting::craft_for_catalog(PREVIEW, &grid([chest, Some(Stack::new(item("poplar_boat"), 1)), None, None, None, None, None, None, None]));
    assert_eq!(chest_boat.map(|stack| iname(stack.item)), Some("poplar_chest_boat"));
    // В обычном профиле такого рецепта нет.
    assert!(crate::crafting::craft_for_catalog(NATIVE, &grid([p, None, p, p, p, p, None, None, None])).is_none_or(|stack| blocks::item_name(stack.item) != Some("poplar_boat")));
    // Лодка выпадает предметом тополя (имя сущности = имя предмета).
    for name in ["poplar_boat", "poplar_chest_boat"] {
        let kind = crate::minecart::Kind::from_name(name).unwrap();
        assert!(kind.is_boat());
        assert_eq!(blocks::item_named(kind.name()), Some(item(name)));
    }
}

#[test]
fn shelf_mushroom_sticks_to_a_side_drops_itself_and_grows_with_bone_meal() {
    let mut world = plant_world();
    let stone = blocks::state_by_name("stone").unwrap();
    world.set_block_silently(0, 64, 0, stone);
    let (name, default_state) = PREVIEW.block_for_item(item("shelf_mushroom")).unwrap().unwrap();
    for (face, facing, at) in [(2, "north", (0, 64, -1)), (3, "south", (0, 64, 1)), (4, "west", (-1, 64, 0)), (5, "east", (1, 64, 0))] {
        let result = crate::placing::place(&mut world, name, default_state, (0, 64, 0), &click(face), (30.5, 64.0, 30.5));
        assert!(matches!(result, crate::placing::Placement::Placed), "{facing}: {result:?}");
        let state = world.get_block(at.0, at.1, at.2);
        let rule = blocks::orientation_of(state).unwrap();
        assert_eq!((rule.value(state, "facing"), rule.value(state, "age")), (Some(facing), Some("0")));
        assert!(crate::plants::survives(&world, state, at.0, at.1, at.2), "{facing}");
    }
    // Без опоры гриб не стоит: сняли камень — все четыре ломаются при проверке.
    world.set_block_silently(0, 64, 0, 0);
    for at in [(0, 64, -1), (0, 64, 1), (-1, 64, 0), (1, 64, 0)] {
        let state = world.get_block(at.0, at.1, at.2);
        assert!(!crate::plants::survives(&world, state, at.0, at.1, at.2));
        assert!(crate::plants::check(&mut world, at.0, at.1, at.2, false));
        assert_eq!(world.get_block(at.0, at.1, at.2), 0);
    }
    // Маленький и большой выпадают собой (1 и 2), рукой и любым инструментом; мука делает большим один раз.
    let small = blocks::state_by_name("shelf_mushroom").unwrap();
    let rule = blocks::orientation("shelf_mushroom").unwrap();
    let large = rule.state(&[("age", "1"), ("facing", "north")]).unwrap();
    for (state, count) in [(small, 1), (large, 2)] {
        let drop = PREVIEW.drops_by_luck(state, None, 0.5).unwrap().unwrap();
        assert_eq!((iname(drop.item), drop.count), ("shelf_mushroom", count));
        assert_eq!(PREVIEW.mining_properties(state).unwrap().0, 0.0);
        assert_eq!(PREVIEW.light_properties(state).unwrap().0, 1, "светит на 1 как бурый гриб");
    }
    world.set_block_silently(0, 64, 0, stone);
    let result = crate::placing::place(&mut world, name, default_state, (0, 64, 0), &click(5), (30.5, 64.0, 30.5));
    assert!(matches!(result, crate::placing::Placement::Placed));
    assert!(crate::farming::bone_meal(&mut world, (1, 64, 0)).is_some());
    let grown = world.get_block(1, 64, 0);
    assert_eq!(blocks::orientation_of(grown).and_then(|rule| rule.value(grown, "age")), Some("1"));
    assert_eq!(blocks::orientation_of(grown).and_then(|rule| rule.value(grown, "facing")), Some("east"));
    assert!(crate::farming::bone_meal(&mut world, (1, 64, 0)).is_none(), "у большого мука не тратится");
}

#[test]
fn red_shrub_needs_soil_and_bone_meal_plants_one_neighbour() {
    let mut world = plant_world();
    let shrub = blocks::state_by_name("red_shrub").unwrap();
    // Почва: дёрн и земля — да; камень, песок — нет.
    for (below, ok) in [("grass_block", true), ("dirt", true), ("podzol", true), ("moss_block", true), ("mud", true), ("stone", false), ("sand", false)] {
        world.set_block_silently(0, 63, 0, blocks::state_by_name(below).unwrap());
        assert_eq!(crate::plants::can_place(&world, shrub, 0, 64, 0), ok, "{below}");
    }
    world.set_block_silently(0, 63, 0, blocks::state_by_name("grass_block").unwrap());
    world.set_block_silently(0, 64, 0, shrub);
    // Мука: ровно один новый куст на одном из четырёх горизонтальных соседей, куст-источник остаётся.
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..60 {
        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] { world.set_block_silently(dx, 64, dz, 0); }
        assert!(crate::farming::bone_meal(&mut world, (0, 64, 0)).is_some());
        let grown: Vec<_> = [(1, 0), (-1, 0), (0, 1), (0, -1)].into_iter().filter(|&(dx, dz)| world.get_block(dx, 64, dz) == shrub).collect();
        assert_eq!(grown.len(), 1);
        seen.insert(grown[0]);
    }
    assert_eq!(seen.len(), 4, "все четыре стороны достижимы");
    // Нет подходящего места (кругом камень над почвой или нет почвы): мука не тратится.
    let stone = blocks::state_by_name("stone").unwrap();
    for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] { world.set_block_silently(dx, 64, dz, stone); }
    assert!(crate::farming::bone_meal(&mut world, (0, 64, 0)).is_none());
    // Куст заменяется любым блоком без выпадения (тег replaceable) и горит.
    assert!(blocks::replaceable(shrub));
    assert_eq!(crate::compost::chance("red_shrub"), Some(30));
    assert_eq!(crate::compost::chance("shelf_mushroom"), Some(65));
    let drop = PREVIEW.drops_by_luck(shrub, None, 0.5).unwrap().unwrap();
    assert_eq!((iname(drop.item), drop.count), ("red_shrub", 1));
}

#[test]
fn mushroom_soups_accept_any_two_mushrooms_including_shelf() {
    let (bowl, shelf, red, brown) = (item("bowl"), item("shelf_mushroom"), item("red_mushroom"), item("brown_mushroom"));
    let s = |id| Some(Stack::new(id, 1));
    let craft = |cells: &[Option<Stack>]| {
        let mut grid = vec![None; 9];
        for (index, cell) in cells.iter().enumerate() { grid[index] = *cell; }
        crate::crafting::craft_for_catalog(PREVIEW, &grid).map(|stack| iname(stack.item))
    };
    // Наблюдение 26.3 (крафтер): суп из миски и любых двух грибов, три гриба или один — нет.
    for pair in [(shelf, red), (shelf, brown), (shelf, shelf), (red, red), (brown, brown), (red, brown)] {
        assert_eq!(craft(&[s(bowl), s(pair.0), s(pair.1)]), Some("mushroom_stew"), "{pair:?}");
    }
    assert_eq!(craft(&[s(bowl), s(shelf)]), None);
    assert_eq!(craft(&[s(bowl), s(red), s(red), s(brown)]), None);
    let flower = item("dandelion");
    for pair in [(shelf, red), (shelf, shelf), (brown, brown)] {
        assert_eq!(craft(&[s(pair.0), s(pair.1), s(bowl), s(flower)]), Some("suspicious_stew"), "{pair:?}");
    }
    assert_eq!(craft(&[s(shelf), s(bowl), s(flower)]), None);
    let rabbit = [s(item("cooked_rabbit")), s(item("carrot")), s(item("baked_potato")), s(bowl)];
    for mushroom in [shelf, red, brown] {
        let mut cells = rabbit.to_vec();
        cells.push(s(mushroom));
        assert_eq!(craft(&cells), Some("rabbit_stew"));
    }
    // В профиле 775 полочного гриба нет, а старые рецепты прежние.
    assert_eq!(crate::crafting::craft_for_catalog(NATIVE, &[s(bowl), s(red), s(brown), None, None, None, None, None, None]).map(|stack| blocks::item_name(stack.item)), Some(Some("mushroom_stew")));
}

#[test]
fn poplar_saplings_grow_into_the_measured_tree() {
    use std::collections::BTreeMap;
    let sapling = blocks::state_by_name("poplar_sapling").unwrap();
    let stage1 = blocks::orientation("poplar_sapling").unwrap().state(&[("stage", "1")]).unwrap();
    assert_ne!(sapling, stage1);
    let mut heights = BTreeMap::new();
    let (mut colours, mut leaf_total, mut with_ring, mut ring_lengths) = (BTreeMap::new(), 0usize, 0usize, BTreeMap::new());
    let trees = 400;
    for index in 0..trees {
        let mut world = crate::world::World::in_memory_with_catalog(PREVIEW);
        // Мир в памяти начинает с одного и того же семени: каждое дерево получает свою часть потока случайных чисел.
        for _ in 0..=index * 3 { world.random_bits(); }
        for x in -12..=12 { for z in -12..=12 { world.set_block_silently(x, 63, z, blocks::state_by_name("grass_block").unwrap()); } }
        world.set_block_silently(0, 64, 0, stage1);
        assert!(crate::farming::grow_tree(&mut world, (0, 64, 0), stage1));
        let top = (64..90).take_while(|&y| blocks::block_at_state(world.get_block(0, y, 0)) == Some("poplar_log")).last().unwrap();
        *heights.entry(top - 63).or_insert(0) += 1;
        let (mut leaves, mut names, mut y_min, mut y_max) = (0, std::collections::BTreeSet::new(), 99, 0);
        let mut ring_max = 0;
        for x in -12..=12 { for z in -12..=12 { for y in 64..95 {
            let name = blocks::block_at_state(world.get_block(x, y, z)).unwrap_or("");
            if name.ends_with("_poplar_leaves") { leaves += 1; names.insert(name); y_min = y_min.min(y); y_max = y_max.max(y); }
            // Ветки: брёвна вне оси ствола, на высоте «макушка − 3 … − 1» верхнего кольца.
            if name == "poplar_log" && (x != 0 || z != 0) && y >= top - 3 { ring_max = ring_max.max(x.abs() + z.abs()); }
        } } }
        assert_eq!(names.len(), 1, "один цвет листвы на дерево");
        *colours.entry(*names.iter().next().unwrap()).or_insert(0) += 1;
        // Листва от «макушка − 4» до «макушка + 2»; кольцо веток под ней.
        assert!(y_min == top - 4 && (top + 1..=top + 2).contains(&y_max), "слои листвы {y_min}..{y_max} при макушке {top}");
        leaf_total += leaves;
        if ring_max > 0 { with_ring += 1; *ring_lengths.entry(ring_max).or_insert(0) += 1; }
        // Нижнее кольцо: хотя бы одна ветка на «макушка − 4».
        assert!([(1, 0), (-1, 0), (0, 1), (0, -1)].iter().any(|(dx, dz)| blocks::block_at_state(world.get_block(*dx, top - 4, *dz)) == Some("poplar_log")));
    }
    assert_eq!(heights.keys().copied().collect::<Vec<_>>(), [7, 8, 9, 10, 11], "{heights:?}");
    assert!(heights.values().all(|&n| (40..=120).contains(&n)), "ствол 7–11 примерно поровну: {heights:?}");
    let mean = leaf_total as f64 / trees as f64;
    assert!((190.0..280.0).contains(&mean), "листьев на дерево {mean} (замер: среднее около 235)");
    assert_eq!(colours.len(), 3, "{colours:?}");
    assert!(colours["yellow_poplar_leaves"] > colours["red_poplar_leaves"].max(colours["orange_poplar_leaves"]) * 8 / 10);
    // Верхнее кольцо есть примерно у 55 % (замер 36 из 63): длины 1…3.
    let share = with_ring as f64 / trees as f64;
    assert!((0.4..0.7).contains(&share), "доля деревьев с верхним кольцом {share} {ring_lengths:?}");
}
