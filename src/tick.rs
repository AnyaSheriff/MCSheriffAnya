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
use crate::shared::Shared;
use crate::inventory::Stack;
use crate::items;
use crate::redstone;
use crate::world::{TickKind, World};

/// Сколько длится такт.
const TICK: Duration = Duration::from_millis(50);

/// Как часто мир записывается на диск: раз в секунду.
const SAVE_EVERY: u64 = 20;

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
    let mut ticker = tokio::time::interval(TICK);

    // Если такт где-то задержался, догонять пропущенное незачем: мир просто
    // пойдёт дальше, а не будет наверстывать рывком. Но и сетку тактов сдвигать
    // нельзя: `Delay` отсчитывает следующий такт от пробуждения, и опоздание на
    // пару миллисекунд копится — за полминуты набегает лишний такт, схемы
    // начинают отставать от часов мира. `Skip` пропускает потерянное и
    // возвращается на исходную сетку по 50 мс.
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    let mut ticks: u64 = 0;

    loop {
        ticker.tick().await;
        ticks += 1;

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
        // подключениям.
        {
            let mut world = shared.world.lock().expect("мир захвачен другим потоком");

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
                .map(|member| (member.position.x, member.position.y, member.position.z))
                .collect();

            standing.extend(
                shared
                    .items
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
            drop_destroyed(&shared, &mut world);

            // Песок и гравий, под которыми не стало опоры, отправляются в полёт.
            start_falling(&shared, &mut world);

            // Выброшенные предметы летят и падают. О блоки они упираются, поэтому
            // мир нужен и здесь.
            let solid = |x, y, z| blocks::solid_top(world.get_block(x, y, z));

            shared
                .items
                .lock()
                .expect("предметы захвачены другим потоком")
                .step(&solid);

            // Долетевшие до земли снова становятся блоками.
            land_falling(&shared, &mut world);

            if ticks.is_multiple_of(SAVE_EVERY) {
                world.save_if_needed();
            }

            if ticks.is_multiple_of(UNLOAD_EVERY) {
                unload_empty_chunks(&shared, &mut world);
            }

        }

        // Работа такта сделана: подключения рассылают, что случилось.
        shared.ticks.send_replace(ticks);
    }
}

/// Убирает из памяти чанки, рядом с которыми никого нет.
///
/// Без этого память растёт вместе с тем, сколько мира обошли игроки, и
/// обратно уже не возвращается.
fn unload_empty_chunks(shared: &Shared, world: &mut World) {
    let players: Vec<(i32, i32)> = shared
        .players
        .lock()
        .expect("список игроков захвачен другим потоком")
        .members()
        .iter()
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
fn start_falling(shared: &Shared, world: &mut World) {
    for (x, y, z) in world.take_falling() {
        let state = world.get_block(x, y, z);

        // Пока место ждало своей очереди, всё могло измениться.
        if !redstone::falls(state) || world.get_block(x, y - 1, z) != crate::world::AIR {
            continue;
        }

        // Что выпадет, если приземлиться будет некуда.
        let item = blocks::drop_of(state, None).unwrap_or(0);
        let entity_id = shared.next_entity_id();

        world.set_block(x, y, z, crate::world::AIR);

        shared
            .items
            .lock()
            .expect("предметы захвачены другим потоком")
            .drop_block(
                entity_id,
                state,
                Stack::new(item, 1),
                (x as f64 + 0.5, y as f64, z as f64 + 0.5),
            );
    }
}

/// Ставит обратно в мир то, что долетело до земли.
///
/// «If it lands with the bottom center of its hitbox in a replaceable block,
/// and the block below can support it, then the falling block returns to its
/// block state. Otherwise, it breaks and drops as an item.»
fn land_falling(shared: &Shared, world: &mut World) {
    let landed = shared
        .items
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

        // Место свободно — блок встаёт обратно.
        if world.get_block(x, y, z) == crate::world::AIR {
            world.set_block(x, y, z, state);
            redstone::settle(world);
            continue;
        }

        // Занято — падает предметом.
        let entity_id = shared.next_entity_id();

        shared
            .items
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

/// Роняет предметы из блоков, которые сломал не игрок, а сам мир.
fn drop_destroyed(shared: &Shared, world: &mut World) {
    for ((x, y, z), state) in world.take_destroyed() {
        // Инструмент тут ни при чём: блок сломал не игрок.
        let luck = world.random_unit();

        let Some(item) = blocks::drop_by_luck(state, None, luck) else {
            continue;
        };

        let entity_id = shared.next_entity_id();

        shared
            .items
            .lock()
            .expect("предметы захвачены другим потоком")
            .drop_item(
                entity_id,
                Stack::new(item, 1),
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

        fluids::update((x, y, z), &view)
    };

    if let Some(state) = wanted {
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

            if fluids::fluid_at(next).is_none() && fluids::can_flow_into(next) {
                world.schedule(x + dx, y + dy, z + dz, kind.delay());
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
