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
    shared.players.lock().unwrap().ui.info.insert([1;16], crate::presentation::Info {ping:42,health:17.0,max_health:24.0,food:18,level:3,exp:0.5});
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
