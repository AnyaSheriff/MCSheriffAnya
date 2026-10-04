// Кровать и якорь возрождения со стороны подключения: щелчок, сон,
// пробуждение, точка возрождения и само возрождение у кровати или якоря.
// Счёт мест и правил — в bed.rs, пропуск ночи — в tick.rs.
//
// Порядок пакетов снят с оригинала чёрным ящиком (bed-sleep-day,
// bed-sleep-night, bed-leave, bed-respawn, bed-nether, anchor-overworld,
// anchor-nether): сообщения и взмах — раньше повтора блоков, «занята» и данные
// блок-сущности кровати — общей пачкой, поза — свойствами в конце такта.

use super::*;

use crate::bed::{self, RespawnPoint};
use crate::commands_admin::{Tag, translate};

/// Анимации сущности: взмах основной рукой и «встал с кровати».
const ANIMATION_SWING: u8 = 0;
const ANIMATION_WAKE_UP: u8 = 2;

/// Game Event 0: «нет кровати или заряженного якоря» — клиент сам пишет
/// «You have no home bed or charged respawn anchor, or it was obstructed»
/// (чёрный ящик, bed-respawn: `no_respawn_block_available` перед Respawn).
const NO_RESPAWN_BLOCK: u8 = 0;

/// Высота спящего над блоком кровати (чёрный ящик, bed-sleep-night: сверка
/// места на 0,688 над полом).
const SLEEP_HEIGHT: f64 = 0.6875;

/// System Chat с переводимым текстом: над панелью (`overlay`) или в чат.
fn message(key: &str, with: Vec<Tag>, overlay: bool) -> Vec<u8> {
    let mut body = encode_varint(SYSTEM_CHAT);

    body.extend_from_slice(&translate(key, with).encode());
    body.push(u8::from(overlay));
    body
}

/// Взмах рукой после удачного щелчка: оригинал шлёт его и самому игроку —
/// дважды (чёрный ящик, bed-sleep-day, anchor-nether: два `Animate 0`).
fn swing_twice(world: &mut World, entity_id: i32) {
    world.note_animation(entity_id, ANIMATION_SWING);
    world.note_animation(entity_id, ANIMATION_SWING);
}

/// Повтор щёлкнутого блока и блока за гранью — как у прочих щелчков.
fn repeat_clicked(world: &mut World, clicked: bed::Pos, face: i32) {
    let (bx, by, bz) = offset_towards(clicked.0, clicked.1, clicked.2, face);

    world.tell_directly(clicked.0, clicked.1, clicked.2);
    world.tell_directly(bx, by, bz);
}

/// Ставит личную точку возрождения. Новая (другое место или мир) — «Respawn
/// point set» в чат (вики, Bed: «The message "Respawn point set" is displayed
/// in chat when the respawn point is successfully changed»; та же кровать —
/// без сообщения, чёрный ящик bed-sleep-day). Поворот обновляется всегда:
/// по нему выбирается сторона кровати. Возвращает, сменилась ли точка.
fn set_respawn(world: &mut World, player: &mut PlayerState, pos: bed::Pos) -> bool {
    let changed = player.respawn.is_none_or(|old| old.dimension != player.dimension || old.pos != pos);

    player.respawn = Some(RespawnPoint { dimension: player.dimension, pos, yaw: player.yaw, forced: false });

    if changed {
        world.note_private(player.entity_id, message("block.minecraft.set_spawn", Vec::new(), false));
    }

    changed
}

/// Сколько спит и сколько нужно — всем игрокам обычного мира над панелью:
/// «Sleeping through this night» или «X/Y players sleeping» (чёрный ящик,
/// bed-sleep-night: `sleep.skipping_night`; bed-leave: `sleep.players_sleeping`
/// с 0 и 1).
fn announce_sleepers(shared: &Shared, world: &mut World) {
    let percentage = shared.sleeping_percentage.load(std::sync::atomic::Ordering::Relaxed);
    let (viewers, (total, sleeping, _)) = {
        let players = shared.players.lock().expect("список игроков захвачен другим потоком");
        let viewers: Vec<i32> = players.members().iter().filter(|member| member.dimension == Dimension::Overworld).map(|member| member.entity_id).collect();

        (viewers, players.sleep_status(Dimension::Overworld, world.tick()))
    };

    let body = match bed::sleepers_needed(total, percentage) {
        Some(needed) if sleeping >= needed => message("sleep.skipping_night", Vec::new(), true),
        Some(needed) => message("sleep.players_sleeping", vec![Tag::Int(sleeping as i32), Tag::Int(needed as i32)], true),
        // Больше 100 % — ночь не пропустить (вики, Bed: «No amount of rest
        // can pass this night»). [проверить] ключ перевода.
        None => message("sleep.not_possible", Vec::new(), true),
    };

    for viewer in viewers {
        world.note_private(viewer, body.clone());
    }
}

/// Есть ли рядом с изголовьем чудища, мешающие спать (вики, Bed).
fn monsters_near(shared: &Shared, dimension: Dimension, head: bed::Pos) -> bool {
    let (wide, high) = bed::MONSTER_REACH;
    let centre = (f64::from(head.0) + 0.5, f64::from(head.1), f64::from(head.2) + 0.5);
    let items = shared.items_in(dimension).lock().expect("предметы захвачены другим потоком");

    items.mobs().iter().any(|mob| {
        mob.kind.hostile() && !mob.is_dead() && (mob.x - centre.0).abs() <= wide && (mob.y - centre.1).abs() <= high && (mob.z - centre.2).abs() <= wide
    })
}

/// Щелчок по кровати. None — это не кровать.
///
/// Как у оригинала (вики, Bed; чёрный ящик): щелчок по изножью — это щелчок
/// по изголовью; в Незере и Энде кровать взрывается; занятая — «This bed is
/// occupied»; дальше — далеко ли, не загорожена ли, точка возрождения
/// (и днём тоже), ночь ли, нет ли рядом чудищ; и только тогда — сон.
pub(super) fn use_bed(world: &mut World, shared: &Shared, player: &mut PlayerState, clicked: bed::Pos, face: i32, sequence: i32) -> Option<Reaction> {
    if !bed::is_bed(world.get_block(clicked.0, clicked.1, clicked.2)) {
        return None;
    }

    let accepted = Some(Reaction::accepted(sequence));

    // Одинокое изножье: щелчок ничего не делает. [проверить] у оригинала.
    let Some(head) = bed::head_of(world, clicked) else {
        repeat_clicked(world, clicked, face);
        return accepted;
    };

    // Не в обычном мире — взрыв в изголовье (вики, Bed: «The explosion
    // centers on the head part of the bed»). Изголовье убирается молча, а
    // изножье ломается с частицами (2001) — раньше вспышки; обе половины —
    // общей пачкой (чёрный ящик, bed-nether).
    if !bed::beds_work(player.dimension) {
        let foot = bed::foot_of(world, head);

        world.set_block(head.0, head.1, head.2, world::AIR);

        if let Some(foot) = foot {
            let state = world.get_block(foot.0, foot.1, foot.2);

            if bed::is_bed(state) {
                world.note_level_event(foot.0, foot.1, foot.2, 2001, state);
                world.set_block(foot.0, foot.1, foot.2, world::AIR);
            }
        }

        let centre = (f64::from(head.0) + 0.5, f64::from(head.1) + 0.5, f64::from(head.2) + 0.5);

        crate::tick::respawn_block_blast(shared, player.dimension, world, centre);
        swing_twice(world, player.entity_id);
        repeat_clicked(world, clicked, face);
        return accepted;
    }

    let refuse = |world: &mut World, player: &PlayerState, key: &str| {
        world.note_private(player.entity_id, message(key, Vec::new(), true));
    };

    if bed::occupied(world, head) {
        refuse(world, player, "block.minecraft.bed.occupied");
    } else if player.sleeping.is_none() && !player.vit.dead {
        if !bed::bed_in_reach(world, head, (player.x, player.y, player.z)) {
            refuse(world, player, "block.minecraft.bed.too_far_away");
        } else if bed::bed_obstructed(world, head) {
            refuse(world, player, "block.minecraft.bed.obstructed");
        } else {
            set_respawn(world, player, head);

            let raining = world.weather.is_raining();
            let thundering = world.weather.is_thundering();

            if !bed::dark_enough(world.time_of_day(), raining, thundering) {
                refuse(world, player, "block.minecraft.bed.no_sleep");
            } else if player.game_mode != GAME_MODE_CREATIVE && monsters_near(shared, player.dimension, head) {
                refuse(world, player, "block.minecraft.bed.not_safe");
            } else {
                fall_asleep(world, shared, player, head);
            }
        }
    }

    swing_twice(world, player.entity_id);
    repeat_clicked(world, clicked, face);

    accepted
}

/// Лечь: место — над изголовьем, сон отмечен в общем списке (с такта мира,
/// пропуск ночи считает от него), всем в мире — сколько спит.
fn fall_asleep(world: &mut World, shared: &Shared, player: &mut PlayerState, head: bed::Pos) {
    // «Занята» — до повтора щёлкнутого блока: повтор уже «занятый», а обе
    // половины и данные блок-сущности уходят пачкой (чёрный ящик,
    // bed-sleep-night).
    bed::set_occupied(world, head, true);
    bed::note_entities(world, head);

    player.sleeping = Some(head);
    player.progress.slept();
    player.x = f64::from(head.0) + 0.5;
    player.y = f64::from(head.1) + SLEEP_HEIGHT;
    player.z = f64::from(head.2) + 0.5;

    shared.players.lock().expect("список игроков захвачен другим потоком").set_sleeping(&player.uuid, Some(world.tick()));
    announce_sleepers(shared, world);
    publish_player(shared, player);
}

/// Встать с кровати: взмах «встал», (сообщение о спящих — если встал сам),
/// сверка места у кровати лицом к изголовью, «занята» снимается пачкой
/// (чёрный ящик, bed-leave и bed-sleep-night).
pub(super) fn wake_up(shared: &Shared, player: &mut PlayerState, announce: bool) {
    let Some(head) = player.sleeping.take() else {
        return;
    };

    shared.players.lock().expect("список игроков захвачен другим потоком").set_sleeping(&player.uuid, None);

    let mut world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");

    world.note_animation(player.entity_id, ANIMATION_WAKE_UP);

    if announce {
        announce_sleepers(shared, &mut world);
    }

    // Встать некуда — остаёмся над кроватью. [проверить] у оригинала.
    let spot = bed::bed_standup(&world, head, player.yaw).unwrap_or((player.x, f64::from(head.1) + 0.5625, player.z));

    player.x = spot.0;
    player.y = spot.1;
    player.z = spot.2;
    player.yaw = bed::yaw_toward(spot, head);

    let body = position_body(player, Sync::Absolute);

    world.note_private(player.entity_id, body);

    if bed::is_bed(world.get_block(head.0, head.1, head.2)) {
        bed::set_occupied(&mut world, head, false);
        bed::note_entities(&mut world, head);
    }

    drop(world);
    publish_player(shared, player);
}

/// Что сказать о кровати, сломанной игроком (чёрный ящик, bed-place-break):
/// в выживании — слом второй половины (2001 с её состоянием), в творческом —
/// слом изножья в самом изножье [проверить] изголовье в творческом; предмет
/// — из изголовья.
pub(super) struct BedBreak {
    pub event_at: bed::Pos,
    pub event_state: i32,
    pub drop_at: bed::Pos,
}

pub(super) fn bed_break_plan(world: &World, pos: bed::Pos, creative: bool) -> Option<BedBreak> {
    let head = bed::head_of(world, pos)?;
    let foot = bed::foot_of(world, head)?;
    let other = if pos == head { foot } else { head };
    let event_at = if creative { pos } else { other };

    Some(BedBreak { event_at, event_state: world.get_block(event_at.0, event_at.1, event_at.2), drop_at: head })
}

/// Player Command: действие 0 — «встать с кровати».
pub(super) fn player_command(payload: &[u8], player: &mut PlayerState, shared: &Shared) {
    const LEAVE_BED: i32 = 0;

    let mut cursor = Cursor::new(payload);

    if let (Some(_entity), Some(LEAVE_BED)) = (cursor.varint(), cursor.varint()) {
        wake_up(shared, player, true);
    }
}

/// Такт спящего: кровать пропала, рассвело или игрок умер — он встаёт
/// (вики, Bed: «if the bed is destroyed while the player is in it … the
/// player wakes prematurely»).
pub(super) fn tick(shared: &Shared, player: &mut PlayerState) {
    // Ночь пропущена (поручение `WakeUp` из tick.rs пришло к концу прошлого
    // такта): встать сейчас — события уйдут в такте утра, как у оригинала, а
    // свойства — в его конце (чёрный ящик, bed-sleep-night: на +100 всё).
    if std::mem::take(&mut player.wake_due) {
        wake_up(shared, player, false);
        player.net.hold_meta = true;
        return;
    }

    let Some(head) = player.sleeping else {
        return;
    };

    let (bed_gone, day) = {
        let world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");
        let gone = !bed::is_bed(world.get_block(head.0, head.1, head.2));
        let day = !bed::dark_enough(world.time_of_day(), world.weather.is_raining(), world.weather.is_thundering());

        (gone, day)
    };

    if bed_gone || day || player.vit.dead {
        wake_up(shared, player, true);
    }
}

/// Щелчок по якорю возрождения, который не заряжают: None — не якорь, или
/// пустой, или его заряжают светокамнем (это `redstone::use_with_item`).
///
/// В Незере заряженный якорь ставит точку возрождения (сообщение, звук
/// `set_spawn`), в прочих мирах взрывается: блок убирается молча, вспышка
/// силы 5 с огнём в его середине (вики, Respawn Anchor; чёрный ящик,
/// anchor-overworld, anchor-nether).
pub(super) fn use_anchor(world: &mut World, shared: &Shared, player: &mut PlayerState, clicked: bed::Pos, face: i32, held: Option<&str>, sequence: i32) -> Option<Reaction> {
    let state = world.get_block(clicked.0, clicked.1, clicked.2);

    if !bed::is_anchor(state) {
        return None;
    }

    let charges = bed::anchor_charges(state)?;

    if charges == 0 || (held == Some("glowstone") && charges < 4) {
        return None;
    }

    if !bed::anchors_work(player.dimension) {
        let centre = (f64::from(clicked.0) + 0.5, f64::from(clicked.1) + 0.5, f64::from(clicked.2) + 0.5);

        world.set_block(clicked.0, clicked.1, clicked.2, world::AIR);
        crate::tick::respawn_block_blast(shared, player.dimension, world, centre);
        swing_twice(world, player.entity_id);
        repeat_clicked(world, clicked, face);

        return Some(Reaction::accepted(sequence));
    }

    // Та же точка — ни сообщения, ни звука, ни взмаха. [проверить]
    if set_respawn(world, player, clicked) {
        world.note_sound(clicked.0, clicked.1, clicked.2, "minecraft:block.respawn_anchor.set_spawn", 1.0, 1.0);
        swing_twice(world, player.entity_id);
    }

    repeat_clicked(world, clicked, face);

    Some(Reaction::accepted(sequence))
}

/// Куда возрождается игрок: измерение, место, поворот; `missing` — точки
/// не нашлось (Game Event 0); `deplete` — пакет звука якоря, отдавший заряд.
pub(super) struct Target {
    pub dimension: Dimension,
    pub at: (f64, f64, f64),
    pub yaw: Option<f32>,
    pub missing: bool,
    pub deplete: Option<Vec<u8>>,
}

/// Звук `deplete` якоря самому возрождённому — в углу блока, разделом
/// «блоки» (чёрный ящик, anchor-nether: звук в (3; 0; 3), между событием
/// прав и свойствами). [проверить] слышат ли его другие.
fn deplete_sound((x, y, z): bed::Pos) -> Vec<u8> {
    let mut body = encode_varint(SOUND_EFFECT);

    body.extend_from_slice(&encode_varint(0));
    body.extend_from_slice(&encode_string("minecraft:block.respawn_anchor.deplete"));
    body.push(0);
    body.extend_from_slice(&encode_varint(SOUND_BLOCK));
    push_i32(&mut body, x * 8);
    push_i32(&mut body, y * 8);
    push_i32(&mut body, z * 8);
    push_f32(&mut body, 1.0);
    push_f32(&mut body, 1.0);
    body.extend_from_slice(&0i64.to_be_bytes());
    body
}

/// Место возрождения: у кровати (в её мире, где кровати работают), у
/// заряженного якоря (в Незере — заряд тратится), у точки по команде
/// `spawnpoint`; иначе — у точки появления мира, а личная точка забывается
/// (вики, Bed: «If a bed is obstructed, the player's spawn point is cleared
/// after they respawn»; Respawn Anchor).
pub(super) fn respawn_target(shared: &Shared, player: &mut PlayerState) -> Target {
    let mut missing = false;

    if let Some(point) = player.respawn {
        let mut world = shared.world_in(point.dimension).lock().expect("мир захвачен другим потоком");

        world.ensure(point.pos.0.div_euclid(CHUNK_SIZE), point.pos.2.div_euclid(CHUNK_SIZE));

        let state = world.get_block(point.pos.0, point.pos.1, point.pos.2);
        let charges = bed::anchor_charges(state).filter(|_| bed::is_anchor(state)).unwrap_or(0);

        let found = if bed::is_bed(state) && bed::beds_work(point.dimension) {
            bed::bed_standup(&world, point.pos, point.yaw).map(|at| (at, false))
        } else if charges > 0 && bed::anchors_work(point.dimension) {
            bed::anchor_standup(&world, point.pos).map(|at| (at, true))
        } else if point.forced {
            bed::forced_standup(&world, point.pos).map(|at| (at, false))
        } else {
            None
        };

        if let Some((at, anchor)) = found {
            let deplete = anchor.then(|| {
                if let Some(less) = bed::with_charges(state, charges - 1) {
                    world.set_block(point.pos.0, point.pos.1, point.pos.2, less);
                }

                deplete_sound(point.pos)
            });
            let yaw = if point.forced { point.yaw } else { bed::yaw_toward(at, point.pos) };

            return Target { dimension: point.dimension, at, yaw: Some(yaw), missing: false, deplete };
        }

        if !point.forced {
            player.respawn = None;
            missing = true;
        }
    }

    Target { dimension: Dimension::Overworld, at: on_the_ground(shared, player.spawn_point), yaw: None, missing, deplete: None }
}

/// Game Event 0 — до Respawn (чёрный ящик, bed-respawn).
pub(super) async fn send_no_respawn_block(stream: &mut Outgoing<'_>) -> io::Result<()> {
    send_game_event(stream, NO_RESPAWN_BLOCK, 0.0).await
}

/// Точка по команде `spawnpoint`: личная, в обычном мире, годится и без
/// кровати.
pub(super) fn set_forced(player: &mut PlayerState, (x, y, z): (f64, f64, f64)) {
    player.respawn = Some(RespawnPoint {
        dimension: Dimension::Overworld,
        pos: (x.floor() as i32, y.floor() as i32, z.floor() as i32),
        yaw: 0.0,
        forced: true,
    });
}
