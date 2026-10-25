use super::*;

fn shared() -> Shared {
    Shared::new(World::in_memory(), World::in_memory(), "unused/playerdata".into(), "unused/skins".into(),
        crate::config::server_properties::ServerProperties::default(),
        crate::ops::Ops::open(std::path::Path::new("unused/ops.json")), crate::skins::Settings::default())
}

#[tokio::test(start_paused = true)]
async fn unfinished_lighting_does_not_wait_or_consume_socket_output() {
    let shared = shared();
    let dimension = crate::dimension::Dimension::Overworld;
    let mut chunks = ProgressiveChunks::new(dimension);
    let job = tokio::spawn(std::future::pending());
    chunks.light.push(((0, 0), job));
    let mut out = Outgoing::capture();
    out.send(&[0x0b]).await.unwrap();
    let before = out.queued().to_vec();
    let mut sent = SentChunks::new();
    let result = tokio::time::timeout(Duration::from_millis(1), chunks.step(&mut out, &shared, dimension, (0, 0), &mut sent, 1)).await;
    assert!(!result.unwrap().unwrap());
    assert!(sent.is_empty());
    assert_eq!(out.queued(), before);
    chunks.light.pop().unwrap().1.abort();
}

#[tokio::test]
async fn completed_lighting_outside_view_is_not_sent() {
    let shared = shared();
    let dimension = crate::dimension::Dimension::Overworld;
    let mut chunks = ProgressiveChunks::new(dimension);
    chunks.light.push(((0, 0), tokio::spawn(async { Ok(None) })));
    tokio::task::yield_now().await;
    let mut out = Outgoing::capture();
    let mut sent = SentChunks::from([(9, 9)]);
    assert!(chunks.step(&mut out, &shared, dimension, (9, 9), &mut sent, 0).await.unwrap());
    assert_eq!(sent, SentChunks::from([(9, 9)]));
    assert!(out.queued().is_empty());
}

#[tokio::test]
async fn dimension_change_discards_pending_chunk_work() {
    let shared = shared();
    let mut chunks = ProgressiveChunks::new(crate::dimension::Dimension::Overworld);
    chunks.light.push(((0, 0), tokio::spawn(std::future::pending())));
    let mut out = Outgoing::capture();
    let mut sent = SentChunks::from([(0, 0)]);
    assert!(chunks.step(&mut out, &shared, crate::dimension::Dimension::Nether, (0, 0), &mut sent, 0).await.unwrap());
    assert!(chunks.light.is_empty());
    assert!(out.queued().is_empty());
}

#[tokio::test(start_paused = true)]
async fn dirty_lighting_does_not_block_gameplay_or_lose_a_new_edit() {
    let shared = shared();
    let dimension = crate::dimension::Dimension::Overworld;
    let mut lights = LightUpdates { dimension: Some(dimension), ..Default::default() };
    lights.running.push(((0, 0), tokio::spawn(std::future::pending())));
    lights.pending.insert((0, 0));
    let mut out = Outgoing::capture();
    tokio::time::timeout(Duration::from_millis(1), lights.step(&mut out, &shared, dimension, (0, 0), 1)).await.unwrap().unwrap();
    assert!(lights.pending.contains(&(0, 0)));
    assert_eq!(lights.running.len(), 1);
    assert!(out.queued().is_empty());
    lights.running.pop().unwrap().1.abort();
}

#[tokio::test]
async fn dirty_lighting_is_discarded_when_changing_dimension() {
    let shared = shared();
    let mut lights = LightUpdates { dimension: Some(crate::dimension::Dimension::Overworld), ..Default::default() };
    lights.pending.insert((0, 0));
    lights.running.push(((0, 0), tokio::spawn(std::future::pending())));
    let mut out = Outgoing::capture();
    lights.step(&mut out, &shared, crate::dimension::Dimension::Nether, (0, 0), 1).await.unwrap();
    assert!(lights.running.is_empty());
    assert!(lights.pending.is_empty());
    assert!(out.queued().is_empty());
}

#[test]
fn queued_preparation_is_bounded_deduplicated_and_cancelled_before_work() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    // Отдельный процесс отделяет глобальную квоту от соседних тестов.
    if std::env::var("CHUNK_QUEUE_TEST_WORKER").as_deref() != Ok("1") {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "network::play::chunk_stream_tests::queued_preparation_is_bounded_deduplicated_and_cancelled_before_work", "--nocapture"])
            .env("CHUNK_QUEUE_TEST_WORKER", "1").status().unwrap();
        assert!(status.success());
        return;
    }
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().max_blocking_threads(1).build().unwrap();
    runtime.block_on(async {
        let (release, blocked) = std::sync::mpsc::channel();
        let (started, started_rx) = std::sync::mpsc::channel();
        let blocker = tokio::task::spawn_blocking(move || { started.send(()).unwrap(); blocked.recv().unwrap(); });
        started_rx.recv().unwrap();
        let calls = Arc::new(AtomicUsize::new(0));
        let mut first = Prefetch::new();
        first.set_demand((0, 0), 100_000);
        first.schedule_with(("test-bounded".into(), 0, 0), || panic!("устаревшая работа"));
        let mut second = Prefetch::new();
        second.set_demand((0, 0), 100_000);
        second.slots = first.slots.clone();
        second.schedule_with(("test-bounded".into(), 0, 0), || panic!("дубль"));
        assert!(second.pending.is_empty());
        for index in 1..1000 {
            let calls = calls.clone();
            first.schedule_with(("test-bounded".into(), index, 0), move || {
                calls.fetch_add(1, Ordering::SeqCst);
                panic!("устаревшая работа")
            });
        }
        assert_eq!(first.pending.len(), PREPARATIONS_PER_CONNECTION);
        second.schedule_with(("test-bounded-second".into(), 0, 0), || panic!("общая квота подключения"));
        assert!(second.pending.is_empty());
        let mut third = Prefetch::new();
        third.set_demand((0, 0), 100_000);
        let mut fourth = Prefetch::new();
        fourth.set_demand((0, 0), 100_000);
        for index in 0..1000 {
            let calls = calls.clone();
            third.schedule_with(("test-disconnect".into(), index, 0), move || {
                calls.fetch_add(1, Ordering::SeqCst);
                panic!("отключённый игрок")
            });
        }
        assert_eq!(third.pending.len(), PREPARATIONS_PER_CONNECTION);
        assert_eq!(preparations().lock().unwrap().len(), PREPARATIONS_GLOBAL);
        fourth.schedule_with(("test-global".into(), 0, 0), || panic!("глобальная квота"));
        assert!(fourth.pending.is_empty());
        first.set_demand((100_000, 100_000), 1);
        let cancellation = third.demand.0.clone();
        drop(third);
        assert!(!preparation_wanted(&cancellation, (0, 0)));
        release.send(()).unwrap();
        blocker.await.unwrap();
        tokio::task::spawn_blocking(|| ()).await.unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(first.collect(&shared(), crate::dimension::Dimension::Overworld).is_empty());
        assert!(first.pending.is_empty());
        assert!(preparations().lock().unwrap().is_empty());
    });
}

#[tokio::test]
async fn a_stale_light_chunk_cannot_overwrite_newer_b_and_is_rescheduled() {
    for version in [mcsa_protocol::JavaVersion::V26_1_2, mcsa_protocol::JavaVersion::V26_3] {
        let shared = shared();
        let dimension = crate::dimension::Dimension::Overworld;
        let a: LitChunk = {
            let mut world = shared.world_in(dimension).lock().unwrap();
            for x in -1..=1 { for z in -1..=1 { world.ensure(x, z); } }
            let (_, fresh) = world.light_job(0, 0).unwrap().finish();
            world.store_light(fresh);
            world.light_job(0, 0).unwrap().prepare().into()
        };
        {
            let mut world = shared.world_in(dimension).lock().unwrap();
            world.set_block(16, 10, 0, crate::blocks::state_by_name("torch").unwrap());
        }
        let mut updates = LightUpdates { dimension: Some(dimension), ..Default::default() };
        updates.pending.insert((0, 0));
        let mut out = Outgoing::capture_content(version, shared.catalog().content_codec());
        updates.step(&mut out, &shared, dimension, (0, 0), 1).await.unwrap();
        while !updates.running.iter().all(|(_, job)| job.is_finished()) { tokio::task::yield_now().await; }
        updates.step(&mut out, &shared, dimension, (0, 0), 1).await.unwrap();
        let b = out.queued().to_vec();
        assert!(!b.is_empty());
        assert_eq!(send_prepared_chunk(&mut out, shared.world_in(dimension), 0, 0, &a).await.unwrap(), None);
        assert_eq!(out.queued(), b);
        let mut chunks = ProgressiveChunks::new(dimension);
        chunks.light.push(((0, 0), tokio::spawn(async { Ok(Some(a)) })));
        tokio::task::yield_now().await;
        let mut sent = SentChunks::new();
        assert!(!chunks.step(&mut out, &shared, dimension, (0, 0), &mut sent, 0).await.unwrap());
        assert!(sent.is_empty());
        assert_eq!(out.queued(), b);
        assert_eq!(chunks.light.len(), 1);
    }
}

#[tokio::test]
async fn unavailable_neighbour_does_not_stall_other_prepared_chunks() {
    let shared = shared();
    let dimension = crate::dimension::Dimension::Overworld;
    {
        let mut world = shared.world_in(dimension).lock().unwrap();
        for x in -2..=2 { for z in -2..=2 { if (x, z) != (0, 0) { world.ensure(x, z); } } }
    }
    let mut chunks = ProgressiveChunks::new(dimension);
    let deadline = std::time::Instant::now() + Duration::from_secs(60);
    chunks.loads.retry.insert((0, 0), (deadline, 3));
    let mut out = Outgoing::capture();
    let mut sent = SentChunks::new();
    for _ in 0..32 {
        chunks.step(&mut out, &shared, dimension, (0, 0), &mut sent, 1).await.unwrap();
        assert!(!chunks.loads.pending.contains(&(0, 0)));
        if sent.len() == 8 { break; }
        while !chunks.light.iter().all(|(_, job)| job.is_finished()) { tokio::task::yield_now().await; }
    }
    assert_eq!(sent.len(), 8);
    assert!(!sent.contains(&(0, 0)));
    assert_eq!(chunks.loads.retry[&(0, 0)].0, deadline);
}

#[test]
fn command_suggestion_input_limit_counts_utf16_before_traversal() {
    let shared = shared();
    for (text, allowed) in [("a".repeat(32_500), true), ("a".repeat(32_501), false),
        ("😀".repeat(16_250), true), ("😀".repeat(16_251), false), ("a".repeat(2_000_000), false)] {
        let mut payload = encode_varint(42);
        payload.extend_from_slice(&encode_string(&text));
        assert_eq!(suggestions_for(&payload, &shared, &[1; 16], 777).is_some(), allowed);
    }
}

#[tokio::test]
async fn background_chunk_serialization_preserves_wire_bytes_and_rejects_a_later_edit() {
    for version in [mcsa_protocol::JavaVersion::V26_1_2, mcsa_protocol::JavaVersion::V26_3] {
        let shared = shared();
        let dimension = crate::dimension::Dimension::Overworld;
        let demand = Demand::default();
        *demand.lock().unwrap() = Some(((0, 0), 1));
        let mut direct = Outgoing::capture_content(version, shared.catalog().content_codec());
        let registry = direct.chunk_biomes().unwrap();
        let pending = {
            let mut world = shared.world_in(dimension).lock().unwrap();
            for x in -1..=1 { for z in -1..=1 { world.ensure(x, z); } }
            world.set_block(1, 10, 1, crate::blocks::state_by_name("barrel").unwrap());
            world.set_block(2, 10, 1, crate::blocks::state_by_name("torch").unwrap());
            let build = ChunkBuild { snapshot: chunk_snapshot(&world, 0, 0), registry, encoder: direct.chunk_encoder() };
            start_light(world.light_job(0, 0).unwrap(), demand.0.clone(), Some(build)).unwrap()
        };
        let prepared = pending.1.await.unwrap().unwrap().unwrap();
        send_chunk(&mut direct, shared.world_in(dimension), 0, 0, &prepared.light).await.unwrap();
        let mut background = Outgoing::capture_content(version, shared.catalog().content_codec());
        assert!(send_prepared_chunk(&mut background, shared.world_in(dimension), 0, 0, &prepared).await.unwrap().is_some());
        assert_eq!(background.queued(), direct.queued());
        shared.world_in(dimension).lock().unwrap().set_block(2, 10, 1, world::AIR);
        let before = background.queued().to_vec();
        assert!(send_prepared_chunk(&mut background, shared.world_in(dimension), 0, 0, &prepared).await.unwrap().is_none());
        assert_eq!(background.queued(), before);
    }
}

#[test]
fn restoring_a_player_below_the_world_relocates_without_losing_saved_state() {
    let mut inventory = Inventory::new();
    inventory.set(36, Some(Stack::new(1, 23)));
    inventory.set_cursor(Some(Stack::new(1, 7)));
    let abilities = playerdata::Abilities { flying: true, ..playerdata::Abilities::for_mode(1) };
    let saved = PlayerData { abilities, shoulders: [None, None], inventory: inventory.clone(),
        x: 100.0, y: f64::from(world::MIN_Y - 10), z: -50.0, yaw: 23.0, pitch: 7.0, game_mode: 1,
        dimension: Default::default(), vitals: playerdata::SavedVitals { experience_total: 42, ..Default::default() },
        respawn: None, ender: playerdata::empty_ender() };
    let player = PlayerState::new([9; 16], "VoidRestore".into(), Some(saved), None, 127, 1, (1.5, 64.0, 2.5));
    assert_eq!((player.x, player.y, player.z), (1.5, 64.0, 2.5));
    assert_eq!(player.inventory, inventory);
    assert_eq!(player.abilities, abilities);
    assert_eq!(player.vit.xp_total, 42);
}

#[test]
fn oversized_completion_disconnects_even_after_its_connection_budget_is_empty() {
    let shared = shared();
    let mut player = PlayerState::new([8; 16], "TabLimit".into(), None, None, 127, 1, (0.5, 64.0, 0.5));
    shared.players.lock().unwrap().join(player.member(), 1).unwrap();
    for _ in 0..16 { assert!(player.net.completion_budget.take(0)); }
    assert!(!player.net.completion_budget.take(0));
    for text in ["a".repeat(32_501), "😀".repeat(16_251), "a".repeat(2_000_000)] {
        let mut body = encode_varint(TAB_COMPLETE_REQUEST);
        body.extend(encode_varint(42));
        body.extend(encode_string(&text));
        let packet = PacketInfo { id: TAB_COMPLETE_REQUEST, length: body.len() as i32, body,
            teleport: None, punch: false, foreign_inventory: false, legacy_inventory: None };
        assert!(handle_client_packet_inner(&packet, &mut player, &shared).suggestions.is_none());
        let orders = shared.players.lock().unwrap().take_orders(&player.uuid);
        assert!(matches!(&orders[..], [Order::Kick { reason }] if reason.contains("32500")));
    }
}
