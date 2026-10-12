# Structure audit 2 — saved vanilla 26.1.2 versus our generator

Seed **100554032945340**. Reference: `~/.cache/mcsheriffanya/vanilla-struct/world`, read only. No servers, jars, game sources, mappings, datapacks or forbidden directories were used. This report measures the supplied world; it does **not** establish 1:1 generation parity.

## Coverage and interpretation

The nine core windows contain 49,152 chunks per dimension (12,582,912 block columns). Centre: chunk intervals [-64,64); other centres (gx×512,gz×512): ±32, upper exclusive. The 8-chunk halo expands these to ±72/±40: 71,936 chunks per dimension. Only start chunks in the core count toward rates. Halo starts and blocks supply spacing neighbours and bounding-box coverage. Every chunk intersecting a box is checked, including its interior; corner-only completeness checks are insufficient.

The reader validates region record extents, zlib and NBT, full status, coordinates, sections and heightmap presence. The supplied reference currently has **49,152 full core chunks and 71,936 full core+halo chunks in each dimension**, with no missing/non-full/truncated records detected. The reported OOM location (8,-524) is presently saved as full. The separate palette/section integrity check is retained in `integrity.json`; a prior failed save is not evidence that the final saved record is incomplete. No suspect record is silently substituted with air.

Counts are exact for these saved windows. Bracketed uncertainty is the 95% percentile interval of 4,000 window-cluster bootstrap resamples, in starts per million block columns, fixed seed 2612. It measures variability between these nine windows, not a multi-seed confidence claim. Zero observations give a degenerate bootstrap interval and do not prove impossibility (under an independent Poisson model, the zero-count 95% upper rate is 3/area). Same-seed terrain and biomes differ; a count difference alone does not justify changing spacing.

Nearest-neighbour (NN) distances use all measured halo starts of the same type; a core start contributes only if its nearest measured neighbour is closer than the halo boundary. Values show median blocks and number of uncensored starts. Raw coordinates, composition, rotations, professions/types, containers, spawners and ring probes are in [the evidence summary](structures-audit-2-data/summary.json) and compressed per-dimension vanilla JSON files.

## Counts, rates and spacing

Window order throughout: **(-1,-1), (-1,0), (-1,1), (0,-1), (0,0), (0,1), (1,-1), (1,0), (1,1)**. Counts are vanilla / ours before / ours after. `None` means unavailable.

| Dimension/type | Vanilla per window | Ours per window | Total V/B/A | 95% rate interval V / A | NN median (n), V / A |
|---|---|---|---|---|---|
| overworld/ancient_city | 0 0 0 0 0 0 1 0 0 | 2 2 0 0 0 0 0 4 0 | 1 / 8 / 8 | [0.0, 0.318] / [0.0, 1.483] | None (0) / 345.7 (4) |
| overworld/buried_treasure | 4 5 3 3 1 0 3 0 3 | 1 0 0 0 1 0 2 0 0 | 22 / 4 / 4 | [0.763, 3.179] / [0.0, 0.742] | 50.6 (14) / 137.64 (2) |
| overworld/desert_pyramid | 0 0 0 0 2 2 0 0 0 | 0 0 0 0 0 0 0 0 0 | 4 / 0 / 0 | [0.0, 0.636] / [0.0, 0.0] | None (0) / None (0) |
| overworld/igloo | 0 0 0 0 0 0 0 0 0 | 0 0 0 0 0 0 0 0 0 | 0 / 0 / 0 | [0.0, 0.0] / [0.0, 0.0] | None (0) / None (0) |
| overworld/jungle_pyramid | 0 0 0 0 0 0 0 0 0 | 0 0 0 0 0 0 0 0 0 | 0 / 0 / 0 | [0.0, 0.0] / [0.0, 0.0] | None (0) / None (0) |
| overworld/mansion | 0 0 0 0 0 0 0 0 0 | 0 0 0 0 0 0 0 0 0 | 0 / 0 / 0 | [0.0, 0.0] / [0.0, 0.0] | None (0) / None (0) |
| overworld/mineshaft | 20 13 22 27 54 11 11 10 23 | 9 15 12 15 60 13 9 16 25 | 191 / 174 / 174 | [12.504, 19.179] / [11.55, 16.292] | 121.85 (177) / 115.38 (160) |
| overworld/mineshaft_mesa | 0 0 0 0 2 4 0 0 0 | 0 0 1 0 0 0 0 0 0 | 6 / 1 / 1 | [0.0, 1.272] / [0.0, 0.318] | 107.33 (4) / None (0) |
| overworld/monument | 1 1 0 0 1 0 0 1 0 | 0 0 0 1 6 0 0 0 3 | 4 / 10 / 10 | [0.106, 0.556] / [0.106, 1.325] | None (0) / 544.0 (3) |
| overworld/ocean_ruin_cold | 4 8 0 0 0 0 0 10 0 | 0 1 0 1 18 0 6 3 8 | 22 / 37 / 37 | [0.0, 4.45] / [0.954, 4.36] | 208.61 (11) / 231.26 (24) |
| overworld/ocean_ruin_warm | 0 0 7 0 14 0 1 0 0 | 2 0 4 7 15 0 0 5 3 | 22 / 36 / 36 | [0.106, 3.02] / [1.378, 3.974] | 257.99 (13) / 252.98 (25) |
| overworld/pillager_outpost | 0 0 0 0 0 0 0 0 1 | 0 1 1 0 0 0 0 0 0 | 1 / 2 / 2 | [0.0, 0.318] / [0.0, 0.424] | None (0) / None (0) |
| overworld/ruined_portal | 0 0 0 2 2 1 3 0 4 | 0 1 0 1 0 3 0 0 1 | 12 / 6 / 6 | [0.318, 1.907] / [0.079, 1.166] | None (0) / None (0) |
| overworld/ruined_portal_desert | 0 0 0 0 3 2 0 0 0 | 0 0 0 0 0 0 0 0 0 | 5 / 0 / 0 | [0.0, 0.715] / [0.0, 0.0] | 794.86 (1) / None (0) |
| overworld/ruined_portal_jungle | 0 0 1 0 1 0 0 0 0 | 1 1 1 0 0 0 0 0 0 | 2 / 3 / 3 | [0.0, 0.318] / [0.0, 0.556] | None (0) / None (0) |
| overworld/ruined_portal_mountain | 0 0 0 0 1 1 0 0 0 | 0 0 1 0 0 0 0 1 0 | 2 / 2 / 2 | [0.0, 0.318] / [0.0, 0.424] | None (0) / None (0) |
| overworld/ruined_portal_ocean | 1 2 2 1 4 0 0 4 0 | 1 0 1 2 12 0 1 1 3 | 14 / 21 / 21 | [0.556, 1.801] / [0.636, 2.361] | 446.28 (3) / 426.93 (6) |
| overworld/ruined_portal_swamp | 0 0 0 0 0 0 0 0 0 | 0 0 0 0 1 1 0 0 0 | 0 / 2 / 2 | [0.0, 0.0] / [0.0, 0.318] | None (0) / None (0) |
| overworld/shipwreck | 4 4 6 1 8 0 1 10 2 | 1 1 2 6 23 0 6 5 7 | 36 / 51 / 51 | [1.669, 4.768] / [2.225, 5.298] | 208.0 (22) / 213.47 (36) |
| overworld/shipwreck_beached | 0 0 1 0 0 0 0 0 1 | 0 0 0 0 0 0 0 0 0 | 2 / 0 / 0 | [0.0, 0.424] / [0.0, 0.0] | None (0) / None (0) |
| overworld/stronghold | 0 1 0 0 0 0 0 0 0 | 0 1 1 0 0 0 0 1 0 | 1 / 3 / 3 | [0.0, 0.318] / [0.0, 0.53] | None (0) / None (0) |
| overworld/swamp_hut | 0 0 0 0 0 0 0 0 0 | 0 0 0 0 0 1 0 0 0 | 0 / 1 / 1 | [0.0, 0.0] / [0.0, 0.318] | None (0) / None (0) |
| overworld/trail_ruins | 0 0 0 0 1 0 1 0 1 | 0 0 0 0 1 1 0 0 0 | 3 / 2 / 2 | [0.079, 0.477] / [0.0, 0.318] | None (0) / 329.85 (1) |
| overworld/trial_chambers | 4 4 4 4 15 3 3 4 4 | 3 4 4 4 16 3 4 3 4 | 45 / 45 / 45 | [3.338, 3.735] / [3.285, 3.815] | 374.89 (9) / 364.86 (19) |
| overworld/village_desert | 0 0 0 0 4 2 0 0 0 | 0 0 0 0 0 0 0 0 0 | 6 / 0 / 0 | [0.0, 0.848] / [0.0, 0.0] | 528.0 (1) / None (0) |
| overworld/village_plains | 0 0 0 2 0 0 0 0 2 | 1 0 0 0 1 0 0 1 0 | 4 / 3 / 3 | [0.0, 0.848] / [0.079, 0.445] | 352.36 (1) / None (0) |
| overworld/village_savanna | 0 0 0 0 3 0 1 0 0 | 0 0 0 0 0 0 0 0 0 | 4 / 0 / 0 | [0.0, 0.572] / [0.0, 0.0] | 395.16 (2) / None (0) |
| overworld/village_snowy | 0 0 0 0 0 0 0 0 0 | 0 0 0 0 0 0 0 0 0 | 0 / 0 / 0 | [0.0, 0.0] / [0.0, 0.0] | None (0) / None (0) |
| overworld/village_taiga | 0 0 0 0 0 0 0 0 1 | 0 0 0 0 0 2 0 0 0 | 1 / 0 / 2 | [0.0, 0.318] / [0.0, 0.636] | None (0) / None (0) |
| the_nether/bastion_remnant | 5 6 1 2 10 1 4 3 3 | 0 3 4 1 12 1 7 2 2 | 35 / 32 / 32 | [2.013, 3.815] / [1.483, 3.603] | 240.53 (15) / 297.62 (18) |
| the_nether/fortress | 1 0 4 3 13 4 1 2 2 | 4 1 2 4 10 3 0 2 2 | 30 / 28 / 28 | [1.378, 3.02] / [1.483, 2.797] | 302.52 (18) / 415.08 (13) |
| the_nether/nether_fossil | 19 22 45 66 385 45 100 23 88 | 0 0 0 0 0 0 0 0 0 | 793 / 0 / 0 | [36.028, 81.327] / [0.0, 0.0] | 32.0 (792) / None (0) |
| the_nether/ruined_portal_nether | 1 2 3 3 11 4 3 4 4 | 2 2 3 3 13 4 1 2 4 | 35 / 34 / 34 | [2.305, 3.285] / [2.013, 3.115] | 449.14 (7) / 426.93 (6) |
| the_end/end_city | 0 1 0 1 0 1 1 0 0 | 0 1 2 2 3 1 1 0 3 | 4 / 13 / 13 | [0.064, 0.742] / [0.636, 1.669] | None (0) / 419.98 (1) |

## Biomes at starts

Vanilla samples the start-chunk centre at first-piece minimum Y. Our existing marker metadata samples the surface biome (Nether/End use their dimension biome map). These are observations, not equivalent 3D biome predicates. The saved biome-exposure counts at Y64/Y32 are in the summary; underground acceptance needs a separate 3D exposure denominator.

| Dimension/type | Vanilla start biomes | Ours marker biomes |
|---|---|---|
| overworld/ancient_city | deep_dark:1 | frozen_peaks:2, grove:3, snowy_slopes:1, stony_peaks:2 |
| overworld/buried_treasure | beach:19, snowy_beach:3 | beach:2, snowy_beach:2 |
| overworld/desert_pyramid | desert:4 | — |
| overworld/igloo | — | — |
| overworld/jungle_pyramid | — | — |
| overworld/mansion | — | — |
| overworld/mineshaft | beach:4, birch_forest:5, cherry_grove:1, cold_ocean:7, deep_cold_ocean:3, deep_frozen_ocean:3, deep_lukewarm_ocean:1, deep_ocean:3, desert:24, dripstone_caves:7, flower_forest:1, forest:22, frozen_ocean:12, jungle:1, lukewarm_ocean:15, lush_caves:5, ocean:7, old_growth_birch_forest:1, plains:12, river:14, savanna:8, snowy_taiga:1, sparse_jungle:1, sunflower_plains:1, taiga:5, warm_ocean:24, windswept_savanna:1, wooded_badlands:2 | bamboo_jungle:2, birch_forest:4, cold_ocean:9, dark_forest:4, deep_cold_ocean:12, deep_frozen_ocean:18, deep_lukewarm_ocean:10, deep_ocean:5, flower_forest:1, forest:6, frozen_ocean:8, jungle:5, lukewarm_ocean:21, meadow:1, ocean:5, old_growth_birch_forest:2, old_growth_pine_taiga:1, old_growth_spruce_taiga:3, plains:5, river:1, savanna:3, snowy_plains:2, snowy_taiga:1, sparse_jungle:4, stony_shore:1, swamp:7, taiga:4, warm_ocean:29 |
| overworld/mineshaft_mesa | badlands:2, eroded_badlands:1, wooded_badlands:3 | wooded_badlands:1 |
| overworld/monument | deep_cold_ocean:1, deep_frozen_ocean:1, deep_lukewarm_ocean:1, deep_ocean:1 | deep_cold_ocean:3, deep_frozen_ocean:3, deep_lukewarm_ocean:4 |
| overworld/ocean_ruin_cold | cold_ocean:3, deep_cold_ocean:4, deep_frozen_ocean:2, deep_ocean:3, frozen_ocean:9, ocean:1 | cold_ocean:7, deep_cold_ocean:6, deep_frozen_ocean:10, deep_ocean:6, frozen_ocean:7, ocean:1 |
| overworld/ocean_ruin_warm | deep_lukewarm_ocean:2, lukewarm_ocean:12, warm_ocean:8 | deep_lukewarm_ocean:5, lukewarm_ocean:10, warm_ocean:21 |
| overworld/pillager_outpost | plains:1 | grove:1, plains:1 |
| overworld/ruined_portal | dripstone_caves:2, eroded_badlands:1, flower_forest:1, forest:2, lush_caves:1, river:4, savanna:1 | forest:1, grove:2, old_growth_birch_forest:1, old_growth_spruce_taiga:1, plains:1 |
| overworld/ruined_portal_desert | desert:5 | — |
| overworld/ruined_portal_jungle | jungle:1, sparse_jungle:1 | bamboo_jungle:1, sparse_jungle:2 |
| overworld/ruined_portal_mountain | badlands:2 | frozen_peaks:1, wooded_badlands:1 |
| overworld/ruined_portal_ocean | beach:1, deep_cold_ocean:3, deep_ocean:1, frozen_ocean:3, lukewarm_ocean:1, ocean:1, warm_ocean:4 | cold_ocean:2, deep_cold_ocean:2, deep_frozen_ocean:4, deep_ocean:2, frozen_ocean:1, lukewarm_ocean:2, warm_ocean:8 |
| overworld/ruined_portal_swamp | — | swamp:2 |
| overworld/shipwreck | cold_ocean:3, deep_cold_ocean:4, deep_frozen_ocean:1, deep_lukewarm_ocean:2, deep_ocean:1, frozen_ocean:8, lukewarm_ocean:7, ocean:4, warm_ocean:6 | cold_ocean:6, deep_cold_ocean:5, deep_frozen_ocean:8, deep_lukewarm_ocean:3, deep_ocean:2, frozen_ocean:4, lukewarm_ocean:7, ocean:5, warm_ocean:11 |
| overworld/shipwreck_beached | beach:2 | — |
| overworld/stronghold | deep_frozen_ocean:1 | birch_forest:1, dark_forest:1, frozen_peaks:1 |
| overworld/swamp_hut | — | swamp:1 |
| overworld/trail_ruins | jungle:1, old_growth_birch_forest:1, old_growth_spruce_taiga:1 | old_growth_spruce_taiga:2 |
| overworld/trial_chambers | cold_ocean:4, deep_cold_ocean:2, deep_frozen_ocean:1, deep_lukewarm_ocean:1, desert:9, dripstone_caves:1, flower_forest:1, forest:4, frozen_ocean:2, jungle:1, lukewarm_ocean:5, ocean:3, old_growth_pine_taiga:1, plains:1, river:2, savanna:1, snowy_beach:1, stony_shore:1, warm_ocean:3, windswept_savanna:1 | badlands:1, birch_forest:1, dark_forest:1, deep_cold_ocean:2, deep_frozen_ocean:5, deep_lukewarm_ocean:2, deep_ocean:1, frozen_ocean:4, jungle:2, lukewarm_ocean:4, ocean:2, old_growth_pine_taiga:1, plains:2, river:1, snowy_taiga:1, sparse_jungle:2, swamp:2, warm_ocean:8, windswept_forest:1, wooded_badlands:2 |
| overworld/village_desert | desert:6 | — |
| overworld/village_plains | plains:4 | plains:3 |
| overworld/village_savanna | lukewarm_ocean:1, savanna:3 | — |
| overworld/village_snowy | — | — |
| overworld/village_taiga | taiga:1 | taiga:2 |
| the_nether/bastion_remnant | crimson_forest:12, nether_wastes:17, soul_sand_valley:4, warped_forest:2 | crimson_forest:3, nether_wastes:11, soul_sand_valley:14, warped_forest:4 |
| the_nether/fortress | basalt_deltas:9, crimson_forest:7, nether_wastes:7, soul_sand_valley:4, warped_forest:3 | basalt_deltas:8, crimson_forest:6, nether_wastes:9, soul_sand_valley:5 |
| the_nether/nether_fossil | basalt_deltas:4, crimson_forest:5, nether_wastes:13, soul_sand_valley:771 | — |
| the_nether/ruined_portal_nether | basalt_deltas:8, crimson_forest:7, nether_wastes:11, soul_sand_valley:6, warped_forest:3 | basalt_deltas:4, crimson_forest:5, nether_wastes:13, soul_sand_valley:11, warped_forest:1 |
| the_end/end_city | end_highlands:4 | end_highlands:13 |

## Placement, support and terrain adaptation

Datum is the piece BB minimum Y plus saved ground_level_delta on the vanilla jigsaw side; our datum is the exported piece minimum Y. It is **not universally a walkable floor**. Depth uses final OCEAN_FLOOR on vanilla and highest collision-bearing block on ours, at the piece centre. Both can include the structure itself, trees and roofs. Positive depth denotes datum below that final top. Nether heightmaps see the roof; they do not measure the local cavern floor. Sea reference is Y63 Overworld, Y32 Nether; End has no sea. Raw per-piece sea deltas are in vanilla evidence.

Terrain-matching pieces are excluded from datum summaries on both sides. Our roads and hay features store a deferred Y=0, not a world elevation, and are excluded from vertical bounds; village box Y spans rigid pieces only. Nine BB-datum probes (corners, edges and centre; duplicates removed) retain the actual block and block immediately below. Air counts are diagnostic, not a floating-piece verdict: bridges, ruined pieces, entrances and empty BB margins intentionally contain air. Complete vanilla boxes are used; pieces with missing bounds are excluded from geometry inventories.

| Dimension/type | Complete V starts | Datum Y min/median/max V / A | Depth min/median/max V / A | Air below datum probes V / A |
|---|---|---|---|---|
| overworld/ancient_city | 1/1 | -51/-51/-51 / -52/-52/-52 | 85/155/177 / 183/190/203 | 4/721 / 1/72 |
| overworld/buried_treasure | 22/22 | 48/54.5/66 / 59/61.5/63 | 1/2/7 / 1/1/3 | 0/22 / 0/4 |
| overworld/desert_pyramid | 4/4 | 62/66.5/89 / unavailable | 0/0/27 / unavailable | 4/36 / 0/0 |
| overworld/igloo | 0/0 | unavailable / unavailable | unavailable / unavailable | 0/0 / 0/0 |
| overworld/jungle_pyramid | 0/0 | unavailable / unavailable | unavailable / unavailable | 0/0 / 0/0 |
| overworld/mansion | 0/0 | unavailable / unavailable | unavailable / unavailable | 0/0 / 0/0 |
| overworld/mineshaft | 191/191 | -62/-1/50 / -62/-3/49 | -30/61/165 / -11/58/173 | 3493/209961 / 19314/203580 |
| overworld/mineshaft_mesa | 6/6 | 49/70/99 / 88/99/106 | -42/8/82 / -35/12/31 | 1560/6723 / 407/1341 |
| overworld/monument | 4/4 | 39/39/39 / 39/39/39 | 16/16/16 / 16/16/16 | 0/36 / 0/90 |
| overworld/ocean_ruin_cold | 22/22 | 29/47/53 / 33/43.5/76 | -2/5/26 / -2/3/33 | 3/1296 / 10/684 |
| overworld/ocean_ruin_warm | 22/22 | 17/48/60 / 33/42.5/66 | -1/4/31 / -1/4/11 | 0/414 / 13/720 |
| overworld/pillager_outpost | 1/1 | 69/72/74 / 57/79/110 | -1/3/19 / -1/3/20 | 0/72 / 0/81 |
| overworld/ruined_portal | 12/12 | -20/50.5/64 / -39/3.5/139 | 0/23.5/98 / 2/77/125 | 2/108 / 0/54 |
| overworld/ruined_portal_desert | 5/5 | 59/61/78 / unavailable | 0/6/6 / unavailable | 2/45 / 0/0 |
| overworld/ruined_portal_jungle | 2/2 | 93/93.5/94 / 64/66/73 | 0/3/6 / 1/3/8 | 0/18 / 0/27 |
| overworld/ruined_portal_mountain | 2/2 | 62/78.5/95 / 71/90/109 | 0/1.5/3 / 11/20/29 | 0/18 / 0/18 |
| overworld/ruined_portal_ocean | 14/14 | 36/45.5/52 / 32/44/61 | 1/7.5/26 / 1/6/14 | 0/126 / 0/189 |
| overworld/ruined_portal_swamp | 0/0 | unavailable / 59/60/61 | unavailable / 2/6/10 | 0/0 / 0/18 |
| overworld/shipwreck | 36/36 | 36/47.5/67 / 33/43/63 | -1/6/26 / 0/5/40 | 6/324 / 1/459 |
| overworld/shipwreck_beached | 2/2 | 59/60/61 / unavailable | 8/8.5/9 / unavailable | 0/18 / 0/0 |
| overworld/stronghold | 1/1 | -60/-52/-38 / -27/-7/9 | 71/91/110 / 45/94/191 | 12/1707 / 585/2709 |
| overworld/swamp_hut | 0/0 | unavailable / 65/65/65 | unavailable / 4/4/4 | 0/0 / 3/9 |
| overworld/trail_ruins | 3/3 | 49/59/60 / 46/53/71 | 8/18.5/61 / 2/29/48 | 36/483 / 0/333 |
| overworld/trial_chambers | 45/45 | -40/-33/-21 / -54/-40/-24 | 45/87/146 / 62/94/156 | 6504/63148 / 42/405 |
| overworld/village_desert | 6/6 | 63/69/94 / unavailable | -4/2/14 / unavailable | 8/965 / 0/0 |
| overworld/village_plains | 4/4 | 63/64.5/102 / 55/69/105 | -3/4/11 / -1/5/11 | 8/918 / 0/666 |
| overworld/village_savanna | 4/4 | 63/70/120 / unavailable | -1/4/9 / unavailable | 0/598 / 0/0 |
| overworld/village_snowy | 0/0 | unavailable / unavailable | unavailable / unavailable | 0/0 / 0/0 |
| overworld/village_taiga | 1/1 | 64/67/72 / 53/71/80 | -1/4/10 / 0/6/19 | 5/208 / 0/370 |
| the_nether/bastion_remnant | 35/35 | 33/33/33 / 29/50/85 | 94/94/94 / 42/77/98 | 2468/17770 / 3841/12982 |
| the_nether/fortress | 30/30 | 48/59/81 / 29/62/87 | 46/68/79 / 40/65/98 | 3445/31563 / 3710/31563 |
| the_nether/nether_fossil | 793/793 | 33/61/111 / unavailable | 16/66/94 / unavailable | 443/5451 / 0/0 |
| the_nether/ruined_portal_nether | 35/35 | 27/44/93 / 28/39/100 | 34/83/100 / 27/88/99 | 5/315 / 1/306 |
| the_end/end_city | 4/4 | 60/109/161 / 60/109/242 | 0/13/56 / -6/16/87 | 1766/1998 / 4329/5064 |

Four side-midpoint probes at distances **1,4,8,12,16** from each non-terrain-matching piece give median final top minus datum. They may intersect neighbouring pieces; these profiles are descriptive. They cannot isolate beard_thin / beard_box / bury / encapsulate density changes without matching unmodified terrain. Our village/outpost cubic skirt differs in implementation; ancient-city, stronghold, trial-chamber and trail-ruin dedicated density adaptation remains absent. No ground was added solely because a BB probe found air.

| Dimension/type | Vanilla ring profile | Ours ring profile |
|---|---|---|
| overworld/ancient_city | [153.0, 154.0, 154.5, 154.0, 154.0] | [165.0, 164.0, 160.5, 160.0, 153.5] |
| overworld/buried_treasure | [2.0, 2.0, 3.0, 2.5, 2.5] | [1.0, 2.0, 2.0, 0.5, 0.5] |
| overworld/desert_pyramid | [3.0, 3.0, 3.0, 3.5, 4.0] | [None, None, None, None, None] |
| overworld/igloo | [None, None, None, None, None] | [None, None, None, None, None] |
| overworld/jungle_pyramid | [None, None, None, None, None] | [None, None, None, None, None] |
| overworld/mansion | [None, None, None, None, None] | [None, None, None, None, None] |
| overworld/mineshaft | [61.0, 61.0, 61.0, 61.0, 61.0] | [58.0, 59.0, 59.0, 59.0, 59.0] |
| overworld/mineshaft_mesa | [8.0, 8.0, 8.0, 8.0, 7.0] | [12.0, 12.0, 12.0, 11.0, 10.0] |
| overworld/monument | [-1.5, -2.0, -2.0, -2.0, -2.0] | [-2.0, -1.0, -1.0, -1.0, -1.0] |
| overworld/ocean_ruin_cold | [-1.0, -1.0, 0.0, 0.0, 0.0] | [-1.0, -1.0, -1.0, -1.0, -1.0] |
| overworld/ocean_ruin_warm | [-1.0, -1.0, -1.0, -1.0, -0.5] | [-1.0, -1.0, -1.0, 0.0, -0.5] |
| overworld/pillager_outpost | [-1.0, -1.0, -1.0, -1.0, -1.0] | [-1.0, 0.5, 0.0, 0.0, 0.0] |
| overworld/ruined_portal | [22.5, 12.0, 14.0, 16.0, 18.5] | [82.0, 83.5, 78.0, 78.5, 74.5] |
| overworld/ruined_portal_desert | [1.5, 1.5, 1.0, 1.0, 1.0] | [None, None, None, None, None] |
| overworld/ruined_portal_jungle | [0.5, 2.0, 4.0, 8.0, 5.5] | [2.0, 1.0, 7.5, 2.5, 3.5] |
| overworld/ruined_portal_mountain | [0.0, 0.0, 0.0, 0.0, 0.0] | [14.0, 17.0, 17.5, 16.0, 5.0] |
| overworld/ruined_portal_ocean | [1.0, 1.0, 1.0, 1.0, 1.0] | [1.0, 1.0, 1.0, 1.0, 1.0] |
| overworld/ruined_portal_swamp | [None, None, None, None, None] | [0.5, 1.0, 2.5, 2.0, 3.0] |
| overworld/shipwreck | [0.0, 0.0, 0.0, 0.0, 0.0] | [0.0, 0.0, 0.0, 0.0, 0.0] |
| overworld/shipwreck_beached | [3.5, 3.5, 1.5, 0.5, 1.5] | [None, None, None, None, None] |
| overworld/stronghold | [91.0, 91.0, 91.0, 91.0, 91.0] | [94.0, 94.0, 94.0, 95.0, 95.0] |
| overworld/swamp_hut | [None, None, None, None, None] | [1.5, 2.5, 3.5, 1.5, 3.0] |
| overworld/trail_ruins | [19.0, 18.0, 17.0, 17.0, 18.0] | [28.0, 28.0, 28.0, 28.0, 26.0] |
| overworld/trial_chambers | [87.0, 87.0, 87.0, 87.0, 87.0] | [96.0, 95.0, 94.0, 94.0, 94.0] |
| overworld/village_desert | [-1.0, -1.0, -1.0, -1.0, -1.0] | [None, None, None, None, None] |
| overworld/village_plains | [-1.0, -1.0, -1.0, -1.0, -1.0] | [-1.0, -1.0, 0.0, 0.0, 1.0] |
| overworld/village_savanna | [-1.0, -1.0, -1.0, -1.0, -1.0] | [None, None, None, None, None] |
| overworld/village_snowy | [None, None, None, None, None] | [None, None, None, None, None] |
| overworld/village_taiga | [-1.0, 1.0, 1.0, 0.5, 1.0] | [4.0, 4.0, 5.0, 5.0, 4.0] |
| the_nether/bastion_remnant | [94.0, 94.0, 94.0, 94.0, 94.0] | [77.0, 77.0, 77.0, 77.0, 77.0] |
| the_nether/fortress | [68.0, 68.0, 68.0, 68.0, 68.0] | [65.0, 65.0, 65.0, 65.0, 65.0] |
| the_nether/nether_fossil | [66.0, 66.0, 66.0, 66.0, 66.0] | [None, None, None, None, None] |
| the_nether/ruined_portal_nether | [83.0, 83.0, 83.0, 83.0, 83.0] | [88.0, 88.0, 88.0, 88.0, 88.0] |
| the_end/end_city | [12.0, 0.0, -12.0, -13.5, -11.0] | [12, 1.0, -10.5, -12, -14.0] |

Chunk-border completeness is a coverage check, not proof of correct painting. Procedural branch pruning and intentional ruin gaps cannot be classified as cut pieces from final bounding boxes alone. A production-mask comparison shares search-radius mistakes. The independent complete-plan comparisons below cover five families; other families still need that check before a clipping verdict.

## Composition

Piece counts include vanilla metadata children, including entity/feature markers. Our snapshot-based ancient cities and trial chambers do not retain an equivalent jigsaw decomposition. Whole snapshot boxes must not be compared as single vanilla pieces. Type and rotation histograms are preserved in the JSON; numeric orientation conventions are not assumed identical.

| Dimension/type | Pieces min/median/max V / A | Median total box X/Y/Z V / A | Distinct piece names V / A |
|---|---|---|---|
| overworld/ancient_city | 97/97/97 / unavailable | 219/31/218 / 216.0/31.0/224.0 | 36 / 1 |
| overworld/buried_treasure | 1/1/1 / 1/1/1 | 1.0/1.0/1.0 / 1.0/1.0/1.0 | 1 / 1 |
| overworld/desert_pyramid | 1/1/1 / unavailable | 21.0/15.0/21.0 / None/None/None | 1 / 0 |
| overworld/igloo | unavailable / unavailable | None/None/None / None/None/None | 0 / 0 |
| overworld/jungle_pyramid | unavailable / unavailable | None/None/None / None/None/None | 0 / 0 |
| overworld/mansion | unavailable / unavailable | None/None/None / None/None/None | 0 / 0 |
| overworld/mineshaft | 36/121/319 / 41/125/259 | 128/22/148 / 132.5/22.0/139.0 | 4 / 4 |
| overworld/mineshaft_mesa | 54/122.5/193 / 149/149/149 | 129.0/22.0/137.5 / 136/22/153 | 4 / 4 |
| overworld/monument | 1/1/1 / unavailable | 58.0/23.0/58.0 / 58.0/23.0/58.0 | 1 / 1 |
| overworld/ocean_ruin_cold | 3/3/24 / 1/1/9 | 7.0/7.0/6.0 / 7/7/7 | 30 / 12 |
| overworld/ocean_ruin_warm | 1/1/9 / 1/1/9 | 6.0/7.0/7.0 / 7.0/7.0/7.0 | 10 / 12 |
| overworld/pillager_outpost | 11/11/11 / 4/4.5/5 | 48/32/48 / 32.5/39.5/29.0 | 8 / 5 |
| overworld/ruined_portal | 1/1/1 / 1/1/1 | 9.0/9.0/8.5 / 7.0/8.0/9.5 | 8 / 4 |
| overworld/ruined_portal_desert | 1/1/1 / unavailable | 10/9/7 / None/None/None | 4 / 0 |
| overworld/ruined_portal_jungle | 1/1/1 / 1/1/1 | 9.5/7.5/7.5 / 8/9/9 | 2 / 3 |
| overworld/ruined_portal_mountain | 1/1/1 / 1/1/1 | 8.5/8.0/9.0 / 11.5/8.5/9.5 | 2 / 2 |
| overworld/ruined_portal_ocean | 1/1/1 / 1/1/1 | 8.0/9.0/9.0 / 9/9/9 | 8 / 10 |
| overworld/ruined_portal_swamp | unavailable / 1/1/1 | None/None/None / 10.5/9.5/9.5 | 0 / 2 |
| overworld/shipwreck | 1/1/1 / 1/1/1 | 17.0/9.0/9.0 / 16/9/9 | 16 / 19 |
| overworld/shipwreck_beached | 1/1/1 / unavailable | 16.5/9.0/12.5 / None/None/None | 2 / 0 |
| overworld/stronghold | 195/195/195 / 89/97/118 | 149/33/128 / 111/27/111 | 13 / 13 |
| overworld/swamp_hut | unavailable / 1/1/1 | None/None/None / 7/7/9 | 0 / 1 |
| overworld/trail_ruins | 18/18/18 / 17/18.5/20 | 25/20/50 / 24.5/23.5/51.0 | 29 / 24 |
| overworld/trial_chambers | 177/252/313 / unavailable | 107/51/105 / 104/51/97 | 182 / 1 |
| overworld/village_desert | 58/80.5/126 / unavailable | 99.0/22.5/75.0 / None/None/None | 58 / 0 |
| overworld/village_plains | 90/125.5/134 / 110/110/113 | 144.5/19.5/122.0 / 146/31/125 | 57 / 39 |
| overworld/village_savanna | 68/89/156 / unavailable | 111.5/32.5/133.5 / None/None/None | 61 / 0 |
| overworld/village_snowy | unavailable / unavailable | None/None/None / None/None/None | 0 / 0 |
| overworld/village_taiga | 108/108/108 / 85/85/85 | 130/25/125 / 130.0/25.5/125.0 | 37 / 32 |
| the_nether/bastion_remnant | 43/89/183 / 43/92.5/180 | 56/56/50 / 50.0/65.0/56.0 | 163 / 158 |
| the_nether/fortress | 44/127.5/187 / 48/131/205 | 196.0/32.0/191.5 / 171.0/30.0/188.0 | 14 / 14 |
| the_nether/nether_fossil | 1/1/1 / unavailable | 4/5/4 / None/None/None | 14 / 0 |
| the_nether/ruined_portal_nether | 1/1/1 / 1/1/1 | 9/9/9 / 9.0/9.0/9.0 | 11 / 11 |
| the_end/end_city | 9/54/118 / 12/29/118 | 59.0/79.0/63.5 / 61/81/46 | 18 / 19 |

| Dimension/type | Saved vanilla rotations | Ours rotations (quarters or facing code) |
|---|---|---|
| overworld/ancient_city | CLOCKWISE_180:19, CLOCKWISE_90:34, COUNTERCLOCKWISE_90:23, NONE:21 | 0:4, 1:1, 2:2, 3:1 |
| overworld/buried_treasure | -1:22 | None:4 |
| overworld/desert_pyramid | 0:1, 1:2, 3:1 | — |
| overworld/igloo | — | — |
| overworld/jungle_pyramid | — | — |
| overworld/mansion | — | — |
| overworld/mineshaft | -1:4728, 0:4743, 1:4451, 2:4763, 3:4644 | 0:5865, 1:5589, 2:5633, 3:5533 |
| overworld/mineshaft_mesa | -1:143, 0:175, 1:146, 2:129, 3:154 | 0:40, 1:35, 2:35, 3:39 |
| overworld/monument | 0:1, 3:3 | 0:4, 1:1, 2:4, 3:1 |
| overworld/ocean_ruin_cold | CLOCKWISE_180:24, CLOCKWISE_90:30, COUNTERCLOCKWISE_90:63, NONE:27 | 0:15, 1:16, 2:24, 3:21 |
| overworld/ocean_ruin_warm | CLOCKWISE_180:11, CLOCKWISE_90:5, COUNTERCLOCKWISE_90:12, NONE:18 | 0:20, 1:14, 2:30, 3:16 |
| overworld/pillager_outpost | CLOCKWISE_180:3, CLOCKWISE_90:4, COUNTERCLOCKWISE_90:2, NONE:2 | 0:3, 1:3, 2:1, 3:2 |
| overworld/ruined_portal | 2:12 | 0:3, 3:3 |
| overworld/ruined_portal_desert | 2:5 | — |
| overworld/ruined_portal_jungle | 2:2 | 0:1, 2:1, 3:1 |
| overworld/ruined_portal_mountain | 2:2 | 0:1, 3:1 |
| overworld/ruined_portal_ocean | 2:14 | 0:3, 1:7, 2:7, 3:4 |
| overworld/ruined_portal_swamp | — | 0:1, 1:1 |
| overworld/shipwreck | CLOCKWISE_180:6, CLOCKWISE_90:13, COUNTERCLOCKWISE_90:10, NONE:7 | 0:14, 1:14, 2:11, 3:12 |
| overworld/shipwreck_beached | COUNTERCLOCKWISE_90:1, NONE:1 | — |
| overworld/stronghold | 0:53, 1:46, 2:44, 3:52 | None:304 |
| overworld/swamp_hut | — | 0:1 |
| overworld/trail_ruins | CLOCKWISE_180:3, CLOCKWISE_90:15, COUNTERCLOCKWISE_90:19, NONE:17 | 0:3, 1:16, 2:3, 3:15 |
| overworld/trial_chambers | CLOCKWISE_180:2965, CLOCKWISE_90:2829, COUNTERCLOCKWISE_90:2803, NONE:2694 | 0:11, 1:9, 2:10, 3:15 |
| overworld/village_desert | CLOCKWISE_180:141, CLOCKWISE_90:105, COUNTERCLOCKWISE_90:114, NONE:143 | — |
| overworld/village_plains | CLOCKWISE_180:118, CLOCKWISE_90:119, COUNTERCLOCKWISE_90:130, NONE:108 | 0:71, 1:102, 2:71, 3:89 |
| overworld/village_savanna | CLOCKWISE_180:105, CLOCKWISE_90:102, COUNTERCLOCKWISE_90:85, NONE:110 | — |
| overworld/village_snowy | — | — |
| overworld/village_taiga | CLOCKWISE_180:22, CLOCKWISE_90:27, COUNTERCLOCKWISE_90:27, NONE:32 | 0:43, 1:42, 2:43, 3:42 |
| the_nether/bastion_remnant | CLOCKWISE_180:960, CLOCKWISE_90:835, COUNTERCLOCKWISE_90:869, NONE:988 | 0:727, 1:705, 2:695, 3:700 |
| the_nether/fortress | 0:884, 1:885, 2:810, 3:928 | 0:831, 1:812, 2:946, 3:918 |
| the_nether/nether_fossil | CLOCKWISE_180:200, CLOCKWISE_90:194, COUNTERCLOCKWISE_90:173, NONE:226 | — |
| the_nether/ruined_portal_nether | 2:35 | 0:9, 1:8, 2:8, 3:9 |
| the_end/end_city | CLOCKWISE_180:49, CLOCKWISE_90:69, COUNTERCLOCKWISE_90:41, NONE:76 | 0:135, 1:168, 2:126, 3:174 |

## Containers, loot tables and spawners

Vanilla rows retain compatible literal LootTable identifiers in complete core-start boxes, including chest-minecart entity loot. Other families inside the same box (e.g. dungeon loot below a village) are kept in raw evidence but excluded here. Pyramid treasure and archaeology below the saved BB are included using their table IDs and horizontal footprint: at chunk (-63,22), the saved BB begins at Y67 but four treasure chests are at Y56. Spawner rows likewise filter compatible species; all spatial overlaps remain in the evidence. Overlapping starts of the same family can still share spatial records; this is measured association, not provenance from generator code. Our containers use eager item slots and have no persisted LootTable identifier; absence of a field is separate from a chest lacking initialization. Physical chest/barrel totals, initialized containers and uninitialized positions are in the summary.

| Dimension/type | Vanilla loot tables (count) | Ours containers / uninitialized chest+barrel blocks | Spawner species V / A |
|---|---|---|---|
| overworld/ancient_city | chests/ancient_city:27, chests/ancient_city_ice_box:1 | 189 / 15 (bounds 8/8) | — / spider:2, zombie:5 |
| overworld/buried_treasure | chests/buried_treasure:22 | 4 / 0 (bounds 4/4) | — / — |
| overworld/desert_pyramid | archaeology/desert_pyramid:28, chests/desert_pyramid:16 | 0 / 0 (bounds 0/0) | — / — |
| overworld/igloo | — | 0 / 0 (bounds 0/0) | — / — |
| overworld/jungle_pyramid | — | 0 / 0 (bounds 0/0) | — / — |
| overworld/mansion | — | 0 / 0 (bounds 0/0) | — / — |
| overworld/mineshaft | chests/abandoned_mineshaft:673 | 2 / 164 (bounds 130/174) | cave_spider:441 / cave_spider:356, skeleton:10, spider:9, zombie:22 |
| overworld/mineshaft_mesa | chests/abandoned_mineshaft:17 | 0 / 0 (bounds 1/1) | cave_spider:11 / cave_spider:4 |
| overworld/monument | — | 0 / 0 (bounds 10/10) | — / — |
| overworld/ocean_ruin_cold | archaeology/ocean_ruin_cold:363, chests/underwater_ruin_big:7, chests/underwater_ruin_small:45 | 71 / 0 (bounds 37/37) | — / — |
| overworld/ocean_ruin_warm | archaeology/ocean_ruin_warm:218, chests/underwater_ruin_big:4, chests/underwater_ruin_small:42 | 71 / 0 (bounds 35/36) | — / — |
| overworld/pillager_outpost | chests/pillager_outpost:1 | 2 / 0 (bounds 2/2) | — / — |
| overworld/ruined_portal | chests/ruined_portal:12 | 6 / 0 (bounds 6/6) | — / — |
| overworld/ruined_portal_desert | chests/ruined_portal:5 | 0 / 0 (bounds 0/0) | — / — |
| overworld/ruined_portal_jungle | chests/ruined_portal:2 | 3 / 0 (bounds 3/3) | — / — |
| overworld/ruined_portal_mountain | chests/ruined_portal:2 | 2 / 0 (bounds 2/2) | — / — |
| overworld/ruined_portal_ocean | chests/ruined_portal:14 | 22 / 0 (bounds 21/21) | — / — |
| overworld/ruined_portal_swamp | — | 2 / 0 (bounds 2/2) | — / — |
| overworld/shipwreck | chests/shipwreck_map:32, chests/shipwreck_supply:25, chests/shipwreck_treasure:27 | 111 / 0 (bounds 51/51) | — / — |
| overworld/shipwreck_beached | chests/shipwreck_map:1, chests/shipwreck_supply:1, chests/shipwreck_treasure:1 | 0 / 0 (bounds 0/0) | — / — |
| overworld/stronghold | chests/stronghold_corridor:4, chests/stronghold_crossing:1, chests/stronghold_library:3 | 27 / 5 (bounds 3/3) | silverfish:1 / silverfish:3, skeleton:1, spider:1, zombie:1 |
| overworld/swamp_hut | — | 0 / 0 (bounds 1/1) | — / — |
| overworld/trail_ruins | archaeology/trail_ruins_common:215, archaeology/trail_ruins_rare:77 | 0 / 0 (bounds 2/2) | — / — |
| overworld/trial_chambers | chests/trial_chambers/corridor:274, chests/trial_chambers/entrance:104, chests/trial_chambers/intersection:25, chests/trial_chambers/intersection_barrel:34, chests/trial_chambers/reward:121, chests/trial_chambers/supply:9, dispensers/trial_chambers/chamber:1014, dispensers/trial_chambers/corridor:8, pots/trial_chambers/corridor:1919 | 0 / 1007 (bounds 45/45) | — / cave_spider:8, skeleton:4, spider:4, zombie:9 |
| overworld/village_desert | chests/village/village_desert_house:10, chests/village/village_weaponsmith:1 | 0 / 0 (bounds 0/0) | — / — |
| overworld/village_plains | chests/village/village_plains_house:7, chests/village/village_tannery:1 | 0 / 8 (bounds 2/3) | — / — |
| overworld/village_savanna | chests/village/village_butcher:1, chests/village/village_cartographer:5, chests/village/village_mason:3, chests/village/village_savanna_house:6, chests/village/village_tannery:3 | 0 / 0 (bounds 0/0) | — / — |
| overworld/village_snowy | — | 0 / 0 (bounds 0/0) | — / — |
| overworld/village_taiga | chests/village/village_taiga_house:8, chests/village/village_tannery:1 | 0 / 18 (bounds 2/2) | — / — |
| the_nether/bastion_remnant | chests/bastion_bridge:7, chests/bastion_hoglin_stable:23, chests/bastion_other:256, chests/bastion_treasure:11 | 0 / 289 (bounds 32/32) | magma_cube:10 / magma_cube:5 |
| the_nether/fortress | chests/nether_bridge:146 | 0 / 134 (bounds 25/28) | blaze:61 / blaze:50 |
| the_nether/nether_fossil | — | 0 / 0 (bounds 0/0) | — / — |
| the_nether/ruined_portal_nether | chests/ruined_portal:31 | 34 / 0 (bounds 34/34) | — / — |
| the_end/end_city | chests/end_city_treasure:14 | 0 / 48 (bounds 13/13) | — / — |

## Saved entities

Entities below are read from entity regions and spatially associated with complete core-start boxes. Save-time populations include natural spawns, movement, breeding and deaths. Negative villager ages in the taiga sample have already advanced from their newborn value; snapshot coordinates are not trustworthy original template spawn markers. VillagerData (profession/type/level), age and persistence are preserved. Empty entity-region records mean no saved entities, not a failed chunk.

| Dimension/type | Saved vanilla entities inside complete bounds | Ours planned fresh-chunk markers (before collision filtering) |
|---|---|---|
| overworld/ancient_city | — | chest_minecart:1; saved frames:0 |
| overworld/buried_treasure | — | —; saved frames:0 |
| overworld/desert_pyramid | — | —; saved frames:0 |
| overworld/igloo | — | —; saved frames:0 |
| overworld/jungle_pyramid | — | —; saved frames:0 |
| overworld/mansion | — | —; saved frames:0 |
| overworld/mineshaft | chest_minecart:673, item:20 | chest_minecart:567; saved frames:0 |
| overworld/mineshaft_mesa | chest_minecart:17 | chest_minecart:6; saved frames:0 |
| overworld/monument | elder_guardian:9 | elder_guardian:30; saved frames:0 |
| overworld/ocean_ruin_cold | drowned:63, item:1, zombie_nautilus:2 | drowned:130; saved frames:0 |
| overworld/ocean_ruin_warm | drowned:41, item:1 | drowned:56; saved frames:0 |
| overworld/pillager_outpost | allay:2, cow:4 | —; saved frames:0 |
| overworld/ruined_portal | — | —; saved frames:0 |
| overworld/ruined_portal_desert | — | —; saved frames:0 |
| overworld/ruined_portal_jungle | — | —; saved frames:0 |
| overworld/ruined_portal_mountain | — | —; saved frames:0 |
| overworld/ruined_portal_ocean | item:4 | —; saved frames:0 |
| overworld/ruined_portal_swamp | — | —; saved frames:0 |
| overworld/shipwreck | — | —; saved frames:0 |
| overworld/shipwreck_beached | — | —; saved frames:0 |
| overworld/stronghold | — | —; saved frames:0 |
| overworld/swamp_hut | — | cat_all_black:1, witch:1; saved frames:0 |
| overworld/trail_ruins | — | —; saved frames:0 |
| overworld/trial_chambers | chest_minecart:14, item:6 | chest_minecart:12; saved frames:0 |
| overworld/village_desert | camel:4, cat:16, cow:4, iron_golem:4, pig:4, rabbit:21, villager:31 | —; saved frames:0 |
| overworld/village_plains | cat:8, chicken:4, cow:22, donkey:1, horse:9, iron_golem:4, item:1, pig:5, sheep:8, villager:42 | —; saved frames:0 |
| overworld/village_savanna | armadillo:3, bee:1, cat:10, chicken:9, cow:5, horse:2, iron_golem:4, item:3, pig:12, sheep:2, villager:24 | —; saved frames:0 |
| overworld/village_snowy | — | —; saved frames:0 |
| overworld/village_taiga | cat:1, fox:4, pig:7, sheep:2, villager:12 | —; saved frames:0 |
| the_nether/bastion_remnant | hoglin:49, piglin:789, piglin_brute:169, strider:5 | —; saved frames:0 |
| the_nether/fortress | item:1, strider:7 | —; saved frames:0 |
| the_nether/nether_fossil | — | —; saved frames:0 |
| the_nether/ruined_portal_nether | — | —; saved frames:0 |
| the_end/end_city | shulker:108 | shulker:267; saved frames:4 |

Our generator defers structure mobs to fresh-chunk hooks in tick.rs: End shulkers, monument elders, mansion occupants, hut witch/black cat, igloo inhabitants, outpost cage golems/allays, ocean-ruin drowned and mineshaft carts. Chunk::generated alone does not execute those hooks; its empty mob array is not proof these hooks are absent. Village inhabitants and bastion piglin/brute/hoglin hooks are missing. Pillager and ordinary guardian natural spawning are separate from saved generation entities. Collision filtering, actual hook success and reload populations remain unverified without running the simulation.

## Concrete coordinates

Coordinates are **start chunks** followed by inclusive **block bounding boxes**. Examples in opposite columns are distribution samples, not matched structures: our placement RNG and terrain differ.

| Dimension/type | Vanilla example | Ours example |
|---|---|---|
| overworld/ancient_city | [513, -527]; BB [8102, -52, -8541, 8320, -22, -8324] | [-490, -541]; BB [-7950, -52, -8765, -7735, -22, -8548] |
| overworld/buried_treasure | [-537, -17]; BB [-8583, 64, -263, -8583, 64, -263] | [-508, -482]; BB [-8119, 61, -7703, -8119, 61, -7703] |
| overworld/desert_pyramid | [-63, 22]; BB [-1008, 67, 352, -988, 81, 372] | —; BB — |
| overworld/mineshaft | [-529, -539]; BB [-8517, -42, -8713, -8390, -18, -8552] | [-535, -512]; BB [-8613, -6, -8244, -8505, 14, -8102] |
| overworld/mineshaft_mesa | [-22, 526]; BB [-376, 80, 8396, -283, 101, 8493] | [-522, 539]; BB [-8407, 88, 8540, -8272, 109, 8692] |
| overworld/monument | [-535, -506]; BB [-8589, 39, -8125, -8532, 61, -8068] | [6, -540]; BB [67, 39, -8669, 124, 61, -8612] |
| overworld/ocean_ruin_cold | [-539, -511]; BB [-8624, 40, -8181, -8618, 46, -8176] | [-529, 23]; BB [-8464, 64, 368, -8459, 70, 374] |
| overworld/ocean_ruin_warm | [-530, 481]; BB [-8509, 42, 7664, -8468, 64, 7708] | [-489, -514]; BB [-7824, 53, -8224, -7819, 59, -8218] |
| overworld/pillager_outpost | [501, 529]; BB [7985, 69, 8448, 8032, 100, 8495] | [-533, 23]; BB [-8535, 79, 368, -8503, 128, 393] |
| overworld/ruined_portal | [-32, -540]; BB [-511, 62, -8639, -506, 71, -8634] | [-511, 17]; BB [-8176, -9, 272, -8168, -3, 280] |
| overworld/ruined_portal_desert | [-59, 23]; BB [-943, 64, 368, -934, 73, 374] | —; BB — |
| overworld/ruined_portal_jungle | [-539, 502]; BB [-8625, 93, 8037, -8619, 99, 8041] | [-543, -496]; BB [-8688, 66, -7936, -8681, 74, -7928] |
| overworld/ruined_portal_mountain | [-23, 522]; BB [-360, 62, 8352, -353, 70, 8360] | [-536, 529]; BB [-8576, 71, 8464, -8563, 79, 8472] |
| overworld/ruined_portal_ocean | [-517, -507]; BB [-8272, 37, -8111, -8267, 46, -8106] | [-497, -512]; BB [-7952, 41, -8192, -7943, 50, -8186] |
| overworld/ruined_portal_swamp | —; BB — | [48, 2]; BB [768, 59, 32, 774, 68, 41] |
| overworld/shipwreck | [-533, -519]; BB [-8532, 54, -8293, -8509, 62, -8285] | [-496, -514]; BB [-7936, 44, -8224, -7921, 52, -8216] |
| overworld/shipwreck_beached | [-490, 516]; BB [-7851, 61, 8267, -7828, 69, 8275] | —; BB — |
| overworld/stronghold | [-511, 17]; BB [-8210, -60, 236, -8062, -28, 363] | [-520, 4]; BB [-8363, -16, 38, -8240, 10, 127] |
| overworld/swamp_hut | —; BB — | [-9, 500]; BB [-144, 65, 8000, -138, 71, 8008] |
| overworld/trail_ruins | [45, -27]; BB [708, 57, -442, 732, 75, -393] | [34, -19]; BB [534, 52, -314, 558, 74, -263] |
| overworld/trial_chambers | [-527, -544]; BB [-8499, -44, -8785, -8389, 19, -8693] | [-534, -537]; BB [-8617, -34, -8674, -8493, 13, -8582] |
| overworld/village_desert | [-48, 2]; BB [-786, 62, 5, -734, 74, 78] | —; BB — |
| overworld/village_plains | [-32, -507]; BB [-578, 62, -8163, -450, 105, -8033] | [-524, -544]; BB [-8460, 60, -8754, -8315, 90, -8630] |
| overworld/village_savanna | [-50, -44]; BB [-875, 62, -782, -715, 104, -634] | —; BB — |
| overworld/village_taiga | [510, 484]; BB [8096, 62, 7672, 8225, 86, 7796] | [-17, 494]; BB [-337, 67, 7852, -208, 86, 7976] |
| the_nether/bastion_remnant | [-529, -523]; BB [-8495, 31, -8384, -8440, 95, -8321] | [-538, -5]; BB [-8628, 31, -80, -8579, 86, -33] |
| the_nether/fortress | [-538, 494]; BB [-8727, 48, 7841, -8541, 76, 8019] | [-538, -532]; BB [-8657, 38, -8562, -8515, 65, -8389] |
| the_nether/nether_fossil | [-514, -514]; BB [-8221, 87, -8211, -8221, 90, -8209] | —; BB — |
| the_nether/ruined_portal_nether | [-517, -507]; BB [-8272, 93, -8112, -8264, 101, -8105] | [-543, -496]; BB [-8688, 31, -7936, -8681, 39, -7928] |
| the_end/end_city | [-538, -14]; BB [-8638, 61, -256, -8566, 166, -164] | [-500, 8]; BB [-8002, 60, 128, -7985, 109, 144] |

## Fixes, before/after and priorities

The baseline was regenerated from **this copy**, not copied from the first report. This copy already contains later dungeon/carver/geode work: for example, its baseline has 1,346 Overworld spawners and 69,262 budding-amethyst blocks. Differences from the first report are not credited to this task.

| Change | Before | After | Verification and limits |
|---|---:|---:|---|
| Bastion spawners assigned magma cubes, core physical records | 0 magma / 63 blaze | 6 magma / 57 blaze | All 63 positions and blocks unchanged; exact six changes in `spawner-fix.json`; generated-chunk and reload regression |
| Taiga village accepted starts, core windows | 0 | 2 | Vanilla has 1. Measured 32 forms and one layout at vanilla start chunk (510,484); no invented snowy substitution. This restores the type, not its distribution |
| End-ship elytra item frames, core chunks | 0 | 4 | Four sampled core-start ships now each retain one frame. Measured static frame coordinates from halo ships; rotation, supporting wall, negative coordinates and chunk save/reload tested |
| Overworld chest blocks | 2,404 | 2,425 | The two taiga villages add 21 chests. Their loot and inhabitants remain missing; this is a partial village implementation |
| Nether / End chest initialization | 34 / 0 container records | 34 / 0 | Unchanged. Core worlds contain 481 / 46 chest blocks; this functional gap remains open |

End example: our frame at **(-518,126,984)** contains elytra and faces west. Measured vanilla frame centres include **(-7634.5,109.5,-265.96875)** (south) and **(-7734.96875,125.5,7753.5)** (east); both reduce to ship-local frame cell **(6,5,7)**. There are no ships among the four vanilla **core-start** cities, but five ships among halo starts, with four frame associations usable directly inside their saved bounds. Halo specimens inform geometry, never core start-rate denominators. Full evidence is `ship-frame-measurements.json`.

The additional border audit bypasses production `near()`/search radii: it supplies the complete accepted plan directly to each intersecting chunk painter, then compares surviving solid cells against `Chunk::generated`. This catches a plan piece omitted by a too-small production search radius. It is stronger than calling the same radius-limited builder twice, but does not independently validate the planner or intentional topology.

| Our family | Core starts checked | Planned solid cells | Changed cells | Chunk-seam cells checked | Changed seam cells |
|---|---:|---:|---:|---:|---:|
| Plains villages | 3 | 14,526 | 0 | 3,467 | 0 |
| Taiga villages | 2 | 9,744 | 0 | 1,673 | 0 |
| Fortresses | 28 | 772,022 | 0 | 175,287 | 0 |
| Bastions | 32 | 1,505,683 | 0 | 372,762 | 0 |
| End cities | 13 | 69,337 | 0 | 10,011 | 0 |

This checks **2,371,312 solid cells**, including **563,200 seam cells**. No clipping correction was indicated in these families. Mineshaft, stronghold, portal, ruin, outpost and snapshot-family monolithic comparisons remain unverified; their saved bounds and probes alone do not establish absence of clipping.

Prioritized remaining work:

1. **P1 — initialize generated loot. Partial / unfixed.** Dungeon, village, fortress, bastion and End-city chests lack their expected loot initialization. The new reference resolves identifiers (including all four bastion table families), positions and pyramid basements. It does not resolve unopened contents or current pool weights. The indexed wiki loot module explicitly describes older Java data and is incomplete even for known trim pools; no older/invented table was presented as verified 26.1.2 loot. Implement version-verified pools and template-to-table markers, then test generation, overlap and reload. The taiga addition increases uninitialized chest blocks by 21; it does not solve village loot. Empty job-site barrels must not automatically receive loot.
2. **P1 — village/bastion inhabitants. Partial / unfixed.** Vanilla complete core village boxes contain 109 villagers; the taiga specimen has 12 (types/professions/ages retained), and bastions contain 169 brutes, 789 piglins and 49 hoglins. Some saved occupants moved or were born after generation. Derive validated per-piece spawn markers and a once-only persistence path; do not transpose all save-time animals or professions into every instance. Existing shulker/hut/igloo/outpost/mansion/monument hooks were counted as plans, not declared absent. Ship frames were safe to restore because their positions are static and repeated across rotations.
3. **P1 — missing types/features. Partial.** Taiga now has a measured layout; snowy villages remain skipped because no snowy specimen exists in this reference. Nether fossils have **793 core starts** and measured piece names/boxes but no own generator; end gateways have **12 core block entities** and no own generator. Fossil placement/support search and gateway exit/lifecycle semantics still need an independently derived implementation; their absence is not a frequency knob. Four desert-well pairs (8 archaeology records) are present in the core Overworld. Wells and Overworld fossils remain unimplemented. No jar-derived assets or substitute layouts were used.
4. **P2 — biome/terrain acceptance before frequency tuning.** Vanilla versus ours: desert villages 6/0, savanna villages 4/0, pyramids 4/0, buried treasure 22/4, End cities 4/13, ancient cities 1/8, monuments 4/10; trial chambers are 45/45. These are real same-area differences, but the terrain and biome fields differ. Validate biome-conditioned exposure and start-height gates before adjusting spacing or hard-coding this seed. Nether fortress 30/28, bastion 35/32 and portal 35/34 intervals overlap. No spacing/height rule was changed without a demonstrated mechanism.
5. **P2 — actual floors and density adaptation. Partial.** Ring-height and datum probes are measured for every vanilla piece and all exported own pieces. They do not isolate beard_thin/beard_box/bury/encapsulate deformation from natural terrain, foliage, neighbouring structures, or Nether ceilings. Dedicated paired terrain and voxel floor segmentation are still required for a 1:1 adaptation/floating verdict. Whole-snapshot cities/chambers lack comparable jigsaw decomposition. Do not interpret metadata BB air or End bridges as unsupported floors and add ground blindly.
6. **P2 — generative composition. Partial.** Full saved vanilla piece/type/rotation/box inventories and our available plans are retained. One taiga layout and the existing fixed libraries reproduce sampled arrangements, not the original distribution. Add more independently measured layouts or reconstruct connection rules. Taiga samples may retain natural blocks captured inside a piece; one specimen cannot establish a clean procedural template or abandoned-village frequency.

## Validation and reproduction

Final checks: **2,238 Rust tests passed, 0 failed, 65 ignored** (`RUST_TEST_THREADS=2`); **workspace/all-target release clippy passed with no warnings**; **13 Python audit tests passed**. The ignored native generation audits and independent border audit were run explicitly and passed. Plugin WASM examples built successfully for `wasm32-wasip2` before the root suite. Logs: `tmp/audit2/tests-final.log`, `clippy.log`, `python-tests.log`, `plugins.log`, `border.log`.

All commands operate in the copy. Cargo invocations are serialized; the explicit user-requested `-j 4` is used for root tests and clippy. Plugin WASM examples are built first with the copy's own target/tmp directories. No formatting, commits, pushes, sockets or Minecraft server operations.

```sh
# Reference census and geometry; repeat DIM=the_nether and the_end.
PYTHONDONTWRITEBYTECODE=1 python3 tools/measure/structures_audit2.py ~/.cache/mcsheriffanya/vanilla-struct/world tmp/audit2/vanilla-overworld.json --dimension overworld --geometry
PYTHONDONTWRITEBYTECODE=1 python3 tools/measure/structures_audit2_integrity.py ~/.cache/mcsheriffanya/vanilla-struct/world tools/research/structures-audit-2-data/integrity.json
# Reproduce the measured taiga asset (no network/server entry point).
PYTHONDONTWRITEBYTECODE=1 python3 tools/measure/structures_audit2_extract.py ~/.cache/mcsheriffanya/vanilla-struct/world tmp/audit2/vanilla-overworld.json tmp/audit2/taiga-reproduced.txt tmp/audit2/extract
# Native start export, then probe/chunk selection; repeat each dimension.
CARGO_TARGET_DIR=$PWD/target CARGO_NET_OFFLINE=true TMPDIR=$PWD/tmp AUDIT_OUT=$PWD/tmp/audit2/after AUDIT_DIM=overworld AUDIT_HALO=8 AUDIT_LIMIT=0 nice -n 19 cargo test --release -j 4 offline_structures_audit -- --ignored --nocapture
PYTHONDONTWRITEBYTECODE=1 python3 tools/measure/structures_audit2_probes.py tmp/audit2/after
CARGO_TARGET_DIR=$PWD/target CARGO_NET_OFFLINE=true TMPDIR=$PWD/tmp AUDIT_OUT=$PWD/tmp/audit2/after AUDIT_DIM=overworld AUDIT_HALO=8 AUDIT_CHUNKS=$PWD/tmp/audit2/after/overworld-selection.json AUDIT_PROBES=$PWD/tmp/audit2/after/overworld-probes.json nice -n 19 cargo test --release -j 4 offline_structures_audit -- --ignored --nocapture
CARGO_TARGET_DIR=$PWD/target CARGO_NET_OFFLINE=true TMPDIR=$PWD/tmp AUDIT_OUT=$PWD/tmp/audit2/after nice -n 19 cargo test --release -j 4 offline_structure_border_audit -- --ignored --nocapture
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tools/measure -p 'test_structures_audit*.py'
PYTHONDONTWRITEBYTECODE=1 python3 tools/measure/structures_audit2_report.py
```

Raw before/after chunk JSONL and logs remain in `tmp/audit2`. Start-rate denominators are exactly 49,152 core chunks per dimension in both runs. Extra generated chunks supply geometry only. Regression tests cover malformed/truncated records, full chunks missing sections, palette lengths, halo separation, negative bounds, NN censoring, deterministic window intervals, pyramid basement attribution, magma-cube persistence, measured frame geometry/persistence and taiga whole-plan versus tiled painting.

No new black-box scenarios: the generation triggers and chunk serialization are exercised by the offline generator/regressions. Manually placing a spawner or frame would not exercise the fixed generation path. Runtime structure-population tests remain unverified. An unrelated pre-existing plugin test assumed a global tick counter of zero and failed under parallel tests (due 1243 instead of 1200); its assertions now use the plugin's actual enable tick, preserving the 1200-tick delay/period check.

Primary evidence: saved 26.1.2 world metadata, region/entity records, and this project's generator. Prose cross-checks: [End City](https://minecraft.wiki/w/End_City#End_ship), [Bastion magma-cube spawners](https://minecraft.wiki/w/Tutorial:Froglight_farming#Bastion_Remnants). [The indexed loot module](https://minecraft.wiki/w/Module:LootChest) exposes older data and was **not** used to invent current loot pools.

**Audit status: partial 1:1 coverage.** Counts, saved composition/entities/loot IDs, observed geometry profiles and selected independent border checks now have a real structure-enabled reference. Exact floors/floating classification, isolated density adaptation, complete procedural composition and several functional gaps remain unresolved as listed above.

