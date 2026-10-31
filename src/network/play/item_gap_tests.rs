//! Потерянный журнал: настоящая граница Items и пакеты777 в памяти.
use super::*;
use crate::stackdata;
use mcsa_protocol::{JavaVersion, ids::{self, RegistryKind}};

fn shared() -> Shared {
    let catalog = crate::gameplay_catalog::Catalog::new(crate::gameplay_catalog::Profile::Preview26_3);
    Shared::new(World::in_memory_with_catalog(catalog), World::in_memory_with_catalog(catalog),
        Default::default(), Default::default(), Default::default(),
        crate::ops::Ops::open(Path::new("tmp/item-gap-ops.json")), crate::skins::Settings::default())
}
fn out(shared: &Shared) -> Outgoing<'static> {
    Outgoing::capture_content(JavaVersion::V26_3, shared.catalog().content_codec())
}
fn wire_id(id: i32) -> i32 {
    ids::translate(RegistryKind::PlayClientbound, JavaVersion::V26_1_2, JavaVersion::V26_3, id as u32).unwrap() as i32
}
async fn packets(out: &Outgoing<'_>) -> Vec<(i32, Vec<u8>)> {
    let mut bytes = out.queued();
    let mut result = Vec::new();
    while !bytes.is_empty() {
        let packet = read_packet(&mut bytes).await.unwrap();
        result.push((packet.id, packet.payload().to_vec()));
    }
    result
}
fn entity_id(bytes: &[u8]) -> i32 { decode_varint(bytes).unwrap().0 }
fn f64_at(cursor: &mut Cursor<'_>) -> f64 {
    f64::from_be_bytes(cursor.bytes(8).unwrap().try_into().unwrap())
}
fn flood(items: &mut items::Items) {
    // Настоящий bounded журнал; не меняем LIMIT и не строим тысячи сущностей.
    for fuse in 0..65_540 {
        items.note_frame(items::Change::Fuse { entity_id: 900, fuse });
    }
}

#[tokio::test]
async fn gap_777_removes_ghost_and_restores_missing_current_state_without_stale_replay() {
    let shared = shared();
    let dimension = crate::dimension::Dimension::Overworld;
    let stack = Stack::new(blocks::item_named("stone").unwrap(), 9).with_extras(stackdata::Data {
        damage: 3, name: Some("Текущее имя".into()), ..Default::default()
    });
    let mut seen = SeenItems::new();
    let mut from;
    let expected;
    {
        let mut items = shared.items_in(dimension).lock().unwrap();
        from = items.watch_changes(1);
        items.summon_item(10, stack, (0.0, 64.0, 0.0), false);
        seen.insert(10, shown_item(&items.items()[0]));
        items.items_mut()[0].health = 0.0;
        items.remove_broken(); // Этот Gone потеряется: старый клиент хранит ghost10.
        items.prime(11, blocks::state_by_name("stone").unwrap(), (1.25, 65.5, -2.75), (0.12, -0.3, 0.04), 37, None);
        seen.insert(11, shown_item(&items.items()[0])); // Живой старый ID тоже reset.
        items.drop_block(12, blocks::state_by_name("sand").unwrap(), stack, (2.5, 70.0, 1.5));
        items.summon_item(13, stack, (3.5, 64.0, 1.5), false); // Spawn потеряется.
        flood(&mut items);
        // Реальный partial pickup пишет Gone -> Dropped с ТЕМ ЖЕ ID.
        items.take_from_first_inside(&|at| at.0 == 3.5, 2);
        items.note_frame(items::Change::Stack { entity_id: 13, stack: stack.with_count(99), no_gravity: true });
        items.summon_item(14, stack, (4.5, 64.0, 1.5), false);
        items.items_mut().iter_mut().find(|item| item.entity_id == 14).unwrap().health = 0.0;
        items.remove_broken(); // Tail-only Dropped -> Gone тоже не воспроизводится.
        items.note_frame(items::Change::Taken { entity_id: 13, by: 77, count: 99, remaining: false });
        items.note_frame(items::Change::Gone { entity_id: 987 }); // Неизвестное семейство оставлено прежнему пути.
        items.note_frame(items::Change::Taken { entity_id: 30, by: 77, count: 1, remaining: true }); // XP, не Dropped.
        for item in items.items_mut() {
            item.no_gravity = item.entity_id != 13;
            item.on_ground = item.entity_id != 11;
            item.vx = 0.125; item.vy = -0.25; item.vz = 0.375;
            item.ticks = 42;
        }
        expected = items.items().to_vec();
    }
    let mut carts = HashMap::new();
    carts.insert(20, ShownCart::of(&crate::minecart::Minecart::new(20, crate::minecart::Kind::Plain, (0.0, 64.0, 0.0))));
    let mut frames = HashMap::new();
    frames.insert(21, ShownFrame { pending: false, meta: None });
    let mut mobs = HashMap::new();
    let orb = crate::xp::XpOrb::new(30, 1, (0.0, 64.0, 0.0), (0.0, 0.0, 0.0));
    let mut wire = out(&shared);
    mobs.insert(30, mobs::send_orb(&mut wire, &orb).await.unwrap());
    let prefix = packets(&wire).await.len();
    send_pending_item_changes(&mut wire, &shared, dimension, &mut from, &mut seen,
        Known { carts: &mut carts, frames: &mut frames, mobs: &mut mobs, listener: (0.0, 64.0, 0.0) }, 1).await.unwrap();
    assert_eq!(seen.len(), 3);
    assert!(!seen.contains_key(&10));
    assert!(carts.contains_key(&20) && frames.contains_key(&21) && mobs.contains_key(&30));
    let packets: Vec<_> = packets(&wire).await.into_iter().skip(prefix).collect();
    let spawns: Vec<_> = packets.iter().filter(|(id, _)| *id == wire_id(ADD_ENTITY)).map(|(_, p)| entity_id(p)).collect();
    assert_eq!(spawns, [11, 12, 13]);
    let falling_spawn = packets.iter().find(|(id, p)| *id == wire_id(ADD_ENTITY) && entity_id(p) == 12).unwrap();
    let mut falling = Cursor::new(&falling_spawn.1);
    falling.varint(); assert_eq!(falling.bytes(16), Some(expected[1].uuid.as_slice()));
    let kind = ids::translate(RegistryKind::Entity, JavaVersion::V26_1_2, JavaVersion::V26_3, FALLING_BLOCK_ENTITY_TYPE as u32).unwrap();
    assert_eq!(falling.varint(), Some(kind as i32));
    for value in [expected[1].x, expected[1].y, expected[1].z] { assert_eq!(f64_at(&mut falling), value); }
    assert_eq!(falling.u8(), Some(0), "старый Spawn блока действительно не передаёт текущую скорость");
    assert_eq!(falling.bytes(3), Some([0,0,0].as_slice()));
    let state = ids::translate(RegistryKind::BlockState, JavaVersion::V26_1_2, JavaVersion::V26_3, expected[1].block.unwrap() as u32).unwrap();
    assert_eq!(falling.varint(), Some(state as i32)); assert_eq!(falling.offset, falling.bytes.len());
    let destroys: Vec<_> = packets.iter().filter(|(id, _)| *id == wire_id(REMOVE_ENTITIES)).map(|(_, p)| {
        let mut c = Cursor::new(p); assert_eq!(c.varint(), Some(1)); c.varint().unwrap()
    }).collect();
    assert_eq!(destroys, [10, 11, 987]);
    assert!(packets[..2].iter().all(|(id, _)| *id == wire_id(REMOVE_ENTITIES)), "сначала весь старый набор");
    let pickups: Vec<_> = packets.iter().filter(|(id, _)| *id == wire_id(PICK_UP_ITEM)).map(|(_, p)| entity_id(p)).collect();
    assert_eq!(pickups, [30], "Taken предмета устарел; XP остаётся прежним");
    for item in &expected {
        let teleports: Vec<_> = packets.iter().filter(|(id, p)| *id == wire_id(TELEPORT_ENTITY) && entity_id(p) == item.entity_id).collect();
        assert_eq!(teleports.len(), 1);
        let mut c = Cursor::new(&teleports[0].1);
        assert_eq!(c.varint(), Some(item.entity_id));
        assert_eq!(c.varint(), Some(0), "777 linear positionType");
        for expected in [item.x, item.y, item.z] { assert_eq!(f64_at(&mut c), expected); }
        assert_eq!(c.f32(), Some(0.0)); assert_eq!(c.f32(), Some(0.0));
        assert_eq!(c.u8(), Some(u8::from(item.on_ground))); assert_eq!(c.offset, c.bytes.len());
        // В777 наш converter делит absolute sync на позицию и Motion.
        // Проверяем самостоятельный пакет скорости, включая точный LPVec3.
        let motion: Vec<_> = packets.iter().filter(|(id, p)| *id == wire_id(0x65) && entity_id(p) == item.entity_id).collect();
        assert_eq!(motion.len(), 1);
        let mut expected_motion = encode_varint(item.entity_id);
        types::push_velocity(&mut expected_motion, item.vx, item.vy, item.vz);
        assert_eq!(motion[0].1, expected_motion);
        let mut speed = Cursor::new(&motion[0].1);
        assert_eq!(speed.varint(), Some(item.entity_id));
        let first = speed.u8().unwrap();
        assert_ne!(first, 0, "ненулевая текущая скорость не заменяется нулевой из Falling Spawn");
        let second = speed.u8().unwrap();
        let packed = u64::from(first) | (u64::from(second) << 8) | (u64::from(speed.u32().unwrap()) << 16);
        let extra = if first & 4 != 0 { speed.varint().unwrap() as u64 } else { 0 };
        let scale = u64::from(first & 3) + (extra << 2);
        assert!(scale > 0);
        for (index, expected) in [item.vx, item.vy, item.vz].into_iter().enumerate() {
            let part = (packed >> (3 + 15 * index)) & 0x7fff;
            assert!(part <= 32766);
            let decoded = (part as f64 / 32766.0 * 2.0 - 1.0) * scale as f64;
            assert!((decoded - expected).abs() <= scale as f64 / 32766.0, "LPVec3 component {index}: {decoded} != {expected}");
        }
        assert_eq!(speed.offset, speed.bytes.len());
        let metadata: Vec<_> = packets.iter().filter(|(id, p)| *id == wire_id(SET_ENTITY_DATA) && entity_id(p) == item.entity_id).collect();
        assert!(metadata.iter().any(|(_, p)| {
            let (_, n) = decode_varint(p).unwrap(); p[n..].starts_with(&[5, 8, u8::from(item.no_gravity)])
        }), "NoGravity передан явно для всех трёх видов");
        if item.entity_id == 11 {
            let mut c = Cursor::new(&metadata.last().unwrap().1);
            c.varint(); assert_eq!(c.u8(), Some(5)); assert_eq!(c.varint(), Some(8)); assert_eq!(c.u8(), Some(1));
            assert_eq!(c.u8(), Some(TNT_FUSE_PROPERTY)); assert_eq!(c.varint(), Some(PROPERTY_INT)); assert_eq!(c.varint(), Some(37));
            assert_eq!(c.u8(), Some(TNT_BLOCK_PROPERTY)); assert_eq!(c.varint(), Some(PROPERTY_BLOCK_STATE));
            let state = ids::translate(RegistryKind::BlockState, JavaVersion::V26_1_2, JavaVersion::V26_3, item.block.unwrap() as u32).unwrap();
            assert_eq!(c.varint(), Some(state as i32)); assert_eq!(c.u8(), Some(PROPERTIES_END));
        }
        assert!(seen[&item.entity_id].first_sent);
    }
    // Current stack/extras пришли общим полноценным кодеком, а не stale Stack.
    let mut properties = out(&shared);
    send_dropped_item(&mut properties, &expected[2]).await.unwrap();
    let property_packets = self::packets(&properties).await;
    let expected_properties = property_packets.iter().find(|(id, _)| *id == wire_id(SET_ENTITY_DATA)).unwrap();
    assert!(packets.contains(expected_properties));
    let mut next = out(&shared);
    send_pending_item_changes(&mut next, &shared, dimension, &mut from, &mut seen,
        Known { carts: &mut carts, frames: &mut frames, mobs: &mut mobs, listener: (0.0,64.0,0.0) }, 1).await.unwrap();
    assert!(next.queued().is_empty(), "tail не воспроизводится повторно");
    send_item_moves(&mut next, &shared, dimension, &mut seen).await.unwrap();
    assert!(next.queued().is_empty(), "shown cache соответствует уже отправленной полной позиции");
}

#[tokio::test]
async fn normal_777_drop_is_byte_identical_and_legacy_gap_does_not_reset() {
    let shared = shared(); let dimension = crate::dimension::Dimension::Overworld;
    let mut from = shared.items_in(dimension).lock().unwrap().watch_changes(1);
    let item = {
        let mut items = shared.items_in(dimension).lock().unwrap();
        items.prime(81, blocks::state_by_name("tnt").unwrap(), (0.0,64.0,0.0), (0.1,0.2,0.3), 80, None);
        items.items()[0].clone()
    };
    let mut expected = out(&shared); send_dropped_item(&mut expected, &item).await.unwrap();
    let mut actual = out(&shared); let mut seen = SeenItems::new();
    let (mut carts, mut frames, mut mobs) = (HashMap::new(), HashMap::new(), HashMap::new());
    send_pending_item_changes(&mut actual, &shared, dimension, &mut from, &mut seen,
        Known { carts: &mut carts, frames: &mut frames, mobs: &mut mobs, listener: (0.0,64.0,0.0) }, 1).await.unwrap();
    assert_eq!(actual.queued(), expected.queued());
    let stale = from;
    flood(&mut shared.items_in(dimension).lock().unwrap());
    let mut legacy = Outgoing::capture(); from = stale;
    send_pending_item_changes(&mut legacy, &shared, dimension, &mut from, &mut seen,
        Known { carts: &mut carts, frames: &mut frames, mobs: &mut mobs, listener: (0.0,64.0,0.0) }, 1).await.unwrap();
    assert!(legacy.queued().is_empty(), "legacy получает старый tail, но не snapshot reset");
    assert!(seen.contains_key(&81));
}

#[tokio::test]
async fn identity_collision_rejects_before_any_recovery_packet() {
    let dimension = crate::dimension::Dimension::Overworld;
    for tail_collision in [false, true] {
        let shared = shared();
        let mut seen = SeenItems::new();
        let mut from;
        {
            let mut items = shared.items_in(dimension).lock().unwrap();
            from = items.watch_changes(7);
            items.summon_item(81, Stack::new(blocks::item_named("stone").unwrap(),1), (0.0,64.0,0.0), false);
            flood(&mut items);
            if tail_collision { items.note_frame(items::Change::Frame(crate::frame::Frame::new(81, (0,64,0), 2, false))); }
        }
        let mut carts = HashMap::new(); let mut frames = HashMap::new(); let mut mobs = HashMap::new();
        if !tail_collision { frames.insert(81, ShownFrame { pending: false, meta: None }); }
        let mut wire = out(&shared);
        let result = send_pending_item_changes(&mut wire, &shared, dimension, &mut from, &mut seen,
            Known { carts: &mut carts, frames: &mut frames, mobs: &mut mobs, listener: (0.0,64.0,0.0) }, 7).await;
        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::InvalidData);
        assert!(wire.queued().is_empty()); assert!(seen.is_empty());
    }
}

#[tokio::test]
async fn interrupted_recovery_returns_error_and_keeps_only_completed_shown_entries() {
    let mut items = items::Items::new();
    items.drop_item(1, Stack::new(blocks::item_named("stone").unwrap(),1), (0.0,64.0,0.0), (0.1,0.2,0.3), 0);
    let current: Vec<_> = (1..=2000).map(|entity_id| {
        let mut item = items.items()[0].clone(); item.entity_id = entity_id; item.uuid = items::uuid_for(entity_id); item
    }).collect();
    let (mut socket, peer) = tokio::io::duplex(1);
    drop(peer); // Следующий автоматический flush большого reset прервётся.
    let mut wire = Outgoing::duplex(&mut socket);
    let mut seen = SeenItems::new();
    let result = recover_dropped(&mut wire, &mut seen, &[], &current).await;
    assert!(result.is_err(), "прерванное восстановление не возвращает успешный набор IDs");
    assert!(!seen.is_empty() && seen.len() < current.len());
    assert_eq!(wire.flush().await.unwrap_err().kind(), io::ErrorKind::BrokenPipe,
        "после частичной записи поток нельзя продолжать");
}
