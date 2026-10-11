use super::*;

#[test]
fn discarded_packets_release_tick_counts_even_after_receiving() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    let waiting = Arc::new([AtomicUsize::new(0), AtomicUsize::new(0)]);
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    sender.send(TickPermit::new(&waiting, 3, true)).ok().unwrap();
    sender.send(TickPermit::new(&waiting, 3, true)).ok().unwrap();
    let received = receiver.try_recv().unwrap();
    assert_eq!(waiting[1].load(Ordering::Acquire), 2);
    drop(receiver);
    assert_eq!(waiting[1].load(Ordering::Acquire), 1);
    drop(received);
    assert_eq!(waiting[1].load(Ordering::Acquire), 0);
    assert!(sender.send(TickPermit::new(&waiting, 3, true)).is_err());
    assert_eq!(waiting[1].load(Ordering::Acquire), 0);
}

fn server(label: &str) -> Shared {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tmp").join(format!("ghost-{label}-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    Shared::new(World::in_memory(), World::in_memory(), root.clone(), root.join("skins"),
        Default::default(), crate::ops::Ops::open(&root.join("ops.json")), Default::default())
}

// Progress хранит статистику относительно cwd. Отдельный процесс держит
// и эти файлы в tmp, не меняя cwd параллельных проверок всего сервера.
fn isolated(test: &str) -> bool {
    if std::env::var("GHOST_TEST_WORKER").as_deref() == Ok(test) { return false; }
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tmp")
        .join(format!("ghost-worker-{test}-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", &format!("network::play::disconnect_tests::{test}"), "--nocapture"])
        .env("GHOST_TEST_WORKER", test).current_dir(&root).output().unwrap();
    std::fs::remove_dir_all(root).unwrap();
    assert!(output.status.success(), "{}\n{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    true
}

#[tokio::test(start_paused = true)]
async fn duplicate_waits_for_save_and_restores_inventory_position_and_cursor_once() {
    if isolated("duplicate_waits_for_save_and_restores_inventory_position_and_cursor_once") { return; }
    let shared = server("replace");
    let uuid = [241; 16];
    let first = shared.sessions.acquire("SheriffAnya", uuid).await;
    let control = first.control();
    let mut player = PlayerState::new(uuid, "SheriffAnya".into(), None, None, 0x7f,
        shared.next_reader(), (12.5, 70.0, -8.25));
    let diamond = blocks::item_named("diamond").unwrap();
    player.inventory.set(36, Some(Stack::new(diamond, 7)));
    player.inventory.set_cursor(Some(Stack::new(diamond, 3)));
    {
        let mut players = shared.players.lock().unwrap();
        player.entity_id = players.join(player.member(), shared.next_entity_id()).unwrap().0.entity_id;
        players.connection(uuid, control.clone());
    }
    let (mut writer, _peer) = tokio::io::duplex(1);
    let mut out = Outgoing::duplex(&mut writer);
    out.control(control.clone());
    out.send(&[1, 2, 3]).await.unwrap();
    let replacement = shared.sessions.acquire("SheriffAnya", uuid);
    tokio::pin!(replacement);
    tokio::select! {
        biased;
        _ = &mut replacement => panic!("данные прежней сессии ещё не сохранены"),
        _ = control.cancelled() => {}
    }
    assert_eq!(out.flush().await.unwrap_err().kind(), io::ErrorKind::ConnectionAborted);
    assert_eq!(shared.players.lock().unwrap().online(), 1);
    finish_play(&shared, &mut player);
    assert_eq!(shared.players.lock().unwrap().online(), 0);
    drop(first);
    let _new_session = replacement.await;
    let saved = playerdata::load(&shared.playerdata, &uuid).unwrap();
    assert_eq!((saved.x, saved.y, saved.z), (12.5, 70.0, -8.25));
    assert_eq!(saved.inventory.cursor(), None);
    let count: i32 = saved.inventory.slots().iter().flatten()
        .filter(|stack| stack.item == diamond).map(|stack| stack.count).sum();
    assert_eq!(count, 10);
    std::fs::remove_dir_all(&shared.playerdata).unwrap();
}

#[tokio::test(start_paused = true)]
async fn command_kick_reaches_the_writer_and_normal_cleanup_removes_the_player() {
    if isolated("command_kick_reaches_the_writer_and_normal_cleanup_removes_the_player") { return; }
    let shared = server("kick");
    let uuid = [242; 16];
    let lease = shared.sessions.acquire("KickPlayer", uuid).await;
    let control = lease.control();
    let mut player = PlayerState::new(uuid, "KickPlayer".into(), None, None, 0x7f,
        shared.next_reader(), (4.0, 72.0, 6.0));
    {
        let mut players = shared.players.lock().unwrap();
        player.entity_id = players.join(player.member(), shared.next_entity_id()).unwrap().0.entity_id;
        players.connection(uuid, control.clone());
    }
    let (mut writer, _peer) = tokio::io::duplex(1);
    let mut out = Outgoing::duplex(&mut writer);
    out.control(control.clone());
    out.send(&[1, 2, 3]).await.unwrap();
    let (result, ()) = tokio::join!(out.flush(), async {
        tokio::time::sleep(Duration::from_millis(1)).await;
        shared.players.lock().unwrap().order(&uuid, Order::Kick { reason: "за дело".into() });
    });
    assert_eq!(result.unwrap_err().kind(), io::ErrorKind::ConnectionAborted);
    assert_eq!(control.reason().unwrap().plain, "за дело");
    finish_play(&shared, &mut player);
    assert_eq!(shared.players.lock().unwrap().online(), 0);
    assert_eq!(playerdata::load(&shared.playerdata, &uuid).unwrap().x, 4.0);
    std::fs::remove_dir_all(&shared.playerdata).unwrap();
}
