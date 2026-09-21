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

pub mod flat;
pub mod region;

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use crate::fluids;
use crate::journal::{self, Journal};
use crate::redstone;
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
    sections: Vec<Option<Vec<i32>>>,
}

impl Chunk {
    fn new() -> Self {
        Self {
            sections: (0..SECTIONS).map(|_| None).collect(),
        }
    }

    /// Чанк ровного мира: земля одинаковой высоты по всей площади.
    fn flat() -> Self {
        let mut chunk = Self::new();

        for y in MIN_Y..MIN_Y + WORLD_HEIGHT {
            let state = flat::block_at(y);

            if state == AIR {
                continue;
            }

            for index in 0..(CHUNK_SIZE * CHUNK_SIZE) as usize {
                let (section, inside) = split(y);

                chunk.filled_section(section)[inside + index] = state;
            }
        }

        chunk
    }

    /// Состояние блока внутри чанка.
    fn block(&self, x: i32, y: i32, z: i32) -> i32 {
        let (section, inside) = split(y);

        match &self.sections[section] {
            Some(blocks) => blocks[inside + corner(x, z)],
            None => AIR,
        }
    }

    /// Ставит блок. Возвращает false, если он уже был таким.
    fn set(&mut self, x: i32, y: i32, z: i32, state: i32) -> bool {
        let (section, inside) = split(y);
        let index = inside + corner(x, z);

        // В воздушную секцию воздух ставить незачем — заводить её ради этого
        // тем более.
        if state == AIR && self.sections[section].is_none() {
            return false;
        }

        let blocks = self.filled_section(section);

        if blocks[index] == state {
            return false;
        }

        blocks[index] = state;
        true
    }

    /// Секция, готовая к записи: воздушная заводится на месте.
    fn filled_section(&mut self, section: usize) -> &mut Vec<i32> {
        self.sections[section].get_or_insert_with(|| vec![AIR; SECTION_BLOCKS])
    }

    /// Сколько в чанке блоков, отличных от воздуха.
    #[cfg(test)]
    fn not_air(&self) -> usize {
        self.sections
            .iter()
            .flatten()
            .map(|blocks| blocks.iter().filter(|state| **state != AIR).count())
            .sum()
    }

    /// Блоки секции. None — секция целиком из воздуха.
    fn section(&self, section: usize) -> Option<&[i32]> {
        self.sections[section].as_deref()
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

        out
    }

    /// Читает чанк из байтов файла региона.
    fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if *bytes.first()? != CHUNK_VERSION {
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
                let state = reader.i32().ok()?;
                let length = reader.u16().ok()? as usize;

                for _ in 0..length {
                    *blocks.get_mut(at)? = state;
                    at += 1;
                }
            }
        }

        Some(chunk)
    }
}

/// Разбивает блоки секции на записи «состояние и сколько его подряд».
fn runs_of(blocks: &[i32]) -> Vec<(i32, u16)> {
    let mut runs: Vec<(i32, u16)> = Vec::new();

    for state in blocks {
        match runs.last_mut() {
            // Подряд одного состояния не может быть больше, чем блоков
            // в секции, так что счётчик не переполнится.
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
const CHUNK_VERSION: u8 = 1;

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
            block_events: Vec::new(),
            redstone: redstone::Memory::new(),
            destroyed: Vec::new(),
            dirty: HashSet::new(),
        }
    }

    /// Открывает мир: готовит директорию, в которой лежат регионы.
    ///
    /// Сами чанки не читаются: каждый читается тогда, когда до него дошли.
    pub fn open(directory: impl AsRef<Path>) -> io::Result<Self> {
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
    pub fn section_blocks(&self, chunk_x: i32, chunk_z: i32, section: usize) -> Option<&[i32]> {
        self.chunks
            .get(&(chunk_x, chunk_z))
            .and_then(|chunk| chunk.section(section))
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
            None => (Chunk::flat(), false),
        };

        self.chunks.insert((chunk_x, chunk_z), chunk);

        // Такты, дожидавшиеся этого чанка, возвращаются в очередь. Срок у них
        // уже вышел, поэтому сработают на ближайшем такте — в игре такой
        // просроченный такт тоже выполняется сразу.
        let woken: Vec<Scheduled> = self
            .parked
            .extract_if(.., |entry| chunk_key(entry.place.0, entry.place.2) == (chunk_x, chunk_z))
            .collect();

        for entry in woken {
            self.waiting.remove(&(entry.place, entry.kind));
            self.push_scheduled(entry.place, entry.kind, self.tick + 1, entry.priority);
        }

        // Прочитанное с диска может быть несогласованным — сервер могли
        // остановить посреди хода поршня. Свежесложенной земле чинить нечего.
        if from_disk {
            redstone::repair_chunk(self, chunk_x, chunk_z);
        }

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
        if self.path.as_os_str().is_empty() {
            return None;
        }

        let directory = self.path.join(REGIONS);

        match region::read(&directory, chunk_x, chunk_z) {
            Ok(Some(bytes)) => match Chunk::from_bytes(&bytes) {
                Some(chunk) => Some(chunk),
                None => {
                    log_warn!(
                        "Чанк {} {} не читается, складываю заново",
                        chunk_x,
                        chunk_z
                    );
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
        let mut text = format!("age = {}\ntime-offset = {}\n", self.tick, self.time_offset);

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
            let mut world = World::open(&directory).expect("открыть мир");

            world.ensure(0, 0);
            world.set_block(3, flat::SURFACE, 4, stone);
            world.save_if_needed();
        }

        let mut world = World::open(&directory).expect("открыть мир снова");

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

        let mut world = World::open(&directory).expect("открыть мир");

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
            let mut world = World::open(&path).expect("мир открылся");

            world.advance(16);
            world.set_time_of_day(13_000);
            world.save_if_needed();

            assert_eq!(world.time_of_day(), 13_000);
        }

        // Открываем заново — время на месте.
        let again = World::open(&path).expect("мир открылся снова");

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
            let mut world = World::open(&path).expect("мир открылся");

            world.ensure(0, 0);
            world.schedule_once(1, 2, 3, 4, -3);
            world.schedule(5, 6, 7, 2);
            world.save_if_needed();
        }

        let mut again = World::open(&path).expect("мир открылся снова");
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

        let mut world = World::open(&path).expect("мир открылся");
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
