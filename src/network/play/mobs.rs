// Мобы для клиента Java: появление, свойства, движение и события.
//
// Порядок и частота пакетов сняты с оригинала чёрным ящиком — сценарии
// cow-physics, cow-hit, cow-feed, cow-breed, chicken-fall, pig-baby,
// sheep-shear, mob-plate и зонды probe-cow-plain, probe-animals-plain.
// Устроено так:
//
// - Появление: Add Entity и свойства не по умолчанию — в миг появления. Всё
//   остальное рассылает слежение, раз в такт и до хода мобов (снимок
//   `Items::mob_view`): конец такта показывает то, что было до хода.
// - В первый прогон слежения свойства уходят ещё раз (у оригинала они «грязные»
//   после создания), сразу после первой сверки места.
// - Раз в 3 такта от появления (и вне очереди, когда моба ударили): скорость,
//   если заметно изменилась, потом место шагом (перенос — при смене «на
//   земле»), поворот, поворот головы; раз в 60 тактов место уходит в любом
//   случае. Ушибленному скорость уходит ещё раз в самом конце.
// - Свойства — каждый такт, что изменились; атрибуты (скорость шага) —
//   на третьем такте жизни и после правки данных командой.
//
// Что не сверено, помечено «[проверить]».

use super::*;
use crate::mob::{Event, Kind, Mob};
use crate::xp::XpOrb;

/// Место снаряжения «голова».
const EQUIPMENT_HEAD: u8 = 5;

/// Пакеты: событие сущности, урон, поворот головы, атрибуты, звук сущности.
const ENTITY_EVENT: i32 = 0x22;
const DAMAGE_EVENT: i32 = 0x19;
const ROTATE_HEAD: i32 = 0x53;
const UPDATE_ATTRIBUTES: i32 = 0x83;
const LEVEL_PARTICLES: i32 = 0x2F;
const ENTITY_SOUND: i32 = 0x74;

/// Виды значений свойств (protocol.json, entityMetadataEntry).
const TYPE_BYTE: i32 = 0;
const TYPE_INT: i32 = 1;
const TYPE_FLOAT: i32 = 3;
const TYPE_BOOL: i32 = 8;
const TYPE_POSE: i32 = 20;
const TYPE_COW_VARIANT: i32 = 23;
const TYPE_COW_SOUND: i32 = 24;
const TYPE_PIG_VARIANT: i32 = 28;
const TYPE_PIG_SOUND: i32 = 29;
const TYPE_CHICKEN_VARIANT: i32 = 30;
const TYPE_CHICKEN_SOUND: i32 = 31;

/// Поза «умирает» (protocol.json, Pose: 7 — dying).
const POSE_DYING: i32 = 7;

/// Номер атрибута «скорость движения» (registry attribute 26.1).
const ATTRIBUTE_MOVEMENT_SPEED: i32 = 22;

/// Раз во сколько тактов от появления моб рассылает место и скорость, и раз
/// во сколько — место, даже если оно не изменилось (как у вагонеток).
const EVERY: u64 = 3;
const FORCED: u64 = 60;

/// На каком такте жизни уходят атрибуты: у животных на третьем, у крипера на
/// первом (чёрный ящик, creeper-explode); у зомби — сразу при появлении.
fn attributes_at(kind: Kind) -> u64 {
    match kind {
        Kind::Creeper => 1,
        _ => 3,
    }
}

/// Меньшие сдвиги и перемены скорости не сообщаются.
const MOVED: f64 = 7.629_394_531_25e-6;
const SPED: f64 = 1e-7;

/// Номера частиц (minecraft-data 26.1, particles.json).
const PARTICLE_BLOCK: i32 = 1;

/// Дальше этого от игрока (в блоках) мобов клиенту не показывают: как у
/// оригинала, где у животных дальность отслеживания — 10 чанков, но не
/// больше дальности обзора. [проверить: у оригинала свои пороги входа и выхода]
fn tracking_range(shared: &Shared) -> f64 {
    f64::from(shared.properties.view_distance.clamp(2, 10)) * 16.0
}

/// Одно свойство моба: номер, вид значения и сами байты значения.
type Property = (u8, i32, Vec<u8>);

/// Моб, как его знает клиент.
pub(super) struct ShownMob {
    place: (f64, f64, f64),
    grid: (i64, i64, i64),
    yaw: u8,
    pitch: u8,
    head: u8,
    speed: (f64, f64, f64),
    on_ground: bool,
    impulses: u32,
    teleports: u32,
    attr_changes: u32,
    attributes_sent: bool,

    /// Слежение ещё не прогонялось: свойства уйдут ещё раз целиком.
    first: bool,
    properties: Vec<Property>,
}

fn float(value: f32) -> Vec<u8> {
    value.to_be_bytes().to_vec()
}

/// Свойства моба, которые мы ведём, — все, с текущими значениями.
/// Номера — из minecraft-data 26.1 (entities.json, metadataKeys): 0 флаги,
/// 4 тишина, 6 поза, 9 здоровье, 15 флаги моба (NoAI — 1, «агрессивен» — 4),
/// 16 детёныш; далее по виду: у крипера 16 — направление запала (VarInt),
/// 17 — заряжен, 18 — подожжён; у паука 16 — флаги (1 — лезет по стене);
/// у скелета 16 — превращение в бродягу.
fn properties(mob: &Mob) -> Vec<Property> {
    let fire = u8::from(mob.vitals.fire > 0);
    let aggressive = u8::from(mob.kind.hostile() && mob.h.target.is_some()) << 2;
    let mut out: Vec<Property> = vec![
        (0, TYPE_BYTE, vec![fire]),
        (4, TYPE_BOOL, vec![u8::from(mob.silent)]),
        (6, TYPE_POSE, encode_varint(if mob.is_dead() { POSE_DYING } else { 0 })),
        (9, TYPE_FLOAT, float(mob.vitals.health)),
        (15, TYPE_BYTE, vec![u8::from(mob.no_ai) | aggressive]),
    ];

    match mob.kind {
        Kind::Cow => {
            out.push((16, TYPE_BOOL, vec![u8::from(mob.is_baby())]));
            out.push((18, TYPE_COW_VARIANT, encode_varint(0)));
            out.push((19, TYPE_COW_SOUND, encode_varint(0)));
        }
        Kind::Pig => {
            out.push((16, TYPE_BOOL, vec![u8::from(mob.is_baby())]));
            out.push((18, TYPE_INT, encode_varint(0)));
            out.push((19, TYPE_PIG_VARIANT, encode_varint(0)));
            out.push((20, TYPE_PIG_SOUND, encode_varint(0)));
        }
        Kind::Sheep => {
            out.push((16, TYPE_BOOL, vec![u8::from(mob.is_baby())]));
            out.push((18, TYPE_BYTE, vec![(mob.color & 15) | (u8::from(mob.sheared) << 4)]));
        }
        Kind::Chicken => {
            out.push((16, TYPE_BOOL, vec![u8::from(mob.is_baby())]));
            out.push((18, TYPE_CHICKEN_VARIANT, encode_varint(1)));
            out.push((19, TYPE_CHICKEN_SOUND, encode_varint(0)));
        }
        Kind::Zombie => out.push((16, TYPE_BOOL, vec![u8::from(mob.is_baby())])),
        Kind::Skeleton => out.push((16, TYPE_BOOL, vec![0])),
        Kind::Creeper => {
            out.push((16, TYPE_INT, encode_varint(if mob.h.swell_dir == 0 { -1 } else { mob.h.swell_dir })));
            out.push((17, TYPE_BOOL, vec![0]));
            out.push((18, TYPE_BOOL, vec![u8::from(mob.h.lit)]));
        }
        Kind::Spider => out.push((16, TYPE_BYTE, vec![u8::from(mob.h.climbing)])),
    }

    out
}

/// Значения по умолчанию: пока свойство такое, при появлении оно не
/// присылается. Варианты животных у нас всегда обычные, и клиент знает
/// их сам.
fn is_default(entry: &Property, kind: Kind) -> bool {
    let (key, _, value) = entry;

    match (*key, kind) {
        (16, Kind::Creeper) => value.as_slice() == encode_varint(-1).as_slice(),
        (0 | 4 | 6 | 15 | 16, _) => value.iter().all(|byte| *byte == 0),
        (9, _) => value.as_slice() == float(1.0),
        (18, Kind::Sheep) => value.iter().all(|byte| *byte == 0),
        (18, Kind::Creeper) | (17, Kind::Creeper) => value.iter().all(|byte| *byte == 0),
        (18..=20, _) => true,
        _ => false,
    }
}

/// Пакет свойств: перечисленные записи.
async fn send_properties(stream: &mut Outgoing<'_>, entity_id: i32, entries: &[&Property]) -> io::Result<()> {
    if entries.is_empty() {
        return Ok(());
    }

    let mut data = encode_varint(SET_ENTITY_DATA);

    data.extend_from_slice(&encode_varint(entity_id));

    for (key, kind, value) in entries.iter().copied() {
        data.push(*key);
        data.extend_from_slice(&encode_varint(*kind));
        data.extend_from_slice(value);
    }

    data.push(PROPERTIES_END);
    write_body(stream, data).await
}

/// Появление моба: Add Entity и свойства не по умолчанию.
async fn send_mob(stream: &mut Outgoing<'_>, mob: &Mob) -> io::Result<ShownMob> {
    let mut body = encode_varint(ADD_ENTITY);

    body.extend_from_slice(&encode_varint(mob.entity_id));
    body.extend_from_slice(&mob.uuid);
    body.extend_from_slice(&encode_varint(mob.kind.java_type()));
    push_f64(&mut body, mob.x);
    push_f64(&mut body, mob.y);
    push_f64(&mut body, mob.z);
    types::push_velocity(&mut body, mob.vx, mob.vy, mob.vz);
    body.push(cart_angle(mob.pitch));
    body.push(cart_angle(mob.yaw));
    body.push(cart_angle(mob.head_yaw));
    body.extend_from_slice(&encode_varint(0));
    write_body(stream, body).await?;

    let properties = properties(mob);
    let changed: Vec<&Property> = properties.iter().filter(|entry| !is_default(entry, mob.kind)).collect();

    send_properties(stream, mob.entity_id, &changed).await?;

    send_mob_equipment(stream, mob).await?;

    // Зомби — атрибуты сразу при появлении (чёрный ящик, zombie-hit).
    if mob.kind == Kind::Zombie {
        send_attributes(stream, mob).await?;
    }

    Ok(ShownMob {
        place: (mob.x, mob.y, mob.z),
        grid: grid_of((mob.x, mob.y, mob.z)),
        yaw: cart_angle(mob.yaw),
        pitch: cart_angle(mob.pitch),
        head: cart_angle(mob.head_yaw),
        speed: (mob.vx, mob.vy, mob.vz),
        on_ground: mob.on_ground,
        impulses: mob.impulses,
        teleports: mob.teleports,
        attr_changes: mob.attr_changes,
        attributes_sent: mob.kind == Kind::Zombie,
        first: true,
        properties,
    })
}

/// Атрибуты: сообщается одна скорость шага (у оригинала так же: клиенту
/// нужен только изменённый), базовое значение — одинарной точности.
async fn send_attributes(stream: &mut Outgoing<'_>, mob: &Mob) -> io::Result<()> {
    let mut body = encode_varint(UPDATE_ATTRIBUTES);

    body.extend_from_slice(&encode_varint(mob.entity_id));
    body.extend_from_slice(&encode_varint(1));
    body.extend_from_slice(&encode_varint(ATTRIBUTE_MOVEMENT_SPEED));
    push_f64(&mut body, f64::from(mob.kind.speed() as f32));
    body.extend_from_slice(&encode_varint(0));
    write_body(stream, body).await
}

/// Показывает клиенту всех мобов вокруг: о них журнал не скажет — они
/// появились раньше.
pub(super) async fn send_all_mobs(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState) -> io::Result<()> {
    player.seen_mobs.clear();
    player.seen_arrows.clear();
    player.seen_bow_arrows.clear();
    send_mob_ticks(stream, shared, player).await
}

/// Событие моба — клиенту: удар, смерть, звук, частицы.
pub(super) async fn send_mob_event(
    stream: &mut Outgoing<'_>,
    entity_id: i32,
    at: (f64, f64, f64),
    event: &Event,
    shown: &HashMap<i32, ShownMob>,
    listener: (f64, f64, f64),
) -> io::Result<()> {
    if !shown.contains_key(&entity_id) {
        return Ok(());
    }

    let heard = |place: (f64, f64, f64)| (place.0 - listener.0).powi(2) + (place.1 - listener.1).powi(2) + (place.2 - listener.2).powi(2) <= 48.0 * 48.0;

    match event {
        Event::Hurt { source, cause, .. } => {
            let mut body = encode_varint(DAMAGE_EVENT);

            body.extend_from_slice(&encode_varint(entity_id));
            body.extend_from_slice(&encode_varint(source.type_id()));
            body.extend_from_slice(&encode_varint(if *cause == 0 { 0 } else { cause + 1 }));
            body.extend_from_slice(&encode_varint(if *cause == 0 { 0 } else { cause + 1 }));
            body.push(0);
            write_body(stream, body).await?;
        }
        Event::Animation(kind) => {
            let mut body = encode_varint(ENTITY_ANIMATION);

            body.extend_from_slice(&encode_varint(entity_id));
            body.push(*kind);
            write_body(stream, body).await?;
        }
        Event::Death => send_status(stream, entity_id, 3).await?,
        Event::Poof => send_status(stream, entity_id, 60).await?,
        Event::Hearts => send_status(stream, entity_id, 18).await?,
        Event::Eating => send_status(stream, entity_id, 10).await?,
        Event::Swing => send_animation(stream, entity_id).await?,
        Event::Sound { name, volume, pitch } if heard(at) => send_sound(stream, name, SOUND_NEUTRAL, at, *volume, *pitch).await?,
        Event::PlayerSound { name, at: place } if heard(*place) => send_sound(stream, name, SOUND_PLAYERS, *place, 1.0, 1.0).await?,
        Event::EntitySound { name, category, volume, pitch } if heard(at) => {
            let mut body = encode_varint(ENTITY_SOUND);

            body.extend_from_slice(&encode_varint(0));
            body.extend_from_slice(&encode_string(name));
            body.push(0);
            body.extend_from_slice(&encode_varint(*category));
            body.extend_from_slice(&encode_varint(entity_id));
            body.extend_from_slice(&volume.to_be_bytes());
            body.extend_from_slice(&pitch.to_be_bytes());
            body.extend_from_slice(&0i64.to_be_bytes());
            write_body(stream, body).await?;
        }
        Event::Landing { block, count } => send_particles(stream, PARTICLE_BLOCK, Some(*block), at, (0.0, 0.0, 0.0), 0.15, *count).await?,
        Event::Particle { kind, at: place, offset, speed, count } => send_particles(stream, kind.id(), None, *place, *offset, *speed, *count).await?,
        Event::Sound { .. } | Event::PlayerSound { .. } | Event::EntitySound { .. } | Event::Loot(_) => {}
    }

    Ok(())
}

/// Что у моба в руках и на голове: лук у скелета (основная рука, место 0) и
/// шлем на голове (место 5) — по одному пакету на моба; шлем показывается
/// железным [проверить: вид шлема не хранится].
async fn send_mob_equipment(stream: &mut Outgoing<'_>, mob: &Mob) -> io::Result<()> {
    let mut slots: Vec<(u8, Stack)> = Vec::new();

    if mob.kind == Kind::Skeleton && mob.h.bow && let Some(bow) = blocks::item_named("bow") {
        slots.push((EQUIPMENT_MAIN_HAND, Stack::new(bow, 1)));
    }

    if mob.h.helmet && let Some(helmet) = blocks::item_named("iron_helmet") {
        slots.push((EQUIPMENT_HEAD, Stack::new(helmet, 1)));
    }

    if slots.is_empty() {
        return Ok(());
    }

    let mut body = encode_varint(SET_EQUIPMENT);

    body.extend_from_slice(&encode_varint(mob.entity_id));

    // Места идут подряд, старший бит номера — «за этим есть ещё».
    for (index, (slot, stack)) in slots.iter().enumerate() {
        body.push(if index + 1 < slots.len() { *slot | 0x80 } else { *slot });
        push_slot(&mut body, Some(*stack));
    }

    write_body(stream, body).await
}

/// Взмах рукой (Entity Animation 0).
async fn send_animation(stream: &mut Outgoing<'_>, entity_id: i32) -> io::Result<()> {
    let mut body = encode_varint(ENTITY_ANIMATION);

    body.extend_from_slice(&encode_varint(entity_id));
    body.push(ANIMATION_SWING_MAIN_ARM);
    write_body(stream, body).await
}

/// Событие сущности (Entity Event): номер сущности — i32, событие — байт.
async fn send_status(stream: &mut Outgoing<'_>, entity_id: i32, status: u8) -> io::Result<()> {
    let mut body = encode_varint(ENTITY_EVENT);

    body.extend_from_slice(&entity_id.to_be_bytes());
    body.push(status);
    write_body(stream, body).await
}

/// Звук в точке: раздел, громкость, высота; место — в восьмых долях,
/// отсекаемых к нулю.
async fn send_sound(stream: &mut Outgoing<'_>, name: &str, category: i32, (x, y, z): (f64, f64, f64), volume: f32, pitch: f32) -> io::Result<()> {
    let mut body = encode_varint(SOUND_EFFECT);

    body.extend_from_slice(&encode_varint(0));
    body.extend_from_slice(&encode_string(name));
    body.push(0);
    body.extend_from_slice(&encode_varint(category));
    push_i32(&mut body, (x * 8.0) as i32);
    push_i32(&mut body, (y * 8.0) as i32);
    push_i32(&mut body, (z * 8.0) as i32);
    body.extend_from_slice(&volume.to_be_bytes());
    body.extend_from_slice(&pitch.to_be_bytes());
    body.extend_from_slice(&0i64.to_be_bytes());

    write_body(stream, body).await
}

/// Частицы (Level Particles): вид, блок для частицы блока, место, разброс,
/// скорость, число.
pub(super) async fn send_particles(
    stream: &mut Outgoing<'_>,
    particle: i32,
    block: Option<i32>,
    (x, y, z): (f64, f64, f64),
    (ox, oy, oz): (f32, f32, f32),
    speed: f32,
    count: i32,
) -> io::Result<()> {
    let mut body = encode_varint(LEVEL_PARTICLES);

    body.push(0); // не издалека
    body.push(0); // не всегда

    for value in [x, y, z] {
        body.extend_from_slice(&value.to_be_bytes());
    }

    for value in [ox, oy, oz, speed] {
        body.extend_from_slice(&value.to_be_bytes());
    }

    push_i32(&mut body, count);
    body.extend_from_slice(&encode_varint(particle));

    if let Some(block) = block {
        body.extend_from_slice(&encode_varint(block));
    }

    write_body(stream, body).await
}

/// Конец такта у мобов: те, кто вошёл в дальность, показываются; ушедшие —
/// убираются; у показанных рассылается изменившееся.
pub(super) async fn send_mob_ticks(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState) -> io::Result<()> {
    let range = tracking_range(shared);

    let (mobs, orbs, orb_ids): (Vec<Mob>, Vec<XpOrb>, Vec<i32>) = {
        let items = shared.items_in(player.dimension).lock().expect("предметы захвачены другим потоком");
        let near = |x: f64, z: f64| (x - player.x).abs() <= range && (z - player.z).abs() <= range;

        (
            items.mob_view().iter().filter(|mob| near(mob.x, mob.z)).cloned().collect(),
            items.orb_view().iter().filter(|orb| near(orb.x, orb.z)).cloned().collect(),
            items.orbs().iter().filter(|orb| near(orb.x, orb.z)).map(|orb| orb.entity_id).collect(),
        )
    };

    // Ушедшие из дальности или пропавшие из мира (шарики опыта живут в том
    // же списке показанного: их убирают подбор и `Gone` журнала).
    let mut present: HashSet<i32> = mobs.iter().map(|mob| mob.entity_id).collect();

    present.extend(orbs.iter().map(|orb| orb.entity_id));
    present.extend(orb_ids);
    let gone: Vec<i32> = player.seen_mobs.keys().filter(|id| !present.contains(id)).copied().collect();

    for entity_id in gone {
        send_remove_entity(stream, entity_id).await?;
        player.seen_mobs.remove(&entity_id);
    }

    for mob in &mobs {
        let mut shown = match player.seen_mobs.remove(&mob.entity_id) {
            Some(shown) => shown,
            None => {
                let shown = send_mob(stream, mob).await?;

                // Родившийся во время хода мобов: слежение увидит его со
                // следующего такта (у оригинала первая сверка — тогда).
                if mob.late {
                    player.seen_mobs.insert(mob.entity_id, shown);
                    continue;
                }

                shown
            }
        };

        let speed = (mob.vx, mob.vy, mob.vz);
        let impulse = mob.impulses != shown.impulses;
        let due = mob.ticks % EVERY == 0;

        if due || impulse {
            let change = (speed.0 - shown.speed.0).powi(2) + (speed.1 - shown.speed.1).powi(2) + (speed.2 - shown.speed.2).powi(2);

            if change > SPED || (speed == (0.0, 0.0, 0.0) && shown.speed != (0.0, 0.0, 0.0)) {
                send_velocity(stream, mob.entity_id, speed).await?;
                shown.speed = speed;
            }

            if due {
                let place = (mob.x, mob.y, mob.z);
                let grid = grid_of(place);
                let moved = (place.0 - shown.place.0).powi(2) + (place.1 - shown.place.1).powi(2) + (place.2 - shown.place.2).powi(2) >= MOVED;
                let forced = mob.ticks % FORCED == 0;
                let (yaw, pitch) = (cart_angle(mob.yaw), cart_angle(mob.pitch));
                let turned = yaw != shown.yaw || pitch != shown.pitch;
                let step = (grid.0 - shown.grid.0, grid.1 - shown.grid.1, grid.2 - shown.grid.2);
                let fits = |value: i64| i16::try_from(value).is_ok();
                let whole = Position { x: mob.x, y: mob.y, z: mob.z, yaw: mob.yaw, pitch: mob.pitch, on_ground: mob.on_ground };

                if mob.teleports != shown.teleports || mob.on_ground != shown.on_ground || !(fits(step.0) && fits(step.1) && fits(step.2)) {
                    send_teleport_with_speed(stream, mob.entity_id, whole, speed).await?;
                    (shown.place, shown.grid, shown.yaw, shown.pitch) = (place, grid, yaw, pitch);
                    shown.on_ground = mob.on_ground;
                    shown.teleports = mob.teleports;
                } else if moved || forced || turned {
                    let position = moved || forced;
                    let kind = match (position, turned) {
                        (true, true) => ENTITY_POSITION_AND_ROTATION,
                        (true, false) => ENTITY_POSITION,
                        (false, _) => ENTITY_ROTATION,
                    };
                    let mut body = encode_varint(kind);

                    body.extend_from_slice(&encode_varint(mob.entity_id));

                    if kind != ENTITY_ROTATION {
                        push_i16(&mut body, step.0 as i16);
                        push_i16(&mut body, step.1 as i16);
                        push_i16(&mut body, step.2 as i16);
                        (shown.place, shown.grid) = (place, grid);
                    }

                    if kind != ENTITY_POSITION {
                        body.push(yaw);
                        body.push(pitch);
                        (shown.yaw, shown.pitch) = (yaw, pitch);
                    }

                    body.push(u8::from(mob.on_ground));
                    shown.on_ground = mob.on_ground;
                    write_body(stream, body).await?;
                }

                // Поворот головы — отдельным пакетом.
                let head = cart_angle(mob.head_yaw);

                if head != shown.head {
                    let mut body = encode_varint(ROTATE_HEAD);

                    body.extend_from_slice(&encode_varint(mob.entity_id));
                    body.push(head);
                    write_body(stream, body).await?;
                    shown.head = head;
                }
            }
        }

        // Свойства: в первый прогон — все не по умолчанию ещё раз, потом
        // только изменившиеся.
        let now = properties(mob);
        let changed: Vec<&Property> = if shown.first {
            now.iter().filter(|entry| !is_default(entry, mob.kind)).collect()
        } else {
            now.iter().filter(|entry| !shown.properties.contains(entry)).collect()
        };

        send_properties(stream, mob.entity_id, &changed).await?;
        shown.properties = now;

        if shown.first && mob.kind == Kind::Zombie {
            send_attributes(stream, mob).await?;
        }

        // Снаряжение уходит и второй раз в первом проходе (чёрный ящик,
        // skeleton-stand и zombie-attack).
        if shown.first {
            send_mob_equipment(stream, mob).await?;
        }

        shown.first = false;

        // Атрибуты: на третьем такте жизни и после каждой правки данных.
        if (!shown.attributes_sent && mob.ticks >= attributes_at(mob.kind)) || mob.attr_changes != shown.attr_changes {
            send_attributes(stream, mob).await?;
            shown.attributes_sent = true;
            shown.attr_changes = mob.attr_changes;
        }

        // Ушибленному скорость уходит ещё раз — в самом конце.
        if impulse {
            send_velocity(stream, mob.entity_id, speed).await?;
            shown.speed = speed;
            shown.impulses = mob.impulses;
        }

        player.seen_mobs.insert(mob.entity_id, shown);
    }

    send_orb_ticks(stream, player, &orbs).await?;
    send_arrow_ticks(stream, shared, player).await
}

/// Вид сущности «шарик опыта» (minecraft-data 26.1: 49) и номер её свойства
/// «значение» (чёрный ящик, xp-orb-far: `8=значение`).
const EXPERIENCE_ORB_TYPE: i32 = 49;
const ORB_VALUE_KEY: u8 = 8;

/// Раз во сколько тактов от появления шарик рассылает место и скорость
/// (у мобов — 3): чёрный ящик, xp-orb-far — на 20-м, 40-м и 60-м.
const ORB_EVERY: u64 = 20;

fn orb_properties(orb: &XpOrb) -> Vec<Property> {
    vec![(ORB_VALUE_KEY, TYPE_INT, encode_varint(orb.value))]
}

/// Появление шарика: Add Entity со скоростью толчка и свойство «значение».
pub(super) async fn send_orb(stream: &mut Outgoing<'_>, orb: &XpOrb) -> io::Result<ShownMob> {
    let mut body = encode_varint(ADD_ENTITY);

    body.extend_from_slice(&encode_varint(orb.entity_id));
    body.extend_from_slice(&orb.uuid);
    body.extend_from_slice(&encode_varint(EXPERIENCE_ORB_TYPE));
    push_f64(&mut body, orb.x);
    push_f64(&mut body, orb.y);
    push_f64(&mut body, orb.z);
    types::push_velocity(&mut body, orb.vx, orb.vy, orb.vz);
    body.extend_from_slice(&[0, 0, 0]);
    body.extend_from_slice(&encode_varint(0));
    write_body(stream, body).await?;

    let properties = orb_properties(orb);

    send_properties(stream, orb.entity_id, &properties.iter().collect::<Vec<_>>()).await?;

    Ok(ShownMob {
        place: (orb.x, orb.y, orb.z),
        grid: grid_of((orb.x, orb.y, orb.z)),
        yaw: 0,
        pitch: 0,
        head: 0,
        speed: (orb.vx, orb.vy, orb.vz),
        on_ground: orb.on_ground,
        impulses: 0,
        teleports: 0,
        attr_changes: 0,
        attributes_sent: true,
        first: true,
        properties,
    })
}

/// Слежение за шариками опыта — как за мобами, но раз в 20 тактов: скорость,
/// если изменилась, потом место шагом (переносом при смене «на земле»); в
/// первый прогон и раз в 60 тактов место уходит в любом случае, а в первый
/// прогон свойство «значение» — ещё раз.
async fn send_orb_ticks(stream: &mut Outgoing<'_>, player: &mut PlayerState, orbs: &[XpOrb]) -> io::Result<()> {
    for orb in orbs {
        let mut shown = match player.seen_mobs.remove(&orb.entity_id) {
            Some(shown) => shown,
            None => send_orb(stream, orb).await?,
        };

        let speed = (orb.vx, orb.vy, orb.vz);

        if orb.age % ORB_EVERY == 0 {
            let change = (speed.0 - shown.speed.0).powi(2) + (speed.1 - shown.speed.1).powi(2) + (speed.2 - shown.speed.2).powi(2);

            if change > SPED || (speed == (0.0, 0.0, 0.0) && shown.speed != (0.0, 0.0, 0.0)) {
                send_velocity(stream, orb.entity_id, speed).await?;
                shown.speed = speed;
            }

            let place = (orb.x, orb.y, orb.z);
            let grid = grid_of(place);
            let moved = (place.0 - shown.place.0).powi(2) + (place.1 - shown.place.1).powi(2) + (place.2 - shown.place.2).powi(2) >= MOVED;
            let step = (grid.0 - shown.grid.0, grid.1 - shown.grid.1, grid.2 - shown.grid.2);
            let fits = |value: i64| i16::try_from(value).is_ok();

            if orb.on_ground != shown.on_ground || !(fits(step.0) && fits(step.1) && fits(step.2)) {
                let whole = Position { x: orb.x, y: orb.y, z: orb.z, yaw: 0.0, pitch: 0.0, on_ground: orb.on_ground };

                send_teleport_with_speed(stream, orb.entity_id, whole, speed).await?;
                (shown.place, shown.grid) = (place, grid);
                shown.on_ground = orb.on_ground;
            } else if moved || orb.age % FORCED == 0 {
                let mut body = encode_varint(ENTITY_POSITION);

                body.extend_from_slice(&encode_varint(orb.entity_id));
                push_i16(&mut body, step.0 as i16);
                push_i16(&mut body, step.1 as i16);
                push_i16(&mut body, step.2 as i16);
                body.push(u8::from(orb.on_ground));
                write_body(stream, body).await?;
                (shown.place, shown.grid) = (place, grid);
                shown.on_ground = orb.on_ground;
            }
        }

        // Свойство «значение»: в первый прогон — ещё раз.
        if shown.first {
            send_properties(stream, orb.entity_id, &orb_properties(orb).iter().collect::<Vec<_>>()).await?;
            shown.first = false;
        }

        player.seen_mobs.insert(orb.entity_id, shown);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Стрелы
// ---------------------------------------------------------------------------
//
// Появление — Add Entity с номером стрелка в поле данных; дальше сдвиг и
// поворот пакетами, как у мобов; воткнувшаяся — свойство «в земле» и
// перенос в точку остановки. Частота и порядок пакетов — по замеру
// оригинала (сценарий skeleton-arrow) [проверить].

/// Стрела, как её знает клиент.
pub(super) struct ShownArrow {
    grid: (i64, i64, i64),
    yaw: u8,
    pitch: u8,
    in_ground: bool,
    first: bool,
}

/// Раз во сколько тактов жизни стрела сверяет место (`updateInterval` 20 у
/// снарядов) [проверить: замерено только до 17-го такта].
const ARROW_SYNC: u64 = 20;

/// Стрелы для клиента: появившиеся показываются, пропавшие убираются. Полёт
/// клиент считает сам по скорости из пакета появления (чёрный ящик,
/// skeleton-arrow): сверка места нулевым сдвигом в первом проходе, потом
/// раз в 20 тактов, а застрявшая сообщается нулевой скоростью.
pub(super) async fn send_arrow_ticks(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState) -> io::Result<()> {
    let range = tracking_range(shared);
    let arrows: Vec<crate::arrow::Arrow> = {
        let items = shared.items_in(player.dimension).lock().expect("предметы захвачены другим потоком");

        items.arrows().iter().filter(|arrow| (arrow.x - player.x).abs() <= range && (arrow.z - player.z).abs() <= range).cloned().collect()
    };
    let present: HashSet<i32> = arrows.iter().map(|arrow| arrow.entity_id).collect();
    let gone: Vec<i32> = player.seen_arrows.keys().filter(|id| !present.contains(id)).copied().collect();

    for entity_id in gone {
        send_remove_entity(stream, entity_id).await?;
        player.seen_arrows.remove(&entity_id);
    }

    for arrow in &arrows {
        let place = (arrow.x, arrow.y, arrow.z);
        let grid = grid_of(place);
        let (yaw, pitch) = (cart_angle(arrow.yaw), cart_angle(arrow.pitch));

        let Some(mut shown) = player.seen_arrows.remove(&arrow.entity_id) else {
            let mut body = encode_varint(ADD_ENTITY);

            body.extend_from_slice(&encode_varint(arrow.entity_id));
            body.extend_from_slice(&arrow.uuid);
            body.extend_from_slice(&encode_varint(crate::arrow::JAVA_TYPE));
            push_f64(&mut body, arrow.x);
            push_f64(&mut body, arrow.y);
            push_f64(&mut body, arrow.z);
            types::push_velocity(&mut body, arrow.vx, arrow.vy, arrow.vz);
            body.push(pitch);
            body.push(yaw);
            body.push(0);
            body.extend_from_slice(&encode_varint(arrow.owner));
            write_body(stream, body).await?;

            player.seen_arrows.insert(arrow.entity_id, ShownArrow { grid, yaw, pitch, in_ground: false, first: true });
            continue;
        };

        if shown.first {
            // Первая сверка: место как в пакете появления — сдвиг нулевой.
            let mut body = encode_varint(ENTITY_POSITION_AND_ROTATION);

            body.extend_from_slice(&encode_varint(arrow.entity_id));
            push_i16(&mut body, 0);
            push_i16(&mut body, 0);
            push_i16(&mut body, 0);
            body.push(shown.yaw);
            body.push(shown.pitch);
            body.push(0);
            write_body(stream, body).await?;
            shown.first = false;
        } else if arrow.ticks.is_multiple_of(ARROW_SYNC) && !arrow.in_ground {
            let step = (grid.0 - shown.grid.0, grid.1 - shown.grid.1, grid.2 - shown.grid.2);
            let fits = |value: i64| i16::try_from(value).is_ok();

            if fits(step.0) && fits(step.1) && fits(step.2) {
                let mut body = encode_varint(ENTITY_POSITION_AND_ROTATION);

                body.extend_from_slice(&encode_varint(arrow.entity_id));
                push_i16(&mut body, step.0 as i16);
                push_i16(&mut body, step.1 as i16);
                push_i16(&mut body, step.2 as i16);
                body.push(yaw);
                body.push(pitch);
                body.push(0);
                write_body(stream, body).await?;
            } else {
                let whole = Position { x: arrow.x, y: arrow.y, z: arrow.z, yaw: arrow.yaw, pitch: arrow.pitch, on_ground: false };

                send_teleport_with_speed(stream, arrow.entity_id, whole, (arrow.vx, arrow.vy, arrow.vz)).await?;
            }

            (shown.grid, shown.yaw, shown.pitch) = (grid, yaw, pitch);
        }

        if arrow.in_ground != shown.in_ground {
            send_velocity(stream, arrow.entity_id, (arrow.vx, arrow.vy, arrow.vz)).await?;
            shown.in_ground = arrow.in_ground;
        }

        player.seen_arrows.insert(arrow.entity_id, shown);
    }

    Ok(())
}
