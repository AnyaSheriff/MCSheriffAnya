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
use std::sync::atomic::{AtomicBool, Ordering};
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

/// Лимиты обычного плагина: работу ограничивает топливо (~20 мс счёта на
/// вызов); часы — только страховка от зависания. Срок по часам щедрый
/// (200 мс): на одноядерной машине система может ненадолго снять процесс с
/// ядра, и при сроке в 20 мс исправный плагин выгружался бы за чужую паузу.
pub const PLUGIN_LIMITS: Limits = Limits { fuel: 20_000_000, epochs: 20, memory: 32 << 20 };

/// Лимиты модуля: он доверенный и может делать больше.
pub const MODULE_LIMITS: Limits = Limits { fuel: 400_000_000, epochs: 20, memory: 256 << 20 };

/// Длительность эпохи часов.
const EPOCH: Duration = Duration::from_millis(10);

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
type Damage = wasmtime::component::TypedFunc<(mcsa::api::types::Player, String, f64), (Option<f64>,)>;
type InventoryEvent = wasmtime::component::TypedFunc<(mcsa::api::types::Player, u32, u32, i32, i32, i32), (bool,)>;
type Death = wasmtime::component::TypedFunc<(mcsa::api::types::Player, bool), ()>;

/// Загруженный плагин.
pub struct Loaded {
    pub manifest: Manifest,
    pub trusted: bool,
    pub store: Store<PluginCtx>,
    instance: Module,
    completion: Option<Completion>,
    damage: Option<Damage>,
    death: Option<Death>,
    inventory_event: Option<InventoryEvent>,
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
        let flag = Arc::clone(&stop);

        let started = std::thread::Builder::new().name("plugin clock".into()).spawn(move || {
            while !flag.load(Ordering::Relaxed) {
                std::thread::sleep(EPOCH);
                engine.increment_epoch();
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
        let inventory_event = raw.get_export_index(&mut store, None, "mcsa:api/inventory-events@0.1.0")
            .and_then(|parent| raw.get_export_index(&mut store, Some(&parent), "on-inventory"))
            .map(|index| raw.get_typed_func::<(mcsa::api::types::Player, u32, u32, i32, i32, i32), (bool,)>(&mut store, &index))
            .transpose().map_err(|error| short(&error))?;
        let mut plugin = Loaded { manifest, trusted, store, instance, completion, damage, death, inventory_event, limits, stats: Stats::default() };

        // enable: здесь плагин регистрирует команды и задачи. Упал — не грузим.
        if let Err(reason) = plugin.call(self.shared_tick(), |instance, store| instance.mcsa_api_lifecycle().call_enable(store)) {
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

    pub fn damage(&mut self, who: &PlayerView, cause: &str, amount: f32) -> Option<f32> {
        let now = self.shared_tick();
        let mut damage = f64::from(amount);
        let mut crashes = Vec::new();
        let mut cancelled = false;
        for plugin in &mut self.loaded {
            let Some(function) = plugin.damage else { continue };
            match plugin.call(now, |_, store| {
                let (result,) = function.call(&mut *store, (who.to_wit(), cause.to_string(), damage))?;
                Ok(result)
            }) {
                Ok(None) => { cancelled = true; break; }
                Ok(Some(value)) if value.is_finite() && (0.0..=f64::from(f32::MAX)).contains(&value) => damage = value,
                Ok(_) => crashes.push(Crash { plugin: plugin.manifest.name.clone(), reason: "обработчик урона вернул некорректное число".into() }),
                Err(reason) => crashes.push(Crash { plugin: plugin.manifest.name.clone(), reason }),
            }
        }
        self.bury(crashes);
        if cancelled { None } else { Some(damage as f32) }
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

    pub fn death(&mut self, who: &PlayerView, keep_inventory: bool) {
        let now = self.shared_tick();
        let mut crashes = Vec::new();
        for plugin in &mut self.loaded {
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
                    crashes.push(Crash { plugin: plugin.manifest.name.clone(), reason });
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

        self.store.set_fuel(self.limits.fuel).map_err(|error| error.to_string())?;
        self.store.set_epoch_deadline(self.limits.epochs);

        let _inside = InsideGuard::enter();
        let started = std::time::Instant::now();
        let result = call(&self.instance, &mut self.store).map_err(|error| short(&error));

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
