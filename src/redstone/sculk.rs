// Датчик скалка и калиброванный датчик скалка: слышат вибрации — игровые
// события вокруг — и отвечают сигналом.
//
// По разбору tools/research/components-rest.md, §9 (страницы minecraft.wiki:
// Sculk Sensor, Calibrated Sculk Sensor, Vibration). Кода игры и чужих ядер
// нет. Тайминги сверены чёрным ящиком (сценарии sculk-lever,
// sculk-calibrated).
//
// Как это устроено. Деталь, с которой что-то случилось (рычаг щёлкнул,
// дверь открылась, блок поставили), сообщает о вибрации — месте и частоте
// (`game_event`). Каждый датчик в радиусе, который сейчас свободен, берёт
// себе ближайшую вибрацию такта («If multiple vibrations occur in the same
// tick, the sensor selects the closest one»). В фазе блок-сущностей
// выбранная вибрация отправляется в путь — «a signal travels from the
// vibration source to the sensor at a speed of one block per game tick», —
// а долетевшая включает датчик: `active` на 30 тактов (калиброванный — на
// 10), потом `cooldown` на 10, потом снова `inactive`.

use std::collections::BTreeMap;

use super::{Block, Dir, Kind, MAX_POWER, NEIGHBOUR_ORDER, PRIORITY_NORMAL, Pos, power_from, set, settle, step};
use crate::world::World;

/// Сколько держится `active`: обычный — «activated for 30 game ticks»,
/// калиброванный — 10.
const ACTIVE_TICKS: u64 = 30;
const CALIBRATED_ACTIVE_TICKS: u64 = 10;

/// Сколько держится `cooldown`: «a cooldown period of 10 game ticks».
const COOLDOWN_TICKS: u64 = 10;

/// Радиус слышимости: обычный — 8, калиброванный — 16.
const RANGE: f64 = 8.0;
const CALIBRATED_RANGE: f64 = 16.0;

/// Вибрация, которую датчик выбрал или которая к нему летит.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Vibration {
    frequency: u8,

    /// Расстояние от места события до датчика, в блоках.
    distance: f64,

    /// Где событие случилось.
    from: Pos,
}

/// Что датчик помнит между тактами — у оригинала это его блок-сущность.
#[derive(Clone, Copy, Debug, Default)]
pub struct Listener {
    /// Ближайшая вибрация этого такта: в путь она уйдёт в фазе
    /// блок-сущностей.
    chosen: Option<Vibration>,

    /// Вибрация в пути и сколько тактов ей ещё лететь.
    travelling: Option<(Vibration, u64)>,

    /// Частота последней пойманной вибрации — её читает сравнитель
    /// (`last_vibration_frequency`).
    last_frequency: u8,
}

/// Датчики мира, по координатам.
pub type Sensors = BTreeMap<Pos, Listener>;

/// Датчик ли это (любой из двух).
pub fn is_sensor(name: &str) -> bool {
    matches!(name, "sculk_sensor" | "calibrated_sculk_sensor")
}

/// Датчик ли в этом состоянии — по нему мир ведёт список датчиков.
pub fn is_sensor_state(state: i32) -> bool {
    crate::blocks::block_at_state(state).is_some_and(is_sensor)
}

fn calibrated(block: &Block) -> bool {
    block.name == "calibrated_sculk_sensor"
}

/// Сила выхода: «max(1, 15 - floor(15 / detection_range * distance))».
fn strength(distance: f64, range: f64) -> u8 {
    let lost = (f64::from(MAX_POWER) / range * distance).floor() as i64;

    (i64::from(MAX_POWER) - lost).clamp(1, i64::from(MAX_POWER)) as u8
}

/// Сила датчика по его состоянию.
pub fn power_value(block: &Block) -> u8 {
    block.value("power").and_then(|value| value.parse().ok()).unwrap_or(0)
}

/// Грань ввода калиброванного датчика — та, что с аметистом. Сигнал в неё
/// задаёт частоту, которую датчик слушает; в эту сторону датчик не питает.
/// Это грань напротив `facing`: у `facing=north` не питается провод
/// с юга (сценарий sculk-calibrated).
pub fn input_face(block: &Block) -> Option<Dir> {
    if calibrated(block) { block.facing().map(Dir::opposite) } else { None }
}

/// Сила, которую датчик отдаёт в сторону `toward`: во все стороны, кроме
/// грани ввода калиброванного.
pub fn output_toward(block: &Block, toward: Dir) -> u8 {
    if input_face(block) == Some(toward) {
        return 0;
    }

    power_value(block)
}

/// Что читает сравнитель: частоту последней вибрации, пока датчик
/// `active`; в откате уже 0 (сценарий sculk-lever: сравнитель гаснет через
/// 2 такта после начала отката).
pub fn comparator_reading(world: &World, pos: Pos, block: &Block) -> u8 {
    if !block.is("sculk_sensor_phase", "active") {
        return 0;
    }

    world.redstone.sensors.get(&pos).map_or(0, |listener| listener.last_frequency)
}

/// Частота, на которую настроен калиброванный датчик: сила сигнала в грань
/// ввода; 0 — слушает всё.
fn tuned_to(world: &World, pos: Pos, block: &Block) -> u8 {
    match input_face(block) {
        Some(face) => power_from(world, pos, face, false),
        None => 0,
    }
}

/// Случилось игровое событие частоты `frequency` в месте `from`: каждый
/// свободный датчик в радиусе запоминает его, если оно ближе уже выбранного
/// в этом такте.
pub fn game_event(world: &mut World, from: Pos, frequency: u8) {
    if world.redstone.sensors.is_empty() {
        return;
    }

    let near: Vec<(Pos, f64)> = world
        .redstone
        .sensors
        .keys()
        .filter(|pos| **pos != from)
        .map(|pos| {
            let (dx, dy, dz) = (f64::from(pos.0 - from.0), f64::from(pos.1 - from.1), f64::from(pos.2 - from.2));

            (*pos, (dx * dx + dy * dy + dz * dz).sqrt())
        })
        .filter(|(_, distance)| *distance <= CALIBRATED_RANGE)
        .collect();

    for (pos, distance) in near {
        let block = Block::at(world, pos);

        if block.kind != Kind::Sculk {
            world.redstone.sensors.remove(&pos);
            continue;
        }

        let range = if calibrated(&block) { CALIBRATED_RANGE } else { RANGE };

        // Занятый датчик не слышит: «cannot detect any other vibrations
        // while activated or while a signal is traveling to it».
        if distance > range || !block.is("sculk_sensor_phase", "inactive") {
            continue;
        }

        let tuned = tuned_to(world, pos, &block);

        if tuned != 0 && tuned != frequency {
            continue;
        }

        let Some(listener) = world.redstone.sensors.get_mut(&pos) else {
            continue;
        };

        if listener.travelling.is_some() {
            continue;
        }

        // При равных расстояниях остаётся более раннее.
        if listener.chosen.is_none_or(|chosen| distance < chosen.distance) {
            listener.chosen = Some(Vibration { frequency, distance, from });
        }
    }
}

/// Фаза блок-сущностей: выбранные вибрации уходят в путь, долетевшие
/// включают датчики. Порядок — по координатам.
pub fn sensors_tick(world: &mut World) {
    if world.redstone.sensors.is_empty() {
        return;
    }

    let positions: Vec<Pos> = world.redstone.sensors.keys().copied().collect();

    for pos in positions {
        if !world.has_chunk(pos.0.div_euclid(16), pos.2.div_euclid(16)) {
            world.redstone.sensors.remove(&pos);
            continue;
        }

        let Some(listener) = world.redstone.sensors.get_mut(&pos) else {
            continue;
        };

        if let Some(chosen) = listener.chosen.take() {
            // Лёт — по блоку за такт: «one block per game tick». Датчик
            // в четырёх блоках щёлкает через 3 такта после события, в шести —
            // через 5 (сценарии sculk-lever, sculk-calibrated).
            // [проверить] округление дробного расстояния.
            let ticks = chosen.distance.floor() as u64;

            listener.travelling = Some((chosen, ticks));
            world.note_vibration(chosen.from, pos, ticks as i32);
        }

        let Some(listener) = world.redstone.sensors.get_mut(&pos) else {
            continue;
        };

        let Some((vibration, left)) = listener.travelling else {
            continue;
        };

        // Долетает, когда счёт дошёл до нуля в этом же такте.
        if left > 1 {
            listener.travelling = Some((vibration, left - 1));
            continue;
        }

        listener.travelling = None;
        activate(world, pos, vibration);
        settle(world);
    }
}

/// Долетевшая вибрация включает датчик.
fn activate(world: &mut World, pos: Pos, vibration: Vibration) {
    let block = Block::at(world, pos);

    if block.kind != Kind::Sculk || !block.is("sculk_sensor_phase", "inactive") {
        return;
    }

    let (range, active) = if calibrated(&block) {
        (CALIBRATED_RANGE, CALIBRATED_ACTIVE_TICKS)
    } else {
        (RANGE, ACTIVE_TICKS)
    };

    if let Some(listener) = world.redstone.sensors.get_mut(&pos) {
        listener.last_frequency = vibration.frequency;
    }

    let power = strength(vibration.distance, range).to_string();
    let state = block
        .with("sculk_sensor_phase", "active")
        .and_then(|state| block.orientation?.with(state, "power", &power));

    if let Some(state) = state {
        set(world, pos, state);
    }

    world.schedule_block(pos.0, pos.1, pos.2, active, PRIORITY_NORMAL);

    // Громкость 1 — снято с оригинала (вики пишет 0,73).
    let pitch = 0.8 + world.random_unit() * 0.2;
    world.note_sound(pos.0, pos.1, pos.2, "minecraft:block.sculk_sensor.clicking", 1.0, pitch);
}

/// Запланированный такт датчика: `active` → `cooldown` (сигнал снят),
/// `cooldown` → `inactive`.
pub fn ticked(world: &mut World, pos: Pos, block: &Block) {
    match block.value("sculk_sensor_phase") {
        Some("active") => {
            let state = block
                .with("sculk_sensor_phase", "cooldown")
                .and_then(|state| block.orientation?.with(state, "power", "0"));

            if let Some(state) = state {
                set(world, pos, state);
            }

            world.schedule_block(pos.0, pos.1, pos.2, COOLDOWN_TICKS, PRIORITY_NORMAL);
        }
        // Щелчок выключения — в конце отката, не в его начале, и громкостью
        // 1 (снято с оригинала: сценарий sculk-lever).
        Some("cooldown") => {
            let pitch = 0.8 + world.random_unit() * 0.2;
            world.note_sound(pos.0, pos.1, pos.2, "minecraft:block.sculk_sensor.clicking_stop", 1.0, pitch);

            if let Some(state) = block.with("sculk_sensor_phase", "inactive") {
                set(world, pos, state);
            }
        }
        _ => {}
    }
}

/// Кого ещё будит смена силы датчика: соседей блока под ним — его датчик
/// питает сильно («powers the block below the sensor strongly»).
pub fn reach(pos: Pos) -> Vec<Pos> {
    let below = step(pos, Dir::Down);

    NEIGHBOUR_ORDER.iter().map(|dir| step(below, *dir)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strength_falls_with_distance() {
        // Таблица вики: у обычного датчика 1 блок — 14, 4 — 8, 8 — 1.
        assert_eq!(strength(1.0, RANGE), 14);
        assert_eq!(strength(4.0, RANGE), 8);
        assert_eq!(strength(8.0, RANGE), 1);
        assert_eq!(strength(0.5, RANGE), 15);
        // Калиброванный: вдвое дальше — та же сила.
        assert_eq!(strength(8.0, CALIBRATED_RANGE), 8);
        assert_eq!(strength(6.0, CALIBRATED_RANGE), 10);
    }
}
