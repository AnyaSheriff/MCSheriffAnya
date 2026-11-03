// Стол зачарования, наковальня и точило: расчёты без сети.
//
// Стол. Предложения считаются от «семени зачарования» игрока (`XpSeed`):
// генератор — линейный конгруэнтный, как java.util.Random (его устройство
// описано в документации Java), засеянный семенем. Три стоимости берутся
// подряд из одного генератора; список зачарований для ячейки j — из
// генератора, засеянного семенем + j. Формулы — вики, Enchanting mechanics;
// порядок кандидатов и «основные предметы» сверены чёрным ящиком: 46
// предложений оригинала (стол с 15 полками, 4 семени, 17 предметов —
// enchant-offers, enchant-probe) совпали с расчётом все до одного.
//
// Наковальня — вики, Anvil mechanics; точило — вики, Grindstone.

use crate::blocks;
use crate::inventory::Stack;
use crate::stackdata::{self, Data};

// ---------------------------------------------------------------------------
// Генератор
// ---------------------------------------------------------------------------

/// Генератор как java.util.Random: 48 бит состояния, множитель 0x5DEECE66D.
pub struct JavaRandom(u64);

const MULTIPLIER: u64 = 0x5_DEEC_E66D;
const MASK: u64 = (1 << 48) - 1;

impl JavaRandom {
    pub fn new(seed: i64) -> Self {
        Self((seed as u64 ^ MULTIPLIER) & MASK)
    }

    fn next(&mut self, bits: u32) -> i32 {
        self.0 = (self.0.wrapping_mul(MULTIPLIER).wrapping_add(0xB)) & MASK;

        (self.0 >> (48 - bits)) as i64 as i32
    }

    /// Число от 0 (включительно) до `bound` (нет).
    pub fn next_int(&mut self, bound: i32) -> i32 {
        if bound <= 0 {
            return 0;
        }

        if bound.isolate_lowest_one() == bound {
            return ((i64::from(bound) * i64::from(self.next(31))) >> 31) as i32;
        }

        loop {
            let bits = self.next(31);
            let value = bits % bound;

            if bits.wrapping_sub(value).wrapping_add(bound - 1) >= 0 {
                return value;
            }
        }
    }

    pub fn next_float(&mut self) -> f32 {
        self.next(24) as f32 / (1 << 24) as f32
    }
}

/// Новое семя зачарования: случайное, не ноль. `salt` — чтобы двое,
/// зашедшие в одно мгновение, не получили одно и то же.
pub fn new_seed(salt: u64) -> i32 {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |time| time.as_nanos() as u64);
    let mut random = crate::mob::Rng::new(nanos ^ salt);

    loop {
        let seed = random.next_u64() as i32;

        if seed != 0 {
            return seed;
        }
    }
}

// ---------------------------------------------------------------------------
// Зачарования
// ---------------------------------------------------------------------------

/// Какие предметы — «основные» для зачарования на столе (вики, Enchanting
/// mechanics: «all enchantments to which the item is not a primary item are
/// removed»). Сверено чёрным ящиком: шипы — только нагрудник, небесная кара
/// и бич членистоногих — не булава.
const SWORD: u32 = 1;
const SPEAR: u32 = 1 << 1;
const MINING: u32 = 1 << 2;
const HEAD: u32 = 1 << 3;
const CHEST: u32 = 1 << 4;
const LEGS: u32 = 1 << 5;
const FEET: u32 = 1 << 6;
const ARMOR: u32 = HEAD | CHEST | LEGS | FEET;
const BOW: u32 = 1 << 7;
const CROSSBOW: u32 = 1 << 8;
const TRIDENT: u32 = 1 << 9;
const ROD: u32 = 1 << 10;
const MACE: u32 = 1 << 11;
const DURABLE: u32 = 1 << 12;

/// Зачарование: вес, стоимости уровня (наименьшая и наибольшая: основа и
/// прибавка за уровень), множитель наковальни для предмета, основные
/// предметы стола (0 — на столе не выпадает: сокровище или проклятие).
struct Def {
    name: &'static str,
    weight: i32,
    min: (i32, i32),
    max: (i32, i32),
    anvil: i32,
    primary: u32,
}

impl Def {
    fn min_cost(&self, level: i32) -> i32 {
        self.min.0 + self.min.1 * (level - 1)
    }

    fn max_cost(&self, level: i32) -> i32 {
        self.max.0 + self.max.1 * (level - 1)
    }
}

/// Таблица стоимостей — вики, Enchanting mechanics («Enchantment cost
/// ranges»); множители — вики, Anvil mechanics. Порядок — тот, в каком
/// кандидаты идут на столе (сверено чёрным ящиком, enchant-probe): старые
/// номера зачарований; «выпад» (новое, копья) — последним [проверить место].
const DEFS: [Def; 43] = [
    Def { name: "protection", weight: 10, min: (1, 11), max: (12, 11), anvil: 1, primary: ARMOR },
    Def { name: "fire_protection", weight: 5, min: (10, 8), max: (18, 8), anvil: 2, primary: ARMOR },
    Def { name: "feather_falling", weight: 5, min: (5, 6), max: (11, 6), anvil: 2, primary: FEET },
    Def { name: "blast_protection", weight: 2, min: (5, 8), max: (13, 8), anvil: 4, primary: ARMOR },
    Def { name: "projectile_protection", weight: 5, min: (3, 6), max: (9, 6), anvil: 2, primary: ARMOR },
    Def { name: "respiration", weight: 2, min: (10, 10), max: (40, 10), anvil: 4, primary: HEAD },
    Def { name: "aqua_affinity", weight: 2, min: (1, 0), max: (41, 0), anvil: 4, primary: HEAD },
    Def { name: "thorns", weight: 1, min: (10, 20), max: (60, 20), anvil: 8, primary: CHEST },
    Def { name: "depth_strider", weight: 2, min: (10, 10), max: (25, 10), anvil: 4, primary: FEET },
    Def { name: "frost_walker", weight: 2, min: (10, 10), max: (25, 10), anvil: 4, primary: 0 },
    Def { name: "binding_curse", weight: 1, min: (25, 0), max: (50, 0), anvil: 8, primary: 0 },
    Def { name: "soul_speed", weight: 1, min: (10, 10), max: (25, 10), anvil: 8, primary: 0 },
    Def { name: "swift_sneak", weight: 1, min: (25, 25), max: (75, 25), anvil: 8, primary: 0 },
    Def { name: "sharpness", weight: 10, min: (1, 11), max: (21, 11), anvil: 1, primary: SWORD | SPEAR },
    Def { name: "smite", weight: 5, min: (5, 8), max: (25, 8), anvil: 2, primary: SWORD | SPEAR },
    Def { name: "bane_of_arthropods", weight: 5, min: (5, 8), max: (25, 8), anvil: 2, primary: SWORD | SPEAR },
    Def { name: "knockback", weight: 5, min: (5, 20), max: (55, 20), anvil: 2, primary: SWORD | SPEAR },
    Def { name: "fire_aspect", weight: 2, min: (10, 20), max: (60, 20), anvil: 4, primary: SWORD | SPEAR },
    Def { name: "looting", weight: 2, min: (15, 9), max: (65, 9), anvil: 4, primary: SWORD | SPEAR },
    Def { name: "sweeping_edge", weight: 2, min: (5, 9), max: (20, 9), anvil: 4, primary: SWORD },
    Def { name: "efficiency", weight: 10, min: (1, 10), max: (51, 10), anvil: 1, primary: MINING },
    Def { name: "silk_touch", weight: 1, min: (15, 0), max: (65, 0), anvil: 8, primary: MINING },
    Def { name: "unbreaking", weight: 5, min: (5, 8), max: (55, 8), anvil: 2, primary: DURABLE },
    Def { name: "fortune", weight: 2, min: (15, 9), max: (65, 9), anvil: 4, primary: MINING },
    Def { name: "power", weight: 10, min: (1, 10), max: (16, 10), anvil: 1, primary: BOW },
    Def { name: "punch", weight: 2, min: (12, 20), max: (37, 20), anvil: 4, primary: BOW },
    Def { name: "flame", weight: 2, min: (20, 0), max: (50, 0), anvil: 4, primary: BOW },
    Def { name: "infinity", weight: 1, min: (20, 0), max: (50, 0), anvil: 8, primary: BOW },
    Def { name: "luck_of_the_sea", weight: 2, min: (15, 9), max: (65, 9), anvil: 4, primary: ROD },
    Def { name: "lure", weight: 2, min: (15, 9), max: (65, 9), anvil: 4, primary: ROD },
    Def { name: "loyalty", weight: 5, min: (12, 7), max: (50, 0), anvil: 1, primary: TRIDENT },
    Def { name: "impaling", weight: 2, min: (1, 8), max: (21, 8), anvil: 4, primary: TRIDENT },
    Def { name: "riptide", weight: 2, min: (17, 7), max: (50, 0), anvil: 4, primary: TRIDENT },
    Def { name: "channeling", weight: 1, min: (25, 0), max: (50, 0), anvil: 8, primary: TRIDENT },
    Def { name: "multishot", weight: 2, min: (20, 0), max: (50, 0), anvil: 4, primary: CROSSBOW },
    Def { name: "quick_charge", weight: 5, min: (12, 20), max: (50, 0), anvil: 2, primary: CROSSBOW },
    Def { name: "piercing", weight: 10, min: (1, 10), max: (50, 0), anvil: 1, primary: CROSSBOW },
    Def { name: "density", weight: 5, min: (5, 8), max: (25, 8), anvil: 2, primary: MACE },
    Def { name: "breach", weight: 2, min: (15, 9), max: (65, 9), anvil: 4, primary: MACE },
    Def { name: "wind_burst", weight: 2, min: (15, 9), max: (65, 9), anvil: 4, primary: 0 },
    Def { name: "mending", weight: 2, min: (25, 25), max: (75, 25), anvil: 4, primary: 0 },
    Def { name: "vanishing_curse", weight: 1, min: (25, 0), max: (50, 0), anvil: 8, primary: 0 },
    Def { name: "lunge", weight: 5, min: (5, 8), max: (25, 8), anvil: 2, primary: SPEAR },
];

fn def(name: &str) -> Option<&'static Def> {
    DEFS.iter().find(|def| def.name == name)
}

/// Имя зачарования по номеру реестра (`stackdata::ENCHANTMENTS`).
fn name_of(number: u16) -> &'static str {
    stackdata::ENCHANTMENTS.get(usize::from(number)).map_or("", |(name, _)| *name)
}

fn max_level(number: u16) -> i32 {
    stackdata::ENCHANTMENTS.get(usize::from(number)).map_or(1, |(_, max)| *max)
}

fn is_curse(number: u16) -> bool {
    name_of(number).ends_with("_curse")
}

/// Несовместимые зачарования 26.1.2 (minecraft-data 26.1, enchantments.json,
/// `exclude`): в наборе оружия теперь и «пронзатель».
fn exclusive(a: &str, b: &str) -> bool {
    crate::commands_world::exclusive(a, b)
}

#[cfg(test)]
mod itemaudit_tests {
    use super::*;

    #[test]
    fn combat_enchantment_conflicts_include_impaling_but_not_lunge_mending() {
        for name in ["sharpness", "smite", "bane_of_arthropods", "density", "breach"] {
            assert!(exclusive(name, "impaling"));
            assert!(exclusive("impaling", name));
        }
        assert!(!exclusive("lunge", "mending"));
        assert!(!exclusive("loyalty", "channeling"));
        for (name, _) in stackdata::ENCHANTMENTS { assert!(!exclusive(name, name)); }
    }
}

/// К какой группе основных предметов относится предмет.
fn kinds(item: &str) -> u32 {
    let mut kinds = 0;

    for (suffix, kind) in [
        ("_sword", SWORD),
        ("_spear", SPEAR),
        ("_pickaxe", MINING),
        ("_axe", MINING),
        ("_shovel", MINING),
        ("_hoe", MINING),
        ("_helmet", HEAD),
        ("_chestplate", CHEST),
        ("_leggings", LEGS),
        ("_boots", FEET),
    ] {
        if item.ends_with(suffix) {
            kinds |= kind;
        }
    }

    kinds |= match item {
        "bow" => BOW,
        "crossbow" => CROSSBOW,
        "trident" => TRIDENT,
        "fishing_rod" => ROD,
        "mace" => MACE,
        _ => 0,
    };

    kinds | DURABLE
}

/// Зачаровываемость предмета (вики, Enchanting mechanics, таблица
/// «Enchantability»; значения — компонент `enchantable`, combat_table.rs).
pub fn enchantability(item: i32) -> i32 {
    crate::combat::info(item).map_or(0, |info| info.enchantable)
}

/// Книжные полки вокруг стола: 32 места через блок от стола на его высоте
/// и выше; блок между столом и полкой должен быть «проводящим»
/// (`enchantment_power_transmitter`: воздух и прочие заменяемые) — вики,
/// Enchanting Table, «Bookshelves».
pub fn bookshelves(state_at: impl Fn((i32, i32, i32)) -> i32, (x, y, z): (i32, i32, i32)) -> i32 {
    let shelf = blocks::state_by_name("bookshelf");
    let mut count = 0;

    for dx in -2..=2 {
        for dz in -2..=2 {
            if dx * dx != 4 && dz * dz != 4 {
                continue;
            }

            for dy in 0..=1 {
                let between = (x + dx / 2, y + dy, z + dz / 2);

                if Some(state_at((x + dx, y + dy, z + dz))) == shelf && blocks::replaceable(state_at(between)) {
                    count += 1;
                }
            }
        }
    }

    count
}

/// Стоимости трёх предложений в уровнях (0 — предложения нет).
pub fn costs(seed: i32, shelves: i32, enchantability: i32) -> [i32; 3] {
    if enchantability <= 0 {
        return [0; 3];
    }

    let shelves = shelves.min(15);
    let mut random = JavaRandom::new(i64::from(seed));
    let mut out = [0; 3];

    for (slot, cost) in out.iter_mut().enumerate() {
        let base = random.next_int(8) + 1 + (shelves >> 1) + random.next_int(shelves + 1);
        let value = match slot {
            0 => (base / 3).max(1),
            1 => base * 2 / 3 + 1,
            _ => base.max(shelves * 2),
        };

        *cost = if value < slot as i32 + 1 { 0 } else { value };
    }

    out
}

/// Подбор зачарований на уровень `cost` (вики, Enchanting mechanics:
/// «Step two – Find possible enchantments», «Step three – Select a set»).
fn select(random: &mut JavaRandom, item: &str, enchantability: i32, cost: i32) -> Vec<(u16, i32)> {
    select_with_treasure(random, item, enchantability, cost, false)
}

fn select_with_treasure(random: &mut JavaRandom, item: &str, enchantability: i32, cost: i32, treasure: bool) -> Vec<(u16, i32)> {
    if enchantability <= 0 {
        return Vec::new();
    }

    let quarter = enchantability / 4 + 1;
    let mut level = cost + 1 + random.next_int(quarter) + random.next_int(quarter);
    let spread = (random.next_float() + random.next_float() - 1.0) * 0.15;

    level = ((level as f32 + level as f32 * spread).round() as i32).max(1);

    let book = item == "book";
    let kinds = kinds(item);
    let mut candidates: Vec<(&'static Def, i32)> = Vec::new();

    for def in DEFS.iter().filter(|def| {
        let treasure_kind = match def.name {
            "mending" | "vanishing_curse" => DURABLE,
            "binding_curse" => ARMOR,
            "frost_walker" => FEET,
            _ => 0,
        };
        let allowed = def.primary | if treasure { treasure_kind } else { 0 };
        allowed != 0 && (book || allowed & kinds != 0)
    }) {
        let Some(number) = stackdata::enchantment_number(def.name) else { continue };

        if let Some(found) = (1..=max_level(number)).rev().find(|&l| def.min_cost(l) <= level && level <= def.max_cost(l)) {
            candidates.push((def, found));
        }
    }

    let mut chosen: Vec<(&'static Def, i32)> = Vec::new();

    if candidates.is_empty() {
        return Vec::new();
    }

    let pick = |random: &mut JavaRandom, candidates: &[(&'static Def, i32)]| -> (&'static Def, i32) {
        let total: i32 = candidates.iter().map(|(def, _)| def.weight).sum();
        let mut roll = random.next_int(total);

        for &(def, found) in candidates {
            roll -= def.weight;

            if roll < 0 {
                return (def, found);
            }
        }

        candidates[candidates.len() - 1]
    };

    chosen.push(pick(random, &candidates));

    while random.next_int(50) <= level {
        let (last, _) = chosen[chosen.len() - 1];

        candidates.retain(|(def, _)| def.name != last.name && !exclusive(def.name, last.name));

        if candidates.is_empty() {
            break;
        }

        chosen.push(pick(random, &candidates));
        level /= 2;
    }

    chosen.into_iter().filter_map(|(def, found)| Some((stackdata::enchantment_number(def.name)?, found))).collect()
}

/// Список зачарований ячейки `slot`: генератор засеян семенем + номер
/// ячейки; у книги, если выпало несколько, одно убирается наугад.
fn slot_list(seed: i32, slot: usize, item: &str, enchantability: i32, cost: i32) -> (Vec<(u16, i32)>, JavaRandom) {
    let mut random = JavaRandom::new(i64::from(seed) + slot as i64);
    let mut list = select(&mut random, item, enchantability, cost);

    if item == "book" && list.len() > 1 {
        let index = random.next_int(list.len() as i32) as usize;

        list.remove(index);
    }

    (list, random)
}

/// Зачарованный товар: тот же подбор на заданный уровень, без меню стола.
pub fn merchant_enchant(stack: Stack, level: i32, seed: i64) -> Stack {
    let Some(name)=blocks::item_name(stack.item) else { return stack; };
    let mut random=JavaRandom::new(seed);
    let mut data=stack.extras();
    for (id,level) in select(&mut random,name,enchantability(stack.item),level) { data.set_enchant(id,level); }
    stack.with_extras(data)
}

/// Bukkit ItemFactory: копия предмета, подбор ядра и отдельные сохранённые чары книги.
pub fn enchant_with_levels(stack: Stack, level: i32, treasure: bool, seed: i64) -> Stack {
    let Some(name)=blocks::item_name(stack.item) else {return stack;};
    let mut random=JavaRandom::new(seed);let mut data=stack.extras();
    let list=select_with_treasure(&mut random,name,enchantability(stack.item),level,treasure);let mut result=stack;
    if name=="book" && !list.is_empty() {result.item=blocks::item_named("enchanted_book").unwrap_or(stack.item);for (id,level) in list {data.set_stored(id,level);}}
    else {for (id,level) in list {data.set_enchant(id,level);}}
    result.with_extras(data)
}

/// Можно ли зачаровать на столе: зачаровываемый и ещё без зачарований.
fn enchantable_here(stack: &Stack) -> Option<(&'static str, i32)> {
    let name = blocks::item_name(stack.item)?;
    let enchantability = enchantability(stack.item);
    let extras = stack.extras();

    (enchantability > 0 && extras.enchants.is_empty() && extras.stored.is_empty()).then_some((name, enchantability))
}

/// Свойства окна стола (Set Container Property 0–9): три стоимости, семя
/// (младшие 16 бит — чёрный ящик, enchant-offers), три подсказки
/// (номер зачарования, −1 — нет) и их уровни.
pub fn properties(seed: i32, shelves: i32, item: Option<Stack>) -> [i16; 10] {
    let mut out = [0, 0, 0, seed as i16, -1, -1, -1, -1, -1, -1];
    let Some((name, enchantability)) = item.as_ref().and_then(enchantable_here) else {
        return out;
    };
    let costs = costs(seed, shelves, enchantability);

    for (slot, &cost) in costs.iter().enumerate() {
        out[slot] = cost as i16;

        if cost <= 0 {
            continue;
        }

        let (list, mut random) = slot_list(seed, slot, name, enchantability, cost);

        if !list.is_empty() {
            let (number, level) = list[random.next_int(list.len() as i32) as usize];

            out[4 + slot] = number as i16;
            out[7 + slot] = level as i16;
        }
    }

    out
}

/// Зачаровать предмет предложением `slot`: новая стопка или None (нечего
/// зачаровывать, ничего не выпало). Книга становится зачарованной книгой.
pub fn enchant(seed: i32, shelves: i32, stack: Stack, slot: usize) -> Option<Stack> {
    let (name, enchantability) = enchantable_here(&stack)?;
    let cost = *costs(seed, shelves, enchantability).get(slot)?;

    if cost <= 0 {
        return None;
    }

    let (list, _) = slot_list(seed, slot, name, enchantability, cost);

    if list.is_empty() {
        return None;
    }

    let mut data = stack.extras();
    let mut stack = stack;

    if name == "book" {
        stack.item = blocks::item_named("enchanted_book")?;

        for (number, level) in list {
            data.set_stored(number, level);
        }
    } else {
        for (number, level) in list {
            data.set_enchant(number, level);
        }
    }

    Some(stack.with_extras(data))
}

// ---------------------------------------------------------------------------
// Наковальня
// ---------------------------------------------------------------------------

/// Длиннее имя не принимается (вики, Anvil: «up to 50 characters»).
pub const MAX_NAME: usize = 50;

/// Дороже — «Слишком дорого!» (вики, Anvil mechanics).
pub const TOO_EXPENSIVE: i32 = 40;

/// Чем чинится предмет (minecraft-data 26.1, items.json: `repairWith`).
fn repair_materials(item: &str) -> &'static [&'static str] {
    const PLANKS: &[&str] = &[
        "oak_planks", "spruce_planks", "birch_planks", "jungle_planks", "acacia_planks", "cherry_planks", "dark_oak_planks",
        "pale_oak_planks", "mangrove_planks", "bamboo_planks", "crimson_planks", "warped_planks",
    ];

    let material = item.split('_').next().unwrap_or("");
    let tool = ["_sword", "_spear", "_pickaxe", "_axe", "_shovel", "_hoe"].iter().any(|suffix| item.ends_with(suffix));
    let armor = ["_helmet", "_chestplate", "_leggings", "_boots"].iter().any(|suffix| item.ends_with(suffix));

    match (material, tool, armor) {
        _ if item == "turtle_helmet" => &["turtle_scute"],
        _ if item == "elytra" => &["phantom_membrane"],
        _ if item == "mace" => &["breeze_rod"],
        _ if item == "shield" => PLANKS,
        _ if item == "wolf_armor" => &["armadillo_scute"],
        ("wooden", true, _) => PLANKS,
        ("stone", true, _) => &["cobblestone", "cobbled_deepslate", "blackstone"],
        ("iron" | "chainmail", _, true) | ("iron", true, _) => &["iron_ingot"],
        ("copper", _, _) if tool || armor => &["copper_ingot"],
        ("golden", _, _) if tool || armor => &["gold_ingot"],
        ("diamond", _, _) if tool || armor => &["diamond"],
        ("netherite", _, _) if tool || armor => &["netherite_ingot"],
        ("leather", _, true) => &["leather"],
        _ => &[],
    }
}

/// Итог наковальни: что выйдет, цена в уровнях, сколько единиц материала
/// уходит на починку (0 — правая стопка уходит целиком).
#[derive(Clone, Debug, PartialEq)]
pub struct AnvilOutcome {
    pub result: Option<Stack>,
    pub cost: i32,
    pub repair_items: i32,
}

/// Зачарования стопки для наковальни: у книги — её `stored_enchantments`.
fn enchants_of(data: &Data, book: bool) -> &Vec<(u16, i32)> {
    if book { &data.stored } else { &data.enchants }
}

/// Расчёт наковальни (вики, Anvil mechanics). `name` — присланное клиентом
/// имя (пусто — снять своё имя), `creative` — творческий режим.
pub fn anvil(left: Option<Stack>, right: Option<Stack>, name: Option<&str>, creative: bool) -> AnvilOutcome {
    let empty = |cost| AnvilOutcome { result: None, cost, repair_items: 0 };
    let Some(left) = left else {
        return empty(0);
    };
    let Some(left_name) = blocks::item_name(left.item) else {
        return empty(0);
    };

    let left_data = left.extras();
    let left_book = left_name == "enchanted_book";
    let mut data = left_data.clone();
    let mut cost = 0;
    let mut repair_items = 0;
    let max = crate::combat::max_durability(&left);
    let damageable = max > 0 && !left_data.unbreakable;
    let mut base = left_data.repair_cost;
    let right_data = right.map(|right| right.extras()).unwrap_or_default();

    if let Some(right) = right {
        base += right_data.repair_cost;

        let right_name = blocks::item_name(right.item).unwrap_or("");
        let right_book = right_name == "enchanted_book" && !right_data.stored.is_empty();

        if damageable && repair_materials(left_name).contains(&right_name) {
            // Починка материалом: каждая единица — четверть прочности.
            let mut repair = data.damage.min(max / 4);

            if repair <= 0 {
                return empty(0);
            }

            while repair > 0 && repair_items < right.count {
                data.damage -= repair;
                cost += 1;
                repair = data.damage.min(max / 4);
                repair_items += 1;
            }
        } else {
            if !right_book && (right.item != left.item || !damageable) {
                return empty(0);
            }

            // Два одинаковых предмета: прочность обоих и 12 % сверху.
            if damageable && !right_book {
                let total = (max - left_data.damage) + (max - right_data.damage) + max * 12 / 100;
                let damage = (max - total).max(0);

                if damage < data.damage {
                    data.damage = damage;
                    cost += 2;
                }
            }

            let mut any_applied = false;
            let mut any_refused = false;

            for &(number, level) in enchants_of(&right_data, right_book) {
                let enchantment = name_of(number);
                let current = enchants_of(&data, left_book).iter().find(|(known, _)| *known == number).map_or(0, |(_, l)| *l);
                let mut new_level = if current == level { level + 1 } else { level.max(current) };
                let mut fits = creative || left_book || crate::commands_world::enchantable(enchantment, left_name);

                for &(other, _) in enchants_of(&data, left_book) {
                    if other != number && exclusive(enchantment, name_of(other)) {
                        fits = false;
                        cost += 1;
                    }
                }

                if !fits {
                    any_refused = true;
                    continue;
                }

                any_applied = true;
                new_level = new_level.min(max_level(number));

                if left_book {
                    data.set_stored(number, new_level);
                } else {
                    data.set_enchant(number, new_level);
                }

                let multiplier = def(enchantment).map_or(1, |def| def.anvil);
                let multiplier = if right_book { (multiplier / 2).max(1) } else { multiplier };

                cost += multiplier * new_level;

                if left.count > 1 {
                    cost = TOO_EXPENSIVE;
                }
            }

            if any_refused && !any_applied {
                return empty(0);
            }
        }
    }

    // Имя: новое — 1 уровень; пустое при своём имени — снять, тоже 1.
    let mut rename = 0;

    match name.filter(|name| !name.trim().is_empty()) {
        Some(name) => {
            if left_data.name.as_deref() != Some(name) {
                rename = 1;
                cost += 1;
                data.name = Some(name.to_string());
            }
        }
        None => {
            if left_data.name.is_some() {
                rename = 1;
                cost += 1;
                data.name = None;
            }
        }
    }

    let mut total = base + cost;

    if cost <= 0 {
        return empty(total);
    }

    // Одно переименование дороже 39 не бывает.
    if rename == cost && rename > 0 && total >= TOO_EXPENSIVE {
        total = TOO_EXPENSIVE - 1;
    }

    if total >= TOO_EXPENSIVE && !creative {
        return AnvilOutcome { result: None, cost: total, repair_items };
    }

    // Цена прошлых работ: большая из двух, после работы — вдвое и ещё 1
    // (переименование её не растит).
    let mut penalty = data.repair_cost.max(right_data.repair_cost);

    if rename != cost || rename == 0 {
        penalty = penalty * 2 + 1;
    }

    data.repair_cost = penalty;

    AnvilOutcome { result: Some(left.with_extras(data)), cost: total, repair_items }
}

/// Наковальня на ступень хуже (вики, Anvil: «12% chance of being
/// damaged»: целая → надколотая → повреждённая → разрушена). None —
/// разрушена.
pub fn anvil_damaged(name: &str) -> Option<&'static str> {
    match name {
        "anvil" => Some("chipped_anvil"),
        "chipped_anvil" => Some("damaged_anvil"),
        _ => None,
    }
}

/// Вероятность, что наковальня после работы станет хуже.
pub const ANVIL_DAMAGE_CHANCE: f64 = 0.12;

// ---------------------------------------------------------------------------
// Точило
// ---------------------------------------------------------------------------

/// Можно ли положить на точило: изнашиваемое или зачарованное.
pub fn grindable(stack: &Stack) -> bool {
    let data = stack.extras();

    crate::combat::info(stack.item).is_some_and(|info| info.max_damage > 0) || !data.enchants.is_empty() || !data.stored.is_empty()
}

/// Что выйдет из точила (вики, Grindstone): с одного предмета снимаются
/// все зачарования, кроме проклятий; два одинаковых сливаются — прочность
/// обоих и 5 % сверху, проклятия обоих остаются.
pub fn grind(top: Option<Stack>, bottom: Option<Stack>) -> Option<Stack> {
    let (base, other) = match (top, bottom) {
        (None, None) => return None,
        (Some(one), None) | (None, Some(one)) => (one, None),
        (Some(top), Some(bottom)) => (top, Some(bottom)),
    };

    let mut data = base.extras();

    if let Some(other) = other {
        let max = crate::combat::max_durability(&base);

        if other.item != base.item || max <= 0 || base.count > 1 || other.count > 1 {
            return None;
        }

        let other_data = other.extras();
        let total = (max - data.damage) + (max - other_data.damage) + max * 5 / 100;

        data.damage = (max - total).max(0);

        for &(number, level) in &other_data.enchants {
            if is_curse(number) && data.level(name_of(number)) < level {
                data.set_enchant(number, level);
            }
        }
    } else if data.enchants.iter().chain(&data.stored).all(|&(number, _)| is_curse(number)) {
        // Нечего снимать.
        return None;
    }

    data.enchants.retain(|&(number, _)| is_curse(number));
    data.stored.retain(|&(number, _)| is_curse(number));

    // Цена прошлых работ: заново, по одной работе за каждое оставшееся
    // проклятие [проверить].
    let curses = data.enchants.len() + data.stored.len();

    data.repair_cost = (0..curses).fold(0, |penalty, _| penalty * 2 + 1);

    let mut stack = base;

    if blocks::item_name(stack.item) == Some("enchanted_book") && data.stored.is_empty() {
        stack.item = blocks::item_named("book")?;
    }

    Some(stack.with_extras(data))
}

/// Опыт за снятые зачарования: сумма наименьших стоимостей их уровней, из
/// неё половина и случайная прибавка до второй половины (вики, Grindstone:
/// «experience … between half and the full sum»).
pub fn grind_xp(inputs: &[Option<Stack>], roll: impl FnOnce(i32) -> i32) -> i32 {
    let mut sum = 0;

    for stack in inputs.iter().flatten() {
        let data = stack.extras();

        for &(number, level) in data.enchants.iter().chain(&data.stored) {
            if !is_curse(number)
                && let Some(def) = def(name_of(number))
            {
                sum += def.min_cost(level);
            }
        }
    }

    if sum <= 0 {
        return 0;
    }

    let half = (sum + 1) / 2;

    half + roll(half)
}

// ---------------------------------------------------------------------------
// Станки: какие блоки открывают окно
// ---------------------------------------------------------------------------

/// Станок с окном без своего хранилища: предметы лежат в окне, пока оно
/// открыто, и возвращаются игроку при закрытии.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Station {
    Crafting,
    Enchanting,
    Anvil,
    Grindstone,
    Smithing,
    Stonecutter,
    Loom,
    Cartography,
}

impl Station {
    pub fn of(block: &str) -> Option<Station> {
        match block {
            "crafting_table" => Some(Station::Crafting),
            "enchanting_table" => Some(Station::Enchanting),
            "anvil" | "chipped_anvil" | "damaged_anvil" => Some(Station::Anvil),
            "grindstone" => Some(Station::Grindstone),
            "smithing_table" => Some(Station::Smithing),
            "stonecutter" => Some(Station::Stonecutter),
            "loom" => Some(Station::Loom),
            "cartography_table" => Some(Station::Cartography),
            _ => None,
        }
    }

    /// Вид меню (registries, menu: enchantment 13, anvil 8, grindstone 15)
    /// и заголовок окна.
    pub fn menu(self) -> (i32, &'static str) {
        match self {
            Station::Crafting => (12, "container.crafting"),
            Station::Enchanting => (13, "container.enchant"),
            Station::Anvil => (8, "container.repair"),
            Station::Grindstone => (15, "container.grindstone_title"),
            Station::Smithing => (21, "container.upgrade"),
            Station::Stonecutter => (24, "container.stonecutter"),
            Station::Loom => (18, "container.loom"),
            Station::Cartography => (23, "container.cartography_table"),
        }
    }

    /// Ячеек станка в окне (у наковальни и точила третья — итог, у
    /// кузнечного стола четвёртая).
    pub fn slots(self) -> usize {
        match self {
            Station::Crafting => 10,
            Station::Enchanting | Station::Stonecutter => 2,
            Station::Anvil | Station::Grindstone | Station::Cartography => 3,
            Station::Smithing | Station::Loom => 4,
        }
    }

    /// Сколько у станка входных ячеек: итог стоит сразу за ними.
    pub fn inputs(self) -> usize {
        match self {
            Station::Crafting => 9,
            Station::Smithing | Station::Loom => 3,
            Station::Stonecutter => 1,
            _ => 2,
        }
    }
}

/// Рыболовные сокровища: уровень 30, включая сокровищные чары (вики,
/// Fishing: «enchanted at level 30»). [проверить] распределение на оригинале.
pub fn fishing_treasure(seed: i64, stack: Stack) -> Stack {
    let Some(name) = blocks::item_name(stack.item) else { return stack; };
    let mut random = JavaRandom::new(seed);
    let enchants = select_with_treasure(&mut random, name, enchantability(stack.item), 30, true);
    let mut data = stack.extras();
    let mut result = stack;
    if name == "book" {
        result.item = blocks::item_named("enchanted_book").unwrap_or(stack.item);
        for (number, level) in enchants { data.set_stored(number, level); }
    } else {
        for (number, level) in enchants { data.set_enchant(number, level); }
    }
    result.with_extras(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(name: &str) -> Stack {
        Stack::new(blocks::item_named(name).expect(name), 1)
    }

    /// Генератор повторяет java.util.Random: известные первые значения
    /// `new Random(42)`: nextInt(10) = 0, 3, 8.
    #[test]
    fn the_generator_is_java_random() {
        let mut random = JavaRandom::new(42);

        assert_eq!([random.next_int(10), random.next_int(10), random.next_int(10)], [0, 3, 8]);
    }

    /// Предложения оригинала (чёрный ящик, enchant-offers и enchant-probe):
    /// семя робота, 15 полок и 0 полок.
    #[test]
    fn offers_match_the_original() {
        let seed = -1_346_738_504;

        assert_eq!(costs(seed, 15, 10), [6, 17, 30]);
        assert_eq!(costs(seed, 0, 10), [1, 6, 0]);
        assert_eq!(properties(seed, 15, Some(item("diamond_sword"))), [6, 17, 30, 26296, 33, 10, 17, 1, 1, 2]);
        assert_eq!(properties(seed, 15, Some(item("book"))), [6, 17, 30, 26296, 6, 9, 24, 1, 3, 1]);
        assert_eq!(properties(seed, 15, Some(item("iron_pickaxe"))), [6, 17, 30, 26296, 8, 34, 8, 2, 1, 3]);
        assert_eq!(properties(seed, 15, Some(item("golden_helmet"))), [6, 17, 30, 26296, 27, 31, 0, 2, 2, 1]);
        assert_eq!(properties(seed, 15, Some(item("bow"))), [6, 17, 30, 26296, 40, 40, 40, 1, 2, 3]);
        assert_eq!(properties(seed, 15, Some(item("mace"))), [6, 17, 30, 26296, 6, 4, 4, 1, 1, 2]);
        assert_eq!(properties(seed, 15, Some(item("diamond_leggings"))), [6, 17, 30, 26296, 40, 27, 40, 1, 3, 3]);
        assert_eq!(properties(seed, 0, Some(item("diamond_sword"))), [1, 6, 0, 26296, 33, 17, -1, 1, 1, -1]);
        // Второе семя: копьё и трезубец.
        assert_eq!(properties(1_407_873_423, 15, Some(item("iron_spear"))), [8, 7, 30, 29071, 21, 33, 40, 1, 2, 3]);
        assert_eq!(properties(1_407_873_423, 15, Some(item("trident"))), [8, 7, 30, 29071, 15, 40, 5, 1, 1, 1]);
    }

    /// Книга за первую ячейку становится зачарованной книгой с «плотностью»
    /// (чёрный ящик, enchant-probe: stored_enchantments {id 6, level 1}).
    #[test]
    fn a_book_becomes_an_enchanted_book() {
        let book = enchant(-1_346_738_504, 15, item("book"), 0).expect("зачаровалась");

        assert_eq!(blocks::item_name(book.item), Some("enchanted_book"));
        assert_eq!(book.extras().stored, vec![(6, 1)]);
        assert!(properties(-1_346_738_504, 15, Some(book))[..3].iter().all(|&cost| cost == 0));
    }

    /// Полки считаются через воздух; заслонённая — нет.
    #[test]
    fn bookshelves_need_air_between() {
        let shelf = blocks::state_by_name("bookshelf").unwrap();
        let stone = blocks::state_by_name("stone").unwrap();
        let at = |(x, y, z): (i32, i32, i32)| match (x, y, z) {
            (2, 0, 0) | (2, 1, 0) | (-2, 0, 1) => shelf,
            (-1, 0, 0) | (-1, 0, 1) => stone,
            _ => crate::world::AIR,
        };

        assert_eq!(bookshelves(at, (0, 0, 0)), 2);
    }

    /// Переименование стоит 1 уровень; цена работ становится 0 → 0 (только
    /// имя её не растит).
    #[test]
    fn renaming_costs_one_level() {
        let out = anvil(Some(item("diamond_sword")), None, Some("Меч"), false);
        let result = out.result.expect("есть итог");

        assert_eq!(out.cost, 1);
        assert_eq!(result.extras().name.as_deref(), Some("Меч"));
        assert_eq!(result.extras().repair_cost, 0);
        assert_eq!(anvil(Some(item("diamond_sword")), None, None, false).result, None);
    }

    /// Починка алмазами: каждая единица — четверть прочности, уровень за
    /// единицу; цена работ 0 → 1.
    #[test]
    fn repairing_with_material() {
        let sword = item("diamond_sword").with_extras(Data { damage: 1000, ..Data::default() });
        let out = anvil(Some(sword), Some(Stack::new(blocks::item_named("diamond").unwrap(), 5)), None, false);
        let result = out.result.expect("есть итог");

        assert_eq!((out.cost, out.repair_items), (3, 3));
        assert_eq!(result.extras().damage, 1000 - 390 * 2 - 220);
        assert_eq!(result.extras().repair_cost, 1);
    }

    /// Книга «острота V» на меч с «остротой V»: уровень не растёт выше
    /// наибольшего, цена — множитель книги на уровень.
    #[test]
    fn combining_books_and_items() {
        let sharpness = stackdata::enchantment_number("sharpness").unwrap();
        let mut book_data = Data::default();

        book_data.set_stored(sharpness, 4);

        let book = item("enchanted_book").with_extras(book_data.clone());
        let mut sword_data = Data::default();

        sword_data.set_enchant(sharpness, 4);

        let sword = item("diamond_sword").with_extras(sword_data);
        let out = anvil(Some(sword), Some(book), None, false);

        assert_eq!(out.cost, 5);
        assert_eq!(out.result.unwrap().extras().level("sharpness"), 5);

        // Несовместимое («небесная кара» на меч с «остротой») — отказ.
        let smite = stackdata::enchantment_number("smite").unwrap();
        let mut smite_book = Data::default();

        smite_book.set_stored(smite, 1);

        let mut sharp_sword = Data::default();

        sharp_sword.set_enchant(sharpness, 1);
        assert_eq!(anvil(Some(item("diamond_sword").with_extras(sharp_sword)), Some(item("enchanted_book").with_extras(smite_book)), None, false).result, None);
    }

    /// Чёрный ящик, anvil-book-targets: обычная книга не принимает чары
    /// книги на наковальне. Зачарованные книги при этом объединяются.
    #[test]
    fn anvil_requires_an_enchanted_book_as_book_target() {
        let sharpness = stackdata::enchantment_number("sharpness").unwrap();
        let mut data = Data::default();
        data.set_stored(sharpness, 1);
        let book = item("enchanted_book").with_extras(data);

        assert_eq!(anvil(Some(item("book")), Some(book), None, false).result, None);
        let result = anvil(Some(book), Some(book), None, false).result.expect("книги объединяются");
        assert_eq!(result.extras().stored, vec![(sharpness, 2)]);
        assert!(result.extras().enchants.is_empty());
    }

    /// Совместимые чары смешанной книги применяются, отказ от остальных
    /// добавляет один уровень к цене (anvil-book-targets).
    #[test]
    fn anvil_applies_only_compatible_enchantments_from_mixed_book() {
        let sharpness = stackdata::enchantment_number("sharpness").unwrap();
        let smite = stackdata::enchantment_number("smite").unwrap();
        let unbreaking = stackdata::enchantment_number("unbreaking").unwrap();
        let mut sword = Data::default();
        sword.set_enchant(smite, 1);
        let mut book = Data::default();
        book.set_stored(sharpness, 1);
        book.set_stored(unbreaking, 1);
        let out = anvil(Some(item("diamond_sword").with_extras(sword)), Some(item("enchanted_book").with_extras(book)), None, false);

        assert_eq!(out.cost, 2);
        let result = out.result.expect("совместимые чары перенесены").extras();
        assert_eq!(result.level("smite"), 1);
        assert_eq!(result.level("sharpness"), 0);
        assert_eq!(result.level("unbreaking"), 1);
    }

    /// Дороже 39 — «Слишком дорого»: итога нет, цена видна; в творческом
    /// режиме — можно.
    #[test]
    fn too_expensive_at_forty() {
        let sword = item("diamond_sword").with_extras(Data { damage: 100, repair_cost: 39, ..Data::default() });
        let other = item("diamond_sword");
        let out = anvil(Some(sword), Some(other), None, false);

        assert_eq!((out.result, out.cost), (None, 41));
        assert!(anvil(Some(sword), Some(other), None, true).result.is_some());

        // Одно имя — не дороже 39.
        let named = anvil(Some(sword), None, Some("x"), false);

        assert_eq!(named.cost, 39);
        assert!(named.result.is_some());
    }

    /// Точило снимает всё, кроме проклятий, и даёт опыт от половины суммы.
    #[test]
    fn the_grindstone_keeps_curses() {
        let mut data = Data::default();

        data.set_enchant(stackdata::enchantment_number("sharpness").unwrap(), 5);
        data.set_enchant(stackdata::enchantment_number("vanishing_curse").unwrap(), 1);
        data.repair_cost = 7;

        let sword = item("diamond_sword").with_extras(data);
        let out = grind(Some(sword), None).expect("есть итог");

        assert_eq!(out.extras().enchants, vec![(stackdata::enchantment_number("vanishing_curse").unwrap(), 1)]);
        assert_eq!(out.extras().repair_cost, 1);
        // Острота V: 1 + 11·4 = 45 → от 23 до 45.
        assert_eq!(grind_xp(&[Some(sword)], |_| 0), 23);
        assert_eq!(grind_xp(&[Some(sword)], |half| half - 1), 45);
        assert_eq!(grind(Some(item("diamond_sword")), None), None);
    }
}
