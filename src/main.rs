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
mod bedrock;
mod config;
mod world;
mod blocks;
mod blocks_table;
mod hardness_table;
mod mining;
mod placing;
mod pottery;
mod portal;
mod plants;
mod farming;
mod block_behaviour;
mod vegetation;
mod copper;
mod compost;
mod scaffolding;
mod dripstone;
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
mod shared;
mod log;
mod commands;
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
use network::play::{play_session, player_left};
use network::status::handle_status;
use shared::Shared;
use world::World;

/// Путь к файлу настроек сервера относительно рабочей директории процесса.
const SERVER_PROPERTIES_PATH: &str = "config/server.properties";

/// Путь к файлу мира. Мир лежит рядом с настройками, в директории world.
const WORLD_PATH: &str = "world";

fn main() -> io::Result<()> {
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

    runtime.block_on(server())
}

async fn server() -> io::Result<()> {
    let started = Instant::now();

    // Первые строки — как у обычного сервера: что за сервер, что грузим.
    log_info!("Запуск сервера MCSheriffAnya (Minecraft 26.1.2, протокол 775)");

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
    let settings = config::mcsheriffanya::Settings::load(Path::new(config::mcsheriffanya::FILE));
    log::set_debug(settings.debug);
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
    let overworld = World::open(WORLD_PATH, properties.level_seed, properties.world_kind)?;
    let nether = World::open(
        Dimension::Nether.folder(Path::new(WORLD_PATH)),
        Some(overworld.seed()),
        WorldKind::Nether,
    )?;
    let end = World::open(
        Dimension::End.folder(Path::new(WORLD_PATH)),
        Some(overworld.seed()),
        WorldKind::End,
    )?;

    let mut shared = Shared::with_end(
        overworld,
        nether,
        end,
        PathBuf::from(playerdata::DIRECTORY),
        PathBuf::from(skins::DIRECTORY),
        (*properties).clone(),
        ops::Ops::open(std::path::Path::new(ops::FILE)),
        skins::Settings::load(Path::new(skins::SETTINGS_FILE)),
    );
    *shared.maps.lock().expect("карты") = maps::Maps::open(Path::new(WORLD_PATH))?;
    shared.settings = settings;
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
    mut socket: TcpStream,
    addr: std::net::SocketAddr,
    properties: Arc<ServerProperties>,
    shared: Arc<Shared>,
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
            // Пока ядро понимает одну версию. Когда появится свой перевод
            // между версиями, здесь будет не отказ, а выбор переводчика.
            if handshake.protocol_version != PROTOCOL_VERSION {
                log_info!(
                    "Вход отклонён: версия клиента (протокол {}) не подходит, ядру нужен протокол {}",
                    handshake.protocol_version,
                    PROTOCOL_VERSION
                );

                send_login_disconnect(
                    &mut socket,
                    &format!(
                        "Ваша версия не подходит под ядро сервера\nНужен Minecraft {} (протокол {})",
                        VERSION_NAME, PROTOCOL_VERSION
                    ),
                )
                .await?;

                return Ok(());
            }

            let mut login = read_login_start(&mut socket).await?;

            // Ник — как у оригинала. Иначе длинный ник отключил бы всех
            // остальных, а «../» попал бы в пути файлов.
            if !valid_name(&login.username) {
                log_info!("Вход отклонён: недопустимый ник {:?}", login.username);
                send_login_disconnect(&mut socket, "Недопустимый ник").await?;
                return Ok(());
            }

            // Опознаватель считаем сами, из ника, как оригинал без проверки
            // Mojang: присланному клиентом верить нельзя — к нему привязаны
            // права и файл игрока.
            let sent = login.uuid;
            login.uuid = offline_uuid(&login.username);
            adopt_old_identity(&shared, &login.username, &sent, &login.uuid);

            // Как у обычного сервера: опознаватель игрока и откуда он пришёл —
            // с какого адреса и на какой адрес сервера стучался. Адрес сервера
            // полезен, когда у него несколько имён или он за туннелем.
            reports::known_player(&login.username);
            log_info!("Опознаватель игрока {} — {}", login.username, format_uuid(&login.uuid));

            // Блокировки по нику и адресу и белый список — раньше проверки на
            // «сервер полон», как у оригинала (commands_admin.rs).
            let is_op = shared.ops.lock().expect("права захвачены другим потоком").level_of(&login.uuid) > 0;

            if let Some((json, plain)) = commands_admin::login_refusal(&login.username, &login.uuid, addr.ip(), is_op) {
                log_info!("{} не пущен: {}", login.username, plain.replace('\n', " "));
                send_login_disconnect_json(&mut socket, &json).await?;
                return Ok(());
            }

            let online = shared
                .players
                .lock()
                .expect("список игроков захвачен другим потоком")
                .online();
            if online >= properties.max_players.max(0) as usize {
                log_info!("{} не пущен: сервер полон", login.username);
                send_login_disconnect(&mut socket, "Сервер полон").await?;
                return Ok(());
            }

            // Откуда пришёл и куда стучался: адрес сервера обычный сервер не
            // печатает, а нам он полезен — у сервера может быть несколько имён.
            let came_from = addr.to_string();
            let came_to = format!("{}:{}", handshake.server_address, handshake.server_port);

            // Двух игроков с одним ником на сервере быть не должно: их не
            // отличить ни в чате, ни в списке. Второго не пускаем сразу — так
            // он хотя бы увидит причину, а не молчаливый обрыв связи.
            //
            // Проверка здесь не последняя: между ней и появлением игрока
            // в мире идёт обмен с клиентом, и за это время ник мог занять
            // кто-то другой. Окончательно решает список игроков — он и
            // отказывает, если ник всё-таки заняли.
            let taken = shared
                .players
                .lock()
                .expect("список игроков захвачен другим потоком")
                .name_taken(&login.username);

            if taken {
                log_info!("{} не пущен: ник уже занят", login.username);
                send_login_disconnect(
                    &mut socket,
                    &format!("Игрок с ником {} уже на сервере", login.username),
                )
                .await?;

                return Ok(());
            }

            commands_admin::note_address(&login.uuid, addr.ip());
            // Сжатие включается до Login Success: он уже идёт в новом виде.
            network::login::send_set_compression(&mut socket).await?;
            send_login_success(&mut socket, &login.username, &login.uuid).await?;
            log_debug!("Login Success отправлен игроку {}", login.username);

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

            // Завершение фазы. Какие ещё реестры потребует клиент — покажет
            // его лог: он называет недостающий реестр или тег.
            send_finish_configuration(&mut socket).await?;
            log_debug!("Finish Configuration отправлен");

            let look = read_and_log_configuration_packets(&mut socket, look_from_settings).await?;
            log_debug!("Configuration: чтение пакетов клиента завершено");

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
                )
                .await;

                // Игрока убираем из списка, только если он в него попал.
                // Отказанному убирать нечего, а по опознавателю он совпадает
                // с тем, кто уже играет под тем же ником, — и убрал бы его.
                if matches!(entered, Ok(true)) {
                    player_left(&shared, &login.username, &login.uuid);
                    log_debug!("Play: игрок {} вышел", login.username);
                }

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
