//! Внутренний каталог не является версией сетевого протокола.
//! Preview бетонных и шерстяных плит: выбранные pinned JSON, базовые ID775 неизменны.
use crate::{JavaVersion, ids::{self, RegistryKind}};

/// Неизменяемые внутренние идентичности; существующие строки нельзя перенумеровывать.
/// Источники: selected concrete-slabs-26.3.json / wool-slabs-26.3.json; не игровое наблюдение.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SlabFamily { Concrete, Wool, Mineral, Poplar }

/// Форма блока каталога: плита (6 состояний), ступени (80), цельный блок (1), стенка (324),
/// «мощная сера» (5 состояний), серный шип (20 состояний) и «Wood» — прочие блоки тополя,
/// у которых есть дубовый двойник с теми же свойствами (`input_name`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BlockForm { Slab, Stairs, Full, Wall, PotentSulfur, Spike, Wood, Plant, Boat, Item }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SlabDescriptor {
    pub family: SlabFamily,
    pub form: BlockForm,
    pub name: &'static str,
    pub item: i32,
    pub block: i32,
    pub first_state: i32,
    pub last_state: i32,
    pub default_state: i32,
    pub wire_item: u32,
    pub wire_block: u32,
    pub wire_first_state: u32,
    pub input_name: &'static str,
    pub input_item: i32,
}

/// Совместимое имя прежнего API; принадлежность задаётся явным family.
pub type ConcreteSlab = SlabDescriptor;

pub static CONCRETE_SLABS: [ConcreteSlab; 16] = [
    ConcreteSlab { family: SlabFamily::Concrete, form: BlockForm::Slab, name: "white_concrete_slab", item: 1506, block: 1168, first_state: 29873, last_state: 29878, default_state: 29876, wire_item: 721, wire_block: 800, wire_first_state: 18307, input_name: "white_concrete", input_item: 615 },
    ConcreteSlab { family: SlabFamily::Concrete, form: BlockForm::Slab, name: "orange_concrete_slab", item: 1507, block: 1169, first_state: 29879, last_state: 29884, default_state: 29882, wire_item: 722, wire_block: 801, wire_first_state: 18313, input_name: "orange_concrete", input_item: 616 },
    ConcreteSlab { family: SlabFamily::Concrete, form: BlockForm::Slab, name: "magenta_concrete_slab", item: 1508, block: 1170, first_state: 29885, last_state: 29890, default_state: 29888, wire_item: 723, wire_block: 802, wire_first_state: 18319, input_name: "magenta_concrete", input_item: 617 },
    ConcreteSlab { family: SlabFamily::Concrete, form: BlockForm::Slab, name: "light_blue_concrete_slab", item: 1509, block: 1171, first_state: 29891, last_state: 29896, default_state: 29894, wire_item: 724, wire_block: 803, wire_first_state: 18325, input_name: "light_blue_concrete", input_item: 618 },
    ConcreteSlab { family: SlabFamily::Concrete, form: BlockForm::Slab, name: "yellow_concrete_slab", item: 1510, block: 1172, first_state: 29897, last_state: 29902, default_state: 29900, wire_item: 725, wire_block: 804, wire_first_state: 18331, input_name: "yellow_concrete", input_item: 619 },
    ConcreteSlab { family: SlabFamily::Concrete, form: BlockForm::Slab, name: "lime_concrete_slab", item: 1511, block: 1173, first_state: 29903, last_state: 29908, default_state: 29906, wire_item: 726, wire_block: 805, wire_first_state: 18337, input_name: "lime_concrete", input_item: 620 },
    ConcreteSlab { family: SlabFamily::Concrete, form: BlockForm::Slab, name: "pink_concrete_slab", item: 1512, block: 1174, first_state: 29909, last_state: 29914, default_state: 29912, wire_item: 727, wire_block: 806, wire_first_state: 18343, input_name: "pink_concrete", input_item: 621 },
    ConcreteSlab { family: SlabFamily::Concrete, form: BlockForm::Slab, name: "gray_concrete_slab", item: 1513, block: 1175, first_state: 29915, last_state: 29920, default_state: 29918, wire_item: 728, wire_block: 807, wire_first_state: 18349, input_name: "gray_concrete", input_item: 622 },
    ConcreteSlab { family: SlabFamily::Concrete, form: BlockForm::Slab, name: "light_gray_concrete_slab", item: 1514, block: 1176, first_state: 29921, last_state: 29926, default_state: 29924, wire_item: 729, wire_block: 808, wire_first_state: 18355, input_name: "light_gray_concrete", input_item: 623 },
    ConcreteSlab { family: SlabFamily::Concrete, form: BlockForm::Slab, name: "cyan_concrete_slab", item: 1515, block: 1177, first_state: 29927, last_state: 29932, default_state: 29930, wire_item: 730, wire_block: 809, wire_first_state: 18361, input_name: "cyan_concrete", input_item: 624 },
    ConcreteSlab { family: SlabFamily::Concrete, form: BlockForm::Slab, name: "purple_concrete_slab", item: 1516, block: 1178, first_state: 29933, last_state: 29938, default_state: 29936, wire_item: 731, wire_block: 810, wire_first_state: 18367, input_name: "purple_concrete", input_item: 625 },
    ConcreteSlab { family: SlabFamily::Concrete, form: BlockForm::Slab, name: "blue_concrete_slab", item: 1517, block: 1179, first_state: 29939, last_state: 29944, default_state: 29942, wire_item: 732, wire_block: 811, wire_first_state: 18373, input_name: "blue_concrete", input_item: 626 },
    ConcreteSlab { family: SlabFamily::Concrete, form: BlockForm::Slab, name: "brown_concrete_slab", item: 1518, block: 1180, first_state: 29945, last_state: 29950, default_state: 29948, wire_item: 733, wire_block: 812, wire_first_state: 18379, input_name: "brown_concrete", input_item: 627 },
    ConcreteSlab { family: SlabFamily::Concrete, form: BlockForm::Slab, name: "green_concrete_slab", item: 1519, block: 1181, first_state: 29951, last_state: 29956, default_state: 29954, wire_item: 734, wire_block: 813, wire_first_state: 18385, input_name: "green_concrete", input_item: 628 },
    ConcreteSlab { family: SlabFamily::Concrete, form: BlockForm::Slab, name: "red_concrete_slab", item: 1520, block: 1182, first_state: 29957, last_state: 29962, default_state: 29960, wire_item: 735, wire_block: 814, wire_first_state: 18391, input_name: "red_concrete", input_item: 629 },
    ConcreteSlab { family: SlabFamily::Concrete, form: BlockForm::Slab, name: "black_concrete_slab", item: 1521, block: 1183, first_state: 29963, last_state: 29968, default_state: 29966, wire_item: 736, wire_block: 815, wire_first_state: 18397, input_name: "black_concrete", input_item: 630 },
];


/// Отдельное семейство: metadata не разрешает копировать правила добычи бетона.
pub static WOOL_SLABS: [SlabDescriptor; 16] = [
    SlabDescriptor { family: SlabFamily::Wool, form: BlockForm::Slab, name: "white_wool_slab", item: 1522, block: 1184, first_state: 29969, last_state: 29974, default_state: 29972, wire_item: 282, wire_block: 183, wire_first_state: 3705, input_name: "white_wool", input_item: 213 },
    SlabDescriptor { family: SlabFamily::Wool, form: BlockForm::Slab, name: "orange_wool_slab", item: 1523, block: 1185, first_state: 29975, last_state: 29980, default_state: 29978, wire_item: 283, wire_block: 184, wire_first_state: 3711, input_name: "orange_wool", input_item: 214 },
    SlabDescriptor { family: SlabFamily::Wool, form: BlockForm::Slab, name: "magenta_wool_slab", item: 1524, block: 1186, first_state: 29981, last_state: 29986, default_state: 29984, wire_item: 284, wire_block: 185, wire_first_state: 3717, input_name: "magenta_wool", input_item: 215 },
    SlabDescriptor { family: SlabFamily::Wool, form: BlockForm::Slab, name: "light_blue_wool_slab", item: 1525, block: 1187, first_state: 29987, last_state: 29992, default_state: 29990, wire_item: 285, wire_block: 186, wire_first_state: 3723, input_name: "light_blue_wool", input_item: 216 },
    SlabDescriptor { family: SlabFamily::Wool, form: BlockForm::Slab, name: "yellow_wool_slab", item: 1526, block: 1188, first_state: 29993, last_state: 29998, default_state: 29996, wire_item: 286, wire_block: 187, wire_first_state: 3729, input_name: "yellow_wool", input_item: 217 },
    SlabDescriptor { family: SlabFamily::Wool, form: BlockForm::Slab, name: "lime_wool_slab", item: 1527, block: 1189, first_state: 29999, last_state: 30004, default_state: 30002, wire_item: 287, wire_block: 188, wire_first_state: 3735, input_name: "lime_wool", input_item: 218 },
    SlabDescriptor { family: SlabFamily::Wool, form: BlockForm::Slab, name: "pink_wool_slab", item: 1528, block: 1190, first_state: 30005, last_state: 30010, default_state: 30008, wire_item: 288, wire_block: 189, wire_first_state: 3741, input_name: "pink_wool", input_item: 219 },
    SlabDescriptor { family: SlabFamily::Wool, form: BlockForm::Slab, name: "gray_wool_slab", item: 1529, block: 1191, first_state: 30011, last_state: 30016, default_state: 30014, wire_item: 289, wire_block: 190, wire_first_state: 3747, input_name: "gray_wool", input_item: 220 },
    SlabDescriptor { family: SlabFamily::Wool, form: BlockForm::Slab, name: "light_gray_wool_slab", item: 1530, block: 1192, first_state: 30017, last_state: 30022, default_state: 30020, wire_item: 290, wire_block: 191, wire_first_state: 3753, input_name: "light_gray_wool", input_item: 221 },
    SlabDescriptor { family: SlabFamily::Wool, form: BlockForm::Slab, name: "cyan_wool_slab", item: 1531, block: 1193, first_state: 30023, last_state: 30028, default_state: 30026, wire_item: 291, wire_block: 192, wire_first_state: 3759, input_name: "cyan_wool", input_item: 222 },
    SlabDescriptor { family: SlabFamily::Wool, form: BlockForm::Slab, name: "purple_wool_slab", item: 1532, block: 1194, first_state: 30029, last_state: 30034, default_state: 30032, wire_item: 292, wire_block: 193, wire_first_state: 3765, input_name: "purple_wool", input_item: 223 },
    SlabDescriptor { family: SlabFamily::Wool, form: BlockForm::Slab, name: "blue_wool_slab", item: 1533, block: 1195, first_state: 30035, last_state: 30040, default_state: 30038, wire_item: 293, wire_block: 194, wire_first_state: 3771, input_name: "blue_wool", input_item: 224 },
    SlabDescriptor { family: SlabFamily::Wool, form: BlockForm::Slab, name: "brown_wool_slab", item: 1534, block: 1196, first_state: 30041, last_state: 30046, default_state: 30044, wire_item: 294, wire_block: 195, wire_first_state: 3777, input_name: "brown_wool", input_item: 225 },
    SlabDescriptor { family: SlabFamily::Wool, form: BlockForm::Slab, name: "green_wool_slab", item: 1535, block: 1197, first_state: 30047, last_state: 30052, default_state: 30050, wire_item: 295, wire_block: 196, wire_first_state: 3783, input_name: "green_wool", input_item: 226 },
    SlabDescriptor { family: SlabFamily::Wool, form: BlockForm::Slab, name: "red_wool_slab", item: 1536, block: 1198, first_state: 30053, last_state: 30058, default_state: 30056, wire_item: 296, wire_block: 197, wire_first_state: 3789, input_name: "red_wool", input_item: 227 },
    SlabDescriptor { family: SlabFamily::Wool, form: BlockForm::Slab, name: "black_wool_slab", item: 1537, block: 1199, first_state: 30059, last_state: 30064, default_state: 30062, wire_item: 297, wire_block: 198, wire_first_state: 3795, input_name: "black_wool", input_item: 228 },
];

/// Ступени 26.3: внутренние ID дописаны после плит (предметы 1538–1569, блоки 1200–1231,
/// состояния 30065–32624). Источник — pinned blocks.json/items.json 777; порядок свойств
/// facing, half, shape, waterlogged, как у прежних ступеней.
pub static CONCRETE_STAIRS: [SlabDescriptor; 16] = [
    SlabDescriptor { family: SlabFamily::Concrete, form: BlockForm::Stairs, name: "white_concrete_stairs", item: 1538, block: 1200, first_state: 30065, last_state: 30144, default_state: 30076, wire_item: 705, wire_block: 784, wire_first_state: 17027, input_name: "white_concrete", input_item: 615 },
    SlabDescriptor { family: SlabFamily::Concrete, form: BlockForm::Stairs, name: "orange_concrete_stairs", item: 1539, block: 1201, first_state: 30145, last_state: 30224, default_state: 30156, wire_item: 706, wire_block: 785, wire_first_state: 17107, input_name: "orange_concrete", input_item: 616 },
    SlabDescriptor { family: SlabFamily::Concrete, form: BlockForm::Stairs, name: "magenta_concrete_stairs", item: 1540, block: 1202, first_state: 30225, last_state: 30304, default_state: 30236, wire_item: 707, wire_block: 786, wire_first_state: 17187, input_name: "magenta_concrete", input_item: 617 },
    SlabDescriptor { family: SlabFamily::Concrete, form: BlockForm::Stairs, name: "light_blue_concrete_stairs", item: 1541, block: 1203, first_state: 30305, last_state: 30384, default_state: 30316, wire_item: 708, wire_block: 787, wire_first_state: 17267, input_name: "light_blue_concrete", input_item: 618 },
    SlabDescriptor { family: SlabFamily::Concrete, form: BlockForm::Stairs, name: "yellow_concrete_stairs", item: 1542, block: 1204, first_state: 30385, last_state: 30464, default_state: 30396, wire_item: 709, wire_block: 788, wire_first_state: 17347, input_name: "yellow_concrete", input_item: 619 },
    SlabDescriptor { family: SlabFamily::Concrete, form: BlockForm::Stairs, name: "lime_concrete_stairs", item: 1543, block: 1205, first_state: 30465, last_state: 30544, default_state: 30476, wire_item: 710, wire_block: 789, wire_first_state: 17427, input_name: "lime_concrete", input_item: 620 },
    SlabDescriptor { family: SlabFamily::Concrete, form: BlockForm::Stairs, name: "pink_concrete_stairs", item: 1544, block: 1206, first_state: 30545, last_state: 30624, default_state: 30556, wire_item: 711, wire_block: 790, wire_first_state: 17507, input_name: "pink_concrete", input_item: 621 },
    SlabDescriptor { family: SlabFamily::Concrete, form: BlockForm::Stairs, name: "gray_concrete_stairs", item: 1545, block: 1207, first_state: 30625, last_state: 30704, default_state: 30636, wire_item: 712, wire_block: 791, wire_first_state: 17587, input_name: "gray_concrete", input_item: 622 },
    SlabDescriptor { family: SlabFamily::Concrete, form: BlockForm::Stairs, name: "light_gray_concrete_stairs", item: 1546, block: 1208, first_state: 30705, last_state: 30784, default_state: 30716, wire_item: 713, wire_block: 792, wire_first_state: 17667, input_name: "light_gray_concrete", input_item: 623 },
    SlabDescriptor { family: SlabFamily::Concrete, form: BlockForm::Stairs, name: "cyan_concrete_stairs", item: 1547, block: 1209, first_state: 30785, last_state: 30864, default_state: 30796, wire_item: 714, wire_block: 793, wire_first_state: 17747, input_name: "cyan_concrete", input_item: 624 },
    SlabDescriptor { family: SlabFamily::Concrete, form: BlockForm::Stairs, name: "purple_concrete_stairs", item: 1548, block: 1210, first_state: 30865, last_state: 30944, default_state: 30876, wire_item: 715, wire_block: 794, wire_first_state: 17827, input_name: "purple_concrete", input_item: 625 },
    SlabDescriptor { family: SlabFamily::Concrete, form: BlockForm::Stairs, name: "blue_concrete_stairs", item: 1549, block: 1211, first_state: 30945, last_state: 31024, default_state: 30956, wire_item: 716, wire_block: 795, wire_first_state: 17907, input_name: "blue_concrete", input_item: 626 },
    SlabDescriptor { family: SlabFamily::Concrete, form: BlockForm::Stairs, name: "brown_concrete_stairs", item: 1550, block: 1212, first_state: 31025, last_state: 31104, default_state: 31036, wire_item: 717, wire_block: 796, wire_first_state: 17987, input_name: "brown_concrete", input_item: 627 },
    SlabDescriptor { family: SlabFamily::Concrete, form: BlockForm::Stairs, name: "green_concrete_stairs", item: 1551, block: 1213, first_state: 31105, last_state: 31184, default_state: 31116, wire_item: 718, wire_block: 797, wire_first_state: 18067, input_name: "green_concrete", input_item: 628 },
    SlabDescriptor { family: SlabFamily::Concrete, form: BlockForm::Stairs, name: "red_concrete_stairs", item: 1552, block: 1214, first_state: 31185, last_state: 31264, default_state: 31196, wire_item: 719, wire_block: 798, wire_first_state: 18147, input_name: "red_concrete", input_item: 629 },
    SlabDescriptor { family: SlabFamily::Concrete, form: BlockForm::Stairs, name: "black_concrete_stairs", item: 1553, block: 1215, first_state: 31265, last_state: 31344, default_state: 31276, wire_item: 720, wire_block: 799, wire_first_state: 18227, input_name: "black_concrete", input_item: 630 },
];

pub static WOOL_STAIRS: [SlabDescriptor; 16] = [
    SlabDescriptor { family: SlabFamily::Wool, form: BlockForm::Stairs, name: "white_wool_stairs", item: 1554, block: 1216, first_state: 31345, last_state: 31424, default_state: 31356, wire_item: 266, wire_block: 167, wire_first_state: 2425, input_name: "white_wool", input_item: 213 },
    SlabDescriptor { family: SlabFamily::Wool, form: BlockForm::Stairs, name: "orange_wool_stairs", item: 1555, block: 1217, first_state: 31425, last_state: 31504, default_state: 31436, wire_item: 267, wire_block: 168, wire_first_state: 2505, input_name: "orange_wool", input_item: 214 },
    SlabDescriptor { family: SlabFamily::Wool, form: BlockForm::Stairs, name: "magenta_wool_stairs", item: 1556, block: 1218, first_state: 31505, last_state: 31584, default_state: 31516, wire_item: 268, wire_block: 169, wire_first_state: 2585, input_name: "magenta_wool", input_item: 215 },
    SlabDescriptor { family: SlabFamily::Wool, form: BlockForm::Stairs, name: "light_blue_wool_stairs", item: 1557, block: 1219, first_state: 31585, last_state: 31664, default_state: 31596, wire_item: 269, wire_block: 170, wire_first_state: 2665, input_name: "light_blue_wool", input_item: 216 },
    SlabDescriptor { family: SlabFamily::Wool, form: BlockForm::Stairs, name: "yellow_wool_stairs", item: 1558, block: 1220, first_state: 31665, last_state: 31744, default_state: 31676, wire_item: 270, wire_block: 171, wire_first_state: 2745, input_name: "yellow_wool", input_item: 217 },
    SlabDescriptor { family: SlabFamily::Wool, form: BlockForm::Stairs, name: "lime_wool_stairs", item: 1559, block: 1221, first_state: 31745, last_state: 31824, default_state: 31756, wire_item: 271, wire_block: 172, wire_first_state: 2825, input_name: "lime_wool", input_item: 218 },
    SlabDescriptor { family: SlabFamily::Wool, form: BlockForm::Stairs, name: "pink_wool_stairs", item: 1560, block: 1222, first_state: 31825, last_state: 31904, default_state: 31836, wire_item: 272, wire_block: 173, wire_first_state: 2905, input_name: "pink_wool", input_item: 219 },
    SlabDescriptor { family: SlabFamily::Wool, form: BlockForm::Stairs, name: "gray_wool_stairs", item: 1561, block: 1223, first_state: 31905, last_state: 31984, default_state: 31916, wire_item: 273, wire_block: 174, wire_first_state: 2985, input_name: "gray_wool", input_item: 220 },
    SlabDescriptor { family: SlabFamily::Wool, form: BlockForm::Stairs, name: "light_gray_wool_stairs", item: 1562, block: 1224, first_state: 31985, last_state: 32064, default_state: 31996, wire_item: 274, wire_block: 175, wire_first_state: 3065, input_name: "light_gray_wool", input_item: 221 },
    SlabDescriptor { family: SlabFamily::Wool, form: BlockForm::Stairs, name: "cyan_wool_stairs", item: 1563, block: 1225, first_state: 32065, last_state: 32144, default_state: 32076, wire_item: 275, wire_block: 176, wire_first_state: 3145, input_name: "cyan_wool", input_item: 222 },
    SlabDescriptor { family: SlabFamily::Wool, form: BlockForm::Stairs, name: "purple_wool_stairs", item: 1564, block: 1226, first_state: 32145, last_state: 32224, default_state: 32156, wire_item: 276, wire_block: 177, wire_first_state: 3225, input_name: "purple_wool", input_item: 223 },
    SlabDescriptor { family: SlabFamily::Wool, form: BlockForm::Stairs, name: "blue_wool_stairs", item: 1565, block: 1227, first_state: 32225, last_state: 32304, default_state: 32236, wire_item: 277, wire_block: 178, wire_first_state: 3305, input_name: "blue_wool", input_item: 224 },
    SlabDescriptor { family: SlabFamily::Wool, form: BlockForm::Stairs, name: "brown_wool_stairs", item: 1566, block: 1228, first_state: 32305, last_state: 32384, default_state: 32316, wire_item: 278, wire_block: 179, wire_first_state: 3385, input_name: "brown_wool", input_item: 225 },
    SlabDescriptor { family: SlabFamily::Wool, form: BlockForm::Stairs, name: "green_wool_stairs", item: 1567, block: 1229, first_state: 32385, last_state: 32464, default_state: 32396, wire_item: 279, wire_block: 180, wire_first_state: 3465, input_name: "green_wool", input_item: 226 },
    SlabDescriptor { family: SlabFamily::Wool, form: BlockForm::Stairs, name: "red_wool_stairs", item: 1568, block: 1230, first_state: 32465, last_state: 32544, default_state: 32476, wire_item: 280, wire_block: 181, wire_first_state: 3545, input_name: "red_wool", input_item: 227 },
    SlabDescriptor { family: SlabFamily::Wool, form: BlockForm::Stairs, name: "black_wool_stairs", item: 1569, block: 1231, first_state: 32545, last_state: 32624, default_state: 32556, wire_item: 281, wire_block: 182, wire_first_state: 3625, input_name: "black_wool", input_item: 228 },
];

/// Сера и киноварь 26.2: 28 объектов, дописаны после ступеней (предметы с 1570, блоки с 1232, состояния с 32625).
/// Номера 777 и состояния — из pinned blocks.json/items.json; `input_*` — ингредиент ручного рецепта.
pub static MINERALS: [SlabDescriptor; 28] = [
    SlabDescriptor { family: SlabFamily::Mineral, form: BlockForm::Full, name: "sulfur", item: 1570, block: 1232, first_state: 32625, last_state: 32625, default_state: 32625, wire_item: 26, wire_block: 1088, wire_first_state: 28044, input_name: "sulfur_spike", input_item: 1584 },
    SlabDescriptor { family: SlabFamily::Mineral, form: BlockForm::Full, name: "polished_sulfur", item: 1571, block: 1233, first_state: 32626, last_state: 32626, default_state: 32626, wire_item: 31, wire_block: 1093, wire_first_state: 28460, input_name: "sulfur", input_item: 1570 },
    SlabDescriptor { family: SlabFamily::Mineral, form: BlockForm::Full, name: "chiseled_sulfur", item: 1572, block: 1234, first_state: 32627, last_state: 32627, default_state: 32627, wire_item: 39, wire_block: 1101, wire_first_state: 29282, input_name: "sulfur_slab", input_item: 1574 },
    SlabDescriptor { family: SlabFamily::Mineral, form: BlockForm::Full, name: "sulfur_bricks", item: 1573, block: 1235, first_state: 32628, last_state: 32628, default_state: 32628, wire_item: 35, wire_block: 1097, wire_first_state: 28871, input_name: "polished_sulfur", input_item: 1571 },
    SlabDescriptor { family: SlabFamily::Mineral, form: BlockForm::Slab, name: "sulfur_slab", item: 1574, block: 1236, first_state: 32629, last_state: 32634, default_state: 32632, wire_item: 28, wire_block: 1090, wire_first_state: 28050, input_name: "sulfur", input_item: 1570 },
    SlabDescriptor { family: SlabFamily::Mineral, form: BlockForm::Stairs, name: "sulfur_stairs", item: 1575, block: 1237, first_state: 32635, last_state: 32714, default_state: 32646, wire_item: 29, wire_block: 1091, wire_first_state: 28056, input_name: "sulfur", input_item: 1570 },
    SlabDescriptor { family: SlabFamily::Mineral, form: BlockForm::Wall, name: "sulfur_wall", item: 1576, block: 1238, first_state: 32715, last_state: 33038, default_state: 32718, wire_item: 30, wire_block: 1092, wire_first_state: 28136, input_name: "sulfur", input_item: 1570 },
    SlabDescriptor { family: SlabFamily::Mineral, form: BlockForm::Slab, name: "polished_sulfur_slab", item: 1577, block: 1239, first_state: 33039, last_state: 33044, default_state: 33042, wire_item: 32, wire_block: 1094, wire_first_state: 28461, input_name: "polished_sulfur", input_item: 1571 },
    SlabDescriptor { family: SlabFamily::Mineral, form: BlockForm::Stairs, name: "polished_sulfur_stairs", item: 1578, block: 1240, first_state: 33045, last_state: 33124, default_state: 33056, wire_item: 33, wire_block: 1095, wire_first_state: 28467, input_name: "polished_sulfur", input_item: 1571 },
    SlabDescriptor { family: SlabFamily::Mineral, form: BlockForm::Wall, name: "polished_sulfur_wall", item: 1579, block: 1241, first_state: 33125, last_state: 33448, default_state: 33128, wire_item: 34, wire_block: 1096, wire_first_state: 28547, input_name: "polished_sulfur", input_item: 1571 },
    SlabDescriptor { family: SlabFamily::Mineral, form: BlockForm::Slab, name: "sulfur_brick_slab", item: 1580, block: 1242, first_state: 33449, last_state: 33454, default_state: 33452, wire_item: 36, wire_block: 1098, wire_first_state: 28872, input_name: "sulfur_bricks", input_item: 1573 },
    SlabDescriptor { family: SlabFamily::Mineral, form: BlockForm::Stairs, name: "sulfur_brick_stairs", item: 1581, block: 1243, first_state: 33455, last_state: 33534, default_state: 33466, wire_item: 37, wire_block: 1099, wire_first_state: 28878, input_name: "sulfur_bricks", input_item: 1573 },
    SlabDescriptor { family: SlabFamily::Mineral, form: BlockForm::Wall, name: "sulfur_brick_wall", item: 1582, block: 1244, first_state: 33535, last_state: 33858, default_state: 33538, wire_item: 38, wire_block: 1100, wire_first_state: 28958, input_name: "sulfur_bricks", input_item: 1573 },
    SlabDescriptor { family: SlabFamily::Mineral, form: BlockForm::PotentSulfur, name: "potent_sulfur", item: 1583, block: 1245, first_state: 33859, last_state: 33863, default_state: 33859, wire_item: 27, wire_block: 1089, wire_first_state: 28045, input_name: "sulfur", input_item: 1570 },
    SlabDescriptor { family: SlabFamily::Mineral, form: BlockForm::Spike, name: "sulfur_spike", item: 1584, block: 1246, first_state: 33864, last_state: 33883, default_state: 33869, wire_item: 1572, wire_block: 1224, wire_first_state: 33586, input_name: "", input_item: 0 },
    SlabDescriptor { family: SlabFamily::Mineral, form: BlockForm::Full, name: "cinnabar", item: 1585, block: 1247, first_state: 33884, last_state: 33884, default_state: 33884, wire_item: 40, wire_block: 1102, wire_first_state: 29283, input_name: "", input_item: 0 },
    SlabDescriptor { family: SlabFamily::Mineral, form: BlockForm::Full, name: "polished_cinnabar", item: 1586, block: 1248, first_state: 33885, last_state: 33885, default_state: 33885, wire_item: 44, wire_block: 1106, wire_first_state: 29694, input_name: "cinnabar", input_item: 1585 },
    SlabDescriptor { family: SlabFamily::Mineral, form: BlockForm::Full, name: "chiseled_cinnabar", item: 1587, block: 1249, first_state: 33886, last_state: 33886, default_state: 33886, wire_item: 52, wire_block: 1114, wire_first_state: 30516, input_name: "cinnabar_slab", input_item: 1589 },
    SlabDescriptor { family: SlabFamily::Mineral, form: BlockForm::Full, name: "cinnabar_bricks", item: 1588, block: 1250, first_state: 33887, last_state: 33887, default_state: 33887, wire_item: 48, wire_block: 1110, wire_first_state: 30105, input_name: "polished_cinnabar", input_item: 1586 },
    SlabDescriptor { family: SlabFamily::Mineral, form: BlockForm::Slab, name: "cinnabar_slab", item: 1589, block: 1251, first_state: 33888, last_state: 33893, default_state: 33891, wire_item: 41, wire_block: 1103, wire_first_state: 29284, input_name: "cinnabar", input_item: 1585 },
    SlabDescriptor { family: SlabFamily::Mineral, form: BlockForm::Stairs, name: "cinnabar_stairs", item: 1590, block: 1252, first_state: 33894, last_state: 33973, default_state: 33905, wire_item: 42, wire_block: 1104, wire_first_state: 29290, input_name: "cinnabar", input_item: 1585 },
    SlabDescriptor { family: SlabFamily::Mineral, form: BlockForm::Wall, name: "cinnabar_wall", item: 1591, block: 1253, first_state: 33974, last_state: 34297, default_state: 33977, wire_item: 43, wire_block: 1105, wire_first_state: 29370, input_name: "cinnabar", input_item: 1585 },
    SlabDescriptor { family: SlabFamily::Mineral, form: BlockForm::Slab, name: "polished_cinnabar_slab", item: 1592, block: 1254, first_state: 34298, last_state: 34303, default_state: 34301, wire_item: 45, wire_block: 1107, wire_first_state: 29695, input_name: "polished_cinnabar", input_item: 1586 },
    SlabDescriptor { family: SlabFamily::Mineral, form: BlockForm::Stairs, name: "polished_cinnabar_stairs", item: 1593, block: 1255, first_state: 34304, last_state: 34383, default_state: 34315, wire_item: 46, wire_block: 1108, wire_first_state: 29701, input_name: "polished_cinnabar", input_item: 1586 },
    SlabDescriptor { family: SlabFamily::Mineral, form: BlockForm::Wall, name: "polished_cinnabar_wall", item: 1594, block: 1256, first_state: 34384, last_state: 34707, default_state: 34387, wire_item: 47, wire_block: 1109, wire_first_state: 29781, input_name: "polished_cinnabar", input_item: 1586 },
    SlabDescriptor { family: SlabFamily::Mineral, form: BlockForm::Slab, name: "cinnabar_brick_slab", item: 1595, block: 1257, first_state: 34708, last_state: 34713, default_state: 34711, wire_item: 49, wire_block: 1111, wire_first_state: 30106, input_name: "cinnabar_bricks", input_item: 1588 },
    SlabDescriptor { family: SlabFamily::Mineral, form: BlockForm::Stairs, name: "cinnabar_brick_stairs", item: 1596, block: 1258, first_state: 34714, last_state: 34793, default_state: 34725, wire_item: 50, wire_block: 1112, wire_first_state: 30112, input_name: "cinnabar_bricks", input_item: 1588 },
    SlabDescriptor { family: SlabFamily::Mineral, form: BlockForm::Wall, name: "cinnabar_brick_wall", item: 1597, block: 1259, first_state: 34794, last_state: 35117, default_state: 34797, wire_item: 51, wire_block: 1113, wire_first_state: 30192, input_name: "cinnabar_bricks", input_item: 1588 },
];

/// Тополь 26.3: 23 блока (предметы с 1598, блоки с 1260, состояния с 35118). Номера 777 — из pinned
/// blocks.json/items.json. `input_name`/`input_item` — дубовый двойник (те же свойства и порядок
/// состояний, поэтому таблицы твёрдости, света и формы берутся по смещению от него); `item: -1` —
/// у блока нет предмета (настенные таблички, горшок с саженцем). Лодки — предметы без блока (`BlockForm::Boat`); их сущности — `entity_ids`.
pub static POPLAR: [SlabDescriptor; 63] = [
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Wood, name: "poplar_planks", item: 1598, block: 1260, first_state: 35118, last_state: 35118, default_state: 35118, wire_item: 72, wire_block: 23, wire_first_state: 27, input_name: "oak_planks", input_item: 36 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Stairs, name: "poplar_stairs", item: 1599, block: 1261, first_state: 35119, last_state: 35198, default_state: 35130, wire_item: 524, wire_block: 574, wire_first_state: 14139, input_name: "oak_stairs", input_item: 442 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Slab, name: "poplar_slab", item: 1600, block: 1262, first_state: 35199, last_state: 35204, default_state: 35202, wire_item: 350, wire_block: 662, wire_first_state: 15231, input_name: "oak_slab", input_item: 271 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Wood, name: "poplar_fence", item: 1601, block: 1263, first_state: 35205, last_state: 35236, default_state: 35236, wire_item: 426, wire_block: 700, wire_first_state: 15907, input_name: "oak_fence", input_item: 345 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Wood, name: "poplar_fence_gate", item: 1602, block: 1264, first_state: 35237, last_state: 35268, default_state: 35244, wire_item: 941, wire_block: 690, wire_first_state: 15587, input_name: "oak_fence_gate", input_item: 822 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Wood, name: "poplar_door", item: 1603, block: 1265, first_state: 35269, last_state: 35332, default_state: 35280, wire_item: 898, wire_block: 710, wire_first_state: 16483, input_name: "oak_door", input_item: 781 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Wood, name: "poplar_trapdoor", item: 1604, block: 1266, first_state: 35333, last_state: 35396, default_state: 35348, wire_item: 920, wire_block: 374, wire_first_state: 9360, input_name: "oak_trapdoor", input_item: 802 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Wood, name: "poplar_button", item: 1605, block: 1267, first_state: 35397, last_state: 35420, default_state: 35406, wire_item: 867, wire_block: 504, wire_first_state: 12634, input_name: "oak_button", input_item: 752 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Wood, name: "poplar_pressure_plate", item: 1606, block: 1268, first_state: 35421, last_state: 35422, default_state: 35422, wire_item: 884, wire_block: 318, wire_first_state: 8547, input_name: "oak_pressure_plate", input_item: 768 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Wood, name: "poplar_sign", item: 1607, block: 1269, first_state: 35423, last_state: 35454, default_state: 35440, wire_item: 1111, wire_block: 263, wire_first_state: 7179, input_name: "oak_sign", input_item: 989 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Wood, name: "poplar_wall_sign", item: -1, block: 1270, first_state: 35455, last_state: 35462, default_state: 35456, wire_item: 0, wire_block: 278, wire_first_state: 7487, input_name: "oak_wall_sign", input_item: 0 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Wood, name: "poplar_hanging_sign", item: 1608, block: 1271, first_state: 35463, last_state: 35526, default_state: 35512, wire_item: 1124, wire_block: 291, wire_first_state: 8207, input_name: "oak_hanging_sign", input_item: 1001 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Wood, name: "poplar_wall_hanging_sign", item: -1, block: 1272, first_state: 35527, last_state: 35534, default_state: 35528, wire_item: 0, wire_block: 302, wire_first_state: 8407, input_name: "oak_wall_hanging_sign", input_item: 0 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Wood, name: "poplar_shelf", item: 1609, block: 1273, first_state: 35535, last_state: 35598, default_state: 35544, wire_item: 385, wire_block: 233, wire_first_state: 4732, input_name: "oak_shelf", input_item: 314 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Wood, name: "poplar_log", item: 1610, block: 1274, first_state: 35599, last_state: 35601, default_state: 35600, wire_item: 172, wire_block: 60, wire_first_state: 166, input_name: "oak_log", input_item: 134 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Wood, name: "poplar_wood", item: 1611, block: 1275, first_state: 35602, last_state: 35604, default_state: 35603, wire_item: 212, wire_block: 83, wire_first_state: 234, input_name: "oak_wood", input_item: 171 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Wood, name: "stripped_poplar_log", item: 1612, block: 1276, first_state: 35605, last_state: 35607, default_state: 35606, wire_item: 187, wire_block: 73, wire_first_state: 204, input_name: "stripped_oak_log", input_item: 148 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Wood, name: "stripped_poplar_wood", item: 1613, block: 1277, first_state: 35608, last_state: 35610, default_state: 35609, wire_item: 199, wire_block: 93, wire_first_state: 264, input_name: "stripped_oak_wood", input_item: 159 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Wood, name: "poplar_sapling", item: 1614, block: 1278, first_state: 35611, last_state: 35612, default_state: 35611, wire_item: 86, wire_block: 35, wire_first_state: 86, input_name: "oak_sapling", input_item: 49 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Wood, name: "potted_poplar_sapling", item: -1, block: 1279, first_state: 35613, last_state: 35613, default_state: 35613, wire_item: 0, wire_block: 472, wire_first_state: 12381, input_name: "potted_oak_sapling", input_item: 0 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Wood, name: "orange_poplar_leaves", item: 1615, block: 1280, first_state: 35614, last_state: 35641, default_state: 35641, wire_item: 225, wire_block: 104, wire_first_state: 547, input_name: "oak_leaves", input_item: 182 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Wood, name: "red_poplar_leaves", item: 1616, block: 1281, first_state: 35642, last_state: 35669, default_state: 35669, wire_item: 224, wire_block: 103, wire_first_state: 519, input_name: "oak_leaves", input_item: 182 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Wood, name: "yellow_poplar_leaves", item: 1617, block: 1282, first_state: 35670, last_state: 35697, default_state: 35697, wire_item: 226, wire_block: 105, wire_first_state: 575, input_name: "oak_leaves", input_item: 182 },
    // Растения 26.3 без дубового двойника: таблицы состояний берутся у красного гриба (одно состояние).
    // Состояния полочного гриба: age (0, 1) × facing (north, south, west, east); по умолчанию age=0, north.
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Plant, name: "shelf_mushroom", item: 1618, block: 1283, first_state: 35698, last_state: 35705, default_state: 35698, wire_item: 319, wire_block: 447, wire_first_state: 11227, input_name: "red_mushroom", input_item: 0 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Plant, name: "red_shrub", item: 1619, block: 1284, first_state: 35706, last_state: 35706, default_state: 35706, wire_item: 241, wire_block: 144, wire_first_state: 2367, input_name: "red_mushroom", input_item: 0 },
    // Лодки тополя: предметы без блока (block -1, пустой диапазон состояний).
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Boat, name: "poplar_boat", item: 1620, block: -1, first_state: 35707, last_state: 35706, default_state: -1, wire_item: 993, wire_block: 0, wire_first_state: 0, input_name: "poplar_planks", input_item: 1598 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Boat, name: "poplar_chest_boat", item: 1621, block: -1, first_state: 35707, last_state: 35706, default_state: -1, wire_item: 994, wire_block: 0, wire_first_state: 0, input_name: "poplar_boat", input_item: 1620 },
    // Мебель 26.3: соломенная кровать (двойник white_bed: те же свойства и порядок состояний), подушки,
    // карты строений и пластинка Bounce. Предметы без блока — `BlockForm::Item` (пустой диапазон состояний).
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Wood, name: "straw_bed", item: 1622, block: 1285, first_state: 35707, last_state: 35722, default_state: 35710, wire_item: 1235, wire_block: 135, wire_first_state: 2286, input_name: "white_bed", input_item: 1087 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Item, name: "white_cushion", item: 1623, block: -1, first_state: 35723, last_state: 35722, default_state: -1, wire_item: 1170, wire_block: 0, wire_first_state: 0, input_name: "white_wool_slab", input_item: 0 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Item, name: "orange_cushion", item: 1624, block: -1, first_state: 35723, last_state: 35722, default_state: -1, wire_item: 1171, wire_block: 0, wire_first_state: 0, input_name: "orange_wool_slab", input_item: 0 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Item, name: "magenta_cushion", item: 1625, block: -1, first_state: 35723, last_state: 35722, default_state: -1, wire_item: 1172, wire_block: 0, wire_first_state: 0, input_name: "magenta_wool_slab", input_item: 0 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Item, name: "light_blue_cushion", item: 1626, block: -1, first_state: 35723, last_state: 35722, default_state: -1, wire_item: 1173, wire_block: 0, wire_first_state: 0, input_name: "light_blue_wool_slab", input_item: 0 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Item, name: "yellow_cushion", item: 1627, block: -1, first_state: 35723, last_state: 35722, default_state: -1, wire_item: 1174, wire_block: 0, wire_first_state: 0, input_name: "yellow_wool_slab", input_item: 0 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Item, name: "lime_cushion", item: 1628, block: -1, first_state: 35723, last_state: 35722, default_state: -1, wire_item: 1175, wire_block: 0, wire_first_state: 0, input_name: "lime_wool_slab", input_item: 0 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Item, name: "pink_cushion", item: 1629, block: -1, first_state: 35723, last_state: 35722, default_state: -1, wire_item: 1176, wire_block: 0, wire_first_state: 0, input_name: "pink_wool_slab", input_item: 0 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Item, name: "gray_cushion", item: 1630, block: -1, first_state: 35723, last_state: 35722, default_state: -1, wire_item: 1177, wire_block: 0, wire_first_state: 0, input_name: "gray_wool_slab", input_item: 0 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Item, name: "light_gray_cushion", item: 1631, block: -1, first_state: 35723, last_state: 35722, default_state: -1, wire_item: 1178, wire_block: 0, wire_first_state: 0, input_name: "light_gray_wool_slab", input_item: 0 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Item, name: "cyan_cushion", item: 1632, block: -1, first_state: 35723, last_state: 35722, default_state: -1, wire_item: 1179, wire_block: 0, wire_first_state: 0, input_name: "cyan_wool_slab", input_item: 0 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Item, name: "purple_cushion", item: 1633, block: -1, first_state: 35723, last_state: 35722, default_state: -1, wire_item: 1180, wire_block: 0, wire_first_state: 0, input_name: "purple_wool_slab", input_item: 0 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Item, name: "blue_cushion", item: 1634, block: -1, first_state: 35723, last_state: 35722, default_state: -1, wire_item: 1181, wire_block: 0, wire_first_state: 0, input_name: "blue_wool_slab", input_item: 0 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Item, name: "brown_cushion", item: 1635, block: -1, first_state: 35723, last_state: 35722, default_state: -1, wire_item: 1182, wire_block: 0, wire_first_state: 0, input_name: "brown_wool_slab", input_item: 0 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Item, name: "green_cushion", item: 1636, block: -1, first_state: 35723, last_state: 35722, default_state: -1, wire_item: 1183, wire_block: 0, wire_first_state: 0, input_name: "green_wool_slab", input_item: 0 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Item, name: "red_cushion", item: 1637, block: -1, first_state: 35723, last_state: 35722, default_state: -1, wire_item: 1184, wire_block: 0, wire_first_state: 0, input_name: "red_wool_slab", input_item: 0 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Item, name: "black_cushion", item: 1638, block: -1, first_state: 35723, last_state: 35722, default_state: -1, wire_item: 1185, wire_block: 0, wire_first_state: 0, input_name: "black_wool_slab", input_item: 0 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Item, name: "ocean_monument_map", item: 1639, block: -1, first_state: 35723, last_state: 35722, default_state: -1, wire_item: 1239, wire_block: 0, wire_first_state: 0, input_name: "filled_map", input_item: 0 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Item, name: "woodland_mansion_map", item: 1640, block: -1, first_state: 35723, last_state: 35722, default_state: -1, wire_item: 1240, wire_block: 0, wire_first_state: 0, input_name: "filled_map", input_item: 0 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Item, name: "buried_trial_chambers_map", item: 1641, block: -1, first_state: 35723, last_state: 35722, default_state: -1, wire_item: 1241, wire_block: 0, wire_first_state: 0, input_name: "filled_map", input_item: 0 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Item, name: "jungle_pyramid_map", item: 1642, block: -1, first_state: 35723, last_state: 35722, default_state: -1, wire_item: 1242, wire_block: 0, wire_first_state: 0, input_name: "filled_map", input_item: 0 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Item, name: "swamp_hut_map", item: 1643, block: -1, first_state: 35723, last_state: 35722, default_state: -1, wire_item: 1243, wire_block: 0, wire_first_state: 0, input_name: "filled_map", input_item: 0 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Item, name: "desert_village_map", item: 1644, block: -1, first_state: 35723, last_state: 35722, default_state: -1, wire_item: 1244, wire_block: 0, wire_first_state: 0, input_name: "filled_map", input_item: 0 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Item, name: "plains_village_map", item: 1645, block: -1, first_state: 35723, last_state: 35722, default_state: -1, wire_item: 1245, wire_block: 0, wire_first_state: 0, input_name: "filled_map", input_item: 0 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Item, name: "savanna_village_map", item: 1646, block: -1, first_state: 35723, last_state: 35722, default_state: -1, wire_item: 1246, wire_block: 0, wire_first_state: 0, input_name: "filled_map", input_item: 0 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Item, name: "snowy_village_map", item: 1647, block: -1, first_state: 35723, last_state: 35722, default_state: -1, wire_item: 1247, wire_block: 0, wire_first_state: 0, input_name: "filled_map", input_item: 0 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Item, name: "taiga_village_map", item: 1648, block: -1, first_state: 35723, last_state: 35722, default_state: -1, wire_item: 1248, wire_block: 0, wire_first_state: 0, input_name: "filled_map", input_item: 0 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Item, name: "buried_treasure_map", item: 1649, block: -1, first_state: 35723, last_state: 35722, default_state: -1, wire_item: 1249, wire_block: 0, wire_first_state: 0, input_name: "filled_map", input_item: 0 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Item, name: "buried_ancient_city_map", item: 1650, block: -1, first_state: 35723, last_state: 35722, default_state: -1, wire_item: 1250, wire_block: 0, wire_first_state: 0, input_name: "filled_map", input_item: 0 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Item, name: "buried_mineshaft_map", item: 1651, block: -1, first_state: 35723, last_state: 35722, default_state: -1, wire_item: 1251, wire_block: 0, wire_first_state: 0, input_name: "filled_map", input_item: 0 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Item, name: "desert_pyramid_map", item: 1652, block: -1, first_state: 35723, last_state: 35722, default_state: -1, wire_item: 1252, wire_block: 0, wire_first_state: 0, input_name: "filled_map", input_item: 0 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Item, name: "abandoned_camp_map", item: 1653, block: -1, first_state: 35723, last_state: 35722, default_state: -1, wire_item: 1253, wire_block: 0, wire_first_state: 0, input_name: "filled_map", input_item: 0 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Item, name: "warm_ocean_ruins_map", item: 1654, block: -1, first_state: 35723, last_state: 35722, default_state: -1, wire_item: 1254, wire_block: 0, wire_first_state: 0, input_name: "filled_map", input_item: 0 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Item, name: "music_disc_bounce", item: 1655, block: -1, first_state: 35723, last_state: 35722, default_state: -1, wire_item: 1463, wire_block: 0, wire_first_state: 0, input_name: "music_disc_pigstep", input_item: 0 },
    // Куб серы 26.3: ведро и яйцо призыва (предметы без блока). Сущность куба привязана к яйцу (`entity_ids_of`).
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Item, name: "sulfur_cube_bucket", item: 1656, block: -1, first_state: 35723, last_state: 35722, default_state: -1, wire_item: 1140, wire_block: 0, wire_first_state: 0, input_name: "bucket", input_item: 0 },
    SlabDescriptor { family: SlabFamily::Poplar, form: BlockForm::Item, name: "sulfur_cube_spawn_egg", item: 1657, block: -1, first_state: 35723, last_state: 35722, default_state: -1, wire_item: 1316, wire_block: 0, wire_first_state: 0, input_name: "slime_spawn_egg", input_item: 0 },
];

/// Сущности лодок тополя: (внутренний номер после 775, номер в 777 по pinned entities.json).
/// Внутренние номера идут сразу за последним видом сущности 775 (156).
pub const fn entity_ids_of(name: &str) -> Option<(u32, u32)> {
    match name.as_bytes() {
        b"poplar_boat" => Some((157, 106)),
        b"poplar_chest_boat" => Some((158, 107)),
        // Сущность подушки (цвет — в metadata): привязана к строке белой подушки.
        b"white_cushion" => Some((159, 33)),
        // Куб серы (раскладка metadata — своя, индексы 26.3); привязан к строке яйца призыва.
        b"sulfur_cube_spawn_egg" => Some((160, 133)),
        _ => None,
    }
}

fn entity_ids(row: &SlabDescriptor) -> Option<(u32, u32)> { entity_ids_of(row.name) }

impl SlabDescriptor {
    /// Есть ли у строки предмет (у настенных табличек и горшка его нет).
    pub const fn has_item(&self) -> bool { self.item >= 0 }
    /// Есть ли у строки блок (у лодок его нет).
    pub const fn has_block(&self) -> bool { self.block >= 0 }
    /// Сколько состояний у блока: 6 у плиты, 80 у ступеней.
    pub const fn state_count(&self) -> u32 { (self.last_state - self.first_state + 1) as u32 }
}

/// Дописываемый порядок canonical-каталога: старые бетонные ID не меняются.
/// Только плиты: прежние потребители (рецепты 3→6, камнерез) их и ждут.
pub fn slabs() -> impl Iterator<Item = &'static SlabDescriptor> {
    CONCRETE_SLABS.iter().chain(WOOL_SLABS.iter())
}

/// Только ступени.
pub fn stairs() -> impl Iterator<Item = &'static SlabDescriptor> {
    CONCRETE_STAIRS.iter().chain(WOOL_STAIRS.iter())
}

/// Сера и киноварь (плиты, ступени, стенки, цельные блоки, мощная сера, шип).
pub fn minerals() -> impl Iterator<Item = &'static SlabDescriptor> {
    MINERALS.iter()
}

/// Все объекты каталога: плиты, ступени, сера и киноварь, затем тополь.
pub fn entries() -> impl Iterator<Item = &'static SlabDescriptor> {
    slabs().chain(stairs()).chain(minerals()).chain(poplar())
}

/// Тополь (дерево, листва, саженец, таблички, полка).
pub fn poplar() -> impl Iterator<Item = &'static SlabDescriptor> {
    POPLAR.iter()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdSpace { Canonical, Wire(JavaVersion) }

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ContentCodec { white_slab: bool, native_view: bool }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Id(ids::IdTranslationError),
    Unrepresentable { kind: RegistryKind, id: u32, target: IdSpace },
}

impl ContentCodec {
    pub const fn native_view(mut self) -> Self { self.native_view = self.white_slab; self }
    pub const fn is_native_view(self) -> bool { self.native_view }
    pub(crate) fn output_space(self, version: JavaVersion) -> IdSpace {
        if self.native_view && version == JavaVersion::V26_1_2 { IdSpace::Wire(version) } else { self.packet_space(version) }
    }
    pub const fn is_preview(self) -> bool { self.white_slab }
    /// Лодка тополя во внутреннем пространстве: раскладка metadata — как у дубовой лодки
    /// (те же 14 полей, [проверить] сверкой с 777). None — это не сущность расширения.
    pub fn entity_layout_base(self, id: u32) -> Option<&'static str> {
        if !self.white_slab { return None; }
        match id { 157 => Some("oak_boat"), 158 => Some("oak_chest_boat"), 159 => Some("cushion"), 160 => Some("sulfur_cube"), _ => None }
    }
    /// Только граница внутреннего packet builder: раскладка775 содержит
    /// canonical content. К настоящему входному Slot775 это неприменимо.
    pub(crate) fn packet_space(self, version: JavaVersion) -> IdSpace {
        if self.white_slab && version == JavaVersion::V26_1_2 { IdSpace::Canonical }
        else { IdSpace::Wire(version) }
    }
    pub(crate) fn packet_id(self, kind: RegistryKind, from: JavaVersion, to: JavaVersion, id: u32) -> Result<u32, Error> {
        self.map(kind,self.packet_space(from),self.output_space(to),id)
    }

    /// Только вызывающий серверный каталог разрешает этот private preview.
    /// Версия подключившегося клиента сама по себе его не выбирает.
    pub const fn white_slab_preview() -> Self { Self { white_slab: true, native_view: false } }

    fn extension(kind: RegistryKind, space: IdSpace, id: u32) -> Option<(u32, u32, u32)> {
        entries().find_map(|row| {
            let (first, last, wire) = match kind {
                RegistryKind::Item if !row.has_item() => return None,
                RegistryKind::Item => (row.item as u32, row.item as u32, row.wire_item),
                RegistryKind::Block if !row.has_block() => return None,
                RegistryKind::Block => (row.block as u32, row.block as u32, row.wire_block),
                RegistryKind::BlockState if row.state_count() == 0 => return None,
                RegistryKind::BlockState => (row.first_state as u32, row.last_state as u32, row.wire_first_state),
                RegistryKind::Entity => { let (canonical, wire) = entity_ids(row)?; (canonical, canonical, wire) }
                _ => return None,
            };
            match space {
                IdSpace::Canonical if (first..=last).contains(&id) => Some((id, first, wire)),
                IdSpace::Wire(JavaVersion::V26_3) if (wire..=wire + last - first).contains(&id) => Some((first + id - wire, first, wire)),
                _ => None,
            }
        })
    }

    pub fn map(self, kind: RegistryKind, from: IdSpace, to: IdSpace, id: u32) -> Result<u32, Error> {
        // Оба Wire обозначают настоящие протокольные пространства. Даже при
        // preview Canonical1506 не становится допустимым Wire775 item1506.
        if self.white_slab && let Some((canonical, first, wire)) = Self::extension(kind, from, id) {
                return match to {
                    IdSpace::Canonical => Ok(canonical),
                    IdSpace::Wire(JavaVersion::V26_3) => {
                        Ok(wire + canonical - first)
                    }
                    IdSpace::Wire(version @ (JavaVersion::V1_21_11 | JavaVersion::V26_1_2 | JavaVersion::V26_2)) => {
                        // Общий объект26.2 сохраняет настоящий вид; новый объект26.3
                        // получает явно заданного775-двойника с переводом его ID.
                        if version == JavaVersion::V26_2 {
                            match ids::translate(kind, JavaVersion::V26_3, version, wire + canonical - first) {
                                Ok(mapped) => return Ok(mapped),
                                Err(ids::IdTranslationError::MissingTarget(_)) => (),
                                Err(error) => return Err(Error::Id(error)),
                            }
                        }
                        let substitute = crate::content_views::substitute(kind, canonical)
                            .ok_or(Error::Unrepresentable { kind, id: canonical, target: to })?;
                        ids::translate(kind, JavaVersion::V26_1_2, version, substitute).map_err(Error::Id)
                    }
                    _ => Err(Error::Unrepresentable { kind, id: canonical, target: to }),
                };
        }
        let version = |space| match space { IdSpace::Canonical => JavaVersion::V26_1_2, IdSpace::Wire(version) => version };
        match ids::translate(kind, version(from), version(to), id) {
            Err(ids::IdTranslationError::MissingTarget(_)) if version(from) == JavaVersion::V26_1_2 && to == IdSpace::Wire(JavaVersion::V1_21_11) => {
                // Явные цветок/горшок: те же формы по blocks.json. [проверить]:
                // внешний вид настоящим клиентом774; игровые правила остаются у ядра.
                crate::generated_step1::substitute(kind,id).ok_or(Error::Unrepresentable { kind,id,target:to })
            }
            result => result.map_err(Error::Id),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extension_is_canonical_and_never_an_alias_for_wire775() {
        let codec = ContentCodec::white_slab_preview();
        for row in entries() {
            let kinds = [(RegistryKind::Item,row.item as u32,row.item as u32,row.wire_item),
                (RegistryKind::Block,row.block as u32,row.block as u32,row.wire_block),(RegistryKind::BlockState,row.first_state as u32,row.last_state as u32,row.wire_first_state)];
            for (kind, first, last, wire) in kinds.into_iter().filter(|(kind, ..)| match kind {
                RegistryKind::Item => row.has_item(),
                RegistryKind::Block => row.has_block(),
                _ => row.state_count() > 0,
            }) {
                for id in first..=last {
                    let target = wire + id - first;
                    assert_eq!(codec.map(kind,IdSpace::Canonical,IdSpace::Wire(JavaVersion::V26_3),id),Ok(target));
                    assert_eq!(codec.map(kind,IdSpace::Wire(JavaVersion::V26_3),IdSpace::Canonical,target),Ok(id));
                    assert!(ContentCodec::default().map(kind,IdSpace::Canonical,IdSpace::Wire(JavaVersion::V26_3),id).is_err());
                    assert!(codec.map(kind,IdSpace::Wire(JavaVersion::V26_1_2),IdSpace::Canonical,id).is_err());
                    assert!(codec.map(kind,IdSpace::Canonical,IdSpace::Wire(JavaVersion::V26_1_2),id).is_ok());
                    for version in [JavaVersion::V1_12_2] {
                        assert!(matches!(codec.map(kind,IdSpace::Canonical,IdSpace::Wire(version),id),Err(Error::Unrepresentable { .. })));
                    }
                }
            }
        }
    }
    #[test]
    fn base_ids_and_errors_stay_equal_to_existing_tables() {
        for codec in [ContentCodec::default(),ContentCodec::white_slab_preview()] {
            for kind in [RegistryKind::Item,RegistryKind::Block,RegistryKind::BlockState] {
                for id in [0,1,2,500,u32::MAX] {
                    assert_eq!(codec.map(kind,IdSpace::Canonical,IdSpace::Wire(JavaVersion::V26_3),id),
                        ids::translate(kind,JavaVersion::V26_1_2,JavaVersion::V26_3,id).map_err(Error::Id));
                }
            }
        }
    }
}
