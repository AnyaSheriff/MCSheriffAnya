// Элитры: полёт, износ раз в секунду, удар о стену, ускорение ракетой.
//
// Вики (Elytra): полёт начинается прыжком в воздухе (Player Command
// «start fall flying», 6) при надетых в нагрудный слот элитрах, у которых
// прочности больше 1; кончается на земле, в воде, на лестнице или когда
// элитры «сломаны»; «Durability decreases by one point each second when
// gliding» (с зачарованием «прочность» — как у инструментов); о стену:
// «damage = 10 × (change in horizontal velocity) − 3» (вид урона
// fly_into_wall); ракета в руке при полёте — разгон к 33,5 блока/с и не
// тратит прочность. Само движение считает клиент (сервер принимает
// пакеты места как есть), поэтому «проверки полёта» с выгоном тут нет.
// Что не сверено, помечено «[проверить]».

use super::*;

/// Номер действия Player Command: начать полёт на элитрах (minecraft-data
/// 26.1, protocol.json, entity_action).
const START_FALL_FLYING: i32 = 6;

/// Слот нагрудника в инвентаре игрока.
const CHEST_SLOT: i32 = 6;

/// Что игрок знает о своём полёте.
#[derive(Default)]
pub(super) struct Glide {
    pub active: bool,
    /// Тактов в полёте: от них считается износ и поза.
    pub ticks: u32,
    /// Горизонтальная скорость прошлого такта (для удара о стену).
    speed: f64,
    /// Осталось тактов разгона ракетой (длится полёт ракеты).
    boost: u32,
    /// Клиент сообщил об упоре в стену (бит 2 байта флагов пакета места).
    pub wall: bool,
}

/// Годятся ли элитры в нагрудном слоте: надеты и не «сломаны».
fn usable(player: &PlayerState) -> bool {
    player.inventory.slot(CHEST_SLOT).is_some_and(|stack| {
        crate::combat::item_name(stack.item) == Some("elytra") && crate::combat::durability_left(&stack).is_none_or(|left| left > 1)
    })
}

/// Player Command: 6 — начать полёт. Условия у оригинала: не на земле, не в
/// воде, не едет, нет левитации; иначе клиенту возвращается «не летит».
pub(super) fn command(payload: &[u8], player: &mut PlayerState, shared: &Shared) {
    let mut cursor = Cursor::new(payload);

    let (Some(_entity), Some(action)) = (cursor.varint(), cursor.varint()) else {
        return;
    };

    if action != START_FALL_FLYING || player.glide.active {
        return;
    }

    if player.on_ground || player.net.in_water || player.ride.vehicle.is_some() || player.vit.dead || !usable(player) {
        return;
    }

    player.glide = Glide { active: true, wall: player.glide.wall, ..Glide::default() };
    publish_player(shared, player);
}

/// Конец полёта: на землю, в воду, в лодку, без элитр.
fn stop(shared: &Shared, player: &mut PlayerState) {
    player.glide = Glide { wall: player.glide.wall, ..Glide::default() };
    publish_player(shared, player);
}

/// Такт полёта: условия, износ, удар о стену. Скорость по горизонтали —
/// сдвиг места за такт.
pub(super) fn tick(shared: &Shared, player: &mut PlayerState) {
    if !player.glide.active {
        return;
    }

    if player.on_ground || player.net.in_water || player.ride.vehicle.is_some() || !usable(player) {
        stop(shared, player);

        return;
    }

    player.glide.ticks += 1;

    // Износ: раз в секунду на единицу (вики, Elytra: «each second»).
    if player.glide.ticks.is_multiple_of(20)
        && let Some(stack) = player.inventory.slot(CHEST_SLOT)
        && player.game_mode != GAME_MODE_CREATIVE
    {
        pvp::wear_slot(shared, player, CHEST_SLOT, stack, 1, false);
    }

    // Удар о стену: скорость упала — урон 10 × потеря − 3 (вид fly_into_wall).
    let (dx, _, dz) = player.net.last_delta;
    let speed = dx.hypot(dz);
    let lost = player.glide.speed - speed;

    player.glide.speed = speed;

    if player.game_mode != GAME_MODE_CREATIVE && player.glide.wall && lost > 0.0 {
        let damage = (lost * 10.0 - 3.0) as f32;

        if damage > 0.0 {
            player.net.hits.push(vitals::Pending { amount: damage, source: crate::survival::Source::FlyIntoWall, direct: 0, hurt_yaw: None, killer: None, push: None, announced: false });
        }
    }
}

/// Ракета в руке при полёте: расходуется (в выживании), разгон длится
/// `flight` секунд полёта; прочность не тратится. Возвращает, была ли.
pub(super) fn use_rocket(shared: &Shared, player: &mut PlayerState, hand: i32) -> bool {
    let slot = if hand == 1 { inventory::OFFHAND } else { player.inventory.selected_slot() };
    let Some(stack) = player.inventory.slot(slot) else {
        return false;
    };

    if !player.glide.active || crate::combat::item_name(stack.item) != Some("firework_rocket") {
        return false;
    }

    let flight = stack.extras().rocket().flight.max(1) as u32;

    // [проверить] ракета-сущность, привязанная к игроку, не создаётся; разгон —
    // по правилу вики: к 1,5 блока/такт вдоль взгляда за время полёта.
    player.glide.boost = 10 * (flight + 1);

    if player.game_mode != GAME_MODE_CREATIVE {
        player.inventory.set(slot, inventory::remainder(stack, stack.count - 1));
        player.net.slot_changed(slot);
    }

    let _ = shared;

    true
}

/// Разгон ракетой: каждый такт скорость подтягивается к взгляду (вики,
/// Firework Rocket: прибавка 0,1 вдоль взгляда и половина разницы до 1,5).
/// Возвращает новую скорость, которую надо сообщить клиенту.
pub(super) fn boost(player: &mut PlayerState) -> Option<(f64, f64, f64)> {
    if !player.glide.active || player.glide.boost == 0 {
        return None;
    }

    player.glide.boost -= 1;

    let look = looking(player.yaw, player.pitch);
    let v = player.net.last_delta;

    Some((
        v.0 + look.0 * 0.1 + (look.0 * 1.5 - v.0) * 0.5,
        v.1 + look.1 * 0.1 + (look.1 * 1.5 - v.1) * 0.5,
        v.2 + look.2 * 0.1 + (look.2 * 1.5 - v.2) * 0.5,
    ))
}
