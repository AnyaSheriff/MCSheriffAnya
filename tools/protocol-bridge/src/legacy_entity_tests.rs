use super::*;

fn bytes(hex: &str) -> Vec<u8> {
    hex.as_bytes().as_chunks::<2>().0.iter().map(|pair|
        u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap()).collect()
}
fn fixture(name: &str) -> Vec<u8> {
    let data: serde_json::Value = serde_json::from_str(include_str!("../testdata/legacy-item-entities.json")).unwrap();
    bytes(data["entities"].as_array().unwrap().iter().find(|row| row["name"] == name).unwrap()["payload"].as_str().unwrap())
}
fn window() -> HeightWindow { HeightWindow::new(-64).unwrap() }
fn translate(tracker: &mut Tracker, id: u32, body: &[u8]) -> Result<Vec<Vec<u8>>, Error> {
    tracker.translate(id, body, window(), Some(46), &[])
}
fn spawn(y: f64) -> Vec<u8> {
    let recorded = fixture("spawn_entity");
    let mut out = recorded[..17].to_vec(); uint(&mut out, ITEM);
    for value in [978.5, y, -587.5] { out.extend(value.to_be_bytes()); }
    crate::entity_sync::push_velocity(&mut out, 0.01, 0.0, 0.0);
    out.extend([0, 0, 0, 0]); out
}
fn metadata(count: u8) -> Vec<u8> {
    let (mut stack, _) = crate::legacy_items::decode(&[0, 7, count, 0, 0, 0],
        crate::legacy_items::Context { enchantments: &[] }).unwrap();
    stack.as_mut().unwrap().count = u32::from(count);
    let mut out = vec![47, 0, 0, 0, 1, 1, 0, 2, 6, 0, 3, 8, 0, 4, 8, 0, 5, 8, 1, 8, 7];
    out.extend(crate::items::encode(crate::wire::INTERNAL, stack.as_ref(), false,
        crate::items::Context { shared_enchantment_count: Some(0) }).unwrap()); out.push(255); out
}
fn registered() -> Tracker {
    let mut state = Tracker::default(); translate(&mut state, 0x01, &spawn(-59.5)).unwrap(); state
}

#[test]
fn observed_item_spawn_properties_motion_collect_and_remove_match340() {
    let mut state = Tracker::default();
    let actual = translate(&mut state, 0x01, &spawn(-59.5)).unwrap();
    assert_eq!(actual, vec![[vec![0], fixture("spawn_entity")].concat()]);
    assert_eq!(translate(&mut state, 0x63, &metadata(3)).unwrap(),
        vec![[vec![0x3c], fixture("entity_metadata")].concat()]);
    // Точная форма нашего send_item_properties: только NoGravity и Stack.
    let normal = metadata(3); let sparse = [vec![47], normal[16..].to_vec()].concat();
    assert_eq!(translate(&mut state, 0x63, &sparse).unwrap(),
        vec![vec![0x3c,47,5,6,1,6,5,0,7,3,0,0,0,255]]);
    let gravity = [vec![47], normal[19..].to_vec()].concat();
    assert_eq!(translate(&mut state, 0x63, &gravity).unwrap(),
        vec![vec![0x3c,47,6,5,0,7,3,0,0,0,255]]);
    let mut speed = vec![47]; crate::entity_sync::push_velocity(&mut speed, 0.01, 0.0, 0.0);
    assert_eq!(translate(&mut state, 0x65, &speed).unwrap(), vec![[vec![0x3e], fixture("entity_velocity")].concat()]);
    assert_eq!(translate(&mut state, 0x7c, &fixture("collect")).unwrap(), vec![[vec![0x4b], fixture("collect")].concat()]);
    assert!(state.items.contains_key(&47));
    assert_eq!(translate(&mut state, 0x4d, &fixture("entity_destroy")).unwrap(),
        vec![[vec![0x32], fixture("entity_destroy")].concat()]);
    assert!(state.items.is_empty());
}

#[test]
fn partial_pickup_updates_stack_until_separate_remove_and_player_is_not_an_item() {
    let mut state = registered();
    for count in [3, 2, 1] {
        let data = translate(&mut state, 0x63, &metadata(count)).unwrap();
        let encoded = &data[0]; assert_eq!(&encoded[encoded.len()-9..], &[6, 5, 0, 7, count, 0, 0, 0, 255]);
        translate(&mut state, 0x7c, &[47, 46, 1]).unwrap(); assert!(state.items.contains_key(&47));
    }
    assert!(translate(&mut state, 0x7c, &[46, 47, 1]).is_err());
    assert!(translate(&mut state, 0x7c, &[47, 99, 1]).is_err());
    assert!(translate(&mut state, 0x7c, &[47, 46, 0]).is_err());
    assert!(translate(&mut state, 0x4d, &[2, 47, 46]).is_err());
    assert!(state.items.contains_key(&47));
    assert_eq!(translate(&mut state, 0x65, &[46, 0]).unwrap(), vec![vec![0x3e, 46, 0, 0, 0, 0, 0, 0]]);
    translate(&mut state, 0x4d, &[1, 47]).unwrap();
    translate(&mut state, 0x4d, &[1, 47]).unwrap();
    assert!(translate(&mut state, 0x63, &metadata(1)).is_err());
}

#[test]
fn invalid_truncated_and_extra_bytes_never_commit_entity_lifecycle() {
    let spawn = spawn(-59.5);
    for size in 0..spawn.len() {
        let mut state = Tracker::default(); assert!(translate(&mut state, 0x01, &spawn[..size]).is_err()); assert!(state.items.is_empty());
    }
    let mut state = registered(); let before = state.items.clone();
    for (id, mut payload) in [(0x01, spawn), (0x63, metadata(3)), (0x4d, vec![1, 47]), (0x7c, vec![47, 46, 1])] {
        payload.push(0); assert!(translate(&mut state, id, &payload).is_err()); assert_eq!(state.items, before);
    }
    assert!(translate(&mut state, 0x63, &[47, 5, 8, 1, 5, 8, 0, 255]).is_err());
    assert!(translate(&mut state, 0x63, &[47, 6, 20, 0, 255]).is_err());
    assert!(translate(&mut state, 0x63, &[47, 2, 6, 1]).is_err());
    assert!(translate(&mut state, 0x65, &[47, 8]).is_err());
    assert_eq!(state.items, before);
}

#[test]
fn bounds_unknown_kind_duplicate_spawn_and_outside_window_are_explicit() {
    let mut state = Tracker::default();
    for y in [-64.01, 192.0, f64::NAN] { assert!(translate(&mut state, 0x01, &spawn(y)).is_err()); assert!(state.items.is_empty()); }
    let mut other = spawn(0.0); other[17] = 1;
    assert!(translate(&mut state, 0x01, &other).is_err());
    translate(&mut state, 0x01, &spawn(-59.5)).unwrap();
    assert!(translate(&mut state, 0x01, &spawn(-59.5)).is_err());
    let pose = state.item(47).unwrap();
    Arc::make_mut(&mut state.items).extend((0..LIMIT as u32).map(|id| (id, pose)));
    let mut full = spawn(-59.5); full.splice(..1, [0xf0, 0xff, 7]);
    assert!(matches!(translate(&mut state, 0x01, &full), Err(Error::TooLarge)));
}

#[test]
fn relative_position_uses_server_coordinates_and_rejects_window_crossing_atomically() {
    let mut state = registered();
    let step = [47, 0x10, 0, 0x10, 0, 0xf0, 0, 1];
    assert_eq!(translate(&mut state, 0x35, &step).unwrap(), vec![[vec![0x26], step.to_vec()].concat()]);
    let pose = state.item(47).unwrap(); assert_eq!((pose.x, pose.y, pose.z), (979.5, -58.5, -588.5));
    let mut state = Tracker::default(); translate(&mut state, 0x01, &spawn(191.5)).unwrap();
    let before = state.items.clone(); assert!(translate(&mut state, 0x35, &step).is_err()); assert_eq!(state.items, before);
    let look = [47, 64, 192, 1]; assert_eq!(translate(&mut state, 0x38, &look).unwrap(), vec![vec![0x28,47,64,192,1]]);
}

#[test]
fn absolute_stop_emits_teleport_then_zero_motion_and_rejects_unrepresentable_speed() {
    let mut state = registered(); let mut sync = vec![47];
    for value in [12f64, -60.0, 20.0, 0.0, 0.0, 0.0] { sync.extend(value.to_be_bytes()); }
    sync.extend(90f32.to_be_bytes()); sync.extend((-90f32).to_be_bytes()); sync.push(1);
    let mut expected = vec![0x4c,47]; for value in [12f64,4.0,20.0] { expected.extend(value.to_be_bytes()); } expected.extend([64,192,1]);
    assert_eq!(translate(&mut state, 0x23, &sync).unwrap(), vec![expected,vec![0x3e,47,0,0,0,0,0,0]]);
    let before = state.items.clone(); sync[25..33].copy_from_slice(&4f64.to_be_bytes());
    assert!(translate(&mut state, 0x23, &sync).is_err()); assert_eq!(state.items, before);
}

fn respawn(dimension: &str) -> Vec<u8> {
    let mut body = vec![0x52, 0]; uint(&mut body, dimension.len() as u32); body.extend(dimension.as_bytes());
    body.extend([0;8]); body.extend([0,255,0,1,0,0,63,0]); body
}
#[test]
fn session_dispatch_and_dimension_reset_keep_existing_modern_tracker_semantics() {
    let mut session = crate::legacy_play::Session::new(window(), 2).unwrap();
    session.entity = Some(46); session.dimension = Some(0);
    session.clientbound(&[vec![1],spawn(-59.5)].concat(), &[]).unwrap();
    session.control(&respawn("minecraft:overworld")).unwrap();
    session.clientbound(&[vec![0x63],metadata(3)].concat(), &[]).unwrap();
    session.control(&respawn("minecraft:the_nether")).unwrap();
    assert!(session.clientbound(&[vec![0x63],metadata(3)].concat(), &[]).is_err());
    // Это регрессия выбранной семантики, не black-box доказательство340 respawn.
}

#[test]
fn control_snapshot_shares_tracker_until_a_valid_item_change() {
    let mut state = registered(); let snapshot = state.clone();
    assert!(Arc::ptr_eq(&state.items, &snapshot.items));
    assert!(translate(&mut state, 0x4d, &[1]).is_err());
    assert!(Arc::ptr_eq(&state.items, &snapshot.items));
    translate(&mut state, 0x4d, &[1,47]).unwrap();
    assert!(state.items.is_empty()); assert!(snapshot.items.contains_key(&47));
}

#[test]
fn relative_grid_matches_own_item_sender_at_negative_and_half_coordinates() {
    // src/network/play.rs::grid_of использует round(), а send_item_ticks
    // вычитает две такие сетки. Ниже явные пакеты для половинных координат.
    for (before, delta, expected) in [(0.5f64,1i16,2f64),(-0.5,-1,-2.0),(1.5,-3,-1.0),(-1.5,3,1.0)] {
        let mut state = Tracker::default(); let mut body = spawn(-59.5);
        body[18..26].copy_from_slice(&(before / 4096.0).to_be_bytes());
        translate(&mut state, 0x01, &body).unwrap();
        let mut step = vec![47]; step.extend(delta.to_be_bytes()); step.extend([0,0,0,0,1]);
        let result = translate(&mut state, 0x35, &step).unwrap();
        assert_eq!(state.item(47).unwrap().x, expected / 4096.0);
        assert_eq!(result[0][1..], step);
    }
}
