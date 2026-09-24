// Мир: хранение блоков.
//
// Представление простое и «в лоб»: блоки хранятся по чанкам, состояния блоков —
// числами (теми же, какими они передаются клиенту в пакетах). Ни сжатия, ни
// регионов: мир состоит из ровной земли и того, что игроки в ней изменили.
//
// Чанки складываются по мере надобности — когда игрок до них дошёл (см. flat.rs
// и `ensure`), и с этого мгновения хранятся вместе с остальным миром.
//
// Состояние блока — число из таблицы состояний самой игры. Сервер обязан
// знать эти числа: по протоколу клиент их не сообщает. Нам нужны только те
// состояния, которые мы сами ставим, поэтому таблица ограничена константами
// ниже; правильность каждого числа проверяется глазами в игре.
//
// Мир лежит на диске одним файлом: список непустых блоков по чанкам. Пустые
// блоки не записываются — иначе файл распух бы в 384 раза на ровном месте.
// Сохранение происходит сразу при каждом изменении: сервер маленький, и
// терять постройки при падении не хочется.

pub mod cave_biomes;
pub mod flat;
pub mod light;
pub mod noise;
pub mod ocean;
pub mod region;
pub mod terrain;
pub mod trees;
pub mod underground;
pub mod vegetation;

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use crate::config::server_properties::WorldKind;
use crate::fluids;
use crate::journal::{self, Journal};
use crate::redstone;
use crate::world::terrain::{Column, Style, Terrain};
use crate::{log_debug, log_error, log_info, log_warn};

/// Воздух — состояние 0. Это единственное число, в котором мы уверены
/// заранее: пустая секция чанка кодируется именно нулём.
pub const AIR: i32 = 0;

/// Нижняя граница мира: как у обычного overworld.
pub const MIN_Y: i32 = -64;

/// Высота мира.
pub const WORLD_HEIGHT: i32 = 384;

/// Секций в чанке.
pub const SECTIONS: i32 = WORLD_HEIGHT / 16;

/// Блоков в секции: 16 × 16 × 16.
pub const SECTION_BLOCKS: usize = 4096;

/// Сторона чанка в блоках.
const CHUNK_SIZE: i32 = 16;

/// Сколько тактов длятся сутки.
pub const DAY_LENGTH: i64 = 24_000;

/// Одно изменение блока — то, что нужно разослать остальным игрокам.
#[derive(Clone, Copy)]
pub struct BlockChange {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub state: i32,

    /// Отправить отдельным сообщением, не складывая в пачку с соседями по
    /// секции: так уходят повторы блока после щелчка игрока.
    pub direct: bool,
}

/// Действие блока — то, что клиент проигрывает сам.
///
/// Так устроен ход поршня: сервер сообщает «поршень в этом месте поехал
/// туда-то», и клиент двигает блоки у себя плавно, пока сервер выжидает
/// положенные два такта.
#[derive(Clone, Copy)]
pub struct BlockAction {
    pub x: i32,
    pub y: i32,
    pub z: i32,

    /// Что происходит: у поршня 0 — выдвигается, 1 — задвигается,
    /// 2 — отменяет начатое выдвигание.
    pub action: u8,

    /// Уточнение: у поршня — сторона, куда он смотрит.
    pub param: u8,

    /// Номер блока (не состояния): по нему клиент сверяет, к чему это
    /// относится.
    pub block: i32,
}

/// Звук в точке мира: щелчок рычага, ход поршня.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldSound {
    pub x: i32,
    pub y: i32,
    pub z: i32,

    /// Имя звука как в игре, с приставкой `minecraft:`.
    pub name: &'static str,
    pub volume: f32,
    pub pitch: f32,
}

/// Событие мира, которое надо донести до клиентов: изменение блока или
/// действие блока.
///
/// Журнал у них один: порядок между ними важен. Действие поршня обязано
/// уйти раньше, чем изменение «поршень выдвинут», а воздух на месте
/// сломанного блока — раньше действия, в которое клиент этот воздух
/// заполнит. Два журнала такого порядка не держат: подключение выливало бы
/// сперва один целиком, потом другой, и действие более позднего такта
/// обгоняло бы изменения более раннего.
#[derive(Clone, Copy)]
pub enum WorldEvent {
    Change(BlockChange),
    Action(BlockAction),

    /// Звук, который слышно в этом месте.
    Sound(WorldSound),
}

/// Блоки одного чанка — по секциям.
///
/// Секция — это куб 16×16×16, и хранится она отдельно. Причина простая:
/// почти весь мир — воздух. Чанк целиком занял бы больше трети мегабайта,
/// и сотня пройденных чанков съела бы десятки мегабайт ни за чем; а так
/// воздушная секция не занимает ничего.
///
/// Порядок блоков внутри секции — тот же, в котором они идут в пакете чанка:
/// сначала по Y (снизу вверх), внутри — по Z, внутри — по X. Поэтому секцию
/// не приходится перекладывать при отправке.
struct Chunk {
    /// Секции снизу вверх. None — секция целиком из воздуха.
    ///
    /// Состояние блока хранится в двух байтах, а не в четырёх: номеров
    /// состояний в игре около тридцати тысяч, и в два байта они влезают
    /// с запасом. На дальности в тридцать два чанка это половина памяти
    /// сервера — сотни мегабайт.
    ///
    /// Секция лежит под `Arc`: свет чанка считается вне захвата мира, и ему
    /// достаётся не копия блоков, а та же секция. Правка в мире в это время
    /// заводит себе новую копию секции, а считающий свет дочитывает старую.
    sections: Vec<Option<Arc<[Packed]>>>,

    /// Биомы чанка: по одному на клетку 4×4×4 блока — так же дробно, как их
    /// передаёт протокол, и в том же порядке: снизу вверх, внутри слоя по Z,
    /// потом по X. У поверхности и выше — биом столбца, в толще под ней —
    /// пещерный, где его выбрали климат и глубина.
    ///
    /// Номер биома хранится в двух байтах: биомов в реестре меньше сотни.
    biomes: Vec<BiomeId>,

    /// Собственный свет чанка — посчитанный только по его блокам (см.
    /// light.rs) — и номер правки блоков, к которой он относится. Не сходится
    /// номер с `revision` — свет устарел и будет пересчитан при отправке.
    light: Option<(u32, Arc<light::ChunkLight>)>,

    /// Номер правки блоков: растёт при каждом изменении.
    revision: u32,
}

/// Номер биома в реестре, как он лежит в памяти чанка.
pub type BiomeId = u16;

/// Состояние блока в том виде, в каком оно лежит в памяти чанка.
type Packed = u16;

/// Ужимает состояние для хранения. Номеров состояний в игре около тридцати
/// тысяч; если однажды их станет больше, чем помещается, лучше об этом
/// узнать сразу, а не получить чужой блок в мире.
fn pack(state: i32) -> Packed {
    Packed::try_from(state).unwrap_or_else(|_| panic!("состояние {} не влезает в два байта", state))
}

/// Разворачивает хранимое состояние обратно.
fn unpack(state: Packed) -> i32 {
    state as i32
}

/// Сколько клеток биома приходится на сторону чанка: биом задаётся на каждые
/// четыре блока, так же как в пакете чанка.
const BIOME_CELLS: usize = (CHUNK_SIZE / 4) as usize;

/// Слоёв клеток биома по высоте мира.
const BIOME_LAYERS: usize = (WORLD_HEIGHT / 4) as usize;

/// Клеток биома в слое 4×4.
const BIOME_LAYER: usize = BIOME_CELLS * BIOME_CELLS;

/// Клеток биома в секции: 4×4×4.
pub const SECTION_BIOMES: usize = BIOME_LAYER * 4;

/// Место клетки биома в `Chunk::biomes`: слой по высоте (от дна мира), затем
/// клетка по Z и по X.
fn biome_cell(layer: usize, cell_x: usize, cell_z: usize) -> usize {
    layer * BIOME_LAYER + cell_z * BIOME_CELLS + cell_x
}

impl Chunk {
    fn new() -> Self {
        Self {
            sections: (0..SECTIONS).map(|_| None).collect(),
            biomes: vec![0; BIOME_LAYERS * BIOME_LAYER],
            light: None,
            revision: 0,
        }
    }

    /// Считает собственный свет чанка по его нынешним блокам.
    fn light_up(&mut self) {
        self.light = Some((self.revision, Arc::new(light::chunk_light(&self.sections))));
    }

    /// Собственный свет, если он не устарел.
    fn fresh_light(&self) -> Option<&Arc<light::ChunkLight>> {
        match &self.light {
            Some((revision, light)) if *revision == self.revision => Some(light),
            _ => None,
        }
    }

    /// Чанк сложенного мира: рельеф, биомы и всё, что из них следует.
    fn generated(generator: &Generator, chunk_x: i32, chunk_z: i32) -> Self {
        let mut chunk = Self::new();

        // Шумы считаются один раз на столбец, а не на каждый блок: иначе на
        // чанк приходилось бы сто тысяч обращений к шуму. Берём и кайму
        // в блок шириной — крутизну склона видно только по соседям.
        const SIDE: i32 = CHUNK_SIZE + 2;
        let (from_x, from_z) = (chunk_x * CHUNK_SIZE - 1, chunk_z * CHUNK_SIZE - 1);
        let grid = generator.relief_grid(from_x, from_z, from_x + SIDE - 1, from_z + SIDE - 1);
        let mut columns = Vec::with_capacity((SIDE * SIDE) as usize);

        for local_x in -1..=CHUNK_SIZE {
            for local_z in -1..=CHUNK_SIZE {
                let (x, z) = (
                    chunk_x * CHUNK_SIZE + local_x,
                    chunk_z * CHUNK_SIZE + local_z,
                );

                columns.push(generator.column_in(x, z, grid.as_ref()));
            }
        }

        // Шумы пещер — тоже по сетке, до самого высокого столбца чанка.
        let highest = columns
            .iter()
            .map(|column| column.height)
            .max()
            .unwrap_or(MIN_Y);
        let caves = generator.cave_grid(chunk_x * CHUNK_SIZE, chunk_z * CHUNK_SIZE, highest);

        let at =
            |local_x: i32, local_z: i32| columns[((local_x + 1) * SIDE + local_z + 1) as usize];

        for local_x in 0..CHUNK_SIZE {
            for local_z in 0..CHUNK_SIZE {
                let x = chunk_x * CHUNK_SIZE + local_x;
                let z = chunk_z * CHUNK_SIZE + local_z;

                // Крутизна — как у оригинала: южный сосед выше северного
                // на четыре и больше или западный выше восточного.
                let mut column = at(local_x, local_z);
                column.steep = at(local_x, local_z + 1).height - at(local_x, local_z - 1).height
                    >= 4
                    || at(local_x - 1, local_z).height - at(local_x + 1, local_z).height >= 4;

                // Столбец заполняется сверху вниз: так видно, где кончается
                // толща камня над блоком, — от её верха считается глубина
                // для травы, земли и песка.
                let top = generator.top_of(&column);
                let mut run_top = top;

                for y in (MIN_Y..=top).rev() {
                    let solid = generator.solid_in(grid.as_ref(), &column, x, y, z);

                    if !solid {
                        run_top = y - 1;
                    }

                    let state =
                        generator.block_with(&column, caves.as_ref(), x, y, z, solid, run_top);

                    if state == AIR {
                        continue;
                    }

                    let (section, inside) = split(y);
                    let index = (local_z * CHUNK_SIZE + local_x) as usize;

                    chunk.filled_section(section)[inside + index] = pack(state);
                }

                // Биом клетки у всех её столбцов один (его решает середина
                // клетки), поэтому решаем его один раз — по середине: и
                // поверхностный, и пещерные под ним.
                if local_x % 4 == 0 && local_z % 4 == 0 {
                    let middle = at(local_x + 2, local_z + 2);
                    let surface = generator.biome_of(&column);
                    // Редкие осенние пятна в обычном лесу — свой номер биома,
                    // чтобы клиент красил листву и траву по-осеннему.
                    let autumn = surface == terrain::Biome::Forest
                        && matches!(generator, Generator::Normal(terrain) if terrain.autumn_at(x + 2, z + 2));
                    let surface = if autumn { terrain::autumn_forest_number() } else { surface.number() as BiomeId };

                    chunk.fill_biome_column(
                        (local_x / 4) as usize,
                        (local_z / 4) as usize,
                        surface,
                        matches!(generator, Generator::Normal(_)).then_some(&middle),
                    );
                }
            }
        }

        // Руда и пятна камня — после рельефа и пещер: им надо знать, где
        // камень и где воздух. Деревья — после руды: им до неё дела нет.
        generator.lay_ores(&mut chunk, chunk_x, chunk_z);
        underground::dig(generator, &mut chunk, chunk_x, chunk_z);
        generator.grow_trees(&mut chunk, chunk_x, chunk_z);
        vegetation::decorate(generator, &mut chunk, chunk_x, chunk_z, &columns);
        ocean::decorate(generator, &mut chunk, chunk_x, chunk_z, &columns);
        cave_biomes::decorate(generator, &mut chunk, chunk_x, chunk_z);

        // Свет — когда все блоки на местах.
        chunk.light_up();

        chunk
    }

    /// Заполняет биомы одного столбца клеток 4×4: у поверхности и выше —
    /// `surface`, в толще — пещерный биом, если его выбирают климат и
    /// глубина под серединой клетки `middle`. Без `middle` пещерных биомов
    /// нет (ровный мир).
    fn fill_biome_column(
        &mut self,
        cell_x: usize,
        cell_z: usize,
        surface: BiomeId,
        middle: Option<&Column>,
    ) {

        for layer in 0..BIOME_LAYERS {
            // Середина клетки по высоте.
            let y = MIN_Y + layer as i32 * 4 + 2;
            let biome = middle
                .and_then(|column| cave_biomes::biome_at(column, y))
                .map_or(surface, |cave| cave.number() as BiomeId);

            self.biomes[biome_cell(layer, cell_x, cell_z)] = biome;
        }
    }

    /// Биом клетки, в которой лежит блок (местные координаты чанка).
    fn biome(&self, local_x: i32, y: i32, local_z: i32) -> BiomeId {
        let layer = ((y - MIN_Y) / 4) as usize;

        self.biomes[biome_cell(layer, (local_x / 4) as usize, (local_z / 4) as usize)]
    }

    /// Ставит блок дерева, если он попал в этот чанк, и не затирает то, что
    /// уже стоит: ствол важнее листвы.
    fn put_generated(&mut self, x: i32, y: i32, z: i32, state: i32, over: bool) {
        if !(0..CHUNK_SIZE).contains(&x) || !(0..CHUNK_SIZE).contains(&z) {
            return;
        }

        if !(MIN_Y..MIN_Y + WORLD_HEIGHT).contains(&y) {
            return;
        }

        let (section, inside) = split(y);
        let index = (z * CHUNK_SIZE + x) as usize;
        let place = &mut self.filled_section(section)[inside + index];

        if over || unpack(*place) == AIR {
            *place = pack(state);
        }
    }

    /// Чанк ровного мира: земля одинаковой высоты по всей площади.
    #[cfg(test)]
    fn flat() -> Self {
        let mut chunk = Self::new();

        for y in MIN_Y..MIN_Y + WORLD_HEIGHT {
            let state = flat::block_at(y);

            if state == AIR {
                continue;
            }

            for index in 0..(CHUNK_SIZE * CHUNK_SIZE) as usize {
                let (section, inside) = split(y);

                chunk.filled_section(section)[inside + index] = pack(state);
            }
        }

        chunk
    }

    /// Состояние блока внутри чанка.
    fn block(&self, x: i32, y: i32, z: i32) -> i32 {
        let (section, inside) = split(y);

        match &self.sections[section] {
            Some(blocks) => unpack(blocks[inside + corner(x, z)]),
            None => AIR,
        }
    }

    /// Ставит блок. Возвращает false, если он уже был таким.
    fn set(&mut self, x: i32, y: i32, z: i32, state: i32) -> bool {
        let (section, inside) = split(y);
        let index = inside + corner(x, z);

        // Тот же блок ставить незачем, а в воздушную секцию воздух — тем
        // более: заводить её ради этого не надо.
        if self.block(x, y, z) == state {
            return false;
        }

        self.filled_section(section)[index] = pack(state);
        true
    }

    /// Верх самой высокой непустой секции: выше него в чанке только воздух.
    fn highest(&self) -> i32 {
        match self.sections.iter().rposition(Option::is_some) {
            Some(section) => MIN_Y + (section as i32 + 1) * 16 - 1,
            None => MIN_Y - 1,
        }
    }

    /// Секция, готовая к записи: воздушная заводится на месте.
    ///
    /// Всякая правка блоков идёт через неё, поэтому здесь же растёт номер
    /// правки: собственный свет чанка с этого мгновения устарел.
    fn filled_section(&mut self, section: usize) -> &mut [Packed] {
        self.revision = self.revision.wrapping_add(1);

        let blocks = self.sections[section].get_or_insert_with(|| Arc::from([pack(AIR); SECTION_BLOCKS]));

        Arc::make_mut(blocks)
    }

    /// Сколько в чанке блоков, отличных от воздуха.
    #[cfg(test)]
    fn not_air(&self) -> usize {
        self.sections
            .iter()
            .flatten()
            .map(|blocks| blocks.iter().filter(|state| unpack(**state) != AIR).count())
            .sum()
    }

    /// Блоки секции. None — секция целиком из воздуха.
    fn section(&self, section: usize) -> Option<&[Packed]> {
        self.sections[section].as_deref()
    }

    /// Места, куда вода и лава должны потечь сразу: пустые клетки (или
    /// рыхлый снег, или другая жидкость) сбоку и снизу от них. Такт
    /// жидкости решает, чем станет само место, поэтому будить надо не
    /// источник — он и так останется источником, — а соседнюю пустоту: она
    /// по такту станет текущей водой. Так у оригинала подземные водопады и
    /// лава в пещерах текут с первого мгновения.
    ///
    /// Соседи за краем чанка тоже попадают в список: если соседа ещё нет,
    /// такт подождёт его. Координаты мировые, вместе с задержкой такта.
    fn fluids_to_wake(&self, chunk_x: i32, chunk_z: i32) -> Vec<((i32, i32, i32), u64)> {
        let mut found = Vec::new();
        // Состояния воды и лавы идут подряд: жидкость узнаётся сравнением
        // номера, без разбора каждого блока.
        let (water, lava) = (fluids::Kind::Water.states(), fluids::Kind::Lava.states());
        let liquid = |state: i32| (water.0..=water.1).contains(&state) || (lava.0..=lava.1).contains(&state);

        for (section, blocks) in self.sections.iter().enumerate() {
            let Some(blocks) = blocks else {
                continue;
            };

            for (index, packed) in blocks.iter().enumerate() {
                if !liquid(unpack(*packed)) {
                    continue;
                }

                let Some((kind, _)) = fluids::fluid_at(unpack(*packed)) else {
                    continue;
                };

                let (x, z) = ((index % 16) as i32, (index / 16 % 16) as i32);
                let y = MIN_Y + section as i32 * 16 + (index / 256) as i32;
                let open = |state: i32| match fluids::fluid_at(state) {
                    Some((other, _)) => other != kind,
                    None => fluids::can_flow_into(state),
                };

                for (dx, dy, dz) in [(0, -1, 0), (-1, 0, 0), (1, 0, 0), (0, 0, -1), (0, 0, 1)] {
                    let (nx, ny, nz) = (x + dx, y + dy, z + dz);

                    if ny < MIN_Y {
                        continue;
                    }

                    // Соседа за краем чанка отсюда не видно: край проверит
                    // мир, когда чанк ляжет рядом с соседом (`wake_border`).
                    let inside = (0..16).contains(&nx) && (0..16).contains(&nz);

                    if inside && open(self.block(nx, ny, nz)) {
                        found.push(((chunk_x * 16 + nx, ny, chunk_z * 16 + nz), kind.delay()));
                    }
                }
            }
        }

        found.sort_unstable();
        found.dedup();
        found
    }

}

impl Chunk {
    /// Записывает чанк в байты для файла региона.
    ///
    /// Записывается не каждый блок по отдельности, а «состояние и сколько его
    /// подряд»: у ровной земли в секции всего несколько таких записей вместо
    /// четырёх тысяч блоков.
    fn to_bytes(&self) -> Vec<u8> {
        let mut out = vec![CHUNK_VERSION];

        for section in &self.sections {
            let Some(blocks) = section else {
                out.push(0); // секция целиком из воздуха
                continue;
            };

            out.push(1);

            let runs = runs_of(blocks);

            out.extend_from_slice(&(runs.len() as u16).to_be_bytes());

            for (state, length) in runs {
                out.extend_from_slice(&state.to_be_bytes());
                out.extend_from_slice(&length.to_be_bytes());
            }
        }

        // Биомы идут в конце — так же, «номер и сколько его подряд», в
        // порядке клеток: у чанка без пещерных биомов это одна-две записи.
        let runs = runs_of(&self.biomes);

        out.extend_from_slice(&(runs.len() as u16).to_be_bytes());

        for (biome, length) in runs {
            out.extend_from_slice(&biome.to_be_bytes());
            out.extend_from_slice(&length.to_be_bytes());
        }

        out
    }

    /// Читает чанк из байтов файла региона.
    fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let version = *bytes.first()?;
        let wide = version == CHUNK_VERSION_WIDE;

        if version != CHUNK_VERSION && version != CHUNK_VERSION_FLAT_BIOMES && !wide {
            return None;
        }

        let mut reader = Reader::new(bytes.get(1..)?);
        let mut chunk = Chunk::new();

        for section in 0..SECTIONS as usize {
            if reader.u8().ok()? == 0 {
                continue;
            }

            let runs = reader.u16().ok()?;
            let blocks = chunk.filled_section(section);

            let mut at = 0usize;

            for _ in 0..runs {
                let state = if wide {
                    pack(reader.i32().ok()?)
                } else {
                    reader.u16().ok()?
                };

                let length = reader.u16().ok()? as usize;

                for _ in 0..length {
                    *blocks.get_mut(at)? = state;
                    at += 1;
                }
            }
        }

        if version == CHUNK_VERSION {
            // Биомы объёмные — записи «номер и сколько подряд». Они обязаны
            // покрыть все клетки ровно: иначе запись испорчена.
            let runs = reader.u16().ok()?;
            let mut at = 0usize;

            for _ in 0..runs {
                let biome = reader.u16().ok()?;
                let length = reader.u16().ok()? as usize;

                chunk.biomes.get_mut(at..at + length)?.fill(biome);
                at += length;
            }

            if at != chunk.biomes.len() {
                return None;
            }

            return Some(chunk);
        }

        // Прежние записи: биом один на весь столбец клетки 4×4, по два
        // байта на клетку. Его может и не быть: чанк записан сервером, когда
        // биом был один на весь мир. Тогда остаются нули — это первый биом
        // реестра, и мир от этого не пропадёт.
        for cell in 0..BIOME_LAYER {
            let Ok(biome) = reader.u16() else {
                break;
            };

            for layer in 0..BIOME_LAYERS {
                chunk.biomes[layer * BIOME_LAYER + cell] = biome;
            }
        }

        Some(chunk)
    }
}

/// Разбивает блоки секции (или биомы чанка) на записи «значение и сколько
/// его подряд».
fn runs_of(blocks: &[u16]) -> Vec<(u16, u16)> {
    let mut runs: Vec<(u16, u16)> = Vec::new();

    for state in blocks {
        match runs.last_mut() {
            // Подряд одного значения не может быть больше, чем блоков
            // в секции или клеток биома в чанке, так что счётчик не
            // переполнится.
            Some((last, length)) if last == state => *length += 1,
            _ => runs.push((*state, 1)),
        }
    }

    runs
}

/// Секция, в которой лежит эта высота, и начало её слоя внутри секции.
fn split(y: i32) -> (usize, usize) {
    let from_bottom = (y - MIN_Y) as usize;
    let height = CHUNK_SIZE as usize;

    (
        from_bottom / height,
        (from_bottom % height) * (CHUNK_SIZE * CHUNK_SIZE) as usize,
    )
}

/// Место блока в слое секции.
fn corner(x: i32, z: i32) -> usize {
    let local_x = x.rem_euclid(CHUNK_SIZE) as usize;
    let local_z = z.rem_euclid(CHUNK_SIZE) as usize;

    local_z * CHUNK_SIZE as usize + local_x
}

/// Приоритет жидкостей в очереди: после всех блочных тактов, как в игре.
const FLUID_PRIORITY: i8 = 1;

/// Какой это такт: блочный (редстоун) или жидкостный. В игре это две
/// разные очереди, и блочные идут первыми.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum TickKind {
    Block,
    Fluid,
}

/// Ключ ожидающего такта: место и вид такта.
type Waiting = ((i32, i32, i32), TickKind);

/// Запись в очереди запланированных тактов.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Scheduled {
    due: u64,
    priority: i8,
    order: u64,
    place: (i32, i32, i32),
    kind: TickKind,
}

/// Директория с регионами внутри директории мира. Название как в оригинале.
const REGIONS: &str = "region";

/// Версия записи чанка. Меняется, если меняется раскладка байтов или само
/// устройство мира, чтобы старая запись не была прочитана неправильно.
///
/// С третьей версии биомы объёмные: клетка 4×4×4, записаны «номер и сколько
/// подряд».
const CHUNK_VERSION: u8 = 3;

/// Вторая версия: состояния по два байта, биом один на столбец клетки 4×4.
/// Такие чанки читаются, их биом растягивается на всю высоту.
const CHUNK_VERSION_FLAT_BIOMES: u8 = 2;

/// Первый вид записи чанка: состояния по четыре байта, биомы как во второй.
/// Такие файлы мы ещё читаем — у кого-то мир записан ими, и терять его
/// незачем.
const CHUNK_VERSION_WIDE: u8 = 1;

/// Мир целиком: чанки и список изменений.
pub struct World {
    /// Состояние генератора случайных чисел — см. `random_unit`.
    noise: u32,

    /// Места, о которых клиентам скажут позже: (место, такт, как есть).
    /// «Как есть» — служебный блок не подменяется пустотой.
    late: Vec<((i32, i32, i32), u64, bool)>,

    chunks: HashMap<(i32, i32), Chunk>,

    /// Все события мира по порядку: изменения блоков и действия блоков.
    /// Каждое подключение помнит, сколько записей оно уже разослало своему
    /// клиенту, и досылает остальные. Так изменение, сделанное одним
    /// игроком, доходит до остальных.
    events: Journal<WorldEvent>,

    /// Места, о которых клиентам скажут позже, и когда именно.
    ///

    /// Такты, чей срок пришёл, пока их чанк был выгружен. Ждут, когда чанк
    /// прочитают снова, — как в игре, где такты хранятся вместе с чанком.
    /// Выбросить их нельзя: без них схема в этом чанке умерла бы (MC-711).
    parked: Vec<Scheduled>,


    /// Файл, в котором мир лежит на диске.
    path: PathBuf,

    /// Сколько тактов мир уже прожил.
    ///
    /// Такт — это шаг времени мира, примерно двадцатая доля секунды. По нему
    /// считается всё, что происходит само: пока это только течение жидкостей.
    tick: u64,

    /// Запланированные такты: места, которые надо пересчитать, и когда.
    ///
    /// Очередь упорядочена по сроку, потом по приоритету (меньше — раньше),
    /// потом по порядку записи. Так требует редстоун: повторители, смотрящие
    /// в другие повторители, обязаны сработать раньше остальных, иначе схемы
    /// ведут себя по-разному при одинаковой постройке.
    scheduled: BinaryHeap<Reverse<Scheduled>>,

    /// Что сейчас ждёт своего такта и под каким номером записи. По этому
    /// видно, ждёт ли место уже, и отличается свежая запись от устаревшей:
    /// перезаписанная на более ранний срок старая остаётся в очереди и просто
    /// пропускается.
    ///
    /// Блочные и жидкостные такты ждут порознь: у одного места может быть
    /// и тот и другой, как в игре, — иначе запланированная вода мешала бы
    /// повторителю сработать вовремя.
    waiting: HashMap<Waiting, (u64, u64)>,

    /// Номер следующей записи в очереди — для порядка среди равных.
    next_order: u64,

    /// Блоки, изменившиеся с прошлого разбора, и что в них стояло раньше:
    /// редстоун сообщает об изменении соседям, а по прежнему блоку понимает,
    /// кого ещё надо предупредить, когда деталь сняли. Складываются стопкой,
    /// чтобы разбор шёл вглубь — так же, как это делает игра.
    changed: Vec<((i32, i32, i32), i32)>,

    /// Места, из которых блок должен свалиться: под ним пусто.
    /// Сам мир сущностей не заводит, поэтому этим займётся такт.
    falling: Vec<(i32, i32, i32)>,

    /// Насколько часы мира переведены относительно его возраста.
    time_offset: i64,

    /// Семя мира: из него складывается рельеф. Живёт в level.dat, чтобы мир
    /// оставался тем же и после перезапуска.
    seed: i64,

    /// Тип мира: обычный, суперплоский или «Супер плавность». Живёт
    /// в level.dat рядом с семенем.
    kind: WorldKind,

    /// Правила, по которым складывается ещё не сложенный кусок мира.
    /// Под Arc: сеть берёт их себе и складывает чанк, не держа замок мира.
    generator: Arc<Generator>,

    /// Блочные события: то, что делается не в свой запланированный такт,
    /// а отдельной фазой в конце такта мира. Так устроен ход поршня.
    block_events: Vec<(i32, i32, i32)>,

    /// Память редстоуна: то, что не помещается в состояние блока.
    pub redstone: redstone::Memory,

    /// Блоки, сломанные не игроком, а самим миром: поршень раздавил траву,
    /// у провода убрали опору. Из них должны выпасть предметы, но заводить
    /// их здесь нечем — этим займётся такт.
    destroyed: Vec<((i32, i32, i32), i32)>,

    /// Чанки, изменившиеся с последней записи на диск.
    ///
    /// Записывать файл на каждый блок нельзя: он пишется целиком, а вода за
    /// один такт меняет десятки блоков — сервер только и делал бы, что писал.
    /// Поэтому то, что меняется само, копится и пишется раз в секунду, а
    /// сделанное игроком записывается сразу же: терять постройки нельзя.
    ///
    /// Когда мир будет храниться по регионам, запись подешевеет, и копить
    /// станет незачем.
    dirty: HashSet<(i32, i32)>,
}

impl World {
    /// Пустой мир без файла на диске — только для проверок.
    #[cfg(test)]
    pub fn in_memory() -> Self {
        Self {
            noise: 0x9E37_79B9,
            late: Vec::new(),
            chunks: HashMap::new(),
            events: Journal::new(),
            parked: Vec::new(),
            path: PathBuf::new(),
            tick: 0,
            scheduled: BinaryHeap::new(),
            waiting: HashMap::new(),
            next_order: 0,
            changed: Vec::new(),
            falling: Vec::new(),
            time_offset: 0,
            seed: 0,
            kind: WorldKind::Flat,
            generator: Arc::new(Generator::Flat),
            block_events: Vec::new(),
            redstone: redstone::Memory::new(),
            destroyed: Vec::new(),
            dirty: HashSet::new(),
        }
    }

    /// Открывает мир: готовит директорию, в которой лежат регионы.
    ///
    /// Сами чанки не читаются: каждый читается тогда, когда до него дошли.
    pub fn open(
        directory: impl AsRef<Path>,
        wanted_seed: Option<i64>,
        wanted_kind: WorldKind,
    ) -> io::Result<Self> {
        let path = directory.as_ref().to_path_buf();

        fs::create_dir_all(path.join(REGIONS))?;

        // Мир прежнего образца лежал одним файлом. Читать его нечем, и
        // устройство мира с тех пор поменялось, поэтому просто откладываем
        // его в сторону.
        let single_file = path.join("blocks.mcrw");

        if single_file.exists() {
            let old = single_file.with_extension("old");

            log_warn!(
                "Мир прежнего образца отложен: {}. Новый мир хранится по регионам в {}",
                old.display(),
                path.join(REGIONS).display()
            );

            let _ = fs::rename(&single_file, &old);
        }

        log_info!("Подготовка мира \"{}\"", path.display());
        log_debug!("Мир: регионы в {}", path.join(REGIONS).display());

        // Сколько мир уже прожил, который в нём час и что ждёт своего
        // такта — из файла мира. Нет файла — мир новый, время утреннее.
        let level = read_level(&path.join(LEVEL));
        let (tick, time_offset) = (level.age, level.time_offset);

        // Семя: у сложенного мира своё, записанное в level.dat, и менять его
        // нельзя — иначе новые чанки не сойдутся со старыми. У нового мира
        // семя берётся из настроек, а если там пусто — выбирается само.
        let seed = match (level.seed, wanted_seed) {
            (Some(saved), wanted) => {
                if let Some(wanted) = wanted
                    && wanted != saved
                {
                    log_warn!(
                        "Семя мира из настроек ({}) не применяется: у мира уже своё ({})",
                        wanted,
                        saved
                    );
                }

                saved
            }
            (None, Some(wanted)) => wanted,
            (None, None) => random_seed(),
        };

        log_info!("Семя мира: {}", seed);

        // Тип мира — так же, как семя: у сложенного мира свой, записанный,
        // и его нельзя сменить правкой настроек, иначе новые чанки не
        // сойдутся со старыми.
        let kind = match level.kind {
            Some(saved) => {
                if saved != wanted_kind {
                    log_warn!(
                        "Тип мира из настроек ({}) не применяется: у мира уже свой ({})",
                        wanted_kind.name(),
                        saved.name()
                    );
                }

                saved
            }
            None => wanted_kind,
        };

        log_info!("Тип мира: {}", kind.name());

        let mut world = Self {
            noise: 0x9E37_79B9 ^ std::process::id(),
            late: Vec::new(),
            chunks: HashMap::new(),
            events: Journal::new(),
            parked: Vec::new(),
            path,
            tick,
            scheduled: BinaryHeap::new(),
            waiting: HashMap::new(),
            next_order: 0,
            changed: Vec::new(),
            falling: Vec::new(),
            time_offset,
            seed,
            kind,
            generator: Arc::new(match kind {
                WorldKind::Flat => Generator::Flat,
                WorldKind::Normal => Generator::Normal(Terrain::new(seed, Style::Vanilla)),
                WorldKind::SuperSmooth => Generator::Normal(Terrain::new(seed, Style::Smooth)),
            }),
            block_events: Vec::new(),
            redstone: redstone::Memory::new(),
            destroyed: Vec::new(),
            dirty: HashSet::new(),
        };

        if !level.waiting.is_empty() {
            log_debug!("Мир: ждут своего такта мест — {}", level.waiting.len());
        }

        for (place, kind, delay, priority) in level.waiting {
            let due = world.tick + delay;

            world.push_scheduled(place, kind, due, priority);
        }

        // Ходы поршней, начатые до остановки сервера.
        for (pos, extending, facing) in level.moving {
            let Some(facing) = redstone::Dir::by_number(facing) else {
                continue;
            };

            let cargo = level
                .cargo
                .iter()
                .filter(|(piston, _, _)| *piston == pos)
                .map(|(_, place, state)| (*place, *state))
                .collect();

            world
                .redstone
                .restore_moving(pos, redstone::Moving::restored(pos, extending, facing, cargo));
        }

        Ok(world)
    }

    /// Записывает изменившиеся чанки на диск.
    ///
    /// Пишутся только те чанки, которых коснулись, и каждый — в свой регион.
    /// Поэтому запись стоит столько же и в маленьком мире, и в большом.
    pub fn save_if_needed(&mut self) {
        if self.path.as_os_str().is_empty() {
            self.dirty.clear();
            return;
        }

        let directory = self.path.join(REGIONS);

        for (chunk_x, chunk_z) in std::mem::take(&mut self.dirty) {
            let Some(chunk) = self.chunks.get(&(chunk_x, chunk_z)) else {
                continue;
            };

            if let Err(e) = region::write(&directory, chunk_x, chunk_z, &chunk.to_bytes()) {
                log_error!("Не удалось сохранить чанк {} {}: {}", chunk_x, chunk_z, e);

                // Не записалось — пусть попробует записаться в следующий раз.
                self.dirty.insert((chunk_x, chunk_z));
            }
        }

        // Файл мира — после чанков, а не до. В нём записаны начатые ходы
        // поршней, а едущие блоки уже сняты с мест в чанках; если сервер
        // прервут между двумя записями, пусть лучше пропадёт запись о ходе,
        // чем в чанках останутся старые блоки, а ход доиграется поверх них.
        // Время идёт всегда, поэтому пишется и без изменённых чанков.
        self.save_level();
    }

    /// Состояние блока по мировым координатам. Всё, что не задано, — воздух.
    pub fn get_block(&self, x: i32, y: i32, z: i32) -> i32 {
        // Выше неба и ниже дна блоков нет. Спрашивают об этом постоянно —
        // поршень у потолка, провод у дна, — и отвечать надо спокойно,
        // а не падать.
        if !in_bounds(y) {
            return AIR;
        }

        let Some(chunk) = self.chunks.get(&chunk_key(x, z)) else {
            return AIR;
        };

        chunk.block(x, y, z)
    }

    /// Ставит блок и запоминает изменение для рассылки.
    /// Возвращает false, если блок уже был таким.
    pub fn set_block(&mut self, x: i32, y: i32, z: i32, state: i32) -> bool {
        let old = self.get_block(x, y, z);
        let changed = self.set_block_silently(x, y, z, state);

        if changed {
            self.events.push(WorldEvent::Change(BlockChange { x, y, z, state, direct: false }));

            // Соседей надо пересчитать: рядом могла стоять вода, которой
            // теперь есть куда течь, — или наоборот, её путь перекрыли.
            self.schedule_around(x, y, z, state);

            // А редстоун узнает об изменении при ближайшем разборе.
            self.changed.push(((x, y, z), old));

            // На диск это попадёт вместе с остальными изменениями: запись
            // идёт раз в секунду, а не на каждый блок.
            self.dirty.insert(chunk_key(x, z));
        }

        changed
    }

    /// Ставит блок, не запоминая изменение: для начальной расстановки, когда
    /// рассылать ещё некому.
    pub fn set_block_silently(&mut self, x: i32, y: i32, z: i32, state: i32) -> bool {
        if !in_bounds(y) {
            return false;
        }

        self.chunks
            .entry(chunk_key(x, z))
            .or_insert_with(Chunk::new)
            .set(x, y, z, state)
    }


    /// Ставит блок, ничего не сообщая клиентам.
    ///
    /// Нужно для служебных блоков, о которых клиент знать не должен:
    /// «движущийся поршень» сервер держит у себя, чтобы помнить, что куда
    /// едет, а клиент рисует движение сам.
    pub fn set_block_quietly(&mut self, x: i32, y: i32, z: i32, state: i32) -> bool {
        let old = self.get_block(x, y, z);
        let changed = self.set_block_silently(x, y, z, state);

        if changed {
            self.schedule_around(x, y, z, state);
            self.changed.push(((x, y, z), old));
            self.dirty.insert(chunk_key(x, z));
        }

        changed
    }




    /// Помечает чанк изменившимся — для проверок, которые правят блоки
    /// в обход обычного пути.
    #[cfg(test)]
    pub fn mark_dirty_for_tests(&mut self, chunk_x: i32, chunk_z: i32) {
        self.dirty.insert((chunk_x, chunk_z));
    }

    /// Блоки одной секции чанка в порядке пакета чанка.
    ///
    /// None — секции нет вовсе: там воздух. Клиенту такая секция и
    /// отправляется как пустая.
    pub fn section_blocks(&self, chunk_x: i32, chunk_z: i32, section: usize) -> Option<&[Packed]> {
        self.chunks
            .get(&(chunk_x, chunk_z))
            .and_then(|chunk| chunk.section(section))
    }

    /// Тело пакета чанка: секции и биомы в том виде, в каком они уходят
    /// клиенту. Само составление передано сюда снаружи — мир не знает, как
    /// устроены пакеты.
    ///
    /// Готовое тело нарочно не запоминается: на дальности в тридцать два
    /// чанка это триста мегабайт памяти, а собирается оно быстрее, чем
    /// уходит по сети (проверено замером — скорость та же).
    pub fn chunk_packet<T>(
        &self,
        chunk_x: i32,
        chunk_z: i32,
        build: impl FnOnce(&[Option<Arc<[Packed]>>], &[BiomeId]) -> T,
    ) -> Option<T> {
        let chunk = self.chunks.get(&(chunk_x, chunk_z))?;

        Some(build(&chunk.sections, &chunk.biomes))
    }

    /// Всё, что нужно, чтобы посчитать свет чанка вне захвата мира: блоки и
    /// собственный свет его и восьми соседей. None — чанка нет в памяти.
    ///
    /// Блоки не копируются: секции общие с миром (см. `Chunk::sections`).
    pub fn light_job(&self, chunk_x: i32, chunk_z: i32) -> Option<light::LightJob> {
        self.chunks.get(&(chunk_x, chunk_z))?;

        Some(light::LightJob {
            chunks: std::array::from_fn(|slot| {
                let place = (chunk_x + slot as i32 % 3 - 1, chunk_z + slot as i32 / 3 - 1);
                let chunk = self.chunks.get(&place)?;

                Some(light::LightInput {
                    place,
                    revision: chunk.revision,
                    sections: chunk.sections.clone(),
                    light: chunk.fresh_light().cloned(),
                })
            }),
        })
    }

    /// Кладёт в чанки пересчитанный собственный свет — если их блоки с тех
    /// пор не менялись.
    pub fn store_light(&mut self, recounted: Vec<light::Recounted>) {
        for fresh in recounted {
            if let Some(chunk) = self.chunks.get_mut(&fresh.place)
                && chunk.revision == fresh.revision
            {
                chunk.light = Some((fresh.revision, fresh.light));
            }
        }
    }

    /// Небесный и блочный свет в блоке — с учётом соседних чанков. Для
    /// проверок: считает свет всего чанка.
    #[cfg(test)]
    pub fn light_at(&mut self, x: i32, y: i32, z: i32) -> (u8, u8) {
        let Some(job) = self.light_job(x.div_euclid(16), z.div_euclid(16)) else {
            return (light::FULL, 0);
        };
        let (light, recounted) = job.finish();

        self.store_light(recounted);
        light.at(x.rem_euclid(16), y, z.rem_euclid(16))
    }

    /// Откуда брать ещё не готовый чанк. Его можно унести с собой: чтение
    /// с диска и складывание нового чанка — работа долгая, и держать на ней
    /// замок мира нельзя, иначе встанет такт.
    pub fn source(&self) -> ChunkSource {
        ChunkSource {
            path: self.path.clone(),
            generator: Arc::clone(&self.generator),
        }
    }

    /// Есть ли этот чанк в памяти.
    pub fn has_chunk(&self, chunk_x: i32, chunk_z: i32) -> bool {
        self.chunks.contains_key(&(chunk_x, chunk_z))
    }

    /// Кладёт готовый чанк в мир. Если его успели положить, пока он
    /// складывался, — оставляем тот, что уже есть.
    pub fn accept(&mut self, chunk_x: i32, chunk_z: i32, ready: ReadyChunk) {
        if self.chunks.contains_key(&(chunk_x, chunk_z)) {
            return;
        }

        self.chunks.insert((chunk_x, chunk_z), ready.chunk);
        self.after_chunk_arrived(chunk_x, chunk_z, ready.from_disk);
        self.wake_fluids(&ready.wake);
        self.wake_border(chunk_x, chunk_z);
    }

    /// Вода и лава на краю чанка, которым есть куда течь в соседний чанк
    /// (или из соседа — в этот): видно, только когда оба лежат в мире.
    fn wake_border(&mut self, chunk_x: i32, chunk_z: i32) {
        let mut places = Vec::new();

        for (dx, dz) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            let (Some(here), Some(there)) =
                (self.chunks.get(&(chunk_x, chunk_z)), self.chunks.get(&(chunk_x + dx, chunk_z + dz)))
            else {
                continue;
            };

            for step in 0..16 {
                // Пара столбцов по обе стороны края.
                let (hx, hz, tx, tz) = match (dx, dz) {
                    (1, _) => (15, step, 0, step),
                    (-1, _) => (0, step, 15, step),
                    (_, 1) => (step, 15, step, 0),
                    _ => (step, 0, step, 15),
                };

                for section in 0..SECTIONS as usize {
                    if here.sections[section].is_none() && there.sections[section].is_none() {
                        continue;
                    }

                    for y in MIN_Y + section as i32 * 16..MIN_Y + section as i32 * 16 + 16 {
                        let (a, b) = (here.block(hx, y, hz), there.block(tx, y, tz));
                        let flows = |from: i32, to: i32| {
                            fluids::fluid_at(from).is_some_and(|(kind, _)| match fluids::fluid_at(to) {
                                Some((other, _)) => other != kind,
                                None => fluids::can_flow_into(to),
                            })
                        };

                        if flows(a, b) {
                            places.push(((
                                (chunk_x + dx) * 16 + tx, y, (chunk_z + dz) * 16 + tz,
                            ), fluids::fluid_at(a).map_or(fluids::WATER_DELAY, |(kind, _)| kind.delay())));
                        }

                        if flows(b, a) {
                            places.push(((
                                chunk_x * 16 + hx, y, chunk_z * 16 + hz,
                            ), fluids::fluid_at(b).map_or(fluids::WATER_DELAY, |(kind, _)| kind.delay())));
                        }
                    }
                }
            }
        }

        self.wake_fluids(&places);
    }

    /// Ставит в очередь на растекание места рядом с водой и лавой.
    fn wake_fluids(&mut self, places: &[((i32, i32, i32), u64)]) {
        for &((x, y, z), delay) in places {
            self.schedule(x, y, z, delay);
        }
    }

    /// Что делается, когда чанк появился в мире: разбудить отложенные такты
    /// и починить прочитанное с диска.
    fn after_chunk_arrived(&mut self, chunk_x: i32, chunk_z: i32, from_disk: bool) {
        let woken: Vec<Scheduled> = self
            .parked
            .extract_if(.., |entry| chunk_key(entry.place.0, entry.place.2) == (chunk_x, chunk_z))
            .collect();

        for entry in woken {
            self.waiting.remove(&(entry.place, entry.kind));
            self.push_scheduled(entry.place, entry.kind, self.tick + 1, entry.priority);
        }

        if from_disk {
            redstone::repair_chunk(self, chunk_x, chunk_z);
        }
    }

    /// Готовит чанк, если его ещё нет: складывает землю ровного мира.
    ///
    /// Чанки появляются по мере надобности — когда игрок до них дошёл, а не
    /// все сразу: мир бесконечен, и заранее его не наготовишь.
    pub fn ensure(&mut self, chunk_x: i32, chunk_z: i32) -> bool {
        if self.chunks.contains_key(&(chunk_x, chunk_z)) {
            return false;
        }

        let (chunk, from_disk) = match self.read_chunk(chunk_x, chunk_z) {
            Some(chunk) => (chunk, true),
            None => (Chunk::generated(&self.generator, chunk_x, chunk_z), false),
        };

        let wake = chunk.fluids_to_wake(chunk_x, chunk_z);

        self.chunks.insert((chunk_x, chunk_z), chunk);

        // Такты, дожидавшиеся этого чанка, возвращаются в очередь, а
        // прочитанное с диска чинится: сервер могли остановить посреди хода
        // поршня. Жидкостям, которым есть куда течь, — свой такт.
        self.after_chunk_arrived(chunk_x, chunk_z, from_disk);
        self.wake_fluids(&wake);
        self.wake_border(chunk_x, chunk_z);

        true
    }

    /// Выгружает из памяти чанки, до которых никому нет дела.
    ///
    /// `keep` отвечает, нужен ли чанк. Всё изменённое перед этим
    /// записывается, поэтому выгрузка ничего не теряет: понадобится — чанк
    /// прочитается с диска заново.
    ///
    /// Возвращает, сколько чанков выгружено.
    pub fn unload_far<F>(&mut self, keep: F) -> usize
    where
        F: Fn(i32, i32) -> bool,
    {
        self.save_if_needed();

        let before = self.chunks.len();

        self.chunks.retain(|(x, z), _| keep(*x, *z));

        // Ждать пересчёта в выгруженных чанках больше некому.
        self.redstone.forget_outside(|x, z| self.chunks.contains_key(&chunk_key(x, z)));

        before - self.chunks.len()
    }

    /// Читает чанк с диска. None — его там нет или он не читается.
    fn read_chunk(&self, chunk_x: i32, chunk_z: i32) -> Option<Chunk> {
        read_chunk_at(&self.path, chunk_x, chunk_z)
    }

    /// События мира, начиная с записи `from` — то, что осталось разослать.
    ///
    /// Возвращает копию, а не срез: рассылка — дело сетевое, и держать мир
    /// захваченным, пока идёт отправка, нельзя.
    pub fn events_since(&mut self, from: usize, reader: journal::Reader) -> (Vec<WorldEvent>, usize) {
        (self.events.since(from, reader), self.events.count())
    }

    /// Только изменения блоков из журнала — для проверок.
    #[cfg(test)]
    pub fn changes_since(&mut self, from: usize, reader: journal::Reader) -> (Vec<BlockChange>, usize) {
        let (events, count) = self.events_since(from, reader);
        let changes = events
            .into_iter()
            .filter_map(|event| match event {
                WorldEvent::Change(change) => Some(change),
                WorldEvent::Action(_) | WorldEvent::Sound(_) => None,
            })
            .collect();

        (changes, count)
    }

    /// Только действия блоков из журнала — для проверок.
    #[cfg(test)]
    pub fn actions_since(&mut self, from: usize, reader: journal::Reader) -> (Vec<BlockAction>, usize) {
        let (events, count) = self.events_since(from, reader);
        let actions = events
            .into_iter()
            .filter_map(|event| match event {
                WorldEvent::Action(action) => Some(action),
                WorldEvent::Change(_) | WorldEvent::Sound(_) => None,
            })
            .collect();

        (actions, count)
    }

    /// Записывает читателя событий: с этого места ему будет досылаться всё
    /// новое, а прочитанное всеми — выбрасываться.
    pub fn watch_events(&mut self, reader: journal::Reader) -> usize {
        self.events.watch(reader)
    }

    /// То же под старым именем — для проверок.
    #[cfg(test)]
    pub fn watch_changes(&mut self, reader: journal::Reader) -> usize {
        self.watch_events(reader)
    }

    /// То же под старым именем — для проверок.
    #[cfg(test)]
    pub fn watch_actions(&mut self, reader: journal::Reader) -> usize {
        self.watch_events(reader)
    }

    /// Сообщает всем клиентам о действии блока.
    pub fn note_action(&mut self, action: BlockAction) {
        self.events.push(WorldEvent::Action(action));
    }

    /// Сообщает всем клиентам о звуке в этом месте.
    pub fn note_sound(&mut self, x: i32, y: i32, z: i32, name: &'static str, volume: f32, pitch: f32) {
        self.events.push(WorldEvent::Sound(WorldSound { x, y, z, name, volume, pitch }));
    }

    /// Случайное число от 0 до 1 — для высоты звуков и прочего, где в игре
    /// тоже случайность. Простой сдвиговый генератор: точность ему не нужна.
    pub fn random_unit(&mut self) -> f32 {
        let mut x = self.noise.max(1);
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.noise = x;
        (x % 10_000) as f32 / 10_000.0
    }

    /// Сообщает клиентам, что стоит в этом месте сейчас, — даже если для
    /// мира это не изменение, — отдельным сообщением, не в пачке: так
    /// оригинал повторяет клиенту блок, по которому тот щёлкнул, и блок
    /// за гранью щелчка.
    pub fn tell_directly(&mut self, x: i32, y: i32, z: i32) {
        self.tell_state(x, y, z, true, false);
    }

    /// Сообщает клиентам, что стоит в этом месте, через столько-то тактов.
    ///
    /// Так уходят блоки, поставленные ходом поршня: у оригинала изменения
    /// этой фазы такта рассылаются только в следующем такте (сверено
    /// чёрным ящиком). Что там стоит, читается в момент отправки.
    pub fn tell_later(&mut self, x: i32, y: i32, z: i32, delay: u64) {
        self.late.push(((x, y, z), self.tick + delay.max(1), false));
    }

    /// То же, но служебный «движущийся поршень» уходит как есть: так
    /// оригинал сообщает о месте сломанного поршнем блока.
    pub fn tell_later_as_is(&mut self, x: i32, y: i32, z: i32, delay: u64) {
        self.late.push(((x, y, z), self.tick + delay.max(1), true));
    }

    fn tell_state(&mut self, x: i32, y: i32, z: i32, direct: bool, as_is: bool) {
        let mut state = self.get_block(x, y, z);

        // Служебный «движущийся поршень» клиенту обычно не показывают:
        // для него это место пустое, и так же говорит игра.
        if !as_is
            && let Some((low, high)) = crate::blocks::states_of("moving_piston")
            && (low..=high).contains(&state)
        {
            state = AIR;
        }

        self.events.push(WorldEvent::Change(BlockChange { x, y, z, state, direct }));
    }

    /// Отписывает читателя: подключение закрылось.
    pub fn forget_reader(&mut self, reader: journal::Reader) {
        self.events.forget(reader);
    }

    /// Просит пересчитать это место через столько-то тактов — для жидкостей.
    ///
    /// Если место уже ждёт, берётся более ранний срок: раз его потревожили
    /// снова, тянуть незачем.
    pub fn schedule(&mut self, x: i32, y: i32, z: i32, delay: u64) {
        let due = self.tick + delay.max(1);

        if let Some((waiting, _)) = self.waiting.get(&((x, y, z), TickKind::Fluid))
            && *waiting <= due
        {
            return;
        }

        self.push_scheduled((x, y, z), TickKind::Fluid, due, FLUID_PRIORITY);
    }

    /// Просит пересчитать место через столько-то тактов с таким приоритетом —
    /// для редстоуна.
    ///
    /// Место может ждать только одного такта: пока он не прошёл, новые
    /// просьбы отбрасываются. Возвращает false, если просьба отброшена.
    /// Так устроено в игре, и от этого зависит поведение повторителей.
    pub fn schedule_once(&mut self, x: i32, y: i32, z: i32, delay: u64, priority: i8) -> bool {
        if self.waiting.contains_key(&((x, y, z), TickKind::Block)) {
            return false;
        }

        self.push_scheduled((x, y, z), TickKind::Block, self.tick + delay.max(1), priority);
        true
    }

    /// Просит пересчитать место, даже если оно уже чего-то ждёт: прежняя
    /// заявка заменяется новой.
    ///
    /// Правило «одна заявка за раз» вики формулирует только для повторителя;
    /// кнопке оно вредит — её отпускание просто пропало бы, и она залипла бы
    /// навсегда.
    pub fn schedule_block(&mut self, x: i32, y: i32, z: i32, delay: u64, priority: i8) {
        self.push_scheduled((x, y, z), TickKind::Block, self.tick + delay.max(1), priority);
    }

    /// Кладёт запись в очередь, помечая прежнюю запись этого места устаревшей.
    fn push_scheduled(&mut self, place: (i32, i32, i32), kind: TickKind, due: u64, priority: i8) {
        let order = self.next_order;
        self.next_order += 1;

        self.waiting.insert((place, kind), (due, order));
        self.scheduled.push(Reverse(Scheduled {
            due,
            priority,
            order,
            place,
            kind,
        }));
    }

    /// Просит пересчитать место и всех его соседей — для жидкостей.
    ///
    /// Задержка зависит от того, лава ли тут замешана: она течёт заметно
    /// медленнее воды.
    fn schedule_around(&mut self, x: i32, y: i32, z: i32, state: i32) {
        let delay = match fluids::fluid_at(state) {
            Some((kind, _)) => kind.delay(),
            None => fluids::WATER_DELAY,
        };

        let places = [
            (x, y, z),
            (x, y + 1, z),
            (x, y - 1, z),
            (x - 1, y, z),
            (x + 1, y, z),
            (x, y, z - 1),
            (x, y, z + 1),
        ];

        for (x, y, z) in places {
            self.schedule(x, y, z, delay);
        }
    }

    /// Двигает время мира на такт вперёд и отдаёт места, которым пора, —
    /// по приоритету, а среди равных по порядку записи. У каждого места
    /// сказано, блочный это такт или жидкостный.
    ///
    /// За один такт мир разбирает не больше `limit` мест: столько же, сколько
    /// в игре. Остальные не пропадают, а остаются ждать следующего такта —
    /// иначе схема, наплодившая много работы, теряла бы часть навсегда.
    pub fn advance(&mut self, limit: usize) -> Vec<((i32, i32, i32), TickKind)> {
        self.tick += 1;

        self.wake_forgotten_moves();

        // Места, о которых пришла пора сказать: берём то, что стоит там
        // сейчас, а не то, что стояло, когда решили сказать.
        let tick = self.tick;
        let late: Vec<((i32, i32, i32), bool)> = self
            .late
            .extract_if(.., |(_, when, _)| *when <= tick)
            .map(|(place, _, as_is)| (place, as_is))
            .collect();

        for ((x, y, z), as_is) in late {
            self.tell_state(x, y, z, false, as_is);
        }

        let mut due = Vec::new();

        while let Some(Reverse(next)) = self.scheduled.peek() {
            if next.due > self.tick || due.len() >= limit {
                break;
            }

            let next = *next;
            self.scheduled.pop();

            // Устаревшая запись: место перезаписали на другой срок.
            if self.waiting.get(&(next.place, next.kind)) != Some(&(next.due, next.order)) {
                continue;
            }

            // Чанк выгружен — такт подождёт, пока его прочитают снова.
            // В `waiting` он остаётся: и чтобы не завёлся второй, и чтобы
            // попасть на диск.
            if !self.chunks.contains_key(&chunk_key(next.place.0, next.place.2)) {
                self.parked.push(next);
                continue;
            }

            self.waiting.remove(&(next.place, next.kind));
            due.push((next.place, next.kind));
        }

        due
    }

    /// Ждёт ли это место своего блочного такта.
    fn has_block_tick(&self, place: (i32, i32, i32)) -> bool {
        self.waiting.contains_key(&(place, TickKind::Block))
            || self
                .parked
                .iter()
                .any(|entry| entry.place == place && entry.kind == TickKind::Block)
    }

    /// Возвращает в очередь ходы поршней, о которых напомнить больше некому.
    ///
    /// Каждый начатый ход доигрывает свой запланированный такт. Если такт
    /// потерялся — мир прочитан без него, — ход завис бы навсегда: едущие
    /// блоки сняты со своих мест, а на местах назначения стоит служебный
    /// «движущийся поршень». Для игрока это выглядит так, что у поршня
    /// пропала голова, а блок на нём стал невидимым, и починить это в игре
    /// нельзя. Поэтому раз в такт проверяем: у каждого хода такт на месте.
    fn wake_forgotten_moves(&mut self) {
        let moves: Vec<(i32, i32, i32)> = self
            .redstone
            .moving_pistons()
            .map(|(place, _)| *place)
            .collect();

        for place in moves {
            if !self.has_block_tick(place) {
                self.push_scheduled(place, TickKind::Block, self.tick + 1, 0);
            }
        }
    }

    /// Просит выполнить блочное событие в этом месте — в конце такта мира.
    ///
    /// Поршень трогается не в свой запланированный такт, а позже, отдельной
    /// фазой: в игре его ход — это «block event», и идёт он после всех
    /// запланированных тактов. От этого зависят тайминги схем.
    pub fn queue_block_event(&mut self, x: i32, y: i32, z: i32) {
        let place = (x, y, z);

        if !self.block_events.contains(&place) {
            self.block_events.push(place);
        }
    }

    /// Забирает накопившиеся блочные события.
    ///
    /// Забирается только то, что накопилось к этому мгновению: события,
    /// заведённые во время разбора, ждут следующего такта — как в игре.
    pub fn take_block_events(&mut self) -> Vec<(i32, i32, i32)> {
        std::mem::take(&mut self.block_events)
    }

    /// Записывает возраст мира, часы и то, что ждёт своего такта.
    ///
    /// Без запланированных тактов после перезапуска встал бы весь редстоун,
    /// который держится на времени: часы на факелах, повторители в середине
    /// задержки, начатый ход поршня.
    fn save_level(&self) {
        let mut text = format!(
            "age = {}\ntime-offset = {}\nseed = {}\ntype = {}\n",
            self.tick,
            self.time_offset,
            self.seed,
            self.kind.name()
        );

        for ((place, kind), (due, _)) in &self.waiting {
            let kind = match kind {
                TickKind::Block => "block",
                TickKind::Fluid => "fluid",
            };

            // Записываем не срок, а сколько ждать: возраст мира при чтении
            // тот же, но так запись понятнее и не зависит от него.
            let delay = due.saturating_sub(self.tick).max(1);

            // Приоритет тоже записываем: от него зависит, кто сработает
            // раньше при одинаковом сроке, а на этом держатся схемы.
            let priority = self
                .scheduled
                .iter()
                .find(|Reverse(entry)| entry.place == *place && entry.due == *due)
                .map(|Reverse(entry)| entry.priority)
                .unwrap_or(0);

            text.push_str(&format!(
                "tick = {} {} {} {} {} {}\n",
                place.0, place.1, place.2, kind, delay, priority
            ));
        }

        // Начатые ходы поршней: без них едущий блок пропал бы совсем —
        // с мест он уже снят, а куда ехал, знает только память.
        for (pos, moving) in self.redstone.moving_pistons() {
            text.push_str(&format!(
                "moving = {} {} {} {} {}\n",
                pos.0,
                pos.1,
                pos.2,
                u8::from(moving.extending),
                moving.facing.number()
            ));

            for (place, state) in &moving.cargo {
                text.push_str(&format!(
                    "cargo = {} {} {} {} {} {} {}\n",
                    pos.0, pos.1, pos.2, place.0, place.1, place.2, state
                ));
            }
        }

        if let Err(error) = fs::write(self.path.join(LEVEL), text) {
            log_error!("Не удалось записать {}: {}", LEVEL, error);
        }
    }

    /// Который сейчас час в мире: такты суток, от 0 до 23 999.
    ///
    /// Сутки — 24 000 тактов. Отдельно от возраста мира: время можно
    /// перевести командой, а возраст идёт своим ходом.
    pub fn time_of_day(&self) -> i64 {
        (self.tick as i64 + self.time_offset).rem_euclid(DAY_LENGTH)
    }

    /// Переводит часы мира.
    pub fn set_time_of_day(&mut self, time: i64) {
        self.time_offset = time.rem_euclid(DAY_LENGTH) - self.tick as i64;
    }

    /// Который сейчас такт мира.
    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// Изменившиеся блоки, которые редстоун ещё не разобрал, и что там
    /// стояло раньше: последний изменившийся — первым.
    pub fn pop_changed(&mut self) -> Option<((i32, i32, i32), i32)> {
        self.changed.pop()
    }

    /// Забирает все изменившиеся блоки разом, в том же порядке, в каком их
    /// выдавал бы `pop_changed`. Что изменится, пока этот список разбирают,
    /// ляжет в очередь заново и дождётся следующего раза.
    pub fn take_changed(&mut self) -> Vec<((i32, i32, i32), i32)> {
        let mut taken = std::mem::take(&mut self.changed);

        taken.reverse();
        taken
    }

    /// Запоминает, что блок сломал сам мир: из него должен выпасть предмет.
    pub fn note_destroyed(&mut self, x: i32, y: i32, z: i32) {
        let state = self.get_block(x, y, z);

        if state != AIR {
            self.destroyed.push(((x, y, z), state));
        }
    }

    /// Запоминает, что блоку не на чем держаться и он должен упасть.
    pub fn note_falling(&mut self, x: i32, y: i32, z: i32) {
        let place = (x, y, z);

        if !self.falling.contains(&place) {
            self.falling.push(place);
        }
    }

    /// Забирает места, откуда пора падать.
    pub fn take_falling(&mut self) -> Vec<(i32, i32, i32)> {
        std::mem::take(&mut self.falling)
    }

    /// Забирает сломанное миром — чтобы такт уронил предметы.
    pub fn take_destroyed(&mut self) -> Vec<((i32, i32, i32), i32)> {
        std::mem::take(&mut self.destroyed)
    }

    /// Отмечает место изменившимся, хотя блок в нём тот же: у сравнителя
    /// может смениться сила на выходе, а состояние блока — нет.
    pub fn note_changed(&mut self, x: i32, y: i32, z: i32) {
        let state = self.get_block(x, y, z);

        self.changed.push(((x, y, z), state));
    }

    /// Сколько мест ждут пересчёта — по этому видно, что мир не «кипит»:
    /// когда всё утекло, ждать нечему. Нужно для проверок.
    #[cfg(test)]
    pub fn scheduled_count(&self) -> usize {
        self.waiting.len()
    }
}

/// Читает чанк с диска по пути мира. None — его там нет или он не читается.
///
/// Свободная функция, а не метод: читать чанк нужно и тогда, когда мира под
/// рукой нет — например, пока он занят тактом.
fn read_chunk_at(path: &Path, chunk_x: i32, chunk_z: i32) -> Option<Chunk> {
    if path.as_os_str().is_empty() {
        return None;
    }

    let directory = path.join(REGIONS);

    match region::read(&directory, chunk_x, chunk_z) {
        Ok(Some(bytes)) => match Chunk::from_bytes(&bytes) {
            // Свет на диске не хранится: он считается заново по блокам.
            Some(mut chunk) => {
                chunk.light_up();
                Some(chunk)
            }
            None => {
                log_warn!("Чанк {} {} не читается, складываю заново", chunk_x, chunk_z);
                None
            }
        },
        Ok(None) => None,
        Err(e) => {
            log_error!("Не удалось прочитать чанк {} {}: {}", chunk_x, chunk_z, e);
            None
        }
    }
}

/// Сколько блоков над землёй может занять растительность: кактус — самый
/// высокий из того, что мы сажаем.
const PLANT_ROOM: i32 = 4;

/// Готовый чанк, который осталось положить в мир.
pub struct ReadyChunk {
    chunk: Chunk,
    from_disk: bool,
    /// Места рядом с водой и лавой, которые надо сразу пустить в такт:
    /// найдены заранее, вне захвата мира.
    wake: Vec<((i32, i32, i32), u64)>,
}

/// Откуда берутся ещё не готовые чанки: папка мира и правила складывания.
///
/// Живёт отдельно от мира нарочно: пока чанк читается с диска или
/// складывается заново, замок мира держать нельзя — такт не должен ждать.
pub struct ChunkSource {
    path: PathBuf,
    generator: Arc<Generator>,
}

impl ChunkSource {
    /// Читает чанк с диска, а если его там нет — складывает заново.
    pub fn take(&self, chunk_x: i32, chunk_z: i32) -> ReadyChunk {
        let (chunk, from_disk) = match read_chunk_at(&self.path, chunk_x, chunk_z) {
            Some(chunk) => (chunk, true),
            None => (Chunk::generated(&self.generator, chunk_x, chunk_z), false),
        };
        let wake = chunk.fluids_to_wake(chunk_x, chunk_z);

        ReadyChunk { chunk, from_disk, wake }
    }
}

/// Чем складывается ещё не сложенный кусок мира.
///
/// Разница в размере вариантов не важна: генератор у мира один и живёт
/// в `Arc`, копий не делается.
#[allow(clippy::large_enum_variant)]
enum Generator {
    /// Обычный мир: рельеф, биомы, море.
    Normal(Terrain),

    /// Ровный мир — настройка `level-type=minecraft:flat` у оригинала.
    /// Земля одинаковой высоты по всей площади.
    Flat,
}

impl World {
    /// Где появляется игрок, зашедший впервые.
    ///
    /// В ровном мире это начало координат. В обычном там может оказаться
    /// море, поэтому от начала координат расходимся кругами и ищем первое
    /// место на суше — как делает игра, выбирая точку появления.
    pub fn spawn_position(&self) -> (f64, f64, f64) {
        /// Насколько далеко от начала координат искать сушу.
        const SEARCH: i32 = 3_000;

        /// Через сколько блоков проверять следующее место.
        const STEP: i32 = 16;

        let mut ring = 0;

        while ring * STEP <= SEARCH {
            let side = ring * STEP;

            // Обходим кольцо: его углы и стороны с тем же шагом.
            for step in -ring..=ring {
                let along = step * STEP;

                for (x, z) in [
                    (along, -side),
                    (along, side),
                    (-side, along),
                    (side, along),
                ] {
                    let column = self.generator.column_at(x, z);

                    // Суша выше уровня моря и не голый лёд: на воде и на
                    // отвесных пиках появляться незачем.
                    if column.height >= terrain::SEA && !column.biome.is_ocean() {
                        log_info!(
                            "Точка появления: {} {} {} ({})",
                            x,
                            column.height + 1,
                            z,
                            column.biome.name()
                        );

                        return (x as f64 + 0.5, (column.height + 1) as f64, z as f64 + 0.5);
                    }
                }
            }

            ring += 1;
        }

        // Суши не нашлось — появляемся над водой в начале координат.
        (0.5, (terrain::SEA + 1) as f64, 0.5)
    }
}

impl Generator {
    /// Всё про столбец в этом месте: высота поверхности и биом. Считается
    /// один раз на место — дальше столбец заполняется по нему.
    fn column_at(&self, x: i32, z: i32) -> Column {
        match self {
            Generator::Normal(terrain) => terrain.column_at(x, z),
            Generator::Flat => Column::flat(flat::GROUND),
        }
    }

    /// Сетка плотности для участка — только у «Обычного» мира: остальные
    /// обходятся картой высот.
    fn relief_grid(
        &self,
        x_from: i32,
        z_from: i32,
        x_to: i32,
        z_to: i32,
    ) -> Option<terrain::ReliefGrid> {
        match self {
            Generator::Normal(terrain) if terrain.style() == terrain::Style::Vanilla => {
                Some(terrain.relief_grid(x_from, z_from, x_to, z_to))
            }
            _ => None,
        }
    }

    /// Столбец, когда сетка плотности уже построена.
    fn column_in(&self, x: i32, z: i32, grid: Option<&terrain::ReliefGrid>) -> Column {
        match self {
            Generator::Normal(terrain) => terrain.column_in(x, z, grid),
            Generator::Flat => Column::flat(flat::GROUND),
        }
    }

    /// Камень ли в этом месте по рельефу.
    fn solid_in(
        &self,
        grid: Option<&terrain::ReliefGrid>,
        column: &Column,
        x: i32,
        y: i32,
        z: i32,
    ) -> bool {
        match self {
            Generator::Normal(terrain) => terrain.solid_in(grid, column, x, y, z),
            Generator::Flat => y <= column.height,
        }
    }

    /// Сетка шумов пещер над чанком — где пещеры есть.
    fn cave_grid(&self, x: i32, z: i32, y_to: i32) -> Option<terrain::CaveGrid> {
        match self {
            Generator::Normal(terrain) => {
                Some(terrain.cave_grid(x, z, x + CHUNK_SIZE - 1, z + CHUNK_SIZE - 1, y_to))
            }
            Generator::Flat => None,
        }
    }

    /// Что стоит в этом месте столбца.
    #[allow(clippy::too_many_arguments)]
    fn block_with(
        &self,
        column: &Column,
        caves: Option<&terrain::CaveGrid>,
        x: i32,
        y: i32,
        z: i32,
        solid: bool,
        run_top: i32,
    ) -> i32 {
        match (self, caves) {
            (Generator::Normal(terrain), Some(caves)) => {
                terrain.block_with(column, caves, x, y, z, solid, run_top)
            }
            _ => flat::block_at(y),
        }
    }

    /// Кладёт в чанк руду и пятна земли, гравия, гранита и прочего.
    ///
    /// Где лежат гнёзда, решает рельеф (`Terrain::ore_blocks_in`); здесь —
    /// только можно ли положить блок на это место: гнездо ложится лишь
    /// в камень, глубинный сланец, гранит, диорит, андезит и туф (вики,
    /// «Ore (feature)»), а блок, соседствующий с воздухом, пропускается
    /// с шансом из таблицы — каждый блок отдельно.
    fn lay_ores(&self, chunk: &mut Chunk, chunk_x: i32, chunk_z: i32) {
        let Generator::Normal(terrain) = self else {
            return;
        };

        let stone = terrain::block_named("stone");
        let deepslate = terrain::block_named("deepslate");
        let takes = [
            stone,
            deepslate,
            terrain::block_named("granite"),
            terrain::block_named("diorite"),
            terrain::block_named("andesite"),
            terrain::block_named("tuff"),
        ];
        let water = terrain::block_named("water");
        let lava = terrain::block_named("lava");

        for (x, y, z, placement) in terrain.ore_blocks_in(chunk_x, chunk_z, chunk.highest()) {
            let (local_x, local_z) = (x - chunk_x * CHUNK_SIZE, z - chunk_z * CHUNK_SIZE);

            if !(MIN_Y..MIN_Y + WORLD_HEIGHT).contains(&y) {
                continue;
            }

            let here = chunk.block(local_x, y, local_z);

            if !takes.contains(&here) {
                continue;
            }

            if placement.air_skip > 0.0 {
                // Соседи за краем чанка нам не видны — считаем их камнем.
                let open = [
                    (1, 0, 0),
                    (-1, 0, 0),
                    (0, 1, 0),
                    (0, -1, 0),
                    (0, 0, 1),
                    (0, 0, -1),
                ]
                .iter()
                .any(|(dx, dy, dz)| {
                    let (nx, ny, nz) = (local_x + dx, y + dy, local_z + dz);

                    (0..CHUNK_SIZE).contains(&nx)
                        && (0..CHUNK_SIZE).contains(&nz)
                        && (MIN_Y..MIN_Y + WORLD_HEIGHT).contains(&ny)
                        && [AIR, water, lava].contains(&chunk.block(nx, ny, nz))
                });

                if open && terrain::chance_at(terrain.seed(), x, y, z, placement.air_skip) {
                    continue;
                }
            }

            // Под нулём — в глубинном сланце — у руды своя разновидность.
            let deep = placement.has_deep && (here == deepslate || y < 0);
            let name = if deep {
                format!("deepslate_{}", placement.block)
            } else {
                placement.block.to_string()
            };

            if let Some(state) = crate::blocks::state_by_name(&name) {
                chunk.put_generated(local_x, y, local_z, state, true);
            }
        }
    }

    /// Сажает деревья этого чанка.
    ///
    /// Смотреть приходится шире самого чанка: ствол у соседа, а крона —
    /// у нас. Поэтому обходим полосу вокруг чанка и рисуем то, что попало
    /// внутрь. Соседний чанк посчитает то же самое и нарисует свою часть —
    /// договариваться им не о чем.
    fn grow_trees(&self, chunk: &mut Chunk, chunk_x: i32, chunk_z: i32) {
        const REACH: i32 = terrain::TREE_REACH;

        let Generator::Normal(terrain) = self else {
            return;
        };

        let ground = TreeGround::new();

        for local_x in -REACH..(CHUNK_SIZE + REACH) {
            for local_z in -REACH..(CHUNK_SIZE + REACH) {
                let x = chunk_x * CHUNK_SIZE + local_x;
                let z = chunk_z * CHUNK_SIZE + local_z;

                // Дерево редкое, а столбец считать дорого: сперва дешёвая
                // проверка «а не здесь ли ствол вообще», и только потом сам
                // столбец.
                if !terrain::tree_possible(terrain.seed(), x, z) {
                    continue;
                }

                let column = terrain.column_at(x, z);

                let Some(tree) = terrain.tree_at(&column, x, z) else {
                    continue;
                };

                draw_tree(chunk, &tree, local_x, local_z, column.height, x, z, &ground);
            }
        }
    }

    /// Какой биом у этого столбца.
    fn biome_of(&self, column: &Column) -> terrain::Biome {
        match self {
            Generator::Normal(_) => column.biome,
            Generator::Flat => terrain::Biome::Plains,
        }
    }

    /// До какой высоты имеет смысл считать столбец: выше неё пусто.
    ///
    /// Чуть выше поверхности: там стоят трава, цветы, снег и кактусы.
    fn top_of(&self, column: &Column) -> i32 {
        match self {
            Generator::Normal(_) => column.height.max(terrain::SEA) + PLANT_ROOM,
            Generator::Flat => flat::GROUND,
        }
    }
}

/// Что дерево видит под собой: воду и землю, которую может заменить.
/// Ищется по имени один раз на чанк.
struct TreeGround {
    water: i32,
    /// Земля, которую дерево может заменить подзолом или корнями.
    soil: Vec<(i32, i32)>,
    /// Трава: под нижним бревном она становится простой землёй, прочая
    /// земля — подзол, ил — остаётся как есть.
    grass: Option<(i32, i32)>,
}

impl TreeGround {
    fn new() -> TreeGround {
        TreeGround {
            water: terrain::block_named("water"),
            soil: [
                "grass_block",
                "dirt",
                "coarse_dirt",
                "podzol",
                "rooted_dirt",
                "mud",
            ]
            .iter()
            .filter_map(|name| crate::blocks::states_of(name))
            .collect(),
            grass: crate::blocks::states_of("grass_block"),
        }
    }
}

/// Рисует в чанке ту часть дерева, что попала в него. Ствол стоит в (x, z)
/// — местные (local_x, local_z) — на земле высоты `height`.
#[allow(clippy::too_many_arguments)]
fn draw_tree(
    chunk: &mut Chunk,
    tree: &trees::Tree,
    local_x: i32,
    local_z: i32,
    height: i32,
    x: i32,
    z: i32,
    ground: &TreeGround,
) {
    let TreeGround { water, soil, grass } = ground;
    let (water, grass) = (*water, *grass);

    let by_text = |text: &str| {
        crate::blocks::state_from_text(text)
            .unwrap_or_else(|| terrain::block_named(text))
    };
    let leaves = by_text(tree.leaves);
    // У огромных грибов «ствол» записан полным состоянием, оси
    // у него нет.
    let log = |axis: terrain::Axis| {
        if tree.log.contains('[') {
            return by_text(tree.log);
        }

        let axis = match axis {
            terrain::Axis::X => "x",
            terrain::Axis::Y => "y",
            terrain::Axis::Z => "z",
        };

        crate::blocks::state_from_text(&format!("{}[axis={}]", tree.log, axis))
            .unwrap_or_else(|| terrain::block_named(tree.log))
    };
    let logs = [
        log(terrain::Axis::X),
        log(terrain::Axis::Y),
        log(terrain::Axis::Z),
    ];
    let blocks = tree.blocks(x, z);

    // Земля под деревом — первой: пока над ней не выросли ни
    // корни, ни ствол, её верх виден сразу.
    for (dx, dy, dz, part) in &blocks {
        if let terrain::Part::Ground(text) = part {
            let (bx, bz) = (local_x + dx, local_z + dz);

            if !(0..CHUNK_SIZE).contains(&bx) || !(0..CHUNK_SIZE).contains(&bz) {
                continue;
            }

            let near = height + dy;
            let lowest = (near - 3).max(MIN_Y);
            let highest = (near + 3).min(MIN_Y + WORLD_HEIGHT - 1);

            // Сверху вниз до первого блока, что не воздух и не
            // вода; меняем его, только если это земля.
            let Some(y) = (lowest..=highest)
                .rev()
                .find(|&y| ![AIR, water].contains(&chunk.block(bx, y, bz)))
            else {
                continue;
            };

            let here = chunk.block(bx, y, bz);

            if !soil.iter().any(|(low, high)| (*low..=*high).contains(&here)) {
                continue;
            }

            if *text == "dirt"
                && !grass.is_some_and(|(low, high)| (low..=high).contains(&here))
            {
                continue;
            }

            if let Some(state) = crate::blocks::state_from_text(text) {
                chunk.put_generated(bx, y, bz, state, true);
            }
        }
    }

    // Листва не затирает стволы и ветви, поэтому ставится раньше,
    // а брёвна — поверх неё.
    for (dx, dy, dz, part) in &blocks {
        if *part == terrain::Part::Leaves {
            chunk.put_generated(
                local_x + dx,
                height + dy,
                local_z + dz,
                leaves,
                false,
            );
        }
    }

    for (dx, dy, dz, part) in &blocks {
        if let terrain::Part::Log(axis) = part {
            let state = logs[*axis as usize];

            chunk.put_generated(
                local_x + dx,
                height + dy,
                local_z + dz,
                state,
                true,
            );
        }
    }

    // Лианы, какао, корни — последними и только на свободное
    // место: в воздух, а что бывает затоплено — и в воду.
    for (dx, dy, dz, part) in &blocks {
        let terrain::Part::Block(text) = part else {
            continue;
        };
        let (bx, by, bz) = (local_x + dx, height + dy, local_z + dz);

        if !(0..CHUNK_SIZE).contains(&bx)
            || !(0..CHUNK_SIZE).contains(&bz)
            || !(MIN_Y..MIN_Y + WORLD_HEIGHT).contains(&by)
        {
            continue;
        }

        let here = chunk.block(bx, by, bz);
        let state = if here == AIR {
            crate::blocks::state_from_text(text)
        } else if here == water {
            let wet = match text.strip_suffix(']') {
                Some(open) => format!("{},waterlogged=true]", open),
                None => format!("{}[waterlogged=true]", text),
            };

            crate::blocks::state_from_text(&wet)
        } else {
            None
        };

        if let Some(state) = state {
            chunk.put_generated(bx, by, bz, state, true);
        }
    }
}

/// Случайное семя для нового мира: берём его из часов.
fn random_seed() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_nanos() as i64)
        .unwrap_or(0)
}

/// Имя файла с общими сведениями о мире — как у оригинального сервера.
/// Внутри — наш собственный простой вид, не формат игры.
const LEVEL: &str = "level.dat";

/// Место из файла мира, ждущее своего такта: где, какой такт, через сколько
/// и с каким приоритетом.
type SavedTick = ((i32, i32, i32), TickKind, u64, i8);

/// Едущий блок из файла мира: чей поршень, куда встанет и что это за блок.
type SavedCargo = ((i32, i32, i32), (i32, i32, i32), i32);

/// Что прочиталось из файла мира.
struct Level {
    age: u64,
    time_offset: i64,

    /// Семя мира. `None` — в файле его нет: мир новый.
    seed: Option<i64>,

    /// Тип мира. `None` — в файле его нет: мир новый или записан прежним
    /// сервером, у которого типов не было.
    kind: Option<WorldKind>,

    /// Места, ждущие своего такта.
    waiting: Vec<SavedTick>,

    /// Начатые ходы поршней: где поршень, выдвигается ли и куда смотрит.
    moving: Vec<((i32, i32, i32), bool, u8)>,

    /// Что едет: у какого поршня, куда встанет и что это за блок.
    cargo: Vec<SavedCargo>,
}

/// Читает файл мира. Нет файла или он испорчен — начинаем с нуля: терять
/// из-за этого мир незачем.
fn read_level(path: &Path) -> Level {
    let mut level = Level {
        age: 0,
        time_offset: 0,
        seed: None,
        kind: None,
        waiting: Vec::new(),
        moving: Vec::new(),
        cargo: Vec::new(),
    };

    let Ok(text) = fs::read_to_string(path) else {
        return level;
    };

    for line in text.lines() {
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };

        let value = value.trim();

        match name.trim() {
            "age" => level.age = value.parse().unwrap_or(0),
            "time-offset" => level.time_offset = value.parse().unwrap_or(0),
            "seed" => level.seed = value.parse().ok(),
            "type" => level.kind = Some(WorldKind::from_setting(value)),
            "tick" => {
                if let Some(waiting) = read_waiting(value) {
                    level.waiting.push(waiting);
                }
            }
            "moving" => {
                if let Some(moving) = read_moving(value) {
                    level.moving.push(moving);
                }
            }
            "cargo" => {
                if let Some(cargo) = read_cargo(value) {
                    level.cargo.push(cargo);
                }
            }
            _ => {}
        }
    }

    level
}

/// Разбирает строку «x y z выдвигается сторона».
fn read_moving(value: &str) -> Option<((i32, i32, i32), bool, u8)> {
    let mut parts = value.split_whitespace();

    let x = parts.next()?.parse().ok()?;
    let y = parts.next()?.parse().ok()?;
    let z = parts.next()?.parse().ok()?;
    let extending = parts.next()?.parse::<u8>().ok()? != 0;
    let facing = parts.next()?.parse().ok()?;

    Some(((x, y, z), extending, facing))
}

/// Разбирает строку «поршень x y z место x y z состояние».
fn read_cargo(value: &str) -> Option<SavedCargo> {
    let mut parts = value.split_whitespace();
    let mut number = || parts.next()?.parse::<i32>().ok();

    let piston = (number()?, number()?, number()?);
    let place = (number()?, number()?, number()?);
    let state = number()?;

    Some((piston, place, state))
}

/// Разбирает строку «x y z вид через-сколько приоритет».
fn read_waiting(value: &str) -> Option<SavedTick> {
    let mut parts = value.split_whitespace();

    let x = parts.next()?.parse().ok()?;
    let y = parts.next()?.parse().ok()?;
    let z = parts.next()?.parse().ok()?;

    let kind = match parts.next()? {
        "block" => TickKind::Block,
        "fluid" => TickKind::Fluid,
        _ => return None,
    };

    let delay = parts.next()?.parse().ok()?;
    let priority = parts.next().and_then(|value| value.parse().ok()).unwrap_or(0);

    Some(((x, y, z), kind, delay, priority))
}

/// Ключ чанка по мировым координатам блока.
fn chunk_key(x: i32, z: i32) -> (i32, i32) {
    (x.div_euclid(CHUNK_SIZE), z.div_euclid(CHUNK_SIZE))
}

/// Попадает ли высота в границы мира.
fn in_bounds(y: i32) -> bool {
    (MIN_Y..MIN_Y + WORLD_HEIGHT).contains(&y)
}

/// Читатель простых чисел из файла мира.
struct Reader<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Reader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    /// Забирает следующие N байт, сдвигая позицию чтения.
    fn take<const N: usize>(&mut self) -> io::Result<[u8; N]> {
        let end = self.offset + N;

        let slice = self.bytes.get(self.offset..end).ok_or_else(|| {
            io::Error::new(io::ErrorKind::UnexpectedEof, "файл мира обрывается")
        })?;

        self.offset = end;

        Ok(slice.try_into().expect("длина среза уже проверена"))
    }

    fn u8(&mut self) -> io::Result<u8> {
        Ok(self.take::<1>()?[0])
    }

    fn u16(&mut self) -> io::Result<u16> {
        Ok(u16::from_be_bytes(self.take::<2>()?))
    }

    fn i32(&mut self) -> io::Result<i32> {
        Ok(i32::from_be_bytes(self.take::<4>()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Чанк складывается сам, когда до него дошли: под ногами земля,
    /// над ней воздух.
    #[test]
    fn a_new_chunk_comes_with_ground() {
        let mut world = World::in_memory();

        // Пока чанка нет — там пусто.
        assert_eq!(world.get_block(100, flat::SURFACE - 1, 100), AIR);

        assert!(world.ensure(6, 6));

        let grass = crate::blocks::state_by_name("grass_block").unwrap();
        let bedrock = crate::blocks::state_by_name("bedrock").unwrap();

        // Верхний блок земли — трава, ходят по его верху.
        assert_eq!(world.get_block(100, flat::SURFACE - 1, 100), grass);
        assert_eq!(world.get_block(100, flat::SURFACE, 100), AIR);
        assert_eq!(world.get_block(100, flat::bottom(), 100), bedrock);

        // Земля есть по всей площади чанка, а не только в одном месте.
        assert_eq!(world.get_block(111, flat::SURFACE - 1, 97), grass);

        // Второй раз тот же чанк не складывается заново.
        assert!(!world.ensure(6, 6));
    }

    /// Построенное игроком остаётся: земля складывается только там, где
    /// чанка ещё не было.
    #[test]
    fn building_survives_the_ground() {
        let mut world = World::in_memory();

        world.ensure(0, 0);
        world.set_block(1, flat::SURFACE, 1, 1);

        assert!(!world.ensure(0, 0));
        assert_eq!(world.get_block(1, flat::SURFACE, 1), 1);
    }

    /// Мир записывается по чанкам и читается обратно: поставленный блок
    /// переживает перезапуск, а нетронутая земля складывается заново.
    #[test]
    fn a_changed_chunk_is_read_back_from_its_region() {
        let directory = std::env::temp_dir().join("rustcraft_world_round_trip");
        let _ = fs::remove_dir_all(&directory);

        let stone = crate::blocks::state_by_name("stone").unwrap();

        {
            let mut world = World::open(&directory, Some(1), WorldKind::Flat).expect("открыть мир");

            world.ensure(0, 0);
            world.set_block(3, flat::SURFACE, 4, stone);
            world.save_if_needed();
        }

        let mut world =
            World::open(&directory, Some(1), WorldKind::Flat).expect("открыть мир снова");

        world.ensure(0, 0);
        assert_eq!(world.get_block(3, flat::SURFACE, 4), stone);

        // Земля на месте, и её не пришлось хранить целиком.
        let grass = crate::blocks::state_by_name("grass_block").unwrap();
        assert_eq!(world.get_block(3, flat::GROUND, 4), grass);
    }

    /// Чанк в байтах занимает немного: земля записывается не поблочно,
    /// а «состояние и сколько его подряд».
    #[test]
    fn a_flat_chunk_is_small_on_disk() {
        let bytes = Chunk::flat().to_bytes().len();

        assert!(bytes < 256, "чанк занял {} байт", bytes);

        // И читается обратно тем же.
        let read = Chunk::from_bytes(&Chunk::flat().to_bytes()).expect("прочитать");

        assert_eq!(read.block(5, flat::GROUND, 5), Chunk::flat().block(5, flat::GROUND, 5));
        assert_eq!(read.not_air(), Chunk::flat().not_air());
    }

    /// Объёмные биомы записываются и читаются обратно клетка в клетку, и
    /// записью «номер и сколько подряд» они почти ничего не стоят.
    #[test]
    fn volume_biomes_survive_the_disk() {
        let mut chunk = Chunk::flat();
        let plain = chunk.to_bytes().len();
        let lush = terrain::Biome::LushCaves.number() as BiomeId;
        let deep = terrain::Biome::DeepDark.number() as BiomeId;

        for layer in 0..BIOME_LAYERS {
            for cell in 0..BIOME_LAYER {
                chunk.biomes[layer * BIOME_LAYER + cell] = match layer {
                    0..=10 => deep,
                    11..=30 if cell % 3 == 0 => lush,
                    _ => cell as BiomeId,
                };
            }
        }

        let bytes = chunk.to_bytes();
        let read = Chunk::from_bytes(&bytes).expect("прочитать");

        assert_eq!(bytes[0], CHUNK_VERSION);
        assert_eq!(read.biomes, chunk.biomes);
        assert_eq!(read.not_air(), chunk.not_air());
        assert!(plain < 256, "ровный чанк занял {} байт", plain);
    }

    /// Чанки прежних версий читаются: их биом был один на столбец клетки,
    /// теперь он растягивается на всю высоту.
    #[test]
    fn older_chunks_are_still_read() {
        let stone = crate::blocks::state_by_name("stone").unwrap();

        for version in [CHUNK_VERSION_WIDE, CHUNK_VERSION_FLAT_BIOMES] {
            let mut bytes = vec![version];

            for section in 0..SECTIONS {
                if section != 0 {
                    bytes.push(0);
                    continue;
                }

                // Нижняя секция целиком из камня: одна запись.
                bytes.push(1);
                bytes.extend_from_slice(&1u16.to_be_bytes());

                if version == CHUNK_VERSION_WIDE {
                    bytes.extend_from_slice(&stone.to_be_bytes());
                } else {
                    bytes.extend_from_slice(&(stone as u16).to_be_bytes());
                }

                bytes.extend_from_slice(&(SECTION_BLOCKS as u16).to_be_bytes());
            }

            let without_biomes = bytes.clone();

            for cell in 0..BIOME_LAYER {
                bytes.extend_from_slice(&(cell as u16 + 3).to_be_bytes());
            }

            let chunk = Chunk::from_bytes(&bytes).expect("прочитать прежний чанк");

            assert_eq!(chunk.block(7, MIN_Y + 15, 9), stone);
            assert_eq!(chunk.block(7, MIN_Y + 16, 9), AIR);

            for layer in 0..BIOME_LAYERS {
                for cell in 0..BIOME_LAYER {
                    assert_eq!(chunk.biomes[layer * BIOME_LAYER + cell], cell as BiomeId + 3);
                }
            }

            // Совсем старый чанк без биомов — первый биом реестра.
            let chunk = Chunk::from_bytes(&without_biomes).expect("прочитать чанк без биомов");

            assert!(chunk.biomes.iter().all(|biome| *biome == 0));

            // Прочитанный старый чанк записывается уже новой версией.
            assert_eq!(chunk.to_bytes()[0], CHUNK_VERSION);
        }
    }

    /// Испорченная запись биомов — не повод читать чанк как попало.
    #[test]
    fn broken_biomes_are_refused() {
        let mut bytes = Chunk::flat().to_bytes();

        bytes.truncate(bytes.len() - 2);
        assert!(Chunk::from_bytes(&bytes).is_none());

        // Записи покрывают больше клеток, чем есть.
        let mut bytes = Chunk::flat().to_bytes();
        let length = bytes.len();

        bytes[length - 2..].copy_from_slice(&u16::MAX.to_be_bytes());
        assert!(Chunk::from_bytes(&bytes).is_none());
    }

    /// Воздушные секции не занимают памяти: пустой чанк почти ничего не стоит.
    #[test]
    fn air_costs_nothing() {
        let chunk = Chunk::new();

        assert!(chunk.sections.iter().all(|section| section.is_none()));

        // У ровной земли заняты две секции из двадцати четырёх: слои земли
        // приходятся на границу между ними.
        let flat = Chunk::flat();
        let filled = flat.sections.iter().flatten().count();

        assert_eq!(filled, 2);
    }

    /// Чанк, до которого никому нет дела, выгружается из памяти, но не
    /// теряется: изменённое перед этим записано, и читается обратно.
    #[test]
    fn far_chunks_are_unloaded_but_not_lost() {
        let directory = std::env::temp_dir().join("rustcraft_world_unload");
        let _ = fs::remove_dir_all(&directory);

        let stone = crate::blocks::state_by_name("stone").unwrap();

        let mut world = World::open(&directory, Some(1), WorldKind::Flat).expect("открыть мир");

        world.ensure(0, 0);
        world.ensure(10, 10);
        world.set_block(160, flat::SURFACE, 160, stone); // это чанк 10 10

        // Оставляем только начальный чанк.
        assert_eq!(world.unload_far(|x, z| (x, z) == (0, 0)), 1);

        // И убеждаемся, что дальний вернулся с диска целым.
        world.ensure(10, 10);
        assert_eq!(world.get_block(160, flat::SURFACE, 160), stone);
    }

    /// За такт разбирается не больше положенного, а лишнее ждёт следующего
    /// такта, а не пропадает.
    #[test]
    fn work_beyond_the_limit_waits_instead_of_being_lost() {
        let mut world = World::in_memory();

        // Такты срабатывают только в загруженном чанке.
        world.ensure(0, 0);

        for x in 0..5 {
            world.schedule_once(x, 1, 0, 1, 0);
        }

        let first = world.advance(2);
        assert_eq!(first.len(), 2, "за такт взято больше положенного");

        let second = world.advance(2);
        assert_eq!(second.len(), 2, "отложенное не вернулось");

        let third = world.advance(2);
        assert_eq!(third.len(), 1, "последнее место потерялось");

        // И больше ничего не осталось.
        assert!(world.advance(2).is_empty());
    }


    /// За границами мира блоков нет, и спрашивать о них можно спокойно:
    /// поршень у потолка обращается именно туда.
    #[test]
    fn asking_beyond_the_world_is_safe() {
        let mut world = World::in_memory();

        world.set_block(0, 1, 0, 1);

        assert_eq!(world.get_block(0, MIN_Y + WORLD_HEIGHT, 0), AIR);
        assert_eq!(world.get_block(0, MIN_Y - 1, 0), AIR);
        assert_eq!(world.get_block(0, 100_000, 0), AIR);
        assert_eq!(world.get_block(0, -100_000, 0), AIR);

        // И поставить туда ничего нельзя.
        assert!(!world.set_block(0, MIN_Y + WORLD_HEIGHT, 0, 1));
    }


    /// Часы мира переживают перезапуск: возраст и смещение записываются
    /// в файл мира и читаются обратно.
    #[test]
    fn the_clock_survives_a_restart() {
        let mut path = std::env::temp_dir();
        path.push(format!("rustcraft-level-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);

        {
            let mut world = World::open(&path, Some(1), WorldKind::Flat).expect("мир открылся");

            world.advance(16);
            world.set_time_of_day(13_000);
            world.save_if_needed();

            assert_eq!(world.time_of_day(), 13_000);
        }

        // Открываем заново — время на месте.
        let again = World::open(&path, Some(1), WorldKind::Flat).expect("мир открылся снова");

        assert_eq!(again.time_of_day(), 13_000, "часы сбросились");
        assert_eq!(again.tick(), 1, "возраст мира потерялся");

        let _ = fs::remove_dir_all(&path);
    }


    /// Запланированные такты переживают перезапуск: без них после
    /// перезагрузки встали бы часы на факелах и повторители в середине
    /// задержки.
    #[test]
    fn waiting_places_survive_a_restart() {
        let mut path = std::env::temp_dir();
        path.push(format!("rustcraft-ticks-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);

        {
            let mut world = World::open(&path, Some(1), WorldKind::Flat).expect("мир открылся");

            world.ensure(0, 0);
            world.schedule_once(1, 2, 3, 4, -3);
            world.schedule(5, 6, 7, 2);
            world.save_if_needed();
        }

        let mut again = World::open(&path, Some(1), WorldKind::Flat).expect("мир открылся снова");
        again.ensure(0, 0);

        // Через четыре такта должно всплыть место повторителя, через два —
        // место жидкости.
        let mut seen = Vec::new();

        for _ in 0..5 {
            for (place, kind) in again.advance(crate::tick::PER_TICK) {
                seen.push((place, kind));
            }
        }

        assert!(
            seen.contains(&((1, 2, 3), TickKind::Block)),
            "блочный такт не вернулся: {seen:?}"
        );
        assert!(
            seen.contains(&((5, 6, 7), TickKind::Fluid)),
            "жидкостный такт не вернулся: {seen:?}"
        );

        let _ = fs::remove_dir_all(&path);
    }


    /// Такт в выгруженном чанке не пропадает: он ждёт, пока чанк прочитают
    /// снова, и тогда срабатывает. Иначе схема в ушедшем из виду чанке
    /// умирала бы, как в тикете MC-711.
    #[test]
    fn a_tick_waits_for_its_chunk_to_come_back() {
        let mut path = std::env::temp_dir();
        path.push(format!("rustcraft-parked-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);

        let mut world = World::open(&path, Some(1), WorldKind::Flat).expect("мир открылся");
        world.ensure(0, 0);
        world.ensure(10, 10);

        // Такт в дальнем чанке, а сам чанк тут же выгружаем.
        world.schedule_once(165, 5, 165, 2, 0);
        world.unload_far(|x, z| (x, z) == (0, 0));

        // Срок прошёл, но чанка нет — такт не должен ни сработать, ни пропасть.
        let mut fired = Vec::new();

        for _ in 0..4 {
            fired.extend(world.advance(crate::tick::PER_TICK));
        }

        assert!(fired.is_empty(), "такт сработал в выгруженном чанке: {fired:?}");

        // Чанк вернулся — такт срабатывает на ближайшем такте.
        world.ensure(10, 10);
        fired.extend(world.advance(crate::tick::PER_TICK));
        fired.extend(world.advance(crate::tick::PER_TICK));

        assert!(
            fired.contains(&((165, 5, 165), TickKind::Block)),
            "такт так и не сработал после возвращения чанка: {fired:?}"
        );

        let _ = fs::remove_dir_all(&path);
    }

}

#[cfg(test)]
mod spawn_tests {
    use super::*;

    /// Точка появления — на суше и над водой: в обычном мире начало координат
    /// запросто оказывается морем.
    #[test]
    fn the_spawn_point_stands_on_land() {
        let directory = std::env::temp_dir().join("mc_spawn_point");
        let _ = fs::remove_dir_all(&directory);

        let world = World::open(
            &directory,
            Some(100_554_032_945_340),
            WorldKind::SuperSmooth,
        )
        .expect("мир");
        let (x, y, z) = world.spawn_position();

        let column = world.generator.column_at(x as i32, z as i32);

        assert!(!column.biome.is_ocean(), "появление в море: {:?}", column.biome);
        assert!(y as i32 > terrain::SEA, "появление под водой: y = {}", y);
        assert_eq!(y as i32, column.height + 1, "не на поверхности");
    }
}

#[cfg(test)]
mod speed {
    use super::*;

    /// Сколько времени уходит на чанк. Не проверка, а замер: запускается
    /// вручную, `cargo test chunk_speed -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn chunk_speed() {
        for style in [terrain::Style::Vanilla, terrain::Style::Smooth] {
            let generator = Generator::Normal(Terrain::new(100_554_032_945_340, style));

            let started = std::time::Instant::now();
            let count = 64;

            for i in 0..count {
                Chunk::generated(&generator, i % 8, i / 8);
            }

            let each = started.elapsed() / count as u32;

            println!(
                "{:?} — чанк: {:?}, на 441 чанк ушло бы {:?}",
                style,
                each,
                each * 441
            );
        }
    }

    /// Сколько времени уходит на каждый этап генерации чанка «Обычного»
    /// мира: `cargo test --release chunk_phases -- --ignored --nocapture`.
    /// Этапы после рельефа прогоняются на готовом чанке ещё раз и меряются
    /// по отдельности; рельеф — это всё остальное время.
    #[test]
    #[ignore]
    fn chunk_phases() {
        use std::time::{Duration, Instant};

        let generator =
            Generator::Normal(Terrain::new(100_554_032_945_340, terrain::Style::Vanilla));
        let count = 64;
        let mut total = Duration::ZERO;
        let mut phases = [Duration::ZERO; 7];
        let mut nests = Duration::ZERO;
        let mut ore_biomes = Duration::ZERO;
        let mut ready = HashMap::new();

        for i in 0..count {
            let (chunk_x, chunk_z) = (i % 8, i / 8);
            let started = Instant::now();
            let mut chunk = Chunk::generated(&generator, chunk_x, chunk_z);

            total += started.elapsed();

            let highest = chunk.highest();
            let mut timed = |index: usize, work: &mut dyn FnMut(&mut Chunk)| {
                let started = Instant::now();

                work(&mut chunk);
                phases[index] += started.elapsed();
            };

            let (from_x, from_z) = (chunk_x * CHUNK_SIZE - 1, chunk_z * CHUNK_SIZE - 1);

            timed(0, &mut |_| {
                let grid = generator.relief_grid(
                    from_x,
                    from_z,
                    from_x + CHUNK_SIZE + 1,
                    from_z + CHUNK_SIZE + 1,
                );

                for x in from_x..from_x + CHUNK_SIZE + 2 {
                    for z in from_z..from_z + CHUNK_SIZE + 2 {
                        std::hint::black_box(generator.column_in(x, z, grid.as_ref()));
                    }
                }
            });
            timed(1, &mut |chunk| generator.lay_ores(chunk, chunk_x, chunk_z));

            if let Generator::Normal(terrain) = &generator {
                let started = Instant::now();

                std::hint::black_box(terrain.ore_blocks_in(chunk_x, chunk_z, highest));
                nests += started.elapsed();

                let started = Instant::now();

                for from_x in chunk_x - 1..=chunk_x + 1 {
                    for from_z in chunk_z - 1..=chunk_z + 1 {
                        std::hint::black_box(terrain.column_at(from_x * 16 + 8, from_z * 16 + 8));
                    }
                }

                ore_biomes += started.elapsed();
            }
            timed(2, &mut |chunk| {
                underground::dig(&generator, chunk, chunk_x, chunk_z)
            });
            timed(3, &mut |chunk| {
                generator.grow_trees(chunk, chunk_x, chunk_z)
            });

            let grid = generator.relief_grid(
                from_x,
                from_z,
                from_x + CHUNK_SIZE + 1,
                from_z + CHUNK_SIZE + 1,
            );
            let mut columns = Vec::new();

            for x in from_x..from_x + CHUNK_SIZE + 2 {
                for z in from_z..from_z + CHUNK_SIZE + 2 {
                    columns.push(generator.column_in(x, z, grid.as_ref()));
                }
            }

            timed(4, &mut |chunk| {
                vegetation::decorate(&generator, chunk, chunk_x, chunk_z, &columns);
                ocean::decorate(&generator, chunk, chunk_x, chunk_z, &columns)
            });
            timed(5, &mut |chunk| {
                cave_biomes::decorate(&generator, chunk, chunk_x, chunk_z)
            });
            timed(6, &mut |chunk| chunk.light_up());

            ready.insert((chunk_x, chunk_z), chunk);
        }

        // Сшивка света с соседями — у чанков, у которых все восемь соседей
        // сложены, — и сколько памяти занимает свет.
        let mut world = World::in_memory();
        let light_bytes: usize = ready
            .values()
            .map(|chunk| chunk.light.as_ref().map_or(0, |(_, light)| light.bytes()))
            .sum();

        world.chunks = ready;

        let mut stitched = 0;
        let started = Instant::now();

        for chunk_x in 1..7 {
            for chunk_z in 1..7 {
                let job = world.light_job(chunk_x, chunk_z).expect("чанк");

                std::hint::black_box(job.finish());
                stitched += 1;
            }
        }

        let stitch = started.elapsed() / stitched;

        let each = |time: Duration| time / count as u32;
        let rest: Duration = phases.iter().sum();

        println!("весь чанк: {:?}", each(total));
        println!("  сетка и столбцы: {:?}", each(phases[0]));
        println!(
            "  блоки (пещеры, водоёмы, поверхность): {:?}",
            each(total.saturating_sub(rest))
        );
        println!(
            "  руда: {:?} (из них расчёт гнёзд {:?}, биомы соседних чанков {:?})",
            each(phases[1]),
            each(nests),
            each(ore_biomes)
        );
        println!("  подземелье: {:?}", each(phases[2]));
        println!("  деревья: {:?}", each(phases[3]));
        println!("  растительность и море: {:?}", each(phases[4]));
        println!("  пещерные биомы: {:?}", each(phases[5]));
        println!("  свет чанка: {:?}", each(phases[6]));
        println!("сшивка света с соседями перед отправкой: {:?}", stitch);
        println!(
            "память на свет: {} КБ на чанк (блоки — до {} КБ)",
            light_bytes / count as usize / 1024,
            SECTIONS as usize * SECTION_BLOCKS * 2 / 1024
        );
    }

    /// Растения на чанк по биомам — как tools/measure/plants_census.py
    /// считает у оригинала (биом — по середине чанка на поверхности,
    /// двухблочные — по нижней половине):
    /// `cargo test --release plants_census -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn plants_census() {
        const WATCH: [&str; 40] = [
            "sunflower", "lilac", "rose_bush", "peony", "tall_grass", "large_fern", "pumpkin", "melon", "sugar_cane",
            "cactus", "sweet_berry_bush", "lily_pad", "dandelion", "poppy", "cornflower", "azure_bluet", "short_grass",
            "fern", "pink_petals", "wildflowers", "dead_bush", "lily_of_the_valley", "oxeye_daisy", "allium",
            "red_tulip", "orange_tulip", "white_tulip", "pink_tulip", "blue_orchid", "bush", "firefly_bush",
            "leaf_litter", "short_dry_grass", "tall_dry_grass", "brown_mushroom", "red_mushroom", "seagrass",
            "tall_seagrass", "kelp", "cactus_flower",
        ];
        // PLANTS_TSV=1 — все растения построчно, как `plants_census.py --tsv`.
        let tsv = std::env::var_os("PLANTS_TSV").is_some();
        let generator = Generator::Normal(Terrain::new(100_554_032_945_340, terrain::Style::Vanilla));
        let mut chunks: HashMap<&str, u64> = HashMap::new();
        let mut counts: HashMap<(&str, &str), u64> = HashMap::new();
        // Чанки через восемь на квадрате 800×800 — по полосам, каждая в своём
        // потоке.
        let rows: Vec<i32> = (-400..400).step_by(8).collect();
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
        let parts: Vec<_> = std::thread::scope(|scope| {
            let handles: Vec<_> = rows
                .chunks(rows.len().div_ceil(threads))
                .map(|part| {
                    let generator = &generator;

                    scope.spawn(move || {
                        let mut chunks: HashMap<&str, u64> = HashMap::new();
                        let mut counts: HashMap<(&str, &str), u64> = HashMap::new();

                        for &chunk_x in part {
                            for chunk_z in (-400..400).step_by(8) {
                                let chunk = Chunk::generated(generator, chunk_x, chunk_z);
                                let biome =
                                    generator.column_at(chunk_x * CHUNK_SIZE + 8, chunk_z * CHUNK_SIZE + 8).biome.name();

                                *chunks.entry(biome).or_default() += 1;

                                for x in 0..CHUNK_SIZE {
                                    for z in 0..CHUNK_SIZE {
                                        for y in 32..208 {
                                            let state = chunk.block(x, y, z);

                                            if state == AIR {
                                                continue;
                                            }

                                            // Верхняя половина двухблочного стоит на нижней
                                            // того же растения — её не считаем.
                                            if let Some(name) = crate::blocks::block_at_state(state)
                                                && WATCH.contains(&name)
                                                && crate::blocks::block_at_state(chunk.block(x, y - 1, z)) != Some(name)
                                            {
                                                *counts.entry((biome, name)).or_default() += 1;
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        (chunks, counts)
                    })
                })
                .collect();

            handles.into_iter().map(|handle| handle.join().expect("поток переписи упал")).collect()
        });

        for (part_chunks, part_counts) in parts {
            for (biome, n) in part_chunks {
                *chunks.entry(biome).or_default() += n;
            }

            for (key, n) in part_counts {
                *counts.entry(key).or_default() += n;
            }
        }

        let mut biomes: Vec<_> = chunks.iter().filter(|(_, n)| **n >= 20).collect();
        biomes.sort_by_key(|(_, n)| std::cmp::Reverse(**n));

        for (biome, n) in biomes {
            if tsv {
                let mut names = WATCH;
                names.sort_unstable();

                for name in names {
                    if let Some(count) = counts.get(&(*biome, name)) {
                        println!("{}\t{}\t{}\t{:.3}", biome, n, name, *count as f64 / *n as f64);
                    }
                }

                continue;
            }

            let mut line: Vec<(&str, f64)> =
                WATCH.iter().map(|name| (*name, *counts.get(&(*biome, *name)).unwrap_or(&0) as f64 / *n as f64)).collect();
            line.sort_by(|a, b| b.1.total_cmp(&a.1));
            let text: Vec<String> = line.iter().take(8).filter(|(_, v)| *v > 0.0).map(|(k, v)| format!("{} {:.1}", k, v)).collect();

            println!("{} ({} чанков): {}", biome, n, text.join(", "));
        }
    }

    /// Сколько воды и лавы в обычных чанках сразу ставится течь и сколько
    /// стоит их поиск.
    /// `cargo test --release fluids_wake_census -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn fluids_wake_census() {
        let generator = Generator::Normal(Terrain::new(100_554_032_945_340, terrain::Style::Vanilla));
        let chunks: Vec<Chunk> = (0..64).map(|i| Chunk::generated(&generator, i % 8 * 7, i / 8 * 7)).collect();
        let started = std::time::Instant::now();
        let found: usize = chunks.iter().enumerate().map(|(i, chunk)| chunk.fluids_to_wake(i as i32 % 8 * 7, i as i32 / 8 * 7).len()).sum();

        println!("на чанк: мест {:.1}, поиск {:?}", found as f64 / 64.0, started.elapsed() / 64);
    }

    /// Разбор места, где вода не течёт: участок 5×5 чанков кладётся в мир
    /// так же, как при игре, прогоняются такты жидкостей, и в середине
    /// ищутся места, которые правило растекания ещё хотело бы изменить.
    /// `CHUNK="-13 -81" cargo test --release fluids_missed -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn fluids_missed() {
        let place = std::env::var("CHUNK").unwrap_or_else(|_| "-13 -81".into());
        let mut parts = place.split_whitespace().map(|v| v.parse::<i32>().unwrap());
        let (cx, cz) = (parts.next().unwrap(), parts.next().unwrap());
        let generator = Arc::new(Generator::Normal(Terrain::new(100_554_032_945_340, terrain::Style::Vanilla)));
        let mut world = World::in_memory();
        let source = ChunkSource { path: std::env::temp_dir().join("mcsheriffanya-no-such-world"), generator };

        for dx in -2..=2 {
            for dz in -2..=2 {
                world.accept(cx + dx, cz + dz, source.take(cx + dx, cz + dz));
            }
        }

        if let Ok(spot) = std::env::var("SPOT") {
            let v: Vec<i32> = spot.split_whitespace().map(|p| p.parse().unwrap()).collect();
            let fresh = source.take(v[0].div_euclid(16), v[2].div_euclid(16));
            let s = fresh.chunk.block(v[0].rem_euclid(16), v[1], v[2].rem_euclid(16));
            let column = generator_column(&source, v[0], v[2]);
            println!("свежий чанк в {:?}: {:?}; столбец: высота {}, биом {}", v, crate::blocks::block_at_state(s), column.0, column.1);
            println!("в списке разбуженных: {}", fresh.wake.iter().any(|((x, y, z), _)| (*x, *y, *z) == (v[0], v[1], v[2])));
            for (dx, dy, dz) in [(1, 0, 0), (-1, 0, 0), (0, 0, 1), (0, 0, -1), (0, 1, 0), (0, -1, 0)] {
                let (x, y, z) = (v[0] + dx, v[1] + dy, v[2] + dz);
                let chunk = source.take(x.div_euclid(16), z.div_euclid(16));
                println!("  сосед {} {} {} при генерации: {:?}", x, y, z, crate::blocks::block_at_state(chunk.chunk.block(x.rem_euclid(16), y, z.rem_euclid(16))));
            }
        }

        for _ in 0..std::env::var("TICKS").ok().and_then(|t| t.parse().ok()).unwrap_or(300) {
            for ((x, y, z), kind) in world.advance(crate::tick::PER_TICK) {
                if kind == TickKind::Fluid {
                    crate::tick::flow(&mut world, x, y, z);
                }
            }
        }

        let block = |x: i32, y: i32, z: i32| world.get_block(x, y, z);
        let mut stuck = 0;

        for x in cx * 16 - 16..cx * 16 + 32 {
            for z in cz * 16 - 16..cz * 16 + 32 {
                for y in MIN_Y..MIN_Y + WORLD_HEIGHT {
                    let state = world.get_block(x, y, z);

                    if state != AIR && fluids::fluid_at(state).is_none() {
                        continue;
                    }

                    if let Some(next) = fluids::update((x, y, z), &block)
                        && next != state
                    {
                        stuck += 1;

                        if stuck <= 8 {
                            let around: Vec<String> = [(1, 0, 0), (-1, 0, 0), (0, 0, 1), (0, 0, -1), (0, 1, 0), (0, -1, 0)]
                                .iter()
                                .map(|(dx, dy, dz)| {
                                    let s = world.get_block(x + dx, y + dy, z + dz);
                                    format!("{}{}", crate::blocks::block_at_state(s).unwrap_or("?"),
                                        fluids::fluid_at(s).map(|(_, l)| format!(":{}", l)).unwrap_or_default())
                                })
                                .collect();
                            println!("стоит {} {} {}: {:?} → {:?}; +x -x +z -z +y -y: {:?}", x, y, z,
                                crate::blocks::block_at_state(state), crate::blocks::block_at_state(next), around);
                        }
                    }
                }
            }
        }

        println!("мест, где жидкость ещё должна измениться: {}", stuck);
    }

    fn generator_column(source: &ChunkSource, x: i32, z: i32) -> (i32, &'static str) {
        let column = source.generator.column_at(x, z);
        (column.height, column.biome.name())
    }

    /// Вода над пустотой, сложенная вместе с чанком, сразу ставится течь,
    /// а замкнутая камнем — нет: ей течь некуда.
    #[test]
    fn generated_water_starts_flowing() {
        let mut chunk = Chunk::new();
        let water = fluids::state_of(fluids::Kind::Water, fluids::SOURCE);
        let stone = crate::blocks::state_by_name("stone").expect("камень есть");

        chunk.set(3, 70, 3, water);
        chunk.set(8, 70, 8, water);

        for (x, y, z) in [(7, 70, 8), (9, 70, 8), (8, 70, 7), (8, 70, 9), (8, 69, 8)] {
            chunk.set(x, y, z, stone);
        }

        // Будится пустота под водой, а не сама вода: такт жидкости решает,
        // чем станет место.
        let delay = fluids::Kind::Water.delay();
        let woken = chunk.fluids_to_wake(2, -1);

        assert!(woken.contains(&((35, 69, -13), delay)), "{:?}", woken);
        assert!(!woken.iter().any(|((x, _, z), _)| (*x, *z) == (40, -8)), "замкнутая вода разбужена: {:?}", woken);
    }

    /// Дно под водой по биомам: глубина и блоки сверху дна и на 1..4 ниже —
    /// в том же виде, что `tools/measure/seabed_census.py` у мира оригинала.
    /// Сравнение: `tools/measure/seabed_compare.py`.
    /// `cargo test --release seabed_census -- --ignored --nocapture | grep -P '^(depth|layer)\t'`.
    #[test]
    #[ignore]
    fn seabed_census() {
        type Layers = HashMap<(&'static str, i32), HashMap<&'static str, u64>>;

        let name = |state| crate::blocks::block_at_state(state).unwrap_or("air");
        let wet = |block: &str| {
            matches!(block, "water" | "seagrass" | "tall_seagrass" | "kelp" | "kelp_plant" | "bubble_column" | "sea_pickle")
                || block.ends_with("coral")
                || block.ends_with("coral_fan")
        };
        let top = |block: &str| wet(block) || matches!(block, "ice" | "packed_ice" | "blue_ice" | "lily_pad");
        let generator = Generator::Normal(Terrain::new(100_554_032_945_340, terrain::Style::Vanilla));
        let rows: Vec<i32> = (-300..300).step_by(6).collect();
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
        let parts: Vec<(HashMap<&str, Vec<i32>>, Layers)> = std::thread::scope(|scope| {
            let handles: Vec<_> = rows
                .chunks(rows.len().div_ceil(threads))
                .map(|part| {
                    let generator = &generator;

                    scope.spawn(move || {
                        let mut depth: HashMap<&str, Vec<i32>> = HashMap::new();
                        let mut layers: Layers = HashMap::new();

                        for &chunk_x in part {
                            for chunk_z in (-300..300).step_by(6) {
                                let chunk = Chunk::generated(generator, chunk_x, chunk_z);

                                for x in 0..CHUNK_SIZE {
                                    for z in 0..CHUNK_SIZE {
                                        let mut y = terrain::SEA;

                                        if !top(name(chunk.block(x, y, z))) {
                                            continue;
                                        }

                                        while y > -60 && top(name(chunk.block(x, y, z))) {
                                            y -= 1;
                                        }

                                        let biome = generator
                                            .column_at(chunk_x * CHUNK_SIZE + x, chunk_z * CHUNK_SIZE + z)
                                            .biome
                                            .name();

                                        depth.entry(biome).or_default().push(terrain::SEA - y);

                                        for k in 0..5 {
                                            *layers
                                                .entry((biome, k))
                                                .or_default()
                                                .entry(name(chunk.block(x, y - k, z)))
                                                .or_default() += 1;
                                        }
                                    }
                                }
                            }
                        }

                        (depth, layers)
                    })
                })
                .collect();

            handles.into_iter().map(|handle| handle.join().expect("поток переписи упал")).collect()
        });

        let mut depth: HashMap<&str, Vec<i32>> = HashMap::new();
        let mut layers: Layers = HashMap::new();

        for (part_depth, part_layers) in parts {
            for (biome, values) in part_depth {
                depth.entry(biome).or_default().extend(values);
            }

            for (key, blocks) in part_layers {
                for (block, n) in blocks {
                    *layers.entry(key).or_default().entry(block).or_default() += n;
                }
            }
        }

        for (biome, values) in &mut depth {
            if values.len() < 2000 {
                continue;
            }

            values.sort();
            let at = |part: f64| values[((values.len() - 1) as f64 * part) as usize];
            println!("depth\t{}\t{}\t{}\t{}\t{}", biome, at(0.1), at(0.5), at(0.9), values.len());

            for k in 0..5 {
                let blocks = &layers[&(*biome, k)];
                let total: u64 = blocks.values().sum();
                let mut sorted: Vec<_> = blocks.iter().collect();
                sorted.sort_by(|a, b| b.1.cmp(a.1));

                for (block, n) in sorted.into_iter().take(8) {
                    println!("layer\t{}\t{}\t{}\t{:.4}", biome, k, block, *n as f64 / total as f64);
                }
            }
        }
    }

    /// Подводное на чанк по биомам — как tools/measure/ocean_census.py
    /// считает у оригинала (биом — по середине чанка на поверхности):
    /// кораллы, морские огурцы, айсберги, лёд на воде, магма и пузырьковые
    /// колонны. Высоты айсбергов — от верхнего блока воды (у оригинала он
    /// на y=62, у нас — `terrain::SEA`).
    /// `cargo test --release ocean_census -- --ignored --nocapture`.
    ///
    /// Тёплых и замёрзших океанов мало, поэтому чанки выбираются по биому
    /// середины на квадрате 4000×4000 чанков через восемь: каждого биома —
    /// не больше 300 (OCEAN_CAP) первых по порядку.
    #[test]
    #[ignore]
    fn ocean_census() {
        const ROWS: [&str; 32] = [
            "coral_block", "coral", "coral_fan", "coral_wall_fan", "sea_pickle", "pickles", "sea_pickle_on_coral",
            "sea_pickle_on_other", "coral_chunks", "coral_kinds_sum", "packed_ice", "packed_ice_above",
            "packed_ice_below", "snow_block", "snow_block_above", "snow_block_below", "blue_ice", "blue_ice_above",
            "blue_ice_below", "berg_cols", "berg_chunks", "berg_max_sum", "berg_max_top", "sea_ice", "sea_open",
            "magma_block", "magma_wet_floor", "magma_wet_under", "magma_dry", "magma_floor_y_sum", "bubble_column",
            "bubble_bottoms",
        ];
        let sea = terrain::SEA;
        let cap: usize = std::env::var("OCEAN_CAP").ok().and_then(|text| text.parse().ok()).unwrap_or(300);
        // Имя блока по состоянию — заранее: иначе поиск на каждый блок.
        let names: Vec<&str> =
            (0..=u16::MAX as i32).map(|state| crate::blocks::block_at_state(state).unwrap_or("")).collect();
        let name = |state: i32| names[state as usize];
        let generator = Generator::Normal(Terrain::new(100_554_032_945_340, terrain::Style::Vanilla));
        let threads = std::thread::available_parallelism().map_or(4, |n| n.get());
        // Сперва биомы середин — это дёшево, потом чанки нужных.
        let lines: Vec<i32> = (-2000..2000).step_by(8).collect();
        let found: Vec<Vec<(i32, i32, &str)>> = std::thread::scope(|scope| {
            let handles: Vec<_> = lines
                .chunks(lines.len().div_ceil(threads))
                .map(|part| {
                    let generator = &generator;

                    scope.spawn(move || {
                        let mut found = Vec::new();

                        for &chunk_x in part {
                            for chunk_z in (-2000..2000).step_by(8) {
                                let middle = generator.column_at(chunk_x * CHUNK_SIZE + 8, chunk_z * CHUNK_SIZE + 8);

                                found.push((chunk_x, chunk_z, middle.biome.name()));
                            }
                        }

                        found
                    })
                })
                .collect();

            handles.into_iter().map(|handle| handle.join().expect("поток переписи упал")).collect()
        });
        let mut taken: HashMap<&str, usize> = HashMap::new();
        let picked: Vec<(i32, i32, &str)> = found
            .into_iter()
            .flatten()
            .filter(|(_, _, biome)| {
                let count = taken.entry(biome).or_default();

                *count += 1;
                *count <= cap
            })
            .collect();
        let parts: Vec<_> = std::thread::scope(|scope| {
            let handles: Vec<_> = picked
                .chunks(picked.len().div_ceil(threads))
                .map(|part| {
                    let (generator, name) = (&generator, &name);

                    scope.spawn(move || {
                        let mut chunks: HashMap<&str, u64> = HashMap::new();
                        let mut counts: HashMap<(&str, &str), u64> = HashMap::new();

                        for &(chunk_x, chunk_z, biome) in part {
                            {
                                let chunk = Chunk::generated(generator, chunk_x, chunk_z);
                                let mut add = |row: &'static str, n: u64| *counts.entry((biome, row)).or_default() += n;
                                let top = chunk.highest();
                                let mut kinds = std::collections::HashSet::new();
                                let mut berg_top = 0;

                                *chunks.entry(biome).or_default() += 1;

                                for x in 0..CHUNK_SIZE {
                                    for z in 0..CHUNK_SIZE {
                                        let mut surface = None;

                                        for y in (MIN_Y..=top).rev() {
                                            let state = chunk.block(x, y, z);

                                            if state == AIR {
                                                continue;
                                            }

                                            let here = name(state);

                                            if surface.is_none() {
                                                surface = Some(if here == "snow" { y - 1 } else { y });
                                            }

                                            let coral = |suffix: &str| !here.starts_with("dead_") && here.ends_with(suffix);

                                            if coral("_coral_block") {
                                                add("coral_block", 1);
                                            } else if coral("_coral_wall_fan") {
                                                add("coral_wall_fan", 1);
                                            } else if coral("_coral_fan") {
                                                add("coral_fan", 1);
                                            } else if coral("_coral") {
                                                add("coral", 1);
                                            } else if here == "sea_pickle" {
                                                let count = crate::blocks::orientation_of(state)
                                                    .and_then(|rule| rule.value(state, "pickles"))
                                                    .and_then(|text| text.parse().ok())
                                                    .unwrap_or(1);
                                                let under = name(chunk.block(x, y - 1, z));

                                                add("sea_pickle", 1);
                                                add("pickles", count);
                                                add(
                                                    if under.ends_with("_coral_block") {
                                                        "sea_pickle_on_coral"
                                                    } else {
                                                        "sea_pickle_on_other"
                                                    },
                                                    1,
                                                );
                                            } else if matches!(here, "packed_ice" | "snow_block" | "blue_ice") {
                                                add(here, 1);
                                                add(
                                                    match (here, y > sea) {
                                                        ("packed_ice", true) => "packed_ice_above",
                                                        ("packed_ice", false) => "packed_ice_below",
                                                        ("snow_block", true) => "snow_block_above",
                                                        ("snow_block", false) => "snow_block_below",
                                                        (_, true) => "blue_ice_above",
                                                        _ => "blue_ice_below",
                                                    },
                                                    1,
                                                );
                                            } else if here == "magma_block" {
                                                let above = name(chunk.block(x, y + 1, z));

                                                add("magma_block", 1);
                                                add(
                                                    if matches!(above, "water" | "bubble_column") {
                                                        "magma_wet_under"
                                                    } else {
                                                        "magma_dry"
                                                    },
                                                    1,
                                                );
                                            } else if here == "bubble_column" {
                                                add("bubble_column", 1);

                                                if name(chunk.block(x, y - 1, z)) == "magma_block" {
                                                    add("bubble_bottoms", 1);
                                                }
                                            }

                                            if !here.starts_with("dead_")
                                                && (here.ends_with("_coral_block")
                                                    || here.ends_with("_coral")
                                                    || here.ends_with("_coral_fan")
                                                    || here.ends_with("_coral_wall_fan"))
                                            {
                                                kinds.insert(here.split('_').next().unwrap_or(""));
                                            }
                                        }

                                        // Поверхность моря: лёд или открытая вода там, где над
                                        // ней ничего; айсберги — по столбцам.
                                        match surface {
                                            Some(y) if y == sea => match name(chunk.block(x, y, z)) {
                                                "ice" => add("sea_ice", 1),
                                                "water" => add("sea_open", 1),
                                                _ => {}
                                            },
                                            Some(y)
                                                if y > sea
                                                    && y <= sea + 60
                                                    && matches!(
                                                        name(chunk.block(x, y, z)),
                                                        "packed_ice" | "snow_block" | "blue_ice"
                                                    ) =>
                                            {
                                                add("berg_cols", 1);
                                                berg_top = berg_top.max(y - sea);
                                            }
                                            _ => {}
                                        }
                                    }
                                }

                                if berg_top > 0 {
                                    add("berg_chunks", 1);
                                    add("berg_max_sum", berg_top as u64);
                                    let best = counts.entry((biome, "berg_max_top")).or_default();
                                    *best = (*best).max(berg_top as u64);
                                }

                                if !kinds.is_empty() {
                                    *counts.entry((biome, "coral_chunks")).or_default() += 1;
                                }

                                *counts.entry((biome, "coral_kinds_sum")).or_default() += kinds.len() as u64;
                            }
                        }

                        (chunks, counts)
                    })
                })
                .collect();

            handles.into_iter().map(|handle| handle.join().expect("поток переписи упал")).collect()
        });

        let mut chunks: HashMap<&str, u64> = HashMap::new();
        let mut counts: HashMap<(&str, &str), u64> = HashMap::new();

        for (part_chunks, part_counts) in parts {
            for (biome, n) in part_chunks {
                *chunks.entry(biome).or_default() += n;
            }

            for (key, n) in part_counts {
                let total = counts.entry(key).or_default();

                *total = if key.1 == "berg_max_top" { (*total).max(n) } else { *total + n };
            }
        }

        let mut biomes: Vec<_> = chunks.iter().filter(|(_, n)| **n >= 20).collect();
        biomes.sort_by_key(|(_, n)| std::cmp::Reverse(**n));

        for (biome, n) in biomes {
            let line: Vec<String> = ROWS
                .iter()
                .filter_map(|row| {
                    let count = *counts.get(&(*biome, *row))?;

                    (count > 0 && *row != "berg_max_top").then(|| format!("{} {:.2}", row, count as f64 / *n as f64))
                })
                .collect();

            println!("{} ({} чанков): {}", biome, n, line.join(", "));

            if let Some(top) = counts.get(&(*biome, "berg_max_top")) {
                println!("  самый высокий айсберг: {}", top);
            }
        }
    }

    /// Сколько в чанках блоков, у которых на клиенте своя объёмная модель
    /// (сундуки, спавнеры, ульи, скалк и прочее): клиент рисует их как
    /// сущности, и тысячи таких в кадре тормозят.
    /// `cargo test --release block_entity_census -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn block_entity_census() {
        const WITH_MODEL: [&str; 16] = [
            "chest", "trapped_chest", "spawner", "trial_spawner", "bee_nest", "beehive", "sculk_sensor",
            "calibrated_sculk_sensor", "sculk_shrieker", "sculk_catalyst", "bell", "decorated_pot", "creaking_heart",
            "suspicious_sand", "suspicious_gravel", "vault",
        ];

        for style in [terrain::Style::Smooth, terrain::Style::Vanilla] {
            let generator = Generator::Normal(Terrain::new(100_554_032_945_340, style));
            let mut counts: HashMap<&str, u64> = HashMap::new();
            let side = 21;

            for chunk_x in -side / 2..=side / 2 {
                for chunk_z in -side / 2..=side / 2 {
                    let chunk = Chunk::generated(&generator, chunk_x, chunk_z);

                    for x in 0..CHUNK_SIZE {
                        for z in 0..CHUNK_SIZE {
                            for y in MIN_Y..MIN_Y + WORLD_HEIGHT {
                                let state = chunk.block(x, y, z);

                                if state == AIR {
                                    continue;
                                }

                                if let Some(name) = crate::blocks::block_at_state(state)
                                    && WITH_MODEL.contains(&name)
                                {
                                    *counts.entry(name).or_default() += 1;
                                }
                            }
                        }
                    }
                }
            }

            println!("{:?}, {} чанков: {:?}", style, side * side, counts);
        }
    }

    /// Разведка подземелья: из чего сложен камень под землёй в сложенных
    /// чанках — сколько пустот, сколько руды и какой. Запускается вручную:
    /// `cargo test underground -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn underground() {
        let generator = Generator::Normal(Terrain::smooth(100_554_032_945_340));
        let air = AIR;
        let water = terrain::block_named("water");
        let lava = terrain::block_named("lava");

        let mut counts: HashMap<String, u64> = HashMap::new();
        let mut hollow = 0u64;
        let mut solid = 0u64;

        for i in 0..64 {
            let (chunk_x, chunk_z) = (i % 8 * 3, i / 8 * 3);
            let chunk = Chunk::generated(&generator, chunk_x, chunk_z);

            for x in 0..CHUNK_SIZE {
                for z in 0..CHUNK_SIZE {
                    let column = generator.column_at(chunk_x * 16 + x, chunk_z * 16 + z);

                    // Только то, что под землёй: от дна до поверхности.
                    for y in (MIN_Y + 5)..column.height.min(terrain::SEA) {
                        let state = chunk.block(x, y, z);

                        if state == air || state == water || state == lava {
                            hollow += 1;
                            continue;
                        }

                        solid += 1;

                        let name = crate::blocks::block_at_state(state).unwrap_or("?");

                        if name.ends_with("_ore")
                            || matches!(
                                name,
                                "dirt" | "gravel" | "granite" | "diorite" | "andesite" | "tuff"
                            )
                        {
                            *counts
                                .entry(name.trim_start_matches("deepslate_").to_string())
                                .or_default() += 1;
                        }
                    }
                }
            }
        }

        let total = (hollow + solid) as f64;
        let ores: u64 = counts
            .iter()
            .filter(|(name, _)| name.ends_with("_ore"))
            .map(|(_, n)| n)
            .sum();

        println!(
            "под землёй: пустот {:.1}%, руды {:.2}%",
            100.0 * hollow as f64 / total,
            100.0 * ores as f64 / total
        );

        let mut rows: Vec<(String, u64)> = counts.into_iter().collect();
        rows.sort_by_key(|(_, n)| std::cmp::Reverse(*n));

        for (name, n) in rows {
            println!("  {:>7.3}%  {}", 100.0 * n as f64 / total, name);
        }
    }
}
