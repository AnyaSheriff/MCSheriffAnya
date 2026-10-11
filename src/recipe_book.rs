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

#[derive(Default)]
pub struct Book {
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

enum Display {
    Crafting(&'static crafting::Recipe),
    Furnace { kind: Kind, input: i32, result: i32, xp: f32 },
    Stonecutting { input: i32, result: i32, count: i32 },
}
struct Entry { key: String, display: Display }

fn entries() -> &'static [Entry] {
    static ENTRIES: OnceLock<Vec<Entry>> = OnceLock::new();
    ENTRIES.get_or_init(|| {
        let mut entries: Vec<Entry> = crafting::recipes().iter().enumerate().map(|(index, recipe)| Entry {
            key: format!("crafting/{}/{index}", blocks::item_name(recipe.result).unwrap_or("unknown")),
            display: Display::Crafting(recipe),
        }).collect();
        for (kind, key) in [(Kind::Furnace, "smelting"), (Kind::BlastFurnace, "blasting"), (Kind::Smoker, "smoking")] {
            for name in blocks::item_names() {
                let input = blocks::item_named(name).unwrap();
                if let Some((result, xp)) = furnace::smelt(kind, input) {
                    entries.push(Entry { key: format!("{key}/{name}"), display: Display::Furnace { kind, input, result, xp } });
                }
            }
        }
        for cut in crate::stonecutter::recipes() {
            let (input, result) = (blocks::item_name(cut.input).unwrap_or("unknown"), blocks::item_name(cut.output).unwrap_or("unknown"));
            entries.push(Entry { key: format!("stonecutting/{result}_from_{input}"), display: Display::Stonecutting { input: cut.input, result: cut.output, count: cut.count } });
        }
        entries
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
    pub fn learn_crafting(&mut self, id: usize) {
        if let Some(entry) = entries().get(id) { self.dirty |= self.known.insert(entry.key.clone()); }
    }
    pub fn knows_crafting(&self, id: usize) -> bool { entries().get(id).is_some_and(|e| self.known.contains(&e.key)) }
    pub fn crafting_display(&self, id: i32) -> Option<&'static crafting::Recipe> {
        let entry = entries().get(usize::try_from(id).ok()?)?;
        if !self.known.contains(&entry.key) { return None; }
        match entry.display { Display::Crafting(recipe) => Some(recipe), _ => None }
    }
    pub fn scan(&mut self, names: &[&str], occupied: usize, water: bool) {
        for entry in entries() {
            if self.known.contains(&entry.key) { continue; }
            let unlocked = match &entry.display {
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
            self.known.extend(entries().iter().filter(|e| e.key.starts_with("crafting/crafting_table/")).map(|e| e.key.clone()));
        }
    }

    pub fn started(&self) -> bool { self.started }

    /// Пакеты «рецепты добавлены» к отправке. Первый — со всем, что известно, с флагом замены.
    pub fn update(&mut self) -> Vec<Vec<u8>> {
        if self.started { self.grant_initial(); }
        let added: Vec<_> = entries().iter().enumerate().filter(|(_, e)| self.known.contains(&e.key) && !self.sent.contains(&e.key)).collect();
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
            for cell in recipe.cells { slot(&mut body, cell); }
            stack(&mut body, recipe.result, recipe.count);
            slot(&mut body, &[blocks::item_named("crafting_table").unwrap()]);
            body.extend([0, 3, 1]); // группа отсутствует, misc, требования есть
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
            slot(&mut body, &[*input]);
            stack(&mut body, *result, *count);
            slot(&mut body, &[blocks::item_named("stonecutter").unwrap()]);
            body.extend([0, 10, 0]); // группа и требования отсутствуют, категория stonecutter
        }
    }
    body.push(if announce { 3 } else { 0 });
    body
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
    pub fn give_all(&mut self) { self.batch = true; self.dirty |= entries().iter().fold(false, |any, e| self.known.insert(e.key.clone()) | any); }

    /// `recipe take <игрок> *` и `recipe take <игрок> <имя>`: пакет «убрать из книги» для
    /// уже показанных (пусто — имя не однозначно, ничего не сделано).
    pub fn take(&mut self, names: &[String]) -> Option<Vec<u8>> {
        let keys: Vec<String> = if names.is_empty() { self.known.iter().cloned().collect() } else {
            let mut keys = Vec::new();
            for name in names {
                let name = name.strip_prefix("minecraft:").unwrap_or(name);
                let found: Vec<_> = entries().iter().filter(|e| matches!(&e.display, Display::Crafting(r) if blocks::item_name(r.result) == Some(name))).collect();
                if found.len() != 1 { return None; }
                keys.push(found[0].key.clone());
            }
            keys
        };
        let ids: Vec<i32> = entries().iter().enumerate().filter(|(_, e)| keys.contains(&e.key) && self.sent.contains(&e.key)).map(|(i, _)| i as i32).collect();
        for key in &keys { self.dirty |= self.known.remove(key); self.sent.remove(key); }
        let mut body = encode_varint(REMOVE);
        body.extend(encode_varint(ids.len() as i32));
        for id in ids { body.extend(encode_varint(id)); }
        Some(body)
    }

    /// Knowledge Book: сначала разрешаем весь список, затем открываем его.
    /// Wiki, Knowledge Book: «adding it to their recipe book». Синтетические
    /// ключи текущего recipe registry не выдаём за vanilla ID: поддержаны
    /// только однозначные crafting-result IDs; остальные отвергаем [проверить].
    pub fn learn_named(&mut self, names: &[String]) -> bool {
        let mut keys = Vec::new();
        for name in names {
            let name = name.strip_prefix("minecraft:").unwrap_or(name);
            let candidates: Vec<_> = entries().iter().filter(|entry| match &entry.display {
                Display::Crafting(recipe) => blocks::item_name(recipe.result) == Some(name),
                _ => false,
            }).collect();
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
    body.extend(encode_varint(recipe.cells.len() as i32)); for cell in recipe.cells { slot(&mut body, cell); }
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

}
