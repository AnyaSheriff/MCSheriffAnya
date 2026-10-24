// Древние города (ancient city): где стоят и чем сложены.
//
// Где — по вики (Structure set «ancient_cities»: spacing 24, separation 8,
// salt 20083232, linear): мир поделён на области 24×24 чанка, в каждой одна
// попытка — чанк со сдвигом 0…15 по каждой оси, ровно. Город встаёт, если в
// начале — глубокая тьма (вики, Ancient City: «generate only in the deep
// dark biome»). Пол — на одной высоте (вики: «The floor always generates at
// Y=-51»; замер: низ стартового куска −52 у всех городов). Случай — свой
// (своя генерация), закон — оригинала.
//
// Чем — замер мира оригинала 26.1.2 (tools/measure/ancient_city.py): сервер
// оригинала только складывал мир, а блоки сняты с region-файлов в рамках
// кусков города (запись о строении: 73…96 кусков, рамка до 218×31×216).
// Город у оригинала собирается из кусков случайно (jigsaw), поэтому каждый
// снятый город — свой образец (src/world/ancient_city.txt, формат образцов
// испытательных камер): город повторяет один из них, повёрнутый на
// случайную четверть вокруг угла чанка старта. Порода в кусках остаётся
// как была, воздух кусков вырезает залы, остальное — блоки города: скалк и
// его датчики, крикуны (can_summon=true — так у оригинала в городах),
// катализаторы, души-фонари и огонь, свечи, рамка из укреплённого
// глубинного сланца, ледник со снегом и плотным льдом.
//
// Сундуки — с таблицами по месту (поле LootTable у экземпляров:
// chests/ancient_city, в леднике — chests/ancient_city_ice_box).
//
// Мобы сами в городе не появляются (вики: «Mobs cannot spawn naturally in
// ancient cities») — `inside` для правила появления; варден приходит
// только на крик крикуна.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use super::template;
use super::trial_chambers::{parse, turn, unturn, Template, KEEP};

/// Область попыток, зазор и соль (вики, Structure set).
pub const SPACING: i32 = 24;
pub const SEPARATION: i32 = 8;
const SALT: u64 = 20_083_232;

/// Как далеко от чанка старта доходит рамка (замер: сдвиг до −112, ширина
/// до 218 — не дальше 8 чанков при любом повороте).
const REACH: i32 = 8;

/// Высота, на которой проверяется биом начала: пол города.
const FLOOR: i32 = -51;

fn library() -> &'static [Template] {
    static LIBRARY: OnceLock<Vec<Template>> = OnceLock::new();

    LIBRARY.get_or_init(|| parse(include_str!("ancient_city.txt")))
}

/// Чанк попытки в области: ровный сдвиг 0…15.
pub fn attempt(seed: i64, region_x: i32, region_z: i32) -> (i32, i32) {
    template::attempt(seed, region_x, region_z, SPACING, SEPARATION, SALT)
}

/// Какой из снятых городов и на сколько четвертей повёрнут.
fn pick(seed: i64, chunk_x: i32, chunk_z: i32) -> (usize, u8) {
    let dice = template::roll(seed, chunk_x, chunk_z, SALT ^ 0xA7C1);

    ((dice % library().len().max(1) as u64) as usize, ((dice >> 16) % 4) as u8)
}

/// Стоит ли город в этой области: чанк старта.
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

    let (chunk_x, chunk_z) = attempt(terrain.seed(), region_x, region_z);
    let column = terrain.column_at(chunk_x * 16 + 8, chunk_z * 16 + 8);
    let dark = super::cave_biomes::biome_at(&column, FLOOR).is_some_and(|biome| biome == super::terrain::Biome::DeepDark);
    let found = (dark && !library().is_empty()).then_some((chunk_x, chunk_z));

    #[cfg(not(seedmap_core))]
    cache.lock().unwrap_or_else(|poison| poison.into_inner()).insert(key, found);
    found
}

/// Поставленный город: образец, поворот, угол чанка старта.
struct Placed {
    template: &'static Template,
    turns: u8,
    corner: (i32, i32),
}

impl Placed {
    fn new(seed: i64, start_x: i32, start_z: i32) -> Option<Placed> {
        let (index, turns) = pick(seed, start_x, start_z);

        Some(Placed { template: library().get(index)?, turns, corner: (start_x * 16, start_z * 16) })
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
}

/// Старты городов, чьи рамки могут задеть этот чанк.
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

/// Таблица добычи по имени из образца.
#[cfg(not(seedmap_core))]
fn table(name: &str) -> &'static [crate::loot::Pool] {
    match name {
        "chests/ancient_city_ice_box" => &crate::loot::ANCIENT_CITY_ICE_BOX,
        _ => &crate::loot::ANCIENT_CITY,
    }
}

/// Кладёт в сложенный чанк части ближних городов — после пещер и пещерных
/// украшений: в кусках порода и скалк глубокой тьмы уступают городу.
#[cfg(not(seedmap_core))]
pub(super) fn build(generator: &super::Generator, chunk: &mut super::Chunk, chunk_x: i32, chunk_z: i32) {
    let super::Generator::Normal(terrain) = generator else {
        return;
    };

    for (start_x, start_z) in near(terrain, chunk_x, chunk_z) {
        if let Some(placed) = Placed::new(terrain.seed(), start_x, start_z) {
            paint(terrain.seed(), &placed, chunk_x, chunk_z, chunk, terrain.preview());
        }
    }
}

/// Блоки и сундуки города в чанке (chunk_x, chunk_z).
#[cfg(not(seedmap_core))]
fn paint(seed: i64, placed: &Placed, chunk_x: i32, chunk_z: i32, chunk: &mut super::Chunk, split_loot: bool) {
    let template = placed.template;
    let states = &template.states[usize::from(placed.turns % 4)];

    for local_x in 0..16 {
        for local_z in 0..16 {
            let Some((a, b)) = placed.to_local(chunk_x * 16 + local_x, chunk_z * 16 + local_z) else {
                continue;
            };

            for y in 0..template.height {
                let cell = template.cell(a, y, b);

                if cell != KEEP {
                    chunk.put_generated(local_x, template.bottom + y, local_z, states[usize::from(cell)], true);
                }
            }
        }
    }

    for &(a, y, b, kind, data) in &template.entities {
        let (x, z) = placed.to_world(a, b);

        if kind != "chest" || x.div_euclid(16) != chunk_x || z.div_euclid(16) != chunk_z {
            continue;
        }

        let wy = template.bottom + y;
        let mut dice = crate::loot::Dice::new(template::roll(seed, x, z, 0xAC17 ^ (wy as u64)));
        let mut chest = crate::containers::Container::new(crate::containers::Kind::Chest);

        // Прежний профиль сохраняет старую раскладку и последовательность RNG.
        chest.slots = if split_loot {
            crate::loot::fill_split_stacks(table(data), chest.slots.len(), &mut dice)
        } else {
            crate::loot::fill(table(data), chest.slots.len(), &mut dice)
        };
        chunk.put_generated_container(x - chunk_x * 16, wy, z - chunk_z * 16, chest);
    }
}

/// Внутри ли рамки города клетка (правило появления мобов).
pub fn inside(terrain: &super::terrain::Terrain, x: i32, y: i32, z: i32) -> bool {
    let (chunk_x, chunk_z) = (x.div_euclid(16), z.div_euclid(16));

    near(terrain, chunk_x, chunk_z).into_iter().filter_map(|(start_x, start_z)| Placed::new(terrain.seed(), start_x, start_z)).any(|placed| {
        (placed.template.bottom..placed.template.bottom + placed.template.height).contains(&y) && placed.to_local(x, z).is_some()
    })
}

/// Ближайший город (`locate structure minecraft:ancient_city`): чанк старта.
pub fn nearest(terrain: &super::terrain::Terrain, chunk_x: i32, chunk_z: i32, rings: i32) -> Option<(i32, i32)> {
    template::nearest(chunk_x, chunk_z, SPACING, rings, &|region_x, region_z| start_in(terrain, region_x, region_z))
}

#[cfg(all(test, not(seedmap_core)))]
mod tests {
    use super::*;
    use crate::blocks;

    fn count(template: &Template, name: &str) -> usize {
        let wanted: Vec<u16> = template.names.iter().enumerate().filter(|(_, known)| known.split('[').next() == Some(name)).map(|(index, _)| index as u16).collect();

        template.cells.iter().filter(|cell| wanted.contains(cell)).count()
    }

    /// Образцы разобраны: все состояния палитры известны, сундуки — там же,
    /// где их блоки, у каждого таблица города или ледника; пол — на −52.
    #[test]
    fn templates_parse() {
        let library = library();

        assert!(library.len() >= 3);

        for template in library {
            assert_eq!(template.start_bottom, -52);

            for (index, name) in template.names.iter().enumerate().skip(2) {
                assert!(blocks::state_from_text(name).is_some(), "{name}");
                assert_ne!(template.states[0][index], super::super::AIR, "{name}");
            }

            for &(a, y, b, kind, data) in &template.entities {
                let block = template.names[usize::from(template.cell(a, y, b))].split('[').next().unwrap_or("");

                assert_eq!(block, kind, "{kind} {data} в {a},{y},{b}");
                assert!(matches!(data, "chests/ancient_city" | "chests/ancient_city_ice_box"), "{data}");
            }

            assert!(count(template, "reinforced_deepslate") >= 56, "рамка из укреплённого сланца");
            // Крикуны города вызывают вардена (вики: «can_summon … true in
            // ancient cities»).
            assert!(template.names.iter().filter(|name| name.starts_with("sculk_shrieker")).all(|name| name.contains("can_summon=true")));
        }
    }

    /// Старты: в своей области, не ближе 8 чанков к соседней.
    #[test]
    fn attempts_keep_the_separation() {
        for region_x in -6..6 {
            for region_z in -6..6 {
                let (x, z) = attempt(42, region_x, region_z);

                assert!((0..SPACING - SEPARATION).contains(&(x - region_x * SPACING)));
                assert!((0..SPACING - SEPARATION).contains(&(z - region_z * SPACING)));
                assert_eq!(pick(42, x, z), pick(42, x, z));
            }
        }
    }

    /// Рамка при любом повороте не уходит дальше `REACH` чанков от старта.
    #[test]
    fn frames_stay_within_reach() {
        for (index, template) in library().iter().enumerate() {
            for turns in 0..4 {
                let placed = Placed { template, turns, corner: (0, 0) };

                for (a, b) in [(0, 0), (template.width - 1, 0), (0, template.depth - 1), (template.width - 1, template.depth - 1)] {
                    let (x, z) = placed.to_world(a, b);

                    assert!(x.div_euclid(16).abs() <= REACH && z.div_euclid(16).abs() <= REACH, "образец {index}, поворот {turns}");
                    assert_eq!(placed.to_local(x, z), Some((a, b)));
                }
            }
        }
    }

    /// Перепись блоков города — против замера оригинала (tools/measure/
    /// ancient_city.py census): среднее на город по нашим образцам в пределах
    /// разброса оригинала.
    #[test]
    fn ancient_city_census() {
        let library = library();

        for (name, measured, spread) in MEASURED {
            let ours: f64 = library.iter().map(|template| count(template, name) as f64).sum::<f64>() / library.len() as f64;

            assert!((ours - measured).abs() <= spread, "{name}: у нас {ours}, оригинал {measured}");
        }
    }

    /// Города в нашем мире: глубокая тьма бывает, город в ней стоит, блоки
    /// сложенного чанка — из образца (`cargo test --release ancient_city_world
    /// -- --ignored --nocapture`).
    #[test]
    #[ignore]
    fn ancient_city_world() {
        use super::super::{terrain, Chunk, Generator};

        let generator = Generator::Normal(terrain::Terrain::new(100_554_032_945_340, terrain::Style::Vanilla));
        let Generator::Normal(terrain) = &generator else { unreachable!() };
        let starts: Vec<(i32, i32)> = (-6..6).flat_map(|x| (-6..6).map(move |z| (x, z))).filter_map(|(x, z)| start_in(terrain, x, z)).collect();

        println!("областей 144, городов {}", starts.len());
        assert!(!starts.is_empty());

        let (start_x, start_z) = starts[0];
        let placed = Placed::new(terrain.seed(), start_x, start_z).expect("образец есть");
        let names = ["deepslate_bricks", "deepslate_tiles", "reinforced_deepslate", "sculk_shrieker", "sculk_sensor", "soul_lantern", "chest", "gray_wool"];
        let mut counts = [0u32; 8];
        let mut expected = [0u32; 8];

        for chunk_x in start_x - REACH..=start_x + REACH {
            for chunk_z in start_z - REACH..=start_z + REACH {
                let chunk = Chunk::generated(&generator, chunk_x, chunk_z);

                for x in 0..16 {
                    for z in 0..16 {
                        let Some((a, b)) = placed.to_local(chunk_x * 16 + x, chunk_z * 16 + z) else {
                            continue;
                        };

                        for y in 0..placed.template.height {
                            let cell = placed.template.cell(a, y, b);

                            if cell == KEEP {
                                continue;
                            }

                            let want = placed.template.names[usize::from(cell)].split('[').next().unwrap_or("");
                            let name = blocks::block_at_state(chunk.block(x, placed.template.bottom + y, z)).unwrap_or("air");

                            if let Some(at) = names.iter().position(|known| *known == name) {
                                counts[at] += 1;
                            }

                            if let Some(at) = names.iter().position(|known| *known == want) {
                                expected[at] += 1;
                            }
                        }
                    }
                }
            }
        }

        println!("город {start_x},{start_z}, поворот {}: {}", placed.turns, names.iter().zip(counts).map(|(name, count)| format!("{name}={count}")).collect::<Vec<_>>().join(" "));
        assert_eq!(counts, expected);
        assert!(inside(terrain, start_x * 16 + 8, FLOOR, start_z * 16 + 8) || placed.to_local(start_x * 16 + 8, start_z * 16 + 8).is_none());
    }

    /// Перепись замера оригинала (6 городов, семя 100554032945340): имя,
    /// среднее на город, допуск (разброс между городами).
    const MEASURED: [(&str, f64, f64); 13] = [
        ("gray_wool", 8074.7, 1500.0),
        ("deepslate_bricks", 7462.7, 1200.0),
        ("sculk_vein", 7682.8, 1800.0),
        ("sculk", 6214.7, 1500.0),
        ("deepslate_tiles", 5083.2, 900.0),
        ("sculk_sensor", 294.0, 60.0),
        ("soul_lantern", 135.5, 40.0),
        ("candle", 87.2, 35.0),
        ("soul_fire", 61.5, 20.0),
        ("reinforced_deepslate", 56.0, 0.5),
        ("sculk_catalyst", 48.2, 15.0),
        ("sculk_shrieker", 45.7, 15.0),
        ("chest", 24.0, 8.0),
    ];
}

#[cfg(all(test, not(seedmap_core)))]
pub(super) fn audit_box(seed: i64, cx: i32, cz: i32) -> serde_json::Value {
    let p = Placed::new(seed,cx,cz).unwrap();
    let a = p.to_world(0,0);
    let b = p.to_world(p.template.width-1,p.template.depth-1);
    let y = p.template.bottom;
    serde_json::json!({"name":"whole_snapshot","rotation":p.turns,"box":[a.0.min(b.0),y,a.1.min(b.1),a.0.max(b.0),y+p.template.height-1,a.1.max(b.1)]})
}
