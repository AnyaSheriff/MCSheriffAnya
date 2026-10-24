// Хозяин плагинов: загрузка, выгрузка и вызовы.
//
// Каждый плагин — свой `Store` wasmtime: своя память, свои задачи и команды.
// Перед каждым вызовом в плагин ему выдаётся топливо (счёт команд wasm) и
// срок по часам (эпохи); кончилось топливо или вышел срок — вызов обрывается
// ловушкой, и плагин выгружается с записью в лог. Память ограничена на
// весь `Store`. Так плагин не может ни зависнуть в такте, ни съесть память
// сервера.
//
// Все вызовы идут под одним замком хозяина: плагины друг с другом не
// спорят, а порядок вызовов — порядок загрузки.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use wasmtime::component::{Component, HasSelf, Linker, ResourceTable};
use wasmtime::{Config, Engine, Store, StoreLimitsBuilder};
use wasmtime_wasi::WasiCtxBuilder;

use super::api::{CommandRegistry, Module, PluginCtx, mcsa};
use super::manifest::{self, Manifest};
use super::{ChatOutcome, PlayerView, Verdict};
use crate::shared::Shared;
use crate::{log_debug, log_error, log_warn};

/// Папка обычных плагинов (песочница) и доверенных модулей.
pub const PLUGINS_DIR: &str = "plugins";
pub const MODULES_DIR: &str = "modules";

/// Лимиты вызова. Топливо — примерно по команде wasm на единицу; миллион
/// единиц — порядка миллисекунды.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// Топливо на один вызов.
    pub fuel: u64,
    /// Срок вызова по часам, в «эпохах» по `EPOCH` каждая.
    pub epochs: u64,
    /// Память плагина, байт.
    pub memory: usize,
}

/// Лимит включает работу управляемого рантайма: полный GC TeaVM на реальном
/// наборе конфигураций требует до 190 млн команд вместе с обработчиком.
/// Запас ограничен 240 млн; память и срок 200 мс остаются независимыми границами.
/// Срок по часам щедрый
/// (200 мс): на одноядерной машине система может ненадолго снять процесс с
/// ядра, и при сроке в 20 мс исправный плагин выгружался бы за чужую паузу.
pub const PLUGIN_LIMITS: Limits = Limits { fuel: 240_000_000, epochs: 20, memory: 32 << 20 };

/// Лимиты модуля: он доверенный и может делать больше.
pub const MODULE_LIMITS: Limits = Limits { fuel: 400_000_000, epochs: 20, memory: 256 << 20 };

/// Длительность эпохи часов.
const EPOCH: Duration = Duration::from_millis(10);

// Не более секунды на подготовку чанков и файловый ввод-вывод ядра; время гостя ограничено отдельно.
const NATIVE_WAIT_EPOCHS: u64 = 100;
#[derive(Default)]
pub(super) struct NativeWait { active: AtomicBool, left: AtomicU64 }
impl NativeWait {
    fn reset(&self) { self.left.store(NATIVE_WAIT_EPOCHS,Ordering::Relaxed); }
    pub(super) fn enter(&self) -> NativeWaitGuard<'_> { self.active.store(true,Ordering::Relaxed);NativeWaitGuard(self) }
    fn consume(&self) -> bool { self.active.load(Ordering::Relaxed) && self.left.fetch_update(Ordering::Relaxed,Ordering::Relaxed,|left|left.checked_sub(1)).is_ok() }
}
pub(super) struct NativeWaitGuard<'a>(&'a NativeWait);
impl Drop for NativeWaitGuard<'_> { fn drop(&mut self) { self.0.active.store(false,Ordering::Relaxed); } }


/// Самый большой `.wasm`.
const MAX_WASM: u64 = 64 << 20;

thread_local! {
    /// Этот поток сейчас внутри вызова плагина. Из плагина нельзя снова
    /// войти в хозяина (команда, событие): замок уже его.
    static INSIDE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Находится ли поток внутри вызова плагина.
pub fn inside_plugin() -> bool {
    INSIDE.with(|flag| flag.get())
}

struct InsideGuard;

impl InsideGuard {
    fn enter() -> Self {
        INSIDE.with(|flag| flag.set(true));
        InsideGuard
    }
}

impl Drop for InsideGuard {
    fn drop(&mut self) {
        INSIDE.with(|flag| flag.set(false));
    }
}

type Completion = wasmtime::component::TypedFunc<(mcsa::api::types::Sender, String, Vec<String>), (Vec<String>,)>;
type DamageFrom = wasmtime::component::TypedFunc<(mcsa::api::types::Player, Option<mcsa::api::types::Player>, String, f64), (Option<f64>,)>;
type Damage = wasmtime::component::TypedFunc<(mcsa::api::types::Player, String, f64), (Option<f64>,)>;
type DragEvent = wasmtime::component::TypedFunc<(mcsa::api::types::Player, u32, Vec<i32>), (bool,)>;
type EntityInteract = wasmtime::component::TypedFunc<(mcsa::api::types::Player, String, u32, Option<u32>), (bool,)>;
type PrepareEnchant = wasmtime::component::TypedFunc<(mcsa::api::types::Player, mcsa::api::inventory::Item, Vec<i32>), (Vec<i32>,)>;
type EnchantEvent = wasmtime::component::TypedFunc<(mcsa::api::types::Player, mcsa::api::inventory::Item, mcsa::api::inventory::Item, u32, i32, String, i32), (Option<mcsa::api::inventory::Item>,)>;
type WantsPopulation = wasmtime::component::TypedFunc<(mcsa::api::types::Dimension,), (bool,)>;
type Populate = wasmtime::component::TypedFunc<(mcsa::api::types::Dimension,i32,i32,i64,Vec<String>,Vec<u32>), (Vec<String>,)>;
type WorldInit = wasmtime::component::TypedFunc<(mcsa::api::types::Dimension,), ()>;
type GeneratedLoot = wasmtime::component::TypedFunc<(mcsa::api::types::BlockPos,Vec<Option<mcsa::api::inventory::Item>>),(Vec<Option<mcsa::api::inventory::Item>>,)>;
type EntityDeath = wasmtime::component::TypedFunc<(Option<mcsa::api::types::Player>,String,bool,Vec<mcsa::api::inventory::Item>),(Vec<mcsa::api::inventory::Item>,)>;
type NativeEvent = wasmtime::component::TypedFunc<(u32,Vec<String>),(Vec<String>,)>;
type BlockDropEvent = wasmtime::component::TypedFunc<(mcsa::api::types::Player,mcsa::api::types::BlockPos,String,Vec<String>),(Vec<String>,)>;
type GroundItemEvent = wasmtime::component::TypedFunc<(mcsa::api::types::Player,String,bool,i32),(bool,)>;
type FurnaceEvent = wasmtime::component::TypedFunc<(mcsa::api::types::BlockPos, i32), (i32,)>;
type TradeEvent = wasmtime::component::TypedFunc<(mcsa::api::types::Player, bool, Vec<String>, Vec<String>), (Vec<i32>,)>;
type FoodEvent = wasmtime::component::TypedFunc<(mcsa::api::types::Player, i32, Option<mcsa::api::inventory::Item>), (Option<i32>,)>;
type PrepareAnvil = wasmtime::component::TypedFunc<(mcsa::api::types::Player, Vec<Option<mcsa::api::inventory::Item>>, Option<mcsa::api::inventory::Item>, i32), (i32,)>;
type PrepareCraft = wasmtime::component::TypedFunc<(mcsa::api::types::Player, Vec<Option<mcsa::api::inventory::Item>>, Option<mcsa::api::inventory::Item>, bool, bool), (Option<mcsa::api::inventory::Item>,)>;
type ItemEvent = wasmtime::component::TypedFunc<(mcsa::api::types::Player, u32, i32), (bool,)>;
type InventoryEvent = wasmtime::component::TypedFunc<(mcsa::api::types::Player, u32, u32, i32, i32, i32), (bool,)>;
type RespawnEvent = wasmtime::component::TypedFunc<(mcsa::api::types::Player, mcsa::api::types::Location, bool, bool), (mcsa::api::types::Location,)>;
type MoveEvent = wasmtime::component::TypedFunc<(mcsa::api::types::Player, mcsa::api::types::Location, bool), (Option<mcsa::api::types::Location>,)>;
type InteractEvent = wasmtime::component::TypedFunc<(mcsa::api::types::Player, u32, u32, Option<mcsa::api::types::BlockPos>, u32), (u32,)>;
type Death = wasmtime::component::TypedFunc<(mcsa::api::types::Player, bool), ()>;

/// Загруженный плагин.
pub struct Loaded {
    pub manifest: Manifest,
    pub trusted: bool,
    pub store: Store<PluginCtx>,
    instance: Module,
    completion: Option<Completion>,
    damage: Option<Damage>,
    damage_from: Option<DamageFrom>,
    death: Option<Death>,
    inventory_event: Option<InventoryEvent>,
    item_event: Option<ItemEvent>,
    prepare_craft: Option<PrepareCraft>,
    prepare_anvil: Option<PrepareAnvil>,
    prepare_enchant: Option<PrepareEnchant>,
    enchant_event: Option<EnchantEvent>,
    food_event: Option<FoodEvent>,
    trade_event: Option<TradeEvent>,
    furnace_event: Option<FurnaceEvent>,
    ground_item: Option<GroundItemEvent>,
    block_drop: Option<BlockDropEvent>,
    native_event: Option<NativeEvent>,
    entity_death: Option<EntityDeath>,
    generated_loot: Option<GeneratedLoot>,
    wants_population: Option<WantsPopulation>,
    populate: Option<Populate>,
    world_init: Option<WorldInit>,
    entity_interact: Option<EntityInteract>,
    drag_event: Option<DragEvent>,
    move_event: Option<MoveEvent>,
    respawn_event: Option<RespawnEvent>,
    interact_event: Option<InteractEvent>,
    limits: Limits,
    pub stats: Stats,
}

/// Сколько плагин тратит: вызовов, всего времени, самый долгий вызов.
#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    pub calls: u64,
    pub nanos: u64,
    pub worst: u64,
}

impl Stats {
    fn add(&mut self, nanos: u64) {
        self.calls += 1;
        self.nanos += nanos;
        self.worst = self.worst.max(nanos);
    }

    /// Среднее на вызов, мкс.
    pub fn average_micros(&self) -> f64 {
        if self.calls == 0 { 0.0 } else { self.nanos as f64 / self.calls as f64 / 1000.0 }
    }
}

/// Хозяин.
pub struct Host {
    engine: Engine,
    sandbox: Linker<PluginCtx>,
    trusted: Linker<PluginCtx>,
    pub loaded: Vec<Loaded>,
    shared: Arc<Shared>,
    /// Корень: где лежат `plugins/` и `modules/` (рабочая папка сервера).
    root: PathBuf,
    registry: CommandRegistry,
    names: Arc<Mutex<Vec<String>>>,
    epochs: Option<Arc<AtomicBool>>,
    native_wait: Arc<NativeWait>,
    /// Сколько хозяин тратит на такт целиком (задачи всех плагинов).
    pub tick_stats: Stats,
}

/// Результат попытки загрузить.
pub type Outcome = Result<String, String>;

/// Падение плагина во время вызова.
struct Crash {
    plugin: String,
    reason: String,
}

fn linkers(engine: &Engine) -> wasmtime::Result<(Linker<PluginCtx>, Linker<PluginCtx>)> {
    fn common(linker: &mut Linker<PluginCtx>) -> wasmtime::Result<()> {
        // Системный интерфейс WASI нужен самой Rust-среде плагина (разметка
        // памяти, паника, случайное семя); прав он не даёт: ни файлов, ни
        // сети, ни окружения, ни ввода — `WasiCtx` пустой.
        wasmtime_wasi::p2::add_to_linker_sync(linker)?;

        mcsa::api::types::add_to_linker::<PluginCtx, HasSelf<PluginCtx>>(linker, |ctx| ctx)?;
        mcsa::api::log::add_to_linker::<PluginCtx, HasSelf<PluginCtx>>(linker, |ctx| ctx)?;
        mcsa::api::server::add_to_linker::<PluginCtx, HasSelf<PluginCtx>>(linker, |ctx| ctx)?;
        mcsa::api::presentation::add_to_linker::<PluginCtx, HasSelf<PluginCtx>>(linker, |ctx| ctx)?;
        mcsa::api::player::add_to_linker::<PluginCtx, HasSelf<PluginCtx>>(linker, |ctx| ctx)?;
        mcsa::api::world::add_to_linker::<PluginCtx, HasSelf<PluginCtx>>(linker, |ctx| ctx)?;
        mcsa::api::scheduler::add_to_linker::<PluginCtx, HasSelf<PluginCtx>>(linker, |ctx| ctx)?;
        mcsa::api::command::add_to_linker::<PluginCtx, HasSelf<PluginCtx>>(linker, |ctx| ctx)?;
        mcsa::api::attributes::add_to_linker::<PluginCtx, HasSelf<PluginCtx>>(linker, |ctx| ctx)?;
        mcsa::api::inventory::add_to_linker::<PluginCtx, HasSelf<PluginCtx>>(linker, |ctx| ctx)?;
        mcsa::api::entities::add_to_linker::<PluginCtx, HasSelf<PluginCtx>>(linker, |ctx| ctx)?;
        mcsa::api::https::add_to_linker::<PluginCtx, HasSelf<PluginCtx>>(linker, |ctx| ctx)?;
        mcsa::api::storage::add_to_linker::<PluginCtx, HasSelf<PluginCtx>>(linker, |ctx| ctx)?;
        Ok(())
    }

    let mut sandbox = Linker::new(engine);
    common(&mut sandbox)?;

    let mut trusted = Linker::new(engine);
    common(&mut trusted)?;
    // Только модулям. Здесь же встанут глубокие зацепки (пакеты, механики,
    // генерация мира) — отдельными интерфейсами рядом с `privileged`.
    mcsa::api::privileged::add_to_linker::<PluginCtx, HasSelf<PluginCtx>>(&mut trusted, |ctx| ctx)?;

    Ok((sandbox, trusted))
}

impl Host {
    pub fn new(shared: Arc<Shared>, root: PathBuf) -> wasmtime::Result<Host> {
        let mut config = Config::new();

        // Исключения нужны Java/TeaVM. Управляемые объекты WASM выключены:
        // куча Java остаётся линейной и под прежним StoreLimits.
        config.wasm_gc(false);
        config.wasm_function_references(false);
        config.wasm_exceptions(true);
        config.consume_fuel(true);
        config.epoch_interruption(true);
        // Компиляция в машинный код на загрузке; кэша на диске нет — плагины
        // небольшие, а лишний файл — лишнее, за чем следить.
        config.cranelift_opt_level(wasmtime::OptLevel::Speed);

        let engine = Engine::new(&config)?;
        let (sandbox, trusted) = linkers(&engine)?;

        Ok(Host {
            engine,
            sandbox,
            trusted,
            loaded: Vec::new(),
            shared,
            root,
            registry: Arc::new(Mutex::new(HashMap::new())),
            names: Arc::new(Mutex::new(Vec::new())),
            epochs: None,
            native_wait: Arc::new(NativeWait::default()),
            tick_stats: Stats::default(),
        })
    }

    /// Запускает часы эпох при первой загрузке плагина (без плагинов поток
    /// не нужен).
    fn ensure_clock(&mut self) {
        if self.epochs.is_some() {
            return;
        }

        let stop = Arc::new(AtomicBool::new(false));
        let engine = self.engine.clone();
        let native_wait = Arc::clone(&self.native_wait);
        let flag = Arc::clone(&stop);

        let started = std::thread::Builder::new().name("plugin clock".into()).spawn(move || {
            while !flag.load(Ordering::Relaxed) {
                std::thread::sleep(EPOCH);
                if !native_wait.consume() { engine.increment_epoch(); }
            }
        });

        if let Err(error) = started {
            log_error!("Плагины: не запустились часы вызовов: {}", error);
        }

        self.epochs = Some(stop);
    }

    pub fn is_empty(&self) -> bool {
        self.loaded.is_empty()
    }

    fn publish_names(&self) {
        *self.names.lock().expect("список плагинов захвачен другим потоком") = self.loaded.iter().map(|plugin| plugin.manifest.name.clone()).collect();
    }

    /// Где лежит плагин `name`: сперва модули, потом плагины.
    fn find_folder(&self, name: &str) -> Option<(PathBuf, bool)> {
        if !manifest::valid_name(name) {
            return None;
        }

        [(MODULES_DIR, true), (PLUGINS_DIR, false)].into_iter().map(|(dir, trusted)| (self.root.join(dir).join(name), trusted)).find(|(folder, _)| folder.join("plugin.toml").is_file())
    }

    /// Все плагины на диске: (имя, доверенный).
    pub fn available(&self) -> Vec<(String, bool)> {
        available_in(&self.root)
    }

    pub fn is_loaded(&self, name: &str) -> bool {
        self.loaded.iter().any(|plugin| plugin.manifest.name == name)
    }

    /// Загружает плагин по имени папки.
    pub fn load(&mut self, name: &str) -> Outcome {
        if self.is_loaded(name) {
            return Err(format!("{name} уже загружен"));
        }

        let (folder, trusted) = self.find_folder(name).ok_or_else(|| format!("нет такого плагина: {name} (ищу в {MODULES_DIR}/ и {PLUGINS_DIR}/)"))?;

        self.load_from(&folder, trusted)
    }

    fn load_from(&mut self, folder: &Path, trusted: bool) -> Outcome {
        let manifest = Manifest::read(folder)?;
        let folder_name = folder.file_name().and_then(|name| name.to_str()).unwrap_or("");

        if manifest.name != folder_name {
            return Err(format!("имя в plugin.toml («{}») не совпадает с папкой («{folder_name}»)", manifest.name));
        }

        if self.is_loaded(&manifest.name) {
            return Err(format!("{} уже загружен", manifest.name));
        }

        if let Some(missing) = manifest.dependencies.iter().find(|dependency| !self.is_loaded(dependency)) {
            return Err(format!("{}: сперва нужен плагин {missing}", manifest.name));
        }

        let wasm_path = folder.join(manifest.wasm_file());
        let size = std::fs::metadata(&wasm_path).map_err(|error| format!("{}: {error}", manifest.wasm_file()))?.len();

        if size > MAX_WASM {
            return Err(format!("{}: файл больше {} МиБ", manifest.wasm_file(), MAX_WASM >> 20));
        }

        let bytes = std::fs::read(&wasm_path).map_err(|error| format!("{}: {error}", manifest.wasm_file()))?;
        let component = Component::new(&self.engine, &bytes).map_err(|error| format!("{}: не wasm-компонент: {}", manifest.wasm_file(), short(&error)))?;

        let limits = if trusted { MODULE_LIMITS } else { PLUGIN_LIMITS };

        let network = super::network::Access::new(&manifest.network_hosts, self.shared.settings.plugin_network_hosts.get(&manifest.name).map(Vec::as_slice).unwrap_or_default());
        crate::log_info!("[{}] HTTPS: {}", manifest.name, network.description());
        let ctx = PluginCtx {
            native_wait: Arc::clone(&self.native_wait),
            pending_mobs: HashMap::new(),
            network,
            chests: Default::default(),
            native_items: Default::default(),
            name: manifest.name.clone(),
            trusted,
            shared: Arc::clone(&self.shared),
            folder: folder.to_path_buf(),
            // Пустой WASI: ни стандартных потоков, ни файлов, ни окружения.
            wasi: WasiCtxBuilder::new().build(),
            table: ResourceTable::new(),
            limits: StoreLimitsBuilder::new().memory_size(limits.memory).instances(32).memories(8).tables(32).table_elements(100_000).trap_on_grow_failure(true).build(),
            tick: 0,
            tasks: Vec::new(),
            next_handle: 0,
            commands: Vec::new(),
            registry: Arc::clone(&self.registry),
            loaded: Arc::clone(&self.names),
        };

        let mut store = Store::new(&self.engine, ctx);

        store.limiter(|ctx| &mut ctx.limits);
        store.set_fuel(limits.fuel).map_err(|error| error.to_string())?;
        store.set_epoch_deadline(limits.epochs);
        store.epoch_deadline_trap();

        self.ensure_clock();

        let linker = if trusted { &self.trusted } else { &self.sandbox };
        let raw = {
            let _inside = InsideGuard::enter();

            // Проверяем обязательные экспорты до исполнения стартового кода гостя.
            let prepared = linker.instantiate_pre(&component);
            let validated = prepared.and_then(|pre| {
                super::api::ModulePre::new(pre.clone())?;
                pre.instantiate(&mut store)
            });
            validated.map_err(|error| {
                let text = short(&error);

                if text.contains("privileged") {
                    format!("{}: плагину нужен интерфейс модулей (privileged), а он лежит в {PLUGINS_DIR}/ — перенеси в {MODULES_DIR}/", manifest.name)
                } else {
                    format!("{}: не запустился: {text}", manifest.name)
                }
            })?
        };

        let instance = Module::new(&mut store, &raw).map_err(|error| short(&error))?;
        let completion = raw.get_export_index(&mut store, None, "mcsa:api/completion@0.1.0")
            .and_then(|parent| raw.get_export_index(&mut store, Some(&parent), "on-tab-complete"))
            .map(|index| raw.get_typed_func::<(mcsa::api::types::Sender, String, Vec<String>), (Vec<String>,)>(&mut store, &index))
            .transpose().map_err(|error| {
                self.drop_registrations(&manifest.name);
                crate::presentation::release(&self.shared, &manifest.name);
                short(&error)
            })?;
        let combat = raw.get_export_index(&mut store, None, "mcsa:api/combat-events@0.1.0");
        let damage_from = combat.as_ref().and_then(|parent| raw.get_export_index(&mut store, Some(parent), "on-damage-from"))
            .map(|index| raw.get_typed_func::<(mcsa::api::types::Player, Option<mcsa::api::types::Player>, String, f64), (Option<f64>,)>(&mut store, &index))
            .transpose().map_err(|error| {
                self.drop_registrations(&manifest.name);
                crate::presentation::release(&self.shared, &manifest.name);
                short(&error)
            })?;
        let damage = combat.as_ref().and_then(|parent| raw.get_export_index(&mut store, Some(parent), "on-damage"))
            .map(|index| raw.get_typed_func::<(mcsa::api::types::Player, String, f64), (Option<f64>,)>(&mut store, &index))
            .transpose().map_err(|error| {
                self.drop_registrations(&manifest.name);
                crate::presentation::release(&self.shared, &manifest.name);
                short(&error)
            })?;
        let death = combat.as_ref().and_then(|parent| raw.get_export_index(&mut store, Some(parent), "on-death"))
            .map(|index| raw.get_typed_func::<(mcsa::api::types::Player, bool), ()>(&mut store, &index))
            .transpose().map_err(|error| {
                self.drop_registrations(&manifest.name);
                crate::presentation::release(&self.shared, &manifest.name);
                short(&error)
            })?;
        let drag_event = raw.get_export_index(&mut store, None, "mcsa:api/item-events@0.1.0")
            .and_then(|parent| raw.get_export_index(&mut store, Some(&parent), "on-drag"))
            .map(|index| raw.get_typed_func::<(mcsa::api::types::Player, u32, Vec<i32>), (bool,)>(&mut store, &index))
            .transpose().map_err(|error| {
                self.drop_registrations(&manifest.name);
                crate::presentation::release(&self.shared, &manifest.name);
                short(&error)
            })?;
        let entity_interact = raw.get_export_index(&mut store, None, "mcsa:api/item-events@0.1.0")
            .and_then(|parent|raw.get_export_index(&mut store,Some(&parent),"on-entity"))
            .map(|index|raw.get_typed_func::<(mcsa::api::types::Player,String,u32,Option<u32>),(bool,)>(&mut store,&index)).transpose().map_err(|error|short(&error))?;
        let prepare_craft = raw.get_export_index(&mut store,None,"mcsa:api/item-events@0.1.0")
            .and_then(|parent|raw.get_export_index(&mut store,Some(&parent),"on-prepare-craft"))
            .map(|index|raw.get_typed_func::<(mcsa::api::types::Player,Vec<Option<mcsa::api::inventory::Item>>,Option<mcsa::api::inventory::Item>,bool,bool),(Option<mcsa::api::inventory::Item>,)>(&mut store,&index)).transpose().map_err(|error|short(&error))?;
        let prepare_anvil = raw.get_export_index(&mut store,None,"mcsa:api/item-events@0.1.0")
            .and_then(|parent|raw.get_export_index(&mut store,Some(&parent),"on-prepare-anvil"))
            .map(|index|raw.get_typed_func::<(mcsa::api::types::Player,Vec<Option<mcsa::api::inventory::Item>>,Option<mcsa::api::inventory::Item>,i32),(i32,)>(&mut store,&index)).transpose().map_err(|error|short(&error))?;
        let prepare_enchant = raw.get_export_index(&mut store,None,"mcsa:api/item-events@0.1.0")
            .and_then(|parent|raw.get_export_index(&mut store,Some(&parent),"on-prepare-enchant"))
            .map(|index|raw.get_typed_func::<(mcsa::api::types::Player,mcsa::api::inventory::Item,Vec<i32>),(Vec<i32>,)>(&mut store,&index)).transpose().map_err(|error|short(&error))?;
        let enchant_event = raw.get_export_index(&mut store,None,"mcsa:api/item-events@0.1.0")
            .and_then(|parent|raw.get_export_index(&mut store,Some(&parent),"on-enchant"))
            .map(|index|raw.get_typed_func::<(mcsa::api::types::Player,mcsa::api::inventory::Item,mcsa::api::inventory::Item,u32,i32,String,i32),(Option<mcsa::api::inventory::Item>,)>(&mut store,&index)).transpose().map_err(|error|short(&error))?;
        let generation=raw.get_export_index(&mut store,None,"mcsa:api/generation-events@0.1.0");
        let generated_loot=generation.as_ref().and_then(|p|raw.get_export_index(&mut store,Some(p),"on-loot")).map(|i|raw.get_typed_func::<(mcsa::api::types::BlockPos,Vec<Option<mcsa::api::inventory::Item>>),(Vec<Option<mcsa::api::inventory::Item>>,)>(&mut store,&i)).transpose().map_err(|e|short(&e))?;
        let wants_population=generation.as_ref().and_then(|p|raw.get_export_index(&mut store,Some(p),"wants-population")).map(|i|raw.get_typed_func::<(mcsa::api::types::Dimension,),(bool,)>(&mut store,&i)).transpose().map_err(|e|short(&e))?;
        let populate=generation.as_ref().and_then(|p|raw.get_export_index(&mut store,Some(p),"on-populate")).map(|i|raw.get_typed_func::<(mcsa::api::types::Dimension,i32,i32,i64,Vec<String>,Vec<u32>),(Vec<String>,)>(&mut store,&i)).transpose().map_err(|e|short(&e))?;
        let world_init=generation.as_ref().and_then(|p|raw.get_export_index(&mut store,Some(p),"on-world-init")).map(|i|raw.get_typed_func::<(mcsa::api::types::Dimension,),()>(&mut store,&i)).transpose().map_err(|e|short(&e))?;
        let entity_death=raw.get_export_index(&mut store,None,"mcsa:api/item-events@0.1.0").and_then(|p|raw.get_export_index(&mut store,Some(&p),"on-entity-death")).map(|i|raw.get_typed_func::<(Option<mcsa::api::types::Player>,String,bool,Vec<mcsa::api::inventory::Item>),(Vec<mcsa::api::inventory::Item>,)>(&mut store,&i)).transpose().map_err(|e|short(&e))?;
        let native_event=raw.get_export_index(&mut store,None,"mcsa:api/native-events@0.1.0").and_then(|p|raw.get_export_index(&mut store,Some(&p),"on-event")).map(|i|raw.get_typed_func::<(u32,Vec<String>),(Vec<String>,)>(&mut store,&i)).transpose().map_err(|e|short(&e))?;
        let block_drop=raw.get_export_index(&mut store,None,"mcsa:api/item-events@0.1.0").and_then(|p|raw.get_export_index(&mut store,Some(&p),"on-block-drop")).map(|i|raw.get_typed_func::<(mcsa::api::types::Player,mcsa::api::types::BlockPos,String,Vec<String>),(Vec<String>,)>(&mut store,&i)).transpose().map_err(|e|short(&e))?;
        let ground_item=raw.get_export_index(&mut store,None,"mcsa:api/item-events@0.1.0").and_then(|p|raw.get_export_index(&mut store,Some(&p),"on-ground-item")).map(|i|raw.get_typed_func::<(mcsa::api::types::Player,String,bool,i32),(bool,)>(&mut store,&i)).transpose().map_err(|e|short(&e))?;
        let furnace_event = raw.get_export_index(&mut store,None,"mcsa:api/item-events@0.1.0")
            .and_then(|parent|raw.get_export_index(&mut store,Some(&parent),"on-furnace"))
            .map(|index|raw.get_typed_func::<(mcsa::api::types::BlockPos,i32),(i32,)>(&mut store,&index)).transpose().map_err(|error|short(&error))?;
        let trade_event = raw.get_export_index(&mut store,None,"mcsa:api/trading-events@0.1.0")
            .and_then(|parent|raw.get_export_index(&mut store,Some(&parent),"on-trade"))
            .map(|index|raw.get_typed_func::<(mcsa::api::types::Player,bool,Vec<String>,Vec<String>),(Vec<i32>,)>(&mut store,&index)).transpose().map_err(|error|short(&error))?;
        let food_event = raw.get_export_index(&mut store,None,"mcsa:api/item-events@0.1.0")
            .and_then(|parent|raw.get_export_index(&mut store,Some(&parent),"on-food"))
            .map(|index|raw.get_typed_func::<(mcsa::api::types::Player,i32,Option<mcsa::api::inventory::Item>),(Option<i32>,)>(&mut store,&index)).transpose().map_err(|error|short(&error))?;
        let item_event = raw.get_export_index(&mut store, None, "mcsa:api/item-events@0.1.0")
            .and_then(|parent| raw.get_export_index(&mut store, Some(&parent), "on-item"))
            .map(|index| raw.get_typed_func::<(mcsa::api::types::Player, u32, i32), (bool,)>(&mut store, &index))
            .transpose().map_err(|error| {
                self.drop_registrations(&manifest.name);
                crate::presentation::release(&self.shared, &manifest.name);
                short(&error)
            })?;
        let inventory_event = raw.get_export_index(&mut store, None, "mcsa:api/inventory-events@0.1.0")
            .and_then(|parent| raw.get_export_index(&mut store, Some(&parent), "on-inventory"))
            .map(|index| raw.get_typed_func::<(mcsa::api::types::Player, u32, u32, i32, i32, i32), (bool,)>(&mut store, &index))
            .transpose().map_err(|error| short(&error))?;
        let player_events = raw.get_export_index(&mut store, None, "mcsa:api/player-events@0.1.0");
        let move_event = player_events.as_ref().and_then(|parent| raw.get_export_index(&mut store, Some(parent), "on-move"))
            .map(|index| raw.get_typed_func::<(mcsa::api::types::Player, mcsa::api::types::Location, bool), (Option<mcsa::api::types::Location>,)>(&mut store, &index))
            .transpose().map_err(|error| short(&error))?;
        let respawn_event = player_events.as_ref().and_then(|parent| raw.get_export_index(&mut store, Some(parent), "on-respawn"))
            .map(|index| raw.get_typed_func::<(mcsa::api::types::Player, mcsa::api::types::Location, bool, bool), (mcsa::api::types::Location,)>(&mut store, &index))
            .transpose().map_err(|error| short(&error))?;
        let interact_event = player_events.as_ref().and_then(|parent| raw.get_export_index(&mut store, Some(parent), "on-interact"))
            .map(|index| raw.get_typed_func::<(mcsa::api::types::Player, u32, u32, Option<mcsa::api::types::BlockPos>, u32), (u32,)>(&mut store, &index))
            .transpose().map_err(|error| short(&error))?;
        let mut plugin = Loaded { manifest, trusted, store, instance, completion, damage, damage_from, death, inventory_event, item_event, prepare_craft, prepare_anvil, prepare_enchant, enchant_event, food_event, trade_event, furnace_event, ground_item, block_drop, native_event, entity_death, generated_loot, wants_population, populate, world_init, entity_interact, drag_event, move_event, respawn_event, interact_event, limits, stats: Stats::default() };

        // enable: здесь плагин регистрирует команды и задачи. Упал — не грузим.
        // Холодная инициализация классов и конфигурации не входит в бюджет такта.
        // Память и права прежние; отдельный конечный предел — 400 млн / 2 с.
        let startup_fuel=plugin.limits.fuel.max(400_000_000);let startup_epochs=plugin.limits.epochs.max(200);
        if let Err(reason) = plugin.call(self.shared_tick(), |instance, store| {store.set_fuel(startup_fuel)?;store.set_epoch_deadline(startup_epochs);instance.mcsa_api_lifecycle().call_enable(store)}) {
            crate::presentation::release(&self.shared, &plugin.manifest.name);
            self.drop_registrations(&plugin.manifest.name);
            return Err(format!("{}: enable не удался: {reason}", plugin.manifest.name));
        }

        let summary = format!("{} {}{}", plugin.manifest.name, plugin.manifest.version, if trusted { " (модуль)" } else { "" });

        self.loaded.push(plugin);
        self.publish_names();
        give_back_memory();

        Ok(summary)
    }

    fn shared_tick(&self) -> u64 {
        TICK.load(Ordering::Relaxed)
    }

    fn drop_registrations(&self, name: &str) {
        let mut players=self.shared.players.lock().expect("игроки");let mut resets=Vec::new();
        let modes:HashMap<_,_>=players.members().iter().map(|p|(p.uuid,p.game_mode)).collect();
        for (uuid,info) in &mut players.ui.info {for field in 0..3 {if info.plugin_state.flight_owners[field].as_deref()==Some(name) {
            info.plugin_state.flight_owners[field]=None;
            match field {0=>{info.plugin_state.flight_allowed=modes.get(uuid).is_some_and(|mode|matches!(mode,1|3));if !info.plugin_state.flight_allowed {info.plugin_state.flying=false;}},1=>info.plugin_state.flying=false,_=>info.plugin_state.flight_fall_damage=None}
            resets.push((*uuid,field as u8));
        }}}
        let mut silent_resets=Vec::new();for (uuid,info) in &mut players.ui.info {if info.plugin_state.silent_owner.as_deref()==Some(name) {info.plugin_state.silent=false;info.plugin_state.silent_owner=None;silent_resets.push(*uuid);}}
        for uuid in silent_resets {players.order(&uuid,crate::players::Order::PluginSilent {owner:name.into(),value:None});}
        for (uuid,field) in resets {players.order(&uuid,crate::players::Order::PluginFlight {owner:name.into(),field,value: -2});}drop(players);

        for dimension in crate::dimension::Dimension::ALL {self.shared.world_in(dimension).lock().expect("мир").plugin_monster_limits.retain(|(owner,_)|owner!=name); }
        self.registry.lock().expect("список команд захвачен другим потоком").retain(|_, owner| owner != name);
    }

    /// Кто зависит от `name` (прямо или через других), в порядке загрузки.
    pub fn dependents(&self, name: &str) -> Vec<String> {
        let mut found: Vec<String> = vec![name.to_string()];

        for plugin in &self.loaded {
            if plugin.manifest.dependencies.iter().any(|dependency| found.contains(dependency)) {
                found.push(plugin.manifest.name.clone());
            }
        }

        found.remove(0);
        found
    }

    /// Выгружает плагин; от него не должны зависеть загруженные.
    pub fn unload(&mut self, name: &str) -> Outcome {
        if !self.is_loaded(name) {
            return Err(format!("{name} не загружен"));
        }

        let dependents = self.dependents(name);

        if !dependents.is_empty() {
            return Err(format!("от {name} зависят: {} — сперва выгрузи их", dependents.join(", ")));
        }

        self.remove(name, true);
        Ok(format!("{name} выгружен"))
    }

    /// Убирает плагин. `polite` — позвать `disable` (после падения нельзя).
    fn remove(&mut self, name: &str, polite: bool) {
        let Some(index) = self.loaded.iter().position(|plugin| plugin.manifest.name == name) else { return };
        let mut plugin = self.loaded.remove(index);

        if polite && let Err(reason) = plugin.call(self.shared_tick(), |instance, store| instance.mcsa_api_lifecycle().call_disable(store)) {
            log_warn!("Плагин {}: disable не удался: {}", name, reason);
        }

        crate::presentation::release(&self.shared, name);
        self.drop_registrations(name);
        self.publish_names();
        // Всё остальное — задачи, команды, память — уходит вместе со `Store`.
    }

    /// Перезагружает плагин и тех, кто от него зависит.
    pub fn reload(&mut self, name: &str) -> Outcome {
        if !self.is_loaded(name) {
            return Err(format!("{name} не загружен"));
        }

        let dependents = self.dependents(name);
        let mut order = vec![name.to_string()];

        order.extend(dependents.iter().cloned());

        // Выгрузка — с конца, загрузка — с начала.
        for each in order.iter().rev() {
            self.remove(each, true);
        }

        let mut done = Vec::new();

        for each in &order {
            match self.load(each) {
                Ok(summary) => done.push(summary),
                Err(error) => return Err(format!("перезагрузка прервана на {each}: {error}")),
            }
        }

        Ok(format!("перезагружено: {}", done.join(", ")))
    }

    /// Загружает всё, что лежит на диске, в порядке зависимостей. Отчёт —
    /// строки для лога.
    pub fn load_all(&mut self) -> Vec<Outcome> {
        let mut waiting: Vec<(String, bool)> = self.available();
        let mut report = Vec::new();

        loop {
            let before = waiting.len();
            let mut next = Vec::new();

            for (name, trusted) in waiting {
                let ready = match Manifest::read(&self.root.join(if trusted { MODULES_DIR } else { PLUGINS_DIR }).join(&name)) {
                    Ok(manifest) => manifest.dependencies.iter().all(|dependency| self.is_loaded(dependency)),
                    // Плохой plugin.toml — пусть load сам скажет, что не так.
                    Err(_) => true,
                };

                if !ready {
                    next.push((name, trusted));
                    continue;
                }

                if self.is_loaded(&name) {
                    continue;
                }

                report.push(self.load(&name));
            }

            waiting = next;

            if waiting.is_empty() || waiting.len() == before {
                break;
            }
        }

        for (name, _) in waiting {
            report.push(Err(format!("{name}: не хватает зависимостей или они не загрузились")));
        }

        report
    }

    /// Выгружает всё (остановка сервера): с конца.
    pub fn unload_all(&mut self) {
        let names: Vec<String> = self.loaded.iter().map(|plugin| plugin.manifest.name.clone()).collect();

        for name in names.iter().rev() {
            self.remove(name, true);
        }
    }

    /// Убирает упавших и пишет об этом в лог.
    fn bury(&mut self, crashes: Vec<Crash>) {
        for crash in crashes {
            log_error!("Плагин {} упал: {} — выгружен", crash.plugin, crash.reason);

            // Те, кто от него зависел, остались без основы — тоже выгружаем.
            for dependent in self.dependents(&crash.plugin).into_iter().rev() {
                log_warn!("Плагин {} выгружен: упал {}, от которого он зависит", dependent, crash.plugin);
                self.remove(&dependent, false);
            }

            self.remove(&crash.plugin, false);
        }
    }

    /// Зовёт каждый плагин по очереди; упавшие выгружаются.
    fn each<R>(&mut self, mut call: impl FnMut(&Module, &mut Store<PluginCtx>) -> wasmtime::Result<R>, mut on_result: impl FnMut(&str, R) -> bool) {
        let now = self.shared_tick();
        let mut crashes = Vec::new();

        for plugin in &mut self.loaded {
            match plugin.call(now, &mut call) {
                Ok(result) => {
                    if !on_result(&plugin.manifest.name, result) {
                        break;
                    }
                }
                Err(reason) => crashes.push(Crash { plugin: plugin.manifest.name.clone(), reason }),
            }
        }

        self.bury(crashes);
    }

    // ----- события -----

    pub fn respawn(&mut self, who: &PlayerView, destination: &PlayerView, bed: bool, anchor: bool) -> PlayerView {
        let now = self.shared_tick();let mut to = destination.clone();let mut crashes = Vec::new();
        for plugin in &mut self.loaded {
            let Some(function) = plugin.respawn_event else { continue; };
            let location = mcsa::api::types::Location { dimension: super::api::dimension_to_wit(to.dimension), x: to.x, y: to.y, z: to.z, yaw: Some(to.yaw), pitch: Some(to.pitch) };
            match plugin.call(now, |_, store| {let (result,) = function.call(&mut *store, (who.to_wit(), location, bed, anchor))?;Ok(result)}) {
                Ok(value) if valid_move(&value, super::api::dimension_from_wit(value.dimension)) => {to.dimension = super::api::dimension_from_wit(value.dimension);to.x = value.x;to.y = value.y;to.z = value.z;to.yaw = value.yaw.unwrap_or(to.yaw);to.pitch = value.pitch.unwrap_or(to.pitch);}
                Ok(_) => crashes.push(Crash { plugin: plugin.manifest.name.clone(), reason: "Недопустимое место возрождения".into() }),
                Err(reason) => crashes.push(Crash { plugin: plugin.manifest.name.clone(), reason }),
            }
        }
        self.bury(crashes);to
    }

    pub fn player_move(&mut self, who: &PlayerView, destination: &PlayerView, jump: bool) -> Option<PlayerView> {
        let now = self.shared_tick();
        let mut to = destination.clone();
        let mut crashes = Vec::new();
        let mut cancelled = false;
        for plugin in &mut self.loaded {
            let Some(function) = plugin.move_event else { continue; };
            let location = mcsa::api::types::Location { dimension: super::api::dimension_to_wit(to.dimension), x: to.x, y: to.y, z: to.z, yaw: Some(to.yaw), pitch: Some(to.pitch) };
            match plugin.call(now, |_, store| {
                let (result,) = function.call(&mut *store, (who.to_wit(), location, jump))?;
                Ok(result)
            }) {
                Ok(None) => { cancelled = true; break; }
                Ok(Some(value)) if valid_move(&value, who.dimension) => {
                    to.x = value.x; to.y = value.y; to.z = value.z;
                    to.yaw = value.yaw.unwrap_or(to.yaw); to.pitch = value.pitch.unwrap_or(to.pitch);
                }
                Ok(_) => { cancelled = true; crashes.push(Crash { plugin: plugin.manifest.name.clone(), reason: "Недопустимое место движения; для смены мира используйте teleport".into() }); }
                Err(reason) => crashes.push(Crash { plugin: plugin.manifest.name.clone(), reason }),
            }
        }
        self.bury(crashes);
        if cancelled { None } else { Some(to) }
    }

    pub fn interact(&mut self, who: &PlayerView, action: u32, hand: u32, block: Option<(i32, i32, i32)>, face: u32) -> u32 {
        let now = self.shared_tick();
        let mut denied = 0;
        let mut crashes = Vec::new();
        for plugin in &mut self.loaded {
            let Some(function) = plugin.interact_event else { continue; };
            let block = block.map(|(x, y, z)| mcsa::api::types::BlockPos { dimension: super::api::dimension_to_wit(who.dimension), x, y, z });
            match plugin.call(now, |_, store| {
                let (result,) = function.call(&mut *store, (who.to_wit(), action, hand, block, face))?;
                Ok(result)
            }) {
                Ok(value @ 0..=15) => denied |= value,
                Ok(_) => crashes.push(Crash { plugin: plugin.manifest.name.clone(), reason: "Недопустимый ответ обработчика взаимодействия".into() }),
                Err(reason) => crashes.push(Crash { plugin: plugin.manifest.name.clone(), reason }),
            }
        }
        self.bury(crashes);
        denied
    }

    pub fn damage(&mut self, who: &PlayerView, cause: &str, amount: f32) -> Option<f32> {self.damage_from(who,None,cause,amount)}
    pub fn damage_from(&mut self, who: &PlayerView, attacker: Option<&PlayerView>, cause: &str, amount: f32) -> Option<f32> {
        let now = self.shared_tick();
        let mut damage = f64::from(amount);
        let mut crashes = Vec::new();
        let mut cancelled = false;
        for plugin in &mut self.loaded {
            let result = if let Some(function) = plugin.damage_from {
                plugin.call(now, |_,store| {let (result,) = function.call(&mut *store,(who.to_wit(),attacker.map(PlayerView::to_wit),cause.to_string(),damage))?;Ok(result)})
            } else if let Some(function) = plugin.damage {
                plugin.call(now, |_,store| {let (result,) = function.call(&mut *store,(who.to_wit(),cause.to_string(),damage))?;Ok(result)})
            } else {continue};
            match result {
                Ok(None) => { cancelled = true; break; }
                Ok(Some(value)) if value.is_finite() && (0.0..=f64::from(f32::MAX)).contains(&value) => damage = value,
                Ok(_) => crashes.push(Crash { plugin: plugin.manifest.name.clone(), reason: "обработчик урона вернул некорректное число".into() }),
                Err(reason) => crashes.push(Crash { plugin: plugin.manifest.name.clone(), reason }),
            }
        }
        self.bury(crashes);
        if cancelled { None } else { Some(damage as f32) }
    }

    pub fn drag_event(&mut self, owner: &str, who: &PlayerView, handle: u32, slots: Vec<i32>, legacy: (i32,i32,i32)) -> bool {
        let now = self.shared_tick();
        let Some(plugin) = self.loaded.iter_mut().find(|p| p.manifest.name == owner) else {return true;};
        let Some(function) = plugin.drag_event else {return self.inventory_event(owner,who,handle,0,legacy);};
        match plugin.call(now, |_,store| {let (result,) = function.call(&mut *store,(who.to_wit(),handle,slots))?;Ok(result)}) {
            Ok(result) => result,
            Err(reason) => {let name=plugin.manifest.name.clone();self.bury(vec![Crash {plugin:name,reason}]);true},
        }
    }

    pub fn entity_interact(&mut self, who: &PlayerView, target: &str, hand: u32, slot: Option<u32>) -> bool {
        let now=self.shared_tick();let mut crashes=Vec::new();let mut cancelled=false;
        for plugin in &mut self.loaded {let Some(function)=plugin.entity_interact else {continue;};
            match plugin.call(now,|_,store|{let (result,)=function.call(&mut *store,(who.to_wit(),target.to_string(),hand,slot))?;Ok(result)}) {
                Ok(true)=>{cancelled=true;break;},Ok(false)=>(),Err(reason)=>crashes.push(Crash {plugin:plugin.manifest.name.clone(),reason}),
            }
        }self.bury(crashes);cancelled
    }

    pub fn prepare_craft(&mut self, who: &PlayerView, grid: &[Option<crate::inventory::Stack>], mut result: Option<crate::inventory::Stack>, repair: bool, table: bool) -> Option<crate::inventory::Stack> {
        let now=self.shared_tick();let mut crashes=Vec::new();
        for plugin in &mut self.loaded {let Some(function)=plugin.prepare_craft else {continue;};
            let input=plugin.store.data_mut().inventory_items(grid);let output=plugin.store.data_mut().inventory_items(&[result]).pop().flatten();
            let answer=plugin.call(now,|_,store|{let (value,)=function.call(&mut *store,(who.to_wit(),input,output,repair,table))?;Ok(value)});
            match answer {Ok(value)=>match plugin.store.data_mut().inventory_stack(value) {Ok(value)=>result=value,Err(reason)=>crashes.push(Crash {plugin:plugin.manifest.name.clone(),reason})},Err(reason)=>crashes.push(Crash {plugin:plugin.manifest.name.clone(),reason}),}
        }self.bury(crashes);result
    }

    pub fn prepare_anvil(&mut self, who: &PlayerView, inputs: &[Option<crate::inventory::Stack>], result: Option<crate::inventory::Stack>, mut cost: i32) -> i32 {
        let now=self.shared_tick();let mut crashes=Vec::new();
        for plugin in &mut self.loaded {let Some(function)=plugin.prepare_anvil else {continue;};
            let input=plugin.store.data_mut().inventory_items(inputs);let output=plugin.store.data_mut().inventory_items(&[result]).pop().flatten();
            match plugin.call(now,|_,store|{let (value,)=function.call(&mut *store,(who.to_wit(),input,output,cost))?;Ok(value)}) {
                Ok(value) if (0..=32767).contains(&value)=>cost=value,
                Ok(_)=>crashes.push(Crash {plugin:plugin.manifest.name.clone(),reason:"Стоимость наковальни должна быть от 0 до 32767".into()}),
                Err(reason)=>crashes.push(Crash {plugin:plugin.manifest.name.clone(),reason}),
            }
        }self.bury(crashes);cost
    }

    pub fn prepare_enchant(&mut self, who: &PlayerView, item: crate::inventory::Stack, mut properties: Vec<i16>) -> Vec<i16> {
        let now=self.shared_tick();let mut crashes=Vec::new();
        for plugin in &mut self.loaded {let Some(function)=plugin.prepare_enchant else {continue;};let Some(item)=plugin.store.data_mut().inventory_items(&[Some(item)]).pop().flatten() else {continue;};
            let input=properties.iter().map(|v|i32::from(*v)).collect();
            match plugin.call(now,|_,store|{let (value,)=function.call(&mut *store,(who.to_wit(),item,input))?;Ok(value)}) {
                Ok(value) if value.is_empty()=>{for i in 0..3 {properties[i]=0;properties[4+i]= -1;properties[7+i]= -1;}break;},
                Ok(value) if value.len()==10&&value.iter().enumerate().all(|(i,v)|if i<3 {(0..=32767).contains(v)} else if i==3 {*v==i32::from(properties[3])} else if i<7 {(-1..crate::stackdata::ENCHANTMENTS.len() as i32).contains(v)} else {(-1..=255).contains(v)})=>properties=value.into_iter().map(|v|v as i16).collect(),
                Ok(_)=>crashes.push(Crash {plugin:plugin.manifest.name.clone(),reason:"Неверные предложения зачарования".into()}),Err(reason)=>crashes.push(Crash {plugin:plugin.manifest.name.clone(),reason}),
            }
        }self.bury(crashes);properties
    }
    #[allow(clippy::too_many_arguments)]
    pub fn enchant_event(&mut self, who: &PlayerView, item: crate::inventory::Stack, mut result: crate::inventory::Stack, button: u32, cost: i32, hint: String, level: i32) -> Option<crate::inventory::Stack> {
        let now=self.shared_tick();let mut crashes=Vec::new();let mut cancelled=false;
        for plugin in &mut self.loaded {let Some(function)=plugin.enchant_event else {continue;};let mut items=plugin.store.data_mut().inventory_items(&[Some(item),Some(result)]).into_iter();let (Some(Some(input)),Some(Some(output)))=(items.next(),items.next()) else {continue;};
            match plugin.call(now,|_,store|{let (value,)=function.call(&mut *store,(who.to_wit(),input,output,button,cost,hint.clone(),level))?;Ok(value)}) {
                Ok(None)=>{cancelled=true;break;},Ok(value)=>match plugin.store.data_mut().inventory_stack(value) {Ok(Some(value))=>result=value,Ok(None)=>{cancelled=true;break;},Err(reason)=>crashes.push(Crash {plugin:plugin.manifest.name.clone(),reason})},Err(reason)=>crashes.push(Crash {plugin:plugin.manifest.name.clone(),reason}),
            }
        }self.bury(crashes);(!cancelled).then_some(result)
    }

    pub fn world_init(&mut self, dimension: crate::dimension::Dimension) {
        let now=self.shared_tick();let mut crashes=Vec::new();for plugin in &mut self.loaded {let Some(function)=plugin.world_init else {continue;};if let Err(reason)=plugin.call(now,|_,store|function.call(store,(super::api::dimension_to_wit(dimension),))) {crashes.push(Crash {plugin:plugin.manifest.name.clone(),reason});}}self.bury(crashes);
    }
    pub fn populate(&mut self, dimension: crate::dimension::Dimension, x: i32, z: i32, seed: i64, mut chunk: crate::world::Chunk) -> crate::world::Chunk {
        let now=self.shared_tick();let mut crashes=Vec::new();let dimension=super::api::dimension_to_wit(dimension);
        for plugin in &mut self.loaded {let (Some(wants),Some(populate))=(plugin.wants_population,plugin.populate) else {continue;};
            let result=plugin.call(now,|_,store|{let (wanted,)=wants.call(&mut *store,(dimension,))?;if !wanted {return Ok(Vec::new());}
                let (palette,runs)=chunk.population_snapshot();
                // Полный проход 98304 блоков имеет отдельный конечный бюджет.
                store.set_fuel(400_000_000)?;store.set_epoch_deadline(200);
                let (edits,)=populate.call(&mut *store,(dimension,x,z,seed.wrapping_add((x as i64).wrapping_mul(341873128712)).wrapping_add((z as i64).wrapping_mul(132897987541)),palette,runs))?;Ok(edits)
            });
            let result=result.and_then(|edits|if edits.is_empty() {Ok(())}else {chunk.apply_population(edits)});
            if let Err(reason)=result {crashes.push(Crash {plugin:plugin.manifest.name.clone(),reason:format!("Генерация чанка {x} {z}: {reason}")});}
        }
        self.bury(crashes);
        for (pos,slots) in chunk.population_loot() {
            let slots=self.generated_loot(super::api::dimension_from_wit(dimension),(x*16+pos.0,pos.1,z*16+pos.2),slots);
            chunk.replace_population_loot(pos,slots);
        }
        chunk
    }
    pub fn generated_loot(&mut self,dimension:crate::dimension::Dimension,pos:(i32,i32,i32),mut slots:Vec<Option<crate::inventory::Stack>>)->Vec<Option<crate::inventory::Stack>> {
        let now=self.shared_tick();let mut crashes=Vec::new();let dimension=super::api::dimension_to_wit(dimension);
            for plugin in &mut self.loaded {let Some(function)=plugin.generated_loot else {continue;};let encoded=plugin.store.data_mut().inventory_items(&slots);let block=mcsa::api::types::BlockPos {dimension,x:pos.0,y:pos.1,z:pos.2};
                let result=plugin.call(now,|_,store|{let (items,)=function.call(&mut *store,(block,encoded))?;Ok(items)}).and_then(|items|{if items.len()!=slots.len() {return Err("Неверный размер сгенерированной добычи".into());}items.into_iter().map(|item|plugin.store.data_mut().inventory_stack(item)).collect::<Result<Vec<_>,String>>()});
                match result {Ok(items)=>slots=items,Err(reason)=>crashes.push(Crash {plugin:plugin.manifest.name.clone(),reason})}
            }
        self.bury(crashes);slots
    }

    pub fn entity_death(&mut self, who: Option<&PlayerView>, uuid: &str, keep: bool, mut drops: Vec<crate::inventory::Stack>) -> Vec<crate::inventory::Stack> {
        let now=self.shared_tick();let mut crashes=Vec::new();for plugin in &mut self.loaded {let Some(function)=plugin.entity_death else {continue;};let encoded=plugin.store.data_mut().inventory_items(&drops.iter().copied().map(Some).collect::<Vec<_>>()).into_iter().flatten().collect();
            let result=plugin.call(now,|_,store|{let (items,)=function.call(&mut *store,(who.map(PlayerView::to_wit),uuid.to_string(),keep,encoded))?;Ok(items)}).and_then(|items|{if items.len()>512 {return Err("Слишком много предметов смерти".into());}items.into_iter().map(|item|plugin.store.data_mut().inventory_stack(Some(item)).and_then(|item|item.ok_or("Пустой предмет смерти".into()))).collect::<Result<Vec<_>,String>>()});
            match result {Ok(items)=>drops=items,Err(reason)=>crashes.push(Crash {plugin:plugin.manifest.name.clone(),reason})}
        }self.bury(crashes);drops
    }
    pub fn native_event(&mut self, kind: u32, mut fields: Vec<String>) -> Option<Vec<String>> {
        // Очередь переживает шаг сущностей: до доставки моб мог исчезнуть.
        // Не передаём Bukkit обработчик с уже неразрешимым UUID.
        if let Some(plugin)=self.loaded.first_mut() {
            let ctx=plugin.store.data_mut();
            let mut exists=|id: &str| super::api::mcsa::api::entities::Host::valid(ctx,id.to_string());
            match kind {
                1 => {let mut index=0;fields.retain(|id| {index+=1;index==1||exists(id)});if fields.len()<=1 {return Some(fields);}},
                2 | 4 if fields.first().is_some_and(|id|!exists(id)) => return Some(fields),
                4 if fields.get(1).is_some_and(|id|!id.is_empty()&&!exists(id)) => fields[1].clear(),
                11 if fields.first().is_some_and(|id|!id.is_empty()&&!exists(id)) => fields[0].clear(),
                _ => {},
            }
        }
        let now=self.shared_tick();let mut crashes=Vec::new();let mut cancelled=false;for plugin in &mut self.loaded {let Some(function)=plugin.native_event else {continue;};
            match plugin.call(now,|_,store|{let (value,)=function.call(&mut *store,(kind,fields.clone()))?;Ok(value)}) {
                Ok(value) if !value.is_empty()&&value.len()<=100000&&value.iter().map(String::len).sum::<usize>()<=4*1024*1024&&(value[0]=="0"||value[0]=="1") => {if value[0]=="1" {cancelled=true;break;}fields=value.into_iter().skip(1).collect();},
                Ok(_)=>crashes.push(Crash {plugin:plugin.manifest.name.clone(),reason:"Неверный ответ нативного события".into()}),Err(reason)=>crashes.push(Crash {plugin:plugin.manifest.name.clone(),reason}),
            }
        }self.bury(crashes);(!cancelled).then_some(fields)
    }
    pub fn block_drop(&mut self, who: &PlayerView, pos: (i32,i32,i32), data: &str, mut items: Vec<String>) -> Vec<String> {
        let now=self.shared_tick();let mut crashes=Vec::new();for plugin in &mut self.loaded {let Some(function)=plugin.block_drop else {continue;};let block=mcsa::api::types::BlockPos {dimension:super::api::dimension_to_wit(who.dimension),x:pos.0,y:pos.1,z:pos.2};
            match plugin.call(now,|_,store|{let (value,)=function.call(&mut *store,(who.to_wit(),block,data.to_string(),items.clone()))?;Ok(value)}) {Ok(value) if value.len()<=items.len()&&value.iter().all(|i|items.contains(i))=>items=value,Ok(_)=>crashes.push(Crash {plugin:plugin.manifest.name.clone(),reason:"В добычу блока добавлена посторонняя сущность".into()}),Err(reason)=>crashes.push(Crash {plugin:plugin.manifest.name.clone(),reason})}
        }self.bury(crashes);items
    }
    pub fn ground_item(&mut self, who: &PlayerView, uuid: &str, pickup: bool, remaining: i32) -> bool {
        let now=self.shared_tick();let mut crashes=Vec::new();let mut cancelled=false;for plugin in &mut self.loaded {let Some(function)=plugin.ground_item else {continue;};match plugin.call(now,|_,store|{let (value,)=function.call(&mut *store,(who.to_wit(),uuid.to_string(),pickup,remaining))?;Ok(value)}) {Ok(true)=>{cancelled=true;break;},Ok(false)=>{},Err(reason)=>crashes.push(Crash {plugin:plugin.manifest.name.clone(),reason})}}self.bury(crashes);cancelled
    }
    pub fn furnace_event(&mut self, dimension: crate::dimension::Dimension, pos: (i32,i32,i32), mut total: i32) -> i32 {
        let now=self.shared_tick();let mut crashes=Vec::new();
        for plugin in &mut self.loaded {let Some(function)=plugin.furnace_event else {continue;};let block=mcsa::api::types::BlockPos {dimension:super::api::dimension_to_wit(dimension),x:pos.0,y:pos.1,z:pos.2};
            match plugin.call(now,|_,store|{let (value,)=function.call(&mut *store,(block,total))?;Ok(value)}) {Ok(value) if (1..=72000).contains(&value)=>total=value,Ok(_)=>crashes.push(Crash {plugin:plugin.manifest.name.clone(),reason:"Время плавки вне диапазона 1..72000".into()}),Err(reason)=>crashes.push(Crash {plugin:plugin.manifest.name.clone(),reason})}
        }self.bury(crashes);total
    }

    pub fn trade_event(&mut self, who: &PlayerView, closed: bool, slots: &[Option<crate::inventory::Stack>], offers: &mut [crate::villager::Offer]) -> bool {
        let now=self.shared_tick();let mut crashes=Vec::new();let mut cancelled=false;
        for plugin in &mut self.loaded {let Some(function)=plugin.trade_event else {continue;};
            let mut flat=Vec::new();let mut cells=Vec::new();
            for (destination,stacks) in [(&mut cells,slots.to_vec()),(&mut flat,offers.iter().flat_map(|o|[Some(o.buy),o.buy_b,Some(o.sell)]).collect())] {
                for item in plugin.store.data_mut().inventory_items(&stacks) {let item=item.unwrap_or(mcsa::api::inventory::Item {kind:"air".into(),count:0,metadata:String::new()});destination.extend([item.kind,item.count.to_string(),item.metadata]);}
            }
            let mut encoded=Vec::new();for (offer,items) in offers.iter().zip(flat.chunks(9)) {encoded.extend_from_slice(items);encoded.push(offer.special.to_string());}
            match plugin.call(now,|_,store| {let (value,)=function.call(&mut *store,(who.to_wit(),closed,cells,encoded))?;Ok(value)}) {
                Ok(values) if values.len()==offers.len()+1&&(0..=1).contains(&values[0]) => {cancelled|=values[0]!=0;for (offer,value) in offers.iter_mut().zip(&values[1..]) {offer.special = *value;}},
                Ok(_)=>crashes.push(Crash {plugin:plugin.manifest.name.clone(),reason:"Неверные цены торгового события".into()}),Err(reason)=>crashes.push(Crash {plugin:plugin.manifest.name.clone(),reason}),
            }
        }self.bury(crashes);cancelled
    }

    pub fn food_level(&mut self, who: &PlayerView, mut level: i32, item: Option<crate::inventory::Stack>) -> Option<i32> {
        let now=self.shared_tick();let mut crashes=Vec::new();let mut cancelled=false;
        for plugin in &mut self.loaded {let Some(function)=plugin.food_event else {continue;};let item=plugin.store.data_mut().inventory_items(&[item]).pop().flatten();
            match plugin.call(now,|_,store|{let (value,)=function.call(&mut *store,(who.to_wit(),level,item))?;Ok(value)}) {
                Ok(None)=>{cancelled=true;break;},Ok(Some(value)) if (0..=20).contains(&value)=>level=value,
                Ok(_)=>crashes.push(Crash {plugin:plugin.manifest.name.clone(),reason:"Уровень еды должен быть от 0 до 20".into()}),Err(reason)=>crashes.push(Crash {plugin:plugin.manifest.name.clone(),reason}),
            }
        }self.bury(crashes);(!cancelled).then_some(level)
    }

    pub fn item_event(&mut self, who: &PlayerView, kind: u32, slot: i32) -> bool {
        let now = self.shared_tick();let mut crashes = Vec::new();let mut cancelled = false;
        for plugin in &mut self.loaded {
            let Some(function) = plugin.item_event else { continue };
            match plugin.call(now, |_, store| {let (result,) = function.call(&mut *store, (who.to_wit(), kind, slot))?;Ok(result)}) {
                Ok(true) => {cancelled = true;break;}, Ok(false) => (),
                Err(reason) => crashes.push(Crash {plugin: plugin.manifest.name.clone(), reason}),
            }
        }
        self.bury(crashes);cancelled
    }

    pub fn inventory_event(&mut self, owner: &str, who: &PlayerView, handle: u32, kind: u32, click: (i32, i32, i32)) -> bool {
        let now = self.shared_tick();
        let Some(plugin) = self.loaded.iter_mut().find(|plugin| plugin.manifest.name == owner) else { return true; };
        if kind == 1 && plugin.store.data().chests.viewers.get(&who.uuid) == Some(&handle) { plugin.store.data_mut().chests.viewers.remove(&who.uuid); }
        let Some(function) = plugin.inventory_event else { return true; };
        let result = plugin.call(now, |_, store| {
            let (cancelled,) = function.call(&mut *store, (who.to_wit(), handle, kind, click.0, click.1, click.2))?;
            Ok(cancelled)
        });
        match result {
            Ok(cancelled) => cancelled,
            Err(reason) => { self.bury(vec![Crash { plugin: owner.into(), reason }]); true }
        }
    }

    #[cfg(test)]
    pub fn death(&mut self, who: &PlayerView, keep_inventory: bool) {self.death_legacy(who,keep_inventory);self.entity_death(Some(who),&crate::network::login::format_uuid(&who.uuid),keep_inventory,Vec::new());}
    pub fn death_legacy(&mut self, who: &PlayerView, keep_inventory: bool) {
        let now = self.shared_tick();
        let mut crashes = Vec::new();
        for plugin in &mut self.loaded {
            if plugin.entity_death.is_some() {continue;}
            let Some(function) = plugin.death else { continue };
            if let Err(reason) = plugin.call(now, |_, store| function.call(&mut *store, (who.to_wit(), keep_inventory))) {
                crashes.push(Crash { plugin: plugin.manifest.name.clone(), reason });
            }
        }
        self.bury(crashes);
    }

    pub fn player_join(&mut self, who: &PlayerView) {
        let who = who.to_wit();

        self.each(|instance, store| instance.mcsa_api_events().call_on_player_join(store, &who), |_, ()| true);
    }

    pub fn player_quit(&mut self, who: &PlayerView) {
        let who = who.to_wit();

        self.each(|instance, store| instance.mcsa_api_events().call_on_player_quit(store, &who), |_, ()| true);
    }

    /// Чат: плагины по очереди; первый отменивший останавливает цепочку, а
    /// изменённый текст идёт следующему.
    pub fn chat(&mut self, who: &PlayerView, message: &str) -> ChatOutcome {
        use mcsa::api::types::ChatResult;

        let who = who.to_wit();
        let current = Mutex::new(message.to_string());
        let mut cancelled = false;

        self.each(
            |instance, store| {
                let text = current.lock().expect("текст чата").clone();

                instance.mcsa_api_events().call_on_chat(store, &who, &text)
            },
            |name, result| match result {
                ChatResult::Pass => true,
                ChatResult::Cancel => {
                    log_debug!("Плагин {} отменил сообщение игрока", name);
                    cancelled = true;
                    false
                }
                ChatResult::Replace(text) => {
                    *current.lock().expect("текст чата") = text;
                    true
                }
            },
        );

        if cancelled {
            return ChatOutcome::Cancel;
        }

        let text = current.into_inner().expect("текст чата");

        if text == message { ChatOutcome::Pass } else { ChatOutcome::Replace(text) }
    }

    fn verdict(&mut self, call: impl Fn(&Module, &mut Store<PluginCtx>) -> wasmtime::Result<mcsa::api::types::Verdict>) -> Verdict {
        let mut denied = false;

        self.each(&call, |name, result| {
            if result == mcsa::api::types::Verdict::Deny {
                log_debug!("Плагин {} отменил событие", name);
                denied = true;
                return false;
            }

            true
        });

        if denied { Verdict::Deny } else { Verdict::Allow }
    }

    pub fn block_break(&mut self, who: &PlayerView, pos: &mcsa::api::types::BlockPos, block: &str) -> Verdict {
        let who = who.to_wit();

        self.verdict(|instance, store| instance.mcsa_api_events().call_on_block_break(store, &who, *pos, block))
    }

    pub fn block_place(&mut self, who: &PlayerView, pos: &mcsa::api::types::BlockPos, block: &str) -> Verdict {
        let who = who.to_wit();

        self.verdict(|instance, store| instance.mcsa_api_events().call_on_block_place(store, &who, *pos, block))
    }

    pub fn player_command(&mut self, who: &PlayerView, line: &str) -> Verdict {
        let who = who.to_wit();

        self.verdict(|instance, store| instance.mcsa_api_events().call_on_player_command(store, &who, line))
    }

    // ----- команды -----

    /// Есть ли у плагинов команда с таким именем и какой нужен уровень.
    pub fn command_level(&self, name: &str) -> Option<i32> {
        self.loaded.iter().flat_map(|plugin| plugin.store.data().commands.iter()).find(|command| command.name == name).map(|command| i32::from(command.level))
    }

    /// Выполняет команду плагина. None — такой нет.
    pub fn run_command(&mut self, from: mcsa::api::types::Sender, name: &str, args: &[String]) -> Option<(bool, String)> {
        let owner = self.registry.lock().expect("список команд захвачен другим потоком").get(name).cloned()?;
        let now = self.shared_tick();
        let index = self.loaded.iter().position(|plugin| plugin.manifest.name == owner)?;
        let args: Vec<String> = args.to_vec();

        let result = self.loaded[index].call(now, |instance, store| instance.mcsa_api_events().call_on_command(store, &from, name, &args));

        match result {
            Ok(result) => Some((result.success, result.message.unwrap_or_default())),
            Err(reason) => {
                self.bury(vec![Crash { plugin: owner, reason }]);
                Some((false, "Команда плагина упала, плагин выгружен".to_string()))
            }
        }
    }

    /// Список команд с учётом прав вызывающего, без обращения к гостю.
    pub fn command_names(&self, level: u8) -> Vec<String> {
        let mut names: Vec<_> = self.loaded.iter().flat_map(|plugin| &plugin.store.data().commands)
            .filter(|command| command.level <= level).map(|command| command.name.clone()).collect();
        names.sort(); names
    }

    pub fn complete(&mut self, from: mcsa::api::types::Sender, level: u8, name: &str, args: Vec<String>) -> Option<Vec<String>> {
        let needed = self.command_level(name)?;
        if i32::from(level) < needed { return Some(Vec::new()); }
        let index = self.loaded.iter().position(|plugin| plugin.store.data().commands.iter().any(|command| command.name == name))?;
        let Some(function) = self.loaded[index].completion else { return Some(Vec::new()) };
        let now = self.shared_tick();
        let result = self.loaded[index].call(now, |_, store| {
            let (values,) = function.call(&mut *store, (from, name.to_string(), args))?;
            Ok(values)
        });
        match result {
            Ok(values) => Some(values.into_iter().filter(|value| value.len() <= 512 && !value.chars().any(char::is_control)).take(100).collect()),
            Err(reason) => { let plugin = self.loaded[index].manifest.name.clone(); self.bury(vec![Crash { plugin, reason }]); Some(Vec::new()) }
        }
    }

    // ----- такт -----

    /// Задачи планировщика на такте `now`: на потоке такта, без замков мира.
    pub fn tick(&mut self, now: u64) {
        TICK.store(now, Ordering::Relaxed);

        let started = std::time::Instant::now();

        let mut crashes = Vec::new();

        for plugin in &mut self.loaded {
            plugin.store.data_mut().tick = now;

            let due = plugin.store.data_mut().due_tasks(now);

            for task in due {
                if let Err(reason) = plugin.call(now, |instance, store| instance.mcsa_api_events().call_on_task(store, task)) {
                    crashes.push(Crash { plugin: plugin.manifest.name.clone(), reason:format!("Задача {task}: {reason}") });
                    break;
                }
            }
        }

        self.bury(crashes);
        self.tick_stats.add(started.elapsed().as_nanos() as u64);
    }
}

/// Номер такта мира для API: пока такт его не сообщил, 0.
static TICK: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

impl Loaded {
    /// Один вызов плагина с топливом, сроком и охраной от возврата в хозяина.
    fn call<R>(&mut self, now: u64, call: impl FnOnce(&Module, &mut Store<PluginCtx>) -> wasmtime::Result<R>) -> Result<R, String> {
        self.store.data_mut().tick = now;
        self.store.data().native_wait.reset();

        self.store.set_fuel(self.limits.fuel).map_err(|error| error.to_string())?;
        self.store.set_epoch_deadline(self.limits.epochs);

        let _inside = InsideGuard::enter();
        let started = std::time::Instant::now();
        let result = call(&self.instance, &mut self.store).map_err(|error| {
            // Подробный лог сохраняет место ловушки и остаток бюджета, не текст данных плагина.
            log_debug!("Ловушка {}: elapsed_us={}, fuel_left={}, {error:?}",self.manifest.name,started.elapsed().as_micros(),self.store.get_fuel().unwrap_or(0));
            // Интеграционная проверка сохраняет стек Wasm до сокращения причины выгрузки.
            #[cfg(test)]
            eprintln!("{}",format!("{error:?}").chars().take(4000).collect::<String>());
            short(&error)
        });

        self.stats.add(started.elapsed().as_nanos() as u64);
        result
    }
}

/// После компиляции компонента в куче остаются свободные страницы, которые
/// аллокатор сам не отдаёт системе, — а компилятору их нужно несколько МиБ
/// на каждый плагин. Возвращаем (замер: docs/plugins.md).
#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn give_back_memory() {
    // SAFETY: `malloc_trim` ничего не требует от вызывающего, кроме
    // того, что аллокатор — glibc (условие компиляции).
    unsafe {
        libc::malloc_trim(0);
    }
}

#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
fn give_back_memory() {}

/// Все плагины на диске под `root`: (имя, доверенный); сперва модули.
pub fn available_in(root: &Path) -> Vec<(String, bool)> {
    let mut found = Vec::new();

    for (dir, trusted) in [(MODULES_DIR, true), (PLUGINS_DIR, false)] {
        let Ok(entries) = std::fs::read_dir(root.join(dir)) else { continue };
        let mut names: Vec<String> = entries.flatten().filter(|entry| entry.path().join("plugin.toml").is_file()).filter_map(|entry| entry.file_name().into_string().ok()).collect();

        names.sort();
        found.extend(names.into_iter().map(|name| (name, trusted)));
    }

    found
}

/// Причина ошибки одной-двумя строками: цепочку «из-за» wasmtime разворачивает
/// длинным списком кадров, в лог нужна суть.
pub fn short(error: &wasmtime::Error) -> String {
    // Суть — в корне цепочки: сверху лежат «error while executing at wasm
    // backtrace» и кадры стека.
    let root = error.root_cause().to_string();
    let text = format!("{error:#}");
    let first = root.lines().next().unwrap_or("").to_string();

    if text.contains("all fuel consumed") {
        return "плагин выбрал всё топливо (слишком долгий вызов)".to_string();
    }

    if text.contains("epoch deadline") || text.contains("interrupt") {
        return "плагин не уложился в срок вызова".to_string();
    }

    if text.contains("memory growth failure") || text.contains("growing memory") || text.contains("resource limit exceeded") && text.contains("memory") {
        return "плагин упёрся в лимит памяти".to_string();
    }

    if text.contains("unreachable") {
        return "паника или недостижимый код в плагине".to_string();
    }

    first
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_are_shortened_to_the_point() {
        let error = wasmtime::Error::msg("wasm trap: all fuel consumed by WebAssembly");

        assert!(short(&error).contains("топливо"));
        assert_eq!(short(&wasmtime::Error::msg("something\nelse")), "something");
        assert!(short(&wasmtime::Error::msg("forcing a memory growth failure to be a trap")).contains("памяти"));
    }
}

// Плагины не могут пропустить NaN, бесконечность и чужое измерение в физику.
fn valid_move(value: &mcsa::api::types::Location, dimension: crate::dimension::Dimension) -> bool {
    super::api::dimension_from_wit(value.dimension) == dimension
        && value.x.is_finite() && value.x.abs() <= 30_000_000.0
        && value.z.is_finite() && value.z.abs() <= 30_000_000.0
        && value.y.is_finite() && value.y.abs() <= 20_000_000.0
        && value.yaw.is_none_or(f32::is_finite) && value.pitch.is_none_or(f32::is_finite)
}

#[cfg(test)]
mod movement_tests {
    use super::*;
    #[test]
    fn plugin_destinations_reject_nonfinite_coordinates_and_cross_world_moves() {
        let valid = mcsa::api::types::Location {dimension: mcsa::api::types::Dimension::Overworld, x: 0.0, y: 70.0, z: 0.0, yaw: Some(0.0), pitch: Some(0.0)};
        assert!(valid_move(&valid, crate::dimension::Dimension::Overworld));
        for value in [f64::NAN, f64::INFINITY, 30_000_001.0] {assert!(!valid_move(&mcsa::api::types::Location {x: value, ..valid}, crate::dimension::Dimension::Overworld));}
        assert!(!valid_move(&mcsa::api::types::Location {yaw: Some(f32::NAN), ..valid}, crate::dimension::Dimension::Overworld));
        assert!(!valid_move(&valid, crate::dimension::Dimension::Nether));
    }
}

#[cfg(test)]
mod native_wait_tests {
    use super::*;
    #[test]
    fn native_wait_credit_is_finite_and_never_pauses_guest_execution() {
        let wait=NativeWait::default();wait.reset();assert!(!wait.consume());
        {
            let _guard=wait.enter();
            for _ in 0..NATIVE_WAIT_EPOCHS {assert!(wait.consume());}
            for _ in 0..100 {assert!(!wait.consume());}
        }
        wait.reset();
        {let _guard=wait.enter();assert!(wait.consume());}
        assert!(!wait.consume());
    }
}
