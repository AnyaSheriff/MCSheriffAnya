//! Второе представление общего Mob, без изменения правил Java.
//! Ключи и биты: Mojang bedrock-protocol-docs, r/26_u1 (944), html/enums.html.
//! Вики, Entity: «entity data»; варианты и позы [проверить] живым клиентом.
use crate::mob::{Kind, Mob};
use super::mobdata::Value;

/// В minecraft-data Flags2 пропущен LAYING_DOWN: используем абсолютные
/// номера ActorFlags из документации Mojang, а не позиции того массива.
pub fn flag(bits: &mut [u64; 2], index: usize, on: bool) {
    if on { bits[index / 64] |= 1u64 << (index % 64); }
}

pub fn flags(m: &Mob) -> [u64; 2] {
    let mut b = [0; 2];
    let mut set = |i, on| flag(&mut b, i, on);
    set(0, m.vitals.fire > 0 || m.kind == Kind::Blaze && m.nether.charging);
    set(1, m.kind == Kind::Fox && m.wild.crouching);
    set(4, m.h.using || m.h.drawing > 0 || m.h.drinking > 0 || m.land.drinking > 0);
    set(5, m.land.invisible || m.vitals.has_effect(crate::survival::EffectKind::Invisibility));
    set(8, m.horse.slots[0].is_some() && crate::mob::horse::equine(m.kind) || m.nether.saddled || m.land.saddle || m.newmobs.harness.is_some());
    set(10, m.kind == Kind::Creeper && m.h.lit);
    set(16, m.no_ai);
    set(17, m.silent);
    set(18, m.h.climbing);
    // Плавает и летает — постоянные флаги вида: берутся из профиля BDS, без профиля — из видов.
    let profiled = super::bds_profile::of(m.kind).is_some();
    set(20, !profiled && crate::mob::water::creature(m.kind));
    set(21, !profiled && matches!(m.kind, Kind::Bee | Kind::Bat | Kind::Parrot | Kind::Allay | Kind::Ghast | Kind::HappyGhast | Kind::Phantom | Kind::EnderDragon | Kind::Vex));
    set(23, m.kind == Kind::Bat && m.small.resting);
    set(25, m.tame.anger_until > m.tame.clock || m.nether.anger > 0 || m.wild.anger > 0 || m.kind == Kind::Enderman && m.h.provoked);
    set(26, m.tame.begging);
    set(27, m.nether.charging);
    set(30, m.leash.is_some_and(|h| h.id() != 0));
    set(33, m.kind == Kind::ElderGuardian);
    set(36, m.horse.chest || m.land.chest);
    set(38, m.kind == Kind::EndCrystal);
    set(39, m.horse.rearing > 0 || m.land.standing);
    // Прыжок силой, управление WASD и рывок — только под седлом: у BDS ни у лошади без седла, ни у верблюда
    // этих флагов нет (запись d-horse, d-camel); скелетная лошадь у BDS появляется уже готовой к езде.
    let saddled = m.horse.slots[0].is_some() || m.nether.saddled || m.land.saddle;
    let ridden = saddled || m.kind == Kind::SkeletonHorse;
    set(44, ridden && (crate::mob::horse::equine(m.kind) || matches!(m.kind, Kind::Camel | Kind::Strider)));
    set(45, ridden && crate::mob::horse::equine(m.kind));
    set(46, saddled && m.kind == Kind::Camel);
    // Дрожь: зоглин-в-мире (хоглин, пиглин, брут в Верхнем мире) и мёрзнущий страйдер вне лавы.
    set(40, m.kind == Kind::Strider && m.nether.cold || matches!(m.kind, Kind::Hoglin | Kind::Piglin | Kind::PiglinBrute) && m.newmobs.outside > 0 && !m.nether.immune);
    set(51, m.small.dancing);
    set(59, m.kind == Kind::Turtle && m.water.has_egg);
    set(60, m.kind == Kind::Turtle && m.water.laying > 0);
    set(63, m.horse.eating > 0 || m.land.eating > 0 || m.wild.eating > 0);
    set(65, m.kind == Kind::Panda && m.land.sneeze > 0);
    set(66, m.kind == Kind::Ocelot && m.land.trusting);
    set(67, m.kind == Kind::Panda && m.land.rolling > 0);
    set(68, m.kind == Kind::Panda && m.land.worried);
    set(76, m.kind == Kind::Fox && m.wild.sleeping);
    set(82, m.illager.captain);
    set(94, m.kind == Kind::Piglin && m.nether.admiring > 0);
    set(97, m.kind == Kind::Goat && m.land.ram_ticks > 0);
    set(98, m.kind == Kind::Axolotl && m.water.playing_dead > 0);
    set(102, m.kind == Kind::Frog && m.water.tongue > 0);
    if m.kind == Kind::Warden {
        use crate::mob::warden::Pose;
        set(84, m.warden.pose == Pose::Roaring);
        set(104, m.warden.pose == Pose::Emerging);
        set(105, m.warden.pose == Pose::Sniffing);
        set(106, m.warden.pose == Pose::Digging);
        set(107, m.warden.sonic > 0);
    }
    set(108, m.kind == Kind::Camel && m.land.dash > 0);
    if let Some(p) = super::bds_profile::of(m.kind) {
        b[1] |= p.flags2;
    }
    b
}

/// Java и Bedrock по-разному называют заклинателя и его челюсти.
/// Иллюзор в Bedrock отсутствует; ближайший вид — заклинатель [проверить].
pub fn identifier(kind: Kind) -> &'static str {
    match kind {
        Kind::Evoker | Kind::Illusioner => "minecraft:evocation_illager",
        Kind::EvokerFangs => "minecraft:evocation_fang",
        Kind::TropicalFish => "minecraft:tropicalfish",
        _ => kind.bedrock_name(),
    }
}

pub fn entries(m: &Mob) -> Vec<(u32, Value)> {
    use Value::{Byte, Float, Int, Long};
    // Размер — пара BDS (у детёныша размер вида, а не половина), клиент умножает его на scale.
    let (width, height, _) = super::bds_profile::shape(m.kind, m.is_baby(), m.size());
    let mut d = vec![
        (5, Long(if m.tame.owner.is_some() { i64::from(m.tame.owner_id) } else { -1 })),
        // Без поводка у BDS 0 (при появлении), снятый поводок — тот же 0 после синхронизации.
        (37, Long(m.leash.map_or(0, |h| i64::from(h.id())))),
        (53, Float(width)), (54, Float(height)),
    ];
    let variant = match m.kind {
        // Порядок Java — по алфавиту (CAT_VARIANTS), Bedrock — по вики Cat: white 0, tuxedo (black) 1, red 2,
        // siamese 3, british_shorthair 4, calico 5, persian 6, ragdoll 7, tabby 8, all_black 9, jellie 10.
        Kind::Cat => Some(*[9, 1, 4, 5, 10, 6, 7, 2, 3, 8, 0].get(usize::from(m.tame.variant)).unwrap_or(&1)),
        Kind::Horse => { d.push((43, Int((m.horse.variant >> 8) & 0xff))); Some(m.horse.variant & 0xff) }
        Kind::Llama | Kind::TraderLlama => {
            d.push((75, Int(i32::from(m.land.strength))));
            d.push((76, Int(5)));
            Some(i32::from(m.land.variant))
        }
        Kind::Rabbit | Kind::Fox => Some(i32::from(m.wild.variant)),
        Kind::Parrot => Some(i32::from(m.small.variant)),
        Kind::Mooshroom => Some(i32::from(m.land.brown)),
        Kind::Panda => Some(i32::from(m.land.personality())),
        Kind::Axolotl | Kind::Frog => Some(m.water.variant),
        Kind::TropicalFish => {
            let (shape, pattern, base, overlay) = crate::mob::water::unpack_variant(m.water.variant).unwrap_or((0, 0, 0, 0));
            d.extend([(43, Int(i32::from(pattern))), (3, Byte(base as i8)), (82, Byte(overlay as i8))]);
            Some(i32::from(shape))
        }
        Kind::Slime | Kind::MagmaCube => Some(m.nether.size.max(1)),
        _ => None,
    };
    if let Some(v) = variant { d.push((2, Int(v))); }
    match m.kind {
        // Ошейник у BDS 0, пока зверь дикий; у прирученного — цвет (по умолчанию красный, как у Java).
        Kind::Wolf | Kind::Cat => d.push((3, Byte(if m.tame.owner.is_some() { m.tame.collar.min(15) as i8 } else { 0 }))),
        // Цвет панциря у BDS — `variant` (16 — без окраски), а `color` остаётся 0 [проверить: порядок красок].
        Kind::Shulker => d.extend([(2, Int(i32::from(m.h.shell_color))), (64, Byte(m.h.peek as i8)), (65, Byte(m.h.face as i8))]),
        Kind::Pufferfish => d.push((86, Byte(m.water.puff as i8))),
        Kind::Goat => d.push((122, Int(if m.is_baby() { 0 } else { m.land.horns.iter().filter(|v| **v).count() as i32 }))),
        Kind::Creeper => d.extend([(20, Int(m.h.fuse)), (21, Int(m.h.swell_dir))]),
        Kind::Enderman => d.push((23, Int(m.h.carried.map_or(0, |s| super::world::bedrock_state(s) as i32)))),
        _ => {}
    }
    // Постоянные значения вида, которые BDS шлёт сверх общих: тип монстра, вместимость, шаги и т. п.
    if let Some(p) = super::bds_profile::of(m.kind) {
        for (key, value) in p.extra {
            // У детёныша верблюда вместимости нет (запись b-camel).
            if m.is_baby() && m.kind == Kind::Camel && matches!(key, 44 | 45) { continue; }
            if !d.iter().any(|(k, _)| k == key) { d.push((*key, value.value())); }
        }
    }
    d
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extended_flags_use_mojang_absolute_numbers() {
        let mut b = [0; 2];
        for n in [0, 63, 64, 65, 104, 107, 126] { flag(&mut b, n, true); }
        assert_eq!(b, [1 | (1 << 63), 3 | (1 << 40) | (1 << 43) | (1 << 62)]);
        let mut m = Mob::new(1, Kind::Warden, (0.0, 0.0, 0.0));
        m.warden.pose = crate::mob::warden::Pose::Emerging;
        assert_ne!(flags(&m)[1] & (1 << 40), 0);
        assert_eq!(flags(&m)[1] & (1 << 39), 0);
    }
    #[test]
    fn cat_registry_order_and_horse_markings_are_translated() {
        let mut m = Mob::new(1, Kind::Cat, (0.0, 0.0, 0.0));
        m.tame.variant = 0;
        assert!(entries(&m).contains(&(2, Value::Int(9)))); // all_black
        m.tame.variant = 10;
        assert!(entries(&m).contains(&(2, Value::Int(0)))); // white
        m.kind = Kind::Horse;
        m.horse.variant = 3 | (4 << 8);
        assert!(entries(&m).contains(&(2, Value::Int(3))));
        assert!(entries(&m).contains(&(43, Value::Int(4))));
    }
    #[test]
    fn all_shared_kinds_have_a_client_identifier() {
        for kind in Kind::ALL { assert!(identifier(kind).starts_with("minecraft:")); }
        assert_eq!(identifier(Kind::EvokerFangs), "minecraft:evocation_fang");
        assert_eq!(identifier(Kind::Illusioner), identifier(Kind::Evoker));
    }
}
