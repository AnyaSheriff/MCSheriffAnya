use super::*;

fn amount(stacks: &[Stack], name: &str) -> i32 {
    stacks.iter().filter(|stack| blocks::item_name(stack.item) == Some(name)).map(|stack| stack.count).sum()
}

#[test]
fn looting_adds_common_drops_at_every_level_without_changing_plain_ranges() {
    let cases = [
        (Kind::Cow, "beef", 1, 3), (Kind::Cow, "leather", 0, 2),
        (Kind::Mooshroom, "beef", 1, 3), (Kind::Pig, "porkchop", 1, 3),
        (Kind::Sheep, "mutton", 1, 2), (Kind::Chicken, "chicken", 1, 1),
        (Kind::Chicken, "feather", 0, 2), (Kind::Zombie, "rotten_flesh", 0, 2),
        (Kind::Husk, "rotten_flesh", 0, 2), (Kind::ZombieVillager, "rotten_flesh", 0, 2),
        (Kind::Creeper, "gunpowder", 0, 2), (Kind::Spider, "string", 0, 2),
        (Kind::Cat, "string", 0, 2), (Kind::Enderman, "ender_pearl", 0, 1),
        (Kind::Llama, "leather", 0, 2), (Kind::TraderLlama, "leather", 0, 2),
        (Kind::Horse, "leather", 0, 2), (Kind::Donkey, "leather", 0, 2),
        (Kind::Mule, "leather", 0, 2), (Kind::SkeletonHorse, "bone", 0, 2),
        (Kind::ZombieHorse, "rotten_flesh", 0, 2), (Kind::Drowned, "rotten_flesh", 0, 2),
        (Kind::Zoglin, "rotten_flesh", 1, 3), (Kind::Rabbit, "rabbit_hide", 0, 1),
        (Kind::Rabbit, "rabbit", 0, 1), (Kind::CaveSpider, "string", 0, 2),
        (Kind::Blaze, "blaze_rod", 0, 1), (Kind::Phantom, "phantom_membrane", 0, 1),
        (Kind::WitherSkeleton, "bone", 0, 2), (Kind::Ghast, "ghast_tear", 0, 1),
        (Kind::Ghast, "gunpowder", 0, 2), (Kind::ZombifiedPiglin, "rotten_flesh", 0, 1),
        (Kind::ZombifiedPiglin, "gold_nugget", 0, 1), (Kind::Hoglin, "porkchop", 2, 4),
        (Kind::Hoglin, "leather", 0, 1), (Kind::Strider, "string", 2, 5),
        (Kind::Parrot, "feather", 1, 2), (Kind::Squid, "ink_sac", 1, 3),
        (Kind::GlowSquid, "glow_ink_sac", 1, 3), (Kind::Dolphin, "cod", 0, 1),
        (Kind::Turtle, "seagrass", 0, 2),
    ];
    for (kind, name, minimum, maximum) in cases {
        for level in 0..=3 {
            let mut mob = Mob::new(1, kind, (0.0, 64.0, 0.0));
            mob.looting = level;
            mob.credit_player();
            let mut observed = (i32::MAX, 0);
            for seed in 0..512 {
                let count = amount(&loot(&mob, &mut Rng::new(seed)), name);
                assert!((minimum..=maximum + level).contains(&count), "{kind:?} {name} {level}: {count}");
                observed.0 = observed.0.min(count);
                observed.1 = observed.1.max(count);
            }
            assert_eq!(observed, (minimum, maximum + level), "{kind:?} {name} {level}");
        }
    }
}

#[test]
fn looting_keeps_wool_golem_drops_and_baby_exclusions_and_cooks_meat() {
    for seed in 0..128 {
        let mut sheep = Mob::new(1, Kind::Sheep, (0.0, 64.0, 0.0));
        sheep.looting = 3;
        let drops = loot(&sheep, &mut Rng::new(seed));
        assert_eq!(amount(&drops, "white_wool"), 1);
        sheep.sheared = true;
        assert_eq!(amount(&loot(&sheep, &mut Rng::new(seed)), "white_wool"), 0);
        for kind in [Kind::IronGolem, Kind::SnowGolem] {
            let mut mob = Mob::new(1, kind, (0.0, 64.0, 0.0));
            let plain = loot(&mob, &mut Rng::new(seed));
            mob.looting = 3;
            assert_eq!(loot(&mob, &mut Rng::new(seed)), plain);
        }
        for kind in [Kind::Cow, Kind::Mooshroom, Kind::Pig, Kind::Sheep, Kind::Chicken] {
            let mut mob = Mob::new(1, kind, (0.0, 64.0, 0.0));
            mob.looting = 3;
            mob.age = BABY_AGE;
            assert!(loot(&mob, &mut Rng::new(seed)).is_empty());
        }
        let mut cow = Mob::new(1, Kind::Cow, (0.0, 64.0, 0.0));
        cow.looting = 3;
        let raw = amount(&loot(&cow, &mut Rng::new(seed)), "beef");
        cow.vitals.fire = 100;
        let cooked = loot(&cow, &mut Rng::new(seed));
        assert_eq!(amount(&cooked, "beef"), 0);
        assert_eq!(amount(&cooked, "cooked_beef"), raw);
    }
}

#[test]
fn player_only_loot_uses_active_ranged_credit_and_expires() {
    for kind in [Kind::Blaze, Kind::Phantom, Kind::Breeze] {
        let mut mob = Mob::new(30, kind, (0.0, 64.0, 0.0));
        assert!(!mob.by_player);
        assert!(loot(&mob, &mut Rng::new(30 ^ 0x5EED)).is_empty());
        mob.credit_player();
        let mut any_drop = false;
        for seed in 0..64 { any_drop |= !loot(&mob, &mut Rng::new(seed)).is_empty(); }
        assert!(any_drop, "{kind:?}");
        mob.ticks = mob.xp_until;
        for seed in 0..64 { assert!(loot(&mob, &mut Rng::new(seed)).is_empty(), "{kind:?}"); }
    }
}

#[test]
fn common_looting_is_a_separate_roll_not_a_uniform_wider_range() {
    // Вики, Looting: прибавка round(U * level). Для базы 0..2 и Looting III
    // частоты шести исходов — 1:3:5:5:3:1, а не прежние равные 1:1:1:1:1:1.
    // Проверка распределения ловит ошибку, которую одинаковые min/max скрывают.
    for (kind, name) in [(Kind::Llama, "leather"), (Kind::Drowned, "rotten_flesh"),
        (Kind::CaveSpider, "string"), (Kind::Squid, "ink_sac")] {
        let mut mob = Mob::new(1, kind, (0.0, 64.0, 0.0));
        mob.looting = 3;
        mob.credit_player();
        let minimum = i32::from(kind == Kind::Squid);
        let mut histogram = [0; 6];
        let samples = 18_000;
        let mut rng = Rng::new(0x10_0719);
        for _ in 0..samples {
            let count = amount(&loot(&mob, &mut rng), name) - minimum;
            histogram[count as usize] += 1;
        }
        for (actual, weight) in histogram.into_iter().zip([1, 3, 5, 5, 3, 1]) {
            assert!((f64::from(actual) / f64::from(samples) - f64::from(weight) / 18.0).abs() < 0.015,
                "{kind:?}: {histogram:?}");
        }
    }
}

#[test]
fn looting_does_not_multiply_fish_drops_or_carried_equipment() {
    for seed in 0..128 {
        for kind in [Kind::Cod, Kind::Salmon, Kind::TropicalFish, Kind::Pufferfish] {
            let mut fish = Mob::new(1, kind, (0.0, 64.0, 0.0));
            let plain = loot(&fish, &mut Rng::new(seed));
            fish.looting = 3;
            assert_eq!(loot(&fish, &mut Rng::new(seed)), plain, "{kind:?}");
        }
        let mut llama = Mob::new(1, Kind::Llama, (0.0, 64.0, 0.0));
        llama.looting = 3;
        llama.land.chest = true;
        assert_eq!(amount(&loot(&llama, &mut Rng::new(seed)), "chest"), 1);
        for (kind, raw, cooked) in [(Kind::Rabbit, "rabbit", "cooked_rabbit"),
            (Kind::Hoglin, "porkchop", "cooked_porkchop"), (Kind::Dolphin, "cod", "cooked_cod")] {
            let mut mob = Mob::new(1, kind, (0.0, 64.0, 0.0));
            mob.looting = 3;
            let plain = amount(&loot(&mob, &mut Rng::new(seed)), raw);
            mob.vitals.fire = 100;
            let burning = loot(&mob, &mut Rng::new(seed));
            assert_eq!(amount(&burning, raw), 0);
            assert_eq!(amount(&burning, cooked), plain, "{kind:?}");
        }
    }
}

#[test]
fn rejected_hits_do_not_replace_killer_or_looting() {
    let mut mob = Mob::new(1, Kind::Cow, (0.0, 64.0, 0.0));
    mob.hurt_credited(1.0, Source::PlayerAttack, None, [1; 16], 1).unwrap();
    mob.ticks += 2;
    let before = (mob.kill_credit, mob.xp_until, mob.looting);
    assert!(mob.hurt_credited(1.0, Source::Thorns, None, [2; 16], 3).is_none());
    assert_eq!((mob.kill_credit, mob.xp_until, mob.looting), before);
}

#[test]
fn tamed_wolf_lethal_bite_grants_loot_and_xp_in_the_same_tick() {
    struct Floor;
    impl View for Floor {
        fn block(&self, _: i32, y: i32, _: i32) -> i32 {
            if y <= 0 { blocks::state_by_name("stone").unwrap() } else { 0 }
        }
        fn loaded(&self, _: i32, _: i32) -> bool { true }
    }
    for (tamed, own_looting) in [(false, 0), (true, 0), (false, 3), (true, 3)] {
        let mut wolf = Mob::new(5, Kind::Wolf, (0.5, 1.0, 0.5));
        if tamed { tame::tame(&mut wolf, ([7; 16], 1)); }
        if own_looting > 0 {
            let mut data = crate::stackdata::Data::default();
            data.set_enchant(crate::stackdata::enchantment_number("looting").unwrap(), own_looting);
            data.set_enchant(crate::stackdata::enchantment_number("fire_aspect").unwrap(), 2);
            equipment::set_slot(&mut wolf, 0, Some(crate::inventory::Stack::new(blocks::item_named("diamond_sword").unwrap(), 1).with_extras(data)));
        }
        wolf.tame.ordered_sit = false;
        wolf.tame.target = Some(tame::Target::Mob(6));
        let mut victim = Mob::new(6, Kind::Breeze, (1.5, 1.0, 0.5));
        victim.no_ai = true;
        victim.vitals.health = 1.0;
        // Старый удар не должен передать «добычу» текущему убийце-волку.
        victim.looting = 3;
        let mut items = crate::items::Items::new();
        items.add_mob(wolf);
        items.add_mob(victim);
        let mut id = 100;
        step_all(&mut items, &Floor, &[], Env { difficulty: 2, sky: 15, sun_up: false, clock: 1000, tick: 1000 },
            &mut Rng::new(2), &mut || { id += 1; id });
        let dead = items.mobs().iter().find(|mob| mob.entity_id == 6).unwrap();
        assert!(dead.is_dead());
        assert_eq!(dead.looting, own_looting);
        if own_looting > 0 { assert!(dead.vitals.fire >= 160); }
        assert_eq!(dead.kill_credit, tamed.then_some([7; 16]));
        assert_eq!(!items.orbs().is_empty(), tamed);
        let rods = items.items().iter().filter(|drop| blocks::item_name(drop.stack.item) == Some("breeze_rod"))
            .map(|drop| drop.stack.count).sum::<i32>();
        assert_eq!(((1 + own_looting)..=(2 * (1 + own_looting))).contains(&rods), tamed);
        if !tamed { assert_eq!(rods, 0); }
    }
}
