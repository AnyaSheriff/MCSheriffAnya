// Лук игрока: выстрел по отпущенной кнопке и рассказ клиентам о стрелах
// (сам полёт — arrow.rs).
//
// Вики, Bow: лук натягивается 20 тактов до полной силы; меньше 0,1 — выстрела
// нет; стрела берётся из инвентаря (вторая рука, панель, рюкзак), у
// творческого и с зачарованием «бесконечность» не тратится; лук
// изнашивается на 1. Что не сверено с оригиналом, помечено «[проверить]».

use super::*;

use crate::arrow::{Arrow, Pickup, Shooter};
use crate::combat;

/// Пакеты: скорость сущности, сверка места, звук от сущности.
const ENTITY_SOUND: i32 = 0x74;
/// Пакеты: скорость сущности, сверка места, убрать сущность, звук.
const ENTITY_VELOCITY: i32 = 0x65;
const ENTITY_POSITION_SYNC: i32 = 0x23;

/// Что клиенту о стреле уже сказано.
#[derive(Clone, Copy, Debug)]
pub(super) struct ShownArrow {
    in_ground: bool,
    /// Нулевой сдвиг ещё не отправлен (стрела скелета в такт выстрела).
    first: bool,
    /// Место, которое клиент знает (сетка 1/4096): от него считается сдвиг.
    grid: (i64, i64, i64),
    /// Поворот при появлении (по скорости выстрела).
    yaw: u8,
    pitch: u8,
    /// Флаги (свойство 8), которые клиент знает.
    flags: u8,
    /// Возраст последнего сообщённого перелома полёта: подключение, отставшее
    /// на такт-другой, всё равно его пошлёт.
    resynced: u32,
}

/// Первый слот с обычной стрелой: вторая рука, панель, рюкзак.
pub(super) fn arrow_slot(player: &PlayerState) -> Option<i32> {
    arrow_slot_matching(player, true)
}

fn arrow_slot_matching(player: &PlayerState, tipped: bool) -> Option<i32> {
    let order = std::iter::once(inventory::OFFHAND).chain(inventory::FIRST_HOTBAR..=inventory::LAST_HOTBAR).chain(inventory::FIRST_MAIN..=inventory::LAST_MAIN);

    order.into_iter().find(|slot| player.inventory.slot(*slot).is_some_and(|stack| matches!(blocks::item_name(stack.item), Some("arrow")) || (tipped && blocks::item_name(stack.item) == Some("tipped_arrow"))))
}

/// Чем заряжать арбалет в руке `hand`: сперва то, что в другой руке
/// (стрела или ракета), потом первая стрела в инвентаре (вики, Crossbow:
/// «Arrows in the off-hand, or the selected hotbar slot if the crossbow is
/// in the off-hand, are prioritized first»; ракета — «must be present in
/// the player's offhand»). Ячейка и ракета ли это.
pub(super) fn crossbow_ammo(player: &PlayerState, hand: i32) -> Option<(i32, bool)> {
    let other = if hand == 1 { player.inventory.selected_slot() } else { inventory::OFFHAND };

    match player.inventory.slot(other).and_then(|stack| combat::item_name(stack.item)) {
        Some("arrow") => Some((other, false)),
        Some("firework_rocket") => Some((other, true)),
        _ => arrow_slot_matching(player, false).map(|slot| (slot, false)),
    }
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

    let ammo = slot.and_then(|slot| player.inventory.slot(slot));
    let potion = ammo.filter(|stack| blocks::item_name(stack.item) == Some("tipped_arrow")).and_then(|stack| stack.extras().potion);
    let infinity = data.level("infinity") > 0 && ammo.is_none_or(|stack| blocks::item_name(stack.item) == Some("arrow"));
    let shot = Shot {
        potion,
        speed: combat::arrow_speed(power),
        crit: power >= 1.0,
        power: data.level("power"),
        punch: data.level("punch"),
        flame: data.level("flame") > 0,
        pierce: 0,
        pickup: if creative || infinity { Pickup::CreativeOnly } else { Pickup::Yes },
        crossbow: false,
        trident: None,
        loyalty: 0,
    };

    shoot(shared, player, &shot, 0.0);
    player.progress.used(bow.item);

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

    let dimension = player.dimension;
    let pitch = 1.0 / (vitals::world_random(shared, dimension) * 0.4 + 1.2) + power * 0.5;

    shared
        .world_in(dimension)
        .lock()
        .expect("мир захвачен другим потоком")
        .note_sound_player_at((player.x, player.y, player.z), "minecraft:entity.arrow.shoot", 1.0, pitch);
}

/// Чем и как стреляют: скорость, свойства стрелы.
#[derive(Clone, Copy)]
pub(super) struct Shot {
    pub potion: Option<u16>,
    pub speed: f64,
    pub crit: bool,
    pub power: i32,
    pub punch: i32,
    pub flame: bool,
    /// Сколько сущностей стрела ещё пробьёт («пробивание»).
    pub pierce: i32,
    pub pickup: Pickup,
    /// Из арбалета: свой звук попадания.
    pub crossbow: bool,
    /// Брошенный трезубец (сам предмет) и его «верность».
    pub trident: Option<Stack>,
    pub loyalty: i32,
}

/// Выпускает стрелу от глаз игрока по взгляду, повёрнутому на `yaw_offset`
/// градусов (боковые стрелы «залпа»).
fn shoot(shared: &Shared, player: &PlayerState, shot: &Shot, yaw_offset: f32) {
    let dimension = player.dimension;
    let roll = || vitals::world_random(shared, dimension);

    // Неточность: к каждой оси направления — треугольная добавка в пределах
    // ±0,0172275 (вики, Arrow), разность двух равномерных (чёрный ящик: у
    // оригинала все 17 выстрелов луком, арбалетом и трезубцем в пределах
    // ±0,0163 по оси при длине 1).
    let wobble = || f64::from(roll() - roll()) * 0.0172275;
    let (dx, dy, dz) = crate::arrow::aim(player.yaw + yaw_offset, player.pitch, 1.0);
    let (last_x, last_y, last_z) = player.net.last_delta;
    // К скорости стрелы прибавляется скорость стрелка (по вертикали — только в воздухе).
    let velocity = (
        (dx + wobble()) * shot.speed + last_x,
        (dy + wobble()) * shot.speed + if player.on_ground { 0.0 } else { last_y },
        (dz + wobble()) * shot.speed + last_z,
    );
    let eye = player.y + if player.sneaking { 1.27 } else { 1.62 } - 0.1;
    let entity_id = shared.next_entity_id();
    let arrow = Arrow {
        entity_id,
        uuid: items::uuid_for(entity_id),
        dimension,
        x: player.x,
        y: eye,
        z: player.z,
        vx: velocity.0,
        vy: velocity.1,
        vz: velocity.2,
        owner: player.entity_id,
        owner_uuid: player.uuid,
        owner_name: player.name.clone(),
        shooter: Shooter::Player,
        effects: crate::arrow::tipped_effects(shot.potion),
        potion: shot.potion,
        crit: shot.crit,
        power: shot.power,
        punch: shot.punch,
        flame: shot.flame,
        pierce_left: shot.pierce,
        pierced: Vec::new(),
            killed: Vec::new(),
        crossbow: shot.crossbow,
        trident: shot.trident,
        loyalty: shot.loyalty,
        dealt: false,
        returning: false,
        collected: false,
        impulse: false,
        resync: None,
        // Трезубец поворачивается сам (догоняет полёт), стрела — по скорости.
        rot: shot.trident.map(|_| crate::arrow::angles_of(velocity)),
        ground_block: (0, 0, 0),
        pickup: shot.pickup,
        in_ground: false,
        ground_ticks: 0,
        age: 0,
        fresh: false,
        landed: false,
        start: ((player.x, eye, player.z), velocity),
    };

    shared.arrows.lock().expect("стрелы захвачены другим потоком").spawn(arrow);
}

/// Трезубец в руке `hand` занесён `ticks` тактов и отпущен: бросок, если
/// держали не меньше 10 тактов (вики, Trident: «0.5s»). Летит 2,5 блока за
/// такт, изнашивается на 1 ещё в руке и уходит из инвентаря (у творческого
/// остаётся) (вики, Trident).
pub(super) fn throw_trident(shared: &Shared, player: &mut PlayerState, hand: i32, ticks: u32) {
    if ticks < 10 {
        return;
    }

    let slot = if hand == 1 { inventory::OFFHAND } else { player.inventory.selected_slot() };
    let Some(trident) = player.inventory.slot(slot) else {
        return;
    };

    if combat::item_name(trident.item) != Some("trident") {
        return;
    }

    let creative = player.game_mode == GAME_MODE_CREATIVE;
    let data = trident.extras();

    // «Тягун»: трезубец не летит, игрока вертит (`vitals::tick_spin`), сам
    // рывок считает клиент (вики, Riptide).
    if data.level("riptide") > 0 {
        riptide(shared, player, slot, trident, creative, data.level("riptide"));
        return;
    }

    let dimension = player.dimension;
    let thrown = if creative {
        trident
    } else {
        let mut roll = || vitals::world_random(shared, dimension);
        let worn = combat::wear(&trident, 1, false, &mut roll);

        match combat::add_damage(trident, worn) {
            Some(next) => next,
            None => return,
        }
    };
    let shot = Shot {
        potion: None,
        speed: crate::arrow::TRIDENT_SPEED,
        crit: false,
        power: 0,
        punch: 0,
        flame: false,
        pierce: 0,
        pickup: if creative { Pickup::CreativeOnly } else { Pickup::Yes },
        crossbow: false,
        trident: Some(thrown.with_count(1)),
        loyalty: data.level("loyalty"),
    };

    shoot(shared, player, &shot, 0.0);
    player.progress.used(trident.item);

    // Звук броска шлёт слежение за трезубцем (`send_arrow_ticks`).
    if !creative {
        player.inventory.set(slot, None);
        player.net.slot_changed(slot);
    }
}

/// Бросок с «тягуном» уровня `level`: износ 1, звук riptide_1…3 от самого
/// игрока, вращение на 20 тактов (вики, Riptide; чёрный ящик,
/// trident-riptide: на +0 звук, 8=4, ячейка и экипировка с износом).
fn riptide(shared: &Shared, player: &mut PlayerState, slot: i32, trident: Stack, creative: bool, level: i32) {
    player.progress.used(trident.item);
    let dimension = player.dimension;

    if !creative {
        let mut roll = || vitals::world_random(shared, dimension);
        let worn = combat::wear(&trident, 1, false, &mut roll);

        match combat::add_damage(trident, worn) {
            Some(next) => player.inventory.set(slot, Some(next)),
            None => pvp::replace_worn(shared, player, slot, trident, worn),
        }

        player.net.slot_changed(slot);
    }

    let sound = match level {
        1 => "minecraft:item.trident.riptide_1",
        2 => "minecraft:item.trident.riptide_2",
        _ => "minecraft:item.trident.riptide_3",
    };

    shared.world_in(dimension).lock().expect("мир захвачен другим потоком").note_entity_sound(player.entity_id, sound, 1.0, 1.0);
    player.net.spin_since = Some(0);
    player.net.spin_pose = false;
    player.net.spin_from = (player.x, player.y, player.z);
    super::publish_player(shared, player);
}

/// Можно ли метнуться «тягуном»: игрок в воде (ноги или голова) — «only
/// when a player stands in water or when exposed to rain» (вики, Riptide).
/// Дождь — World::in_rain (weather.rs).
pub(super) fn riptide_allowed(shared: &Shared, player: &PlayerState) -> bool {
    let world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");
    let (x, z) = (player.x.floor() as i32, player.z.floor() as i32);

    [player.y, player.y + 1.0].iter().any(|y| crate::mob::is_water(world.get_block(x, y.floor() as i32, z)))
        || world.in_rain(player.x, player.y, player.z, 1.8)
}

/// Арбалет в руке `hand` натянут до конца: заряжается стрелой из инвентаря
/// (вики, Crossbow).
pub(super) fn crossbow_load(player: &mut PlayerState, hand: i32) {
    let slot = if hand == 1 { inventory::OFFHAND } else { player.inventory.selected_slot() };
    let Some(crossbow) = player.inventory.slot(slot) else {
        return;
    };

    if combat::item_name(crossbow.item) != Some("crossbow") {
        return;
    }

    let creative = player.game_mode == GAME_MODE_CREATIVE;
    let ammo = crossbow_ammo(player, hand);

    if ammo.is_none() && !creative {
        return;
    }

    let mut data = crossbow.extras();

    // С «залпом» в арбалете три стрелы, но из инвентаря берётся одна
    // (вики, Multishot: «consumes only one arrow»); ракета ложится со своими
    // довесками (полёт, взрывы).
    data.charged = if data.level("multishot") > 0 { 3 } else { 1 };
    data.charged_rocket = ammo.filter(|(_, rocket)| *rocket).and_then(|(ammo_slot, _)| player.inventory.slot(ammo_slot)).map(|stack| stack.extras().rocket());
    player.inventory.set(slot, Some(crossbow.with_extras(data)));
    player.net.slot_changed_later(slot);

    if let Some((arrow_slot, _)) = ammo
        && !creative
        && let Some(stack) = player.inventory.slot(arrow_slot)
    {
        player.inventory.set(arrow_slot, inventory::remainder(stack, stack.count - 1));
        player.net.slot_changed_later(arrow_slot);
    }
}

/// Заряженный арбалет в руке `hand` нажат: выстрел. Возвращает, был ли он.
/// Стрела 3,15 блока за такт, критическая; «залп» — три стрелы по ±10°,
/// боковые не подбираются; износ 1 за стрелу (вики, Crossbow, Multishot).
pub(super) fn crossbow_fire(shared: &Shared, player: &mut PlayerState, hand: i32) -> bool {
    let slot = if hand == 1 { inventory::OFFHAND } else { player.inventory.selected_slot() };
    let Some(crossbow) = player.inventory.slot(slot) else {
        return false;
    };

    if combat::item_name(crossbow.item) != Some("crossbow") {
        return false;
    }

    let mut data = crossbow.extras();

    if data.charged == 0 {
        return false;
    }

    let creative = player.game_mode == GAME_MODE_CREATIVE;
    let count = data.charged;
    player.progress.used(crossbow.item);
    player.progress.trigger("adventure/ol_betsy");

    if let Some(rocket) = data.charged_rocket.take() {
        fire_rockets(shared, player, &rocket, count);
        data.charged = 0;

        // Ракета изнашивает арбалет на 3, «залп» — на 9 (вики, Crossbow:
        // «uses 3 durability per shot… 9 durability… with each shot»).
        let unloaded = crossbow.with_extras(data);

        if creative {
            player.inventory.set(slot, Some(unloaded));
        } else {
            let dimension = player.dimension;
            let mut roll = || vitals::world_random(shared, dimension);
            let worn = combat::wear(&unloaded, 3 * i32::from(count), false, &mut roll);

            match combat::add_damage(unloaded, worn) {
                Some(next) => player.inventory.set(slot, Some(next)),
                None => pvp::replace_worn(shared, player, slot, unloaded, worn),
            }
        }

        return true;
    }

    let shot = Shot {
        potion: None,
        speed: combat::CROSSBOW_SPEED,
        crit: true,
        power: 0,
        punch: 0,
        flame: false,
        pierce: data.level("piercing"),
        pickup: if creative { Pickup::CreativeOnly } else { Pickup::Yes },
        crossbow: true,
        trident: None,
        loyalty: 0,
    };
    // Порядок: средняя, левая, правая — повороты стрел при появлении у
    // оригинала 0, −10, +10 (чёрный ящик, crossbow-multishot: yaw −1, −8, 7),
    // а поворот стрелы — взгляд с обратным знаком.
    let offsets: &[f32] = if count >= 3 { &[0.0, combat::MULTISHOT_ANGLE, -combat::MULTISHOT_ANGLE] } else { &[0.0] };
    let dimension = player.dimension;

    // На каждую стрелу — звук выстрела, потом сама стрела (чёрный ящик,
    // crossbow-multishot: звук, сущность, звук, сущность…); высота случайна.
    for (index, offset) in offsets.iter().enumerate() {
        let side = Shot { pickup: if index == 0 { shot.pickup } else { Pickup::CreativeOnly }, crossbow: true, ..shot };
        // Средняя звучит ровно (у оригинала ~1,00 во всех записях), боковые —
        // со случайной высотой (1,16 и 0,90 в crossbow-multishot) [проверить].
        let pitch = if index == 0 { 1.0 } else { 1.0 / (vitals::world_random(shared, dimension) * 0.5 + 1.8) + if index == 1 { 0.63 } else { 0.43 } };

        shared
            .world_in(dimension)
            .lock()
            .expect("мир захвачен другим потоком")
            .note_sound_player_at((player.x, player.y, player.z), "minecraft:item.crossbow.shoot", 1.0, pitch);
        shoot(shared, player, &side, *offset);
    }

    // Стрелявшему ячейку не шлют: выстрел его клиент показывает сам (чёрный
    // ящик, crossbow-shoot: у оригинала после выстрела Set Slot нет), соседям
    // уходит экипировка с износом.
    data.charged = 0;

    let unloaded = crossbow.with_extras(data);

    // Износ 1 за каждую стрелу (чёрный ящик, crossbow-multishot: damage=3).
    if creative {
        player.inventory.set(slot, Some(unloaded));
    } else {
        let mut roll = || vitals::world_random(shared, dimension);
        let worn = combat::wear(&unloaded, i32::from(count), false, &mut roll);

        match combat::add_damage(unloaded, worn) {
            Some(next) => player.inventory.set(slot, Some(next)),
            None => pvp::replace_worn(shared, player, slot, unloaded, worn),
        }
    }

    true
}

/// Выстрел ракетами из арбалета: на каждую — звук выстрела и ракета от глаз
/// (−0,15) по взгляду (у «залпа» ±10°), 1,6 блока за такт с неточностью
/// стрелы, летит прямо (вики, Crossbow, Firework Rocket) [проверить место
/// появления и высоту звука].
fn fire_rockets(shared: &Shared, player: &PlayerState, rocket: &crate::stackdata::Fireworks, count: u8) {
    let Some(item) = blocks::item_named("firework_rocket") else {
        return;
    };
    let mut data = crate::stackdata::Data::default();

    data.set_rocket(rocket.clone());

    let stack = Stack::new(item, 1).with_extras(data);
    let offsets: &[f32] = if count >= 3 { &[0.0, combat::MULTISHOT_ANGLE, -combat::MULTISHOT_ANGLE] } else { &[0.0] };
    let dimension = player.dimension;
    let roll = || vitals::world_random(shared, dimension);
    let eye = player.y + if player.sneaking { 1.27 } else { 1.62 } - 0.15;

    for (index, offset) in offsets.iter().enumerate() {
        let pitch = if index == 0 { 1.0 } else { 1.0 / (roll() * 0.5 + 1.8) + if index == 1 { 0.63 } else { 0.43 } };

        shared
            .world_in(dimension)
            .lock()
            .expect("мир захвачен другим потоком")
            .note_sound_player_at((player.x, player.y, player.z), "minecraft:item.crossbow.shoot", 1.0, pitch);

        let wobble = || f64::from(roll() - roll()) * 0.0172275;
        let (dx, dy, dz) = crate::arrow::aim(player.yaw + offset, player.pitch, 1.0);
        let speed = crate::firework::CROSSBOW_SPEED;
        let velocity = ((dx + wobble()) * speed, (dy + wobble()) * speed, (dz + wobble()) * speed);
        let lifetime = crate::firework::lifetime(rocket.flight, roll(), roll());
        let entity_id = shared.next_entity_id();
        let flying = crate::firework::Rocket::new(entity_id, dimension, stack, (player.x, eye, player.z), velocity, true, (player.entity_id, player.uuid, player.name.clone()), lifetime);

        shared.thrown.lock().expect("снаряды захвачены другим потоком").rockets.push(flying);
    }
}

/// Рассказывает клиенту о стрелах (и игроков, и скелетов): появление,
/// нулевая сверка места, остановка, сверка раз в 20 тактов, исчезновение.
///
/// Полёт клиент считает сам по скорости из пакета появления: пока стрела
/// летит, пакетов о ней нет (чёрный ящик, skeleton-arrow: между сдвигом
/// 0,0,0 и остановкой у оригинала тихо). Слежение оригинала смотрит на мир
/// до хода сущностей, поэтому стрелу, выпущенную игроком (она в этом же такте
/// уже сделала ход), оно показывает вместе с нулевым сдвигом сразу, а стрелу
/// скелета (выпущена в ход мобов, ещё не двигалась) — со сдвигом в следующем
/// такте; приземление узнаёт на такт позже.
pub(super) async fn send_arrow_ticks(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState) -> io::Result<()> {
    let arrows = shared.arrows.lock().expect("стрелы захвачены другим потоком").in_dimension(player.dimension);
    let range = f64::from(shared.properties.view_distance.clamp(2, 10)) * 16.0;

    let present: HashSet<i32> = arrows.iter().filter(|arrow| (arrow.x - player.x).abs() <= range && (arrow.z - player.z).abs() <= range).map(|arrow| arrow.entity_id).collect();
    let vanished: Vec<i32> = player.seen_bow_arrows.keys().filter(|id| !present.contains(id)).copied().collect();

    for id in vanished {
        player.seen_bow_arrows.remove(&id);
        send_remove_entity(stream, id).await?;
    }

    for arrow in &arrows {
        if !present.contains(&arrow.entity_id) {
            continue;
        }

        let Some(mut shown) = player.seen_bow_arrows.get(&arrow.entity_id).copied() else {
            // Появление — с местом и скоростью выстрела, как их знал сервер в
            // миг выстрела (чёрный ящик, pvp-bow: место у стрелка). Увиденная
            // позже — там, где она теперь.
            let ((sx, sy, sz), (svx, svy, svz)) = if arrow.age <= 1 { arrow.start } else { ((arrow.x, arrow.y, arrow.z), (arrow.vx, arrow.vy, arrow.vz)) };
            let (yaw, pitch) = Arrow { vx: svx, vy: svy, vz: svz, ..arrow.clone() }.angles();
            let mut body = encode_varint(ADD_ENTITY);

            body.extend_from_slice(&encode_varint(arrow.entity_id));
            body.extend_from_slice(&arrow.uuid);
            body.extend_from_slice(&encode_varint(arrow.entity_type()));
            push_f64(&mut body, sx);
            push_f64(&mut body, sy);
            push_f64(&mut body, sz);
            types::push_velocity(&mut body, svx, svy, svz);
            body.push(cart_angle(pitch));
            body.push(cart_angle(yaw));
            body.push(0);
            body.extend_from_slice(&encode_varint(arrow.owner));
            write_body(stream, body).await?;

            // Свойства не по умолчанию — в связке с появлением: искры
            // натянутой, «пробивание», «верность» и блеск трезубца.
            if let Some(data) = arrow_data(arrow) {
                write_body(stream, data).await?;
            }

            // Звук броска — от самой сущности, следом за её появлением
            // (чёрный ящик, trident-throw: entity_sound_effect трезубца).
            if arrow.trident.is_some() && !arrow.is_mob_shot() && arrow.age <= 1 && arrow.resync.is_none() {
                let mut body = encode_varint(ENTITY_SOUND);

                body.extend_from_slice(&encode_varint(0));
                body.extend_from_slice(&encode_string("minecraft:item.trident.throw"));
                body.push(0);
                body.extend_from_slice(&encode_varint(SOUND_PLAYERS));
                body.extend_from_slice(&encode_varint(arrow.entity_id));
                body.extend_from_slice(&1.0f32.to_be_bytes());
                body.extend_from_slice(&1.0f32.to_be_bytes());
                body.extend_from_slice(&0i64.to_be_bytes());
                write_body(stream, body).await?;
            }

            let shown = ShownArrow { in_ground: false, first: arrow.age == 0, grid: grid_of((sx, sy, sz)), yaw: cart_angle(yaw), pitch: cart_angle(pitch), flags: arrow.flags(), resynced: arrow.resync.map_or(0, |(due, ..)| due) };

            // Нулевой сдвиг с поворотом: сразу за появлением, если стрела уже
            // ходила (чёрный ящик, arrow-pickup: «arrow сдвиг 0,0,0»), иначе —
            // в следующем такте (чёрный ящик, skeleton-arrow).
            if !shown.first {
                send_zero_move(stream, arrow.entity_id, &shown).await?;

                // Свойства, заданные после создания, слежение шлёт ещё раз
                // (чёрный ящик, crossbow-piercing: «8=1 9=2» до и после
                // нулевого сдвига; trident-loyalty: «11=2 12=true»).
                if arrow.age <= 1
                    && !arrow.is_mob_shot()
                    && let Some(data) = arrow_data(arrow)
                {
                    write_body(stream, data).await?;
                }
            }

            player.seen_bow_arrows.insert(arrow.entity_id, shown);
            continue;
        };

        if shown.first {
            send_zero_move(stream, arrow.entity_id, &shown).await?;
            shown.first = false;
        }

        // Перелом полёта прошлого такта (отскок трезубца, пробитая цель,
        // возврат к хозяину): скорость, сдвиг к тогдашнему месту, флаги
        // (чёрный ящик, trident-loyalty: +2 и +3; crossbow-piercing: +2).
        if let Some((due, at, velocity, flags, facing)) = arrow.resync
            && due > shown.resynced
            && due <= arrow.age
        {
            shown.resynced = due;

            let mut body = encode_varint(ENTITY_VELOCITY);

            body.extend_from_slice(&encode_varint(arrow.entity_id));
            types::push_velocity(&mut body, velocity.0, velocity.1, velocity.2);
            write_body(stream, body).await?;

            send_move_to(stream, arrow, &mut shown, at, facing).await?;

            if flags != shown.flags {
                let mut data = encode_varint(SET_ENTITY_DATA);

                data.extend_from_slice(&encode_varint(arrow.entity_id));
                data.push(ARROW_FLAGS);
                data.extend_from_slice(&encode_varint(PROPERTY_BYTE));
                data.push(flags);
                data.push(PROPERTIES_END);
                write_body(stream, data).await?;
                shown.flags = flags;
            }
        }

        if arrow.in_ground != shown.in_ground {
            // Слежение оригинала узнаёт о приземлении на такт позже (чёрный
            // ящик, arrow-pickup: в такте выстрела — нулевой сдвиг, остановка —
            // на +1).
            if arrow.in_ground && arrow.ground_ticks == 0 {
                player.seen_bow_arrows.insert(arrow.entity_id, shown);
                continue;
            }

            // Встала — нулевая скорость; севшая сразу после выстрела (arrow-pickup)
            // ещё сдвиг до точки остановки с тем поворотом, с каким летела, и
            // свойство «в блоке»; севшая в конце долгого полёта (skeleton-arrow,
            // 18-й такт) у оригинала присылает одну скорость [проверить].
            let mut body = encode_varint(ENTITY_VELOCITY);

            body.extend_from_slice(&encode_varint(arrow.entity_id));
            types::push_velocity(&mut body, arrow.vx, arrow.vy, arrow.vz);
            write_body(stream, body).await?;

            // Трезубец садится всегда полностью (чёрный ящик, trident-throw:
            // на 9-м такте скорость, сдвиг и «в блоке»).
            // Отскочившая стрела — тоже (wither-half-armor: на +12 скорость,
            // сдвиг с поворотом и «в блоке»).
            let full = arrow.age <= LANDING_SYNC_AGE || arrow.trident.is_some() || arrow.resync.is_some();

            if arrow.in_ground && full {
                send_move_to(stream, arrow, &mut shown, (arrow.x, arrow.y, arrow.z), arrow.facing()).await?;
            }

            if full || !arrow.in_ground {
                let mut data = encode_varint(SET_ENTITY_DATA);

                data.extend_from_slice(&encode_varint(arrow.entity_id));
                data.push(ARROW_IN_GROUND);
                data.extend_from_slice(&encode_varint(PROPERTY_BOOLEAN));
                data.push(u8::from(arrow.in_ground));
                data.push(PROPERTIES_END);
                write_body(stream, data).await?;
            }

            shown.in_ground = arrow.in_ground;
        } else if !arrow.in_ground && arrow.lived().is_multiple_of(ARROW_SYNC) && arrow.lived() > 0 {
            // Раз в 20 тактов — сверка места (у оригинала так у всех
            // сущностей; замерено только до 18-го такта) [проверить].
            send_position_sync(stream, arrow).await?;
        } else if arrow.in_ground && (arrow.trident.is_some() || arrow.resync.is_some()) && arrow.lived().is_multiple_of(ARROW_SYNC) {
            // Лежащий трезубец раз в 20 тактов шлёт нулевой сдвиг с поворотом
            // через 20, 40, 60 тактов после броска (чёрный ящик, trident-throw:
            // +20, +40, +60); у стрел так не бывает (skeleton-arrow), кроме
            // отскочившей (wither-half-armor: нулевой сдвиг на +20).
            send_move_to(stream, arrow, &mut shown, (arrow.x, arrow.y, arrow.z), arrow.facing()).await?;
        }

        player.seen_bow_arrows.insert(arrow.entity_id, shown);
    }

    Ok(())
}

/// Раз во сколько тактов жизни стрела сверяет место.
const ARROW_SYNC: u32 = 20;

/// До какого возраста севшая стрела сообщается полностью (скорость, сдвиг,
/// свойство «в блоке»): у оригинала севшая в первые такты — так
/// (arrow-pickup), севшая на 18-м — одной скоростью (skeleton-arrow).
const LANDING_SYNC_AGE: u32 = 2;

/// Свойства стрелы не по умолчанию одним пакетом: флаги (искры), уровень
/// «пробивания», у трезубца — «верность» и блеск зачарованного (minecraft-data,
/// entities.json: flags, pierce_level, loyalty, foil). `None` — всё по умолчанию.
fn arrow_data(arrow: &Arrow) -> Option<Vec<u8>> {
    let foil = arrow.trident.is_some_and(|trident| !trident.extras().enchants.is_empty());

    if arrow.flags() == 0 && arrow.pierce_left <= 0 && arrow.loyalty <= 0 && !foil && arrow.effects.is_empty() {
        return None;
    }

    let mut data = encode_varint(SET_ENTITY_DATA);

    data.extend_from_slice(&encode_varint(arrow.entity_id));

    if arrow.flags() != 0 {
        data.push(ARROW_FLAGS);
        data.extend_from_slice(&encode_varint(PROPERTY_BYTE));
        data.push(arrow.flags());
    }

    if arrow.pierce_left > 0 {
        data.push(ARROW_PIERCE);
        data.extend_from_slice(&encode_varint(PROPERTY_BYTE));
        data.push(arrow.pierce_left.clamp(0, 127) as u8);
    }

    if arrow.loyalty > 0 {
        data.push(TRIDENT_LOYALTY);
        data.extend_from_slice(&encode_varint(PROPERTY_BYTE));
        data.push(arrow.loyalty.clamp(0, 127) as u8);
    }

    if !arrow.effects.is_empty() && arrow.trident.is_none() {
        // minecraft-data 26.1 entities.json: Arrow, effect_color = 11.
        data.push(11);
        data.extend_from_slice(&encode_varint(PROPERTY_INT));
        data.extend_from_slice(&encode_varint(crate::thrown::potion_color(&arrow.effects)));
    }

    if foil {
        data.push(TRIDENT_FOIL);
        data.extend_from_slice(&encode_varint(PROPERTY_BOOLEAN));
        data.push(1);
    }

    data.push(PROPERTIES_END);

    Some(data)
}

/// Сдвиг стрелы к месту `at` с поворотом `facing` от того, что клиент знает;
/// слишком далёкий — сверкой места.
async fn send_move_to(stream: &mut Outgoing<'_>, arrow: &Arrow, shown: &mut ShownArrow, at: (f64, f64, f64), (yaw, pitch): (f32, f32)) -> io::Result<()> {
    let grid = grid_of(at);
    let step = (grid.0 - shown.grid.0, grid.1 - shown.grid.1, grid.2 - shown.grid.2);

    if [step.0, step.1, step.2].iter().all(|value| i16::try_from(*value).is_ok()) {
        let mut body = encode_varint(0x36);

        body.extend_from_slice(&encode_varint(arrow.entity_id));
        push_i16(&mut body, step.0 as i16);
        push_i16(&mut body, step.1 as i16);
        push_i16(&mut body, step.2 as i16);
        body.push(cart_angle(yaw));
        body.push(cart_angle(pitch));
        body.push(0);
        write_body(stream, body).await?;
    } else {
        send_position_sync(stream, arrow).await?;
    }

    shown.grid = grid;

    Ok(())
}

/// Нулевой сдвиг с поворотом выстрела: сверка места сразу после появления.
async fn send_zero_move(stream: &mut Outgoing<'_>, entity_id: i32, shown: &ShownArrow) -> io::Result<()> {
    let mut body = encode_varint(0x36);

    body.extend_from_slice(&encode_varint(entity_id));
    push_i16(&mut body, 0);
    push_i16(&mut body, 0);
    push_i16(&mut body, 0);
    body.push(shown.yaw);
    body.push(shown.pitch);
    body.push(0);
    write_body(stream, body).await
}

/// Свойства стрелы: флаги (искры), «в блоке»; вид значения «логическое».
const ARROW_FLAGS: u8 = 8;
const ARROW_PIERCE: u8 = 9;
const ARROW_IN_GROUND: u8 = 10;
const TRIDENT_LOYALTY: u8 = 11;
const TRIDENT_FOIL: u8 = 12;
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
