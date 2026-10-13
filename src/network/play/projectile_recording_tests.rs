// Записи официального 26.1.2; только сетевые наблюдения, без серверного кода.
use super::*;
use crate::thrown::{Flying, Kind};

#[derive(serde::Deserialize)]
struct Observation {
    tick: u32,
    velocity: [f64; 3],
    at: Option<[f64; 3]>,
    rotation: Option<[f32; 2]>,
}

#[derive(serde::Deserialize)]
struct Recording {
    name: String,
    kind: String,
    start: [f64; 3],
    velocity: [f64; 3],
    rotation: [f32; 2],
    moving: bool,
    spawn_rotation: Option<[i8; 2]>,
    observations: Vec<Observation>,
}

fn recordings() -> Vec<Recording> {
    serde_json::from_str(include_str!("../../../tools/blackbox/fixtures/thrown-26.1.2.json")).unwrap()
}

fn tuple(v: [f64; 3]) -> (f64, f64, f64) { (v[0], v[1], v[2]) }

#[test]
fn all_recorded_launch_angles_are_compatible_before_motion_inheritance() {
    let records = recordings();
    assert_eq!(records.len(), 51);
    let mut checked = 0;
    for r in records {
        if let Some(rotation) = r.spawn_rotation {
            checked += 1;
            // Packed velocity теряет знак почти нулевой компоненты; moving
            // не содержит пакетов движения игрока, известен лишь угол взгляда
            // с разбросом (audit-thrown.py). Это проверка границ, не точного RNG.
            let error = if r.moving { 1.5 } else { 0.01 };
            for (degrees, expected) in r.rotation.into_iter().zip(rotation) {
                assert!([-error, 0.0, error].into_iter().any(|d| cart_angle(degrees + d) as i8 == expected), "{}: {degrees} -> {expected}", r.name);
            }
        }
    }
    assert_eq!(checked, 28, "остальные 23 записи не содержат Spawn rotation");
}

#[test]
fn every_recorded_flight_keeps_ten_tick_physics_and_rotation_through_apex() {
    let shared = shared();
    let mut world = shared.world_in(Dimension::Overworld).lock().unwrap();
    for x in -6..=6 {
        for z in -6..=6 { world.set_block_silently(x * 16, -20, z * 16, blocks::state_by_name("stone").unwrap()); }
    }
    let mut checked = 0;
    for r in recordings() {
        let kind = match r.kind.as_str() { "egg" => Kind::Egg, "snowball" => Kind::Snowball, _ => Kind::EnderPearl };
        let mut flying = Flying::potion(400, Dimension::Overworld, kind, Stack::new(blocks::item_named(&r.kind).unwrap(), 1), tuple(r.start), tuple(r.velocity), 900);
        flying.rot = (r.rotation[0], r.rotation[1]);
        flying.start_rot = flying.rot;
        shared.thrown.lock().unwrap().flying = vec![flying];
        let mut age = 0;
        for observation in r.observations {
            assert_eq!(observation.tick % 10, 0, "{}: cadence", r.name);
            while age < observation.tick {
                crate::thrown::step(&shared, Dimension::Overworld, &mut world);
                age += 1;
            }
            let shots = shared.thrown.lock().unwrap();
            let f = &shots.flying[0];
            for (actual, expected) in [f.vx, f.vy, f.vz].into_iter().zip(observation.velocity) {
                assert!((actual - expected).abs() < 0.0007, "{} +{age}: velocity {actual}/{expected}", r.name);
            }
            if let Some(at) = observation.at {
                for (actual, expected) in [f.x, f.y, f.z].into_iter().zip(at) {
                    assert!((actual - expected).abs() < 0.007, "{} +{age}: position {actual}/{expected}", r.name);
                }
            }
            if let Some(rotation) = observation.rotation {
                for (actual, expected) in [f.rot.0, f.rot.1].into_iter().zip(rotation) {
                    let tolerance = if r.moving { 0.12 } else { 0.005 };
                    assert!((actual - expected).abs() < tolerance, "{} +{age}: rotation {actual}/{expected}", r.name);
                }
            }
            checked += 1;
        }
    }
    assert!(checked > 80);
}

#[tokio::test]
async fn moving_launch_keeps_pre_inheritance_rotation_even_if_tracker_is_late() {
    for item in ["egg", "snowball", "ender_pearl"] {
        let shared = shared();
        let mut p = player(item);
        p.pitch = 66.0;
        p.net.last_delta = (0.0, 0.3332, 0.29);
        p.on_ground = false;
        thrown::throw_potion(&shared, &mut p, 0);
        let mut f = shared.thrown.lock().unwrap().flying[0].clone();
        let shot = (f.vx, f.vy - 0.3332, f.vz - 0.29);
        assert_eq!(f.start_rot, crate::thrown::angles(shot));
        assert!((f.rot.1 - crate::thrown::angles(f.start.1).1).abs() > 10.0);
        f.age = 1;
        f.rot.1 += 3.0;
        let mut out = Outgoing::capture();
        thrown::send_launch(&mut out, &shared, &mut p, &f).await.unwrap();
        let ShownRotation { yaw, pitch } = spawn_rotation(&packets(&out).await[0]);
        assert_eq!((yaw, pitch), (cart_angle(f.start_rot.0), cart_angle(f.start_rot.1)));
    }
}

struct ShownRotation { yaw: u8, pitch: u8 }

fn spawn_rotation(packet: &PacketInfo) -> ShownRotation {
    let mut c = Cursor::new(packet.payload());
    c.varint().unwrap();
    c.bytes(16).unwrap();
    c.varint().unwrap();
    c.bytes(24).unwrap();
    // Эти броски имеют scale 1 или 2, без продолжения packed velocity.
    c.bytes(6).unwrap();
    let pitch = c.u8().unwrap();
    let yaw = c.u8().unwrap();
    ShownRotation { yaw, pitch }
}

#[test]
fn falling_owner_is_ignored_until_the_collision_region_is_clear() {
    for (pitch, vy) in [(0.0, -0.36037355795641823), (30.0, -1.1181102362204725), (-45.0, 0.6825367759262648)] {
        let shared = shared();
        let mut p = player("egg");
        p.game_mode = GAME_MODE_SURVIVAL;
        // Публикация игрока уже ниже точки броска; прежде тесный overlap
        // снимал in_owner, хотя расширенный ящик попадания ещё пересекался.
        (p.x, p.y, p.z) = (3.5, 6.5, 2.5);
        p.pitch = pitch;
        shared.players.lock().unwrap().join(p.member(), p.entity_id).unwrap();
        let f = Flying::potion(400, p.dimension, Kind::Egg, Stack::new(blocks::item_named("egg").unwrap(), 1), (3.5, 8.37, 2.5), (0.0, vy, 1.3), p.entity_id);
        shared.thrown.lock().unwrap().flying.push(f);
        let mut world = shared.world_in(Dimension::Overworld).lock().unwrap();
        world.set_block_silently(0, -20, 0, blocks::state_by_name("stone").unwrap());
        crate::thrown::step(&shared, p.dimension, &mut world);
        assert_eq!(shared.thrown.lock().unwrap().flying.len(), 1, "pitch={pitch}");
        assert!(shared.players.lock().unwrap().take_orders(&p.uuid).is_empty());
    }
}

#[test]
fn pearl_position_flags_preserve_motion_and_reset_fall_distance() {
    let mut p = player("ender_pearl");
    p.vit.fall = 12.0;
    p.net.last_delta = (0.1, -0.3, 0.2);
    let body = position_body(&mut p, Sync::Pearl);
    assert_eq!(i32::from_be_bytes(body[body.len()-4..].try_into().unwrap()), 504);
    assert_eq!(p.vit.fall, 0.0);
    assert_eq!(p.net.last_delta, (0.1, -0.3, 0.2));
    assert!(p.awaiting_teleport);
}

#[test]
fn pearl_sound_is_journaled_before_removal_at_impact() {
    let shared = shared();
    let mut p = player("ender_pearl");
    p.game_mode = GAME_MODE_SURVIVAL;
    shared.players.lock().unwrap().join(p.member(), p.entity_id).unwrap();
    shared.spawn_mobs.store(false, std::sync::atomic::Ordering::Relaxed);
    let mut world = shared.world_in(Dimension::Overworld).lock().unwrap();
    world.set_block_silently(0, 63, 0, blocks::state_by_name("stone").unwrap());
    let start = world.watch_events(p.reader);
    shared.thrown.lock().unwrap().flying.push(Flying::potion(400, p.dimension, Kind::EnderPearl,
        Stack::new(blocks::item_named("ender_pearl").unwrap(), 1), (0.5, 64.1, 0.5), (0.0, -0.5, 0.0), p.entity_id));
    crate::thrown::step(&shared, p.dimension, &mut world);
    let (events, _) = world.events_since(start, p.reader);
    let sound = events.iter().position(|e| matches!(e, WorldEvent::Sound(s) if s.name == "minecraft:entity.player.teleport")).unwrap();
    let removal = events.iter().position(|e| matches!(e, WorldEvent::ThrownSnapshot { early: false, .. })).unwrap();
    assert!(sound < removal);
    assert!(matches!(shared.players.lock().unwrap().take_orders(&p.uuid).as_slice(), [Order::PearlTeleport { y, .. }] if (*y - 64.1).abs() < 1e-9));
}

#[test]
fn hatching_keeps_probability_and_announces_babies_before_egg_removal() {
    let mut counts = [0; 5];
    for a in 0..8 { for b in 0..32 { counts[crate::thrown::egg_hatch(a,b)] += 1; } }
    assert_eq!((counts[0], counts[1], counts[4]), (224, 31, 1));
    let shared = shared();
    let mut world = shared.world_in(Dimension::Overworld).lock().unwrap();
    world.set_block_silently(0, 63, 0, blocks::state_by_name("stone").unwrap());
    for attempt in 0..256 {
        let start = world.watch_events(1);
        shared.items_in(Dimension::Overworld).lock().unwrap().mobs_mut().iter_mut().for_each(|m| { m.x = 100.0; });
        let mut f = Flying::potion(400 + attempt, Dimension::Overworld, Kind::Egg, Stack::new(blocks::item_named("egg").unwrap(), 1), (0.5, 64.1, 0.5), (0.01, -0.5, 0.0), 900);
        f.rot.0 = 134.488;
        shared.thrown.lock().unwrap().flying.push(f);
        crate::thrown::step(&shared, Dimension::Overworld, &mut world);
        let (events, _) = world.events_since(start, 1);
        if let Some(index) = events.iter().position(|e| matches!(e, WorldEvent::MobSpawn(_))) {
            let WorldEvent::MobSpawn(chick) = &events[index] else { unreachable!() };
            assert_eq!(chick.age, crate::mob::BABY_AGE);
            assert_eq!((chick.yaw, chick.pitch), (134.488, 0.0));
            assert!((chick.y - 64.0).abs() < 1e-9);
            let removal = events.iter().position(|e| matches!(e, WorldEvent::ThrownSnapshot { early: false, .. })).unwrap();
            assert!(index < removal);
            return;
        }
    }
    panic!("детерминированный поток RNG не дал ни одного цыплёнка");
}

#[tokio::test]
async fn pearl_damage_sends_health_before_next_tick_metadata_and_velocity() {
    let shared = shared();
    shared.difficulty.store(crate::survival::NORMAL, std::sync::atomic::Ordering::Relaxed);
    let mut p = player("ender_pearl");
    p.game_mode = GAME_MODE_SURVIVAL;
    p.on_ground = true;
    let mut out = Outgoing::capture();
    vitals::pearl_hit(&mut out, &shared, &mut p).await.unwrap();
    vitals::tick(&mut out, &shared, &mut p).await.unwrap();
    let first = packets(&out).await;
    assert_eq!(p.vit.health, 15.0);
    assert!(first.iter().any(|p| p.id == 0x19));
    assert!(first.iter().any(|p| p.id == 0x68));
    assert!(!first.iter().any(|p| p.id == SET_ENTITY_DATA || p.id == 0x65));
    vitals::tick(&mut out, &shared, &mut p).await.unwrap();
    let second = packets(&out).await;
    assert!(second[first.len()..].iter().any(|p| p.id == SET_ENTITY_DATA));
    assert!(second[first.len()..].iter().any(|p| p.id == 0x65));
}

#[tokio::test]
async fn peaceful_healing_runs_before_pearl_damage_and_entity_tick_is_not_repeated() {
    let shared = shared();
    shared.difficulty.store(crate::survival::PEACEFUL, std::sync::atomic::Ordering::Relaxed);
    let mut p = player("ender_pearl");
    p.game_mode = GAME_MODE_SURVIVAL;
    p.vit.age = 19;
    let mut out = Outgoing::capture();
    vitals::before_pearl_teleport(&mut out, &shared, &mut p).await.unwrap();
    vitals::pearl_hit(&mut out, &shared, &mut p).await.unwrap();
    assert_eq!(p.vit.health, 15.0);
    vitals::tick(&mut out, &shared, &mut p).await.unwrap();
    assert_eq!(p.vit.age, 20);
    assert_eq!(p.vit.health, 15.0, "мирное лечение уже прошло до попадания");
    assert_eq!(p.vit.invulnerable, 19);
    vitals::tick(&mut out, &shared, &mut p).await.unwrap();
    assert_eq!(p.vit.age, 21);
}
