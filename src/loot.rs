// Добыча сундуков построек: таблицы и раскладка по ячейкам.
//
// Таблицы — вики (Module:LootChest/chests, «poolsJava»: число бросков, на
// каждый предмет — сколько штук и вес). Сундук у оригинала хранит ссылку на
// таблицу и бросает её при первом открытии; у нас бросок — при постройке
// чанка, по месту сундука (содержимое до открытия всё равно никто не видит).
// Предметы ложатся в случайные пустые ячейки [проверить: оригинал ещё делит
// стопки на части].

use crate::inventory::Stack;

/// Предмет таблицы: имя, сколько штук (от и до), вес.
pub type Entry = (&'static str, i32, i32, u32);

/// Пул: сколько бросков (от и до), предметы и вес пустого исхода.
pub struct Pool {
    pub rolls: (i32, i32),
    pub entries: &'static [Entry],
    pub empty: u32,
}

/// Алтарь крепости — сундук коридора (stronghold_corridor).
pub const STRONGHOLD_CORRIDOR: [Pool; 2] = [
    Pool {
        rolls: (2, 3),
        entries: &[
            ("ender_pearl", 1, 1, 10),
            ("diamond", 1, 3, 3),
            ("iron_ingot", 1, 5, 10),
            ("gold_ingot", 1, 3, 5),
            ("redstone", 4, 9, 5),
            ("bread", 1, 3, 15),
            ("apple", 1, 3, 15),
            ("iron_pickaxe", 1, 1, 5),
            ("iron_sword", 1, 1, 5),
            ("iron_chestplate", 1, 1, 5),
            ("iron_helmet", 1, 1, 5),
            ("iron_leggings", 1, 1, 5),
            ("iron_boots", 1, 1, 5),
            ("golden_apple", 1, 1, 1),
            ("leather", 1, 5, 1),
            ("iron_horse_armor", 1, 1, 1),
            ("copper_horse_armor", 1, 1, 1),
            ("golden_horse_armor", 1, 1, 1),
            ("diamond_horse_armor", 1, 1, 1),
            ("music_disc_otherside", 1, 1, 1),
            ("enchanted_book", 1, 1, 1),
        ],
        empty: 0,
    },
    Pool { rolls: (1, 1), entries: &[("eye_armor_trim_smithing_template", 1, 1, 1)], empty: 9 },
];

/// Кладовая большой комнаты крепости (stronghold_crossing).
pub const STRONGHOLD_CROSSING: [Pool; 1] = [Pool {
    rolls: (1, 4),
    entries: &[
        ("iron_ingot", 1, 5, 10),
        ("gold_ingot", 1, 3, 5),
        ("redstone", 4, 9, 5),
        ("coal", 3, 8, 10),
        ("bread", 1, 3, 15),
        ("apple", 1, 3, 15),
        ("iron_pickaxe", 1, 1, 1),
        ("enchanted_book", 1, 1, 1),
    ],
    empty: 0,
}];

/// Библиотека крепости (stronghold_library).
pub const STRONGHOLD_LIBRARY: [Pool; 2] = [
    Pool {
        rolls: (2, 10),
        entries: &[("book", 1, 3, 20), ("paper", 2, 7, 20), ("map", 1, 1, 1), ("compass", 1, 1, 1), ("enchanted_book", 1, 1, 10)],
        empty: 0,
    },
    Pool { rolls: (1, 1), entries: &[("eye_armor_trim_smithing_template", 1, 1, 1)], empty: 0 },
];

/// Таблица добычи сундука (дальше к ним добавятся таблицы деревень).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum Table {
    StrongholdCorridor,
    StrongholdCrossing,
    StrongholdLibrary,
}

impl Table {
    pub fn pools(self) -> &'static [Pool] {
        match self {
            Table::StrongholdCorridor => &STRONGHOLD_CORRIDOR,
            Table::StrongholdCrossing => &STRONGHOLD_CROSSING,
            Table::StrongholdLibrary => &STRONGHOLD_LIBRARY,
        }
    }
}

/// Простой счётчик случая: каждый вызов — новое 64-битное число.
pub struct Dice(u64);

impl Dice {
    pub fn new(seed: u64) -> Self {
        Self(seed)
    }

    pub fn next(&mut self) -> u64 {
        // splitmix64.
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;

        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Целое от `low` до `high` включительно.
    pub fn between(&mut self, low: i32, high: i32) -> i32 {
        if high <= low {
            return low;
        }

        low + (self.next() % (high - low + 1) as u64) as i32
    }
}

/// Зачарованная книга: одно зачарование случайного вида и уровня (вики:
/// «same as a level-30 enchantment on an enchanting table») [проверить:
/// закон стола уровня 30 — у помощника зачарований; книга с хранимыми
/// зачарованиями — тоже там].
fn enchanted_book(item: i32, dice: &mut Dice) -> Stack {
    let mut data = crate::stackdata::Data::default();
    let choices: Vec<(usize, i32)> = crate::stackdata::ENCHANTMENTS
        .iter()
        .enumerate()
        .filter(|(_, (name, _))| !name.ends_with("_curse") && !matches!(*name, "mending" | "frost_walker" | "soul_speed" | "swift_sneak" | "wind_burst"))
        .map(|(number, (_, top))| (number, *top))
        .collect();
    let (number, top) = choices[(dice.next() % choices.len() as u64) as usize];

    data.set_enchant(number as u16, dice.between((top + 1) / 2, top));

    Stack::new(item, 1).with_extras(data)
}

/// Бросок таблицы: предметы, как выпали.
pub fn roll(table: &[Pool], dice: &mut Dice) -> Vec<Stack> {
    let mut out = Vec::new();

    for pool in table {
        let total: u32 = pool.entries.iter().map(|entry| entry.3).sum::<u32>() + pool.empty;

        if total == 0 {
            continue;
        }

        for _ in 0..dice.between(pool.rolls.0, pool.rolls.1) {
            let mut pick = (dice.next() % u64::from(total)) as u32;
            let Some(&(name, low, high, _)) = pool.entries.iter().find(|entry| {
                let hit = pick < entry.3;
                pick = pick.saturating_sub(entry.3);
                hit
            }) else {
                continue;
            };
            let Some(item) = crate::blocks::item_named(name) else {
                continue;
            };

            out.push(if name == "enchanted_book" { enchanted_book(item, dice) } else { Stack::new(item, dice.between(low, high)) });
        }
    }

    out
}

/// Сундук на `size` ячеек с добычей таблицы: каждый предмет — в случайную
/// пустую ячейку.
pub fn fill(table: &[Pool], size: usize, dice: &mut Dice) -> Vec<Option<Stack>> {
    let mut slots = vec![None; size];

    for stack in roll(table, dice) {
        let free: Vec<usize> = (0..size).filter(|&at| slots[at].is_none()).collect();

        if free.is_empty() {
            break;
        }

        slots[free[(dice.next() % free.len() as u64) as usize]] = Some(stack);
    }

    slots
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Числа предметов и доли — как в таблицах вики: у библиотеки шаблон
    /// отделки «око» всегда, у алтаря — в 10 %; бросков 2–10 и 2–3.
    #[test]
    fn stronghold_tables_follow_the_wiki() {
        let trim = crate::blocks::item_named("eye_armor_trim_smithing_template").expect("шаблон есть");
        let pearl = crate::blocks::item_named("ender_pearl").expect("жемчуг есть");
        let (mut altar_trims, mut pearls) = (0, 0);
        let runs = 4000;

        for seed in 0..runs {
            let mut dice = Dice::new(seed);
            let library = roll(&STRONGHOLD_LIBRARY, &mut dice);

            assert!((3..=11).contains(&library.len()));
            assert_eq!(library.iter().filter(|s| s.item == trim).count(), 1);

            let altar = roll(&STRONGHOLD_CORRIDOR, &mut dice);
            let trims = altar.iter().filter(|s| s.item == trim).count();

            assert!((2..=4).contains(&altar.len()));
            altar_trims += trims;
            pearls += usize::from(altar.iter().any(|s| s.item == pearl));

            let store = roll(&STRONGHOLD_CROSSING, &mut dice);

            assert!((1..=4).contains(&store.len()));
        }

        let trim_share = altar_trims as f64 / runs as f64;
        // Вики: шаблон в алтаре — 10 %, жемчуг — 22,8 %.
        assert!((trim_share - 0.10).abs() < 0.02, "шаблон {}", trim_share);
        assert!((pearls as f64 / runs as f64 - 0.228).abs() < 0.03, "жемчуг {}", pearls as f64 / runs as f64);
    }

    #[test]
    fn a_filled_chest_keeps_every_stack() {
        let mut dice = Dice::new(7);
        let slots = fill(&STRONGHOLD_LIBRARY, 27, &mut dice);

        assert!(slots.iter().flatten().count() >= 3);
        assert_eq!(slots.len(), 27);
    }
}
