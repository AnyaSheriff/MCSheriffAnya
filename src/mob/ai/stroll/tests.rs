use super::*;
use crate::{blocks, mob::{Mob, ai}};

// Пруд на уровне земли, один блок глубиной, берег без ограды.
struct Pond;
impl View for Pond {
    fn block(&self, x: i32, y: i32, z: i32) -> i32 {
        let pond = (0..=4).contains(&x) && (-3..=3).contains(&z);
        if y == 0 && pond { blocks::state_by_name("water").unwrap() }
        else if y <= 0 { blocks::state_by_name("stone").unwrap() }
        else { 0 }
    }
    fn loaded(&self, _: i32, _: i32) -> bool { true }
}

#[test]
fn land_mobs_never_end_a_stroll_in_the_pond() {
    for kind in [Kind::Cow, Kind::Pig, Kind::Sheep, Kind::Chicken, Kind::Zombie, Kind::Skeleton] {
        let mut rng = Rng::new(19);
        let mut walks = 0;
        for _ in 0..1000 {
            let path = route(kind, &Pond, (-1, 1, 0), &mut rng);
            if let Some(&end) = path.last() {
                assert!(land(&Pond, end), "{kind:?}: {end:?}");
                walks += 1;
            }
        }
        assert!(walks > 900, "{kind:?}: {walks}");
    }
}

#[test]
fn passive_and_hostile_idle_loops_use_land_destinations() {
    let ctx = ai::Ctx { players: &[], mates: &[], difficulty: 2, sky: 4, sun_up: false };
    for kind in [Kind::Cow, Kind::Zombie, Kind::Skeleton] {
        let mut mob = Mob::new(1, kind, (-0.5, 1.0, 0.5));
        mob.on_ground = true;
        let mut rng = Rng::new(31);
        let mut walks = 0;
        for _ in 0..20000 {
            mob.brain.stop();
            ai::think(&mut mob, &Pond, &ctx, &mut rng);
            if let Some(&end) = mob.brain.path.last() {
                assert!(land(&Pond, end), "{kind:?}: {end:?}");
                walks += 1;
            }
        }
        assert!(walks > 50, "{kind:?}: {walks}");
    }
}

#[test]
fn cow_in_water_selects_land_above_its_feet() {
    let mut rng = Rng::new(71);
    for _ in 0..300 {
        let path = route(Kind::Cow, &Pond, (2, 0, 0), &mut rng);
        let &end = path.last().expect("рядом есть берег");
        assert!(land(&Pond, end), "{end:?}");
        assert_eq!(end.1, 1);
    }
}

#[test]
fn drowned_can_choose_water_and_amphibians_are_not_filtered() {
    for kind in [Kind::Drowned, Kind::Turtle, Kind::Axolotl, Kind::Frog] {
        let mut rng = Rng::new(19);
        assert!((0..1000).any(|_| route(kind, &Pond, (-1, 1, 0), &mut rng)
            .last().is_some_and(|&end| path::water_cell(&Pond, end))), "{kind:?}");
    }
    for kind in [Kind::Guardian, Kind::ElderGuardian, Kind::Dolphin, Kind::Squid,
        Kind::GlowSquid, Kind::Cod, Kind::Salmon, Kind::Pufferfish, Kind::TropicalFish,
        Kind::Tadpole, Kind::Strider] {
        assert!(!avoids_water(kind), "{kind:?}");
    }
}

#[test]
fn cow_does_not_accept_a_partial_path_ending_in_water() {
    struct Ocean;
    impl View for Ocean {
        fn block(&self, x: i32, y: i32, z: i32) -> i32 {
            if (4..=6).contains(&x) && (-1..=1).contains(&z) && y <= 4 {
                return blocks::state_by_name("stone").unwrap();
            }
            if y <= 0 { blocks::state_by_name("water").unwrap() } else { 0 }
        }
        fn loaded(&self, _: i32, _: i32) -> bool { true }
    }
    let partial = path::find(&Ocean, (0, 0, 0), (5, 5, 0), path::MAX_NODES);
    assert!(partial.last().is_some_and(|&end| path::water_cell(&Ocean, end)));
    let mut rng = Rng::new(27);
    for _ in 0..100 {
        assert!(route(Kind::Cow, &Ocean, (0, 0, 0), &mut rng).is_empty());
    }
}

#[test]
fn cow_floats_and_leaves_the_pond() {
    let mut mob = Mob::new(1, Kind::Cow, (2.5, 0.0, 0.5));
    let ctx = ai::Ctx { players: &[], mates: &[], difficulty: 2, sky: 15, sun_up: false };
    let mut rng = Rng::new(71);
    let mut rose = false;
    let mut escaped = false;
    for _ in 0..1200 {
        mob.tick(&Pond, &ctx, &mut rng);
        rose |= mob.y > 0.5;
        escaped |= mob.on_ground && land(&Pond, mob.cell());
    }
    assert!(rose, "корова всплывает");
    assert!(escaped, "корова выбирается: {},{},{}", mob.x, mob.y, mob.z);
}
