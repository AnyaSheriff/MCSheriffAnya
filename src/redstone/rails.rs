// Рельсы: обычная, с питанием, активирующая и детекторная — как они ложатся,
// соединяются с соседками, держатся на опоре и передают питание.
//
// Всё — по разбору tools/research/components-rest.md §12 и страницам
// minecraft.wiki: Rail, Powered Rail, Activator Rail, Detector Rail,
// Redstone components, Block update. Кода игры и чужих ядер нет.
//
// Чего вики не говорит прямо, помечено «[проверить]» — это сверит чёрный
// ящик (сценарии rail-*).
//
// Рельса отзывается на «сосед изменился», а не на обновление формы: на вики
// (Block update) все четыре вида стоят в списке откликов на NC, а в списке
// откликов на PP их нет. Форму рельса берёт сама, когда её ставят («When
// being placed, it updates its shape» — там же, «Self update»), и тогда же
// перестраивает соседок, которые могут к ней примкнуть. Потом форма
// меняется только у обычной рельсы в развилке — от сигнала.

use super::{Block, Dir, Kind, MAX_POWER, PRIORITY_NORMAL, Pos, any_power, is_leaves, pop, set, step, yes_no};
use crate::blocks;
use crate::world::World;

/// Сколько рельс за исходной передаёт питание в каждую сторону (вики,
/// Powered Rail: «up to eight away from the original activation source.
/// Thus a single activation source can activate up to 17 rails»).
const CHAIN: usize = 8;

/// Раз во сколько тактов включённая детекторная рельса смотрит, есть ли на
/// ней ещё вагонетка (вики, Detector Rail: «Every 20th game tick after the
/// rail first becomes powered, it checks whether any minecart is still
/// present»). Сверено чёрным ящиком (rail-detector): вагонетку уводят через
/// 1, 5, 15, 25 и 35 тактов — рельса гаснет через 20 или 40 тактов после
/// включения.
const DETECTOR_RECHECK: u64 = 20;

/// Форма рельсы — значение свойства `shape`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Shape {
    NorthSouth,
    EastWest,

    /// Наклон: поднимается в эту сторону.
    Ascending(Dir),

    /// Поворот: север или юг и восток или запад.
    Curve(Dir, Dir),
}

impl Shape {
    fn parse(name: &str) -> Option<Shape> {
        Some(match name {
            "north_south" => Shape::NorthSouth,
            "east_west" => Shape::EastWest,
            _ => match name.strip_prefix("ascending_") {
                Some(dir) => Shape::Ascending(Dir::from_name(dir)?),
                None => {
                    let (vertical, horizontal) = name.split_once('_')?;

                    Shape::Curve(Dir::from_name(vertical)?, Dir::from_name(horizontal)?)
                }
            },
        })
    }

    fn name(self) -> &'static str {
        match self {
            Shape::NorthSouth => "north_south",
            Shape::EastWest => "east_west",
            Shape::Ascending(Dir::East) => "ascending_east",
            Shape::Ascending(Dir::West) => "ascending_west",
            Shape::Ascending(Dir::North) => "ascending_north",
            Shape::Ascending(_) => "ascending_south",
            Shape::Curve(Dir::North, Dir::East) => "north_east",
            Shape::Curve(Dir::North, _) => "north_west",
            Shape::Curve(_, Dir::East) => "south_east",
            Shape::Curve(..) => "south_west",
        }
    }

    /// Прямая вдоль этой стороны.
    fn along(dir: Dir) -> Shape {
        match dir {
            Dir::North | Dir::South => Shape::NorthSouth,
            _ => Shape::EastWest,
        }
    }

    /// Куда ведут два конца.
    fn ends(self) -> [Dir; 2] {
        match self {
            Shape::NorthSouth => [Dir::North, Dir::South],
            Shape::EastWest => [Dir::West, Dir::East],
            Shape::Ascending(dir) => [dir.opposite(), dir],
            Shape::Curve(vertical, horizontal) => [vertical, horizontal],
        }
    }

    /// Поднят ли конец в эту сторону.
    fn raised(self, dir: Dir) -> bool {
        self == Shape::Ascending(dir)
    }
}

/// Рельса ли это.
pub(super) fn is_rail(name: &str) -> bool {
    matches!(name, "rail" | "powered_rail" | "activator_rail" | "detector_rail")
}

/// Поворачивает ли рельса: поворот бывает только у обычной (вики, Powered
/// Rail: «Unlike their unpowered counterparts, and like detector and
/// activator rails, powered rails are always straight»).
fn bends(name: &str) -> bool {
    name == "rail"
}

/// Форма рельсы в этом месте, если там рельса.
pub(super) fn shape_of(block: &Block) -> Option<Shape> {
    if !is_rail(block.name) {
        return None;
    }

    Shape::parse(block.value("shape")?)
}

/// Высота конца рельсы в сторону `dir`: на уровне рельсы или на блок выше,
/// если она туда поднимается. None — конца в ту сторону нет.
fn end_height(pos: Pos, shape: Shape, dir: Dir) -> Option<i32> {
    shape
        .ends()
        .contains(&dir)
        .then_some(pos.1 + i32::from(shape.raised(dir)))
}

/// Сторона от одного места к соседнему по горизонтали (высота не важна).
fn dir_between(from: Pos, to: Pos) -> Option<Dir> {
    match (to.0 - from.0, to.2 - from.2) {
        (0, -1) => Some(Dir::North),
        (0, 1) => Some(Dir::South),
        (-1, 0) => Some(Dir::West),
        (1, 0) => Some(Dir::East),
        _ => None,
    }
}

/// Соединены ли две соседние рельсы: концы смотрят друг на друга и сходятся
/// по высоте.
fn linked(a: Pos, a_shape: Shape, b: Pos, b_shape: Shape) -> bool {
    let Some(dir) = dir_between(a, b) else {
        return false;
    };

    match (end_height(a, a_shape, dir), end_height(b, b_shape, dir.opposite())) {
        (Some(here), Some(there)) => here == there,
        _ => false,
    }
}

/// Рельсы в соседнем столбце в сторону `dir`: на той же высоте, на блок
/// выше и на блок ниже — в таком порядке.
fn rails_toward(world: &World, pos: Pos, dir: Dir) -> impl Iterator<Item = (Pos, Block, Shape)> + '_ {
    let beside = step(pos, dir);

    [0, 1, -1].into_iter().filter_map(move |dy| {
        let at = (beside.0, beside.1 + dy, beside.2);
        let block = Block::at(world, at);

        shape_of(&block).map(|shape| (at, block, shape))
    })
}

/// Соседка, с которой рельса соединена в сторону `dir`.
fn linked_toward(world: &World, pos: Pos, shape: Shape, dir: Dir) -> Option<(Pos, Block, Shape)> {
    if !shape.ends().contains(&dir) {
        return None;
    }

    rails_toward(world, pos, dir).find(|(at, _, other)| linked(pos, shape, *at, *other))
}

/// Стороны, где рельса и правда соединена с соседкой.
fn links(world: &World, pos: Pos, shape: Shape) -> Vec<Dir> {
    shape
        .ends()
        .into_iter()
        .filter(|dir| linked_toward(world, pos, shape, *dir).is_some())
        .collect()
}

/// Может ли рельса `other` примкнуть к рельсе в `pos`: она уже смотрит туда
/// концом на подходящей высоте или у неё свободен конец — соединена меньше
/// чем с двумя. Рельса в линии занята: новая рядом с серединой линии с ней
/// не соединяется (вики, Rail: «or if placed beside an existing rail line»).
fn can_join(world: &World, other: Pos, other_shape: Shape, pos: Pos) -> bool {
    let Some(dir) = dir_between(other, pos) else {
        return false;
    };

    if let Some(height) = end_height(other, other_shape, dir) {
        // Конец на нашей высоте или на блок выше, если сама соседка на блок
        // выше: тогда к ней поднимемся мы.
        if height == pos.1 || (height == pos.1 + 1 && other.1 == pos.1 + 1) {
            return true;
        }
    }

    links(world, other, other_shape).len() < 2
}

/// Соседка в сторону `dir`, которая может примкнуть: первая рельса в том
/// столбце (своя высота, выше, ниже), если она не занята.
fn candidate(world: &World, pos: Pos, dir: Dir) -> Option<Pos> {
    let (at, _, shape) = rails_toward(world, pos, dir).next()?;

    can_join(world, at, shape, pos).then_some(at)
}

/// Прямая получает наклон, если рельса на одном из её концов стоит на блок
/// выше. Если выше оба — предпочтение «west, east, south, and north» (вики,
/// Rail и Powered Rail): запад раньше востока, юг раньше севера.
fn with_slope(shape: Shape, higher: impl Fn(Dir) -> bool) -> Shape {
    let order = match shape {
        Shape::NorthSouth => [Dir::South, Dir::North],
        Shape::EastWest => [Dir::West, Dir::East],
        _ => return shape,
    };

    order
        .into_iter()
        .find(|dir| higher(*dir))
        .map_or(shape, Shape::Ascending)
}

/// Какую форму рельса возьмёт по соседкам, которые могут к ней примкнуть.
///
/// Вики (Rail, Redstone components): соседки по одной оси — прямая по ней;
/// две под углом — поворот; три или четыре — поворот, «a rail prefers south
/// over north, and east over west», а под сигналом наоборот («A rail's
/// preference inverts when a redstone signal is applied»). Прямые рельсы
/// при соседках по обеим осям ложатся восток–запад (Powered Rail, Detector
/// Rail). Соседок нет — форма остаётся прежней: при постановке это взгляд
/// игрока («the new rail orients itself in the direction the player is
/// facing»), при постановке командой — заданная.
fn wanted_shape(world: &World, pos: Pos, block: &Block, current: Shape) -> Shape {
    let near = |dir| candidate(world, pos, dir);
    let (north, south, west, east) = (near(Dir::North), near(Dir::South), near(Dir::West), near(Dir::East));
    let vertical = north.is_some() || south.is_some();
    let horizontal = west.is_some() || east.is_some();

    let shape = match (vertical, horizontal) {
        (false, false) => return current,
        (true, false) => Shape::NorthSouth,
        (false, true) => Shape::EastWest,
        (true, true) if bends(block.name) => {
            let powered = any_power(world, pos);
            let vertical = match (powered, north.is_some(), south.is_some()) {
                (false, _, true) | (true, false, _) => Dir::South,
                _ => Dir::North,
            };
            let horizontal = match (powered, west.is_some(), east.is_some()) {
                (false, _, true) | (true, false, _) => Dir::East,
                _ => Dir::West,
            };

            Shape::Curve(vertical, horizontal)
        }
        (true, true) => Shape::EastWest,
    };

    let found = |dir: Dir| match dir {
        Dir::North => north,
        Dir::South => south,
        Dir::West => west,
        _ => east,
    };

    with_slope(shape, |dir| found(dir).is_some_and(|at| at.1 == pos.1 + 1))
}

/// Форма соседки, которая примыкает к новой рельсе со стороны `toward`,
/// сохраняя свои живые соединения.
///
/// Вики (Rail): «An existing straight, north-south rail re-orients to
/// east-west when a new rail is placed at the east or west sides» и может
/// стать поворотом или наклоном; прямые рельсы с соседками по обеим осям —
/// восток–запад, «even if it is already connected to another rail to the
/// north or south» (Powered Rail).
fn joined_shape(world: &World, pos: Pos, block: &Block, shape: Shape, toward: Dir, new_rail: Pos) -> Option<Shape> {
    let kept = links(world, pos, shape);

    if kept.len() >= 2 {
        return None;
    }

    let joined = match kept.first() {
        None => Shape::along(toward),
        Some(&other) if other == toward.opposite() || other == toward => Shape::along(toward),
        Some(_) if !bends(block.name) => Shape::EastWest,
        Some(&other) => {
            let (vertical, horizontal) = if matches!(other, Dir::West | Dir::East) {
                (toward, other)
            } else {
                (other, toward)
            };

            Shape::Curve(vertical, horizontal)
        }
    };

    // Наклон — к той, что выше: к новой рельсе или к прежней соседке.
    Some(with_slope(joined, |dir| {
        if dir == toward {
            return new_rail.1 == pos.1 + 1;
        }

        kept.contains(&dir)
            && linked_toward(world, pos, shape, dir).is_some_and(|(at, _, _)| at.1 == pos.1 + 1)
    }))
}

/// Новая рельса легла: соседки, в которые смотрят её концы и которые ещё не
/// соединены с ней, перестраиваются к ней, если у них свободен конец.
fn join_neighbours(world: &mut World, pos: Pos, shape: Shape) {
    for dir in shape.ends() {
        // Конец поднят — соседка на блок выше; иначе на той же высоте или
        // на блок ниже (она поднимется к нам).
        let heights: &[i32] = if shape.raised(dir) { &[1] } else { &[0, -1] };
        let beside = step(pos, dir);
        let found = heights.iter().find_map(|dy| {
            let at = (beside.0, beside.1 + dy, beside.2);
            let block = Block::at(world, at);

            shape_of(&block).map(|other| (at, block, other))
        });

        let Some((at, block, other)) = found else {
            continue;
        };

        if linked(pos, shape, at, other) {
            continue;
        }

        if let Some(joined) = joined_shape(world, at, &block, other, dir.opposite(), pos)
            && joined != other
            && let Some(state) = block.with("shape", joined.name())
        {
            set(world, at, state);
        }
    }
}

/// Держит ли блок рельсу сверху: «a block whose top face has a rim around
/// the edge, such as any full block, hopper, top slab, upside-down stairs,
/// or top trapdoor» (вики, Rail). Открытый люк не держит (вики, Detector
/// Rail: «If a detector rail is updated while on an opened top trapdoor, the
/// rail breaks»). Листва не держит (вики, Conductivity).
pub fn holds_rail(state: i32) -> bool {
    let Some(name) = blocks::block_at_state(state) else {
        return false;
    };

    if name.ends_with("_trapdoor") {
        let value = |property| blocks::orientation(name).and_then(|block| block.value(state, property));

        return value("half") == Some("top") && value("open") != Some("true");
    }

    !is_leaves(state) && blocks::carries_dust(state)
}

/// Держится ли рельса: опора снизу, а у наклона — ещё и блок, на который он
/// опирается поднятым концом. [проверить]: второе на вики не описано —
/// сценарий rail-slope-support.
fn supported(world: &World, pos: Pos, shape: Shape) -> bool {
    if !holds_rail(world.get_block(pos.0, pos.1 - 1, pos.2)) {
        return false;
    }

    match shape {
        Shape::Ascending(dir) => {
            let high = step(pos, dir);

            holds_rail(world.get_block(high.0, high.1, high.2))
        }
        _ => true,
    }
}

/// Рельсу поставили (игрок, команда, поршень): она берёт форму по соседкам
/// и перестраивает их к себе. Прямые рельсы тут же проверяют себя как по
/// «сосед изменился» — опору и питание; детекторная — есть ли на ней
/// вагонетка (вики, Block update, «Self update»: «When being placed, it
/// checks whether it is pressed and updates its state»). [проверить]:
/// проверяет ли опору при постановке обычная рельса.
pub(super) fn placed(world: &mut World, pos: Pos, block: &Block) {
    let Some(current) = shape_of(block) else {
        return;
    };

    let shape = wanted_shape(world, pos, block, current);

    if shape != current
        && let Some(state) = block.with("shape", shape.name())
    {
        set(world, pos, state);
    }

    join_neighbours(world, pos, shape);

    let block = Block::at(world, pos);

    match block.kind {
        Kind::PoweredRail => neighbour_changed(world, pos, &block, false),
        Kind::DetectorRail => {
            if !supported(world, pos, shape) {
                pop(world, pos);
                return;
            }

            if !block.is("powered", "true") && detector_has_cart(world, pos) {
                press(world, pos, &block);
            }
        }
        _ => {}
    }
}

/// У рельсы изменился сосед. Все виды проверяют опору. Обычная рельса
/// с тремя-четырьмя соседками перестраивает поворот, если обновление
/// пришло от источника сигнала (вики, Block update: «a rail accepts only NC
/// updates from power components»; Redstone components: «When activated, a
/// rail in a "T" junction flips to curve the other way (activating a rail in
/// another configuration has no effect)»). Рельсы с питанием и
/// активирующие пересчитывают питание.
///
/// `from_signal` — пришло ли обновление от детали, дающей сигнал.
pub(super) fn neighbour_changed(world: &mut World, pos: Pos, block: &Block, from_signal: bool) {
    let Some(shape) = shape_of(block) else {
        return;
    };

    if !supported(world, pos, shape) {
        pop(world, pos);
        return;
    }

    match block.kind {
        Kind::Rail if from_signal => {
            // [проверить]: считаются ли соседки на блок выше и ниже.
            let around = super::HORIZONTAL
                .iter()
                .filter(|dir| rails_toward(world, pos, **dir).next().is_some())
                .count();

            if around >= 3 {
                let wanted = wanted_shape(world, pos, block, shape);

                if wanted != shape
                    && let Some(state) = block.with("shape", wanted.name())
                {
                    set(world, pos, state);

                    // Повернула к одиночной соседке — та поворачивает к ней,
                    // как при постановке (чёрный ящик, rail-shapes).
                    join_neighbours(world, pos, wanted);
                }
            }
        }

        Kind::PoweredRail => {
            let powered = rail_powered(world, pos, block, shape);

            if powered != block.is("powered", "true")
                && let Some(state) = block.with("powered", yes_no(powered))
            {
                set(world, pos, state);
            }
        }

        _ => {}
    }
}

/// Запитана ли рельса с питанием или активирующая: прямо от источника или
/// запитанного блока — или через цепочку соединённых рельс того же вида,
/// в которой одна из первых восьми в какую-либо сторону запитана прямо.
/// Сила исходного сигнала на дальность не влияет, передаёт только та, что
/// запитана прямо (вики, Powered Rail, Activator Rail, Redstone components:
/// «Only the original activated activator rail can activate other activator
/// rails»).
fn rail_powered(world: &World, pos: Pos, block: &Block, shape: Shape) -> bool {
    any_power(world, pos)
        || shape
            .ends()
            .into_iter()
            .any(|dir| chain_powered(world, pos, shape, dir, block.name))
}

/// Идёт по цепочке рельс вида `name` в сторону `dir`: найдётся ли в пределах
/// восьми запитанная прямо. Такие рельсы всегда прямые, так что сторона
/// хода не меняется; наклоны цепочку не рвут.
fn chain_powered(world: &World, pos: Pos, shape: Shape, dir: Dir, name: &str) -> bool {
    let (mut here, mut here_shape) = (pos, shape);

    for _ in 0..CHAIN {
        let Some((next, next_block, next_shape)) = linked_toward(world, here, here_shape, dir) else {
            return false;
        };

        if next_block.name != name {
            return false;
        }

        if any_power(world, next) {
            return true;
        }

        (here, here_shape) = (next, next_shape);
    }

    false
}

/// Кого ещё будит рельса с питанием или активирующая, у которой сменилось
/// питание: соседей блока под собой, а наклонная — и соседей блока над
/// собой. Вики этого не расписывает (Block update, «Rails — info needed»).
/// [проверить]: сценарий rail-powered-chain (наблюдатель под рельсой).
pub(super) fn power_reach(block: &Block, pos: Pos) -> Vec<Pos> {
    let around = |centre: Pos| super::NEIGHBOUR_ORDER.iter().map(move |dir| step(centre, *dir));
    let mut reach: Vec<Pos> = around(step(pos, Dir::Down)).collect();

    if shape_of(block).is_some_and(|shape| matches!(shape, Shape::Ascending(_))) {
        reach.extend(around(step(pos, Dir::Up)));
    }

    reach
}

// ---------------------------------------------------------------------------
// Детекторная рельса
// ---------------------------------------------------------------------------

/// Вагонетка, как её видят детекторные рельсы: где стоит и что покажет
/// сравнитель. Кладёт их сюда такт мира (вагонетки — minecart.rs).
#[derive(Clone, Copy, Debug)]
pub struct Cart {
    /// Середина низа ящика.
    pub x: f64,
    pub y: f64,
    pub z: f64,

    /// Заполненность для сравнителя — у вагонетки с сундуком и с воронкой;
    /// у прочих None, сравнитель тогда покажет 0.
    pub reading: Option<u8>,
}

/// Ящик вагонетки: 0,98 × 0,7 (components-rest.md §12.5, minecraft-data).
const CART_WIDTH: f64 = crate::minecart::WIDTH;
const CART_HEIGHT: f64 = crate::minecart::HEIGHT;

impl Cart {
    /// Касается ли ящик вагонетки клетки рельсы. Вики (Detector Rail):
    /// «when any part of any minecart's bounding box comes into contact with
    /// it». [проверить]: касание самой границы клетки.
    fn touches(&self, pos: Pos) -> bool {
        let half = CART_WIDTH / 2.0;

        self.x - half < (pos.0 + 1) as f64
            && self.x + half > pos.0 as f64
            && self.z - half < (pos.2 + 1) as f64
            && self.z + half > pos.2 as f64
            && self.y < (pos.1 + 1) as f64
            && self.y + CART_HEIGHT > pos.1 as f64
    }
}

/// Первая по порядку появления вагонетка на детекторной рельсе в этом месте
/// — сравнитель читает её («reads only the minecart that was placed
/// first»).
pub(super) fn cart_on(world: &World, pos: Pos) -> Option<Cart> {
    world.redstone.carts.iter().find(|cart| cart.touches(pos)).copied()
}

/// Есть ли на детекторной рельсе вагонетка.
pub fn detector_has_cart(world: &World, pos: Pos) -> bool {
    cart_on(world, pos).is_some()
}

/// Что сравнитель читает с детекторной рельсы: включённая — заполненность
/// вагонетки с сундуком или воронкой, иначе 0 (вики, Detector Rail: «For
/// any other type of minecart … the comparator's output is zero»).
pub fn comparator_reading(world: &World, pos: Pos, block: &Block) -> u8 {
    if !block.is("powered", "true") {
        return 0;
    }

    cart_on(world, pos).and_then(|cart| cart.reading).unwrap_or(0).min(MAX_POWER)
}

/// Вагонетка наехала: рельса включается и через 20 тактов проверит себя.
fn press(world: &mut World, pos: Pos, block: &Block) {
    if let Some(state) = block.with("powered", "true") {
        set(world, pos, state);
    }

    world.schedule_once(pos.0, pos.1, pos.2, DETECTOR_RECHECK, PRIORITY_NORMAL);
    super::settle(world);
    super::comparator_update(world, pos);
}

/// Фаза сущностей: детекторные рельсы под вагонетками включаются.
pub(super) fn press_detectors(world: &mut World) {
    if world.redstone.carts.is_empty() {
        return;
    }

    let mut pressed: Vec<Pos> = Vec::new();

    for cart in &world.redstone.carts {
        let half = CART_WIDTH / 2.0;

        for x in (cart.x - half).floor() as i32..=(cart.x + half).floor() as i32 {
            for y in cart.y.floor() as i32..=(cart.y + CART_HEIGHT).floor() as i32 {
                for z in (cart.z - half).floor() as i32..=(cart.z + half).floor() as i32 {
                    if cart.touches((x, y, z)) && !pressed.contains(&(x, y, z)) {
                        pressed.push((x, y, z));
                    }
                }
            }
        }
    }

    for pos in pressed {
        let block = Block::at(world, pos);

        if block.kind == Kind::DetectorRail && !block.is("powered", "true") {
            press(world, pos, &block);
        }
    }
}

/// Такт детекторной рельсы: вагонетка ещё здесь — ждём следующие 20 тактов,
/// нет — рельса выключается.
pub(super) fn detector_ticked(world: &mut World, pos: Pos, block: &Block) {
    if !block.is("powered", "true") {
        return;
    }

    if detector_has_cart(world, pos) {
        world.schedule_once(pos.0, pos.1, pos.2, DETECTOR_RECHECK, PRIORITY_NORMAL);
    } else if let Some(state) = block.with("powered", "false") {
        set(world, pos, state);
        super::settle(world);
        super::comparator_update(world, pos);
    }
}

/// Кого будит детекторная рельса, сменившая питание, кроме своих соседей:
/// соседей блока под собой — его она сильно питает (вики, Detector Rail:
/// «strongly powers the block supporting the detector rail»).
pub(super) fn detector_reach(pos: Pos) -> Vec<Pos> {
    let below = step(pos, Dir::Down);

    super::NEIGHBOUR_ORDER.iter().map(|dir| step(below, *dir)).collect()
}
