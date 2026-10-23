// Команды мира и сущностей: worldborder, tick, clone, attribute, enchant,
// locate, forceload, spreadplayers, random, ride, rotate, spectate,
// fillbiome, place, loot.
//
// Ответы — как у оригинала: текстом игры с ключом перевода и подстановками
// (снято чёрным ящиком, пробник cmdworld: числа идут строками, отформатиро-
// ванными по-явовски — «%.1f», «%.2f»; целые — числами; ошибка — красная
// обёртка `{"text":"","color":"red","extra":[…]}`). Синтаксис и пределы —
// вики, страницы «Commands/<имя>».

use std::collections::{BTreeSet, HashMap};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::commands::{Answer, Effect, Source};
use crate::dimension::Dimension;
use crate::network::nbt::Compound;
use crate::network::varint::encode_varint;
use crate::players::Order;
use crate::shared::Shared;

// ---------------------------------------------------------------------------
// Текст ответа
// ---------------------------------------------------------------------------

/// Подстановка в текст игры.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Arg {
    /// Строка (у оригинала так идут и дробные числа — уже отформатированными).
    Str(String),
    /// Целое число.
    Int(i64),
    /// Вложенный текст с ключом перевода.
    Tr(String, Vec<Arg>),
    /// Простой текст `{"text": …}`.
    Text(String),
    /// Дробное число (тег double): так оригинал шлёт значения `attribute`
    /// (пробник: `{"":0.10000000149011612}`).
    Double(f64),
    /// Имя игрока, как его шлёт оригинал: текст с подстановкой, щелчком
    /// «/tell имя » и наведением show_entity (пробник: `enchant`, `give`).
    Player(String, [u8; 16]),
    /// Готовый компаунд текста (зачарование с уровнем).
    Raw(Box<RawText>),
}

/// Готовый текст: компаунд не клонируется, поэтому хранится строитель.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum RawText {
    /// Название зачарования с уровнем: `{"translate":"enchantment.minecraft.X",
    /// "color":"gray","extra":[" ",{"translate":"enchantment.level.N"}]}`.
    Enchantment { name: String, level: Option<i32>, curse: bool },
}

pub(crate) fn s(text: impl Into<String>) -> Arg {
    Arg::Str(text.into())
}

/// Текст игры: `{"translate": key, "with": [...]}`. Если все подстановки —
/// строки, список строк; все числа — список чисел; иначе — список
/// компаундов, где простые значения лежат под пустым именем (пробник:
/// `{"with":[{"":"0.3"},…,{"":100}]}`).
pub(crate) fn component(key: &str, args: &[Arg]) -> Compound {
    // Порядок полей — как у оригинала (пробник): `with`, затем `translate`.
    with_args(Compound::new(), args).string("translate", key)
}

/// Добавляет список подстановок `with` (если они есть).
fn with_args(out: Compound, args: &[Arg]) -> Compound {
    if args.is_empty() {
        return out;
    }

    if args.iter().all(|arg| matches!(arg, Arg::Str(_))) {
        let texts: Vec<&str> = args.iter().map(|arg| if let Arg::Str(text) = arg { text.as_str() } else { "" }).collect();

        out.strings("with", &texts)
    } else if args.iter().all(|arg| matches!(arg, Arg::Int(value) if i32::try_from(*value).is_ok())) {
        let numbers: Vec<i32> = args.iter().map(|arg| if let Arg::Int(value) = arg { *value as i32 } else { 0 }).collect();

        out.ints("with", &numbers)
    } else {
        out.compounds("with", args.iter().map(arg_compound).collect())
    }
}

fn arg_compound(arg: &Arg) -> Compound {
    match arg {
        Arg::Str(text) => Compound::new().string("", text),
        Arg::Int(value) => match i32::try_from(*value) {
            Ok(value) => Compound::new().int("", value),
            Err(_) => Compound::new().string("", &value.to_string()),
        },
        Arg::Tr(key, args) => component(key, args),
        Arg::Text(text) => Compound::new().string("text", text),
        Arg::Double(value) => Compound::new().double("", *value),
        Arg::Player(name, uuid) => player_component(name, uuid),
        Arg::Raw(raw) => match raw.as_ref() {
            RawText::Enchantment { name, level, curse } => {
                let mut out = Compound::new().string("color", if *curse { "red" } else { "gray" });

                if let Some(level) = level {
                    out = out.compounds("extra", vec![Compound::new().string("", " "), Compound::new().string("translate", &format!("enchantment.level.{}", level))]);
                }

                out.string("translate", &format!("enchantment.minecraft.{}", name))
            }
        },
    }
}

/// Имя игрока текстом игры (пробник: `{"text":"blackbox5","insertion":
/// "blackbox5","click_event":{"action":"suggest_command","command":"/tell
/// blackbox5 "},"hover_event":{"action":"show_entity","id":"minecraft:player",
/// "uuid":[…],"name":"blackbox5"}}`).
fn player_component(name: &str, uuid: &[u8; 16]) -> Compound {
    let words: Vec<i32> = uuid.chunks(4).map(|four| i32::from_be_bytes([four[0], four[1], four[2], four[3]])).collect();

    Compound::new()
        .compound("click_event", Compound::new().string("action", "suggest_command").string("command", &format!("/tell {} ", name)))
        .string("insertion", name)
        .string("text", name)
        .compound("hover_event", Compound::new().string("name", name).string("action", "show_entity").string("id", "minecraft:player").int_array("uuid", &words))
}

/// Английская запись текста — для консоли и Bedrock (шаблоны — языковая
/// таблица en_us из minecraft-data).
pub(crate) fn english(key: &str, args: &[Arg]) -> String {
    let template = TEMPLATES.iter().find(|(known, _)| *known == key).map_or(key, |(_, text)| *text);
    let mut out = String::new();
    let mut rest = template;
    let mut index = 0;

    while let Some(at) = rest.find("%s") {
        out.push_str(&rest[..at]);
        out.push_str(&args.get(index).map(arg_english).unwrap_or_default());
        index += 1;
        rest = &rest[at + 2..];
    }

    out.push_str(rest);

    if template == key && !args.is_empty() {
        out.push_str(&format!(" {}", args.iter().map(arg_english).collect::<Vec<_>>().join(", ")));
    }

    out
}

fn arg_english(arg: &Arg) -> String {
    match arg {
        Arg::Str(text) | Arg::Text(text) | Arg::Player(text, _) => text.clone(),
        Arg::Int(value) => value.to_string(),
        Arg::Double(value) => java_double(*value),
        Arg::Tr(key, args) => english(key, args),
        Arg::Raw(raw) => match raw.as_ref() {
            RawText::Enchantment { name, level, .. } => match level {
                Some(level) => format!("{} {}", name, level),
                None => name.clone(),
            },
        },
    }
}

/// Удачный ответ.
pub(crate) fn tr(key: &str, args: &[Arg]) -> Answer {
    Answer {
        reply: english(key, args),
        effect: Effect::None,
        component: Some(component(key, args).encode_network()),
        rich: Vec::new(),
        result: Some(1),
    }
}

/// Ошибка: та же запись в красной обёртке.
pub(crate) fn fail(key: &str, args: &[Arg]) -> Answer {
    let wrapped = Compound::new().string("color", "red").compounds("extra", vec![component(key, args)]).string("text", "");

    Answer {
        reply: english(key, args),
        effect: Effect::None,
        component: Some(wrapped.encode_network()),
        rich: Vec::new(),
        result: None,
    }
}

/// Ошибка разбора: «Incorrect argument for command» — как у оригинала без
/// строки-указателя места [проверить: оригинал шлёт вторую строку с
/// `command.context.here`].
fn bad_syntax() -> Answer {
    fail("command.unknown.argument", &[])
}

/// Формат числа как у Явы (`String.format("%.Nf")`): кратчайшая запись
/// числа, округлённая половиной вверх (пробник: 1500.25 → «1500.3»; у Rust
/// `{:.1}` дал бы «1500.2»).
pub(crate) fn java_fixed(value: f64, decimals: usize) -> String {
    if !value.is_finite() {
        return value.to_string();
    }

    let negative = value < 0.0;
    let shortest = format!("{}", value.abs());
    let (whole, fraction) = shortest.split_once('.').unwrap_or((&shortest, ""));

    // Цифры числа без точки и место точки.
    let mut digits: Vec<u8> = whole.bytes().chain(fraction.bytes()).map(|b| b - b'0').collect();
    let point = whole.len();
    let keep = point + decimals;

    while digits.len() < keep + 1 {
        digits.push(0);
    }

    let round_up = digits[keep] >= 5;

    digits.truncate(keep);

    if round_up {
        let mut index = keep;

        loop {
            if index == 0 {
                digits.insert(0, 1);
                break;
            }

            index -= 1;

            if digits[index] == 9 {
                digits[index] = 0;
            } else {
                digits[index] += 1;
                break;
            }
        }
    }

    let point = point + (digits.len() - keep);
    let text: String = digits.iter().map(|d| char::from(b'0' + d)).collect();
    let (whole, fraction) = text.split_at(point);
    let whole = if whole.is_empty() { "0" } else { whole };
    let body = if decimals == 0 { whole.to_string() } else { format!("{}.{}", whole, fraction) };

    if negative && body.bytes().any(|b| (b'1'..=b'9').contains(&b)) {
        format!("-{}", body)
    } else {
        body
    }
}

// ---------------------------------------------------------------------------
// Общее состояние
// ---------------------------------------------------------------------------

/// Состояние команд мира: рамки, такт, удержанные чанки, последовательности
/// `random`, атрибуты игроков для чтения.
#[derive(Default)]
pub struct State {
    pub borders: Mutex<[Border; 3]>,
    pub ticking: Mutex<Ticking>,

    /// Удержанные чанки (`forceload`): измерение (номер) и чанк.
    pub forced: Mutex<BTreeSet<(usize, i32, i32)>>,

    /// Последовательности `random`: имя → сколько значений выдано.
    sequences: Mutex<HashMap<String, u64>>,

    /// Снимок подключения и ещё не исполненные им правки команд.
    player_attributes: Mutex<HashMap<[u8; 16], AttributeSnapshot>>,

    /// Кто на ком едет (`ride`): наездник → транспорт (номера сущностей).
    pub rides: Mutex<HashMap<i32, i32>>,
}

/// Номера пакетов (protocol.json 26.1 из minecraft-data).
const INITIALIZE_WORLD_BORDER: i32 = 0x2B;
const SET_BORDER_CENTER: i32 = 0x58;
const SET_BORDER_LERP_SIZE: i32 = 0x59;
const SET_BORDER_SIZE: i32 = 0x5A;
const SET_BORDER_WARNING_DELAY: i32 = 0x5B;
const SET_BORDER_WARNING_DISTANCE: i32 = 0x5C;
const SET_TICKING_STATE: i32 = 0x7F;
const STEP_TICK: i32 = 0x80;
const SET_PASSENGERS: i32 = 0x6B;
const SYSTEM_CHAT: i32 = 0x79;

/// Разослать пакет игрокам (только этого измерения, если оно задано).
fn broadcast(shared: &Shared, dimension: Option<Dimension>, body: Vec<u8>) {
    let mut players = shared.players.lock().expect("список игроков захвачен другим потоком");
    let targets: Vec<[u8; 16]> = players.members().iter().filter(|member| dimension.is_none_or(|wanted| member.dimension == wanted)).map(|member| member.uuid).collect();

    for uuid in targets {
        players.order(&uuid, Order::Packet(body.clone()));
    }
}

/// Пакет системного сообщения с этим текстом.
fn chat_packet(text: Compound) -> Vec<u8> {
    let mut body = encode_varint(SYSTEM_CHAT);

    body.extend_from_slice(&text.encode_network());
    body.push(0);
    body
}

// ---------------------------------------------------------------------------
// Рамка мира
// ---------------------------------------------------------------------------

/// Наибольший поперечник рамки (вики, World border: 59 999 968).
pub const MAX_SIZE: f64 = 59_999_968.0;

/// Дальше этого от нуля рамка не бывает (поле «portal teleport boundary»
/// пакета у оригинала — 29 999 984).
pub const MAX_CENTER: f64 = 29_999_984.0;

/// Рамка мира одного измерения (с 25w36a у каждого своя — вики, World border).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Border {
    pub center_x: f64,
    pub center_z: f64,

    /// Поперечник в начале и в конце изменения; без изменения равны.
    pub from: f64,
    pub to: f64,

    /// Изменение: сколько тактов всего и сколько осталось (снято пробником:
    /// поле speed пакета — такты, `set 2000 2` → speed 2).
    pub lerp_total: i64,
    pub lerp_left: i64,

    pub damage_per_block: f64,
    pub buffer: f64,
    pub warning_blocks: i32,

    /// Время предупреждения в тактах (у оригинала при входе — 300).
    pub warning_time: i32,
}

impl Default for Border {
    fn default() -> Self {
        // Пробник: при входе Initialize World Border — центр 0, 0, поперечник
        // 59 999 968, предупреждение 5 блоков и 300 тактов; урон 0,2 и запас
        // 5 — `damage amount 0.2` отвечает успехом, `damage buffer 5` —
        // «ничего не изменилось».
        Self { center_x: 0.0, center_z: 0.0, from: MAX_SIZE, to: MAX_SIZE, lerp_total: 0, lerp_left: 0, damage_per_block: 0.2, buffer: 5.0, warning_blocks: 5, warning_time: 300 }
    }
}

impl Border {
    /// Поперечник сейчас.
    pub fn size(&self) -> f64 {
        if self.lerp_left <= 0 || self.lerp_total <= 0 {
            return self.to;
        }

        let done = 1.0 - self.lerp_left as f64 / self.lerp_total as f64;

        self.from + (self.to - self.from) * done
    }

    /// Такт мира: изменение поперечника идёт дальше.
    pub fn step(&mut self) {
        if self.lerp_left > 0 {
            self.lerp_left -= 1;

            if self.lerp_left == 0 {
                self.from = self.to;
            }
        }
    }

    /// Края: наименьшие и наибольшие x и z.
    pub fn bounds(&self) -> (f64, f64, f64, f64) {
        let half = self.size() / 2.0;
        let clamp = |value: f64| value.clamp(-MAX_CENTER, MAX_CENTER);

        (clamp(self.center_x - half), clamp(self.center_x + half), clamp(self.center_z - half), clamp(self.center_z + half))
    }

    /// Расстояние до ближайшего края: внутри — положительное, снаружи —
    /// отрицательное.
    pub fn distance_inside(&self, x: f64, z: f64) -> f64 {
        let (min_x, max_x, min_z, max_z) = self.bounds();

        (x - min_x).min(max_x - x).min(z - min_z).min(max_z - z)
    }

    /// Урон за пребывание снаружи (вики, World border: урон тем, кто дальше
    /// запаса за рамкой, растёт с расстоянием): не меньше 1, по
    /// `damage_per_block` за каждый блок за запасом. None — урона нет.
    /// [проверить] округление и частоту чёрным ящиком.
    pub fn damage_at(&self, x: f64, z: f64) -> Option<f32> {
        let beyond = -(self.distance_inside(x, z) + self.buffer);

        if beyond <= 0.0 || self.damage_per_block <= 0.0 {
            return None;
        }

        Some((beyond * self.damage_per_block).floor().max(1.0) as f32)
    }

    /// Initialize World Border — при входе и смене измерения.
    pub fn init_packet(&self) -> Vec<u8> {
        let mut body = encode_varint(INITIALIZE_WORLD_BORDER);

        body.extend_from_slice(&self.center_x.to_be_bytes());
        body.extend_from_slice(&self.center_z.to_be_bytes());
        body.extend_from_slice(&self.size().to_be_bytes());
        body.extend_from_slice(&self.to.to_be_bytes());
        body.extend_from_slice(&encode_varint(self.lerp_left as i32));
        body.extend_from_slice(&encode_varint(MAX_CENTER as i32));
        body.extend_from_slice(&encode_varint(self.warning_blocks));
        body.extend_from_slice(&encode_varint(self.warning_time));
        body
    }
}

/// Рамка измерения сейчас.
pub fn border_of(shared: &Shared, dimension: Dimension) -> Border {
    shared.world_cmd.borders.lock().expect("рамки захвачены другим потоком")[dimension.index()]
}

/// Такт мира для рамок: изменения поперечника идут дальше.
pub fn step_borders(shared: &Shared) {
    for border in shared.world_cmd.borders.lock().expect("рамки захвачены другим потоком").iter_mut() {
        border.step();
    }
}

/// Время: целое с единицей `t`, `s` или `d` (вики, Argument types: time).
fn parse_time(text: &str) -> Option<i32> {
    let (number, scale) = match text.chars().last()? {
        't' => (&text[..text.len() - 1], 1.0),
        's' => (&text[..text.len() - 1], 20.0),
        'd' => (&text[..text.len() - 1], 24_000.0),
        _ => (text, 1.0),
    };

    let value: f64 = number.parse().ok()?;
    let ticks = (value * scale).round();

    (ticks >= 0.0 && ticks <= f64::from(i32::MAX)).then_some(ticks as i32)
}

/// Одна координата: число или `~` со сдвигом от `from`; целое по x и z —
/// середина блока, если `centre` (вики, Argument types: vec2/vec3).
fn coordinate(text: &str, from: f64, centre: bool) -> Option<f64> {
    let value = match text.strip_prefix('~') {
        Some("") => from,
        Some(shift) => from + shift.parse::<f64>().ok()?,
        None if centre && !text.contains('.') => text.parse::<i64>().ok()? as f64 + 0.5,
        None => text.parse::<f64>().ok()?,
    };

    value.is_finite().then_some(value)
}

/// Целое место блока: число или `~` со сдвигом.
fn block_coordinate(text: &str, from: f64) -> Option<i32> {
    match text.strip_prefix('~') {
        Some("") => Some(from.floor() as i32),
        Some(shift) => Some((from + shift.parse::<f64>().ok()?).floor() as i32),
        None => text.parse::<i32>().ok(),
    }
}

fn block_place(parts: &[&str], origin: (f64, f64, f64)) -> Option<(i32, i32, i32)> {
    let [x, y, z] = parts else {
        return None;
    };

    Some((block_coordinate(x, origin.0)?, block_coordinate(y, origin.1)?, block_coordinate(z, origin.2)?))
}

fn worldborder(parts: &[&str], source: &Source, shared: &Shared) -> Answer {
    let dimension = crate::commands::dimension_of(source, shared);
    let index = dimension.index();
    let mut borders = shared.world_cmd.borders.lock().expect("рамки захвачены другим потоком");
    let border = &mut borders[index];

    let send = |body: Vec<u8>| broadcast(shared, Some(dimension), body);

    match parts {
        ["get"] => tr("commands.worldborder.get", &[s(java_fixed(border.size(), 0))]),
        ["set" | "add", distance, rest @ ..] if rest.len() <= 1 => {
            let Ok(distance) = distance.parse::<f64>() else {
                return bad_syntax();
            };

            if distance.abs() > MAX_SIZE {
                // [проверить]: у оригинала ещё строка с местом ошибки.
                return fail(if distance > 0.0 { "argument.double.big" } else { "argument.double.low" }, &[Arg::Double(if distance > 0.0 { MAX_SIZE } else { -MAX_SIZE }), Arg::Double(distance)]);
            }

            let time = match rest.first() {
                Some(text) => match parse_time(text) {
                    Some(time) => time,
                    None => return bad_syntax(),
                },
                None => 0,
            };

            let now = border.size();
            let size = if parts[0] == "add" { now + distance } else { distance };

            if now == size {
                return fail("commands.worldborder.set.failed.nochange", &[]);
            }

            if size < 1.0 {
                return fail("commands.worldborder.set.failed.small", &[]);
            }

            if size > MAX_SIZE {
                return fail("commands.worldborder.set.failed.big", &[Arg::Int(MAX_SIZE as i64)]);
            }

            if time > 0 {
                border.from = now;
                border.to = size;
                border.lerp_total = i64::from(time);
                border.lerp_left = i64::from(time);

                let mut body = encode_varint(SET_BORDER_LERP_SIZE);

                body.extend_from_slice(&now.to_be_bytes());
                body.extend_from_slice(&size.to_be_bytes());
                body.extend_from_slice(&encode_varint(time));
                drop(borders);
                send(body);

                let key = if size < now { "commands.worldborder.set.shrink" } else { "commands.worldborder.set.grow" };

                tr(key, &[s(java_fixed(size, 1)), s(java_fixed(f64::from(time) / 20.0, 2))])
            } else {
                border.from = size;
                border.to = size;
                border.lerp_total = 0;
                border.lerp_left = 0;

                let mut body = encode_varint(SET_BORDER_SIZE);

                body.extend_from_slice(&size.to_be_bytes());
                drop(borders);
                send(body);

                tr("commands.worldborder.set.immediate", &[s(java_fixed(size, 1))])
            }
        }
        ["center", x, z] => {
            let origin = crate::commands::origin_of(source, shared);

            let (Some(x), Some(z)) = (coordinate(x, origin.0, true), coordinate(z, origin.2, true)) else {
                return bad_syntax();
            };

            if x.abs() > MAX_CENTER || z.abs() > MAX_CENTER {
                return fail("commands.worldborder.set.failed.far", &[Arg::Int(MAX_CENTER as i64)]);
            }

            if border.center_x == x && border.center_z == z {
                return fail("commands.worldborder.center.failed", &[]);
            }

            border.center_x = x;
            border.center_z = z;

            let mut body = encode_varint(SET_BORDER_CENTER);

            body.extend_from_slice(&x.to_be_bytes());
            body.extend_from_slice(&z.to_be_bytes());
            drop(borders);
            send(body);

            tr("commands.worldborder.center.success", &[s(java_fixed(x, 2)), s(java_fixed(z, 2))])
        }
        ["damage", "amount", value] => match value.parse::<f32>() {
            Ok(value) if value >= 0.0 => {
                if border.damage_per_block == f64::from(value) {
                    return fail("commands.worldborder.damage.amount.failed", &[]);
                }

                border.damage_per_block = f64::from(value);

                tr("commands.worldborder.damage.amount.success", &[s(java_fixed(f64::from(value), 2))])
            }
            Ok(_) => fail("argument.float.low", &[]),
            Err(_) => bad_syntax(),
        },
        ["damage", "buffer", value] => match value.parse::<f32>() {
            Ok(value) if value >= 0.0 => {
                if border.buffer == f64::from(value) {
                    return fail("commands.worldborder.damage.buffer.failed", &[]);
                }

                border.buffer = f64::from(value);

                tr("commands.worldborder.damage.buffer.success", &[s(java_fixed(f64::from(value), 2))])
            }
            Ok(_) => fail("argument.float.low", &[]),
            Err(_) => bad_syntax(),
        },
        ["warning", "distance", value] => match value.parse::<i32>() {
            Ok(value) if value >= 0 => {
                if border.warning_blocks == value {
                    return fail("commands.worldborder.warning.distance.failed", &[]);
                }

                border.warning_blocks = value;

                let mut body = encode_varint(SET_BORDER_WARNING_DISTANCE);

                body.extend_from_slice(&encode_varint(value));
                drop(borders);
                send(body);

                tr("commands.worldborder.warning.distance.success", &[Arg::Int(i64::from(value))])
            }
            _ => bad_syntax(),
        },
        ["warning", "time", value] => match parse_time(value) {
            Some(value) => {
                if border.warning_time == value {
                    return fail("commands.worldborder.warning.time.failed", &[]);
                }

                border.warning_time = value;

                let mut body = encode_varint(SET_BORDER_WARNING_DELAY);

                body.extend_from_slice(&encode_varint(value));
                drop(borders);
                send(body);

                tr("commands.worldborder.warning.time.success", &[s(java_fixed(f64::from(value) / 20.0, 2))])
            }
            None => bad_syntax(),
        },
        _ => bad_syntax(),
    }
}

// ---------------------------------------------------------------------------
// Такт
// ---------------------------------------------------------------------------

/// Скорость и заморозка такта (`tick`).
#[derive(Debug)]
pub struct Ticking {
    /// Тактов в секунду.
    pub rate: f32,
    pub frozen: bool,

    /// Сколько тактов ещё сделать в заморозке (`tick step`).
    pub step_left: i32,

    /// Разгон (`tick sprint`).
    sprint: Option<Sprint>,

    /// Сколько длились последние такты, мс — для `tick query`.
    spent: std::collections::VecDeque<f32>,
    intervals: std::collections::VecDeque<f32>,
    last_start: Option<Instant>,
}

#[derive(Debug)]
struct Sprint {
    left: i64,
    done: i64,
    started: Instant,
}

impl Default for Ticking {
    fn default() -> Self {
        Self { rate: 20.0, frozen: false, step_left: 0, sprint: None, spent: std::collections::VecDeque::new(), intervals: Default::default(), last_start: None }
    }
}

/// Сколько тактов помнить для `tick query` (пробник: «Sample: 100»).
const SAMPLES: usize = 100;

/// Что делать такту мира.
pub struct Plan {
    /// Сколько ждать до следующего такта.
    pub period: Duration,

    /// Идёт ли мир (блоки, сущности, время) — в заморозке нет.
    pub run_world: bool,
}

impl Ticking {
    /// Пауза по реальному времени не входит в TPS и длительность разгона.
    /// Сам freeze, оставшиеся step и sprint при этом не меняются.
    pub fn resumed_after_idle(&mut self, paused: Duration) {
        self.last_start = None;
        if let Some(sprint) = &mut self.sprint {
            let now = Instant::now();
            let started_pause = now.checked_sub(paused).unwrap_or(now);
            sprint.started = if sprint.started >= started_pause { now }
                else { sprint.started.checked_add(paused).unwrap_or(now) };
        }
    }

    pub fn metrics(&self) -> (f32, f32) {
        let mspt = self.spent.iter().sum::<f32>() / self.spent.len().max(1) as f32;
        let elapsed = if self.intervals.is_empty() { mspt } else { self.intervals.iter().sum::<f32>() / self.intervals.len() as f32 };
        (self.rate.min(1000.0 / elapsed.max(0.001)), mspt)
    }

    pub fn note_start(&mut self, now: Instant) {
        if let Some(last) = self.last_start.replace(now) {
            self.intervals.push_back(now.saturating_duration_since(last).as_secs_f32() * 1000.0);
            if self.intervals.len() > SAMPLES { self.intervals.pop_front(); }
        }
    }

    fn period(&self) -> Duration {
        Duration::from_secs_f64(1.0 / f64::from(self.rate))
    }

    fn state_packet(&self) -> Vec<u8> {
        let mut body = encode_varint(SET_TICKING_STATE);

        body.extend_from_slice(&self.rate.to_be_bytes());
        body.push(u8::from(self.frozen));
        body
    }

    fn step_packet(&self) -> Vec<u8> {
        let mut body = encode_varint(STEP_TICK);

        body.extend_from_slice(&encode_varint(self.step_left));
        body
    }
}

/// Пакеты состояния такта для входящего игрока (пробник: сразу за
/// Initialize World Border — Set Ticking State и Step Tick).
pub fn ticking_packets(shared: &Shared) -> [Vec<u8>; 2] {
    let ticking = shared.world_cmd.ticking.lock().expect("такт захвачен другим потоком");

    [ticking.state_packet(), ticking.step_packet()]
}

/// Решает, каким будет следующий такт. Зовётся тактом мира в начале каждого
/// круга.
pub fn next_tick(shared: &Shared) -> Plan {
    let mut ticking = shared.world_cmd.ticking.lock().expect("такт захвачен другим потоком");

    if let Some(sprint) = &mut ticking.sprint {
        if sprint.left > 0 {
            sprint.left -= 1;
            sprint.done += 1;

            return Plan { period: Duration::ZERO, run_world: true };
        }

        // Разгон кончился: отчёт операторам «от имени сервера», серым
        // курсивом (пробник: chat.type.admin, «Server»).
        let elapsed = sprint.started.elapsed().as_secs_f64().max(1e-9);
        let done = sprint.done.max(1);
        let per_second = (done as f64 / elapsed) as i64;
        let per_tick = elapsed * 1000.0 / done as f64;

        ticking.sprint = None;

        let state = ticking.state_packet();
        let period = ticking.period();

        drop(ticking);

        let report = Compound::new()
            .compounds("with", vec![Compound::new().string("", "Server"), component("commands.tick.sprint.report", &[Arg::Int(per_second), s(java_fixed(per_tick, 2))])])
            .string("color", "gray")
            .boolean("italic", true)
            .string("translate", "chat.type.admin");

        tell_operators(shared, chat_packet(report));
        broadcast(shared, None, state);
        crate::log_info!("{}", english("commands.tick.sprint.report", &[Arg::Int(per_second), s(java_fixed(per_tick, 2))]));

        return Plan { period, run_world: true };
    }

    let run_world = if ticking.frozen {
        if ticking.step_left > 0 {
            ticking.step_left -= 1;
            true
        } else {
            false
        }
    } else {
        true
    };

    Plan { period: ticking.period(), run_world }
}

/// Записать, сколько длился такт (для `tick query`).
pub fn note_spent(shared: &Shared, spent: Duration) {
    let mut ticking = shared.world_cmd.ticking.lock().expect("такт захвачен другим потоком");

    ticking.spent.push_back(spent.as_secs_f32() * 1000.0);

    while ticking.spent.len() > SAMPLES {
        ticking.spent.pop_front();
    }
}

/// Отправить пакет всем операторам в сети.
fn tell_operators(shared: &Shared, body: Vec<u8>) {
    let uuids: Vec<[u8; 16]> = shared.players.lock().expect("список игроков захвачен другим потоком").members().iter().map(|member| member.uuid).collect();
    let operators: Vec<[u8; 16]> = {
        let ops = shared.ops.lock().expect("права захвачены другим потоком");

        uuids.into_iter().filter(|uuid| ops.level_of(uuid) >= 2).collect()
    };
    let mut players = shared.players.lock().expect("список игроков захвачен другим потоком");

    for uuid in operators {
        players.order(&uuid, Order::Packet(body.clone()));
    }
}

fn tick(parts: &[&str], shared: &Shared) -> Answer {
    let mut ticking = shared.world_cmd.ticking.lock().expect("такт захвачен другим потоком");

    match parts {
        ["query"] => {
            let status = if ticking.sprint.is_some() {
                "commands.tick.status.sprinting"
            } else if ticking.frozen {
                "commands.tick.status.frozen"
            } else {
                let average = ticking.spent.iter().sum::<f32>() / ticking.spent.len().max(1) as f32;

                if f64::from(average) > ticking.period().as_secs_f64() * 1000.0 { "commands.tick.status.lagging" } else { "commands.tick.status.running" }
            };

            let mut sorted: Vec<f32> = ticking.spent.iter().copied().collect();

            sorted.sort_by(f32::total_cmp);

            let average = sorted.iter().sum::<f32>() / sorted.len().max(1) as f32;
            let percentile = |share: f64| sorted.get(((sorted.len() as f64 * share) as usize).min(sorted.len().saturating_sub(1))).copied().unwrap_or(0.0);
            let target = 1000.0 / f64::from(ticking.rate);

            let mut lines = vec![tr(status, &[])];

            if ticking.sprint.is_some() {
                lines.push(tr("commands.tick.query.rate.sprinting", &[s(java_fixed(f64::from(ticking.rate), 1)), s(java_fixed(f64::from(average), 1))]));
            } else {
                lines.push(tr("commands.tick.query.rate.running", &[s(java_fixed(f64::from(ticking.rate), 1)), s(java_fixed(f64::from(average), 1)), s(java_fixed(target, 1))]));
            }

            lines.push(tr(
                "commands.tick.query.percentiles",
                &[s(java_fixed(f64::from(percentile(0.5)), 1)), s(java_fixed(f64::from(percentile(0.95)), 1)), s(java_fixed(f64::from(percentile(0.99)), 1)), Arg::Int(sorted.len() as i64)],
            ));

            several(lines)
        }
        ["rate", rate] => match rate.parse::<f32>() {
            Ok(rate) if (1.0..=10_000.0).contains(&rate) => {
                ticking.rate = rate;

                let body = ticking.state_packet();

                drop(ticking);
                broadcast(shared, None, body);

                tr("commands.tick.rate.success", &[s(java_fixed(f64::from(rate), 1))])
            }
            Ok(rate) if rate < 1.0 => fail("argument.float.low", &[Arg::Int(1)]),
            Ok(_) => fail("argument.float.big", &[Arg::Int(10_000)]),
            Err(_) => bad_syntax(),
        },
        ["freeze"] | ["unfreeze"] => {
            let frozen = parts[0] == "freeze";

            ticking.frozen = frozen;
            ticking.step_left = 0;

            // Заморозка обрывает разгон [проверить].
            ticking.sprint = None;

            let body = ticking.state_packet();

            drop(ticking);
            broadcast(shared, None, body);

            tr(if frozen { "commands.tick.status.frozen" } else { "commands.tick.status.running" }, &[])
        }
        ["step", "stop"] => {
            if ticking.step_left == 0 {
                return fail("commands.tick.step.stop.fail", &[]);
            }

            ticking.step_left = 0;

            let body = ticking.step_packet();

            drop(ticking);
            broadcast(shared, None, body);

            tr("commands.tick.step.stop.success", &[])
        }
        ["step", rest @ ..] if rest.len() <= 1 => {
            let steps = match rest.first() {
                Some(text) => match parse_time(text) {
                    Some(steps) if steps >= 1 => steps,
                    _ => return bad_syntax(),
                },
                None => 1,
            };

            if !ticking.frozen || ticking.sprint.is_some() {
                return fail("commands.tick.step.fail", &[]);
            }

            ticking.step_left = steps;

            let body = ticking.step_packet();

            drop(ticking);
            broadcast(shared, None, body);

            tr("commands.tick.step.success", &[Arg::Int(i64::from(steps))])
        }
        ["sprint", "stop"] => {
            let Some(sprint) = &mut ticking.sprint else {
                return fail("commands.tick.sprint.stop.fail", &[]);
            };

            // Отчёт придёт со следующим тактом, как по окончании.
            sprint.left = 0;

            tr("commands.tick.sprint.stop.success", &[])
        }
        ["sprint", rest @ ..] if rest.len() <= 1 => {
            let ticks = match rest.first() {
                Some(text) => match parse_time(text) {
                    Some(ticks) if ticks >= 1 => ticks,
                    _ => return bad_syntax(),
                },
                None => return bad_syntax(),
            };

            // Вики: «as fast as it can for the specified amount of ticks plus
            // one». Пробник: в начале Set Ticking State (как есть).
            ticking.sprint = Some(Sprint { left: i64::from(ticks) + 1, done: 0, started: Instant::now() });

            let body = ticking.state_packet();

            drop(ticking);
            broadcast(shared, None, body);

            tr("commands.tick.status.sprinting", &[])
        }
        _ => bad_syntax(),
    }
}

/// Несколько строк ответа: в консоль — через перевод строки, игроку — по
/// сообщению на строку (у оригинала — отдельные System Chat).
fn several(lines: Vec<Answer>) -> Answer {
    let reply = lines.iter().map(|line| line.reply.as_str()).collect::<Vec<_>>().join("\n");
    let mut joined = Vec::new();

    for line in &lines {
        if let Some(bytes) = &line.component {
            joined.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
            joined.extend_from_slice(bytes);
        }
    }

    Answer { reply, effect: Effect::None, component: Some([MANY_MARK.as_slice(), &joined].concat()), rich: Vec::new(), result: Some(1) }
}

/// Пометка «в ответе несколько сообщений»: дальше идут длина и тело
/// каждого (сетевой NBT не начинается с 0xFF).
pub const MANY_MARK: [u8; 1] = [0xFF];

/// Разбирает ответ на отдельные сообщения (см. `several`).
pub fn split_components(component: &[u8]) -> Vec<&[u8]> {
    let Some(mut rest) = component.strip_prefix(MANY_MARK.as_slice()) else {
        return vec![component];
    };

    let mut out = Vec::new();

    while rest.len() >= 4 {
        let length = u32::from_be_bytes([rest[0], rest[1], rest[2], rest[3]]) as usize;
        let Some(body) = rest.get(4..4 + length) else {
            break;
        };

        out.push(body);
        rest = &rest[4 + length..];
    }

    out
}

// ---------------------------------------------------------------------------
// Вход
// ---------------------------------------------------------------------------

/// Выполняет команду этой группы.
pub fn run(name: &str, parts: &[&str], source: &Source, shared: &Shared) -> Answer {
    match name {
        "worldborder" => worldborder(parts, source, shared),
        "tick" => tick(parts, shared),
        "clone" => clone(parts, source, shared),
        "attribute" => attribute(parts, source, shared),
        "enchant" => enchant(parts, source, shared),
        "forceload" => forceload(parts, source, shared),
        "random" => random(parts, source, shared),
        "rotate" => rotate(parts, source, shared),
        "spreadplayers" => spreadplayers(parts, source, shared),
        "spectate" => spectate(parts, source, shared),
        "ride" => ride(parts, source, shared),
        "locate" => locate(parts, source, shared),
        "fillbiome" => fillbiome(parts, source, shared),
        "place" => place(parts, source, shared),
        "loot" => loot(parts, source, shared),
        _ => bad_syntax(),
    }
}

// ---------------------------------------------------------------------------
// Цели
// ---------------------------------------------------------------------------

/// Имя игрока в ответе — простой текст [проверить: у оригинала к нему
/// прилагаются наведение и щелчок].
fn player_arg(name: &str) -> Arg {
    Arg::Text(name.to_string())
}

/// Цель команды: игрок или моб.
#[derive(Clone, Debug)]
pub(crate) enum Target {
    Player { uuid: [u8; 16], name: String, entity_id: i32, dimension: Dimension, at: (f64, f64, f64) },
    Mob { entity_id: i32, kind: crate::mob::Kind, dimension: Dimension, at: (f64, f64, f64) },
}

impl Target {
    pub(crate) fn entity_id(&self) -> i32 {
        match self {
            Target::Player { entity_id, .. } | Target::Mob { entity_id, .. } => *entity_id,
        }
    }

    fn dimension(&self) -> Dimension {
        match self {
            Target::Player { dimension, .. } | Target::Mob { dimension, .. } => *dimension,
        }
    }

    fn at(&self) -> (f64, f64, f64) {
        match self {
            Target::Player { at, .. } | Target::Mob { at, .. } => *at,
        }
    }

    /// Имя в ответе: игрок — полным текстом (щелчок, наведение), моб —
    /// переводом вида [проверить у мобов].
    fn name(&self) -> Arg {
        match self {
            Target::Player { name, uuid, .. } => Arg::Player(name.clone(), *uuid),
            Target::Mob { kind, .. } => Arg::Tr(format!("entity.minecraft.{}", kind.name()), Vec::new()),
        }
    }

    /// Имя простой строкой (пробник: `attribute`, `enchant … itemless` —
    /// `{"":"blackbox5"}`).
    fn plain_name(&self) -> Arg {
        match self {
            Target::Player { name, .. } => s(name),
            Target::Mob { kind, .. } => s(crate::survival::item_title(kind.name())),
        }
    }
}

/// Цели по записи: игроки (`find_targets`) и мобы (`@e[…]`, `@n[…]`).
pub(crate) fn targets(named: &str, source: &Source, shared: &Shared) -> Vec<Target> {
    let players: Vec<crate::players::Member> = shared.players.lock().expect("список игроков захвачен другим потоком").members().to_vec();
    let player_target = |uuid: &[u8; 16]| {
        players.iter().find(|member| &member.uuid == uuid).map(|member| Target::Player {
            uuid: member.uuid,
            name: member.name.clone(),
            entity_id: member.entity_id,
            dimension: member.dimension,
            at: (member.position.x, member.position.y, member.position.z),
        }).or_else(|| {
            let dimension = crate::commands::dimension_of(source, shared);
            let items = shared.items_in(dimension).lock().expect("предметы захвачены другим потоком");
            items.mobs().iter().find(|m| m.uuid == *uuid && !m.is_dead()).map(|m| Target::Mob { entity_id: m.entity_id, kind: m.kind, dimension, at: (m.x,m.y,m.z) })
        })
    };

    if named.starts_with("@e") || named.starts_with("@n") {
        let Some(selector) = crate::commands::Selector::parse(named) else {
            return Vec::new();
        };

        let origin = crate::commands::origin_of(source, shared);
        let dimension = crate::commands::dimension_of(source, shared);
        let mut out: Vec<(f64, Target)> = Vec::new();

        if selector.takes("player") {
            for member in players.iter().filter(|member| member.dimension == dimension) {
                let at = (member.position.x, member.position.y, member.position.z);

                if selector.reaches(origin, at) {
                    out.push((distance(origin, at), player_target(&member.uuid).expect("игрок из списка")));
                }
            }
        }

        let items = shared.items_in(dimension).lock().expect("предметы захвачены другим потоком");

        for mob in items.mobs().iter().filter(|mob| mob.dead_for == 0) {
            let at = (mob.x, mob.y, mob.z);

            if selector.takes_tagged(mob.kind.name(), &mob.tags) && selector.reaches(origin, at) {
                out.push((distance(origin, at), Target::Mob { entity_id: mob.entity_id, kind: mob.kind, dimension, at }));
            }
        }

        drop(items);

        if named.starts_with("@n") || named.contains("sort=nearest") {
            out.sort_by(|a, b| a.0.total_cmp(&b.0));
        }

        let limit = selector.limit.unwrap_or(usize::MAX);

        return out.into_iter().take(limit).map(|(_, target)| target).collect();
    }

    crate::commands::find_targets(named, source, shared).iter().filter_map(|(uuid, _)| player_target(uuid)).collect()
}

fn distance(a: (f64, f64, f64), b: (f64, f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2) + (a.2 - b.2).powi(2)).sqrt()
}

fn no_entity(named: &str) -> Answer {
    if named.starts_with("@e") || named.starts_with("@n") {
        fail("argument.entity.notfound.entity", &[])
    } else {
        fail("argument.entity.notfound.player", &[])
    }
}

// ---------------------------------------------------------------------------
// clone
// ---------------------------------------------------------------------------

/// Предел блоков у `clone` и `fill` (правило `max_block_modifications`,
/// по умолчанию 32 768 — вики, Commands/clone).
const BLOCK_LIMIT: i64 = 32_768;

/// Взятый из источника блок: сдвиг от угла, состояние и хранилище.
type Taken = ((i32, i32, i32), i32, Option<crate::containers::Container>);

fn clone(parts: &[&str], source: &Source, shared: &Shared) -> Answer {
    let origin = crate::commands::origin_of(source, shared);
    let mut rest = parts;
    let mut from_dimension = crate::commands::dimension_of(source, shared);

    if let ["from", name, tail @ ..] = rest {
        match Dimension::from_name(name) {
            Some(dimension) => from_dimension = dimension,
            None => return fail("argument.dimension.invalid", &[s(*name)]),
        }

        rest = tail;
    }

    let mut to_dimension = from_dimension;

    if rest.len() < 9 {
        return bad_syntax();
    }

    let (Some(begin), Some(end)) = (block_place(&rest[0..3], origin), block_place(&rest[3..6], origin)) else {
        return bad_syntax();
    };

    rest = &rest[6..];

    if let ["to", name, tail @ ..] = rest {
        match Dimension::from_name(name) {
            Some(dimension) => to_dimension = dimension,
            None => return fail("argument.dimension.invalid", &[s(*name)]),
        }

        rest = tail;
    }

    if rest.len() < 3 {
        return bad_syntax();
    }

    let Some(destination) = block_place(&rest[0..3], origin) else {
        return bad_syntax();
    };

    rest = &rest[3..];

    // [strict] [replace|masked|filtered <фильтр>] [force|move|normal]
    let mut strict = false;
    let mut mask = "replace";
    let mut filter: Option<&str> = None;
    let mut mode = "normal";

    if let ["strict", tail @ ..] = rest {
        strict = true;
        rest = tail;
    }

    match rest {
        ["replace" | "masked", tail @ ..] => {
            mask = rest[0];
            rest = tail;
        }
        ["filtered", wanted, tail @ ..] => {
            mask = "filtered";
            filter = Some(wanted);
            rest = tail;
        }
        _ => {}
    }

    match rest {
        [] => {}
        ["force" | "move" | "normal"] => mode = rest[0],
        _ => return bad_syntax(),
    }

    let low = (begin.0.min(end.0), begin.1.min(end.1), begin.2.min(end.2));
    let high = (begin.0.max(end.0), begin.1.max(end.1), begin.2.max(end.2));
    let size = (high.0 - low.0, high.1 - low.1, high.2 - low.2);
    let target_low = destination;
    let target_high = (destination.0 + size.0, destination.1 + size.1, destination.2 + size.2);
    let volume = (i64::from(size.0) + 1) * (i64::from(size.1) + 1) * (i64::from(size.2) + 1);

    if volume > BLOCK_LIMIT {
        return fail("commands.clone.toobig", &[Arg::Int(BLOCK_LIMIT), Arg::Int(volume)]);
    }

    let overlaps = from_dimension == to_dimension
        && low.0 <= target_high.0
        && target_low.0 <= high.0
        && low.1 <= target_high.1
        && target_low.1 <= high.1
        && low.2 <= target_high.2
        && target_low.2 <= high.2;

    if overlaps && mode != "force" {
        return fail("commands.clone.overlap", &[]);
    }

    let lowest = crate::world::MIN_Y;
    let highest = crate::world::MIN_Y + crate::world::WORLD_HEIGHT - 1;

    if low.1 < lowest || high.1 > highest || target_low.1 < lowest || target_high.1 > highest {
        return fail("argument.pos.outofworld", &[]);
    }

    let filter_state = match filter {
        Some(text) => match crate::blocks::state_from_text(text.split('{').next().unwrap_or(text)) {
            Some(state) => Some((state, text.contains('['))),
            None => return fail("argument.block.id.invalid", &[s(text)]),
        },
        None => None,
    };

    // Блоки и хранилища источника — сперва все (на случай force с
    // наложением), потом — в цель.
    let mut taken: Vec<Taken> = Vec::new();

    {
        let world = shared.world_in(from_dimension).lock().expect("мир захвачен другим потоком");

        for chunk_x in low.0 >> 4..=high.0 >> 4 {
            for chunk_z in low.2 >> 4..=high.2 >> 4 {
                if !world.has_chunk(chunk_x, chunk_z) {
                    return fail("argument.pos.unloaded", &[]);
                }
            }
        }
    }

    {
        let target = shared.world_in(to_dimension).lock().expect("мир захвачен другим потоком");

        for chunk_x in target_low.0 >> 4..=target_high.0 >> 4 {
            for chunk_z in target_low.2 >> 4..=target_high.2 >> 4 {
                if !target.has_chunk(chunk_x, chunk_z) {
                    return fail("argument.pos.unloaded", &[]);
                }
            }
        }
    }

    {
        let mut world = shared.world_in(from_dimension).lock().expect("мир захвачен другим потоком");

        for y in low.1..=high.1 {
            for z in low.2..=high.2 {
                for x in low.0..=high.0 {
                    let state = world.get_block(x, y, z);
                    let air = crate::blocks::block_at_state(state).is_none_or(|name| name == "air");

                    let keep = match mask {
                        "masked" => !air,
                        "filtered" => filter_state.is_some_and(|(wanted, exact)| if exact { wanted == state } else { crate::blocks::block_at_state(wanted) == crate::blocks::block_at_state(state) }),
                        _ => true,
                    };

                    if !keep {
                        continue;
                    }

                    let container = world.with_container(x, y, z, |container| container.clone());

                    taken.push(((x - low.0, y - low.1, z - low.2), state, container));
                }
            }
        }

        // `move`: источник становится воздухом (у оригинала — только то,
        // что перенесено) [проверить порядок обновлений].
        if mode == "move" {
            let air = crate::blocks::state_by_name("air").unwrap_or(0);

            for ((dx, dy, dz), _, _) in &taken {
                let (x, y, z) = (low.0 + dx, low.1 + dy, low.2 + dz);

                world.with_container(x, y, z, |container| container.slots.fill(None));
                world.set_block(x, y, z, air);
            }
        }
    }

    let mut world = shared.world_in(to_dimension).lock().expect("мир захвачен другим потоком");
    let mut changed = 0i64;
    let mut placed = Vec::new();

    for ((dx, dy, dz), state, container) in &taken {
        let (x, y, z) = (target_low.0 + dx, target_low.1 + dy, target_low.2 + dz);

        world.with_container(x, y, z, |container| container.slots.fill(None));

        let differs = world.get_block(x, y, z) != *state;

        if differs {
            world.note_commanded(x, y, z);
        }

        if differs && world.set_block(x, y, z, *state) {
            placed.push((x, y, z));
        }

        if let Some(container) = container {
            world.with_container(x, y, z, |there| *there = container.clone());
        }

        // Оригинал шлёт каждый скопированный блок, даже прежний (чёрный
        // ящик, cmd-clone: повтор той же копии).
        if !differs {
            world.announce_change(x, y, z, *state);
        }

        // Как у оригинала, считается каждый поставленный блок, даже если
        // он был таким же [проверить пробником].
        changed += 1;
    }

    if changed == 0 {
        return fail("commands.clone.failed", &[]);
    }

    world.take_spilled();
    crate::containers::after_change(&mut world);

    if !strict {
        for ((dx, dy, dz), _, _) in &taken {
            crate::placing::refresh_neighbours(&mut world, target_low.0 + dx, target_low.1 + dy, target_low.2 + dz);
        }

        crate::redstone::settle(&mut world);

        for place in placed {
            crate::redstone::command_placed(&mut world, place);
            crate::redstone::settle(&mut world);
        }
    }

    world.save_if_due();

    tr("commands.clone.success", &[Arg::Int(changed)])
}

// ---------------------------------------------------------------------------
// attribute
// ---------------------------------------------------------------------------

/// Правка атрибутов игрока: исполняет подключение (`Order::Attribute`).
#[derive(Clone, Debug, PartialEq)]
pub enum AttributeEdit {
    SetBase { attribute: i32, value: f64 },
    ResetBase { attribute: i32 },
    AddModifier { attribute: i32, modifier: crate::attributes::Modifier },
    RemoveModifier { attribute: i32, id: String },
}

struct AttributeSnapshot {
    current: crate::attributes::Attributes,
    pending: std::collections::VecDeque<AttributeEdit>,
}

impl Default for AttributeSnapshot {
    fn default() -> Self {
        Self { current: crate::attributes::Attributes::player(), pending: Default::default() }
    }
}

impl AttributeSnapshot {
    fn projected(&self) -> crate::attributes::Attributes {
        let mut attributes = self.current.clone();
        for edit in &self.pending { apply_attribute_edit(&mut attributes, edit); }
        attributes
    }
}

/// Какой атрибут правится.
pub fn edited_attribute(edit: &AttributeEdit) -> i32 {
    match edit {
        AttributeEdit::SetBase { attribute, .. } | AttributeEdit::ResetBase { attribute } | AttributeEdit::AddModifier { attribute, .. } | AttributeEdit::RemoveModifier { attribute, .. } => *attribute,
    }
}

/// Применяет правку к атрибутам (зовёт подключение игрока).
pub fn apply_attribute_edit(attributes: &mut crate::attributes::Attributes, edit: &AttributeEdit) {
    match edit {
        AttributeEdit::SetBase { attribute, value } => attributes.set_base(*attribute, *value),
        AttributeEdit::ResetBase { attribute } => attributes.set_base(*attribute, crate::attributes::player_base(*attribute)),
        AttributeEdit::AddModifier { attribute, modifier } => attributes.set_modifier(*attribute, modifier.clone()),
        AttributeEdit::RemoveModifier { attribute, id } => attributes.remove_modifier(*attribute, id),
    }
}

/// Записать атрибуты игрока для чтения командами (зовёт подключение).
pub fn publish_attributes(shared: &Shared, uuid: &[u8; 16], attributes: &crate::attributes::Attributes) {
    let mut known = shared.world_cmd.player_attributes.lock().expect("атрибуты захвачены другим потоком");

    let snapshot = known.entry(*uuid).or_default();
    if snapshot.current != *attributes {
        snapshot.current = attributes.clone();
    }
}

/// Подключение исполнило правку: снимаем её с ожидания вместе с новым снимком.
pub fn finish_attribute_edit(shared: &Shared, uuid: &[u8; 16], attributes: &crate::attributes::Attributes) {
    let mut known = shared.world_cmd.player_attributes.lock().expect("атрибуты захвачены другим потоком");
    let snapshot = known.entry(*uuid).or_default();
    // Подключение исполняет поручения строго в порядке очереди.
    snapshot.pending.pop_front();
    snapshot.current = attributes.clone();
}

pub fn forget_attributes(shared: &Shared, uuid: &[u8; 16]) {
    shared.world_cmd.player_attributes.lock().expect("атрибуты захвачены другим потоком").remove(uuid);
}

/// Есть ли атрибут у игрока (вики, Attribute: у игрока нет «follow_range»,
/// «flying_speed», «spawn_reinforcements», «tempt_range») [проверить].
fn player_has(attribute: i32) -> bool {
    !matches!(crate::attributes::name_of(attribute), Some("follow_range" | "flying_speed" | "spawn_reinforcements" | "tempt_range") | None)
}

/// Значение числа в ответе `attribute` — как у оригинала, `%s` от double
/// (Ява: кратчайшая запись с «.0» у целых) [проверить].
fn java_double(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e7 {
        format!("{:.1}", value)
    } else {
        format!("{}", value)
    }
}

fn attribute(parts: &[&str], source: &Source, shared: &Shared) -> Answer {
    let [named, attribute_name, rest @ ..] = parts else {
        return bad_syntax();
    };

    let Some(attribute) = crate::attributes::by_name(attribute_name) else {
        return fail("argument.resource.not_found", &[s(*attribute_name), s("minecraft:attribute")]);
    };

    let found = targets(named, source, shared);

    let Some(target) = found.first().cloned() else {
        return no_entity(named);
    };

    if found.len() > 1 {
        return fail("argument.entity.toomany", &[]);
    }

    let attribute_arg = Arg::Tr(format!("attribute.name.{}", crate::attributes::name_of(attribute).unwrap_or("")), Vec::new());

    let Target::Player { uuid, .. } = &target else {
        // Мобы: атрибуты у нас зашиты в виды, команде пока не видны.
        return fail("commands.attribute.failed.entity", &[target.plain_name()]);
    };

    if !player_has(attribute) {
        return fail("commands.attribute.failed.no_attribute", &[target.plain_name(), attribute_arg]);
    }

    // tmp/bb/ours-cmd-attribute.json: пакет команд пришёл разом на +42;
    // get/remove читали старый снимок после успешного add. Учитываем принятые,
    // но ещё не исполненные правки; vanilla-cmd-attribute.json даёт 0.3.
    let mut known = shared.world_cmd.player_attributes.lock().expect("атрибуты захвачены другим потоком");
    let snapshot = known.entry(*uuid).or_default();
    snapshot.current.set_catalog(shared.catalog());
    let attributes = snapshot.projected();
    let entry = attributes.get(attribute).cloned().unwrap_or(crate::attributes::Attribute { id: attribute, base: crate::attributes::player_base(attribute), modifiers: Vec::new() });
    let scaled = |value: f64, scale: Option<&&str>| -> Result<f64, Answer> {
        match scale {
            Some(text) => text.parse::<f64>().map(|scale| value * scale).map_err(|_| bad_syntax()),
            None => Ok(value),
        }
    };

    let mut edit = |edit: AttributeEdit| {
        snapshot.pending.push_back(edit.clone());
        shared.players.lock().expect("список игроков захвачен другим потоком").order(uuid, Order::Attribute(edit));
    };

    match rest {
        ["get", scale @ ..] if scale.len() <= 1 => match scaled(attributes.value(attribute), scale.first()) {
            Ok(_) => tr("commands.attribute.value.get.success", &[attribute_arg, target.plain_name(), Arg::Double(attributes.value(attribute))]),
            Err(answer) => answer,
        },
        ["base", "get", scale @ ..] if scale.len() <= 1 => match scaled(entry.base, scale.first()) {
            Ok(_) => tr("commands.attribute.base_value.get.success", &[attribute_arg, target.plain_name(), Arg::Double(entry.base)]),
            Err(answer) => answer,
        },
        ["base", "set", value] => {
            let Ok(value) = value.parse::<f64>() else {
                return bad_syntax();
            };

            edit(AttributeEdit::SetBase { attribute, value });

            tr("commands.attribute.base_value.set.success", &[attribute_arg, target.plain_name(), Arg::Double(value)])
        }
        ["base", "reset"] => {
            let value = crate::attributes::player_base(attribute);

            edit(AttributeEdit::ResetBase { attribute });

            tr("commands.attribute.base_value.reset.success", &[attribute_arg, target.plain_name(), Arg::Double(value)])
        }
        ["modifier", "add", id, amount, operation] => {
            let (Ok(amount), Some(operation)) = (amount.parse::<f64>(), crate::attributes::Operation::from_name(operation)) else {
                return bad_syntax();
            };

            let id = namespaced(id);

            if entry.modifiers.iter().any(|modifier| modifier.id == id) {
                return fail("commands.attribute.failed.modifier_already_present", &[s(&id), attribute_arg, target.plain_name()]);
            }

            edit(AttributeEdit::AddModifier { attribute, modifier: crate::attributes::Modifier::new(&id, amount, operation) });

            tr("commands.attribute.modifier.add.success", &[s(&id), attribute_arg, target.plain_name()])
        }
        ["modifier", "remove", id] => {
            let id = namespaced(id);

            if !entry.modifiers.iter().any(|modifier| modifier.id == id) {
                return fail("commands.attribute.failed.no_modifier", &[attribute_arg, target.plain_name(), s(&id)]);
            }

            edit(AttributeEdit::RemoveModifier { attribute, id: id.clone() });

            tr("commands.attribute.modifier.remove.success", &[s(&id), attribute_arg, target.plain_name()])
        }
        ["modifier", "value", "get", id, scale @ ..] if scale.len() <= 1 => {
            let id = namespaced(id);

            let Some(modifier) = entry.modifiers.iter().find(|modifier| modifier.id == id) else {
                return fail("commands.attribute.failed.no_modifier", &[attribute_arg, target.plain_name(), s(&id)]);
            };

            match scaled(modifier.amount, scale.first()) {
                Ok(_) => tr("commands.attribute.modifier.value.get.success", &[s(&id), attribute_arg, target.plain_name(), Arg::Double(modifier.amount)]),
                Err(answer) => answer,
            }
        }
        _ => bad_syntax(),
    }
}

/// Опознаватель с пространством имён: `x` → `minecraft:x`.
fn namespaced(id: &str) -> String {
    if id.contains(':') { id.to_string() } else { format!("minecraft:{}", id) }
}

// ---------------------------------------------------------------------------
// enchant
// ---------------------------------------------------------------------------

/// Подходит ли зачарование предмету (вики, страницы зачарований:
/// «Primary/Supported items») — по имени предмета.
pub(crate) fn enchantable(enchantment: &str, item: &str) -> bool {
    let sword = item.ends_with("_sword");
    let axe = item.ends_with("_axe");
    let spear = item.ends_with("_spear");
    let pickaxe = item.ends_with("_pickaxe");
    let shovel = item.ends_with("_shovel");
    let hoe = item.ends_with("_hoe");
    let helmet = item.ends_with("_helmet") || item == "turtle_helmet";
    let chest = item.ends_with("_chestplate");
    let legs = item.ends_with("_leggings");
    let boots = item.ends_with("_boots");
    let armor = helmet || chest || legs || boots;
    let tool = pickaxe || shovel || axe || hoe;
    let durable = armor || tool || sword || spear || matches!(item, "bow" | "crossbow" | "trident" | "fishing_rod" | "shears" | "flint_and_steel" | "carrot_on_a_stick" | "warped_fungus_on_a_stick" | "shield" | "elytra" | "brush" | "mace");
    match enchantment {
        "sharpness" => sword || axe || spear,
        "smite" | "bane_of_arthropods" => sword || axe || spear || item == "mace",
        "knockback" | "fire_aspect" | "looting" => sword || spear || (enchantment == "fire_aspect" && item == "mace"),
        "sweeping_edge" => sword,
        "efficiency" => tool || item == "shears",
        "silk_touch" | "fortune" => tool,
        "unbreaking" => durable,
        "mending" => durable,
        "vanishing_curse" => durable || matches!(item, "compass" | "carved_pumpkin") || item.ends_with("_head") || item.ends_with("_skull"),
        "binding_curse" => armor || item == "elytra" || item == "carved_pumpkin" || item.ends_with("_head") || item.ends_with("_skull"),
        "protection" | "fire_protection" | "blast_protection" | "projectile_protection" => armor,
        "thorns" => armor,
        "respiration" | "aqua_affinity" => helmet,
        "swift_sneak" => legs,
        "feather_falling" | "depth_strider" | "frost_walker" | "soul_speed" => boots,
        "power" | "punch" | "flame" | "infinity" => item == "bow",
        "multishot" | "piercing" | "quick_charge" => item == "crossbow",
        "loyalty" | "riptide" | "channeling" | "impaling" => item == "trident",
        "luck_of_the_sea" | "lure" => item == "fishing_rod",
        "density" | "breach" | "wind_burst" => item == "mace",
        "lunge" => spear,
        _ => false,
    }
}

/// Несовместимые зачарования (вики, Enchanting: «Incompatible enchantments»).
pub(crate) fn exclusive(a: &str, b: &str) -> bool {
    const SETS: [&[&str]; 8] = [
        &["sharpness", "smite", "bane_of_arthropods", "density", "breach", "impaling"],
        &["protection", "fire_protection", "blast_protection", "projectile_protection"],
        &["depth_strider", "frost_walker"],
        &["silk_touch", "fortune"],
        &["infinity", "mending"],
        &["multishot", "piercing"],
        &["riptide", "loyalty"],
        &["riptide", "channeling"],
    ];

    a != b && SETS.iter().any(|set| set.contains(&a) && set.contains(&b))
}

/// Команда добавляет новые чары, но не заменяет уже имеющиеся, даже
/// тем же уровнем или нулём (чёрный ящик, enchant-command-validation).
/// Наковальня, напротив, объединяет одинаковые чары через `exclusive`.
fn can_command_enchant(name: &str, item: &str, existing: &[(u16, i32)]) -> bool {
    enchantable(name, item) && !existing.iter().any(|(other, _)| {
        crate::stackdata::ENCHANTMENTS.get(*other as usize).is_some_and(|(other, _)| *other == name || exclusive(name, other))
    })
}

/// Название зачарования с уровнем (уровень не пишется, только если и он,
/// и наибольший — 1); проклятия — красным.
fn enchant_name(name: &str, level: i32, max: i32) -> Arg {
    let level = (level != 1 || max != 1).then_some(level);

    Arg::Raw(Box::new(RawText::Enchantment { name: name.to_string(), level, curse: name.ends_with("_curse") }))
}

/// Общая экипировка целей /item: тот же selector и limit, что у других
/// команд сущностей. Именованных игроков обрабатывает прежняя ветка.
pub(crate) fn replace_equipment(named: &str, slot: u8, stack: Option<crate::inventory::Stack>, source: &Source, shared: &Shared) -> Option<Answer> {
    if !(named.starts_with("@e") || named.starts_with("@n")) { return None; }
    let found = targets(named, source, shared);
    if found.is_empty() { return Some(no_entity(named)); }
    let mut changed = 0;
    for target in found {
        match target {
            Target::Mob { entity_id, dimension, .. } => {
                let mut items = shared.items_in(dimension).lock().expect("сущности");
                if let Some(mob) = items.mobs_mut().iter_mut().find(|mob| mob.entity_id == entity_id)
                    && crate::mob::equipment::set_slot(mob, slot, stack) { changed += 1; }
            }
            Target::Player { uuid, .. } => {
                let order = if slot == 0 { Order::Hold { stack } } else {
                    Order::SetSlot { slot: match slot { 1 => crate::inventory::OFFHAND, 2 => 8, 3 => 7, 4 => 6, _ => 5 }, stack }
                };
                shared.players.lock().expect("игроки").order(&uuid, order);
                changed += 1;
            }
        }
    }
    Some(Answer::saying(format!("Экипировка изменена у сущностей: {changed}")))
}

fn enchant(parts: &[&str], source: &Source, shared: &Shared) -> Answer {
    let (named, enchantment, level) = match parts {
        [named, enchantment] => (*named, *enchantment, 1),
        [named, enchantment, level] => match level.parse::<i32>() {
            Ok(level) if level >= 0 => (*named, *enchantment, level),
            _ => return bad_syntax(),
        },
        _ => return bad_syntax(),
    };

    let name = enchantment.strip_prefix("minecraft:").unwrap_or(enchantment);

    let Some(number) = crate::stackdata::enchantment_number(name) else {
        return fail("argument.resource.not_found", &[s(enchantment), s("minecraft:enchantment")]);
    };

    let max = crate::stackdata::ENCHANTMENTS[number as usize].1;

    if level > max {
        return fail("commands.enchant.failed.level", &[Arg::Int(i64::from(level)), Arg::Int(i64::from(max))]);
    }

    let found = targets(named, source, shared);

    if found.is_empty() {
        return no_entity(named);
    }

    let mut applied = 0;
    let mut last_failure = None;

    for target in &found {
        let held = match target {
            Target::Player { uuid, .. } => shared.players.lock().expect("список игроков захвачен другим потоком").members().iter().find(|member| &member.uuid == uuid).and_then(|member| member.held),
            Target::Mob { entity_id, dimension, .. } => shared.items_in(*dimension).lock().expect("сущности").mobs().iter()
                .find(|mob| mob.entity_id == *entity_id).and_then(|mob| crate::mob::equipment::slot(mob, 0)),
        };

        let Some(stack) = held else {
            last_failure = Some(fail("commands.enchant.failed.itemless", &[target.plain_name()]));
            continue;
        };

        let item = crate::blocks::item_name(stack.item).unwrap_or("");
        let existing = stack.extras();
        if !can_command_enchant(name, item, &existing.enchants) {
            // Пробник: название предмета строкой по-английски («Diamond Sword»).
            let item_arg = s(crate::survival::item_title(item));

            last_failure = Some(fail("commands.enchant.failed.incompatible", &[item_arg]));
            continue;
        }

        // Нулевой уровень допустим, но на незачарованном предмете ничего
        // не меняет: оригинал не присылает повторный слот инвентаря.
        if level > 0 {
            match target {
                Target::Player { uuid, .. } => { shared.players.lock().expect("список игроков захвачен другим потоком").order(uuid, Order::Enchant { enchantment: i32::from(number), level }); }
                Target::Mob { entity_id, dimension, .. } => {
                    let mut items = shared.items_in(*dimension).lock().expect("сущности");
                    if let Some(mob) = items.mobs_mut().iter_mut().find(|mob| mob.entity_id == *entity_id) {
                        let mut data = stack.extras(); data.set_enchant(number, level);
                        crate::mob::equipment::set_slot(mob, 0, Some(stack.with_extras(data)));
                    }
                }
            }
        }
        applied += 1;
    }

    if applied == 0 {
        return match (found.len(), last_failure) {
            (1, Some(answer)) => answer,
            _ => fail("commands.enchant.failed", &[]),
        };
    }

    let shown = enchant_name(name, level, max);

    if found.len() == 1 {
        tr("commands.enchant.success.single", &[shown, found[0].name()])
    } else {
        tr("commands.enchant.success.multiple", &[shown, Arg::Int(applied)])
    }
}

// ---------------------------------------------------------------------------
// forceload
// ---------------------------------------------------------------------------

/// Наибольшее число чанков в одной команде (вики, Commands/forceload: 256).
const FORCELOAD_LIMIT: i64 = 256;

/// Удержанные чанки измерения (такт не выгружает их и даёт им случайные
/// такты, как чанкам рядом с игроком).
pub fn forced_chunks(shared: &Shared, dimension: Dimension) -> Vec<(i32, i32)> {
    shared.world_cmd.forced.lock().expect("удержанные чанки захвачены другим потоком").iter().filter(|(index, _, _)| *index == dimension.index()).map(|(_, x, z)| (*x, *z)).collect()
}

fn chunk_pos(x: i32, z: i32) -> String {
    format!("[{}, {}]", x, z)
}

fn forceload(parts: &[&str], source: &Source, shared: &Shared) -> Answer {
    let dimension = crate::commands::dimension_of(source, shared);
    let dimension_arg = s(format!("minecraft:{}", dimension.name()));
    let origin = crate::commands::origin_of(source, shared);
    let column = |x: &str, z: &str| -> Option<(i32, i32)> { Some((block_coordinate(x, origin.0)?.div_euclid(16), block_coordinate(z, origin.2)?.div_euclid(16))) };
    let mut forced = shared.world_cmd.forced.lock().expect("удержанные чанки захвачены другим потоком");
    let index = dimension.index();

    match parts {
        ["query"] => {
            // Пробник: порядок — по z, затем по x.
            let mut chunks: Vec<(i32, i32)> = forced.iter().filter(|(at, _, _)| *at == index).map(|(_, x, z)| (*z, *x)).collect();

            chunks.sort_unstable();

            let list: Vec<String> = chunks.iter().map(|(z, x)| chunk_pos(*x, *z)).collect();

            match list.len() {
                0 => tr("commands.forceload.added.none", &[dimension_arg]),
                1 => tr("commands.forceload.list.single", &[dimension_arg, s(list.join(", "))]),
                count => tr("commands.forceload.list.multiple", &[Arg::Int(count as i64), dimension_arg, s(list.join(", "))]),
            }
        }
        ["query", x, z] => {
            let Some((chunk_x, chunk_z)) = column(x, z) else {
                return bad_syntax();
            };

            if forced.contains(&(index, chunk_x, chunk_z)) {
                tr("commands.forceload.query.success", &[s(chunk_pos(chunk_x, chunk_z)), dimension_arg])
            } else {
                fail("commands.forceload.query.failure", &[s(chunk_pos(chunk_x, chunk_z)), dimension_arg])
            }
        }
        ["remove", "all"] => {
            forced.retain(|(at, _, _)| *at != index);

            tr("commands.forceload.removed.all", &[dimension_arg])
        }
        ["add" | "remove", x1, z1, rest @ ..] if rest.is_empty() || rest.len() == 2 => {
            let adding = parts[0] == "add";
            let Some(from) = column(x1, z1) else {
                return bad_syntax();
            };
            let to = match rest {
                [x2, z2] => match column(x2, z2) {
                    Some(to) => to,
                    None => return bad_syntax(),
                },
                _ => from,
            };

            let (low, high) = ((from.0.min(to.0), from.1.min(to.1)), (from.0.max(to.0), from.1.max(to.1)));
            let count = (i64::from(high.0 - low.0) + 1) * (i64::from(high.1 - low.1) + 1);

            if count > FORCELOAD_LIMIT {
                return fail("commands.forceload.toobig", &[Arg::Int(FORCELOAD_LIMIT), Arg::Int(count)]);
            }

            let mut changed = 0;
            let mut last = low;

            for chunk_x in low.0..=high.0 {
                for chunk_z in low.1..=high.1 {
                    let done = if adding { forced.insert((index, chunk_x, chunk_z)) } else { forced.remove(&(index, chunk_x, chunk_z)) };

                    if done {
                        changed += 1;
                        last = (chunk_x, chunk_z);
                    }
                }
            }

            drop(forced);

            if adding && changed > 0 {
                // Удержанный чанк загружается сразу.
                let mut world = shared.world_in(dimension).lock().expect("мир захвачен другим потоком");

                for chunk_x in low.0..=high.0 {
                    for chunk_z in low.1..=high.1 {
                        world.ensure(chunk_x, chunk_z);
                    }
                }
            }

            match (adding, changed) {
                (true, 0) => fail("commands.forceload.added.failure", &[]),
                (false, 0) => fail("commands.forceload.removed.failure", &[]),
                (true, 1) => tr("commands.forceload.added.single", &[s(chunk_pos(last.0, last.1)), dimension_arg]),
                (false, 1) => tr("commands.forceload.removed.single", &[s(chunk_pos(last.0, last.1)), dimension_arg]),
                (true, _) => tr("commands.forceload.added.multiple", &[Arg::Int(changed), dimension_arg, s(chunk_pos(low.0, low.1)), s(chunk_pos(high.0, high.1))]),
                (false, _) => tr("commands.forceload.removed.multiple", &[Arg::Int(changed), dimension_arg, s(chunk_pos(low.0, low.1)), s(chunk_pos(high.0, high.1))]),
            }
        }
        _ => bad_syntax(),
    }
}

// ---------------------------------------------------------------------------
// random
// ---------------------------------------------------------------------------

/// Случайное число из [low, high] — от времени и счётчика последовательности
/// (своё, с оригиналом не совпадает: его числа зависят от семени мира).
fn roll(low: i64, high: i64, salt: u64) -> i64 {
    let mut value = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(1, |passed| passed.as_nanos() as u64) ^ salt.wrapping_mul(0x9E37_79B9_7F4A_7C15);

    value ^= value >> 33;
    value = value.wrapping_mul(0xFF51_AFD7_ED55_8CCD);
    value ^= value >> 33;

    low + (value % (high - low + 1) as u64) as i64
}

fn parse_range(text: &str) -> Option<(i64, i64)> {
    let (low, high) = text.split_once("..")?;

    Some((low.parse().ok()?, high.parse().ok()?))
}

fn random(parts: &[&str], source: &Source, shared: &Shared) -> Answer {
    match parts {
        ["value" | "roll", range, sequence @ ..] if sequence.len() <= 1 => {
            let Some((low, high)) = parse_range(range) else {
                return bad_syntax();
            };

            let span = high - low + 1;

            if span < 2 {
                return fail("commands.random.error.range_too_small", &[]);
            }

            if span > 2_147_483_646 {
                return fail("commands.random.error.range_too_large", &[]);
            }

            let salt = match sequence.first() {
                Some(name) => {
                    let mut sequences = shared.world_cmd.sequences.lock().expect("последовательности захвачены другим потоком");
                    let counter = sequences.entry(namespaced(name)).or_insert(0);

                    *counter += 1;
                    *counter
                }
                None => 0,
            };

            let value = roll(low, high, salt);

            if parts[0] == "value" {
                return tr("commands.random.sample.success", &[Arg::Int(value)]);
            }

            // `roll` слышат все (вики, Commands/random: «broadcast»).
            let who = match source {
                Source::Player { name, .. } => player_arg(name),
                Source::Console => s("Server"),
                // Командный блок подписывается своим именем («@» по умолчанию).
                Source::Block { name, .. } => s(name),
            };
            let message = component("commands.random.roll", &[who.clone(), Arg::Int(value), Arg::Int(low), Arg::Int(high)]);

            broadcast(shared, None, chat_packet(message));

            Answer::saying(english("commands.random.roll", &[who, Arg::Int(value), Arg::Int(low), Arg::Int(high)]))
                .silent_for_player()
        }
        ["reset", "*", ..] => {
            let mut sequences = shared.world_cmd.sequences.lock().expect("последовательности захвачены другим потоком");
            let count = sequences.len();

            sequences.clear();

            tr("commands.random.reset.all.success", &[Arg::Int(count as i64)])
        }
        ["reset", name, ..] => {
            let name = namespaced(name);

            shared.world_cmd.sequences.lock().expect("последовательности захвачены другим потоком").insert(name.clone(), 0);

            tr("commands.random.reset.success", &[s(name)])
        }
        _ => bad_syntax(),
    }
}

trait Quiet {
    fn silent_for_player(self) -> Self;
}

impl Quiet for Answer {
    /// Ответ только в консоль: игроку сообщение уже ушло рассылкой.
    fn silent_for_player(mut self) -> Self {
        self.component = Some(Vec::new());
        self
    }
}

// ---------------------------------------------------------------------------
// rotate
// ---------------------------------------------------------------------------

/// Поворот: число или `~` со сдвигом от прежнего.
fn angle(text: &str, from: f32) -> Option<f32> {
    match text.strip_prefix('~') {
        Some("") => Some(from),
        Some(shift) => Some(from + shift.parse::<f32>().ok()?),
        None => text.parse::<f32>().ok(),
    }
}

/// Взгляд из глаз `eyes` на точку.
fn facing(eyes: (f64, f64, f64), at: (f64, f64, f64)) -> (f32, f32) {
    let (dx, dy, dz) = (at.0 - eyes.0, at.1 - eyes.1, at.2 - eyes.2);
    let flat = (dx * dx + dz * dz).sqrt();
    let yaw = (dz.atan2(dx).to_degrees() - 90.0) as f32;
    let pitch = (-(dy.atan2(flat).to_degrees())) as f32;

    (wrap_degrees(yaw), wrap_degrees(pitch))
}

fn wrap_degrees(value: f32) -> f32 {
    let mut value = value % 360.0;

    if value >= 180.0 {
        value -= 360.0;
    }

    if value < -180.0 {
        value += 360.0;
    }

    value
}

/// Высота глаз игрока стоя (вики, Player: 1,62).
const EYE_HEIGHT: f64 = 1.62;

fn rotate(parts: &[&str], source: &Source, shared: &Shared) -> Answer {
    let [named, rest @ ..] = parts else {
        return bad_syntax();
    };

    let found = targets(named, source, shared);

    let Some(target) = found.first().cloned() else {
        return no_entity(named);
    };

    if found.len() > 1 {
        return fail("argument.entity.toomany", &[]);
    }

    let (yaw_now, pitch_now) = match &target {
        Target::Player { uuid, .. } => shared.players.lock().expect("список игроков захвачен другим потоком").members().iter().find(|member| &member.uuid == uuid).map_or((0.0, 0.0), |member| (member.position.yaw, member.position.pitch)),
        Target::Mob { .. } => (0.0, 0.0),
    };

    let at = target.at();
    let eyes = (at.0, at.1 + EYE_HEIGHT, at.2);

    let (yaw, pitch) = match rest {
        ["facing", "entity", other, anchor @ ..] if anchor.len() <= 1 => {
            let Some(other) = targets(other, source, shared).into_iter().next() else {
                return no_entity(other);
            };

            let point = other.at();
            let point = if anchor.first() == Some(&"eyes") { (point.0, point.1 + EYE_HEIGHT, point.2) } else { point };

            facing(eyes, point)
        }
        ["facing", x, y, z] => {
            let origin = crate::commands::origin_of(source, shared);

            let (Some(x), Some(y), Some(z)) = (coordinate(x, origin.0, true), coordinate(y, origin.1, false), coordinate(z, origin.2, true)) else {
                return bad_syntax();
            };

            facing(eyes, (x, y, z))
        }
        [yaw, pitch] => {
            let (Some(yaw), Some(pitch)) = (angle(yaw, yaw_now), angle(pitch, pitch_now)) else {
                return bad_syntax();
            };

            (yaw, pitch)
        }
        _ => return bad_syntax(),
    };

    match &target {
        Target::Player { uuid, .. } => {
            shared.players.lock().expect("список игроков захвачен другим потоком").order(uuid, Order::Look { yaw, pitch: pitch.clamp(-90.0, 90.0) });
        }
        Target::Mob { entity_id, dimension, .. } => {
            let mut items = shared.items_in(*dimension).lock().expect("предметы захвачены другим потоком");

            if let Some(mob) = items.mobs_mut().iter_mut().find(|mob| mob.entity_id == *entity_id) {
                mob.yaw = yaw;
                mob.head_yaw = yaw;
                mob.pitch = pitch.clamp(-90.0, 90.0);
            }
        }
    }

    tr("commands.rotate.success", &[target.name()])
}

// ---------------------------------------------------------------------------
// spreadplayers
// ---------------------------------------------------------------------------

fn spreadplayers(parts: &[&str], source: &Source, shared: &Shared) -> Answer {
    let origin = crate::commands::origin_of(source, shared);

    let (x, z, spread, range, max_height, rest) = match parts {
        [x, z, spread, range, "under", height, rest @ ..] => (*x, *z, *spread, *range, height.parse::<i32>().ok(), rest),
        [x, z, spread, range, rest @ ..] => (*x, *z, *spread, *range, None, rest),
        _ => return bad_syntax(),
    };

    let [teams, named] = rest else {
        return bad_syntax();
    };

    let (Some(center_x), Some(center_z), Ok(spread), Ok(range)) = (coordinate(x, origin.0, true), coordinate(z, origin.2, true), spread.parse::<f32>(), range.parse::<f32>()) else {
        return bad_syntax();
    };

    if *teams != "true" && *teams != "false" {
        return bad_syntax();
    }

    if range < spread + 1.0 {
        return fail("argument.float.low", &[s(java_fixed(f64::from(spread) + 1.0, 1)), s(java_fixed(f64::from(range), 1))]);
    }

    let found = targets(named, source, shared);

    if found.is_empty() {
        return no_entity(named);
    }

    let dimension = crate::commands::dimension_of(source, shared);
    let top = max_height.unwrap_or(crate::world::MIN_Y + crate::world::WORLD_HEIGHT);
    let mut spots: Vec<(f64, f64, f64)> = Vec::new();
    let mut salt = 1u64;

    // Случайные места в квадрате ±range вокруг середины, не ближе spread друг
    // к другу, на верхнем твёрдом блоке ниже `top` (вики, Commands/spreadplayers).
    for _ in &found {
        let mut chosen = None;

        for _ in 0..1000 {
            salt += 1;

            let px = (center_x - f64::from(range)).floor() as i64 + roll(0, (f64::from(range) * 2.0) as i64, salt);
            let pz = (center_z - f64::from(range)).floor() as i64 + roll(0, (f64::from(range) * 2.0) as i64, salt * 31);
            let (px, pz) = (px as f64 + 0.5, pz as f64 + 0.5);

            if spots.iter().any(|(x, _, z)| ((x - px).powi(2) + (z - pz).powi(2)).sqrt() < f64::from(spread)) {
                continue;
            }

            let mut world = shared.world_in(dimension).lock().expect("мир захвачен другим потоком");
            let (block_x, block_z) = (px.floor() as i32, pz.floor() as i32);

            world.ensure(block_x.div_euclid(16), block_z.div_euclid(16));

            let ground = (crate::world::MIN_Y..top).rev().find(|y| crate::blocks::block_at_state(world.get_block(block_x, *y, block_z)).is_some_and(|name| name != "air" && name != "cave_air" && name != "void_air"));

            let Some(ground) = ground else {
                continue;
            };

            let below = crate::blocks::block_at_state(world.get_block(block_x, ground, block_z)).unwrap_or("air");

            if below == "lava" || below == "fire" || below == "magma_block" {
                continue;
            }

            chosen = Some((px, f64::from(ground + 1), pz));
            break;
        }

        let Some(spot) = chosen else {
            return fail("commands.spreadplayers.failed.entities", &[Arg::Int(found.len() as i64), s(java_fixed(center_x, 2)), s(java_fixed(center_z, 2)), s(java_fixed(f64::from(range), 2))]);
        };

        spots.push(spot);
    }

    let mut total = 0.0;

    for (index, (target, spot)) in found.iter().zip(&spots).enumerate() {
        let nearest = spots.iter().enumerate().filter(|(other, _)| *other != index).map(|(_, other)| ((other.0 - spot.0).powi(2) + (other.2 - spot.2).powi(2)).sqrt()).fold(f64::MAX, f64::min);

        if nearest < f64::MAX {
            total += nearest;
        }

        match target {
            Target::Player { uuid, .. } => {
                shared.players.lock().expect("список игроков захвачен другим потоком").order(uuid, Order::Teleport { x: spot.0, y: spot.1, z: spot.2 });
            }
            Target::Mob { entity_id, dimension, .. } => {
                let mut items = shared.items_in(*dimension).lock().expect("предметы захвачены другим потоком");

                if let Some(mob) = items.mobs_mut().iter_mut().find(|mob| mob.entity_id == *entity_id) {
                    (mob.x, mob.y, mob.z) = *spot;
                }
            }
        }
    }

    let average = if found.len() > 1 { total / found.len() as f64 } else { 0.0 };

    tr("commands.spreadplayers.success.entities", &[Arg::Int(found.len() as i64), s(java_fixed(center_x, 2)), s(java_fixed(center_z, 2)), s(java_fixed(average, 2))])
}

// ---------------------------------------------------------------------------
// spectate
// ---------------------------------------------------------------------------

fn spectate(parts: &[&str], source: &Source, shared: &Shared) -> Answer {
    let (target, viewer) = match parts {
        [] => (None, None),
        [target] => (Some(*target), None),
        [target, viewer] => (Some(*target), Some(*viewer)),
        _ => return bad_syntax(),
    };

    let viewer = match viewer {
        Some(named) => match targets(named, source, shared).into_iter().find(|target| matches!(target, Target::Player { .. })) {
            Some(found) => found,
            None => return no_entity(named),
        },
        None => match source {
            Source::Player { name, .. } => match targets(name, source, shared).into_iter().next() {
                Some(found) => found,
                None => return fail("permissions.requires.player", &[]),
            },
            Source::Console | Source::Block { .. } => return fail("permissions.requires.player", &[]),
        },
    };

    let Target::Player { uuid, entity_id: own_id, .. } = &viewer else {
        return bad_syntax();
    };

    let mode = shared.players.lock().expect("список игроков захвачен другим потоком").members().iter().find(|member| &member.uuid == uuid).map_or(0, |member| member.game_mode);

    let Some(named) = target else {
        shared.players.lock().expect("список игроков захвачен другим потоком").order(uuid, Order::Spectate { entity_id: None });

        return tr("commands.spectate.success.stopped", &[]);
    };

    let Some(watched) = targets(named, source, shared).into_iter().next() else {
        return no_entity(named);
    };

    if watched.entity_id() == *own_id {
        return fail("commands.spectate.self", &[]);
    }

    if mode != crate::commands::GAME_MODE_SPECTATOR {
        return fail("commands.spectate.not_spectator", &[viewer.name()]);
    }

    shared.players.lock().expect("список игроков захвачен другим потоком").order(uuid, Order::Spectate { entity_id: Some(watched.entity_id()) });

    tr("commands.spectate.success.started", &[watched.name()])
}

// ---------------------------------------------------------------------------
// ride
// ---------------------------------------------------------------------------

/// Set Passengers: транспорт и его наездники.
fn passengers_packet(vehicle: i32, riders: &[i32]) -> Vec<u8> {
    let mut body = encode_varint(SET_PASSENGERS);

    body.extend_from_slice(&encode_varint(vehicle));
    body.extend_from_slice(&encode_varint(riders.len() as i32));

    for rider in riders {
        body.extend_from_slice(&encode_varint(*rider));
    }

    body
}

/// Наездники транспорта по таблице поездок.
fn riders_of(rides: &HashMap<i32, i32>, vehicle: i32) -> Vec<i32> {
    let mut riders: Vec<i32> = rides.iter().filter(|(_, on)| **on == vehicle).map(|(rider, _)| *rider).collect();

    riders.sort_unstable();
    riders
}

/// Наездники сущности, если есть — для показа входящему игроку.
pub fn passenger_packets(shared: &Shared) -> Vec<Vec<u8>> {
    let rides = shared.world_cmd.rides.lock().expect("поездки захвачены другим потоком");
    let vehicles: BTreeSet<i32> = rides.values().copied().collect();

    vehicles.into_iter().map(|vehicle| passengers_packet(vehicle, &riders_of(&rides, vehicle))).collect()
}

fn ride(parts: &[&str], source: &Source, shared: &Shared) -> Answer {
    let [named, action, rest @ ..] = parts else {
        return bad_syntax();
    };

    let found = targets(named, source, shared);

    let Some(rider) = found.first().cloned() else {
        return no_entity(named);
    };

    if found.len() > 1 {
        return fail("argument.entity.toomany", &[]);
    }

    let mut rides = shared.world_cmd.rides.lock().expect("поездки захвачены другим потоком");

    match (*action, rest) {
        ("mount", [vehicle_named]) => {
            let vehicles = targets(vehicle_named, source, shared);

            let Some(vehicle) = vehicles.first().cloned() else {
                return no_entity(vehicle_named);
            };

            if vehicles.len() > 1 {
                return fail("argument.entity.toomany", &[]);
            }

            if let Some(current) = rides.get(&rider.entity_id()) {
                let current_name = if *current == vehicle.entity_id() { vehicle.name() } else { s(current.to_string()) };

                return fail("commands.ride.already_riding", &[rider.name(), current_name]);
            }

            if matches!(vehicle, Target::Player { .. }) {
                return fail("commands.ride.mount.failure.cant_ride_players", &[]);
            }

            // Нельзя сесть на себя и на своего же наездника.
            let mut up = Some(vehicle.entity_id());

            while let Some(at) = up {
                if at == rider.entity_id() {
                    return fail("commands.ride.mount.failure.loop", &[]);
                }

                up = rides.get(&at).copied();
            }

            if vehicle.dimension() != rider.dimension() {
                return fail("commands.ride.mount.failure.wrong_dimension", &[]);
            }

            rides.insert(rider.entity_id(), vehicle.entity_id());

            let body = passengers_packet(vehicle.entity_id(), &riders_of(&rides, vehicle.entity_id()));

            drop(rides);
            broadcast(shared, Some(vehicle.dimension()), body);

            tr("commands.ride.mount.success", &[rider.name(), vehicle.name()])
        }
        ("dismount", []) => {
            let Some(vehicle) = rides.remove(&rider.entity_id()) else {
                return fail("commands.ride.not_riding", &[rider.name()]);
            };

            let body = passengers_packet(vehicle, &riders_of(&rides, vehicle));

            drop(rides);
            broadcast(shared, Some(rider.dimension()), body);

            let vehicle_name = targets("@e", source, shared).into_iter().find(|target| target.entity_id() == vehicle).map_or_else(|| s(vehicle.to_string()), |target| target.name());

            tr("commands.ride.dismount.success", &[rider.name(), vehicle_name])
        }
        _ => bad_syntax(),
    }
}

// ---------------------------------------------------------------------------
// locate
// ---------------------------------------------------------------------------

/// Постройки, какие знает команда: имя в реестре и есть ли такая у нашей
/// генерации (остальные — «не нашлось», как у оригинала в мире без них).
const STRUCTURES: [&str; 34] = [
    "ancient_city", "bastion_remnant", "buried_treasure", "desert_pyramid", "end_city", "fortress", "igloo", "jungle_pyramid",
    "mansion", "mineshaft", "mineshaft_mesa", "monument", "nether_fossil", "ocean_ruin_cold", "ocean_ruin_warm", "pillager_outpost",
    "ruined_portal", "ruined_portal_desert", "ruined_portal_jungle", "ruined_portal_mountain", "ruined_portal_nether", "ruined_portal_ocean",
    "ruined_portal_swamp", "shipwreck", "shipwreck_beached", "stronghold", "swamp_hut", "trail_ruins", "trial_chambers", "village_desert",
    "village_plains", "village_savanna", "village_snowy", "village_taiga",
];

/// Ответ «нашлось» (пробник, locate biome): `[x, y, z]` зелёным в
/// `chat.square_brackets`, щелчок подставляет перенос, x и z — числа, y —
/// строка («~» у построек), расстояние — целое.
fn located(key: &str, what: &str, (x, y, z): (i32, Option<i32>, i32), from: (f64, f64, f64)) -> Answer {
    let height = y.map_or("~".to_string(), |y| y.to_string());
    let command = format!("/tp @s {} {} {}", x, height, z);
    let away = ((f64::from(x) - from.0.floor()).powi(2) + (f64::from(z) - from.2.floor()).powi(2)).sqrt().floor() as i64;
    let coordinates = Compound::new()
        .compounds("with", vec![Compound::new().int("", x), Compound::new().string("", &height), Compound::new().int("", z)])
        .string("translate", "chat.coordinates");
    let bracketed = Compound::new()
        .compounds("with", vec![coordinates])
        .compound("click_event", Compound::new().string("action", "suggest_command").string("command", &command))
        .string("color", "green")
        .compound("hover_event", Compound::new().string("action", "show_text").compound("value", Compound::new().string("translate", "chat.coordinates.tooltip")))
        .string("translate", "chat.square_brackets");
    let message = Compound::new().compounds("with", vec![Compound::new().string("", what), bracketed, Compound::new().int("", away as i32)]).string("translate", key);

    Answer { reply: english(key, &[s(what), s(format!("[{}, {}, {}]", x, height, z)), Arg::Int(away)]), effect: Effect::None, component: Some(message.encode_network()), rich: Vec::new(), result: Some(1) }
}

fn locate(parts: &[&str], source: &Source, shared: &Shared) -> Answer {
    let origin = crate::commands::origin_of(source, shared);
    let dimension = crate::commands::dimension_of(source, shared);

    match parts {
        ["structure", name] => {
            let short = name.trim_start_matches('#').strip_prefix("minecraft:").unwrap_or(name.trim_start_matches('#'));

            if !STRUCTURES.contains(&short) {
                return fail("commands.locate.structure.invalid", &[s(*name)]);
            }

            let world = shared.world_in(dimension).lock().expect("мир захвачен другим потоком");
            let chunk = ((origin.0.floor() as i32).div_euclid(16), (origin.2.floor() as i32).div_euclid(16));

            // Крепости — на кольцах, их оригинал находит на любом удалении
            // и называет угол чанка старта (свой мир оригинала: «[1584, ~,
            // 480] (1655 blocks away)» из 0 0).
            if short == "stronghold" {
                return match world.nearest_structure(short, chunk.0, chunk.1, 0) {
                    Some((chunk_x, chunk_z)) => located("commands.locate.structure.success", "minecraft:stronghold", (chunk_x * 16, None, chunk_z * 16), origin),
                    None => fail("commands.locate.structure.not_found", &[s("minecraft:stronghold")]),
                };
            }

            // Деревни оригинал называет углом чанка старта (свой мир
            // оригинала: «[288, ~, 1984]» у старта в чанке 18, 124).
            // Шахты, храмы, хижина и иглу — тоже угол чанка старта (свой мир
            // оригинала: шахта со стартом в чанке −7, −10 — «[-112, ~, -160]»,
            // хижина — «[4816, ~, 1600]»).
            let corner = short.starts_with("village_") || matches!(short, "mineshaft" | "mineshaft_mesa" | "desert_pyramid" | "jungle_pyramid" | "swamp_hut" | "igloo");
            // оригинала: «[288, ~, 1984]» у старта в чанке 18, 124); так же
            // разрушенные порталы, кораблекрушения, руины в океане и руины
            // тропы (свой мир оригинала: все 924 ответа locate — кратны 16).
            let corner = corner || ["village_", "ruined_portal", "shipwreck", "ocean_ruin", "trail_ruins"].iter().any(|prefix| short.starts_with(prefix));

            // Лесные особняки оригинал находит и дальше 100 чанков и называет
            // угол чанка старта (свой мир оригинала, семя 100554032945340:
            // «[-4432, ~, -2112] (4701 blocks away)» из 0 0, «… (7945 blocks
            // away)» из 20000 20000) — ищем на 100 областей вокруг.
            if short == "mansion" {
                return match world.nearest_structure(short, chunk.0, chunk.1, 100 * crate::world::mansion::SPACING / 27) {
                    Some((chunk_x, chunk_z)) => located("commands.locate.structure.success", "minecraft:mansion", (chunk_x * 16, None, chunk_z * 16), origin),
                    None => fail("commands.locate.structure.not_found", &[s("minecraft:mansion")]),
                };
            }

            // Клад оригинал называет клеткой 9, 9 чанка (вики, Structure set:
            // «locate_offset of x:9, y:0, z:9»).
            if short == "buried_treasure" {
                return match world.nearest_structure(short, chunk.0, chunk.1, 6) {
                    Some((chunk_x, chunk_z)) => located("commands.locate.structure.success", "minecraft:buried_treasure", (chunk_x * 16 + 9, None, chunk_z * 16 + 9), origin),
                    None => fail("commands.locate.structure.not_found", &[s("minecraft:buried_treasure")]),
                };
            }

            // Оригинал ищет до 100 чанков вокруг (вики, Commands/locate).
            match world.nearest_structure(short, chunk.0, chunk.1, 6) {
                Some((chunk_x, chunk_z)) if corner && (chunk_x - chunk.0).abs() <= 100 && (chunk_z - chunk.1).abs() <= 100 => {
                    located("commands.locate.structure.success", &format!("minecraft:{}", short), (chunk_x * 16, None, chunk_z * 16), origin)
                }
                Some((chunk_x, chunk_z)) if (chunk_x - chunk.0).abs() <= 100 && (chunk_z - chunk.1).abs() <= 100 => {
                    located("commands.locate.structure.success", &format!("minecraft:{}", short), (chunk_x * 16 + 8, None, chunk_z * 16 + 8), origin)
                }
                _ => fail("commands.locate.structure.not_found", &[s(format!("minecraft:{}", short))]),
            }
        }
        ["biome", name] => {
            let short = name.strip_prefix("minecraft:").unwrap_or(name);

            if !crate::world::terrain::Biome::ALL.iter().any(|biome| biome.name() == short) {
                return fail("argument.resource.not_found", &[s(*name), s("minecraft:worldgen/biome")]);
            }

            let world = shared.world_in(dimension).lock().expect("мир захвачен другим потоком");
            let (home_x, home_z) = (origin.0.floor() as i32, origin.2.floor() as i32);

            // Оригинал: до 6400 блоков, шаг 32 (вики, Commands/locate).
            const STEP: i32 = 32;
            const RADIUS: i32 = 6400;

            for ring in 0..=RADIUS / STEP {
                let mut best: Option<(i32, i32)> = None;

                for dx in -ring..=ring {
                    for dz in -ring..=ring {
                        if dx.abs() != ring && dz.abs() != ring {
                            continue;
                        }

                        let (x, z) = (home_x + dx * STEP, home_z + dz * STEP);

                        if world.surface_biome(x, z) == short {
                            let closer = best.is_none_or(|(bx, bz)| (x - home_x).pow(2) + (z - home_z).pow(2) < (bx - home_x).pow(2) + (bz - home_z).pow(2));

                            if closer {
                                best = Some((x, z));
                            }
                        }
                    }
                }

                if let Some((x, z)) = best {
                    return located("commands.locate.biome.success", &format!("minecraft:{}", short), (x, Some(origin.1.floor() as i32), z), origin);
                }
            }

            fail("commands.locate.biome.not_found", &[s(format!("minecraft:{}", short))])
        }
        ["poi", name] => {
            // Точек интереса (кровати, места работы) у нас пока нет.
            fail("commands.locate.poi.not_found", &[s(*name)])
        }
        _ => bad_syntax(),
    }
}

// ---------------------------------------------------------------------------
// fillbiome
// ---------------------------------------------------------------------------

fn fillbiome(parts: &[&str], source: &Source, shared: &Shared) -> Answer {
    let origin = crate::commands::origin_of(source, shared);

    let (corners, biome, only) = match parts {
        [x1, y1, z1, x2, y2, z2, biome] => ([*x1, *y1, *z1, *x2, *y2, *z2], *biome, None),
        [x1, y1, z1, x2, y2, z2, biome, "replace", only] => ([*x1, *y1, *z1, *x2, *y2, *z2], *biome, Some(*only)),
        _ => return bad_syntax(),
    };

    let (Some(a), Some(b)) = (block_place(&corners[0..3], origin), block_place(&corners[3..6], origin)) else {
        return bad_syntax();
    };

    let number = |name: &str| {
        let short = name.strip_prefix("minecraft:").unwrap_or(name);

        crate::world::terrain::Biome::ALL.iter().find(|biome| biome.name() == short).map(|biome| biome.number() as crate::world::BiomeId)
    };

    let Some(biome_number) = number(biome) else {
        return fail("argument.resource.not_found", &[s(biome), s("minecraft:worldgen/biome")]);
    };

    let only = match only {
        Some(name) => match number(name) {
            Some(found) => Some(found),
            None => return fail("argument.resource.not_found", &[s(name), s("minecraft:worldgen/biome")]),
        },
        None => None,
    };

    let low = (a.0.min(b.0), a.1.min(b.1), a.2.min(b.2));
    let high = (a.0.max(b.0), a.1.max(b.1), a.2.max(b.2));

    // Предел — по клеткам 4×4×4 вокруг области, как у `fill` (вики,
    // Commands/fillbiome) [проверить].
    let volume = (i64::from(high.0 - low.0) + 1) * (i64::from(high.1 - low.1) + 1) * (i64::from(high.2 - low.2) + 1);

    if volume > BLOCK_LIMIT {
        return fail("commands.fillbiome.toobig", &[Arg::Int(BLOCK_LIMIT), Arg::Int(volume)]);
    }

    let dimension = crate::commands::dimension_of(source, shared);
    let mut world = shared.world_in(dimension).lock().expect("мир захвачен другим потоком");

    for chunk_x in low.0 >> 4..=high.0 >> 4 {
        for chunk_z in low.2 >> 4..=high.2 >> 4 {
            if !world.has_chunk(chunk_x, chunk_z) {
                return fail("argument.pos.unloaded", &[]);
            }
        }
    }

    let changed = world.fill_biome(low, high, biome_number, only);

    // [проверить]: оригинал шлёт клиентам Chunk Biomes; у нас клиент увидит
    // новый биом, когда чанк придёт снова.
    // Углы в ответе — начала клеток 4×4×4 (пробник: 2150 −60 100 … 2152
    // −58 102 → «2148, −60, 100 and 2152, −60, 100»).
    let cell = |value: i32| i64::from(value.div_euclid(4) * 4);
    let corners = [Arg::Int(cell(low.0)), Arg::Int(cell(low.1)), Arg::Int(cell(low.2)), Arg::Int(cell(high.0)), Arg::Int(cell(high.1)), Arg::Int(cell(high.2))];
    let mut args = vec![Arg::Int(changed as i64)];

    args.extend(corners);

    tr("commands.fillbiome.success.count", &args)
}

// ---------------------------------------------------------------------------
// place
// ---------------------------------------------------------------------------

fn place(parts: &[&str], _source: &Source, _shared: &Shared) -> Answer {
    // Готовых «фич» и шаблонов из наборов данных у нас нет: генерация
    // своя (деревья, постройки Незера и Энда кладутся при складывании
    // чанка). Отвечаем, как оригинал на неизвестное имя.
    match parts {
        ["feature", name, ..] => fail("commands.place.feature.invalid", &[s(*name)]),
        ["structure", name, ..] => fail("commands.place.structure.invalid", &[s(*name)]),
        ["template", name, ..] => fail("commands.place.template.invalid", &[s(*name)]),
        ["jigsaw", pool, ..] => fail("commands.place.jigsaw.invalid", &[s(*pool)]),
        _ => bad_syntax(),
    }
}

// ---------------------------------------------------------------------------
// loot
// ---------------------------------------------------------------------------

/// Добыча по таблице `blocks/<блок>` или `mine <место>`: что выпало бы из
/// блока без инструмента (blocks::drop_of). Других таблиц у нас нет.
fn loot_items(source_parts: &[&str], source: &Source, shared: &Shared) -> Result<Vec<crate::inventory::Stack>, Answer> {
    match source_parts {
        ["loot", table] => {
            let short = table.strip_prefix("minecraft:").unwrap_or(table);

            let Some(block) = short.strip_prefix("blocks/") else {
                return Err(fail("argument.resource_or_id.failed_to_parse", &[s(*table)]));
            };

            let Some(state) = crate::blocks::state_by_name(block) else {
                return Err(fail("argument.resource_or_id.failed_to_parse", &[s(*table)]));
            };

            Ok(crate::blocks::drop_of(state, None).map(|item| crate::inventory::Stack::new(item, 1)).into_iter().collect())
        }
        ["mine", x, y, z, ..] => {
            let origin = crate::commands::origin_of(source, shared);

            let Some((x, y, z)) = block_place(&[*x, *y, *z], origin) else {
                return Err(bad_syntax());
            };

            let world = shared.world_in(crate::commands::dimension_of(source, shared)).lock().expect("мир захвачен другим потоком");
            let state = world.get_block(x, y, z);

            Ok(crate::blocks::drop_of(state, None).map(|item| crate::inventory::Stack::new(item, 1)).into_iter().collect())
        }
        _ => Err(bad_syntax()),
    }
}

fn loot(parts: &[&str], source: &Source, shared: &Shared) -> Answer {
    let origin = crate::commands::origin_of(source, shared);

    match parts {
        ["give", named, rest @ ..] => {
            let stacks = match loot_items(rest, source, shared) {
                Ok(stacks) => stacks,
                Err(answer) => return answer,
            };

            let found: Vec<Target> = targets(named, source, shared).into_iter().filter(|target| matches!(target, Target::Player { .. })).collect();

            if found.is_empty() {
                return no_entity(named);
            }

            let mut players = shared.players.lock().expect("список игроков захвачен другим потоком");

            for target in &found {
                if let Target::Player { uuid, .. } = target {
                    for stack in &stacks {
                        players.order(uuid, Order::Pick { stack: *stack });
                    }
                }
            }

            drop(players);
            loot_answer("commands.drop.success.single", "commands.drop.success.multiple", &stacks)
        }
        ["spawn", x, y, z, rest @ ..] => {
            let stacks = match loot_items(rest, source, shared) {
                Ok(stacks) => stacks,
                Err(answer) => return answer,
            };

            let (Some(x), Some(y), Some(z)) = (coordinate(x, origin.0, true), coordinate(y, origin.1, false), coordinate(z, origin.2, true)) else {
                return bad_syntax();
            };

            let rest_text = format!("{} {} {}", x, y, z);
            let _ = rest_text;

            for stack in &stacks {
                let item_name = crate::blocks::item_name(stack.item).unwrap_or("air");
                let command = format!("summon item {} {} {} {{Item:{{id:\"minecraft:{}\",count:{}}}}}", x, y, z, item_name, stack.count);

                crate::commands::run(&command, &Source::Console, shared);
            }

            loot_answer("commands.drop.success.single", "commands.drop.success.multiple", &stacks)
        }
        ["insert" | "replace", ..] => {
            // [проверить]: в хранилище и в ячейки — позже.
            fail("commands.drop.no_held_items", &[])
        }
        _ => bad_syntax(),
    }
}

fn loot_answer(single: &str, multiple: &str, stacks: &[crate::inventory::Stack]) -> Answer {
    match stacks {
        [stack] => tr(single, &[Arg::Int(i64::from(stack.count)), Arg::Tr(format!("item.minecraft.{}", crate::blocks::item_name(stack.item).unwrap_or("air")), Vec::new())]),
        _ => tr(multiple, &[Arg::Int(stacks.len() as i64)]),
    }
}

/// Английские шаблоны ответов (языковая таблица en_us 26.1 из
/// minecraft-data) — для консоли, журнала и Bedrock.
const TEMPLATES: &[(&str, &str)] = &[
    ("argument.entity.options.limit.toosmall", "Limit must be at least 1"),
    ("commands.data.entity.query", "%s has the following entity data: %s"),
    ("argument.nbtpath.nothing_found", "Found no elements matching %s"),
    ("commands.generic.permission", "You do not have permission to use this command"),
    ("argument.block.id.invalid", "Unknown block type '%s'"),
    ("argument.dimension.invalid", "Unknown dimension '%s'"),
    ("argument.double.big", "Double must not be more than %s: found %s"),
    ("argument.double.low", "Double must not be less than %s: found %s"),
    ("argument.entity.notfound.entity", "No entity was found"),
    ("argument.entity.notfound.player", "No player was found"),
    ("argument.entity.toomany", "Only one entity is allowed, but the provided selector allows more than one"),
    ("argument.float.big", "Float must not be more than %s: found %s"),
    ("argument.float.low", "Float must not be less than %s: found %s"),
    ("argument.pos.outofworld", "That position is out of this world!"),
    ("argument.pos.unloaded", "That position is not loaded"),
    ("argument.resource.not_found", "Can't find element '%s' of type '%s'"),
    ("argument.resource_or_id.failed_to_parse", "Failed to parse structure: %s"),
    ("command.unknown.argument", "Incorrect argument for command"),
    ("commands.attribute.base_value.get.success", "The base value of attribute %s for entity %s is %s"),
    ("commands.attribute.base_value.reset.success", "The base value for attribute %s for entity %s reset to default %s"),
    ("commands.attribute.base_value.set.success", "The base value for attribute %s for entity %s set to %s"),
    ("commands.attribute.failed.entity", "%s is not a valid entity for this command"),
    ("commands.attribute.failed.modifier_already_present", "Modifier %s is already present on attribute %s for entity %s"),
    ("commands.attribute.failed.no_attribute", "Entity %s has no attribute %s"),
    ("commands.attribute.failed.no_modifier", "Attribute %s for entity %s has no modifier %s"),
    ("commands.attribute.modifier.add.success", "Added modifier %s to attribute %s for entity %s"),
    ("commands.attribute.modifier.remove.success", "Removed modifier %s from attribute %s for entity %s"),
    ("commands.attribute.modifier.value.get.success", "The value of modifier %s on attribute %s for entity %s is %s"),
    ("commands.attribute.value.get.success", "The value of attribute %s for entity %s is %s"),
    ("commands.clone.failed", "No blocks were cloned"),
    ("commands.clone.overlap", "The source and destination areas cannot overlap"),
    ("commands.clone.success", "Successfully cloned %s block(s)"),
    ("commands.clone.toobig", "Too many blocks in the specified area (maximum %s, but specified %s)"),
    ("commands.drop.no_held_items", "Entity can't hold any items"),
    ("commands.drop.success.multiple", "Dropped %s items"),
    ("commands.drop.success.single", "Dropped %s %s"),
    ("commands.enchant.failed", "Nothing changed. Targets either have no item in their hands or the enchantment could not be applied"),
    ("commands.enchant.failed.entity", "%s is not a valid entity for this command"),
    ("commands.enchant.failed.incompatible", "%s cannot support that enchantment"),
    ("commands.enchant.failed.itemless", "%s is not holding any item"),
    ("commands.enchant.failed.level", "%s is higher than the maximum level of %s supported by that enchantment"),
    ("commands.enchant.success.multiple", "Applied enchantment %s to %s entities"),
    ("commands.enchant.success.single", "Applied enchantment %s to %s's item"),
    ("commands.fillbiome.success.count", "%s biome entry/entries set between %s, %s, %s and %s, %s, %s"),
    ("commands.fillbiome.toobig", "Too many blocks in the specified volume (maximum %s, but specified %s)"),
    ("commands.forceload.added.failure", "No chunks were marked for force loading"),
    ("commands.forceload.added.multiple", "Marked %s chunks in %s from %s to %s to be force loaded"),
    ("commands.forceload.added.none", "No force loaded chunks were found in %s"),
    ("commands.forceload.added.single", "Marked chunk %s in %s to be force loaded"),
    ("commands.forceload.list.multiple", "%s force loaded chunks were found in %s at: %s"),
    ("commands.forceload.list.single", "A force loaded chunk was found in %s at: %s"),
    ("commands.forceload.query.failure", "Chunk at %s in %s is not marked for force loading"),
    ("commands.forceload.query.success", "Chunk at %s in %s is marked for force loading"),
    ("commands.forceload.removed.all", "Unmarked all force loaded chunks in %s"),
    ("commands.forceload.removed.failure", "No chunks were removed from force loading"),
    ("commands.forceload.removed.multiple", "Unmarked %s chunks in %s from %s to %s for force loading"),
    ("commands.forceload.removed.single", "Unmarked chunk %s in %s for force loading"),
    ("commands.forceload.toobig", "Too many chunks in the specified area (maximum %s, but specified %s)"),
    ("commands.locate.biome.not_found", "Could not find a biome of type \"%s\" within reasonable distance"),
    ("commands.locate.biome.success", "The nearest %s is at %s (%s blocks away)"),
    ("commands.locate.poi.not_found", "Could not find a point of interest of type \"%s\" within reasonable distance"),
    ("commands.locate.structure.invalid", "There is no structure with type \"%s\""),
    ("commands.locate.structure.not_found", "Could not find a structure of type \"%s\" nearby"),
    ("commands.locate.structure.success", "The nearest %s is at %s (%s blocks away)"),
    ("commands.place.feature.invalid", "There is no feature with type \"%s\""),
    ("commands.place.jigsaw.invalid", "There is no template pool with type \"%s\""),
    ("commands.place.structure.invalid", "There is no structure with type \"%s\""),
    ("commands.place.template.invalid", "There is no template with id \"%s\""),
    ("commands.random.error.range_too_large", "The range of the random value must be at most 2147483646"),
    ("commands.random.error.range_too_small", "The range of the random value must be at least 2"),
    ("commands.random.reset.all.success", "Reset %s random sequence(s)"),
    ("commands.random.reset.success", "Reset random sequence %s"),
    ("commands.random.roll", "%s rolled %s (from %s to %s)"),
    ("commands.random.sample.success", "Randomized value: %s"),
    ("commands.ride.already_riding", "%s is already riding %s"),
    ("commands.ride.dismount.success", "%s stopped riding %s"),
    ("commands.ride.mount.failure.cant_ride_players", "Players can't be ridden"),
    ("commands.ride.mount.failure.loop", "Can't mount entity on itself or any of its passengers"),
    ("commands.ride.mount.failure.wrong_dimension", "Can't mount entity in different dimension"),
    ("commands.ride.mount.success", "%s started riding %s"),
    ("commands.ride.not_riding", "%s is not riding any vehicle"),
    ("commands.rotate.success", "Rotated %s"),
    ("commands.spectate.not_spectator", "%s is not in spectator mode"),
    ("commands.spectate.self", "Cannot spectate yourself"),
    ("commands.spectate.success.started", "Now spectating %s"),
    ("commands.spectate.success.stopped", "No longer spectating an entity"),
    ("commands.spreadplayers.failed.entities", "Could not spread %s entity/entities around %s, %s (too many entities for space - try using spread of at most %s)"),
    ("commands.spreadplayers.success.entities", "Spread %s entity/entities around %s, %s with an average distance of %s block(s) apart"),
    ("commands.tick.query.percentiles", "Percentiles: P50: %sms P95: %sms P99: %sms. Sample: %s"),
    ("commands.tick.query.rate.running", "Target tick rate: %s per second.\nAverage time per tick: %sms (Target: %sms)"),
    ("commands.tick.query.rate.sprinting", "Target tick rate: %s per second (ignored, reference only).\nAverage time per tick: %sms"),
    ("commands.tick.rate.success", "Set the target tick rate to %s per second"),
    ("commands.tick.sprint.report", "Sprint completed with %s ticks per second, or %s ms per tick"),
    ("commands.tick.sprint.stop.fail", "No tick sprint in progress"),
    ("commands.tick.sprint.stop.success", "Interrupted the current tick sprint"),
    ("commands.tick.status.frozen", "The game is frozen"),
    ("commands.tick.status.lagging", "The game is running, but can't keep up with the target tick rate"),
    ("commands.tick.status.running", "The game is running normally"),
    ("commands.tick.status.sprinting", "The game is sprinting"),
    ("commands.tick.step.fail", "Unable to step the game - the game must be frozen first"),
    ("commands.tick.step.stop.fail", "No tick step in progress"),
    ("commands.tick.step.stop.success", "Interrupted the current tick step"),
    ("commands.tick.step.success", "Stepping %s tick(s)"),
    ("commands.worldborder.center.failed", "Nothing changed. The world border is already centered there"),
    ("commands.worldborder.center.success", "Set the center of the world border to %s, %s"),
    ("commands.worldborder.damage.amount.failed", "Nothing changed. The world border damage is already that amount"),
    ("commands.worldborder.damage.amount.success", "Set the world border damage to %s per block each second"),
    ("commands.worldborder.damage.buffer.failed", "Nothing changed. The world border damage buffer is already that distance"),
    ("commands.worldborder.damage.buffer.success", "Set the world border damage buffer to %s block(s)"),
    ("commands.worldborder.get", "The world border is currently %s block(s) wide"),
    ("commands.worldborder.set.failed.big", "World border cannot be bigger than %s blocks wide"),
    ("commands.worldborder.set.failed.far", "World border cannot be further out than %s blocks"),
    ("commands.worldborder.set.failed.nochange", "Nothing changed. The world border is already that size"),
    ("commands.worldborder.set.failed.small", "World border cannot be smaller than 1 block wide"),
    ("commands.worldborder.set.grow", "Growing the world border to %s blocks wide over %s seconds"),
    ("commands.worldborder.set.immediate", "Set the world border to %s block(s) wide"),
    ("commands.worldborder.set.shrink", "Shrinking the world border to %s block(s) wide over %s second(s)"),
    ("commands.worldborder.warning.distance.failed", "Nothing changed. The world border warning is already that distance"),
    ("commands.worldborder.warning.distance.success", "Set the world border warning distance to %s block(s)"),
    ("commands.worldborder.warning.time.failed", "Nothing changed. The world border warning is already that amount of time"),
    ("commands.worldborder.warning.time.success", "Set the world border warning time to %s second(s)"),
    ("permissions.requires.player", "A player is required to run this command here"),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_resume_preserves_freeze_steps_sprint_and_resets_wall_metrics() {
        let now = Instant::now();
        let mut ticking = Ticking {
            frozen: true, step_left: 7,
            sprint: Some(Sprint { left: 123, done: 17, started: now - Duration::from_secs(10) }),
            last_start: Some(now - Duration::from_secs(10)),
            ..Default::default()
        };
        let started = ticking.sprint.as_ref().unwrap().started;
        ticking.resumed_after_idle(Duration::from_secs(5));
        assert!(ticking.frozen);
        assert_eq!(ticking.step_left,7);
        let sprint = ticking.sprint.as_ref().unwrap();
        assert_eq!((sprint.left,sprint.done),(123,17));
        assert_eq!(sprint.started,started + Duration::from_secs(5));
        assert!(ticking.last_start.is_none());
        ticking.note_start(Instant::now());
        assert!(ticking.intervals.is_empty(),"пауза не попала в TPS");
    }

    #[test]
    fn sprint_started_during_idle_does_not_get_future_start_time() {
        let mut ticking = Ticking {
            sprint: Some(Sprint { left: 11, done: 0, started: Instant::now() }),
            ..Default::default()
        };
        ticking.resumed_after_idle(Duration::from_secs(300));
        assert!(ticking.sprint.as_ref().unwrap().started <= Instant::now());
        assert_eq!(ticking.sprint.as_ref().unwrap().left,11);
    }

    #[test]
    fn numbers_are_rounded_half_up_like_java() {
        // Пробник: `worldborder set 1500.25 1` → «1500.3».
        assert_eq!(java_fixed(1500.25, 1), "1500.3");
        assert_eq!(java_fixed(0.2, 2), "0.20");
        assert_eq!(java_fixed(59_999_968.0, 0), "59999968");
        assert_eq!(java_fixed(-20.25, 2), "-20.25");
        assert_eq!(java_fixed(0.05, 2), "0.05");
        assert_eq!(java_fixed(9.96, 1), "10.0");
        assert_eq!(java_fixed(-0.01, 1), "0.0");
    }

    #[test]
    fn time_takes_units() {
        assert_eq!(parse_time("15"), Some(15));
        assert_eq!(parse_time("2s"), Some(40));
        assert_eq!(parse_time("1d"), Some(24_000));
        assert_eq!(parse_time("3t"), Some(3));
        assert_eq!(parse_time("-1"), None);
        assert_eq!(parse_time("x"), None);
    }

    #[test]
    fn the_border_shrinks_tick_by_tick_and_hurts_outside_the_buffer() {
        let mut border = Border { from: 100.0, to: 50.0, lerp_total: 10, lerp_left: 10, ..Border::default() };

        assert_eq!(border.size(), 100.0);

        for _ in 0..5 {
            border.step();
        }

        assert_eq!(border.size(), 75.0);

        for _ in 0..5 {
            border.step();
        }

        assert_eq!(border.size(), 50.0);

        // Края ±25; запас 5 — урон только дальше 30, не меньше 1.
        assert_eq!(border.damage_at(28.0, 0.0), None);
        assert_eq!(border.damage_at(31.0, 0.0), Some(1.0));
        assert_eq!(border.damage_at(0.0, -40.0), Some(2.0));
    }

    #[test]
    fn the_initial_border_packet_matches_the_vanilla_defaults() {
        let body = Border::default().init_packet();

        // номер, центр, поперечники, скорость 0, предел 29999984, 5 блоков, 300 тактов
        assert_eq!(body[0], 0x2B);
        assert_eq!(&body[17..25], &MAX_SIZE.to_be_bytes());
        assert_eq!(&body[body.len() - 3..], &[5, 0xAC, 0x02]);
    }

    #[test]
    fn several_answers_split_back_into_messages() {
        let joined = several(vec![tr("commands.tick.status.running", &[]), tr("commands.tick.rate.success", &[s("20.0")])]);
        let parts = split_components(joined.component.as_deref().unwrap());

        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0], component("commands.tick.status.running", &[]).encode_network().as_slice());
        assert_eq!(joined.reply, "The game is running normally\nSet the target tick rate to 20.0 per second");
    }

    #[test]
    fn english_fills_the_template() {
        assert_eq!(english("commands.clone.success", &[Arg::Int(2)]), "Successfully cloned 2 block(s)");
        assert_eq!(english("commands.worldborder.center.success", &[s("0.50"), s("0.50")]), "Set the center of the world border to 0.50, 0.50");
    }

    #[test]
    fn enchantments_fit_their_items_and_exclude_each_other() {
        assert!(enchantable("sharpness", "diamond_sword"));
        assert!(!enchantable("protection", "diamond_sword"));
        assert!(enchantable("protection", "iron_boots"));
        assert!(exclusive("sharpness", "smite"));
        assert!(!exclusive("sharpness", "unbreaking"));
    }

    #[test]
    fn command_enchant_rejects_books_and_existing_enchantments() {
        let sharpness = crate::stackdata::enchantment_number("sharpness").unwrap();
        let unbreaking = crate::stackdata::enchantment_number("unbreaking").unwrap();

        for (name, _) in crate::stackdata::ENCHANTMENTS {
            assert!(!can_command_enchant(name, "book", &[]), "{name}");
            assert!(!can_command_enchant(name, "enchanted_book", &[]), "{name}");
        }
        assert!(can_command_enchant("sharpness", "diamond_sword", &[]));
        assert!(!can_command_enchant("sharpness", "diamond_sword", &[(sharpness, 1)]));
        assert!(!can_command_enchant("sharpness", "diamond_sword", &[(sharpness, 5)]));
        assert!(!can_command_enchant("smite", "diamond_sword", &[(sharpness, 1)]));
        assert!(can_command_enchant("unbreaking", "diamond_sword", &[(sharpness, 1)]));
        assert!(can_command_enchant("sharpness", "diamond_sword", &[(unbreaking, 3)]));
    }

    /// minecraft-data 26.1: category vanishing не содержит recovery_compass;
    /// подтверждено чёрным ящиком enchant-compasses.
    #[test]
    fn vanishing_accepts_compass_but_not_recovery_compass() {
        assert!(can_command_enchant("vanishing_curse", "compass", &[]));
        assert!(!can_command_enchant("vanishing_curse", "recovery_compass", &[]));
    }

    /// Независимая матрица разрешённых предметов: minecraft-data 26.1,
    /// enchantments.json (`category`) × items.json (`enchantCategories`).
    #[test]
    fn enchantment_applicability_matches_item_categories() {
        let categories = [
            ("aqua_affinity", "head_armor"),
            ("bane_of_arthropods", "weapon"),
            ("binding_curse", "equippable"),
            ("blast_protection", "armor"),
            ("breach", "mace"),
            ("channeling", "trident"),
            ("density", "mace"),
            ("depth_strider", "foot_armor"),
            ("efficiency", "mining"),
            ("feather_falling", "foot_armor"),
            ("fire_aspect", "fire_aspect"),
            ("fire_protection", "armor"),
            ("flame", "bow"),
            ("fortune", "mining_loot"),
            ("frost_walker", "foot_armor"),
            ("impaling", "trident"),
            ("infinity", "bow"),
            ("knockback", "melee_weapon"),
            ("looting", "melee_weapon"),
            ("loyalty", "trident"),
            ("luck_of_the_sea", "fishing"),
            ("lunge", "lunge"),
            ("lure", "fishing"),
            ("mending", "durability"),
            ("multishot", "crossbow"),
            ("piercing", "crossbow"),
            ("power", "bow"),
            ("projectile_protection", "armor"),
            ("protection", "armor"),
            ("punch", "bow"),
            ("quick_charge", "crossbow"),
            ("respiration", "head_armor"),
            ("riptide", "trident"),
            ("sharpness", "sharp_weapon"),
            ("silk_touch", "mining_loot"),
            ("smite", "weapon"),
            ("soul_speed", "foot_armor"),
            ("sweeping_edge", "sweeping"),
            ("swift_sneak", "leg_armor"),
            ("thorns", "armor"),
            ("unbreaking", "durability"),
            ("vanishing_curse", "vanishing"),
            ("wind_burst", "mace"),
        ];
        let items: Vec<serde_json::Value> = serde_json::from_str(include_str!("../tools/data/items.json")).unwrap();
        assert_eq!(categories.len(), crate::stackdata::ENCHANTMENTS.len());
        for (enchantment, category) in categories {
            assert!(crate::stackdata::enchantment_number(enchantment).is_some());
            for item in &items {
                let name = item["name"].as_str().unwrap();
                let expected = item["enchantCategories"].as_array().is_some_and(|cats| cats.iter().any(|cat| cat.as_str() == Some(category)));
                assert_eq!(enchantable(enchantment, name), expected, "{enchantment} на {name}");
            }
        }
    }

    #[test]
    fn ranges_and_angles_parse() {
        assert_eq!(parse_range("1..6"), Some((1, 6)));
        assert_eq!(parse_range("6"), None);
        assert_eq!(angle("~10", 5.0), Some(15.0));
        assert_eq!(angle("90", 5.0), Some(90.0));

        // Взгляд на точку прямо к +z — поворот 0, к −x — 90.
        let (yaw, pitch) = facing((0.0, 0.0, 0.0), (0.0, 0.0, 5.0));

        assert!(yaw.abs() < 1e-4 && pitch.abs() < 1e-4);
        assert!((facing((0.0, 0.0, 0.0), (-5.0, 0.0, 0.0)).0 - 90.0).abs() < 1e-4);
    }
}
