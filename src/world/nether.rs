// Незер: из чего он сложен.
//
// Рельеф — диапазонный шум с верхним и нижним сплошными слоями,
// интерполированный в сетке 4×8×4. Фаза климата и рельефа проверяется по
// сохранённым region-файлам двух семян 26.3, без чтения кода игры.
// Узкие ходы пока используют собственную пару шумов [проверить]. Ниже
// моря лавы (y 31) все пустоты залиты лавой.
//
// Биомы — ближайшие точки температуры, влажности и смещения. Климат
// считается в клетках 4×4 и не меняется по высоте. От биома зависят
// поверхность и наполнение: нилий в лесах, песок и почва душ в долине,
// базальт и чернокамень в дельтах.
//
// Крепости — src/world/nether_fortress.rs: кладутся до объектов (опоры
// доходят до породы) и ещё раз после них (куски чисты от грибов и
// светокамня). Бастионы — src/world/nether_bastion.rs, в регионах, где не
// выпала крепость; кладутся теми же двумя проходами.
//
// Всё решается только по координатам и семени: объект, выросший у края
// соседнего чанка (гриб, светокамень, столб), дорисовывается в этом чанке
// по тем же числам, и на стыке ничего не рвётся.
//
// Незер живёт в той же сетке высот, что обычный мир (−64..320): клиенту он
// объявлен с этими границами, а заполнен только от 0 до 127 — так формат
// чанков и все расчёты высоты остаются общими.

use std::sync::{Mutex, OnceLock};
use std::collections::HashMap;

#[cfg(not(seedmap_core))]
use super::nether_bastion as bastion;
use super::nether_fortress as fortress;
use super::noise::Noise;
use super::terrain::Biome;
#[cfg(not(seedmap_core))]
use super::{biome_cell, pack, split, BiomeId, Chunk, AIR, BIOME_CELLS, BIOME_LAYERS, CHUNK_SIZE};
#[cfg(seedmap_core)]
use super::{AIR, CHUNK_SIZE};
use crate::blocks;

/// Верх лавового моря: ниже и на этой высоте пустоты залиты лавой.
pub const LAVA_SEA: i32 = 31;

pub const STRUCTURE_SPACING: i32 = super::nether_fortress::REGION;

/// Самый верх Незера: выше потолка из бедрока ничего нет.
pub const CEILING: i32 = 127;

/// Слоёв бедрока у пола и у потолка: крайний сплошной, дальше реже на
/// пятую долю с каждым слоем (замер: 100, 80, 60, 40, 20 %).
const BEDROCK_LAYERS: i32 = 5;

/// Блоков в местном куске чанка: 16 × 16 × 128.
const PIECE: usize = (CHUNK_SIZE * CHUNK_SIZE * (CEILING + 1)) as usize;

/// minecraft.wiki/w/World_generation: «specifies biomes with a single point».
/// Третья координата — смещение от климатической плоскости, а не вес площади.
/// Порядок совпадает с `Biome::NETHER`. Проверено по сохранённым биомам 26.3.
const CLIMATE_POINTS: [(f64, f64, f64); 5] = [
    (0.0, 0.0, 0.0), (0.4, 0.0, 0.0), (0.0, 0.5, 0.375),
    (0.0, -0.5, 0.0), (-0.5, 0.0, 0.175),
];

#[cfg(all(test, not(seedmap_core)))]
const SHARES: [f64; 5] = [0.363, 0.222, 0.085, 0.171, 0.159];

/// Замер оригинала по высотам (tools/measure/nether_layers.py, 19 тысяч
/// чанков мира vanilla-big): доля воздуха больших пещер над морем лавы.
/// Сверху пещеры кончаются к y 114 — там начинается сплошной потолок.
const MEASURED_AIR: [(i32, f64); 22] = [
    (32, 0.492),
    (40, 0.473),
    (48, 0.473),
    (56, 0.464),
    (64, 0.458),
    (72, 0.459),
    (76, 0.455),
    (80, 0.461),
    (88, 0.474),
    (96, 0.478),
    (104, 0.478),
    (105, 0.435),
    (106, 0.383),
    (107, 0.320),
    (108, 0.245),
    (109, 0.177),
    (110, 0.127),
    (111, 0.090),
    (112, 0.064),
    (113, 0.019),
    (114, 0.002),
    (115, 0.0),
];

/// То же — доля узких ходов (у оригинала это cave_air): их прорезают
/// после поверхности, поэтому они есть и в толще потолка, до y 120.
const MEASURED_CAVE_AIR: [(i32, f64); 17] = [
    (32, 0.041),
    (40, 0.045),
    (52, 0.047),
    (60, 0.047),
    (64, 0.044),
    (72, 0.043),
    (80, 0.042),
    (88, 0.040),
    (96, 0.040),
    (104, 0.038),
    (106, 0.045),
    (108, 0.056),
    (110, 0.064),
    (112, 0.068),
    (114, 0.072),
    (120, 0.063),
    (121, 0.0),
];

/// Узкие ходы выбираются независимо от основного диапазонного поля.
/// Внизу доля относится к оставшейся породе: вычитание старой оценки
/// лавового моря из нового поля давало лишние лавовые карманы.
/// Геометрия ходов пока статистическая, фаза оригинала [проверить].
fn tunnel_share(y: i32) -> f64 {
    if !(2..=120).contains(&y) {
        return 0.0;
    }
    if y <= LAVA_SEA {
        return 0.05;
    }
    (interpolate(&MEASURED_CAVE_AIR, y) / (1.0 - interpolate(&MEASURED_AIR, y))).min(1.0)
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
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Void {
    Rock,
    /// Большая пещера: её пол и потолок покрывает поверхность биома.
    Cavern,
    /// Узкий ход. У оригинала его прорезают уже после поверхности (там он
    /// cave_air), поэтому стенки хода — голая порода.
    Tunnel,
}

/// Где перемычки в блок толщиной убираются: над морем лавы, ниже бедрока
/// потолка.
const MEMBRANES: std::ops::RangeInclusive<i32> = LAVA_SEA + 2..=CEILING - BEDROCK_LAYERS - 1;

/// Перемычка между пещерами снизу и сверху становится пещерой; тонкие
/// стенки узких ходов остаются (у оригинала ходы режут отдельно, после
/// поверхности, и перемычки у них есть). Соседи берутся исходные, так что
/// решение не зависит от порядка обхода.
fn membrane(below: Void, above: Void) -> Option<Void> {
    (below == Void::Cavern && above == Void::Cavern).then_some(Void::Cavern)
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

    /// Мелочь: огонь, грибы, корни, ростки, лозы — не держит опору.
    fn is_loose(&self, state: i32) -> bool {
        [
            self.fire,
            self.soul_fire,
            self.brown_mushroom,
            self.red_mushroom,
            self.crimson_roots,
            self.warped_roots,
            self.sprouts,
            self.crimson_fungus,
            self.warped_fungus,
            self.weeping_vines,
            self.weeping_vines_plant,
            self.twisting_vines,
            self.twisting_vines_plant,
        ]
        .contains(&state)
    }

    /// Порода, по которой идут поверхность и руда: её можно заменять.
    fn is_rock(&self, state: i32) -> bool {
        state == self.netherrack || state == self.basalt || state == self.blackstone
    }
}

/// Пороги шумов по высотам 0..=127.
struct Thresholds {
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
    caverns: TerrainNoise,
    density_columns: Mutex<HashMap<(i32, i32), [f64; 17]>>,
    along: Noise,
    across: Noise,
    heat: ClimateNoise,
    damp: ClimateNoise,
    /// Породы дельт: базальт, чернокамень, незеррак.
    rocks: Noise,
    /// Пятна поверхности: песок и почва душ, гравий у моря, магма.
    patches: Noise,
    /// Второй шум пятен — независимый от первого.
    specks: Noise,
    thresholds: OnceLock<Thresholds>,
    states: States,
}

impl Nether {
    pub fn new(seed: i64) -> Nether {
        Nether {
            seed,
            caverns: TerrainNoise::new(seed),
            density_columns: Mutex::new(HashMap::new()),
            along: Noise::new(seed ^ 0x1B_AD_5E_ED),
            across: Noise::new(seed ^ 0x7A_11_C0_DE),
            heat: ClimateNoise::new(seed),
            damp: ClimateNoise::new(seed.wrapping_add(1)),
            rocks: Noise::new(seed ^ 0x5A_B0_C4_77),
            patches: Noise::new(seed ^ 0x2F_09_E1_5D),
            specks: Noise::new(seed ^ 0x61_3C_AA_09),
            thresholds: OnceLock::new(),
            states: States::new(),
        }
    }

    /// Семя мира.
    #[allow(dead_code)] // для порталов и строений
    pub fn seed(&self) -> i64 {
        self.seed
    }

    /// Узлы рельефа имеют шаг 4×8×4; внутри ячейки плотность линейная.
    /// Кэш ограничен и не влияет на результат или порядок генерации.
    fn density(&self, x: i32, y: i32, z: i32) -> f64 {
        let (cx, cy, cz) = (x.div_euclid(4), y.div_euclid(8), z.div_euclid(4));
        let (tx, ty, tz) = (f64::from(x.rem_euclid(4)) / 4.0,
            f64::from(y.rem_euclid(8)) / 8.0, f64::from(z.rem_euclid(4)) / 4.0);
        let mut cache = self.density_columns.lock().unwrap();
        if cache.len() > 16_384 { cache.clear(); }
        let mut sides = [0.0; 4];
        for dz in 0..2 {
            for dx in 0..2 {
                let row = cache.entry((cx + dx, cz + dz)).or_insert_with(|| {
                    std::array::from_fn(|i| self.caverns.at(f64::from((cx + dx) * 4), (i * 8) as f64, f64::from((cz + dz) * 4)))
                });
                sides[(dz * 2 + dx) as usize] = row[cy as usize] * (1.0 - ty) + row[cy as usize + 1] * ty;
            }
        }
        (sides[0] * (1.0 - tx) + sides[1] * tx) * (1.0 - tz)
            + (sides[2] * (1.0 - tx) + sides[3] * tx) * tz
    }

    /// Пересечение двух полей задаёт тонкие связанные ходы. Прежний шаг
    /// 300 блоков сливал их в большие полости и сглаживал перепады полов.
    /// Ширина и заполнение сверяются с region-файлами, фаза [проверить].
    fn tunnel(&self, x: i32, y: i32, z: i32) -> f64 {
        let (fx, fy, fz) = (f64::from(x), f64::from(y), f64::from(z));
        self.along.at3(fx / 48.0 + 0.71, fy / 48.0 + 0.29, fz / 48.0 + 0.53)
            .abs().max(self.across.at3(fx / 48.0 + 0.19, fy / 48.0 + 0.83, fz / 48.0 + 0.47).abs())
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

            let mut tunnels = Vec::with_capacity(SAMPLES);
            let mut rocks = Vec::with_capacity(SAMPLES);
            let mut state = (self.seed as u64) ^ 0x5DEE_CE66;

            for _ in 0..SAMPLES {
                let mut next = || {
                    state = scramble(state.wrapping_add(0x9E37_79B9_7F4A_7C15));
                    (state % 4_000_000) as i32 - 2_000_000
                };
                let (x, y, z) = (next(), next().rem_euclid(128), next());

                tunnels.push(self.tunnel(x, y, z));
                rocks.push(self.rock(x, y, z));
            }

            tunnels.sort_by(f64::total_cmp);
            rocks.sort_by(f64::total_cmp);

            let heights = 0..=CEILING;

            Thresholds {
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

    /// Что в этом месте рельефа, с перемычками: порода в один блок между
    /// пещерами сверху и снизу выше моря лавы тоже пуста. Замер оригинала
    /// (nether_layers): перемычек толщиной в блок 0,10 на столбец, у нас
    /// без этого было 0,16–0,18 — лишние полы и потолки, и опоры крепостей
    /// чаще упирались в них высоко под мостом.
    fn void(&self, x: i32, y: i32, z: i32) -> Void {
        let here = self.raw_void(x, y, z);

        if here != Void::Rock || !(MEMBRANES.contains(&y)) {
            return here;
        }

        membrane(self.raw_void(x, y - 1, z), self.raw_void(x, y + 1, z)).unwrap_or(Void::Rock)
    }

    /// Что в этом месте рельефа по шумам: порода, большая пещера или узкий
    /// ход.
    fn raw_void(&self, x: i32, y: i32, z: i32) -> Void {
        if !(0..=CEILING).contains(&y) {
            return Void::Rock;
        }

        let thresholds = self.thresholds();
        let tunnel = thresholds.tunnels[y as usize];

        // Где пещер или ходов на этой высоте нет вовсе, их шум не считается.
        if self.density(x, y, z) <= 0.0 {
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
    pub(super) fn open(&self, x: i32, y: i32, z: i32) -> bool {
        (1..CEILING).contains(&y) && !self.bedrock(x, y, z) && self.hollow(x, y, z)
    }

    /// Сплошная ли порода в этом месте рельефа (без бедрока).
    pub(super) fn rock_at(&self, x: i32, y: i32, z: i32) -> bool {
        (1..CEILING).contains(&y) && !self.bedrock(x, y, z) && !self.hollow(x, y, z)
    }

    /// Климат хранится по клеткам 4×4. Две пары октав с периодами 128/64
    /// клеток сохраняют фазу от семени; нормировка по выборке её разрушала.
    fn climate(&self, x: i32, z: i32) -> (f64, f64) {
        let (x, z) = (f64::from(x.div_euclid(4)), f64::from(z.div_euclid(4)));
        (self.heat.at(x, z), self.damp.at(x, z))
    }

    fn nearest((heat, damp): (f64, f64)) -> usize {
        // Климат сравнивается с точностью 1/10000, в том числе у границ.
        let quantize = |v: f64| (v * 10_000.0) as i64;
        let (heat, damp) = (quantize(heat), quantize(damp));
        CLIMATE_POINTS.iter().enumerate().min_by_key(|(_, (t, h, offset))| {
            (heat - quantize(*t)).pow(2) + (damp - quantize(*h)).pow(2) + quantize(*offset).pow(2)
        }).unwrap().0
    }

    #[cfg(not(seedmap_core))]
    pub fn progress_structure(&self, x: i32, y: i32, z: i32) -> Option<&'static str> {
        if self.fortresses_near(x.div_euclid(16), z.div_euclid(16)).iter().any(|parts| super::in_structure_bounds(parts.iter().map(|p| p.bounds), (x, y, z))) {
            return Some("nether/find_fortress");
        }
        self.bastions_near(x.div_euclid(16), z.div_euclid(16)).iter()
            .any(|parts| super::in_structure_bounds(parts.iter().map(|p| p.bounds), (x, y, z)))
            .then_some("nether/find_bastion")
    }

    /// Лежит ли клетка в ящике какого-либо куска крепости.
    #[cfg(not(seedmap_core))]
    pub fn in_fortress(&self, x: i32, y: i32, z: i32) -> bool {
        self.fortresses_near(x.div_euclid(16), z.div_euclid(16))
            .iter()
            .flatten()
            .any(|piece| piece.bounds[0] <= x && x <= piece.bounds[3] && piece.bounds[1] <= y && y <= piece.bounds[4] && piece.bounds[2] <= z && z <= piece.bounds[5])
    }

    /// Биом столбца.
    pub fn biome_at(&self, x: i32, z: i32) -> Biome {
        Biome::NETHER[Nether::nearest(self.climate(x, z))]
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
    #[cfg(not(seedmap_core))]
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
    #[cfg(not(seedmap_core))]
    fn piece(&self, chunk_x: i32, chunk_z: i32) -> Piece {
        self.piece_with(chunk_x, chunk_z, true)
    }

    /// То же, с постройками или без: мир оригинала для замеров рельефа
    /// (vanilla-big) снят без построек, и сверка идёт с таким же нашим.
    #[cfg(not(seedmap_core))]
    fn piece_with(&self, chunk_x: i32, chunk_z: i32, structures: bool) -> Piece {
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

        let (fortresses, bastions) = if structures {
            (self.fortresses_near(chunk_x, chunk_z), self.bastions_near(chunk_x, chunk_z))
        } else {
            (Vec::new(), Vec::new())
        };

        self.relief(&mut piece, &biomes);
        self.surface(&mut piece, &biomes);
        self.blobs(&mut piece, chunk_x, chunk_z);
        // Крепость — до объектов и растений, как у оригинала: опоры доходят
        // до породы, а не до шляпы гриба или светокамня.
        self.fortresses(&mut piece, &fortresses, true);
        self.bastions(&mut piece, &bastions);
        self.features(&mut piece, chunk_x, chunk_z);
        self.decorate(&mut piece, &biomes);
        self.springs(&mut piece, chunk_x, chunk_z);
        // Объекты решаются по рельефу и могли зайти в пустоты крепости —
        // куски кладутся ещё раз, уже без опор.
        self.fortresses(&mut piece, &fortresses, false);
        self.bastions(&mut piece, &bastions);

        piece
    }

    /// Бастион региона — там, где не выпала крепость (в дельтах их нет).
    pub fn bastion_start(&self, region_x: i32, region_z: i32) -> Option<(i32, i32)> {
        let start = fortress::region_start(self.seed, region_x, region_z);

        (start.2 >= fortress::FORTRESS_SHARE
            && self.biome_at(start.0 * CHUNK_SIZE + 8, start.1 * CHUNK_SIZE + 8) != Biome::BasaltDeltas)
            .then_some((start.0, start.1))
    }

    /// Бастионы, задевающие этот чанк.
    #[cfg(not(seedmap_core))]
    fn bastions_near(&self, chunk_x: i32, chunk_z: i32) -> Vec<Vec<bastion::Placed>> {
        let span = |at: i32| (at - bastion::RADIUS_CHUNKS).div_euclid(STRUCTURE_SPACING)..=(at + bastion::RADIUS_CHUNKS).div_euclid(STRUCTURE_SPACING);

        span(chunk_x)
            .flat_map(|region_x| span(chunk_z).map(move |region_z| (region_x, region_z)))
            .filter_map(|(region_x, region_z)| self.bastion_start(region_x, region_z))
            .filter(|(start_x, start_z)| {
                (start_x - chunk_x).abs() <= bastion::RADIUS_CHUNKS && (start_z - chunk_z).abs() <= bastion::RADIUS_CHUNKS
            })
            .map(|(start_x, start_z)| bastion::plan(self.seed, start_x, start_z))
            .collect()
    }

    /// Кладёт куски бастионов (опор у них нет) и убирает растения, у
    /// которых бастион отнял опору.
    #[cfg(not(seedmap_core))]
    fn bastions(&self, piece: &mut Piece, bastions: &[Vec<bastion::Placed>]) {
        if bastions.is_empty() {
            return;
        }

        let passable = |state: i32| is_air(state);
        let mut canvas = fortress::Canvas {
            blocks: &mut piece.blocks,
            x0: piece.x0,
            z0: piece.z0,
            passable: &passable,
            air: AIR,
            supports: false,
        };

        for plan in bastions {
            bastion::paint(self.seed, plan, &mut canvas);
        }

        self.drop_unsupported(piece);
    }

    /// Постройка региона: крепость или бастион (замер оригинала: в каждом
    /// регионе 27×27 чанков одна попытка; крепость в 40 %).
    /// minecraft.wiki/w/Bastion_Remnant: «except basalt deltas» — неудачная
    /// попытка бастиона не превращается в дополнительную крепость.
    pub fn fortress_start(&self, region_x: i32, region_z: i32) -> Option<(i32, i32)> {
        let (chunk_x, chunk_z, roll) = fortress::region_start(self.seed, region_x, region_z);
        (roll < fortress::FORTRESS_SHARE).then_some((chunk_x, chunk_z))
    }

    /// Крепости, задевающие этот чанк: каждая раскладывается целиком по
    /// семени, а кладётся только своей частью.
    #[cfg(not(seedmap_core))]
    fn fortresses_near(&self, chunk_x: i32, chunk_z: i32) -> Vec<Vec<fortress::Placed>> {
        fortress::regions_near(chunk_x, chunk_z)
            .filter_map(|(region_x, region_z)| self.fortress_start(region_x, region_z))
            .filter(|(start_x, start_z)| {
                (start_x - chunk_x).abs() <= fortress::RADIUS_CHUNKS && (start_z - chunk_z).abs() <= fortress::RADIUS_CHUNKS
            })
            .map(|(start_x, start_z)| fortress::plan(self.seed, start_x, start_z))
            .collect()
    }

    /// Кладёт куски крепостей (с опорами или без) и убирает растения,
    /// у которых крепость отняла опору.
    #[cfg(not(seedmap_core))]
    fn fortresses(&self, piece: &mut Piece, fortresses: &[Vec<fortress::Placed>], supports: bool) {
        let states = &self.states;

        if fortresses.is_empty() {
            return;
        }

        let passable = |state: i32| is_air(state) || state == states.lava || states.is_loose(state);
        let mut canvas = fortress::Canvas {
            blocks: &mut piece.blocks,
            x0: piece.x0,
            z0: piece.z0,
            passable: &passable,
            air: AIR,
            supports,
        };

        for plan in fortresses {
            fortress::paint(plan, &mut canvas);
        }

        self.drop_unsupported(piece);
    }

    /// Растения без опоры: на полу — над пустотой (снизу вверх), лозы
    /// с потолка — под пустотой (сверху вниз).
    #[cfg(not(seedmap_core))]
    fn drop_unsupported(&self, piece: &mut Piece) {
        let states = &self.states;

        for local_z in 0..CHUNK_SIZE {
            for local_x in 0..CHUNK_SIZE {
                for hanging in [false, true] {
                    for step in 1..CEILING {
                        let y = if hanging { CEILING - step } else { step };
                        let at = Piece::index(local_x, y, local_z);
                        let here = piece.blocks[at];
                        let vine = here == states.weeping_vines || here == states.weeping_vines_plant;

                        if !states.is_loose(here) || vine != hanging {
                            continue;
                        }

                        let holder = Piece::index(local_x, if hanging { y + 1 } else { y - 1 }, local_z);

                        if is_air(piece.blocks[holder]) {
                            piece.blocks[at] = AIR;
                        }
                    }
                }
            }
        }
    }

    /// Рельеф: бедрок, порода, лава моря и карманов, пустоты.
    #[cfg(not(seedmap_core))]
    fn relief(&self, piece: &mut Piece, biomes: &[Biome]) {
        let states = &self.states;
        let thresholds = self.thresholds();

        for local_z in 0..CHUNK_SIZE {
            for local_x in 0..CHUNK_SIZE {
                let (x, z) = (piece.x0 + local_x, piece.z0 + local_z);
                let deltas = biomes[(local_z * CHUNK_SIZE + local_x) as usize] == Biome::BasaltDeltas;

                let mut voids = [Void::Rock; (CEILING + 1) as usize];

                for (y, void) in voids.iter_mut().enumerate() {
                    *void = self.raw_void(x, y as i32, z);
                }
                for y in MEMBRANES {
                    let at = y as usize;

                    if voids[at] == Void::Rock
                        && let Some(open) = membrane(voids[at - 1], voids[at + 1])
                    {
                        voids[at] = open;
                    }
                }

                for y in 0..=CEILING {
                    let void = if self.bedrock(x, y, z) { Void::Rock } else { voids[y as usize] };
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
    #[cfg(not(seedmap_core))]
    fn surface(&self, piece: &mut Piece, biomes: &[Biome]) {
        let states = &self.states;

        for local_z in 0..CHUNK_SIZE {
            for local_x in 0..CHUNK_SIZE {
                let (x, z) = (piece.x0 + local_x, piece.z0 + local_z);
                let biome = biomes[(local_z * CHUNK_SIZE + local_x) as usize];
                let (fx, fz) = (f64::from(x), f64::from(z));

                // Дно долины: над бедроком — слой песка и почвы душ.
                if biome == Biome::SoulSandValley {
                    for y in 1..=soul_bottom(self.hash(0xB077, x, 0, z)) {
                        let at = Piece::index(local_x, y, local_z);

                        if states.is_rock(piece.blocks[at]) {
                            let patch = self.patches.octaves3(fx / 9.0 + 0.5, f64::from(y) / 5.0 + 0.1, fz / 9.0 + 0.3, 2);
                            piece.blocks[at] = if patch > 0.0 { states.soul_sand } else { states.soul_soil };
                        }
                    }
                }

                for y in 1..CEILING {
                    let here = piece.blocks[Piece::index(local_x, y, local_z)];

                    if !states.is_rock(here) {
                        continue;
                    }

                    let above = piece.blocks[Piece::index(local_x, y + 1, local_z)];
                    let below = piece.blocks[Piece::index(local_x, y - 1, local_z)];
                    let forest = matches!(biome, Biome::CrimsonForest | Biome::WarpedForest);
                    // Поверхность предшествует узким ходам: нилий не
                    // восстанавливается на обнажённой ими породе.
                    let floor = above == AIR || (above == states.lava && !forest);
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
                                p if p < -0.35 => Some((states.gravel, depth)),
                                _ => None,
                            },
                            Biome::SoulSandValley => {
                                // Гравий у моря — своими пятнами, не
                                // в ущерб одному из двух песков.
                                let gravel = self.patches.octaves3(fx / 9.0 + 7.3, fy / 5.0 + 2.9, fz / 9.0 + 4.1, 2);

                                if shore && gravel < -0.12 {
                                    Some((states.gravel, gravel_depth(self.hash(0x6EA7, x, y, z))))
                                } else if self.specks.octaves3(fx / 14.0, fy / 9.0, fz / 14.0, 2) < SOUL_GAPS {
                                    None
                                } else {
                                    // Песок или почва — пятнами; слой толще,
                                    // чем у прочих: чаще 3–4 блока.
                                    let soul = if patch > 0.0 { states.soul_sand } else { states.soul_soil };
                                    Some((soul, soul_depth(self.hash(0xDE98, x, y, z))))
                                }
                            }
                            Biome::CrimsonForest | Biome::WarpedForest if is_air(above) => {
                                if self.specks.octaves3(fx / 6.0, fy / 6.0, fz / 6.0, 2) < -0.6 {
                                    None
                                } else if biome == Biome::CrimsonForest {
                                    Some((states.crimson_nylium, 1))
                                } else {
                                    Some((states.warped_nylium, 1))
                                }
                            }
                            Biome::BasaltDeltas => {
                                if shore && patch < -0.2 {
                                    Some((states.gravel, gravel_depth(self.hash(0x6EA7, x, y, z))))
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

                                if self.specks.octaves3(fx / 14.0, fy / 9.0, fz / 14.0, 2) >= SOUL_GAPS {
                                    let patch = self.patches.octaves3(fx / 9.0 + 0.5, fy / 5.0 + 0.1, fz / 9.0 + 0.3, 2);
                                    let soul = if patch > 0.0 { states.soul_sand } else { states.soul_soil };
                                    let depth = soul_depth(self.hash(0xCE11, x, y, z));

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
    #[cfg(not(seedmap_core))]
    fn blobs(&self, piece: &mut Piece, chunk_x: i32, chunk_z: i32) {
        self.blobs_with_culling(piece,chunk_x,chunk_z,true);
    }

    #[cfg(not(seedmap_core))]
    fn blobs_with_culling(&self, piece: &mut Piece, chunk_x: i32, chunk_z: i32, cull: bool) {
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
                        let y = if kind.state == Blob::DEBRIS && kind.high == 24 {
                            // minecraft.wiki/w/Ancient_Debris: «triangle distribution».
                            kind.low + dice.below(9) + dice.below(9)
                        } else {
                            kind.low + dice.below((kind.high - kind.low + 1) as u32)
                        };
                        // В замере рудная масса смещена ниже выбранной высоты;
                        // симметричный центр давал лишнюю руду в слое 112..127.
                        let y = y - i32::from(matches!(kind.state, Blob::QUARTZ | Blob::GOLD));
                        let size = kind.size.0 + dice.unit() * (kind.size.1 - kind.size.0);
                        let radius = (size * 0.75 / std::f64::consts::PI).cbrt();
                        let reach = radius.ceil() as i32 + 1;

                        // Все броски делаются до отсечения: иначе соседний
                        // чанк пропускал три числа и следующие пятна расходились.
                        let (sx, sy, sz) = (0.7 + dice.unit() * 0.6, 0.6 + dice.unit() * 0.5, 0.7 + dice.unit() * 0.6);
                        if cull && !piece.near(x, z, reach) {
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
                        for dx in -reach..=reach {
                            for dy in -reach..=reach {
                                for dz in -reach..=reach {
                                    let (bx, by, bz) = (x + dx, y + dy, z + dz);
                                    let (fx, fy, fz) = (f64::from(dx) / sx, f64::from(dy) / sy, f64::from(dz) / sz);
                                    let distance = (fx * fx + fy * fy + fz * fz).sqrt();
                                    let edge = radius * (0.8 + 0.4 * unit(self.hash(0xED6E, bx, by, bz)));

                                    // minecraft.wiki/w/Ore_(feature): «Maximum height» 117.
                                    // В обоих сохранённых мирах крайний рудный слой — 117.
                                    if distance > edge || (matches!(kind.state, Blob::QUARTZ | Blob::GOLD) && by > kind.high) {
                                        continue;
                                    }

                                    if let Some(at) = piece.at(bx, by, bz)
                                        && (piece.blocks[at] == states.netherrack
                                            || (kind.state == Blob::DEBRIS && states.is_rock(piece.blocks[at])))
                                    {
                                        if kind.state == Blob::DEBRIS && self.exposed_to_air(bx, by, bz) {
                                            continue;
                                        }
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

    /// Проверка соседей по исходному рельефу, в том числе за границей чанка.
    /// Лава не считается воздухом: обломки могут касаться лавовых карманов.
    #[cfg(not(seedmap_core))]
    fn exposed_to_air(&self, x: i32, y: i32, z: i32) -> bool {
        [(1, 0, 0), (-1, 0, 0), (0, 1, 0), (0, -1, 0), (0, 0, 1), (0, 0, -1)]
            .iter().any(|&(dx, dy, dz)| y + dy > LAVA_SEA && self.open(x + dx, y + dy, z + dz))
    }

    /// Крупные объекты: столбы базальта, огромные грибы, светокамень,
    /// лавовые лужи и столбики дельт. Порядок обхода — по чанкам-источникам
    /// и видам, одинаковый для всех чанков: объекты ставятся только в
    /// пустоту, и кто раньше встал, того и место — на обе стороны стыка.
    #[cfg(not(seedmap_core))]
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
    #[cfg(not(seedmap_core))]
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
    #[cfg(not(seedmap_core))]
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

            // Огромный гриб требует нилия; голый пол узкого хода не
            // подходит. Условие аналитическое и одинаково за краем чанка.
            if self.void(x, floor + 1, z) != Void::Cavern
                || self.specks.octaves3(f64::from(x) / 6.0, f64::from(floor) / 6.0, f64::from(z) / 6.0, 2) < -0.6
            {
                continue;
            }

            // Ствол не упирается в потолок: не хватает места — гриб ниже,
            // а шляпу срезает потолок. У оригинала так же: пятая часть
            // одиночных грибов со стволом в 1–3 блока под самой породой.
            let room = (floor + 1..floor + 1 + height + 2).take_while(|&y| self.open(x, y, z)).count() as i32;

            if room < 3 {
                continue;
            }

            let height = height.min(room - 1);
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

            // Шляпа — по замеру оригинала (tools/measure/nether_fungus.py,
            // 475 одиночных багровых грибов vanilla-big): крышка над стволом
            // в одно кольцо, под ней стенки на расстоянии 2 от ствола —
            // столбиками по нескольку блоков, внутри у ствола редкие наросты,
            // изредка юбка на расстоянии 3 ниже. Столбики стоят друг на друге,
            // поэтому сверху открыто только ~40 % наростов (у прежних колец,
            // шагавших вширь, — почти 60 %, и шляпы занимали 41 % полов леса
            // вместо 31 %).
            let wall_run = 4 + height.min(13) / 4;
            let cap = |piece: &mut Piece, dx: i32, y: i32, dz: i32, roll: &mut Dice| {
                let block = if roll.unit() < 0.03 { states.shroomlight } else { wart };

                piece.put_into_air(x + dx, y, z + dz, block)
            };

            for dx in -3i32..=3 {
                for dz in -3i32..=3 {
                    let ring = dx.abs().max(dz.abs());
                    let corner = dx.abs() == ring && dz.abs() == ring;

                    match ring {
                        0 => {
                            if roll.unit() < 0.5 {
                                cap(piece, 0, top + 1, 0, &mut roll);
                            }
                        }
                        1 => {
                            // Крышка и под ней — столбик от верха вниз.
                            if roll.unit() < 0.55 {
                                cap(piece, dx, top + 1, dz, &mut roll);
                            }
                            if roll.unit() < 0.45 {
                                let mut y = top;

                                loop {
                                    cap(piece, dx, y, dz, &mut roll);
                                    y -= 1;
                                    if y <= top - 5 || roll.unit() >= 0.62 {
                                        break;
                                    }
                                }
                            }
                        }
                        2 => {
                            if roll.unit() < 0.06 {
                                cap(piece, dx, top + 1, dz, &mut roll);
                            }
                            if roll.unit() < if corner { 0.35 } else { 0.65 } {
                                // Стенка: столбик с верха 0, −1 или −2 вниз.
                                let start = top - roll.below(3);
                                let length = 1 + roll.below(wall_run as u32);

                                for down in 0..length {
                                    cap(piece, dx, start - down, dz, &mut roll);
                                }
                                if biome == Biome::CrimsonForest && roll.unit() < 0.12 {
                                    let bottom = start - length + 1;
                                    let length = 1 + roll.below(4);

                                    for down in 1..=length {
                                        let tip = down == length;
                                        let vine = if tip { states.weeping_vines } else { states.weeping_vines_plant };

                                        if !piece.put_into_air(x + dx, bottom - down, z + dz, vine) {
                                            break;
                                        }
                                    }
                                }
                            }
                        }
                        _ => {
                            // Юбка: редкие столбики ниже стенок.
                            if !corner && roll.unit() < 0.1 {
                                let start = top - 1 - roll.below(5);

                                for down in 0..1 + roll.below(3) {
                                    cap(piece, dx, start - down, dz, &mut roll);
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
    #[cfg(not(seedmap_core))]
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
    #[cfg(not(seedmap_core))]
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
    #[cfg(not(seedmap_core))]
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
    #[cfg(not(seedmap_core))]
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
    #[cfg(not(seedmap_core))]
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
    #[cfg(not(seedmap_core))]
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
                (0.146, states.crimson_roots),
                (0.018, states.crimson_fungus),
                (0.0018, states.warped_fungus),
            ]),
            Biome::WarpedForest if floor == states.warped_nylium => pick(&[
                (0.114, states.warped_roots),
                (0.096, states.sprouts),
                (0.019, states.warped_fungus),
                (0.016, states.twisting_vines),
                (0.0026, states.crimson_roots),
            ]),
            Biome::NetherWastes if floor == states.netherrack => pick(&[
                (0.0074, states.fire),
                (0.0018, states.brown_mushroom),
                (0.0012, states.red_mushroom),
            ]),
            Biome::SoulSandValley if floor == states.soul_soil || floor == states.soul_sand => {
                pick(&[(0.0046, states.soul_fire), (0.0026, states.crimson_roots)])
            }
            Biome::SoulSandValley if floor == states.netherrack => pick(&[(0.006, states.fire)]),
            Biome::BasaltDeltas if floor == states.basalt || floor == states.blackstone => {
                pick(&[(0.0008, states.brown_mushroom), (0.0008, states.red_mushroom)])
            }
            _ => None,
        }
    }

    /// Старт окаменелости ищется в базовой колонке до узких ходов и объектов.
    pub(super) fn fossil_floor_below(&self, x: i32, from: i32, z: i32) -> Option<i32> {
        (32..from.min(CEILING)).rev().find(|&y| self.density(x,y,z) > 0.0 && self.density(x,y+1,z) <= 0.0)
    }

    /// Сплошная ли порода в этом месте (для высоты разрушенных порталов).
    pub(super) fn solid_rock(&self, x: i32, y: i32, z: i32) -> bool {
        self.rock_at(x, y, z)
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

/// Порог шума, ниже которого пол и потолок долины душ остаются голым
/// незерраком (замер: ~17 % полов и потолков долины).
const SOUL_GAPS: f64 = -0.45;

/// Толщина слоя песка или почвы душ на полу и потолке долины — замер
/// оригинала (где слой есть): 1 — 4 %, 2 — 10 %, 3 — 36 %, 4 — 35 %,
/// 5 — 11 %, 6 — 4 %.
fn soul_depth(roll: u64) -> i32 {
    const PARTS: [(i32, u64); 6] = [(1, 4), (2, 10), (3, 36), (4, 35), (5, 11), (6, 4)];
    let mut left = roll % 100;

    for (depth, part) in PARTS {
        if left < part {
            return depth;
        }
        left -= part;
    }

    4
}

/// Толщина душ над бедроком пола долины: у оригинала нижние несколько слоёв
/// между выступами бедрока — песок и почва душ (замер: y 1 — почти всё, что
/// не бедрок, y 2 — 86 %, y 3 — 52 %, y 4 — 14 %).
fn soul_bottom(roll: u64) -> i32 {
    match roll % 100 {
        0..14 => 1,
        14..48 => 2,
        48..86 => 3,
        _ => 4,
    }
}

/// Толщина гравия у моря лавы в долине и дельтах: замер оригинала — там
/// гравий лежит толстым слоем (y 25–31, в столбце гравий на гравии), а не
/// коркой в блок: в дельтах ~0,1 блока на столбец при 7 % полов у моря.
fn gravel_depth(roll: u64) -> i32 {
    2 + (roll % 5) as i32
}

/// Попыток лавы в стене на чанк.
const SPRING_TRIES: usize = 12;

/// Попыток огромного гриба на чанк.
const FUNGUS_TRIES: usize = 32;

/// Попыток грозди светокамня на чанк.
const GLOWSTONE_TRIES: f64 = 0.5;

/// Попыток кучки столбиков на чанк дельт.
const DELTA_COLUMN_TRIES: usize = 4;

/// Доля потолка багрового леса, откуда свисают лозы.
const WEEPING_PART: f64 = 0.014;

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

/// Число попыток руд: minecraft.wiki/w/Ore_(feature), «Spawn tries per chunk».
/// Объём эллипсоида отличается от числа поставленных блоков: граница,
/// растяжение и замещение породы отбрасывают часть. Диапазоны проверяются
/// по блокам на чанк и слоям обоих семян переписи, а не по сумме попыток.
/// Пятна в толще Незера. Высоты и объёмы — по замеру оригинала, блоков на
/// столбец пустошей: кварц 0,5 (y 8–117), золото 0,18, древние обломки
/// 0,006 (y 8–24), гравий 0,6 (пятнами y 4–37: по 8 блоков высоты 0,09 /
/// 0,17 / 0,16 от y 0, выше y 40 почти нет; в дельтах пятен нет), чернокамень 0,74 (y 5–31), магма
/// 0,77 (y 23–37, гуще всего 27–33), базальт 0,4 (кроме багрового леса и дельт).
const BLOBS: [Blob; 8] = [
    Blob { state: Blob::QUARTZ, tries: 16.0, deltas: 32.0, low: 10, high: 117, size: (10.0, 24.0) },
    Blob { state: Blob::GOLD, tries: 10.0, deltas: 20.0, low: 10, high: 117, size: (6.0, 14.0) },
    Blob { state: Blob::DEBRIS, tries: 1.0, deltas: 1.0, low: 8, high: 24, size: (1.0, 2.5) },
    Blob { state: Blob::GRAVEL, tries: 6.6, deltas: 0.0, low: 4, high: 37, size: (20.0, 50.0) },
    Blob { state: Blob::BLACKSTONE, tries: 8.0, deltas: 3.0, low: 5, high: 31, size: (20.0, 50.0) },
    Blob { state: Blob::MAGMA, tries: 14.5, deltas: 4.0, low: 24, high: 34, size: (20.0, 50.0) },
    Blob { state: Blob::BASALT, tries: 9.0, deltas: 0.0, low: 5, high: 120, size: (10.0, 40.0) },
    // minecraft.wiki/w/Ancient_Debris: «evenly from levels 8 to 119».
    Blob { state: Blob::DEBRIS, tries: 1.0, deltas: 1.0, low: 8, high: 119, size: (1.0, 2.0) },
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

/// Поток java.util.Random: 48-битный LCG из документации Java API.
/// Нужен для фазы климатических октав и размещения построек Незера.
pub(super) struct LegacyRandom(u64);

impl LegacyRandom {
    pub(super) fn new(seed: i64) -> Self {
        Self((seed as u64 ^ 0x0005_DEEC_E66D) & ((1u64 << 48) - 1))
    }

    fn bits(&mut self, count: u32) -> u64 {
        self.0 = self.0.wrapping_mul(0x0005_DEEC_E66D).wrapping_add(11) & ((1u64 << 48) - 1);
        self.0 >> (48 - count)
    }

    pub(super) fn below(&mut self, bound: i32) -> i32 {
        assert!(bound > 0);
        if (bound as u32).is_power_of_two() {
            return ((bound as u64 * self.bits(31)) >> 31) as i32;
        }
        loop {
            let bits = self.bits(31) as i32;
            let value = bits % bound;
            if bits.wrapping_sub(value).wrapping_add(bound - 1) >= 0 {
                return value;
            }
        }
    }

    fn long(&mut self) -> i64 {
        ((self.bits(32) as i32 as i64) << 32).wrapping_add(self.bits(32) as i32 as i64)
    }

    pub(super) fn for_chunk(seed: i64, x: i32, z: i32) -> Self {
        let mut random = Self::new(seed);
        let a = random.long();
        let b = random.long();
        Self::new(i64::from(x).wrapping_mul(a) ^ i64::from(z).wrapping_mul(b) ^ seed)
    }

    fn double(&mut self) -> f64 {
        ((self.bits(26) << 27) + self.bits(27)) as f64 / (1u64 << 53) as f64
    }
}

/// Улучшенный шум Перлина: независимая таблица и начало каждой октавы.
/// Начальная таблица тасуется вперёд; три дробных сдвига не дают узла в (0,0).
struct LegacyOctave {
    origin: [f64; 3],
    permutation: [u8; 256],
}

impl LegacyOctave {
    fn new(random: &mut LegacyRandom) -> Self {
        let origin = std::array::from_fn(|_| random.double() * 256.0);
        let mut permutation = std::array::from_fn(|i| i as u8);
        for i in 0..256 {
            let j = i + random.below((256 - i) as i32) as usize;
            permutation.swap(i, j);
        }
        Self { origin, permutation }
    }

    fn at(&self, x: f64, z: f64) -> f64 {
        self.at3(x, 0.0, z, 0.0)
    }

    fn at3(&self, x: f64, y: f64, z: f64, smear: f64) -> f64 {
        let v = [x + self.origin[0], y + self.origin[1], z + self.origin[2]];
        let floor = v.map(f64::floor);
        let mut fraction: [f64; 3] = std::array::from_fn(|i| v[i] - floor[i]);
        let fade = fraction.map(|t| t * t * t * (t * (t * 6.0 - 15.0) + 10.0));
        // Сглаживание использует исходную высоту, градиент — смазанную.
        if smear > 0.0 {
            fraction[1] -= (y.min(fraction[1]) / smear + 1.0e-7).floor() * smear;
        }
        let p = |i: i64| i64::from(self.permutation[(i & 255) as usize]);
        let mut sum = 0.0;
        for dx in 0..2 {
            for dy in 0..2 {
                for dz in 0..2 {
                    let hash = p(p(p(floor[0] as i64 + dx) + floor[1] as i64 + dy) + floor[2] as i64 + dz) & 15;
                    let [x, y, z] = [fraction[0] - dx as f64, fraction[1] - dy as f64, fraction[2] - dz as f64];
                    let u = if hash < 8 { x } else { y };
                    let v = if hash < 4 { y } else if hash == 12 || hash == 14 { x } else { z };
                    let gradient = (if hash & 1 == 0 { u } else { -u }) + (if hash & 2 == 0 { v } else { -v });
                    sum += gradient * (if dx == 0 { 1.0 - fade[0] } else { fade[0] })
                        * (if dy == 0 { 1.0 - fade[1] } else { fade[1] })
                        * (if dz == 0 { 1.0 - fade[2] } else { fade[2] });
                }
            }
        }
        sum
    }
}

/// Климат: сумма двух независимых пар октав. Параметры и порядок потока
/// подтверждены долями и клетками обоих сохранённых миров переписи 26.3.
struct ClimateNoise([[LegacyOctave; 2]; 2]);

impl ClimateNoise {
    fn new(seed: i64) -> Self {
        let mut random = LegacyRandom::new(seed);
        Self(std::array::from_fn(|_| {
            // Нулевая октава и пять пропущенных ступеней до частоты 1/64.
            let _ = LegacyOctave::new(&mut random);
            for _ in 0..5 * 262 { random.bits(32); }
            let high = LegacyOctave::new(&mut random);
            let low = LegacyOctave::new(&mut random);
            [low, high]
        }))
    }

    fn at(&self, x: f64, z: f64) -> f64 {
        let mut sum = 0.0;
        for (i, pair) in self.0.iter().enumerate() {
            let scale = if i == 0 { 1.0 } else { 1.018_126_888_217_522_7 };
            sum += pair[0].at(x * scale / 128.0, z * scale / 128.0) * (2.0 / 3.0)
                + pair[1].at(x * scale / 64.0, z * scale / 64.0) / 3.0;
        }
        sum * (10.0 / 9.0)
    }
}

/// Диапазонный шум: два независимых поля по 16 октав и селектор из восьми.
/// minecraft.wiki/w/World_generation: «two pairs of base heights».
/// Фаза и сетка проверены по пустотам двух миров 26.3, без чтения кода игры.
struct TerrainNoise {
    lower: [LegacyOctave; 16],
    upper: [LegacyOctave; 16],
    selector: [LegacyOctave; 8],
}

impl TerrainNoise {
    fn new(seed: i64) -> Self {
        let mut random = LegacyRandom::new(seed);
        Self {
            lower: std::array::from_fn(|_| LegacyOctave::new(&mut random)),
            upper: std::array::from_fn(|_| LegacyOctave::new(&mut random)),
            selector: std::array::from_fn(|_| LegacyOctave::new(&mut random)),
        }
    }

    fn at(&self, x: f64, y: f64, z: f64) -> f64 {
        let (fx, fy, fz) = (x * 684.412 * 0.25, y * 684.412 * 0.375, z * 684.412 * 0.25);
        let smear = 684.412 * 0.375 * 8.0;
        let mut selector = 0.0;
        let mut frequency = 1.0;
        for octave in &self.selector {
            selector += octave.at3(fx / 80.0 * frequency, fy / 60.0 * frequency,
                fz / 80.0 * frequency, smear / 60.0 * frequency) / frequency;
            frequency *= 0.5;
        }
        let blend = ((selector / 10.0 + 1.0) * 0.5).clamp(0.0, 1.0);
        let (mut lower, mut upper) = (0.0, 0.0);
        frequency = 1.0;
        for (low, high) in self.lower.iter().zip(&self.upper) {
            if blend < 1.0 {
                lower += low.at3(fx * frequency, fy * frequency, fz * frequency, smear * frequency) / frequency;
            }
            if blend > 0.0 {
                upper += high.at3(fx * frequency, fy * frequency, fz * frequency, smear * frequency) / frequency;
            }
            frequency *= 0.5;
        }
        let density = (lower * (1.0 - blend) + upper * blend) / 65_536.0;
        let top = ((128.0 - y) / 24.0).clamp(0.0, 1.0);
        let density = 0.9375 * (1.0 - top) + density * top;
        let bottom = ((y + 8.0) / 32.0).clamp(0.0, 1.0);
        2.5 * (1.0 - bottom) + density * bottom
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

#[cfg(all(test, not(seedmap_core)))]
mod tests {
    use super::*;

    /// Фикстура снята только с биомов сохранённых регионов оригинала 26.3.
    #[test]
    fn census_biomes_match_both_seeds() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../testdata/world-profile/genaudit-26.3/nether-biomes.json"
        )).unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let nether = Nether::new(case["seed"].as_i64().unwrap());
            for point in case["points"].as_array().unwrap() {
                let (x, z) = (point[0].as_i64().unwrap() as i32, point[1].as_i64().unwrap() as i32);
                assert_eq!(nether.biome_at(x, z).name(), point[2].as_str().unwrap(), "{}, {x}, {z}", nether.seed);
            }
            let mut counts = std::collections::BTreeMap::new();
            for x in (-256..272).step_by(4) {
                for z in (-256..272).step_by(4) {
                    *counts.entry(nether.biome_at(x, z).name()).or_insert(0i64) += 1;
                }
            }
            for biome in Biome::NETHER {
                let actual = counts.get(biome.name()).copied().unwrap_or(0);
                let expected = case["counts"][biome.name()].as_i64().unwrap_or(0);
                assert!((actual - expected).abs() <= 8, "{} {}: {actual} / {expected}", nether.seed, biome.name());
            }
            // Проверяем также биомы в реально сложенном чанке и отрицательные клетки.
            for (cx, cz) in [(-16, -16), (-1, -1), (0, 0), (16, 16)] {
                let chunk = nether.chunk(cx, cz);
                for x in 0..4 {
                    for z in 0..4 {
                        let expected = nether.biome_at(cx * 16 + x as i32 * 4, cz * 16 + z as i32 * 4).number() as BiomeId;
                        for layer in 0..BIOME_LAYERS {
                            assert_eq!(chunk.biomes[biome_cell(layer, x, z)], expected);
                        }
                    }
                }
            }
        }
    }

    /// Независимые блоки оригинала проверяют фазу поля и оба сплошных края.
    #[test]
    fn census_terrain_phase_and_slides() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../testdata/world-profile/genaudit-26.3/nether-terrain.json"
        )).unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let n = Nether::new(case["seed"].as_i64().unwrap());
            let points = case["points"].as_array().unwrap();
            let different = points.iter().filter(|p| {
                let (x,y,z) = (p[0].as_i64().unwrap() as i32,p[1].as_i64().unwrap() as i32,p[2].as_i64().unwrap() as i32);
                (n.density(x,y,z) > 0.0) != p[3].as_bool().unwrap()
            }).count();
            // Поверхность, объекты и узкие ходы — отдельные стадии.
            assert!(different * 100 <= points.len() * 5, "{}: {different}/{}", n.seed, points.len());
            // Очистка кэша и порядок запросов не меняют ни поле, ни чанк.
            let before = n.chunk(-1,0);
            n.density_columns.lock().unwrap().clear();
            let _ = n.chunk(1,-1);
            let after = n.chunk(-1,0);
            for y in 0..=CEILING {
                for x in 0..16 {
                    for z in 0..16 {
                        assert_eq!(before.block(x,y,z),after.block(x,y,z));
                    }
                }
            }
        }
    }

    /// Отсечение дальних пятен не должно менять поток следующих объектов.
    #[test]
    fn ore_halo_culling_preserves_the_random_stream() {
        let n=Nether::new(1_792_932_773_345_722_119);
        for (cx,cz) in [(-1,-1),(0,0),(1,-1)] {
            let mut fast=Piece { x0:cx*16,z0:cz*16,blocks:vec![n.states.netherrack;PIECE] };
            let mut reference=Piece { x0:cx*16,z0:cz*16,blocks:fast.blocks.clone() };
            n.blobs_with_culling(&mut fast,cx,cz,true);
            n.blobs_with_culling(&mut reference,cx,cz,false);
            assert_eq!(fast.blocks,reference.blocks,"{cx},{cz}");
        }
    }

    /// Разреженная выборка настоящих чанков обоих окон переписи.
    /// Для 81 чанка допуск шире полного окна; верхний проход обломков
    /// проверяется отдельно, чтобы среднее не скрывало неверную высоту.
    #[test]
    fn census_ore_density_and_upper_debris_pass() {
        for (seed, expected) in [
            (1_792_932_773_345_722_119, [40.550,116.819,1.557]),
            (100_554_032_945_340, [30.848,90.104,1.374]),
        ] {
            let n = Nether::new(seed);
            let states = [n.states.gold,n.states.quartz,n.states.debris];
            let mut counts = [0usize;3];
            let mut high_debris = 0;
            for cx in (-16..=16).step_by(4) {
                for cz in (-16..=16).step_by(4) {
                    let chunk = n.chunk(cx,cz);
                    for y in 0..=CEILING {
                        for x in 0..16 {
                            for z in 0..16 {
                                let state = chunk.block(x,y,z);
                                for (i, ore) in states.iter().enumerate() {
                                    counts[i] += usize::from(state == *ore);
                                }
                                high_debris += usize::from(state == n.states.debris && y >= 32);
                            }
                        }
                    }
                }
            }
            for i in 0..3 {
                let actual = counts[i] as f64 / 81.0;
                assert!((actual - expected[i]).abs() <= expected[i] * 0.30,
                    "{seed} {}: {actual} / {}", states[i], expected[i]);
            }
            assert!(high_debris > 0);
        }
    }

    #[test]
    fn census_fortress_starts_and_rejected_delta_bastions() {
        for (seed, starts) in [
            (1_792_932_773_345_722_119, vec![(-6i32, -5i32)]),
            (100_554_032_945_340, vec![(-13i32, -6i32), (-9, 3), (4, -14)]),
        ] {
            let nether = Nether::new(seed);
            for (cx, cz) in starts {
                assert_eq!(nether.fortress_start(cx.div_euclid(27), cz.div_euclid(27)), Some((cx, cz)));
                assert!(nether.bastion_start(cx.div_euclid(27), cz.div_euclid(27)).is_none());
            }
            let mut rejected = 0;
            for rx in -8..8 {
                for rz in -8..8 {
                    let (cx, cz, roll) = fortress::region_start(seed, rx, rz);
                    if roll >= fortress::FORTRESS_SHARE && nether.biome_at(cx * 16 + 8, cz * 16 + 8) == Biome::BasaltDeltas {
                        assert!(nether.fortress_start(rx, rz).is_none());
                        assert!(nether.bastion_start(rx, rz).is_none());
                        rejected += 1;
                    }
                }
            }
            assert!(rejected > 0);
        }
    }

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
    fn no_single_block_slabs_between_caverns() {
        assert_eq!(membrane(Void::Cavern, Void::Cavern), Some(Void::Cavern));
        assert_eq!(membrane(Void::Tunnel, Void::Cavern), None);
        assert_eq!(membrane(Void::Rock, Void::Cavern), None);

        // Рельеф куска и решение по координатам сходятся, перемычек в блок
        // между пещерами нет.
        let nether = Nether::new(7);
        let biomes = vec![Biome::NetherWastes; 256];
        let mut piece = Piece { x0: 32, z0: -48, blocks: vec![AIR; PIECE] };
        nether.relief(&mut piece, &biomes);
        for at in 0..256 {
            let (lx, lz) = (at % 16, at / 16);
            for y in MEMBRANES {
                let (x, z) = (32 + lx, -48 + lz);
                assert_eq!(nether.hollow(x, y, z), is_air(piece.blocks[Piece::index(lx, y, lz)]));
                let slab = !nether.hollow(x, y, z)
                    && nether.void(x, y - 1, z) == Void::Cavern
                    && nether.void(x, y + 1, z) == Void::Cavern
                    && nether.raw_void(x, y - 1, z) == Void::Cavern
                    && nether.raw_void(x, y + 1, z) == Void::Cavern;
                assert!(!slab, "перемычка в ({x}, {y}, {z})");
            }
        }
    }

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

        // Блоки — по чанкам: 128 квадратов 5×5 чанков вразброс по ±9 тысяч
        // блоков (пещеры и ходы крупные — выборка должна захватить многие).
        let places: Vec<(i32, i32)> = (0..128)
            .flat_map(|area| {
                let (ax, az) = ((area % 16) * 73 - 580, (area / 16) * 131 - 560);
                (0..25).map(move |at| (ax + at % 5, az + at / 5))
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
                                (nether.piece_with(cx, cz, false), biomes)
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
        // Биом → (блок, полоса по 8 блоков высоты) → штук.
        let mut heights: HashMap<&str, HashMap<(&str, usize), f64>> = HashMap::new();

        for (piece, biomes) in &chunks {
            for at in 0..256 {
                let (lx, lz) = (at % 16, at / 16);
                let biome = biomes[at as usize].name();
                let column: Vec<i32> = (0..=CEILING).map(|y| piece.blocks[Piece::index(lx, y, lz)]).collect();

                *columns.entry(biome).or_default() += 1.0;

                for (y, &state) in column.iter().enumerate() {
                    *volume.entry(biome).or_default().entry(name(state)).or_default() += 1.0;

                    if !is_air(state) && !matches!(name(state), "bedrock" | "lava" | "netherrack") {
                        *heights.entry(biome).or_default().entry((name(state), y / 8 * 8)).or_default() += 1.0;
                    }

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

        println!("\n== высоты (блок: доли по 8 блокам высоты, в штуках на столбец)");
        for (biome, count) in &order {
            let Some(table) = heights.get(biome) else { continue };
            let mut names: Vec<&str> = table.keys().map(|(block, _)| *block).collect();
            names.sort();
            names.dedup();

            for block in names {
                let row: Vec<String> = (0..128)
                    .step_by(8)
                    .filter_map(|y| table.get(&(block, y)).map(|n| format!("{}:{:.3}", y, n / count)))
                    .collect();
                println!("  {:<16} {:<20} {}", biome, block, row.join(" "));
            }
        }
    }

    /// Незер по слоям — тот же вывод, что пишет с мира оригинала
    /// tools/measure/nether_layers.py: доли блоков на каждой высоте, полы и
    /// потолки на столбец, толщина пустот и перемычек. Сверка —
    /// tools/measure/nether_layers_compare.py.
    /// `NETHER_LAYERS=файл.tsv cargo test --release nether_layers -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn nether_layers() {
        use std::collections::HashMap;
        use std::fmt::Write as _;

        let nether = Nether::new(100_554_032_945_340);
        let out = std::env::var("NETHER_LAYERS").unwrap_or_else(|_| "nether_layers_ours.tsv".into());
        let name = |state: i32| if state == TUNNEL { "cave_air" } else { blocks::block_at_state(state).unwrap_or("?") };
        let plants = [
            "fire",
            "soul_fire",
            "crimson_roots",
            "warped_roots",
            "nether_sprouts",
            "crimson_fungus",
            "warped_fungus",
            "brown_mushroom",
            "red_mushroom",
            "weeping_vines",
            "weeping_vines_plant",
            "twisting_vines",
            "twisting_vines_plant",
        ];

        // 768 квадратов 2×2 чанка вразброс по ±50 тысяч блоков. Пещеры
        // крупные и по высоте тянутся на десятки блоков: сотня квадратов
        // 5×5 врала на 2–3 % доли пустот на целых полосах высоты (у начала
        // на y 48–71 выходило 0,44 против 0,46 по самому шуму на всём мире,
        // в другом месте — наоборот), нужны много мелких мест.
        const AREAS: i32 = 768;
        const SIDE: i32 = 2;
        let places: Vec<(i32, i32)> = (0..AREAS)
            .flat_map(|area| {
                let (ax, az) = ((area % 32) * 197 - 3100, (area / 32) * 263 - 3150);
                (0..SIDE * SIDE).map(move |at| (ax + at % SIDE, az + at / SIDE))
            })
            .collect();
        let pieces: Vec<Piece> = std::thread::scope(|scope| {
            let workers: Vec<_> = places
                .chunks(places.len().div_ceil(8))
                .map(|part| {
                    let nether = &nether;
                    scope.spawn(move || part.iter().map(|&(cx, cz)| nether.piece_with(cx, cz, false)).collect::<Vec<_>>())
                })
                .collect();
            workers.into_iter().flat_map(|worker| worker.join().unwrap()).collect()
        });

        let mut per_y: Vec<HashMap<&str, u64>> = vec![HashMap::new(); 128];
        let mut stats: [HashMap<i32, u64>; 6] = Default::default();
        let [floors, ceilings, floor_y, ceiling_y, gaps, slabs] = &mut stats;

        for piece in &pieces {
            for at in 0..256 {
                let column: Vec<&str> = (0..=CEILING).map(|y| name(piece.blocks[Piece::index(at % 16, y, at / 16)])).collect();
                let open: Vec<bool> =
                    column.iter().map(|block| matches!(*block, "air" | "cave_air") || plants.contains(block)).collect();

                for (y, block) in column.iter().enumerate() {
                    *per_y[y].entry(block).or_default() += 1;
                }

                let (mut floor_count, mut ceiling_count) = (0, 0);

                for y in 32..126 {
                    if !open[y] && column[y] != "lava" && open[y + 1] {
                        floor_count += 1;
                        *floor_y.entry(y as i32).or_default() += 1;
                    }
                    if !open[y] && column[y] != "lava" && open[y - 1] && column[y - 1] != "lava" {
                        ceiling_count += 1;
                        *ceiling_y.entry(y as i32).or_default() += 1;
                    }
                }

                *floors.entry(floor_count).or_default() += 1;
                *ceilings.entry(ceiling_count).or_default() += 1;

                let mut y = 32;

                while y < 127 {
                    let mut top = y;

                    while top < 127 && open[top] == open[y] {
                        top += 1;
                    }

                    if open[y] {
                        *gaps.entry((top - y).min(99) as i32).or_default() += 1;
                    } else if y > 32 && top < 127 {
                        *slabs.entry((top - y).min(99) as i32).or_default() += 1;
                    }

                    y = top;
                }
            }
        }

        // Как далеко по горизонтали тянется пустота: доля пустых среди мест
        // в d блоках по x от пустого (по квадратам 5×5 чанков).
        let mut reach: HashMap<(i32, i32), (u64, u64)> = HashMap::new();
        let width = SIDE * 16;
        for group in pieces.chunks((SIDE * SIDE) as usize) {
            for y in [40, 56, 72, 88] {
                let open_at = |x: i32, z: i32| {
                    let piece = &group[((z / 16) * SIDE + x / 16) as usize];
                    let name = name(piece.blocks[Piece::index(x % 16, y, z % 16)]);
                    name == "air" || name == "cave_air"
                };
                for z in 0..width {
                    for x in 0..width {
                        if !open_at(x, z) {
                            continue;
                        }
                        for d in [2, 4, 8, 16, 32] {
                            if x + d < width {
                                let entry = reach.entry((y, d)).or_default();
                                entry.0 += 1;
                                entry.1 += u64::from(open_at(x + d, z));
                            }
                        }
                    }
                }
            }
        }

        let columns = (pieces.len() * 256) as f64;
        let mut text = format!("chunks\t-\t{}\n", pieces.len());
        let mut keys: Vec<_> = reach.keys().copied().collect();
        keys.sort();
        for (y, d) in keys {
            let (all, open) = reach[&(y, d)];
            let _ = writeln!(text, "reach{}\t{}\t{:.6}", y, d, open as f64 / all as f64);
        }

        for (y, row) in per_y.iter().enumerate() {
            let total: u64 = row.values().sum();

            for (block, count) in row {
                let _ = writeln!(text, "y{}\t{}\t{:.6}", y, block, *count as f64 / total as f64);
            }
        }

        for (title, table) in ["floors", "ceilings", "floor_y", "ceiling_y", "gap", "slab"].iter().zip(&stats) {
            let mut rows: Vec<_> = table.iter().collect();
            rows.sort();

            for (value, count) in rows {
                let _ = writeln!(text, "{}\t{}\t{:.6}", title, value, *count as f64 / columns);
            }
        }

        std::fs::write(&out, text).unwrap();
        println!("чанков: {} → {}", pieces.len(), out);
    }

    /// Крепости: доли построек по регионам и блоки в столбцах под рамками
    /// кусков — тот же счёт, что `nether_fortress.py blocks` с мира
    /// оригинала. `cargo test --release fortress_blocks -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn fortress_blocks() {
        use std::collections::{HashMap, HashSet};

        let nether = Nether::new(100_554_032_945_340);
        let mut regions: Vec<(i32, i32)> = (-20..20).flat_map(|x| (-20..20).map(move |z| (x, z))).collect();
        let (mut fortresses, mut deltas) = (0, 0);

        for &(x, z) in &regions {
            if let Some((cx, cz)) = nether.fortress_start(x, z) {
                fortresses += 1;
                deltas += usize::from(nether.biome_at(cx * 16 + 8, cz * 16 + 8) == Biome::BasaltDeltas);
            }
        }
        println!("регионов {}, крепостей {} ({:.1} %), из них в дельтах {}", regions.len(), fortresses, 100.0 * fortresses as f64 / regions.len() as f64, deltas);

        regions.sort_by_key(|(x, z)| x * x + z * z);
        // Сколько крепостей брать и сколько ближайших пропустить: у одной
        // крепости все опоры стоят на одном рельефе, и 40 крепостей дают
        // разброс долей по высоте в несколько процентов.
        let env = |name: &str, default: usize| std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default);
        let starts: Vec<(i32, i32)> = regions
            .iter()
            .filter_map(|&(x, z)| nether.fortress_start(x, z))
            .skip(env("FORTRESS_SKIP", 0))
            .take(env("FORTRESSES", 40))
            .collect();
        let plans: Vec<Vec<fortress::Placed>> = starts.iter().map(|&(x, z)| fortress::plan(nether.seed, x, z)).collect();
        // Столбец → высоты рамок кусков над ним (низ, верх).
        type Spans = HashMap<(i32, i32), Vec<(i32, i32)>>;
        let mut columns: Vec<Spans> = Vec::new();
        let mut chunks: HashSet<(i32, i32)> = HashSet::new();

        for plan in &plans {
            let mut set = Spans::new();
            for piece in plan {
                let b = piece.bounds;
                for x in b[0]..=b[3] {
                    for z in b[2]..=b[5] {
                        set.entry((x, z)).or_default().push((b[1], b[4]));
                        chunks.insert((x >> 4, z >> 4));
                    }
                }
            }
            columns.push(set);
        }

        let chunks: Vec<(i32, i32)> = chunks.into_iter().collect();
        let pieces: HashMap<(i32, i32), Piece> = std::thread::scope(|scope| {
            let workers: Vec<_> = chunks
                .chunks(chunks.len().div_ceil(8))
                .map(|part| {
                    let nether = &nether;
                    scope.spawn(move || part.iter().map(|&(cx, cz)| ((cx, cz), nether.piece(cx, cz))).collect::<Vec<_>>())
                })
                .collect();
            workers.into_iter().flat_map(|worker| worker.join().unwrap()).collect()
        });
        let mut total: HashMap<&str, u64> = HashMap::new();
        let counted = ["nether_bricks", "nether_brick_fence", "nether_brick_stairs", "spawner", "chest", "nether_wart", "soul_sand", "lava"];

        for set in &columns {
            for (&(x, z), spans) in set {
                let piece = &pieces[&(x >> 4, z >> 4)];
                for y in 1..127 {
                    let name = blocks::block_at_state(piece.blocks[Piece::index(x & 15, y, z & 15)]).unwrap_or("?");
                    if counted.contains(&name) {
                        *total.entry(name).or_default() += 1;
                    }
                    if name == "nether_bricks" && y < 45 {
                        *total.entry("nether_bricks ниже y 45").or_default() += 1;
                    }
                    if name == "nether_bricks" && !spans.iter().any(|(low, high)| (*low..=*high).contains(&y)) {
                        *total.entry("nether_bricks вне рамок (опоры)").or_default() += 1;
                    }
                }
            }
        }

        println!("крепостей {}, кусков в среднем {:.1}", plans.len(), plans.iter().map(Vec::len).sum::<usize>() as f64 / plans.len() as f64);

        // Опоры по видам кусков: кирпич под низом куска, блоков на кусок.
        let bricks = blocks::state_by_name("nether_bricks").unwrap();
        let mut supports: HashMap<&str, (u64, u64)> = HashMap::new();
        let mut ends: HashMap<i32, u64> = HashMap::new();
        let mut below: HashMap<&str, u64> = HashMap::new();
        let name = |state: i32| if state == TUNNEL { "cave_air" } else { blocks::block_at_state(state).unwrap_or("?") };
        for plan in &plans {
            for placed in plan {
                let entry = supports.entry(placed.kind.id()).or_default();
                entry.1 += 1;
                for (x, bottom, z) in placed.support_columns() {
                    let piece = &pieces[&(x >> 4, z >> 4)];
                    let mut y = bottom - 1;
                    while y > 0 && piece.blocks[Piece::index(x & 15, y, z & 15)] == bricks {
                        entry.0 += 1;
                        y -= 1;
                    }
                    if y < bottom - 1 && matches!(placed.kind.id(), "nebcr" | "nebs") {
                        *ends.entry(y / 4 * 4).or_default() += 1;
                        *below.entry(name(piece.blocks[Piece::index(x & 15, y, z & 15)])).or_default() += 1;
                    }
                }
            }
        }
        let mut kinds: Vec<_> = supports.into_iter().collect();
        kinds.sort();
        for (kind, (length, count)) in kinds {
            println!("  опоры {:<7} {:6.0}", kind, length as f64 / count as f64);
        }
        let mut ends: Vec<_> = ends.into_iter().collect();
        ends.sort();
        println!("  низ опор мостов по высоте: {:?}", ends);
        let mut below: Vec<_> = below.into_iter().collect();
        below.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
        println!("  под опорами: {:?}", &below[..below.len().min(8)]);
        let mut rows: Vec<_> = total.into_iter().collect();
        rows.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
        for (name, count) in rows {
            println!("  {:<22} {:8.1}", name, count as f64 / plans.len() as f64);
        }
    }

    /// Крепость видна в чанках: кирпич есть, стыки чанков одинаковы при
    /// любом порядке складывания, одно семя — одни байты чанка.
    #[test]
    fn fortress_chunks_are_deterministic() {
        let nether = Nether::new(100_554_032_945_340);
        let (start_x, start_z) = (-20..20)
            .flat_map(|x| (-20..20).map(move |z| (x, z)))
            .find_map(|(x, z)| nether.fortress_start(x, z))
            .expect("крепость в 1600 регионах");
        let bricks = blocks::state_by_name("nether_bricks").unwrap();
        let one = nether.piece(start_x, start_z);
        let two = Nether::new(100_554_032_945_340).piece(start_x, start_z);

        assert!(one.blocks.iter().filter(|state| **state == bricks).count() > 100);
        assert_eq!(one.blocks, two.blocks);
        assert_eq!(nether.chunk(start_x, start_z).to_bytes(), Nether::new(100_554_032_945_340).chunk(start_x, start_z).to_bytes());
    }

    /// Бастионы: доля регионов и блоки в рамках кусков — тот же счёт, что
    /// `nether_bastion.py blocks` с мира оригинала, и время чанка с
    /// бастионом. `cargo test --release bastion_blocks -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn bastion_blocks() {
        use std::collections::{HashMap, HashSet};
        use std::time::Instant;

        let nether = Nether::new(100_554_032_945_340);
        let mut regions: Vec<(i32, i32)> = (-20..20).flat_map(|x| (-20..20).map(move |z| (x, z))).collect();
        let found = regions.iter().filter(|&&(x, z)| nether.bastion_start(x, z).is_some()).count();
        println!("регионов {}, бастионов {} ({:.1} %)", regions.len(), found, 100.0 * found as f64 / regions.len() as f64);

        regions.sort_by_key(|(x, z)| x * x + z * z);
        let plans: Vec<Vec<bastion::Placed>> = regions
            .iter()
            .filter_map(|&(x, z)| nether.bastion_start(x, z))
            .take(40)
            .map(|(x, z)| bastion::plan(nether.seed, x, z))
            .collect();
        let mut cache: HashMap<(i32, i32), Piece> = HashMap::new();
        let mut total: HashMap<&str, u64> = HashMap::new();
        let (mut chunks, mut spent) = (0, 0.0);

        for plan in &plans {
            let mut cells = HashSet::new();
            for placed in plan {
                let b = placed.bounds;
                for x in b[0]..=b[3] {
                    for y in b[1]..=b[4] {
                        for z in b[2]..=b[5] {
                            cells.insert((x, y, z));
                        }
                    }
                }
            }
            for &(x, y, z) in &cells {
                let piece = cache.entry((x >> 4, z >> 4)).or_insert_with(|| {
                    let started = Instant::now();
                    let piece = nether.piece(x >> 4, z >> 4);
                    spent += started.elapsed().as_secs_f64() * 1000.0;
                    chunks += 1;
                    piece
                });
                let state = piece.blocks[Piece::index(x & 15, y, z & 15)];
                let name = if state == TUNNEL { "cave_air" } else { blocks::block_at_state(state).unwrap_or("?") };
                *total.entry(name).or_default() += 1;
            }
        }

        println!("бастионов {}, чанков {} по {:.2} мс", plans.len(), chunks, spent / chunks as f64);
        let mut rows: Vec<_> = total.into_iter().collect();
        rows.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
        for (name, count) in rows.into_iter().take(24) {
            println!("  {:<36} {:8.1}", name, count as f64 / plans.len() as f64);
        }
    }

    /// Бастион виден в чанке старта и складывается одинаково.
    #[test]
    fn bastion_chunks_are_deterministic() {
        let nether = Nether::new(100_554_032_945_340);
        let (start_x, start_z) = (0..10).flat_map(|x| (0..10).map(move |z| (x, z))).find_map(|(x, z)| nether.bastion_start(x, z)).unwrap();
        let one = nether.piece(start_x, start_z);
        let two = Nether::new(100_554_032_945_340).piece(start_x, start_z);
        let stone = ["polished_blackstone_bricks", "cracked_polished_blackstone_bricks", "blackstone"].map(|name| blocks::state_by_name(name).unwrap());

        assert!(one.blocks.iter().filter(|state| stone.contains(state)).count() > 100);
        assert_eq!(one.blocks, two.blocks);
    }

    /// Скорость: сколько в среднем складывается чанк Незера и из чего это
    /// время. `cargo test --release nether_chunk_phases -- --ignored --nocapture`.
    /// Куски лесов для счёта шляп огромных грибов — тот же счёт, что
    /// `tools/measure/nether_fungus.py region` по миру оригинала:
    /// `NETHER_FUNGUS=файл cargo test --release nether_fungus_dump -- --ignored`,
    /// потом `nether_fungus.py dump файл`.
    #[test]
    #[ignore]
    fn nether_fungus_dump() {
        let nether = Nether::new(100_554_032_945_340);
        let out = std::env::var("NETHER_FUNGUS").unwrap_or_else(|_| "nether_fungus_ours.bin".into());
        let side = 12;
        // Квадраты 12×12 чанков, серединой в лесу.
        let mut areas = Vec::new();
        for at in 0..4000 {
            let (ax, az) = ((at % 63) * 37 - 1100, (at / 63) * 41 - 1300);
            let biome = nether.biome_at(ax * 16 + side * 8, az * 16 + side * 8);
            let wanted = if areas.len() % 3 == 2 { Biome::WarpedForest } else { Biome::CrimsonForest };
            if biome == wanted {
                areas.push((ax, az));
            }
            if areas.len() == 24 {
                break;
            }
        }
        let code = |state: i32| -> u8 {
            if is_air(state) {
                return 0;
            }
            match blocks::block_at_state(state).unwrap_or("?") {
                "crimson_stem" => 1,
                "nether_wart_block" => 2,
                "shroomlight" => 3,
                "crimson_nylium" | "warped_nylium" => 4,
                "crimson_roots" | "warped_roots" | "nether_sprouts" | "crimson_fungus" | "warped_fungus" | "weeping_vines"
                | "weeping_vines_plant" | "twisting_vines" | "twisting_vines_plant" | "fire" => 5,
                "warped_stem" => 7,
                "warped_wart_block" => 8,
                "lava" => 9,
                _ => 6,
            }
        };
        let width = (side * 16) as usize;
        let grids: Vec<Vec<u8>> = std::thread::scope(|scope| {
            let workers: Vec<_> = areas
                .iter()
                .map(|&(ax, az)| {
                    let nether = &nether;
                    scope.spawn(move || {
                        let mut grid = vec![6u8; width * width * 96];
                        for cz in 0..side {
                            for cx in 0..side {
                                let piece = nether.piece_with(ax + cx, az + cz, false);
                                for y in 32..128 {
                                    for lz in 0..16 {
                                        for lx in 0..16 {
                                            let (x, z) = ((cx * 16 + lx) as usize, (cz * 16 + lz) as usize);
                                            grid[((y as usize - 32) * width + z) * width + x] =
                                                code(piece.blocks[Piece::index(lx, y, lz)]);
                                        }
                                    }
                                }
                            }
                        }
                        grid
                    })
                })
                .collect();
            workers.into_iter().map(|worker| worker.join().unwrap()).collect()
        });
        let mut bytes = Vec::new();
        for grid in grids {
            bytes.extend_from_slice(&(width as u32).to_le_bytes());
            bytes.extend_from_slice(&grid);
        }
        std::fs::write(&out, bytes).unwrap();
        println!("квадратов: {} → {}", areas.len(), out);
    }

    #[test]
    #[ignore]
    fn nether_chunk_phases() {
        use std::time::{Duration, Instant};

        let nether = Nether::new(100_554_032_945_340);
        let count = 64;
        let mut phases = [Duration::ZERO; 6];
        let mut total = Duration::ZERO;

        // Пороги считаются один раз на мир — не в счёт.
        nether.thresholds();

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

// Вики, Bastion Remnant: «magma cube spawner». Запись SpawnData в мире
// 26.1.2, seed 100554032945340: (-8406,39,478). Маска рисуется тем же
// построителем и в том же порядке: одной рамки недостаточно при наложении.
#[cfg(not(seedmap_core))]
impl Nether {
    pub(super) fn bastion_spawners(&self, cx: i32, cz: i32) -> std::collections::HashSet<(i32,i32,i32)> {
        let plans = self.bastions_near(cx, cz);
        if plans.is_empty() { return std::collections::HashSet::new(); }
        let mut mask = vec![AIR; 128*16*16];
        let mut canvas = fortress::Canvas { blocks: &mut mask, x0: cx*16, z0: cz*16, passable: &|_| true, air: AIR, supports: false };
        for plan in plans { bastion::paint(self.seed, &plan, &mut canvas); }
        let spawner = blocks::state_by_name("spawner").unwrap();
        mask.iter().enumerate().filter_map(|(i,&state)| (state == spawner).then_some((i as i32%16,i as i32/256,i as i32/16%16))).collect()
    }
}
