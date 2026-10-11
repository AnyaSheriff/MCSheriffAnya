//! Снаряды общего мира; движение и попадание считает Java-логика.
//! Имена Bedrock — вики, Entity «Identifier»; данные — Mojang r/26_u1.
//! [проверить] Ориентация моделей и цвет нестандартных зелий на клиенте 944.
use super::{mobdata::Value, things::{self, Thing}};

fn actor(id: i32, name: &'static str, at: (f64, f64, f64), v: (f64, f64, f64)) -> Thing {
    let (yaw, pitch) = crate::arrow::angles_of(v);
    Thing { id, name, place: [at.0 as f32, at.1 as f32, at.2 as f32, -yaw, -pitch, -yaw],
        motion: [v.0 as f32, v.1 as f32, v.2 as f32],
        data: vec![(0, Value::Long((1 << 48) | (1 << 49)))] }
}
pub fn thrown(f: &crate::thrown::Flying) -> Thing {
    use crate::thrown::Kind::*;
    let at = (f.x, f.y, f.z);
    let v = (f.vx, f.vy, f.vz);
    let name = match f.kind {
        Splash | Lingering => {
            let potion = f.stack.as_ref().and_then(|s| s.extras().potion).and_then(|p| crate::survival::POTIONS.get(usize::from(p))).copied().unwrap_or("water");
            let mut thing = things::potion(f.entity_id, f.kind == Lingering, potion, at, v);
            // У BDS брошенное зелье несёт хозяина (owner_eid; запись p-throw-splash_potion).
            thing.data.push((5, Value::Long(i64::from(f.owner))));
            return thing;
        }
        DragonFireball => "minecraft:dragon_fireball",
        SmallFireball => "minecraft:small_fireball",
        GhastFireball => "minecraft:fireball",
        Snowball => "minecraft:snowball",
        Egg => "minecraft:egg",
        EnderPearl => "minecraft:ender_pearl",
        ExperienceBottle => "minecraft:xp_bottle",
        WitherSkull if f.dangerous => "minecraft:wither_skull_dangerous",
        WitherSkull => "minecraft:wither_skull",
        LlamaSpit => "minecraft:llama_spit",
        WindCharge => "minecraft:wind_charge_projectile",
        BreezeWindCharge => "minecraft:breeze_wind_charge_projectile",
    };
    let mut thing = actor(f.entity_id, name, at, v);
    // У BDS брошенные снежок, яйцо, жемчуг и пузырёк опыта хозяина в данных не несут (owner_eid -1; запись p-throw-*).
    let owner = if matches!(f.kind, Snowball | Egg | EnderPearl | ExperienceBottle) { -1 } else { i64::from(f.owner) };
    thing.data.push((5, Value::Long(owner)));
    if f.is_wind() || f.is_fireball() { thing.data[0] = (0, Value::Long(1 << 48)); }
    if f.is_fireball() {
        thing.data.extend([(29, Value::Float(f.push.0 as f32)), (30, Value::Float(f.push.1 as f32)), (31, Value::Float(f.push.2 as f32))]);
    }
    thing
}
pub fn arrow(a: &crate::arrow::Arrow) -> Thing {
    // У Bedrock нет spectral_arrow: обычная модель, общий эффект сохраняется.
    let mut thing = actor(a.entity_id, if a.trident.is_some() { "minecraft:thrown_trident" } else { "minecraft:arrow" }, (a.x, a.y, a.z), (a.vx, a.vy, a.vz));
    let (yaw, pitch) = a.facing();
    thing.place[3..].copy_from_slice(&[-yaw, -pitch, -yaw]);
    let mut flags = (1i64 << 48) | (1i64 << 49);
    if a.flame { flags |= 1; }
    if a.crit { flags |= 1 << 13; }
    if a.returning { flags |= 1 << 53; }
    thing.data = vec![(0, Value::Long(flags)), (5, Value::Long(i64::from(a.owner))), (139, Value::Long(i64::from(a.owner)))];
    let potion = a.potion.and_then(|p| crate::survival::POTIONS.get(usize::from(p)));
    thing.data.push((2, Value::Int(potion.map_or(0, |p| i32::from(things::bedrock_potion(p)) + 1))));
    thing
}
pub fn bobber(b: &crate::fishing::Bobber) -> Thing {
    let mut thing = actor(b.id, "minecraft:fishing_hook", b.at, b.velocity);
    thing.data.extend([(5, Value::Long(i64::from(b.owner))), (6, Value::Long(b.hooked.map_or(-1, i64::from)))]);
    thing
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{codec::In, mobdata::read_test_entries};
    fn round_trip(t: &Thing) {
        let p = things::add_packet(t);
        let mut r = In::new(&p);
        assert_eq!((r.varint(), r.zigzag64(), r.varint64()), (Some(13), Some(i64::from(t.id)), Some(t.id as u64)));
        assert_eq!(r.string().as_deref(), Some(t.name));
        for f in t.place[..3].iter().chain(t.motion.iter()) { assert_eq!(r.lf32(), Some(*f)); }
        for f in [t.place[4], t.place[3], t.place[5], t.place[3]] { assert_eq!(r.lf32(), Some(f)); }
        assert_eq!(r.varint(), Some(0));
        // Появление дополняет данные профилем BDS (размер, флаги по умолчанию).
        let mut want = t.data.clone();
        super::super::bds_profile::extend_thing(t.name, &mut want);
        assert_eq!(read_test_entries(&mut r), want);
        assert_eq!((r.varint(), r.varint(), r.varint(), r.remaining()), (Some(0), Some(0), Some(0), 0));
    }
    #[test]
    fn every_throwable_is_visible_and_round_trips() {
        use crate::thrown::Kind::*;
        for kind in [Splash, Lingering, DragonFireball, SmallFireball, GhastFireball, Snowball, Egg, EnderPearl, ExperienceBottle, WitherSkull, LlamaSpit, WindCharge, BreezeWindCharge] {
            let f = crate::thrown::Flying::mob_fireball(260, crate::dimension::Dimension::Overworld, kind, (1.0, 64.0, -2.0), (0.0, 0.0, 1.0), 20, 1);
            let t = thrown(&f);
            assert_ne!(t.name, "minecraft:arrow");
            round_trip(&t);
        }
    }
    #[test]
    fn trident_owner_loyalty_and_tipped_arrow_state_survive_encoding() {
        let mut a = crate::arrow::Arrow::from_mob(70, crate::dimension::Dimension::Overworld, (1.0, 64.0, 2.0), (0.3, -0.2, 0.5), 20, "skeleton");
        a.potion = crate::survival::potion_number("poison");
        let t = arrow(&a);
        assert_eq!(t.name, "minecraft:arrow");
        assert!(t.data.contains(&(2, Value::Int(26))));
        round_trip(&t);
        a.trident = crate::blocks::item_named("trident").map(|id| crate::inventory::Stack::new(id, 1));
        a.returning = true;
        let t = arrow(&a);
        assert_eq!(t.name, "minecraft:thrown_trident");
        assert!(t.data.contains(&(5, Value::Long(20))));
        assert!(matches!(t.data[0].1, Value::Long(f) if f & (1 << 53) != 0));
        round_trip(&t);
    }
    #[test]
    fn fishing_hook_has_owner_and_hooked_target() {
        let mut b = crate::fishing::Bobber::new(99, 20, crate::dimension::Dimension::Overworld, (1.0, 64.0, 2.0), (0.3, -0.2, 0.5), 0, 0.0);
        b.hooked = Some(50);
        let t = bobber(&b);
        assert_eq!(t.name, "minecraft:fishing_hook");
        assert!(t.data.contains(&(6, Value::Long(50))));
        round_trip(&t);
    }
}
