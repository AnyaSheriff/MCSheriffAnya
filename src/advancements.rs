// Достижения (вики, Advancement): дерево из пяти вкладок, условия, выдача и
// запись по файлу на игрока.
//
// Дерево составлено по таблицам вики (названия — ключами перевода
// `advancements.<путь>.title/.description`, значок, рамка, родитель, опыт
// награды); расклад по экрану (x, y) оригинал считает при загрузке, тут —
// своё выравнивание деревом [проверить]. Книга рецептов — recipe_book.rs.
//
// Файл — `world/players/advancements/<uuid>.json`, как у оригинала 26.1:
// `{"minecraft:<путь>": {"criteria": {"<условие>": "<время>"}, "done": bool},
// "DataVersion": N}` (снято с мира чёрного ящика). Клиенту уходит Update
// Advancements: видимые достижения, их условия и время выполнения.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde_json::{Value, json};

use crate::network::varint::{encode_string, encode_varint};

pub const DIRECTORY: &str = "world/players/advancements";

/// Update Advancements (minecraft-data 26.1: advancements).
pub const UPDATE_ADVANCEMENTS: i32 = 0x82;
/// Serverbound Seen Advancements (advancement_tab).
pub const SEEN_ADVANCEMENTS: i32 = 0x32;

#[derive(Debug)]
pub enum Criteria {
    /// Одно условие.
    One(&'static str),
    /// Любое из (одна группа «или»).
    Any(&'static [&'static str]),
    /// Все (каждое — своя группа).
    All(&'static [&'static str]),
}

#[derive(Debug)]
pub struct Def {
    pub id: &'static str,
    /// Название по-английски — для строки консоли, как её пишет оригинал.
    pub title: &'static str,
    pub parent: Option<&'static str>,
    /// Значок — предмет.
    pub icon: &'static str,
    /// 0 — задача, 1 — испытание, 2 — цель.
    pub frame: u8,
    pub hidden: bool,
    /// Опыт награды.
    pub xp: i32,
    pub criteria: Criteria,
}

impl Criteria {
    pub fn names(&self) -> Vec<String> {
        match self {
            Criteria::One(name) => vec![name.to_string()],
            Criteria::Any(list) | Criteria::All(list) => list.iter().map(|name| name.to_string()).collect(),
        }
    }

    /// Требования: группы «или», связанные «и».
    pub fn requirements(&self) -> Vec<Vec<String>> {
        match self {
            Criteria::One(name) => vec![vec![name.to_string()]],
            Criteria::Any(list) => vec![list.iter().map(|name| name.to_string()).collect()],
            Criteria::All(list) => list.iter().map(|name| vec![name.to_string()]).collect(),
        }
    }
}

impl Def {
    pub fn is_root(&self) -> bool {
        self.parent.is_none()
    }

    /// Корень показывается своим фоном.
    pub fn background(&self) -> Option<&'static str> {
        self.is_root().then_some(match self.id {
            "story/root" => "minecraft:gui/advancements/backgrounds/stone",
            "nether/root" => "minecraft:gui/advancements/backgrounds/nether",
            "end/root" => "minecraft:gui/advancements/backgrounds/end",
            "adventure/root" => "minecraft:gui/advancements/backgrounds/adventure",
            _ => "minecraft:gui/advancements/backgrounds/husbandry",
        })
    }

    /// Ключ перевода: `story/mine_stone` → `advancements.story.mine_stone`.
    pub fn key(&self) -> String {
        format!("advancements.{}", self.id.replace('/', "."))
    }

    pub fn title(&self) -> Value {
        json!({"translate": format!("{}.title", self.key())})
    }

    pub fn description(&self) -> Value {
        json!({"translate": format!("{}.description", self.key())})
    }

}

const MOBS: [&str; 41] = [
    "minecraft:blaze", "minecraft:bogged", "minecraft:breeze", "minecraft:camel_husk", "minecraft:cave_spider", "minecraft:creaking",
    "minecraft:creeper", "minecraft:drowned", "minecraft:elder_guardian", "minecraft:ender_dragon", "minecraft:enderman",
    "minecraft:endermite", "minecraft:evoker", "minecraft:ghast", "minecraft:guardian", "minecraft:hoglin", "minecraft:husk",
    "minecraft:magma_cube", "minecraft:parched", "minecraft:phantom", "minecraft:piglin", "minecraft:piglin_brute",
    "minecraft:pillager", "minecraft:ravager", "minecraft:shulker", "minecraft:silverfish", "minecraft:skeleton", "minecraft:slime",
    "minecraft:spider", "minecraft:stray", "minecraft:vex", "minecraft:vindicator", "minecraft:witch", "minecraft:wither",
    "minecraft:wither_skeleton", "minecraft:zoglin", "minecraft:zombie", "minecraft:zombie_horse", "minecraft:zombie_nautilus",
    "minecraft:zombie_villager", "minecraft:zombified_piglin",
];

const ANIMALS: [&str; 26] = [
    "minecraft:armadillo", "minecraft:axolotl", "minecraft:bee", "minecraft:camel", "minecraft:cat", "minecraft:chicken",
    "minecraft:cow", "minecraft:donkey", "minecraft:fox", "minecraft:frog", "minecraft:goat", "minecraft:hoglin", "minecraft:horse",
    "minecraft:llama", "minecraft:mooshroom", "minecraft:mule", "minecraft:nautilus", "minecraft:ocelot", "minecraft:panda",
    "minecraft:pig", "minecraft:rabbit", "minecraft:sheep", "minecraft:sniffer", "minecraft:strider", "minecraft:turtle", "minecraft:wolf",
];

const CATS: [&str; 11] = [
    "minecraft:all_black", "minecraft:british_shorthair", "minecraft:calico", "minecraft:jellie", "minecraft:persian",
    "minecraft:ragdoll", "minecraft:red", "minecraft:siamese", "minecraft:tabby", "minecraft:white", "minecraft:black",
];

const WOLVES: [&str; 9] = [
    "minecraft:ashen", "minecraft:black", "minecraft:chestnut", "minecraft:pale", "minecraft:rusty", "minecraft:snowy",
    "minecraft:spotted", "minecraft:striped", "minecraft:woods",
];

const FOODS: [&str; 40] = [
    "apple", "baked_potato", "beef", "beetroot", "beetroot_soup", "bread", "carrot", "chicken", "chorus_fruit", "cod", "cooked_beef",
    "cooked_chicken", "cooked_cod", "cooked_mutton", "cooked_porkchop", "cooked_rabbit", "cooked_salmon", "cookie", "dried_kelp",
    "enchanted_golden_apple", "glow_berries", "golden_apple", "golden_carrot", "honey_bottle", "melon_slice", "mushroom_stew",
    "mutton", "poisonous_potato", "porkchop", "potato", "pufferfish", "pumpkin_pie", "rabbit", "rabbit_stew", "rotten_flesh",
    "salmon", "spider_eye", "suspicious_stew", "sweet_berries", "tropical_fish",
];

/// Биомы обычного мира для «Время приключений», срез 26.1.2 [проверить].
pub const BIOMES: [&str; 54] = [
    "minecraft:badlands", "minecraft:bamboo_jungle", "minecraft:beach", "minecraft:birch_forest", "minecraft:cherry_grove",
    "minecraft:cold_ocean", "minecraft:dark_forest", "minecraft:deep_cold_ocean", "minecraft:deep_dark", "minecraft:deep_frozen_ocean",
    "minecraft:deep_lukewarm_ocean", "minecraft:deep_ocean", "minecraft:desert", "minecraft:dripstone_caves", "minecraft:eroded_badlands",
    "minecraft:flower_forest", "minecraft:forest", "minecraft:frozen_ocean", "minecraft:frozen_peaks", "minecraft:frozen_river",
    "minecraft:grove", "minecraft:ice_spikes", "minecraft:jagged_peaks", "minecraft:jungle", "minecraft:lukewarm_ocean",
    "minecraft:lush_caves", "minecraft:mangrove_swamp", "minecraft:meadow", "minecraft:mushroom_fields", "minecraft:ocean",
    "minecraft:old_growth_birch_forest", "minecraft:old_growth_pine_taiga", "minecraft:old_growth_spruce_taiga", "minecraft:pale_garden",
    "minecraft:plains", "minecraft:river", "minecraft:savanna", "minecraft:savanna_plateau", "minecraft:snowy_beach", "minecraft:snowy_plains",
    "minecraft:snowy_slopes", "minecraft:snowy_taiga", "minecraft:sparse_jungle", "minecraft:stony_peaks", "minecraft:stony_shore",
    "minecraft:sunflower_plains", "minecraft:swamp", "minecraft:taiga", "minecraft:warm_ocean", "minecraft:windswept_forest",
    "minecraft:windswept_gravelly_hills", "minecraft:windswept_hills", "minecraft:windswept_savanna", "minecraft:wooded_badlands",
];

const TRIMS: [&str; 8] = [
    "armor_trimmed_minecraft:spire_armor_trim_smithing_template_smithing_trim", "armor_trimmed_minecraft:snout_armor_trim_smithing_template_smithing_trim",
    "armor_trimmed_minecraft:rib_armor_trim_smithing_template_smithing_trim", "armor_trimmed_minecraft:ward_armor_trim_smithing_template_smithing_trim",
    "armor_trimmed_minecraft:silence_armor_trim_smithing_template_smithing_trim", "armor_trimmed_minecraft:vex_armor_trim_smithing_template_smithing_trim",
    "armor_trimmed_minecraft:tide_armor_trim_smithing_template_smithing_trim", "armor_trimmed_minecraft:wayfinder_armor_trim_smithing_template_smithing_trim",
];

pub const TREE: &[Def] = &[
    Def { id: "story/root", title: "Minecraft", parent: None, icon: "grass_block", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("crafting_table") },
    Def { id: "story/mine_stone", title: "Stone Age", parent: Some("story/root"), icon: "wooden_pickaxe", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("get_stone") },
    Def { id: "story/upgrade_tools", title: "Getting an Upgrade", parent: Some("story/mine_stone"), icon: "stone_pickaxe", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("stone_pickaxe") },
    Def { id: "story/smelt_iron", title: "Acquire Hardware", parent: Some("story/upgrade_tools"), icon: "iron_ingot", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("iron") },
    Def { id: "story/obtain_armor", title: "Suit Up", parent: Some("story/smelt_iron"), icon: "iron_chestplate", frame: 0, hidden: false, xp: 0, criteria: Criteria::Any(&["iron_helmet", "iron_chestplate", "iron_leggings", "iron_boots"]) },
    Def { id: "story/lava_bucket", title: "Hot Stuff", parent: Some("story/smelt_iron"), icon: "lava_bucket", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("lava_bucket") },
    Def { id: "story/iron_tools", title: "Isn't It Iron Pick", parent: Some("story/smelt_iron"), icon: "iron_pickaxe", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("iron_pickaxe") },
    Def { id: "story/deflect_arrow", title: "Not Today, Thank You", parent: Some("story/obtain_armor"), icon: "shield", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("deflected_projectile") },
    Def { id: "story/form_obsidian", title: "Ice Bucket Challenge", parent: Some("story/lava_bucket"), icon: "obsidian", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("obsidian") },
    Def { id: "story/mine_diamond", title: "Diamonds!", parent: Some("story/iron_tools"), icon: "diamond", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("diamond") },
    Def { id: "story/enter_the_nether", title: "We Need to Go Deeper", parent: Some("story/form_obsidian"), icon: "flint_and_steel", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("entered_nether") },
    Def { id: "story/shiny_gear", title: "Cover Me with Diamonds", parent: Some("story/mine_diamond"), icon: "diamond_chestplate", frame: 0, hidden: false, xp: 0, criteria: Criteria::Any(&["diamond_helmet", "diamond_chestplate", "diamond_leggings", "diamond_boots"]) },
    Def { id: "story/enchant_item", title: "Enchanter", parent: Some("story/mine_diamond"), icon: "enchanted_book", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("enchanted_item") },
    Def { id: "story/cure_zombie_villager", title: "Zombie Doctor", parent: Some("story/enter_the_nether"), icon: "golden_apple", frame: 2, hidden: false, xp: 0, criteria: Criteria::One("cured_zombie") },
    Def { id: "story/follow_ender_eye", title: "Eye Spy", parent: Some("story/enter_the_nether"), icon: "ender_eye", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("in_stronghold") },
    Def { id: "story/enter_the_end", title: "The End?", parent: Some("story/follow_ender_eye"), icon: "end_stone", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("entered_end") },
    Def { id: "nether/root", title: "Nether", parent: None, icon: "red_nether_bricks", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("entered_nether") },
    Def { id: "nether/return_to_sender", title: "Return to Sender", parent: Some("nether/root"), icon: "fire_charge", frame: 1, hidden: false, xp: 50, criteria: Criteria::One("killed_ghast") },
    Def { id: "nether/find_bastion", title: "Those Were the Days", parent: Some("nether/root"), icon: "polished_blackstone_bricks", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("bastion") },
    Def { id: "nether/obtain_ancient_debris", title: "Hidden in the Depths", parent: Some("nether/root"), icon: "ancient_debris", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("ancient_debris") },
    Def { id: "nether/fast_travel", title: "Subspace Bubble", parent: Some("nether/root"), icon: "map", frame: 1, hidden: false, xp: 100, criteria: Criteria::One("travelled") },
    Def { id: "nether/find_fortress", title: "A Terrible Fortress", parent: Some("nether/root"), icon: "nether_bricks", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("fortress") },
    Def { id: "nether/obtain_crying_obsidian", title: "Who is Cutting Onions?", parent: Some("nether/root"), icon: "crying_obsidian", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("crying_obsidian") },
    Def { id: "nether/distract_piglin", title: "Oh Shiny", parent: Some("nether/root"), icon: "gold_ingot", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("consume_item") },
    Def { id: "nether/ride_strider", title: "This Boat Has Legs", parent: Some("nether/root"), icon: "warped_fungus_on_a_stick", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("used_warped_fungus_on_a_stick") },
    Def { id: "nether/uneasy_alliance", title: "Uneasy Alliance", parent: Some("nether/return_to_sender"), icon: "ghast_tear", frame: 1, hidden: false, xp: 100, criteria: Criteria::One("killed_ghast") },
    Def { id: "nether/loot_bastion", title: "War Pigs", parent: Some("nether/find_bastion"), icon: "chest", frame: 0, hidden: false, xp: 0, criteria: Criteria::Any(&["loot_bastion_other", "loot_bastion_treasure", "loot_bastion_hoglin_stable", "loot_bastion_bridge"]) },
    Def { id: "nether/netherite_armor", title: "Cover Me in Debris", parent: Some("nether/obtain_ancient_debris"), icon: "netherite_chestplate", frame: 1, hidden: false, xp: 100, criteria: Criteria::All(&["netherite_helmet", "netherite_chestplate", "netherite_leggings", "netherite_boots"]) },
    Def { id: "nether/get_wither_skull", title: "Spooky Scary Skeleton", parent: Some("nether/find_fortress"), icon: "wither_skeleton_skull", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("wither_skull") },
    Def { id: "nether/obtain_blaze_rod", title: "Into Fire", parent: Some("nether/find_fortress"), icon: "blaze_rod", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("blaze_rod") },
    Def { id: "nether/charge_respawn_anchor", title: "Not Quite \"Nine\" Lives", parent: Some("nether/obtain_crying_obsidian"), icon: "respawn_anchor", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("charged_respawn_anchor") },
    Def { id: "nether/ride_strider_in_overworld_lava", title: "Feels Like Home", parent: Some("nether/ride_strider"), icon: "warped_fungus_on_a_stick", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("ride_entity_distance") },
    Def { id: "nether/explore_nether", title: "Hot Tourist Destinations", parent: Some("nether/ride_strider"), icon: "netherite_boots", frame: 1, hidden: false, xp: 500, criteria: Criteria::All(&["nether_wastes", "soul_sand_valley", "crimson_forest", "warped_forest", "basalt_deltas"]) },
    Def { id: "nether/summon_wither", title: "Withering Heights", parent: Some("nether/get_wither_skull"), icon: "nether_star", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("summoned_entity") },
    Def { id: "nether/brew_potion", title: "Local Brewery", parent: Some("nether/obtain_blaze_rod"), icon: "potion", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("potion") },
    Def { id: "nether/create_beacon", title: "Bring Home the Beacon", parent: Some("nether/summon_wither"), icon: "beacon", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("construct_beacon") },
    Def { id: "nether/all_potions", title: "A Furious Cocktail", parent: Some("nether/brew_potion"), icon: "milk_bucket", frame: 1, hidden: false, xp: 100, criteria: Criteria::One("all_effects") },
    Def { id: "nether/create_full_beacon", title: "Beaconator", parent: Some("nether/create_beacon"), icon: "beacon", frame: 2, hidden: false, xp: 0, criteria: Criteria::One("construct_beacon") },
    Def { id: "nether/all_effects", title: "How Did We Get Here?", parent: Some("nether/all_potions"), icon: "bucket", frame: 1, hidden: true, xp: 1000, criteria: Criteria::One("all_effects") },
    Def { id: "end/root", title: "The End", parent: None, icon: "end_stone", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("entered_end") },
    Def { id: "end/kill_dragon", title: "Free the End", parent: Some("end/root"), icon: "dragon_head", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("killed_dragon") },
    Def { id: "end/dragon_egg", title: "The Next Generation", parent: Some("end/kill_dragon"), icon: "dragon_egg", frame: 2, hidden: false, xp: 0, criteria: Criteria::One("dragon_egg") },
    Def { id: "end/enter_end_gateway", title: "Remote Getaway", parent: Some("end/kill_dragon"), icon: "ender_pearl", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("entered_end_gateway") },
    Def { id: "end/respawn_dragon", title: "The End... Again...", parent: Some("end/kill_dragon"), icon: "end_crystal", frame: 2, hidden: false, xp: 0, criteria: Criteria::One("summoned_dragon") },
    Def { id: "end/dragon_breath", title: "You Need a Mint", parent: Some("end/kill_dragon"), icon: "dragon_breath", frame: 2, hidden: false, xp: 0, criteria: Criteria::One("dragon_breath") },
    Def { id: "end/find_end_city", title: "The City at the End of the Game", parent: Some("end/enter_end_gateway"), icon: "purpur_block", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("in_city") },
    Def { id: "end/elytra", title: "Sky's the Limit", parent: Some("end/find_end_city"), icon: "elytra", frame: 2, hidden: false, xp: 0, criteria: Criteria::One("elytra") },
    Def { id: "end/levitate", title: "Great View From Up Here", parent: Some("end/find_end_city"), icon: "shulker_shell", frame: 1, hidden: false, xp: 50, criteria: Criteria::One("levitated") },
    Def { id: "adventure/root", title: "Adventure", parent: None, icon: "map", frame: 0, hidden: false, xp: 0, criteria: Criteria::Any(&["killed_something", "killed_by_something"]) },
    Def { id: "adventure/heart_transplanter", title: "Heart Transplanter", parent: Some("adventure/root"), icon: "creaking_heart", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("place_creaking_heart") },
    Def { id: "adventure/voluntary_exile", title: "Voluntary Exile", parent: Some("adventure/root"), icon: "white_banner", frame: 0, hidden: true, xp: 0, criteria: Criteria::One("voluntary_exile") },
    Def { id: "adventure/use_lodestone", title: "Country Lode, Take Me Home", parent: Some("adventure/root"), icon: "lodestone", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("use_lodestone") },
    Def { id: "adventure/spyglass_at_parrot", title: "Is It a Bird?", parent: Some("adventure/root"), icon: "spyglass", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("spyglass_at_parrot") },
    Def { id: "adventure/kill_a_mob", title: "Monster Hunter", parent: Some("adventure/root"), icon: "iron_sword", frame: 0, hidden: false, xp: 0, criteria: Criteria::Any(&MOBS) },
    Def { id: "adventure/read_power_of_chiseled_bookshelf", title: "The Power of Books", parent: Some("adventure/root"), icon: "chiseled_bookshelf", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("chiseled_bookshelf") },
    Def { id: "adventure/trade", title: "What a Deal!", parent: Some("adventure/root"), icon: "emerald", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("traded") },
    Def { id: "adventure/trim_with_any_armor_pattern", title: "Crafting a New Look", parent: Some("adventure/root"), icon: "dune_armor_trim_smithing_template", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("armor_trimmed") },
    Def { id: "adventure/honey_block_slide", title: "Sticky Situation", parent: Some("adventure/root"), icon: "honey_block", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("honey_block_slide") },
    Def { id: "adventure/ol_betsy", title: "Ol' Betsy", parent: Some("adventure/root"), icon: "crossbow", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("shot_crossbow") },
    Def { id: "adventure/lightning_rod_with_villager_no_fire", title: "Surge Protector", parent: Some("adventure/root"), icon: "lightning_rod", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("lightning_rod_with_villager_no_fire") },
    Def { id: "adventure/fall_from_world_height", title: "Caves & Cliffs", parent: Some("adventure/root"), icon: "water_bucket", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("fall_from_world_height") },
    Def { id: "adventure/salvage_sherd", title: "Respecting the Remnants", parent: Some("adventure/root"), icon: "brush", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("desert_pyramid") },
    Def { id: "adventure/avoid_vibration", title: "Sneak 100", parent: Some("adventure/root"), icon: "sculk_sensor", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("avoid_vibration") },
    Def { id: "adventure/sleep_in_bed", title: "Sweet Dreams", parent: Some("adventure/root"), icon: "red_bed", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("slept_in_bed") },
    Def { id: "adventure/hero_of_the_village", title: "Hero of the Village", parent: Some("adventure/voluntary_exile"), icon: "white_banner", frame: 1, hidden: true, xp: 100, criteria: Criteria::One("hero_of_the_village") },
    Def { id: "adventure/spyglass_at_ghast", title: "Is It a Balloon?", parent: Some("adventure/spyglass_at_parrot"), icon: "spyglass", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("spyglass_at_ghast") },
    Def { id: "adventure/throw_trident", title: "A Throwaway Joke", parent: Some("adventure/kill_a_mob"), icon: "trident", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("shot_trident") },
    Def { id: "adventure/kill_mob_near_sculk_catalyst", title: "It Spreads", parent: Some("adventure/kill_a_mob"), icon: "sculk_catalyst", frame: 1, hidden: false, xp: 0, criteria: Criteria::One("kill_mob_near_sculk_catalyst") },
    Def { id: "adventure/shoot_arrow", title: "Take Aim", parent: Some("adventure/kill_a_mob"), icon: "bow", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("shot_arrow") },
    Def { id: "adventure/kill_all_mobs", title: "Monsters Hunted", parent: Some("adventure/kill_a_mob"), icon: "diamond_sword", frame: 1, hidden: false, xp: 100, criteria: Criteria::All(&MOBS) },
    Def { id: "adventure/totem_of_undying", title: "Postmortal", parent: Some("adventure/kill_a_mob"), icon: "totem_of_undying", frame: 2, hidden: false, xp: 0, criteria: Criteria::One("used_totem") },
    Def { id: "adventure/spear_many_mobs", title: "Mob Kabob", parent: Some("adventure/kill_a_mob"), icon: "iron_spear", frame: 2, hidden: false, xp: 0, criteria: Criteria::One("spear_many_mobs") },
    Def { id: "adventure/summon_iron_golem", title: "Hired Help", parent: Some("adventure/trade"), icon: "carved_pumpkin", frame: 2, hidden: false, xp: 0, criteria: Criteria::One("summoned_golem") },
    Def { id: "adventure/trade_at_world_height", title: "Star Trader", parent: Some("adventure/trade"), icon: "emerald", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("trade_at_world_height") },
    Def { id: "adventure/trim_with_all_exclusive_armor_patterns", title: "Smithing with Style", parent: Some("adventure/trim_with_any_armor_pattern"), icon: "silence_armor_trim_smithing_template", frame: 1, hidden: false, xp: 150, criteria: Criteria::All(&TRIMS) },
    Def { id: "adventure/two_birds_one_arrow", title: "Two Birds, One Arrow", parent: Some("adventure/ol_betsy"), icon: "crossbow", frame: 1, hidden: false, xp: 65, criteria: Criteria::One("two_birds") },
    Def { id: "adventure/whos_the_pillager_now", title: "Who's the Pillager Now?", parent: Some("adventure/ol_betsy"), icon: "crossbow", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("kill_pillager") },
    Def { id: "adventure/arbalistic", title: "Arbalistic", parent: Some("adventure/ol_betsy"), icon: "crossbow", frame: 1, hidden: true, xp: 85, criteria: Criteria::One("arbalistic") },
    Def { id: "adventure/craft_decorated_pot_using_only_sherds", title: "Careful Restoration", parent: Some("adventure/salvage_sherd"), icon: "decorated_pot", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("pot_crafted_using_only_sherds") },
    Def { id: "adventure/adventuring_time", title: "Adventuring Time", parent: Some("adventure/sleep_in_bed"), icon: "diamond_boots", frame: 1, hidden: false, xp: 500, criteria: Criteria::All(&BIOMES) },
    Def { id: "adventure/play_jukebox_in_meadows", title: "Sound of Music", parent: Some("adventure/sleep_in_bed"), icon: "jukebox", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("play_jukebox_in_meadows") },
    Def { id: "adventure/walk_on_powder_snow_with_leather_boots", title: "Light as a Rabbit", parent: Some("adventure/sleep_in_bed"), icon: "leather_boots", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("walk_on_powder_snow_with_leather_boots") },
    Def { id: "adventure/spyglass_at_dragon", title: "Is It a Plane?", parent: Some("adventure/spyglass_at_ghast"), icon: "spyglass", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("spyglass_at_dragon") },
    Def { id: "adventure/very_very_frightening", title: "Very Very Frightening", parent: Some("adventure/throw_trident"), icon: "trident", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("struck_villager") },
    Def { id: "adventure/sniper_duel", title: "Sniper Duel", parent: Some("adventure/shoot_arrow"), icon: "arrow", frame: 1, hidden: false, xp: 50, criteria: Criteria::One("killed_skeleton") },
    Def { id: "adventure/bullseye", title: "Bullseye", parent: Some("adventure/shoot_arrow"), icon: "target", frame: 1, hidden: false, xp: 50, criteria: Criteria::One("bullseye") },
    Def { id: "adventure/brush_armadillo", title: "Isn't It Scute?", parent: Some("adventure/root"), icon: "armadillo_scute", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("brush_armadillo") },
    Def { id: "adventure/minecraft_trials_edition", title: "Minecraft: Trial(s) Edition", parent: Some("adventure/root"), icon: "chiseled_tuff", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("trial_chamber") },
    Def { id: "adventure/crafters_crafting_crafters", title: "Crafters Crafting Crafters", parent: Some("adventure/root"), icon: "crafter", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("crafter_crafted_crafter") },
    Def { id: "adventure/lighten_up", title: "Lighten Up", parent: Some("adventure/minecraft_trials_edition"), icon: "copper_bulb", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("lighten_up") },
    Def { id: "adventure/who_needs_rockets", title: "Who Needs Rockets?", parent: Some("adventure/minecraft_trials_edition"), icon: "wind_charge", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("use_wind_charge") },
    Def { id: "adventure/under_lock_and_key", title: "Under Lock and Key", parent: Some("adventure/minecraft_trials_edition"), icon: "trial_key", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("open_vault") },
    Def { id: "adventure/revaulting", title: "Revaulting", parent: Some("adventure/under_lock_and_key"), icon: "ominous_trial_key", frame: 2, hidden: false, xp: 0, criteria: Criteria::One("open_vault") },
    Def { id: "adventure/blowback", title: "Blowback", parent: Some("adventure/minecraft_trials_edition"), icon: "wind_charge", frame: 1, hidden: false, xp: 40, criteria: Criteria::One("kill_breeze") },
    Def { id: "adventure/overoverkill", title: "Over-Overkill", parent: Some("adventure/minecraft_trials_edition"), icon: "mace", frame: 1, hidden: false, xp: 50, criteria: Criteria::One("overoverkill") },
    Def { id: "husbandry/root", title: "Husbandry", parent: None, icon: "hay_block", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("consumed_item") },
    Def { id: "husbandry/place_dried_ghast_in_water", title: "Stay Hydrated!", parent: Some("husbandry/root"), icon: "dried_ghast", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("placed_dried_ghast_in_water") },
    Def { id: "husbandry/safely_harvest_honey", title: "Bee Our Guest", parent: Some("husbandry/root"), icon: "honey_bottle", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("safely_harvest_honey") },
    Def { id: "husbandry/breed_an_animal", title: "The Parrots and the Bats", parent: Some("husbandry/root"), icon: "wheat", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("bred") },
    Def { id: "husbandry/allay_deliver_item_to_player", title: "You've Got a Friend in Me", parent: Some("husbandry/root"), icon: "cookie", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("allay_deliver_item_to_player") },
    Def { id: "husbandry/ride_a_boat_with_a_goat", title: "Whatever Floats Your Goat!", parent: Some("husbandry/root"), icon: "oak_boat", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("ride_a_boat_with_a_goat") },
    Def { id: "husbandry/tame_an_animal", title: "Best Friends Forever", parent: Some("husbandry/root"), icon: "lead", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("tame_animal") },
    Def { id: "husbandry/make_a_sign_glow", title: "Glow and Behold!", parent: Some("husbandry/root"), icon: "glow_ink_sac", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("make_a_sign_glow") },
    Def { id: "husbandry/fishy_business", title: "Fishy Business", parent: Some("husbandry/root"), icon: "fishing_rod", frame: 0, hidden: false, xp: 0, criteria: Criteria::Any(&["cod", "salmon", "tropical_fish", "pufferfish"]) },
    Def { id: "husbandry/silk_touch_nest", title: "Total Beelocation", parent: Some("husbandry/root"), icon: "bee_nest", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("silk_touch_nest") },
    Def { id: "husbandry/tadpole_in_a_bucket", title: "Bukkit Bukkit", parent: Some("husbandry/root"), icon: "tadpole_bucket", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("tadpole_in_a_bucket") },
    Def { id: "husbandry/obtain_sniffer_egg", title: "Smells Interesting", parent: Some("husbandry/root"), icon: "sniffer_egg", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("sniffer_egg") },
    Def { id: "husbandry/plant_seed", title: "A Seedy Place", parent: Some("husbandry/root"), icon: "wheat", frame: 0, hidden: false, xp: 0, criteria: Criteria::Any(&["wheat", "pumpkin_stem", "melon_stem", "beetroots", "nether_wart", "torchflower", "pitcher_pod"]) },
    Def { id: "husbandry/wax_on", title: "Wax On", parent: Some("husbandry/safely_harvest_honey"), icon: "honeycomb", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("wax_on") },
    Def { id: "husbandry/bred_all_animals", title: "Two by Two", parent: Some("husbandry/breed_an_animal"), icon: "golden_carrot", frame: 1, hidden: false, xp: 100, criteria: Criteria::All(&ANIMALS) },
    Def { id: "husbandry/allay_deliver_cake_to_note_block", title: "Birthday Song", parent: Some("husbandry/allay_deliver_item_to_player"), icon: "note_block", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("allay_deliver_cake_to_note_block") },
    Def { id: "husbandry/complete_catalogue", title: "A Complete Catalogue", parent: Some("husbandry/tame_an_animal"), icon: "cod", frame: 1, hidden: false, xp: 50, criteria: Criteria::All(&CATS) },
    Def { id: "husbandry/tactical_fishing", title: "Tactical Fishing", parent: Some("husbandry/fishy_business"), icon: "pufferfish_bucket", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("pufferfish_bucket") },
    Def { id: "husbandry/leash_all_frog_variants", title: "When the Squad Hops into Town", parent: Some("husbandry/tadpole_in_a_bucket"), icon: "lead", frame: 0, hidden: false, xp: 0, criteria: Criteria::All(&["temperate", "warm", "cold"]) },
    Def { id: "husbandry/feed_snifflet", title: "Little Sniffs", parent: Some("husbandry/obtain_sniffer_egg"), icon: "torchflower_seeds", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("feed_snifflet") },
    Def { id: "husbandry/balanced_diet", title: "A Balanced Diet", parent: Some("husbandry/plant_seed"), icon: "apple", frame: 1, hidden: false, xp: 100, criteria: Criteria::All(&FOODS) },
    Def { id: "husbandry/obtain_netherite_hoe", title: "Serious Dedication", parent: Some("husbandry/plant_seed"), icon: "netherite_hoe", frame: 1, hidden: false, xp: 100, criteria: Criteria::One("netherite_hoe") },
    Def { id: "husbandry/wax_off", title: "Wax Off", parent: Some("husbandry/wax_on"), icon: "stone_axe", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("wax_off") },
    Def { id: "husbandry/axolotl_in_a_bucket", title: "The Cutest Predator", parent: Some("husbandry/tactical_fishing"), icon: "axolotl_bucket", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("axolotl_in_a_bucket") },
    Def { id: "husbandry/froglights", title: "With Our Powers Combined!", parent: Some("husbandry/leash_all_frog_variants"), icon: "verdant_froglight", frame: 1, hidden: false, xp: 0, criteria: Criteria::All(&["ochre_froglight", "verdant_froglight", "pearlescent_froglight"]) },
    Def { id: "husbandry/plant_any_sniffer_seed", title: "Planting the Past", parent: Some("husbandry/feed_snifflet"), icon: "pitcher_plant", frame: 0, hidden: false, xp: 0, criteria: Criteria::Any(&["torchflower", "pitcher_pod"]) },
    Def { id: "husbandry/kill_axolotl_target", title: "The Healing Power of Friendship!", parent: Some("husbandry/axolotl_in_a_bucket"), icon: "axolotl_bucket", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("killed_axolotl_target") },
    Def { id: "husbandry/repair_wolf_armor", title: "Good as New", parent: Some("husbandry/tame_an_animal"), icon: "wolf_armor", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("repair_wolf_armor") },
    Def { id: "husbandry/whole_pack", title: "The Whole Pack", parent: Some("husbandry/tame_an_animal"), icon: "bone", frame: 1, hidden: false, xp: 50, criteria: Criteria::All(&WOLVES) },
    Def { id: "husbandry/remove_wolf_armor", title: "Shear Brilliance", parent: Some("husbandry/tame_an_animal"), icon: "shears", frame: 0, hidden: false, xp: 0, criteria: Criteria::One("remove_wolf_armor") },
];

pub fn def(id: &str) -> Option<&'static Def> {
    TREE.iter().find(|def| def.id == id)
}

/// Нормализует имя: `story/root` и `minecraft:story/root` — одно и то же.
pub fn find(name: &str) -> Option<&'static Def> {
    def(name.strip_prefix("minecraft:").unwrap_or(name))
}

/// Потомки достижения (не включая его) в порядке дерева.
pub fn descendants(id: &str) -> Vec<&'static Def> {
    let mut out = Vec::new();
    let mut stack = vec![id];

    while let Some(at) = stack.pop() {
        for child in TREE.iter().filter(|child| child.parent == Some(at)) {
            out.push(child);
            stack.push(child.id);
        }
    }

    out.sort_by_key(|def| TREE.iter().position(|other| other.id == def.id));
    out
}

/// Предки достижения (не включая его), от корня.
pub fn ancestors(id: &str) -> Vec<&'static Def> {
    let mut out = Vec::new();
    let mut at = def(id).and_then(|def| def.parent);

    while let Some(parent) = at.and_then(self::def) {
        out.push(parent);
        at = parent.parent;
    }

    out.reverse();
    out
}

/// Расклад по экрану: x — глубина, y — по листьям, родитель посередине
/// между первым и последним ребёнком.
fn layout() -> &'static HashMap<&'static str, (f32, f32)> {
    static LAYOUT: OnceLock<HashMap<&'static str, (f32, f32)>> = OnceLock::new();

    LAYOUT.get_or_init(|| {
        fn place(def: &'static Def, depth: f32, next_y: &mut f32, out: &mut HashMap<&'static str, (f32, f32)>) -> f32 {
            let children: Vec<&'static Def> = TREE.iter().filter(|child| child.parent == Some(def.id)).collect();
            let y = if children.is_empty() {
                let y = *next_y;
                *next_y += 1.0;
                y
            } else {
                let ys: Vec<f32> = children.iter().map(|child| place(child, depth + 1.0, next_y, out)).collect();
                (ys[0] + ys[ys.len() - 1]) / 2.0
            };

            out.insert(def.id, (depth, y));
            y
        }

        let mut out = HashMap::new();

        for root in TREE.iter().filter(|def| def.is_root()) {
            let mut next_y = 0.0;
            place(root, 0.0, &mut next_y, &mut out);
        }

        out
    })
}

/// Время в виде файла оригинала: `2026-09-20 20:37:08 +0000`.
fn format_time(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let days = secs.div_euclid(86400);
    let rest = secs.rem_euclid(86400);
    // Гражданская дата по числу дней от 1970-01-01.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { year + 1 } else { year };

    format!("{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02} +0000", rest / 3600, rest % 3600 / 60, rest % 60)
}

fn parse_time(text: &str) -> Option<i64> {
    let (date, rest) = text.split_once(' ')?;
    let (clock, zone) = rest.split_once(' ')?;
    let mut d = date.split('-').map(|part| part.parse::<i64>().ok());
    let (y, m, day) = (d.next()??, d.next()??, d.next()??);
    let mut c = clock.split(':').map(|part| part.parse::<i64>().ok());
    let (h, mi, s) = (c.next()??, c.next()??, c.next()??);
    let sign = if zone.starts_with('-') { -1 } else { 1 };
    let offset = sign * (zone.get(1..3)?.parse::<i64>().ok()? * 3600 + zone.get(3..5)?.parse::<i64>().ok()? * 60);
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;

    Some((days * 86400 + h * 3600 + mi * 60 + s - offset) * 1000)
}

fn now_ms() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|since| since.as_millis() as i64).unwrap_or(0)
}

/// Что игрок прошёл: по достижению — условия с временем выполнения.
#[derive(Default, Debug, Clone)]
pub struct AdvState {
    progress: BTreeMap<String, BTreeMap<String, i64>>,
    /// Что клиент уже знает (показано).
    shown: HashSet<&'static str>,
    /// Чей прогресс изменился с последней рассылки.
    changed: BTreeSet<&'static str>,
    /// Завершённые с последней проверки — для объявления и награды.
    completed: Vec<&'static Def>,
    dirty: bool,
    /// Клиент хоть раз получал полный пакет.
    started: bool,
    /// Вкладка, которую игрок открыл последней.
    pub tab: Option<String>,
}

impl AdvState {
    pub fn new() -> Self {
        AdvState::default()
    }

    pub fn has(&self, id: &str, criterion: &str) -> bool {
        self.progress.get(id).is_some_and(|got| got.contains_key(criterion))
    }

    pub fn is_done(&self, id: &str) -> bool {
        let Some(def) = def(id) else {
            return false;
        };

        self.done_by(def, self.progress.get(id))
    }

    fn done_by(&self, def: &Def, got: Option<&BTreeMap<String, i64>>) -> bool {
        let Some(got) = got else {
            return false;
        };

        def.criteria.requirements().iter().all(|group| group.iter().any(|name| got.contains_key(name)))
    }

    /// Выдать условие. Верно, если оно только что выдано. Завершение
    /// записывается для объявления.
    pub fn grant(&mut self, id: &str, criterion: &str) -> bool {
        let Some(def) = def(id) else {
            return false;
        };

        if !def.criteria.names().iter().any(|name| name == criterion) || self.has(id, criterion) {
            return false;
        }

        let was = self.is_done(id);

        self.progress.entry(id.to_string()).or_default().insert(criterion.to_string(), now_ms());
        self.changed.insert(def.id);
        self.dirty = true;

        if !was && self.is_done(id) {
            self.completed.push(def);
        }

        true
    }

    /// Выдать все недостающие условия.
    pub fn grant_all(&mut self, id: &str) -> bool {
        let Some(def) = def(id) else {
            return false;
        };
        let mut any = false;

        for name in def.criteria.names() {
            any |= self.grant(id, &name);
        }

        any
    }

    pub fn revoke(&mut self, id: &str, criterion: &str) -> bool {
        let Some(def) = def(id) else {
            return false;
        };
        let Some(got) = self.progress.get_mut(id) else {
            return false;
        };

        if got.remove(criterion).is_none() {
            return false;
        }

        if got.is_empty() {
            self.progress.remove(id);
        }

        self.changed.insert(def.id);
        self.dirty = true;
        true
    }

    pub fn revoke_all(&mut self, id: &str) -> bool {
        let Some(def) = def(id) else {
            return false;
        };
        let any = self.progress.remove(id).is_some();

        if any {
            self.changed.insert(def.id);
            self.dirty = true;
        }

        any
    }

    /// Достижения, завершённые с прошлого вызова.
    pub fn take_completed(&mut self) -> Vec<&'static Def> {
        std::mem::take(&mut self.completed)
    }

    /// Видно ли достижение клиенту: оно выполнено, либо выполнен родитель,
    /// либо дед (но корень — только выполненный) — как у оригинала [проверить].
    pub fn visible(&self, def: &Def) -> bool {
        let mut at = Some(def);

        for level in 0..=2 {
            let Some(node) = at else {
                return false;
            };

            if self.is_done(node.id) {
                return true;
            }

            let _ = level;
            at = node.parent.and_then(self::def);
        }

        false
    }

    /// Тело Update Advancements. `reset` — полный пакет (вход); без него —
    /// только разница: новые видимые, пропавшие и изменённый прогресс.
    pub fn update_body(&mut self, reset: bool) -> Option<Vec<u8>> {
        let visible: Vec<&'static Def> = TREE.iter().filter(|def| self.visible(def)).collect();
        let visible_ids: HashSet<&'static str> = visible.iter().map(|def| def.id).collect();
        let added: Vec<&'static Def> = visible.iter().copied().filter(|def| reset || !self.shown.contains(def.id)).collect();
        let removed: Vec<&'static str> = if reset { Vec::new() } else { self.shown.iter().copied().filter(|id| !visible_ids.contains(id)).collect() };
        let touched: Vec<&'static Def> = visible.iter().copied().filter(|def| reset || self.changed.contains(def.id) || !self.shown.contains(def.id)).collect();

        self.changed.clear();

        if !reset && added.is_empty() && removed.is_empty() && touched.is_empty() {
            return None;
        }

        let mut body = encode_varint(UPDATE_ADVANCEMENTS);
        body.push(reset as u8);
        body.extend(encode_varint(added.len() as i32));

        for def in &added {
            body.extend(encode_string(&format!("minecraft:{}", def.id)));
            body.extend(advancement_bytes(def));
        }

        body.extend(encode_varint(removed.len() as i32));

        for id in &removed {
            body.extend(encode_string(&format!("minecraft:{id}")));
        }

        body.extend(encode_varint(touched.len() as i32));

        for def in &touched {
            body.extend(encode_string(&format!("minecraft:{}", def.id)));
            let got = self.progress.get(def.id);
            let names = def.criteria.names();

            body.extend(encode_varint(names.len() as i32));

            for name in names {
                body.extend(encode_string(&name));

                match got.and_then(|got| got.get(&name)) {
                    Some(at) => {
                        body.push(1);
                        body.extend_from_slice(&at.to_be_bytes());
                    }
                    None => body.push(0),
                }
            }
        }

        // «Показывать достижения»: пакет входа — да.
        body.push(1);

        self.shown = visible_ids;
        self.started = true;
        Some(body)
    }

    pub fn started(&self) -> bool {
        self.started
    }

    pub fn to_json(&self) -> String {
        let mut root = serde_json::Map::new();

        for (id, got) in &self.progress {
            let criteria: serde_json::Map<String, Value> = got.iter().map(|(name, at)| (name.clone(), Value::String(format_time(*at)))).collect();

            root.insert(format!("minecraft:{id}"), json!({"criteria": criteria, "done": self.is_done(id)}));
        }

        root.insert("DataVersion".into(), Value::from(crate::stats::DATA_VERSION));

        serde_json::to_string_pretty(&Value::Object(root)).unwrap_or_default()
    }

    pub fn from_json(text: &str) -> Option<AdvState> {
        let root: Value = serde_json::from_str(text).ok()?;
        let mut state = AdvState::new();

        for (key, value) in root.as_object()? {
            let Some(id) = key.strip_prefix("minecraft:") else {
                continue;
            };
            let Some(def) = def(id) else {
                continue;
            };
            let Some(criteria) = value.get("criteria").and_then(Value::as_object) else {
                continue;
            };
            let names = def.criteria.names();
            let got: BTreeMap<String, i64> = criteria
                .iter()
                .filter(|(name, _)| names.contains(name))
                .filter_map(|(name, at)| Some((name.clone(), parse_time(at.as_str()?).unwrap_or(0))))
                .collect();

            if !got.is_empty() {
                state.progress.insert(id.to_string(), got);
            }
        }

        Some(state)
    }

    pub fn path_for(uuid: &[u8; 16]) -> PathBuf {
        Path::new(DIRECTORY).join(format!("{}.json", crate::network::login::format_uuid(uuid)))
    }

    pub fn load(uuid: &[u8; 16]) -> AdvState {
        std::fs::read_to_string(AdvState::path_for(uuid)).ok().and_then(|text| AdvState::from_json(&text)).unwrap_or_default()
    }

    pub fn save(&mut self, uuid: &[u8; 16]) {
        if !self.dirty {
            return;
        }

        let path = AdvState::path_for(uuid);

        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let tmp = path.with_extension("json.tmp");

        if std::fs::write(&tmp, self.to_json()).is_ok() && std::fs::rename(&tmp, &path).is_ok() {
            self.dirty = false;
        }
    }
}

/// Достижение для пакета: родитель, показ, требования, телеметрия.
fn advancement_bytes(def: &Def) -> Vec<u8> {
    let mut out = Vec::new();

    match def.parent {
        Some(parent) => {
            out.push(1);
            out.extend(encode_string(&format!("minecraft:{parent}")));
        }
        None => out.push(0),
    }

    // Показ.
    out.push(1);
    out.extend(crate::scoreboard::encode_component(&def.title()));
    out.extend(crate::scoreboard::encode_component(&def.description()));

    // Значок: предмет, число, правки компонентов (нет).
    out.extend(encode_varint(crate::blocks::item_named(def.icon).unwrap_or(0)));
    out.extend(encode_varint(1));
    out.extend(encode_varint(0));
    out.extend(encode_varint(0));

    out.extend(encode_varint(def.frame as i32));

    let background = def.background();
    // Биты: 1 — фон, 2 — всплывающее окно, 4 — скрытое. Корни без окна.
    let flags = background.is_some() as i32 | ((!def.is_root()) as i32) << 1 | (def.hidden as i32) << 2;

    out.extend_from_slice(&flags.to_be_bytes());

    if let Some(background) = background {
        out.extend(encode_string(background));
    }

    let (x, y) = layout().get(def.id).copied().unwrap_or((0.0, 0.0));

    out.extend_from_slice(&x.to_be_bytes());
    out.extend_from_slice(&y.to_be_bytes());

    // Требования.
    let requirements = def.criteria.requirements();

    out.extend(encode_varint(requirements.len() as i32));

    for group in requirements {
        out.extend(encode_varint(group.len() as i32));

        for name in group {
            out.extend(encode_string(&name));
        }
    }

    // Телеметрия: только у пространства minecraft.
    out.push(1);
    out
}

/// Объявление в чате: «%s has made the advancement [..]».
pub fn announcement(player: &str, def: &Def) -> Value {
    let kind = match def.frame {
        1 => "challenge",
        2 => "goal",
        _ => "task",
    };
    let color = if def.frame == 1 { "dark_purple" } else { "green" };
    let hover = json!({"color": color, "translate": format!("{}.title", def.key()), "extra": ["\n", {"translate": format!("{}.description", def.key())}]});
    let link = json!({"color": color, "translate": "chat.square_brackets", "with": [
        {"translate": format!("{}.title", def.key()), "hover_event": {"action": "show_text", "value": hover}}
    ]});

    json!({"translate": format!("chat.type.advancement.{kind}"), "with": [player, link]})
}

/// Предметы, чьё появление в инвентаре выдаёт условия: `(достижение, условие, предметы)`.
pub const HAS_ITEM: &[(&str, &str, &[&str])] = &[
    ("story/root", "crafting_table", &["crafting_table"]),
    ("story/mine_stone", "get_stone", &["cobblestone", "blackstone", "cobbled_deepslate"]),
    ("story/upgrade_tools", "stone_pickaxe", &["stone_pickaxe"]),
    ("story/smelt_iron", "iron", &["iron_ingot"]),
    ("story/obtain_armor", "iron_helmet", &["iron_helmet"]),
    ("story/obtain_armor", "iron_chestplate", &["iron_chestplate"]),
    ("story/obtain_armor", "iron_leggings", &["iron_leggings"]),
    ("story/obtain_armor", "iron_boots", &["iron_boots"]),
    ("story/lava_bucket", "lava_bucket", &["lava_bucket"]),
    ("story/iron_tools", "iron_pickaxe", &["iron_pickaxe"]),
    ("story/form_obsidian", "obsidian", &["obsidian"]),
    ("story/mine_diamond", "diamond", &["diamond"]),
    ("story/shiny_gear", "diamond_helmet", &["diamond_helmet"]),
    ("story/shiny_gear", "diamond_chestplate", &["diamond_chestplate"]),
    ("story/shiny_gear", "diamond_leggings", &["diamond_leggings"]),
    ("story/shiny_gear", "diamond_boots", &["diamond_boots"]),
    ("nether/obtain_ancient_debris", "ancient_debris", &["ancient_debris"]),
    ("nether/obtain_crying_obsidian", "crying_obsidian", &["crying_obsidian"]),
    ("nether/obtain_blaze_rod", "blaze_rod", &["blaze_rod"]),
    ("nether/netherite_armor", "netherite_helmet", &["netherite_helmet"]),
    ("nether/netherite_armor", "netherite_chestplate", &["netherite_chestplate"]),
    ("nether/netherite_armor", "netherite_leggings", &["netherite_leggings"]),
    ("nether/netherite_armor", "netherite_boots", &["netherite_boots"]),
    ("end/dragon_egg", "dragon_egg", &["dragon_egg"]),
    ("end/elytra", "elytra", &["elytra"]),
    ("husbandry/obtain_netherite_hoe", "netherite_hoe", &["netherite_hoe"]),
    ("husbandry/obtain_sniffer_egg", "sniffer_egg", &["sniffer_egg"]),
    ("nether/get_wither_skull", "wither_skull", &["wither_skeleton_skull"]),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_is_consistent() {
        let mut ids = HashSet::new();

        for def in TREE {
            assert!(ids.insert(def.id), "повтор {}", def.id);
            assert!(def.parent.is_none_or(|parent| self::def(parent).is_some()), "нет родителя {}", def.id);
            assert!(crate::blocks::item_named(def.icon).is_some(), "нет значка {} у {}", def.icon, def.id);
        }

        assert_eq!(TREE.iter().filter(|def| def.is_root()).count(), 5);
    }

    #[test]
    fn items_in_has_item_exist_and_match_criteria() {
        for (id, criterion, items) in HAS_ITEM {
            let def = def(id).unwrap_or_else(|| panic!("{id}"));

            assert!(def.criteria.names().iter().any(|name| name == criterion), "{id}/{criterion}");

            for item in *items {
                assert!(crate::blocks::item_named(item).is_some(), "{item}");
            }
        }
    }

    #[test]
    fn grant_completes_and_reveals_children() {
        let mut state = AdvState::new();

        // Корень скрыт, пока не выполнен.
        assert!(!state.visible(def("story/root").unwrap()));
        assert!(state.grant("story/root", "crafting_table"));
        assert!(!state.grant("story/root", "crafting_table"));
        assert!(state.is_done("story/root"));
        assert!(state.visible(def("story/mine_stone").unwrap()));
        assert!(state.visible(def("story/upgrade_tools").unwrap()));
        assert!(!state.visible(def("story/smelt_iron").unwrap()));
        assert_eq!(state.take_completed().len(), 1);
    }

    #[test]
    fn any_and_all_requirements() {
        let mut state = AdvState::new();

        state.grant("story/obtain_armor", "iron_boots");
        assert!(state.is_done("story/obtain_armor"));

        state.grant("nether/netherite_armor", "netherite_boots");
        assert!(!state.is_done("nether/netherite_armor"));

        for piece in ["netherite_helmet", "netherite_chestplate", "netherite_leggings"] {
            state.grant("nether/netherite_armor", piece);
        }

        assert!(state.is_done("nether/netherite_armor"));
    }

    #[test]
    fn json_roundtrip_matches_original_shape() {
        let mut state = AdvState::new();

        state.grant("story/root", "crafting_table");

        let text = state.to_json();
        let value: Value = serde_json::from_str(&text).unwrap();

        assert_eq!(value["minecraft:story/root"]["done"], true);
        assert!(value["minecraft:story/root"]["criteria"]["crafting_table"].as_str().unwrap().ends_with("+0000"));
        assert_eq!(value["DataVersion"], crate::stats::DATA_VERSION);

        let back = AdvState::from_json(&text).unwrap();

        assert!(back.is_done("story/root"));
    }

    #[test]
    fn time_roundtrip() {
        let ms = 1_790_000_000_000;

        assert_eq!(parse_time(&format_time(ms)), Some(ms));
        assert_eq!(format_time(0), "1970-01-01 00:00:00 +0000");
        assert_eq!(parse_time("2026-09-20 20:37:08 +0300"), Some(parse_time("2026-09-20 17:37:08 +0000").unwrap()));
    }

    #[test]
    fn update_sends_full_then_changes() {
        let mut state = AdvState::new();

        // Первый пакет — полный, даже если видимого нет.
        let first = state.update_body(true).unwrap();
        assert_eq!(first[0..3], [0x82, 0x01, 0x01]);
        assert!(state.update_body(false).is_none());

        state.grant("story/root", "crafting_table");
        let next = state.update_body(false).unwrap();

        // Не сброс; видны корень и два его потомка.
        assert_eq!(next[2], 0);
        assert_eq!(next[3], 3);
    }
}
