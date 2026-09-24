// Освещение: небесный и блочный свет.
//
// Свет в игре — два числа от 0 до 15 у каждого блока. Небесный приходит
// сверху: под открытым небом он полный и вниз по прозрачным блокам идёт не
// слабея, а в стороны и вверх теряет по единице на шаг. Блочный идёт от
// светящихся блоков (лава, факел, светокамень) и теряет по единице на шаг во
// все стороны. Блок, сквозь который свет проходит, отнимает ещё своё:
// листва и вода — единицу, камень — всё (minecraft.wiki, страница Light).
//
// Сколько блок светит и сколько гасит — по таблице LIGHT в blocks_table.rs,
// собранной tools/make_block_table.py. Там же — какие грани блока закрыты
// целиком: плита, ступени, грядка и тропинка пропускают свет не во все
// стороны (у нижней плиты свет не проходит сквозь её низ).
//
// Считается свет в два приёма, и оба — вне захвата мира:
//
// 1. Собственный свет чанка — только по его блокам, будто вокруг пустота,
//    в которую свет уходит без следа. Считается, когда чанк сложен или
//    прочитан с диска, и хранится в чанке. После правки блоков он помечается
//    устаревшим (номер правки не сходится) и пересчитывается при следующей
//    отправке чанка.
//
// 2. Сшивка: перед отправкой чанка свет его и восьми соседей сводится
//    вместе — от каждой границы чанков свет растекается туда, где его меньше.
//    Сшитый свет не хранится: соседи меняются, а сшивка дешёвая.

use std::sync::{Arc, OnceLock};

use super::{Packed, SECTIONS, SECTION_BLOCKS, WORLD_HEIGHT};
use crate::blocks_table::LIGHT;

/// Самый яркий свет.
pub const FULL: u8 = 15;

/// Байтов в массиве света секции: по половине байта на блок.
pub const NIBBLES: usize = SECTION_BLOCKS / 2;

/// Слоёв блоков по высоте мира.
const LAYERS: usize = WORLD_HEIGHT as usize;

/// Блоков в слое чанка 16×16.
const LAYER: usize = 256;

/// Секций в чанке — числом для индексов.
const SECTION_COUNT: usize = SECTIONS as usize;

// Свойства состояния блока для света сложены в одно число: сколько блок
// светит (биты 0–3), сколько гасит (биты 4–7) и какие грани закрыты целиком
// (биты 8–13, по одному на направление — см. `DIRECTIONS`).

fn emitted(props: u16) -> u8 {
    (props & 0xF) as u8
}

fn filtered(props: u16) -> u8 {
    ((props >> 4) & 0xF) as u8
}

fn closed(props: u16) -> u8 {
    (props >> 8) as u8
}

/// Свойства всех состояний, по номеру состояния.
///
/// Длина — все номера, какие помещаются в два байта: состояние в чанке
/// хранится именно так, и выйти за таблицу оно не может.
fn table() -> &'static [u16] {
    static TABLE: OnceLock<Vec<u16>> = OnceLock::new();

    TABLE.get_or_init(|| {
        let mut table = vec![0u16; 1 << 16];

        for &(low, high, emit, filter, faces) in LIGHT {
            let props = emit as u16 | (filter as u16) << 4 | (faces as u16) << 8;

            table[low as usize..=high as usize].fill(props);
        }

        table
    })
}

/// Сколько света испускает блок в этом состоянии.
#[cfg(test)]
pub fn emission(state: i32) -> u8 {
    emitted(table()[state as u16 as usize])
}

/// Сколько света гасит блок в этом состоянии: 0 — прозрачен, 15 — глух.
#[cfg(test)]
pub fn filter(state: i32) -> u8 {
    filtered(table()[state as u16 as usize])
}

/// Направление шага света: куда сдвинуться и какие грани на пути.
struct Direction {
    x: i32,
    y: i32,
    z: i32,

    /// Грань блока, через которую свет из него выходит.
    face: u8,

    /// Грань соседа, через которую свет в него входит.
    back: u8,

    /// Шаг вниз: только так небесный свет идёт, не слабея.
    down: bool,
}

/// Шесть направлений. Биты граней — как в таблице LIGHT: низ, верх, север,
/// юг, запад, восток. Север — к меньшему Z, запад — к меньшему X.
const DIRECTIONS: [Direction; 6] = [
    Direction { x: 0, y: -1, z: 0, face: 1, back: 2, down: true },
    Direction { x: 0, y: 1, z: 0, face: 2, back: 1, down: false },
    Direction { x: 0, y: 0, z: -1, face: 4, back: 8, down: false },
    Direction { x: 0, y: 0, z: 1, face: 8, back: 4, down: false },
    Direction { x: -1, y: 0, z: 0, face: 16, back: 32, down: false },
    Direction { x: 1, y: 0, z: 0, face: 32, back: 16, down: false },
];

/// Номера направлений в `DIRECTIONS`, нужные по имени. Противоположное
/// направление — соседний номер: `номер ^ 1`.
const DOWN: usize = 0;
const SOUTH: usize = 3;
const EAST: usize = 5;

/// Сколько света дойдёт из блока со светом `level` в соседа.
///
/// Свет теряет на шаге то, что гасит сосед, но не меньше единицы. Небесный
/// свет, идущий прямо вниз во всю силу сквозь прозрачное, не теряет ничего —
/// так небо освещает дно колодца. Закрытая грань у любого из двух блоков
/// не пропускает свет вовсе.
fn passed(level: u8, from: u16, to: u16, direction: &Direction, sky: bool) -> u8 {
    if closed(from) & direction.face != 0 || closed(to) & direction.back != 0 {
        return 0;
    }

    let filter = filtered(to);

    if filter >= FULL {
        return 0;
    }

    if sky && direction.down && level == FULL && filter == 0 {
        return FULL;
    }

    level.saturating_sub(filter.max(1))
}

/// Свет одной секции: либо одинаковый во всех блоках, либо массив по
/// половине байта на блок — в том виде, в каком он уходит в пакет.
///
/// Одинаковых секций подавляющее большинство: толща камня тёмная, небо над
/// землёй светлое. На них массив не заводится вовсе.
#[derive(Clone, Debug, PartialEq)]
pub enum LightSection {
    Even(u8),
    Mixed(Arc<[u8; NIBBLES]>),
}

impl LightSection {
    /// Свет блока по его месту в секции (тот же порядок, что у блоков).
    pub fn get(&self, index: usize) -> u8 {
        match self {
            LightSection::Even(level) => *level,
            LightSection::Mixed(nibbles) => (nibbles[index / 2] >> ((index % 2) * 4)) & 0xF,
        }
    }

    /// Секция по свету каждого блока.
    fn from_levels(levels: &[u8]) -> Self {
        let first = levels[0];

        if levels.iter().all(|level| *level == first) {
            return LightSection::Even(first);
        }

        let mut nibbles = [0u8; NIBBLES];

        // Чётный блок — младшая половина байта, нечётный — старшая
        // (minecraft.wiki, Chunk format: «even blocks take the first nibble»).
        for (byte, [even, odd]) in nibbles.iter_mut().zip(levels.as_chunks::<2>().0) {
            *byte = even | odd << 4;
        }

        LightSection::Mixed(Arc::new(nibbles))
    }

    /// Свет каждого блока секции по байту на блок — для правки.
    fn levels(&self) -> Box<[u8; SECTION_BLOCKS]> {
        match self {
            LightSection::Even(level) => Box::new([*level; SECTION_BLOCKS]),
            LightSection::Mixed(nibbles) => {
                let mut levels = Box::new([0u8; SECTION_BLOCKS]);

                for ([even, odd], byte) in levels.as_chunks_mut::<2>().0.iter_mut().zip(nibbles.iter()) {
                    *even = byte & 0xF;
                    *odd = byte >> 4;
                }

                levels
            }
        }
    }

    /// Массив для пакета: по половине байта на блок.
    pub fn nibbles(&self) -> [u8; NIBBLES] {
        match self {
            LightSection::Even(level) => [level | level << 4; NIBBLES],
            LightSection::Mixed(nibbles) => **nibbles,
        }
    }
}

/// Свет чанка: небесный и блочный, по секциям снизу вверх.
#[derive(Clone, Debug, PartialEq)]
pub struct ChunkLight {
    pub sky: Vec<LightSection>,
    pub block: Vec<LightSection>,
}

impl ChunkLight {
    /// Свет чанка без единого блока: небо всюду полное, блочного нет.
    pub fn open_sky() -> Self {
        Self {
            sky: vec![LightSection::Even(FULL); SECTION_COUNT],
            block: vec![LightSection::Even(0); SECTION_COUNT],
        }
    }

    /// Небесный и блочный свет в блоке (местные координаты чанка).
    #[cfg(test)]
    pub fn at(&self, local_x: i32, y: i32, local_z: i32) -> (u8, u8) {
        let layer = (y - super::MIN_Y) as usize;
        let index = (layer % 16) * LAYER + local_z as usize * 16 + local_x as usize;

        (self.sky[layer / 16].get(index), self.block[layer / 16].get(index))
    }

    /// Сколько памяти занимают массивы света — для замера.
    #[cfg(test)]
    pub fn bytes(&self) -> usize {
        self.sky
            .iter()
            .chain(&self.block)
            .filter(|section| matches!(section, LightSection::Mixed(_)))
            .count()
            * NIBBLES
    }

    fn sections(&self, sky: bool) -> &[LightSection] {
        if sky { &self.sky } else { &self.block }
    }
}

/// Собственный свет чанка — только по его блокам.
///
/// За краями чанка считается пустота, в которую свет уходит без следа:
/// то, что придёт от соседей, добавит сшивка (`LightJob`).
pub fn chunk_light(sections: &[Option<Arc<[Packed]>>]) -> ChunkLight {
    let Some(top) = sections.iter().rposition(Option::is_some) else {
        return ChunkLight::open_sky();
    };

    // Считаем до верха самой высокой непустой секции и ещё на секцию выше:
    // туда может дотянуться свет факела, стоящего на самом верху. Выше —
    // только небо.
    let layers = ((top + 2) * 16).min(LAYERS);
    let cells = layers * LAYER;
    let table = table();

    let mut props = vec![0u16; cells];
    let mut sky = vec![0u8; cells];
    let mut block = vec![0u8; cells];
    let mut queue: Vec<u32> = Vec::new();

    // Свойства блоков — одним проходом, заодно и светящиеся блоки.
    for (section, states) in sections.iter().enumerate().take(top + 1) {
        let Some(states) = states else {
            continue;
        };
        let base = section * SECTION_BLOCKS;

        for (index, state) in states.iter().enumerate() {
            let cell = table[*state as usize];

            props[base + index] = cell;

            if emitted(cell) > 0 {
                block[base + index] = emitted(cell);
                queue.push((base + index) as u32);
            }
        }
    }

    spread(&mut block, &props, &mut queue, layers, false);

    // Небо — сначала по столбцам сверху вниз: так оно освещает всё, что под
    // открытым небом, без обхода. Заодно видно, где свет кончается совсем.
    let mut dark_below = layers;

    for column in 0..LAYER {
        let mut level = FULL;
        let mut above = 0u16;
        let mut layer = layers;

        while layer > 0 {
            layer -= 1;

            let cell = column + layer * LAYER;

            level = passed(level, above, props[cell], &DIRECTIONS[DOWN], true);

            if level == 0 {
                break;
            }

            sky[cell] = level;
            above = props[cell];
            dark_below = dark_below.min(layer);
        }
    }

    // Потом — вбок и вверх: под навес, в пещеры, в щели. Растекаться может
    // только то, что лежит между самым низким светлым блоком и самым высоким
    // слоем, где свет где-то неполный: выше всё небо одинаково полное, и
    // течь ему некуда.
    let partial = (dark_below..layers)
        .rev()
        .find(|layer| sky[layer * LAYER..(layer + 1) * LAYER].iter().any(|level| *level < FULL));

    if let Some(partial) = partial {
        for cell in dark_below * LAYER..(partial + 1) * LAYER {
            if sky[cell] >= 2 {
                relax_neighbours(&mut sky, &props, &mut queue, cell, layers, true);
            }
        }
    }

    spread(&mut sky, &props, &mut queue, layers, true);

    let mut light = ChunkLight::open_sky();

    for section in 0..(layers / 16) {
        let range = section * SECTION_BLOCKS..(section + 1) * SECTION_BLOCKS;

        light.sky[section] = LightSection::from_levels(&sky[range.clone()]);
        light.block[section] = LightSection::from_levels(&block[range]);
    }

    light
}

/// Сосед блока `cell` в направлении `direction` внутри чанка высотой
/// `layers` слоёв. None — сосед за краем.
fn neighbour(cell: usize, direction: &Direction, layers: usize) -> Option<usize> {
    let x = (cell % 16) as i32 + direction.x;
    let z = (cell / 16 % 16) as i32 + direction.z;
    let layer = (cell / LAYER) as i32 + direction.y;

    if !(0..16).contains(&x) || !(0..16).contains(&z) || !(0..layers as i32).contains(&layer) {
        return None;
    }

    Some(layer as usize * LAYER + z as usize * 16 + x as usize)
}

/// Отдаёт свет блока `cell` соседям, у которых его меньше, и ставит их
/// в очередь.
fn relax_neighbours(
    light: &mut [u8],
    props: &[u16],
    queue: &mut Vec<u32>,
    cell: usize,
    layers: usize,
    sky: bool,
) {
    let level = light[cell];

    for direction in &DIRECTIONS {
        let Some(next) = neighbour(cell, direction, layers) else {
            continue;
        };

        let reached = passed(level, props[cell], props[next], direction, sky);

        if reached > light[next] {
            light[next] = reached;
            queue.push(next as u32);
        }
    }
}

/// Растекание света от блоков из очереди — по кругу, пока есть куда.
///
/// Свет только прибывает, поэтому порядок обхода на итог не влияет; очередь
/// по порядку поступления просто обходит каждый блок меньше раз.
fn spread(light: &mut [u8], props: &[u16], queue: &mut Vec<u32>, layers: usize, sky: bool) {
    let mut at = 0;

    while at < queue.len() {
        let cell = queue[at] as usize;
        at += 1;

        if light[cell] < 2 {
            continue;
        }

        relax_neighbours(light, props, queue, cell, layers, sky);
    }

    queue.clear();
}

/// Чанк для сшивки света: его блоки и собственный свет.
pub struct LightInput {
    /// Какой чанк.
    pub place: (i32, i32),

    /// Номер правки блоков, к которой относится снимок.
    pub revision: u32,

    pub sections: Vec<Option<Arc<[Packed]>>>,

    /// Собственный свет. None — устарел или ещё не посчитан.
    pub light: Option<Arc<ChunkLight>>,
}

/// Собственный свет, посчитанный заново при сшивке: его стоит положить
/// обратно в чанк, чтобы не считать ещё раз.
pub struct Recounted {
    pub place: (i32, i32),
    pub revision: u32,
    pub light: Arc<ChunkLight>,
}

/// Всё, что нужно для света чанка вне захвата мира: сам чанк и восемь
/// соседей (тех, что есть в памяти). Блоки не копируются — секции общие
/// с миром, и правка в мире заводит себе новую копию секции.
pub struct LightJob {
    /// Чанки 3×3 по порядку: сначала по X, потом по Z; середина — наш.
    pub chunks: [Option<LightInput>; 9],
}

/// Место чанка среди девяти: середина — четвёртое.
const CENTER: usize = 4;

/// Место блока в сшиваемом куске 3×3 чанка: X и Z от −16 до 31 (наш чанк —
/// от 0 до 15), слой — от дна мира.
#[derive(Clone, Copy)]
struct Spot {
    x: i32,
    layer: i32,
    z: i32,
}

impl Spot {
    fn pack(self) -> u32 {
        (self.x + 16) as u32 | ((self.z + 16) as u32) << 6 | (self.layer as u32) << 12
    }

    fn unpack(packed: u32) -> Self {
        Spot {
            x: (packed & 63) as i32 - 16,
            z: ((packed >> 6) & 63) as i32 - 16,
            layer: (packed >> 12) as i32,
        }
    }

    fn step(self, direction: &Direction) -> Option<Self> {
        let next = Spot {
            x: self.x + direction.x,
            layer: self.layer + direction.y,
            z: self.z + direction.z,
        };
        let inside = (-16..32).contains(&next.x)
            && (-16..32).contains(&next.z)
            && (0..LAYERS as i32).contains(&next.layer);

        inside.then_some(next)
    }

    /// Какой это чанк из девяти, какая секция и место в ней.
    fn locate(self) -> (usize, usize, usize) {
        let (x, z) = ((self.x + 16) as usize, (self.z + 16) as usize);
        let chunk = (z / 16) * 3 + x / 16;
        let layer = self.layer as usize;
        let index = (layer % 16) * LAYER + (z % 16) * 16 + x % 16;

        (chunk, layer / 16, index)
    }

    /// Сколько шагов по горизонтали отсюда до нашего чанка. Свету слабее
    /// этого до нас не дойти, и тратить на него время незачем.
    fn distance(self) -> u8 {
        let off = |value: i32| (-value).max(value - 15).max(0);

        (off(self.x) + off(self.z)) as u8
    }
}

/// Чанк в сшивке: его секции блоков и собственный свет одного вида.
type SeamChunk<'a> = (&'a [Option<Arc<[Packed]>>], &'a [LightSection]);

/// Сшиваемый свет одного вида (небесный или блочный) у девяти чанков.
struct Seam<'a> {
    chunks: [Option<SeamChunk<'a>>; 9],

    /// Секции, в которых свет уже правился: по байту на блок.
    edited: Vec<Option<Box<[u8; SECTION_BLOCKS]>>>,

    table: &'static [u16],
    sky: bool,
    queue: Vec<u32>,
}

impl Seam<'_> {
    /// Свойства блока. None — чанка нет в памяти, туда свет не идёт.
    fn props(&self, (chunk, section, index): (usize, usize, usize)) -> Option<u16> {
        let (sections, _) = self.chunks[chunk]?;

        Some(match &sections[section] {
            Some(states) => self.table[states[index] as usize],
            None => 0,
        })
    }

    fn light(&self, (chunk, section, index): (usize, usize, usize)) -> u8 {
        if let Some(levels) = &self.edited[chunk * SECTION_COUNT + section] {
            return levels[index];
        }

        self.chunks[chunk].map_or(0, |(_, light)| light[section].get(index))
    }

    fn set_light(&mut self, (chunk, section, index): (usize, usize, usize), level: u8) {
        let slot = chunk * SECTION_COUNT + section;

        if self.edited[slot].is_none() {
            let own = self.chunks[chunk].map(|(_, light)| light[section].levels());

            self.edited[slot] = own;
        }

        if let Some(levels) = &mut self.edited[slot] {
            levels[index] = level;
        }
    }

    /// Пропускает свет из `from` в `to`, если там его станет больше и он
    /// ещё может дойти до нашего чанка.
    fn relax(&mut self, from: Spot, to: Spot, direction: &Direction) {
        let (source, target) = (from.locate(), to.locate());
        let level = self.light(source);

        if level < 2 {
            return;
        }

        let (Some(from_props), Some(to_props)) = (self.props(source), self.props(target)) else {
            return;
        };

        let reached = passed(level, from_props, to_props, direction, self.sky);

        if reached > self.light(target) && reached > to.distance() {
            self.set_light(target, reached);
            self.queue.push(to.pack());
        }
    }

    /// Растекание от всего, что в очереди.
    fn spread(&mut self) {
        let mut at = 0;

        while at < self.queue.len() {
            let spot = Spot::unpack(self.queue[at]);
            at += 1;

            for direction in &DIRECTIONS {
                if let Some(next) = spot.step(direction) {
                    self.relax(spot, next, direction);
                }
            }
        }

        self.queue.clear();
    }

    /// Свет через границы соседних чанков: где по одну сторону светлее, чем
    /// может дать другая, — свет переходит и растекается дальше.
    fn stitch(&mut self) {
        // Пары соседей: (левый или северный, правый или южный, направление).
        let mut pairs = Vec::with_capacity(12);

        for row in 0..3 {
            for column in 0..3 {
                let chunk = row * 3 + column;

                if column < 2 {
                    pairs.push((chunk, chunk + 1, EAST));
                }

                if row < 2 {
                    pairs.push((chunk, chunk + 3, SOUTH));
                }
            }
        }

        for (first, second, forward) in pairs {
            let (Some((_, first_light)), Some((_, second_light))) =
                (self.chunks[first], self.chunks[second])
            else {
                continue;
            };

            // Угол первого чанка в координатах куска.
            let origin_x = (first % 3) as i32 * 16 - 16;
            let origin_z = (first / 3) as i32 * 16 - 16;

            for section in 0..SECTION_COUNT {
                // Одинаковые по всей секции стороны, отличающиеся не больше
                // чем на единицу, друг другу ничего не дадут.
                if let (LightSection::Even(a), LightSection::Even(b)) =
                    (&first_light[section], &second_light[section])
                    && a.abs_diff(*b) <= 1
                {
                    continue;
                }

                for layer in section * 16..section * 16 + 16 {
                    for along in 0..16 {
                        let (near, far) = if forward == EAST {
                            (
                                Spot { x: origin_x + 15, layer: layer as i32, z: origin_z + along },
                                Spot { x: origin_x + 16, layer: layer as i32, z: origin_z + along },
                            )
                        } else {
                            (
                                Spot { x: origin_x + along, layer: layer as i32, z: origin_z + 15 },
                                Spot { x: origin_x + along, layer: layer as i32, z: origin_z + 16 },
                            )
                        };

                        self.relax(near, far, &DIRECTIONS[forward]);
                        self.relax(far, near, &DIRECTIONS[forward ^ 1]);
                    }
                }
            }
        }

        self.spread();
    }

    /// Свет нашего чанка после сшивки.
    fn center(&self) -> Vec<LightSection> {
        let (_, own) = self.chunks[CENTER].expect("свет считается только у чанка из памяти");

        (0..SECTION_COUNT)
            .map(|section| match &self.edited[CENTER * SECTION_COUNT + section] {
                Some(levels) => LightSection::from_levels(&levels[..]),
                None => own[section].clone(),
            })
            .collect()
    }
}

impl LightJob {
    /// Свет чанка с учётом соседей и заново посчитанный собственный свет
    /// тех, у кого он устарел.
    pub fn finish(self) -> (ChunkLight, Vec<Recounted>) {
        let mut recounted = Vec::new();
        let mut own: [Option<Arc<ChunkLight>>; 9] = Default::default();

        for (slot, input) in self.chunks.iter().enumerate() {
            let Some(input) = input else {
                continue;
            };

            own[slot] = Some(match &input.light {
                Some(light) => Arc::clone(light),
                None => {
                    let light = Arc::new(chunk_light(&input.sections));

                    recounted.push(Recounted {
                        place: input.place,
                        revision: input.revision,
                        light: Arc::clone(&light),
                    });

                    light
                }
            });
        }

        let mut result = [true, false].map(|sky| {
            let mut seam = Seam {
                chunks: std::array::from_fn(|slot| {
                    let input = self.chunks[slot].as_ref()?;
                    let light = own[slot].as_ref()?;

                    Some((&input.sections[..], light.sections(sky)))
                }),
                edited: vec![None; 9 * SECTION_COUNT],
                table: table(),
                sky,
                queue: Vec::new(),
            };

            seam.stitch();
            seam.center()
        });

        let block = std::mem::take(&mut result[1]);
        let sky = std::mem::take(&mut result[0]);

        (ChunkLight { sky, block }, recounted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(name: &str) -> i32 {
        crate::blocks::state_by_name(name).expect(name)
    }

    #[test]
    fn the_table_knows_light_sources_and_blockers() {
        assert_eq!(emission(state("lava")), 15);
        assert_eq!(emission(state("glowstone")), 15);
        assert_eq!(emission(state("torch")), 14);
        assert_eq!(emission(state("magma_block")), 3);
        assert_eq!(emission(state("stone")), 0);

        assert_eq!(filter(state("stone")), 15);
        assert_eq!(filter(state("water")), 1);
        assert_eq!(filter(state("oak_leaves")), 1);
        assert_eq!(filter(state("glass")), 0);
        assert_eq!(filter(0), 0);
    }

    /// Свет, зависящий от состояния: по вики.
    #[test]
    fn light_depends_on_the_state_where_the_wiki_says_so() {
        let furnace = crate::blocks::orientation("furnace").expect("печь");
        let cold = state("furnace");
        let lit = furnace.with(cold, "lit", "true").expect("горящая печь");

        assert_eq!(emission(cold), 0);
        assert_eq!(emission(lit), 13);

        let vines = crate::blocks::orientation("cave_vines").expect("лоза");
        let bare = state("cave_vines");
        let berries = vines.with(bare, "berries", "true").expect("с ягодами");

        assert_eq!(emission(berries), 14);
        assert_eq!(emission(vines.with(bare, "berries", "false").expect("без ягод")), 0);

        let candle = crate::blocks::orientation("candle").expect("свеча");
        let four = candle
            .state(&[("candles", "4"), ("lit", "true"), ("waterlogged", "false")])
            .expect("четыре свечи");

        assert_eq!(emission(four), 12);
        assert_eq!(emission(state("glow_lichen")), 7);
    }

    use crate::world::{World, flat::GROUND};

    /// Мир из ровных чанков вокруг начала координат.
    fn flat_world() -> World {
        let mut world = World::in_memory();

        for chunk_x in -2..=2 {
            for chunk_z in -2..=2 {
                world.ensure(chunk_x, chunk_z);
            }
        }

        world
    }

    /// Кладёт прямоугольник блоков на высоте `y`, от угла до угла включительно.
    fn fill(world: &mut World, (from_x, from_z): (i32, i32), (to_x, to_z): (i32, i32), y: i32, name: &str) {
        let block = state(name);

        for x in from_x..=to_x {
            for z in from_z..=to_z {
                world.set_block_silently(x, y, z, block);
            }
        }
    }

    /// Под открытым небом свет полный от неба до земли, под навесом — слабеет
    /// на единицу с каждым шагом от края.
    #[test]
    fn open_sky_is_full_and_fades_under_a_roof() {
        let mut world = flat_world();
        let floor = GROUND + 1;

        assert_eq!(world.light_at(8, floor, 8), (15, 0));
        assert_eq!(world.light_at(8, 200, 8), (15, 0));

        // Навес 9×9 над землёй, на высоте четырёх блоков.
        fill(&mut world, (4, 4), (12, 12), floor + 3, "stone");

        assert_eq!(world.light_at(3, floor, 8).0, 15, "у края навеса — небо");
        assert_eq!(world.light_at(4, floor, 8).0, 14, "шаг под навес");
        assert_eq!(world.light_at(6, floor, 8).0, 12);
        assert_eq!(world.light_at(8, floor, 8).0, 10, "середина навеса");
        assert_eq!(world.light_at(8, floor + 3, 8).0, 0, "в самом камне темно");
        assert_eq!(world.light_at(8, floor + 4, 8).0, 15, "на навесе — небо");

        // Земля глухая: под дёрном неба нет.
        assert_eq!(world.light_at(8, GROUND, 8).0, 0);
    }

    /// Листва и вода гасят небесный свет по единице на блок.
    #[test]
    fn leaves_and_water_dim_the_sky_step_by_step() {
        let mut world = flat_world();
        let floor = GROUND + 1;

        for y in floor..floor + 3 {
            fill(&mut world, (-8, -8), (8, 8), y, "water");
        }

        assert_eq!(world.light_at(0, floor + 2, 0).0, 14);
        assert_eq!(world.light_at(0, floor + 1, 0).0, 13);
        assert_eq!(world.light_at(0, floor, 0).0, 12);
    }

    /// Замкнутая пещера тёмная, и ночь в ней от дня не отличается.
    #[test]
    fn a_closed_cave_is_dark() {
        let mut world = flat_world();

        // Каменная коробка 7×7×7 с пустотой 5×5×5 внутри.
        for y in 10..17 {
            let wall = y == 10 || y == 16;

            for x in 0..7 {
                for z in 0..7 {
                    let edge = x == 0 || x == 6 || z == 0 || z == 6;
                    let block = if wall || edge { state("stone") } else { 0 };

                    world.set_block_silently(x, y, z, block);
                }
            }
        }

        for (x, y, z) in [(3, 13, 3), (1, 11, 1), (5, 15, 5)] {
            assert_eq!(world.light_at(x, y, z), (0, 0), "в пещере светло: {x} {y} {z}");
        }
    }

    /// Пещеры сложенного мира в толще тёмные: небо в них почти не заходит.
    #[test]
    fn generated_caves_are_mostly_dark() {
        use crate::world::terrain::{Style, Terrain};
        use crate::world::{Chunk, Generator, MIN_Y};

        let generator = Generator::Normal(Terrain::new(100_554_032_945_340, Style::Vanilla));
        let (mut hollow, mut lit) = (0, 0);

        for chunk_x in 0..4 {
            for chunk_z in 0..4 {
                let chunk = Chunk::generated(&generator, chunk_x, chunk_z);
                let (_, light) = chunk.light.as_ref().expect("свет посчитан при складывании");

                for y in MIN_Y + 8..-16 {
                    for x in 0..16 {
                        for z in 0..16 {
                            if chunk.block(x, y, z) != 0 {
                                continue;
                            }

                            hollow += 1;

                            if light.at(x, y, z).0 > 0 {
                                lit += 1;
                            }
                        }
                    }
                }
            }
        }

        assert!(hollow > 1000, "пещер не нашлось: {hollow}");
        assert!(lit * 20 < hollow, "в глубоких пещерах светло: {lit} из {hollow}");
    }

    /// Лава светит в полную силу, и свет слабеет на единицу за шаг.
    #[test]
    fn lava_shines_fifteen_and_fades() {
        let mut world = flat_world();
        let y = GROUND + 5;

        world.set_block_silently(8, y, 8, state("lava"));

        assert_eq!(world.light_at(8, y, 8).1, 15);
        assert_eq!(world.light_at(9, y, 8).1, 14);
        assert_eq!(world.light_at(8, y + 3, 8).1, 12);
        assert_eq!(world.light_at(8, y, 3).1, 10);
        assert_eq!(world.light_at(10, y + 2, 10).1, 9, "по прямой через углы");
        assert_eq!(world.light_at(8, y, 30).1, 0, "дальше пятнадцати — темно");
    }

    /// Свет переходит через границу чанков — и блочный, и небесный.
    #[test]
    fn light_crosses_chunk_borders() {
        let mut world = flat_world();
        let floor = GROUND + 1;

        // Светокамень у самого края чанка: соседний чанк освещён им же.
        world.set_block_silently(15, floor + 5, 8, state("glowstone"));

        assert_eq!(world.light_at(16, floor + 5, 8).1, 14);
        assert_eq!(world.light_at(20, floor + 5, 8).1, 10);
        assert_eq!(world.light_at(14, floor + 5, 8).1, 14);

        // И в обратную сторону — от светокамня за границей.
        world.set_block_silently(-1, floor + 5, 24, state("glowstone"));

        assert_eq!(world.light_at(0, floor + 5, 24).1, 14);
        assert_eq!(world.light_at(3, floor + 5, 24).1, 11);

        // Навес над двумя чанками по X и тремя по Z; открыт только край по
        // X = 16. Небо заходит под навес только оттуда.
        fill(&mut world, (-16, -16), (15, 31), floor + 3, "stone");

        assert_eq!(world.light_at(16, floor, 8).0, 15);
        assert_eq!(world.light_at(15, floor, 8).0, 14);
        assert_eq!(world.light_at(10, floor, 8).0, 9);
        assert_eq!(world.light_at(-1, floor, 8).0, 0, "дальше пятнадцати шагов — темно");
    }

    /// Убрали кусок крыши — под ним снова небо.
    #[test]
    fn removing_a_roof_brings_the_sky_back() {
        let mut world = flat_world();
        let floor = GROUND + 1;

        fill(&mut world, (-16, -16), (31, 31), floor + 3, "stone");

        assert_eq!(world.light_at(5, floor, 8).0, 0, "под сплошной крышей темно");

        world.set_block(5, floor + 3, 8, 0);

        assert_eq!(world.light_at(5, floor, 8).0, 15, "в дыру светит небо");
        assert_eq!(world.light_at(6, floor, 8).0, 14);
        assert_eq!(world.light_at(5, floor, 12).0, 11);

        // И наоборот: дыру заложили — снова темно.
        world.set_block(5, floor + 3, 8, state("stone"));

        assert_eq!(world.light_at(5, floor, 8).0, 0);
    }

    /// Нижняя плита не пускает свет вниз, но светится сама.
    #[test]
    fn a_bottom_slab_blocks_light_downward() {
        let mut world = flat_world();
        let floor = GROUND + 1;
        let slab = state("oak_slab");

        assert_eq!(
            crate::blocks::orientation("oak_slab").and_then(|block| block.value(slab, "type")),
            Some("bottom")
        );

        fill(&mut world, (-16, -16), (31, 31), floor + 3, "oak_slab");

        assert_eq!(world.light_at(5, floor + 3, 8).0, 15, "в плите — небо");
        assert_eq!(world.light_at(5, floor, 8).0, 0, "под плитами темно");
    }

    /// Собственный свет хранится без массивов там, где секция однородна.
    #[test]
    fn even_sections_take_no_arrays() {
        let world = flat_world();
        let chunk = world.chunks.get(&(0, 0)).expect("чанк");
        let (_, light) = chunk.light.as_ref().expect("свет");

        // Ровный мир: во всех секциях свет одинаковый — сверху небо, снизу
        // темно, а слой земли целиком в одной секции… кроме неё самой.
        assert!(light.bytes() <= 2 * NIBBLES, "массивов слишком много: {}", light.bytes());
    }

    /// Свет «в лоб»: каждый блок берёт лучшее из того, что дают соседи, и так
    /// по кругу, пока что-то меняется. Медленно, зато без хитростей — им
    /// проверяется быстрый счёт.
    fn light_by_brute_force(sections: &[Option<Arc<[Packed]>>], sky: bool) -> Vec<u8> {
        let table = table();
        let props = |cell: usize| {
            sections[cell / SECTION_BLOCKS]
                .as_ref()
                .map_or(0, |states| table[states[cell % SECTION_BLOCKS] as usize])
        };
        // Выше секции над самым высоким блоком — только небо: туда не
        // дотягивается даже свет факела, стоящего на самом верху.
        let layers = sections.iter().rposition(Option::is_some).map_or(16, |top| (top + 2) * 16).min(LAYERS);
        let mut light: Vec<u8> = (0..layers * LAYER)
            .map(|cell| if sky { 0 } else { emitted(props(cell)) })
            .collect();

        for pass in 0.. {
            let mut changed = false;

            // Проходы то в одну сторону, то в другую: так свет расходится
            // за считанные круги.
            for step in 0..layers * LAYER {
                let cell = if pass % 2 == 0 { step } else { layers * LAYER - 1 - step };
                let mut best = light[cell];

                // Сверху — открытое небо.
                if sky && cell / LAYER == layers - 1 {
                    best = best.max(passed(FULL, 0, props(cell), &DIRECTIONS[DOWN], true));
                }

                for (number, direction) in DIRECTIONS.iter().enumerate() {
                    // Сосед, из которого свет пришёл бы в этом направлении.
                    let Some(from) = neighbour(cell, &DIRECTIONS[number ^ 1], layers) else {
                        continue;
                    };

                    best = best.max(passed(light[from], props(from), props(cell), direction, sky));
                }

                if best > light[cell] {
                    light[cell] = best;
                    changed = true;
                }
            }

            if !changed {
                break;
            }
        }

        light.resize(LAYERS * LAYER, if sky { FULL } else { 0 });
        light
    }

    /// Быстрый счёт сходится со счётом «в лоб» на сложенных чанках.
    #[test]
    fn chunk_light_matches_brute_force() {
        use crate::world::terrain::{Style, Terrain};
        use crate::world::{Chunk, Generator};

        let generator = Generator::Normal(Terrain::new(100_554_032_945_340, Style::Vanilla));

        for (chunk_x, chunk_z) in [(0, 0), (-7, 2)] {
            let chunk = Chunk::generated(&generator, chunk_x, chunk_z);
            let light = chunk_light(&chunk.sections);

            for sky in [true, false] {
                let expected = light_by_brute_force(&chunk.sections, sky);

                for (cell, level) in expected.iter().enumerate() {
                    let got = light.sections(sky)[cell / SECTION_BLOCKS].get(cell % SECTION_BLOCKS);

                    assert_eq!(
                        got, *level,
                        "чанк {chunk_x} {chunk_z}, {} свет, блок {cell}",
                        if sky { "небесный" } else { "блочный" }
                    );
                }
            }
        }
    }

    /// Сшитый свет чанка сходится со счётом «в лоб» по всему куску 3×3:
    /// всё, что дальше соседей, до нашего чанка не достаёт.
    #[test]
    fn stitched_light_matches_brute_force_over_neighbours() {
        use crate::world::terrain::{Style, Terrain};
        use crate::world::{Chunk, Generator};

        let generator = Generator::Normal(Terrain::new(100_554_032_945_340, Style::Vanilla));
        let (center_x, center_z) = (3, 5);
        let mut chunks: Vec<Chunk> = (0..9)
            .map(|slot| Chunk::generated(&generator, center_x + slot % 3 - 1, center_z + slot / 3 - 1))
            .collect();

        // Свет у самых границ: светокамень в углу чанка наискосок, лава
        // у края соседа, факел у нашего края под навесом.
        chunks[0].set(15, 150, 15, state("glowstone"));
        chunks[3].set(15, 150, 8, state("lava"));
        chunks[5].set(0, 140, 3, state("torch"));

        for x in 10..16 {
            for z in 0..16 {
                chunks[4].set(x, 142, z, state("stone"));
            }
        }

        // Весь кусок одним массивом: X и Z от 0 до 47, слои снизу вверх.
        const SIDE: usize = 48;
        let table = table();
        let place = |x: usize, layer: usize, z: usize| (layer * SIDE + z) * SIDE + x;
        let mut props = vec![0u16; SIDE * SIDE * LAYERS];

        for (slot, chunk) in chunks.iter().enumerate() {
            for (section, states) in chunk.sections.iter().enumerate() {
                let Some(states) = states else {
                    continue;
                };

                for (index, state) in states.iter().enumerate() {
                    let x = slot % 3 * 16 + index % 16;
                    let z = slot / 3 * 16 + index / 16 % 16;
                    let layer = section * 16 + index / LAYER;

                    props[place(x, layer, z)] = table[*state as usize];
                }
            }
        }

        let job = LightJob {
            chunks: std::array::from_fn(|slot| {
                let chunk = &chunks[slot];

                Some(LightInput {
                    place: (slot as i32 % 3, slot as i32 / 3),
                    revision: 0,
                    sections: chunk.sections.clone(),
                    light: None,
                })
            }),
        };
        let (stitched, recounted) = job.finish();

        assert_eq!(recounted.len(), 9, "свет без готового собственного считается заново");

        // Выше секции над самым высоким блоком куска — только небо.
        let top = chunks
            .iter()
            .filter_map(|chunk| chunk.sections.iter().rposition(Option::is_some))
            .max()
            .unwrap_or(0);
        let layers = ((top + 2) * 16).min(LAYERS);

        for sky in [true, false] {
            let mut light: Vec<u8> = props.iter().map(|cell| if sky { 0 } else { emitted(*cell) }).collect();

            for pass in 0.. {
                let mut changed = false;

                for step in 0..layers * SIDE * SIDE {
                    let cell = if pass % 2 == 0 { layers * SIDE * SIDE - 1 - step } else { step };
                    let (x, z, layer) = (cell % SIDE, cell / SIDE % SIDE, cell / (SIDE * SIDE));

                    let mut best = light[cell];

                    if sky && layer == layers - 1 {
                        best = best.max(passed(FULL, 0, props[cell], &DIRECTIONS[DOWN], true));
                    }

                    for (number, direction) in DIRECTIONS.iter().enumerate() {
                        let back = &DIRECTIONS[number ^ 1];
                        let (fx, fl, fz) = (
                            x as i32 + back.x,
                            layer as i32 + back.y,
                            z as i32 + back.z,
                        );

                        if !(0..SIDE as i32).contains(&fx)
                            || !(0..SIDE as i32).contains(&fz)
                            || !(0..layers as i32).contains(&fl)
                        {
                            continue;
                        }

                        let from = place(fx as usize, fl as usize, fz as usize);

                        best = best.max(passed(light[from], props[from], props[cell], direction, sky));
                    }

                    if best > light[cell] {
                        light[cell] = best;
                        changed = true;
                    }
                }

                if !changed {
                    break;
                }
            }

            for layer in 0..LAYERS {
                for z in 0..16 {
                    for x in 0..16 {
                        let expected = match layer < layers {
                            true => light[place(x + 16, layer, z + 16)],
                            false if sky => FULL,
                            false => 0,
                        };
                        let got = stitched.sections(sky)[layer / 16].get((layer % 16) * LAYER + z * 16 + x);

                        assert_eq!(got, expected, "{} свет в {x} {layer} {z}", if sky { "небесный" } else { "блочный" });
                    }
                }
            }
        }
    }
}
