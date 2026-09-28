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
    let mut ticks: u64 = 0;

    loop {
        tokio::time::sleep_until(next).await;

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

        next += TICK;
        ticks += 1;

        let started = std::time::Instant::now();

        // Сперва — пакеты игроков, что накопились за прошлый такт: их
        // применяют сами подключения, а такт ждёт, пока они закончат.
        // Ждём недолго: застрявшее подключение не должно останавливать мир.
        shared.tick_starts.send_replace(ticks);

        let patience = tokio::time::Instant::now() + Duration::from_millis(5);

        while shared.waiting_for_tick.load(std::sync::atomic::Ordering::Acquire) > 0
            && tokio::time::Instant::now() < patience
        {
            tokio::task::yield_now().await;
        }

        // Работа такта — в своём блоке: замок мира отпускается до сигнала
        // подключениям. Миры измерений идут по очереди, каждый со своими
        // предметами; два мира разом не держатся.
        for dimension in Dimension::ALL {
            let mut world = shared.world_in(dimension).lock().expect("мир захвачен другим потоком");

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
            }

            // Поршни трогаются отдельной фазой, после всех запланированных
            // тактов: так в игре. Заведённое во время этой фазы ждёт следующего
            // такта — иначе цепочка поршней срабатывала бы вся разом.
            for (x, y, z) in world.take_block_events() {
                redstone::block_event(&mut world, (x, y, z));
                redstone::settle(&mut world);
            }

            // Плиты слушают не сигнал, а мир: на них встают игроки и падают
            // предметы. Собираем, кто где стоит, и отдаём плитам.
            let mut standing: Vec<(f64, f64, f64)> = shared
                .players
                .lock()
                .expect("список игроков захвачен другим потоком")
                .members()
                .iter()
                .filter(|member| member.dimension == dimension)
                .map(|member| (member.position.x, member.position.y, member.position.z))
                .collect();

            standing.extend(
                shared
                    .items_in(dimension)
                    .lock()
                    .expect("предметы захвачены другим потоком")
                    .items()
                    .iter()
                    .map(|item| (item.x, item.y, item.z)),
            );

            redstone::step_plates(&mut world, &standing);

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

            let splashes = shared
                .items_in(dimension)
                .lock()
                .expect("предметы захвачены другим потоком")
                .step_with_water(&solid, &holds_block, &loaded, &water);

            // Предмет вошёл в воду — плеск, как у оригинала.
            for splash in splashes {
                let pitch = 1.0 + (world.random_unit() - world.random_unit()) * 0.4;
                world.note_sound_at(splash.at, "minecraft:entity.generic.splash", splash.volume, pitch);
            }

            // Долетевшие до земли снова становятся блоками.
            land_falling(&shared, dimension, &mut world);

            if ticks.is_multiple_of(SAVE_EVERY) {
                world.save_if_needed();
            }

            if ticks.is_multiple_of(UNLOAD_EVERY) {
                unload_empty_chunks(&shared, dimension, &mut world);
            }

        }

        // Работа такта сделана: подключения рассылают, что случилось.
        // Долгий такт — повод разобраться: подключения узнают о нём позже,
        // и схемы у игроков начинают запаздывать.
        let spent = started.elapsed();

        if spent > SLOW_TICK {
            log_debug!("Такт {} занял {} мс", ticks, spent.as_millis());
        }

        shared.ticks.send_replace(ticks);
    }
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

    let unloaded = world.unload_far(|x, z| {
        players
            .iter()
            .any(|(px, pz)| (x - px).abs() <= keep && (z - pz).abs() <= keep)
    });

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
        if !redstone::falls(state) || !redstone::free_to_fall_into(world.get_block(x, y - 1, z)) {
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

        world.set_block(x, y, z, crate::world::AIR);

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

        if redstone::free_to_fall_into(there) {
            world.set_block(x, y, z, hardened_in_water(state, there));
            redstone::settle(world);

            // Оригинал через такт повторяет клиентам приземлившийся блок
            // (снято чёрным ящиком, сценарий sand-into-water).
            world.tell_later(x, y, z, 1);
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

        let Some(stack) = blocks::drops_by_luck(state, None, luck) else {
            continue;
        };

        let entity_id = shared.next_entity_id();

        shared
            .items_in(dimension)
            .lock()
            .expect("предметы захвачены другим потоком")
            .drop_item(
                entity_id,
                stack,
                (x as f64 + 0.5, y as f64 + 0.25, z as f64 + 0.5),
                (0.0, 0.0, 0.0),
                items::MINED_DELAY,
            );
    }
}

/// Пересчитывает одно место: во что там должна превратиться жидкость.
pub(crate) fn flow(world: &mut World, x: i32, y: i32, z: i32) {
    // Правилам нужно смотреть на несколько блоков вперёд, поэтому им даётся
    // не набор соседей, а способ спросить любое место мира. Смотреть и менять
    // одновременно нельзя, поэтому сперва считаем, и только потом ставим.
    let wanted = {
        let view = |x, y, z| world.get_block(x, y, z);

        fluids::in_world(world.is_nether(), || fluids::update((x, y, z), &view))
    };

    if let Some(state) = wanted {
        let here = world.get_block(x, y, z);

        // У оригинала жидкость толкает сама: пустое место получает её на
        // такте той жидкости, что рядом, — через задержку после того, как
        // она туда пришла. Наш такт пустого места мог наступить раньше
        // (его завело другое изменение рядом), и тогда он взял бы жидкость,
        // пришедшую к соседу в этом же такте. Такое место ждёт своей
        // очереди (сценарий чёрного ящика water-meets-lava).
        if let Some((kind, _)) = fluids::fluid_at(state)
            && fluids::fluid_at(here).is_none()
            && [(0, 1, 0), (-1, 0, 0), (1, 0, 0), (0, 0, -1), (0, 0, 1)]
                .iter()
                .any(|(dx, dy, dz)| world.is_fresh_fluid(x + dx, y + dy, z + dz))
        {
            world.schedule(x, y, z, kind.delay_in(world.is_nether()));
            return;
        }

        if fluids::fluid_at(state).is_some() {
            world.note_fresh_fluid(x, y, z);
        }

        // Вода сносит траву, цветы, факелы, провод — и они выпадают
        // предметами. Лава сносит то же, но сжигает: ничего не выпадает
        // (вики, Water и Lava).

        if matches!(fluids::fluid_at(state), Some((fluids::Kind::Water, _))) && fluids::washed_away(here) {
            world.note_destroyed(x, y, z);
        }

        // Изменение само разошлётся клиентам и само попросит пересчитать
        // соседей: этим занимается мир.
        world.set_block(x, y, z, state);
        return;
    }

    // Сама жидкость не изменилась, но её такт пришёл не зря: рядом что-то
    // поменялось — например, под ней залило пустоту, и теперь ей можно
    // течь вбок. Решает за себя каждое место, поэтому пустоту рядом надо
    // спросить заново: иначе вода так и стоит у края, пока её не тронут.
    let here = world.get_block(x, y, z);

    if let Some((kind, _)) = fluids::fluid_at(here) {
        for (dx, dy, dz) in [(0, -1, 0), (-1, 0, 0), (1, 0, 0), (0, 0, -1), (0, 0, 1)] {
            let next = world.get_block(x + dx, y + dy, z + dz);

            if fluids::fluid_at(next).is_none() && fluids::flows_through(next) {
                world.schedule(x + dx, y + dy, z + dz, kind.delay_in(world.is_nether()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks;
    use crate::fluids::Kind;
    use crate::world::AIR;

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
        items.drop_item(1, crate::inventory::Stack::new(1, 1), (1.5, 64.2, 0.5), (0.0, 0.0, 0.0), Duration::from_secs(60));

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

        world.set_block(0, 64, 0, fluids::state_of(Kind::Lava, fluids::SOURCE));
        world.set_block(3, 64, 0, water(fluids::SOURCE));

        run(&mut world, 400);

        // Камень встаёт там, где лава дотекла до воды, — рядом с источником
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
}
