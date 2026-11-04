//! Настоящая команда → очередь владельца → публикация → снимок → Equipment.
use super::*;
use crate::gameplay_catalog::{Catalog, Profile};
use std::sync::atomic::Ordering;

const SPAWN: (f64, f64, f64) = (0.5, 64.0, 0.5);
static NEXT_SCENE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn scene(profile: Profile) -> (Shared, PlayerState, PlayerState, Arc<tokio::sync::Notify>) {
    let catalog = Catalog::new(profile);
    let ops_dir = std::env::temp_dir().join(format!("mcsa-command-equipment-{}-{}",
        std::process::id(), NEXT_SCENE.fetch_add(1, Ordering::Relaxed)));
    std::fs::create_dir(&ops_dir).unwrap();
    let shared = Shared::new(
        crate::world::World::in_memory_with_catalog(catalog), crate::world::World::in_memory_with_catalog(catalog),
        Default::default(), Default::default(), Default::default(),
        crate::ops::Ops::open(&ops_dir.join("ops.json")),
        crate::skins::Settings::default(),
    );
    let mut owner = PlayerState::new_for_catalog([0xd1;16], "Target".into(), None, None, 0x7f, 91, SPAWN, catalog);
    let mut observer = PlayerState::new_for_catalog([0xd2;16], "Observer".into(), None, None, 0x7f, 92, SPAWN, catalog);
    owner.entity_id = 91; observer.entity_id = 92;
    owner.inventory.set(inventory::FIRST_HOTBAR, Some(Stack::new(blocks::item_named("wooden_sword").unwrap(), 1)));
    owner.last_attack = 7;
    let wake = {
        let mut players = shared.players.lock().unwrap();
        players.join(owner.member(), owner.entity_id).unwrap();
        players.join(observer.member(), observer.entity_id).unwrap();
        players.waker(&owner.uuid)
    };
    // Только исходное состояние до команды: изменённую руку публикует
    // исключительно production apply_command_hold, не сам тест.
    publish_player(&shared, &owner); publish_player(&shared, &observer);
    shared.ops.lock().unwrap().give(&observer.uuid, &observer.name, crate::ops::HIGHEST_LEVEL);
    std::fs::remove_dir_all(ops_dir).unwrap();
    (shared, owner, observer, wake)
}

fn command(observer: &mut PlayerState, shared: &Shared, damage: i32) {
    let text = format!("item replace entity Target weapon.mainhand with wooden_sword[damage={damage}]");
    let mut body = encode_varint(SERVERBOUND_CHAT_COMMAND);
    body.extend(encode_string(&text));
    let packet = PacketInfo { id: SERVERBOUND_CHAT_COMMAND, length: body.len() as i32, body,
        teleport: None, punch: false, foreign_inventory: false, legacy_inventory: None };
    handle_client_packet(&packet, observer, shared);
}

async fn apply_owner(shared: &Shared, owner: &mut PlayerState) -> Outgoing<'static> {
    let mut out = Outgoing::capture();
    let orders = take_orders(shared, &owner.uuid);
    assert_eq!(orders.len(), 1);
    let Order::CommandHold { stack, completion } = orders.into_iter().next().unwrap() else { panic!("командная очередь"); };
    apply_command_hold(&mut out, shared, owner, stack, completion).await.unwrap();
    out
}

#[tokio::test]
async fn command_equipment_reaches_the_same_frozen_tick_in_both_poll_orders() {
    use std::future::Future;
    for observer_first in [false, true] {
        let (shared, mut owner, mut observer, wake) = scene(Profile::Preview26_3);
        let mut seen = Seen::new();
        send_other_moves(&mut Outgoing::capture(), &shared, &observer, &mut seen, Phase::Moves).await.unwrap();
        for (damage, tick) in [15,56,98,139].into_iter().enumerate() {
            shared.ticks.send_replace(tick - 1);
            shared.tick_starts.send_replace(tick);
            let sender = TickPermit::new(&shared.waiting_for_tick, tick - 1, true);
            let before_attributes = (owner.attrs_touched.clone(), owner.speed_due.clone(), owner.last_attack);
            command(&mut observer, &shared, damage as i32 + 1);
            drop(sender);
            assert_eq!(shared.waiting_for_tick[((tick - 1) % 2) as usize].load(Ordering::Acquire), 1);
            // Notify сохраняет wake, даже если владелец ещё не начал ждать.
            assert!(tokio::time::timeout(Duration::from_millis(20), wake.notified()).await.is_ok());
            let mut freeze = std::pin::pin!(async {
                assert!(crate::tick::wait_for_player_packets(&shared, tick).await);
                crate::tick::publish_finished_tick(&shared, tick);
            });
            if observer_first {
                let mut context = std::task::Context::from_waker(std::task::Waker::noop());
                assert!(freeze.as_mut().poll(&mut context).is_pending(), "снимок ждёт владельца");
                assert_eq!(*shared.ticks.borrow(), tick - 1);
            }
            let own = apply_owner(&shared, &mut owner).await;
            freeze.await;
            assert_eq!((owner.attrs_touched.clone(), owner.speed_due.clone(), owner.last_attack), before_attributes);
            let stack = owner.inventory.held().unwrap();
            assert_eq!(stack.extras().damage, damage as i32 + 1);
            let mut own_bytes = own.queued();
            let slot = read_packet(&mut own_bytes).await.unwrap();
            assert_eq!(slot.id, SET_CONTAINER_SLOT);
            let mut cursor = Cursor::new(slot.payload());
            assert_eq!(cursor.varint(), Some(PLAYER_WINDOW));
            assert_eq!(cursor.varint(), Some(owner.state_id));
            assert_eq!(cursor.u16(), Some(inventory::FIRST_HOTBAR as u16));
            let mut expected_slot = Vec::new(); push_slot(&mut expected_slot, Some(stack));
            assert_eq!(&slot.payload()[cursor.offset..], expected_slot);
            assert!(own_bytes.is_empty());
            // Сам production freeze снял новую руку; тест её не публикует.
            let snapshot = shared.players.lock().unwrap().published().to_vec();
            assert_eq!(snapshot.iter().find(|(p,_)| p.uuid == owner.uuid).unwrap().0.held, Some(stack));
            observer.publication = Some(TrackingPublication { tick, events: None, players: snapshot });
            let mut other = Outgoing::capture();
            send_other_moves(&mut other, &shared, &observer, &mut seen, Phase::Gear).await.unwrap();
            let mut bytes = other.queued();
            let equipment = read_packet(&mut bytes).await.unwrap();
            assert_eq!(equipment.id, SET_EQUIPMENT);
            let mut expected = encode_varint(owner.entity_id); expected.push(EQUIPMENT_MAIN_HAND);
            push_slot(&mut expected, Some(stack));
            assert_eq!(equipment.payload(), expected);
            assert!(bytes.is_empty(), "без ранних Attributes или дубликатов");
            assert_eq!(*shared.ticks.borrow(), tick);
        }
    }
}

#[tokio::test]
async fn timed_out_command_completion_and_its_clones_release_only_once() {
    let (shared, mut owner, mut observer, _) = scene(Profile::Preview26_3);
    shared.tick_starts.send_replace(15); shared.ticks.send_replace(14);
    command(&mut observer, &shared, 1);
    let orders = take_orders(&shared, &owner.uuid);
    let copy = orders[0].clone();
    let Order::CommandHold { stack, completion } = orders.into_iter().next().unwrap() else { panic!("команда"); };
    // Владелец не ответил: прежний предел 5 мс остаётся верхней границей.
    assert!(!tokio::time::timeout(Duration::from_secs(1), crate::tick::wait_for_player_packets(&shared, 15)).await.unwrap());
    assert_eq!(shared.waiting_for_tick[0].load(Ordering::Acquire), 0);
    assert!(crate::tick::wait_for_player_packets(&shared, 17).await);
    apply_command_hold(&mut Outgoing::capture(), &shared, &mut owner, stack, completion).await.unwrap();
    drop(copy);
    assert_eq!(shared.waiting_for_tick[0].load(Ordering::Acquire), 0, "expiry и поздний finish не вычитают дважды");
}

#[test]
fn disconnect_drops_pending_command_and_registry_is_bounded() {
    let (shared, owner, mut observer, _) = scene(Profile::Preview26_3);
    shared.tick_starts.send_replace(15); shared.ticks.send_replace(14);
    command(&mut observer, &shared, 1);
    shared.players.lock().unwrap().leave(&owner.uuid);
    assert_eq!(shared.waiting_for_tick[0].load(Ordering::Acquire), 0);
    // Исчезнувшая цель не оставляет билет даже при прямом вызове очереди.
    shared.order_mainhand(&owner.uuid, owner.inventory.held());
    assert_eq!(shared.waiting_for_tick[0].load(Ordering::Acquire), 0);
    shared.players.lock().unwrap().join(owner.member(), owner.entity_id).unwrap();
    shared.players.lock().unwrap().waker(&owner.uuid);
    for _ in 0..1030 { shared.order_mainhand(&owner.uuid, owner.inventory.held()); }
    assert_eq!(shared.waiting_for_tick[0].load(Ordering::Acquire), 1024);
    shared.expire_command_equipment(15);
    assert_eq!(shared.waiting_for_tick[0].load(Ordering::Acquire), 0);
    let orders = take_orders(&shared, &owner.uuid);
    assert_eq!(orders.len(), 1030, "перегрузка не теряет поручения");
    drop(orders);
    assert_eq!(shared.waiting_for_tick[0].load(Ordering::Acquire), 0);
}

#[tokio::test]
async fn gated_wait_wakes_only_for_command_and_does_not_lose_an_early_notify() {
    use std::future::Future;
    let (shared, owner, mut observer, wake) = scene(Profile::Preview26_3);
    shared.players.lock().unwrap().order(&owner.uuid, Order::Teleport { x: 1.0, y: 64.0, z: 2.0 });
    let mut waiting = std::pin::pin!(command_hold_wake(&shared, &owner.uuid, &wake));
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    assert!(waiting.as_mut().poll(&mut context).is_pending(), "обычное поручение не прерывает gate");
    command(&mut observer, &shared, 1);
    waiting.await;
    let orders = take_orders(&shared, &owner.uuid);
    assert!(matches!(orders.as_slice(), [Order::Teleport { .. }, Order::CommandHold { .. }]));
    drop(orders);
    // Очередь и Notify уже готовы до первого poll ожидающего владельца.
    command(&mut observer, &shared, 2);
    command_hold_wake(&shared, &owner.uuid, &wake).await;
    drop(take_orders(&shared, &owner.uuid));
    assert!(!shared.waiting_for_tick.iter().any(|v| v.load(Ordering::Acquire) != 0));
}

#[test]
fn between_ticks_command_targets_the_next_boundary_and_preserves_fifo() {
    let (shared, _, mut observer, _) = scene(Profile::Preview26_3);
    let uuid = [0xd1;16];
    shared.tick_starts.send_replace(14); shared.ticks.send_replace(14);
    shared.players.lock().unwrap().order(&uuid, Order::Clear);
    command(&mut observer, &shared, 2);
    let after = Stack::new(blocks::item_named("stone").unwrap(), 1);
    shared.players.lock().unwrap().order(&uuid, Order::Give { stack: after });
    assert_eq!(shared.waiting_for_tick[0].load(Ordering::Acquire), 1);
    shared.expire_command_equipment(14);
    assert_eq!(shared.waiting_for_tick[0].load(Ordering::Acquire), 1);
    let orders = take_orders(&shared, &uuid);
    assert!(matches!(orders.as_slice(), [Order::Clear, Order::CommandHold { .. }, Order::Give { .. }]));
    drop(orders);
    assert_eq!(shared.waiting_for_tick[0].load(Ordering::Acquire), 0);
}

#[test]
fn legacy_profile_bedrock_and_other_slots_keep_the_old_orders() {
    for profile in [Profile::Java26_1_2, Profile::Preview26_3] {
        let (shared, owner, mut observer, wake) = scene(profile);
        if profile == Profile::Preview26_3 {
            // После leave новый участник без Java waker соответствует старому
            // пути Bedrock: основной профиль сервера сам по себе недостаточен.
            shared.players.lock().unwrap().leave(&owner.uuid);
            shared.players.lock().unwrap().join(owner.member(), owner.entity_id).unwrap();
        }
        command(&mut observer, &shared, 1);
        let orders = take_orders(&shared, &owner.uuid);
        assert!(matches!(orders.as_slice(), [Order::Hold { .. }]));
        assert!(!shared.waiting_for_tick.iter().any(|v| v.load(Ordering::Acquire) != 0));
        let mut notified = std::pin::pin!(wake.notified());
        use std::future::Future;
        assert!(notified.as_mut().poll(&mut std::task::Context::from_waker(std::task::Waker::noop())).is_pending());
        commands::run("item replace entity Target hotbar.1 with stone", &Source::Console, &shared);
        assert!(matches!(take_orders(&shared, &owner.uuid).as_slice(), [Order::SetSlot { .. }]));
    }
}
