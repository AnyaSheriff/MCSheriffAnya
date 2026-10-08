// Утопленник и кадавр (чёрный ящик: drowned-attack, drowned-trident,
// zombie-to-drowned, husk-hunger).

use super::*;

#[test]
fn a_husk_strike_carries_hunger_and_does_not_burn_in_the_sun() {
    let view = Flat::new();
    let players = [player_at(1, 2.5, 0.5)];
    let mut husk = Mob::new(9, Kind::Husk, (0.5, 1.0, 0.5));
    let hits = hunt(&mut husk, &view, &ctx_for(&players, 2, 15, false), 100);
    let melee = hits[0].1.melee.expect("удар");

    assert!(melee.hunger && !melee.wither);
    assert_eq!(melee.damage, 3.0);
    assert_eq!((hostile::hunger_ticks(1), hostile::hunger_ticks(2), hostile::hunger_ticks(3)), (0, 140, 280));

    let mut zombie = Mob::new(10, Kind::Zombie, (0.5, 1.0, 0.5));

    assert!(!hunt(&mut zombie, &view, &ctx_for(&players, 2, 15, false), 100)[0].1.melee.expect("удар").hunger);

    // Солнце: утопленник горит, кадавр нет.
    let sunny = Sunny(Flat::new());
    let day = ctx_for(&[], 2, 15, true);
    let mut rng = Rng::new(5);
    let mut husk = Mob::new(1, Kind::Husk, (0.5, 1.0, 0.5));
    let mut drowned = Mob::new(2, Kind::Drowned, (0.5, 1.0, 0.5));

    for _ in 0..200 {
        husk.tick(&sunny, &day, &mut rng);
        drowned.tick(&sunny, &day, &mut rng);
    }

    assert_eq!(husk.vitals.health, 20.0, "кадавр не горит");
    assert!(drowned.vitals.health < 20.0, "утопленник горит вне воды");
}

#[test]
fn a_drowned_attacks_only_at_night_or_when_the_player_is_in_water() {
    let view = Flat::new();
    let players = [player_at(1, 6.5, 0.5)];
    let mut rng = Rng::new(3);
    let mut day = Mob::new(9, Kind::Drowned, (0.5, 1.0, 0.5));

    for _ in 0..100 {
        day.tick(&view, &ctx_for(&players, 2, 15, false), &mut rng);
    }

    assert_eq!(day.h.target, None, "днём на суше не трогает");

    let mut night = Mob::new(10, Kind::Drowned, (0.5, 1.0, 0.5));

    for _ in 0..100 {
        night.tick(&view, &ctx_for(&players, 2, 4, false), &mut rng);
    }

    assert_eq!(night.h.target, Some(1), "ночью нападает");

    let mut wet = Flat::new();

    wet.water.push((6, 1, 0));
    wet.water.push((6, 2, 0));

    let mut swimmer = Mob::new(11, Kind::Drowned, (0.5, 1.0, 0.5));

    for _ in 0..100 {
        swimmer.tick(&wet, &ctx_for(&players, 2, 15, false), &mut rng);
    }

    assert_eq!(swimmer.h.target, Some(1), "днём нападает на игрока в воде");
}

#[test]
fn a_drowned_with_a_trident_throws_it_forty_ticks_after_the_target_and_then_every_forty() {
    let view = Flat::new();
    let players = [player_at(1, 5.5, 0.5)];
    let mut drowned = Mob::new(9, Kind::Drowned, (0.5, 1.0, 0.5));

    drowned.h.hand = hostile::HAND_TRIDENT;

    let hits = hunt(&mut drowned, &view, &ctx_for(&players, 2, 4, false), 200);

    assert!(hits.len() >= 3, "бросков: {}", hits.len());
    assert!(hits.iter().all(|(_, plan)| plan.shot.is_some_and(|shot| shot.trident && shot.speed == hostile::TRIDENT_SPEED) && plan.melee.is_none()));

    for pair in hits.windows(2) {
        assert_eq!(pair[1].0 - pair[0].0, 40, "раз в 40 тактов");
    }

    assert!(drowned.h.using, "трезубец занесён");
}

#[test]
fn a_zombie_in_water_shakes_after_thirty_seconds_and_becomes_a_drowned() {
    let mut wet = Flat::new();

    for x in -12..12 {
        for z in -12..12 {
            for y in 1..=3 {
                wet.water.push((x, y, z));
            }
        }
    }

    let mut zombie = Mob::new(9, Kind::Zombie, (0.5, 1.0, 0.5));
    let mut rng = Rng::new(3);
    let mut shaken = None;
    let mut done = None;

    for tick in 1..1200 {
        zombie.tick(&wet, &ctx(), &mut rng);

        if shaken.is_none() && zombie.h.converting > 0 {
            shaken = Some(tick);
        }

        if let Some(kind) = zombie.h.converted.take() {
            done = Some((tick, kind));
            break;
        }
    }

    assert_eq!(shaken, Some(602), "трясётся на 602-м такте (чёрный ящик, zombie-to-drowned)");
    assert_eq!(done.map(|(tick, kind)| (tick - shaken.unwrap_or(0), kind)), Some((302, Kind::Drowned)));

    // Кадавр в воде становится зомби.
    let mut husk = Mob::new(10, Kind::Husk, (0.5, 1.0, 0.5));
    let mut result = None;

    for _ in 0..1200 {
        husk.tick(&wet, &ctx(), &mut rng);

        if let Some(kind) = husk.h.converted.take() {
            result = Some(kind);
            break;
        }
    }

    assert_eq!(result, Some(Kind::Zombie));

    // Вне воды счёта нет.
    let mut dry = Mob::new(11, Kind::Zombie, (30.5, 1.0, 30.5));

    run(&mut dry, &wet, 700);
    assert_eq!((dry.h.converting, dry.h.converted), (0, None));
}

#[test]
fn a_converted_mob_keeps_place_equipment_and_flags() {
    let mut old = Mob::new(1, Kind::Zombie, (3.5, 2.0, 4.5));

    old.yaw = 33.0;
    old.silent = true;
    old.h.helmet = true;
    old.vx = 0.01;

    let new = convert(&old, Kind::Drowned, 2);

    assert_eq!((new.kind, new.x, new.y, new.z, new.yaw, new.silent, new.h.helmet, new.vx), (Kind::Drowned, 3.5, 2.0, 4.5, 33.0, true, true, 0.01));
    assert_eq!(new.vitals.health, 20.0);
}

#[test]
fn a_drowned_drops_what_it_holds_and_may_drop_copper() {
    let mut drowned = Mob::new(1, Kind::Drowned, (0.5, 1.0, 0.5));
    let mut rng = Rng::new(9);

    drowned.h.offhand_shell = true;
    drowned.h.hand = hostile::HAND_TRIDENT;
    drowned.h.hand_drop = 1.0;
    drowned.by_player = true;

    let shell = blocks::item_named("nautilus_shell").expect("ракушка");
    let trident = blocks::item_named("trident").expect("трезубец");
    let copper = blocks::item_named("copper_ingot").expect("медь");
    let mut copper_seen = false;

    for _ in 0..200 {
        let drops = loot(&drowned, &mut rng);

        assert!(drops.iter().any(|stack| stack.item == shell), "ракушка выпадает всегда");
        assert!(drops.iter().any(|stack| stack.item == trident), "шанс 1,0");
        copper_seen |= drops.iter().any(|stack| stack.item == copper);
    }

    assert!(copper_seen, "медный слиток выпал хоть раз");

    drowned.h.hand_drop = 0.0;
    drowned.by_player = false;
    assert!(!loot(&drowned, &mut rng).iter().any(|stack| stack.item == trident || stack.item == copper));
}

#[test]
fn drowned_and_husk_survive_the_region_file() {
    let mut drowned = Mob::new(1, Kind::Drowned, (1.5, 2.0, 3.5));

    drowned.h.hand = hostile::HAND_TRIDENT;
    drowned.h.offhand_shell = true;

    let mut bytes = Vec::new();

    drowned.write(&mut bytes);

    let read = Mob::read(&mut bytes.as_slice()).expect("читается");

    assert_eq!((read.kind, read.h.hand, read.h.offhand_shell), (Kind::Drowned, hostile::HAND_TRIDENT, true));

    let mut husk = Vec::new();

    Mob::new(2, Kind::Husk, (0.5, 1.0, 0.5)).write(&mut husk);
    assert_eq!(Mob::read(&mut husk.as_slice()).map(|mob| mob.kind), Some(Kind::Husk));
}

#[test]
fn deserts_spawn_husks_and_rivers_and_oceans_drowned() {
    let desert = spawn::overworld_kinds("desert", false);

    assert!(desert.contains(&(Kind::Husk, 80)) && desert.contains(&(Kind::Zombie, 19)));
    assert_eq!(spawn::overworld_kinds("river", true), vec![(Kind::Drowned, 100)]);
    assert_eq!(spawn::overworld_kinds("deep_ocean", true), vec![(Kind::Drowned, 5)]);
    assert!(spawn::overworld_kinds("plains", true).is_empty());
    assert_eq!(spawn::overworld_kinds("plains", false), spawn::HOSTILE.to_vec());
}

#[test]
fn a_drowned_swims_to_a_target_in_water_and_strikes() {
    let mut pool = Flat::new();

    for x in 0..5 {
        for y in 1..=2 {
            for z in 0..5 {
                pool.water.push((x, y, z));
            }
        }
    }

    let players = [player_at(1, 4.5, 4.5)];
    let mut drowned = Mob::new(9, Kind::Drowned, (0.5, 1.0, 0.5));
    let hits = hunt(&mut drowned, &pool, &ctx_for(&players, 2, 4, false), 300);

    assert!(!hits.is_empty(), "доплыл и ударил");
    assert!(drowned.h.wet);
}
