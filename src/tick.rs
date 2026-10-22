// Такт мира: то, что происходит само, без игроков.
//
// До сих пор сервер шевелился только в ответ на пакеты: пришёл пакет — что-то
// изменилось. Жидкости так не сделать: вода течёт сама, даже когда все стоят
// на месте. Отсюда и такт — шаг времени мира, двадцать раз в секунду, как
// в игре.
//
// Пересчитывается не весь мир: мир помнит места, которые надо проверить, и на
// каждом такте отдаёт те, которым пора. Поэтому стоящая вода ничего не стоит:
// пока её никто не трогает, проверять нечего.

mod idle;

use std::sync::Arc;
use std::time::Duration;

use crate::blocks;
use crate::fluids;
use crate::{log_debug, log_error, log_warn};
use crate::dimension::Dimension;
use crate::shared::Shared;
use crate::inventory::Stack;
use crate::items;
use crate::redstone;
use crate::containers;
use crate::tnt;
use crate::world::{TickKind, World};

/// Сколько длится такт.
const TICK: Duration = Duration::from_millis(50);

/// Как часто мир записывается на диск: раз в секунду.
const SAVE_EVERY: u64 = 20;

/// Такт дольше этого пишется в подробный лог.
const SLOW_TICK: Duration = Duration::from_millis(40);

/// Насколько мир может отстать от часов, чтобы ещё догонять.
const CATCH_UP_LIMIT: Duration = Duration::from_secs(2);

/// Как часто мир убирает из памяти чанки, до которых никому нет дела.
const UNLOAD_EVERY: u64 = 100;

/// Насколько дальше видимого чанки ещё держатся в памяти.
///
/// Меньше дальности прорисовки держать нельзя — выгрузился бы чанк, который
/// игрок видит. А запас нужен, чтобы чанк не выгружался и не читался снова,
/// когда игрок ходит туда-сюда через границу.
///
/// Мир, сложенный впрок (настройка `prefetch_chunks`), тоже держится: иначе
/// его выгружало бы сразу после складывания.
const KEEP_EXTRA: i32 = 2;

/// Сколько такт ждёт применения уже прочитанных действий игроков (щелчки,
/// команды, пакеты места). Ждать дольше нельзя: застрявшее подключение
/// остановило бы мир.
const PATIENCE: Duration = Duration::from_millis(5);

/// Сколько мест пересчитывается за один такт.
///
/// Столько же, сколько в игре: 65 536. Предел нужен на случай, если кто-то
/// устроит водопад во весь мир — лучше разложить работу на несколько тактов,
/// чем задержать всё остальное. Лишнее не пропадает, а ждёт своей очереди.
pub const PER_TICK: usize = 65_536;

/// Запускает такт мира. Управление возвращается сразу, работа идёт до
/// остановки сервера.
///
/// Такт живёт в своём потоке и на своём ядре, отдельно от всего остального.
/// Причина простая: такт обязан просыпаться каждые пятьдесят миллисекунд, и
/// если его вытеснит разбор пакетов или складывание чанков, мир дёрнется.
/// Остальная работа — сеть, генерация, скины — раскладывается по оставшимся
/// ядрам и такту не мешает.
pub fn start(shared: Arc<Shared>) {
    let started = std::thread::Builder::new()
        .name("world tick".to_string())
        .spawn(move || {
            // Ядро для такта — последнее: первые обычно занимают прерывания.
            if let Some(core) = tick_core() {
                pin_to_core(core);
            }

            // Свой однопоточный исполнитель: в этом потоке больше ничего не
            // крутится, и делить его не с кем.
            let alone = tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .build()
                .expect("исполнитель такта");

            alone.block_on(run(shared));
        });

    if let Err(e) = started {
        log_error!("Не удалось отвести такту отдельный поток: {}", e);
    }
}

/// Сколько ядер у машины.
pub fn cores() -> usize {
    std::thread::available_parallelism().map(|count| count.get()).unwrap_or(1)
}

/// Какое ядро отводится такту. На одноядерной машине — никакое: делить
/// нечего, и привязка только помешает.
pub fn tick_core() -> Option<usize> {
    let cores = cores();

    if cores < 2 { None } else { Some(cores - 1) }
}

/// Уводит нынешний поток с ядра такта: пусть работает на любом другом.
///
/// Так рабочие потоки сети и генерации не отбирают у такта его ядро.
#[cfg(not(target_os = "linux"))]
pub fn keep_off_tick_core() {}

/// Уводит нынешний поток с ядра такта (только Linux: `sched_setaffinity`).
#[cfg(target_os = "linux")]
pub fn keep_off_tick_core() {
    let Some(tick_core) = tick_core() else {
        return;
    };

    // SAFETY: заполняем набор ядер по правилам libc; нулевой первый довод
    // означает «этот поток».
    unsafe {
        let mut set: libc::cpu_set_t = std::mem::zeroed();

        libc::CPU_ZERO(&mut set);

        for core in 0..cores() {
            if core != tick_core {
                libc::CPU_SET(core, &mut set);
            }
        }

        libc::sched_setaffinity(0, size_of::<libc::cpu_set_t>(), &set);
    }
}

/// Привязывает нынешний поток к одному ядру.
///
/// Без привязки планировщик перекидывает поток с ядра на ядро, и каждый
/// переезд стоит промаха по кэшу. Такту это особенно заметно: он трогает
/// один и тот же мир двадцать раз в секунду.
#[cfg(not(target_os = "linux"))]
fn pin_to_core(_core: usize) {}

/// Привязка к ядру — только Linux; на macOS и Windows планировщик решает сам.
#[cfg(target_os = "linux")]
fn pin_to_core(core: usize) {
    // SAFETY: заполняем набор ядер по правилам libc и передаём его как есть.
    // Нулевой первый довод означает «этот поток».
    unsafe {
        let mut set: libc::cpu_set_t = std::mem::zeroed();

        libc::CPU_ZERO(&mut set);
        libc::CPU_SET(core, &mut set);

        if libc::sched_setaffinity(0, size_of::<libc::cpu_set_t>(), &set) != 0 {
            log_warn!("Такт не удалось привязать к ядру {}", core);
        }
    }
}

async fn run(shared: Arc<Shared>) {
    // Сетка тактов — по 50 мс от запуска. Задержался такт (запись мира,
    // складывание чанков) — пропущенные такты делаются подряд, пока мир не
    // догонит часы: так у оригинала (снято чёрным ящиком — с пропуском мир
    // навсегда отставал на такт, и вода текла на такт позже). Только при
    // отставании больше двух секунд, как и оригинал, сдаёмся: пишем в лог и
    // начинаем сетку заново, иначе после долгой заминки мир мчался бы
    // рывком.
    let mut next = tokio::time::Instant::now();
    let mut presence = shared.players.lock().expect("игроки").presence();
    let mut ticks: u64 = 0;
    let mut phases = PhaseStats::default();
    let mut mob_rng = crate::mob::Rng::new(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |passed| passed.as_nanos() as u64));
    // Ночь пропускается в начале этого такта (`morning`): спящих будят
    // концом прошлого.
    let mut morning_due = false;

    loop {
        // Пауза пустого сервера сильнее freeze/step/sprint: их счётчики ждут
        // входа игрока; функции и плагины тоже не тикают в пустом мире.
        if let Some(paused) = idle::wait_tick(&shared, &mut presence, next).await {
            next = tokio::time::Instant::now();
            shared.world_cmd.ticking.lock().expect("такт").resumed_after_idle(paused);
        }
        let behind = tokio::time::Instant::now().saturating_duration_since(next);

        if behind > CATCH_UP_LIMIT {
            let skipped = behind.as_millis() / TICK.as_millis();
            log_warn!(
                "Не успеваю! Сервер отстал на {} мс, пропускаю тактов: {}",
                behind.as_millis(),
                skipped
            );
            next = tokio::time::Instant::now();
        }

        // Скорость, заморозка, шаги и разгон — команда `tick`
        // (commands_world.rs). При 20 тактах в секунду — те же 50 мс.
        let plan = crate::commands_world::next_tick(&shared);

        if plan.period.is_zero() {
            // Разгон: без ожидания и без догонялок по сетке.
            next = tokio::time::Instant::now();
        } else {
            next += if plan.period == TICK { TICK } else { plan.period };
        }

        ticks += 1;

        let started = std::time::Instant::now();
        shared.world_cmd.ticking.lock().expect("такт").note_start(started);

        // Где лежат предметы к началу такта — по этим местам их подбирают
        // игроки, как у оригинала, где подбор идёт раньше шага предметов.
        for dimension in Dimension::ALL {
            shared.items_in(dimension).lock().expect("предметы захвачены другим потоком").note_tick_start();
        }

        // Сперва — пакеты игроков, что накопились за прошлый такт: их
        // применяют сами подключения, а такт ждёт, пока они закончат.
        // Ждём недолго: застрявшее подключение не должно останавливать мир.
        shared.tick_starts.send_replace(ticks);

        let patience = tokio::time::Instant::now() + PATIENCE;

        let earlier = &shared.waiting_for_tick[(ticks.wrapping_sub(1) % 2) as usize];

        while earlier.load(std::sync::atomic::Ordering::Acquire) > 0
            && tokio::time::Instant::now() < patience
        {
            tokio::task::yield_now().await;
        }

        if earlier.load(std::sync::atomic::Ordering::Acquire) > 0 {
            log_debug!("Такт {}: действия игроков не успели к началу такта", ticks);
        }

        // Функции: `#minecraft:tick` и запланированное `schedule` — в начале
        // такта, после действий игроков, без замков (functions.rs).
        crate::functions::tick(&shared, ticks);

        // Задачи плагинов: там же, без замков (plugins/mod.rs).
        crate::plugins::tick(ticks);

        // Перенесённые и подтвердившие перенос — на такт ближе к тому, чтобы
        // встать на блок под ногами (`players::SETTLE_TICKS`).
        shared.players.lock().expect("список игроков захвачен другим потоком").settle();

        // Работа такта — в своём блоке: замок мира отпускается до сигнала
        // подключениям. Миры измерений идут по очереди, каждый со своими
        // предметами; два мира разом не держатся.
        // Заморозка (`tick freeze`): мир не идёт — ни блоки, ни сущности,
        // ни время; игроки — идут (вики, Commands/tick).
        if plan.run_world {
            crate::commands_world::step_borders(&shared);
        }

        for dimension in Dimension::ALL.into_iter().filter(|_| plan.run_world) {
            let mut world = shared.world_in(dimension).lock().expect("мир захвачен другим потоком");

            // Кто где стоит — для растяжек: их такт проверяет, не ушли ли
            // с них.
            let mut clock = std::time::Instant::now();
            let (present, mut player_chunks) = occupants(&shared, dimension);

            // Удержанные чанки (`forceload`) тикают, как чанки у игрока.
            player_chunks.extend(crate::commands_world::forced_chunks(&shared, dimension));
            redstone::note_occupants(&mut world, present);
            sculk_prepare(&shared, dimension, &mut world);

            // Где вагонетки — для детекторных рельс: их такт проверяет, не
            // уехала ли вагонетка (её могла увести команда в начале такта).
            redstone::note_carts(&mut world, cart_marks(&shared, dimension));
            redstone::note_frames(&mut world, frame_marks(&shared, dimension));

            // Погода — в начале такта мира, до запланированных тактов
            // (чёрный ящик, weather-rain: сила дождя меняется уже в такте
            // команды).
            if dimension == Dimension::Overworld && std::mem::take(&mut morning_due) {
                morning(&shared, &mut world);
                morning_gifts(&shared, &world);
            }

            world.fire_difficulty = shared.difficulty.load(std::sync::atomic::Ordering::Relaxed);
            world.tick_weather(shared.advance_weather.load(std::sync::atomic::Ordering::Relaxed));

            let due = world.advance(PER_TICK);

            for ((x, y, z), kind) in due {
                match kind {
                    TickKind::Block => redstone::ticked(&mut world, (x, y, z)),
                    TickKind::Fluid => flow(&mut world, x, y, z),
                }

                // Каждый запланированный такт разбирается до конца, прежде чем
                // начнётся следующий: так в игре, и от этого зависит порядок
                // срабатывания деталей внутри одного такта.
                redstone::settle(&mut world);

                // Командный блок дождался такта: его команду (и цепочку за
                // ним) выполняет общий разбор команд, а он берёт замок мира
                // сам — мир на это время отпускается (command_block.rs).
                let runs = world.take_command_runs();

                if !runs.is_empty() {
                    drop(world);
                    crate::command_block::run_queued(&shared, dimension, runs);
                    world = shared.world_in(dimension).lock().expect("мир захвачен другим потоком");
                }
            }

            // Песок и гравий, чей такт падения пришёл, — в полёт сразу, в
            // фазе запланированных тактов, как у оригинала: место пустеет в
            // той же рассылке (сценарий sand-into-water).
            start_falling(&shared, dimension, &mut world);
            phases.add(0, &mut clock);

            // Случайные такты — после запланированных, до рассылки (вики,
            // Tick: фаза чанков). Из них у нас пока только горящая руда.
            natural_lightning(&shared, dimension, &mut world, &player_chunks);
            redstone::random_ticks(&mut world, &player_chunks, shared.properties.simulation_distance);
            crate::farming::random_ticks(&mut world, &player_chunks, shared.properties.simulation_distance, crate::farming::random_tick_speed());
            redstone::settle(&mut world);
            phases.add(1, &mut clock);

            // Изменения блоков уходят клиентам пачкой — здесь, до блочных
            // событий, как у оригинала.
            world.note_broadcast();

            // Поршни трогаются отдельной фазой, после всех запланированных
            // тактов: так в игре. Заведённое во время этой фазы ждёт следующего
            // такта — иначе цепочка поршней срабатывала бы вся разом.
            // События, заведённые по ходу этой фазы, исполняются в ней же.
            while let Some((place, kind, param)) = world.take_block_event() {
                redstone::block_event(&mut world, place, kind, param);
                redstone::settle(&mut world);
            }

            phases.add(2, &mut clock);

            // Выброшенное выбрасывателями и раздатчиками — в мир; туда же
            // подожжённый динамит: в фазе сущностей этого же такта его запал
            // уже убывает.
            spawn_ejected(&shared, dimension, &mut world);
            spawn_primed(&shared, dimension, &mut world);

            // Молнии бьют в первом же такте: искры уходят в этом такте, а
            // громоотвод и лампа — со следующей рассылкой (чёрный ящик,
            // lightning-rod).
            step_bolts(&shared, dimension, &mut world);

            // Вагонетки катятся — сущности, как и предметы ниже; детекторные
            // рельсы под ними включаются в этой же фазе (чёрный ящик,
            // rail-detector: рельса и лампа — в рассылке следующего такта
            // после появления вагонетки).
            step_carts(&shared, dimension, &mut world);
            explode_carts(&shared, dimension, &mut world);
            hopper_carts(&shared, dimension, &mut world);
            redstone::note_carts(&mut world, cart_marks(&shared, dimension));

            // Рамки: загруженные с чанками встают в мир, а те, у которых
            // убрали опору, отваливаются на своей проверке.
            step_frames(&shared, dimension, &mut world);

            // Мобы: мысли, движение, разведение, добыча погибших.
            step_mobs(&shared, dimension, &mut world, &mut mob_rng, ticks);

            // Стрелы: полёт, попадание, застревание (arrow.rs).
            crate::arrow::step(&shared, dimension, &mut world);
            crate::fishing::step(&shared, dimension, &mut world);

            // Брошенные зелья, огненные шары, облака (thrown.rs).
            crate::thrown::step(&shared, dimension, &mut world);
            let kills = shared.items_in(dimension).lock().expect("предметы захвачены другим потоком").take_progress_kills();
            if !kills.is_empty() {
                let mut players = shared.players.lock().expect("список игроков захвачен другим потоком");
                for (uuid, kind) in kills {
                    players.order(&uuid, crate::players::Order::Progress(crate::progress::Event::Killed { kind, dimension }));
                }
            }

            // Плиты, растяжки и руда слушают не только сигнал, но и мир: на
            // них встают игроки и падают предметы. Список — заново: только
            // что выброшенное выбрасывателями тоже в счёт.
            let (standing, _) = occupants(&shared, dimension);
            redstone::note_occupants(&mut world, standing);
            redstone::step_entities(&mut world);

            // Всё, что изменилось за такт, должно дойти до соседей по редстоуну.
            redstone::settle(&mut world);

            // Поршень раздавил траву, у провода убрали опору — из сломанного
            // должны выпасть предметы.
            drop_destroyed(&shared, dimension, &mut world);

            // Песок и гравий, под которыми не стало опоры, отправляются в полёт.
            start_falling(&shared, dimension, &mut world);

            // Выброшенные предметы летят и падают. О блоки они упираются, поэтому
            // мир нужен и здесь. Предмет ложится на всё, у чего есть ящик, —
            // не проваливается в ступени и заборы. Падающий блок — как раньше,
            // только на твёрдый верх: в невысокий блок (сундук, нижнюю плиту)
            // он проходит и там разбивается, как у оригинала. В выгруженных
            // чанках мира нет — там всё замирает, а не проваливается в пустоту.
            let solid = |x, y, z| blocks::stops_items(world.get_block(x, y, z));
            let holds_block = |x, y, z| blocks::solid_top(world.get_block(x, y, z));
            let loaded = |chunk_x, chunk_z| world.has_chunk(chunk_x, chunk_z);

            let water = |x, y, z| {
                let view = |x, y, z| world.get_block(x, y, z);
                fluids::item_water((x, y, z), &view)
            };

            // Шарики опыта тянутся к ближайшему игроку (не наблюдателю) и
            // подбираются им в этом же такте.
            let pullers: Vec<(i32, (f64, f64, f64))> = shared
                .players
                .lock()
                .expect("список игроков захвачен другим потоком")
                .members()
                .iter()
                .filter(|member| member.dimension == dimension && member.game_mode != 3)
                .map(|member| (member.entity_id, (member.position.x, member.position.y, member.position.z)))
                .collect();

            let (splashes, exploding, gains) = {
                let mut items = shared.items_in(dimension).lock().expect("предметы захвачены другим потоком");
                let splashes = items.step_with_water(&solid, &holds_block, &loaded, &water);

                let gains = items.step_orbs(&pullers, &solid);

                // Сперва шаг, потом запал — у каждого динамита.
                (splashes, items.burn_fuses(&loaded), gains)
            };

            // Подобранное шариками уходит подключениям: они прибавят очки и
            // скажут клиенту после пакета подбора.
            if !gains.is_empty() {
                let mut players = shared.players.lock().expect("список игроков захвачен другим потоком");

                for (entity_id, points) in gains {
                    if let Some(uuid) = players.members().iter().find(|member| member.entity_id == entity_id).map(|member| member.uuid) {
                        players.order(&uuid, crate::players::Order::XpTaken { points });
                    }
                }
            }

            // Предмет вошёл в воду — плеск, как у оригинала.
            for splash in splashes {
                let pitch = 1.0 + (world.random_unit() - world.random_unit()) * 0.4;
                world.note_sound_at(splash.at, "minecraft:entity.generic.splash", splash.volume, pitch);
            }

            // Большой взрыв особого динамита дорушивает своё — по частям.
            continue_blasts(&shared, dimension, &mut world);

            // Догоревший динамит взрывается — по порядку появления.
            explode_all(&shared, dimension, &mut world, exploding);

            // Задетые стрелой или взрывом кристаллы Края взрываются следом.
            detonate_crystals(&shared, dimension, &mut world);

            // Долетевшие до земли снова становятся блоками.
            land_falling(&shared, dimension, &mut world);

            // Плиты нажимаются по местам после шага: предмет жмёт плиту,
            // когда на неё упал, а не когда пролетал клетку (чёрный ящик,
            // plate-items).
            let (moved, _) = occupants(&shared, dimension);
            redstone::step_plates(&mut world, &moved);
            redstone::settle(&mut world);
            phases.add(3, &mut clock);

            // Урон мобов, стрел и взрывов этого такта тоже запускает реакции
            // до конца такта (вики, Infested: «takes damage», Oozing: «dies»).
            {
                let targets = blast_targets(&shared, dimension);
                let mut items = shared.items_in(dimension).lock().expect("сущности");
                crate::effect_reactions::mobs(&mut world, &mut items, shared.mob_griefing.load(std::sync::atomic::Ordering::Relaxed), &targets, &mut || shared.next_entity_id(), &mut mob_rng);
            }

            // Игроки видят содержимое хранилищ таким, каким оно стало к фазе
            // сущностей: что воронки перенесут дальше, окна увидят в
            // следующем такте (снято чёрным ящиком, hopper-chain).
            // Вагонетки-хранилища — тоже: их окна видят то же время.
            let carts = shared
                .items_in(dimension)
                .lock()
                .expect("предметы захвачены другим потоком")
                .carts()
                .iter()
                .filter(|cart| !cart.slots.is_empty())
                .map(|cart| (cart.entity_id, cart.slots.clone()))
                .collect();

            world.publish_contents(carts);

            // Фаза блок-сущностей: датчики света (раз в 20 тактов), воронки,
            // потом ходы поршней, которым пора кончиться (они появились позже
            // воронок).
            crate::beacon::tick(&shared, dimension, &mut world);
            redstone::daylight_tick(&mut world);
            redstone::sensors_tick(&mut world);
            sculk_finish(&shared, dimension, &mut world, ticks);
            containers::tick_hoppers(
                &mut world,
                &mut shared.items_in(dimension).lock().expect("предметы захвачены другим потоком"),
            );
            redstone::settle(&mut world);
            shove_entities(&shared, dimension, &world);
            redstone::finish_due_moves(&mut world);
            spill_containers(&shared, dimension, &mut world);
            phases.add(4, &mut clock);

            if ticks.is_multiple_of(SAVE_EVERY) {
                world.save_if_needed();
                if dimension == Dimension::Overworld && let Err(e) = shared.maps.lock().expect("карты").save() { log_warn!("Карты не записаны: {}", e); }
            }

            if ticks.is_multiple_of(UNLOAD_EVERY) {
                unload_empty_chunks(&shared, dimension, &mut world);
            }

            phases.add(5, &mut clock);
        }

        // Работа такта сделана: подключения рассылают, что случилось.
        // Долгий такт — повод разобраться: подключения узнают о нём позже,
        // и схемы у игроков начинают запаздывать.
        let spent = started.elapsed();

        if spent > SLOW_TICK {
            log_debug!("Такт {} занял {} мс", ticks, spent.as_millis());
        }

        phases.tick_done(spent);
        crate::commands_world::note_spent(&shared, spent);
        if ticks.is_multiple_of(20) { crate::presentation::tick_footer(&shared); }

        if plan.run_world {
            morning_due = wake_sleepers(&shared);
        }

        if ticks.is_multiple_of(PHASE_REPORT_EVERY) {
            phases.report();
        }

        // Сначала фиксируем рассылку, потом будим подключения. Их поздние
        // публикации относятся уже к следующему концу такта [проверить].
        {
            let _publication = shared.publication.lock().expect("публикация такта");
            for dimension in Dimension::ALL {
                shared.world_in(dimension).lock().expect("мир").publish_events();
            }
            shared.players.lock().expect("игроки").publish_tick();
            shared.ticks.send_replace(ticks);
        }
    }
}

/// Раз во сколько тактов писать в лог (при `debug`), сколько времени ушло на
/// каждую фазу такта: по этому видно, что оптимизировать.
const PHASE_REPORT_EVERY: u64 = 200;

/// Время фаз такта, накопленное между отчётами.
#[derive(Default)]
struct PhaseStats {
    /// По фазам: запланированные такты, случайные такты, блочные события,
    /// сущности, блок-сущности, запись и выгрузка.
    spent: [Duration; 6],

    /// Весь такт: сумма и самый долгий.
    total: Duration,
    longest: Duration,
    ticks: u32,
}

impl PhaseStats {
    const NAMES: [&'static str; 6] = ["такты", "случайные", "события", "сущности", "блок-сущности", "запись"];

    /// Прибавить время с прошлой отметки к фазе и сдвинуть отметку.
    fn add(&mut self, phase: usize, clock: &mut std::time::Instant) {
        let now = std::time::Instant::now();

        self.spent[phase] += now - *clock;
        *clock = now;
    }

    fn tick_done(&mut self, spent: Duration) {
        self.total += spent;
        self.longest = self.longest.max(spent);
        self.ticks += 1;
    }

    /// В лог — средние по фазам (мкс на такт) и самый долгий такт; счёт
    /// начинается заново.
    fn report(&mut self) {
        if self.ticks == 0 {
            return;
        }

        let per_tick = |spent: Duration| spent.as_micros() / self.ticks as u128;
        let parts: Vec<String> = Self::NAMES
            .iter()
            .zip(self.spent)
            .map(|(name, spent)| format!("{} {}", name, per_tick(spent)))
            .collect();

        log_debug!(
            "Такт: в среднем {} мкс ({}), самый долгий {} мс",
            per_tick(self.total),
            parts.join(", "),
            self.longest.as_millis()
        );

        *self = PhaseStats::default();
    }
}

/// Событие шага (частота 1) отдельно от приземления: у ковра ноги на y+1/16,
/// прежнее y-0.2 попадает в подложку. Мох не меняет прежний выбор подложки.
fn quiet_step_at(world: &World, at: redstone::sculk_family::Point) -> bool {
    let x = at.0.floor() as i32; let z = at.2.floor() as i32;
    let feet = world.get_block(x, at.1.floor() as i32, z);
    // Допуск только для выбранной высоты контакта, не модель физики опоры.
    let carpet_contact = (at.1 - (at.1.floor() + 1.0 / 16.0)).abs() <= 1.0e-5;
    if carpet_contact && blocks::block_at_state(feet).is_some_and(|name| name.ends_with("_carpet") && redstone::sculk_family::quiet_block(name)) {
        return true;
    }
    let below = world.get_block(x, (at.1 - 0.2).floor() as i32, z);
    redstone::sculk_family::quiet_step_state(world.catalog(), below)
}

/// Снимки для скалка: UUID переживает выход и смену измерения; шаги берутся
/// из движения, а не из звуков. [проверить] порог шага настоящего клиента.
fn sculk_prepare(shared: &Shared, dimension: Dimension, world: &mut World) {
    use redstone::sculk_family::{self, Player};
    let players: Vec<_> = {
        let members = shared.players.lock().expect("игроки");
        members.members().iter().filter(|member| member.dimension == dimension && member.game_mode != 3)
            .map(|member| (Player { id: member.entity_id, uuid: member.uuid,
                at: (member.position.x, member.position.y, member.position.z),
                survival: matches!(member.game_mode, 0 | 2) && members.gear(&member.uuid).health > 0.0,
                sneaking: member.sneaking }, member.position.on_ground, member.unsettled))
            .collect()
    };
    world.redstone.sculk.players = players.iter().map(|(p, _, _)| *p).collect();
    world.redstone.sculk.enabled = shared.difficulty.load(std::sync::atomic::Ordering::Relaxed) != crate::survival::PEACEFUL;
    {
        let mut warnings = shared.sculk_warnings.lock().expect("предупреждения");
        world.redstone.sculk.warnings.clear();
        for (player, _, _) in &players {
            let warning = warnings.entry(player.uuid).or_insert_with(|| sculk_family::load_warning(&shared.playerdata, &player.uuid));
            warning.tick();
            world.redstone.sculk.warnings.insert(player.uuid, *warning);
        }
    }
    world.redstone.sculk.footsteps.retain(|id, _| players.iter().any(|(p, _, _)| p.id == *id));
    for (player, ground, unsettled) in players {
        let old = world.redstone.sculk.footsteps.insert(player.id, (player.at, 0.0, ground));
        if let Some((old, walked, was_ground)) = old {
            let distance = crate::mob::warden::distance(old, player.at);
            if unsettled > 0 || distance > 8.0 { continue; }
            let cell = (player.at.0.floor() as i32, (player.at.1 - 0.2).floor() as i32, player.at.2.floor() as i32);
            let floor = blocks::block_at_state(world.get_block(cell.0, cell.1, cell.2)).unwrap_or("air");
            let quiet = if ground && was_ground { quiet_step_at(world, player.at) }
                else { sculk_family::quiet_block(floor) };
            if quiet || player.sneaking { continue; }
            let walked = walked + distance * 0.6;
            if ground && (!was_ground || walked >= 1.0) {
                redstone::game_event_from(world, player.at, if was_ground { 1 } else { 2 }, Some(player.id), false);
            }
            world.redstone.sculk.footsteps.insert(player.id, (player.at, if ground && walked >= 1.0 { walked.fract() } else { walked }, ground));
        }
    }
    let mut items = shared.items_in(dimension).lock().expect("предметы");
    items.sculk_catalysts = world.redstone.sculk.catalysts.iter().copied()
        .filter(|p| world.has_chunk(p.0.div_euclid(16), p.2.div_euclid(16))).collect();
    world.redstone.sculk.wardens = items.mobs().iter().filter(|mob| mob.kind == crate::mob::Kind::Warden && !mob.is_dead())
        .map(|mob| (mob.x, mob.y, mob.z)).collect();
}

fn sculk_finish(shared: &Shared, dimension: Dimension, world: &mut World, ticks: u64) {
    {
        let mut items = shared.items_in(dimension).lock().expect("предметы");
        for charge in std::mem::take(&mut items.sculk_deaths) {
            redstone::sculk_family::feed(world, charge.catalyst, charge.at, charge.amount);
        }
        for event in std::mem::take(&mut items.vibrations) {
            // Событие шага моба использует ту же политику; обычной проверки мобов не было.
            // Приземление (частота 2) остаётся на прежнем пути без обобщения шага.
            let floor = world.get_block(event.at.0.floor() as i32, (event.at.1 - 0.2).floor() as i32, event.at.2.floor() as i32);
            let quiet = match event.frequency {
                1 => quiet_step_at(world, event.at),
                2 => blocks::block_at_state(floor).is_some_and(redstone::sculk_family::quiet_block),
                _ => false,
            };
            if quiet { continue; }
            redstone::game_event_from(world, event.at, event.frequency, event.source, event.projectile);
        }
    }
    let mut warnings = shared.sculk_warnings.lock().expect("предупреждения");
    for (uuid, warning) in &world.redstone.sculk.warnings {
        let changed = warnings.get(uuid).is_none_or(|old| old.level != warning.level);
        warnings.insert(*uuid, *warning);
        if (changed || ticks.is_multiple_of(200)) && let Err(error) = redstone::sculk_family::save_warning(&shared.playerdata, uuid, *warning) {
            crate::log_warn!("Не записано предупреждение крикуна: {}", error);
        }
    }
}

fn sculk_deliver(world: &mut World, items: &mut crate::items::Items, rng: &mut crate::mob::Rng, next_id: &mut dyn FnMut() -> i32) {
    use crate::mob::{Kind, warden};
    let events = std::mem::take(&mut world.redstone.sculk.events);
    for mob in items.mobs_mut().iter_mut().filter(|mob| mob.kind == Kind::Warden && !mob.is_dead() && !mob.no_ai) {
        let at = (mob.x, mob.y + mob.eye(), mob.z);
        let heard = events.iter().filter(|event| event.source != Some(mob.entity_id))
            .filter(|event| warden::distance(event.at, at) <= 16.0 && !redstone::sculk_family::occluded(world, event.at, at))
            .min_by(|a, b| warden::distance(a.at, at).total_cmp(&warden::distance(b.at, at)).then(b.frequency.cmp(&a.frequency)));
        if let Some(event) = heard {
            let distance = warden::distance(event.at, at);
            let left = (distance.floor() as i32).max(1);
            let was_free = mob.warden.heard.is_none();
            mob.warden.hear(warden::Heard { source: event.source, at: event.at, projectile: event.projectile, left });
            // Сценарии warden-melee и warden-sonic-boom: хранитель слышит вибрацию —
            // частица летит от места события к нему.
            if was_free && mob.warden.heard.is_some() {
                world.note_vibration_at(event.at, (mob.x.floor() as i32, (mob.y + mob.eye()).floor() as i32, mob.z.floor() as i32), left);
            }
        }
    }
    for pos in std::mem::take(&mut world.redstone.sculk.summons) {
        if !world.redstone.sculk.enabled || items.mobs().iter().any(|mob| mob.kind == Kind::Warden && !mob.is_dead()
            && (mob.x - f64::from(pos.0)).abs() <= 48.0 && (mob.y - f64::from(pos.1)).abs() <= 48.0 && (mob.z - f64::from(pos.2)).abs() <= 48.0) { continue; }
        if let Some(at) = warden::spawn_spot(&MobView(world), pos, rng) {
            let mut mob = crate::mob::Mob::new(next_id(), Kind::Warden, at);
            // Сценарий shrieker-warning: «agitated» в точке появления, затем emerge.
            world.note_sound_at(at, "minecraft:entity.warden.agitated", 5.0, 1.0);
            warden::emerge(&mut mob);
            mob.warden.calm = warden::CALM;
            mob.warden.darkness_phase = 115;
            items.add_mob(mob);
        }
    }
}

/// Кто в этом мире может нажать плиту, задеть растяжку, встать на руду:
/// игроки (кроме наблюдателей — они сквозь всё проходят) и то, что лежит
/// в мире. Заодно — чанки, где стоят игроки: около них идут случайные такты.
fn occupants(shared: &Shared, dimension: Dimension) -> (Vec<redstone::Occupant>, Vec<(i32, i32)>) {
    /// Режим наблюдателя.
    const SPECTATOR: i32 = 3;

    let mut standing = Vec::new();
    let mut chunks = Vec::new();

    for member in shared
        .players
        .lock()
        .expect("список игроков захвачен другим потоком")
        .members()
        .iter()
        .filter(|member| member.dimension == dimension)
    {
        let (x, y, z) = (member.position.x, member.position.y, member.position.z);

        chunks.push(((x.floor() as i32).div_euclid(16), (z.floor() as i32).div_euclid(16)));

        if member.game_mode != SPECTATOR {
            // Только что перенесённый игрок ещё не стоит на блоке под ногами
            // (`players::SETTLE_TICKS`, sculk-shrieker).
            standing.push(redstone::Occupant {
                player: Some(member.entity_id),
                on_ground: member.unsettled == 0,
                ..redstone::Occupant::player(x, y, z, member.sneaking)
            });
        }
    }

    standing.extend(
        shared
            .items_in(dimension)
            .lock()
            .expect("предметы захвачены другим потоком")
            .items()
            .iter()
            .map(|item| match item.block {
                Some(_) => redstone::Occupant::falling_block(item.x, item.y, item.z, item.on_ground),
                None => redstone::Occupant::item(item.x, item.y, item.z, item.on_ground),
            }),
    );

    standing.extend(
        shared
            .items_in(dimension)
            .lock()
            .expect("предметы захвачены другим потоком")
            .carts()
            .iter()
            .map(|cart| redstone::Occupant::minecart(cart.x, cart.y, cart.z, cart.on_ground || cart.on_rail)),
    );

    // Мобы — живые: жмут и деревянные, и каменные плиты, задевают растяжки.
    standing.extend(
        shared
            .items_in(dimension)
            .lock()
            .expect("предметы захвачены другим потоком")
            .mobs()
            .iter()
            .filter(|mob| mob.kind != crate::mob::Kind::Bat)
            .map(|mob| {
                let (width, height) = mob.size();

                redstone::Occupant::mob(mob.x, mob.y, mob.z, width, height, mob.on_ground)
            }),
    );

    (standing, chunks)
}

/// Едущие блоки поршней делают шаг и толкают сущности на пути: предметы
/// (с динамитом и падающими блоками), вагонетки и игроков. Наблюдатели
/// сквозь всё проходят. Игрок упирается в стену (piston-push-wall).
/// Выдвигаемая слизь подбрасывает только предметы [проверить]: вагонетки
/// и игроков.
fn shove_entities(shared: &Shared, dimension: Dimension, world: &World) {
    /// Режим наблюдателя.
    const SPECTATOR: i32 = 3;

    /// Рамка игрока: 0,6 в ширину и 1,8 в высоту (вики, Player).
    const HALF_WIDTH: f64 = 0.3;
    const HEIGHT: f64 = 1.8;

    let shoves = redstone::shoves(world);

    if shoves.is_empty() {
        return;
    }

    let solid = |x, y, z| blocks::stops_items(world.get_block(x, y, z));
    let walls = |x, y, z| blocks::has_box(world.get_block(x, y, z));

    {
        let mut items = shared.items_in(dimension).lock().expect("предметы захвачены другим потоком");
        let view = |x, y, z| world.get_block(x, y, z);

        items.shove(&shoves, &solid);

        // Вагонетки — тем же порядком: каждый едущий блок по очереди.
        for shove in &shoves {
            for cart in items.carts_mut() {
                let (min, max) = cart.bounds();

                if let Some(distance) = redstone::shove_distance(shove, min, max) {
                    cart.shoved(&view, (shove.dir.0 * distance, shove.dir.1 * distance, shove.dir.2 * distance));
                }
            }
        }
    }

    let mut players = shared.players.lock().expect("список игроков захвачен другим потоком");
    let standing: Vec<i32> = players
        .members()
        .iter()
        .filter(|member| member.dimension == dimension && member.game_mode != SPECTATOR)
        .map(|member| member.entity_id)
        .collect();

    for shove in &shoves {
        for &entity_id in &standing {
            let Some(member) = players.members().iter().find(|member| member.entity_id == entity_id) else {
                continue;
            };
            let at = &member.position;
            let min = (at.x - HALF_WIDTH, at.y, at.z - HALF_WIDTH);
            let max = (at.x + HALF_WIDTH, at.y + HEIGHT, at.z + HALF_WIDTH);

            if let Some(distance) = redstone::shove_distance(shove, min, max) {
                // Упирается в стену: дальше неё сервер игрока не двигает.
                let distance = redstone::free_run(min, max, shove.dir, distance, &walls);

                if distance <= 0.0 {
                    continue;
                }

                players.shove(entity_id, (shove.dir.0 * distance, shove.dir.1 * distance, shove.dir.2 * distance));
            }
        }
    }
}

/// Где стоят вагонетки этого мира и что покажет сравнитель за каждой — для
/// детекторных рельс, в порядке появления.
fn cart_marks(shared: &Shared, dimension: Dimension) -> Vec<redstone::Cart> {
    shared
        .items_in(dimension)
        .lock()
        .expect("предметы захвачены другим потоком")
        .carts()
        .iter()
        .map(|cart| redstone::Cart { x: cart.x, y: cart.y, z: cart.z, reading: cart.reading() })
        .collect()
}

/// Где висят рамки этого мира и что они показывают сравнителю.
fn frame_marks(shared: &Shared, dimension: Dimension) -> Vec<redstone::FrameMark> {
    shared
        .items_in(dimension)
        .lock()
        .expect("предметы захвачены другим потоком")
        .frames()
        .iter()
        .map(|frame| redstone::FrameMark { pos: frame.pos, facing: frame.facing, power: frame.comparator_power() })
        .collect()
}

/// Такт рамок. Прочитанные вместе с чанками заводятся сущностями (если
/// такой рамки ещё нет: чанк мог выгрузиться и вернуться). Каждая рамка
/// живёт на такт дольше, и на каждом сотом такте жизни проверяет опору: без
/// неё она ломается — уходит из мира, звучит поломка, выпадают она сама и
/// её предмет (чёрный ящик, item-frame-facing и item-frame-break).
fn step_frames(shared: &Shared, dimension: Dimension, world: &mut World) {
    let loaded = world.take_loaded_frames();

    let mut items = shared.items_in(dimension).lock().expect("предметы захвачены другим потоком");

    for mut frame in loaded {
        if items.frames().iter().any(|there| (there.pos, there.facing) == (frame.pos, frame.facing)) {
            continue;
        }

        frame.entity_id = shared.next_entity_id();
        frame.uuid = crate::frame::Frame::new(frame.entity_id, frame.pos, frame.facing, frame.glow).uuid;
        items.add_frame(frame);
    }

    if items.frames().is_empty() {
        return;
    }

    let mut falling: Vec<i32> = Vec::new();

    for frame in items.frames_mut() {
        // Возраст растёт после проверки: в такте появления он нулевой, и
        // проверка приходится на сотый такт после него, а не на 99-й.
        let loaded = world.has_chunk(frame.pos.0.div_euclid(16), frame.pos.2.div_euclid(16));

        if loaded && frame.age > 0 && frame.age % crate::frame::SURVIVAL_CHECK == 0 && !frame.supported(&|x, y, z| world.get_block(x, y, z)) {
            falling.push(frame.entity_id);
        }

        frame.age += 1;
    }

    if falling.is_empty() {
        return;
    }

    let broken = items.remove_frames(|frame| falling.contains(&frame.entity_id));

    for frame in &broken {
        crate::frame::break_frame(world, &mut items, shared, frame, true);
    }

    drop(items);
    redstone::note_frames(world, frame_marks(shared, dimension));

    for frame in &broken {
        redstone::comparator_update(world, frame.pos);
    }
}

/// Мир глазами мобов (mob::View).
pub struct MobView<'a>(pub &'a World);

impl crate::mob::View for MobView<'_> {
    fn catalog(&self) -> crate::gameplay_catalog::Catalog { self.0.catalog() }
    fn biome(&self, x: i32, y: i32, z: i32) -> &'static str { self.0.mob_biome(x,y,z) }
    fn griefing(&self) -> bool { self.0.wildlife_griefing }
    fn raining(&self) -> bool { !self.0.is_nether() && !self.0.is_end() && self.0.weather.is_raining() }
    fn always_day(&self) -> bool { self.0.is_nether() || self.0.is_end() }

    fn piglin_safe(&self) -> bool { self.0.is_nether() }
    fn clouds(&self) -> bool { !self.0.is_nether() && !self.0.is_end() }
    fn block(&self, x: i32, y: i32, z: i32) -> i32 {
        self.0.get_block(x, y, z)
    }

    fn loaded(&self, chunk_x: i32, chunk_z: i32) -> bool {
        self.0.has_chunk(chunk_x, chunk_z)
    }

    fn open_sky(&self, x: i32, y: i32, z: i32) -> bool {
        self.0.open_sky(x, y, z)
    }

    fn rain(&self, x: i32, y: i32, z: i32) -> bool {
        self.0.rain_at(x, y, z)
    }

    fn thunder(&self) -> bool { self.0.weather.is_thundering() }

    fn temperature(&self, x: i32, y: i32, z: i32) -> f32 {
        self.0.temperature_at(x, y, z)
    }

    fn light(&self, x: i32, y: i32, z: i32, sky: i32) -> (u8, u8) {
        match self.0.light_cached(x, y, z) {
            Some((s, b)) => ((i32::from(s) - (15 - sky).max(0)).max(0) as u8, b),
            None => (if self.0.open_sky(x, y, z) { sky as u8 } else { 0 }, 0),
        }
    }
}

/// Игроки, которых мобы замечают: где стоят, что держат в руке.
fn watchers(shared: &Shared, dimension: Dimension) -> Vec<crate::mob::ai::Watcher> {
    /// Режим наблюдателя.
    const SPECTATOR: i32 = 3;

    let riders: Vec<i32> = shared.riding.lock().expect("седоки").vehicles().flat_map(|(_,ids)| ids.iter().copied()).collect();
    let players = shared.players.lock().expect("список игроков захвачен другим потоком");

    players
        .members()
        .iter()
        .filter(|member| member.dimension == dimension)
        .map(|member| crate::mob::ai::Watcher {
            id: member.entity_id,
            targetable: (member.game_mode == 0 || member.game_mode == 2) && players.gear(&member.uuid).health > 0.0,
            x: member.position.x,
            y: member.position.y,
            z: member.position.z,
            eye: if member.sneaking { 1.27 } else { 1.62 },
            holding: [member.held.and_then(|stack| blocks::item_name(stack.item)), None],
            spectator: member.game_mode == SPECTATOR,
            look: (member.position.yaw, member.position.pitch),
            // Тыква на голове (вики, Enderman: «carved pumpkin»).
            pumpkin: players.gear(&member.uuid).worn[1].and_then(|stack| blocks::item_name(stack.item)) == Some("carved_pumpkin"),
            // Золотая броня усмиряет пиглинов (вики, Piglin); worn[1..] — шлем, нагрудник, штаны, ботинки.
            sprinting: players.gear(&member.uuid).sprinting,
            riding: riders.contains(&member.entity_id),
            gold_armor: players.gear(&member.uuid).worn[1..].iter().flatten().any(|stack| matches!(blocks::item_name(stack.item), Some("golden_helmet" | "golden_chestplate" | "golden_leggings" | "golden_boots"))),
        })
        .collect()
}

/// Записывает мобов в их чанки — в файл они лягут вместе с чанком.
pub fn store_mobs(shared: &Shared, dimension: Dimension, world: &mut World) {
    let items = shared.items_in(dimension).lock().expect("предметы захвачены другим потоком");

    world.store_mobs(items.mobs());
}

/// Снаряды одного выстрела моба. Снимок оружия получен до его износа,
/// поэтому даже последний залп сломанного арбалета сохраняет чары.
fn mob_projectiles(fired: &crate::mob::Fired, dimension: Dimension, difficulty: u8, rng: &mut crate::mob::Rng, next_id: &mut dyn FnMut() -> i32) -> Vec<crate::arrow::Arrow> {
    let crossbow = fired.weapon.filter(|stack| blocks::item_name(stack.item) == Some("crossbow"));
    let data = crossbow.map(|stack| stack.extras()).unwrap_or_default();
    let offsets: &[f32] = if fired.shot.crossbow && data.level("multishot") > 0 {
        &[0.0, crate::combat::MULTISHOT_ANGLE, -crate::combat::MULTISHOT_ANGLE]
    } else { &[0.0] };
    let mut arrows = Vec::with_capacity(offsets.len());
    for offset in offsets {
        let mut gauss = || {
            let (a, b) = (rng.unit().max(1.0e-12), rng.unit());
            (-2.0 * a.ln()).sqrt() * (std::f64::consts::TAU * b).cos()
        };
        let angle = f64::from(*offset).to_radians();
        let (dx, dz) = (fired.shot.aim.0 - fired.shot.from.0, fired.shot.aim.2 - fired.shot.from.2);
        let aim = if *offset == 0.0 { fired.shot.aim } else {
            (fired.shot.from.0 + dx * angle.cos() - dz * angle.sin(), fired.shot.aim.1,
                fired.shot.from.2 + dx * angle.sin() + dz * angle.cos())
        };
        let velocity = crate::arrow::launch(fired.shot.from, aim, fired.shot.speed, crate::mob::hostile::arrow_error(difficulty), &mut gauss);
        let killer = crate::mob::hostile::killer_key(fired.kind);
        let mut arrow = crate::arrow::Arrow::from_mob(next_id(), dimension, fired.shot.from, velocity, fired.shooter, killer);
        arrow.effects.extend(crate::mob::skeleton::arrow_effect(fired.kind));
        if !fired.shot.crossbow && let Some(bow) = fired.weapon { arrow.enchant_from_bow(&bow.extras()); }
        arrow.crossbow = fired.shot.crossbow;
        if fired.shot.crossbow { arrow.pierce_left = data.level("piercing").max(0); }
        if fired.shot.trident { arrow.trident = blocks::item_named("trident").map(|item| crate::inventory::Stack::new(item, 1)); }
        arrows.push(arrow);
    }
    arrows
}

/// Такт мобов. Прочитанные вместе с чанками заводятся в мир; в новых чанках
/// Обычного мира селятся животные (если не выключено `gamerule spawn_mobs`),
/// а в темноте — враждебные; дальше — мысли, движение, разведение, добыча
/// погибших, удары, стрелы и взрывы крипера, а то, что мобы делают с блоками
/// (овца съела траву) и предметами (курица снесла яйцо), исполняется здесь.
/// Раз в 200 тактов моб записывается в свой чанк.
fn step_mobs(shared: &Shared, dimension: Dimension, world: &mut World, rng: &mut crate::mob::Rng, ticks: u64) {
    let loaded = world.take_loaded_mobs();
    crate::archaeology::decay(world,*shared.ticks.borrow());
    let fresh = world.take_new_chunks();
    if dimension==Dimension::Overworld {
        let mut maps=shared.maps.lock().expect("карты");
        for &(cx,cz) in &fresh {
            for (x,y,z,_) in world.chunk_containers(cx,cz) {
                let (x,z)=(cx*16+x,cz*16+z);
                let Some(mut slots)=world.container(x,y,z).map(|c|c.slots.clone()) else {continue;};
                let mut changed=false;
                for stack in slots.iter_mut().flatten() { changed |= crate::maps::resolve_explorer(stack,&mut maps,world,(x,z)); }
                if changed {world.with_container(x,y,z,|c|c.slots=slots);}
            }
        }
    }

    // Список игроков берётся до замка предметов: два замка разом не держим.
    let watchers = watchers(shared, dimension);
    // Хозяева приручённых и держатели поводков — по UUID (mob/tame.rs).
    let owners: Vec<([u8; 16], i32)> = {
        let players = shared.players.lock().expect("список игроков захвачен другим потоком");

        players.members().iter().filter(|member| member.dimension == dimension).map(|member| (member.uuid, member.entity_id)).collect()
    };
    let difficulty = shared.difficulty.load(std::sync::atomic::Ordering::Relaxed);
    let time = world.time_of_day();
    let nether = world.is_nether();
    // В Энде ни неба, ни солнца: нежить не горит, свет неба нулевой.
    let skyless = nether || world.is_end();
    let env = crate::mob::Env {
        difficulty,
        sky: if skyless { 0 } else { world.weather.dim_sky(redstone::ambient_sky(time)) },
        // Гроза гасит небо так, что дня нет (у оригинала день — пока
        // небо темнее не больше чем на 3): нежить не горит [проверить].
        sun_up: !skyless && !(12_540..23_460).contains(&time) && world.weather.dim_sky(15) > 11,
        clock: world.clock(),
        tick: world.tick() as i64,
    };

    // Свет для мобов, которым он нужен (паук, появление): считается здесь,
    // пока мир в руках целиком, и ложится в запоминание мира.
    let dark_spots: Vec<(i32, i32, i32)> = {
        let items = shared.items_in(dimension).lock().expect("предметы захвачены другим потоком");

        items.mobs().iter().filter(|mob| mob.kind == crate::mob::Kind::Spider).map(|mob| mob.cell()).collect()
    };

    for (x, y, z) in dark_spots {
        world.light_full(x, y, z);
    }

    // Бой Края — до хода мобов: кристаллы и дракон появляются в этом же
    // такте, полоса босса уходит и когда мобов ещё нет.
    if dimension == Dimension::End {
        crate::dragon_fight::tick(shared, world, watchers.len(), ticks);
    }

    let spawning = dimension == Dimension::Overworld && shared.spawn_mobs.load(std::sync::atomic::Ordering::Relaxed);
    // Враждебные в темноте — в Обычном мире и в Энде (там одни эндермены:
    // вики, Enderman, «Spawning»).
    let hostile_spawning = matches!(dimension, Dimension::Overworld | Dimension::End | Dimension::Nether)
        && shared.spawn_mobs.load(std::sync::atomic::Ordering::Relaxed)
        && shared.spawn_monsters.load(std::sync::atomic::Ordering::Relaxed);
    let kinds = if dimension == Dimension::End { &crate::mob::spawn::END_HOSTILE[..] } else { &crate::mob::spawn::HOSTILE[..] };
    let mut natural: Vec<crate::mob::Mob> = Vec::new();

    if hostile_spawning && difficulty > crate::survival::PEACEFUL && ticks.is_multiple_of(8) && !watchers.is_empty() {
        let players: Vec<(f64, f64, f64)> = watchers.iter().filter(|player| !player.spectator).map(|player| (player.x, player.y, player.z)).collect();
        let crowd = {
            let items = shared.items_in(dimension).lock().expect("предметы захвачены другим потоком");

            items.mobs().iter().filter(|mob| mob.kind.hostile() && !mob.is_dead()).count()
        };

        if crowd < crate::mob::spawn::MONSTER_CAP {
            let ambient = if skyless { 0 } else { world.weather.spawn_sky(redstone::ambient_sky(time)) };
            let surface = |x: i32, z: i32| {
                let mut y = crate::world::MIN_Y + crate::world::WORLD_HEIGHT - 1;

                while y > crate::world::MIN_Y && !blocks::stops_items(world.get_block(x, y, z)) {
                    y -= 1;
                }

                y + 1
            };
            // Незер: виды по биому, а на кирпиче крепости — свои (вики, Nether
            // Fortress); нужен только блочный свет не выше 11 (вики, Mob spawning).
            natural = if nether {
                let kinds_at = |x: i32, y: i32, z: i32| {
                    let on_bricks = blocks::block_at_state(world.get_block(x, y - 1, z)) == Some("nether_bricks");

                    if on_bricks && world.in_fortress(x, y, z) {
                        return crate::mob::spawn::FORTRESS.to_vec();
                    }

                    crate::mob::spawn::nether_kinds(world.surface_biome(x, z)).to_vec()
                };

                crate::mob::spawn::hostile_attempt_in(&MobView(world), &players, &surface, &kinds_at, 48, rng, &mut || shared.next_entity_id())
            } else if dimension == Dimension::End {
                crate::mob::spawn::hostile_attempt(&MobView(world), &players, &surface, kinds, rng, &mut || shared.next_entity_id())
            } else {
                // Обычный мир: пустыня даёт кадавров, реки и океаны — утопленников.
                let kinds_at = |x: i32, y: i32, z: i32| {
                    let wet = crate::mob::is_water(world.get_block(x, y, z));

                    // В воде в рамке подводной крепости — стражи (вики, Guardian,
                    // «Spawning»: «only in ocean monuments»).
                    if wet && world.in_monument(x, y, z) {
                        return crate::mob::spawn::MONUMENT.to_vec();
                    }

                    // В рамке хижины ведьмы — одни ведьмы (вики, Swamp Hut:
                    // «witches can spawn at any block within the 7×7×9 area»).
                    if !wet && world.in_swamp_hut(x, y, z) {
                        return vec![(crate::mob::Kind::Witch, 1)];
                    }

                    // В древнем городе мобы сами не появляются (вики, Ancient
                    // City: «Mobs cannot spawn naturally»).
                    if world.in_ancient_city(x, y, z) {
                        return Vec::new();
                    }

                    // В области аванпоста — одни разбойники (вики, Pillager
                    // Outpost: «72×58×72 area centered on the top level of
                    // the watchtower»).
                    if !wet && world.in_outpost(x, y, z) {
                        return vec![(crate::mob::Kind::Pillager, 1)];
                    }

                    crate::mob::spawn::overworld_kinds(world.surface_biome(x, z), wet)
                };

                crate::mob::spawn::hostile_attempt_in(&MobView(world), &players, &surface, &kinds_at, 12, rng, &mut || shared.next_entity_id())
            };

            // Свет каждой найденной точки: небесный с поправкой на час и
            // блочный; в темноте (блочный 0, небесный ≤ случайного 0…7)
            // моб остаётся [проверить: случайное берётся после выбора места].
            natural.retain(|mob| {
                let (x, y, z) = mob.cell();
                let (sky, block) = world.light_full(x, y, z);
                let sky = (i32::from(sky) - (15 - ambient).max(0)).max(0) as u8;

                if mob.kind == crate::mob::Kind::Guardian {
                    // Стражу свет не нужен; под открытым небом — 1 к 20 (вики,
                    // Guardian: «spawning fails 95% of the time») [проверить].
                    !world.open_sky(x, y, z) || rng.below(20) == 0
                } else if mob.kind == crate::mob::Kind::Slime {
                    crate::mob::slime::can_spawn(world.seed(), (x, y, z), world.mob_biome(x, y, z), sky.max(block), world.clock(), rng)
                } else if mob.kind == crate::mob::Kind::Pillager && world.in_outpost(x, y, z) {
                    // Разбойникам аванпоста небо не мешает: только блочный
                    // свет не выше 8 (вики, Pillager: «if the block light
                    // level is less than or equal to 8»).
                    block <= 8
                } else if dimension == Dimension::Overworld && matches!(world.mob_biome(x, y, z), "mushroom_fields" | "deep_dark") {
                    false
                } else if nether { block <= 11 } else { crate::mob::spawn::dark_enough(sky, block, rng) }
            });
        }
    }

    let mut items = shared.items_in(dimension).lock().expect("предметы захвачены другим потоком");

    let targets: Vec<_> = watchers.iter().map(|p| crate::tnt::Target { entity_id: p.id, x: p.x, y: p.y, z: p.z, sneaking: false }).collect();
    crate::effect_reactions::mobs(world, &mut items, shared.mob_griefing.load(std::sync::atomic::Ordering::Relaxed), &targets, &mut || shared.next_entity_id(), rng);

    for mut mob in loaded {
        mob.entity_id = shared.next_entity_id();
        mob.uuid = mob.land.saved_uuid.unwrap_or_else(|| crate::mob::uuid_for(mob.entity_id));
        mob.brain = crate::mob::ai::Brain::new(mob.entity_id);
        items.add_mob(mob);
    }

    for mut mob in natural {
        // Разбойник аванпоста — с долей 6 % капитан со зловещим флагом (вики,
        // Pillager Outpost: «a 6% chance for a pillager to spawn as a patrol
        // leader») [проверить: доля у появления в аванпосте].
        if mob.kind == crate::mob::Kind::Pillager && rng.below(100) < 6 {
            let (x, y, z) = mob.cell();

            mob.illager.captain = world.in_outpost(x, y, z);
        }

        items.add_mob(mob);
    }

    if spawning && ticks.is_multiple_of(20) {
        for mob in crate::mob::water::spawn::attempt(world,&watchers,items.mobs(),rng,&mut || shared.next_entity_id()) { items.add_mob(mob); }
    }

    step_spawners(shared, world, &mut items, &watchers, difficulty, shared.spawn_mobs.load(std::sync::atomic::Ordering::Relaxed), skyless, time, rng);

    // Испытательные спавнеры и хранилища (trial.rs).
    step_trials(shared, world, &mut items, dimension, difficulty, rng);

    // Древние стражи подводных крепостей: при сложении чанка, по три на
    // крепость (вики, Elder Guardian: «spawned with the ocean monument»;
    // замер, monument.rs), не исчезают.
    if dimension == Dimension::Overworld {
        for (chunk_x, chunk_z) in &fresh {
            for at in world.monument_elders(*chunk_x, *chunk_z) {
                let mut elder = crate::mob::Mob::new(shared.next_entity_id(), crate::mob::Kind::ElderGuardian, at);

                elder.persistent = true;
                elder.persistent_tag = true;
                items.add_mob(elder);
            }

            // Утопленники руин в океане: при сложении чанка, на местах замера
            // (ocean_ruin.rs; вики, Ocean Ruins: «many of the structures spawn
            // with drowned … persistent»).
            for at in world.ruin_drowned(*chunk_x, *chunk_z) {
                let mut drowned = crate::mob::Mob::new(shared.next_entity_id(), crate::mob::Kind::Drowned, at);

                drowned.persistent = true;
                drowned.persistent_tag = true;
                items.add_mob(drowned);
            }
        }
    }

    // Вагонетки с сундуком заброшенных шахт: при сложении чанка, на рельсе
    // (вики, Mineshaft: «Any chest found in a mineshaft always occurs as a
    // minecart with chest that rests on top of some rails»; замер,
    // mineshaft.rs), добыча — abandoned_mineshaft. Рельсы нет (кусок не лёг
    // у воды) — нет и вагонетки.
    if dimension == Dimension::Overworld {
        for (chunk_x, chunk_z) in &fresh {
            for ((x, y, z), luck) in world.mineshaft_carts(*chunk_x, *chunk_z) {
                let Some(place) = crate::minecart::place_on_rail(world.get_block(x, y, z), (x, y, z)) else {
                    continue;
                };
                let mut cart = crate::minecart::Minecart::new(shared.next_entity_id(), crate::minecart::Kind::Chest, place);

                cart.slots = crate::loot::fill(crate::loot::Table::Mineshaft.pools(), cart.slots.len(), &mut crate::loot::Dice::new(luck));
                items.add_cart(cart);
            }
        }
    }

    // Хижины ведьм: при сложении чанка — ведьма и чёрная кошка (вики, Swamp
    // Hut, «Mobs»: «one witch and one stray black cat spawn inside the hut.
    // These never naturally despawn»; ведьмы нет на мирной сложности).
    if dimension == Dimension::Overworld {
        for (chunk_x, chunk_z) in &fresh {
            for at in world.hut_dwellers(*chunk_x, *chunk_z) {
                if difficulty > crate::survival::PEACEFUL {
                    let mut witch = crate::mob::Mob::new(shared.next_entity_id(), crate::mob::Kind::Witch, at);

                    witch.persistent = true;
                    witch.persistent_tag = true;
                    if crate::mob::spawn::fits(&MobView(world), &witch) { items.add_mob(witch); }
                }

                let mut cat = crate::mob::Mob::new(shared.next_entity_id(), crate::mob::Kind::Cat, at);

                cat.tame.variant = crate::mob::tame::variant_named(crate::mob::Kind::Cat, "all_black").unwrap_or(0);
                cat.persistent = true;
                cat.persistent_tag = true;
                if crate::mob::spawn::fits(&MobView(world), &cat) { items.add_mob(cat); }
            }
        }
    }

    // Клетки аванпостов: железный голем (вики, Pillager Outpost: «An iron
    // golem has a 85% chance of spawning»; замер entities), не исчезает.
    // Аллаи появляются с клетками один раз, в том числе на мирной сложности.
    if dimension == Dimension::Overworld {
        for (chunk_x, chunk_z) in &fresh {
            for at in world.outpost_allays(*chunk_x, *chunk_z) {
                let mut allay=crate::mob::Mob::new(shared.next_entity_id(),crate::mob::Kind::Allay,at);
                allay.persistent=true; allay.persistent_tag=true;
                if crate::mob::spawn::fits(&MobView(world), &allay) { items.add_mob(allay); }
            }
            for at in world.outpost_golems(*chunk_x, *chunk_z) {
                let mut golem = crate::mob::Mob::new(shared.next_entity_id(), crate::mob::Kind::IronGolem, at);

                golem.persistent = true;
                golem.persistent_tag = true;
                if crate::mob::spawn::fits(&MobView(world), &golem) { items.add_mob(golem); }
            }
        }
    }

    // Лесные особняки: поборники и вызыватели при сложении чанка, на местах
    // замера (mansion.rs; вики, Woodland Mansion: «Once all the vindicators
    // and evokers are killed … they do not respawn»), не исчезают; на мирной
    // сложности их нет.
    if dimension == Dimension::Overworld {
        for (chunk_x, chunk_z) in &fresh {
            for (at, kind) in world.mansion_dwellers(*chunk_x, *chunk_z) {
                let kind = match kind { "allay"=>crate::mob::Kind::Allay,"evoker"=>crate::mob::Kind::Evoker,_=>crate::mob::Kind::Vindicator };
                if kind.hostile() && difficulty==crate::survival::PEACEFUL {continue;}
                let mut illager = crate::mob::Mob::new(shared.next_entity_id(), kind, at);

                illager.persistent = true;
                illager.persistent_tag = true;
                if crate::mob::spawn::fits(&MobView(world), &illager) { items.add_mob(illager); }
            }
        }
    }

    // Подвалы иглу: житель и зомби-житель (вики, Igloo; замер entities),
    // не исчезают.
    if dimension == Dimension::Overworld {
        for (chunk_x, chunk_z) in &fresh {
            for (at, zombie) in world.igloo_dwellers(*chunk_x, *chunk_z) {
                let kind = if zombie { crate::mob::Kind::ZombieVillager } else { crate::mob::Kind::Villager };
                let mut dweller = crate::mob::Mob::new(shared.next_entity_id(), kind, at);

                dweller.persistent = true;
                dweller.persistent_tag = true;
                if crate::mob::spawn::fits(&MobView(world), &dweller) { items.add_mob(dweller); }
            }
        }
    }

    // Шалкеры энд-городов: при сложении чанка, на местах стражей
    // (end_city::sentries) — к соседней твёрдой грани.
    if dimension == Dimension::End {
        for (chunk_x, chunk_z) in &fresh {
            for (x, y, z) in world.city_sentries(*chunk_x, *chunk_z) {
                let mut shulker = crate::mob::Mob::new(shared.next_entity_id(), crate::mob::Kind::Shulker, (f64::from(x) + 0.5, f64::from(y), f64::from(z) + 0.5));

                shulker.h.face = crate::mob::hostile::attach_face(&MobView(world), (x, y, z), rng).unwrap_or(0);
                if crate::mob::spawn::fits(&MobView(world), &shulker) { items.add_mob(shulker); }
            }
        }
    }

    if spawning && !world.is_flat() {
        for (chunk_x, chunk_z) in fresh {
            let biome = world.surface_biome(chunk_x * 16 + 8, chunk_z * 16 + 8);
            let animals = if crate::mob::spawn::land_table(biome).is_empty() {
                crate::mob::spawn::for_new_chunk_in(&MobView(world), chunk_x, chunk_z, biome, rng, &mut || shared.next_entity_id())
            } else { crate::mob::spawn::land_for_new_chunk(&MobView(world), chunk_x, chunk_z, biome, rng, &mut || shared.next_entity_id()) };
            for mob in animals {
                items.add_mob(mob);
            }

            // Волки в лесах и тайгах (mob/spawn.rs).
            let biome = world.surface_biome(chunk_x * 16 + 8, chunk_z * 16 + 8);

            for wolf in crate::mob::spawn::wolves_for_new_chunk(&MobView(world), chunk_x, chunk_z, biome, rng, &mut || shared.next_entity_id()) {
                items.add_mob(wolf);
            }
        }
    }

    if spawning && world.is_flat() && ticks.is_multiple_of(crate::mob::spawn::flat::INTERVAL) {
        let sky = world.weather.spawn_sky(redstone::ambient_sky(time));
        for mob in crate::mob::spawn::flat::cycle(&MobView(world), &watchers, items.mobs(), world.spawn_position(), sky, rng, &mut || shared.next_entity_id()) {
            items.add_mob(mob);
        }
    }

    // Фантомы: попытка раз в 1200–2400 тактов на мир (mob/phantom.rs).
    if dimension == Dimension::Overworld && spawning && shared.spawn_monsters.load(std::sync::atomic::Ordering::Relaxed) && shared.spawn_phantoms.load(std::sync::atomic::Ordering::Relaxed) && difficulty > crate::survival::PEACEFUL {
        let wait = shared.phantom_wait.fetch_sub(1, std::sync::atomic::Ordering::Relaxed) - 1;

        if wait <= 0 {
            shared.phantom_wait.store(crate::mob::phantom::WAIT_MIN + rng.below(crate::mob::phantom::WAIT_SPREAD) as i32, std::sync::atomic::Ordering::Relaxed);

            // Ночь или гроза: небо темнее хотя бы на 5 (вики, Phantom).
            let ambient = world.weather.spawn_sky(redstone::ambient_sky(time));

            if 15 - ambient >= 5 {
                let spots: Vec<crate::mob::phantom::Spot> = {
                    let players = shared.players.lock().expect("список игроков захвачен другим потоком");

                    players
                        .members()
                        .iter()
                        .filter(|member| member.dimension == dimension && member.game_mode != 3)
                        .map(|member| crate::mob::phantom::Spot { x: member.position.x, y: member.position.y, z: member.position.z, rest: players.rest(&member.uuid) })
                        .collect()
                };
                let day = world.clock() / 24_000;
                let regional = crate::mob::phantom::regional(difficulty, world.clock(), 0, crate::mob::phantom::moon_brightness(day));

                for spot in spots {
                    if let Some(at) = crate::mob::phantom::try_spot(&MobView(world), &spot, regional, rng) {
                        for _ in 0..crate::mob::phantom::group_size(difficulty, rng) {
                            items.add_mob(crate::mob::phantom::spawn(shared.next_entity_id(), at));
                        }
                    }
                }
            }
        }
    }

    // Набеги и патрули (raid.rs): места работы жителей — «деревня».
    if dimension == Dimension::Overworld {
        let pois: Vec<(i32, i32, i32)> = items.mobs().iter().filter(|mob| mob.kind == crate::mob::Kind::Villager && !mob.is_dead()).filter_map(|mob| mob.villager.job_site).collect();
        let rules = crate::raid::Rules {
            difficulty,
            raids: shared.raids.load(std::sync::atomic::Ordering::Relaxed),
            patrols: spawning && shared.spawn_patrols.load(std::sync::atomic::Ordering::Relaxed),
            day: world.clock() / 24_000,
        };
        let mut raids = std::mem::take(&mut world.raids);
        let outcome = crate::raid::tick(&mut raids, items.mobs_mut(), &MobView(world), &watchers, &pois, rules, rng, &mut || shared.next_entity_id());

        world.raids = raids;

        for (viewer, body) in outcome.private {
            world.note_private(viewer, body);
        }

        for (player, effect) in outcome.effects {
            world.note_mob_effect(player, effect);
        }

        for (mob, effect) in outcome.mob_effects {
            if let Some(mob) = items.mobs_mut().iter_mut().find(|known| known.entity_id == mob) {
                mob.vitals.give_effect_shown(effect);
            }
        }

        for (x, y, z) in outcome.resonate {
            world.note_sound(x, y, z, "minecraft:block.bell.resonate", 1.0, 1.0);
        }

        for mob in outcome.born {
            items.add_mob(mob);
        }

        // Разоритель, упёршись в листву или посевы, ломает их (вики, Ravager:
        // «all leaves … beetroots, carrots, potatoes, wheat, pitcher crop,
        // torchflower crop»; не при mob_griefing=false).
        if shared.mob_griefing.load(std::sync::atomic::Ordering::Relaxed) {
            let ravagers: Vec<(f64, f64, f64)> = items.mobs().iter().filter(|mob| mob.kind == crate::mob::Kind::Ravager && mob.wall && !mob.is_dead() && !mob.no_ai).map(|mob| (mob.x, mob.y, mob.z)).collect();

            for (x, y, z) in ravagers {
                for (bx, by, bz) in crate::mob::illager::ravager_reach_cells((x, y, z)) {
                    let state = world.get_block(bx, by, bz);

                    if blocks::block_at_state(state).is_some_and(crate::mob::illager::ravager_breaks) {
                        world.note_destroyed(bx, by, bz);
                        world.set_block(bx, by, bz, blocks::state_by_name("air").expect("воздух"));
                        world.note_level_event(bx, by, bz, 2001, state);
                    }
                }
            }
        }
    }

    if dimension == Dimension::Overworld && shared.trader_spawner.lock().expect("торговец").tick(shared.spawn_traders.load(std::sync::atomic::Ordering::Relaxed), spawning, watchers.iter().any(|p| !p.spectator), rng) {
        let eligible: Vec<_>=watchers.iter().filter(|p| !p.spectator).collect();
        let player=eligible[rng.below(eligible.len() as u32) as usize];
        for _ in 0..10 {
            let x=player.x.floor() as i32 + rng.below(96) as i32-48;
            let z=player.z.floor() as i32 + rng.below(96) as i32-48;
            if !world.has_chunk(x.div_euclid(16),z.div_euclid(16)) || world.surface_biome(x,z)=="deep_dark" { continue; }
            let y=world.rain_top(x,z).filter(|y| crate::mob::spawn::spawnable(&MobView(world),x,*y,z,3));
            let Some(y)=y else { continue; };
            let mut trader=crate::mob::Mob::new(shared.next_entity_id(),crate::mob::Kind::WanderingTrader,(f64::from(x)+0.5,f64::from(y),f64::from(z)+0.5));
            if !crate::mob::spawn::fits(&MobView(world),&trader) { continue; }
            trader.land.despawn=48000; trader.villager.offers=crate::mob::trader::offers(rng);
            let (id,uuid)=(trader.entity_id,trader.uuid);
            let mut llamas=Vec::new();
            for _ in 0..2 {
                for _ in 0..10 {
                    let lx=x+rng.below(8) as i32-4; let lz=z+rng.below(8) as i32-4;
                    let Some(ly)=world.rain_top(lx,lz).filter(|y| crate::mob::spawn::spawnable(&MobView(world),lx,*y,lz,2)) else { continue; };
                    let mut llama=crate::mob::Mob::new(shared.next_entity_id(),crate::mob::Kind::TraderLlama,(f64::from(lx)+0.5,f64::from(ly),f64::from(lz)+0.5));
                    crate::mob::land::initialize(&mut llama,rng); llama.land.trader=Some(id);
                    if crate::mob::spawn::fits(&MobView(world),&llama) {
                        crate::mob::leash::attach(&mut llama,crate::mob::leash::Holder::Player { id,uuid }); llamas.push(llama); break;
                    }
                }
            }
            if llamas.len()!=2 { continue; }
            items.add_mob(trader); for llama in llamas { items.add_mob(llama); }
            shared.trader_spawner.lock().expect("торговец").spawned(); break;
        }
    }
    sculk_deliver(world, &mut items, rng, &mut || shared.next_entity_id());
    world.wildlife_griefing = shared.mob_griefing.load(std::sync::atomic::Ordering::Relaxed);
    crate::hive::tick(world, &mut items, &mut || shared.next_entity_id());
    crate::mob::newblocks::blocks_tick(world, &mut items, &watchers, spawning && difficulty > 0, rng, &mut || shared.next_entity_id());

    // Кошки, что сидят: сундук под ними не открывается (mob/tame.rs).
    world.sitting_cats = items.mobs().iter().filter(|mob| mob.kind == crate::mob::Kind::Cat && mob.tame.sitting && !mob.is_dead()).map(|mob| (mob.x, mob.y, mob.z)).collect();

    if dimension == Dimension::Overworld && spawning && ticks.is_multiple_of(20) {
        crate::mob::small_world::spawn_bats(&mut items,world,&watchers,shared,rng);
    }
    crate::mob::tame::bind_owners(items.mobs_mut(), &owners);
    crate::mob::small_world::step(&mut items, world, &watchers, shared, dimension, rng);

    if items.mobs().is_empty() {
        // Слежению есть о чём рассказать, пока у клиентов остались показанные
        // мобы — или пока в мире есть шарики опыта: их движение рассылает то
        // же слежение (без мобов рядом шарики замирали, xp-orb-far).
        if !items.mob_view().is_empty() || !items.orbs().is_empty() {
            world.note_tracker();
        }

        items.snapshot_mobs();
        return;
    }

    // Слежение рассылает клиентам мобов такими, какими они стоят до своего
    // хода (чёрный ящик, cow-physics).
    items.snapshot_mobs();
    world.note_tracker();

    crate::mob::water::eggs::mob_steps(world,items.mobs_mut(),shared.mob_griefing.load(std::sync::atomic::Ordering::Relaxed));
    let boats: Vec<_> = items.carts().iter().filter(|cart| matches!(cart.kind,crate::minecart::Kind::Boat(_)) && cart.vx*cart.vx+cart.vz*cart.vz > 0.0001)
        .map(|cart|(cart.x,cart.y-0.5,cart.z)).collect();
    for mob in items.mobs_mut().iter_mut().filter(|mob| mob.kind==crate::mob::Kind::Dolphin && mob.water.anger==0) {
        mob.water.boat=boats.iter().find(|at| (at.0-mob.x).powi(2)+(at.1-mob.y).powi(2)+(at.2-mob.z).powi(2)<100.0).copied();
    }
    crate::mob::tame::bind_owners(items.mobs_mut(), &owners);
    crate::mob::wildlife::bind_trust(items.mobs_mut(), &owners);

    crate::mob::newblocks::copper_tick(world, &mut items, rng);
    {
        let riding = shared.riding.lock().expect("учёт седоков");
        for mob in items.mobs_mut().iter_mut().filter(|m| m.kind == crate::mob::Kind::HappyGhast) {
            let ridden = !riding.passengers(mob.entity_id).is_empty();
            if mob.newmobs.ridden != ridden {
                if !ridden { mob.newmobs.still = 10; }
                mob.events.push(crate::mob::Event::Sound{name:if ridden {"minecraft:entity.happy_ghast.harness_goggles_down"}else{"minecraft:entity.happy_ghast.harness_goggles_up"},volume:1.0,pitch:1.0});
                if !ridden {mob.newmobs.home=Some((mob.x,mob.y,mob.z));}
            }
            mob.newmobs.ridden = ridden;
            // «Стоит смирно» считается и у NoAI (чёрный ящик, happy-ghast-ride: 19=true на такт после схода).
            let near = watchers.iter().any(|p| !p.spectator && (p.x-mob.x).abs() <= 2.0 && (p.z-mob.z).abs() <= 2.0 && p.y >= mob.y+mob.size().1 && p.y < mob.y+mob.size().1+2.0);
            if ridden {mob.newmobs.still=0;} else if near {mob.newmobs.still=10;} else {mob.newmobs.still=(mob.newmobs.still-1).max(0);}
        }
    }
    {
        let riding=shared.riding.lock().expect("седоки");
        for mob in items.mobs_mut().iter_mut().filter(|m| crate::mob::land::rideable(m.kind)) { mob.land.rider=riding.driver(mob.entity_id); }
    }
    let rideables: Vec<_>=items.mobs().iter().filter(|m| crate::mob::land::rideable(m.kind)).map(|m| m.entity_id).collect();
    let mut rider_hurt = {
        let mut riding = shared.riding.lock().expect("учёт седоков");
        crate::mob::horse::traps(&mut items, &mut riding, &watchers, rng, &mut || shared.next_entity_id());
        crate::mob::horse::sync_riding(&mut items, &mut riding, &owners, &MobView(world))
    };
    let riders: Vec<i32> = shared.riding.lock().expect("седоки").vehicles().flat_map(|(_, ids)| ids.iter().copied()).collect();
    crate::mob::movement_enchantments::step(world, &mut items, &watchers, &riders);
    let outcome = crate::mob::step_all(&mut items, &MobView(world), &watchers, env, rng, &mut || shared.next_entity_id());
    rider_hurt.extend(crate::mob::horse::sync_riding(&mut items, &mut shared.riding.lock().expect("учёт седоков"), &owners, &MobView(world)));
    for (id, damage) in rider_hurt {
        world.note_mob_hurt(id, damage, crate::survival::Source::Fall, 0, "", None, (0.0, 0.0, 0.0));
    }

    {
        let mut riding=shared.riding.lock().expect("седоки");
        for id in rideables {
            if items.mobs().iter().find(|m| m.entity_id==id).is_none_or(|m| m.is_dead() || (crate::mob::land::llama(m.kind) && m.land.rider.is_none())) { riding.clear_vehicle(id); }
        }
    }
    crate::mob::arthropod::step(&mut items, world, crate::mob::arthropod::Rules {
        griefing: shared.mob_griefing.load(std::sync::atomic::Ordering::Relaxed),
        drops: shared.block_drops.load(std::sync::atomic::Ordering::Relaxed),
    }, rng, &mut || shared.next_entity_id());

    for (uuid, event) in outcome.progress {
        shared.players.lock().expect("список игроков захвачен другим потоком").order(&uuid, crate::players::Order::Progress(event));
    }
    for hit in outcome.enchant_retaliation {
        let dir = watchers.iter().find(|p| p.id == hit.target).map(|p| {
            let delta = (p.x - hit.at.0, p.z - hit.at.2);
            let length = delta.0.hypot(delta.1).max(1e-9);
            (delta.0 / length, delta.1 / length)
        });
        world.note_mob_hurt(hit.target, hit.damage, crate::survival::Source::Thorns, hit.attacker,
            crate::mob::hostile::killer_key(hit.kind), dir, hit.at);
        if let Some(player) = watchers.iter().find(|p| p.id == hit.target) {
            world.note_sound_player_at((player.x, player.y, player.z), "minecraft:enchant.thorns.hit", 1.0, 1.0);
        }
    }
    for strike in outcome.strikes {
        // Зомби бьёт жителя: убитый превращается в зомби-жителя (вики, Zombie
        // Villager, «Conversion»: лёгкая 0 %, обычная 50 %, трудная 100 %).
        if strike_villager(&mut items, world, &strike, difficulty, rng, &mut || shared.next_entity_id()) {
            continue;
        }

        world.note_enchanted_mob_hurt(
            strike.melee.target,
            strike.melee.damage,
            // Луч стража — магия от существа (mob/guardian.rs).
            if crate::mob::guardian::guardian(strike.kind) { crate::survival::Source::IndirectMagic } else { crate::survival::Source::MobAttack },
            strike.attacker,
            crate::mob::hostile::killer_key(strike.kind),
            Some(strike.melee.push),
            strike.at,
            strike.enchantments,
        );

        // Иссушитель: 10 секунд иссушения (вики, Wither Skeleton).
        if strike.melee.wither {
            let wither = crate::survival::Effect { kind: crate::survival::EffectKind::Wither, amplifier: 0, ticks_left: 200, ambient: false, visible: true, show_icon: true };

            world.note_mob_hit_effect(strike.melee.target, wither);
        }

        // Кадавр: «Голод» (вики, Husk; чёрный ящик, husk-hunger).
        if strike.melee.hunger {
            let ticks = crate::mob::hostile::hunger_ticks(difficulty);

            if ticks > 0 {
                let hunger = crate::survival::Effect { kind: crate::survival::EffectKind::Hunger, amplifier: 0, ticks_left: ticks, ambient: false, visible: true, show_icon: true };

                world.note_mob_hit_effect(strike.melee.target, hunger);
            }
        }
    }

    for (player,attacker,at,puff,push) in outcome.stings {
        if difficulty == crate::survival::PEACEFUL { continue; }
        world.note_mob_hurt(player,f32::from(1+puff),crate::survival::Source::MobAttack,attacker,"entity.minecraft.pufferfish",Some(push),at);
        world.note_mob_hit_effect(player,crate::survival::Effect { kind: crate::survival::EffectKind::Poison,amplifier:0,ticks_left:60*i32::from(puff),ambient:false,visible:true, show_icon:true });
    }
    for (player,effect) in outcome.water_effects { world.note_mob_effect(player,effect); }
    for player in outcome.axolotl_assists {world.note_axolotl_assist(player);}
    for player in outcome.darkness { redstone::sculk_family::darkness(world, player); }
    for crate::mob::warden::Boom { attacker, target, from, to } in outcome.sonic_booms {
        if let Some(mob) = items.mobs_mut().iter_mut().find(|mob| mob.entity_id == target) {
            if mob.hurt(crate::mob::warden::SONIC_DAMAGE, crate::survival::Source::SonicBoom,
                Some(crate::mob::Push { dir: (0.0, 0.0), strength: 0.0, cause: attacker })).is_some() {
                let resistance = if matches!(mob.kind, crate::mob::Kind::IronGolem | crate::mob::Kind::Shulker | crate::mob::Kind::Warden) { 1.0 } else { 0.0 };
                let push = crate::mob::warden::sonic_velocity((to.0 - from.0, to.1 - from.1, to.2 - from.2), resistance);
                mob.vx += push.0; mob.vy += push.1; mob.vz += push.2;
            }
        } else {
            world.note_mob_hurt(target, crate::mob::warden::SONIC_DAMAGE, crate::survival::Source::SonicBoom,
                attacker, "entity.minecraft.warden", None, from);
        }
    }

    // Проклятия древних стражей (mob/guardian.rs).
    for (player, silent) in outcome.curses {
        world.note_elder_curse(player, silent);
    }

    // Превращения зомби: звук события мира (вики, Drowned).
    for ((x, y, z), id) in outcome.level_events {
        world.note_level_event(x, y, z, id, 0);
    }

    for fired in outcome.shots {
        let arrows = mob_projectiles(&fired, dimension, difficulty, rng, &mut || shared.next_entity_id());
        // Замок стрел — после предметов (shared.rs).
        let mut flying = shared.arrows.lock().expect("стрелы захвачены другим потоком");
        for arrow in arrows { flying.spawn(arrow); }
    }

    for (shooter, (from, velocity)) in outcome.spits {
        let flying = crate::thrown::Flying::spit(shared.next_entity_id(), dimension, from, velocity, shooter);
        shared.thrown.lock().expect("снаряды захвачены другим потоком").flying.push(flying);
    }

    // Снежки снежных големов (mob/golem.rs): летят, как брошенные игроком.
    for (shooter, (from, velocity)) in outcome.snowballs {
        let flying = crate::thrown::Flying::snowball(shared.next_entity_id(), dimension, from, velocity, shooter);

        shared.thrown.lock().expect("снаряды захвачены другим потоком").flying.push(flying);
    }

    // След снежного голема: слой снега, если правило mobGriefing позволяет
    // (вики, Snow Golem).
    if shared.mob_griefing.load(std::sync::atomic::Ordering::Relaxed)
        && let Some(snow) = blocks::state_by_name("snow")
    {
        for (x, y, z) in outcome.snow {
            if world.get_block(x, y, z) == crate::world::AIR {
                world.set_block(x, y, z, snow);
            }
        }
    }

    // Заряды ветра бризов (mob/breeze.rs): по прямой, взрыв ветра силой 3.
    for (shooter, (from, velocity)) in outcome.wind_charges {
        let flying = crate::thrown::Flying::wind_charge(shared.next_entity_id(), dimension, from, velocity, shooter, true);

        shared.thrown.lock().expect("снаряды захвачены другим потоком").flying.push(flying);
    }

    // Огненные шары ифритов и гастов (mob/nether.rs): летят как шар дракона.
    for (shooter, launch) in outcome.launches {
        let kind = if launch.small { crate::thrown::Kind::SmallFireball } else { crate::thrown::Kind::GhastFireball };
        let flying = crate::thrown::Flying::mob_fireball(shared.next_entity_id(), dimension, kind, launch.from, launch.dir, shooter, launch.power);

        shared.thrown.lock().expect("снаряды захвачены другим потоком").flying.push(flying);
    }

    for (shooter, target, at) in outcome.bullet_hits {
        let levitation = crate::survival::Effect {
            kind: crate::survival::EffectKind::Levitation,
            amplifier: 0,
            ticks_left: crate::mob::hostile::LEVITATION_TICKS,
            ambient: false,
            visible: true, show_icon: true,
        };

        world.note_mob_hurt(target, crate::mob::hostile::BULLET_DAMAGE, crate::survival::Source::MobAttack, shooter, crate::mob::hostile::killer_key(crate::mob::Kind::Shulker), None, at);
        world.note_mob_effect(target, levitation);
    }

    // Блоки эндерменов: взятый пропадает, поставленный встаёт (вики, Enderman).
    for (x, y, z) in outcome.grabs {
        if let Some(air) = blocks::state_by_name("air") {
            world.set_block(x, y, z, air);
        }
    }

    for ((x, y, z), state) in outcome.puts {
        world.set_block(x, y, z, state);
    }

    for eat in outcome.eaten {
        let (x, y, z) = eat.at;

        // Съеденная трава: у высокой травы блок пропадает, у дёрна — земля
        // (вики, Sheep: «Eating grass»); клиенты видят разлетающиеся частицы
        // (событие мира 2001).
        let old = world.get_block(x, y, z);
        let new = if eat.block { blocks::state_by_name("dirt") } else { blocks::state_by_name("air") };

        if let Some(new) = new {
            world.note_level_event(x, y, z, 2001, old);
            world.set_block(x, y, z, new);
        }
    }

    for (x, y, z) in outcome.eggs {
        if let Some(egg) = blocks::item_named("egg") {
            items.drop_item(shared.next_entity_id(), crate::inventory::Stack::new(egg, 1), (x, y + 0.3, z), (0.0, 0.0, 0.0), items::MINED_DELAY);
        }
    }

    barter(&mut items, shared, &watchers, rng);
    crate::mob::land::pickup_food(&mut items);

    if ticks.is_multiple_of(200) {
        world.store_mobs(items.mobs());
    }

    dragon_xp(&mut items, rng, &mut || shared.next_entity_id());
    drop(items);
    creeper_blasts(shared, dimension, world, &outcome.blasts);
    wither_deeds(shared, dimension, world, outcome.withers);
}

/// Заказы иссушителей (mob/wither.rs): полоса босса — сразу, в ходе сущности;
/// конец зарядки — взрыв силой 7 и глобальное событие 1023 (wither-summon:
/// выпавшее, пакет взрыва, потом 1023); черепа летят снарядами; после удара —
/// ломка блоков 3×4×3 со звуком 1022.
fn wither_deeds(shared: &Shared, dimension: Dimension, world: &mut World, deeds: Vec<(i32, crate::mob::wither::Deed)>) {
    use crate::mob::wither::Deed;

    let griefing = shared.mob_griefing.load(std::sync::atomic::Ordering::Relaxed);

    for (wither, deed) in deeds {
        match deed {
            Deed::Bar(share) => world.note_wither_bar(wither, share),
            Deed::Charged { at, cell } => {
                let players = blast_targets(shared, dimension);

                {
                    let mut items = shared.items_in(dimension).lock().expect("предметы захвачены другим потоком");
                    let mut next_id = || shared.next_entity_id();

                    world.blast_source = wither;
                    world.blast_killer = crate::mob::hostile::killer_key(crate::mob::Kind::Wither);
                    tnt::explode_wither(world, &mut items, &players, at, crate::mob::wither::SPAWN_POWER, griefing, None, &mut next_id);
                    world.blast_killer = "";
                }

                redstone::settle(world);
                drop_destroyed(shared, dimension, world);
                world.note_level_event(cell.0, cell.1, cell.2, 1023, 0);
            }
            Deed::Skull { from, dir, dangerous } => {
                let skull = crate::thrown::Flying::wither_skull(shared.next_entity_id(), dimension, from, (0.0, 0.0, 0.0), dir, wither, dangerous);

                shared.thrown.lock().expect("снаряды захвачены другим потоком").flying.push(skull);
            }
            Deed::Break { feet } if griefing => {
                let cells = crate::mob::wither::break_cells(&MobView(world), feet);

                for (x, y, z) in &cells {
                    let state = world.get_block(*x, *y, *z);

                    world.note_level_event(*x, *y, *z, 2001, state);
                    world.note_destroyed(*x, *y, *z);
                    world.set_block(*x, *y, *z, crate::world::AIR);
                }

                if !cells.is_empty() {
                    world.note_level_event(feet.0, feet.1, feet.2, 1022, 0);
                    redstone::settle(world);
                    drop_destroyed(shared, dimension, world);
                }
            }
            Deed::Break { .. } => {}
        }
    }
}

/// Обмен с пиглинами (вики, Bartering): пиглин без дела подбирает золотой
/// слиток рядом, 6 секунд рассматривает его и бросает в сторону игрока одну
/// вещь из таблицы обмена (`mob::nether::BARTER`).
fn barter(items: &mut items::Items, shared: &Shared, watchers: &[crate::mob::ai::Watcher], rng: &mut crate::mob::Rng) {
    use crate::mob::Kind;

    let Some(gold) = blocks::item_named("gold_ingot") else {
        return;
    };

    // Рассмотрел — бросает добычу туда, куда смотрит [проверить: к игроку, что бросил слиток].
    // Чёрный ящик, piglin-barter: вещь вылетает на 1,49 выше ног в сторону
    // ближайшего игрока, скорость 0,3 вдоль (dx, dy + 1, dz) [проверить: одна точка].
    let done: Vec<(f64, f64, f64, f32)> = items
        .mobs_mut()
        .iter_mut()
        .filter(|mob| mob.kind == Kind::Piglin && mob.nether.barter_done && !mob.is_dead())
        .map(|mob| {
            mob.nether.barter_done = false;
            mob.nether.held.clear();

            (mob.x, mob.y + 1.0, mob.z, mob.yaw)
        })
        .collect();

    for (x, y, z, yaw) in done {
        let rad = f64::from(yaw).to_radians();
        let y = y + 0.49;
        let velocity = watchers
            .iter()
            .filter(|player| !player.spectator)
            .min_by(|a, b| ((a.x - x).powi(2) + (a.z - z).powi(2)).total_cmp(&((b.x - x).powi(2) + (b.z - z).powi(2))))
            .map(|player| {
                let (dx, dy, dz) = (player.x - x, player.y - (y - 1.49) + 1.0, player.z - z);
                let length = (dx * dx + dy * dy + dz * dz).sqrt().max(1.0e-9);

                (dx / length * 0.3, dy / length * 0.3, dz / length * 0.3)
            })
            .unwrap_or((-rad.sin() * 0.3, 0.2, rad.cos() * 0.3));

        for stack in crate::mob::nether::barter_loot(rng) {
            items.drop_item(shared.next_entity_id(), stack, (x, y, z), velocity, items::MINED_DELAY);
        }
    }

    // Подбор слитка: тем, кто свободен.
    let idle: Vec<(usize, i32, (f64, f64, f64))> = items
        .mobs()
        .iter()
        .enumerate()
        .filter(|(_, mob)| mob.kind == Kind::Piglin && !mob.is_dead() && !mob.no_ai && mob.nether.admiring == 0 && mob.nether.held.is_empty())
        .map(|(index, mob)| (index, mob.entity_id, (mob.x, mob.y, mob.z)))
        .collect();

    for (index, id, at) in idle {
        let mut taken_one = false;
        let taken = items.take_near(at, id, |stack| {
            if stack.item == gold && !taken_one {
                taken_one = true;
                1
            } else {
                0
            }
        });

        if !taken.is_empty() {
            // Вики, Advancement: gold pickup credited to its thrower.
            for uuid in taken.iter().filter_map(|item| item.thrower) {
                shared.players.lock().expect("список игроков захвачен другим потоком").order(&uuid,
                    crate::players::Order::Progress(crate::progress::Event::Trigger("nether/distract_piglin")));
            }
            let mob = &mut items.mobs_mut()[index];

            mob.nether.admiring = crate::mob::nether::ADMIRE_TICKS;
            mob.nether.held = vec![crate::inventory::Stack::new(gold, 1)];
            mob.brain.stop();
        }
    }
}

/// Спавнеры (spawner.rs): раз в такт задержка убывает, пока игрок ближе
/// `RequiredPlayerRange`; на нуле ставится до `SpawnCount` мобов в коробке
/// дальности — если рядом меньше `MaxNearbyEntities` того же вида; задержка
/// берётся заново из `MinSpawnDelay…MaxSpawnDelay`. Вики, Monster Spawner;
/// чёрный ящик, spawner-zombie. Правило игры `spawn_mobs` и мирная сложность
/// (для враждебных) останавливают постановку. Частицы — событие мира 2004 у
/// блока и у каждого моба; у моба ещё событие сущности 20 («пуф»).
/// Такт испытательных спавнеров и хранилищ: игроки рядом, мобы волны,
/// выброшенная награда (trial.rs).
fn step_trials(shared: &Shared, world: &mut World, items: &mut items::Items, dimension: Dimension, difficulty: u8, rng: &mut crate::mob::Rng) {
    crate::ominous::step(shared,world,items,dimension,*shared.ticks.borrow(),rng);
    if !world.has_trials() {
        return;
    }

    let visitors: Vec<crate::trial::Visitor> = shared
        .players
        .lock()
        .expect("список игроков захвачен другим потоком")
        .members()
        .iter()
        .filter(|member| member.dimension == dimension && member.game_mode != 3)
        .map(|member| crate::trial::Visitor {
            id: member.entity_id,
            uuid: member.uuid,
            at: (member.position.x, member.position.y, member.position.z),
            eye: if member.sneaking { 1.27 } else { 1.62 },
            counts: matches!(member.game_mode, 0 | 2),
        })
        .collect();
    // Спавнер молчит на мирной и без правила spawnMobs (вики, Trial Spawner).
    let can_spawn = shared.spawn_mobs.load(std::sync::atomic::Ordering::Relaxed) && difficulty != crate::survival::PEACEFUL;
    let now = *shared.ticks.borrow();
    let mut born: Vec<crate::mob::Mob> = Vec::new();
    let mut drops: Vec<crate::trial::Ejected> = Vec::new();
    let states: std::collections::HashMap<i32, bool> = items.mobs().iter().map(|mob| (mob.entity_id, !mob.is_dead())).collect();

    let mob_targets:Vec<_>=items.mobs().iter().filter(|m|!m.is_dead()).map(|m|(m.entity_id,(m.x,m.y,m.z))).collect();
    crate::trial::step(
        world,
        &visitors,
        &mob_targets,
        now,
        can_spawn,
        rng,
        &|id| states.get(&id).copied(),
        &mut |_world, spawn| {
            let mut mob = crate::mob::Mob::new(shared.next_entity_id(), spawn.kind, spawn.at);

            mob.face(spawn.yaw);
            // Голова у поставленного не повёрнута (trial-spawner-wave: head=0).
            mob.head_yaw = 0.0;
            mob.no_ai = spawn.no_ai;
            mob.silent = spawn.silent;
            if spawn.ominous && !spawn.bare {let seed=now ^ mob.entity_id as u64;crate::mob::trial_gear::equip(&mut mob,&mut crate::mob::Rng::new(seed));}
            // Мобы испытаний не исчезают (вики: «do not naturally despawn»).
            mob.persistent = true;

            let id = mob.entity_id;

            born.push(mob);
            id
        },
        &mut |ejected| drops.push(ejected),
    );

    for mob in born {
        items.add_mob(mob);
    }

    for (stack, at, velocity) in drops {
        items.drop_item(shared.next_entity_id(), stack, at, velocity, crate::items::MINED_DELAY);
    }
}

#[allow(clippy::too_many_arguments)]
fn step_spawners(shared: &Shared, world: &mut World, items: &mut items::Items, watchers: &[crate::mob::ai::Watcher], difficulty: u8, spawn_mobs: bool, skyless: bool, time: i64, rng: &mut crate::mob::Rng) {
    world.flush_spawner_followups();

    for (x, y, z) in world.spawner_places() {
        let Some(data) = world.spawner(x, y, z).cloned() else {
            continue;
        };

        // Игрок в шаре дальности (вики: «a player within 16 blocks»).
        let (cx, cy, cz) = (f64::from(x) + 0.5, f64::from(y) + 0.5, f64::from(z) + 0.5);
        let range = f64::from(data.required_range);

        if !watchers.iter().any(|player| !player.spectator && (player.x - cx).powi(2) + (player.y - cy).powi(2) + (player.z - cz).powi(2) <= range * range) {
            continue;
        }

        // Задержка убывает, пока больше нуля; постановка — в такте, когда она уже нуль.
        if data.delay > 0 {
            world.with_spawner(x, y, z, |data| data.delay -= 1);
            continue;
        }

        let kind = data.kind();
        let hostile = kind.is_some_and(|kind| kind.hostile());
        let mut spawned = Vec::new();

        if let Some(kind) = kind
            && spawn_mobs
            && !(hostile && difficulty == crate::survival::PEACEFUL)
        {
            // Сколько таких рядом: коробка дальности ±SpawnRange по сторонам и ±1 по высоте.
            let reach = f64::from(data.spawn_range);
            let nearby = items
                .mobs()
                .iter()
                .filter(|mob| mob.kind == kind && !mob.is_dead() && (mob.x - cx).abs() <= reach + 0.5 && (mob.z - cz).abs() <= reach + 0.5 && (mob.y - cy).abs() <= 2.0)
                .count();
            let ambient = if skyless { 0 } else { world.weather.spawn_sky(redstone::ambient_sky(time)) };

            if nearby < usize::try_from(data.max_nearby).unwrap_or(0) {
                for _ in 0..data.spawn_count {
                    let Some(mob) = crate::spawner::try_spawn(&data, kind, (x, y, z), &MobView(world), rng, &mut || shared.next_entity_id()) else {
                        continue;
                    };

                    // Враждебным нужна темнота: небесный свет ниже случайного 0…31 и
                    // общий не выше 7 (вики, Monster Spawner: «light level 7 or less»)
                    // [проверить: порог у оригинала].
                    if hostile {
                        let (mx, my, mz) = mob.cell();
                        let (sky, block) = world.light_full(mx, my, mz);
                        let sky = (i32::from(sky) - (15 - ambient).max(0)).max(0);

                        if kind == crate::mob::Kind::Silverfish {
                            let nearest = watchers.iter().filter(|player| !player.spectator).map(|player| {
                                (player.x - mob.x).powi(2) + (player.y - mob.y).powi(2) + (player.z - mob.z).powi(2)
                            }).fold(f64::INFINITY, f64::min);

                            if !crate::mob::arthropod::silverfish_spawn(sky.max(i32::from(block)) as u8, world.get_block(mx, my - 1, mz), nearest) {
                                continue;
                            }
                        } else if kind == crate::mob::Kind::CaveSpider {
                            // Вики, Cave Spider: «light level of 0» (блочный свет).
                            if !crate::mob::spawn::dark_enough(sky as u8, block, rng) {
                                continue;
                            }
                        } else if sky > rng.below(32) as i32 || sky.max(i32::from(block)) > 7 {
                            continue;
                        }
                    }

                    spawned.push(mob);
                }
            }
        }

        // Постановка удалась: частицы у блока (одно событие мира на залп), «пуф»
        // у каждого моба (событие сущности 20), задержка заново; без постановки
        // (темно не вышло, места нет) спавнер пробует каждый такт (вики,
        // Monster Spawner; чёрный ящик, spawner-zombie, spawner-light).
        if spawned.is_empty() {
            continue;
        }

        for mut mob in spawned {
            mob.events.push(crate::mob::Event::Status(20));
            items.add_mob(mob);
        }

        world.note_level_event(x, y, z, crate::spawner::SPAWN_PARTICLES, 0);
        world.with_spawner(x, y, z, |data| data.reset_delay(rng));

        // Клиентам — блок, данные и событие на такт позже.
        world.queue_spawner_followup(x, y, z);
    }
}

/// Удар зомби по жителю. Урон по жителю — 3 при любой сложности: поправка по
/// сложности только для игроков (чёрный ящик, zombie-villager-infect: на трудной
/// здоровье жителя 20, 17, 14, 11, 8, 5, 2).
/// Удар, что убил бы жителя, на обычной с шансом 50 % и на трудной всегда
/// превращает его в зомби-жителя с теми же профессией, опытом и сделками:
/// житель пропадает, на его месте встаёт зомби-житель (детёныш остаётся
/// детёнышем), клиентам — событие мира 1026 (вики, Zombie Villager;
/// чёрный ящик, zombie-villager-infect). Возвращает, был ли удар по жителю.
fn strike_villager(items: &mut items::Items, world: &mut World, strike: &crate::mob::Strike, difficulty: u8, rng: &mut crate::mob::Rng, next_id: &mut dyn FnMut() -> i32) -> bool {
    use crate::mob::Kind;

    let Some(index) = items.mobs().iter().position(|mob| mob.entity_id == strike.melee.target && mob.kind == Kind::Villager) else {
        return false;
    };

    let damage = strike.melee.damage;
    let mob = &mut items.mobs_mut()[index];

    if mob.is_dead() {
        return true;
    }

    let infects = mob.vitals.health - damage <= 0.0 && matches!(strike.kind, Kind::Zombie | Kind::ZombieVillager) && (difficulty >= 3 || (difficulty == 2 && rng.below(2) == 0));

    if !infects {
        let push = crate::mob::Push { dir: strike.melee.push, strength: 0.4, cause: strike.attacker };

        if mob.hurt_burning(damage, crate::survival::Source::MobAttack, Some(push), 1.0, strike.enchantments.fire_ticks).is_some()
            && strike.enchantments.melee_knockback > 0.0 {
            mob.knockback(push.dir.0, push.dir.1, f64::from(strike.enchantments.melee_knockback) * 0.5);
        }

        return true;
    }

    // Удар прошёл, но смерть отменена: клиентам — урон, потом превращение
    // (чёрный ящик, zombie-villager-infect: damage_event, затем появление
    // зомби-жителя со скоростью жителя).
    mob.events.push(crate::mob::Event::Hurt { source: crate::survival::Source::MobAttack, cause: strike.attacker, at: (mob.x, mob.y, mob.z) });

    let mut zombie = crate::mob::Mob::new(next_id(), Kind::ZombieVillager, (mob.x, mob.y, mob.z));

    (zombie.vx, zombie.vy, zombie.vz) = (mob.vx, mob.vy, mob.vz);

    zombie.yaw = mob.yaw;
    zombie.head_yaw = mob.head_yaw;
    zombie.pitch = mob.pitch;
    zombie.age = mob.age;
    zombie.no_ai = mob.no_ai;
    zombie.silent = mob.silent;
    zombie.on_ground = mob.on_ground;
    zombie.villager = std::mem::take(&mut mob.villager);
    zombie.villager.finalized = true;
    zombie.villager.trading = None;
    let (id, (x, y, z)) = (mob.entity_id, mob.cell());

    items.flush_mob_events(next_id, rng);
    world.note_level_event(x, y, z, 1026, 0);
    items.remove_mobs(|gone| gone.entity_id == id);
    items.add_mob_late(zombie);
    true
}

/// Опыт умирающего дракона: с 154-го такта подъёма десять шариков по 8 %
/// и последний в 20 % на 200-м (вики, Ender Dragon: «Starting 7.7 seconds
/// (154 game ticks) … ten drops of 960 and one drop of 2400») [проверить:
/// такты между шариками].
fn dragon_xp(items: &mut items::Items, rng: &mut crate::mob::Rng, next_id: &mut dyn FnMut() -> i32) {
    let dying: Vec<(usize, i32, (f64, f64, f64))> = items
        .mobs()
        .iter()
        .enumerate()
        .filter(|(_, mob)| mob.kind == crate::mob::Kind::EnderDragon && mob.is_dead() && mob.dragon.xp > 0)
        .map(|(index, mob)| (index, mob.dead_for, (mob.x, mob.y + 4.0, mob.z)))
        .collect();

    for (index, dead_for, at) in dying {
        let total = items.mobs()[index].dragon.xp;
        let portion = if dead_for == crate::mob::DRAGON_DEATH_TICKS {
            total / 5
        } else if dead_for >= 154 && (dead_for - 154) % 5 == 0 {
            total * 8 / 100
        } else {
            continue;
        };

        items.drop_experience(portion, at, next_id, rng);

        if dead_for == crate::mob::DRAGON_DEATH_TICKS {
            items.mobs_mut()[index].dragon.xp = 0;
        }
    }
}

/// Взрывы криперов: центр — их ноги, сила 3; каждый разбирается до конца, как
/// у динамита. Виновник — крипер: игрок, что погиб, узнает его по имени.
fn creeper_blasts(shared: &Shared, dimension: Dimension, world: &mut World, blasts: &[crate::mob::Blast]) {
    if blasts.is_empty() {
        return;
    }

    let players = blast_targets(shared, dimension);

    for blast in blasts {
        {
            let mut items = shared.items_in(dimension).lock().expect("предметы захвачены другим потоком");
            let mut next_id = || shared.next_entity_id();

            world.blast_source = blast.shooter;
            world.blast_killer = crate::mob::hostile::killer_key(crate::mob::Kind::Creeper);
            tnt::explode(world, &mut items, &players, blast.at, blast.power, false, &mut next_id);
            world.blast_killer = "";
            prime_sounds(world, &mut items);
        }

        redstone::settle(world);
        drop_destroyed(shared, dimension, world);
    }
}

/// Такт вагонеток: каждая в порядке появления, со встречами друг с другом
/// и с игроками. В выгруженном чанке вагонетка стоит, как и всё там.
/// Провалившиеся в пустоту пропадают.
fn step_carts(shared: &Shared, dimension: Dimension, world: &mut World) {
    /// Режим наблюдателя: сквозь всё проходит, вагонетку не толкает.
    const SPECTATOR: i32 = 3;

    if shared.items_in(dimension).lock().expect("предметы захвачены другим потоком").carts().is_empty() {
        return;
    }

    let players: Vec<(f64, f64, f64)> = shared
        .players
        .lock()
        .expect("список игроков захвачен другим потоком")
        .members()
        .iter()
        .filter(|member| member.dimension == dimension && member.game_mode != SPECTATOR)
        .map(|member| (member.position.x, member.position.y, member.position.z))
        .collect();

    let mut items = shared.items_in(dimension).lock().expect("предметы захвачены другим потоком");

    {
        let view = |x, y, z| world.get_block(x, y, z);
        let loaded = |cart: &crate::minecart::Minecart| {
            world.has_chunk((cart.x.floor() as i32).div_euclid(16), (cart.z.floor() as i32).div_euclid(16))
        };

        crate::minecart::step_all(items.carts_mut(), &view, &players, loaded);
    }

    items.remove_carts(|cart| cart.lost());

    prime_sounds(world, &mut items);
    boat_sounds(world, &mut items);
}

/// Звуки лодок за такт: плеск входа в воду (высота — как у предметов,
/// 1 ± 0,4) и вёсла (0,8…1,2 [проверить]).
fn boat_sounds(world: &mut World, items: &mut items::Items) {
    for cart in items.carts_mut().iter_mut().filter(|cart| !cart.sounds.is_empty()) {
        for (name, at, volume) in std::mem::take(&mut cart.sounds) {
            let pitch = if name == crate::minecart::boat::SPLASH_SOUND {
                1.0 + (world.random_unit() - world.random_unit()) * 0.4
            } else {
                0.8 + world.random_unit() * 0.4
            };

            world.note_sound_at(at, name, volume, pitch);
        }
    }
}

/// Звук поджога у вагонеток с динамитом, подожжённых в этом такте — там,
/// где они стоят (зонд t-act: звук вместе с событием «подожжена»).
fn prime_sounds(world: &mut World, items: &mut items::Items) {
    for cart in items.carts_mut().iter_mut().filter(|cart| cart.just_primed) {
        cart.just_primed = false;
        world.note_sound_at((cart.x, cart.y, cart.z), tnt::PRIMED_SOUND, 1.0, 1.0);
    }
}

/// Вагонетки с динамитом, которым пора, взрываются — по порядку появления,
/// каждая до конца, как динамит (`explode_all`). Центр — место вагонетки.
/// [проверить]: центр и порядок относительно прочих сущностей.
fn explode_carts(shared: &Shared, dimension: Dimension, world: &mut World) {
    let blasts: Vec<(i32, (f64, f64, f64), f64)> = shared
        .items_in(dimension)
        .lock()
        .expect("предметы захвачены другим потоком")
        .carts()
        .iter()
        .filter_map(|cart| Some((cart.entity_id, (cart.x, cart.y, cart.z), cart.blast?)))
        .collect();

    if blasts.is_empty() {
        return;
    }

    let players = blast_targets(shared, dimension);

    // Сама взорвавшаяся пропадает после взрыва: у оригинала задетые
    // вагонетки разбиваются раньше (зонд t-act).
    for (entity_id, centre, bonus) in blasts {
        let power = crate::minecart::blast_power(bonus, world.random_unit());

        {
            let mut items = shared.items_in(dimension).lock().expect("предметы захвачены другим потоком");
            let mut next_id = || shared.next_entity_id();

            tnt::explode(world, &mut items, &players, centre, power, true, &mut next_id);
            items.remove_carts(|cart| cart.entity_id == entity_id);
            prime_sounds(world, &mut items);
        }

        redstone::settle(world);
        drop_destroyed(shared, dimension, world);
    }
}

/// Вагонетки с воронкой тянут предметы — сразу после своего шага, в фазе
/// сущностей (зонд minecart-hopper: первый предмет — в такт появления).
fn hopper_carts(shared: &Shared, dimension: Dimension, world: &mut World) {
    let mut items = shared.items_in(dimension).lock().expect("предметы захвачены другим потоком");

    if !items.carts().iter().any(|cart| cart.kind == crate::minecart::Kind::Hopper && cart.enabled) {
        return;
    }

    let mut carts = std::mem::take(items.carts_vec());

    for cart in carts.iter_mut().filter(|cart| cart.kind == crate::minecart::Kind::Hopper && cart.enabled) {
        let at = (cart.x, cart.y, cart.z);

        containers::cart_pull(world, &mut items, &mut cart.slots, at);
    }

    *items.carts_vec() = carts;
    drop(items);

    containers::after_change(world);
}

/// Убирает из памяти чанки, рядом с которыми никого нет.
///
/// Без этого память растёт вместе с тем, сколько мира обошли игроки, и
/// обратно уже не возвращается.
fn unload_empty_chunks(shared: &Shared, dimension: Dimension, world: &mut World) {
    let players: Vec<(i32, i32)> = shared
        .players
        .lock()
        .expect("список игроков захвачен другим потоком")
        .members()
        .iter()
        .filter(|member| member.dimension == dimension)
        .map(|member| {
            (
                (member.position.x.floor() as i32).div_euclid(16),
                (member.position.z.floor() as i32).div_euclid(16),
            )
        })
        .collect();

    let keep = shared.properties.view_distance + KEEP_EXTRA.max(shared.settings.prefetch_chunks + 1);

    // Мобы уходят в файл вместе со своими чанками: запись — до выгрузки,
    // а из мира убираются те, чей чанк выгрузился.
    store_mobs(shared, dimension, world);

    // Удержанные (`forceload`) не выгружаются.
    let forced = crate::commands_world::forced_chunks(shared, dimension);
    let unloaded = world.unload_far(|x, z| {
        players
            .iter()
            .any(|(px, pz)| (x - px).abs() <= keep && (z - pz).abs() <= keep)
            || forced.contains(&(x, z))
    });

    if unloaded > 0 {
        shared
            .items_in(dimension)
            .lock()
            .expect("предметы захвачены другим потоком")
            .remove_mobs(|mob| {
                let (x, z) = mob.chunk();

                !world.has_chunk(x, z)
            });
    }

    if unloaded > 0 {
        log_debug!("Мир: выгружено чанков — {}", unloaded);
    }
}

/// Пускает в свободное падение блоки, под которыми не стало опоры.
///
/// Блок пропадает из мира и становится сущностью — дальше он летит по тем же
/// правилам, что и выброшенные предметы.
fn start_falling(shared: &Shared, dimension: Dimension, world: &mut World) {
    for (x, y, z) in world.take_falling() {
        let state = world.get_block(x, y, z);

        // Пока место ждало своей очереди, всё могло измениться. Падает блок
        // сквозь всё заменяемое: воздух, воду, траву (вики, Falling Block:
        // «Block below is replaceable»).
        let scaffold = blocks::block_at_state(state) == Some("scaffolding");
        let dripstone = blocks::block_at_state(state) == Some("pointed_dripstone");
        if dripstone && crate::dripstone::supported(world, (x, y, z), state) { continue; }
        if scaffold && crate::scaffolding::distance(world, (x, y, z)) < 7 { continue; }
        if !dripstone && ((!scaffold && !redstone::falls(state)) || !redstone::free_to_fall_into(world.get_block(x, y - 1, z))) {
            continue;
        }

        // Что выпадет, если приземлиться будет некуда: сам блок. Кроме
        // подозрительных песка и гравия — они просто разбиваются (вики,
        // Falling Block). Пустая стопка — «не выпадет ничего».
        let item = blocks::block_at_state(state)
            .filter(|name| !name.starts_with("suspicious_"))
            .and_then(blocks::item_for_block);
        let stack = item.map_or(Stack::new(0, 0), |item| Stack::new(item, 1));
        let entity_id = shared.next_entity_id();

        let water = (scaffold || dripstone) && blocks::orientation_of(state).and_then(|b| b.value(state, "waterlogged")) == Some("true");
        let state = if water { blocks::orientation_of(state).and_then(|b| b.with(state, "waterlogged", "false")).unwrap_or(state) } else { state };
        world.set_block(x, y, z, if water { blocks::state_by_name("water").unwrap() } else { crate::world::AIR });

        shared
            .items_in(dimension)
            .lock()
            .expect("предметы захвачены другим потоком")
            .drop_block(
                entity_id,
                state,
                stack,
                (x as f64 + 0.5, y as f64, z as f64 + 0.5),
            );
    }
}

/// Ставит обратно в мир то, что долетело до земли.
///
/// «If it lands with the bottom center of its hitbox in a replaceable block,
/// and the block below can support it, then the falling block returns to its
/// block state. Otherwise, it breaks and drops as an item.»
fn land_falling(shared: &Shared, dimension: Dimension, world: &mut World) {
    let landed = shared
        .items_in(dimension)
        .lock()
        .expect("предметы захвачены другим потоком")
        .take_landed();

    for block in landed {
        let Some(state) = block.block else {
            continue;
        };

        let (x, y, z) = (
            block.x.floor() as i32,
            block.y.round() as i32,
            block.z.floor() as i32,
        );

        // Место свободно — блок встаёт обратно. Заменяемое (трава, вода,
        // снежный слой) он просто занимает: так у оригинала.
        let there = world.get_block(x, y, z);

        let dripstone = blocks::block_at_state(state) == Some("pointed_dripstone");
        // Вики, Pointed Dripstone: «breaks upon landing»; запись
        // dripstone-placement-support.md:20,23 — 1045/0, не частицы 2001.
        if dripstone { world.note_level_event(x, y, z, 1045, 0); }
        if !dripstone && redstone::free_to_fall_into(there) {
            // Леса приземляются с прежним состоянием (distance=7), а пересчёт идёт
            // следующим тактом (чёрный ящик, scaffolding-support).
            let scaffold = blocks::block_at_state(state) == Some("scaffolding");
            let landed = if scaffold { state } else { hardened_in_water(state, there) };
            world.set_block(x, y, z, landed);
            if scaffold { world.schedule_block_once(x, y, z, 1); }

            // Наковальня, упав, звенит (событие мира 1031, вики, World
            // Event: «Anvil lands») [проверить порядок].
            if crate::enchanting::Station::of(blocks::block_at_state(state).unwrap_or("")) == Some(crate::enchanting::Station::Anvil) {
                world.note_level_event(x, y, z, 1031, 0);
            }

            // Оригинал говорит о приземлившемся блоке дважды: сразу и ещё
            // раз общей рассылкой следующего такта (снято чёрным ящиком,
            // сценарий sand-into-water).
            world.tell_directly(x, y, z);
            redstone::settle(world);
            continue;
        }

        // Занято — падает предметом. Если выпадать нечему (подозрительный
        // песок), он просто разбивается.
        if block.stack.count <= 0 {
            continue;
        }

        let entity_id = shared.next_entity_id();

        shared
            .items_in(dimension)
            .lock()
            .expect("предметы захвачены другим потоком")
            .drop_item(
                entity_id,
                block.stack,
                (block.x, block.y, block.z),
                (0.0, 0.0, 0.0),
                items::MINED_DELAY,
            );
    }
}

/// Бетонная пыль, приземлившаяся в воду, становится бетоном того же цвета
/// (вики, Falling Block: «Becomes concrete if it intersects water while
/// falling or after landing»). Где именно она встаёт, если вода глубокая, —
/// на дне или у поверхности, — вики не уточняет: у нас на дне (сверить
/// чёрным ящиком).
fn hardened_in_water(state: i32, there: i32) -> i32 {
    let in_water = matches!(fluids::fluid_at(there), Some((fluids::Kind::Water, _)));

    blocks::block_at_state(state)
        .filter(|_| in_water)
        .and_then(|name| name.strip_suffix("_powder"))
        .filter(|name| name.ends_with("_concrete"))
        .and_then(blocks::state_by_name)
        .unwrap_or(state)
}

/// Роняет предметы из блоков, которые сломал не игрок, а сам мир.
fn drop_destroyed(shared: &Shared, dimension: Dimension, world: &mut World) {
    for ((x, y, z), state) in world.take_destroyed() {
        // Инструмент тут ни при чём: блок сломал не игрок.
        let luck = world.random_unit();

        // Посев — несколько стопок (farming.rs), прочее — одна.
        let stacks = match crate::farming::harvest(world, state, 0) {
            Some(stacks) => stacks,
            // Блоки каталога 26.3 (шип серы): выпадение по каталогу мира, без инструмента.
            None if matches!(world.catalog().block_entry(state), Ok(Some(_))) =>
                world.catalog().drops_by_luck(state, None, luck).ok().flatten().into_iter().collect(),
            None => blocks::world_drops(state, luck).into_iter().collect(),
        };

        for stack in stacks {
            let entity_id = shared.next_entity_id();
            let (at, velocity) = scattered((x, y, z), || f64::from(world.random_unit()));

            shared
                .items_in(dimension)
                .lock()
                .expect("предметы захвачены другим потоком")
                .drop_item(entity_id, stack, at, velocity, items::MINED_DELAY);
        }
    }
}

/// Где и с какой скоростью появляется предмет, выпавший из сломанного блока.
///
/// Снято чёрным ящиком (water-washes, dust-on-slab, tnt-*): точка случайна
/// в пределах ±0,25 от середины блока по каждой оси (по высоте — середина
/// предмета, его низ на 0,125 ниже), скорость вверх 0,2, вбок — случайная,
/// до 0,1 в каждую сторону. Без этого смытые водой предметы плыли строго по
/// оси блока, и всплески воды звучали не там.
pub(crate) fn scattered(
    (x, y, z): (i32, i32, i32),
    mut random: impl FnMut() -> f64,
) -> ((f64, f64, f64), (f64, f64, f64)) {
    let mut offset = || random() * 0.5 - 0.25;
    let at = (
        f64::from(x) + 0.5 + offset(),
        f64::from(y) + 0.5 + offset() - 0.125,
        f64::from(z) + 0.5 + offset(),
    );
    let velocity = (random() * 0.2 - 0.1, 0.2, random() * 0.2 - 0.1);

    (at, velocity)
}

/// Предметы, выброшенные выбрасывателями и раздатчиками, становятся
/// сущностями: с той точкой и скоростью, что выбрало хранилище.
fn spawn_ejected(shared: &Shared, dimension: Dimension, world: &mut World) {
    for (at, velocity, stack) in world.take_ejected() {
        let entity_id = shared.next_entity_id();

        shared
            .items_in(dimension)
            .lock()
            .expect("предметы захвачены другим потоком")
            .drop_item(entity_id, stack, at, velocity, items::DISPENSED_DELAY);
    }
}

/// Подожжённый динамит становится сущностью: запал и толчок выбрал тот,
/// кто поджёг.
fn spawn_primed(shared: &Shared, dimension: Dimension, world: &mut World) {
    let primed = world.take_primed();

    if primed.is_empty() {
        return;
    }

    let state = blocks::state_by_name("tnt").unwrap_or(crate::world::AIR);
    let mut items = shared.items_in(dimension).lock().expect("предметы захвачены другим потоком");

    for (at, velocity, fuse, custom) in primed {
        items.prime(shared.next_entity_id(), state, at, velocity, fuse, custom);
    }
}

/// Взрывы догоревшего динамита. Каждый разбирается до конца — редстоун,
/// выпавшее из отвалившегося, — прежде чем следующий: второй взрыв видит
/// мир уже без того, что снёс первый.
fn explode_all(shared: &Shared, dimension: Dimension, world: &mut World, exploding: Vec<items::Dropped>) {
    if exploding.is_empty() {
        return;
    }

    let players = blast_targets(shared, dimension);

    for primed in exploding {
        {
            let mut items = shared.items_in(dimension).lock().expect("предметы захвачены другим потоком");
            let mut next_id = || shared.next_entity_id();

            tnt::explode_primed(world, &mut items, &players, &primed, &mut next_id);
            prime_sounds(world, &mut items);
        }

        redstone::settle(world);
        drop_destroyed(shared, dimension, world);
    }
}

/// Взрыв кристалла Края силой 6 в его точке (вики, End Crystal: «explosion
/// power of 6»): кристалл убирается до взрыва; дракон, которого он лечил,
/// получает 10 (вики: «takes 10 HP damage»). Виновник — `culprit` (0 —
/// никто). Возвращает, был ли такой кристалл.
pub(crate) fn blow_crystal(
    world: &mut World,
    items: &mut items::Items,
    players: &[tnt::Target],
    crystal_id: i32,
    culprit: i32,
    next_id: &mut dyn FnMut() -> i32,
) -> bool {
    /// Сила взрыва кристалла.
    const POWER: f32 = 6.0;

    let removed = items.remove_mobs(|mob| mob.entity_id == crystal_id);

    let Some(crystal) = removed.first() else {
        return false;
    };
    let centre = (crystal.x, crystal.y, crystal.z);

    for dragon in items.mobs_mut().iter_mut().filter(|mob| mob.healer == crystal_id) {
        dragon.healer = 0;
        dragon.hurt(10.0, crate::survival::Source::Explosion, None);
    }

    world.blast_source = culprit;
    world.blast_by_player = culprit != 0;
    tnt::explode(world, items, players, centre, POWER, false, next_id);
    prime_sounds(world, items);

    true
}

/// Кристаллы, задетые не ударом игрока (стрела, взрыв рядом), взрываются в
/// такте после шага [проверить: у оригинала, возможно, в тот же такт, что и
/// задевший их взрыв]. Взрыв одного может задеть другой — тогда и тот, по
/// кругу.
fn detonate_crystals(shared: &Shared, dimension: Dimension, world: &mut World) {
    loop {
        let doomed: Vec<i32> = {
            let mut items = shared.items_in(dimension).lock().expect("предметы захвачены другим потоком");
            items.mobs_mut().iter().filter(|mob| mob.exploding).map(|mob| mob.entity_id).collect()
        };

        if doomed.is_empty() {
            return;
        }

        let players = blast_targets(shared, dimension);

        for crystal_id in doomed {
            {
                let mut items = shared.items_in(dimension).lock().expect("предметы захвачены другим потоком");
                let mut next_id = || shared.next_entity_id();

                blow_crystal(world, &mut items, &players, crystal_id, 0, &mut next_id);
                items.flush_mob_events(&mut next_id, &mut crate::mob::Rng::new(u64::from(crystal_id as u32)));
            }

            redstone::settle(world);
            drop_destroyed(shared, dimension, world);
        }
    }
}

/// Игроки этого мира, которых может толкнуть взрыв.
/// Проспали ли спящие ночь (вики, Bed: «Once all players in a world are
/// asleep, after 5 seconds (100 ticks) the time of day changes to sunrise»;
/// доля — правило `players_sleeping_percentage`): к следующему такту их
/// будят (поручение подключениям — пробуждение у них уходит в том же такте,
/// что и утро: чёрный ящик bed-sleep-night, утро, взмах «встал», сверка
/// места и «не занята» — на +100 от того, как лёг). Возвращает, наступит ли
/// утро в начале следующего такта.
fn wake_sleepers(shared: &Shared) -> bool {
    let now = shared.world_in(Dimension::Overworld).lock().expect("мир захвачен другим потоком").tick();
    let percentage = shared.sleeping_percentage.load(std::sync::atomic::Ordering::Relaxed);
    let mut players = shared.players.lock().expect("список игроков захвачен другим потоком");
    let (total, sleeping, deep) = players.sleep_status(Dimension::Overworld, now);

    let Some(needed) = crate::bed::sleepers_needed(total, percentage) else {
        return false;
    };

    if sleeping == 0 || sleeping < needed || deep < needed {
        return false;
    }

    let mut gifts = shared.morning_gifts.lock().expect("подарки захвачены другим потоком");

    for uuid in players.sleepers() {
        if let Some(member) = players.members().iter().find(|member| member.uuid == uuid) {
            gifts.push((uuid, (member.position.x, member.position.y, member.position.z)));
        }

        players.set_sleeping(&uuid, None);
        players.order(&uuid, crate::players::Order::WakeUp);
    }

    true
}

/// Подарки кошек: каждая ручная кошка проснувшегося хозяина в 10 блоках от кровати,
/// что не сидит по приказу, с шансом 70 % кладёт подарок (вики, Cat, «Morning gifts»).
fn morning_gifts(shared: &Shared, world: &World) {
    let gifts: Vec<([u8; 16], (f64, f64, f64))> = std::mem::take(&mut *shared.morning_gifts.lock().expect("подарки захвачены другим потоком"));

    if gifts.is_empty() {
        return;
    }

    let mut rng = crate::mob::Rng::new(world.tick() ^ 0x05ee_dc47);
    let mut items = shared.items_in(Dimension::Overworld).lock().expect("предметы захвачены другим потоком");
    let mut cats: Vec<(f64, f64, f64)> = Vec::new();

    for (owner, bed) in gifts {
        for mob in items.mobs() {
            let near = (mob.x - bed.0).powi(2) + (mob.y - bed.1).powi(2) + (mob.z - bed.2).powi(2) <= 100.0;

            if mob.kind == crate::mob::Kind::Cat && mob.tame.owner == Some(owner) && !mob.tame.ordered_sit && !mob.is_dead() && near && rng.below(100) < 70 {
                cats.push((mob.x, mob.y, mob.z));
            }
        }
    }

    for (x, y, z) in cats {
        if let Some(item) = blocks::item_named(crate::mob::tame::morning_gift(&mut rng)) {
            items.drop_item(shared.next_entity_id(), crate::inventory::Stack::new(item, 1), (x, y + 0.5, z), (0.0, 0.2, 0.0), items::MINED_DELAY);
        }
    }
}

/// Утро после сна: часы — на начало следующих суток, а дождь кончается и
/// цикл погоды начинается заново, если погода меняется сама (вики, Bed:
/// «the weather cycle resets only during rainy or snowy weather»).
fn morning(shared: &Shared, world: &mut World) {
    let time = world.time_of_day();

    world.set_time_of_day(time + crate::bed::until_morning(time));

    if shared.advance_weather.load(std::sync::atomic::Ordering::Relaxed) && world.weather.raining {
        world.weather.reset_cycle();
    }
}

/// Взрыв кровати вне обычного мира и якоря вне Незера (bed.rs): сила 5,
/// с огнём; каждый сломанный блок разбирается, выпавшее — в мир.
pub(crate) fn respawn_block_blast(shared: &Shared, dimension: Dimension, world: &mut World, centre: (f64, f64, f64)) {
    let players = blast_targets(shared, dimension);

    {
        let mut items = shared.items_in(dimension).lock().expect("предметы захвачены другим потоком");
        let mut next_id = || shared.next_entity_id();

        tnt::explode_with_fire(world, &mut items, &players, centre, crate::bed::EXPLOSION_POWER, &mut next_id);
        prime_sounds(world, &mut items);
    }

    redstone::settle(world);
    drop_destroyed(shared, dimension, world);
}

pub(crate) fn blast_targets(shared: &Shared, dimension: Dimension) -> Vec<tnt::Target> {
    /// Режим наблюдателя: сквозь всё проходит, взрыв его не толкает.
    const SPECTATOR: i32 = 3;

    shared
        .players
        .lock()
        .expect("список игроков захвачен другим потоком")
        .members()
        .iter()
        .filter(|member| member.dimension == dimension && member.game_mode != SPECTATOR)
        .map(|member| tnt::Target {
            entity_id: member.entity_id,
            x: member.position.x,
            y: member.position.y,
            z: member.position.z,
            sneaking: member.sneaking,
        })
        .collect()
}

/// Молнии грозы (lightning.rs, `natural_target`): в каждом чанке в
/// пределах дальности симуляции от игроков — с шансом 1/100 000 за такт.
/// Фаза чанков, до случайных тактов (вики, Tick) [проверить].
fn natural_lightning(shared: &Shared, dimension: Dimension, world: &mut World, players: &[(i32, i32)]) {
    if !world.weather.is_thundering() || players.is_empty() {
        return;
    }

    let distance = shared.properties.simulation_distance;
    let mut chunks: Vec<(i32, i32)> = Vec::new();

    for (px, pz) in players {
        for chunk_x in px - distance..=px + distance {
            for chunk_z in pz - distance..=pz + distance {
                if !chunks.contains(&(chunk_x, chunk_z)) && world.has_chunk(chunk_x, chunk_z) {
                    chunks.push((chunk_x, chunk_z));
                }
            }
        }
    }

    let mut strikes = Vec::new();
    let mut living: Option<Vec<(f64, f64, f64)>> = None;

    for chunk in chunks {
        if !world.random_bits().is_multiple_of(crate::lightning::NATURAL_CHANCE) {
            continue;
        }

        // Живые — только когда удар выпал: это редкость.
        let living = living.get_or_insert_with(|| {
            let mut places: Vec<(f64, f64, f64)> = shared
                .players
                .lock()
                .expect("список игроков захвачен другим потоком")
                .members()
                .iter()
                .filter(|member| member.dimension == dimension && member.game_mode != 3)
                .map(|member| (member.position.x, member.position.y, member.position.z))
                .collect();
            let items = shared.items_in(dimension).lock().expect("предметы захвачены другим потоком");

            places.extend(items.mobs().iter().filter(|mob| !mob.is_dead()).map(|mob| (mob.x, mob.y, mob.z)));
            places
        });

        if let Some(at) = crate::lightning::natural_target(world, chunk, living) {
            strikes.push(at);
        }
    }

    if strikes.is_empty() {
        return;
    }

    let mut items = shared.items_in(dimension).lock().expect("предметы захвачены другим потоком");

    for at in strikes {
        let difficulty = shared.difficulty.load(std::sync::atomic::Ordering::Relaxed);
        // Вики, Skeleton Horse: грозовые молнии, 0,75–6,75 % в зависимости
        // от местной сложности. [проверить] местной сложности пока нет:
        // используем нижнюю границу диапазона сложности мира.
        let chance = match difficulty { 1 => 0.0075, 2 => 0.015, 3 => 0.0225, _ => 0.0 };
        // Вики, Tutorial:Skeleton horse farming: «lightning rods will not
        // spawn skeleton trap horses». Притяжение громоотвода ещё не ведётся.
        let rod = blocks::block_at_state(world.get_block(at.0.floor() as i32, at.1.floor() as i32 - 1, at.2.floor() as i32)).is_some_and(|name| name.ends_with("lightning_rod"));
        let trap = !rod && shared.spawn_mobs.load(std::sync::atomic::Ordering::Relaxed) && world.random_unit() < chance;
        let mut bolt = crate::lightning::Bolt::new(shared.next_entity_id(), at);
        if trap {
            let mut horse = crate::mob::Mob::new(shared.next_entity_id(), crate::mob::Kind::SkeletonHorse, at);
            horse.horse.trap = true;
            items.add_mob(horse);
            bolt.visual_only = true;
        }
        items.add_bolt(bolt);
    }
}

/// Такт молний; огонь от них — только на обычной и сложной сложности.
/// Сложность — нынешняя, не из server.properties: её меняет команда
/// (чёрный ящик, trident-channeling: огонь после `difficulty normal`).
/// Светящаяся молния бьёт игроков и мобов рядом (вики, Lightning).
fn step_bolts(shared: &Shared, dimension: Dimension, world: &mut World) {
    let fire = shared.difficulty.load(std::sync::atomic::Ordering::Relaxed) >= crate::config::server_properties::DIFFICULTY_NORMAL;
    let mut items = shared.items_in(dimension).lock().expect("предметы захвачены другим потоком");
    let mut lit: Vec<crate::lightning::Bolt> = Vec::new();
    let mut ended = Vec::new();
    let living: Vec<_> = items.mobs().iter().filter(|m| !m.is_dead()).map(|m| (m.entity_id, (m.x, m.y, m.z))).collect();

    items.step_bolts(|bolt| {
        let alive = crate::lightning::step(world, bolt, fire);

        if alive && bolt.is_lit() && !bolt.visual_only {
            for &(id, at) in &living {
                if bolt.reaches(at) && !bolt.struck_entities.contains(&id) { bolt.struck_entities.push(id); }
            }
            lit.push(bolt.clone());
        }
        if !alive { ended.push(bolt.clone()); }

        alive
    });

    for bolt in ended {
        if items.mobs().iter().any(|m| m.kind == crate::mob::Kind::Villager && !m.is_dead() && bolt.protects(m.entity_id, (m.x, m.y, m.z))) {
            let mut players = shared.players.lock().expect("список игроков захвачен другим потоком");
            let nearby: Vec<_> = players.members().iter().filter(|m| m.dimension == dimension
                && (m.position.x - bolt.x).powi(2) + (m.position.y - bolt.y).powi(2) + (m.position.z - bolt.z).powi(2) <= 900.0).map(|m| m.uuid).collect();
            for uuid in nearby { players.order(&uuid, crate::players::Order::Progress(crate::progress::Event::Trigger("adventure/lightning_rod_with_villager_no_fire"))); }
        }
    }

    if lit.is_empty() {
        return;
    }

    for mob in items.mobs_mut().iter_mut().filter(|mob| !mob.is_dead() && lit.iter().any(|bolt| bolt.reaches((mob.x, mob.y, mob.z)))) {
        let strikes: Vec<_>=lit.iter().filter(|b| b.reaches((mob.x,mob.y,mob.z))).map(|b| b.entity_id).collect();
        for bolt in strikes { crate::mob::land::lightning(mob,bolt); }
        if mob.vitals.fire > 0 {
            mob.vitals.fire += 1;
        } else {
            mob.vitals.ignite(crate::lightning::STRIKE_FIRE_TICKS);
        }

        for uuid in lit.iter().filter(|bolt| bolt.reaches((mob.x, mob.y, mob.z))).filter_map(|bolt| bolt.channeler) {
            if mob.kind == crate::mob::Kind::Villager {
                shared.players.lock().expect("список игроков захвачен другим потоком").order(&uuid,
                    crate::players::Order::Progress(crate::progress::Event::Trigger("adventure/very_very_frightening")));
            }
        }
        if mob.kind == crate::mob::Kind::CopperGolem && !mob.newmobs.waxed {
            mob.newmobs.oxidation=mob.newmobs.oxidation.saturating_sub(1);
            mob.newmobs.oxidize_at=0;
        }
        mob.hurt(crate::lightning::STRIKE_DAMAGE, crate::survival::Source::Lightning, None);
    }

    drop(items);

    let struck: Vec<i32> = shared
        .players
        .lock()
        .expect("список игроков захвачен другим потоком")
        .members()
        .iter()
        .filter(|member| member.dimension == dimension && lit.iter().any(|bolt| bolt.reaches((member.position.x, member.position.y, member.position.z))))
        .map(|member| member.entity_id)
        .collect();

    for entity_id in struck {
        world.note_struck(entity_id);
    }
}

/// Отложенное большим взрывом особого динамита (custom_tnt.rs): часть за
/// такт. У обычного динамита отложенного нет — здесь ничего не делается.
fn continue_blasts(shared: &Shared, dimension: Dimension, world: &mut World) {
    if !world.blast_pending() {
        return;
    }

    let went = {
        let mut items = shared.items_in(dimension).lock().expect("предметы захвачены другим потоком");
        let mut next_id = || shared.next_entity_id();

        tnt::continue_blasts(world, &mut items, &mut next_id)
    };

    if went {
        redstone::settle(world);
        drop_destroyed(shared, dimension, world);
    }
}

/// Хранилища, чей блок сломали, высыпают содержимое на месте блока.
fn spill_containers(shared: &Shared, dimension: Dimension, world: &mut World) {
    for ((x, y, z), container) in world.take_spilled() {
        // Эндер-сундук ничего не высыпает: его содержимое у игрока.
        if container.kind == containers::Kind::EnderChest {
            continue;
        }

        // Шулкер-ящик выпадает сам, с содержимым внутри (вики, Shulker Box:
        // «retains its inventory when broken»).
        let slots = if container.kind == containers::Kind::ShulkerBox
            && let Some(item) = blocks::block_at_state(container.block).and_then(blocks::item_for_block)
        {
            let stack = containers::with_contents(Stack::new(item, 1), &container.slots);
            let entity_id = shared.next_entity_id();
            let (dx, dz) = (world.random_unit() as f64 * 0.5 + 0.25, world.random_unit() as f64 * 0.5 + 0.25);

            shared
                .items_in(dimension)
                .lock()
                .expect("предметы захвачены другим потоком")
                .drop_item(entity_id, stack, (x as f64 + dx, y as f64 + 0.25, z as f64 + dz), (0.0, 0.2, 0.0), items::MINED_DELAY);

            Vec::new()
        } else {
            container.slots
        };

        for stack in slots.into_iter().flatten() {
            let entity_id = shared.next_entity_id();
            let (dx, dz) = (world.random_unit() as f64 * 0.8 + 0.1, world.random_unit() as f64 * 0.8 + 0.1);

            shared
                .items_in(dimension)
                .lock()
                .expect("предметы захвачены другим потоком")
                .drop_item(entity_id, stack, (x as f64 + dx, y as f64 + 0.5, z as f64 + dz), (0.0, 0.2, 0.0), items::MINED_DELAY);
        }

        // Сломанная печь роняет и накопленный опыт (вики, Smelting: «any
        // accumulated experience … dropped»).
        let points = if container.heat.xp > 0.0 { crate::furnace::whole_points(container.heat.xp, world.random_unit()) } else { 0 };

        if points > 0 {
            let mut rng = crate::mob::Rng::new(u64::from(shared.next_entity_id() as u32) ^ world.tick());

            shared.items_in(dimension).lock().expect("предметы захвачены другим потоком").drop_experience(
                points,
                (x as f64 + 0.5, y as f64 + 0.5, z as f64 + 0.5),
                &mut || shared.next_entity_id(),
                &mut rng,
            );
        }
    }
}

/// Такт жидкости: она пересчитывает свой уровень и растекается.
///
/// Как у оригинала, жидкость «толкает»: такт есть только у самой жидкости,
/// пустое место своего такта не ждёт — его заливает соседняя жидкость своим
/// тактом (`fluids::spreading`). Такт, заведённый для жидкости, которой
/// здесь больше нет или которая сменилась (источник стал текущей водой и
/// наоборот), пропадает (`World::fluid_tick_holds`).
pub(crate) fn flow(world: &mut World, x: i32, y: i32, z: i32) {
    let holds = world.fluid_tick_holds((x, y, z));
    world.begin_tick((x, y, z), TickKind::Fluid);

    if !holds {
        return;
    }

    // Правилам нужно смотреть на несколько блоков вперёд, поэтому им даётся
    // не набор соседей, а способ спросить любое место мира. Смотреть и менять
    // одновременно нельзя, поэтому сперва считаем, и только потом ставим.
    let wanted = {
        let view = |x, y, z| world.get_block(x, y, z);

        fluids::in_world(world.is_nether(), || fluids::update_with_catalog((x, y, z), &view, world.catalog()))
    };

    if let Some(state) = wanted {
        let here = world.get_block(x, y, z);

        // Лава, ставшая камнем от воды, шипит: событие мира 1501 (чёрный
        // ящик, water-meets-lava).
        if matches!(fluids::fluid_at(here), Some((fluids::Kind::Lava, _))) && fluids::fluid_at(state).is_none() {
            world.note_level_event(x, y, z, 1501, 0);
        }

        // Изменение само разошлётся клиентам и само попросит пересчитать
        // соседей: этим занимается мир.
        world.set_block(x, y, z, state);
    }

    // Высохшая или застывшая жидкость никуда не течёт.
    let targets = {
        let view = |x, y, z| world.get_block(x, y, z);

        fluids::in_world(world.is_nether(), || fluids::spreading_with_catalog((x, y, z), &view, world.catalog()))
    };

    for ((tx, ty, tz), state) in targets {
        let there = world.get_block(tx, ty, tz);

        // Вода, в которую сверху полилась лава, становится камнем и шипит.
        if fluids::fluid_at(there).is_some() && fluids::fluid_at(state).is_none() {
            world.note_level_event(tx, ty, tz, 1501, 0);
        }

        // Вода сносит траву, цветы, факелы, провод — и они выпадают
        // предметами. Лава сносит то же, но сжигает: ничего не выпадает
        // (вики, Water и Lava).
        if matches!(fluids::fluid_at(state), Some((fluids::Kind::Water, _))) && fluids::washed_away(there) {
            world.note_destroyed(tx, ty, tz);
        }

        if crate::campfire::lit(there) && !crate::campfire::lit(state) { crate::campfire::set_lit(world, (tx, ty, tz), false); }
        world.set_block(tx, ty, tz, state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks;
    use crate::fluids::Kind;
    use crate::world::AIR;

    #[test]
    fn mob_crossbow_multishot_and_piercing_use_the_actual_weapon_snapshot() {
        let stack = |name: &str, enchanted: bool| {
            let mut data = crate::stackdata::Data::default();
            if enchanted {
                for (name, level) in [("multishot", 1), ("piercing", 4)] {
                    data.set_enchant(crate::stackdata::enchantment_number(name).unwrap(), level);
                }
            }
            crate::inventory::Stack::new(blocks::item_named(name).unwrap(), 1).with_extras(data)
        };
        let mut fired = crate::mob::Fired { kind: crate::mob::Kind::Pillager, shooter: 4,
            shot: crate::mob::hostile::Shot { from: (0.5, 1.5, 0.5), aim: (0.5, 0.6, 8.5), speed: 1.6, trident: false, crossbow: true },
            weapon: Some(stack("crossbow", false)) };
        let plain = mob_projectiles(&fired, Dimension::Overworld, 3, &mut crate::mob::Rng::new(42), &mut || 10);
        assert_eq!(plain.len(), 1);
        assert_eq!(plain[0].pierce_left, 0);
        let mut id = 10;
        fired.weapon = Some(stack("crossbow", true));
        let triple = mob_projectiles(&fired, Dimension::Overworld, 3, &mut crate::mob::Rng::new(42), &mut || { id += 1; id });
        assert_eq!(triple.len(), 3);
        assert!(triple.iter().all(|a| a.pierce_left == 4 && a.crossbow && a.owner == 4 && !a.crit));
        assert_eq!((triple[0].vx, triple[0].vy, triple[0].vz), (plain[0].vx, plain[0].vy, plain[0].vz));
        assert!(triple[1].vx < triple[0].vx && triple[2].vx > triple[0].vx);
        assert_eq!(triple.iter().map(|a| a.entity_id).collect::<Vec<_>>(), vec![11, 12, 13]);
        fired.weapon = Some(stack("stick", true));
        let wrong_item = mob_projectiles(&fired, Dimension::Overworld, 3, &mut crate::mob::Rng::new(42), &mut || 20);
        assert_eq!(wrong_item.len(), 1);
        assert_eq!(wrong_item[0].pierce_left, 0);
    }

    /// Камень.
    const STONE: i32 = 1;

    /// Прогоняет столько тактов мира.
    fn run(world: &mut World, ticks: usize) {
        for _ in 0..ticks {
            for ((x, y, z), kind) in world.advance(PER_TICK) {
                if kind == TickKind::Fluid {
                    flow(world, x, y, z);
                }
            }
        }
    }

    /// Ровный каменный пол на высоте 63, от -12 до 12 по обеим осям.
    fn floor() -> World {
        let mut world = World::in_memory();

        for x in -12..=12 {
            for z in -12..=12 {
                world.set_block_silently(x, 63, z, STONE);
            }
        }

        world
    }

    fn water(level: i32) -> i32 {
        fluids::state_of(Kind::Water, level)
    }

    /// Источник забрали (раздатчик с ведром): на его место вода от соседей
    /// возвращается их тактом, через 5 тактов, вместе с их спадом (чёрный
    /// ящик, dispenser-water).
    #[test]
    fn a_taken_source_refills_with_the_neighbours() {
        let mut world = floor();

        world.set_block(0, 64, 0, water(0));
        run(&mut world, 60);
        assert_eq!(world.get_block(1, 64, 0), water(1));

        world.set_block(0, 64, 0, AIR);
        redstone::settle(&mut world);

        for tick in 1..=5 {
            run(&mut world, 1);
            let here = world.get_block(0, 64, 0);

            if tick < 5 {
                assert_eq!(here, AIR, "вода вернулась через {tick}");
            }
        }

        assert_eq!(world.get_block(0, 64, 0), water(2));
    }

    /// Спад после забранного источника идёт волнами: на его место вода
    /// возвращается уровнем 2, потом 4, потом 6 — место пересчитывается
    /// после соседей, чей такт завёлся раньше (чёрный ящик, dispenser-water).
    #[test]
    fn a_taken_source_recedes_in_waves() {
        let mut world = floor();

        // С запада — раздатчик, как в сценарии: от него зависит, чей такт
        // заведётся раньше.
        world.set_block(-1, 64, 0, STONE);
        world.set_block(0, 64, 0, water(0));
        run(&mut world, 60);
        world.set_block(0, 64, 0, AIR);

        let mut seen = Vec::new();

        for _ in 0..3 {
            run(&mut world, 5);
            seen.push(fluids::fluid_at(world.get_block(0, 64, 0)).map(|(_, level)| level));
        }

        assert_eq!(seen, [Some(2), Some(4), Some(6)]);
    }

    /// Такт, заведённый для источника, пропадает, если источник стал текущей
    /// водой: у оригинала это разные жидкости.
    #[test]
    fn a_tick_for_a_source_does_not_serve_flowing_water() {
        let mut world = floor();

        world.set_block(0, 64, 0, water(0));
        assert!(world.fluid_tick_holds((0, 64, 0)));

        world.set_block_silently(0, 64, 0, water(3));
        assert!(!world.fluid_tick_holds((0, 64, 0)));
    }

    /// То же с настоящим раздатчиком: включаясь, он будит соседний источник,
    /// но этот такт пропадает вместе с источником, которое забирает ведро, —
    /// вода возвращается тактом соседей, через 5 после забора.
    #[test]
    fn a_dispenser_takes_the_source_and_water_returns_with_the_neighbours() {
        let mut world = floor();
        let dispenser = blocks::orientation("dispenser").unwrap().state(&[("facing", "east"), ("triggered", "false")]).unwrap();
        world.set_block(-1, 64, 0, dispenser);
        redstone::settle(&mut world);
        let bucket = blocks::item_named("bucket").unwrap();
        world.with_container(-1, 64, 0, |container| container.slots[0] = Some(crate::inventory::Stack::new(bucket, 1)));

        world.set_block(0, 64, 0, water(0));
        redstone::settle(&mut world);

        let full = |world: &mut World| {
            for ((x, y, z), kind) in world.advance(PER_TICK) {
                match kind {
                    TickKind::Block => redstone::ticked(world, (x, y, z)),
                    TickKind::Fluid => flow(world, x, y, z),
                }
                redstone::settle(world);
            }
        };

        for _ in 0..60 {
            full(&mut world);
        }

        world.set_block(-1, 65, 0, blocks::state_by_name("redstone_block").unwrap());
        redstone::settle(&mut world);

        let mut taken = None;

        for tick in 1..=12 {
            full(&mut world);
            let here = world.get_block(0, 64, 0);

            match taken {
                None if here == AIR => taken = Some(tick),
                Some(at) if fluids::fluid_at(here).is_some() => {
                    assert_eq!(tick - at, 5, "вода вернулась не через 5 тактов");
                    return;
                }
                _ => {}
            }
        }

        panic!("вода не вернулась");
    }

    /// Сколько гравия и песка падает в только что сложенном обычном мире и
    /// от чего: у оригинала такие «потолки» висят, пока их не тронут.
    /// `cargo test --release falling_at_load -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn falling_at_load() {
        let path = std::env::temp_dir().join(format!("mc-falling-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        let mut world = World::open(&path, Some(100_554_032_945_340), crate::config::server_properties::WorldKind::Normal)
            .expect("мир открылся");

        for chunk_x in 0..10 {
            for chunk_z in -10..0 {
                world.ensure(chunk_x, chunk_z);
            }
        }

        let (mut fluid_ticks, mut block_ticks, mut falls) = (0, 0, 0);
        let mut first = Vec::new();

        for tick in 0..200 {
            for ((x, y, z), kind) in world.advance(PER_TICK) {
                match kind {
                    TickKind::Block => {
                        block_ticks += 1;
                        redstone::ticked(&mut world, (x, y, z));
                    }
                    TickKind::Fluid => {
                        fluid_ticks += 1;
                        flow(&mut world, x, y, z);
                    }
                }
                redstone::settle(&mut world);
            }

            let falling = world.take_falling();
            if first.len() < 10 {
                first.extend(falling.iter().map(|place| (tick, *place)));
            }
            falls += falling.len();
        }

        // Текущая вода и лава в этих чанках после тактов (у оригинала в
        // сложенных чанках — меньше блока на чанк).
        let mut flowing = std::collections::HashMap::new();
        for chunk_x in 1..9 {
            for chunk_z in -9..-1 {
                for x in chunk_x * 16..chunk_x * 16 + 16 {
                    for z in chunk_z * 16..chunk_z * 16 + 16 {
                        for y in crate::world::MIN_Y..200 {
                            if let Some((kind, level)) = fluids::fluid_at(world.get_block(x, y, z))
                                && level != fluids::SOURCE
                            {
                                *flowing.entry((format!("{:?}", kind), y.div_euclid(16) * 16)).or_insert(0) += 1;
                            }
                        }
                    }
                }
            }
        }
        let mut rows: Vec<_> = flowing.into_iter().collect();
        rows.sort();
        println!("текущая жидкость на 64 чанка по высотам: {:?}", rows);

        println!("тактов жидкости {}, тактов блоков {}, падений {}", fluid_ticks, block_ticks, falls);
        println!("первые падения (такт, место): {:?}", first);
        let _ = std::fs::remove_dir_all(&path);
    }

    /// Водопад со столба: на каком такте вода приходит в клетки пола.
    /// Сверка с оригиналом (сценарий waterfall): низ водопада +24, первый
    /// уровень +29, второй +34, третий +39 от постановки источника.
    #[test]
    fn waterfall_timing_matches_the_original() {
        let mut world = floor();

        for y in 64..=67 {
            world.set_block_silently(0, y, 0, STONE);
        }

        world.set_block(0, 68, 0, water(fluids::SOURCE));

        let mut first: std::collections::HashMap<(i32, i32, i32), usize> = std::collections::HashMap::new();
        let watch = [(1, 64, 0), (2, 64, 0), (3, 64, 0), (4, 64, 0), (3, 64, -1), (5, 64, 0)];

        for tick in 1..=60 {
            run(&mut world, 1);

            for place in watch {
                if fluids::fluid_at(world.get_block(place.0, place.1, place.2)).is_some() {
                    first.entry(place).or_insert(tick);
                }
            }
        }

        // Постановка источника — такт 0; у оригинала она уходит в такте +0,
        // а первое растекание — в +4, то есть через 5 тактов после неё.
        let expected = [((1, 64, 0), 24), ((2, 64, 0), 29), ((3, 64, 0), 34), ((4, 64, 0), 39), ((3, 64, -1), 39)];

        for (place, tick) in expected {
            assert_eq!(first.get(&place).copied(), Some(tick + 1), "{:?}: {:?}", place, first);
        }
    }

    /// Предмет в ручье разгоняется по течению и всплывает так же, как у
    /// оригинала: скорость по течению за такт — 0,02759, 0,05432, 0,08033,
    /// 0,10548…, вверх — 0,00049, 0,00098, 0,00146… (снято
    /// tools/blackbox/itemflow.js с сервера 26.1.2).
    #[test]
    fn an_item_drifts_down_a_stream_like_the_original() {
        let mut world = floor();

        // Жёлоб вдоль x: стенки по бокам, сзади камень, источник в начале.
        for x in -1..=12 {
            world.set_block_silently(x, 64, -1, STONE);
            world.set_block_silently(x, 64, 1, STONE);
        }
        world.set_block_silently(-1, 64, 0, STONE);
        world.set_block(0, 64, 0, water(fluids::SOURCE));
        run(&mut world, 60);

        let mut items = crate::items::Items::new();
        items.drop_item(1, crate::inventory::Stack::new(1, 1), (1.5, 64.2, 0.5), (0.0, 0.0, 0.0), 1200);

        let solid = |x, y, z| blocks::stops_items(world.get_block(x, y, z));
        let loaded = |_, _| true;
        let water = |x, y, z| {
            let view = |x, y, z| world.get_block(x, y, z);
            fluids::item_water((x, y, z), &view)
        };

        let mut seen = Vec::new();

        for _ in 0..8 {
            items.step_with_water(&solid, &solid, &loaded, &water);
            let item = &items.items()[0];
            seen.push((item.vx, item.vy));
        }

        let expected_x = [0.02759, 0.05432, 0.08033, 0.10548, 0.12995, 0.15363, 0.17665, 0.19899];
        let expected_y = [0.00049, 0.00098, 0.00146, 0.00189, 0.00238, 0.00281, 0.00324, 0.00366];

        for (tick, ((vx, vy), (ex, ey))) in seen.iter().zip(expected_x.iter().zip(expected_y.iter())).enumerate() {
            assert!((vx - ex).abs() < 0.0003, "такт {}: по течению {} вместо {} ({:?})", tick + 1, vx, ex, seen);
            assert!((vy - ey).abs() < 0.0001, "такт {}: вверх {} вместо {} ({:?})", tick + 1, vy, ey, seen);
        }
    }

    /// Вода от источника растекается на семь блоков и дальше не идёт.
    #[test]
    fn water_spreads_seven_blocks() {
        let mut world = floor();

        world.set_block(0, 64, 0, water(fluids::SOURCE));
        run(&mut world, 100);

        assert_eq!(world.get_block(1, 64, 0), water(1));
        assert_eq!(world.get_block(4, 64, 0), water(4));
        assert_eq!(world.get_block(7, 64, 0), water(7));
        assert_eq!(world.get_block(8, 64, 0), AIR);

        // Растекается во все стороны, а не только по одной оси.
        assert_eq!(world.get_block(-7, 64, 0), water(7));
        assert_eq!(world.get_block(0, 64, 7), water(7));
    }

    /// Дыра в полу: вода уходит в неё, а не течёт дальше поверху.
    ///
    /// Проверяется в жёлобе — канавке со стенками: на открытом полу вода
    /// просто обтекла бы дыру сбоку, и о самой дыре это ничего не сказало бы.
    #[test]
    fn water_falls_into_a_hole() {
        let mut world = floor();

        for x in -1..=8 {
            world.set_block_silently(x, 64, -1, STONE);
            world.set_block_silently(x, 64, 1, STONE);
        }
        world.set_block_silently(-1, 64, 0, STONE);

        world.set_block_silently(3, 63, 0, AIR);
        world.set_block(0, 64, 0, water(fluids::SOURCE));
        run(&mut world, 200);

        // В дыру налилось.
        assert!(fluids::fluid_at(world.get_block(3, 63, 0)).is_some());

        // А дальше по поверхности вода не пошла.
        assert_eq!(world.get_block(4, 64, 0), AIR);
    }

    /// Убрали источник — вода высыхает вся.
    #[test]
    fn water_dries_up_when_the_source_is_gone() {
        let mut world = floor();

        world.set_block(0, 64, 0, water(fluids::SOURCE));
        run(&mut world, 100);
        assert_ne!(world.get_block(5, 64, 0), AIR);

        world.set_block(0, 64, 0, AIR);
        run(&mut world, 200);

        for x in -8..=8 {
            assert_eq!(world.get_block(x, 64, 0), AIR, "осталась вода в {}", x);
        }
    }

    /// Лава, дотёкшая до воды, превращается в булыжник.
    #[test]
    fn lava_meeting_water_turns_to_stone() {
        let cobblestone = blocks::states_of("cobblestone").unwrap().0;
        let mut world = floor();

        // Лава успела растечься на шаг, прежде чем рядом поставили воду
        // (как в сценарии чёрного ящика water-meets-lava).
        world.set_block(0, 64, 0, fluids::state_of(Kind::Lava, fluids::SOURCE));
        run(&mut world, 35);
        world.set_block(3, 64, 0, water(fluids::SOURCE));

        run(&mut world, 400);

        // Камень встаёт там, где вода дотекла до лавы, — рядом с источником
        // лавы, потому что вода растекается быстрее.
        assert_eq!(world.get_block(1, 64, 0), cobblestone);

        // Вода на месте, лава дальше камня не прошла.
        assert!(fluids::fluid_at(world.get_block(3, 64, 0)).is_some());
        assert_eq!(fluids::fluid_at(world.get_block(2, 64, 0)).map(|(kind, _)| kind), Some(Kind::Water));
    }

    /// Когда всё утекло, мир перестаёт пересчитываться: стоячая вода ничего
    /// не стоит.
    #[test]
    fn a_settled_world_stops_working() {
        let mut world = floor();

        world.set_block(0, 64, 0, water(fluids::SOURCE));
        run(&mut world, 300);

        assert_eq!(world.scheduled_count(), 0);
    }

    /// Бесконечная вода: между двумя источниками появляется третий, и он
    /// восстанавливается, сколько его ни черпай.
    #[test]
    fn two_sources_make_an_endless_one() {
        let mut world = floor();

        world.set_block(-1, 64, 0, water(fluids::SOURCE));
        world.set_block(1, 64, 0, water(fluids::SOURCE));
        run(&mut world, 50);

        assert_eq!(world.get_block(0, 64, 0), water(fluids::SOURCE));

        // Зачерпнули — набралось снова.
        world.set_block(0, 64, 0, AIR);
        run(&mut world, 50);

        assert_eq!(world.get_block(0, 64, 0), water(fluids::SOURCE));
    }

    /// Вода смывает цветок, и он выпадает; лава сжигает — без выпадения.
    #[test]
    fn water_washes_a_flower_away_and_lava_burns_it() {
        let dirt = blocks::state_by_name("dirt").unwrap();
        let poppy = blocks::state_by_name("poppy").unwrap();

        for (kind, drops) in [(Kind::Water, true), (Kind::Lava, false)] {
            let mut world = floor();

            world.set_block_silently(1, 63, 0, dirt);
            world.set_block_silently(1, 64, 0, poppy);
            world.set_block(0, 64, 0, fluids::state_of(kind, fluids::SOURCE));
            run(&mut world, 100);

            assert!(fluids::fluid_at(world.get_block(1, 64, 0)).is_some(), "{kind:?} не затекла");
            assert_eq!(
                world.take_destroyed().iter().any(|(place, state)| *place == (1, 64, 0) && *state == poppy),
                drops,
                "{kind:?}"
            );
        }
    }

    /// Рыхлый снег рядом с тем, что поменялось, никуда не девается.
    #[test]
    fn powder_snow_stays_when_a_neighbour_changes() {
        let mut world = floor();
        let snow = blocks::state_by_name("powder_snow").unwrap();

        world.set_block(0, 64, 0, snow);
        world.set_block(1, 64, 0, STONE);
        world.set_block(1, 64, 0, AIR);
        run(&mut world, 20);

        assert_eq!(world.get_block(0, 64, 0), snow);
    }

    /// Бетонная пыль, упавшая в воду, становится бетоном; песок — песком.
    #[test]
    fn concrete_powder_hardens_in_water() {
        let powder = blocks::state_by_name("red_concrete_powder").unwrap();
        let concrete = blocks::state_by_name("red_concrete").unwrap();
        let sand = blocks::state_by_name("sand").unwrap();
        let water = water(3);

        assert_eq!(hardened_in_water(powder, water), concrete);
        assert_eq!(hardened_in_water(powder, AIR), powder);
        assert_eq!(hardened_in_water(sand, water), sand);
    }

    /// Ручей к обрыву получается узким: вода идёт к спуску, а не разливается
    /// во все стороны.
    #[test]
    fn a_stream_to_a_cliff_stays_narrow() {
        let mut world = floor();

        // Обрыв: пола нет начиная с четвёртого блока к востоку.
        for x in 4..=12 {
            for z in -12..=12 {
                world.set_block_silently(x, 63, z, AIR);
            }
        }

        world.set_block(0, 64, 0, water(fluids::SOURCE));
        run(&mut world, 200);

        // К обрыву вода дошла и полилась вниз.
        assert!(fluids::fluid_at(world.get_block(4, 64, 0)).is_some());
        assert!(fluids::fluid_at(world.get_block(4, 63, 0)).is_some());

        // А вбок от источника не растеклась: спуск был только в одну сторону.
        assert_eq!(world.get_block(-1, 64, 0), AIR);
        assert_eq!(world.get_block(0, 64, 3), AIR);
    }

    /// Рамка без опоры отваливается ровно на сотом такте после появления, а не
    /// раньше (сценарии item-frame-break и item-frame-facing); с опорой —
    /// держится. Вместе с ней выпадает предмет рамки.
    #[test]
    fn a_frame_without_support_falls_on_its_hundredth_tick() {
        let shared = Shared::new(
            World::in_memory(),
            World::in_memory(),
            std::path::PathBuf::new(),
            std::path::PathBuf::new(),
            Default::default(),
            crate::ops::Ops::open(&std::env::temp_dir().join(format!("mcsheriffanya-tick-frame-ops-{}.json", std::process::id()))),
            crate::skins::Settings::default(),
        );
        let overworld = Dimension::Overworld;
        let mut world = shared.world_in(overworld).lock().unwrap();

        world.set_block_silently(1, 64, 0, STONE);
        world.set_block_silently(1, 64, 5, STONE);

        {
            let mut items = shared.items_in(overworld).lock().unwrap();

            // Одна рамка на камне, другая — на камне, который потом уберут.
            assert!(crate::frame::place(&mut world, &mut items, &shared, (1, 64, 0), crate::frame::WEST, false));
            assert!(crate::frame::place(&mut world, &mut items, &shared, (1, 64, 5), crate::frame::WEST, false));
        }

        world.set_block_silently(1, 64, 5, AIR);

        let frames = |world: &mut World| {
            let _ = world;
            shared.items_in(overworld).lock().unwrap().frames().len()
        };

        for _ in 0..100 {
            step_frames(&shared, overworld, &mut world);
        }

        assert_eq!(frames(&mut world), 2, "раньше сотого такта рамка держится");

        step_frames(&shared, overworld, &mut world);
        assert_eq!(frames(&mut world), 1, "на сотом отвалилась только висящая в воздухе");
        assert_eq!(shared.items_in(overworld).lock().unwrap().items().len(), 1, "выпала сама рамка");

        for _ in 0..300 {
            step_frames(&shared, overworld, &mut world);
        }

        assert_eq!(frames(&mut world), 1, "с опорой висит и дальше");
    }

    /// Общее состояние для проверок мобов: два пустых мира в памяти.
    fn mob_server() -> Shared {
        Shared::new(
            World::in_memory(),
            World::in_memory(),
            std::path::PathBuf::new(),
            std::path::PathBuf::new(),
            Default::default(),
            crate::ops::Ops::open(&std::env::temp_dir().join(format!("mcsheriffanya-tick-mob-ops-{}.json", std::process::id()))),
            crate::skins::Settings::default(),
        )
    }

    #[test]
    fn fresh_flat_chunks_do_not_generate_uncapped_animal_herds() {
        let shared = mob_server();
        shared.spawn_mobs.store(true, std::sync::atomic::Ordering::Relaxed);
        let dimension = Dimension::Overworld;
        let mut world = shared.world_in(dimension).lock().unwrap();
        assert!(world.is_flat());
        for cx in 0..20 { for cz in 0..20 { world.ensure(cx, cz); } }
        step_mobs(&shared, dimension, &mut world, &mut crate::mob::Rng::new(71), 1);
        assert!(shared.items_in(dimension).lock().unwrap().mobs().is_empty());
    }

    #[test]
    fn falling_dripstone_lands_with_1045_and_drops_the_item() {
        let shared = mob_server();
        let dimension = Dimension::Overworld;
        let mut w = shared.world_in(dimension).lock().unwrap();
        w.set_block_silently(0, 0, 0, blocks::state_by_name("stone").unwrap());
        let state = blocks::state_from_text("pointed_dripstone[vertical_direction=down]").unwrap();
        w.set_block_silently(0, 3, 0, state);
        crate::dripstone::ticked(&mut w, (0, 3, 0));
        start_falling(&shared, dimension, &mut w);
        {
            let mut items = shared.items_in(dimension).lock().unwrap();
            for _ in 0..30 {
                items.step(&|_, y, _| y <= 0, &|_, y, _| y <= 0, &|_, _| true);
            }
        }
        let cursor = w.watch_events(1);
        land_falling(&shared, dimension, &mut w);
        let events = w.events_since(cursor, 1).0;
        let levels: Vec<_> = events.iter().filter_map(|e| match e {
            crate::world::WorldEvent::Level { x, y, z, effect, data, .. } => Some((*x, *y, *z, *effect, *data)),
            _ => None,
        }).collect();
        assert_eq!(levels, vec![(0, 1, 0, 1045, 0)]);
        assert_eq!(w.get_block(0, 1, 0), AIR);
        let items = shared.items_in(dimension).lock().unwrap();
        assert_eq!(items.items().len(), 1);
        assert!(items.items()[0].block.is_none());
        assert_eq!(items.items()[0].stack, Stack::new(blocks::item_named("pointed_dripstone").unwrap(), 1));
    }

    #[test]
    fn scaffolding_and_dripstone_enter_the_real_falling_entity_queue() {
        let shared = mob_server();
        let dimension = Dimension::Overworld;
        let mut w = shared.world_in(dimension).lock().unwrap();
        let scaffold = blocks::state_from_text("scaffolding[distance=7,waterlogged=true]").unwrap();
        let drip = blocks::state_from_text("pointed_dripstone[vertical_direction=down]").unwrap();
        w.set_block_silently(0, 10, 0, scaffold);
        w.set_block_silently(2, 10, 0, drip);
        w.set_block_silently(2, 9, 0, drip);
        crate::scaffolding::ticked(&mut w, (0, 10, 0));
        crate::dripstone::ticked(&mut w, (2, 10, 0));
        start_falling(&shared, dimension, &mut w);
        assert_eq!(fluids::fluid_at(w.get_block(0, 10, 0)), Some((fluids::Kind::Water, 0)));
        assert_eq!(w.get_block(2, 10, 0), AIR);
        assert_eq!(w.get_block(2, 9, 0), AIR);
        let items = shared.items_in(dimension).lock().unwrap();
        assert_eq!(items.items().len(), 3);
        let first = items.items()[0].block.unwrap();
        assert_eq!(blocks::orientation_of(first).unwrap().value(first, "waterlogged"), Some("false"));
    }

    /// Мобы в такте мира падают, встают на землю, получают урон падения
    /// и через снимок отдают слежению состояние до своего хода.
    #[test]
    fn mobs_fall_in_the_world_tick_and_the_view_lags_one_step() {
        let shared = mob_server();
        let overworld = Dimension::Overworld;
        let mut world = shared.world_in(overworld).lock().unwrap();
        let mut rng = crate::mob::Rng::new(1);
        let start = f64::from(crate::world::flat::SURFACE) + 6.0;

        world.ensure(0, 0);
        shared.items_in(overworld).lock().unwrap().add_mob(crate::mob::Mob::new(1, crate::mob::Kind::Cow, (8.5, start, 8.5)));

        step_mobs(&shared, overworld, &mut world, &mut rng, 1);

        {
            let items = shared.items_in(overworld).lock().unwrap();

            // Снимок — до хода: корова ещё на месте, а сама уже сделала ход.
            assert_eq!(items.mob_view()[0].ticks, 0);
            assert_eq!(items.mob_view()[0].y, start);
            assert_eq!(items.mobs()[0].ticks, 1);
        }

        for tick in 2..70 {
            step_mobs(&shared, overworld, &mut world, &mut rng, tick);
        }

        let items = shared.items_in(overworld).lock().unwrap();
        let cow = &items.mobs()[0];

        assert!(cow.on_ground);
        assert_eq!(cow.y, f64::from(crate::world::flat::SURFACE));
        assert_eq!(cow.vitals.health, 7.0, "падение с шести блоков: урон ceil(6 − 3) = 3");
    }

    /// Плиты и растяжки видят мобов: живые, они жмут и каменные плиты.
    #[test]
    fn mobs_are_living_occupants() {
        let shared = mob_server();
        let overworld = Dimension::Overworld;

        shared.items_in(overworld).lock().unwrap().add_mob(crate::mob::Mob::new(1, crate::mob::Kind::Pig, (3.5, 64.0, 3.5)));

        let (standing, _) = occupants(&shared, overworld);

        assert_eq!(standing.len(), 1);
        assert!(standing[0].living);
        assert_eq!((standing[0].width, standing[0].height), (0.9, 0.9));
    }

    /// Съеденная овцой трава становится землёй, снесённое курицей яйцо
    /// падает предметом, детёныш у пары в любви рождается.
    #[test]
    fn mobs_change_the_world_around_them() {
        let shared = mob_server();
        let overworld = Dimension::Overworld;
        let mut world = shared.world_in(overworld).lock().unwrap();
        let mut rng = crate::mob::Rng::new(3);
        let surface = crate::world::flat::SURFACE;

        world.ensure(0, 0);
        world.set_block_silently(4, surface - 1, 4, blocks::state_by_name("grass_block").unwrap());

        {
            let mut items = shared.items_in(overworld).lock().unwrap();
            let mut sheep = crate::mob::Mob::new(1, crate::mob::Kind::Sheep, (4.5, f64::from(surface), 4.5));
            let mut hen = crate::mob::Mob::new(2, crate::mob::Kind::Chicken, (9.5, f64::from(surface), 9.5));

            sheep.sheared = true;
            sheep.brain.eating = 5;
            hen.egg_timer = 5;
            items.add_mob(sheep);
            items.add_mob(hen);
        }

        for tick in 1..=10 {
            step_mobs(&shared, overworld, &mut world, &mut rng, tick);
        }

        let eggs = shared.items_in(overworld).lock().unwrap().items().iter().filter(|item| blocks::item_name(item.stack.item) == Some("egg")).count();

        assert_eq!(eggs, 1, "курица снесла яйцо на пятом такте");
        assert_eq!(world.get_block(4, surface - 1, 4), blocks::state_by_name("dirt").unwrap(), "дёрн под овцой стал землёй");
        assert!(!shared.items_in(overworld).lock().unwrap().mobs()[0].sheared, "шерсть отросла");
    }

    /// В ровном мире новые чанки сами не создают стада: включённое правило
    /// разрешает только последующие циклы при наличии игрока.
    #[test]
    fn flat_chunk_generation_never_bypasses_the_periodic_creature_cap() {
        let shared = mob_server();
        let overworld = Dimension::Overworld;
        let mut rng = crate::mob::Rng::new(4);

        // Ровный мир с травой: раньше сюда ошибочно попадал шанс 10 %.
        {
            let mut world = shared.world_in(overworld).lock().unwrap();

            for x in 0..40 {
                world.ensure(x, 0);
            }

            step_mobs(&shared, overworld, &mut world, &mut rng, 1);
        }

        assert!(shared.items_in(overworld).lock().unwrap().mobs().iter().all(|mob| !mob.persistent || mob.ticks > 0), "мобы появились и сделали ход");

        let with_rule = shared.items_in(overworld).lock().unwrap().mobs().len();

        assert_eq!(with_rule, 0, "стада не создаются вместе с плоскими чанками");

        let quiet = mob_server();

        quiet.spawn_mobs.store(false, std::sync::atomic::Ordering::Relaxed);

        {
            let mut world = quiet.world_in(overworld).lock().unwrap();

            for x in 0..40 {
                world.ensure(x, 0);
            }

            step_mobs(&quiet, overworld, &mut world, &mut rng, 1);
        }

        assert!(quiet.items_in(overworld).lock().unwrap().mobs().is_empty(), "правило выключено — никого");
    }
}

#[cfg(test)]
#[path = "tick/sculk_step_tests.rs"]
mod sculk_step_tests;
