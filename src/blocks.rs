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

    DROPS
        .iter()
        .find(|(low, high, _)| (*low..=*high).contains(&state))
        .map(|(_, _, item)| *item)
}

/// Что выпадает из блока, когда в деле есть доля случая.
///
/// `luck` — случайное число от 0 до 1. Трава и папоротник без ножниц дают
/// семена пшеницы в одном случае из восьми (вики: «Short Grass», «Fern»),
/// с ножницами — себя. Остальное — как у `drop_of`.
pub fn drop_by_luck(state: i32, tool: Option<i32>, luck: f32) -> Option<i32> {
    const GRASSY: [&str; 4] = ["short_grass", "fern", "tall_grass", "large_fern"];

    if block_at_state(state).is_some_and(|name| GRASSY.contains(&name)) {
        let shears = item_named("shears");

        if tool.is_some() && tool == shears {
            return drop_of(state, tool);
        }

        return (luck < 0.125).then(|| item_named("wheat_seeds")).flatten();
    }

    drop_of(state, tool)
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

    #[test]
    fn grass_gives_seeds_by_luck_and_itself_to_shears() {
        let grass = state_by_name("short_grass").expect("трава есть");
        let seeds = item_named("wheat_seeds").expect("семена есть");
        let shears = item_named("shears").expect("ножницы есть");

        assert_eq!(drop_by_luck(grass, None, 0.05), Some(seeds));
        assert_eq!(drop_by_luck(grass, None, 0.5), None);
        assert_eq!(drop_by_luck(grass, Some(shears), 0.5), drop_of(grass, Some(shears)));

        let stone = state_by_name("stone").expect("камень есть");
        assert_eq!(drop_by_luck(stone, None, 0.5), drop_of(stone, None));
    }
}
