// Станки для игроков Bedrock: верстак и сетка 2×2, стол зачарования,
// наковальня, точило; окна печей и стойки.
//
// Расчёты — общие с Java (crafting.rs, enchanting.rs, furnace.rs): здесь
// только то, как это выглядит в протоколе Bedrock. Источники: описание
// протокола minecraft-data 1.26.10 (CraftingData, RecipeIngredient,
// EnchantOption, ItemStackRequest, ContainerSetData), документация Mojang
// к r/26_u1 (additional_docs/ServerAuthInventory/PlayerUIContainer.md —
// номера ячеек экранов; html/enums.html — ContainerEnumName и ContainerType).
//
// Как это устроено у Bedrock. Предметы на верстаке, в наковальне, на столе
// и на точиле лежат не в мире, а в «контейнере интерфейса игрока» (Player UI
// Container): клиент перекладывает их запросами, называя вид контейнера и
// номер ячейки. Итог (крафт, наковальня, точило, зачарование) рождается в
// особой ячейке «Created Item Output» (контейнер 60, ячейка 50): запрос
// «скрафтить рецепт N», затем «израсходовать то-то» и «взять итог».
// Рецепты клиент знает из пакета Crafting Data; номера рецептов — наши.

use std::sync::OnceLock;

use super::codec::Out;
use super::inventory::bedrock_item;
use super::session;
use crate::crafting::{self, Recipe};
use crate::enchanting;
use crate::inventory::Stack;

/// Виды контейнеров интерфейса (ContainerEnumName), которые называет клиент.
pub const ANVIL_INPUT: u8 = 0;
pub const ANVIL_MATERIAL: u8 = 1;
pub const CRAFTING_INPUT: u8 = 13;
pub const ENCHANTING_INPUT: u8 = 22;
pub const ENCHANTING_LAPIS: u8 = 23;
pub const FURNACE_FUEL: u8 = 24;
pub const FURNACE_INGREDIENT: u8 = 25;
pub const FURNACE_RESULT: u8 = 26;
pub const BREWING_INPUT: u8 = 9;
pub const BREWING_RESULT: u8 = 10;
pub const BREWING_FUEL: u8 = 11;
pub const BLAST_FURNACE_INGREDIENT: u8 = 45;
pub const SMOKER_INGREDIENT: u8 = 46;
pub const GRINDSTONE_INPUT: u8 = 50;
pub const GRINDSTONE_ADDITIONAL: u8 = 51;

/// Ячейки контейнера интерфейса (PlayerUIContainer.md).
pub const UI_ANVIL_INPUT: u8 = 1;
pub const UI_ANVIL_MATERIAL: u8 = 2;
pub const UI_ENCHANTING_INPUT: u8 = 14;
pub const UI_ENCHANTING_LAPIS: u8 = 15;
pub const UI_GRINDSTONE_INPUT: u8 = 16;
pub const UI_GRINDSTONE_ADDITIONAL: u8 = 17;
pub const UI_GRID_2X2: u8 = 28;
pub const UI_GRID_3X3: u8 = 32;

/// Вид экрана (ContainerType) для Container Open.
pub const WORKBENCH: u8 = 1;
pub const ENCHANTMENT: u8 = 3;
pub const ANVIL: u8 = 5;
pub const GRINDSTONE: u8 = 26;

/// Станок игрока.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Workbench,
    Enchanting,
    Anvil,
    Grindstone,
}

impl Kind {
    /// Станок по имени блока (`crafting_table` и то, что есть у Java).
    pub fn of(block: &str) -> Option<Kind> {
        if block == "crafting_table" {
            return Some(Kind::Workbench);
        }

        enchanting::Station::of(block).map(|station| match station {
            enchanting::Station::Enchanting => Kind::Enchanting,
            enchanting::Station::Anvil => Kind::Anvil,
            enchanting::Station::Grindstone => Kind::Grindstone,
        })
    }

    pub fn screen(self) -> u8 {
        match self {
            Kind::Workbench => WORKBENCH,
            Kind::Enchanting => ENCHANTMENT,
            Kind::Anvil => ANVIL,
            Kind::Grindstone => GRINDSTONE,
        }
    }

    /// Какие ячейки интерфейса у станка (кроме сетки 2×2: она есть всегда).
    pub fn has_slot(self, slot: u8) -> bool {
        match self {
            Kind::Workbench => (UI_GRID_3X3..UI_GRID_3X3 + 9).contains(&slot),
            Kind::Enchanting => matches!(slot, UI_ENCHANTING_INPUT | UI_ENCHANTING_LAPIS),
            Kind::Anvil => matches!(slot, UI_ANVIL_INPUT | UI_ANVIL_MATERIAL),
            Kind::Grindstone => matches!(slot, UI_GRINDSTONE_INPUT | UI_GRINDSTONE_ADDITIONAL),
        }
    }
}

/// Ячейка интерфейса по виду контейнера и номеру, как их называет клиент.
/// None — такого в интерфейсе нет.
pub fn ui_slot(container: u8, slot: u8) -> Option<u8> {
    let ok = match container {
        ANVIL_INPUT => slot == UI_ANVIL_INPUT,
        ANVIL_MATERIAL => slot == UI_ANVIL_MATERIAL,
        CRAFTING_INPUT => (UI_GRID_2X2..UI_GRID_3X3 + 9).contains(&slot),
        ENCHANTING_INPUT => slot == UI_ENCHANTING_INPUT,
        ENCHANTING_LAPIS => slot == UI_ENCHANTING_LAPIS,
        GRINDSTONE_INPUT => slot == UI_GRINDSTONE_INPUT,
        GRINDSTONE_ADDITIONAL => slot == UI_GRINDSTONE_ADDITIONAL,
        _ => false,
    };

    ok.then_some(slot)
}

/// Ячейка сетки 2×2 (28–31) — доступна без станка.
pub fn is_grid_2x2(slot: u8) -> bool {
    (UI_GRID_2X2..UI_GRID_2X2 + 4).contains(&slot)
}

// ---------------------------------------------------------------------------
// Окна хранилищ с особыми ячейками: печи и стойка
// ---------------------------------------------------------------------------

/// Как клиент называет ячейки окна: (вид контейнера, номер) по порядку
/// ячеек Java. None — окно обычное (всё как LevelEntityContainer).
/// Печь: ячейка 0 — вход, 1 — топливо, 2 — выход; стойка Java: 0–2 —
/// бутылки, 3 — ингредиент, 4 — порошок, у Bedrock — ингредиент 0,
/// бутылки 1–3, порошок 4. [проверить] живым клиентом.
pub fn layout(kind: crate::containers::Kind) -> Option<Vec<(u8, u8)>> {
    use crate::containers::Kind as C;

    Some(match kind {
        C::Furnace => vec![(FURNACE_INGREDIENT, 0), (FURNACE_FUEL, 1), (FURNACE_RESULT, 2)],
        C::BlastFurnace => vec![(BLAST_FURNACE_INGREDIENT, 0), (FURNACE_FUEL, 1), (FURNACE_RESULT, 2)],
        C::Smoker => vec![(SMOKER_INGREDIENT, 0), (FURNACE_FUEL, 1), (FURNACE_RESULT, 2)],
        C::BrewingStand => {
            vec![(BREWING_RESULT, 1), (BREWING_RESULT, 2), (BREWING_RESULT, 3), (BREWING_INPUT, 0), (BREWING_FUEL, 4)]
        }
        _ => return None,
    })
}

/// Container Set Data: свойство окна (печь: 0 — сколько жарится, 1 — сколько
/// ещё горит, 2 — на сколько топлива, 3 — накопленный опыт; стойка: 0 — сколько варится, 1 —
/// заряды порошка, 2 — всего зарядов).
pub fn set_data_packet(window: u8, property: i32, value: i32) -> Vec<u8> {
    let mut out = Out::packet(session::CONTAINER_SET_DATA);
    out.u8(window).zigzag32(property).zigzag32(value);
    out.bytes
}

/// Свойства Bedrock по свойствам Java (`furnace::properties`). У печи Java:
/// горит, топливо, жарится, нужно; клиент Bedrock ждёт «жарится» на шкале
/// 0–200 — у плавилен с меньшим временем масштабируем. [проверить] клиентом.
pub fn bedrock_properties(kind: crate::containers::Kind, java: &[i16]) -> Vec<(i32, i32)> {
    use crate::containers::Kind as C;

    let at = |index: usize| i32::from(java.get(index).copied().unwrap_or(0));

    match kind {
        C::Furnace | C::BlastFurnace | C::Smoker => {
            let total = at(3);
            let cook = if total > 0 { at(2) * 200 / total } else { 0 };

            // Четвёртое — накопленный опыт: BDS шлёт его вместе с тремя
            // остальными (снято по сети, tmp/br-bds); опыт печи Bedrock
            // пока не ведёт [проверить].
            vec![(0, cook), (1, at(0)), (2, at(1)), (3, 0)]
        }
        // Всего зарядов — 20, когда они есть; у пустой стойки BDS шлёт нули.
        C::BrewingStand => vec![(0, at(0)), (1, at(1)), (2, if at(1) > 0 { 20 } else { 0 })],
        _ => Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Crafting Data
// ---------------------------------------------------------------------------

/// Номер рецепта для клиента: с единицы (ноль нельзя), у вариантов одного
/// рецепта — разный.
fn network_id(variant: usize) -> u32 {
    variant as u32 + 1
}

/// Один вариант рецепта для клиента.
struct Variant {
    recipe: usize,
    /// Предметы клетки (по одному на вариант).
    cells: Vec<Cell>,
}

#[derive(Clone)]
enum Cell {
    Empty,
    Item(i32),
    Tag(&'static str),
}

/// Все варианты рецептов; номер варианта + 1 — номер рецепта у клиента.
fn variants() -> &'static [Variant] {
    static VARIANTS: OnceLock<Vec<Variant>> = OnceLock::new();

    VARIANTS.get_or_init(|| {
        let mut out = Vec::new();

        for (index, recipe) in crafting::recipes().iter().enumerate() {
            // Выходной предмет должен существовать у Bedrock.
            if bedrock_item(recipe.result).is_none() {
                continue;
            }

            // Для каждой клетки — как её описать: пусто, предмет, тег, либо
            // несколько предметов (тогда рецепт размножается по ним).
            let mut options: Vec<Vec<Cell>> = Vec::new();
            let mut usable = true;

            for cell in recipe.cells {
                if cell.is_empty() {
                    options.push(vec![Cell::Empty]);
                    continue;
                }

                let known: Vec<i32> = cell.iter().copied().filter(|item| bedrock_item(*item).is_some()).collect();

                if known.is_empty() {
                    usable = false;
                    break;
                }

                if known.len() == 1 {
                    options.push(vec![Cell::Item(known[0])]);
                } else if let Some(tag) = tag_of(&known) {
                    options.push(vec![Cell::Tag(tag)]);
                } else {
                    options.push(known.into_iter().map(Cell::Item).collect());
                }
            }

            if !usable {
                continue;
            }

            // Перебор вариантов, но не больше 12 на рецепт.
            let mut combos: Vec<Vec<Cell>> = vec![Vec::new()];

            for choices in &options {
                let mut next = Vec::new();

                for combo in &combos {
                    for choice in choices {
                        let mut with = combo.clone();
                        with.push(choice.clone());
                        next.push(with);
                    }
                }

                combos = next;

                if combos.len() > 12 {
                    combos.truncate(12);
                }
            }

            out.extend(combos.into_iter().map(|cells| Variant { recipe: index, cells }));
        }

        out
    })
}

/// Тег Bedrock для набора предметов (по именам). [проверить] имена тегов.
fn tag_of(items: &[i32]) -> Option<&'static str> {
    let names: Vec<&str> = items.iter().filter_map(|item| crate::blocks::item_name(*item)).collect();

    if names.len() != items.len() || names.len() < 4 {
        return None;
    }

    if names.iter().all(|name| name.ends_with("_planks")) {
        Some("minecraft:planks")
    } else if names.iter().all(|name| name.ends_with("_wool")) {
        Some("minecraft:wool")
    } else if names.iter().all(|name| name.ends_with("_slab") && !name.contains("stone") && !name.contains("brick")) {
        Some("minecraft:wooden_slabs")
    } else if names.iter().all(|name| name.contains("log") || name.contains("wood") || name.contains("stem") || name.contains("hyphae")) {
        Some("minecraft:logs")
    } else {
        None
    }
}

/// Рецепт по номеру, присланному клиентом.
pub fn recipe_by_network_id(id: u32) -> Option<&'static Recipe> {
    let variant = variants().get(usize::try_from(id).ok()?.checked_sub(1)?)?;

    crafting::recipes().get(variant.recipe)
}

fn write_ingredient(out: &mut Out, cell: &Cell) {
    match cell {
        Cell::Empty => {
            out.u8(0).zigzag32(0);
        }
        Cell::Item(java) => {
            let (id, metadata, _) = bedrock_item(*java).unwrap_or((0, 0, 0));

            out.u8(1).li16(id as i16).li16(metadata as i16).zigzag32(1);
        }
        Cell::Tag(tag) => {
            out.u8(3).string(tag).zigzag32(1);
        }
    }
}

/// Предмет рецепта (ItemLegacy): без номера стопки.
fn write_output(out: &mut Out, item: i32, count: i32) {
    let (id, metadata, block) = bedrock_item(item).unwrap_or((0, 0, 0));

    out.zigzag32(id).lu16(count as u16).varint(metadata).zigzag32(block);

    // У щита после дополнительных данных ещё blocking_tick (как в write_item).
    if id == super::inventory::SHIELD {
        out.varint(18).lu16(0).li32(0).li32(0).li64(0);
    } else {
        out.varint(10).lu16(0).li32(0).li32(0);
    }
}

/// Пакет Crafting Data со всеми рецептами верстака. Печь и зелья клиенту
/// не нужны: считает сервер.
pub fn crafting_data_packet() -> &'static [u8] {
    static PACKET: OnceLock<Vec<u8>> = OnceLock::new();

    PACKET.get_or_init(|| {
        let mut out = Out::packet(session::CRAFTING_DATA);
        let all = variants();

        out.varint(all.len() as u32);

        for (number, variant) in all.iter().enumerate() {
            let recipe = &crafting::recipes()[variant.recipe];
            let mut uuid = [0u8; 16];

            uuid[..8].copy_from_slice(b"mcsacraf");
            uuid[8..12].copy_from_slice(&(number as u32).to_be_bytes());

            let name = format!("mcsa:recipe_{number}");

            if recipe.shaped {
                out.zigzag32(1).string(&name).zigzag32(i32::from(recipe.width)).zigzag32(i32::from(recipe.height));

                for cell in &variant.cells {
                    write_ingredient(&mut out, cell);
                }

                out.varint(1);
                write_output(&mut out, recipe.result, recipe.count);
                out.uuid(uuid).string("crafting_table").zigzag32(0).bool(true);
            } else {
                out.zigzag32(0).string(&name).varint(variant.cells.len() as u32);

                for cell in &variant.cells {
                    write_ingredient(&mut out, cell);
                }

                out.varint(1);
                write_output(&mut out, recipe.result, recipe.count);
                out.uuid(uuid).string("crafting_table").zigzag32(0);
            }

            // Всегда открыт (RecipeUnlockingRequirement: always_unlocked).
            out.u8(1).varint(network_id(number));
        }

        // Зелья, переливание, редукторы — нет; «очистить прежние» — да.
        out.varint(0).varint(0).varint(0).bool(true);
        out.bytes
    })
}

// ---------------------------------------------------------------------------
// Стол зачарования
// ---------------------------------------------------------------------------

/// Номера предложений стола для клиента: далеко от номеров рецептов,
/// зависят от семени, чтобы после зачарования старые не годились.
const OFFER_BASE: u32 = 10_000;

pub fn offer_id(seed: i32, slot: usize) -> u32 {
    OFFER_BASE + (seed.unsigned_abs() % 100_000) * 3 + slot as u32
}

/// Какое предложение (0–2) названо номером; None — не от этого семени.
pub fn offer_slot(seed: i32, id: u32) -> Option<usize> {
    let rest = id.checked_sub(OFFER_BASE)?;

    (rest / 3 == seed.unsigned_abs() % 100_000).then_some((rest % 3) as usize)
}

/// Номер зачарования Bedrock по имени Java (minecraft.wiki, Bedrock Edition
/// data values, Enchantment IDs). [проверить]
pub fn bedrock_enchantment(name: &str) -> Option<u8> {
    Some(match name {
        "protection" => 0,
        "fire_protection" => 1,
        "feather_falling" => 2,
        "blast_protection" => 3,
        "projectile_protection" => 4,
        "thorns" => 5,
        "respiration" => 6,
        "depth_strider" => 7,
        "aqua_affinity" => 8,
        "sharpness" => 9,
        "smite" => 10,
        "bane_of_arthropods" => 11,
        "knockback" => 12,
        "fire_aspect" => 13,
        "looting" => 14,
        "efficiency" => 15,
        "silk_touch" => 16,
        "unbreaking" => 17,
        "fortune" => 18,
        "power" => 19,
        "punch" => 20,
        "flame" => 21,
        "infinity" => 22,
        "luck_of_the_sea" => 23,
        "lure" => 24,
        "frost_walker" => 25,
        "mending" => 26,
        "binding_curse" => 27,
        "vanishing_curse" => 28,
        "impaling" => 29,
        "riptide" => 30,
        "loyalty" => 31,
        "channeling" => 32,
        "multishot" => 33,
        "piercing" => 34,
        "quick_charge" => 35,
        "soul_speed" => 36,
        "swift_sneak" => 37,
        "wind_burst" => 38,
        "density" => 39,
        "breach" => 40,
        _ => return None,
    })
}

/// Player Enchant Options: предложения стола для предмета на нём. Пустой
/// список — нечего предлагать. Подсказка у каждого предложения — одно
/// зачарование (как у Java, ячейки 4–9 свойств окна). Номер предложения —
/// беззнаковый varint, как у остальных номеров рецептов (документация
/// Mojang: TypedServerNetId<RecipeNetIdTag, unsigned int>).
pub fn enchant_options_packet(seed: i32, shelves: i32, item: Option<Stack>) -> Vec<u8> {
    let props = enchanting::properties(seed, shelves, item);
    let mut out = Out::packet(session::PLAYER_ENCHANT_OPTIONS);
    let offers: Vec<usize> = (0..3).filter(|&slot| props[slot] > 0).collect();

    out.varint(offers.len() as u32);

    for slot in offers {
        let clue = props[4 + slot];
        let level = props[7 + slot];
        let name = crate::stackdata::ENCHANTMENTS.get(clue.max(0) as usize).map_or("", |(name, _)| *name);
        let bedrock = bedrock_enchantment(name);

        out.varint(props[slot] as u32).li32(0);

        // Три списка (надеты, в руке, у самого предмета): подсказка — у самого.
        out.varint(0).varint(0);

        match (clue >= 0, bedrock) {
            (true, Some(id)) => {
                out.varint(1).u8(id).u8(level.clamp(0, 255) as u8);
            }
            _ => {
                out.varint(0);
            }
        }

        out.string(&title(name)).varint(offer_id(seed, slot));
    }

    out.bytes
}

fn title(name: &str) -> String {
    let mut text = String::new();

    for (index, word) in name.split('_').enumerate() {
        if index > 0 {
            text.push(' ');
        }

        let mut letters = word.chars();

        if let Some(first) = letters.next() {
            text.extend(first.to_uppercase());
            text.push_str(letters.as_str());
        }
    }

    text
}

// ---------------------------------------------------------------------------
// Данные предмета для клиента
// ---------------------------------------------------------------------------

/// NBT предмета Bedrock (little-endian) для зачарований, износа, цены
/// работ и имени: `{ench:[{id:s,lvl:s}], Damage:i, RepairCost:i,
/// display:{Name:""}}`. None — довесков нет.
pub fn item_nbt(stack: &Stack) -> Option<Vec<u8>> {
    const INT: u8 = 3;
    const SHORT: u8 = 2;
    const STRING: u8 = 8;
    const LIST: u8 = 9;
    const COMPOUND: u8 = 10;

    fn name(out: &mut Vec<u8>, value: &str) {
        out.extend_from_slice(&(value.len() as u16).to_le_bytes());
        out.extend_from_slice(value.as_bytes());
    }

    let data = stack.extras();
    let enchants: Vec<(u8, i32)> = data
        .enchants
        .iter()
        .chain(&data.stored)
        .filter_map(|&(number, level)| Some((bedrock_enchantment(crate::stackdata::ENCHANTMENTS.get(usize::from(number))?.0)?, level)))
        .collect();

    if enchants.is_empty() && data.damage == 0 && data.repair_cost == 0 && data.name.is_none() {
        return None;
    }

    let mut out = vec![COMPOUND];
    name(&mut out, "");

    if !enchants.is_empty() {
        out.push(LIST);
        name(&mut out, "ench");
        out.push(COMPOUND);
        out.extend_from_slice(&(enchants.len() as i32).to_le_bytes());

        for (id, level) in enchants {
            out.push(SHORT);
            name(&mut out, "id");
            out.extend_from_slice(&i16::from(id).to_le_bytes());
            out.push(SHORT);
            name(&mut out, "lvl");
            out.extend_from_slice(&(level as i16).to_le_bytes());
            out.push(0);
        }
    }

    if data.damage != 0 {
        out.push(INT);
        name(&mut out, "Damage");
        out.extend_from_slice(&data.damage.to_le_bytes());
    }

    if data.repair_cost != 0 {
        out.push(INT);
        name(&mut out, "RepairCost");
        out.extend_from_slice(&data.repair_cost.to_le_bytes());
    }

    if let Some(text) = &data.name {
        out.push(COMPOUND);
        name(&mut out, "display");
        out.push(STRING);
        name(&mut out, "Name");
        name(&mut out, text);
        out.push(0);
    }

    out.push(0);
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::item_named;

    #[test]
    fn crafting_data_lists_the_recipes() {
        let packet = crafting_data_packet();
        let mut reader = super::super::codec::In::new(packet);

        assert_eq!(reader.varint(), Some(session::CRAFTING_DATA));
        assert!(reader.varint().unwrap_or(0) > 900);
        // Начало списка: рецепт с формой или без, имя.
        assert!(matches!(reader.zigzag32(), Some(0 | 1)));
        assert!(reader.string().is_some_and(|name| name.starts_with("mcsa:recipe_")));
        // Конец: четыре пустых списка/флага.
        assert_eq!(packet[packet.len() - 4..], [0, 0, 0, 1]);
    }

    #[test]
    fn recipes_are_found_by_network_id() {
        assert!(recipe_by_network_id(0).is_none());
        assert!(recipe_by_network_id(1).is_some());

        let table = item_named("crafting_table").expect("верстак");
        let found = (1..2000).filter_map(recipe_by_network_id).any(|recipe| recipe.result == table);

        assert!(found);
    }

    #[test]
    fn many_item_cells_become_tags_or_variants() {
        let planks: Vec<i32> = ["oak_planks", "birch_planks", "spruce_planks", "acacia_planks"].iter().filter_map(|n| item_named(n)).collect();

        assert_eq!(tag_of(&planks), Some("minecraft:planks"));
        assert_eq!(tag_of(&planks[..2]), None);
    }

    #[test]
    fn offers_round_trip() {
        for slot in 0..3 {
            assert_eq!(offer_slot(-1_346_738_504, offer_id(-1_346_738_504, slot)), Some(slot));
        }

        assert_eq!(offer_slot(5, offer_id(6, 1)), None);
        assert_eq!(offer_slot(5, 3), None);
    }

    #[test]
    fn the_table_offers_enchantments() {
        let sword = Stack::new(item_named("diamond_sword").expect("меч"), 1);
        let packet = enchant_options_packet(-1_346_738_504, 15, Some(sword));
        let mut reader = super::super::codec::In::new(&packet);

        assert_eq!(reader.varint(), Some(session::PLAYER_ENCHANT_OPTIONS));
        assert_eq!(reader.varint(), Some(3));
        assert_eq!(reader.varint(), Some(6));

        let none = enchant_options_packet(5, 0, None);
        assert_eq!(none[2..], [0]);
    }

    #[test]
    fn enchanted_items_carry_nbt() {
        let sword = Stack::new(item_named("diamond_sword").expect("меч"), 1);

        assert!(item_nbt(&sword).is_none());

        let mut data = sword.extras();
        data.set_enchant(crate::stackdata::enchantment_number("sharpness").expect("острота"), 3);

        let nbt = item_nbt(&sword.with_extras(data)).expect("данные");

        assert!(nbt.windows(4).any(|w| w == b"ench"));
        assert!(nbt.windows(3).any(|w| w == b"lvl"));
    }

    #[test]
    fn stations_know_their_cells() {
        assert!(Kind::Workbench.has_slot(32) && Kind::Workbench.has_slot(40) && !Kind::Workbench.has_slot(41));
        assert!(Kind::Anvil.has_slot(UI_ANVIL_MATERIAL) && !Kind::Anvil.has_slot(UI_ENCHANTING_LAPIS));
        assert_eq!(ui_slot(CRAFTING_INPUT, 30), Some(30));
        assert_eq!(ui_slot(CRAFTING_INPUT, 41), None);
        assert_eq!(Kind::of("crafting_table"), Some(Kind::Workbench));
        assert_eq!(Kind::of("chipped_anvil"), Some(Kind::Anvil));
        assert_eq!(Kind::of("stone"), None);
    }
}
