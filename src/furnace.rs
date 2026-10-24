// Печи (обычная, плавильная, коптильня) и варочная стойка.
//
// Содержимое у них — обычное хранилище (containers.rs): у печи три ячейки
// (0 — что плавится, 1 — топливо, 2 — выход), у стойки пять (0–2 —
// бутылки, 3 — ингредиент, 4 — огненный порошок). Сверх ячеек у них свой
// «жар» (`Heat`): сколько ещё горит топливо, сколько плавится предмет,
// сколько опыта накоплено; у стойки — время варки и заряды порошка.
//
// Правила — со страниц minecraft.wiki: Smelting (рецепты и опыт),
// Template:Smelting table (топливо), Furnace, Blast Furnace, Smoker,
// Brewing, Brewing Stand, Hopper. Такты, свойства окна и порядок пакетов
// сняты чёрным ящиком: сценарии furnace-smelt, furnace-blast,
// furnace-hopper, brewing-awkward, brewing-swiftness.

use crate::blocks;
use crate::containers::{Container, Kind, Pos};
use crate::inventory::Stack;
use crate::world::World;

/// Жар печи или стойки — то, что сверх ячеек.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Heat {
    /// Печь: сколько тактов ещё горит топливо (`lit_time_remaining`).
    /// Стойка: сколько тактов ещё варится (`BrewTime`).
    pub lit: i32,

    /// Печь: на сколько тактов было последнее топливо (`lit_total_time`).
    /// Стойка: заряды огненного порошка (`Fuel`, до 20).
    pub lit_total: i32,

    /// Печь: сколько тактов плавится предмет (`cooking_time_spent`).
    pub cook: i32,

    /// Печь: сколько тактов нужно на предмет (`cooking_total_time`).
    pub cook_total: i32,

    /// Печь: накопленный опыт за выплавленное (вики, Smelting: «accumulating
    /// it in a hidden counter»).
    pub xp: f32,

    /// Стойка: с каким ингредиентом началась варка (номер предмета).
    pub ingredient: i32,
}

/// Печь ли это (любая из трёх).
pub fn is_oven(kind: Kind) -> bool {
    matches!(kind, Kind::Furnace | Kind::BlastFurnace | Kind::Smoker)
}

/// Ячейки печи и стойки.
pub const INPUT: usize = 0;
pub const FUEL: usize = 1;
pub const OUTPUT: usize = 2;
pub const INGREDIENT: usize = 3;
pub const POWDER: usize = 4;

/// Время варки (вики, Brewing Stand: «20 seconds»; чёрный ящик,
/// brewing-awkward: свойство 0 — 400 и вниз по одному).
pub const BREW_TIME: i32 = 400;

/// Заряды одного огненного порошка (вики, Brewing: «Holds 20 charges»).
pub const POWDER_CHARGES: i32 = 20;

fn name_of(item: i32) -> &'static str {
    blocks::item_name(item).unwrap_or("")
}

fn item(name: &str) -> Option<i32> {
    blocks::item_named(name)
}

// ---------------------------------------------------------------------------
// Рецепты и топливо
// ---------------------------------------------------------------------------

/// Деревья Обычного мира: из них — уголь и топливо (вики, Smelting:
/// «Input must be an Overworld log or wood»; Template:Smelting table:
/// «Overworld Planks…»). Незерские стволы не горят.
const WOODS: [&str; 11] = ["oak", "spruce", "birch", "jungle", "acacia", "dark_oak", "mangrove", "cherry", "pale_oak", "bamboo", "poplar"];

/// Цвета красителей — для глазурованной терракоты, шерсти и ковров.
const COLORS: [&str; 16] = [
    "white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray", "light_gray", "cyan", "purple", "blue", "brown",
    "green", "red", "black",
];

/// Еда: печь и коптильня (вики, Smelting, «Food»).
fn food(name: &str) -> Option<(&'static str, f32)> {
    Some(match name {
        "potato" => ("baked_potato", 0.35),
        "kelp" => ("dried_kelp", 0.1),
        "beef" => ("cooked_beef", 0.35),
        "porkchop" => ("cooked_porkchop", 0.35),
        "mutton" => ("cooked_mutton", 0.35),
        "chicken" => ("cooked_chicken", 0.35),
        "rabbit" => ("cooked_rabbit", 0.35),
        "cod" => ("cooked_cod", 0.35),
        "salmon" => ("cooked_salmon", 0.35),
        _ => return None,
    })
}

/// Руды и снаряжение: печь и плавильная печь (вики, Smelting, «Ores» и
/// «Gear»).
fn ore(name: &str) -> Option<(&'static str, f32)> {
    let plain = name.strip_prefix("deepslate_").unwrap_or(name);

    Some(match plain {
        "raw_iron" | "iron_ore" => ("iron_ingot", 0.7),
        "raw_copper" | "copper_ore" => ("copper_ingot", 0.7),
        "raw_gold" | "nether_gold_ore" | "gold_ore" => ("gold_ingot", 1.0),
        "ancient_debris" => ("netherite_scrap", 2.0),
        "redstone_ore" => ("redstone", 0.7),
        "coal_ore" => ("coal", 0.1),
        "emerald_ore" => ("emerald", 1.0),
        "lapis_ore" => ("lapis_lazuli", 0.2),
        "diamond_ore" => ("diamond", 1.0),
        "nether_quartz_ore" => ("quartz", 0.2),
        _ => return gear(name),
    })
}

/// Снаряжение в самородки: инструменты, оружие, броня, конская и
/// наутилусова броня (вики, Smelting, «Gear»: опыт 0,1).
fn gear(name: &str) -> Option<(&'static str, f32)> {
    const PIECES: [&str; 12] = [
        "shovel", "pickaxe", "axe", "hoe", "sword", "spear", "helmet", "chestplate", "leggings", "boots", "horse_armor",
        "nautilus_armor",
    ];

    let (metal, piece) = name.split_once('_')?;

    if !PIECES.contains(&piece) {
        return None;
    }

    Some(match metal {
        "iron" => ("iron_nugget", 0.1),
        // Кольчуга — только броня (вики: «chainmail armor»).
        "chainmail" if matches!(piece, "helmet" | "chestplate" | "leggings" | "boots") => ("iron_nugget", 0.1),
        "golden" => ("gold_nugget", 0.1),
        "copper" => ("copper_nugget", 0.1),
        _ => return None,
    })
}

/// Только обычная печь (вики, Smelting, «Furnace-only»).
fn furnace_only(name: &str) -> Option<(String, f32)> {
    let fixed = match name {
        "cobblestone" => Some(("stone", 0.1)),
        "stone" => Some(("smooth_stone", 0.1)),
        "stone_bricks" => Some(("cracked_stone_bricks", 0.1)),
        "cobbled_deepslate" => Some(("deepslate", 0.1)),
        "deepslate_bricks" => Some(("cracked_deepslate_bricks", 0.1)),
        "deepslate_tiles" => Some(("cracked_deepslate_tiles", 0.1)),
        "sandstone" => Some(("smooth_sandstone", 0.1)),
        "red_sandstone" => Some(("smooth_red_sandstone", 0.1)),
        "nether_bricks" => Some(("cracked_nether_bricks", 0.1)),
        "basalt" => Some(("smooth_basalt", 0.1)),
        "polished_blackstone_bricks" => Some(("cracked_polished_blackstone_bricks", 0.1)),
        "quartz_block" => Some(("smooth_quartz", 0.1)),
        "clay" => Some(("terracotta", 0.35)),
        "sand" | "red_sand" => Some(("glass", 0.1)),
        "wet_sponge" => Some(("sponge", 0.15)),
        "chorus_fruit" => Some(("popped_chorus_fruit", 0.1)),
        "sea_pickle" => Some(("lime_dye", 0.1)),
        // Вики: «JE: 1» (MC-210211).
        "cactus" => Some(("green_dye", 1.0)),
        "clay_ball" => Some(("brick", 0.3)),
        "netherrack" => Some(("nether_brick", 0.1)),
        "resin_clump" => Some(("resin_brick", 0.1)),
        _ => None,
    };

    if let Some((result, xp)) = fixed {
        return Some((result.to_string(), xp));
    }

    // Крашеная терракота — в глазурованную того же цвета.
    if let Some(color) = name.strip_suffix("_terracotta").filter(|color| COLORS.contains(color)) {
        return Some((format!("{color}_glazed_terracotta"), 0.1));
    }

    // Брёвна и древесина Обычного мира, в том числе обтёсанные, — в уголь.
    let wood = name.strip_prefix("stripped_").unwrap_or(name);

    if let Some(kind) = wood.strip_suffix("_log").or_else(|| wood.strip_suffix("_wood"))
        && WOODS.contains(&kind)
        && kind != "bamboo"
    {
        return Some(("charcoal".to_string(), 0.15));
    }

    // Листва — в опавшие листья (вики: «Any leaves»). [проверить] азалия.
    if name.ends_with("_leaves") {
        return Some(("leaf_litter".to_string(), 0.1));
    }

    None
}

/// Что выйдет из предмета в печи этого вида и сколько опыта (вики,
/// Smelting: печь — все рецепты, плавильная — руды и снаряжение,
/// коптильня — еда). None — не плавится.
pub fn smelt(kind: Kind, input: i32) -> Option<(i32, f32)> {
    let name = name_of(input);
    let (result, xp) = match kind {
        Kind::Smoker => food(name).map(|(result, xp)| (result.to_string(), xp))?,
        Kind::BlastFurnace => ore(name).map(|(result, xp)| (result.to_string(), xp))?,
        Kind::Furnace => food(name)
            .or_else(|| ore(name))
            .map(|(result, xp)| (result.to_string(), xp))
            .or_else(|| furnace_only(name))?,
        _ => return None,
    };

    Some((item(&result)?, xp))
}

/// Сколько тактов плавится предмет: 200 в печи, вдвое быстрее в
/// плавильной и коптильне (вики, Blast Furnace и Smoker: «twice as fast»;
/// чёрный ящик, furnace-blast: свойство 3 — 100).
pub fn cook_time(kind: Kind) -> i32 {
    match kind {
        Kind::BlastFurnace | Kind::Smoker => 100,
        _ => 200,
    }
}

/// Сколько тактов горит топливо в обычной печи (вики, Template:Smelting
/// table, «Burning time»). None — не топливо.
pub fn burn_time(fuel: i32) -> Option<i32> {
    let name = name_of(fuel);

    let fixed = match name {
        "lava_bucket" => 20000,
        "coal_block" => 16000,
        "dried_kelp_block" => 4000,
        "blaze_rod" => 2400,
        "coal" | "charcoal" => 1600,
        "bamboo_mosaic" | "bamboo_mosaic_stairs" | "chiseled_bookshelf" | "bamboo_block" | "stripped_bamboo_block" => 300,
        "bamboo_mosaic_slab" => 150,
        "mangrove_roots" | "ladder" | "crafting_table" | "cartography_table" | "fletching_table" | "smithing_table" | "loom"
        | "bookshelf" | "lectern" | "composter" | "chest" | "trapped_chest" | "barrel" | "daylight_detector" | "jukebox"
        | "note_block" | "crossbow" | "bow" | "fishing_rod" => 300,
        "wooden_pickaxe" | "wooden_shovel" | "wooden_hoe" | "wooden_axe" | "wooden_sword" | "wooden_spear" => 200,
        "bowl" | "stick" | "dead_bush" | "azalea" | "flowering_azalea" | "leaf_litter" | "short_dry_grass"
        | "tall_dry_grass" | "mangrove_propagule" => 100,
        "bamboo" | "scaffolding" => 50,
        _ => 0,
    };

    if fixed > 0 {
        return Some(fixed);
    }

    // Лодки и плоты, с сундуком и без.
    if name.ends_with("_boat") || name.ends_with("_raft") {
        return Some(1200);
    }

    if name.ends_with("_banner") || name.ends_with("_sapling") {
        return Some(if name.ends_with("_banner") { 300 } else { 100 });
    }

    if let Some(color) = name.strip_suffix("_wool").filter(|color| COLORS.contains(color)) {
        let _ = color;
        return Some(100);
    }

    if name.strip_suffix("_carpet").is_some_and(|color| COLORS.contains(&color)) {
        return Some(67);
    }

    // Брёвна и древесина Обычного мира.
    let wood = name.strip_prefix("stripped_").unwrap_or(name);

    if let Some(kind) = wood.strip_suffix("_log").or_else(|| wood.strip_suffix("_wood"))
        && WOODS.contains(&kind)
    {
        return Some(300);
    }

    // Деревянное из досок Обычного мира.
    for wood in WOODS {
        let Some(rest) = name.strip_prefix(wood).and_then(|rest| rest.strip_prefix('_')) else {
            continue;
        };

        return match rest {
            "planks" | "stairs" | "pressure_plate" | "trapdoor" | "fence_gate" | "fence" | "shelf" => Some(300),
            "slab" => Some(150),
            "door" | "sign" | "hanging_sign" => Some(200),
            "button" => Some(100),
            _ => None,
        };
    }

    None
}

/// Сколько тактов топливо горит в печи этого вида: плавильная и коптильня
/// жгут вдвое быстрее (вики, Template:Smelting table; чёрный ящик,
/// furnace-blast: уголь — 800, палка в коптильне — 50).
pub fn burn_in(kind: Kind, fuel: i32) -> Option<i32> {
    let time = burn_time(fuel)?;

    Some(match kind {
        Kind::BlastFurnace | Kind::Smoker => time / 2,
        _ => time,
    })
}

// ---------------------------------------------------------------------------
// Варка
// ---------------------------------------------------------------------------

/// Ингредиенты стойки (вики, Brewing: «Base ingredients and modifiers» и
/// «Effect ingredients»).
pub fn is_ingredient(item: i32) -> bool {
    matches!(
        name_of(item),
        "nether_wart"
            | "redstone"
            | "glowstone_dust"
            | "fermented_spider_eye"
            | "gunpowder"
            | "dragon_breath"
            | "sugar"
            | "rabbit_foot"
            | "glistering_melon_slice"
            | "spider_eye"
            | "pufferfish"
            | "magma_cream"
            | "golden_carrot"
            | "blaze_powder"
            | "ghast_tear"
            | "turtle_helmet"
            | "phantom_membrane"
            | "breeze_rod"
            | "stone"
            | "cobweb"
            | "slime_block"
    )
}

/// Бутылка ли это — в ячейки 0–2 стойки (вики, Brewing Stand: зелья и
/// пустые бутылки).
pub fn is_bottle(item: i32) -> bool {
    matches!(name_of(item), "potion" | "splash_potion" | "lingering_potion" | "glass_bottle")
}

/// Во что превращается зелье от ингредиента (вики, Brewing, «Brewing
/// recipes»). Имена — как в `survival::POTIONS`.
pub fn mix(potion: &str, ingredient: &str) -> Option<&'static str> {
    use crate::survival::POTIONS;

    let known = |name: &str| POTIONS.iter().copied().find(|known| *known == name);

    match (potion, ingredient) {
        ("water", "nether_wart") => Some("awkward"),
        ("water", "glowstone_dust") => Some("thick"),
        ("water", "fermented_spider_eye") => Some("weakness"),
        (
            "water",
            "redstone" | "sugar" | "rabbit_foot" | "glistering_melon_slice" | "spider_eye" | "magma_cream" | "blaze_powder"
            | "ghast_tear" | "breeze_rod" | "stone" | "cobweb" | "slime_block",
        ) => Some("mundane"),
        ("awkward", ingredient) => match ingredient {
            "sugar" => Some("swiftness"),
            "rabbit_foot" => Some("leaping"),
            "glistering_melon_slice" => Some("healing"),
            "spider_eye" => Some("poison"),
            "pufferfish" => Some("water_breathing"),
            "magma_cream" => Some("fire_resistance"),
            "golden_carrot" => Some("night_vision"),
            "blaze_powder" => Some("strength"),
            "ghast_tear" => Some("regeneration"),
            "turtle_helmet" => Some("turtle_master"),
            "phantom_membrane" => Some("slow_falling"),
            "breeze_rod" => Some("wind_charged"),
            "cobweb" => Some("weaving"),
            "slime_block" => Some("oozing"),
            "stone" => Some("infested"),
            _ => None,
        },
        // Порча (вики, «Corrupting effect»; усиленные прыжок и скорость не
        // портятся).
        (potion, "fermented_spider_eye") => match potion {
            "night_vision" => Some("invisibility"),
            "long_night_vision" => Some("long_invisibility"),
            "swiftness" | "leaping" => Some("slowness"),
            "long_swiftness" | "long_leaping" => Some("long_slowness"),
            "healing" | "poison" | "long_poison" => Some("harming"),
            "strong_healing" | "strong_poison" => Some("strong_harming"),
            _ => None,
        },
        // Редстоун продлевает, светопыль усиливает — у зелий, где такое есть.
        (potion, "redstone") if !potion.starts_with("long_") && !potion.starts_with("strong_") => known(&format!("long_{potion}")),
        (potion, "glowstone_dust") if !potion.starts_with("long_") && !potion.starts_with("strong_") => {
            known(&format!("strong_{potion}"))
        }
        _ => None,
    }
}

/// Что выйдет из бутылки с этим ингредиентом. Порох делает из питьевого
/// взрывное, драконье дыхание из взрывного — оседающее (вики, Brewing,
/// «Splash and lingering potions»); прочее меняет само зелье, вид бутылки
/// остаётся.
pub fn brew_bottle(bottle: Stack, ingredient: i32) -> Option<Stack> {
    let ingredient = name_of(ingredient);
    let kind = name_of(bottle.item);

    match (kind, ingredient) {
        ("potion", "gunpowder") => return Some(Stack { item: item("splash_potion")?, ..bottle }),
        ("splash_potion", "dragon_breath") => return Some(Stack { item: item("lingering_potion")?, ..bottle }),
        (_, "gunpowder" | "dragon_breath") => return None,
        ("potion" | "splash_potion" | "lingering_potion", _) => {}
        _ => return None,
    }

    let mut extras = bottle.extras();
    let now = crate::survival::POTIONS.get(usize::from(extras.potion?))?;
    let next = mix(now, ingredient)?;

    extras.potion = crate::survival::potion_number(next);
    Some(bottle.with_extras(extras))
}

/// Можно ли варить: ингредиент годный и хоть одна бутылка от него меняется.
fn can_brew(slots: &[Option<Stack>]) -> bool {
    let Some(ingredient) = slots.get(INGREDIENT).copied().flatten() else {
        return false;
    };

    slots.iter().take(3).flatten().any(|bottle| brew_bottle(*bottle, ingredient.item).is_some())
}

// ---------------------------------------------------------------------------
// Ячейки: что куда кладётся
// ---------------------------------------------------------------------------

/// Сколько влезает в ячейку: в бутылочные ячейки стойки — по одной.
pub fn slot_limit(kind: Kind, slot: usize, item: i32) -> i32 {
    if kind == Kind::Beacon { return i32::from(slot == 0 && crate::beacon::payment(item)); }
    if kind == Kind::BrewingStand && slot < 3 { return 1; }

    blocks::stack_size(item)
}

/// Можно ли положить предмет в ячейку рукой (окно): в выход печи — ничего,
/// в топливо — только топливо и ведро; у стойки — бутылки в 0–2, ингредиент
/// в 3, огненный порошок в 4 (вики, Furnace и Brewing Stand).
pub fn may_place(kind: Kind, slot: usize, item: i32) -> bool {
    match kind {
        Kind::Beacon => slot == 0 && crate::beacon::payment(item),
        Kind::Campfire => false,
        Kind::Furnace | Kind::BlastFurnace | Kind::Smoker => match slot {
            INPUT => true,
            FUEL => burn_time(item).is_some() || name_of(item) == "bucket",
            _ => false,
        },
        Kind::BrewingStand => match slot {
            0..=2 => is_bottle(item),
            INGREDIENT => is_ingredient(item),
            POWDER => name_of(item) == "blaze_powder",
            _ => false,
        },
        _ => true,
    }
}

/// В какие ячейки предмет кладёт воронка (выбрасыватель, крафтер), если он
/// летит в направлении `travel`. None — хранилище не печь и не стойка.
/// Печь: сверху — во вход (любой предмет), сбоку и снизу — в топливо
/// (только топливо). Стойка: сверху — в ингредиент (только годный), сбоку —
/// порошок в топливо, бутылки — в пустые бутылочные ячейки (вики, Hopper).
pub fn insert_slots(kind: Kind, travel: (i32, i32, i32), item: i32) -> Option<Vec<usize>> {
    let from_top = travel.1 < 0;

    Some(match kind {
        Kind::Furnace | Kind::BlastFurnace | Kind::Smoker if from_top => vec![INPUT],
        Kind::Furnace | Kind::BlastFurnace | Kind::Smoker if burn_time(item).is_some() => vec![FUEL],
        Kind::Furnace | Kind::BlastFurnace | Kind::Smoker => Vec::new(),
        Kind::BrewingStand if from_top => {
            if is_ingredient(item) {
                vec![INGREDIENT]
            } else {
                Vec::new()
            }
        }
        Kind::BrewingStand if name_of(item) == "blaze_powder" => vec![POWDER],
        Kind::BrewingStand if is_bottle(item) => vec![0, 1, 2],
        Kind::BrewingStand => Vec::new(),
        _ => return None,
    })
}

/// Можно ли воронке снизу вытянуть предмет из этой ячейки: у печи — выход
/// и пустое ведро из топлива, у стойки — бутылки (вики, Hopper).
pub fn may_extract(kind: Kind, slot: usize, item: i32) -> bool {
    match kind {
        Kind::Furnace | Kind::BlastFurnace | Kind::Smoker => slot == OUTPUT || (slot == FUEL && name_of(item) == "bucket"),
        Kind::BrewingStand => slot < 3,
        _ => true,
    }
}

/// Порядок, в котором воронка снизу перебирает ячейки: у печи выход раньше
/// топлива. [проверить] порядок у оригинала.
pub fn extract_order(kind: Kind, size: usize) -> Vec<usize> {
    if is_oven(kind) {
        vec![OUTPUT, FUEL]
    } else {
        (0..size).collect()
    }
}

/// Вход печи сменился не тем же предметом (рукой, воронкой, командой):
/// плавка начинается заново и время на предмет считается по новому входу.
/// Так у оригинала: свойство 3 становится 200, когда руду положили
/// командой в ячейку, и остаётся 0, когда печь поставили с рудой в данных
/// (чёрный ящик, furnace-smelt).
pub fn after_slots_changed(container: &mut Container, before: &[Option<Stack>]) {
    if !is_oven(container.kind) {
        return;
    }

    let now = container.slots.get(INPUT).copied().flatten();
    let was = before.get(INPUT).copied().flatten();
    let same = match (now, was) {
        (Some(now), Some(was)) => now.stacks_with(&was),
        (None, None) => true,
        _ => false,
    };

    if !same {
        container.heat.cook = 0;
        container.heat.cook_total = cook_time(container.kind);
    }
}

/// Жар из данных блока в команде (`setblock … furnace{…}`): поля
/// `lit_time_remaining`, `lit_total_time`, `cooking_time_spent`,
/// `cooking_total_time`, у стойки — `BrewTime` и `Fuel`. Чего нет — ноль:
/// печь, поставленная с рудой без `cooking_total_time`, плавит бесконечно
/// (чёрный ящик, первая запись furnace-smelt: свойство 3 — 0, свойство 2
/// растёт за 400).
pub fn heat_from_text(container: &mut Container, text: &str) {
    let number = |key: &str| -> i32 {
        let Some(at) = text.find(&format!("{key}:")) else {
            return 0;
        };
        let rest = &text[at + key.len() + 1..];
        let end = rest.find([',', '}']).unwrap_or(rest.len());

        rest[..end].trim().trim_end_matches(['s', 'S', 'b', 'B']).parse().unwrap_or(0)
    };

    if is_oven(container.kind) {
        container.heat = Heat {
            lit: number("lit_time_remaining"),
            lit_total: number("lit_total_time"),
            cook: number("cooking_time_spent"),
            cook_total: number("cooking_total_time"),
            ..Heat::default()
        };
    } else if container.kind == Kind::BrewingStand {
        container.heat = Heat { lit: number("BrewTime"), lit_total: number("Fuel"), ..Heat::default() };
    }
}

/// Свойства окна (Set Container Property) по порядку номеров. Печь: 0 —
/// сколько ещё горит, 1 — на сколько было топливо, 2 — сколько плавится,
/// 3 — сколько нужно. Стойка: 0 — сколько ещё варится, 1 — заряды порошка
/// (чёрный ящик, furnace-smelt и brewing-awkward).
pub fn properties(container: &Container) -> Vec<i16> {
    let heat = &container.heat;
    let short = |value: i32| value.clamp(i16::MIN as i32, i16::MAX as i32) as i16;

    match container.kind {
        Kind::Furnace | Kind::BlastFurnace | Kind::Smoker => {
            vec![short(heat.lit), short(heat.lit_total), short(heat.cook), short(heat.cook_total)]
        }
        Kind::BrewingStand => vec![short(heat.lit), short(heat.lit_total)],
        Kind::Beacon => container.beacon.iter().map(|v| short(*v)).collect(),
        _ => Vec::new(),
    }
}

pub fn properties_for_catalog(container: &Container, catalog: crate::gameplay_catalog::Catalog) -> Vec<i16> {
    let mut props = properties(container);
    if container.kind == Kind::BrewingStand && catalog.profile() == crate::gameplay_catalog::Profile::Preview26_3 {
        // Оригинал777, brewing-awkward/swiftness: кроме остатка/топлива,
        // свойства2/3 содержат полное время варки и ёмкость порошка.
        // brewing-awkward: до первой заправки ёмкость0, после неё20;
        // остаток может стать0, но полное время уже начатой варки сохраняется.
        let capacity = if container.heat.lit_total > 0 || container.heat.cook_total > 0 { POWDER_CHARGES } else { 0 };
        props.extend([container.heat.cook_total.clamp(0, i16::MAX as i32) as i16, capacity as i16]);
    }
    props
}

/// Игрок забрал выплавленное: весь накопленный опыт уходит ему, дробная
/// часть — шансом на ещё одно очко (вики, Smelting: «0.2×6 = 1.2, so this
/// grants 1 point, plus a 20% chance of another 1 point»). Возвращает очки.
pub fn take_experience(world: &mut World, (x, y, z): Pos) -> i32 {
    let stored = world
        .with_container(x, y, z, |container| std::mem::take(&mut container.heat.xp))
        .unwrap_or(0.0);

    whole_points(stored, world.random_unit())
}

/// Очки опыта из дробного: целая часть и шанс по дробной.
pub fn whole_points(stored: f32, roll: f32) -> i32 {
    let whole = stored.floor();
    let extra = i32::from(roll < stored - whole);

    whole as i32 + extra
}

// ---------------------------------------------------------------------------
// Такт
// ---------------------------------------------------------------------------

/// Такт печи — фаза блочных сущностей. Порядок снят чёрным ящиком
/// (furnace-smelt): горящая печь сперва теряет такт горения; погасшая, если
/// есть что плавить, сжигает топливо (в том же такте: свойства 0 и 1 —
/// 1600, плавка 1); горящая с годным входом плавит на такт больше, и на
/// `cook_total` предмет готов. Без огня плавка откатывается по 2 за такт
/// (вики, Smelting: «rewinds at double speed»).
pub fn tick_oven(world: &mut World, pos: Pos) {
    let (x, y, z) = pos;
    let Some(container) = world.container(x, y, z) else {
        return;
    };
    let kind = container.kind;
    let mut heat = container.heat;
    let mut slots = container.slots.clone();
    let was_lit = heat.lit > 0;

    if heat.lit > 0 {
        heat.lit -= 1;
    }

    let recipe = slots[INPUT].and_then(|input| smelt(kind, input.item));
    let room = |slots: &[Option<Stack>], result: i32| match slots[OUTPUT] {
        None => true,
        Some(out) => out.item == result && out.data.is_none() && out.tnt.is_none() && out.count < blocks::stack_size(result),
    };
    let can = recipe.is_some_and(|(result, _)| room(&slots, result));
    let fuel = slots[FUEL].and_then(|stack| burn_in(kind, stack.item).map(|time| (stack, time)));

    if heat.lit == 0
        && can
        && let Some((stack, time)) = fuel
    {
        heat.lit = time;
        heat.lit_total = time;

        // Лава сгорает, ведро остаётся (вики, Smelting).
        slots[FUEL] = if name_of(stack.item) == "lava_bucket" {
            item("bucket").map(|bucket| Stack::new(bucket, 1))
        } else {
            (stack.count > 1).then(|| stack.with_count(stack.count - 1))
        };
    }

    if heat.lit > 0 && can {
        heat.cook += 1;

        if heat.cook == heat.cook_total
            && let Some((result, xp)) = recipe
        {
            heat.cook = 0;
            heat.cook_total = cook_time(kind);
            heat.xp += xp;

            let input = slots[INPUT].expect("вход проверен рецептом");

            // Мокрая губка сушится, и ведро в топливе наполняется (вики,
            // Smelting: «If an empty bucket is in the fuel slot…»).
            if name_of(input.item) == "wet_sponge"
                && slots[FUEL].is_some_and(|fuel| name_of(fuel.item) == "bucket" && fuel.count == 1)
            {
                slots[FUEL] = item("water_bucket").map(|water| Stack::new(water, 1));
            }

            slots[OUTPUT] = Some(match slots[OUTPUT] {
                Some(out) => out.with_count(out.count + 1),
                None => Stack::new(result, 1),
            });
            slots[INPUT] = (input.count > 1).then(|| input.with_count(input.count - 1));
        }
    } else if heat.lit == 0 && heat.cook > 0 {
        heat.cook = (heat.cook - 2).clamp(0, heat.cook_total.max(0));
    } else if !can {
        // Горит, но плавить нечего. [проверить]
        heat.cook = 0;
    }

    let lit = heat.lit > 0;

    // Без перемен чанк не трогаем — иначе он писался бы на диск каждый такт.
    if world.container(x, y, z).is_some_and(|c| c.slots != slots || c.heat != heat) {
        world.with_container(x, y, z, |container| {
            container.slots = slots;
            container.heat = heat;
        });
    }

    if was_lit != lit {
        let state = world.get_block(x, y, z);

        if let Some(next) = blocks::orientation_of(state).and_then(|o| o.with(state, "lit", if lit { "true" } else { "false" })) {
            world.set_block(x, y, z, next);
        }
    }
}

/// Такт стойки. Снято чёрным ящиком (brewing-awkward): порошок кладётся
/// в топливо, когда зарядов нет; варка начинается в том же такте и сразу
/// берёт заряд (свойства 400 и 19); на последнем такте варки — событие 1035
/// и новые бутылки, окно видит их тактом позже.
pub fn tick_brewing(world: &mut World, pos: Pos) {
    let (x, y, z) = pos;
    let Some(container) = world.container(x, y, z) else {
        return;
    };
    let mut heat = container.heat;
    let mut slots = container.slots.clone();
    let mut brewed = false;

    if heat.lit_total <= 0
        && let Some(powder) = slots[POWDER].filter(|stack| name_of(stack.item) == "blaze_powder")
    {
        heat.lit_total = POWDER_CHARGES;
        slots[POWDER] = (powder.count > 1).then(|| powder.with_count(powder.count - 1));
    }

    let can = can_brew(&slots);
    let ingredient = slots[INGREDIENT].map_or(0, |stack| stack.item);

    if heat.lit > 0 {
        heat.lit -= 1;

        if heat.lit == 0 && can {
            brew(&mut slots);
            brewed = true;
        } else if !can || ingredient != heat.ingredient {
            heat.lit = 0;
        }
    } else if can && heat.lit_total > 0 {
        heat.lit_total -= 1;
        heat.lit = BREW_TIME;
        heat.ingredient = ingredient;
        if world.catalog().profile() == crate::gameplay_catalog::Profile::Preview26_3 {
            heat.cook_total = BREW_TIME;
        }
    }

    if world.container(x, y, z).is_some_and(|c| c.slots != slots || c.heat != heat) {
        world.with_container(x, y, z, |container| {
            container.slots.clone_from(&slots);
            container.heat = heat;
        });
    }

    if brewed {
        world.note_level_event(x, y, z, 1035, 0);
    }

    // Бутылки видны на блоке (`has_bottle_N`).
    let mut state = world.get_block(x, y, z);
    let before = state;

    for (index, bottle) in slots.iter().take(3).enumerate() {
        let has = if bottle.is_some() { "true" } else { "false" };

        if let Some(next) = blocks::orientation_of(state).and_then(|o| o.with(state, &format!("has_bottle_{index}"), has)) {
            state = next;
        }
    }

    if state != before {
        world.set_block(x, y, z, state);
    }
}

/// Варка готова: каждая бутылка, которую ингредиент меняет, меняется,
/// ингредиента становится на один меньше. Драконье дыхание оставляет
/// пустую бутылку, если ячейка опустела. [проверить] иначе она выпадает.
fn brew(slots: &mut [Option<Stack>]) {
    let Some(ingredient) = slots[INGREDIENT] else {
        return;
    };

    for slot in slots.iter_mut().take(3) {
        if let Some(bottle) = *slot
            && let Some(next) = brew_bottle(bottle, ingredient.item)
        {
            *slot = Some(next);
        }
    }

    slots[INGREDIENT] = if ingredient.count > 1 {
        Some(ingredient.with_count(ingredient.count - 1))
    } else if name_of(ingredient.item) == "dragon_breath" {
        item("glass_bottle").map(|bottle| Stack::new(bottle, 1))
    } else {
        None
    };
}

/// Запись жара на диск — после ячеек хранилища.
pub fn write_heat(heat: &Heat, out: &mut Vec<u8>) {
    for value in [heat.lit, heat.lit_total, heat.cook, heat.cook_total, heat.ingredient] {
        out.extend_from_slice(&value.to_be_bytes());
    }

    out.extend_from_slice(&heat.xp.to_be_bytes());
}

/// Чтение жара, записанного `write_heat`.
pub fn read_heat(take: &mut dyn FnMut(usize) -> Option<Vec<u8>>) -> Option<Heat> {
    let mut number = || Some(i32::from_be_bytes(take(4)?.try_into().ok()?));

    let (lit, lit_total, cook, cook_total, ingredient) = (number()?, number()?, number()?, number()?, number()?);
    let xp = f32::from_be_bytes(take(4)?.try_into().ok()?);

    Some(Heat { lit, lit_total, cook, cook_total, xp, ingredient })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(name: &str) -> i32 {
        blocks::item_named(name).unwrap_or_else(|| panic!("нет предмета {name}"))
    }

    #[test]
    fn recipes_follow_the_wiki_tables() {
        assert_eq!(smelt(Kind::Furnace, id("iron_ore")), Some((id("iron_ingot"), 0.7)));
        assert_eq!(smelt(Kind::BlastFurnace, id("raw_iron")), Some((id("iron_ingot"), 0.7)));
        assert_eq!(smelt(Kind::Smoker, id("beef")), Some((id("cooked_beef"), 0.35)));
        // Коптильня не плавит руду, плавильная — еду.
        assert_eq!(smelt(Kind::Smoker, id("raw_iron")), None);
        assert_eq!(smelt(Kind::BlastFurnace, id("beef")), None);
        assert_eq!(smelt(Kind::BlastFurnace, id("cobblestone")), None);
        assert_eq!(smelt(Kind::Furnace, id("cobblestone")), Some((id("stone"), 0.1)));
        assert_eq!(smelt(Kind::Furnace, id("stripped_cherry_wood")), Some((id("charcoal"), 0.15)));
        assert_eq!(smelt(Kind::Furnace, id("crimson_stem")), None);
        assert_eq!(smelt(Kind::Furnace, id("lime_terracotta")), Some((id("lime_glazed_terracotta"), 0.1)));
        assert_eq!(smelt(Kind::BlastFurnace, id("chainmail_boots")), Some((id("iron_nugget"), 0.1)));
        assert_eq!(smelt(Kind::Furnace, id("golden_horse_armor")), Some((id("gold_nugget"), 0.1)));
        assert_eq!(smelt(Kind::Furnace, id("deepslate_diamond_ore")), Some((id("diamond"), 1.0)));
        assert_eq!(smelt(Kind::Furnace, id("cactus")), Some((id("green_dye"), 1.0)));
    }

    #[test]
    fn fuel_burns_as_in_the_wiki_table() {
        assert_eq!(burn_time(id("coal")), Some(1600));
        assert_eq!(burn_time(id("lava_bucket")), Some(20000));
        assert_eq!(burn_time(id("oak_planks")), Some(300));
        assert_eq!(burn_time(id("oak_slab")), Some(150));
        assert_eq!(burn_time(id("crimson_planks")), None);
        assert_eq!(burn_time(id("stick")), Some(100));
        assert_eq!(burn_time(id("white_carpet")), Some(67));
        assert_eq!(burn_time(id("moss_carpet")), None);
        assert_eq!(burn_time(id("bamboo")), Some(50));
        assert_eq!(burn_time(id("mangrove_chest_boat")), Some(1200));
        assert_eq!(burn_time(id("oak_button")), Some(100));
        assert_eq!(burn_time(id("stone_button")), None);
        // Плавильная и коптильня жгут вдвое быстрее (furnace-blast).
        assert_eq!(burn_in(Kind::BlastFurnace, id("coal")), Some(800));
        assert_eq!(burn_in(Kind::Smoker, id("stick")), Some(50));
    }

    #[test]
    fn potions_brew_by_the_wiki_table() {
        assert_eq!(mix("water", "nether_wart"), Some("awkward"));
        assert_eq!(mix("water", "sugar"), Some("mundane"));
        assert_eq!(mix("water", "golden_carrot"), None);
        assert_eq!(mix("awkward", "sugar"), Some("swiftness"));
        assert_eq!(mix("swiftness", "redstone"), Some("long_swiftness"));
        assert_eq!(mix("swiftness", "glowstone_dust"), Some("strong_swiftness"));
        assert_eq!(mix("fire_resistance", "glowstone_dust"), None);
        assert_eq!(mix("long_swiftness", "glowstone_dust"), None);
        assert_eq!(mix("strong_swiftness", "fermented_spider_eye"), None);
        assert_eq!(mix("long_night_vision", "fermented_spider_eye"), Some("long_invisibility"));
        assert_eq!(mix("slowness", "glowstone_dust"), Some("strong_slowness"));
        assert_eq!(mix("awkward", "turtle_helmet"), Some("turtle_master"));
        assert_eq!(mix("turtle_master", "glowstone_dust"), Some("strong_turtle_master"));
    }

    #[test]
    fn gunpowder_and_breath_change_the_bottle() {
        let swift = Stack::new(id("potion"), 1)
            .with_extras(crate::stackdata::Data { potion: crate::survival::potion_number("swiftness"), ..Default::default() });
        let splash = brew_bottle(swift, id("gunpowder")).expect("взрывное");

        assert_eq!(splash.item, id("splash_potion"));
        assert_eq!(splash.extras().potion, crate::survival::potion_number("swiftness"));
        assert_eq!(brew_bottle(splash, id("dragon_breath")).map(|stack| stack.item), Some(id("lingering_potion")));
        assert_eq!(brew_bottle(swift, id("dragon_breath")), None);
    }

    #[test]
    fn brewing_263_publishes_total_duration_and_fuel_capacity() {
        use crate::gameplay_catalog::{Catalog, Profile};
        for profile in [Profile::Java26_1_2, Profile::Preview26_3] {
            let catalog=Catalog::new(profile);
            let mut world=World::in_memory_with_catalog(catalog);
            world.set_block(0,0,0,blocks::state_by_name("brewing_stand").unwrap());
            let initial=if profile==Profile::Preview26_3 {vec![0,0,0,0]} else {vec![0,0]};
            assert_eq!(properties_for_catalog(world.container(0,0,0).unwrap(),catalog),initial);
            world.with_container(0,0,0,|container| {
                container.slots[0]=Some(Stack::new(id("potion"),1).with_extras(crate::stackdata::Data {
                    potion:crate::survival::potion_number("water"),..Default::default()
                }));
                container.slots[INGREDIENT]=Some(Stack::new(id("nether_wart"),1));
                container.slots[POWDER]=Some(Stack::new(id("blaze_powder"),1));
            });
            tick_brewing(&mut world,(0,0,0));
            let expected=if profile==Profile::Preview26_3 {vec![400,19,400,20]} else {vec![400,19]};
            assert_eq!(properties_for_catalog(world.container(0,0,0).unwrap(),catalog),expected);
            for _ in 0..400 {tick_brewing(&mut world,(0,0,0));}
            let container=world.container(0,0,0).unwrap();
            assert_eq!(container.slots[0].unwrap().extras().potion,crate::survival::potion_number("awkward"));
            let expected=if profile==Profile::Preview26_3 {vec![0,19,400,20]} else {vec![0,19]};
            assert_eq!(properties_for_catalog(container,catalog),expected);
        }
    }

    #[test]
    fn a_furnace_smelts_like_the_original() {
        let mut container = Container::new(Kind::Furnace);
        let before = container.slots.clone();

        container.slots[INPUT] = Some(Stack::new(id("iron_ore"), 2));
        after_slots_changed(&mut container, &before);
        assert_eq!(properties(&container), vec![0, 0, 0, 200]);
        assert!(may_place(Kind::Furnace, FUEL, id("coal")));
        assert!(!may_place(Kind::Furnace, FUEL, id("iron_ore")));
        assert!(!may_place(Kind::Furnace, OUTPUT, id("coal")));
        assert_eq!(insert_slots(Kind::Furnace, (0, -1, 0), id("dirt")), Some(vec![INPUT]));
        assert_eq!(insert_slots(Kind::Furnace, (1, 0, 0), id("dirt")), Some(Vec::new()));
        assert_eq!(insert_slots(Kind::Furnace, (1, 0, 0), id("coal")), Some(vec![FUEL]));
        assert!(may_extract(Kind::Furnace, OUTPUT, id("iron_ingot")));
        assert!(!may_extract(Kind::Furnace, INPUT, id("iron_ore")));
    }

    #[test]
    fn experience_rounds_by_chance() {
        assert_eq!(whole_points(1.4, 0.5), 1);
        assert_eq!(whole_points(1.4, 0.3), 2);
        assert_eq!(whole_points(2.0, 0.0), 2);
    }

    #[test]
    fn heat_comes_from_command_data() {
        let mut container = Container::new(Kind::Furnace);

        heat_from_text(&mut container, "{Items:[],lit_time_remaining:30s,cooking_total_time:200s}");
        assert_eq!(properties(&container), vec![30, 0, 0, 200]);
    }
}
