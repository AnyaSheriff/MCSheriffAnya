// Книга рецептов (вики, Recipe book, «Unlocking»). Получение предметов
// открывает рецепты; список и настройки сохраняются отдельно от инвентаря.
// Пакеты — разрешённая таблица minecraft-data pc/26.1/protocol.json.
// [проверить] для многокомпонентных рецептов без правила в таблице вики
// используем наличие всех видов ингредиентов. Специальные рецепты,
// которых нет в crafting::table, здесь не выдумываются.
use std::collections::BTreeSet;
use std::sync::OnceLock;
use crate::{blocks, crafting, furnace};
use crate::containers::Kind;
use crate::network::varint::encode_varint;

pub const SETTINGS: i32 = 0x4c;
pub const ADD: i32 = 0x4a;
pub const REMOVE: i32 = 0x4b;
pub const CLIENT_SETTINGS: i32 = 0x2e;
// OptVarInt: локальная группа0 (raw1); official raw22 не является стабильным ID.
const CONCRETE_SLAB_GROUP: u8 = 1;
const WOOL_SLAB_GROUP: u8 = 2;
const CONCRETE_STAIRS_GROUP: u8 = 3;
const WOOL_STAIRS_GROUP: u8 = 4;
const MINERAL_GROUP: u8 = 5;
#[cfg(test)]
const WHITE_SLAB_CUT_KEY: &str = "minecraft:white_concrete_slab_from_white_concrete_stonecutting";

#[derive(Default)]
pub struct Book {
    catalog: Option<crate::gameplay_catalog::Catalog>,
    known: BTreeSet<String>,
    sent: BTreeSet<String>,
    pub settings: [bool; 8],
    started: bool,
    dirty: bool,
    /// Отпечаток инвентаря при последнем просмотре: рецепты открывает перемена
    /// в инвентаре, а не каждый такт (вики, Recipe: «unlocked ... by obtaining an ingredient»).
    signature: Option<u64>,
    /// Рецепт верстака выдан при первом входе (вики: он открыт сразу; `recipe take` его не возвращает).
    granted: bool,
    /// Следующая рассылка — одним пакетом (Knowledge Book, `recipe give`); иначе по пакету на рецепт
    /// (чёрный ящик, craft-recipe-unlock: награды достижений приходят по одному).
    batch: bool,
}

#[derive(Clone)]
enum Display {
    Crafting(&'static crafting::Recipe),
    Furnace { kind: Kind, input: i32, result: i32, xp: f32 },
    Stonecutting { input: i32, result: i32, count: i32 },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Slab { Concrete(usize), Wool(usize), ConcreteStairs(usize), WoolStairs(usize), Mineral(usize) }
impl Slab {
    fn input_name(self) -> &'static str {
        match self {
            Self::Concrete(index) => crate::gameplay_catalog::CONCRETE_SLABS[index].input_name,
            Self::Wool(index) => crate::gameplay_catalog::WOOL_SLABS[index].input_name,
            Self::ConcreteStairs(index) => crate::gameplay_catalog::CONCRETE_STAIRS[index].input_name,
            Self::WoolStairs(index) => crate::gameplay_catalog::WOOL_STAIRS[index].input_name,
            Self::Mineral(index) => crate::gameplay_catalog::MINERALS[index].input_name,
        }
    }
    fn group(self) -> u8 {
        match self {
            Self::Concrete(_) => CONCRETE_SLAB_GROUP, Self::Wool(_) => WOOL_SLAB_GROUP,
            Self::ConcreteStairs(_) => CONCRETE_STAIRS_GROUP, Self::WoolStairs(_) => WOOL_STAIRS_GROUP,
            Self::Mineral(_) => MINERAL_GROUP,
        }
    }
}
#[derive(Clone)]
struct Entry { key: String, display: Display, recipe_id: Option<crafting::RecipeId>, slab: Option<Slab> }

impl Entry {
    fn named(&self, catalog: crate::gameplay_catalog::Catalog, name: &str) -> bool {
        // Только имя, подтверждённое обычными recipe take/give777; не имя результата.
        if self.slab.is_some() && matches!(self.display, Display::Stonecutting { .. }) { return self.key.strip_prefix("minecraft:") == Some(name); }
        matches!(&self.display, Display::Crafting(recipe) if catalog.name(crate::gameplay_catalog::Kind::Item, recipe.result).ok() == Some(name))
    }
}

fn entries() -> &'static [Entry] {
    static ENTRIES: OnceLock<Vec<Entry>> = OnceLock::new();
    ENTRIES.get_or_init(|| {
        let mut entries: Vec<Entry> = crafting::recipes().iter().enumerate().map(|(index, recipe)| Entry {
            key: format!("crafting/{}/{index}", blocks::item_name(recipe.result).unwrap_or("unknown")),
            display: Display::Crafting(recipe), recipe_id: Some(crafting::RecipeId::Native(index)), slab: None,
        }).collect();
        for (kind, key) in [(Kind::Furnace, "smelting"), (Kind::BlastFurnace, "blasting"), (Kind::Smoker, "smoking")] {
            for name in blocks::item_names() {
                let input = blocks::item_named(name).unwrap();
                if let Some((result, xp)) = furnace::smelt(kind, input) {
                    entries.push(Entry { key: format!("{key}/{name}"), recipe_id: None, slab: None, display: Display::Furnace { kind, input, result, xp } });
                }
            }
        }
        for cut in crate::stonecutter::recipes() {
            let (input, result) = (blocks::item_name(cut.input).unwrap_or("unknown"), blocks::item_name(cut.output).unwrap_or("unknown"));
            entries.push(Entry { key: format!("stonecutting/{result}_from_{input}"), recipe_id: None, slab: None, display: Display::Stonecutting { input: cut.input, result: cut.output, count: cut.count } });
        }
        entries
    })
}

/// Первая позиция ступеней в расширенном списке: после прежних записей, пар бетонных плит и шерстяных плит.
fn stairs_start() -> usize {
    entries().len() + 2 * crate::gameplay_catalog::CONCRETE_SLABS.len() + crate::gameplay_catalog::WOOL_SLABS.len()
}

fn entries_for_catalog(catalog: crate::gameplay_catalog::Catalog) -> &'static [Entry] {
    if catalog.profile() == crate::gameplay_catalog::Profile::Java26_1_2 { return entries(); }
    static PREVIEW: OnceLock<Vec<Entry>> = OnceLock::new();
    PREVIEW.get_or_init(|| {
        let mut extended = entries().to_vec();
        for (index, row) in crate::gameplay_catalog::CONCRETE_SLABS.iter().enumerate() {
            extended.push(Entry {
                key: format!("minecraft:{}", row.name),
                display: Display::Crafting(crafting::recipe_for_catalog(catalog, crafting::RecipeId::concrete_slab(index).expect("цвет рецепта")).expect("рецепт профиля")),
                recipe_id: crafting::RecipeId::concrete_slab(index), slab: Some(Slab::Concrete(index)),
            });
            extended.push(Entry {
                key: format!("minecraft:{}_from_{}_stonecutting", row.name, row.input_name),
                display: Display::Stonecutting { input: row.input_item, result: row.item, count: 2 },
                recipe_id: None, slab: Some(Slab::Concrete(index)),
            });
        }
        // Дополняем после прежних shaped/cut пар: их сетевые позиции и ключи неизменны.
        for (index, row) in crate::gameplay_catalog::WOOL_SLABS.iter().enumerate() {
            let id = crafting::RecipeId::WoolSlab(index as u8);
            extended.push(Entry {
                key: format!("minecraft:{}", row.name),
                display: Display::Crafting(crafting::recipe_for_catalog(catalog, id).expect("рецепт профиля")),
                recipe_id: Some(id), slab: Some(Slab::Wool(index)),
            });
        }
        // Ступени 26.3 дописаны после плит: бетон — пара shaped+камнерез (1→1), шерсть — только shaped.
        for (index, row) in crate::gameplay_catalog::CONCRETE_STAIRS.iter().enumerate() {
            let id = crafting::RecipeId::ConcreteStairs(index as u8);
            extended.push(Entry {
                key: format!("minecraft:{}", row.name),
                display: Display::Crafting(crafting::recipe_for_catalog(catalog, id).expect("рецепт профиля")),
                recipe_id: Some(id), slab: Some(Slab::ConcreteStairs(index)),
            });
            extended.push(Entry {
                key: format!("minecraft:{}_from_{}_stonecutting", row.name, row.input_name),
                display: Display::Stonecutting { input: row.input_item, result: row.item, count: 1 },
                recipe_id: None, slab: Some(Slab::ConcreteStairs(index)),
            });
        }
        for (index, row) in crate::gameplay_catalog::WOOL_STAIRS.iter().enumerate() {
            let id = crafting::RecipeId::WoolStairs(index as u8);
            extended.push(Entry {
                key: format!("minecraft:{}", row.name),
                display: Display::Crafting(crafting::recipe_for_catalog(catalog, id).expect("рецепт профиля")),
                recipe_id: Some(id), slab: Some(Slab::WoolStairs(index)),
            });
        }
        // Сера и киноварь: сперва рецепты верстака (по строкам таблицы), затем камнерез.
        for (row_index, recipe) in crafting::mineral_recipes() {
            let row = &crate::gameplay_catalog::MINERALS[usize::from(*row_index)];
            extended.push(Entry {
                key: format!("minecraft:{}", row.name),
                display: Display::Crafting(recipe),
                recipe_id: Some(crafting::RecipeId::Mineral(*row_index)), slab: Some(Slab::Mineral(usize::from(*row_index))),
            });
        }
        for cut in crate::stonecutter::mineral_recipes() {
            let find = |item: i32| crate::gameplay_catalog::MINERALS.iter().position(|row| row.item == item).expect("строка серы");
            let (input, output) = (find(cut.input), find(cut.output));
            extended.push(Entry {
                key: format!("minecraft:{}_from_{}_stonecutting", crate::gameplay_catalog::MINERALS[output].name, crate::gameplay_catalog::MINERALS[input].name),
                display: Display::Stonecutting { input: cut.input, result: cut.output, count: cut.count },
                recipe_id: None, slab: Some(Slab::Mineral(output)),
            });
        }
        extended
    })
}

/// Вики: обычно нужен один указанный ингредиент, а не целый рецепт.
fn unlock(recipe: &crafting::Recipe, names: &[&str], occupied: usize, water: bool) -> bool {
    let result = blocks::item_name(recipe.result).unwrap_or("");
    let has = |name: &str| names.contains(&name);
    let any = |items: &[&str]| items.iter().any(|item| has(item));
    let planks = || names.iter().any(|n| n.ends_with("_planks"));
    let tools = ["axe", "hoe", "pickaxe", "shovel", "sword", "helmet", "chestplate", "leggings", "boots", "spear"];
    if result == "crafting_table" { return false; }
    if result == "chest" { return occupied >= 10; }
    if (result.ends_with("_boat") || result == "bamboo_raft") && !result.contains("chest") { return water; }
    for (prefix, ingredient) in [("wooden", "stick"), ("iron", "iron_ingot"), ("golden", "gold_ingot"), ("diamond", "diamond"), ("copper", "copper_ingot")] {
        if result.strip_prefix(prefix).and_then(|s| s.strip_prefix('_')).is_some_and(|s| tools.contains(&s)) { return has(ingredient); }
    }
    if result.strip_prefix("stone_").is_some_and(|s| tools.contains(&s)) || result == "furnace" { return any(&["cobblestone", "blackstone", "cobbled_deepslate"]); }
    let ingredients: Vec<_> = recipe.cells.iter().flat_map(|cell| cell.iter()).filter_map(|id| blocks::item_name(*id)).collect();
    if result.ends_with("_dye") { return ingredients.iter().any(|item| has(item)); }
    if result.ends_with("_stained_glass") { return has("glass"); }
    if result.ends_with("_terracotta") { return has("terracotta"); }
    // Перекраска шерсти, кроватей и ковров открывается нужным красителем (чёрный ящик, craft-recipe-unlock:
    // от белой шерсти открылись кровать, ковёр, баннер и картина, но не цветная шерсть).
    if (result.ends_with("_wool") || result.ends_with("_bed") || result.ends_with("_carpet")) && ingredients.iter().any(|n| n.ends_with("_dye")) {
        return ingredients.iter().filter(|n| n.ends_with("_dye")).any(|dye| has(dye));
    }
    if result.ends_with("_bundle") { return has("bundle") || names.iter().any(|name| name.ends_with("_dye")); }
    if result.ends_with("_concrete_powder") { return any(&["sand", "gravel"]); }
    if result.ends_with("_candle") || result.ends_with("_carpet") || result.ends_with("_stained_glass_pane") {
        return ingredients.iter().any(|item| has(item));
    }
    if result.ends_with("_bed") || result.ends_with("_banner") {
        return ingredients.iter().filter(|item| item.ends_with("_wool")).any(|item| has(item));
    }
    if result.ends_with("_fence") || result.ends_with("_fence_gate") || result.ends_with("_sign") {
        return ingredients.iter().filter(|item| !matches!(**item, "stick" | "chain" | "iron_chain")).any(|item| has(item));
    }
    if result.starts_with("waxed_") { return ingredients.iter().filter(|item| **item != "honeycomb").any(|item| has(item)); }
    match result {
        "iron_block" | "bucket" | "hopper" | "iron_bars" | "iron_nugget" | "minecart" | "shears" | "shield" | "smithing_table" => has("iron_ingot"),
        "gold_block" | "gold_nugget" | "golden_apple" => has("gold_ingot"),
        "diamond_block" | "jukebox" => has("diamond"),
        "redstone_block" | "clock" | "compass" | "dropper" | "note_block" | "piston" | "redstone_torch" | "target" => has("redstone"),
        "comparator" | "diorite" | "granite" | "daylight_detector" | "observer" => has("quartz"),
        "ladder" => has("stick"),
        "bow" | "fishing_rod" | "loom" | "tripwire_hook" => has("string"),
        "chest_minecart" | "furnace_minecart" | "hopper_minecart" | "tnt_minecart" | "rail" => has("minecart"),
        "amethyst_block" | "spyglass" | "tinted_glass" => has("amethyst_shard"),
        "ender_eye" | "fire_charge" | "magma_cream" => has("blaze_powder"),
        "writable_book" | "bookshelf" | "lectern" => has("book"),
        "glass_bottle" | "glass_pane" => has("glass"),
        "bundle" => any(&["leather", "string"]),
        "item_frame" | "leather_helmet" | "leather_chestplate" | "leather_leggings" | "leather_boots" | "leather_horse_armor" => has("leather"),
        "glistering_melon_slice" | "melon" | "melon_seeds" => has("melon_slice"),
        "barrel" => planks() || names.iter().any(|name| name.strip_suffix("_slab").is_some_and(|wood| blocks::item_named(&format!("{wood}_planks")).is_some())),
        "stick" if recipe.cells.iter().flat_map(|cell| cell.iter()).any(|id| blocks::item_name(*id) == Some("bamboo")) => has("bamboo"),
        "stick" => planks(),
        "dark_prismarine" | "prismarine" | "prismarine_bricks" => has("prismarine_shard"),
        "activator_rail" | "detector_rail" | "powered_rail" => has("rail"),
        "slime_block" | "sticky_piston" => has("slime_ball"),
        "scaffolding" => has("bamboo"),
        "cookie" => has("cocoa_beans"),
        "lightning_rod" => has("copper_ingot"),
        "andesite" => has("diorite"),
        "end_crystal" | "ender_chest" => has("ender_eye"),
        "golden_carrot" => has("gold_nugget"),
        "spectral_arrow" => has("glowstone_dust"),
        "firework_rocket" | "tnt" => has("gunpowder"),
        "honey_block" => has("honey_bottle"),
        "beehive" => has("honeycomb"),
        "red_nether_bricks" => has("nether_wart"),
        "flower_banner_pattern" => has("oxeye_daisy"),
        "book" | "cartography_table" => has("paper"),
        "end_rod" => has("popped_chorus_fruit"),
        "bread" => has("wheat"),
        "beetroot_soup" => has("beetroot"),
        "brewing_stand" => has("blaze_rod"),
        "anvil" => has("iron_block"),
        "beacon" => has("nether_star"),
        "netherite_ingot" if ingredients.contains(&"netherite_scrap") => has("netherite_scrap"),
        "torch" => has("stone_pickaxe"),
        "grindstone" => has("stone_slab"),
        "trapped_chest" => has("tripwire_hook"),
        "warped_fungus_on_a_stick" => has("warped_fungus"),
        "cauldron" => has("water_bucket"),
        "skull_banner_pattern" => has("wither_skeleton_skull"),
        "painting" => names.iter().any(|name| name.ends_with("_wool")),
        "packed_mud" => has("mud"),
        "crafter" => has("dropper"),
        "dispenser" => has("bow"),
        "carrot_on_a_stick" => has("carrot"),
        "jack_o_lantern" => has("carved_pumpkin"),
        "lever" => has("cobblestone"),
        "map" => has("compass"),
        "rabbit_stew" => has("cooked_rabbit"),
        "creeper_banner_pattern" => has("creeper_head"),
        "respawn_anchor" => has("crying_obsidian"),
        "cake" => has("egg"),
        "mojang_banner_pattern" => has("enchanted_golden_apple"),
        "fletching_table" => has("flint"),
        "smoker" => has("furnace"),
        "redstone_lamp" => has("glowstone"),
        "coarse_dirt" => has("gravel"),
        "mossy_stone_bricks" => has("mossy_cobblestone"),
        "enchanting_table" => has("obsidian"),
        "sea_lantern" => has("prismarine_crystals"),
        "repeater" => has("redstone_torch"),
        "turtle_helmet" => has("turtle_scute"),
        "shulker_box" => has("shulker_shell"),
        "blast_furnace" => has("smooth_stone"),
        "armor_stand" => has("smooth_stone_slab"),
        "soul_lantern" => has("soul_torch"),
        "fermented_spider_eye" => has("spider_eye"),
        "stonecutter" => has("stone"),
        "lodestone" => has("iron_ingot"),
        "chain" | "iron_chain" | "lantern" => any(&["iron_ingot", "iron_nugget"]),
        "mushroom_stew" => any(&["bowl", "red_mushroom", "brown_mushroom", "mushroom_stew"]),
        "pumpkin_pie" => any(&["pumpkin", "carved_pumpkin"]),
        "campfire" => any(&["coal", "charcoal", "stick"]),
        "arrow" => any(&["feather", "flint"]),
        "flint_and_steel" => any(&["flint", "obsidian"]),
        "conduit" => any(&["heart_of_the_sea", "nautilus_shell"]),
        "candle" => any(&["honeycomb", "string"]),
        "crossbow" => any(&["iron_ingot", "string", "tripwire_hook"]),
        "bowl" => any(&["red_mushroom", "brown_mushroom", "mushroom_stew"]),
        "soul_torch" | "soul_campfire" => any(&["soul_sand", "soul_soil"]),

        _ => recipe.cells.iter().filter(|cell| !cell.is_empty()).all(|cell| cell.iter().any(|id| blocks::item_name(*id).is_some_and(has))),
    }
}

impl Book {
    pub fn for_catalog(catalog: crate::gameplay_catalog::Catalog) -> Self { Self { catalog: Some(catalog), ..Self::default() } }
    fn catalog(&self) -> crate::gameplay_catalog::Catalog {
        self.catalog.unwrap_or(crate::gameplay_catalog::Catalog::new(crate::gameplay_catalog::Profile::Java26_1_2))
    }
    fn entries(&self) -> &'static [Entry] { entries_for_catalog(self.catalog()) }
    fn entry_for_recipe(&self, id: crafting::RecipeId) -> Option<&'static Entry> {
        let position = match id {
            crafting::RecipeId::Native(index) => index,
            crafting::RecipeId::WoolSlab(index) if usize::from(index) < crate::gameplay_catalog::WOOL_SLABS.len() =>
                entries().len() + 2 * crate::gameplay_catalog::CONCRETE_SLABS.len() + usize::from(index),
            crafting::RecipeId::ConcreteStairs(index) if usize::from(index) < crate::gameplay_catalog::CONCRETE_STAIRS.len() =>
                stairs_start() + 2 * usize::from(index),
            crafting::RecipeId::WoolStairs(index) if usize::from(index) < crate::gameplay_catalog::WOOL_STAIRS.len() =>
                stairs_start() + 2 * crate::gameplay_catalog::CONCRETE_STAIRS.len() + usize::from(index),
            crafting::RecipeId::Mineral(_) => return self.entries().iter().find(|entry| entry.recipe_id == Some(id)),
            _ => entries().len() + 2 * id.slab_index()?,
        };
        let entry = self.entries().get(position)?;
        (entry.recipe_id == Some(id)).then_some(entry)
    }
    pub fn learn_recipe(&mut self, id: crafting::RecipeId) {
        if let Some(entry) = self.entry_for_recipe(id) { self.dirty |= self.known.insert(entry.key.clone()); }
    }
    pub fn knows_recipe(&self, id: crafting::RecipeId) -> bool {
        self.entry_for_recipe(id).is_some_and(|entry| self.known.contains(&entry.key))
    }
    pub fn knows_crafting(&self, id: usize) -> bool { self.knows_recipe(crafting::RecipeId::Native(id)) }
    pub fn crafting_display(&self, id: i32) -> Option<&'static crafting::Recipe> {
        let entry = self.entries().get(usize::try_from(id).ok()?)?;
        if !self.known.contains(&entry.key) { return None; }
        match entry.display { Display::Crafting(recipe) => Some(recipe), _ => None }
    }
    pub fn scan(&mut self, names: &[&str], occupied: usize, water: bool) {
        for entry in self.entries() {
            if self.known.contains(&entry.key) { continue; }
            let unlocked = match &entry.display {
                Display::Crafting(_) if entry.slab.is_some() => names.contains(&entry.slab.expect("цвет рецепта").input_name()),
                Display::Crafting(recipe) => unlock(recipe, names, occupied, water),
                Display::Furnace { input, .. } | Display::Stonecutting { input, .. } => blocks::item_name(*input).is_some_and(|name| names.contains(&name)),
            };
            if unlocked { self.known.insert(entry.key.clone()); self.dirty = true; }
        }
    }

    pub fn settings_body(&self) -> Vec<u8> {
        let mut body = encode_varint(SETTINGS);
        body.extend(self.settings.iter().map(|b| u8::from(*b)));
        body
    }

    pub fn set_settings(&mut self, book: i32, open: bool, filtering: bool) {
        if (0..4).contains(&book) {
            self.settings[book as usize * 2] = open;
            self.settings[book as usize * 2 + 1] = filtering;
            self.dirty = true;
        }
    }

    pub fn grant_initial(&mut self) {
        if !self.granted {
            self.granted = true;
            self.dirty = true;
            self.known.extend(self.entries().iter().filter(|e| e.key.starts_with("crafting/crafting_table/")).map(|e| e.key.clone()));
        }
    }

    pub fn started(&self) -> bool { self.started }

    /// Пакеты «рецепты добавлены» к отправке. Первый — со всем, что известно, с флагом замены.
    pub fn update(&mut self) -> Vec<Vec<u8>> {
        if self.started { self.grant_initial(); }
        let added: Vec<_> = self.entries().iter().enumerate().filter(|(_, e)| self.known.contains(&e.key) && !self.sent.contains(&e.key)).collect();
        if self.started && added.is_empty() { return Vec::new(); }
        let replace = !self.started;
        let groups: Vec<Vec<_>> = if replace || self.batch { vec![added] } else { added.into_iter().map(|one| vec![one]).collect() };
        self.started = true;
        self.batch = false;
        groups.into_iter().map(|group| {
            let mut body = encode_varint(ADD);
            body.extend(encode_varint(group.len() as i32));
            for (id, entry) in group { self.sent.insert(entry.key.clone()); body.extend(entry_body(id, entry, !replace)); }
            body.push(u8::from(replace));
            body
        }).collect()
    }

    fn path(uuid: &[u8; 16]) -> std::path::PathBuf {
        std::path::Path::new("world/players/recipes").join(format!("{}.json", crate::network::login::format_uuid(uuid)))
    }
    pub fn load(uuid: &[u8; 16]) -> Self {
        std::fs::read_to_string(Self::path(uuid)).ok().and_then(|text| Self::from_json(&text)).unwrap_or_default()
    }
    pub fn load_for_catalog(uuid: &[u8; 16], catalog: crate::gameplay_catalog::Catalog) -> Self {
        let loaded = Self::load(uuid);
        Self { known: loaded.known, settings: loaded.settings, granted: loaded.granted, ..Self::for_catalog(catalog) }
    }
    fn from_json(text: &str) -> Option<Self> {
        let value: serde_json::Value = serde_json::from_str(text).ok()?;
        let known = value.get("recipes")?.as_array()?.iter().filter_map(|v| v.as_str().map(str::to_owned)).collect();
        let settings = serde_json::from_value(value.get("settings").cloned().unwrap_or_default()).unwrap_or_default();
        let granted = value.get("granted").and_then(serde_json::Value::as_bool).unwrap_or(true);
        Some(Self { known, settings, granted, ..Self::default() })
    }
    fn to_json(&self) -> String {
        serde_json::json!({"recipes": self.known, "settings": self.settings, "granted": self.granted}).to_string()
    }
    pub fn save(&mut self, uuid: &[u8; 16]) {
        if !self.dirty { return; }
        let path = Self::path(uuid);
        if let Some(parent) = path.parent() { let _ = std::fs::create_dir_all(parent); }
        let tmp = path.with_extension("json.tmp");
        if std::fs::write(&tmp, self.to_json()).is_ok() && std::fs::rename(tmp, path).is_ok() { self.dirty = false; }
    }
}

fn entry_body(id: usize, entry: &Entry, announce: bool) -> Vec<u8> {
    let mut body = encode_varint(id as i32);
    match &entry.display {
        Display::Crafting(recipe) => {
            body.extend(encode_varint(i32::from(recipe.shaped)));
            if recipe.shaped {
                body.extend(encode_varint(i32::from(recipe.width)));
                body.extend(encode_varint(i32::from(recipe.height)));
            }
            body.extend(encode_varint(recipe.cells.len() as i32));
            for cell in recipe.cells {
                // Пустая клетка ступеней — слот типа 0, как в наблюдении 777.
                if entry.slab.is_some() && !cell.is_empty() { ingredient_set(&mut body, cell); }
                else { slot(&mut body, cell); }
            }
            stack(&mut body, recipe.result, recipe.count);
            slot(&mut body, &[blocks::item_named("crafting_table").unwrap()]);
            // Обычное наблюдение777: building_blocks; группа назначена локально.
            body.extend(if let Some(slab) = entry.slab { [slab.group(), 0, 1] } else { [0, 3, 1] });
            let cells: Vec<_> = recipe.cells.iter().filter(|cell| !cell.is_empty()).collect();
            body.extend(encode_varint(cells.len() as i32));
            for cell in cells {
                body.extend(encode_varint(cell.len() as i32 + 1)); // IDSet: размер + 1
                for id in *cell { body.extend(encode_varint(*id)); }
            }
        }
        Display::Furnace { kind, input, result, xp } => {
            body.extend(encode_varint(2));
            slot(&mut body, &[*input]);
            body.push(1); // любое топливо
            stack(&mut body, *result, 1);
            let (station, category) = match kind { Kind::BlastFurnace => ("blast_furnace", 8), Kind::Smoker => ("smoker", 9), _ => ("furnace", 6) };
            slot(&mut body, &[blocks::item_named(station).unwrap()]);
            body.extend(encode_varint(furnace::cook_time(*kind)));
            body.extend(xp.to_be_bytes());
            body.extend([0, category, 0]); // группа и требования отсутствуют
        }
        Display::Stonecutting { input, result, count } => {
            body.extend(encode_varint(3));
            if entry.slab.is_some() { ingredient_set(&mut body, &[*input]); }
            else { slot(&mut body, &[*input]); }
            stack(&mut body, *result, *count);
            slot(&mut body, &[blocks::item_named("stonecutter").unwrap()]);
            if entry.slab.is_some() {
                // Обычное наблюдение777: без группы, stonecutter, один IDSet требования.
                body.extend([0, 10, 1, 1, 2]); body.extend(encode_varint(*input));
            } else { body.extend([0, 10, 0]); } // прежняя таблица775
        }
    }
    body.push(if announce { 3 } else { 0 });
    body
}

fn ingredient_set(out: &mut Vec<u8>, ids: &[i32]) {
    out.push(6); out.extend(encode_varint(ids.len() as i32 + 1));
    for id in ids { out.extend(encode_varint(*id)); }
}

fn slot(out: &mut Vec<u8>, ids: &[i32]) {
    match ids {
        [] => out.push(0),
        [id] => { out.push(4); out.extend(encode_varint(*id)); }
        _ => { out.push(10); out.extend(encode_varint(ids.len() as i32)); for id in ids { slot(out, &[*id]); } }
    }
}
fn stack(out: &mut Vec<u8>, item: i32, count: i32) {
    out.push(5);
    out.extend(encode_varint(item));
    out.extend(encode_varint(count));
    out.extend([0, 0]);
}

impl Book {
    /// Инвентарь изменился с прошлого просмотра? Первый просмотр — всегда да.
    pub fn changed(&mut self, signature: u64) -> bool { self.signature.replace(signature) != Some(signature) }

    /// `recipe give <игрок> *`.
    pub fn give_all(&mut self) { self.batch = true; self.dirty |= self.entries().iter().fold(false, |any, e| self.known.insert(e.key.clone()) | any); }

    /// `recipe take <игрок> *` и `recipe take <игрок> <имя>`: пакет «убрать из книги» для
    /// уже показанных (пусто — имя не однозначно, ничего не сделано).
    pub fn take(&mut self, names: &[String]) -> Option<Vec<u8>> {
        let keys: Vec<String> = if names.is_empty() { self.entries().iter().filter(|entry| self.known.contains(&entry.key)).map(|entry| entry.key.clone()).collect() } else {
            let mut keys = Vec::new();
            for name in names {
                let name = name.strip_prefix("minecraft:").unwrap_or(name);
                let found: Vec<_> = self.entries().iter().filter(|e| e.named(self.catalog(), name)).collect();
                if found.len() != 1 { return None; }
                keys.push(found[0].key.clone());
            }
            keys
        };
        let ids: Vec<i32> = self.entries().iter().enumerate().filter(|(_, e)| keys.contains(&e.key) && self.sent.contains(&e.key)).map(|(i, _)| i as i32).collect();
        for key in &keys { self.dirty |= self.known.remove(key); self.sent.remove(key); }
        let mut body = encode_varint(REMOVE);
        body.extend(encode_varint(ids.len() as i32));
        for id in ids { body.extend(encode_varint(id)); }
        Some(body)
    }

    /// Knowledge Book: сначала разрешаем весь список, затем открываем его.
    /// Wiki, Knowledge Book: «adding it to their recipe book». Синтетические
    /// ключи текущего recipe registry не выдаём за vanilla ID: поддержаны
    /// однозначные crafting-result IDs и наблюдённое имя нового stonecut;
    /// остальные отвергаем [проверить].
    pub fn learn_named(&mut self, names: &[String]) -> bool {
        let mut keys = Vec::new();
        for name in names {
            let name = name.strip_prefix("minecraft:").unwrap_or(name);
            let candidates: Vec<_> = self.entries().iter().filter(|entry| entry.named(self.catalog(), name)).collect();
            if candidates.len() != 1 { return false; }
            keys.push(candidates[0].key.clone());
        }
        for key in keys { self.dirty |= self.known.insert(key); }
        self.batch = true;
        true
    }
}

/// Place Recipe без ингредиентов: клиент рисует призрачный рецепт.
pub fn ghost(window: i32, recipe: &crafting::Recipe) -> Vec<u8> {
    let mut body = encode_varint(0x3f); body.extend(encode_varint(window));
    body.extend(encode_varint(i32::from(recipe.shaped)));
    if recipe.shaped { body.extend(encode_varint(i32::from(recipe.width))); body.extend(encode_varint(i32::from(recipe.height))); }
    body.extend(encode_varint(recipe.cells.len() as i32));
    let slab = crafting::catalog_recipes().any(|known| std::ptr::eq(known, recipe));
    for cell in recipe.cells {
        if slab && !cell.is_empty() { ingredient_set(&mut body, cell); }
        else { slot(&mut body, cell); }
    }
    stack(&mut body, recipe.result, recipe.count); slot(&mut body, &[blocks::item_named("crafting_table").unwrap()]); body
}

#[cfg(test)]
mod tests {
    use super::*;
    fn recipe(name: &str) -> &'static crafting::Recipe { crafting::recipes().iter().find(|r| blocks::item_name(r.result) == Some(name)).unwrap() }
    #[test]
    fn unlocking_uses_the_key_ingredient_and_environment_exceptions() {
        for (result, ingredient) in [("iron_pickaxe", "iron_ingot"), ("diamond_sword", "diamond"), ("bow", "string"), ("wooden_axe", "stick"), ("torch", "stone_pickaxe"), ("beacon", "nether_star"), ("grindstone", "stone_slab"), ("trapped_chest", "tripwire_hook"), ("cauldron", "water_bucket"), ("painting", "white_wool"), ("packed_mud", "mud"), ("crafter", "dropper")] {
            assert!(unlock(recipe(result), &[ingredient], 1, false), "{result}");
            assert!(!unlock(recipe(result), &["dirt"], 1, false), "{result}");
        }
        // Верстак открывается при входе, а не предметами (craft-limited: после `recipe take` не возвращается).
        assert!(!unlock(recipe("crafting_table"), &[], 0, false));
        // Перекраска шерсти — нужным красителем, а не любой шерстью (craft-recipe-unlock).
        assert!(!unlock(recipe("lime_wool"), &["white_wool"], 1, false));
        assert!(unlock(recipe("lime_wool"), &["lime_dye"], 1, false));
        assert!(!unlock(recipe("chest"), &["oak_planks"], 9, false));
        assert!(unlock(recipe("chest"), &["dirt"], 10, false));
        assert!(!unlock(recipe("oak_boat"), &["oak_planks"], 1, false));
        assert!(unlock(recipe("oak_boat"), &[], 0, true));
    }
    #[test]
    fn unlocks_survive_reload_and_only_new_recipes_are_sent() {
        let mut book = Book::default();
        book.scan(&["iron_ingot"], 1, false);
        book.set_settings(0, true, false);
        let first = book.update();
        assert_eq!(first.len(), 1);
        assert_eq!(first[0][0], ADD as u8);
        assert_eq!(first[0].last(), Some(&1));
        // Следом — рецепт верстака отдельным пакетом без замены.
        let table = book.update();
        assert_eq!(table.len(), 1);
        assert_eq!(table[0].last(), Some(&0));
        assert!(book.update().is_empty());
        let mut loaded = Book::from_json(&book.to_json()).unwrap();
        assert_eq!(book.known, loaded.known);
        assert!(loaded.settings[0]);
        assert_eq!(loaded.update().len(), 1);
        loaded.scan(&["diamond"], 1, false);
        let added = loaded.update();
        assert!(added.len() > 1, "по пакету на рецепт");
        assert!(added.iter().all(|body| body.last() == Some(&0)));
    }
    #[test]
    fn take_removes_only_shown_recipes_and_batches_follow_the_command() {
        let mut book = Book::default();
        book.update();
        book.scan(&["cobblestone"], 1, false);
        assert!(book.update().iter().any(|body| body.len() > 1));
        let removed = book.take(&[]).unwrap();
        assert_eq!(removed[0], REMOVE as u8);
        assert!(book.take(&["minecraft:stone_pickaxe".into()]).is_some());
        assert!(book.update().is_empty(), "после take заново не открывается, пока инвентарь не изменится");
        book.give_all();
        assert_eq!(book.update().len(), 1, "give — одним пакетом");
    }
    #[test]
    fn stonecutting_recipes_unlock_with_their_input() {
        let mut book = Book::default();
        book.update();
        book.scan(&["cobblestone"], 1, false);
        let total: usize = book.update().iter().map(|b| b.len()).sum();
        assert!(total > 0);
        assert!(entries().iter().any(|e| matches!(e.display, Display::Stonecutting { .. }) && book.known.contains(&e.key)));
    }
    #[test]
    fn initial_grant_without_java_packets_respects_later_revocation() {
        let mut book = Book::default(); book.grant_initial();
        let id = crafting::recipes().iter().position(|r| blocks::item_name(r.result) == Some("crafting_table")).unwrap();
        assert!(book.knows_crafting(id));
        book.take(&[]); book.grant_initial(); assert!(!book.knows_crafting(id));
        book.give_all(); assert!(book.knows_crafting(id));
    }

    fn preview() -> crate::gameplay_catalog::Catalog { crate::gameplay_catalog::Catalog::new(crate::gameplay_catalog::Profile::Preview26_3) }

    #[test]
    fn profile_extension_preserves_every_native_display_and_is_inert_in_base() {
        let base = entries(); let extended = entries_for_catalog(preview());
        assert_eq!(extended.len(), base.len() + 2 * crate::gameplay_catalog::CONCRETE_SLABS.len() + crate::gameplay_catalog::WOOL_SLABS.len()
            + 2 * crate::gameplay_catalog::CONCRETE_STAIRS.len() + crate::gameplay_catalog::WOOL_STAIRS.len() + crate::crafting::mineral_recipes().len() + crate::stonecutter::mineral_recipes().len());
        for (id, native) in base.iter().enumerate() {
            assert_eq!(native.key, extended[id].key);
            assert_eq!(entry_body(id, native, false), entry_body(id, &extended[id], false));
        }
        let mut book = Book::for_catalog(preview());
        book.scan(&["dirt"], 1, false);
        assert!(!book.knows_recipe(crafting::RecipeId::WhiteConcreteSlab));
        book.scan(&["white_concrete"], 1, false);
        assert!(book.knows_recipe(crafting::RecipeId::WhiteConcreteSlab));
        book.update();
        assert!(book.crafting_display(base.len() as i32).is_some());
        let saved = book.to_json();
        let mut native = Book::from_json(&saved).unwrap();
        assert!(!native.knows_recipe(crafting::RecipeId::WhiteConcreteSlab));
        assert!(native.crafting_display(base.len() as i32).is_none());
        assert!(!native.learn_named(&["white_concrete_slab".into()]));
        native.take(&[]);
        assert!(native.to_json().contains("minecraft:white_concrete_slab"), "неизвестный ключ сохраняется без активации");
        let mut restored = Book { catalog: Some(preview()), ..Book::from_json(&saved).unwrap() };
        assert!(restored.knows_recipe(crafting::RecipeId::WhiteConcreteSlab));
        let removed = restored.take(&["minecraft:white_concrete_slab".into()]).unwrap();
        assert_eq!(removed, vec![REMOVE as u8, 0]); // после загрузки ещё не показывался
        assert!(!restored.knows_recipe(crafting::RecipeId::WhiteConcreteSlab));
        assert!(restored.learn_named(&["minecraft:white_concrete_slab".into()]));
        assert!(restored.knows_recipe(crafting::RecipeId::WhiteConcreteSlab));
        assert!(!restored.learn_named(&["white_concrete_slab".into(), "unknown".into()]));
    }

    #[test]
    fn shaped_extension_uses_observed_building_category_and_id_set_displays() {
        let entries = entries_for_catalog(preview());
        let entry = &entries[super::entries().len()];
        let body = entry_body(0, entry, true);
        let mut expected = vec![0, 1, 3, 1, 3];
        for _ in 0..3 { expected.extend([6, 2]); expected.extend(encode_varint(615)); }
        stack(&mut expected, crate::gameplay_catalog::WHITE_SLAB_ITEM, 6);
        slot(&mut expected, &[blocks::item_named("crafting_table").unwrap()]);
        expected.extend([CONCRETE_SLAB_GROUP, 0, 1, 3]);
        for _ in 0..3 { expected.push(2); expected.extend(encode_varint(615)); }
        expected.push(3);
        assert_eq!(body, expected);
        let recipe = crafting::recipe_for_catalog(preview(), crafting::RecipeId::WhiteConcreteSlab).unwrap();
        let ghost = ghost(1, recipe);
        assert_eq!(&ghost[..2], &[0x3f, 1]);
        let mut expected_ghost = vec![0x3f, 1];
        expected_ghost.extend(&expected[1..expected.len() - (4 + 3 * 3 + 1)]);
        assert_eq!(ghost, expected_ghost);
    }

    #[test]
    fn stonecut_extension_preserves_shaped_identity_and_unlocks_independently() {
        let mut book = Book::for_catalog(preview());
        let shaped_id = entries().len(); let cut_id = shaped_id + 1;
        book.learn_recipe(crafting::RecipeId::WhiteConcreteSlab);
        assert!(book.knows_recipe(crafting::RecipeId::WhiteConcreteSlab));
        assert!(book.crafting_display(shaped_id as i32).is_some());
        assert!(!book.known.contains(WHITE_SLAB_CUT_KEY));
        book.scan(&["white_concrete"], 1, false);
        assert!(book.known.contains(WHITE_SLAB_CUT_KEY));
        assert!(book.crafting_display(cut_id as i32).is_none());
        book.take(&["minecraft:white_concrete_slab".into()]);
        assert!(!book.knows_recipe(crafting::RecipeId::WhiteConcreteSlab));
        assert!(book.known.contains(WHITE_SLAB_CUT_KEY), "имя shaped не отнимает stonecut");
        assert!(book.take(&[WHITE_SLAB_CUT_KEY.into()]).is_some());
        assert!(!book.known.contains(WHITE_SLAB_CUT_KEY));
        assert!(book.learn_named(&[WHITE_SLAB_CUT_KEY.into()]));
        assert!(!book.knows_recipe(crafting::RecipeId::WhiteConcreteSlab), "имя stonecut не открывает shaped");
        assert!(book.known.contains(WHITE_SLAB_CUT_KEY));
        let entry = &book.entries()[cut_id];
        let actual = entry_body(0, entry, true);
        let mut expected = vec![0, 3, 6, 2]; expected.extend(encode_varint(615));
        stack(&mut expected, crate::gameplay_catalog::WHITE_SLAB_ITEM, 2);
        slot(&mut expected, &[blocks::item_named("stonecutter").unwrap()]);
        expected.extend([0, 10, 1, 1, 2]); expected.extend(encode_varint(615)); expected.push(3);
        assert_eq!(actual, expected);
        let mut base = Book::from_json(&book.to_json()).unwrap();
        assert!(base.crafting_display(cut_id as i32).is_none());
        assert!(!base.learn_named(&[WHITE_SLAB_CUT_KEY.into()]));
        assert!(base.take(&[WHITE_SLAB_CUT_KEY.into()]).is_none());
        base.take(&[]);
        assert!(base.known.contains(WHITE_SLAB_CUT_KEY), "неизвестный ключ не стирается базовым профилем");
    }

    #[test]
    fn family_unlocks_only_own_color_and_keeps_white_display_positions() {
        use crate::gameplay_catalog::CONCRETE_SLABS;
        let base_len = entries().len();
        let all = entries_for_catalog(preview());
        assert_eq!(all[base_len].key, "minecraft:white_concrete_slab");
        assert_eq!(all[base_len + 1].key, WHITE_SLAB_CUT_KEY);
        for (index, row) in CONCRETE_SLABS.iter().enumerate() {
            let id = crafting::RecipeId::concrete_slab(index).unwrap();
            let shaped = base_len + index * 2; let cut = shaped + 1;
            let mut book = Book::for_catalog(preview());
            book.scan(&[row.input_name], 1, false);
            let known_family = all.iter().filter(|entry| entry.slab.is_some() && book.known.contains(&entry.key)).count();
            assert_eq!(known_family, 4, "{} открывает только свои плиту и ступени (по паре shaped+камнерез)", row.name);
            assert!(book.knows_recipe(id));
            assert_eq!(book.crafting_display(shaped as i32).unwrap().result, row.item);
            assert!(book.crafting_display(cut as i32).is_none());
            assert_eq!(all[shaped].slab, Some(Slab::Concrete(index)));
            assert_eq!(all[cut].slab, Some(Slab::Concrete(index)));
            assert!(book.take(&[all[cut].key.clone()]).is_some());
            assert!(book.knows_recipe(id));
            assert!(book.learn_named(&[all[cut].key.clone()]));
            assert!(book.take(&[all[shaped].key.clone()]).is_some());
            assert!(!book.knows_recipe(id));
            assert!(book.known.contains(&all[cut].key));
            book.learn_recipe(id);
            assert!(book.knows_recipe(id), "O(1) lookup соответствует display-позиции");
            let mut base = Book::from_json(&book.to_json()).unwrap();
            assert!(!base.knows_recipe(id));
            assert!(!base.learn_named(&[all[shaped].key.clone()]));
            assert!(!base.learn_named(&[all[cut].key.clone()]));
            base.take(&[]);
            assert!(base.known.contains(&all[shaped].key));
            assert!(base.known.contains(&all[cut].key));
        }
    }

    #[test]
    fn wool_family_unlocks_own_shaped_and_preserves_existing_display_ids() {
        use crate::gameplay_catalog::{CONCRETE_SLABS, WOOL_SLABS};
        let base_len = entries().len();
        let wool_start = base_len + 2 * CONCRETE_SLABS.len();
        let all = entries_for_catalog(preview());
        assert_eq!(all.len(), wool_start + WOOL_SLABS.len() + 2 * crate::gameplay_catalog::CONCRETE_STAIRS.len() + crate::gameplay_catalog::WOOL_STAIRS.len() + crate::crafting::mineral_recipes().len() + crate::stonecutter::mineral_recipes().len());
        for (index, base) in entries().iter().enumerate() {
            assert_eq!(all[index].key, base.key);
            assert_eq!(entry_body(index, &all[index], false), entry_body(index, base, false));
        }
        for (index, row) in CONCRETE_SLABS.iter().enumerate() {
            assert_eq!(all[base_len + 2 * index].key, format!("minecraft:{}", row.name));
            assert_eq!(all[base_len + 2 * index + 1].key, format!("minecraft:{}_from_{}_stonecutting", row.name, row.input_name));
        }
        for (index, row) in WOOL_SLABS.iter().enumerate() {
            let id = crafting::RecipeId::WoolSlab(index as u8);
            let display = wool_start + index;
            let mut book = Book::for_catalog(preview());
            assert!(book.crafting_display(display as i32).is_none(), "неизвестный display закрыт");
            book.scan(&[row.input_name], 1, false);
            let known_slabs: Vec<_> = all.iter().filter(|entry| entry.slab.is_some() && book.known.contains(&entry.key)).collect();
            assert_eq!(known_slabs.len(), 2, "{} открывает только свои shaped плиту и ступени", row.input_name);
            assert_eq!(known_slabs[0].key, format!("minecraft:{}", row.name));
            assert_eq!(known_slabs[1].key, format!("minecraft:{}", row.input_name.replace("_wool", "_wool_stairs")));
            assert!(book.knows_recipe(id));
            assert_eq!(book.crafting_display(display as i32).unwrap().result, row.item);
            assert_eq!(book.entry_for_recipe(id).unwrap().slab, Some(Slab::Wool(index)));
            let invented_cut = format!("minecraft:{}_from_{}_stonecutting", row.name, row.input_name);
            assert!(!book.learn_named(std::slice::from_ref(&invented_cut)));
            assert!(book.take(&[invented_cut]).is_none());
            assert!(book.take(&[all[display].key.clone()]).is_some());
            assert!(!book.knows_recipe(id));
            assert!(book.learn_named(&[all[display].key.clone()]));
            assert!(book.knows_recipe(id));
            let saved = book.to_json();
            let loaded = Book::from_json(&saved).unwrap();
            assert!(loaded.known.contains(&all[display].key));
            let mut native = loaded;
            assert!(!native.knows_recipe(id));
            assert!(!native.learn_named(&[all[display].key.clone()]));
            native.take(&[]);
            assert!(native.known.contains(&all[display].key), "базовый профиль сохраняет чужой ключ");
            let mut restored = Book { known: native.known, ..Book::for_catalog(preview()) };
            assert!(restored.knows_recipe(id));
            restored.take(&[all[display].key.clone()]);
            restored.learn_recipe(id);
            assert!(restored.knows_recipe(id), "typed lookup не указывает на соседний цвет/семейство");
        }
        let mut book = Book::for_catalog(preview());
        book.learn_recipe(crafting::RecipeId::WoolSlab(255));
        assert!(!book.knows_recipe(crafting::RecipeId::WoolSlab(255)));
    }

    #[test]
    fn wool_displays_have_own_group_and_exact_color_ingredients() {
        let start = entries().len() + 2 * crate::gameplay_catalog::CONCRETE_SLABS.len();
        assert_ne!(WOOL_SLAB_GROUP, CONCRETE_SLAB_GROUP);
        for (index, row) in crate::gameplay_catalog::WOOL_SLABS.iter().enumerate() {
            let entry = &entries_for_catalog(preview())[start + index];
            let mut expected = vec![0, 1, 3, 1, 3];
            for _ in 0..3 { ingredient_set(&mut expected, &[row.input_item]); }
            stack(&mut expected, row.item, 6);
            slot(&mut expected, &[blocks::item_named("crafting_table").unwrap()]);
            let mut expected_ghost = vec![0x3f, 1];
            expected_ghost.extend_from_slice(&expected[1..]);
            let recipe = crafting::recipe_for_catalog(preview(), crafting::RecipeId::WoolSlab(index as u8)).unwrap();
            assert_eq!(ghost(1, recipe), expected_ghost, "призрачный рецепт {}", row.name);
            expected.extend([WOOL_SLAB_GROUP, 0, 1, 3]);
            for _ in 0..3 { expected.push(2); expected.extend(encode_varint(row.input_item)); }
            expected.push(3);
            assert_eq!(entry_body(0, entry, true), expected, "{}", row.name);
        }
    }


    #[test]
    fn stairs_entries_follow_observed_26_3_displays_and_unlocks() {
        use crate::gameplay_catalog::{CONCRETE_STAIRS, WOOL_STAIRS};
        let all = entries_for_catalog(preview());
        let start = stairs_start();
        // Бетон: shaped 3x3 (6 непустых клеток, 3 пустых слота типа 0) и камнерез 1 -> 1.
        let shaped = &all[start];
        assert_eq!(shaped.key, "minecraft:white_concrete_stairs");
        let mut expected = vec![0, 1, 3, 3, 9];
        for cell in 0..9 {
            if matches!(cell, 1 | 2 | 5) { expected.push(0); } else { expected.extend([6, 2]); expected.extend(encode_varint(615)); }
        }
        stack(&mut expected, CONCRETE_STAIRS[0].item, 4);
        slot(&mut expected, &[blocks::item_named("crafting_table").unwrap()]);
        expected.extend([CONCRETE_STAIRS_GROUP, 0, 1, 6]);
        for _ in 0..6 { expected.push(2); expected.extend(encode_varint(615)); }
        expected.push(3);
        assert_eq!(entry_body(0, shaped, true), expected);
        let cut = &all[start + 1];
        assert_eq!(cut.key, "minecraft:white_concrete_stairs_from_white_concrete_stonecutting");
        let mut expected = vec![0, 3, 6, 2]; expected.extend(encode_varint(615));
        stack(&mut expected, CONCRETE_STAIRS[0].item, 1);
        slot(&mut expected, &[blocks::item_named("stonecutter").unwrap()]);
        expected.extend([0, 10, 1, 1, 2]); expected.extend(encode_varint(615)); expected.push(3);
        assert_eq!(entry_body(0, cut, true), expected);
        // Шерсть: только shaped, своя группа; камнерезной записи нет.
        let wool = start + 2 * CONCRETE_STAIRS.len();
        assert_eq!(all.len(), wool + WOOL_STAIRS.len() + crate::crafting::mineral_recipes().len() + crate::stonecutter::mineral_recipes().len());
        for (index, row) in WOOL_STAIRS.iter().enumerate() {
            assert_eq!(all[wool + index].key, format!("minecraft:{}", row.name));
            assert_eq!(all[wool + index].slab.unwrap().group(), WOOL_STAIRS_GROUP);
            let id = crafting::RecipeId::WoolStairs(index as u8);
            let mut book = Book::for_catalog(preview());
            book.scan(&[row.input_name], 1, false);
            assert!(book.knows_recipe(id) && book.entry_for_recipe(id).unwrap().key == all[wool + index].key);
            assert!(!book.learn_named(&[format!("minecraft:{}_from_{}_stonecutting", row.name, row.input_name)]));
            assert_eq!(book.crafting_display((wool + index) as i32).unwrap().result, row.item);
        }
        for (index, row) in CONCRETE_STAIRS.iter().enumerate() {
            let id = crafting::RecipeId::ConcreteStairs(index as u8);
            let mut book = Book::for_catalog(preview());
            book.scan(&[row.input_name], 1, false);
            assert!(book.knows_recipe(id));
            assert!(book.known.contains(&all[start + 2 * index + 1].key));
            assert!(Book::for_catalog(crate::gameplay_catalog::Catalog::new(crate::gameplay_catalog::Profile::Java26_1_2)).entry_for_recipe(id).is_none());
        }
    }
}
