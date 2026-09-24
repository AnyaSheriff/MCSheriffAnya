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

use std::collections::HashSet;
use std::sync::OnceLock;

use crate::blocks;
use crate::world::AIR;

/// Уровень источника.
pub const SOURCE: i32 = 0;

/// Уровень падающей жидкости — той, что льётся сверху.
pub const FALLING: i32 = 8;

/// Как далеко растекается вода по ровному месту.
const WATER_REACH: i32 = 7;

/// Как далеко растекается лава — заметно ближе воды.
const LAVA_REACH: i32 = 3;

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

    /// Докуда растекается.
    fn reach(self) -> i32 {
        match self {
            Kind::Water => WATER_REACH,
            Kind::Lava => LAVA_REACH,
        }
    }

    /// За сколько шагов ищет спуск, выбирая сторону.
    fn search(self) -> i32 {
        match self {
            Kind::Water => WATER_SEARCH,
            Kind::Lava => LAVA_SEARCH,
        }
    }

    /// Через сколько тактов делается следующий шаг.
    pub fn delay(self) -> u64 {
        match self {
            Kind::Water => WATER_DELAY,
            Kind::Lava => LAVA_DELAY,
        }
    }
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

/// Место, куда жидкость может затечь: пустое, занятое ею же или занятое тем,
/// что она уничтожает.
///
/// Рыхлый снег вода и лава не обтекают, а ломают, — поэтому он считается
/// проходимым. Трава и прочее, что сминается потоком, сюда пока не входит:
/// этих блоков у нас ещё нет.
pub fn can_flow_into(state: i32) -> bool {
    state == AIR || is_powder_snow(state) || fluid_at(state).is_some()
}

/// Может ли это место принять жидкость сверху.
///
/// Пустое — может. Занятое текущей жидкостью — тоже: она не заполняет блок
/// целиком, и сверху в неё продолжает литься. А источник полон, и принимать
/// ему нечего.
fn drains_into(state: i32) -> bool {
    match fluid_at(state) {
        Some((_, SOURCE)) => false,
        Some(_) => true,
        None => state == AIR || is_powder_snow(state),
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
    if !can_flow_into(here) {
        return None;
    }

    let supply = supply(at, block);

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

    if let Some((kind, _)) = fluid_at(block(x, y + 1, z)) {
        // Падающей жидкость остаётся, пока под ней есть куда падать. Дальше
        // она растекается вбок обычным первым уровнем.
        let level = if drains_into(block(x, y - 1, z)) {
            FALLING
        } else {
            1
        };

        return Some((kind, level));
    }

    let mut best: Option<(Kind, i32)> = None;

    for (side, (dx, dz)) in SIDES.iter().enumerate() {
        let neighbour = (x + dx, y, z + dz);

        let Some((kind, level)) = fluid_at(block(neighbour.0, neighbour.1, neighbour.2)) else {
            continue;
        };

        // Падающая жидкость вбок не растекается: она уходит вниз.
        if level >= FALLING {
            continue;
        }

        let next = level + 1;

        if next > kind.reach() {
            continue;
        }

        // Течёт ли сосед в нашу сторону: он выбирает направление по тому,
        // где ближе спуск.
        if !flows_towards(neighbour, opposite(side), kind, block) {
            continue;
        }

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

    if !can_flow_into(block(x + dx, y, z + dz)) {
        return false;
    }

    // Источник разливается вбок даже над дырой, а текущая жидкость — нет:
    // она целиком уходит вниз.
    let is_source = matches!(fluid_at(block(x, y, z)), Some((_, SOURCE)));

    if !is_source && drains_into(block(x, y - 1, z)) {
        return false;
    }

    let weights: Vec<Option<i32>> = (0..SIDES.len())
        .map(|side| weight(from, side, kind, block))
        .collect();

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

    if !can_flow_into(block(start.0, y, start.1)) {
        return None;
    }

    // Обход в ширину: сперва места в одном шаге, потом в двух и так далее.
    // Как только нашли место, из-под которого можно спуститься, это и есть
    // ответ — дальше искать незачем.
    let mut seen: HashSet<(i32, i32)> = HashSet::from([(x, z), start]);
    let mut edge = vec![start];

    for distance in 0..=kind.search() {
        let mut next = Vec::new();

        for (x, z) in edge {
            if drains_into(block(x, y - 1, z)) {
                return Some(distance);
            }

            for (dx, dz) in SIDES {
                let step = (x + dx, z + dz);

                if seen.contains(&step) || !can_flow_into(block(step.0, y, step.1)) {
                    continue;
                }

                seen.insert(step);
                next.push(step);
            }
        }

        edge = next;
    }

    None
}

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
    !can_flow_into(below) || matches!(fluid_at(below), Some((Kind::Water, SOURCE)))
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

    /// Лава растекается ближе воды: от третьего уровня дальше некуда.
    #[test]
    fn lava_does_not_go_as_far() {
        let mut place = Place::with_floor();
        place.set((0, 64, 0), lava(2));
        assert_eq!(place.update((1, 64, 0)), Some(lava(3)));

        let mut place = Place::with_floor();
        place.set((0, 64, 0), lava(3));
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

        // На дне: под ногами пол — вода растекается дальше.
        place.set((0, 65, 0), water(FALLING));
        assert_eq!(place.update((0, 64, 0)), Some(water(1)));
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
