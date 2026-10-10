// Лодки и сидение: поставить лодку на воду, сесть, ехать (клиент ведёт лодку
// сам и шлёт Move Vehicle), слезть, показать седоков остальным.
//
// Порядок и числа сняты чёрным ящиком (boat-ride, boat-drive, boat-break):
// лодка встаёт в точку, куда упёрся луч от глаз (по воде — на её
// поверхность), предмет уходит; щелчок по лодке — Set Passengers и сверка
// места игрока (флаги 24: поворот относительный, как у `tp`); приседание —
// Set Passengers пустой и сверка в точку выхода; Move Vehicle ставит лодку
// туда, куда сказал водитель, и клиентам она уходит обычным сдвигом;
// Paddle Boat — свойства вёсел 11 и 12. Что не сверено, помечено
// «[проверить]».

use super::*;

use crate::minecart::{Kind, Minecart};
use crate::riding;

/// Serverbound: Move Vehicle (ведёт сам клиент), Paddle Boat.
pub(super) const VEHICLE_MOVE: i32 = 0x22;
pub(super) const STEER_BOAT: i32 = 0x23;

/// Clientbound Set Passengers.
const SET_PASSENGERS: i32 = 0x6B;

/// Свойства лодки: вёсла (minecraft-data 26.1, entities.json: paddle_left —
/// 11, paddle_right — 12, логические).
const PADDLE_LEFT: u8 = 11;
const PADDLE_RIGHT: u8 = 12;

/// Как далеко достаёт луч при постановке (вики, Boat: как у ведра).
const REACH: f64 = 5.0;

/// Что игрок знает о сидении: на чём едет и каких седоков клиенту уже
/// показано у каждого транспорта.
#[derive(Default)]
pub(super) struct Ride {
    pub vehicle: Option<i32>,
    seen: HashMap<i32, Vec<i32>>,
    version: u64,
}

/// Транспорты с седоками: номер и седоки по порядку.
type Seats = Vec<(i32, Vec<i32>)>;

/// Где транспорт: место, высота ящика, вид, поворот.
type Spot = ((f64, f64, f64), f64, f64, f32);

/// Куда ведёт луч от глаз: точка на поверхности воды или твёрдого блока.
/// Воздух и всё прочее пропускается; луч идёт мелкими шагами, последний
/// промежуток делится пополам до точности 1e-6.
fn ray_hit(world: &World, eye: (f64, f64, f64), direction: (f64, f64, f64), reach: f64) -> Option<(f64, f64, f64)> {
    // Внутри ли точка воды или твёрдого блока.
    let inside = |at: (f64, f64, f64)| {
        let (bx, by, bz) = (at.0.floor() as i32, at.1.floor() as i32, at.2.floor() as i32);
        let state = world.get_block(bx, by, bz);

        if let Some((fluids::Kind::Water, level)) = fluids::fluid_at(state) {
            let above = world.get_block(bx, by + 1, bz);
            let full = level >= 8 || fluids::fluid_at(above).is_some_and(|(kind, _)| kind == fluids::Kind::Water);
            let height = if full { 1.0 } else { f64::from(8 - level) / 9.0 };

            return at.1 - f64::from(by) < height;
        }

        blocks::has_box(state) && fluids::fluid_at(state).is_none()
    };
    let point = |t: f64| (eye.0 + direction.0 * t, eye.1 + direction.1 * t, eye.2 + direction.2 * t);
    let mut t = 0.0;

    while t <= reach {
        let next = t + 0.01;

        if inside(point(next)) {
            let (mut low, mut high) = (t, next);

            for _ in 0..40 {
                let mid = (low + high) / 2.0;

                if inside(point(mid)) {
                    high = mid;
                } else {
                    low = mid;
                }
            }

            return Some(point(high));
        }

        t = next;
    }

    None
}

/// Right-click лодкой в руке `hand`: ставится в точку луча. Возвращает, была
/// ли постановка.
pub(super) fn place_boat(shared: &Shared, player: &mut PlayerState, hand: i32) -> bool {
    let slot = if hand == 1 { inventory::OFFHAND } else { player.inventory.selected_slot() };
    let Some(stack) = player.inventory.slot(slot) else {
        return false;
    };
    let Some(kind) = blocks::item_name(stack.item).and_then(Kind::from_name).filter(|kind| kind.is_boat()) else {
        return false;
    };

    if player.game_mode == GAME_MODE_SPECTATOR || player.vit.dead {
        return false;
    }

    let eye = (player.x, player.y + if player.sneaking { 1.27 } else { 1.62 }, player.z);
    let direction = looking_table(player.yaw, player.pitch);
    let reach = if player.game_mode == GAME_MODE_CREATIVE { REACH } else { 4.5 };
    let hit = {
        let world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");

        ray_hit(&world, eye, direction, reach)
    };

    // Мимо воды и блоков лодка не ставится (вики, Boat: «placed on water or
    // on the ground»); взгляд мимо — ничего.
    let Some(at) = hit else {
        return true;
    };
    let mut boat = Minecart::new(shared.next_entity_id(), kind, at);

    player.progress.used(stack.item);
    boat.yaw = player.yaw;
    shared.items_in(player.dimension).lock().expect("предметы захвачены другим потоком").add_cart(boat);

    if player.game_mode != GAME_MODE_CREATIVE {
        player.inventory.set(slot, inventory::remainder(stack, stack.count - 1));
        player.net.slot_changed(slot);
    }

    true
}

/// Войдёт ли ящик игрока при шаге в `to` в лодку, в которой его не было.
pub(super) fn into_boat(shared: &Shared, player: &PlayerState, to: (f64, f64, f64)) -> bool {
    const HALF: f64 = 0.3;
    const EPSILON: f64 = 1.0e-7;

    let tall = if player.sneaking { 1.5 } else { 1.8 };
    let inside = |at: (f64, f64, f64), boat: (f64, f64, f64)| {
        let reach = crate::minecart::boat::WIDTH / 2.0 + HALF - EPSILON;

        (at.0 - boat.0).abs() < reach
            && (at.2 - boat.2).abs() < reach
            && at.1 < boat.1 + crate::minecart::boat::HEIGHT - EPSILON
            && at.1 + tall > boat.1 + EPSILON
    };
    let from = (player.x, player.y, player.z);

    shared
        .items_in(player.dimension)
        .lock()
        .expect("предметы захвачены другим потоком")
        .carts()
        .iter()
        .filter(|cart| cart.kind.is_boat())
        .map(|cart| (cart.x, cart.y, cart.z))
        .any(|boat| inside(to, boat) && !inside(from, boat))
}

/// Сколько мест у транспорта-лодки или вагонетки; None — не оседлать.
fn seats_of(kind: Kind) -> Option<usize> {
    match kind {
        Kind::Boat(_) => Some(crate::minecart::BOAT_SEATS),
        // Пустая вагонетка везёт одного; с грузом, печью или динамитом — нет
        // (вики, Minecart: «Only a normal minecart can carry passengers»).
        Kind::Plain => Some(1),
        _ => None,
    }
}

/// Щелчок по лодке или вагонетке: сесть. Возвращает, было ли это ездовое.
pub(super) fn mount(shared: &Shared, player: &mut PlayerState, target: i32) -> bool {
    /// Дальше этого щёлкнуть нельзя (как у удара).
    const REACH_USE: f64 = 6.0;

    if player.sneaking || player.ride.vehicle.is_some() || player.vit.dead {
        return false;
    }

    let mut items = shared.items_in(player.dimension).lock().expect("предметы захвачены другим потоком");
    let Some(cart) = items.carts_mut().iter_mut().find(|cart| cart.entity_id == target) else {
        return false;
    };
    let Some(seats) = seats_of(cart.kind) else {
        return false;
    };

    if (cart.x - player.x).powi(2) + (cart.y - player.y).powi(2) + (cart.z - player.z).powi(2) > REACH_USE * REACH_USE {
        return true;
    }

    let mut riding = shared.riding.lock().expect("учёт седоков захвачен другим потоком");

    if riding.mount(player.entity_id, target, seats) {
        // Водитель лодки — первый севший (у оригинала: вёсла у него).
        if cart.kind.is_boat() && riding.driver(target) == Some(player.entity_id) {
            cart.driver = Some(player.entity_id);
        }
    }

    true
}

/// Приседание в лодке: слезть.
pub(super) fn dismount(shared: &Shared, player: &PlayerState) {
    let Some(vehicle) = shared.riding.lock().expect("учёт седоков захвачен другим потоком").dismount(player.entity_id) else {
        return;
    };

    release_driver(shared, player.dimension, vehicle, player.entity_id);
}

/// Седок слез: у лодки без водителя вёсла опускаются, а ход становится
/// серверным.
fn release_driver(shared: &Shared, dimension: Dimension, vehicle: i32, rider: i32) {
    let next = shared.riding.lock().expect("учёт седоков захвачен другим потоком").driver(vehicle);
    let mut items = shared.items_in(dimension).lock().expect("предметы захвачены другим потоком");
    if let Some(mob) = items.mobs_mut().iter_mut().find(|m| m.entity_id == vehicle) { mob.land.rider = next; }

    if let Some(cart) = items.carts_mut().iter_mut().find(|cart| cart.entity_id == vehicle)
        && cart.driver == Some(rider)
    {
        cart.driver = next;
        cart.set_paddle((false, false));
    }
}

/// Move Vehicle: водитель лодки сообщает её новое место.
pub(super) fn vehicle_move(payload: &[u8], player: &mut PlayerState, shared: &Shared) {
    let Some(vehicle) = player.ride.vehicle else {
        return;
    };
    let (Some((x, y, z)), Some((yaw, pitch))) = (read_position(payload), read_rotation(payload, 24)) else {
        return;
    };

    // Как у пакетов места: не число и безумные координаты не принимаются.
    let sane = |value: f64, limit: f64| value.is_finite() && value.abs() <= limit;

    if !(sane(x, 30_000_000.0) && sane(y, 20_000_000.0) && sane(z, 30_000_000.0)) || !(yaw.is_finite() && pitch.is_finite()) {
        return;
    }

    let mut items = shared.items_in(player.dimension).lock().expect("предметы захвачены другим потоком");
    if let Some(mob) = items.mobs_mut().iter_mut().find(|m| m.entity_id == vehicle && m.kind == crate::mob::Kind::Camel) {
        if mob.land.rider == Some(player.entity_id) && mob.land.saddle && !mob.land.sitting && !mob.is_dead()
            && (x-mob.x).hypot(z-mob.z) <= 5.0 && (y-mob.y).abs() <= 5.0 {
            mob.x=x; mob.y=y; mob.z=z; mob.face(yaw);
        }
        return;
    }
    let Some(cart) = items.carts_mut().iter_mut().find(|cart| cart.entity_id == vehicle) else {
        return;
    };

    // Ведёт тот, у кого вёсла: чужой пакет (второй седок) не принимается.
    if cart.driver != Some(player.entity_id) {
        return;
    }

    let cm = ((x - cart.x).hypot(y - cart.y).hypot(z - cart.z) * 100.0).round() as i32;
    player.progress.custom(if cart.kind.is_boat() { "boat_one_cm" } else { "minecart_one_cm" }, cm);
    cart.drive_to((x, y, z), yaw);
    let seat = riding::boat_seat((x,y,z));
    player.set_position(seat.0,seat.1,seat.2);
}

/// Paddle Boat: левое и правое весло.
pub(super) fn steer_boat(payload: &[u8], player: &PlayerState, shared: &Shared) {
    let (Some(left), Some(right)) = (payload.first(), payload.get(1)) else {
        return;
    };
    let Some(vehicle) = player.ride.vehicle else {
        return;
    };
    let mut items = shared.items_in(player.dimension).lock().expect("предметы захвачены другим потоком");

    if let Some(cart) = items.carts_mut().iter_mut().find(|cart| cart.entity_id == vehicle)
        && cart.driver == Some(player.entity_id)
    {
        cart.set_paddle((*left != 0, *right != 0));
    }
}

/// Свойства вёсел: уходят вместе со свойствами удара (send_cart_data).
pub(super) fn push_paddles(data: &mut Vec<u8>, (left, right): (bool, bool)) {
    data.push(PADDLE_LEFT);
    data.extend_from_slice(&encode_varint(PROPERTY_BOOLEAN_KIND));
    data.push(u8::from(left));
    data.push(PADDLE_RIGHT);
    data.extend_from_slice(&encode_varint(PROPERTY_BOOLEAN_KIND));
    data.push(u8::from(right));
}

/// Вид значения «логическое» в свойствах (protocol.json: 8).
const PROPERTY_BOOLEAN_KIND: i32 = 8;

/// Место выхода: с лодки в `at` (низ ящика) игрок, смотрящий по `yaw`,
/// ставится на сушу рядом, если есть, иначе — на верх лодки (чёрный ящик,
/// boat-drive: лодка на воде, выход в (3,5; 1,962; 3) — верх ящика
/// 1,4 + 0,5625; boat-ride — (3,5; 2,018; 1,953)). Рядом ищется по сторонам: вперёд, вбок, назад
/// [проверить: порядок и расстояние у оригинала].
fn exit_place(world: &World, at: (f64, f64, f64), height: f64, yaw: f32) -> (f64, f64, f64) {
    let (sin, cos) = f64::from(yaw).to_radians().sin_cos();
    let forward = (-sin, cos);
    let reach = 1.0;
    let tries = [forward, (cos, sin), (-cos, -sin), (sin, -cos)];

    for (dx, dz) in tries {
        let (x, z) = (at.0 + dx * reach, at.2 + dz * reach);
        let (bx, bz) = (x.floor() as i32, z.floor() as i32);

        // Ступить можно в клетку лодки или ниже, но не на уступ выше неё
        // (boat-ride: лодка в бассейне, рядом каменный бортик в её клетке —
        // оригинал ставит не на бортик, а на верх лодки).
        for by in [at.1.floor() as i32, at.1.floor() as i32 - 1] {
            let below = world.get_block(bx, by - 1, bz);
            let feet = world.get_block(bx, by, bz);
            let head = world.get_block(bx, by + 1, bz);

            if blocks::has_box(below)
                && fluids::fluid_at(below).is_none()
                && !blocks::has_box(feet)
                && !blocks::has_box(head)
                && fluids::fluid_at(feet).is_none()
                && fluids::fluid_at(head).is_none()
            {
                return (x, f64::from(by), z);
            }
        }
    }

    (at.0, at.1 + height, at.2)
}

/// Что вызывающему нужно знать о сидении: каждый круг цикла, перед
/// рассылкой остальных. Приводит собственное сидение игрока в порядок
/// (сел — сверка места на сиденье; слез или лодку сломали — сверка на
/// место выхода), переносит седока за транспортом и пересказывает клиенту
/// Set Passengers по тем транспортам, что он видит.
pub(super) async fn send_riding(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState) -> io::Result<()> {
    let (version, mine, lists): (u64, Option<i32>, Seats) = {
        let riding = shared.riding.lock().expect("учёт седоков захвачен другим потоком");

        (riding.version(), riding.vehicle_of(player.entity_id), riding.vehicles().map(|(vehicle, seats)| (vehicle, seats.to_vec())).collect())
    };

    // Транспорт под седоком: где он (лодки и вагонетки — из предметов).
    let place_of = |vehicle: i32| -> Option<Spot> {
        let cart_spot = shared
            .items_in(player.dimension)
            .lock()
            .expect("предметы захвачены другим потоком")
            .carts()
            .iter()
            .find(|cart| cart.entity_id == vehicle)
            .map(|cart| ((cart.x, cart.y, cart.z), cart.dims().1, riding::BOAT_SEAT_DROP, cart.yaw));
        cart_spot.or_else(|| shared.items_in(player.dimension).lock().expect("мобы").mobs().iter().find(|m| m.entity_id == vehicle && !m.is_dead()).map(|m| ((m.x,m.y,m.z),m.size().1,if m.kind == crate::mob::Kind::Camel { if m.land.sitting { 0.8 } else { 1.85 } } else { 1.37 },m.yaw)))
    };

    if mine != player.ride.vehicle {
        match mine {
            Some(vehicle) => {
                if let Some((at, _, offset, _)) = place_of(vehicle) {
                    let seat = (at.0, at.1+offset, at.2);

                    player.ride.vehicle = Some(vehicle);
                    player.set_position(seat.0, seat.1, seat.2);
                    publish_player(shared, player);
                    send_player_position_as(stream, player, Sync::Command).await?;
                } else {
                    // Транспорта уже нет: сесть не вышло.
                    shared.riding.lock().expect("учёт седоков захвачен другим потоком").dismount(player.entity_id);
                }
            }
            None => {
                let old = player.ride.vehicle.take();
                let spot = old.and_then(place_of);
                let exit = match spot {
                    Some((at, height, _, _)) => {
                        let world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");

                        exit_place(&world, at, height, player.yaw)
                    }
                    // Лодку сломали: игрок остаётся там, где сидел.
                    None => (player.x, player.y, player.z),
                };

                player.set_position(exit.0, exit.1, exit.2);
                publish_player(shared, player);
                send_player_position_as(stream, player, Sync::Command).await?;
            }
        }
    } else if let Some(vehicle) = player.ride.vehicle
        && let Some((at, _, offset, _)) = place_of(vehicle)
    {
        // Едет: за транспортом (вагонетка катится сама, лодку ведёт он).
        let seat = (at.0, at.1+offset, at.2);

        if (seat.0 - player.x).abs() + (seat.1 - player.y).abs() + (seat.2 - player.z).abs() > 1.0e-9 {
            // Статистика пути — только у лодок и вагонеток, не у верховых мобов.
            let kind = shared.items_in(player.dimension).lock().expect("предметы захвачены другим потоком").carts().iter().find(|cart| cart.entity_id == vehicle).map(|cart| cart.kind);

            if let Some(kind) = kind {
                player.progress.custom(if kind.is_boat() { "boat_one_cm" } else { "minecart_one_cm" }, ((seat.0 - player.x).hypot(seat.1 - player.y).hypot(seat.2 - player.z) * 100.0).round() as i32);
            }

            player.set_position(seat.0, seat.1, seat.2);
            publish_player(shared, player);
        }
    }

    if version == player.ride.version && player.ride.seen.is_empty() && lists.is_empty() {
        return Ok(());
    }

    player.ride.version = version;

    // Транспорты, о которых клиент знает: показанные вагонетки и мобы.
    let known = |player: &PlayerState, vehicle: i32| player.seen_carts.contains_key(&vehicle) || player.seen_mobs.contains_key(&vehicle) || player.ride.vehicle == Some(vehicle);
    let now: HashMap<i32, Vec<i32>> = lists.into_iter().filter(|(vehicle, _)| known(player, *vehicle)).collect();
    let mut changed: Vec<i32> = Vec::new();

    for (vehicle, seats) in &now {
        if player.ride.seen.get(vehicle) != Some(seats) {
            changed.push(*vehicle);
        }
    }

    for vehicle in player.ride.seen.keys() {
        if !now.contains_key(vehicle) {
            changed.push(*vehicle);
        }
    }

    changed.sort_unstable();

    for vehicle in changed {
        let seats = now.get(&vehicle).cloned().unwrap_or_default();
        let mut body = encode_varint(SET_PASSENGERS);

        body.extend_from_slice(&encode_varint(vehicle));
        body.extend_from_slice(&encode_varint(seats.len() as i32));

        for rider in &seats {
            body.extend_from_slice(&encode_varint(*rider));
        }

        write_body(stream, body).await?;

        if seats.is_empty() {
            player.ride.seen.remove(&vehicle);
        } else {
            player.ride.seen.insert(vehicle, seats);
        }
    }

    Ok(())
}

// 26.1 player_command: start_riding_jump=3, stop_riding_jump=4.
pub(super) fn mount_command(payload: &[u8], player: &PlayerState, shared: &Shared) {
    let mut c=Cursor::new(payload);
    let (Some(_),Some(action),Some(charge))=(c.varint(),c.varint(),c.varint()) else { return; };
    if action!=3 { return; }
    let Some(id)=player.ride.vehicle else { return; };
    let mut items=shared.items_in(player.dimension).lock().expect("мобы");
    if let Some(mob)=items.mobs_mut().iter_mut().find(|m| m.entity_id==id && m.land.rider==Some(player.entity_id)) { crate::mob::land::dash(mob,charge); }
}

// Первое нажатие движения поднимает сидящего верблюда (вики, Camel).
pub(super) fn input(flags:u8, player:&PlayerState, shared:&Shared) {
    if flags & 15 == 0 { return; }
    let Some(id)=player.ride.vehicle else { return; };
    let mut items=shared.items_in(player.dimension).lock().expect("мобы");
    if let Some(m)=items.mobs_mut().iter_mut().find(|m| m.entity_id==id && m.kind==crate::mob::Kind::Camel && m.land.rider==Some(player.entity_id) && m.land.saddle && m.land.sitting) {
        m.land.sitting=false; m.land.pose_tick=m.tame.clock.abs();
    }
}
