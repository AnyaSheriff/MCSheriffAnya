// Правила установки блоков.
//
// Клиент не сообщает, каким состоянием ставить блок: в сообщении о щелчке есть
// только координаты блока, грань и точка щелчка, а что за блок — известно лишь
// по предмету в руке. Каким боком его повернуть, сервер считает сам.
//
// Правила у разных блоков разные, и общего среди них нет: ступени смотрят туда
// же, куда и игрок, а печь или сундук — наоборот, поэтому «повернуть к игроку»
// не годится. Что удалось выяснить, собрано здесь; для блоков, про которые
// правило ещё не выяснено, состояние берётся таким, каким его задала игра
// по умолчанию.

use crate::blocks::{self, Family, Orientation};
use crate::fluids;
use crate::log_debug;
use crate::world::{self, World};

/// Чем кончилась попытка поставить блок.
#[derive(Debug, PartialEq)]
pub enum Placement {
    /// Блок поставлен — предмет расходуется.
    Placed,
    /// Плита слилась с плитой в двойную.
    Merged,
    /// Ставить нечего или некуда, мир не менялся.
    Nothing,
    /// Отказ: клиенту надо напомнить, что на самом деле в этих местах.
    Refused(Vec<(i32, i32, i32)>),
}

/// Ставит блок `name` по щелчку `click` по блоку `clicked` — общее для
/// игроков Java и Bedrock. `player` — где стоит игрок (ноги): в него блок
/// не ставится.
#[cfg(test)]
pub fn place(
    world: &mut World,
    name: &'static str,
    default_state: i32,
    clicked: (i32, i32, i32),
    click: &Click,
    player: (f64, f64, f64),
) -> Placement {
    place_tagged(world, name, default_state, clicked, click, player, None)
}

/// То же, но со стопкой, у которой есть пометка: особый динамит
/// (custom_tnt.rs) запоминает свой вид в чанке — до разбора редстоуна,
/// чтобы динамит, поставленный у сигнала, загорелся уже своим видом.
pub fn place_tagged(
    world: &mut World,
    name: &'static str,
    default_state: i32,
    clicked: (i32, i32, i32),
    click: &Click,
    player: (f64, f64, f64),
    custom: Option<crate::custom_tnt::KindId>,
) -> Placement {
    let (clicked_x, clicked_y, clicked_z) = clicked;
    let (px, py, pz) = player;

    // Факел на стене — другой блок, настенный; на потолок факел не вешается.
    let Some((name, default_state)) = block_for_face(name, default_state, click.face) else {
        log_debug!("Постановка: {} на эту грань не ставится", name);
        return Placement::Nothing;
    };

    let orientation = blocks::orientation(name);

    // Плита, поставленная на плиту того же вида, слипается с ней в двойную:
    // занимает место того же блока, а не соседнего.
    if let Some(orientation) = orientation
        && let Some(double) = merge_into_double(orientation, world.get_block(clicked_x, clicked_y, clicked_z), click)
    {
        // Двойная плита занимает блок целиком: если в нижней или верхней
        // половине стоит игрок, он оказался бы замурован.
        if blocks_player(clicked_x, clicked_y, clicked_z, double, px, py, pz) {
            log_debug!("Постановка: двойная плита в {} {} {} задела бы игрока — не ставим", clicked_x, clicked_y, clicked_z);
            return Placement::Refused(vec![(clicked_x, clicked_y, clicked_z)]);
        }

        world.set_block(clicked_x, clicked_y, clicked_z, double);
        refresh_neighbours(world, clicked_x, clicked_y, clicked_z);
        log_debug!("Постановка: плита {} стала двойной в {} {} {}", name, clicked_x, clicked_y, clicked_z);
        return Placement::Merged;
    }

    let (dx, dy, dz) = match click.face {
        0 => (0, -1, 0),
        1 => (0, 1, 0),
        2 => (0, 0, -1),
        3 => (0, 0, 1),
        4 => (-1, 0, 0),
        5 => (1, 0, 0),
        _ => (0, 0, 0),
    };

    // Щелчок по траве, снежному слою, лиане ставит блок на её место, а не
    // рядом: такой блок заменяется поставленным. Воздух и жидкость щелчком
    // не выбираются — по ним не щёлкают.
    let clicked_state = world.get_block(clicked_x, clicked_y, clicked_z);
    let (x, y, z) = if clicked_state != world::AIR && fluids::fluid_at(clicked_state).is_none() && is_replaceable(clicked_state) {
        (clicked_x, clicked_y, clicked_z)
    } else {
        (clicked_x + dx, clicked_y + dy, clicked_z + dz)
    };

    // Ставить можно в пустое место или туда, где стоит жидкость: блок её
    // вытесняет, как в игре.
    if !is_replaceable(world.get_block(x, y, z)) {
        return Placement::Nothing;
    }

    // Состояние считается по тому, как игрок щёлкнул, и по тому, что стоит
    // вокруг: забор срастается с соседями, а у кнопки поворот зависит
    // от грани щелчка.
    let state = orientation
        .and_then(|orientation| state_in_world(orientation, click, world, x, y, z))
        .unwrap_or(default_state);

    // Лист, поставленный игроком, не гниёт (farming.rs).
    let state = crate::farming::placed(world, state, (x, y, z));

    // Проводу, факелу, рычагу нужна опора: в воздухе они не держатся.
    if !has_support(orientation, state, world, x, y, z) {
        log_debug!("Постановка: {} в {} {} {} не на что опереться", name, x, y, z);
        return Placement::Refused(vec![(x, y, z)]);
    }

    // Растения — только на своём: цветок на земле, тростник у воды, кактус
    // на песке.
    if !crate::plants::can_place(world, state, x, y, z) {
        log_debug!("Постановка: {} в {} {} {} расти не на чем", name, x, y, z);
        return Placement::Refused(vec![(x, y, z)]);
    }

    // Дверь, высокое растение и кровать занимают два блока и ставятся сразу
    // целыми: дверь и растение — нижняя половина в этот блок, верхняя над
    // ним; кровать — изножье в этот блок, изголовье дальше от игрока.
    if let Some(orientation) = orientation
        && let Some((state, other, other_state)) = pair_to_place(orientation, state, click, (x, y, z))
    {
        return place_pair(world, name, orientation, (x, y, z, state), (other, other_state), player);
    }

    // Блок в игрока не ставится: он окажется замурован. Блоки без ящика —
    // трава, факел — никому не мешают.
    if blocks_player(x, y, z, state, px, py, pz) {
        log_debug!("Постановка: блок в {} {} {} задел бы игрока — не ставим", x, y, z);
        return Placement::Refused(vec![(x, y, z)]);
    }

    world.set_block(x, y, z, state);

    if custom.is_some() {
        world.set_custom_tnt((x, y, z), custom);
    }

    crate::redstone::game_event(world, (x, y, z), crate::redstone::FREQ_PLACE);

    // Соседние заборы, стенки и панели должны увидеть новый блок и срастись
    // с ним, а редстоун — узнать о нём.
    refresh_neighbours(world, x, y, z);
    crate::redstone::settle(world);

    // То, что делает только постановка игроком: крючок собирает цепь.
    crate::redstone::placed_by_player(world, (x, y, z));

    // На диск это попадёт с общей записью раз в секунду: писать под замком
    // мира на каждый блок незачем.
    log_debug!("Постановка: блок {} ({}) в {} {} {}", name, state, x, y, z);
    Placement::Placed
}

/// Блок из двух частей — какой частью он ставится сюда и где и какой будет
/// вторая. None — блок обычный, из одной части.
///
/// Пары две: половины друг над другом (`half` = `lower`/`upper` — двери и
/// высокие растения) и кровать (`part` = `foot`/`head`). Кровать смотрит
/// туда же, куда игрок, и изголовье встаёт дальше от него (вики, Bed:
/// «the foot of the bed is placed on the block selected and the head of the
/// bed on the block farther away from the player»).
fn pair_to_place(
    orientation: &Orientation,
    state: i32,
    click: &Click,
    (x, y, z): (i32, i32, i32),
) -> Option<(i32, (i32, i32, i32), i32)> {
    // Росток кувшинчика ставится одним блоком: вторая половина вырастает
    // у него потом.
    if orientation.name == "pitcher_crop" {
        return None;
    }

    if orientation.value(state, "half").is_some_and(|half| half == "lower" || half == "upper") {
        let lower = orientation.with(state, "half", "lower")?;
        let upper = other_half(orientation, lower, "upper")?;

        return Some((lower, (x, y + 1, z), upper));
    }

    if orientation.value(state, "part").is_some() {
        let facing = direction_from_yaw(click.yaw);
        let foot = orientation.state(&[("facing", facing), ("part", "foot")])?;
        let head = orientation.with(foot, "part", "head")?;
        let other = blocks::pair_partner(foot, (x, y, z))?;

        return Some((foot, other, head));
    }

    None
}

/// Ставит блок из двух частей — обе сразу или никакой.
///
/// Второе место должно быть свободным или заменяемым и в пределах мира:
/// дверь у самого потолка мира без верхней половины не ставится. Двери
/// нужна опора снизу; кровати на Java — нет (вики, Bed: «In Java, beds do
/// not require supporting blocks»); высокому растению — своя земля, её
/// проверил вызывающий.
fn place_pair(
    world: &mut World,
    name: &str,
    orientation: &Orientation,
    (x, y, z, state): (i32, i32, i32, i32),
    ((ox, oy, oz), other_state): ((i32, i32, i32), i32),
    (px, py, pz): (f64, f64, f64),
) -> Placement {
    let both = vec![(x, y, z), (ox, oy, oz)];

    if oy >= world::MIN_Y + world::WORLD_HEIGHT || !is_replaceable(world.get_block(ox, oy, oz)) {
        log_debug!("Постановка: второй части {} места нет — в {} {} {} не встанет", name, x, y, z);
        return Placement::Refused(both);
    }

    if is_door(orientation) && !blocks::solid_top(world.get_block(x, y - 1, z)) {
        log_debug!("Постановка: под дверью нет опоры — в {} {} {} она не встанет", x, y, z);
        return Placement::Refused(both);
    }

    if blocks_player(x, y, z, state, px, py, pz) || blocks_player(ox, oy, oz, other_state, px, py, pz) {
        log_debug!("Постановка: {} в {} {} {} задел бы игрока — не ставим", name, x, y, z);
        return Placement::Refused(both);
    }

    world.set_block(x, y, z, state);
    world.set_block(ox, oy, oz, other_state);
    refresh_neighbours(world, x, y, z);
    refresh_neighbours(world, ox, oy, oz);
    crate::redstone::settle(world);

    // У кровати — данные блок-сущности обеих половин (bed.rs).
    if name.ends_with("_bed") {
        crate::bed::note_entities(world, (ox, oy, oz));
    }

    log_debug!("Постановка: {} ({}) в {} {} {} и {} {} {}", name, state, x, y, z, ox, oy, oz);
    Placement::Placed
}

/// Ломает блок игроком — общее для Java и Bedrock. Возвращает, что было на
/// месте, или None, если там и так пусто. Выпадение предметов — дело
/// вызывающего: оно зависит от режима игры и инструмента.
pub fn break_block(world: &mut World, x: i32, y: i32, z: i32) -> Option<i32> {
    let broken = world.get_block(x, y, z);

    if broken == world::AIR {
        return None;
    }

    // Крючок растяжки, сломанный игроком, звучит «detach».
    crate::redstone::hook_broken_by_player(world, (x, y, z), broken);

    world.set_block(x, y, z, world::AIR);
    crate::redstone::game_event(world, (x, y, z), crate::redstone::FREQ_DESTROY);

    // Дверь, высокое растение и кровать занимают два блока и ломаются тоже
    // целиком: иначе от них осталась бы висеть вторая половинка. Вторая
    // убирается без выпадения — предмет один на обе, и его выпадение считает
    // вызывающий по сломанной половине.
    if let Some((ox, oy, oz)) = blocks::pair_partner(broken, (x, y, z))
        && blocks::is_pair(broken, (x, y, z), world.get_block(ox, oy, oz), (ox, oy, oz))
    {
        world.set_block(ox, oy, oz, world::AIR);
        refresh_neighbours(world, ox, oy, oz);
    }

    // Поршень состоит из двух половин, и ломается целиком — с какой
    // стороны его ни ломай.
    if let Some((px, py, pz)) = crate::redstone::piston_pair(world, (x, y, z), broken) {
        world.set_block(px, py, pz, world::AIR);
    }

    // Соседи могли срастись с этим блоком — теперь им надо расцепиться.
    refresh_neighbours(world, x, y, z);

    // Редстоун должен узнать, что блока не стало: провод на нём пропадёт,
    // а запитанное им погаснет.
    crate::redstone::settle(world);

    Some(broken)
}

/// Как игрок щёлкнул: по какой грани, куда попал и куда смотрит.
pub struct Click {
    /// Грань, по которой пришёлся щелчок: 0 — низ, 1 — верх, 2 — север,
    /// 3 — юг, 4 — запад, 5 — восток.
    pub face: i32,

    /// Высота точки щелчка внутри блока: 0 — самый низ, 1 — самый верх.
    pub cursor_y: f32,

    /// Поворот игрока вокруг вертикали.
    pub yaw: f32,

    /// Наклон взгляда: вверх отрицательный, вниз положительный. По нему
    /// поршень понимает, что его ставят головой вверх или вниз.
    pub pitch: f32,
}

/// Рост игрока и ширина его плеч — по ним видно, заденет ли его блок.
pub const PLAYER_WIDTH: f64 = 0.6;
pub const PLAYER_HEIGHT: f64 = 1.8;

/// Стороны света, с которыми блок может срастись: имя свойства и смещение
/// соседа по X и Z. Север в игре — это минус Z, восток — плюс X.
const SIDES: [(&str, i32, i32); 4] = [
    ("north", 0, -1),
    ("east", 1, 0),
    ("south", 0, 1),
    ("west", -1, 0),
];

/// Шесть соседей блока: смещения по X, Y и Z.
const NEIGHBOURS: [(i32, i32, i32); 6] = [
    (0, -1, 0),
    (0, 1, 0),
    (0, 0, -1),
    (0, 0, 1),
    (-1, 0, 0),
    (1, 0, 0),
];

/// Каким состоянием ставить блок — с учётом того, как игрок щёлкнул.
///
/// None означает, что правило для этого блока ещё не выяснено: тогда ставить
/// надо состоянием по умолчанию.
///
/// Соседей здесь не спрашивают: то, что зависит от них, подставляет
/// state_in_world — ему для этого нужен мир.
pub fn state_for(orientation: &Orientation, click: &Click) -> Option<i32> {
    match orientation.family {
        // Бревно ложится поперёк грани, по которой щёлкнули, — куда при этом
        // смотрит игрок, значения не имеет.
        Family::Pillar => orientation.state(&[("axis", axis_from_face(click.face)?)]),

        // У ступеней высокая сторона смотрит туда же, куда и игрок, а низкая
        // ступенька достаётся той половине блока, по которой пришёлся щелчок.
        //
        // Форма ступени (прямая, угловая) зависит от соседних ступеней и здесь
        // не считается: одиночная ступень всегда прямая, а углы между соседними
        // ступенями пока не срастаются.
        Family::Stairs => orientation.state(&[
            ("facing", direction_from_yaw(click.yaw)),
            ("half", half_name(click)),
        ]),

        // Дверь смотрит туда же, куда и игрок. Ставится она нижней половиной:
        // верхняя приставляется к ней сверху отдельным блоком.
        Family::Door => orientation.state(&[
            ("facing", direction_from_yaw(click.yaw)),
            ("half", "lower"),
        ]),

        // Калитка открывается от игрока, то есть смотрит туда же, куда и он.
        Family::Gate => orientation.state(&[("facing", direction_from_yaw(click.yaw))]),

        // Кнопка и рычаг крепятся к той грани, по которой щёлкнули: сверху —
        // на пол, снизу — на потолок, сбоку — на стену.
        Family::Face => orientation.state(&[
            ("face", face_name(click.face)?),
            ("facing", facing_for_face(click)?),
        ]),

        // У забора, панели и стенки от щелчка не зависит ничего: они смотрят
        // не на игрока, а на то, что стоит рядом. Стороны подставит
        // state_in_world.
        Family::Fence | Family::Pane | Family::Wall => orientation.state(&[]),

        // Плита занимает ту половину блока, по которой пришёлся щелчок.
        Family::Slab => orientation.state(&[("type", half_name(click))]),

        // Провод от щелчка не зависит: как соединиться с соседями, он решает
        // сам, а силу ему даёт редстоун. Кладётся он крестиком — состояние
        // по умолчанию у него точка, но игра ставит именно крестик.
        Family::Wire => orientation.state(&[
            ("north", "side"),
            ("east", "side"),
            ("south", "side"),
            ("west", "side"),
        ]),

        // Настенный факел смотрит от стены — в ту сторону, откуда щёлкнули.
        // Напольному смотреть некуда.
        Family::Torch => {
            if orientation.name.contains("wall") {
                orientation.state(&[("facing", facing_for_face(click)?)])
            } else {
                orientation.state(&[])
            }
        }

        // У повторителя и сравнителя «facing» — это сторона входа, а вход
        // ближе к игроку: то есть смотрят они навстречу игроку.
        Family::Diode => orientation.state(&[("facing", opposite_direction(direction_from_yaw(click.yaw)))]),

        // Поршень встаёт головой к игроку: смотришь вниз — голова торчит
        // вверх. Вики: «facing — сторона, противоположная той, куда смотрит
        // игрок при установке».
        Family::Piston => orientation.state(&[("facing", facing_the_player(click))]),

        // Наблюдатель встаёт глазами от игрока: «the observing face always
        // faces away from you». То есть смотрит он туда же, куда и игрок.
        Family::Other if orientation.name == "observer" => {
            orientation.state(&[("facing", opposite_direction(facing_the_player(click)))])
        }

        // Сундук смотрит защёлкой на игрока: «The opposite from the
        // direction the player faces» (вики, Chest). С соседом он сливается
        // в `state_in_world`.
        Family::Other if is_chest_name(orientation.name) => {
            orientation.state(&[("facing", opposite_direction(direction_from_yaw(click.yaw))), ("type", "single")])
        }

        // Настенная табличка смотрит от стены — в сторону грани, по которой
        // щёлкнули; стоящая поворачивается по взгляду игрока на 16 румбов
        // (вики, Sign: «rotation … 0 … 15»; рыскание + 180).
        Family::Other if orientation.name.ends_with("_wall_sign") => {
            let facing = match click.face {
                2 => "north",
                3 => "south",
                4 => "west",
                _ => "east",
            };

            orientation.state(&[("facing", facing)])
        }
        // Череп или голова на полу — по взгляду игрока на 16 румбов, без
        // поворота на 180° (чёрный ящик, wither-summon: рыскание −18° —
        // rotation 15, рыскание 0 — rotation 0).
        Family::Other if (orientation.name.ends_with("_skull") || (orientation.name.ends_with("_head") && orientation.name != "piston_head")) && !orientation.name.contains("wall") => {
            let turn = ((click.yaw * 16.0 / 360.0 + 0.5).floor() as i32).rem_euclid(16);

            orientation.state(&[("rotation", &turn.to_string())])
        }
        Family::Other if orientation.name.ends_with("_sign") && !orientation.name.contains("hanging") => {
            let turn = (((180.0 + click.yaw) * 16.0 / 360.0 + 0.5).floor() as i32).rem_euclid(16);

            orientation.state(&[("rotation", &turn.to_string())])
        }

        // Эндер-сундук смотрит как сундук: защёлкой на игрока (вики, Ender
        // Chest: «facing … opposite the direction the player faces»).
        Family::Other if orientation.name == "ender_chest" => {
            orientation.state(&[("facing", opposite_direction(direction_from_yaw(click.yaw)))])
        }

        // Шулкер-ящик открывается от той грани, по которой щёлкнули: «facing»
        // — сторона крышки (вики, Shulker Box: «the face of the block it was
        // placed against»).
        Family::Other if orientation.name.ends_with("shulker_box") => {
            let facing = match click.face {
                0 => "down",
                1 => "up",
                2 => "north",
                3 => "south",
                4 => "west",
                _ => "east",
            };

            orientation.state(&[("facing", facing)])
        }

        // Бочка, выбрасыватель и раздатчик — лицом к игроку по всем шести
        // сторонам, как поршень (вики, Dropper: «The opposite from the
        // direction the player faces»; Barrel — так же).
        Family::Other if matches!(orientation.name, "barrel" | "dropper" | "dispenser") => {
            orientation.state(&[("facing", facing_the_player(click))])
        }

        // Воронка выходом в тот блок, по которому щёлкнули; вверх она не
        // смотрит — щелчок по потолку даёт «вниз» (вики, Hopper: «aiming at
        // the surface to which its output should face»).
        Family::Other if orientation.name == "hopper" => {
            let facing = match click.face {
                2 => "south",
                3 => "north",
                4 => "east",
                5 => "west",
                _ => "down",
            };

            orientation.state(&[("facing", facing)])
        }

        // Рельса без соседок ложится по взгляду игрока (вики, Rail: «in
        // java the new rail orients itself in the direction the player is
        // facing», с 19w39a). К соседкам она перестроится сама, когда
        // встанет (redstone/rails.rs). [проверить]: так ли у рельс
        // с питанием, детекторной и активирующей — на их страницах вики
        // пишет «north-south», у активирующей — «testingame».
        Family::Other if is_rail(orientation.name) => {
            let shape = match direction_from_yaw(click.yaw) {
                "north" | "south" => "north_south",
                _ => "east_west",
            };

            orientation.state(&[("shape", shape)])
        }

        // Правило не выяснено — состояние по умолчанию.
        Family::Other => None,
    }
}

/// Сундук, поставленный рядом с одиночным сундуком того же вида и с той же
/// стороной, сливается с ним в большой (вики, Chest). Если подходят оба
/// соседа, берётся тот, что по часовой стрелке от стороны сундука, —
/// [проверить] чёрным ящиком. Соседа перестраивает его обновление формы.
/// Сундук ли это, что сливается с соседом в большой: простой, ловушка,
/// медный любой стадии.
fn is_chest_name(name: &str) -> bool {
    matches!(name, "chest" | "trapped_chest") || name.ends_with("copper_chest")
}

fn chest_joined(orientation: &Orientation, world: &World, (x, y, z): (i32, i32, i32), state: i32) -> i32 {
    let Some(facing) = orientation.value(state, "facing") else {
        return state;
    };
    let offset = |name: &str| match name {
        "north" => (0, -1),
        "south" => (0, 1),
        "west" => (-1, 0),
        _ => (1, 0),
    };
    let (fx, fz) = offset(facing);
    // По часовой стрелке от стороны сундука — сосед, к которому новый будет
    // «левой» половиной; против — «правой».
    let (cx, cz) = (-fz, fx);

    for (dx, dz, own) in [(cx, cz, "left"), (-cx, -cz, "right")] {
        let other = world.get_block(x + dx, y, z + dz);

        // Медный сундук сливается с любым медным (вики, Copper Chest);
        // стадии уравнивает потом обновление формы (redstone.rs).
        let same = blocks::block_at_state(other)
            .is_some_and(|name| name == orientation.name || (name.ends_with("copper_chest") && orientation.name.ends_with("copper_chest")));

        let other_value = |name: &str| blocks::orientation_of(other).and_then(|o| o.value(other, name));

        if same
            && other_value("facing") == Some(facing)
            && other_value("type") == Some("single")
            && let Some(joined) = orientation.with(state, "type", own)
        {
            return joined;
        }
    }

    state
}

/// Какой блок ставить от предмета с учётом грани: факел на стене — это
/// другой блок, настенный.
///
/// Для всего, кроме факелов, возвращает то же, что получил. None означает,
/// что на эту грань предмет не ставится: факел не вешают на потолок.
pub fn block_for_face(
    name: &'static str,
    default_state: i32,
    face: i32,
) -> Option<(&'static str, i32)> {
    if !name.ends_with("torch") || name.contains("wall") {
        return Some((name, default_state));
    }

    match face {
        // Снизу факел не держится.
        0 => None,
        // Сверху — напольный, как и есть.
        1 => Some((name, default_state)),
        // Сбоку — настенный: у него своё имя с «wall».
        _ => {
            let wall = blocks::orientation(&name.replace("torch", "wall_torch"))?;

            Some((wall.name, blocks::state_by_name(wall.name)?))
        }
    }
}

/// Держится ли деталь на том, что вокруг: провод, повторитель и плита — на
/// блоке снизу, факел, рычаг и кнопка — на том, к чему прикреплены.
///
/// Прочие блоки держатся сами по себе.
pub fn has_support(orientation: Option<&Orientation>, state: i32, world: &World, x: i32, y: i32, z: i32) -> bool {
    let Some(orientation) = orientation else {
        return true;
    };

    let below_is_solid = || blocks::solid_top(world.get_block(x, y - 1, z));

    match orientation.family {
        Family::Wire | Family::Diode => below_is_solid(),
        Family::Other if orientation.name.ends_with("_pressure_plate") => below_is_solid(),
        // Рельсе нужен верх с ободком: полный блок, воронка, верхняя плита,
        // перевёрнутые ступени, закрытый верхний люк (вики, Rail).
        Family::Other if is_rail(orientation.name) => crate::redstone::holds_rail(world.get_block(x, y - 1, z)),
        Family::Torch => {
            if orientation.name.contains("wall") {
                let (dx, dz) = match orientation.value(state, "facing") {
                    Some("north") => (0, 1),
                    Some("south") => (0, -1),
                    Some("west") => (1, 0),
                    _ => (-1, 0),
                };

                blocks::has_box(world.get_block(x + dx, y, z + dz))
            } else {
                below_is_solid()
            }
        }
        Family::Face => {
            let (dx, dy, dz) = match (orientation.value(state, "face"), orientation.value(state, "facing")) {
                (Some("floor"), _) => (0, -1, 0),
                (Some("ceiling"), _) => (0, 1, 0),
                (_, Some("north")) => (0, 0, 1),
                (_, Some("south")) => (0, 0, -1),
                (_, Some("west")) => (1, 0, 0),
                _ => (-1, 0, 0),
            };

            blocks::has_box(world.get_block(x + dx, y + dy, z + dz))
        }
        _ => true,
    }
}

/// Куда должна смотреть голова поршня: на игрока.
///
/// Считается по взгляду целиком, а не по одному повороту: из трёх осей
/// берётся та, вдоль которой игрок смотрит сильнее всего, и голова
/// направляется ей навстречу. Поэтому, глядя под ноги, ставишь поршень
/// головой вверх, а глядя на север — головой на юг.
fn facing_the_player(click: &Click) -> &'static str {
    let yaw = click.yaw.to_radians();
    let pitch = click.pitch.to_radians();

    // Взгляд как направление: север — это минус Z, наклон вниз — минус Y.
    let x = -yaw.sin() * pitch.cos();
    let y = -pitch.sin();
    let z = yaw.cos() * pitch.cos();

    if y.abs() > x.abs() && y.abs() > z.abs() {
        return if y > 0.0 { "down" } else { "up" };
    }

    if x.abs() > z.abs() {
        return if x > 0.0 { "west" } else { "east" };
    }

    if z > 0.0 { "north" } else { "south" }
}

/// Сторона, противоположная этой.
pub fn opposite_direction(direction: &str) -> &'static str {
    match direction {
        "north" => "south",
        "south" => "north",
        "east" => "west",
        "west" => "east",
        "up" => "down",
        _ => "up",
    }
}

/// Каким состоянием ставить блок, если известно ещё и что стоит вокруг.
///
/// Забор, панель и стенка срастаются с соседями, а у калитки от соседей
/// зависит признак «встроена в стену». Всё это видно только по миру, поэтому
/// одного щелчка таким блокам мало.
pub fn state_in_world(
    orientation: &Orientation,
    click: &Click,
    world: &World,
    x: i32,
    y: i32,
    z: i32,
) -> Option<i32> {
    let state = state_for(orientation, click)?;

    if is_chest_name(orientation.name) {
        return Some(chest_joined(orientation, world, (x, y, z), state));
    }

    with_neighbours(orientation, world, x, y, z, state)
}

/// То же состояние блока, но с пересчитанными по соседям свойствами.
fn with_neighbours(
    orientation: &Orientation,
    world: &World,
    x: i32,
    y: i32,
    z: i32,
    state: i32,
) -> Option<i32> {
    let mut updated = state;

    for (name, value) in neighbour_values(orientation, world, x, y, z, state) {
        updated = orientation.with(updated, name, value)?;
    }

    Some(updated)
}

/// Значения свойств, которые зависят не от щелчка, а от соседей.
///
/// Считаются по миру: забор срастается с тем, что стоит рядом, у стенки
/// от соседей зависит и бок, и столб, а у калитки — признак «встроена
/// в стену».
fn neighbour_values(
    orientation: &Orientation,
    world: &World,
    x: i32,
    y: i32,
    z: i32,
    state: i32,
) -> Vec<(&'static str, &'static str)> {
    let mut values = Vec::new();

    // Стороны по порядку SIDES: север, восток, юг, запад.
    let mut joined = [false; 4];

    if orientation.family.connects_to_sides() {
        for (index, (name, dx, dz)) in SIDES.into_iter().enumerate() {
            let neighbour = world.get_block(x + dx, y, z + dz);
            let value = side_value(orientation, neighbour);

            joined[index] = value != "none" && value != "false";
            values.push((name, value));
        }
    }

    // Столб у стенки есть у одиночной стенки и у поворотов, а пропадает, когда
    // стенка тянется в одну сторону или стоит на перекрёстке: там её бок и так
    // занимает весь блок, и столб был бы лишним.
    if orientation.family == Family::Wall {
        let [north, east, south, west] = joined;
        let straight = (north && south && !east && !west) || (east && west && !north && !south);
        let crossroads = north && south && east && west;

        values.push(("up", yes_no(!straight && !crossroads)));
    }

    // Калитка «встроена в стену», когда стенки стоят с её торцов — тех,
    // что по бокам от её полотна.
    if orientation.family == Family::Gate {
        let [[first_x, first_z], [second_x, second_z]] =
            match orientation.value(state, "facing") {
                Some("north") | Some("south") => [[1, 0], [-1, 0]],
                _ => [[0, -1], [0, 1]],
            };

        let in_wall = is_wall(world.get_block(x + first_x, y, z + first_z))
            || is_wall(world.get_block(x + second_x, y, z + second_z));

        values.push(("in_wall", yes_no(in_wall)));
    }

    values
}

/// Насколько блок срастается с соседом — значением свойства.
///
/// У забора и панели это просто «да/нет», а у стенки три степени: «нет»,
/// «низко» и «высоко». От степени зависит, докуда дотягивается её бок.
fn side_value(orientation: &Orientation, neighbour: i32) -> &'static str {
    if orientation.family == Family::Wall {
        return wall_side(neighbour);
    }

    yes_no(joins(orientation, neighbour))
}

/// Дотягивается ли стенка до соседа и на какую высоту.
///
/// Во всю высоту — до другой стенки, до панели и до полного куба: у них бок
/// твёрдый во весь блок. Пониже — до всего остального твёрдого. А до забора
/// не дотягивается вовсе: забор в стенку не вставляется.
fn wall_side(neighbour: i32) -> &'static str {
    if neighbour == world::AIR || !blocks::has_box(neighbour) {
        return "none";
    }

    let Some(name) = blocks::block_at_state(neighbour) else {
        return "none";
    };

    if family_of(name) == Some(Family::Fence) {
        return "none";
    }

    if blocks::full_cube(neighbour) || matches!(family_of(name), Some(Family::Wall | Family::Pane)) {
        return "tall";
    }

    "low"
}

/// Срастается ли забор или панель с тем, что стоит в соседнем блоке.
///
/// Срастается со всем твёрдым — с тем, с чем можно столкнуться: у такого блока
/// есть за что зацепиться. Плита, ступени и дверь тоже твёрдые, поэтому забор
/// тянется и к ним. Пустое место и то, сквозь что проходят — трава, факел,
/// вода, — не считаются.
///
/// Исключения два: забор из незерской породы не срастается с обычным — это
/// разные породы дерева, — и забор не срастается со стенкой.
fn joins(orientation: &Orientation, neighbour: i32) -> bool {
    if neighbour == world::AIR || !blocks::has_box(neighbour) {
        return false;
    }

    let Some(name) = blocks::block_at_state(neighbour) else {
        return false;
    };

    if orientation.family == Family::Fence {
        if family_of(name) == Some(Family::Wall) {
            return false;
        }

        // Свои породы: обычный забор с незерским не срастается.
        if family_of(name) == Some(Family::Fence)
            && is_nether_wood(orientation.name) != is_nether_wood(name)
        {
            return false;
        }
    }

    true
}

/// Незерская ли это порода дерева. Заборы из неё держатся особняком.
fn is_nether_wood(name: &str) -> bool {
    name.starts_with("crimson_") || name.starts_with("warped_")
}

/// Семейство блока по его имени.
fn family_of(name: &str) -> Option<Family> {
    blocks::orientation(name).map(|orientation| orientation.family)
}

/// Стенка ли это: стенки отличаются от прочих соединяющихся блоков тем,
/// что у них есть столб.
fn is_wall(state: i32) -> bool {
    blocks::orientation_of(state).is_some_and(|orientation| orientation.family == Family::Wall)
}

/// «Да» или «нет» значением свойства — у признаков порядок такой.
fn yes_no(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}

/// Пересчитывает формы соседей после изменения блока.
///
/// Забор, поставленный рядом с другим забором, должен срастись с ним — и
/// наоборот: старый забор тоже должен увидеть нового соседа. То же со стенками
/// и панелями. Игра пересчитывает это при каждом изменении блока, и сервер
/// обязан делать так же, иначе заборы срастаются в одну сторону.
///
/// Изменения записываются в мир обычным порядком, поэтому их увидят все.
pub fn refresh_neighbours(world: &mut World, x: i32, y: i32, z: i32) {
    for (dx, dy, dz) in NEIGHBOURS {
        let (nx, ny, nz) = (x + dx, y + dy, z + dz);
        let state = world.get_block(nx, ny, nz);

        let Some(name) = blocks::block_at_state(state) else {
            continue;
        };
        let Some(orientation) = blocks::orientation(name) else {
            continue;
        };

        if !orientation.family.connects_to_sides() && orientation.family != Family::Gate {
            continue;
        }

        if let Some(updated) = with_neighbours(orientation, world, nx, ny, nz, state)
            && updated != state
        {
            world.set_block(nx, ny, nz, updated);
        }
    }
}

/// Вторую половину двери считают по первой: у обеих половин поворот, сторона
/// петель и открытость общие, а различаются они только свойством «половина».
///
/// Дверь всегда занимает два блока по высоте, поэтому, поставив нижнюю
/// половину, сервер обязан поставить и верхнюю — иначе от двери останется
/// одна половинка, в которую нельзя войти.
pub fn other_half(orientation: &Orientation, state: i32, half: &str) -> Option<i32> {
    orientation.with(state, "half", half)
}

/// Можно ли поставить блок прямо в это место.
///
/// Пустое место — можно. Воду и лаву блок вытесняет: в игре так и есть,
/// иначе под водой было бы не построить ничего. Траву, снежный слой в один
/// слой, лианы и прочее из тега `minecraft:replaceable` — тоже: они
/// пропадают без выпадения (вики, Tall Grass: «Placing any block on tall
/// grass breaks the grass and causes it to not drop anything»).
pub fn is_replaceable(state: i32) -> bool {
    blocks::replaceable(state)
}

/// Дверь ли это.
///
/// Дверь — единственный блок, который занимает два блока по высоте, поэтому
/// и ставить, и ломать её приходится парой.
pub fn is_door(orientation: &Orientation) -> bool {
    orientation.family == Family::Door
}

/// Мешает ли блок, поставленный в этот блок, стоящему там игроку.
///
/// Ставить блок в игрока нельзя: он окажется замурован. Проверяется ящик
/// блока — у травы, факела и воды ящика нет, и они никому не мешают.
///
/// Игрок занимает 0.6 блока в ширину и 1.8 в высоту, а стоит ногами в точке
/// положения: ящик игрока — от точки вверх на его рост.
pub fn blocks_player(x: i32, y: i32, z: i32, state: i32, px: f64, py: f64, pz: f64) -> bool {
    if !blocks::has_box(state) {
        return false;
    }

    let half = PLAYER_WIDTH / 2.0;

    overlaps(px - half, px + half, x as f64, x as f64 + 1.0)
        && overlaps(py, py + PLAYER_HEIGHT, y as f64, y as f64 + 1.0)
        && overlaps(pz - half, pz + half, z as f64, z as f64 + 1.0)
}

/// Пересекаются ли два отрезка.
///
/// Касание концами пересечением не считается: игрок, стоящий вплотную к стене,
/// не мешает поставить блок в соседнем с собой блоке — он и так в нём стоит.
/// А вот стоит он ровно на границе блока — блок в этот блок поставить можно.
fn overlaps(from_a: f64, to_a: f64, from_b: f64, to_b: f64) -> bool {
    from_a < to_b && from_b < to_a
}


/// Становится ли блок, по которому щёлкнули, двойной плитой.
///
/// Если поставить плиту на плиту того же вида, они занимают место одного блока
/// и слипаются в двойную. Здесь разобран только очевидный случай — щелчок по
/// свободной половине плиты: сверху по нижней, снизу по верхней. Щелчок по
/// боку плиты в игре ставит новую плиту рядом, а не сливает их, и это тоже
/// учтено.
pub fn merge_into_double(
    orientation: &Orientation,
    clicked_state: i32,
    click: &Click,
) -> Option<i32> {
    if orientation.family != Family::Slab {
        return None;
    }

    let free_half_is_clicked = match orientation.value(clicked_state, "type")? {
        "bottom" => click.face == 1,
        "top" => click.face == 0,
        // Плита уже двойная — сливать нечего.
        _ => false,
    };

    if !free_half_is_clicked {
        return None;
    }

    orientation.state(&[("type", "double")])
}

/// Как крепится кнопка или рычаг: на полу, на потолке или на стене.
///
/// Щелчок по верхней грани блока крепит кнопку на пол, по нижней — на потолок,
/// по боковой — на стену.
fn face_name(face: i32) -> Option<&'static str> {
    match face {
        0 => Some("ceiling"),
        1 => Some("floor"),
        2..=5 => Some("wall"),
        _ => None,
    }
}

/// Куда смотрит кнопка или рычаг.
///
/// На стене — от стены: щелчок по северной грани ставит кнопку с северной
/// стороны блока, и смотрит она на север. На полу и на потолке направление
/// берётся у игрока.
fn facing_for_face(click: &Click) -> Option<&'static str> {
    match click.face {
        0 | 1 => Some(direction_from_yaw(click.yaw)),
        _ => direction_from_face(click.face),
    }
}

/// Название стороны света по грани блока.
fn direction_from_face(face: i32) -> Option<&'static str> {
    match face {
        2 => Some("north"),
        3 => Some("south"),
        4 => Some("west"),
        5 => Some("east"),
        _ => None,
    }
}

/// Ось «столбового» блока — по грани, по которой щёлкнули.
///
/// Блок ложится поперёк этой грани: щелчок по боку кладёт бревно набок, щелчок
/// сверху или снизу ставит его стоймя.
fn axis_from_face(face: i32) -> Option<&'static str> {
    match face {
        0 | 1 => Some("y"),
        2 | 3 => Some("z"),
        4 | 5 => Some("x"),
        _ => None,
    }
}

/// Рельса ли это: обычная, с питанием, детекторная или активирующая.
fn is_rail(name: &str) -> bool {
    matches!(name, "rail" | "powered_rail" | "detector_rail" | "activator_rail")
}

/// Направление, в которое смотрит игрок.
///
/// Поворот в игре отсчитывается так: 0 — на юг, 90 — на запад, 180 — на север,
/// 270 — на восток.
fn direction_from_yaw(yaw: f32) -> &'static str {
    let yaw = yaw.rem_euclid(360.0);

    if !(45.0..315.0).contains(&yaw) {
        "south"
    } else if yaw < 135.0 {
        "west"
    } else if yaw < 225.0 {
        "north"
    } else {
        "east"
    }
}

/// Какая половина блока достаётся поставленному: верхняя или нижняя.
///
/// Щелчок снизу, по потолку, даёт верхнюю половину, щелчок сверху — нижнюю,
/// а щелчок по боку — ту, на которую пришлась точка щелчка.
fn half_name(click: &Click) -> &'static str {
    let top = match click.face {
        0 => true,
        1 => false,
        _ => click.cursor_y > 0.5,
    };

    if top { "top" } else { "bottom" }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks;

    fn click(face: i32, cursor_y: f32, yaw: f32) -> Click {
        Click { face, cursor_y, yaw, pitch: 0.0 }
    }

    fn state(block: &str, click: &Click) -> i32 {
        let orientation = blocks::orientation(block).expect("правило установки");
        state_for(orientation, click).expect("состояние посчиталось")
    }

    fn property(block: &str, state: i32, name: &str) -> &'static str {
        let orientation = blocks::orientation(block).expect("правило установки");
        orientation.value(state, name).expect("свойство есть")
    }

    /// Бревно ложится поперёк грани: щелчок по боку кладёт его набок, щелчок
    /// сверху или снизу ставит стоймя. Взгляд игрока тут ни при чём.
    #[test]
    fn a_log_lies_across_the_clicked_face() {
        assert_eq!(state("oak_log", &click(1, 0.5, 0.0)), 137); // сверху — стоймя
        assert_eq!(state("oak_log", &click(0, 0.5, 0.0)), 137); // снизу — тоже стоймя
        assert_eq!(state("oak_log", &click(2, 0.5, 0.0)), 138); // с севера — вдоль оси z
        assert_eq!(state("oak_log", &click(3, 0.5, 0.0)), 138);
        assert_eq!(state("oak_log", &click(4, 0.5, 0.0)), 136); // с запада — вдоль оси x
        assert_eq!(state("oak_log", &click(5, 0.5, 0.0)), 136);

        // Поворот игрока на бревно не влияет.
        assert_eq!(
            state("oak_log", &click(2, 0.5, 0.0)),
            state("oak_log", &click(2, 0.5, 180.0))
        );
    }

    /// Ступени поворачиваются по взгляду: высокая сторона — от игрока.
    #[test]
    fn stairs_turn_with_the_player() {
        assert_eq!(property("oak_stairs", state("oak_stairs", &click(1, 0.5, 0.0)), "facing"), "south");
        assert_eq!(property("oak_stairs", state("oak_stairs", &click(1, 0.5, 180.0)), "facing"), "north");
        assert_eq!(property("oak_stairs", state("oak_stairs", &click(1, 0.5, 90.0)), "facing"), "west");
        assert_eq!(property("oak_stairs", state("oak_stairs", &click(1, 0.5, 270.0)), "facing"), "east");
    }

    /// Половина ступени и плиты зависит от грани и точки щелчка: щелчок сверху
    /// даёт низкую половину, снизу — высокую, а по боку решает точка щелчка.
    #[test]
    fn clicking_top_gives_the_bottom_half() {
        assert_eq!(property("oak_stairs", state("oak_stairs", &click(1, 0.5, 0.0)), "half"), "bottom");
        assert_eq!(property("oak_stairs", state("oak_stairs", &click(0, 0.5, 0.0)), "half"), "top");

        // Щелчок по боку: выше середины — верхняя половина, ниже — нижняя.
        assert_eq!(property("oak_stairs", state("oak_stairs", &click(2, 0.9, 0.0)), "half"), "top");
        assert_eq!(property("oak_stairs", state("oak_stairs", &click(2, 0.1, 0.0)), "half"), "bottom");

        assert_eq!(property("oak_slab", state("oak_slab", &click(1, 0.5, 0.0)), "type"), "bottom");
        assert_eq!(property("oak_slab", state("oak_slab", &click(0, 0.5, 0.0)), "type"), "top");
        assert_eq!(property("oak_slab", state("oak_slab", &click(3, 0.8, 0.0)), "type"), "top");
        assert_eq!(property("oak_slab", state("oak_slab", &click(3, 0.2, 0.0)), "type"), "bottom");
    }

    /// Череп на полу — по взгляду на 16 румбов без поворота на 180°
    /// (чёрный ящик, wither-summon: рыскание −18° — 15, рыскание 0 — 0).
    #[test]
    fn a_floor_skull_turns_with_the_look() {
        assert_eq!(state("wither_skeleton_skull", &click(1, 0.5, -18.4)), 10986);
        assert_eq!(state("wither_skeleton_skull", &click(1, 0.5, 0.0)), 10971);
        assert_eq!(property("skeleton_skull", state("skeleton_skull", &click(1, 0.5, 90.0)), "rotation"), "4");
    }

    /// Калитка смотрит туда же, куда и игрок: открывается она от него.
    #[test]
    fn a_gate_faces_the_way_the_player_looks() {
        assert_eq!(property("oak_fence_gate", state("oak_fence_gate", &click(1, 0.5, 0.0)), "facing"), "south");
        assert_eq!(property("oak_fence_gate", state("oak_fence_gate", &click(1, 0.5, 90.0)), "facing"), "west");
    }

    /// Поворот округляется до четырёх направлений: границы — посередине между
    /// ними, а поворот вне обычного круга (клиент может прислать любой)
    /// приводится к нему.
    #[test]
    fn the_turn_is_rounded_to_four_directions() {
        assert_eq!(direction_from_yaw(0.0), "south");
        assert_eq!(direction_from_yaw(44.0), "south");
        assert_eq!(direction_from_yaw(46.0), "west");
        assert_eq!(direction_from_yaw(90.0), "west");
        assert_eq!(direction_from_yaw(180.0), "north");
        assert_eq!(direction_from_yaw(270.0), "east");
        assert_eq!(direction_from_yaw(360.0), "south");
        assert_eq!(direction_from_yaw(-90.0), "east");
        assert_eq!(direction_from_yaw(450.0), "west");
    }

    /// Плита на плиту того же вида слипается в двойную — но только если
    /// щёлкнули по её свободной половине. Щелчок по боку ставит новую плиту
    /// рядом, а не сливает их.
    #[test]
    fn a_slab_on_a_slab_becomes_double() {
        let slab = blocks::orientation("oak_slab").expect("правило для плиты");

        let bottom = 13333; // плита в нижней половине блока
        let top = 13331; // плита в верхней половине

        // Щелчок сверху по нижней плите и снизу по верхней — двойная плита.
        assert_eq!(property("oak_slab", merge_into_double(slab, bottom, &click(1, 0.5, 0.0)).expect("слияние"), "type"), "double");
        assert_eq!(property("oak_slab", merge_into_double(slab, top, &click(0, 0.5, 0.0)).expect("слияние"), "type"), "double");

        // Щелчок по боку — плита встанет рядом, сливаться не с чем.
        assert_eq!(merge_into_double(slab, bottom, &click(2, 0.9, 0.0)), None);

        // Щелчок сверху по верхней плите — там уже занято.
        assert_eq!(merge_into_double(slab, top, &click(1, 0.5, 0.0)), None);

        // Чужой блок и уже двойная плита не сливаются ни с чем.
        assert_eq!(merge_into_double(slab, 1, &click(1, 0.5, 0.0)), None); // камень
        let double = 13335;
        assert_eq!(merge_into_double(slab, double, &click(1, 0.5, 0.0)), None);
    }

    /// Ступени и бревно двойными плитами не становятся: это правило только
    /// про плиты.
    #[test]
    fn only_slabs_merge() {
        let log = blocks::orientation("oak_log").expect("правило для бревна");
        let stairs = blocks::orientation("oak_stairs").expect("правило для ступеней");

        assert_eq!(merge_into_double(log, 137, &click(1, 0.5, 0.0)), None);
        assert_eq!(merge_into_double(stairs, 3918, &click(1, 0.5, 0.0)), None);
    }

    /// Блок в игрока не ставится: он окажется замурован. Занимает игрок два
    /// блока по высоте, а в ширину — 0.6, поэтому соседний блок уже свободен.
    #[test]
    fn a_block_inside_the_player_is_not_allowed() {
        let stone = 1;
        let torch = 3370; // факел ящика не имеет — в нём и стоять можно

        // Игрок стоит в блоке (0, 64, 0), ногами в его нижней границе.
        let (x, y, z) = (0, 64, 0);
        let (px, py, pz) = (0.5, 64.0, 0.5);

        assert!(blocks_player(x, y, z, stone, px, py, pz)); // блок под ногами
        assert!(blocks_player(x, y + 1, z, stone, px, py, pz)); // и там, где голова
        assert!(!blocks_player(x, y - 1, z, stone, px, py, pz)); // под ногами — можно

        // Соседний блок не задет: до него игрок не достаёт.
        assert!(!blocks_player(x + 1, y, z, stone, px, py, pz));
        assert!(!blocks_player(x, y, z + 1, stone, px, py, pz));

        // А факел в тот же блок поставить можно.
        assert!(!blocks::has_box(torch));
        assert!(!blocks_player(x, y, z, torch, px, py, pz));
    }

    /// Дверь занимает два блока: поставив нижнюю половинку, сервер обязан
    /// поставить и верхнюю — иначе от двери останется половинка, в которую
    /// нельзя войти.
    #[test]
    fn a_door_takes_two_blocks() {
        let door = blocks::orientation("oak_door").expect("правило для двери");
        assert!(is_door(door));

        let lower = state("oak_door", &click(1, 0.5, 0.0));
        assert_eq!(property("oak_door", lower, "half"), "lower");

        let upper = other_half(door, lower, "upper").expect("верхняя половинка");
        assert_eq!(property("oak_door", upper, "half"), "upper");

        // Поворот и сторона петель у половинок общие: это одна дверь.
        assert_eq!(
            property("oak_door", upper, "facing"),
            property("oak_door", lower, "facing")
        );
        assert_eq!(
            property("oak_door", upper, "hinge"),
            property("oak_door", lower, "hinge")
        );

        // Верхняя половинка — вторая половинка этой же двери, а камень и
        // дверь из другого дерева — нет.
        assert!(blocks::same_block(lower, upper));
        assert!(!blocks::same_block(lower, 1)); // камень

        let spruce = blocks::state_by_name("spruce_door").expect("еловая дверь");
        assert!(!blocks::same_block(spruce, upper));

        // Где вторая половина — над нижней и под верхней.
        assert_eq!(blocks::pair_partner(lower, (0, 64, 0)), Some((0, 65, 0)));
        assert_eq!(blocks::pair_partner(upper, (0, 65, 0)), Some((0, 64, 0)));
    }

    /// Забор срастается с тем, что стоит рядом: с камнем — да, с воздухом —
    /// нет. Отсюда и берётся «забор не соединяется», когда соединение
    /// не считалось.
    #[test]
    fn a_fence_joins_what_stands_next_to_it() {
        let fence = blocks::orientation("oak_fence").expect("правило для забора");
        let mut world = World::in_memory();

        // Камень к северу от того места, куда встанет забор.
        world.set_block(0, 64, -1, 1);

        let state = state_in_world(fence, &click(1, 0.5, 0.0), &world, 0, 64, 0)
            .expect("состояние забора");

        assert_eq!(property("oak_fence", state, "north"), "true");
        assert_eq!(property("oak_fence", state, "south"), "false");
        assert_eq!(property("oak_fence", state, "east"), "false");
        assert_eq!(property("oak_fence", state, "west"), "false");
    }

    /// Сосед, появившийся позже, тоже срастается: уже стоящий забор о нём
    /// узнаёт, а не остаётся с прежними свойствами.
    #[test]
    fn a_fence_already_standing_joins_a_new_neighbour() {
        let fence = blocks::orientation("oak_fence").expect("правило для забора");
        let mut world = World::in_memory();

        let state =
            state_in_world(fence, &click(1, 0.5, 0.0), &world, 0, 64, 0).expect("состояние");
        world.set_block(0, 64, 0, state);
        assert_eq!(property("oak_fence", state, "south"), "false");

        // Ставим камень с юга и пересчитываем соседей — забор должен срастись.
        world.set_block(0, 64, 1, 1);
        refresh_neighbours(&mut world, 0, 64, 1);

        assert_eq!(property("oak_fence", world.get_block(0, 64, 0), "south"), "true");
    }

    /// У стенки бок бывает трёх видов: у полного соседа — «tall», у узкого —
    /// «low», а где пусто — «none». Столб она ставит, только если соседей
    /// не ровно два напротив.
    #[test]
    fn a_wall_measures_its_neighbours() {
        let wall = blocks::orientation("cobblestone_wall").expect("правило для стенки");
        let mut world = World::in_memory();

        world.set_block(0, 64, -1, 1); // камень — сосед во весь блок
        world.set_block(0, 64, 1, 13333); // плита — сосед узкий

        let state =
            state_in_world(wall, &click(1, 0.5, 0.0), &world, 0, 64, 0).expect("состояние стенки");

        assert_eq!(property("cobblestone_wall", state, "north"), "tall");
        assert_eq!(property("cobblestone_wall", state, "south"), "low");
        assert_eq!(property("cobblestone_wall", state, "east"), "none");
        assert_eq!(property("cobblestone_wall", state, "west"), "none");

        // Соседи стоят друг напротив друга — столба нет.
        assert_eq!(property("cobblestone_wall", state, "up"), "false");
    }

    /// Кнопка крепится к той грани, по которой щёлкнули, и смотрит от неё:
    /// щелчок по северной грани — кнопка на стене, лицом на север. На пол
    /// и потолок она ложится по взгляду игрока.
    #[test]
    fn a_button_takes_the_clicked_face() {
        let wall_button = state("oak_button", &click(2, 0.5, 0.0));
        assert_eq!(property("oak_button", wall_button, "face"), "wall");
        assert_eq!(property("oak_button", wall_button, "facing"), "north");

        let floor_button = state("oak_button", &click(1, 0.5, 90.0));
        assert_eq!(property("oak_button", floor_button, "face"), "floor");
        assert_eq!(property("oak_button", floor_button, "facing"), "west");

        let ceiling_button = state("oak_button", &click(0, 0.5, 180.0));
        assert_eq!(property("oak_button", ceiling_button, "face"), "ceiling");
        assert_eq!(property("oak_button", ceiling_button, "facing"), "north");
    }

    /// Блок ставится в пустое место и в жидкость — она вытесняется, — но не
    /// в занятое место.
    #[test]
    fn a_block_replaces_air_and_water() {
        assert!(is_replaceable(world::AIR));
        assert!(is_replaceable(fluids::state_of(fluids::Kind::Water, fluids::SOURCE)));
        assert!(is_replaceable(fluids::state_of(fluids::Kind::Water, 5)));
        assert!(is_replaceable(fluids::state_of(fluids::Kind::Lava, fluids::SOURCE)));

        // Камень и всё прочее заменять нельзя.
        assert!(!is_replaceable(1));
        assert!(!is_replaceable(13333));

        // А траву можно: блок встаёт на её место.
        assert!(is_replaceable(blocks::state_by_name("short_grass").unwrap()));
    }

    /// Мир с каменным полом на высоте 63 — для постановки.
    fn floor() -> World {
        let mut world = World::in_memory();

        for x in -4..=4 {
            for z in -4..=4 {
                world.set_block_silently(x, 63, z, 1);
                world.set_block_silently(x, 62, z, 1);
            }
        }

        world
    }

    fn name_at(world: &World, x: i32, y: i32, z: i32) -> Option<&'static str> {
        blocks::block_at_state(world.get_block(x, y, z))
    }

    /// Щелчок по траве ставит блок на место травы, а не над ней.
    #[test]
    fn a_click_on_grass_places_into_the_grass() {
        let mut world = floor();
        let dirt = blocks::state_by_name("dirt").unwrap();
        let grass = blocks::state_by_name("short_grass").unwrap();

        world.set_block_silently(0, 63, 0, dirt);
        world.set_block_silently(0, 64, 0, grass);

        let placed = place(&mut world, "stone", 1, (0, 64, 0), &click(1, 0.5, 0.0), (5.5, 64.0, 5.5));

        assert_eq!(placed, Placement::Placed);
        assert_eq!(world.get_block(0, 64, 0), 1);
        assert_eq!(world.get_block(0, 65, 0), world::AIR);
    }

    /// Подсолнух ставится сразу целым — обе половины; ломается тоже целым,
    /// и вторая половина при этом не выпадает (выпадение считают по
    /// сломанной половине).
    #[test]
    fn a_sunflower_is_placed_and_broken_whole() {
        let mut world = floor();
        let dirt = blocks::state_by_name("dirt").unwrap();
        let sunflower = blocks::state_by_name("sunflower").unwrap();

        world.set_block_silently(0, 63, 0, dirt);

        let placed = place(&mut world, "sunflower", sunflower, (0, 63, 0), &click(1, 0.5, 0.0), (5.5, 64.0, 5.5));

        assert_eq!(placed, Placement::Placed);
        assert_eq!(name_at(&world, 0, 64, 0), Some("sunflower"));
        assert_eq!(name_at(&world, 0, 65, 0), Some("sunflower"));

        let upper = world.get_block(0, 65, 0);
        assert_eq!(break_block(&mut world, 0, 65, 0), Some(upper));
        assert_eq!(world.get_block(0, 64, 0), world::AIR, "нижняя половина осталась");
        assert!(world.take_destroyed().is_empty(), "вторая половина выпала отдельно");

        // Над ним занято — не ставится вовсе, ни половины.
        world.set_block_silently(0, 65, 0, 1);

        let refused = place(&mut world, "sunflower", sunflower, (0, 63, 0), &click(1, 0.5, 0.0), (5.5, 64.0, 5.5));

        assert!(matches!(refused, Placement::Refused(_)));
        assert_eq!(world.get_block(0, 64, 0), world::AIR);
    }

    /// Кровать ставится изножьем в выбранный блок, изголовьем — дальше от
    /// игрока, туда, куда он смотрит; ломается целиком с любой стороны.
    #[test]
    fn a_bed_takes_two_blocks_along_the_look() {
        let mut world = floor();
        let bed = blocks::orientation("red_bed").unwrap();
        let default = blocks::state_by_name("red_bed").unwrap();

        // Смотрит на восток (поворот 270).
        let placed = place(&mut world, "red_bed", default, (0, 63, 0), &click(1, 0.5, 270.0), (-2.5, 64.0, 0.5));

        assert_eq!(placed, Placement::Placed);

        let foot = world.get_block(0, 64, 0);
        let head = world.get_block(1, 64, 0);

        assert_eq!(bed.value(foot, "part"), Some("foot"));
        assert_eq!(bed.value(foot, "facing"), Some("east"));
        assert_eq!(bed.value(head, "part"), Some("head"));
        assert_eq!(bed.value(head, "facing"), Some("east"));

        assert_eq!(break_block(&mut world, 1, 64, 0), Some(head));
        assert_eq!(world.get_block(0, 64, 0), world::AIR, "изножье осталось");

        // Изголовью некуда встать — кровать не ставится.
        world.set_block_silently(1, 64, 0, 1);

        let refused = place(&mut world, "red_bed", default, (0, 63, 0), &click(1, 0.5, 270.0), (-2.5, 64.0, 0.5));

        assert!(matches!(refused, Placement::Refused(_)));
        assert_eq!(world.get_block(0, 64, 0), world::AIR);
    }

    /// У самого верха мира дверь не ставится: верхней половине там не
    /// место.
    #[test]
    fn a_door_does_not_fit_under_the_sky_limit() {
        let mut world = World::in_memory();
        let top = world::MIN_Y + world::WORLD_HEIGHT - 1;
        let door = blocks::state_by_name("oak_door").unwrap();

        world.set_block_silently(0, top - 1, 0, 1);

        let refused = place(&mut world, "oak_door", door, (0, top - 1, 0), &click(1, 0.5, 0.0), (5.5, 64.0, 5.5));

        assert!(matches!(refused, Placement::Refused(_)));
        assert_eq!(world.get_block(0, top, 0), world::AIR);
    }

    /// Плита не сливается в двойную, если в этой половине стоит игрок.
    #[test]
    fn a_double_slab_does_not_wall_the_player_in() {
        let mut world = floor();
        let bottom = blocks::orientation("oak_slab").unwrap().state(&[("type", "bottom")]).unwrap();

        world.set_block_silently(0, 64, 0, bottom);

        // Игрок стоит на нижней плите — в её верхней половине.
        let refused = place(&mut world, "oak_slab", bottom, (0, 64, 0), &click(1, 0.5, 0.0), (0.5, 64.5, 0.5));

        assert!(matches!(refused, Placement::Refused(_)));
        assert_eq!(world.get_block(0, 64, 0), bottom);

        // Отошёл — сливается.
        let merged = place(&mut world, "oak_slab", bottom, (0, 64, 0), &click(1, 0.5, 0.0), (3.5, 64.0, 0.5));
        assert_eq!(merged, Placement::Merged);
    }

    /// Поршень встаёт головой к игроку: смотришь на север — голова на юг,
    /// смотришь под ноги — голова вверх.
    #[test]
    fn a_piston_faces_the_player() {
        let facing = |yaw: f32, pitch: f32| {
            facing_the_player(&Click { face: 1, cursor_y: 0.5, yaw, pitch })
        };

        // Повороты: 0 — юг, 90 — запад, 180 — север, 270 — восток.
        assert_eq!(facing(0.0, 0.0), "north");
        assert_eq!(facing(180.0, 0.0), "south");
        assert_eq!(facing(90.0, 0.0), "east");
        assert_eq!(facing(270.0, 0.0), "west");

        // Взгляд под ноги и в небо.
        assert_eq!(facing(0.0, 89.0), "up");
        assert_eq!(facing(0.0, -89.0), "down");

        // Пологий взгляд вниз ещё не считается «под ноги».
        assert_eq!(facing(0.0, 20.0), "north");
    }
}
