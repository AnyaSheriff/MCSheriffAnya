// Регрессия реальной генерации: проверяем итоговые блоки, а не только план.
use super::*;
use super::super::{Chunk, Generator, structure_terrain::{self, Ground}, terrain::{Style, Terrain}};

pub(super) fn earth(state: i32) -> bool {
    // Руда остаётся частью грунта; пересекающиеся дороги могут опираться друг на друга.
    blocks::block_at_state(state).is_some_and(|name| name.ends_with("_ore")) || matches!(blocks::block_at_state(state), Some("stone" | "dirt_path" | "dirt" | "grass_block" | "sand" | "sandstone" | "gravel" | "coarse_dirt" | "mud" | "clay" | "terracotta" | "red_sand" | "red_sandstone" | "podzol" | "snow_block"))
}

fn check_village(seed: i64, kind: Kind, sx: i32, sz: i32) {
    let terrain = Terrain::new(seed, Style::Vanilla);
    let generator = Generator::Normal(Terrain::new(seed, Style::Vanilla));
    let library = library(kind).unwrap();
    // Сначала другой тип мира с тем же семенем: ключ кэша обязан различать их.
    let _ = plan_cached(&Terrain::new(seed, Style::Smooth), kind, sx, sz);
    let pieces = plan_cached(&terrain, kind, sx, sz);
    let bounds = pieces.iter().fold([i32::MAX, i32::MAX, i32::MIN, i32::MIN], |b, p| [b[0].min(p.bounds[0]), b[1].min(p.bounds[1]), b[2].max(p.bounds[2]), b[3].max(p.bounds[3])]);
    let original = Ground::new(&terrain);
    assert_eq!(*pieces, plan(seed, kind, sx, sz, &|x, z| original.supported_height(x, z)), "план без кэша после перезапуска");
    let mut chunks = HashMap::new();
    let mut positions = Vec::new();
    for cx in bounds[0].div_euclid(16)..=bounds[2].div_euclid(16) {
        for cz in bounds[1].div_euclid(16)..=bounds[3].div_euclid(16) {
            chunks.insert((cx, cz), Chunk::generated(&generator, cx, cz));
            positions.push((cx, cz));
        }
    }
    let block = |x: i32, y: i32, z: i32| chunks[&(x.div_euclid(16), z.div_euclid(16))].block(x.rem_euclid(16), y, z.rem_euclid(16));
    let mut streets = 0;
    let mut floors = 0;
    for &(cx, cz) in &positions {
        let beards = structure_terrain::near(&terrain, cx, cz);
        let ground = Ground::new(&terrain);
        for piece in pieces.iter() {
            if piece.template == HAY_PILE { continue; }
            let template = &library.templates[piece.template];
            for b in 0..template.length {
                for a in 0..template.width {
                    let (dx, dz) = turn(a, b, piece.turns);
                    let (x, z) = (piece.origin.0 + dx, piece.origin.2 + dz);
                    if x.div_euclid(16) != cx || z.div_euclid(16) != cz { continue; }
                    if template.rigid {
                        if let Cell::Block(_) = template.cell(a, 0, b) {
                            floors += 1;
                            let y = piece.origin.1;
                            assert_ne!(block(x, y - 1, z), super::super::AIR, "{kind:?} {} floor ({x},{y},{z})", template.key);
                        }
                    } else {
                        let base = structure_terrain::street_height(&ground, x, z, &beards);
                        for y in 0..template.height {
                            let Cell::Block(index) = template.cell(a, y, b) else { continue };
                            // Поздняя структура может заменить дорогу (например,
                            // храм пустыни): это уже не блок уличного образца.
                            if block(x, base + y, z) != template.states[piece.turns as usize][index as usize] { continue; }
                            streets += 1;
                            // Верхний слой двухслойного замера может лежать на
                            // нижнем; земля должна быть непосредственно под ним.
                            let under = base - 1;
                            assert!(earth(block(x, under, z)), "{kind:?} {} street ({x},{},{z}), ground at {under}: {:?}", template.key, base + y, blocks::block_at_state(block(x, under, z)));
                        }
                    }
                }
            }
        }
    }
    println!("{kind:?}: {} chunks, {streets} street blocks, {floors} floor blocks", positions.len());
    assert!(streets > 100, "{kind:?}: {streets}");
    assert!(floors > 100, "{kind:?}: {floors}");
    // Новый Terrain и обратный порядок чанков; сравниваем все блоки секций.
    let restarted = Generator::Normal(Terrain::new(seed, Style::Vanilla));
    for &(cx, cz) in positions.iter().rev() {
        assert_eq!(chunks[&(cx, cz)].sections, Chunk::generated(&restarted, cx, cz).sections, "порядок чанков {cx},{cz}");
    }
}

#[test]
fn reported_entire_plains_village_has_supported_streets_and_floors() {
    let seed = 1_791_716_133_710_533_940;
    let terrain = Terrain::new(seed, Style::Vanilla);
    let (sx, sz) = nearest(&terrain, Kind::Plains, -11, 5, 12).unwrap();
    let kind = Kind::Plains;
    let original = terrain.column_at(-184, 119).height;
    let wrong = terrain.column_in(-184, 119, None).height;
    assert!(wrong > original + 3, "источник ошибки: {wrong} / {original}");
    check_village(seed, kind, sx, sz);
}

#[test]
fn desert_village_has_supported_streets_and_floors() {
    let seed = 42;
    let terrain = Terrain::new(seed, Style::Vanilla);
    // После расширения пустынь ближайшую к нулю деревню (-22,87)
    // пересекает пирамида (-26,86), чья комната намеренно вырезает дом.
    // Для проверки опоры выбираем отдельную деревню без этого пересечения.
    let (sx, sz) = nearest(&terrain, Kind::Desert, -128, 0, 12).unwrap();
    check_village(seed, Kind::Desert, sx, sz);
}

#[test]
fn savanna_village_has_supported_streets_and_floors() {
    let seed = 1_234_567_890;
    let terrain = Terrain::new(seed, Style::Vanilla);
    let (sx, sz) = nearest(&terrain, Kind::Savanna, 0, 0, 12).unwrap();
    check_village(seed, Kind::Savanna, sx, sz);
}
