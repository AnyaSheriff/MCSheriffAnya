// Таблицы деревень: minecraft.wiki Module:LootChest/chests, oldid=3499911.
// Только poolsJava, ревизия 2026-03-27, до 26.1.2. Идентификаторы
// сундуков проверены по vanilla-struct; вероятности содержимого [проверить].
use crate::loot::Pool;

const VILLAGE_ARMORER: [Pool; 1] = [
    Pool { rolls: (1,5), empty: 0, entries: &[
        ("iron_ingot",1,3,2),
        ("bread",1,4,4),
        ("iron_helmet",1,1,1),
        ("emerald",1,1,1),
    ] },
];

const VILLAGE_BUTCHER: [Pool; 1] = [
    Pool { rolls: (1,5), empty: 0, entries: &[
        ("emerald",1,1,1),
        ("porkchop",1,3,6),
        ("wheat",1,3,6),
        ("beef",1,3,6),
        ("mutton",1,3,6),
        ("coal",1,3,3),
    ] },
];

const VILLAGE_CARTOGRAPHER: [Pool; 2] = [
    Pool { rolls: (1,5), empty: 0, entries: &[
        ("map",1,3,10),
        ("paper",1,5,15),
        ("compass",1,1,5),
        ("bread",1,4,15),
        ("stick",1,2,5),
    ] },
    Pool { rolls: (1,1), empty: 2, entries: &[
        ("bundle",1,1,1),
    ] },
];

const VILLAGE_FISHERMAN: [Pool; 1] = [
    Pool { rolls: (1,5), empty: 0, entries: &[
        ("emerald",1,1,1),
        ("cod",1,3,2),
        ("salmon",1,3,1),
        ("water_bucket",1,3,1),
        ("barrel",1,3,1),
        ("wheat_seeds",1,3,3),
        ("coal",1,3,2),
    ] },
];

const VILLAGE_FLETCHER: [Pool; 1] = [
    Pool { rolls: (1,5), empty: 0, entries: &[
        ("emerald",1,1,1),
        ("arrow",1,3,2),
        ("feather",1,3,6),
        ("egg",1,3,2),
        ("flint",1,3,6),
        ("stick",1,3,6),
    ] },
];

const VILLAGE_MASON: [Pool; 1] = [
    Pool { rolls: (1,5), empty: 0, entries: &[
        ("clay_ball",1,3,1),
        ("flower_pot",1,1,1),
        ("stone",1,1,2),
        ("stone_bricks",1,1,2),
        ("bread",1,4,4),
        ("yellow_dye",1,1,1),
        ("smooth_stone",1,1,1),
        ("emerald",1,1,1),
    ] },
];

const VILLAGE_SHEPHERD: [Pool; 1] = [
    Pool { rolls: (1,5), empty: 0, entries: &[
        ("white_wool",1,8,6),
        ("black_wool",1,3,3),
        ("gray_wool",1,3,2),
        ("brown_wool",1,3,2),
        ("light_gray_wool",1,3,2),
        ("emerald",1,1,1),
        ("shears",1,1,1),
        ("wheat",1,6,6),
    ] },
];

const VILLAGE_TANNERY: [Pool; 2] = [
    Pool { rolls: (1,5), empty: 0, entries: &[
        ("leather",1,3,1),
        ("leather_chestplate",1,1,2),
        ("leather_boots",1,1,2),
        ("leather_helmet",1,1,2),
        ("bread",1,4,5),
        ("leather_leggings",1,1,2),
        ("saddle",1,1,1),
        ("emerald",1,4,1),
    ] },
    Pool { rolls: (1,1), empty: 2, entries: &[
        ("bundle",1,1,1),
    ] },
];

const VILLAGE_TEMPLE: [Pool; 1] = [
    Pool { rolls: (3,8), empty: 0, entries: &[
        ("redstone",1,4,2),
        ("bread",1,4,7),
        ("rotten_flesh",1,4,7),
        ("lapis_lazuli",1,4,1),
        ("gold_ingot",1,4,1),
        ("emerald",1,4,1),
    ] },
];

const VILLAGE_TOOLSMITH: [Pool; 1] = [
    Pool { rolls: (3,8), empty: 0, entries: &[
        ("diamond",1,3,1),
        ("iron_ingot",1,5,5),
        ("gold_ingot",1,3,1),
        ("bread",1,3,15),
        ("iron_pickaxe",1,1,5),
        ("coal",1,3,1),
        ("stick",1,3,20),
        ("iron_shovel",1,1,5),
    ] },
];

const VILLAGE_WEAPONSMITH: [Pool; 2] = [
    Pool { rolls: (3,8), empty: 0, entries: &[
        ("diamond",1,3,3),
        ("iron_ingot",1,5,10),
        ("gold_ingot",1,3,5),
        ("bread",1,3,15),
        ("apple",1,3,15),
        ("iron_pickaxe",1,1,5),
        ("iron_sword",1,1,5),
        ("iron_chestplate",1,1,5),
        ("iron_helmet",1,1,5),
        ("iron_leggings",1,1,5),
        ("iron_boots",1,1,5),
        ("obsidian",3,7,5),
        ("oak_sapling",3,7,5),
        ("saddle",1,1,3),
        ("iron_horse_armor",1,1,1),
        ("copper_horse_armor",1,1,1),
        ("golden_horse_armor",1,1,1),
        ("diamond_horse_armor",1,1,1),
        ("iron_spear",1,1,5),
        ("copper_spear",1,1,7),
    ] },
    Pool { rolls: (1,1), empty: 2, entries: &[
        ("bundle",1,1,1),
    ] },
];

const VILLAGE_DESERT_HOUSE: [Pool; 2] = [
    Pool { rolls: (3,8), empty: 0, entries: &[
        ("clay_ball",1,1,1),
        ("green_dye",1,1,1),
        ("cactus",1,4,10),
        ("wheat",1,7,10),
        ("bread",1,4,10),
        ("book",1,1,1),
        ("dead_bush",1,3,2),
        ("emerald",1,3,1),
    ] },
    Pool { rolls: (1,1), empty: 2, entries: &[
        ("bundle",1,1,1),
    ] },
];

const VILLAGE_PLAINS_HOUSE: [Pool; 2] = [
    Pool { rolls: (3,8), empty: 0, entries: &[
        ("gold_nugget",1,3,1),
        ("dandelion",1,1,2),
        ("poppy",1,1,1),
        ("potato",1,7,10),
        ("bread",1,4,10),
        ("apple",1,5,10),
        ("book",1,1,1),
        ("feather",1,1,1),
        ("emerald",1,4,2),
        ("oak_sapling",1,2,5),
    ] },
    Pool { rolls: (1,1), empty: 2, entries: &[
        ("bundle",1,1,1),
    ] },
];

const VILLAGE_SAVANNA_HOUSE: [Pool; 2] = [
    Pool { rolls: (3,8), empty: 0, entries: &[
        ("gold_nugget",1,3,1),
        ("short_grass",1,1,5),
        ("tall_grass",1,1,5),
        ("bread",1,4,10),
        ("wheat_seeds",1,5,10),
        ("emerald",1,4,2),
        ("acacia_sapling",1,2,10),
        ("saddle",1,1,1),
        ("torch",1,2,1),
        ("bucket",1,1,1),
    ] },
    Pool { rolls: (1,1), empty: 2, entries: &[
        ("bundle",1,1,1),
    ] },
];

const VILLAGE_SNOWY_HOUSE: [Pool; 2] = [
    Pool { rolls: (3,8), empty: 0, entries: &[
        ("blue_ice",1,1,1),
        ("snow_block",1,1,4),
        ("potato",1,7,10),
        ("bread",1,4,10),
        ("beetroot_seeds",1,5,10),
        ("beetroot_soup",1,1,1),
        ("furnace",1,1,1),
        ("emerald",1,4,1),
        ("snowball",1,7,10),
        ("coal",1,4,5),
    ] },
    Pool { rolls: (1,1), empty: 2, entries: &[
        ("bundle",1,1,1),
    ] },
];

const VILLAGE_TAIGA_HOUSE: [Pool; 2] = [
    Pool { rolls: (3,8), empty: 0, entries: &[
        ("iron_nugget",1,5,1),
        ("fern",1,1,2),
        ("large_fern",1,1,2),
        ("potato",1,7,10),
        ("sweet_berries",1,7,5),
        ("bread",1,4,10),
        ("pumpkin_seeds",1,5,5),
        ("pumpkin_pie",1,1,1),
        ("emerald",1,4,2),
        ("spruce_sapling",1,5,5),
        ("spruce_sign",1,1,1),
        ("spruce_log",1,5,10),
    ] },
    Pool { rolls: (1,1), empty: 2, entries: &[
        ("bundle",1,1,1),
    ] },
];

pub fn pools(name: &str) -> Option<&'static [Pool]> {
    Some(match name {
        "village_armorer" => &VILLAGE_ARMORER,
        "village_butcher" => &VILLAGE_BUTCHER,
        "village_cartographer" => &VILLAGE_CARTOGRAPHER,
        "village_fisherman" => &VILLAGE_FISHERMAN,
        "village_fletcher" => &VILLAGE_FLETCHER,
        "village_mason" => &VILLAGE_MASON,
        "village_shepherd" => &VILLAGE_SHEPHERD,
        "village_tannery" => &VILLAGE_TANNERY,
        "village_temple" => &VILLAGE_TEMPLE,
        "village_toolsmith" => &VILLAGE_TOOLSMITH,
        "village_weaponsmith" => &VILLAGE_WEAPONSMITH,
        "village_desert_house" => &VILLAGE_DESERT_HOUSE,
        "village_plains_house" => &VILLAGE_PLAINS_HOUSE,
        "village_savanna_house" => &VILLAGE_SAVANNA_HOUSE,
        "village_snowy_house" => &VILLAGE_SNOWY_HOUSE,
        "village_taiga_house" => &VILLAGE_TAIGA_HOUSE,
        _ => return None,
    })
}
