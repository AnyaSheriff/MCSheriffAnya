// Незер: из чего он сложен.
//
// Рельеф подогнан по замеру мира оригинала 26.1.2 (tools/research/nether.md,
// tools/measure/nether_census.py): на каждой высоте доля пустот та же, что
// у оригинала.
//
// Пустоты двух видов, как и в обычном мире:
// — большие пещеры — объём между морем лавы и потолком: где объёмный шум
//   выше порога своей высоты;
// — узкие ходы — везде от пола до потолка: где два шума разом близки к нулю.
//   Ниже моря лавы (y 31) пустоты залиты лавой — отсюда лавовые карманы
//   в толще.
// Порог каждой высоты берётся из распределения самого шума (квантиль), так
// что доля пустот на высоте y совпадает с замеренной, а не подбирается
// вручную.
//
// Биомы — по двум плоским климатическим шумам: у оригинала биом Незера по
// высоте столбца не меняется (замер tools/measure/nether_biomes.py: ни одной
// клетки 4×4 с разными биомами на y 0..127). Доли площади и размер пятен
// подогнаны к замеру. От биома зависят только поверхность и наполнение:
// нилий в лесах, песок и почва душ в долине, базальт и чернокамень в дельтах.
//
// Всё решается только по координатам и семени: объект, выросший у края
// соседнего чанка (гриб, светокамень, столб), дорисовывается в этом чанке
// по тем же числам, и на стыке ничего не рвётся.
//
// Незер живёт в той же сетке высот, что обычный мир (−64..320): клиенту он
// объявлен с этими границами, а заполнен только от 0 до 127 — так формат
// чанков и все расчёты высоты остаются общими.

use std::sync::OnceLock;

use super::noise::Noise;
use super::terrain::Biome;
use super::{biome_cell, pack, split, BiomeId, Chunk, AIR, BIOME_CELLS, BIOME_LAYERS, CHUNK_SIZE};
use crate::blocks;

/// Верх лавового моря: ниже и на этой высоте пустоты залиты лавой.
pub const LAVA_SEA: i32 = 31;

/// Самый верх Незера: выше потолка из бедрока ничего нет.
pub const CEILING: i32 = 127;

/// Слоёв бедрока у пола и у потолка: крайний сплошной, дальше реже на
/// пятую долю с каждым слоем (замер: 100, 80, 60, 40, 20 %).
const BEDROCK_LAYERS: i32 = 5;

/// Блоков в местном куске чанка: 16 × 16 × 128.
const PIECE: usize = (CHUNK_SIZE * CHUNK_SIZE * (CEILING + 1)) as usize;

/// Доли площади биомов Незера — замер оригинала: 29 областей мира
/// vanilla-big, 53 тысячи чанков (tools/measure/nether_biomes.py). Вики даёт
/// близкие числа: 36,3 / 22,2 / 8,5 / 17,1 / 15,9 %. Порядок — как в
/// `Biome::NETHER`.
const SHARES: [f64; 5] = [0.346, 0.223, 0.066, 0.181, 0.183];

/// Где биомы лежат на климатической плоскости (жар, сырость) — в долях
/// разброса шума. Порядок — как в `Biome::NETHER`: пустоши посередине,
/// остальные по сторонам.
const PLACES: [(f64, f64); 5] = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.2), (0.0, -1.0), (-1.0, 0.0)];

/// Во сколько блоков крупнейший слой климатического шума: от него размер
/// пятен биомов. Подогнано к замеру оригинала: у соседних клеток 4×4 биом
/// сменяется в 1,7 % случаев.
const CLIMATE_SCALE: f64 = 760.0;

/// Доля пустот больших пещер на высоте y (0..=127) — замер оригинала за
/// вычетом узких ходов.
fn cavern_share(y: i32) -> f64 {
    const RISE: [(i32, f64); 13] = [
        (20, 0.0),
        (21, 0.02),
        (22, 0.10),
        (23, 0.24),
        (24, 0.41),
        (26, 0.45),
        (31, 0.47),
        (40, 0.51),
        (60, 0.53),
        (90, 0.57),
        (104, 0.56),
        (108, 0.33),
        (114, 0.0),
    ];

    interpolate(&RISE, y)
}

/// Доля узких ходов на высоте y — замер оригинала: ниже моря лавы это
/// лавовые карманы, выше — cave_air.
fn tunnel_share(y: i32) -> f64 {
    const TUNNELS: [(i32, f64); 10] = [
        (1, 0.0),
        (2, 0.015),
        (5, 0.043),
        (20, 0.068),
        (31, 0.06),
        (32, 0.035),
        (104, 0.03),
        (113, 0.073),
        (120, 0.065),
        (121, 0.0),
    ];

    interpolate(&TUNNELS, y)
}

/// Доля незеррака в толще базальтовых дельт на высоте y — замер: внизу
/// и в середине почти сплошь базальт, к потолку незеррака больше.
fn delta_netherrack_share(y: i32) -> f64 {
    const SHARE: [(i32, f64); 5] = [(0, 0.08), (32, 0.10), (80, 0.17), (104, 0.24), (116, 0.30)];

    interpolate(&SHARE, y)
}

/// Доля чернокамня в толще дельт — замер: около седьмой части везде.
const DELTA_BLACKSTONE: f64 = 0.14;

/// Линейная интерполяция по точкам (высота, значение); вне точек — край.
fn interpolate(points: &[(i32, f64)], y: i32) -> f64 {
    let first = points[0];
    let last = points[points.len() - 1];

    if y <= first.0 {
        return first.1;
    }
    if y >= last.0 {
        return last.1;
    }

    for pair in points.windows(2) {
        let ((y0, v0), (y1, v1)) = (pair[0], pair[1]);

        if (y0..=y1).contains(&y) {
            let t = f64::from(y - y0) / f64::from(y1 - y0);
            return v0 + (v1 - v0) * t;
        }
    }

    last.1
}

/// Что в месте рельефа.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Void {
    Rock,
    /// Большая пещера: её пол и потолок покрывает поверхность биома.
    Cavern,
    /// Узкий ход. У оригинала его прорезают уже после поверхности (там он
    /// cave_air), поэтому стенки хода — голая порода.
    Tunnel,
}

/// Метка узкого хода в куске чанка, пока он складывается; в чанк идёт
/// воздухом.
const TUNNEL: i32 = -1;

/// Пусто ли: воздух пещеры или узкого хода.
fn is_air(state: i32) -> bool {
    state == AIR || state == TUNNEL
}

/// Состояния блоков Незера — берутся из таблицы один раз.
struct States {
    bedrock: i32,
    netherrack: i32,
    lava: i32,
    basalt: i32,
    blackstone: i32,
    magma: i32,
    gravel: i32,
    soul_sand: i32,
    soul_soil: i32,
    crimson_nylium: i32,
    warped_nylium: i32,
    quartz: i32,
    gold: i32,
    debris: i32,
    glowstone: i32,
    fire: i32,
    soul_fire: i32,
    brown_mushroom: i32,
    red_mushroom: i32,
    crimson_roots: i32,
    warped_roots: i32,
    sprouts: i32,
    crimson_fungus: i32,
    warped_fungus: i32,
    weeping_vines: i32,
    weeping_vines_plant: i32,
    twisting_vines: i32,
    twisting_vines_plant: i32,
    crimson_stem: i32,
    warped_stem: i32,
    nether_wart: i32,
    warped_wart: i32,
    shroomlight: i32,
}

impl States {
    fn new() -> States {
        let named = |name: &str| blocks::state_by_name(name).unwrap_or_else(|| panic!("блок {} есть в таблице", name));

        States {
            bedrock: named("bedrock"),
            netherrack: named("netherrack"),
            lava: named("lava"),
            basalt: named("basalt"),
            blackstone: named("blackstone"),
            magma: named("magma_block"),
            gravel: named("gravel"),
            soul_sand: named("soul_sand"),
            soul_soil: named("soul_soil"),
            crimson_nylium: named("crimson_nylium"),
            warped_nylium: named("warped_nylium"),
            quartz: named("nether_quartz_ore"),
            gold: named("nether_gold_ore"),
            debris: named("ancient_debris"),
            glowstone: named("glowstone"),
            fire: named("fire"),
            soul_fire: named("soul_fire"),
            brown_mushroom: named("brown_mushroom"),
            red_mushroom: named("red_mushroom"),
            crimson_roots: named("crimson_roots"),
            warped_roots: named("warped_roots"),
            sprouts: named("nether_sprouts"),
            crimson_fungus: named("crimson_fungus"),
            warped_fungus: named("warped_fungus"),
            weeping_vines: named("weeping_vines"),
            weeping_vines_plant: named("weeping_vines_plant"),
            twisting_vines: named("twisting_vines"),
            twisting_vines_plant: named("twisting_vines_plant"),
            crimson_stem: named("crimson_stem"),
            warped_stem: named("warped_stem"),
            nether_wart: named("nether_wart_block"),
            warped_wart: named("warped_wart_block"),
            shroomlight: named("shroomlight"),
        }
    }

    /// Порода, по которой идут поверхность и руда: её можно заменять.
    fn is_rock(&self, state: i32) -> bool {
        state == self.netherrack || state == self.basalt || state == self.blackstone
    }
}

/// Подгонка климата: разброс шума и сдвиги биомов, при которых доли
/// площади сходятся с замером.
struct Fit {
    spread: f64,
    weights: [f64; 5],
}

/// Пороги шумов по высотам 0..=127.
struct Thresholds {
    /// Большие пещеры: пусто выше порога.
    caverns: Vec<f64>,
    /// Узкие ходы: пусто ниже порога.
    tunnels: Vec<f64>,
    /// Дельты: незеррак ниже порога.
    delta_netherrack: Vec<f64>,
    /// Дельты: чернокамень выше этого порога шума пород.
    delta_blackstone: f64,
}

/// Как складывается Незер этого мира.
pub struct Nether {
    seed: i64,
    caverns: Noise,
    along: Noise,
    across: Noise,
    heat: Noise,
    damp: Noise,
    /// Породы дельт: базальт, чернокамень, незеррак.
    rocks: Noise,
    /// Пятна поверхности: песок и почва душ, гравий у моря, магма.
    patches: Noise,
    /// Второй шум пятен — независимый от первого.
    specks: Noise,
    thresholds: OnceLock<Thresholds>,
    fit: OnceLock<Fit>,
    states: States,
}

impl Nether {
    pub fn new(seed: i64) -> Nether {
        Nether {
            seed,
            caverns: Noise::new(seed ^ 0x4E_E7_48_E3),
            along: Noise::new(seed ^ 0x1B_AD_5E_ED),
            across: Noise::new(seed ^ 0x7A_11_C0_DE),
            heat: Noise::new(seed ^ 0x3E_A7_0F_1E),
            damp: Noise::new(seed ^ 0x0D_A3_9B_17),
            rocks: Noise::new(seed ^ 0x5A_B0_C4_77),
            patches: Noise::new(seed ^ 0x2F_09_E1_5D),
            specks: Noise::new(seed ^ 0x61_3C_AA_09),
            thresholds: OnceLock::new(),
            fit: OnceLock::new(),
            states: States::new(),
        }
    }

    /// Семя мира.
    #[allow(dead_code)] // для порталов и строений
    pub fn seed(&self) -> i64 {
        self.seed
    }

    /// Шум больших пещер.
    fn cavern(&self, x: i32, y: i32, z: i32) -> f64 {
        self.caverns.octaves3(f64::from(x) / 72.0 + 0.37, f64::from(y) / 30.0 + 0.61, f64::from(z) / 72.0 + 0.13, 4)
    }

    /// Шум узких ходов: насколько далеко от нуля хоть один из двух шумов.
    fn tunnel(&self, x: i32, y: i32, z: i32) -> f64 {
        let (fx, fy, fz) = (f64::from(x), f64::from(y), f64::from(z));

        self.along
            .at3(fx / 48.0 + 0.71, fy / 24.0 + 0.29, fz / 48.0 + 0.53)
            .abs()
            .max(self.across.at3(fx / 48.0 + 0.19, fy / 24.0 + 0.83, fz / 48.0 + 0.47).abs())
    }

    /// Шум пород дельт.
    fn rock(&self, x: i32, y: i32, z: i32) -> f64 {
        self.rocks.octaves3(f64::from(x) / 11.0 + 0.23, f64::from(y) / 7.0 + 0.41, f64::from(z) / 11.0 + 0.67, 2)
    }

    /// Пороги высот: на высоте y пусто, если шум пещер выше порога пещер
    /// или шум ходов ниже порога ходов. Пороги — квантили распределения
    /// шумов, снятого по выборке мест, чтобы доли совпали с замером.
    fn thresholds(&self) -> &Thresholds {
        self.thresholds.get_or_init(|| {
            const SAMPLES: usize = 60_000;

            let mut caverns = Vec::with_capacity(SAMPLES);
            let mut tunnels = Vec::with_capacity(SAMPLES);
            let mut rocks = Vec::with_capacity(SAMPLES);
            let mut state = (self.seed as u64) ^ 0x5DEE_CE66;

            for _ in 0..SAMPLES {
                let mut next = || {
                    state = scramble(state.wrapping_add(0x9E37_79B9_7F4A_7C15));
                    (state % 4_000_000) as i32 - 2_000_000
                };
                let (x, y, z) = (next(), next().rem_euclid(128), next());

                caverns.push(self.cavern(x, y, z));
                tunnels.push(self.tunnel(x, y, z));
                rocks.push(self.rock(x, y, z));
            }

            caverns.sort_by(f64::total_cmp);
            tunnels.sort_by(f64::total_cmp);
            rocks.sort_by(f64::total_cmp);

            let heights = 0..=CEILING;

            Thresholds {
                // Пещеры: пусто выше порога — порог там, где сверху
                // остаётся нужная доля. Нет пещер — порог выше всех.
                caverns: heights
                    .clone()
                    .map(|y| match cavern_share(y) {
                        share if share <= 0.0 => f64::INFINITY,
                        share => quantile(&caverns, 1.0 - share),
                    })
                    .collect(),
                // Ходы: пусто ниже порога.
                tunnels: heights
                    .clone()
                    .map(|y| match tunnel_share(y) {
                        share if share <= 0.0 => f64::NEG_INFINITY,
                        share => quantile(&tunnels, share),
                    })
                    .collect(),
                delta_netherrack: heights.map(|y| quantile(&rocks, delta_netherrack_share(y))).collect(),
                delta_blackstone: quantile(&rocks, 1.0 - DELTA_BLACKSTONE),
            }
        })
    }

    /// Что в этом месте рельефа: порода, большая пещера или узкий ход.
    fn void(&self, x: i32, y: i32, z: i32) -> Void {
        if !(0..=CEILING).contains(&y) {
            return Void::Rock;
        }

        let thresholds = self.thresholds();
        let (cavern, tunnel) = (thresholds.caverns[y as usize], thresholds.tunnels[y as usize]);

        // Где пещер или ходов на этой высоте нет вовсе, их шум не считается.
        if cavern.is_finite() && self.cavern(x, y, z) > cavern {
            Void::Cavern
        } else if tunnel.is_finite() && self.tunnel(x, y, z) < tunnel {
            Void::Tunnel
        } else {
            Void::Rock
        }
    }

    /// Пусто ли в этом месте.
    fn hollow(&self, x: i32, y: i32, z: i32) -> bool {
        self.void(x, y, z) != Void::Rock
    }

    /// Бедрок у пола и потолка: крайний слой сплошной, дальше реже.
    fn bedrock(&self, x: i32, y: i32, z: i32) -> bool {
        let depth = y.min(CEILING - y);

        if !(0..BEDROCK_LAYERS).contains(&depth) {
            return false;
        }

        let roll = scramble(
            self.seed as u64
                ^ (x as u64).wrapping_mul(0x9E37_79B9)
                ^ (y as u64).wrapping_mul(0x85EB_CA6B)
                ^ (z as u64).wrapping_mul(0xC2B2_AE35),
        );

        (roll % BEDROCK_LAYERS as u64) as i32 >= depth
    }

    /// Открыто ли место для объектов — пусто и не бедрок, над морем лавы
    /// или в нём (решается только по координатам).
    fn open(&self, x: i32, y: i32, z: i32) -> bool {
        (1..CEILING).contains(&y) && !self.bedrock(x, y, z) && self.hollow(x, y, z)
    }

    /// Сплошная ли порода в этом месте рельефа (без бедрока).
    fn rock_at(&self, x: i32, y: i32, z: i32) -> bool {
        (1..CEILING).contains(&y) && !self.bedrock(x, y, z) && !self.hollow(x, y, z)
    }

    /// Климат столбца: жар и сырость.
    fn climate(&self, x: i32, z: i32) -> (f64, f64) {
        let (fx, fz) = (f64::from(x) / CLIMATE_SCALE, f64::from(z) / CLIMATE_SCALE);

        (self.heat.octaves(fx + 0.31, fz + 0.77, 4), self.damp.octaves(fx + 0.53, fz + 0.19, 4))
    }

    /// Какой биом из пяти ближе всего к этому климату — со сдвигами,
    /// подогнанными под доли площади.
    fn nearest(climate: (f64, f64), spread: f64, weights: &[f64; 5]) -> usize {
        let (heat, damp) = (climate.0 / spread, climate.1 / spread);
        let mut best = (f64::INFINITY, 0);

        for (at, (place, weight)) in PLACES.iter().zip(weights).enumerate() {
            let distance = (heat - place.0).powi(2) + (damp - place.1).powi(2) - weight;

            if distance < best.0 {
                best = (distance, at);
            }
        }

        best.1
    }

    /// Подгонка климата: по выборке мест находит разброс шума и такие
    /// сдвиги биомов, чтобы доли площади сошлись с `SHARES`.
    fn fit(&self) -> &Fit {
        self.fit.get_or_init(|| {
            const SAMPLES: usize = 30_000;

            let mut state = (self.seed as u64) ^ 0x0B10_3E5A;
            let mut next = || {
                state = scramble(state.wrapping_add(0x9E37_79B9_7F4A_7C15));
                (state % 8_000_000) as i32 - 4_000_000
            };
            let samples: Vec<(f64, f64)> = (0..SAMPLES).map(|_| self.climate(next(), next())).collect();
            let spread = (samples.iter().map(|(h, d)| h * h + d * d).sum::<f64>() / (2 * SAMPLES) as f64).sqrt();
            let mut weights = [0.0; 5];

            for _ in 0..400 {
                let mut counts = [0usize; 5];

                for climate in &samples {
                    counts[Nether::nearest(*climate, spread, &weights)] += 1;
                }

                for at in 0..5 {
                    weights[at] += 0.5 * (SHARES[at] - counts[at] as f64 / SAMPLES as f64);
                }
            }

            Fit { spread, weights }
        })
    }

    /// Биом столбца.
    pub fn biome_at(&self, x: i32, z: i32) -> Biome {
        let fit = self.fit();

        Biome::NETHER[Nether::nearest(self.climate(x, z), fit.spread, &fit.weights)]
    }

    /// Число для решения в этом месте: своё для каждой точки и соли.
    fn hash(&self, salt: u64, x: i32, y: i32, z: i32) -> u64 {
        scramble(
            (self.seed as u64)
                ^ salt.wrapping_mul(0xD6E8_FEB8_6659_FD93)
                ^ (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
                ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F)
                ^ (z as u64).wrapping_mul(0x1656_67B1_9E37_79F9),
        )
    }

    /// Выпало ли событие с вероятностью `part` в этом месте.
    fn chance(&self, salt: u64, x: i32, y: i32, z: i32, part: f64) -> bool {
        unit(self.hash(salt, x, y, z)) < part
    }

    /// Кости для объектов чанка: свои для каждого чанка и вида объекта.
    fn dice(&self, salt: u64, chunk_x: i32, chunk_z: i32) -> Dice {
        Dice(self.hash(salt, chunk_x, 0x5EED, chunk_z))
    }

    /// Чанк Незера.
    pub(super) fn chunk(&self, chunk_x: i32, chunk_z: i32) -> Chunk {
        let piece = self.piece(chunk_x, chunk_z);
        let mut chunk = Chunk::new();

        // Кусок уложен так же, как секции: по высоте, внутри слоя по Z,
        // потом по X, — поэтому переносится слоями по 16 блоков.
        for section_y in 0..=CEILING / CHUNK_SIZE {
            let from = (section_y * CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE) as usize;
            let blocks = &piece.blocks[from..from + (CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE) as usize];

            if blocks.iter().all(|state| *state == AIR || *state == TUNNEL) {
                continue;
            }

            let (section, _) = split(section_y * CHUNK_SIZE);
            let target = chunk.filled_section(section);

            for (place, state) in target.iter_mut().zip(blocks) {
                *place = pack(if *state == TUNNEL { AIR } else { *state });
            }
        }

        // Биом по высоте не меняется: весь столбец клеток — один биом,
        // его решает середина клетки.
        for cell_x in 0..BIOME_CELLS {
            for cell_z in 0..BIOME_CELLS {
                let x = chunk_x * CHUNK_SIZE + cell_x as i32 * 4 + 2;
                let z = chunk_z * CHUNK_SIZE + cell_z as i32 * 4 + 2;
                let biome = self.biome_at(x, z).number() as BiomeId;

                for layer in 0..BIOME_LAYERS {
                    chunk.biomes[biome_cell(layer, cell_x, cell_z)] = biome;
                }
            }
        }

        chunk
    }

    /// Блоки чанка на высотах 0..=127: рельеф, поверхность, руда, объекты.
    fn piece(&self, chunk_x: i32, chunk_z: i32) -> Piece {
        let mut piece = Piece {
            x0: chunk_x * CHUNK_SIZE,
            z0: chunk_z * CHUNK_SIZE,
            blocks: vec![AIR; PIECE],
        };
        let mut biomes = [Biome::NetherWastes; (CHUNK_SIZE * CHUNK_SIZE) as usize];

        for (at, biome) in biomes.iter_mut().enumerate() {
            let (x, z) = (piece.x0 + at as i32 % CHUNK_SIZE, piece.z0 + at as i32 / CHUNK_SIZE);
            *biome = self.biome_at(x, z);
        }

        self.relief(&mut piece, &biomes);
        self.surface(&mut piece, &biomes);
        self.blobs(&mut piece, chunk_x, chunk_z);
        self.features(&mut piece, chunk_x, chunk_z);
        self.decorate(&mut piece, &biomes);
        self.springs(&mut piece, chunk_x, chunk_z);

        piece
    }

    /// Рельеф: бедрок, порода, лава моря и карманов, пустоты.
    fn relief(&self, piece: &mut Piece, biomes: &[Biome]) {
        let states = &self.states;
        let thresholds = self.thresholds();

        for local_z in 0..CHUNK_SIZE {
            for local_x in 0..CHUNK_SIZE {
                let (x, z) = (piece.x0 + local_x, piece.z0 + local_z);
                let deltas = biomes[(local_z * CHUNK_SIZE + local_x) as usize] == Biome::BasaltDeltas;

                for y in 0..=CEILING {
                    let void = if self.bedrock(x, y, z) { Void::Rock } else { self.void(x, y, z) };
                    let state = if self.bedrock(x, y, z) {
                        states.bedrock
                    } else if void == Void::Rock {
                        if deltas {
                            // Дельты сложены базальтом с пятнами чернокамня;
                            // незеррак — к потолку.
                            match self.rock(x, y, z) {
                                rock if rock < thresholds.delta_netherrack[y as usize] => states.netherrack,
                                rock if rock > thresholds.delta_blackstone => states.blackstone,
                                _ => states.basalt,
                            }
                        } else {
                            states.netherrack
                        }
                    } else if y <= LAVA_SEA {
                        states.lava
                    } else if void == Void::Tunnel {
                        TUNNEL
                    } else {
                        AIR
                    };

                    piece.blocks[Piece::index(local_x, y, local_z)] = state;
                }
            }
        }
    }

    /// Поверхность по биомам: что лежит на полу и что на потолке пустот.
    ///
    /// Замер оригинала (верх пола / низ потолка):
    /// — пустоши: незеррак; у моря лавы (пол y 26..35) — пятна песка душ
    ///   (~30 %) и гравия (~15 %);
    /// — долина душ: песок и почва душ пополам (~80 %) слоем в 2–4 блока,
    ///   на потолке так же; у моря ещё и гравий (~25 %);
    /// — леса: нилий в один блок на полу (~80 %);
    /// — дельты: базальт и чернокамень, магма (~8 %), потолок — базальт.
    fn surface(&self, piece: &mut Piece, biomes: &[Biome]) {
        let states = &self.states;

        for local_z in 0..CHUNK_SIZE {
            for local_x in 0..CHUNK_SIZE {
                let (x, z) = (piece.x0 + local_x, piece.z0 + local_z);
                let biome = biomes[(local_z * CHUNK_SIZE + local_x) as usize];
                let (fx, fz) = (f64::from(x), f64::from(z));

                for y in 1..CEILING {
                    let here = piece.blocks[Piece::index(local_x, y, local_z)];

                    if !states.is_rock(here) {
                        continue;
                    }

                    let above = piece.blocks[Piece::index(local_x, y + 1, local_z)];
                    let below = piece.blocks[Piece::index(local_x, y - 1, local_z)];
                    let floor = above == AIR || (above == states.lava && biome != Biome::CrimsonForest && biome != Biome::WarpedForest);
                    let ceiling = below == AIR;
                    let shore = floor && (26..=35).contains(&y);

                    if floor {
                        let fy = f64::from(y);
                        let patch = self.patches.octaves3(fx / 9.0 + 0.5, fy / 5.0 + 0.1, fz / 9.0 + 0.3, 2);
                        // Глубина слоя: 1–3 блока, свой для каждого места.
                        let depth = 1 + (self.hash(0xDE97, x, y, z) % 3) as i32;

                        let top = match biome {
                            Biome::NetherWastes if shore => match patch {
                                p if p > 0.06 => Some((states.soul_sand, depth)),
                                p if p < -0.2 => Some((states.gravel, depth)),
                                _ => None,
                            },
                            Biome::SoulSandValley => {
                                // Гравий у моря — своими пятнами, не
                                // в ущерб одному из двух песков.
                                let gravel = self.patches.octaves3(fx / 9.0 + 7.3, fy / 5.0 + 2.9, fz / 9.0 + 4.1, 2);

                                if shore && gravel < -0.12 {
                                    Some((states.gravel, depth))
                                } else if self.specks.octaves3(fx / 14.0, fy / 9.0, fz / 14.0, 2) < -0.4 {
                                    None
                                } else {
                                    // Песок или почва — пятнами; слой толще,
                                    // чем у прочих: 2–4 блока.
                                    let soul = if patch > 0.0 { states.soul_sand } else { states.soul_soil };
                                    Some((soul, depth + 1))
                                }
                            }
                            Biome::CrimsonForest | Biome::WarpedForest if above == AIR => {
                                if self.specks.octaves3(fx / 6.0, fy / 6.0, fz / 6.0, 2) < -0.38 {
                                    None
                                } else if biome == Biome::CrimsonForest {
                                    Some((states.crimson_nylium, 1))
                                } else {
                                    Some((states.warped_nylium, 1))
                                }
                            }
                            Biome::BasaltDeltas => {
                                if shore && patch < -0.3 {
                                    Some((states.gravel, 1))
                                } else if above == AIR && self.specks.octaves3(fx / 5.0, fy / 5.0, fz / 5.0, 2) > 0.38 {
                                    Some((states.magma, 1))
                                } else if here == states.netherrack {
                                    // На полу дельт незеррака почти нет.
                                    Some((states.basalt, 1))
                                } else {
                                    None
                                }
                            }
                            _ => None,
                        };

                        if let Some((state, depth)) = top {
                            for down in 0..depth {
                                let at = Piece::index(local_x, y - down, local_z);

                                if y - down < 1 || !states.is_rock(piece.blocks[at]) {
                                    break;
                                }

                                piece.blocks[at] = state;
                            }
                        }
                    }

                    if ceiling {
                        match biome {
                            Biome::SoulSandValley => {
                                let (fx, fy) = (f64::from(x), f64::from(y));

                                if self.specks.octaves3(fx / 14.0, fy / 9.0, fz / 14.0, 2) >= -0.4 {
                                    let patch = self.patches.octaves3(fx / 9.0 + 0.5, fy / 5.0 + 0.1, fz / 9.0 + 0.3, 2);
                                    let soul = if patch > 0.0 { states.soul_sand } else { states.soul_soil };
                                    let depth = 1 + (self.hash(0xCE11, x, y, z) % 2) as i32;

                                    for up in 0..depth {
                                        let at = Piece::index(local_x, y + up, local_z);

                                        if y + up >= CEILING || !states.is_rock(piece.blocks[at]) {
                                            break;
                                        }

                                        piece.blocks[at] = soul;
                                    }
                                }
                            }
                            Biome::BasaltDeltas => {
                                piece.blocks[Piece::index(local_x, y, local_z)] = states.basalt;
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    /// Пятна в толще: руда, гравий, чернокамень, магма, базальт. Каждое
    /// пятно заменяет только незеррак (руда в дельтах — ещё и базальт
    /// не трогает). Пятна соседних чанков, заходящие в этот, рисуются по
    /// тем же числам.
    fn blobs(&self, piece: &mut Piece, chunk_x: i32, chunk_z: i32) {
        let states = &self.states;

        for near_x in chunk_x - 1..=chunk_x + 1 {
            for near_z in chunk_z - 1..=chunk_z + 1 {
                let biome = self.biome_at(near_x * CHUNK_SIZE + 8, near_z * CHUNK_SIZE + 8);

                for (salt, kind) in BLOBS.iter().enumerate() {
                    let tries = if biome == Biome::BasaltDeltas { kind.deltas } else { kind.tries };
                    let tries = if kind.state == Blob::BASALT && matches!(biome, Biome::CrimsonForest | Biome::BasaltDeltas) {
                        0.0
                    } else {
                        tries
                    };
                    let mut dice = self.dice(0xB10B + salt as u64, near_x, near_z);
                    // Дробная часть числа попыток — вероятность ещё одной.
                    let count = tries.floor() as i32 + i32::from(dice.unit() < tries.fract());

                    for _ in 0..count {
                        let x = near_x * CHUNK_SIZE + dice.below(CHUNK_SIZE as u32);
                        let z = near_z * CHUNK_SIZE + dice.below(CHUNK_SIZE as u32);
                        let y = kind.low + dice.below((kind.high - kind.low + 1) as u32);
                        let size = kind.size.0 + dice.unit() * (kind.size.1 - kind.size.0);
                        let radius = (size * 0.75 / std::f64::consts::PI).cbrt();
                        let reach = radius.ceil() as i32 + 1;

                        if !piece.near(x, z, reach) {
                            // Вся порода пятна за краем чанка: кости уже
                            // брошены, дальше считать нечего.
                            continue;
                        }

                        let state = match kind.state {
                            Blob::QUARTZ => states.quartz,
                            Blob::GOLD => states.gold,
                            Blob::DEBRIS => states.debris,
                            Blob::GRAVEL => states.gravel,
                            Blob::BLACKSTONE => states.blackstone,
                            Blob::MAGMA => states.magma,
                            _ => states.basalt,
                        };
                        // Сплюснутость и наклон пятна — свои у каждого.
                        let (sx, sy, sz) = (0.7 + dice.unit() * 0.6, 0.6 + dice.unit() * 0.5, 0.7 + dice.unit() * 0.6);

                        for dx in -reach..=reach {
                            for dy in -reach..=reach {
                                for dz in -reach..=reach {
                                    let (bx, by, bz) = (x + dx, y + dy, z + dz);
                                    let (fx, fy, fz) = (f64::from(dx) / sx, f64::from(dy) / sy, f64::from(dz) / sz);
                                    let distance = (fx * fx + fy * fy + fz * fz).sqrt();
                                    let edge = radius * (0.8 + 0.4 * unit(self.hash(0xED6E, bx, by, bz)));

                                    if distance > edge {
                                        continue;
                                    }

                                    if let Some(at) = piece.at(bx, by, bz)
                                        && piece.blocks[at] == states.netherrack
                                    {
                                        piece.blocks[at] = state;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Крупные объекты: столбы базальта, огромные грибы, светокамень,
    /// лавовые лужи и столбики дельт. Порядок обхода — по чанкам-источникам
    /// и видам, одинаковый для всех чанков: объекты ставятся только в
    /// пустоту, и кто раньше встал, того и место — на обе стороны стыка.
    fn features(&self, piece: &mut Piece, chunk_x: i32, chunk_z: i32) {
        for near_x in chunk_x - 1..=chunk_x + 1 {
            for near_z in chunk_z - 1..=chunk_z + 1 {
                self.pillars(piece, near_x, near_z);
                self.fungi(piece, near_x, near_z);
                self.glowstone(piece, near_x, near_z);
                self.delta_columns(piece, near_x, near_z);
            }
        }

        self.delta_pools(piece);
    }

    /// Пол под этим местом: первый сверху вниз сплошной блок над пустотой,
    /// не ниже моря лавы. Решается только по координатам.
    fn floor_below(&self, x: i32, from: i32, z: i32) -> Option<i32> {
        if !self.open(x, from, z) {
            return None;
        }

        (LAVA_SEA..from).rev().find(|&y| self.rock_at(x, y, z)).filter(|&y| y >= LAVA_SEA && self.open(x, y + 1, z))
    }

    /// Потолок над этим местом: первый снизу вверх сплошной блок.
    fn ceiling_above(&self, x: i32, from: i32, z: i32) -> Option<i32> {
        if !self.open(x, from, z) {
            return None;
        }

        (from + 1..CEILING).find(|&y| !self.open(x, y, z)).filter(|&y| self.rock_at(x, y, z))
    }

    /// Столбы базальта долины душ: от пола до потолка, с наростами по бокам.
    /// Замер: базальта в долине ~0,3 блока на столбец, часть — в столбах.
    fn pillars(&self, piece: &mut Piece, chunk_x: i32, chunk_z: i32) {
        let mut dice = self.dice(0x9111A5, chunk_x, chunk_z);

        if dice.unit() >= 0.6 {
            return;
        }

        let x = chunk_x * CHUNK_SIZE + dice.below(CHUNK_SIZE as u32);
        let z = chunk_z * CHUNK_SIZE + dice.below(CHUNK_SIZE as u32);
        let y = LAVA_SEA + 2 + dice.below(70);

        if !piece.near(x, z, 2) || self.biome_at(x, z) != Biome::SoulSandValley {
            return;
        }

        let (Some(floor), Some(ceiling)) = (self.floor_below(x, y, z), self.ceiling_above(x, y, z)) else {
            return;
        };

        if ceiling - floor > 48 {
            return;
        }

        let basalt = self.states.basalt;

        for y in floor + 1..ceiling {
            piece.put_into_air(x, y, z, basalt);

            // Наросты: у подножия и под потолком чаще.
            let edge = (y - floor).min(ceiling - y);
            let part = if edge <= 2 { 0.7 } else { 0.18 };

            for (side, (dx, dz)) in [(1, 0), (-1, 0), (0, 1), (0, -1)].into_iter().enumerate() {
                if self.chance(0x51DE + side as u64, x, y, z, part) {
                    piece.put_into_air(x + dx, y, z + dz, basalt);
                }
            }
        }
    }

    /// Огромные грибы багрового и искажённого лесов: ствол 4–13 блоков
    /// (изредка вдвое выше), шляпа из наростов до трёх блоков от ствола,
    /// в ней светогрибы; с багровой шляпы свисают лозы (вики, Huge fungus).
    /// Частота — по замеру: на столбец багрового леса ~0,23 блока стебля
    /// и ~1,2 нароста.
    fn fungi(&self, piece: &mut Piece, chunk_x: i32, chunk_z: i32) {
        let states = &self.states;
        let mut dice = self.dice(0xF0461, chunk_x, chunk_z);

        for _ in 0..FUNGUS_TRIES {
            let x = chunk_x * CHUNK_SIZE + dice.below(CHUNK_SIZE as u32);
            let z = chunk_z * CHUNK_SIZE + dice.below(CHUNK_SIZE as u32);
            let from = LAVA_SEA + 1 + dice.below(80);
            let mut height = 4 + dice.below(10);
            let doubled = dice.below(12) == 0;
            let thick = dice.below(16) == 0;
            let seed = dice.next();

            if doubled {
                height *= 2;
            }

            if !piece.near(x, z, 5) {
                continue;
            }

            let biome = self.biome_at(x, z);
            let (stem, wart) = match biome {
                Biome::CrimsonForest => (states.crimson_stem, states.nether_wart),
                Biome::WarpedForest => (states.warped_stem, states.warped_wart),
                _ => continue,
            };
            let Some(floor) = self.floor_below(x, from, z) else {
                continue;
            };

            // Ствол не упирается в потолок: не хватает места — гриб ниже.
            let room = (floor + 1..floor + 1 + height + 2).take_while(|&y| self.open(x, y, z)).count() as i32;

            if room < 6 {
                continue;
            }

            let height = height.min(room - 2);
            let top = floor + height;
            let mut roll = Dice(seed);

            for y in floor + 1..=top {
                piece.put_into_air(x, y, z, stem);

                if thick && y <= floor + 2 {
                    for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
                        piece.put_into_air(x + dx, y, z + dz, stem);
                    }
                }
            }

            // Шляпа: сверху сплошная крышка, ниже — кольца по краю, пустые
            // внутри; чем ниже, тем шире и реже.
            let radius = if height >= 8 { 3 } else { 2 };
            let depth = (height / 3 + 1).clamp(2, 5);

            for layer in 0..=depth {
                let y = top + 1 - layer;
                let ring = (1 + layer).min(radius);

                for dx in -ring..=ring {
                    for dz in -ring..=ring {
                        let edge = dx.abs().max(dz.abs());
                        let corner = dx.abs() == ring && dz.abs() == ring;
                        let keep = if layer == 0 {
                            !corner || roll.unit() < 0.3
                        } else if edge == ring {
                            roll.unit() < if corner { 0.3 } else { 0.92 - 0.06 * layer as f64 }
                        } else {
                            false
                        };

                        if !keep || (dx == 0 && dz == 0 && layer > 0) {
                            continue;
                        }

                        let block = if roll.unit() < 0.035 { states.shroomlight } else { wart };

                        piece.put_into_air(x + dx, y, z + dz, block);

                        // Лозы с нижнего края багровой шляпы.
                        if biome == Biome::CrimsonForest && layer == depth && roll.unit() < 0.1 {
                            let length = 1 + roll.below(4);

                            for down in 1..=length {
                                let tip = down == length;
                                let vine = if tip { states.weeping_vines } else { states.weeping_vines_plant };

                                if !piece.put_into_air(x + dx, y - down, z + dz, vine) {
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    /// Светокамень: грозди под потолком. Замер: ~0,015 блока на столбец
    /// во всех биомах, кроме долины душ (там в пять раз реже); чаще всего
    /// у самого верха пещер, y 100–110.
    fn glowstone(&self, piece: &mut Piece, chunk_x: i32, chunk_z: i32) {
        let mut dice = self.dice(0x6710, chunk_x, chunk_z);
        let tries = GLOWSTONE_TRIES.floor() as i32 + i32::from(dice.unit() < GLOWSTONE_TRIES.fract());

        for _ in 0..tries {
            let x = chunk_x * CHUNK_SIZE + dice.below(CHUNK_SIZE as u32);
            let z = chunk_z * CHUNK_SIZE + dice.below(CHUNK_SIZE as u32);
            let from = LAVA_SEA + 2 + dice.below(90);
            let size = 8 + dice.below(40);
            let seed = dice.next();

            if !piece.near(x, z, 5) {
                continue;
            }

            if self.biome_at(x, z) == Biome::SoulSandValley && Dice(seed).unit() < 0.8 {
                continue;
            }

            let Some(ceiling) = self.ceiling_above(x, from, z) else {
                continue;
            };

            // Гроздь растёт от потолка вниз: каждый новый блок — сосед
            // одного из прежних.
            let mut roll = Dice(seed ^ 0x6105);
            let mut cluster = vec![(x, ceiling - 1, z)];

            for _ in 0..size {
                let (px, py, pz) = cluster[roll.below(cluster.len() as u32) as usize];
                let (dx, dy, dz) = match roll.below(6) {
                    0 => (1, 0, 0),
                    1 => (-1, 0, 0),
                    2 => (0, 0, 1),
                    3 => (0, 0, -1),
                    _ => (0, -1, 0),
                };
                let next = (px + dx, py + dy, pz + dz);

                if (next.0 - x).abs() > 3 || (next.2 - z).abs() > 3 || ceiling - next.1 > 8 {
                    continue;
                }

                cluster.push(next);
            }

            for (bx, by, bz) in cluster {
                piece.put_into_air(bx, by, bz, self.states.glowstone);
            }
        }
    }

    /// Столбики базальта на полу дельт: по нескольку рядом, высотой 1–5,
    /// изредка выше.
    fn delta_columns(&self, piece: &mut Piece, chunk_x: i32, chunk_z: i32) {
        let mut dice = self.dice(0xC01, chunk_x, chunk_z);

        for _ in 0..DELTA_COLUMN_TRIES {
            let x = chunk_x * CHUNK_SIZE + dice.below(CHUNK_SIZE as u32);
            let z = chunk_z * CHUNK_SIZE + dice.below(CHUNK_SIZE as u32);
            let from = LAVA_SEA + 1 + dice.below(80);
            let tall = dice.below(6) == 0;
            let seed = dice.next();

            if !piece.near(x, z, 3) || self.biome_at(x, z) != Biome::BasaltDeltas {
                continue;
            }

            let mut roll = Dice(seed);

            for _ in 0..3 + roll.below(5) {
                let (cx, cz) = (x + roll.below(5) - 2, z + roll.below(5) - 2);
                let height = if tall { 4 + roll.below(7) } else { 1 + roll.below(4) };
                let Some(floor) = self.floor_below(cx, from, cz) else {
                    continue;
                };

                for y in floor + 1..=floor + height {
                    if !self.open(cx, y, cz) {
                        break;
                    }

                    piece.put_into_air(cx, y, cz, self.states.basalt);
                }
            }
        }
    }

    /// Лавовые лужи дельт: верх пола в пятнах становится лавой, край лужи —
    /// магмой. Лава ставится только там, где по бокам и снизу сплошная
    /// порода рельефа: лужа не растекается.
    fn delta_pools(&self, piece: &mut Piece) {
        let states = &self.states;

        for local_z in 0..CHUNK_SIZE {
            for local_x in 0..CHUNK_SIZE {
                let (x, z) = (piece.x0 + local_x, piece.z0 + local_z);
                let (fx, fz) = (f64::from(x), f64::from(z));

                for y in LAVA_SEA + 1..CEILING - 1 {
                    let at = Piece::index(local_x, y, local_z);

                    if !states.is_rock(piece.blocks[at]) || piece.blocks[Piece::index(local_x, y + 1, local_z)] != AIR {
                        continue;
                    }

                    let pool = self.rocks.octaves3(fx / 7.0 + 3.1, f64::from(y) / 4.0, fz / 7.0 + 5.3, 2);

                    if pool < 0.2 || self.biome_at(x, z) != Biome::BasaltDeltas {
                        continue;
                    }

                    let enclosed = [(1, 0), (-1, 0), (0, 1), (0, -1)]
                        .iter()
                        .all(|(dx, dz)| self.rock_at(x + dx, y, z + dz))
                        && self.rock_at(x, y - 1, z);

                    piece.blocks[at] = if enclosed && pool > 0.27 { states.lava } else { states.magma };
                }
            }
        }
    }

    /// Мелочь на полу и потолке этого чанка: огонь, грибы, корни, ростки,
    /// лозы. Всё — в один блок шириной, поэтому только в своём чанке.
    ///
    /// Частоты — замер оригинала, штук на столбец биома: багровый лес —
    /// корни 0,12, грибы 0,015, плачущие лозы 0,04 (блоков 0,14);
    /// искажённый — корни 0,11, ростки 0,09, грибы 0,017, вьющиеся лозы
    /// 0,011 (блоков 0,045); пустоши — огонь 0,009, грибы по 0,001; долина —
    /// огонь душ 0,004, огонь 0,002, багровые корни 0,002.
    fn decorate(&self, piece: &mut Piece, biomes: &[Biome]) {
        let states = &self.states;

        for local_z in 0..CHUNK_SIZE {
            for local_x in 0..CHUNK_SIZE {
                let (x, z) = (piece.x0 + local_x, piece.z0 + local_z);
                let biome = biomes[(local_z * CHUNK_SIZE + local_x) as usize];

                for y in LAVA_SEA..CEILING - 1 {
                    let here = piece.blocks[Piece::index(local_x, y, local_z)];
                    let above = piece.blocks[Piece::index(local_x, y + 1, local_z)];

                    if is_air(above) && !is_air(here) && here != states.lava {
                        let roll = unit(self.hash(0xF100, x, y, z));
                        let plant = self.floor_plant(biome, here, roll);

                        if let Some(plant) = plant {
                            if plant == states.twisting_vines {
                                let length = 1 + (self.hash(0x7115, x, y, z) % 7) as i32;

                                for up in 1..=length {
                                    let vine = if up == length { states.twisting_vines } else { states.twisting_vines_plant };

                                    if !piece.put_into_air(x, y + up, z, vine) {
                                        break;
                                    }
                                }
                            } else {
                                piece.blocks[Piece::index(local_x, y + 1, local_z)] = plant;
                            }
                        }
                    }

                    // Плачущие лозы с потолка багрового леса, вокруг
                    // места, откуда растут, — наросты на потолке.
                    let below = piece.blocks[Piece::index(local_x, y - 1, local_z)];

                    if biome == Biome::CrimsonForest
                        && is_air(below)
                        && states.is_rock(here)
                        && self.chance(0x3EE9, x, y, z, WEEPING_PART)
                    {
                        piece.blocks[Piece::index(local_x, y, local_z)] = states.nether_wart;

                        let length = 1 + (self.hash(0x1E46, x, y, z) % 6) as i32;

                        for down in 1..=length {
                            let vine = if down == length { states.weeping_vines } else { states.weeping_vines_plant };

                            if !piece.put_into_air(x, y - down, z, vine) {
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    /// Лава в стенах над морем: источник в породе, открытый в пустоту одной
    /// стороной (потечёт, как только чанк ляжет в мир), или замурованный
    /// со всех сторон. Замер: ~0,025 блока лавы на столбец выше y 31 во всех
    /// биомах, ровно по высоте.
    fn springs(&self, piece: &mut Piece, chunk_x: i32, chunk_z: i32) {
        let mut dice = self.dice(0x5981, chunk_x, chunk_z);

        for _ in 0..SPRING_TRIES {
            let x = piece.x0 + dice.below(CHUNK_SIZE as u32);
            let z = piece.z0 + dice.below(CHUNK_SIZE as u32);
            let y = LAVA_SEA + 1 + dice.below((CEILING - LAVA_SEA - 5) as u32);
            let Some(at) = piece.at(x, y, z) else {
                continue;
            };

            if !self.states.is_rock(piece.blocks[at]) || !self.rock_at(x, y + 1, z) || !self.rock_at(x, y - 1, z) {
                continue;
            }

            let open = [(1, 0), (-1, 0), (0, 1), (0, -1)].iter().filter(|(dx, dz)| self.open(x + dx, y, z + dz)).count();

            if open <= 1 {
                piece.blocks[at] = self.states.lava;
            }
        }
    }

    /// Что вырастает на этом полу: по биому, блоку пола и броску.
    fn floor_plant(&self, biome: Biome, floor: i32, roll: f64) -> Option<i32> {
        let states = &self.states;
        let pick = |table: &[(f64, i32)]| {
            let mut edge = 0.0;

            table.iter().find(|(part, _)| {
                edge += part;
                roll < edge
            })
            .map(|(_, state)| *state)
        };

        match biome {
            Biome::CrimsonForest if floor == states.crimson_nylium => pick(&[
                (0.08, states.crimson_roots),
                (0.010, states.crimson_fungus),
                (0.001, states.warped_fungus),
            ]),
            Biome::WarpedForest if floor == states.warped_nylium => pick(&[
                (0.072, states.warped_roots),
                (0.056, states.sprouts),
                (0.011, states.warped_fungus),
                (0.0085, states.twisting_vines),
                (0.0015, states.crimson_roots),
            ]),
            Biome::NetherWastes if floor == states.netherrack => pick(&[
                (0.0037, states.fire),
                (0.0009, states.brown_mushroom),
                (0.0007, states.red_mushroom),
            ]),
            Biome::SoulSandValley if floor == states.soul_soil || floor == states.soul_sand => {
                pick(&[(0.0023, states.soul_fire), (0.0013, states.crimson_roots)])
            }
            Biome::SoulSandValley if floor == states.netherrack => pick(&[(0.003, states.fire)]),
            Biome::BasaltDeltas if floor == states.basalt || floor == states.blackstone => {
                pick(&[(0.0005, states.brown_mushroom), (0.0005, states.red_mushroom)])
            }
            _ => None,
        }
    }

    /// Высота, на которой можно стоять в этом столбце: первое место снизу
    /// над морем лавы, где есть пол, а над ним два блока пустоты.
    #[allow(dead_code)] // для порталов: где построить парный
    pub fn standing_height(&self, x: i32, z: i32) -> Option<i32> {
        (LAVA_SEA + 1..CEILING - 2).find(|&y| {
            !self.hollow(x, y - 1, z) && !self.bedrock(x, y - 1, z) && self.hollow(x, y, z) && self.hollow(x, y + 1, z)
        })
    }
}

/// Попыток лавы в стене на чанк.
const SPRING_TRIES: usize = 12;

/// Попыток огромного гриба на чанк.
const FUNGUS_TRIES: usize = 16;

/// Попыток грозди светокамня на чанк.
const GLOWSTONE_TRIES: f64 = 0.5;

/// Попыток кучки столбиков на чанк дельт.
const DELTA_COLUMN_TRIES: usize = 4;

/// Доля потолка багрового леса, откуда свисают лозы.
const WEEPING_PART: f64 = 0.011;

/// Пятно в толще: что, сколько попыток на чанк (в дельтах — своё число),
/// на каких высотах и какого размера (блоков, от и до).
struct Blob {
    state: u8,
    tries: f64,
    deltas: f64,
    low: i32,
    high: i32,
    size: (f64, f64),
}

impl Blob {
    const QUARTZ: u8 = 0;
    const GOLD: u8 = 1;
    const DEBRIS: u8 = 2;
    const GRAVEL: u8 = 3;
    const BLACKSTONE: u8 = 4;
    const MAGMA: u8 = 5;
    const BASALT: u8 = 6;
}

/// Пятна в толще Незера. Высоты и объёмы — по замеру оригинала, блоков на
/// столбец пустошей: кварц 0,5 (y 8–117), золото 0,18, древние обломки
/// 0,006 (y 8–24), гравий 0,6 (y 5–41), чернокамень 0,74 (y 5–31), магма
/// 0,77 (y 26–37), базальт 0,4 (кроме багрового леса и дельт).
const BLOBS: [Blob; 7] = [
    Blob { state: Blob::QUARTZ, tries: 26.0, deltas: 52.0, low: 10, high: 117, size: (6.0, 16.0) },
    Blob { state: Blob::GOLD, tries: 14.0, deltas: 28.0, low: 10, high: 117, size: (4.0, 10.0) },
    Blob { state: Blob::DEBRIS, tries: 1.2, deltas: 1.2, low: 8, high: 24, size: (1.0, 3.0) },
    Blob { state: Blob::GRAVEL, tries: 5.6, deltas: 3.0, low: 5, high: 41, size: (20.0, 50.0) },
    Blob { state: Blob::BLACKSTONE, tries: 8.0, deltas: 3.0, low: 5, high: 31, size: (20.0, 50.0) },
    Blob { state: Blob::MAGMA, tries: 12.0, deltas: 4.0, low: 26, high: 37, size: (20.0, 50.0) },
    Blob { state: Blob::BASALT, tries: 9.0, deltas: 0.0, low: 5, high: 120, size: (10.0, 40.0) },
];

/// Блоки чанка на высотах 0..=127 — пока он складывается.
struct Piece {
    x0: i32,
    z0: i32,
    blocks: Vec<i32>,
}

impl Piece {
    /// Место блока: по высоте, внутри слоя по Z, потом по X — как в секции.
    fn index(local_x: i32, y: i32, local_z: i32) -> usize {
        ((y * CHUNK_SIZE + local_z) * CHUNK_SIZE + local_x) as usize
    }

    /// Место блока с мировыми координатами, если он в этом чанке.
    fn at(&self, x: i32, y: i32, z: i32) -> Option<usize> {
        let (local_x, local_z) = (x - self.x0, z - self.z0);

        ((0..CHUNK_SIZE).contains(&local_x) && (0..CHUNK_SIZE).contains(&local_z) && (0..=CEILING).contains(&y))
            .then(|| Piece::index(local_x, y, local_z))
    }

    /// Достаёт ли объект с серединой в (x, z) и размахом `reach` до чанка.
    fn near(&self, x: i32, z: i32, reach: i32) -> bool {
        x + reach >= self.x0 && x - reach < self.x0 + CHUNK_SIZE && z + reach >= self.z0 && z - reach < self.z0 + CHUNK_SIZE
    }

    /// Ставит блок в пустоту. Ложь — место занято (или вне мира); за краем
    /// чанка — истина: там его ставит соседний чанк, а объект продолжается.
    fn put_into_air(&mut self, x: i32, y: i32, z: i32, state: i32) -> bool {
        if !(0..=CEILING).contains(&y) {
            return false;
        }

        match self.at(x, y, z) {
            Some(at) if is_air(self.blocks[at]) => {
                self.blocks[at] = state;
                true
            }
            Some(_) => false,
            None => true,
        }
    }
}

/// Кости: повторяемый поток чисел из одного зерна.
struct Dice(u64);

impl Dice {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        scramble(self.0)
    }

    /// Целое от 0 до `bound` − 1.
    fn below(&mut self, bound: u32) -> i32 {
        (self.next() % u64::from(bound.max(1))) as i32
    }

    /// Дробное от 0 до 1.
    fn unit(&mut self) -> f64 {
        unit(self.next())
    }
}

/// Число в дробь от 0 до 1.
fn unit(value: u64) -> f64 {
    (value >> 11) as f64 / (1u64 << 53) as f64
}

/// Квантиль отсортированной выборки.
fn quantile(sorted: &[f64], share: f64) -> f64 {
    let at = ((share.clamp(0.0, 1.0)) * (sorted.len() - 1) as f64).round() as usize;
    sorted[at]
}

/// Перемешивает биты числа — для случайных решений по месту.
fn scramble(mut value: u64) -> u64 {
    value ^= value >> 33;
    value = value.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    value ^= value >> 33;
    value = value.wrapping_mul(0xC4CE_B9FE_1A85_EC53);
    value ^ (value >> 33)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Доли пустот, лавы и бедрока по высотам — против замера оригинала.
    /// `cargo test --release nether_profile -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn nether_profile() {
        let nether = Nether::new(100_554_032_945_340);
        let named = |name: &str| blocks::state_by_name(name).unwrap();
        let (bedrock, lava) = (named("bedrock"), named("lava"));
        let mut counts = vec![[0u64; 4]; 128];
        let chunks = 12;

        for chunk_x in 0..chunks {
            for chunk_z in 0..chunks {
                let chunk = nether.chunk(chunk_x * 7, chunk_z * 7);

                for x in 0..CHUNK_SIZE {
                    for z in 0..CHUNK_SIZE {
                        for y in 0..=CEILING {
                            let state = chunk.block(x, y, z);
                            let kind = if state == AIR {
                                0
                            } else if state == lava {
                                1
                            } else if state == bedrock {
                                2
                            } else {
                                3
                            };
                            counts[y as usize][kind] += 1;
                        }
                    }
                }
            }
        }

        println!("y\tвоздух\tлава\tбедрок");
        for (y, row) in counts.iter().enumerate() {
            let total = row.iter().sum::<u64>() as f64;
            println!("{}\t{:.3}\t{:.3}\t{:.3}", y, row[0] as f64 / total, row[1] as f64 / total, row[2] as f64 / total);
        }
    }

    /// Пол и потолок: сплошной бедрок в крайних слоях, над морем есть где
    /// стоять.
    #[test]
    fn floor_ceiling_and_room() {
        let nether = Nether::new(1);

        assert!(nether.bedrock(3, 0, 5) && nether.bedrock(3, CEILING, 5));
        assert!(!nether.bedrock(3, 60, 5));
        assert!((0..40).any(|x| nether.standing_height(x * 13, x * 7).is_some()));
    }

    /// Все пять биомов встречаются, и доли площади близки к замеру.
    #[test]
    fn biome_shares_follow_the_measurement() {
        let nether = Nether::new(100_554_032_945_340);
        let mut counts = [0usize; 5];
        let side = 160;

        for i in 0..side {
            for j in 0..side {
                let biome = nether.biome_at(i * 97 - 8_000, j * 97 - 8_000);
                counts[Biome::NETHER.iter().position(|b| *b == biome).unwrap()] += 1;
            }
        }

        for (at, count) in counts.iter().enumerate() {
            let share = *count as f64 / (side * side) as f64;
            assert!((share - SHARES[at]).abs() < 0.06, "{:?}: {:.3} против {:.3}", Biome::NETHER[at], share, SHARES[at]);
        }
    }

    /// Блоки чанка зависят только от координат и семени: не от того, какие
    /// чанки сложены до него и каким экземпляром генератора.
    #[test]
    fn chunks_are_decided_by_coordinates() {
        let nether = Nether::new(7);
        let one = nether.piece(3, -2);
        let _ = nether.piece(4, -2);
        let two = Nether::new(7).piece(3, -2);

        assert_eq!(one.blocks, two.blocks);
    }

    /// Биом записан во всех клетках столбца одинаково и совпадает с
    /// решением по середине клетки.
    #[test]
    fn biome_is_the_same_up_the_column() {
        let nether = Nether::new(11);
        let chunk = nether.chunk(5, 9);

        for cell in 0..BIOME_CELLS * BIOME_CELLS {
            let (cell_x, cell_z) = (cell % BIOME_CELLS, cell / BIOME_CELLS);
            let first = chunk.biomes[biome_cell(0, cell_x, cell_z)];

            assert!((0..BIOME_LAYERS).all(|layer| chunk.biomes[biome_cell(layer, cell_x, cell_z)] == first));
            assert_eq!(
                first,
                nether.biome_at(5 * CHUNK_SIZE + cell_x as i32 * 4 + 2, 9 * CHUNK_SIZE + cell_z as i32 * 4 + 2).number()
                    as BiomeId
            );
        }
    }

    /// Перепись Незера — те же доли, что снимает с мира оригинала
    /// tools/measure/nether_biomes.py: биомы, смена биома у соседних клеток
    /// 4×4, верх пола и низ потолка по биомам, мелочь и объём на столбец.
    /// `cargo test --release nether_census -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn nether_census() {
        use std::collections::HashMap;

        let nether = Nether::new(100_554_032_945_340);
        let name = |state: i32| if state == TUNNEL { "cave_air" } else { blocks::block_at_state(state).unwrap_or("?") };
        let open = |state: i32| {
            let block = name(state);
            is_air(state) || matches!(
                block,
                "lava" | "fire" | "soul_fire" | "crimson_roots" | "warped_roots" | "nether_sprouts" | "crimson_fungus"
                    | "warped_fungus" | "brown_mushroom" | "red_mushroom" | "weeping_vines" | "weeping_vines_plant"
                    | "twisting_vines" | "twisting_vines_plant"
            )
        };

        // Биомы и пятна — по клеткам 4×4 на большой площади: 1024 × 1024
        // клетки, как сплошной квадрат замера оригинала и больше.
        let side = 1024;
        let grid: Vec<Biome> = (0..side * side)
            .map(|at| nether.biome_at((at % side) * 4 + 2 - 2048, (at / side) * 4 + 2 - 2048))
            .collect();
        let mut shares: HashMap<&str, usize> = HashMap::new();
        let mut changes = (0, 0);

        for z in 0..side {
            for x in 0..side {
                let here = grid[(z * side + x) as usize];
                *shares.entry(here.name()).or_default() += 1;

                if x + 1 < side {
                    changes.1 += 1;
                    changes.0 += usize::from(grid[(z * side + x + 1) as usize] != here);
                }
                if z + 1 < side {
                    changes.1 += 1;
                    changes.0 += usize::from(grid[((z + 1) * side + x) as usize] != here);
                }
            }
        }

        let mut shares: Vec<_> = shares.into_iter().collect();
        shares.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
        println!(
            "биомы: {}",
            shares
                .iter()
                .map(|(biome, count)| format!("{} {:.1}%", biome, 100.0 * *count as f64 / (side * side) as f64))
                .collect::<Vec<_>>()
                .join(", ")
        );
        println!("смена биома у соседних клеток 4×4: {:.2}%", 100.0 * changes.0 as f64 / changes.1 as f64);

        // Блоки — по чанкам: 16 квадратов 10×10 чанков вразброс.
        let places: Vec<(i32, i32)> = (0..16)
            .flat_map(|area| {
                let (ax, az) = ((area % 4) * 157 - 230, (area / 4) * 211 - 400);
                (0..100).map(move |at| (ax + at % 10, az + at / 10))
            })
            .collect();
        let chunks: Vec<(Piece, Vec<Biome>)> = std::thread::scope(|scope| {
            let workers: Vec<_> = places
                .chunks(places.len().div_ceil(10))
                .map(|part| {
                    let nether = &nether;
                    scope.spawn(move || {
                        part.iter()
                            .map(|&(cx, cz)| {
                                let biomes = (0..256).map(|at| nether.biome_at(cx * 16 + at % 16, cz * 16 + at / 16)).collect();
                                (nether.piece(cx, cz), biomes)
                            })
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            workers.into_iter().flat_map(|worker| worker.join().unwrap()).collect()
        });

        type Table = HashMap<&'static str, HashMap<&'static str, f64>>;
        let (mut floor, mut floor_low, mut ceiling, mut loose, mut volume): (Table, Table, Table, Table, Table) = Default::default();
        let mut columns: HashMap<&str, f64> = HashMap::new();

        for (piece, biomes) in &chunks {
            for at in 0..256 {
                let (lx, lz) = (at % 16, at / 16);
                let biome = biomes[at as usize].name();
                let column: Vec<i32> = (0..=CEILING).map(|y| piece.blocks[Piece::index(lx, y, lz)]).collect();

                *columns.entry(biome).or_default() += 1.0;

                for (y, &state) in column.iter().enumerate() {
                    *volume.entry(biome).or_default().entry(name(state)).or_default() += 1.0;

                    if !is_air(state) && open(state) && name(state) != "lava" {
                        *loose.entry(biome).or_default().entry(name(state)).or_default() += 1.0;
                    }

                    if !(1..127).contains(&y) || open(state) {
                        continue;
                    }

                    let (above, below) = (column[y + 1], column[y - 1]);

                    if open(above) && name(above) != "lava" && y >= 31 {
                        let table = if y <= 34 { &mut floor_low } else { &mut floor };
                        *table.entry(biome).or_default().entry(name(state)).or_default() += 1.0;
                    }
                    if is_air(below) && y >= 32 {
                        *ceiling.entry(biome).or_default().entry(name(state)).or_default() += 1.0;
                    }
                }
            }
        }

        let total: f64 = columns.values().sum();
        println!("столбцов: {} чанков: {}", total, total / 256.0);

        let mut order: Vec<_> = columns.iter().map(|(biome, count)| (*biome, *count)).collect();
        order.sort_by(|a, b| b.1.total_cmp(&a.1));

        let show = |title: &str, table: &Table, per_column: bool, top: usize| {
            println!("\n== {}", title);

            for (biome, count) in &order {
                let Some(row) = table.get(biome) else { continue };
                let sum: f64 = row.values().sum();
                let mut row: Vec<_> = row.iter().collect();
                row.sort_by(|a, b| b.1.total_cmp(a.1));
                let items: Vec<String> = row
                    .iter()
                    .take(top)
                    .map(|(block, n)| {
                        if per_column {
                            format!("{} {:.3}", block, **n / count)
                        } else {
                            format!("{} {:.1}%", block, 100.0 * **n / sum)
                        }
                    })
                    .collect();
                println!("  {:<18} {}", biome, items.join(", "));
            }
        };

        show("верх пола (над ним пусто, y ≥ 35)", &floor, false, 12);
        show("верх пола у моря лавы (y 31..34)", &floor_low, false, 12);
        show("низ потолка (под ним пусто)", &ceiling, false, 12);
        show("мелочь на столбец", &loose, true, 20);
        show("объём на столбец (y 0..127)", &volume, true, 24);
    }

    /// Скорость: сколько в среднем складывается чанк Незера и из чего это
    /// время. `cargo test --release nether_chunk_phases -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn nether_chunk_phases() {
        use std::time::{Duration, Instant};

        let nether = Nether::new(100_554_032_945_340);
        let count = 64;
        let mut phases = [Duration::ZERO; 6];
        let mut total = Duration::ZERO;

        // Пороги и подгонка климата считаются один раз на мир — не в счёт.
        nether.thresholds();
        nether.fit();

        for i in 0..count {
            let (chunk_x, chunk_z) = (i % 8 - 20, i / 8 + 13);
            let started = Instant::now();
            let _ = nether.chunk(chunk_x, chunk_z);
            total += started.elapsed();

            let mut piece = Piece { x0: chunk_x * CHUNK_SIZE, z0: chunk_z * CHUNK_SIZE, blocks: vec![AIR; PIECE] };
            let mut time = |at: usize, work: &mut dyn FnMut(&mut Piece)| {
                let started = Instant::now();
                work(&mut piece);
                phases[at] += started.elapsed();
            };
            let mut biomes = Vec::new();

            time(0, &mut |piece| {
                biomes = (0..256).map(|at| nether.biome_at(piece.x0 + at % 16, piece.z0 + at / 16)).collect();
            });
            time(1, &mut |piece| nether.relief(piece, &biomes));
            time(2, &mut |piece| nether.surface(piece, &biomes));
            time(3, &mut |piece| nether.blobs(piece, chunk_x, chunk_z));
            time(4, &mut |piece| nether.features(piece, chunk_x, chunk_z));
            time(5, &mut |piece| nether.decorate(piece, &biomes));
        }

        let each = |time: Duration| time.as_secs_f64() * 1000.0 / count as f64;

        println!("чанк Незера целиком: {:.2} мс", each(total));
        for (name, time) in ["биомы", "рельеф", "поверхность", "пятна", "объекты", "мелочь"].iter().zip(phases) {
            println!("  {:<12} {:.2} мс", name, each(time));
        }
    }
}
