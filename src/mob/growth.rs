// Вики, Golden Dandelion (oldid=3525498): «resets and locks its aging time»,
// «2 second use cooldown per mob». Particles и все feed interactions [проверить].
use super::{Event, Kind, Mob, BABY_AGE};

pub fn eligible(kind: Kind) -> bool {
    matches!(kind, Kind::Cow | Kind::Pig | Kind::Sheep | Kind::Chicken | Kind::Wolf | Kind::Cat | Kind::Llama | Kind::TraderLlama | Kind::Camel | Kind::Goat | Kind::PolarBear | Kind::Panda | Kind::Ocelot | Kind::Mooshroom | Kind::Hoglin | Kind::Strider)
}

pub fn tick_age(mob: &mut Mob) {
    if mob.age < 0 && (!mob.kind.hostile() || eligible(mob.kind)) && !mob.age_locked {
        mob.age += 1;
    } else if mob.age > 0 { mob.age -= 1; }
}

pub fn toggle(mob: &mut Mob) -> bool {
    if !eligible(mob.kind) || !mob.is_baby() || mob.is_dead() || mob.ticks < mob.growth_cooldown { return false; }
    mob.age_locked = !mob.age_locked;
    mob.age = BABY_AGE;
    mob.persistent_tag = true;
    mob.growth_cooldown = mob.ticks + 40;
    mob.events.push(Event::EntitySound { name: if mob.age_locked { "minecraft:item.golden_dandelion.use" } else { "minecraft:item.golden_dandelion.unuse" }, category: 4, volume: 1.0, pitch: 1.0 });
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn golden_dandelion_toggles_with_per_mob_cooldown_and_persists() {
        for kind in [Kind::Cow,Kind::Sheep,Kind::Pig,Kind::Chicken,Kind::Wolf,Kind::Cat,Kind::Goat] {
            let mut m=Mob::new(1,kind,(0.5,1.0,0.5));m.age = -10;
            assert!(toggle(&mut m));assert!(m.age_locked && m.persistent_tag);assert_eq!(m.age,BABY_AGE);
            for _ in 0..100 {tick_age(&mut m);} assert_eq!(m.age,BABY_AGE);
            assert!(!toggle(&mut m));m.ticks=39;assert!(!toggle(&mut m));
            let mut bytes=Vec::new();m.write(&mut bytes);let copy=Mob::read(&mut bytes.as_slice()).unwrap();
            assert!(copy.age_locked && copy.persistent_tag);assert_eq!(copy.age,BABY_AGE);
            m.ticks=40;assert!(toggle(&mut m));assert!(!m.age_locked);
            tick_age(&mut m);assert_eq!(m.age,BABY_AGE+1);
        }
        let mut adult=Mob::new(1,Kind::Cow,(0.0,0.0,0.0));assert!(!toggle(&mut adult));
        for kind in [Kind::Villager,Kind::Zombie,Kind::ZombieVillager] {
            let mut m=Mob::new(1,kind,(0.0,0.0,0.0));m.age = -10;assert!(!toggle(&mut m));
        }
    }
}
