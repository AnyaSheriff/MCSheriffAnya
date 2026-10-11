// Энд: из чего он сложен.
//
// Рельеф подогнан по замеру мира оригинала 26.1.2 (tools/research/end.md,
// tools/measure/end_census.py, end_islands.py, end_profile.py, end_main.py,
// end_biomes.py): 67 600 чанков вокруг центра, ±2048 блоков.
//
// — Главный остров: сплошной диск радиусом ~66 блоков (верх 61 в центре и
//   57 у края диска, низ 11 → 19), дальше рваная кайма до ~145 блоков.
// — Пустота: от ~150 до ~950 блоков ни одного блока.
// — Внешние острова: с ~950 блоков доля площади растёт до 0,40 к 1050 и
//   0,45 к 1200, дальше держится около 0,45. Острова огромные (сотни
//   блоков), плоские сверху (верх 60–61), с круглым краем: толщина растёт
//   от 4 у самого края до 47 в 60 блоках от края.
//
// Как это складывается. Остров — там, где шум «острова» выше порога своего
// расстояния (порог — квантиль распределения шума, поэтому доля площади
// сходится с замером, а не подбирается вручную). Насколько глубоко столбец
// от края острова, берётся из квантилей замеренного распределения
// «расстояние до края» (`EDGE`), а верх и низ столбца — из замеренного
// среднего профиля `PROFILE` плюс шум неровности. Так средний профиль и
// распределение толщин совпадают с замером; форма самих пятен — своя.
//
// Постройки главного острова (вики, End spike / Exit portal): десять
// обсидиановых столбов по кругу радиусом 42–43 с бедроком и огнём сверху,
// два самых коротких — в клетках из железных прутьев; подставка выходного
// портала в (0,0). Высоты столбов по местам перемешаны от семени (их набор —
// 76…103 через 3, радиус диска 2…5 — снят замером). Кристаллы Края —
// сущности, их у нас пока нет: столбы стоят без них.
//
// Хорус — простые деревья на внешних островах (высоты и ветви подогнаны
// по числу блоков на столбец, вид — грубее оригинала).
//
// Всё решается только по координатам и семени, поэтому граница чанков
// нигде не рвётся.
//
// Энд живёт в той же сетке высот, что остальные измерения (−64..320):
// объявлен клиенту с этими границами, а заполнен только от 0 до 255.

use std::sync::OnceLock;

use super::end_city;
use super::noise::Noise;
use super::terrain::Biome;
#[cfg(not(seedmap_core))]
use super::{biome_cell, pack, split, BiomeId, Chunk, AIR, BIOME_CELLS, BIOME_LAYERS, CHUNK_SIZE};
#[cfg(seedmap_core)]
use super::{AIR, CHUNK_SIZE};
use crate::blocks;

/// Клетка с обсидианом, куда приходит игрок из портала: центр площадки
/// 5×5 (вики, End portal: (100, 48, 0), игрок встаёт на (100, 49, 0)).
pub const PLATFORM: (i32, i32, i32) = (100, 48, 0);

/// Верх главного острова в центре: y самого верхнего эндемняка.
pub const CENTER_TOP: i32 = 61;

/// Радиус главного острова: дальше идут только отдельные куски каймы.
const MAIN_RADIUS: f64 = 66.0;

/// Расстояние, с которого начинаются внешние острова.
const OUTER_FROM: f64 = 945.0;

/// Вес мелких островков в шуме внешних островов.
const ISLETS: f64 = 0.8;

/// Доля площади, занятая внешними островами, по расстоянию от центра
/// (замер, кольца по 25 блоков).
fn outer_cover(distance: f64) -> f64 {
    match distance {
        d if d <= OUTER_FROM => 0.0,
        d if d < 1050.0 => (d - OUTER_FROM) * 0.003_81,
        d if d < 1200.0 => 0.40 + (d - 1050.0) * (0.05 / 150.0),
        _ => 0.45,
    }
}

/// Доля площади, занятая эндемняком главного острова и его каймы, по
/// расстоянию от центра (замер, кольца по 4 блока).
const MAIN_COVER: [(f64, f64); 24] = [
    (0.0, 1.0),
    (66.0, 0.99),
    (72.0, 0.99),
    (76.0, 0.86),
    (80.0, 0.76),
    (84.0, 0.64),
    (88.0, 0.60),
    (92.0, 0.54),
    (96.0, 0.47),
    (100.0, 0.43),
    (104.0, 0.40),
    (108.0, 0.33),
    (112.0, 0.26),
    (116.0, 0.20),
    (120.0, 0.14),
    (124.0, 0.12),
    (128.0, 0.11),
    (132.0, 0.09),
    (136.0, 0.07),
    (140.0, 0.05),
    (144.0, 0.01),
    (148.0, 0.0),
    (150.0, 0.0),
    (1.0e9, 0.0),
];

/// Верх и низ диска главного острова по расстоянию от центра (замер:
/// средние по кольцам).
const MAIN_DISC: [(f64, f64, f64); 8] = [
    (0.0, 61.0, 11.3),
    (12.0, 61.0, 12.3),
    (24.0, 60.3, 13.0),
    (36.0, 59.6, 14.2),
    (48.0, 58.6, 16.2),
    (56.0, 58.0, 17.6),
    (64.0, 57.3, 19.4),
    (68.0, 56.9, 21.0),
];

/// Квантили расстояния от столбца до края острова (в блоках, по четырём
/// соседям), через 1/40 распределения — по внешним островам.
const EDGE: [f64; 41] = [
    1.0, 1.0, 2.0, 3.0, 3.0, 4.0, 5.0, 6.0, 7.0, 9.0, 10.0, 11.0, 12.0, 13.0, 15.0, 16.0, 17.0, 18.0, 20.0, 21.0,
    23.0, 24.0, 25.0, 27.0, 29.0, 30.0, 32.0, 34.0, 36.0, 37.0, 40.0, 42.0, 44.0, 47.0, 50.0, 53.0, 56.0, 61.0,
    67.0, 76.0, 131.0,
];

/// Средний верх и низ столбца по расстоянию до края острова (замер).
const PROFILE: [(f64, f64, f64); 17] = [
    (1.0, 47.2, 43.0),
    (2.0, 49.1, 40.5),
    (3.0, 50.5, 38.4),
    (4.0, 51.5, 36.5),
    (5.0, 52.3, 34.5),
    (6.0, 52.9, 32.7),
    (8.0, 54.0, 29.6),
    (10.0, 54.9, 27.5),
    (12.0, 55.7, 25.8),
    (15.0, 56.5, 23.7),
    (20.0, 57.5, 21.1),
    (25.0, 58.2, 19.2),
    (30.0, 58.7, 17.9),
    (40.0, 59.6, 16.3),
    (50.0, 60.1, 15.3),
    (60.0, 60.7, 14.4),
    (62.0, 61.0, 14.0),
];

/// Линейная подстановка по таблице (первый столбец — по возрастанию).
fn lookup<const N: usize>(table: &[(f64, f64, f64); N], at: f64) -> (f64, f64) {
    if at <= table[0].0 {
        return (table[0].1, table[0].2);
    }

    for pair in table.windows(2) {
        if at <= pair[1].0 {
            let part = (at - pair[0].0) / (pair[1].0 - pair[0].0);
            return (pair[0].1 + (pair[1].1 - pair[0].1) * part, pair[0].2 + (pair[1].2 - pair[0].2) * part);
        }
    }

    let last = table[N - 1];
    (last.1, last.2)
}

/// Доля главного острова на этом расстоянии.
fn main_cover(distance: f64) -> f64 {
    let (from, to) = MAIN_COVER
        .windows(2)
        .find(|pair| distance <= pair[1].0)
        .map(|pair| (pair[0], pair[1]))
        .unwrap_or((MAIN_COVER[22], MAIN_COVER[23]));

    from.1 + (to.1 - from.1) * ((distance - from.0) / (to.0 - from.0)).clamp(0.0, 1.0)
}

/// Расстояние до края по доле q ∈ [0, 1] распределения.
fn edge_by_share(share: f64) -> f64 {
    let at = share.clamp(0.0, 1.0) * 40.0;
    let low = (at.floor() as usize).min(39);

    EDGE[low] + (EDGE[low + 1] - EDGE[low]) * (at - low as f64)
}

/// Выборка значений шума, по которой значение переводится в долю
/// («какая часть мест лежит ниже»).
struct Ranks(Vec<f32>);

impl Ranks {
    fn new(seed: u64, noise: impl Fn(i32, i32) -> f64) -> Ranks {
        const SAMPLES: usize = 40_000;

        let mut state = seed ^ 0x51ED_2705;
        let mut next = || {
            state = scramble(state.wrapping_add(0x9E37_79B9_7F4A_7C15));
            (state % 6_000_000) as i32 - 3_000_000
        };
        let mut values: Vec<f32> = (0..SAMPLES).map(|_| noise(next(), next()) as f32).collect();

        values.sort_by(f32::total_cmp);
        Ranks(values)
    }

    /// Какая доля выборки ниже этого значения.
    fn share_below(&self, value: f64) -> f64 {
        self.0.partition_point(|sample| f64::from(*sample) < value) as f64 / self.0.len() as f64
    }
}

/// Как складывается Энд этого мира.
pub struct End {
    seed: i64,
    /// Шум внешних островов.
    islands: Noise,
    /// Шум каймы главного острова.
    fringe: Noise,
    /// Шум мелких островков.
    islets: Noise,
    /// Неровность верха и низа.
    top: Noise,
    bottom: Noise,
    /// Разброс границ биомов.
    jitter: Noise,
    outer_ranks: OnceLock<Ranks>,
    main_ranks: OnceLock<Ranks>,
    states: States,
}

/// Состояния блоков Энда — берутся из таблицы один раз.
struct States {
    end_stone: i32,
    obsidian: i32,
    bedrock: i32,
    fire: i32,
    chorus_flower: i32,
    torch: [i32; 4],
}

impl States {
    fn new() -> States {
        let named = |name: &str| blocks::state_by_name(name).unwrap_or_else(|| panic!("блок {} есть в таблице", name));
        let torch = |facing: &str| {
            blocks::state_from_text(&format!("wall_torch[facing={}]", facing)).unwrap_or_else(|| panic!("факел есть в таблице"))
        };

        States {
            end_stone: named("end_stone"),
            obsidian: named("obsidian"),
            bedrock: named("bedrock"),
            fire: named("fire"),
            chorus_flower: blocks::state_from_text("chorus_flower[age=5]").expect("хорус в таблице"),
            torch: [torch("north"), torch("east"), torch("south"), torch("west")],
        }
    }
}

/// Один из десяти обсидиановых столбов.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Spike {
    /// Центр столба.
    pub x: i32,
    pub z: i32,
    /// Радиус диска: клетка в столбе, если dx² + dz² ≤ r² + 1 (замер).
    pub radius: i32,
    /// Высота бедрока сверху: обсидиан идёт от y = 0 до `height − 1`.
    pub height: i32,
    /// Клетка из железных прутьев вокруг кристалла.
    pub caged: bool,
}

/// Места столбов — не зависят от семени (замер совпал с вики).
pub const SPIKE_PLACES: [(i32, i32); 10] =
    [(-42, -1), (-34, -25), (-34, 24), (-13, -40), (-13, 39), (12, -40), (12, 39), (33, -25), (33, 24), (42, 0)];

/// Столбы мира: высоты 76…103 через 3 перемешаны по местам от семени.
pub fn spikes(seed: i64) -> [Spike; 10] {
    let mut order: [i32; 10] = std::array::from_fn(|i| i as i32);
    let mut state = (seed as u64) ^ 0x5B1E_5A17;

    for at in (1..10).rev() {
        state = scramble(state.wrapping_add(0x9E37_79B9_7F4A_7C15));
        order.swap(at, (state % (at as u64 + 1)) as usize);
    }

    std::array::from_fn(|i| {
        let step = order[i];
        let height = 76 + step * 3;

        Spike {
            x: SPIKE_PLACES[i].0,
            z: SPIKE_PLACES[i].1,
            radius: 2 + step / 3,
            height,
            // Клетки — на двух самых коротких, кроме самого короткого
            // (замер: клетки на высотах 79 и 82).
            caged: step == 1 || step == 2,
        }
    })
}

impl End {
    pub fn new(seed: i64) -> End {
        End {
            seed,
            islands: Noise::new(seed ^ 0x0E_4D_15_1A),
            fringe: Noise::new(seed ^ 0x3F_21_9C_E5),
            islets: Noise::new(seed ^ 0x2B_7E_15_16),
            top: Noise::new(seed ^ 0x77_B3_04_A9),
            bottom: Noise::new(seed ^ 0x1D_C8_62_5F),
            jitter: Noise::new(seed ^ 0x6A_09_E6_67),
            outer_ranks: OnceLock::new(),
            main_ranks: OnceLock::new(),
            states: States::new(),
        }
    }

    /// Семя мира.
    #[allow(dead_code)] // для порталов и построек
    pub fn seed(&self) -> i64 {
        self.seed
    }

    /// Шум внешних островов в точке.
    fn island_noise(&self, x: i32, z: i32) -> f64 {
        let (fx, fz) = (f64::from(x), f64::from(z));

        // Крупные острова и, поверх них, мелкие пятна — отдельные островки
        // в сотни блоков площадью (замер: их вдвое-втрое больше, чем даёт
        // один слой шума).
        self.islands.octaves(fx / 190.0 + 0.37, fz / 190.0 + 0.71, 5) + ISLETS * self.islets.octaves(fx / 55.0 + 0.19, fz / 55.0 + 0.83, 2)
    }

    /// Шум каймы главного острова в точке.
    fn fringe_noise(&self, x: i32, z: i32) -> f64 {
        self.fringe.octaves(f64::from(x) / 45.0 + 0.13, f64::from(z) / 45.0 + 0.59, 4)
    }

    /// Верх и низ эндемняка в этом столбце; None — пустота.
    pub fn column(&self, x: i32, z: i32) -> Option<(i32, i32)> {
        let distance = f64::from(x).hypot(f64::from(z));

        // Главный остров: диск, дальше кайма.
        if distance < 150.0 {
            return self.main_column(x, z, distance);
        }

        let cover = outer_cover(distance);

        if cover <= 0.0 {
            return None;
        }

        let ranks = self.outer_ranks.get_or_init(|| Ranks::new(self.seed as u64, |x, z| self.island_noise(x, z)));
        let above = ranks.share_below(self.island_noise(x, z));

        if above < 1.0 - cover {
            return None;
        }

        let depth = edge_by_share((above - (1.0 - cover)) / cover);

        Some(self.shape(x, z, depth, 0.0))
    }

    fn main_column(&self, x: i32, z: i32, distance: f64) -> Option<(i32, i32)> {
        // Диск: сплошной, верх и низ — по замеру радиальных средних.
        if distance <= MAIN_RADIUS + 2.0 {
            let (top, bottom) = lookup(&MAIN_DISC, distance);
            // У центра верх ровный (подставка выходного портала стоит на
            // нём), дальше неровность нарастает.
            let rough = (distance / 40.0).min(1.0);

            return Some(self.rough(x, z, top, bottom, 1.4 * rough, 2.5 * rough));
        }

        let cover = main_cover(distance);

        if cover <= 0.0 {
            return None;
        }

        let ranks = self.main_ranks.get_or_init(|| Ranks::new(self.seed as u64, |x, z| self.fringe_noise(x, z)));
        let above = ranks.share_below(self.fringe_noise(x, z));

        if above < 1.0 - cover {
            return None;
        }

        let depth = edge_by_share((above - (1.0 - cover)) / cover);

        Some(self.shape(x, z, depth, -2.0))
    }

    /// Верх и низ столбца по расстоянию до края острова.
    fn shape(&self, x: i32, z: i32, depth: f64, deeper: f64) -> (i32, i32) {
        let (top, bottom) = lookup(&PROFILE, depth);
        let rim = (-depth / 8.0).exp();

        self.rough(x, z, top, bottom + deeper * (depth / 40.0).min(1.0), 1.8 + 3.0 * rim, 3.5 + 4.0 * (-depth / 10.0).exp())
    }

    /// Неровность: верх и низ гуляют вокруг средних.
    fn rough(&self, x: i32, z: i32, top: f64, bottom: f64, top_spread: f64, bottom_spread: f64) -> (i32, i32) {
        const NORM: f64 = 3.6;

        let (fx, fz) = (f64::from(x), f64::from(z));
        let top = top + self.top.octaves(fx / 26.0 + 0.21, fz / 26.0 + 0.47, 3) * NORM * top_spread;
        let bottom = bottom + self.bottom.octaves(fx / 19.0 + 0.83, fz / 19.0 + 0.11, 3) * NORM * bottom_spread;
        let top = top.round() as i32;
        let bottom = (bottom.round() as i32).clamp(1, top);

        (bottom, top.max(bottom))
    }

    /// Биом столбца: главный остров и пустота вокруг него — «край»; дальше
    /// по толщине острова под клеткой (замер: у пустоты и тонких краёв
    /// малые острова и пустоши, у толстых — верховья).
    pub fn biome_at(&self, x: i32, z: i32) -> Biome {
        if f64::from(x).hypot(f64::from(z)) < 1000.0 {
            return Biome::TheEnd;
        }

        let jitter = self.jitter.octaves(f64::from(x) / 37.0 + 0.5, f64::from(z) / 37.0 + 0.5, 2);
        let choice = (jitter + 1.0) / 2.0;

        match self.column(x, z) {
            None => match choice {
                c if c < 0.56 => Biome::SmallEndIslands,
                c if c < 0.80 => Biome::EndBarrens,
                _ => Biome::EndMidlands,
            },
            Some((bottom, top)) => {
                let thick = f64::from(top - bottom + 1) + (jitter * 10.0);

                match thick {
                    t if t >= 38.0 => Biome::EndHighlands,
                    t if t >= 24.0 => Biome::EndMidlands,
                    t if t >= 6.0 => {
                        if choice < 0.68 {
                            Biome::EndMidlands
                        } else {
                            Biome::EndBarrens
                        }
                    }
                    _ => {
                        if choice < 0.5 {
                            Biome::SmallEndIslands
                        } else {
                            Biome::EndBarrens
                        }
                    }
                }
            }
        }
    }

    /// Вершина острова над этим столбцом, если он есть.
    #[allow(dead_code)] // для порталов и строений
    pub fn surface(&self, x: i32, z: i32) -> Option<i32> {
        self.column(x, z).map(|(_, top)| top)
    }

    /// Чанк Энда.
    #[cfg(not(seedmap_core))]
    pub(super) fn chunk(&self, chunk_x: i32, chunk_z: i32) -> Chunk {
        let mut chunk = Chunk::new();
        let (x0, z0) = (chunk_x * CHUNK_SIZE, chunk_z * CHUNK_SIZE);
        let mut tops = [None::<i32>; (CHUNK_SIZE * CHUNK_SIZE) as usize];

        for local_z in 0..CHUNK_SIZE {
            for local_x in 0..CHUNK_SIZE {
                if let Some((bottom, top)) = self.column(x0 + local_x, z0 + local_z) {
                    tops[(local_z * CHUNK_SIZE + local_x) as usize] = Some(top);

                    for y in bottom..=top {
                        put(&mut chunk, local_x, y, local_z, self.states.end_stone);
                    }
                }
            }
        }

        self.chorus(&mut chunk, chunk_x, chunk_z, &tops);
        self.cities(&mut chunk, chunk_x, chunk_z);
        self.podium(&mut chunk, x0, z0);
        self.spikes(&mut chunk, x0, z0);

        // Биом — по клетке 4×4, весь столбец клеток одинаков.
        for cell_x in 0..BIOME_CELLS {
            for cell_z in 0..BIOME_CELLS {
                let biome = self.biome_at(x0 + cell_x as i32 * 4 + 2, z0 + cell_z as i32 * 4 + 2).number() as BiomeId;

                for layer in 0..BIOME_LAYERS {
                    chunk.biomes[biome_cell(layer, cell_x, cell_z)] = biome;
                }
            }
        }

        chunk
    }

    /// Столбы: обсидиановый диск от дна, бедрок и огонь сверху, клетка.
    #[cfg(not(seedmap_core))]
    fn spikes(&self, chunk: &mut Chunk, x0: i32, z0: i32) {
        let s = &self.states;

        for spike in spikes(self.seed) {
            let reach = spike.radius + 3;

            if spike.x + reach < x0 || spike.x - reach >= x0 + CHUNK_SIZE || spike.z + reach < z0 || spike.z - reach >= z0 + CHUNK_SIZE {
                continue;
            }

            let inside = |x: i32, z: i32| (0..CHUNK_SIZE).contains(&(x - x0)) && (0..CHUNK_SIZE).contains(&(z - z0));
            let set = |chunk: &mut Chunk, x: i32, y: i32, z: i32, state: i32| {
                if inside(x, z) {
                    put(chunk, x - x0, y, z - z0, state);
                }
            };

            for dx in -spike.radius..=spike.radius {
                for dz in -spike.radius..=spike.radius {
                    if dx * dx + dz * dz > spike.radius * spike.radius + 1 {
                        continue;
                    }

                    for y in 0..spike.height {
                        set(chunk, spike.x + dx, y, spike.z + dz, s.obsidian);
                    }
                }
            }

            set(chunk, spike.x, spike.height, spike.z, s.bedrock);
            set(chunk, spike.x, spike.height + 1, spike.z, s.fire);

            if spike.caged {
                for (x, y, z, state) in cage(spike.height) {
                    set(chunk, spike.x + x, y, spike.z + z, state);
                }
            }
        }
    }

    /// Подставка выходного портала в (0, 0): чаша из бедрока, столб с
    /// факелами. Порталы внутри чаши появляются, когда портал включают.
    #[cfg(not(seedmap_core))]
    fn podium(&self, chunk: &mut Chunk, x0: i32, z0: i32) {
        if x0 > 4 || x0 + CHUNK_SIZE <= -4 || z0 > 4 || z0 + CHUNK_SIZE <= -4 {
            return;
        }

        for (x, y, z, state) in podium_blocks(&self.states) {
            if (0..CHUNK_SIZE).contains(&(x - x0)) && (0..CHUNK_SIZE).contains(&(z - z0)) {
                put(chunk, x - x0, y, z - z0, state);
            }
        }
    }

    /// Хорус: простые деревья на внешних островах верховий и середины.
    #[cfg(not(seedmap_core))]
    fn chorus(&self, chunk: &mut Chunk, chunk_x: i32, chunk_z: i32, tops: &[Option<i32>]) {
        let (x0, z0) = (chunk_x * CHUNK_SIZE, chunk_z * CHUNK_SIZE);
        // Клетки, занятые деревьями этого чанка: соседние деревья не растут
        // друг сквозь друга.
        let mut taken = std::collections::HashSet::new();

        // Дерево растёт в столбце, если выпал жребий; ветви не выходят за
        // чанк — столбцы у краёв пропускаем.
        for local_z in 3..CHUNK_SIZE - 3 {
            for local_x in 3..CHUNK_SIZE - 3 {
                let Some(top) = tops[(local_z * CHUNK_SIZE + local_x) as usize] else { continue };
                let (x, z) = (x0 + local_x, z0 + local_z);

                if f64::from(x).hypot(f64::from(z)) < 1000.0 {
                    continue;
                }

                let mut roll = self.dice(x, z);

                if roll.unit() >= CHORUS_PER_COLUMN {
                    continue;
                }

                if !matches!(self.biome_at(x, z), Biome::EndHighlands | Biome::EndMidlands) {
                    continue;
                }

                self.tree(chunk, local_x, top + 1, local_z, &mut roll, &mut taken);
            }
        }
    }

    /// Город региона: чанк старта и низ, если остров под основанием
    /// достаточно высок и биом подходит (end_city.rs).
    pub fn city_at(&self, region_x: i32, region_z: i32) -> Option<(i32, i32, i32)> {
        let (chunk_x, chunk_z) = end_city::region_start(self.seed, region_x, region_z);
        let (x, z) = end_city::root_corner(chunk_x, chunk_z);

        if f64::from(x).hypot(f64::from(z)) < OUTER_FROM {
            return None;
        }

        if !matches!(self.biome_at(x + 4, z + 4), Biome::EndHighlands | Biome::EndMidlands) {
            return None;
        }

        // Низ города — на верхе эндемняка под углами и серединой основания.
        let mut top = i32::MAX;

        for (dx, dz) in [(0, 0), (9, 0), (0, 9), (9, 9), (4, 4)] {
            top = top.min(self.surface(x + dx, z + dz)?);
        }

        (top >= end_city::MIN_TOP).then_some((chunk_x, chunk_z, top + 1))
    }

    /// Шалкеры энд-городов, стоящие в чанке (end_city::sentries).
    #[cfg(not(seedmap_core))]
    pub fn city_sentries(&self, chunk_x: i32, chunk_z: i32) -> Vec<(i32, i32, i32)> {
        let (x0, z0) = (chunk_x * CHUNK_SIZE, chunk_z * CHUNK_SIZE);
        let mut out = Vec::new();

        for (region_x, region_z) in end_city::regions_near(chunk_x, chunk_z) {
            let Some((start_x, start_z, y)) = self.city_at(region_x, region_z) else { continue };

            if (start_x - chunk_x).abs() > end_city::RADIUS_CHUNKS || (start_z - chunk_z).abs() > end_city::RADIUS_CHUNKS {
                continue;
            }

            out.extend(end_city::sentries(&end_city::plan(self.seed, start_x, start_z, y), x0, z0));
        }

        out
    }

    #[cfg(not(seedmap_core))]
    pub fn in_city(&self, x: i32, y: i32, z: i32) -> bool {
        end_city::regions_near(x.div_euclid(16), z.div_euclid(16)).into_iter()
            .filter_map(|(rx, rz)| self.city_at(rx, rz))
            .any(|(cx, cz, height)| super::in_structure_bounds(end_city::plan(self.seed, cx, cz, height).iter().map(|p| p.bounds), (x, y, z)))
    }

    /// Энд-города, задевающие чанк.
    #[cfg(not(seedmap_core))]
    fn cities(&self, chunk: &mut Chunk, chunk_x: i32, chunk_z: i32) {
        let (x0, z0) = (chunk_x * CHUNK_SIZE, chunk_z * CHUNK_SIZE);

        for (region_x, region_z) in end_city::regions_near(chunk_x, chunk_z) {
            let Some((start_x, start_z, y)) = self.city_at(region_x, region_z) else { continue };

            if (start_x - chunk_x).abs() > end_city::RADIUS_CHUNKS || (start_z - chunk_z).abs() > end_city::RADIUS_CHUNKS {
                continue;
            }

            let pieces = end_city::plan(self.seed, start_x, start_z, y);
            end_city::paint(&pieces, x0, z0, AIR, &mut |x, y, z, state| put(chunk, x - x0, y, z - z0, state));
        }
    }

    /// Одно дерево хоруса — выращено по правилам роста цветка (вики, Chorus
    /// Flower, Growth): цветок растёт вверх, пока над ним пусто, с шансом по
    /// высоте стебля под ним («100% chance if zero or one plant block is
    /// directly beneath, 60% if above a two-block unbranched plant, 50% …
    /// branch, 40% … three-block unbranched, 25% … branch, 20% … four-block
    /// unbranched»); не вырос вверх — ветвится вбок («1–4 branches … on an
    /// unbranched plant or 0–3 on a branched plant», в сторону, где пусто
    /// вокруг и снизу); не вырос никуда — засыхает (age 5).
    #[cfg(not(seedmap_core))]
    fn tree(&self, chunk: &mut Chunk, x: i32, y: i32, z: i32, roll: &mut Dice, taken: &mut std::collections::HashSet<(i32, i32, i32)>) {
        let mut cells: Vec<(i32, i32, i32)> = Vec::new();
        // Живые цветки: место и сколько блоков стебля под ним и ветвился ли он.
        let mut flowers: Vec<((i32, i32, i32), i32, bool)> = vec![((x, y, z), 0, false)];
        let mut dead: Vec<(i32, i32, i32)> = Vec::new();
        let free = |cells: &[(i32, i32, i32)], flowers: &[((i32, i32, i32), i32, bool)], dead: &[(i32, i32, i32)], c: (i32, i32, i32)| {
            (3..CHUNK_SIZE - 3).contains(&c.0)
                && (3..CHUNK_SIZE - 3).contains(&c.2)
                && c.1 < y + 22
                && !taken.contains(&c)
                && !cells.contains(&c)
                && !flowers.iter().any(|f| f.0 == c)
                && !dead.contains(&c)
        };

        for _ in 0..40 {
            if flowers.is_empty() {
                break;
            }

            let mut next = Vec::new();

            for (at, height, branched) in flowers {
                let up = (at.0, at.1 + 1, at.2);
                let share = match (height, branched) {
                    (0 | 1, _) => 1.0,
                    (2, false) => 0.6,
                    (2, true) => 0.5,
                    (3, false) => 0.4,
                    (3, true) => 0.25,
                    (4, false) => 0.2,
                    _ => 0.0,
                };
                let sides = [(1, 0), (-1, 0), (0, 1), (0, -1)];
                let clear_around = |c: (i32, i32, i32)| sides.iter().all(|(dx, dz)| free(&cells, &next, &dead, (c.0 + dx, c.1, c.2 + dz)));

                if roll.unit() < share && free(&cells, &next, &dead, up) && clear_around(up) {
                    cells.push(at);
                    next.push((up, height + 1, branched));
                    continue;
                }

                let tries = if branched { roll.below(4) } else { 1 + roll.below(4) };
                let mut grown = false;

                for _ in 0..tries {
                    let (dx, dz) = sides[roll.below(4) as usize];
                    let side = (at.0 + dx, at.1, at.2 + dz);
                    let below = (side.0, side.1 - 1, side.2);
                    let others = sides
                        .iter()
                        .filter(|(ox, oz)| (*ox, *oz) != (-dx, -dz))
                        .all(|(ox, oz)| free(&cells, &next, &dead, (side.0 + ox, side.1, side.2 + oz)));

                    if free(&cells, &next, &dead, side) && others && free(&cells, &next, &dead, below) && below.1 > y {
                        next.push((side, 0, true));
                        grown = true;
                    }
                }

                if grown {
                    cells.push(at);
                } else {
                    dead.push(at);
                }
            }

            flowers = next;
        }

        let has = |cx: i32, cy: i32, cz: i32| cells.contains(&(cx, cy, cz)) || dead.contains(&(cx, cy, cz)) || flowers.iter().any(|f| f.0 == (cx, cy, cz));

        for &(cx, cy, cz) in &cells {
            let text = format!(
                "chorus_plant[down={},east={},north={},south={},up={},west={}]",
                (cx, cy, cz) == (x, y, z) || has(cx, cy - 1, cz),
                has(cx + 1, cy, cz),
                has(cx, cy, cz - 1),
                has(cx, cy, cz + 1),
                has(cx, cy + 1, cz),
                has(cx - 1, cy, cz)
            );
            let state = blocks::state_from_text(&text).unwrap_or_else(|| panic!("хорус в таблице: {}", text));

            put(chunk, cx, cy, cz, state);
        }

        for (fx, fy, fz) in dead.into_iter().chain(flowers.into_iter().map(|f| f.0)) {
            put(chunk, fx, fy, fz, self.states.chorus_flower);
            taken.insert((fx, fy, fz));
        }

        taken.extend(cells);
    }

    /// Кости для решений по столбцу.
    fn dice(&self, x: i32, z: i32) -> Dice {
        Dice(scramble(
            (self.seed as u64) ^ (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (z as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F) ^ 0xC40_5115,
        ))
    }
}

/// Сколько деревьев хоруса на столбец верховий и середины. Замер: 0,08
/// блока стебля и 0,01 цветка на столбец внешних островов; у дерева
/// ~8 блоков стебля на цветок. Подогнано переписью end_city_census.
const CHORUS_PER_COLUMN: f64 = 0.0095;

/// Кости.
struct Dice(u64);

impl Dice {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        scramble(self.0)
    }

    fn below(&mut self, bound: i32) -> i32 {
        (self.next() % bound.max(1) as u64) as i32
    }

    fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// Клетка из 73 железных прутьев: стенка 5×5 в три слоя над бедроком и
/// сплошная крышка на четвёртом (замер: слои y = h…h+2 по 16, h+3 — 25).
pub fn cage(height: i32) -> Vec<(i32, i32, i32, i32)> {
    let mut out = Vec::new();

    for y in height..=height + 3 {
        for dx in -2..=2 {
            for dz in -2..=2 {
                let wall = dx == -2 || dx == 2 || dz == -2 || dz == 2;

                if y <= height + 2 && !wall {
                    continue;
                }

                // Пруток соединяется с соседними прутками клетки.
                let has = |ax: i32, az: i32| {
                    (-2..=2).contains(&ax)
                        && (-2..=2).contains(&az)
                        && (y == height + 3 || ax == -2 || ax == 2 || az == -2 || az == 2)
                };
                let text = format!(
                    "iron_bars[east={},north={},south={},waterlogged=false,west={}]",
                    has(dx + 1, dz),
                    has(dx, dz - 1),
                    has(dx, dz + 1),
                    has(dx - 1, dz)
                );
                let state = blocks::state_from_text(&text).expect("прутья в таблице");

                out.push((dx, y, dz, state));
            }
        }
    }

    out
}

/// Клетки внутри чаши подставки, где встаёт выходной портал: круг
/// dx² + dz² ≤ 5 без центрального столба. Их двадцать (замер оригинала
/// после смерти дракона; вики называет 16).
pub fn portal_cells() -> Vec<(i32, i32)> {
    let mut out = Vec::new();

    for dx in -2i32..=2 {
        for dz in -2i32..=2 {
            let d2 = dx * dx + dz * dz;

            if d2 <= 5 && d2 != 0 {
                out.push((dx, dz));
            }
        }
    }

    out
}

/// Блоки подставки, как их снял замер оригинала (бой с драконом на нашем
/// экземпляре оригинала, `tools/measure/end_main.py`): `(x, y, z, состояние)`.
/// Портальная плоскость лежит на верхнем эндемняке в (0, 0), y = 61:
/// — под ней круг из 21 бедрока (dx² + dz² ≤ 5);
/// — на её уровне кольцо из 16 бедрока (dx² + dz² от 8 до 10), внутри
///   него пусто (у включённого — 20 порталов) и столб в центре;
/// — столб бедрока в четыре блока над плоскостью, факелы на третьем;
///   яйцо дракона над столбом кладёт бой.
fn podium_blocks(s: &States) -> Vec<(i32, i32, i32, i32)> {
    let base = CENTER_TOP;
    let mut out = Vec::new();

    for dx in -3i32..=3 {
        for dz in -3i32..=3 {
            let d2 = dx * dx + dz * dz;

            if d2 <= 5 {
                out.push((dx, base - 1, dz, s.bedrock));
            }

            match d2 {
                0 => out.push((dx, base, dz, s.bedrock)),
                1..=5 => out.push((dx, base, dz, AIR)),
                8..=10 => out.push((dx, base, dz, s.bedrock)),
                _ => {}
            }
        }
    }

    for y in base + 1..=base + 3 {
        out.push((0, y, 0, s.bedrock));
    }

    for (at, (dx, dz)) in [(0, -1), (1, 0), (0, 1), (-1, 0)].into_iter().enumerate() {
        out.push((dx, base + 2, dz, s.torch[at]));
    }

    out
}

/// Клетки, где встаёт включённый выходной портал: `(x, y, z)`.
#[allow(dead_code)] // пока нет боя с драконом, который его включает
pub fn exit_portal_blocks() -> Vec<(i32, i32, i32)> {
    portal_cells().into_iter().map(|(dx, dz)| (dx, CENTER_TOP, dz)).collect()
}

/// Кладёт блок в чанк: `x`, `z` — внутри чанка, `y` — высота мира.
#[cfg(not(seedmap_core))]
fn put(chunk: &mut Chunk, x: i32, y: i32, z: i32, state: i32) {
    if !(0..256).contains(&y) {
        return;
    }

    let (section, layer) = split(y);
    let blocks = chunk.filled_section(section);

    blocks[layer + (z * CHUNK_SIZE + x) as usize] = pack(state);
}

/// Перемешивает биты числа — для случайных решений по месту.
fn scramble(mut value: u64) -> u64 {
    value ^= value >> 33;
    value = value.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    value ^= value >> 33;
    value = value.wrapping_mul(0xC4CE_B9FE_1A85_EC53);
    value ^ (value >> 33)
}

#[cfg(all(test, not(seedmap_core)))]
mod tests {
    use super::*;

    /// Хорус растёт по правилам вики: стебель связан вниз до эндемняка,
    /// цветки только на концах, и деревья есть на внешних островах.
    #[test]
    fn chorus_trees_are_connected_and_tipped_with_flowers() {
        let end = End::new(100554032945340);
        let plant = |state: i32| blocks::block_at_state(state) == Some("chorus_plant");
        let flower = |state: i32| blocks::block_at_state(state) == Some("chorus_flower");
        let (mut plants, mut flowers, mut covered) = (0, 0, 0);

        for chunk_x in 70..110 {
            for chunk_z in 60..80 {
                let chunk = end.chunk(chunk_x, chunk_z);
                let at = |x: i32, y: i32, z: i32| {
                    let (section, layer) = split(y);
                    chunk.section(section).map_or(AIR, |blocks| super::super::unpack(blocks[layer + (z * CHUNK_SIZE + x) as usize]))
                };

                for x in 0..16 {
                    for z in 0..16 {
                        for y in 40..120 {
                            let state = at(x, y, z);

                            if plant(state) {
                                plants += 1;
                                let below = at(x, y - 1, z);
                                let sideways = [(1, 0), (-1, 0), (0, 1), (0, -1)]
                                    .iter()
                                    .any(|(dx, dz)| (0..16).contains(&(x + dx)) && (0..16).contains(&(z + dz)) && (plant(at(x + dx, y, z + dz)) || flower(at(x + dx, y, z + dz))));
                                assert!(plant(below) || blocks::block_at_state(below) == Some("end_stone") || sideways, "стебель висит в {} {} {}", x, y, z);
                            } else if flower(state) {
                                flowers += 1;
                                // Над цветком пусто, кроме редких случаев, когда
                                // соседнее дерево или город легли сверху.
                                if at(x, y + 1, z) != AIR {
                                    covered += 1;
                                }
                            }
                        }
                    }
                }
            }
        }

        assert!(plants > 200, "стеблей {}", plants);
        assert!(flowers > 20, "цветков {}", flowers);
        assert!(covered * 50 < flowers, "накрытых цветков {} из {}", covered, flowers);
        let ratio = plants as f64 / flowers as f64;
        assert!((4.0..12.0).contains(&ratio), "стеблей на цветок {}", ratio);
    }

    /// Перепись блоков городов и хоруса на внешних островах — тот же счёт,
    /// что `tools/measure/end_city.py blocks` с мира оригинала: блоков на
    /// 10⁴ столбцов с эндемняком дальше 1000 от центра и городов на регион.
    /// Запуск: cargo test --release end_city_census -- --ignored --nocapture
    #[test]
    #[ignore]
    fn end_city_census() {
        let end = End::new(100554032945340);
        let names = ["purpur_block", "purpur_pillar", "purpur_stairs", "purpur_slab", "end_stone_bricks", "magenta_stained_glass", "end_rod", "chest", "obsidian", "ladder", "dragon_head", "brewing_stand", "chorus_plant", "chorus_flower"];
        let mut counts = vec![0u64; names.len()];
        let mut columns = 0u64;
        let mut chunks = 0u64;
        let mut cities = std::collections::HashSet::new();
        let mut regions = std::collections::HashSet::new();
        let half: i32 = 128;

        for chunk_x in -half..half {
            for chunk_z in -half..half {
                if (chunk_x * 16 + 8).pow(2) + (chunk_z * 16 + 8).pow(2) < 1000 * 1000 {
                    continue;
                }

                chunks += 1;
                regions.insert((chunk_x.div_euclid(end_city::REGION), chunk_z.div_euclid(end_city::REGION)));

                if end.city_at(chunk_x.div_euclid(end_city::REGION), chunk_z.div_euclid(end_city::REGION)).is_some_and(|(sx, sz, _)| (sx, sz) == (chunk_x, chunk_z)) {
                    cities.insert((chunk_x, chunk_z));
                }

                let chunk = end.chunk(chunk_x, chunk_z);
                let mut stone = [false; 256];

                for section in 0..chunk.sections.len() {
                    let Some(blocks) = chunk.section(section) else { continue };

                    for (at, packed) in blocks.iter().enumerate() {
                        let state = super::super::unpack(*packed);

                        if state == AIR {
                            continue;
                        }

                        let name = blocks::block_at_state(state).unwrap_or("");

                        if name == "end_stone" {
                            stone[at & 255] = true;
                        } else if let Some(i) = names.iter().position(|n| *n == name) {
                            counts[i] += 1;
                        }
                    }
                }

                columns += stone.iter().filter(|s| **s).count() as u64;
            }
        }

        eprintln!("чанков {}, столбцов с эндемняком {}, регионов {}, городов {}", chunks, columns, regions.len(), cities.len());

        for (name, count) in names.iter().zip(&counts) {
            eprintln!("  {:<22} {:>8}  на 10⁴ столбцов {:.1}", name, count, *count as f64 * 1e4 / columns.max(1) as f64);
        }
    }

    #[test]
    fn terrain_is_deterministic() {
        let (a, b) = (End::new(42), End::new(42));

        for (x, z) in [(0, 0), (30, -20), (1100, 300), (-1500, 1700), (2500, -900)] {
            assert_eq!(a.column(x, z), b.column(x, z));
        }
    }

    #[test]
    fn main_island_is_solid_in_the_middle_and_the_gap_is_void() {
        let end = End::new(7);

        for (x, z) in [(0, 0), (20, 10), (-40, 30), (10, -55)] {
            let (bottom, top) = end.column(x, z).expect("остров");
            assert!((55..=64).contains(&top), "верх {} в {} {}", top, x, z);
            assert!((8..=24).contains(&bottom), "низ {} в {} {}", bottom, x, z);
        }

        assert_eq!(end.column(0, 0).map(|c| c.1), Some(CENTER_TOP));

        for (x, z) in [(300, 0), (0, -500), (-700, 300), (600, 600), (900, 10)] {
            assert_eq!(end.column(x, z), None, "пустота в {} {}", x, z);
        }
    }

    #[test]
    fn outer_islands_cover_about_forty_five_percent() {
        let end = End::new(7);
        let mut covered = 0;
        let mut total = 0;

        for x in (-2000..2000).step_by(16) {
            for z in (-2000..2000).step_by(16) {
                if f64::from(x).hypot(f64::from(z)) < 1200.0 {
                    continue;
                }

                total += 1;
                covered += usize::from(end.column(x, z).is_some());
            }
        }

        let share = covered as f64 / total as f64;
        assert!((0.36..0.54).contains(&share), "доля {}", share);
    }

    #[test]
    fn no_outer_island_before_the_gap_ends() {
        let end = End::new(11);

        for step in 0..360 {
            let angle = f64::from(step).to_radians();

            for radius in [200.0, 500.0, 900.0, 940.0] {
                let (x, z) = ((radius * angle.cos()) as i32, (radius * angle.sin()) as i32);
                assert_eq!(end.column(x, z), None);
            }
        }
    }

    #[test]
    fn spikes_use_the_measured_sizes() {
        let list = spikes(123);
        let mut heights: Vec<i32> = list.iter().map(|s| s.height).collect();

        heights.sort_unstable();
        assert_eq!(heights, vec![76, 79, 82, 85, 88, 91, 94, 97, 100, 103]);

        for spike in list {
            assert_eq!(spike.radius, 2 + (spike.height - 76) / 9);
            assert_eq!(spike.caged, spike.height == 79 || spike.height == 82);
            assert!(f64::from(spike.x).hypot(f64::from(spike.z)) > 40.5 && f64::from(spike.x).hypot(f64::from(spike.z)) < 43.5);
        }

        assert_eq!(cage(79).len(), 73);
        assert_eq!(spikes(1), spikes(1));
    }

    #[test]
    fn spikes_are_shuffled_by_seed() {
        assert!((1..40).any(|seed| spikes(seed) != spikes(0)));
    }

    #[test]
    fn spike_shape_is_a_disc_in_the_chunk() {
        let end = End::new(5);
        let spike = spikes(5)[9];
        let chunk_x = spike.x.div_euclid(16);
        let chunk_z = spike.z.div_euclid(16);
        let chunk = end.chunk(chunk_x, chunk_z);
        let (local_x, local_z) = (spike.x.rem_euclid(16), spike.z.rem_euclid(16));
        let at = |x: i32, y: i32, z: i32| {
            let (section, layer) = split(y);
            chunk.sections[section].as_ref().map(|blocks| i32::from(blocks[layer + (z * 16 + x) as usize])).unwrap_or(AIR)
        };
        let obsidian = blocks::state_by_name("obsidian").unwrap();

        assert_eq!(at(local_x, 0, local_z), obsidian);
        assert_eq!(at(local_x, spike.height - 1, local_z), obsidian);
        assert_eq!(at(local_x, spike.height, local_z), blocks::state_by_name("bedrock").unwrap());
        assert_eq!(at(local_x, spike.height + 1, local_z), blocks::state_by_name("fire").unwrap());
    }

    #[test]
    fn podium_has_twenty_portal_cells_like_the_original() {
        assert_eq!(portal_cells().len(), 20);
        assert_eq!(exit_portal_blocks().len(), 20);
    }

    #[test]
    fn podium_matches_the_measured_bowl() {
        let states = States::new();
        let blocks: Vec<_> = podium_blocks(&states);
        let count = |y: i32, state: i32| blocks.iter().filter(|b| b.1 == y && b.3 == state).count();

        assert_eq!(count(CENTER_TOP - 1, states.bedrock), 21);
        assert_eq!(count(CENTER_TOP, states.bedrock), 17);
        assert_eq!(count(CENTER_TOP + 1, states.bedrock), 1);
        assert_eq!(count(CENTER_TOP + 3, states.bedrock), 1);
        assert_eq!(blocks.iter().filter(|b| b.3 == states.torch[0] || b.3 == states.torch[1] || b.3 == states.torch[2] || b.3 == states.torch[3]).count(), 4);
    }

    #[test]
    fn biomes_follow_the_measured_zones() {
        let end = End::new(9);

        assert_eq!(end.biome_at(0, 0), Biome::TheEnd);
        assert_eq!(end.biome_at(500, 500), Biome::TheEnd);

        let mut seen = std::collections::HashSet::new();

        for x in (1200..1800).step_by(8) {
            for z in (-1200..-600).step_by(8) {
                seen.insert(end.biome_at(x, z).number());
            }
        }

        assert!(seen.len() >= 4, "биомов {}", seen.len());
    }

    /// Перепись нашего Энда для сверки с оригиналом
    /// (tools/measure/end_compare.py): `END_CENSUS=<файл> cargo test --release
    /// end_census -- --ignored`. Строка: x, z, низ, верх — через 4 блока.
    #[test]
    #[ignore]
    fn end_census() {
        let Ok(path) = std::env::var("END_CENSUS") else { return };
        let end = End::new(100_554_032_945_340);
        let mut out = String::new();

        for x in (-2040..2040).step_by(4) {
            for z in (-2040..2040).step_by(4) {
                if let Some((bottom, top)) = end.column(x, z) {
                    out.push_str(&format!("{}\t{}\t{}\t{}\n", x, z, bottom, top));
                }
            }
        }

        std::fs::write(path, out).expect("запись переписи");
    }

    /// Хорус на внешних островах: блоков стебля и цветков на занятый
    /// столбец — рядом с замером (0,081 и 0,0104).
    #[test]
    fn chorus_is_about_as_common_as_in_the_original() {
        let end = End::new(100_554_032_945_340);
        let (plant, flower) = (blocks::state_by_name("chorus_plant").unwrap(), blocks::state_by_name("chorus_flower").unwrap());
        let (mut plants, mut flowers, mut columns) = (0usize, 0usize, 0usize);

        for chunk_x in 80..112 {
            for chunk_z in -16..16 {
                let chunk = end.chunk(chunk_x, chunk_z);

                for x in 0..16 {
                    for z in 0..16 {
                        columns += usize::from(end.column(chunk_x * 16 + x, chunk_z * 16 + z).is_some());
                    }
                }

                for section in chunk.sections.iter().flatten() {
                    for state in section.iter() {
                        let state = i32::from(*state);
                        plants += usize::from(blocks::same_block(state, plant));
                        flowers += usize::from(blocks::same_block(state, flower));
                    }
                }
            }
        }

        let (per_plant, per_flower) = (plants as f64 / columns as f64, flowers as f64 / columns as f64);
        eprintln!("хорус: стебля {:.4}, цветков {:.4} на столбец ({} столбцов)", per_plant, per_flower, columns);
        assert!((0.04..0.16).contains(&per_plant), "стебля {}", per_plant);
        assert!((0.005..0.03).contains(&per_flower), "цветков {}", per_flower);
    }

    /// Доли биомов вне 1000 блоков — рядом с замером оригинала (малые
    /// острова 44 %, середины 24 %, верховья 20 %, пустоши 12 %).
    #[test]
    fn biome_shares_are_near_the_measured_ones() {
        let end = End::new(100_554_032_945_340);
        let mut counts = std::collections::HashMap::new();
        let mut total = 0.0;

        for x in (1100..2100).step_by(8) {
            for z in (-1000..1000).step_by(8) {
                *counts.entry(end.biome_at(x, z).number()).or_insert(0.0) += 1.0;
                total += 1.0;
            }
        }

        let share = |biome: Biome| counts.get(&biome.number()).copied().unwrap_or(0.0) / total;
        eprintln!(
            "биомы: малые {:.2}, середины {:.2}, верховья {:.2}, пустоши {:.2}",
            share(Biome::SmallEndIslands),
            share(Biome::EndMidlands),
            share(Biome::EndHighlands),
            share(Biome::EndBarrens)
        );
        assert!((0.30..0.58).contains(&share(Biome::SmallEndIslands)));
        assert!((0.15..0.35).contains(&share(Biome::EndMidlands)));
        assert!((0.10..0.30).contains(&share(Biome::EndHighlands)));
        assert!((0.05..0.25).contains(&share(Biome::EndBarrens)));
    }
}
