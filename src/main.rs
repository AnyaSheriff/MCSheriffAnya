// Точка входа сервера.
//
// Шаг 3: сервер теперь умеет:
// - загружать (и при необходимости создавать) config/server.properties;
// - разбирать пакет Handshake;
// - при next_state=1 (Status) отвечать на Status Request корректным
//   JSON-описанием сервера (Server List Ping) и отвечать Pong на Ping;
// - при next_state=2 (Login) разбирать Login Start, отправлять Login Success
//   и проводить игрока через Configuration в Play.
//
// Мир общий для всех подключений: блоки, которые поставил или сломал один
// игрок, видны всем. Пока в мире нет генерации — только один блок камня
// в точке появления и всё, что построили игроки.

mod network;
mod colors;
mod bedrock;
mod config;
mod world;
mod blocks;
mod blocks_table;
mod gameplay_catalog;
mod hardness_table;
mod mining;
mod placing;
mod pottery;
mod portal;
mod plants;
mod farming;
mod frost;
mod soul;
mod block_behaviour;
mod vegetation;
mod copper;
mod compost;
mod scaffolding;
mod dripstone;
mod sulfur_spike;
mod poplar;
mod entity;
mod player;
mod fluids;
mod inventory;
mod journal;
mod items;
mod minecart;
mod riding;
mod frame;
mod painting;
mod mob;
mod arrow;
mod thrown;
mod wind;
mod trial;
mod archaeology;
mod ominous;
mod firework;
mod ender_eye;
mod loot;
mod playerdata;
mod redstone;
mod tnt;
mod survival;
mod effect_reactions;
mod teleport_item;
mod attributes;
mod snbt;
mod stackdata;
mod combat_table;
mod combat;
mod xp;
mod lightning;
mod weather;
mod bed;
mod custom_tnt;
mod permissions;
mod containers;
mod furnace;
mod campfire;
mod beacon;
mod fire;
mod fishing;
mod maps;
mod crafting;
mod window;
mod villager;
mod raid;
mod sign;
mod spawner;
mod hive;
mod enchanting;
mod smithing;
mod loom;
mod bundle;
mod book;
mod stonecutter;
mod skins;
mod tick;
mod chat;
mod ops;
mod players;
mod presentation;
mod shared;
mod sessions;
mod log;
mod commands;
mod commands_feedback;
mod command_block;
mod commands_world;
mod commands_score;
mod scoreboard;
mod stats;
mod advancements;
mod progress;
mod recipe_book;
mod commands_progress;
mod functions;
mod commands_admin;
mod dimension;
mod dragon_fight;
mod reports;
mod plugins;
mod auth;
mod console;

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;

use config::server_properties::{load_server_properties, ServerProperties, WorldKind};
use dimension::Dimension;
use network::configuration::{
    read_and_log_configuration_packets, read_known_packs, read_login_acknowledged,
    send_brand, send_finish_configuration, send_known_packs, send_registry_data, send_update_tags,
};
use network::handshake::read_handshake;
use network::{PROTOCOL_VERSION, VERSION_NAME};
use network::login::{
    format_uuid, offline_uuid, read_login_start, send_login_disconnect, send_login_disconnect_json, send_login_success,
    valid_name,
};
use network::play::play_session;
use network::status::handle_status;
use shared::Shared;
use world::World;

/// Путь к файлу настроек сервера относительно рабочей директории процесса.
const SERVER_PROPERTIES_PATH: &str = "config/server.properties";

/// Путь к файлу мира. Мир лежит рядом с настройками, в директории world.
const WORLD_PATH: &str = "world";

fn main() -> io::Result<()> {
    #[cfg(target_os = "linux")]
    {
        let args: Vec<_> = std::env::args_os().collect();
        if args.get(1).is_some_and(|v| v == "--upgrade-world-26.3") {
            if args.len()!=3 { return Err(io::Error::new(io::ErrorKind::InvalidInput,"usage: --upgrade-world-26.3 WORLD")); }
            return world::migration::upgrade(Path::new(&args[2]));
        }
    }
    use std::io::IsTerminal;
    if !config::first_start::prepare(Path::new("."),std::io::stdin().is_terminal(),&mut std::io::stdin().lock(),&mut std::io::stdout().lock())? { return Ok(()); }
    // Неподдержанная игровая версия не должна запускать даже перенос хранения.
    // Передаём ровно эти проверенные настройки в runtime, не читая их повторно.
    let first_start = !Path::new(config::mcsheriffanya::FILE).exists();
    let settings = config::mcsheriffanya::Settings::load(Path::new(config::mcsheriffanya::FILE));
    settings.validate()?;
    // Проверяем мир до создания каталогов, слушателей и фоновых писателей.
    // Замок живёт дольше runtime; session.lock измерений остаются отдельными.
    let world_guard = world::startup::prepare_for_version(Path::new(WORLD_PATH), settings.java_version.expect("проверенная версия"))?;
    log_debug!("Проверка заявленного профиля мира: {:?}", world_guard.check);
    if !world_guard.has_exclusive_lock() {
        log_warn!("На этой платформе доступна только читающая проверка профиля мира; общий межпроцессный замок пока не реализован");
    }
    // Такту отводится отдельное ядро, всё остальное делит оставшиеся: сеть,
    // складывание чанков, скины. Если ядро всего одно, делить нечего —
    // работаем как придётся.
    let cores = tick::cores();
    let others = cores.saturating_sub(1).max(1);

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(others)
        // Один поток сверху: чтение консоли (tokio::io::stdin) держит
        // блокирующий поток всё время работы. Без запаса на одно- и
        // двухъядерной машине он занимал весь пул, и сборка чанков
        // (spawn_blocking) не начиналась никогда: игрок входил без чанков
        // и отваливался по тайм-ауту.
        .max_blocking_threads(others + 1)
        .enable_all()
        .on_thread_start(move || {
            // Рабочие потоки не залезают на ядро такта.
            tick::keep_off_tick_core();
        })
        .build()?;

    runtime.block_on(server(settings, first_start))
}

async fn server(settings: config::mcsheriffanya::Settings, first_start: bool) -> io::Result<()> {
    let started = Instant::now();

    // Первые строки — как у обычного сервера: что за сервер, что грузим.
    log_info!("Запуск сервера MCSheriffAnya");

    match tick::tick_core() {
        Some(core) => log_info!(
            "Ядер: {}. Такту отведено ядро {}, остальные — сети и складыванию мира",
            tick::cores(),
            core
        ),
        None => log_info!("Ядро всего одно: такт и всё прочее делят его"),
    }

    log_info!("Загрузка настроек");

    // Создаём необходимые рабочие директории, если их ещё нет.
    ensure_directories(&["config", "world", "logs", playerdata::DIRECTORY, skins::DIRECTORY])?;

    // Свои настройки — отдельно от server.properties: тот должен выглядеть
    // как у обычного сервера.
    let primary = settings.java_version.expect("проверенная версия");
    log_info!("Основная версия игры: Minecraft Java {} (протокол {})", primary.name(), primary.protocol());
    if first_start {
        log_info!("Первый запуск: основная версия задаётся строкой java-version={} в config/mcsa.properties", VERSION_NAME);
        log_info!("Чтобы выбрать более старую версию, остановите сервер, сохраните копию мира и укажите поддерживаемую версию в java-version. В этой сборке доступны 26.1.2 и новый мир26.3; прежняя внутренняя версия {}", VERSION_NAME);
        log_info!("При online-mode=false: offline-license-mode=cosmetics — лицензии для косметики; disabled — без проверки лицензий. Пароли настраиваются отдельно через auth-enabled");
    }
    log::set_debug(settings.debug);
    log::set_console_colors(settings.console_colors);
    world::terrain::AUTUMN_FORESTS.store(settings.autumn_forests, std::sync::atomic::Ordering::Relaxed);
    world::terrain::PINK_CHERRY_GROVES.store(settings.pink_cherry_groves, std::sync::atomic::Ordering::Relaxed);

    if settings.debug {
        log_info!("Подробный лог включён (config/mcsa.properties)");
    }

    // Отчёты об ошибках — только если владелец их включил.
    reports::start(settings.send_error_reports);

    // Образец таблицы «кому чей скин» — чтобы было видно, что писать.
    skins::prepare(Path::new(skins::DIRECTORY));

    // Загружаем настройки сервера (файл создаётся с дефолтами, если его нет).
    let properties = load_server_properties(SERVER_PROPERTIES_PATH)?;
    log_debug!(
        "Конфигурация загружена: motd=\"{}\", max-players={}, server-port={}, \
         view-distance={}, simulation-distance={}",
        properties.motd,
        properties.max_players,
        properties.server_port,
        properties.view_distance,
        properties.simulation_distance
    );

    network::packet::set_compression_threshold(properties.compression_threshold);
    if settings.verify_license(properties.online_mode) {
        network::online::initialize().await?;
    }

    let bind_address = format!("0.0.0.0:{}", properties.server_port);
    let listener = TcpListener::bind(&bind_address).await?;
    log_info!("Сервер запущен на *:{}", properties.server_port);
    log_debug!("Слушаем {}", bind_address);

    // Оборачиваем настройки в Arc, чтобы безопасно делиться ими между
    // задачами tokio::spawn, обрабатывающими разные подключения.
    let properties = Arc::new(properties);

    // Всё, что подключения делят между собой: мир, список игроков и чат.
    // Обычный мир и Незер: у Незера своя папка внутри папки мира, как у
    // оригинала, и то же семя.
    let catalog = gameplay_catalog::Catalog::for_version(primary);
    let open_world = |path: &Path, seed, kind| {
        if primary == network::NATIVE_VERSION { World::open(path, seed, kind) }
        else { World::open_with_catalog(path, seed, kind, catalog) }
    };
    let overworld = open_world(Path::new(WORLD_PATH), properties.level_seed, properties.world_kind)?;
    let nether = open_world(
        &Dimension::Nether.folder(Path::new(WORLD_PATH)),
        Some(overworld.seed()),
        WorldKind::Nether,
    )?;
    let end = open_world(
        &Dimension::End.folder(Path::new(WORLD_PATH)),
        Some(overworld.seed()),
        WorldKind::End,
    )?;

    let mut skin_settings = skins::Settings::load(Path::new(skins::SETTINGS_FILE));
    settings.apply_skin_policy(properties.online_mode, &mut skin_settings);
    let mut shared = Shared::with_end(
        overworld,
        nether,
        end,
        PathBuf::from(playerdata::DIRECTORY),
        PathBuf::from(skins::DIRECTORY),
        (*properties).clone(),
        ops::Ops::open(std::path::Path::new(ops::FILE)),
        skin_settings,
    );
    *shared.maps.lock().expect("карты") = maps::Maps::open(Path::new(WORLD_PATH))?;
    shared.settings = settings;
    if settings.auth_enabled && !properties.online_mode {
        shared.auth = Some(Arc::new(auth::Service::open(Path::new("auth.db"))?));
    }
    shared.scoreboard = std::sync::Mutex::new(scoreboard::Scoreboard::open(Path::new(WORLD_PATH)));
    shared.functions = std::sync::Mutex::new(functions::Functions::new(Path::new(WORLD_PATH)));
    let shared = Arc::new(shared);

    // Табло пишется на диск своим потоком, раз в пять секунд, если менялось.
    {
        let shared = Arc::clone(&shared);
        std::thread::spawn(move || loop {
            std::thread::sleep(std::time::Duration::from_secs(5));
            save_scoreboard(&shared);
        });
    }

    // Функции метки `#minecraft:load` — при запуске.
    functions::load(&shared);

    // Плагины и модули (plugins/, modules/): грузятся до первого игрока.
    plugins::start(Arc::clone(&shared));

    // Остановка сервера: консоль сообщает о ней, а ждёт её вот этот приёмник.
    let (shutdown_sender, mut shutdown) = watch::channel(false);

    console::start(Arc::clone(&shared), shutdown_sender);

    // Такт мира: без него жидкости стояли бы на месте.
    tick::start(Arc::clone(&shared));

    // Игроки Bedrock — по UDP, на своём порту.
    bedrock::start(Arc::clone(&shared));

    log_info!(
        "Готово ({:.3} с)! Чтобы узнать команды, набери help",
        started.elapsed().as_secs_f32()
    );

    loop {
        tokio::select! {
            accepted = listener.accept() => {
                // Ошибка приёма (например, кончились дескрипторы файлов) —
                // не повод гасить сервер без сохранения: пишем и ждём.
                let (socket, addr) = match accepted {
                    Ok(accepted) => accepted,
                    Err(error) => {
                        log_error!("Не удалось принять подключение: {}", error);
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                        continue;
                    }
                };
                log_debug!("Новое подключение: {}", addr);

                // Без этого маленькие пакеты копятся в ядре по 40 мс
                // (алгоритм Нейгла с отложенным подтверждением), и звук
                // приходит на такт позже действия, а ход поршня дёргается.
                if let Err(error) = socket.set_nodelay(true) {
                    log_debug!("Не удалось отключить склейку пакетов: {}", error);
                }

                let properties = Arc::clone(&properties);
                let shared = Arc::clone(&shared);

                tokio::spawn(async move {
                    if let Err(e) = handle_connection(socket, addr, properties, shared).await {
                        // ConnectionReset / BrokenPipe / UnexpectedEof — это нормальное
                        // завершение сессии: клиент закрыл соединение со своей стороны
                        // (например, не отправив Ping после Status Response).
                        // Такие ошибки не являются сбоем сервера и не логируются.
                        let client_closed = matches!(
                            e.kind(),
                            io::ErrorKind::ConnectionReset
                                | io::ErrorKind::BrokenPipe
                                | io::ErrorKind::UnexpectedEof
                        );

                        if !client_closed {
                            log_error!("Ошибка при обработке подключения {}: {}", addr, e);
                        }
                    }
                });
            }
            _ = shutdown.changed() => {
                break;
            }
            // Остановка снаружи — сигналом. Без этого всё, что изменилось
            // за последнюю секунду, пропадало бы: мир пишется по часам.
            _ = stop_signal() => {
                log_info!("Остановка сервера по сигналу");
                break;
            }
        }
    }

    // Последовательность как у обычного сервера: остановка, игроки, мир.
    // Игроки пишутся каждый своим подключением при выходе — поэтому всех
    // выгоняем и ждём, пока подключения их запишут и уйдут из списка.
    log_info!("Остановка сервера");
    log_info!("Сохранение игроков");
    kick_everyone(&shared).await;
    // Плагины выгружаются до записи мира: им есть что дописать.
    plugins::shutdown();
    log_info!("Сохранение мира");
    for dimension in Dimension::ALL {
        let mut world = shared.world_in(dimension).lock().expect("мир захвачен другим потоком");

        // Мобы ложатся в файл вместе со своими чанками.
        tick::store_mobs(&shared, dimension, &mut world);
        world.save_all();
    }
    save_scoreboard(&shared);
    log_info!("Мир сохранён");

    // Терминал был переключён на посимвольный ввод — возвращаем как было,
    // иначе в нём потом не видно, что набираешь.
    console::give_terminal_back();

    // Выходим сразу, не дожидаясь задач.
    //
    // Одна из них — чтение клавиатуры, и она ждёт нажатия: пока его нет,
    // выход не заканчивается, и после `stop` терминал так и висел бы до
    // первой нажатой клавиши. Всё, что нужно было сохранить, к этому времени
    // уже на диске: игроки записаны при выходе, мир — только что.
    std::process::exit(0);
}

/// Раньше опознавателем игрока был тот, что присылает клиент, и под ним
/// лежат его файл и права. Теперь опознаватель считается из ника — старое
/// переносим на новый, чтобы игрок не потерял вещи и права. Старый файл не
/// трогаем: копия, а не перенос.
fn adopt_old_identity(shared: &Shared, name: &str, sent: &[u8; 16], uuid: &[u8; 16]) {
    if sent == uuid {
        return;
    }

    let directory = Path::new(playerdata::DIRECTORY);
    let old = playerdata::path_for(directory, sent);
    let new = playerdata::path_for(directory, uuid);

    if !new.exists() && old.exists() {
        match std::fs::copy(&old, &new) {
            Ok(_) => log_info!(
                "Файл игрока {} перенесён на опознаватель {} (был {})",
                name,
                format_uuid(uuid),
                format_uuid(sent)
            ),
            Err(error) => log_warn!("Не удалось перенести файл игрока {}: {}", name, error),
        }
    }

    shared
        .ops
        .lock()
        .expect("права захвачены другим потоком")
        .adopt(uuid, name);
}

/// Выгоняет всех с сообщением об остановке и ждёт, пока они уйдут (не
/// дольше трёх секунд: зависшее подключение не должно держать остановку).
async fn kick_everyone(shared: &Shared) {
    {
        let mut players = shared
            .players
            .lock()
            .expect("список игроков захвачен другим потоком");
        let everyone: Vec<[u8; 16]> = players.members().iter().map(|member| member.uuid).collect();
        for uuid in everyone {
            players.order(
                &uuid,
                players::Order::Kick {
                    reason: "Сервер остановлен".to_string(),
                },
            );
        }
    }

    let deadline = Instant::now() + std::time::Duration::from_secs(3);
    while Instant::now() < deadline {
        let online = shared
            .players
            .lock()
            .expect("список игроков захвачен другим потоком")
            .online();
        if online == 0 {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    log_warn!("Не все игроки успели выйти до остановки");
}

/// Ждёт сигнала остановки от системы: Ctrl-C (на Windows только он).
#[cfg(not(unix))]
async fn stop_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

/// Ждёт сигнала остановки от системы: Ctrl-C или просьбы завершиться.
#[cfg(unix)]
async fn stop_signal() {
    use tokio::signal::unix::{SignalKind, signal};

    let mut terminate = match signal(SignalKind::terminate()) {
        Ok(terminate) => terminate,
        Err(_) => {
            // Подписаться не вышло — ждём вечно, остановят другим путём.
            std::future::pending::<()>().await;
            return;
        }
    };

    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = terminate.recv() => {}
    }
}

/// Создаёт список директорий относительно рабочей директории процесса,
/// если они ещё не существуют.
fn ensure_directories(dirs: &[&str]) -> io::Result<()> {
    for dir in dirs {
        let path = Path::new(dir);
        if !path.exists() {
            std::fs::create_dir_all(path)?;
            log_info!("Создана директория: {}", dir);
        }
    }
    Ok(())
}

/// Обрабатывает одно TCP-подключение.
///
/// Сначала читает пакет Handshake, затем в зависимости от поля
/// next_state переходит в соответствующее состояние протокола.
async fn handle_connection(
    socket: TcpStream,
    addr: std::net::SocketAddr,
    properties: Arc<ServerProperties>,
    shared: Arc<Shared>,
) -> io::Result<()> {
    handle_connection_inner(socket, addr, properties, shared, false).await
}

async fn handle_connection_inner(
    socket: TcpStream, addr: std::net::SocketAddr, properties: Arc<ServerProperties>,
    shared: Arc<Shared>, _probe_776: bool,
) -> io::Result<()> {
    // Подключился не клиент Minecraft (telnet, браузер) — справка текстом.
    if shared.settings.text_info {
        match network::text_info::classify(&socket).await {
            network::text_info::Visitor::Game => {}
            visitor => {
                log_debug!("{} подключился не игрой — отвечаю справкой", addr);
                return network::text_info::answer(socket, visitor, &properties, &shared).await;
            }
        }
    }

    let mut socket = network::connection::Connection::new(socket);
    let handshake = read_handshake(&mut socket).await?;

    log_debug!(
        "Handshake: protocol_version={}, address={}, port={}, next_state={}",
        handshake.protocol_version,
        handshake.server_address,
        handshake.server_port,
        handshake.next_state
    );

    match handshake.next_state {
        // Состояние Status: клиент запрашивает информацию о сервере
        // (отображается в списке серверов клиента Minecraft).
        1 => {
            // Сколько игроков показать в списке серверов — берём из общего
            // списка, а не из выдуманного числа.
            let online = shared
                .players
                .lock()
                .expect("список игроков захвачен другим потоком")
                .online();

            handle_status(
                &mut socket,
                &handshake,
                &properties.motd,
                online as i32,
                properties.max_players,
                shared.settings.java_version.unwrap_or(network::NATIVE_VERSION),
            )
            .await?;
        }
        // Состояние Login: игрок входит на сервер.
        2 => {
            // Версию проверяем до всего остального. Клиент другой версии
            // шлёт дальше пакеты, которых мы не знаем, и разбор упирался бы
            // в невнятную «ошибку сетевого протокола» — вместо этого сразу
            // говорим человеку, в чём дело.
            //
            // Клиентская и внутренняя версии разделены; допуск задаётся
            // отдельно для проверенных сочетаний клиента и основного.
            // 770/771/772/773/774/776 прошли прежний checklist на обоих основных: CONTINUED.md.
            // Это клиентский codec canonical775, а не отдельные игровые правила.
            let negotiated = if matches!(handshake.protocol_version,770|771|772|773|774|776) && matches!(shared.catalog().profile(),
                gameplay_catalog::Profile::Java26_1_2 | gameplay_catalog::Profile::Preview26_3) {
                Ok(mcsa_protocol::Negotiated {client:mcsa_protocol::JavaVersion::from_protocol(handshake.protocol_version).expect("770/771/772/773/774/776"),native:network::NATIVE_VERSION})
            } else if shared.catalog().profile() == gameplay_catalog::Profile::Preview26_3 && handshake.protocol_version == 775 {
                mcsa_protocol::Negotiated::native_only(775, network::NATIVE_VERSION)
            } else if shared.catalog().profile() == gameplay_catalog::Profile::Preview26_3 {
                mcsa_protocol::Negotiated::native_only(handshake.protocol_version, mcsa_protocol::JavaVersion::V26_3)
                    .map(|mut route| { route.native = network::NATIVE_VERSION; route })
            } else { mcsa_protocol::Negotiated::native_only(handshake.protocol_version, network::NATIVE_VERSION) };
            // Только отдельный тестовый бинарник может исследовать ещё закрытый маршрут.
            #[cfg(test)]
            let negotiated = if (shared.settings.java_version==Some(mcsa_protocol::JavaVersion::V26_3)
                || (_probe_776 && addr.ip().is_loopback() && !properties.online_mode && !shared.settings.auth_enabled && shared.auth.is_none()))
                && handshake.protocol_version==777 {
                Ok(mcsa_protocol::Negotiated {client:mcsa_protocol::JavaVersion::V26_3,native:network::NATIVE_VERSION})
            } else if shared.settings.java_version==Some(mcsa_protocol::JavaVersion::V1_12_2) && handshake.protocol_version==340 {
                Ok(mcsa_protocol::Negotiated {client:mcsa_protocol::JavaVersion::V1_12_2,native:network::NATIVE_VERSION})
            } else if _probe_776 && matches!(handshake.protocol_version,770|771|772|773|774|776) && addr.ip().is_loopback()
                && !properties.online_mode {
                Ok(mcsa_protocol::Negotiated { client: mcsa_protocol::JavaVersion::from_protocol(handshake.protocol_version).expect("770/771/772/773/774/776"), native: network::NATIVE_VERSION })
            } else {negotiated};
            if negotiated.is_err() {
                log_info!(
                    "Вход отклонён: версия клиента (протокол {}) не подходит, ядру нужен протокол {}",
                    handshake.protocol_version,
                    PROTOCOL_VERSION
                );

                send_login_disconnect(
                    &mut socket,
                    &format!(
                        "Ваша версия не подходит под ядро сервера\nНужен Minecraft {} (протокол {})",
                        shared.settings.java_version.unwrap_or(network::NATIVE_VERSION).name(),
                        shared.settings.java_version.unwrap_or(network::NATIVE_VERSION).protocol()
                    ),
                )
                .await?;

                return Ok(());
            }

            socket.protocol = negotiated.ok();
            #[cfg(test)]
            { socket.probe_776 = matches!(socket.client_version(),mcsa_protocol::JavaVersion::V1_21_5 | mcsa_protocol::JavaVersion::V1_21_6 | mcsa_protocol::JavaVersion::V1_21_8 | mcsa_protocol::JavaVersion::V1_21_9|mcsa_protocol::JavaVersion::V1_21_11|mcsa_protocol::JavaVersion::V26_2); }
            let legacy = socket.client_version() == mcsa_protocol::JavaVersion::V1_12_2;
            if legacy {
                // В340 нет Configuration-диалога. До реализации старого
                // интерфейса нельзя обходить парольный барьер.
                if shared.settings.auth_enabled || shared.auth.is_some() {
                    send_login_disconnect(&mut socket, "Вход с 1.12.2 при включённой защите паролем ещё не поддерживается").await?;
                    return Ok(());
                }
                let state = mcsa_protocol::legacy_play::Session::new(
                    mcsa_protocol::legacy_chunk::HeightWindow::new(-64).map_err(|error| io::Error::other(format!("Окно340: {error:?}")))?,
                    properties.difficulty,
                ).map_err(|error| io::Error::other(format!("Состояние340: {error:?}")))?;
                socket.legacy = Some(Arc::new(std::sync::Mutex::new(state)));
            }
            let mut login = read_login_start(&mut socket).await?;

            // Ник — как у оригинала. Иначе длинный ник отключил бы всех
            // остальных, а «../» попал бы в пути файлов.
            if !valid_name(&login.username) {
                log_info!("Вход отклонён: недопустимый ник {:?}", login.username);
                send_login_disconnect(&mut socket, "Недопустимый ник").await?;
                return Ok(());
            }

            let password_auth = shared.auth.as_ref();
            if password_auth.is_some_and(|auth| auth.blocked(addr.ip())) {
                send_login_disconnect(&mut socket, auth::BLOCKED).await?;
                return Ok(());
            }
            let try_premium = !properties.online_mode
                && shared.settings.verify_license(false)
                && shared.premium.should_verify(&login.username, addr.ip()).await;
            let authenticated = if properties.online_mode || try_premium {
                let auth = network::online::Authenticator::get()?;
                let ip = properties.prevent_proxy_connections.then_some(addr.ip());
                match auth.authenticate(&mut socket, &login.username, ip).await {
                    Ok(profile) => {
                        login.uuid = profile.uuid;
                        login.username = profile.name.clone();
                        Some(profile)
                    }
                    Err(error) => {
                        if try_premium {
                            shared.premium.failed(&login.username, addr.ip(), auth::now());
                            let _ = send_login_disconnect(&mut socket, auth::premium::RETRY).await;
                            return Ok(());
                        }
                        if let network::online::Failure::Protocol(ref cause) = error {
                            log_debug!("Ошибка проверки входа {}: {}", addr, cause);
                        }
                        let json = serde_json::json!({"translate": error.translation()}).to_string();
                        send_login_disconnect_json(&mut socket, &json).await?;
                        return Ok(());
                    }
                }
            } else {
                // Старый перенос личности остаётся только в offline-mode:
                // в online-mode права определяются проверенным UUID.
                let sent = login.uuid;
                login.uuid = offline_uuid(&login.username);
                if password_auth.is_none() {
                    adopt_old_identity(&shared, &login.username, &sent, &login.uuid);
                }
                None
            };

            let verified_uuid = authenticated.as_ref().map(|p| p.uuid);
            let auth_identity = password_auth.map(|auth| auth.identity(&login.username, verified_uuid));
            if let Some(identity) = &auth_identity {
                login.uuid = identity.uuid;
                login.username = identity.name.clone();
            }

            // Как у обычного сервера: опознаватель игрока и откуда он пришёл —
            // с какого адреса и на какой адрес сервера стучался. Адрес сервера
            // полезен, когда у него несколько имён или он за туннелем.
            reports::known_player(&login.username);
            log_info!("Опознаватель игрока {} — {}", login.username, format_uuid(&login.uuid));

            // Блокировки по нику и адресу и белый список — раньше проверки на
            // «сервер полон», как у оригинала (commands_admin.rs).
            let is_op = password_auth.is_none() && shared.ops.lock().expect("права захвачены другим потоком").level_of(&login.uuid) > 0;

            if password_auth.is_none() && let Some((json, plain)) = commands_admin::login_refusal(&login.username, &login.uuid, addr.ip(), is_op) {
                log_info!("{} не пущен: {}", login.username, plain.replace('\n', " "));
                send_login_disconnect_json(&mut socket, &json).await?;
                return Ok(());
            }

            let online = shared
                .players
                .lock()
                .expect("список игроков захвачен другим потоком")
                .online();
            if online >= properties.max_players.max(0) as usize
                && !shared.players.lock().expect("список игроков").name_taken(&login.username) {
                log_info!("{} не пущен: сервер полон", login.username);
                send_login_disconnect(&mut socket, "Сервер полон").await?;
                return Ok(());
            }

            // Откуда пришёл и куда стучался: адрес сервера обычный сервер не
            // печатает, а нам он полезен — у сервера может быть несколько имён.
            let came_from = addr.to_string();
            let came_to = format!("{}:{}", handshake.server_address, handshake.server_port);

            // Сжатие включается до Login Success: он уже идёт в новом виде.
            network::login::send_set_compression(&mut socket, properties.compression_threshold).await?;
            send_login_success(&mut socket, &login.username, &login.uuid, authenticated.as_ref().and_then(|profile| profile.skin.as_ref()), properties.compression_threshold).await?;
            log_debug!("Login Success отправлен игроку {}", login.username);

            if legacy {
                // Старый протокол сразу переходит в Play. Проверки личности,
                // запретов и вместимости выше — те же, что у родного клиента.
                play_session(&mut socket, &login.username, &login.uuid,
                    network::configuration::ClientLook::DEFAULT, &shared,
                    (&came_from, &came_to), authenticated.map(|profile| profile.skin)).await?;
                return Ok(());
            }

            // Один срок на согласование Configuration и ввод пароля.
            let probe_776 = socket.probe_776_enabled();
            let config_deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(90);
            let negotiation = async {
                // Фаза Configuration.
                read_login_acknowledged(&mut socket).await?;
                log_debug!("Login Acknowledged получен — клиент в фазе Configuration");

                // Название сервера — первым, как у оригинала: клиент
                // показывает его на экране F3.
                send_brand(&mut socket).await?;

                // Обмен Known Packs: сервер объявляет набор ресурсов, который
                // считает общим с клиентом. Без этого обмена клиент не сможет
                // взять содержимое записей реестров из собственных файлов.
                send_known_packs(&mut socket).await?;
                log_debug!("Known Packs отправлен (minecraft:core)");

                let (packs, look_from_settings) = read_known_packs(&mut socket).await?;
                for pack in &packs {
                    log_debug!(
                        "Known Packs клиента: {}:{} версия \"{}\"",
                        pack.namespace, pack.id, pack.version
                    );
                }

                // Данные реестров: клиент требует их до тегов и Finish Configuration.
                let entries = send_registry_data(&mut socket).await?;
                log_debug!("Registry Data отправлен ({} записей)", entries);

                // Теги реестров: клиент требует их при обработке Finish Configuration,
                // из known packs они не берутся.
                send_update_tags(&mut socket).await?;
                log_debug!("Update Tags отправлен (теги minecraft:damage_type)");
                Ok::<_, io::Error>(look_from_settings)
            };
            let look_from_settings = if password_auth.is_some() || probe_776 {
                match tokio::time::timeout_at(config_deadline, negotiation).await {
                    Ok(result) => result?,
                    Err(_) => {
                        auth::dialog::disconnect(&mut socket, auth::dialog::TIMEOUT).await?;
                        return Ok(());
                    }
                }
            } else { negotiation.await? };

            // Парольный барьер: до Finish Configuration нет игрока, прав,
            // плагиновых join-событий или чтения playerdata. Старую личность
            // при auth-enabled вообще не переносим, включая Ops::adopt.
            let _auth_session = if !auth::needs_chat_login(socket.client_version())
                && let (Some(auth), Some(identity)) = (password_auth, &auth_identity) {
                if !auth::dialog::authenticate(&mut socket, auth, identity, addr.ip(), &shared.ops, verified_uuid, config_deadline).await? {
                    return Ok(());
                }
                let is_op = shared.ops.lock().expect("ops").level_of(&login.uuid) > 0;
                if let Some((_, plain)) = commands_admin::login_refusal(&login.username, &login.uuid, addr.ip(), is_op) {
                    auth::dialog::disconnect(&mut socket, &plain).await?;
                    return Ok(());
                }
                Some(auth.session(identity))
            } else { None };

            // Завершение фазы. Какие ещё реестры потребует клиент — покажет
            // его лог: он называет недостающий реестр или тег.
            send_finish_configuration(&mut socket).await?;
            log_debug!("Finish Configuration отправлен");

            let mut look = if probe_776 {
                tokio::time::timeout_at(config_deadline, read_and_log_configuration_packets(&mut socket, look_from_settings)).await
                    .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, "776 Configuration ACK не получен"))??
            } else { read_and_log_configuration_packets(&mut socket, look_from_settings).await? };
            log_debug!("Configuration: чтение пакетов клиента завершено");

            // Клиент без dialogs получает только отдельный замороженный мир.
            let _chat_auth_session = if look.is_some() && auth::needs_chat_login(socket.client_version())
                && let (Some(auth), Some(identity)) = (password_auth, &auth_identity) {
                if !network::play::chat_login(&mut socket, &shared, identity, addr.ip(), verified_uuid, config_deadline).await? {
                    return Ok(());
                }
                let is_op = shared.ops.lock().expect("ops").level_of(&login.uuid) > 0;
                if let Some((_, plain)) = commands_admin::login_refusal(&login.username, &login.uuid, addr.ip(), is_op) {
                    auth::dialog::disconnect(&mut socket, &plain).await?;
                    return Ok(());
                }
                let session = auth.session(identity);
                // ACK возврата уже прочитан лимбо; реестры согласуются заново.
                look = tokio::time::timeout_at(config_deadline, async {
                    send_brand(&mut socket).await?;
                    send_known_packs(&mut socket).await?;
                    let (_, settings) = read_known_packs(&mut socket).await?;
                    send_registry_data(&mut socket).await?;
                    send_update_tags(&mut socket).await?;
                    send_finish_configuration(&mut socket).await?;
                    read_and_log_configuration_packets(&mut socket, settings).await
                }).await.map_err(|_| io::Error::new(io::ErrorKind::TimedOut, auth::dialog::TIMEOUT))??;
                Some(session)
            } else { None };

            // Фаза Play: вводим игрока в мир.
            // Если клиент отключился, не подтвердив фазу, входить некуда.
            if let Some(look) = look {
                let entered = play_session(
                    &mut socket,
                    &login.username,
                    &login.uuid,
                    look,
                    &shared,
                    (&came_from, &came_to),
                    authenticated.map(|profile| profile.skin),
                )
                .await;

                entered?;
            }
        }
        // Переход с другого сервера (Transfer): как у оригинала с
        // `accepts-transfers=false` — отказ с объяснением, а не молчаливый
        // обрыв.
        3 => {
            log_info!("Вход отклонён: переход с другого сервера ({})", addr);
            send_login_disconnect(&mut socket, "Этот сервер не принимает переходы с других серверов").await?;
        }
        other => {
            log_info!("Неизвестное значение next_state: {}", other);
        }
    }

    Ok(())
}

/// Записать табло, если менялось: рядом, потом на место.
fn save_scoreboard(shared: &Shared) {
    let Some((file, text)) = shared.scoreboard.lock().expect("табло захвачено другим потоком").take_save() else {
        return;
    };

    if let Some(folder) = file.parent() {
        let _ = std::fs::create_dir_all(folder);
    }

    let fresh = file.with_extension("dat_new");

    if let Err(error) = std::fs::write(&fresh, text).and_then(|()| std::fs::rename(&fresh, &file)) {
        log_warn!("Табло не записано ({}): {}", file.display(), error);
    }
}

#[cfg(test)]
#[path = "network/gameplay_probe.rs"]
mod gameplay_probe;

#[cfg(test)]
#[path = "network/catalog_preview_probe.rs"]
mod catalog_preview_probe;
