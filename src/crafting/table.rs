// Рецепты верстака: создано из таблицы minecraft-data (data/pc/26.1/recipes.json),
// это данные, не код. Варианты одного рецепта с разными предметами (тег
// «любые доски» и т. п.) слиты: в каждой клетке — все подходящие предметы.
// Не править руками.

use super::Recipe;

pub const RECIPES: &[Recipe] = &[
    Recipe { shaped: true, width: 3, height: 2, result: 872, count: 1, cells: &[&[40], &[], &[40], &[40], &[40], &[40]] }, // acacia_boat
    Recipe { shaped: true, width: 2, height: 3, result: 785, count: 3, cells: &[&[40], &[40], &[40], &[40], &[40], &[40]] }, // acacia_door
    Recipe { shaped: true, width: 3, height: 2, result: 349, count: 3, cells: &[&[40], &[947], &[40], &[40], &[947], &[40]] }, // acacia_fence
    Recipe { shaped: true, width: 3, height: 2, result: 826, count: 1, cells: &[&[947], &[40], &[947], &[947], &[40], &[947]] }, // acacia_fence_gate
    Recipe { shaped: true, width: 3, height: 3, result: 1005, count: 6, cells: &[&[400], &[], &[400], &[152], &[152], &[152], &[152], &[152], &[152]] }, // acacia_hanging_sign
    Recipe { shaped: true, width: 2, height: 1, result: 772, count: 1, cells: &[&[40], &[40]] }, // acacia_pressure_plate
    Recipe { shaped: true, width: 3, height: 3, result: 306, count: 6, cells: &[&[152], &[152], &[152], &[], &[], &[], &[152], &[152], &[152]] }, // acacia_shelf
    Recipe { shaped: true, width: 3, height: 3, result: 993, count: 3, cells: &[&[40], &[40], &[40], &[40], &[40], &[40], &[], &[947], &[]] }, // acacia_sign
    Recipe { shaped: true, width: 3, height: 1, result: 275, count: 6, cells: &[&[40], &[40], &[40]] }, // acacia_slab
    Recipe { shaped: true, width: 3, height: 3, result: 446, count: 4, cells: &[&[40], &[], &[], &[40], &[40], &[], &[40], &[40], &[40]] }, // acacia_stairs
    Recipe { shaped: true, width: 3, height: 2, result: 806, count: 2, cells: &[&[40], &[40], &[40], &[40], &[40], &[40]] }, // acacia_trapdoor
    Recipe { shaped: true, width: 2, height: 2, result: 175, count: 3, cells: &[&[138], &[138], &[138], &[138]] }, // acacia_wood
    Recipe { shaped: true, width: 3, height: 3, result: 837, count: 6, cells: &[&[905], &[947], &[905], &[905], &[719], &[905], &[905], &[947], &[905]] }, // activator_rail
    Recipe { shaped: true, width: 2, height: 2, result: 88, count: 1, cells: &[&[903], &[903], &[903], &[903]] }, // amethyst_block
    Recipe { shaped: true, width: 3, height: 1, result: 709, count: 6, cells: &[&[6], &[6], &[6]] }, // andesite_slab
    Recipe { shaped: true, width: 3, height: 3, result: 692, count: 4, cells: &[&[6], &[], &[], &[6], &[6], &[], &[6], &[6], &[6]] }, // andesite_stairs
    Recipe { shaped: true, width: 3, height: 2, result: 467, count: 6, cells: &[&[6], &[6], &[6], &[6], &[6], &[6]] }, // andesite_wall
    Recipe { shaped: true, width: 3, height: 3, result: 479, count: 1, cells: &[&[90], &[90], &[90], &[], &[905], &[], &[905], &[905], &[905]] }, // anvil
    Recipe { shaped: true, width: 3, height: 3, result: 1255, count: 1, cells: &[&[947], &[947], &[947], &[], &[947], &[], &[947], &[285], &[947]] }, // armor_stand
    Recipe { shaped: true, width: 1, height: 3, result: 896, count: 4, cells: &[&[983], &[947], &[950]] }, // arrow
    Recipe { shaped: true, width: 2, height: 3, result: 790, count: 3, cells: &[&[45], &[45], &[45], &[45], &[45], &[45]] }, // bamboo_door
    Recipe { shaped: true, width: 3, height: 2, result: 354, count: 3, cells: &[&[45], &[947], &[45], &[45], &[947], &[45]] }, // bamboo_fence
    Recipe { shaped: true, width: 3, height: 2, result: 831, count: 1, cells: &[&[947], &[45], &[947], &[947], &[45], &[947]] }, // bamboo_fence_gate
    Recipe { shaped: true, width: 3, height: 3, result: 1010, count: 6, cells: &[&[400], &[], &[400], &[170], &[170], &[170], &[170], &[170], &[170]] }, // bamboo_hanging_sign
    Recipe { shaped: true, width: 1, height: 2, result: 48, count: 1, cells: &[&[280], &[280]] }, // bamboo_mosaic
    Recipe { shaped: true, width: 3, height: 1, result: 281, count: 6, cells: &[&[48], &[48], &[48]] }, // bamboo_mosaic_slab
    Recipe { shaped: true, width: 3, height: 3, result: 452, count: 4, cells: &[&[48], &[], &[], &[48], &[48], &[], &[48], &[48], &[48]] }, // bamboo_mosaic_stairs
    Recipe { shaped: true, width: 2, height: 1, result: 777, count: 1, cells: &[&[45], &[45]] }, // bamboo_pressure_plate
    Recipe { shaped: true, width: 3, height: 2, result: 882, count: 1, cells: &[&[45], &[], &[45], &[45], &[45], &[45]] }, // bamboo_raft
    Recipe { shaped: true, width: 3, height: 3, result: 307, count: 6, cells: &[&[170], &[170], &[170], &[], &[], &[], &[170], &[170], &[170]] }, // bamboo_shelf
    Recipe { shaped: true, width: 3, height: 3, result: 998, count: 3, cells: &[&[45], &[45], &[45], &[45], &[45], &[45], &[], &[947], &[]] }, // bamboo_sign
    Recipe { shaped: true, width: 3, height: 1, result: 280, count: 6, cells: &[&[45], &[45], &[45]] }, // bamboo_slab
    Recipe { shaped: true, width: 3, height: 3, result: 451, count: 4, cells: &[&[45], &[], &[], &[45], &[45], &[], &[45], &[45], &[45]] }, // bamboo_stairs
    Recipe { shaped: true, width: 3, height: 2, result: 811, count: 2, cells: &[&[45], &[45], &[45], &[45], &[45], &[45]] }, // bamboo_trapdoor
    Recipe { shaped: true, width: 3, height: 3, result: 1355, count: 1, cells: &[&[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[271, 272, 273, 274, 275, 276, 277, 278, 279, 280, 282, 283], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[271, 272, 273, 274, 275, 276, 277, 278, 279, 280, 282, 283], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47]] }, // barrel
    Recipe { shaped: true, width: 3, height: 3, result: 456, count: 1, cells: &[&[195], &[195], &[195], &[195], &[1241], &[195], &[322], &[322], &[322]] }, // beacon
    Recipe { shaped: true, width: 3, height: 3, result: 1381, count: 1, cells: &[&[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[1379], &[1379], &[1379], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47]] }, // beehive
    Recipe { shaped: true, width: 3, height: 2, result: 868, count: 1, cells: &[&[38], &[], &[38], &[38], &[38], &[38]] }, // birch_boat
    Recipe { shaped: true, width: 2, height: 3, result: 783, count: 3, cells: &[&[38], &[38], &[38], &[38], &[38], &[38]] }, // birch_door
    Recipe { shaped: true, width: 3, height: 2, result: 347, count: 3, cells: &[&[38], &[947], &[38], &[38], &[947], &[38]] }, // birch_fence
    Recipe { shaped: true, width: 3, height: 2, result: 824, count: 1, cells: &[&[947], &[38], &[947], &[947], &[38], &[947]] }, // birch_fence_gate
    Recipe { shaped: true, width: 3, height: 3, result: 1003, count: 6, cells: &[&[400], &[], &[400], &[150], &[150], &[150], &[150], &[150], &[150]] }, // birch_hanging_sign
    Recipe { shaped: true, width: 2, height: 1, result: 770, count: 1, cells: &[&[38], &[38]] }, // birch_pressure_plate
    Recipe { shaped: true, width: 3, height: 3, result: 308, count: 6, cells: &[&[150], &[150], &[150], &[], &[], &[], &[150], &[150], &[150]] }, // birch_shelf
    Recipe { shaped: true, width: 3, height: 3, result: 991, count: 3, cells: &[&[38], &[38], &[38], &[38], &[38], &[38], &[], &[947], &[]] }, // birch_sign
    Recipe { shaped: true, width: 3, height: 1, result: 273, count: 6, cells: &[&[38], &[38], &[38]] }, // birch_slab
    Recipe { shaped: true, width: 3, height: 3, result: 444, count: 4, cells: &[&[38], &[], &[], &[38], &[38], &[], &[38], &[38], &[38]] }, // birch_stairs
    Recipe { shaped: true, width: 3, height: 2, result: 804, count: 2, cells: &[&[38], &[38], &[38], &[38], &[38], &[38]] }, // birch_trapdoor
    Recipe { shaped: true, width: 2, height: 2, result: 173, count: 3, cells: &[&[136], &[136], &[136], &[136]] }, // birch_wood
    Recipe { shaped: true, width: 3, height: 3, result: 1282, count: 1, cells: &[&[228], &[228], &[228], &[228], &[228], &[228], &[], &[947], &[]] }, // black_banner
    Recipe { shaped: true, width: 3, height: 2, result: 1102, count: 1, cells: &[&[228], &[228], &[228], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47]] }, // black_bed
    Recipe { shaped: false, width: 2, height: 1, result: 1102, count: 1, cells: &[&[1082], &[1098]] }, // black_bed
    Recipe { shaped: true, width: 2, height: 1, result: 521, count: 3, cells: &[&[228], &[228]] }, // black_carpet
    Recipe { shaped: false, width: 2, height: 1, result: 521, count: 1, cells: &[&[1082], &[517]] }, // black_carpet
    Recipe { shaped: true, width: 3, height: 2, result: 854, count: 1, cells: &[&[1018], &[1018], &[1018], &[195], &[228], &[195]] }, // black_harness
    Recipe { shaped: false, width: 2, height: 1, result: 854, count: 1, cells: &[&[1082], &[850]] }, // black_harness
    Recipe { shaped: true, width: 3, height: 3, result: 546, count: 8, cells: &[&[195], &[195], &[195], &[195], &[1082], &[195], &[195], &[195], &[195]] }, // black_stained_glass
    Recipe { shaped: true, width: 3, height: 2, result: 562, count: 16, cells: &[&[546], &[546], &[546], &[546], &[546], &[546]] }, // black_stained_glass_pane
    Recipe { shaped: true, width: 3, height: 3, result: 562, count: 8, cells: &[&[409], &[409], &[409], &[409], &[1082], &[409], &[409], &[409], &[409]] }, // black_stained_glass_pane
    Recipe { shaped: true, width: 3, height: 3, result: 502, count: 8, cells: &[&[522], &[522], &[522], &[522], &[1082], &[522], &[522], &[522], &[522]] }, // black_terracotta
    Recipe { shaped: true, width: 3, height: 1, result: 1387, count: 6, cells: &[&[1386], &[1386], &[1386]] }, // blackstone_slab
    Recipe { shaped: true, width: 3, height: 3, result: 1388, count: 4, cells: &[&[1386], &[], &[], &[1386], &[1386], &[], &[1386], &[1386], &[1386]] }, // blackstone_stairs
    Recipe { shaped: true, width: 3, height: 2, result: 472, count: 6, cells: &[&[1386], &[1386], &[1386], &[1386], &[1386], &[1386]] }, // blackstone_wall
    Recipe { shaped: true, width: 3, height: 3, result: 1357, count: 1, cells: &[&[905], &[905], &[905], &[905], &[335], &[905], &[304], &[304], &[304]] }, // blast_furnace
    Recipe { shaped: true, width: 3, height: 3, result: 1278, count: 1, cells: &[&[224], &[224], &[224], &[224], &[224], &[224], &[], &[947], &[]] }, // blue_banner
    Recipe { shaped: true, width: 3, height: 2, result: 1098, count: 1, cells: &[&[224], &[224], &[224], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47]] }, // blue_bed
    Recipe { shaped: false, width: 2, height: 1, result: 1098, count: 1, cells: &[&[1078], &[1102]] }, // blue_bed
    Recipe { shaped: true, width: 2, height: 1, result: 517, count: 3, cells: &[&[224], &[224]] }, // blue_carpet
    Recipe { shaped: false, width: 2, height: 1, result: 517, count: 1, cells: &[&[1078], &[521]] }, // blue_carpet
    Recipe { shaped: true, width: 3, height: 2, result: 850, count: 1, cells: &[&[1018], &[1018], &[1018], &[195], &[224], &[195]] }, // blue_harness
    Recipe { shaped: false, width: 2, height: 1, result: 850, count: 1, cells: &[&[1078], &[854]] }, // blue_harness
    Recipe { shaped: true, width: 3, height: 3, result: 542, count: 8, cells: &[&[195], &[195], &[195], &[195], &[1078], &[195], &[195], &[195], &[195]] }, // blue_stained_glass
    Recipe { shaped: true, width: 3, height: 2, result: 558, count: 16, cells: &[&[542], &[542], &[542], &[542], &[542], &[542]] }, // blue_stained_glass_pane
    Recipe { shaped: true, width: 3, height: 3, result: 558, count: 8, cells: &[&[409], &[409], &[409], &[409], &[1078], &[409], &[409], &[409], &[409]] }, // blue_stained_glass_pane
    Recipe { shaped: true, width: 3, height: 3, result: 498, count: 8, cells: &[&[522], &[522], &[522], &[522], &[1078], &[522], &[522], &[522], &[522]] }, // blue_terracotta
    Recipe { shaped: true, width: 3, height: 3, result: 1445, count: 2, cells: &[&[899], &[1445], &[899], &[899], &[91, 114], &[899], &[899], &[899], &[899]] }, // bolt_armor_trim_smithing_template
    Recipe { shaped: true, width: 3, height: 3, result: 580, count: 1, cells: &[&[1083], &[1083], &[1083], &[1083], &[1083], &[1083], &[1083], &[1083], &[1083]] }, // bone_block
    Recipe { shaped: true, width: 3, height: 3, result: 318, count: 1, cells: &[&[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[1030], &[1030], &[1030], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47]] }, // bookshelf
    Recipe { shaped: true, width: 3, height: 3, result: 895, count: 1, cells: &[&[], &[947], &[949], &[947], &[], &[949], &[], &[947], &[949]] }, // bow
    Recipe { shaped: true, width: 3, height: 2, result: 893, count: 4, cells: &[&[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[]] }, // bowl
    Recipe { shaped: true, width: 3, height: 1, result: 954, count: 1, cells: &[&[953], &[953], &[953]] }, // bread
    Recipe { shaped: true, width: 3, height: 2, result: 1127, count: 1, cells: &[&[], &[1117], &[], &[9, 35, 1386], &[9, 35, 1386], &[9, 35, 1386]] }, // brewing_stand
    Recipe { shaped: true, width: 3, height: 1, result: 290, count: 6, cells: &[&[305], &[305], &[305]] }, // brick_slab
    Recipe { shaped: true, width: 3, height: 3, result: 420, count: 4, cells: &[&[305], &[], &[], &[305], &[305], &[], &[305], &[305], &[305]] }, // brick_stairs
    Recipe { shaped: true, width: 3, height: 2, result: 459, count: 6, cells: &[&[305], &[305], &[305], &[305], &[305], &[305]] }, // brick_wall
    Recipe { shaped: true, width: 2, height: 2, result: 305, count: 1, cells: &[&[1026], &[1026], &[1026], &[1026]] }, // bricks
    Recipe { shaped: true, width: 3, height: 3, result: 1279, count: 1, cells: &[&[225], &[225], &[225], &[225], &[225], &[225], &[], &[947], &[]] }, // brown_banner
    Recipe { shaped: true, width: 3, height: 2, result: 1099, count: 1, cells: &[&[225], &[225], &[225], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47]] }, // brown_bed
    Recipe { shaped: false, width: 2, height: 1, result: 1099, count: 1, cells: &[&[1079], &[1102]] }, // brown_bed
    Recipe { shaped: true, width: 2, height: 1, result: 518, count: 3, cells: &[&[225], &[225]] }, // brown_carpet
    Recipe { shaped: false, width: 2, height: 1, result: 518, count: 1, cells: &[&[1079], &[521]] }, // brown_carpet
    Recipe { shaped: true, width: 3, height: 2, result: 851, count: 1, cells: &[&[1018], &[1018], &[1018], &[195], &[225], &[195]] }, // brown_harness
    Recipe { shaped: false, width: 2, height: 1, result: 851, count: 1, cells: &[&[1079], &[854]] }, // brown_harness
    Recipe { shaped: true, width: 3, height: 3, result: 543, count: 8, cells: &[&[195], &[195], &[195], &[195], &[1079], &[195], &[195], &[195], &[195]] }, // brown_stained_glass
    Recipe { shaped: true, width: 3, height: 2, result: 559, count: 16, cells: &[&[543], &[543], &[543], &[543], &[543], &[543]] }, // brown_stained_glass_pane
    Recipe { shaped: true, width: 3, height: 3, result: 559, count: 8, cells: &[&[409], &[409], &[409], &[409], &[1079], &[409], &[409], &[409], &[409]] }, // brown_stained_glass_pane
    Recipe { shaped: true, width: 3, height: 3, result: 499, count: 8, cells: &[&[522], &[522], &[522], &[522], &[1079], &[522], &[522], &[522], &[522]] }, // brown_terracotta
    Recipe { shaped: true, width: 1, height: 3, result: 1426, count: 1, cells: &[&[950], &[907], &[947]] }, // brush
    Recipe { shaped: true, width: 3, height: 2, result: 1013, count: 1, cells: &[&[905], &[], &[905], &[], &[905], &[]] }, // bucket
    Recipe { shaped: true, width: 1, height: 2, result: 1037, count: 1, cells: &[&[949], &[1018]] }, // bundle
    Recipe { shaped: true, width: 3, height: 3, result: 1086, count: 1, cells: &[&[1019], &[1019], &[1019], &[1085], &[1032, 1033, 1034], &[1085], &[953], &[953], &[953]] }, // cake
    Recipe { shaped: true, width: 3, height: 2, result: 744, count: 1, cells: &[&[], &[903], &[], &[903], &[743], &[903]] }, // calibrated_sculk_sensor
    Recipe { shaped: true, width: 3, height: 3, result: 1376, count: 1, cells: &[&[], &[947], &[], &[947], &[897, 898], &[947], &[134, 135, 136, 137, 138, 139, 140, 141, 142, 145, 146, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 171, 172, 173, 174, 175, 176, 177, 178, 179, 180, 181], &[134, 135, 136, 137, 138, 139, 140, 141, 142, 145, 146, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 171, 172, 173, 174, 175, 176, 177, 178, 179, 180, 181], &[134, 135, 136, 137, 138, 139, 140, 141, 142, 145, 146, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 171, 172, 173, 174, 175, 176, 177, 178, 179, 180, 181]] }, // campfire
    Recipe { shaped: true, width: 1, height: 2, result: 1399, count: 1, cells: &[&[949], &[1379]] }, // candle
    Recipe { shaped: true, width: 2, height: 2, result: 860, count: 1, cells: &[&[1054], &[], &[], &[1228]] }, // carrot_on_a_stick
    Recipe { shaped: true, width: 2, height: 3, result: 1358, count: 1, cells: &[&[1029], &[1029], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47]] }, // cartography_table
    Recipe { shaped: true, width: 3, height: 3, result: 1128, count: 1, cells: &[&[905], &[], &[905], &[905], &[], &[905], &[905], &[905], &[905]] }, // cauldron
    Recipe { shaped: true, width: 3, height: 2, result: 874, count: 1, cells: &[&[41], &[], &[41], &[41], &[41], &[41]] }, // cherry_boat
    Recipe { shaped: true, width: 2, height: 3, result: 786, count: 3, cells: &[&[41], &[41], &[41], &[41], &[41], &[41]] }, // cherry_door
    Recipe { shaped: true, width: 3, height: 2, result: 350, count: 3, cells: &[&[41], &[947], &[41], &[41], &[947], &[41]] }, // cherry_fence
    Recipe { shaped: true, width: 3, height: 2, result: 827, count: 1, cells: &[&[947], &[41], &[947], &[947], &[41], &[947]] }, // cherry_fence_gate
    Recipe { shaped: true, width: 3, height: 3, result: 1006, count: 6, cells: &[&[400], &[], &[400], &[153], &[153], &[153], &[153], &[153], &[153]] }, // cherry_hanging_sign
    Recipe { shaped: true, width: 2, height: 1, result: 773, count: 1, cells: &[&[41], &[41]] }, // cherry_pressure_plate
    Recipe { shaped: true, width: 3, height: 3, result: 309, count: 6, cells: &[&[153], &[153], &[153], &[], &[], &[], &[153], &[153], &[153]] }, // cherry_shelf
    Recipe { shaped: true, width: 3, height: 3, result: 994, count: 3, cells: &[&[41], &[41], &[41], &[41], &[41], &[41], &[], &[947], &[]] }, // cherry_sign
    Recipe { shaped: true, width: 3, height: 1, result: 276, count: 6, cells: &[&[41], &[41], &[41]] }, // cherry_slab
    Recipe { shaped: true, width: 3, height: 3, result: 447, count: 4, cells: &[&[41], &[], &[], &[41], &[41], &[], &[41], &[41], &[41]] }, // cherry_stairs
    Recipe { shaped: true, width: 3, height: 2, result: 807, count: 2, cells: &[&[41], &[41], &[41], &[41], &[41], &[41]] }, // cherry_trapdoor
    Recipe { shaped: true, width: 2, height: 2, result: 176, count: 3, cells: &[&[139], &[139], &[139], &[139]] }, // cherry_wood
    Recipe { shaped: true, width: 3, height: 3, result: 332, count: 1, cells: &[&[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47]] }, // chest
    Recipe { shaped: true, width: 3, height: 3, result: 319, count: 1, cells: &[&[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[271, 272, 273, 274, 275, 276, 277, 278, 279, 280, 282, 283], &[271, 272, 273, 274, 275, 276, 277, 278, 279, 280, 282, 283], &[271, 272, 273, 274, 275, 276, 277, 278, 279, 280, 282, 283], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47]] }, // chiseled_bookshelf
    Recipe { shaped: true, width: 1, height: 2, result: 98, count: 1, cells: &[&[110], &[110]] }, // chiseled_copper
    Recipe { shaped: true, width: 1, height: 2, result: 386, count: 1, cells: &[&[713], &[713]] }, // chiseled_deepslate
    Recipe { shaped: true, width: 1, height: 2, result: 427, count: 1, cells: &[&[293], &[293]] }, // chiseled_nether_bricks
    Recipe { shaped: true, width: 1, height: 2, result: 1393, count: 1, cells: &[&[1391], &[1391]] }, // chiseled_polished_blackstone
    Recipe { shaped: true, width: 1, height: 2, result: 482, count: 1, cells: &[&[294], &[294]] }, // chiseled_quartz_block
    Recipe { shaped: true, width: 1, height: 2, result: 571, count: 1, cells: &[&[295], &[295]] }, // chiseled_red_sandstone
    Recipe { shaped: true, width: 1, height: 2, result: 419, count: 1, cells: &[&[417], &[417]] }, // chiseled_resin_bricks
    Recipe { shaped: true, width: 1, height: 2, result: 199, count: 1, cells: &[&[286], &[286]] }, // chiseled_sandstone
    Recipe { shaped: true, width: 1, height: 2, result: 379, count: 1, cells: &[&[291], &[291]] }, // chiseled_stone_bricks
    Recipe { shaped: true, width: 1, height: 2, result: 16, count: 1, cells: &[&[13], &[13]] }, // chiseled_tuff
    Recipe { shaped: true, width: 1, height: 2, result: 25, count: 1, cells: &[&[22], &[22]] }, // chiseled_tuff_bricks
    Recipe { shaped: true, width: 2, height: 2, result: 343, count: 1, cells: &[&[1027], &[1027], &[1027], &[1027]] }, // clay
    Recipe { shaped: true, width: 3, height: 3, result: 1055, count: 1, cells: &[&[], &[909], &[], &[909], &[718], &[909], &[], &[909], &[]] }, // clock
    Recipe { shaped: true, width: 3, height: 3, result: 83, count: 1, cells: &[&[897], &[897], &[897], &[897], &[897], &[897], &[897], &[897], &[897]] }, // coal_block
    Recipe { shaped: true, width: 2, height: 2, result: 29, count: 4, cells: &[&[28], &[63], &[63], &[28]] }, // coarse_dirt
    Recipe { shaped: true, width: 3, height: 3, result: 1430, count: 2, cells: &[&[899], &[1430], &[899], &[899], &[35], &[899], &[899], &[899], &[899]] }, // coast_armor_trim_smithing_template
    Recipe { shaped: true, width: 3, height: 1, result: 713, count: 6, cells: &[&[9], &[9], &[9]] }, // cobbled_deepslate_slab
    Recipe { shaped: true, width: 3, height: 3, result: 696, count: 4, cells: &[&[9], &[], &[], &[9], &[9], &[], &[9], &[9], &[9]] }, // cobbled_deepslate_stairs
    Recipe { shaped: true, width: 3, height: 2, result: 475, count: 6, cells: &[&[9], &[9], &[9], &[9], &[9], &[9]] }, // cobbled_deepslate_wall
    Recipe { shaped: true, width: 3, height: 1, result: 289, count: 6, cells: &[&[35], &[35], &[35]] }, // cobblestone_slab
    Recipe { shaped: true, width: 3, height: 3, result: 337, count: 4, cells: &[&[35], &[], &[], &[35], &[35], &[], &[35], &[35], &[35]] }, // cobblestone_stairs
    Recipe { shaped: true, width: 3, height: 2, result: 457, count: 6, cells: &[&[35], &[35], &[35], &[35], &[35], &[35]] }, // cobblestone_wall
    Recipe { shaped: true, width: 3, height: 3, result: 722, count: 1, cells: &[&[], &[719], &[], &[719], &[902], &[719], &[1], &[1], &[1]] }, // comparator
    Recipe { shaped: true, width: 3, height: 3, result: 1035, count: 1, cells: &[&[], &[905], &[], &[905], &[718], &[905], &[], &[905], &[]] }, // compass
    Recipe { shaped: true, width: 3, height: 3, result: 1354, count: 1, cells: &[&[271, 272, 273, 274, 275, 276, 277, 278, 279, 280, 282, 283], &[], &[271, 272, 273, 274, 275, 276, 277, 278, 279, 280, 282, 283], &[271, 272, 273, 274, 275, 276, 277, 278, 279, 280, 282, 283], &[], &[271, 272, 273, 274, 275, 276, 277, 278, 279, 280, 282, 283], &[271, 272, 273, 274, 275, 276, 277, 278, 279, 280, 282, 283], &[271, 272, 273, 274, 275, 276, 277, 278, 279, 280, 282, 283], &[271, 272, 273, 274, 275, 276, 277, 278, 279, 280, 282, 283]] }, // composter
    Recipe { shaped: true, width: 3, height: 3, result: 681, count: 1, cells: &[&[1333], &[1333], &[1333], &[1333], &[1339], &[1333], &[1333], &[1333], &[1333]] }, // conduit
    Recipe { shaped: true, width: 3, height: 1, result: 1103, count: 8, cells: &[&[953], &[1066], &[953]] }, // cookie
    Recipe { shaped: true, width: 2, height: 3, result: 920, count: 1, cells: &[&[907], &[907], &[907], &[947], &[], &[947]] }, // copper_axe
    Recipe { shaped: true, width: 3, height: 2, result: 392, count: 16, cells: &[&[907], &[907], &[907], &[907], &[907], &[907]] }, // copper_bars
    Recipe { shaped: true, width: 3, height: 3, result: 91, count: 1, cells: &[&[907], &[907], &[907], &[907], &[907], &[907], &[907], &[907], &[907]] }, // copper_block
    Recipe { shaped: true, width: 3, height: 2, result: 962, count: 1, cells: &[&[907], &[], &[907], &[907], &[], &[907]] }, // copper_boots
    Recipe { shaped: true, width: 3, height: 3, result: 1477, count: 4, cells: &[&[], &[91], &[], &[91], &[1117], &[91], &[], &[718], &[]] }, // copper_bulb
    Recipe { shaped: true, width: 1, height: 3, result: 401, count: 1, cells: &[&[1307], &[907], &[1307]] }, // copper_chain
    Recipe { shaped: true, width: 3, height: 3, result: 1485, count: 1, cells: &[&[907], &[907], &[907], &[907], &[332], &[907], &[907], &[907], &[907]] }, // copper_chest
    Recipe { shaped: true, width: 3, height: 3, result: 960, count: 1, cells: &[&[907], &[], &[907], &[907], &[907], &[907], &[907], &[907], &[907]] }, // copper_chestplate
    Recipe { shaped: true, width: 2, height: 3, result: 793, count: 3, cells: &[&[907], &[907], &[907], &[907], &[907], &[907]] }, // copper_door
    Recipe { shaped: true, width: 3, height: 3, result: 1469, count: 4, cells: &[&[], &[91], &[], &[91], &[], &[91], &[], &[91], &[]] }, // copper_grate
    Recipe { shaped: true, width: 3, height: 2, result: 959, count: 1, cells: &[&[907], &[907], &[907], &[907], &[], &[907]] }, // copper_helmet
    Recipe { shaped: true, width: 2, height: 3, result: 921, count: 1, cells: &[&[907], &[907], &[], &[947], &[], &[947]] }, // copper_hoe
    Recipe { shaped: true, width: 3, height: 3, result: 907, count: 1, cells: &[&[1307], &[1307], &[1307], &[1307], &[1307], &[1307], &[1307], &[1307], &[1307]] }, // copper_ingot
    Recipe { shaped: false, width: 1, height: 1, result: 907, count: 9, cells: &[&[91, 114]] }, // copper_ingot
    Recipe { shaped: true, width: 3, height: 3, result: 1366, count: 1, cells: &[&[1307], &[1307], &[1307], &[1307], &[367], &[1307], &[1307], &[1307], &[1307]] }, // copper_lantern
    Recipe { shaped: true, width: 3, height: 3, result: 961, count: 1, cells: &[&[907], &[907], &[907], &[907], &[], &[907], &[907], &[], &[907]] }, // copper_leggings
    Recipe { shaped: true, width: 3, height: 3, result: 919, count: 1, cells: &[&[907], &[907], &[907], &[], &[947], &[], &[], &[947], &[]] }, // copper_pickaxe
    Recipe { shaped: true, width: 1, height: 3, result: 918, count: 1, cells: &[&[907], &[947], &[947]] }, // copper_shovel
    Recipe { shaped: true, width: 3, height: 3, result: 1299, count: 1, cells: &[&[], &[], &[907], &[], &[947], &[], &[947], &[], &[]] }, // copper_spear
    Recipe { shaped: true, width: 1, height: 3, result: 917, count: 1, cells: &[&[907], &[907], &[947]] }, // copper_sword
    Recipe { shaped: true, width: 1, height: 3, result: 367, count: 4, cells: &[&[1307], &[897, 898], &[947]] }, // copper_torch
    Recipe { shaped: true, width: 2, height: 2, result: 814, count: 1, cells: &[&[907], &[907], &[907], &[907]] }, // copper_trapdoor
    Recipe { shaped: true, width: 3, height: 3, result: 1104, count: 1, cells: &[&[905], &[905], &[905], &[905], &[333], &[905], &[718], &[730], &[718]] }, // crafter
    Recipe { shaped: true, width: 2, height: 2, result: 333, count: 1, cells: &[&[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47]] }, // crafting_table
    Recipe { shaped: true, width: 1, height: 3, result: 331, count: 1, cells: &[&[140], &[414], &[140]] }, // creaking_heart
    Recipe { shaped: true, width: 2, height: 3, result: 791, count: 3, cells: &[&[46], &[46], &[46], &[46], &[46], &[46]] }, // crimson_door
    Recipe { shaped: true, width: 3, height: 2, result: 355, count: 3, cells: &[&[46], &[947], &[46], &[46], &[947], &[46]] }, // crimson_fence
    Recipe { shaped: true, width: 3, height: 2, result: 832, count: 1, cells: &[&[947], &[46], &[947], &[947], &[46], &[947]] }, // crimson_fence_gate
    Recipe { shaped: true, width: 3, height: 3, result: 1011, count: 6, cells: &[&[400], &[], &[400], &[157], &[157], &[157], &[157], &[157], &[157]] }, // crimson_hanging_sign
    Recipe { shaped: true, width: 2, height: 2, result: 180, count: 3, cells: &[&[145], &[145], &[145], &[145]] }, // crimson_hyphae
    Recipe { shaped: true, width: 2, height: 1, result: 778, count: 1, cells: &[&[46], &[46]] }, // crimson_pressure_plate
    Recipe { shaped: true, width: 3, height: 3, result: 310, count: 6, cells: &[&[157], &[157], &[157], &[], &[], &[], &[157], &[157], &[157]] }, // crimson_shelf
    Recipe { shaped: true, width: 3, height: 3, result: 999, count: 3, cells: &[&[46], &[46], &[46], &[46], &[46], &[46], &[], &[947], &[]] }, // crimson_sign
    Recipe { shaped: true, width: 3, height: 1, result: 282, count: 6, cells: &[&[46], &[46], &[46]] }, // crimson_slab
    Recipe { shaped: true, width: 3, height: 3, result: 453, count: 4, cells: &[&[46], &[], &[], &[46], &[46], &[], &[46], &[46], &[46]] }, // crimson_stairs
    Recipe { shaped: true, width: 3, height: 2, result: 812, count: 2, cells: &[&[46], &[46], &[46], &[46], &[46], &[46]] }, // crimson_trapdoor
    Recipe { shaped: true, width: 3, height: 3, result: 1340, count: 1, cells: &[&[947], &[905], &[947], &[949], &[745], &[949], &[], &[947], &[]] }, // crossbow
    Recipe { shaped: true, width: 2, height: 2, result: 102, count: 4, cells: &[&[91], &[91], &[91], &[91]] }, // cut_copper
    Recipe { shaped: true, width: 3, height: 1, result: 110, count: 6, cells: &[&[102], &[102], &[102]] }, // cut_copper_slab
    Recipe { shaped: true, width: 3, height: 3, result: 106, count: 4, cells: &[&[102], &[], &[], &[102], &[102], &[], &[102], &[102], &[102]] }, // cut_copper_stairs
    Recipe { shaped: true, width: 2, height: 2, result: 572, count: 4, cells: &[&[570], &[570], &[570], &[570]] }, // cut_red_sandstone
    Recipe { shaped: true, width: 3, height: 1, result: 296, count: 6, cells: &[&[572], &[572], &[572]] }, // cut_red_sandstone_slab
    Recipe { shaped: true, width: 2, height: 2, result: 200, count: 4, cells: &[&[198], &[198], &[198], &[198]] }, // cut_sandstone
    Recipe { shaped: true, width: 3, height: 1, result: 287, count: 6, cells: &[&[200], &[200], &[200]] }, // cut_sandstone_slab
    Recipe { shaped: true, width: 3, height: 3, result: 1276, count: 1, cells: &[&[222], &[222], &[222], &[222], &[222], &[222], &[], &[947], &[]] }, // cyan_banner
    Recipe { shaped: true, width: 3, height: 2, result: 1096, count: 1, cells: &[&[222], &[222], &[222], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47]] }, // cyan_bed
    Recipe { shaped: false, width: 2, height: 1, result: 1096, count: 1, cells: &[&[1076], &[1102]] }, // cyan_bed
    Recipe { shaped: true, width: 2, height: 1, result: 515, count: 3, cells: &[&[222], &[222]] }, // cyan_carpet
    Recipe { shaped: false, width: 2, height: 1, result: 515, count: 1, cells: &[&[1076], &[521]] }, // cyan_carpet
    Recipe { shaped: true, width: 3, height: 2, result: 848, count: 1, cells: &[&[1018], &[1018], &[1018], &[195], &[222], &[195]] }, // cyan_harness
    Recipe { shaped: false, width: 2, height: 1, result: 848, count: 1, cells: &[&[1076], &[854]] }, // cyan_harness
    Recipe { shaped: true, width: 3, height: 3, result: 540, count: 8, cells: &[&[195], &[195], &[195], &[195], &[1076], &[195], &[195], &[195], &[195]] }, // cyan_stained_glass
    Recipe { shaped: true, width: 3, height: 2, result: 556, count: 16, cells: &[&[540], &[540], &[540], &[540], &[540], &[540]] }, // cyan_stained_glass_pane
    Recipe { shaped: true, width: 3, height: 3, result: 556, count: 8, cells: &[&[409], &[409], &[409], &[409], &[1076], &[409], &[409], &[409], &[409]] }, // cyan_stained_glass_pane
    Recipe { shaped: true, width: 3, height: 3, result: 496, count: 8, cells: &[&[522], &[522], &[522], &[522], &[1076], &[522], &[522], &[522], &[522]] }, // cyan_terracotta
    Recipe { shaped: true, width: 3, height: 2, result: 876, count: 1, cells: &[&[42], &[], &[42], &[42], &[42], &[42]] }, // dark_oak_boat
    Recipe { shaped: true, width: 2, height: 3, result: 787, count: 3, cells: &[&[42], &[42], &[42], &[42], &[42], &[42]] }, // dark_oak_door
    Recipe { shaped: true, width: 3, height: 2, result: 351, count: 3, cells: &[&[42], &[947], &[42], &[42], &[947], &[42]] }, // dark_oak_fence
    Recipe { shaped: true, width: 3, height: 2, result: 828, count: 1, cells: &[&[947], &[42], &[947], &[947], &[42], &[947]] }, // dark_oak_fence_gate
    Recipe { shaped: true, width: 3, height: 3, result: 1007, count: 6, cells: &[&[400], &[], &[400], &[154], &[154], &[154], &[154], &[154], &[154]] }, // dark_oak_hanging_sign
    Recipe { shaped: true, width: 2, height: 1, result: 774, count: 1, cells: &[&[42], &[42]] }, // dark_oak_pressure_plate
    Recipe { shaped: true, width: 3, height: 3, result: 311, count: 6, cells: &[&[154], &[154], &[154], &[], &[], &[], &[154], &[154], &[154]] }, // dark_oak_shelf
    Recipe { shaped: true, width: 3, height: 3, result: 995, count: 3, cells: &[&[42], &[42], &[42], &[42], &[42], &[42], &[], &[947], &[]] }, // dark_oak_sign
    Recipe { shaped: true, width: 3, height: 1, result: 277, count: 6, cells: &[&[42], &[42], &[42]] }, // dark_oak_slab
    Recipe { shaped: true, width: 3, height: 3, result: 448, count: 4, cells: &[&[42], &[], &[], &[42], &[42], &[], &[42], &[42], &[42]] }, // dark_oak_stairs
    Recipe { shaped: true, width: 3, height: 2, result: 808, count: 2, cells: &[&[42], &[42], &[42], &[42], &[42], &[42]] }, // dark_oak_trapdoor
    Recipe { shaped: true, width: 2, height: 2, result: 178, count: 3, cells: &[&[141], &[141], &[141], &[141]] }, // dark_oak_wood
    Recipe { shaped: true, width: 3, height: 3, result: 565, count: 1, cells: &[&[1248], &[1248], &[1248], &[1248], &[1082], &[1248], &[1248], &[1248], &[1248]] }, // dark_prismarine
    Recipe { shaped: true, width: 3, height: 1, result: 300, count: 6, cells: &[&[565], &[565], &[565]] }, // dark_prismarine_slab
    Recipe { shaped: true, width: 3, height: 3, result: 568, count: 4, cells: &[&[565], &[], &[], &[565], &[565], &[], &[565], &[565], &[565]] }, // dark_prismarine_stairs
    Recipe { shaped: true, width: 3, height: 3, result: 742, count: 1, cells: &[&[195], &[195], &[195], &[902], &[902], &[902], &[271, 272, 273, 274, 275, 276, 277, 278, 279, 280, 282, 283], &[271, 272, 273, 274, 275, 276, 277, 278, 279, 280, 282, 283], &[271, 272, 273, 274, 275, 276, 277, 278, 279, 280, 282, 283]] }, // daylight_detector
    Recipe { shaped: true, width: 3, height: 3, result: 320, count: 1, cells: &[&[], &[1026], &[], &[1026], &[], &[1026], &[], &[1026], &[]] }, // decorated_pot
    Recipe { shaped: true, width: 3, height: 1, result: 715, count: 6, cells: &[&[382], &[382], &[382]] }, // deepslate_brick_slab
    Recipe { shaped: true, width: 3, height: 3, result: 698, count: 4, cells: &[&[382], &[], &[], &[382], &[382], &[], &[382], &[382], &[382]] }, // deepslate_brick_stairs
    Recipe { shaped: true, width: 3, height: 2, result: 477, count: 6, cells: &[&[382], &[382], &[382], &[382], &[382], &[382]] }, // deepslate_brick_wall
    Recipe { shaped: true, width: 2, height: 2, result: 382, count: 4, cells: &[&[10], &[10], &[10], &[10]] }, // deepslate_bricks
    Recipe { shaped: true, width: 3, height: 1, result: 716, count: 6, cells: &[&[384], &[384], &[384]] }, // deepslate_tile_slab
    Recipe { shaped: true, width: 3, height: 3, result: 699, count: 4, cells: &[&[384], &[], &[], &[384], &[384], &[], &[384], &[384], &[384]] }, // deepslate_tile_stairs
    Recipe { shaped: true, width: 3, height: 2, result: 478, count: 6, cells: &[&[384], &[384], &[384], &[384], &[384], &[384]] }, // deepslate_tile_wall
    Recipe { shaped: true, width: 2, height: 2, result: 384, count: 4, cells: &[&[382], &[382], &[382], &[382]] }, // deepslate_tiles
    Recipe { shaped: true, width: 3, height: 3, result: 835, count: 6, cells: &[&[905], &[], &[905], &[905], &[764], &[905], &[905], &[718], &[905]] }, // detector_rail
    Recipe { shaped: true, width: 2, height: 3, result: 940, count: 1, cells: &[&[899], &[899], &[899], &[947], &[], &[947]] }, // diamond_axe
    Recipe { shaped: true, width: 3, height: 3, result: 93, count: 1, cells: &[&[899], &[899], &[899], &[899], &[899], &[899], &[899], &[899], &[899]] }, // diamond_block
    Recipe { shaped: true, width: 3, height: 2, result: 974, count: 1, cells: &[&[899], &[], &[899], &[899], &[], &[899]] }, // diamond_boots
    Recipe { shaped: true, width: 3, height: 3, result: 972, count: 1, cells: &[&[899], &[], &[899], &[899], &[899], &[899], &[899], &[899], &[899]] }, // diamond_chestplate
    Recipe { shaped: true, width: 3, height: 2, result: 971, count: 1, cells: &[&[899], &[899], &[899], &[899], &[], &[899]] }, // diamond_helmet
    Recipe { shaped: true, width: 2, height: 3, result: 941, count: 1, cells: &[&[899], &[899], &[], &[947], &[], &[947]] }, // diamond_hoe
    Recipe { shaped: true, width: 3, height: 3, result: 973, count: 1, cells: &[&[899], &[899], &[899], &[899], &[], &[899], &[899], &[], &[899]] }, // diamond_leggings
    Recipe { shaped: true, width: 3, height: 3, result: 939, count: 1, cells: &[&[899], &[899], &[899], &[], &[947], &[], &[], &[947], &[]] }, // diamond_pickaxe
    Recipe { shaped: true, width: 1, height: 3, result: 938, count: 1, cells: &[&[899], &[947], &[947]] }, // diamond_shovel
    Recipe { shaped: true, width: 3, height: 3, result: 1302, count: 1, cells: &[&[], &[], &[899], &[], &[947], &[], &[947], &[], &[]] }, // diamond_spear
    Recipe { shaped: true, width: 1, height: 3, result: 937, count: 1, cells: &[&[899], &[899], &[947]] }, // diamond_sword
    Recipe { shaped: true, width: 2, height: 2, result: 4, count: 2, cells: &[&[35], &[902], &[902], &[35]] }, // diorite
    Recipe { shaped: true, width: 3, height: 1, result: 712, count: 6, cells: &[&[4], &[4], &[4]] }, // diorite_slab
    Recipe { shaped: true, width: 3, height: 3, result: 695, count: 4, cells: &[&[4], &[], &[], &[4], &[4], &[], &[4], &[4], &[4]] }, // diorite_stairs
    Recipe { shaped: true, width: 3, height: 2, result: 471, count: 6, cells: &[&[4], &[4], &[4], &[4], &[4], &[4]] }, // diorite_wall
    Recipe { shaped: true, width: 3, height: 3, result: 729, count: 1, cells: &[&[35], &[35], &[35], &[35], &[895], &[35], &[35], &[718], &[35]] }, // dispenser
    Recipe { shaped: true, width: 3, height: 3, result: 649, count: 1, cells: &[&[1118], &[1118], &[1118], &[1118], &[361], &[1118], &[1118], &[1118], &[1118]] }, // dried_ghast
    Recipe { shaped: true, width: 3, height: 3, result: 1028, count: 1, cells: &[&[1108], &[1108], &[1108], &[1108], &[1108], &[1108], &[1108], &[1108], &[1108]] }, // dried_kelp_block
    Recipe { shaped: true, width: 2, height: 2, result: 26, count: 1, cells: &[&[1420], &[1420], &[1420], &[1420]] }, // dripstone_block
    Recipe { shaped: true, width: 3, height: 3, result: 730, count: 1, cells: &[&[35], &[35], &[35], &[35], &[], &[35], &[35], &[718], &[35]] }, // dropper
    Recipe { shaped: true, width: 3, height: 3, result: 1429, count: 2, cells: &[&[899], &[1429], &[899], &[899], &[198], &[899], &[899], &[899], &[899]] }, // dune_armor_trim_smithing_template
    Recipe { shaped: true, width: 3, height: 3, result: 441, count: 1, cells: &[&[900], &[900], &[900], &[900], &[900], &[900], &[900], &[900], &[900]] }, // emerald_block
    Recipe { shaped: true, width: 3, height: 3, result: 434, count: 1, cells: &[&[], &[1030], &[], &[899], &[322], &[899], &[322], &[322], &[322]] }, // enchanting_table
    Recipe { shaped: true, width: 3, height: 3, result: 1283, count: 1, cells: &[&[195], &[195], &[195], &[195], &[1129], &[195], &[195], &[1118], &[195]] }, // end_crystal
    Recipe { shaped: true, width: 1, height: 2, result: 324, count: 4, cells: &[&[1117], &[1285]] }, // end_rod
    Recipe { shaped: true, width: 3, height: 1, result: 705, count: 6, cells: &[&[437], &[437], &[437]] }, // end_stone_brick_slab
    Recipe { shaped: true, width: 3, height: 3, result: 687, count: 4, cells: &[&[437], &[], &[], &[437], &[437], &[], &[437], &[437], &[437]] }, // end_stone_brick_stairs
    Recipe { shaped: true, width: 3, height: 2, result: 470, count: 6, cells: &[&[437], &[437], &[437], &[437], &[437], &[437]] }, // end_stone_brick_wall
    Recipe { shaped: true, width: 2, height: 2, result: 437, count: 4, cells: &[&[436], &[436], &[436], &[436]] }, // end_stone_bricks
    Recipe { shaped: true, width: 3, height: 3, result: 440, count: 1, cells: &[&[322], &[322], &[322], &[322], &[1129], &[322], &[322], &[322], &[322]] }, // ender_chest
    Recipe { shaped: true, width: 1, height: 2, result: 99, count: 1, cells: &[&[111], &[111]] }, // exposed_chiseled_copper
    Recipe { shaped: true, width: 3, height: 3, result: 1478, count: 4, cells: &[&[], &[95], &[], &[95], &[1117], &[95], &[], &[718], &[]] }, // exposed_copper_bulb
    Recipe { shaped: true, width: 3, height: 3, result: 1470, count: 4, cells: &[&[], &[95], &[], &[95], &[], &[95], &[], &[95], &[]] }, // exposed_copper_grate
    Recipe { shaped: true, width: 2, height: 2, result: 103, count: 4, cells: &[&[95], &[95], &[95], &[95]] }, // exposed_cut_copper
    Recipe { shaped: true, width: 3, height: 1, result: 111, count: 6, cells: &[&[103], &[103], &[103]] }, // exposed_cut_copper_slab
    Recipe { shaped: true, width: 3, height: 3, result: 107, count: 4, cells: &[&[103], &[], &[], &[103], &[103], &[], &[103], &[103], &[103]] }, // exposed_cut_copper_stairs
    Recipe { shaped: true, width: 3, height: 3, result: 1433, count: 2, cells: &[&[899], &[1433], &[899], &[899], &[436], &[899], &[899], &[899], &[899]] }, // eye_armor_trim_smithing_template
    Recipe { shaped: true, width: 3, height: 3, result: 1054, count: 1, cells: &[&[], &[], &[947], &[], &[947], &[949], &[947], &[], &[949]] }, // fishing_rod
    Recipe { shaped: true, width: 2, height: 3, result: 1359, count: 1, cells: &[&[983], &[983], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47]] }, // fletching_table
    Recipe { shaped: true, width: 3, height: 3, result: 1444, count: 2, cells: &[&[899], &[1444], &[899], &[899], &[1223], &[899], &[899], &[899], &[899]] }, // flow_armor_trim_smithing_template
    Recipe { shaped: true, width: 3, height: 2, result: 1227, count: 1, cells: &[&[1026], &[], &[1026], &[], &[1026], &[]] }, // flower_pot
    Recipe { shaped: true, width: 3, height: 3, result: 335, count: 1, cells: &[&[9, 35, 1386], &[9, 35, 1386], &[9, 35, 1386], &[9, 35, 1386], &[], &[9, 35, 1386], &[9, 35, 1386], &[9, 35, 1386], &[9, 35, 1386]] }, // furnace
    Recipe { shaped: true, width: 3, height: 2, result: 1121, count: 3, cells: &[&[195], &[], &[195], &[], &[195], &[]] }, // glass_bottle
    Recipe { shaped: true, width: 3, height: 2, result: 409, count: 16, cells: &[&[195], &[195], &[195], &[195], &[195], &[195]] }, // glass_pane
    Recipe { shaped: true, width: 3, height: 3, result: 1130, count: 1, cells: &[&[1119], &[1119], &[1119], &[1119], &[1107], &[1119], &[1119], &[1119], &[1119]] }, // glistering_melon_slice
    Recipe { shaped: true, width: 2, height: 2, result: 368, count: 1, cells: &[&[1057], &[1057], &[1057], &[1057]] }, // glowstone
    Recipe { shaped: true, width: 3, height: 3, result: 92, count: 1, cells: &[&[909], &[909], &[909], &[909], &[909], &[909], &[909], &[909], &[909]] }, // gold_block
    Recipe { shaped: true, width: 3, height: 3, result: 909, count: 1, cells: &[&[1119], &[1119], &[1119], &[1119], &[1119], &[1119], &[1119], &[1119], &[1119]] }, // gold_ingot
    Recipe { shaped: false, width: 1, height: 1, result: 909, count: 9, cells: &[&[92]] }, // gold_ingot
    Recipe { shaped: true, width: 3, height: 3, result: 987, count: 1, cells: &[&[909], &[909], &[909], &[909], &[894], &[909], &[909], &[909], &[909]] }, // golden_apple
    Recipe { shaped: true, width: 2, height: 3, result: 930, count: 1, cells: &[&[909], &[909], &[909], &[947], &[], &[947]] }, // golden_axe
    Recipe { shaped: true, width: 3, height: 2, result: 978, count: 1, cells: &[&[909], &[], &[909], &[909], &[], &[909]] }, // golden_boots
    Recipe { shaped: true, width: 3, height: 3, result: 1233, count: 1, cells: &[&[1119], &[1119], &[1119], &[1119], &[1228], &[1119], &[1119], &[1119], &[1119]] }, // golden_carrot
    Recipe { shaped: true, width: 3, height: 3, result: 976, count: 1, cells: &[&[909], &[], &[909], &[909], &[909], &[909], &[909], &[909], &[909]] }, // golden_chestplate
    Recipe { shaped: true, width: 3, height: 3, result: 230, count: 1, cells: &[&[1119], &[1119], &[1119], &[1119], &[229], &[1119], &[1119], &[1119], &[1119]] }, // golden_dandelion
    Recipe { shaped: true, width: 3, height: 2, result: 975, count: 1, cells: &[&[909], &[909], &[909], &[909], &[], &[909]] }, // golden_helmet
    Recipe { shaped: true, width: 2, height: 3, result: 931, count: 1, cells: &[&[909], &[909], &[], &[947], &[], &[947]] }, // golden_hoe
    Recipe { shaped: true, width: 3, height: 3, result: 977, count: 1, cells: &[&[909], &[909], &[909], &[909], &[], &[909], &[909], &[], &[909]] }, // golden_leggings
    Recipe { shaped: true, width: 3, height: 3, result: 929, count: 1, cells: &[&[909], &[909], &[909], &[], &[947], &[], &[], &[947], &[]] }, // golden_pickaxe
    Recipe { shaped: true, width: 1, height: 3, result: 928, count: 1, cells: &[&[909], &[947], &[947]] }, // golden_shovel
    Recipe { shaped: true, width: 3, height: 3, result: 1301, count: 1, cells: &[&[], &[], &[909], &[], &[947], &[], &[947], &[], &[]] }, // golden_spear
    Recipe { shaped: true, width: 1, height: 3, result: 927, count: 1, cells: &[&[909], &[909], &[947]] }, // golden_sword
    Recipe { shaped: true, width: 3, height: 1, result: 708, count: 6, cells: &[&[2], &[2], &[2]] }, // granite_slab
    Recipe { shaped: true, width: 3, height: 3, result: 691, count: 4, cells: &[&[2], &[], &[], &[2], &[2], &[], &[2], &[2], &[2]] }, // granite_stairs
    Recipe { shaped: true, width: 3, height: 2, result: 463, count: 6, cells: &[&[2], &[2], &[2], &[2], &[2], &[2]] }, // granite_wall
    Recipe { shaped: true, width: 3, height: 3, result: 1274, count: 1, cells: &[&[220], &[220], &[220], &[220], &[220], &[220], &[], &[947], &[]] }, // gray_banner
    Recipe { shaped: true, width: 3, height: 2, result: 1094, count: 1, cells: &[&[220], &[220], &[220], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47]] }, // gray_bed
    Recipe { shaped: false, width: 2, height: 1, result: 1094, count: 1, cells: &[&[1074], &[1102]] }, // gray_bed
    Recipe { shaped: true, width: 2, height: 1, result: 513, count: 3, cells: &[&[220], &[220]] }, // gray_carpet
    Recipe { shaped: false, width: 2, height: 1, result: 513, count: 1, cells: &[&[1074], &[521]] }, // gray_carpet
    Recipe { shaped: true, width: 3, height: 2, result: 846, count: 1, cells: &[&[1018], &[1018], &[1018], &[195], &[220], &[195]] }, // gray_harness
    Recipe { shaped: false, width: 2, height: 1, result: 846, count: 1, cells: &[&[1074], &[854]] }, // gray_harness
    Recipe { shaped: true, width: 3, height: 3, result: 538, count: 8, cells: &[&[195], &[195], &[195], &[195], &[1074], &[195], &[195], &[195], &[195]] }, // gray_stained_glass
    Recipe { shaped: true, width: 3, height: 2, result: 554, count: 16, cells: &[&[538], &[538], &[538], &[538], &[538], &[538]] }, // gray_stained_glass_pane
    Recipe { shaped: true, width: 3, height: 3, result: 554, count: 8, cells: &[&[409], &[409], &[409], &[409], &[1074], &[409], &[409], &[409], &[409]] }, // gray_stained_glass_pane
    Recipe { shaped: true, width: 3, height: 3, result: 494, count: 8, cells: &[&[522], &[522], &[522], &[522], &[1074], &[522], &[522], &[522], &[522]] }, // gray_terracotta
    Recipe { shaped: true, width: 3, height: 3, result: 1280, count: 1, cells: &[&[226], &[226], &[226], &[226], &[226], &[226], &[], &[947], &[]] }, // green_banner
    Recipe { shaped: true, width: 3, height: 2, result: 1100, count: 1, cells: &[&[226], &[226], &[226], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47]] }, // green_bed
    Recipe { shaped: false, width: 2, height: 1, result: 1100, count: 1, cells: &[&[1080], &[1102]] }, // green_bed
    Recipe { shaped: true, width: 2, height: 1, result: 519, count: 3, cells: &[&[226], &[226]] }, // green_carpet
    Recipe { shaped: false, width: 2, height: 1, result: 519, count: 1, cells: &[&[1080], &[521]] }, // green_carpet
    Recipe { shaped: true, width: 3, height: 2, result: 852, count: 1, cells: &[&[1018], &[1018], &[1018], &[195], &[226], &[195]] }, // green_harness
    Recipe { shaped: false, width: 2, height: 1, result: 852, count: 1, cells: &[&[1080], &[854]] }, // green_harness
    Recipe { shaped: true, width: 3, height: 3, result: 544, count: 8, cells: &[&[195], &[195], &[195], &[195], &[1080], &[195], &[195], &[195], &[195]] }, // green_stained_glass
    Recipe { shaped: true, width: 3, height: 2, result: 560, count: 16, cells: &[&[544], &[544], &[544], &[544], &[544], &[544]] }, // green_stained_glass_pane
    Recipe { shaped: true, width: 3, height: 3, result: 560, count: 8, cells: &[&[409], &[409], &[409], &[409], &[1080], &[409], &[409], &[409], &[409]] }, // green_stained_glass_pane
    Recipe { shaped: true, width: 3, height: 3, result: 500, count: 8, cells: &[&[522], &[522], &[522], &[522], &[1080], &[522], &[522], &[522], &[522]] }, // green_terracotta
    Recipe { shaped: true, width: 3, height: 2, result: 1360, count: 1, cells: &[&[947], &[284], &[947], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47]] }, // grindstone
    Recipe { shaped: true, width: 2, height: 1, result: 767, count: 1, cells: &[&[905], &[905]] }, // heavy_weighted_pressure_plate
    Recipe { shaped: true, width: 2, height: 2, result: 726, count: 1, cells: &[&[1382], &[1382], &[1382], &[1382]] }, // honey_block
    Recipe { shaped: true, width: 2, height: 2, result: 1383, count: 1, cells: &[&[1379], &[1379], &[1379], &[1379]] }, // honeycomb_block
    Recipe { shaped: true, width: 3, height: 3, result: 728, count: 1, cells: &[&[905], &[], &[905], &[905], &[332], &[905], &[], &[905], &[]] }, // hopper
    Recipe { shaped: true, width: 3, height: 3, result: 1443, count: 2, cells: &[&[899], &[1443], &[899], &[899], &[522], &[899], &[899], &[899], &[899]] }, // host_armor_trim_smithing_template
    Recipe { shaped: true, width: 2, height: 3, result: 935, count: 1, cells: &[&[905], &[905], &[905], &[947], &[], &[947]] }, // iron_axe
    Recipe { shaped: true, width: 3, height: 2, result: 391, count: 16, cells: &[&[905], &[905], &[905], &[905], &[905], &[905]] }, // iron_bars
    Recipe { shaped: true, width: 3, height: 3, result: 90, count: 1, cells: &[&[905], &[905], &[905], &[905], &[905], &[905], &[905], &[905], &[905]] }, // iron_block
    Recipe { shaped: true, width: 3, height: 2, result: 970, count: 1, cells: &[&[905], &[], &[905], &[905], &[], &[905]] }, // iron_boots
    Recipe { shaped: true, width: 1, height: 3, result: 400, count: 1, cells: &[&[1306], &[905], &[1306]] }, // iron_chain
    Recipe { shaped: true, width: 3, height: 3, result: 968, count: 1, cells: &[&[905], &[], &[905], &[905], &[905], &[905], &[905], &[905], &[905]] }, // iron_chestplate
    Recipe { shaped: true, width: 2, height: 3, result: 780, count: 3, cells: &[&[905], &[905], &[905], &[905], &[905], &[905]] }, // iron_door
    Recipe { shaped: true, width: 3, height: 2, result: 967, count: 1, cells: &[&[905], &[905], &[905], &[905], &[], &[905]] }, // iron_helmet
    Recipe { shaped: true, width: 2, height: 3, result: 936, count: 1, cells: &[&[905], &[905], &[], &[947], &[], &[947]] }, // iron_hoe
    Recipe { shaped: true, width: 3, height: 3, result: 905, count: 1, cells: &[&[1306], &[1306], &[1306], &[1306], &[1306], &[1306], &[1306], &[1306], &[1306]] }, // iron_ingot
    Recipe { shaped: false, width: 1, height: 1, result: 905, count: 9, cells: &[&[90]] }, // iron_ingot
    Recipe { shaped: true, width: 3, height: 3, result: 969, count: 1, cells: &[&[905], &[905], &[905], &[905], &[], &[905], &[905], &[], &[905]] }, // iron_leggings
    Recipe { shaped: true, width: 3, height: 3, result: 934, count: 1, cells: &[&[905], &[905], &[905], &[], &[947], &[], &[], &[947], &[]] }, // iron_pickaxe
    Recipe { shaped: true, width: 1, height: 3, result: 933, count: 1, cells: &[&[905], &[947], &[947]] }, // iron_shovel
    Recipe { shaped: true, width: 3, height: 3, result: 1300, count: 1, cells: &[&[], &[], &[905], &[], &[947], &[], &[947], &[], &[]] }, // iron_spear
    Recipe { shaped: true, width: 1, height: 3, result: 932, count: 1, cells: &[&[905], &[905], &[947]] }, // iron_sword
    Recipe { shaped: true, width: 2, height: 2, result: 801, count: 1, cells: &[&[905], &[905], &[905], &[905]] }, // iron_trapdoor
    Recipe { shaped: true, width: 3, height: 3, result: 1225, count: 1, cells: &[&[947], &[947], &[947], &[947], &[1018], &[947], &[947], &[947], &[947]] }, // item_frame
    Recipe { shaped: true, width: 1, height: 2, result: 359, count: 1, cells: &[&[358], &[323]] }, // jack_o_lantern
    Recipe { shaped: true, width: 3, height: 3, result: 344, count: 1, cells: &[&[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[899], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47]] }, // jukebox
    Recipe { shaped: true, width: 3, height: 2, result: 870, count: 1, cells: &[&[39], &[], &[39], &[39], &[39], &[39]] }, // jungle_boat
    Recipe { shaped: true, width: 2, height: 3, result: 784, count: 3, cells: &[&[39], &[39], &[39], &[39], &[39], &[39]] }, // jungle_door
    Recipe { shaped: true, width: 3, height: 2, result: 348, count: 3, cells: &[&[39], &[947], &[39], &[39], &[947], &[39]] }, // jungle_fence
    Recipe { shaped: true, width: 3, height: 2, result: 825, count: 1, cells: &[&[947], &[39], &[947], &[947], &[39], &[947]] }, // jungle_fence_gate
    Recipe { shaped: true, width: 3, height: 3, result: 1004, count: 6, cells: &[&[400], &[], &[400], &[151], &[151], &[151], &[151], &[151], &[151]] }, // jungle_hanging_sign
    Recipe { shaped: true, width: 2, height: 1, result: 771, count: 1, cells: &[&[39], &[39]] }, // jungle_pressure_plate
    Recipe { shaped: true, width: 3, height: 3, result: 312, count: 6, cells: &[&[151], &[151], &[151], &[], &[], &[], &[151], &[151], &[151]] }, // jungle_shelf
    Recipe { shaped: true, width: 3, height: 3, result: 992, count: 3, cells: &[&[39], &[39], &[39], &[39], &[39], &[39], &[], &[947], &[]] }, // jungle_sign
    Recipe { shaped: true, width: 3, height: 1, result: 274, count: 6, cells: &[&[39], &[39], &[39]] }, // jungle_slab
    Recipe { shaped: true, width: 3, height: 3, result: 445, count: 4, cells: &[&[39], &[], &[], &[39], &[39], &[], &[39], &[39], &[39]] }, // jungle_stairs
    Recipe { shaped: true, width: 3, height: 2, result: 805, count: 2, cells: &[&[39], &[39], &[39], &[39], &[39], &[39]] }, // jungle_trapdoor
    Recipe { shaped: true, width: 2, height: 2, result: 174, count: 3, cells: &[&[137], &[137], &[137], &[137]] }, // jungle_wood
    Recipe { shaped: true, width: 3, height: 3, result: 336, count: 3, cells: &[&[947], &[], &[947], &[947], &[947], &[947], &[947], &[], &[947]] }, // ladder
    Recipe { shaped: true, width: 3, height: 3, result: 1364, count: 1, cells: &[&[1306], &[1306], &[1306], &[1306], &[323], &[1306], &[1306], &[1306], &[1306]] }, // lantern
    Recipe { shaped: true, width: 3, height: 3, result: 197, count: 1, cells: &[&[901], &[901], &[901], &[901], &[901], &[901], &[901], &[901], &[901]] }, // lapis_block
    Recipe { shaped: true, width: 3, height: 3, result: 1262, count: 2, cells: &[&[949], &[949], &[], &[949], &[949], &[], &[], &[], &[949]] }, // lead
    Recipe { shaped: true, width: 2, height: 2, result: 1018, count: 1, cells: &[&[1254], &[1254], &[1254], &[1254]] }, // leather
    Recipe { shaped: true, width: 3, height: 2, result: 958, count: 1, cells: &[&[1018], &[], &[1018], &[1018], &[], &[1018]] }, // leather_boots
    Recipe { shaped: true, width: 3, height: 3, result: 956, count: 1, cells: &[&[1018], &[], &[1018], &[1018], &[1018], &[1018], &[1018], &[1018], &[1018]] }, // leather_chestplate
    Recipe { shaped: true, width: 3, height: 2, result: 955, count: 1, cells: &[&[1018], &[1018], &[1018], &[1018], &[], &[1018]] }, // leather_helmet
    Recipe { shaped: true, width: 3, height: 3, result: 1261, count: 1, cells: &[&[1018], &[], &[1018], &[1018], &[1018], &[1018], &[1018], &[], &[1018]] }, // leather_horse_armor
    Recipe { shaped: true, width: 3, height: 3, result: 957, count: 1, cells: &[&[1018], &[1018], &[1018], &[1018], &[], &[1018], &[1018], &[], &[1018]] }, // leather_leggings
    Recipe { shaped: true, width: 3, height: 3, result: 731, count: 1, cells: &[&[271, 272, 273, 274, 275, 276, 277, 278, 279, 280, 282, 283], &[271, 272, 273, 274, 275, 276, 277, 278, 279, 280, 282, 283], &[271, 272, 273, 274, 275, 276, 277, 278, 279, 280, 282, 283], &[], &[318], &[], &[], &[271, 272, 273, 274, 275, 276, 277, 278, 279, 280, 282, 283], &[]] }, // lectern
    Recipe { shaped: true, width: 1, height: 2, result: 733, count: 1, cells: &[&[947], &[35]] }, // lever
    Recipe { shaped: true, width: 3, height: 3, result: 1270, count: 1, cells: &[&[216], &[216], &[216], &[216], &[216], &[216], &[], &[947], &[]] }, // light_blue_banner
    Recipe { shaped: true, width: 3, height: 2, result: 1090, count: 1, cells: &[&[216], &[216], &[216], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47]] }, // light_blue_bed
    Recipe { shaped: false, width: 2, height: 1, result: 1090, count: 1, cells: &[&[1070], &[1102]] }, // light_blue_bed
    Recipe { shaped: true, width: 2, height: 1, result: 509, count: 3, cells: &[&[216], &[216]] }, // light_blue_carpet
    Recipe { shaped: false, width: 2, height: 1, result: 509, count: 1, cells: &[&[1070], &[521]] }, // light_blue_carpet
    Recipe { shaped: true, width: 3, height: 2, result: 842, count: 1, cells: &[&[1018], &[1018], &[1018], &[195], &[216], &[195]] }, // light_blue_harness
    Recipe { shaped: false, width: 2, height: 1, result: 842, count: 1, cells: &[&[1070], &[854]] }, // light_blue_harness
    Recipe { shaped: true, width: 3, height: 3, result: 534, count: 8, cells: &[&[195], &[195], &[195], &[195], &[1070], &[195], &[195], &[195], &[195]] }, // light_blue_stained_glass
    Recipe { shaped: true, width: 3, height: 2, result: 550, count: 16, cells: &[&[534], &[534], &[534], &[534], &[534], &[534]] }, // light_blue_stained_glass_pane
    Recipe { shaped: true, width: 3, height: 3, result: 550, count: 8, cells: &[&[409], &[409], &[409], &[409], &[1070], &[409], &[409], &[409], &[409]] }, // light_blue_stained_glass_pane
    Recipe { shaped: true, width: 3, height: 3, result: 490, count: 8, cells: &[&[522], &[522], &[522], &[522], &[1070], &[522], &[522], &[522], &[522]] }, // light_blue_terracotta
    Recipe { shaped: true, width: 3, height: 3, result: 1275, count: 1, cells: &[&[221], &[221], &[221], &[221], &[221], &[221], &[], &[947], &[]] }, // light_gray_banner
    Recipe { shaped: true, width: 3, height: 2, result: 1095, count: 1, cells: &[&[221], &[221], &[221], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47]] }, // light_gray_bed
    Recipe { shaped: false, width: 2, height: 1, result: 1095, count: 1, cells: &[&[1075], &[1102]] }, // light_gray_bed
    Recipe { shaped: true, width: 2, height: 1, result: 514, count: 3, cells: &[&[221], &[221]] }, // light_gray_carpet
    Recipe { shaped: false, width: 2, height: 1, result: 514, count: 1, cells: &[&[1075], &[521]] }, // light_gray_carpet
    Recipe { shaped: true, width: 3, height: 2, result: 847, count: 1, cells: &[&[1018], &[1018], &[1018], &[195], &[221], &[195]] }, // light_gray_harness
    Recipe { shaped: false, width: 2, height: 1, result: 847, count: 1, cells: &[&[1075], &[854]] }, // light_gray_harness
    Recipe { shaped: true, width: 3, height: 3, result: 539, count: 8, cells: &[&[195], &[195], &[195], &[195], &[1075], &[195], &[195], &[195], &[195]] }, // light_gray_stained_glass
    Recipe { shaped: true, width: 3, height: 2, result: 555, count: 16, cells: &[&[539], &[539], &[539], &[539], &[539], &[539]] }, // light_gray_stained_glass_pane
    Recipe { shaped: true, width: 3, height: 3, result: 555, count: 8, cells: &[&[409], &[409], &[409], &[409], &[1075], &[409], &[409], &[409], &[409]] }, // light_gray_stained_glass_pane
    Recipe { shaped: true, width: 3, height: 3, result: 495, count: 8, cells: &[&[522], &[522], &[522], &[522], &[1075], &[522], &[522], &[522], &[522]] }, // light_gray_terracotta
    Recipe { shaped: true, width: 2, height: 1, result: 766, count: 1, cells: &[&[909], &[909]] }, // light_weighted_pressure_plate
    Recipe { shaped: true, width: 1, height: 3, result: 734, count: 1, cells: &[&[907], &[907], &[907]] }, // lightning_rod
    Recipe { shaped: true, width: 3, height: 3, result: 1272, count: 1, cells: &[&[218], &[218], &[218], &[218], &[218], &[218], &[], &[947], &[]] }, // lime_banner
    Recipe { shaped: true, width: 3, height: 2, result: 1092, count: 1, cells: &[&[218], &[218], &[218], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47]] }, // lime_bed
    Recipe { shaped: false, width: 2, height: 1, result: 1092, count: 1, cells: &[&[1072], &[1102]] }, // lime_bed
    Recipe { shaped: true, width: 2, height: 1, result: 511, count: 3, cells: &[&[218], &[218]] }, // lime_carpet
    Recipe { shaped: false, width: 2, height: 1, result: 511, count: 1, cells: &[&[1072], &[521]] }, // lime_carpet
    Recipe { shaped: true, width: 3, height: 2, result: 844, count: 1, cells: &[&[1018], &[1018], &[1018], &[195], &[218], &[195]] }, // lime_harness
    Recipe { shaped: false, width: 2, height: 1, result: 844, count: 1, cells: &[&[1072], &[854]] }, // lime_harness
    Recipe { shaped: true, width: 3, height: 3, result: 536, count: 8, cells: &[&[195], &[195], &[195], &[195], &[1072], &[195], &[195], &[195], &[195]] }, // lime_stained_glass
    Recipe { shaped: true, width: 3, height: 2, result: 552, count: 16, cells: &[&[536], &[536], &[536], &[536], &[536], &[536]] }, // lime_stained_glass_pane
    Recipe { shaped: true, width: 3, height: 3, result: 552, count: 8, cells: &[&[409], &[409], &[409], &[409], &[1072], &[409], &[409], &[409], &[409]] }, // lime_stained_glass_pane
    Recipe { shaped: true, width: 3, height: 3, result: 492, count: 8, cells: &[&[522], &[522], &[522], &[522], &[1072], &[522], &[522], &[522], &[522]] }, // lime_terracotta
    Recipe { shaped: true, width: 3, height: 3, result: 1384, count: 1, cells: &[&[379], &[379], &[379], &[379], &[905], &[379], &[379], &[379], &[379]] }, // lodestone
    Recipe { shaped: true, width: 2, height: 2, result: 1342, count: 1, cells: &[&[949], &[949], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47]] }, // loom
    Recipe { shaped: true, width: 1, height: 2, result: 1224, count: 1, cells: &[&[87], &[1223]] }, // mace
    Recipe { shaped: true, width: 3, height: 3, result: 1269, count: 1, cells: &[&[215], &[215], &[215], &[215], &[215], &[215], &[], &[947], &[]] }, // magenta_banner
    Recipe { shaped: true, width: 3, height: 2, result: 1089, count: 1, cells: &[&[215], &[215], &[215], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47]] }, // magenta_bed
    Recipe { shaped: false, width: 2, height: 1, result: 1089, count: 1, cells: &[&[1069], &[1102]] }, // magenta_bed
    Recipe { shaped: true, width: 2, height: 1, result: 508, count: 3, cells: &[&[215], &[215]] }, // magenta_carpet
    Recipe { shaped: false, width: 2, height: 1, result: 508, count: 1, cells: &[&[1069], &[521]] }, // magenta_carpet
    Recipe { shaped: true, width: 3, height: 2, result: 841, count: 1, cells: &[&[1018], &[1018], &[1018], &[195], &[215], &[195]] }, // magenta_harness
    Recipe { shaped: false, width: 2, height: 1, result: 841, count: 1, cells: &[&[1069], &[854]] }, // magenta_harness
    Recipe { shaped: true, width: 3, height: 3, result: 533, count: 8, cells: &[&[195], &[195], &[195], &[195], &[1069], &[195], &[195], &[195], &[195]] }, // magenta_stained_glass
    Recipe { shaped: true, width: 3, height: 2, result: 549, count: 16, cells: &[&[533], &[533], &[533], &[533], &[533], &[533]] }, // magenta_stained_glass_pane
    Recipe { shaped: true, width: 3, height: 3, result: 549, count: 8, cells: &[&[409], &[409], &[409], &[409], &[1069], &[409], &[409], &[409], &[409]] }, // magenta_stained_glass_pane
    Recipe { shaped: true, width: 3, height: 3, result: 489, count: 8, cells: &[&[522], &[522], &[522], &[522], &[1069], &[522], &[522], &[522], &[522]] }, // magenta_terracotta
    Recipe { shaped: true, width: 2, height: 2, result: 576, count: 1, cells: &[&[1126], &[1126], &[1126], &[1126]] }, // magma_block
    Recipe { shaped: true, width: 3, height: 2, result: 880, count: 1, cells: &[&[44], &[], &[44], &[44], &[44], &[44]] }, // mangrove_boat
    Recipe { shaped: true, width: 2, height: 3, result: 789, count: 3, cells: &[&[44], &[44], &[44], &[44], &[44], &[44]] }, // mangrove_door
    Recipe { shaped: true, width: 3, height: 2, result: 353, count: 3, cells: &[&[44], &[947], &[44], &[44], &[947], &[44]] }, // mangrove_fence
    Recipe { shaped: true, width: 3, height: 2, result: 830, count: 1, cells: &[&[947], &[44], &[947], &[947], &[44], &[947]] }, // mangrove_fence_gate
    Recipe { shaped: true, width: 3, height: 3, result: 1009, count: 6, cells: &[&[400], &[], &[400], &[156], &[156], &[156], &[156], &[156], &[156]] }, // mangrove_hanging_sign
    Recipe { shaped: true, width: 2, height: 1, result: 776, count: 1, cells: &[&[44], &[44]] }, // mangrove_pressure_plate
    Recipe { shaped: true, width: 3, height: 3, result: 313, count: 6, cells: &[&[156], &[156], &[156], &[], &[], &[], &[156], &[156], &[156]] }, // mangrove_shelf
    Recipe { shaped: true, width: 3, height: 3, result: 997, count: 3, cells: &[&[44], &[44], &[44], &[44], &[44], &[44], &[], &[947], &[]] }, // mangrove_sign
    Recipe { shaped: true, width: 3, height: 1, result: 279, count: 6, cells: &[&[44], &[44], &[44]] }, // mangrove_slab
    Recipe { shaped: true, width: 3, height: 3, result: 450, count: 4, cells: &[&[44], &[], &[], &[44], &[44], &[], &[44], &[44], &[44]] }, // mangrove_stairs
    Recipe { shaped: true, width: 3, height: 2, result: 810, count: 2, cells: &[&[44], &[44], &[44], &[44], &[44], &[44]] }, // mangrove_trapdoor
    Recipe { shaped: true, width: 2, height: 2, result: 179, count: 3, cells: &[&[142], &[142], &[142], &[142]] }, // mangrove_wood
    Recipe { shaped: true, width: 3, height: 3, result: 1232, count: 1, cells: &[&[1029], &[1029], &[1029], &[1029], &[1035], &[1029], &[1029], &[1029], &[1029]] }, // map
    Recipe { shaped: true, width: 3, height: 2, result: 855, count: 1, cells: &[&[905], &[], &[905], &[905], &[905], &[905]] }, // minecart
    Recipe { shaped: true, width: 2, height: 1, result: 262, count: 3, cells: &[&[263], &[263]] }, // moss_carpet
    Recipe { shaped: true, width: 3, height: 1, result: 704, count: 6, cells: &[&[321], &[321], &[321]] }, // mossy_cobblestone_slab
    Recipe { shaped: true, width: 3, height: 3, result: 686, count: 4, cells: &[&[321], &[], &[], &[321], &[321], &[], &[321], &[321], &[321]] }, // mossy_cobblestone_stairs
    Recipe { shaped: true, width: 3, height: 2, result: 458, count: 6, cells: &[&[321], &[321], &[321], &[321], &[321], &[321]] }, // mossy_cobblestone_wall
    Recipe { shaped: true, width: 3, height: 1, result: 702, count: 6, cells: &[&[377], &[377], &[377]] }, // mossy_stone_brick_slab
    Recipe { shaped: true, width: 3, height: 3, result: 684, count: 4, cells: &[&[377], &[], &[], &[377], &[377], &[], &[377], &[377], &[377]] }, // mossy_stone_brick_stairs
    Recipe { shaped: true, width: 3, height: 2, result: 462, count: 6, cells: &[&[377], &[377], &[377], &[377], &[377], &[377]] }, // mossy_stone_brick_wall
    Recipe { shaped: true, width: 3, height: 1, result: 292, count: 6, cells: &[&[381], &[381], &[381]] }, // mud_brick_slab
    Recipe { shaped: true, width: 3, height: 3, result: 422, count: 4, cells: &[&[381], &[], &[], &[381], &[381], &[], &[381], &[381], &[381]] }, // mud_brick_stairs
    Recipe { shaped: true, width: 3, height: 2, result: 465, count: 6, cells: &[&[381], &[381], &[381], &[381], &[381], &[381]] }, // mud_brick_wall
    Recipe { shaped: true, width: 2, height: 2, result: 381, count: 4, cells: &[&[380], &[380], &[380], &[380]] }, // mud_bricks
    Recipe { shaped: true, width: 2, height: 2, result: 1263, count: 1, cells: &[&[], &[1119, 1306, 1307], &[1029], &[]] }, // name_tag
    Recipe { shaped: true, width: 3, height: 2, result: 428, count: 6, cells: &[&[425], &[1246], &[425], &[425], &[1246], &[425]] }, // nether_brick_fence
    Recipe { shaped: true, width: 3, height: 1, result: 293, count: 6, cells: &[&[425], &[425], &[425]] }, // nether_brick_slab
    Recipe { shaped: true, width: 3, height: 3, result: 429, count: 4, cells: &[&[425], &[], &[], &[425], &[425], &[], &[425], &[425], &[425]] }, // nether_brick_stairs
    Recipe { shaped: true, width: 3, height: 2, result: 466, count: 6, cells: &[&[425], &[425], &[425], &[425], &[425], &[425]] }, // nether_brick_wall
    Recipe { shaped: true, width: 2, height: 2, result: 425, count: 1, cells: &[&[1246], &[1246], &[1246], &[1246]] }, // nether_bricks
    Recipe { shaped: true, width: 3, height: 3, result: 94, count: 1, cells: &[&[910], &[910], &[910], &[910], &[910], &[910], &[910], &[910], &[910]] }, // netherite_block
    Recipe { shaped: true, width: 3, height: 3, result: 1427, count: 2, cells: &[&[899], &[1427], &[899], &[899], &[360], &[899], &[899], &[899], &[899]] }, // netherite_upgrade_smithing_template
    Recipe { shaped: true, width: 3, height: 3, result: 749, count: 1, cells: &[&[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[718], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47]] }, // note_block
    Recipe { shaped: true, width: 3, height: 2, result: 864, count: 1, cells: &[&[36], &[], &[36], &[36], &[36], &[36]] }, // oak_boat
    Recipe { shaped: true, width: 2, height: 3, result: 781, count: 3, cells: &[&[36], &[36], &[36], &[36], &[36], &[36]] }, // oak_door
    Recipe { shaped: true, width: 3, height: 2, result: 345, count: 3, cells: &[&[36], &[947], &[36], &[36], &[947], &[36]] }, // oak_fence
    Recipe { shaped: true, width: 3, height: 2, result: 822, count: 1, cells: &[&[947], &[36], &[947], &[947], &[36], &[947]] }, // oak_fence_gate
    Recipe { shaped: true, width: 3, height: 3, result: 1001, count: 6, cells: &[&[400], &[], &[400], &[148], &[148], &[148], &[148], &[148], &[148]] }, // oak_hanging_sign
    Recipe { shaped: true, width: 2, height: 1, result: 768, count: 1, cells: &[&[36], &[36]] }, // oak_pressure_plate
    Recipe { shaped: true, width: 3, height: 3, result: 314, count: 6, cells: &[&[148], &[148], &[148], &[], &[], &[], &[148], &[148], &[148]] }, // oak_shelf
    Recipe { shaped: true, width: 3, height: 3, result: 989, count: 3, cells: &[&[36], &[36], &[36], &[36], &[36], &[36], &[], &[947], &[]] }, // oak_sign
    Recipe { shaped: true, width: 3, height: 1, result: 271, count: 6, cells: &[&[36], &[36], &[36]] }, // oak_slab
    Recipe { shaped: true, width: 3, height: 3, result: 442, count: 4, cells: &[&[36], &[], &[], &[36], &[36], &[], &[36], &[36], &[36]] }, // oak_stairs
    Recipe { shaped: true, width: 3, height: 2, result: 802, count: 2, cells: &[&[36], &[36], &[36], &[36], &[36], &[36]] }, // oak_trapdoor
    Recipe { shaped: true, width: 2, height: 2, result: 171, count: 3, cells: &[&[134], &[134], &[134], &[134]] }, // oak_wood
    Recipe { shaped: true, width: 3, height: 3, result: 727, count: 1, cells: &[&[35], &[35], &[35], &[718], &[718], &[902], &[35], &[35], &[35]] }, // observer
    Recipe { shaped: true, width: 3, height: 3, result: 1268, count: 1, cells: &[&[214], &[214], &[214], &[214], &[214], &[214], &[], &[947], &[]] }, // orange_banner
    Recipe { shaped: true, width: 3, height: 2, result: 1088, count: 1, cells: &[&[214], &[214], &[214], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47]] }, // orange_bed
    Recipe { shaped: false, width: 2, height: 1, result: 1088, count: 1, cells: &[&[1068], &[1102]] }, // orange_bed
    Recipe { shaped: true, width: 2, height: 1, result: 507, count: 3, cells: &[&[214], &[214]] }, // orange_carpet
    Recipe { shaped: false, width: 2, height: 1, result: 507, count: 1, cells: &[&[1068], &[521]] }, // orange_carpet
    Recipe { shaped: true, width: 3, height: 2, result: 840, count: 1, cells: &[&[1018], &[1018], &[1018], &[195], &[214], &[195]] }, // orange_harness
    Recipe { shaped: false, width: 2, height: 1, result: 840, count: 1, cells: &[&[1068], &[854]] }, // orange_harness
    Recipe { shaped: true, width: 3, height: 3, result: 532, count: 8, cells: &[&[195], &[195], &[195], &[195], &[1068], &[195], &[195], &[195], &[195]] }, // orange_stained_glass
    Recipe { shaped: true, width: 3, height: 2, result: 548, count: 16, cells: &[&[532], &[532], &[532], &[532], &[532], &[532]] }, // orange_stained_glass_pane
    Recipe { shaped: true, width: 3, height: 3, result: 548, count: 8, cells: &[&[409], &[409], &[409], &[409], &[1068], &[409], &[409], &[409], &[409]] }, // orange_stained_glass_pane
    Recipe { shaped: true, width: 3, height: 3, result: 488, count: 8, cells: &[&[522], &[522], &[522], &[522], &[1068], &[522], &[522], &[522], &[522]] }, // orange_terracotta
    Recipe { shaped: true, width: 1, height: 2, result: 101, count: 1, cells: &[&[113], &[113]] }, // oxidized_chiseled_copper
    Recipe { shaped: true, width: 3, height: 3, result: 1480, count: 4, cells: &[&[], &[97], &[], &[97], &[1117], &[97], &[], &[718], &[]] }, // oxidized_copper_bulb
    Recipe { shaped: true, width: 3, height: 3, result: 1472, count: 4, cells: &[&[], &[97], &[], &[97], &[], &[97], &[], &[97], &[]] }, // oxidized_copper_grate
    Recipe { shaped: true, width: 2, height: 2, result: 105, count: 4, cells: &[&[97], &[97], &[97], &[97]] }, // oxidized_cut_copper
    Recipe { shaped: true, width: 3, height: 1, result: 113, count: 6, cells: &[&[105], &[105], &[105]] }, // oxidized_cut_copper_slab
    Recipe { shaped: true, width: 3, height: 3, result: 109, count: 4, cells: &[&[105], &[], &[], &[105], &[105], &[], &[105], &[105], &[105]] }, // oxidized_cut_copper_stairs
    Recipe { shaped: true, width: 3, height: 3, result: 986, count: 1, cells: &[&[947], &[947], &[947], &[947], &[213, 214, 215, 216, 217, 218, 219, 220, 221, 222, 223, 224, 225, 226, 227, 228], &[947], &[947], &[947], &[947]] }, // painting
    Recipe { shaped: true, width: 2, height: 1, result: 264, count: 3, cells: &[&[266], &[266]] }, // pale_moss_carpet
    Recipe { shaped: true, width: 3, height: 2, result: 878, count: 1, cells: &[&[43], &[], &[43], &[43], &[43], &[43]] }, // pale_oak_boat
    Recipe { shaped: true, width: 2, height: 3, result: 788, count: 3, cells: &[&[43], &[43], &[43], &[43], &[43], &[43]] }, // pale_oak_door
    Recipe { shaped: true, width: 3, height: 2, result: 352, count: 3, cells: &[&[43], &[947], &[43], &[43], &[947], &[43]] }, // pale_oak_fence
    Recipe { shaped: true, width: 3, height: 2, result: 829, count: 1, cells: &[&[947], &[43], &[947], &[947], &[43], &[947]] }, // pale_oak_fence_gate
    Recipe { shaped: true, width: 3, height: 3, result: 1008, count: 6, cells: &[&[400], &[], &[400], &[155], &[155], &[155], &[155], &[155], &[155]] }, // pale_oak_hanging_sign
    Recipe { shaped: true, width: 2, height: 1, result: 775, count: 1, cells: &[&[43], &[43]] }, // pale_oak_pressure_plate
    Recipe { shaped: true, width: 3, height: 3, result: 315, count: 6, cells: &[&[155], &[155], &[155], &[], &[], &[], &[155], &[155], &[155]] }, // pale_oak_shelf
    Recipe { shaped: true, width: 3, height: 3, result: 996, count: 3, cells: &[&[43], &[43], &[43], &[43], &[43], &[43], &[], &[947], &[]] }, // pale_oak_sign
    Recipe { shaped: true, width: 3, height: 1, result: 278, count: 6, cells: &[&[43], &[43], &[43]] }, // pale_oak_slab
    Recipe { shaped: true, width: 3, height: 3, result: 449, count: 4, cells: &[&[43], &[], &[], &[43], &[43], &[], &[43], &[43], &[43]] }, // pale_oak_stairs
    Recipe { shaped: true, width: 3, height: 2, result: 809, count: 2, cells: &[&[43], &[43], &[43], &[43], &[43], &[43]] }, // pale_oak_trapdoor
    Recipe { shaped: true, width: 2, height: 2, result: 177, count: 3, cells: &[&[140], &[140], &[140], &[140]] }, // pale_oak_wood
    Recipe { shaped: true, width: 3, height: 1, result: 1029, count: 3, cells: &[&[257], &[257], &[257]] }, // paper
    Recipe { shaped: true, width: 3, height: 3, result: 1273, count: 1, cells: &[&[219], &[219], &[219], &[219], &[219], &[219], &[], &[947], &[]] }, // pink_banner
    Recipe { shaped: true, width: 3, height: 2, result: 1093, count: 1, cells: &[&[219], &[219], &[219], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47]] }, // pink_bed
    Recipe { shaped: false, width: 2, height: 1, result: 1093, count: 1, cells: &[&[1073], &[1102]] }, // pink_bed
    Recipe { shaped: true, width: 2, height: 1, result: 512, count: 3, cells: &[&[219], &[219]] }, // pink_carpet
    Recipe { shaped: false, width: 2, height: 1, result: 512, count: 1, cells: &[&[1073], &[521]] }, // pink_carpet
    Recipe { shaped: true, width: 3, height: 2, result: 845, count: 1, cells: &[&[1018], &[1018], &[1018], &[195], &[219], &[195]] }, // pink_harness
    Recipe { shaped: false, width: 2, height: 1, result: 845, count: 1, cells: &[&[1073], &[854]] }, // pink_harness
    Recipe { shaped: true, width: 3, height: 3, result: 537, count: 8, cells: &[&[195], &[195], &[195], &[195], &[1073], &[195], &[195], &[195], &[195]] }, // pink_stained_glass
    Recipe { shaped: true, width: 3, height: 2, result: 553, count: 16, cells: &[&[537], &[537], &[537], &[537], &[537], &[537]] }, // pink_stained_glass_pane
    Recipe { shaped: true, width: 3, height: 3, result: 553, count: 8, cells: &[&[409], &[409], &[409], &[409], &[1073], &[409], &[409], &[409], &[409]] }, // pink_stained_glass_pane
    Recipe { shaped: true, width: 3, height: 3, result: 493, count: 8, cells: &[&[522], &[522], &[522], &[522], &[1073], &[522], &[522], &[522], &[522]] }, // pink_terracotta
    Recipe { shaped: true, width: 3, height: 3, result: 723, count: 1, cells: &[&[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[35], &[905], &[35], &[35], &[718], &[35]] }, // piston
    Recipe { shaped: true, width: 2, height: 2, result: 7, count: 4, cells: &[&[6], &[6], &[6], &[6]] }, // polished_andesite
    Recipe { shaped: true, width: 3, height: 1, result: 711, count: 6, cells: &[&[7], &[7], &[7]] }, // polished_andesite_slab
    Recipe { shaped: true, width: 3, height: 3, result: 694, count: 4, cells: &[&[7], &[], &[], &[7], &[7], &[], &[7], &[7], &[7]] }, // polished_andesite_stairs
    Recipe { shaped: true, width: 2, height: 2, result: 364, count: 4, cells: &[&[363], &[363], &[363], &[363]] }, // polished_basalt
    Recipe { shaped: true, width: 2, height: 2, result: 1390, count: 4, cells: &[&[1386], &[1386], &[1386], &[1386]] }, // polished_blackstone
    Recipe { shaped: true, width: 3, height: 1, result: 1395, count: 6, cells: &[&[1394], &[1394], &[1394]] }, // polished_blackstone_brick_slab
    Recipe { shaped: true, width: 3, height: 3, result: 1396, count: 4, cells: &[&[1394], &[], &[], &[1394], &[1394], &[], &[1394], &[1394], &[1394]] }, // polished_blackstone_brick_stairs
    Recipe { shaped: true, width: 3, height: 2, result: 474, count: 6, cells: &[&[1394], &[1394], &[1394], &[1394], &[1394], &[1394]] }, // polished_blackstone_brick_wall
    Recipe { shaped: true, width: 2, height: 2, result: 1394, count: 4, cells: &[&[1390], &[1390], &[1390], &[1390]] }, // polished_blackstone_bricks
    Recipe { shaped: true, width: 2, height: 1, result: 765, count: 1, cells: &[&[1390], &[1390]] }, // polished_blackstone_pressure_plate
    Recipe { shaped: true, width: 3, height: 1, result: 1391, count: 6, cells: &[&[1390], &[1390], &[1390]] }, // polished_blackstone_slab
    Recipe { shaped: true, width: 3, height: 3, result: 1392, count: 4, cells: &[&[1390], &[], &[], &[1390], &[1390], &[], &[1390], &[1390], &[1390]] }, // polished_blackstone_stairs
    Recipe { shaped: true, width: 3, height: 2, result: 473, count: 6, cells: &[&[1390], &[1390], &[1390], &[1390], &[1390], &[1390]] }, // polished_blackstone_wall
    Recipe { shaped: true, width: 2, height: 2, result: 10, count: 4, cells: &[&[9], &[9], &[9], &[9]] }, // polished_deepslate
    Recipe { shaped: true, width: 3, height: 1, result: 714, count: 6, cells: &[&[10], &[10], &[10]] }, // polished_deepslate_slab
    Recipe { shaped: true, width: 3, height: 3, result: 697, count: 4, cells: &[&[10], &[], &[], &[10], &[10], &[], &[10], &[10], &[10]] }, // polished_deepslate_stairs
    Recipe { shaped: true, width: 3, height: 2, result: 476, count: 6, cells: &[&[10], &[10], &[10], &[10], &[10], &[10]] }, // polished_deepslate_wall
    Recipe { shaped: true, width: 2, height: 2, result: 5, count: 4, cells: &[&[4], &[4], &[4], &[4]] }, // polished_diorite
    Recipe { shaped: true, width: 3, height: 1, result: 703, count: 6, cells: &[&[5], &[5], &[5]] }, // polished_diorite_slab
    Recipe { shaped: true, width: 3, height: 3, result: 685, count: 4, cells: &[&[5], &[], &[], &[5], &[5], &[], &[5], &[5], &[5]] }, // polished_diorite_stairs
    Recipe { shaped: true, width: 2, height: 2, result: 3, count: 4, cells: &[&[2], &[2], &[2], &[2]] }, // polished_granite
    Recipe { shaped: true, width: 3, height: 1, result: 700, count: 6, cells: &[&[3], &[3], &[3]] }, // polished_granite_slab
    Recipe { shaped: true, width: 3, height: 3, result: 682, count: 4, cells: &[&[3], &[], &[], &[3], &[3], &[], &[3], &[3], &[3]] }, // polished_granite_stairs
    Recipe { shaped: true, width: 2, height: 2, result: 17, count: 4, cells: &[&[12], &[12], &[12], &[12]] }, // polished_tuff
    Recipe { shaped: true, width: 3, height: 1, result: 18, count: 6, cells: &[&[17], &[17], &[17]] }, // polished_tuff_slab
    Recipe { shaped: true, width: 3, height: 3, result: 19, count: 4, cells: &[&[17], &[], &[], &[17], &[17], &[], &[17], &[17], &[17]] }, // polished_tuff_stairs
    Recipe { shaped: true, width: 3, height: 2, result: 20, count: 6, cells: &[&[17], &[17], &[17], &[17], &[17], &[17]] }, // polished_tuff_wall
    Recipe { shaped: true, width: 3, height: 3, result: 834, count: 6, cells: &[&[909], &[], &[909], &[909], &[947], &[909], &[909], &[718], &[909]] }, // powered_rail
    Recipe { shaped: true, width: 2, height: 2, result: 563, count: 1, cells: &[&[1248], &[1248], &[1248], &[1248]] }, // prismarine
    Recipe { shaped: true, width: 3, height: 1, result: 299, count: 6, cells: &[&[564], &[564], &[564]] }, // prismarine_brick_slab
    Recipe { shaped: true, width: 3, height: 3, result: 567, count: 4, cells: &[&[564], &[], &[], &[564], &[564], &[], &[564], &[564], &[564]] }, // prismarine_brick_stairs
    Recipe { shaped: true, width: 3, height: 1, result: 298, count: 6, cells: &[&[563], &[563], &[563]] }, // prismarine_slab
    Recipe { shaped: true, width: 3, height: 3, result: 566, count: 4, cells: &[&[563], &[], &[], &[563], &[563], &[], &[563], &[563], &[563]] }, // prismarine_stairs
    Recipe { shaped: true, width: 3, height: 2, result: 460, count: 6, cells: &[&[563], &[563], &[563], &[563], &[563], &[563]] }, // prismarine_wall
    Recipe { shaped: true, width: 3, height: 3, result: 1277, count: 1, cells: &[&[223], &[223], &[223], &[223], &[223], &[223], &[], &[947], &[]] }, // purple_banner
    Recipe { shaped: true, width: 3, height: 2, result: 1097, count: 1, cells: &[&[223], &[223], &[223], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47]] }, // purple_bed
    Recipe { shaped: false, width: 2, height: 1, result: 1097, count: 1, cells: &[&[1077], &[1102]] }, // purple_bed
    Recipe { shaped: true, width: 2, height: 1, result: 516, count: 3, cells: &[&[223], &[223]] }, // purple_carpet
    Recipe { shaped: false, width: 2, height: 1, result: 516, count: 1, cells: &[&[1077], &[521]] }, // purple_carpet
    Recipe { shaped: true, width: 3, height: 2, result: 849, count: 1, cells: &[&[1018], &[1018], &[1018], &[195], &[223], &[195]] }, // purple_harness
    Recipe { shaped: false, width: 2, height: 1, result: 849, count: 1, cells: &[&[1077], &[854]] }, // purple_harness
    Recipe { shaped: true, width: 3, height: 3, result: 541, count: 8, cells: &[&[195], &[195], &[195], &[195], &[1077], &[195], &[195], &[195], &[195]] }, // purple_stained_glass
    Recipe { shaped: true, width: 3, height: 2, result: 557, count: 16, cells: &[&[541], &[541], &[541], &[541], &[541], &[541]] }, // purple_stained_glass_pane
    Recipe { shaped: true, width: 3, height: 3, result: 557, count: 8, cells: &[&[409], &[409], &[409], &[409], &[1077], &[409], &[409], &[409], &[409]] }, // purple_stained_glass_pane
    Recipe { shaped: true, width: 3, height: 3, result: 497, count: 8, cells: &[&[522], &[522], &[522], &[522], &[1077], &[522], &[522], &[522], &[522]] }, // purple_terracotta
    Recipe { shaped: true, width: 2, height: 2, result: 327, count: 4, cells: &[&[1285], &[1285], &[1285], &[1285]] }, // purpur_block
    Recipe { shaped: true, width: 1, height: 2, result: 328, count: 1, cells: &[&[297], &[297]] }, // purpur_pillar
    Recipe { shaped: true, width: 3, height: 1, result: 297, count: 6, cells: &[&[327, 328], &[327, 328], &[327, 328]] }, // purpur_slab
    Recipe { shaped: true, width: 3, height: 3, result: 329, count: 4, cells: &[&[327, 328], &[], &[], &[327, 328], &[327, 328], &[], &[327, 328], &[327, 328], &[327, 328]] }, // purpur_stairs
    Recipe { shaped: true, width: 2, height: 2, result: 483, count: 1, cells: &[&[902], &[902], &[902], &[902]] }, // quartz_block
    Recipe { shaped: true, width: 2, height: 2, result: 484, count: 4, cells: &[&[483], &[483], &[483], &[483]] }, // quartz_bricks
    Recipe { shaped: true, width: 1, height: 2, result: 485, count: 2, cells: &[&[483], &[483]] }, // quartz_pillar
    Recipe { shaped: true, width: 3, height: 1, result: 294, count: 6, cells: &[&[482, 483, 485], &[482, 483, 485], &[482, 483, 485]] }, // quartz_slab
    Recipe { shaped: true, width: 3, height: 3, result: 486, count: 4, cells: &[&[482, 483, 485], &[], &[], &[482, 483, 485], &[482, 483, 485], &[], &[482, 483, 485], &[482, 483, 485], &[482, 483, 485]] }, // quartz_stairs
    Recipe { shaped: true, width: 3, height: 3, result: 836, count: 16, cells: &[&[905], &[], &[905], &[905], &[947], &[905], &[905], &[], &[905]] }, // rail
    Recipe { shaped: true, width: 3, height: 3, result: 1442, count: 2, cells: &[&[899], &[1442], &[899], &[899], &[522], &[899], &[899], &[899], &[899]] }, // raiser_armor_trim_smithing_template
    Recipe { shaped: true, width: 3, height: 3, result: 85, count: 1, cells: &[&[906], &[906], &[906], &[906], &[906], &[906], &[906], &[906], &[906]] }, // raw_copper_block
    Recipe { shaped: true, width: 3, height: 3, result: 86, count: 1, cells: &[&[908], &[908], &[908], &[908], &[908], &[908], &[908], &[908], &[908]] }, // raw_gold_block
    Recipe { shaped: true, width: 3, height: 3, result: 84, count: 1, cells: &[&[904], &[904], &[904], &[904], &[904], &[904], &[904], &[904], &[904]] }, // raw_iron_block
    Recipe { shaped: true, width: 3, height: 3, result: 1036, count: 1, cells: &[&[1425], &[1425], &[1425], &[1425], &[1035], &[1425], &[1425], &[1425], &[1425]] }, // recovery_compass
    Recipe { shaped: true, width: 3, height: 3, result: 1281, count: 1, cells: &[&[227], &[227], &[227], &[227], &[227], &[227], &[], &[947], &[]] }, // red_banner
    Recipe { shaped: true, width: 3, height: 2, result: 1101, count: 1, cells: &[&[227], &[227], &[227], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47]] }, // red_bed
    Recipe { shaped: false, width: 2, height: 1, result: 1101, count: 1, cells: &[&[1081], &[1102]] }, // red_bed
    Recipe { shaped: true, width: 2, height: 1, result: 520, count: 3, cells: &[&[227], &[227]] }, // red_carpet
    Recipe { shaped: false, width: 2, height: 1, result: 520, count: 1, cells: &[&[1081], &[521]] }, // red_carpet
    Recipe { shaped: true, width: 3, height: 2, result: 853, count: 1, cells: &[&[1018], &[1018], &[1018], &[195], &[227], &[195]] }, // red_harness
    Recipe { shaped: false, width: 2, height: 1, result: 853, count: 1, cells: &[&[1081], &[854]] }, // red_harness
    Recipe { shaped: true, width: 3, height: 1, result: 710, count: 6, cells: &[&[579], &[579], &[579]] }, // red_nether_brick_slab
    Recipe { shaped: true, width: 3, height: 3, result: 693, count: 4, cells: &[&[579], &[], &[], &[579], &[579], &[], &[579], &[579], &[579]] }, // red_nether_brick_stairs
    Recipe { shaped: true, width: 3, height: 2, result: 468, count: 6, cells: &[&[579], &[579], &[579], &[579], &[579], &[579]] }, // red_nether_brick_wall
    Recipe { shaped: true, width: 2, height: 2, result: 579, count: 1, cells: &[&[1246], &[1120], &[1120], &[1246]] }, // red_nether_bricks
    Recipe { shaped: true, width: 2, height: 2, result: 570, count: 1, cells: &[&[62], &[62], &[62], &[62]] }, // red_sandstone
    Recipe { shaped: true, width: 3, height: 1, result: 295, count: 6, cells: &[&[570, 571], &[570, 571], &[570, 571]] }, // red_sandstone_slab
    Recipe { shaped: true, width: 3, height: 3, result: 573, count: 4, cells: &[&[570, 571, 572], &[], &[], &[570, 571, 572], &[570, 571, 572], &[], &[570, 571, 572], &[570, 571, 572], &[570, 571, 572]] }, // red_sandstone_stairs
    Recipe { shaped: true, width: 3, height: 2, result: 461, count: 6, cells: &[&[570], &[570], &[570], &[570], &[570], &[570]] }, // red_sandstone_wall
    Recipe { shaped: true, width: 3, height: 3, result: 545, count: 8, cells: &[&[195], &[195], &[195], &[195], &[1081], &[195], &[195], &[195], &[195]] }, // red_stained_glass
    Recipe { shaped: true, width: 3, height: 2, result: 561, count: 16, cells: &[&[545], &[545], &[545], &[545], &[545], &[545]] }, // red_stained_glass_pane
    Recipe { shaped: true, width: 3, height: 3, result: 561, count: 8, cells: &[&[409], &[409], &[409], &[409], &[1081], &[409], &[409], &[409], &[409]] }, // red_stained_glass_pane
    Recipe { shaped: true, width: 3, height: 3, result: 501, count: 8, cells: &[&[522], &[522], &[522], &[522], &[1081], &[522], &[522], &[522], &[522]] }, // red_terracotta
    Recipe { shaped: true, width: 3, height: 3, result: 720, count: 1, cells: &[&[718], &[718], &[718], &[718], &[718], &[718], &[718], &[718], &[718]] }, // redstone_block
    Recipe { shaped: true, width: 3, height: 3, result: 748, count: 1, cells: &[&[], &[718], &[], &[718], &[368], &[718], &[], &[718], &[]] }, // redstone_lamp
    Recipe { shaped: true, width: 1, height: 2, result: 719, count: 1, cells: &[&[718], &[947]] }, // redstone_torch
    Recipe { shaped: true, width: 3, height: 2, result: 721, count: 1, cells: &[&[719], &[718], &[719], &[1], &[1], &[1]] }, // repeater
    Recipe { shaped: true, width: 3, height: 3, result: 414, count: 1, cells: &[&[413], &[413], &[413], &[413], &[413], &[413], &[413], &[413], &[413]] }, // resin_block
    Recipe { shaped: true, width: 3, height: 1, result: 417, count: 6, cells: &[&[415], &[415], &[415]] }, // resin_brick_slab
    Recipe { shaped: true, width: 3, height: 3, result: 416, count: 4, cells: &[&[415], &[], &[], &[415], &[415], &[], &[415], &[415], &[415]] }, // resin_brick_stairs
    Recipe { shaped: true, width: 3, height: 2, result: 418, count: 6, cells: &[&[415], &[415], &[415], &[415], &[415], &[415]] }, // resin_brick_wall
    Recipe { shaped: true, width: 2, height: 2, result: 415, count: 1, cells: &[&[1247], &[1247], &[1247], &[1247]] }, // resin_bricks
    Recipe { shaped: true, width: 3, height: 3, result: 1398, count: 1, cells: &[&[1385], &[1385], &[1385], &[368], &[368], &[368], &[1385], &[1385], &[1385]] }, // respawn_anchor
    Recipe { shaped: true, width: 3, height: 3, result: 1437, count: 2, cells: &[&[899], &[1437], &[899], &[899], &[360], &[899], &[899], &[899], &[899]] }, // rib_armor_trim_smithing_template
    Recipe { shaped: true, width: 3, height: 2, result: 838, count: 1, cells: &[&[], &[1018], &[], &[1018], &[905], &[1018]] }, // saddle
    Recipe { shaped: true, width: 2, height: 2, result: 198, count: 1, cells: &[&[59], &[59], &[59], &[59]] }, // sandstone
    Recipe { shaped: true, width: 3, height: 1, result: 286, count: 6, cells: &[&[198, 199], &[198, 199], &[198, 199]] }, // sandstone_slab
    Recipe { shaped: true, width: 3, height: 3, result: 439, count: 4, cells: &[&[198, 199, 200], &[], &[], &[198, 199, 200], &[198, 199, 200], &[], &[198, 199, 200], &[198, 199, 200], &[198, 199, 200]] }, // sandstone_stairs
    Recipe { shaped: true, width: 3, height: 2, result: 469, count: 6, cells: &[&[198], &[198], &[198], &[198], &[198], &[198]] }, // sandstone_wall
    Recipe { shaped: true, width: 3, height: 3, result: 717, count: 6, cells: &[&[270], &[949], &[270], &[270], &[], &[270], &[270], &[], &[270]] }, // scaffolding
    Recipe { shaped: true, width: 3, height: 3, result: 569, count: 1, cells: &[&[1248], &[1249], &[1248], &[1249], &[1249], &[1249], &[1248], &[1249], &[1248]] }, // sea_lantern
    Recipe { shaped: true, width: 3, height: 3, result: 1428, count: 2, cells: &[&[899], &[1428], &[899], &[899], &[35], &[899], &[899], &[899], &[899]] }, // sentry_armor_trim_smithing_template
    Recipe { shaped: true, width: 3, height: 3, result: 1440, count: 2, cells: &[&[899], &[1440], &[899], &[899], &[522], &[899], &[899], &[899], &[899]] }, // shaper_armor_trim_smithing_template
    Recipe { shaped: true, width: 2, height: 2, result: 1106, count: 1, cells: &[&[], &[905], &[905], &[]] }, // shears
    Recipe { shaped: true, width: 3, height: 3, result: 1296, count: 1, cells: &[&[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[905], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[]] }, // shield
    Recipe { shaped: true, width: 1, height: 3, result: 582, count: 1, cells: &[&[1305], &[332], &[1305]] }, // shulker_box
    Recipe { shaped: true, width: 3, height: 3, result: 1441, count: 2, cells: &[&[899], &[1441], &[899], &[899], &[9], &[899], &[899], &[899], &[899]] }, // silence_armor_trim_smithing_template
    Recipe { shaped: true, width: 3, height: 3, result: 725, count: 1, cells: &[&[1031], &[1031], &[1031], &[1031], &[1031], &[1031], &[1031], &[1031], &[1031]] }, // slime_block
    Recipe { shaped: true, width: 2, height: 3, result: 1361, count: 1, cells: &[&[905], &[905], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47]] }, // smithing_table
    Recipe { shaped: true, width: 3, height: 3, result: 1356, count: 1, cells: &[&[], &[134, 135, 136, 137, 138, 139, 140, 141, 142, 145, 146, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 171, 172, 173, 174, 175, 176, 177, 178, 179, 180, 181], &[], &[134, 135, 136, 137, 138, 139, 140, 141, 142, 145, 146, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 171, 172, 173, 174, 175, 176, 177, 178, 179, 180, 181], &[335], &[134, 135, 136, 137, 138, 139, 140, 141, 142, 145, 146, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 171, 172, 173, 174, 175, 176, 177, 178, 179, 180, 181], &[], &[134, 135, 136, 137, 138, 139, 140, 141, 142, 145, 146, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 171, 172, 173, 174, 175, 176, 177, 178, 179, 180, 181], &[]] }, // smoker
    Recipe { shaped: true, width: 3, height: 1, result: 707, count: 6, cells: &[&[301], &[301], &[301]] }, // smooth_quartz_slab
    Recipe { shaped: true, width: 3, height: 3, result: 690, count: 4, cells: &[&[301], &[], &[], &[301], &[301], &[], &[301], &[301], &[301]] }, // smooth_quartz_stairs
    Recipe { shaped: true, width: 3, height: 1, result: 701, count: 6, cells: &[&[302], &[302], &[302]] }, // smooth_red_sandstone_slab
    Recipe { shaped: true, width: 3, height: 3, result: 683, count: 4, cells: &[&[302], &[], &[], &[302], &[302], &[], &[302], &[302], &[302]] }, // smooth_red_sandstone_stairs
    Recipe { shaped: true, width: 3, height: 1, result: 706, count: 6, cells: &[&[303], &[303], &[303]] }, // smooth_sandstone_slab
    Recipe { shaped: true, width: 3, height: 3, result: 689, count: 4, cells: &[&[303], &[], &[], &[303], &[303], &[], &[303], &[303], &[303]] }, // smooth_sandstone_stairs
    Recipe { shaped: true, width: 3, height: 1, result: 285, count: 6, cells: &[&[304], &[304], &[304]] }, // smooth_stone_slab
    Recipe { shaped: true, width: 3, height: 3, result: 1436, count: 2, cells: &[&[899], &[1436], &[899], &[899], &[1386], &[899], &[899], &[899], &[899]] }, // snout_armor_trim_smithing_template
    Recipe { shaped: true, width: 3, height: 1, result: 338, count: 6, cells: &[&[340], &[340], &[340]] }, // snow
    Recipe { shaped: true, width: 2, height: 2, result: 340, count: 1, cells: &[&[1017], &[1017], &[1017], &[1017]] }, // snow_block
    Recipe { shaped: true, width: 3, height: 3, result: 1377, count: 1, cells: &[&[], &[947], &[], &[947], &[361, 362], &[947], &[134, 135, 136, 137, 138, 139, 140, 141, 142, 145, 146, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 171, 172, 173, 174, 175, 176, 177, 178, 179, 180, 181], &[134, 135, 136, 137, 138, 139, 140, 141, 142, 145, 146, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 171, 172, 173, 174, 175, 176, 177, 178, 179, 180, 181], &[134, 135, 136, 137, 138, 139, 140, 141, 142, 145, 146, 148, 149, 150, 151, 152, 153, 154, 155, 156, 157, 158, 159, 160, 161, 162, 163, 164, 165, 166, 167, 168, 169, 171, 172, 173, 174, 175, 176, 177, 178, 179, 180, 181]] }, // soul_campfire
    Recipe { shaped: true, width: 3, height: 3, result: 1365, count: 1, cells: &[&[1306], &[1306], &[1306], &[1306], &[366], &[1306], &[1306], &[1306], &[1306]] }, // soul_lantern
    Recipe { shaped: true, width: 1, height: 3, result: 366, count: 4, cells: &[&[897, 898], &[947], &[361, 362]] }, // soul_torch
    Recipe { shaped: true, width: 3, height: 3, result: 1293, count: 2, cells: &[&[], &[1057], &[], &[1057], &[896], &[1057], &[], &[1057], &[]] }, // spectral_arrow
    Recipe { shaped: true, width: 3, height: 3, result: 1438, count: 2, cells: &[&[899], &[1438], &[899], &[899], &[327], &[899], &[899], &[899], &[899]] }, // spire_armor_trim_smithing_template
    Recipe { shaped: true, width: 3, height: 2, result: 866, count: 1, cells: &[&[37], &[], &[37], &[37], &[37], &[37]] }, // spruce_boat
    Recipe { shaped: true, width: 2, height: 3, result: 782, count: 3, cells: &[&[37], &[37], &[37], &[37], &[37], &[37]] }, // spruce_door
    Recipe { shaped: true, width: 3, height: 2, result: 346, count: 3, cells: &[&[37], &[947], &[37], &[37], &[947], &[37]] }, // spruce_fence
    Recipe { shaped: true, width: 3, height: 2, result: 823, count: 1, cells: &[&[947], &[37], &[947], &[947], &[37], &[947]] }, // spruce_fence_gate
    Recipe { shaped: true, width: 3, height: 3, result: 1002, count: 6, cells: &[&[400], &[], &[400], &[149], &[149], &[149], &[149], &[149], &[149]] }, // spruce_hanging_sign
    Recipe { shaped: true, width: 2, height: 1, result: 769, count: 1, cells: &[&[37], &[37]] }, // spruce_pressure_plate
    Recipe { shaped: true, width: 3, height: 3, result: 316, count: 6, cells: &[&[149], &[149], &[149], &[], &[], &[], &[149], &[149], &[149]] }, // spruce_shelf
    Recipe { shaped: true, width: 3, height: 3, result: 990, count: 3, cells: &[&[37], &[37], &[37], &[37], &[37], &[37], &[], &[947], &[]] }, // spruce_sign
    Recipe { shaped: true, width: 3, height: 1, result: 272, count: 6, cells: &[&[37], &[37], &[37]] }, // spruce_slab
    Recipe { shaped: true, width: 3, height: 3, result: 443, count: 4, cells: &[&[37], &[], &[], &[37], &[37], &[], &[37], &[37], &[37]] }, // spruce_stairs
    Recipe { shaped: true, width: 3, height: 2, result: 803, count: 2, cells: &[&[37], &[37], &[37], &[37], &[37], &[37]] }, // spruce_trapdoor
    Recipe { shaped: true, width: 2, height: 2, result: 172, count: 3, cells: &[&[135], &[135], &[135], &[135]] }, // spruce_wood
    Recipe { shaped: true, width: 1, height: 3, result: 1056, count: 1, cells: &[&[903], &[907], &[907]] }, // spyglass
    Recipe { shaped: true, width: 1, height: 2, result: 947, count: 4, cells: &[&[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47]] }, // stick
    Recipe { shaped: true, width: 1, height: 2, result: 947, count: 1, cells: &[&[270], &[270]] }, // stick
    Recipe { shaped: true, width: 1, height: 2, result: 724, count: 1, cells: &[&[1031], &[723]] }, // sticky_piston
    Recipe { shaped: true, width: 2, height: 3, result: 925, count: 1, cells: &[&[9, 35, 1386], &[9, 35, 1386], &[9, 35, 1386], &[947], &[], &[947]] }, // stone_axe
    Recipe { shaped: true, width: 3, height: 1, result: 291, count: 6, cells: &[&[376], &[376], &[376]] }, // stone_brick_slab
    Recipe { shaped: true, width: 3, height: 3, result: 421, count: 4, cells: &[&[376], &[], &[], &[376], &[376], &[], &[376], &[376], &[376]] }, // stone_brick_stairs
    Recipe { shaped: true, width: 3, height: 2, result: 464, count: 6, cells: &[&[376], &[376], &[376], &[376], &[376], &[376]] }, // stone_brick_wall
    Recipe { shaped: true, width: 2, height: 2, result: 376, count: 4, cells: &[&[1], &[1], &[1], &[1]] }, // stone_bricks
    Recipe { shaped: true, width: 2, height: 3, result: 926, count: 1, cells: &[&[9, 35, 1386], &[9, 35, 1386], &[], &[947], &[], &[947]] }, // stone_hoe
    Recipe { shaped: true, width: 3, height: 3, result: 924, count: 1, cells: &[&[9, 35, 1386], &[9, 35, 1386], &[9, 35, 1386], &[], &[947], &[], &[], &[947], &[]] }, // stone_pickaxe
    Recipe { shaped: true, width: 2, height: 1, result: 764, count: 1, cells: &[&[1], &[1]] }, // stone_pressure_plate
    Recipe { shaped: true, width: 1, height: 3, result: 923, count: 1, cells: &[&[9, 35, 1386], &[947], &[947]] }, // stone_shovel
    Recipe { shaped: true, width: 3, height: 1, result: 284, count: 6, cells: &[&[1], &[1], &[1]] }, // stone_slab
    Recipe { shaped: true, width: 3, height: 3, result: 1298, count: 1, cells: &[&[], &[], &[9, 35, 1386], &[], &[947], &[], &[947], &[], &[]] }, // stone_spear
    Recipe { shaped: true, width: 3, height: 3, result: 688, count: 4, cells: &[&[1], &[], &[], &[1], &[1], &[], &[1], &[1], &[1]] }, // stone_stairs
    Recipe { shaped: true, width: 1, height: 3, result: 922, count: 1, cells: &[&[9, 35, 1386], &[9, 35, 1386], &[947]] }, // stone_sword
    Recipe { shaped: true, width: 3, height: 2, result: 1362, count: 1, cells: &[&[], &[905], &[], &[1], &[1], &[1]] }, // stonecutter
    Recipe { shaped: true, width: 2, height: 2, result: 163, count: 3, cells: &[&[152], &[152], &[152], &[152]] }, // stripped_acacia_wood
    Recipe { shaped: true, width: 2, height: 2, result: 161, count: 3, cells: &[&[150], &[150], &[150], &[150]] }, // stripped_birch_wood
    Recipe { shaped: true, width: 2, height: 2, result: 164, count: 3, cells: &[&[153], &[153], &[153], &[153]] }, // stripped_cherry_wood
    Recipe { shaped: true, width: 2, height: 2, result: 168, count: 3, cells: &[&[157], &[157], &[157], &[157]] }, // stripped_crimson_hyphae
    Recipe { shaped: true, width: 2, height: 2, result: 165, count: 3, cells: &[&[154], &[154], &[154], &[154]] }, // stripped_dark_oak_wood
    Recipe { shaped: true, width: 2, height: 2, result: 162, count: 3, cells: &[&[151], &[151], &[151], &[151]] }, // stripped_jungle_wood
    Recipe { shaped: true, width: 2, height: 2, result: 167, count: 3, cells: &[&[156], &[156], &[156], &[156]] }, // stripped_mangrove_wood
    Recipe { shaped: true, width: 2, height: 2, result: 159, count: 3, cells: &[&[148], &[148], &[148], &[148]] }, // stripped_oak_wood
    Recipe { shaped: true, width: 2, height: 2, result: 166, count: 3, cells: &[&[155], &[155], &[155], &[155]] }, // stripped_pale_oak_wood
    Recipe { shaped: true, width: 2, height: 2, result: 160, count: 3, cells: &[&[149], &[149], &[149], &[149]] }, // stripped_spruce_wood
    Recipe { shaped: true, width: 2, height: 2, result: 169, count: 3, cells: &[&[158], &[158], &[158], &[158]] }, // stripped_warped_hyphae
    Recipe { shaped: true, width: 3, height: 3, result: 732, count: 1, cells: &[&[], &[718], &[], &[718], &[505], &[718], &[], &[718], &[]] }, // target
    Recipe { shaped: true, width: 3, height: 3, result: 1435, count: 2, cells: &[&[899], &[1435], &[899], &[899], &[563], &[899], &[899], &[899], &[899]] }, // tide_armor_trim_smithing_template
    Recipe { shaped: true, width: 3, height: 3, result: 196, count: 2, cells: &[&[], &[903], &[], &[903], &[195], &[903], &[], &[903], &[]] }, // tinted_glass
    Recipe { shaped: true, width: 3, height: 3, result: 747, count: 1, cells: &[&[951], &[59, 62], &[951], &[59, 62], &[951], &[59, 62], &[951], &[59, 62], &[951]] }, // tnt
    Recipe { shaped: true, width: 1, height: 2, result: 323, count: 4, cells: &[&[897, 898], &[947]] }, // torch
    Recipe { shaped: true, width: 1, height: 3, result: 745, count: 2, cells: &[&[905], &[947], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47]] }, // tripwire_hook
    Recipe { shaped: true, width: 3, height: 1, result: 22, count: 6, cells: &[&[21], &[21], &[21]] }, // tuff_brick_slab
    Recipe { shaped: true, width: 3, height: 3, result: 23, count: 4, cells: &[&[21], &[], &[], &[21], &[21], &[], &[21], &[21], &[21]] }, // tuff_brick_stairs
    Recipe { shaped: true, width: 3, height: 2, result: 24, count: 6, cells: &[&[21], &[21], &[21], &[21], &[21], &[21]] }, // tuff_brick_wall
    Recipe { shaped: true, width: 2, height: 2, result: 21, count: 4, cells: &[&[17], &[17], &[17], &[17]] }, // tuff_bricks
    Recipe { shaped: true, width: 3, height: 1, result: 13, count: 6, cells: &[&[12], &[12], &[12]] }, // tuff_slab
    Recipe { shaped: true, width: 3, height: 3, result: 14, count: 4, cells: &[&[12], &[], &[], &[12], &[12], &[], &[12], &[12], &[12]] }, // tuff_stairs
    Recipe { shaped: true, width: 3, height: 2, result: 15, count: 6, cells: &[&[12], &[12], &[12], &[12], &[12], &[12]] }, // tuff_wall
    Recipe { shaped: true, width: 3, height: 2, result: 888, count: 1, cells: &[&[889], &[889], &[889], &[889], &[], &[889]] }, // turtle_helmet
    Recipe { shaped: true, width: 3, height: 3, result: 1434, count: 2, cells: &[&[899], &[1434], &[899], &[899], &[35], &[899], &[899], &[899], &[899]] }, // vex_armor_trim_smithing_template
    Recipe { shaped: true, width: 3, height: 3, result: 1432, count: 2, cells: &[&[899], &[1432], &[899], &[899], &[9], &[899], &[899], &[899], &[899]] }, // ward_armor_trim_smithing_template
    Recipe { shaped: true, width: 2, height: 3, result: 792, count: 3, cells: &[&[47], &[47], &[47], &[47], &[47], &[47]] }, // warped_door
    Recipe { shaped: true, width: 3, height: 2, result: 356, count: 3, cells: &[&[47], &[947], &[47], &[47], &[947], &[47]] }, // warped_fence
    Recipe { shaped: true, width: 3, height: 2, result: 833, count: 1, cells: &[&[947], &[47], &[947], &[947], &[47], &[947]] }, // warped_fence_gate
    Recipe { shaped: true, width: 2, height: 2, result: 861, count: 1, cells: &[&[1054], &[], &[], &[251]] }, // warped_fungus_on_a_stick
    Recipe { shaped: true, width: 3, height: 3, result: 1012, count: 6, cells: &[&[400], &[], &[400], &[158], &[158], &[158], &[158], &[158], &[158]] }, // warped_hanging_sign
    Recipe { shaped: true, width: 2, height: 2, result: 181, count: 3, cells: &[&[146], &[146], &[146], &[146]] }, // warped_hyphae
    Recipe { shaped: true, width: 2, height: 1, result: 779, count: 1, cells: &[&[47], &[47]] }, // warped_pressure_plate
    Recipe { shaped: true, width: 3, height: 3, result: 317, count: 6, cells: &[&[158], &[158], &[158], &[], &[], &[], &[158], &[158], &[158]] }, // warped_shelf
    Recipe { shaped: true, width: 3, height: 3, result: 1000, count: 3, cells: &[&[47], &[47], &[47], &[47], &[47], &[47], &[], &[947], &[]] }, // warped_sign
    Recipe { shaped: true, width: 3, height: 1, result: 283, count: 6, cells: &[&[47], &[47], &[47]] }, // warped_slab
    Recipe { shaped: true, width: 3, height: 3, result: 454, count: 4, cells: &[&[47], &[], &[], &[47], &[47], &[], &[47], &[47], &[47]] }, // warped_stairs
    Recipe { shaped: true, width: 3, height: 2, result: 813, count: 2, cells: &[&[47], &[47], &[47], &[47], &[47], &[47]] }, // warped_trapdoor
    Recipe { shaped: true, width: 1, height: 2, result: 118, count: 1, cells: &[&[130], &[130]] }, // waxed_chiseled_copper
    Recipe { shaped: false, width: 2, height: 1, result: 118, count: 1, cells: &[&[98], &[1379]] }, // waxed_chiseled_copper
    Recipe { shaped: true, width: 3, height: 3, result: 1481, count: 4, cells: &[&[], &[114], &[], &[114], &[1117], &[114], &[], &[718], &[]] }, // waxed_copper_bulb
    Recipe { shaped: false, width: 2, height: 1, result: 1481, count: 1, cells: &[&[1477], &[1379]] }, // waxed_copper_bulb
    Recipe { shaped: true, width: 3, height: 3, result: 1473, count: 4, cells: &[&[], &[114], &[], &[114], &[], &[114], &[], &[114], &[]] }, // waxed_copper_grate
    Recipe { shaped: false, width: 2, height: 1, result: 1473, count: 1, cells: &[&[1469], &[1379]] }, // waxed_copper_grate
    Recipe { shaped: true, width: 2, height: 2, result: 122, count: 4, cells: &[&[114], &[114], &[114], &[114]] }, // waxed_cut_copper
    Recipe { shaped: false, width: 2, height: 1, result: 122, count: 1, cells: &[&[102], &[1379]] }, // waxed_cut_copper
    Recipe { shaped: true, width: 3, height: 1, result: 130, count: 6, cells: &[&[122], &[122], &[122]] }, // waxed_cut_copper_slab
    Recipe { shaped: false, width: 2, height: 1, result: 130, count: 1, cells: &[&[110], &[1379]] }, // waxed_cut_copper_slab
    Recipe { shaped: true, width: 3, height: 3, result: 126, count: 4, cells: &[&[122], &[], &[], &[122], &[122], &[], &[122], &[122], &[122]] }, // waxed_cut_copper_stairs
    Recipe { shaped: false, width: 2, height: 1, result: 126, count: 1, cells: &[&[106], &[1379]] }, // waxed_cut_copper_stairs
    Recipe { shaped: true, width: 1, height: 2, result: 119, count: 1, cells: &[&[131], &[131]] }, // waxed_exposed_chiseled_copper
    Recipe { shaped: false, width: 2, height: 1, result: 119, count: 1, cells: &[&[99], &[1379]] }, // waxed_exposed_chiseled_copper
    Recipe { shaped: true, width: 3, height: 3, result: 1482, count: 4, cells: &[&[], &[115], &[], &[115], &[1117], &[115], &[], &[718], &[]] }, // waxed_exposed_copper_bulb
    Recipe { shaped: false, width: 2, height: 1, result: 1482, count: 1, cells: &[&[1478], &[1379]] }, // waxed_exposed_copper_bulb
    Recipe { shaped: true, width: 3, height: 3, result: 1474, count: 4, cells: &[&[], &[115], &[], &[115], &[], &[115], &[], &[115], &[]] }, // waxed_exposed_copper_grate
    Recipe { shaped: false, width: 2, height: 1, result: 1474, count: 1, cells: &[&[1470], &[1379]] }, // waxed_exposed_copper_grate
    Recipe { shaped: true, width: 2, height: 2, result: 123, count: 4, cells: &[&[115], &[115], &[115], &[115]] }, // waxed_exposed_cut_copper
    Recipe { shaped: false, width: 2, height: 1, result: 123, count: 1, cells: &[&[103], &[1379]] }, // waxed_exposed_cut_copper
    Recipe { shaped: true, width: 3, height: 1, result: 131, count: 6, cells: &[&[123], &[123], &[123]] }, // waxed_exposed_cut_copper_slab
    Recipe { shaped: false, width: 2, height: 1, result: 131, count: 1, cells: &[&[111], &[1379]] }, // waxed_exposed_cut_copper_slab
    Recipe { shaped: true, width: 3, height: 3, result: 127, count: 4, cells: &[&[123], &[], &[], &[123], &[123], &[], &[123], &[123], &[123]] }, // waxed_exposed_cut_copper_stairs
    Recipe { shaped: false, width: 2, height: 1, result: 127, count: 1, cells: &[&[107], &[1379]] }, // waxed_exposed_cut_copper_stairs
    Recipe { shaped: true, width: 1, height: 2, result: 121, count: 1, cells: &[&[133], &[133]] }, // waxed_oxidized_chiseled_copper
    Recipe { shaped: false, width: 2, height: 1, result: 121, count: 1, cells: &[&[101], &[1379]] }, // waxed_oxidized_chiseled_copper
    Recipe { shaped: true, width: 3, height: 3, result: 1484, count: 4, cells: &[&[], &[117], &[], &[117], &[1117], &[117], &[], &[718], &[]] }, // waxed_oxidized_copper_bulb
    Recipe { shaped: false, width: 2, height: 1, result: 1484, count: 1, cells: &[&[1480], &[1379]] }, // waxed_oxidized_copper_bulb
    Recipe { shaped: true, width: 3, height: 3, result: 1476, count: 4, cells: &[&[], &[117], &[], &[117], &[], &[117], &[], &[117], &[]] }, // waxed_oxidized_copper_grate
    Recipe { shaped: false, width: 2, height: 1, result: 1476, count: 1, cells: &[&[1472], &[1379]] }, // waxed_oxidized_copper_grate
    Recipe { shaped: true, width: 2, height: 2, result: 125, count: 4, cells: &[&[117], &[117], &[117], &[117]] }, // waxed_oxidized_cut_copper
    Recipe { shaped: false, width: 2, height: 1, result: 125, count: 1, cells: &[&[105], &[1379]] }, // waxed_oxidized_cut_copper
    Recipe { shaped: true, width: 3, height: 1, result: 133, count: 6, cells: &[&[125], &[125], &[125]] }, // waxed_oxidized_cut_copper_slab
    Recipe { shaped: false, width: 2, height: 1, result: 133, count: 1, cells: &[&[113], &[1379]] }, // waxed_oxidized_cut_copper_slab
    Recipe { shaped: true, width: 3, height: 3, result: 129, count: 4, cells: &[&[125], &[], &[], &[125], &[125], &[], &[125], &[125], &[125]] }, // waxed_oxidized_cut_copper_stairs
    Recipe { shaped: false, width: 2, height: 1, result: 129, count: 1, cells: &[&[109], &[1379]] }, // waxed_oxidized_cut_copper_stairs
    Recipe { shaped: true, width: 1, height: 2, result: 120, count: 1, cells: &[&[132], &[132]] }, // waxed_weathered_chiseled_copper
    Recipe { shaped: false, width: 2, height: 1, result: 120, count: 1, cells: &[&[100], &[1379]] }, // waxed_weathered_chiseled_copper
    Recipe { shaped: true, width: 3, height: 3, result: 1483, count: 4, cells: &[&[], &[116], &[], &[116], &[1117], &[116], &[], &[718], &[]] }, // waxed_weathered_copper_bulb
    Recipe { shaped: false, width: 2, height: 1, result: 1483, count: 1, cells: &[&[1479], &[1379]] }, // waxed_weathered_copper_bulb
    Recipe { shaped: true, width: 3, height: 3, result: 1475, count: 4, cells: &[&[], &[116], &[], &[116], &[], &[116], &[], &[116], &[]] }, // waxed_weathered_copper_grate
    Recipe { shaped: false, width: 2, height: 1, result: 1475, count: 1, cells: &[&[1471], &[1379]] }, // waxed_weathered_copper_grate
    Recipe { shaped: true, width: 2, height: 2, result: 124, count: 4, cells: &[&[116], &[116], &[116], &[116]] }, // waxed_weathered_cut_copper
    Recipe { shaped: false, width: 2, height: 1, result: 124, count: 1, cells: &[&[104], &[1379]] }, // waxed_weathered_cut_copper
    Recipe { shaped: true, width: 3, height: 1, result: 132, count: 6, cells: &[&[124], &[124], &[124]] }, // waxed_weathered_cut_copper_slab
    Recipe { shaped: false, width: 2, height: 1, result: 132, count: 1, cells: &[&[112], &[1379]] }, // waxed_weathered_cut_copper_slab
    Recipe { shaped: true, width: 3, height: 3, result: 128, count: 4, cells: &[&[124], &[], &[], &[124], &[124], &[], &[124], &[124], &[124]] }, // waxed_weathered_cut_copper_stairs
    Recipe { shaped: false, width: 2, height: 1, result: 128, count: 1, cells: &[&[108], &[1379]] }, // waxed_weathered_cut_copper_stairs
    Recipe { shaped: true, width: 3, height: 3, result: 1439, count: 2, cells: &[&[899], &[1439], &[899], &[899], &[522], &[899], &[899], &[899], &[899]] }, // wayfinder_armor_trim_smithing_template
    Recipe { shaped: true, width: 1, height: 2, result: 100, count: 1, cells: &[&[112], &[112]] }, // weathered_chiseled_copper
    Recipe { shaped: true, width: 3, height: 3, result: 1479, count: 4, cells: &[&[], &[96], &[], &[96], &[1117], &[96], &[], &[718], &[]] }, // weathered_copper_bulb
    Recipe { shaped: true, width: 3, height: 3, result: 1471, count: 4, cells: &[&[], &[96], &[], &[96], &[], &[96], &[], &[96], &[]] }, // weathered_copper_grate
    Recipe { shaped: true, width: 2, height: 2, result: 104, count: 4, cells: &[&[96], &[96], &[96], &[96]] }, // weathered_cut_copper
    Recipe { shaped: true, width: 3, height: 1, result: 112, count: 6, cells: &[&[104], &[104], &[104]] }, // weathered_cut_copper_slab
    Recipe { shaped: true, width: 3, height: 3, result: 108, count: 4, cells: &[&[104], &[], &[], &[104], &[104], &[], &[104], &[104], &[104]] }, // weathered_cut_copper_stairs
    Recipe { shaped: true, width: 3, height: 3, result: 1267, count: 1, cells: &[&[213], &[213], &[213], &[213], &[213], &[213], &[], &[947], &[]] }, // white_banner
    Recipe { shaped: true, width: 3, height: 2, result: 1087, count: 1, cells: &[&[213], &[213], &[213], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47]] }, // white_bed
    Recipe { shaped: false, width: 2, height: 1, result: 1087, count: 1, cells: &[&[1067], &[1102]] }, // white_bed
    Recipe { shaped: true, width: 2, height: 1, result: 506, count: 3, cells: &[&[213], &[213]] }, // white_carpet
    Recipe { shaped: false, width: 2, height: 1, result: 506, count: 1, cells: &[&[1067], &[521]] }, // white_carpet
    Recipe { shaped: true, width: 3, height: 2, result: 839, count: 1, cells: &[&[1018], &[1018], &[1018], &[195], &[213], &[195]] }, // white_harness
    Recipe { shaped: false, width: 2, height: 1, result: 839, count: 1, cells: &[&[1067], &[854]] }, // white_harness
    Recipe { shaped: true, width: 3, height: 3, result: 531, count: 8, cells: &[&[195], &[195], &[195], &[195], &[1067], &[195], &[195], &[195], &[195]] }, // white_stained_glass
    Recipe { shaped: true, width: 3, height: 2, result: 547, count: 16, cells: &[&[531], &[531], &[531], &[531], &[531], &[531]] }, // white_stained_glass_pane
    Recipe { shaped: true, width: 3, height: 3, result: 547, count: 8, cells: &[&[409], &[409], &[409], &[409], &[1067], &[409], &[409], &[409], &[409]] }, // white_stained_glass_pane
    Recipe { shaped: true, width: 3, height: 3, result: 487, count: 8, cells: &[&[522], &[522], &[522], &[522], &[1067], &[522], &[522], &[522], &[522]] }, // white_terracotta
    Recipe { shaped: true, width: 2, height: 2, result: 213, count: 1, cells: &[&[949], &[949], &[949], &[949]] }, // white_wool
    Recipe { shaped: false, width: 2, height: 1, result: 213, count: 1, cells: &[&[1067], &[228]] }, // white_wool
    Recipe { shaped: true, width: 3, height: 3, result: 1431, count: 2, cells: &[&[899], &[1431], &[899], &[899], &[321], &[899], &[899], &[899], &[899]] }, // wild_armor_trim_smithing_template
    Recipe { shaped: true, width: 3, height: 3, result: 891, count: 1, cells: &[&[890], &[], &[], &[890], &[890], &[890], &[890], &[], &[890]] }, // wolf_armor
    Recipe { shaped: true, width: 2, height: 3, result: 915, count: 1, cells: &[&[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[947], &[], &[947]] }, // wooden_axe
    Recipe { shaped: true, width: 2, height: 3, result: 916, count: 1, cells: &[&[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[], &[947], &[], &[947]] }, // wooden_hoe
    Recipe { shaped: true, width: 3, height: 3, result: 914, count: 1, cells: &[&[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[], &[947], &[], &[], &[947], &[]] }, // wooden_pickaxe
    Recipe { shaped: true, width: 1, height: 3, result: 913, count: 1, cells: &[&[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[947], &[947]] }, // wooden_shovel
    Recipe { shaped: true, width: 3, height: 3, result: 1297, count: 1, cells: &[&[], &[], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[], &[947], &[], &[947], &[], &[]] }, // wooden_spear
    Recipe { shaped: true, width: 1, height: 3, result: 912, count: 1, cells: &[&[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[947]] }, // wooden_sword
    Recipe { shaped: true, width: 3, height: 3, result: 1271, count: 1, cells: &[&[217], &[217], &[217], &[217], &[217], &[217], &[], &[947], &[]] }, // yellow_banner
    Recipe { shaped: true, width: 3, height: 2, result: 1091, count: 1, cells: &[&[217], &[217], &[217], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47], &[36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47]] }, // yellow_bed
    Recipe { shaped: false, width: 2, height: 1, result: 1091, count: 1, cells: &[&[1071], &[1102]] }, // yellow_bed
    Recipe { shaped: true, width: 2, height: 1, result: 510, count: 3, cells: &[&[217], &[217]] }, // yellow_carpet
    Recipe { shaped: false, width: 2, height: 1, result: 510, count: 1, cells: &[&[1071], &[521]] }, // yellow_carpet
    Recipe { shaped: true, width: 3, height: 2, result: 843, count: 1, cells: &[&[1018], &[1018], &[1018], &[195], &[217], &[195]] }, // yellow_harness
    Recipe { shaped: false, width: 2, height: 1, result: 843, count: 1, cells: &[&[1071], &[854]] }, // yellow_harness
    Recipe { shaped: true, width: 3, height: 3, result: 535, count: 8, cells: &[&[195], &[195], &[195], &[195], &[1071], &[195], &[195], &[195], &[195]] }, // yellow_stained_glass
    Recipe { shaped: true, width: 3, height: 2, result: 551, count: 16, cells: &[&[535], &[535], &[535], &[535], &[535], &[535]] }, // yellow_stained_glass_pane
    Recipe { shaped: true, width: 3, height: 3, result: 551, count: 8, cells: &[&[409], &[409], &[409], &[409], &[1071], &[409], &[409], &[409], &[409]] }, // yellow_stained_glass_pane
    Recipe { shaped: true, width: 3, height: 3, result: 491, count: 8, cells: &[&[522], &[522], &[522], &[522], &[1071], &[522], &[522], &[522], &[522]] }, // yellow_terracotta
    Recipe { shaped: false, width: 2, height: 1, result: 2, count: 1, cells: &[&[4], &[902]] }, // granite
    Recipe { shaped: false, width: 2, height: 1, result: 6, count: 2, cells: &[&[4], &[35]] }, // andesite
    Recipe { shaped: false, width: 4, height: 1, result: 1030, count: 1, cells: &[&[1029], &[1029], &[1029], &[1018]] }, // book
    Recipe { shaped: false, width: 1, height: 1, result: 1031, count: 9, cells: &[&[725]] }, // slime_ball
    Recipe { shaped: false, width: 9, height: 1, result: 523, count: 1, cells: &[&[339], &[339], &[339], &[339], &[339], &[339], &[339], &[339], &[339]] }, // packed_ice
    Recipe { shaped: false, width: 1, height: 1, result: 36, count: 4, cells: &[&[134]] }, // oak_planks
    Recipe { shaped: false, width: 1, height: 1, result: 37, count: 4, cells: &[&[135]] }, // spruce_planks
    Recipe { shaped: false, width: 1, height: 1, result: 38, count: 4, cells: &[&[136]] }, // birch_planks
    Recipe { shaped: false, width: 1, height: 1, result: 39, count: 4, cells: &[&[137]] }, // jungle_planks
    Recipe { shaped: false, width: 1, height: 1, result: 40, count: 4, cells: &[&[138]] }, // acacia_planks
    Recipe { shaped: false, width: 1, height: 1, result: 41, count: 4, cells: &[&[139]] }, // cherry_planks
    Recipe { shaped: false, width: 1, height: 1, result: 42, count: 4, cells: &[&[141]] }, // dark_oak_planks
    Recipe { shaped: false, width: 1, height: 1, result: 43, count: 4, cells: &[&[140]] }, // pale_oak_planks
    Recipe { shaped: false, width: 1, height: 1, result: 1067, count: 1, cells: &[&[243, 1083]] }, // white_dye
    Recipe { shaped: false, width: 1, height: 1, result: 44, count: 4, cells: &[&[142]] }, // mangrove_planks
    Recipe { shaped: false, width: 1, height: 1, result: 1068, count: 1, cells: &[&[231, 238, 245]] }, // orange_dye
    Recipe { shaped: false, width: 2, height: 1, result: 1068, count: 2, cells: &[&[1081], &[1071]] }, // orange_dye
    Recipe { shaped: false, width: 1, height: 1, result: 45, count: 2, cells: &[&[147]] }, // bamboo_planks
    Recipe { shaped: false, width: 1, height: 1, result: 1069, count: 1, cells: &[&[235]] }, // magenta_dye
    Recipe { shaped: false, width: 3, height: 1, result: 1069, count: 3, cells: &[&[1078], &[1081], &[1073]] }, // magenta_dye
    Recipe { shaped: false, width: 4, height: 1, result: 1069, count: 4, cells: &[&[1078], &[1081], &[1081], &[1067]] }, // magenta_dye
    Recipe { shaped: false, width: 1, height: 1, result: 1069, count: 2, cells: &[&[526]] }, // magenta_dye
    Recipe { shaped: false, width: 2, height: 1, result: 1069, count: 2, cells: &[&[1077], &[1073]] }, // magenta_dye
    Recipe { shaped: false, width: 1, height: 1, result: 46, count: 4, cells: &[&[145]] }, // crimson_planks
    Recipe { shaped: false, width: 1, height: 1, result: 1070, count: 1, cells: &[&[234]] }, // light_blue_dye
    Recipe { shaped: false, width: 2, height: 1, result: 1070, count: 2, cells: &[&[1078], &[1067]] }, // light_blue_dye
    Recipe { shaped: false, width: 1, height: 1, result: 47, count: 4, cells: &[&[146]] }, // warped_planks
    Recipe { shaped: false, width: 1, height: 1, result: 1071, count: 1, cells: &[&[229, 230, 260]] }, // yellow_dye
    Recipe { shaped: false, width: 1, height: 1, result: 1071, count: 2, cells: &[&[525]] }, // yellow_dye
    Recipe { shaped: false, width: 2, height: 1, result: 1072, count: 2, cells: &[&[1080], &[1067]] }, // lime_dye
    Recipe { shaped: false, width: 1, height: 1, result: 1073, count: 1, cells: &[&[240, 259, 342]] }, // pink_dye
    Recipe { shaped: false, width: 1, height: 1, result: 1073, count: 2, cells: &[&[528]] }, // pink_dye
    Recipe { shaped: false, width: 2, height: 1, result: 1073, count: 2, cells: &[&[1081], &[1067]] }, // pink_dye
    Recipe { shaped: false, width: 2, height: 1, result: 1074, count: 2, cells: &[&[1082], &[1067]] }, // gray_dye
    Recipe { shaped: false, width: 1, height: 1, result: 1074, count: 1, cells: &[&[232]] }, // gray_dye
    Recipe { shaped: false, width: 1, height: 1, result: 1075, count: 1, cells: &[&[236, 239, 241]] }, // light_gray_dye
    Recipe { shaped: false, width: 3, height: 1, result: 1075, count: 3, cells: &[&[1082], &[1067], &[1067]] }, // light_gray_dye
    Recipe { shaped: false, width: 2, height: 1, result: 1075, count: 2, cells: &[&[1074], &[1067]] }, // light_gray_dye
    Recipe { shaped: false, width: 2, height: 1, result: 1076, count: 2, cells: &[&[1078], &[1080]] }, // cyan_dye
    Recipe { shaped: false, width: 1, height: 1, result: 1076, count: 2, cells: &[&[246]] }, // cyan_dye
    Recipe { shaped: false, width: 9, height: 1, result: 564, count: 1, cells: &[&[1248], &[1248], &[1248], &[1248], &[1248], &[1248], &[1248], &[1248], &[1248]] }, // prismarine_bricks
    Recipe { shaped: false, width: 2, height: 1, result: 1077, count: 2, cells: &[&[1078], &[1081]] }, // purple_dye
    Recipe { shaped: false, width: 1, height: 1, result: 1078, count: 1, cells: &[&[242, 901]] }, // blue_dye
    Recipe { shaped: false, width: 1, height: 1, result: 1079, count: 1, cells: &[&[1066]] }, // brown_dye
    Recipe { shaped: false, width: 1, height: 1, result: 1081, count: 1, cells: &[&[233, 237, 1288]] }, // red_dye
    Recipe { shaped: false, width: 1, height: 1, result: 1081, count: 2, cells: &[&[527]] }, // red_dye
    Recipe { shaped: false, width: 1, height: 1, result: 1082, count: 1, cells: &[&[244, 1064]] }, // black_dye
    Recipe { shaped: false, width: 1, height: 1, result: 1083, count: 3, cells: &[&[1084]] }, // bone_meal
    Recipe { shaped: false, width: 1, height: 1, result: 1083, count: 9, cells: &[&[580]] }, // bone_meal
    Recipe { shaped: false, width: 1, height: 1, result: 1085, count: 3, cells: &[&[1382]] }, // sugar
    Recipe { shaped: false, width: 1, height: 1, result: 1085, count: 1, cells: &[&[257]] }, // sugar
    Recipe { shaped: false, width: 9, height: 1, result: 577, count: 1, cells: &[&[1120], &[1120], &[1120], &[1120], &[1120], &[1120], &[1120], &[1120], &[1120]] }, // nether_wart_block
    Recipe { shaped: false, width: 1, height: 1, result: 1108, count: 9, cells: &[&[1028]] }, // dried_kelp
    Recipe { shaped: false, width: 1, height: 1, result: 1109, count: 4, cells: &[&[357]] }, // pumpkin_seeds
    Recipe { shaped: false, width: 1, height: 1, result: 1110, count: 1, cells: &[&[1107]] }, // melon_seeds
    Recipe { shaped: false, width: 1, height: 1, result: 1119, count: 9, cells: &[&[909]] }, // gold_nugget
    Recipe { shaped: false, width: 3, height: 1, result: 1124, count: 1, cells: &[&[1123], &[248], &[1085]] }, // fermented_spider_eye
    Recipe { shaped: false, width: 1, height: 1, result: 1125, count: 2, cells: &[&[1117]] }, // blaze_powder
    Recipe { shaped: false, width: 2, height: 1, result: 1126, count: 1, cells: &[&[1125], &[1031]] }, // magma_cream
    Recipe { shaped: false, width: 2, height: 1, result: 1129, count: 1, cells: &[&[1116], &[1125]] }, // ender_eye
    Recipe { shaped: false, width: 2, height: 1, result: 114, count: 1, cells: &[&[91], &[1379]] }, // waxed_copper_block
    Recipe { shaped: false, width: 2, height: 1, result: 115, count: 1, cells: &[&[95], &[1379]] }, // waxed_exposed_copper
    Recipe { shaped: false, width: 2, height: 1, result: 116, count: 1, cells: &[&[96], &[1379]] }, // waxed_weathered_copper
    Recipe { shaped: false, width: 2, height: 1, result: 117, count: 1, cells: &[&[97], &[1379]] }, // waxed_oxidized_copper
    Recipe { shaped: false, width: 9, height: 1, result: 631, count: 8, cells: &[&[1067], &[59], &[59], &[59], &[59], &[63], &[63], &[63], &[63]] }, // white_concrete_powder
    Recipe { shaped: false, width: 9, height: 1, result: 632, count: 8, cells: &[&[1068], &[59], &[59], &[59], &[59], &[63], &[63], &[63], &[63]] }, // orange_concrete_powder
    Recipe { shaped: false, width: 9, height: 1, result: 633, count: 8, cells: &[&[1069], &[59], &[59], &[59], &[59], &[63], &[63], &[63], &[63]] }, // magenta_concrete_powder
    Recipe { shaped: false, width: 9, height: 1, result: 634, count: 8, cells: &[&[1070], &[59], &[59], &[59], &[59], &[63], &[63], &[63], &[63]] }, // light_blue_concrete_powder
    Recipe { shaped: false, width: 9, height: 1, result: 635, count: 8, cells: &[&[1071], &[59], &[59], &[59], &[59], &[63], &[63], &[63], &[63]] }, // yellow_concrete_powder
    Recipe { shaped: false, width: 9, height: 1, result: 636, count: 8, cells: &[&[1072], &[59], &[59], &[59], &[59], &[63], &[63], &[63], &[63]] }, // lime_concrete_powder
    Recipe { shaped: false, width: 9, height: 1, result: 637, count: 8, cells: &[&[1073], &[59], &[59], &[59], &[59], &[63], &[63], &[63], &[63]] }, // pink_concrete_powder
    Recipe { shaped: false, width: 9, height: 1, result: 638, count: 8, cells: &[&[1074], &[59], &[59], &[59], &[59], &[63], &[63], &[63], &[63]] }, // gray_concrete_powder
    Recipe { shaped: false, width: 9, height: 1, result: 639, count: 8, cells: &[&[1075], &[59], &[59], &[59], &[59], &[63], &[63], &[63], &[63]] }, // light_gray_concrete_powder
    Recipe { shaped: false, width: 9, height: 1, result: 640, count: 8, cells: &[&[1076], &[59], &[59], &[59], &[59], &[63], &[63], &[63], &[63]] }, // cyan_concrete_powder
    Recipe { shaped: false, width: 9, height: 1, result: 641, count: 8, cells: &[&[1077], &[59], &[59], &[59], &[59], &[63], &[63], &[63], &[63]] }, // purple_concrete_powder
    Recipe { shaped: false, width: 9, height: 1, result: 642, count: 8, cells: &[&[1078], &[59], &[59], &[59], &[59], &[63], &[63], &[63], &[63]] }, // blue_concrete_powder
    Recipe { shaped: false, width: 9, height: 1, result: 643, count: 8, cells: &[&[1079], &[59], &[59], &[59], &[59], &[63], &[63], &[63], &[63]] }, // brown_concrete_powder
    Recipe { shaped: false, width: 9, height: 1, result: 644, count: 8, cells: &[&[1080], &[59], &[59], &[59], &[59], &[63], &[63], &[63], &[63]] }, // green_concrete_powder
    Recipe { shaped: false, width: 9, height: 1, result: 645, count: 8, cells: &[&[1081], &[59], &[59], &[59], &[59], &[63], &[63], &[63], &[63]] }, // red_concrete_powder
    Recipe { shaped: false, width: 9, height: 1, result: 646, count: 8, cells: &[&[1082], &[59], &[59], &[59], &[59], &[63], &[63], &[63], &[63]] }, // black_concrete_powder
    Recipe { shaped: false, width: 2, height: 1, result: 144, count: 1, cells: &[&[32], &[143]] }, // muddy_mangrove_roots
    Recipe { shaped: false, width: 9, height: 1, result: 147, count: 1, cells: &[&[270], &[270], &[270], &[270], &[270], &[270], &[270], &[270], &[270]] }, // bamboo_block
    Recipe { shaped: false, width: 9, height: 1, result: 680, count: 1, cells: &[&[523], &[523], &[523], &[523], &[523], &[523], &[523], &[523], &[523]] }, // blue_ice
    Recipe { shaped: false, width: 3, height: 1, result: 1219, count: 3, cells: &[&[951], &[1125], &[897]] }, // fire_charge
    Recipe { shaped: false, width: 1, height: 1, result: 1220, count: 4, cells: &[&[1223]] }, // wind_charge
    Recipe { shaped: false, width: 3, height: 1, result: 1221, count: 1, cells: &[&[1030], &[1064], &[950]] }, // writable_book
    Recipe { shaped: false, width: 2, height: 1, result: 1226, count: 1, cells: &[&[1225], &[1065]] }, // glow_item_frame
    Recipe { shaped: false, width: 1, height: 1, result: 718, count: 9, cells: &[&[720]] }, // redstone
    Recipe { shaped: false, width: 2, height: 1, result: 214, count: 1, cells: &[&[1068], &[228]] }, // orange_wool
    Recipe { shaped: false, width: 2, height: 1, result: 215, count: 1, cells: &[&[1069], &[228]] }, // magenta_wool
    Recipe { shaped: false, width: 2, height: 1, result: 216, count: 1, cells: &[&[1070], &[228]] }, // light_blue_wool
    Recipe { shaped: false, width: 2, height: 1, result: 217, count: 1, cells: &[&[1071], &[228]] }, // yellow_wool
    Recipe { shaped: false, width: 2, height: 1, result: 218, count: 1, cells: &[&[1072], &[228]] }, // lime_wool
    Recipe { shaped: false, width: 3, height: 1, result: 1242, count: 1, cells: &[&[357], &[1085], &[1032]] }, // pumpkin_pie
    Recipe { shaped: false, width: 2, height: 1, result: 219, count: 1, cells: &[&[1073], &[228]] }, // pink_wool
    Recipe { shaped: false, width: 2, height: 1, result: 1243, count: 3, cells: &[&[951], &[1029]] }, // firework_rocket
    Recipe { shaped: false, width: 2, height: 1, result: 220, count: 1, cells: &[&[1074], &[228]] }, // gray_wool
    Recipe { shaped: false, width: 2, height: 1, result: 221, count: 1, cells: &[&[1075], &[228]] }, // light_gray_wool
    Recipe { shaped: false, width: 2, height: 1, result: 222, count: 1, cells: &[&[1076], &[228]] }, // cyan_wool
    Recipe { shaped: false, width: 2, height: 1, result: 223, count: 1, cells: &[&[1077], &[228]] }, // purple_wool
    Recipe { shaped: false, width: 2, height: 1, result: 224, count: 1, cells: &[&[1078], &[228]] }, // blue_wool
    Recipe { shaped: false, width: 2, height: 1, result: 225, count: 1, cells: &[&[1079], &[228]] }, // brown_wool
    Recipe { shaped: false, width: 2, height: 1, result: 226, count: 1, cells: &[&[1080], &[228]] }, // green_wool
    Recipe { shaped: false, width: 2, height: 1, result: 738, count: 1, cells: &[&[734], &[1379]] }, // waxed_lightning_rod
    Recipe { shaped: false, width: 2, height: 1, result: 227, count: 1, cells: &[&[1081], &[228]] }, // red_wool
    Recipe { shaped: false, width: 2, height: 1, result: 739, count: 1, cells: &[&[735], &[1379]] }, // waxed_exposed_lightning_rod
    Recipe { shaped: false, width: 2, height: 1, result: 228, count: 1, cells: &[&[1082], &[224]] }, // black_wool
    Recipe { shaped: false, width: 5, height: 1, result: 1252, count: 1, cells: &[&[1230], &[1251], &[893], &[1228], &[248, 249]] }, // rabbit_stew
    Recipe { shaped: false, width: 2, height: 1, result: 740, count: 1, cells: &[&[736], &[1379]] }, // waxed_weathered_lightning_rod
    Recipe { shaped: false, width: 2, height: 1, result: 741, count: 1, cells: &[&[737], &[1379]] }, // waxed_oxidized_lightning_rod
    Recipe { shaped: false, width: 2, height: 1, result: 746, count: 1, cells: &[&[332], &[745]] }, // trapped_chest
    Recipe { shaped: false, width: 1, height: 1, result: 750, count: 1, cells: &[&[1]] }, // stone_button
    Recipe { shaped: false, width: 1, height: 1, result: 751, count: 1, cells: &[&[1390]] }, // polished_blackstone_button
    Recipe { shaped: false, width: 1, height: 1, result: 752, count: 1, cells: &[&[36]] }, // oak_button
    Recipe { shaped: false, width: 1, height: 1, result: 753, count: 1, cells: &[&[37]] }, // spruce_button
    Recipe { shaped: false, width: 1, height: 1, result: 754, count: 1, cells: &[&[38]] }, // birch_button
    Recipe { shaped: false, width: 1, height: 1, result: 755, count: 1, cells: &[&[39]] }, // jungle_button
    Recipe { shaped: false, width: 1, height: 1, result: 756, count: 1, cells: &[&[40]] }, // acacia_button
    Recipe { shaped: false, width: 1, height: 1, result: 757, count: 1, cells: &[&[41]] }, // cherry_button
    Recipe { shaped: false, width: 1, height: 1, result: 758, count: 1, cells: &[&[42]] }, // dark_oak_button
    Recipe { shaped: false, width: 1, height: 1, result: 759, count: 1, cells: &[&[43]] }, // pale_oak_button
    Recipe { shaped: false, width: 1, height: 1, result: 760, count: 1, cells: &[&[44]] }, // mangrove_button
    Recipe { shaped: false, width: 1, height: 1, result: 761, count: 1, cells: &[&[45]] }, // bamboo_button
    Recipe { shaped: false, width: 1, height: 1, result: 762, count: 1, cells: &[&[46]] }, // crimson_button
    Recipe { shaped: false, width: 1, height: 1, result: 763, count: 1, cells: &[&[47]] }, // warped_button
    Recipe { shaped: false, width: 7, height: 1, result: 1290, count: 1, cells: &[&[893], &[1288], &[1288], &[1288], &[1288], &[1288], &[1288]] }, // beetroot_soup
    Recipe { shaped: false, width: 1, height: 1, result: 1306, count: 9, cells: &[&[905]] }, // iron_nugget
    Recipe { shaped: false, width: 1, height: 1, result: 1307, count: 9, cells: &[&[907]] }, // copper_nugget
    Recipe { shaped: false, width: 2, height: 1, result: 797, count: 1, cells: &[&[793], &[1379]] }, // waxed_copper_door
    Recipe { shaped: false, width: 2, height: 1, result: 798, count: 1, cells: &[&[794], &[1379]] }, // waxed_exposed_copper_door
    Recipe { shaped: false, width: 2, height: 1, result: 799, count: 1, cells: &[&[795], &[1379]] }, // waxed_weathered_copper_door
    Recipe { shaped: false, width: 2, height: 1, result: 800, count: 1, cells: &[&[796], &[1379]] }, // waxed_oxidized_copper_door
    Recipe { shaped: false, width: 9, height: 1, result: 1327, count: 1, cells: &[&[1331], &[1331], &[1331], &[1331], &[1331], &[1331], &[1331], &[1331], &[1331]] }, // music_disc_5
    Recipe { shaped: false, width: 2, height: 1, result: 818, count: 1, cells: &[&[814], &[1379]] }, // waxed_copper_trapdoor
    Recipe { shaped: false, width: 2, height: 1, result: 819, count: 1, cells: &[&[815], &[1379]] }, // waxed_exposed_copper_trapdoor
    Recipe { shaped: false, width: 2, height: 1, result: 820, count: 1, cells: &[&[816], &[1379]] }, // waxed_weathered_copper_trapdoor
    Recipe { shaped: false, width: 2, height: 1, result: 821, count: 1, cells: &[&[817], &[1379]] }, // waxed_oxidized_copper_trapdoor
    Recipe { shaped: false, width: 4, height: 1, result: 1341, count: 1, cells: &[&[893], &[248], &[249], &[229, 230, 231, 232, 233, 234, 235, 236, 237, 238, 239, 240, 241, 242, 243, 244, 245]] }, // suspicious_stew
    Recipe { shaped: false, width: 2, height: 1, result: 1343, count: 1, cells: &[&[1029], &[241]] }, // flower_banner_pattern
    Recipe { shaped: false, width: 2, height: 1, result: 1344, count: 1, cells: &[&[1029], &[1238]] }, // creeper_banner_pattern
    Recipe { shaped: false, width: 2, height: 1, result: 321, count: 1, cells: &[&[35], &[263, 411]] }, // mossy_cobblestone
    Recipe { shaped: false, width: 2, height: 1, result: 1345, count: 1, cells: &[&[1029], &[1235]] }, // skull_banner_pattern
    Recipe { shaped: false, width: 2, height: 1, result: 1346, count: 1, cells: &[&[1029], &[988]] }, // mojang_banner_pattern
    Recipe { shaped: false, width: 2, height: 1, result: 1351, count: 1, cells: &[&[1029], &[305]] }, // field_masoned_banner_pattern
    Recipe { shaped: false, width: 2, height: 1, result: 1352, count: 1, cells: &[&[1029], &[411]] }, // bordure_indented_banner_pattern
    Recipe { shaped: false, width: 2, height: 1, result: 856, count: 1, cells: &[&[332], &[855]] }, // chest_minecart
    Recipe { shaped: false, width: 2, height: 1, result: 857, count: 1, cells: &[&[335], &[855]] }, // furnace_minecart
    Recipe { shaped: false, width: 2, height: 1, result: 858, count: 1, cells: &[&[747], &[855]] }, // tnt_minecart
    Recipe { shaped: false, width: 2, height: 1, result: 1370, count: 1, cells: &[&[1366], &[1379]] }, // waxed_copper_lantern
    Recipe { shaped: false, width: 2, height: 1, result: 859, count: 1, cells: &[&[728], &[855]] }, // hopper_minecart
    Recipe { shaped: false, width: 2, height: 1, result: 1371, count: 1, cells: &[&[1367], &[1379]] }, // waxed_exposed_copper_lantern
    Recipe { shaped: false, width: 2, height: 1, result: 1372, count: 1, cells: &[&[1368], &[1379]] }, // waxed_weathered_copper_lantern
    Recipe { shaped: false, width: 2, height: 1, result: 1373, count: 1, cells: &[&[1369], &[1379]] }, // waxed_oxidized_copper_lantern
    Recipe { shaped: false, width: 2, height: 1, result: 865, count: 1, cells: &[&[332], &[864]] }, // oak_chest_boat
    Recipe { shaped: false, width: 2, height: 1, result: 867, count: 1, cells: &[&[332], &[866]] }, // spruce_chest_boat
    Recipe { shaped: false, width: 2, height: 1, result: 869, count: 1, cells: &[&[332], &[868]] }, // birch_chest_boat
    Recipe { shaped: false, width: 5, height: 1, result: 1382, count: 4, cells: &[&[726], &[1121], &[1121], &[1121], &[1121]] }, // honey_bottle
    Recipe { shaped: false, width: 2, height: 1, result: 871, count: 1, cells: &[&[332], &[870]] }, // jungle_chest_boat
    Recipe { shaped: false, width: 2, height: 1, result: 873, count: 1, cells: &[&[332], &[872]] }, // acacia_chest_boat
    Recipe { shaped: false, width: 2, height: 1, result: 875, count: 1, cells: &[&[332], &[874]] }, // cherry_chest_boat
    Recipe { shaped: false, width: 2, height: 1, result: 877, count: 1, cells: &[&[332], &[876]] }, // dark_oak_chest_boat
    Recipe { shaped: false, width: 2, height: 1, result: 879, count: 1, cells: &[&[332], &[878]] }, // pale_oak_chest_boat
    Recipe { shaped: false, width: 2, height: 1, result: 881, count: 1, cells: &[&[332], &[880]] }, // mangrove_chest_boat
    Recipe { shaped: false, width: 2, height: 1, result: 883, count: 1, cells: &[&[332], &[882]] }, // bamboo_chest_raft
    Recipe { shaped: false, width: 2, height: 1, result: 1400, count: 1, cells: &[&[1399], &[1067]] }, // white_candle
    Recipe { shaped: false, width: 2, height: 1, result: 377, count: 1, cells: &[&[376], &[263, 411]] }, // mossy_stone_bricks
    Recipe { shaped: false, width: 2, height: 1, result: 1401, count: 1, cells: &[&[1399], &[1068]] }, // orange_candle
    Recipe { shaped: false, width: 2, height: 1, result: 1402, count: 1, cells: &[&[1399], &[1069]] }, // magenta_candle
    Recipe { shaped: false, width: 2, height: 1, result: 1403, count: 1, cells: &[&[1399], &[1070]] }, // light_blue_candle
    Recipe { shaped: false, width: 2, height: 1, result: 892, count: 1, cells: &[&[905], &[983]] }, // flint_and_steel
    Recipe { shaped: false, width: 2, height: 1, result: 380, count: 1, cells: &[&[32], &[953]] }, // packed_mud
    Recipe { shaped: false, width: 2, height: 1, result: 1404, count: 1, cells: &[&[1399], &[1071]] }, // yellow_candle
    Recipe { shaped: false, width: 2, height: 1, result: 1405, count: 1, cells: &[&[1399], &[1072]] }, // lime_candle
    Recipe { shaped: false, width: 2, height: 1, result: 1406, count: 1, cells: &[&[1399], &[1073]] }, // pink_candle
    Recipe { shaped: false, width: 2, height: 1, result: 1407, count: 1, cells: &[&[1399], &[1074]] }, // gray_candle
    Recipe { shaped: false, width: 2, height: 1, result: 1408, count: 1, cells: &[&[1399], &[1075]] }, // light_gray_candle
    Recipe { shaped: false, width: 1, height: 1, result: 897, count: 9, cells: &[&[83]] }, // coal
    Recipe { shaped: false, width: 2, height: 1, result: 1409, count: 1, cells: &[&[1399], &[1076]] }, // cyan_candle
    Recipe { shaped: false, width: 2, height: 1, result: 1410, count: 1, cells: &[&[1399], &[1077]] }, // purple_candle
    Recipe { shaped: false, width: 2, height: 1, result: 1411, count: 1, cells: &[&[1399], &[1078]] }, // blue_candle
    Recipe { shaped: false, width: 1, height: 1, result: 899, count: 9, cells: &[&[93]] }, // diamond
    Recipe { shaped: false, width: 2, height: 1, result: 1412, count: 1, cells: &[&[1399], &[1079]] }, // brown_candle
    Recipe { shaped: false, width: 1, height: 1, result: 900, count: 9, cells: &[&[441]] }, // emerald
    Recipe { shaped: false, width: 2, height: 1, result: 1413, count: 1, cells: &[&[1399], &[1080]] }, // green_candle
    Recipe { shaped: false, width: 1, height: 1, result: 901, count: 9, cells: &[&[197]] }, // lapis_lazuli
    Recipe { shaped: false, width: 2, height: 1, result: 1414, count: 1, cells: &[&[1399], &[1081]] }, // red_candle
    Recipe { shaped: false, width: 2, height: 1, result: 1415, count: 1, cells: &[&[1399], &[1082]] }, // black_candle
    Recipe { shaped: false, width: 1, height: 1, result: 904, count: 9, cells: &[&[84]] }, // raw_iron
    Recipe { shaped: false, width: 1, height: 1, result: 906, count: 9, cells: &[&[85]] }, // raw_copper
    Recipe { shaped: false, width: 1, height: 1, result: 908, count: 9, cells: &[&[86]] }, // raw_gold
    Recipe { shaped: false, width: 2, height: 1, result: 396, count: 1, cells: &[&[392], &[1379]] }, // waxed_copper_bars
    Recipe { shaped: false, width: 2, height: 1, result: 397, count: 1, cells: &[&[393], &[1379]] }, // waxed_exposed_copper_bars
    Recipe { shaped: false, width: 8, height: 1, result: 910, count: 1, cells: &[&[911], &[911], &[911], &[911], &[909], &[909], &[909], &[909]] }, // netherite_ingot
    Recipe { shaped: false, width: 1, height: 1, result: 910, count: 9, cells: &[&[94]] }, // netherite_ingot
    Recipe { shaped: false, width: 2, height: 1, result: 398, count: 1, cells: &[&[394], &[1379]] }, // waxed_weathered_copper_bars
    Recipe { shaped: false, width: 2, height: 1, result: 399, count: 1, cells: &[&[395], &[1379]] }, // waxed_oxidized_copper_bars
    Recipe { shaped: false, width: 2, height: 1, result: 405, count: 1, cells: &[&[401], &[1379]] }, // waxed_copper_chain
    Recipe { shaped: false, width: 2, height: 1, result: 406, count: 1, cells: &[&[402], &[1379]] }, // waxed_exposed_copper_chain
    Recipe { shaped: false, width: 2, height: 1, result: 407, count: 1, cells: &[&[403], &[1379]] }, // waxed_weathered_copper_chain
    Recipe { shaped: false, width: 2, height: 1, result: 408, count: 1, cells: &[&[404], &[1379]] }, // waxed_oxidized_copper_chain
    Recipe { shaped: false, width: 9, height: 1, result: 410, count: 1, cells: &[&[1107], &[1107], &[1107], &[1107], &[1107], &[1107], &[1107], &[1107], &[1107]] }, // melon
    Recipe { shaped: false, width: 1, height: 1, result: 413, count: 9, cells: &[&[414]] }, // resin_clump
    Recipe { shaped: false, width: 3, height: 1, result: 948, count: 1, cells: &[&[248], &[249], &[893]] }, // mushroom_stew
    Recipe { shaped: false, width: 1, height: 1, result: 953, count: 9, cells: &[&[505]] }, // wheat
    Recipe { shaped: false, width: 2, height: 1, result: 1489, count: 1, cells: &[&[1485], &[1379]] }, // waxed_copper_chest
    Recipe { shaped: false, width: 2, height: 1, result: 1490, count: 1, cells: &[&[1486], &[1379]] }, // waxed_exposed_copper_chest
    Recipe { shaped: false, width: 2, height: 1, result: 1491, count: 1, cells: &[&[1487], &[1379]] }, // waxed_weathered_copper_chest
    Recipe { shaped: false, width: 2, height: 1, result: 1492, count: 1, cells: &[&[1488], &[1379]] }, // waxed_oxidized_copper_chest
    Recipe { shaped: false, width: 2, height: 1, result: 1497, count: 1, cells: &[&[1493], &[1379]] }, // waxed_copper_golem_statue
    Recipe { shaped: false, width: 2, height: 1, result: 1498, count: 1, cells: &[&[1494], &[1379]] }, // waxed_exposed_copper_golem_statue
    Recipe { shaped: false, width: 2, height: 1, result: 1499, count: 1, cells: &[&[1495], &[1379]] }, // waxed_weathered_copper_golem_statue
    Recipe { shaped: false, width: 2, height: 1, result: 1500, count: 1, cells: &[&[1496], &[1379]] }, // waxed_oxidized_copper_golem_statue
    Recipe { shaped: false, width: 9, height: 1, result: 505, count: 1, cells: &[&[953], &[953], &[953], &[953], &[953], &[953], &[953], &[953], &[953]] }, // hay_block
];
