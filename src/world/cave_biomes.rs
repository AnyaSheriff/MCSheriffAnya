// Пещерные биомы: пышные пещеры, капельниковые пещеры и глубокая тьма.
//
// Как их выбирает игра (вики, «World generation» → таблица биомов): у каждой
// клетки 4×4×4 кроме пяти чисел климата есть шестое — «глубина», около нуля
// у поверхности и на 1/128 больше с каждым блоком вниз. У наземных биомов
// глубина 0 и 1, у пещерных — свои промежутки:
//
//   пышные пещеры         влажность 0,7…1,0          глубина 0,2…0,9
//   капельниковые пещеры  континентальность 0,8…1,0  глубина 0,2…0,9
//   глубокая тьма         эрозия −1,0…−0,375         глубина 1,1
//
// а остальные числа у них во весь промежуток. Биом выбирается ближайший —
// по сумме квадратов расстояний до его промежутков. Поэтому пещерный биом
// захватывает и места, где его число чуть не дотягивает, если наземный
// оттуда ещё дальше по глубине, и у самой поверхности пещерных биомов нет:
// там ближе всего наземный с глубиной 0.
//
// Числа климата у нас свои и распределены иначе, чем у игры: суши больше,
// а континентальность у пятой части столбцов упирается в 1. Поэтому две
// поправки, подогнанные по замеру мира оригинала (доли клеток ниже
// поверхности: наземных 82,9%, капельниковых 8,1%, глубокой тьмы 4,9%,
// пышных 4,1%; у нас с поправками — 83,4 / 8,5 / 4,2 / 4,0):
//   - промах по климату весит втрое больше промаха по глубине — иначе
//     пещерный биом расползается далеко за свой промежуток;
//   - капельниковым пещерам нужна континентальность 1 (самая глубь суши),
//     а глубина — точкой 0,5 посреди промежутка; по таблице вики они
//     занимали бы треть всей толщи.
//
// Глубину мы считаем от поверхности столбца: у игры она идёт от сглаженной
// высоты рельефа, у нас — от настоящей, по смыслу это то же самое. Глубокая
// тьма поэтому бывает только там, где до дна мира больше ~135 блоков, то есть
// под горами, — как в игре.
//
// Украшения — после растительности, отдельным проходом: мох, азалии, лужи
// с глиной и капельными листьями, пещерные лозы и споровые цветы в пышных
// пещерах; пятна капельника, сталактиты и сталагмиты в капельниковых;
// скалк, его жилы, датчики и крикуны в глубокой тьме. Над пышными пещерами
// на поверхности иногда растёт дерево с азалией, от него вниз идут корни.
//
// Всё решается числом от координат и семени мира: пятна — объёмным шумом,
// растения — перемешанным числом от точки. Сталактиты, лозы и листья стоят
// в своём столбце, поэтому чанку не нужно знать соседей; дерево с азалией
// больше столбца, и его, как обычные деревья, рисует каждый чанк, которого
// оно касается. Скалк смотрит на соседние блоки во все стороны, поэтому у
// граней чанка глубокой тьме нужна кайма — блоки соседа в полосу шириной
// в блок (`Rim`): каждый чанк ставит только свои блоки, но решает по пустотам
// с обеих сторон границы, и шва на ней нет.

use std::sync::OnceLock;

use super::noise::Noise;
use super::terrain::{self, Biome, Climate, Column, ReliefGrid, Terrain, SEA};
use super::underground::{hash, scramble, unit};
use super::{trees, Chunk, Generator, AIR, CHUNK_SIZE, MIN_Y, WORLD_HEIGHT};

include!("cave_choice.rs");

// ---------------------------------------------------------------------------
// Украшения
// ---------------------------------------------------------------------------

/// Какой пещерный биом у клетки.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Cave {
    Lush,
    Dripstone,
    DeepDark,
}

/// Номера пещерных биомов в реестре — чтобы узнавать их в клетках чанка.
struct Numbers {
    lush: u16,
    dripstone: u16,
    deep_dark: u16,
}

impl Numbers {
    fn new() -> Numbers {
        Numbers {
            lush: Biome::LushCaves.number() as u16,
            dripstone: Biome::DripstoneCaves.number() as u16,
            deep_dark: Biome::DeepDark.number() as u16,
        }
    }

    fn cave(&self, biome: u16) -> Option<Cave> {
        if biome == self.lush {
            Some(Cave::Lush)
        } else if biome == self.dripstone {
            Some(Cave::Dripstone)
        } else if biome == self.deep_dark {
            Some(Cave::DeepDark)
        } else {
            None
        }
    }
}

/// Верх мира — первая высота, где блоков уже нет.
const TOP: i32 = MIN_Y + WORLD_HEIGHT;

/// Вопросы, которые задаются числу от точки: у каждого свой.
const SALT_FLOOR_PLANT: u64 = 201;
const SALT_CEILING_PLANT: u64 = 202;
const SALT_LENGTH: u64 = 203;
const SALT_BERRIES: u64 = 204;
const SALT_FACING: u64 = 205;
const SALT_DRIPLEAF: u64 = 206;
const SALT_STALACTITE: u64 = 207;
const SALT_STALAGMITE: u64 = 208;
const SALT_SCULK_BLOCK: u64 = 209;
const SALT_AZALEA: u64 = 210;
const SALT_ROOTS: u64 = 211;

/// Семена шумов пятен.
const SALT_NOISE_MOSS: u64 = 301;
const SALT_NOISE_POOL: u64 = 302;
const SALT_NOISE_DRIPSTONE: u64 = 303;
const SALT_NOISE_SCULK: u64 = 304;

/// Размер пятен мха и капельника в блоках, лужи шире, скалк — крупными
/// языками.
const MOSS_SCALE: f64 = 5.0;
const POOL_SCALE: f64 = 7.0;
const DRIPSTONE_SCALE: f64 = 5.0;
const SCULK_SCALE: f64 = 7.0;

/// Сдвиг координат шума: в целых точках шум Перлина равен нулю, и пятна
/// шли бы решёткой.
const NOISE_OFFSET: f64 = 0.371;

/// Мох на полу — где шум выше этого: почти весь пол пышной пещеры, у
/// потолка чуть реже.
const MOSS_FLOOR: f64 = -0.15;
const MOSS_CEILING: f64 = -0.05;

/// Лужа — где шум луж выше этого; ещё немного ниже — глиняный берег.
const POOL: f64 = 0.3;
const POOL_RIM: f64 = 0.22;

/// Что растёт на мху (доли столбцов): ковёр мха, трава, высокая трава,
/// азалия, цветущая азалия. Остальное — голый мох.
const MOSS_CARPET: f64 = 0.25;
const SHORT_GRASS: f64 = 0.2;
const TALL_GRASS: f64 = 0.05;
const AZALEA_BUSH: f64 = 0.04;
const FLOWERING_AZALEA_BUSH: f64 = 0.025;

/// Капельный лист в воде: малый в воде глубиной в блок, большой — в воде
/// до четырёх блоков.
const SMALL_DRIPLEAF: f64 = 0.25;
const BIG_DRIPLEAF: f64 = 0.12;
const DRIPLEAF_WATER: i32 = 4;

/// С потолка: пещерная лоза и споровый цветок. Лоза длиной до восьми
/// блоков, ягоды — на каждом седьмом блоке лозы.
const CAVE_VINES: f64 = 0.12;
const CAVE_VINES_LONGEST: u64 = 8;
const BERRIES: f64 = 0.14;
const SPORE_BLOSSOM: f64 = 0.01;

/// Капельник: пятна блоков — где шум выше этого, у потолка и у пола.
const DRIPSTONE_PATCH: f64 = -0.05;

/// Сталактит — с каждого седьмого места потолка, длиной до восьми блоков
/// (вики, «Pointed Dripstone»); под сталактитом чаще всего сталагмит,
/// одинокий сталагмит редок.
const STALACTITE: f64 = 0.14;
const STALAGMITE_UNDER: f64 = 0.6;
const STALAGMITE_ALONE: f64 = 0.03;
const DRIPSTONE_LONGEST: u64 = 8;

/// Скалк — где шум выше этого, рядом с ним узкая полоса жил.
const SCULK: f64 = -0.1;
const SCULK_VEIN: f64 = -0.22;

/// Датчик и крикун на полу из скалка.
const SCULK_SENSOR: f64 = 0.02;
const SCULK_SHRIEKER: f64 = 0.006;

/// Дерево с азалией — на одном месте из стольких, где под землёй пышные
/// пещеры. Корни ищут пещеру не глубже `ROOTS_REACH`, а не нашли — уходят
/// в землю на несколько блоков.
const AZALEA_TREE: f64 = 1.0 / 300.0;
const ROOTS_REACH: i32 = 48;
const ROOTS_SHORT: i32 = 4;

/// Состояния блоков, нужные проходу: ищутся по имени один раз.
struct Palette {
    water: i32,
    /// Камень, в который ложатся мох, капельник и скалк.
    rock: [i32; 8],

    moss: i32,
    moss_carpet: i32,
    short_grass: i32,
    tall_grass: [i32; 2],
    azalea: i32,
    flowering_azalea: i32,
    clay: i32,
    /// Малый капельный лист по сторонам света: нижняя половина в воде,
    /// верхняя на воздухе.
    small_dripleaf: [[i32; 2]; 4],
    /// Стебель большого капельного листа: сухой и в воде.
    big_dripleaf_stem: [[i32; 2]; 4],
    big_dripleaf: [[i32; 2]; 4],
    /// Кончик лозы (возраст, ягоды) и её тело (ягоды).
    cave_vines: [[i32; 2]; 26],
    cave_vines_plant: [i32; 2],
    spore_blossom: i32,
    rooted_dirt: i32,
    hanging_roots: [i32; 2],

    dripstone: i32,
    /// Острый капельник: [вверх, вниз] × [base, middle, frustum, tip,
    /// tip_merge].
    pointed: [[i32; 5]; 2],

    sculk: i32,
    /// Жила скалка по маске граней: бит 0 — низ, дальше верх, север, юг,
    /// запад, восток.
    sculk_vein: [i32; 64],
    sculk_sensor: i32,
    sculk_shrieker: i32,
}

/// Грани жилы скалка в порядке битов маски и шаги к соседу по ним.
const FACES: [&str; 6] = ["down", "up", "north", "south", "west", "east"];
const FACE_STEPS: [(i32, i32, i32); 6] = [
    (0, -1, 0),
    (0, 1, 0),
    (0, 0, -1),
    (0, 0, 1),
    (-1, 0, 0),
    (1, 0, 0),
];

/// Стороны света для листьев.
const FACINGS: [&str; 4] = ["north", "south", "west", "east"];

/// Толщина острого капельника от места крепления к кончику.
const BASE: usize = 0;
const MIDDLE: usize = 1;
const FRUSTUM: usize = 2;
const TIP: usize = 3;
const TIP_MERGE: usize = 4;

fn palette() -> &'static Palette {
    static PALETTE: OnceLock<Palette> = OnceLock::new();

    PALETTE.get_or_init(|| {
        let wet = |text: &str| [state(&format!("{},waterlogged=false]", text)), state(&format!("{},waterlogged=true]", text))];
        let by_facing = |make: &dyn Fn(&str) -> [i32; 2]| FACINGS.map(make);

        Palette {
            water: state("water"),
            rock: ["stone", "deepslate", "granite", "diorite", "andesite", "tuff", "dirt", "gravel"].map(state),

            moss: state("moss_block"),
            moss_carpet: state("moss_carpet"),
            short_grass: state("short_grass"),
            tall_grass: [state("tall_grass[half=lower]"), state("tall_grass[half=upper]")],
            azalea: state("azalea"),
            flowering_azalea: state("flowering_azalea"),
            clay: state("clay"),
            small_dripleaf: by_facing(&|facing| {
                [
                    state(&format!("small_dripleaf[facing={},half=lower,waterlogged=true]", facing)),
                    state(&format!("small_dripleaf[facing={},half=upper,waterlogged=false]", facing)),
                ]
            }),
            big_dripleaf_stem: by_facing(&|facing| wet(&format!("big_dripleaf_stem[facing={}", facing))),
            big_dripleaf: by_facing(&|facing| wet(&format!("big_dripleaf[facing={},tilt=none", facing))),
            cave_vines: std::array::from_fn(|age| {
                [false, true].map(|berries| state(&format!("cave_vines[age={},berries={}]", age, berries)))
            }),
            cave_vines_plant: [false, true].map(|berries| state(&format!("cave_vines_plant[berries={}]", berries))),
            spore_blossom: state("spore_blossom"),
            rooted_dirt: state("rooted_dirt"),
            hanging_roots: wet("hanging_roots["),

            dripstone: state("dripstone_block"),
            pointed: ["up", "down"].map(|direction| {
                ["base", "middle", "frustum", "tip", "tip_merge"].map(|thickness| {
                    state(&format!(
                        "pointed_dripstone[thickness={},vertical_direction={},waterlogged=false]",
                        thickness, direction
                    ))
                })
            }),

            sculk: state("sculk"),
            sculk_vein: std::array::from_fn(|mask| {
                let faces: Vec<String> = FACES
                    .iter()
                    .enumerate()
                    .map(|(bit, face)| format!("{}={}", face, mask >> bit & 1 == 1))
                    .collect();

                state(&format!("sculk_vein[{},waterlogged=false]", faces.join(",")))
            }),
            sculk_sensor: state("sculk_sensor[power=0,sculk_sensor_phase=inactive,waterlogged=false]"),
            // Природный крикун зовёт хранителя и вне древнего города (вики,
            // Sculk Shrieker: «naturally generated»).
            sculk_shrieker: state("sculk_shrieker[can_summon=true,shrieking=false,waterlogged=false]"),
        }
    })
}

/// Состояние по записи; блока нет в таблице — это ошибка в коде, а не мира.
/// Пустые скобки (`hanging_roots[,waterlogged=…]`) допускаются.
fn state(text: &str) -> i32 {
    let text = text.replace("[,", "[");

    crate::blocks::state_from_text(&text).unwrap_or_else(|| panic!("блока {} нет в таблице", text))
}

impl Palette {
    /// Камень, который можно сменить мхом, капельником или скалком.
    fn rock(&self, state: i32) -> bool {
        self.rock.contains(&state)
    }

    /// Пустота пещеры: воздух или вода.
    fn open(&self, state: i32) -> bool {
        state == AIR || state == self.water
    }
}

/// Шумы пятен — от числа мира.
struct Patches {
    moss: Noise,
    pool: Noise,
    dripstone: Noise,
    sculk: Noise,
}

impl Patches {
    fn new(key: u64) -> Patches {
        let noise = |salt: u64| Noise::new(scramble(key ^ salt) as i64);

        Patches {
            moss: noise(SALT_NOISE_MOSS),
            pool: noise(SALT_NOISE_POOL),
            dripstone: noise(SALT_NOISE_DRIPSTONE),
            sculk: noise(SALT_NOISE_SCULK),
        }
    }
}

/// Значение шума пятен в блоке.
fn patch(noise: &Noise, scale: f64, (x, y, z): (i32, i32, i32)) -> f64 {
    noise.at3(
        x as f64 / scale + NOISE_OFFSET,
        y as f64 / scale + NOISE_OFFSET,
        z as f64 / scale + NOISE_OFFSET,
    )
}

/// Число мира для пещерных биомов: семя со своей солью.
fn world_key(terrain: &Terrain) -> u64 {
    scramble(terrain.seed() as u64 ^ 0x00CA_7EB1_03E5)
}

/// Доля от 0 до 1 для точки и вопроса.
fn roll(key: u64, (x, y, z): (i32, i32, i32), salt: u64) -> f64 {
    unit(hash(key, x, y, z, salt))
}

/// Проход по чанку: деревья с азалией над пышными пещерами, затем убранство
/// самих пещер. `columns` — столбцы чанка с каймой в блок, `grid` — его
/// сетка плотности: по ним считаются блоки соседей у граней.
pub(super) fn decorate(
    generator: &Generator,
    chunk: &mut Chunk,
    chunk_x: i32,
    chunk_z: i32,
    columns: &[Column],
    grid: Option<&ReliefGrid>,
) {
    let Generator::Normal(terrain) = generator else {
        return;
    };

    let key = world_key(terrain);
    let blocks = palette();

    grow_azaleas(terrain, key, chunk, chunk_x, chunk_z, blocks);

    let numbers = Numbers::new();

    // Пещерные клетки ищем по столбцам клеток 4×4: у каждого — самый
    // нижний и самый верхний пещерный слой.
    let mut spans = [None::<(usize, usize)>; 16];

    for (layer, row) in chunk.biomes.chunks(16).enumerate() {
        for (cell, biome) in row.iter().enumerate() {
            if numbers.cave(*biome).is_some() {
                let span = spans[cell].get_or_insert((layer, layer));

                span.1 = layer;
            }
        }
    }

    // Глубокая тьма у соседа тоже задевает наш край: чанк без пещерных
    // клеток ещё смотрит, нет ли её за гранями.
    let caves = spans.iter().any(Option::is_some);
    let rim = Rim::new(generator, chunk, (chunk_x, chunk_z), columns, grid, caves);

    if !caves && rim.sides.iter().all(Option::is_none) {
        return;
    }

    let area = Area {
        key,
        chunk_x,
        chunk_z,
        blocks,
        numbers,
        patches: Patches::new(key),
        rim,
    };

    for local_x in 0..CHUNK_SIZE {
        for local_z in 0..CHUNK_SIZE {
            let Some((low, high)) = spans[(local_z / 4 * 4 + local_x / 4) as usize] else {
                continue;
            };

            let from = MIN_Y + low as i32 * 4;
            let to = MIN_Y + high as i32 * 4 + 3;

            area.column(chunk, local_x, local_z, from, to);
        }
    }

    area.sculk_rim(chunk);
}

/// Всё, что нужно проходу по чанку.
struct Area {
    key: u64,
    chunk_x: i32,
    chunk_z: i32,
    blocks: &'static Palette,
    numbers: Numbers,
    patches: Patches,
    rim: Rim,
}

/// Пустота столбца от пола до потолка: `bottom` — первый пустой блок над
/// полом, `top` — последний под потолком, `water` — сколько снизу воды.
#[derive(Clone, Copy)]
struct Run {
    local_x: i32,
    local_z: i32,
    bottom: i32,
    top: i32,
    water: i32,
}

impl Run {
    /// Сколько блоков воздуха над водой.
    fn air(&self) -> i32 {
        self.top - self.bottom + 1 - self.water
    }
}

impl Area {
    /// Мировые координаты блока по местным.
    fn world(&self, local_x: i32, y: i32, local_z: i32) -> (i32, i32, i32) {
        (
            self.chunk_x * CHUNK_SIZE + local_x,
            y,
            self.chunk_z * CHUNK_SIZE + local_z,
        )
    }

    /// Пещерный биом клетки блока.
    fn cave(&self, chunk: &Chunk, local_x: i32, y: i32, local_z: i32) -> Option<Cave> {
        self.numbers.cave(chunk.biome(local_x, y, local_z))
    }

    /// Проходит столбец от `from` до `to`: каждую пустоту — от пола до
    /// потолка — убирает по биому её клеток.
    fn column(&self, chunk: &mut Chunk, local_x: i32, local_z: i32, from: i32, to: i32) {
        let blocks = self.blocks;
        let mut y = from.max(MIN_Y + 1);

        while y <= to {
            if !blocks.open(chunk.block(local_x, y, local_z)) {
                y += 1;
                continue;
            }

            let mut bottom = y;

            while bottom > MIN_Y && blocks.open(chunk.block(local_x, bottom - 1, local_z)) {
                bottom -= 1;
            }

            let mut top = y;

            while top + 1 < TOP && blocks.open(chunk.block(local_x, top + 1, local_z)) {
                top += 1;
            }

            let mut water = 0;

            while bottom + water <= top && chunk.block(local_x, bottom + water, local_z) == blocks.water {
                water += 1;
            }

            let run = Run {
                local_x,
                local_z,
                bottom,
                top,
                water,
            };

            self.run(chunk, run);
            y = top + 2;
        }
    }

    /// Убирает одну пустоту столбца.
    fn run(&self, chunk: &mut Chunk, run: Run) {
        let Run {
            local_x,
            local_z,
            bottom,
            top,
            ..
        } = run;

        // Пол и потолок — только настоящие: у дна и верха мира их нет.
        let floor = (bottom > MIN_Y).then(|| self.cave(chunk, local_x, bottom, local_z)).flatten();
        let ceiling = (top + 1 < TOP).then(|| self.cave(chunk, local_x, top, local_z)).flatten();

        match floor {
            Some(Cave::Lush) => self.lush_floor(chunk, run),
            Some(Cave::Dripstone) => self.dripstone_patch(chunk, local_x, bottom - 1, local_z),
            _ => {}
        }

        match ceiling {
            Some(Cave::Lush) => self.lush_ceiling(chunk, run),
            Some(Cave::Dripstone) => self.dripstone_patch(chunk, local_x, top + 1, local_z),
            _ => {}
        }

        // Острый капельник — там, где пустота капельниковая хоть сверху,
        // хоть снизу: сталактит и сталагмит решают вместе, встречаются ли.
        if floor == Some(Cave::Dripstone) || ceiling == Some(Cave::Dripstone) {
            self.pointed_dripstone(chunk, run, floor == Some(Cave::Dripstone), ceiling == Some(Cave::Dripstone));
        }

        // Глубокая тьма смотрит на стены, а не только на пол и потолок.
        if floor == Some(Cave::DeepDark)
            || ceiling == Some(Cave::DeepDark)
            || (bottom..=top)
                .step_by(4)
                .chain([top])
                .any(|y| self.cave(chunk, local_x, y, local_z) == Some(Cave::DeepDark))
        {
            self.sculk(chunk, run);
        }
    }

    // --- Пышные пещеры -----------------------------------------------------

    /// Пол пышной пещеры: мох с травой и азалиями, лужи с глиной, капельные
    /// листья в воде.
    fn lush_floor(&self, chunk: &mut Chunk, run: Run) {
        let blocks = self.blocks;
        let Run {
            local_x,
            local_z,
            bottom,
            water,
            ..
        } = run;
        let floor_y = bottom - 1;
        let floor = chunk.block(local_x, floor_y, local_z);

        if !crate::blocks::full_cube(floor) {
            return;
        }

        let place = self.world(local_x, floor_y, local_z);

        // Дно под водой выстлано глиной.
        if water > 0 {
            if blocks.rock(floor) {
                chunk.put_generated(local_x, floor_y, local_z, blocks.clay, true);
            }

            self.dripleaf(chunk, run);
            return;
        }

        if !blocks.rock(floor) {
            return;
        }

        let pool = patch(&self.patches.pool, POOL_SCALE, place);

        // Лужа: пол опускается на блок и заливается водой. Вода не должна
        // вытечь — со всех сторон и снизу у неё твёрдое. Соседей за краем
        // чанка не видно, поэтому у края лужи нет.
        if pool > POOL && self.holds_water(chunk, local_x, floor_y, local_z) {
            chunk.put_generated(local_x, floor_y, local_z, blocks.water, true);

            if blocks.rock(chunk.block(local_x, floor_y - 1, local_z)) {
                chunk.put_generated(local_x, floor_y - 1, local_z, blocks.clay, true);
            }

            let pooled = Run {
                bottom: floor_y,
                water: 1,
                ..run
            };

            self.dripleaf(chunk, pooled);
            return;
        }

        if pool > POOL_RIM {
            chunk.put_generated(local_x, floor_y, local_z, blocks.clay, true);
            return;
        }

        if patch(&self.patches.moss, MOSS_SCALE, place) <= MOSS_FLOOR {
            return;
        }

        chunk.put_generated(local_x, floor_y, local_z, blocks.moss, true);

        let mut pick = roll(self.key, place, SALT_FLOOR_PLANT);
        let mut next = |share: f64| {
            pick -= share;
            pick < 0.0
        };

        if next(MOSS_CARPET) {
            put_in_air(chunk, local_x, bottom, local_z, blocks.moss_carpet);
        } else if next(SHORT_GRASS) {
            put_in_air(chunk, local_x, bottom, local_z, blocks.short_grass);
        } else if next(TALL_GRASS) {
            if run.air() >= 2 {
                put_in_air(chunk, local_x, bottom, local_z, blocks.tall_grass[0]);
                put_in_air(chunk, local_x, bottom + 1, local_z, blocks.tall_grass[1]);
            }
        } else if next(AZALEA_BUSH) {
            put_in_air(chunk, local_x, bottom, local_z, blocks.azalea);
        } else if next(FLOWERING_AZALEA_BUSH) {
            put_in_air(chunk, local_x, bottom, local_z, blocks.flowering_azalea);
        }
    }

    /// Удержит ли лужа воду на этом месте пола: вокруг и снизу твёрдые
    /// блоки, а соседи внутри чанка.
    fn holds_water(&self, chunk: &Chunk, local_x: i32, y: i32, local_z: i32) -> bool {
        if !(1..CHUNK_SIZE - 1).contains(&local_x) || !(1..CHUNK_SIZE - 1).contains(&local_z) {
            return false;
        }

        [(1, 0, 0), (-1, 0, 0), (0, 0, 1), (0, 0, -1), (0, -1, 0)]
            .iter()
            .all(|(dx, dy, dz)| {
                let near = chunk.block(local_x + dx, y + dy, local_z + dz);

                near == self.blocks.water || crate::blocks::full_cube(near)
            })
    }

    /// Капельный лист в мелкой воде: малый — в воде глубиной в блок, лист
    /// над водой; большой — стебель из воды, лист на воздухе.
    fn dripleaf(&self, chunk: &mut Chunk, run: Run) {
        let blocks = self.blocks;
        let Run {
            local_x,
            local_z,
            bottom,
            water,
            ..
        } = run;

        if water > DRIPLEAF_WATER || run.air() < 2 {
            return;
        }

        let place = self.world(local_x, bottom, local_z);
        let facing = (hash(self.key, place.0, place.1, place.2, SALT_FACING) % 4) as usize;
        let pick = roll(self.key, place, SALT_DRIPLEAF);

        if water == 1 && pick < SMALL_DRIPLEAF {
            let [lower, upper] = blocks.small_dripleaf[facing];

            chunk.put_generated(local_x, bottom, local_z, lower, true);
            chunk.put_generated(local_x, bottom + 1, local_z, upper, true);
        } else if (SMALL_DRIPLEAF..SMALL_DRIPLEAF + BIG_DRIPLEAF).contains(&pick) {
            // Стебель — вся вода и иногда ещё блок над ней.
            let extra = (hash(self.key, place.0, place.1, place.2, SALT_LENGTH) % 2) as i32;
            let extra = extra.min(run.air() - 1);
            let head = bottom + water + extra;

            for y in bottom..head {
                let wet = (y < bottom + water) as usize;

                chunk.put_generated(local_x, y, local_z, blocks.big_dripleaf_stem[facing][wet], true);
            }

            chunk.put_generated(local_x, head, local_z, blocks.big_dripleaf[facing][0], true);
        }
    }

    /// Потолок пышной пещеры: мох, пещерные лозы с ягодами, споровые цветы.
    fn lush_ceiling(&self, chunk: &mut Chunk, run: Run) {
        let blocks = self.blocks;
        let Run {
            local_x,
            local_z,
            top,
            ..
        } = run;
        let ceiling_y = top + 1;
        let ceiling = chunk.block(local_x, ceiling_y, local_z);

        if !crate::blocks::full_cube(ceiling) {
            return;
        }

        let place = self.world(local_x, ceiling_y, local_z);

        if blocks.rock(ceiling) && patch(&self.patches.moss, MOSS_SCALE, place) > MOSS_CEILING {
            chunk.put_generated(local_x, ceiling_y, local_z, blocks.moss, true);
        }

        // Под потолком должен быть воздух: в воде ни лоза, ни цветок не
        // растут.
        if chunk.block(local_x, top, local_z) != AIR {
            return;
        }

        let pick = roll(self.key, place, SALT_CEILING_PLANT);

        if pick < SPORE_BLOSSOM {
            chunk.put_generated(local_x, top, local_z, blocks.spore_blossom, true);
            return;
        }

        if pick >= SPORE_BLOSSOM + CAVE_VINES {
            return;
        }

        // Лоза свисает до первого не воздуха и не длиннее своей длины.
        let length = 1 + (hash(self.key, place.0, place.1, place.2, SALT_LENGTH) % CAVE_VINES_LONGEST) as i32;
        let mut end = top;

        while end > (top - length + 1).max(MIN_Y + 1) && chunk.block(local_x, end - 1, local_z) == AIR {
            end -= 1;
        }

        for y in end..=top {
            let at = self.world(local_x, y, local_z);
            let berries = (roll(self.key, at, SALT_BERRIES) < BERRIES) as usize;
            let state = if y == end {
                let age = (hash(self.key, at.0, at.1, at.2, SALT_LENGTH) % 26) as usize;

                blocks.cave_vines[age][berries]
            } else {
                blocks.cave_vines_plant[berries]
            };

            chunk.put_generated(local_x, y, local_z, state, true);
        }
    }

    // --- Капельниковые пещеры -----------------------------------------------

    /// Пятно блоков капельника на полу или потолке.
    fn dripstone_patch(&self, chunk: &mut Chunk, local_x: i32, y: i32, local_z: i32) {
        let here = chunk.block(local_x, y, local_z);

        if self.blocks.rock(here)
            && patch(&self.patches.dripstone, DRIPSTONE_SCALE, self.world(local_x, y, local_z)) > DRIPSTONE_PATCH
        {
            chunk.put_generated(local_x, y, local_z, self.blocks.dripstone, true);
        }
    }

    /// Сталактит с потолка и сталагмит с пола одной пустоты. Если их
    /// кончики сходятся вплотную, оба становятся `tip_merge` — это
    /// капельниковая колонна.
    fn pointed_dripstone(&self, chunk: &mut Chunk, run: Run, on_floor: bool, on_ceiling: bool) {
        let blocks = self.blocks;
        let Run {
            local_x,
            local_z,
            bottom,
            top,
            water,
        } = run;

        // В воде острый капельник мы не ставим.
        if water > 0 {
            return;
        }

        let room = top - bottom + 1;
        let ceiling = self.world(local_x, top + 1, local_z);
        let floor = self.world(local_x, bottom - 1, local_z);
        // Место крепления: капельник, а камень под ним становится капельником.
        let holds = |chunk: &Chunk, y: i32| {
            let here = chunk.block(local_x, y, local_z);

            here == blocks.dripstone || blocks.rock(here)
        };

        let length = |place: (i32, i32, i32)| {
            // Два броска вместо одного: короткие чаще длинных.
            let value = hash(self.key, place.0, place.1, place.2, SALT_LENGTH);
            let first = value % DRIPSTONE_LONGEST;
            let second = (value >> 16) % DRIPSTONE_LONGEST;

            1 + first.min(second) as i32
        };

        let mut down = 0;

        if on_ceiling && holds(chunk, top + 1) && roll(self.key, ceiling, SALT_STALACTITE) < STALACTITE {
            down = length(ceiling).min(room);
        }

        let mut up = 0;
        let alone = if down > 0 { STALAGMITE_UNDER } else { STALAGMITE_ALONE };

        if on_floor && holds(chunk, bottom - 1) && roll(self.key, floor, SALT_STALAGMITE) < alone {
            up = length(floor).min(room - down);
        }

        let merged = down > 0 && up > 0 && down + up == room;

        if down > 0 {
            chunk.put_generated(local_x, top + 1, local_z, blocks.dripstone, true);

            for (step, thickness) in thicknesses(down, merged).enumerate() {
                chunk.put_generated(local_x, top - step as i32, local_z, blocks.pointed[1][thickness], true);
            }
        }

        if up > 0 {
            chunk.put_generated(local_x, bottom - 1, local_z, blocks.dripstone, true);

            for (step, thickness) in thicknesses(up, merged).enumerate() {
                chunk.put_generated(local_x, bottom + step as i32, local_z, blocks.pointed[0][thickness], true);
            }
        }
    }

    // --- Глубокая тьма ---------------------------------------------------------

    /// Скалк на полу, стенах и потолке пустоты: блоки скалка пятнами, по краю
    /// пятен — жилы на соседних гранях, на полу из скалка — изредка датчик
    /// или крикун.
    fn sculk(&self, chunk: &mut Chunk, run: Run) {
        let blocks = self.blocks;
        let Run {
            local_x,
            local_z,
            bottom,
            top,
            ..
        } = run;

        for y in bottom..=top {
            if chunk.block(local_x, y, local_z) != AIR
                || self.cave(chunk, local_x, y, local_z) != Some(Cave::DeepDark)
            {
                continue;
            }

            let mut faces = 0usize;

            for (bit, (dx, dy, dz)) in FACE_STEPS.iter().enumerate() {
                let (nx, ny, nz) = (local_x + dx, y + dy, local_z + dz);

                if !(MIN_Y..TOP).contains(&ny) {
                    continue;
                }

                // Блок соседнего чанка: скалком его сделает сам сосед — ему
                // видна наша пустота. Здесь — только грань жилы на его камне.
                if !(0..CHUNK_SIZE).contains(&nx) || !(0..CHUNK_SIZE).contains(&nz) {
                    if self.rim.beyond(nx, ny, nz) == Beyond::Rock {
                        let value = patch(&self.patches.sculk, SCULK_SCALE, self.world(nx, ny, nz));

                        if value > SCULK_VEIN && value <= SCULK {
                            faces |= 1 << bit;
                        }
                    }

                    continue;
                }

                let near = chunk.block(nx, ny, nz);

                if near != blocks.sculk && !blocks.rock(near) {
                    continue;
                }

                let value = patch(&self.patches.sculk, SCULK_SCALE, self.world(nx, ny, nz));

                if value > SCULK {
                    chunk.put_generated(nx, ny, nz, blocks.sculk, true);
                } else if value > SCULK_VEIN {
                    faces |= 1 << bit;
                }
            }

            if y > MIN_Y && chunk.block(local_x, y - 1, local_z) == blocks.sculk {
                let pick = roll(self.key, self.world(local_x, y, local_z), SALT_SCULK_BLOCK);

                if pick < SCULK_SHRIEKER {
                    chunk.put_generated(local_x, y, local_z, blocks.sculk_shrieker, true);
                    continue;
                }

                if pick < SCULK_SHRIEKER + SCULK_SENSOR {
                    chunk.put_generated(local_x, y, local_z, blocks.sculk_sensor, true);
                    continue;
                }
            }

            if faces != 0 {
                chunk.put_generated(local_x, y, local_z, blocks.sculk_vein[faces], true);
            }
        }
    }

    /// Свои крайние блоки у пустот глубокой тьмы соседа: камень становится
    /// скалком, если здесь пятно, — как от пустоты своего чанка. Жилы на
    /// воздухе соседа ставит сам сосед.
    fn sculk_rim(&self, chunk: &mut Chunk) {
        let blocks = self.blocks;

        for (side, strip) in self.rim.sides.iter().enumerate() {
            let Some(strip) = strip else {
                continue;
            };

            for along in 0..CHUNK_SIZE {
                let (_, (local_x, local_z)) = across(side, along);

                for y in strip.from..=strip.to {
                    if strip.at(along, y) != Beyond::Dark || !blocks.rock(chunk.block(local_x, y, local_z)) {
                        continue;
                    }

                    if patch(&self.patches.sculk, SCULK_SCALE, self.world(local_x, y, local_z)) > SCULK {
                        chunk.put_generated(local_x, y, local_z, blocks.sculk, true);
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Кайма у граней чанка
// ---------------------------------------------------------------------------

/// Каким скалку виден блок соседнего чанка за гранью.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Beyond {
    /// Ни камень, ни пустота глубокой тьмы.
    Other,
    /// Камень, в который ложится скалк.
    Rock,
    /// Воздух в клетке глубокой тьмы.
    Dark,
}

/// Блоки соседей в полосу шириной в блок за гранями чанка — только на
/// высотах, где у грани с той или другой стороны глубокая тьма. Считаются
/// по неизменяемым снимкам подземелья, включая руду, жеоды и комнаты.
struct Rim {
    /// Запад, восток, север, юг.
    sides: [Option<RimSide>; 4],
}

/// Полоса за одной гранью: блоки от `from` до `to` по высоте в каждом из 16
/// столбцов вдоль грани.
struct RimSide {
    from: i32,
    to: i32,
    blocks: Vec<Beyond>,
}

impl RimSide {
    fn at(&self, along: i32, y: i32) -> Beyond {
        if !(self.from..=self.to).contains(&y) {
            return Beyond::Other;
        }

        self.blocks[(along * (self.to - self.from + 1) + y - self.from) as usize]
    }
}

/// Шаг наружу через грань: запад, восток, север, юг.
const OUTWARD: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

/// Местные координаты столбца за гранью `side` и своего крайнего столбца
/// напротив него — для места `along` вдоль грани.
fn across(side: usize, along: i32) -> ((i32, i32), (i32, i32)) {
    let edge = match side {
        0 => (0, along),
        1 => (CHUNK_SIZE - 1, along),
        2 => (along, 0),
        _ => (along, CHUNK_SIZE - 1),
    };
    let (dx, dz) = OUTWARD[side];

    ((edge.0 + dx, edge.1 + dz), edge)
}

/// Слои глубокой тьмы в столбце клеток — битами, нижний слой младшим.
type Layers = u128;

impl Rim {
    /// Кайма чанка. `mine` — есть ли у самого чанка пещерные клетки: без
    /// них своих слоёв глубокой тьмы у граней нет, смотрим только соседей.
    fn new(
        generator: &Generator,
        chunk: &Chunk,
        (chunk_x, chunk_z): (i32, i32),
        columns: &[Column],
        grid: Option<&ReliefGrid>,
        mine: bool,
    ) -> Rim {
        let Generator::Normal(terrain) = generator else { unreachable!() };
        let numbers = Numbers::new();
        let near = |(local_x, local_z): (i32, i32)| &columns[((local_x + 1) * (CHUNK_SIZE + 2) + local_z + 1) as usize];
        let world = |(local_x, local_z): (i32, i32)| (chunk_x * CHUNK_SIZE + local_x, chunk_z * CHUNK_SIZE + local_z);

        let sides = std::array::from_fn(|side| {
            // Слои глубокой тьмы по четырём клеткам вдоль грани: у себя —
            // по готовым биомам чанка, у соседа — по середине его клетки,
            // так же, как он решит их сам. Середину считаем, только если
            // эрозия рядом вообще пускает глубокую тьму.
            let mut ours: [Layers; 4] = [0; 4];
            let mut theirs: [Layers; 4] = [0; 4];

            for cell in 0..4 {
                let (outside, edge) = across(side, cell * 4 + 2);

                if mine {
                    for layer in 0..WORLD_HEIGHT / 4 {
                        if numbers.cave(chunk.biome(edge.0, MIN_Y + layer * 4, edge.1)) == Some(Cave::DeepDark) {
                            ours[cell as usize] |= 1 << layer;
                        }
                    }
                }

                if near(outside).climate.erosion >= DEEP_DARK_NEAR {
                    continue;
                }

                let (x, z) = world(outside);
                let middle = terrain.column_in((x & !3) + 2, (z & !3) + 2, grid);

                for layer in 0..WORLD_HEIGHT / 4 {
                    if biome_at(&middle, MIN_Y + layer * 4 + 2) == Some(Biome::DeepDark) {
                        theirs[cell as usize] |= 1 << layer;
                    }
                }
            }

            let all = ours.iter().chain(&theirs).fold(0, |all, layers| all | layers);

            if all == 0 {
                return None;
            }

            let from = MIN_Y + all.trailing_zeros() as i32 * 4;
            let to = MIN_Y + (Layers::BITS - 1 - all.leading_zeros()) as i32 * 4 + 3;
            // Сосед должен видеть те же ходы, жеоды и комнаты: один
            // шум рельефа уже не описывает его пустоты. Снимок не зависит
            // от того, загружен ли сосед, и не запускает декоратор повторно.
            let (dx,dz) = OUTWARD[side];
            let (cx,cz) = (chunk_x+dx,chunk_z+dz);
            let mut snapshot = (*super::underground::base_chunk(generator,cx,cz)).clone();
            super::underground::build_dungeons(generator,&mut snapshot,cx,cz);
            let mut blocks = Vec::with_capacity((CHUNK_SIZE * (to - from + 1)) as usize);

            for along in 0..CHUNK_SIZE {
                let (outside, _) = across(side, along);
                let dark = theirs[(along / 4) as usize];

                rim_column(&snapshot, world(outside), (from, to), dark, &mut blocks);
            }

            Some(RimSide { from, to, blocks })
        });

        Rim { sides }
    }

    /// Блок соседа за гранью по местным координатам (одна из них — −1 или
    /// 16).
    fn beyond(&self, local_x: i32, y: i32, local_z: i32) -> Beyond {
        let (side, along) = match (local_x, local_z) {
            (-1, _) => (0, local_z),
            (CHUNK_SIZE, _) => (1, local_z),
            (_, -1) => (2, local_x),
            _ => (3, local_x),
        };

        self.sides[side].as_ref().map_or(Beyond::Other, |strip| strip.at(along, y))
    }
}

/// Один столбец каймы из того же снимка, который получит сосед.
fn rim_column(chunk: &Chunk, (x,z): (i32,i32), (from,to): (i32,i32), dark: Layers, out: &mut Vec<Beyond>) {
    let blocks = palette();
    for y in from..=to {
        let state = chunk.block(x.rem_euclid(CHUNK_SIZE),y,z.rem_euclid(CHUNK_SIZE));
        out.push(if state == AIR && dark >> ((y-MIN_Y)/4) & 1 == 1 {
            Beyond::Dark
        } else if blocks.rock(state) {
            Beyond::Rock
        } else {
            Beyond::Other
        });
    }
}

/// Толщина острого капельника длиной `length` от места крепления к кончику
/// (вики, «Pointed Dripstone»): у крепления base, у кончика tip, перед ним
/// frustum, между ними middle. Короткий: в один блок — только кончик, в два
/// — frustum и кончик.
fn thicknesses(length: i32, merged: bool) -> impl Iterator<Item = usize> {
    (0..length).map(move |step| {
        let from_tip = length - 1 - step;

        match from_tip {
            0 if merged => TIP_MERGE,
            0 => TIP,
            1 => FRUSTUM,
            _ if step == 0 => BASE,
            _ => MIDDLE,
        }
    })
}

/// Ставит растение, только если место свободно.
fn put_in_air(chunk: &mut Chunk, local_x: i32, y: i32, local_z: i32, state: i32) {
    if (MIN_Y..TOP).contains(&y) && chunk.block(local_x, y, local_z) == AIR {
        chunk.put_generated(local_x, y, local_z, state, true);
    }
}

// ---------------------------------------------------------------------------
// Дерево с азалией
// ---------------------------------------------------------------------------

/// Растёт ли на этой земле дерево с азалией: на траве и земле, не в
/// пустыне, не на снегу гор и не на берегу.
fn grassy(biome: Biome) -> bool {
    !biome.is_ocean()
        && !matches!(
            biome,
            Biome::Desert
                | Biome::Badlands
                | Biome::ErodedBadlands
                | Biome::WoodedBadlands
                | Biome::Beach
                | Biome::SnowyBeach
                | Biome::StonyShore
                | Biome::River
                | Biome::FrozenRiver
                | Biome::Swamp
                | Biome::MangroveSwamp
                | Biome::MushroomFields
                | Biome::IceSpikes
                | Biome::SnowySlopes
                | Biome::JaggedPeaks
                | Biome::FrozenPeaks
                | Biome::StonyPeaks
        )
}

/// Где стоит дерево с азалией: место с редким числом, под ним пышные
/// пещеры, наверху — суша с травой. Решается только по месту и миру, так что
/// соседние чанки видят одно и то же дерево.
fn azalea_at(terrain: &Terrain, key: u64, x: i32, z: i32) -> Option<Column> {
    if roll(key, (x, 0, z), SALT_AZALEA) >= AZALEA_TREE {
        return None;
    }

    if !lush_below(&terrain.climate_at(x, z)) {
        return None;
    }

    let column = terrain.column_at(x, z);

    if column.height < SEA || !grassy(column.biome) {
        return None;
    }

    // Обычное дерево на этом месте важнее.
    if terrain::tree_possible(terrain.seed(), x, z) && terrain.tree_at(&column, x, z).is_some() {
        return None;
    }

    // Как и обычное дерево — только на земле, не на камне склона или
    // голой вершины.
    if !terrain.tree_ground(&column, x, z) {
        return None;
    }

    Some(column)
}

/// Деревья с азалией, которых касается чанк, и корни под теми, что стоят
/// в нём самом.
fn grow_azaleas(terrain: &Terrain, key: u64, chunk: &mut Chunk, chunk_x: i32, chunk_z: i32, blocks: &Palette) {
    const REACH: i32 = terrain::TREE_REACH;

    let mut ground = None;

    for local_x in -REACH..CHUNK_SIZE + REACH {
        for local_z in -REACH..CHUNK_SIZE + REACH {
            let (x, z) = (chunk_x * CHUNK_SIZE + local_x, chunk_z * CHUNK_SIZE + local_z);

            let Some(column) = azalea_at(terrain, key, x, z) else {
                continue;
            };

            let tree = trees::azalea_at(terrain.seed(), x, z);
            let ground = ground.get_or_insert_with(|| super::TreeGround {
                seed: if terrain.preview() { terrain.seed() } else { 0 },
                terrain: terrain.preview().then_some(terrain),
                ..super::TreeGround::new()
            });

            super::draw_tree(chunk, &tree, local_x, local_z, column.height, x, z, ground);

            if (0..CHUNK_SIZE).contains(&local_x) && (0..CHUNK_SIZE).contains(&local_z) {
                grow_roots(key, chunk, blocks, local_x, local_z, (x, column.height, z));
            }
        }
    }
}

/// Корни дерева с азалией: корнистая земля от земли под стволом вниз до
/// пещеры, в пещере под ней — свисающие корни. Не нашли пещеру поблизости —
/// корни уходят в землю на несколько блоков.
fn grow_roots(key: u64, chunk: &mut Chunk, blocks: &Palette, local_x: i32, local_z: i32, (x, height, z): (i32, i32, i32)) {
    let rootable = |state: i32| blocks.rock(state) || state == blocks.rooted_dirt || crate::blocks::block_at_state(state) == Some("grass_block");
    let lowest = (height - ROOTS_REACH).max(MIN_Y + 1);
    let mut cave = None;

    for y in (lowest..height).rev() {
        let here = chunk.block(local_x, y, local_z);

        if blocks.open(here) {
            cave = Some(y);
            break;
        }

        if !rootable(here) {
            break;
        }
    }

    let end = match cave {
        Some(y) => y + 1,
        None => height - ROOTS_SHORT + (hash(key, x, height, z, SALT_ROOTS) % 2) as i32,
    };

    for y in end..height {
        if rootable(chunk.block(local_x, y, local_z)) {
            chunk.put_generated(local_x, y, local_z, blocks.rooted_dirt, true);
        }
    }

    if let Some(y) = cave {
        let wet = (chunk.block(local_x, y, local_z) == blocks.water) as usize;

        chunk.put_generated(local_x, y, local_z, blocks.hanging_roots[wet], true);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::terrain::Style;

    const SEED: i64 = 100_554_032_945_340;

    fn climate(humidity: f64, continent: f64, erosion: f64) -> Climate {
        Climate {
            continent,
            erosion,
            weirdness: 0.0,
            temperature: 0.0,
            humidity,
        }
    }

    /// Выбор по таблице вики: влажно — пышные, далеко от моря —
    /// капельниковые, глубоко под горами — глубокая тьма; у поверхности —
    /// всегда наземный.
    #[test]
    fn caves_follow_the_table() {
        let plain = climate(0.0, 0.2, 0.2);

        assert_eq!(cave_biome(&climate(0.8, 0.2, 0.2), 0.5), Some(Biome::LushCaves));
        assert_eq!(cave_biome(&climate(0.0, 1.0, 0.2), 0.5), Some(Biome::DripstoneCaves));
        assert_eq!(cave_biome(&climate(0.0, 0.5, 0.2), 0.5), None);
        assert_eq!(cave_biome(&climate(0.0, 0.2, -0.6), 1.2), Some(Biome::DeepDark));
        assert_eq!(cave_biome(&plain, 0.5), None);
        assert_eq!(cave_biome(&climate(0.0, 0.2, -0.6), 0.5), None);

        for depth in [0.0, 0.05, NO_CAVES_ABOVE] {
            assert_eq!(cave_biome(&climate(1.0, 1.0, -1.0), depth), None, "глубина {}", depth);
        }

        // Океан: ни капельника, ни тьмы.
        assert_ne!(cave_biome(&climate(0.0, -0.5, -0.8), 0.5), Some(Biome::DripstoneCaves));
    }

    /// Толщина капельника от крепления к кончику.
    #[test]
    fn dripstone_thins_to_its_tip() {
        let of = |length, merged| thicknesses(length, merged).collect::<Vec<_>>();

        assert_eq!(of(1, false), vec![TIP]);
        assert_eq!(of(2, false), vec![FRUSTUM, TIP]);
        assert_eq!(of(3, false), vec![BASE, FRUSTUM, TIP]);
        assert_eq!(of(5, true), vec![BASE, MIDDLE, MIDDLE, FRUSTUM, TIP_MERGE]);
    }

    /// Все состояния, которые ставит проход, есть в таблице блоков.
    #[test]
    fn every_state_exists() {
        let blocks = palette();

        assert_eq!(crate::blocks::block_at_state(blocks.sculk_shrieker), Some("sculk_shrieker"));
        assert_eq!(
            crate::blocks::orientation_of(blocks.sculk_shrieker).and_then(|rule| rule.value(blocks.sculk_shrieker, "can_summon")),
            Some("true")
        );
        assert_eq!(
            crate::blocks::orientation_of(blocks.pointed[1][TIP]).and_then(|rule| rule.value(blocks.pointed[1][TIP], "vertical_direction")),
            Some("down")
        );
    }

    /// Чанки, под которыми есть пещерные биомы: по нескольку на каждый.
    fn cave_chunks(terrain: &Terrain, each: usize) -> Vec<(Biome, i32, i32)> {
        let mut found: Vec<(Biome, i32, i32)> = Vec::new();

        for i in 0..4096 {
            let (x, z) = ((i % 64) * 64 + 2, (i / 64) * 64 + 2);
            let column = terrain.column_at(x, z);

            for y in (MIN_Y..column.height).step_by(4) {
                let Some(biome) = biome_at(&column, y + 2) else {
                    continue;
                };

                if found.iter().filter(|(kind, _, _)| *kind == biome).count() < each
                    && !found.iter().any(|(_, cx, cz)| (*cx, *cz) == (x >> 4, z >> 4))
                {
                    found.push((biome, x >> 4, z >> 4));
                }
            }
        }

        found
    }

    /// Пещерные биомы лежат только в толще: клетки у поверхности и выше
    /// остаются биомом столбца, как было до объёмных биомов, а пещерная
    /// клетка всегда глубже поверхности.
    #[test]
    fn cave_biomes_stay_underground() {
        let generator = Generator::Normal(Terrain::new(SEED, Style::Vanilla));
        let Generator::Normal(terrain) = &generator else {
            unreachable!()
        };
        let places = cave_chunks(terrain, 2);
        let numbers = Numbers::new();
        let mut caves = [0; 3];

        for kind in [Biome::LushCaves, Biome::DripstoneCaves, Biome::DeepDark] {
            assert!(places.iter().any(|(biome, _, _)| *biome == kind), "нет {:?}", kind);
        }

        for (_, chunk_x, chunk_z) in places {
            let chunk = Chunk::generated(&generator, chunk_x, chunk_z);

            for cell_x in 0..4 {
                for cell_z in 0..4 {
                    let middle =
                        terrain.column_at(chunk_x * 16 + cell_x * 4 + 2, chunk_z * 16 + cell_z * 4 + 2);
                    let surface = middle.biome.number() as u16;

                    for layer in 0..96 {
                        let y = MIN_Y + layer * 4 + 2;
                        let biome = chunk.biomes[layer as usize * 16 + cell_z as usize * 4 + cell_x as usize];

                        match numbers.cave(biome) {
                            Some(cave) => {
                                caves[cave as usize] += 1;
                                assert!(
                                    y < middle.height - 12,
                                    "пещерный биом на Y {} при поверхности {}",
                                    y,
                                    middle.height
                                );
                            }
                            None => assert_eq!(biome, surface, "клетка на Y {} сменила биом", y),
                        }
                    }
                }
            }
        }

        assert!(caves.iter().all(|count| *count > 0), "клеток по биомам: {:?}", caves);
    }

    /// Один и тот же мир — одни и те же пещеры до блока; другой мир —
    /// другие.
    #[test]
    fn caves_repeat_with_the_seed() {
        let generator = Generator::Normal(Terrain::new(SEED, Style::Vanilla));
        let Generator::Normal(terrain) = &generator else {
            unreachable!()
        };

        for (_, chunk_x, chunk_z) in cave_chunks(terrain, 1) {
            let one = Chunk::generated(&generator, chunk_x, chunk_z);
            let two = Chunk::generated(&generator, chunk_x, chunk_z);

            assert_eq!(one.biomes, two.biomes);
            assert!(one.sections == two.sections, "чанк {} {} сложился иначе", chunk_x, chunk_z);
        }

        let other = Terrain::new(SEED + 1, Style::Vanilla);

        assert_ne!(world_key(terrain), world_key(&other));
    }

    /// В каждом пещерном биоме ставится своё убранство, и только в нём.
    #[test]
    fn each_cave_gets_its_decor() {
        let generator = Generator::Normal(Terrain::new(SEED, Style::Vanilla));
        let Generator::Normal(terrain) = &generator else {
            unreachable!()
        };
        let blocks = palette();
        let numbers = Numbers::new();
        let mut seen = [[false; 3]; 3];

        for (_, chunk_x, chunk_z) in cave_chunks(terrain, 3) {
            let chunk = Chunk::generated(&generator, chunk_x, chunk_z);

            for local_x in 0..16 {
                for local_z in 0..16 {
                    for y in MIN_Y..TOP {
                        let state = chunk.block(local_x, y, local_z);
                        let name = crate::blocks::block_at_state(state).unwrap_or("?");
                        let kind = match name {
                            "moss_block" | "cave_vines" | "big_dripleaf" | "small_dripleaf" => 0,
                            "pointed_dripstone" => 1,
                            "sculk" | "sculk_vein" => 2,
                            _ => continue,
                        };

                        // Мох бывает и в данжах, и под деревом с азалией —
                        // смотрим только блоки пещерных клеток.
                        let Some(cave) = numbers.cave(chunk.biome(local_x, y, local_z)) else {
                            continue;
                        };

                        seen[kind][cave as usize] = true;

                        if name == "pointed_dripstone" {
                            // Капельник держится за блок капельника.
                            let down = state == blocks.pointed[1][TIP] || state == blocks.pointed[1][TIP_MERGE];

                            if down && y + 1 < TOP {
                                let above = chunk.block(local_x, y + 1, local_z);

                                assert!(
                                    above == blocks.dripstone
                                        || crate::blocks::block_at_state(above) == Some("pointed_dripstone"),
                                    "сталактит висит на {:?}",
                                    crate::blocks::block_at_state(above)
                                );
                            }
                        }
                    }
                }
            }
        }

        assert!(seen[0][Cave::Lush as usize], "в пышных пещерах нет мха и лоз");
        assert!(seen[1][Cave::Dripstone as usize], "в капельниковых нет капельника");
        assert!(seen[2][Cave::DeepDark as usize], "в глубокой тьме нет скалка");
        assert!(!seen[0][Cave::DeepDark as usize] && !seen[2][Cave::Lush as usize]);
    }

    /// Скалк не видит границы чанков: камень рядом с пустотой глубокой тьмы
    /// становится скалком, а на воздух ложится жила — так же, как внутри
    /// чанка, по какую бы сторону границы ни лежала пустота.
    #[test]
    fn sculk_crosses_the_border() {
        let generator = Generator::Normal(Terrain::new(SEED, Style::Vanilla));
        let Generator::Normal(terrain) = &generator else {
            unreachable!()
        };
        let blocks = palette();
        let numbers = Numbers::new();
        let patches = Patches::new(world_key(terrain));
        let deep = |x: i32, z: i32| {
            let column = terrain.column_at(x, z);

            (MIN_Y..column.height).step_by(4).any(|y| biome_at(&column, y + 2) == Some(Biome::DeepDark))
        };

        let mut sculk = 0;
        let mut veins = 0;
        let mut wrong = Vec::new();

        // Границы по x и по z, у которых глубокая тьма по обе стороны: в
        // крайней клетке одного чанка и в первой клетке следующего. Пещера на
        // самой границе бывает не у каждой — берём первые несколько, где
        // есть что проверить. Места вдоль границы — (поперёк, y, вдоль).
        for axis in 0..2 {
            let flip = |(across, along): (i32, i32)| if axis == 0 { (across, along) } else { (along, across) };
            let borders = (0..4096).map(|i| ((i % 64) * 4, (i / 64) * 4)).filter(|chunk| {
                let (across, along) = flip(*chunk);
                let (x, z) = flip((across * CHUNK_SIZE + 14, along * CHUNK_SIZE + 6));
                let (far_x, far_z) = flip((across * CHUNK_SIZE + 18, along * CHUNK_SIZE + 6));

                deep(x, z) && deep(far_x, far_z)
            });
            let mut checked = 0;

            for chunk in borders {
                if checked == 6 {
                    break;
                }

                let (step_x, step_z) = flip((1, 0));
                let starts = [chunk, (chunk.0 + step_x, chunk.1 + step_z)];
                let sides = starts.map(|(chunk_x, chunk_z)| Chunk::generated(&generator, chunk_x, chunk_z));
                let before = sculk;

                for along in 0..CHUNK_SIZE {
                    for y in MIN_Y..TOP {
                        // Камень P с одной стороны, пустота Q вплотную с другой.
                        for (from, to) in [(0, 1), (1, 0)] {
                            let (p_across, q_across) = if from == 0 { (CHUNK_SIZE - 1, 0) } else { (0, CHUNK_SIZE - 1) };
                            let ((p_x, p_z), (q_x, q_z)) = (flip((p_across, along)), flip((q_across, along)));
                            let (p, q) = (sides[from].block(p_x, y, p_z), sides[to].block(q_x, y, q_z));
                            let dark = numbers.cave(sides[to].biome(q_x, y, q_z)) == Some(Cave::DeepDark);
                            let open = q == AIR
                                || blocks.sculk_vein.contains(&q)
                                || q == blocks.sculk_sensor
                                || q == blocks.sculk_shrieker;

                            if !dark || !open {
                                continue;
                            }

                            let (chunk_x, chunk_z) = starts[from];
                            let place = (chunk_x * CHUNK_SIZE + p_x, y, chunk_z * CHUNK_SIZE + p_z);
                            let value = patch(&patches.sculk, SCULK_SCALE, place);

                            if value > SCULK && (blocks.rock(p) || p == blocks.sculk) {
                                sculk += 1;

                                if p != blocks.sculk {
                                    wrong.push(("камень не стал скалком", place));
                                }
                            }

                            // Грань жилы, смотрящая на P: запад или север,
                            // если P по эту сторону, иначе восток или юг.
                            let face = 1 << (2 + 2 * (1 - axis) + (from == 1) as i32);

                            if blocks.rock(p)
                                && value > SCULK_VEIN
                                && value <= SCULK
                                && q != blocks.sculk_sensor
                                && q != blocks.sculk_shrieker
                            {
                                veins += 1;

                                if !blocks.sculk_vein.iter().enumerate().any(|(mask, state)| *state == q && mask & face != 0) {
                                    wrong.push(("нет жилы на грани", place));
                                }
                            }
                        }
                    }
                }

                checked += (sculk > before) as usize;
            }

            assert!(checked > 0, "нет границы глубокой тьмы поперёк оси {}", axis);
        }

        assert!(sculk > 0 && veins > 0, "на границах глубокой тьмы нечего проверять");
        assert!(
            wrong.is_empty(),
            "{} из {} мест скалка и {} жил на границе решены иначе: {:?}",
            wrong.len(),
            sculk,
            veins,
            &wrong[..wrong.len().min(8)]
        );
    }

    /// Дерево с азалией у края чанка рисуют оба чанка, каждый свою часть,
    /// не сговариваясь; корни уходят вниз под стволом.
    #[test]
    fn an_azalea_is_shared_across_the_border() {
        let generator = Generator::Normal(Terrain::new(SEED, Style::Vanilla));
        let Generator::Normal(terrain) = &generator else {
            unreachable!()
        };
        let key = world_key(terrain);
        let leaves = ["azalea_leaves", "flowering_azalea_leaves"];

        let (x, z, column) = (0..4096 * 64)
            .map(|i| ((i % 2048) * 16 + 15, i / 2048))
            .find_map(|(x, z)| azalea_at(terrain, key, x, z).map(|column| (x, z, column)))
            .expect("дерево с азалией у края чанка");

        let (chunk_x, chunk_z) = (x >> 4, z >> 4);
        let own = Chunk::generated(&generator, chunk_x, chunk_z);
        let east = Chunk::generated(&generator, chunk_x + 1, chunk_z);
        let (local_x, local_z) = (x & 15, z & 15);
        let has_leaves = |chunk: &Chunk, columns: std::ops::Range<i32>| {
            columns.into_iter().any(|lx| {
                (-3..=3).any(|dz| {
                    let lz = local_z + dz;

                    (0..16).contains(&lz)
                        && (column.height..column.height + 12).any(|y| {
                            leaves.contains(&crate::blocks::block_at_state(chunk.block(lx, y, lz)).unwrap_or("?"))
                        })
                })
            })
        };

        assert_eq!(
            crate::blocks::block_at_state(own.block(local_x, column.height + 1, local_z)),
            Some("oak_log")
        );
        assert_eq!(own.block(local_x, column.height - 1, local_z), palette().rooted_dirt);
        assert!(has_leaves(&own, 12..16), "у своего чанка нет листвы");
        assert!(has_leaves(&east, 0..4), "соседний чанк не дорисовал крону");
    }

    /// Перепись: сколько клеток под землёй досталось каждому пещерному
    /// биому и сколько поставлено украшений. Запускается вручную:
    /// `cargo test --release cave_census -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn cave_census() {
        use std::collections::HashMap;

        let terrain = Terrain::new(SEED, Style::Vanilla);
        let mut cells: HashMap<&str, u64> = HashMap::new();
        let mut heights: HashMap<&str, (i32, i32, i64)> = HashMap::new();
        let mut columns = 0u64;
        let mut deep_columns = 0u64;

        for i in 0..4096 {
            let (x, z) = ((i % 64) * 64 + 2, (i / 64) * 64 + 2);
            let column = terrain.column_at(x, z);

            columns += 1;

            if column.climate.erosion < -0.375 {
                deep_columns += 1;
            }

            for layer in 0..96 {
                let y = MIN_Y + layer * 4 + 2;

                if y > column.height {
                    break;
                }

                let name = biome_at(&column, y).map_or("наземный", |biome| biome.name());

                *cells.entry(name).or_default() += 1;

                let range = heights.entry(name).or_insert((y, y, 0i64));

                range.0 = range.0.min(y);
                range.1 = range.1.max(y);
                range.2 += y as i64;
            }
        }

        let total: u64 = cells.values().sum();
        let mut rows: Vec<_> = cells.into_iter().collect();

        rows.sort_by_key(|(_, count)| std::cmp::Reverse(*count));

        println!("столбцов {}, с эрозией ниже −0,375: {}", columns, deep_columns);


        for (name, count) in rows {
            let (low, high, sum) = heights[name];

            println!(
                "  {:>6.2}%  {} (Y {}…{}, в среднем {})",
                100.0 * count as f64 / total as f64,
                name,
                low,
                high,
                sum / count as i64
            );
        }
    }

    /// Скалк у края чанка и в середине: сколько блоков скалка и жил
    /// приходится на столбец в крайних столбцах (x или z по модулю 16 — 0
    /// или 15) и в остальных, и сколько — на сотню пустот глубокой тьмы (у
    /// края их может быть больше или меньше: чанки выбраны по одной клетке).
    /// Шва нет — числа близки.
    /// `cargo test --release sculk_border_census -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn sculk_border_census() {
        let generator = Generator::Normal(Terrain::new(SEED, Style::Vanilla));
        let Generator::Normal(terrain) = &generator else {
            unreachable!()
        };
        let blocks = palette();
        let places: Vec<(i32, i32)> = cave_chunks(terrain, 64)
            .into_iter()
            .filter(|(biome, _, _)| *biome == Biome::DeepDark)
            .map(|(_, chunk_x, chunk_z)| (chunk_x, chunk_z))
            .collect();
        // [край, середина] × [скалк, жилы, пустоты глубокой тьмы, столбцы]
        let mut counts = [[0u64; 4]; 2];
        let numbers = Numbers::new();
        let mut spent = std::time::Duration::ZERO;

        for (chunk_x, chunk_z) in &places {
            let started = std::time::Instant::now();
            let chunk = Chunk::generated(&generator, *chunk_x, *chunk_z);

            spent += started.elapsed();

            for local_x in 0..CHUNK_SIZE {
                for local_z in 0..CHUNK_SIZE {
                    let edge = [0, CHUNK_SIZE - 1].contains(&local_x) || [0, CHUNK_SIZE - 1].contains(&local_z);
                    let row = &mut counts[if edge { 0 } else { 1 }];

                    row[3] += 1;

                    for y in MIN_Y..TOP {
                        let state = chunk.block(local_x, y, local_z);

                        if state == blocks.sculk {
                            row[0] += 1;
                        } else if blocks.sculk_vein.contains(&state) {
                            row[1] += 1;
                        }

                        if (state == AIR
                            || blocks.sculk_vein.contains(&state)
                            || state == blocks.sculk_sensor
                            || state == blocks.sculk_shrieker)
                            && numbers.cave(chunk.biome(local_x, y, local_z)) == Some(Cave::DeepDark)
                        {
                            row[2] += 1;
                        }
                    }
                }
            }
        }

        println!(
            "чанков с глубокой тьмой: {}, в среднем {:?} на чанк",
            places.len(),
            spent / places.len() as u32
        );

        for (name, [sculk, veins, dark, columns]) in [("край", counts[0]), ("середина", counts[1])] {
            println!(
                "  {}: на столбец скалка {:.3}, жил {:.3}, пустот {:.2}; на сотню пустот скалка {:.2}, жил {:.2} ({} столбцов)",
                name,
                sculk as f64 / columns as f64,
                veins as f64 / columns as f64,
                dark as f64 / columns as f64,
                100.0 * sculk as f64 / dark as f64,
                100.0 * veins as f64 / dark as f64,
                columns
            );
        }
    }

    /// Перепись украшений: в чанках над каждым пещерным биомом считает
    /// поставленные блоки. `cargo test --release cave_decor -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn cave_decor() {
        use std::collections::HashMap;

        let generator = Generator::Normal(Terrain::new(SEED, Style::Vanilla));
        let Generator::Normal(terrain) = &generator else {
            unreachable!()
        };
        let mut found: HashMap<&str, Vec<(i32, i32)>> = HashMap::new();

        for i in 0..4096 {
            let (x, z) = ((i % 64) * 64 + 2, (i / 64) * 64 + 2);
            let column = terrain.column_at(x, z);

            for y in (MIN_Y..column.height).step_by(4) {
                if let Some(biome) = biome_at(&column, y + 2) {
                    let list = found.entry(biome.name()).or_default();

                    if list.len() < 12 && !list.contains(&(x >> 4, z >> 4)) {
                        list.push((x >> 4, z >> 4));
                    }
                }
            }
        }

        let started = std::time::Instant::now();
        let mut chunks = 0;
        let mut pass = std::time::Duration::ZERO;

        for (biome, places) in &found {
            let mut counts: HashMap<&str, u64> = HashMap::new();

            for (chunk_x, chunk_z) in places {
                let mut chunk = Chunk::generated(&generator, *chunk_x, *chunk_z);
                let (from_x, from_z) = (chunk_x * CHUNK_SIZE - 1, chunk_z * CHUNK_SIZE - 1);
                let grid = terrain.relief_grid(from_x, from_z, from_x + CHUNK_SIZE + 1, from_z + CHUNK_SIZE + 1);
                let columns: Vec<Column> = (from_x..from_x + CHUNK_SIZE + 2)
                    .flat_map(|x| (from_z..from_z + CHUNK_SIZE + 2).map(move |z| (x, z)))
                    .map(|(x, z)| terrain.column_in(x, z, Some(&grid)))
                    .collect();
                let again = std::time::Instant::now();

                decorate(&generator, &mut chunk, *chunk_x, *chunk_z, &columns, Some(&grid));
                pass += again.elapsed();
                chunks += 1;

                for section in chunk.sections.iter().flatten() {
                    for state in section.iter() {
                        let name = crate::blocks::block_at_state(*state as i32).unwrap_or("?");

                        if matches!(
                            name,
                            "moss_block" | "moss_carpet" | "azalea" | "flowering_azalea" | "clay" | "small_dripleaf"
                                | "big_dripleaf" | "big_dripleaf_stem" | "cave_vines" | "cave_vines_plant" | "spore_blossom"
                                | "rooted_dirt" | "hanging_roots" | "azalea_leaves" | "flowering_azalea_leaves"
                                | "dripstone_block" | "pointed_dripstone" | "sculk" | "sculk_vein" | "sculk_sensor"
                                | "sculk_shrieker"
                        ) {
                            *counts.entry(name).or_default() += 1;
                        }
                    }
                }
            }

            let mut rows: Vec<_> = counts.into_iter().collect();

            rows.sort();
            println!("{} ({} чанков): {:?}", biome, places.len(), rows);
        }

        println!(
            "в среднем на чанк {:?}, из них проход пещерных биомов {:?}",
            (started.elapsed() - pass) / chunks,
            pass / chunks
        );
    }
}
