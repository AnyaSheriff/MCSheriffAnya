// Кровати и якорь возрождения: сон, точка возрождения, взрывы.
//
// Всё, что можно посчитать по одному миру, — здесь: где изголовье, можно ли
// сейчас спать, куда встать после сна или возрождения, заряды якоря. Сеть
// (сообщения, поза, пакеты) — в network/play.rs, пропуск ночи — в tick.rs.
//
// Источники: вики (Bed, Respawn Anchor, Explosion) и чёрный ящик (сценарии
// bed-*, anchor-*).

use crate::blocks;
use crate::dimension::Dimension;
use crate::world::World;

/// Место блока.
pub type Pos = (i32, i32, i32);

/// Сила взрыва кровати вне обычного мира и якоря вне Незера (вики, Bed:
/// «The explosion has power 5»; Respawn Anchor: «a power of 5 and sets fire»).
pub const EXPLOSION_POWER: f32 = 5.0;

/// Сколько тактов игрок спит, прежде чем ночь можно пропустить (вики, Bed:
/// «after 5 seconds (100 ticks) the time of day changes to sunrise»).
pub const DEEP_SLEEP_TICKS: u32 = 100;

/// Насколько далеко от кровати можно лечь (вики, Bed: «within a distance of 3
/// blocks {{in|Java}}»): по горизонтали до середины любой половины — 3,
/// по высоте — 2. [проверить] точную мерку.
const BED_REACH: f64 = 3.0;
const BED_REACH_UP: f64 = 2.0;

/// Чудища мешают спать в этой коробке вокруг изголовья (вики, Bed: «within
/// 8 blocks of the bed head horizontally … and 5 blocks vertically»).
pub const MONSTER_REACH: (f64, f64) = (8.0, 5.0);

/// Вид блок-сущности кровати: оригинал шлёт её данные (пустые) после
/// постановки и при каждой смене «занята» — изголовье, потом изножье
/// (чёрный ящик, bed-place-break, bed-sleep-night: `block_entity type=25 {}`).
pub const BLOCK_ENTITY: i32 = 25;

/// Пустой составной тег NBT: данные блок-сущности кровати.
const EMPTY_COMPOUND: [u8; 2] = [0x0A, 0x00];

/// Данные блок-сущности обеих половин — уходят после пачки.
pub fn note_entities(world: &mut World, head: Pos) {
    let mut places = vec![head];

    places.extend(foot_of(world, head));

    for (x, y, z) in places {
        if is_bed(world.get_block(x, y, z)) {
            world.note_block_entity(x, y, z, BLOCK_ENTITY, EMPTY_COMPOUND.to_vec());
        }
    }
}

/// Высота кровати: встающий на неё — на этой высоте над её блоком.
const BED_HEIGHT: f64 = 0.5625;

/// Личная точка возрождения игрока: кровать, якорь или место по команде
/// `spawnpoint` (`forced`).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct RespawnPoint {
    pub dimension: Dimension,
    pub pos: Pos,
    /// Куда игрок смотрел, ставя точку: по нему выбирается сторона кровати.
    pub yaw: f32,
    /// Поставлена командой: годится и без кровати или якоря.
    pub forced: bool,
}

impl RespawnPoint {
    /// Длина записи в файле игрока: есть ли точка, измерение, место,
    /// поворот, признак команды.
    pub const SIZE: usize = 1 + 1 + 12 + 4 + 1;

    /// Запись постоянной длины (пустая — нулями).
    pub fn write(point: Option<&RespawnPoint>, out: &mut Vec<u8>) {
        let Some(point) = point else {
            out.extend_from_slice(&[0; Self::SIZE]);
            return;
        };

        out.push(1);
        out.push(point.dimension.index() as u8);
        out.extend_from_slice(&point.pos.0.to_be_bytes());
        out.extend_from_slice(&point.pos.1.to_be_bytes());
        out.extend_from_slice(&point.pos.2.to_be_bytes());
        out.extend_from_slice(&point.yaw.to_be_bytes());
        out.push(u8::from(point.forced));
    }

    /// None — запись битая; Some(None) — точки нет.
    pub fn read(bytes: &[u8]) -> Option<Option<RespawnPoint>> {
        if bytes.len() != Self::SIZE {
            return None;
        }

        if bytes[0] == 0 {
            return Some(None);
        }

        let int = |at: usize| bytes.get(at..at + 4).and_then(|part| part.try_into().ok()).map(i32::from_be_bytes);
        let yaw = f32::from_be_bytes(bytes.get(14..18)?.try_into().ok()?);

        Some(Some(RespawnPoint {
            dimension: *Dimension::ALL.get(bytes[1] as usize)?,
            pos: (int(2)?, int(6)?, int(10)?),
            yaw: if yaw.is_finite() { yaw } else { 0.0 },
            forced: bytes[18] != 0,
        }))
    }
}

/// Кровать ли это (любого из 16 цветов).
pub fn is_bed(state: i32) -> bool {
    blocks::block_at_state(state).is_some_and(|name| name.ends_with("_bed"))
}

/// Якорь ли это.
pub fn is_anchor(state: i32) -> bool {
    blocks::block_at_state(state) == Some("respawn_anchor")
}

fn value(state: i32, property: &str) -> Option<&'static str> {
    blocks::orientation_of(state)?.value(state, property)
}

/// Шаг по стороне света (север — −z, восток — +x).
pub fn facing_step(facing: &str) -> Option<(i32, i32)> {
    match facing {
        "north" => Some((0, -1)),
        "south" => Some((0, 1)),
        "west" => Some((-1, 0)),
        "east" => Some((1, 0)),
        _ => None,
    }
}

/// Куда смотрит кровать (от изножья к изголовью).
pub fn bed_facing(state: i32) -> Option<(i32, i32)> {
    facing_step(value(state, "facing")?)
}

/// Изголовье кровати, по любой половине которой щёлкнули. None — это не
/// кровать или от неё осталась одна половина (щелчок по такому изножью
/// у оригинала ничего не делает).
pub fn head_of(world: &World, pos: Pos) -> Option<Pos> {
    let state = world.get_block(pos.0, pos.1, pos.2);

    if !is_bed(state) {
        return None;
    }

    if value(state, "part") == Some("head") {
        return Some(pos);
    }

    let (dx, dz) = bed_facing(state)?;
    let head = (pos.0 + dx, pos.1, pos.2 + dz);

    is_bed(world.get_block(head.0, head.1, head.2)).then_some(head)
}

/// Изножье при изголовье.
pub fn foot_of(world: &World, head: Pos) -> Option<Pos> {
    let (dx, dz) = bed_facing(world.get_block(head.0, head.1, head.2))?;

    Some((head.0 - dx, head.1, head.2 - dz))
}

/// Кровать в `pos` смотрит своей парой в сторону `toward` (шаг по x, y, z),
/// а там больше нет её второй половины.
pub fn lost_partner(world: &World, pos: Pos, toward: (i32, i32, i32)) -> bool {
    let state = world.get_block(pos.0, pos.1, pos.2);

    if !is_bed(state) {
        return false;
    }

    let Some((fx, fz)) = bed_facing(state) else {
        return false;
    };
    let head = value(state, "part") == Some("head");
    let partner_step = if head { (-fx, 0, -fz) } else { (fx, 0, fz) };

    if toward != partner_step {
        return false;
    }

    let other = world.get_block(pos.0 + toward.0, pos.1, pos.2 + toward.2);

    !(is_bed(other) && value(other, "facing") == value(state, "facing") && value(other, "part") != value(state, "part"))
}

/// Обе половины кровати получают признак «занята».
pub fn set_occupied(world: &mut World, head: Pos, occupied: bool) {
    let text = if occupied { "true" } else { "false" };
    let mut places = vec![head];

    places.extend(foot_of(world, head));

    for (x, y, z) in places {
        let state = world.get_block(x, y, z);

        if is_bed(state)
            && let Some(changed) = blocks::orientation_of(state).and_then(|block| block.with(state, "occupied", text))
            && changed != state
        {
            world.set_block(x, y, z, changed);
        }
    }
}

/// Занята ли кровать.
pub fn occupied(world: &World, head: Pos) -> bool {
    value(world.get_block(head.0, head.1, head.2), "occupied") == Some("true")
}

/// Работают ли здесь кровати (спать и ставить точку): только в обычном
/// мире; в Незере и Энде кровать взрывается (вики, Bed).
pub fn beds_work(dimension: Dimension) -> bool {
    dimension == Dimension::Overworld
}

/// Работает ли здесь якорь: только в Незере, в прочих мирах заряженный
/// якорь взрывается (вики, Respawn Anchor).
pub fn anchors_work(dimension: Dimension) -> bool {
    dimension == Dimension::Nether
}

/// Можно ли сейчас спать (вики, Bed: «when the internal sky light is 11 or
/// below. This occurs between 12523 and 23477 ticks in clear weather …
/// between 12002 and 23998 ticks in rainy weather, and at any time of day
/// during thunderstorms»).
pub fn dark_enough(time_of_day: i64, raining: bool, thundering: bool) -> bool {
    let time = time_of_day.rem_euclid(24_000);

    thundering || (raining && (12_002..=23_998).contains(&time)) || (12_523..=23_477).contains(&time)
}

/// Утро после пропущенной ночи: начало следующих суток (вики, Bed: «the time
/// of day changes to sunrise (time 0)»). Возвращает прибавку к часам.
pub fn until_morning(time_of_day: i64) -> i64 {
    24_000 - time_of_day.rem_euclid(24_000)
}

/// Сколько спящих нужно, чтобы пропустить ночь: доля из правила
/// `players_sleeping_percentage` от игроков мира, но не меньше одного
/// (вики, Game rule: «What percentage of players must sleep to skip the
/// night»). Больше 100 — ночь не пропустить вовсе (None).
pub fn sleepers_needed(players: usize, percentage: i32) -> Option<usize> {
    if percentage > 100 {
        return None;
    }

    let share = (players as i64 * i64::from(percentage.max(0)) + 99) / 100;

    Some((share as usize).max(1))
}

/// Достаёт ли игрок до кровати, чтобы лечь (рядом с любой половиной).
pub fn bed_in_reach(world: &World, head: Pos, (x, y, z): (f64, f64, f64)) -> bool {
    let near = |(bx, by, bz): Pos| {
        (x - f64::from(bx) - 0.5).abs() <= BED_REACH
            && (y - f64::from(by)).abs() <= BED_REACH_UP
            && (z - f64::from(bz) - 0.5).abs() <= BED_REACH
    };

    near(head) || foot_of(world, head).is_some_and(near)
}

/// Над изголовьем блок, в котором не лечь (вики, Bed: «This bed is
/// obstructed» — когда встать после сна некуда). [проверить] у оригинала
/// мерка — твёрдый блок прямо над изголовьем.
pub fn bed_obstructed(world: &World, head: Pos) -> bool {
    let above = world.get_block(head.0, head.1 + 1, head.2);

    blocks::full_cube(above)
}

/// Свободно ли место для стоящего игрока: в ногах и в голове нечем
/// столкнуться.
fn free(world: &World, (x, y, z): Pos) -> bool {
    !blocks::has_box(world.get_block(x, y, z)) && !blocks::has_box(world.get_block(x, y + 1, z))
}

/// Можно ли встать в этот столбец ногами в `pos`: снизу твёрдый верх,
/// в ногах и в голове пусто.
fn can_stand(world: &World, pos: Pos) -> bool {
    blocks::solid_top(world.get_block(pos.0, pos.1 - 1, pos.2)) && free(world, pos)
}

/// Место, куда встать у столбца рядом с кроватью: на уровне кровати или
/// ниже — вики, Bed: «the block at the level of the bed must be air or
/// non-solid … a solid block below it and two non-colliding blocks for the
/// player to stand in 0-2 blocks below the bed». [проверить] глубину.
fn stand_near_bed(world: &World, (x, y, z): Pos) -> Option<(f64, f64, f64)> {
    if blocks::has_box(world.get_block(x, y, z)) {
        return None;
    }

    (0..2)
        .map(|down| (x, y - down, z))
        .find(|pos| can_stand(world, *pos))
        .map(|(x, y, z)| (f64::from(x) + 0.5, f64::from(y), f64::from(z) + 0.5))
}

/// Куда встать после сна или возрождения у кровати (вики, Bed): сперва по
/// бокам изголовья — со стороны, с какой игрок смотрел на кровать, ставя
/// точку (ровно вдоль — правая); дальше вокруг кровати по часовой стрелке
/// (правая) или против (левая), все 10 мест; потом над изголовьем и над
/// изножьем. [проверить] порядок мест после первого.
pub fn bed_standup(world: &World, bed: Pos, yaw: f32) -> Option<(f64, f64, f64)> {
    let head = head_of(world, bed).unwrap_or(bed);
    let (fx, fz) = bed_facing(world.get_block(head.0, head.1, head.2))?;
    // Правая сторона кровати, если смотреть от изножья к изголовью.
    let (rx, rz) = (-fz, fx);
    let foot = (head.0 - fx, head.1, head.2 - fz);

    // Куда смотрел игрок: yaw 0 — на юг (+z), 90 — на запад (−x).
    let radians = f64::from(yaw).to_radians();
    let look = (-radians.sin(), radians.cos());
    let toward_right = look.0 * f64::from(rx) + look.1 * f64::from(rz);
    // Смотрел в сторону правой — значит стоял слева.
    let left = toward_right > 1e-6;

    let at = |base: Pos, (ax, az): (i32, i32), (bx, bz): (i32, i32)| (base.0 + ax + bx, base.1, base.2 + az + bz);
    let (r, l, f, b, o) = ((rx, rz), (-rx, -rz), (fx, fz), (-fx, -fz), (0, 0));
    let right_round = [at(head, r, o), at(foot, r, o), at(foot, r, b), at(foot, b, o), at(foot, l, b), at(foot, l, o), at(head, l, o), at(head, l, f), at(head, f, o), at(head, r, f)];
    let left_round = [at(head, l, o), at(foot, l, o), at(foot, l, b), at(foot, b, o), at(foot, r, b), at(foot, r, o), at(head, r, o), at(head, r, f), at(head, f, o), at(head, l, f)];
    let round = if left { left_round } else { right_round };

    if let Some(spot) = round.iter().find_map(|pos| stand_near_bed(world, *pos)) {
        return Some(spot);
    }

    // Над кроватью: ногами на её верх.
    [head, foot]
        .into_iter()
        .map(|(x, y, z)| (x, y + 1, z))
        .find(|pos| free(world, *pos))
        .map(|(x, y, z)| (f64::from(x) + 0.5, f64::from(y - 1) + BED_HEIGHT, f64::from(z) + 0.5))
}

/// Столбцы вокруг якоря по порядку (вики, Respawn Anchor, рисунок «Tile
/// priority»: восемь столбцов кольцом). Первым у оригинала оказался север
/// (−z): чёрный ящик anchor-nether — возрождение в (3,5; 0; 2,5) у якоря
/// в (3; 0; 3), хотя юг тоже свободен; значит, оси на рисунке смотрят от
/// нас, и порядок — −z, −x, +x, +z, потом углы −x−z, +x−z, −x+z, +x+z.
/// [проверить] порядок после первого.
const ANCHOR_COLUMNS: [(i32, i32); 8] = [(0, -1), (-1, 0), (1, 0), (0, 1), (-1, -1), (1, -1), (-1, 1), (1, 1)];

/// Куда встать у якоря (вики, Respawn Anchor: «Respawn attempts are made per
/// column for layers 1 and 2, and if all of those attempts fail, then per
/// column for layer 3»): слой 1 — ниже якоря, 2 — вровень, 3 — выше, и
/// в третьем слое ещё место над самим якорем. Место — 1×2×1 с опорой снизу.
pub fn anchor_standup(world: &World, anchor: Pos) -> Option<(f64, f64, f64)> {
    let (x, y, z) = anchor;
    let low = ANCHOR_COLUMNS.iter().flat_map(|(dx, dz)| [(x + dx, y - 1, z + dz), (x + dx, y, z + dz)]);
    let high = ANCHOR_COLUMNS.iter().chain(std::iter::once(&(0, 0))).map(|(dx, dz)| (x + dx, y + 1, z + dz));

    low.chain(high)
        .find(|pos| can_stand(world, *pos))
        .map(|(x, y, z)| (f64::from(x) + 0.5, f64::from(y), f64::from(z) + 0.5))
}

/// Место по команде `spawnpoint`: годится, если игроку там не тесно.
pub fn forced_standup(world: &World, pos: Pos) -> Option<(f64, f64, f64)> {
    free(world, pos).then(|| (f64::from(pos.0) + 0.5, f64::from(pos.1), f64::from(pos.2) + 0.5))
}

/// Поворот лицом к середине блока: возрождённый у кровати и вставший
/// смотрит на изголовье (вики, Bed: «The player is always respawned facing
/// the head of the bed»), у якоря — на якорь.
pub fn yaw_toward((x, _, z): (f64, f64, f64), (bx, _, bz): Pos) -> f32 {
    let dx = f64::from(bx) + 0.5 - x;
    let dz = f64::from(bz) + 0.5 - z;

    if dx == 0.0 && dz == 0.0 {
        return 0.0;
    }

    (-dx).atan2(dz).to_degrees() as f32
}

/// Заряды якоря.
pub fn anchor_charges(state: i32) -> Option<u8> {
    value(state, "charges")?.parse().ok()
}

/// Якорь с другим числом зарядов.
pub fn with_charges(state: i32, charges: u8) -> Option<i32> {
    blocks::orientation_of(state)?.with(state, "charges", &charges.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::AIR;

    fn state(text: &str) -> i32 {
        blocks::state_from_text(text).expect("состояние есть в таблице")
    }

    /// Каменный пол на высоте 0, кровать изножьем в (0,1,0) головой на юг.
    fn world_with_bed() -> World {
        let mut world = World::in_memory();

        for x in -4..=4 {
            for z in -4..=4 {
                world.set_block_silently(x, 0, z, state("stone"));
            }
        }

        world.set_block_silently(0, 1, 0, state("minecraft:red_bed[facing=south,occupied=false,part=foot]"));
        world.set_block_silently(0, 1, 1, state("minecraft:red_bed[facing=south,occupied=false,part=head]"));
        world
    }

    #[test]
    fn every_colour_is_a_bed() {
        let colours = [
            "white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray", "light_gray", "cyan", "purple", "blue", "brown",
            "green", "red", "black",
        ];

        for colour in colours {
            let bed = blocks::state_by_name(&format!("{}_bed", colour)).expect("кровать есть");

            assert!(is_bed(bed), "{}", colour);
        }
    }

    #[test]
    fn a_click_on_either_half_finds_the_head() {
        let world = world_with_bed();

        assert_eq!(head_of(&world, (0, 1, 0)), Some((0, 1, 1)));
        assert_eq!(head_of(&world, (0, 1, 1)), Some((0, 1, 1)));
        assert_eq!(foot_of(&world, (0, 1, 1)), Some((0, 1, 0)));
    }

    #[test]
    fn a_lonely_foot_has_no_head() {
        let mut world = world_with_bed();

        world.set_block_silently(0, 1, 1, AIR);
        assert_eq!(head_of(&world, (0, 1, 0)), None);
    }

    #[test]
    fn a_half_without_its_partner_is_noticed() {
        let mut world = world_with_bed();

        assert!(!lost_partner(&world, (0, 1, 0), (0, 0, 1)));
        world.set_block_silently(0, 1, 1, AIR);
        assert!(lost_partner(&world, (0, 1, 0), (0, 0, 1)));
        // Сосед с другой стороны паре не помеха.
        assert!(!lost_partner(&world, (0, 1, 0), (1, 0, 0)));
    }

    #[test]
    fn occupied_marks_both_halves() {
        let mut world = world_with_bed();

        set_occupied(&mut world, (0, 1, 1), true);
        assert!(occupied(&world, (0, 1, 1)));
        assert_eq!(value(world.get_block(0, 1, 0), "occupied"), Some("true"));
        set_occupied(&mut world, (0, 1, 1), false);
        assert!(!occupied(&world, (0, 1, 1)));
    }

    #[test]
    fn night_and_thunder_allow_sleep() {
        assert!(!dark_enough(6_000, false, false));
        assert!(!dark_enough(12_522, false, false));
        assert!(dark_enough(12_523, false, false));
        assert!(dark_enough(23_477, false, false));
        assert!(!dark_enough(23_478, false, false));
        assert!(dark_enough(12_002, true, false));
        assert!(dark_enough(6_000, true, true));
    }

    #[test]
    fn morning_is_the_start_of_the_next_day() {
        assert_eq!(until_morning(14_000), 10_000);
        assert_eq!(until_morning(0), 24_000);
    }

    #[test]
    fn sleepers_needed_follow_the_percentage() {
        assert_eq!(sleepers_needed(1, 100), Some(1));
        assert_eq!(sleepers_needed(4, 50), Some(2));
        assert_eq!(sleepers_needed(3, 50), Some(2));
        assert_eq!(sleepers_needed(5, 0), Some(1));
        assert_eq!(sleepers_needed(2, 101), None);
    }

    #[test]
    fn beds_work_only_in_the_overworld_and_anchors_only_in_the_nether() {
        assert!(beds_work(Dimension::Overworld));
        assert!(!beds_work(Dimension::Nether));
        assert!(!beds_work(Dimension::End));
        assert!(anchors_work(Dimension::Nether));
        assert!(!anchors_work(Dimension::Overworld));
    }

    #[test]
    fn the_bed_is_reached_from_three_blocks() {
        let world = world_with_bed();

        assert!(bed_in_reach(&world, (0, 1, 1), (0.5, 1.0, -2.5)));
        assert!(!bed_in_reach(&world, (0, 1, 1), (0.5, 1.0, -3.6)));
    }

    #[test]
    fn standing_up_goes_to_the_side_the_player_came_from() {
        let world = world_with_bed();

        // Кровать на юг: правая сторона — запад (−x). Смотрел ровно вдоль —
        // правая; смотрел на запад (стоял на востоке) — левая, восток.
        assert_eq!(bed_standup(&world, (0, 1, 1), 0.0), Some((-0.5, 1.0, 1.5)));
        assert_eq!(bed_standup(&world, (0, 1, 1), 90.0), Some((1.5, 1.0, 1.5)));
        assert_eq!(bed_standup(&world, (0, 1, 1), -90.0), Some((-0.5, 1.0, 1.5)));
    }

    #[test]
    fn a_blocked_side_moves_round_the_bed() {
        let mut world = world_with_bed();

        world.set_block_silently(-1, 1, 1, state("stone"));
        assert_eq!(bed_standup(&world, (0, 1, 1), 0.0), Some((-0.5, 1.0, 0.5)));
    }

    #[test]
    fn a_walled_bed_is_left_from_above() {
        let mut world = world_with_bed();

        for x in -1..=1 {
            for z in -1..=2 {
                if (x, z) != (0, 0) && (x, z) != (0, 1) {
                    world.set_block_silently(x, 1, z, state("stone"));
                    world.set_block_silently(x, 2, z, state("stone"));
                }
            }
        }

        assert_eq!(bed_standup(&world, (0, 1, 1), 0.0), Some((0.5, 1.5625, 1.5)));
    }

    #[test]
    fn the_anchor_puts_the_player_north_first() {
        let mut world = World::in_memory();

        for x in -3..=3 {
            for z in -3..=3 {
                world.set_block_silently(x, 0, z, state("stone"));
            }
        }

        world.set_block_silently(0, 1, 0, state("minecraft:respawn_anchor[charges=2]"));
        // Слой 1 (ниже якоря) занят полом — первым годится север вровень.
        assert_eq!(anchor_standup(&world, (0, 1, 0)), Some((0.5, 1.0, -0.5)));

        world.set_block_silently(0, 1, -1, state("stone"));
        assert_eq!(anchor_standup(&world, (0, 1, 0)), Some((-0.5, 1.0, 0.5)));
    }

    #[test]
    fn respawn_points_survive_the_file() {
        let point = RespawnPoint { dimension: Dimension::Nether, pos: (-5, 70, 123), yaw: 45.5, forced: false };
        let mut bytes = Vec::new();

        RespawnPoint::write(Some(&point), &mut bytes);
        assert_eq!(bytes.len(), RespawnPoint::SIZE);
        assert_eq!(RespawnPoint::read(&bytes), Some(Some(point)));

        bytes.clear();
        RespawnPoint::write(None, &mut bytes);
        assert_eq!(RespawnPoint::read(&bytes), Some(None));
    }

    #[test]
    fn the_respawned_player_faces_the_bed() {
        // С запада на изголовье — лицом на восток (yaw −90).
        assert!((yaw_toward((-0.5, 1.0, 1.5), (0, 1, 1)) + 90.0).abs() < 1e-4);
        assert!(yaw_toward((0.5, 1.0, -0.5), (0, 1, 1)).abs() < 1e-4);
    }

    #[test]
    fn anchor_charges_are_read_and_changed() {
        let anchor = state("minecraft:respawn_anchor[charges=3]");

        assert_eq!(anchor_charges(anchor), Some(3));
        assert_eq!(with_charges(anchor, 2).and_then(anchor_charges), Some(2));
    }
}
