// Номера видов в наших файлах: это не номера сетевого протокола.
// Старые номера нельзя менять или переиспользовать.
use super::Kind;

pub(super) const EXTENDED: u8 = 0x80;
const WIDE: u8 = 0x7f;
// Старый вариант скелета с двумя таймерами замерзания после тела записи.
pub(super) const SNOW_SKELETON: u16 = 63;

// Одна таблица порождает оба направления; match по Kind остаётся полным,
// поэтому добавление вида требует назначить ему постоянный номер.
macro_rules! save_kinds {
    ($($kind:ident => $id:literal),+ $(,)?) => {
        impl Kind {
            pub(super) fn save_id(self) -> u16 {
                match self { $(Kind::$kind => $id,)+ }
            }

            pub(super) fn from_save_id(id: u16) -> Option<Self> {
                match id {
                    $($id => Some(Kind::$kind),)+
                    _ => None,
                }
            }
        }
    };
}

save_kinds! {
    Cow => 0,
    Pig => 1,
    Sheep => 2,
    Chicken => 3,
    Zombie => 4,
    Skeleton => 5,
    Creeper => 6,
    Spider => 7,
    EndCrystal => 8,
    EnderDragon => 9,
    Enderman => 10,
    Shulker => 11,
    ShulkerBullet => 12,
    Witch => 13,
    Villager => 14,
    ZombieVillager => 15,
    Blaze => 16,
    WitherSkeleton => 17,
    MagmaCube => 18,
    Ghast => 19,
    ZombifiedPiglin => 20,
    Piglin => 21,
    PiglinBrute => 22,
    Hoglin => 23,
    Strider => 24,
    Drowned => 25,
    Husk => 26,
    Slime => 27,
    Wolf => 28,
    Cat => 29,
    LeashKnot => 30,
    CaveSpider => 31,
    Silverfish => 32,
    IronGolem => 33,
    SnowGolem => 34,
    Phantom => 35,
    Guardian => 36,
    ElderGuardian => 37,
    Wither => 60,
    Stray => 61,
    Bogged => 62,
    Pillager => 64,
    Vindicator => 65,
    Evoker => 66,
    Vex => 67,
    Illusioner => 68,
    Ravager => 69,
    EvokerFangs => 70,
    Llama => 71,
    TraderLlama => 72,
    WanderingTrader => 73,
    Camel => 74,
    Goat => 75,
    PolarBear => 76,
    Panda => 77,
    Ocelot => 78,
    Mooshroom => 79,
    Breeze => 80,
    Cod => 81,
    Salmon => 82,
    TropicalFish => 83,
    Pufferfish => 84,
    Squid => 85,
    GlowSquid => 86,
    Dolphin => 87,
    Turtle => 88,
    Axolotl => 89,
    Frog => 90,
    Tadpole => 91,
    Warden => 92,
    Horse => 93,
    Donkey => 94,
    Mule => 95,
    SkeletonHorse => 96,
    ZombieHorse => 97,
    Rabbit => 98,
    Fox => 99,
    Bee => 100,
    Armadillo => 101,
    Sniffer => 102,
    Parrot => 103,
    Bat => 104,
    Allay => 105,
}

// 0x7f/0xff раньше не обозначали ни один вид. Теперь за ними идёт u16 BE;
// бит EXTENDED по-прежнему относится только к общему хвосту, не к номеру.
pub(super) fn write_head(out: &mut Vec<u8>, id: u16, extended: bool) {
    out.push(WIDE | if extended { EXTENDED } else { 0 });
    out.extend_from_slice(&id.to_be_bytes());
}

pub(super) fn read_head(bytes: &mut &[u8]) -> Option<(u16, bool)> {
    let (&head, rest) = bytes.split_first()?;
    *bytes = rest;
    let id = if head & !EXTENDED == WIDE {
        let (id, rest) = bytes.split_at_checked(2)?;
        *bytes = rest;
        u16::from_be_bytes(id.try_into().ok()?)
    } else {
        u16::from(head & !EXTENDED)
    };
    Some((id, head & EXTENDED != 0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mob::{Mob, leash};

    // Байты прежнего писателя: без вызовов нового write и без таблицы save_id.
    fn legacy_record(id: u8, holder: Option<u8>) -> Vec<u8> {
        let mut bytes = vec![id | if holder.is_some() { 0x80 } else { 0 }];
        for value in [12.25f64, 64.0, -3.5] { bytes.extend(value.to_be_bytes()); }
        for value in [90.0f32, 1.5] { bytes.extend(value.to_be_bytes()); }
        for value in [-1200i32, 30] { bytes.extend(value.to_be_bytes()); }
        bytes.extend([0xf7, 14]);
        bytes.extend(1234i32.to_be_bytes());
        match id {
            10 => bytes.extend(42i32.to_be_bytes()),
            11 => bytes.extend([4, 12]),
            27 => { bytes.extend(3i32.to_be_bytes()); bytes.push(3); }
            63 => { bytes.extend(123i32.to_be_bytes()); bytes.extend(77i32.to_be_bytes()); }
            _ => {}
        }
        if let Some(holder) = holder {
            let name = "Старое имя".as_bytes();
            bytes.push(3);
            bytes.extend((name.len() as u16).to_be_bytes());
            bytes.extend(name);
            bytes.push(holder);
            match holder {
                1 => bytes.extend([0x42; 16]),
                2 => { for value in [-12i32, 65, 32] { bytes.extend(value.to_be_bytes()); } }
                _ => {}
            }
        }
        bytes
    }

    #[test]
    fn old_records_keep_their_fields_and_tails() {
        for (id, kind) in [
            (0, Kind::Cow), (2, Kind::Sheep), (5, Kind::Skeleton),
            (10, Kind::Enderman), (11, Kind::Shulker), (27, Kind::Slime),
            (61, Kind::Stray), (62, Kind::Bogged), (63, Kind::Skeleton), (92, Kind::Warden),
        ] {
            for holder in [None, Some(0), Some(1), Some(2)] {
                let old = legacy_record(id, holder);
                let mut rest = old.as_slice();
                let mob = Mob::read(&mut rest).expect("старая запись читается");
                assert!(rest.is_empty());
                assert_eq!(mob.kind, kind);
                assert_eq!((mob.x, mob.y, mob.z, mob.yaw, mob.head_yaw), (12.25, 64.0, -3.5, 90.0, 90.0));
                assert_eq!((mob.vitals.health, mob.age, mob.love, mob.color, mob.egg_timer), (1.5, -1200, 30, 14, 1234));
                assert!(mob.no_ai && mob.sheared && mob.h.helmet && mob.h.bow && mob.h.offhand_shell);
                assert!(!mob.h.lit);
                assert_eq!(mob.h.hand, 3);
                assert_eq!(mob.name.as_deref(), holder.map(|_| "Старое имя"));
                assert_eq!(mob.persistent_tag, holder.is_some());
                assert_eq!(mob.leash, match holder {
                    Some(1) => Some(leash::Holder::Player { id: 0, uuid: [0x42; 16] }),
                    Some(2) => Some(leash::Holder::Knot { id: 0, at: (-12, 65, 32) }),
                    _ => None,
                });
                if id == 10 { assert_eq!(mob.h.carried, Some(42)); }
                if id == 11 { assert_eq!((mob.h.face, mob.h.shell_color), (4, 12)); }
                if id == 27 { assert_eq!(mob.nether.size, 3); assert!(mob.silent && mob.nether.persistence_required); }
                assert_eq!((mob.h.water_ticks, mob.h.converting), if id == 63 { (123, 77) } else { (0, 0) });

                let mut new = Vec::new();
                mob.write(&mut new);
                assert_eq!(new[0], if holder.is_some() { 0xff } else { 0x7f });
                assert_eq!(&new[1..3], &u16::from(id).to_be_bytes());
                assert_eq!(&new[3..], &old[1..], "тело и хвост не меняются");
                assert!(Mob::read(&mut new.as_slice()).is_some());
                for end in 0..old.len() {
                    assert!(Mob::read(&mut &old[..end]).is_none(), "обрезанная старая запись {id}, {end}");
                }
                for end in 0..new.len() {
                    assert!(Mob::read(&mut &new[..end]).is_none(), "обрезанная новая запись {id}, {end}");
                }
            }
        }
    }

    #[test]
    fn every_kind_survives_new_records_with_and_without_common_tails() {
        for kind in Kind::ALL {
            for extended in [false, true] {
                let mut mob = Mob::new(0, kind, (1.5, 64.0, -3.25));
                mob.persistent_tag = extended;
                let mut bytes = Vec::new();
                mob.write(&mut bytes);
                assert_eq!(bytes[0], if extended { 0xff } else { 0x7f });
                let mut rest = bytes.as_slice();
                let loaded = Mob::read(&mut rest).expect("новая запись читается");
                assert!(rest.is_empty());
                assert_eq!(loaded.kind, kind);
                assert_eq!(loaded.persistent_tag, extended);
                let mut again = Vec::new();
                loaded.write(&mut again);
                assert_eq!(again, bytes, "{kind:?}");
            }
        }
    }

    #[test]
    fn mixed_old_and_new_records_preserve_boundaries_and_reject_unknown_ids() {
        let mut bytes = legacy_record(63, Some(2));
        Mob::new(0, Kind::Warden, (0.0, 64.0, 0.0)).write(&mut bytes);
        bytes.extend(legacy_record(11, None));
        let mut rest = bytes.as_slice();
        for kind in [Kind::Skeleton, Kind::Warden, Kind::Shulker] {
            assert_eq!(Mob::read(&mut rest).unwrap().kind, kind);
        }
        assert!(rest.is_empty());
        for id in [38, 93, 127, 128, 255, 256, 4096, u16::MAX] {
            let mut bytes = Vec::new();
            write_head(&mut bytes, id, true);
            bytes.extend(&legacy_record(0, Some(0))[1..]);
            assert!(Mob::read(&mut bytes.as_slice()).is_none());
        }
        for id in [38, 93, 126] {
            assert!(Mob::read(&mut legacy_record(id, None).as_slice()).is_none());
        }
    }

    #[test]
    fn every_kind_has_a_unique_save_id_and_round_trips() {
        let mut ids = std::collections::HashSet::new();
        let mut kinds = std::collections::HashSet::new();
        for kind in Kind::ALL {
            let id = kind.save_id();
            assert!(ids.insert(id), "повтор номера {id}");
            assert!(kinds.insert(kind), "повтор вида {kind:?}");
            assert_ne!(id, SNOW_SKELETON, "номер старого варианта скелета занят");
            assert_eq!(Kind::from_save_id(id), Some(kind));
        }
        for id in 0..=u16::MAX {
            if let Some(kind) = Kind::from_save_id(id) {
                assert!(kinds.contains(&kind));
                assert_eq!(kind.save_id(), id);
            }
        }
    }

    #[test]
    fn wide_headers_preserve_all_id_bits_and_the_tail_flag() {
        for id in [0, 63, 92, 127, 128, 255, 256, 4096, 32768, u16::MAX] {
            for extended in [false, true] {
                let mut bytes = Vec::new();
                write_head(&mut bytes, id, extended);
                assert_eq!(bytes.len(), 3);
                assert_eq!(&bytes[1..], &id.to_be_bytes());
                bytes.push(42);
                let mut rest = bytes.as_slice();
                assert_eq!(read_head(&mut rest), Some((id, extended)));
                assert_eq!(rest, &[42]);
            }
        }
        for bytes in [&[][..], &[WIDE], &[WIDE, 0], &[WIDE | EXTENDED], &[WIDE | EXTENDED, 0]] {
            assert!(read_head(&mut &bytes[..]).is_none());
        }
    }
}
