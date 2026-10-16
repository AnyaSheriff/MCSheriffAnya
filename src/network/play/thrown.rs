// Брошенные зелья: бросок по правой кнопке и рассказ клиентам о зельях,
// огненных шарах дракона и облаках (сам полёт — thrown.rs).
//
// Порядок пакетов снят чёрным ящиком (splash-potion*, lingering-potion*,
// dragon-fireball): появление с местом и скоростью броска, предмет (свойство
// 8), нулевой сдвиг, предмет ещё раз; раз в 10 тактов — скорость и место.
// Раскол — облако, событие мира, уход снаряда. Что не сверено, помечено «[проверить]».

use super::*;

use crate::thrown::{Cloud, Flying, Kind};

/// Пакеты: скорость сущности, сдвиг с поворотом.
const ENTITY_VELOCITY: i32 = 0x65;
const ENTITY_MOVE_ROTATE: i32 = 0x36;

/// Событие сущности: взрыв ракеты (вики, Entity statuses: 17).
const ENTITY_EVENT: i32 = 0x22;
const ROCKET_BLAST_EVENT: u8 = 17;

/// Свойства ракеты: предмет (8), пущена под углом (10) (minecraft-data,
/// entities.json: fireworks_item, attached_to_target, shot_at_angle).
const ROCKET_ANGLED_PROPERTY: u8 = 10;

/// Свойства: предмет снаряда (8), радиус (8), ожидание (9), частица (10)
/// облака (minecraft-data, entities.json); виды значений — слот, число с
/// точкой, логическое, частица.
const ITEM_PROPERTY: u8 = 8;
const RADIUS_PROPERTY: u8 = 8;
const WAITING_PROPERTY: u8 = 9;
const PARTICLE_PROPERTY: u8 = 10;
const PROPERTY_FLOAT: i32 = 3;
const PROPERTY_BOOLEAN: i32 = 8;
const PROPERTY_PARTICLE: i32 = 16;

/// Номера частиц (minecraft-data 26.1, particles.json).
const PARTICLE_DRAGON_BREATH: i32 = 8;
const PARTICLE_ENTITY_EFFECT: i32 = 21;

/// Раз во сколько тактов снаряд сверяет место (splash-potion: на +10).
const FLYING_SYNC: u32 = 10;

/// Раз во сколько тактов облако шлёт нулевой сдвиг (lingering-potion: +60
/// от первого сдвига).
const CLOUD_SYNC: u32 = 60;

/// Что клиенту о снаряде или облаке уже сказано.
#[derive(Clone, Copy, Debug)]
pub(super) enum Shown {
    Ominous,
    Flying { grid: (i64, i64, i64) },
    /// Брошенный снаряд: что клиент знает о месте и повороте (для сдвига без поворота).
    Shot { grid: (i64, i64, i64), rot: (u8, u8), age: u32 },
    /// Око Края: `fresh` — появление ушло при броске, слежение ещё не
    /// присылало первого (нулевого) сдвига.
    Eye { grid: (i64, i64, i64), fresh: bool },
    /// Облако: первый сдвиг уже ушёл, какие радиус и ожидание клиент знает.
    Cloud { started: bool, radius: f32, waiting: bool },
}

/// Правая кнопка со взрывным/оседающим зельем или снежком в руке `hand`: бросок.
/// Возвращает, был ли он.
pub(super) fn throw_potion(shared: &Shared, player: &mut PlayerState, hand: i32) -> bool {
    if !(0..=1).contains(&hand) { return false; }
    let slot = if hand == 1 { inventory::OFFHAND } else { player.inventory.selected_slot() };
    let Some(stack) = player.inventory.slot(slot) else {
        return false;
    };
    let kind = match crate::combat::item_name(stack.item) {
        Some("splash_potion") => Kind::Splash,
        Some("lingering_potion") => Kind::Lingering,
        Some("snowball") => Kind::Snowball,
        Some("egg" | "blue_egg" | "brown_egg") => Kind::Egg,
        Some("ender_pearl") => Kind::EnderPearl,
        Some("experience_bottle") => Kind::ExperienceBottle,
        _ => return false,
    };

    if player.game_mode == GAME_MODE_SPECTATOR || player.vit.dead {
        return false;
    }

    let now = *shared.ticks.borrow();
    if kind == Kind::EnderPearl && player.item_cooldowns.get("ender_pearl").is_some_and(|until| now < *until) { return false; }
    let dimension = player.dimension;
    let roll = || vitals::world_random(shared, dimension);
    let speed = match kind {
        Kind::Snowball | Kind::Egg | Kind::EnderPearl => 1.5,
        Kind::ExperienceBottle => 0.7,
        _ => crate::thrown::POTION_SPEED,
    };
    let lifted = matches!(kind, Kind::Splash | Kind::Lingering | Kind::ExperienceBottle);
    let (velocity, rotation) = if matches!(kind, Kind::Snowball | Kind::Egg | Kind::EnderPearl) {
        projectile::launch_simple(shared, player, speed)
    } else {
        projectile::launch(shared, player, speed, lifted, 0.0)
    };
    let at = (player.x, player.y + if player.sneaking { 1.27 } else { 1.62 } - 0.1, player.z);

    // Звук броска — до появления; у взрывного раздел «игроки», у оседающего
    // «нейтральные» (чёрный ящик, splash-potion, lingering-potion); высота
    // 0,4 / (0,8…1,2) (вики, Splash Potion).
    let pitch = 0.4 / (roll() * 0.4 + 0.8);

    let mut flying = Flying::potion(shared.next_entity_id(), dimension, kind, stack.with_count(1), at, velocity, player.entity_id);
    flying.start_rot = rotation;
    flying.rot = rotation;
    flying.prev.2 = rotation;

    {
        let mut world = shared.world_in(dimension).lock().expect("мир захвачен другим потоком");

        crate::redstone::game_event_from(&mut world, (player.x, player.y, player.z), 3, Some(player.entity_id), false);
        match kind {
            Kind::Egg => world.note_sound_player_at((player.x, player.y, player.z), "minecraft:entity.egg.throw", 0.5, pitch),
            // Жемчуг и снежок — раздел «нейтральные», яйцо — «игроки» (чёрный ящик,
            // endermite-pearl, itemaudit3-snowball-infested, itemaudit3-egg).
            Kind::EnderPearl => world.note_sound_neutral_at((player.x, player.y, player.z), "minecraft:entity.ender_pearl.throw", 0.5, pitch),
            Kind::ExperienceBottle => world.note_sound_neutral_at((player.x, player.y, player.z), "minecraft:entity.experience_bottle.throw", 0.5, pitch),
            Kind::Snowball => world.note_sound_neutral_at((player.x, player.y, player.z), "minecraft:entity.snowball.throw", 0.5, pitch),
            Kind::Lingering => world.note_sound_neutral_at((player.x, player.y, player.z), "minecraft:entity.lingering_potion.throw", 0.5, pitch),
            _ => world.note_sound_player_at((player.x, player.y, player.z), "minecraft:entity.splash_potion.throw", 0.5, pitch),
        }
        world.note_projectile_launch(flying.clone());
        shared.thrown.lock().expect("снаряды захвачены другим потоком").flying.push(flying);
    }
    player.progress.used(stack.item);
    if kind == Kind::EnderPearl { player.item_cooldowns.insert("ender_pearl", now + 20); }

    // Инвентарь целиком клиент получает от слежения, между появлением зелья
    // и его первым сдвигом (splash-potion: «инвентарь все» там).
    if player.game_mode != GAME_MODE_CREATIVE {
        player.inventory.set(slot, inventory::remainder(stack, stack.count - 1));
    }

    true
}

/// Чёрный ящик throw-*: появление в точке броска, даже если рассылка отстала.
pub(super) async fn send_launch(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState, flying: &Flying) -> io::Result<()> {
    let range = f64::from(shared.properties.view_distance.clamp(2, 10)) * 16.0;
    if flying.dimension != player.dimension || (flying.x - player.x).abs() > range || (flying.z - player.z).abs() > range
        || player.seen_thrown.contains_key(&flying.entity_id) { return Ok(()); }
    // Появление — с местом и скоростью броска (зелье уже сделало
    // первый ход), потом предмет, нулевой сдвиг, предмет ещё раз
    // (splash-potion). Увиденное позже — там, где оно до хода.
    let ((sx, sy, sz), (svx, svy, svz)) = if flying.age <= 1 { flying.start } else { (flying.prev.0, flying.prev.1) };
    let (yaw, pitch) = if flying.is_fireball() { (0.0, 0.0) } else if flying.age <= 1 { flying.start_rot } else { flying.prev.2 };
    let mut body = encode_varint(ADD_ENTITY);

    body.extend_from_slice(&encode_varint(flying.entity_id));
    body.extend_from_slice(&flying.uuid);
    body.extend_from_slice(&encode_varint(flying.entity_type()));
    push_f64(&mut body, sx);
    push_f64(&mut body, sy);
    push_f64(&mut body, sz);
    types::push_velocity(&mut body, svx, svy, svz);
    body.push(cart_angle(pitch));
    body.push(cart_angle(yaw));
    body.push(0);
    body.extend_from_slice(&encode_varint(flying.owner));
    write_body(stream, body).await?;

    send_item(stream, flying).await?;

    if flying.owner == player.entity_id && flying.age <= 1 && !flying.is_fireball() {
        send_container_content(stream, player).await?;
    }

    let mut body = encode_varint(ENTITY_POSITION);

    body.extend_from_slice(&encode_varint(flying.entity_id));
    push_i16(&mut body, 0);
    push_i16(&mut body, 0);
    push_i16(&mut body, 0);
    body.push(0);
    write_body(stream, body).await?;

    send_item(stream, flying).await?;
    player.seen_thrown.insert(flying.entity_id, Shown::Shot { grid: grid_of((sx, sy, sz)), rot: (cart_angle(yaw), cart_angle(pitch)), age: flying.age });
    Ok(())
}

/// throw3-egg-sync-impact.md:19–22: снимок +10 ещё живого снаряда
/// уходит до события 3 и удаления. Одинаково для живых и разбившихся.
pub(super) async fn update_flying(stream: &mut Outgoing<'_>, player: &mut PlayerState, flying: &Flying) -> io::Result<()> {
    let Some(Shown::Shot { grid, rot, age }) = player.seen_thrown.get(&flying.entity_id).copied() else {
        return Ok(());
    };

    if flying.age <= age { return Ok(()); }
    player.seen_thrown.insert(flying.entity_id, Shown::Shot { grid, rot, age: flying.age });

    // Раз в 10 тактов — скорость и место до хода (splash-potion).
    // Записи throw-* скрывают сдвиги за пределы площадки 8×8×8:
    // [проверить throw2-*] точный выбор пакетов у каждого вида.
    if flying.age > 1 && (flying.age - 1).is_multiple_of(FLYING_SYNC) {
        let (at, velocity, (yaw, pitch)) = flying.prev;

        if flying.kind != crate::thrown::Kind::LlamaSpit {
            let mut body = encode_varint(ENTITY_VELOCITY);

            body.extend_from_slice(&encode_varint(flying.entity_id));
            types::push_velocity(&mut body, velocity.0, velocity.1, velocity.2);
            write_body(stream, body).await?;
        }

        let next = grid_of(at);
        if let Some(step) = projectile::relative_step(grid, next) {
            let turned = (cart_angle(yaw), cart_angle(pitch)) != rot;
            let mut body = encode_varint(if turned { ENTITY_MOVE_ROTATE } else { ENTITY_POSITION });
            body.extend_from_slice(&encode_varint(flying.entity_id));
            for delta in step { push_i16(&mut body, delta); }
            if turned {
                body.push(cart_angle(yaw));
                body.push(cart_angle(pitch));
            }
            body.push(0);
            write_body(stream, body).await?;
        } else {
            write_body(stream, projectile::position_sync(flying.entity_id, at, velocity, (yaw, pitch), false)).await?;
        }
        player.seen_thrown.insert(flying.entity_id, Shown::Shot { grid: next, rot: (cart_angle(yaw), cart_angle(pitch)), age: flying.age });
    }
    Ok(())
}

/// Пакет Set Cooldown (protocol.json 26.1: 0x16 — группа, такты).
const SET_COOLDOWN: i32 = 0x16;

/// Правая кнопка с зарядом ветра в руке `hand`: бросок по взгляду из глаз
/// (wind-charge-throw: появился в 1,62 над ногами, скорость 1,5 с малым
/// разбросом), звук броска, откат 10 тактов. Возвращает тело пакета отката,
/// если бросок был; в откате бросок не идёт.
pub(super) fn throw_wind_charge(shared: &Shared, player: &mut PlayerState, hand: i32) -> Option<Vec<u8>> {
    let slot = if hand == 1 { inventory::OFFHAND } else { player.inventory.selected_slot() };
    let stack = player.inventory.slot(slot)?;

    if crate::combat::item_name(stack.item) != Some("wind_charge") || player.game_mode == GAME_MODE_SPECTATOR || player.vit.dead {
        return None;
    }

    let now = *shared.ticks.borrow();

    if player.item_cooldowns.get("wind_charge").is_some_and(|until| now < *until) {
        return None;
    }

    let dimension = player.dimension;
    let roll = || vitals::world_random(shared, dimension);
    let velocity = projectile::velocity(shared, player, crate::wind::THROW_SPEED, false, 0.0);
    let at = (player.x, player.y + if player.sneaking { 1.27 } else { 1.62 }, player.z);
    // Звук броска: «нейтральные», 0,5, высота 0,4 / (0,8…1,2) (вики, Wind
    // Charge: «1/3–0.5»; wind-charge-throw: 0,387).
    let pitch = 0.4 / (roll() * 0.4 + 0.8);

    let flying = Flying::wind_charge(shared.next_entity_id(), dimension, at, velocity, player.entity_id, false);
    {
        let mut world = shared.world_in(dimension).lock().expect("мир");
        world.note_sound_neutral_at((player.x, player.y, player.z), "minecraft:entity.wind_charge.throw", 0.5, pitch);
        world.note_projectile_launch(flying.clone());
        shared.thrown.lock().expect("снаряды захвачены другим потоком").flying.push(flying);
    }

    if player.game_mode != GAME_MODE_CREATIVE {
        player.inventory.set(slot, inventory::remainder(stack, stack.count - 1));
    }

    player.item_cooldowns.insert("wind_charge", now + crate::wind::COOLDOWN as u64);

    let mut body = encode_varint(SET_COOLDOWN);

    body.extend_from_slice(&encode_string("minecraft:wind_charge"));
    body.extend_from_slice(&encode_varint(crate::wind::COOLDOWN));
    Some(body)
}

/// Раз во сколько тактов око Края сверяет место (eye-of-ender-far: скорость
/// и сдвиг на +4, +8…).
const EYE_SYNC: u32 = 4;

/// Правая кнопка с оком Края в руке `hand`: бросок к ближайшей крепости.
/// Нет крепости (ровный мир без построек) — ничего (чёрный ящик,
/// eye-of-ender-none). Возвращает, был ли бросок.
pub(super) fn throw_eye(shared: &Shared, player: &mut PlayerState, hand: i32) -> bool {
    let slot = if hand == 1 { inventory::OFFHAND } else { player.inventory.selected_slot() };
    let Some(stack) = player.inventory.slot(slot) else {
        return false;
    };

    if crate::combat::item_name(stack.item) != Some("ender_eye") || player.game_mode == GAME_MODE_SPECTATOR || player.vit.dead {
        return false;
    }

    let dimension = player.dimension;
    let mut world = shared.world_in(dimension).lock().expect("мир захвачен другим потоком");
    let Some(stronghold) = world.stronghold_target(player.x, player.z) else {
        return false;
    };
    // Появляется на середине роста бросившего (eye-of-ender-far: +0,9).
    let at = (player.x, player.y + if player.sneaking { 0.75 } else { 0.9 }, player.z);
    let eye = crate::ender_eye::Eye::thrown(shared.next_entity_id(), dimension, stack.with_count(1), at, stronghold);
    let entity_id = eye.entity_id;

    shared.thrown.lock().expect("снаряды захвачены другим потоком").eyes.push(eye);
    player.progress.used(stack.item);

    // Порядок оригинала: появление с предметом, звук броска (раздел
    // «нейтральные», громкость 1, высота 0,4 / (0,8…1,2)), взмах руки
    // (eye-of-ender-far; вики: «pitch 1⁄3-0.5»).
    let pitch = 0.4 / (world.random_unit() * 0.4 + 0.8);

    world.note_eye_launch(entity_id);
    world.note_sound_neutral_at((player.x, player.y, player.z), "minecraft:entity.ender_eye.launch", 1.0, pitch);
    world.note_animation(player.entity_id, 0);
    drop(world);

    if player.game_mode != GAME_MODE_CREATIVE {
        player.inventory.set(slot, inventory::remainder(stack, stack.count - 1));
        player.net.slot_changed(slot);
    }

    true
}

/// Появление брошенного ока и его предмет — сразу при броске.
pub(super) async fn send_eye_launch(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState, entity_id: i32) -> io::Result<()> {
    let eye = shared.thrown.lock().expect("снаряды захвачены другим потоком").eyes.iter().find(|eye| eye.entity_id == entity_id).cloned();
    let Some(eye) = eye else {
        return Ok(());
    };
    let range = f64::from(shared.properties.view_distance.clamp(2, 10)) * 16.0;

    if eye.dimension != player.dimension || (eye.x - player.x).abs() > range || (eye.z - player.z).abs() > range {
        return Ok(());
    }

    send_eye_spawn(stream, &eye, eye.prev).await?;
    player.seen_thrown.insert(entity_id, Shown::Eye { grid: grid_of(eye.prev.0), fresh: true });
    Ok(())
}

/// Появление ока (без поворота, номер бросившего не шлётся — 0) и его
/// предмет (свойство 8).
async fn send_eye_spawn(stream: &mut Outgoing<'_>, eye: &crate::ender_eye::Eye, (at, velocity): crate::ender_eye::State) -> io::Result<()> {
    let mut body = encode_varint(ADD_ENTITY);

    body.extend_from_slice(&encode_varint(eye.entity_id));
    body.extend_from_slice(&eye.uuid);
    body.extend_from_slice(&encode_varint(crate::ender_eye::EYE_TYPE));
    push_f64(&mut body, at.0);
    push_f64(&mut body, at.1);
    push_f64(&mut body, at.2);
    types::push_velocity(&mut body, velocity.0, velocity.1, velocity.2);
    body.push(0);
    body.push(0);
    body.push(0);
    body.extend_from_slice(&encode_varint(0));
    write_body(stream, body).await?;
    send_eye_item(stream, eye).await
}

async fn send_eye_item(stream: &mut Outgoing<'_>, eye: &crate::ender_eye::Eye) -> io::Result<()> {
    let mut body = encode_varint(SET_ENTITY_DATA);

    body.extend_from_slice(&encode_varint(eye.entity_id));
    body.push(ITEM_PROPERTY);
    body.extend_from_slice(&encode_varint(PROPERTY_SLOT));
    push_slot(&mut body, Some(eye.stack));
    body.push(PROPERTIES_END);
    write_body(stream, body).await
}

/// Очередной такт слежения за оком: первый раз — нулевой сдвиг и предмет;
/// раз в 4 такта — скорость и сдвиг без поворота к месту до хода.
async fn update_eye(stream: &mut Outgoing<'_>, player: &mut PlayerState, eye: &crate::ender_eye::Eye) -> io::Result<()> {
    match player.seen_thrown.get(&eye.entity_id).copied() {
        None => {
            // Подошёл позже броска: появление там, где око до хода [проверить].
            send_eye_spawn(stream, eye, eye.prev).await?;
            send_zero_move(stream, eye.entity_id).await?;
            send_eye_item(stream, eye).await?;
            player.seen_thrown.insert(eye.entity_id, Shown::Eye { grid: grid_of(eye.prev.0), fresh: false });
        }
        Some(Shown::Eye { grid, fresh: true }) => {
            send_zero_move(stream, eye.entity_id).await?;
            send_eye_item(stream, eye).await?;
            player.seen_thrown.insert(eye.entity_id, Shown::Eye { grid, fresh: false });
        }
        Some(Shown::Eye { grid, fresh: false }) if eye.age > 1 && (eye.age - 1).is_multiple_of(EYE_SYNC) => {
            let (at, velocity) = eye.prev;
            let mut body = encode_varint(ENTITY_VELOCITY);

            body.extend_from_slice(&encode_varint(eye.entity_id));
            types::push_velocity(&mut body, velocity.0, velocity.1, velocity.2);
            write_body(stream, body).await?;

            let next = grid_of(at);
            let mut body = encode_varint(ENTITY_POSITION);

            body.extend_from_slice(&encode_varint(eye.entity_id));
            push_i16(&mut body, (next.0 - grid.0).clamp(i64::from(i16::MIN), i64::from(i16::MAX)) as i16);
            push_i16(&mut body, (next.1 - grid.1).clamp(i64::from(i16::MIN), i64::from(i16::MAX)) as i16);
            push_i16(&mut body, (next.2 - grid.2).clamp(i64::from(i16::MIN), i64::from(i16::MAX)) as i16);
            body.push(0);
            write_body(stream, body).await?;
            player.seen_thrown.insert(eye.entity_id, Shown::Eye { grid: next, fresh: false });
        }
        _ => {}
    }

    Ok(())
}

/// Ракета фейерверка в основной руке щёлкнута по блоку: пуск из точки
/// щелчка, сдвинутой на 0,15 от грани (вики, Firework Rocket: «launched up
/// vertically by using its item on a block») [проверить сдвиг]. Возвращает,
/// был ли пуск; ракету из руки тратит вызывающий.
pub(super) fn launch_rocket(shared: &Shared, player: &mut PlayerState, at: (f64, f64, f64), face: i32) -> bool {
    let Some(stack) = player.inventory.held() else {
        return false;
    };

    if crate::combat::item_name(stack.item) != Some("firework_rocket") || player.game_mode == GAME_MODE_SPECTATOR || player.vit.dead {
        return false;
    }

    let dimension = player.dimension;
    let roll = || vitals::world_random(shared, dimension);
    // Скорость вбок — нормальное распределение с σ 0,001 (вики).
    let gauss = || {
        let (a, b) = (f64::from(roll()).max(1.0e-9), f64::from(roll()));

        (-2.0 * a.ln()).sqrt() * (std::f64::consts::TAU * b).cos() * 0.001
    };
    let velocity = (gauss(), 0.05, gauss());
    let normal = match face {
        0 => (0.0, -1.0, 0.0),
        1 => (0.0, 1.0, 0.0),
        2 => (0.0, 0.0, -1.0),
        3 => (0.0, 0.0, 1.0),
        4 => (-1.0, 0.0, 0.0),
        _ => (1.0, 0.0, 0.0),
    };
    let at = (at.0 + normal.0 * 0.15, at.1 + normal.1 * 0.15, at.2 + normal.2 * 0.15);
    let lifetime = crate::firework::lifetime(stack.extras().rocket().flight, roll(), roll());
    // Предмет ракеты — вся стопка из руки, как была до траты (чёрный ящик,
    // firework-launch: из двух ракет у сущности itemCount 2).
    let rocket = crate::firework::Rocket::new(shared.next_entity_id(), dimension, stack, at, velocity, false, (player.entity_id, player.uuid, player.name.clone()), lifetime);

    shared.thrown.lock().expect("снаряды захвачены другим потоком").rockets.push(rocket);
    player.progress.used(stack.item);

    true
}

/// Событие 17 взорвавшейся ракеты — тем, кто её видит.
pub(super) async fn send_rocket_blast(stream: &mut Outgoing<'_>, player: &PlayerState, entity_id: i32) -> io::Result<()> {
    if !player.seen_thrown.contains_key(&entity_id) {
        return Ok(());
    }

    let mut body = encode_varint(ENTITY_EVENT);

    push_i32(&mut body, entity_id);
    body.push(ROCKET_BLAST_EVENT);
    write_body(stream, body).await
}

/// Правая кнопка с пустым пузырьком у облака дыхания дракона: набрать его.
/// Возвращает, набрал ли.
pub(super) fn fill_breath(shared: &Shared, player: &mut PlayerState, hand: i32) -> bool {
    let slot = if hand == 1 { inventory::OFFHAND } else { player.inventory.selected_slot() };
    let Some(stack) = player.inventory.slot(slot) else {
        return false;
    };

    if crate::combat::item_name(stack.item) != Some("glass_bottle") || player.game_mode == GAME_MODE_SPECTATOR {
        return false;
    }

    if !crate::thrown::bottle_breath(shared, player.dimension, (player.x, player.y, player.z)) {
        return false;
    }

    player.progress.used(stack.item);
    player.progress.trigger("end/dragon_breath");
    let Some(breath) = blocks::item_named("dragon_breath") else {
        return false;
    };

    shared
        .world_in(player.dimension)
        .lock()
        .expect("мир захвачен другим потоком")
        .note_sound_neutral_at((player.x, player.y, player.z), "minecraft:item.bottle.fill_dragonbreath", 1.0, 1.0);

    if player.game_mode == GAME_MODE_CREATIVE {
        return true;
    }

    if stack.count <= 1 {
        player.inventory.set(slot, Some(Stack::new(breath, 1)));
        player.net.slot_changed(slot);
    } else {
        player.inventory.set(slot, inventory::remainder(stack, stack.count - 1));
        player.net.slot_changed(slot);
        shared.players.lock().expect("список игроков захвачен другим потоком").order(&player.uuid, crate::players::Order::Pick { stack: Stack::new(breath, 1) });
    }

    true
}

/// Рассказывает клиенту о снарядах и облаках его измерения. `early` — в
/// начале хода: только облака, которые слежение видит на такт позже (их
/// прошлый радиус, в том числе у только что истаявших).
pub(super) async fn send_thrown_ticks(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState, early: bool, snapshot: Option<&crate::thrown::Thrown>) -> io::Result<()> {
    let live;
    let state = if let Some(snapshot) = snapshot { snapshot } else {
        live = shared.thrown.lock().expect("снаряды").snapshot(player.dimension);
        &live
    };
    let (ominous, eyes, eyes_gone) = (&state.ominous, &state.eyes, &state.eyes_gone);
    let (flying, clouds, impacts, faded, rockets) = (&state.flying, &state.clouds, &state.impacts, &state.faded, &state.rockets);
    let blasts: Vec<_> = state.blasts.iter().map(|b| &b.last).collect();

    if early {
        for flying in &state.landed {
            update_flying(stream, player, flying).await?;
        }
        for cloud in clouds.iter().chain(faded.iter()).filter(|c| c.lag) {
            update_cloud(stream, player, cloud).await?;
        }

        return Ok(());
    }

    let range = f64::from(shared.properties.view_distance.clamp(2, 10)) * 16.0;
    let (viewer_x, viewer_z) = (player.x, player.z);
    let near = |at: (f64, f64, f64)| (at.0 - viewer_x).abs() <= range && (at.2 - viewer_z).abs() <= range;

    for flying in &state.landed {
        update_flying(stream, player, flying).await?;
    }

    // Расколы такта: облако, событие мира, уход снаряда (lingering-potion).
    for impact in impacts {
        if let Some(cloud) = impact.cloud.and_then(|id| clouds.iter().find(|c| c.entity_id == id))
            && near((cloud.x, cloud.y, cloud.z))
        {
            send_cloud_spawn(stream, cloud).await?;
            player.seen_thrown.insert(cloud.entity_id, Shown::Cloud { started: false, radius: 3.0, waiting: false });
        }

        let ((x, y, z), effect, data) = impact.event;

        // Снежок: событие сущности 3 (частицы) до ухода (чёрный ящик, snow-golem-shoot).
        if let Some(status) = impact.status {
            if player.seen_thrown.contains_key(&impact.flying) {
                let mut body = encode_varint(ENTITY_EVENT);

                push_i32(&mut body, impact.flying);
                body.push(status);
                write_body(stream, body).await?;
            }
        } else if effect >= 0 && near((f64::from(x), f64::from(y), f64::from(z))) {
            let mut body = encode_varint(WORLD_EVENT);

            push_i32(&mut body, effect);
            push_packed_position(&mut body, x, y, z);
            push_i32(&mut body, data);
            body.push(0);
            write_body(stream, body).await?;
        }

        if player.seen_thrown.remove(&impact.flying).is_some() {
            send_remove_entity(stream, impact.flying).await?;
        }
    }

    // Пропавшие оки: последняя сверка, звук, уход, у расколотого — событие
    // 2003 (eye-of-ender-far).
    for gone in eyes_gone {
        let eye = &gone.last;

        if !player.seen_thrown.contains_key(&eye.entity_id) {
            continue;
        }

        update_eye(stream, player, eye).await?;
        player.seen_thrown.remove(&eye.entity_id);

        // Звук — и у раскола, и у упавшего предметом (eye-of-ender-far:
        // оба исхода со звуком).
        let sound = crate::world::WorldSound {
            x: eye.x.floor() as i32,
            y: eye.y.floor() as i32,
            z: eye.z.floor() as i32,
            name: "minecraft:entity.ender_eye.death",
            volume: 1.0,
            pitch: 1.0,
            exact: Some((eye.x, eye.y, eye.z)),
            of_block: false,
            of_player: false,
            except: None,
        };

        send_sound(stream, &sound).await?;
        send_remove_entity(stream, eye.entity_id).await?;

        if gone.shattered {
            let mut body = encode_varint(WORLD_EVENT);

            push_i32(&mut body, 2003);
            push_packed_position(&mut body, eye.x.floor() as i32, eye.y.floor() as i32, eye.z.floor() as i32);
            push_i32(&mut body, 0);
            body.push(0);
            write_body(stream, body).await?;
        }
    }

    // Ушедшие без раскола (истаявшие облака, улетевшие снаряды).
    let present: HashSet<i32> = flying
        .iter()
        .filter(|f| near((f.x, f.y, f.z)))
        .map(|f| f.entity_id)
        .chain(clouds.iter().filter(|c| near((c.x, c.y, c.z))).map(|c| c.entity_id))
        .chain(rockets.iter().filter(|r| near((r.x, r.y, r.z))).map(|r| r.entity_id))
        .chain(eyes.iter().filter(|e| near((e.x, e.y, e.z))).map(|e| e.entity_id))
        .chain(ominous.iter().filter(|s|near(s.at)).map(|s|s.id))
        .collect();
    // Взорвавшиеся в этом такте ракеты: сверка места такта — до ухода.
    for rocket in &blasts {
        if player.seen_thrown.contains_key(&rocket.entity_id) {
            update_rocket(stream, player, rocket).await?;
        }
    }

    let vanished: Vec<i32> = player.seen_thrown.keys().filter(|id| !present.contains(id)).copied().collect();

    for id in vanished {
        player.seen_thrown.remove(&id);
        send_remove_entity(stream, id).await?;
    }

    for flying in flying.iter().filter(|f| present.contains(&f.entity_id)) {
        if !player.seen_thrown.contains_key(&flying.entity_id) {
            send_launch(stream, shared, player, flying).await?;
        } else {
            update_flying(stream, player, flying).await?;
        }
    }

    for rocket in rockets.iter().filter(|r| present.contains(&r.entity_id)) {
        update_rocket(stream, player, rocket).await?;
    }

    for s in ominous.iter().filter(|s|present.contains(&s.id)) {
        if let std::collections::hash_map::Entry::Vacant(entry)=player.seen_thrown.entry(s.id) {
            let mut body=encode_varint(ADD_ENTITY);body.extend(encode_varint(s.id));body.extend(crate::items::uuid_for(s.id));
            body.extend(encode_varint(crate::ominous::ENTITY_TYPE));
            push_f64(&mut body,s.at.0);push_f64(&mut body,s.at.1);push_f64(&mut body,s.at.2);
            types::push_velocity(&mut body,0.0,0.0,0.0);body.extend([0,0,0]);body.extend(encode_varint(0));
            write_body(stream,body).await?;
            let mut body=encode_varint(SET_ENTITY_DATA);body.extend(encode_varint(s.id));body.push(8);body.extend(encode_varint(PROPERTY_SLOT));
            push_slot(&mut body,Some(s.stack));body.push(PROPERTIES_END);write_body(stream,body).await?;
            entry.insert(Shown::Ominous);
        }
    }

    for eye in eyes.iter().filter(|e| present.contains(&e.entity_id)) {
        update_eye(stream, player, eye).await?;
    }

    for cloud in clouds.iter().filter(|c| present.contains(&c.entity_id)) {
        if let std::collections::hash_map::Entry::Vacant(entry) = player.seen_thrown.entry(cloud.entity_id) {
            // Облако, о котором клиент не знал (подошёл позже) [проверить].
            send_cloud_spawn(stream, cloud).await?;
            entry.insert(Shown::Cloud { started: false, radius: 3.0, waiting: false });
            continue;
        }

        if !cloud.lag {
            update_cloud(stream, player, cloud).await?;
        }
    }

    Ok(())
}

/// Очередной такт слежения за ракетой: появление с местом и скоростью до
/// хода и свойства (предмет, «под углом»); раз в 10 тактов — скорость и
/// сдвиг с поворотом [проверить чёрным ящиком, firework-launch].
async fn update_rocket(stream: &mut Outgoing<'_>, player: &mut PlayerState, rocket: &crate::firework::Rocket) -> io::Result<()> {
    let Some(Shown::Flying { grid }) = player.seen_thrown.get(&rocket.entity_id).copied() else {
        // Появление — с поворотом 0/0 и номером пустившего; потом предмет (и
        // «под углом»), нулевой сдвиг, свойства ещё раз (чёрный ящик,
        // firework-blast, firework-crossbow).
        let ((sx, sy, sz), (svx, svy, svz)) = if rocket.life <= 1 { rocket.start } else { (rocket.prev.0, rocket.prev.1) };
        let (yaw, pitch) = if rocket.life <= 1 { rocket.start_rot } else { rocket.prev.2 };
        let mut body = encode_varint(ADD_ENTITY);

        body.extend_from_slice(&encode_varint(rocket.entity_id));
        body.extend_from_slice(&rocket.uuid);
        body.extend_from_slice(&encode_varint(crate::firework::ROCKET_TYPE));
        push_f64(&mut body, sx);
        push_f64(&mut body, sy);
        push_f64(&mut body, sz);
        types::push_velocity(&mut body, svx, svy, svz);
        body.push(cart_angle(pitch));
        body.push(cart_angle(yaw));
        body.push(0);
        body.extend_from_slice(&encode_varint(rocket.owner));
        write_body(stream, body).await?;

        let mut body = encode_varint(SET_ENTITY_DATA);

        body.extend_from_slice(&encode_varint(rocket.entity_id));
        body.push(ITEM_PROPERTY);
        body.extend_from_slice(&encode_varint(PROPERTY_SLOT));
        push_slot(&mut body, Some(rocket.stack));

        if rocket.angled {
            body.push(ROCKET_ANGLED_PROPERTY);
            body.extend_from_slice(&encode_varint(PROPERTY_BOOLEAN));
            body.push(1);
        }

        body.push(PROPERTIES_END);

        let properties = body;

        write_body(stream, properties.clone()).await?;
        send_zero_move(stream, rocket.entity_id).await?;
        write_body(stream, properties).await?;
        player.seen_thrown.insert(rocket.entity_id, Shown::Flying { grid: grid_of((sx, sy, sz)) });
        return Ok(());
    };

    if rocket.life > 1 && (rocket.life - 1).is_multiple_of(FLYING_SYNC) {
        let (at, velocity, (yaw, pitch)) = rocket.prev;
        let mut body = encode_varint(ENTITY_VELOCITY);

        body.extend_from_slice(&encode_varint(rocket.entity_id));
        types::push_velocity(&mut body, velocity.0, velocity.1, velocity.2);
        write_body(stream, body).await?;

        let next = grid_of(at);
        if let Some(step) = projectile::relative_step(grid, next) {
            let mut body = encode_varint(ENTITY_MOVE_ROTATE);
            body.extend_from_slice(&encode_varint(rocket.entity_id));
            for delta in step { push_i16(&mut body, delta); }
            body.push(cart_angle(yaw));
            body.push(cart_angle(pitch));
            body.push(0);
            write_body(stream, body).await?;
        } else {
            write_body(stream, projectile::position_sync(rocket.entity_id, at, velocity, (yaw, pitch), false)).await?;
        }
        player.seen_thrown.insert(rocket.entity_id, Shown::Flying { grid: next });
    }

    Ok(())
}

/// Очередной такт слежения за облаком: первый сдвиг с частицей, нулевой
/// сдвиг раз в 60 тактов, перемены радиуса и ожидания.
async fn update_cloud(stream: &mut Outgoing<'_>, player: &mut PlayerState, cloud: &Cloud) -> io::Result<()> {
    {
        let Some(Shown::Cloud { started, radius, waiting }) = player.seen_thrown.get(&cloud.entity_id).copied() else {
            return Ok(());
        };

        let lag = u32::from(cloud.lag);

        if cloud.age < lag {
            return Ok(());
        }

        let view_age = cloud.age - lag;

        if !started {
            send_zero_move(stream, cloud.entity_id).await?;
            send_particle(stream, cloud).await?;
        } else if view_age.is_multiple_of(CLOUD_SYNC) {
            send_zero_move(stream, cloud.entity_id).await?;
        }

        let (now_radius, now_waiting) = if cloud.lag { cloud.prev } else { (cloud.radius, cloud.waiting) };

        if started && (now_radius != radius || now_waiting != waiting) {
            let mut body = encode_varint(SET_ENTITY_DATA);

            body.extend_from_slice(&encode_varint(cloud.entity_id));

            if now_radius != radius {
                body.push(RADIUS_PROPERTY);
                body.extend_from_slice(&encode_varint(PROPERTY_FLOAT));
                body.extend_from_slice(&now_radius.to_be_bytes());
            }

            if now_waiting != waiting {
                body.push(WAITING_PROPERTY);
                body.extend_from_slice(&encode_varint(PROPERTY_BOOLEAN));
                body.push(u8::from(now_waiting));
            }

            body.push(PROPERTIES_END);
            write_body(stream, body).await?;
        }

        let (radius, waiting) = if started { (now_radius, now_waiting) } else { (radius, waiting) };

        player.seen_thrown.insert(cloud.entity_id, Shown::Cloud { started: true, radius, waiting });
    }

    Ok(())
}

/// Предмет снаряда (свойство 8): у зелья — само зелье; у шара нет.
async fn send_item(stream: &mut Outgoing<'_>, flying: &Flying) -> io::Result<()> {
    if flying.stack.is_none() && !flying.no_gravity { return Ok(()); }
    let mut body = encode_varint(SET_ENTITY_DATA);
    body.extend_from_slice(&encode_varint(flying.entity_id));
    if flying.no_gravity {
        body.push(5); body.extend_from_slice(&encode_varint(PROPERTY_BOOLEAN)); body.push(1);
    }
    if let Some(stack) = flying.stack {
        body.push(ITEM_PROPERTY);
        body.extend_from_slice(&encode_varint(PROPERTY_SLOT));
        push_slot(&mut body, Some(stack));
    }
    body.push(PROPERTIES_END);
    write_body(stream, body).await
}

/// Нулевой сдвиг без поворота.
async fn send_zero_move(stream: &mut Outgoing<'_>, entity_id: i32) -> io::Result<()> {
    let mut body = encode_varint(ENTITY_POSITION);

    body.extend_from_slice(&encode_varint(entity_id));
    push_i16(&mut body, 0);
    push_i16(&mut body, 0);
    push_i16(&mut body, 0);
    body.push(0);
    write_body(stream, body).await
}

/// Появление облака и его частица (lingering-potion: место — точка раскола,
/// без скорости и поворота).
async fn send_cloud_spawn(stream: &mut Outgoing<'_>, cloud: &Cloud) -> io::Result<()> {
    let mut body = encode_varint(ADD_ENTITY);

    body.extend_from_slice(&encode_varint(cloud.entity_id));
    body.extend_from_slice(&cloud.uuid);
    body.extend_from_slice(&encode_varint(crate::thrown::CLOUD_TYPE));
    push_f64(&mut body, cloud.x);
    push_f64(&mut body, cloud.y);
    push_f64(&mut body, cloud.z);
    types::push_velocity(&mut body, 0.0, 0.0, 0.0);
    body.push(0);
    body.push(0);
    body.push(0);
    body.extend_from_slice(&encode_varint(0));
    write_body(stream, body).await?;

    send_particle(stream, cloud).await
}

/// Частица облака (свойство 10): дыхание дракона с силой 1 или эффект с
/// цветом зелья (dragon-fireball, lingering-potion).
async fn send_particle(stream: &mut Outgoing<'_>, cloud: &Cloud) -> io::Result<()> {
    let mut body = encode_varint(SET_ENTITY_DATA);

    body.extend_from_slice(&encode_varint(cloud.entity_id));
    body.push(PARTICLE_PROPERTY);
    body.extend_from_slice(&encode_varint(PROPERTY_PARTICLE));

    if cloud.dragon {
        body.extend_from_slice(&encode_varint(PARTICLE_DRAGON_BREATH));
        body.extend_from_slice(&1.0f32.to_be_bytes());
    } else {
        body.extend_from_slice(&encode_varint(PARTICLE_ENTITY_EFFECT));
        body.extend_from_slice(&crate::thrown::opaque(cloud.color).to_be_bytes());
    }

    body.push(PROPERTIES_END);
    write_body(stream, body).await
}
