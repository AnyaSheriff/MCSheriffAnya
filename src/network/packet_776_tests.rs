use super::*;
use mcsa_protocol::{Negotiated, probe_776, ids::{self, RegistryKind}};

#[tokio::test]
async fn core_776_login_has_fresh_session_uuid_and_preserves_signed_profile() {
    use std::io::Read;
    use mcsa_protocol::login::Success;
    let skin = crate::skins::Skin { value: "own-test-value".into(), signature: "own-test-signature".into() };
    let mut previous = None;
    for threshold in [-1, 1] {
        let (left, mut right) = tokio::io::duplex(4096);
        let mut server = CryptoStream::new(left);
        server.protocol = Some(Negotiated { client: JavaVersion::V26_2, native: wire::INTERNAL });
        super::super::login::send_login_success(&mut server, "McsaProbe", &[7; 16], Some(&skin), threshold).await.unwrap();
        let length = read_varint(&mut right).await.unwrap() as usize;
        let mut framed = vec![0; length]; right.read_exact(&mut framed).await.unwrap();
        let body = if threshold < 0 { framed } else {
            let (size, offset) = decode_varint(&framed).unwrap();
            let mut body = Vec::new();
            flate2::read::ZlibDecoder::new(&framed[offset..]).read_to_end(&mut body).unwrap();
            assert_eq!(body.len(), size as usize); body
        };
        assert_eq!(body[0], 2);
        let success = Success::decode(JavaVersion::V26_2, &body[1..]).unwrap();
        assert_eq!(success.uuid, [7; 16]); assert_eq!(success.username, "McsaProbe");
        assert_eq!(success.properties[0].value, skin.value);
        assert_eq!(success.properties[0].signature.as_deref(), Some(skin.signature.as_str()));
        let session = success.session_id.unwrap();
        assert_eq!(session[6] & 0xf0, 0x40); assert_eq!(session[8] & 0xc0, 0x80);
        assert_ne!(Some(session), previous); assert_ne!(session, success.uuid);
        assert_eq!(server.login_session_id, Some(session));
        previous = Some(session);
    }
}

#[tokio::test]
async fn core_776_requires_capability_and_retains_it_across_reader_split() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let peer = tokio::net::TcpStream::connect(listener.local_addr().unwrap()).await.unwrap();
    let mut peer = CryptoStream::new(peer);
    let mut stream = CryptoStream::new(listener.accept().await.unwrap().0);
    stream.protocol = Some(Negotiated { client: JavaVersion::V26_2, native: wire::INTERNAL });
    assert!(Outgoing::new(&mut stream).send(&[0x69, 0]).await.is_err());
    assert!(configuration_body(false, JavaVersion::V26_2, &[3]).is_err());
    stream.probe_776 = true;
    stream.login_session_id = Some([9; 16]);
    {
        let mut out = Outgoing::new(&mut stream);
        out.send(&[0x69, 0]).await.unwrap(); out.flush().await.unwrap();
    }
    assert_eq!(read_packet(&mut peer).await.unwrap().body, [0x69, 0]);
    let mut reader = stream.take_reader().unwrap();
    assert!(reader.probe_776_enabled()); assert_eq!(reader.login_session_id, Some([9; 16]));
    let mut framed = Vec::new(); frame(&mut framed, &[0, 17], compression_threshold());
    write_bytes(&mut peer, &framed).await.unwrap();
    let ack = read_play_packet_routed(&mut reader, JavaVersion::V26_2, None, true).await.unwrap();
    assert_eq!(ack.teleport.unwrap(), mcsa_protocol::teleport::Confirmation { id: 17, pose: None });
}

#[tokio::test]
async fn core_776_short_ack_rejects_tail_and_foreign_click_forces_resync() {
    for body in [vec![0], vec![0, 1, 0], vec![0, 128], vec![0, 1]] {
        let mut framed = Vec::new(); frame(&mut framed, &body, compression_threshold());
        let result = read_play_packet_routed(&mut framed.as_slice(), JavaVersion::V26_2, None, true).await;
        assert_eq!(result.is_ok(), body == [0, 1]);
    }
    let body = [0x12, 0, 255, 255, 255, 255, 15, 0, 36, 0, 0, 0, 0];
    let mut framed = Vec::new(); frame(&mut framed, &body, compression_threshold());
    let read = read_play_packet_routed(&mut framed.as_slice(), JavaVersion::V26_2, None, true).await.unwrap();
    assert!(read.foreign_inventory); assert_eq!(read.id, 0x12);
    let mut framed = Vec::new(); frame(&mut framed, &[0, 1], compression_threshold());
    assert!(read_play_packet_with_legacy(&mut framed.as_slice(), JavaVersion::V26_2, None).await.is_err());
}

#[test]
fn complete_own_stonecutter_packet_maps_every_input_and_output() {
    let body = crate::stonecutter::recipe_packet();
    let converted = probe_776::clientbound(&mut Default::default(), &body, wire::Context::default()).unwrap().into_owned();
    let (id, mut at) = decode_varint(&converted).unwrap(); assert_eq!(id, 0x85);
    let read = |at: &mut usize| {
        let (value, size) = decode_varint(&converted[*at..]).unwrap(); *at += size; value
    };
    assert_eq!(read(&mut at), 0);
    assert_eq!(read(&mut at), crate::stonecutter::recipes().len() as i32);
    for recipe in crate::stonecutter::recipes() {
        assert_eq!(read(&mut at), 2);
        assert_eq!(read(&mut at) as u32, ids::translate(RegistryKind::Item, wire::INTERNAL, JavaVersion::V26_2, recipe.input as u32).unwrap());
        assert_eq!(read(&mut at), 5);
        assert_eq!(read(&mut at) as u32, ids::translate(RegistryKind::Item, wire::INTERNAL, JavaVersion::V26_2, recipe.output as u32).unwrap());
        assert_eq!(read(&mut at), recipe.count);
        assert_eq!(read(&mut at), 0); assert_eq!(read(&mut at), 0);
    }
    assert_eq!(at, converted.len());
    for end in 0..body.len() {
        assert!(probe_776::clientbound(&mut Default::default(), &body[..end], wire::Context::default()).is_err());
    }
}

#[test]
fn first_own_advancement_update_reaches_the_typed_776_path() {
    let body = crate::advancements::AdvState::new().update_body(true).unwrap();
    let result = probe_776::clientbound(&mut Default::default(), &body, wire::Context::default()).unwrap();
    assert_eq!(decode_varint(&result).unwrap().0, 0x82);
    assert!(probe_776::clientbound(&mut Default::default(), &body[..body.len() - 1], wire::Context::default()).is_err());
}
