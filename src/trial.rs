// Испытательный спавнер и хранилище (вики, Trial Spawner, Vault): данные
// блок-сущностей, такт, награды.
//
// Кода игры здесь нет: правила — со страниц minecraft.wiki и из чёрного
// ящика (сценарии trial-spawner-wave, vault-open). Что не сверено,
// помечено «[проверить]».
//
// Испытательный спавнер (вики, Trial Spawner):
// - состояние — свойство блока `trial_spawner_state`: inactive (моб не
//   задан), waiting_for_players, active, waiting_for_reward_ejection,
//   ejecting_reward, cooldown;
// - замечает игроков в выживании и приключении в 14 блоках при прямой
//   видимости; волна: всего `total_mobs + per_player·(игроки−1)`, разом
//   `simultaneous_mobs + …`, раз в `ticks_between_spawn`;
// - все поставленные мобы побеждены — через 40 тактов награда: по предмету
//   на каждого замеченного игрока раз в 30 тактов; потом остывание 36000
//   тактов (30 минут);
// - игрок со «Знамением испытаний» (Trial Omen) делает спавнер зловещим.
//
// Хранилище (вики, Vault):
// - `vault_state`: inactive, active (рядом игрок, ещё не получавший
//   награды), unlocking, ejecting;
// - ключ испытаний открывает его один раз на игрока (список до 128
//   игроков, старейший забывается), награда выбрасывается по стопке.

use crate::inventory::Stack;
use crate::loot::{Dice, Pool};

/// Номера видов блок-сущностей (реестр `minecraft:block_entity_type`,
/// tools/blackbox/data/registries.json: trial_spawner 44, vault 45).
pub const SPAWNER_ENTITY: i32 = 44;
pub const VAULT_ENTITY: i32 = 45;

/// Дальность, на которой спавнер замечает игроков (вики: 14).
pub const PLAYER_RANGE: f64 = 14.0;
/// Остывание после награды (вики: 36000 тактов).
pub const COOLDOWN: u64 = 36_000;
/// Пауза перед наградой и между её предметами (вики: 40 и 30 тактов).
pub const REWARD_DELAY: u64 = 40;
pub const EJECT_EVERY: u64 = 30;

/// Хранилище замечает игрока в 4 блоках и забывает дальше 4,5 (вики, Vault:
/// `activation_range` 4, `deactivation_range` 4,5).
pub const VAULT_ACTIVATION: f64 = 4.0;
pub const VAULT_DEACTIVATION: f64 = 4.5;
/// Сколько игроков помнит хранилище (вики: 128).
pub const VAULT_MEMORY: usize = 128;

/// События мира (вики, Java Edition protocol, World Event): спавнер
/// заметил игрока 3013, поставил моба 3011 (у блока) и 3012 (у моба),
/// выбросил награду 3014; хранилище ожило 3015, уснуло 3016, выбросило 3017;
/// [проверить номера по чёрному ящику].
pub const EVENT_SPAWN_AT_SPAWNER: i32 = 3011;
pub const EVENT_SPAWN_AT_MOB: i32 = 3012;
pub const EVENT_DETECT_PLAYER: i32 = 3013;
pub const EVENT_EJECT_ITEM: i32 = 3014;
pub const EVENT_VAULT_ACTIVATE: i32 = 3015;
pub const EVENT_VAULT_DEACTIVATE: i32 = 3016;
pub const EVENT_VAULT_EJECT: i32 = 3017;

/// Что ставит спавнер в одной волне (вики, Trial Spawner, «spawner config»).
#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    /// Вид моба (`minecraft:zombie`) и его флаги из `spawn_potentials`.
    pub entity: String,
    pub no_ai: bool,
    pub silent: bool,
    /// Без снаряжения: заданный рукой `spawn_potentials` без `equipment` (чёрный ящик, ominous-spawner).
    pub bare: bool,
    pub total_mobs: f32,
    pub simultaneous_mobs: f32,
    pub total_per_player: f32,
    pub simultaneous_per_player: f32,
    pub ticks_between_spawn: u32,
    pub spawn_range: i32,
    /// Таблица награды (`loot_tables_to_eject`, один выбор на награду).
    pub rewards: Vec<(String, u32)>,
}

impl Default for Config {
    /// Поставленный рукой спавнер (вики: «Default» — 6 / +2, 2 / +1, 40 тактов).
    fn default() -> Self {
        Self {
            entity: String::new(),
            no_ai: false,
            silent: false,
            bare: false,
            total_mobs: 6.0,
            simultaneous_mobs: 2.0,
            total_per_player: 2.0,
            simultaneous_per_player: 1.0,
            ticks_between_spawn: 40,
            spawn_range: 4,
            rewards: vec![("minecraft:spawners/trial_chamber/consumables".into(), 1), ("minecraft:spawners/trial_chamber/key".into(), 1)],
        }
    }
}

impl Config {
    /// Сколько всего и сколько разом при `players` замеченных игроках.
    pub fn totals(&self, players: usize) -> (u32, u32) {
        let extra = players.saturating_sub(1) as f32;

        ((self.total_mobs + self.total_per_player * extra).floor() as u32, (self.simultaneous_mobs + self.simultaneous_per_player * extra).floor() as u32)
    }

    fn apply(&mut self, value: &crate::snbt::Value) {
        let number = |key: &str| value.get(key).and_then(crate::snbt::Value::as_f64);

        if let Some(first) = value.get("spawn_potentials").and_then(|list| match list {
            crate::snbt::Value::List(items) => items.first(),
            _ => None,
        }) && let Some(entity) = first.get("data").and_then(|data| data.get("entity"))
        {
            self.read_entity(entity);
        }

        for (key, slot) in [
            ("total_mobs", &mut self.total_mobs),
            ("simultaneous_mobs", &mut self.simultaneous_mobs),
            ("total_mobs_added_per_player", &mut self.total_per_player),
            ("simultaneous_mobs_added_per_player", &mut self.simultaneous_per_player),
        ] {
            if let Some(new) = number(key) {
                *slot = new as f32;
            }
        }

        if let Some(ticks) = number("ticks_between_spawn") {
            self.ticks_between_spawn = ticks.max(0.0) as u32;
        }

        if let Some(range) = number("spawn_range") {
            self.spawn_range = range as i32;
        }

        if let Some(crate::snbt::Value::List(tables)) = value.get("loot_tables_to_eject") {
            self.rewards = tables
                .iter()
                .filter_map(|entry| Some((entry.get("data")?.as_str()?.to_string(), entry.get("weight").and_then(crate::snbt::Value::as_f64).unwrap_or(1.0) as u32)))
                .collect();
        }
    }

    fn read_entity(&mut self, entity: &crate::snbt::Value) {
        if let Some(id) = entity.get("id").and_then(crate::snbt::Value::as_str) {
            self.entity = if id.contains(':') { id.to_string() } else { format!("minecraft:{id}") };
        }

        self.no_ai = entity.get("NoAI").and_then(crate::snbt::Value::as_f64).is_some_and(|v| v != 0.0);
        self.silent = entity.get("Silent").and_then(crate::snbt::Value::as_f64).is_some_and(|v| v != 0.0);
        self.bare = true;
    }

    /// Вид моба, если он нам знаком.
    pub fn kind(&self) -> Option<crate::mob::Kind> {
        crate::mob::Kind::from_name(&self.entity)
    }
}

/// Блок-сущность испытательного спавнера.
#[derive(Clone, Debug, PartialEq)]
pub struct Spawner {
    pub normal: Config,
    pub ominous: Config,
    pub required_range: f64,
    pub cooldown_length: u64,
    /// Замеченные игроки (номера сущностей) — по ним волна и награда.
    pub players: Vec<i32>,
    /// Живые мобы этой волны (номера сущностей).
    pub mobs: Vec<i32>,
    pub total_spawned: u32,
    pub next_spawn_at: u64,
    pub cooldown_end_at: u64,
    /// Когда следующий шаг награды (пауза, предмет).
    pub eject_at: u64,
    /// Сколько предметов награды ещё выбросить и из какой таблицы.
    pub to_eject: u32,
    pub reward_table: String,
    /// Моб выбран (спавнер ушёл из «inactive»): только тогда клиент видит
    /// `spawn_data` (trial-spawner-wave: при постановке данные `{}`).
    pub picked: bool,
    pub next_item_at:u64,
}

impl Default for Spawner {
    fn default() -> Self {
        Self {
            normal: Config::default(),
            ominous: Config {rewards:ominous_rewards(),..Config::default()},
            required_range: PLAYER_RANGE,
            cooldown_length: COOLDOWN,
            players: Vec::new(),
            mobs: Vec::new(),
            total_spawned: 0,
            next_spawn_at: 0,
            cooldown_end_at: 0,
            eject_at: 0,
            to_eject: 0,
            reward_table: String::new(),
            picked: false,
            next_item_at:0,
        }
    }
}

impl Spawner {
    /// Поля из SNBT команды `setblock … trial_spawner{…}`: `normal_config`,
    /// `ominous_config`, `required_player_range`, `target_cooldown_length`.
    /// Ссылка на готовую настройку (`"minecraft:trial_chamber/breeze"`) —
    /// по таблице `preset`.
    pub fn apply_snbt(&mut self, text: &str) -> Result<(), String> {
        let value = crate::snbt::parse(text).ok_or_else(|| format!("Не понял данные блока: {text}"))?;

        for (key, config) in [("normal_config", &mut self.normal), ("ominous_config", &mut self.ominous)] {
            match value.get(key) {
                Some(crate::snbt::Value::Str(id)) => {
                    if let Some(found) = preset(id) {
                        *config = found;
                    }
                }
                Some(inner) => config.apply(inner),
                None => {}
            }
        }

        if let Some(range) = value.get("required_player_range").and_then(crate::snbt::Value::as_f64) {
            self.required_range = range.clamp(1.0, 128.0);
        }

        if let Some(length) = value.get("target_cooldown_length").and_then(crate::snbt::Value::as_f64) {
            self.cooldown_length = length.max(0.0) as u64;
        }

        Ok(())
    }

    /// Настройка по виду блока: зловещий берёт свою.
    pub fn config(&self, ominous: bool) -> &Config {
        if ominous { &self.ominous } else { &self.normal }
    }

    /// NBT для клиента: только `spawn_data` с мобом — по нему клиент крутит
    /// зверька внутри (чёрный ящик, trial-spawner-wave: `{}` у пустого,
    /// `{"spawn_data":{"entity":{"Silent":1,"id":…,"NoAI":1}}}` — поля в
    /// порядке хеш-таблицы Java).
    pub fn nbt(&self, ominous: bool) -> Vec<u8> {
        let config = self.config(ominous);
        let mut out = vec![0x0A];

        if self.picked && !config.entity.is_empty() {
            name(&mut out, 0x0A, "spawn_data");
            name(&mut out, 0x0A, "entity");

            let mut keys: Vec<&str> = vec!["id"];

            if config.no_ai {
                keys.push("NoAI");
            }

            if config.silent {
                keys.push("Silent");
            }

            for index in crate::scoreboard::java_order(&keys) {
                match keys[index] {
                    "id" => {
                        name(&mut out, 0x08, "id");
                        string(&mut out, &config.entity);
                    }
                    flag => {
                        name(&mut out, 0x01, flag);
                        out.push(1);
                    }
                }
            }

            out.extend_from_slice(&[0, 0]);
        }

        out.push(0);
        out
    }

    /// Запись для файла региона: место, настройки, состояние волны.
    pub fn write(&self, place: (i32, i32, i32), out: &mut Vec<u8>) {
        for value in [place.0, place.1, place.2] {
            out.extend_from_slice(&value.to_be_bytes());
        }

        for config in [&self.normal, &self.ominous] {
            write_config(config, out);
        }

        out.extend_from_slice(&self.required_range.to_be_bytes());
        out.extend_from_slice(&self.cooldown_length.to_be_bytes());
        out.extend_from_slice(&self.cooldown_end_at.to_be_bytes());
    }

    /// Читает запись `write`; None — она испорчена. Волна, шедшая при
    /// выгрузке, не продолжается: её мобы и игроки — номера сущностей.
    pub fn read(bytes: &mut &[u8]) -> Option<((i32, i32, i32), Spawner)> {
        let place = (take_i32(bytes)?, take_i32(bytes)?, take_i32(bytes)?);
        let normal = read_config(bytes)?;
        let ominous = read_config(bytes)?;
        let required_range = f64::from_be_bytes(take(bytes, 8)?.try_into().ok()?);
        let cooldown_length = u64::from_be_bytes(take(bytes, 8)?.try_into().ok()?);
        let cooldown_end_at = u64::from_be_bytes(take(bytes, 8)?.try_into().ok()?);

        Some((place, Spawner { normal, ominous, required_range, cooldown_length, cooldown_end_at, ..Spawner::default() }))
    }
}

/// Готовые настройки спавнеров испытаний (вики, Trial Spawner, таблица
/// настроек): breeze — 20 тактов, 2 / +1, 1 / +0,5; прочие — 20 тактов,
/// 6 / +2, 3 / +0,5 [проверить: ближние, дальние, малые; у зловещих —
/// по вики не расписаны].
pub fn preset(id: &str) -> Option<Config> {
    let id = id.strip_prefix("minecraft:").unwrap_or(id);
    let rest = id.strip_prefix("trial_chamber/")?;
    let (group, variant) = rest.split_once('/').unwrap_or((rest, "normal"));
    let variant = variant.split('/').next().unwrap_or(variant);
    let mob = match group {
        "breeze" => "breeze",
        "melee" => match variant {
            "husk" => "husk",
            "spider" => "spider",
            _ => "zombie",
        },
        "small_melee" => match variant {
            "slime" => "slime",
            "cave_spider" => "cave_spider",
            "silverfish" => "silverfish",
            _ => "zombie",
        },
        "ranged" => match variant {
            "stray" => "stray",
            "poison_skeleton" => "bogged",
            _ => "skeleton",
        },
        "slow_ranged" => match variant {
            "stray" => "stray",
            "poison_skeleton" => "bogged",
            _ => "skeleton",
        },
        _ => return None,
    };
    let base = Config { rewards:if id.ends_with("/ominous") {ominous_rewards()} else {Config::default().rewards}, entity: format!("minecraft:{mob}"), ticks_between_spawn: 20, simultaneous_mobs: 3.0, simultaneous_per_player: 0.5, ..Config::default() };

    let mut config = match group {
        "breeze" => Config { total_mobs: 2.0, total_per_player: 1.0, simultaneous_mobs: 1.0, simultaneous_per_player: 0.5, ..base },
        "slow_ranged" => Config { ticks_between_spawn: 160, simultaneous_mobs: 4.0, simultaneous_per_player: 2.0, ..base },
        _ => base,
    };
    // Вики, Ominous Trial Spawner: без снаряжения — вдвое больше мобов
    // за волну и ещё один одновременно. [проверить] прибавка за игрока.
    if id.ends_with("/ominous") && matches!(mob,"spider"|"cave_spider"|"silverfish"|"slime"|"breeze") {
        config.total_mobs *= 2.0; config.total_per_player *= 2.0; config.simultaneous_mobs += 1.0;
    }
    Some(config)
}

fn write_config(config: &Config, out: &mut Vec<u8>) {
    let text = |out: &mut Vec<u8>, value: &str| {
        out.push(value.len().min(255) as u8);
        out.extend_from_slice(&value.as_bytes()[..value.len().min(255)]);
    };

    text(out, &config.entity);
    out.push(u8::from(config.no_ai) | (u8::from(config.silent) << 1) | (u8::from(config.bare) << 2));

    for value in [config.total_mobs, config.simultaneous_mobs, config.total_per_player, config.simultaneous_per_player] {
        out.extend_from_slice(&value.to_be_bytes());
    }

    out.extend_from_slice(&config.ticks_between_spawn.to_be_bytes());
    out.extend_from_slice(&config.spawn_range.to_be_bytes());
    out.push(config.rewards.len().min(255) as u8);

    for (table, weight) in config.rewards.iter().take(255) {
        text(out, table);
        out.extend_from_slice(&weight.to_be_bytes());
    }
}

fn read_config(bytes: &mut &[u8]) -> Option<Config> {
    let text = |bytes: &mut &[u8]| -> Option<String> {
        let length = usize::from(take(bytes, 1)?[0]);

        String::from_utf8(take(bytes, length)?.to_vec()).ok()
    };
    let float = |bytes: &mut &[u8]| -> Option<f32> { Some(f32::from_be_bytes(take(bytes, 4)?.try_into().ok()?)) };
    let entity = text(bytes)?;
    let flags = take(bytes, 1)?[0];
    let (total_mobs, simultaneous_mobs, total_per_player, simultaneous_per_player) = (float(bytes)?, float(bytes)?, float(bytes)?, float(bytes)?);
    let ticks_between_spawn = u32::from_be_bytes(take(bytes, 4)?.try_into().ok()?);
    let spawn_range = take_i32(bytes)?;
    let count = take(bytes, 1)?[0];
    let mut rewards = Vec::new();

    for _ in 0..count {
        let table = text(bytes)?;

        rewards.push((table, u32::from_be_bytes(take(bytes, 4)?.try_into().ok()?)));
    }

    Some(Config { entity, no_ai: flags & 1 != 0, silent: flags & 2 != 0, bare: flags & 4 != 0, total_mobs, simultaneous_mobs, total_per_player, simultaneous_per_player, ticks_between_spawn, spawn_range, rewards })
}

/// Блок-сущность хранилища.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Vault {
    /// Своя таблица награды (`config.loot_table`); пусто — по виду хранилища.
    pub loot: String,
    /// Уже награждённые игроки (UUID), старейший первым.
    pub rewarded: Vec<[u8; 16]>,
    /// Рядом стоящие ненаграждённые игроки: их видит клиент (частицы).
    pub connected: Vec<[u8; 16]>,
    /// Что осталось выбросить и когда следующий шаг.
    pub to_eject: Vec<Stack>,
    pub ejected: u32,
    pub step_at: u64,
    /// Ключ только что вставлен: в такте хранилища блок и данные
    /// повторяются ещё раз (vault-open: на +1 после щелчка).
    pub echo: bool,
}

impl Vault {
    /// Поля из SNBT команды: `config:{loot_table:"…"}`.
    pub fn apply_snbt(&mut self, text: &str) -> Result<(), String> {
        let value = crate::snbt::parse(text).ok_or_else(|| format!("Не понял данные блока: {text}"))?;

        if let Some(table) = value.get("config").and_then(|config| config.get("loot_table")).and_then(crate::snbt::Value::as_str) {
            self.loot = table.to_string();
        }

        Ok(())
    }

    /// Таблица награды: своя или по виду (вики: reward и reward_ominous).
    pub fn table(&self, ominous: bool) -> String {
        if !self.loot.is_empty() {
            return self.loot.clone();
        }

        if ominous { "minecraft:chests/trial_chambers/reward_ominous".into() } else { "minecraft:chests/trial_chambers/reward".into() }
    }

    /// Награждён ли игрок; запомнить нового (до 128, старейший забывается).
    pub fn rewarded(&self, uuid: &[u8; 16]) -> bool {
        self.rewarded.contains(uuid)
    }

    pub fn remember(&mut self, uuid: [u8; 16]) {
        if self.rewarded.len() >= VAULT_MEMORY {
            self.rewarded.remove(0);
        }

        self.rewarded.push(uuid);
    }

    /// NBT для клиента: `shared_data` с подошедшими игроками (чёрный ящик,
    /// vault-open: `{"shared_data":{}}`, потом `connected_players` — список
    /// UUID массивами из четырёх целых).
    pub fn nbt(&self) -> Vec<u8> {
        let mut out = vec![0x0A];

        name(&mut out, 0x0A, "shared_data");

        // Что выйдет следующим — у открывающегося (vault-open: «display_item»
        // раньше подошедших — порядок хеш-таблицы Java) [проверить: у ожившего
        // оригинал листает случайные предметы таблицы, у нас пусто].
        if let Some(next) = self.to_eject.first()
            && let Some(item) = crate::blocks::item_name(next.item)
        {
            name(&mut out, 0x0A, "display_item");
            name(&mut out, 0x03, "count");
            out.extend_from_slice(&next.count.to_be_bytes());
            name(&mut out, 0x08, "id");
            string(&mut out, &format!("minecraft:{item}"));
            out.push(0);
        }

        if !self.connected.is_empty() {
            name(&mut out, 0x09, "connected_players");
            out.push(0x0B);
            out.extend_from_slice(&(self.connected.len() as i32).to_be_bytes());

            for uuid in &self.connected {
                out.extend_from_slice(&4i32.to_be_bytes());
                out.extend_from_slice(uuid);
            }
        }

        out.extend_from_slice(&[0, 0]);
        out
    }

    pub fn write(&self, place: (i32, i32, i32), out: &mut Vec<u8>) {
        for value in [place.0, place.1, place.2] {
            out.extend_from_slice(&value.to_be_bytes());
        }

        out.push(self.loot.len().min(255) as u8);
        out.extend_from_slice(&self.loot.as_bytes()[..self.loot.len().min(255)]);
        out.push(self.rewarded.len().min(VAULT_MEMORY) as u8);

        for uuid in self.rewarded.iter().take(VAULT_MEMORY) {
            out.extend_from_slice(uuid);
        }
    }

    pub fn read(bytes: &mut &[u8]) -> Option<((i32, i32, i32), Vault)> {
        let place = (take_i32(bytes)?, take_i32(bytes)?, take_i32(bytes)?);
        let length = usize::from(take(bytes, 1)?[0]);
        let loot = String::from_utf8(take(bytes, length)?.to_vec()).ok()?;
        let count = usize::from(take(bytes, 1)?[0]);
        let mut rewarded = Vec::with_capacity(count);

        for _ in 0..count {
            rewarded.push(take(bytes, 16)?.try_into().ok()?);
        }

        Some((place, Vault { loot, rewarded, ..Vault::default() }))
    }
}

/// Блок-сущность испытаний в чанке: спавнер или хранилище.
#[derive(Clone, Debug, PartialEq)]
pub enum Data {
    Spawner(Box<Spawner>),
    Vault(Box<Vault>),
}

impl Data {
    /// Новая запись по имени блока; None — блок не из этих.
    pub fn for_block(name: &str) -> Option<Data> {
        match name {
            "trial_spawner" => Some(Data::Spawner(Box::default())),
            "vault" => Some(Data::Vault(Box::default())),
            _ => None,
        }
    }

    /// Запись сгенерированной камеры (world/trial_chambers.rs): спавнер —
    /// «обычная|зловещая» настройки, хранилище — таблица награды.
    pub fn generated(kind: &str, data: &str) -> Option<Data> {
        match kind {
            "spawner" => {
                let (normal, ominous) = data.split_once('|').unwrap_or((data, data));

                Some(Data::Spawner(Box::new(Spawner { normal: preset(normal)?, ominous: preset(ominous).or_else(|| preset(normal))?, ..Spawner::default() })))
            }
            "vault" => Some(Data::Vault(Box::new(Vault { loot: if data.is_empty() { String::new() } else if data.contains(':') { data.to_string() } else { format!("minecraft:{data}") }, ..Vault::default() }))),
            _ => None,
        }
    }

    pub fn write(&self, place: (i32, i32, i32), out: &mut Vec<u8>) {
        match self {
            Data::Spawner(spawner) => {
                out.push(0);
                spawner.write(place, out);
            }
            Data::Vault(vault) => {
                out.push(1);
                vault.write(place, out);
            }
        }
    }

    pub fn read(bytes: &mut &[u8]) -> Option<((i32, i32, i32), Data)> {
        match take(bytes, 1)?[0] {
            0 => Spawner::read(bytes).map(|(place, spawner)| (place, Data::Spawner(Box::new(spawner)))),
            1 => Vault::read(bytes).map(|(place, vault)| (place, Data::Vault(Box::new(vault)))),
            _ => None,
        }
    }

    /// NBT для клиента и номер вида блок-сущности.
    pub fn nbt(&self, ominous: bool) -> (i32, Vec<u8>) {
        match self {
            Data::Spawner(spawner) => (SPAWNER_ENTITY, spawner.nbt(ominous)),
            Data::Vault(vault) => (VAULT_ENTITY, vault.nbt()),
        }
    }
}

fn take<'a>(bytes: &mut &'a [u8], count: usize) -> Option<&'a [u8]> {
    let (head, rest) = bytes.split_at_checked(count)?;

    *bytes = rest;
    Some(head)
}

fn take_i32(bytes: &mut &[u8]) -> Option<i32> {
    Some(i32::from_be_bytes(take(bytes, 4)?.try_into().ok()?))
}

fn name(out: &mut Vec<u8>, tag: u8, key: &str) {
    out.push(tag);
    string(out, key);
}

fn string(out: &mut Vec<u8>, value: &str) {
    out.extend_from_slice(&(value.len() as u16).to_be_bytes());
    out.extend_from_slice(value.as_bytes());
}

// ---------------------------------------------------------------------------
// Таблицы награды
// ---------------------------------------------------------------------------

/// Награда спавнера: съестное (вики, Trial Spawner, «Loot»: хлеб 1–3 ×3,
/// жареная курица ×3, печёный картофель 1–3 ×2, зелья регенерации и
/// быстроты ×1) [проверить: зелья — пока бутылки воды без эффекта].
const CONSUMABLES: [Pool; 1] = [Pool {
    rolls: (1, 1),
    entries: &[("bread", 1, 3, 3), ("cooked_chicken", 1, 1, 3), ("baked_potato", 1, 3, 2), ("potion", 1, 1, 2)],
    empty: 0,
}];

/// Ключ испытаний (вики: «Trial Key»).
const KEY: [Pool; 1] = [Pool { rolls: (1, 1), entries: &[("trial_key", 1, 1, 1)], empty: 0 }];
const OMINOUS_KEY: [Pool; 1] = [Pool { rolls: (1, 1), entries: &[("ominous_trial_key", 1, 1, 1)], empty: 0 }];

/// Награда хранилища (вики, Vault, «Loot», Java): обычная часть, редкая
/// (1–3 броска) и особая (25 %) [проверить: зачарования предметов и книг
/// обычного хранилища].
const VAULT_REWARD: [Pool; 3] = [
    Pool {
        rolls: (1, 1),
        entries: &[
            ("shield", 1, 1, 300),
            ("arrow", 2, 8, 92),
            ("tipped_arrow", 2, 8, 92),
            ("iron_axe", 1, 1, 200),
            ("wind_charge", 1, 3, 69),
            ("iron_ingot", 1, 4, 69),
            ("crossbow", 1, 1, 200),
            ("diamond", 1, 2, 23),
            ("golden_carrot", 1, 2, 200),
            ("diamond_axe", 1, 1, 100),
            ("bow", 1, 1, 300),
            ("ominous_bottle", 1, 1, 46),
            ("diamond_chestplate", 1, 1, 100),
            ("enchanted_book", 1, 1, 400),
            ("iron_chestplate", 1, 1, 200),
            ("emerald", 2, 4, 392),
            ("wind_charge", 4, 12, 23),
            ("honey_bottle", 1, 2, 69),
        ],
        empty: 0,
    },
    Pool {
        rolls: (1, 3),
        entries: &[
            ("diamond", 1, 2, 1),
            ("arrow", 2, 8, 4),
            ("tipped_arrow", 2, 8, 4),
            ("ominous_bottle", 1, 1, 2),
            ("wind_charge", 4, 12, 1),
            ("wind_charge", 1, 3, 3),
            ("emerald", 2, 4, 4),
            ("honey_bottle", 1, 2, 3),
            ("iron_ingot", 1, 4, 3),
        ],
        empty: 0,
    },
    Pool {
        rolls: (1, 1),
        entries: &[("music_disc_precipice", 1, 1, 2), ("bolt_armor_trim_smithing_template", 1, 1, 3), ("trident", 1, 1, 1), ("golden_apple", 1, 1, 4), ("guster_banner_pattern", 1, 1, 2)],
        empty: 36,
    },
];

// Вики, Ominous Vault, Java loot; [проверить] уровни зачарований.
const OMINOUS_CONSUMABLES:[Pool;1]=[Pool {rolls:(1,1),empty:0,entries:&[
    ("cooked_beef",1,2,3),("baked_potato",2,4,3),("golden_carrot",1,2,2),
    ("potion:regeneration:potion",1,1,1),("potion:strength:potion",1,1,1)]}];
const OMINOUS_REWARD:[Pool;3]=[
    Pool {rolls:(1,1),empty:0,entries:&[
        ("diamond",2,3,58),("emerald",4,10,145),("wind_charge",8,12,116),("potion:strong_slowness:tipped_arrow",4,12,87),("ominous_bottle",1,1,29),
        ("diamond_block",1,1,60),("emerald_block",1,1,300),("iron_block",1,1,240),("golden_apple",1,1,180),
        ("enchant:crossbow",1,1,240),("enchant:diamond_axe",1,1,180),("enchant:diamond_chestplate",1,1,180),
        ("enchanted_book",1,1,120),("breach:enchanted_book",1,1,60),("density:enchanted_book",1,1,60),("wind_burst:enchanted_book",1,1,120)]},
    Pool {rolls:(1,3),empty:0,entries:&[("emerald",4,10,5),("wind_charge",8,12,4),("potion:strong_slowness:tipped_arrow",4,12,3),("diamond",2,3,2),("ominous_bottle",1,1,1)]},
    Pool {rolls:(1,1),empty:10,entries:&[("enchanted_golden_apple",1,1,9),("flow_armor_trim_smithing_template",1,1,9),("flow_banner_pattern",1,1,6),("music_disc_creator",1,1,3),("heavy_core",1,1,3)]},
];
fn ominous_rewards()->Vec<(String,u32)> {
    vec![("minecraft:spawners/ominous/trial_chamber/consumables".into(),7),("minecraft:spawners/ominous/trial_chamber/key".into(),3)]
}

/// Превращение обычного испытания от знамения. Возвращает старую волну,
/// которую надо убрать без добычи. [проверить] обнаружение по тактам.
pub fn make_ominous(world:&mut crate::world::World,at:(f64,f64,f64),now:u64)->Option<Vec<i32>> {
    let places:Vec<_>=world.trial_places().into_iter().filter(|p| {
        let block=world.get_block(p.0,p.1,p.2);
        !ominous_state(block) && matches!(world.trial(p.0,p.1,p.2),Some(Data::Spawner(_)))
            && crate::mob::warden::distance(at,(p.0 as f64+0.5,p.1 as f64+0.5,p.2 as f64+0.5))<PLAYER_RANGE
            && clear(world,(p.0 as f64+0.5,p.1 as f64+0.5,p.2 as f64+0.5),(at.0,at.1+1.62,at.2))
    }).collect();
    if places.is_empty() {return None;}
    let mut gone=Vec::new();
    for p in places {
        world.with_trial(p.0,p.1,p.2,|data|if let Data::Spawner(s)=data {
            gone.append(&mut s.mobs);s.players.clear();s.total_spawned=0;s.cooldown_end_at=0;
            s.next_spawn_at=now+40;s.next_item_at=now+160;
            if s.ominous.entity.is_empty() {s.ominous.entity=s.normal.entity.clone();}
        });
        // Чёрный ящик, ominous-spawner: на такте эффекта события 3020 (у игрока 0, у спавнера 1), на следующем блок уже ominous+active, звука нет.
        world.note_level_event(at.0.floor() as i32,(at.1+1.62).floor() as i32,at.2.floor() as i32,3020,0);world.note_level_event(p.0,p.1,p.2,3020,1);
        set_prop(world,p,"ominous","true");set_prop(world,p,"trial_spawner_state","active");
        note(world,p);
    }
    Some(gone)
}

/// Предметы по ссылке на таблицу; незнакомая таблица — пусто.
pub fn roll_table(table: &str, dice: &mut Dice) -> Vec<Stack> {
    let pools: &[Pool] = match table.strip_prefix("minecraft:").unwrap_or(table) {
        "spawners/trial_chamber/consumables" => &CONSUMABLES,
        "spawners/ominous/trial_chamber/consumables" => &OMINOUS_CONSUMABLES,
        "spawners/trial_chamber/key" => &KEY,
        "spawners/ominous/trial_chamber/key" => &OMINOUS_KEY,
        "chests/trial_chambers/reward" => &VAULT_REWARD,
        "chests/trial_chambers/reward_ominous" => &OMINOUS_REWARD,
        _ => return Vec::new(),
    };

    let mut loot=crate::loot::roll(pools, dice);
    if table.ends_with("reward_ominous") {
        for stack in &mut loot {
            let mut data=stack.extras();
            if crate::blocks::item_name(stack.item)==Some("ominous_bottle") {data.ominous=Some(dice.between(2,4) as u8);}
            if let Some((id,_))=crate::stackdata::ENCHANTMENTS.iter().enumerate().find(|(_,e)|e.0=="wind_burst")
                && data.level("wind_burst")>0 {data.set_enchant(id as u16,1);}
            if crate::blocks::item_name(stack.item)==Some("enchanted_book") {data.stored=std::mem::take(&mut data.enchants);}
            *stack=stack.with_extras(data);
        }
    }
    loot
}

/// Выбор таблицы награды спавнера по весам (`loot_tables_to_eject`).
pub fn pick_reward(rewards: &[(String, u32)], dice: &mut Dice) -> String {
    let total: u32 = rewards.iter().map(|(_, weight)| *weight).sum();

    if total == 0 {
        return String::new();
    }

    let mut pick = (dice.next() % u64::from(total)) as u32;

    for (table, weight) in rewards {
        if pick < *weight {
            return table.clone();
        }

        pick -= weight;
    }

    String::new()
}

// ---------------------------------------------------------------------------
// Такт
// ---------------------------------------------------------------------------

/// Свойство состояния блока.
fn prop(state: i32, key: &str) -> Option<&'static str> {
    crate::blocks::orientation_of(state)?.value(state, key)
}

/// Зловещий ли блок (свойство `ominous`).
pub fn ominous_state(state: i32) -> bool {
    prop(state, "ominous") == Some("true")
}

/// Ставит блоку новое значение свойства; клиенты получат блок пачкой.
fn set_prop(world: &mut crate::world::World, (x, y, z): (i32, i32, i32), key: &str, value: &str) {
    let state = world.get_block(x, y, z);

    if let Some(new) = crate::blocks::orientation_of(state).and_then(|o| o.with(state, key, value))
        && new != state
    {
        world.set_block(x, y, z, new);
    }
}

/// Вики, Trial Omen: видимый спавнер в радиусе 15. Пока переводим только
/// ожидающий спавнер; сброс активной волны и cooldown [проверить].
pub fn omen_candidates(world: &crate::world::World, at: (f64,f64,f64), eye: f64) -> Vec<(i32,i32,i32)> {
    world.trial_places().into_iter().filter(|&(x,y,z)| {
        let block=world.get_block(x,y,z);
        let centre=(f64::from(x)+0.5,f64::from(y)+0.5,f64::from(z)+0.5);
        !ominous_state(block) && prop(block,"trial_spawner_state")==Some("waiting_for_players")
            && matches!(world.trial(x,y,z),Some(Data::Spawner(s)) if !s.normal.entity.is_empty())
            && (at.0-centre.0).powi(2)+(at.1-centre.1).powi(2)+(at.2-centre.2).powi(2)<225.0
            && clear(world,centre,(at.0,at.1+eye,at.2))
    }).collect()
}

pub fn activate_omen(world: &mut crate::world::World, positions: &[(i32,i32,i32)]) {
    for &p in positions {
        set_prop(world,p,"ominous","true");
        note(world,p);
    }
}

/// Bad Omen не складывает длительности: 15 минут за уровень или более
/// долгий уже действующий Trial Omen (вики, Trial Omen).
pub fn omen_duration(bad_level: i32, remaining: i32) -> i32 {
    if remaining == crate::survival::INFINITE { remaining }
    else { remaining.max(18_000 * bad_level.clamp(1,5)) }
}

/// Игрок рядом: номер, UUID, ноги, глаза, считается ли (выживание или
/// приключение — вики: «Only Survival and Adventure players count»).
#[derive(Clone, Copy, Debug)]
pub struct Visitor {
    pub id: i32,
    pub uuid: [u8; 16],
    pub at: (f64, f64, f64),
    pub eye: f64,
    pub counts: bool,
}

/// Выброшенная награда: стопка, откуда и с какой скоростью.
pub type Ejected = (Stack, (f64, f64, f64), (f64, f64, f64));

/// Что такт испытательных блоков просит у мира мобов: поставить моба.
pub struct Spawn {
    pub ominous: bool,
    pub kind: crate::mob::Kind,
    pub at: (f64, f64, f64),
    pub yaw: f32,
    pub no_ai: bool,
    pub silent: bool,
    pub bare: bool,
}

/// Прямая видимость между точками: на пути нет полного блока. Клетка, из
/// которой смотрят (сам спавнер), не в счёт.
fn clear(world: &crate::world::World, from: (f64, f64, f64), to: (f64, f64, f64)) -> bool {
    let start = (from.0.floor() as i32, from.1.floor() as i32, from.2.floor() as i32);
    let length = ((to.0 - from.0).powi(2) + (to.1 - from.1).powi(2) + (to.2 - from.2).powi(2)).sqrt();
    let steps = (length / 0.1).ceil().max(1.0) as i32;

    (1..=steps).all(|step| {
        let part = f64::from(step) / f64::from(steps);
        let cell = ((from.0 + (to.0 - from.0) * part).floor() as i32, (from.1 + (to.1 - from.1) * part).floor() as i32, (from.2 + (to.2 - from.2) * part).floor() as i32);

        cell == start || !crate::mob::solid(world.get_block(cell.0, cell.1, cell.2))
    })
}

fn note(world: &mut crate::world::World, (x, y, z): (i32, i32, i32)) {
    let state = world.get_block(x, y, z);

    if let Some(data) = world.trial(x, y, z) {
        let (kind, nbt) = data.nbt(ominous_state(state));

        world.note_block_entity(x, y, z, kind, nbt);
    }
}

/// Такт всех испытательных блоков мира. `now` — номер такта сервера;
/// `alive` — жив ли ещё (и не убран ли) моб с таким номером: Some(true) —
/// жив, Some(false) — умирает, None — его больше нет; `spawn` ставит моба
/// и отдаёт его номер.
#[allow(clippy::too_many_arguments)]
pub fn step(
    world: &mut crate::world::World,
    visitors: &[Visitor],
    mob_targets: &[(i32, (f64,f64,f64))],
    now: u64,
    can_spawn: bool,
    rng: &mut crate::mob::Rng,
    alive: &dyn Fn(i32) -> Option<bool>,
    spawn: &mut dyn FnMut(&mut crate::world::World, Spawn) -> i32,
    drop: &mut dyn FnMut(Ejected),
) {
    for pos in world.trial_places() {
        match world.trial(pos.0, pos.1, pos.2) {
            Some(Data::Spawner(_)) => step_spawner(world, pos, visitors, mob_targets, now, can_spawn, rng, alive, spawn, drop),
            Some(Data::Vault(_)) => step_vault(world, pos, visitors, now, rng, drop),
            None => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn step_spawner(
    world: &mut crate::world::World,
    pos: (i32, i32, i32),
    visitors: &[Visitor],
    mob_targets: &[(i32, (f64,f64,f64))],
    now: u64,
    can_spawn: bool,
    rng: &mut crate::mob::Rng,
    alive: &dyn Fn(i32) -> Option<bool>,
    spawn: &mut dyn FnMut(&mut crate::world::World, Spawn) -> i32,
    drop: &mut dyn FnMut(Ejected),
) {
    let Some(Data::Spawner(spawner)) = world.trial(pos.0, pos.1, pos.2).cloned() else {
        return;
    };
    let mut spawner = *spawner;
    let block = world.get_block(pos.0, pos.1, pos.2);
    let ominous = ominous_state(block);
    let state = prop(block, "trial_spawner_state").unwrap_or("inactive");
    let centre = (f64::from(pos.0) + 0.5, f64::from(pos.1) + 0.5, f64::from(pos.2) + 0.5);
    let config = spawner.config(ominous).clone();
    // Замеченные: в шаре дальности, считающиеся, с прямой видимостью до глаз.
    let seen = |world: &crate::world::World| -> Vec<i32> {
        visitors
            .iter()
            .filter(|v| v.counts)
            .filter(|v| (v.at.0 - centre.0).powi(2) + (v.at.1 - centre.1).powi(2) + (v.at.2 - centre.2).powi(2) < spawner.required_range.powi(2))
            .filter(|v| clear(world, centre, (v.at.0, v.at.1 + v.eye, v.at.2)))
            .map(|v| v.id)
            .collect()
    };
    let mut changed = false;

    match state {
        // Моб задан — ждать игроков (trial-spawner-wave: в следующем такте
        // после постановки, с данными блока).
        "inactive" => {
            if !config.entity.is_empty() {
                spawner.picked = true;
                world.with_trial(pos.0, pos.1, pos.2, |data| *data = Data::Spawner(Box::new(spawner.clone())));
                set_prop(world, pos, "trial_spawner_state", "waiting_for_players");
                note(world, pos);
            }

            return;
        }
        "waiting_for_players" => {
            // Сгенерированный спавнер стоит в этом состоянии сразу: моб выбран.
            if !spawner.picked {
                spawner.picked = true;
                world.with_trial(pos.0, pos.1, pos.2, |data| *data = Data::Spawner(Box::new(spawner.clone())));
            }

            if config.entity.is_empty() {
                set_prop(world, pos, "trial_spawner_state", "inactive");
                note(world, pos);
                return;
            }

            if !can_spawn {
                return;
            }

            let found = seen(world);

            if found.is_empty() {
                return;
            }

            // Заметил: событие 3013 с числом игроков, первый моб — через 40
            // тактов (trial-spawner-wave: 3013/1, next_mob_spawns_at +40)
            // [проверить: когда спавнер смотрит — у оригинала через 11 тактов
            // после постановки].
            world.note_level_event(pos.0, pos.1, pos.2, EVENT_DETECT_PLAYER, found.len() as i32);
            spawner.players = found;
            spawner.mobs.clear();
            spawner.total_spawned = 0;
            spawner.next_spawn_at = now + 40;
            spawner.next_item_at = now + 160;
            set_prop(world, pos, "trial_spawner_state", "active");
            changed = true;
        }
        "active" => {
            if ominous && now>=spawner.next_item_at && can_spawn {
                let nearby=|at:(f64,f64,f64)| (at.0-centre.0).powi(2)+(at.1-centre.1).powi(2)+(at.2-centre.2).powi(2)<PLAYER_RANGE.powi(2);
                let players:Vec<_>=visitors.iter().filter(|v|v.counts && nearby(v.at)).map(|v|(v.at,v.eye)).collect();
                if !players.is_empty() {
                    let mut candidates=players;
                    candidates.extend(mob_targets.iter().filter(|(id,at)|spawner.mobs.contains(id) && nearby(*at)).map(|(_,at)|(*at,1.0)));
                    let (target,eye)=candidates[rng.below(candidates.len() as u32) as usize];
                    // Высота 2..=5 блоков над ногами, предмет в центре клетки (чёрный ящик, ominous-spawner: 4,5 и 5,5; вики: «random height 2 to 5»).
                    let at=(target.0,target.1.floor()+2.0+f64::from(rng.below(4))+0.5,target.2);
                    if clear(world,(target.0,target.1+eye,target.2),at) {
                        world.ominous_births.push((crate::ominous::item(rng),at));
                    }
                }
                spawner.next_item_at=now+160;
            }
            for id in seen(world) {
                if !spawner.players.contains(&id) {
                    spawner.players.push(id);
                }
            }

            // Убранные мобы забываются; умирающие ещё занимают место (trial-spawner-wave:
            // второй зомби — в такте после того, как первый убран).
            spawner.mobs.retain(|id| alive(*id).is_some());

            let (total, at_once) = config.totals(spawner.players.len());
            let living = spawner.mobs.iter().filter(|id| alive(**id) == Some(true)).count();

            // Все побеждены — пауза перед наградой (trial-spawner-wave: в такте
            // смерти последнего).
            if spawner.total_spawned >= total && living == 0 {
                spawner.mobs.clear();
                spawner.eject_at = now + REWARD_DELAY;
                set_prop(world, pos, "trial_spawner_state", "waiting_for_reward_ejection");
                changed = true;
            } else if now >= spawner.next_spawn_at && spawner.mobs.len() < at_once as usize && spawner.total_spawned < total && can_spawn {
              // Несколько попыток места за такт [проверить: сколько у оригинала].
              for _ in 0..5 {
                let range = f64::from(config.spawn_range);
                let x = centre.0 + (rng.unit() - rng.unit()) * range;
                let z = centre.2 + (rng.unit() - rng.unit()) * range;
                let y = pos.1 + rng.below(3) as i32 - 1;
                let cell = (x.floor() as i32, y, z.floor() as i32);
                if let Some(kind) = config.kind()
                    && crate::mob::spawn::space::spawner(&crate::tick::MobView(world),
                        &crate::mob::Mob::new(0, kind, (x, f64::from(y), z)))
                    && clear(world, centre, (x, f64::from(y) + 0.5, z))
                {
                    let yaw = (rng.unit() * 360.0) as f32;
                    let id = spawn(world, Spawn { ominous, kind, at: (x, f64::from(y), z), yaw, no_ai: config.no_ai, silent: config.silent, bare: config.bare });

                    spawner.mobs.push(id);
                    spawner.total_spawned += 1;
                    spawner.next_spawn_at = now + u64::from(config.ticks_between_spawn);
                    world.note_level_event(pos.0, pos.1, pos.2, EVENT_SPAWN_AT_SPAWNER, i32::from(ominous));
                    world.note_level_event(cell.0, cell.1, cell.2, EVENT_SPAWN_AT_MOB, i32::from(ominous));
                    // Блок повторяется вместе с данными (trial-spawner-wave).
                    world.announce_change(pos.0, pos.1, pos.2, block);
                    changed = true;
                    break;
                }
              }
            }
        }
        // Пауза кончилась — заслонка открыта, награда из выбранной таблицы по
        // предмету на игрока (trial-spawner-wave: open_shutter через 40, первый
        // предмет через 20, закрытие через 30 после последнего).
        "waiting_for_reward_ejection" => {
            if now < spawner.eject_at {
                return;
            }

            let mut dice = Dice::new(u64::from(rng.below(u32::MAX)) << 32 | u64::from(rng.below(u32::MAX)));

            spawner.reward_table = pick_reward(&config.rewards, &mut dice);
            spawner.to_eject = spawner.players.len().max(1) as u32;
            spawner.eject_at = now + 20;
            world.note_sound(pos.0, pos.1, pos.2, "minecraft:block.trial_spawner.open_shutter", 1.0, 1.0);
            set_prop(world, pos, "trial_spawner_state", "ejecting_reward");
            changed = true;
        }
        "ejecting_reward" => {
            if now < spawner.eject_at {
                return;
            }

            if spawner.to_eject > 0 {
                let mut dice = Dice::new(u64::from(rng.below(u32::MAX)) << 32 | u64::from(rng.below(u32::MAX)));

                for stack in roll_table(&spawner.reward_table, &mut dice) {
                    drop((stack, (centre.0, f64::from(pos.1) + 1.08, centre.2), eject_velocity(rng)));
                }

                world.note_level_event(pos.0, pos.1, pos.2, EVENT_EJECT_ITEM, 0);
                spawner.to_eject -= 1;
                spawner.eject_at = now + EJECT_EVERY;
                world.with_trial(pos.0, pos.1, pos.2, |data| *data = Data::Spawner(Box::new(spawner.clone())));
                return;
            }

            world.note_sound(pos.0, pos.1, pos.2, "minecraft:block.trial_spawner.close_shutter", 1.0, 1.0);
            spawner.cooldown_end_at = now + spawner.cooldown_length;
            spawner.players.clear();
            set_prop(world, pos, "trial_spawner_state", "cooldown");
            changed = true;
        }
        // Остывание (вики: 30 минут) — снова ждать игроков.
        "cooldown" => {
            if now < spawner.cooldown_end_at {
                return;
            }

            set_prop(world, pos, "trial_spawner_state", "waiting_for_players");
            if ominous {set_prop(world,pos,"ominous","false");}
            changed = true;
        }
        _ => return,
    }

    let active = prop(world.get_block(pos.0, pos.1, pos.2), "trial_spawner_state") == Some("active");

    world.with_trial(pos.0, pos.1, pos.2, |data| *data = Data::Spawner(Box::new(spawner.clone())));

    if changed {
        let nbt = spawner.nbt_at(ominous, active.then_some(spawner.next_spawn_at));

        world.note_block_entity(pos.0, pos.1, pos.2, SPAWNER_ENTITY, nbt);
    }
}

/// Скорость выброшенной награды: вверх около 0,2 и чуть вбок
/// (trial-spawner-wave, vault-open: 0,19…0,21 вверх, до ±0,015 вбок) [проверить].
fn eject_velocity(rng: &mut crate::mob::Rng) -> (f64, f64, f64) {
    ((rng.unit() - 0.5) * 0.03, 0.18 + rng.unit() * 0.04, (rng.unit() - 0.5) * 0.03)
}

impl Spawner {
    /// NBT для клиента с полем `next_mob_spawns_at`, пока волна идёт
    /// (trial-spawner-wave: `{"next_mob_spawns_at":…,"spawn_data":…}`).
    pub fn nbt_at(&self, ominous: bool, next: Option<u64>) -> Vec<u8> {
        let mut out = self.nbt(ominous);

        if let Some(next) = next {
            // Поле — первым (порядок хеш-таблицы Java: корзина 1 раньше 10).
            let mut head = vec![0x0A, 0x04];

            string(&mut head, "next_mob_spawns_at");
            head.extend_from_slice(&next.to_be_bytes());
            head.extend_from_slice(&out[1..]);
            out = head;
        }

        out
    }
}

fn step_vault(world: &mut crate::world::World, pos: (i32, i32, i32), visitors: &[Visitor], now: u64, rng: &mut crate::mob::Rng, drop: &mut dyn FnMut(Ejected)) {
    let Some(Data::Vault(vault)) = world.trial(pos.0, pos.1, pos.2).cloned() else {
        return;
    };
    let mut vault = *vault;
    let block = world.get_block(pos.0, pos.1, pos.2);
    let state = prop(block, "vault_state").unwrap_or("inactive");
    let centre = (f64::from(pos.0) + 0.5, f64::from(pos.1) + 0.5, f64::from(pos.2) + 0.5);

    // Новое хранилище смотрит вокруг раз в 20 тактов от постановки (vault-open:
    // поставлено на 10395, ожило на 10435 — игрок подошёл на 10418).
    if vault.step_at == 0 {
        vault.step_at = now + 20;
        world.with_trial(pos.0, pos.1, pos.2, |data| *data = Data::Vault(Box::new(vault)));
        return;
    }

    // Повтор блока и данных после ключа (vault-open: +1).
    if vault.echo {
        vault.echo = false;
        world.announce_change(pos.0, pos.1, pos.2, block);
        world.note_block_entity(pos.0, pos.1, pos.2, VAULT_ENTITY, vault.nbt());
        world.with_trial(pos.0, pos.1, pos.2, |data| {
            if let Data::Vault(stored) = data {
                stored.echo = false;
            }
        });
    }

    if now < vault.step_at {
        return;
    }

    let mut changed = false;

    match state {
        "inactive" | "active" => {
            vault.step_at = now + 20;

            let near = |range: f64| -> Vec<[u8; 16]> {
                visitors
                    .iter()
                    .filter(|v| (v.at.0 - centre.0).powi(2) + (v.at.1 - centre.1).powi(2) + (v.at.2 - centre.2).powi(2) <= range * range)
                    .map(|v| v.uuid)
                    .filter(|uuid| !vault.rewarded(uuid))
                    .collect()
            };
            let connected = if state == "active" { near(VAULT_DEACTIVATION) } else { near(VAULT_ACTIVATION) };

            if state == "inactive" && !connected.is_empty() {
                world.note_level_event(pos.0, pos.1, pos.2, EVENT_VAULT_ACTIVATE, 0);
                set_prop(world, pos, "vault_state", "active");
                changed = true;
            } else if state == "active" && connected.is_empty() {
                world.note_level_event(pos.0, pos.1, pos.2, EVENT_VAULT_DEACTIVATE, 0);
                set_prop(world, pos, "vault_state", "inactive");
                changed = true;
            }

            changed |= connected != vault.connected;
            vault.connected = connected;
        }
        // Открытие: через 13 тактов после ключа заслонка открывается
        // (vault-open: ключ на 10448, open_shutter на 10461), дальше по
        // стопке раз в 20 тактов, после последней — закрытие и сон.
        "unlocking" => {
            world.note_sound(pos.0, pos.1, pos.2, "minecraft:block.vault.open_shutter", 1.0, 1.0);
            set_prop(world, pos, "vault_state", "ejecting");
            vault.step_at = now + 20;
            changed = true;
        }
        "ejecting" => {
            vault.step_at = now + 20;

            if vault.to_eject.is_empty() {
                world.note_sound(pos.0, pos.1, pos.2, "minecraft:block.vault.close_shutter", 1.0, 1.0);
                world.note_level_event(pos.0, pos.1, pos.2, EVENT_VAULT_DEACTIVATE, 0);
                set_prop(world, pos, "vault_state", "inactive");
                vault.connected.clear();
            } else {
                let stack = vault.to_eject.remove(0);
                let ejected = vault.ejected;
                let total = ejected + vault.to_eject.len() as u32 + 1;
                // Высота звука растёт от 0,8 до 1,2 к последней стопке
                // (vault-open: две стопки — 0,8 и 1,2) [проверить на трёх и больше].
                let pitch = if total <= 1 { 0.8 } else { 0.8 + 0.4 * ejected as f32 / (total - 1) as f32 };

                drop((stack, (centre.0, f64::from(pos.1) + 1.08, centre.2), eject_velocity(rng)));
                world.note_level_event(pos.0, pos.1, pos.2, EVENT_VAULT_EJECT, 0);
                world.note_sound(pos.0, pos.1, pos.2, "minecraft:block.vault.eject_item", 1.0, pitch);
                vault.ejected += 1;
                // Блок повторяется вместе с данными (vault-open).
                world.announce_change(pos.0, pos.1, pos.2, block);
            }

            changed = true;
        }
        _ => {}
    }

    world.with_trial(pos.0, pos.1, pos.2, |data| *data = Data::Vault(Box::new(vault.clone())));

    if changed {
        world.note_block_entity(pos.0, pos.1, pos.2, VAULT_ENTITY, vault.nbt());
    }
}

/// Щелчок по хранилищу предметом `held` (имя). Ожившее хранилище берёт
/// ключ своего вида у ненаграждённого игрока: звук `insert_item`, блок
/// `unlocking`, награда — по стопке (vault-open). Уже награждённому —
/// `reject_rewarded_player`, не тот предмет — `insert_item_fail` (вики,
/// Vault, «Sounds») [проверить оба]. None — щелчок не про хранилище;
/// Some(true) — ключ тратится.
pub fn use_vault(world: &mut crate::world::World, pos: (i32, i32, i32), uuid: [u8; 16], held: Option<&str>, now: u64, seed: u64) -> Option<bool> {
    let block = world.get_block(pos.0, pos.1, pos.2);

    if crate::blocks::block_at_state(block) != Some("vault") {
        return None;
    }

    let Some(Data::Vault(vault)) = world.trial(pos.0, pos.1, pos.2).cloned() else {
        return None;
    };
    let mut vault = *vault;
    let ominous = ominous_state(block);
    let key = if ominous { "ominous_trial_key" } else { "trial_key" };

    if prop(block, "vault_state") != Some("active") {
        return Some(false);
    }

    let Some(held) = held else {
        return Some(false);
    };

    if held != key {
        world.note_sound(pos.0, pos.1, pos.2, "minecraft:block.vault.insert_item_fail", 1.0, 1.0);
        return Some(false);
    }

    if vault.rewarded(&uuid) {
        world.note_sound(pos.0, pos.1, pos.2, "minecraft:block.vault.reject_rewarded_player", 1.0, 1.0);
        return Some(false);
    }

    let mut dice = Dice::new(seed);

    vault.remember(uuid);
    vault.connected.retain(|other| *other != uuid);
    vault.to_eject = roll_table(&vault.table(ominous), &mut dice);
    vault.ejected = 0;
    vault.step_at = now + 13;
    vault.echo = true;
    world.note_sound(pos.0, pos.1, pos.2, "minecraft:block.vault.insert_item", 1.0, 1.0);

    if let Some(state) = crate::blocks::orientation_of(block).and_then(|o| o.with(block, "vault_state", "unlocking")) {
        world.set_block(pos.0, pos.1, pos.2, state);
    }

    world.note_block_entity(pos.0, pos.1, pos.2, VAULT_ENTITY, vault.nbt());
    world.with_trial(pos.0, pos.1, pos.2, |data| *data = Data::Vault(Box::new(vault)));
    Some(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks;

    #[test]
    fn trial_spawner_rejects_flooded_land_mobs_but_keeps_water_mobs() {
        for (entity,medium,expected) in [("sheep","air",true),("sheep","water",false),("sheep","light[waterlogged=true]",false),("cod","water",true)] {
            let mut world=crate::world::World::in_memory();
            let floor=blocks::state_by_name("grass_block").unwrap();
            let fluid=blocks::state_from_text(medium).unwrap();
            for x in -5..=5 { for z in -5..=5 {
                world.set_block_silently(x,60,z,floor);
                for y in 61..=65 {world.set_block_silently(x,y,z,fluid);}
            }}
            let pos=(0,62,0);
            world.set_block(pos.0,pos.1,pos.2,blocks::state_from_text("trial_spawner[trial_spawner_state=active]").unwrap());
            world.with_trial(pos.0,pos.1,pos.2,|d| if let Data::Spawner(s)=d {
                s.players=vec![1];s.picked=true;s.normal.entity=format!("minecraft:{entity}");
            });
            let player=Visitor {id:1,uuid:[1;16],at:(4.5,62.0,4.5),eye:1.62,counts:true};
            let mut born=Vec::new();
            let mut rng=crate::mob::Rng::new(71);
            for now in 1..=40 {
                step(&mut world,&[player],&[],now,true,&mut rng,&|_|Some(true),
                    &mut |_,spawn| {born.push(spawn);born.len() as i32},&mut |_|{});
            }
            assert_eq!(!born.is_empty(),expected,"{entity} in {medium}");
        }
    }

    #[test]
    fn snbt_sets_the_wave() {
        let mut spawner = Spawner::default();

        spawner
            .apply_snbt(r#"{normal_config:{spawn_potentials:[{data:{entity:{id:"minecraft:zombie",NoAI:1b,Silent:1b}},weight:1}],total_mobs:2.0f,simultaneous_mobs:1.0f,total_mobs_added_per_player:0.0f,simultaneous_mobs_added_per_player:0.0f,ticks_between_spawn:20,loot_tables_to_eject:[{data:"minecraft:spawners/trial_chamber/key",weight:1}]}}"#)
            .expect("разобралось");

        assert_eq!(spawner.normal.entity, "minecraft:zombie");
        assert!(spawner.normal.no_ai && spawner.normal.silent);
        assert_eq!(spawner.normal.totals(1), (2, 1));
        assert_eq!(spawner.normal.ticks_between_spawn, 20);
        assert_eq!(spawner.normal.rewards, vec![("minecraft:spawners/trial_chamber/key".to_string(), 1)]);
    }

    #[test]
    fn the_client_sees_the_mob_in_java_hash_order() {
        let mut spawner = Spawner::default();

        spawner.apply_snbt(r#"{normal_config:{spawn_potentials:[{data:{entity:{id:"minecraft:zombie",NoAI:1b,Silent:1b}},weight:1}]}}"#).expect("разобралось");
        assert_eq!(spawner.nbt(false), vec![0x0A, 0], "до выбора моба клиент видит пустые данные");
        spawner.picked = true;

        let text = String::from_utf8_lossy(&spawner.nbt(false)).to_string();
        let at = |key: &str| text.find(key).expect("поле есть");

        // trial-spawner-wave: Silent, id, NoAI.
        assert!(at("Silent") < at("\0\u{2}id") && at("\0\u{2}id") < at("NoAI"));
        assert_eq!(Spawner::default().nbt(false), vec![0x0A, 0]);
    }

    #[test]
    fn players_scale_the_wave() {
        let breeze = preset("minecraft:trial_chamber/breeze").expect("бриз");

        assert_eq!(breeze.totals(1), (2, 1));
        assert_eq!(breeze.totals(3), (4, 2));
        // Вики: «with 2 players, 8 mobs spawn in total, 3 at once».
        assert_eq!(preset("minecraft:trial_chamber/melee/zombie").expect("зомби").totals(2), (8, 3));
    }

    #[test]
    fn records_survive_the_file() {
        let spawner = Spawner { normal: preset("trial_chamber/breeze").expect("бриз"), cooldown_end_at: 1234, ..Spawner::default() };

        let mut out = Vec::new();
        let data = Data::Spawner(Box::new(spawner));

        data.write((1, -40, 3), &mut out);
        assert_eq!(Data::read(&mut out.as_slice()), Some(((1, -40, 3), data)));

        let mut vault = Vault { loot: "minecraft:chests/trial_chambers/reward".into(), ..Vault::default() };

        vault.remember([7; 16]);

        let mut out = Vec::new();
        let data = Data::Vault(Box::new(vault));

        data.write((0, 0, 0), &mut out);
        assert_eq!(Data::read(&mut out.as_slice()), Some(((0, 0, 0), data)));
    }

    #[test]
    fn a_vault_forgets_the_oldest_of_128() {
        let mut vault = Vault::default();

        for n in 0..=128u8 {
            vault.remember([n; 16]);
        }

        assert_eq!(vault.rewarded.len(), 128);
        assert!(!vault.rewarded(&[0; 16]) && vault.rewarded(&[128; 16]));
    }

    #[test]
    fn the_key_table_gives_a_key() {
        let stacks = roll_table("minecraft:spawners/trial_chamber/key", &mut Dice::new(1));

        assert_eq!(stacks.len(), 1);
        assert_eq!(crate::blocks::item_name(stacks[0].item), Some("trial_key"));
    }

    #[test]
    fn measured_chamber_entries_become_blocks_data() {
        let Some(Data::Spawner(spawner)) = Data::generated("spawner", "trial_chamber/breeze/normal|trial_chamber/breeze/ominous") else {
            panic!("спавнер бриза");
        };

        assert_eq!(spawner.normal.entity, "minecraft:breeze");
        assert_eq!(spawner.normal.totals(1), (2, 1));

        let Some(Data::Vault(vault)) = Data::generated("vault", "chests/trial_chambers/reward_ominous") else {
            panic!("хранилище");
        };

        assert_eq!(vault.table(true), "minecraft:chests/trial_chambers/reward_ominous");
        assert_eq!(Data::generated("chest", "chests/trial_chambers/supply"), None);
    }
    #[test]
    fn ominous_conversion_resets_wave_once_and_keys_are_distinct() {
        let mut world=crate::world::World::in_memory();let pos=(0,70,0);
        world.set_block(pos.0,pos.1,pos.2,blocks::state_from_text("trial_spawner[trial_spawner_state=active]").unwrap());
        world.with_trial(pos.0,pos.1,pos.2,|d|if let Data::Spawner(s)=d {s.mobs=vec![42];s.total_spawned=4;s.normal.entity="minecraft:zombie".into();});
        assert_eq!(make_ominous(&mut world,(2.5,70.0,0.5),100),Some(vec![42]));
        assert!(ominous_state(world.get_block(pos.0,pos.1,pos.2)));
        assert!(make_ominous(&mut world,(2.5,70.0,0.5),101).is_none());
        let Some(Data::Spawner(s))=world.trial(pos.0,pos.1,pos.2) else {panic!()};
        assert_eq!(s.total_spawned,0);assert_eq!(s.ominous.entity,"minecraft:zombie");
        assert_eq!(s.ominous.rewards,ominous_rewards());
        let key=roll_table("spawners/ominous/trial_chamber/key",&mut Dice::new(2));
        assert_eq!(blocks::item_name(key[0].item),Some("ominous_trial_key"));
        let mut heavy=false;
        for seed in 1..200 {
            heavy |= roll_table("chests/trial_chambers/reward_ominous",&mut Dice::new(seed)).iter().any(|s|blocks::item_name(s.item)==Some("heavy_core"));
            assert!(!roll_table("chests/trial_chambers/reward",&mut Dice::new(seed)).iter().any(|s|blocks::item_name(s.item)==Some("heavy_core")));
        }
        assert!(heavy);
    }

    #[test]
    fn ominous_items_wait_eight_seconds_and_require_a_nearby_player() {
        let mut world=crate::world::World::in_memory();let at=(0,70,0);
        world.set_block(at.0,at.1,at.2,blocks::state_from_text("trial_spawner[ominous=true,trial_spawner_state=active]").unwrap());
        world.with_trial(at.0,at.1,at.2,|d|if let Data::Spawner(s)=d {
            s.players=vec![1];s.mobs=vec![42];s.total_spawned=1;s.next_item_at=260;
            s.ominous.entity="minecraft:zombie".into();s.ominous.simultaneous_mobs=1.0;
        });
        let player=Visitor {id:1,uuid:[1;16],at:(2.5,70.0,0.5),eye:1.62,counts:true};
        let mut rng=crate::mob::Rng::new(5);
        for (now,count) in [(259,0),(260,1),(261,1),(419,1),(420,2)] {
            step(&mut world,&[player],&[(42,(3.5,70.0,0.5))],now,true,&mut rng,&|_|Some(true),&mut |_,_|panic!("волна заполнена"),&mut |_|panic!("не время награды"));
            assert_eq!(world.ominous_births.len(),count);
        }
        assert!(world.ominous_births.iter().all(|(_,at)|(72.5..=75.5).contains(&at.1)));
        step(&mut world,&[],&[(42,(3.5,70.0,0.5))],580,true,&mut rng,&|_|Some(true),&mut |_,_|panic!(),&mut |_|panic!());
        assert_eq!(world.ominous_births.len(),2);
    }
    #[test]
    fn ominous_presets_preserve_species_and_increase_unarmored_waves() {
        for name in ["melee/husk","ranged/stray","small_melee/cave_spider","breeze"] {
            let normal=preset(&format!("trial_chamber/{name}/normal")).unwrap();
            let ominous=preset(&format!("trial_chamber/{name}/ominous")).unwrap();
            assert_eq!(normal.entity,ominous.entity);
            if name.contains("cave_spider") || name=="breeze" {assert_eq!(ominous.total_mobs,normal.total_mobs*2.0);}
        }
    }

}
