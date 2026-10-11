//! Профили видов, снятые с официального BDS 1.26.13.1 (наблюдение по сети,
//! `tools/blackbox/br-cases.js` + `br-profile.js`): что BDS шлёт клиенту при
//! появлении взрослого моба и детёныша — постоянные флаги, размеры, масштаб,
//! скорость, свойства и значения данных, которые не выводятся из состояния
//! Java. Таблица `bds_profile_table.rs` создаётся генератором, не руками.
//!
//! Главное, что показало сравнение: у BDS размер (`boundingbox_*`) детёныша —
//! не половина взрослого, а размер вида, и клиент сам умножает его на `scale`;
//! поэтому мы больше не делим размер на два, а шлём пару как BDS.

use super::mobdata::Value;
use crate::mob::Kind;

/// Значение постоянного ключа данных.
#[derive(Clone, Copy, Debug)]
pub enum V {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
}

impl V {
    pub fn value(self) -> Value {
        match self {
            V::Byte(v) => Value::Byte(v),
            V::Short(v) => Value::Short(v),
            V::Int(v) => Value::Int(v),
            V::Long(v) => Value::Long(v),
            V::Float(v) => Value::Float(v),
        }
    }
}

/// Детёныш: размер и масштаб, как их шлёт BDS.
pub struct Baby {
    pub width: f32,
    pub height: f32,
    pub scale: f32,
    /// Постоянные флаги детёныша (у BDS они бывают другими: счастливый гаст не «ходит»).
    pub flags: u64,
}

pub struct Profile {
    pub id: &'static str,
    /// Постоянные биты флагов (без состояний вроде «детёныш», «сидит»).
    pub flags: u64,
    pub flags2: u64,
    pub width: f32,
    pub height: f32,
    pub scale: f32,
    /// Скорость `minecraft:movement`; NaN — атрибута у вида нет.
    pub speed: f32,
    pub extra: &'static [(u32, V)],
    pub baby: Option<Baby>,
}

include!("bds_profile_table.rs");

pub fn of(kind: Kind) -> Option<&'static Profile> {
    let id = super::entity_state::identifier(kind);

    PROFILES.iter().find(|p| p.id == id)
}

/// Дополняет данные снаряда или транспорта тем, что BDS шлёт им при появлении: постоянные флаги,
/// размер, значения по умолчанию. Свои значения не затираются; флаги складываются.
pub fn extend_thing(name: &str, data: &mut Vec<(u32, Value)>) {
    let Some(p) = THINGS.iter().find(|p| p.id == name) else {
        return;
    };

    match data.iter_mut().find(|(key, _)| *key == 0) {
        Some((_, Value::Long(flags))) => *flags |= p.flags as i64,
        _ => data.insert(0, (0, Value::Long(p.flags as i64))),
    }

    let mut put = |key: u32, value: Value| {
        if !data.iter().any(|(k, _)| *k == key) {
            data.push((key, value));
        }
    };

    put(53, Value::Float(p.width));
    put(54, Value::Float(p.height));
    put(92, Value::Long(p.flags2 as i64));

    for (key, value) in p.extra {
        put(*key, value.value());
    }
}

/// Виды, которым BDS после появления флаг `breathing` не ставит (замечено по
/// `set_entity_data` в записи, `br-breathing.js`).
pub fn breathes(kind: Kind) -> bool {
    !matches!(kind, Kind::Blaze | Kind::CopperGolem | Kind::Creaking | Kind::EndCrystal | Kind::EnderDragon | Kind::IronGolem | Kind::Strider | Kind::Vex)
}

/// Постоянные флаги вида с учётом детёныша.
pub fn static_flags(p: &Profile, baby: bool) -> u64 {
    match (&p.baby, baby) {
        (Some(b), true) => b.flags,
        _ => p.flags,
    }
}

/// Размер и масштаб, которые клиент должен получить. Слаймы и магмовые кубики
/// берут размер из своего размера (у BDS он тот же: 0,52 на единицу).
pub fn shape(kind: Kind, baby: bool, java_size: (f64, f64)) -> (f32, f32, f32) {
    let Some(p) = of(kind) else {
        return (java_size.0 as f32, java_size.1 as f32, if baby { 0.5 } else { 1.0 });
    };

    if matches!(kind, Kind::Slime | Kind::MagmaCube) {
        return (java_size.0 as f32, java_size.1 as f32, 1.0);
    }

    match (baby, &p.baby) {
        (true, Some(b)) => (b.width, b.height, b.scale),
        (true, None) => (p.width, p.height, 0.5),
        _ => (p.width, p.height, p.scale),
    }
}

/// `minecraft:movement`: значение BDS для вида, умноженное на отношение
/// текущей скорости Java к базовой (эффекты, бег, бонус детёныша).
pub fn movement(kind: Kind, java_now: f64) -> Option<f32> {
    let p = of(kind)?;

    if p.speed.is_nan() {
        return None;
    }

    // Скорость лошадей случайна и у BDS, и у Java (0,1125–0,3375): берём значение Java как есть.
    if crate::mob::horse::equine(kind) && java_now > 0.0 {
        return Some(java_now as f32);
    }

    let base = kind.speed();

    Some(if base > 0.0 && java_now > 0.0 { p.speed * (java_now / base) as f32 } else { p.speed })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn table_is_sorted_by_identifier_and_covers_most_kinds() {
        assert!(PROFILES.windows(2).all(|w| w[0].id < w[1].id));
        let missing: Vec<_> = Kind::ALL.iter().filter(|k| of(**k).is_none()).map(|k| k.name()).collect();

        assert!(missing.len() <= 12, "без профиля: {missing:?}");
    }

    /// Запись BDS: у детёныша коровы размер взрослого, масштаб 0,5 (b-cow);
    /// у котёнка и кролика масштаб 0,4, у черепашки 0,16.
    #[test]
    fn a_baby_keeps_the_adult_box_and_uses_the_species_scale() {
        let adult = shape(Kind::Cow, false, (0.9, 1.4));
        let baby = shape(Kind::Cow, true, (0.45, 0.7));

        assert_eq!((adult.0, adult.1, adult.2), (0.9, 1.3, 1.0));
        assert_eq!((baby.0, baby.1, baby.2), (0.9, 1.3, 0.5));
        assert_eq!(shape(Kind::Cat, true, (0.3, 0.35)).2, 0.4);
        assert_eq!(shape(Kind::Rabbit, true, (0.2, 0.3)).2, 0.4);
        assert_eq!(shape(Kind::Turtle, true, (0.1, 0.1)).2, 0.16);
    }

    #[test]
    fn slimes_keep_the_java_size() {
        assert_eq!(shape(Kind::Slime, false, (1.04, 1.04)), (1.04, 1.04, 1.0));
    }

    #[test]
    fn movement_scales_the_bedrock_value_by_the_java_ratio() {
        let cow = movement(Kind::Cow, Kind::Cow.speed()).expect("у коровы есть скорость");

        assert!((cow - 0.25).abs() < 1e-6);
        assert!((movement(Kind::Cow, Kind::Cow.speed() * 2.0).unwrap() - 0.5).abs() < 1e-6);
        assert!(movement(Kind::EndCrystal, 0.0).is_none());
    }

    #[test]
    fn things_get_the_bds_box_and_flags_without_losing_their_own_data() {
        let mut data = vec![(0, Value::Long(1 << 48)), (11, Value::Int(3))];

        extend_thing("minecraft:minecart", &mut data);
        assert!(data.contains(&(11, Value::Int(3))));
        assert!(data.contains(&(53, Value::Float(0.98))));
        assert!(matches!(data[0], (0, Value::Long(f)) if f & (1 << 48) != 0 && f & (1 << 37) != 0));

        let mut none = vec![(0, Value::Long(5))];

        extend_thing("minecraft:no_such_thing", &mut none);
        assert_eq!(none, vec![(0, Value::Long(5))]);
    }

    #[test]
    fn walkers_breathe_and_golems_do_not() {
        assert!(breathes(Kind::Cow) && breathes(Kind::Zombie));
        assert!(!breathes(Kind::IronGolem) && !breathes(Kind::Blaze));
    }
}
