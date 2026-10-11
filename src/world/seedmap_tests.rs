use super::{seedmap::{self, Map, Dimension}, terrain::{Terrain, Style}, Chunk, Generator};
use seedmap_core::world::seedmap as extracted;

#[test]
fn extracted_biomes_and_spawn_agree_with_server() {
    for seed in [i64::MIN, -81273491, 123, i64::MAX] {
        for smooth in [false, true] {
            let terrain = Terrain::new(seed, if smooth { Style::Smooth } else { Style::Vanilla });
            let map = extracted::Map::new(seed, extracted::Dimension::Overworld, smooth, false);
            for (x, z) in [(0, 0), (-1, -1), (-17, 33), (1041, -2307), (29_000_000 - 16, -28_999_997)] {
                assert_eq!(i32::from(map.biome(x, z)), terrain.column_at(x, z).biome.number(), "seed {seed}, smooth {smooth}, {x},{z}");
            }
            assert_eq!(map.spawn(), super::spawn_search::find(|x, z| terrain.column_at(x, z)));
        }
        for (dimension, external) in [(Dimension::Nether, extracted::Dimension::Nether), (Dimension::End, extracted::Dimension::End)] {
            let server = Map::new(seed, dimension, false, false);
            let map = extracted::Map::new(seed, external, false, false);
            for (x, z) in [(-1, -1), (0, 0), (1001, -1002), (3999, 4077)] {
                assert_eq!(server.biome(x, z), map.biome(x, z));
            }
        }
    }
}

#[test]
fn every_structure_agrees_across_native_builds() {
    for (seed, smooth) in [(7123121, false), (-6712339, true), (9223372036854775000, false)] {
        for id in 0..3 {
            let server = Map::new(seed, Dimension::from_id(id).unwrap(), smooth, false);
            let map = extracted::Map::new(seed, extracted::Dimension::from_id(id).unwrap(), smooth, false);
            for kind in 0..20 {
                let expected = server.structures(kind, -2048, -2048, 2048, 2048).unwrap();
                let actual = map.structures(kind, -2048, -2048, 2048, 2048).unwrap();
                assert_eq!(expected.iter().map(|m| (m.kind, m.x, m.z)).collect::<Vec<_>>(),
                    actual.iter().map(|m| (m.kind, m.x, m.z)).collect::<Vec<_>>(), "seed {seed}, dimension {id}, kind {kind}");
            }
        }
    }
}

#[test]
fn map_matches_biome_cells_in_generated_chunks() {
    let seed = -7712349;
    for (generator, dimension) in [
        (Generator::Normal(Terrain::new(seed, Style::Vanilla)), extracted::Dimension::Overworld),
        (Generator::Nether(super::nether::Nether::new(seed)), extracted::Dimension::Nether),
        (Generator::End(super::the_end::End::new(seed)), extracted::Dimension::End),
    ] {
        let (cx, cz) = (-2, 67);
        let chunk = Chunk::generated(&generator, cx, cz);
        let map = extracted::Map::new(seed, dimension, false, false);
        for x in 0..16 { for z in 0..16 {
            assert_eq!(chunk.biome(x, 319, z), u16::from(map.biome(cx * 16 + x, cz * 16 + z)));
        } }
    }
}

#[test]
fn palette_covers_the_server_registry() {
    assert_eq!(super::terrain::biome_registry_size(), 65);
    for id in 0..65 { assert_eq!(seedmap::color(id), extracted::color(id)); }
}
