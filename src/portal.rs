// Порталы в Незер: зажечь рамку, разрушиться без рамки, перенести игрока
// и найти или построить парный портал.
//
// Правила — по вики (Nether portal): рамка из обсидиана, внутри от 2×3 до
// 21×21; координаты в Незере в восемь раз короче; парный портал ищется
// в Незере в 16 блоках, в обычном мире — в 128; если его нет, строится
// новый 2×3 внутри с площадкой. Уголки рамки не обязательны.

use crate::blocks;
use crate::dimension::Dimension;
use crate::world::{World, AIR};

/// Какие проёмы зажигаются: ширина и высота внутри рамки, от и до. У
/// оригинала от 2×3 до 21×21; сервер может задать свои
/// (`portal-min-width`, `portal-min-height`, `portal-max-width`,
/// `portal-max-height` в config/mcsa.properties).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sizes {
    pub min_width: i32,
    pub min_height: i32,
    pub max_width: i32,
    pub max_height: i32,
}

/// Размеры у оригинала.
pub const VANILLA: Sizes = Sizes { min_width: 2, min_height: 3, max_width: 21, max_height: 21 };

/// Самая большая сторона, какую можно задать: огромный проём заполняется
/// порталом за один такт.
pub const LARGEST: i32 = 256;

impl Sizes {
    /// Размеры, приведённые в допустимые рамки: сторона от 1 до `LARGEST`,
    /// наименьшее не больше наибольшего.
    pub fn sane(self) -> Sizes {
        let max_width = self.max_width.clamp(1, LARGEST);
        let max_height = self.max_height.clamp(1, LARGEST);

        Sizes {
            min_width: self.min_width.clamp(1, max_width),
            min_height: self.min_height.clamp(1, max_height),
            max_width,
            max_height,
        }
    }
}

/// Ось портала: вдоль x или вдоль z.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Axis {
    X,
    Z,
}

impl Axis {
    fn step(self) -> (i32, i32) {
        match self {
            Axis::X => (1, 0),
            Axis::Z => (0, 1),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Axis::X => "x",
            Axis::Z => "z",
        }
    }
}

fn state(name: &str) -> i32 {
    blocks::state_from_text(name).unwrap_or_else(|| panic!("блок {} есть в таблице", name))
}

fn obsidian() -> i32 {
    state("obsidian")
}

/// Состояние портала вдоль этой оси.
fn portal(axis: Axis) -> i32 {
    state(&format!("nether_portal[axis={}]", axis.name()))
}

/// Портал ли это (любой оси).
pub fn is_portal(block: i32) -> bool {
    blocks::block_at_state(block) == Some("nether_portal")
}

/// Можно ли на этом месте зажечь портал: пусто или огонь.
fn fillable(block: i32) -> bool {
    block == AIR || matches!(blocks::block_at_state(block), Some("fire" | "soul_fire"))
}

/// Проём портала: нижний левый угол, ось, ширина и высота.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Opening {
    pub corner: (i32, i32, i32),
    pub axis: Axis,
    pub width: i32,
    pub height: i32,
}

/// Ищет рамку вокруг этого места по данной оси: вниз до пола из обсидиана,
/// влево до стенки, потом ширина, высота и проверка всей рамки.
fn opening_at(world: &World, (x, y, z): (i32, i32, i32), axis: Axis, sizes: Sizes) -> Option<Opening> {
    let block = |(x, y, z): (i32, i32, i32)| world.get_block(x, y, z);
    let obsidian = obsidian();
    let (dx, dz) = axis.step();

    if !fillable(block((x, y, z))) {
        return None;
    }

    // Вниз до пола.
    let mut bottom = y;
    while fillable(block((x, bottom - 1, z))) {
        bottom -= 1;
        if y - bottom > sizes.max_height {
            return None;
        }
    }
    if block((x, bottom - 1, z)) != obsidian {
        return None;
    }

    // Влево до стенки.
    let (mut left_x, mut left_z) = (x, z);
    while fillable(block((left_x - dx, bottom, left_z - dz))) {
        left_x -= dx;
        left_z -= dz;
        if (x - left_x).abs() + (z - left_z).abs() > sizes.max_width {
            return None;
        }
    }
    if block((left_x - dx, bottom, left_z - dz)) != obsidian {
        return None;
    }

    // Ширина: вправо, пока пусто и под ногами обсидиан.
    let mut width = 0;
    while width <= sizes.max_width {
        let at = (left_x + dx * width, bottom, left_z + dz * width);

        if !fillable(block(at)) {
            break;
        }
        if block((at.0, bottom - 1, at.2)) != obsidian {
            return None;
        }
        width += 1;
    }
    if !(sizes.min_width..=sizes.max_width).contains(&width) || block((left_x + dx * width, bottom, left_z + dz * width)) != obsidian {
        return None;
    }

    // Высота: вверх, пока весь ряд пуст, а по бокам обсидиан.
    let mut height = 0;
    'rows: while height <= sizes.max_height {
        let row = bottom + height;

        for column in 0..width {
            if !fillable(block((left_x + dx * column, row, left_z + dz * column))) {
                break 'rows;
            }
        }
        if block((left_x - dx, row, left_z - dz)) != obsidian
            || block((left_x + dx * width, row, left_z + dz * width)) != obsidian
        {
            return None;
        }
        height += 1;
    }
    if !(sizes.min_height..=sizes.max_height).contains(&height) {
        return None;
    }

    // Потолок из обсидиана.
    for column in 0..width {
        if block((left_x + dx * column, bottom + height, left_z + dz * column)) != obsidian {
            return None;
        }
    }

    Some(Opening { corner: (left_x, bottom, left_z), axis, width, height })
}

/// Заполняет проём порталом.
fn fill(world: &mut World, opening: Opening) {
    let (dx, dz) = opening.axis.step();
    let block = portal(opening.axis);
    let (x, y, z) = opening.corner;

    for row in 0..opening.height {
        for column in 0..opening.width {
            world.set_block(x + dx * column, y + row, z + dz * column, block);
        }
    }

    world.note_portal(opening.corner);
}

/// Зажигалка по этому месту: если оно внутри целой рамки — загорается
/// портал. Возвращает true, если портал появился. `sizes` — какие проёмы
/// зажигаются (`Settings::portal_sizes`).
pub fn light(world: &mut World, at: (i32, i32, i32), sizes: Sizes) -> bool {
    let sizes = sizes.sane();

    for axis in [Axis::X, Axis::Z] {
        if let Some(opening) = opening_at(world, at, axis, sizes) {
            fill(world, opening);
            return true;
        }
    }

    false
}

/// Держится ли портал в этом месте: по своей оси и по вертикали рядом должны
/// быть портал или обсидиан. Нет — портал гаснет (и следом весь проём: у
/// соседних блоков тоже пропадёт опора).
pub fn holds(world: &World, (x, y, z): (i32, i32, i32), state: i32) -> bool {
    let axis = match blocks::orientation_of(state).and_then(|block| block.value(state, "axis")) {
        Some("z") => Axis::Z,
        _ => Axis::X,
    };
    let (dx, dz) = axis.step();
    let good = |block: i32| is_portal(block) || block == obsidian();

    [(dx, 0, dz), (-dx, 0, -dz), (0, 1, 0), (0, -1, 0)]
        .iter()
        .all(|(ox, oy, oz)| good(world.get_block(x + ox, y + oy, z + oz)))
}

/// Куда попадает стоящий в портале: другое измерение и место там —
/// координаты делятся или умножаются на 8.
pub fn destination(from: Dimension, (x, y, z): (f64, f64, f64)) -> (Dimension, (f64, f64, f64)) {
    match from {
        Dimension::Overworld => (Dimension::Nether, (x / 8.0, y, z / 8.0)),
        Dimension::Nether => (Dimension::Overworld, (x * 8.0, y, z * 8.0)),
        // Порталы Незера в Энде не зажечь; на всякий случай — домой.
        Dimension::End => (Dimension::Overworld, (x, y, z)),
    }
}

/// Сколько тактов стоять в портале в выживании у оригинала (в творческом —
/// такт); на сервере — настройка `portal-delay`.
pub const DELAY: u32 = 80;

/// Сколько тактов этому игроку стоять в портале до перехода.
pub fn wait(settings: &crate::config::mcsheriffanya::Settings, creative: bool) -> u32 {
    if creative { 1 } else { settings.portal_delay.max(1) }
}

/// Стоит ли игрок в портале: портал у ног или на уровне груди; портал
/// Края — только у ног (блок ниже человека, в нём стоят).
pub fn standing_in(world: &World, (x, y, z): (f64, f64, f64)) -> bool {
    let (x, y, z) = (x.floor() as i32, y.floor() as i32, z.floor() as i32);

    is_portal(world.get_block(x, y, z)) || is_portal(world.get_block(x, y + 1, z)) || is_end_portal(world.get_block(x, y, z)) || world.gateway_exit((x as f64,y as f64,z as f64)).is_some()
}

/// Портал Края ли этот блок.
pub fn is_end_portal(state: i32) -> bool {
    blocks::block_at_state(state) == Some("end_portal")
}

/// Стоит ли игрок в портале Края: в нём переход мгновенный (вики, End
/// portal: «immediately opens up a loading screen»), а не через 80 тактов.
pub fn standing_in_end_portal(world: &World, (x, y, z): (f64, f64, f64)) -> bool {
    is_end_portal(world.get_block(x.floor() as i32, y.floor() as i32, z.floor() as i32))
}

/// Как расставлены двенадцать рамок вокруг проёма 3×3: смещение от центра
/// и куда рамка смотрит (внутрь), как у оригинала.
const FRAME_RING: [((i32, i32), &str); 12] = [
    ((-1, -2), "south"),
    ((0, -2), "south"),
    ((1, -2), "south"),
    ((-1, 2), "north"),
    ((0, 2), "north"),
    ((1, 2), "north"),
    ((-2, -1), "east"),
    ((-2, 0), "east"),
    ((-2, 1), "east"),
    ((2, -1), "west"),
    ((2, 0), "west"),
    ((2, 1), "west"),
];

/// Событие мира «портал Края открылся» — слышно на весь мир.
pub const END_PORTAL_SPAWN: i32 = 1038;

/// Вложили око в рамку: если теперь все двенадцать рамок кольца, куда
/// входит эта, с глазами и смотрят внутрь, проём 3×3 заполняется порталом
/// Края (блоки внутри исчезают без выпадения). Возвращает центр открытого
/// портала.
pub fn open_end_portal(world: &mut World, (x, y, z): (i32, i32, i32)) -> Option<(i32, i32, i32)> {
    for (offset, _) in FRAME_RING {
        let center = (x - offset.0, y, z - offset.1);

        if FRAME_RING.iter().all(|((dx, dz), facing)| {
            let state = world.get_block(center.0 + dx, y, center.2 + dz);
            let Some(rule) = blocks::orientation("end_portal_frame") else { return false };

            rule.value(state, "eye") == Some("true") && rule.value(state, "facing") == Some(*facing)
        }) {
            let portal = blocks::state_by_name("end_portal")?;

            for dx in -1..=1 {
                for dz in -1..=1 {
                    world.set_block(center.0 + dx, y, center.2 + dz, portal);
                }
            }

            world.note_level_event(center.0, y, center.2, END_PORTAL_SPAWN, 0);

            return Some(center);
        }
    }

    None
}

/// Строит площадку Края: обсидиан 5×5 под ногами и три слоя воздуха над
/// ней (вики, End portal). Игрок встаёт на неё.
pub fn build_end_platform(world: &mut World) -> (f64, f64, f64) {
    let (px, py, pz) = crate::world::the_end::PLATFORM;
    let obsidian = obsidian();

    // Чанки под площадкой должны быть в памяти, иначе блоки не лягут.
    for chunk_x in (px - 2).div_euclid(16)..=(px + 2).div_euclid(16) {
        for chunk_z in (pz - 2).div_euclid(16)..=(pz + 2).div_euclid(16) {
            world.ensure(chunk_x, chunk_z);
        }
    }

    for dx in -2..=2 {
        for dz in -2..=2 {
            world.set_block(px + dx, py, pz + dz, obsidian);

            for dy in 1..=3 {
                if world.get_block(px + dx, py + dy, pz + dz) != AIR {
                    world.set_block(px + dx, py + dy, pz + dz, AIR);
                }
            }
        }
    }

    (f64::from(px) + 0.5, f64::from(py + 1), f64::from(pz) + 0.5)
}

/// Переход через портал из этого места: другое измерение и где там встать
/// (у найденного или построенного парного портала).
pub fn travel(shared: &crate::shared::Shared, from: Dimension, at: (f64, f64, f64)) -> (Dimension, (f64, f64, f64)) {
    if let Some(exit) = shared.world_in(from).lock().expect("мир захвачен другим потоком").gateway_exit(at) { return (from,exit); }
    // [проверить] Автоматическое построение парных врат без exit_portal.
    // Портал Края: из обычного мира и Незера — на площадку, из Края —
    // домой, в обычный мир.
    if standing_in_end_portal(&shared.world_in(from).lock().expect("мир захвачен другим потоком"), at) {
        if from == Dimension::End {
            let spawn = shared.world_in(Dimension::Overworld).lock().expect("мир захвачен другим потоком").spawn_position();

            return (Dimension::Overworld, spawn);
        }

        let mut world = shared.world_in(Dimension::End).lock().expect("мир захвачен другим потоком");
        let place = build_end_platform(&mut world);
        crate::redstone::settle(&mut world);

        return (Dimension::End, place);
    }

    let (dimension, rough) = destination(from, at);
    let mut world = shared.world_in(dimension).lock().expect("мир захвачен другим потоком");
    let place = arrive(&mut world, dimension, rough);
    crate::redstone::settle(&mut world);

    (dimension, place)
}

/// Радиус поиска парного портала: в Незере 16 блоков, в обычном мире 128.
fn search_radius(dimension: Dimension) -> i32 {
    match dimension {
        Dimension::Overworld => 128,
        Dimension::Nether => 16,
        Dimension::End => 16,
    }
}

/// Ищет парный портал рядом с этим местом — по списку проёмов мира, а не
/// обходом сотен чанков: ближайший по горизонтали в радиусе поиска, который
/// всё ещё на месте. Возвращает низ проёма.
fn find(world: &mut World, dimension: Dimension, (x, z): (i32, i32)) -> Option<(i32, i32, i32)> {
    let radius = search_radius(dimension);
    let mut near: Vec<(i32, i32, i32)> = world
        .portals()
        .iter()
        .copied()
        .filter(|(px, _, pz)| (px - x).abs() <= radius && (pz - z).abs() <= radius)
        .collect();

    near.sort_by_key(|(px, _, pz)| i64::from(px - x).pow(2) + i64::from(pz - z).pow(2));

    for corner in near {
        world.ensure(corner.0.div_euclid(16), corner.2.div_euclid(16));

        if is_portal(world.get_block(corner.0, corner.1, corner.2)) {
            return Some(corner);
        }

        // Портал сломали — забыть.
        world.forget_portal(corner);
    }

    None
}

/// Строит портал 2×3 внутри (рамка 4×5) с площадкой из обсидиана и
/// расчищенным местом вокруг, как делает оригинал, когда парного нет.
/// Низ проёма — на высоте `floor`.
fn build(world: &mut World, (x, floor, z): (i32, i32, i32)) -> (i32, i32, i32) {
    let obsidian = obsidian();
    let block = portal(Axis::X);

    // Площадка 4×3 под порталом и воздух над ней, чтобы было где встать.
    for dx in -1..=2 {
        for dz in -1..=1 {
            world.set_block(x + dx, floor - 1, z + dz, obsidian);

            for dy in 0..4 {
                if dz != 0 || !(0..=1).contains(&dx) {
                    world.set_block(x + dx, floor + dy, z + dz, AIR);
                }
            }
        }
    }

    world.note_portal((x, floor, z));

    // Рамка 4×5 вдоль x: столбы по краям, перекладины сверху и снизу.
    for dy in -1..=3 {
        world.set_block(x - 1, floor + dy, z, obsidian);
        world.set_block(x + 2, floor + dy, z, obsidian);
    }
    for dx in 0..=1 {
        world.set_block(x + dx, floor - 1, z, obsidian);
        world.set_block(x + dx, floor + 3, z, obsidian);

        for dy in 0..3 {
            world.set_block(x + dx, floor + dy, z, block);
        }
    }

    (x, floor, z)
}

/// Высоты, на которых в этом измерении может стоять портал.
fn heights(dimension: Dimension) -> (i32, i32) {
    match dimension {
        Dimension::Overworld => (crate::world::MIN_Y + 1, 300),
        Dimension::Nether => (1, 122),
        Dimension::End => (1, 250),
    }
}

/// Место для нового портала рядом с точкой: ближайший снизу пол с тремя
/// пустыми блоками над ним; не нашлось — на той же высоте, в воздухе
/// (площадка всё равно строится).
fn spot(world: &World, dimension: Dimension, (x, y, z): (i32, i32, i32)) -> (i32, i32, i32) {
    let (low, high) = heights(dimension);
    let y = y.clamp(low + 1, high - 4);
    let free = |x: i32, y: i32, z: i32| (0..4).all(|dy| world.get_block(x, y + dy, z) == AIR);
    let floor = |x: i32, y: i32, z: i32| {
        let below = world.get_block(x, y - 1, z);
        below != AIR && crate::fluids::fluid_at(below).is_none()
    };

    for distance in 0..=16 {
        for (dx, dz) in [(0, 0), (distance, 0), (-distance, 0), (0, distance), (0, -distance)] {
            let (px, pz) = (x + dx, z + dz);

            for step in 0..(high - low) {
                for py in [y - step, y + step] {
                    if (low + 1..high - 4).contains(&py) && free(px, py, pz) && free(px + 1, py, pz) && floor(px, py, pz) {
                        return (px, py, pz);
                    }
                }
            }
        }
    }

    (x, y, z)
}

/// Место прибытия через портал: найти парный или построить новый.
/// Возвращает, где встать (середина нижнего блока проёма).
pub fn arrive(world: &mut World, dimension: Dimension, at: (f64, f64, f64)) -> (f64, f64, f64) {
    let (x, y, z) = (at.0.floor() as i32, at.1.floor() as i32, at.2.floor() as i32);

    // Чанки рядом с точкой прибытия — чтобы было где искать место и строить.
    for chunk_x in (x - 16).div_euclid(16)..=(x + 16).div_euclid(16) {
        for chunk_z in (z - 16).div_euclid(16)..=(z + 16).div_euclid(16) {
            world.ensure(chunk_x, chunk_z);
        }
    }

    let place = match find(world, dimension, (x, z)) {
        Some(place) => place,
        None => {
            let place = spot(world, dimension, (x, y, z));
            build(world, place)
        }
    };

    (f64::from(place.0) + 0.5, f64::from(place.1), f64::from(place.2) + 0.5)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Рамка 4×5 из обсидиана вдоль x с полом на высоте 64.
    fn frame(world: &mut World) {
        let obsidian = obsidian();

        for dy in 0..5 {
            world.set_block(0, 64 + dy, 0, obsidian);
            world.set_block(3, 64 + dy, 0, obsidian);
        }
        for dx in 1..=2 {
            world.set_block(dx, 64, 0, obsidian);
            world.set_block(dx, 68, 0, obsidian);
        }
    }

    #[test]
    fn a_frame_lights_up() {
        let mut world = World::in_memory();
        frame(&mut world);

        assert!(light(&mut world, (1, 65, 0), VANILLA));
        for (x, y) in [(1, 65), (2, 65), (1, 67), (2, 67)] {
            assert!(is_portal(world.get_block(x, y, 0)), "нет портала в {} {}", x, y);
        }
    }

    #[test]
    fn an_open_frame_does_not_light() {
        let mut world = World::in_memory();
        frame(&mut world);
        world.set_block(2, 68, 0, AIR);

        assert!(!light(&mut world, (1, 65, 0), VANILLA));
        assert_eq!(world.get_block(1, 65, 0), AIR);
    }

    #[test]
    fn a_broken_frame_does_not_hold() {
        let mut world = World::in_memory();
        frame(&mut world);
        assert!(light(&mut world, (1, 65, 0), VANILLA));

        world.set_block(0, 65, 0, AIR);
        let here = world.get_block(1, 65, 0);
        assert!(!holds(&world, (1, 65, 0), here));
    }

    /// Рамка с проёмом `width` × 3 вдоль x.
    fn wide_frame(world: &mut World, width: i32) {
        let obsidian = obsidian();

        for dy in 0..5 {
            world.set_block(0, 64 + dy, 0, obsidian);
            world.set_block(width + 1, 64 + dy, 0, obsidian);
        }
        for dx in 1..=width {
            world.set_block(dx, 64, 0, obsidian);
            world.set_block(dx, 68, 0, obsidian);
        }
    }

    #[test]
    fn the_opening_sizes_follow_the_settings() {
        let sized = |min_width, min_height, max_width, max_height| Sizes { min_width, min_height, max_width, max_height };

        // 22 в ширину — больше, чем у оригинала: не зажигается.
        let mut world = World::in_memory();
        wide_frame(&mut world, 22);
        assert!(!light(&mut world, (1, 65, 0), VANILLA));

        // С наибольшей шириной 22 — зажигается целиком.
        assert!(light(&mut world, (1, 65, 0), sized(2, 3, 22, 21)));
        assert!(is_portal(world.get_block(22, 67, 0)));

        // Проём 2×3 не зажечь, когда наименьшая ширина 3 или высота 4.
        let mut world = World::in_memory();
        frame(&mut world);
        assert!(!light(&mut world, (1, 65, 0), sized(3, 3, 21, 21)));
        assert!(!light(&mut world, (1, 65, 0), sized(2, 4, 21, 21)));

        // Наименьшая ширина 1 — горит и проём 1×3.
        let mut world = World::in_memory();
        wide_frame(&mut world, 1);
        assert!(!light(&mut world, (1, 65, 0), VANILLA));
        assert!(light(&mut world, (1, 65, 0), sized(1, 3, 21, 21)));

        // Наименьшее больше наибольшего — берётся наибольшее.
        assert_eq!(sized(30, 3, 21, 21).sane(), sized(21, 3, 21, 21));
        assert_eq!(sized(0, 0, 1000, -5).sane(), sized(1, 1, LARGEST, 1));
    }

    #[test]
    fn coordinates_scale_by_eight() {
        assert_eq!(destination(Dimension::Overworld, (800.0, 70.0, -160.0)), (Dimension::Nether, (100.0, 70.0, -20.0)));
        assert_eq!(destination(Dimension::Nether, (10.0, 40.0, 1.0)), (Dimension::Overworld, (80.0, 40.0, 8.0)));
    }

    /// Двенадцать рамок вокруг центра (10, 70, 10): с глазами или без.
    fn ring(world: &mut World, eye: bool) {
        for ((dx, dz), facing) in FRAME_RING {
            let state = blocks::state_from_text(&format!("end_portal_frame[eye={},facing={}]", eye, facing)).unwrap();

            world.set_block(10 + dx, 70, 10 + dz, state);
        }
    }

    #[test]
    fn twelve_eyes_open_the_end_portal() {
        let mut world = World::in_memory();
        ring(&mut world, true);
        world.set_block(10, 70, 10, obsidian());

        assert_eq!(open_end_portal(&mut world, (11, 70, 8)), Some((10, 70, 10)));

        for dx in -1..=1 {
            for dz in -1..=1 {
                assert!(is_end_portal(world.get_block(10 + dx, 70, 10 + dz)), "нет портала в {} {}", dx, dz);
            }
        }

        assert!(standing_in(&world, (10.5, 70.75, 10.5)));
        assert!(standing_in_end_portal(&world, (10.5, 70.75, 10.5)));
    }

    #[test]
    fn eleven_eyes_do_not() {
        let mut world = World::in_memory();
        ring(&mut world, true);
        world.set_block(9, 70, 8, blocks::state_from_text("end_portal_frame[eye=false,facing=south]").unwrap());

        assert_eq!(open_end_portal(&mut world, (10, 70, 8)), None);
        assert!(!is_end_portal(world.get_block(10, 70, 10)));
    }

    #[test]
    fn a_frame_facing_out_breaks_the_ring() {
        let mut world = World::in_memory();
        ring(&mut world, true);
        world.set_block(10, 70, 8, blocks::state_from_text("end_portal_frame[eye=true,facing=north]").unwrap());

        assert_eq!(open_end_portal(&mut world, (11, 70, 8)), None);
    }

    #[test]
    fn the_platform_is_five_by_five_with_air_above() {
        let mut world = World::in_memory();
        world.set_block(101, 50, 1, obsidian());

        let (x, y, z) = build_end_platform(&mut world);

        assert_eq!((x, y, z), (100.5, 49.0, 0.5));

        for dx in -2..=2 {
            for dz in -2..=2 {
                assert_eq!(world.get_block(100 + dx, 48, dz), obsidian());
                assert_eq!(world.get_block(100 + dx, 49, dz), AIR);
                assert_eq!(world.get_block(100 + dx, 51, dz), AIR);
            }
        }

        assert_eq!(world.get_block(103, 48, 0), AIR);
    }

    #[test]
    fn the_end_maps_home_by_the_book() {
        assert_eq!(destination(Dimension::End, (5.0, 60.0, 7.0)).0, Dimension::Overworld);
    }
}
