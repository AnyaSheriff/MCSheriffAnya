// Лук игрока: выстрел по отпущенной кнопке и рассказ клиентам о стрелах
// (сам полёт — arrow.rs).
//
// Вики, Bow: лук натягивается 20 тактов до полной силы; меньше 0,1 — выстрела
// нет; стрела берётся из инвентаря (вторая рука, панель, рюкзак), у
// творческого и с зачарованием «бесконечность» не тратится; лук
// изнашивается на 1. Что не сверено с оригиналом, помечено «[проверить]».

use super::*;

use crate::player_arrow::{Arrow, Pickup};
use crate::combat;

/// Пакеты: скорость сущности, сверка места, убрать сущность, звук.
const ENTITY_VELOCITY: i32 = 0x65;
const ENTITY_POSITION_SYNC: i32 = 0x23;

/// Что клиенту о стреле уже сказано.
#[derive(Clone, Copy, Debug)]
pub(super) struct ShownArrow {
    moves: u32,
    in_ground: bool,
    crit_sent: bool,
}

/// Первый слот с обычной стрелой: вторая рука, панель, рюкзак.
fn arrow_slot(player: &PlayerState) -> Option<i32> {
    let arrow = blocks::item_named("arrow")?;
    let order = std::iter::once(inventory::OFFHAND).chain(inventory::FIRST_HOTBAR..=inventory::LAST_HOTBAR).chain(inventory::FIRST_MAIN..=inventory::LAST_MAIN);

    order.into_iter().find(|slot| player.inventory.slot(*slot).is_some_and(|stack| stack.item == arrow))
}

/// Кнопку отпустили: лук был натянут `ticks` тактов в руке `hand`.
pub(super) fn release(shared: &Shared, player: &mut PlayerState, hand: i32, ticks: u32) {
    let power = combat::bow_power(ticks);

    if power < 0.1 {
        return;
    }

    let bow_slot = if hand == 1 { inventory::OFFHAND } else { player.inventory.selected_slot() };
    let Some(bow) = player.inventory.slot(bow_slot) else {
        return;
    };

    if combat::item_name(bow.item) != Some("bow") {
        return;
    }

    let data = bow.extras();
    let creative = player.game_mode == GAME_MODE_CREATIVE;
    let slot = arrow_slot(player);

    if slot.is_none() && !creative {
        return;
    }

    let infinity = data.level("infinity") > 0;
    let dimension = player.dimension;
    let roll = || vitals::world_random(shared, dimension);

    // Неточность: 0,0172275 (вики, Arrow) — к каждой оси направления.
    let wobble = || f64::from(roll() + roll() + roll() + roll() - 2.0) * 0.0172275 * 1.732;
    let (dx, dy, dz) = crate::player_arrow::aim(player.yaw, player.pitch, 1.0);
    let speed = combat::arrow_speed(power);
    let (last_x, last_y, last_z) = player.net.last_delta;
    // К скорости стрелы прибавляется скорость стрелка (по вертикали — только в воздухе).
    let velocity = (
        (dx + wobble()) * speed + last_x,
        (dy + wobble()) * speed + if player.on_ground { 0.0 } else { last_y },
        (dz + wobble()) * speed + last_z,
    );

    let owner = (player.entity_id, player.uuid, player.name.clone());
    let entity_id = shared.next_entity_id();
    let arrow = Arrow {
        entity_id,
        uuid: items::uuid_for(entity_id),
        dimension,
        x: player.x,
        y: player.y + if player.sneaking { 1.27 } else { 1.62 } - 0.1,
        z: player.z,
        vx: velocity.0,
        vy: velocity.1,
        vz: velocity.2,
        owner: owner.0,
        owner_uuid: owner.1,
        owner_name: owner.2,
        crit: power >= 1.0,
        power: data.level("power"),
        punch: data.level("punch"),
        flame: data.level("flame") > 0,
        pickup: if creative || infinity { Pickup::CreativeOnly } else { Pickup::Yes },
        in_ground: false,
        ground_ticks: 0,
        age: 0,
        moves: 0,
        landed: false,
        start: ((player.x, player.y + if player.sneaking { 1.27 } else { 1.62 } - 0.1, player.z), velocity),
    };

    shared.arrows.lock().expect("стрелы захвачены другим потоком").spawn(arrow);

    // Стрела тратится (кроме творческого и «бесконечности»).
    if let Some(slot) = slot
        && !creative
        && !infinity
        && let Some(stack) = player.inventory.slot(slot)
    {
        player.inventory.set(slot, inventory::remainder(stack, stack.count - 1));
        player.net.slot_changed(slot);
    }

    // Лук изнашивается на 1.
    if !creative {
        pvp::wear_slot(shared, player, bow_slot, bow, 1, false);
    }

    let pitch = 1.0 / (vitals::world_random(shared, dimension) * 0.4 + 1.2) + power * 0.5;

    shared
        .world_in(dimension)
        .lock()
        .expect("мир захвачен другим потоком")
        .note_sound_player_at((player.x, player.y, player.z), "minecraft:entity.arrow.shoot", 1.0, pitch);
}

/// Рассказывает клиенту о стрелах: появление, скорость, сверка места,
/// исчезновение.
pub(super) async fn send_arrow_ticks(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState) -> io::Result<()> {
    let arrows = shared.arrows.lock().expect("стрелы захвачены другим потоком").in_dimension(player.dimension);
    let range = f64::from(shared.properties.view_distance.clamp(2, 10)) * 16.0;

    let present: HashSet<i32> = arrows.iter().map(|arrow| arrow.entity_id).collect();
    let vanished: Vec<i32> = player.seen_bow_arrows.keys().filter(|id| !present.contains(id)).copied().collect();

    for id in vanished {
        player.seen_bow_arrows.remove(&id);
        send_remove_entity(stream, id).await?;
    }

    for arrow in &arrows {
        if (arrow.x - player.x).abs() > range || (arrow.z - player.z).abs() > range {
            continue;
        }

        let speed = (arrow.vx, arrow.vy, arrow.vz);
        let Some(mut shown) = player.seen_bow_arrows.get(&arrow.entity_id).copied() else {
            // Появление — с местом и скоростью выстрела, как их знал сервер в
            // миг выстрела (чёрный ящик, pvp-bow: место у стрелка).
            let ((sx, sy, sz), (svx, svy, svz)) = arrow.start;
            let (yaw, pitch) = crate::player_arrow::Arrow { vx: svx, vy: svy, vz: svz, ..arrow.clone() }.angles();
            let mut body = encode_varint(ADD_ENTITY);

            body.extend_from_slice(&encode_varint(arrow.entity_id));
            body.extend_from_slice(&arrow.uuid);
            body.extend_from_slice(&encode_varint(crate::player_arrow::ARROW_TYPE));
            push_f64(&mut body, sx);
            push_f64(&mut body, sy);
            push_f64(&mut body, sz);
            types::push_velocity(&mut body, svx, svy, svz);
            body.push(cart_angle(pitch));
            body.push(cart_angle(yaw));
            body.push(0);
            body.extend_from_slice(&encode_varint(arrow.owner));
            write_body(stream, body).await?;

            // Свойства не по умолчанию: полностью натянутая — с искрами.
            if arrow.crit {
                let mut data = encode_varint(SET_ENTITY_DATA);

                data.extend_from_slice(&encode_varint(arrow.entity_id));
                data.push(ARROW_FLAGS);
                data.extend_from_slice(&encode_varint(PROPERTY_BYTE));
                data.push(1);
                data.push(PROPERTIES_END);
                write_body(stream, data).await?;
            }

            player.seen_bow_arrows.insert(arrow.entity_id, ShownArrow { moves: 0, in_ground: false, crit_sent: arrow.crit });
            continue;
        };

        if arrow.moves != shown.moves || arrow.in_ground != shown.in_ground {
            if arrow.in_ground != shown.in_ground || arrow.in_ground {
                // Встала — точное место и нулевая скорость.
                send_position_sync(stream, arrow).await?;

                let mut data = encode_varint(SET_ENTITY_DATA);

                data.extend_from_slice(&encode_varint(arrow.entity_id));
                data.push(ARROW_IN_GROUND);
                data.extend_from_slice(&encode_varint(PROPERTY_BOOLEAN));
                data.push(u8::from(arrow.in_ground));
                data.push(PROPERTIES_END);
                write_body(stream, data).await?;
            } else {
                let mut body = encode_varint(ENTITY_VELOCITY);

                body.extend_from_slice(&encode_varint(arrow.entity_id));
                types::push_velocity(&mut body, speed.0, speed.1, speed.2);
                write_body(stream, body).await?;

                // Раз в 20 тактов — сверка места (у оригинала так у всех сущностей).
                if arrow.age % 20 == 0 {
                    send_position_sync(stream, arrow).await?;
                }
            }

            shown.moves = arrow.moves;
            shown.in_ground = arrow.in_ground;
        }

        shown.crit_sent = arrow.crit;
        player.seen_bow_arrows.insert(arrow.entity_id, shown);
    }

    Ok(())
}

/// Свойства стрелы: флаги (искры), «в блоке»; вид значения «логическое».
const ARROW_FLAGS: u8 = 8;
const ARROW_IN_GROUND: u8 = 10;
const PROPERTY_BOOLEAN: i32 = 8;

/// Сверка места и скорости стрелы (Entity Position Sync).
async fn send_position_sync(stream: &mut Outgoing<'_>, arrow: &Arrow) -> io::Result<()> {
    let (yaw, pitch) = arrow.angles();
    let mut body = encode_varint(ENTITY_POSITION_SYNC);

    body.extend_from_slice(&encode_varint(arrow.entity_id));

    for value in [arrow.x, arrow.y, arrow.z, arrow.vx, arrow.vy, arrow.vz] {
        push_f64(&mut body, value);
    }

    push_f32(&mut body, yaw);
    push_f32(&mut body, pitch);
    body.push(u8::from(arrow.in_ground));

    write_body(stream, body).await
}
