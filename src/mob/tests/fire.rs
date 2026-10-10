use super::*;

struct Hot(i32);
impl View for Hot {
    fn block(&self, _x: i32, y: i32, _z: i32) -> i32 {
        if y == 1 { self.0 } else if y < 1 { blocks::state_by_name("stone").unwrap() } else { 0 }
    }
    fn loaded(&self, _: i32, _: i32) -> bool { true }
}

#[test]
fn fire_burns_passive_mobs_but_not_crystals_or_blazes() {
    let view = Hot(blocks::state_by_name("fire").unwrap());
    let mut cow = Mob::new(1, Kind::Cow, (0.5, 1.0, 0.5));
    cow.no_ai = true;
    run(&mut cow, &view, 1);
    assert!(cow.vitals.health < Kind::Cow.max_health());
    assert!(cow.vitals.fire > 0);
    let mut crystal = Mob::new(2, Kind::EndCrystal, (0.5, 1.0, 0.5));
    run(&mut crystal, &view, 40);
    assert!(!crystal.exploding);
    let mut blaze = Mob::new(3, Kind::Blaze, (0.5, 1.0, 0.5));
    blaze.no_ai = true;
    run(&mut blaze, &view, 40);
    assert_eq!(blaze.vitals.health, Kind::Blaze.max_health());
    assert_eq!(blaze.vitals.fire, 0);
}

#[test]
fn campfire_contact_damages_without_setting_mob_on_fire() {
    for (name, damage) in [("campfire", 1.0), ("soul_campfire", 2.0)] {
        let view = Hot(blocks::state_by_name(name).unwrap());
        let mut cow = Mob::new(1, Kind::Cow, (0.5, 1.0, 0.5));
        cow.no_ai = true;
        run(&mut cow, &view, 1);
        assert_eq!(cow.vitals.health, Kind::Cow.max_health() - damage);
        assert_eq!(cow.vitals.fire, 0);
    }
}
