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
    chunks.light = Some(((0, 0), job));
    let mut out = Outgoing::capture();
    out.send(&[0x0b]).await.unwrap();
    let before = out.queued().to_vec();
    let mut sent = SentChunks::new();
    let result = tokio::time::timeout(Duration::from_millis(1), chunks.step(&mut out, &shared, dimension, (0, 0), &mut sent, 1)).await;
    assert!(!result.unwrap().unwrap());
    assert!(sent.is_empty());
    assert_eq!(out.queued(), before);
    chunks.light.take().unwrap().1.abort();
}

#[tokio::test]
async fn completed_lighting_outside_view_is_not_sent() {
    let shared = shared();
    let dimension = crate::dimension::Dimension::Overworld;
    let mut chunks = ProgressiveChunks::new(dimension);
    chunks.light = Some(((0, 0), tokio::spawn(async { (ChunkLight::open_sky(), Vec::new()) })));
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
    chunks.light = Some(((0, 0), tokio::spawn(std::future::pending())));
    let mut out = Outgoing::capture();
    let mut sent = SentChunks::from([(0, 0)]);
    assert!(chunks.step(&mut out, &shared, crate::dimension::Dimension::Nether, (0, 0), &mut sent, 0).await.unwrap());
    assert!(chunks.light.is_none());
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
