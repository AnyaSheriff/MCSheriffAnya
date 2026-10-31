// Настоящий TCP и отдельные test-процессы: COMPRESSION общий для процесса.
// Ни мира на диске, ни запущенного сервера Minecraft здесь нет.
use super::*;
use std::process::{Command, Stdio};
use std::sync::{atomic::Ordering, mpsc};
use std::time::Instant as WallInstant;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::runtime::{Builder, Runtime};

const CHILD: &str = "network::play::inbound_runtime_tests::isolated_tcp_reader_fixture";
const CHILD_MODE: &str = "MCSA_INBOUND_FIXTURE_MODE";

#[test]
fn inbound_runtime_isolation_tcp_matrix_and_lifecycle() {
    // Один child за раз: изменение packet::COMPRESSION не затрагивает другие tests.
    for mode in ["-1:0:normal", "-1:1:normal", "0:0:normal", "0:1:normal",
        "256:0:normal", "256:1:normal", "-1:0:negative", "0:1:malformed", "0:1:cancel",
        "-1:0:legacy", "-1:0:fairness"] {
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args(["--exact", CHILD, "--ignored", "--nocapture"])
            .env(CHILD_MODE, mode).stdout(Stdio::inherit()).stderr(Stdio::inherit())
            .spawn().unwrap();
        let deadline = WallInstant::now() + Duration::from_secs(12);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(status.success(), "TCP reader fixture {mode}: {status}");
                break;
            }
            if WallInstant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("TCP reader fixture {mode}: внешний watchdog 12с");
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

fn wait_until(mut ready: impl FnMut() -> bool, bound: Duration) -> bool {
    let deadline = WallInstant::now() + bound;
    loop {
        if ready() { return true; }
        if WallInstant::now() >= deadline { return false; }
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn runtime(name: &str) -> Runtime {
    Builder::new_multi_thread().worker_threads(1).max_blocking_threads(1)
        .thread_name(name).enable_all().build().unwrap()
}

struct TcpFixture {
    main: Runtime,
    peer_runtime: Runtime,
    server: Option<Connection>,
    peer: Option<Connection>,
    shared: Arc<Shared>,
    alive: KeepAlive,
    inbound: Option<Inbound>,
    id: i64,
    threshold: i32,
}

impl TcpFixture {
    fn new(threshold: i32, encrypted: bool, use_main_reader: bool) -> Self {
        super::super::packet::set_compression_threshold(threshold);
        let main = runtime("test-main-one-worker");
        let peer_runtime = Builder::new_current_thread().enable_all().build().unwrap();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let peer = std::net::TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (server, _) = listener.accept().unwrap();
        peer.set_nonblocking(true).unwrap(); server.set_nonblocking(true).unwrap();
        let mut server = {
            let _entered = main.enter();
            Connection::new(tokio::net::TcpStream::from_std(server).unwrap())
        };
        let mut peer = {
            let _entered = peer_runtime.enter();
            Connection::new(tokio::net::TcpStream::from_std(peer).unwrap())
        };
        server.protocol = Some(mcsa_protocol::Negotiated {
            client: mcsa_protocol::JavaVersion::V26_3,
            native: mcsa_protocol::wire::INTERNAL,
        });
        server.content = mcsa_protocol::content::ContentCodec::white_slab_preview();
        if encrypted { server.enable_encryption(&[37; 16]); peer.enable_encryption(&[37; 16]); }
        // Продвигаем CFB8 ДО take_reader: сброс IV при переносе сломает ACK.
        let prelude = [0x35, 0x00, 0x06];
        let mut framed = Vec::new();
        super::super::packet::frame(&mut framed, &prelude, threshold);
        peer_runtime.block_on(peer.write_all(&framed)).unwrap();
        assert_eq!(main.block_on(read_packet(&mut server)).unwrap().body, prelude);
        let catalog = crate::gameplay_catalog::Catalog::new(crate::gameplay_catalog::Profile::Preview26_3);
        let shared = Arc::new(Shared::new(World::in_memory_with_catalog(catalog),
            World::in_memory_with_catalog(catalog), Default::default(),
            Default::default(), Default::default(), crate::ops::Ops::open(&std::env::temp_dir()
                .join(format!("mcsa-inbound-no-ops-{}.json", std::process::id()))), crate::skins::Settings::default()));
        let alive = KeepAlive::new();
        alive.make_due_for_test();
        main.block_on(async {
            let mut outgoing = Outgoing::new(&mut server);
            outgoing.keep_alive(alive.clone());
            send_keep_alive_due(&mut outgoing, &alive).await.unwrap();
        });
        let challenge = peer_runtime.block_on(read_packet(&mut peer)).unwrap();
        let id = i64::from_be_bytes(challenge.payload().try_into().unwrap());
        assert!(alive.deadline().is_some(), "настоящий challenge flush должен начать deadline");
        let (pending, pending_rx) = mpsc::channel();
        server.probe_read_pending(pending);
        let inbound = if use_main_reader {
            Inbound::start_on(main.handle(), &mut server, &shared, alive.clone())
        } else {
            Inbound::start(&mut server, &shared, alive.clone())
        }.unwrap();
        // Не task-start/yield: сигнал только после реального TCP poll_read(Pending).
        pending_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        Self { main, peer_runtime, server: Some(server), peer: Some(peer),
            shared, alive, inbound: Some(inbound), id, threshold }
    }

    fn encoded(&self, id: i32, payload: &[u8]) -> Vec<u8> {
        let mut body = encode_varint(id);
        body.extend_from_slice(payload);
        let body = mcsa_protocol::wire::translate(mcsa_protocol::State::Play,
            mcsa_protocol::Direction::Serverbound, mcsa_protocol::wire::INTERNAL,
            mcsa_protocol::JavaVersion::V26_3, &body).unwrap();
        let mut framed = Vec::new();
        super::super::packet::frame(&mut framed, &body, self.threshold);
        framed
    }

    fn write_raw(&mut self, frame: &[u8]) {
        self.peer_runtime.block_on(self.peer.as_mut().unwrap().write_all(frame)).unwrap();
    }

    fn send(&mut self, id: i32, payload: &[u8]) { self.write_raw(&self.encoded(id, payload)); }

    fn queued(&self) -> usize { self.shared.waiting_for_tick[0].load(Ordering::Acquire) }

    fn next(&mut self) -> io::Result<Incoming> {
        let inbound = self.inbound.as_mut().unwrap();
        self.main.block_on(async {
            tokio::time::timeout(Duration::from_secs(2), inbound.recv())
                .await.expect("FIFO receive deadline").expect("FIFO closed before terminal error")
        })
    }

    fn expect_game(&mut self, slot: u8) {
        let (packet, arrived, permit) = self.next().unwrap();
        assert_eq!(packet.id, SET_CARRIED_ITEM);
        assert_eq!(packet.payload(), &[0, slot]);
        assert_eq!(arrived, 0);
        drop(permit);
    }

    fn expect_error(&mut self, kind: io::ErrorKind) {
        let error = match self.next() { Err(error) => error, Ok(_) => panic!("ожидалась ошибка после gameplay") };
        assert_eq!(error.kind(), kind);
        if kind == io::ErrorKind::InvalidData { assert!(error.to_string().contains("Недопустимая длина пакета: 0")); }
        assert_eq!(self.queued(), 0);
        assert_eq!(self.inbound.as_ref().unwrap()._budget.usage(), (0, 0));
    }
}

fn held_legacy_session(primary: &mut TcpFixture) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let mut peer = std::net::TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let (server, _) = listener.accept().unwrap();
    server.set_nonblocking(true).unwrap();
    peer.set_write_timeout(Some(Duration::from_secs(2))).unwrap();
    let mut server = {
        let _entered = primary.main.enter();
        Connection::new(tokio::net::TcpStream::from_std(server).unwrap())
    };
    server.protocol = Some(mcsa_protocol::Negotiated {
        client: mcsa_protocol::JavaVersion::V1_12_2,
        native: mcsa_protocol::wire::INTERNAL,
    });
    let session = Arc::new(Mutex::new(mcsa_protocol::legacy_play::Session::new(
        mcsa_protocol::legacy_chunk::HeightWindow::new(-64).unwrap(), 2).unwrap()));
    server.legacy = Some(session.clone());
    let (pending, pending_rx) = mpsc::channel();
    server.probe_read_pending(pending);
    let legacy = Inbound::start(&mut server, &primary.shared, KeepAlive::new()).unwrap();
    pending_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let held = session.lock().unwrap();
    let (waiting, waiting_rx) = mpsc::channel();
    super::super::packet::probe_legacy_lock_wait(waiting);
    // Разрешённый own legacy KeepAlive340: header0x0b и i64, не угадывание777.
    let mut body = vec![0x0b];
    body.extend_from_slice(&71i64.to_be_bytes());
    let mut frame = Vec::new();
    super::super::packet::frame(&mut frame, &body, -1);
    std::io::Write::write_all(&mut peer, &frame).unwrap();
    // Сигнал приходит из actual try_lock(WouldBlock), не из task-start.
    waiting_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    primary.send(SERVERBOUND_KEEP_ALIVE, &primary.id.to_be_bytes());
    assert!(wait_until(|| primary.alive.deadline().is_none(), Duration::from_secs(2)),
        "legacy session mutex не должен останавливать777ACK на том же productionIO");
    assert!(!primary.alive.closed() && !primary.alive.expired());
    // Удерживали session всё время приёма primaryACK; теперь можно завершать reader.
    drop(held);
    drop(legacy);
    drop(server);
}

fn hot_reader_fairness(hot: &mut TcpFixture) {
    let mut cold = TcpFixture::new(-1, false, false);
    let handle = super::super::inbound_runtime::handle().unwrap();
    let (entered, entered_rx) = mpsc::channel();
    let (release, release_rx) = mpsc::channel();
    // Test-only gate: оба реальных TCP уже Pending. Пока IOworker здесь,
    // заполняем оба сокета, затем одновременно разрешаем их обслуживание.
    handle.spawn(async move {
        entered.send(()).unwrap();
        release_rx.recv_timeout(Duration::from_secs(3)).unwrap();
    });
    entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let mut batch = hot.encoded(SERVERBOUND_KEEP_ALIVE, &hot.id.to_be_bytes());
    for _ in 0..512 { batch.extend(hot.encoded(SERVERBOUND_KEEP_ALIVE, &(hot.id + 1).to_be_bytes())); }
    batch.extend(hot.encoded(SET_CARRIED_ITEM, &[0, 7]));
    hot.write_raw(&batch);
    cold.send(SERVERBOUND_KEEP_ALIVE, &cold.id.to_be_bytes());
    release.send(()).unwrap();
    assert!(wait_until(|| cold.alive.deadline().is_none() && hot.queued() == 1,
        Duration::from_secs(2)), "оба готовых reader должны сделать прогресс");
    assert!(hot.alive.deadline().is_none());
    assert!(!cold.alive.closed() && !cold.alive.expired());
    assert!(!hot.alive.closed() && !hot.alive.expired());
    assert!(hot.inbound.as_ref().unwrap()._yield_count.load(Ordering::Relaxed) >= 8,
        "512+ACK кадров обязаны реально пройти64-frame yield, не только TCP readiness");
    assert_eq!(cold.queued(), 0, "ACK не занимает gameplay FIFO");
    // Достаём gameplay горячего reader без main.block_on: его единственный
    // worker всё ещё заблокирован внешним World mutex.
    let queued = hot.inbound.as_mut().unwrap().packets.try_recv().unwrap().unwrap();
    assert_eq!(queued.0.id, SET_CARRIED_ITEM);
    assert_eq!(queued.0.payload(), &[0, 7]);
    drop(queued);
    assert_eq!(hot.queued(), 0);
    assert_eq!(hot.inbound.as_ref().unwrap()._budget.usage(), (0, 0));
    drop(cold.inbound.take());
    drop(cold.server.take());
    drop(cold.peer.take());
}

#[test]
#[ignore = "запускается последовательно отдельным test-процессом из parent fixture"]
fn isolated_tcp_reader_fixture() {
    let mode = std::env::var(CHILD_MODE).expect("fixture запускается только через parent watchdog");
    let parts: Vec<_> = mode.split(':').collect();
    assert_eq!(parts.len(), 3);
    let threshold: i32 = parts[0].parse().unwrap();
    assert!([-1, 0, 256].contains(&threshold));
    let negative = parts[2] == "negative";
    let mut f = TcpFixture::new(threshold, parts[1] == "1", negative);
    // Заимствование main отдельно: callback мутирует TCP fixture, а не runtime.
    let main_handle = f.main.handle().clone();
    let world_owner = f.shared.clone();
    let held = world_owner.world_in(crate::dimension::Dimension::Overworld).lock().unwrap();
    let blocked = f.shared.clone();
    let (entered, entered_rx) = mpsc::channel();
    let (finished, finished_rx) = mpsc::channel();
    main_handle.spawn(async move {
        entered.send(()).unwrap();
        let _held = blocked.world_in(crate::dimension::Dimension::Overworld).lock().unwrap();
        let _ = finished.send(());
    });
    entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if parts[2] == "legacy" { held_legacy_session(&mut f); return; }
        if parts[2] == "fairness" { hot_reader_fairness(&mut f); return; }
        if negative {
            f.send(SERVERBOUND_KEEP_ALIVE, &f.id.to_be_bytes());
            assert!(!wait_until(|| f.alive.deadline().is_none(), Duration::from_millis(200)),
                "negative control обязан обнаружить голодание main reader");
            return;
        }
        f.send(SET_CARRIED_ITEM, &[0, 1]);
        assert!(wait_until(|| f.queued() == 1, Duration::from_secs(2)));
        f.send(SERVERBOUND_KEEP_ALIVE, &(f.id + 1).to_be_bytes());
        f.send(SET_CARRIED_ITEM, &[0, 2]);
        assert!(wait_until(|| f.queued() == 2, Duration::from_secs(2)));
        assert!(f.alive.deadline().is_some(), "wrong ACK уже декодирован перед gameplay2");
        let ack = f.encoded(SERVERBOUND_KEEP_ALIVE, &f.id.to_be_bytes());
        f.write_raw(&ack[..1]);
        assert!(f.alive.deadline().is_some(), "одного байта рамки недостаточно для ACK");
        if parts[2] == "cancel" {
            let budget = f.inbound.as_ref().unwrap()._budget.clone();
            let cancelled = f.inbound.as_ref().unwrap().reader.abort_handle();
            drop(f.inbound.take());
            assert!(f.alive.closed());
            assert_eq!(f.queued(), 0);
            assert_eq!(budget.usage(), (0, 0));
            drop(f.server.take());
            assert!(wait_until(|| cancelled.is_finished(), Duration::from_secs(2)),
                "отменённый reader должен завершиться без main unlock");
            let mut byte = [0];
            let closed = f.peer_runtime.block_on(async {
                tokio::time::timeout(Duration::from_secs(2), f.peer.as_mut().unwrap().read(&mut byte))
                    .await.expect("reader fd должен закрыться без main unlock")
            });
            // При отмене частично отправленного кадра байт может остаться
            // в TCP receive buffer: закрытие тогда даёт RST вместо FIN/EOF.
            // Разрешены только эти два исхода, не данные и не любая ошибка.
            match closed {
                Ok(0) => {},
                Err(error) if error.kind() == io::ErrorKind::ConnectionReset => {},
                other => panic!("ожидалось закрытие отменённого reader: {other:?}"),
            }
            return;
        }
        f.write_raw(&ack[1..]);
        assert!(wait_until(|| f.alive.deadline().is_none(), Duration::from_secs(2)),
            "matching TCP ACK обязан приниматься ДО unlock main mutex");
        assert!(!f.alive.closed() && !f.alive.expired(), "deadline снят matching ACK, не закрытием control");
        f.send(SERVERBOUND_KEEP_ALIVE, &f.id.to_be_bytes());
        f.send(SET_CARRIED_ITEM, &[0, 3]);
        assert!(wait_until(|| f.queued() == 3, Duration::from_secs(2)));
        if parts[2] == "malformed" { f.write_raw(&[0]); }
        else { f.peer_runtime.block_on(f.peer.as_mut().unwrap().shutdown()).unwrap(); }
        assert!(wait_until(|| f.inbound.as_ref().unwrap().reader.is_finished(), Duration::from_secs(2)));
        assert!(!f.alive.closed(), "normal EOF/error handoff не обходит gameplay FIFO");
        assert!(!f.alive.expired());
    }));
    assert!(finished_rx.try_recv().is_err(), "main mutex всё ещё должен быть удержан");
    drop(held);
    finished_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    if let Err(error) = result { std::panic::resume_unwind(error); }
    if negative {
        assert!(wait_until(|| f.alive.deadline().is_none(), Duration::from_secs(2)),
            "negative control читает тот же ACK только после unlock");
        assert!(!f.alive.closed() && !f.alive.expired());
    } else if !["cancel", "legacy", "fairness"].contains(&parts[2]) {
        for slot in 1..=3 { f.expect_game(slot); }
        f.expect_error(if parts[2] == "malformed" { io::ErrorKind::InvalidData } else { io::ErrorKind::UnexpectedEof });
    }
    drop(f.inbound.take());
    drop(f.server.take());
    drop(f.peer.take());
    println!("MCSA_INBOUND_FIXTURE_OK mode={mode}");
}
