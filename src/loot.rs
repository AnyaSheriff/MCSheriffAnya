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

/// Вагонетка с сундуком заброшенной шахты (abandoned_mineshaft).
pub const MINESHAFT: [Pool; 3] = [
    Pool {
        rolls: (1, 1),
        entries: &[
            ("golden_apple", 1, 1, 20),
            ("enchanted_golden_apple", 1, 1, 1),
            ("name_tag", 1, 1, 30),
            ("enchanted_book", 1, 1, 10),
            ("iron_pickaxe", 1, 1, 5),
        ],
        empty: 5,
    },
    Pool {
        rolls: (2, 4),
        entries: &[
            ("iron_ingot", 1, 5, 10),
            ("gold_ingot", 1, 3, 5),
            ("redstone", 4, 9, 5),
            ("lapis_lazuli", 4, 9, 5),
            ("diamond", 1, 2, 3),
            ("coal", 3, 8, 10),
            ("bread", 1, 3, 15),
            ("glow_berries", 3, 6, 15),
            ("melon_seeds", 2, 4, 10),
            ("pumpkin_seeds", 2, 4, 10),
            ("beetroot_seeds", 2, 4, 10),
        ],
        empty: 0,
    },
    Pool {
        rolls: (3, 3),
        entries: &[("rail", 4, 8, 20), ("powered_rail", 1, 4, 5), ("detector_rail", 1, 4, 5), ("activator_rail", 1, 4, 5), ("torch", 1, 16, 15)],
        empty: 0,
    },
];

/// Сундук пустынного храма (desert_pyramid).
pub const DESERT_PYRAMID: [Pool; 3] = [
    Pool {
        rolls: (2, 4),
        entries: &[
            ("diamond", 1, 3, 5),
            ("iron_ingot", 1, 5, 15),
            ("gold_ingot", 2, 7, 15),
            ("emerald", 1, 3, 15),
            ("bone", 4, 6, 25),
            ("spider_eye", 1, 3, 25),
            ("rotten_flesh", 3, 7, 25),
            ("leather", 1, 5, 20),
            ("iron_horse_armor", 1, 1, 15),
            ("copper_horse_armor", 1, 1, 15),
            ("golden_horse_armor", 1, 1, 10),
            ("diamond_horse_armor", 1, 1, 5),
            ("enchanted_book", 1, 1, 20),
            ("golden_apple", 1, 1, 20),
            ("enchanted_golden_apple", 1, 1, 2),
        ],
        empty: 15,
    },
    Pool {
        rolls: (4, 4),
        entries: &[("bone", 1, 8, 10), ("gunpowder", 1, 8, 10), ("rotten_flesh", 1, 8, 10), ("string", 1, 8, 10), ("sand", 1, 8, 10)],
        empty: 0,
    },
    Pool { rolls: (1, 1), entries: &[("dune_armor_trim_smithing_template", 2, 2, 1)], empty: 6 },
];

/// Сундук храма в джунглях (jungle_temple).
pub const JUNGLE_TEMPLE: [Pool; 2] = [
    Pool {
        rolls: (2, 6),
        entries: &[
            ("diamond", 1, 3, 3),
            ("iron_ingot", 1, 5, 10),
            ("gold_ingot", 2, 7, 15),
            ("emerald", 1, 3, 2),
            ("bone", 4, 6, 20),
            ("bamboo", 1, 3, 15),
            ("rotten_flesh", 3, 7, 16),
            ("leather", 1, 5, 3),
            ("iron_horse_armor", 1, 1, 1),
            ("copper_horse_armor", 1, 1, 1),
            ("golden_horse_armor", 1, 1, 1),
            ("diamond_horse_armor", 1, 1, 1),
            ("enchanted_book", 1, 1, 1),
        ],
        empty: 0,
    },
    Pool { rolls: (1, 1), entries: &[("wild_armor_trim_smithing_template", 2, 2, 1)], empty: 2 },
];

/// Раздатчик-ловушка храма в джунглях (jungle_temple_dispenser).
pub const JUNGLE_DISPENSER: [Pool; 1] = [Pool { rolls: (1, 2), entries: &[("arrow", 2, 7, 30)], empty: 0 }];

/// Сундук подвала иглу (igloo_chest).
pub const IGLOO: [Pool; 2] = [
    Pool {
        rolls: (2, 8),
        entries: &[
            ("apple", 1, 3, 15),
            ("coal", 1, 4, 15),
            ("gold_nugget", 1, 3, 10),
            ("stone_axe", 1, 1, 2),
            ("rotten_flesh", 1, 1, 10),
            ("emerald", 1, 1, 1),
            ("wheat", 2, 3, 10),
        ],
        empty: 0,
    },
    Pool { rolls: (1, 1), entries: &[("golden_apple", 1, 1, 1)], empty: 0 },
];


// Таблицы строений из одного куска — вики, Module:LootChest/chests,
// «poolsJava». Карты сокровищ («buried-treasure-map») пропущены: карт у нас
// ещё нет [проверить: когда появятся — карта к ближнему кладу].

/// Разрушенный портал (ruined_portal).
pub const RUINED_PORTAL: [Pool; 2] = [
    Pool {
        rolls: (4, 8),
        entries: &[
            ("obsidian", 1, 2, 40),
            ("flint", 1, 4, 40),
            ("iron_nugget", 9, 18, 40),
            ("flint_and_steel", 1, 1, 40),
            ("fire_charge", 1, 1, 40),
            ("golden_apple", 1, 1, 15),
            ("gold_nugget", 4, 24, 15),
            ("enchant:golden_sword", 1, 1, 15),
            ("enchant:golden_axe", 1, 1, 15),
            ("enchant:golden_hoe", 1, 1, 15),
            ("enchant:golden_pickaxe", 1, 1, 15),
            ("enchant:golden_shovel", 1, 1, 15),
            ("enchant:golden_helmet", 1, 1, 15),
            ("enchant:golden_chestplate", 1, 1, 15),
            ("enchant:golden_leggings", 1, 1, 15),
            ("enchant:golden_boots", 1, 1, 15),
            ("glistering_melon_slice", 4, 12, 5),
            ("golden_horse_armor", 1, 1, 5),
            ("light_weighted_pressure_plate", 1, 1, 5),
            ("golden_carrot", 4, 12, 5),
            ("clock", 1, 1, 5),
            ("gold_ingot", 2, 8, 5),
            ("bell", 1, 1, 1),
            ("enchanted_golden_apple", 1, 1, 1),
            ("gold_block", 1, 2, 1),
        ],
        empty: 0,
    },
    Pool { rolls: (1, 1), entries: &[("lodestone", 1, 2, 2)], empty: 1 },
];

/// Пул брони наутилуса — общий у кораблекрушений, клада и руин в океане.
const NAUTILUS_ARMOR: Pool = Pool {
    rolls: (1, 1),
    entries: &[("copper_nautilus_armor", 1, 1, 20), ("iron_nautilus_armor", 1, 1, 10), ("golden_nautilus_armor", 1, 1, 5), ("diamond_nautilus_armor", 1, 1, 2)],
    empty: 148,
};

/// Шаблон отделки «берег» — у всех сундуков кораблекрушения.
const COAST_TRIM: Pool = Pool { rolls: (1, 1), entries: &[("coast_armor_trim_smithing_template", 2, 2, 1)], empty: 5 };

/// Кораблекрушение, сундук с картой (shipwreck_map).
pub const SHIPWRECK_MAP: [Pool; 4] = [
    Pool { rolls: (1, 1), entries: &[("buried_treasure_map", 1, 1, 1)], empty: 0 },
    Pool {
        rolls: (3, 3),
        entries: &[("compass", 1, 1, 1), ("map", 1, 1, 1), ("clock", 1, 1, 1), ("paper", 1, 10, 20), ("feather", 1, 5, 10), ("book", 1, 5, 5)],
        empty: 0,
    },
    COAST_TRIM,
    NAUTILUS_ARMOR,
];

/// Кораблекрушение, сундук с припасами (shipwreck_supply).
pub const SHIPWRECK_SUPPLY: [Pool; 3] = [
    Pool {
        rolls: (3, 10),
        entries: &[
            ("paper", 1, 12, 8),
            ("potato", 2, 6, 7),
            ("moss_block", 1, 4, 7),
            ("poisonous_potato", 2, 6, 7),
            ("carrot", 4, 8, 7),
            ("wheat", 8, 21, 7),
            ("coal", 2, 8, 6),
            ("rotten_flesh", 5, 24, 5),
            ("bamboo", 1, 3, 2),
            ("suspicious_stew", 1, 1, 10),
            ("pumpkin", 1, 3, 2),
            ("gunpowder", 1, 5, 3),
            ("tnt", 1, 2, 1),
            ("enchant:leather_helmet", 1, 1, 3),
            ("enchant:leather_chestplate", 1, 1, 3),
            ("enchant:leather_leggings", 1, 1, 3),
            ("enchant:leather_boots", 1, 1, 3),
        ],
        empty: 0,
    },
    COAST_TRIM,
    NAUTILUS_ARMOR,
];

/// Кораблекрушение, сундук с сокровищем (shipwreck_treasure).
pub const SHIPWRECK_TREASURE: [Pool; 4] = [
    Pool {
        rolls: (3, 6),
        entries: &[("iron_ingot", 1, 5, 90), ("gold_ingot", 1, 5, 10), ("emerald", 1, 5, 40), ("diamond", 1, 1, 5), ("experience_bottle", 1, 1, 5)],
        empty: 0,
    },
    Pool { rolls: (2, 5), entries: &[("iron_nugget", 1, 10, 50), ("gold_nugget", 1, 10, 10), ("lapis_lazuli", 1, 10, 20)], empty: 0 },
    COAST_TRIM,
    NAUTILUS_ARMOR,
];

/// Зарытое сокровище (buried_treasure).
pub const BURIED_TREASURE: [Pool; 7] = [
    Pool { rolls: (1, 1), entries: &[("heart_of_the_sea", 1, 1, 1)], empty: 0 },
    Pool { rolls: (5, 8), entries: &[("iron_ingot", 1, 4, 20), ("gold_ingot", 1, 4, 10), ("tnt", 1, 2, 5)], empty: 0 },
    Pool { rolls: (1, 3), entries: &[("emerald", 4, 8, 5), ("diamond", 1, 2, 5), ("prismarine_crystals", 1, 5, 5)], empty: 0 },
    Pool { rolls: (0, 1), entries: &[("leather_chestplate", 1, 1, 1), ("iron_sword", 1, 1, 1), ("iron_spear", 1, 1, 1)], empty: 0 },
    Pool { rolls: (2, 2), entries: &[("cooked_cod", 2, 4, 1), ("cooked_salmon", 2, 4, 1)], empty: 0 },
    Pool { rolls: (0, 2), entries: &[("potion:water_breathing:potion", 1, 1, 1)], empty: 0 },
    NAUTILUS_ARMOR,
];

/// Большие руины в океане (underwater_ruin_big).
pub const UNDERWATER_RUIN_BIG: [Pool; 3] = [
    Pool {
        rolls: (2, 8),
        entries: &[("coal", 1, 4, 10), ("gold_nugget", 1, 3, 10), ("emerald", 1, 1, 1), ("wheat", 2, 3, 10), ("stone_spear", 1, 1, 2)],
        empty: 0,
    },
    Pool {
        rolls: (1, 1),
        entries: &[
            ("golden_apple", 1, 1, 1),
            ("enchanted_book", 1, 1, 5),
            ("leather_chestplate", 1, 1, 1),
            ("golden_helmet", 1, 1, 1),
            ("enchant:fishing_rod", 1, 1, 5),
            ("buried_treasure_map", 1, 1, 10),
        ],
        empty: 0,
    },
    NAUTILUS_ARMOR,
];

/// Малые руины в океане (underwater_ruin_small).
pub const UNDERWATER_RUIN_SMALL: [Pool; 3] = [
    Pool {
        rolls: (2, 8),
        entries: &[("coal", 1, 4, 10), ("stone_axe", 1, 1, 2), ("rotten_flesh", 1, 1, 5), ("emerald", 1, 1, 1), ("wheat", 2, 3, 10), ("stone_spear", 1, 1, 2)],
        empty: 0,
    },
    Pool {
        rolls: (1, 1),
        entries: &[("leather_chestplate", 1, 1, 1), ("golden_helmet", 1, 1, 1), ("enchant:fishing_rod", 1, 1, 5), ("buried_treasure_map", 1, 1, 5)],
        empty: 0,
    },
    NAUTILUS_ARMOR,
];

/// Подозрительный гравий холодных руин (archaeology/ocean_ruin_cold).
#[allow(dead_code)] // добыча кисти — когда появится кисть [проверить]
pub const ARCHAEOLOGY_OCEAN_RUIN_COLD: [Pool; 1] = [Pool {
    rolls: (1, 1),
    entries: &[
        ("blade_pottery_sherd", 1, 1, 1),
        ("explorer_pottery_sherd", 1, 1, 1),
        ("mourner_pottery_sherd", 1, 1, 1),
        ("plenty_pottery_sherd", 1, 1, 1),
        ("iron_axe", 1, 1, 1),
        ("emerald", 1, 1, 2),
        ("wheat", 1, 1, 2),
        ("wooden_hoe", 1, 1, 2),
        ("coal", 1, 1, 2),
        ("gold_nugget", 1, 1, 2),
    ],
    empty: 0,
}];

/// Подозрительный песок тёплых руин (archaeology/ocean_ruin_warm).
#[allow(dead_code)] // добыча кисти — когда появится кисть [проверить]
pub const ARCHAEOLOGY_OCEAN_RUIN_WARM: [Pool; 1] = [Pool {
    rolls: (1, 1),
    entries: &[
        ("angler_pottery_sherd", 1, 1, 1),
        ("shelter_pottery_sherd", 1, 1, 1),
        ("snort_pottery_sherd", 1, 1, 1),
        ("sniffer_egg", 1, 1, 1),
        ("iron_axe", 1, 1, 1),
        ("emerald", 1, 1, 2),
        ("wheat", 1, 1, 2),
        ("wooden_hoe", 1, 1, 2),
        ("coal", 1, 1, 2),
        ("gold_nugget", 1, 1, 2),
    ],
    empty: 0,
}];

/// Обычный подозрительный гравий руин тропы (archaeology/trail_ruins_common).
#[allow(dead_code)] // добыча кисти — когда появится кисть [проверить]
pub const ARCHAEOLOGY_TRAIL_RUINS_COMMON: [Pool; 1] = [Pool {
    rolls: (1, 1),
    entries: &[
        ("emerald", 1, 1, 2),
        ("wheat", 1, 1, 2),
        ("wooden_hoe", 1, 1, 2),
        ("clay", 1, 1, 2),
        ("brick", 1, 1, 2),
        ("yellow_dye", 1, 1, 2),
        ("blue_dye", 1, 1, 2),
        ("light_blue_dye", 1, 1, 2),
        ("white_dye", 1, 1, 2),
        ("orange_dye", 1, 1, 2),
        ("red_candle", 1, 1, 2),
        ("green_candle", 1, 1, 2),
        ("purple_candle", 1, 1, 2),
        ("brown_candle", 1, 1, 2),
        ("magenta_stained_glass_pane", 1, 1, 1),
        ("pink_stained_glass_pane", 1, 1, 1),
        ("blue_stained_glass_pane", 1, 1, 1),
        ("light_blue_stained_glass_pane", 1, 1, 1),
        ("red_stained_glass_pane", 1, 1, 1),
        ("yellow_stained_glass_pane", 1, 1, 1),
        ("purple_stained_glass_pane", 1, 1, 1),
        ("spruce_hanging_sign", 1, 1, 1),
        ("oak_hanging_sign", 1, 1, 1),
        ("gold_nugget", 1, 1, 1),
        ("coal", 1, 1, 1),
        ("wheat_seeds", 1, 1, 1),
        ("beetroot_seeds", 1, 1, 1),
        ("dead_bush", 1, 1, 1),
        ("flower_pot", 1, 1, 1),
        ("string", 1, 1, 1),
        ("lead", 1, 1, 1),
    ],
    empty: 0,
}];

/// Редкий подозрительный гравий руин тропы (archaeology/trail_ruins_rare).
#[allow(dead_code)] // добыча кисти — когда появится кисть [проверить]
pub const ARCHAEOLOGY_TRAIL_RUINS_RARE: [Pool; 1] = [Pool {
    rolls: (1, 1),
    entries: &[
        ("burn_pottery_sherd", 1, 1, 1),
        ("danger_pottery_sherd", 1, 1, 1),
        ("friend_pottery_sherd", 1, 1, 1),
        ("heart_pottery_sherd", 1, 1, 1),
        ("heartbreak_pottery_sherd", 1, 1, 1),
        ("howl_pottery_sherd", 1, 1, 1),
        ("sheaf_pottery_sherd", 1, 1, 1),
        ("wayfinder_armor_trim_smithing_template", 1, 1, 1),
        ("raiser_armor_trim_smithing_template", 1, 1, 1),
        ("shaper_armor_trim_smithing_template", 1, 1, 1),
        ("host_armor_trim_smithing_template", 1, 1, 1),
        ("music_disc_relic", 1, 1, 1),
    ],
    empty: 0,
}];

/// Таблица добычи сундука (дальше к ним добавятся таблицы деревень).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[allow(clippy::enum_variant_names)]
pub enum Table {
    StrongholdCorridor,
    StrongholdCrossing,
    StrongholdLibrary,
    Mineshaft,
    DesertPyramid,
    JungleTemple,
    JungleDispenser,
    Igloo,
}

impl Table {
    pub fn pools(self) -> &'static [Pool] {
        match self {
            Table::StrongholdCorridor => &STRONGHOLD_CORRIDOR,
            Table::StrongholdCrossing => &STRONGHOLD_CROSSING,
            Table::StrongholdLibrary => &STRONGHOLD_LIBRARY,
            Table::Mineshaft => &MINESHAFT,
            Table::DesertPyramid => &DESERT_PYRAMID,
            Table::JungleTemple => &JUNGLE_TEMPLE,
            Table::JungleDispenser => &JUNGLE_DISPENSER,
            Table::Igloo => &IGLOO,
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

/// Предмет с приставкой: «enchant:» — одно случайное подходящее зачарование
/// (вики: «random-enchanted», функция enchant_randomly) [проверить: у нас —
/// первое из подбора стола на случайный уровень 1–30]; «potion:<зелье>» —
/// зелье этого вида.
fn special(name: &str, item: i32, stack: Stack, dice: &mut Dice) -> Stack {
    if let Some(potion) = name.strip_prefix("potion:").and_then(|rest| rest.split(':').next()) {
        let data = crate::stackdata::Data { potion: crate::survival::POTIONS.iter().position(|known| *known == potion).map(|at| at as u16), ..Default::default() };

        return Stack::new(item, stack.count).with_extras(data);
    }

    if name.starts_with("enchant:") {
        let level = dice.between(1, 30);
        let enchanted = crate::enchanting::merchant_enchant(stack, level, dice.next() as i64);
        let mut data = enchanted.extras();

        data.enchants.truncate(1);
        return Stack::new(item, enchanted.count).with_extras(data);
    }

    stack
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
            let Some(item) = crate::blocks::item_named(name.rsplit(':').next().unwrap_or(name)) else {
                continue;
            };

            out.push(if name == "enchanted_book" { enchanted_book(item, dice) } else { special(name, item, Stack::new(item, dice.between(low, high)), dice) });
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

    /// Новые таблицы (вики, Module:LootChest/chests): все предметы есть в
    /// нашей таблице предметов, числа бросков как у вики.
    #[test]
    fn structure_tables_follow_the_wiki() {
        for table in [Table::Mineshaft, Table::DesertPyramid, Table::JungleTemple, Table::JungleDispenser, Table::Igloo] {
            for pool in table.pools() {
                for entry in pool.entries {
                    assert!(crate::blocks::item_named(entry.0).is_some(), "{table:?}: нет предмета {}", entry.0);
                }
            }
        }

        let rail = crate::blocks::item_named("rail").expect("рельсы есть");

        for seed in 0..500 {
            let mut dice = Dice::new(seed);
            let cart = roll(&MINESHAFT, &mut dice);

            // 0–1 + 2–4 + 3 предмета; рельсы — самый частый (вес 20 из 50).
            assert!((5..=8).contains(&cart.len()), "{}", cart.len());
            assert!(cart.iter().all(|stack| stack.item != rail || (4..=8).contains(&stack.count)));

            let desert = roll(&DESERT_PYRAMID, &mut dice);

            assert!((4..=9).contains(&desert.len()));
            assert!((1..=2).contains(&roll(&JUNGLE_DISPENSER, &mut dice).len()));
            assert!((3..=9).contains(&roll(&IGLOO, &mut dice).len()));
        }
    }

    #[test]
    fn a_filled_chest_keeps_every_stack() {
        let mut dice = Dice::new(7);
        let slots = fill(&STRONGHOLD_LIBRARY, 27, &mut dice);

        assert!(slots.iter().flatten().count() >= 3);
        assert_eq!(slots.len(), 27);
    }
}
