// Как складывается мир: климат, биомы, рельеф.
//
// Устройство взято с вики (разбор — в tools/research/worldgen.md): шесть
// параметров климата, у каждого свои уровни, и биом выбирается по таблицам
// «уровень такой, уровень сякой — вот биом». Высота складывается сплайнами:
// континентальность задаёт основной уровень, эрозия — размах, гребни и
// долины — рельеф поверх.
//
// Шумы у нас свои, поэтому по семени мы с оригиналом не совпадаем и не
// стремимся: повторяем устройство, а не числа.
//
// Всё считается от семени мира и от координат: у чанка нет соседей, к которым
// надо обращаться, поэтому любой кусок мира можно сложить в любом порядке и
// в любое время — и он всегда получится одинаковым.

use crate::blocks;
use crate::world::AIR;
use crate::world::noise::{Noise, Random};

/// Уровень моря: до этой высоты низины залиты водой. Как в игре.
pub const SEA: i32 = 63;

/// Дно мира.
pub const BOTTOM: i32 = crate::world::MIN_Y;

/// Высота, с которой вниз идёт глубинный сланец вместо камня.
const DEEPSLATE_FROM: i32 = 0;

/// Ниже этой высоты пустоты пещер залиты лавой — как в игре.
const LAVA_LEVEL: i32 = -54;

/// Насколько глубоко под поверхностью начинаются пещеры: выше них земля
/// остаётся целой, иначе своды обрушивались бы прямо под ногами.
const CAVE_ROOF: i32 = 6;

/// Биомы, которые мы складываем. Имена — из реестра игры: клиент знает их
/// по своим файлам, и цвет травы, неба и воды берёт оттуда же.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Biome {
    // Море
    Ocean,
    DeepOcean,
    ColdOcean,
    DeepColdOcean,
    FrozenOcean,
    DeepFrozenOcean,
    LukewarmOcean,
    DeepLukewarmOcean,
    WarmOcean,
    MushroomFields,
    // Берег и вода на суше
    Beach,
    SnowyBeach,
    StonyShore,
    River,
    FrozenRiver,
    Swamp,
    MangroveSwamp,
    // Равнинная суша
    Plains,
    SunflowerPlains,
    SnowyPlains,
    IceSpikes,
    Forest,
    FlowerForest,
    BirchForest,
    OldGrowthBirchForest,
    DarkForest,
    Taiga,
    SnowyTaiga,
    OldGrowthPineTaiga,
    OldGrowthSpruceTaiga,
    Jungle,
    SparseJungle,
    BambooJungle,
    Savanna,
    Desert,
    // Высокая суша
    Meadow,
    CherryGrove,
    SavannaPlateau,
    Grove,
    SnowySlopes,
    JaggedPeaks,
    FrozenPeaks,
    StonyPeaks,
    WindsweptHills,
    WindsweptGravellyHills,
    WindsweptForest,
    WindsweptSavanna,
    Badlands,
    ErodedBadlands,
    WoodedBadlands,
    PaleGarden,
}

impl Biome {
    /// Все биомы по порядку: этот же порядок — их номера в реестре, который
    /// сервер шлёт клиенту.
    pub const ALL: [Biome; 51] = [
        Biome::Ocean,
        Biome::DeepOcean,
        Biome::ColdOcean,
        Biome::DeepColdOcean,
        Biome::FrozenOcean,
        Biome::DeepFrozenOcean,
        Biome::LukewarmOcean,
        Biome::DeepLukewarmOcean,
        Biome::WarmOcean,
        Biome::MushroomFields,
        Biome::Beach,
        Biome::SnowyBeach,
        Biome::StonyShore,
        Biome::River,
        Biome::FrozenRiver,
        Biome::Swamp,
        Biome::MangroveSwamp,
        Biome::Plains,
        Biome::SunflowerPlains,
        Biome::SnowyPlains,
        Biome::IceSpikes,
        Biome::Forest,
        Biome::FlowerForest,
        Biome::BirchForest,
        Biome::OldGrowthBirchForest,
        Biome::DarkForest,
        Biome::Taiga,
        Biome::SnowyTaiga,
        Biome::OldGrowthPineTaiga,
        Biome::OldGrowthSpruceTaiga,
        Biome::Jungle,
        Biome::SparseJungle,
        Biome::BambooJungle,
        Biome::Savanna,
        Biome::Desert,
        Biome::Meadow,
        Biome::CherryGrove,
        Biome::SavannaPlateau,
        Biome::Grove,
        Biome::SnowySlopes,
        Biome::JaggedPeaks,
        Biome::FrozenPeaks,
        Biome::StonyPeaks,
        Biome::WindsweptHills,
        Biome::WindsweptGravellyHills,
        Biome::WindsweptForest,
        Biome::WindsweptSavanna,
        Biome::Badlands,
        Biome::ErodedBadlands,
        Biome::WoodedBadlands,
        Biome::PaleGarden,
    ];

    /// Имя биома в реестре игры.
    pub fn name(self) -> &'static str {
        match self {
            Biome::Ocean => "ocean",
            Biome::DeepOcean => "deep_ocean",
            Biome::ColdOcean => "cold_ocean",
            Biome::DeepColdOcean => "deep_cold_ocean",
            Biome::FrozenOcean => "frozen_ocean",
            Biome::DeepFrozenOcean => "deep_frozen_ocean",
            Biome::LukewarmOcean => "lukewarm_ocean",
            Biome::DeepLukewarmOcean => "deep_lukewarm_ocean",
            Biome::WarmOcean => "warm_ocean",
            Biome::MushroomFields => "mushroom_fields",
            Biome::Beach => "beach",
            Biome::SnowyBeach => "snowy_beach",
            Biome::StonyShore => "stony_shore",
            Biome::River => "river",
            Biome::FrozenRiver => "frozen_river",
            Biome::Swamp => "swamp",
            Biome::MangroveSwamp => "mangrove_swamp",
            Biome::Plains => "plains",
            Biome::SunflowerPlains => "sunflower_plains",
            Biome::SnowyPlains => "snowy_plains",
            Biome::IceSpikes => "ice_spikes",
            Biome::Forest => "forest",
            Biome::FlowerForest => "flower_forest",
            Biome::BirchForest => "birch_forest",
            Biome::OldGrowthBirchForest => "old_growth_birch_forest",
            Biome::DarkForest => "dark_forest",
            Biome::Taiga => "taiga",
            Biome::SnowyTaiga => "snowy_taiga",
            Biome::OldGrowthPineTaiga => "old_growth_pine_taiga",
            Biome::OldGrowthSpruceTaiga => "old_growth_spruce_taiga",
            Biome::Jungle => "jungle",
            Biome::SparseJungle => "sparse_jungle",
            Biome::BambooJungle => "bamboo_jungle",
            Biome::Savanna => "savanna",
            Biome::Desert => "desert",
            Biome::Meadow => "meadow",
            Biome::CherryGrove => "cherry_grove",
            Biome::SavannaPlateau => "savanna_plateau",
            Biome::Grove => "grove",
            Biome::SnowySlopes => "snowy_slopes",
            Biome::JaggedPeaks => "jagged_peaks",
            Biome::FrozenPeaks => "frozen_peaks",
            Biome::StonyPeaks => "stony_peaks",
            Biome::WindsweptHills => "windswept_hills",
            Biome::WindsweptGravellyHills => "windswept_gravelly_hills",
            Biome::WindsweptForest => "windswept_forest",
            Biome::WindsweptSavanna => "windswept_savanna",
            Biome::Badlands => "badlands",
            Biome::ErodedBadlands => "eroded_badlands",
            Biome::WoodedBadlands => "wooded_badlands",
            Biome::PaleGarden => "pale_garden",
        }
    }

    /// Номер биома в реестре: место в списке `ALL`.
    pub fn number(self) -> i32 {
        Biome::ALL
            .iter()
            .position(|biome| *biome == self)
            .unwrap_or_else(|| panic!("биом {:?} забыт в списке ALL", self)) as i32
    }

    /// Основная температура и влажность биома — те же числа, что в игре
    /// (вики, «Biome» → List of biome climates). По ним клиент выбирает цвет
    /// травы и листвы, а из температуры считается цвет неба.
    pub fn climate(self) -> (f32, f32) {
        match self.name() {
        "ocean" => (0.5, 0.5),
        "deep_ocean" => (0.5, 0.5),
        "cold_ocean" => (0.5, 0.5),
        "deep_cold_ocean" => (0.5, 0.5),
        "frozen_ocean" => (0.0, 0.5),
        "deep_frozen_ocean" => (0.0, 0.5),
        "lukewarm_ocean" => (0.5, 0.5),
        "deep_lukewarm_ocean" => (0.5, 0.5),
        "warm_ocean" => (0.5, 0.5),
        "mushroom_fields" => (0.9, 1.0),
        "beach" => (0.8, 0.4),
        "snowy_beach" => (0.05, 0.3),
        "stony_shore" => (0.2, 0.3),
        "river" => (0.5, 0.5),
        "frozen_river" => (0.0, 0.5),
        "swamp" => (0.8, 0.9),
        "mangrove_swamp" => (0.8, 0.9),
        "plains" => (0.8, 0.4),
        "sunflower_plains" => (0.8, 0.4),
        "snowy_plains" => (0.0, 0.5),
        "ice_spikes" => (0.0, 0.5),
        "forest" => (0.7, 0.8),
        "flower_forest" => (0.7, 0.8),
        "birch_forest" => (0.6, 0.6),
        "old_growth_birch_forest" => (0.6, 0.6),
        "dark_forest" => (0.7, 0.8),
        "taiga" => (0.25, 0.8),
        "snowy_taiga" => (-0.5, 0.4),
        "old_growth_pine_taiga" => (0.3, 0.8),
        "old_growth_spruce_taiga" => (0.25, 0.8),
        "jungle" => (0.95, 0.9),
        "sparse_jungle" => (0.95, 0.8),
        "bamboo_jungle" => (0.95, 0.9),
        "savanna" => (2.0, 0.0),
        "desert" => (2.0, 0.0),
        "meadow" => (0.5, 0.8),
        "cherry_grove" => (0.5, 0.8),
        "savanna_plateau" => (2.0, 0.0),
        "grove" => (-0.2, 0.8),
        "snowy_slopes" => (-0.3, 0.9),
        "jagged_peaks" => (-0.7, 0.9),
        "frozen_peaks" => (-0.7, 0.9),
        "stony_peaks" => (1.0, 0.3),
        "windswept_hills" => (0.2, 0.3),
        "windswept_gravelly_hills" => (0.2, 0.3),
        "windswept_forest" => (0.2, 0.3),
        "windswept_savanna" => (2.0, 0.0),
        "badlands" => (2.0, 0.0),
        "eroded_badlands" => (2.0, 0.0),
        "wooded_badlands" => (2.0, 0.0),
        "pale_garden" => (0.7, 0.8),
            _ => (0.8, 0.4),
        }
    }

    /// Морской ли это биом: в нём вода стоит до уровня моря.
    pub fn is_ocean(self) -> bool {
        matches!(
            self,
            Biome::Ocean
                | Biome::DeepOcean
                | Biome::ColdOcean
                | Biome::DeepColdOcean
                | Biome::FrozenOcean
                | Biome::DeepFrozenOcean
                | Biome::LukewarmOcean
                | Biome::DeepLukewarmOcean
                | Biome::WarmOcean
        )
    }

    /// Холодный ли биом: в нём идёт снег, а вода сверху замерзает.
    pub fn is_freezing(self) -> bool {
        matches!(
            self,
            Biome::SnowyPlains
                | Biome::IceSpikes
                | Biome::SnowyTaiga
                | Biome::SnowyBeach
                | Biome::SnowySlopes
                | Biome::Grove
                | Biome::JaggedPeaks
                | Biome::FrozenPeaks
                | Biome::FrozenRiver
                | Biome::FrozenOcean
                | Biome::DeepFrozenOcean
        )
    }
}

/// Шесть чисел, по которым выбирается биом и складывается рельеф.
#[derive(Clone, Copy, Debug)]
pub struct Climate {
    pub continent: f64,
    pub erosion: f64,
    pub weirdness: f64,
    pub temperature: f64,
    pub humidity: f64,
}

impl Climate {
    /// Гребни и долины: `1 - |3|weirdness| - 2|` (вики, «peaks and valleys»).
    pub fn peaks_valleys(&self) -> f64 {
        1.0 - ((3.0 * self.weirdness.abs()) - 2.0).abs()
    }

    /// Уровень температуры, от 0 (мороз) до 4 (жара).
    pub fn temperature_level(&self) -> u8 {
        level(self.temperature, &[-0.45, -0.15, 0.2, 0.55])
    }

    /// Уровень влажности, от 0 (сушь) до 4 (сырость).
    pub fn humidity_level(&self) -> u8 {
        level(self.humidity, &[-0.35, -0.1, 0.1, 0.3])
    }

    /// Уровень эрозии, от 0 (горы) до 6 (ровно).
    pub fn erosion_level(&self) -> u8 {
        level(self.erosion, &[-0.78, -0.375, -0.2225, 0.05, 0.45, 0.55])
    }

    /// Уровень гребней и долин.
    pub fn pv_level(&self) -> PeaksValleys {
        let pv = self.peaks_valleys();

        if pv < -0.85 {
            PeaksValleys::Valleys
        } else if pv < -0.2 {
            PeaksValleys::Low
        } else if pv < 0.2 {
            PeaksValleys::Mid
        } else if pv < 0.7 {
            PeaksValleys::High
        } else {
            PeaksValleys::Peaks
        }
    }

    /// Насколько место удалено от моря вглубь суши.
    pub fn land_kind(&self) -> Land {
        match self.continent {
            c if c < -1.05 => Land::Mushroom,
            c if c < -0.455 => Land::DeepOcean,
            c if c < -0.19 => Land::Ocean,
            c if c < -0.11 => Land::Coast,
            c if c < 0.03 => Land::Near,
            c if c < 0.3 => Land::Mid,
            _ => Land::Far,
        }
    }
}

/// Уровни гребней и долин.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum PeaksValleys {
    Valleys,
    Low,
    Mid,
    High,
    Peaks,
}

/// Насколько место удалено вглубь суши.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Land {
    Mushroom,
    DeepOcean,
    Ocean,
    Coast,
    Near,
    Mid,
    Far,
}

/// Всё, что нужно знать про столбец мира: считается один раз на место, а не
/// на каждый блок.
#[derive(Clone, Copy, Debug)]
pub struct Column {
    /// Высота последнего непустого блока.
    pub height: i32,
    pub biome: Biome,
    pub climate: Climate,
}

impl Column {
    /// Колонка ровного мира: земля везде одной высоты, биом один.
    pub fn flat(height: i32) -> Column {
        Column {
            height,
            biome: Biome::Plains,
            climate: Climate {
                continent: 0.0,
                erosion: 0.0,
                weirdness: 0.0,
                temperature: 0.0,
                humidity: 0.0,
            },
        }
    }
}

/// Рельеф мира: набор шумов от одного семени.
pub struct Terrain {
    continent: Noise,
    erosion: Noise,
    weirdness: Noise,
    temperature: Noise,
    humidity: Noise,
    /// Мелкая неровность поверхности — чтобы склоны не были гладкими.
    detail: Noise,
    /// Крупные полости под землёй.
    caves: Noise,
    /// Узкие ходы: их прокладывают два шума сразу.
    tunnels: Noise,
    tunnels_across: Noise,
    /// Где лежат жилы руды.
    ores: Noise,
    seed: i64,
}

impl Terrain {
    /// Заводит рельеф от семени мира. Каждому шуму достаётся своё семя, иначе
    /// климат и рельеф повторяли бы друг друга.
    pub fn new(seed: i64) -> Terrain {
        Terrain {
            continent: Noise::new(seed),
            erosion: Noise::new(seed ^ 0x51_7C_C1_B7),
            weirdness: Noise::new(seed ^ 0x27_22_0A_95),
            temperature: Noise::new(seed ^ 0x6E_8D_1B_57),
            humidity: Noise::new(seed ^ 0x1B_87_35_4D),
            detail: Noise::new(seed ^ 0x3A_9F_04_E1),
            caves: Noise::new(seed ^ 0x64_C3_9A_11),
            tunnels: Noise::new(seed ^ 0x12_7D_55_09),
            tunnels_across: Noise::new(seed ^ 0x7E_41_B0_23),
            ores: Noise::new(seed ^ 0x05_6B_E7_8D),
            seed,
        }
    }

    /// Климат в этом месте. Масштабы подобраны так, чтобы материк читался на
    /// километрах, эрозия — на сотнях блоков, гребни — на десятках.
    ///
    /// Сумма октав жмётся к середине: сами по себе её значения почти не
    /// доходят до краёв, и тогда весь мир оказывается одного климата — вечный
    /// лес без пустынь и снегов. Поэтому каждое значение растягивается на весь
    /// промежуток от -1 до 1. У континентальности вдобавок сдвиг в сторону
    /// суши: иначе море занимает почти весь мир.
    pub fn climate_at(&self, x: i32, z: i32) -> Climate {
        let (x, z) = (x as f64, z as f64);

        Climate {
            continent: stretch(self.continent.octaves(x / 2400.0, z / 2400.0, 4), 2.4, 0.42),
            erosion: stretch(self.erosion.octaves(x / 900.0, z / 900.0, 3), 2.1, 0.0),
            weirdness: stretch(self.weirdness.octaves(x / 400.0, z / 400.0, 3), 2.1, 0.0),
            temperature: stretch(self.temperature.octaves(x / 1700.0, z / 1700.0, 2), 2.0, 0.0),
            humidity: stretch(self.humidity.octaves(x / 1200.0, z / 1200.0, 3), 2.2, 0.0),
        }
    }

    /// Всё про столбец: высота поверхности и биом.
    pub fn column_at(&self, x: i32, z: i32) -> Column {
        let climate = self.climate_at(x, z);
        let height = self.height_of(&climate, x, z);
        let biome = biome_of(&climate, height);

        Column { height, biome, climate }
    }

    /// Высота поверхности: основной уровень от континентальности, размах от
    /// эрозии, рельеф поверх — от гребней и долин.
    fn height_of(&self, climate: &Climate, x: i32, z: i32) -> i32 {
        // Основной уровень: от глубин океана до приподнятой суши.
        let base = spline(
            &[
                (-1.2, 40.0),
                (-1.05, 38.0),
                (-0.455, 44.0),
                (-0.19, 56.0),
                (-0.11, 62.0),
                (0.03, 67.0),
                (0.3, 73.0),
                (1.0, 82.0),
            ],
            climate.continent,
        );

        // Размах рельефа: при низкой эрозии горы, при высокой — равнина.
        let spread = spline(
            &[
                (-1.0, 62.0),
                (-0.78, 48.0),
                (-0.375, 30.0),
                (-0.2225, 22.0),
                (0.05, 13.0),
                (0.45, 7.0),
                (0.55, 4.0),
                (1.0, 2.0),
            ],
            climate.erosion,
        );

        // Гребни и долины: в долине рельеф уходит вниз, на гребне — вверх.
        let ridges = spline(
            &[(-1.0, -0.8), (-0.4, -0.25), (0.0, 0.0), (0.4, 0.45), (1.0, 1.0)],
            climate.peaks_valleys(),
        );

        let detail = self.detail.octaves(x as f64 / 55.0, z as f64 / 55.0, 3) * 2.5;

        // В море рельеф глушится: дно ровнее суши.
        let under_water = climate.continent < -0.19;
        let spread = if under_water { spread.min(8.0) } else { spread };

        let mut height = base + spread * ridges + detail;

        // Русло реки: в долинах суша опускается к самой воде.
        if !under_water && climate.pv_level() == PeaksValleys::Valleys {
            let depth = (-climate.peaks_valleys() - 0.85) / 0.15;
            let river = SEA as f64 - 2.0 - depth * 2.0;

            height = height.min(river);
        }

        height.round() as i32
    }

    /// Что стоит в этом месте столбца.
    pub fn block_at(&self, column: &Column, x: i32, y: i32, z: i32) -> i32 {
        if y < BOTTOM || y >= crate::world::MIN_Y + crate::world::WORLD_HEIGHT {
            return AIR;
        }

        // Дно мира: сплошной слой, а над ним ещё несколько с прорехами —
        // так у оригинала, и так его не пробить.
        if y == BOTTOM {
            return named("bedrock");
        }

        if y <= BOTTOM + 4 && self.bedrock_at(x, y, z) {
            return named("bedrock");
        }

        if y > column.height {
            // Над землёй: до уровня моря вода, выше — воздух. В холодных
            // местах вода сверху схвачена льдом.
            if y <= SEA && column.height < SEA {
                return if y == SEA && column.biome.is_freezing() {
                    named("ice")
                } else {
                    named("water")
                };
            }

            // На самой земле растёт трава и лежит снег.
            if column.height >= SEA {
                if let Some(plant) = self.plant_at(column, x, y, z) {
                    return named(plant);
                }
            }

            return AIR;
        }

        let depth = column.height - y;

        // Пещеры проедают камень, но не подходят к самой поверхности и не
        // трогают дно мира.
        if depth > CAVE_ROOF && y > BOTTOM + 5 && self.hollow_at(x, y, z) {
            return if y <= LAVA_LEVEL { named("lava") } else { AIR };
        }

        match surface(column, depth) {
            Some(name) => named(name),
            None => self.underground_at(column, x, y, z),
        }
    }

    /// Что растёт или лежит на земле в этом месте столбца.
    ///
    /// Всё редкое решается своим числом от координат: одно и то же место
    /// всегда даёт одно и то же, и соседние чанки сходятся без сговора.
    fn plant_at(&self, column: &Column, x: i32, y: i32, z: i32) -> Option<&'static str> {
        let above = y - column.height;
        let biome = column.biome;

        // В холодных местах на земле лежит снег — тонким слоем, как в игре.
        if above == 1 && biome.is_freezing() && !matches!(biome, Biome::FrozenRiver) {
            return Some("snow");
        }

        // Кактус растёт столбиком в два-три блока.
        if matches!(biome, Biome::Desert) {
            if above <= 3 && chance(x, z, 811, 0.004) {
                return Some("cactus");
            }

            if above == 1 && chance(x, z, 823, 0.01) {
                return Some("dead_bush");
            }

            return None;
        }

        if above != 1 {
            return None;
        }

        // Трава и цветы: густота зависит от биома.
        let (grass, flowers) = match biome {
            Biome::Plains | Biome::SunflowerPlains | Biome::Meadow => (0.3, 0.05),
            Biome::Forest | Biome::BirchForest | Biome::OldGrowthBirchForest => (0.25, 0.03),
            Biome::FlowerForest => (0.25, 0.2),
            Biome::Jungle | Biome::BambooJungle => (0.45, 0.02),
            Biome::SparseJungle => (0.3, 0.02),
            Biome::Savanna | Biome::SavannaPlateau | Biome::WindsweptSavanna => (0.3, 0.0),
            Biome::Taiga | Biome::SnowyTaiga | Biome::OldGrowthPineTaiga
            | Biome::OldGrowthSpruceTaiga => (0.15, 0.01),
            Biome::Swamp | Biome::MangroveSwamp => (0.2, 0.01),
            Biome::DarkForest => (0.15, 0.02),
            Biome::Badlands | Biome::ErodedBadlands | Biome::WoodedBadlands => (0.0, 0.0),
            _ => (0.05, 0.005),
        };

        if chance(x, z, 907, flowers) {
            // Цветы чередуются: какой именно — тоже от места.
            let kind = pick(x, z, 911, &["dandelion", "poppy", "cornflower", "azure_bluet"]);

            return Some(kind);
        }

        if chance(x, z, 919, grass) {
            let cold = matches!(
                biome,
                Biome::Taiga | Biome::SnowyTaiga | Biome::OldGrowthPineTaiga
                    | Biome::OldGrowthSpruceTaiga
            );

            return Some(if cold && chance(x, z, 929, 0.3) { "fern" } else { "short_grass" });
        }

        None
    }

    /// Пусто ли здесь: тут проходит пещера.
    ///
    /// Пустоты двух видов, как их описывает вики: крупные полости — там, где
    /// объёмный шум поднимается выше порога, и узкие ходы — там, где сразу
    /// два шума проходят близко к нулю. Ходы тянутся ниточками, полости
    /// расширяют их в залы.
    fn hollow_at(&self, x: i32, y: i32, z: i32) -> bool {
        let (fx, fy, fz) = (x as f64, y as f64, z as f64);

        // Ходы: близость обоих шумов к нулю задаёт линию их пересечения.
        let along = self.tunnels.at3(fx / 140.0, fy / 90.0, fz / 140.0);
        let across = self.tunnels_across.at3(fx / 140.0, fy / 90.0, fz / 140.0);

        if along.abs() < 0.07 && across.abs() < 0.07 {
            return true;
        }

        // Полости: чем глубже, тем чаще, но у самого дна снова реже.
        let room = self.caves.octaves3(fx / 70.0, fy / 45.0, fz / 70.0, 2);
        let deep = ((y - BOTTOM) as f64 / 120.0).clamp(0.0, 1.0);

        room > 0.52 - 0.12 * deep
    }

    /// Что лежит под землёй в этом месте: камень, глубинный сланец или руда.
    fn underground_at(&self, column: &Column, x: i32, y: i32, z: i32) -> i32 {
        let deepslate = y < DEEPSLATE_FROM;
        let stone = if deepslate { "deepslate" } else { "stone" };

        match self.ore_at(column, x, y, z) {
            Some(ore) => {
                // Под нулём руда лежит в глубинном сланце, и вид у неё свой.
                let name = if deepslate {
                    format!("deepslate_{}", ore)
                } else {
                    ore.to_string()
                };

                blocks::state_by_name(&name).unwrap_or_else(|| named(stone))
            }
            None => named(stone),
        }
    }

    /// Какая руда лежит в этом месте, если лежит.
    ///
    /// Руда идёт жилами, а не вкраплениями: мир разбит на ячейки, в каждой
    /// может завестись одна жила — своей породы, со своим средоточием и
    /// размером. Блок принадлежит жиле, если попал в её шар. Соседние ячейки
    /// тоже спрашиваем: жила может заходить за их край.
    ///
    /// Так дешевле, чем спрашивать шум про каждую руду в каждом блоке, и
    /// ближе к игре, где жилы тоже раскладываются по чанку целиком.
    fn ore_at(&self, column: &Column, x: i32, y: i32, z: i32) -> Option<&'static str> {
        /// Сторона ячейки, в которой может завестись одна жила.
        const CELL: i32 = 8;

        /// Дальше этого жила из соседней ячейки не дотянется.
        const REACH: i32 = 3;

        // Спрашиваем соседнюю ячейку только с той стороны, до которой
        // отсюда ближе, чем может дотянуться жила. Обычно это одна-две
        // ячейки вместо всех двадцати семи.
        let side = |inside: i32| {
            let low = if inside < REACH { -1 } else { 0 };
            let high = if inside >= CELL - REACH { 1 } else { 0 };

            low..=high
        };

        for dx in side(x.rem_euclid(CELL)) {
            for dy in side(y.rem_euclid(CELL)) {
                for dz in side(z.rem_euclid(CELL)) {
                    let cell = (
                        x.div_euclid(CELL) + dx,
                        y.div_euclid(CELL) + dy,
                        z.div_euclid(CELL) + dz,
                    );

                    let Some((ore, centre, radius)) = self.vein_in(column, cell) else {
                        continue;
                    };

                    let (cx, cy, cz) = centre;
                    let distance = (x - cx).pow(2) + (y - cy).pow(2) + (z - cz).pow(2);

                    if distance <= radius * radius {
                        return Some(ore);
                    }
                }
            }
        }

        None
    }

    /// Какая жила завелась в этой ячейке: порода, средоточие и размер.
    fn vein_in(
        &self,
        column: &Column,
        (cell_x, cell_y, cell_z): (i32, i32, i32),
    ) -> Option<(&'static str, (i32, i32, i32), i32)> {
        /// Сторона ячейки — та же, что и при поиске.
        const CELL: i32 = 8;

        let mixed = (cell_x as i64)
            .wrapping_mul(341_873_128_712)
            .wrapping_add((cell_y as i64).wrapping_mul(6_364_136_223))
            .wrapping_add((cell_z as i64).wrapping_mul(132_897_987_541))
            ^ self.seed;

        let mut random = Random::new(mixed);

        // Середина ячейки по высоте — она и решает, какая руда сюда годится.
        let y = cell_y * CELL + (random.next() % CELL as u64) as i32;

        // Какие руды могут лежать на этой высоте — считаем их доли, не
        // складывая список: списку понадобилась бы память, а решение тут
        // принимается на каждый блок камня.
        let fits = |ore: &Ore| {
            y >= ore.heights.0
                && y <= ore.heights.1
                && (ore.name != "emerald_ore" || is_mountain(column.biome))
        };

        let total: u32 = ORES.iter().filter(|ore| fits(ore)).map(|ore| ore.share).sum();

        if total == 0 {
            return None;
        }

        // Выбираем руду по её доле: редкие выпадают реже частых.
        let mut roll = (random.next() % total as u64) as u32;
        let mut chosen = None;

        for ore in ORES.iter().filter(|ore| fits(ore)) {
            if roll < ore.share {
                chosen = Some(*ore);
                break;
            }

            roll -= ore.share;
        }

        let chosen = chosen?;

        // Не в каждой ячейке есть жила: иначе камня бы не осталось.
        if random.next() % 100 >= VEIN_IN_CELL {
            return None;
        }

        let centre = (
            cell_x * CELL + (random.next() % CELL as u64) as i32,
            y,
            cell_z * CELL + (random.next() % CELL as u64) as i32,
        );

        // Размер жилы — от руды; у редких он меньше.
        let radius = chosen.radius;

        Some((chosen.name, centre, radius))
    }

    /// Есть ли бедрок в этой точке россыпи у дна: чем ближе к дну, тем чаще.
    fn bedrock_at(&self, x: i32, y: i32, z: i32) -> bool {
        let layer = (y - BOTTOM) as u64;
        let mixed = (x as i64)
            .wrapping_mul(341_873_128_712)
            .wrapping_add((z as i64).wrapping_mul(132_897_987_541))
            .wrapping_add((y as i64).wrapping_mul(7_919))
            ^ self.seed;

        Random::new(mixed).next() % 5 >= layer
    }
}

/// Какой биом при таком климате и такой высоте.
///
/// Порядок разбора — как в таблицах на вики: сперва море, потом долины и
/// берег, потом суша по эрозии, гребням, температуре и влажности.
fn biome_of(climate: &Climate, height: i32) -> Biome {
    let temperature = climate.temperature_level();
    let humidity = climate.humidity_level();
    let erosion = climate.erosion_level();
    let pv = climate.pv_level();

    // Море: влажность, эрозия и странность тут ни при чём — только
    // континентальность и температура.
    match climate.land_kind() {
        Land::Mushroom => return Biome::MushroomFields,
        Land::DeepOcean => {
            return match temperature {
                0 => Biome::DeepFrozenOcean,
                1 => Biome::DeepColdOcean,
                2 => Biome::DeepOcean,
                3 => Biome::DeepLukewarmOcean,
                _ => Biome::WarmOcean,
            };
        }
        Land::Ocean => {
            return match temperature {
                0 => Biome::FrozenOcean,
                1 => Biome::ColdOcean,
                2 => Biome::Ocean,
                3 => Biome::LukewarmOcean,
                _ => Biome::WarmOcean,
            };
        }
        _ => {}
    }

    // Суша ниже уровня моря — это залитая низина: море подступило к берегу.
    if height < SEA {
        return if temperature == 0 { Biome::FrozenOcean } else { Biome::Ocean };
    }

    // Долины: реки, а при самой ровной земле — болота.
    if pv == PeaksValleys::Valleys {
        if erosion == 6 && matches!(climate.land_kind(), Land::Near | Land::Mid | Land::Far) {
            return match temperature {
                0 => Biome::FrozenRiver,
                1 | 2 => Biome::Swamp,
                _ => Biome::MangroveSwamp,
            };
        }

        return if temperature == 0 { Biome::FrozenRiver } else { Biome::River };
    }

    // Побережье: пляж, а при обрывистом рельефе — каменистый берег.
    if climate.land_kind() == Land::Coast {
        if erosion <= 1 && matches!(pv, PeaksValleys::Low | PeaksValleys::Mid) {
            return Biome::StonyShore;
        }

        if height <= SEA + 2 {
            return beach(temperature);
        }
    }

    // Горы: низкая эрозия поднимает землю, и наверху уже не растёт трава.
    if erosion <= 1 {
        match pv {
            PeaksValleys::Peaks | PeaksValleys::High => {
                return match temperature {
                    0..=2 if climate.weirdness < 0.0 => Biome::JaggedPeaks,
                    0..=2 => Biome::FrozenPeaks,
                    3 => Biome::StonyPeaks,
                    _ => badlands(humidity, climate.weirdness),
                };
            }
            PeaksValleys::Mid => {
                return match (temperature, humidity) {
                    (0..=2, 0 | 1) => Biome::SnowySlopes,
                    (0..=2, _) => Biome::Grove,
                    _ => plateau(temperature, humidity, climate.weirdness),
                };
            }
            _ => {}
        }
    }

    // Обдуваемые всеми ветрами места: высокая эрозия и неровный рельеф.
    if erosion == 5 && matches!(pv, PeaksValleys::Mid | PeaksValleys::Peaks) {
        return shattered(temperature, humidity, climate.weirdness);
    }

    // Плоскогорья: приподнятая ровная земля вдали от моря.
    if matches!(erosion, 2 | 3)
        && matches!(pv, PeaksValleys::High | PeaksValleys::Peaks)
        && matches!(climate.land_kind(), Land::Mid | Land::Far)
    {
        return plateau(temperature, humidity, climate.weirdness);
    }

    middle(temperature, humidity, climate.weirdness)
}

/// Пляжные биомы — только по температуре.
fn beach(temperature: u8) -> Biome {
    match temperature {
        0 => Biome::SnowyBeach,
        4 => Biome::Desert,
        _ => Biome::Beach,
    }
}

/// Бесплодные земли — по влажности и странности.
fn badlands(humidity: u8, weirdness: f64) -> Biome {
    match humidity {
        0 | 1 if weirdness > 0.0 => Biome::ErodedBadlands,
        0..=2 => Biome::Badlands,
        _ => Biome::WoodedBadlands,
    }
}

/// Средние биомы — основная суша, по температуре и влажности.
fn middle(temperature: u8, humidity: u8, weirdness: f64) -> Biome {
    let weird = weirdness > 0.0;

    match (temperature, humidity) {
        (0, 0) if weird => Biome::IceSpikes,
        (0, 0 | 1) => Biome::SnowyPlains,
        (0, 2) if weird => Biome::SnowyTaiga,
        (0, 2) => Biome::SnowyPlains,
        (0, 3) => Biome::SnowyTaiga,
        (0, _) => Biome::Taiga,

        (1, 0) if weird => Biome::Forest,
        (1, 0 | 1) => Biome::Plains,
        (1, 2) => Biome::Forest,
        (1, 3) => Biome::Taiga,
        (1, _) if weird => Biome::OldGrowthPineTaiga,
        (1, _) => Biome::OldGrowthSpruceTaiga,

        (2, 0) if weird => Biome::SunflowerPlains,
        (2, 0) => Biome::FlowerForest,
        (2, 1) => Biome::Plains,
        (2, 2) => Biome::Forest,
        (2, 3) if weird => Biome::OldGrowthBirchForest,
        (2, 3) => Biome::BirchForest,
        (2, _) => Biome::DarkForest,

        (3, 0 | 1) => Biome::Savanna,
        (3, 2) if weird => Biome::Plains,
        (3, 2) => Biome::Forest,
        (3, 3) if weird => Biome::SparseJungle,
        (3, 3) => Biome::Jungle,
        (3, _) if weird => Biome::BambooJungle,
        (3, _) => Biome::Jungle,

        _ => Biome::Desert,
    }
}

/// Плоскогорья: луга, вишнёвые рощи, саванновые плоскогорья.
fn plateau(temperature: u8, humidity: u8, weirdness: f64) -> Biome {
    let weird = weirdness > 0.0;

    match (temperature, humidity) {
        (0, 0) if weird => Biome::IceSpikes,
        (0, 0..=2) => Biome::SnowyPlains,
        (0, _) => Biome::SnowyTaiga,

        (1, 0) if weird => Biome::CherryGrove,
        (1, 0 | 1) => Biome::Meadow,
        (1, 2) if weird => Biome::Meadow,
        (1, 2) => Biome::Forest,
        (1, 3) if weird => Biome::Meadow,
        (1, 3) => Biome::Taiga,
        (1, _) if weird => Biome::OldGrowthPineTaiga,
        (1, _) => Biome::OldGrowthSpruceTaiga,

        (2, 0 | 1) if weird => Biome::CherryGrove,
        (2, 0 | 1) => Biome::Meadow,
        (2, 2) => Biome::Forest,
        (2, 3) if weird => Biome::BirchForest,
        (2, 3) => Biome::Meadow,
        (2, _) => Biome::PaleGarden,

        (3, 0 | 1 | 2) => Biome::SavannaPlateau,
        (3, _) => Biome::Jungle,

        (_, 0 | 1) if weird => Biome::ErodedBadlands,
        (_, 0 | 1 | 2) => Biome::Badlands,
        _ => Biome::WoodedBadlands,
    }
}

/// Расколотые биомы: высокая эрозия, обдуваемые ветром холмы.
fn shattered(temperature: u8, humidity: u8, weirdness: f64) -> Biome {
    let weird = weirdness > 0.0;

    match (temperature, humidity) {
        (4, _) => Biome::Desert,
        (0 | 1, 0 | 1) => Biome::WindsweptGravellyHills,
        (0 | 1, 2) => Biome::WindsweptHills,
        (0 | 1, _) => Biome::WindsweptForest,
        (2, 0..=2) => Biome::WindsweptHills,
        (2, _) => Biome::WindsweptForest,
        (3, 0 | 1) => Biome::Savanna,
        (3, 2) if weird => Biome::Plains,
        (3, 2) => Biome::Forest,
        (3, 3) if weird => Biome::SparseJungle,
        (3, 3) => Biome::Jungle,
        (3, _) if weird => Biome::BambooJungle,
        _ => Biome::Jungle,
    }
}

/// Дерево: из чего ствол, из чего крона и какой она формы.
#[derive(Clone, Copy, Debug)]
pub struct Tree {
    pub log: &'static str,
    pub leaves: &'static str,
    pub trunk: i32,
    /// Ель и её родня — с острой кроной, остальные — с округлой.
    pub pointed: bool,
}

impl Terrain {
    /// Растёт ли в этом месте дерево.
    ///
    /// Место должно быть подходящим: земля выше уровня моря, биом лесной или
    /// степной, и рядом не должно быть другого ствола — иначе деревья
    /// срастались бы в сплошную стену. Соседей спрашиваем тем же способом,
    /// каким решаем про себя: сговариваться чанкам не о чем.
    pub fn tree_at(&self, column: &Column, x: i32, z: i32) -> Option<Tree> {
        if column.height < SEA || column.biome.is_ocean() {
            return None;
        }

        let (density, tree) = match column.biome {
            Biome::Forest => (0.035, OAK),
            Biome::FlowerForest => (0.02, OAK),
            Biome::BirchForest => (0.035, BIRCH),
            Biome::OldGrowthBirchForest => (0.045, BIRCH),
            Biome::DarkForest => (0.055, DARK_OAK),
            Biome::Taiga | Biome::SnowyTaiga => (0.03, SPRUCE),
            Biome::OldGrowthPineTaiga | Biome::OldGrowthSpruceTaiga => (0.05, SPRUCE),
            Biome::Grove => (0.03, SPRUCE),
            Biome::Jungle => (0.06, JUNGLE),
            Biome::BambooJungle => (0.04, JUNGLE),
            Biome::SparseJungle => (0.02, JUNGLE),
            Biome::Savanna | Biome::SavannaPlateau => (0.004, ACACIA),
            Biome::Plains | Biome::SunflowerPlains => (0.002, OAK),
            Biome::Meadow => (0.001, OAK),
            Biome::Swamp => (0.01, OAK),
            Biome::CherryGrove => (0.03, CHERRY),
            _ => return None,
        };

        if !chance(x, z, 1_301, density) {
            return None;
        }

        // Уступаем соседу, который ближе к началу отсчёта: так из двух
        // стволов рядом остаётся один, и оба чанка решают одинаково.
        // Высоту соседа не спрашиваем нарочно: это стоило бы пересчёта всех
        // шумов на каждое дерево, а на густоту леса почти не влияет.
        for dx in -2..=2 {
            for dz in -2..=2 {
                if (dx, dz) == (0, 0) || (dz, dx) > (0, 0) {
                    continue;
                }

                if chance(x + dx, z + dz, 1_301, density) {
                    return None;
                }
            }
        }

        // Высота ствола своя у каждого дерева.
        let extra = (Random::new((x as i64) * 7 + (z as i64) * 13 + 97).next() % 3) as i32;

        Some(Tree { trunk: tree.trunk + extra, ..tree })
    }
}

/// Породы деревьев, какие мы сажаем.
const OAK: Tree = Tree { log: "oak_log", leaves: "oak_leaves", trunk: 4, pointed: false };
const BIRCH: Tree = Tree { log: "birch_log", leaves: "birch_leaves", trunk: 5, pointed: false };
const DARK_OAK: Tree =
    Tree { log: "dark_oak_log", leaves: "dark_oak_leaves", trunk: 5, pointed: false };
const SPRUCE: Tree = Tree { log: "spruce_log", leaves: "spruce_leaves", trunk: 6, pointed: true };
const JUNGLE: Tree =
    Tree { log: "jungle_log", leaves: "jungle_leaves", trunk: 7, pointed: false };
const ACACIA: Tree = Tree { log: "acacia_log", leaves: "acacia_leaves", trunk: 5, pointed: false };
const CHERRY: Tree = Tree { log: "cherry_log", leaves: "cherry_leaves", trunk: 5, pointed: false };

/// Может ли в этом месте вообще стоять ствол.
///
/// Дешёвая проверка перед дорогой: самая густая чаща у нас реже одного ствола
/// на шестнадцать мест, поэтому почти для всех мест столбец считать незачем.
pub fn tree_possible(x: i32, z: i32) -> bool {
    chance(x, z, 1_301, DENSEST_FOREST)
}

/// Самая большая густота деревьев среди всех биомов.
const DENSEST_FOREST: f64 = 0.06;

/// Состояние блока по имени — для тех, кто складывает деревья.
pub fn block_named(name: &str) -> i32 {
    named(name)
}

/// Выпало ли редкое событие в этом месте: своё число для каждой точки,
/// одно и то же при каждом обращении.
fn chance(x: i32, z: i32, salt: i64, part: f64) -> bool {
    if part <= 0.0 {
        return false;
    }

    let mixed = (x as i64)
        .wrapping_mul(341_873_128_712)
        .wrapping_add((z as i64).wrapping_mul(132_897_987_541))
        .wrapping_add(salt.wrapping_mul(7_919));

    let value = Random::new(mixed).next() % 10_000;

    (value as f64) < part * 10_000.0
}

/// Выбирает одно из нескольких — тоже по месту.
fn pick<'a>(x: i32, z: i32, salt: i64, choices: &[&'a str]) -> &'a str {
    let mixed = (x as i64)
        .wrapping_mul(1_619)
        .wrapping_add((z as i64).wrapping_mul(31_337))
        .wrapping_add(salt);

    choices[(Random::new(mixed).next() as usize) % choices.len()]
}

/// Горный ли это биом: в горах растёт изумруд, и только там.
fn is_mountain(biome: Biome) -> bool {
    matches!(
        biome,
        Biome::JaggedPeaks
            | Biome::FrozenPeaks
            | Biome::StonyPeaks
            | Biome::SnowySlopes
            | Biome::Grove
            | Biome::Meadow
            | Biome::WindsweptHills
            | Biome::WindsweptGravellyHills
            | Biome::WindsweptForest
    )
}

/// Руда: как зовётся, на каких высотах лежит, какая у неё доля среди прочих
/// на той же высоте и насколько крупны её жилы.
///
/// Доли и промежутки высот — со страницы «Ore» на вики: уголь и железо
/// встречаются повсюду, алмаз только у дна, изумруд только в горах.
#[derive(Clone, Copy)]
struct Ore {
    name: &'static str,
    heights: (i32, i32),
    share: u32,
    radius: i32,
}

/// В скольких ячейках из ста заводится жила.
const VEIN_IN_CELL: u64 = 15;

const ORES: [Ore; 8] = [
    Ore { name: "coal_ore", heights: (0, 190), share: 30, radius: 3 },
    Ore { name: "iron_ore", heights: (-64, 72), share: 26, radius: 3 },
    Ore { name: "copper_ore", heights: (-16, 112), share: 14, radius: 3 },
    Ore { name: "redstone_ore", heights: (-64, 15), share: 12, radius: 2 },
    Ore { name: "lapis_ore", heights: (-64, 64), share: 5, radius: 2 },
    Ore { name: "gold_ore", heights: (-64, 32), share: 6, radius: 2 },
    Ore { name: "diamond_ore", heights: (-64, 16), share: 3, radius: 2 },
    Ore { name: "emerald_ore", heights: (-16, 256), share: 4, radius: 1 },
];

/// Чем укрыт биом сверху: имя блока на такой глубине от поверхности.
/// `None` — значит, дальше идёт камень.
fn surface(column: &Column, depth: i32) -> Option<&'static str> {
    let biome = column.biome;
    let height = column.height;
    let under_water = height < SEA;

    // Под водой трава не растёт: дно всегда песчаное или каменистое.
    if under_water {
        return match biome {
            Biome::StonyShore => None,
            _ if height < SEA - 7 => match depth {
                0..=2 => Some("gravel"),
                _ => None,
            },
            _ => match depth {
                0..=3 => Some("sand"),
                _ => None,
            },
        };
    }

    match biome {
        Biome::Desert | Biome::Beach => match depth {
            0..=4 => Some("sand"),
            5..=8 => Some("sandstone"),
            _ => None,
        },
        Biome::SnowyBeach => match depth {
            0..=4 => Some("sand"),
            5..=6 => Some("sandstone"),
            _ => None,
        },
        Biome::Badlands | Biome::ErodedBadlands | Biome::WoodedBadlands => match depth {
            0 => Some("red_sand"),
            1..=10 => Some("terracotta"),
            _ => None,
        },
        Biome::StonyShore | Biome::StonyPeaks | Biome::JaggedPeaks | Biome::FrozenPeaks => None,
        Biome::WindsweptGravellyHills => match depth {
            0..=1 => Some("gravel"),
            _ => None,
        },
        Biome::SnowySlopes | Biome::SnowyPlains | Biome::IceSpikes | Biome::SnowyTaiga => {
            match depth {
                0 => Some("snow_block"),
                1..=3 => Some("dirt"),
                _ => None,
            }
        }
        Biome::Grove => match depth {
            0 => Some("snow_block"),
            1 => Some("dirt"),
            2..=3 => Some("dirt"),
            _ => None,
        },
        Biome::MushroomFields => match depth {
            0 => Some("mycelium"),
            1..=3 => Some("dirt"),
            _ => None,
        },
        Biome::Swamp | Biome::MangroveSwamp => match depth {
            0 => Some("grass_block"),
            1..=3 => Some("dirt"),
            _ => None,
        },
        // Высокие кряжи оголяются: на них держится только камень.
        _ if height > SEA + 50 => None,
        _ => match depth {
            0 => Some("grass_block"),
            1..=3 => Some("dirt"),
            _ => None,
        },
    }
}

/// Растягивает значение шума на весь промежуток от -1 до 1 и сдвигает его.
/// Края обрезаются: за ними значение всё равно осталось бы крайним.
fn stretch(value: f64, times: f64, shift: f64) -> f64 {
    (value * times + shift).clamp(-1.0, 1.0)
}

/// Ломаная по точкам: между соседними точками значение идёт ровно, за краями
/// остаётся крайним. На таких ломаных в игре держится весь рельеф.
fn spline(points: &[(f64, f64)], at: f64) -> f64 {
    let Some(&(first_x, first_y)) = points.first() else {
        return 0.0;
    };

    if at <= first_x {
        return first_y;
    }

    for pair in points.windows(2) {
        let (left_x, left_y) = pair[0];
        let (right_x, right_y) = pair[1];

        if at <= right_x {
            let part = (at - left_x) / (right_x - left_x);

            return left_y + (right_y - left_y) * part;
        }
    }

    points.last().map(|&(_, y)| y).unwrap_or(0.0)
}

/// Уровень значения по границам: сколько границ оно превысило.
fn level(value: f64, bounds: &[f64]) -> u8 {
    bounds.iter().filter(|bound| value >= **bound).count() as u8
}

/// Состояние блока по имени — то, которое игра считает обычным.
fn named(name: &str) -> i32 {
    blocks::state_by_name(name).unwrap_or_else(|| panic!("блок {} есть в таблице", name))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Мир повторяется: одно семя — один и тот же рельеф.
    #[test]
    fn the_same_seed_gives_the_same_world() {
        let one = Terrain::new(1234);
        let same = Terrain::new(1234);
        let other = Terrain::new(1235);

        let mut differs = false;

        for step in 0..64 {
            let (x, z) = (step * 7, step * 13);

            assert_eq!(one.column_at(x, z).height, same.column_at(x, z).height);
            differs |= one.column_at(x, z).height != other.column_at(x, z).height;
        }

        assert!(differs, "разные семена дали один и тот же мир");
    }

    /// Поверхность не проваливается за пределы мира и не упирается в потолок.
    #[test]
    fn the_surface_stays_in_the_world() {
        let terrain = Terrain::new(99);

        for step in 0..3000 {
            let x = (step * 37) % 6000 - 3000;
            let z = (step * 53) % 6000 - 3000;
            let height = terrain.column_at(x, z).height;

            assert!(height > BOTTOM + 8, "поверхность у дна мира: {}", height);
            assert!(height < 250, "поверхность под небом: {}", height);
        }
    }

    /// Ломаная идёт через свои точки и не выходит за края.
    #[test]
    fn a_spline_goes_through_its_points() {
        let points = [(-1.0, 10.0), (0.0, 20.0), (1.0, 40.0)];

        assert_eq!(spline(&points, -1.0), 10.0);
        assert_eq!(spline(&points, 0.0), 20.0);
        assert_eq!(spline(&points, 1.0), 40.0);
        assert_eq!(spline(&points, -5.0), 10.0, "за левым краем");
        assert_eq!(spline(&points, 5.0), 40.0, "за правым краем");
        assert_eq!(spline(&points, 0.5), 30.0, "посередине");
    }

    /// Уровни считаются по границам с вики.
    #[test]
    fn levels_match_the_wiki() {
        let cold = Climate {
            continent: 0.0,
            erosion: 0.0,
            weirdness: 0.0,
            temperature: -1.0,
            humidity: -1.0,
        };

        assert_eq!(cold.temperature_level(), 0);
        assert_eq!(cold.humidity_level(), 0);

        let hot = Climate { temperature: 0.9, humidity: 0.9, ..cold };

        assert_eq!(hot.temperature_level(), 4);
        assert_eq!(hot.humidity_level(), 4);

        // Границы эрозии: семь уровней.
        let flat = Climate { erosion: 0.9, ..cold };
        let steep = Climate { erosion: -0.9, ..cold };

        assert_eq!(flat.erosion_level(), 6);
        assert_eq!(steep.erosion_level(), 0);
    }

    /// Гребни и долины считаются по формуле с вики.
    #[test]
    fn peaks_and_valleys_follow_the_formula() {
        let climate = |weirdness| Climate {
            continent: 0.0,
            erosion: 0.0,
            weirdness,
            temperature: 0.0,
            humidity: 0.0,
        };

        // При странности 0 значение -1: это дно долины.
        assert!((climate(0.0).peaks_valleys() + 1.0).abs() < 1e-9);
        // При 2/3 — единица: это гребень.
        assert!((climate(2.0 / 3.0).peaks_valleys() - 1.0).abs() < 1e-9);
        // Формула симметрична: знак странности не важен.
        assert_eq!(climate(0.5).peaks_valleys(), climate(-0.5).peaks_valleys());
    }

    /// Низины залиты водой до уровня моря, и ни каплей выше.
    #[test]
    fn the_sea_fills_the_lowlands() {
        let terrain = Terrain::new(77);
        let water = named("water");
        let ice = named("ice");

        let mut seen_water = false;

        for step in 0..3000 {
            let x = (step * 61) % 3000 - 1500;
            let z = (step * 29) % 3000 - 1500;
            let column = terrain.column_at(x, z);

            if column.height < SEA {
                let top = terrain.block_at(&column, x, SEA, z);

                assert!(top == water || top == ice, "низина не залита: {}", top);
                assert_eq!(terrain.block_at(&column, x, SEA + 1, z), AIR);
                seen_water = true;
            }
        }

        assert!(seen_water, "во всём мире не нашлось воды");
    }

    /// Под поверхностью камень, а глубже нуля — глубинный сланец.
    #[test]
    fn stone_below_and_deepslate_deeper() {
        let terrain = Terrain::new(8);
        let column = terrain.column_at(500, 500);

        assert_eq!(terrain.block_at(&column, 500, column.height - 12, 500), named("stone"));
        assert_eq!(terrain.block_at(&column, 500, -20, 500), named("deepslate"));
    }

    /// Дно мира сплошное, а над ним бедрок с прорехами.
    #[test]
    fn the_bottom_is_solid_bedrock() {
        let terrain = Terrain::new(5);
        let bedrock = named("bedrock");

        for x in 0..40 {
            let column = terrain.column_at(x, x);

            assert_eq!(terrain.block_at(&column, x, BOTTOM, x), bedrock);
            assert_ne!(terrain.block_at(&column, x, BOTTOM + 5, x), bedrock);
        }
    }

    /// Биомы встречаются разные, и у моря он морской.
    #[test]
    fn the_world_has_different_biomes() {
        let terrain = Terrain::new(2024);
        let mut seen: Vec<Biome> = Vec::new();

        for step in 0..20000 {
            let x = (step * 43) % 12000 - 6000;
            let z = (step * 71) % 12000 - 6000;
            let column = terrain.column_at(x, z);

            if !seen.contains(&column.biome) {
                seen.push(column.biome);
            }
        }

        assert!(seen.len() >= 8, "биомов слишком мало: {:?}", seen);
        assert!(seen.iter().any(|biome| biome.is_ocean()), "нет ни одного моря");
    }

    /// В списке ALL нет пропусков: любой биом, который может выбрать
    /// генератор, имеет номер. Пропущенный уходил бы клиенту чужим биомом.
    #[test]
    fn no_biome_is_missing_from_the_list() {
        let terrain = Terrain::new(31337);

        for step in 0..20000 {
            let x = (step * 43) % 12000 - 6000;
            let z = (step * 71) % 12000 - 6000;

            // Паникует, если биома нет в списке.
            terrain.column_at(x, z).biome.number();
        }

        // И прямая проверка каждого вида, какой умеет выбирать генератор.
        for biome in [
            Biome::Badlands,
            Biome::ErodedBadlands,
            Biome::WoodedBadlands,
            Biome::PaleGarden,
            Biome::MushroomFields,
            Biome::WindsweptSavanna,
        ] {
            biome.number();
        }
    }

    /// У каждого биома свой номер, и имена не повторяются.
    #[test]
    fn every_biome_has_its_own_number() {
        for (i, biome) in Biome::ALL.iter().enumerate() {
            assert_eq!(biome.number(), i as i32);
        }

        let mut names: Vec<&str> = Biome::ALL.iter().map(|biome| biome.name()).collect();
        let count = names.len();

        names.sort_unstable();
        names.dedup();

        assert_eq!(names.len(), count, "имена биомов повторяются");
    }
}

#[cfg(test)]
mod survey {
    use super::*;
    use std::collections::HashMap;

    /// Разведка: печатает, из чего складывается мир — доли биомов и разброс
    /// высот. Не проверка, а инструмент настройки; запускается вручную:
    /// `cargo test survey -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn what_the_world_is_made_of() {
        let terrain = Terrain::new(100_554_032_945_340);

        let mut biomes: HashMap<&str, u32> = HashMap::new();
        let mut heights: Vec<i32> = Vec::new();
        let mut under_water = 0;
        let mut total = 0;

        // Шаг в четыре блока: как ячейка биома в пакете чанка.
        for x in (-4000..4000).step_by(16) {
            for z in (-4000..4000).step_by(16) {
                let column = terrain.column_at(x, z);

                *biomes.entry(column.biome.name()).or_default() += 1;
                heights.push(column.height);

                if column.height < SEA {
                    under_water += 1;
                }

                total += 1;
            }
        }

        heights.sort_unstable();

        println!("мест просмотрено: {}", total);
        println!("под водой: {:.1}%", 100.0 * under_water as f64 / total as f64);
        println!(
            "высоты: нижняя {}, четверть {}, середина {}, три четверти {}, верхняя {}",
            heights[0],
            heights[heights.len() / 4],
            heights[heights.len() / 2],
            heights[heights.len() * 3 / 4],
            heights[heights.len() - 1]
        );

        // Заодно смотрим, что под землёй: сколько пустот и сколько руды.
        let mut hollow = 0u32;
        let mut ores: HashMap<&str, u32> = HashMap::new();
        let mut stone = 0u32;

        for step in 0..400 {
            let x = (step * 37) % 2000 - 1000;
            let z = (step * 53) % 2000 - 1000;
            let column = terrain.column_at(x, z);

            for y in (BOTTOM + 6)..column.height {
                let block = terrain.block_at(&column, x, y, z);

                if block == AIR || block == named("lava") {
                    hollow += 1;
                    continue;
                }

                stone += 1;

                if let Some(ore) = terrain.ore_at(&column, x, y, z) {
                    *ores.entry(ore).or_default() += 1;
                }
            }
        }

        let underground = hollow + stone;

        println!(
            "под землёй: пустот {:.1}%, руды {:.2}%",
            100.0 * hollow as f64 / underground as f64,
            100.0 * ores.values().sum::<u32>() as f64 / underground as f64
        );

        let mut ore_rows: Vec<(&str, u32)> = ores.into_iter().collect();
        ore_rows.sort_by_key(|(_, count)| std::cmp::Reverse(*count));

        for (name, count) in ore_rows {
            println!("  {:>6.3}%  {}", 100.0 * count as f64 / underground as f64, name);
        }

        let mut sorted: Vec<(&str, u32)> = biomes.into_iter().collect();
        sorted.sort_by_key(|(_, count)| std::cmp::Reverse(*count));

        println!("биомы:");
        for (name, count) in sorted {
            println!("  {:>6.2}%  {}", 100.0 * count as f64 / total as f64, name);
        }
    }
}
