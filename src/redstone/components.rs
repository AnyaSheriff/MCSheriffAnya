// Остальные детали редстоуна: мишень, датчик дневного света, растяжка
// с крючками, редстоуновая руда, колокол, громоотвод — и то, что их будит:
// сущности в мире, время суток, случайный такт.
//
// Всё — по разбору tools/research/components-rest.md (страницы minecraft.wiki:
// Target, Daylight Detector, Talk:Daylight Detector, Light, String, Tripwire
// Hook, Redstone Ore, Bell, Lightning Rod, Pressure Plate, Random tick) и
// tools/research/update-order.md. Кода игры и чужих ядер нет.
//
// Чего вики не говорит прямо, помечено «[проверить]» — это сверит чёрный
// ящик (сценарии target-*, daylight-*, tripwire-*, redstone-ore-*, bell-*,
// note-heads, plate-items).

use std::sync::OnceLock;

use super::{Block, Dir, HORIZONTAL, Kind, MAX_POWER, PRIORITY_NORMAL, Pos, set, settle, step, yes_no};
use crate::blocks;
use crate::world::World;

// ---------------------------------------------------------------------------
// Сущности: кто стоит, лежит, задевает
// ---------------------------------------------------------------------------

/// Сущность, которая может нажать плиту, задеть растяжку или встать на руду:
/// игрок, моб или то, что лежит в мире (выброшенный предмет, падающий блок).
#[derive(Clone, Copy, Debug)]
pub struct Occupant {
    /// Середина низа ящика сущности.
    pub x: f64,
    pub y: f64,
    pub z: f64,

    /// Ширина и высота ящика.
    pub width: f64,
    pub height: f64,

    /// Живое ли: игрок или моб. Каменные плиты слушают только живых.
    pub living: bool,

    /// Крадётся ли: крадущийся игрок руду не зажигает.
    pub sneaking: bool,

    /// Лежит ли на земле — для руды: зажигает её тот, кто на ней стоит.
    pub on_ground: bool,
}

impl Occupant {
    /// Игрок: ящик 0,6 × 1,8, крадущийся — 0,6 × 1,5 (вики, Player). Лежит
    /// ли он на земле, сервер не знает — считаем, что да: руда смотрит, есть
    /// ли она прямо под ногами. Только что перенесённый — нет (tick.rs,
    /// `occupants`; `players::SETTLE_TICKS`).
    pub fn player(x: f64, y: f64, z: f64, sneaking: bool) -> Self {
        Self {
            x,
            y,
            z,
            width: 0.6,
            height: if sneaking { 1.5 } else { 1.8 },
            living: true,
            sneaking,
            on_ground: true,
        }
    }

    /// Моб: живой, ящик по виду (mob/mod.rs). Стоит на земле — руду зажигает
    /// тот, кто на ней стоит.
    pub fn mob(x: f64, y: f64, z: f64, width: f64, height: f64, on_ground: bool) -> Self {
        Self { x, y, z, width, height, living: true, sneaking: false, on_ground }
    }

    /// Выброшенный предмет: ящик 0,25 × 0,25 (вики, Item (entity)).
    pub fn item(x: f64, y: f64, z: f64, on_ground: bool) -> Self {
        Self {
            x,
            y,
            z,
            width: 0.25,
            height: 0.25,
            living: false,
            sneaking: false,
            on_ground,
        }
    }

    /// Падающий блок: ящик 0,98 × 0,98 (вики, Falling Block).
    pub fn falling_block(x: f64, y: f64, z: f64, on_ground: bool) -> Self {
        Self {
            width: 0.98,
            height: 0.98,
            ..Self::item(x, y, z, on_ground)
        }
    }

    /// Вагонетка: ящик 0,98 × 0,7 (вики, Minecart). Деревянные плиты и
    /// растяжки её чувствуют, каменные — нет: она не живая.
    pub fn minecart(x: f64, y: f64, z: f64, on_ground: bool) -> Self {
        Self {
            width: crate::minecart::WIDTH,
            height: crate::minecart::HEIGHT,
            ..Self::item(x, y, z, on_ground)
        }
    }

    /// Пересекает ли ящик сущности кусок блока `pos` от высоты `low` до
    /// `high` (доли блока) во всю его ширину. Строго: касание границы не
    /// считается (вики, String: с 25w31a «entities can no longer trigger
    /// tripwire by exactly touching its former detection boundary»).
    pub(super) fn crosses(&self, pos: Pos, low: f64, high: f64) -> bool {
        let half = self.width / 2.0;

        self.x - half < (pos.0 + 1) as f64
            && self.x + half > pos.0 as f64
            && self.z - half < (pos.2 + 1) as f64
            && self.z + half > pos.2 as f64
            && self.y < pos.1 as f64 + high
            && self.y + self.height > pos.1 as f64 + low
    }
}

/// Всё, что сущности делают с редстоуном за такт: плиты, растяжки, руда.
/// Кто где — из последнего списка (`World::redstone.occupants`).
pub(super) fn step_entities(world: &mut World) {
    let occupants = world.redstone.occupants.clone();

    touch_tripwires(world, &occupants);
    step_on_ores(world, &occupants);
    super::sculk::step_on_shriekers(world, &occupants);
}

// ---------------------------------------------------------------------------
// Мишень
// ---------------------------------------------------------------------------

/// Сколько держится сигнал мишени: от стрелы и трезубца — 20 тактов, от
/// прочих снарядов — 8 (вики, Target: «emits redstone power for 8 gt. Arrows
/// and tridents instead cause the target to emit power for 20 gt»).
const TARGET_ARROW_TICKS: u64 = 20;
const TARGET_OTHER_TICKS: u64 = 8;

/// Сила сигнала мишени по месту попадания: смещения от середины грани по
/// двум её осям, каждое от 0 до 0,5. Вики говорит только «from 1 to 15» —
/// чем ближе к середине, тем больше; квадратные кольца и линейный шаг —
/// наша гипотеза из разбора (components-rest.md §1). [проверить] сеткой
/// выстрелов в чёрном ящике.
#[allow(dead_code)] // снарядов пока нет: позовёт попадание стрелы
pub fn target_power(du: f64, dv: f64) -> u8 {
    let off = du.abs().max(dv.abs()).min(0.5);

    ((15.0 * (0.5 - off) / 0.5).ceil() as i64).clamp(1, MAX_POWER as i64) as u8
}

/// В мишень попал снаряд: сила по месту попадания, держится 20 тактов от
/// стрелы и трезубца, 8 — от прочего. Новое попадание перезаписывает и силу,
/// и срок. [проверить]: что делает попадание в уже горящую мишень.
#[allow(dead_code)] // снарядов пока нет
pub fn target_hit(world: &mut World, pos: Pos, power: u8, arrow: bool) {
    let block = Block::at(world, pos);

    if block.kind != Kind::Target {
        return;
    }

    if let Some(state) = block.with("power", &power.clamp(1, MAX_POWER).to_string()) {
        set(world, pos, state);
    }

    let held = if arrow { TARGET_ARROW_TICKS } else { TARGET_OTHER_TICKS };

    world.schedule_block(pos.0, pos.1, pos.2, held, PRIORITY_NORMAL);
    settle(world);
}

/// Сила мишени или датчика света по свойству `power`.
pub(super) fn power_value(block: &Block) -> u8 {
    block.value("power").and_then(|value| value.parse().ok()).unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Громоотвод
// ---------------------------------------------------------------------------

/// Сколько держится сигнал громоотвода после удара молнии (вики, Lightning
/// Rod: «at level 15 for 8 game ticks»).
const ROD_TICKS: u64 = 8;

/// В громоотвод ударила молния: 8 тактов он включён и сильно питает блок,
/// на котором стоит (lightning.rs).
pub fn strike(world: &mut World, pos: Pos) {
    let block = Block::at(world, pos);

    if block.kind != Kind::LightningRod {
        return;
    }

    if let Some(state) = block.with("powered", "true") {
        set(world, pos, state);
    }

    world.schedule_block(pos.0, pos.1, pos.2, ROD_TICKS, PRIORITY_NORMAL);
    settle(world);
}

// ---------------------------------------------------------------------------
// Датчик дневного света
// ---------------------------------------------------------------------------

/// Раз во сколько тактов датчик пересчитывает выход: «updated every 20
/// gametime» — по возрасту мира, не по времени суток (вики, Daylight
/// Detector). Обновления блоков на него не действуют.
const DAYLIGHT_PERIOD: u64 = 20;

/// Внутренний небесный свет в ясную погоду по времени суток — снят
/// с оригинала 26.1.2 (сценарий daylight-dusk: инвертированный датчик под
/// открытым небом от 11800 до 13200). Вечером свет падает на единицу
/// каждые 164 такта начиная с такта ~12030, до 4; все 15 переходов записи
/// сходятся с этим и только с этим (перебор начала, шага и округления:
/// округление вниз, шаг 163,5…164,5). Ни таблица вики Light, ни линия
/// 12040→13670 из timelines.md с записью не сходятся. Утро — зеркально
/// относительно полудня: рост до 15 к такту 23970. [проверить] утро и
/// начало спада с точностью до такта: время в записи известно лишь до
/// сдвига, который вносит сама запись.
const DUSK_START: i64 = 12_030;
const DAWN_END: i64 = 23_970;
const TICKS_PER_LEVEL: f64 = 164.0;

/// Небесный свет под открытым небом в этот час — для мобов (свет паука, появление).
pub fn ambient_sky(time: i64) -> i32 {
    internal_sky(time)
}

/// Внутренний небесный свет при небесном свете 15: днём 15, ночью 4.
fn internal_sky(time: i64) -> i32 {
    let time = time.rem_euclid(crate::world::DAY_LENGTH);

    let level = if time <= DUSK_START || time >= DAWN_END {
        15.0
    } else if time < 18_000 {
        15.0 - (time - DUSK_START) as f64 / TICKS_PER_LEVEL
    } else {
        15.0 - (DAWN_END - time) as f64 / TICKS_PER_LEVEL
    };

    (level.floor() as i32).max(4)
}

/// Множитель солнца для обычного датчика. Формула со страницы обсуждения
/// вики (Talk:Daylight Detector): «round((SL − D) · TOD)», где TOD =
/// cos(2π·(t + 0,2·(t < 0,5 ? −t : 1 − t))), а t — доля суток от полудня
/// с «кривизной» угла неба со страницы Daylight cycle: α/360 =
/// (1 − cos(π·f) + 4·f)/6, f = frac((T − 6000)/24000). По разбору она
/// сходится с таблицей вики на 23 996 тактах из 24 000.
fn sun(time: i64) -> f64 {
    use std::f64::consts::PI;

    let f = ((time - 6_000) as f64 / crate::world::DAY_LENGTH as f64).rem_euclid(1.0);
    let t = (1.0 - (PI * f).cos() + 4.0 * f) / 6.0;
    let t = t + 0.2 * if t < 0.5 { -t } else { 1.0 - t };

    (2.0 * PI * t).cos()
}

/// Что должен выдавать датчик: небесный свет в его клетке `sky`, время
/// суток, режим.
///
/// Тень вычитается из внутреннего света до всего остального («The effects
/// of shade are applied before inverting»). Обычный — `round(base·TOD)`,
/// инвертированный — `15 − base`, оба зажаты в 0..15. [проверить]: ночью
/// в тени (0 < SL < 11) у обычного оба множителя отрицательны и выход
/// положительный — так описано в 2014 году; и способ округления.
#[cfg(test)]
fn daylight_output(sky: u8, time: i64, inverted: bool) -> u8 {
    daylight_output_with(sky, internal_sky(time), time, inverted)
}

/// То же при внутреннем свете `internal` — с погодой (weather.rs, `dim_sky`:
/// «Rain and thunderstorms reduce the output», вики, Daylight Detector).
fn daylight_output_with(sky: u8, internal: i32, time: i64, inverted: bool) -> u8 {
    let base = internal - (15 - sky as i32);

    let power = if inverted {
        15 - base
    } else {
        (base as f64 * sun(time)).round() as i32
    };

    power.clamp(0, MAX_POWER as i32) as u8
}

/// Выход датчика в этом месте сейчас. В Незере обычный всегда 0,
/// инвертированный — 15 (вики, Daylight Detector, Java).
fn daylight_power(world: &mut World, pos: Pos, inverted: bool) -> u8 {
    if world.is_nether() {
        return if inverted { MAX_POWER } else { 0 };
    }

    let sky = world.sky_light(pos.0, pos.1, pos.2);

    let time = world.time_of_day();

    daylight_output_with(sky, world.weather.dim_sky(internal_sky(time)), time, inverted)
}

/// Датчики света пересчитывают выход раз в 20 тактов. У оригинала это
/// блок-сущность, поэтому здесь — фаза блок-сущностей. [проверить]: порядок
/// среди прочих блок-сущностей (у нас — раньше воронок) и между датчиками
/// (у нас — по координатам).
///
/// Фаза снята чёрным ящиком (daylight-dusk с привязкой к сетке возраста):
/// у оригинала пересчёт приходит в такте, которому «обновление времени»
/// приписывает возраст 19 по модулю 20, — на такт раньше, чем приходит
/// само обновление с кратным 20 возрастом. Мы рассылаем возраст уже после
/// прибавки такта, поэтому пересчитываем, когда `tick % 20 == 19`.
pub fn daylight_tick(world: &mut World) {
    if !(world.tick() + 1).is_multiple_of(DAYLIGHT_PERIOD) || world.redstone.daylight.is_empty() {
        return;
    }

    let detectors: Vec<Pos> = world.redstone.daylight.iter().copied().collect();

    for pos in detectors {
        if !world.has_chunk(pos.0.div_euclid(16), pos.2.div_euclid(16)) {
            world.redstone.daylight.remove(&pos);
            continue;
        }

        refresh_daylight(world, pos);
    }
}

/// Пересчитывает один датчик: сменилась сила — меняется состояние.
fn refresh_daylight(world: &mut World, pos: Pos) {
    let block = Block::at(world, pos);

    if block.kind != Kind::Daylight {
        world.redstone.daylight.remove(&pos);
        return;
    }

    let power = daylight_power(world, pos, block.is("inverted", "true"));

    if power != power_value(&block)
        && let Some(state) = block.with("power", &power.to_string())
    {
        set(world, pos, state);
        settle(world);
    }
}

/// Щелчок по датчику: режим меняется на обратный, выход пересчитывается
/// сразу (вики, Daylight Detector: «To invert a daylight detector, … use»).
/// Состояние меняется одной правкой — и режим, и сила. [проверить]: сразу
/// ли пересчитывается сила или только в ближайшем кратном 20 такте.
pub(super) fn toggle_daylight(world: &mut World, pos: Pos, block: &Block) -> Option<i32> {
    let inverted = !block.is("inverted", "true");
    let power = daylight_power(world, pos, inverted);

    block
        .with("inverted", yes_no(inverted))
        .and_then(|state| block.orientation?.with(state, "power", &power.to_string()))
}

// ---------------------------------------------------------------------------
// Растяжка и крючки
// ---------------------------------------------------------------------------

/// Через сколько тактов задетая растяжка проверяет, есть ли на ней ещё
/// кто-то (вики, String: «schedules a block tick … 10 game ticks later»).
/// Столько же держит сигнал крючок, у которого порвали растяжку («for 5
/// redstone ticks»).
const TRIPWIRE_RECHECK: u64 = 10;

/// Сколько растяжек может быть в цепи: от 1 до 40, второй крючок — не
/// дальше 41 блока (вики, Tripwire Hook).
const MAX_WIRES: i32 = 40;

/// Звуки цепи (вики, String / Tripwire Hook, Java): громкость 0,4.
const ATTACH: &str = "minecraft:block.tripwire.attach";
const CLICK_ON: &str = "minecraft:block.tripwire.click_on";
const CLICK_OFF: &str = "minecraft:block.tripwire.click_off";
const DETACH: &str = "minecraft:block.tripwire.detach";

/// Место в `distance` шагах от `pos` в сторону `dir`.
fn step_by(pos: Pos, dir: Dir, distance: i32) -> Pos {
    let (dx, dy, dz) = dir.offset();

    (pos.0 + dx * distance, pos.1 + dy * distance, pos.2 + dz * distance)
}

/// Ящик растяжки по высоте: в исправной цепи — тонкий, от 1/16 до
/// 1/16 + 3/32, вне цепи — от низа до половины блока (вики, Tripwire Hook:
/// «0.09375 … with the bottom 0.0625 … above», «0.5 blocks high, bottom not
/// raised»). По ширине — весь блок.
fn wire_span(wire: &Block) -> (f64, f64) {
    if wire.is("attached", "true") {
        (0.0625, 0.0625 + 0.09375)
    } else {
        (0.0, 0.5)
    }
}

/// Задевает ли кто-нибудь растяжку. Предметы, застрявшие в сплошном блоке,
/// не считаются (вики, String) — таких у нас не бывает: предмет упирается
/// в блоки.
fn anyone_on_wire(occupants: &[Occupant], pos: Pos, wire: &Block) -> bool {
    let (low, high) = wire_span(wire);

    occupants.iter().any(|occupant| occupant.crosses(pos, low, high))
}

/// Сущности задевают растяжки: неактивная растяжка, которую пересёк ящик
/// сущности, включается — сообщает соседям, будит крючки к югу и западу
/// (это делает block_changed) и через 10 тактов проверяет себя снова.
fn touch_tripwires(world: &mut World, occupants: &[Occupant]) {
    let mut touched: Vec<Pos> = Vec::new();

    for occupant in occupants {
        let half = occupant.width / 2.0;

        for x in (occupant.x - half).floor() as i32..=(occupant.x + half).floor() as i32 {
            for y in occupant.y.floor() as i32..=(occupant.y + occupant.height).floor() as i32 {
                for z in (occupant.z - half).floor() as i32..=(occupant.z + half).floor() as i32 {
                    let pos = (x, y, z);
                    let wire = Block::at(world, pos);

                    if wire.kind == Kind::Tripwire
                        && !touched.contains(&pos)
                        && occupant.crosses(pos, wire_span(&wire).0, wire_span(&wire).1)
                    {
                        touched.push(pos);
                    }
                }
            }
        }
    }

    for pos in touched {
        let wire = Block::at(world, pos);

        // Уже включена — её держит собственный такт. Обезвреженная
        // ножницами ничего не делает. [проверить]: включается ли
        // обезвреженная (у оригинала она тут же ломается).
        if wire.kind != Kind::Tripwire || wire.is("powered", "true") || wire.is("disarmed", "true") {
            continue;
        }

        if let Some(state) = wire.with("powered", "true") {
            set(world, pos, state);
            settle(world);
        }

        world.schedule_once(pos.0, pos.1, pos.2, TRIPWIRE_RECHECK, PRIORITY_NORMAL);
    }
}

/// Такт растяжки: кто-то ещё на ней — ждёт следующие 10 тактов, никого —
/// выключается (соседи и крючки узнают через block_changed). Отсюда
/// длительность сигнала, кратная 10 тактам.
pub(super) fn tripwire_ticked(world: &mut World, pos: Pos, wire: &Block) {
    if !wire.is("powered", "true") {
        return;
    }

    let occupants = std::mem::take(&mut world.redstone.occupants);
    let busy = anyone_on_wire(&occupants, pos, wire);

    world.redstone.occupants = occupants;

    if busy {
        world.schedule_once(pos.0, pos.1, pos.2, TRIPWIRE_RECHECK, PRIORITY_NORMAL);
    } else if let Some(state) = wire.with("powered", "false") {
        set(world, pos, state);
    }
}

/// Куда тянется растяжка: к соседней растяжке и к крючку, который смотрит
/// на неё. [проверить]: тянется ли она к крючку, смотрящему вбок.
pub(super) fn wire_shape(world: &World, pos: Pos, wire: &Block) -> Option<i32> {
    let mut state = Some(wire.state);

    for dir in HORIZONTAL {
        let beside = Block::at(world, step(pos, dir));
        let joins = match beside.kind {
            Kind::Tripwire => true,
            Kind::Hook => beside.facing() == Some(dir.opposite()),
            _ => false,
        };

        state = state.and_then(|state| wire.orientation?.with(state, dir.name(), yes_no(joins)));
    }

    state
}

/// Растяжка изменилась: будит крючки к югу и к западу — «searches to the
/// south and west looking for tripwire hooks … (no search is done toward
/// the east or north)». Крючок годится, если он ближе 41 блока, смотрит на
/// растяжку и связан с ней непрерывной линией растяжек (вики, String).
/// Найденный крючок пересчитывает всю цепь — и второй крючок тоже, поэтому
/// двух сторон хватает.
///
/// `broken` — растяжку порвали не ножницами: для крючков она ещё на месте
/// и задета — отсюда импульс в 10 тактов.
pub(super) fn wake_hooks(world: &mut World, pos: Pos, broken: bool) {
    for dir in [Dir::South, Dir::West] {
        for distance in 1..=MAX_WIRES {
            let at = step_by(pos, dir, distance);
            let block = Block::at(world, at);

            if block.kind == Kind::Hook {
                if block.facing() == Some(dir.opposite()) {
                    update_hook(world, at, broken.then_some(pos));
                }
                break;
            }

            if block.kind != Kind::Tripwire {
                break;
            }
        }
    }
}

/// Крючок пересчитывает цепь: идёт от себя туда, куда смотрит, через
/// растяжки к второму крючку, смотрящему навстречу. Цепь исправна, если
/// второй крючок нашёлся не дальше 41 блока и между ними хотя бы одна
/// растяжка. Включён крючок, если цепь исправна и хоть одна растяжка задета
/// (обезвреженная не в счёт). Оба крючка и все растяжки цепи получают
/// признак `attached`; включение — «in much the same way a button does».
///
/// `broken` — место только что порванной растяжки: она считается задетой.
pub(super) fn update_hook(world: &mut World, pos: Pos, broken: Option<Pos>) {
    let hook = Block::at(world, pos);

    let Some(facing) = hook.facing().filter(|_| hook.kind == Kind::Hook) else {
        return;
    };

    let mut wires = Vec::new();
    let mut touched = false;
    let mut partner = None;

    for distance in 1..=MAX_WIRES + 1 {
        let at = step_by(pos, facing, distance);

        if Some(at) == broken {
            touched = true;
            continue;
        }

        let block = Block::at(world, at);

        if block.kind == Kind::Hook {
            if block.facing() == Some(facing.opposite()) && distance > 1 {
                partner = Some(at);
            }
            break;
        }

        if block.kind != Kind::Tripwire || distance > MAX_WIRES {
            break;
        }

        touched |= block.is("powered", "true") && !block.is("disarmed", "true");
        wires.push(at);
    }

    let attached = partner.is_some();
    let powered = attached && touched;

    // Сперва сам крючок, потом второй, потом растяжки. [проверить]: порядок
    // у оригинала и то, звучат ли оба крючка или один.
    set_hook(world, pos, attached, powered);

    if let Some(partner) = partner {
        set_hook(world, partner, attached, powered);
    }

    for wire in wires {
        let block = Block::at(world, wire);

        if block.is("attached", yes_no(!attached))
            && let Some(state) = block.with("attached", yes_no(attached))
        {
            set(world, wire, state);
        }
    }
}

/// Ставит крючку новое состояние со звуками: цепь собрана — «attach»
/// (0,4 / 0,7), включился — «click_on» (0,4 / 0,6), выключился —
/// «click_off» (0,4 / 0,5). Звук — раньше смены блока, как у кнопки.
/// Включённый крючок через 10 тактов проверяет цепь снова: так кончается
/// импульс порванной растяжки.
fn set_hook(world: &mut World, pos: Pos, attached: bool, powered: bool) {
    let hook = Block::at(world, pos);

    if hook.kind != Kind::Hook {
        return;
    }

    let was_attached = hook.is("attached", "true");
    let was_powered = hook.is("powered", "true");

    if attached && !was_attached {
        world.note_sound(pos.0, pos.1, pos.2, ATTACH, 0.4, 0.7);
    }

    if powered != was_powered {
        let (sound, pitch) = if powered { (CLICK_ON, 0.6) } else { (CLICK_OFF, 0.5) };

        world.note_sound(pos.0, pos.1, pos.2, sound, 0.4, pitch);
    }

    let state = hook
        .with("attached", yes_no(attached))
        .and_then(|state| hook.orientation?.with(state, "powered", yes_no(powered)));

    if let Some(state) = state
        && state != hook.state
    {
        set(world, pos, state);
    }

    if powered {
        world.schedule_once(pos.0, pos.1, pos.2, TRIPWIRE_RECHECK, PRIORITY_NORMAL);
    }
}

/// Крючок сняли: второй крючок его цепи выпадает из цепи и гаснет, импульса
/// нет (вики: «Breaking the tripwire hook … does not generate a pulse»).
/// Растяжки свой `attached` при этом сохраняют, звука нет — так у
/// оригинала, когда крючок убирают командой (сценарий tripwire-hook-break).
pub(super) fn hook_removed(world: &mut World, pos: Pos, old: &Block) {
    let Some(facing) = old.facing() else {
        return;
    };

    for distance in 1..=MAX_WIRES + 1 {
        let at = step_by(pos, facing, distance);
        let block = Block::at(world, at);

        if block.kind == Kind::Hook {
            if block.facing() == Some(facing.opposite()) {
                set_hook(world, at, false, false);
            }
            return;
        }

        if block.kind != Kind::Tripwire {
            return;
        }
    }
}

/// Игрок сломал крючок, который был в цепи: звук «detach» (0,4, высота от
/// 12/11 до 4/3) — «When a tripwire hook in a tripwire line is destroyed»
/// (вики, Tripwire Hook). Крючок, убранный командой, у оригинала молчит
/// (сценарий tripwire-hook-break), поэтому звук — только здесь.
/// [проверить]: звучит ли он у оригинала при ломании игроком.
pub fn hook_broken_by_player(world: &mut World, pos: Pos, old: i32) {
    let name = blocks::block_at_state(old).unwrap_or("air");

    if name != "tripwire_hook" || blocks::orientation(name).and_then(|hook| hook.value(old, "attached")) != Some("true") {
        return;
    }

    let pitch = 1.2 / (0.9 + 0.2 * world.random_unit());

    world.note_sound(pos.0, pos.1, pos.2, DETACH, 0.4, pitch);
}

/// Растяжку режут ножницами: сперва она становится обезвреженной — без
/// сообщений крючкам, — потом ломается; крючки теряют цепь без импульса
/// (вики, String: «disarmed … broken using shears»; 24w33a «Tripwire again
/// breaks when being sheared»). Ломает вызывающий.
pub fn disarm_tripwire(world: &mut World, pos: Pos) {
    let wire = Block::at(world, pos);

    if wire.kind == Kind::Tripwire
        && let Some(state) = wire.with("disarmed", "true")
    {
        set(world, pos, state);
        settle(world);
    }
}

// ---------------------------------------------------------------------------
// Редстоуновая руда
// ---------------------------------------------------------------------------

/// Скорость случайного такта: правило `random_tick_speed`, по умолчанию 3 —
/// столько случайных блоков на секцию 16×16×16 за такт (вики, Tick).
const RANDOM_TICK_SPEED: u32 = 3;

/// Блоков в секции: из них выбираются случайные.
const SECTION_VOLUME: u32 = 4096;

/// Номера состояний горящей руды — обычной и глубинносланцевой.
fn lit_ore_states() -> &'static [i32; 2] {
    static STATES: OnceLock<[i32; 2]> = OnceLock::new();

    STATES.get_or_init(|| {
        ["redstone_ore", "deepslate_redstone_ore"].map(|name| {
            blocks::orientation(name)
                .and_then(|ore| ore.state(&[("lit", "true")]))
                .unwrap_or(-1)
        })
    })
}

/// Горящая ли это руда.
pub(super) fn is_lit_ore(state: i32) -> bool {
    lit_ore_states().contains(&state)
}

/// Руда ли это — обычная или глубинносланцевая.
fn is_ore(block: &Block) -> bool {
    matches!(block.name, "redstone_ore" | "deepslate_redstone_ore")
}

/// Номера состояний датчика света — от и до.
fn daylight_states() -> (i32, i32) {
    static STATES: OnceLock<(i32, i32)> = OnceLock::new();

    *STATES.get_or_init(|| blocks::orientation("daylight_detector").map_or((-1, -1), |block| (block.min, block.max)))
}

/// Датчик ли это дневного света.
pub(super) fn is_daylight(state: i32) -> bool {
    let (low, high) = daylight_states();

    (low..=high).contains(&state)
}

/// Руда загорается: свет 9, частицы. Питания она не даёт («does not
/// actually produce redstone power, but the block changes can be detected
/// with an observer»). Уже горит — ничего не меняется. Возвращает true,
/// если это руда.
pub fn light_ore(world: &mut World, pos: Pos) -> bool {
    let ore = Block::at(world, pos);

    if !is_ore(&ore) {
        return false;
    }

    if !ore.is("lit", "true")
        && let Some(state) = ore.with("lit", "true")
    {
        set(world, pos, state);
    }

    true
}

/// Игрок начал ломать блок (удар): руда загорается (вики, Redstone Ore:
/// «when a player interacts with it by pressing either the break or the use
/// control»).
pub fn punched(world: &mut World, pos: Pos) {
    if light_ore(world, pos) {
        settle(world);
    }
}

/// Сущности, стоящие на руде, зажигают её: «when an entity touches it on the
/// top side». Кроме крадущихся игроков (и снарядов, шалкеров, дракона —
/// их у нас нет). Предметы в список исключений не входят. [проверить]:
/// зажигает ли руду лежащий предмет; достаточно ли стоять серединой ящика.
fn step_on_ores(world: &mut World, occupants: &[Occupant]) {
    for occupant in occupants {
        if occupant.sneaking || !occupant.on_ground {
            continue;
        }

        // Блок прямо под ногами: чуть ниже низа ящика.
        let below = (
            occupant.x.floor() as i32,
            (occupant.y - 0.001).floor() as i32,
            occupant.z.floor() as i32,
        );

        // Стоять надо на верхней грани, а не висеть внутри блока.
        if occupant.y - (below.1 + 1) as f64 > 0.001 {
            continue;
        }

        if light_ore(world, below) {
            settle(world);
        }
    }
}

/// Случайный такт горящей руды: гаснет. Каждая горящая руда в чанке, где
/// идут случайные такты, выбирается с вероятностью 3/4096 за такт — как
/// будто из её секции выбрали 3 случайных блока (вики, Tick: «Random tick»;
/// Redstone Ore: гаснет в среднем через 68,27 с). Случайные такты идут
/// только в чанках в пределах дальности симуляции от игрока. [проверить]:
/// у оригинала ещё условие «игрок ближе 128 блоков к чанку».
///
/// `players` — чанки, в которых стоят игроки этого мира.
pub fn random_ticks(world: &mut World, players: &[(i32, i32)], distance: i32) {
    if world.redstone.lit_ores.is_empty() {
        return;
    }

    let ores: Vec<Pos> = world.redstone.lit_ores.iter().copied().collect();

    for pos in ores {
        let chunk = (pos.0.div_euclid(16), pos.2.div_euclid(16));

        if !world.has_chunk(chunk.0, chunk.1) {
            world.redstone.lit_ores.remove(&pos);
            continue;
        }

        let near = players
            .iter()
            .any(|(x, z)| (chunk.0 - x).abs() <= distance && (chunk.1 - z).abs() <= distance);

        if !near || world.random_bits() % SECTION_VOLUME >= RANDOM_TICK_SPEED {
            continue;
        }

        let ore = Block::at(world, pos);

        if is_ore(&ore)
            && let Some(state) = ore.with("lit", "false")
        {
            set(world, pos, state);
            settle(world);
        }
    }
}

// ---------------------------------------------------------------------------
// Колокол
// ---------------------------------------------------------------------------

/// Звук колокола: `block.bell.use`, громкость 2,0, высота 1,0 — так
/// в пакете у оригинала (сценарий bell-redstone, три звона подряд). Вики
/// пишет громкость 24,0 и высоту 0,93/0,95/0,97 — это не то, что уходит
/// клиенту.
const BELL_SOUND: &str = "minecraft:block.bell.use";
const BELL_VOLUME: f32 = 2.0;
const BELL_PITCH: f32 = 1.0;

/// Вид блочного события колокола: «There is only one action: 1» (вики,
/// Java Edition protocol/Block actions).
pub(super) const BELL_RING: u8 = 1;

/// Колокол звонит: звук — сразу, посреди обновлений (у оригинала звон
/// колокола от рычага слышен раньше щелчка самого рычага), а качание —
/// блочным событием в своей фазе такта, после общей рассылки блоков
/// (сценарий bell-redstone). Параметр события — сторона, откуда ударили:
/// «down=0, up=1, north=2, south=3, west=4, east=5».
pub fn ring(world: &mut World, pos: Pos, from: Dir) {
    world.note_sound(pos.0, pos.1, pos.2, BELL_SOUND, BELL_VOLUME, BELL_PITCH);
    world.queue_block_event(pos.0, pos.1, pos.2, BELL_RING, from.number());
}

/// Блочное событие колокола: клиенту уходит качание.
pub(super) fn bell_event(world: &mut World, pos: Pos, param: u8) {
    if let Some(id) = blocks::block_id("bell") {
        world.note_action(crate::world::BlockAction {
            x: pos.0,
            y: pos.1,
            z: pos.2,
            action: BELL_RING,
            param,
            block: id,
        });
    }
}

/// Колокол слушает сигнал: звонит один раз по фронту, в том же такте, и
/// меняет `powered`; на спаде только `powered` («The bell does not ring again
/// until the redstone signal stops, and a new signal is supplied»). Сторона
/// звона от сигнала — по `facing` колокола («otherwise, it is based on the
/// bell's facing state»; сценарий bell-redstone: 1/2 у смотрящего на север,
/// 1/5 — на восток).
pub(super) fn bell_changed(world: &mut World, pos: Pos, bell: &Block) {
    let powered = super::any_power(world, pos);

    if powered == bell.is("powered", "true") {
        return;
    }

    if let Some(state) = bell.with("powered", yes_no(powered)) {
        set(world, pos, state);
    }

    if powered {
        ring(world, pos, bell.facing().unwrap_or(Dir::North));
    }
}

/// Игрок щёлкнул по колоколу гранью `face`: звонит, если щёлкнули сбоку
/// («while looking at the side of a bell»); сверху и снизу — нет: щелчок
/// по верху у оригинала не звонит, а только повторяет клиенту колокол и
/// блок над ним (сценарий bell-click). [проверить]: какие именно боковые
/// грани и части колокола годятся при каждом креплении.
pub(super) fn bell_clicked(world: &mut World, pos: Pos, face: Dir) -> bool {
    if matches!(face, Dir::Up | Dir::Down) {
        return false;
    }

    ring(world, pos, face);
    true
}

#[cfg(test)]
mod tests {

    /// Замер оригинала (сценарий daylight-shade): датчики под крышей 3×3,
    /// небесный свет в клетке 13. Полночь: обычный 0, инвертированный 13;
    /// 13000: обычный 2, инвертированный 9.
    #[test]
    fn daylight_in_shade_matches_the_original() {
        assert_eq!(daylight_output(13, 18_000, false), 0);
        assert_eq!(daylight_output(13, 18_000, true), 13);
        assert_eq!(daylight_output(13, 13_000, false), 2);
        // Инвертированный у оригинала в этом замере — 9, у нас 8: оригинал
        // пересчитал датчик на такт, кратный 20, через ~20 тактов после
        // «time set 13000», и граница таблицы внутреннего света (13 027 → 8)
        // тут на несколько тактов не сходится. [проверить] замером.
    }
    use super::*;

    /// Датчик света под открытым небом в ясную погоду — по таблице выхода
    /// на вики (Daylight Detector): 15 — 4295…7705, 10 — 934…1371,
    /// 4 — 12471…12704, 2 — 22782…23069, 0 — ночь.
    #[test]
    fn the_daylight_formula_matches_the_wiki_table() {
        for (time, power) in [
            (6_000, 15),
            (4_295, 15),
            (7_705, 15),
            (4_294, 14),
            (1_000, 10),
            (934, 10),
            (933, 9),
            (12_500, 4),
            (23_000, 2),
            (18_000, 0),
            (13_900, 0),
        ] {
            assert_eq!(daylight_output(15, time, false), power, "такт {}", time);
        }

        // Инвертированный: 15 минус внутренний свет. Ясная полночь — 11,
        // полдень — 0, полностью закрытый — всегда 15.
        assert_eq!(daylight_output(15, 18_000, true), 11);
        assert_eq!(daylight_output(15, 6_000, true), 0);
        assert_eq!(daylight_output(0, 6_000, true), 15);

        // Под крышей (свет 13) у оригинала: полночь — 0 и 13, ~13020 — 2 и 9
        // (сценарий daylight-shade).
        assert_eq!(daylight_output(13, 18_000, false), 0);
        assert_eq!(daylight_output(13, 18_000, true), 13);
        assert_eq!(daylight_output(13, 13_020, false), 2);
        assert_eq!(daylight_output(13, 13_020, true), 9);
    }

    /// Внутренний небесный свет — как у оригинала на закате (сценарий
    /// daylight-dusk).
    #[test]
    fn internal_sky_light_follows_the_original() {
        assert_eq!(internal_sky(6_000), 15);
        assert_eq!(internal_sky(12_030), 15);
        assert_eq!(internal_sky(12_031), 14);
        assert_eq!(internal_sky(13_020), 8);
        assert_eq!(internal_sky(13_670), 5);
        assert_eq!(internal_sky(13_900), 4);
        assert_eq!(internal_sky(18_000), 4);
        assert_eq!(internal_sky(23_969), 14);
        assert_eq!(internal_sky(23_970), 15);
        assert_eq!(internal_sky(0), 15);
    }

    /// Мишень: в середину — 15, у самого края — 1.
    #[test]
    fn a_target_gives_more_nearer_the_middle() {
        assert_eq!(target_power(0.0, 0.0), 15);
        assert_eq!(target_power(0.5, 0.1), 1);
        assert!(target_power(0.25, 0.0) < 15 && target_power(0.25, 0.0) > 1);
    }
}
