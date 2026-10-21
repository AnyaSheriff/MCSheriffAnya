// Добыча по minecraft.wiki/w/Breaking: «All operations are floating-point».
// Таблица твёрдости и допустимых инструментов — только minecraft-data JSON.

use crate::{blocks, inventory::Stack};
use crate::gameplay_catalog::{Catalog, Profile};

const BASE: Catalog = Catalog::new(Profile::Java26_1_2);

mod drops;
pub use drops::{block_drops, experience};

pub fn properties(state: i32) -> (f32, &'static str, &'static [i32]) {
    let table = crate::hardness_table::MINING;
    let index = table.partition_point(|&(_, high, ..)| high < state);
    table.get(index).filter(|&&(low, ..)| low <= state)
        .map_or((-1.0, "default", &[]), |&(_, _, hardness, material, tools)| (hardness, material, tools))
}

pub fn properties_with_catalog(catalog: Catalog, state: i32) -> (f32, &'static str, &'static [i32]) {
    catalog.mining_properties(state).unwrap_or((-1.0, "default", &[]))
}

pub fn harvests(state: i32, tool: Option<Stack>) -> bool {
    harvests_with_catalog(BASE, state, tool)
}

pub fn harvests_with_catalog(catalog: Catalog, state: i32, tool: Option<Stack>) -> bool {
    let Ok((_, _, tools)) = catalog.mining_properties(state) else { return false; };
    tools.is_empty() || tool.is_some_and(|stack| tools.contains(&stack.item))
}

fn tool_speed(catalog: Catalog, state: i32, tool: Option<Stack>) -> f32 {
    let Some(name) = tool.and_then(|stack| crate::combat::item_name(stack.item)) else { return 1.0; };
    let (_, material, _) = properties_with_catalog(catalog, state);
    let block = blocks::block_at_state(state).unwrap_or("");
    if name == "shears" {
        return if block == "cobweb" || material.contains("leaves") { 15.0 }
            else if material == "wool" { 5.0 }
            else if matches!(block, "vine" | "glow_lichen" | "tripwire") { 2.0 }
            else { 1.0 };
    }
    if name.ends_with("_sword") {
        return if material == "sword_instantly_mines" { 30.0 }
            else if block == "cobweb" { 15.0 }
            else if material.contains("plant") || material.contains("leaves") || material.contains("gourd") { 1.5 }
            else { 1.0 };
    }
    let Some((tier, kind)) = name.split_once('_') else { return 1.0; };
    let effective = material.split(';').any(|part| part.strip_prefix("mineable/") == Some(kind))
        || (material == "incorrect_for_wooden_tool" && kind == "pickaxe");
    if !effective { return 1.0; }
    match tier {
        "wooden" => 2.0, "stone" => 4.0, "copper" => 5.0, "iron" => 6.0,
        "diamond" => 8.0, "netherite" => 9.0, "golden" => 12.0, _ => 1.0,
    }
}

#[derive(Clone, Copy)]
pub struct Conditions {
    pub haste: i32,
    pub fatigue: i32,
    pub underwater: bool,
    pub on_ground: bool,
    pub efficiency: f32,
    pub block_speed: f32,
    pub submerged_speed: f32,
}

/// Доля блока за такт: speed / hardness / (30 или 100).
#[cfg(test)]
pub fn progress(state: i32, tool: Option<Stack>, env: Conditions) -> f32 {
    progress_with_catalog(BASE, state, tool, env)
}

pub fn progress_with_catalog(catalog: Catalog, state: i32, tool: Option<Stack>, env: Conditions) -> f32 {
    let (hardness, _, _) = properties_with_catalog(catalog, state);
    if hardness < 0.0 { return 0.0; }
    if crate::hardness_table::INSTANT.iter().any(|&(low, high)| (low..=high).contains(&state)) || hardness == 0.0 { return 1.0; }
    let mut speed = tool_speed(catalog, state, tool);
    if speed > 1.0 { speed += env.efficiency; }
    speed *= 1.0 + 0.2 * env.haste.max(0) as f32;
    speed *= match env.fatigue { 0 => 1.0, 1 => 0.3, 2 => 0.09, 3 => 0.0027, _ => 0.00081 };
    speed *= env.block_speed;
    if env.underwater { speed *= env.submerged_speed; }
    if !env.on_ground { speed /= 5.0; }
    speed / hardness / if harvests_with_catalog(catalog, state, tool) { 30.0 } else { 100.0 }
}

/// [проверить] Порог FINISH 70% — из задания; первый такт входит в счёт.
pub fn finished(progress: f32, since: u64) -> bool {
    progress * (since.saturating_add(1) as f32) >= 0.7
}

pub fn wear(state: i32, tool: Stack) -> i32 {
    wear_with_catalog(BASE, state, tool)
}

pub fn wear_with_catalog(catalog: Catalog, state: i32, tool: Stack) -> i32 {
    if properties_with_catalog(catalog, state).0 <= 0.0 { return 0; }
    let name = crate::combat::item_name(tool.item).unwrap_or("");
    if name.ends_with("_sword") { 2 }
    else if ["_pickaxe", "_axe", "_shovel", "_hoe"].iter().any(|suffix| name.ends_with(suffix)) || name == "shears" { 1 }
    else { 0 }
}

/// Бетонные плиты используют общий расчёт инструмента; особые старые выпадения
/// (листва, Silk Touch и Fortune) остаются в прежнем обработчике.
pub fn block_drops_with_catalog(catalog: Catalog, state: i32, tool: Option<Stack>, mut roll: impl FnMut() -> f32) -> Vec<Stack> {
    match catalog.slab_entry(state) {
        Ok(Some(_)) => catalog.drops_by_luck(state, tool.map(|stack| stack.item), roll()).ok().flatten().into_iter().collect(),
        Ok(None) => block_drops(state, tool, roll),
        Err(_) => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay_catalog::{WHITE_SLAB_FIRST_STATE, WHITE_SLAB_LAST_STATE};
    fn state(name: &str) -> i32 { blocks::state_by_name(name).unwrap() }
    fn tool(name: &str) -> Option<Stack> { Some(Stack::new(blocks::item_named(name).unwrap(), 1)) }
    fn env() -> Conditions { Conditions { haste: 0, fatigue: 0, underwater: false, on_ground: true, efficiency: 0.0, block_speed: 1.0, submerged_speed: 0.2 } }

    #[test]
    fn preview_slab_uses_shared_mining_and_guarded_drops() {
        let catalog = Catalog::new(Profile::Preview26_3);
        let pickaxe = tool("iron_pickaxe");
        assert!(!harvests_with_catalog(catalog, i32::MAX, pickaxe));
        let concrete = state("white_concrete");
        for slab in WHITE_SLAB_FIRST_STATE..=WHITE_SLAB_LAST_STATE {
            assert_eq!(progress_with_catalog(catalog, slab, pickaxe, env()), progress(concrete, pickaxe, env()));
            assert_eq!(progress_with_catalog(catalog, slab, None, env()), progress(concrete, None, env()));
            assert!(harvests_with_catalog(catalog, slab, pickaxe));
            assert!(!harvests_with_catalog(catalog, slab, None));
            assert_eq!(wear_with_catalog(catalog, slab, pickaxe.unwrap()), 1);
            let drops = block_drops_with_catalog(catalog, slab, pickaxe, || 0.5);
            assert_eq!(drops.len(), 1);
            assert_eq!(drops[0].item, crate::gameplay_catalog::WHITE_SLAB_ITEM);
            assert_eq!(drops[0].count, if slab >= WHITE_SLAB_FIRST_STATE + 4 { 2 } else { 1 });
            assert!(block_drops_with_catalog(catalog, slab, None, || 0.5).is_empty());
            assert_eq!(progress_with_catalog(BASE, slab, pickaxe, env()), 0.0);
            assert!(!harvests_with_catalog(BASE, slab, pickaxe));
            assert_eq!(wear_with_catalog(BASE, slab, pickaxe.unwrap()), 0);
            assert!(block_drops_with_catalog(BASE, slab, pickaxe, || 0.5).is_empty());
        }
    }

    #[test]
    fn tools_tiers_and_harvest_penalty() {
        let stone = state("stone");
        assert_eq!(progress(stone, None, env()), 1.0 / 1.5 / 100.0);
        for (name, speed) in [("wooden", 2.0), ("stone", 4.0), ("copper", 5.0), ("iron", 6.0), ("diamond", 8.0), ("netherite", 9.0), ("golden", 12.0)] {
            assert_eq!(progress(stone, tool(&format!("{name}_pickaxe")), env()), speed / 1.5 / 30.0);
        }
        assert_eq!(progress(state("obsidian"), tool("golden_pickaxe"), env()), 12.0 / 50.0 / 100.0);
        assert!(!harvests(stone, tool("diamond_shovel")));
        assert!(harvests(state("dirt"), None));
    }

    #[test]
    fn effects_water_air_and_instant_break() {
        let stone = state("stone");
        let pick = tool("diamond_pickaxe");
        let mut e = env(); e.haste = 2; e.efficiency = 26.0;
        assert!(progress(stone, pick, e) >= 1.0);
        let base = progress(stone, pick, env());
        for (level, multiplier) in [(1, 0.3), (2, 0.09), (3, 0.0027), (4, 0.00081), (255, 0.00081)] {
            let mut e = env(); e.fatigue = level;
            assert!((progress(stone, pick, e) - base * multiplier).abs() < 1e-7);
        }
        let mut e = env(); e.underwater = true; e.on_ground = false;
        assert!((progress(stone, pick, e) - base / 25.0).abs() < 1e-7);
        e.submerged_speed = 1.0;
        assert!((progress(stone, pick, e) - base / 5.0).abs() < 1e-7);
        assert_eq!(progress(state("torch"), None, e), 1.0);
        assert_eq!(progress(state("bedrock"), pick, e), 0.0);
        assert!(!finished(0.1, 5)); assert!(finished(0.1, 6));
    }

    #[test]
    fn mining_wear_and_special_tools() {
        assert_eq!(wear(state("stone"), tool("iron_pickaxe").unwrap()), 1);
        assert_eq!(wear(state("stone"), tool("iron_sword").unwrap()), 2);
        assert_eq!(wear(state("torch"), tool("iron_pickaxe").unwrap()), 0);
        assert_eq!(wear(state("stone"), tool("bow").unwrap()), 0);
        assert_eq!(tool_speed(BASE, state("cobweb"), tool("shears")), 15.0);
        assert_eq!(progress(state("bamboo"), tool("iron_sword"), env()), 1.0);
    }
}
