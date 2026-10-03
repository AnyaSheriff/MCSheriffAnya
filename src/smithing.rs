// Кузнечный стол: улучшение до незерита и отделка брони.
//
// Три входа (шаблон, основа, добавка) и итог. Правила — вики (Smithing
// Table, Smithing Template, Armor Trim, Netherite Upgrade) и чёрный ящик
// (smithing-upgrade, smithing-trim, smithing-reject): ячейка принимает то, что
// годится в неё хоть по одному рецепту; итог забирается — уходит по одному из
// каждого входа; отделка поверх такой же (тот же узор и материал) итога не
// даёт, поверх другой — заменяет её.

use crate::blocks;
use crate::inventory::Stack;
use crate::stackdata::TRIM_PATTERNS;

/// Предметы-материалы отделки по номерам `TRIM_MATERIALS`.
const MATERIAL_ITEMS: [&str; 11] =
    ["amethyst_shard", "copper_ingot", "diamond", "emerald", "gold_ingot", "iron_ingot", "lapis_lazuli", "netherite_ingot", "quartz", "redstone", "resin_brick"];

const UPGRADE_TEMPLATE: &str = "netherite_upgrade_smithing_template";

/// Узор отделки по шаблону (`sentry_armor_trim_smithing_template`).
fn pattern_of(template: &str) -> Option<u8> {
    let name = template.strip_suffix("_armor_trim_smithing_template")?;

    TRIM_PATTERNS.iter().position(|known| *known == name).map(|number| number as u8)
}

fn is_template(name: &str) -> bool {
    name == UPGRADE_TEMPLATE || pattern_of(name).is_some()
}

/// Броня, на которую ложится отделка: вся, вместе с черепашьим панцирем
/// (вики, Armor Trim).
fn is_trimmable(name: &str) -> bool {
    name == "turtle_helmet" || ["_helmet", "_chestplate", "_leggings", "_boots"].iter().any(|part| name.ends_with(part))
}

/// Что улучшается до незерита: алмазные инструменты, оружие, броня, броня
/// коню и наутилусу (вики, Smithing Table; чёрный ящик smithing-reject).
pub const UPGRADES: [&str; 12] =
    ["sword", "shovel", "pickaxe", "axe", "hoe", "helmet", "chestplate", "leggings", "boots", "horse_armor", "spear", "nautilus_armor"];

/// Что получится из алмазной вещи на столе: её незеритовая пара, если есть.
fn netherite_of(name: &str) -> Option<i32> {
    let rest = name.strip_prefix("diamond_").filter(|rest| UPGRADES.contains(rest))?;

    blocks::item_named(&format!("netherite_{rest}"))
}

fn is_upgradable(name: &str) -> bool {
    netherite_of(name).is_some()
}

fn material_of(name: &str) -> Option<u8> {
    MATERIAL_ITEMS.iter().position(|known| *known == name).map(|number| number as u8)
}

/// Годится ли предмет в ячейку стола (0 — шаблон, 1 — основа, 2 — добавка).
pub fn may_place(slot: usize, stack: &Stack) -> bool {
    let Some(name) = blocks::item_name(stack.item) else {
        return false;
    };

    match slot {
        0 => is_template(name),
        1 => is_upgradable(name) || is_trimmable(name),
        2 => name == "netherite_ingot" || material_of(name).is_some(),
        _ => false,
    }
}

/// Итог по трём ячейкам; None — рецепта нет.
pub fn result(template: Option<Stack>, base: Option<Stack>, addition: Option<Stack>) -> Option<Stack> {
    let (template, base, addition) = (template?, base?, addition?);
    let template_name = blocks::item_name(template.item)?;
    let base_name = blocks::item_name(base.item)?;
    let addition_name = blocks::item_name(addition.item)?;

    if template_name == UPGRADE_TEMPLATE {
        // Алмазное — в незеритовое; износ, зачарования и имя остаются.
        if addition_name != "netherite_ingot" {
            return None;
        }

        let item = netherite_of(base_name)?;

        return Some(Stack { item, count: 1, ..base });
    }

    let pattern = pattern_of(template_name)?;
    let material = material_of(addition_name)?;

    if !is_trimmable(base_name) {
        return None;
    }

    let mut data = base.extras();

    if data.trim == Some((material, pattern)) {
        return None;
    }

    data.trim = Some((material, pattern));

    Some(Stack { count: 1, ..base }.with_extras(data))
}

/// Свойство окна 0 (`hasRecipeError`): все три ячейки заняты, а итога нет —
/// клиент рисует красный крест (чёрный ящик smithing-trim: 1 при той же
/// отделке, 0 после того, как убрали добавку).
pub fn recipe_error(inputs: &[Option<Stack>], result: Option<Stack>) -> bool {
    result.is_none() && inputs.iter().take(3).all(Option::is_some)
}

/// Шаблон улучшения до незерита.
pub fn upgrade_template() -> Option<i32> {
    blocks::item_named(UPGRADE_TEMPLATE)
}

/// Пары «алмазная вещь — незеритовая» для рецептов Bedrock.
pub fn upgrade_pairs() -> Vec<(i32, i32)> {
    UPGRADES
        .iter()
        .filter_map(|rest| Some((blocks::item_named(&format!("diamond_{rest}"))?, blocks::item_named(&format!("netherite_{rest}"))?)))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str) -> Stack {
        Stack::new(blocks::item_named(name).expect(name), 1)
    }

    #[test]
    fn upgrade_keeps_the_extras() {
        let mut data = crate::stackdata::Data { damage: 50, ..Default::default() };

        data.set_enchant(28, 3);

        let chest = item("diamond_chestplate").with_extras(data.clone());
        let out = result(Some(item(UPGRADE_TEMPLATE)), Some(chest), Some(item("netherite_ingot"))).expect("итог");

        assert_eq!(blocks::item_name(out.item), Some("netherite_chestplate"));
        assert_eq!(out.extras(), data);
        // Железо и другой материал — не рецепт.
        assert!(result(Some(item(UPGRADE_TEMPLATE)), Some(item("iron_sword")), Some(item("netherite_ingot"))).is_none());
        assert!(result(Some(item(UPGRADE_TEMPLATE)), Some(item("diamond_sword")), Some(item("gold_ingot"))).is_none());
        assert!(result(Some(item(UPGRADE_TEMPLATE)), Some(item("diamond_spear")), Some(item("netherite_ingot"))).is_some());
    }

    #[test]
    fn trim_replaces_but_not_repeats() {
        let template = item("sentry_armor_trim_smithing_template");
        let out = result(Some(template), Some(item("iron_helmet")), Some(item("gold_ingot"))).expect("итог");

        assert_eq!(out.extras().trim, Some((4, 8)));
        assert!(result(Some(template), Some(out), Some(item("gold_ingot"))).is_none());

        let other = result(Some(template), Some(out), Some(item("diamond"))).expect("замена");

        assert_eq!(other.extras().trim, Some((2, 8)));
        assert!(result(Some(item(UPGRADE_TEMPLATE)), Some(item("iron_helmet")), Some(item("netherite_ingot"))).is_none());
    }

    #[test]
    fn slots_take_their_own() {
        assert!(may_place(0, &item("coast_armor_trim_smithing_template")));
        assert!(!may_place(0, &item("iron_ingot")));
        assert!(may_place(1, &item("turtle_helmet")) && may_place(1, &item("diamond_pickaxe")));
        assert!(!may_place(1, &item("iron_sword")) && !may_place(1, &item("elytra")) && !may_place(1, &item("diamond_block")));
        assert!(may_place(2, &item("resin_brick")) && may_place(2, &item("netherite_ingot")) && !may_place(2, &item("gold_nugget")));
    }
}
