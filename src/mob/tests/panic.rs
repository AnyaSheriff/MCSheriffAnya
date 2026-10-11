use super::*;

#[test]
fn punched_cows_ramp_turn_and_remain_nearby_for_sixty_ticks() {
    let view = Flat::new();
    let mut early = 0.0;
    let mut late = 0.0;
    let mut radius = 0.0;
    let mut turns_back = 0;
    for seed in 1..=64 {
        let mut cow = Mob::new(1, Kind::Cow, (0.5, 1.0, 0.5));
        cow.on_ground = true;
        cow.vy = -GRAVITY * VERTICAL_DRAG;
        cow.hurt(1.0, Source::PlayerAttack, Some(Push {
            dir: (0.0, 1.0), strength: BASE_KNOCKBACK, cause: 7,
        })).unwrap();
        let mut rng = Rng::new(seed);
        let mut peak: f64 = 0.0;
        let mut farthest: f64 = 0.0;
        let mut reversed = false;
        for tick in 0..60 {
            let before = (cow.x, cow.z);
            cow.tick(&view, &ctx(), &mut rng);
            let speed = (cow.x - before.0).hypot(cow.z - before.1);
            // Первые три такта — сам толчок; сравниваем его затухающий хвост.
            if (3..9).contains(&tick) { early += speed / 6.0; }
            if (15..40).contains(&tick) { peak = peak.max(speed); }
            reversed |= tick > 10 && cow.z < before.1 - 0.03;
            farthest = farthest.max((cow.x - 0.5).hypot(cow.z - 0.5));
            assert!(speed < 0.61, "seed={seed} tick={tick} speed={speed}");
        }
        late += peak;
        radius += farthest;
        turns_back += i32::from(reversed);
    }
    early /= 64.0;
    late /= 64.0;
    radius /= 64.0;
    // Чёрный ящик, panic-v1/v2: хвост отдачи 0,11–0,17, бег 0,30–0,35.
    // Записи обрезаются радиусом слежения: это статистические границы,
    // не искусственная привязка случайного пути к месту удара [проверить].
    assert!((0.10..0.24).contains(&early), "early={early}");
    assert!((0.30..0.36).contains(&late), "late={late}");
    assert!(early < late * 0.75, "early={early} late={late}");
    assert!(turns_back >= 48, "turns_back={turns_back}");
    assert!(radius < 7.0, "mean maximum radius={radius}");
}

#[test]
fn panic_expires_in_air_and_finishes_its_last_route_without_restarting() {
    let view = Flat::new();
    let mut cow = Mob::new(1, Kind::Cow, (0.5, 100.0, 0.5));
    cow.hurt(1.0, Source::PlayerAttack, None).unwrap();
    let mut rng = Rng::new(8);
    for _ in 0..40 { cow.tick(&view, &ctx(), &mut rng); }
    assert!(!cow.brain.is_panicking(), "время не заморожено в воздухе");

    cow = Mob::new(1, Kind::Cow, (0.5, 1.0, 0.5));
    cow.on_ground = true;
    cow.brain.panic(2);
    cow.brain.go(vec![(0, 1, 5)], 2.0);
    for _ in 0..60 {
        cow.tick(&view, &ctx(), &mut rng);
        if !cow.brain.is_panicking() { break; }
    }
    assert!(!cow.brain.is_panicking());
    assert!(!cow.brain.walking(), "путь паники не перешёл в прогулку");
    assert!((cow.z - 5.5).abs() < 0.6);
}

#[test]
fn damage_tags_and_neutral_animals_choose_panic_without_requiring_a_push() {
    for kind in [Kind::Cow, Kind::Sheep, Kind::Pig, Kind::Chicken, Kind::Mooshroom] {
        for source in [Source::PlayerAttack, Source::Arrow, Source::Magic, Source::Cactus,
            Source::InFire, Source::OnFire, Source::Lava, Source::Freeze] {
            let mut mob = Mob::new(1, kind, (0.5, 1.0, 0.5));
            mob.hurt(1.0, source, None).unwrap();
            assert!(mob.brain.is_panicking(), "{kind:?} {source:?}");
        }
        for source in [Source::Fall, Source::Generic, Source::Drown, Source::Campfire] {
            let mut mob = Mob::new(1, kind, (0.5, 1.0, 0.5));
            mob.hurt(1.0, source, None).unwrap();
            assert!(!mob.brain.is_panicking(), "{kind:?} {source:?}");
        }
    }
    for kind in [Kind::Wolf, Kind::Panda, Kind::PolarBear] {
        let mut mob = Mob::new(1, kind, (0.5, 1.0, 0.5));
        assert!(!ai::panic::on_hurt(&mob, Source::PlayerAttack));
        assert!(ai::panic::on_hurt(&mob, Source::InFire));
        mob.age = -100;
        assert_eq!(ai::panic::on_hurt(&mob, Source::PlayerAttack), kind == Kind::PolarBear);
    }
    for kind in [Kind::Zombie, Kind::Bee, Kind::IronGolem, Kind::SkeletonHorse, Kind::ZombieHorse] {
        let mob = Mob::new(1, kind, (0.5, 1.0, 0.5));
        assert!(!ai::panic::on_hurt(&mob, Source::PlayerAttack));
    }
}

#[test]
fn burning_cow_interrupts_grazing_and_heads_for_reachable_water() {
    let mut view = Flat::new();
    view.water.push((2, 1, 0));
    let mut cow = Mob::new(1, Kind::Cow, (0.5, 1.0, 0.5));
    cow.on_ground = true;
    cow.brain.eating = 40;
    cow.vitals.ignite(160);
    let mut rng = Rng::new(11);
    let plan = cow.tick(&view, &ctx(), &mut rng);
    assert_eq!(cow.brain.doing, ai::Doing::Panic);
    assert_eq!(cow.brain.eating, 0);
    assert_eq!(plan.speed, 2.0);
    for _ in 0..30 { cow.tick(&view, &ctx(), &mut rng); }
    assert_eq!(cow.vitals.fire, 0);
}

#[test]
fn adult_and_baby_walks_keep_ground_speeds_and_scale_air_input() {
    let view = Flat::new();
    for (kind, modifier, terminal) in [(Kind::Cow, 1.0, 0.0881057),
        (Kind::Cow, 2.0, 0.3524229), (Kind::Pig, 2.0, 0.5506608),
        (Kind::Sheep, 2.0, 0.4660793), (Kind::Chicken, 1.4, 0.2698238)] {
        for age in [0, -24000] {
            let mut mob = Mob::new(1, kind, (0.5, 1.0, 0.5));
            mob.age = age;
            mob.on_ground = true;
            let mut first = 0.0;
            let mut last = 0.0;
            for tick in 0..20 {
                let before = mob.z;
                mob.physics(&view, modifier, false);
                last = mob.z - before;
                if tick == 0 { first = last; }
            }
            assert!((last - terminal).abs() < 0.0001, "{kind:?}: {last}");
            assert!(first < last * 0.5);
            mob.y = 100.0;
            mob.on_ground = false;
            mob.vz = 0.0;
            let before = mob.z;
            mob.physics(&view, modifier, false);
            assert!((mob.z - before - 0.02 * modifier * mob.movement_speed()).abs() < 1.0e-9);
        }
    }
}

#[test]
fn panic_navigation_does_not_depend_on_the_attackers_position() {
    let view = Flat::new();
    for seed in 1..=32 {
        let mut a = Mob::new(1, Kind::Cow, (0.5, 1.0, 0.5));
        a.on_ground = true;
        a.hurt(1.0, Source::PlayerAttack, None).unwrap();
        let mut b = a.clone();
        a.fright = Some((0.5, -8.0));
        b.fright = Some((0.5, 8.0));
        let mut rng_a = Rng::new(seed);
        let mut rng_b = Rng::new(seed);
        for _ in 0..60 {
            a.tick(&view, &ctx(), &mut rng_a);
            b.tick(&view, &ctx(), &mut rng_b);
            assert_eq!((a.x, a.y, a.z), (b.x, b.y, b.z));
        }
    }
}

#[test]
fn walking_and_coasting_preserve_ground_slipperiness() {
    struct Floor(&'static str);
    impl View for Floor {
        fn block(&self, _: i32, y: i32, _: i32) -> i32 {
            if y <= 0 { blocks::state_by_name(self.0).unwrap() } else { 0 }
        }
        fn loaded(&self, _: i32, _: i32) -> bool { true }
    }
    let mut coast = Vec::new();
    for block in ["stone", "ice"] {
        let mut cow = Mob::new(1, Kind::Cow, (0.5, 1.0, 0.5));
        cow.on_ground = true;
        for _ in 0..40 { cow.physics(&Floor(block), 1.0, false); }
        let start = cow.z;
        for _ in 0..10 { cow.physics(&Floor(block), 0.0, false); }
        coast.push(cow.z - start);
    }
    assert!(coast[0] < 0.11, "stone={}", coast[0]);
    assert!(coast[1] > coast[0] * 4.0, "ice={}", coast[1]);
}
