use super::*;
use crate::{generated_client_chain as tables, ids::{self, RegistryKind as K}, Negotiated};

const KINDS: [K;6] = [K::Item,K::Block,K::BlockState,K::Entity,K::Component,K::Particle];

fn packet(kind: K, from: JavaVersion, to: JavaVersion, name: &str, payload: &[u8]) -> Vec<u8> {
    let entries = tables::packets(kind,from,to).unwrap();
    let id = entries.iter().position(|r| r.0 == name).unwrap();
    let mut out = Vec::new(); wire::uint(&mut out,id as u32);out.extend(payload);out
}

#[test]
fn every_adjacent_static_id_is_bounded_exact_and_reversible() {
    for step in STEPS {
        for (from,to) in [(step.older,step.newer),(step.newer,step.older)] {
            for kind in KINDS {
                let (count,ranges) = tables::ranges(kind,from,to).unwrap();
                for id in 0..count {
                    assert_eq!(ids::translate(kind,from,from,id),Ok(id));
                    let offset = ranges.iter().find(|&&(a,b,_)| (a..=b).contains(&id)).unwrap().2;
                    if offset == 65535 {
                        assert_eq!(ids::translate(kind,from,to,id),Err(ids::IdTranslationError::MissingTarget(id)));
                    } else {
                        let expected = (i64::from(id)+i64::from(offset)) as u32;
                        assert_eq!(ids::translate(kind,from,to,id),Ok(expected));
                        assert_eq!(ids::translate(kind,to,from,expected),Ok(id));
                    }
                }
                for id in [count,u32::MAX] {
                    assert_eq!(ids::translate(kind,from,to,id),Err(ids::IdTranslationError::UnknownId(id)));
                    assert_eq!(ids::translate(kind,from,from,id),Err(ids::IdTranslationError::UnknownId(id)));
                }
            }
        }
    }
}

#[test]
fn each_step_validates_every_control_payload_and_rejects_tails() {
    for step in STEPS {
        for (from,to) in [(step.older,step.newer),(step.newer,step.older)] {
            for (state,direction,kind,name,payload) in [
                (State::Play,Direction::Clientbound,K::PlayClientbound,"keep_alive",vec![0xff;8]),
                (State::Play,Direction::Serverbound,K::PlayServerbound,"keep_alive",vec![0xff;8]),
                (State::Play,Direction::Clientbound,K::PlayClientbound,"ping",vec![0x80;4]),
                (State::Play,Direction::Serverbound,K::PlayServerbound,"pong",vec![0x80;4]),
                (State::Play,Direction::Serverbound,K::PlayServerbound,"held_item_slot",vec![0,8]),
                (State::Play,Direction::Serverbound,K::PlayServerbound,"player_loaded",vec![]),
                (State::Play,Direction::Serverbound,K::PlayServerbound,"tick_end",vec![]),
                (State::Play,Direction::Clientbound,K::PlayClientbound,"chunk_batch_start",vec![]),
                (State::Configuration,Direction::Clientbound,K::ConfigurationClientbound,"finish_configuration",vec![]),
                (State::Configuration,Direction::Serverbound,K::ConfigurationServerbound,"finish_configuration",vec![]),
            ] {
                let body = packet(kind,from,to,name,&payload);
                let mapped = step.translate_control(state,direction,from,&body).unwrap();
                assert_eq!(mapped,packet(kind,to,from,name,&payload));
                assert_eq!(step.translate_control(state,direction,to,&mapped),Ok(body.clone()));
                for end in 0..body.len() {assert!(step.translate_control(state,direction,from,&body[..end]).is_err());}
                let mut tail = body;tail.push(0);assert!(step.translate_control(state,direction,from,&tail).is_err());
            }
            for slot in [-1i16,9,i16::MAX] {
                let body = packet(K::PlayServerbound,from,to,"held_item_slot",&slot.to_be_bytes());
                assert!(step.translate_control(State::Play,Direction::Serverbound,from,&body).is_err());
            }
        }
    }
}

#[test]
fn tables_and_paths_never_authorize_opaque_or_gameplay_payloads() {
    for step in STEPS {
        for (from,to) in [(step.older,step.newer),(step.newer,step.older)] {
            for name in ["map_chunk","entity_metadata","window_items","custom_payload","set_slot","declare_recipes"] {
                let body = packet(K::PlayClientbound,from,to,name,&[0;32]);
                assert!(step.translate_control(State::Play,Direction::Clientbound,from,&body).is_err());
                for primary in [JavaVersion::V26_1_2,JavaVersion::V26_3] {
                    assert!(chain::translate_control(State::Play,Direction::Clientbound,from,primary,&body).is_err());
                    assert!(Negotiated::native_only(from.protocol(),primary).is_err());
                }
            }
            assert!(step.translate_control(State::Play,Direction::Serverbound,from,&vec![0;8*1024*1024+1]).is_err());
            assert!(step.translate_control(State::Play,Direction::Serverbound,JavaVersion::V1_12_2,&[0]).is_err());
            if step!=STEPS[0] && step!=STEPS[1] && step!=STEPS[2] {assert!(ids::translate(K::Attribute,from,to,0).is_err());}
            if from.protocol() < 771 {assert!(crate::items::decode(from,&[0],false,Default::default()).is_err());}
            assert!(wire::translate(State::Play,Direction::Serverbound,from,to,&[0]).is_err());
        }
    }
    let fabricated = Step {older:JavaVersion::V1_21_5,newer:JavaVersion::V1_21_9};
    assert!(fabricated.translate_control(State::Play,Direction::Serverbound,fabricated.older,&[0]).is_err());
}

#[test]
fn old_client_id_composition_uses_each_edge_and_preserves_missing_content() {
    for step in STEPS {
        for kind in KINDS {
            let (count,_) = tables::ranges(kind,step.older,step.newer).unwrap();
            for id in 0..count {
                for primary in [JavaVersion::V26_1_2,JavaVersion::V26_3] {
                    let route = chain::path(step.older,primary).unwrap();
                    let expected = route.windows(2).try_fold(id,|value,edge| ids::translate(kind,edge[0],edge[1],value));
                    assert_eq!(ids::translate(kind,step.older,primary,id),expected);
                    if let Ok(mapped) = expected {assert_eq!(ids::translate(kind,primary,step.older,mapped),Ok(id));}
                }
            }
        }
    }
}

#[test]
fn release_aliases_identify_clients_without_creating_gameplay_profiles() {
    for (protocol,names) in [(773,&["1.21.9","1.21.10"][..]),(772,&["1.21.7","1.21.8"][..]),(771,&["1.21.6"][..]),(770,&["1.21.5"][..])] {
        let version = JavaVersion::from_protocol(protocol).unwrap();
        for name in names {assert_eq!(JavaVersion::from_name(name),Some(version));}
        for primary in [JavaVersion::V26_1_2,JavaVersion::V26_3] {
            assert!(Negotiated::native_only(protocol,primary).is_err());
        }
        assert!(crate::login::Success::decode(version,&[]).is_err());
    }
}
