// Командные блоки: импульсный (command_block), повторяющийся
// (repeating_command_block) и цепной (chain_command_block).
//
// По странице minecraft.wiki «Command Block» и чёрному ящику (сценарии
// cmdblock-impulse, cmdblock-chain, cmdblock-repeat). Кода игры и чужих ядер
// нет.
//
// Устроено в два слоя. Под замком мира живёт всё, что касается редстоуна:
// фронт сигнала, запланированный такт, повтор каждый такт. Сама команда
// выполняется общим разбором команд (commands.rs), а он берёт замок мира
// сам, поэтому такт, дойдя до командного блока, только ставит его в очередь
// (`World::queue_command_run`), отпускает мир и выполняет очередь здесь —
// в том же такте, по порядку («Chained command blocks execute simultaneously
// in the same game tick in the order they are chained»).

use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};

use crate::blocks;
use crate::commands::{self, Source};
use crate::dimension::Dimension;
use crate::redstone::{self, Pos};
use crate::shared::Shared;
use crate::world::World;

/// Вид блок-сущности `minecraft:command_block` (реестр
/// `minecraft:block_entity_type`, tools/blackbox/data/registries.json) — один
/// на все три блока.
pub const BLOCK_ENTITY: i32 = 23;

/// Имя источника команды, пока блок не переименован: «Unnamed blocks use @».
pub const DEFAULT_NAME: &str = "@";

/// Режим блока — по виду блока.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// Импульсный: раз на фронт сигнала.
    Impulse,

    /// Повторяющийся: каждый такт, пока есть сигнал или «всегда активен».
    Repeat,

    /// Цепной: когда сработал блок, смотрящий в него.
    Chain,
}

impl Mode {
    /// Имя блока этого режима.
    pub fn block(self) -> &'static str {
        match self {
            Mode::Impulse => "command_block",
            Mode::Repeat => "repeating_command_block",
            Mode::Chain => "chain_command_block",
        }
    }

    /// Режим из пакета Program Command Block (minecraft-data, поле mode):
    /// 0 — цепной (SEQUENCE), 1 — повторяющийся (AUTO), 2 — импульсный
    /// (REDSTONE).
    pub fn from_packet(mode: i32) -> Option<Self> {
        match mode {
            0 => Some(Mode::Chain),
            1 => Some(Mode::Repeat),
            2 => Some(Mode::Impulse),
            _ => None,
        }
    }
}

/// Режим командного блока в этом состоянии; None — это не командный блок.
///
/// Зовётся на каждую смену блока в чанке, поэтому сверяет номер с
/// промежутками состояний трёх блоков, а не ищет имя.
pub fn mode_of(state: i32) -> Option<Mode> {
    use std::sync::OnceLock;

    static RANGES: OnceLock<Vec<(i32, i32, Mode)>> = OnceLock::new();

    let ranges = RANGES.get_or_init(|| {
        [Mode::Impulse, Mode::Repeat, Mode::Chain]
            .into_iter()
            .filter_map(|mode| blocks::orientation(mode.block()).map(|o| (o.min, o.max, mode)))
            .collect()
    });

    ranges.iter().find(|(min, max, _)| (*min..=*max).contains(&state)).map(|(_, _, mode)| *mode)
}

/// Правила игры командных блоков (вики, Game rule; имена 26.1 — со
/// снимка 25w44a, прежние принимаются тоже).
pub struct Rules {
    /// `command_block_output` (commandBlockOutput): сообщать ли операторам,
    /// что сделал блок. По умолчанию да.
    pub output: AtomicBool,

    /// `command_blocks_work` (commandBlocksEnabled): работают ли блоки вообще.
    /// В 26.1.2 это правило заменило настройку `enable-command-block`: в
    /// server.properties оригинала 26.1.2 её больше нет (чёрный ящик:
    /// оригинал переписывает файл при запуске, и строки там нет), а блоки
    /// на нём работают.
    pub work: AtomicBool,

    /// `max_command_sequence_length` (maxCommandChainLength): сколько
    /// команд цепочка выполняет за такт. По умолчанию 65536.
    pub max_sequence: AtomicI32,
}

impl Default for Rules {
    fn default() -> Self {
        Self {
            output: AtomicBool::new(true),
            work: AtomicBool::new(true),
            max_sequence: AtomicI32::new(65_536),
        }
    }
}

/// Правило командных блоков из команды `gamerule`: Some(ответ) — правило
/// наше и выставлено (или прочитано), None — не наше.
pub fn gamerule(shared: &Shared, rule: &str, value: Option<&str>) -> Option<Result<String, String>> {
    let rules = &shared.command_block_rules;
    let (name, flag) = match rule {
        "command_block_output" | "commandBlockOutput" => ("command_block_output", Some(&rules.output)),
        "command_blocks_work" | "commandBlocksEnabled" => ("command_blocks_work", Some(&rules.work)),
        "max_command_sequence_length" | "maxCommandChainLength" => ("max_command_sequence_length", None),
        _ => return None,
    };

    let answer = match (flag, value) {
        (Some(flag), None) => Ok(format!("Правило {} сейчас: {}", name, flag.load(Ordering::Relaxed))),
        (Some(flag), Some(value @ ("true" | "false"))) => {
            flag.store(value == "true", Ordering::Relaxed);
            Ok(format!("Правило {}: {}", name, value))
        }
        (Some(_), Some(value)) => Err(format!("Ожидалось true или false, а не «{}»", value)),
        (None, None) => Ok(format!("Правило {} сейчас: {}", name, rules.max_sequence.load(Ordering::Relaxed))),
        (None, Some(value)) => match value.parse::<i32>() {
            Ok(number) if number >= 0 => {
                rules.max_sequence.store(number, Ordering::Relaxed);
                Ok(format!("Правило {}: {}", name, number))
            }
            _ => Err(format!("Ожидалось целое число не меньше 0, а не «{}»", value)),
        },
    };

    Some(answer)
}

/// Данные блок-сущности командного блока — поля NBT оригинала (вики,
/// Command Block, «Block data»).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Data {
    /// `Command`.
    pub command: String,

    /// `auto`: «всегда активен» (иначе — «нужен сигнал»).
    pub auto: bool,

    /// `powered`: есть ли сигнал.
    pub powered: bool,

    /// `conditionMet`: выполнено ли условие при последней проверке.
    pub condition_met: bool,

    /// `SuccessCount`: итог последней команды; его читает сравнитель.
    pub success_count: i32,

    /// `LastOutput`: последний ответ с временем, «[ЧЧ:ММ:СС] текст»; пусто —
    /// ответа нет.
    pub last_output: String,

    /// `TrackOutput`: хранить ли ответ.
    pub track_output: bool,

    /// `LastExecution`: такт мира, в котором блок последний раз выполнил
    /// команду; 0 — не выполнял.
    pub last_execution: i64,

    /// `UpdateLastExecution`: не выполнять дважды за такт.
    pub update_last_execution: bool,

    /// `CustomName`: имя источника вместо «@» (текст как есть).
    pub custom_name: Option<String>,

    /// Последний ответ — ошибка (у оригинала он красный).
    pub last_failed: bool,
}

impl Default for Data {
    fn default() -> Self {
        Self {
            command: String::new(),
            auto: false,
            powered: false,
            condition_met: false,
            success_count: 0,
            last_output: String::new(),
            track_output: true,
            last_execution: 0,
            update_last_execution: true,
            custom_name: None,
            last_failed: false,
        }
    }
}

/// Признаки в записи на диск.
const FLAG_AUTO: u8 = 1;
const FLAG_POWERED: u8 = 2;
const FLAG_CONDITION: u8 = 4;
const FLAG_TRACK: u8 = 8;
const FLAG_UPDATE_LAST: u8 = 16;
const FLAG_NAMED: u8 = 32;
const FLAG_FAILED: u8 = 64;

impl Data {
    /// Имя источника команды.
    pub fn name(&self) -> &str {
        self.custom_name.as_deref().unwrap_or(DEFAULT_NAME)
    }

    /// Правит данные по записи SNBT из команды (`setblock … {Command:"…",
    /// auto:1b}`). Неизвестные поля пропускаются, как у оригинала. Ошибка —
    /// запись не разобрать.
    pub fn apply_snbt(&mut self, text: &str) -> Result<(), String> {
        let value = crate::snbt::parse(text).ok_or_else(|| format!("Не понял данные блока: {}", text))?;
        let crate::snbt::Value::Map(fields) = &value else {
            return Err(format!("Данные блока — не составное значение: {}", text));
        };
        let flag = |value: &crate::snbt::Value| value.as_f64().map(|number| number != 0.0);

        for (name, value) in fields {
            match name.as_str() {
                "Command" => self.command = value.as_str().unwrap_or_default().to_string(),
                "auto" => self.auto = flag(value).unwrap_or(self.auto),
                "powered" => self.powered = flag(value).unwrap_or(self.powered),
                "conditionMet" => self.condition_met = flag(value).unwrap_or(self.condition_met),
                "TrackOutput" => self.track_output = flag(value).unwrap_or(self.track_output),
                "UpdateLastExecution" => self.update_last_execution = flag(value).unwrap_or(self.update_last_execution),
                "SuccessCount" => self.success_count = value.as_f64().map_or(self.success_count, |n| n as i32),
                "LastExecution" => self.last_execution = value.as_f64().map_or(self.last_execution, |n| n as i64),
                "LastOutput" => self.last_output = value.as_str().unwrap_or_default().to_string(),
                "CustomName" => self.custom_name = value.as_str().map(str::to_string),
                _ => {}
            }
        }

        Ok(())
    }

    /// Сетевой NBT для Block Entity Data — поля, как их шлёт оригинал
    /// (чёрный ящик, cmdblock-impulse: после setblock с данными и при
    /// открытии окна).
    ///
    /// Порядок полей — как у оригинала: conditionMet, auto, powered, Command,
    /// LastExecution, SuccessCount, LastOutput, TrackOutput,
    /// UpdateLastExecution. LastOutput — текст `{"extra":[ответ],"text":"[время] "}`,
    /// ошибка красная (`{"color":"red","extra":[…],"text":""}`).
    pub fn network_nbt(&self) -> Vec<u8> {
        let mut out = vec![TAG_COMPOUND];

        byte(&mut out, "conditionMet", self.condition_met);
        byte(&mut out, "auto", self.auto);
        byte(&mut out, "powered", self.powered);
        string(&mut out, "Command", &self.command);

        if self.last_execution != 0 {
            head(&mut out, TAG_LONG, "LastExecution");
            out.extend_from_slice(&self.last_execution.to_be_bytes());
        }

        head(&mut out, TAG_INT, "SuccessCount");
        out.extend_from_slice(&self.success_count.to_be_bytes());

        if self.track_output && !self.last_output.is_empty() {
            let (time, text) = match self.last_output.split_once("] ") {
                Some((time, text)) => (format!("{}] ", time), text),
                None => (String::new(), self.last_output.as_str()),
            };

            head(&mut out, TAG_COMPOUND, "LastOutput");
            head(&mut out, TAG_LIST, "extra");
            out.push(TAG_COMPOUND);
            out.extend_from_slice(&1i32.to_be_bytes());

            if self.last_failed {
                string(&mut out, "color", "red");
                head(&mut out, TAG_LIST, "extra");
                out.push(TAG_COMPOUND);
                out.extend_from_slice(&1i32.to_be_bytes());
                string(&mut out, "text", text);
                out.push(TAG_END);
                string(&mut out, "text", "");
            } else {
                string(&mut out, "text", text);
            }

            out.push(TAG_END);
            string(&mut out, "text", &time);
            out.push(TAG_END);
        }

        byte(&mut out, "TrackOutput", self.track_output);
        byte(&mut out, "UpdateLastExecution", self.update_last_execution);

        if let Some(name) = &self.custom_name {
            string(&mut out, "CustomName", name);
        }

        out.push(TAG_END);
        out
    }

    /// Запись на диск (свой формат мира): место в чанке, признаки, числа,
    /// строки с длиной.
    pub fn write(&self, (x, y, z): (i32, i32, i32), out: &mut Vec<u8>) {
        let flags = [
            (self.auto, FLAG_AUTO),
            (self.powered, FLAG_POWERED),
            (self.condition_met, FLAG_CONDITION),
            (self.track_output, FLAG_TRACK),
            (self.update_last_execution, FLAG_UPDATE_LAST),
            (self.custom_name.is_some(), FLAG_NAMED),
            (self.last_failed, FLAG_FAILED),
        ]
        .iter()
        .filter(|(on, _)| *on)
        .fold(0u8, |all, (_, flag)| all | flag);

        out.push(((x << 4) | z) as u8);
        out.extend_from_slice(&(y as i16).to_be_bytes());
        out.push(flags);
        out.extend_from_slice(&self.success_count.to_be_bytes());
        out.extend_from_slice(&self.last_execution.to_be_bytes());

        for text in [self.command.as_str(), self.last_output.as_str(), self.custom_name.as_deref().unwrap_or("")] {
            let bytes = &text.as_bytes()[..text.len().min(u16::MAX as usize)];

            out.extend_from_slice(&(bytes.len() as u16).to_be_bytes());
            out.extend_from_slice(bytes);
        }
    }

    /// Читает запись `write`; None — запись испорчена.
    pub fn read(rest: &mut &[u8]) -> Option<((i32, i32, i32), Self)> {
        let (head, tail) = rest.split_at_checked(16)?;
        let place = ((head[0] >> 4) as i32, i16::from_be_bytes([head[1], head[2]]) as i32, (head[0] & 15) as i32);
        let flags = head[3];
        let success_count = i32::from_be_bytes(head[4..8].try_into().ok()?);
        let last_execution = i64::from_be_bytes(head[8..16].try_into().ok()?);
        let mut tail = tail;
        let mut text = || -> Option<String> {
            let (length, after) = tail.split_at_checked(2)?;
            let (bytes, after) = after.split_at_checked(u16::from_be_bytes([length[0], length[1]]) as usize)?;

            tail = after;
            Some(String::from_utf8_lossy(bytes).into_owned())
        };
        let (command, last_output, name) = (text()?, text()?, text()?);

        *rest = tail;

        Some((
            place,
            Self {
                command,
                auto: flags & FLAG_AUTO != 0,
                powered: flags & FLAG_POWERED != 0,
                condition_met: flags & FLAG_CONDITION != 0,
                success_count,
                last_output,
                track_output: flags & FLAG_TRACK != 0,
                last_execution,
                update_last_execution: flags & FLAG_UPDATE_LAST != 0,
                custom_name: (flags & FLAG_NAMED != 0).then_some(name),
                last_failed: flags & FLAG_FAILED != 0,
            },
        ))
    }
}

/// Типы тегов NBT, что нужны данным блока.
const TAG_END: u8 = 0x00;
const TAG_BYTE: u8 = 0x01;
const TAG_INT: u8 = 0x03;
const TAG_LONG: u8 = 0x04;
const TAG_STRING: u8 = 0x08;
const TAG_LIST: u8 = 0x09;
const TAG_COMPOUND: u8 = 0x0A;

/// Тип и имя тега.
fn head(out: &mut Vec<u8>, kind: u8, name: &str) {
    out.push(kind);
    out.extend_from_slice(&(name.len() as u16).to_be_bytes());
    out.extend_from_slice(name.as_bytes());
}

/// Тег-признак.
fn byte(out: &mut Vec<u8>, name: &str, value: bool) {
    head(out, TAG_BYTE, name);
    out.push(u8::from(value));
}

/// Тег-строка (строка — в «modified UTF-8», как у сетевого NBT).
fn string(out: &mut Vec<u8>, name: &str, value: &str) {
    head(out, TAG_STRING, name);
    out.extend_from_slice(&crate::network::nbt::text_component(value)[1..]);
}

/// Куда смотрит блок в этом состоянии.
fn facing(state: i32) -> Option<(i32, i32, i32)> {
    let value = blocks::orientation_of(state)?.value(state, "facing")?;

    Some(match value {
        "north" => (0, 0, -1),
        "south" => (0, 0, 1),
        "west" => (-1, 0, 0),
        "east" => (1, 0, 0),
        "up" => (0, 1, 0),
        _ => (0, -1, 0),
    })
}

/// Условный ли блок (свойство `conditional`).
fn conditional(state: i32) -> bool {
    blocks::orientation_of(state).and_then(|o| o.value(state, "conditional")) == Some("true")
}

/// Отмечает, выполнено ли условие: безусловный — всегда; условный — если
/// блок позади (против `facing`) командный и его последняя команда удалась
/// («only when the command block behind it has executed successfully
/// (Success count is greater than 0)»).
fn mark_condition(world: &mut World, (x, y, z): Pos) -> bool {
    let state = world.get_block(x, y, z);
    let met = if conditional(state) {
        let (dx, dy, dz) = facing(state).unwrap_or((0, -1, 0));
        let behind = (x - dx, y - dy, z - dz);

        mode_of(world.get_block(behind.0, behind.1, behind.2)).is_some()
            && world.command_block(behind.0, behind.1, behind.2).is_some_and(|data| data.success_count > 0)
    } else {
        true
    };

    world.with_command_block(x, y, z, |data| data.condition_met = met);
    met
}

// ---------------------------------------------------------------------------
// Редстоун: под замком мира
// ---------------------------------------------------------------------------

/// У блока сменился сигнал рядом (`powered` — есть ли он теперь). Фронт
/// запоминается в `powered`; на подъёме импульсный и повторяющийся в режиме
/// «нужен сигнал» заводят такт через 1 (чёрный ящик, cmdblock-impulse:
/// команда — в такте после рычага). Цепной сигнал только запоминает: его
/// будит блок позади.
pub fn react(world: &mut World, pos: Pos, powered: bool) {
    let (x, y, z) = pos;
    let Some(mode) = mode_of(world.get_block(x, y, z)) else {
        return;
    };
    let Some((was, auto)) = world.with_command_block(x, y, z, |data| (data.powered, data.auto)) else {
        return;
    };

    if powered == was {
        return;
    }

    world.with_command_block(x, y, z, |data| data.powered = powered);

    if powered && !auto && mode != Mode::Chain {
        mark_condition(world, pos);
        world.schedule_once(x, y, z, 1, redstone::PRIORITY_NORMAL);
    }
}

/// Блок стал «всегда активен» (окно или данные команды): импульсный и
/// повторяющийся без сигнала заводят такт через 1 — импульсный сработает
/// один раз, повторяющийся пойдёт каждый такт. [проверить] у блока, уже
/// стоявшего «всегда активен», такт не заводится.
pub fn became_auto(world: &mut World, pos: Pos) {
    let (x, y, z) = pos;
    let Some(mode) = mode_of(world.get_block(x, y, z)) else {
        return;
    };
    let powered = world.command_block(x, y, z).is_some_and(|data| data.powered);

    if mode != Mode::Chain && !powered {
        mark_condition(world, pos);
        world.schedule_once(x, y, z, 1, redstone::PRIORITY_NORMAL);
    }
}

/// Запланированный такт командного блока: импульсный и повторяющийся
/// встают в очередь на выполнение; повторяющийся заводит такт на следующий,
/// пока есть сигнал или «всегда активен» («executes every game tick»).
///
/// Сигнал в самом такте не проверяется: рычаг, выключенный в начале такта,
/// повторяющийся ещё раз выполняет — и больше нет (чёрный ящик,
/// cmdblock-repeat: «tick» в такте выключения рычага, дальше тишина).
pub fn ticked(world: &mut World, pos: Pos) {
    let (x, y, z) = pos;
    let Some(mode) = mode_of(world.get_block(x, y, z)) else {
        return;
    };
    let Some(active) = world.command_block(x, y, z).map(|data| data.powered || data.auto) else {
        return;
    };

    if mode == Mode::Chain {
        return;
    }

    world.queue_command_run(x, y, z);

    if mode == Mode::Repeat && active {
        world.schedule_once(x, y, z, 1, redstone::PRIORITY_NORMAL);
    }
}

/// Сила для сравнителя: SuccessCount («signal strength specific to the
/// success count»), не больше 15.
pub fn comparator_reading(world: &World, (x, y, z): Pos) -> Option<u8> {
    mode_of(world.get_block(x, y, z))?;

    Some(world.command_block(x, y, z).map_or(0, |data| data.success_count.clamp(0, 15) as u8))
}

// ---------------------------------------------------------------------------
// Выполнение: без замка мира
// ---------------------------------------------------------------------------

/// Выполняет блоки из очереди такта и цепочки за ними — по порядку, в этом
/// же такте. Замок мира `dimension` держать нельзя: команды берут его сами.
pub fn run_queued(shared: &Shared, dimension: Dimension, starts: Vec<Pos>) {
    for start in starts {
        run_from(shared, dimension, start);
    }
}

/// Блок `start` (импульсный или повторяющийся) выполняет команду, а за ним —
/// цепные, куда он смотрит, пока цепь не кончится. Счёт шагов —
/// `max_command_sequence_length`.
fn run_from(shared: &Shared, dimension: Dimension, start: Pos) {
    let limit = shared.command_block_rules.max_sequence.load(Ordering::Relaxed).max(0) as i64;
    let mut steps = 0i64;

    // Сам первый блок: условие у импульсного отмечено, когда заводился
    // такт; у повторяющегося — каждый такт заново.
    let Some(direction) = step(shared, dimension, start, true) else {
        return;
    };

    let (mut at, mut direction) = (start, direction);

    loop {
        steps += 1;

        if steps > limit {
            break;
        }

        let next = (at.0 + direction.0, at.1 + direction.1, at.2 + direction.2);

        match step(shared, dimension, next, false) {
            Some(ahead) => {
                at = next;
                direction = ahead;
            }
            None => break,
        }
    }
}

/// Один блок цепи. Возвращает, куда смотрит блок (туда идёт цепь), или
/// None — цепь здесь обрывается: не командный блок (для цепи — не цепной),
/// цепной без сигнала и не «всегда активен», или уже выполнявший команду
/// в этом такте («loops execute only once»).
fn step(shared: &Shared, dimension: Dimension, pos: Pos, first: bool) -> Option<(i32, i32, i32)> {
    let (x, y, z) = pos;

    // Под замком — решить, выполнять ли, и запомнить такт выполнения.
    let (command, name, track, direction) = {
        let mut world = shared.world_in(dimension).lock().expect("мир захвачен другим потоком");
        let state = world.get_block(x, y, z);
        let mode = mode_of(state)?;

        if !first && mode != Mode::Chain {
            return None;
        }

        let data = world.command_block(x, y, z)?.clone();
        let now = world.tick() as i64;

        if !first && !(data.powered || data.auto) {
            return None;
        }

        if data.update_last_execution && data.last_execution == now {
            return None;
        }

        let direction = facing(state)?;
        let met = if first && mode == Mode::Impulse { data.condition_met } else { mark_condition(&mut world, pos) };

        if !met {
            // Условие не выполнено: команды нет, итог — ноль, а цепь идёт
            // дальше («it triggers another chain command block it is facing,
            // without executing the command»).
            world.with_command_block(x, y, z, |data| data.success_count = 0);
            redstone::comparator_update(&mut world, pos);
            redstone::settle(&mut world);
            return Some(direction);
        }

        world.with_command_block(x, y, z, |data| {
            if data.update_last_execution {
                data.last_execution = now;
            }
        });

        let name = data.name().to_string();

        (data.command, name, data.track_output, direction)
    };

    let works = shared.command_block_rules.work.load(Ordering::Relaxed);
    let (success, output) = if command.trim().is_empty() || !works {
        (0, None)
    } else {
        execute(shared, dimension, pos, &name, &command)
    };

    let mut world = shared.world_in(dimension).lock().expect("мир захвачен другим потоком");

    world.with_command_block(x, y, z, |data| {
        data.success_count = success;

        if track && let Some(text) = &output {
            data.last_output = format!("[{}] {}", crate::log::local_time(), text);
            data.last_failed = success == 0;
        }
    });

    // Блок с новым ответом (LastOutput) оригинал рассылает заново, тем же
    // состоянием (чёрный ящик, cmdblock-chain: в пачке такта — блоки 0, 1,
    // 2 и 4, удачные и нет, а условный 3, не выполнявший, — нет;
    // cmdblock-repeat: `say` ответа не даёт — и блоки не рассылаются).
    if track && output.is_some() {
        let state = world.get_block(x, y, z);

        world.announce_change(x, y, z, state);
    }
    redstone::comparator_update(&mut world, pos);
    redstone::settle(&mut world);

    Some(direction)
}

/// Выполняет команду блока через общий разбор. Возвращает итог (SuccessCount)
/// и ответ (для LastOutput); удачный ответ уходит операторам в чат и в лог,
/// если включено `command_block_output` («broadcast to all other ops in
/// multiplayer mode»), — серым курсивом «[@: ответ]» (перевод
/// `chat.type.admin`).
fn execute(shared: &Shared, dimension: Dimension, (x, y, z): Pos, name: &str, command: &str) -> (i32, Option<String>) {
    let command = command.strip_prefix('/').unwrap_or(command);
    let source = Source::Block { x, y, z, dimension, name: name.to_string() };

    commands::take_result();

    let answer = commands::run(command, &source, shared);
    let success = commands::take_result().unwrap_or_else(|| i32::from(!commands::looks_failed(&answer.reply)));

    if answer.reply.is_empty() {
        return (success, None);
    }

    if success > 0 && shared.command_block_rules.output.load(Ordering::Relaxed) {
        crate::log_info!("[{}: {}]", name, answer.reply);
        broadcast_to_ops(shared, name, &answer.reply);
    }

    (success, Some(answer.reply))
}

/// «[имя: текст]» серым курсивом — всем операторам в игре.
fn broadcast_to_ops(shared: &Shared, name: &str, text: &str) {
    let component = admin_message(name, text);
    let ops: Vec<[u8; 16]> = {
        let ops = shared.ops.lock().expect("права захвачены другим потоком");

        shared
            .players
            .lock()
            .expect("список игроков захвачен другим потоком")
            .members()
            .iter()
            .filter(|member| ops.level_of(&member.uuid) > 0)
            .map(|member| member.uuid)
            .collect()
    };
    let mut players = shared.players.lock().expect("список игроков захвачен другим потоком");

    for uuid in ops {
        players.order(&uuid, crate::players::Order::Message { component: component.clone() });
    }
}

/// Компонент «[имя: текст]» серым курсивом в сетевом NBT — как у
/// оригинала (чёрный ящик, cmdblock-chain): `{"with":[{"":имя},{"":текст}],
/// "color":"gray","italic":1b,"translate":"chat.type.admin"}` (в списке
/// разнородных частей строка обёрнута составным с пустым именем).
pub fn admin_message(name: &str, text: &str) -> Vec<u8> {
    let mut out = vec![TAG_COMPOUND];

    head(&mut out, TAG_LIST, "with");
    out.push(TAG_COMPOUND);
    out.extend_from_slice(&2i32.to_be_bytes());

    for value in [name, text] {
        string(&mut out, "", value);
        out.push(TAG_END);
    }

    string(&mut out, "color", "gray");
    byte(&mut out, "italic", true);
    string(&mut out, "translate", "chat.type.admin");
    out.push(TAG_END);
    out
}

/// Ответ на Program Command Block: `{"with":[команда],
/// "translate":"advMode.setCommand.success"}` — «Command set: …» на языке
/// клиента (чёрный ящик, cmdblock-impulse).
pub fn command_set_message(command: &str) -> Vec<u8> {
    let mut out = vec![TAG_COMPOUND];

    head(&mut out, TAG_LIST, "with");
    out.push(TAG_STRING);
    out.extend_from_slice(&1i32.to_be_bytes());
    out.extend_from_slice(&crate::network::nbt::text_component(command)[1..]);
    string(&mut out, "translate", "advMode.setCommand.success");
    out.push(TAG_END);
    out
}

// ---------------------------------------------------------------------------
// Окно: Program Command Block
// ---------------------------------------------------------------------------

/// Может ли игрок открыть окно блока и программировать его: оператор
/// (уровень 2 и выше) в творческом режиме («only if the player is in
/// Creative mode, and has the proper permissions»).
pub fn may_edit(shared: &Shared, uuid: &[u8; 16], game_mode: i32) -> bool {
    game_mode == commands::GAME_MODE_CREATIVE && shared.ops.lock().expect("права захвачены другим потоком").level_of(uuid) >= 2
}

/// Пакет Program Command Block (кнопка «Готово»): команда, режим и признаки
/// (1 — хранить вывод, 2 — условный, 4 — всегда активен). Блок меняет вид
/// по режиму, сохраняя направление; SuccessCount сбрасывается («Leaving the
/// command block GUI by clicking Done ... resets the success count to 0»).
/// Возвращает ответ игроку («Command set: …», `command_set_message`) или
/// None — блока нет.
pub fn program(world: &mut World, pos: Pos, command: &str, mode: i32, flags: u8) -> Option<Vec<u8>> {
    let (x, y, z) = pos;
    let state = world.get_block(x, y, z);

    mode_of(state)?;

    let mode = Mode::from_packet(mode)?;
    let track = flags & 1 != 0;
    let conditional = flags & 2 != 0;
    let auto = flags & 4 != 0;
    let direction = blocks::orientation_of(state).and_then(|o| o.value(state, "facing")).unwrap_or("north");
    let wanted = blocks::orientation(mode.block())
        .and_then(|o| o.with(o.min, "facing", direction))
        .and_then(|s| blocks::orientation_of(s)?.with(s, "conditional", if conditional { "true" } else { "false" }))?;
    let was_auto = world.command_block(x, y, z).is_some_and(|data| data.auto);

    world.with_command_block(x, y, z, |data| {
        data.command = command.to_string();
        data.track_output = track;
        data.success_count = 0;

        if !track {
            data.last_output.clear();
        }
    });

    if wanted != state {
        world.set_block(x, y, z, wanted);
    }

    world.with_command_block(x, y, z, |data| data.auto = auto);

    if auto && !was_auto {
        became_auto(world, pos);
    }

    // Блок рассылается заново и с прежним видом (чёрный ящик,
    // cmdblock-impulse: после «Command set» — повтор блока).
    let state = world.get_block(x, y, z);

    world.announce_change(x, y, z, state);
    redstone::comparator_update(world, pos);
    redstone::settle(world);

    Some(command_set_message(command))
}

/// Ставит данные из записи команды (`setblock … {Command:"…"}`) блоку в
/// этом месте. Ошибка — запись не разобрать.
pub fn set_from_command(world: &mut World, pos: Pos, text: &str) -> Result<bool, String> {
    let (x, y, z) = pos;
    let Some(mut data) = world.command_block(x, y, z).cloned() else {
        return Ok(false);
    };

    data.apply_snbt(text)?;

    let auto = data.auto;
    world.with_command_block(x, y, z, |there| *there = data);

    if auto {
        became_auto(world, pos);
    }

    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modes_by_state() {
        for (name, mode) in [("command_block", Mode::Impulse), ("repeating_command_block", Mode::Repeat), ("chain_command_block", Mode::Chain)] {
            let state = blocks::state_by_name(name).unwrap();

            assert_eq!(mode_of(state), Some(mode), "{name}");
        }

        assert_eq!(mode_of(blocks::state_by_name("stone").unwrap()), None);
        assert_eq!(Mode::from_packet(0), Some(Mode::Chain));
        assert_eq!(Mode::from_packet(2), Some(Mode::Impulse));
    }

    #[test]
    fn snbt_and_disk_round_trip() {
        let mut data = Data::default();

        data.apply_snbt(r#"{Command:"setblock ~ ~1 ~ minecraft:stone",auto:1b,TrackOutput:0b,SuccessCount:3,CustomName:"Кнопка"}"#).unwrap();

        assert_eq!(data.command, "setblock ~ ~1 ~ minecraft:stone");
        assert!(data.auto && !data.track_output);
        assert_eq!(data.success_count, 3);
        assert_eq!(data.name(), "Кнопка");

        data.last_output = "[12:00:00] готово".to_string();
        data.last_execution = 1234;

        let mut out = Vec::new();

        data.write((3, -60, 15), &mut out);

        let mut rest = out.as_slice();
        let (place, back) = Data::read(&mut rest).unwrap();

        assert_eq!(place, (3, -60, 15));
        assert_eq!(back, data);
        assert!(rest.is_empty());
        assert!(data.apply_snbt("{Command:").is_err());
    }

    #[test]
    fn admin_message_is_a_compound() {
        let bytes = admin_message("@", "ok");

        assert_eq!(bytes[0], 0x0A);
        assert_eq!(*bytes.last().unwrap(), 0x00);
        assert!(bytes.windows(15).any(|w| w == b"chat.type.admin"));
    }

    #[test]
    fn rules_from_gamerule() {
        let rules = Rules::default();

        assert!(rules.output.load(Ordering::Relaxed));
        assert!(rules.work.load(Ordering::Relaxed));
        assert_eq!(rules.max_sequence.load(Ordering::Relaxed), 65_536);
    }
}
