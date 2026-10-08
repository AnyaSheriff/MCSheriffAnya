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
const EQUIPMENT_OFF_HAND: u8 = 1;

/// Пакеты: событие сущности, урон, поворот головы, атрибуты, звук сущности.
const ENTITY_EVENT: i32 = 0x22;
const DAMAGE_EVENT: i32 = 0x19;
const ROTATE_HEAD: i32 = 0x53;
const UPDATE_ATTRIBUTES: i32 = 0x83;
const LEVEL_PARTICLES: i32 = 0x2F;
const ENTITY_SOUND: i32 = 0x74;
const LINK_ENTITIES: i32 = 0x64;

/// Виды значений свойств (protocol.json, entityMetadataEntry).
const TYPE_BYTE: i32 = 0;
const TYPE_INT: i32 = 1;
const TYPE_LONG: i32 = 2;
const TYPE_OPT_COMPONENT: i32 = 6;
const TYPE_OPT_UUID: i32 = 13;
const TYPE_CAT_VARIANT: i32 = 21;
const TYPE_WOLF_VARIANT: i32 = 25;
const TYPE_FLOAT: i32 = 3;
const TYPE_BOOL: i32 = 8;
const TYPE_OPT_BLOCK_POS: i32 = 11;
const TYPE_DIRECTION: i32 = 12;
const TYPE_OPT_BLOCK_STATE: i32 = 15;
const TYPE_PARTICLES: i32 = 17;
const TYPE_VILLAGER_DATA: i32 = 18;
const TYPE_POSE: i32 = 20;
const TYPE_COW_VARIANT: i32 = 23;
const TYPE_COW_SOUND: i32 = 24;
const TYPE_PIG_VARIANT: i32 = 28;
const TYPE_PIG_SOUND: i32 = 29;
const TYPE_CHICKEN_VARIANT: i32 = 30;
const TYPE_CHICKEN_SOUND: i32 = 31;

/// Фаза дракона «висит на месте» (вики, Ender Dragon: DragonPhase 10).
const DRAGON_PHASE_HOVER: i32 = 10;

/// Поза «умирает» (protocol.json, Pose: 7 — dying).
const POSE_DYING: i32 = 7;

/// Номера атрибутов «скорость движения» и «броня» (registry attribute 26.1).
const ATTRIBUTE_MOVEMENT_SPEED: i32 = 22;
const ATTRIBUTE_ARMOR: i32 = 0;
const ATTRIBUTE_MAX_HEALTH: i32 = 19;

/// Раз во сколько тактов от появления моб рассылает место и скорость, и раз
/// во сколько — место, даже если оно не изменилось (как у вагонеток).
const EVERY: u64 = 3;
const FORCED: u64 = 60;

/// На каком такте жизни уходят атрибуты: у животных на третьем, у крипера на
/// первом (чёрный ящик, creeper-explode); у зомби — сразу при появлении.
fn attributes_at(kind: Kind) -> u64 {
    match kind {
        Kind::Creeper => 1,
        // Кристаллу и дракону атрибуты не шлются вовсе (чёрный ящик,
        // end-crystal-explode, dragon-summon).
        Kind::EndCrystal | Kind::EnderDragon | Kind::ShulkerBullet | Kind::LeashKnot => u64::MAX,
        _ => 3,
    }
}

/// Шлются ли атрибуты сразу при появлении и ещё раз в первом прогоне
/// слежения (чёрный ящик, zombie-hit и enderman-stand).
fn attributes_at_spawn(kind: Kind) -> bool {
    matches!(kind, Kind::Zombie | Kind::ZombieVillager | Kind::Drowned | Kind::Husk | Kind::Enderman | Kind::ZombifiedPiglin | Kind::Piglin | Kind::MagmaCube | Kind::Slime)
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

    /// `modded` — как показано клиенту (атрибуты, у ведьмы и рука).
    modded: bool,

    /// Слежение ещё не прогонялось: свойства уйдут ещё раз целиком.
    first: bool,
    properties: Vec<Property>,

    /// Наибольшее здоровье, как показано (у волка оно меняется при
    /// приручении — атрибут max_health; чёрный ящик, wolf-tame).
    max_health: f32,
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

    // Кристалл Края — не живое: флаги, тишина, цель луча (11 — Optional
    // BlockPos: 0 — нет), «показывать подножие» (minecraft-data: beam_target,
    // show_bottom; по умолчанию подножие есть — чёрный ящик,
    // end-crystal-explode: призванный свойств не шлёт).
    if mob.kind == Kind::EndCrystal {
        let mut beam = Vec::new();

        match mob.beam_target {
            None => beam.push(0),
            Some((x, y, z)) => {
                beam.push(1);
                beam.extend_from_slice(&(((i64::from(x) & 0x3FF_FFFF) << 38) | ((i64::from(z) & 0x3FF_FFFF) << 12) | (i64::from(y) & 0xFFF)).to_be_bytes());
            }
        }

        return vec![(0, TYPE_BYTE, vec![fire]), (4, TYPE_BOOL, vec![u8::from(mob.silent)]), (8, TYPE_OPT_BLOCK_POS, beam), (9, TYPE_BOOL, vec![1])];
    }

    // Снаряд шалкера и узел поводка — тоже не живые: флаги и тишина
    // (minecraft-data: shulker_bullet и leash_knot — только общие свойства).
    if matches!(mob.kind, Kind::ShulkerBullet | Kind::LeashKnot) {
        return vec![(0, TYPE_BYTE, vec![fire]), (4, TYPE_BOOL, vec![u8::from(mob.silent)])];
    }
    // «Агрессивен» (бит 4) у эндермена и шалкера не ставится: у первого
    // злость — свойство 17 (чёрный ящик, enderman-stare, shulker-ai).
    let aggressive = u8::from((mob.kind.hostile() && !matches!(mob.kind, Kind::Enderman | Kind::Shulker | Kind::Slime | Kind::MagmaCube) && mob.h.target.is_some()) || (mob.kind == Kind::Wolf && mob.tame.melee)) << 2;
    let mut out: Vec<Property> = vec![
        (0, TYPE_BYTE, vec![fire]),
        (2, TYPE_OPT_COMPONENT, custom_name(mob)),
        (4, TYPE_BOOL, vec![u8::from(mob.silent)]),
        (6, TYPE_POSE, encode_varint(if mob.is_dead() { POSE_DYING } else { 0 })),
        (9, TYPE_FLOAT, float(mob.vitals.health)),
        (15, TYPE_BYTE, vec![u8::from(mob.no_ai) | aggressive]),
    ];

    // Утопленник с занесённым трезубцем «использует предмет» (чёрный ящик,
    // drowned-trident: 8=1 с захвата цели).
    if mob.kind == Kind::Drowned {
        out.insert(4, (8, TYPE_BYTE, vec![u8::from(mob.h.using)]));
    }

    // Частицы эффектов (10): у ведьмы — от выпитых зелий (чёрный ящик,
    // witch-stand: `10=[entity_effect]` в такте конца питья).
    if matches!(mob.kind, Kind::Witch | Kind::Villager | Kind::ZombieVillager | Kind::CaveSpider | Kind::Silverfish) {
        let mut particles = Vec::new();

        super::vitals::push_effect_particles(&mut particles, &mob.vitals.effect_colors());
        // Ключ и вид уже в записи — оставляем одно значение.
        particles.drain(..1 + encode_varint(TYPE_PARTICLES).len());
        out.insert(5, (10, TYPE_PARTICLES, particles));
    }

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
        // Зомби, кадавр и утопленник (чёрный ящик, zombie-to-drowned: 18=true на
        // 602-м такте — зомби трясётся перед превращением).
        Kind::Zombie | Kind::Husk | Kind::Drowned => {
            out.push((16, TYPE_BOOL, vec![u8::from(mob.is_baby())]));
            out.push((18, TYPE_BOOL, vec![u8::from(mob.h.converting > 0)]));
        }
        // Зомби-житель (чёрный ящик, zombie-villager-stand/-cure): 19 — обращается,
        // 20 — данные жителя, 21 — «villager_finalized».
        Kind::ZombieVillager => {
            let data = &mob.villager;
            let mut value = encode_varint(i32::from(data.kind));

            value.extend_from_slice(&encode_varint(i32::from(data.profession)));
            value.extend_from_slice(&encode_varint(i32::from(data.level)));
            out.push((16, TYPE_BOOL, vec![u8::from(mob.is_baby())]));
            out.push((19, TYPE_BOOL, vec![u8::from(data.conversion > 0)]));
            out.push((20, TYPE_VILLAGER_DATA, value));
            out.push((21, TYPE_BOOL, vec![u8::from(data.finalized)]));
        }
        Kind::Skeleton => out.push((16, TYPE_BOOL, vec![0])),
        // Мобы Незера (minecraft-data 26.1, metadataKeys) [проверить по чёрному ящику, *-stand]:
        // ифрит — флаги (1 — горит заряд), гаст — заряжает, куб — размер,
        // пиглин — неуязвим к превращению, детёныш, арбалет, танец; хоглин —
        // неуязвим, детёныш; страйдер — ускорение, мёрзнет, седло.
        Kind::Blaze => out.push((16, TYPE_BYTE, vec![u8::from(mob.nether.charging)])),
        Kind::Ghast => out.push((16, TYPE_BOOL, vec![u8::from(mob.nether.charging)])),
        Kind::MagmaCube | Kind::Slime => out.push((16, TYPE_INT, encode_varint(mob.nether.size.max(1)))),
        Kind::WitherSkeleton | Kind::Silverfish => {}
        Kind::ZombifiedPiglin => out.push((16, TYPE_BOOL, vec![u8::from(mob.is_baby())])),
        Kind::Piglin => {
            out.push((16, TYPE_BOOL, vec![u8::from(mob.nether.immune)]));
            out.push((17, TYPE_BOOL, vec![u8::from(mob.is_baby())]));
            out.push((18, TYPE_BOOL, vec![0]));
            out.push((19, TYPE_BOOL, vec![0]));
        }
        Kind::PiglinBrute => out.push((16, TYPE_BOOL, vec![u8::from(mob.nether.immune)])),
        Kind::Hoglin => {
            out.push((17, TYPE_BOOL, vec![u8::from(mob.is_baby())]));
            out.push((18, TYPE_BOOL, vec![u8::from(mob.nether.immune)]));
        }
        Kind::Strider => {
            out.push((16, TYPE_INT, encode_varint(0)));
            out.push((17, TYPE_BOOL, vec![u8::from(mob.nether.cold)]));
            out.push((18, TYPE_BOOL, vec![u8::from(mob.nether.saddled)]));
        }
        Kind::Creeper => {
            out.push((16, TYPE_INT, encode_varint(if mob.h.swell_dir == 0 { -1 } else { mob.h.swell_dir })));
            out.push((17, TYPE_BOOL, vec![0]));
            out.push((18, TYPE_BOOL, vec![u8::from(mob.h.lit)]));
        }
        Kind::Spider | Kind::CaveSpider => out.push((16, TYPE_BYTE, vec![u8::from(mob.h.climbing)])),
        // Фаза дракона: 10 — висит (minecraft-data: phase; чёрный ящик,
        // dragon-summon: призванный фазы не шлёт — она по умолчанию).
        Kind::EnderDragon => out.push((16, TYPE_INT, encode_varint(i32::from(mob.dragon.phase)))),
        Kind::EndCrystal | Kind::ShulkerBullet | Kind::LeashKnot => {}
        // Волк (minecraft-data: flags, owneruuid, interested, collar_color,
        // anger_end_time, variant, sound_variant; чёрный ящик, wolf-tame,
        // wolf-defend, probe-cats). Голос — всегда обычный [проверить: у
        // естественных волков он случаен].
        Kind::Wolf => {
            let tame = &mob.tame;

            out.push((16, TYPE_BOOL, vec![u8::from(mob.is_baby())]));
            out.push((18, TYPE_BYTE, vec![tame.flags()]));
            out.push((19, TYPE_OPT_UUID, owner_uuid(tame.owner)));
            out.push((20, TYPE_BOOL, vec![u8::from(tame.begging)]));
            out.push((21, TYPE_INT, encode_varint(i32::from(tame.collar))));
            out.push((22, TYPE_LONG, encode_varlong(if tame.anger_until > tame.clock { tame.anger_until } else { -1 })));
            out.push((23, TYPE_WOLF_VARIANT, encode_varint(i32::from(tame.variant))));
        }
        // Кошка (minecraft-data: flags, owneruuid, variant, is_lying,
        // relax_state_one, collar_color, sound_variant; чёрный ящик, cat-tame).
        Kind::Cat => {
            let tame = &mob.tame;

            out.push((16, TYPE_BOOL, vec![u8::from(mob.is_baby())]));
            out.push((18, TYPE_BYTE, vec![tame.flags()]));
            out.push((19, TYPE_OPT_UUID, owner_uuid(tame.owner)));
            out.push((20, TYPE_CAT_VARIANT, encode_varint(i32::from(tame.variant))));
            out.push((21, TYPE_BOOL, vec![0]));
            out.push((22, TYPE_BOOL, vec![0]));
            out.push((23, TYPE_INT, encode_varint(i32::from(tame.collar))));
        }
        // Эндермен (minecraft-data: carry_state, creepy, stared_at): блок в
        // руках — Optional BlockState: само состояние, 0 (воздух) — пусто
        // (чёрный ящик, enderman-stand: дёрн без снега — 16=9); кричит
        // (сердит), на него смотрят.
        Kind::Enderman => {
            out.push((16, TYPE_OPT_BLOCK_STATE, encode_varint(mob.h.carried.unwrap_or(0))));
            out.push((17, TYPE_BOOL, vec![u8::from(mob.h.target.is_some())]));
            out.push((18, TYPE_BOOL, vec![u8::from(mob.h.stared_at)]));
        }
        // Шалкер (minecraft-data: attach_face, peek, color).
        Kind::Shulker => {
            out.push((16, TYPE_DIRECTION, encode_varint(i32::from(mob.h.face))));
            out.push((17, TYPE_BYTE, vec![mob.h.peek]));
            out.push((18, TYPE_BYTE, vec![mob.h.shell_color]));
        }
        // Ведьма (minecraft-data: is_celebrating, using_item): пьёт ли.
        Kind::Witch => {
            out.push((16, TYPE_BOOL, vec![0]));
            out.push((17, TYPE_BOOL, vec![u8::from(mob.h.drinking > 0)]));
        }
        // Житель (minecraft-data: baby, age_locked, unhappy_counter,
        // villager_data, villager_finalized; чёрный ящик, villager-shake):
        // мотание головой — счёт 40 вниз, данные — тип, профессия, уровень.
        Kind::Villager => {
            let data = &mob.villager;
            let mut value = encode_varint(i32::from(data.kind));

            value.extend_from_slice(&encode_varint(i32::from(data.profession)));
            value.extend_from_slice(&encode_varint(i32::from(data.level)));
            out.push((16, TYPE_BOOL, vec![u8::from(mob.is_baby())]));
            out.push((17, TYPE_BOOL, vec![0]));
            out.push((18, TYPE_INT, encode_varint(data.unhappy)));
            out.push((19, TYPE_VILLAGER_DATA, value));
            out.push((20, TYPE_BOOL, vec![u8::from(data.finalized)]));
        }
    }

    out
}

/// Имя моба (свойство 2, Optional Text Component): нет — 0, есть — 1 и текст
/// (чёрный ящик, name-tag: `2={"type":"string","value":"Бобик"}`).
fn custom_name(mob: &Mob) -> Vec<u8> {
    match &mob.name {
        None => vec![0],
        Some(name) => {
            let mut out = vec![1];

            out.extend_from_slice(&nbt::text_component(name));
            out
        }
    }
}

/// Хозяин (Optional UUID): нет — 0, есть — 1 и 16 байт.
fn owner_uuid(owner: Option<[u8; 16]>) -> Vec<u8> {
    match owner {
        None => vec![0],
        Some(uuid) => {
            let mut out = vec![1];

            out.extend_from_slice(&uuid);
            out
        }
    }
}

/// Значения по умолчанию: пока свойство такое, при появлении оно не
/// присылается. Варианты животных у нас всегда обычные, и клиент знает
/// их сам.
fn is_default(entry: &Property, kind: Kind) -> bool {
    let (key, _, value) = entry;

    match (*key, kind) {
        (2, _) => value.as_slice() == [0],
        // Волк и кошка: ошейник красный, вариант — бледный и чёрная, злости нет
        // (чёрный ящик, probe-cats).
        (18..=20, Kind::Wolf) | (18 | 19 | 21 | 22, Kind::Cat) => value.iter().all(|byte| *byte == 0),
        (21, Kind::Wolf) | (23, Kind::Cat) => value.as_slice() == encode_varint(i32::from(crate::mob::tame::RED)).as_slice(),
        (22, Kind::Wolf) => value.as_slice() == encode_varlong(-1).as_slice(),
        (23, Kind::Wolf) => value.as_slice() == encode_varint(i32::from(crate::mob::tame::WOLF_DEFAULT)).as_slice(),
        (20, Kind::Cat) => value.as_slice() == encode_varint(i32::from(crate::mob::tame::CAT_DEFAULT)).as_slice(),
        (8, Kind::EndCrystal | Kind::Drowned) => value.as_slice() == [0],
        (9, Kind::EndCrystal) => value.as_slice() == [1],
        (16, Kind::MagmaCube | Kind::Slime) => value.as_slice() == encode_varint(1).as_slice(),
        (17..=19, Kind::Piglin | Kind::Hoglin | Kind::Strider) => value.iter().all(|byte| *byte == 0),
        (16, Kind::EnderDragon) => value.as_slice() == encode_varint(DRAGON_PHASE_HOVER).as_slice(),
        (16, Kind::Creeper) => value.as_slice() == encode_varint(-1).as_slice(),
        (18, Kind::Shulker) => value.as_slice() == [16],
        (17 | 18, Kind::Enderman | Kind::Shulker | Kind::Witch) => value.iter().all(|byte| *byte == 0),
        // Житель по умолчанию — равнинный без профессии, новичок (чёрный ящик,
        // villager-shake: призванный без данных шлёт только 9 и 15).
        (19, Kind::ZombieVillager) => value.as_slice() == [0],
        (20, Kind::ZombieVillager) => value.as_slice() == [crate::villager::PLAINS, crate::villager::NONE, 1],
        (21, Kind::ZombieVillager) => value.as_slice() == [0],
        (19, Kind::Villager) => value.as_slice() == [crate::villager::PLAINS, crate::villager::NONE, 1],
        (17 | 18 | 20, Kind::Villager) => value.iter().all(|byte| *byte == 0),
        (0 | 4 | 6 | 15 | 16, _) => value.iter().all(|byte| *byte == 0),
        (9, _) => value.as_slice() == float(1.0),
        (10, _) => value.as_slice() == [0],
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
    // Узел поводка появляется в углу своего блока (чёрный ящик, lead-attach:
    // появление в целых x, y, z забора).
    let place = if mob.kind == Kind::LeashKnot { (mob.x.floor(), mob.y.floor(), mob.z.floor()) } else { (mob.x, mob.y, mob.z) };

    push_f64(&mut body, place.0);
    push_f64(&mut body, place.1);
    push_f64(&mut body, place.2);
    types::push_velocity(&mut body, mob.vx, mob.vy, mob.vz);
    body.push(cart_angle(mob.pitch));
    body.push(cart_angle(mob.yaw));
    body.push(cart_angle(mob.head_yaw));
    // Данные появления: у снаряда шалкера — номер стрелявшего (чёрный ящик,
    // shulker-ai: data = номер шалкера).
    body.extend_from_slice(&encode_varint(if mob.kind == Kind::ShulkerBullet { mob.h.owner } else { 0 }));
    write_body(stream, body).await?;

    let properties = properties(mob);
    let changed: Vec<&Property> = properties.iter().filter(|entry| !is_default(entry, mob.kind)).collect();

    send_properties(stream, mob.entity_id, &changed).await?;

    send_mob_equipment(stream, mob).await?;

    if mob.spawn_poof {
        send_status(stream, mob.entity_id, 20).await?;
    }

    // Зомби и эндермен — атрибуты сразу при появлении (чёрный ящик,
    // zombie-hit, enderman-stand).
    if attributes_at_spawn(mob.kind) {
        send_attributes(stream, mob).await?;
    }

    // Дикий волк — наибольшее здоровье сразу (чёрный ящик, wolf-tame:
    // max_health 8 при появлении; у призванного с хозяином — нет).
    if wild_wolf(mob) {
        send_max_health(stream, mob).await?;
    }

    // На поводке: связь с держателем (Link Entities).
    if let Some(holder) = mob.leash {
        send_leash(stream, mob.entity_id, holder.id()).await?;
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
        attributes_sent: attributes_at_spawn(mob.kind),
        modded: modded(mob),
        first: true,
        properties,
        max_health: mob.vitals.max_health,
    })
}

/// Дикий волк: ему наибольшее здоровье шлётся при появлении.
fn wild_wolf(mob: &Mob) -> bool {
    mob.kind == Kind::Wolf && !mob.tame.tamed()
}

/// Атрибут «наибольшее здоровье» один (чёрный ящик, wolf-tame).
async fn send_max_health(stream: &mut Outgoing<'_>, mob: &Mob) -> io::Result<()> {
    let mut body = encode_varint(UPDATE_ATTRIBUTES);

    body.extend_from_slice(&encode_varint(mob.entity_id));
    body.extend_from_slice(&encode_varint(1));
    body.extend_from_slice(&encode_varint(ATTRIBUTE_MAX_HEALTH));
    push_f64(&mut body, f64::from(mob.vitals.max_health));
    push_modifiers(&mut body, None);
    write_body(stream, body).await
}

/// Link Entities (0x64): моб на поводке у держателя; 0 — поводок снят
/// (чёрный ящик, lead-attach: `vehicleId 0` при обрыве).
async fn send_leash(stream: &mut Outgoing<'_>, entity_id: i32, holder: i32) -> io::Result<()> {
    let mut body = encode_varint(LINK_ENTITIES);

    body.extend_from_slice(&entity_id.to_be_bytes());
    body.extend_from_slice(&holder.to_be_bytes());
    write_body(stream, body).await
}

/// Скорость шага в пакете атрибутов: как у вида, но у шалкера, что не
/// ходит, — 0,7, значение атрибута по умолчанию (чёрный ящик, shulker-stand).
fn attribute_speed(kind: Kind) -> f64 {
    if matches!(kind, Kind::Shulker | Kind::Ghast) { 0.7 } else { kind.speed() }
}

/// Атрибуты: сообщается одна скорость шага (у оригинала так же: клиенту
/// нужен только изменённый), базовое значение — одинарной точности.
async fn send_attributes(stream: &mut Outgoing<'_>, mob: &Mob) -> io::Result<()> {
    let mut body = encode_varint(UPDATE_ATTRIBUTES);

    body.extend_from_slice(&encode_varint(mob.entity_id));

    // Магмовый куб: броня 3·размер, скорость 0,2 + 0,1·размер, здоровье
    // размер² (чёрный ящик, magma-cube-stand).
    if crate::mob::slime::cube(mob.kind) {
        let size = f64::from(mob.nether.size.max(1));

        body.extend_from_slice(&encode_varint(if mob.kind == Kind::MagmaCube { 3 } else { 2 }));
        if mob.kind == Kind::MagmaCube {
            body.extend_from_slice(&encode_varint(ATTRIBUTE_ARMOR));
            push_f64(&mut body, 3.0 * size);
            push_modifiers(&mut body, None);
        }
        body.extend_from_slice(&encode_varint(ATTRIBUTE_MOVEMENT_SPEED));
        push_f64(&mut body, crate::mob::slime::speed(mob.nether.size));
        push_modifiers(&mut body, None);
        body.extend_from_slice(&encode_varint(ATTRIBUTE_MAX_HEALTH));
        push_f64(&mut body, size * size);
        push_modifiers(&mut body, None);

        return write_body(stream, body).await;
    }

    body.extend_from_slice(&encode_varint(1));
    body.extend_from_slice(&encode_varint(ATTRIBUTE_MOVEMENT_SPEED));
    push_f64(&mut body, f64::from(attribute_speed(mob.kind) as f32));

    // Пьющая ведьма: модификатор «minecraft:drinking» −0,25; сердитый
    // эндермен — «minecraft:attacking» +0,15; оба — сложением (чёрный ящик,
    // witch-stand, enderman-stare).
    let modifier = match mob.kind {
        Kind::Witch if mob.h.drinking > 0 => Some(("minecraft:drinking", -0.25, 0)),
        Kind::Enderman if mob.h.target.is_some() => Some(("minecraft:attacking", crate::mob::hostile::ATTACKING_BONUS, 0)),
        // Детёныш-зомби быстрее: «minecraft:baby» +50 % базы, операция 1
        // (чёрный ящик, zombie-villager-stand).
        Kind::Zombie | Kind::ZombieVillager | Kind::Drowned | Kind::Husk if mob.is_baby() => Some(("minecraft:baby", 0.5, 1)),
        _ => None,
    };

    push_modifiers(&mut body, modifier);
    write_body(stream, body).await
}

/// Модификаторы атрибута: ни одного или один (имя, прибавка, сложение).
fn push_modifiers(body: &mut Vec<u8>, modifier: Option<(&str, f64, u8)>) {
    match modifier {
        Some((name, amount, operation)) => {
            body.extend_from_slice(&encode_varint(1));
            body.extend_from_slice(&encode_string(name));
            push_f64(body, amount);
            body.push(operation);
        }
        None => body.extend_from_slice(&encode_varint(0)),
    }
}

/// Броня шалкера: база 0, у закрытого — «minecraft:covered» +20 (чёрный
/// ящик, shulker-ai: открылся — броня без модификаторов, закрылся — с ним).
async fn send_shulker_armor(stream: &mut Outgoing<'_>, mob: &Mob) -> io::Result<()> {
    let mut body = encode_varint(UPDATE_ATTRIBUTES);

    body.extend_from_slice(&encode_varint(mob.entity_id));
    body.extend_from_slice(&encode_varint(1));
    body.extend_from_slice(&encode_varint(ATTRIBUTE_ARMOR));
    push_f64(&mut body, 0.0);
    push_modifiers(&mut body, mob.h.covered.then_some(("minecraft:covered", 20.0, 0)));
    write_body(stream, body).await
}

/// Состояние, что меняет атрибуты на лету: ведьма пьёт, эндермен сердит,
/// шалкер закрыт.
fn modded(mob: &Mob) -> bool {
    match mob.kind {
        Kind::Witch => mob.h.drinking > 0,
        Kind::Zombie | Kind::ZombieVillager | Kind::Drowned | Kind::Husk => mob.is_baby(),
        Kind::Enderman => mob.h.target.is_some(),
        Kind::Shulker => mob.h.peek == 0,
        _ => false,
    }
}

/// Показывает клиенту всех мобов вокруг: о них журнал не скажет — они
/// появились раньше.
pub(super) async fn send_all_mobs(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState) -> io::Result<()> {
    player.seen_mobs.clear();
    player.seen_bow_arrows.clear();
    player.seen_thrown.clear();
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
        Event::Status(status) => send_status(stream, entity_id, *status).await?,
        Event::Leash(holder) => send_leash(stream, entity_id, *holder).await?,
        Event::Sound { name, volume, pitch } if heard(at) => {
            let category = if name.starts_with("minecraft:entity.slime.") { 5 } else { SOUND_NEUTRAL };
            send_sound(stream, name, category, at, *volume, *pitch).await?;
        }
        Event::SoundAt { name, volume, pitch, at: place } if heard(*place) => {
            let category = if name.starts_with("minecraft:entity.slime.") { 5 } else { SOUND_NEUTRAL };
            send_sound(stream, name, category, *place, *volume, *pitch).await?;
        }
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
        Event::Sound { .. } | Event::SoundAt { .. } | Event::PlayerSound { .. } | Event::EntitySound { .. } | Event::Loot(_) => {}
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

    // Утопленник: основная рука — трезубец, ракушка или удочка; вторая — ракушка.
    if let Some(item) = crate::mob::hostile::hand_item(mob.h.hand).and_then(blocks::item_named) {
        slots.push((EQUIPMENT_MAIN_HAND, Stack::new(item, 1)));
    }

    if mob.h.offhand_shell && let Some(shell) = blocks::item_named("nautilus_shell") {
        slots.push((EQUIPMENT_OFF_HAND, Stack::new(shell, 1)));
    }

    if mob.h.helmet && let Some(helmet) = blocks::item_named("iron_helmet") {
        slots.push((EQUIPMENT_HEAD, Stack::new(helmet, 1)));
    }

    if slots.is_empty() {
        return Ok(());
    }

    send_equipment_slots(stream, mob.entity_id, &slots).await
}

/// Зелье в руке пьющей ведьмы (чёрный ящик, witch-stand: `potion` с
/// potion_contents, по окончании — пустая рука).
async fn send_witch_hand(stream: &mut Outgoing<'_>, mob: &Mob) -> io::Result<()> {
    let held = if mob.h.drinking > 0 {
        blocks::item_named("potion").map(|item| Stack::new(item, 1).with_extras(crate::stackdata::Data { potion: Some(mob.h.potion), ..Default::default() }))
    } else {
        None
    };
    let mut body = encode_varint(SET_EQUIPMENT);

    body.extend_from_slice(&encode_varint(mob.entity_id));
    body.push(EQUIPMENT_MAIN_HAND);
    push_slot(&mut body, held);
    write_body(stream, body).await
}

/// Пакет снаряжения: места и стопки.
async fn send_equipment_slots(stream: &mut Outgoing<'_>, entity_id: i32, slots: &[(u8, Stack)]) -> io::Result<()> {

    let mut body = encode_varint(SET_EQUIPMENT);

    body.extend_from_slice(&encode_varint(entity_id));

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

/// Показывает только что вышедшего с облачком моба (`WorldEvent::MobSpawn`):
/// появление, данные и событие 20; сверка места — позже, в слежении. Моб
/// приходит копией: в мирной сложности его уберут в этом же такте.
pub(super) async fn send_released_mob(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState, mob: &Mob) -> io::Result<()> {
    let range = tracking_range(shared);

    if (mob.x - player.x).abs() <= range && (mob.z - player.z).abs() <= range && !player.seen_mobs.contains_key(&mob.entity_id) {
        let shown = send_mob(stream, mob).await?;

        player.seen_mobs.insert(mob.entity_id, shown);
    }

    Ok(())
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
        // Изменившиеся свойства гонят слежение вне очереди: скорость, сдвиг и
        // поворот уходят в тот же такт (чёрный ящик, enderman-stare: скорость
        // 0,-0.078 вместе с 18=true на +1, а не на +3; shulker-ai: поворот
        // головы вместе с раскрытием).
        let dirty = !shown.first && properties(mob) != shown.properties;
        let due = mob.ticks % EVERY == 0 || dirty;

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
                // Кристалл Края — не живое существо: нулевой сдвиг раз в 60 тактов у него не
                // шлётся (чёрный ящик, dragon-crystal-heal: кристалл стоял 80 тактов, у оригинала
                // на +60 ничего, у дракона рядом — шлётся); сдвиг при появлении (такт 0) — как у всех
                // [проверить: другие неживые].
                let forced = mob.ticks % FORCED == 0 && (!matches!(mob.kind, Kind::EndCrystal | Kind::LeashKnot) || mob.ticks == 0);
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
            // У дракона здоровье в первый прогон уходит и равное единице: оно
            // «грязное» с рождения (чёрный ящик, dragon-death: `9=1 15=1`).
            now.iter().filter(|entry| !is_default(entry, mob.kind) || (mob.kind == Kind::EnderDragon && entry.0 == 9)).collect()
        } else {
            now.iter().filter(|entry| !shown.properties.contains(entry)).collect()
        };

        send_properties(stream, mob.entity_id, &changed).await?;
        shown.properties = now;

        // Ведьма начала или кончила пить, эндермен рассердился или остыл,
        // шалкер открылся или закрылся: атрибуты (у ведьмы — и рука).
        if modded(mob) != shown.modded {
            if mob.kind == Kind::Shulker {
                send_shulker_armor(stream, mob).await?;
            } else {
                send_attributes(stream, mob).await?;
            }

            if mob.kind == Kind::Witch {
                send_witch_hand(stream, mob).await?;
            }

            shown.modded = modded(mob);
        }

        if shown.first && attributes_at_spawn(mob.kind) {
            send_attributes(stream, mob).await?;
        }

        // Волк: наибольшее здоровье — второй раз в первом прогоне (дикий) и
        // при перемене (приручение: 40; чёрный ящик, wolf-tame).
        if (shown.first && wild_wolf(mob)) || (mob.kind == Kind::Wolf && mob.vitals.max_health != shown.max_health) {
            send_max_health(stream, mob).await?;
            shown.max_health = mob.vitals.max_health;
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

    send_orb_ticks(stream, player, &orbs).await
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
        modded: false,
        first: true,
        properties,
        max_health: 0.0,
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

#[cfg(test)]
mod slime_tests {
    use super::*;

    #[test]
    fn slime_metadata_uses_size_not_baby_and_omits_aggressive_flag() {
        let mut mob = Mob::new(1, Kind::Slime, (0.0, 0.0, 0.0));
        crate::mob::slime::set_size(&mut mob, 4);
        mob.h.target = Some(7);
        let props = properties(&mob);
        assert!(props.contains(&(16, TYPE_INT, encode_varint(4))));
        assert!(props.contains(&(9, TYPE_FLOAT, float(16.0))));
        assert!(props.contains(&(15, TYPE_BYTE, vec![0])));
        assert!(attributes_at_spawn(Kind::Slime));
        assert!(!is_default(&(16, TYPE_INT, encode_varint(4)), Kind::Slime));
        assert!(is_default(&(16, TYPE_INT, encode_varint(1)), Kind::Slime));
    }
}
