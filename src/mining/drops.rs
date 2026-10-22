// Добыча зачарованным инструментом: minecraft.wiki/w/Fortune и /Silk_Touch.
// Таблица вероятностей руд также приведена в
// minecraft.wiki/w/Forum:Add_a_fortune_graph_for_ores («1 (40%) … 4 (20%)»).
// [проверить] Распределения случайных количеств чёрным ящиком 26.1.2.
use crate::{blocks, inventory::Stack};

fn uniform(roll: f32, choices: i32) -> i32 {
    ((roll.clamp(0.0, 0.999_999) * choices as f32) as i32).min(choices - 1)
}

fn multiplier(level: i32, roll: f32) -> i32 {
    if level == 0 { 1 } else { uniform(roll, level + 2).max(1) }
}

fn silk_block(name: &str) -> bool {
    name.ends_with("_ore") || name.ends_with("_leaves")
        || name.ends_with("_stained_glass") || name.ends_with("_stained_glass_pane")
        || matches!(name, "stone" | "deepslate" | "grass_block" | "podzol" | "mycelium"
            | "crimson_nylium" | "warped_nylium" | "glass" | "glass_pane"
            | "ice" | "packed_ice" | "blue_ice" | "bookshelf" | "ender_chest"
            | "clay" | "glowstone" | "sea_lantern" | "melon" | "gravel" | "snow_block"
            | "brown_mushroom_block" | "red_mushroom_block" | "mushroom_stem"
            | "amethyst_cluster" | "small_amethyst_bud" | "medium_amethyst_bud" | "large_amethyst_bud")
}

/// Листва может одновременно дать саженец, палки и яблоко. Независимые
/// броски: minecraft.wiki/w/Leaves, таблица Fortune; не один общий drop.
/// [проверить] Частоты большой выборкой чёрного ящика 26.1.2.
pub fn block_drops(state: i32, tool: Option<Stack>, mut roll: impl FnMut() -> f32) -> Vec<Stack> {
    let Some(name) = blocks::block_at_state(state) else { return Vec::new(); };
    if !name.ends_with("_leaves") { return drops(state, tool, roll).into_iter().collect(); }
    let data = tool.map(|stack| stack.extras()).unwrap_or_default();
    let shears = tool.is_some_and(|stack| blocks::item_name(stack.item) == Some("shears"));
    let mut result = Vec::new();
    let mut add = |name: &str, count| {
        if let Some(item) = blocks::item_named(name) { result.push(Stack::new(item, count)); }
    };
    if shears || data.level("silk_touch") > 0 {
        add(name, 1);
        return result;
    }
    let level = data.level("fortune").clamp(0, 4) as usize;
    let sapling_chance = if name == "jungle_leaves" { [0.025, 1.0 / 36.0, 0.03125, 1.0 / 24.0, 0.1] }
        else { [0.05, 0.0625, 1.0 / 12.0, 0.1, 0.1] };
    let sapling = match name {
        "mangrove_leaves" => None,
        "azalea_leaves" => Some("azalea".to_owned()),
        "flowering_azalea_leaves" => Some("flowering_azalea".to_owned()),
        // Тополь: три цвета листвы, один саженец; яблок нет (вики, Poplar).
        _ if name.ends_with("_poplar_leaves") => Some("poplar_sapling".to_owned()),
        _ => Some(format!("{}_sapling", name.trim_end_matches("_leaves"))),
    };
    if let Some(sapling) = sapling && roll() < sapling_chance[level] { add(&sapling, 1); }
    let stick_chance = [0.02, 1.0 / 45.0, 0.025, 1.0 / 30.0, 0.1][level];
    if roll() < stick_chance { add("stick", 1 + uniform(roll(), 2)); }
    if matches!(name, "oak_leaves" | "dark_oak_leaves") && roll() < [0.005, 1.0 / 180.0, 0.00625, 1.0 / 120.0, 0.025][level] {
        add("apple", 1);
    }
    result
}

/// Одна стопка обычного блока; составные выпадения посевов/ульев/горшков
/// остаются у своих обработчиков. Зачарования берутся из настоящей стопки,
/// поэтому одна и та же функция обслуживает Java и Bedrock.
pub fn drops(state: i32, tool: Option<Stack>, mut roll: impl FnMut() -> f32) -> Option<Stack> {
    let name = blocks::block_at_state(state)?;
    let data = tool.map(|stack| stack.extras()).unwrap_or_default();
    let silk = data.level("silk_touch") > 0;
    // Те же правила специальных блоков, что у Java-обработчика:
    // заражённый блок Silk сохраняет даже без подходящего инструмента.
    if crate::mob::arthropod::host(state).is_some() {
        return if silk { crate::mob::arthropod::silk_drop(state) } else { None };
    }
    if name == "creaking_heart" {
        return crate::mob::newblocks::heart_drop(state, silk, data.level("fortune"), roll());
    }
    if !super::harvests(state, tool) { return None; }
    if let Some(sculk) = crate::redstone::sculk_family::mined_drop(state, silk) { return sculk; }
    let stack = |name: &str, count| blocks::item_named(name).map(|item| Stack::new(item, count));
    if name == "frosted_ice" { return None; }
    if name == "snow" {
        // Чёрный ящик enchant-mining-special: 3 слоя + Silk → snow×3,
        // 8 слоёв + Silk → snow_block×1; обычная лопата → snowball×3.
        let layers = blocks::orientation_of(state).and_then(|block| block.value(state, "layers"))
            .and_then(|layers| layers.parse::<i32>().ok()).unwrap_or(1);
        return if silk { stack(if layers == 8 { "snow_block" } else { "snow" }, if layers == 8 { 1 } else { layers }) }
            else { stack("snowball", layers) };
    }
    if silk && silk_block(name) { return stack(name, 1); }
    if name == "snow_block" { return stack("snowball", 4); }
    let fortune = if silk { 0 } else { data.level("fortune").clamp(0, 255) };
    if matches!(name, "short_grass" | "fern" | "tall_grass" | "large_fern")
        && !tool.is_some_and(|stack| blocks::item_name(stack.item) == Some("shears")) {
        // Fortune увеличивает количество семян, но не шанс 1/8.
        // Вики, Fortune: «uniform distribution»; [проверить] 26.1.2.
        return if roll() < 0.125 { stack("wheat_seeds", 1 + uniform(roll(), 2 * fortune + 1)) } else { None };
    }
    // Обычные руды: количество × max(1, равномерное 0..уровень+1).
    // Сначала базовый бросок; отдельный бросок множителя не коррелирует с ним.
    let ore = match name.strip_prefix("deepslate_").unwrap_or(name) {
        "coal_ore" => Some(("coal", 1, 1)),
        "diamond_ore" => Some(("diamond", 1, 1)),
        "emerald_ore" => Some(("emerald", 1, 1)),
        "iron_ore" => Some(("raw_iron", 1, 1)),
        "gold_ore" => Some(("raw_gold", 1, 1)),
        "copper_ore" => Some(("raw_copper", 2, 5)),
        "lapis_ore" => Some(("lapis_lazuli", 4, 9)),
        "nether_gold_ore" => Some(("gold_nugget", 2, 6)),
        "nether_quartz_ore" => Some(("quartz", 1, 1)),
        _ => None,
    };
    if let Some((item, low, high)) = ore {
        let base = if low == high { low } else { low + uniform(roll(), high - low + 1) };
        return stack(item, base * if fortune == 0 { 1 } else { multiplier(fortune, roll()) });
    }
    // Fortune добавляет 0..уровень к базовому количеству. Верхние пределы
    // glowstone/sea_lantern/melon ограничивают суммарную стопку.
    let additive = match name {
        "redstone_ore" | "deepslate_redstone_ore" => Some(("redstone", 4, 5, i32::MAX)),
        "glowstone" => Some(("glowstone_dust", 2, 4, 4)),
        "sea_lantern" => Some(("prismarine_crystals", 2, 3, 5)),
        "melon" => Some(("melon_slice", 3, 7, 9)),
        _ => None,
    };
    if let Some((item, low, high, cap)) = additive {
        let base = low + uniform(roll(), high - low + 1);
        let bonus = if fortune == 0 { 0 } else { uniform(roll(), fortune + 1) };
        return stack(item, (base + bonus).min(cap));
    }
    if name == "gravel" {
        // Вики, Gravel: 10%, 1/7, 1/4 и 100% при Fortune 0/I/II/III.
        let chance = [0.1, 1.0 / 7.0, 0.25, 1.0][fortune.min(3) as usize];
        return stack(if roll() < chance { "flint" } else { "gravel" }, 1);
    }
    if name == "bookshelf" { return stack("book", 3); }
    if name == "ender_chest" { return stack("obsidian", 8); }
    if name == "clay" { return stack("clay_ball", 4); }
    if name == "amethyst_cluster" {
        let pick = tool.and_then(|s| blocks::item_name(s.item)).is_some_and(|n| n.ends_with("_pickaxe"));
        return stack("amethyst_shard", if pick { 4 * if fortune == 0 { 1 } else { multiplier(fortune, roll()) } } else { 2 });
    }
    blocks::drops_by_luck(state, tool.map(|stack| stack.item), roll())
}

/// Silk Touch подавляет опыт руд/скалка; неверный инструмент не даёт
/// опыта даже при разрушении блока. Спавнер Silk Touch не сохраняет.
pub fn experience(state: i32, tool: Option<Stack>, roll: f32) -> i32 {
    if !super::harvests(state, tool) { return 0; }
    let Some(name) = blocks::block_at_state(state) else { return 0; };
    if name != "spawner" && tool.is_some_and(|stack| stack.extras().level("silk_touch") > 0) { return 0; }
    if name == "creaking_heart" {
        return if blocks::orientation_of(state).and_then(|block| block.value(state, "natural")) == Some("true") {
            20 + uniform(roll, 5)
        } else { 0 };
    }
    crate::survival::ore_xp(name, roll)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tool(name: &str, silk: i32, fortune: i32) -> Option<Stack> {
        let mut data = crate::stackdata::Data::default();
        data.set_enchant(crate::stackdata::enchantment_number("silk_touch").unwrap(), silk);
        data.set_enchant(crate::stackdata::enchantment_number("fortune").unwrap(), fortune);
        Some(Stack::new(blocks::item_named(name).unwrap(), 1).with_extras(data))
    }
    fn state(name: &str) -> i32 { blocks::state_by_name(name).unwrap() }
    fn check_drop(block: &str, held: Option<Stack>, roll: f32, item: &str, count: i32) {
        let dropped = drops(state(block), held, || roll).unwrap();
        assert_eq!((blocks::item_name(dropped.item), dropped.count), (Some(item), count), "{block}");
    }

    #[test]
    fn silk_preserves_ores_and_glass_without_experience_and_never_bypasses_tool_tier() {
        let silk = tool("diamond_pickaxe", 1, 3);
        for block in ["coal_ore", "deepslate_diamond_ore", "redstone_ore", "lapis_ore", "glass", "blue_ice", "stone", "bookshelf", "oak_leaves"] {
            check_drop(block, silk, 0.99, block, 1);
            assert_eq!(experience(state(block), silk, 0.99), 0);
        }
        for wrong in [None, tool("wooden_pickaxe", 1, 3), tool("diamond_shovel", 1, 3)] {
            assert!(drops(state("diamond_ore"), wrong, || 0.99).is_none());
            assert_eq!(experience(state("diamond_ore"), wrong, 0.99), 0);
        }
        assert_eq!(experience(state("diamond_ore"), tool("diamond_pickaxe", 0, 3), 0.99), 7);
        assert!(experience(state("spawner"), silk, 0.5) > 0);
    }

    #[test]
    fn fortune_ore_distribution_and_independent_base_count() {
        for (level, expected) in [(0, vec![1, 1]), (1, vec![1, 1, 2]), (2, vec![1, 1, 2, 3]), (3, vec![1, 1, 2, 3, 4])] {
            for (index, count) in expected.iter().enumerate() {
                check_drop("diamond_ore", tool("diamond_pickaxe", 0, level), (index as f32 + 0.5) / expected.len() as f32, "diamond", *count);
            }
        }
        for (block, item, low, high) in [("copper_ore", "raw_copper", 2, 5), ("lapis_ore", "lapis_lazuli", 4, 9), ("nether_gold_ore", "gold_nugget", 2, 6)] {
            check_drop(block, tool("diamond_pickaxe", 0, 0), 0.0, item, low);
            check_drop(block, tool("diamond_pickaxe", 0, 0), 0.999, item, high);
            let mut rolls = [0.0, 0.999].into_iter();
            assert_eq!(drops(state(block), tool("diamond_pickaxe", 0, 3), || rolls.next().unwrap()).unwrap().count, low * 4);
        }
    }

    #[test]
    fn fortune_additive_caps_and_silk_gravel_precedence() {
        let fortune = tool("diamond_pickaxe", 0, 3);
        for (block, item, count) in [("redstone_ore", "redstone", 8), ("glowstone", "glowstone_dust", 4), ("sea_lantern", "prismarine_crystals", 5), ("melon", "melon_slice", 9)] {
            check_drop(block, fortune, 0.999, item, count);
        }
        check_drop("gravel", fortune, 0.999, "flint", 1);
        check_drop("gravel", tool("diamond_shovel", 1, 3), 0.0, "gravel", 1);
        check_drop("gravel", None, 0.099, "flint", 1);
        check_drop("gravel", None, 0.1, "gravel", 1);
        check_drop("oak_planks", fortune, 0.999, "oak_planks", 1);
    }

    #[test]
    fn leaves_can_drop_sapling_sticks_and_apple_together() {
        let result = block_drops(state("oak_leaves"), tool("diamond_hoe", 0, 3), || 0.0);
        let named: Vec<_> = result.iter().map(|stack| (blocks::item_name(stack.item).unwrap(), stack.count)).collect();
        assert_eq!(named, vec![("oak_sapling", 1), ("stick", 1), ("apple", 1)]);
        assert!(block_drops(state("oak_leaves"), None, || 0.99).is_empty());
        for held in [tool("shears", 0, 3), tool("diamond_hoe", 1, 3)] {
            let result = block_drops(state("oak_leaves"), held, || panic!("ножницы/Silk без бросков"));
            assert_eq!(result, vec![Stack::new(blocks::item_named("oak_leaves").unwrap(), 1)]);
        }
        for (leaf, sapling) in [("jungle_leaves", "jungle_sapling"), ("azalea_leaves", "azalea"), ("flowering_azalea_leaves", "flowering_azalea"), ("pale_oak_leaves", "pale_oak_sapling")] {
            let result = block_drops(state(leaf), None, || 0.0);
            assert_eq!(blocks::item_name(result[0].item), Some(sapling));
            assert!(result.iter().all(|stack| blocks::item_name(stack.item) != Some("apple")));
        }
        let mangrove = block_drops(state("mangrove_leaves"), None, || 0.0);
        assert_eq!(mangrove, vec![Stack::new(blocks::item_named("stick").unwrap(), 1)]);
    }

    #[test]
    fn fortune_changes_leaf_chances_and_grass_quantity_not_seed_chance() {
        for level in 0..=3 {
            let held = tool("diamond_hoe", 0, level);
            let mut rolls = [0.124, 0.999].into_iter();
            let seed = drops(state("short_grass"), held, || rolls.next().unwrap()).unwrap();
            assert_eq!((blocks::item_name(seed.item), seed.count), (Some("wheat_seeds"), 1 + 2 * level));
            assert!(drops(state("short_grass"), held, || 0.125).is_none());
        }
        let mut rolls = [0.075, 0.99, 0.99].into_iter();
        assert!(block_drops(state("oak_leaves"), tool("diamond_hoe", 0, 1), || rolls.next().unwrap()).is_empty());
        let mut rolls = [0.075, 0.99, 0.99].into_iter();
        let result = block_drops(state("oak_leaves"), tool("diamond_hoe", 0, 2), || rolls.next().unwrap());
        assert_eq!(result, vec![Stack::new(blocks::item_named("oak_sapling").unwrap(), 1)]);
        check_drop("short_grass", tool("shears", 0, 3), 0.0, "short_grass", 1);
    }

    #[test]
    fn snow_requires_shovel_and_silk_preserves_layers_and_blocks() {
        assert!(drops(state("snow"), None, || 0.0).is_none());
        let shovel = tool("diamond_shovel", 0, 3);
        let silk = tool("diamond_shovel", 1, 0);
        for layers in 1..=8 {
            let state = blocks::state_from_text(&format!("snow[layers={layers}]")).unwrap();
            let plain = drops(state, shovel, || 0.0).unwrap();
            assert_eq!((blocks::item_name(plain.item), plain.count), (Some("snowball"), layers));
            let kept = drops(state, silk, || 0.0).unwrap();
            assert_eq!((blocks::item_name(kept.item), kept.count),
                if layers == 8 { (Some("snow_block"), 1) } else { (Some("snow"), layers) });
        }
        check_drop("snow_block", shovel, 0.0, "snowball", 4);
        check_drop("snow_block", silk, 0.0, "snow_block", 1);
        assert!(block_drops(state("frosted_ice"), tool("diamond_pickaxe", 1, 0), || 0.0).is_empty());
    }

    #[test]
    fn special_infested_and_heart_drops_share_java_rules() {
        // Нужен Silk, но не кирка: заражённый камень превращается в обычный.
        check_drop("infested_stone", tool("diamond_hoe", 1, 0), 0.0, "stone", 1);
        assert!(block_drops(state("infested_stone"), tool("diamond_pickaxe", 0, 3), || 0.0).is_empty());
        let natural = blocks::state_from_text("creaking_heart[natural=true]").unwrap();
        let placed = blocks::state_from_text("creaking_heart[natural=false]").unwrap();
        let plain = tool("diamond_axe", 0, 0);
        let fortune = tool("diamond_axe", 0, 3);
        let silk = tool("diamond_axe", 1, 3);
        for held in [plain, fortune] {
            assert_eq!(experience(natural, held, 0.0), 20);
            assert_eq!(experience(natural, held, 0.999), 24);
            assert_eq!(experience(placed, held, 0.0), 0);
        }
        assert_eq!(experience(natural, silk, 0.0), 0);
        check_drop("creaking_heart", silk, 0.999, "creaking_heart", 1);
        check_drop("creaking_heart", fortune, 0.999, "resin_clump", 6);
    }
}
