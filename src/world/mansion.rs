// Лесные особняки (woodland mansion): где стоят и чем сложены.
//
// Где — по вики (Woodland Mansion, «Generation»: «generate rarely in dark
// forests and pale gardens»; structure set: spacing 80, separation 20,
// linear): мир поделён на области 80×80 чанков, в каждой одна попытка —
// чанк со сдвигом 0…59. Биом середины чанка — тёмный лес или бледный сад.
// Случай — свой (своя генерация), закон — оригинала.
//
// Чем — замер мира оригинала 26.1.2 (tools/measure/mansion.py, 24 особняка,
// семя 100554032945340): у оригинала особняк — 490…620 кусков-образцов
// (комнаты 1x1/1x2/2x2, тайные комнаты, стены, полы коридоров, ковры,
// крыши) на случайном плане из клеток. Образцы кусков сняты отдельно
// (src/world/mansion.txt: 72 образца, у каждой клетки — то, что стоит у
// неперекрытых экземпляров; пустота образца — где всё увиденное
// объясняется прежним), а план — целиком, как у снятых особняков
// (src/world/mansion_layouts.txt: куски по порядку сложения с поворотом и
// местом). Особняк повторяет один из снятых планов, повёрнутый на
// случайную четверть; сборка снятых особняков из образцов сходится с ними
// самими на 99,77 % клеток (tools/measure/mansion.py check; остальное —
// формы стенок, панелей, заборов и ступеней на стыках кусков — стенки,
// панели и заборы досчитывает `join_seams`, — и листва деревьев, выросших
// у оригинала позже).
//
// Рамка всего особняка у оригинала одна на поворот (замер: 79 × 61 от угла
// чанка старта, −60…18 × −12…48 без поворота, дальше по часовой −34…26 ×
// −60…18, −4…74 × −34…26, −12…48 × −4…74). Пол первого этажа — на высоте
// верхнего блока земли, наименьшей в четырёх точках: (7, 7) от угла чанка
// старта и на 5 блоков дальше по каждой оси — в сторону поворота (замер:
// 23 особняка из 24 по рельефу того же семени без строений) [проверить:
// один особняк на 2 блока выше].
//
// Под полом первого этажа — фундамент из булыжника до земли (вики:
// «generate a cobblestone foundation underneath the entire structure if
// there are any gaps»; замер: столбец под каждой непустой клеткой пола до
// первого твёрдого блока).
//
// Обитатели (замер entities: вызыватели и поборники стоят на местах
// появления, не исчезают) и сундуки: с таблицей woodland_mansion, с
// предметами образца (аллиумы, саженцы, топор, жемчуг — вики, «Loot»),
// спавнер пауков. Тихони в камерах есть у оригинала (в среднем ~2 на
// камеру), у нас их пока нет [проверить: когда появятся тихони].
//
// Раскладка зависит только от семени и области, поэтому каждый чанк сам
// находит ближние особняки и кладёт только то, что в него попало.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use super::template::{self, Cell, Orient, Template};
use crate::blocks;

/// Область попыток, зазор и соль (вики, Structure set).
pub const SPACING: i32 = 80;
pub const SEPARATION: i32 = 20;
const SALT: u64 = 10_387_319;

/// Наименьший угол рамки от угла чанка старта по повороту (замер) и
/// сторона рамки без поворота (x × z).
const FRAMES: [(i32, i32); 4] = [(-60, -12), (-34, -60), (-4, -34), (-12, -4)];
const FRAME: (i32, i32) = (79, 61);

/// Точки высоты: (7, 7) от угла чанка старта и на 5 дальше в сторону
/// поворота (замер).
const PROBE: i32 = 7;
const PROBE_SPAN: [(i32, i32); 4] = [(5, 5), (-5, 5), (-5, -5), (5, -5)];

/// Как далеко от чанка старта доходит рамка (−60…74 блока): 5 чанков.
const REACH: i32 = 5;

/// Особые клетки образца, кроме блоков: сундук с предметами, спавнер, моб.
#[derive(Default)]
struct Extras {
    items: Vec<((i32, i32, i32), &'static str)>,
    spawners: Vec<((i32, i32, i32), &'static str)>,
    mobs: Vec<((i32, i32, i32), &'static str)>,
}

struct Piece {
    template: usize,
    orient: Orient,
    /// Угол рамки куска от угла рамки особняка и высота от пола входа.
    at: (i32, i32, i32),
}

struct Layout {
    /// Поворот снятого особняка (по рамке).
    rotation: u8,
    pieces: Vec<Piece>,
}

struct Library {
    templates: Vec<Template>,
    extras: Vec<Extras>,
    layouts: Vec<Layout>,
}

fn library() -> &'static Library {
    static LIBRARY: OnceLock<Library> = OnceLock::new();

    LIBRARY.get_or_init(|| parse(include_str!("mansion.txt"), include_str!("mansion_layouts.txt")))
}

/// Образцы (блоки — через template::parse, особые строки — здесь) и планы.
fn parse(text: &'static str, layouts: &str) -> Library {
    let mut plain = String::with_capacity(text.len());
    let mut extras: Vec<Extras> = Vec::new();

    for line in text.lines() {
        let words: Vec<&'static str> = line.split_whitespace().collect();

        match words.as_slice() {
            ["template", ..] => extras.push(Extras::default()),
            [kind @ ("items" | "spawner" | "mob"), a, y, b, data, ..] => {
                let at = (a.parse().unwrap_or(0), y.parse().unwrap_or(0), b.parse().unwrap_or(0));

                if let Some(extra) = extras.last_mut() {
                    match *kind {
                        "items" => extra.items.push((at, data)),
                        "spawner" => extra.spawners.push((at, data)),
                        _ => extra.mobs.push((at, data)),
                    }
                }
                continue;
            }
            _ => {}
        }

        plain.push_str(line);
        plain.push('\n');
    }

    let templates = template::parse(&plain);
    let index: HashMap<&str, usize> = templates.iter().enumerate().map(|(at, template)| (template.name.as_str(), at)).collect();
    let mut out: Vec<Layout> = Vec::new();
    let mut raw: Vec<(usize, Orient, (i32, i32, i32))> = Vec::new();

    fn finish(raw: &mut Vec<(usize, Orient, (i32, i32, i32))>, out: &mut Vec<Layout>) {
        if raw.is_empty() {
            return;
        }

        // Поворот снятого — по углу его рамки (у каждого поворота свой).
        let low = (raw.iter().map(|piece| piece.2.0).min().unwrap_or(0), raw.iter().map(|piece| piece.2.2).min().unwrap_or(0));
        let rotation = FRAMES.iter().position(|frame| *frame == low).unwrap_or(0) as u8;
        let pieces = raw.drain(..).map(|(template, orient, (x, y, z))| Piece { template, orient, at: (x - low.0, y, z - low.1) }).collect();

        out.push(Layout { rotation, pieces });
    }

    for line in layouts.lines() {
        let words: Vec<&str> = line.split_whitespace().collect();

        match words.as_slice() {
            ["mansion", ..] => finish(&mut raw, &mut out),
            ["p", name, turns, mirror, x, y, z] => {
                let Some(&template) = index.get(name) else { continue };
                let number = |word: &str| word.parse::<i32>().unwrap_or(0);
                let orient = Orient { turns: number(turns) as u8 % 4, mirror: *mirror == "1" };

                raw.push((template, orient, (number(x), number(y), number(z))));
            }
            _ => {}
        }
    }
    finish(&mut raw, &mut out);

    Library { templates, extras, layouts: out }
}

/// Особняк области: план, поворот, угол рамки в мире и чанк старта.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Mansion {
    pub layout: usize,
    /// Итоговый поворот (как у рамок оригинала) и сколько четвертей
    /// добавлено к снятому плану.
    pub rotation: u8,
    turn: u8,
    /// Наименьший угол рамки: x, пол входа, z.
    pub origin: (i32, i32, i32),
    pub chunk: (i32, i32),
}

impl Mansion {
    /// Сторона рамки в мире (x, z).
    pub fn footprint(&self) -> (i32, i32) {
        if self.rotation % 2 == 1 { (FRAME.1, FRAME.0) } else { FRAME }
    }
}

pub fn attempt(seed: i64, region_x: i32, region_z: i32) -> (i32, i32) {
    template::attempt(seed, region_x, region_z, SPACING, SEPARATION, SALT)
}

/// Подходит ли биом (вики: тёмный лес и бледный сад).
pub fn biome_fits(biome: super::terrain::Biome) -> bool {
    matches!(biome, super::terrain::Biome::DarkForest | super::terrain::Biome::PaleGarden)
}

/// Высота пола по точкам замера: наименьший верх земли.
fn floor_height(height: &dyn Fn(i32, i32) -> i32, chunk_x: i32, chunk_z: i32, rotation: u8) -> i32 {
    let (x, z) = (chunk_x * 16 + PROBE, chunk_z * 16 + PROBE);
    let (sx, sz) = PROBE_SPAN[usize::from(rotation % 4)];

    [(0, 0), (sx, 0), (0, sz), (sx, sz)].iter().map(|(dx, dz)| height(x + dx, z + dz)).min().unwrap_or(super::terrain::SEA)
}

/// Особняк области обычного мира.
pub fn start_in(terrain: &super::terrain::Terrain, region_x: i32, region_z: i32) -> Option<Mansion> {
    type Starts = Mutex<HashMap<(i64, bool, i32, i32), Option<Mansion>>>;
    static CACHE: OnceLock<Starts> = OnceLock::new();
    #[cfg(not(seedmap_core))]
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    #[cfg(not(seedmap_core))]
    let key = (terrain.seed(), terrain.style() == super::terrain::Style::Vanilla, region_x, region_z);

    #[cfg(not(seedmap_core))]
    if let Some(found) = cache.lock().unwrap_or_else(|poison| poison.into_inner()).get(&key) {
        return *found;
    }

    let seed = terrain.seed();
    let (chunk_x, chunk_z) = attempt(seed, region_x, region_z);
    let found = biome_fits(terrain.column_in(chunk_x * 16 + 8, chunk_z * 16 + 8, None).biome)
        .then(|| place(seed, chunk_x, chunk_z, &|x, z| terrain.column_at(x, z).height));

    #[cfg(not(seedmap_core))]
    cache.lock().unwrap_or_else(|poison| poison.into_inner()).insert(key, found);
    found
}

/// План, поворот и место особняка со стартом в этом чанке.
fn place(seed: i64, chunk_x: i32, chunk_z: i32, height: &dyn Fn(i32, i32) -> i32) -> Mansion {
    let dice = template::roll(seed, chunk_x, chunk_z, SALT ^ 0x3A45);
    let layout = (dice % library().layouts.len() as u64) as usize;
    let rotation = ((dice >> 20) % 4) as u8;
    let turn = (rotation + 4 - library().layouts[layout].rotation) % 4;
    let (fx, fz) = FRAMES[usize::from(rotation)];
    let y = floor_height(height, chunk_x, chunk_z, rotation);

    Mansion { layout, rotation, turn, origin: (chunk_x * 16 + fx, y, chunk_z * 16 + fz), chunk: (chunk_x, chunk_z) }
}

/// Прямоугольник (x, z, ширина, длина) в рамке `width × length`, повёрнутый
/// по часовой на `turns` четвертей вместе с рамкой (как Orient::offset).
fn turn_rect((mut x, mut z, mut sx, mut sz): (i32, i32, i32, i32), (mut width, mut length): (i32, i32), turns: u8) -> (i32, i32, i32, i32) {
    for _ in 0..turns % 4 {
        (x, z, sx, sz) = (length - z - sz, x, sz, sx);
        (width, length) = (length, width);
    }

    (x, z, sx, sz)
}

/// Сундук особняка: таблица добычи или предметы образца.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Chest {
    Loot,
    Items(&'static str),
}

/// Особняк, собранный из кусков: состояния клеток от угла рамки (y — от
/// пола входа), −1 — не трогать.
pub struct Built {
    width: i32,
    length: i32,
    bottom: i32,
    height: i32,
    states: Vec<i32>,
    pub chests: Vec<((i32, i32, i32), Chest)>,
    pub spawners: Vec<((i32, i32, i32), &'static str)>,
    pub mobs: Vec<((i32, i32, i32), &'static str)>,
}

const KEEP: i32 = -1;

impl Built {
    fn index(&self, x: i32, y: i32, z: i32) -> Option<usize> {
        ((0..self.width).contains(&x) && (0..self.length).contains(&z) && (self.bottom..self.bottom + self.height).contains(&y))
            .then(|| (((y - self.bottom) * self.length + z) * self.width + x) as usize)
    }

    /// Состояние клетки (от угла рамки, y от пола входа), если особняк её трогает.
    pub fn state(&self, x: i32, y: i32, z: i32) -> Option<i32> {
        self.index(x, y, z).map(|at| self.states[at]).filter(|state| *state != KEEP)
    }
}

fn state_of(text: &str) -> i32 {
    blocks::state_from_text(text).or_else(|| blocks::state_by_name(text.split('[').next().unwrap_or(text))).unwrap_or(super::AIR)
}

/// Состояние образца в положении куска. Сундуки-половинки в отражённом
/// куске у оригинала одиночные (замер: 1x2_a9 и 2x2_a2 — 180 из 180
/// отражённых половинок single, неотражённые — left/right).
fn turned(text: &str, orient: Orient) -> String {
    let text = template::orient_text(text, orient);

    if orient.mirror && text.starts_with("chest[") {
        return text.replace("type=left", "type=single").replace("type=right", "type=single");
    }

    text
}

/// Собирает план `layout`, повёрнутый ещё на `turn` четвертей.
fn assemble(layout: usize, turn: u8) -> Built {
    let library = library();
    let plan = &library.layouts[layout];
    let frame = if plan.rotation % 2 == 1 { (FRAME.1, FRAME.0) } else { FRAME };
    let (width, length) = if turn % 2 == 1 { (frame.1, frame.0) } else { frame };
    let bottom = plan.pieces.iter().map(|piece| piece.at.1).min().unwrap_or(0);
    let height = plan.pieces.iter().map(|piece| piece.at.1 + library.templates[piece.template].height).max().unwrap_or(0) - bottom;
    let mut built = Built { width, length, bottom, height, states: vec![KEEP; (width * length * height).max(0) as usize], chests: Vec::new(), spawners: Vec::new(), mobs: Vec::new() };
    // Кто положил клетку: для форм на стыках кусков.
    let mut owner = vec![u16::MAX; built.states.len()];
    let mut chests: HashMap<(i32, i32, i32), Chest> = HashMap::new();
    let mut spawners: HashMap<(i32, i32, i32), &'static str> = HashMap::new();

    for (number, piece) in plan.pieces.iter().enumerate() {
        let template = &library.templates[piece.template];
        let extras = &library.extras[piece.template];
        let orient = Orient { turns: (piece.orient.turns + turn) % 4, mirror: piece.orient.mirror };
        let (fx, fz) = template.footprint(piece.orient);
        let (x0, z0, _, _) = turn_rect((piece.at.0, piece.at.2, fx, fz), frame, turn);
        let palette: Vec<i32> = template.texts.iter().map(|text| state_of(&turned(text, orient))).collect();
        let world = |a: i32, y: i32, b: i32| {
            let (dx, dz) = orient.offset(a, b, template.width, template.length);

            (x0 + dx, piece.at.1 + y, z0 + dz)
        };

        for (a, y, b, cell) in template.cells() {
            let (x, wy, z) = world(a, y, b);
            let Some(at) = built.index(x, wy, z) else { continue };

            built.states[at] = match cell {
                Cell::Block(index) => palette[usize::from(index)],
                _ => super::AIR,
            };
            owner[at] = number as u16;
            chests.remove(&(x, wy, z));
            spawners.remove(&(x, wy, z));
        }

        for ((a, y, b), _) in &template.loot {
            chests.insert(world(*a, *y, *b), Chest::Loot);
        }
        for ((a, y, b), items) in &extras.items {
            chests.insert(world(*a, *y, *b), Chest::Items(items));
        }
        for ((a, y, b), kind) in &extras.spawners {
            spawners.insert(world(*a, *y, *b), kind);
        }
        for ((a, y, b), kind) in &extras.mobs {
            built.mobs.push((world(*a, *y, *b), kind));
        }
    }

    join_seams(&mut built, &owner);
    built.chests = chests.into_iter().collect();
    built.chests.sort_by_key(|(at, _)| *at);
    built.spawners = spawners.into_iter().collect();
    built.spawners.sort_by_key(|(at, _)| *at);
    built
}

/// Заборы, панели и стенки на стыках кусков срастаются с соседом из
/// другого куска (замер: у оригинала формы на стыках пересчитаны — сторона
/// к соседнему куску соединена). Стороны внутри куска — как в образце.
fn join_seams(built: &mut Built, owner: &[u16]) {
    use blocks::Family;
    const SIDES: [(&str, i32, i32); 4] = [("north", 0, -1), ("east", 1, 0), ("south", 0, 1), ("west", -1, 0)];
    let family_of = |state: i32| blocks::orientation_of(state).map(|orientation| orientation.family);

    for y in built.bottom..built.bottom + built.height {
        for z in 0..built.length {
            for x in 0..built.width {
                let Some(at) = built.index(x, y, z) else { continue };
                let state = built.states[at];
                let Some(orientation) = blocks::orientation_of(state).filter(|orientation| orientation.family.connects_to_sides()) else {
                    continue;
                };
                let wall = orientation.family == Family::Wall;
                // Высота боковин стенки — как у уже соединённых сторон.
                let tall = SIDES.iter().any(|(side, _, _)| orientation.value(state, side) == Some("tall"));
                let mut updated = state;

                for (side, dx, dz) in SIDES {
                    let Some(near) = built.index(x + dx, y, z + dz) else { continue };

                    if owner[near] == owner[at] || built.states[near] == KEEP {
                        continue;
                    }

                    let neighbour = built.states[near];
                    let solid = neighbour != super::AIR && blocks::has_box(neighbour);
                    let joins = solid
                        && !matches!((orientation.family, family_of(neighbour)), (Family::Fence, Some(Family::Wall)) | (Family::Wall, Some(Family::Fence)));
                    let value = match (wall, joins) {
                        (true, true) if tall => "tall",
                        (true, true) => "low",
                        (true, false) => "none",
                        (false, joins) => if joins { "true" } else { "false" },
                    };

                    updated = orientation.with(updated, side, value).unwrap_or(updated);
                }

                if wall && updated != state {
                    let joined: Vec<bool> = SIDES.iter().map(|(side, _, _)| orientation.value(updated, side) != Some("none")).collect();
                    let straight = (joined[0] && joined[2] && !joined[1] && !joined[3]) || (joined[1] && joined[3] && !joined[0] && !joined[2]);
                    let crossroads = joined.iter().all(|joined| *joined);

                    updated = orientation.with(updated, "up", if straight || crossroads { "false" } else { "true" }).unwrap_or(updated);
                }

                built.states[at] = updated;
            }
        }
    }
}

/// Собранный особняк (по плану и добавленному повороту), из запаса.
fn built(mansion: &Mansion) -> Arc<Built> {
    type Cache = Mutex<HashMap<(usize, u8), Arc<Built>>>;
    static CACHE: OnceLock<Cache> = OnceLock::new();
    #[cfg(not(seedmap_core))]
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    #[cfg(not(seedmap_core))]
    let key = (mansion.layout, mansion.turn);

    #[cfg(not(seedmap_core))]
    if let Some(found) = cache.lock().unwrap_or_else(|poison| poison.into_inner()).get(&key) {
        return found.clone();
    }

    let made = Arc::new(assemble(mansion.layout, mansion.turn));
    #[cfg(not(seedmap_core))]
    let mut cache = cache.lock().unwrap_or_else(|poison| poison.into_inner());

    // Собранный особняк — около 0,6 МБ: держим немного.
    #[cfg(not(seedmap_core))]
    if cache.len() >= 12 {
        cache.clear();
    }
    #[cfg(not(seedmap_core))]
    cache.insert(key, made.clone());
    made
}

/// Особняки, чья рамка задевает чанк.
fn near(terrain: &super::terrain::Terrain, chunk_x: i32, chunk_z: i32) -> Vec<Mansion> {
    let mut out = Vec::new();

    for region_x in (chunk_x - REACH).div_euclid(SPACING)..=(chunk_x + REACH).div_euclid(SPACING) {
        for region_z in (chunk_z - REACH).div_euclid(SPACING)..=(chunk_z + REACH).div_euclid(SPACING) {
            let Some(mansion) = start_in(terrain, region_x, region_z) else { continue };
            let (sx, sz) = mansion.footprint();
            let (x0, _, z0) = mansion.origin;

            if x0 <= chunk_x * 16 + 15 && x0 + sx > chunk_x * 16 && z0 <= chunk_z * 16 + 15 && z0 + sz > chunk_z * 16 {
                out.push(mansion);
            }
        }
    }

    out
}

/// Можно ли залить клетку фундаментом: пусто, жидкость, растения и
/// деревья (у оригинала деревья растут уже после строений — под особняком
/// их нет).
fn under_floor(state: i32) -> bool {
    if state == super::AIR || blocks::replaceable(state) {
        return true;
    }

    blocks::block_at_state(state).is_some_and(|name| {
        matches!(name, "water" | "lava" | "cave_air" | "mushroom_stem" | "brown_mushroom_block" | "red_mushroom_block")
            || name.ends_with("_leaves")
            || name.ends_with("_log")
            || name.ends_with("_wood")
            || name.ends_with("moss_carpet")
            || name.ends_with("hanging_moss")
    })
}

/// Кладёт в сложенный чанк части ближних особняков.
#[cfg(not(seedmap_core))]
pub(super) fn build(generator: &super::Generator, chunk: &mut super::Chunk, chunk_x: i32, chunk_z: i32) {
    let super::Generator::Normal(terrain) = generator else {
        return;
    };

    for mansion in near(terrain, chunk_x, chunk_z) {
        paint(terrain.seed(), &mansion, &built(&mansion), chunk, chunk_x, chunk_z);
    }
}

#[cfg(not(seedmap_core))]
fn paint(seed: i64, mansion: &Mansion, built: &Built, chunk: &mut super::Chunk, chunk_x: i32, chunk_z: i32) {
    let (x0, y0, z0) = mansion.origin;
    let cobblestone = state_of("cobblestone");

    for lx in 0..16 {
        for lz in 0..16 {
            let (x, z) = (chunk_x * 16 + lx - x0, chunk_z * 16 + lz - z0);

            if !(0..built.width).contains(&x) || !(0..built.length).contains(&z) {
                continue;
            }

            for y in built.bottom..built.bottom + built.height {
                if let Some(state) = built.state(x, y, z)
                    && (super::MIN_Y..super::MIN_Y + super::WORLD_HEIGHT).contains(&(y0 + y))
                {
                    chunk.put_generated(lx, y0 + y, lz, state, true);
                }
            }

            // Фундамент: под непустой клеткой пола — до земли.
            if built.state(x, 0, z).is_some_and(|state| state != super::AIR) {
                let mut y = y0 - 1;

                while y >= super::MIN_Y && under_floor(chunk.block(lx, y, lz)) {
                    chunk.put_generated(lx, y, lz, cobblestone, true);
                    y -= 1;
                }
            }
        }
    }

    for ((x, y, z), kind) in &built.chests {
        let (lx, wy, lz) = (x0 + x - chunk_x * 16, y0 + y, z0 + z - chunk_z * 16);

        if !(0..16).contains(&lx) || !(0..16).contains(&lz) {
            continue;
        }

        let trapped = blocks::block_at_state(chunk.block(lx, wy, lz)) == Some("trapped_chest");
        let mut chest = crate::containers::Container::new(if trapped { crate::containers::Kind::TrappedChest } else { crate::containers::Kind::Chest });

        chest.slots = match kind {
            Chest::Loot => {
                let mut dice = crate::loot::Dice::new(template::roll(seed, x0 + x, z0 + z, 0x3A46 ^ (wy as u64)));

                crate::loot::fill(&crate::loot::WOODLAND_MANSION, chest.slots.len(), &mut dice)
            }
            Chest::Items(items) => {
                let mut slots = vec![None; chest.slots.len()];

                for (slot, stack) in items_of(items) {
                    if let Some(cell) = slots.get_mut(slot) {
                        *cell = Some(stack);
                    }
                }
                slots
            }
        };
        chunk.put_generated_container(lx, wy, lz, chest);
    }

    for ((x, y, z), kind) in &built.spawners {
        chunk.put_generated_spawner(x0 + x - chunk_x * 16, y0 + y, z0 + z - chunk_z * 16, crate::spawner::Data::of(kind));
    }
}

/// Предметы сундука образца: «ячейка:предмет:число[:зачарование=уровень]» через «;».
#[cfg(not(seedmap_core))]
fn items_of(text: &str) -> Vec<(usize, crate::inventory::Stack)> {
    let mut out = Vec::new();

    for entry in text.split(';') {
        let parts: Vec<&str> = entry.split(':').collect();
        let (Some(slot), Some(name), Some(count)) = (parts.first().and_then(|slot| slot.parse::<usize>().ok()), parts.get(1), parts.get(2).and_then(|count| count.parse::<i32>().ok())) else {
            continue;
        };
        let Some(item) = blocks::item_named(name) else { continue };
        let mut stack = crate::inventory::Stack::new(item, count);

        if let Some(enchants) = parts.get(3) {
            let mut data = crate::stackdata::Data::default();

            for pair in enchants.split(',') {
                let Some((enchant, level)) = pair.split_once('=') else { continue };

                if let Some(number) = crate::stackdata::ENCHANTMENTS.iter().position(|(known, _)| *known == enchant) {
                    data.set_enchant(number as u16, level.parse().unwrap_or(1));
                }
            }
            stack = stack.with_extras(data);
        }

        out.push((slot, stack));
    }

    out
}

/// Обитатели особняков в чанке: место (середина клетки) и вид
/// («vindicator», «evoker»; тихонь у нас нет — пропускаются).
pub fn dwellers(terrain: &super::terrain::Terrain, chunk_x: i32, chunk_z: i32) -> Vec<((f64, f64, f64), &'static str)> {
    let mut out = Vec::new();

    for mansion in near(terrain, chunk_x, chunk_z) {
        let (x0, y0, z0) = mansion.origin;

        for ((x, y, z), kind) in &built(&mansion).mobs {
            let (x, z) = (x0 + x, z0 + z);

            if x.div_euclid(16) == chunk_x && z.div_euclid(16) == chunk_z && matches!(*kind, "vindicator" | "evoker" | "allay") {
                // Вики, Allay: «one to three» в каждой тюремной камере.
                // Маркеры замера — камеры; [проверить] распределение внутри них.
                let count=if *kind=="allay" {1+template::roll(terrain.seed(),x,z,0xA11A ^ (y0+y) as u64)%3} else {1};
                for _ in 0..count {out.push(((f64::from(x)+0.5,f64::from(y0+y),f64::from(z)+0.5),*kind));}
            }
        }
    }

    out
}

/// Ближайший особняк (`locate structure mansion`): чанк старта.
pub fn nearest(terrain: &super::terrain::Terrain, chunk_x: i32, chunk_z: i32, rings: i32) -> Option<(i32, i32)> {
    template::nearest(chunk_x, chunk_z, SPACING, rings, &|region_x, region_z| start_in(terrain, region_x, region_z).map(|mansion| mansion.chunk))
}

#[cfg(all(test, not(seedmap_core)))]
mod tests {
    use super::*;

    /// 72 образца и 24 плана; каждый кусок плана — известный образец, и
    /// все куски лежат в рамке своего поворота.
    #[test]
    fn templates_and_layouts_parse() {
        let library = library();

        assert_eq!(library.templates.len(), 72);
        assert_eq!(library.layouts.len(), 24);
        assert!(library.templates.iter().any(|template| template.name == "1x2_s1"), "тайная комната");

        for plan in &library.layouts {
            let frame = if plan.rotation % 2 == 1 { (FRAME.1, FRAME.0) } else { FRAME };

            assert!((480..640).contains(&plan.pieces.len()), "кусков {}", plan.pieces.len());
            assert_eq!(library.templates[plan.pieces[0].template].name, "entrance");

            for piece in &plan.pieces {
                let (fx, fz) = library.templates[piece.template].footprint(piece.orient);

                assert!(piece.at.0 >= 0 && piece.at.2 >= 0 && piece.at.0 + fx <= frame.0 && piece.at.2 + fz <= frame.1, "{:?}", piece.at);
            }
        }
    }

    /// Поворот прямоугольника вместе с рамкой — как поворот клеток.
    #[test]
    fn rects_turn_with_the_frame() {
        for turns in 0..4u8 {
            let rect = (3, 5, 4, 2);
            let (x, z, sx, sz) = turn_rect(rect, (10, 7), turns);
            let orient = Orient { turns, mirror: false };
            let corners: Vec<(i32, i32)> = [(3, 5), (6, 6)].iter().map(|(a, b)| orient.offset(*a, *b, 10, 7)).collect();

            assert_eq!(x, corners.iter().map(|c| c.0).min().unwrap());
            assert_eq!(z, corners.iter().map(|c| c.1).min().unwrap());
            assert_eq!((sx, sz), if turns % 2 == 1 { (2, 4) } else { (4, 2) });
        }
    }

    /// Рамка при любом добавленном повороте ложится в рамку оригинала:
    /// поворот всего плана переводит рамку одного поворота в рамку другого
    /// (вход снятого без поворота — (58, 19) от угла рамки, после поворота
    /// по часовой — (41, 58), как у снятых с поворотом).
    #[test]
    fn frames_follow_the_rotation() {
        let (x, z, _, _) = turn_rect((58, 19, 1, 1), FRAME, 1);

        assert_eq!((x, z), (41, 58));
        assert_eq!((-60 + 58, -12 + 19), (-2, 7));
        assert_eq!((FRAMES[1].0 + x, FRAMES[1].1 + z), (7, -2));
    }

    /// Высота пола — наименьший верх земли в четырёх точках поворота.
    #[test]
    fn floor_is_the_lowest_probe() {
        let height = |x: i32, z: i32| if (x, z) == (12, 2) { 70 } else { 80 };

        assert_eq!(floor_height(&height, 0, 0, 3), 70);
        assert_eq!(floor_height(&height, 0, 0, 0), 80);
    }

    #[test]
    fn attempts_keep_the_separation() {
        for region_x in -3..3 {
            for region_z in -3..3 {
                let (x, z) = attempt(42, region_x, region_z);
                let (dx, dz) = (x - region_x * SPACING, z - region_z * SPACING);

                assert!((0..SPACING - SEPARATION).contains(&dx) && (0..SPACING - SEPARATION).contains(&dz));
            }
        }
    }

    /// Сборка: сундуков с добычей столько же, сколько у снятого особняка
    /// (замер: сундуки с LootTable в рамке), и все они — сундуки; поборников
    /// и вызывателей — как у снятого; пол входа на высоте 0.
    #[test]
    fn assembled_mansions_have_rooms_and_dwellers() {
        const LOOT: [usize; 24] = [8, 6, 3, 6, 4, 4, 5, 4, 2, 5, 9, 2, 3, 4, 4, 5, 4, 4, 10, 7, 0, 7, 7, 7];
        // Обитатели снятых особняков (замер entities, `tick freeze`).
        const VINDICATORS: [usize; 24] = [12, 23, 20, 10, 17, 10, 12, 15, 15, 18, 17, 16, 15, 19, 20, 30, 17, 18, 32, 8, 14, 19, 16, 20];
        const EVOKERS: [usize; 24] = [1, 5, 5, 3, 5, 3, 2, 3, 3, 4, 3, 6, 5, 7, 3, 5, 2, 3, 2, 1, 1, 5, 2, 5];

        for layout in 0..library().layouts.len() {
            let built = assemble(layout, (layout % 4) as u8);

            assert_eq!(built.bottom, 0);
            assert_eq!(built.chests.iter().filter(|(_, chest)| *chest == Chest::Loot).count(), LOOT[layout], "план {layout}");
            assert_eq!(built.mobs.iter().filter(|(_, kind)| *kind == "vindicator").count(), VINDICATORS[layout], "план {layout}");
            assert_eq!(built.mobs.iter().filter(|(_, kind)| *kind == "evoker").count(), EVOKERS[layout], "план {layout}");

            for ((x, y, z), _) in &built.chests {
                let name = blocks::block_at_state(built.state(*x, *y, *z).unwrap_or(0)).unwrap_or("");

                assert!(name.ends_with("chest"), "{name}");
            }
        }
    }

    #[test]
    fn prison_templates_have_allay_markers() {
        assert!((0..library().layouts.len()).any(|layout|assemble(layout,0).mobs.iter().any(|(_,kind)|*kind=="allay")));
    }

    /// Предметы образцов: восемь аллиумов, топор с эффективностью I.
    #[test]
    fn template_chests_hold_their_items() {
        let axe = items_of("10:iron_axe:1:efficiency=1");

        assert_eq!(axe.len(), 1);
        assert_eq!(axe[0].0, 10);
        assert_eq!(items_of("8:allium:1;10:allium:1").len(), 2);
    }

    /// Перепись блоков в рамках кусков на особняк против замера оригинала
    /// (tools/measure/mansion.py census, 24 особняка): сборка из образцов по
    /// тем же планам. Без воздуха: у оригинала в рамках ещё рельеф и
    /// деревья, у нас там «не трогать».
    #[test]
    fn mansion_census() {
        const VANILLA: [(&str, f64); 12] = [
            ("dark_oak_planks", 15829.1),
            ("birch_planks", 8476.0),
            ("cobblestone", 3489.2),
            ("dark_oak_log", 2643.3),
            ("red_carpet", 1605.4),
            ("dark_oak_stairs", 1444.3),
            ("white_carpet", 984.6),
            ("cobblestone_stairs", 849.8),
            ("glass_pane", 677.0),
            ("cobblestone_wall", 600.9),
            ("bookshelf", 147.6),
            ("chest", 41.3),
        ];
        let mut counts: HashMap<&str, f64> = HashMap::new();
        let layouts = library().layouts.len();

        for layout in 0..layouts {
            let built = assemble(layout, 0);

            for state in built.states.iter().filter(|state| **state != KEEP) {
                *counts.entry(blocks::block_at_state(*state).unwrap_or("?")).or_default() += 1.0 / layouts as f64;
            }
        }

        for (name, vanilla) in VANILLA {
            let ours = counts.get(name).copied().unwrap_or(0.0);

            eprintln!("{name:20} оригинал {vanilla:9.1} у нас {ours:9.1}");
            assert!((ours - vanilla).abs() <= vanilla * 0.03 + 2.0, "{name}: {ours} против {vanilla}");
        }
    }
    /// Особняки в нашем мире: доля областей с особняком (у оригинала, семя
    /// 100554032945340: `locate` из 169 точек через 5000 блоков нашёл 57 в
    /// квадрате ±32000 — не меньше 2,3 % областей) и перепись сложенных
    /// чанков: стены, комнаты, фундамент, ближайший к нулю.
    #[test]
    #[ignore]
    fn mansion_world() {
        use crate::world::{terrain, Chunk, Generator};

        let generator = Generator::Normal(terrain::Terrain::new(100_554_032_945_340, terrain::Style::Vanilla));
        let Generator::Normal(terrain) = &generator else { unreachable!() };
        let found: Vec<Mansion> = (-25..25).flat_map(|x| (-25..25).map(move |z| (x, z))).filter_map(|(x, z)| start_in(terrain, x, z)).collect();

        println!("особняков в 50×50 областях: {} ({:.1} %)", found.len(), found.len() as f64 / 25.0);
        println!("ближайший к нулю: {:?}", nearest(terrain, 0, 0, 30));

        for mansion in found.iter().take(3) {
            let (sx, sz) = mansion.footprint();
            let (x0, y0, z0) = mansion.origin;
            let mut counts: HashMap<&str, u32> = HashMap::new();
            let mut foundation = 0;

            for chunk_x in x0.div_euclid(16)..=(x0 + sx - 1).div_euclid(16) {
                for chunk_z in z0.div_euclid(16)..=(z0 + sz - 1).div_euclid(16) {
                    let chunk = Chunk::generated(&generator, chunk_x, chunk_z);

                    for lx in 0..16 {
                        for lz in 0..16 {
                            let (x, z) = (chunk_x * 16 + lx, chunk_z * 16 + lz);

                            if !(x0..x0 + sx).contains(&x) || !(z0..z0 + sz).contains(&z) {
                                continue;
                            }

                            for y in y0..y0 + 32 {
                                *counts.entry(blocks::block_at_state(chunk.block(lx, y, lz)).unwrap_or("?")).or_default() += 1;
                            }

                            foundation += (1..40).take_while(|dy| blocks::block_at_state(chunk.block(lx, y0 - dy, lz)) == Some("cobblestone")).count();
                        }
                    }
                }
            }

            let mut top: Vec<_> = counts.into_iter().collect();

            top.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
            println!("{:?} план {} поворот {} фундамент {foundation}: {:?}", mansion.chunk, mansion.layout, mansion.rotation, &top[..10.min(top.len())]);
            assert!(top.iter().any(|(name, count)| *name == "dark_oak_planks" && *count > 10_000));
        }

        assert!(!found.is_empty());
    }
}

