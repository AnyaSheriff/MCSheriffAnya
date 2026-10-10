// Прогресс игрока: статистика (stats.rs) и достижения (advancements.rs) в одном
// месте, чтобы места событий в игре дёргали один короткий вызов.
//
// События ставят счётчики и выдают условия достижений; рассылка клиенту,
// объявление в чате, награда опытом и запись на диск — раз в такт на
// подключении игрока (network/play.rs, `progress_tick`).

use crate::advancements::{self, AdvState};
use crate::dimension::Dimension;
use crate::stats::{Cat, Stats};

/// События из общего такта; [Bedrock: потом] прогресс хранит Java-подключение.
#[derive(Clone, Debug)]
pub enum Event {
    Trigger(&'static str),
    Bred(&'static str),
    Killed { kind: &'static str, dimension: Dimension },
    DamageDealt(crate::survival::Hit),
    Stat(Cat, &'static str, i32),
    ProjectileHit { trident: bool, kind: &'static str, killed: bool, distance: f64, crossbow: bool },
}

/// Как часто писать на диск, тактов (5 минут, как автосохранение оригинала).
pub const SAVE_EVERY: u64 = 6000;

pub struct Progress {
    pub stats: Stats,
    pub adv: AdvState,
    pub recipes: crate::recipe_book::Book,
    /// Клиент просил статистику (Client Status, действие 1).
    pub stats_requested: bool,
    /// Сколько тактов в игре — для записи на диск.
    pub ticks: u64,

}

impl Progress {
    pub fn load(uuid: &[u8; 16]) -> Self {
        Progress { stats: Stats::load(uuid), adv: AdvState::load(uuid), recipes: crate::recipe_book::Book::load(uuid), stats_requested: false, ticks: 0 }
    }

    pub fn save(&mut self, uuid: &[u8; 16]) {
        self.stats.save(uuid);
        self.adv.save(uuid);
        self.recipes.save(uuid);
    }

    // ---- статистика ----

    pub fn custom(&mut self, name: &str, by: i32) {
        self.stats.custom(name, by);
    }

    /// Предмет израсходован на использование (блок поставлен и т. п.).
    pub fn used(&mut self, item: i32) {
        if let Some(name) = crate::blocks::item_name(item) {
            self.stats.add(Cat::Used, name, 1);
        }
    }

    /// Удар считается использованием только для предмета-оружия, не для
    /// случайного блока в руке (вики, Statistics: «Times Used») [проверить].
    pub fn used_weapon(&mut self, item: i32) {
        if crate::combat::info(item).is_some_and(|info| info.damage_per_attack > 0) { self.used(item); }
    }

    pub fn picked_up(&mut self, item: i32, count: i32) {
        if let Some(name) = crate::blocks::item_name(item) {
            self.stats.add(Cat::PickedUp, name, count);
        }
    }

    pub fn dropped(&mut self, item: i32, count: i32) {
        if let Some(name) = crate::blocks::item_name(item) {
            self.stats.add(Cat::Dropped, name, count);
            self.stats.custom("drop", 1);
        }
    }

    /// Сломан блок (состояние) не в творческом.
    pub fn mined(&mut self, state: i32) {
        if let Some(name) = crate::blocks::block_at_state(state) {
            self.stats.add(Cat::Mined, name, 1);
        }
    }

    /// Нанесён урон ударом: в десятых очка здоровья (полусердца).
    pub fn dealt(&mut self, damage: f32) {
        self.stats.custom("damage_dealt", (damage * 10.0).round() as i32);
    }

    pub fn damage_dealt(&mut self, hit: crate::survival::Hit) {
        self.dealt(hit.health_damage());
        self.custom("damage_dealt_absorbed", (hit.absorbed * 10.0).round() as i32);
        self.custom("damage_dealt_resisted", (hit.resisted * 10.0).round() as i32);
    }

    pub fn took(&mut self, damage: f32) {
        self.stats.custom("damage_taken", (damage * 10.0).round() as i32);
    }

    /// Игрок убил моба (`mob_kills`, `killed`, достижения).
    pub fn killed_mob(&mut self, name: &str) {
        if matches!(name, "end_crystal" | "shulker_bullet" | "leash_knot") { return; }
        self.stats.custom("mob_kills", 1);
        self.stats.add(Cat::Killed, name, 1);
        self.adv.grant("adventure/root", "killed_something");

        let key = format!("minecraft:{name}");

        self.adv.grant("adventure/kill_a_mob", &key);
        self.adv.grant("adventure/kill_all_mobs", &key);

        if name == "ender_dragon" { self.adv.grant("end/kill_dragon", "killed_dragon"); }
    }

    /// Игрок достроил моба из блоков (вики, Advancement: «Withering Heights»
    /// и «Hired Help») [проверить: оригинал даёт всем игрокам рядом].
    pub fn summoned(&mut self, name: &str) {
        match name {
            "wither" => {
                self.adv.grant("nether/summon_wither", "summoned_entity");
            }
            "iron_golem" => {
                self.adv.grant("adventure/summon_iron_golem", "summoned_golem");
            }
            _ => {}
        }
    }

    pub fn killed_player(&mut self) {
        self.adv.grant("adventure/root", "killed_something");
        self.stats.custom("player_kills", 1);
        self.stats.add(Cat::Killed, "player", 1);
    }

    /// Игрок умер: счётчики, сброс времени с последней смерти и сна.
    pub fn died(&mut self, killer_kind: Option<&str>) {
        self.stats.custom("deaths", 1);
        self.stats.set_custom("time_since_death", 0);
        self.stats.set_custom("time_since_rest", 0);

        if let Some(kind) = killer_kind {
            self.adv.grant("adventure/root", "killed_by_something");
            self.stats.add(Cat::KilledBy, kind, 1);
        }
    }

    /// Вики, Statistics: время игры включает экран смерти, время после смерти — нет.
    /// [проверить] отсчёт на первом такте сна/возрождения.
    pub fn tick_player(&mut self, sneaking: bool, alive: bool, sleeping: bool) {
        self.ticks += 1;
        self.stats.custom("play_time", 1);
        self.stats.custom("total_world_time", 1);
        if alive { self.stats.custom("time_since_death", 1); }
        if sleeping { self.stats.set_custom("time_since_rest", 0); }
        else if alive { self.stats.custom("time_since_rest", 1); }
        if sneaking && alive { self.stats.custom("sneak_time", 1); }
    }

    /// Пройденное за пакет места (метры → сантиметры по округлению, как у оригинала).
    #[allow(clippy::too_many_arguments)]
    pub fn moved(&mut self, dx: f64, dy: f64, dz: f64, on_ground: bool, sprinting: bool, sneaking: bool, jumped: bool) {
        let flat = dx.hypot(dz);
        let cm = (flat * 100.0).round() as i32;

        if jumped {
            self.stats.custom("jump", 1);
        }

        if on_ground && cm > 0 {
            let name = if sprinting {
                "sprint_one_cm"
            } else if sneaking {
                "crouch_one_cm"
            } else {
                "walk_one_cm"
            };

            self.stats.custom(name, cm);
        } else if !on_ground && cm > 25 {
            self.stats.custom("fly_one_cm", cm);
        }
        let _ = dy; // Вертикаль падения учитывается при приземлении, не на каждом пакете.
    }

    /// Состоялась сделка с жителем.
    pub fn traded(&mut self) {
        self.stats.custom("traded_with_villager", 1);
        self.adv.grant("adventure/trade", "traded");
    }

    /// Сработал тотем бессмертия.
    pub fn used_totem(&mut self) {
        self.stats.add(Cat::Used, "totem_of_undying", 1);
        self.adv.grant("adventure/totem_of_undying", "used_totem");
    }

    /// Съедено или выпито: статистика предмета, достижения про еду.
    pub fn consumed(&mut self, item: i32) {
        let Some(name) = crate::blocks::item_name(item) else {
            return;
        };

        self.stats.add(Cat::Used, name, 1);
        self.adv.grant("husbandry/root", "consumed_item");
        self.adv.grant("husbandry/balanced_diet", name);
    }

    /// Предмет зачарован у стола.
    pub fn enchanted(&mut self) {
        self.stats.custom("enchant_item", 1);
        self.adv.grant("story/enchant_item", "enchanted_item");
    }

    /// Игрок стоит в биоме: «Время приключений» и «Горячие туристические места».
    pub fn biome(&mut self, dimension: Dimension, name: &str) {
        let name = name.strip_prefix("minecraft:").unwrap_or(name);
        let key = format!("minecraft:{name}");

        match dimension {
            Dimension::Overworld => {
                self.adv.grant("adventure/adventuring_time", &key);
            }
            Dimension::Nether => {
                self.adv.grant("nether/explore_nether", name.strip_prefix("minecraft:").unwrap_or(name));
            }
            Dimension::End => {}
        }
    }

    /// Лёг в кровать.
    pub fn slept(&mut self) {
        self.stats.custom("sleep_in_bed", 1);
        // Вики, Phantom: «Entering a bed … resets» время без сна.
        self.stats.set_custom("time_since_rest", 0);
        self.adv.grant("adventure/sleep_in_bed", "slept_in_bed");
    }

    /// Одиночное условие после подтверждённого события (вики, Advancement).
    pub fn trigger(&mut self, id: &str) {
        if let Some(def) = advancements::def(id)
            && let advancements::Criteria::One(criterion) = def.criteria
        {
            self.adv.grant(id, criterion);
        }
    }

    pub fn crafted(&mut self, item: i32, count: i32) {
        if let Some(name) = crate::blocks::item_name(item) {
            self.stats.add(Cat::Crafted, name, count);
        }
    }

    /// Вики, Advancement: зелье нужно забрать из варочной стойки;
    /// пустая бутылка сама по себе не является зельем.
    pub fn brewed(&mut self, item: i32) {
        if matches!(crate::blocks::item_name(item), Some("potion" | "splash_potion" | "lingering_potion")) {
            self.trigger("nether/brew_potion");
        }
    }

    pub fn broken(&mut self, item: i32) {
        if let Some(name) = crate::blocks::item_name(item) {
            self.stats.add(Cat::Broken, name, 1);
        }
    }

    pub fn bred(&mut self, kind: &str) {
        self.custom("animals_bred", 1);
        self.trigger("husbandry/breed_an_animal");
        self.adv.grant("husbandry/bred_all_animals", &format!("minecraft:{kind}"));
    }

    pub fn tamed(&mut self, kind: &str, variant: &str) {
        self.trigger("husbandry/tame_an_animal");
        let id = match kind { "cat" => "husbandry/complete_catalogue", "wolf" => "husbandry/whole_pack", _ => return };
        self.adv.grant(id, &format!("minecraft:{variant}"));
    }

    pub fn trimmed(&mut self, template: &str) {
        if template.ends_with("_armor_trim_smithing_template") {
            self.trigger("adventure/trim_with_any_armor_pattern");
            self.adv.grant("adventure/trim_with_all_exclusive_armor_patterns", &format!("armor_trimmed_minecraft:{template}_smithing_trim"));
        }
    }

    pub fn planted(&mut self, block: &str) {
        let criterion = match block { "torchflower_crop" => "torchflower", "pitcher_crop" => "pitcher_pod", other => other };
        self.adv.grant("husbandry/plant_seed", criterion);
        self.adv.grant("husbandry/plant_any_sniffer_seed", criterion);
    }

    /// Очередь мира передаёт событие только его участнику, не всем зрителям.
    pub fn event(&mut self, event: Event) {
        match event {
            Event::Trigger(id) => self.trigger(id),
            Event::Bred(kind) => self.bred(kind),
            Event::Killed { kind, dimension } => {
                self.killed_mob(kind);
                if kind == "ghast" && dimension == Dimension::Overworld { self.trigger("nether/uneasy_alliance"); }
            }
            Event::DamageDealt(hit) => self.damage_dealt(hit),
            Event::Stat(cat, name, count) => self.stats.add(cat, name, count),
            Event::ProjectileHit { trident, kind, killed, distance, crossbow } => {
                self.trigger(if trident { "adventure/throw_trident" } else { "adventure/shoot_arrow" });
                if killed && crossbow && kind == "pillager" { self.trigger("adventure/whos_the_pillager_now"); }
                if killed && kind == "skeleton" && distance >= 50.0 {
                    self.trigger("adventure/sniper_duel");
                }
            }
        }
    }

    // ---- достижения ----

    /// Перейдено в другое измерение.
    pub fn entered(&mut self, dimension: Dimension) {
        match dimension {
            Dimension::Nether => {
                self.adv.grant("story/enter_the_nether", "entered_nether");
                self.adv.grant("nether/root", "entered_nether");
            }
            Dimension::End => {
                self.adv.grant("story/enter_the_end", "entered_end");
                self.adv.grant("end/root", "entered_end");
            }
            _ => {}
        }
    }

    /// Просмотр инвентаря: выдать условия «есть предмет». `names` — имена
    /// всех предметов у игрока, включая броню и вторую руку.
    pub fn scan_items(&mut self, names: &[&str]) {
        // Вики, Advancement: полный комплект и три огнесвета нужны одновременно.
        // Не кэшируем: /advancement revoke должен позволять повторную выдачу.
        for (id, items) in [
            ("nether/netherite_armor", &["netherite_helmet", "netherite_chestplate", "netherite_leggings", "netherite_boots"][..]),
            ("husbandry/froglights", &["ochre_froglight", "verdant_froglight", "pearlescent_froglight"][..]),
        ] {
            if items.iter().all(|item| names.contains(item)) {
                for item in items { self.adv.grant(id, item); }
            }
        }

        for (id, criterion, items) in advancements::HAS_ITEM {
            if *id == "nether/netherite_armor" { continue; }
            if self.adv.has(id, criterion) {
                continue;
            }

            if items.iter().any(|item| names.contains(item)) {
                self.adv.grant(id, criterion);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn building_a_wither_or_a_golem_grants_its_advancement() {
        let mut progress = Progress { stats: Stats::new(), adv: AdvState::new(), recipes: crate::recipe_book::Book::default(), stats_requested: false, ticks: 0 };

        progress.summoned("wither");
        progress.summoned("snow_golem");
        assert!(progress.adv.has("nether/summon_wither", "summoned_entity"));
        assert!(!progress.adv.has("adventure/summon_iron_golem", "summoned_golem"));
        progress.summoned("iron_golem");
        assert!(progress.adv.has("adventure/summon_iron_golem", "summoned_golem"));
    }

    fn fresh() -> Progress {
        Progress { stats: Stats::new(), adv: AdvState::new(), recipes: crate::recipe_book::Book::default(), stats_requested: false, ticks: 0 }
    }

    fn assert_done(p: &Progress, ids: &[&str]) {
        for id in ids { assert!(p.adv.is_done(id), "{id}"); }
    }

    #[test]
    fn minecraft_tab_inventory_and_actions() {
        for (id, criterion, alternatives) in advancements::HAS_ITEM.iter().filter(|(id, _, _)| id.starts_with("story/")) {
            for item in *alternatives {
                let mut p = fresh();
                p.scan_items(&[item]);
                assert!(p.adv.has(id, criterion), "{id}: {item}");
            }
        }
        let mut p = fresh();
        p.entered(Dimension::Nether);
        p.entered(Dimension::End);
        p.enchanted();
        for id in ["story/cure_zombie_villager", "story/follow_ender_eye", "story/deflect_arrow"] { p.event(Event::Trigger(id)); }
        assert_done(&p, &["story/enter_the_nether", "story/enter_the_end", "story/enchant_item", "story/cure_zombie_villager", "story/follow_ender_eye", "story/deflect_arrow"]);
        p.scan_items(&["diamond"]);
        p.adv.revoke_all("story/mine_diamond");
        p.scan_items(&["diamond"]);
        assert_done(&p, &["story/mine_diamond"]);
    }

    #[test]
    fn nether_tab_compound_inventory_biomes_and_kills() {
        let mut p = fresh();
        let armor = ["netherite_helmet", "netherite_chestplate", "netherite_leggings", "netherite_boots"];
        for piece in armor { p.scan_items(&[piece]); }
        assert!(!p.adv.is_done("nether/netherite_armor"));
        p.scan_items(&armor);
        for biome in ["nether_wastes", "soul_sand_valley", "crimson_forest", "warped_forest", "basalt_deltas"] { p.biome(Dimension::Nether, biome); }
        p.event(Event::Killed { kind: "ghast", dimension: Dimension::Nether });
        assert!(!p.adv.is_done("nether/return_to_sender"));
        assert!(!p.adv.is_done("nether/uneasy_alliance"));
        p.event(Event::Killed { kind: "ghast", dimension: Dimension::Overworld });
        for id in ["nether/find_fortress", "nether/find_bastion", "nether/fast_travel", "nether/brew_potion", "nether/charge_respawn_anchor", "nether/return_to_sender"] { p.event(Event::Trigger(id)); }
        assert_done(&p, &["nether/netherite_armor", "nether/explore_nether", "nether/uneasy_alliance", "nether/find_fortress", "nether/find_bastion", "nether/fast_travel", "nether/brew_potion", "nether/charge_respawn_anchor", "nether/return_to_sender"]);
    }

    #[test]
    fn end_tab_events_and_inventory() {
        let mut p = fresh();
        p.entered(Dimension::End);
        p.killed_mob("ender_dragon");
        p.scan_items(&["dragon_egg", "elytra", "dragon_breath"]);
        assert!(!p.adv.is_done("end/dragon_breath"), "requires bottle collection, not inventory");
        for id in ["end/respawn_dragon", "end/dragon_breath", "end/find_end_city", "end/levitate"] { p.event(Event::Trigger(id)); }
        assert_done(&p, &["end/root", "end/kill_dragon", "end/dragon_egg", "end/elytra", "end/respawn_dragon", "end/dragon_breath", "end/find_end_city", "end/levitate"]);
        assert!(!p.adv.is_done("end/enter_end_gateway"), "gateway travel system absent");
    }

    #[test]
    fn adventure_tab_projectiles_trims_and_actions() {
        for (kind, killed, distance, expected) in [("skeleton", true, 49.99, false), ("skeleton", false, 50.0, false), ("stray", true, 60.0, false), ("skeleton", true, 50.0, true)] {
            let mut p = fresh();
            p.event(Event::ProjectileHit { trident: false, kind, killed, distance, crossbow: false });
            assert_done(&p, &["adventure/shoot_arrow"]);
            assert_eq!(p.adv.is_done("adventure/sniper_duel"), expected);
        }
        let mut p = fresh();
        p.event(Event::ProjectileHit { trident: true, kind: "cow", killed: false, distance: 1.0, crossbow: false });
        p.traded(); p.slept(); p.used_totem();
        p.trimmed("diamond");
        assert!(!p.adv.is_done("adventure/trim_with_any_armor_pattern"));
        for trim in ["spire", "snout", "rib", "ward", "silence", "vex", "tide", "wayfinder"] { p.trimmed(&format!("{trim}_armor_trim_smithing_template")); }
        for biome in advancements::BIOMES { p.biome(Dimension::Overworld, biome.strip_prefix("minecraft:").unwrap()); }
        assert_done(&p, &["adventure/throw_trident", "adventure/trade", "adventure/sleep_in_bed", "adventure/totem_of_undying", "adventure/trim_with_any_armor_pattern", "adventure/trim_with_all_exclusive_armor_patterns", "adventure/adventuring_time"]);
        p.died(Some("zombie"));
        assert_done(&p, &["adventure/root"]);
        assert!(!p.adv.is_done("adventure/kill_a_mob"));
        p.killed_mob("cow");
        assert!(!p.adv.is_done("adventure/kill_a_mob"));
        p.killed_mob("zombie");
        assert_done(&p, &["adventure/kill_a_mob"]);
    }

    #[test]
    fn husbandry_tab_breeding_taming_food_and_plants() {
        let mut p = fresh();
        for animal in ["cow", "pig", "sheep", "chicken", "wolf", "cat", "hoglin", "strider"] {
            p.event(Event::Bred(animal));
            assert!(p.adv.has("husbandry/bred_all_animals", &format!("minecraft:{animal}")));
        }
        assert_eq!(p.stats.get(Cat::Custom, "animals_bred"), 8);
        for cat in crate::mob::tame::CAT_VARIANTS { p.tamed("cat", cat); }
        for wolf in crate::mob::tame::WOLF_VARIANTS { p.tamed("wolf", wolf); }
        for food in advancements::def("husbandry/balanced_diet").unwrap().criteria.names() {
            p.consumed(crate::blocks::item_named(&food).unwrap());
        }
        for plant in ["wheat", "pumpkin_stem", "melon_stem", "beetroots", "nether_wart", "torchflower_crop", "pitcher_crop"] { p.planted(plant); }
        let lights = ["ochre_froglight", "verdant_froglight", "pearlescent_froglight"];
        for light in lights { p.scan_items(&[light]); }
        assert!(!p.adv.is_done("husbandry/froglights"));
        p.scan_items(&lights);
        assert_done(&p, &["husbandry/root", "husbandry/breed_an_animal", "husbandry/tame_an_animal", "husbandry/complete_catalogue", "husbandry/whole_pack", "husbandry/balanced_diet", "husbandry/plant_seed", "husbandry/plant_any_sniffer_seed", "husbandry/froglights"]);
        assert!(!p.adv.is_done("husbandry/bred_all_animals"), "missing animal systems remain required");
    }

    #[test]
    fn statistics_counts_amounts_once_and_excludes_non_living_entities() {
        let mut p = fresh();
        let diamond = crate::blocks::item_named("diamond").unwrap();
        p.crafted(diamond, 4); p.used(diamond); p.broken(diamond); p.picked_up(diamond, 12); p.dropped(diamond, 12);
        for (category, count) in [(Cat::Crafted, 4), (Cat::Used, 1), (Cat::Broken, 1), (Cat::PickedUp, 12), (Cat::Dropped, 12)] { assert_eq!(p.stats.get(category, "diamond"), count); }
        assert_eq!(p.stats.get(Cat::Custom, "drop"), 1);
        p.killed_mob("end_crystal"); p.killed_mob("shulker_bullet"); p.killed_mob("leash_knot");
        assert_eq!(p.stats.get(Cat::Custom, "mob_kills"), 0);
        p.killed_mob("zombie"); p.died(Some("skeleton"));
        assert_eq!(p.stats.get(Cat::Killed, "zombie"), 1);
        assert_eq!(p.stats.get(Cat::KilledBy, "skeleton"), 1);
        p.moved(1.0, 0.0, 0.0, true, false, false, false);
        p.moved(2.0, 0.0, 0.0, true, true, false, false);
        p.moved(3.0, 0.0, 0.0, true, false, true, false);
        p.moved(0.25, 0.4, 0.0, false, false, false, true);
        p.moved(0.30, 0.0, 0.0, false, false, false, false);
        for (name, expected) in [("walk_one_cm", 100), ("sprint_one_cm", 200), ("crouch_one_cm", 300), ("fly_one_cm", 30), ("jump", 1)] { assert_eq!(p.stats.get(Cat::Custom, name), expected, "{name}"); }
        p.tick_player(true, true, false); p.tick_player(false, true, false); p.slept();
        assert_eq!(p.stats.get(Cat::Custom, "time_since_rest"), 0);
        assert_eq!(p.stats.get(Cat::Custom, "time_since_death"), 2);
        assert_eq!(p.stats.get(Cat::Custom, "play_time"), 2);
        assert_eq!(p.stats.get(Cat::Custom, "sneak_time"), 1);
    }
}

#[cfg(test)]
mod regression_tests {
    use super::*;
    fn fresh() -> Progress {
        Progress { stats: Stats::new(), adv: AdvState::new(), recipes: Default::default(), stats_requested: false, ticks: 0 }
    }

    #[test]
    fn brewing_requires_a_potion_and_weapon_use_requires_a_weapon() {
        for (name, brewed) in [("glass_bottle", false), ("blaze_powder", false), ("potion", true), ("splash_potion", true), ("lingering_potion", true)] {
            let mut p = fresh();
            p.brewed(crate::blocks::item_named(name).unwrap());
            assert_eq!(p.adv.is_done("nether/brew_potion"), brewed, "{name}");
        }
        let mut p = fresh();
        for (name, expected) in [("dirt", 0), ("bow", 0), ("diamond_sword", 1), ("iron_axe", 1), ("mace", 1)] {
            p.used_weapon(crate::blocks::item_named(name).unwrap());
            assert_eq!(p.stats.get(Cat::Used, name), expected, "{name}");
        }
    }

    #[test]
    fn death_screen_and_sleep_do_not_advance_the_wrong_clock() {
        let mut p = fresh();
        p.tick_player(true, true, false);
        p.died(None);
        p.tick_player(true, false, false);
        assert_eq!(p.stats.get(Cat::Custom, "play_time"), 2);
        assert_eq!(p.stats.get(Cat::Custom, "time_since_death"), 0);
        assert_eq!(p.stats.get(Cat::Custom, "sneak_time"), 1);
        p.tick_player(false, true, true);
        assert_eq!(p.stats.get(Cat::Custom, "time_since_death"), 1);
        assert_eq!(p.stats.get(Cat::Custom, "time_since_rest"), 0);
        p.tick_player(false, true, false);
        assert_eq!(p.stats.get(Cat::Custom, "time_since_rest"), 1);
    }
    #[test]
    fn actual_damage_separates_health_absorption_and_resistance() {
        let mut p = fresh();
        let mut target = crate::survival::Vitals::new();
        target.give_effect(crate::survival::EffectKind::Resistance, 0, 100);
        target.give_effect(crate::survival::EffectKind::Absorption, 0, 100);
        let hit = target.hurt(10.0, crate::survival::Source::Generic, false).unwrap();
        p.event(Event::DamageDealt(hit));
        assert_eq!(target.health, 16.0);
        for (name, expected) in [("damage_dealt", 40), ("damage_dealt_absorbed", 40), ("damage_dealt_resisted", 20)] {
            assert_eq!(p.stats.get(Cat::Custom, name), expected, "{name}");
        }
        assert!(target.hurt(10.0, crate::survival::Source::Generic, false).is_none());
    }

}
