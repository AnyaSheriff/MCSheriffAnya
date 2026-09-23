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
pub use super::trees::{Axis, Part, TREE_REACH, tree_possible};

pub const SEA: i32 = 63;

/// Дно мира.
pub const BOTTOM: i32 = crate::world::MIN_Y;

/// Высота, с которой вниз идёт глубинный сланец вместо камня.
const DEEPSLATE_FROM: i32 = 0;

/// Ниже этой высоты пустоты пещер залиты лавой — как в игре.
const LAVA_LEVEL: i32 = -55;

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
    /// Оголяются ли в этом биоме крутые склоны до камня.
    pub fn bares_steep_slopes(self) -> bool {
        matches!(
            self,
            Biome::SnowySlopes
                | Biome::Grove
                | Biome::Meadow
                | Biome::CherryGrove
                | Biome::WindsweptHills
                | Biome::WindsweptForest
                | Biome::WindsweptGravellyHills
                | Biome::StonyShore
        )
    }

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
    /// Крутой склон: у соседа с одной стороны земля на четыре блока и
    /// больше выше, чем с другой. Там вместо травы камень. Решает чанк —
    /// ему видны соседние столбцы.
    pub steep: bool,
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
            steep: false,
        }
    }
}

/// Ширина клетки сетки плотности по горизонтали — как у оригинала.
const CELL_WIDTH: i32 = 4;

/// Высота клетки сетки плотности.
const CELL_HEIGHT: i32 = 8;

/// Опора рельефа в столбце «Обычного» мира.
#[derive(Clone, Copy, Debug)]
struct Relief {
    /// Где лежала бы поверхность без объёмного шума.
    surface: f64,
    /// На сколько блоков шум может её сдвинуть.
    wobble: f64,
    /// Мелкая изломанность ветреных холмов: ещё один объёмный шум,
    /// частый по горизонтали, — на столько блоков он сдвигает поверхность.
    rugged: f64,
    /// Выше этой высоты камня нет.
    cap: f64,
}

/// Сетка плотности над участком: значения в узлах через 4 блока по
/// горизонтали и 8 по высоте. Между узлами плотность интерполируется,
/// ниже сетки — заведомо камень, выше — заведомо воздух.
pub struct ReliefGrid {
    x0: i32,
    z0: i32,
    across_z: i32,
    y0: i32,
    layers: i32,
    nodes: Vec<f64>,
}

impl ReliefGrid {
    /// Плотность в узле сетки.
    fn node(&self, cx: i32, cz: i32, layer: i32) -> f64 {
        self.nodes[((cx * self.across_z + cz) * self.layers + layer) as usize]
    }

    /// Камень ли в этом месте.
    pub fn solid(&self, x: i32, y: i32, z: i32) -> bool {
        if y < self.y0 {
            return true;
        }

        let layer = (y - self.y0) / CELL_HEIGHT;

        if layer >= self.layers - 1 {
            return false;
        }

        let (cx, cz) = ((x - self.x0) / CELL_WIDTH, (z - self.z0) / CELL_WIDTH);
        let tx = ((x - self.x0) % CELL_WIDTH) as f64 / CELL_WIDTH as f64;
        let tz = ((z - self.z0) % CELL_WIDTH) as f64 / CELL_WIDTH as f64;
        let ty = ((y - self.y0) % CELL_HEIGHT) as f64 / CELL_HEIGHT as f64;

        let lerp = |a: f64, b: f64, t: f64| a + (b - a) * t;
        let at = |dx: i32, dy: i32, dz: i32| self.node(cx + dx, cz + dz, layer + dy);

        let bottom = lerp(
            lerp(at(0, 0, 0), at(1, 0, 0), tx),
            lerp(at(0, 0, 1), at(1, 0, 1), tx),
            tz,
        );
        let top = lerp(
            lerp(at(0, 1, 0), at(1, 1, 0), tx),
            lerp(at(0, 1, 1), at(1, 1, 1), tx),
            tz,
        );

        lerp(bottom, top, ty) > 0.0
    }

    /// Высота верхнего камня в столбце.
    pub fn top(&self, x: i32, z: i32) -> i32 {
        let highest = self.y0 + (self.layers - 1) * CELL_HEIGHT;

        (self.y0..highest)
            .rev()
            .find(|&y| self.solid(x, y, z))
            .unwrap_or(self.y0 - 1)
    }
}

/// Сетка шумов пещер: в узлах через 4 блока по горизонтали и 8 по высоте
/// три значения — два шума ходов и шум полостей.
pub struct CaveGrid {
    x0: i32,
    z0: i32,
    across_z: i32,
    y0: i32,
    layers: i32,
    nodes: Vec<[f64; 3]>,
}

impl CaveGrid {
    /// Пусто ли в этом месте: шумы тянутся между узлами, пороги — как
    /// ниже в `hollow_by`.
    /// `depth` — сколько камня над местом: у самой поверхности залы
    /// сходят на нет, остаются только узкие ходы.
    pub fn hollow(&self, x: i32, y: i32, z: i32, depth: i32) -> bool {
        let (cx, cz) = ((x - self.x0) / CELL_WIDTH, (z - self.z0) / CELL_WIDTH);
        let layer = (y - self.y0) / CELL_HEIGHT;
        let tx = ((x - self.x0) % CELL_WIDTH) as f64 / CELL_WIDTH as f64;
        let tz = ((z - self.z0) % CELL_WIDTH) as f64 / CELL_WIDTH as f64;
        let ty = ((y - self.y0) % CELL_HEIGHT) as f64 / CELL_HEIGHT as f64;

        let at = |dx: i32, dy: i32, dz: i32| {
            &self.nodes[(((cx + dx) * self.across_z + cz + dz) * self.layers + layer + dy) as usize]
        };
        let mut values = [0.0; 3];

        for (index, value) in values.iter_mut().enumerate() {
            let lerp = |a: f64, b: f64, t: f64| a + (b - a) * t;
            let corner = |dx: i32, dy: i32, dz: i32| at(dx, dy, dz)[index];
            let bottom = lerp(
                lerp(corner(0, 0, 0), corner(1, 0, 0), tx),
                lerp(corner(0, 0, 1), corner(1, 0, 1), tx),
                tz,
            );
            let top = lerp(
                lerp(corner(0, 1, 0), corner(1, 1, 0), tx),
                lerp(corner(0, 1, 1), corner(1, 1, 1), tx),
                tz,
            );

            *value = lerp(bottom, top, ty);
        }

        hollow_by(values, y, depth)
    }
}

/// Пусто ли место при таких шумах пещер.
///
/// Ходы — там, где оба шума ходов близки к нулю: так пустота тянется
/// ниточкой по линии их пересечения. Полости — где шум полостей выше порога.
fn hollow_by([along, across, room]: [f64; 3], y: i32, depth: i32) -> bool {
    let fy = y as f64;

    // У самого дна пещеры сходят на нет: по замерам оригинала ниже −54
    // пустот в семь раз меньше, чем было у нас, и лава там — лужи на
    // дне редких пещер, а не сплошное море.
    let floor = smooth_step((fy + 65.0) / 20.0);
    let width = 0.09 * (0.3 + 0.7 * floor) * (1.0 - 0.45 * smooth_step((fy - 45.0) / 30.0));

    if along.abs() < width && across.abs() < width {
        return true;
    }

    // Полости: чем глубже, тем чаще — до глубинного сланца; у дна
    // снова реже. Выше нуля у оригинала пустот больше, чем кажется: там и
    // пещеры, и подземные озёра.
    let deep = ((SEA - y) as f64 / 100.0).clamp(0.0, 1.0);
    let shallow = smooth_step((fy + 10.0) / 30.0) * (1.0 - smooth_step((fy - 35.0) / 25.0));

    // У самой поверхности залов почти нет: по замерам оригинала пустот
    // в верхних 30 блоках под землёй втрое меньше, чем выходило у нас.
    // В толще гор (выше ~100) оригинал оставляет пещеры и у поверхности.
    let roof = (1.0 - smooth_step((depth - CAVE_ROOF) as f64 / 26.0)) * (1.0 - smooth_step((fy - 80.0) / 40.0));

    // В толще гор, выше ~80, у оригинала пещер у поверхности заметно больше,
    // чем в равнинной земле: нависаний там почти каждый пятый столбец.
    let peaks = smooth_step((fy - 75.0) / 45.0);

    room > 0.405 - 0.08 * deep - 0.23 * shallow - 0.2 * peaks + 0.3 * (1.0 - floor) + 0.2 * roof
}

/// Какой рельеф складывать: как у оригинала или наш плавный.
///
/// Под землёй они одинаковы — пещеры, руда, породы. Различается только
/// поверхность: у оригинала она обрывистая, с уступами и высокими горами,
/// у «Супер плавности» — мягкая, без резких перепадов.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Style {
    /// Обычный мир — подгоняется к ванильным замерам.
    Vanilla,
    /// «Супер плавность» — рельеф, каким он был до подгонки под ваниль.
    Smooth,
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
    /// Объёмный шум «Обычного» рельефа: скалы, обрывы, нависания.
    relief: Noise,
    /// Подземные водоёмы: где пещера залита и до какого уровня.
    flood: Noise,
    flood_level: Noise,
    /// Крупные полости под землёй.
    caves: Noise,
    /// Узкие ходы: их прокладывают два шума сразу.
    tunnels: Noise,
    tunnels_across: Noise,
    style: Style,
    seed: i64,
}

impl Terrain {
    /// Заводит рельеф от семени мира. Каждому шуму достаётся своё семя, иначе
    /// климат и рельеф повторяли бы друг друга.
    pub fn new(seed: i64, style: Style) -> Terrain {
        Terrain {
            continent: Noise::new(seed),
            erosion: Noise::new(seed ^ 0x51_7C_C1_B7),
            weirdness: Noise::new(seed ^ 0x27_22_0A_95),
            temperature: Noise::new(seed ^ 0x6E_8D_1B_57),
            humidity: Noise::new(seed ^ 0x1B_87_35_4D),
            detail: Noise::new(seed ^ 0x3A_9F_04_E1),
            relief: Noise::new(seed ^ 0x5D_21_E8_3F),
            flood: Noise::new(seed ^ 0x2B_6A_D4_71),
            flood_level: Noise::new(seed ^ 0x49_0C_97_E5),
            caves: Noise::new(seed ^ 0x64_C3_9A_11),
            tunnels: Noise::new(seed ^ 0x12_7D_55_09),
            tunnels_across: Noise::new(seed ^ 0x7E_41_B0_23),
            style,
            seed,
        }
    }

    /// Семя мира, от которого заведены шумы.
    pub fn seed(&self) -> i64 {
        self.seed
    }

    /// Какой рельеф складывается.
    pub fn style(&self) -> Style {
        self.style
    }

    /// Плавный рельеф — для проверок, которым не важен стиль.
    #[cfg(test)]
    pub fn smooth(seed: i64) -> Terrain {
        Terrain::new(seed, Style::Smooth)
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
        // Температура и влажность решают только биом, а биом у оригинала
        // один на клетку 4×4: берём их в середине клетки. Так граница биома
        // — и край льда на воде — идёт ломаной по клеткам, а не дугой.
        let (cell_x, cell_z) = (((x & !3) + 2) as f64, ((z & !3) + 2) as f64);
        let (x, z) = (x as f64, z as f64);

        // В узлах решётки шум Перлина равен нулю, и через каждые 400 блоков
        // странность обнулялась бы — ноль у неё значит русло реки, и
        // по миру шли бы ямки-русла в клетку. У «Обычного» узлы сдвинуты на
        // дробную долю; «Супер плавность» остаётся прежней, чтобы её миры
        // не получили швов.
        let off = match self.style {
            Style::Vanilla => 0.371,
            Style::Smooth => 0.0,
        };

        Climate {
            continent: stretch(self.continent.octaves(x / 2400.0 + off, z / 2400.0 + off, 4), 2.4, 0.42),
            // У «Обычного» эрозия размашистее: горы занимают заметную часть
            // суши, как у оригинала. «Супер плавность» остаётся прежней.
            erosion: stretch(
                self.erosion.octaves(x / 900.0 + off, z / 900.0 + off, 3),
                match self.style {
                    Style::Vanilla => 3.2,
                    Style::Smooth => 2.1,
                },
                0.0,
            ),
            weirdness: stretch(self.weirdness.octaves(x / 600.0 + off, z / 600.0 + off, 3), 2.1, 0.0),
            // Мелкие октавы дают границе рябь на десятках блоков: край
            // холодных мест рваный, как у оригинала.
            temperature: stretch(
                self.temperature
                    .octaves(cell_x / 1700.0 + off, cell_z / 1700.0 + off, 5),
                2.0,
                0.0,
            ),
            humidity: stretch(
                self.humidity.octaves(cell_x / 1200.0 + off, cell_z / 1200.0 + off, 5),
                2.2,
                0.0,
            ),
        }
    }

    /// Всё про столбец: высота поверхности и биом.
    ///
    /// У «Обычного» мира высоту даёт объёмная плотность, поэтому для одного
    /// столбца строится маленькая сетка из четырёх опорных столбцов — та же,
    /// что у чанка, и ответ выходит тем же.
    pub fn column_at(&self, x: i32, z: i32) -> Column {
        match self.style {
            Style::Smooth => self.column_in(x, z, None),
            Style::Vanilla => {
                let grid = self.relief_grid(x, z, x, z);

                self.column_in(x, z, Some(&grid))
            }
        }
    }

    /// Столбец, когда сетка плотности уже построена (или не нужна).
    pub fn column_in(&self, x: i32, z: i32, grid: Option<&ReliefGrid>) -> Column {
        let climate = self.climate_at(x, z);
        let height = match grid {
            Some(grid) => grid.top(x, z),
            None => self.smooth_height(&climate, x, z),
        };
        let biome = biome_of(&climate, height);

        Column {
            height,
            biome,
            climate,
            steep: false,
        }
    }

    /// Камень ли в этом месте рельефа (без пещер).
    pub fn solid_in(
        &self,
        grid: Option<&ReliefGrid>,
        column: &Column,
        x: i32,
        y: i32,
        z: i32,
    ) -> bool {
        match grid {
            Some(grid) => grid.solid(x, y, z),
            None => y <= column.height,
        }
    }

    /// Сетка плотности, покрывающая блоки от (x_from, z_from) до (x_to, z_to)
    /// включительно. Опорные столбцы — через каждые 4 блока, узлы по высоте —
    /// через каждые 8, как у оригинала; между узлами плотность плавно
    /// интерполируется. Узлы считаются только в полосе, где плотность может
    /// сменить знак: ниже неё заведомо камень, выше — заведомо воздух.
    pub fn relief_grid(&self, x_from: i32, z_from: i32, x_to: i32, z_to: i32) -> ReliefGrid {
        let x0 = x_from.div_euclid(CELL_WIDTH) * CELL_WIDTH;
        let z0 = z_from.div_euclid(CELL_WIDTH) * CELL_WIDTH;
        let across_x = (x_to.div_euclid(CELL_WIDTH) * CELL_WIDTH - x0) / CELL_WIDTH + 2;
        let across_z = (z_to.div_euclid(CELL_WIDTH) * CELL_WIDTH - z0) / CELL_WIDTH + 2;

        let mut reliefs = Vec::with_capacity((across_x * across_z) as usize);

        for cx in 0..across_x {
            for cz in 0..across_z {
                let (x, z) = (x0 + cx * CELL_WIDTH, z0 + cz * CELL_WIDTH);
                let climate = self.climate_at(x, z);

                reliefs.push(self.relief_at(&climate, x, z));
            }
        }

        let low = reliefs
            .iter()
            .map(|r| r.surface.min(r.cap) - r.wobble - r.rugged)
            .fold(f64::MAX, f64::min);
        let high = reliefs
            .iter()
            .map(|r| (r.surface + r.wobble + r.rugged).min(r.cap))
            .fold(f64::MIN, f64::max);
        let y0 = ((low as i32) - CELL_HEIGHT).div_euclid(CELL_HEIGHT) * CELL_HEIGHT;
        let y_top = ((high as i32) + 2 * CELL_HEIGHT).div_euclid(CELL_HEIGHT) * CELL_HEIGHT;
        let layers = (y_top - y0) / CELL_HEIGHT + 1;

        let mut nodes = Vec::with_capacity((across_x * across_z * layers) as usize);

        for cx in 0..across_x {
            for cz in 0..across_z {
                let relief = reliefs[(cx * across_z + cz) as usize];
                let (x, z) = (x0 + cx * CELL_WIDTH, z0 + cz * CELL_WIDTH);

                for layer in 0..layers {
                    nodes.push(self.density(&relief, x, y0 + layer * CELL_HEIGHT, z));
                }
            }
        }

        ReliefGrid {
            x0,
            z0,
            across_z,
            y0,
            layers,
            nodes,
        }
    }

    /// Плотность в узле: больше нуля — камень. Поверхность там, где
    /// `surface`, но объёмный шум сдвигает её на `wobble` блоков вверх или
    /// вниз — по-разному на разной высоте, отсюда скалы, обрывы и нависания.
    /// Выше `cap` камня нет: там русло реки или толща океана.
    fn density(&self, relief: &Relief, x: i32, y: i32, z: i32) -> f64 {
        let (fx, fy, fz) = (x as f64, y as f64, z as f64);
        let shake = self.relief.octaves3(fx / 56.0, fy / 24.0, fz / 56.0, 3) * 1.7;
        let mut density = relief.surface - fy + relief.wobble * shake;

        if relief.rugged > 0.0 {
            density += relief.rugged * self.relief.octaves3(fx / 12.0 + 900.0, fy / 12.0, fz / 12.0, 2) * 1.5;
        }

        density.min(relief.cap - fy)
    }

    /// Опора рельефа «Обычного» мира в столбце: где лежит поверхность,
    /// насколько её раскачивает объёмный шум и выше чего камня быть не может.
    ///
    /// Уровень — от континентальности, размах гор — от эрозии, гребни и
    /// долины — от странности; острые пики — только там, где высокая суша,
    /// низкая эрозия и гребни, как у оригинала. Раскачка сильная в горах и
    /// почти никакая у воды: поэтому скалы в горах, а берега ровные.
    fn relief_at(&self, climate: &Climate, x: i32, z: i32) -> Relief {
        let (fx, fz) = (x as f64, z as f64);

        let base = spline(
            &[
                (-1.2, 30.0),
                (-1.05, 28.0),
                (-0.455, 34.0),
                (-0.19, 52.0),
                (-0.11, 62.0),
                (0.03, 64.0),
                (0.3, 66.0),
                (1.0, 70.0),
            ],
            climate.continent,
        );

        let spread = spline(
            &[
                (-1.0, 92.0),
                (-0.78, 88.0),
                (-0.375, 60.0),
                (-0.2225, 32.0),
                (0.05, 14.0),
                (0.45, 7.0),
                (0.55, 4.0),
                (1.0, 2.0),
            ],
            climate.erosion,
        );

        let ridges = spline(
            &[
                (-1.0, -0.8),
                (-0.4, -0.25),
                (-0.2, -0.05),
                // Пики у оригинала начинаются уже с «высоких» гребней
                // (0.2…0.7): там земля почти так же высока, как на пиках.
                (0.2, 0.5),
                (0.7, 0.9),
                (1.0, 1.0),
            ],
            climate.peaks_valleys(),
        );

        // Суша и море переходят друг в друга плавно: `land` от 0 в море до 1
        // на берегу. Резкая граница давала бы отвесную стенку вдоль всего
        // побережья.
        let land = smooth_step((climate.continent + 0.21) / 0.1);

        // Горный массив поднят целиком: чем ниже эрозия и дальше от моря,
        // тем выше вся земля, а гребни лишь добавляют сверху и долины
        // немного срезают — как у оригинала, где горы стоят хребтами, а не
        // одиночными холмами среди ям.
        let far_inland = smooth_step((climate.continent + 0.19) / 0.25);
        let lift = if ridges >= 0.0 { 0.15 + 0.85 * ridges } else { 0.15 + 0.15 * ridges };
        let sea_floor = base + spread.min(10.0) * ridges;
        let mut ground = base + spread * lift * (0.35 + 0.65 * far_inland);
        let mountains = smooth_step((spread - 30.0) / 60.0) * far_inland;

        // Острые пики: гребневой шум — единица минус модуль обычного —
        // даёт хребты вместо холмов.
        if ridges > 0.0 {
            // Модуль сглажен у нуля: гребень острый, но без излома — излом
            // давал отвесную стенку вдоль всего хребта.
            let crest = self.detail.octaves(fx / 200.0 + 400.0, fz / 200.0, 2);
            let ridge = 1.0 - (crest * crest + 0.004).sqrt() * 2.2;

            ground += ridge.max(0.0).powi(2) * 42.0 * smooth_step((spread - 30.0) / 50.0) * ridges;
        }

        // Раздробленная земля ветреных холмов: у оригинала при эрозии около
        // 0.5 (уровень 5) рельеф поднят и изломан сильнее соседей — отсюда
        // в поясе высот 70–99 основная доля крутых ступеней (замер: 4–6%
        // пар у ветреных холмов против долей процента у равнин).
        let shattered = (1.0 - ((climate.erosion - 0.5) / 0.12).powi(2)).max(0.0) * far_inland;

        ground += 16.0 * shattered * (0.4 + 0.6 * ridges.max(0.0));

        let mut surface = sea_floor + (ground - sea_floor) * land;

        // Насколько место далеко от воды: у берега раскачка стихает, иначе
        // из воды торчали бы одинокие блоки.
        let inland = smooth_step((surface - SEA as f64 - 2.0) / 10.0);
        let hills = smooth_step((spread - 8.0) / 20.0) * (1.0 - mountains);
        let wobble = 1.5 + (3.5 + 10.0 * hills + 6.0 * mountains + 6.0 * shattered) * inland * land;
        let rugged = 30.0 * shattered * inland * land;

        // Океан остаётся океаном: дно не выходит к поверхности. У берега
        // потолок плавно уходит вверх.
        let mut cap = SEA as f64 - 3.0 + 200.0 * land;

        // Русло реки: долина опускается к воде плавно, склонами, а в самом
        // русле потолок не даёт раскачке нависать над водой.
        let valley = smooth_step((-climate.peaks_valleys() - 0.7) / 0.2) * land;

        if valley > 0.0 {
            let depth = ((-climate.peaks_valleys() - 0.85) / 0.15).clamp(0.0, 1.0);
            let river = SEA as f64 - 2.0 - depth * 2.0;

            surface += (surface.min(river) - surface) * valley;
            cap = cap.min(river + (1.0 - valley) * 60.0);
        }

        Relief {
            surface,
            wobble,
            rugged,
            cap,
        }
    }

    /// Мягкий рельеф «Супер плавности»: основной уровень от континентальности,
    /// размах от эрозии, рельеф поверх — от гребней и долин.
    fn smooth_height(&self, climate: &Climate, x: i32, z: i32) -> i32 {
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

    /// Что стоит в этом месте столбца — для проверок: сам выясняет, камень
    /// ли тут и как глубоко от поверхности. Чанк знает это и так и зовёт
    /// `block_with`.
    #[cfg(test)]
    pub fn block_at(&self, column: &Column, x: i32, y: i32, z: i32) -> i32 {
        let grid = match self.style {
            Style::Vanilla => Some(self.relief_grid(x, z, x, z)),
            Style::Smooth => None,
        };
        let solid = |y: i32| self.solid_in(grid.as_ref(), column, x, y, z);
        let mut run_top = y;

        while run_top < y + 16 && solid(run_top + 1) {
            run_top += 1;
        }

        let caves = self.cave_grid(x, z, x, z, y);

        self.block_with(column, &caves, x, y, z, solid(y), run_top)
    }

    /// Что стоит в этом месте столбца. `solid` — камень ли тут по рельефу,
    /// `run_top` — верх сплошной толщи, в которой лежит блок: от него
    /// считается глубина для травы, земли и песка. Так и уступ под
    /// нависанием получает свою траву.
    pub fn block_with(
        &self,
        column: &Column,
        caves: &CaveGrid,
        x: i32,
        y: i32,
        z: i32,
        solid: bool,
        run_top: i32,
    ) -> i32 {
        if y < BOTTOM || y >= crate::world::MIN_Y + crate::world::WORLD_HEIGHT {
            return AIR;
        }

        // Дно мира: сплошной слой, а над ним ещё несколько с прорехами —
        // так у оригинала, и так его не пробить.
        if y == BOTTOM {
            return common().bedrock;
        }

        if y <= BOTTOM + 4 && self.bedrock_at(x, y, z) {
            return common().bedrock;
        }

        if !solid {
            // Над землёй: до уровня моря вода, выше — воздух. В холодных
            // местах вода сверху схвачена льдом.
            if y <= SEA && column.height < SEA {
                return if y == SEA && column.biome.is_freezing() {
                    common().ice
                } else {
                    common().water
                };
            }

            // На самой земле растёт трава и лежит снег.
            if y > column.height && column.height >= SEA {
                if let Some(plant) = self.plant_at(column, x, y, z) {
                    return named(plant);
                }
            }

            return AIR;
        }

        let depth = run_top - y;

        // Пещеры проедают камень, но не подходят к самой поверхности и не
        // трогают дно мира.
        if depth > CAVE_ROOF && y > BOTTOM + 5 && caves.hollow(x, y, z, depth) {
            return match self.aquifer_at(x, y, z) {
                Fluid::Air => AIR,
                Fluid::Water => common().water,
                Fluid::Lava => common().lava,
                Fluid::Barrier => self.underground_at(column, x, y, z),
            };
        }

        let surface_column = Column {
            height: run_top,
            ..*column
        };

        match surface(&surface_column, depth) {
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

        // Цветы растут полянками, а не ровной россыпью: у оригинала они
        // сбиваются в пятна, между которыми их нет вовсе. Поляной считается
        // каждая четвёртая клетка 8×8; в цветочном лесу цветы всюду.
        let meadow = biome == Biome::FlowerForest || chance(x >> 3, z >> 3, 913, 0.25);

        if meadow && chance(x, z, 907, flowers) {
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
    ///
    /// Сами шумы, как у оригинала, берутся в узлах сетки 4×8×4 и плавно
    /// тянутся между ними (`CaveGrid`): считать их для каждого блока
    /// в несколько раз дороже, а пещеры от этого не меняются.
    fn cave_noise(&self, x: i32, y: i32, z: i32) -> [f64; 3] {
        let (fx, fy, fz) = (x as f64, y as f64, z as f64);

        [
            self.tunnels.at3(fx / 140.0, fy / 90.0, fz / 140.0),
            self.tunnels_across.at3(fx / 140.0, fy / 90.0, fz / 140.0),
            self.caves.octaves3(fx / 70.0, fy / 45.0, fz / 70.0, 2),
        ]
    }

    /// Сетка шумов пещер над участком — от дна мира до `y_to`.
    pub fn cave_grid(&self, x_from: i32, z_from: i32, x_to: i32, z_to: i32, y_to: i32) -> CaveGrid {
        let x0 = x_from.div_euclid(CELL_WIDTH) * CELL_WIDTH;
        let z0 = z_from.div_euclid(CELL_WIDTH) * CELL_WIDTH;
        let across_x = (x_to.div_euclid(CELL_WIDTH) * CELL_WIDTH - x0) / CELL_WIDTH + 2;
        let across_z = (z_to.div_euclid(CELL_WIDTH) * CELL_WIDTH - z0) / CELL_WIDTH + 2;
        let y0 = BOTTOM.div_euclid(CELL_HEIGHT) * CELL_HEIGHT;
        let layers = (y_to.max(y0).div_euclid(CELL_HEIGHT) * CELL_HEIGHT - y0) / CELL_HEIGHT + 2;
        let mut nodes = Vec::with_capacity((across_x * across_z * layers) as usize);

        for cx in 0..across_x {
            for cz in 0..across_z {
                for layer in 0..layers {
                    nodes.push(self.cave_noise(
                        x0 + cx * CELL_WIDTH,
                        y0 + layer * CELL_HEIGHT,
                        z0 + cz * CELL_WIDTH,
                    ));
                }
            }
        }

        CaveGrid {
            x0,
            z0,
            across_z,
            y0,
            layers,
            nodes,
        }
    }

    /// Чем заполнена пещера в этом месте: воздухом, водой или лавой.
    ///
    /// Как у оригинала, толща поделена на клетки 16×12×16, у каждой клетки
    /// своя середина (сдвинутая случайно) и свой уровень жидкости — или она
    /// сухая. Место принадлежит ближайшей середине; где две соседние клетки
    /// с разной жидкостью сходятся вплотную, остаётся каменная перемычка,
    /// чтобы вода не висела стеной над воздухом и не встречалась с лавой.
    /// Ниже −55 любая пустота — лава.
    fn aquifer_at(&self, x: i32, y: i32, z: i32) -> Fluid {
        if y <= LAVA_LEVEL {
            return Fluid::Lava;
        }

        let (cell_x, cell_y, cell_z) = (x.div_euclid(16), y.div_euclid(12), z.div_euclid(16));
        let mut nearest = (f64::MAX, (0, 0, 0));
        let mut second = (f64::MAX, (0, 0, 0));

        for dx in -1..=1 {
            for dy in -1..=1 {
                for dz in -1..=1 {
                    let cell = (cell_x + dx, cell_y + dy, cell_z + dz);
                    let (mx, my, mz) = aquifer_middle(cell);
                    let distance =
                        (mx - x as f64).powi(2) + (my - y as f64).powi(2) + (mz - z as f64).powi(2);

                    if distance < nearest.0 {
                        second = nearest;
                        nearest = (distance, cell);
                    } else if distance < second.0 {
                        second = (distance, cell);
                    }
                }
            }
        }

        let own = self.aquifer_fill(nearest.1);
        let other = self.aquifer_fill(second.1);
        let wet = |fill: Option<(i32, Fluid)>| {
            fill.filter(|(level, _)| y <= *level)
                .map(|(_, fluid)| fluid)
        };

        // Перемычка: на стыке двух клеток, если на этой высоте у них разное
        // — у одной вода, у другой воздух или лава.
        let gap = second.0.sqrt() - nearest.0.sqrt();

        if gap < 1.0 && wet(own) != wet(other) {
            return Fluid::Barrier;
        }

        wet(own).unwrap_or(Fluid::Air)
    }

    /// Уровень и род жидкости клетки водоёмов, `None` — клетка сухая.
    ///
    /// Залитость решает плавный шум: залитые клетки идут соседями, озёрами,
    /// а не вразброс. Чем глубже, тем реже; ниже −10 часть водоёмов —
    /// лавовые. Уровень тоже от плавного шума — у соседних клеток он часто
    /// один, и водоём выходит цельным.
    fn aquifer_fill(&self, cell: (i32, i32, i32)) -> Option<(i32, Fluid)> {
        let (cx, cy, cz) = (cell.0 as f64, cell.1 as f64, cell.2 as f64);
        let middle = cell.1 * 12 + 6;

        // Выше уровня моря водоёмов не бывает: вода там ушла бы наружу.
        if middle > SEA - 4 {
            return None;
        }

        // Пороги подобраны по замерам оригинала: выше −10 залита примерно
        // четверть пещер, глубже — считанные проценты.
        let threshold = if middle >= 0 {
            0.0
        } else if middle >= -40 {
            0.33
        } else {
            0.22
        };
        let flood = self.flood.at3(cx / 3.0, cy / 2.0, cz / 3.0);

        if flood < threshold {
            return None;
        }

        let level = middle + (self.flood_level.at(cx / 4.0, cz / 4.0) * 10.0).round() as i32;
        let level = level.min(SEA - 2);
        // Доля лавовых — по замерам оригинала: треть на средней глубине,
        // десятая часть у самого низа.
        let lava_part = if middle < -40 {
            1
        } else if middle < 0 {
            4
        } else {
            0
        };
        let lava = Random::new(cell_seed(cell) ^ 0x6C_A1).next() % 10 < lava_part;

        Some((level, if lava { Fluid::Lava } else { Fluid::Water }))
    }

    /// Что лежит под землёй в этом месте: камень, а ниже нуля — глубинный
    /// сланец. Руда и пятна гранита, гравия и прочего кладутся потом,
    /// отдельным проходом по чанку (`ore_blocks_in`): гнёзда пересекают
    /// границы столбцов, и по одному столбцу их не разложить.
    fn underground_at(&self, _column: &Column, _x: i32, y: i32, _z: i32) -> i32 {
        if y < DEEPSLATE_FROM {
            common().deepslate
        } else {
            common().stone
        }
    }

    /// Блоки гнёзд, попавшие в этот чанк: где лежит, чем, и по какому
    /// размещению (оттуда берётся шанс пропуска у воздуха).
    ///
    /// Гнездо может начаться в соседнем чанке и зайти в этот, поэтому
    /// перебираются попытки своего чанка и восьми соседних; из каждого
    /// гнезда берётся только то, что попало внутрь. Соседний чанк посчитает
    /// те же гнёзда тем же образом и заберёт свою часть — договариваться им
    /// не о чем.
    ///
    /// Подходит ли место (камень ли там, нет ли рядом воздуха) — решает мир:
    /// он знает, что уже стоит в чанке.
    ///
    /// `highest` — верх камня в чанке: гнездо, целиком лежащее выше, в чанк
    /// всё равно не ляжет (руда кладётся только в камень), и растить его
    /// незачем. Таких много: железо ставит 90 гнёзд на чанк на высотах
    /// 112–384, изумруд в горах — 100.
    pub fn ore_blocks_in(
        &self,
        chunk_x: i32,
        chunk_z: i32,
        highest: i32,
    ) -> Vec<(i32, i32, i32, &'static OrePlacement)> {
        let mut found = Vec::new();

        let inside = |x: i32, z: i32| x.div_euclid(16) == chunk_x && z.div_euclid(16) == chunk_z;

        for from_x in (chunk_x - 1)..=(chunk_x + 1) {
            for from_z in (chunk_z - 1)..=(chunk_z + 1) {
                // Биом решает только для горного изумруда и золота бесплодных
                // земель; берём его один раз — по середине чанка.
                let biome = self.column_at(from_x * 16 + 8, from_z * 16 + 8).biome;

                for (index, placement) in ORE_PLACEMENTS.iter().enumerate() {
                    let allowed = match placement.biomes {
                        OreBiomes::All => true,
                        OreBiomes::Mountains => is_mountain(biome),
                        OreBiomes::Badlands => matches!(
                            biome,
                            Biome::Badlands | Biome::ErodedBadlands | Biome::WoodedBadlands
                        ),
                    };

                    if !allowed {
                        continue;
                    }

                    let mut random = Random::new(
                        (from_x as i64)
                            .wrapping_mul(341_873_128_712)
                            .wrapping_add((from_z as i64).wrapping_mul(132_897_987_541))
                            .wrapping_add(index as i64 * 7_919)
                            ^ self.seed,
                    );

                    // Дробные попытки — это шанс: 1/6 значит «в одном чанке
                    // из шести».
                    let whole = placement.tries.floor() as u32;
                    let part = placement.tries - whole as f64;
                    let tries = whole + u32::from(unit(&mut random) < part);

                    let reach = blob_reach(placement.size);

                    for _ in 0..tries {
                        let x = from_x * 16 + (random.next() % 16) as i32;
                        let z = from_z * 16 + (random.next() % 16) as i32;
                        let y = height_in(&mut random, placement);

                        // Гнездо дальше своего охвата от этого чанка сюда не
                        // дотянется — не тратим на него время. Случайные
                        // числа при этом уже вынуты, так что соседям это
                        // ничего не сдвигает.
                        let near_x = x.clamp(chunk_x * 16, chunk_x * 16 + 15);
                        let near_z = z.clamp(chunk_z * 16, chunk_z * 16 + 15);
                        let seed = random.next();

                        if (x - near_x).abs() > reach || (z - near_z).abs() > reach || y - reach > highest {
                            continue;
                        }

                        // Треугольник у глубинных руд уходит под дно мира:
                        // что оказалось ниже дна, просто не ложится.
                        let in_world = |by: i32| (BOTTOM..crate::world::MIN_Y + crate::world::WORLD_HEIGHT).contains(&by);

                        match Lump::new(seed, (x, y, z), placement.size) {
                            // Крупное гнездо проверяется поблочно, и чанк
                            // перебирает только свою часть его объёма.
                            Some(lump) => {
                                let (low, high) = lump.bounds();
                                let (from_x, to_x) = (low.0.max(chunk_x * 16), high.0.min(chunk_x * 16 + 15));
                                let (from_z, to_z) = (low.2.max(chunk_z * 16), high.2.min(chunk_z * 16 + 15));

                                for bx in from_x..=to_x {
                                    for bz in from_z..=to_z {
                                        for by in low.1..=high.1 {
                                            if in_world(by) && lump.has(bx, by, bz) {
                                                found.push((bx, by, bz, placement));
                                            }
                                        }
                                    }
                                }
                            }
                            None => {
                                for (bx, by, bz) in blob(seed, (x, y, z), placement.size) {
                                    if inside(bx, bz) && in_world(by) {
                                        found.push((bx, by, bz, placement));
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        found
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

/// Состояние блока по имени — для тех, кто складывает деревья.
pub fn block_named(name: &str) -> i32 {
    named(name)
}

/// Выпало ли событие с такой вероятностью в этой точке объёма: своё число
/// для каждой точки, одно и то же при каждом обращении.
pub fn chance_at(x: i32, y: i32, z: i32, part: f64) -> bool {
    chance(x, z.wrapping_add(y.wrapping_mul(1_619)), 4_243, part)
}

/// Выпало ли редкое событие в этом месте: своё число для каждой точки,
/// одно и то же при каждом обращении.
pub(super) fn chance(x: i32, z: i32, salt: i64, part: f64) -> bool {
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
pub fn is_mountain(biome: Biome) -> bool {
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

/// Случайное число от 0 до 1.
fn unit(random: &mut Random) -> f64 {
    (random.next() % 1_000_000) as f64 / 1_000_000.0
}

/// Высота начала гнезда: ровно по промежутку или треугольником — тогда
/// чаще всего в середине (сумма двух ровных дает треугольник).
fn height_in(random: &mut Random, placement: &OrePlacement) -> i32 {
    let span = (placement.high - placement.low).max(0) as u64;

    if placement.triangle {
        let half = span / 2;
        let first = random.next() % (half + 1);
        let second = random.next() % (span - half + 1);

        placement.low + (first + second) as i32
    } else {
        placement.low + (random.next() % (span + 1)) as i32
    }
}

/// Насколько далеко от своего начала может уйти гнездо такого размера.
fn blob_reach(size: u32) -> i32 {
    (blob_blocks(size) as f64).cbrt().ceil() as i32 + 1
}

/// Крупное гнездо — ком из трёх шаров вдоль короткого отрезка, с неровным
/// краем. Объём — то же число блоков, что набрало бы гнездо, растущее от
/// точки (`blob`), но принадлежность блока кому считается по самому блоку.
/// Поэтому чанк, куда ком заходит краем, перебирает только свою часть, а не
/// выращивает весь ком заново: у гранита, диорита, андезита, туфа, земли
/// и гравия гнёзда до 864 блоков, и соседние чанки иначе пересчитывали бы
/// их по девять раз.
struct Lump {
    /// Середины шаров.
    centers: [(f64, f64, f64); 3],
    radius: f64,
    seed: u64,
    reach: i32,
    start: (i32, i32, i32),
}

impl Lump {
    /// Ком для гнезда этого размера; `None` — гнездо мелкое, ему хватает
    /// обычного роста от точки.
    fn new(seed: u64, start: (i32, i32, i32), size: u32) -> Option<Lump> {
        let most = blob_blocks(size);

        if most < 24 {
            return None;
        }

        // Число блоков — как у `blob`: от половины до наибольшего.
        let mut random = Random::new(seed as i64);
        let (from, to) = (most / 2, most);
        let count = (from + (random.next() % (to - from + 1) as u64) as u32) as f64;

        // Отрезок в случайную сторону, длиной около размера кома.
        let length = 0.8 * count.cbrt();
        let mut direction = [0.0; 3];

        for part in direction.iter_mut() {
            *part = (random.next() % 2001) as f64 / 1000.0 - 1.0;
        }

        let norm = direction.iter().map(|part| part * part).sum::<f64>().sqrt().max(1e-6);
        let half = length / 2.0;
        let at = |t: f64| {
            (
                start.0 as f64 + 0.5 + direction[0] / norm * t,
                start.1 as f64 + 0.5 + direction[1] / norm * t,
                start.2 as f64 + 0.5 + direction[2] / norm * t,
            )
        };

        // Радиус — чтобы объём капсулы (шар плюс цилиндр по отрезку) был
        // равен числу блоков: πr²(4r/3 + L) = count, решается подбором.
        let mut radius = count.cbrt() / 2.0;

        for _ in 0..8 {
            let volume = std::f64::consts::PI * radius * radius * (4.0 * radius / 3.0 + length);
            radius *= (count / volume).cbrt();
        }

        Some(Lump {
            centers: [at(-half), at(0.0), at(half)],
            radius,
            seed: random.next(),
            reach: blob_reach(size),
            start,
        })
    }

    /// Какие блоки могут попасть в ком: не дальше охвата гнезда от начала.
    fn bounds(&self) -> ((i32, i32, i32), (i32, i32, i32)) {
        let extent = (self.radius + 0.8 * 0.5 * (self.radius * 3.0)).ceil() as i32;
        let extent = extent.min(self.reach);
        let (x, y, z) = self.start;

        ((x - extent, y - extent, z - extent), (x + extent, y + extent, z + extent))
    }

    /// Лежит ли блок в коме. Край неровный: у самой границы блок берётся
    /// или нет по своему числу — ком выходит бугристым, а не гладким шаром.
    fn has(&self, x: i32, y: i32, z: i32) -> bool {
        let (fx, fy, fz) = (x as f64 + 0.5, y as f64 + 0.5, z as f64 + 0.5);
        let distance = self
            .centers
            .iter()
            .map(|&(cx, cy, cz)| ((fx - cx).powi(2) + (fy - cy).powi(2) + (fz - cz).powi(2)).sqrt())
            .fold(f64::MAX, f64::min);
        let edge = distance - self.radius;

        if edge < -0.7 {
            return true;
        }

        if edge > 0.7 {
            return false;
        }

        let mixed = (x as i64)
            .wrapping_mul(341_873_128_712)
            .wrapping_add((y as i64).wrapping_mul(1_640_531_527))
            .wrapping_add((z as i64).wrapping_mul(132_897_987_541))
            ^ self.seed as i64;
        let roll = (Random::new(mixed).next() % 1000) as f64 / 1000.0;

        roll > (edge + 0.7) / 1.4
    }
}

/// Блоки одного гнезда.
///
/// Гнездо растёт от начальной точки: каждый следующий блок прирастает
/// к одному из уже положенных с какой-нибудь стороны. Так получается
/// плотный неровный ком, а не шар и не змейка. Число блоков — от половины
/// до наибольшего для этого размера по таблице вики: гнёзда в игре редко
/// добирают до наибольшего. От начала гнездо не уходит дальше своего охвата.
fn blob(seed: u64, start: (i32, i32, i32), size: u32) -> Vec<(i32, i32, i32)> {
    let most = blob_blocks(size);

    if most == 0 {
        return Vec::new();
    }

    let mut random = Random::new(seed as i64);

    // Сколько блоков набирает гнездо. Крупные у оригинала добирают от
    // половины до наибольшего; мелкие (размер до 9) — заметно меньше: по
    // замерам ванильного мира руды размером до 9 у нас выходило в полтора
    // раза больше, а уголь и медь сходились.
    let (from, to) = if size <= 9 {
        (most * 3 / 10, most * 7 / 10)
    } else {
        (most / 2, most)
    };
    let count = (from + (random.next() % (to - from + 1) as u64) as u32).max(1);
    let reach = blob_reach(size);

    // Сетка вокруг начала: заняло ли место уже. Своя у потока и общая для
    // всех гнёзд: вместо очистки у каждого гнезда своя метка, и занятым
    // считается место с меткой этого гнезда. Выделять память на каждое из
    // тысяч гнёзд чанка слишком дорого.
    thread_local! {
        static TAKEN: std::cell::RefCell<(Vec<u32>, u32)> = const { std::cell::RefCell::new((Vec::new(), 0)) };
    }

    let side = (reach * 2 + 1) as usize;
    let cell = |x: i32, y: i32, z: i32| -> Option<usize> {
        let (dx, dy, dz) = (
            x - start.0 + reach,
            y - start.1 + reach,
            z - start.2 + reach,
        );

        if dx < 0
            || dy < 0
            || dz < 0
            || dx as usize >= side
            || dy as usize >= side
            || dz as usize >= side
        {
            None
        } else {
            Some((dx as usize * side + dy as usize) * side + dz as usize)
        }
    };

    let mut blocks = Vec::with_capacity(count as usize);
    blocks.push(start);

    TAKEN.with_borrow_mut(|(taken, mark)| {
        if taken.len() < side * side * side {
            taken.resize(side * side * side, 0);
        }

        *mark = mark.wrapping_add(1);

        if *mark == 0 {
            taken.fill(0);
            *mark = 1;
        }

        let mark = *mark;

        if let Some(at) = cell(start.0, start.1, start.2) {
            taken[at] = mark;
        }

        // Попыток больше, чем блоков: часть упрётся в занятое или в край охвата.
        let mut tries = count * 6;

        while (blocks.len() as u32) < count && tries > 0 {
            tries -= 1;

            let (x, y, z) = blocks[(random.next() % blocks.len() as u64) as usize];
            let (dx, dy, dz) = match random.next() % 6 {
                0 => (1, 0, 0),
                1 => (-1, 0, 0),
                2 => (0, 1, 0),
                3 => (0, -1, 0),
                4 => (0, 0, 1),
                _ => (0, 0, -1),
            };
            let next = (x + dx, y + dy, z + dz);

            if let Some(at) = cell(next.0, next.1, next.2)
                && taken[at] != mark
            {
                taken[at] = mark;
                blocks.push(next);
            }
        }
    });

    blocks
}

/// Где разрешено класть гнездо.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OreBiomes {
    All,
    /// Только горы — так растёт изумруд.
    Mountains,
    /// Только бесплодные земли — там лишнее золото.
    Badlands,
}

/// Одно размещение гнёзд со страницы «Ore (feature)» на вики. У иной руды
/// их несколько: у железа, например, три — с разными высотами и числом
/// попыток.
#[derive(Clone, Copy, Debug)]
pub struct OrePlacement {
    /// Что кладётся. Для руды — обычный вид; ниже нуля, в глубинном сланце,
    /// берётся разновидность `deepslate_…`.
    pub block: &'static str,
    /// Есть ли у блока глубинная разновидность.
    pub has_deep: bool,
    /// «Spawn size» с вики.
    pub size: u32,
    /// Попыток на чанк. Дробное — это шанс одной попытки (1/6 и т.п.).
    pub tries: f64,
    pub low: i32,
    pub high: i32,
    /// Треугольное распределение: чаще всего в середине промежутка.
    /// Иначе — ровное.
    pub triangle: bool,
    /// Доля блоков, пропускаемых, если рядом воздух («Skipped when air
    /// exposed»): решается для каждого блока отдельно.
    pub air_skip: f64,
    pub biomes: OreBiomes,
}

/// Размещения верхнего мира — таблица со страницы «Ore (feature)» на вики,
/// строка в строку. Пропущены только те, что привязаны к биомам, которых у
/// нас ещё нет: глина и крупная медь пещер с капельниками, лишний уголь
/// предгорий, заражённый камень.
pub const ORE_PLACEMENTS: [OrePlacement; 27] = {
    use OreBiomes::*;

    const fn ore(
        block: &'static str,
        size: u32,
        tries: f64,
        low: i32,
        high: i32,
        triangle: bool,
        air_skip: f64,
        biomes: OreBiomes,
    ) -> OrePlacement {
        OrePlacement {
            block,
            has_deep: true,
            size,
            tries,
            low,
            high,
            triangle,
            air_skip,
            biomes,
        }
    }

    const fn rock(block: &'static str, size: u32, tries: f64, low: i32, high: i32) -> OrePlacement {
        OrePlacement {
            block,
            has_deep: false,
            size,
            tries,
            low,
            high,
            triangle: false,
            air_skip: 0.0,
            biomes: All,
        }
    }

    [
        rock("dirt", 33, 7.0, 0, 160),
        rock("gravel", 33, 14.0, -64, 320),
        rock("granite", 64, 2.0, 0, 60),
        rock("granite", 64, 1.0 / 6.0, 64, 128),
        rock("diorite", 64, 2.0, 0, 60),
        rock("diorite", 64, 1.0 / 6.0, 64, 128),
        rock("andesite", 64, 2.0, 0, 60),
        rock("andesite", 64, 1.0 / 6.0, 64, 128),
        rock("tuff", 64, 2.0, -64, 0),
        ore("coal_ore", 17, 20.0, 0, 192, true, 0.5, All),
        ore("coal_ore", 17, 30.0, 136, 320, false, 0.0, All),
        ore("iron_ore", 4, 10.0, -64, 72, false, 0.0, All),
        ore("iron_ore", 9, 10.0, -24, 56, true, 0.0, All),
        ore("iron_ore", 9, 90.0, 80, 384, true, 0.0, All),
        ore("copper_ore", 10, 16.0, -16, 112, true, 0.0, All),
        ore("redstone_ore", 8, 4.0, -64, 15, false, 0.0, All),
        // Треугольник уходит ниже дна мира: на вики высоты в таблице обрезаны
        // по дну, но по данным настоящей генерации («Ore», строка «Most found
        // in layers») больше всего редстоуна и алмаза на слое −59, у самого
        // дна. Значит, пик треугольника — на −64, и половина попыток падает
        // за мир.
        ore("redstone_ore", 8, 8.0, -96, -32, true, 0.0, All),
        ore("lapis_ore", 7, 2.0, -32, 32, true, 0.0, All),
        ore("lapis_ore", 7, 4.0, -64, 64, false, 1.0, All),
        ore("gold_ore", 9, 4.0, -64, 32, true, 0.5, All),
        ore("gold_ore", 9, 0.5, -64, -48, false, 0.5, All),
        ore("gold_ore", 9, 50.0, 32, 256, false, 0.0, Badlands),
        ore("diamond_ore", 4, 7.0, -144, 16, true, 0.5, All),
        ore("diamond_ore", 8, 4.0, -144, 16, true, 1.0, All),
        ore("diamond_ore", 12, 1.0 / 9.0, -144, 16, true, 0.7, All),
        ore("diamond_ore", 8, 2.0, -64, -4, false, 0.5, All),
        ore("emerald_ore", 3, 100.0, -16, 480, true, 0.0, Mountains),
    ]
};

/// Сколько блоков самое большее в гнезде такого размера — таблица со
/// страницы «Ore (feature)» на вики (Java Edition).
pub fn blob_blocks(size: u32) -> u32 {
    const MOST: [u32; 65] = [
        0, 0, 0, 4, 5, 8, 9, 10, 10, 13, 16, 17, 23, 24, 24, 29, 32, 37, 46, 52, 52, 60, 68, 68,
        74, 82, 94, 104, 106, 120, 128, 135, 149, 160, 180, 190, 204, 212, 228, 246, 262, 276, 292,
        308, 324, 344, 360, 381, 403, 429, 452, 480, 500, 530, 558, 584, 616, 634, 664, 694, 730,
        760, 790, 826, 864,
    ];

    MOST[(size as usize).min(64)]
}

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

    // Крутой склон в горах оголён до камня — так у оригинала горы каменные
    // по склонам и зелёные или снежные на полках.
    if column.steep && biome.bares_steep_slopes() {
        return None;
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

/// Чем заполнено место пещеры.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Fluid {
    Air,
    Water,
    Lava,
    /// Каменная перемычка между разными водоёмами.
    Barrier,
}

/// Своё число клетки водоёмов.
fn cell_seed((x, y, z): (i32, i32, i32)) -> i64 {
    (x as i64)
        .wrapping_mul(341_873_128_712)
        .wrapping_add((y as i64).wrapping_mul(1_640_531_527))
        .wrapping_add((z as i64).wrapping_mul(132_897_987_541))
}

/// Середина клетки водоёмов — случайно сдвинутая внутри клетки, чтобы
/// границы водоёмов не шли по сетке.
fn aquifer_middle(cell: (i32, i32, i32)) -> (f64, f64, f64) {
    let mut random = Random::new(cell_seed(cell));
    let mut part = |size: i32| (random.next() % size as u64) as f64;

    (
        (cell.0 * 16) as f64 + part(16),
        (cell.1 * 12) as f64 + part(12),
        (cell.2 * 16) as f64 + part(16),
    )
}

/// Плавная ступенька от 0 до 1: ниже нуля — 0, выше единицы — 1, между ними
/// без изломов.
fn smooth_step(t: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);

    t * t * (3.0 - 2.0 * t)
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
/// Частые блоки генерации — их номера находятся один раз: искать номер
/// по имени для каждого из сотни тысяч блоков чанка слишком дорого.
struct Common {
    stone: i32,
    deepslate: i32,
    water: i32,
    lava: i32,
    ice: i32,
    bedrock: i32,
}

/// Номера частых блоков.
fn common() -> &'static Common {
    static COMMON: std::sync::OnceLock<Common> = std::sync::OnceLock::new();

    COMMON.get_or_init(|| Common {
        stone: named("stone"),
        deepslate: named("deepslate"),
        water: named("water"),
        lava: named("lava"),
        ice: named("ice"),
        bedrock: named("bedrock"),
    })
}

fn named(name: &str) -> i32 {
    blocks::state_by_name(name).unwrap_or_else(|| panic!("блок {} есть в таблице", name))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Мир повторяется: одно семя — один и тот же рельеф.
    #[test]
    fn the_same_seed_gives_the_same_world() {
        let one = Terrain::smooth(1234);
        let same = Terrain::smooth(1234);
        let other = Terrain::smooth(1235);

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
        let terrain = Terrain::smooth(99);

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
        let terrain = Terrain::smooth(77);
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
        let terrain = Terrain::smooth(8);
        let column = terrain.column_at(500, 500);

        assert_eq!(terrain.block_at(&column, 500, column.height - 12, 500), named("stone"));
        assert_eq!(terrain.block_at(&column, 500, -20, 500), named("deepslate"));
    }

    /// Дно мира сплошное, а над ним бедрок с прорехами.
    #[test]
    fn the_bottom_is_solid_bedrock() {
        let terrain = Terrain::smooth(5);
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
        let terrain = Terrain::smooth(2024);
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
        let terrain = Terrain::smooth(31337);

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

    /// Гнёзда повторяются от семени, лежат в своём промежутке высот (с поправкой
    /// на размер гнезда) и попадают только в свой чанк.
    #[test]
    fn ore_blobs_stay_where_the_wiki_puts_them() {
        let terrain = Terrain::smooth(42);
        let again = Terrain::smooth(42);

        for (chunk_x, chunk_z) in [(0, 0), (-3, 5), (12, -7)] {
            let found = terrain.ore_blocks_in(chunk_x, chunk_z, i32::MAX);

            assert!(!found.is_empty(), "в чанке нет ни одного гнезда");

            let repeat = again.ore_blocks_in(chunk_x, chunk_z, i32::MAX);
            assert_eq!(found.len(), repeat.len(), "гнёзда не повторяются от семени");

            for (x, y, z, placement) in found {
                assert_eq!(
                    (x.div_euclid(16), z.div_euclid(16)),
                    (chunk_x, chunk_z),
                    "блок вне чанка"
                );

                let reach = blob_reach(placement.size);

                assert!(
                    y >= placement.low.max(BOTTOM) - reach && y <= placement.high + reach,
                    "{} на высоте {} вне {}…{}",
                    placement.block,
                    y,
                    placement.low,
                    placement.high
                );
            }
        }
    }

    /// Гнездо не больше наибольшего по таблице вики и не меньше половины.
    #[test]
    fn a_blob_keeps_to_its_size() {
        for size in [3, 4, 8, 9, 17, 33, 64] {
            let most = blob_blocks(size) as usize;

            for seed in 0..20 {
                let blocks = blob(seed, (0, 0, 0), size);

                assert!(
                    blocks.len() <= most,
                    "размер {}: {} блоков",
                    size,
                    blocks.len()
                );
                let least = if size <= 9 { most * 3 / 10 } else { most / 2 };
                assert!(
                    blocks.len() >= least.max(1),
                    "размер {}: всего {} блоков",
                    size,
                    blocks.len()
                );

                let mut sorted = blocks.clone();
                sorted.sort_unstable();
                sorted.dedup();
                assert_eq!(sorted.len(), blocks.len(), "блок положен дважды");
            }
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
    /// Резкость рельефа, как её меряет робот у оригинала: сколько соседних
    /// столбцов отличаются больше чем на 3 и на 6 блоков. Три тех же места.
    /// `cargo test roughness -- --ignored --nocapture`.
    /// Рельеф по полосам высоты — ступеньки между соседями и нависания,
    /// как их меряет робот у оригинала (tools/research/terrain-measured.md):
    /// `cargo test --release relief_bands -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn relief_bands() {
        let terrain = Terrain::new(100_554_032_945_340, Style::Vanilla);

        // Места — те же биомы, что робот нашёл у оригинала через /locate
        // (tools/research/terrain-measured.md): для каждого — ближайшая
        // к началу мира точка этого биома. Шаг обхода не кратен масштабам
        // шумов: в узлах решётки шум Перлина равен нулю.
        let wanted = [
            Biome::Plains,
            Biome::SunflowerPlains,
            Biome::Meadow,
            Biome::WindsweptHills,
            Biome::WindsweptGravellyHills,
            Biome::JaggedPeaks,
            Biome::FrozenPeaks,
            Biome::StonyPeaks,
            Biome::Badlands,
            Biome::SavannaPlateau,
        ];
        let mut spots: Vec<(i32, i32)> = Vec::new();

        for x in (-8000..=8000).step_by(97) {
            for z in (-8000..=8000).step_by(89) {
                spots.push((x, z));
            }
        }

        spots.sort_unstable_by_key(|&(x, z)| (x as i64).pow(2) + (z as i64).pow(2));

        let mut places = Vec::new();

        for biome in wanted {
            match spots.iter().find(|&&(x, z)| terrain.column_at(x, z).biome == biome) {
                Some(&(x, z)) => places.push((x, z)),
                None => println!("{:?}: не нашлось в пределах 8000", biome),
            }
        }

        let band = |height: i32| match height {
            ..70 => 0,
            70..100 => 1,
            100..140 => 2,
            _ => 3,
        };
        let (mut pairs, mut over3, mut over6, mut most) =
            ([0u64; 4], [0u64; 4], [0u64; 4], [0i32; 4]);
        let (mut columns, mut overhangs, mut rock) = ([0u64; 4], [0u64; 4], [0u64; 4]);
        let mut highest = 0;

        for (cx, cz) in places {
            let climate = terrain.climate_at(cx, cz);
            let relief = terrain.relief_at(&climate, cx, cz);

            println!(
                "место {:?} у {} {}: C {:.2} E {:.2} PV {:.2} → поверхность {:.0}, раскачка {:.0}",
                terrain.column_at(cx, cz).biome,
                cx,
                cz,
                climate.continent,
                climate.erosion,
                climate.peaks_valleys(),
                relief.surface,
                relief.wobble
            );
            const SIZE: i32 = 112;
            let (x0, z0) = (cx - SIZE / 2, cz - SIZE / 2);
            let grid = terrain.relief_grid(x0, z0, x0 + SIZE - 1, z0 + SIZE - 1);
            let caves = terrain.cave_grid(x0, z0, x0 + SIZE - 1, z0 + SIZE - 1, 320);
            let mut heights = vec![0; (SIZE * SIZE) as usize];

            for dx in 0..SIZE {
                for dz in 0..SIZE {
                    let (x, z) = (x0 + dx, z0 + dz);
                    let top = grid.top(x, z);

                    heights[(dx * SIZE + dz) as usize] = top;
                    highest = highest.max(top);

                    // Под верхним камнем — пустота, а под ней снова камень,
                    // в пределах 30 блоков.
                    let open = |y: i32| {
                        !grid.solid(x, y, z)
                            || (top - y > CAVE_ROOF
                                && caves.hollow(x, y, z, top - y)
                                && terrain.aquifer_at(x, y, z) == Fluid::Air)
                    };
                    let gap = (top - 30..top).rev().position(open);
                    let hangs =
                        gap.is_some_and(|gap| (top - 30..top - 1 - gap as i32).any(|y| !open(y)));
                    let bare = (top - 30..top).rev().position(|y| !grid.solid(x, y, z));
                    let rock_hangs = bare.is_some_and(|gap| {
                        (top - 30..top - 1 - gap as i32).any(|y| grid.solid(x, y, z))
                    });

                    rock[band(top)] += u64::from(rock_hangs);

                    columns[band(top)] += 1;
                    overhangs[band(top)] += u64::from(hangs);
                }
            }

            let mut sorted = heights.clone();
            sorted.sort_unstable();
            let at = |part: f64| sorted[((sorted.len() - 1) as f64 * part) as usize];

            println!("    высоты p5/p25/p50/p75/p95/макс: {} / {} / {} / {} / {} / {}", at(0.05), at(0.25), at(0.5), at(0.75), at(0.95), at(1.0));
            let (mut place_pairs, mut place_over3) = (0u64, 0u64);

            for dx in 0..SIZE {
                for dz in 0..SIZE {
                    let here = heights[(dx * SIZE + dz) as usize];

                    for (nx, nz) in [(dx + 1, dz), (dx, dz + 1)] {
                        if nx >= SIZE || nz >= SIZE {
                            continue;
                        }

                        let there = heights[(nx * SIZE + nz) as usize];
                        let (step, index) = ((here - there).abs(), band(here.min(there)));

                        pairs[index] += 1;
                        place_pairs += 1;
                        place_over3 += u64::from(step > 3);
                        over3[index] += u64::from(step > 3);
                        over6[index] += u64::from(step > 6);
                        most[index] = most[index].max(step);
                    }
                }
            }

            println!("    ступенек >3 на месте: {:.2}%", 100.0 * place_over3 as f64 / place_pairs.max(1) as f64);
        }

        for (index, name) in ["ниже 70", "70-99", "100-139", "140+"].iter().enumerate() {
            let share = |count: u64, of: u64| 100.0 * count as f64 / of.max(1) as f64;

            println!(
                "{}: ступенек >3 {:.2}%, >6 {:.2}%, самая большая {}; нависаний {:.1}% (без пещер {:.1}%) из {} столбцов",
                name,
                share(over3[index], pairs[index]),
                share(over6[index], pairs[index]),
                most[index],
                share(overhangs[index], columns[index]),
                share(rock[index], columns[index]),
                columns[index]
            );
        }

        println!("самая высокая точка {}", highest);
        println!(
            "оригинал: ниже 70: 0.79% / 0.45% / 66, нависаний 8.8%; 70-99: 2.65% / 1.35% / 39, 13.2%;"
        );
        println!(
            "          100-139: 1.52% / 0.38% / 23, 18.1%; 140+: 1.01% / 0.32% / 15, 22.1%; самая высокая 182"
        );
    }

    /// Сколько пустот на разной глубине — сравнить с замером оригинала:
    /// `cargo test --release cave_bands -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn cave_bands() {
        let terrain = Terrain::new(100_554_032_945_340, Style::Vanilla);
        let bands = [
            ("ниже -54", -59, -55),
            ("-54..-40", -54, -40),
            ("-39..-10", -39, -10),
            ("выше -10", -9, 61),
        ];
        let mut hollow = [0u64; 4];
        let (mut water, mut lava) = ([0u64; 4], [0u64; 4]);

        for (cx, cz) in [(0, 0), (2000, 2000), (-2400, 1600)] {
            let caves = terrain.cave_grid(cx - 104, cz - 104, cx + 103, cz + 103, SEA);

            for x in cx - 104..cx + 104 {
                for z in cz - 104..cz + 104 {
                    let column = terrain.column_at(x, z);

                    for (index, (_, from, to)) in bands.iter().enumerate() {
                        for y in *from..=(*to).min(column.height - CAVE_ROOF - 1) {
                            if caves.hollow(x, y, z, column.height - y) {
                                match terrain.aquifer_at(x, y, z) {
                                    Fluid::Water => water[index] += 1,
                                    Fluid::Lava => lava[index] += 1,
                                    Fluid::Air => {}
                                    Fluid::Barrier => continue,
                                }

                                hollow[index] += 1;
                            }
                        }
                    }
                }
            }
        }

        for (index, (name, _, _)) in bands.iter().enumerate() {
            println!(
                "{}: {} (вода {}, лава {})",
                name, hollow[index], water[index], lava[index]
            );
        }

        println!(
            "оригинал: ниже -54: 10125 (лава 10121); -54..-40: 188709 (вода 5458, лава 496); -39..-10: 462929 (вода 18957, лава 9510); выше -10: 889944 (вода 216685, лава 657)"
        );
    }

    #[test]
    #[ignore]
    fn roughness() {
        for style in [Style::Vanilla, Style::Smooth] {
            let terrain = Terrain::new(100_554_032_945_340, style);
            let (mut pairs, mut over3, mut over6, mut most) = (0u64, 0u64, 0u64, 0);
            let mut islets = 0u64;
            let mut heights = Vec::new();

            for (cx, cz) in [(0, 0), (2000, 2000), (-2400, 1600)] {
                let size = 208;
                let mut row: Vec<Vec<i32>> = Vec::new();

                for dx in 0..size {
                    let mut line = Vec::new();

                    for dz in 0..size {
                        let height = terrain.column_at(cx - 104 + dx, cz - 104 + dz).height;

                        line.push(height);
                        heights.push(height);
                    }

                    row.push(line);
                }

                // Одинокий клочок суши среди воды: сам над морем, все четыре
                // соседа под ним.
                for dx in 1..size as usize - 1 {
                    for dz in 1..size as usize - 1 {
                        let dry = |x: usize, z: usize| row[x][z] >= SEA;

                        islets += u64::from(
                            dry(dx, dz)
                                && !dry(dx - 1, dz)
                                && !dry(dx + 1, dz)
                                && !dry(dx, dz - 1)
                                && !dry(dx, dz + 1),
                        );
                    }
                }

                for dx in 0..size as usize {
                    for dz in 0..size as usize {
                        for (nx, nz) in [(dx + 1, dz), (dx, dz + 1)] {
                            if nx >= size as usize || nz >= size as usize {
                                continue;
                            }

                            let step = (row[dx][dz] - row[nx][nz]).abs();

                            pairs += 1;
                            over3 += u64::from(step > 3);
                            over6 += u64::from(step > 6);
                            most = most.max(step);
                        }
                    }
                }
            }

            heights.sort_unstable();
            let at = |part: f64| heights[((heights.len() - 1) as f64 * part) as usize];

            println!(
                "{:?}: высоты {} / {} / {} / {} / {}; ступенек >3: {:.2}%, >6: {:.2}%, самая большая {}; одиноких клочков {}",
                style,
                at(0.0),
                at(0.25),
                at(0.5),
                at(0.75),
                at(1.0),
                100.0 * over3 as f64 / pairs as f64,
                100.0 * over6 as f64 / pairs as f64,
                most,
                islets
            );
        }

        println!(
            "оригинал: высоты 9 / 61 / 65 / 71 / 132; ступенек >3: 1.73%, >6: 0.56%, самая большая 39"
        );
    }

    #[test]
    #[ignore]
    fn what_the_world_is_made_of() {
        let terrain = Terrain::smooth(100_554_032_945_340);

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

        // Руду и пустоты считает разведка мира (`world::speed::underground`):
        // руда кладётся проходом по чанку, и видна только в сложенном чанке.

        let mut sorted: Vec<(&str, u32)> = biomes.into_iter().collect();
        sorted.sort_by_key(|(_, count)| std::cmp::Reverse(*count));

        println!("биомы:");
        for (name, count) in sorted {
            println!("  {:>6.2}%  {}", 100.0 * count as f64 / total as f64, name);
        }
    }
}
