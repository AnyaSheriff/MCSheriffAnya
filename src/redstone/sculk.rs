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
    source: Option<i32>,
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
    game_event_from(world, super::sculk_family::center(from), frequency, world.actor, false);
}

/// Тот же канал с виновником: крикуны слышат лишь события игрока.
pub fn game_event_from(world: &mut World, at: (f64, f64, f64), frequency: u8, source: Option<i32>, projectile: bool) {
    if world.plugin_events_enabled&&crate::plugins::active() {if (1..=15).contains(&frequency) {world.plugin_game_events.push(super::sculk_family::GameEvent {at,frequency,source,projectile});}return;}
    deliver_game_event(world,at,frequency,source,projectile);
}
pub fn deliver_game_event(world: &mut World, at: (f64, f64, f64), frequency: u8, source: Option<i32>, projectile: bool) {
    if !(1..=15).contains(&frequency) { return; }
    let from = (at.0.floor() as i32, at.1.floor() as i32, at.2.floor() as i32);
    let sneaking = source.is_some_and(|id| world.redstone.sculk.players.iter().any(|p| p.id == id && p.sneaking));
    if sneaking && frequency <= 3 && !projectile { return; }
    world.redstone.sculk.events.push(super::sculk_family::GameEvent { at, source, frequency, projectile });
    let near: Vec<(Pos, f64)> = world
        .redstone
        .sensors
        .keys()
        .filter(|pos| **pos != from || !matches!(frequency, 12 | 13))
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
        if distance > range || !block.is("sculk_sensor_phase", "inactive") || super::sculk_family::occluded(world, at, super::sculk_family::center(pos)) {
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

        // При равных расстояниях выбирается большая частота.
        if listener.chosen.is_none_or(|chosen| distance < chosen.distance || (distance == chosen.distance && frequency > chosen.frequency)) {
            listener.chosen = Some(Vibration { frequency, distance, from, source });
        }
    }
}

/// Фаза блок-сущностей: выбранные вибрации уходят в путь, долетевшие
/// включают датчики. Порядок — по координатам.
pub fn sensors_tick(world: &mut World) {
    super::sculk_family::tick(world);
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
    if !block.is("waterlogged", "true") {
        let pitch = 0.8 + world.random_unit() * 0.2;
        world.note_sound(pos.0, pos.1, pos.2, "minecraft:block.sculk_sensor.clicking", 1.0, pitch);
    }
    // Датчик крикуну вибрацию не передаёт: у оригинала крикун молчит (сценарий sculk-shrieker).
    // Вики, Sculk Sensor: «vibration resonance» от соседнего аметиста.
    // [проверить] громкость и высота звука каждой частоты.
    for dir in NEIGHBOUR_ORDER {
        let at = step(pos, dir);
        if Block::at(world, at).name == "amethyst_block" {
            world.note_sound(at.0, at.1, at.2, "minecraft:block.amethyst_block.resonate", 1.0, 1.0);
            game_event_from(world, super::sculk_family::center(at), vibration.frequency, vibration.source, false);
        }
    }
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
            if !block.is("waterlogged", "true") {
                let pitch = 0.8 + world.random_unit() * 0.2;
                world.note_sound(pos.0, pos.1, pos.2, "minecraft:block.sculk_sensor.clicking_stop", 1.0, pitch);
            }

            if let Some(state) = block.with("sculk_sensor_phase", "inactive") {
                set(world, pos, state);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Крикун скалка
// ---------------------------------------------------------------------------

/// Сколько кричит крикун: «The shrieker shrieks for 90 game ticks».
pub const SHRIEK_TICKS: u64 = 90;

/// Событие мира «крикун кричит» — снято с оригинала (sculk-shrieker).
const SHRIEK_EVENT: i32 = 3007;

/// Крикун начинает кричать, если ещё не кричит: `shrieking=true`, звук
/// и через 90 тактов — тишина. Предупреждения даёт только can_summon=true.
pub fn shriek_from(world: &mut World, pos: Pos, source: i32) -> bool {
    let block = Block::at(world, pos);

    if block.name != "sculk_shrieker" || block.is("shrieking", "true") {
        return false;
    }

    if block.is("can_summon", "true") && !super::sculk_family::warn(world, pos, source) { return false; }
    let Some(state) = block.with("shrieking", "true") else {
        return false;
    };

    set(world, pos, state);
    world.schedule_block(pos.0, pos.1, pos.2, SHRIEK_TICKS, PRIORITY_NORMAL);

    // Звук и частицы клиент играет сам по событию мира 3007 (сценарий
    // sculk-shrieker).
    world.note_level_event(pos.0, pos.1, pos.2, SHRIEK_EVENT, 0);

    true
}

/// Запланированный такт крикуна: крик кончился. Кто всё ещё стоит на нём,
/// тут же будит его снова — у оригинала новый крик начинается в том же
/// такте (сценарий sculk-shrieker: `shrieking=false` и событие 3007 разом).
pub fn shrieker_ticked(world: &mut World, pos: Pos, block: &Block) {
    if block.is("shrieking", "true")
        && let Some(state) = block.with("shrieking", "false")
    {
        set(world, pos, state);
        super::sculk_family::finish_shriek(world, pos);
    }
}

/// Игрок, вставший на крикуна, будит его: «activates when any player stands
/// on the black part in the center of the block, even when sneaking». Верх
/// крикуна — на половине блока.
pub fn step_on_shriekers(world: &mut World, occupants: &[super::components::Occupant]) {
    for occupant in occupants {
        if occupant.player.is_none() || !occupant.on_ground {
            continue;
        }

        // Блок «под ногами» — на 0,2 ниже ног, как у стоящего на земле.
        // [проверить] точную границу.
        let below = (occupant.x.floor() as i32, (occupant.y - 0.2).floor() as i32, occupant.z.floor() as i32);
        let above_top = occupant.y - (f64::from(below.1) + 0.5);

        if !(-0.001..=0.2).contains(&above_top) {
            continue;
        }

        if shriek_from(world, below, occupant.player.unwrap_or(0)) {
            settle(world);
        }
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
