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
const ATTRIBUTE_ARMOR_TOUGHNESS: i32 = 1;
const ATTRIBUTE_MAX_HEALTH: i32 = 19;
const ATTRIBUTE_OXYGEN_BONUS: i32 = 23;

/// Раз во сколько тактов от появления моб рассылает место и скорость, и раз
/// во сколько — место, даже если оно не изменилось (как у вагонеток).
const EVERY: u64 = 3;
const FORCED: u64 = 60;

/// На каком такте жизни уходят атрибуты: у животных на третьем, у крипера на
/// первом (чёрный ящик, creeper-explode); у зомби — сразу при появлении.
fn attributes_at(mob: &Mob) -> u64 {
    match mob.kind {
        // Чёрный ящик water-spawn-all: у дельфина и взрослой черепахи на 2-м такте.
        Kind::Dolphin => 1,
        Kind::Turtle if !mob.is_baby() => 1,
        Kind::Creeper => 1,
        // Вредина: атрибуты через 2 такта после появления (чёрный ящик, evoker-vex) [проверить].
        Kind::Vex => 1,
        // Кристаллу и дракону атрибуты не шлются вовсе (чёрный ящик,
        // end-crystal-explode, dragon-summon).
        Kind::EndCrystal | Kind::EnderDragon | Kind::ShulkerBullet | Kind::LeashKnot | Kind::EvokerFangs => u64::MAX,
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
    effect_levels: [i32; 4],
    soul_level: i32,
    /// Броня волка, как показана; перемена шлёт снаряжение, а атрибуты
    /// следом — на ближайшем такте жизни, кратном трём (`armor_pending`).
    wolf_armor: Option<i32>,
    armor_pending: bool,
    gear_version: u32,
    /// Стойка: снаряжение и модификаторы брони, как показаны.
    stand_gear: [Option<Stack>; 6],
    stand_mods: Vec<(i32, &'static str, f64)>,
    attributes_sent: bool,

    /// `modded` — как показано клиенту (атрибуты, у ведьмы и рука).
    modded: bool,

    /// Слежение ещё не прогонялось: свойства уйдут ещё раз целиком.
    first: bool,
    properties: Vec<Property>,

    /// Наибольшее здоровье, как показано (у волка оно меняется при
    /// приручении — атрибут max_health; чёрный ящик, wolf-tame).
    max_health: f32,
    /// Сколько выдач эффекта клиент уже видел (свойство 10 уходит заново).
    effect_gives: u32,
    harness: Option<u8>,
    cargo: Option<Stack>,

    /// Показана полоса босса (иссушитель): уходит вместе с ним.
    pub(super) boss: bool,
    /// Снаряжение, как показано (разбойник: заряд и износ арбалета).
    gear: u32,
    mouth: Option<Stack>,
    /// Предмет в руке тихони, как показан (mob/small.rs).
    held: Option<Stack>,
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
    // Свечение (0x40): у разбойников после звона колокола (raid.rs);
    // невидимость (0x20): торговец, выпивший зелье.
    let fire = u8::from(mob.vitals.fire > 0) | if mob.vitals.has_effect(crate::survival::EffectKind::Glowing) { 0x40 } else { 0 } | if mob.land.invisible || mob.vitals.has_effect(crate::survival::EffectKind::Invisibility) { 0x20 } else { 0 };

    // Кристалл Края — не живое: флаги, тишина, цель луча (11 — Optional
    // BlockPos: 0 — нет), «показывать подножие» (minecraft-data: beam_target,
    // show_bottom; по умолчанию подножие есть — чёрный ящик,
    // end-crystal-explode: призванный свойств не шлёт).
    if mob.kind==Kind::ArmorStand {
        // Здоровье 20 уходит всегда (по умолчанию 1): чёрный ящик armorstand-equip-break: `9=20 15=12 16=…`.
        let mut out=vec![(0,TYPE_BYTE,vec![fire|if mob.stand.invisible {32} else {0}]),(5,TYPE_BOOL,vec![u8::from(mob.stand.no_gravity)]),(9,TYPE_FLOAT,float(20.0)),(15,TYPE_BYTE,vec![mob.stand.flags()])];
        for (i,p) in mob.stand.pose.iter().enumerate() {out.push((16+i as u8,9,p.iter().flat_map(|v|v.to_be_bytes()).collect()));}
        return out;
    }
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
    if matches!(mob.kind, Kind::ShulkerBullet | Kind::LeashKnot | Kind::EvokerFangs) {
        return vec![(0, TYPE_BYTE, vec![fire]), (4, TYPE_BOOL, vec![u8::from(mob.silent)])];
    }
    // «Агрессивен» (бит 4) у эндермена и шалкера не ставится: у первого
    // злость — свойство 17 (чёрный ящик, enderman-stare, shulker-ai).
    let aggressive = u8::from(((mob.kind == Kind::Bee && mob.wild.chase) || mob.kind.hostile() && !matches!(mob.kind, Kind::Enderman | Kind::Shulker | Kind::Slime | Kind::MagmaCube | Kind::Guardian | Kind::ElderGuardian | Kind::Pillager | Kind::Evoker | Kind::Illusioner | Kind::Vex | Kind::Breeze | Kind::Warden) && mob.h.target.is_some() && !mob.illager.calm) || (mob.kind == Kind::Wolf && mob.tame.melee) || (mob.kind == Kind::IronGolem && mob.golem.target.is_some())) << 2;
    let mut out: Vec<Property> = vec![
        (0, TYPE_BYTE, vec![fire]),
        (2, TYPE_OPT_COMPONENT, custom_name(mob)),
        (4, TYPE_BOOL, vec![u8::from(mob.silent)]),
        (6, TYPE_POSE, encode_varint(if mob.is_dead() { POSE_DYING } else if mob.kind == Kind::Camel && mob.land.sitting { 10 } else if mob.kind == Kind::Breeze { mob.breeze.pose } else if mob.kind == Kind::Warden { mob.warden.pose.id() } else if mob.kind == Kind::Frog && mob.water.tongue > 0 { 9 } else { 0 })),
        (9, TYPE_FLOAT, float(mob.vitals.health)),
        // Стрел в теле (wither-half-armor: 9=… 12=1 после попадания).
        (12, TYPE_INT, encode_varint(mob.arrows)),
        (15, TYPE_BYTE, vec![u8::from(mob.no_ai) | aggressive]),
    ];

    if crate::mob::water::creature(mob.kind) { out.push((1,TYPE_INT,encode_varint(mob.water.air))); }
    if mob.kind == Kind::Warden { out.push((16, TYPE_INT, encode_varint(i32::from(mob.warden.client_anger)))); }

    // Утопленник с занесённым трезубцем «использует предмет» (чёрный ящик,
    // drowned-trident: 8=1 с захвата цели).
    if mob.kind == Kind::Drowned || mob.kind.skeleton() || mob.kind == Kind::WanderingTrader {
        out.insert(4, (8, TYPE_BYTE, vec![u8::from(mob.h.using || mob.h.drawing > 0 || mob.land.drinking > 0)]));
    }

    // Торговец пьёт зелье невидимости или молоко.
    if mob.kind == Kind::WanderingTrader {
        out.insert(4, (8, TYPE_BYTE, vec![u8::from(mob.land.drinking > 0)]));
    }

    // Разбойник заряжает арбалет — «рука занята» (чёрный ящик, pillager-crossbow: 8=1 17=true).
    if mob.kind == Kind::Pillager {
        out.insert(4, (8, TYPE_BYTE, vec![u8::from(crate::mob::illager::charging(mob))]));
    }

    // Частицы эффектов (10): у ведьмы — от выпитых зелий (чёрный ящик,
    // witch-stand: `10=[entity_effect]` в такте конца питья).
    {
        let mut particles = Vec::new();

        super::vitals::push_effect_particles(&mut particles, &mob.vitals.mob_effect_colors());
        // Ключ и вид уже в записи — оставляем одно значение.
        particles.drain(..1 + encode_varint(TYPE_PARTICLES).len());
        out.insert(5, (10, TYPE_PARTICLES, particles));
    }

    // minecraft-data 26.1: AgeableMob.age_locked, индекс 17.
    if crate::mob::growth::eligible(mob.kind) { out.push((17, TYPE_BOOL, vec![u8::from(mob.age_locked)])); }

    if crate::mob::horse::equine(mob.kind) {
        out.push((16, TYPE_BOOL, vec![u8::from(mob.is_baby())]));
        out.push((18, TYPE_BYTE, vec![mob.horse.flags()]));
        if mob.kind == Kind::Horse { out.push((19, TYPE_INT, encode_varint(mob.horse.variant))); }
        if crate::mob::horse::pack(mob.kind) { out.push((19, TYPE_BOOL, vec![u8::from(mob.horse.chest && !mob.is_dead())])); }
    }

    // Пчела в полёте: «без гравитации» (чёрный ящик, bee-sting: 5=true на +12; hive-exit).
    if mob.kind == Kind::Bee && mob.wild.flying {
        out.insert(3, (5, TYPE_BOOL, vec![1]));
    }

    match mob.kind {
        Kind::ArmorStand => {},
        // Подушка 26.3: цвет в metadata с индексом 8 (minecraft-data 26.3, entities.json); тип — [проверить].
        Kind::Cushion => out.push((8, TYPE_INT, encode_varint(i32::from(mob.color)))),
        Kind::Llama | Kind::TraderLlama | Kind::Camel | Kind::Goat | Kind::Panda | Kind::PolarBear | Kind::Ocelot | Kind::Mooshroom | Kind::WanderingTrader => {
            let s = &mob.land;
            out.push((16, TYPE_BOOL, vec![u8::from(mob.is_baby())]));
            match mob.kind {
                Kind::Llama | Kind::TraderLlama => {
                    out.push((18, TYPE_BYTE, vec![u8::from(s.tamed) << 1]));
                    out.push((19, TYPE_BOOL, vec![u8::from(s.chest)]));
                    out.push((20, TYPE_INT, encode_varint(i32::from(s.strength))));
                    out.push((21, TYPE_INT, encode_varint(i32::from(s.variant))));
                }
                Kind::Camel => {
                    out.push((18, TYPE_BYTE, vec![4 * u8::from(s.saddle)]));
                    out.push((19, TYPE_BOOL, vec![u8::from(s.dash > 0)]));
                    let v=encode_varlong(s.pose_tick);
                    out.push((20, TYPE_LONG, v));
                }
                Kind::Goat => {
                    out.push((18, TYPE_BOOL, vec![u8::from(s.screaming)]));
                    out.push((19, TYPE_BOOL, vec![u8::from(s.horns[0] && !mob.is_baby())]));
                    out.push((20, TYPE_BOOL, vec![u8::from(s.horns[1] && !mob.is_baby())]));
                }
                Kind::Panda => {
                    out.push((18, TYPE_INT, encode_varint(0)));
                    out.push((19, TYPE_INT, encode_varint(s.sneeze)));
                    out.push((20, TYPE_INT, encode_varint(s.eating)));
                    out.push((21, TYPE_BYTE, vec![s.main_gene])); out.push((22, TYPE_BYTE, vec![s.hidden_gene]));
                    out.push((23, TYPE_BYTE, vec![(u8::from(s.sneeze>0)*2) | (u8::from(s.sitting)*4) | (u8::from(s.eating>0 || s.worried)*8) | (u8::from(s.rolling>0)*16)]));
                }
                Kind::PolarBear => out.push((18, TYPE_BOOL, vec![u8::from(s.standing)])),
                Kind::Ocelot => out.push((18, TYPE_BOOL, vec![u8::from(s.trusting)])),
                // Чёрный ящик, mooshroom-shear: вид — число (красный 0, бурый 1), не строка.
                Kind::Mooshroom => out.push((18, TYPE_INT, encode_varint(i32::from(s.brown)))),
                Kind::WanderingTrader => out.push((18, TYPE_INT, encode_varint(0))),
                _ => {}
            }
        }
        Kind::Cod | Kind::Salmon | Kind::TropicalFish | Kind::Pufferfish | Kind::Tadpole => {
            out.push((16, TYPE_BOOL, vec![u8::from(mob.water.from_bucket)]));
            let value = match mob.kind { Kind::Salmon => Some(i32::from(mob.water.salmon_size)), Kind::TropicalFish => Some(mob.water.variant), Kind::Pufferfish => Some(i32::from(mob.water.puff)), _ => None };
            if let Some(value) = value { out.push((17, TYPE_INT, encode_varint(value))); }
        }
        Kind::Squid | Kind::GlowSquid | Kind::Dolphin | Kind::Turtle | Kind::Axolotl | Kind::Frog => {
            out.push((16, TYPE_BOOL, vec![u8::from(mob.is_baby())]));
            match mob.kind {
                Kind::GlowSquid => out.push((18, TYPE_INT, encode_varint(mob.water.dark_ticks))),
                Kind::Dolphin => {
                    out.push((18, TYPE_BOOL, vec![u8::from(mob.water.got_fish)]));
                    out.push((19, TYPE_INT, encode_varint(mob.water.moisture)));
                }
                Kind::Turtle => {
                    out.push((18, TYPE_BOOL, vec![u8::from(mob.water.has_egg)]));
                    out.push((19, TYPE_BOOL, vec![u8::from(mob.water.laying > 0)]));
                }
                Kind::Axolotl => {
                    out.push((18, TYPE_INT, encode_varint(mob.water.variant)));
                    out.push((19, TYPE_BOOL, vec![u8::from(mob.water.playing_dead > 0)]));
                    out.push((20, TYPE_BOOL, vec![u8::from(mob.water.from_bucket)]));
                }
                Kind::Frog => {
                    out.push((18, 27, encode_varint(match mob.water.variant { 1=>2, 2=>0, _=>1 })));
                    out.push((19, 19, encode_varint(if mob.water.tongue > 0 { mob.water.target.unwrap_or(-1)+1 } else { 0 })));
                }
                _ => {}
            }
        }
        Kind::Warden => {},
        Kind::Horse | Kind::Donkey | Kind::Mule | Kind::SkeletonHorse | Kind::ZombieHorse => {}
        Kind::Rabbit | Kind::Fox | Kind::Bee => {
            // minecraft-data 26.1, metadataKeys; [проверить] значения флагов.
            out.push((16, TYPE_BOOL, vec![u8::from(mob.is_baby())]));
            match mob.kind {
                Kind::Rabbit => out.push((18, TYPE_INT, encode_varint(i32::from(mob.wild.variant)))),
                Kind::Fox => {
                    out.push((18, TYPE_INT, encode_varint(i32::from(mob.wild.variant))));
                    out.push((19, TYPE_BYTE, vec![(u8::from(mob.wild.sleeping)<<5) | (u8::from(mob.wild.crouching)<<2) | (u8::from(mob.wild.pouncing)<<4)]));
                    out.push((20, TYPE_OPT_UUID, owner_uuid(mob.wild.trusted[0])));
                    out.push((21, TYPE_OPT_UUID, owner_uuid(mob.wild.trusted[1])));
                }
                _ => {
                    // Флаги пчелы: 2 — катается, 4 — ужалила, 8 — с нектаром.
                    out.push((18, TYPE_BYTE, vec![(u8::from(mob.wild.nectar)<<3) | (u8::from(mob.wild.stung)<<2) | (u8::from(mob.wild.roll)<<1)]));
                    out.push((19, TYPE_LONG, encode_varlong(if mob.wild.anger>0 { mob.tame.clock+i64::from(mob.wild.anger) } else { -1 })));
                }
            }
        }
        // minecraft-data 26.1, metadataKeys и entityMetadataEntry. [проверить]
        Kind::Armadillo => {
            out.push((16, TYPE_BOOL, vec![u8::from(mob.is_baby())]));
            out.push((18, 36, encode_varint(i32::from(mob.small.roll))));
        }
        Kind::Sniffer => {
            out.push((16, TYPE_BOOL, vec![u8::from(mob.is_baby())]));
            out.push((18, 35, encode_varint(i32::from(mob.small.sniff))));
            out.push((19, TYPE_INT, encode_varint(mob.small.drop_seed_at)));
        }
        Kind::Bat => out.push((16, TYPE_BYTE, vec![u8::from(mob.small.resting)])),
        Kind::Parrot => {
            out.push((18, TYPE_BYTE, vec![mob.tame.flags()]));
            out.push((19, TYPE_OPT_UUID, owner_uuid(mob.tame.owner)));
            out.push((20, TYPE_INT, encode_varint(i32::from(mob.small.variant))));
        }
        Kind::Allay => {
            out.push((16, TYPE_BOOL, vec![u8::from(mob.small.dancing)]));
            out.push((17, TYPE_BOOL, vec![u8::from(mob.small.duplicate == 0)]));
        }
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
            // Вики, Egg: «variant depends on which egg variant was thrown».
            // Реестр configuration.rs: cold=0, temperate=1, warm=2.
            out.push((18, TYPE_CHICKEN_VARIANT, encode_varint(match mob.color { 1 => 0, 2 => 2, _ => 1 })));
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
        Kind::Skeleton => out.push((16, TYPE_BOOL, vec![u8::from(mob.h.converting > 0)])),
        Kind::Bogged => out.push((16, TYPE_BOOL, vec![u8::from(mob.sheared)])),
        Kind::Stray => {}
        // Мобы Незера (minecraft-data 26.1, metadataKeys) [проверить по чёрному ящику, *-stand]:
        // ифрит — флаги (1 — горит заряд), гаст — заряжает, куб — размер,
        // пиглин — неуязвим к превращению, детёныш, арбалет, танец; хоглин —
        // неуязвим, детёныш; страйдер — ускорение, мёрзнет, седло.
        Kind::Blaze => out.push((16, TYPE_BYTE, vec![u8::from(mob.nether.charging)])),
        Kind::Ghast => out.push((16, TYPE_BOOL, vec![u8::from(mob.nether.charging)])),
        // Фантом: размер (свойство 16, по умолчанию 0) [проверить по чёрному ящику, phantom-swoop].
        Kind::Phantom => out.push((16, TYPE_INT, encode_varint(mob.phantom.size))),
        Kind::MagmaCube | Kind::Slime => out.push((16, TYPE_INT, encode_varint(mob.nether.size.max(1)))),
        Kind::WitherSkeleton | Kind::Silverfish | Kind::Breeze | Kind::Endermite => {}
        Kind::Zoglin => out.push((16, TYPE_BOOL, vec![u8::from(mob.is_baby())])),
        Kind::CopperGolem => {
            out.push((16, 38, encode_varint(i32::from(mob.newmobs.oxidation))));
            // 1 — берёт из сундука-источника, 3 — кладёт в сундук (copper-golem-sort).
            out.push((17, 37, encode_varint(if mob.newmobs.chest_wait > 0 { if mob.newmobs.cargo.is_some() { 3 } else { 1 } } else { 0 })));
        }
        Kind::Creaking => {
            out.push((16, TYPE_BOOL, vec![u8::from(!mob.newmobs.frozen)]));
            out.push((17, TYPE_BOOL, vec![u8::from(mob.newmobs.active)]));
            out.push((18, TYPE_BOOL, vec![u8::from(mob.is_dead())]));
            let mut home=vec![u8::from(mob.newmobs.heart.is_some())];
            if let Some((x,y,z))=mob.newmobs.heart { push_packed_position(&mut home,x,y,z); }
            out.push((19,TYPE_OPT_BLOCK_POS,home));
        }
        Kind::HappyGhast => {
            out.push((16, TYPE_BOOL, vec![u8::from(mob.is_baby())]));
            out.push((19, TYPE_BOOL, vec![u8::from(mob.newmobs.still>0)]));
        }
        Kind::ZombifiedPiglin => out.push((16, TYPE_BOOL, vec![u8::from(mob.is_baby())])),
        Kind::Piglin => {
            out.push((16, TYPE_BOOL, vec![u8::from(mob.nether.immune)]));
            out.push((17, TYPE_BOOL, vec![u8::from(mob.is_baby())]));
            out.push((18, TYPE_BOOL, vec![0]));
            out.push((19, TYPE_BOOL, vec![0]));
        }
        Kind::PiglinBrute => out.push((16, TYPE_BOOL, vec![u8::from(mob.nether.immune)])),
        Kind::Hoglin => {
            out.push((16, TYPE_BOOL, vec![u8::from(mob.is_baby())]));
            out.push((18, TYPE_BOOL, vec![u8::from(mob.nether.immune)]));
        }
        Kind::Strider => {
            out.push((16, TYPE_BOOL, vec![u8::from(mob.is_baby())]));
            out.push((18, TYPE_INT, encode_varint(0)));
            out.push((19, TYPE_BOOL, vec![u8::from(mob.nether.cold)]));
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
        Kind::EndCrystal | Kind::ShulkerBullet | Kind::LeashKnot | Kind::EvokerFangs => {}
        // Разбойники (minecraft-data: is_celebrating; у разбойника
        // is_charging_crossbow, у заклинателя и иллюзиониста spell_casting;
        // чёрный ящик, pillager-crossbow: 17=true на зарядке, evoker-fangs: 17=1/2).
        Kind::Pillager => {
            out.push((16, TYPE_BOOL, vec![u8::from(mob.illager.celebrating)]));
            out.push((17, TYPE_BOOL, vec![u8::from(crate::mob::illager::charging(mob))]));
        }
        Kind::Evoker | Kind::Illusioner => {
            out.push((16, TYPE_BOOL, vec![u8::from(mob.illager.celebrating)]));
            out.push((17, TYPE_BYTE, vec![mob.illager.spell]));
        }
        Kind::Vindicator | Kind::Ravager => out.push((16, TYPE_BOOL, vec![u8::from(mob.illager.celebrating)])),
        // Вредина: без тяжести (5) и флаг рывка (16, бит 1; чёрный ящик, evoker-fangs).
        Kind::Vex => {
            out.push((5, TYPE_BOOL, vec![u8::from(mob.ticks >= 1)]));
            out.push((16, TYPE_BYTE, vec![u8::from(mob.illager.charging)]));
        }
        // Голем (minecraft-data: flags у железного, pumpkin у снежного; чёрный
        // ящик, iron-golem-build: 16=1 у построенного, snow-golem-shear: 16=0
        // после ножниц).
        Kind::IronGolem | Kind::SnowGolem => out.push((16, TYPE_BYTE, vec![crate::mob::golem::flags(mob)])),
        // Страж (вики, Entity metadata: 16 — «is retracting spikes», 17 — номер
        // цели луча; чёрный ящик, guardian-beam: 17=номер робота при захвате).
        Kind::Guardian | Kind::ElderGuardian => {
            out.push((16, TYPE_BOOL, vec![u8::from(mob.guardian.moving)]));
            out.push((17, TYPE_INT, encode_varint(mob.guardian.target.unwrap_or(0))));
        }
        // Иссушитель (вики, Entity metadata: 16–18 — цели средней, левой и
        // правой головы, 19 — такты неуязвимости).
        Kind::Wither => {
            for (index, target) in mob.wither.targets.iter().enumerate() {
                out.push((16 + index as u8, TYPE_INT, encode_varint(*target)));
            }

            out.push((19, TYPE_INT, encode_varint(mob.wither.invul)));
        }
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
            out.push((17, TYPE_BOOL, vec![u8::from(mob.h.target.is_some() || mob.newmobs.active)]));
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
/// Вышедшая из улья с нектаром пчела в первом прогоне слежения получает
/// флаги явно, даже нулевые: `18=0` (чёрный ящик, hive-exit).
fn ghost_flag(mob: &Mob, entry: &Property) -> bool {
    mob.kind == Kind::Bee && mob.wild.nectar_ghost && entry.0 == 18
}

fn is_default(entry: &Property, kind: Kind) -> bool {
    let (key, _, value) = entry;

    match (*key, kind) {
        (0 | 5 | 15, Kind::ArmorStand) => value.as_slice() == [0],
        (16..=21, Kind::ArmorStand) => {
            let pose = crate::mob::armor_stand::State::default().pose[usize::from(*key) - 16];
            value.as_slice() == pose.iter().flat_map(|v| v.to_be_bytes()).collect::<Vec<u8>>()
        }
        (2, _) => value.as_slice() == [0],
        (17, kind) if crate::mob::growth::eligible(kind) => value.as_slice() == [0],
        (12, _) => value.as_slice() == [0],
        (5, Kind::Bee) => value.as_slice() == [0],
        // Водные виды: общий хвост 18..20 ниже к ним не относится.
        // Иначе при первом показе теряются вариант аксолотля и лягушки.
        (1, Kind::Dolphin) => value.as_slice() == encode_varint(crate::mob::water::DOLPHIN_AIR),
        (1, kind) if crate::mob::water::creature(kind) => value.as_slice() == encode_varint(300),
        (17, Kind::Salmon) => value.as_slice() == encode_varint(1),
        (17, Kind::TropicalFish | Kind::Pufferfish) => value.as_slice() == [0],
        (18, Kind::Frog) => value.as_slice() == encode_varint(1),
        (19, Kind::Dolphin) => value.as_slice() == encode_varint(crate::mob::water::DOLPHIN_MOISTURE),
        (18..=20, kind) if crate::mob::water::creature(kind) => value.iter().all(|byte| *byte == 0),
        (18 | 19, kind) if crate::mob::horse::equine(kind) => value.iter().all(|byte| *byte == 0),
        (18, Kind::Rabbit | Kind::Fox | Kind::Bee) | (19..=21, Kind::Fox) => value.iter().all(|byte| *byte == 0),
        (19, Kind::Bee) => value.as_slice() == encode_varlong(-1).as_slice(),
        (17, Kind::Allay) => value.as_slice() == [1],
        (16..=20, Kind::Armadillo | Kind::Sniffer | Kind::Parrot | Kind::Bat | Kind::Allay) => value.as_slice() == [0],
        // Волк и кошка: ошейник красный, вариант — бледный и чёрная, злости нет
        // (чёрный ящик, probe-cats).
        (18..=20, Kind::Wolf) | (18 | 19 | 21 | 22, Kind::Cat) => value.iter().all(|byte| *byte == 0),
        (21, Kind::Wolf) | (23, Kind::Cat) => value.as_slice() == encode_varint(i32::from(crate::mob::tame::RED)).as_slice(),
        (22, Kind::Wolf) => value.as_slice() == encode_varlong(-1).as_slice(),
        (23, Kind::Wolf) => value.as_slice() == encode_varint(i32::from(crate::mob::tame::WOLF_DEFAULT)).as_slice(),
        (20, Kind::Cat) => value.as_slice() == encode_varint(i32::from(crate::mob::tame::CAT_DEFAULT)).as_slice(),
        (8, Kind::EndCrystal | Kind::Drowned | Kind::Pillager) => value.as_slice() == [0],
        // Скелеты и их родня: без натяжки при появлении 8 не шлют (чёрный ящик, skeleton-arrow, stray-slowness: `4=true 9=20`).
        (8, kind) if kind.skeleton() => value.as_slice() == [0],
        (16, Kind::IronGolem | Kind::Warden) => value.as_slice() == [0],
        (16 | 17, Kind::Guardian | Kind::ElderGuardian) => value.as_slice() == [0],
        (16..=19, Kind::Wither) => value.as_slice() == [0],
        (16, Kind::SnowGolem) => value.as_slice() == [0x10],
        (9, Kind::EndCrystal) => value.as_slice() == [1],
        (16, Kind::Phantom) => value.as_slice() == [0],
        (5 | 16 | 17, Kind::Pillager | Kind::Evoker | Kind::Illusioner | Kind::Vindicator | Kind::Ravager | Kind::Vex) => value.iter().all(|byte| *byte == 0),
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
        (16, Kind::Creaking) => value.as_slice() == [1],
        (17, Kind::CopperGolem) => value.as_slice() == [0],
        (17..=19, Kind::Creaking) | (16..=19, Kind::HappyGhast) => value.iter().all(|b|*b==0),
        (0 | 4 | 6 | 15 | 16, _) => value.iter().all(|byte| *byte == 0),
        (9, _) => value.as_slice() == float(1.0),
        (10, _) => value.as_slice() == [0],
        (18, Kind::Sheep) => value.iter().all(|byte| *byte == 0),
        (18, Kind::Creeper) | (17, Kind::Creeper) => value.iter().all(|byte| *byte == 0),
        (8, Kind::WanderingTrader) => value.as_slice() == [0],
        (18, Kind::Mooshroom) => value.as_slice()==encode_varint(0).as_slice(),
        (19 | 20, Kind::Goat) => value.as_slice()==[1],
        (18..=23, Kind::Llama | Kind::TraderLlama | Kind::Camel | Kind::Goat | Kind::Panda | Kind::PolarBear | Kind::Ocelot | Kind::WanderingTrader) => value.iter().all(|b| *b==0),
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

    if crate::mob::horse::equine(mob.kind) && mob.horse.explicit != 0 {
        send_horse_attributes(stream, mob, true).await?;
    }

    send_mob_equipment(stream, mob, false).await?;

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
    // Призванная лама (53) и панда не шлют (чёрный ящик, llama-spit, panda-sneeze).
    if wild_wolf(mob) || (crate::mob::land::llama(mob.kind) && mob.vitals.max_health != 53.0) {
        send_max_health(stream, mob).await?;
    }

    // На поводке: связь с держателем (Link Entities).
    if let Some(holder) = mob.leash {
        send_leash(stream, mob.entity_id, holder.id()).await?;
    }

    // Иссушитель: полоса босса тому, кому он показан (wither-summon: сразу
    // после появления и свойств).
    let boss = mob.kind == Kind::Wither;

    if boss {
        send_wither_bar(stream, mob).await?;
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
        effect_levels: [0; 4],
        soul_level: 0,
        wolf_armor: mob.tame.armor,
        armor_pending: false,
        gear_version: mob.horse.gear_version,
        stand_gear: crate::mob::equipment::all(mob),
        stand_mods: Vec::new(),
        attributes_sent: attributes_at_spawn(mob.kind) || (crate::mob::horse::equine(mob.kind) && mob.horse.explicit != 0),
        modded: modded(mob),
        first: true,
        properties,
        max_health: mob.vitals.max_health,
        effect_gives: mob.vitals.effect_gives,
        harness: mob.newmobs.harness,
        cargo: mob.newmobs.cargo,
        boss,
        gear: mob.illager.gear,
        mouth: mob.wild.mouth,
        held: mob.small.held,
    })
}

#[cfg(test)]
pub(super) async fn send_compatibility_probe(stream: &mut Outgoing<'_>, mob: &Mob) -> io::Result<()> {
    send_mob(stream, mob).await.map(|_| ())
}

/// Номер полосы босса иссушителя — его собственный UUID.
fn boss_uuid(entity_id: i32) -> [u8; 16] {
    crate::items::uuid_for(entity_id)
}

/// Полоса иссушителя: название — имя сущности с подсказкой «show_entity» и
/// вставкой UUID (wither-summon: `{"insertion":…,"hover_event":{…},
/// "translate":"entity.minecraft.wither"}`), фиолетовая (5), без делений,
/// затемняет небо (флаг 1).
async fn send_wither_bar(stream: &mut Outgoing<'_>, mob: &Mob) -> io::Result<()> {
    let mut body = encode_varint(BOSS_EVENT);
    let uuid = &mob.uuid;
    let words: Vec<i32> = uuid.chunks(4).map(|word| i32::from_be_bytes([word[0], word[1], word[2], word[3]])).collect();
    let key = "entity.minecraft.wither";
    let title = nbt::Compound::new()
        .string("insertion", &crate::network::login::format_uuid(uuid))
        .compound(
            "hover_event",
            nbt::Compound::new().compound("name", nbt::Compound::new().string("translate", key)).string("action", "show_entity").string("id", "minecraft:wither").int_array("uuid", &words),
        )
        .string("translate", key);

    body.extend_from_slice(&boss_uuid(mob.entity_id));
    body.extend_from_slice(&encode_varint(0));
    body.extend_from_slice(&title.encode_network());
    body.extend_from_slice(&crate::mob::wither::bar(mob).to_be_bytes());
    body.extend_from_slice(&encode_varint(5));
    body.extend_from_slice(&encode_varint(0));
    body.push(1);
    write_body(stream, body).await
}

/// Новая доля полосы иссушителя.
pub(super) async fn send_wither_share(stream: &mut Outgoing<'_>, entity_id: i32, share: f32) -> io::Result<()> {
    let mut body = encode_varint(BOSS_EVENT);

    body.extend_from_slice(&boss_uuid(entity_id));
    body.extend_from_slice(&encode_varint(2));
    body.extend_from_slice(&share.to_be_bytes());
    write_body(stream, body).await
}

pub(super) async fn send_boss_remove(stream: &mut Outgoing<'_>, entity_id: i32) -> io::Result<()> {
    let mut body = encode_varint(BOSS_EVENT);

    body.extend_from_slice(&boss_uuid(entity_id));
    body.extend_from_slice(&encode_varint(1));
    write_body(stream, body).await
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
    super::mob_effects::push(&mut body, mob, ATTRIBUTE_MAX_HEALTH, None);
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

/// Атрибуты лошадиных. Заданные данными уходят при появлении двумя пакетами
/// в разном порядке (чёрный ящик, horse-saddle-ride: сперва прыжок, скорость,
/// здоровье, затем здоровье, прыжок, скорость), после правки данных — вторым
/// порядком; остальным — одна скорость (после 3-го такта и после правки).
async fn send_horse_attributes(stream: &mut Outgoing<'_>, mob: &Mob, first_order: bool) -> io::Result<()> {
    let explicit = mob.horse.explicit;
    let health = (ATTRIBUTE_MAX_HEALTH, f64::from(mob.vitals.max_health));
    let jump = (crate::attributes::JUMP_STRENGTH, mob.horse.jump);
    let speed = (ATTRIBUTE_MOVEMENT_SPEED, mob.horse.speed);
    let mut list: Vec<(i32, f64)> = Vec::new();

    if first_order {
        if explicit & 4 != 0 { list.push(jump); }
        list.push(speed);
        if explicit & 1 != 0 { list.push(health); }
    } else {
        if explicit & 1 != 0 { list.push(health); }
        if explicit & 4 != 0 { list.push(jump); }
        list.push(speed);
    }

    let mut body = encode_varint(UPDATE_ATTRIBUTES);

    body.extend_from_slice(&encode_varint(mob.entity_id));
    body.extend_from_slice(&encode_varint(list.len() as i32));

    for (id, value) in list {
        body.extend_from_slice(&encode_varint(id));
        push_f64(&mut body, value);
        super::mob_effects::push(&mut body, mob, id, None);
    }

    write_body(stream, body).await
}

/// Скорость шага в пакете атрибутов: как у вида, но у шалкера, что не
/// ходит, — 0,7, значение атрибута по умолчанию (чёрный ящик, shulker-stand).
pub(super) fn attribute_speed(kind: Kind) -> f64 {
    // Чёрный ящик, trader-spawn-despawn: у торговца в пакете 0,7.
    if kind == Kind::WanderingTrader { return 0.7; }
    // Пчела: скорость шага 0,3, летит она по летучей скорости 0,6 (чёрный ящик, bee-sting).
    if kind == Kind::Bee { return 0.3; }
    if matches!(kind, Kind::Shulker | Kind::Ghast | Kind::Phantom | Kind::Bat | Kind::ArmorStand) { 0.7 } else { kind.speed() }
}

/// Атрибуты: сообщается одна скорость шага (у оригинала так же: клиенту
/// нужен только изменённый), базовое значение — одинарной точности.
async fn send_attributes(stream: &mut Outgoing<'_>, mob: &Mob) -> io::Result<()> {
    let mut body = encode_varint(UPDATE_ATTRIBUTES);

    body.extend_from_slice(&encode_varint(mob.entity_id));

    // Лошадиные: без заданных данными атрибутов — одна скорость (чёрный ящик,
    // donkey-chest: attributes на 3-м такте, только скорость), с заданными —
    // и здоровье с прыжком (horse-saddle-ride).
    if crate::mob::horse::equine(mob.kind) {
        return send_horse_attributes(stream, mob, false).await;
    }

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
        super::mob_effects::push(&mut body, mob, ATTRIBUTE_MOVEMENT_SPEED, None);
        body.extend_from_slice(&encode_varint(ATTRIBUTE_MAX_HEALTH));
        push_f64(&mut body, size * size);
        super::mob_effects::push(&mut body, mob, ATTRIBUTE_MAX_HEALTH, None);

        return write_body(stream, body).await;
    }

    // Дельфин шлёт ещё «кислородный бонус» 0: у взрослого он первым, у детёныша
    // вторым (чёрный ящик water-spawn-all).
    let dolphin = mob.kind == Kind::Dolphin;
    // Железный шлем на мобе: к скорости добавляются стойкость и броня с
    // модификатором «minecraft:armor.helmet» (0 и +2) — чёрный ящик,
    // stray-slowness, bogged-shear.
    body.extend_from_slice(&encode_varint(if dolphin { 2 + if mob.equipment.is_some() { 2 } else { 0 } } else if mob.h.helmet || mob.h.trial_gear.is_some() || mob.equipment.is_some() { 3 } else { 1 }));
    if dolphin && !mob.is_baby() {
        body.extend_from_slice(&encode_varint(ATTRIBUTE_OXYGEN_BONUS));
        push_f64(&mut body, 0.0);
        push_modifiers(&mut body, None);
    }
    body.extend_from_slice(&encode_varint(ATTRIBUTE_MOVEMENT_SPEED));
    // У иссушителя скорость — ровно 0,6 в двойной точности (wither-summon).
    push_f64(&mut body, if crate::mob::horse::equine(mob.kind) { mob.horse.speed } else if mob.kind == Kind::Wither { 0.6 } else { f64::from(attribute_speed(mob.kind) as f32) });

    // Пьющая ведьма: модификатор «minecraft:drinking» −0,25; сердитый
    // эндермен — «minecraft:attacking» +0,15; оба — сложением (чёрный ящик,
    // witch-stand, enderman-stare).
    super::mob_effects::push(&mut body, mob, ATTRIBUTE_MOVEMENT_SPEED, super::mob_effects::speed_modifier(mob));

    if mob.equipment.is_some() || mob.h.trial_gear.is_some() {
        for (attribute,amount) in [(ATTRIBUTE_ARMOR_TOUGHNESS,crate::mob::equipment::bonus(mob,crate::attributes::ARMOR_TOUGHNESS)),(ATTRIBUTE_ARMOR,mob.armor())] {
            body.extend_from_slice(&encode_varint(attribute));push_f64(&mut body,f64::from(amount));push_modifiers(&mut body,None);
        }
    } else if mob.h.helmet {
        for (attribute, amount) in [(ATTRIBUTE_ARMOR_TOUGHNESS, 0.0), (ATTRIBUTE_ARMOR, 2.0)] {
            body.extend_from_slice(&encode_varint(attribute));
            push_f64(&mut body, 0.0);
            push_modifiers(&mut body, Some(("minecraft:armor.helmet", amount, 0)));
        }
    }

    if dolphin && mob.is_baby() {
        body.extend_from_slice(&encode_varint(ATTRIBUTE_OXYGEN_BONUS));
        push_f64(&mut body, 0.0);
        push_modifiers(&mut body, None);
    }
    write_body(stream, body).await
}

/// Правка данных волка (`data modify|merge`): сообщаются все его
/// атрибуты — здоровье, скорость и, в броне, броня со стойкостью (чёрный ящик,
/// tails-wolf-armor: на +5 и +38 здоровье 8 и скорость, со второго раза ещё
/// и «minecraft:armor.body») [проверить: другие мобы].
async fn send_wolf_reload(stream: &mut Outgoing<'_>, mob: &Mob) -> io::Result<()> {
    let mut body = encode_varint(UPDATE_ATTRIBUTES);
    let armored = mob.tame.armor.is_some();

    body.extend_from_slice(&encode_varint(mob.entity_id));
    body.extend_from_slice(&encode_varint(if armored { 4 } else { 2 }));
    body.extend_from_slice(&encode_varint(ATTRIBUTE_MAX_HEALTH));
    push_f64(&mut body, f64::from(mob.vitals.max_health));
    super::mob_effects::push(&mut body, mob, ATTRIBUTE_MAX_HEALTH, None);
    body.extend_from_slice(&encode_varint(ATTRIBUTE_MOVEMENT_SPEED));
    push_f64(&mut body, f64::from(attribute_speed(mob.kind) as f32));
    super::mob_effects::push(&mut body, mob, ATTRIBUTE_MOVEMENT_SPEED, super::mob_effects::speed_modifier(mob));
    if armored {
        for (attribute, amount) in [(ATTRIBUTE_ARMOR, 11.0), (ATTRIBUTE_ARMOR_TOUGHNESS, 0.0)] {
            body.extend_from_slice(&encode_varint(attribute));
            push_f64(&mut body, 0.0);
            push_modifiers(&mut body, Some(("minecraft:armor.body", amount, 0)));
        }
    }
    write_body(stream, body).await
}

/// Атрибуты брони волка: броня 11 и стойкость 0 с модификатором
/// «minecraft:armor.body»; без брони модификаторов нет.
async fn send_wolf_armor(stream: &mut Outgoing<'_>, mob: &Mob) -> io::Result<()> {
    let mut body = encode_varint(UPDATE_ATTRIBUTES);

    body.extend_from_slice(&encode_varint(mob.entity_id));
    body.extend_from_slice(&encode_varint(2));
    for (attribute, amount) in [(ATTRIBUTE_ARMOR, 11.0), (ATTRIBUTE_ARMOR_TOUGHNESS, 0.0)] {
        body.extend_from_slice(&encode_varint(attribute));
        push_f64(&mut body, 0.0);
        push_modifiers(&mut body, mob.tame.armor.is_some().then_some(("minecraft:armor.body", amount, 0)));
    }
    write_body(stream, body).await
}

/// Броня стойки: оба атрибута (броня, стойкость) с модификаторами надетых вещей.
async fn send_stand_armor(stream: &mut Outgoing<'_>, entity_id: i32, mods: &[(i32, &'static str, f64)]) -> io::Result<()> {
    let mut body = encode_varint(UPDATE_ATTRIBUTES);

    body.extend_from_slice(&encode_varint(entity_id));
    body.extend_from_slice(&encode_varint(2));

    for attribute in [ATTRIBUTE_ARMOR, ATTRIBUTE_ARMOR_TOUGHNESS] {
        body.extend_from_slice(&encode_varint(attribute));
        push_f64(&mut body, 0.0);

        let own: Vec<_> = mods.iter().filter(|m| m.0 == attribute).collect();

        body.extend_from_slice(&encode_varint(own.len() as i32));
        for (_, name, amount) in own {
            body.extend_from_slice(&encode_string(name));
            push_f64(&mut body, *amount);
            body.push(0);
        }
    }

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
        Kind::Enderman => mob.h.target.is_some() || mob.newmobs.active,
        Kind::Shulker => mob.h.peek == 0,
        _ => false,
    }
}

/// Показывает клиенту всех мобов вокруг: о них журнал не скажет — они
/// появились раньше.
pub(super) async fn send_all_mobs(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState) -> io::Result<()> {
    player.seen_mobs.clear();
    player.shoulder_shown.clear();
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
    // Звук появления хранителя уходит в тот же такт, когда его самого клиент
    // ещё не получил (сценарии warden-emerge, shrieker-warning).
    let emerging = matches!(event, Event::Sound { name: "minecraft:entity.warden.emerge", .. });

    if !shown.contains_key(&entity_id) && !emerging {
        return Ok(());
    }

    let heard = |place: (f64, f64, f64)| (place.0 - listener.0).powi(2) + (place.1 - listener.1).powi(2) + (place.2 - listener.2).powi(2) <= 48.0 * 48.0;

    match event {
        Event::CommandHurt { source, origin } => {
            write_body(stream, super::vitals::damage_packet(entity_id, *source, origin.direct, origin.cause, origin.at)).await?;
        }
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
            let category = sound_category(name);
            send_sound(stream, name, category, at, *volume, *pitch).await?;
        }
        Event::SoundAt { name, volume, pitch, at: place } if heard(*place) => {
            let category = sound_category(name);
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
        Event::BlockBurst { block, at: place, offset, speed, count } => send_particles(stream, PARTICLE_BLOCK, Some(*block), *place, *offset, *speed, *count).await?,
        Event::Landing { block, count } => send_particles(stream, PARTICLE_BLOCK, Some(*block), at, (0.0, 0.0, 0.0), 0.15, *count).await?,
        Event::Particle { kind, at: place, offset, speed, count } => {
            let item = (*kind == crate::mob::Particle::ItemBreak).then(|| crate::blocks::item_named("wolf_armor").unwrap_or(0));
            send_particles(stream, kind.id(), item, *place, *offset, *speed, *count).await?
        }
        Event::Sound { .. } | Event::SoundAt { .. } | Event::PlayerSound { .. } | Event::EntitySound { .. } | Event::Loot(_) => {}
    }

    Ok(())
}

/// Что у моба в руках и на голове: лук у скелета (основная рука, место 0) и
/// шлем на голове (место 5) — по одному пакету на моба; шлем показывается
/// железным [проверить: вид шлема не хранится].
async fn send_mob_equipment(stream: &mut Outgoing<'_>, mob: &Mob, changed: bool) -> io::Result<()> {
    if mob.equipment.is_some() {
        let gear = crate::mob::equipment::all(mob);
        let mut slots: Vec<_> = gear.iter().enumerate().filter(|(_, stack)| changed || stack.is_some()).map(|(index, stack)| (index as u8, *stack)).collect();
        // Шесть общих ячеек не заменяют отдельные body/saddle слоты.
        let mut extra = Vec::new();
        if crate::mob::horse::equine(mob.kind) { extra.extend([(7, mob.horse.slots[0]), (6, mob.horse.slots[1])]); }
        if mob.kind == Kind::Wolf { extra.push((6, crate::mob::wolf_armor::stack(mob))); }
        if crate::mob::land::llama(mob.kind) {
            extra.push((6, mob.land.decor.and_then(|c| blocks::item_named(&crate::mob::WOOL[usize::from(c)].replace("_wool", "_carpet"))).map(|id| Stack::new(id, 1))));
        }
        if matches!(mob.kind, Kind::Camel | Kind::Strider) {
            let saddled = if mob.kind == Kind::Strider { mob.nether.saddled } else { mob.land.saddle };
            extra.push((7, saddled.then(|| blocks::item_named("saddle")).flatten().map(|id| Stack::new(id, 1))));
        }
        if mob.kind == Kind::HappyGhast {
            extra.push((6, mob.newmobs.harness.and_then(|c| blocks::item_named(&format!("{}_harness", crate::mob::newmobs::COLOURS[c as usize]))).map(|id| Stack::new(id, 1))));
        }
        slots.extend(extra.into_iter().filter(|(_, stack)| changed || stack.is_some()));
        return if slots.is_empty() { Ok(()) } else { send_equipment(stream, mob.entity_id, &slots).await };
    }
    if mob.kind==Kind::ArmorStand {
        // Пустая стойка снаряжения не шлёт, занятая — только занятые места (чёрный ящик armorstand-equip-break).
        let slots:Vec<_>=mob.stand.equipment.iter().enumerate().filter(|(_,s)|s.is_some()).map(|(i,s)|(i as u8,*s)).collect();
        return if slots.is_empty() { Ok(()) } else { super::send_equipment(stream,mob.entity_id,&slots).await };
    }

    if let Some(gear)=&mob.h.trial_gear {return send_equipment(stream,mob.entity_id,&[(0,gear[0]),(4,gear[1]),(5,gear[2])]).await;}
    // Лиса с пустым ртом снаряжения не шлёт (чёрный ящик, fox-feed, fox-hit).
    if mob.kind == Kind::Fox { return if mob.wild.mouth.is_some() { send_fox_hand(stream, mob).await } else { Ok(()) }; }
    if crate::mob::horse::equine(mob.kind) {
        // С 1.21.6 седло — снаряжение, не бит свойства лошади; тело — слот 6.
        // Пустое снаряжение не шлётся (чёрный ящик, donkey-chest/horse-tame).
        if mob.horse.slots.iter().all(|slot| slot.is_none()) {
            return Ok(());
        }
        // Уходят только занятые места (чёрный ящик, horse-saddle-ride: седло —
        // один пакет с местом 7); опустевшее снаряжение — оба места пустыми.
        let mut gear: Vec<(u8, Option<Stack>)> = [(7, mob.horse.slots[0]), (6, mob.horse.slots[1])].into_iter().filter(|(_, s)| s.is_some()).collect();
        if gear.is_empty() { gear = vec![(7, None), (6, None)]; }
        return send_equipment(stream, mob.entity_id, &gear).await;
    }
    let mut slots: Vec<(u8, Stack)> = Vec::new();
    if mob.kind == Kind::Allay && let Some(held) = mob.small.held { slots.push((EQUIPMENT_MAIN_HAND, held)); }

    if mob.kind.skeleton() && mob.h.bow && let Some(bow) = blocks::item_named("bow") {
        slots.push((EQUIPMENT_MAIN_HAND, Stack::new(bow, 1)));
    }

    // Утопленник: основная рука — трезубец, ракушка или удочка; вторая — ракушка.
    if let Some(item) = crate::mob::hostile::hand_item(mob.h.hand).and_then(blocks::item_named) {
        slots.push((EQUIPMENT_MAIN_HAND, Stack::new(item, 1)));
    }

    if mob.h.offhand_shell && let Some(shell) = blocks::item_named("nautilus_shell") {
        slots.push((EQUIPMENT_OFF_HAND, Stack::new(shell, 1)));
    }

    // Разбойник с арбалетом, поборник с топором, вредина с мечом (mob/illager.rs).
    if let Some(item) = crate::mob::illager::hand_item(mob.kind).and_then(blocks::item_named) {
        let mut stack = Stack::new(item, 1);

        if mob.kind == Kind::Pillager {
            stack.data = crate::stackdata::intern(crate::stackdata::Data { damage: mob.illager.wear, charged: u8::from(mob.illager.charged), ..Default::default() });
        }

        slots.push((EQUIPMENT_MAIN_HAND, stack));
    }

    // Капитан: зловещий флаг на голове (чёрный ящик, evoker-fangs: слот 5).
    if mob.illager.captain && let Some(banner) = blocks::item_named("white_banner") {
        let mut stack = Stack::new(banner, 1);

        stack.data = crate::stackdata::intern(crate::stackdata::Data { ominous_banner: true, ..Default::default() });
        slots.push((EQUIPMENT_HEAD, stack));
    }

    if mob.h.helmet && let Some(helmet) = blocks::item_named("iron_helmet") {
        slots.push((EQUIPMENT_HEAD, Stack::new(helmet, 1)));
    }

    if mob.kind == Kind::Wolf {
        if mob.tame.armor.is_none() && !changed { return Ok(()); }
        return super::send_equipment(stream, mob.entity_id, &[(6, crate::mob::wolf_armor::stack(mob))]).await;
    }

    if crate::mob::land::llama(mob.kind) {
        if mob.land.decor.is_none() && !changed { return Ok(()); }
        let decor=mob.land.decor.and_then(|c| blocks::item_named(&crate::mob::WOOL[usize::from(c)].replace("_wool","_carpet"))).map(|id| Stack::new(id,1));
        return super::send_equipment(stream,mob.entity_id,&[(6,decor)]).await;
    }
    if matches!(mob.kind,Kind::Camel | Kind::Strider) {
        // Minecraft-data 26.1: седло — equipment slot 7, не metadata boost_time.
        let saddled = if mob.kind == Kind::Strider { mob.nether.saddled } else { mob.land.saddle };
        if !saddled && !changed { return Ok(()); }
        let saddle=saddled.then(|| blocks::item_named("saddle")).flatten().map(|id| Stack::new(id,1));
        return super::send_equipment(stream,mob.entity_id,&[(7,saddle)]).await;
    }
    if mob.kind==Kind::Panda && mob.land.held.is_none() && !changed { return Ok(()); }
    if mob.kind==Kind::Panda { return super::send_equipment(stream,mob.entity_id,&[(0,mob.land.held)]).await; }
    if mob.kind==Kind::WanderingTrader {
        if mob.land.drinking<=0 && !changed { return Ok(()); }
        let hand=if mob.land.drinking>0 { blocks::item_named(if mob.land.invisible { "milk_bucket" } else { "potion" }).map(|id| {
            let mut stack=Stack::new(id,1); if !mob.land.invisible { let mut d=stack.extras(); d.potion=Some(6); stack=stack.with_extras(d); } stack
        }) } else { None };
        return super::send_equipment(stream,mob.entity_id,&[(0,hand)]).await;
    }
    if let Some(c)=mob.newmobs.harness && let Some(item)=blocks::item_named(&format!("{}_harness",crate::mob::newmobs::COLOURS[c as usize])) {
        slots.push((6,Stack::new(item,1)));
    }
    if let Some(stack)=mob.newmobs.cargo { slots.push((EQUIPMENT_MAIN_HAND,stack)); }
    if slots.is_empty() { return Ok(()); }

    send_equipment_slots(stream, mob.entity_id, &slots).await
}

async fn send_fox_hand(stream: &mut Outgoing<'_>, mob: &Mob) -> io::Result<()> {
    let mut body = encode_varint(SET_EQUIPMENT);
    body.extend_from_slice(&encode_varint(mob.entity_id));
    body.push(EQUIPMENT_MAIN_HAND);
    push_slot(&mut body, mob.wild.mouth);
    write_body(stream, body).await
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
        push_slot(&mut body, (stack.count > 0).then_some(*stack));
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
        if particle == 47 {
            // ItemStackTemplate: номер ПЕРЕД числом; подтверждено официальной пробой.
            body.extend_from_slice(&encode_varint(block));
            body.extend_from_slice(&encode_varint(1));
            body.extend_from_slice(&encode_varint(0));
            body.extend_from_slice(&encode_varint(0));
        } else {
            body.extend_from_slice(&encode_varint(block));
        }
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
        // Полоса иссушителя уходит перед ним самим (wither-summon).
        if player.seen_mobs.get(&entity_id).is_some_and(|shown| shown.boss) {
            send_boss_remove(stream, entity_id).await?;
        }

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
        // Лошадь на свойствах слежение вне очереди не гонит (чёрный ящик, horse-jump: 18=34 на +42, сверка на +45).
        let dirty = !shown.first && !crate::mob::horse::equine(mob.kind) && (properties(mob) != shown.properties || mob.vitals.effect_gives != shown.effect_gives);
        let due = mob.ticks % EVERY == 0 || dirty;

        if due || impulse {
            let change = (speed.0 - shown.speed.0).powi(2) + (speed.1 - shown.speed.1).powi(2) + (speed.2 - shown.speed.2).powi(2);
            // Иссушитель скорость сам по себе не шлёт — только после толчка
            // (wither-summon: падение без пакетов скорости, при ударе — 0,4).
            let tracked = mob.kind != Kind::Wither || impulse;

            if !tracked {
                // Не шлёт, но знает: после /kill скорость уходит один раз, в
                // конце (wither-summon).
                shown.speed = speed;
            } else if change > SPED || (speed == (0.0, 0.0, 0.0) && shown.speed != (0.0, 0.0, 0.0)) {
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

                if mob.teleports != shown.teleports || (mob.kind == Kind::HappyGhast && mob.newmobs.still > 0) || mob.on_ground != shown.on_ground || !(fits(step.0) && fits(step.1) && fits(step.2)) {
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
            now.iter().filter(|entry| (!is_default(entry, mob.kind) && !(mob.kind == Kind::Dolphin && entry.0 == 1)) || (mob.kind == Kind::EnderDragon && entry.0 == 9) || ghost_flag(mob, entry)).collect()
        } else {
            // Воздух дельфина уходит заново каждый такт (чёрный ящик water-spawn-all: `1=4800`
            // даже у NoAI, начиная со 2-го такта).
            now.iter().filter(|entry| !shown.properties.contains(entry) || (mob.kind == Kind::Dolphin && entry.0 == 1) || (entry.0 == 10 && mob.vitals.effect_gives != shown.effect_gives)).collect()
        };

        send_properties(stream, mob.entity_id, &changed).await?;
        shown.properties = now;
        shown.effect_gives = mob.vitals.effect_gives;

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
        if (shown.first && wild_wolf(mob)) || (matches!(mob.kind,Kind::Wolf|Kind::Llama|Kind::TraderLlama|Kind::Panda) && mob.vitals.max_health != shown.max_health) {
            send_max_health(stream, mob).await?;
            shown.max_health = mob.vitals.max_health;
        }

        // Снаряжение уходит и второй раз в первом проходе (чёрный ящик,
        // skeleton-stand и zombie-attack).
        if shown.first && crate::mob::horse::equine(mob.kind) && mob.horse.explicit != 0 {
            send_horse_attributes(stream, mob, false).await?;
        }

        // Разбойник: арбалет заряжен или выстрелил — снаряжение заново
        // (чёрный ящик, pillager-crossbow: charged_projectiles, затем damage).
        let land_gear = mob.kind==Kind::ArmorStand && shown.attr_changes!=mob.attr_changes || (crate::mob::land::rideable(mob.kind) || matches!(mob.kind, Kind::WanderingTrader | Kind::Panda)) && shown.attr_changes != mob.attr_changes;
        let horse_gear = crate::mob::horse::equine(mob.kind) && mob.horse.gear_version != shown.gear_version;

        // Стойка: уходят только изменившиеся ячейки, пустая — ничего (чёрный ящик armorstand-equip-break).
        if mob.kind == Kind::ArmorStand || mob.equipment.is_some() {
            let gear = crate::mob::equipment::all(mob);
            let changed: Vec<(u8, Option<Stack>)> = (0..6).filter(|&i| gear[i] != shown.stand_gear[i]).map(|i| (i as u8, gear[i])).collect();
            if !changed.is_empty() { send_equipment(stream, mob.entity_id, &changed).await?; }
            shown.stand_gear = gear;
        }

        // Броня волка (вики, Wolf Armor): снаряжение уходит сразу, атрибуты
        // брони — на следующем такте жизни, кратном трём (чёрный ящик,
        // tails-wolf-armor: надел на 15-м, атрибуты на 18-м) [проверить].
        let wolf_gear = mob.kind == Kind::Wolf && !shown.first && mob.tame.armor != shown.wolf_armor;

        if mob.kind == Kind::Wolf && shown.armor_pending && !wolf_gear && mob.ticks % 3 == 0 {
            send_wolf_armor(stream, mob).await?;
            shown.armor_pending = false;
        }

        if mob.kind != Kind::ArmorStand && (shown.first || mob.illager.gear != shown.gear || land_gear || horse_gear || shown.held != mob.small.held || wolf_gear) {
            send_mob_equipment(stream, mob, shown.attr_changes != mob.attr_changes || wolf_gear).await?;
            if wolf_gear {
                shown.wolf_armor = mob.tame.armor;
                shown.armor_pending = true;
            }
            shown.gear = mob.illager.gear;
            shown.gear_version = mob.horse.gear_version;
            shown.held = mob.small.held;
        }

        if mob.kind == Kind::Fox && mob.wild.mouth != shown.mouth {
            send_fox_hand(stream, mob).await?;
            shown.mouth = mob.wild.mouth;
        }

        // Радостный гаст: упряжь и груз медного голема (отдельно от общего снаряжения).
        if shown.harness != mob.newmobs.harness || shown.cargo != mob.newmobs.cargo {
            if shown.harness.is_some() && mob.newmobs.harness.is_none() { send_equipment_slots(stream, mob.entity_id, &[(6, Stack::new(0, 0))]).await?; }
            if shown.cargo.is_some() && mob.newmobs.cargo.is_none() { send_equipment_slots(stream, mob.entity_id, &[(0, Stack::new(0, 0))]).await?; }
            // В первом проходе снаряжение уже ушло выше (там же и упряжь).
            if !shown.first { send_mob_equipment(stream, mob, false).await?; }
            shown.harness = mob.newmobs.harness;
            shown.cargo = mob.newmobs.cargo;
        }

        shown.first = false;

        // Атрибуты: на третьем такте жизни и после каждой правки данных.
        let attributes_due = if mob.kind == Kind::Wither && mob.wither.built { 1 } else { attributes_at(mob) };

        let sent_base = (!shown.attributes_sent && mob.ticks >= attributes_due) || (mob.kind != Kind::ArmorStand && mob.attr_changes != shown.attr_changes);
        if sent_base {
            if shown.attributes_sent && mob.kind == Kind::Wolf {
                send_wolf_reload(stream, mob).await?;
            } else {
                send_attributes(stream, mob).await?;
            }
            shown.attributes_sent = true;
            shown.attr_changes = mob.attr_changes;
        }

        let mut changed = super::mob_effects::changed(mob, shown.effect_levels);
        for attribute in super::mob_effects::soul_changed(mob, shown.soul_level) {
            changed.retain(|other| other.id != attribute.id);
            changed.push(attribute);
        }
        shown.soul_level = mob.foot_enchants.soul;
        if sent_base {
            // Скорость уже включает эффект в основной пачке атрибутов.
            changed.retain(|a| a.id != ATTRIBUTE_MOVEMENT_SPEED && !(a.id == ATTRIBUTE_MAX_HEALTH
                && (crate::mob::slime::cube(mob.kind) || (crate::mob::horse::equine(mob.kind) && mob.horse.explicit & 1 != 0))));
        }
        if !changed.is_empty() {
            let mut body = encode_varint(UPDATE_ATTRIBUTES);
            body.extend_from_slice(&crate::attributes::encode_update(mob.entity_id, &changed));
            write_body(stream, body).await?;
        }
        shown.effect_levels = super::mob_effects::levels(mob);

        // Броня стойки: броня и стойкость с модификаторами «minecraft:armor.<ячейка>» — позже надевания
        // (чёрный ящик armorstand-equip-break: +2 надели, +1 сняли). Снятая броня — те же атрибуты без модификаторов.
        if mob.kind == Kind::ArmorStand && shown.attributes_sent {
            let mods = crate::mob::armor_stand::armor_mods(&mob.stand);
            if mods != shown.stand_mods && mob.stand.armor_due.is_none_or(|due| mob.ticks >= due) {
                send_stand_armor(stream, mob.entity_id, &mods).await?;
                shown.stand_mods = mods;
            }
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
        effect_levels: [0; 4],
        soul_level: 0,
        wolf_armor: None,
        armor_pending: false,
        gear_version: 0,
        stand_gear: [None; 6],
        stand_mods: Vec::new(),
        attributes_sent: true,
        modded: false,
        first: true,
        properties,
        max_health: 0.0,
        effect_gives: 0,
        boss: false,
        gear: 0,
        mouth: None,
        held: None,
        harness: None,
        cargo: None,
    })
}

/// Первый отчёт ещё не прошёл, а шарик уже подбирают.
pub(super) async fn orb_before_pickup(stream: &mut Outgoing<'_>, entity_id: i32, shown: &mut ShownMob) -> io::Result<()> {
    if !shown.first { return Ok(()); }
    // Вики, Experience orb: «picked up»; fishing-catch.md:406–412 — нулевой шаг
    // и повтор значения приходят даже при подборе в такте рождения шарика.
    let mut body = encode_varint(ENTITY_POSITION);
    body.extend_from_slice(&encode_varint(entity_id));
    for _ in 0..3 { push_i16(&mut body, 0); }
    body.push(u8::from(shown.on_ground));
    write_body(stream, body).await?;
    send_properties(stream, entity_id, &shown.properties.iter().collect::<Vec<_>>()).await?;
    shown.first = false;
    Ok(())
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

        if orb.ticks % ORB_EVERY == 0 && (orb.ticks != 0 || shown.first) {
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
            } else if moved || orb.ticks % FORCED == 0 {
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

/// Раздел звука моба: слизень, фантом и разбойники — «враждебные» (5);
/// начало и середина зарядки арбалета разбойника — «игроки» (чёрный ящик,
/// pillager-crossbow: loading_start/middle — player, loading_end — hostile).
fn sound_category(name: &str) -> i32 {
    const HOSTILE: [&str; 14] = [
        "minecraft:entity.wither.",
        "minecraft:entity.slime.",
        "minecraft:entity.phantom.",
        "minecraft:entity.pillager.",
        "minecraft:entity.vindicator.",
        "minecraft:entity.evoker",
        "minecraft:entity.vex.",
        "minecraft:entity.illusioner.",
        "minecraft:entity.ravager.",
        "minecraft:item.crossbow.",
        "minecraft:entity.warden.",
        "minecraft:entity.endermite.",
        "minecraft:entity.creaking.",
        "minecraft:entity.zoglin.",
    ];

    if matches!(name, "minecraft:item.crossbow.loading_start" | "minecraft:item.crossbow.loading_middle"
        | "minecraft:item.crossbow.quick_charge_1" | "minecraft:item.crossbow.quick_charge_2" | "minecraft:item.crossbow.quick_charge_3") {
        SOUND_PLAYERS
    } else if HOSTILE.iter().any(|prefix| name.starts_with(prefix)) {
        5
    } else {
        SOUND_NEUTRAL
    }
}

#[cfg(test)]
pub(super) fn water_test_properties(mob: &Mob) -> Vec<Property> { properties(mob) }

#[cfg(test)]
pub(super) fn water_test_spawn_properties(mob: &Mob) -> Vec<Property> {
    properties(mob).into_iter().filter(|entry| !is_default(entry,mob.kind)).collect()
}

#[cfg(test)]
mod small_tests {
    use super::*;
    #[test]
    fn small_mob_metadata_uses_the_26_1_layout() {
        let mut mob=Mob::new(1,Kind::Armadillo,(0.0,1.0,0.0)); mob.small.roll=2;
        assert!(properties(&mob).contains(&(18,36,encode_varint(2))));
        mob.kind=Kind::Sniffer; mob.small.sniff=5; mob.small.drop_seed_at=120;
        assert!(properties(&mob).contains(&(18,35,encode_varint(5))));
        assert!(properties(&mob).contains(&(19,TYPE_INT,encode_varint(120))));
        mob.kind=Kind::Bat; mob.small.resting=true;
        assert!(properties(&mob).contains(&(16,TYPE_BYTE,vec![1])));
        mob.kind=Kind::Parrot; mob.small.variant=4; mob.tame.owner=Some([1;16]); mob.tame.sitting=true;
        assert!(properties(&mob).contains(&(18,TYPE_BYTE,vec![5])));
        assert!(properties(&mob).contains(&(20,TYPE_INT,encode_varint(4))));
        mob.kind=Kind::Allay; mob.small.dancing=true; mob.small.duplicate=1;
        assert!(properties(&mob).contains(&(16,TYPE_BOOL,vec![1])));
        assert!(properties(&mob).contains(&(17,TYPE_BOOL,vec![0])));
        assert!(!is_default(&(17,TYPE_BOOL,vec![0]),Kind::Allay));
    }
}

#[cfg(test)]
mod slime_tests {
    use super::*;

    #[test]
    fn skeleton_variant_metadata_has_conversion_shearing_and_drawing_flags() {
        let mut skeleton = Mob::new(1, Kind::Skeleton, (0.0, 0.0, 0.0));
        skeleton.h.converting = 300;
        assert!(properties(&skeleton).contains(&(16, TYPE_BOOL, vec![1])));
        let stray = Mob::new(2, Kind::Stray, (0.0, 0.0, 0.0));
        assert!(!properties(&stray).iter().any(|(index, _, _)| *index == 16));
        let mut bogged = Mob::new(3, Kind::Bogged, (0.0, 0.0, 0.0));
        assert!(is_default(&(16, TYPE_BOOL, vec![0]), Kind::Bogged));
        bogged.sheared = true;
        bogged.h.drawing = 10;
        assert!(properties(&bogged).contains(&(16, TYPE_BOOL, vec![1])));
        assert!(properties(&bogged).contains(&(8, TYPE_BYTE, vec![1])));
        assert!(!is_default(&(16, TYPE_BOOL, vec![1]), Kind::Bogged));
    }

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

#[cfg(test)]
mod land_metadata_tests {
    use super::*;

    #[test]
    fn land_fields_use_their_own_types_and_are_not_filtered_as_cow_defaults() {
        let mut llama=Mob::new(1,Kind::Llama,(0.0,0.0,0.0));
        llama.land.strength=5; llama.land.variant=3; llama.land.chest=true; llama.land.tamed=true;
        let props=properties(&llama);
        for property in [(18,TYPE_BYTE,vec![2]),(19,TYPE_BOOL,vec![1]),(20,TYPE_INT,encode_varint(5)),(21,TYPE_INT,encode_varint(3))] {
            assert!(props.contains(&property)); assert!(!is_default(&property,Kind::Llama));
        }
        let mut cow=Mob::new(2,Kind::Mooshroom,(0.0,0.0,0.0)); cow.land.brown=true;
        let brown=(18,TYPE_INT,encode_varint(1)); assert!(properties(&cow).contains(&brown)); assert!(!is_default(&brown,cow.kind));
        assert!(!properties(&cow).iter().any(|(_,t,_)| *t==TYPE_COW_VARIANT));
    }

    #[test]
    fn baby_goat_has_no_visible_horns_and_trader_drinking_sets_use_flag() {
        let mut goat=Mob::new(1,Kind::Goat,(0.0,0.0,0.0)); goat.age= -100;
        assert!(properties(&goat).contains(&(19,TYPE_BOOL,vec![0])));
        assert!(properties(&goat).contains(&(20,TYPE_BOOL,vec![0])));
        let mut trader=Mob::new(2,Kind::WanderingTrader,(0.0,0.0,0.0)); trader.land.drinking=12; trader.land.invisible=true;
        assert!(properties(&trader).contains(&(8,TYPE_BYTE,vec![1])));
        assert!(properties(&trader).contains(&(0,TYPE_BYTE,vec![32])));
    }
}
#[cfg(test)]
mod horse_tests {
    use super::*;

    #[test]
    fn horse_spawn_metadata_preserves_tame_variant_and_chest() {
        let mut horse = Mob::new(1, Kind::Horse, (0.0, 0.0, 0.0));
        horse.horse.tamed = true;
        horse.horse.variant = 4 | (3 << 8);
        let props = properties(&horse);
        assert!(props.contains(&(18, TYPE_BYTE, vec![2])));
        assert!(props.contains(&(19, TYPE_INT, encode_varint(772))));
        assert!(!is_default(&(18, TYPE_BYTE, vec![2]), Kind::Horse));
        assert!(!is_default(&(19, TYPE_INT, encode_varint(772)), Kind::Horse));
        assert!(is_default(&(18, TYPE_BYTE, vec![0]), Kind::Horse));
        let mut donkey = Mob::new(2, Kind::Donkey, (0.0, 0.0, 0.0));
        donkey.horse.chest = true;
        assert!(properties(&donkey).contains(&(19, TYPE_BOOL, vec![1])));
        assert!(!is_default(&(19, TYPE_BOOL, vec![1]), Kind::Donkey));
    }
}
#[cfg(test)]
mod new_mob_tests {
    use super::*;

    #[test]
    fn new_metadata_uses_26_1_serializers() {
        let mut copper=Mob::new(1,Kind::CopperGolem,(0.0,0.0,0.0));copper.newmobs.oxidation=2;
        assert!(properties(&copper).contains(&(16,38,encode_varint(2))));
        assert!(properties(&copper).iter().any(|(index,ty,_)| *index==17 && *ty==37));
        let mut creaking=Mob::new(2,Kind::Creaking,(0.0,0.0,0.0));creaking.newmobs.frozen=true;creaking.newmobs.heart=Some((-4,64,9));
        let props=properties(&creaking);
        assert!(props.contains(&(16,TYPE_BOOL,vec![0])));
        assert!(!is_default(&(16,TYPE_BOOL,vec![0]),Kind::Creaking));
        let mut pos=vec![1];push_packed_position(&mut pos,-4,64,9);
        assert!(props.contains(&(19,11,pos)));
        let mut happy=Mob::new(3,Kind::HappyGhast,(0.0,0.0,0.0));happy.age = -100;
        assert!(properties(&happy).contains(&(16,TYPE_BOOL,vec![1])));
    }
}

#[cfg(test)]
mod itemaudit2_tests {
    use super::*;
    #[test]
    fn invisibility_potion_sets_and_clears_the_entity_flag() {
        let mut mob = Mob::new(1, Kind::Cow, (0.0, 0.0, 0.0));
        let flags = |mob: &Mob| properties(mob).into_iter().find(|(id, ..)| *id == 0).unwrap().2[0];
        assert_eq!(flags(&mob) & 0x20, 0);
        mob.vitals.give_effect(crate::survival::EffectKind::Invisibility, 0, 100);
        assert_eq!(flags(&mob) & 0x20, 0x20);
        mob.vitals.remove_effects(None);
        assert_eq!(flags(&mob) & 0x20, 0);
        mob.land.invisible = true;
        assert_eq!(flags(&mob) & 0x20, 0x20);
    }
}

#[cfg(test)]
mod egg_variant_tests {
    use super::*;

    #[test]
    fn hatched_chicken_variant_uses_existing_registry_and_save_field() {
        for (stored, registry) in [(0, 1), (1, 0), (2, 2)] {
            let mut chick = Mob::new(500, Kind::Chicken, (0.5, 64.0, 0.5));
            chick.age = crate::mob::BABY_AGE;
            chick.color = stored;
            let mut saved = Vec::new();
            chick.write(&mut saved);
            let restored = Mob::read(&mut saved.as_slice()).unwrap();
            assert_eq!(restored.color, stored);
            let entry = properties(&restored).into_iter().find(|e| e.0 == 18).unwrap();
            assert_eq!(entry, (18, TYPE_CHICKEN_VARIANT, encode_varint(registry)));
        }
    }
}

#[cfg(test)]
mod orb_pickup_tests {
    use super::*;

    #[tokio::test]
    async fn newborn_orb_sends_first_step_and_value_before_pickup_once() {
        let orb = XpOrb::new(42, 5, (3.5, 1.5, -2.0), (0.0, 0.0, 0.0));
        let mut spawn = Outgoing::capture();
        let mut shown = send_orb(&mut spawn, &orb).await.unwrap();
        let mut out = Outgoing::capture();
        orb_before_pickup(&mut out, 42, &mut shown).await.unwrap();
        let bytes = out.queued().to_vec();
        assert!(!bytes.is_empty());
        assert!(!shown.first);
        orb_before_pickup(&mut out, 42, &mut shown).await.unwrap();
        assert_eq!(out.queued(), bytes);
        let mut input = bytes.as_slice();
        let movement = crate::network::packet::read_packet(&mut input).await.unwrap();
        assert_eq!(movement.id, ENTITY_POSITION);
        assert_eq!(movement.payload(), &[42, 0, 0, 0, 0, 0, 0, 0]);
        let metadata = crate::network::packet::read_packet(&mut input).await.unwrap();
        assert_eq!(metadata.id, SET_ENTITY_DATA);
        assert_eq!(metadata.payload(), &[42, ORB_VALUE_KEY, TYPE_INT as u8, 5, 255]);
        assert!(input.is_empty());
    }
}
