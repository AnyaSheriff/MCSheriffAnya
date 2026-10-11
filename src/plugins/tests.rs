// Проверки хозяина плагинов на настоящих компонентах.
//
// Компоненты — примеры из plugins-examples/ (собираются отдельно:
// `cd plugins-examples && cargo build --release --target wasm32-wasip2`).
// Нет собранного — ошибка: нельзя принять пропуск за успешную загрузку.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::api::mcsa::api::types as wit;
use super::host::Host;
use super::{ChatOutcome, PlayerView, Verdict};
use crate::players::{Member, Position};
use crate::shared::Shared;
use crate::world::World;

fn example_file(name: &str) -> Option<PathBuf> {
    let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("plugins-examples/target/wasm32-wasip2/release").join(format!("{}.wasm", name.replace('-', "_")));

    assert!(file.is_file(), "нет {} (соберите plugins-examples под wasm32-wasip2)", file.display());
    Some(file)
}

/// Своя папка на каждый вызов: параллельные проверки не удаляют чужие ops.
fn server_root(label: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let next = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tmp/test-plugins").join(format!("{label}-{}-{next}", std::process::id()));

    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    root
}

/// Кладёт пример в `<root>/<dir>/<name>/`.
fn install(root: &Path, dir: &str, example: &str, name: &str, extra: &str) -> bool {
    let Some(file) = example_file(example) else { return false };
    let folder = root.join(dir).join(name);

    std::fs::create_dir_all(&folder).unwrap();
    std::fs::copy(file, folder.join(format!("{name}.wasm"))).unwrap();
    std::fs::write(folder.join("plugin.toml"), format!("name = \"{name}\"\nversion = \"0.1.0\"\napi = \"0.1\"\n{extra}")).unwrap();
    true
}

fn shared() -> Arc<Shared> {
    let ops = server_root("ops").join("ops.json");
    let shared = Shared::new(World::in_memory(), World::in_memory(), PathBuf::new(), PathBuf::new(), Default::default(), crate::ops::Ops::open(&ops), crate::skins::Settings::default());

    shared.players.lock().unwrap().join(
        Member {
            uuid: [1; 16],
            name: "Первый".to_string(),
            game_mode: 1,
            entity_id: 1,
            skin: None,
            skin_parts: 0x7F,
            main_hand: 1,
            held: None,
            position: Position { x: 0.0, y: 70.0, z: 0.0, yaw: 0.0, pitch: 0.0, on_ground: true },
            dimension: Default::default(),
            sneaking: false,
            swings: 0,
            shoved: ((0.0, 0.0, 0.0), 0),
            unsettled: 0,
        },
        1,
    );

    Arc::new(shared)
}

fn view() -> PlayerView {
    PlayerView { uuid: [1; 16], name: "Первый".to_string(), game_mode: 1, dimension: Default::default(), x: 0.0, y: 70.0, z: 0.0, yaw: 0.0, pitch: 0.0 }
}

fn console() -> wit::Sender {
    wit::Sender { kind: wit::SenderKind::Console, uuid: None, name: "Console".to_string() }
}

#[test]
fn load_run_command_and_unload() {
    let root = server_root("hello");

    if !install(&root, "plugins", "hello", "hello", "") {
        return;
    }

    let mut host = Host::new(shared(), root).unwrap();

    assert!(host.load("hello").unwrap().starts_with("hello 0.1.0"));
    assert!(host.load("hello").is_err(), "второй раз загрузить нельзя");
    assert_eq!(host.command_level("hello"), Some(0));

    let (success, message) = host.run_command(console(), "hello", &["Аня".to_string()]).unwrap();

    assert!(success);
    assert_eq!(message, "Привет, Аня!");

    // Без аргумента — имя вызвавшего.
    assert_eq!(host.run_command(console(), "hello", &[]).unwrap().1, "Привет, Console!");

    host.player_join(&view());

    assert_eq!(host.unload("hello").unwrap(), "hello выгружен");
    assert!(host.command_level("hello").is_none());
    assert!(host.run_command(console(), "hello", &[]).is_none());
    // После выгрузки загружается снова — имя команды освободилось.
    assert!(host.load("hello").is_ok());
}

#[test]
fn chat_can_be_cancelled_and_changed() {
    let root = server_root("chat");

    if !install(&root, "plugins", "chat-filter", "chat-filter", "") {
        return;
    }

    let mut host = Host::new(shared(), root).unwrap();

    host.load("chat-filter").unwrap();

    assert_eq!(host.chat(&view(), "привет всем"), ChatOutcome::Pass);
    assert_eq!(host.chat(&view(), "это СПАМ"), ChatOutcome::Cancel);
    assert_eq!(host.chat(&view(), "ПРИВЕТ ВСЕМ"), ChatOutcome::Replace("привет всем".to_string()));
    assert_eq!(host.loaded.len(), 1, "плагин остался на месте");
}

#[test]
fn plugins_cannot_shadow_current_gameplay_and_admin_commands() {
    use super::api::mcsa::api::command::{CommandSpec, Host as _};

    let root = server_root("reserved");
    assert!(install(&root, "plugins", "hello", "hello", ""));
    let mut host = Host::new(shared(), root).unwrap();
    host.load("hello").unwrap();

    let ctx = host.loaded[0].store.data_mut();
    for name in ["advancement", "place", "loot", "ride", "tnt", "bossbar", "schedule", "datapack", "return", "teammsg"]
        .into_iter().chain(crate::commands_admin::NAMES.iter().copied())
    {
        let result = ctx.register(CommandSpec { name: name.to_uppercase(), description: String::new(), usage: String::new(), level: 0, permission: None });
        assert!(result.unwrap_err().contains("принадлежит ядру"), "{name}");
    }
    assert_eq!(ctx.commands.len(), 1);
    assert_eq!(host.run_command(console(), "hello", &[]).unwrap().1, "Привет, Console!");
}

#[test]
fn repeated_chat_calls_preserve_results_and_release_guest_returns() {
    let root = server_root("repeated-chat");

    if !install(&root, "plugins", "chat-filter", "chat-filter", "") {
        return;
    }

    let mut host = Host::new(shared(), root).unwrap();

    host.load("chat-filter").unwrap();
    let calls = host.loaded[0].stats.calls;
    let who = view();

    // Разные ветви ABI: пустой ответ, вызов хозяина из WASM и строка из
    // памяти гостя. Повтор проверяет также освобождение результата (post-return).
    for _ in 0..1000 {
        assert_eq!(host.chat(&who, "привет всем"), ChatOutcome::Pass);
        assert_eq!(host.chat(&who, "это СПАМ"), ChatOutcome::Cancel);
        assert_eq!(host.chat(&who, "ПРИВЕТ ВСЕМ"), ChatOutcome::Replace("привет всем".to_string()));
    }

    assert!(host.is_loaded("chat-filter"));
    assert_eq!(host.loaded[0].stats.calls - calls, 3000);
    host.unload("chat-filter").unwrap();
    host.load("chat-filter").unwrap();
    assert_eq!(host.chat(&who, "ПРИВЕТ ВСЕМ"), ChatOutcome::Replace("привет всем".to_string()));
}

#[test]
fn spawn_is_protected_with_the_radius_from_config() {
    let root = server_root("spawn");

    if !install(&root, "plugins", "protect-spawn", "protect-spawn", "") {
        return;
    }

    let shared = shared();
    let (sx, _, sz) = shared.world_in(crate::dimension::Dimension::Overworld).lock().unwrap().spawn_position();
    let mut host = Host::new(Arc::clone(&shared), root.clone()).unwrap();

    host.load("protect-spawn").unwrap();

    // Конфиг создан из текста по умолчанию.
    assert!(std::fs::read_to_string(root.join("plugins/protect-spawn/config.toml")).unwrap().contains("radius = 16"));

    let at = |dx: i32| wit::BlockPos { dimension: wit::Dimension::Overworld, x: sx as i32 + dx, y: 64, z: sz as i32 };

    assert_eq!(host.block_break(&view(), &at(3), "minecraft:stone"), Verdict::Deny);
    assert_eq!(host.block_place(&view(), &at(3), "minecraft:stone"), Verdict::Deny);
    assert_eq!(host.block_break(&view(), &at(40), "minecraft:stone"), Verdict::Allow);

    // Другой радиус — после перезагрузки.
    std::fs::write(root.join("plugins/protect-spawn/config.toml"), "radius = 50\n").unwrap();
    host.reload("protect-spawn").unwrap();
    assert_eq!(host.block_break(&view(), &at(40), "minecraft:stone"), Verdict::Deny);
}

#[test]
fn a_misbehaving_plugin_is_unloaded_and_the_host_goes_on() {
    let root = server_root("bad");

    if !install(&root, "plugins", "misbehave", "misbehave", "") || !install(&root, "plugins", "chat-filter", "chat-filter", "") {
        return;
    }

    let mut host = Host::new(shared(), root).unwrap();

    host.load("chat-filter").unwrap();

    for (word, why) in [("loop", "бесконечный цикл"), ("panic", "паника"), ("memory", "память")] {
        host.load("misbehave").unwrap();

        let started = std::time::Instant::now();

        // Хозяин отвечает, не падая и не зависая; плохой плагин выгружен.
        assert_eq!(host.chat(&view(), word), ChatOutcome::Pass, "{why}");
        assert!(!host.is_loaded("misbehave"), "{why}: плагин должен быть выгружен");
        assert!(started.elapsed().as_secs() < 5, "{why}: слишком долго — {:?}", started.elapsed());
        assert!(host.is_loaded("chat-filter"), "{why}: честный плагин не задет");
    }

    // Долгая, но честная работа в лимит укладывается.
    host.load("misbehave").unwrap();
    assert_eq!(host.chat(&view(), "slow"), ChatOutcome::Pass);
    assert!(host.is_loaded("misbehave"));
}

#[test]
fn sandbox_refuses_privileged_plugins_but_modules_get_them() {
    let root = server_root("sandbox");

    if !install(&root, "plugins", "modinfo", "modinfo", "") || !install(&root, "modules", "modinfo", "modinfo", "") {
        return;
    }

    // Один и тот же компонент: в plugins/ — отказ, в modules/ — работает.
    // `find_folder` ищет сперва модули, поэтому plugins/ убираем на время.
    let plugins_copy = root.join("plugins/modinfo");
    let hidden = root.join("hidden-modinfo");

    std::fs::rename(&plugins_copy, &hidden).unwrap();

    let mut host = Host::new(shared(), root.clone()).unwrap();
    let loaded = host.load("modinfo").unwrap();

    assert!(loaded.contains("(модуль)"));

    let (_, message) = host.run_command(console(), "modinfo", &[]).unwrap();

    assert_eq!(message, "Загружено: modinfo");
    host.unload("modinfo").unwrap();

    // Теперь — только в plugins/.
    std::fs::remove_dir_all(root.join("modules/modinfo")).unwrap();
    std::fs::rename(&hidden, &plugins_copy).unwrap();

    let error = host.load("modinfo").unwrap_err();

    assert!(error.contains("privileged") || error.contains("не запустился"), "{error}");
    assert!(!host.is_loaded("modinfo"));
}

#[test]
fn dependencies_load_first_and_block_unload() {
    let root = server_root("deps");

    if !install(&root, "plugins", "hello", "base", "") || !install(&root, "plugins", "chat-filter", "child", "dependencies = [\"base\"]\n") {
        return;
    }

    let mut host = Host::new(shared(), root).unwrap();

    assert!(host.load("child").unwrap_err().contains("нужен плагин base"));

    let report = host.load_all();

    assert!(report.iter().all(Result::is_ok), "{report:?}");
    assert_eq!(host.loaded.iter().map(|plugin| plugin.manifest.name.as_str()).collect::<Vec<_>>(), ["base", "child"]);
    assert!(host.unload("base").unwrap_err().contains("child"));

    // Перезагрузка основы тянет за собой зависящего.
    assert!(host.reload("base").unwrap().contains("child"));
    assert!(host.is_loaded("base") && host.is_loaded("child"));

    host.unload("child").unwrap();
    host.unload("base").unwrap();
    assert!(host.is_empty());
}

#[test]
fn repeating_tasks_run_on_their_ticks() {
    let root = server_root("tasks");

    if !install(&root, "plugins", "hello", "hello", "") {
        return;
    }

    let mut host = Host::new(shared(), root).unwrap();

    host.load("hello").unwrap();

    // `hello` просит повтор раз в 1200 тактов, первый раз через 1200.
    assert_eq!(host.loaded[0].store.data().tasks.len(), 1);
    host.tick(10);
    assert_eq!(host.loaded[0].store.data().tasks[0].due, 1200);
    host.tick(1200);
    assert_eq!(host.loaded[0].store.data().tasks[0].due, 2400);
    assert!(host.is_loaded("hello"));
}

#[test]
fn quit_and_player_command_events_reach_plugins() {
    let root = server_root("events");

    if !install(&root, "plugins", "hello", "hello", "") {
        return;
    }

    let mut host = Host::new(shared(), root).unwrap();

    host.load("hello").unwrap();
    host.player_quit(&view());
    assert_eq!(host.player_command(&view(), "gamemode creative"), Verdict::Allow);
}

#[test]
fn plugman_and_plugin_commands_go_through_the_command_runner() {
    let root = server_root("plugman");

    if !install(&root, "plugins", "hello", "hello", "") {
        return;
    }

    let shared = shared();

    super::enabled::start_in(Arc::clone(&shared), root);
    assert!(super::active());

    let run = |text: &str| crate::commands::run(text, &crate::commands::Source::Console, &shared).reply;

    assert!(run("plugman list").contains("hello 0.1.0"));
    assert_eq!(run("hello Аня"), "Привет, Аня!");
    assert!(run("plugman info hello").contains("/hello"));
    assert!(run("plugman unload hello").contains("выгружен"));
    assert!(run("plugman unload hello").contains("не загружен"));
    assert!(!super::active());
    assert!(run("plugman load hello").contains("Загружен hello"));
    assert!(run("plugman reload hello").contains("перезагружено"));
    assert!(run("plugman load nope").contains("нет такого плагина"));
    assert!(run("plugman bogus").contains("/plugman list"));

    // Игрок без прав ни плагины не трогает, ни команду плагина с уровнем выше не вызовет.
    let player = crate::commands::Source::Player { uuid: [1; 16], name: "Первый".to_string() };

    assert_eq!(crate::commands::run("plugman list", &player, &shared).reply, "На это у тебя нет прав");

    // Игрок — оператор: /plugman доступен.
    shared.ops.lock().unwrap().give(&[1; 16], "Первый", crate::ops::HIGHEST_LEVEL);
    assert!(crate::commands::run("plugman list", &player, &shared).reply.contains("hello"));

    super::shutdown();
    assert!(!super::active());
}

/// Замер только хозяина: ни сеть, ни такт, ни сервер не запускаются.
#[test]
#[ignore = "ручной замер: --ignored --nocapture --test-threads=1"]
fn benchmark_host() {
    fn rss() -> u64 {
        std::fs::read_to_string("/proc/self/status").unwrap().lines()
            .find_map(|line| line.strip_prefix("VmRSS:").map(|value| value.split_whitespace().next().unwrap().parse().unwrap())).unwrap()
    }
    let root = server_root("benchmark");
    assert!(install(&root, "plugins", "chat-filter", "chat-filter", ""));
    let shared = shared();
    let base_rss = rss();
    let start = std::time::Instant::now();
    let mut host = Host::new(shared, root).unwrap();
    let engine_ms = start.elapsed().as_secs_f64() * 1000.0;
    let empty_rss = rss();
    let start = std::time::Instant::now();
    host.load("chat-filter").unwrap();
    let load_ms = start.elapsed().as_secs_f64() * 1000.0;
    let loaded_rss = rss();
    let who = view();
    for _ in 0..1000 {
        assert_eq!(host.chat(&who, "привет всем"), ChatOutcome::Pass);
    }
    let start = std::time::Instant::now();
    for _ in 0..100_000 {
        assert_eq!(std::hint::black_box(host.chat(&who, std::hint::black_box("привет всем"))), ChatOutcome::Pass);
    }
    let hot_ms = start.elapsed().as_secs_f64() * 1000.0;
    assert!(host.is_loaded("chat-filter"));
    println!("BENCH base_rss_kib={base_rss} empty_rss_kib={empty_rss} loaded_rss_kib={loaded_rss} engine_ms={engine_ms:.3} load_ms={load_ms:.3} hot_100k_ms={hot_ms:.3}");
}
