use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

fn waiting() -> Arc<[AtomicUsize;2]> { Arc::new([AtomicUsize::new(0),AtomicUsize::new(0)]) }
fn packet(id: i32) -> PacketInfo {
    let body = encode_varint(id);
    PacketInfo { id,length:body.len() as i32,body,teleport:None,punch:false,foreign_inventory:false,legacy_inventory:None }
}
fn ack(id: i64) -> PacketInfo {
    let mut result = packet(SERVERBOUND_KEEP_ALIVE);
    result.body.extend_from_slice(&id.to_be_bytes());
    result.length = result.body.len() as i32;
    result
}

#[test]
fn packet_and_byte_limits_are_independent_exact_and_released_by_drop() {
    let budget = Arc::new(InboundBudget::default());
    let full = budget.charge(INBOUND_BYTES).unwrap();
    assert_eq!(budget.usage(),(1,INBOUND_BYTES));
    assert!(budget.charge(1).is_err());
    drop(full);
    assert_eq!(budget.usage(),(0,0));
    assert!(budget.charge(INBOUND_BYTES+1).is_err());
    assert_eq!(budget.usage(),(0,0));
    let charges: Vec<_> = (0..INBOUND_PACKETS).map(|_|budget.charge(0).unwrap()).collect();
    assert_eq!(budget.usage(),(INBOUND_PACKETS,0));
    assert!(budget.charge(0).is_err());
    drop(charges);
    assert_eq!(budget.usage(),(0,0));
}

#[test]
fn full_queue_accepts_ack_without_charge_and_overflow_follows_the_whole_fifo() {
    let budget = Arc::new(InboundBudget::default());
    let waiting = waiting(); let alive = KeepAlive::new();
    let (sender,mut receiver) = tokio::sync::mpsc::unbounded_channel();
    let mut guard = alive.reader_guard();
    let size = inbound_packet_charge(&packet(SWING_ARM));
    for _ in 0..INBOUND_PACKETS {
        assert!(forward_read_budgeted(Ok(packet(SWING_ARM)),1,&waiting,&alive,&sender,&mut guard,Some(&budget)));
    }
    assert_eq!(budget.usage(),(INBOUND_PACKETS,INBOUND_PACKETS*size));
    alive.make_due_for_test();
    let challenge = alive.send_due().unwrap();
    assert!(alive.deadline().is_some());
    assert!(forward_read_budgeted(Ok(ack(challenge)),1,&waiting,&alive,&sender,&mut guard,Some(&budget)));
    assert!(alive.deadline().is_none(),"matching ACK принят при полной очереди");
    assert!(!alive.closed() && !alive.expired());
    assert_eq!(receiver.len(),INBOUND_PACKETS,"ACK не занимает FIFO");
    assert!(!forward_read_budgeted(Ok(packet(SWING_ARM)),1,&waiting,&alive,&sender,&mut guard,Some(&budget)));
    drop(guard); drop(sender);
    assert!(!alive.closed(),"terminal error не закрывает control до своей очереди");
    assert_eq!(waiting[1].load(Ordering::Acquire),INBOUND_PACKETS);
    for left in (0..INBOUND_PACKETS).rev() {
        let incoming = receiver.try_recv().unwrap().unwrap();
        assert_eq!(incoming.0.id,SWING_ARM);
        assert_eq!(budget.usage().0,left+1,"извлечение не освобождает обработку");
        drop(incoming);
        assert_eq!(budget.usage(),(left,left*size));
    }
    let terminal = receiver.try_recv().unwrap().err().expect("overflow идёт после всех действий");
    assert_eq!(terminal.kind(),io::ErrorKind::InvalidData);
    assert_eq!(budget.usage(),(0,0));
    assert_eq!(waiting[1].load(Ordering::Acquire),0);
    assert!(receiver.try_recv().is_err());
}

#[test]
fn failed_channel_handoff_drops_charge_and_tick_permit_then_closes_reader_guard() {
    let budget = Arc::new(InboundBudget::default());
    let waiting = waiting(); let alive = KeepAlive::new();
    let (sender,receiver) = tokio::sync::mpsc::unbounded_channel(); drop(receiver);
    let mut guard = alive.reader_guard();
    assert!(!forward_read_budgeted(Ok(packet(SWING_ARM)),1,&waiting,&alive,&sender,&mut guard,Some(&budget)));
    assert_eq!(budget.usage(),(0,0)); assert_eq!(waiting[1].load(Ordering::Acquire),0);
    drop(guard); assert!(alive.closed());
}

#[tokio::test]
async fn charge_survives_recv_and_deferred_packet_and_releases_on_inbound_drop() {
    let budget = Arc::new(InboundBudget::default());
    let waiting = waiting(); let alive = KeepAlive::new();
    let (sender,receiver) = tokio::sync::mpsc::unbounded_channel();
    let mut guard = alive.reader_guard();
    for _ in 0..2 {
        assert!(forward_read_budgeted(Ok(packet(SWING_ARM)),1,&waiting,&alive,&sender,&mut guard,Some(&budget)));
    }
    let mut inbound = Inbound { packets:receiver,reader:tokio::spawn(std::future::pending()),queued:None,
        alive:alive.clone(),_budget:budget.clone(),_yield_count:Default::default() };
    let processing = inbound.recv().await.unwrap().unwrap();
    assert_eq!(budget.usage().0,2);
    assert!(inbound.take_gameplay(0).is_none(),"пакет будущего такта остаётся deferred");
    assert!(inbound.queued.is_some()); assert_eq!(budget.usage().0,2);
    drop(processing); assert_eq!(budget.usage().0,1);
    drop(inbound); assert_eq!(budget.usage(),(0,0));
    assert_eq!(waiting[1].load(Ordering::Acquire),0); assert!(alive.closed());
}

#[test]
fn eof_preserves_fifo_and_budget_until_previous_action_is_consumed() {
    let budget = Arc::new(InboundBudget::default());
    let waiting = waiting(); let alive = KeepAlive::new();
    let (sender,mut receiver) = tokio::sync::mpsc::unbounded_channel();
    let mut guard = alive.reader_guard();
    assert!(forward_read_budgeted(Ok(packet(SWING_ARM)),1,&waiting,&alive,&sender,&mut guard,Some(&budget)));
    assert!(!forward_read_budgeted(Err(io::Error::new(io::ErrorKind::UnexpectedEof,"конец потока")),1,
        &waiting,&alive,&sender,&mut guard,Some(&budget)));
    drop(guard); assert!(!alive.closed());
    let action = receiver.try_recv().unwrap().unwrap(); assert_eq!(budget.usage().0,1);
    drop(action); assert_eq!(budget.usage(),(0,0));
    assert_eq!(receiver.try_recv().unwrap().err().unwrap().kind(),io::ErrorKind::UnexpectedEof);
}

#[test]
fn small_length_with_large_retained_allocation_is_charged_by_capacity() {
    let budget = Arc::new(InboundBudget::default());
    let waiting = waiting(); let alive = KeepAlive::new();
    let mut body = Vec::with_capacity(INBOUND_BYTES);
    body.extend(encode_varint(SWING_ARM));
    let oversized = PacketInfo { id:SWING_ARM,length:body.len() as i32,body,teleport:None,punch:false,
        foreign_inventory:false,legacy_inventory:None };
    assert!(oversized.body.len() < 8);
    assert!(inbound_packet_charge(&oversized) > INBOUND_BYTES);
    assert!(inbound_packet_budgeted(oversized,1,&waiting,&alive,Some(&budget)).is_err());
    assert_eq!(budget.usage(),(0,0)); assert_eq!(waiting[1].load(Ordering::Acquire),0);
    let small = packet(SWING_ARM);
    let expected = small.body.capacity()+std::mem::size_of::<Incoming>();
    let incoming = inbound_packet_budgeted(small,1,&waiting,&alive,Some(&budget)).unwrap().unwrap();
    assert_eq!(budget.usage(),(1,expected));
    drop(incoming); assert_eq!(budget.usage(),(0,0));
}

#[test]
fn abort_before_first_reader_poll_closes_guard_and_duplicate_socket() {
    let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let mut peer = std::net::TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let (server,_) = listener.accept().unwrap();
    server.set_nonblocking(true).unwrap();
    let mut server = {
        let _entered = runtime.enter();
        Connection::new(tokio::net::TcpStream::from_std(server).unwrap())
    };
    let shared = Shared::new(World::in_memory(),World::in_memory(),Default::default(),Default::default(),
        Default::default(),crate::ops::Ops::open(&std::env::temp_dir().join("mcsa-inbound-abort-no-ops.json")),
        crate::skins::Settings::default());
    let alive = KeepAlive::new();
    // current_thread executor ещё не запускался: настоящий start_on создал
    // reader future, но она гарантированно не получила ни одного poll.
    let mut inbound = Inbound::start_on(runtime.handle(),&mut server,&shared,alive.clone()).unwrap();
    let budget = inbound._budget.clone();
    assert!(!alive.closed());
    inbound.reader.abort();
    let completed = runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(2),&mut inbound.reader).await.unwrap()
    });
    assert!(completed.unwrap_err().is_cancelled());
    assert!(inbound.reader.is_finished());
    assert!(alive.closed(),"guard должен существовать уже до первого poll");
    assert_eq!(budget.usage(),(0,0));
    drop(inbound); drop(server);
    peer.set_read_timeout(Some(Duration::from_secs(1))).unwrap();
    assert_eq!(std::io::Read::read(&mut peer,&mut [0]).unwrap(),0,"дубликат reader fd освобождён");
}
