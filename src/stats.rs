// Статистика игрока (вики, Statistics): счётчики «сломано, сделано, использовано,
// подобрано, выброшено, убито, убит кем» по предметам, блокам и сущностям, а
// также общие (`minecraft:custom`): время игры, пройденные пути, прыжки, урон…
//
// Хранится по файлу на игрока: `world/players/stats/<uuid>.json` — путь и вид
// как у оригинала 26.1 (снято с мира чёрного ящика: корень `stats` с видами
// `minecraft:<вид>` и `DataVersion`). Экран статистики клиент просит пакетом
// Client Status (действие 1), сервер отвечает Award Statistics: только
// ненулевое, а потом — только изменившееся.
//
// Номера в пакете — номера в реестрах: вид статистики (mined 0 … custom 8) и
// номер самой статистики в её реестре. Порядок общих статистик сверен по
// чёрному ящику (jump 23, drop 24, damage_taken 28, deaths 32 и др.).

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use crate::network::varint::encode_varint;

/// Каталог файлов статистики.
pub const DIRECTORY: &str = "world/players/stats";

/// Версия данных, как её пишет оригинал 26.1.2 в файлы игроков.
pub const DATA_VERSION: i32 = 4790;

/// Award Statistics (minecraft-data 26.1: statistics).
pub const AWARD_STATS: i32 = 0x03;

/// Вид статистики; номер — номер в реестре видов (чёрный ящик).
#[derive(Clone, Copy, PartialEq, Eq, Debug, PartialOrd, Ord, Hash)]
pub enum Cat {
    Mined = 0,
    Crafted = 1,
    Used = 2,
    Broken = 3,
    PickedUp = 4,
    Dropped = 5,
    Killed = 6,
    KilledBy = 7,
    Custom = 8,
}

impl Cat {
    pub const ALL: [Cat; 9] = [Cat::Mined, Cat::Crafted, Cat::Used, Cat::Broken, Cat::PickedUp, Cat::Dropped, Cat::Killed, Cat::KilledBy, Cat::Custom];

    pub fn name(self) -> &'static str {
        match self {
            Cat::Mined => "mined",
            Cat::Crafted => "crafted",
            Cat::Used => "used",
            Cat::Broken => "broken",
            Cat::PickedUp => "picked_up",
            Cat::Dropped => "dropped",
            Cat::Killed => "killed",
            Cat::KilledBy => "killed_by",
            Cat::Custom => "custom",
        }
    }

    pub fn by_name(name: &str) -> Option<Cat> {
        Cat::ALL.iter().copied().find(|cat| cat.name() == name)
    }
}

/// Общие статистики по номерам реестра. Порядок снят с чёрного ящика:
/// 1 play_time, 2 total_world_time, 3 time_since_death, 4 time_since_rest,
/// 5 sneak_time, 6 walk, 9 walk_on_water, 10 fall, 12 fly, 19 aviate (!),
/// 23 jump, 24 drop, 25 damage_dealt, 28 damage_taken, 29 shield, 32 deaths,
/// 33 mob_kills, 51 tune_noteblock, 53 trapped chest, 55 enchant_item,
/// 59 open_chest, 62 open_barrel, 65 lectern, 73 anvil, 74 grindstone.
/// Остальное — по порядку вокруг этих опор. Места happy_ghast/nautilus
/// между 17 и 22 — [проверить]: номера 18 и 22 из них двоих не разведены.
pub const CUSTOM: &[&str] = &[
    "leave_game",
    "play_time",
    "total_world_time",
    "time_since_death",
    "time_since_rest",
    "sneak_time",
    "walk_one_cm",
    "crouch_one_cm",
    "sprint_one_cm",
    "walk_on_water_one_cm",
    "fall_one_cm",
    "climb_one_cm",
    "fly_one_cm",
    "walk_under_water_one_cm",
    "minecart_one_cm",
    "boat_one_cm",
    "pig_one_cm",
    "horse_one_cm",
    "happy_ghast_one_cm",
    "aviate_one_cm",
    "swim_one_cm",
    "strider_one_cm",
    "nautilus_one_cm",
    "jump",
    "drop",
    "damage_dealt",
    "damage_dealt_absorbed",
    "damage_dealt_resisted",
    "damage_taken",
    "damage_blocked_by_shield",
    "damage_absorbed",
    "damage_resisted",
    "deaths",
    "mob_kills",
    "animals_bred",
    "player_kills",
    "fish_caught",
    "talked_to_villager",
    "traded_with_villager",
    "eat_cake_slice",
    "fill_cauldron",
    "use_cauldron",
    "clean_armor",
    "clean_banner",
    "clean_shulker_box",
    "interact_with_brewingstand",
    "interact_with_beacon",
    "inspect_dropper",
    "inspect_hopper",
    "inspect_dispenser",
    "play_noteblock",
    "tune_noteblock",
    "pot_flower",
    "trigger_trapped_chest",
    "open_enderchest",
    "enchant_item",
    "play_record",
    "interact_with_furnace",
    "interact_with_crafting_table",
    "open_chest",
    "sleep_in_bed",
    "open_shulker_box",
    "open_barrel",
    "interact_with_blast_furnace",
    "interact_with_smoker",
    "interact_with_lectern",
    "interact_with_campfire",
    "interact_with_cartography_table",
    "interact_with_loom",
    "interact_with_stonecutter",
    "bell_ring",
    "raid_trigger",
    "raid_win",
    "interact_with_anvil",
    "interact_with_grindstone",
    "target_hit",
    "interact_with_smithing_table",
];

/// Номер общей статистики в реестре.
pub fn custom_id(name: &str) -> Option<i32> {
    CUSTOM.iter().position(|custom| *custom == name).map(|at| at as i32)
}

/// Номер сущности по имени: для killed / killed_by.
fn entity_id(name: &str) -> Option<i32> {
    if name == "player" {
        return Some(155);
    }

    crate::mob::Kind::from_name(name).map(|kind| kind.java_type())
}

/// Номер статистики в реестре её вида; None — имя неизвестно.
pub fn stat_id(cat: Cat, name: &str) -> Option<i32> {
    match cat {
        Cat::Custom => custom_id(name),
        Cat::Mined => crate::blocks::block_id(name),
        Cat::Killed | Cat::KilledBy => entity_id(name),
        _ => crate::blocks::item_named(name),
    }
}

/// Счётчики одного игрока.
#[derive(Default, Debug, Clone)]
pub struct Stats {
    values: BTreeMap<(Cat, String), i32>,

    /// Что клиенту уже послано: `None` — ничего (первый ответ — всё).
    sent: Option<HashMap<(Cat, String), i32>>,

    /// Что с прошлого сохранения поменялось.
    dirty: bool,

    /// Значения, при которых счёт табло по такому условию был выровнен
    /// (`sync_board`): счёт идёт приращениями с момента появления цели.
    scored: HashMap<String, i32>,
}

impl Stats {
    pub fn new() -> Self {
        Stats::default()
    }

    /// Прибавить; `name` — без пространства имён.
    pub fn add(&mut self, cat: Cat, name: &str, by: i32) {
        if by == 0 {
            return;
        }

        let value = self.values.entry((cat, name.to_string())).or_insert(0);
        *value = value.saturating_add(by);
        self.dirty = true;
    }

    pub fn custom(&mut self, name: &str, by: i32) {
        debug_assert!(custom_id(name).is_some(), "неизвестная статистика {name}");
        self.add(Cat::Custom, name, by);
    }

    /// Задать значение (время с последней смерти и сна обнуляются).
    pub fn set_custom(&mut self, name: &str, value: i32) {
        debug_assert!(custom_id(name).is_some(), "неизвестная статистика {name}");
        self.values.insert((Cat::Custom, name.to_string()), value);
        self.dirty = true;
    }

    pub fn get(&self, cat: Cat, name: &str) -> i32 {
        self.values.get(&(cat, name.to_string())).copied().unwrap_or(0)
    }

    /// Файл в виде оригинала: `{"stats": {"minecraft:custom": {…}}, "DataVersion": N}`.
    pub fn to_json(&self) -> String {
        let mut kinds = serde_json::Map::new();

        for cat in Cat::ALL {
            let mut group = serde_json::Map::new();

            for ((have, name), value) in &self.values {
                if *have == cat {
                    group.insert(format!("minecraft:{name}"), serde_json::Value::from(*value));
                }
            }

            if !group.is_empty() {
                kinds.insert(format!("minecraft:{}", cat.name()), serde_json::Value::Object(group));
            }
        }

        let mut root = serde_json::Map::new();
        root.insert("stats".into(), serde_json::Value::Object(kinds));
        root.insert("DataVersion".into(), serde_json::Value::from(DATA_VERSION));

        serde_json::to_string_pretty(&serde_json::Value::Object(root)).unwrap_or_default()
    }

    /// Прочитать файл; чужое и неизвестное пропускается.
    pub fn from_json(text: &str) -> Option<Stats> {
        let root: serde_json::Value = serde_json::from_str(text).ok()?;
        let mut stats = Stats::new();

        for (kind, group) in root.get("stats")?.as_object()? {
            let Some(cat) = kind.strip_prefix("minecraft:").and_then(Cat::by_name) else {
                continue;
            };

            for (name, value) in group.as_object()? {
                if let (Some(name), Some(value)) = (name.strip_prefix("minecraft:"), value.as_i64()) {
                    stats.values.insert((cat, name.to_string()), value.clamp(i32::MIN as i64, i32::MAX as i64) as i32);
                }
            }
        }

        Some(stats)
    }

    pub fn path_for(uuid: &[u8; 16]) -> PathBuf {
        Path::new(DIRECTORY).join(format!("{}.json", crate::network::login::format_uuid(uuid)))
    }

    pub fn load(uuid: &[u8; 16]) -> Stats {
        std::fs::read_to_string(Stats::path_for(uuid)).ok().and_then(|text| Stats::from_json(&text)).unwrap_or_default()
    }

    /// Записать, если что-то менялось.
    pub fn save(&mut self, uuid: &[u8; 16]) {
        if !self.dirty {
            return;
        }

        let path = Stats::path_for(uuid);

        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let tmp = path.with_extension("json.tmp");

        if std::fs::write(&tmp, self.to_json()).is_ok() && std::fs::rename(&tmp, &path).is_ok() {
            self.dirty = false;
        }
    }

    pub fn has_updates(&self) -> bool {
        self.sent.as_ref().is_none_or(|sent| self.values.iter().any(|(key, value)| sent.get(key) != Some(value)))
    }

    /// Тело пакета Award Statistics: первый раз — всё ненулевое, потом —
    /// только то, что изменилось с прошлого ответа (вики: «Will only send
    /// the changed values if previously requested»).
    pub fn award_body(&mut self) -> Vec<u8> {
        let before = self.sent.take().unwrap_or_default();
        let mut entries: Vec<(i32, i32, i32)> = Vec::new();

        for ((cat, name), value) in &self.values {
            if before.get(&(*cat, name.clone())) == Some(value) {
                continue;
            }

            if let Some(id) = stat_id(*cat, name) {
                entries.push((*cat as i32, id, *value));
            }
        }

        self.sent = Some(self.values.clone().into_iter().collect());

        let mut body = encode_varint(AWARD_STATS);
        body.extend(encode_varint(entries.len() as i32));

        for (cat, id, value) in entries {
            body.extend(encode_varint(cat));
            body.extend(encode_varint(id));
            body.extend(encode_varint(value));
        }

        body
    }

    /// Привести счета табло по условиям-статистикам (`minecraft.custom:minecraft.jump`)
    /// в соответствие: прибавить к ним то, что накопилось с прошлого раза.
    pub fn sync_board(&mut self, name: &str, board: &mut crate::scoreboard::Scoreboard) {
        for criteria in board.stat_criteria() {
            let Some((kind, what)) = criteria.split_once(':') else {
                continue;
            };
            let kind = kind.strip_prefix("minecraft.").unwrap_or(kind);
            let what = what.strip_prefix("minecraft.").unwrap_or(what);
            let Some(cat) = Cat::by_name(kind) else {
                continue;
            };
            let value = self.get(cat, what);

            match self.scored.insert(criteria.clone(), value) {
                Some(last) if value != last => board.bump_criteria(name, &criteria, value.wrapping_sub(last)),
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_has_the_shape_of_the_original() {
        let mut stats = Stats::new();
        stats.add(Cat::Mined, "oak_log", 1);
        stats.custom("walk_one_cm", 100);

        let text = stats.to_json();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();

        assert_eq!(value["stats"]["minecraft:mined"]["minecraft:oak_log"], 1);
        assert_eq!(value["stats"]["minecraft:custom"]["minecraft:walk_one_cm"], 100);
        assert_eq!(value["DataVersion"], DATA_VERSION);

        let back = Stats::from_json(&text).unwrap();
        assert_eq!(back.get(Cat::Mined, "oak_log"), 1);
        assert_eq!(back.get(Cat::Custom, "walk_one_cm"), 100);
    }

    /// Номера, снятые с чёрного ящика.
    #[test]
    fn custom_ids_match_the_original() {
        for (name, id) in [
            ("play_time", 1),
            ("total_world_time", 2),
            ("time_since_death", 3),
            ("time_since_rest", 4),
            ("sneak_time", 5),
            ("walk_one_cm", 6),
            ("walk_on_water_one_cm", 9),
            ("fall_one_cm", 10),
            ("fly_one_cm", 12),
            ("aviate_one_cm", 19),
            ("jump", 23),
            ("drop", 24),
            ("damage_dealt", 25),
            ("damage_taken", 28),
            ("damage_blocked_by_shield", 29),
            ("deaths", 32),
            ("mob_kills", 33),
            ("tune_noteblock", 51),
            ("trigger_trapped_chest", 53),
            ("enchant_item", 55),
            ("open_chest", 59),
            ("open_barrel", 62),
            ("interact_with_lectern", 65),
            ("interact_with_anvil", 73),
            ("interact_with_grindstone", 74),
        ] {
            assert_eq!(custom_id(name), Some(id), "{name}");
        }
    }

    #[test]
    fn award_sends_everything_then_only_changes() {
        let mut stats = Stats::new();
        stats.custom("jump", 2);
        stats.add(Cat::Dropped, "cobblestone", 2);

        let first = stats.award_body();
        // Пакет, число записей (2), по три числа на каждую.
        assert_eq!(first[0], AWARD_STATS as u8);
        assert_eq!(first[1], 2);

        let same = stats.award_body();
        assert_eq!(same[1], 0);

        stats.custom("jump", 1);
        let changed = stats.award_body();
        assert_eq!(&changed[1..], &[1, 8, 23, 3]);
    }
}
