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
    assert_eq!(host.damage(&view(), "CUSTOM", 4.0), Some(4.0));
    assert_eq!(host.damage(&view(), "FALL", 5.0), Some(5.0));
    let mut destination = view();destination.x += 1.0;
    assert_eq!(host.player_move(&view(), &destination, true).unwrap().x, destination.x);
    assert_eq!(host.respawn(&view(), &destination, false, false).x, destination.x);
    assert_eq!(host.interact(&view(), 3, 0, None, 6), 0);
    host.death(&view(), false);

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

    // `hello` просит повтор раз в 1200 тактов после включения. Общий
    // счётчик уже могли продвинуть другие параллельные проверки.
    assert_eq!(host.loaded[0].store.data().tasks.len(), 1);
    let enabled_at = host.loaded[0].store.data().tick;
    host.tick(enabled_at + 10);
    assert_eq!(host.loaded[0].store.data().tasks[0].due, enabled_at + 1200);
    host.tick(enabled_at + 1200);
    assert_eq!(host.loaded[0].store.data().tasks[0].due, enabled_at + 2400);
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

fn java_info(shared: &Shared) {
    shared.players.lock().unwrap().ui.info.insert([1;16], crate::presentation::Info {ping:42,health:17.0,max_health:24.0,food:18,level:3,exp:0.5,..Default::default()});
}

#[test]
fn custom_tablist_loads_defaults_yields_core_and_cleans_up() {
    let root = server_root("custom-tablist");
    assert!(install(&root, "plugins", "custom-tablist", "custom-tablist", ""));
    let shared = shared();
    java_info(&shared);
    let mut host = Host::new(Arc::clone(&shared), root.clone()).unwrap();
    host.load("custom-tablist").unwrap();
    let first_tick=host.loaded[0].store.data().tick+1;host.tick(first_tick);
    assert_eq!(host.command_level("tablist"), Some(2));
    assert_eq!(std::fs::read_to_string(root.join("plugins/custom-tablist/data/config.yml")).unwrap(), include_str!("../../plugins-examples/custom-tablist/config.yml"));
    {
        let mut players = shared.players.lock().unwrap();
        assert_eq!(players.ui.headers.get(&[1;16]).map(String::as_str), Some("custom-tablist"));
        let owned = &players.ui.owned[&("custom-tablist".into(), [1;16])];
        assert_eq!(owned.teams.len(),1);
        assert_eq!(owned.objectives.len(),1);
        let packets = players.take_orders(&[1;16]);
        assert!(packets.iter().any(|o|matches!(o, crate::players::Order::Packet(p) if p[0]==0x7a)));
        assert!(packets.iter().any(|o|matches!(o, crate::players::Order::Packet(p) if p[0]==0x46)));
    }
    crate::presentation::tick_footer(&shared);
    assert!(shared.players.lock().unwrap().take_orders(&[1;16]).is_empty());
    let base_tick = host.loaded[0].store.data().tick;
    host.tick(base_tick + 19);
    assert!(shared.players.lock().unwrap().take_orders(&[1;16]).is_empty());
    host.tick(base_tick + 20);
    assert!(shared.players.lock().unwrap().take_orders(&[1;16]).iter().any(|o|matches!(o, crate::players::Order::Packet(p) if p[0]==0x7a)));
    host.unload("custom-tablist").unwrap();
    {
        let mut players=shared.players.lock().unwrap();
        assert!(players.ui.headers.is_empty());assert!(players.ui.owned.is_empty());assert!(players.ui.list_names.is_empty());assert!(players.ui.slots.is_empty());
        players.take_orders(&[1;16]);
    }
    crate::presentation::tick_footer(&shared);
    assert!(shared.players.lock().unwrap().take_orders(&[1;16]).iter().any(|o|matches!(o, crate::players::Order::Packet(p) if p[0]==0x7a)));
}

#[test]
fn custom_tablist_reload_commands_chat_and_all_hud_features() {
    let root=server_root("custom-all");
    assert!(install(&root,"plugins","custom-tablist","custom-tablist",""));
    let folder=root.join("plugins/custom-tablist/data");std::fs::create_dir_all(&folder).unwrap();
    let config="bossbar:\n  enabled: true\n  format: '<red>%tps% TPS'\n  progress: 0.25\n  color: BLUE\n  style: SEGMENTED_10\nactionbar:\n  enabled: true\n  lines: ['first', 'second']\n  update-interval: 10\nscoreboard:\n  enabled: true\n  title: '<bold>Статистика'\n  lines: ['%online%', '%mspt%', '%unknown%']\n";
    std::fs::write(folder.join("config.yml"),config).unwrap();
    let shared=shared();java_info(&shared);
    let mut host=Host::new(Arc::clone(&shared),root).unwrap();
    host.load("custom-tablist").unwrap();
    let first_tick=host.loaded[0].store.data().tick+1;host.tick(first_tick);
    {
        let mut players=shared.players.lock().unwrap();
        let owned=&players.ui.owned[&("custom-tablist".into(),[1;16])];
        assert_eq!(owned.bars.len(),1);assert_eq!(owned.objectives.len(),2);
        let bar=owned.bars.values().next().unwrap();assert_eq!((bar.color,bar.style,bar.value),(1,2,250000));
        assert_eq!(players.ui.slots.len(),2);
        assert!(players.take_orders(&[1;16]).iter().any(|o|matches!(o, crate::players::Order::Packet(p) if p[0]==0x57)));
    }
    assert_eq!(host.chat(&view(),"<red>hello %player%"),ChatOutcome::Cancel);
    assert!(shared.chat.lock().unwrap().since(0).iter().any(|line| line.contains("Первый: <red>hello %player%")));
    assert!(host.run_command(console(),"cprefix",&["Первый".into(),"<gold>[Admin]".into()]).unwrap().0);
    assert!(std::fs::read_to_string(folder.join("config.yml")).unwrap().contains("[Admin]"));
    assert!(host.run_command(console(),"tablist",&["toggle".into(),"all".into(),"off".into()]).unwrap().0);
    assert!(shared.players.lock().unwrap().ui.owned.is_empty());
    assert_eq!(host.chat(&view(),"hello"),ChatOutcome::Pass);
    std::fs::write(folder.join("config.yml"),"bad: [").unwrap();
    assert!(!host.run_command(console(),"tablist",&["reload".into()]).unwrap().0);
    assert!(host.is_loaded("custom-tablist"));
}

#[test]
fn presentation_api_validates_ownership_and_bedrock_is_not_sent_java_packets() {
    use super::api::mcsa::api::presentation::Host as _;
    let root=server_root("ui-api");assert!(install(&root,"plugins","hello","hello",""));
    let shared=shared();let mut host=Host::new(Arc::clone(&shared),root).unwrap();host.load("hello").unwrap();
    let uuid=crate::network::login::format_uuid(&[1;16]);
    let ctx=host.loaded[0].store.data_mut();
    assert!(!ctx.set_tab_header_footer(uuid.clone(),"\"head\"".into(),"\"foot\"".into()));
    java_info(&shared);
    assert!(!ctx.set_tab_header_footer(uuid.clone(),"not JSON".into(),"\"foot\"".into()));
    assert!(!ctx.set_action_bar("bad uuid".into(),"\"text\"".into()));
    assert!(!ctx.set_action_bar(uuid.clone(),r#"{"text":"a","extra":"bad"}"#.into()));
    assert!(ctx.set_tab_header_footer(uuid.clone(),"\"head\"".into(),"\"foot\"".into()));
    ctx.name="other".into();
    assert!(!ctx.reset_tab_header_footer(uuid.clone()));
    assert!(ctx.set_tab_header_footer(uuid.clone(),"\"other\"".into(),"\"foot\"".into()));
    ctx.name="hello".into();
    assert!(ctx.reset_player(uuid.clone()));
    assert_eq!(shared.players.lock().unwrap().ui.headers.get(&[1;16]).map(String::as_str),Some("other"));
    let info=ctx.get_player_info(uuid.clone()).unwrap();assert_eq!((info.ping,info.health,info.max_health),(42,17.0,24.0));assert_eq!(info.world,"world");
    assert_eq!(ctx.get_server_info().online,1);
    shared.players.lock().unwrap().leave(&[1;16]);
    assert!(shared.players.lock().unwrap().ui.headers.is_empty());
    assert!(ctx.get_player_info(uuid).is_none());
}

#[test]
fn presentation_resource_updates_removal_and_crash_cleanup() {
    use super::api::mcsa::api::presentation::{self as ui, Host as _};
    let root=server_root("ui-resources");assert!(install(&root,"plugins","misbehave","misbehave",""));
    let shared=shared();java_info(&shared);
    let mut host=Host::new(Arc::clone(&shared),root).unwrap();host.load("misbehave").unwrap();
    let uuid=crate::network::login::format_uuid(&[1;16]);
    let ctx=host.loaded[0].store.data_mut();
    let team=ui::Team{id:"test".into(),prefix:"\"pre\"".into(),suffix:"\"post\"".into(),color:"green".into(),visibility:"never".into(),members:vec!["Первый".into()]};
    assert!(ctx.set_team(uuid.clone(),team.clone()));
    shared.players.lock().unwrap().take_orders(&[1;16]);
    assert!(ctx.set_team(uuid.clone(),team));assert!(shared.players.lock().unwrap().take_orders(&[1;16]).is_empty());
    let objective=ui::Objective{id:"test".into(),title:"\"title\"".into(),hearts:false};
    assert!(ctx.set_objective(uuid.clone(),objective));
    assert!(ctx.set_display_slot(uuid.clone(),"sidebar".into(),Some("test".into())));
    assert!(!ctx.set_display_slot(uuid.clone(),"bad".into(),Some("test".into())));
    assert!(!ctx.set_display_slot(uuid.clone(),"sidebar".into(),Some("missing".into())));
    assert!(ctx.set_score(uuid.clone(),ui::Score{objective:"test".into(),holder:"row".into(),value:17,display:Some("\"text\"".into()),number:Some("\"❤\"".into())}));
    assert!(ctx.remove_score(uuid.clone(),"test".into(),"row".into()));
    assert!(ctx.set_display_slot(uuid.clone(),"sidebar".into(),None));
    assert!(ctx.remove_objective(uuid.clone(),"test".into()));
    assert!(ctx.remove_team(uuid.clone(),"test".into()));
    let mut bar=ui::BossBar{id:"bar".into(),title:"\"title\"".into(),progress:0.5,color:"red".into(),style:"progress".into()};
    assert!(ctx.set_boss_bar(uuid.clone(),bar.clone()));shared.players.lock().unwrap().take_orders(&[1;16]);
    bar.progress=0.25;assert!(ctx.set_boss_bar(uuid.clone(),bar));
    let packets=shared.players.lock().unwrap().take_orders(&[1;16]);
    assert_eq!(packets.len(),1);
    assert!(matches!(&packets[0],crate::players::Order::Packet(p) if p[0]==crate::scoreboard::BOSS_PACKET as u8 && p[17]==2));
    assert!(ctx.remove_boss_bar(uuid.clone(),"bar".into()));
    assert!(ctx.set_tab_header_footer(uuid.clone(),"\"head\"".into(),"\"foot\"".into()));
    assert!(ctx.set_action_bar(uuid,"\"action\"".into()));
    assert_eq!(host.chat(&view(),"panic"),ChatOutcome::Pass);
    assert!(!host.is_loaded("misbehave"));
    let players=shared.players.lock().unwrap();assert!(players.ui.headers.is_empty());assert!(players.ui.actions.is_empty());assert!(players.ui.owned.is_empty());
}

#[test]
fn custom_tablist_handles_a_default_capacity_server() {
    let root=server_root("custom-capacity");assert!(install(&root,"plugins","custom-tablist","custom-tablist",""));
    let shared=shared();java_info(&shared);
    {
        let mut players=shared.players.lock().unwrap();
        let original=players.members()[0].clone();
        for n in 2..=20u8 {
            let mut member=original.clone();member.uuid=[n;16];member.name=format!("Player{n}");member.entity_id=i32::from(n);
            players.join(member,i32::from(n));
            players.ui.info.insert([n;16],crate::presentation::Info{health:20.0,max_health:20.0,..Default::default()});
        }
    }
    let mut host=Host::new(Arc::clone(&shared),root).unwrap();
    host.load("custom-tablist").unwrap();
    let first_tick=host.loaded[0].store.data().tick+1;host.tick(first_tick);let base_tick=host.loaded[0].store.data().tick;host.tick(base_tick+20);
    assert!(host.is_loaded("custom-tablist"));
    assert_eq!(shared.players.lock().unwrap().ui.headers.len(),20);
}

/// Сначала build-fixtures.py и translate.py для трёх JAR в tmp/java-test/plugins.
#[test]
#[ignore = "нужны переведённые Java-компоненты из tools/plugin-translator"]
fn translated_bukkit_jars() {
    let root = server_root("java-jars");
    for name in ["java-command", "java-events", "java-scheduler"] {
        let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tmp/java-test/plugins").join(name);
        let folder = root.join("plugins").join(name);
        std::fs::create_dir_all(folder.join("data")).unwrap();
        for file in ["plugin.toml", "plugin.wasm"] {
            std::fs::copy(source.join(file), folder.join(file)).unwrap();
        }
        if name == "java-scheduler" {
            std::fs::copy(source.join("data/config.yml"), folder.join("data/config.yml")).unwrap();
        }
    }
    let shared = shared();
    let mut host = Host::new(Arc::clone(&shared), root).unwrap();
    for name in ["java-command", "java-events", "java-scheduler"] { host.load(name).unwrap(); }
    let sender = wit::Sender { kind: wit::SenderKind::Player, uuid: Some(crate::network::login::format_uuid(&[1;16])), name: "Первый".into() };
    assert!(host.run_command(sender.clone(), "javahello", &[]).unwrap().0);
    assert_eq!(host.complete(sender.clone(), 4, "javahello", vec!["java-tab".into()]).unwrap(), vec!["java-tab-ok", "java-tab-two"]);
    host.player_join(&view());
    assert_eq!(host.damage(&view(), "CUSTOM", 4.0), Some(1.0));
    assert_eq!(host.damage(&view(), "FALL", 5.0), None);
    host.death(&view(), false);
    assert_eq!(host.chat(&view(), "cancel-java"), ChatOutcome::Cancel);
    assert_eq!(host.chat(&view(), "обычный чат"), ChatOutcome::Pass);
    assert_eq!(host.player_command(&view(), "java-blocked"), Verdict::Deny);
    assert_eq!(host.block_break(&view(), &wit::BlockPos { dimension: wit::Dimension::Overworld, x: 3, y: 70, z: 0 }, "minecraft:stone"), Verdict::Deny);
    let messages = shared.players.lock().unwrap().take_orders(&[1;16]).into_iter().filter(|order| matches!(order, crate::players::Order::Message { .. })).count();
    assert!(messages >= 5, "ответ команды, вход и три отмены");
    let due = host.loaded.iter().flat_map(|plugin| &plugin.store.data().tasks).map(|task| task.due).min().unwrap();
    host.tick(due);
    assert_eq!(host.loaded.len(), 3);
    assert!(shared.chat.lock().unwrap().count() > 0, "задача прочитала YAML и разослала сообщение");
    host.player_quit(&view());
    for name in ["java-command", "java-events", "java-scheduler"] { host.unload(name).unwrap(); }
    assert!(host.is_empty());
}

#[test]
fn bukkit_host_operations_remain_typed_and_bounded() {
    use super::api::mcsa::api::{command::Host as _, player::Host as _, world::Host as _};
    let root = server_root("bukkit-api");
    install(&root, "plugins", "hello", "hello", "");
    let shared = shared();
    let mut host = Host::new(Arc::clone(&shared), root).unwrap();
    host.load("hello").unwrap();
    let ctx = host.loaded[0].store.data_mut();
    let uuid = crate::network::login::format_uuid(&[1;16]);
    assert_eq!(ctx.operator_level(uuid.clone()), Some(0));
    assert_eq!(ctx.operator_level("ошибка".into()), None);
    shared.players.lock().unwrap().ui.info.insert([1;16], crate::presentation::Info { health: 20.0, max_health: 20.0, ..Default::default() });
    for value in [f64::NAN, f64::INFINITY, -1.0, 20.1] { assert!(ctx.set_health(uuid.clone(), value).is_err()); }
    assert!(ctx.set_health(uuid.clone(), 18.0).is_ok());
    assert!(ctx.set_health(uuid.clone(), 0.0).is_ok());
    assert!(ctx.set_health("не UUID".into(), 1.0).is_err());
    let effect = super::api::mcsa::api::player::PotionEffect { kind: "night_vision".into(), duration: 100, amplifier: 0, ambient: false, particles: false, icon: true };
    assert!(ctx.add_potion_effect(uuid.clone(), effect.clone()).is_ok());
    assert!(ctx.add_potion_effect(uuid.clone(), super::api::mcsa::api::player::PotionEffect { duration: -2, ..effect.clone() }).is_err());
    assert!(ctx.add_potion_effect(uuid.clone(), super::api::mcsa::api::player::PotionEffect { amplifier: 256, ..effect.clone() }).is_err());
    assert!(ctx.add_potion_effect(uuid.clone(), super::api::mcsa::api::player::PotionEffect { kind: "несуществующий".into(), ..effect }).is_err());
    assert!(ctx.remove_potion_effect(uuid.clone(), "night_vision".into()).is_ok());
    let orders = shared.players.lock().unwrap().take_orders(&[1;16]);
    assert!(orders.iter().any(|order| matches!(order, crate::players::Order::Health { value } if *value == 18.0)));
    assert!(orders.iter().any(|order| matches!(order, crate::players::Order::Effect(effect) if effect.show_icon && !effect.visible)));
    assert!(ctx.set_game_mode(uuid.clone(), wit::GameMode::Survival));
    assert_eq!(shared.players.lock().unwrap().members()[0].game_mode, 0);
    assert!(ctx.set_inventory_slot(uuid.clone(), 36, Some(wit::ItemStack { item: "stone".into(), count: 3 })).is_ok());
    assert!(ctx.set_inventory_slot(uuid.clone(), 8, None).is_err());
    assert!(ctx.set_inventory_slot(uuid, 36, Some(wit::ItemStack { item: "stone".into(), count: 65 })).is_err());
    ctx.set_time(wit::Dimension::Overworld, 25001);
    assert_eq!(ctx.time(wit::Dimension::Overworld), 1001);
    assert!(ctx.set_weather(wit::Dimension::Overworld, "thunder".into(), 100).is_ok());
    assert_eq!(ctx.weather(wit::Dimension::Overworld), "thunder");
    assert!(ctx.set_weather(wit::Dimension::Overworld, "ошибка".into(), 100).is_err());
    assert!(ctx.set_weather(wit::Dimension::Overworld, "rain".into(), u32::MAX).is_err());
    let border = ctx.get_border(wit::Dimension::Overworld);
    assert_eq!(border.size, 59_999_968.0);
    assert!(ctx.set_border(wit::Dimension::Overworld, super::api::mcsa::api::world::Border { size: f64::NAN, center_x: 0.0, center_z: 0.0 }).is_err());
    assert!(ctx.set_border(wit::Dimension::Overworld, super::api::mcsa::api::world::Border { size: 10_000.0, center_x: 1.0, center_z: -2.0 }).is_ok());
    assert_eq!(ctx.get_border(wit::Dimension::Overworld).size, 10_000.0);
    assert_eq!(ctx.get_border(wit::Dimension::Nether).size, 59_999_968.0);
    for name in ["auth", "changepassword", "op", "stop"] {
        let spec = super::api::mcsa::api::command::CommandSpec { name: name.into(), description: String::new(), usage: String::new(), level: 0, permission: None };
        assert!(ctx.register(spec).is_err(), "имя ядра нельзя занимать");
    }
}

#[test]
fn translated_items_and_attributes_are_checked_and_project_pending_edits() {
    use super::api::mcsa::api::{inventory::{self,Host as _},attributes::{self,Host as _}};
    let root=server_root("item-attributes");install(&root,"plugins","hello","hello","");
    let shared=shared();java_info(&shared);
    let mut host=Host::new(Arc::clone(&shared),root).unwrap();host.load("hello").unwrap();
    let ctx=host.loaded[0].store.data_mut();let uuid=crate::network::login::format_uuid(&[1;16]);
    let item=inventory::Item {kind:"diamond_sword".into(),count:1,metadata:r#"{"damage":3,"maxDamage":10,"pdc":{"hello:ability":{"type":"string","value":"blink"}}}"#.into()};
    assert!(ctx.set(uuid.clone(),36,Some(item.clone())).is_ok());
    let snapshot=ctx.snapshot(uuid.clone()).unwrap();assert!(snapshot[36].as_ref().unwrap().metadata.contains("blink"));
    let stack=shared.players.lock().unwrap().ui.info[&[1;16]].inventory[36].unwrap();
    assert_eq!(crate::combat::max_damage(&stack),10);
    let mut forged=item.clone();forged.metadata=r#"{"_native":99999}"#.into();assert!(ctx.set(uuid.clone(),36,Some(forged)).is_err());
    let mut excessive=item;excessive.count=2;assert!(ctx.set(uuid.clone(),36,Some(excessive)).is_err());
    assert!(ctx.set(uuid.clone(),0,None).is_err());
    assert!(ctx.set_base(uuid.clone(),"attack_damage".into(),7.0).is_ok());
    let modifier=attributes::Modifier {id:"hello:race".into(),amount:0.5,operation:1};
    assert!(ctx.add(uuid.clone(),"attack_damage".into(),modifier.clone()).is_ok());
    assert_eq!(ctx.get(uuid.clone(),"attack_damage".into()).unwrap().value,10.5);
    assert!(ctx.add(uuid.clone(),"attack_damage".into(),modifier).is_err());
    assert!(ctx.remove(uuid.clone(),"attack_damage".into(),"minecraft:sprinting".into()).is_err());
    assert!(ctx.remove(uuid.clone(),"attack_damage".into(),"hello:race".into()).is_ok());
    assert_eq!(ctx.get(uuid.clone(),"attack_damage".into()).unwrap().value,7.0);
    assert!(ctx.set_base(uuid,"attack_damage".into(),f64::NAN).is_err());
}

#[test]
fn plugin_chests_validate_items_queue_real_windows_and_expire_on_unload() {
    use super::api::mcsa::api::inventory::{self, Host as _};
    let root = server_root("chests"); install(&root, "plugins", "hello", "hello", "");
    let shared = shared(); java_info(&shared);
    let mut host = Host::new(Arc::clone(&shared), root).unwrap(); host.load("hello").unwrap();
    let ctx = host.loaded[0].store.data_mut();
    let uuid = crate::network::login::format_uuid(&[1; 16]);
    for size in [0, 8, 10, 55, u32::MAX] { assert!(ctx.chest_create(size, r#"{"text":"Race"}"#.into()).is_err()); }
    assert!(ctx.chest_create(27, r#"{"nbt":"secret"}"#.into()).is_err());
    let handle = ctx.chest_create(27, r#"{"text":"Race","color":"red","extra":[]}"#.into()).unwrap();
    let item = inventory::Item { kind: "diamond".into(), count: 1, metadata: r#"{"pdc":{"hello:race":{"type":"string","value":"elf"}}}"#.into() };
    assert!(ctx.chest_set(handle, 13, Some(item.clone())).is_ok());
    assert!(ctx.chest_set(handle, 27, Some(item.clone())).is_err());
    assert!(ctx.chest_set(handle + 1, 0, None).is_err());
    let mut forged = item; forged.metadata = r#"{"_native":99999}"#.into();
    assert!(ctx.chest_set(handle, 0, Some(forged)).is_err());
    assert!(ctx.chest_snapshot(handle).unwrap()[13].as_ref().unwrap().metadata.contains("elf"));
    assert!(ctx.chest_open(crate::network::login::format_uuid(&[2; 16]), handle).is_err());
    assert!(ctx.chest_open(uuid.clone(), handle).is_ok());
    assert_eq!(ctx.chest_current(uuid.clone()), Some(handle));
    assert!(ctx.chest_close(uuid).is_ok());
    let orders = shared.players.lock().unwrap().take_orders(&[1; 16]);
    let chest = orders.iter().find_map(|order| match order { crate::players::Order::PluginOpen { chest } => Some(chest.clone()), _ => None }).unwrap();
    assert!(chest.lock().unwrap().active);
    assert!(!chest.lock().unwrap().title.contains("extra"));
    assert!(orders.iter().any(|order| matches!(order, crate::players::Order::PluginClose { owner } if owner == "hello")));
    host.unload("hello").unwrap();
    assert!(!chest.lock().unwrap().active);
}

#[test]
fn plugin_respawn_points_are_validated_and_queued_without_changing_worlds() {
    use super::api::mcsa::api::player::Host as _;
    let root = server_root("plugin-spawn");install(&root, "plugins", "hello", "hello", "");
    let shared = shared();java_info(&shared);
    let mut host = Host::new(Arc::clone(&shared), root).unwrap();host.load("hello").unwrap();
    let ctx = host.loaded[0].store.data_mut();let uuid = crate::network::login::format_uuid(&[1;16]);
    let point = wit::Location {dimension: wit::Dimension::Nether, x: -0.5, y: 70.0, z: 1.5, yaw: Some(90.0), pitch: None};
    assert!(ctx.set_respawn(uuid.clone(), Some(wit::Location {x: f64::NAN, ..point})).is_err());
    assert!(ctx.set_respawn(uuid.clone(), Some(point)).is_ok());
    let orders = shared.players.lock().unwrap().take_orders(&[1;16]);
    assert!(orders.iter().any(|order| matches!(order, crate::players::Order::PluginSpawn {point: Some(point)} if point.dimension == crate::dimension::Dimension::Nether && point.pos == (-1,70,1) && !point.forced)));
    assert_eq!(shared.players.lock().unwrap().members()[0].dimension, crate::dimension::Dimension::Overworld);
    assert!(ctx.set_respawn(uuid, None).is_ok());
}

#[test]
fn entity_host_mutates_native_items_and_rejects_invalid_motion() {
    use super::api::mcsa::api::{entities::Host as _, inventory::Item};
    let root=server_root("entity-api");assert!(install(&root,"plugins","hello","hello",""));
    let shared=shared();let mut host=Host::new(Arc::clone(&shared),root).unwrap();host.load("hello").unwrap();let ctx=host.loaded[0].store.data_mut();
    let at=wit::Location {dimension:wit::Dimension::Overworld,x:3.0,y:80.0,z:4.0,yaw:None,pitch:None};
    let item=Item {kind:"potion".into(),count:1,metadata:r#"{"potion":"minecraft:long_poison"}"#.into()};
    let uuid=ctx.drop_item(at,item,false).unwrap();assert!(ctx.valid(uuid.clone()));
    let stack=ctx.item_stack(uuid.clone()).unwrap();assert!(stack.metadata.contains("minecraft:long_poison"));
    let native=shared.items_in(crate::dimension::Dimension::Overworld).lock().unwrap().items()[0].stack;
    assert_eq!(native.extras().potion,crate::survival::potion_number("long_poison"));
    assert!(ctx.set_velocity(uuid.clone(),super::api::mcsa::api::entities::Vector {x:f64::NAN,y:0.0,z:0.0}).is_err());
    ctx.set_velocity(uuid.clone(),super::api::mcsa::api::entities::Vector {x:0.2,y:0.4,z:-0.3}).unwrap();assert_eq!(ctx.velocity(uuid.clone()).unwrap().y,0.4);
    ctx.set_item_stack(uuid.clone(),Item {kind:"diamond".into(),count:2,metadata:String::new()}).unwrap();assert_eq!(ctx.item_stack(uuid.clone()).unwrap().count,2);
    assert!(ctx.item_teleport(uuid.clone(),wit::Location {x:9.0,..at}).unwrap());assert_eq!(ctx.item_location(uuid.clone()).unwrap().x,9.0);
    assert!(ctx.item_teleport(uuid.clone(),wit::Location {dimension:wit::Dimension::Nether,..at}).is_err());
    assert!(ctx.remove(crate::network::login::format_uuid(&[1;16])).is_err());ctx.remove(uuid.clone()).unwrap();assert!(!ctx.valid(uuid.clone()));assert!(ctx.item_stack(uuid).is_err());
    assert!(ctx.drop_item(wit::Location {y:f64::INFINITY,..at},Item {kind:"stone".into(),count:1,metadata:String::new()},true).is_err());
}

#[test]
fn player_state_and_mutations_use_the_connection_snapshot() {
    use super::api::mcsa::api::player::Host as _;
    let root=server_root("player-state-api");assert!(install(&root,"plugins","hello","hello",""));
    let shared=shared();shared.players.lock().unwrap().ui.info.insert([1;16],crate::presentation::Info {plugin_state:crate::presentation::PlayerState {fire_ticks:80,in_water:true,eye_height:1.27,effects:vec![crate::survival::Effect {kind:crate::survival::EffectKind::Poison,amplifier:1,ticks_left:90,ambient:false,visible:true,show_icon:false}],..Default::default()},..Default::default()});
    let mut host=Host::new(Arc::clone(&shared),root).unwrap();host.load("hello").unwrap();let ctx=host.loaded[0].store.data_mut();let uuid=crate::network::login::format_uuid(&[1;16]);
    let state=ctx.get_state(uuid.clone()).unwrap();assert!(state.in_water&&state.on_ground);assert_eq!(state.fire_ticks,80);assert_eq!(state.effects[0].kind,"poison");assert_eq!(state.effects[0].duration,90);assert!(!state.effects[0].icon);
    ctx.set_fire_ticks(uuid.clone(),0).unwrap();assert_eq!(ctx.get_state(uuid.clone()).unwrap().fire_ticks,0);
    assert!(ctx.damage(uuid.clone(),-1.0).is_err());assert!(ctx.damage(uuid.clone(),f64::NAN).is_err());assert!(ctx.give_experience(uuid.clone(),-1).is_err());ctx.damage(uuid.clone(),3.0).unwrap();ctx.give_experience(uuid.clone(),8).unwrap();
    let orders=shared.players.lock().unwrap().take_orders(&[1;16]);assert!(orders.iter().any(|o| matches!(o,crate::players::Order::Hurt {amount,..} if *amount==3.0)));assert!(orders.iter().any(|o| matches!(o,crate::players::Order::Experience {amount:8,levels:false,set:false})));
    assert!(ctx.get_state("не UUID".into()).is_err());
}

#[test]
fn plugin_flight_projects_state_and_rejects_unpermitted_flying() {
    use super::api::mcsa::api::player::Host as _;
    let root=server_root("flight-api");assert!(install(&root,"plugins","hello","hello",""));
    let shared=shared();let mut host=Host::new(Arc::clone(&shared),root.clone()).unwrap();host.load("hello").unwrap();
    shared.players.lock().unwrap().ui.info.insert([1;16],crate::presentation::Info::default());
    let ctx=host.loaded[0].store.data_mut();let uuid=crate::network::login::format_uuid(&[1;16]);
    assert!(!ctx.get_flight(uuid.clone()).unwrap().allowed);
    assert!(ctx.set_flight(uuid.clone(),1,1).is_err());
    ctx.set_flight(uuid.clone(),0,1).unwrap();ctx.set_flight(uuid.clone(),1,1).unwrap();
    ctx.set_flight(uuid.clone(),2,1).unwrap();
    let state=ctx.get_flight(uuid.clone()).unwrap();assert!(state.allowed&&state.flying);assert_eq!(state.fall_damage,1);
    ctx.set_flight(uuid.clone(),0,0).unwrap();assert!(!ctx.get_flight(uuid.clone()).unwrap().flying);
    ctx.set_flight(uuid.clone(),2,-1).unwrap();assert_eq!(ctx.get_flight(uuid.clone()).unwrap().fall_damage,-1);
    assert!(ctx.set_flight(uuid.clone(),3,0).is_err());assert!(ctx.set_flight(uuid,1,-1).is_err());
    host.unload("hello").unwrap();assert!(shared.players.lock().unwrap().ui.info[&[1;16]].plugin_state.flight_owners.iter().all(Option::is_none));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn plugin_world_rays_are_bounded_and_monster_limits_release_on_unload() {
    use super::api::mcsa::api::world::Host as _;
    let root=server_root("rays-and-limits");assert!(install(&root,"plugins","hello","hello",""));
    let shared=shared();let mut host=Host::new(Arc::clone(&shared),root.clone()).unwrap();host.load("hello").unwrap();
    {let mut world=shared.world_in(crate::dimension::Dimension::Overworld).lock().unwrap();world.set_block(0,70,4,crate::blocks::state_by_name("stone").unwrap());}
    let ctx=host.loaded[0].store.data_mut();let start=wit::Location {dimension:wit::Dimension::Overworld,x:0.5,y:70.5,z:0.5,yaw:None,pitch:None};
    let hit=ctx.ray_blocks(start,0.0,0.0,4.0,8.0,0,true).unwrap().unwrap();assert_eq!((hit.block.x,hit.block.y,hit.block.z,hit.face),(0,70,4,2));assert_eq!(hit.z,4.0);
    assert!(ctx.ray_blocks(start,0.0,0.0,1.0,3.0,0,true).unwrap().is_none());
    assert!(ctx.ray_blocks(start,0.0,0.0,0.0,8.0,0,true).is_err());assert!(ctx.ray_blocks(start,0.0,0.0,1.0,513.0,0,true).is_err());
    assert!(ctx.ray_blocks(start,f64::NAN,0.0,1.0,8.0,0,true).is_err());
    let original=ctx.monster_limit(wit::Dimension::Overworld);ctx.set_monster_limit(wit::Dimension::Overworld,0).unwrap();assert_eq!(ctx.monster_limit(wit::Dimension::Overworld),0);assert_eq!(ctx.monster_limit(wit::Dimension::Nether),original);
    assert!(ctx.set_monster_limit(wit::Dimension::Overworld,10_001).is_err());
    host.unload("hello").unwrap();assert!(shared.world_in(crate::dimension::Dimension::Overworld).lock().unwrap().plugin_monster_limits.is_empty());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn entity_snapshots_launch_potions_and_horse_mutations_reach_native_state() {
    use super::api::mcsa::api::{entities::Host as _, player::Host as _, inventory::Item};
    use crate::dimension::Dimension;
    let root=server_root("native-entity-state");assert!(install(&root,"plugins","hello","hello",""));
    let shared=shared();let mut host=Host::new(Arc::clone(&shared),root.clone()).unwrap();host.load("hello").unwrap();
    shared.players.lock().unwrap().ui.info.insert([1;16],crate::presentation::Info {plugin_state:crate::presentation::PlayerState {eye_height:1.62,..Default::default()},..Default::default()});
    let horse=crate::mob::Mob::new(501,crate::mob::Kind::SkeletonHorse,(2.0,70.0,3.0));let horse_uuid=crate::network::login::format_uuid(&horse.uuid);
    shared.items_in(Dimension::Overworld).lock().unwrap().add_mob(horse);
    let ctx=host.loaded[0].store.data_mut();let player=crate::network::login::format_uuid(&[1;16]);
    let snapshot=ctx.describe(horse_uuid.clone()).unwrap();assert_eq!(snapshot.kind,"skeleton_horse");assert!(snapshot.living);assert!(snapshot.width>0.0);assert_eq!(snapshot.at.x,2.0);
    super::api::mcsa::api::entities::Host::set_health(ctx,horse_uuid.clone(),5.0).unwrap();assert_eq!(ctx.describe(horse_uuid.clone()).unwrap().health,5.0);assert!(super::api::mcsa::api::entities::Host::set_health(ctx,horse_uuid.clone(),f64::NAN).is_err());
    let effect=super::api::mcsa::api::player::PotionEffect {kind:"speed".into(),duration:200,amplifier:1,ambient:false,particles:true,icon:true};ctx.add_potion_effect(horse_uuid.clone(),effect).unwrap();assert_eq!(ctx.effects(horse_uuid.clone()).unwrap()[0].duration,200);ctx.remove_potion_effect(horse_uuid.clone(),"speed".into()).unwrap();assert!(ctx.effects(horse_uuid.clone()).unwrap().is_empty());
    ctx.tame_horse(horse_uuid.clone(),true).unwrap();ctx.set_horse_owner(horse_uuid.clone(),Some(player.clone())).unwrap();assert!(ctx.horse(horse_uuid.clone()).unwrap().tamed);
    let saddle=Item {kind:"saddle".into(),count:1,metadata:String::new()};ctx.set_horse_item(horse_uuid.clone(),0,Some(saddle)).unwrap();assert_eq!(ctx.horse(horse_uuid.clone()).unwrap().items[0].as_ref().unwrap().kind,"saddle");
    assert!(ctx.set_horse_item(horse_uuid.clone(),0,Some(Item {kind:"diamond".into(),count:1,metadata:String::new()})).is_err());ctx.set_horse_item(horse_uuid.clone(),0,None).unwrap();assert!(ctx.horse(horse_uuid.clone()).unwrap().items[0].is_none());
    let velocity=||super::api::mcsa::api::entities::Vector {x:0.0,y:0.2,z:1.0};
    let potion=ctx.launch(player.clone(),"splash_potion".into(),velocity()).unwrap();assert_eq!(ctx.describe(potion.clone()).unwrap().shooter,player);ctx.set_potion_item(potion.clone(),Item {kind:"splash_potion".into(),count:1,metadata:r#"{"potion":"minecraft:healing"}"#.into()}).unwrap();assert!(ctx.effects(potion).unwrap().iter().any(|effect|effect.kind=="instant_health"));
    let ball=ctx.launch(player.clone(),"fireball".into(),velocity()).unwrap();ctx.set_explosive(ball.clone(),Some(0.0),Some(false)).unwrap();assert!(ctx.set_explosive(ball.clone(),Some(f32::NAN),None).is_err());
    assert!(ctx.launch(player,"tnt".into(),velocity()).is_err());
    {let thrown=shared.thrown.lock().unwrap();let shot=thrown.flying.iter().find(|shot|crate::network::login::format_uuid(&shot.uuid)==ball).unwrap();assert!(shot.in_owner);assert_eq!(shot.plugin_yield,Some(0.0));assert_eq!(shot.plugin_incendiary,Some(false));assert_eq!(shot.vz,1.0);}
    shared.world_in(Dimension::Overworld).lock().unwrap().set_block(0,63,0,crate::blocks::state_by_name("stone").unwrap());
    let at=wit::Location {dimension:wit::Dimension::Overworld,x:0.5,y:70.0,z:0.5,yaw:Some(45.0),pitch:None};
    let vex=ctx.create_mob(at,"vex".into()).unwrap();assert_eq!(ctx.describe(vex.clone()).unwrap().at.yaw,Some(45.0));assert!(!ctx.in_world(wit::Dimension::Overworld).unwrap().iter().any(|s|s.uuid==vex));
    ctx.mob_control(vex.clone(),0,0).unwrap();ctx.mob_control(vex.clone(),1,1).unwrap();ctx.mob_control(vex.clone(),2,40).unwrap();ctx.set_target(vex.clone(),Some(crate::network::login::format_uuid(&[1;16]))).unwrap();
    ctx.finish_mob(vex.clone(),true).unwrap();assert!(ctx.valid(vex.clone()));assert_eq!(ctx.get_target(vex.clone()).unwrap(),Some(crate::network::login::format_uuid(&[1;16])));
    let aborted=ctx.create_mob(at,"vex".into()).unwrap();ctx.finish_mob(aborted.clone(),false).unwrap();assert!(!ctx.valid(aborted));
    let items=shared.items_in(Dimension::Overworld).lock().unwrap();let vex=items.mobs().iter().find(|m|crate::network::login::format_uuid(&m.uuid)==vex).unwrap();assert!(!vex.persistent);assert_eq!(vex.illager.life,40);assert_eq!(vex.illager.plugin_limited_life,Some(true));drop(items);
    let mut cloud=crate::thrown::Cloud::lingering(610,Dimension::Overworld,(1.0,70.0,2.0),Vec::new(),1,false);cloud.base_potion=crate::survival::potion_number("long_poison");let cloud_uuid=crate::network::login::format_uuid(&cloud.uuid);shared.thrown.lock().unwrap().clouds.push(cloud);
    assert!(ctx.valid(cloud_uuid.clone()));assert_eq!(ctx.describe(cloud_uuid.clone()).unwrap().kind,"area_effect_cloud");assert_eq!(ctx.cloud_potion(cloud_uuid.clone()).unwrap(),Some("long_poison".into()));assert!(ctx.effects(cloud_uuid.clone()).unwrap().is_empty());ctx.remove(cloud_uuid.clone()).unwrap();assert!(!ctx.valid(cloud_uuid));
    let frame=crate::frame::Frame::new(611,(0,70,0),crate::frame::NORTH,false);let frame_uuid=crate::network::login::format_uuid(&frame.uuid);shared.items_in(Dimension::Overworld).lock().unwrap().add_frame(frame);assert!(ctx.valid(frame_uuid.clone()));assert_eq!(ctx.describe(frame_uuid.clone()).unwrap().kind,"item_frame");ctx.remove(frame_uuid.clone()).unwrap();assert!(!ctx.valid(frame_uuid));
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn bukkit_ready_players_hide_incomplete_login_without_changing_generic_online_list() {
    use super::api::mcsa::api::server::Host as _;
    let root=server_root("ready-players");assert!(install(&root,"plugins","hello","hello",""));
    let shared=shared();let mut host=Host::new(Arc::clone(&shared),root).unwrap();host.load("hello").unwrap();let ctx=host.loaded[0].store.data_mut();
    assert_eq!(ctx.online_players().len(),1);assert!(ctx.ready_players().is_empty());
    shared.players.lock().unwrap().ui.info.insert([1;16],crate::presentation::Info::default());
    assert_eq!(ctx.ready_players().len(),1);assert_eq!(ctx.online_players().len(),1);
    shared.players.lock().unwrap().ui.info.remove(&[1;16]);assert!(ctx.ready_players().is_empty());
}
