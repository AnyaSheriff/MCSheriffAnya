//! Сера и киноварь 26.3: 28 объектов каталога (данные 777 из pinned JSON, рецепты — recipes.json + вики).
use super::*;
use crate::inventory::Stack;

const PREVIEW: Catalog = Catalog::new(Profile::Preview26_3);
const NATIVE: Catalog = Catalog::new(Profile::Java26_1_2);

fn row(name: &str) -> &'static SlabDescriptor { MINERALS.iter().find(|row| row.name == name).unwrap() }

#[test]
fn ids_are_appended_without_gaps_and_match_the_pinned_777_tables() {
    let candidate = std::path::Path::new(env!("HOME")).join(".local/share/mcsa-java-26.3-probe/candidate-json");
    assert_eq!(MINERALS.len(), 28);
    assert_eq!(MINERALS[0].item, WOOL_STAIRS[15].item + 1);
    assert_eq!(MINERALS[0].block, WOOL_STAIRS[15].block + 1);
    assert_eq!(MINERALS[0].first_state, WOOL_STAIRS[15].last_state + 1);
    let counts = |form| match form { BlockForm::Full => 1, BlockForm::Slab => 6, BlockForm::Stairs => 80, BlockForm::Wall => 324, BlockForm::PotentSulfur => 5, BlockForm::Spike => 20 };
    for (index, row) in MINERALS.iter().enumerate() {
        assert_eq!(row.family, SlabFamily::Mineral);
        assert_eq!(row.state_count(), counts(row.form), "{}", row.name);
        if index > 0 {
            assert_eq!((row.item, row.block, row.first_state), (MINERALS[index - 1].item + 1, MINERALS[index - 1].block + 1, MINERALS[index - 1].last_state + 1));
        }
        assert_eq!(PREVIEW.item_named(row.name), Ok(Some(row.item)));
        assert!(NATIVE.item_named(row.name).is_err());
        assert_eq!(PREVIEW.stack_size(row.item), Ok(64));
    }
    // Номера 777 сверяются с файлами, если они есть на этой машине.
    let (Ok(blocks), Ok(items)) = (std::fs::read_to_string(candidate.join("blocks.json")), std::fs::read_to_string(candidate.join("items.json"))) else { return; };
    let blocks: Vec<serde_json::Value> = serde_json::from_str(&blocks).unwrap();
    let items: Vec<serde_json::Value> = serde_json::from_str(&items).unwrap();
    for row in &MINERALS {
        let block = blocks.iter().find(|b| b["name"] == row.name).unwrap();
        let item = items.iter().find(|i| i["name"] == row.name).unwrap();
        assert_eq!(block["id"].as_u64(), Some(u64::from(row.wire_block)));
        assert_eq!(block["minStateId"].as_u64(), Some(u64::from(row.wire_first_state)));
        assert_eq!(item["id"].as_u64(), Some(u64::from(row.wire_item)));
        assert_eq!(block["defaultState"].as_u64().unwrap() - block["minStateId"].as_u64().unwrap(), (row.default_state - row.first_state) as u64);
        assert_eq!(block["hardness"], 1.5);
        assert_eq!(block["resistance"], if row.form == BlockForm::Spike { 3.0 } else { 6.0 });
    }
}

#[test]
fn every_state_has_properties_wire_mining_blast_and_drops() {
    let pickaxe = crate::blocks::item_named("wooden_pickaxe");
    let shovel = crate::blocks::item_named("diamond_shovel");
    let water = crate::blocks::blast_resistance(crate::blocks::state_by_name("water").unwrap());
    let mut total = 0;
    for row in &MINERALS {
        for state in row.first_state..=row.last_state {
            total += 1;
            let wire = PREVIEW.to_client(Kind::State, state, JavaVersion::V26_3).unwrap();
            assert_eq!(wire, row.wire_first_state + (state - row.first_state) as u32);
            assert_eq!(PREVIEW.decode_client_id(Kind::State, wire, JavaVersion::V26_3), Ok(state));
            assert!(PREVIEW.to_client(Kind::State, state, JavaVersion::V26_1_2).is_err());
            assert!(NATIVE.mining_properties(state).is_err());
            assert!(NATIVE.drops_by_luck(state, None, 0.5).is_err());
            let spike = row.form == BlockForm::Spike;
            let (hardness, material, _) = PREVIEW.mining_properties(state).unwrap();
            assert_eq!((hardness, material), (1.5, "mineable/pickaxe"));
            let own = if spike { 3.0 } else { 6.0 };
            let wet = PREVIEW.waterlogged(state).unwrap().unwrap_or(false);
            assert_eq!(PREVIEW.blast_resistance(state), Ok(if wet { water.max(own) } else { own }));
            let count = if row.form == BlockForm::Slab && PREVIEW.slab_properties(state).unwrap().unwrap().0 == SlabHalf::Double { 2 } else { 1 };
            assert_eq!(PREVIEW.drops_by_luck(state, pickaxe, 0.5), Ok(Some(Stack::new(row.item, count))), "{}", row.name);
            // Без кирки сера не выпадает; шип выпадает от чего угодно.
            let other = if spike { Some(Stack::new(row.item, 1)) } else { None };
            assert_eq!(PREVIEW.drops_by_luck(state, shovel, 0.5), Ok(other.map(|_| Stack::new(row.item, count))), "{}", row.name);
            assert_eq!(PREVIEW.drops_by_luck(state, None, 0.5), Ok(other.map(|_| Stack::new(row.item, count))), "{}", row.name);
            assert_eq!(crate::mining::harvests_with_catalog(PREVIEW, state, None), spike);
            assert!(PREVIEW.explosion_drops(state, 0.0).unwrap().is_some());
        }
    }
    assert_eq!(total, 8 + 36 + 480 + 1944 + 5 + 20);
}

#[test]
fn walls_keep_the_stone_wall_property_layout_and_text_parsing() {
    let wall = row("polished_cinnabar_wall");
    let rule = PREVIEW.orientation(wall.name).unwrap().unwrap();
    assert_eq!(rule.family, crate::blocks::Family::Wall);
    let stone = crate::blocks::orientation("cobblestone_wall").unwrap();
    assert_eq!(rule.properties.len(), stone.properties.len());
    for (a, b) in rule.properties.iter().zip(stone.properties) { assert_eq!((a.name, a.values, a.default), (b.name, b.values, b.default)); }
    let parsed = PREVIEW.state_from_text("minecraft:polished_cinnabar_wall[east=tall,up=false,waterlogged=true]").unwrap().unwrap();
    assert_eq!(rule.value(parsed, "east"), Some("tall"));
    assert_eq!(PREVIEW.waterlogged(parsed), Ok(Some(true)));
    assert_eq!(PREVIEW.state_from_text("sulfur[east=tall]"), Ok(None));
    assert_eq!(PREVIEW.state_from_text("sulfur"), Ok(Some(row("sulfur").default_state)));
    let spike = PREVIEW.state_from_text("sulfur_spike[thickness=base,vertical_direction=down]").unwrap().unwrap();
    assert_eq!(PREVIEW.orientation_of(spike).unwrap().unwrap().value(spike, "thickness"), Some("base"));
    let potent = PREVIEW.state_from_text("potent_sulfur[potent_sulfur_state=erupting]").unwrap().unwrap();
    assert_eq!(potent, row("potent_sulfur").first_state + 3);
}

#[test]
fn opaque_blocks_behave_like_stone_for_light_and_conduction() {
    for name in ["sulfur", "cinnabar_bricks", "chiseled_sulfur", "potent_sulfur"] {
        let state = row(name).default_state;
        assert_eq!(PREVIEW.light_properties(state).map(|(_, filter, _)| filter), Ok(15), "{name}");
        assert!(crate::blocks::full_cube(state) && crate::blocks::conductive(state) && crate::blocks::solid_top(state), "{name}");
    }
    for name in ["sulfur_wall", "sulfur_spike", "sulfur_slab", "cinnabar_stairs"] {
        let state = row(name).default_state;
        assert_eq!(PREVIEW.light_properties(state).map(|(_, filter, _)| filter), Ok(0), "{name}");
        assert!(!crate::blocks::full_cube(state) && !crate::blocks::conductive(state), "{name}");
    }
}

#[test]
fn manual_recipes_match_the_pinned_recipe_table() {
    let grid = |cells: &[(usize, i32)]| { let mut slots = [None; 9]; for (index, item) in cells { slots[*index] = Some(Stack::new(*item, 1)); } slots };
    let craft = |slots: [Option<Stack>; 9]| crate::crafting::craft_for_catalog(PREVIEW, &slots);
    let item = |name: &str| row(name).item;
    for m in ["sulfur", "cinnabar"] {
        let base = item(m);
        let polished = item(&format!("polished_{m}"));
        let bricks = item(&format!("{m}_bricks"));
        assert_eq!(craft(grid(&[(0, base), (1, base), (2, base)])), Some(Stack::new(item(&format!("{m}_slab")), 6)));
        assert_eq!(craft(grid(&[(0, base), (3, base), (4, base), (6, base), (7, base), (8, base)])), Some(Stack::new(item(&format!("{m}_stairs")), 4)));
        assert_eq!(craft(grid(&[(0, base), (1, base), (2, base), (3, base), (4, base), (5, base)])), Some(Stack::new(item(&format!("{m}_wall")), 6)));
        assert_eq!(craft(grid(&[(0, base), (1, base), (3, base), (4, base)])), Some(Stack::new(polished, 4)));
        assert_eq!(craft(grid(&[(0, polished), (1, polished), (3, polished), (4, polished)])), Some(Stack::new(bricks, 4)));
        assert_eq!(craft(grid(&[(1, polished), (2, polished), (4, polished), (5, polished)])), Some(Stack::new(bricks, 4)));
        assert_eq!(craft(grid(&[(0, polished), (1, polished), (2, polished)])), Some(Stack::new(item(&format!("polished_{m}_slab")), 6)));
        assert_eq!(craft(grid(&[(0, bricks), (1, bricks), (2, bricks)])), Some(Stack::new(item(&format!("{m}_brick_slab")), 6)));
        assert_eq!(craft(grid(&[(0, bricks), (3, bricks), (4, bricks), (6, bricks), (7, bricks), (8, bricks)])), Some(Stack::new(item(&format!("{m}_brick_stairs")), 4)));
        let slab = item(&format!("{m}_slab"));
        assert_eq!(craft(grid(&[(1, slab), (4, slab)])), Some(Stack::new(item(&format!("chiseled_{m}")), 1)));
        // Две плиты рядом, неполные формы и смесь предметов ничего не дают.
        assert_eq!(craft(grid(&[(0, slab), (1, slab)])), None);
        assert_eq!(craft(grid(&[(0, base), (1, base), (3, polished), (4, base)])), None);
        assert!(crate::crafting::craft_for_catalog(NATIVE, &grid(&[(0, base), (1, base), (2, base)])).is_none());
    }
    let spike = item("sulfur_spike");
    assert_eq!(craft(grid(&[(0, spike), (1, spike), (3, spike), (4, spike)])), Some(Stack::new(item("sulfur"), 1)));
    assert_eq!(craft(grid(&[(0, spike), (1, spike), (3, spike)])), None);
    // Мощная сера — девять серы в любом порядке без формы; киноварь так не складывается.
    let sulfur = item("sulfur");
    assert_eq!(craft(grid(&(0..9).map(|i| (i, sulfur)).collect::<Vec<_>>())), Some(Stack::new(item("potent_sulfur"), 1)));
    let cinnabar = item("cinnabar");
    assert_eq!(craft(grid(&(0..9).map(|i| (i, cinnabar)).collect::<Vec<_>>())), None);
    // Узнавание рецепта для книги.
    let slots = grid(&[(0, sulfur), (1, sulfur), (2, sulfur)]);
    let id = crate::crafting::recipe_identity_for_catalog(PREVIEW, &slots).unwrap();
    assert_eq!(id, crate::crafting::RecipeId::Mineral(MINERALS.iter().position(|row| row.name == "sulfur_slab").unwrap() as u8));
    assert_eq!(crate::crafting::recipe_for_catalog(PREVIEW, id).unwrap().result, item("sulfur_slab"));
    assert!(crate::crafting::recipe_for_catalog(NATIVE, id).is_none());
}

#[test]
fn stonecutter_follows_the_wiki_tiers() {
    let choices = |name: &str| -> Vec<(&'static str, i32)> {
        crate::stonecutter::choices_for_catalog(PREVIEW, row(name).item).iter()
            .map(|r| (MINERALS.iter().find(|row| row.item == r.output).unwrap().name, r.count)).collect()
    };
    let sulfur = choices("sulfur");
    assert_eq!(sulfur.len(), 12);
    assert!(sulfur.contains(&("sulfur_slab", 2)) && sulfur.contains(&("sulfur_wall", 1)) && sulfur.contains(&("chiseled_sulfur", 1)));
    assert!(sulfur.contains(&("sulfur_brick_slab", 2)) && sulfur.contains(&("polished_sulfur", 1)));
    assert_eq!(choices("polished_sulfur").len(), 7);
    assert!(choices("polished_sulfur").iter().all(|(name, _)| *name != "chiseled_sulfur" && *name != "polished_sulfur"));
    assert_eq!(choices("sulfur_bricks"), vec![("sulfur_brick_slab", 2), ("sulfur_brick_stairs", 1), ("sulfur_brick_wall", 1)]);
    assert_eq!(choices("cinnabar").len(), 12);
    assert!(choices("chiseled_cinnabar").is_empty() && choices("potent_sulfur").is_empty() && choices("sulfur_spike").is_empty());
    // Результат по кнопке и отказ в старом профиле.
    let input = Some(Stack::new(row("sulfur").item, 1));
    let button = crate::stonecutter::choices_for_catalog(PREVIEW, row("sulfur").item).iter().position(|r| r.output == row("sulfur_slab").item).unwrap() as i32;
    assert_eq!(crate::stonecutter::result_for_catalog(PREVIEW, input, button), Some(Stack::new(row("sulfur_slab").item, 2)));
    assert!(crate::stonecutter::result_for_catalog(NATIVE, input, 0).is_none());
    assert!(crate::stonecutter::result_for_catalog(PREVIEW, input, 12).is_none());
}

#[test]
fn recipe_book_lists_crafting_and_stonecutting_keys() {
    let mut book = crate::recipe_book::Book::for_catalog(PREVIEW);
    assert!(book.learn_named(&["sulfur_slab".to_string()]), "рецепт верстака открывается по имени");
    assert!(book.learn_named(&["sulfur_slab_from_sulfur_stonecutting".to_string()]));
    assert!(book.learn_named(&["chiseled_cinnabar_from_cinnabar_stonecutting".to_string()]));
    assert!(!book.learn_named(&["chiseled_cinnabar_from_polished_cinnabar_stonecutting".to_string()]));
    assert!(!book.learn_named(&["cinnabar".to_string()]), "у киновари нет рецепта верстака");
    let id = crate::crafting::RecipeId::Mineral(MINERALS.iter().position(|row| row.name == "potent_sulfur").unwrap() as u8);
    book.learn_recipe(id);
    assert!(book.knows_recipe(id));
    assert!(!crate::recipe_book::Book::for_catalog(NATIVE).knows_recipe(id));
}

#[test]
fn walls_connect_to_catalog_blocks_stone_walls_and_slabs() {
    use crate::world::World;
    let mut world = World::in_memory_with_catalog(PREVIEW);
    let wall = row("sulfur_wall");
    let rule = PREVIEW.orientation(wall.name).unwrap().unwrap();
    world.set_block_silently(1, 70, 0, row("cinnabar").default_state); // восток: цельный блок — высокая
    world.set_block_silently(-1, 70, 0, row("cinnabar_slab").default_state); // запад: плита — низкая
    world.set_block_silently(0, 70, 1, crate::blocks::state_by_name("cobblestone_wall").unwrap()); // юг: стенка
    world.set_block_silently(0, 70, 0, wall.default_state);
    crate::placing::refresh_neighbours(&mut world, 1, 70, 0);
    let state = world.get_block(0, 70, 0);
    assert_eq!((rule.value(state, "east"), rule.value(state, "west"), rule.value(state, "south"), rule.value(state, "north")),
        (Some("tall"), Some("low"), Some("tall"), Some("none")));
    // Каменная стенка рядом тоже видит стенку каталога.
    let stone = world.get_block(0, 70, 1);
    crate::placing::refresh_neighbours(&mut world, 0, 70, 0);
    assert_eq!(crate::blocks::orientation_of(world.get_block(0, 70, 1)).map(|r| r.value(world.get_block(0, 70, 1), "north")), Some(Some("tall")), "{stone}");
}
