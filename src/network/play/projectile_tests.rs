// Регрессии броска: обработчики входящих пакетов и исходящий буфер без сокетов.
use super::*;

fn shared() -> Shared {
    Shared::new(World::in_memory(), World::in_memory(), Default::default(), Default::default(), Default::default(),
        crate::ops::Ops::open(std::path::Path::new("tmp/throw2-ops.json")), crate::skins::Settings::default())
}

fn player(item: &str) -> PlayerState {
    let mut p = PlayerState::new([61; 16], "Throw2".into(), None, None, 0x7f, 61, (0.5, 64.0, 0.5));
    p.game_mode = GAME_MODE_CREATIVE;
    p.awaiting_teleport = false;
    p.yaw = 0.0;
    p.pitch = 0.0;
    p.inventory.set(p.inventory.selected_slot(), Some(Stack::new(blocks::item_named(item).unwrap(), 1)));
    p
}

fn packet(id: i32, payload: Vec<u8>) -> PacketInfo {
    let mut body = encode_varint(id);
    body.extend(payload);
    PacketInfo { id, length: body.len() as i32, body, teleport: None, punch: false, foreign_inventory: false, legacy_inventory: None }
}

fn movement(at: (f64, f64, f64), ground: bool) -> PacketInfo {
    let mut payload = Vec::new();
    for value in [at.0, at.1, at.2] { push_f64(&mut payload, value); }
    payload.push(u8::from(ground));
    packet(SET_PLAYER_POSITION, payload)
}

fn use_packet() -> PacketInfo {
    let mut payload = vec![0, 1];
    push_f32(&mut payload, 0.0);
    push_f32(&mut payload, 0.0);
    packet(USE_ITEM, payload)
}

async fn packets(out: &Outgoing<'_>) -> Vec<PacketInfo> {
    let mut bytes = out.queued();
    let mut result = Vec::new();
    while !bytes.is_empty() { result.push(read_packet(&mut bytes).await.unwrap()); }
    result
}

#[test]
fn movement_before_use_sets_throw_origin_and_motion_for_all_hand_throwables() {
    for item in ["snowball", "egg", "ender_pearl", "splash_potion", "lingering_potion", "experience_bottle", "wind_charge"] {
        let shared = shared();
        let mut p = player(item);
        p.net.teleported = false;
        handle_client_packet(&movement((0.75, 64.25, 0.25), false), &mut p, &shared);
        handle_client_packet(&use_packet(), &mut p, &shared);
        let shots = shared.thrown.lock().unwrap();
        let f = &shots.flying[0];
        assert_eq!((f.x, f.z), (0.75, 0.25), "{item}");
        assert!((f.y - (64.25 + if item == "wind_charge" { 1.62 } else { 1.52 })).abs() < 1e-12, "{item}");
        assert_eq!(p.net.last_delta, (0.25, 0.25, -0.25));
        assert!((f.vx - 0.25).abs() < 0.1, "{item}");
        assert!(f.vy > 0.15, "{item}");
    }
}

#[test]
fn rotation_without_motion_does_not_reuse_old_walking_velocity() {
    let shared = shared();
    let mut p = player("egg");
    p.net.last_delta = (0.3, 0.2, -0.4);
    let mut payload = Vec::new();
    push_f32(&mut payload, 0.0);
    push_f32(&mut payload, 0.0);
    payload.push(1);
    handle_client_packet(&packet(SET_PLAYER_ROTATION, payload), &mut p, &shared);
    assert_eq!(p.net.last_delta, (0.0, 0.0, 0.0));
}

#[test]
fn inheritance_keeps_horizontal_motion_and_only_airborne_vertical_motion() {
    assert_eq!(projectile::inherit((1.0, 2.0, 3.0), (0.25, -0.5, -0.25), true), (1.25, 2.0, 2.75));
    assert_eq!(projectile::inherit((1.0, 2.0, 3.0), (0.25, -0.5, -0.25), false), (1.25, 1.5, 2.75));
}

#[test]
fn gaussian_throw_spread_has_requested_scale_and_tails() {
    let mut seed = 123456789u64;
    let mut roll = || {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        ((seed >> 40) as f32) / 16777216.0
    };
    let (mut sum, mut squares, mut tails) = (0.0, 0.0, 0);
    for _ in 0..100_000 {
        let v = projectile::noise(&mut roll);
        sum += v;
        squares += v * v;
        tails += usize::from(v.abs() > 0.0225);
    }
    assert!((sum / 100_000.0).abs() < 0.0001);
    assert!(((squares / 100_000.0).sqrt() - 0.0075).abs() < 0.0001);
    assert!((200..350).contains(&tails));
    assert!(projectile::noise(&mut || 0.0).is_finite());
    assert!(projectile::noise(&mut || 1.0).is_finite());
}

#[tokio::test]
async fn gameplay_burst_keeps_fifo_and_stops_at_commands_or_next_tick() {
    let shared = shared();
    let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
    let mut inbound = Inbound { packets: receiver, reader: tokio::spawn(async {}), queued: None };
    for (id, arrived) in [(SET_PLAYER_POSITION, 3), (USE_ITEM, 3), (SERVERBOUND_CHAT_COMMAND, 3), (USE_ITEM, 4)] {
        sender.send(Ok((packet(id, Vec::new()), arrived, TickPermit::new(&shared.waiting_for_tick, arrived, is_gated(id))))).unwrap();
    }
    assert_eq!(inbound.take_gameplay(4).unwrap().0.id, SET_PLAYER_POSITION);
    assert_eq!(inbound.take_gameplay(4).unwrap().0.id, USE_ITEM);
    assert!(inbound.take_gameplay(4).is_none());
    assert_eq!(inbound.recv().await.unwrap().unwrap().0.id, SERVERBOUND_CHAT_COMMAND);
    assert!(inbound.take_gameplay(4).is_none());
    assert_eq!(inbound.take_gameplay(5).unwrap().0.id, USE_ITEM);
    assert!(is_gated(SET_PLAYER_ROTATION));
    assert!(is_gated(vitals::PLAYER_COMMAND));
}

#[tokio::test]
async fn delayed_tracker_uses_launch_snapshot_and_unclipped_ten_tick_sync_once() {
    for item in ["snowball", "egg", "ender_pearl", "splash_potion", "lingering_potion", "experience_bottle", "wind_charge"] {
        let shared = shared();
        let mut p = player(item);
        handle_client_packet(&use_packet(), &mut p, &shared);
        let launch = shared.thrown.lock().unwrap().flying[0].clone();
        let mut state = shared.thrown.lock().unwrap().snapshot(p.dimension);
        let f = &mut state.flying[0];
        f.age = 11;
        f.prev = ((0.5, 66.0, 15.0), (0.0, -0.3, 1.35), (0.0, -10.0));
        f.x = 0.5; f.y = 66.0; f.z = 16.0;
        // Живой снаряд уже исчез: историческое событие всё равно содержит полёт.
        shared.thrown.lock().unwrap().flying.clear();
        let mut out = Outgoing::capture();
        thrown::send_launch(&mut out, &shared, &mut p, &launch).await.unwrap();
        thrown::send_launch(&mut out, &shared, &mut p, &launch).await.unwrap();
        let before = packets(&out).await;
        assert_eq!(before.iter().filter(|p| p.id == ADD_ENTITY).count(), 1, "{item}");
        let spawn = before.iter().find(|p| p.id == ADD_ENTITY).unwrap();
        let mut c = Cursor::new(spawn.payload());
        assert_eq!(c.varint(), Some(launch.entity_id));
        for _ in 0..16 { c.u8().unwrap(); }
        c.varint().unwrap();
        let mut coordinate = || f64::from_be_bytes(c.bytes(8).unwrap().try_into().unwrap());
        assert_eq!((coordinate(), coordinate(), coordinate()), (launch.x, launch.y, launch.z));
        thrown::send_thrown_ticks(&mut out, &shared, &mut p, false, Some(&state)).await.unwrap();
        thrown::send_thrown_ticks(&mut out, &shared, &mut p, false, Some(&state)).await.unwrap();
        let after = packets(&out).await;
        assert_eq!(after[before.len()..].iter().map(|p| p.id).collect::<Vec<_>>(), vec![0x65, 0x23], "{item}");
        let mut c = Cursor::new(after.last().unwrap().payload());
        assert_eq!(c.varint(), Some(launch.entity_id));
        let values: Vec<_> = (0..6).map(|_| f64::from_be_bytes(c.bytes(8).unwrap().try_into().unwrap())).collect();
        assert_eq!(values, vec![0.5, 66.0, 15.0, 0.0, -0.3, 1.35]);
    }
}

#[test]
fn throwable_air_step_matches_ten_tick_recorded_velocity() {
    let shared = shared();
    let dimension = Dimension::Overworld;
    let mut world = shared.world_in(dimension).lock().unwrap();
    world.set_block_silently(0, 63, 0, blocks::state_by_name("stone").unwrap());
    // vanilla-throw-egg-down20.json: пакет появления и скорость на +10.
    let velocity = (0.005249343832021136, 1.3722761399011172, -0.6136849172923151);
    let mut f = crate::thrown::Flying::potion(400, dimension, crate::thrown::Kind::Egg,
        Stack::new(blocks::item_named("egg").unwrap(), 1), (0.5, 70.0, 12.0), velocity, 0);
    f.owner = 900;
    shared.thrown.lock().unwrap().flying.push(f);
    let mut expected = velocity;
    let mut at = (0.5, 70.0, 12.0);
    for _ in 0..10 {
        expected = (expected.0 * 0.99, (expected.1 - 0.03) * 0.99, expected.2 * 0.99);
        at = (at.0 + expected.0, at.1 + expected.1, at.2 + expected.2);
        crate::thrown::step(&shared, dimension, &mut world);
    }
    let shots = shared.thrown.lock().unwrap();
    let f = &shots.flying[0];
    assert!((f.vy - expected.1).abs() < 1e-12);
    for (actual, recorded) in [(f.vx, 0.005), (f.vy, 0.957), (f.vz, -0.555)] {
        assert!((actual - recorded).abs() < 0.0005);
    }
    assert!((f.y - at.1).abs() < 1e-12);
    assert!((f.z - at.2).abs() < 1e-12);
}

#[test]
fn bow_crossbow_and_trident_share_fresh_movement_at_release() {
    for item in ["bow", "crossbow", "trident"] {
        let shared = shared();
        let mut p = player(item);
        p.inventory.set(inventory::OFFHAND, Some(Stack::new(blocks::item_named("arrow").unwrap(), 8)));
        if item == "crossbow" { bow::crossbow_load(&mut p, 0); }
        handle_client_packet(&movement((0.75, 64.25, 0.25), false), &mut p, &shared);
        match item {
            "bow" => bow::release(&shared, &mut p, 0, 20),
            "trident" => bow::throw_trident(&shared, &mut p, 0, 20),
            _ => { assert!(bow::crossbow_fire(&shared, &mut p, 0)); }
        }
        let arrows = shared.arrows.lock().unwrap();
        let a = &arrows.list[0];
        assert_eq!(a.start.0, (0.75, 64.25 + 1.62 - 0.1, 0.25), "{item}");
        assert!((a.vx - 0.25).abs() < 0.1, "{item}");
        assert!((a.vy - 0.25).abs() < 0.1, "{item}");
    }
}

#[tokio::test]
async fn journal_preserves_every_projectile_tick_when_output_is_delayed() {
    let shared = shared();
    let mut p = player("egg");
    p.pitch = -45.0;
    let start = {
        let mut world = shared.world_in(p.dimension).lock().unwrap();
        world.set_block_silently(0, 63, 0, blocks::state_by_name("stone").unwrap());
        world.watch_events(p.reader)
    };
    assert!(thrown::throw_potion(&shared, &mut p, 0));
    for _ in 0..12 {
        crate::thrown::step(&shared, p.dimension, &mut shared.world_in(p.dimension).lock().unwrap());
    }
    let (events, _) = shared.world_in(p.dimension).lock().unwrap().events_since(start, p.reader);
    let mut out = Outgoing::capture();
    let mut ages = Vec::new();
    for event in events {
        match event {
            WorldEvent::ProjectileLaunch(f) => thrown::send_launch(&mut out, &shared, &mut p, &f).await.unwrap(),
            WorldEvent::ThrownSnapshot { early, state } => {
                let state = state.get().unwrap();
                if !early { ages.push(state.flying[0].age); }
                thrown::send_thrown_ticks(&mut out, &shared, &mut p, early, Some(state)).await.unwrap();
            }
            _ => {}
        }
    }
    assert_eq!(ages, (1..=12).collect::<Vec<_>>());
    let ids: Vec<_> = packets(&out).await.iter().map(|p| p.id).collect();
    assert_eq!(ids.iter().filter(|&&id| id == ADD_ENTITY).count(), 1);
    assert_eq!(ids.iter().filter(|&&id| id == 0x65).count(), 1);
    assert_eq!(ids.iter().filter(|&&id| id == ENTITY_POSITION).count(), 1);
    assert!(!ids.contains(&0x36));
    assert_eq!(ids.iter().filter(|&&id| id == 0x23).count(), 1);
}

#[test]
fn relative_motion_never_clamps_or_wraps_at_eight_blocks() {
    assert_eq!(projectile::relative_step((0, 0, 0), (32767, -32768, 0)), Some([32767, -32768, 0]));
    assert_eq!(projectile::relative_step((0, 0, 0), (32768, 0, 0)), None);
    assert_eq!(projectile::relative_step((0, 0, 0), (0, -32769, 0)), None);
    assert_eq!(projectile::relative_step((8192, 8192, 8192), (8192, 8192, 8193)), Some([0, 0, 1]));
}

#[tokio::test]
async fn long_arrow_sync_uses_the_same_before_step_position_and_velocity() {
    let shared = shared();
    let mut p = player("bow");
    let mut a = crate::arrow::Arrow::from_mob(401, p.dimension, (0.5, 70.0, 0.5), (0.0, 0.0, 3.0), 999, "");
    a.age = 1;
    let mut state = crate::arrow::Arrows { list: vec![a], retired: Vec::new() };
    let mut out = Outgoing::capture();
    bow::send_arrow_ticks(&mut out, &shared, &mut p, Some(&state)).await.unwrap();
    let a = &mut state.list[0];
    a.age = 20;
    a.z = 45.0;
    a.vy = -0.5;
    a.vz = 2.4;
    let (at, velocity) = a.before_step();
    bow::send_arrow_ticks(&mut out, &shared, &mut p, Some(&state)).await.unwrap();
    let sent = packets(&out).await;
    let sync = sent.iter().find(|p| p.id == 0x23).unwrap();
    let mut c = Cursor::new(sync.payload());
    assert_eq!(c.varint(), Some(401));
    let values: Vec<_> = (0..6).map(|_| f64::from_be_bytes(c.bytes(8).unwrap().try_into().unwrap())).collect();
    assert_eq!(values, vec![at.0, at.1, at.2, velocity.0, velocity.1, velocity.2]);
}

#[path = "projectile_recording_tests.rs"]
mod recordings;

#[path = "projectile_third_tests.rs"]
mod third_pass;
