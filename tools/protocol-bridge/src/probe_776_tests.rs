use super::*;
use crate::{ids::{self, RegistryKind}, login, Negotiated};
use JavaVersion::{V26_1_2 as OLD, V26_2 as MID, V26_3 as NEW};

#[test]
fn feature_does_not_enable_general_776_routes() {
    for version in [OLD, MID, NEW] {
        assert!(wire::translate(State::Play, Direction::Clientbound, version, MID, &[0x69, 0]).is_err());
        assert!(Tracker::default().translate(version, MID, &[0x69, 0], Context::default()).is_err());
    }
    assert!(ids::translate(RegistryKind::PlayClientbound, OLD, MID, 0x69).is_err());
    assert!(Negotiated::native_only(776, OLD).is_err());
    assert!(wire::translate_inner(State::Play, Direction::Clientbound,
        NEW, MID, &[0x69, 0], Context::default(), true).is_err());
}

#[test]
fn allowed_controls_and_explicitly_unsupported_payloads_are_distinct() {
    assert_eq!(configuration(&[3]).unwrap().as_ref(), &[3]);
    assert_eq!(clientbound(&mut Tracker::default(), &[0x69, 0], Context::default()).unwrap().as_ref(), &[0x69, 0]);
    assert_eq!(serverbound(&[0x35, 0, 2], Context::default()).unwrap().as_ref(), &[0x35, 0, 2]);
    // ACK нужен typed transport, а не непрозрачный passthrough.
    for body in [&[0, 1][..], &[0x38, 0, 0], &[0x42, 0], &[0x3e, 0]] {
        assert!(serverbound(body, Context::default()).is_err());
    }
    for body in [&[0x2f, 0][..], &[0x6d, 0], &[0x06, 0], &[0x01, 0], &[0x34, 0]] {
        assert!(clientbound(&mut Tracker::default(), body, Context::default()).is_err());
    }
    assert!(configuration(&[7, 0]).is_err()); // RegistryData только own target builder.
}

#[test]
fn join_establishes_self_kind_only_after_complete_conversion() {
    let raw = include_bytes!("../testdata/play-login-26.1.2.bin");
    let context = Context { online_mode: Some(false), ..Default::default() };
    let mut tracker = Tracker::default();
    let self_flags = [0x63, 1, 0, 0, 0, 255];
    assert!(clientbound(&mut tracker, &self_flags, context).is_err());
    assert!(clientbound(&mut tracker, &raw[..raw.len() - 1], context).is_err());
    assert!(clientbound(&mut tracker, &self_flags, context).is_err());
    let join = clientbound(&mut tracker, raw, context).unwrap().into_owned();
    assert_eq!(join.len(), raw.len() + 1);
    assert_eq!(clientbound(&mut tracker, &self_flags, context).unwrap().as_ref(), self_flags);
    // Entity metadata уже известного игрока не должна открывать serializer44.
    assert!(clientbound(&mut tracker, &[0x63, 1, 0, 44, 0, 255], context).is_err());
}

#[test]
fn bounded_advancement_layout_776_keeps_coordinates_inside_display() {
    // Один показанный собственный advancement, простые NBT строки и iconTemplate.
    let mut body = vec![0x82, 1, 1, 1, 1, b'x', 0, 1];
    for text in [b"T", b"D"] { body.extend([8, 0, 1]); body.extend(text); }
    body.extend([1, 1, 0, 0, 0]); // stoneTemplate + frame0
    body.extend([0; 4]); // no flags/background
    body.extend(1.25f32.to_be_bytes()); body.extend(2.5f32.to_be_bytes());
    body.extend([0, 0, 0, 0, 0]); // requirements, telemetry, removed, progress, show
    assert_eq!(clientbound(&mut Tracker::default(), &body, Context::default()).unwrap().as_ref(), body);
    for end in 0..body.len() {
        assert!(clientbound(&mut Tracker::default(), &body[..end], Context::default()).is_err());
    }
    // Login API не заменяет transport session UUID: отдельная required envelope.
    let success = login::Success { uuid: [1; 16], username: "Probe".into(), properties: vec![], session_id: None };
    assert!(success.encode(MID).is_err());
}

#[path = "probe_776_item_tests.rs"]
mod item_lifecycle;

#[test]
fn gameplay_776_actions_are_bounded_and_do_not_enable_unknown_dig_enums() {
    let mut placement = vec![0x42, 0];
    placement.extend([0;8]); placement.push(1);
    for value in [0.5f32, 1.0, 0.5] { placement.extend(value.to_be_bytes()); }
    placement.extend([0,0,1]);
    for body in [vec![1,7],vec![0x0c,0],vec![0x1a,7,0,0,0],placement] {
        assert_eq!(serverbound(&body,Context::default()).unwrap().as_ref(),body);
        for end in 0..body.len() { assert!(serverbound(&body[..end],Context::default()).is_err()); }
        let mut tail=body;tail.push(0);assert!(serverbound(&tail,Context::default()).is_err());
    }
    assert!(serverbound(&[0x0c,3],Context::default()).is_err());
    // Полный enum776 снят GUI; неизвестное действие8 остаётся закрыто.
    assert!(serverbound(&[0x29,8,0,0,0,0,0,0,0,0,1,1],Context::default()).is_err());
}

#[test]
fn gameplay_776_player_spawn_movement_and_removal_keep_tracking_consistent() {
    let mut tracker=Tracker::default();
    let mut spawn=vec![1,7];spawn.extend([0;16]);spawn.extend([155,1]);
    for value in [0.5f64,64.0,0.5] { spawn.extend(value.to_be_bytes()); }
    spawn.extend([0;5]);
    let converted=clientbound(&mut tracker,&spawn,Context::default()).unwrap().into_owned();
    assert_eq!(&converted[18..20],&[156,1]);
    let meta=[0x63,7,9,3,0x41,0xa0,0,0,255];
    assert_eq!(clientbound(&mut tracker,&meta,Context::default()).unwrap().as_ref(),meta);
    let mut movement=vec![0x23,7];movement.extend([0;56]);movement.push(1);
    assert_eq!(clientbound(&mut tracker,&movement,Context::default()).unwrap().as_ref(),movement);
    for end in 0..movement.len() { assert!(clientbound(&mut tracker,&movement[..end],Context::default()).is_err()); }
    let mut invalid=movement.clone();invalid[2..10].copy_from_slice(&f64::NAN.to_be_bytes());
    assert!(clientbound(&mut tracker,&invalid,Context::default()).is_err());
    clientbound(&mut tracker,&[0x4d,1,7],Context::default()).unwrap();
    assert!(clientbound(&mut tracker,&meta,Context::default()).is_err());
    assert!(clientbound(&mut tracker,&movement,Context::default()).is_err());
}

#[test]
fn observed_776_mining_actions_preserve_position_sequence_and_reject_unobserved_actions() {
    for action in 0..=8 {
        let mut body = vec![0x28, action];
        // Номер пакета берётся по закреплённой таблице, не по enum действия.
        let packet = (0..100).find(|&id| crate::packet_layouts::packet_info(RegistryKind::PlayServerbound, MID, OLD, id).is_some_and(|p| p.name == "block_dig")).unwrap();
        body[0] = packet as u8;
        body.extend([0; 8]); body.extend([1, 129, 1]);
        let result = serverbound(&body, Context::default());
        if action <= 7 {
            let result = result.unwrap();
            assert_eq!(&result[1..], &body[1..]);
            for end in 1..body.len() { assert!(serverbound(&body[..end], Context::default()).is_err()); }
            body.push(0); assert!(serverbound(&body, Context::default()).is_err());
        } else { assert!(result.is_err()); }
    }
}

#[test]
fn private_dialog_and_recipe_request_validate_complete_envelopes() {
    // Собственный inline compound; ссылки на чужие диалоги запрещены.
    for body in [vec![0x12, 10, 0], vec![0x11]] {
        assert_eq!(configuration(&body).unwrap().as_ref(), body);
        for end in 0..body.len() { assert!(configuration(&body[..end]).is_err()); }
        let mut tail = body; tail.push(0); assert!(configuration(&tail).is_err());
    }
    for body in [vec![0x12, 0], vec![0x12, 1], vec![0x12, 10]] {
        assert!(configuration(&body).is_err());
    }
    let packet = (0..100).find(|&id| crate::packet_layouts::packet_info(RegistryKind::PlayServerbound, MID, OLD, id).is_some_and(|p| p.name == "craft_recipe_request")).unwrap();
    let body = [packet as u8, 1, 129, 1, 1];
    assert_eq!(&serverbound(&body, Context::default()).unwrap()[1..], &body[1..]);
    for end in 0..body.len() { assert!(serverbound(&body[..end], Context::default()).is_err()); }
    let mut invalid = body; invalid[4] = 2;
    assert!(serverbound(&invalid, Context::default()).is_err());
}
