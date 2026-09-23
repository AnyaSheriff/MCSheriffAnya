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
mod config;
mod world;
mod blocks;
mod blocks_table;
mod placing;
mod entity;
mod player;
mod fluids;
mod inventory;
mod journal;
mod items;
mod playerdata;
mod redstone;
mod skins;
mod tick;
mod chat;
mod ops;
mod players;
mod shared;
mod log;
mod commands;
mod console;

use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use tokio::net::{TcpListener, TcpStream};
use tokio::sync::watch;

use config::server_properties::{load_server_properties, ServerProperties};
use network::configuration::{
    read_and_log_configuration_packets, read_known_packs, read_login_acknowledged,
    send_finish_configuration, send_known_packs, send_registry_data, send_update_tags,
};
use network::handshake::read_handshake;
use network::{PROTOCOL_VERSION, VERSION_NAME};
use network::login::{format_uuid, read_login_start, send_login_disconnect, send_login_success};
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
        .max_blocking_threads(others)
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

    if settings.debug {
        log_info!("Подробный лог включён (config/mcsheriffanya.toml)");
    }

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

    let bind_address = format!("0.0.0.0:{}", properties.server_port);
    let listener = TcpListener::bind(&bind_address).await?;
    log_info!("Сервер запущен на *:{}", properties.server_port);
    log_debug!("Слушаем {}", bind_address);

    // Оборачиваем настройки в Arc, чтобы безопасно делиться ими между
    // задачами tokio::spawn, обрабатывающими разные подключения.
    let properties = Arc::new(properties);

    // Всё, что подключения делят между собой: мир, список игроков и чат.
    let shared = Arc::new(Shared::new(
        World::open(WORLD_PATH, properties.level_seed, properties.world_kind)?,
        PathBuf::from(playerdata::DIRECTORY),
        PathBuf::from(skins::DIRECTORY),
        (*properties).clone(),
        ops::Ops::open(std::path::Path::new(ops::FILE)),
        skins::Settings::load(Path::new(skins::SETTINGS_FILE)),
    ));

    // Остановка сервера: консоль сообщает о ней, а ждёт её вот этот приёмник.
    let (shutdown_sender, mut shutdown) = watch::channel(false);

    console::start(Arc::clone(&shared), shutdown_sender);

    // Такт мира: без него жидкости стояли бы на месте.
    tick::start(Arc::clone(&shared));

    log_info!(
        "Готово ({:.3} с)! Чтобы узнать команды, набери help",
        started.elapsed().as_secs_f32()
    );

    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let (socket, addr) = accepted?;
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
    // Игроки пишутся каждый своим подключением при выходе, поэтому здесь
    // только мир: между записями изменения живут только в памяти.
    log_info!("Остановка сервера");
    log_info!("Сохранение игроков");
    log_info!("Сохранение мира");
    shared
        .world
        .lock()
        .expect("мир захвачен другим потоком")
        .save_if_needed();
    log_info!("Мир сохранён");

    // Терминал был переключён на посимвольный ввод — возвращаем как было,
    // иначе в нём потом не видно, что набираешь.
    console::give_terminal_back();

    // Выходим сразу, не дожидаясь задач.
    //
    // Одна из них — чтение клавиатуры, и она ждёт нажатия: пока его нет,
    // выход не заканчивается, и после `stop` терминал так и висел бы до
    // первой нажатой клавиши. Всё, что нужно было сохранить, к этому времени
    // уже на диске: мир и игроки пишутся сразу, а не при выходе.
    std::process::exit(0);
}

/// Ждёт сигнала остановки от системы: Ctrl-C или просьбы завершиться.
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

            let login = read_login_start(&mut socket).await?;

            // Как у обычного сервера: опознаватель игрока и откуда он пришёл —
            // с какого адреса и на какой адрес сервера стучался. Адрес сервера
            // полезен, когда у него несколько имён или он за туннелем.
            log_info!("Опознаватель игрока {} — {}", login.username, format_uuid(&login.uuid));

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

            send_login_success(&mut socket, &login.username, &login.uuid).await?;
            log_debug!("Login Success отправлен игроку {}", login.username);

            // Фаза Configuration.
            read_login_acknowledged(&mut socket).await?;
            log_debug!("Login Acknowledged получен — клиент в фазе Configuration");

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
        other => {
            log_info!("Неизвестное значение next_state: {}", other);
        }
    }

    Ok(())
}
