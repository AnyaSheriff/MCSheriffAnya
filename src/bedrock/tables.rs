// Создано tools/make_bedrock_tables.py — не править руками.

/// Сколько состояний блоков у Bedrock 1.26.10.
pub const BLOCK_STATES: usize = 15846;

/// Номер биома Bedrock для каждого нашего: по порядку `Biome::ALL`, затем
/// свои (`CUSTOM_BIOMES`).
pub const BIOMES: [u32; 55] = [0, 24, 44, 45, 46, 47, 42, 43, 40, 14, 16, 26, 25, 7, 11, 6, 191, 1, 129, 12, 140, 4, 132, 27, 155, 29, 5, 30, 32, 160, 21, 23, 48, 35, 2, 186, 192, 36, 185, 184, 182, 183, 189, 3, 131, 34, 163, 37, 165, 38, 193, 187, 188, 190, 4];

/// Имена тех же биомов у Bedrock.
pub const BIOME_NAMES: [&str; 55] = ["ocean", "deep_ocean", "cold_ocean", "deep_cold_ocean", "frozen_ocean", "deep_frozen_ocean", "lukewarm_ocean", "deep_lukewarm_ocean", "warm_ocean", "mushroom_island", "beach", "cold_beach", "stone_beach", "river", "frozen_river", "swampland", "mangrove_swamp", "plains", "sunflower_plains", "ice_plains", "ice_plains_spikes", "forest", "flower_forest", "birch_forest", "birch_forest_mutated", "roofed_forest", "taiga", "cold_taiga", "mega_taiga", "redwood_taiga_mutated", "jungle", "jungle_edge", "bamboo_jungle", "savanna", "desert", "meadow", "cherry_grove", "savanna_plateau", "grove", "snowy_slopes", "jagged_peaks", "frozen_peaks", "stony_peaks", "extreme_hills", "extreme_hills_mutated", "extreme_hills_plus_trees", "savanna_mutated", "mesa", "mesa_bryce", "mesa_plateau_stone", "pale_garden", "lush_caves", "dripstone_caves", "deep_dark", "forest"];
