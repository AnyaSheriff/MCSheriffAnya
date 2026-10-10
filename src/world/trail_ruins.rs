// Руины тропы (trail ruins): где стоят и чем сложены.
//
// Где — по вики (Structure set «trail_ruins»: spacing 34, separation 8;
// Trail Ruins, «Generation»): мир поделён на области 34×34 чанка, в каждой
// одна попытка — чанк со сдвигом 0…25; руины встают, если биом середины
// чанка — тайга, снежная тайга, старовозрастные тайги, старовозрастный
// березняк или джунгли.
//
// Чем — замер мира оригинала 26.1.2 (tools/measure/template_structs.py dump
// и layouts, template_merge.py templates trail_ruins; src/world/
// trail_ruins.txt): куски сведены по большинству экземпляров (14 руин, 246
// кусков), а раскладка руины — одна из 14 снятых (куски, их повороты и места
// от угла башни), повёрнутая на случайную четверть. Собирать раскладку по
// стыкам, как оригинал, — потом [проверить]. Камень и землю в кусках даёт
// рельеф (руины закопаны в камень — свой камень от чужого по замеру не
// отличить); подозрительный гравий — свой гравий с долей замера 8 % (у
// оригинала его кладёт обработка кусков «archaeology») [проверить: кисти у
// нас нет, блок-сущность без таблицы].
//
// Высота (замер 14 руин): низ башни — на 15 блоков ниже верха земли у её
// середины (у всех 14: −15 ± 1), верх башни торчит над землёй.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use super::template::{self, Cell, Orient, Template};
use crate::blocks;

pub const SPACING: i32 = 34;
pub const SEPARATION: i32 = 8;
const SALT: u64 = 83_469_867;

/// Низ башни под верхом земли (замер).
const BURIED: i32 = 15;

/// Подозрительный среди своего гравия (замер: 94 из ~1190 на руину).
const SUSPICIOUS: f64 = 0.08;

/// Дальше этого от угла башни кусков нет (замер: раскладки до 52 блоков).
const REACH_CHUNKS: i32 = 4;

/// Кусок раскладки: образец, поворот, угол рамки от угла башни.
#[derive(Clone, Copy, Debug)]
struct Entry {
    template: usize,
    turns: u8,
    offset: (i32, i32, i32),
}

struct Library {
    templates: Vec<Template>,
    layouts: Vec<Vec<Entry>>,
}

fn library() -> &'static Library {
    static LIBRARY: OnceLock<Library> = OnceLock::new();

    LIBRARY.get_or_init(|| {
        let text = include_str!("trail_ruins.txt");
        let templates = template::parse(text);
        let mut layouts: Vec<Vec<Entry>> = Vec::new();

        for line in text.lines() {
            if line.starts_with("layout") {
                layouts.push(Vec::new());
                continue;
            }

            if line.starts_with("template") {
                break;
            }

            let words: Vec<&str> = line.split_whitespace().collect();

            if let ([name, turns, dx, dy, dz], Some(layout)) = (words.as_slice(), layouts.last_mut())
                && let Some(template) = templates.iter().position(|known| known.name == *name)
            {
                let number = |text: &str| text.parse::<i32>().unwrap_or(0);

                layout.push(Entry { template, turns: number(turns) as u8, offset: (number(dx), number(dy), number(dz)) });
            }
        }

        Library { templates, layouts }
    })
}

/// Биом руин (вики).
pub fn fits(biome: super::terrain::Biome) -> bool {
    use super::terrain::Biome;

    matches!(biome, Biome::Taiga | Biome::SnowyTaiga | Biome::OldGrowthPineTaiga | Biome::OldGrowthSpruceTaiga | Biome::OldGrowthBirchForest | Biome::Jungle)
}

pub fn attempt(seed: i64, region_x: i32, region_z: i32) -> (i32, i32) {
    template::attempt(seed, region_x, region_z, SPACING, SEPARATION, SALT)
}

/// Поставленный кусок: образец, положение, угол рамки в мире.
#[derive(Clone, Copy, Debug)]
pub struct Placed {
    template: usize,
    orient: Orient,
    origin: (i32, i32, i32),
}

/// Руины области: чанк старта и куски.
#[derive(Clone, Debug)]
pub struct Site {
    pub chunk: (i32, i32),
    pieces: Vec<Placed>,
}

/// Поворот точки (x, z) на `turns` четвертей по часовой.
fn spin(x: i32, z: i32, turns: u8) -> (i32, i32) {
    match turns % 4 {
        0 => (x, z),
        1 => (-z, x),
        2 => (-x, -z),
        _ => (z, -x),
    }
}

/// Рамка куска после поворота всей раскладки: угол и сторона.
fn turned_box(library: &Library, entry: &Entry, turns: u8) -> ((i32, i32), (i32, i32)) {
    let template = &library.templates[entry.template];
    let (fx, fz) = template.footprint(Orient { turns: entry.turns, mirror: false });
    let corners = [(entry.offset.0, entry.offset.2), (entry.offset.0 + fx - 1, entry.offset.2 + fz - 1)].map(|(x, z)| spin(x, z, turns));
    let low = (corners[0].0.min(corners[1].0), corners[0].1.min(corners[1].1));

    (low, template.footprint(Orient { turns: (entry.turns + turns) % 4, mirror: false }))
}

pub fn site_in(terrain: &super::terrain::Terrain, region_x: i32, region_z: i32) -> Option<Arc<Site>> {
    type Sites = Mutex<HashMap<(i64, i32, i32), Option<Arc<Site>>>>;
    static CACHE: OnceLock<Sites> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let key = (terrain.seed(), region_x, region_z);

    if let Some(found) = cache.lock().unwrap_or_else(|poison| poison.into_inner()).get(&key) {
        return found.clone();
    }

    let seed = terrain.seed();
    let (chunk_x, chunk_z) = attempt(seed, region_x, region_z);
    let (x, z) = (chunk_x * 16, chunk_z * 16);
    let library = library();
    let found = (fits(terrain.column_in(x + 8, z + 8, None).biome) && !library.layouts.is_empty()).then(|| {
        let dice = template::roll(seed, chunk_x, chunk_z, SALT ^ 0x7A11);
        let layout = &library.layouts[(dice % library.layouts.len() as u64) as usize];
        let turns = ((dice >> 16) % 4) as u8;
        // Башня — первый кусок раскладки, её угол — угол чанка старта.
        let (tower_low, tower_size) = turned_box(library, &layout[0], turns);
        let shift = (x - tower_low.0, z - tower_low.1);
        let ground = terrain.column_in(x + tower_size.0 / 2, z + tower_size.1 / 2, None).height;
        let base = ground - BURIED - layout[0].offset.1;
        let pieces = layout
            .iter()
            .map(|entry| {
                let (low, _) = turned_box(library, entry, turns);

                Placed {
                    template: entry.template,
                    orient: Orient { turns: (entry.turns + turns) % 4, mirror: false },
                    origin: (low.0 + shift.0, base + entry.offset.1, low.1 + shift.1),
                }
            })
            .collect();

        Arc::new(Site { chunk: (chunk_x, chunk_z), pieces })
    });

    cache.lock().unwrap_or_else(|poison| poison.into_inner()).insert(key, found.clone());
    found
}

fn state_of(text: &str) -> i32 {
    blocks::state_from_text(text).or_else(|| blocks::state_by_name(text.split('[').next().unwrap_or(text))).unwrap_or(super::AIR)
}

pub(super) fn build(generator: &super::Generator, chunk: &mut super::Chunk, chunk_x: i32, chunk_z: i32) {
    let super::Generator::Normal(terrain) = generator else {
        return;
    };
    let seed = terrain.seed();
    let library = library();
    let (cx0, cz0) = (chunk_x * 16, chunk_z * 16);

    for region_x in (chunk_x - REACH_CHUNKS).div_euclid(SPACING)..=(chunk_x + REACH_CHUNKS).div_euclid(SPACING) {
        for region_z in (chunk_z - REACH_CHUNKS).div_euclid(SPACING)..=(chunk_z + REACH_CHUNKS).div_euclid(SPACING) {
            let (x, z) = attempt(seed, region_x, region_z);

            if (x - chunk_x).abs() > REACH_CHUNKS || (z - chunk_z).abs() > REACH_CHUNKS {
                continue;
            }

            let Some(site) = site_in(terrain, region_x, region_z) else { continue };

            for piece in &site.pieces {
                let template = &library.templates[piece.template];
                let (sx, sz) = template.footprint(piece.orient);
                let (x0, y0, z0) = piece.origin;

                if x0 > cx0 + 15 || x0 + sx <= cx0 || z0 > cz0 + 15 || z0 + sz <= cz0 {
                    continue;
                }

                for (a, y, b, cell) in template.cells() {
                    let Cell::Block(index) = cell else { continue };
                    let (dx, dz) = piece.orient.offset(a, b, template.width, template.length);
                    let (x, wy, z) = (x0 + dx, y0 + y, z0 + dz);

                    if x < cx0 || x > cx0 + 15 || z < cz0 || z > cz0 + 15 || !(super::MIN_Y..super::MIN_Y + super::WORLD_HEIGHT).contains(&wy) {
                        continue;
                    }

                    let mut text = template::orient_text(&template.texts[usize::from(index)], piece.orient);

                    if text == "gravel" && template::chance(seed, x, wy, z, 0x7A12) < SUSPICIOUS {
                        text = "suspicious_gravel".to_string();
                    }

                    chunk.put_generated(x - cx0, wy, z - cz0, state_of(&text), true);
                    if text.starts_with("suspicious_") {
                        let loot_seed=template::roll(seed,x,z,0xA2C1 ^ wy as u64);
                        chunk.archaeology.insert((x-cx0,wy,z-cz0),crate::archaeology::generated(if loot_seed.is_multiple_of(5) {"trail_ruins_rare"} else {"trail_ruins_common"},loot_seed));
                    }
                }
            }
        }
    }
}

/// Ближайшие руины (`locate structure trail_ruins`): чанк старта.
pub fn nearest(terrain: &super::terrain::Terrain, chunk_x: i32, chunk_z: i32, rings: i32) -> Option<(i32, i32)> {
    template::nearest(chunk_x, chunk_z, SPACING, rings, &|region_x, region_z| site_in(terrain, region_x, region_z).map(|site| site.chunk))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::terrain::Biome;

    /// 14 раскладок, у каждой первая — башня, все куски найдены в образцах.
    #[test]
    fn layouts_parse() {
        let library = library();

        assert_eq!(library.layouts.len(), 14);

        for layout in &library.layouts {
            assert!(library.templates[layout[0].template].name.contains("tower/tower_"));
            assert!(layout.len() >= 10);
        }
    }

    /// Биомы (вики).
    #[test]
    fn biomes_follow_the_wiki() {
        assert!(fits(Biome::OldGrowthBirchForest));
        assert!(fits(Biome::Jungle));
        assert!(!fits(Biome::BambooJungle));
        assert!(!fits(Biome::Plains));
    }

    /// Поворот раскладки: рамки кусков не меняют размера, только местами.
    #[test]
    fn turning_keeps_the_boxes() {
        let library = library();

        for entry in &library.layouts[0] {
            let (_, (fx, fz)) = turned_box(library, entry, 0);
            let (_, (gx, gz)) = turned_box(library, entry, 1);

            assert_eq!((fx, fz), (gz, gx));
        }
    }

    /// Попытки: в своей области, сдвиг 0…25.
    #[test]
    fn attempts_keep_the_separation() {
        for region_x in -5..5 {
            for region_z in -5..5 {
                let (x, z) = attempt(17, region_x, region_z);

                assert!((0..SPACING - SEPARATION).contains(&(x - region_x * SPACING)));
                assert!((0..SPACING - SEPARATION).contains(&(z - region_z * SPACING)));
            }
        }
    }

    /// Перепись руин нашего мира против замера (`cargo test --release
    /// trail_ruins_world -- --ignored --nocapture`).
    #[test]
    #[ignore]
    fn trail_ruins_world() {
        use crate::world::{terrain, Chunk, Generator};

        let generator = Generator::Normal(terrain::Terrain::new(1_234_567_890, terrain::Style::Vanilla));
        let Generator::Normal(terrain) = &generator else { unreachable!() };
        let names = ["gravel", "suspicious_gravel", "mud_bricks", "bricks", "cobblestone", "coarse_dirt", "white_terracotta", "packed_mud"];
        let mut counts = [0u64; 8];
        let mut sites = 0;

        for region_x in -8..8 {
            for region_z in -8..8 {
                let Some(site) = site_in(terrain, region_x, region_z) else { continue };

                if sites >= 6 {
                    continue;
                }

                sites += 1;
                let library = library();
                let mut seen = std::collections::HashSet::new();

                for piece in &site.pieces {
                    let template = &library.templates[piece.template];
                    let (sx, sz) = template.footprint(piece.orient);
                    let (x0, y0, z0) = piece.origin;

                    for chunk_x in x0.div_euclid(16)..=(x0 + sx - 1).div_euclid(16) {
                        for chunk_z in z0.div_euclid(16)..=(z0 + sz - 1).div_euclid(16) {
                            let chunk = Chunk::generated(&generator, chunk_x, chunk_z);

                            for x in x0.max(chunk_x * 16)..(x0 + sx).min(chunk_x * 16 + 16) {
                                for z in z0.max(chunk_z * 16)..(z0 + sz).min(chunk_z * 16 + 16) {
                                    for y in y0..y0 + template.height {
                                        if !seen.insert((x, y, z)) {
                                            continue;
                                        }

                                        let name = blocks::block_at_state(chunk.block(x - chunk_x * 16, y, z - chunk_z * 16)).unwrap_or("air");

                                        if let Some(at) = names.iter().position(|known| *known == name) {
                                            counts[at] += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        let line: Vec<String> = names.iter().zip(counts).map(|(name, sum)| format!("{}={:.1}", name, sum as f64 / f64::from(sites.max(1)))).collect();

        println!("руин {sites}: {}", line.join(" "));
        assert!(sites > 0);
    }
}
