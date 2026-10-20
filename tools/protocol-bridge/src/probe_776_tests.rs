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
    for body in [&[0, 1][..], &[0x38, 0, 0, 0], &[0x42, 0], &[0x3e, 0]] {
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
