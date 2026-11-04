

    use super::*;
    use crate::{content::{ContentCodec, IdSpace, entries}, JavaVersion};
    #[test]
    fn every_extension_has_a_view_but_views_never_decode_as_extensions() {
        let codec=ContentCodec::white_slab_preview();
        for row in entries() {
            for (kind, first, last) in [(RegistryKind::Item,row.item,row.item),(RegistryKind::Block,row.block,row.block),(RegistryKind::BlockState,row.first_state,row.last_state)] {
                if first<0 || (kind==RegistryKind::BlockState && !row.has_block()){continue;}
                for id in first..=last {
                    let mapped=substitute(kind,id as u32).unwrap_or_else(||panic!("{} {kind:?} {id}",row.name));
                    assert_eq!(codec.map(kind,IdSpace::Canonical,IdSpace::Wire(JavaVersion::V26_1_2),id as u32),Ok(mapped));
                    assert_ne!(codec.map(kind,IdSpace::Wire(JavaVersion::V26_1_2),IdSpace::Canonical,mapped),Ok(id as u32));
                }
            }
        }
    }
    #[test]
    fn native_slot_view_keeps_count_and_canonical_identity() {
        use crate::{wire,State,Direction,items};
        let codec=ContentCodec::white_slab_preview();
        let context=wire::Context{items:items::Context{content:codec.native_view(),..Default::default()},..Default::default()};
        let stack=items::Stack{item:1506,count:3,added:vec![],removed:vec![]};
        let n=(0..256).find(|n|crate::packet_layouts::packet_info(RegistryKind::PlayClientbound,JavaVersion::V26_1_2,JavaVersion::V26_3,*n).is_some_and(|i|i.name=="set_player_inventory")).unwrap();
        let mut packet=Vec::new();wire::uint(&mut packet,n as u32);packet.push(0);
        packet.extend(items::encode_canonical(Some(&stack),false,items::Context{content:codec,..Default::default()}).unwrap());
        let result=wire::translate_with_context(State::Play,Direction::Clientbound,JavaVersion::V26_1_2,JavaVersion::V26_1_2,&packet,context).unwrap();
        let (decoded,size)=items::decode(JavaVersion::V26_1_2,&result[2..],false,context.items).unwrap();
        assert_eq!(size,result.len()-2);let decoded=decoded.unwrap();assert_eq!(decoded.count,3);
        assert_eq!(Some(decoded.item),substitute(RegistryKind::Item,1506));assert_eq!(stack.item,1506);
    }

    #[test]
    fn preview_775_retains_transport_and_state_packet_ids() {
        use crate::{wire, State, Direction};
        let context = wire::Context { items: crate::items::Context {
            content: ContentCodec::white_slab_preview().native_view(), ..Default::default()
        }, ..Default::default() };
        // Эти пакеты не несут ID каталога. Нельзя переводить их в777 только
        // из-за профиля мира: соседний ID может переключить фазу клиента.
        for name in ["keep_alive", "start_configuration", "update_time", "system_chat", "position", "abilities", "chunk_batch_start", "chunk_batch_finished"] {
            let id = (0..256).find(|id| crate::packet_layouts::packet_info(RegistryKind::PlayClientbound,
                JavaVersion::V26_1_2, JavaVersion::V26_3, *id).is_some_and(|info| info.name == name)).unwrap();
            let mut packet = Vec::new(); wire::uint(&mut packet, id as u32);
            packet.extend([0xa5; 32]);
            assert_eq!(wire::translate_with_context(State::Play, Direction::Clientbound,
                JavaVersion::V26_1_2, JavaVersion::V26_1_2, &packet, context).unwrap().as_ref(), packet);
        }
    }
