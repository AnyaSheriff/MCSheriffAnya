// Жидкости: вода и лава.
//
// Жидкость — единственное, что меняется в мире само, без игрока. Поэтому
// здесь только правила: «что должно быть в этом месте, если вокруг вот такое».
// Кто и когда их применяет — дело такта мира (src/tick.rs), а сами правила ни
// про сеть, ни про время ничего не знают и проверяются обычными тестами.
//
// Устройство жидкости в игре такое. У блока воды есть уровень: ноль — это
// источник, дальше единица, двойка и так до семи — чем дальше от источника,
// тем тоньше слой. Восьмёрка означает падающую воду: ту, что льётся сверху.
// У лавы всё то же, только растекается она ближе и медленнее.
//
// Считается всё «притяжением, а не толканием»: каждое место само смотрит по
// сторонам и решает, что в нём должно быть. Так проще: не нужно помнить, кто
// кого залил, и вода сама высыхает, когда источник убрали.
//
// Тонкость, без которой вода ведёт себя неправильно: она течёт не во все
// стороны подряд, а туда, где ближе спуск. Поэтому у каждой стороны считается
// «вес» — за сколько шагов оттуда можно спуститься, — и жидкость идёт только
// в самые лёгкие стороны. Из-за этого ручей к обрыву получается узким,
// а не разливается лужей.
//
// Всё описанное взято с minecraft.wiki — страницы Water, Lava и Fluid.

use std::sync::OnceLock;

use crate::blocks;
use crate::world::AIR;

/// Уровень источника.
pub const SOURCE: i32 = 0;

/// Уровень падающей жидкости — той, что льётся сверху.
pub const FALLING: i32 = 8;

/// Как далеко растекается вода по ровному месту.
const WATER_REACH: i32 = 7;

/// Как далеко растекается лава — заметно ближе воды. Уровень у неё растёт
/// на два за блок (2, 4, 6 — снято чёрным ящиком, сценарий water-meets-lava),
/// так что три блока — это уровень 6.
const LAVA_REACH: i32 = 6;

/// За сколько шагов вода ищет спуск, выбирая, куда течь.
const WATER_SEARCH: i32 = 4;

/// Лава смотрит вперёд не так далеко.
const LAVA_SEARCH: i32 = 2;

/// Через сколько тактов вода делает следующий шаг.
pub const WATER_DELAY: u64 = 5;

/// Лава течёт медленно: блок за полтора десятка тактов с лишним.
pub const LAVA_DELAY: u64 = 30;

/// Сколько источников рядом должно быть, чтобы место само стало источником.
const SOURCES_FOR_NEW: usize = 2;

/// Четыре горизонтальных направления.
const SIDES: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

/// Какая это жидкость.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Water,
    Lava,
}

impl Kind {
    /// Промежуток состояний блока этой жидкости.
    pub fn states(self) -> (i32, i32) {
        match self {
            Kind::Water => range(&WATER_STATES, "water"),
            Kind::Lava => range(&LAVA_STATES, "lava"),
        }
    }

    /// Докуда растекается. Лава в Незере — как вода, на семь блоков
    /// (вики, Lava).
    fn reach(self) -> i32 {
        match self {
            Kind::Water => WATER_REACH,
            Kind::Lava if in_nether() => WATER_REACH,
            Kind::Lava => LAVA_REACH,
        }
    }

    /// На сколько растёт уровень за блок растекания.
    fn step(self) -> i32 {
        match self {
            Kind::Lava if !in_nether() => 2,
            _ => 1,
        }
    }

    /// За сколько шагов ищет спуск, выбирая сторону.
    fn search(self) -> i32 {
        match self {
            Kind::Water => WATER_SEARCH,
            Kind::Lava if in_nether() => WATER_SEARCH,
            Kind::Lava => LAVA_SEARCH,
        }
    }

    /// Через сколько тактов делается следующий шаг. Лава в Незере — втрое
    /// быстрее (вики, Lava: раз в 10 тактов).
    pub fn delay(self) -> u64 {
        self.delay_in(false)
    }

    /// То же для мира, который Незер или нет.
    pub fn delay_in(self, nether: bool) -> u64 {
        match self {
            Kind::Water => WATER_DELAY,
            Kind::Lava if nether => NETHER_LAVA_DELAY,
            Kind::Lava => LAVA_DELAY,
        }
    }
}

/// Через сколько тактов делает шаг лава в Незере.
pub const NETHER_LAVA_DELAY: u64 = 10;

thread_local! {
    /// Считается ли сейчас течение в Незере — ставится на время расчёта
    /// одного места (`in_world`), чтобы не протаскивать признак через все
    /// правила.
    static NETHER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn in_nether() -> bool {
    NETHER.with(|nether| nether.get())
}

/// Выполняет расчёт по правилам мира: в Незере лава течёт дальше и чаще.
pub fn in_world<R>(nether: bool, calculate: impl FnOnce() -> R) -> R {
    let before = NETHER.with(|flag| flag.replace(nether));
    let result = calculate();
    NETHER.with(|flag| flag.set(before));
    result
}

/// Промежутки состояний, которые спрашиваются на каждом шагу.
///
/// Ищутся они перебором по таблице блоков, а спрашиваются десятки раз на
/// каждое место, — поэтому найденное запоминается раз и навсегда.
static WATER_STATES: OnceLock<(i32, i32)> = OnceLock::new();
static LAVA_STATES: OnceLock<(i32, i32)> = OnceLock::new();
static POWDER_SNOW_STATES: OnceLock<(i32, i32)> = OnceLock::new();

/// Промежуток состояний блока с этим именем — с запоминанием.
fn range(cache: &'static OnceLock<(i32, i32)>, name: &'static str) -> (i32, i32) {
    *cache.get_or_init(|| {
        blocks::states_of(name).unwrap_or_else(|| panic!("блок {} есть в таблице", name))
    })
}

/// Жидкость и её уровень в этом состоянии блока. None — это не жидкость.
pub fn fluid_at(state: i32) -> Option<(Kind, i32)> {
    for kind in [Kind::Water, Kind::Lava] {
        let (low, high) = kind.states();

        if (low..=high).contains(&state) {
            return Some((kind, state - low));
        }
    }

    None
}

/// Рыхлый ли это снег.
///
/// Он не жидкость, но жидкости его касается: вода и лава его не обтекают,
/// а уничтожают.
pub fn is_powder_snow(state: i32) -> bool {
    let (low, high) = range(&POWDER_SNOW_STATES, "powder_snow");

    (low..=high).contains(&state)
}

/// Состояние блока для жидкости такого уровня.
pub fn state_of(kind: Kind, level: i32) -> i32 {
    kind.states().0 + level
}

/// Пустое ли место для жидкости в самом узком смысле: воздух, рыхлый снег
/// или сама жидкость.
///
/// Этим мир пользуется, когда будит воду и лаву в только что сложенном или
/// прочитанном чанке: вода у берега, рядом с которой растёт трава, сама по
/// себе траву не смывает, пока её не потревожат. Течению же годится и то,
/// что поток сносит, — это `flows_through`.
pub fn can_flow_into(state: i32) -> bool {
    state == AIR || is_powder_snow(state) || fluid_at(state).is_some()
}

/// Место, куда жидкость может затечь: пустое, занятое ею же или занятое тем,
/// что она уничтожает.
///
/// Рыхлый снег вода и лава не обтекают, а ломают, — поэтому он считается
/// проходимым. То же с тем, что поток сносит (`washed_away`), и с огнём:
/// «Spreading water extinguishes fire» (вики, Water).
pub fn flows_through(state: i32) -> bool {
    can_flow_into(state) || washed_away(state) || is_fire(state)
}

/// Сносит ли текущая жидкость этот блок.
///
/// Вики, Water: «washes away certain types of items or placed blocks, causing
/// them to drop as items ... plants (except trees), snow, torches, ... redstone
/// dust and some other redstone components, cobweb, end rods, heads, and
/// flower pots». Какие именно детали редстоуна, сказано на их страницах:
/// повторитель, сравнитель, кнопка, крюк растяжки, красный факел («water
/// or lava flows into its space»). Рельсы вода не сносит, а ковры — сносит
/// (снято чёрным ящиком, сценарий water-washes: у оригинала на месте ковра
/// вода и выпавший ковёр). Лава сносит то же самое, только без выпадения
/// (вики, Lava).
///
/// Рычаг и нажимная плита на вики не названы — не сносятся (сверить
/// чёрным ящиком).
pub fn washed_away(state: i32) -> bool {
    // Спрашивают об этом на каждом шагу поиска спуска, а ответ зависит только
    // от состояния, — поэтому он считается один раз для всех состояний.
    static WASHED: OnceLock<Vec<bool>> = OnceLock::new();

    let table = WASHED.get_or_init(|| {
        (0..)
            .map_while(|state| blocks::block_at_state(state).map(|_| washed_away_by_name(state)))
            .collect()
    });

    usize::try_from(state).ok().and_then(|state| table.get(state)).copied().unwrap_or(false)
}

/// То же, что `washed_away`, но по имени блока — без запоминания.
fn washed_away_by_name(state: i32) -> bool {
    let Some(name) = blocks::block_at_state(state) else {
        return false;
    };

    crate::plants::washed_away(name)
        || matches!(
            name,
            "snow" | "redstone_wire" | "repeater" | "comparator" | "tripwire_hook" | "end_rod" | "cobweb" | "flower_pot"
        )
        || name.ends_with("torch")
        || name.ends_with("_button")
        || name.ends_with("_carpet")
        || name.starts_with("potted_")
        || name.ends_with("_head")
        || name.ends_with("_skull")
}

/// Огонь: вода его гасит, лава заливает.
fn is_fire(state: i32) -> bool {
    static FIRE: OnceLock<[(i32, i32); 2]> = OnceLock::new();

    let [fire, soul] = *FIRE.get_or_init(|| {
        let range = |name| blocks::states_of(name).unwrap_or((-1, -1));

        [range("fire"), range("soul_fire")]
    });

    (fire.0..=fire.1).contains(&state) || (soul.0..=soul.1).contains(&state)
}

/// Может ли это место принять жидкость сверху.
///
/// Пустое — может, и то, что поток сносит, — тоже. Занятое текущей
/// жидкостью — тоже: она не заполняет блок целиком, и сверху в неё
/// продолжает литься. А источник полон, и принимать ему нечего.
fn drains_into(state: i32) -> bool {
    match fluid_at(state) {
        Some((_, SOURCE)) => false,
        Some(_) => true,
        None => flows_through(state),
    }
}

/// Каким должно стать это место.
///
/// `block` отвечает, что стоит в любом месте мира: правилам нужно смотреть не
/// только на соседей, но и на несколько блоков вперёд — туда, где может быть
/// спуск.
///
/// None означает «менять нечего».
pub fn update<F>(at: (i32, i32, i32), block: &F) -> Option<i32>
where
    F: Fn(i32, i32, i32) -> i32,
{
    let (x, y, z) = at;
    let here = block(x, y, z);

    // Встреча воды и лавы. Правила для разных сторон разные, поэтому
    // разбираются отдельно и раньше всего.
    if let Some(mixed) = mixing(at, here, block) {
        return Some(mixed);
    }

    // Источник сам по себе не меняется: его ставит игрок, он же его и убирает.
    if matches!(fluid_at(here), Some((_, SOURCE))) {
        return None;
    }

    // Твёрдый блок жидкость не трогает.
    if !flows_through(here) {
        return None;
    }

    let supply = supply(at, block);

    // Жидкости здесь нет, и прийти ей неоткуда — место остаётся, каким было.
    // Высыхает только сама жидкость: рыхлый снег или цветок, рядом с которым
    // что-то поменялось, пропадать не должны.
    if supply.is_none() && fluid_at(here).is_none() {
        return None;
    }

    // Вода между двумя источниками сама становится источником — отсюда
    // берётся бесконечная вода.
    if let Some((Kind::Water, _)) = supply
        && becomes_source(at, block)
    {
        let source = state_of(Kind::Water, SOURCE);

        return (here != source).then_some(source);
    }

    let wanted = match supply {
        Some((kind, level)) => state_of(kind, level),
        // Питать это место нечем — оно высыхает.
        None => AIR,
    };

    (wanted != here).then_some(wanted)
}

/// Высота поверхности воды в клетке над её низом: источник 8/9, каждый
/// уровень ниже на девятую, падающая (или с водой сверху) — целый блок.
fn water_height<F>(at: (i32, i32, i32), block: &F) -> Option<f64>
where
    F: Fn(i32, i32, i32) -> i32,
{
    let (Kind::Water, level) = fluid_at(block(at.0, at.1, at.2))? else {
        return None;
    };

    if level >= FALLING || matches!(fluid_at(block(at.0, at.1 + 1, at.2)), Some((Kind::Water, _))) {
        return Some(1.0);
    }

    Some(f64::from(8 - level) / 9.0)
}

/// Вода в клетке для выпавшего предмета: высота и куда она течёт.
///
/// Течение — сумма по четырём соседям: к соседу с водой ниже — на разницу
/// высот; к пустому соседу — только если под ним вода (тогда вода туда
/// стекает). Пустой сосед над сухим полом течения не даёт — у оригинала
/// предмет у края ручья дальше не толкается (tools/blackbox/itemflow.js).
pub fn item_water<F>(at: (i32, i32, i32), block: &F) -> Option<crate::items::ItemWater>
where
    F: Fn(i32, i32, i32) -> i32,
{
    const FULL: f64 = 8.0 / 9.0;

    let height = water_height(at, block)?;
    let (x, y, z) = at;
    let (mut fx, mut fz) = (0.0, 0.0);

    for (dx, dz) in SIDES {
        let side = (x + dx, y, z + dz);
        let there = block(side.0, side.1, side.2);

        let difference = match water_height(side, block) {
            Some(other) => height - other,
            None if fluid_at(there).is_none() && flows_through(there) => {
                match water_height((side.0, y - 1, side.2), block) {
                    Some(below) => height - (below - FULL),
                    None => 0.0,
                }
            }
            None => 0.0,
        };

        fx += f64::from(dx) * difference;
        fz += f64::from(dz) * difference;
    }

    let length = (fx * fx + fz * fz).sqrt();
    let flow = if length > 1e-9 { (fx / length, fz / length) } else { (0.0, 0.0) };

    Some(crate::items::ItemWater { height, flow })
}

/// Лава, которой коснулась вода (сверху или сбоку), твердеет сразу — при
/// обновлении соседа, а не своим тактом: у оригинала булыжник появляется в
/// том же такте, что и вода рядом (сценарий чёрного ящика water-meets-lava).
pub fn hardens_now<F>(at: (i32, i32, i32), block: &F) -> Option<i32>
where
    F: Fn(i32, i32, i32) -> i32,
{
    let here = block(at.0, at.1, at.2);

    matches!(fluid_at(here), Some((Kind::Lava, _)))
        .then(|| mixing(at, here, block))
        .flatten()
}

/// Во что превращается это место от встречи воды и лавы.
///
/// Правил три, и они несимметричны:
/// — текущая лава, которой вода коснулась сбоку или сверху, становится
///   булыжником;
/// — источник лавы в том же случае становится обсидианом;
/// — вода, в которую лава льётся сверху, становится камнем.
fn mixing<F>(at: (i32, i32, i32), here: i32, block: &F) -> Option<i32>
where
    F: Fn(i32, i32, i32) -> i32,
{
    let (x, y, z) = at;

    match fluid_at(here)? {
        // Лава: смотрим, нет ли воды сверху или сбоку. Вода снизу лаву не
        // трогает — она сама превратится в камень.
        (Kind::Lava, level) => {
            let water_near = [(x, y + 1, z)]
                .into_iter()
                .chain(SIDES.map(|(dx, dz)| (x + dx, y, z + dz)))
                .any(|(x, y, z)| matches!(fluid_at(block(x, y, z)), Some((Kind::Water, _))));

            water_near.then(|| hardened(level))
        }
        // Вода, в которую льётся лава сверху, становится камнем.
        (Kind::Water, _) => matches!(fluid_at(block(x, y + 1, z)), Some((Kind::Lava, _)))
            .then(|| named_state("stone")),
    }
}

/// Откуда это место питается и каким уровнем.
///
/// Сверху льётся — значит, падающая. Иначе берём того соседа сбоку, который
/// даёт самый толстый слой, и добавляем к его уровню единицу.
fn supply<F>(at: (i32, i32, i32), block: &F) -> Option<(Kind, i32)>
where
    F: Fn(i32, i32, i32) -> i32,
{
    let (x, y, z) = at;

    // Жидкость питается только своей же жидкостью: вода рядом с лавой лавой
    // не становится — встречу воды с лавой разбирает `mixing`, и там
    // меняется лава, а не вода. Пустое место берёт то, что даёт слой толще.
    let own = fluid_at(block(x, y, z)).map(|(kind, _)| kind);

    // Жидкость, над которой жидкость, — падающая, даже если под ней пол:
    // низ водопада у оригинала тоже падающий, а растекание от него
    // начинается с первого уровня (сценарий чёрного ящика waterfall).
    if let Some((kind, _)) = fluid_at(block(x, y + 1, z)) {
        return Some((kind, FALLING));
    }

    let mut best: Option<(Kind, i32)> = None;

    for (side, (dx, dz)) in SIDES.iter().enumerate() {
        let neighbour = (x + dx, y, z + dz);

        let Some((kind, level)) = fluid_at(block(neighbour.0, neighbour.1, neighbour.2)) else {
            continue;
        };

        if own.is_some_and(|own| own != kind) {
            continue;
        }

        // Падающая жидкость вбок растекается, только если ей некуда падать
        // дальше, — и тогда как источник, первым уровнем.
        let next = if level >= FALLING {
            if drains_into(block(neighbour.0, neighbour.1 - 1, neighbour.2)) {
                continue;
            }

            kind.step()
        } else {
            level + kind.step()
        };

        if next > kind.reach() {
            continue;
        }

        // Течёт ли сосед в нашу сторону: он выбирает направление по тому,
        // где ближе спуск.
        if !flows_towards(neighbour, opposite(side), kind, block) {
            continue;
        }

        // Если в пустое место могут прийти и вода, и лава, у оригинала
        // всё решает порядок их тактов (сценарий water-meets-lava: где-то
        // входит лава и твердеет, где-то вода). Порядка тактов источников
        // мы здесь не знаем — берём слой толще; расходится в паре клеток.
        best = match best {
            Some((_, best_level)) if best_level <= next => best,
            _ => Some((kind, next)),
        };
    }

    best
}

/// Течёт ли жидкость из этого места в эту сторону.
///
/// Сперва главное: если ей есть куда падать, вбок она не идёт вовсе. Иначе
/// считаются веса всех четырёх сторон — за сколько шагов оттуда находится
/// спуск, — и жидкость течёт только в самые лёгкие стороны.
fn flows_towards<F>(from: (i32, i32, i32), side: usize, kind: Kind, block: &F) -> bool
where
    F: Fn(i32, i32, i32) -> i32,
{
    let (x, y, z) = from;
    let (dx, dz) = SIDES[side];

    if !flows_through(block(x + dx, y, z + dz)) {
        return false;
    }

    // Источник разливается вбок даже над дырой, а текущая жидкость — нет:
    // она целиком уходит вниз.
    let is_source = matches!(fluid_at(block(x, y, z)), Some((_, SOURCE)));

    if !is_source && drains_into(block(x, y - 1, z)) {
        return false;
    }

    // Веса всех четырёх сторон — на стеке, без списка в куче.
    let weights: [Option<i32>; 4] = std::array::from_fn(|side| weight(from, side, kind, block));

    match weights.iter().flatten().min() {
        // Спуск виден — течём только в самые лёгкие стороны.
        Some(lightest) => weights[side] == Some(*lightest),
        // Спуска не видно ни с одной стороны — растекаемся ровно во все.
        None => true,
    }
}

/// Вес стороны: за сколько шагов от неё находится спуск.
///
/// None — затечь туда нельзя или спуска в пределах видимости нет.
fn weight<F>(from: (i32, i32, i32), side: usize, kind: Kind, block: &F) -> Option<i32>
where
    F: Fn(i32, i32, i32) -> i32,
{
    let (x, y, z) = from;
    let (dx, dz) = SIDES[side];
    let start = (x + dx, z + dz);

    if !flows_through(block(start.0, y, start.1)) {
        return None;
    }

    // Обход в ширину: сперва места в одном шаге, потом в двух и так далее.
    // Как только нашли место, из-под которого можно спуститься, это и есть
    // ответ — дальше искать незачем.
    //
    // Дальше SEEN_REACH шагов от исходного места обход не уходит, поэтому
    // пройденное помнится в квадрате на стеке, а не в множестве в куче.
    let origin = (x, z);
    let mut seen = [[false; SEEN_SIDE]; SEEN_SIDE];
    let mut mark = |(px, pz): (i32, i32)| -> bool {
        let (i, j) = (px - origin.0 + SEEN_REACH, pz - origin.1 + SEEN_REACH);

        // За краем квадрата обход не бывает, но на всякий случай такое место
        // просто считается уже пройденным.
        if !(0..SEEN_SIDE as i32).contains(&i) || !(0..SEEN_SIDE as i32).contains(&j) {
            return false;
        }

        let cell = &mut seen[i as usize][j as usize];
        let fresh = !*cell;

        *cell = true;
        fresh
    };

    mark(origin);
    mark(start);

    let mut edge = vec![start];

    for distance in 0..=kind.search() {
        let mut next = Vec::new();

        for (x, z) in edge {
            if drains_into(block(x, y - 1, z)) {
                return Some(distance);
            }

            for (dx, dz) in SIDES {
                let step = (x + dx, z + dz);

                if !flows_through(block(step.0, y, step.1)) || !mark(step) {
                    continue;
                }

                next.push(step);
            }
        }

        edge = next;
    }

    None
}

/// Насколько далеко от исходного места заходит обход `weight`: первая
/// сторона — шаг, дальше не больше `WATER_SEARCH + 1` шагов.
const SEEN_REACH: i32 = WATER_SEARCH + 2;

/// Сторона квадрата, в котором `weight` помнит пройденное.
const SEEN_SIDE: usize = (2 * SEEN_REACH + 1) as usize;

/// Становится ли это место источником воды.
///
/// Так и получается бесконечная вода: место, у которого по сторонам два
/// источника, а под ногами твёрдый блок или такой же источник, само
/// становится источником.
fn becomes_source<F>(at: (i32, i32, i32), block: &F) -> bool
where
    F: Fn(i32, i32, i32) -> i32,
{
    let (x, y, z) = at;

    let sources = SIDES
        .iter()
        .filter(|(dx, dz)| {
            matches!(
                fluid_at(block(x + dx, y, z + dz)),
                Some((Kind::Water, SOURCE))
            )
        })
        .count();

    if sources < SOURCES_FOR_NEW {
        return false;
    }

    let below = block(x, y - 1, z);

    // Держаться источнику надо на чём-то: на твёрдом блоке или на таком же
    // источнике. Над текущей водой он не появится — она утечёт.
    !flows_through(below) || matches!(fluid_at(below), Some((Kind::Water, SOURCE)))
}

/// Сторона, противоположная этой.
fn opposite(side: usize) -> usize {
    match side {
        0 => 1,
        1 => 0,
        2 => 3,
        _ => 2,
    }
}

/// Во что превращается лава, встретившая воду.
///
/// Источник даёт обсидиан, текущая лава — булыжник.
fn hardened(level: i32) -> i32 {
    named_state(if level == SOURCE {
        "obsidian"
    } else {
        "cobblestone"
    })
}

/// Состояние блока по его имени — то, которое ставит игра по умолчанию.
fn named_state(name: &'static str) -> i32 {
    blocks::state_by_name(name).unwrap_or_else(|| panic!("блок {} есть в таблице", name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Камень.
    const STONE: i32 = 1;

    /// Кусочек мира для проверок: что не задано — воздух.
    struct Place {
        blocks: HashMap<(i32, i32, i32), i32>,
    }

    impl Place {
        /// Ровный каменный пол на высоте 63.
        fn with_floor() -> Self {
            let mut blocks = HashMap::new();

            for x in -8..=8 {
                for z in -8..=8 {
                    blocks.insert((x, 63, z), STONE);
                }
            }

            Self { blocks }
        }

        fn set(&mut self, at: (i32, i32, i32), state: i32) -> &mut Self {
            self.blocks.insert(at, state);
            self
        }

        fn update(&self, at: (i32, i32, i32)) -> Option<i32> {
            let view = |x, y, z| self.blocks.get(&(x, y, z)).copied().unwrap_or(AIR);

            super::update(at, &view)
        }
    }

    fn water(level: i32) -> i32 {
        state_of(Kind::Water, level)
    }

    fn lava(level: i32) -> i32 {
        state_of(Kind::Lava, level)
    }

    /// Вода и лава узнаются по состоянию блока, а камень — нет.
    #[test]
    fn a_state_tells_the_fluid_and_its_level() {
        assert_eq!(fluid_at(water(SOURCE)), Some((Kind::Water, SOURCE)));
        assert_eq!(fluid_at(water(3)), Some((Kind::Water, 3)));
        assert_eq!(fluid_at(lava(SOURCE)), Some((Kind::Lava, SOURCE)));
        assert_eq!(fluid_at(STONE), None);
        assert_eq!(fluid_at(AIR), None);
    }

    /// Рядом с источником пустое место становится водой первого уровня,
    /// а дальше седьмого вода не идёт.
    #[test]
    fn water_spreads_and_thins_out() {
        let mut place = Place::with_floor();
        place.set((0, 64, 0), water(SOURCE));
        assert_eq!(place.update((1, 64, 0)), Some(water(1)));

        let mut place = Place::with_floor();
        place.set((0, 64, 0), water(6));
        assert_eq!(place.update((1, 64, 0)), Some(water(7)));

        let mut place = Place::with_floor();
        place.set((0, 64, 0), water(7));
        assert_eq!(place.update((1, 64, 0)), None);
    }

    /// В Незере лава течёт как вода: шагом один и на семь блоков.
    #[test]
    fn lava_in_the_nether_goes_further() {
        let mut place = Place::with_floor();
        place.set((0, 64, 0), lava(SOURCE));
        assert_eq!(in_world(true, || place.update((1, 64, 0))), Some(lava(1)));

        let mut place = Place::with_floor();
        place.set((0, 64, 0), lava(6));
        assert_eq!(in_world(true, || place.update((1, 64, 0))), Some(lava(7)));

        // А вне Незера правила прежние — признак не прилипает.
        let mut place = Place::with_floor();
        place.set((0, 64, 0), lava(SOURCE));
        assert_eq!(place.update((1, 64, 0)), Some(lava(2)));
        assert_eq!(Kind::Lava.delay_in(true), NETHER_LAVA_DELAY);
    }

    /// Лава растекается ближе воды и с шагом два: 2, 4, 6, а от шестого
    /// уровня дальше некуда.
    #[test]
    fn lava_does_not_go_as_far() {
        let mut place = Place::with_floor();
        place.set((0, 64, 0), lava(SOURCE));
        assert_eq!(place.update((1, 64, 0)), Some(lava(2)));

        let mut place = Place::with_floor();
        place.set((0, 64, 0), lava(4));
        assert_eq!(place.update((1, 64, 0)), Some(lava(6)));

        let mut place = Place::with_floor();
        place.set((0, 64, 0), lava(6));
        assert_eq!(place.update((1, 64, 0)), None);
    }

    /// Вода течёт туда, где ближе спуск, а в другие стороны не идёт.
    ///
    /// Это и есть правило весов: ручей к обрыву получается узким, а не
    /// растекается лужей во все стороны.
    #[test]
    fn water_flows_towards_the_nearest_drop() {
        let mut place = Place::with_floor();

        place.set((0, 64, 0), water(SOURCE));
        // Дыра в полу в двух шагах к востоку.
        place.set((2, 63, 0), AIR);

        assert_eq!(place.update((1, 64, 0)), Some(water(1)));
        assert_eq!(place.update((-1, 64, 0)), None);
        assert_eq!(place.update((0, 64, 1)), None);
    }

    /// Когда спуска нигде нет, вода растекается ровно во все стороны.
    #[test]
    fn without_a_drop_water_spreads_evenly() {
        let mut place = Place::with_floor();
        place.set((0, 64, 0), water(SOURCE));

        for (dx, dz) in SIDES {
            assert_eq!(place.update((dx, 64, dz)), Some(water(1)));
        }
    }

    /// Под водой место становится падающей водой, а на дне она снова
    /// растекается.
    #[test]
    fn water_falls_and_spreads_at_the_bottom() {
        let mut place = Place::with_floor();

        // Источник в воздухе, под ним пусто.
        place.set((0, 66, 0), water(SOURCE));
        assert_eq!(place.update((0, 65, 0)), Some(water(FALLING)));

        // На дне тоже падающая (у оригинала низ водопада — уровень 8), а
        // вбок от неё растекается первый уровень.
        place.set((0, 65, 0), water(FALLING));
        assert_eq!(place.update((0, 64, 0)), Some(water(FALLING)));

        place.set((0, 64, 0), water(FALLING));
        assert_eq!(place.update((1, 64, 0)), Some(water(1)));
    }

    /// Источник не меняется сам по себе, а текущая вода без питания высыхает.
    #[test]
    fn water_dries_up_without_a_source() {
        let mut place = Place::with_floor();
        place.set((0, 64, 0), water(SOURCE));
        assert_eq!(place.update((0, 64, 0)), None);

        let mut place = Place::with_floor();
        place.set((0, 64, 0), water(3));
        assert_eq!(place.update((0, 64, 0)), Some(AIR));
    }

    /// Между двумя источниками вода сама становится источником — это и есть
    /// бесконечная вода.
    #[test]
    fn water_between_two_sources_becomes_a_source() {
        let mut place = Place::with_floor();
        place.set((-1, 64, 0), water(SOURCE));
        place.set((1, 64, 0), water(SOURCE));

        assert_eq!(place.update((0, 64, 0)), Some(water(SOURCE)));

        // Без опоры снизу источник не появится: вода утечёт вниз.
        let mut place = Place::with_floor();
        place.set((-1, 65, 0), water(SOURCE));
        place.set((1, 65, 0), water(SOURCE));

        assert_ne!(place.update((0, 65, 0)), Some(water(SOURCE)));

        // Одного источника мало.
        let mut place = Place::with_floor();
        place.set((-1, 64, 0), water(SOURCE));

        assert_eq!(place.update((0, 64, 0)), Some(water(1)));
    }

    /// Текущая лава, которой коснулась вода, становится булыжником, а
    /// источник лавы — обсидианом.
    #[test]
    fn lava_hardens_where_water_touches_it() {
        let mut place = Place::with_floor();
        place.set((0, 64, 0), lava(2));
        place.set((1, 64, 0), water(1));
        assert_eq!(place.update((0, 64, 0)), Some(named_state("cobblestone")));

        let mut place = Place::with_floor();
        place.set((0, 64, 0), lava(SOURCE));
        place.set((1, 64, 0), water(1));
        assert_eq!(place.update((0, 64, 0)), Some(named_state("obsidian")));

        // Вода снизу лаву не трогает: по ней лава просто растекается.
        let mut place = Place::with_floor();
        place.set((0, 64, 0), lava(SOURCE));
        place.set((0, 63, 0), water(SOURCE));
        assert_eq!(place.update((0, 64, 0)), None);
    }

    /// Лава, льющаяся сверху в воду, превращает эту воду в камень.
    #[test]
    fn lava_falling_into_water_makes_stone() {
        let mut place = Place::with_floor();

        place.set((0, 65, 0), lava(FALLING));
        place.set((0, 64, 0), water(SOURCE));

        assert_eq!(place.update((0, 64, 0)), Some(named_state("stone")));
    }

    /// В твёрдый блок жидкость не течёт.
    #[test]
    fn a_solid_block_holds_the_water_back() {
        let mut place = Place::with_floor();

        place.set((0, 64, 0), water(SOURCE));
        place.set((1, 64, 0), STONE);

        assert_eq!(place.update((1, 64, 0)), None);
    }

    /// Рыхлый снег и цветок без воды рядом не трогаются: пересчёт места,
    /// где жидкости нет и прийти ей неоткуда, ничего не меняет.
    #[test]
    fn a_dry_place_stays_as_it_is() {
        let snow = blocks::states_of("powder_snow").unwrap().0;
        let poppy = blocks::state_by_name("poppy").unwrap();

        let mut place = Place::with_floor();
        place.set((1, 64, 0), snow);
        place.set((2, 64, 0), poppy);

        assert_eq!(place.update((1, 64, 0)), None);
        assert_eq!(place.update((2, 64, 0)), None);
    }

    /// Вода затекает в траву, цветы, факелы, провод и ковёр — сносит их, — а
    /// в рычаг, камень и рельс нет.
    #[test]
    fn water_washes_plants_and_torches_away() {
        for name in ["short_grass", "poppy", "torch", "redstone_wire", "sunflower", "fire", "white_carpet"] {
            let mut place = Place::with_floor();
            place.set((0, 64, 0), water(SOURCE));
            place.set((1, 64, 0), blocks::state_by_name(name).unwrap());

            assert_eq!(place.update((1, 64, 0)), Some(water(1)), "{name}");
        }

        for name in ["lever", "stone", "sugar_cane", "rail"] {
            let mut place = Place::with_floor();
            place.set((0, 64, 0), water(SOURCE));
            place.set((1, 64, 0), blocks::state_by_name(name).unwrap());

            assert_eq!(place.update((1, 64, 0)), None, "{name}");
        }
    }

    /// Вода над травой не стоит: трава — не опора, вода падает в неё.
    #[test]
    fn water_falls_into_grass_below() {
        let mut place = Place::with_floor();

        place.set((0, 66, 0), water(SOURCE));
        place.set((0, 64, 0), blocks::state_by_name("short_grass").unwrap());

        assert_eq!(place.update((0, 65, 0)), Some(water(FALLING)));
    }

    /// Вода рядом с лавой лавой не становится: питается она только водой.
    #[test]
    fn water_is_not_fed_by_lava() {
        let mut place = Place::with_floor();

        place.set((0, 64, 0), lava(SOURCE));
        place.set((2, 64, 0), water(SOURCE));
        place.set((1, 64, 0), water(1));

        assert_eq!(place.update((1, 64, 0)), None);

        // Воду питать нечем — она высыхает, но в лаву не превращается.
        let mut place = Place::with_floor();

        place.set((0, 64, 0), lava(SOURCE));
        place.set((1, 64, 0), water(3));

        assert_eq!(place.update((1, 64, 0)), Some(AIR));
    }

    /// Вода не обтекает рыхлый снег, а смывает его: место под ним считается
    /// проходимым.
    #[test]
    fn water_washes_powder_snow_away() {
        let snow = blocks::states_of("powder_snow").unwrap().0;

        assert!(is_powder_snow(snow));
        assert!(!is_powder_snow(STONE));
        assert!(can_flow_into(snow));

        let mut place = Place::with_floor();
        place.set((0, 64, 0), water(SOURCE));
        place.set((1, 64, 0), snow);

        assert_eq!(place.update((1, 64, 0)), Some(water(1)));
    }
}
