// Аванпосты разбойников (pillager outpost): где стоят и чем сложены.
//
// Где — по вики (Structure set «pillager_outposts»: spacing 32, separation
// 8, salt 165745296, frequency 0.2 — «Probability of 20%», exclusion zone —
// 10 чанков от деревень): мир поделён на области 32×32 чанка, в каждой одна
// попытка — чанк со сдвигом 0…23, ровно; строится с долей 20 %, если биом
// середины чанка из списка (вики, Pillager Outpost: равнины, пустыня,
// саванна, снежные равнины, тайга, луг, роща, склоны и пики, вишнёвая роща)
// и ни одна деревня не стоит ближе 10 чанков [проверить: оригинал смотрит
// на попытки деревень, у нас — на поставленные деревни].
//
// Чем — замер мира оригинала 26.1.2 (tools/measure/pillager_outpost.py
// export, 13 аванпостов, src/world/pillager_outpost.txt). Аванпост у
// оригинала — мозаика (jigsaw): плита старта с башней, вокруг площадки по
// рельефу, на них случайные куски — клетки, палатки, брёвна, мишени. Каждый
// снятый аванпост — своя раскладка (какие куски, повороты, места), аванпост
// повторяет одну из них, повёрнутую на случайную четверть вокруг угла чанка
// старта. Куски — образцы, сведённые по большинству экземпляров. Высота
// (замер): башня — на медиане земли по кольцу вокруг плиты старта (0 или
// −1), прочие куски — на блок выше медианы земли вокруг себя; под полом
// куска пустота засыпается землёй до рельефа («beard_thin») [проверить].
//
// Башня — список кусков оригинала: сама башня и поверх неё заросшая с
// порчей outpost_rot; замер: булыжника замшелого 11,7 на 195 простого —
// у нас булыжник башни с долей 5,6 % замшелый (лианы — [проверить]).
// Флаги на стенах — белые настенные: узор зловещего флага держит
// сущность-блок, которой у нас ещё нет [проверить].
//
// Сундук башни — chests/pillager_outpost (поле LootTable у экземпляров).
// В клетке feature_cage1 — железный голем (замер: 4 из 4; вики: «An iron
// golem has a 85% chance of spawning»), в клетке с аллаями — по два аллая
// (замер); число 1–3 берётся по вики. Разбойники появляются в области 72×58×72
// вокруг верхнего яруса башни (вики: «centered on the top level of the
// watchtower, the same level as the chest … extends 28 blocks upward and 30
// blocks downward») — `inside`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use super::template::{self, Cell, Template};
use crate::blocks;

/// Область попыток, зазор, соль и доля (вики, Structure set).
pub const SPACING: i32 = 32;
pub const SEPARATION: i32 = 8;
const SALT: u64 = 165_745_296;
const FREQUENCY: f64 = 0.2;

/// Наименьшее расстояние до деревни, чанков (exclusion zone).
const VILLAGE_ZONE: i32 = 10;

/// Как далеко от чанка старта доходят куски (замер: до 31 блока от угла
/// чанка и рамка куска до 7) — 3 чанка в любую сторону.
const REACH: i32 = 3;

/// Доля замшелого булыжника на башне (замер, см. выше).
const MOSS: f64 = 0.056;

/// Доля клеток с железным големом (вики).
const GOLEM: f64 = 0.85;

/// Ярус сундука над низом башни (замер: loot 9 14 10).
const TOP_LEVEL: i32 = 14;

/// Кусок раскладки: образец, поворот, dx, dy, dz, высота от плиты старта.
type Entry = (usize, u8, i32, i32, i32, bool);

struct Library {
    templates: Vec<Template>,
    layouts: Vec<Vec<Entry>>,
}

fn library() -> &'static Library {
    static LIBRARY: OnceLock<Library> = OnceLock::new();

    LIBRARY.get_or_init(|| parse(include_str!("pillager_outpost.txt")))
}

/// Раскладки (`layout <n>`, строки `  <образец> <поворот> <dx> <dy> <dz>
/// start|ring`) — до образцов; образцы читает template.rs.
fn parse(text: &str) -> Library {
    let templates = template::parse(text);
    let mut layouts: Vec<Vec<Entry>> = Vec::new();

    for line in text.lines().take_while(|line| !line.starts_with("template")) {
        let words: Vec<&str> = line.split_whitespace().collect();

        match words.as_slice() {
            ["layout", _] => layouts.push(Vec::new()),
            [name, turns, dx, dy, dz, how] if line.starts_with("  ") => {
                let (Some(layout), Some(index)) = (layouts.last_mut(), templates.iter().position(|known| known.name == *name)) else {
                    continue;
                };
                let number = |word: &str| word.parse::<i32>().unwrap_or(0);

                layout.push((index, number(turns) as u8, number(dx), number(dy), number(dz), *how == "start"));
            }
            _ => {}
        }
    }

    Library { templates, layouts }
}

/// Смещение клетки (a, b) куска с поворотом `turns` от его начала (как в
/// записи оригинала: CLOCKWISE_90 — (a, b) → (−b, a)).
fn turn(a: i32, b: i32, turns: u8) -> (i32, i32) {
    match turns % 4 {
        0 => (a, b),
        1 => (-b, a),
        2 => (-a, -b),
        _ => (b, -a),
    }
}

pub fn attempt(seed: i64, region_x: i32, region_z: i32) -> (i32, i32) {
    template::attempt(seed, region_x, region_z, SPACING, SEPARATION, SALT)
}

/// Биомы аванпоста (вики, Pillager Outpost, «Generation», Java).
pub fn fits(biome: super::terrain::Biome) -> bool {
    use super::terrain::Biome;

    matches!(
        biome,
        Biome::Plains
            | Biome::Desert
            | Biome::Savanna
            | Biome::SnowyPlains
            | Biome::Taiga
            | Biome::Meadow
            | Biome::Grove
            | Biome::SnowySlopes
            | Biome::JaggedPeaks
            | Biome::FrozenPeaks
            | Biome::StonyPeaks
            | Biome::CherryGrove
    )
}

/// Есть ли деревня ближе `VILLAGE_ZONE` чанков к чанку.
fn near_village(terrain: &super::terrain::Terrain, chunk_x: i32, chunk_z: i32) -> bool {
    let spacing = super::village::SPACING;

    for region_x in (chunk_x - VILLAGE_ZONE).div_euclid(spacing)..=(chunk_x + VILLAGE_ZONE).div_euclid(spacing) {
        for region_z in (chunk_z - VILLAGE_ZONE).div_euclid(spacing)..=(chunk_z + VILLAGE_ZONE).div_euclid(spacing) {
            let (x, z) = super::village::attempt(terrain.seed(), region_x, region_z);

            if (x - chunk_x).abs() <= VILLAGE_ZONE && (z - chunk_z).abs() <= VILLAGE_ZONE && super::village::start_in(terrain, region_x, region_z).is_some() {
                return true;
            }
        }
    }

    false
}

/// Стоит ли аванпост в этой области: чанк старта.
pub fn start_in(terrain: &super::terrain::Terrain, region_x: i32, region_z: i32) -> Option<(i32, i32)> {
    type Starts = Mutex<HashMap<(i64, bool, i32, i32), Option<(i32, i32)>>>;
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
    let lucky = template::chance(seed, region_x, 0, region_z, SALT ^ 0xF4E9) < FREQUENCY;
    let found = (lucky
        && !library().layouts.is_empty()
        && fits(terrain.column_in(chunk_x * 16 + 8, chunk_z * 16 + 8, None).biome)
        && !near_village(terrain, chunk_x, chunk_z))
    .then_some((chunk_x, chunk_z));

    #[cfg(not(seedmap_core))]
    cache.lock().unwrap_or_else(|poison| poison.into_inner()).insert(key, found);
    found
}

/// Поставленный кусок: образец, поворот, начало в мире, рамка x0, z0, x1, z1.
#[derive(Clone, Debug)]
struct Placed {
    template: usize,
    turns: u8,
    origin: (i32, i32, i32),
    bounds: [i32; 4],
}

/// Рамка куска с поворотом `turns` от начала (ax, az).
fn bounds_of(template: &Template, turns: u8, ax: i32, az: i32) -> [i32; 4] {
    let (fx, fz) = turn(template.width - 1, template.length - 1, turns);

    [ax.min(ax + fx), az.min(az + fz), ax.max(ax + fx), az.max(az + fz)]
}

/// Медиана земли по кольцу вокруг рамки (как в замере).
fn ring(bounds: [i32; 4], ground: &dyn Fn(i32, i32) -> i32) -> i32 {
    let mut heights = Vec::new();

    for x in bounds[0] - 1..=bounds[2] + 1 {
        heights.push(ground(x, bounds[1] - 1));
        heights.push(ground(x, bounds[3] + 1));
    }

    for z in bounds[1]..=bounds[3] {
        heights.push(ground(bounds[0] - 1, z));
        heights.push(ground(bounds[2] + 1, z));
    }

    heights.sort_unstable();
    heights[heights.len() / 2]
}

/// Раскладка аванпоста со стартом в чанке: снятая по случаю, повёрнутая на
/// случайную четверть вокруг угла чанка.
fn plan(seed: i64, chunk_x: i32, chunk_z: i32, ground: &dyn Fn(i32, i32) -> i32) -> Vec<Placed> {
    let library = library();
    let dice = template::roll(seed, chunk_x, chunk_z, SALT ^ 0x0B5E);
    let layout = &library.layouts[(dice % library.layouts.len() as u64) as usize];
    let spin = ((dice >> 32) % 4) as u8;
    let (x0, z0) = (chunk_x * 16, chunk_z * 16);
    let mut out = Vec::with_capacity(layout.len());

    for &(index, turns, dx, dy, dz, from_plate) in layout {
        let template = &library.templates[index];
        let (ox, oz) = turn(dx, dz, spin);
        let turns = (turns + spin) % 4;
        let (ax, az) = (x0 + ox, z0 + oz);
        let bounds = bounds_of(template, turns, ax, az);
        // Плита старта 16×16 — от угла чанка в повороте башни.
        let base = if from_plate {
            let (fx, fz) = turn(15, 15, turns);

            ring([x0.min(x0 + fx), z0.min(z0 + fz), x0.max(x0 + fx), z0.max(z0 + fz)], ground)
        } else {
            ring(bounds, ground)
        };

        out.push(Placed { template: index, turns, origin: (ax, base + dy, az), bounds });
    }

    out
}

type Cache = OnceLock<Mutex<HashMap<(i64, bool, i32, i32), Arc<Vec<Placed>>>>>;

/// Раскладка — одна на старт, хранится, пока мир жив.
fn plan_cached(terrain: &super::terrain::Terrain, chunk_x: i32, chunk_z: i32) -> Arc<Vec<Placed>> {
    static CACHE: Cache = OnceLock::new();
    #[cfg(not(seedmap_core))]
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    #[cfg(not(seedmap_core))]
    let key = (terrain.seed(), terrain.style() == super::terrain::Style::Vanilla, chunk_x, chunk_z);

    #[cfg(not(seedmap_core))]
    if let Some(found) = cache.lock().unwrap_or_else(|poison| poison.into_inner()).get(&key) {
        return found.clone();
    }

    let source = super::structure_terrain::Ground::new(terrain);
    let ground = |x: i32, z: i32| source.height(x, z);
    let pieces = Arc::new(plan(terrain.seed(), chunk_x, chunk_z, &ground));

    #[cfg(not(seedmap_core))]
    cache.lock().unwrap_or_else(|poison| poison.into_inner()).insert(key, pieces.clone());
    pieces
}

/// Старты аванпостов, чьи куски могут достать до чанка.
fn near(terrain: &super::terrain::Terrain, chunk_x: i32, chunk_z: i32) -> Vec<(i32, i32)> {
    let mut out = Vec::new();
    let reach = REACH + 1;

    for region_x in (chunk_x - reach).div_euclid(SPACING)..=(chunk_x + reach).div_euclid(SPACING) {
        for region_z in (chunk_z - reach).div_euclid(SPACING)..=(chunk_z + reach).div_euclid(SPACING) {
            let (x, z) = attempt(terrain.seed(), region_x, region_z);

            if (x - chunk_x).abs() > reach || (z - chunk_z).abs() > reach {
                continue;
            }

            if let Some(found) = start_in(terrain, region_x, region_z) {
                out.push(found);
            }
        }
    }

    out
}

fn state_of(text: &str) -> i32 {
    blocks::state_from_text(text).or_else(|| blocks::state_by_name(text.split('[').next().unwrap_or(text))).unwrap_or(super::AIR)
}

/// Булыжник башни → замшелый (порча outpost_rot заросшей башни).
fn mossy(text: &str) -> Option<String> {
    text.starts_with("cobblestone").then(|| format!("mossy_{text}"))
}

#[cfg(not(seedmap_core))]
pub(super) fn beards_near(terrain: &super::terrain::Terrain, cx: i32, cz: i32) -> Vec<super::structure_terrain::Beard> {
    near(terrain, cx, cz).into_iter().flat_map(|(sx, sz)| {
        plan_cached(terrain, sx, sz).iter().map(|piece| super::structure_terrain::Beard {
            bounds: piece.bounds, floor: piece.origin.1,
            ceiling: piece.origin.1 + library().templates[piece.template].height - 1,
        }).collect::<Vec<_>>()
    }).collect()
}

/// Кладёт в сложенный чанк куски ближних аванпостов — после деревьев и
/// растительности: куски вытесняют траву и листву.
#[cfg(not(seedmap_core))]
pub(super) fn build(generator: &super::Generator, chunk: &mut super::Chunk, chunk_x: i32, chunk_z: i32) {
    let super::Generator::Normal(terrain) = generator else {
        return;
    };
    let seed = terrain.seed();
    let (x0, z0) = (chunk_x * 16, chunk_z * 16);
    let library = library();

    for (start_x, start_z) in near(terrain, chunk_x, chunk_z) {
        let pieces = plan_cached(terrain, start_x, start_z);
        let ground = super::structure_terrain::Ground::new(terrain);
        let beards = super::structure_terrain::near(terrain, chunk_x, chunk_z);
        let heights: Vec<_> = (0..256).map(|at| super::structure_terrain::height(&ground, x0 + at % 16, z0 + at / 16, &beards)).collect();

        for piece in pieces.iter() {
            let b = piece.bounds;

            if b[0] > x0 + 15 || x0 > b[2] || b[1] > z0 + 15 || z0 > b[3] {
                continue;
            }

            let template = &library.templates[piece.template];
            let tower = template.name == "watchtower";

            for (a, y, bb, cell) in template.cells() {
                let (dx, dz) = turn(a, bb, piece.turns);
                let (x, wy, z) = (piece.origin.0 + dx, piece.origin.1 + y, piece.origin.2 + dz);

                if !(x0..x0 + 16).contains(&x) || !(z0..z0 + 16).contains(&z) {
                    continue;
                }

                let (lx, lz) = (x - x0, z - z0);
                let Cell::Block(index) = cell else {
                    // Как у деревни: воздух соседнего куска не срезает
                    // уже рассчитанную опору более высокого пола.
                    if wy > heights[(lz * 16 + lx) as usize] {
                        chunk.put_generated(lx, wy, lz, super::AIR, true);
                    }
                    continue;
                };
                let text = super::village::rotate_text(&template.texts[usize::from(index)], piece.turns);
                let text = match mossy(&text) {
                    Some(moss) if tower && template::chance(seed, x, wy, z, 0x0B5F) < MOSS => moss,
                    _ => text,
                };

                chunk.put_generated(lx, wy, lz, state_of(&text), true);

                if text.starts_with("chest") {
                    let mut dice = crate::loot::Dice::new(template::roll(seed, x, z, 0x0B60 ^ (wy as u64)));
                    let mut chest = crate::containers::Container::new(crate::containers::Kind::Chest);

                    chest.slots = crate::loot::fill(&crate::loot::PILLAGER_OUTPOST, chest.slots.len(), &mut dice);
                    chunk.put_generated_container(lx, wy, lz, chest);
                }
            }
        }
    }
}

/// Железные големы клеток feature_cage1, чья середина в этом чанке.
pub fn golems(terrain: &super::terrain::Terrain, chunk_x: i32, chunk_z: i32) -> Vec<(f64, f64, f64)> {
    let library = library();
    let mut out = Vec::new();

    for (start_x, start_z) in near(terrain, chunk_x, chunk_z) {
        for piece in plan_cached(terrain, start_x, start_z).iter() {
            if library.templates[piece.template].name != "feature_cage1" {
                continue;
            }

            let b = piece.bounds;
            let (x, z) = (f64::from(b[0] + b[2] + 1) / 2.0, f64::from(b[1] + b[3] + 1) / 2.0);
            let lucky = template::chance(terrain.seed(), b[0], piece.origin.1, b[1], 0x0B61) < GOLEM;

            if lucky && (x.floor() as i32).div_euclid(16) == chunk_x && (z.floor() as i32).div_euclid(16) == chunk_z {
                out.push((x, f64::from(piece.origin.1), z));
            }
        }
    }

    out
}

/// Вики, Allay: «one to three allays» в клетке аванпоста.
/// [проверить] распределение числа и координаты внутри замеренного шаблона.
pub fn allays(terrain: &super::terrain::Terrain, chunk_x: i32, chunk_z: i32) -> Vec<(f64,f64,f64)> {
    near(terrain,chunk_x,chunk_z).into_iter().flat_map(|(sx,sz)|allays_in(terrain.seed(),&plan_cached(terrain,sx,sz),chunk_x,chunk_z)).collect()
}
fn allays_in(seed:i64,pieces:&[Placed],chunk_x:i32,chunk_z:i32)->Vec<(f64,f64,f64)> {
    let mut out=Vec::new();
    for piece in pieces {
        if library().templates[piece.template].name!="feature_cage_with_allays" {continue;}
        let b=piece.bounds;
        let (x,z)=(f64::from(b[0]+b[2]+1)/2.0,f64::from(b[1]+b[3]+1)/2.0);
        if (x.floor() as i32).div_euclid(16)!=chunk_x || (z.floor() as i32).div_euclid(16)!=chunk_z {continue;}
        let count=1+template::roll(seed,b[0],b[1],0x0B62)%3;
        for _ in 0..count {out.push((x,f64::from(piece.origin.1)+1.0,z));}
    }
    out
}

/// Середина верхнего яруса башни аванпоста со стартом в чанке.
fn top_of(terrain: &super::terrain::Terrain, start_x: i32, start_z: i32) -> Option<(i32, i32, i32)> {
    let library = library();

    plan_cached(terrain, start_x, start_z).iter().find(|piece| library.templates[piece.template].name == "watchtower").map(|piece| {
        let b = piece.bounds;

        ((b[0] + b[2]) / 2, piece.origin.1 + TOP_LEVEL, (b[1] + b[3]) / 2)
    })
}

/// В области появления разбойников (72×58×72 у верхнего яруса башни) ли
/// клетка.
pub fn inside(terrain: &super::terrain::Terrain, x: i32, y: i32, z: i32) -> bool {
    let (chunk_x, chunk_z) = (x.div_euclid(16), z.div_euclid(16));

    near(terrain, chunk_x, chunk_z).into_iter().filter_map(|(start_x, start_z)| top_of(terrain, start_x, start_z)).any(|(cx, cy, cz)| {
        (cx - 36..cx + 36).contains(&x) && (cy - 30..cy + 28).contains(&y) && (cz - 36..cz + 36).contains(&z)
    })
}

/// Ближайший аванпост (`locate structure minecraft:pillager_outpost`).
pub fn nearest(terrain: &super::terrain::Terrain, chunk_x: i32, chunk_z: i32, rings: i32) -> Option<(i32, i32)> {
    template::nearest(chunk_x, chunk_z, SPACING, rings, &|region_x, region_z| start_in(terrain, region_x, region_z))
}

#[cfg(all(test, not(seedmap_core)))]
mod tests {
    use super::*;

    #[test]
    fn outpost_uses_original_ground_and_beard_support() {
        use super::super::{Chunk, Generator, terrain::{Style, Terrain}};
        let seed = 1_234_567_890;
        let terrain = Terrain::new(seed, Style::Vanilla);
        let (sx, sz) = nearest(&terrain, 0, 0, 12).unwrap();
        let _ = plan_cached(&Terrain::new(seed, Style::Smooth), sx, sz);
        let pieces = plan_cached(&terrain, sx, sz);
        let direct = plan(seed, sx, sz, &|x, z| terrain.column_at(x, z).height);
        assert_eq!(pieces.iter().map(|p| p.origin).collect::<Vec<_>>(), direct.iter().map(|p| p.origin).collect::<Vec<_>>());
        let generator = Generator::Normal(Terrain::new(seed, Style::Vanilla));
        let mut chunks = HashMap::new();
        let mut supported = 0;
        for piece in pieces.iter() {
            for (a, y, b, cell) in library().templates[piece.template].cells() {
                if y != 0 || !matches!(cell, Cell::Block(_)) { continue; }
                let (dx, dz) = turn(a, b, piece.turns);
                let (x, z) = (piece.origin.0 + dx, piece.origin.2 + dz);
                let (cx, cz) = (x.div_euclid(16), z.div_euclid(16));
                let chunk = chunks.entry((cx, cz)).or_insert_with(|| Chunk::generated(&generator, cx, cz));
                assert_ne!(chunk.block(x.rem_euclid(16), piece.origin.1 - 1, z.rem_euclid(16)), super::super::AIR, "outpost {x},{},{z}", piece.origin.1);
                supported += 1;
            }
        }
        assert!(supported > 50);
    }

    use crate::world::terrain::Biome;

    /// Раскладки и образцы: 13 снятых аванпостов, в каждом башня, все куски
    /// известны, у башни сундук с таблицей аванпоста.
    #[test]
    fn layouts_and_templates_parse() {
        let library = library();

        assert_eq!(library.layouts.len(), 13);

        for layout in &library.layouts {
            assert_eq!(layout.iter().filter(|entry| library.templates[entry.0].name == "watchtower").count(), 1);
            assert!(layout.iter().all(|entry| entry.5 == (library.templates[entry.0].name == "watchtower")));
        }

        let tower = library.templates.iter().find(|template| template.name == "watchtower").expect("башня есть");
        let chests: Vec<(i32, i32, i32)> = tower.cells().filter(|(_, _, _, cell)| matches!(cell, Cell::Block(i) if tower.block_name(*i) == "chest")).map(|(a, y, b, _)| (a, y, b)).collect();

        assert_eq!(chests.len(), 1);
        assert_eq!(tower.loot_at(chests[0].0, chests[0].1, chests[0].2), Some("chests/pillager_outpost"));
        assert_eq!(chests[0].1, TOP_LEVEL);

        for template in &library.templates {
            for text in &template.texts {
                assert!(blocks::state_from_text(text).is_some(), "{}: {text}", template.name);
            }
        }

        for name in ["feature_cage1", "feature_cage2", "feature_cage_with_allays", "feature_logs", "feature_targets", "feature_tent1", "feature_tent2"] {
            assert!(library.templates.iter().any(|template| template.name == name), "{name}");
        }
    }

    /// Перепись образцов против замера оригинала (pillager_outpost.py
    /// census, 13 аванпостов): башня одна на аванпост — её блоки сходятся
    /// с переписью до блока; в палатках шерсть, в мишенях резные тыквы.
    #[test]
    fn outpost_census() {
        let library = library();
        let count = |name: &str, block: &str| {
            let template = library.templates.iter().find(|template| template.name == name).expect("образец есть");

            template.cells().filter(|(_, _, _, cell)| matches!(cell, Cell::Block(i) if template.block_name(*i) == block)).count()
        };

        // Оригинал на аванпост: dark_oak_planks 347, birch_planks 290 — у
        // всех 13 одинаково (всё это башня).
        assert_eq!(count("watchtower", "birch_planks"), 290);
        assert_eq!(count("watchtower", "dark_oak_planks"), 347);
        // Булыжник: простой и замшелый вместе (195,3 + 11,7 у оригинала).
        assert!((195..=210).contains(&count("watchtower", "cobblestone")), "{}", count("watchtower", "cobblestone"));
        assert_eq!(count("watchtower", "white_wall_banner"), 8);
        assert_eq!(count("feature_targets", "carved_pumpkin"), 2);
        assert!(count("feature_tent1", "white_wool") > 0 && count("feature_tent2", "pumpkin") > 0);
        assert!(count("feature_logs", "dark_oak_log") >= 15);
    }

    /// Старты: в своей области, не ближе 8 чанков к соседней; биомы — по вики.
    #[test]
    fn attempts_and_biomes() {
        for region_x in -6..6 {
            for region_z in -6..6 {
                let (x, z) = attempt(42, region_x, region_z);

                assert!((0..SPACING - SEPARATION).contains(&(x - region_x * SPACING)));
                assert!((0..SPACING - SEPARATION).contains(&(z - region_z * SPACING)));
            }
        }

        assert!(fits(Biome::Plains) && fits(Biome::CherryGrove) && fits(Biome::Desert));
        assert!(!fits(Biome::Forest) && !fits(Biome::Ocean) && !fits(Biome::Swamp));
    }

    /// Поворот: куски при любом развороте не выходят за `REACH` чанков.
    #[test]
    fn pieces_stay_within_reach() {
        let library = library();

        for layout_index in 0..library.layouts.len() {
            for spin in 0..4u8 {
                for &(index, turns, dx, _, dz, _) in &library.layouts[layout_index] {
                    let (ox, oz) = turn(dx, dz, spin);
                    let b = bounds_of(&library.templates[index], (turns + spin) % 4, ox, oz);

                    assert!(b.iter().all(|v| v.div_euclid(16).abs() <= REACH), "раскладка {layout_index}, поворот {spin}: {b:?}");
                }
            }
        }
    }

    /// Аванпосты в нашем мире: доля областей с аванпостом, нет деревни ближе
    /// 10 чанков, область разбойников — вокруг башни, `locate` находит.
    #[test]
    fn outposts_stand_in_the_world() {
        use super::super::terrain;

        let terrain = terrain::Terrain::new(100_554_032_945_340, terrain::Style::Vanilla);
        let starts: Vec<(i32, i32)> = (-5..5).flat_map(|x| (-5..5).map(move |z| (x, z))).filter_map(|(x, z)| start_in(&terrain, x, z)).collect();

        assert!(!starts.is_empty() && starts.len() < 40, "{}", starts.len());

        for &(chunk_x, chunk_z) in &starts {
            assert!(!near_village(&terrain, chunk_x, chunk_z));
            assert!(fits(terrain.column_in(chunk_x * 16 + 8, chunk_z * 16 + 8, None).biome));

            let (x, y, z) = top_of(&terrain, chunk_x, chunk_z).expect("башня есть");

            assert!(inside(&terrain, x, y, z) && inside(&terrain, x + 35, y - 30, z - 36));
            assert!(!inside(&terrain, x + 36, y, z) && !inside(&terrain, x, y + 28, z));
        }

        let (chunk_x, chunk_z) = starts[0];

        assert_eq!(nearest(&terrain, chunk_x, chunk_z, 2), Some((chunk_x, chunk_z)));
    }

    /// Перепись сложенных аванпостов нашего мира против замера оригинала
    /// (pillager_outpost.py census, на аванпост: birch_planks 290,
    /// dark_oak_planks 347, булыжник простой и замшелый 207, флагов 8,
    /// сундук 1): `cargo test --release outpost_world -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn outpost_world() {
        use super::super::{terrain, Chunk, Generator};

        let generator = Generator::Normal(terrain::Terrain::new(100_554_032_945_340, terrain::Style::Vanilla));
        let Generator::Normal(terrain) = &generator else { unreachable!() };
        let starts: Vec<(i32, i32)> = (-5..5).flat_map(|x| (-5..5).map(move |z| (x, z))).filter_map(|(x, z)| start_in(terrain, x, z)).take(4).collect();
        let names = ["birch_planks", "dark_oak_planks", "cobblestone", "mossy_cobblestone", "white_wall_banner", "chest", "white_wool", "carved_pumpkin"];

        for (start_x, start_z) in starts {
            let mut counts = [0u32; 8];
            // Только около аванпоста: подземелья под ним — свои сундуки и
            // замшелый булыжник.
            let (_, top, _) = top_of(terrain, start_x, start_z).expect("башня есть");

            for chunk_x in start_x - REACH..=start_x + REACH {
                for chunk_z in start_z - REACH..=start_z + REACH {
                    let chunk = Chunk::generated(&generator, chunk_x, chunk_z);

                    for x in 0..16 {
                        for z in 0..16 {
                            for y in top - 30..top + 28 {
                                let name = blocks::block_at_state(chunk.block(x, y, z)).unwrap_or("air");

                                if let Some(at) = names.iter().position(|known| *known == name) {
                                    counts[at] += 1;
                                }
                            }
                        }
                    }
                }
            }

            println!("аванпост {start_x},{start_z}: {}", names.iter().zip(counts).map(|(name, count)| format!("{name}={count}")).collect::<Vec<_>>().join(" "));
            assert_eq!(counts[0], 290);
            assert_eq!(counts[4], 8);
            assert!(counts[5] >= 1);
            assert!((195..=225).contains(&(counts[2] + counts[3])), "булыжник {}", counts[2] + counts[3]);
        }
    }
    #[test]
    fn each_generated_allay_cage_has_one_to_three_captives_in_its_chunk() {
        let seed=100_554_032_945_340;
        let mut cages=0;
        for sx in -32..32 {
            let pieces=plan(seed,sx,0,&|_,_|64);
            for p in &pieces {
                if library().templates[p.template].name!="feature_cage_with_allays" {continue;}
                let b=p.bounds;let x=f64::from(b[0]+b[2]+1)/2.0;let z=f64::from(b[1]+b[3]+1)/2.0;
                let cx=(x.floor() as i32).div_euclid(16);let cz=(z.floor() as i32).div_euclid(16);
                let all=allays_in(seed,&pieces,cx,cz);
                let count=all.iter().filter(|at|at.0==x && at.2==z && at.1==f64::from(p.origin.1)+1.0).count();
                assert!((1..=3).contains(&count));cages+=1;
                assert!(!allays_in(seed,&pieces,cx+1,cz).iter().any(|at|at.0==x && at.2==z));
            }
        }
        assert!(cages>0);
    }

}

#[cfg(all(test, not(seedmap_core)))]
pub(super) fn audit_pieces(t: &super::terrain::Terrain, cx: i32, cz: i32) -> Vec<serde_json::Value> {
    let ground = super::structure_terrain::Ground::new(t);
    plan(t.seed(),cx,cz,&|x,z| ground.supported_height(x,z)).iter().map(|p| {
        let t = &library().templates[p.template];
        serde_json::json!({"name":t.name,"rotation":p.turns,"box":[p.bounds[0],p.origin.1,p.bounds[1],p.bounds[2],p.origin.1+t.height-1,p.bounds[3]]})
    }).collect()
}
