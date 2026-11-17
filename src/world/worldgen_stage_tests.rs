// Только собственные этапы; оригинал наблюдается по конечному сохранению мира.
use super::*;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

const SEEDS: [i64; 4] = [1_792_932_773_345_722_119, 100_554_032_945_340, 42, -20_261_116];
const CHUNKS: [(i32, i32); 2] = [(512, 512), (512, 513)];
const STAGES: [&str; 22] = [
    "base_underground", "dungeons", "mineshaft", "stronghold", "ruined_portal",
    "shipwreck", "ocean_ruin", "trail_ruins", "trees", "vegetation", "structure_terrain",
    "abandoned_camp", "village", "mansion", "temples", "pillager_outpost", "ocean",
    "monument", "cave_biomes", "ancient_city", "trial_chambers", "final",
];

fn state_hash(chunk: &Chunk) -> String {
    let mut hash = Sha256::new();
    hash.update(b"mcsa-stage-blockstates-v1:y,z,x:i32be\n");
    for y in MIN_Y..MIN_Y + WORLD_HEIGHT {
        for z in 0..CHUNK_SIZE { for x in 0..CHUNK_SIZE {
            hash.update(chunk.block(x, y, z).to_be_bytes());
        } }
    }
    format!("{:x}", hash.finalize())
}

fn biome_hash(chunk: &Chunk) -> String {
    let mut hash = Sha256::new();
    hash.update(b"mcsa-stage-biomes-v1:qy,qz,qx:u16be\n");
    for value in &chunk.biomes { hash.update(value.to_be_bytes()); }
    format!("{:x}", hash.finalize())
}

// RLE сохраняет все высоты, включая воздух; состояние, а не только имя блока.
fn profile(chunk: &Chunk, x: i32, z: i32) -> Vec<[i32; 3]> {
    let mut result = Vec::new();
    let mut start = MIN_Y;
    let mut state = chunk.block(x, start, z);
    for y in MIN_Y + 1..MIN_Y + WORLD_HEIGHT {
        let next = chunk.block(x, y, z);
        if next != state {
            result.push([start, y - 1, state]);
            start = y;
            state = next;
        }
    }
    result.push([start, MIN_Y + WORLD_HEIGHT - 1, state]);
    result
}

fn snapshot(stage: &str, chunk: &Chunk) -> serde_json::Value {
    let mut columns = Vec::new();
    let mut names = BTreeMap::new();
    for x in 0..CHUNK_SIZE { for z in 0..CHUNK_SIZE {
        let runs = profile(chunk, x, z);
        for run in &runs {
            names.insert(run[2], crate::blocks::block_at_state(run[2]).expect("известное состояние"));
        }
        columns.push(json!({"local": [x, z], "runs": runs}));
    } }
    let biomes: Vec<_> = chunk.biomes.iter().map(|id| {
        terrain::Biome::by_number(*id as usize).expect("известный биом").name()
    }).collect();
    json!({"stage": stage, "blockstates_sha256": state_hash(chunk),
        "biome_ids_sha256": biome_hash(chunk), "biome_ids": chunk.biomes, "biomes": biomes,
        "state_names": names, "columns": columns})
}

fn assert_identical(ordinary: &Chunk, traced: &Chunk) {
    // Сравнение каждого состояния, включая пустые секции; хеши — дополнительный контроль.
    for y in MIN_Y..MIN_Y + WORLD_HEIGHT {
        for z in 0..CHUNK_SIZE { for x in 0..CHUNK_SIZE {
            assert_eq!(ordinary.block(x, y, z), traced.block(x, y, z), "({x},{y},{z})");
        } }
    }
    assert_eq!(ordinary.biomes, traced.biomes);
    assert_eq!(state_hash(ordinary), state_hash(traced));
    assert_eq!(biome_hash(ordinary), biome_hash(traced));
}

#[test]
fn observer_reads_all_stages_without_changing_flat_chunk() {
    let ordinary = Chunk::generated(&Generator::Flat, 0, 0);
    let mut stages = Vec::new();
    let traced = Chunk::generated_observed(&Generator::Flat, 0, 0, |stage, chunk| {
        stages.push(stage);
        // Полное чтение внутри callback тоже не должно менять результат.
        let _ = (state_hash(chunk), biome_hash(chunk));
    });
    assert_eq!(stages, STAGES);
    assert_identical(&ordinary, &traced);
}

#[test]
fn profile_preserves_air_and_state_changes_on_same_block_name() {
    let mut chunk = Chunk::new();
    let water = crate::blocks::state_by_name("water").unwrap();
    chunk.put_generated(0, 0, 0, water, true);
    chunk.put_generated(0, 1, 0, water + 1, true);
    let runs = profile(&chunk, 0, 0);
    assert_eq!(runs, vec![[MIN_Y, -1, 0], [0, 0, water], [1, 1, water + 1], [2, MIN_Y + WORLD_HEIGHT - 1, 0]]);
    assert_eq!(crate::blocks::block_at_state(water), crate::blocks::block_at_state(water + 1));
}

#[test]
#[ignore]
fn worldgen_stage_trace_263_export() {
    assert_eq!(std::env::var("MCSA_A6_DENSITY").as_deref(), Ok("0"), "только обычный путь плотности");
    let path = std::env::var("MCSA_WORLDGEN_STAGE_OUTPUT").expect("MCSA_WORLDGEN_STAGE_OUTPUT");
    let file = fs::File::create(path).unwrap();
    let mut out = flate2::write::GzEncoder::new(file, flate2::Compression::fast());
    for seed in SEEDS {
        for (cx, cz) in CHUNKS {
            let generator = Generator::Normal(Terrain::new(seed, Style::Vanilla).with_preview(true));
            let ordinary = Chunk::generated(&generator, cx, cz);
            let mut stages = Vec::new();
            let mut rows = Vec::new();
            let traced = Chunk::generated_observed(&generator, cx, cz, |stage, chunk| {
                stages.push(stage);
                rows.push(snapshot(stage, chunk));
            });
            assert_eq!(stages, STAGES);
            assert_identical(&ordinary, &traced);
            let Generator::Normal(terrain) = &generator else { unreachable!() };
            let from_x = cx * CHUNK_SIZE - 1;
            let from_z = cz * CHUNK_SIZE - 1;
            let grid = terrain.relief_grid(from_x, from_z, from_x + CHUNK_SIZE + 1, from_z + CHUNK_SIZE + 1);
            let mut columns = Vec::new();
            for x in cx * CHUNK_SIZE..(cx + 1) * CHUNK_SIZE {
                for z in cz * CHUNK_SIZE..(cz + 1) * CHUNK_SIZE {
                    let climate = terrain.climate_at(x, z);
                    columns.push(json!({"column": [x, z], "grid_top": grid.top(x, z),
                        "relief_surface_cap_wobble_rugged": terrain.stage_trace_relief(x, z),
                        "climate_continent_erosion_weirdness_temperature_humidity": [climate.continent, climate.erosion,
                            climate.weirdness, climate.temperature, climate.humidity]}));
                }
            }
            serde_json::to_writer(&mut out, &json!({"schema": 1, "version": "26.3", "seed": seed,
                "chunk": [cx, cz], "density_router": false, "holdout": SEEDS[2..].contains(&seed),
                "identity": {"equal_blockstates": true, "equal_biomes": true,
                    "compared_blockstates": WORLD_HEIGHT * CHUNK_SIZE * CHUNK_SIZE,
                    "compared_biomes": ordinary.biomes.len(), "ordinary_blockstates_sha256": state_hash(&ordinary),
                    "ordinary_biome_ids_sha256": biome_hash(&ordinary)},
                "diagnostic_columns": columns, "stages": rows})).unwrap();
            writeln!(out).unwrap();
        }
    }
    out.finish().unwrap();
}
