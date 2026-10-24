// Что плагин может вызвать у сервера: реализация импортов `mcsa:api`.
//
// Каждый вызов — обычная функция хозяина над общим состоянием сервера
// (`Shared`), с теми же замками и тем же порядком, что у остального ядра:
// замок берётся на время одного вызова и отпускается до возврата в плагин.
// Плагина при этом никто не ждёт под замком — в ответ на вызов он получает
// копию данных, не ссылку.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use wasmtime::StoreLimits;
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxView, WasiView};

use crate::dimension::Dimension;
use crate::players::{Member, Order};
use crate::shared::Shared;
use crate::{log_debug, log_error, log_info, log_warn};

wasmtime::component::bindgen!({
    path: "wit/mcsa-api.wit",
    world: "module",
});

use mcsa::api::types as wit;

/// Сколько задач и команд может завести один плагин: чтобы забытый цикл
/// не съел память.
pub const MAX_TASKS: usize = 256;
pub const MAX_COMMANDS: usize = 64;

/// Самый большой файл в папке данных плагина.
pub const MAX_FILE: usize = 4 * 1024 * 1024;

/// Имена команд, которые плагину занимать нельзя: это команды ядра и
/// игры, и подмена `op` или `stop` плагином была бы дырой.
const RESERVED: &[&str] = &[
    "op", "deop", "stop", "ban", "ban-ip", "pardon", "pardon-ip", "banlist", "kick", "whitelist", "gamemode", "give", "tp", "teleport", "help", "list", "say", "me", "msg", "tell",
    "w", "execute", "function", "data", "setblock", "fill", "summon", "kill", "effect", "time", "weather", "gamerule", "difficulty", "save-all", "save-off", "save-on", "plugman",
    "reload", "trigger", "scoreboard", "team", "tag", "tick", "clear", "xp", "experience", "enchant", "item", "damage", "spawnpoint", "setworldspawn", "worldborder",
    "advancement", "attribute", "bossbar", "clone", "datapack", "fillbiome", "forceload", "locate", "loot", "place", "random", "return", "ride", "rotate", "schedule",
    "spectate", "spreadplayers", "teammsg", "tm", "tnt",
];

/// Задача планировщика одного плагина.
#[derive(Clone, Debug, PartialEq)]
pub struct Task {
    pub handle: u32,
    /// Такт, на котором задача сработает.
    pub due: u64,
    pub period: Option<u32>,
    pub task: u32,
}

/// Команда, зарегистрированная плагином.
#[derive(Clone, Debug)]
pub struct Registered {
    pub name: String,
    pub description: String,
    pub usage: String,
    pub level: u8,
}

/// Кто каким плагином занят: имя команды → имя плагина. Общий для всех,
/// чтобы два плагина не заняли одно имя.
pub type CommandRegistry = Arc<Mutex<HashMap<String, String>>>;

/// Состояние одного плагина (`Store<PluginCtx>`).
pub struct PluginCtx {
    pub network: super::network::Access,
    pub chests: super::inventory::Chests,
    pub native_items: super::inventory::NativeItems,
    pub name: String,
    /// Модуль из `modules/` — доверенный.
    pub trusted: bool,
    pub shared: Arc<Shared>,
    /// Папка плагина: `plugin.toml`, `config.toml`, `data/`.
    pub folder: PathBuf,
    pub wasi: WasiCtx,
    pub table: ResourceTable,
    pub limits: StoreLimits,
    /// Такт, который идёт сейчас (для планировщика).
    pub tick: u64,
    pub tasks: Vec<Task>,
    pub next_handle: u32,
    pub commands: Vec<Registered>,
    pub registry: CommandRegistry,
    /// Имена загруженных плагинов — для `privileged.loaded-plugins`.
    pub loaded: Arc<Mutex<Vec<String>>>,
}

impl WasiView for PluginCtx {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView { ctx: &mut self.wasi, table: &mut self.table }
    }
}

impl PluginCtx {
    /// Задачи, чей час пришёл на такте `now`, в порядке номеров. Одноразовые
    /// удаляются, повторяющиеся переносятся.
    pub fn due_tasks(&mut self, now: u64) -> Vec<u32> {
        let mut due: Vec<(u32, u32)> = Vec::new();

        self.tasks.retain_mut(|task| {
            if task.due > now {
                return true;
            }

            due.push((task.handle, task.task));

            match task.period {
                Some(period) => {
                    task.due = now + u64::from(period);
                    true
                }
                None => false,
            }
        });

        due.sort_by_key(|(handle, _)| *handle);
        due.into_iter().map(|(_, task)| task).collect()
    }

    fn add_task(&mut self, delay: u32, period: Option<u32>, task: u32) -> u32 {
        if self.tasks.len() >= MAX_TASKS {
            log_warn!("[{}] слишком много задач (больше {}) — новая не заведена", self.name, MAX_TASKS);
            return 0;
        }

        self.next_handle = self.next_handle.wrapping_add(1).max(1);

        // Задача с задержкой 0 сработает на ближайшем такте, не в этом же
        // вызове: иначе плагин мог бы зациклить такт сам на себе.
        self.tasks.push(Task { handle: self.next_handle, due: self.tick + u64::from(delay.max(1)), period, task });
        self.next_handle
    }

    /// Папка данных плагина.
    pub fn data_dir(&self) -> PathBuf {
        self.folder.join("data")
    }
}

/// UUID строкой («8-4-4-4-12» или без дефисов) в байты.
pub fn parse_uuid(text: &str) -> Option<[u8; 16]> {
    let hex: String = text.chars().filter(|c| *c != '-').collect();

    if hex.len() != 32 || !hex.is_ascii() {
        return None;
    }

    let mut out = [0u8; 16];

    for (index, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&hex[index * 2..index * 2 + 2], 16).ok()?;
    }

    Some(out)
}

pub fn dimension_to_wit(dimension: Dimension) -> wit::Dimension {
    match dimension {
        Dimension::Overworld => wit::Dimension::Overworld,
        Dimension::Nether => wit::Dimension::Nether,
        Dimension::End => wit::Dimension::End,
    }
}

pub fn dimension_from_wit(dimension: wit::Dimension) -> Dimension {
    match dimension {
        wit::Dimension::Overworld => Dimension::Overworld,
        wit::Dimension::Nether => Dimension::Nether,
        wit::Dimension::End => Dimension::End,
    }
}

fn game_mode_to_wit(mode: i32) -> wit::GameMode {
    match mode {
        1 => wit::GameMode::Creative,
        2 => wit::GameMode::Adventure,
        3 => wit::GameMode::Spectator,
        _ => wit::GameMode::Survival,
    }
}

/// Игрок как его видит плагин.
pub fn member_to_wit(member: &Member) -> wit::Player {
    wit::Player {
        uuid: crate::network::login::format_uuid(&member.uuid),
        name: member.name.clone(),
        game_mode: game_mode_to_wit(member.game_mode),
        dimension: dimension_to_wit(member.dimension),
        x: member.position.x,
        y: member.position.y,
        z: member.position.z,
        yaw: member.position.yaw,
        pitch: member.position.pitch,
    }
}

impl super::PlayerView {
    /// Игрок как его видит плагин.
    pub fn to_wit(&self) -> wit::Player {
        wit::Player {
            uuid: crate::network::login::format_uuid(&self.uuid),
            name: self.name.clone(),
            game_mode: game_mode_to_wit(self.game_mode),
            dimension: dimension_to_wit(self.dimension),
            x: self.x,
            y: self.y,
            z: self.z,
            yaw: self.yaw,
            pitch: self.pitch,
        }
    }
}

/// Блок с приставкой: `minecraft:stone`.
fn block_text(state: i32) -> String {
    format!("minecraft:{}", crate::blocks::block_at_state(state).unwrap_or("air"))
}

/// Имя файла в папке данных: без путей, без точки в начале.
pub fn safe_file_name(name: &str) -> Result<&str, String> {
    let bad = name.is_empty() || name.len() > 100 || name.starts_with('.') || name.contains(['/', '\\', '\0', ':']);

    if bad { Err(format!("имя файла «{name}» не годится: без путей и точки в начале")) } else { Ok(name) }
}

// Каждая часть относительного пути проверяется; ссылки из data/ не разыменовываем.
fn data_path(root: &Path, name: &str) -> Result<PathBuf, String> {
    if name.is_empty() || name.len() > 512 { return Err("пустой или слишком длинный путь данных".into()); }
    let mut path = root.to_path_buf();
    let mut paths = vec![path.clone()];
    for part in name.split('/') { path.push(safe_file_name(part)?); paths.push(path.clone()); }
    for candidate in paths {
        match std::fs::symlink_metadata(candidate) {
            Ok(meta) if meta.file_type().is_symlink() => return Err("символические ссылки в data/ запрещены".into()),
            Ok(_) => (),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (),
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(path)
}

fn read_limited(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match std::fs::metadata(path) {
        Ok(meta) if meta.len() > MAX_FILE as u64 => Err(format!("файл больше {} МиБ", MAX_FILE / 1024 / 1024)),
        Ok(_) => std::fs::read(path).map(Some).map_err(|error| error.to_string()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.to_string()),
    }
}

fn write_file(path: &Path, data: &[u8]) -> Result<(), String> {
    if data.len() > MAX_FILE {
        return Err(format!("файл больше {} МиБ", MAX_FILE / 1024 / 1024));
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    // Через временный файл: оборванная запись не оставит половину.
    use std::io::Write;
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let temporary = path.with_file_name(format!(".mcsa-write-{}-{}", std::process::id(), NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new().write(true).create_new(true).open(&temporary).map_err(|error| error.to_string())?;
        file.write_all(data).map_err(|error| error.to_string())?;
        file.sync_all().map_err(|error| error.to_string())?;
        std::fs::rename(&temporary, path).map_err(|error| error.to_string())
    })();
    if result.is_err() { let _ = std::fs::remove_file(temporary); }
    result
}

impl mcsa::api::types::Host for PluginCtx {}

impl mcsa::api::log::Host for PluginCtx {
    fn write(&mut self, level: mcsa::api::log::Level, message: String) {
        use mcsa::api::log::Level;

        match level {
            Level::Debug => log_debug!("[{}] {}", self.name, message),
            Level::Info => log_info!("[{}] {}", self.name, message),
            Level::Warn => log_warn!("[{}] {}", self.name, message),
            Level::Error => log_error!("[{}] {}", self.name, message),
        }
    }
}

impl mcsa::api::server::Host for PluginCtx {
    fn is_stopping(&mut self) -> bool {super::stopping()}
    fn api_version(&mut self) -> String {
        super::manifest::API_VERSION.to_string()
    }

    fn current_tick(&mut self) -> u64 {
        self.tick
    }

    fn broadcast(&mut self, message: String) {
        // Как любое сообщение в чат: без управляющих знаков и не длиннее
        // предела. Цвета («§») плагину пока нельзя — как и игрокам.
        let clean: String = message.chars().filter(|c| !c.is_control() && *c != '§').take(crate::chat::MAX_MESSAGE).collect();

        if !clean.is_empty() {
            self.shared.chat.lock().expect("чат захвачен другим потоком").push(clean);
        }
    }

    fn online_players(&mut self) -> Vec<wit::Player> {
        self.shared.players.lock().expect("список игроков захвачен другим потоком").members().iter().map(member_to_wit).collect()
    }

    fn find_player(&mut self, name: String) -> Option<wit::Player> {
        let players = self.shared.players.lock().expect("список игроков захвачен другим потоком");

        players.members().iter().find(|member| member.name.eq_ignore_ascii_case(&name)).map(member_to_wit)
    }
}

impl mcsa::api::player::Host for PluginCtx {
    fn get_state(&mut self, uuid: String) -> Result<mcsa::api::player::State, String> {
        let uuid = parse_uuid(&uuid).ok_or("UUID не разобрать")?;
        let players = self.shared.players.lock().expect("игроки");
        let member = players.members().iter().find(|member| member.uuid == uuid).ok_or("игрока нет на сервере")?;
        let gear = players.gear(&uuid);
        let state = &players.ui.info.get(&uuid).ok_or("Состояние доступно только для Java")?.plugin_state;
        Ok(mcsa::api::player::State {
            fire_ticks: state.fire_ticks, dead: gear.view.dead, sneaking: member.sneaking,
            on_ground: member.position.on_ground, in_water: state.in_water, in_lava: state.in_lava,
            in_rain: state.in_rain, eye_height: state.eye_height,
            using_item: gear.living & 1 != 0, off_hand: gear.living & 2 != 0,
            effects: state.effects.iter().map(|effect| mcsa::api::player::PotionEffect {
                kind: effect.kind.name().into(), duration: effect.ticks_left, amplifier: effect.amplifier,
                ambient: effect.ambient, particles: effect.visible, icon: effect.show_icon,
            }).collect(),
        })
    }
    fn set_fire_ticks(&mut self, uuid: String, ticks: i32) -> Result<(), String> {
        let uuid = parse_uuid(&uuid).ok_or("UUID не разобрать")?;
        let mut players = self.shared.players.lock().expect("игроки");
        let info = players.ui.info.get_mut(&uuid).ok_or("игрока Java нет на сервере")?;
        info.plugin_state.fire_ticks = ticks.max(0);
        players.order(&uuid, Order::PluginFire { ticks: ticks.max(0) });Ok(())
    }
    fn damage(&mut self, uuid: String, amount: f64) -> Result<(), String> {
        if !amount.is_finite() || amount < 0.0 || amount > f64::from(f32::MAX) { return Err("Урон должен быть конечным неотрицательным числом".into()); }
        let uuid = parse_uuid(&uuid).ok_or("UUID не разобрать")?;
        let mut players = self.shared.players.lock().expect("игроки");
        if !players.ui.info.contains_key(&uuid) { return Err("игрока Java нет на сервере".into()); }
        players.order(&uuid, Order::Hurt { amount: amount as f32, source: crate::survival::Source::Generic });Ok(())
    }
    fn give_experience(&mut self, uuid: String, amount: i32) -> Result<(), String> {
        if amount < 0 {return Err("Количество выдаваемого опыта не может быть отрицательным".into());}
        let uuid = parse_uuid(&uuid).ok_or("UUID не разобрать")?;
        let mut players = self.shared.players.lock().expect("игроки");
        if !players.ui.info.contains_key(&uuid) { return Err("игрока Java нет на сервере".into()); }
        players.order(&uuid, Order::Experience { set: false, amount, levels: false });Ok(())
    }
    fn set_respawn(&mut self, uuid: String, to: Option<wit::Location>) -> Result<(), String> {
        let uuid = parse_uuid(&uuid).ok_or("UUID не разобрать")?;
        let point = match to {
            Some(to) if to.x.is_finite() && to.x.abs() <= 30_000_000.0 && to.z.is_finite() && to.z.abs() <= 30_000_000.0 && to.y.is_finite() && (-2048.0..=2048.0).contains(&to.y) && to.yaw.is_none_or(f32::is_finite) => Some(crate::bed::RespawnPoint { dimension: dimension_from_wit(to.dimension), pos: (to.x.floor() as i32, to.y.floor() as i32, to.z.floor() as i32), yaw: to.yaw.unwrap_or(0.0), forced: false }),
            None => None,
            _ => return Err("Недопустимая точка возрождения".into()),
        };
        let mut players = self.shared.players.lock().expect("игроки");
        if !players.members().iter().any(|member| member.uuid == uuid) {return Err("игрока нет на сервере".into());}
        if !players.ui.info.contains_key(&uuid) {return Err("Точка возрождения API пока поддерживается только для Java".into());}
        players.order(&uuid, Order::PluginSpawn { point });Ok(())
    }

    fn get_bed_action(&mut self, uuid: String, pos: wit::BlockPos) -> Option<mcsa::api::player::BedAction> {
        let uuid = parse_uuid(&uuid)?;
        let (member, dead, sleeping) = {let players = self.shared.players.lock().expect("игроки");(players.members().iter().find(|member| member.uuid == uuid)?.clone(), players.gear(&uuid).view.dead, players.gear(&uuid).sleeping.is_some())};
        let dimension = dimension_from_wit(pos.dimension);if member.dimension != dimension || member.sneaking {return None;}
        let world = self.shared.world_in(dimension).lock().expect("мир");
        let head = crate::bed::head_of(&world, (pos.x, pos.y, pos.z))?;
        let problem = if !crate::bed::beds_work(dimension) {4}
            else if sleeping || dead || crate::bed::occupied(&world, head) {5}
            else if !crate::bed::bed_in_reach(&world, head, (member.position.x, member.position.y, member.position.z)) {1}
            else if crate::bed::bed_obstructed(&world, head) {2}
            else {let items = self.shared.items_in(dimension).lock().expect("сущности");let (wide, high) = crate::bed::MONSTER_REACH;
                if member.game_mode != 1 && items.mobs().iter().any(|mob| mob.kind.hostile() && !mob.is_dead() && (mob.x - (f64::from(head.0)+0.5)).abs() <= wide && (mob.y-f64::from(head.1)).abs() <= high && (mob.z-(f64::from(head.2)+0.5)).abs() <= wide) {3} else {0}};
        let dark = crate::bed::dark_enough(world.time_of_day(), world.weather.is_raining(), world.weather.is_thundering());
        Some(mcsa::api::player::BedAction { bed: wit::BlockPos { dimension: pos.dimension, x: head.0, y: head.1, z: head.2 }, problem, can_spawn: problem == 0, can_sleep: problem == 0 && dark })
    }

    fn set_health(&mut self, uuid: String, health: f64) -> Result<(), String> {
        let uuid = parse_uuid(&uuid).ok_or("UUID не разобрать")?;
        let mut players = self.shared.players.lock().expect("игроки");
        let info = players.ui.info.get(&uuid).ok_or("игрока Java нет на сервере")?;
        if !health.is_finite() || health < 0.0 || health > f64::from(info.max_health) {
            return Err("здоровье: конечное число от 0 до максимума игрока".into());
        }
        if players.gear(&uuid).view.dead { return Err("игрок уже умер".into()); }
        players.order(&uuid, Order::Health { value: health as f32 });
        Ok(())
    }

    fn add_potion_effect(&mut self, uuid: String, effect: mcsa::api::player::PotionEffect) -> Result<(), String> {
        let uuid = parse_uuid(&uuid).ok_or("UUID не разобрать")?;
        let kind = crate::survival::EffectKind::from_name(&effect.kind).ok_or("неизвестный эффект")?;
        if effect.duration < -1 || !(0..=255).contains(&effect.amplifier) {
            return Err("эффект: duration >= -1, amplifier от 0 до 255".into());
        }
        let mut players = self.shared.players.lock().expect("игроки");
        if !players.members().iter().any(|member| member.uuid == uuid) { return Err("игрока нет на сервере".into()); }
        players.order(&uuid, Order::Effect(crate::survival::Effect { kind, amplifier: effect.amplifier, ticks_left: effect.duration, ambient: effect.ambient, visible: effect.particles, show_icon: effect.icon }));
        Ok(())
    }

    fn remove_potion_effect(&mut self, uuid: String, kind: String) -> Result<(), String> {
        let uuid = parse_uuid(&uuid).ok_or("UUID не разобрать")?;
        let kind = crate::survival::EffectKind::from_name(&kind).ok_or("неизвестный эффект")?;
        let mut players = self.shared.players.lock().expect("игроки");
        if !players.members().iter().any(|member| member.uuid == uuid) { return Err("игрока нет на сервере".into()); }
        players.order(&uuid, Order::ClearEffect(kind));
        Ok(())
    }

    fn send_message(&mut self, uuid: String, message: String) -> bool {
        let Some(uuid) = parse_uuid(&uuid) else { return false };
        let mut players = self.shared.players.lock().expect("список игроков захвачен другим потоком");

        if !players.members().iter().any(|member| member.uuid == uuid) {
            return false;
        }

        let clean: String = message.chars().filter(|c| !c.is_control() || *c == '\n').collect();

        players.order(&uuid, Order::Message { component: crate::network::nbt::text_component(&clean) });
        true
    }

    fn teleport(&mut self, uuid: String, to: wit::Location) -> bool {
        let Some(uuid) = parse_uuid(&uuid) else { return false };
        let sane = |value: f64, limit: f64| value.is_finite() && value.abs() <= limit;

        if !(sane(to.x, 30_000_000.0) && sane(to.y, 20_000_000.0) && sane(to.z, 30_000_000.0)) {
            return false;
        }

        let mut players = self.shared.players.lock().expect("список игроков захвачен другим потоком");
        let Some(member) = players.members().iter().find(|member| member.uuid == uuid).cloned() else {
            return false;
        };

        let dimension = dimension_from_wit(to.dimension);

        if dimension == member.dimension {
            players.order(&uuid, Order::Teleport { x: to.x, y: to.y, z: to.z });
            // Перенос — не шаг: игрок «встаёт» на блок не сразу (players.rs).
            players.unsettle(&uuid);
        } else {
            players.order(&uuid, Order::ChangeDimension { dimension, x: to.x, y: to.y, z: to.z });
        }

        if let (Some(yaw), Some(pitch)) = (to.yaw, to.pitch)
            && yaw.is_finite()
            && pitch.is_finite()
        {
            players.order(&uuid, Order::Look { yaw, pitch });
        }

        true
    }

    fn give_item(&mut self, uuid: String, stack: wit::ItemStack) -> Result<(), String> {
        let uuid = parse_uuid(&uuid).ok_or_else(|| "UUID не разобрать".to_string())?;

        if stack.count == 0 || stack.count > 2304 {
            return Err("count: от 1 до 2304".to_string());
        }

        let spec = stack.item.strip_prefix("minecraft:").unwrap_or(&stack.item).to_string();
        let stack = crate::commands::stack_from_spec(&spec, stack.count as i32)?.ok_or_else(|| "воздух выдать нельзя".to_string())?;
        let mut players = self.shared.players.lock().expect("список игроков захвачен другим потоком");

        if !players.members().iter().any(|member| member.uuid == uuid) {
            return Err("игрока нет на сервере".to_string());
        }

        players.order(&uuid, Order::Give { stack });
        Ok(())
    }

    fn set_game_mode(&mut self, uuid: String, mode: wit::GameMode) -> bool {
        let Some(uuid) = parse_uuid(&uuid) else { return false };
        let mode = match mode { wit::GameMode::Survival => 0, wit::GameMode::Creative => 1, wit::GameMode::Adventure => 2, wit::GameMode::Spectator => 3 };
        let mut players = self.shared.players.lock().expect("список игроков занят");
        if !players.members().iter().any(|member| member.uuid == uuid) { return false; }
        players.set_game_mode(&uuid, mode);
        players.order(&uuid, Order::GameMode { mode });
        true
    }

    fn operator_level(&mut self, uuid: String) -> Option<u8> {
        let uuid = parse_uuid(&uuid)?;
        if !self.shared.players.lock().expect("список игроков занят").members().iter().any(|member| member.uuid == uuid) { return None; }
        Some(self.shared.ops.lock().expect("список операторов занят").level_of(&uuid) as u8)
    }

    fn set_inventory_slot(&mut self, uuid: String, slot: u32, stack: Option<wit::ItemStack>) -> Result<(), String> {
        let uuid = parse_uuid(&uuid).ok_or("UUID не разобрать")?;
        if !(9..=44).contains(&slot) { return Err("ячейка: от 9 до 44".into()); }
        let stack = match stack {
            Some(stack) if stack.count > 0 && stack.count <= 64 => crate::commands::stack_from_spec(&stack.item, stack.count as i32)?,
            Some(_) => return Err("count: от 1 до 64".into()),
            None => None,
        };
        let mut players = self.shared.players.lock().expect("список игроков занят");
        if !players.members().iter().any(|member| member.uuid == uuid) { return Err("игрока нет на сервере".into()); }
        players.order(&uuid, Order::SetSlot { slot: slot as i32, stack });
        Ok(())
    }

    fn kick(&mut self, uuid: String, reason: String) -> bool {
        let Some(uuid) = parse_uuid(&uuid) else { return false };
        let mut players = self.shared.players.lock().expect("список игроков захвачен другим потоком");

        if !players.members().iter().any(|member| member.uuid == uuid) {
            return false;
        }

        players.order(&uuid, Order::Kick { reason });
        true
    }
}

impl mcsa::api::world::Host for PluginCtx {
    fn spawn_particle(&mut self, at: wit::Location, name: String, count: u32, dx: f64, dy: f64, dz: f64) -> Result<(), String> {
        if ![at.x,at.y,at.z,dx,dy,dz].iter().all(|v| v.is_finite()) || at.x.abs() > 30_000_000.0 || at.z.abs() > 30_000_000.0 || !(-2048.0..=2048.0).contains(&at.y) || [dx,dy,dz].iter().any(|v| !(0.0..=64.0).contains(v)) || count > 4096 {return Err("Недопустимые параметры частиц".into());}
        if !matches!(name.as_str(), "happy_villager" | "heart") {return Err("Частица вне поддерживаемого каталога".into());}
        let kind = crate::commands_admin::PARTICLES.iter().position(|n| *n == name).ok_or("Неизвестная частица")? as i32;
        self.shared.world_in(dimension_from_wit(at.dimension)).lock().expect("мир").note_particle(kind,None,(at.x,at.y,at.z),(dx as f32,dy as f32,dz as f32),0.0,count as i32);Ok(())
    }
    fn break_naturally(&mut self, pos: wit::BlockPos) -> Result<bool, String> {
        let dimension = dimension_from_wit(pos.dimension);
        let drops = {
            let mut world = self.shared.world_in(dimension).lock().expect("мир");
            if !world.has_chunk(pos.x.div_euclid(16),pos.z.div_euclid(16)) {return Err("чанк не загружен".into());}
            let state = world.get_block(pos.x,pos.y,pos.z);
            if crate::mining::properties(state).0 < 0.0 || matches!(crate::blocks::block_at_state(state),Some("air"|"cave_air"|"void_air")) {return Ok(false);}
            // Шалкер с содержимым уже выпадает через обычную очередь spill_containers.
            let packed = world.container(pos.x,pos.y,pos.z).is_some_and(|c| c.kind == crate::containers::Kind::ShulkerBox);
            let drops = if packed {Vec::new()} else {crate::mining::block_drops(state,None,||world.random_unit())};
            crate::placing::break_block(&mut world,pos.x,pos.y,pos.z);drops
        };
        let mut items=self.shared.items_in(dimension).lock().expect("предметы");
        for stack in drops {items.drop_item(self.shared.next_entity_id(),stack,(f64::from(pos.x)+0.5,f64::from(pos.y)+0.5,f64::from(pos.z)+0.5),(0.0,0.2,0.0),10);}
        Ok(true)
    }

    fn chunk_loaded(&mut self, dimension: wit::Dimension, x: i32, z: i32) -> bool {
        self.shared.world_in(dimension_from_wit(dimension)).lock().expect("мир").has_chunk(x, z)
    }
    fn highest_block(&mut self, dimension: wit::Dimension, x: i32, z: i32) -> Result<i32, String> {
        let world = self.shared.world_in(dimension_from_wit(dimension)).lock().expect("мир");
        if !world.has_chunk(x.div_euclid(16), z.div_euclid(16)) {return Err("чанк не загружен".into());}
        Ok((-64..320).rev().find(|y| !matches!(crate::blocks::block_at_state(world.get_block(x, *y, z)), Some("air" | "cave_air" | "void_air"))).unwrap_or(-64))
    }
    fn random_tick_speed(&mut self) -> u32 { crate::farming::random_tick_speed() }
    fn block_details(&mut self, pos: wit::BlockPos) -> Result<mcsa::api::world::BlockState, String> {
        let mut world = self.shared.world_in(dimension_from_wit(pos.dimension)).lock().expect("мир");
        if !world.has_chunk(pos.x.div_euclid(16), pos.z.div_euclid(16)) {return Err("чанк не загружен".into());}
        let state = world.get_block(pos.x, pos.y, pos.z);
        let mut data = block_text(state);
        let mut age = -1; let mut max_age = -1;
        if let Some(block) = crate::blocks::orientation_of(state) {
            let props: Vec<_> = block.properties.iter().filter_map(|p| Some(format!("{}={}", p.name, block.value(state, p.name)?))).collect();
            if !props.is_empty() {data.push_str(&format!("[{}]", props.join(",")));}
            if let Some(property) = block.properties.iter().find(|p| p.name == "age") {
                age = block.value(state, "age").and_then(|v| v.parse().ok()).unwrap_or(-1);
                max_age = property.values.iter().filter_map(|v| v.parse::<i32>().ok()).max().unwrap_or(-1);
            }
        }
        let (sky, light) = world.light_full(pos.x, pos.y, pos.z);
        Ok(mcsa::api::world::BlockState {data, sky, light: sky.max(light), liquid: matches!(crate::blocks::block_at_state(state), Some("water" | "lava")), passable: !crate::blocks::has_box(state), age, max_age})
    }
    fn update_block(&mut self, pos: wit::BlockPos, data: String, force: bool, physics: bool) -> Result<bool, String> {
        let state = crate::blocks::state_from_text(&data).ok_or("неизвестное состояние блока")?;
        if !(-64..320).contains(&pos.y) {return Err("Высота блока вне мира".into());}
        let dimension = dimension_from_wit(pos.dimension);
        {
            let mut world = self.shared.world_in(dimension).lock().expect("мир");
            if !world.has_chunk(pos.x.div_euclid(16), pos.z.div_euclid(16)) {return Err("чанк не загружен".into());}
            let current = world.get_block(pos.x, pos.y, pos.z);
            if !force && crate::blocks::block_at_state(current) != crate::blocks::block_at_state(state) {return Ok(false);}
            if !physics {world.set_block(pos.x, pos.y, pos.z, state);return Ok(true);}
        }
        self.set_block(pos, data)?;Ok(true)
    }
    fn random_tick(&mut self, pos: wit::BlockPos) -> Result<(), String> {
        let mut world = self.shared.world_in(dimension_from_wit(pos.dimension)).lock().expect("мир");
        if !world.has_chunk(pos.x.div_euclid(16), pos.z.div_euclid(16)) {return Err("чанк не загружен".into());}
        if !(-64..320).contains(&pos.y) {return Err("Высота блока вне мира".into());}
        let state = world.get_block(pos.x, pos.y, pos.z);
        crate::farming::random_tick(&mut world, (pos.x, pos.y, pos.z), state);Ok(())
    }
    fn play_sound(&mut self, at: wit::Location, sound: String, volume: f32, pitch: f32) -> Result<(), String> {
        if !at.x.is_finite() || !at.y.is_finite() || !at.z.is_finite() || at.x.abs() > 30_000_000.0 || at.z.abs() > 30_000_000.0 || !(-2048.0..=2048.0).contains(&at.y) || !volume.is_finite() || !(0.0..=16.0).contains(&volume) || !pitch.is_finite() || !(0.0..=2.0).contains(&pitch) {return Err("Недопустимые параметры звука".into());}
        if !sound.starts_with("minecraft:") || sound.len() > 256 || !sound.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"_:./-".contains(&b)) {return Err("Недопустимый ключ звука".into());}
        let sound = match sound.as_str() {
            "minecraft:block.anvil.use" => "block.anvil.use", "minecraft:entity.enderman.teleport" => "entity.enderman.teleport",
            "minecraft:entity.evoker.prepare_summon" => "entity.evoker.prepare_summon", "minecraft:entity.magma_cube.squish" => "entity.magma_cube.squish",
            "minecraft:entity.piglin.admiring_item" => "entity.piglin.admiring_item", "minecraft:entity.player.attack.crit" => "entity.player.attack.crit",
            _ => return Err("Звук вне поддерживаемого каталога".into()),
        };
        self.shared.world_in(dimension_from_wit(at.dimension)).lock().expect("мир").note_sound_player_at((at.x, at.y, at.z), sound, volume, pitch);Ok(())
    }
    fn get_border(&mut self, dimension: wit::Dimension) -> mcsa::api::world::Border {
        let border = crate::commands_world::border_of(&self.shared, dimension_from_wit(dimension));
        mcsa::api::world::Border { size: border.size(), center_x: border.center_x, center_z: border.center_z }
    }

    fn set_border(&mut self, dimension: wit::Dimension, value: mcsa::api::world::Border) -> Result<(), String> {
        if !value.size.is_finite() || !(1.0..=crate::commands_world::MAX_SIZE).contains(&value.size)
            || !value.center_x.is_finite() || value.center_x.abs() > crate::commands_world::MAX_CENTER
            || !value.center_z.is_finite() || value.center_z.abs() > crate::commands_world::MAX_CENTER {
            return Err("рамка: размер 1..59999968, центр в пределах ±29999984".into());
        }
        let dimension = dimension_from_wit(dimension);
        let packet = {
            let mut borders = self.shared.world_cmd.borders.lock().expect("рамки");
            let border = &mut borders[dimension.index()];
            border.from = value.size;
            border.to = value.size;
            border.center_x = value.center_x;
            border.center_z = value.center_z;
            border.lerp_total = 0;
            border.lerp_left = 0;
            border.init_packet()
        };
        let mut players = self.shared.players.lock().expect("игроки");
        let targets: Vec<_> = players.members().iter().filter(|member| member.dimension == dimension).map(|member| member.uuid).collect();
        for uuid in targets { players.order(&uuid, Order::Packet(packet.clone())); }
        Ok(())
    }

    fn get_block(&mut self, pos: wit::BlockPos) -> Option<String> {
        let world = self.shared.world_in(dimension_from_wit(pos.dimension)).lock().expect("мир захвачен другим потоком");

        if !world.has_chunk(pos.x.div_euclid(16), pos.z.div_euclid(16)) {
            return None;
        }

        Some(block_text(world.get_block(pos.x, pos.y, pos.z)))
    }

    fn set_block(&mut self, pos: wit::BlockPos, block: String) -> Result<(), String> {
        let state = crate::blocks::state_from_text(&block).ok_or_else(|| format!("нет такого блока: {block}"))?;
        let mut world = self.shared.world_in(dimension_from_wit(pos.dimension)).lock().expect("мир захвачен другим потоком");

        if !world.has_chunk(pos.x.div_euclid(16), pos.z.div_euclid(16)) {
            return Err("чанк не загружен".to_string());
        }

        let (x, y, z) = (pos.x, pos.y, pos.z);

        // Как команда `setblock`: после установки соседи и редстоун узнают
        // о перемене в этом же месте (commands.rs, set_blocks).
        if world.get_block(x, y, z) != state {
            world.note_commanded(x, y, z);
        }

        if world.set_block(x, y, z, state) {
            world.take_spilled();
            crate::containers::after_change(&mut world);
            crate::placing::refresh_neighbours(&mut world, x, y, z);
            crate::redstone::settle(&mut world);
            crate::redstone::command_placed(&mut world, (x, y, z));
            crate::redstone::settle(&mut world);
        }

        Ok(())
    }

    fn time(&mut self, dimension: wit::Dimension) -> u32 {
        self.shared.world_in(dimension_from_wit(dimension)).lock().expect("мир занят").time_of_day() as u32
    }

    fn set_time(&mut self, dimension: wit::Dimension, ticks: u32) {
        self.shared.world_in(dimension_from_wit(dimension)).lock().expect("мир занят").set_time_of_day(i64::from(ticks));
    }

    fn weather(&mut self, dimension: wit::Dimension) -> String {
        let world = self.shared.world_in(dimension_from_wit(dimension)).lock().expect("мир занят");
        if world.weather.thundering { "thunder" } else if world.weather.raining { "rain" } else { "clear" }.into()
    }

    fn set_weather(&mut self, dimension: wit::Dimension, kind: String, duration: u32) -> Result<(), String> {
        let kind = crate::weather::Kind::from_name(&kind).ok_or("погода: clear, rain, thunder")?;
        if !(1..=2_400_000).contains(&duration) { return Err("срок: от 1 до 2400000 тактов".into()); }
        self.shared.world_in(dimension_from_wit(dimension)).lock().expect("мир занят").weather.set(kind, duration as i32);
        Ok(())
    }

    fn spawn_location(&mut self, dimension: wit::Dimension) -> wit::Location {
        let (x, y, z) = self.shared.world_in(dimension_from_wit(dimension)).lock().expect("мир захвачен другим потоком").spawn_position();

        wit::Location { dimension, x, y, z, yaw: None, pitch: None }
    }
}

impl mcsa::api::scheduler::Host for PluginCtx {
    fn run_later(&mut self, delay: u32, task: u32) -> u32 {
        self.add_task(delay, None, task)
    }

    fn run_repeating(&mut self, delay: u32, period: u32, task: u32) -> u32 {
        self.add_task(delay, Some(period.max(1)), task)
    }

    fn cancel(&mut self, handle: u32) -> bool {
        let before = self.tasks.len();

        self.tasks.retain(|task| task.handle != handle);
        self.tasks.len() != before
    }
}

impl mcsa::api::command::Host for PluginCtx {
    fn register(&mut self, spec: mcsa::api::command::CommandSpec) -> Result<(), String> {
        let name = spec.name.to_ascii_lowercase();

        if !super::manifest::valid_name(&name) {
            return Err(format!("имя команды «{}» не годится: a-z, 0-9, «-», «_»", spec.name));
        }

        if crate::auth::sensitive_command(&name) || RESERVED.contains(&name.as_str()) || crate::commands_admin::NAMES.contains(&name.as_str()) {
            return Err(format!("команда «{name}» принадлежит ядру — другое имя"));
        }

        if self.commands.len() >= MAX_COMMANDS {
            return Err("слишком много команд".to_string());
        }

        if spec.level > crate::ops::HIGHEST_LEVEL as u8 {
            return Err("level: от 0 до 4".to_string());
        }

        let mut registry = self.registry.lock().expect("список команд захвачен другим потоком");

        if let Some(owner) = registry.get(&name) {
            return Err(format!("команду «{name}» уже занял плагин {owner}"));
        }

        registry.insert(name.clone(), self.name.clone());
        self.commands.push(Registered { name, description: spec.description, usage: spec.usage, level: spec.level });
        Ok(())
    }
}

impl mcsa::api::storage::Host for PluginCtx {
    fn config(&mut self, default_text: String) -> Result<String, String> {
        let path = self.folder.join("config.toml");

        match read_limited(&path)? {
            Some(bytes) => String::from_utf8(bytes).map_err(|_| "config.toml не в UTF-8".to_string()),
            None => {
                write_file(&path, default_text.as_bytes())?;
                Ok(default_text)
            }
        }
    }

    fn save_config(&mut self, text: String) -> Result<(), String> {
        write_file(&self.folder.join("config.toml"), text.as_bytes())
    }

    fn read(&mut self, name: String) -> Result<Option<Vec<u8>>, String> {
        read_limited(&data_path(&self.data_dir(), &name)?)
    }

    fn write(&mut self, name: String, data: Vec<u8>) -> Result<(), String> {
        write_file(&data_path(&self.data_dir(), &name)?, &data)
    }

    fn transfer(&mut self, source: String, target: String, replace: bool, move_file: bool) -> Result<(), String> {
        let source = data_path(&self.data_dir(), &source)?;
        let target = data_path(&self.data_dir(), &target)?;
        if source == target { return Ok(()); }
        if target.exists() && !replace { return Err("целевой файл уже существует".into()); }
        if move_file {
            if let Some(parent) = target.parent() { std::fs::create_dir_all(parent).map_err(|error| error.to_string())?; }
            std::fs::rename(source, target).map_err(|error| error.to_string())
        } else {
            let data = read_limited(&source)?.ok_or("исходный файл отсутствует")?;
            write_file(&target, &data)
        }
    }

    fn delete(&mut self, name: String) -> Result<bool, String> {
        match std::fs::remove_file(data_path(&self.data_dir(), &name)?) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.to_string()),
        }
    }
}

impl mcsa::api::privileged::Host for PluginCtx {
    fn run_console_command(&mut self, command: String) -> String {
        if !self.trusted {
            return "нет прав".to_string();
        }

        let line = command.trim().trim_start_matches('/');

        // Управлять плагинами из плагина нельзя: хозяин занят этим самым
        // вызовом.
        if line.split_whitespace().next() == Some("plugman") {
            return "plugman из плагина не вызывается".to_string();
        }
        // Аккаунты и пароли: даже доверенный плагин их не читает и не меняет.
        if crate::auth::sensitive_command(line) {
            return "команды auth из плагина не вызываются".to_string();
        }

        crate::commands::run(line, &crate::commands::Source::Console, &self.shared).reply
    }

    fn loaded_plugins(&mut self) -> Vec<String> {
        self.loaded.lock().expect("список плагинов захвачен другим потоком").clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_parses_both_spellings() {
        let uuid = parse_uuid("26d479c9-4844-3b0a-ba59-8cc656622a78").unwrap();

        assert_eq!(uuid[0], 0x26);
        assert_eq!(uuid[15], 0x78);
        assert_eq!(parse_uuid("26d479c948443b0aba598cc656622a78"), Some(uuid));
        assert_eq!(parse_uuid("zz"), None);
        assert_eq!(parse_uuid("26d479c9-4844-3b0a-ba59-8cc656622a7"), None);
        assert_eq!(parse_uuid("жжжжжжжжжжжжжжжжжжжжжжжжжжжжжжжж"), None);
    }

    #[test]
    fn file_names_stay_in_the_data_folder() {
        assert!(safe_file_name("saves.json").is_ok());
        assert!(safe_file_name("../x").is_err());
        assert!(safe_file_name("a/b").is_err());
        assert!(safe_file_name(".hidden").is_err());
        assert!(safe_file_name("").is_err());
        assert!(safe_file_name("c:evil").is_err());
    }

    #[test]
    fn relative_data_paths_reject_traversal_and_symlinks() {
        let root = std::env::temp_dir().join(format!("mcsa-path-test-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        assert_eq!(data_path(&root, "races/humans.yml").unwrap(), root.join("races/humans.yml"));
        for name in ["", "/absolute", "a/../b", "a//b", "a/", "../outside", "a/hidden:stream"] { assert!(data_path(&root, name).is_err(), "{name}"); }
        #[cfg(unix)] {
            std::os::unix::fs::symlink(std::env::temp_dir(), root.join("escape")).unwrap();
            assert!(data_path(&root, "escape/file").is_err());
            std::fs::remove_file(root.join("escape")).unwrap();
        }
        std::fs::remove_dir(root).unwrap();
    }

    #[test]
    fn reserved_commands_are_sorted_out() {
        assert!(RESERVED.contains(&"op"));
        assert!(RESERVED.contains(&"plugman"));
    }

    #[test]
    fn files_are_written_whole() {
        let folder = std::env::temp_dir().join(format!("mcsa-api-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&folder);
        let file = folder.join("a.bin");

        write_file(&file, b"abc").unwrap();
        assert_eq!(read_limited(&file).unwrap().unwrap(), b"abc");
        assert_eq!(read_limited(&folder.join("none")).unwrap(), None);
        assert!(write_file(&file, &vec![0; MAX_FILE + 1]).is_err());
        let _ = std::fs::remove_dir_all(&folder);
    }
}
