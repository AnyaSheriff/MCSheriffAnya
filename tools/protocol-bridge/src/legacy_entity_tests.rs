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
    assert!(state.entities.contains_key(&47));
    assert_eq!(translate(&mut state, 0x4d, &fixture("entity_destroy")).unwrap(),
        vec![[vec![0x32], fixture("entity_destroy")].concat()]);
    assert!(state.entities.is_empty());
}

#[test]
fn partial_pickup_updates_stack_until_separate_remove_and_player_is_not_an_item() {
    let mut state = registered();
    for count in [3, 2, 1] {
        let data = translate(&mut state, 0x63, &metadata(count)).unwrap();
        let encoded = &data[0]; assert_eq!(&encoded[encoded.len()-9..], &[6, 5, 0, 7, count, 0, 0, 0, 255]);
        translate(&mut state, 0x7c, &[47, 46, 1]).unwrap(); assert!(state.entities.contains_key(&47));
    }
    assert!(translate(&mut state, 0x7c, &[46, 47, 1]).is_err());
    assert!(translate(&mut state, 0x7c, &[47, 99, 1]).is_err());
    assert!(translate(&mut state, 0x7c, &[47, 46, 0]).is_err());
    assert!(translate(&mut state, 0x4d, &[2, 47, 46]).is_err());
    assert!(state.entities.contains_key(&47));
    assert_eq!(translate(&mut state, 0x65, &[46, 0]).unwrap(), vec![vec![0x3e, 46, 0, 0, 0, 0, 0, 0]]);
    translate(&mut state, 0x4d, &[1, 47]).unwrap();
    translate(&mut state, 0x4d, &[1, 47]).unwrap();
    assert!(translate(&mut state, 0x63, &metadata(1)).is_err());
}

#[test]
fn invalid_truncated_and_extra_bytes_never_commit_entity_lifecycle() {
    let spawn = spawn(-59.5);
    for size in 0..spawn.len() {
        let mut state = Tracker::default(); assert!(translate(&mut state, 0x01, &spawn[..size]).is_err()); assert!(state.entities.is_empty());
    }
    let mut state = registered(); let before = state.entities.clone();
    for (id, mut payload) in [(0x01, spawn), (0x63, metadata(3)), (0x4d, vec![1, 47]), (0x7c, vec![47, 46, 1])] {
        payload.push(0); assert!(translate(&mut state, id, &payload).is_err()); assert_eq!(state.entities, before);
    }
    assert!(translate(&mut state, 0x63, &[47, 5, 8, 1, 5, 8, 0, 255]).is_err());
    assert!(translate(&mut state, 0x63, &[47, 6, 20, 0, 255]).is_err());
    assert!(translate(&mut state, 0x63, &[47, 2, 6, 1]).is_err());
    assert!(translate(&mut state, 0x65, &[47, 8]).is_err());
    assert_eq!(state.entities, before);
}

#[test]
fn bounds_unknown_kind_duplicate_spawn_and_outside_window_are_explicit() {
    let mut state = Tracker::default();
    for y in [-64.01, 192.0, f64::NAN] { assert!(translate(&mut state, 0x01, &spawn(y)).is_err()); assert!(state.entities.is_empty()); }
    let mut other = spawn(0.0); other[17] = 1;
    assert!(translate(&mut state, 0x01, &other).is_err());
    translate(&mut state, 0x01, &spawn(-59.5)).unwrap();
    assert!(translate(&mut state, 0x01, &spawn(-59.5)).is_err());
    let tracked = state.tracked(47).unwrap();
    Arc::make_mut(&mut state.entities).extend((0..LIMIT as u32).map(|id| (id, tracked)));
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
    let before = state.entities.clone(); assert!(translate(&mut state, 0x35, &step).is_err()); assert_eq!(state.entities, before);
    let look = [47, 64, 192, 1]; assert_eq!(translate(&mut state, 0x38, &look).unwrap(), vec![vec![0x28,47,64,192,1]]);
}

#[test]
fn absolute_stop_emits_teleport_then_zero_motion_and_rejects_unrepresentable_speed() {
    let mut state = registered(); let mut sync = vec![47];
    for value in [12f64, -60.0, 20.0, 0.0, 0.0, 0.0] { sync.extend(value.to_be_bytes()); }
    sync.extend(90f32.to_be_bytes()); sync.extend((-90f32).to_be_bytes()); sync.push(1);
    let mut expected = vec![0x4c,47]; for value in [12f64,4.0,20.0] { expected.extend(value.to_be_bytes()); } expected.extend([64,192,1]);
    assert_eq!(translate(&mut state, 0x23, &sync).unwrap(), vec![expected,vec![0x3e,47,0,0,0,0,0,0]]);
    let before = state.entities.clone(); sync[25..33].copy_from_slice(&4f64.to_be_bytes());
    assert!(translate(&mut state, 0x23, &sync).is_err()); assert_eq!(state.entities, before);
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
    assert!(Arc::ptr_eq(&state.entities, &snapshot.entities));
    assert!(translate(&mut state, 0x4d, &[1]).is_err());
    assert!(Arc::ptr_eq(&state.entities, &snapshot.entities));
    translate(&mut state, 0x4d, &[1,47]).unwrap();
    assert!(state.entities.is_empty()); assert!(snapshot.entities.contains_key(&47));
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

fn cow_fixture() -> serde_json::Value {
    serde_json::from_str(include_str!("../testdata/legacy-cow-observations.json")).unwrap()
}
fn cow_event(stage: &str, name: &str) -> serde_json::Value {
    cow_fixture()["events"].as_array().unwrap().iter()
        .find(|row| row["stage"] == stage && row["name"] == name).unwrap().clone()
}
fn cow_spawn(stage: &str) -> Vec<u8> {
    let row = cow_event(stage, "spawn_entity_living");
    let mut out = Vec::new(); uint(&mut out, row["entity"].as_u64().unwrap() as u32);
    out.extend(bytes(row["uuid"].as_str().unwrap())); uint(&mut out, COW);
    for (index, value) in row["position"].as_array().unwrap().iter().enumerate() {
        let value = value.as_f64().unwrap() - if index == 1 { 64.0 } else { 0.0 };
        out.extend(value.to_be_bytes());
    }
    out.push(0); // Native LPVec3: наблюдённая нулевая начальная скорость.
    let angles = row["angles"].as_array().unwrap();
    out.extend([angles[1].as_u64().unwrap() as u8, angles[0].as_u64().unwrap() as u8,
                angles[2].as_u64().unwrap() as u8, 0]);
    out
}
fn cow_metadata(stage: &str) -> (Vec<u8>, Vec<u8>) {
    let row = cow_event(stage, "spawn_entity_living");
    let entity = row["entity"].as_u64().unwrap() as u32;
    let mut native = Vec::new(); uint(&mut native, entity);
    let mut legacy = vec![0x3c]; uint(&mut legacy, entity);
    for field in row["fields"].as_array().unwrap() {
        let index = field["index"].as_u64().unwrap() as u8;
        let value = &field["value"];
        match index {
            4 => { native.extend([4, 8, value.as_u64().unwrap() as u8]); legacy.extend([4, 6, value.as_u64().unwrap() as u8]); }
            7 => {
                native.extend([9, 3]); legacy.extend([7, 2]);
                let value = (value.as_f64().unwrap() as f32).to_be_bytes();
                native.extend(value); legacy.extend(value);
            }
            11 => { native.extend([15, 0, value.as_u64().unwrap() as u8]); legacy.extend([11, 0, value.as_u64().unwrap() as u8]); }
            12 => { native.extend([16, 8, value.as_u64().unwrap() as u8]); legacy.extend([12, 6, value.as_u64().unwrap() as u8]); }
            _ => {}
        }
    }
    native.push(255); legacy.push(255); (native, legacy)
}

#[test]
fn cow_spawn_then_sparse_metadata_matches_observation_and_actual_client_order() {
    for stage in ["adult-spawn", "baby-spawn", "falling-spawn"] {
        let mut state = Tracker::default();
        let recorded = cow_event(stage, "spawn_entity_living");
        let mut expected = bytes(recorded["payload"].as_str().unwrap());
        let metadata_len = bytes(recorded["metadata"].as_str().unwrap()).len();
        expected.truncate(expected.len() - metadata_len); expected.push(255);
        assert_eq!(translate(&mut state, 1, &cow_spawn(stage)).unwrap(),
                   vec![[vec![3], expected].concat()]);
        let (native, legacy) = cow_metadata(stage);
        assert_eq!(translate(&mut state, 0x63, &native).unwrap(), vec![legacy]);
    }
    let mut state = Tracker::default();
    translate(&mut state, 1, &cow_spawn("adult-spawn")).unwrap();
    let body = [vec![47, 9, 3], 4f32.to_be_bytes().to_vec(), vec![255]].concat();
    let observed = cow_event("health4", "entity_metadata");
    assert_eq!(translate(&mut state, 0x63, &body).unwrap(),
               vec![[vec![0x3c], bytes(observed["payload"].as_str().unwrap())].concat()]);
}

#[test]
fn cow_head_relative_and_absolute_packets_preserve_shared_movement() {
    let mut state = Tracker::default();
    translate(&mut state, 1, &cow_spawn("adult-spawn")).unwrap();
    let head = bytes(cow_event("turn", "entity_head_rotation")["payload"].as_str().unwrap());
    assert_eq!(translate(&mut state, 0x53, &head).unwrap(), vec![[vec![0x36], head].concat()]);
    let old = cow_event("teleport", "entity_teleport");
    let mut sync = vec![47];
    for value in [-6.5f64, 0.0, 0.5, 0.0, 0.0, 0.0] { sync.extend(value.to_be_bytes()); }
    sync.extend(180f32.to_be_bytes()); sync.extend(0f32.to_be_bytes()); sync.push(1);
    assert_eq!(translate(&mut state, 0x23, &sync).unwrap(),
               vec![[vec![0x4c], bytes(old["payload"].as_str().unwrap())].concat(),
                    vec![0x3e,47,0,0,0,0,0,0]]);
    translate(&mut state, 1, &cow_spawn("falling-spawn")).unwrap();
    let old = cow_fixture()["events"].as_array().unwrap().iter()
        .find(|r| r["name"] == "rel_entity_move" && r["entity"] == 55
              && r["delta"].as_array().unwrap().iter().any(|v| v.as_i64().unwrap() != 0)).unwrap().clone();
    let body = bytes(old["payload"].as_str().unwrap());
    assert_eq!(translate(&mut state, 0x35, &body).unwrap(), vec![[vec![0x26], body].concat()]);
    assert_eq!(state.tracked(55).unwrap().pose.y, 6.0 - 1902.0 / 4096.0);
    // MoveLook — проверка pinned JSON/собственного sender, НЕ живая запись340.
    let move_look = [47,0,0,0,0,0,0,64,0,1];
    assert_eq!(translate(&mut state, 0x36, &move_look).unwrap(), vec![[vec![0x27],move_look.to_vec()].concat()]);
}

#[test]
fn cow_and_item_roles_do_not_cross_collect_head_or_metadata() {
    let mut state = registered();
    translate(&mut state, 1, &cow_spawn("baby-spawn")).unwrap();
    assert!(state.living(47).is_err()); assert!(state.living(48).is_ok());
    assert!(translate(&mut state, 0x53, &[47,64]).is_err());
    assert!(translate(&mut state, 0x53, &[46,64]).is_err());
    assert!(translate(&mut state, 0x53, &[99,64]).is_err());
    assert!(translate(&mut state, 0x7c, &[48,46,1]).is_err());
    assert!(translate(&mut state, 0x7c, &[47,48,1]).is_err());
    let mut item_fields = metadata(1); item_fields[0] = 48;
    assert!(translate(&mut state, 0x63, &item_fields).is_err());
    assert!(translate(&mut state, 0x63, &[47,16,8,1,255]).is_err());
    assert_eq!(translate(&mut state, 0x65, &[46,0]).unwrap(),
               vec![vec![0x3e,46,0,0,0,0,0,0]]);
    assert_eq!(translate(&mut state, 0x7c, &[47,46,1]).unwrap(), vec![vec![0x4b,47,46,1]]);
}

#[test]
fn cow_unknown_fields_bits_values_and_packet_tails_are_explicit_and_atomic() {
    let mut state = Tracker::default();
    let spawn = cow_spawn("adult-spawn");
    for kind in [100,150] {
        let mut other = spawn.clone(); let mut encoded = Vec::new(); uint(&mut encoded,kind);
        other.splice(17..18,encoded);
        assert!(translate(&mut state,1,&other).is_err()); assert!(state.entities.is_empty());
    }
    for len in 0..spawn.len() {
        assert!(translate(&mut state, 1, &spawn[..len]).is_err());
        assert!(state.entities.is_empty());
    }
    translate(&mut state, 1, &spawn).unwrap();
    let before = state.entities.clone();
    for fields in [
        vec![6,20,7], vec![10,17,1], vec![12,1,1], vec![17,8,1],
        vec![18,23,0], vec![19,24,0], vec![15,0,2], vec![15,0,4],
        vec![16,8,2], vec![2,6,1], vec![9,2,0], vec![16,8,1,16,8,0],
    ] {
        let body = [vec![47],fields,vec![255]].concat();
        assert!(translate(&mut state, 0x63, &body).is_err(), "{body:?}");
        assert_eq!(state.entities, before);
    }
    for value in [f32::NAN, f32::INFINITY, -1.0] {
        let body = [vec![47,9,3],value.to_be_bytes().to_vec(),vec![255]].concat();
        assert!(translate(&mut state, 0x63, &body).is_err());
    }
    let (metadata, _) = cow_metadata("adult-spawn");
    for len in 0..metadata.len() { assert!(translate(&mut state, 0x63, &metadata[..len]).is_err()); }
    for (id, mut body) in [(0x53,vec![47,64]),(0x4d,vec![1,47]),(0x63,metadata)] {
        body.push(0); assert!(translate(&mut state, id, &body).is_err());
        assert_eq!(state.entities, before);
    }
    let mut edge = cow_spawn("baby-spawn"); edge[26..34].copy_from_slice(&192f64.to_be_bytes());
    assert!(translate(&mut state, 1, &edge).is_err()); assert_eq!(state.entities, before);
    translate(&mut state, 0x4d, &[1,47]).unwrap();
    assert!(state.living(47).is_err());
    translate(&mut state, 1, &spawn).unwrap(); assert!(state.living(47).is_ok());
}

#[test]
fn session_cow_attributes_use_existing_codec_and_reject_item_or_unknown_targets() {
    let mut session = crate::legacy_play::Session::new(window(), 2).unwrap();
    session.entity = Some(46); session.dimension = Some(0);
    session.clientbound(&[vec![1],cow_spawn("adult-spawn")].concat(), &[]).unwrap();
    let attribute = |entity| {
        let mut packet = Vec::new(); uint(&mut packet,0x83); uint(&mut packet,entity);
        packet.extend([1,22]); packet.extend(0.31f64.to_be_bytes()); packet.push(0); packet
    };
    let body = attribute(47);
    let observed = cow_fixture()["attributes"].as_array().unwrap().iter()
        .find(|r| r["stage"] == "speed031").unwrap().clone();
    assert_eq!(session.clientbound(&body,&[]).unwrap(),
               vec![[vec![0x4e],bytes(observed["payload"].as_str().unwrap())].concat()]);
    assert_eq!(session.clientbound(&[0x53,47,64],&[]).unwrap(), vec![vec![0x36,47,64]]);
    let self_body = attribute(46);
    assert!(session.clientbound(&self_body,&[]).is_ok());
    assert_eq!(session.clientbound(&[0x63,46,0,0,2,255],&[]).unwrap(),
               vec![vec![0x3c,46,0,0,2,255]]);
    assert!(session.clientbound(&[0x53,46,64],&[]).is_err());
    let mut item_spawn = spawn(-59.5); item_spawn[0] = 48;
    session.clientbound(&[vec![1],item_spawn].concat(),&[]).unwrap();
    for entity in [48,99] {
        let wrong = attribute(entity);
        assert!(session.clientbound(&wrong,&[]).is_err());
    }
    assert!(session.clientbound(&[0x53,48,64],&[]).is_err());
    session.control(&respawn("minecraft:overworld")).unwrap();
    assert!(session.clientbound(&body,&[]).is_ok());
    session.control(&respawn("minecraft:the_nether")).unwrap();
    assert!(session.clientbound(&body,&[]).is_err());
    assert!(session.clientbound(&[0x53,47,64],&[]).is_err());
    assert!(session.clientbound(&self_body,&[]).is_ok());
}

#[test]
fn cow_full_own_property_shapes_filter_initial_defaults_and_accept_only_zero_resets() {
    let source: serde_json::Value = serde_json::from_str(
        include_str!("../testdata/legacy-cow-native-shapes.json")).unwrap();
    let mut state = Tracker::default();
    translate(&mut state,1,&cow_spawn("adult-spawn")).unwrap();
    for case in source["cases"].as_array().unwrap() {
        let initial = bytes(case["initial_filtered_body"].as_str().unwrap());
        assert_eq!(translate(&mut state,0x63,&initial).unwrap(),
                   vec![bytes(case["expected_initial_packet"].as_str().unwrap())]);
        // Полный properties() включает variants, которые send_mob сейчас
        // всегда отфильтровывает. Не объявляем их поддержанными на проводе.
        let full = bytes(case["all_properties_body"].as_str().unwrap());
        assert!(translate(&mut state,0x63,&full).is_err());
        let mut known = vec![47];
        for field in case["properties"].as_array().unwrap() {
            let index = field["index"].as_u64().unwrap() as u8;
            if matches!(index,18 | 19) { continue; }
            known.extend([index,field["serializer"].as_u64().unwrap() as u8]);
            known.extend(bytes(field["value_hex"].as_str().unwrap()));
        }
        known.push(255);
        assert!(translate(&mut state,0x63,&known).is_ok());
    }
    let reset = bytes(source["zero_reset_body"].as_str().unwrap());
    assert_eq!(translate(&mut state,0x63,&reset).unwrap(),
               vec![bytes(source["expected_zero_reset_packet"].as_str().unwrap())]);
    for len in 0..reset.len() { assert!(translate(&mut state,0x63,&reset[..len]).is_err()); }
    for field in [vec![6,20,7],vec![10,17,1],vec![12,1,1],vec![17,8,1],vec![17,8,2]] {
        assert!(translate(&mut state,0x63,&[vec![47],field,vec![255]].concat()).is_err());
    }
    assert!(translate(&mut state,0x63,&[47,6,20,0,6,20,0,255]).is_err());
    assert_eq!(state.entities.len(),1);
}
