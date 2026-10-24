// Плагины WebAssembly.
//
// Плагин — модуль-компонент wasm, написанный под API `mcsa:api`
// (wit/mcsa-api.wit; по-Java — пакет `ru.sheriffanya.mcsa.api`). Сервер
// исполняет их встроенным wasmtime: никакой JVM, всё на Rust.
//
// Две папки рядом с сервером:
//
//   plugins/<имя>/{plugin.toml, <имя>.wasm}   — обычные плагины, песочница
//   modules/<имя>/{plugin.toml, <имя>.wasm}   — доверенные модули, шире права
//
// Загрузка, выгрузка и перезагрузка — на ходу: `/plugman list|load|unload|
// reload|info`. Упавший плагин выгружается с записью в лог, сервер идёт
// дальше.
//
// Потоки. Обработчики событий запрашивает поток действия игрока; он ждёт
// ответ отдельного потока плагинов. Задачи планировщика запрашивает поток
// такта (`tick`), после функций и до работы миров, без замков мира. Все
// вызовы в плагины идут на одном потоке хозяина, по очереди загрузки.
// Правило для всего ядра: в плагин входят, не держа замков мира, игроков,
// предметов и чата — плагин сам их берёт, когда зовёт API.
//
// Без cargo-возможности `plugins` модуль состоит из пустых заглушек, и
// ничего этого в ядре нет.

use std::sync::Arc;

use crate::commands::{Answer, Source};
use crate::dimension::Dimension;
use crate::shared::Shared;

#[cfg(feature = "plugins")]
mod api;
#[cfg(feature = "plugins")]
mod presentation;
#[cfg(feature = "plugins")]
mod network;
#[cfg(feature = "plugins")]
mod inventory;
#[cfg(feature = "plugins")]
mod attributes;
#[cfg(feature = "plugins")]
mod host;
#[cfg(feature = "plugins")]
mod manifest;
#[cfg(feature = "plugins")]
mod plugman;
#[cfg(all(feature = "plugins", test))]
mod tests;

/// Игрок в момент события — копия нужного, без ссылок в состояние сервера.
#[cfg_attr(not(feature = "plugins"), allow(dead_code))] // без плагинов её никто не читает
#[derive(Clone, Debug)]
pub struct PlayerView {
    pub uuid: [u8; 16],
    pub name: String,
    pub game_mode: i32,
    pub dimension: Dimension,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
}

/// Ответ обработчиков отменяемого события.
#[cfg_attr(not(feature = "plugins"), allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    Allow,
    Deny,
}

/// Что стало с сообщением в чате.
#[cfg_attr(not(feature = "plugins"), allow(dead_code))]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ChatOutcome {
    Pass,
    Cancel,
    Replace(String),
}

#[cfg(feature = "plugins")]
mod enabled {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{OnceLock, mpsc};

    use super::api::mcsa::api::types as wit;
    use super::host::{self, Host};
    use super::*;
    use crate::{log_error, log_info, log_warn};

    /// Место хозяина. Сам хозяин (движок wasmtime — несколько МиБ памяти)
    /// заводится, только когда есть что загружать: пока папки пусты, плагины
    /// ядру ничего не стоят.
    pub(super) struct Slot {
        shared: Arc<Shared>,
        root: PathBuf,
        host: Option<Host>,
    }

    impl Slot {
        pub(super) fn host(&mut self) -> Result<&mut Host, String> {
            if self.host.is_none() {
                self.host = Some(Host::new(Arc::clone(&self.shared), self.root.clone()).map_err(|error| host::short(&error))?);
            }

            Ok(self.host.as_mut().expect("хозяин только что заведён"))
        }

        fn publish(&self) {
            ACTIVE.store(self.host.as_ref().is_some_and(|host| !host.is_empty()), Ordering::Relaxed);
        }
    }

    type Job = Box<dyn FnOnce(&mut Slot) + Send>;

    /// Очередь работ потоку плагинов.
    static JOBS: OnceLock<mpsc::Sender<Job>> = OnceLock::new();
    static ACTIVE: AtomicBool = AtomicBool::new(false);

    /// Есть ли что вызывать: без загруженных плагинов события обходятся
    /// одной проверкой флага.
    pub fn active() -> bool {
        ACTIVE.load(Ordering::Relaxed)
    }

    /// Все вызовы в плагины идут на одном своём потоке. Причин две: у
    /// потока такта и у подключений есть свой исполнитель tokio, а системный
    /// интерфейс wasmtime (`wasmtime-wasi`) внутри него блокироваться не
    /// умеет — паника плагина ронялa бы вызывающего; и один поток — это один
    /// порядок вызовов без замков. Вызывающий ждёт ответа, не держа замков
    /// ядра (правило в начале файла).
    fn call<R: Send + 'static>(default: R, job: impl FnOnce(&mut Slot) -> Option<R> + Send + 'static) -> R {
        if host::inside_plugin() {
            return default;
        }

        let Some(jobs) = JOBS.get() else { return default };
        let (reply, answer) = mpsc::sync_channel(1);

        let job: Job = Box::new(move |slot| {
            // Паника хозяина (ошибка в самом ядре) не должна убивать поток
            // плагинов: следующий вызов пойдёт как ни в чём не бывало.
            let result = catch_unwind(AssertUnwindSafe(|| job(slot)));

            // Флаг виден до ответа: после load/unload следующий вызов уже
            // должен видеть новое состояние, независимо от планировщика ОС.
            slot.publish();
            let _ = reply.send(result);
        });

        if jobs.send(job).is_err() {
            return default;
        }

        match answer.recv() {
            Ok(Ok(Some(result))) => result,
            Ok(Ok(None)) => default,
            Ok(Err(_)) => {
                log_error!("Плагины: ошибка в хозяине плагинов (паника), вызов пропущен");
                default
            }
            Err(_) => default,
        }
    }

    /// Запуск: папки, поток плагинов, хозяин (если есть что грузить),
    /// загрузка всего, что лежит на диске.
    pub fn start(shared: Arc<Shared>) {
        start_in(shared, PathBuf::from("."));
    }

    /// То же, но папки `plugins/` и `modules/` — внутри `root`.
    pub fn start_in(shared: Arc<Shared>, root: PathBuf) {
        for dir in [host::PLUGINS_DIR, host::MODULES_DIR] {
            if let Err(error) = std::fs::create_dir_all(root.join(dir)) {
                log_warn!("Плагины: не создать папку {}: {}", dir, error);
            }
        }

        let (jobs, queue) = mpsc::channel::<Job>();
        let mut slot = Slot { shared, root, host: None };

        // Стек побольше обычного: компилятор wasm рекурсивен.
        let started = std::thread::Builder::new().name("plugins".into()).stack_size(16 << 20).spawn(move || {
            // Не на ядре такта: такт ждёт ответа и сидит на своём.
            crate::tick::keep_off_tick_core();

            for job in queue {
                job(&mut slot);
            }
        });

        if let Err(error) = started {
            log_warn!("Плагины не запущены: нет потока ({})", error);
            return;
        }

        JOBS.set(jobs).ok();

        let report = call(Vec::new(), |slot| {
            let found = host::available_in(&slot.root).len();

            if found == 0 {
                return Some(vec![Ok(format!("папки {}/ и {}/ пусты", host::PLUGINS_DIR, host::MODULES_DIR))]);
            }

            Some(match slot.host() {
                Ok(host) => {
                    let mut lines = vec![Ok(format!("найдено {found}"))];

                    lines.extend(host.load_all());
                    lines
                }
                Err(error) => vec![Err(error)],
            })
        });

        // Первая строка — итог поиска, остальные — по плагину.
        for (index, outcome) in report.into_iter().enumerate() {
            match (index, outcome) {
                (0, Ok(text)) => log_info!("Плагины: {}", text),
                (_, Ok(summary)) => log_info!("Плагин загружен: {}", summary),
                (_, Err(error)) => log_warn!("Плагин не загружен: {}", error),
            }
        }
    }

    /// Остановка сервера: всех выгрузить, чтобы плагины сохранились.
    pub fn shutdown() {
        call((), |slot| {
            if let Some(host) = slot.host.as_mut()
                && !host.is_empty()
            {
                log_info!("Остановка плагинов");
                host.unload_all();
            }

            Some(())
        });
    }

    /// Вызов хозяина для `/plugman`: плагинов может ещё не быть, а хозяин —
    /// не заведён (заводится здесь). None — плагины не запущены.
    pub(super) fn with_host_always<R: Send + 'static>(call_host: impl FnOnce(&mut Host) -> R + Send + 'static) -> Option<Result<R, String>> {
        call(None, |slot| Some(Some(slot.host().map(call_host))))
    }

    /// Вызов хозяина по событию: только если плагины есть.
    fn with_host<R: Send + 'static>(default: R, call_host: impl FnOnce(&mut Host) -> R + Send + 'static) -> R {
        if !active() {
            return default;
        }

        call(default, |slot| slot.host.as_mut().map(call_host))
    }

    pub fn tick(now: u64) {
        if active() {
            call((), move |slot| {
                slot.host.as_mut().map(|host| host.tick(now))
            });
        }
    }

    pub fn player_join(shared: &Shared, name: &str) {
        if !active() {
            return;
        }

        let found = shared.players.lock().expect("список игроков захвачен другим потоком").find(name);

        if let Some(member) = found {
            let view = PlayerView::of_member(&member);

            with_host((), move |host| host.player_join(&view));
        }
    }

    pub fn player_quit(view: &PlayerView) {
        let view = view.clone();

        with_host((), move |host| host.player_quit(&view));
    }

    pub fn chat(who: &PlayerView, message: &str) -> ChatOutcome {
        let (who, message) = (who.clone(), message.to_string());

        with_host(ChatOutcome::Pass, move |host| host.chat(&who, &message))
    }

    pub fn block_break(who: &PlayerView, at: (i32, i32, i32), block: &str) -> Verdict {
        let pos = wit::BlockPos { dimension: super::api::dimension_to_wit(who.dimension), x: at.0, y: at.1, z: at.2 };
        let (who, block) = (who.clone(), block.to_string());

        with_host(Verdict::Allow, move |host| host.block_break(&who, &pos, &block))
    }

    pub fn block_place(who: &PlayerView, at: (i32, i32, i32), block: &str) -> Verdict {
        let pos = wit::BlockPos { dimension: super::api::dimension_to_wit(who.dimension), x: at.0, y: at.1, z: at.2 };
        let (who, block) = (who.clone(), block.to_string());

        with_host(Verdict::Allow, move |host| host.block_place(&who, &pos, &block))
    }

    pub fn player_command(who: &PlayerView, line: &str) -> Verdict {
        let (who, line) = (who.clone(), line.to_string());

        with_host(Verdict::Allow, move |host| host.player_command(&who, &line))
    }

    pub fn damage(who: &PlayerView, cause: &str, amount: f32) -> Option<f32> {
        let (who, cause) = (who.clone(), cause.to_string());
        with_host(Some(amount), move |host| host.damage(&who, &cause, amount))
    }

    pub fn inventory_event(owner: &str, who: &PlayerView, handle: u32, kind: u32, click: (i32, i32, i32)) -> bool {
        let (owner, who) = (owner.to_string(), who.clone());
        with_host(true, move |host| host.inventory_event(&owner, &who, handle, kind, click))
    }

    pub fn death(who: &PlayerView, keep_inventory: bool) {
        let who = who.clone();
        with_host((), move |host| host.death(&who, keep_inventory));
    }

    pub fn command_names(shared: &Shared, uuid: &[u8;16]) -> Vec<String> {
        let level = shared.ops.lock().expect("операторы").level_of(uuid) as u8;
        with_host(Vec::new(), move |host| host.command_names(level))
    }

    pub fn tab_complete(shared: &Shared, uuid: &[u8;16], line: &str) -> Option<Vec<String>> {
        if crate::auth::sensitive_command(line) || line.len() > 8192 { return None; }
        let mut words = line.split(' ');
        let name = words.next()?.to_ascii_lowercase();
        let args: Vec<String> = words.map(str::to_string).collect();
        let caller = {
            let players = shared.players.lock().expect("игроки");
            let member = players.members().iter().find(|member| &member.uuid == uuid)?;
            wit::Sender { kind: wit::SenderKind::Player, uuid: Some(crate::network::login::format_uuid(uuid)), name: member.name.clone() }
        };
        let level = shared.ops.lock().expect("операторы").level_of(uuid) as u8;
        with_host(None, move |host| host.complete(caller, level, &name, args))
    }

    /// `/plugman` и команды плагинов: вызывается в начале `commands::run`.
    /// None — это не наша команда.
    pub fn run_command(command: &str, source: &Source, shared: &Shared) -> Option<Answer> {
        let mut words = command.split_whitespace();
        let first = words.next()?;

        if first.eq_ignore_ascii_case("plugman") {
            let args: Vec<String> = words.map(str::to_string).collect();

            return Some(super::plugman::run(&args, source, shared));
        }

        if !active() {
            return None;
        }

        let name = first.to_ascii_lowercase();
        let args: Vec<String> = words.map(str::to_string).collect();

        let level = match source {
            Source::Console => crate::ops::HIGHEST_LEVEL,
            // Командный блок — уровень 2 (commands.rs).
            Source::Block { .. } => 2,
            Source::Player { uuid, .. } => shared.ops.lock().expect("права захвачены другим потоком").level_of(uuid),
        };

        let sender = match source {
            Source::Player { uuid, name } => wit::Sender { kind: wit::SenderKind::Player, uuid: Some(crate::network::login::format_uuid(uuid)), name: name.clone() },
            Source::Console => wit::Sender { kind: wit::SenderKind::Console, uuid: None, name: "Console".to_string() },
            Source::Block { name, .. } => wit::Sender { kind: wit::SenderKind::CommandBlock, uuid: None, name: name.clone() },
        };

        // (ответ, успех)
        let (message, success) = with_host(None, move |host| {
            let needed = host.command_level(&name)?;

            if level < needed {
                return Some(("На это у тебя нет прав".to_string(), true));
            }

            let (success, message) = host.run_command(sender, &name, &args)?;

            Some((message, success))
        })?;

        let mut answer = Answer::saying(message);

        if !success {
            answer.result = None;
        }

        Some(answer)
    }
}

#[cfg(feature = "plugins")]
pub use enabled::{inventory_event, command_names, tab_complete, active, block_break, block_place, chat, damage, death, player_command, player_join, player_quit, run_command, shutdown, start, tick};

#[cfg(feature = "plugins")]
impl PlayerView {
    fn of_member(member: &crate::players::Member) -> PlayerView {
        PlayerView {
            uuid: member.uuid,
            name: member.name.clone(),
            game_mode: member.game_mode,
            dimension: member.dimension,
            x: member.position.x,
            y: member.position.y,
            z: member.position.z,
            yaw: member.position.yaw,
            pitch: member.position.pitch,
        }
    }

    /// Игрок по нику из списка сервера.
    pub fn find(shared: &Shared, name: &str) -> Option<PlayerView> {
        shared.players.lock().expect("список игроков захвачен другим потоком").find(name).map(|member| PlayerView::of_member(&member))
    }
}

// Без плагинов: заглушки, которые компилятор убирает целиком.
#[cfg(not(feature = "plugins"))]
mod disabled {
    use super::*;

    #[inline(always)]
    pub fn active() -> bool {
        false
    }

    pub fn start(_shared: Arc<Shared>) {}

    pub fn shutdown() {}

    pub fn tick(_now: u64) {}

    pub fn player_join(_shared: &Shared, _name: &str) {}

    pub fn player_quit(_view: &PlayerView) {}

    pub fn chat(_who: &PlayerView, _message: &str) -> ChatOutcome {
        ChatOutcome::Pass
    }

    pub fn block_break(_who: &PlayerView, _at: (i32, i32, i32), _block: &str) -> Verdict {
        Verdict::Allow
    }

    pub fn block_place(_who: &PlayerView, _at: (i32, i32, i32), _block: &str) -> Verdict {
        Verdict::Allow
    }

    pub fn player_command(_who: &PlayerView, _line: &str) -> Verdict {
        Verdict::Allow
    }

    pub fn damage(_who: &PlayerView, _cause: &str, amount: f32) -> Option<f32> { Some(amount) }
    pub fn inventory_event(_owner: &str, _who: &PlayerView, _handle: u32, _kind: u32, _click: (i32, i32, i32)) -> bool { true }
    pub fn death(_who: &PlayerView, _keep_inventory: bool) {}

    pub fn command_names(_shared: &Shared, _uuid: &[u8;16]) -> Vec<String> { Vec::new() }

    pub fn tab_complete(_shared: &Shared, _uuid: &[u8;16], _line: &str) -> Option<Vec<String>> { None }

    pub fn run_command(_command: &str, _source: &Source, _shared: &Shared) -> Option<Answer> {
        None
    }
}

#[cfg(not(feature = "plugins"))]
pub use disabled::{inventory_event, command_names, tab_complete, active, block_break, block_place, chat, damage, death, player_command, player_join, player_quit, run_command, shutdown, start, tick};

#[cfg(not(feature = "plugins"))]
impl PlayerView {
    pub fn find(_shared: &Shared, _name: &str) -> Option<PlayerView> {
        None
    }
}
