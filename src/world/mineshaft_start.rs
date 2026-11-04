/// minecraft.wiki/w/Mineshaft: «0.4% chance»; поток подтверждён
/// всеми 292 сохранёнными началами двух миров 26.3.
fn eligible(seed: i64, x: i32, z: i32) -> bool {
    super::template::StructureRandom::for_chunk(seed, x, z).double() < 0.004
}

// Поток раскладки кусков отделён от проверки начала шахты.
fn scramble(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn roll(seed: i64, x: i32, z: i32, salt: u64) -> u64 {
    scramble((seed as u64) ^ (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (z as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ salt)
}

fn eligible_for(terrain: &super::terrain::Terrain, x: i32, z: i32) -> bool {
    if terrain.generation_profile() & 2 != 0 { eligible(terrain.seed(), x, z) }
    else { roll(terrain.seed(), x, z, 0x5AF7_0000) % 1000 < 4 }
}

fn mesa_biome(biome: super::terrain::Biome) -> bool {
    use super::terrain::Biome;

    matches!(biome, Biome::Badlands | Biome::ErodedBadlands | Biome::WoodedBadlands)
}

/// Начинается ли шахта в чанке: (бесплодная ли она, высота поверхности).
pub fn start_at(terrain: &super::terrain::Terrain, chunk_x: i32, chunk_z: i32) -> Option<(bool, i32)> {
    if !eligible_for(terrain, chunk_x, chunk_z) {
        return None;
    }

    let column = terrain.column_at(chunk_x * 16 + 8, chunk_z * 16 + 8);

    // Глубокая тьма — пещерный биом: у оригинала проверка у старта в
    // толще [проверить: на какой высоте]; у нас — под серединой на y −40.
    if super::cave_biomes::biome_at(&column, -40) == Some(super::terrain::Biome::DeepDark) {
        return None;
    }

    Some((mesa_biome(column.biome), column.height))
}

