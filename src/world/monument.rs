// Подводные крепости (ocean monument): где стоят и чем сложены.
//
// Где — по вики (Ocean Monument; Structure set: «spacing 32, separation 5,
// triangular»): мир поделён на области 32×32 чанка, в каждой одна попытка —
// чанк со сдвигом 0…26 по каждой оси, сумма двух случайных пополам (потому
// «треугольный», к середине области чаще). Крепость встаёт, если середина —
// глубокий океан в квадрате 16 блоков и вода (океан или река) в квадрате 29
// блоков (вики, «Generation»). Случай — свой (своя генерация), закон — оригинала.
//
// Чем — замер мира оригинала 26.1.2 (tools/measure/monument.py): сервер
// оригинала только складывал мир, а блоки сняты с region-файлов в рамках
// крепостей (запись о строении: кусок omb, рамка 58×23×58 от y 39). Комнаты
// внутри у оригинала случайны, поэтому каждая снятая крепость — свой образец
// (src/world/monument.txt), крепость повторяет одну из них, повёрнутую на
// случайную четверть. Перепись замера (10 крепостей):
//
// - внутри рамки рельефа нет — только вода и сама крепость (и то, что
//   кладётся позже: ламинария, водоросли, лёд айсбергов);
// - под каждой prismarine_bricks нижнего слоя — опора из неё же вниз до
//   первого твёрдого блока (1929 столбцов на 10 крепостей с опорой; кончаются
//   на песке, гравии, камне);
// - в каждой три древних стража — в верхней комнате и в двух крыльях (замер
//   entities/*.mca: места те же относительно крепости; ставит их такт мобов
//   при сложении чанка — tick.rs).
//
// Раскладка зависит только от семени и области, поэтому каждый чанк сам
// находит ближние крепости и кладёт только то, что в него попало.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::blocks;

/// Область попыток и наименьший зазор (вики: 32 и 5 чанков).
pub const SPACING: i32 = 32;
pub const SEPARATION: i32 = 5;

/// Сторона рамки и её высота (замер: 58×23×58, низ на y 39).
pub const SIZE: i32 = 58;
pub const HEIGHT: i32 = 23;
pub const BOTTOM: i32 = 39;

/// Рамка от чанка старта: от x·16 − 29 до x·16 + 28 (замер: старт (79, 307) —
/// рамка 1235…1292 × 4883…4940).
const BACK: i32 = 29;

/// Клетка образца: вода или блок палитры.
const WATER: u8 = 0;

struct Template {
    /// Палитра: номер в клетке → состояние (0 — вода).
    states: Vec<i32>,
    cells: Vec<u8>,
    /// Где стоят древние стражи (от угла рамки, в блоках).
    elders: Vec<(f64, f64, f64)>,
}

impl Template {
    fn cell(&self, a: i32, y: i32, b: i32) -> u8 {
        self.cells[((y * SIZE + b) * SIZE + a) as usize]
    }
}

fn parse(text: &str) -> Vec<Template> {
    let water = blocks::state_by_name("water").unwrap_or(0);
    let bricks = blocks::state_by_name("prismarine_bricks").unwrap_or(0);
    let mut out: Vec<Template> = Vec::new();
    let mut symbols: HashMap<char, u8> = HashMap::new();
    let mut layer = 0;
    let mut row = 0;

    for line in text.lines() {
        if line.starts_with('#') || line.trim().is_empty() {
            continue;
        }

        let words: Vec<&str> = line.split_whitespace().collect();

        if words[0] == "monument" {
            out.push(Template { states: vec![water, bricks], cells: vec![WATER; (SIZE * SIZE * HEIGHT) as usize], elders: Vec::new() });
            symbols = HashMap::from([('~', WATER), ('#', 1)]);
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
            ["elder", a, y, b] => template.elders.push((a.parse().unwrap_or(0.0), y.parse().unwrap_or(0.0), b.parse().unwrap_or(0.0))),
            [symbol, name] if symbol.chars().count() == 1 => {
                symbols.insert(symbol.chars().next().unwrap_or('?'), template.states.len() as u8);
                template.states.push(blocks::state_by_name(name).unwrap_or(water));
            }
            [cells] => {
                for (a, symbol) in cells.chars().enumerate().take(SIZE as usize) {
                    template.cells[((layer * SIZE + row) * SIZE + a as i32) as usize] = symbols.get(&symbol).copied().unwrap_or(WATER);
                }

                row += 1;
            }
            _ => {}
        }
    }

    out
}

fn library() -> &'static [Template] {
    static LIBRARY: OnceLock<Vec<Template>> = OnceLock::new();

    LIBRARY.get_or_init(|| parse(include_str!("monument.txt")))
}

fn scramble(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn roll(seed: i64, x: i32, z: i32, salt: u64) -> u64 {
    scramble((seed as u64) ^ (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (z as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ salt)
}

/// Чанк попытки в области: «треугольный» сдвиг — полусумма двух 0…26.
pub fn attempt(seed: i64, region_x: i32, region_z: i32) -> (i32, i32) {
    let dice = roll(seed, region_x, region_z, 10_387_313);
    let span = (SPACING - SEPARATION) as u64;
    let part = |shift: u32| (dice >> shift) % span;

    (region_x * SPACING + ((part(0) + part(12)) / 2) as i32, region_z * SPACING + ((part(24) + part(36)) / 2) as i32)
}

fn deep(biome: super::terrain::Biome) -> bool {
    use super::terrain::Biome;

    matches!(biome, Biome::DeepOcean | Biome::DeepColdOcean | Biome::DeepFrozenOcean | Biome::DeepLukewarmOcean)
}

fn watery(biome: super::terrain::Biome) -> bool {
    use super::terrain::Biome;

    deep(biome) || matches!(biome, Biome::Ocean | Biome::ColdOcean | Biome::FrozenOcean | Biome::LukewarmOcean | Biome::WarmOcean | Biome::River | Biome::FrozenRiver)
}

/// Стоит ли крепость в этой области: чанк старта.
pub fn start_in(terrain: &super::terrain::Terrain, region_x: i32, region_z: i32) -> Option<(i32, i32)> {
    type Starts = Mutex<HashMap<(i64, u8, i32, i32), Option<(i32, i32)>>>;
    static CACHE: OnceLock<Starts> = OnceLock::new();
    #[cfg(not(seedmap_core))]
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    #[cfg(not(seedmap_core))]
    let key = (terrain.seed(), terrain.generation_profile(), region_x, region_z);

    #[cfg(not(seedmap_core))]
    if let Some(found) = cache.lock().unwrap_or_else(|poison| poison.into_inner()).get(&key) {
        return *found;
    }

    let (chunk_x, chunk_z) = attempt(terrain.seed(), region_x, region_z);
    let (x, z) = (chunk_x * 16, chunk_z * 16);
    let biome = |dx: i32, dz: i32| terrain.column_in(x + dx, z + dz, None).biome;
    // Вики, Ocean Monument: «everywhere within a 16 block square radius».
    // Проверяются все клетки биомов 4×4, иначе узкая суша между пробами
    // через восемь блоков пропускала неподходящую площадку.
    let ring = |radius, test: fn(super::terrain::Biome) -> bool| {
        biome_square(x,z,radius,&|px,pz| test(biome(px-x,pz-z)))
    };
    let found = (ring(16, deep) && ring(29, watery)).then_some((chunk_x, chunk_z));

    #[cfg(not(seedmap_core))]
    cache.lock().unwrap_or_else(|poison| poison.into_inner()).insert(key, found);
    found
}

/// Какая из снятых крепостей и на сколько четвертей повёрнута.
fn pick(seed: i64, chunk_x: i32, chunk_z: i32) -> (usize, u8) {
    let dice = roll(seed, chunk_x, chunk_z, 0x00A1_7E5B);

    ((dice % library().len().max(1) as u64) as usize, ((dice >> 16) % 4) as u8)
}

/// Клетка образца для клетки рамки (dx, dz от угла) при повороте `turns`.
fn local(dx: i32, dz: i32, turns: u8) -> (i32, i32) {
    let last = SIZE - 1;

    match turns {
        0 => (dx, dz),
        1 => (dz, last - dx),
        2 => (last - dx, last - dz),
        _ => (last - dz, dx),
    }
}

/// Точка образца (a, b) в рамке при повороте `turns` — обратное к `local`.
fn turned(a: f64, b: f64, turns: u8) -> (f64, f64) {
    let size = f64::from(SIZE);

    match turns {
        0 => (a, b),
        1 => (size - b, a),
        2 => (size - a, size - b),
        _ => (b, size - a),
    }
}

/// Старты крепостей, чьи рамки могут задеть этот чанк.
fn near(terrain: &super::terrain::Terrain, chunk_x: i32, chunk_z: i32) -> Vec<(i32, i32)> {
    let mut out = Vec::new();

    for region_x in (chunk_x - 2).div_euclid(SPACING)..=(chunk_x + 2).div_euclid(SPACING) {
        for region_z in (chunk_z - 2).div_euclid(SPACING)..=(chunk_z + 2).div_euclid(SPACING) {
            let (x, z) = attempt(terrain.seed(), region_x, region_z);

            if (x - chunk_x).abs() > 2 || (z - chunk_z).abs() > 2 {
                continue;
            }

            if let Some(found) = start_in(terrain, region_x, region_z) {
                out.push(found);
            }
        }
    }

    out
}

/// Кладёт в сложенный чанк части ближних крепостей — после растительности и
/// украшений океана: в рамке всё прежнее уходит под воду и крепость.
#[cfg(not(seedmap_core))]
pub(super) fn build(generator: &super::Generator, chunk: &mut super::Chunk, chunk_x: i32, chunk_z: i32) {
    let super::Generator::Normal(terrain) = generator else {
        return;
    };

    for (start_x, start_z) in near(terrain, chunk_x, chunk_z) {
        paint(terrain.seed(), start_x, start_z, chunk_x, chunk_z, chunk);
    }
}

/// Блоки крепости со стартом (start_x, start_z) в чанке (chunk_x, chunk_z).
#[cfg(not(seedmap_core))]
fn paint(seed: i64, start_x: i32, start_z: i32, chunk_x: i32, chunk_z: i32, chunk: &mut super::Chunk) {
    let Some(template) = library().get(pick(seed, start_x, start_z).0) else {
        return;
    };
    let turns = pick(seed, start_x, start_z).1;
    let (left, top) = (start_x * 16 - BACK, start_z * 16 - BACK);
    let bricks = template.states[1];

    for local_x in 0..16 {
        for local_z in 0..16 {
            let (dx, dz) = (chunk_x * 16 + local_x - left, chunk_z * 16 + local_z - top);

            if !(0..SIZE).contains(&dx) || !(0..SIZE).contains(&dz) {
                continue;
            }

            let (a, b) = local(dx, dz, turns);

            for y in 0..HEIGHT {
                chunk.put_generated(local_x, BOTTOM + y, local_z, template.states[usize::from(template.cell(a, y, b))], true);
            }

            // Опора: под кирпичом нижнего слоя — кирпич до твёрдого.
            if template.cell(a, 0, b) == 1 {
                let mut y = BOTTOM - 1;

                while y > super::MIN_Y && !holds(chunk.block(local_x, y, local_z)) {
                    chunk.put_generated(local_x, y, local_z, bricks, true);
                    y -= 1;
                }
            }
        }
    }
}

/// Держит ли блок опору: не воздух и не жидкость, не растение.
fn holds(state: i32) -> bool {
    state != super::AIR && !blocks::block_at_state(state).is_some_and(|name| matches!(name, "water" | "lava" | "seagrass" | "tall_seagrass" | "kelp" | "kelp_plant" | "bubble_column"))
}

/// Внутри ли рамки какой-нибудь крепости клетка (правило появления стражей).
pub fn inside(terrain: &super::terrain::Terrain, x: i32, y: i32, z: i32) -> bool {
    if !(BOTTOM..BOTTOM + HEIGHT).contains(&y) {
        return false;
    }

    near(terrain, x.div_euclid(16), z.div_euclid(16)).into_iter().any(|(start_x, start_z)| {
        (start_x * 16 - BACK..start_x * 16 - BACK + SIZE).contains(&x) && (start_z * 16 - BACK..start_z * 16 - BACK + SIZE).contains(&z)
    })
}

/// Древние стражи, стоящие в чанке (их ставит такт мобов при сложении).
pub fn elders(terrain: &super::terrain::Terrain, chunk_x: i32, chunk_z: i32) -> Vec<(f64, f64, f64)> {
    let mut out = Vec::new();

    for (start_x, start_z) in near(terrain, chunk_x, chunk_z) {
        let (index, turns) = pick(terrain.seed(), start_x, start_z);
        let Some(template) = library().get(index) else {
            continue;
        };
        let (left, top) = (start_x * 16 - BACK, start_z * 16 - BACK);

        for &(a, y, b) in &template.elders {
            let (dx, dz) = turned(a, b, turns);
            let (x, z) = (f64::from(left) + dx, f64::from(top) + dz);

            if (x.floor() as i32).div_euclid(16) == chunk_x && (z.floor() as i32).div_euclid(16) == chunk_z {
                out.push((x, f64::from(BOTTOM) + y, z));
            }
        }
    }

    out
}

/// Ближайшая крепость (`locate structure minecraft:monument`): чанк старта.
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

#[cfg(all(test, not(seedmap_core)))]
mod tests {
    use super::*;

    /// Образцы разобраны: четыре крепости, у каждой три древних стража,
    /// 8 золотых блоков в сокровищнице (вики: «eight blocks of gold»).
    #[test]
    fn templates_parse() {
        let library = library();
        let gold = blocks::state_by_name("gold_block").expect("золото есть");

        assert_eq!(library.len(), 4);

        for template in library {
            assert_eq!(template.elders.len(), 3);

            let golds = template.cells.iter().filter(|cell| template.states[usize::from(**cell)] == gold).count();

            assert_eq!(golds, 8);
        }
    }

    /// Поворот: `turned` обратен `local` для середин клеток.
    #[test]
    fn rotation_round_trip() {
        for turns in 0..4 {
            for (dx, dz) in [(0, 0), (5, 40), (57, 3), (20, 20)] {
                let (a, b) = local(dx, dz, turns);
                let (x, z) = turned(f64::from(a) + 0.5, f64::from(b) + 0.5, turns);

                assert_eq!((x.floor() as i32, z.floor() as i32), (dx, dz), "поворот {turns}");
            }
        }
    }

    /// Старты: в своей области, не ближе 5 чанков к соседней.
    #[test]
    fn attempts_keep_the_separation() {
        for region_x in -6..6 {
            for region_z in -6..6 {
                let (x, z) = attempt(42, region_x, region_z);

                assert!((0..SPACING - SEPARATION).contains(&(x - region_x * SPACING)));
                assert!((0..SPACING - SEPARATION).contains(&(z - region_z * SPACING)));
            }
        }
    }

    /// Независимые от подбора климата семена: глубокого океана должно
    /// хватать не только точке старта, но и всей проверяемой окрестности.
    #[test]
    fn deep_oceans_allow_monument_placement() {
        use super::super::terrain::{Style, Terrain};

        let mut starts = 0;
        for seed in [7, 314_159, 987_654_321, -20_261_010] {
            let terrain = Terrain::new(seed, Style::Vanilla);
            for rx in -16..16 {
                for rz in -16..16 {
                    starts += usize::from(start_in(&terrain, rx, rz).is_some());
                }
            }
        }
        println!("holdout monuments: {starts}/4096");
        assert!((246..=574).contains(&starts), "крепостей {starts}/4096, ожидается 6–14%");
    }

    /// Перепись блоков крепости — против замера оригинала (tools/measure/
    /// monument.py census, 10 крепостей, семя 1234567890): среднее на крепость
    /// по нашим четырём образцам в пределах разброса оригинала.
    #[test]
    fn monument_census() {
        const MEASURED: [(&str, f64, f64); 6] = [
            // имя, среднее оригинала, допуск (разброс между крепостями)
            ("prismarine", 6704.0, 250.0),
            ("prismarine_bricks", 6149.2, 200.0),
            ("dark_prismarine", 304.6, 110.0),
            ("sea_lantern", 126.4, 15.0),
            ("gold_block", 8.0, 0.0),
            ("wet_sponge", 44.0, 45.0),
        ];
        let library = library();

        for (name, measured, spread) in MEASURED {
            let state = blocks::state_by_name(name).expect("блок есть");
            let ours: f64 = library.iter().map(|template| template.cells.iter().filter(|cell| template.states[usize::from(**cell)] == state).count() as f64).sum::<f64>() / library.len() as f64;

            assert!((ours - measured).abs() <= spread, "{name}: у нас {ours}, оригинал {measured}");
        }
    }

    /// Крепости в нашем мире (семя замера оригинала 100554032945340, 32×32
    /// области вокруг нуля): сколько стоит и перепись блоков сложенной
    /// крепости — против замера оригинала (`cargo test --release
    /// monument_world -- --ignored --nocapture`).
    #[test]
    #[ignore]
    fn monument_world() {
        use super::super::{terrain, Chunk, Generator};

        let generator = Generator::Normal(terrain::Terrain::new(100_554_032_945_340, terrain::Style::Vanilla));
        let Generator::Normal(terrain) = &generator else { unreachable!() };
        let starts: Vec<(i32, i32)> = (-16..16).flat_map(|x| (-16..16).map(move |z| (x, z))).filter_map(|(x, z)| start_in(terrain, x, z)).collect();

        let centres = (-16..16).flat_map(|x| (-16..16).map(move |z| (x, z))).filter(|&(x, z)| {
            let (cx, cz) = attempt(terrain.seed(), x, z);

            deep(terrain.column_in(cx * 16, cz * 16, None).biome)
        }).count();

        println!("областей 1024, середина в глубоком океане {centres}, крепостей {}", starts.len());
        assert!(!starts.is_empty());

        let (start_x, start_z) = starts[0];
        let names = ["prismarine", "prismarine_bricks", "dark_prismarine", "sea_lantern", "gold_block", "wet_sponge", "water"];
        let mut counts = [0u32; 7];
        let (left, top) = (start_x * 16 - BACK, start_z * 16 - BACK);

        for chunk_x in left.div_euclid(16)..=(left + SIZE - 1).div_euclid(16) {
            for chunk_z in top.div_euclid(16)..=(top + SIZE - 1).div_euclid(16) {
                let chunk = Chunk::generated(&generator, chunk_x, chunk_z);

                for x in 0..16 {
                    for z in 0..16 {
                        let (wx, wz) = (chunk_x * 16 + x, chunk_z * 16 + z);

                        if !(left..left + SIZE).contains(&wx) || !(top..top + SIZE).contains(&wz) {
                            continue;
                        }

                        for y in BOTTOM..BOTTOM + HEIGHT {
                            let name = blocks::block_at_state(chunk.block(x, y, z)).unwrap_or("air");

                            if let Some(at) = names.iter().position(|known| *known == name) {
                                counts[at] += 1;
                            }
                        }
                    }
                }
            }
        }

        println!("крепость {start_x},{start_z}: {}", names.iter().zip(counts).map(|(name, count)| format!("{name}={count}")).collect::<Vec<_>>().join(" "));
        assert_eq!(counts[4], 8);
    }
}

#[cfg(all(test, not(seedmap_core)))]
pub(super) fn audit_box(seed: i64, cx: i32, cz: i32) -> serde_json::Value {
    serde_json::json!({"name":"whole_snapshot","rotation":pick(seed,cx,cz).1,"box":[cx*16-BACK,BOTTOM,cz*16-BACK,cx*16-BACK+SIZE-1,BOTTOM+HEIGHT-1,cz*16-BACK+SIZE-1]})
}

fn biome_square(x:i32,z:i32,radius:i32,accept:&dyn Fn(i32,i32)->bool) -> bool {
    ((x-radius).div_euclid(4)..=(x+radius).div_euclid(4)).all(|qx| {
            ((z-radius).div_euclid(4)..=(z+radius).div_euclid(4)).all(|qz| accept(qx*4+2,qz*4+2))
        })
}

#[cfg(all(test, not(seedmap_core)))]
#[test]
fn monument_checks_every_biome_cell_including_narrow_interior_land() {
    assert!(biome_square(-16,32,16,&|_,_|true));
    assert!(!biome_square(-16,32,16,&|x,z| (x,z)!=(-10,38)));
    assert!(!biome_square(0,0,29,&|x,z| (x,z)!=(30,30)));
    assert!(deep(super::terrain::Biome::DeepFrozenOcean));
    assert!(watery(super::terrain::Biome::River));
    assert!(!deep(super::terrain::Biome::River));
}
