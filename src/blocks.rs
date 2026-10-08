// Таблицы «предмет — блок».
//
// Когда игрок ставит блок, клиент не сообщает, какой это блок: в сообщении
// есть только координаты, грань и точка щелчка. Что у игрока в руке, сервер
// узнаёт отдельно — из сообщения о содержимом слота, и там указан НОМЕР
// ПРЕДМЕТА, а не блока. Предмет и блок в игре — две разные таблицы, и номера
// в них не совпадают: у земли предмет 9, а блок 10, у дубовых досок — 13 и 15.
//
// Номер блока игра не сообщает серверу ни в одном пакете: клиент и сервер
// просто обязаны знать эту таблицу одинаково, потому что она вшита в саму
// игру. Взять её из документации тоже нельзя — на википедии этих чисел нет
// и никогда не было, там прямо написано, что порядок произвольный.
//
// Поэтому таблицы лежат готовым списком в blocks_table.rs, а откуда они
// взялись и на каких условиях — написано в tools/README.md. Здесь только
// поиск по ним и счёт номера состояния.

use crate::blocks_table::{
    BLOCK_NAMES, BLOCK_STATES, CANNOT_BREAK_AT_ALL, CANNOT_BREAK_IN_CREATIVE, EMPTY_BOX,
    BLOCK_IDS, CONDUCTIVE, DEFAULT_STATES, DROPS, FULL_CUBES, HARVEST_TOOLS, ITEM_NAMES, ORIENTED,
    SMALL_STACKS,
};

/// Свойство блока и его значения.
pub struct Property {
    /// Имя свойства, как в игре: axis, facing, half, type и так далее.
    pub name: &'static str,

    /// Значения по порядку — именно в этом порядке игра их и нумерует.
    pub values: &'static [&'static str],

    /// Значение, которое свойство принимает, если правило установки про него
    /// ничего не говорит.
    pub default: &'static str,
}

/// Семейство блока — правило, по которому считается состояние при установке.
///
/// Общего правила нет: ступени смотрят туда же, куда и игрок, а печь или
/// сундук — наоборот, поэтому «повернуть к игроку» не годится. Каждое
/// семейство считается по-своему, в placing.rs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Family {
    /// Бревно и всё, что ставится «столбом»: ось — по грани щелчка.
    Pillar,

    /// Ступени: поворот — по взгляду, полублок — по щелчку.
    Stairs,

    /// Дверь: занимает два блока, поворот — по взгляду.
    Door,

    /// Калитка: смотрит туда же, куда игрок.
    Gate,

    /// Кнопка и рычаг: и грань, и поворот — по щелчку.
    Face,

    /// Забор: соединяется с соседями.
    Fence,

    /// Панель из стекла или прутьев: соединяется с соседями.
    Pane,

    /// Стенка из камня: соединяется с соседями и умеет столб.
    Wall,

    /// Плита: половина — по щелчку.
    Slab,

    /// Красный провод: соединяется с соседями и несёт силу сигнала.
    Wire,

    /// Факел: на полу или на стене — по грани щелчка.
    Torch,

    /// Повторитель и сравнитель: смотрят от игрока.
    Diode,

    /// Поршень: смотрит на игрока, в том числе вверх и вниз.
    Piston,

    /// Прочие блоки со свойствами: правила установки у них нет, но свойства
    /// нужно читать и менять — лампе, нажимной плите, люку.
    Other,
}

impl Family {
    /// Соединяется ли блок с соседями по сторонам.
    ///
    /// У забора, панели и стенки свойства одинаковые — четыре стороны
    /// «да/нет», — и различаются они только тем, с чем именно срастаются.
    pub fn connects_to_sides(self) -> bool {
        matches!(self, Family::Fence | Family::Pane | Family::Wall)
    }
}

/// Блок, состояние которого зависит от того, как игрок его поставил.
pub struct Orientation {
    /// Имя блока, как в игре.
    pub name: &'static str,

    /// Номера первого и последнего состояния блока. Состояния одного блока
    /// идут подряд, поэтому по номеру видно, тот это блок или другой.
    pub min: i32,
    pub max: i32,

    /// Семейство — от него зависит правило установки.
    pub family: Family,

    /// Свойства блока по порядку.
    pub properties: &'static [Property],
}

impl Orientation {
    /// Номер состояния блока при таких значениях свойств.
    ///
    /// Номер устроен как смешанная система счисления: свойства идут по
    /// порядку, и последнее меняется быстрее всех. Значения, про которые
    /// ничего не сказано, берутся такие же, как у состояния по умолчанию.
    ///
    /// None означает, что такого свойства у блока нет: значит, правило
    /// размещения и это свойство друг с другом не сходятся.
    pub fn state(&self, values: &[(&str, &str)]) -> Option<i32> {
        let mut index = 0;
        let mut stride = 1;

        for property in self.properties.iter().rev() {
            let wanted = values
                .iter()
                .find(|(name, _)| *name == property.name)
                .map(|(_, value)| *value)
                .unwrap_or(property.default);

            let position = property.values.iter().position(|value| *value == wanted)?;

            index += position as i32 * stride;
            stride *= property.values.len() as i32;
        }

        Some(self.min + index)
    }

    /// Значение свойства у состояния с таким номером.
    ///
    /// None означает, что номер не принадлежит этому блоку или свойства
    /// с таким именем у него нет.
    pub fn value(&self, state: i32, name: &str) -> Option<&'static str> {
        if state < self.min || state > self.max {
            return None;
        }

        let mut offset = state - self.min;

        for property in self.properties.iter().rev() {
            let count = property.values.len() as i32;
            let position = (offset % count) as usize;
            offset /= count;

            if property.name == name {
                return property.values.get(position).copied();
            }
        }

        None
    }

    /// То же состояние блока, но со свойством `name`, равным `value`.
    ///
    /// Нужно там, где состояние зависит от соседей: у блока их шесть, и менять
    /// приходится по одному свойству за раз, а не собирать состояние заново.
    ///
    /// None означает, что такого свойства у блока нет либо значение ему
    /// не подходит.
    pub fn with(&self, state: i32, name: &str, value: &str) -> Option<i32> {
        if state < self.min || state > self.max {
            return None;
        }

        let mut values: Vec<(&str, &str)> = Vec::with_capacity(self.properties.len());

        for property in self.properties {
            let value = if property.name == name {
                value
            } else {
                self.value(state, property.name)?
            };

            values.push((property.name, value));
        }

        self.state(&values)
    }
}

/// Блок для предмета в руке: имя и состояние по умолчанию.
///
/// None означает, что предмет блоком не является (палка, меч, ведро) либо
/// это такой блок, которого у игрока в руках быть не может (вода, огонь).
pub fn block_for_item(item: i32) -> Option<(&'static str, i32)> {
    // Семена ставят посев, а сам урожай (пшеница) не ставится ничем
    // (вики, Wheat Seeds; farming.rs).
    if let Some(name) = item_name(item) {
        if let Some(crop) = crate::farming::planted_by(name) {
            return state_by_name(crop).and_then(|state| block_at_state(state).map(|name| (name, state)));
        }

        if name == "wheat" {
            return None;
        }
    }

    // Список отсортирован по номеру предмета, поэтому поиск двоичный.
    let index = BLOCK_STATES
        .binary_search_by_key(&item, |(item_id, _, _)| *item_id)
        .ok()?;

    let (_, state, name) = BLOCK_STATES[index];

    Some((name, state))
}

/// Предмет, из которого ставится этот блок.
///
/// Обратное к block_for_item: нужно для «выбора блока», когда игрок наводится
/// на постройку и хочет такой же предмет в руку.
///
/// None означает, что предмета для такого блока нет вовсе: голова поршня,
/// вода, огонь.
pub fn item_for_block(name: &str) -> Option<i32> {
    // Посев выбирается семенами (обратное к block_for_item).
    let seeds = match name {
        "wheat" => Some("wheat_seeds"),
        "beetroots" => Some("beetroot_seeds"),
        "carrots" => Some("carrot"),
        "potatoes" => Some("potato"),
        "melon_stem" | "attached_melon_stem" => Some("melon_seeds"),
        "pumpkin_stem" | "attached_pumpkin_stem" => Some("pumpkin_seeds"),
        "torchflower_crop" => Some("torchflower_seeds"),
        "sweet_berry_bush" => Some("sweet_berries"),
        _ => None,
    };

    if let Some(seeds) = seeds {
        return item_named(seeds);
    }

    BLOCK_STATES
        .iter()
        .find(|(_, _, block)| *block == name)
        .map(|(item, _, _)| *item)
}

/// Правило установки для блока — если оно уже выяснено.
pub fn orientation(name: &str) -> Option<&'static Orientation> {
    // Список отсортирован по имени блока, поэтому поиск двоичный.
    ORIENTED
        .binary_search_by_key(&name, |block| block.name)
        .ok()
        .map(|index| &ORIENTED[index])
}

/// Правило установки для блока с таким состоянием — если оно выяснено.
pub fn orientation_of(state: i32) -> Option<&'static Orientation> {
    block_at_state(state).and_then(orientation)
}

/// Имя блока по номеру состояния.
///
/// None означает, что такого состояния в таблице нет.
pub fn block_at_state(state: i32) -> Option<&'static str> {
    BLOCK_NAMES
        .partition_point(|(low, _, _)| *low <= state)
        .checked_sub(1)
        .and_then(|index| {
            let (low, high, name) = BLOCK_NAMES[index];
            (state >= low && state <= high).then_some(name)
        })
}

/// Номер предмета по имени.
///
/// Нужен там, где предмет называется словом, а не числом: ведро, например.
/// Держать такие номера в коде числами нельзя — они меняются от выпуска
/// к выпуску, а имена нет.
/// Номер блока по имени — не состояния, а самого блока.
pub fn block_id(name: &str) -> Option<i32> {
    BLOCK_IDS
        .binary_search_by_key(&name, |(block, _)| *block)
        .ok()
        .map(|at| BLOCK_IDS[at].1)
}

/// Все названия предметов — для подсказок.
pub fn item_names() -> impl Iterator<Item = &'static str> {
    ITEM_NAMES.iter().map(|(_, name)| *name)
}

pub fn item_named(name: &str) -> Option<i32> {
    ITEM_NAMES
        .iter()
        .find(|(_, item)| *item == name)
        .map(|(number, _)| *number)
}

/// Имя предмета по номеру.
pub fn item_name(item: i32) -> Option<&'static str> {
    ITEM_NAMES
        .binary_search_by_key(&item, |(number, _)| *number)
        .ok()
        .map(|index| ITEM_NAMES[index].1)
}

/// Состояние блока по умолчанию — то, которое и надо ставить.
///
/// Именно оно, а не начало промежутка состояний: у свойств-переключателей
/// первым идёт значение «да», и первым состоянием дёрна оказывается
/// заснеженный, хотя обычный дёрн — следующий за ним.
pub fn state_by_name(name: &str) -> Option<i32> {
    DEFAULT_STATES
        .binary_search_by_key(&name, |(block, _)| *block)
        .ok()
        .map(|index| DEFAULT_STATES[index].1)
}

/// Состояние блока из записи как в командах: `minecraft:lever[face=floor,powered=true]`.
///
/// Приставку `minecraft:` можно опускать. Свойства, о которых не сказано,
/// берутся как у состояния по умолчанию. None — блока нет, свойства ему не
/// подходят или запись искажена.
pub fn state_from_text(text: &str) -> Option<i32> {
    let text = text.trim();
    let (name, properties) = match text.split_once('[') {
        Some((name, rest)) => (name, Some(rest.strip_suffix(']')?)),
        None => (text, None),
    };
    let name = name.strip_prefix("minecraft:").unwrap_or(name);

    let Some(properties) = properties.filter(|list| !list.trim().is_empty()) else {
        return state_by_name(name);
    };

    let mut values = Vec::new();

    for pair in properties.split(',') {
        let (key, value) = pair.split_once('=')?;
        values.push((key.trim(), value.trim()));
    }

    let rule = orientation(name)?;

    if values.iter().any(|(key, _)| !rule.properties.iter().any(|p| p.name == *key)) {
        return None;
    }

    rule.state(&values)
}

/// Промежуток состояний блока с этим именем.
///
/// Нужен там, где состояние считается от начала промежутка: у воды и лавы
/// это уровень — от нуля (источник) и дальше.
pub fn states_of(name: &str) -> Option<(i32, i32)> {
    BLOCK_NAMES
        .iter()
        .find(|(_, _, block)| *block == name)
        .map(|(low, high, _)| (*low, *high))
}

/// Проводит ли блок сигнал редстоуна.
///
/// Проводят полные непрозрачные кубы: камень, земля, доски. Такой блок можно
/// запитать, и через него провод соединяется с проводом этажом выше или ниже.
/// Стекло, листва, плиты и ступени не проводят.
pub fn conductive(state: i32) -> bool {
    CONDUCTIVE
        .partition_point(|(low, _)| *low <= state)
        .checked_sub(1)
        .is_some_and(|index| {
            let (low, high) = CONDUCTIVE[index];
            state >= low && state <= high
        })
}

/// Занимает ли блок свой блок целиком: камень, земля, доски — да, плита,
/// ступени, забор, дверь — нет.
///
/// Полнота нужна там, где блок упирается в соседа: забор срастается только
/// с полным кубом, а дверь только на полный куб и ставится.
pub fn full_cube(state: i32) -> bool {
    if in_ranges(FULL_CUBES, state) {
        return true;
    }

    // Двойная плита занимает блок целиком, хотя по имени она плита: в данных
    // игры она помечена как «block», а из списка полных кубов вычтена по имени.
    orientation_of(state).is_some_and(|block| {
        block.family == Family::Slab && block.value(state, "type") == Some("double")
    })
}

/// Твёрдый ли верх у блока — можно ли поставить на него что-то стоящее.
///
/// Точно это «твёрдость верхней грани»: годится верх, накрывающий блок целиком,
/// пусть даже он и не на самом верху блока. У полного куба верх во весь блок,
/// у плиты и люка — тоже, а у ступеней верх узкий: ступенька занимает лишь
/// половину, и ставить на неё нечего. У забора, двери, панели и стенки верх
/// тоже узкий.
pub fn solid_top(state: i32) -> bool {
    if full_cube(state) {
        return true;
    }

    let Some(name) = block_at_state(state) else {
        return false;
    };

    orientation_of(state).is_some_and(|block| block.family == Family::Slab)
        || name.ends_with("_trapdoor")
}

/// Есть ли у блока ящик, с которым можно столкнуться: у травы, факела и воды
/// его нет вовсе.
pub fn has_box(state: i32) -> bool {
    !in_ranges(EMPTY_BOX, state)
}

/// Нижняя плита: занимает только нижнюю половину блока.
fn bottom_slab(state: i32) -> bool {
    orientation_of(state)
        .is_some_and(|block| block.family == Family::Slab && block.value(state, "type") == Some("bottom"))
}

/// Держит ли верх блока красную пыль, повторитель и сравнитель.
///
/// Вики (Redstone Dust, Redstone Repeater): «can be placed on conductive
/// blocks as well as glowstone, upside-down slabs, glass, upside-down stairs,
/// and hoppers». Нижняя плита сюда не входит, хоть верх у неё и ровный: он
/// на середине блока, а не наверху.
pub fn carries_dust(state: i32) -> bool {
    if bottom_slab(state) {
        return false;
    }

    if solid_top(state) {
        return true;
    }

    let Some(name) = block_at_state(state) else {
        return false;
    };

    name == "hopper"
        || orientation_of(state).is_some_and(|block| {
            block.family == Family::Stairs && block.value(state, "half") == Some("top")
        })
}

/// Упирается ли в блок лежащий предмет или падающий блок.
///
/// Всё, у чего есть ящик, — стена или пол: иначе предмет проваливался бы
/// внутрь ступеней и заборов и лежал бы в них. Исключение — совсем низкие
/// блоки (ковры, снежный слой, кувшинка, повторитель): предмет, лёгший
/// поверх их блока, висел бы в воздухе почти на целый блок, а так ложится
/// на то, что под ними, — ниже настоящего верха лишь на их толщину.
pub fn stops_items(state: i32) -> bool {
    if !has_box(state) {
        return false;
    }

    if solid_top(state) {
        return true;
    }

    !block_at_state(state).is_some_and(|name| {
        name.ends_with("_carpet")
            || matches!(name, "snow" | "lily_pad" | "repeater" | "comparator")
    })
}

/// Заменяемый ли блок: можно ли поставить блок прямо на его место.
///
/// Список — тег `minecraft:replaceable` со страницы Block tag (Java Edition):
/// воздух, вода, лава, трава, папоротники, сухие кусты и сухая трава,
/// морская трава, огонь, снежный слой, лианы, светящийся лишайник, смола,
/// невидимый свет, пустота структур, пузырьковый столб, корни и ростки Незера,
/// опавшие листья, свисающие корни. Снежный слой заменяется, только пока он
/// в один слой (вики, Snow).
pub fn replaceable(state: i32) -> bool {
    const REPLACEABLE: &[&str] = &[
        "air",
        "cave_air",
        "void_air",
        "water",
        "lava",
        "short_grass",
        "fern",
        "dead_bush",
        "bush",
        "short_dry_grass",
        "tall_dry_grass",
        "seagrass",
        "tall_seagrass",
        "fire",
        "soul_fire",
        "vine",
        "glow_lichen",
        "resin_clump",
        "light",
        "tall_grass",
        "large_fern",
        "structure_void",
        "bubble_column",
        "warped_roots",
        "nether_sprouts",
        "crimson_roots",
        "leaf_litter",
        "hanging_roots",
    ];

    if state == crate::world::AIR {
        return true;
    }

    let Some(name) = block_at_state(state) else {
        return false;
    };

    if name == "snow" {
        return orientation_of(state).and_then(|block| block.value(state, "layers")) == Some("1");
    }

    REPLACEABLE.contains(&name)
}

/// Где вторая часть блока из двух частей: двери, высокого растения, кровати.
///
/// У двери и высоких растений (подсолнух, сирень, высокая трава, большой
/// папоротник, малый капельник) половины стоят друг над другом — свойство
/// `half` со значениями `lower`/`upper`. У кровати — рядом по горизонтали:
/// изголовье (`part=head`) стоит от изножья в ту сторону, куда смотрит
/// кровать (вики, Bed: «facing — the direction the head of the bed is
/// pointing»).
///
/// Сама вторая часть здесь не проверяется: есть ли она на месте и та ли
/// это кровать — решает тот, у кого есть мир (`same_block`).
pub fn pair_partner(state: i32, (x, y, z): (i32, i32, i32)) -> Option<(i32, i32, i32)> {
    let block = orientation_of(state)?;

    match block.value(state, "half") {
        Some("lower") => return Some((x, y + 1, z)),
        Some("upper") => return Some((x, y - 1, z)),
        _ => {}
    }

    let (dx, dz) = match block.value(state, "facing")? {
        "north" => (0, -1),
        "south" => (0, 1),
        "west" => (-1, 0),
        "east" => (1, 0),
        _ => return None,
    };

    match block.value(state, "part")? {
        "foot" => Some((x + dx, y, z + dz)),
        "head" => Some((x - dx, y, z - dz)),
        _ => None,
    }
}

/// Вторая ли это часть того же блока: то же имя, и сама она указывает
/// обратно сюда. Так две двери, поставленные одна на другую, не считаются
/// одной: у верхней двери нижняя половина смотрит не туда.
pub fn is_pair(state: i32, pos: (i32, i32, i32), other_state: i32, other: (i32, i32, i32)) -> bool {
    same_block(state, other_state)
        && pair_partner(state, pos) == Some(other)
        && pair_partner(other_state, other) == Some(pos)
}

/// Один ли это блок по имени — у половинок одной двери или кровати имя общее.
pub fn same_block(a: i32, b: i32) -> bool {
    match (block_at_state(a), block_at_state(b)) {
        (Some(a), Some(b)) => a == b,
        _ => false,
    }
}

/// Попадает ли номер состояния в один из промежутков таблицы.
///
/// Промежутки отсортированы и не пересекаются, поэтому достаточно найти
/// последний, начавшийся не позже искомого номера, и проверить его конец.
fn in_ranges(table: &[(i32, i32)], state: i32) -> bool {
    table
        .partition_point(|(low, _)| *low <= state)
        .checked_sub(1)
        .is_some_and(|index| state <= table[index].1)
}

/// Ломает ли этот предмет блоки в творческом режиме.
///
/// В творческом режиме блоки ломаются мгновенно чем угодно, кроме меча,
/// трезубца, булавы и палки отладки: ими нельзя сломать ни одного блока — так
/// задумано в игре. В выживании запрета нет, мечом ломать можно (просто
/// медленно и с износом), поэтому эта проверка нужна только для творческого.
///
/// Копьё сюда не входит: им блоки не ломаются нигде, и за это отвечает
/// can_break_at_all.
pub fn can_break_in_creative(item: i32) -> bool {
    CANNOT_BREAK_IN_CREATIVE.binary_search(&item).is_err()
}

/// Ломает ли этот предмет блоки вообще, в любом режиме игры.
///
/// Кроме копья таких предметов нет: копьё — оружие для удара, а не для
/// копания, и им нельзя сломать ни один блок ни в творческом, ни в выживании.
pub fn can_break_at_all(item: i32) -> bool {
    CANNOT_BREAK_AT_ALL.binary_search(&item).is_err()
}

/// Сколько таких предметов помещается в одну стопку.
///
/// Обычно 64, но у некоторых меньше: инструменты и доспехи кладутся по
/// одному, вёдра и снежки — по шестнадцать. В таблице лежат только они.
pub fn stack_size(item: i32) -> i32 {
    SMALL_STACKS
        .iter()
        .find(|(low, high, _)| (*low..=*high).contains(&item))
        .map(|(_, _, size)| *size)
        .unwrap_or(64)
}

/// Что выпадает из сломанного блока.
///
/// None — не выпадает ничего: так со стеклом, листвой и травой. `tool` —
/// предмет, которым ломали: у части блоков без нужного инструмента не
/// выпадает ничего (камень без кирки, руда без кирки).
pub fn drop_of(state: i32, tool: Option<i32>) -> Option<i32> {
    if !tool_suits(state, tool) {
        return None;
    }

    // Номер 0 — это воздух: в таблице так записаны блоки, у которых
    // выпадение зависит от случая (сухой куст, грибные блоки, стебли).
    // Предмета «воздух» не бывает, поэтому это «ничего».
    DROPS
        .iter()
        .find(|(low, high, _)| (*low..=*high).contains(&state))
        .map(|(_, _, item)| *item)
        .filter(|item| *item != 0)
}

/// Что и сколько выпадает из блока — с долей случая.
///
/// `luck` — случайное число от 0 до 1. Остальное — как у `drop_of`.
///
/// Поверх таблицы выпадений здесь правила, которых в ней нет (по страницам
/// вики этих блоков):
/// — трава и папоротник без ножниц: семена пшеницы в одном случае из восьми;
///   с ножницами — сами они, а высокие — по две штуки низких («When broken
///   using shears, tall grass drops two units of short grass»);
/// — двойная плита: две плиты;
/// — кровать: она сама своего цвета («A bed drops itself»); вторая половина
///   убирается без выпадения, поэтому кровать одна;
/// — сухой куст без ножниц: 0–2 палки;
/// — грибные блоки: 0, 1 или 2 гриба — 77.77 %, 11.11 % и 11.11 %; ножка —
///   ничего;
/// — стебли тыквы и арбуза: 0–3 семечка, тем вероятнее, чем стебель старше
///   (таблица на странице Pumpkin Seeds — это три броска с успехом
///   (возраст + 1) / 15; у прикреплённого стебля — как у самого старого);
/// — смола: по штуке на каждую занятую грань.
///
/// `luck` одно на всё: из него и «выпало ли», и «сколько».
pub fn drops_by_luck(state: i32, tool: Option<i32>, luck: f32) -> Option<crate::inventory::Stack> {
    use crate::inventory::Stack;

    let name = block_at_state(state)?;
    let shears = tool.is_some() && tool == item_named("shears");
    let some = |item: Option<i32>, count: i32| item.filter(|_| count > 0).map(|item| Stack::new(item, count));

    match name {
        // Вики, Infested Block: без Silk Touch предмета нет.
        _ if crate::mob::arthropod::host(state).is_some() => return None,
        // Шулкер-ящик выпадает сам вместе с содержимым — предмет собирает
        // `tick::spill_containers`, а не таблица выпадений.
        _ if name.ends_with("shulker_box") => return None,
        "short_grass" | "fern" | "tall_grass" | "large_fern" => {
            if shears {
                return match name {
                    "tall_grass" => some(item_named("short_grass"), 2),
                    "large_fern" => some(item_named("fern"), 2),
                    _ => some(item_for_block(name), 1),
                };
            }

            return some(item_named("wheat_seeds"), (luck < 0.125) as i32);
        }
        "dead_bush" => {
            if shears {
                return some(item_for_block(name), 1);
            }

            // Как делятся 0, 1 и 2, вики не говорит — поровну (сверить
            // чёрным ящиком).
            return some(item_named("stick"), ((luck * 3.0) as i32).min(2));
        }
        "brown_mushroom_block" | "red_mushroom_block" => {
            let mushroom = if name.starts_with("brown") { "brown_mushroom" } else { "red_mushroom" };
            let count = if luck < 7.0 / 9.0 {
                0
            } else if luck < 8.0 / 9.0 {
                1
            } else {
                2
            };

            return some(item_named(mushroom), count);
        }
        "mushroom_stem" => return None,
        "pumpkin_stem" | "melon_stem" | "attached_pumpkin_stem" | "attached_melon_stem" => {
            let seeds = if name.contains("pumpkin") { "pumpkin_seeds" } else { "melon_seeds" };
            let age = orientation(name)
                .and_then(|block| block.value(state, "age"))
                .and_then(|age| age.parse::<i32>().ok())
                .unwrap_or(7);

            return some(item_named(seeds), three_throws(luck, (age + 1) as f32 / 15.0));
        }
        "resin_clump" => {
            let faces = orientation(name).map_or(0, |block| {
                ["down", "up", "north", "south", "west", "east"]
                    .iter()
                    .filter(|face| block.value(state, face) == Some("true"))
                    .count() as i32
            });

            return some(item_for_block(name), faces);
        }
        _ => {}
    }

    if name.ends_with("_bed") {
        return some(item_for_block(name), 1);
    }

    let item = drop_of(state, tool)?;

    let double_slab = orientation_of(state)
        .is_some_and(|block| block.family == Family::Slab && block.value(state, "type") == Some("double"));

    Some(Stack::new(item, if double_slab { 2 } else { 1 }))
}

/// Сколько успехов из трёх бросков с вероятностью `chance` — по одному
/// случайному числу: отрезок от 0 до 1 делится по вероятностям исходов.
fn three_throws(luck: f32, chance: f32) -> i32 {
    let miss = 1.0 - chance;
    let odds = [
        miss * miss * miss,
        3.0 * chance * miss * miss,
        3.0 * chance * chance * miss,
    ];

    let mut edge = 0.0;

    for (count, odd) in odds.iter().enumerate() {
        edge += odd;

        if luck < edge {
            return count as i32;
        }
    }

    3
}

/// Годится ли этот предмет, чтобы из блока что-то выпало.
///
/// Блока в таблице нет — значит, годится что угодно, хоть рука.
fn tool_suits(state: i32, tool: Option<i32>) -> bool {
    let Some((_, _, suitable)) = HARVEST_TOOLS
        .iter()
        .find(|(low, high, _)| (*low..=*high).contains(&state))
    else {
        return true;
    };

    tool.is_some_and(|tool| suitable.contains(&tool))
}

/// Стойкость блока к взрыву — число из minecraft-data (таблица
/// `hardness_table::RESISTANCE`, вики, Explosion: «Blast resistance»).
/// Блок с водой внутри держит не слабее самой воды: луч проверяет блок
/// в точке, а вода в нём — тоже блок на пути (вики, Explosion: «Sufficiently
/// weak explosions happening on … any height of water or lava don't do block
/// damage»). [проверить] залитые водой ступени и плиты чёрным ящиком.
pub fn blast_resistance(state: i32) -> f32 {
    use std::cmp::Ordering;

    let table = crate::hardness_table::RESISTANCE;
    let own = table
        .binary_search_by(|(low, high, _)| {
            if *high < state {
                Ordering::Less
            } else if *low > state {
                Ordering::Greater
            } else {
                Ordering::Equal
            }
        })
        .map_or(0.0, |index| table[index].2);

    let waterlogged = orientation_of(state).is_some_and(|block| block.value(state, "waterlogged") == Some("true"));

    if waterlogged {
        let water = state_by_name("water").map_or(own, blast_resistance);
        return own.max(water);
    }

    own
}

/// Что выпадает из блока, который сломал сам мир — ход поршня, поток воды,
/// отвал без опоры. Инструмента нет, но паутина при этом всё же даёт нитку:
/// «A cobweb drops one piece of string if broken with a non-Silk Touch sword,
/// if water touches or flows over it, or a piston pushes it» (вики, Cobweb;
/// сценарий чёрного ящика piston-breaks). Рукой — ничего, как и прежде.
pub fn world_drops(state: i32, luck: f32) -> Option<crate::inventory::Stack> {
    if block_at_state(state) == Some("cobweb") {
        return item_named("string").map(|string| crate::inventory::Stack::new(string, 1));
    }

    drops_by_luck(state, None, luck)
}

/// Что выпадает из блока, разрушенного динамитом: «as if mined by a player
/// using a correct, unenchanted tool (or an empty hand if the correct tool
/// would be shears)» (вики, TNT). Инструмент — первый годный из таблицы,
/// кроме ножниц.
pub fn explosion_drops(state: i32, luck: f32) -> Option<crate::inventory::Stack> {
    let shears = item_named("shears");
    let tool = HARVEST_TOOLS
        .iter()
        .find(|(low, high, _)| (*low..=*high).contains(&state))
        .and_then(|(_, _, suitable)| suitable.iter().copied().find(|tool| Some(*tool) != shears));

    drops_by_luck(state, tool, luck)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Состояние, которое игра ставит для этого предмета по умолчанию —
    /// то, что сервер ставит, когда правила поворота ему неизвестны.
    fn default_state(item: i32) -> Option<i32> {
        block_for_item(item).map(|(_, state)| state)
    }

    /// Числа, которые уже проверены глазами в игре: камень сервер ставит
    /// именно этим состоянием, и клиент показывает камень.
    #[test]
    fn known_blocks_have_expected_states() {
        assert_eq!(default_state(1), Some(1)); // камень
        assert_eq!(default_state(28), Some(10)); // земля
        assert_eq!(default_state(36), Some(15)); // дубовые доски
    }

    /// Предметы, которые блоками не являются, ставить некуда.
    #[test]
    fn non_block_items_have_no_state() {
        assert_eq!(default_state(-1), None);
        assert_eq!(default_state(i32::MAX), None);
    }

    /// Номер предмета и номер блока совпадают далеко не всегда — в этом и была
    /// причина, по которой блоки ставились камнем.
    #[test]
    fn item_and_block_numbers_differ() {
        let dirt_item = 28;
        assert_ne!(default_state(dirt_item), Some(dirt_item));
    }

    /// Мечом, трезубцем, булавой и палкой отладки в творческом режиме блоки
    /// не ломаются, а всем остальным — ломаются.
    #[test]
    fn swords_do_not_break_blocks_in_creative() {
        assert!(!can_break_in_creative(912)); // деревянный меч
        assert!(!can_break_in_creative(942)); // незеритовый меч
        assert!(!can_break_in_creative(1332)); // трезубец
        assert!(!can_break_in_creative(1224)); // булава
        assert!(!can_break_in_creative(1309)); // палка отладки

        // Пустая рука (номер -1), камень и палка блоки ломают.
        assert!(can_break_in_creative(-1));
        assert!(can_break_in_creative(1)); // камень
        assert!(can_break_in_creative(947)); // палка
    }

    /// Копьём блоки не ломаются ни в каком режиме игры — ни в творческом,
    /// ни в выживании. Это единственный такой предмет.
    #[test]
    fn spears_break_nothing_anywhere() {
        for spear in [1297, 1298, 1299, 1300, 1301, 1302, 1303] {
            assert!(!can_break_at_all(spear), "копьё {spear}");
        }

        // Меч и трезубец ломают блоки в выживании, поэтому под общий запрет
        // они не попадают — у них запрет только для творческого.
        assert!(can_break_at_all(912)); // деревянный меч
        assert!(can_break_at_all(1332)); // трезубец
        assert!(can_break_at_all(-1)); // пустая рука
        assert!(can_break_at_all(1)); // камень
    }

    /// Свойства читаются обратно из номера состояния: у блока по умолчанию
    /// они такие, какими игра его и ставит.
    #[test]
    fn default_states_have_expected_properties() {
        let log = orientation("oak_log").expect("правило для бревна");
        assert_eq!(log.value(137, "axis"), Some("y"));

        let stairs = orientation("oak_stairs").expect("правило для ступеней");
        assert_eq!(stairs.value(3918, "facing"), Some("north"));
        assert_eq!(stairs.value(3918, "half"), Some("bottom"));

        let slab = orientation("oak_slab").expect("правило для плиты");
        assert_eq!(slab.value(13333, "type"), Some("bottom"));

        let gate = orientation("oak_fence_gate").expect("правило для калитки");
        assert_eq!(gate.value(8653, "facing"), Some("north"));
        assert_eq!(gate.value(8653, "open"), Some("false"));
        assert_eq!(gate.value(8653, "in_wall"), Some("false"));
    }

    /// Свойства складываются в тот же номер состояния, из которого прочитаны:
    /// счёт и чтение должны сходиться.
    #[test]
    fn state_and_value_agree() {
        for name in ["oak_log", "oak_stairs", "oak_slab", "oak_fence_gate"] {
            let block = orientation(name).expect("правило установки");

            for state in block.min..=block.max {
                let values: Vec<(&str, &str)> = block
                    .properties
                    .iter()
                    .map(|property| {
                        (property.name, block.value(state, property.name).expect("значение"))
                    })
                    .collect();

                assert_eq!(block.state(&values), Some(state), "блок {name}, состояние {state}");
            }
        }
    }

    /// Номер состояния чужого блока свойством этого блока не считается.
    #[test]
    fn foreign_state_has_no_values() {
        let log = orientation("oak_log").expect("правило для бревна");

        assert_eq!(log.value(3918, "axis"), None);
        assert_eq!(log.value(135, "axis"), None);
    }

    /// Про блок, правило для которого ещё не выяснено, таблица молчит —
    /// и сервер ставит его состоянием по умолчанию.
    #[test]
    fn unknown_blocks_have_no_orientation() {
        assert!(orientation("stone").is_none());
        assert!(orientation("oak_planks").is_none());
    }

    /// Паутина, сломанная поршнем или водой, даёт нитку, а рукой — ничего.
    #[test]
    fn a_cobweb_broken_by_the_world_drops_string() {
        let cobweb = state_of("cobweb");

        assert!(drops_by_luck(cobweb, None, 0.5).is_none());

        let stack = world_drops(cobweb, 0.5).expect("из паутины выпадает нитка");

        assert_eq!(Some(stack.item), item_named("string"));
        assert_eq!(world_drops(state_of("dirt"), 0.5).map(|stack| stack.item), Some(item_of("dirt")));
    }

    /// Из сломанного блока выпадает то, что положено: из камня булыжник,
    /// из земли земля, а из стекла — ничего.
    #[test]
    fn a_broken_block_drops_what_it_should() {
        let stone = state_of("stone");
        let cobblestone = drop_of(stone, Some(pickaxe())).expect("из камня что-то выпадает");

        assert_ne!(cobblestone, item_of("stone"));
        assert_eq!(block_for_item(cobblestone).map(|(name, _)| name), Some("cobblestone"));

        let dirt = state_of("dirt");
        assert_eq!(drop_of(dirt, None), Some(item_of("dirt")));

        assert_eq!(drop_of(state_of("glass"), Some(pickaxe())), None);
    }

    /// Без подходящего инструмента из блока ничего не выпадает: камень рукой
    /// ломается, но булыжника не даёт.
    #[test]
    fn the_wrong_tool_drops_nothing() {
        let stone = state_of("stone");

        assert_eq!(drop_of(stone, None), None);
        assert_eq!(drop_of(stone, Some(item_of("dirt"))), None);
        assert!(drop_of(stone, Some(pickaxe())).is_some());

        // Земле инструмент не нужен вовсе.
        assert!(drop_of(state_of("dirt"), None).is_some());
    }

    /// Состояние блока по имени — для проверок.
    fn state_of(name: &str) -> i32 {
        BLOCK_STATES
            .iter()
            .find(|(_, _, block)| *block == name)
            .map(|(_, state, _)| *state)
            .expect("такой блок есть")
    }

    /// Номер предмета по имени блока.
    fn item_of(name: &str) -> i32 {
        BLOCK_STATES
            .iter()
            .find(|(_, _, block)| *block == name)
            .map(|(item, _, _)| *item)
            .expect("такой предмет есть")
    }

    /// Какая-нибудь кирка: та, которой ломается камень.
    fn pickaxe() -> i32 {
        HARVEST_TOOLS
            .iter()
            .find(|(low, _, _)| *low == state_of("stone"))
            .map(|(_, _, tools)| tools[0])
            .expect("у камня указан инструмент")
    }

    /// Ставится состояние по умолчанию, а не первое из промежутка: первый
    /// дёрн — заснеженный, а нужен обычный.
    #[test]
    fn the_default_state_is_not_the_first_one() {
        let (first, _) = states_of("grass_block").expect("дёрн есть в таблице");
        let usual = state_by_name("grass_block").expect("у дёрна есть состояние по умолчанию");

        assert_ne!(usual, first, "взялся заснеженный дёрн");
        assert_eq!(usual, first + 1);

        // У блоков без свойств разницы нет.
        assert_eq!(state_by_name("bedrock"), states_of("bedrock").map(|(low, _)| low));

        assert_eq!(state_by_name("такого-блока-нет"), None);
    }

    /// Сигнал проводят полные непрозрачные кубы, а стекло и провод — нет.
    #[test]
    fn conductivity_follows_opacity() {
        let state = |name: &str| state_by_name(name).unwrap();

        assert!(conductive(state("stone")));
        assert!(conductive(state("dirt")));
        assert!(conductive(state("redstone_lamp")));

        assert!(!conductive(state("glass")));
        assert!(!conductive(state("oak_leaves")));
        assert!(!conductive(state("redstone_wire")));
        assert!(!conductive(state("repeater")));
        assert!(!conductive(state("oak_slab")));
        assert!(!conductive(crate::world::AIR));
    }

    /// Красная пыль в руке называется не так, как блок, который она ставит.
    #[test]
    fn redstone_dust_places_a_wire() {
        let dust = item_named("redstone").expect("пыль есть в таблице");

        assert_eq!(block_for_item(dust).map(|(name, _)| name), Some("redstone_wire"));
    }

    /// Редстоуновые блоки попали в таблицу свойств с нужными семействами.
    #[test]
    fn redstone_blocks_have_families() {
        let family = |name: &str| orientation(name).map(|block| block.family);

        assert_eq!(family("redstone_wire"), Some(Family::Wire));
        assert_eq!(family("redstone_torch"), Some(Family::Torch));
        assert_eq!(family("redstone_wall_torch"), Some(Family::Torch));
        assert_eq!(family("repeater"), Some(Family::Diode));
        assert_eq!(family("comparator"), Some(Family::Diode));
        assert_eq!(family("lever"), Some(Family::Face));
        assert_eq!(family("redstone_lamp"), Some(Family::Other));
        assert_eq!(family("stone_pressure_plate"), Some(Family::Other));

        // И свойства читаются: у повторителя четыре задержки.
        let repeater = orientation("repeater").unwrap();
        let state = repeater.state(&[("delay", "3"), ("facing", "east")]).unwrap();
        assert_eq!(repeater.value(state, "delay"), Some("3"));
        assert_eq!(repeater.value(state, "powered"), Some("false"));
    }

    /// Предмет для блока находится и совпадает с тем, из чего блок ставится.
    #[test]
    fn a_block_knows_its_item() {
        let stone_item = item_for_block("stone").expect("камень есть в таблице");

        assert_eq!(block_for_item(stone_item).map(|(name, _)| name), Some("stone"));

        // У пыли предмет называется иначе, и это тоже работает.
        assert_eq!(item_for_block("redstone_wire"), item_named("redstone"));

        // У головы поршня своего предмета нет.
        assert_eq!(item_for_block("piston_head"), None);
    }

    /// Проводимость берётся из списков вики, а не из внешнего вида блока.
    ///
    /// Страница Conductivity перечисляет исключения поимённо: полные кубы,
    /// которые сигнал не проводят, и наоборот.
    #[test]
    fn conductivity_follows_the_lists_not_the_looks() {
        let state = |name: &str| state_by_name(name).unwrap_or_else(|| panic!("нет блока {name}"));

        // Полные кубы, которые сигнал не проводят.
        for name in [
            "redstone_block",
            "observer",
            "piston",
            "sticky_piston",
            "glowstone",
            "tnt",
            "dirt_path",
            "farmland",
            "enchanting_table",
            "honey_block",
            "oak_leaves",
        ] {
            assert!(!conductive(state(name)), "{name} не должен проводить");
        }

        // И наоборот — проводят, хотя полными кубами не выглядят.
        for name in ["slime_block", "mangrove_roots", "barrier", "soul_sand", "target"] {
            assert!(conductive(state(name)), "{name} должен проводить");
        }

        // Обычные блоки как были.
        for name in ["stone", "dirt", "redstone_lamp"] {
            assert!(conductive(state(name)), "{name} должен проводить");
        }

        for name in ["glass", "oak_slab", "oak_stairs", "hopper"] {
            assert!(!conductive(state(name)), "{name} не должен проводить");
        }

        // Двойная плита — уже полный блок, и проводит.
        let double = orientation("oak_slab")
            .and_then(|slab| slab.with(state("oak_slab"), "type", "double"))
            .expect("у плиты есть двойной вид");

        assert!(conductive(double), "двойная плита должна проводить");
    }


    #[test]
    fn state_from_text_reads_properties() {
        assert_eq!(state_from_text("minecraft:stone"), state_by_name("stone"));
        assert_eq!(state_from_text("stone[]"), state_by_name("stone"));

        let lever = state_from_text("minecraft:lever[face=floor,facing=north,powered=true]")
            .expect("рычаг разбирается");
        let rule = orientation("lever").expect("у рычага есть правило");
        assert_eq!(rule.value(lever, "powered"), Some("true"));
        assert_eq!(rule.value(lever, "face"), Some("floor"));

        assert_eq!(state_from_text("lever[nonsense=1]"), None);
        assert_eq!(state_from_text("lever[face=floor"), None);
        assert_eq!(state_from_text("no_such_block"), None);
    }

    /// Только сам предмет — для проверок, где число не важно.
    fn drop_by_luck(state: i32, tool: Option<i32>, luck: f32) -> Option<i32> {
        drops_by_luck(state, tool, luck).map(|stack| stack.item)
    }

    #[test]
    fn grass_gives_seeds_by_luck_and_itself_to_shears() {
        let grass = state_by_name("short_grass").expect("трава есть");
        let seeds = item_named("wheat_seeds").expect("семена есть");
        let shears = item_named("shears").expect("ножницы есть");

        assert_eq!(drop_by_luck(grass, None, 0.05), Some(seeds));
        assert_eq!(drop_by_luck(grass, None, 0.5), None);
        // С ножницами — сама трава, а не семена.
        assert_eq!(drop_by_luck(grass, Some(shears), 0.5), item_named("short_grass"));

        // Высокая трава с ножницами — две низких.
        let tall = state_by_name("tall_grass").expect("высокая трава есть");
        assert_eq!(
            drops_by_luck(tall, Some(shears), 0.5),
            Some(crate::inventory::Stack::new(item_named("short_grass").unwrap(), 2))
        );

        let stone = state_by_name("stone").expect("камень есть");
        assert_eq!(drop_by_luck(stone, None, 0.5), drop_of(stone, None));
    }

    /// Двойная плита — две плиты, кровать — кровать своего цвета, а номера
    /// «воздух» как предмета не выпадает ни из чего.
    #[test]
    fn special_drops_follow_the_wiki() {
        use crate::inventory::Stack;

        let slab = orientation("oak_slab").unwrap();
        let double = slab.state(&[("type", "double")]).unwrap();
        let oak_slab = item_named("oak_slab").unwrap();

        assert_eq!(drops_by_luck(double, None, 0.5), Some(Stack::new(oak_slab, 2)));
        assert_eq!(drops_by_luck(state_by_name("oak_slab").unwrap(), None, 0.5), Some(Stack::new(oak_slab, 1)));

        // Кровать — с любой половины.
        let bed = orientation("red_bed").unwrap();
        for part in ["head", "foot"] {
            let state = bed.state(&[("part", part)]).unwrap();
            assert_eq!(drops_by_luck(state, None, 0.5), Some(Stack::new(item_named("red_bed").unwrap(), 1)));
        }

        // Ни у одного состояния выпадение не «воздух».
        for (low, high, _) in BLOCK_NAMES {
            for state in *low..=*high {
                for luck in [0.0, 0.5, 0.99] {
                    if let Some(stack) = drops_by_luck(state, None, luck) {
                        assert!(stack.item != 0 && stack.count > 0, "состояние {state}: {stack:?}");
                    }
                }
            }
        }

        // Сухой куст без ножниц: палки, 0–2.
        let bush = state_by_name("dead_bush").unwrap();
        let stick = item_named("stick").unwrap();
        assert_eq!(drops_by_luck(bush, None, 0.1), None);
        assert_eq!(drops_by_luck(bush, None, 0.5), Some(Stack::new(stick, 1)));
        assert_eq!(drops_by_luck(bush, None, 0.9), Some(Stack::new(stick, 2)));

        // Грибной блок: чаще ничего, иногда один-два гриба.
        let cap = state_by_name("red_mushroom_block").unwrap();
        let mushroom = item_named("red_mushroom").unwrap();
        assert_eq!(drops_by_luck(cap, None, 0.5), None);
        assert_eq!(drops_by_luck(cap, None, 0.8), Some(Stack::new(mushroom, 1)));
        assert_eq!(drops_by_luck(cap, None, 0.95), Some(Stack::new(mushroom, 2)));

        // Молодой стебель почти всегда пуст, а три семечка — только при
        // самом большом везении.
        let stem = state_by_name("pumpkin_stem").unwrap();
        assert_eq!(drops_by_luck(stem, None, 0.5), None);
        assert_eq!(drops_by_luck(stem, None, 0.9999).map(|stack| stack.count), Some(3));
    }

    /// Вероятности трёх бросков сходятся с таблицей вики (Pumpkin Seeds):
    /// у стебля возраста 0 ничего не выпадает в 81.3 % случаев.
    #[test]
    fn three_throws_match_the_wiki_table() {
        assert_eq!(three_throws(0.812, 1.0 / 15.0), 0);
        assert_eq!(three_throws(0.814, 1.0 / 15.0), 1);
        // Возраст 7: 10.16 % — ничего, 34.84 % — одно.
        assert_eq!(three_throws(0.10, 8.0 / 15.0), 0);
        assert_eq!(three_throws(0.44, 8.0 / 15.0), 1);
        assert_eq!(three_throws(0.46, 8.0 / 15.0), 2);
    }

    /// Заменяемое — по тегу вики: трава да, цветок и камень нет, снег —
    /// только в один слой.
    #[test]
    fn replaceable_follows_the_tag() {
        let snow = orientation("snow").unwrap();

        assert!(replaceable(crate::world::AIR));
        assert!(replaceable(state_by_name("short_grass").unwrap()));
        assert!(replaceable(state_by_name("tall_grass").unwrap()));
        assert!(replaceable(state_by_name("water").unwrap()));
        assert!(replaceable(snow.state(&[("layers", "1")]).unwrap()));
        assert!(!replaceable(snow.state(&[("layers", "2")]).unwrap()));
        assert!(!replaceable(state_by_name("poppy").unwrap()));
        assert!(!replaceable(state_by_name("stone").unwrap()));
    }

    /// Пыль держится на верхней плите и перевёрнутых ступенях, а на нижней
    /// плите — нет.
    #[test]
    fn dust_needs_a_top_that_is_really_on_top() {
        let slab = orientation("oak_slab").unwrap();
        let stairs = orientation("oak_stairs").unwrap();

        assert!(carries_dust(state_by_name("stone").unwrap()));
        assert!(carries_dust(slab.state(&[("type", "top")]).unwrap()));
        assert!(!carries_dust(slab.state(&[("type", "bottom")]).unwrap()));
        assert!(carries_dust(stairs.state(&[("half", "top")]).unwrap()));
        assert!(!carries_dust(stairs.state(&[("half", "bottom")]).unwrap()));
        assert!(carries_dust(state_by_name("hopper").unwrap()));
    }

    /// Вторая часть кровати — в ту сторону, куда она смотрит.
    #[test]
    fn a_bed_head_is_where_it_faces() {
        let bed = orientation("red_bed").unwrap();
        let foot = bed.state(&[("facing", "east"), ("part", "foot")]).unwrap();
        let head = bed.state(&[("facing", "east"), ("part", "head")]).unwrap();

        assert_eq!(pair_partner(foot, (0, 64, 0)), Some((1, 64, 0)));
        assert_eq!(pair_partner(head, (1, 64, 0)), Some((0, 64, 0)));
        assert_eq!(pair_partner(state_by_name("stone").unwrap(), (0, 64, 0)), None);

        // Ступени — не пара, хоть и с «half».
        let stairs = state_by_name("oak_stairs").unwrap();
        assert_eq!(pair_partner(stairs, (0, 64, 0)), None);
    }
}
