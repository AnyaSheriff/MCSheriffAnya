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
use crate::world::{self, World};

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

        // Правило не выяснено — состояние по умолчанию.
        Family::Other => None,
    }
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

/// Можно ли поставить блок прямо в это место, не ломая то, что там стоит.
///
/// Пустое место — можно. Воду и лаву блок вытесняет: в игре так и есть,
/// иначе под водой было бы не построить ничего.
///
/// Трава, снежный слой и прочее, что тоже вытесняется, сюда пока не входит —
/// этих блоков у нас ещё нет.
pub fn is_replaceable(state: i32) -> bool {
    state == world::AIR || fluids::fluid_at(state).is_some()
}

/// Дверь ли это.
///
/// Дверь — единственный блок, который занимает два блока по высоте, поэтому
/// и ставить, и ломать её приходится парой.
pub fn is_door(orientation: &Orientation) -> bool {
    orientation.family == Family::Door
}

/// Вторая половинка той же двери: у половинок одной двери имя блока совпадает.
///
/// Одной двери принадлежат только два её блока — верхний и нижний, — а имя
/// блока отличает дубовую дверь от еловой: у каждой пары оно своё.
pub fn is_same_door(orientation: &Orientation, state: i32) -> bool {
    blocks::orientation_of(state).is_some_and(|other| other.name == orientation.name)
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
        assert!(is_same_door(door, upper));
        assert!(!is_same_door(door, 1)); // камень

        let spruce = blocks::orientation("spruce_door").expect("правило для еловой двери");
        assert!(!is_same_door(spruce, upper));
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
