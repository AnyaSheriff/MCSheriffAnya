// Испытательные камеры (trial chambers): где стоят и чем сложены.
//
// Где — по вики (Trial Chambers, «Generation»: «a grid of 34×34 chunk
// regions»; structure set: spacing 34, separation 12, linear): в каждой
// области одна попытка — чанк со сдвигом 0…21 по каждой оси, ровно
// («linear»). Камеры нет, если в начале — глубокая тьма («the origin point
// cannot generate within a deep dark biome»). Стартовый кусок — на высоте
// −40…−20 (вики: «between Y=-40 and -20»; замер: низ стартового куска −23,
// −28, −35, −28, −41, −32) [проверить: −41 у замера на блок ниже вики].
// Случай — свой (своя генерация), закон — оригинала.
//
// Чем — замер мира оригинала 26.1.2 (tools/measure/trial_chambers.py): сервер
// оригинала только складывал мир, а блоки сняты с region-файлов в рамках
// кусков камеры (запись о строении: 213…301 кусок, вместе рамка до
// 125×70×121). Камера у оригинала собирается из кусков случайно (jigsaw),
// поэтому каждая снятая камера — свой образец (src/world/trial_chambers.txt),
// камера повторяет одну из них, повёрнутую на случайную четверть вокруг угла
// чанка старта. В образце три вида клеток: порода как была (вне кусков или
// пустота образца куска), воздух (комнаты вырезаны в породе) и блок камеры.
//
// Сущности-блоки (замер block_entities): у испытательного спавнера — две
// настройки, обычная и зловещая (trial_chamber/breeze/normal|…/ominous), у
// хранилища — таблица добычи (без своей в записи — chests/trial_chambers/
// reward, вики: «Defaults to»), у сундуков, бочек, раздатчиков и горшков —
// таблица LootTable. Их отдаёт `block_entities` — для блоков-сущностей
// спавнера и хранилища.
//
// Перепись замера (6 камер, семя 100554032945340), среднее на камеру:
// tuff_bricks 45196, воздух 35497, polished_tuff 5118, waxed_copper_block 4807,
// waxed_oxidized_copper 3559, chiseled_tuff_bricks 2873, trial_spawner 20,
// vault 27,5 (из них зловещих 7,2).
//
// Раскладка зависит только от семени и области, поэтому каждый чанк сам
// находит ближние камеры и кладёт только то, что в него попало.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::blocks;

/// Область попыток и наименьший зазор (вики: 34 и 12 чанков).
pub const SPACING: i32 = 34;
pub const SEPARATION: i32 = 12;

/// Высота низа стартового куска (вики: −40…−20).
const START_LOW: i32 = -40;
const START_HIGH: i32 = -20;

/// Как далеко от чанка старта доходит рамка (замер: сдвиг рамки до −81,
/// ширина до 125 — меньше 7 чанков в любую сторону при любом повороте).
const REACH: i32 = 7;

/// Клетки образца: порода как была, воздух, дальше — палитра.
const KEEP: u16 = 0;
const HOLLOW: u16 = 1;

struct Template {
    width: i32,
    depth: i32,
    height: i32,
    /// Низ рамки и низ стартового куска у замера (y).
    bottom: i32,
    start_bottom: i32,
    /// Угол рамки от угла чанка старта (блоки).
    offset: (i32, i32),
    /// Палитра: текст состояния по номеру клетки (с 2).
    names: Vec<&'static str>,
    /// Состояние по номеру клетки для каждого поворота.
    states: [Vec<i32>; 4],
    cells: Vec<u16>,
    /// Сущности-блоки: от угла рамки, вид и данные.
    entities: Vec<(i32, i32, i32, &'static str, &'static str)>,
}

impl Template {
    fn cell(&self, a: i32, y: i32, b: i32) -> u16 {
        self.cells[((y * self.depth + b) * self.width + a) as usize]
    }
}

fn parse(text: &'static str) -> Vec<Template> {
    let mut out: Vec<Template> = Vec::new();
    let mut symbols: HashMap<&'static str, u16> = HashMap::new();
    let mut layer = 0;
    let mut row = 0;

    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }

        let words: Vec<&'static str> = line.split_whitespace().collect();

        if words[0] == "chamber" {
            let number = |at: usize| words.get(at).and_then(|word| word.parse::<i32>().ok()).unwrap_or(0);
            let (width, depth, height) = (number(1), number(2), number(3));

            out.push(Template {
                width,
                depth,
                height,
                bottom: number(4),
                start_bottom: number(7),
                offset: (number(5), number(6)),
                names: vec!["", "air"],
                states: Default::default(),
                cells: vec![KEEP; (width * depth * height).max(0) as usize],
                entities: Vec::new(),
            });
            symbols.clear();
            continue;
        }

        let Some(template) = out.last_mut() else {
            continue;
        };

        match words.as_slice() {
            ["layer", y] => {
                layer = y.parse().unwrap_or(0);
                row = 0;
            }
            ["e", a, y, b, kind, data] => template.entities.push((a.parse().unwrap_or(0), y.parse().unwrap_or(0), b.parse().unwrap_or(0), kind, data)),
            [symbol, name] => {
                symbols.insert(symbol, template.names.len() as u16);
                template.names.push(name);
            }
            [cells] => {
                let base = ((layer * template.depth + row) * template.width) as usize;
                let mut a = 0usize;
                let mut chars = cells.char_indices().peekable();

                while let Some((at, first)) = chars.next() {
                    let value = match first {
                        '.' => KEEP,
                        '#' => HOLLOW,
                        '~' => {
                            chars.next();
                            chars.next();
                            symbols.get(&cells[at..at + 3]).copied().unwrap_or(KEEP)
                        }
                        _ => symbols.get(&cells[at..at + first.len_utf8()]).copied().unwrap_or(KEEP),
                    };
                    let mut count = 0usize;

                    while let Some(&(_, digit)) = chars.peek() {
                        let Some(digit) = digit.to_digit(10) else { break };

                        count = count * 10 + digit as usize;
                        chars.next();
                    }

                    for _ in 0..count.max(1) {
                        if a < template.width as usize {
                            template.cells[base + a] = value;
                        }

                        a += 1;
                    }
                }

                row += 1;
            }
            _ => {}
        }
    }

    for template in &mut out {
        for turns in 0..4u8 {
            template.states[usize::from(turns)] = template
                .names
                .iter()
                .enumerate()
                .map(|(index, name)| match index {
                    0 | 1 => super::AIR,
                    _ => blocks::state_from_text(&super::village::rotate_text(name, turns))
                        .or_else(|| blocks::state_by_name(name.split('[').next().unwrap_or(name)))
                        .unwrap_or(super::AIR),
                })
                .collect();
        }
    }

    out
}

fn library() -> &'static [Template] {
    static LIBRARY: OnceLock<Vec<Template>> = OnceLock::new();

    LIBRARY.get_or_init(|| parse(include_str!("trial_chambers.txt")))
}

fn scramble(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn roll(seed: i64, x: i32, z: i32, salt: u64) -> u64 {
    scramble((seed as u64) ^ (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (z as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ salt)
}

/// Чанк попытки в области: ровный сдвиг 0…21.
pub fn attempt(seed: i64, region_x: i32, region_z: i32) -> (i32, i32) {
    let dice = roll(seed, region_x, region_z, 0x0007_81A1);
    let span = (SPACING - SEPARATION) as u64;

    (region_x * SPACING + (dice % span) as i32, region_z * SPACING + ((dice >> 32) % span) as i32)
}

/// Какая из снятых камер, на сколько четвертей повёрнута и низ стартового куска.
fn pick(seed: i64, chunk_x: i32, chunk_z: i32) -> (usize, u8, i32) {
    let dice = roll(seed, chunk_x, chunk_z, 0x0007_A1C4);
    let index = (dice % library().len().max(1) as u64) as usize;
    let turns = ((dice >> 16) % 4) as u8;
    let start = START_LOW + ((dice >> 24) % (START_HIGH - START_LOW + 1) as u64) as i32;

    (index, turns, start)
}

/// Стоит ли камера в этой области: чанк старта.
pub fn start_in(terrain: &super::terrain::Terrain, region_x: i32, region_z: i32) -> Option<(i32, i32)> {
    type Starts = Mutex<HashMap<(i64, i32, i32), Option<(i32, i32)>>>;
    static CACHE: OnceLock<Starts> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = (terrain.seed(), region_x, region_z);

    if let Some(found) = cache.lock().unwrap_or_else(|poison| poison.into_inner()).get(&key) {
        return *found;
    }

    let (chunk_x, chunk_z) = attempt(terrain.seed(), region_x, region_z);
    let (_, _, start_y) = pick(terrain.seed(), chunk_x, chunk_z);
    let column = terrain.column_in(chunk_x * 16 + 8, chunk_z * 16 + 8, None);
    // Глубокая тьма в начале — камеры нет (вики: «except the deep dark»).
    let dark = super::cave_biomes::biome_at(&column, start_y).is_some_and(|biome| biome == super::terrain::Biome::DeepDark);
    let found = (!dark && !library().is_empty()).then_some((chunk_x, chunk_z));

    cache.lock().unwrap_or_else(|poison| poison.into_inner()).insert(key, found);
    found
}

/// Клетка (dx, dz) от угла чанка старта, повёрнутая на четверть по часовой
/// вокруг этого угла.
fn turn_once((dx, dz): (i32, i32)) -> (i32, i32) {
    (-dz - 1, dx)
}

/// Обратное к `turn_once`.
fn unturn_once((dx, dz): (i32, i32)) -> (i32, i32) {
    (dz, -dx - 1)
}

fn turn(mut at: (i32, i32), turns: u8) -> (i32, i32) {
    for _ in 0..turns % 4 {
        at = turn_once(at);
    }

    at
}

fn unturn(mut at: (i32, i32), turns: u8) -> (i32, i32) {
    for _ in 0..turns % 4 {
        at = unturn_once(at);
    }

    at
}

/// Поставленная камера: образец, поворот, чанк старта и сдвиг по высоте.
struct Placed {
    template: &'static Template,
    turns: u8,
    corner: (i32, i32),
    lift: i32,
}

impl Placed {
    fn new(seed: i64, start_x: i32, start_z: i32) -> Option<Placed> {
        let (index, turns, start_y) = pick(seed, start_x, start_z);
        let template = library().get(index)?;

        Some(Placed { template, turns, corner: (start_x * 16, start_z * 16), lift: start_y - template.start_bottom })
    }

    /// Клетка образца (a, b) → мир (x, z).
    fn to_world(&self, a: i32, b: i32) -> (i32, i32) {
        let (dx, dz) = turn((a + self.template.offset.0, b + self.template.offset.1), self.turns);

        (self.corner.0 + dx, self.corner.1 + dz)
    }

    /// Мир (x, z) → клетка образца (a, b), если она в рамке.
    fn to_local(&self, x: i32, z: i32) -> Option<(i32, i32)> {
        let (dx, dz) = unturn((x - self.corner.0, z - self.corner.1), self.turns);
        let (a, b) = (dx - self.template.offset.0, dz - self.template.offset.1);

        ((0..self.template.width).contains(&a) && (0..self.template.depth).contains(&b)).then_some((a, b))
    }

    fn bottom(&self) -> i32 {
        self.template.bottom + self.lift
    }
}

/// Старты камер, чьи рамки могут задеть этот чанк.
fn near(terrain: &super::terrain::Terrain, chunk_x: i32, chunk_z: i32) -> Vec<(i32, i32)> {
    let mut out = Vec::new();

    for region_x in (chunk_x - REACH).div_euclid(SPACING)..=(chunk_x + REACH).div_euclid(SPACING) {
        for region_z in (chunk_z - REACH).div_euclid(SPACING)..=(chunk_z + REACH).div_euclid(SPACING) {
            let (x, z) = attempt(terrain.seed(), region_x, region_z);

            if (x - chunk_x).abs() > REACH || (z - chunk_z).abs() > REACH {
                continue;
            }

            if let Some(found) = start_in(terrain, region_x, region_z) {
                out.push(found);
            }
        }
    }

    out
}

/// Кладёт в сложенный чанк части ближних камер — после пещер, руды и
/// подземных украшений: в кусках порода уступает комнатам.
pub(super) fn build(generator: &super::Generator, chunk: &mut super::Chunk, chunk_x: i32, chunk_z: i32) {
    let super::Generator::Normal(terrain) = generator else {
        return;
    };

    for (start_x, start_z) in near(terrain, chunk_x, chunk_z) {
        if let Some(placed) = Placed::new(terrain.seed(), start_x, start_z) {
            paint(&placed, chunk_x, chunk_z, chunk);
        }
    }
}

/// Блоки камеры в чанке (chunk_x, chunk_z).
fn paint(placed: &Placed, chunk_x: i32, chunk_z: i32, chunk: &mut super::Chunk) {
    let template = placed.template;
    let states = &template.states[usize::from(placed.turns % 4)];
    let bottom = placed.bottom();

    for local_x in 0..16 {
        for local_z in 0..16 {
            let Some((a, b)) = placed.to_local(chunk_x * 16 + local_x, chunk_z * 16 + local_z) else {
                continue;
            };

            for y in 0..template.height {
                let cell = template.cell(a, y, b);

                if cell != KEEP {
                    chunk.put_generated(local_x, bottom + y, local_z, states[usize::from(cell)], true);
                }
            }
        }
    }
}

/// Сущности-блоки камер в чанке: мир x, y, z, вид («spawner», «vault»,
/// «chest», «barrel», «dispenser», «decorated_pot») и данные (у спавнера —
/// «обычная|зловещая» настройка, у прочих — таблица добычи, без «minecraft:»).
pub fn block_entities(terrain: &super::terrain::Terrain, chunk_x: i32, chunk_z: i32) -> Vec<(i32, i32, i32, &'static str, &'static str)> {
    let mut out = Vec::new();

    for (start_x, start_z) in near(terrain, chunk_x, chunk_z) {
        let Some(placed) = Placed::new(terrain.seed(), start_x, start_z) else {
            continue;
        };

        for &(a, y, b, kind, data) in &placed.template.entities {
            let (x, z) = placed.to_world(a, b);

            if x.div_euclid(16) == chunk_x && z.div_euclid(16) == chunk_z {
                out.push((x, placed.bottom() + y, z, kind, data));
            }
        }
    }

    out
}

/// Ближайшая камера (`locate structure minecraft:trial_chambers`): чанк старта.
pub fn nearest(terrain: &super::terrain::Terrain, chunk_x: i32, chunk_z: i32, regions: i32) -> Option<(i32, i32)> {
    let (home_x, home_z) = (chunk_x.div_euclid(SPACING), chunk_z.div_euclid(SPACING));
    let distance = |(x, z): (i32, i32)| i64::from(x - chunk_x).pow(2) + i64::from(z - chunk_z).pow(2);
    let mut best: Option<(i32, i32)> = None;

    for ring in 0..=regions {
        for dx in -ring..=ring {
            for dz in -ring..=ring {
                if dx.abs() != ring && dz.abs() != ring {
                    continue;
                }

                if let Some(found) = start_in(terrain, home_x + dx, home_z + dz)
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

#[cfg(test)]
mod tests {
    use super::*;

    fn count(template: &Template, name: &str) -> usize {
        let wanted: Vec<u16> = template.names.iter().enumerate().filter(|(_, known)| known.split('[').next() == Some(name)).map(|(index, _)| index as u16).collect();

        template.cells.iter().filter(|cell| wanted.contains(cell)).count()
    }

    /// Образцы разобраны: шесть камер, у каждой спавнеры и хранилища там же,
    /// где их блоки, все состояния палитры известны.
    #[test]
    fn templates_parse() {
        let library = library();

        assert_eq!(library.len(), 6);

        for template in library {
            assert!(template.cells.len() > 300_000);

            for (index, name) in template.names.iter().enumerate().skip(2) {
                assert_ne!(template.states[0][index], super::super::AIR, "{name}");
                assert!(blocks::state_from_text(name).is_some(), "{name}");
            }

            for &(a, y, b, kind, data) in &template.entities {
                let cell = template.cell(a, y, b);
                let block = template.names[usize::from(cell)].split('[').next().unwrap_or("");
                let expected = if kind == "spawner" { "trial_spawner" } else { kind };

                assert_eq!(block, expected, "{kind} {data} в {a},{y},{b}");
            }

            let spawners = template.entities.iter().filter(|entity| entity.3 == "spawner").count();

            assert_eq!(spawners, count(template, "trial_spawner"));
            assert!(template.entities.iter().any(|entity| entity.3 == "vault"));
        }
    }

    /// Поворот: `unturn` обратен `turn`, четыре четверти — на место.
    #[test]
    fn rotation_round_trip() {
        for turns in 0..4 {
            for at in [(0, 0), (-81, 40), (57, -3), (-1, -1)] {
                assert_eq!(unturn(turn(at, turns), turns), at);
            }
        }

        assert_eq!(turn((5, 7), 4), (5, 7));
        assert_eq!(turn((0, -1), 1), (0, 0));
    }

    /// Старты: в своей области, не ближе 12 чанков к соседней; раскладка от
    /// семени одна и та же.
    #[test]
    fn attempts_keep_the_separation() {
        for region_x in -6..6 {
            for region_z in -6..6 {
                let (x, z) = attempt(42, region_x, region_z);

                assert!((0..SPACING - SEPARATION).contains(&(x - region_x * SPACING)));
                assert!((0..SPACING - SEPARATION).contains(&(z - region_z * SPACING)));
                assert_eq!(attempt(42, region_x, region_z), (x, z));
                assert_eq!(pick(42, x, z), pick(42, x, z));

                let (_, _, start) = pick(42, x, z);

                assert!((START_LOW..=START_HIGH).contains(&start));
            }
        }
    }

    /// Рамка при любом повороте не уходит дальше `REACH` чанков от старта.
    #[test]
    fn frames_stay_within_reach() {
        for (index, template) in library().iter().enumerate() {
            for turns in 0..4 {
                let placed = Placed { template, turns, corner: (0, 0), lift: 0 };

                for (a, b) in [(0, 0), (template.width - 1, 0), (0, template.depth - 1), (template.width - 1, template.depth - 1)] {
                    let (x, z) = placed.to_world(a, b);

                    assert!(x.div_euclid(16).abs() <= REACH && z.div_euclid(16).abs() <= REACH, "образец {index}, поворот {turns}");
                    assert_eq!(placed.to_local(x, z), Some((a, b)));
                }
            }
        }
    }

    /// Перепись блоков камеры — против замера оригинала (tools/measure/
    /// trial_chambers.py census, 6 камер, семя 100554032945340): среднее на
    /// камеру по нашим образцам в пределах разброса оригинала.
    #[test]
    fn trial_chambers_census() {
        const MEASURED: [(&str, f64, f64); 8] = [
            // имя, среднее оригинала, допуск (разброс между камерами)
            ("tuff_bricks", 45196.0, 8600.0),
            ("polished_tuff", 5117.8, 720.0),
            ("waxed_copper_block", 4807.3, 1000.0),
            ("waxed_oxidized_copper", 3558.5, 780.0),
            ("chiseled_tuff_bricks", 2872.8, 710.0),
            ("waxed_copper_bulb", 179.7, 45.0),
            ("trial_spawner", 20.0, 8.0),
            ("vault", 27.5, 12.0),
        ];
        let library = library();

        for (name, measured, spread) in MEASURED {
            let ours: f64 = library.iter().map(|template| count(template, name) as f64).sum::<f64>() / library.len() as f64;

            assert!((ours - measured).abs() <= spread, "{name}: у нас {ours}, оригинал {measured}");
        }
    }

    /// Камеры в нашем мире (семя замера оригинала, 16×16 областей вокруг
    /// нуля): сколько стоит, и перепись сложенной камеры — против замера
    /// оригинала (`cargo test --release trial_chambers_world -- --ignored
    /// --nocapture`).
    #[test]
    #[ignore]
    fn trial_chambers_world() {
        use super::super::{terrain, Chunk, Generator};

        let generator = Generator::Normal(terrain::Terrain::new(100_554_032_945_340, terrain::Style::Vanilla));
        let Generator::Normal(terrain) = &generator else { unreachable!() };
        let starts: Vec<(i32, i32)> = (-8..8).flat_map(|x| (-8..8).map(move |z| (x, z))).filter_map(|(x, z)| start_in(terrain, x, z)).collect();

        println!("областей 256, камер {}", starts.len());
        assert!(starts.len() > 200);

        let (start_x, start_z) = starts[0];
        let placed = Placed::new(terrain.seed(), start_x, start_z).expect("образец есть");
        let names = ["tuff_bricks", "polished_tuff", "waxed_copper_block", "waxed_oxidized_copper", "chiseled_tuff_bricks", "trial_spawner", "vault", "air"];
        let mut counts = [0u32; 8];
        let bottom = placed.bottom();
        let mut entities = 0;

        for chunk_x in start_x - REACH..=start_x + REACH {
            for chunk_z in start_z - REACH..=start_z + REACH {
                let chunk = Chunk::generated(&generator, chunk_x, chunk_z);

                entities += block_entities(terrain, chunk_x, chunk_z).len();

                for x in 0..16 {
                    for z in 0..16 {
                        let Some((a, b)) = placed.to_local(chunk_x * 16 + x, chunk_z * 16 + z) else {
                            continue;
                        };

                        for y in 0..placed.template.height {
                            if placed.template.cell(a, y, b) == KEEP {
                                continue;
                            }

                            let name = blocks::block_at_state(chunk.block(x, bottom + y, z)).unwrap_or("air");

                            if let Some(at) = names.iter().position(|known| *known == name) {
                                counts[at] += 1;
                            }
                        }
                    }
                }
            }
        }

        println!("камера {start_x},{start_z}, поворот {}: {}", placed.turns, names.iter().zip(counts).map(|(name, count)| format!("{name}={count}")).collect::<Vec<_>>().join(" "));
        assert_eq!(counts[5] as usize, count(placed.template, "trial_spawner"));
        assert_eq!(entities, placed.template.entities.len());
    }
}
