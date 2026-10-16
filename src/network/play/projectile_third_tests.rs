// Третий проход: пакеты официального 26.1.2, без запуска сервера.
use super::*;
use crate::thrown::{Flying, Kind};

#[test]
fn simple_throw_spread_is_bounded_and_triangular() {
    let mut seed = 981726u64;
    let mut roll = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        (seed >> 40) as f32 / 16777216.0
    };
    let (mut sum, mut squares, mut fourth) = (0.0, 0.0, 0.0);
    for _ in 0..100_000 {
        let value = projectile::simple_noise(&mut roll);
        assert!(value.abs() <= 0.0172275);
        sum += value;
        squares += value * value;
        fourth += value.powi(4);
    }
    assert!((sum / 100_000.0).abs() < 0.0001);
    assert!((squares / 100_000.0 - 0.0172275f64.powi(2) / 6.0).abs() < 0.000001);
    assert!((fourth * 100_000.0 / squares.powi(2) - 2.4).abs() < 0.05);
}

#[test]
fn north_heading_wrap_uses_the_sign_of_noisy_x() {
    // throw-snowball-walk.md:8 и throw-ender_pearl-flat.md:7: x<0 → −128.
    // vanilla-throw-snowball-flat.json: x>0 → 127, значит фиксировать −128 нельзя.
    for (vx, expected) in [(-0.006470121, -128), (0.001342855, 127), (-1e-5, -128), (1e-5, 127)] {
        assert_eq!(cart_angle(crate::thrown::angles((vx, 0.0, -1.5)).0) as i8, expected);
    }
    assert_eq!(cart_angle(crate::thrown::angles((0.0, 0.0, -1.5)).0) as i8, -128);
}

#[derive(serde::Deserialize)]
struct Observation { tick: u32, velocity: [f64; 3], at: Option<[f64; 3]> }

#[derive(serde::Deserialize)]
struct Recording {
    name: String,
    kind: String,
    start: [f64; 3],
    velocity: [f64; 3],
    observations: Vec<Observation>,
}

#[test]
fn third_pass_recorded_velocities_replay_through_apex_and_impact_sync() {
    let records: Vec<Recording> = serde_json::from_str(include_str!("../../../tools/blackbox/fixtures/throw3-26.1.2.json")).unwrap();
    let shared = shared();
    let mut world = shared.world_in(Dimension::Overworld).lock().unwrap();
    for x in -5..=5 { for z in -5..=5 {
        world.set_block_silently(x * 16, -20, z * 16, blocks::state_by_name("stone").unwrap());
    } }
    let mut checked = 0;
    for r in records {
        let kind = match r.kind.as_str() { "egg" => Kind::Egg, "snowball" => Kind::Snowball, _ => Kind::EnderPearl };
        let tuple = |v: [f64; 3]| (v[0], v[1], v[2]);
        shared.thrown.lock().unwrap().flying = vec![Flying::potion(400, Dimension::Overworld, kind,
            Stack::new(blocks::item_named(&r.kind).unwrap(), 1), tuple(r.start), tuple(r.velocity), 900)];
        let mut age = 0;
        for observation in r.observations {
            assert_eq!(observation.tick % 10, 0, "{}", r.name);
            while age < observation.tick {
                crate::thrown::step(&shared, Dimension::Overworld, &mut world);
                age += 1;
            }
            let state = shared.thrown.lock().unwrap();
            let f = &state.flying[0];
            for (actual, expected) in [f.vx, f.vy, f.vz].into_iter().zip(observation.velocity) {
                assert!((actual - expected).abs() < 0.0007, "{} +{age}: {actual}/{expected}", r.name);
            }
            if let Some(at) = observation.at {
                for (actual, expected) in [f.x, f.y, f.z].into_iter().zip(at) {
                    assert!((actual - expected).abs() < 0.008, "{} +{age}: {actual}/{expected}", r.name);
                }
            }
            checked += 1;
        }
    }
    assert!(checked >= 30);
}

#[tokio::test]
async fn recorded_sync_at_impact_is_sent_once_before_status_and_removal() {
    for (z, impact_age) in [(-2.5, 9), (-3.5, 11), (-4.5, 12)] {
        let shared = shared();
        let mut p = player("snowball");
        {
            let mut world = shared.world_in(p.dimension).lock().unwrap();
            world.set_block_silently(3, 4, 6, blocks::state_by_name("stone").unwrap());
            world.set_block_silently(3, -20, -4, blocks::state_by_name("stone").unwrap());
        }
        let mut f = Flying::snowball(400, p.dimension, (3.5, 4.0, z), (0.0, 0.0, 1.0), 0);
        f.no_gravity = true;
        shared.thrown.lock().unwrap().flying.push(f.clone());
        let mut out = Outgoing::capture();
        thrown::send_launch(&mut out, &shared, &mut p, &f).await.unwrap();
        let mut impact_packets = Vec::new();
        for age in 1..=impact_age {
            let before = packets(&out).await.len();
            crate::thrown::step(&shared, p.dimension, &mut shared.world_in(p.dimension).lock().unwrap());
            let state = shared.thrown.lock().unwrap().snapshot(p.dimension);
            thrown::send_thrown_ticks(&mut out, &shared, &mut p, true, Some(&state)).await.unwrap();
            thrown::send_thrown_ticks(&mut out, &shared, &mut p, false, Some(&state)).await.unwrap();
            if age == impact_age {
                assert!(state.flying.is_empty());
                assert_eq!(state.landed.len(), 1);
                impact_packets = packets(&out).await[before..].iter().map(|p| p.id).collect();
            } else { assert_eq!(state.flying.len(), 1); }
        }
        let mut expected = if impact_age == 11 { vec![0x65, 0x23] } else { Vec::new() };
        expected.extend([0x22, REMOVE_ENTITIES]);
        assert_eq!(impact_packets, expected, "z={z}");
        assert_eq!(packets(&out).await.iter().filter(|p| p.id == 0x65).count(), usize::from(impact_age >= 11));
    }
}

#[test]
fn wall_hatches_keep_recorded_placement_head_bounds_and_order() {
    let shared = shared();
    let mut world = shared.world_in(Dimension::Overworld).lock().unwrap();
    let stone = blocks::state_by_name("stone").unwrap();
    world.set_block_silently(3, 4, 6, stone);
    world.set_block_silently(3, 3, 6, stone);
    let mut heads = std::collections::HashSet::new();
    let mut babies = 0;
    for attempt in 0..256 {
        let start = world.watch_events(1);
        for mob in shared.items_in(Dimension::Overworld).lock().unwrap().mobs_mut() { mob.x = 100.0; }
        let mut f = Flying::potion(400 + attempt, Dimension::Overworld, Kind::Egg,
            Stack::new(blocks::item_named("egg").unwrap(), 1), (3.5, 4.0, 5.5), (0.0, 0.0, 1.0), 900);
        f.no_gravity = true;
        shared.thrown.lock().unwrap().flying.push(f);
        crate::thrown::step(&shared, Dimension::Overworld, &mut world);
        let (events, _) = world.events_since(start, 1);
        let removal = events.iter().position(|e| matches!(e, WorldEvent::ThrownSnapshot { early: false, .. })).unwrap();
        for (index, event) in events.iter().enumerate() {
            if let WorldEvent::MobSpawn(chick) = event {
                assert_eq!((chick.x, chick.y, chick.z), (3.5, 3.8, 5.85));
                assert_eq!((chick.age, chick.yaw, chick.pitch), (crate::mob::BABY_AGE, 0.0, 0.0));
                assert!(cart_angle(chick.head_yaw) <= 4);
                heads.insert(cart_angle(chick.head_yaw));
                babies += 1;
                assert!(index < removal);
            }
        }
    }
    assert!(babies > 10);
    assert_eq!(heads.len(), 5);
}

#[test]
fn owner_can_be_hit_after_leaving_then_returning_to_collision_region() {
    let shared = shared();
    let mut p = player("egg");
    p.game_mode = GAME_MODE_SURVIVAL;
    (p.x, p.y, p.z) = (3.5, 0.0, 2.5);
    shared.players.lock().unwrap().join(p.member(), p.entity_id).unwrap();
    let mut world = shared.world_in(p.dimension).lock().unwrap();
    world.set_block_silently(3, -1, 2, blocks::state_by_name("stone").unwrap());
    shared.thrown.lock().unwrap().flying.push(Flying::potion(400, p.dimension, Kind::Egg,
        Stack::new(blocks::item_named("egg").unwrap(), 1), (3.5, 1.52, 2.5), (0.0, 1.5, 0.0), p.entity_id));
    let mut cleared = false;
    for _ in 0..100 {
        crate::thrown::step(&shared, p.dimension, &mut world);
        let state = shared.thrown.lock().unwrap();
        if let Some(f) = state.flying.first() {
            cleared |= !f.in_owner;
        } else {
            assert!(cleared);
            assert!(state.landed[0].y > 1.8, "возврат к хозяину раньше пола");
            assert!(!state.landed[0].in_owner);
            return;
        }
    }
    panic!("яйцо не вернулось");
}

#[test]
fn repeated_probe_cleanup_kills_only_chicks_in_positioned_radius() {
    let shared = shared();
    let dimension = Dimension::Overworld;
    for (id, kind, at) in [
        (401, crate::mob::Kind::Chicken, (100.25, 4.0, 100.25)),
        (402, crate::mob::Kind::Chicken, (103.0, 4.0, 100.25)),
        (403, crate::mob::Kind::Cow, (100.25, 4.0, 100.25)),
    ] {
        shared.items_in(dimension).lock().unwrap().add_mob(crate::mob::Mob::new(id, kind, at));
    }
    crate::commands::run("execute positioned 100.25 4 100.25 run kill @e[type=chicken,distance=..1]", &crate::commands::Source::Console, &shared);
    let items = shared.items_in(dimension).lock().unwrap();
    assert!(items.mobs().iter().find(|m| m.entity_id == 401).unwrap().is_dead());
    for id in [402, 403] { assert!(!items.mobs().iter().find(|m| m.entity_id == id).unwrap().is_dead()); }
}

#[test]
fn endermite_probe_requires_spawn_rule_and_keeps_pre_teleport_owner_position() {
    // throw3: 0/128 при spawn_mobs=false. Размещение при true пока
    // правило вики, не измерение; throw4-pearl-endermite-lifetime проверит его.
    for allowed in [false, true] {
        let shared = shared();
        shared.spawn_mobs.store(allowed, std::sync::atomic::Ordering::Relaxed);
        let mut p = player("ender_pearl");
        p.game_mode = GAME_MODE_SURVIVAL;
        (p.x, p.y, p.z) = (7.5, 66.0, 7.5);
        shared.players.lock().unwrap().join(p.member(), p.entity_id).unwrap();
        let mut world = shared.world_in(p.dimension).lock().unwrap();
        world.set_block_silently(0, 63, 0, blocks::state_by_name("stone").unwrap());
        for i in 0..128 {
            shared.thrown.lock().unwrap().flying.push(Flying::potion(500+i, p.dimension, Kind::EnderPearl,
                Stack::new(blocks::item_named("ender_pearl").unwrap(), 1), (0.5, 64.1, 0.5), (0.0, -0.5, 0.0), p.entity_id));
            crate::thrown::step(&shared, p.dimension, &mut world);
        }
        let items = shared.items_in(p.dimension).lock().unwrap();
        if allowed { assert!(!items.mobs().is_empty()); }
        else { assert!(items.mobs().is_empty()); }
        for mob in items.mobs() {
            assert_eq!(mob.kind, crate::mob::Kind::Endermite);
            assert_eq!((mob.x, mob.y, mob.z), (p.x, p.y, p.z));
            assert_eq!(mob.newmobs.lifetime, 0);
            assert!(!mob.persistent_tag && mob.name.is_none());
        }
    }
}

#[tokio::test]
async fn pearl_impact_snapshot_precedes_teleport_damage_and_deferred_health_metadata() {
    let shared = shared();
    let mut p = player("ender_pearl");
    p.game_mode = GAME_MODE_SURVIVAL;
    shared.difficulty.store(crate::survival::NORMAL, std::sync::atomic::Ordering::Relaxed);
    shared.spawn_mobs.store(false, std::sync::atomic::Ordering::Relaxed);
    shared.players.lock().unwrap().join(p.member(), p.entity_id).unwrap();
    {
        let mut world = shared.world_in(p.dimension).lock().unwrap();
        world.set_block_silently(3, 4, 6, blocks::state_by_name("stone").unwrap());
        world.set_block_silently(3, -20, -4, blocks::state_by_name("stone").unwrap());
    }
    let mut shot = Flying::potion(400, p.dimension, Kind::EnderPearl,
        Stack::new(blocks::item_named("ender_pearl").unwrap(), 1), (3.5, 4.0, -3.5), (0.0, 0.0, 1.0), p.entity_id);
    shot.no_gravity = true;
    shared.thrown.lock().unwrap().flying.push(shot.clone());
    let mut out = Outgoing::capture();
    thrown::send_launch(&mut out, &shared, &mut p, &shot).await.unwrap();
    {
        let mut world = shared.world_in(p.dimension).lock().unwrap();
        for _ in 0..11 { crate::thrown::step(&shared, p.dimension, &mut world); }
    }
    let order = shared.players.lock().unwrap().take_orders(&p.uuid).pop().unwrap();
    let Order::PearlTeleport { x, y, z, shot, .. } = order else { panic!("нет переноса") };
    assert_eq!(shot.age, 11);
    assert_eq!((x, y, z), shot.prev.0);
    let before = packets(&out).await.len();
    thrown::update_flying(&mut out, &mut p, &shot).await.unwrap();
    vitals::before_pearl_teleport(&mut out, &shared, &mut p).await.unwrap();
    (p.x, p.y, p.z) = (x, y, z);
    p.net.last_delta = (0.1, -0.3, 0.2);
    send_player_position_as(&mut out, &mut p, Sync::Pearl).await.unwrap();
    vitals::pearl_hit(&mut out, &shared, &mut p).await.unwrap();
    let phase: Vec<_> = packets(&out).await[before..].iter().map(|p| p.id).collect();
    assert_eq!(phase, vec![0x65, 0x23, PLAYER_POSITION, 0x19]);
    assert_eq!(p.net.last_delta, (0.1, -0.3, 0.2));
    vitals::tick(&mut out, &shared, &mut p).await.unwrap();
    assert_eq!(p.vit.health, 15.0);
    assert!(!packets(&out).await[before..].iter().any(|p| p.id == SET_ENTITY_DATA));
    vitals::tick(&mut out, &shared, &mut p).await.unwrap();
    assert!(packets(&out).await[before..].iter().any(|p| p.id == SET_ENTITY_DATA));
}

#[test]
fn pearl_wake_waits_for_world_publication_but_command_teleport_does_not() {
    let shot = Flying::potion(400, Dimension::Overworld, Kind::EnderPearl,
        Stack::new(blocks::item_named("ender_pearl").unwrap(), 1), (3.5, 4.0, 5.5), (0.0, 0.0, 1.0), 900);
    let order = Order::PearlTeleport { x: 3.5, y: 4.0, z: 5.5, dimension: Dimension::Overworld, tick: 1, shot: Box::new(shot) };
    let mut pending = Vec::new();
    let immediate = projectile::orders_at_tick(vec![order, Order::Teleport { x: 1.0, y: 2.0, z: 3.0 }], &mut pending, None);
    assert!(matches!(immediate.as_slice(), [Order::Teleport { .. }]));
    assert_eq!(pending.len(), 1);
    assert!(projectile::orders_at_tick(Vec::new(), &mut pending, None).is_empty());
    assert!(projectile::orders_at_tick(Vec::new(), &mut pending, Some(0)).is_empty());
    assert_eq!(pending.len(), 1);
    let published = projectile::orders_at_tick(Vec::new(), &mut pending, Some(1));
    assert!(matches!(published.as_slice(), [Order::PearlTeleport { .. }]));
    assert!(pending.is_empty());
    assert!(projectile::orders_at_tick(Vec::new(), &mut pending, Some(1)).is_empty());
}
