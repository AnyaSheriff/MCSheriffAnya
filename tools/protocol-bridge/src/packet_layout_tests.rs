use super::*;
use crate::{Direction, State, ids::{self, IdTranslationError}, wire};
use JavaVersion::{V1_12_2, V26_1_2 as OLD, V26_2 as MID, V26_3 as NEW};

const KINDS: &[RegistryKind] = &[
    RegistryKind::LoginClientbound, RegistryKind::LoginServerbound,
    RegistryKind::ConfigurationClientbound, RegistryKind::ConfigurationServerbound,
    RegistryKind::PlayClientbound, RegistryKind::PlayServerbound,
];

#[test]
fn original_775_777_names_ids_and_equality_are_unchanged() {
    for &kind in KINDS {
        let (a, b) = generated_ids::tables(kind);
        for (from, to, values) in [(OLD, NEW, a), (NEW, OLD, b)] {
            for (id, &target) in values.iter().enumerate() {
                let original = generated_ids::packet_info(kind, from == NEW, id).unwrap();
                assert_eq!(packet_info(kind, from, to, id), Some(PacketInfo {
                    name: original.0, target_id: u32::try_from(target).ok(), schema_equal: original.1,
                }));
            }
            assert_eq!(packet_info(kind, from, to, values.len()), None);
        }
    }
}

#[test]
fn every_776_route_maps_exact_name_with_symmetric_equality_or_missing_target() {
    for &kind in KINDS {
        for (from, to) in [(OLD, MID), (MID, OLD), (MID, NEW), (NEW, MID)] {
            let table = generated_packet_776::route(kind, from, to).unwrap();
            for id in 0..table.len() {
                let info = packet_info(kind, from, to, id).unwrap();
                assert_eq!(name(kind, from, id), Some(info.name));
                if let Some(target) = info.target_id {
                    let inverse = packet_info(kind, to, from, target as usize).unwrap();
                    assert_eq!(inverse.name, info.name);
                    assert_eq!(inverse.target_id, Some(id as u32));
                    assert_eq!(inverse.schema_equal, info.schema_equal);
                } else {
                    assert!(!info.schema_equal);
                    let mut target = 0;
                    while let Some(other) = name(kind, to, target) {
                        assert_ne!(other, info.name);
                        target += 1;
                    }
                }
            }
            for id in [table.len(), 65535, usize::MAX] { assert_eq!(packet_info(kind, from, to, id), None); }
        }
        for id in 0..generated_packet_776::names(kind).unwrap().len() {
            let info = packet_info(kind, MID, MID, id).unwrap();
            assert_eq!(info.target_id, Some(id as u32));
            assert!(info.schema_equal);
        }
    }
    assert_eq!(packet_info(RegistryKind::Item, MID, MID, 0), None);
    assert_eq!(packet_info(RegistryKind::PlayClientbound, V1_12_2, MID, 0), None);
    assert_eq!(packet_info(RegistryKind::PlayClientbound, MID, V1_12_2, 0), None);
}

#[test]
fn spectator_names_are_not_aliases_even_at_the_same_numeric_position() {
    let kind = RegistryKind::PlayServerbound;
    let old = packet_info(kind, OLD, MID, 0x3e).unwrap();
    assert_eq!(old.name, "spectate_entity"); assert_eq!(old.target_id, None); assert!(!old.schema_equal);
    for target in [OLD, NEW] {
        let middle = packet_info(kind, MID, target, 0x3e).unwrap();
        assert_eq!(middle.name, "spectator_action"); assert_eq!(middle.target_id, None); assert!(!middle.schema_equal);
    }
    // В7770x3e — update_sign; старое spectator имя не возвращаем по позиции.
    assert_eq!(name(kind, NEW, 0x3e), Some("update_sign"));
    for target in [OLD, MID] {
        let new = packet_info(kind, NEW, target, 0x2e).unwrap();
        assert_eq!(new.name, "punch"); assert_eq!(new.target_id, None); assert!(!new.schema_equal);
    }
    assert_eq!(packet_info(kind, MID, OLD, 0x40).unwrap().name, "spectate");
    assert_eq!(packet_info(kind, MID, OLD, 0x40).unwrap().target_id, Some(0x40));
}

#[test]
fn schema_equality_belongs_to_the_requested_pair_not_a_modern_boolean() {
    let login = RegistryKind::LoginClientbound;
    assert!(!packet_info(login, OLD, MID, 2).unwrap().schema_equal);
    assert!(packet_info(login, MID, NEW, 2).unwrap().schema_equal);
    let play = RegistryKind::PlayClientbound;
    assert!(!packet_info(play, OLD, MID, 0x31).unwrap().schema_equal);
    // Это равенство кандидатного JSON, НЕ фактического SpawnInfo:
    // private776 observations уже показали byte modes против VarInt777.
    assert!(packet_info(play, MID, NEW, 0x31).unwrap().schema_equal);
    assert!(packet_info(play, OLD, MID, 0x35).unwrap().schema_equal); // relative move
    assert!(!packet_info(play, MID, NEW, 0x35).unwrap().schema_equal);
    assert!(packet_info(play, OLD, MID, 0x30).unwrap().schema_equal); // light
    assert!(!packet_info(play, MID, NEW, 0x30).unwrap().schema_equal);
    assert!(!packet_info(play, OLD, MID, 0x6d).unwrap().schema_equal); // teams776 особый
    assert!(!packet_info(play, MID, NEW, 0x6d).unwrap().schema_equal);
    assert!(!packet_info(play, OLD, MID, 0x14).unwrap().schema_equal); // transitive Slot
}

#[test]
fn private_packet_inventory_does_not_enable_payload_tracker_or_admission_776() {
    for (from, to) in [(MID, OLD), (OLD, MID), (MID, NEW), (NEW, MID), (MID, MID)] {
        for &kind in KINDS {
            assert_eq!(ids::translate(kind, from, to, 0), Err(IdTranslationError::UnsupportedVersion(MID)));
        }
        for state in [State::Login, State::Configuration, State::Play] {
            for direction in [Direction::Clientbound, Direction::Serverbound] {
                assert_eq!(wire::translate(state, direction, from, to, &[0]), Err(wire::Error::UnsupportedVersion(MID)));
            }
        }
        for body in [&[][..], &[0][..], &[0x23][..]] {
            let mut tracker = crate::entities::Tracker::default();
            assert_eq!(tracker.translate(from, to, body, wire::Context::default()), Err(wire::Error::UnsupportedVersion(MID)));
            assert_eq!(tracker.translate_packets(from, to, body, wire::Context::default()), Err(wire::Error::UnsupportedVersion(MID)));
        }
    }
    assert!(crate::Negotiated::native_only(776, OLD).is_err());
}
