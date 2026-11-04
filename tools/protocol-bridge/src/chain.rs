//! Соседние переходы769↔770↔771↔772↔773↔774↔775↔776↔777. Путь не открывает допуск.
//! Пока полная цепь проверяет только перечисленные управляющие пакеты без
//! ID содержимого. Остальные требуют отдельных кодеков и возвращают отказ.
use crate::{Direction, JavaVersion, State, ids::RegistryKind, wire::{self, Cursor, uint}};

const VERSIONS: [JavaVersion; 9] = [JavaVersion::V1_21_4, JavaVersion::V1_21_5, JavaVersion::V1_21_6, JavaVersion::V1_21_8, JavaVersion::V1_21_9, JavaVersion::V1_21_11, JavaVersion::V26_1_2, JavaVersion::V26_2, JavaVersion::V26_3];

pub(crate) fn neighbor(version: JavaVersion) -> JavaVersion {
    match version {
        JavaVersion::V1_21_4 => JavaVersion::V1_21_5,
        JavaVersion::V1_21_5 => JavaVersion::V1_21_6,
        JavaVersion::V1_21_6 => JavaVersion::V1_21_8,
        JavaVersion::V1_21_8 => JavaVersion::V1_21_9,
        JavaVersion::V1_21_9 => JavaVersion::V1_21_11,
        JavaVersion::V1_21_11 => JavaVersion::V26_1_2,
        JavaVersion::V26_1_2 => JavaVersion::V26_2,
        _ => JavaVersion::V26_1_2,
    }
}

/// Направленный маршрут не зависит от основной версии игровых правил.
/// Статический путь ничего не говорит о готовности его самого слабого ребра.
pub fn path(from: JavaVersion, to: JavaVersion) -> Result<Vec<JavaVersion>, wire::Error> {
    let first = VERSIONS.iter().position(|&v| v == from).ok_or(wire::Error::UnsupportedVersion(from))?;
    let last = VERSIONS.iter().position(|&v| v == to).ok_or(wire::Error::UnsupportedVersion(to))?;
    let mut result = VERSIONS[first.min(last)..=first.max(last)].to_vec();
    if first > last { result.reverse(); }
    Ok(result)
}

fn kind(state: State, direction: Direction) -> RegistryKind {
    match (state,direction) {
        (State::Login,Direction::Clientbound) => RegistryKind::LoginClientbound,
        (State::Login,Direction::Serverbound) => RegistryKind::LoginServerbound,
        (State::Configuration,Direction::Clientbound) => RegistryKind::ConfigurationClientbound,
        (State::Configuration,Direction::Serverbound) => RegistryKind::ConfigurationServerbound,
        (State::Play,Direction::Clientbound) => RegistryKind::PlayClientbound,
        (State::Play,Direction::Serverbound) => RegistryKind::PlayServerbound,
    }
}

fn info(registry: RegistryKind, from: JavaVersion, to: JavaVersion, id: usize) -> Option<(&'static str,u32,bool)> {
    if let Some(entries) = crate::generated_client_chain::packets(registry,from,to) {
        let &(name,target,equal) = entries.get(id)?;
        return (target != u16::MAX).then_some((name,u32::from(target),equal));
    }
    if from == JavaVersion::V1_21_11 || to == JavaVersion::V1_21_11 {
        let &(name,target,equal) = crate::generated_step1::packets(registry,from == JavaVersion::V1_21_11)?.get(id)?;
        if target == u16::MAX { return None; }
        Some((name,u32::from(target),equal))
    } else {
        let entry = crate::packet_layouts::packet_info(registry,from,to,id)?;
        Some((entry.name,entry.target_id?,entry.schema_equal))
    }
}

/// Проверка каждого соседнего ребра, включая полное равенство графа схем.
/// Даже from==to проверяется: пустой/неизвестный пакет не становится допустимым.
pub fn translate_control(state: State, direction: Direction, from: JavaVersion, to: JavaVersion, body: &[u8]) -> Result<Vec<u8>, wire::Error> {
    if body.len() > 8 * 1024 * 1024 { return Err(wire::Error::TooLarge); }
    let route = path(from,to)?;
    let mut body = body.to_vec();
    let edges: Vec<_> = if route.len() == 1 {
        // Для проверки родного775 используем его таблицу без смены номера.
        vec![(from,from)]
    } else { route.windows(2).map(|pair| (pair[0],pair[1])).collect() };
    for (source,target) in edges {
        if body.len() > 8 * 1024 * 1024 { return Err(wire::Error::TooLarge); }
        let mut input = Cursor::new(&body);
        let id = input.uint()?;
        let lookup = if source == target {
            let (name,_,_) = info(kind(state,direction),source,neighbor(source),id as usize).ok_or(wire::Error::Invalid)?;
            (name,id,true)
        } else { info(kind(state,direction),source,target,id as usize).ok_or(wire::Error::Invalid)? };
        let (name,mapped,equal) = lookup;
        if !equal { return Err(wire::Error::PayloadUnavailable(name)); }
        let mut out = Vec::new();uint(&mut out,mapped);
        match (state,direction,name) {
            (State::Play,_,"keep_alive") => out.extend(input.take(8)?),
            (State::Play,Direction::Serverbound,"pong") |
            (State::Play,Direction::Clientbound,"ping") => out.extend(input.take(4)?),
            (State::Play,Direction::Serverbound,"player_loaded" | "tick_end") |
            (State::Play,Direction::Clientbound,"chunk_batch_start") |
            (State::Configuration,_,"finish_configuration") => (),
            (State::Play,Direction::Serverbound,"held_item_slot") => {
                let bytes = input.take(2)?;
                let slot = i16::from_be_bytes(bytes.try_into().map_err(|_|wire::Error::Invalid)?);
                if !(0..=8).contains(&slot) { return Err(wire::Error::Invalid); }
                out.extend(bytes);
            }
            _ => return Err(wire::Error::PayloadUnavailable(name)),
        }
        if input.remaining() != 0 { return Err(wire::Error::Invalid); }
        body = out;
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Negotiated, content::{ContentCodec,IdSpace}, ids::{self,RegistryKind as K}};

    #[test]
    fn every_control_route_is_adjacent_and_reversible() {
        for from in VERSIONS {
            for to in VERSIONS {
                let route=path(from,to).unwrap();
                assert_eq!(route.first(),Some(&from));assert_eq!(route.last(),Some(&to));
                assert!(route.windows(2).all(|p| (p[0].protocol()-p[1].protocol()).abs()==1));
                // ID берётся из разрешённой таблицы, значение не зависит от версии.
                let registry=K::PlayServerbound;
                let id=(0..256).find(|&id|info(registry,from,neighbor(from),id).is_some_and(|r|r.0=="keep_alive")).unwrap();
                let mut body=Vec::new();uint(&mut body,id as u32);body.extend((-123456789i64).to_be_bytes());
                let translated=translate_control(State::Play,Direction::Serverbound,from,to,&body).unwrap();
                assert_eq!(translate_control(State::Play,Direction::Serverbound,to,from,&translated).unwrap(),body);
                for n in 0..body.len() { assert!(translate_control(State::Play,Direction::Serverbound,from,to,&body[..n]).is_err()); }
                body.push(0);assert!(translate_control(State::Play,Direction::Serverbound,from,to,&body).is_err());
            }
        }
        assert!(path(JavaVersion::V1_12_2,JavaVersion::V26_3).is_err());
    }

    #[test]
    fn all_explicit_content_projections_are_valid_target_ids() {
        let codec=ContentCodec::white_slab_preview();
        for row in crate::content::entries() {
            for (kind,first,last) in [(K::Item,row.item,row.item),(K::Block,row.block,row.block),(K::BlockState,row.first_state,row.last_state)] {
                if (kind==K::Item && !row.has_item()) || (kind==K::Block && !row.has_block()) || first<0 {continue;}
                for id in first..=last {
                    for version in [JavaVersion::V1_21_11,JavaVersion::V26_2] {
                        let visible=codec.map(kind,IdSpace::Canonical,IdSpace::Wire(version),id as u32).unwrap();
                        assert_eq!(ids::translate(kind,version,version,visible),Ok(visible));
                    }
                }
            }
        }
        // Сера есть в776: её нельзя ухудшать до775-двойника.
        assert_eq!(codec.map(K::Block,IdSpace::Canonical,IdSpace::Wire(JavaVersion::V26_2),1232),Ok(998));
        // Замены — только представление. Назад настоящий white_concrete_slab не восстановить по quartz_slab.
        let shown=codec.map(K::Item,IdSpace::Canonical,IdSpace::Wire(JavaVersion::V1_21_11),1506).unwrap();
        assert_ne!(codec.map(K::Item,IdSpace::Wire(JavaVersion::V1_21_11),IdSpace::Canonical,shown).unwrap(),1506);
    }

    #[test]
    fn older_client_gets_explicit_flowers_and_all_base_content_has_a_view() {
        let codec=ContentCodec::default();
        for (kind,source,target) in [(K::Item,230,229),(K::Block,158,157),(K::BlockState,2322,2121),(K::BlockState,10642,10440)] {
            assert_eq!(codec.map(kind,IdSpace::Canonical,IdSpace::Wire(JavaVersion::V1_21_11),source),Ok(target));
        }
        for kind in [K::Item,K::Block,K::BlockState] {
            let (count,_)=crate::generated_step1::ranges(kind,false).unwrap();
            for source in 0..count {
                let visible=codec.map(kind,IdSpace::Canonical,IdSpace::Wire(JavaVersion::V1_21_11),source).unwrap();
                assert_eq!(ids::translate(kind,JavaVersion::V1_21_11,JavaVersion::V1_21_11,visible),Ok(visible));
            }
            assert!(codec.map(kind,IdSpace::Canonical,IdSpace::Wire(JavaVersion::V1_21_11),u32::MAX).is_err());
        }
        for canonical in [157,158,159,160] {
            let preview=ContentCodec::white_slab_preview();
            for client in [JavaVersion::V1_21_11,JavaVersion::V26_2] {
                let visible=preview.map(K::Entity,IdSpace::Canonical,IdSpace::Wire(client),canonical).unwrap();
                assert_eq!(ids::translate(K::Entity,client,client,visible),Ok(visible));
            }
        }
    }

    #[test]
    fn static_774_ids_roundtrip_and_do_not_enable_admission() {
        for kind in [K::Item,K::Block,K::BlockState,K::Entity,K::Component,K::Particle] {
            for old in [true,false] {
                let (count,_)=crate::generated_step1::ranges(kind,old).unwrap();
                let (from,to)=if old {(JavaVersion::V1_21_11,JavaVersion::V26_1_2)}else{(JavaVersion::V26_1_2,JavaVersion::V1_21_11)};
                for id in 0..count {
                    if let Ok(mapped)=ids::translate(kind,from,to,id) {assert_eq!(ids::translate(kind,to,from,mapped),Ok(id));}
                }
                assert_eq!(ids::translate(kind,from,to,count),Err(ids::IdTranslationError::UnknownId(count)));
            }
        }
        for primary in [JavaVersion::V26_1_2,JavaVersion::V26_3] {
            for client in [774,776] { assert!(Negotiated::native_only(client,primary).is_err()); }
        }
        // Вещи/чанки/metadata и даже равные непрозрачные пакеты запрещены этим API.
        for id in 0..256 {
            if let Some((name,_,_))=info(K::PlayClientbound,JavaVersion::V1_21_11,JavaVersion::V26_1_2,id)
                && matches!(name,"map_chunk"|"window_items"|"entity_metadata"|"custom_payload") {
                let mut body=Vec::new();uint(&mut body,id as u32);body.extend([0;20]);
                assert!(translate_control(State::Play,Direction::Clientbound,JavaVersion::V1_21_11,JavaVersion::V26_3,&body).is_err());
            }
        }
    }
}
