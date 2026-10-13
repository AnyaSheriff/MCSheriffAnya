// Храм в пустыне (desert_pyramid), храм в джунглях (jungle_pyramid) и хижина
// ведьмы (swamp_hut): где стоят и чем сложены.
//
// Где — по вики (Structure set: desert_pyramids, jungle_temples, swamp_huts —
// «spacing 32, separation 8», разброс линейный): мир поделён на области
// 32×32 чанка, в каждой одна попытка — чанк со сдвигом 0…23 по каждой оси.
// Постройка встаёт, если биом подходит (вики: пустыня; джунгли и бамбуковые
// джунгли, не редкие; болото, не мангровое). Угол рамки — угол чанка старта
// (замер: тот же угол у всех 36 экземпляров), сторона — случайная из
// четырёх. Случай — свой (своя генерация), закон — оригинала.
//
// Высота — замер (tools/measure/temples.py ground, по 12 экземпляров): низ
// рамки пирамиды — у нижней точки рельефа вокруг, храма в джунглях и хижины —
// у средней высоты рельефа вокруг плюс один [проверить: у оригинала — по
// клеткам рамки].
//
// Чем — замер мира оригинала 26.1.2 (tools/measure/temples.py templates):
// сервер оригинала только складывал мир, а блоки сняты с region-файлов в
// рамках (записи о строениях: куски tedp, tejp, tesh — рамка и сторона O).
// Все экземпляры сведены в рамку куска (src/world/temples.txt): клетка —
// блок, общий для экземпляров, набор с долями — случайный блок (мшистый
// булыжник в джунглях, подозрительный песок в засыпанной комнате пирамиды),
// «.» — рельеф. Сундуки — с добычей вики (loot.rs), раздатчики джунглей —
// со стрелами. Под пирамидой — песчаник до твёрдого (вики: «the foundation
// extends downward until it reaches the bottom»), под сваями хижины — дубовые
// брёвна до земли (вики: «oak log pillars extending down to the ground»).
// В хижине при сложении — ведьма и чёрная кошка (вики, Swamp Hut, «Mobs»;
// замер entities: обе в клетке (2, 2, 5) рамки, не исчезают).
//
// Раскладка зависит только от семени и области, поэтому каждый чанк сам
// находит ближние постройки и кладёт только то, что в него попало.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::blocks;

/// Область попыток и наименьший зазор (вики: 32 и 8 чанков).
pub const SPACING: i32 = 32;
pub const SEPARATION: i32 = 8;

/// Вид постройки.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Kind {
    DesertPyramid,
    JunglePyramid,
    SwampHut,
}

impl Kind {
    pub const ALL: [Kind; 3] = [Kind::DesertPyramid, Kind::JunglePyramid, Kind::SwampHut];

    /// Имя в `locate structure` и в образцах.
    pub fn name(self) -> &'static str {
        match self {
            Kind::DesertPyramid => "desert_pyramid",
            Kind::JunglePyramid => "jungle_pyramid",
            Kind::SwampHut => "swamp_hut",
        }
    }

    /// Своя соль: у каждого вида свой набор попыток (вики: у наборов разные
    /// соли).
    fn salt(self) -> u64 {
        match self {
            Kind::DesertPyramid => 14_357_617,
            Kind::JunglePyramid => 0x7E3D_0002,
            Kind::SwampHut => 0x7E3D_0003,
        }
    }

    fn biome_fits(self, biome: super::terrain::Biome) -> bool {
        use super::terrain::Biome;

        match self {
            Kind::DesertPyramid => biome == Biome::Desert,
            Kind::JunglePyramid => matches!(biome, Biome::Jungle | Biome::BambooJungle),
            Kind::SwampHut => biome == Biome::Swamp,
        }
    }
}

/// Клетка палитры: блок или случайный из набора (состояние, вес).
#[derive(Clone, Debug)]
enum Choice {
    Fixed(String),
    Random(Vec<(String, u32)>),
}

struct Template {
    /// Ширина (a), длина (b), высота и низ (от низа рамки, может быть ниже 0).
    width: i32,
    depth: i32,
    height: i32,
    low: i32,
    /// Палитра: номер в клетке − 1 (0 — рельеф).
    palette: Vec<Choice>,
    cells: Vec<u8>,
}

impl Template {
    fn cell(&self, a: i32, y: i32, b: i32) -> u8 {
        self.cells[(((y - self.low) * self.depth + b) * self.width + a) as usize]
    }
}

fn parse(text: &str, names: &[&'static str]) -> HashMap<&'static str, Template> {
    let mut out: HashMap<&'static str, Template> = HashMap::new();
    let mut current: Option<&'static str> = None;
    let mut symbols: HashMap<char, u8> = HashMap::new();
    let (mut layer, mut row) = (0, 0);

    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }

        let words: Vec<&str> = line.split_whitespace().collect();

        if !line.starts_with(' ') {
            if words[0] == "layer" {
                layer = words.get(1).and_then(|y| y.parse().ok()).unwrap_or(0);
                row = 0;
                continue;
            }

            let Some(&kind) = names.iter().find(|name| **name == words[0]) else {
                current = None;
                continue;
            };
            let number = |at: usize| words.get(at).and_then(|value| value.parse::<i32>().ok()).unwrap_or(0);
            let (width, depth, height, low) = (number(1), number(2), number(3), number(4));

            out.insert(kind, Template { width, depth, height, low, palette: Vec::new(), cells: vec![0; (width * depth * height).max(0) as usize] });
            current = Some(kind);
            symbols.clear();
            continue;
        }

        let Some(template) = current.and_then(|name| out.get_mut(name)) else {
            continue;
        };

        if words.len() >= 2 && words[0].chars().count() == 1 {
            let weighted: Vec<(String, u32)> = words[1..]
                .iter()
                .filter_map(|word| word.rsplit_once('=').and_then(|(state, weight)| weight.parse().ok().map(|weight| (state.to_string(), weight))))
                .collect();
            let choice = if weighted.len() == words.len() - 1 && weighted.len() > 1 { Choice::Random(weighted) } else { Choice::Fixed(words[1].to_string()) };

            template.palette.push(choice);
            symbols.insert(words[0].chars().next().unwrap_or(' '), template.palette.len() as u8);
        } else if words.len() == 1 {
            for (a, symbol) in words[0].chars().enumerate().take(template.width as usize) {
                let at = (((layer - template.low) * template.depth + row) * template.width + a as i32) as usize;

                if let Some(cell) = template.cells.get_mut(at) {
                    *cell = symbols.get(&symbol).copied().unwrap_or(0);
                }
            }

            row += 1;
        }
    }

    out
}

fn library() -> &'static HashMap<&'static str, Template> {
    static LIBRARY: OnceLock<HashMap<&'static str, Template>> = OnceLock::new();

    LIBRARY.get_or_init(|| parse(include_str!("temples.txt"), &["desert_pyramid", "jungle_pyramid", "swamp_hut"]))
}

fn scramble(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn roll(seed: i64, x: i32, z: i32, salt: u64) -> u64 {
    scramble((seed as u64) ^ (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (z as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ salt)
}

/// Чанк попытки в области: линейный сдвиг 0…23 по каждой оси.
pub fn attempt(kind: Kind, seed: i64, region_x: i32, region_z: i32) -> (i32, i32) {
    let dice = roll(seed, region_x, region_z, kind.salt());
    let span = (SPACING - SEPARATION) as u64;

    (region_x * SPACING + (dice % span) as i32, region_z * SPACING + ((dice >> 20) % span) as i32)
}

/// Постройка: чанк старта, сторона и низ рамки.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Start {
    pub kind: Kind,
    pub chunk: (i32, i32),
    pub facing: u8,
    pub y: i32,
}

impl Start {
    fn template(&self) -> Option<&'static Template> {
        library().get(self.kind.name())
    }

    /// Рамка в плане: x0, z0, x1, z1.
    pub fn area(&self) -> [i32; 4] {
        let Some(template) = self.template() else {
            return [0; 4];
        };
        let (x0, z0) = (self.chunk.0 * 16, self.chunk.1 * 16);
        let (sx, sz) = if self.facing.is_multiple_of(2) { (template.width, template.depth) } else { (template.depth, template.width) };

        [x0, z0, x0 + sx - 1, z0 + sz - 1]
    }

    /// Клетка (a, b) рамки в мире (вбок — a, вперёд — b; у сторон 2 и 3
    /// рамка зеркальна, как у кусков крепостей Незера).
    fn world(&self, a: i32, b: i32) -> (i32, i32) {
        let [x0, z0, x1, z1] = self.area();

        match self.facing {
            0 => (x0 + a, z0 + b),
            2 => (x0 + a, z1 - b),
            1 => (x1 - b, z0 + a),
            _ => (x0 + b, z0 + a),
        }
    }
}

/// Стоит ли постройка вида в области.
pub fn start_in(kind: Kind, terrain: &super::terrain::Terrain, region_x: i32, region_z: i32) -> Option<Start> {
    type Starts = Mutex<HashMap<(Kind, i64, bool, i32, i32), Option<Start>>>;
    static CACHE: OnceLock<Starts> = OnceLock::new();
    #[cfg(not(seedmap_core))]
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    #[cfg(not(seedmap_core))]
    let key = (kind, terrain.seed(), terrain.style() == super::terrain::Style::Vanilla, region_x, region_z);

    #[cfg(not(seedmap_core))]
    if let Some(found) = cache.lock().unwrap_or_else(|poison| poison.into_inner()).get(&key) {
        return *found;
    }

    let found = decide(kind, terrain, region_x, region_z);
    #[cfg(not(seedmap_core))]
    let mut cache = cache.lock().unwrap_or_else(|poison| poison.into_inner());

    #[cfg(not(seedmap_core))]
    if cache.len() > 65_536 {
        cache.clear();
    }

    #[cfg(not(seedmap_core))]
    cache.insert(key, found);
    found
}

fn decide(kind: Kind, terrain: &super::terrain::Terrain, region_x: i32, region_z: i32) -> Option<Start> {
    let chunk = attempt(kind, terrain.seed(), region_x, region_z);
    let facing = (roll(terrain.seed(), chunk.0, chunk.1, kind.salt() ^ 0xFACE) % 4) as u8;
    let mut start = Start { kind, chunk, facing, y: 0 };
    let [x0, z0, x1, z1] = start.area();

    if !kind.biome_fits(terrain.column_in((x0 + x1) / 2, (z0 + z1) / 2, None).biome) {
        return None;
    }

    // Рельеф вокруг рамки (кольцо в двух клетках, шаг 2): у хижины вода —
    // тоже поверхность.
    let mut heights = Vec::new();
    let ground = super::structure_terrain::Ground::new(terrain);

    for x in (x0 - 2..=x1 + 2).step_by(2) {
        for z in (z0 - 2..=z1 + 2).step_by(2) {
            if x < x0 || x > x1 || z < z0 || z > z1 {
                heights.push(ground.height(x, z).max(super::terrain::SEA));
            }
        }
    }

    let average = heights.iter().sum::<i32>() as f64 / heights.len().max(1) as f64;

    start.y = match kind {
        Kind::DesertPyramid => heights.iter().copied().min().unwrap_or(64),
        Kind::JunglePyramid | Kind::SwampHut => average.floor() as i32 + 1,
    };

    Some(start)
}

/// Постройки, чьи рамки могут задеть чанк.
fn near(terrain: &super::terrain::Terrain, chunk_x: i32, chunk_z: i32) -> Vec<Start> {
    let mut out = Vec::new();

    for kind in Kind::ALL {
        for region_x in (chunk_x - 2).div_euclid(SPACING)..=chunk_x.div_euclid(SPACING) {
            for region_z in (chunk_z - 2).div_euclid(SPACING)..=chunk_z.div_euclid(SPACING) {
                let (x, z) = attempt(kind, terrain.seed(), region_x, region_z);

                if !(x..=x + 1).contains(&chunk_x) || !(z..=z + 1).contains(&chunk_z) {
                    continue;
                }

                if let Some(start) = start_in(kind, terrain, region_x, region_z) {
                    out.push(start);
                }
            }
        }
    }

    out
}

fn solid(state: i32) -> bool {
    blocks::full_cube(state)
}

/// Состояние клетки в мире: поворот образца и выбор из набора.
fn state_of(choice: &Choice, facing: u8, dice: u64) -> Option<i32> {
    let text = match choice {
        Choice::Fixed(text) => text.as_str(),
        Choice::Random(options) => {
            let total: u32 = options.iter().map(|(_, weight)| weight).sum();
            let mut pick = (dice % u64::from(total.max(1))) as u32;

            options
                .iter()
                .find(|(_, weight)| {
                    let hit = pick < *weight;
                    pick = pick.saturating_sub(*weight);
                    hit
                })
                .map(|(text, _)| text.as_str())?
        }
    };

    blocks::state_from_text(&super::nether_fortress::orient(text, facing))
}

/// Кладёт в сложенный чанк части ближних построек — после растительности.
#[cfg(not(seedmap_core))]
pub(super) fn build(generator: &super::Generator, chunk: &mut super::Chunk, chunk_x: i32, chunk_z: i32) {
    let super::Generator::Normal(terrain) = generator else {
        return;
    };
    let (x0, z0) = (chunk_x * 16, chunk_z * 16);
    let seed = terrain.seed();

    build_igloos(terrain, chunk, chunk_x, chunk_z);

    for start in near(terrain, chunk_x, chunk_z) {
        let Some(template) = start.template() else {
            continue;
        };
        let (sandstone, log) = (blocks::state_by_name("sandstone").unwrap_or(0), blocks::state_by_name("oak_log").unwrap_or(0));

        for a in 0..template.width {
            for b in 0..template.depth {
                let (x, z) = start.world(a, b);

                if !(x0..x0 + 16).contains(&x) || !(z0..z0 + 16).contains(&z) {
                    continue;
                }

                let (lx, lz) = (x - x0, z - z0);

                for y in template.low..template.low + template.height {
                    let cell = template.cell(a, y, b);

                    if cell == 0 {
                        continue;
                    }

                    let Some(state) = template.palette.get(usize::from(cell) - 1).and_then(|choice| state_of(choice, start.facing, roll(seed, x, z, 0x7E3D_0100 ^ (y as u64)))) else {
                        continue;
                    };
                    let wy = start.y + y;

                    chunk.put_generated(lx, wy, lz, state, true);
                    if crate::archaeology::suspicious(state) {
                        chunk.archaeology.insert((lx,wy,lz),crate::archaeology::generated("desert_pyramid",roll(seed,x,z,0xA2C2 ^ wy as u64)));
                    }

                    match blocks::block_at_state(state) {
                        Some("chest") => {
                            let table = if start.kind == Kind::JunglePyramid { crate::loot::Table::JungleTemple } else { crate::loot::Table::DesertPyramid };
                            let mut dice = crate::loot::Dice::new(roll(seed, x, z, 0x7E3D_0200 ^ (wy as u64)));
                            let mut chest = crate::containers::Container::new(crate::containers::Kind::Chest);

                            chest.slots = crate::loot::fill(table.pools(), chest.slots.len(), &mut dice);
                            chunk.put_generated_container(lx, wy, lz, chest);
                        }
                        Some("dispenser") => {
                            let mut dice = crate::loot::Dice::new(roll(seed, x, z, 0x7E3D_0300 ^ (wy as u64)));
                            let mut dispenser = crate::containers::Container::new(crate::containers::Kind::Dispenser);

                            dispenser.slots = crate::loot::fill(crate::loot::Table::JungleDispenser.pools(), dispenser.slots.len(), &mut dice);
                            chunk.put_generated_container(lx, wy, lz, dispenser);
                        }
                        _ => {}
                    }
                }

                // Опоры: под нижним слоем пирамиды — песчаник, под сваями
                // хижины — брёвна, вниз до твёрдого.
                let bottom = template.cell(a, template.low, b);
                let fill = match start.kind {
                    Kind::DesertPyramid if bottom != 0 => Some(sandstone),
                    Kind::SwampHut if template.palette.get(usize::from(bottom).wrapping_sub(1)).is_some_and(|choice| matches!(choice, Choice::Fixed(text) if text.starts_with("oak_log"))) => Some(log),
                    _ => None,
                };

                if let Some(fill) = fill {
                    let mut y = start.y + template.low - 1;

                    while y > super::MIN_Y && !solid(chunk.block(lx, y, lz)) && (start.kind == Kind::SwampHut || start.y + template.low - y < 48) {
                        chunk.put_generated(lx, y, lz, fill, true);
                        y -= 1;
                    }
                }
            }
        }
    }
}

// Иглу.
//
// Где — по вики (Structure set: igloos — «spacing 32, separation 8»; Igloo:
// снежная тайга, снежные равнины, снежные склоны). Чем — замер мира
// оригинала (tools/measure/temples.py igloo, 13 иглу): образцы igloo/top,
// igloo/middle, igloo/bottom (src/world/igloo.txt) в рамке образца с
// поворотом на четверть; звено шахты — в клетке (2, 4) рамки верха, на 3
// ниже каждое следующее, подвал — в клетке (0, −2), на 6 ниже последнего
// звена (замер: те же сдвиги у всех поворотов). Пол верха — на высоте
// рельефа (замер: рельеф вокруг 62,7…118,5 — пол 63…119). Подвал — у
// половины (вики: «Half of all igloos have an oak trapdoor»), звеньев 4…11
// (замер 13 иглу: 4…10) [проверить: закон длины]; люк над шахтой в клетке
// (3, 0, 5) верха, без подвала там снег. В подвале — сундук (igloo_chest),
// варочная стойка с взрывным зельем слабости (вики), житель и зомби-житель
// (замер entities: клетки (2, 1, 1) и (4, 1, 1) подвала).

fn igloo_library() -> &'static HashMap<&'static str, Template> {
    static LIBRARY: OnceLock<HashMap<&'static str, Template>> = OnceLock::new();

    LIBRARY.get_or_init(|| parse(include_str!("igloo.txt"), &["top", "middle", "bottom"]))
}

const IGLOO_SALT: u64 = 0x7E3D_0004;

/// Иглу: чанк старта, поворот, точка образца верха (клетка a = 0, b = 0),
/// высота пола и звеньев шахты (0 — без подвала).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Igloo {
    pub chunk: (i32, i32),
    pub rot: u8,
    pub origin: (i32, i32),
    pub y: i32,
    pub shaft: i32,
}

/// Оси a и b образца в мире при повороте на `rot` четвертей по часовой.
const ROT_AXES: [((i32, i32), (i32, i32)); 4] = [((1, 0), (0, 1)), ((0, 1), (-1, 0)), ((-1, 0), (0, -1)), ((0, -1), (1, 0))];

fn rot_world(origin: (i32, i32), rot: u8, a: i32, b: i32) -> (i32, i32) {
    let ((ax, az), (bx, bz)) = ROT_AXES[usize::from(rot % 4)];

    (origin.0 + a * ax + b * bx, origin.1 + a * az + b * bz)
}

/// Состояние образца иглу в мире: направления повёрнуты на `rot`.
fn igloo_orient(text: &str, rot: u8) -> String {
    const RING: [&str; 4] = ["north", "east", "south", "west"];
    let turn = |word: &str| RING.iter().position(|known| *known == word).map(|at| RING[(at + usize::from(rot)) % 4]);
    let Some((name, rest)) = text.split_once('[') else {
        return text.to_string();
    };
    let properties: Vec<String> = rest
        .trim_end_matches(']')
        .split(',')
        .map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));

            match (turn(key), key == "facing", turn(value)) {
                (Some(key), _, _) => format!("{key}={value}"),
                (None, true, Some(value)) => format!("facing={value}"),
                _ => pair.to_string(),
            }
        })
        .collect();

    format!("{}[{}]", name, properties.join(","))
}

/// Стоит ли иглу в области.
pub fn igloo_in(terrain: &super::terrain::Terrain, region_x: i32, region_z: i32) -> Option<Igloo> {
    use super::terrain::Biome;

    let seed = terrain.seed();
    let dice = roll(seed, region_x, region_z, IGLOO_SALT);
    let span = (SPACING - SEPARATION) as u64;
    let chunk = (region_x * SPACING + (dice % span) as i32, region_z * SPACING + ((dice >> 20) % span) as i32);
    let more = roll(seed, chunk.0, chunk.1, IGLOO_SALT ^ 0xFACE);
    let rot = (more % 4) as u8;
    let top = igloo_library().get("top")?;
    let (sx, sz) = if rot.is_multiple_of(2) { (top.width, top.depth) } else { (top.depth, top.width) };
    let (x0, z0) = (chunk.0 * 16 + ((more >> 8) % 9) as i32, chunk.1 * 16 + ((more >> 16) % 9) as i32);
    let (x1, z1) = (x0 + sx - 1, z0 + sz - 1);
    let column = terrain.column_at((x0 + x1) / 2, (z0 + z1) / 2);

    if !matches!(column.biome, Biome::SnowyTaiga | Biome::SnowyPlains | Biome::SnowySlopes) {
        return None;
    }

    let origin = match rot {
        0 => (x0, z0),
        1 => (x1, z0),
        2 => (x1, z1),
        _ => (x0, z1),
    };
    let shaft = if (more >> 24).is_multiple_of(2) { 4 + ((more >> 28) % 8) as i32 } else { 0 };

    Some(Igloo { chunk, rot, origin, y: column.height, shaft })
}

/// Куски иглу: образец, точка, высота.
fn igloo_parts(igloo: &Igloo) -> Vec<(&'static str, (i32, i32), i32)> {
    let mut out = vec![("top", igloo.origin, igloo.y)];

    if igloo.shaft > 0 {
        let middle = rot_world(igloo.origin, igloo.rot, 2, 4);

        for k in 1..=igloo.shaft {
            out.push(("middle", middle, igloo.y - 3 * k));
        }

        out.push(("bottom", rot_world(igloo.origin, igloo.rot, 0, -2), igloo.y - 3 * igloo.shaft - 6));
    }

    out
}

fn igloos_near(terrain: &super::terrain::Terrain, chunk_x: i32, chunk_z: i32) -> Vec<Igloo> {
    let mut out = Vec::new();

    for region_x in (chunk_x - 2).div_euclid(SPACING)..=(chunk_x + 1).div_euclid(SPACING) {
        for region_z in (chunk_z - 2).div_euclid(SPACING)..=(chunk_z + 1).div_euclid(SPACING) {
            if let Some(igloo) = igloo_in(terrain, region_x, region_z)
                && (igloo.chunk.0 - chunk_x).abs() <= 2
                && (igloo.chunk.1 - chunk_z).abs() <= 2
            {
                out.push(igloo);
            }
        }
    }

    out
}

/// Кладёт части ближних иглу в чанк.
#[cfg(not(seedmap_core))]
fn build_igloos(terrain: &super::terrain::Terrain, chunk: &mut super::Chunk, chunk_x: i32, chunk_z: i32) {
    let (x0, z0) = (chunk_x * 16, chunk_z * 16);

    for igloo in igloos_near(terrain, chunk_x, chunk_z) {
        for (part, origin, y0) in igloo_parts(&igloo) {
            let Some(template) = igloo_library().get(part) else {
                continue;
            };

            for a in 0..template.width {
                for b in 0..template.depth {
                    let (x, z) = rot_world(origin, igloo.rot, a, b);

                    if !(x0..x0 + 16).contains(&x) || !(z0..z0 + 16).contains(&z) {
                        continue;
                    }

                    for y in 0..template.height {
                        let cell = template.cell(a, y, b);
                        // Люк над шахтой — по подвалу (вики: без подвала —
                        // снег).
                        let text = if part == "top" && (a, y, b) == (3, 0, 5) {
                            Some(if igloo.shaft > 0 { "oak_trapdoor[facing=north,half=top]" } else { "snow_block" }.to_string())
                        } else if cell == 0 {
                            None
                        } else {
                            match &template.palette[usize::from(cell) - 1] {
                                Choice::Fixed(text) => Some(text.clone()),
                                Choice::Random(options) => options.first().map(|(text, _)| text.clone()),
                            }
                        };
                        let Some(state) = text.and_then(|text| blocks::state_from_text(&igloo_orient(&text, igloo.rot))) else {
                            continue;
                        };
                        let (lx, wy, lz) = (x - x0, y0 + y, z - z0);

                        chunk.put_generated(lx, wy, lz, state, true);

                        match blocks::block_at_state(state) {
                            Some("chest") => {
                                let mut dice = crate::loot::Dice::new(roll(terrain.seed(), x, z, IGLOO_SALT ^ (wy as u64)));
                                let mut chest = crate::containers::Container::new(crate::containers::Kind::Chest);

                                chest.slots = crate::loot::fill(crate::loot::Table::Igloo.pools(), chest.slots.len(), &mut dice);
                                chunk.put_generated_container(lx, wy, lz, chest);
                            }
                            Some("brewing_stand") => {
                                let mut stand = crate::containers::Container::new(crate::containers::Kind::BrewingStand);
                                let data = crate::stackdata::Data { potion: crate::survival::potion_number("weakness"), ..Default::default() };

                                if let (Some(slot), Some(item)) = (stand.slots.get_mut(1), crate::blocks::item_named("splash_potion")) {
                                    *slot = Some(crate::inventory::Stack::new(item, 1).with_extras(data));
                                }

                                chunk.put_generated_container(lx, wy, lz, stand);
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }
}

/// Житель и зомби-житель подвалов иглу в чанке: (середина клетки, зомби ли).
pub fn igloo_dwellers(terrain: &super::terrain::Terrain, chunk_x: i32, chunk_z: i32) -> Vec<((f64, f64, f64), bool)> {
    let mut out = Vec::new();

    for igloo in igloos_near(terrain, chunk_x, chunk_z) {
        for (part, origin, y0) in igloo_parts(&igloo) {
            if part != "bottom" {
                continue;
            }

            for (a, b, zombie) in [(2, 1, false), (4, 1, true)] {
                let (x, z) = rot_world(origin, igloo.rot, a, b);

                if x.div_euclid(16) == chunk_x && z.div_euclid(16) == chunk_z {
                    out.push(((f64::from(x) + 0.5, f64::from(y0 + 1), f64::from(z) + 0.5), zombie));
                }
            }
        }
    }

    out
}

/// Ближайшее иглу (`locate structure minecraft:igloo`): чанк старта.
pub fn nearest_igloo(terrain: &super::terrain::Terrain, chunk_x: i32, chunk_z: i32, regions: i32) -> Option<(i32, i32)> {
    let (home_x, home_z) = (chunk_x.div_euclid(SPACING), chunk_z.div_euclid(SPACING));
    let distance = |(x, z): (i32, i32)| i64::from(x - chunk_x).pow(2) + i64::from(z - chunk_z).pow(2);
    let mut best: Option<(i32, i32)> = None;

    for ring in 0..=regions {
        for dx in -ring..=ring {
            for dz in -ring..=ring {
                if dx.abs() != ring && dz.abs() != ring {
                    continue;
                }

                if let Some(found) = igloo_in(terrain, home_x + dx, home_z + dz).map(|igloo| igloo.chunk)
                    && best.is_none_or(|known| distance(found) < distance(known))
                {
                    best = Some(found);
                }
            }
        }

        if let Some(found) = best
            && (distance(found) as f64).sqrt() < f64::from(ring * SPACING)
        {
            break;
        }
    }

    best
}

/// Ведьма и чёрная кошка хижин, стоящих в чанке: середины клеток (их ставит
/// такт при сложении чанка — tick.rs).
pub fn hut_dwellers(terrain: &super::terrain::Terrain, chunk_x: i32, chunk_z: i32) -> Vec<(f64, f64, f64)> {
    near(terrain, chunk_x, chunk_z)
        .into_iter()
        .filter(|start| start.kind == Kind::SwampHut)
        .map(|start| {
            let (x, z) = start.world(2, 5);

            (x, start.y + 2, z)
        })
        .filter(|&(x, _, z)| x.div_euclid(16) == chunk_x && z.div_euclid(16) == chunk_z)
        .map(|(x, y, z)| (f64::from(x) + 0.5, f64::from(y), f64::from(z) + 0.5))
        .collect()
}

/// Внутри ли рамки хижины клетка (вики, Swamp Hut: ведьмы появляются «at any
/// block within the 7×7×9 area» — от уровня под полом до двух над крышей).
pub fn in_hut(terrain: &super::terrain::Terrain, x: i32, y: i32, z: i32) -> bool {
    near(terrain, x.div_euclid(16), z.div_euclid(16)).into_iter().any(|start| {
        let [x0, z0, x1, z1] = start.area();

        start.kind == Kind::SwampHut && (x0..=x1).contains(&x) && (z0..=z1).contains(&z) && (start.y..start.y + 9).contains(&y)
    })
}

/// Ближайшая постройка вида (`locate structure`): чанк старта.
pub fn nearest(kind: Kind, terrain: &super::terrain::Terrain, chunk_x: i32, chunk_z: i32, regions: i32) -> Option<(i32, i32)> {
    let (home_x, home_z) = (chunk_x.div_euclid(SPACING), chunk_z.div_euclid(SPACING));
    let distance = |(x, z): (i32, i32)| i64::from(x - chunk_x).pow(2) + i64::from(z - chunk_z).pow(2);
    let mut best: Option<(i32, i32)> = None;

    for ring in 0..=regions {
        for dx in -ring..=ring {
            for dz in -ring..=ring {
                if dx.abs() != ring && dz.abs() != ring {
                    continue;
                }

                if let Some(found) = start_in(kind, terrain, home_x + dx, home_z + dz).map(|start| start.chunk)
                    && best.is_none_or(|known| distance(found) < distance(known))
                {
                    best = Some(found);
                }
            }
        }

        if let Some(found) = best
            && (distance(found) as f64).sqrt() < f64::from(ring * SPACING)
        {
            break;
        }
    }

    best
}

#[cfg(all(test, not(seedmap_core)))]
mod tests {
    use super::*;

    #[test]
    fn reported_swamp_hut_uses_actual_swamp_surface() {
        use super::super::{Chunk, Generator, terrain::{Style, Terrain, SEA}};
        let seed = 1_791_716_133_710_533_940;
        let terrain = Terrain::new(seed, Style::Vanilla);
        let smooth = Terrain::new(seed, Style::Smooth);
        // После настройки биомов прежняя точка перестала быть болотом.
        // Ищем тот же случай ошибки высоты, не привязываясь к биому точки.
        let (rx, rz, _) = (-32..32).flat_map(|rx| (-32..32).map(move |rz| (rx,rz)))
            .find_map(|(rx,rz)| {
                let start = start_in(Kind::SwampHut, &terrain, rx, rz)?;
                let [x0,z0,x1,z1] = start.area();
                let wrong = terrain.column_in((x0+x1)/2,(z0+z1)/2,None).height;
                let flat = (x0..=x1).all(|x| (z0..=z1).all(|z| {
                    (1..=4).contains(&(start.y+1-terrain.column_at(x,z).height.max(SEA)))
                }));
                ((63..=66).contains(&start.y) && wrong-start.y>=10 && flat).then_some((rx,rz,start))
            }).expect("болотная хижина с расхождением приближённой и настоящей высоты");
        let _ = start_in(Kind::SwampHut, &smooth, rx, rz);
        let start = start_in(Kind::SwampHut, &terrain, rx, rz).unwrap();
        let [x0, z0, x1, z1] = start.area();
        assert!((63..=66).contains(&start.y), "base {}", start.y);
        let wrong = terrain.column_in((x0 + x1) / 2, (z0 + z1) / 2, None).height;
        assert!(wrong - start.y >= 10, "old {wrong} / new {}", start.y);
        assert_eq!(decide(Kind::SwampHut, &terrain, rx, rz).unwrap().y, start.y);
        let generator = Generator::Normal(Terrain::new(seed, Style::Vanilla));
        let mut legs = 0;
        let mut floor = 0;
        for cx in x0.div_euclid(16)..=x1.div_euclid(16) {
            for cz in z0.div_euclid(16)..=z1.div_euclid(16) {
                let chunk = Chunk::generated(&generator, cx, cz);
                for x in x0.max(cx * 16)..=x1.min(cx * 16 + 15) {
                    for z in z0.max(cz * 16)..=z1.min(cz * 16 + 15) {
                        let ground = terrain.column_at(x, z).height;
                        let block = |y| blocks::block_at_state(chunk.block(x - cx * 16, y, z - cz * 16));
                        if block(start.y + 1) == Some("spruce_planks") {
                            floor += 1;
                            assert!((1..=4).contains(&(start.y + 1 - ground.max(SEA))));
                        }
                        if block(start.y) == Some("oak_log") {
                            legs += 1;
                            for y in ground + 1..=start.y { assert_eq!(block(y), Some("oak_log")); }
                            assert!(solid(chunk.block(x - cx * 16, ground, z - cz * 16)));
                        }
                    }
                }
            }
        }
        println!("hut base {}, old height {wrong}, legs {legs}, floor {floor}", start.y);
        assert_eq!(legs, 4);
        assert!(floor > 20);
    }


    /// Образцы разобраны, все блоки палитр есть в нашей таблице.
    #[test]
    fn templates_parse() {
        for kind in Kind::ALL {
            let template = library().get(kind.name()).expect("образец есть");

            assert!(template.width > 0 && template.depth > 0 && template.height > 0);

            for choice in &template.palette {
                let texts: Vec<&str> = match choice {
                    Choice::Fixed(text) => vec![text.as_str()],
                    Choice::Random(options) => options.iter().map(|(text, _)| text.as_str()).collect(),
                };

                for text in texts {
                    for facing in 0..4 {
                        assert!(state_of(&Choice::Fixed(text.to_string()), facing, 0).is_some(), "{kind:?}: {text}");
                    }
                }
            }
        }
    }

    /// Правило места: в своей области, сдвиг 0…23 (вики: 32 и 8).
    #[test]
    fn attempts_keep_the_separation() {
        for kind in Kind::ALL {
            for region_x in -8..8 {
                for region_z in -8..8 {
                    let (x, z) = attempt(kind, 42, region_x, region_z);

                    assert!((0..SPACING - SEPARATION).contains(&(x - region_x * SPACING)));
                    assert!((0..SPACING - SEPARATION).contains(&(z - region_z * SPACING)));
                }
            }
        }
    }

    /// Поворот рамки: углы рамки — углы плана при любой стороне, клетки
    /// не совпадают.
    #[test]
    fn frame_covers_the_area() {
        for facing in 0..4 {
            let start = Start { kind: Kind::JunglePyramid, chunk: (3, -2), facing, y: 64 };
            let template = start.template().expect("образец есть");
            let [x0, z0, x1, z1] = start.area();
            let mut seen = std::collections::HashSet::new();

            for a in 0..template.width {
                for b in 0..template.depth {
                    let (x, z) = start.world(a, b);

                    assert!((x0..=x1).contains(&x) && (z0..=z1).contains(&z));
                    assert!(seen.insert((x, z)));
                }
            }
        }
    }

    /// Иглу: образцы разобраны (верх 7×8×5, звено 3×3×3, подвал 7×9×6 —
    /// замер), все состояния ставятся при любом повороте; в подвале сундук,
    /// варочная стойка и шесть заражённых блоков (замер: 4,2 + 1,4 на иглу
    /// при 9 подвалах из 13).
    #[test]
    fn igloo_templates_parse() {
        let library = igloo_library();
        let size = |name: &str| library.get(name).map(|t| (t.width, t.depth, t.height));

        assert_eq!(size("top"), Some((7, 8, 5)));
        assert_eq!(size("middle"), Some((3, 3, 3)));
        assert_eq!(size("bottom"), Some((7, 9, 6)));

        for template in library.values() {
            for choice in &template.palette {
                if let Choice::Fixed(text) = choice {
                    for rot in 0..4 {
                        assert!(blocks::state_from_text(&igloo_orient(text, rot)).is_some(), "{text}");
                    }
                }
            }
        }

        let bottom = &library["bottom"];
        let count = |prefix: &str| bottom.cells.iter().filter(|cell| **cell != 0 && matches!(&bottom.palette[usize::from(**cell) - 1], Choice::Fixed(text) if text.starts_with(prefix))).count();

        assert_eq!(count("chest"), 1);
        assert_eq!(count("brewing_stand"), 1);
        assert!(count("infested_") >= 3);
        assert_eq!(igloo_orient("bed[facing=south,part=foot]", 1), "bed[facing=west,part=foot]");
    }

    /// Перепись блоков образцов — против замера оригинала (tools/measure/
    /// temples.py census, по 12 экземпляров): ожидаемое число по долям
    /// наборов в пределах разброса оригинала.
    #[test]
    fn temples_census() {
        const MEASURED: [(Kind, &str, f64, f64); 17] = [
            (Kind::DesertPyramid, "cut_sandstone", 239.8, 25.0),
            (Kind::DesertPyramid, "orange_terracotta", 65.8, 3.0),
            (Kind::DesertPyramid, "chiseled_sandstone", 62.0, 4.0),
            (Kind::DesertPyramid, "tnt", 9.0, 0.0),
            (Kind::DesertPyramid, "chest", 4.0, 0.0),
            (Kind::DesertPyramid, "blue_terracotta", 2.0, 0.0),
            (Kind::DesertPyramid, "stone_pressure_plate", 1.0, 0.0),
            (Kind::DesertPyramid, "suspicious_sand", 6.8, 3.0),
            (Kind::JunglePyramid, "mossy_cobblestone", 706.2, 60.0),
            (Kind::JunglePyramid, "cobblestone", 450.8, 60.0),
            (Kind::JunglePyramid, "sticky_piston", 3.0, 0.0),
            (Kind::JunglePyramid, "dispenser", 2.0, 0.0),
            (Kind::JunglePyramid, "chest", 2.0, 0.0),
            (Kind::JunglePyramid, "lever", 3.0, 0.0),
            (Kind::SwampHut, "spruce_planks", 84.0, 2.0),
            (Kind::SwampHut, "spruce_stairs", 26.0, 0.0),
            (Kind::SwampHut, "cauldron", 1.0, 0.0),
        ];

        for (kind, name, measured, spread) in MEASURED {
            let template = library().get(kind.name()).expect("образец есть");
            let expected: f64 = template
                .cells
                .iter()
                .filter(|cell| **cell != 0)
                .map(|cell| match &template.palette[usize::from(*cell) - 1] {
                    Choice::Fixed(text) => f64::from(u8::from(text.split('[').next() == Some(name))),
                    Choice::Random(options) => {
                        let total: u32 = options.iter().map(|(_, weight)| weight).sum();

                        options.iter().filter(|(text, _)| text.split('[').next() == Some(name)).map(|(_, weight)| f64::from(*weight)).sum::<f64>() / f64::from(total)
                    }
                })
                .sum();

            assert!((expected - measured).abs() <= spread, "{kind:?} {name}: у нас {expected:.1}, оригинал {measured}");
        }
    }

    /// `locate structure` находит каждый вид в нашем мире (семя замера
    /// оригинала), а сложенный чанк старта содержит его блоки: сундуки
    /// пирамиды, раздатчики джунглей, котёл хижины, снег иглу
    /// (`cargo test --release structures_world -- --ignored --nocapture`).
    #[test]
    #[ignore]
    fn structures_world() {
        use super::super::{terrain, Chunk, Generator};

        let generator = Generator::Normal(terrain::Terrain::new(100_554_032_945_340, terrain::Style::Vanilla));
        let Generator::Normal(terrain) = &generator else { unreachable!() };
        let marks = [(Kind::DesertPyramid, "tnt"), (Kind::JunglePyramid, "dispenser"), (Kind::SwampHut, "cauldron")];

        for (kind, mark) in marks {
            let (chunk_x, chunk_z) = nearest(kind, terrain, 0, 0, 40).unwrap_or_else(|| panic!("{kind:?} не найден"));
            let start = (-41..41).flat_map(|x| (-41..41).map(move |z| (x, z))).find_map(|(x, z)| start_in(kind, terrain, x, z).filter(|start| start.chunk == (chunk_x, chunk_z))).expect("старт есть");
            let [x0, z0, x1, z1] = start.area();
            let mut found = 0;

            for cx in x0.div_euclid(16)..=x1.div_euclid(16) {
                for cz in z0.div_euclid(16)..=z1.div_euclid(16) {
                    let chunk = Chunk::generated(&generator, cx, cz);

                    for x in 0..16 {
                        for z in 0..16 {
                            for y in start.y - 20..start.y + 20 {
                                found += usize::from(blocks::block_at_state(chunk.block(x, y, z)) == Some(mark));
                            }
                        }
                    }
                }
            }

            println!("{kind:?}: чанк {chunk_x},{chunk_z}, сторона {}, низ {}, «{mark}» {found}", start.facing, start.y);
            assert!(found > 0, "{kind:?}: нет {mark}");
        }

        let igloo = nearest_igloo(terrain, 0, 0, 60).expect("иглу найдено");
        let found = (-61..61).flat_map(|x| (-61..61).map(move |z| (x, z))).find_map(|(x, z)| igloo_in(terrain, x, z).filter(|found| found.chunk == igloo)).expect("иглу есть");
        let chunk = Chunk::generated(&generator, found.origin.0.div_euclid(16), found.origin.1.div_euclid(16));
        let snow = (0..16).flat_map(|x| (0..16).map(move |z| (x, z))).map(|(x, z)| (found.y..found.y + 5).filter(|&y| blocks::block_at_state(chunk.block(x, y, z)) == Some("snow_block")).count()).sum::<usize>();

        println!("иглу: чанк {:?}, шахта {}, снега в чанке точки {snow}", found.chunk, found.shaft);
        assert!(snow > 0);
    }
}

#[cfg(all(test, not(seedmap_core)))]
pub(super) fn audit_piece(s: &Start) -> serde_json::Value {
    let t = s.template().unwrap();
    let a = s.area();
    serde_json::json!({"name":s.kind.name(),"rotation":s.facing,"box":[a[0],s.y+t.low,a[1],a[2],s.y+t.low+t.height-1,a[3]]})
}

#[cfg(all(test, not(seedmap_core)))]
#[test]
fn desert_pyramid_uses_the_documented_set_and_only_desert() {
    assert_eq!(Kind::DesertPyramid.salt(),14_357_617);
    assert!(Kind::DesertPyramid.biome_fits(super::terrain::Biome::Desert));
    assert!(!Kind::DesertPyramid.biome_fits(super::terrain::Biome::Badlands));
    assert!(!Kind::DesertPyramid.biome_fits(super::terrain::Biome::Savanna));
}
