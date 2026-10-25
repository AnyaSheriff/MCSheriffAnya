use super::*;
use super::super::{connection::CryptoStream, packet::{PacketInfo, frame, compression_threshold, write_bytes}};
use mcsa_protocol::{JavaVersion, Negotiated};

fn packet(id: i32, payload: &[u8]) -> PacketInfo {
    let mut body = crate::network::varint::encode_varint(id); body.extend(payload);
    PacketInfo { id, length: body.len() as i32, body, teleport: None, punch: false, foreign_inventory: false, legacy_inventory: None }
}

async fn connection(bodies: &[Vec<u8>]) -> Connection {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let client = tokio::net::TcpStream::connect(listener.local_addr().unwrap()).await.unwrap();
    let mut client = CryptoStream::new(client);
    let mut server = CryptoStream::new(listener.accept().await.unwrap().0);
    server.protocol = Some(Negotiated { client: JavaVersion::V26_2, native: super::super::NATIVE_VERSION });
    server.probe_776 = true;
    for body in bodies {
        let mut bytes = Vec::new(); frame(&mut bytes, body, compression_threshold());
        write_bytes(&mut client, &bytes).await.unwrap();
    }
    server // client closes: readers must stop on EOF, not invent an ACK.
}

#[tokio::test]
async fn guarded_login_and_finish_ack_require_exact_empty_payloads() {
    for body in [vec![3], vec![3, 0], vec![2], vec![]] {
        let mut stream = connection(std::slice::from_ref(&body)).await;
        assert_eq!(read_login_acknowledged(&mut stream).await.is_ok(), body == [3]);
    }
    for body in [vec![3], vec![3, 0], vec![9]] {
        let mut stream = connection(std::slice::from_ref(&body)).await;
        let result = read_and_log_configuration_packets(&mut stream, None).await;
        assert_eq!(result.is_ok_and(|look| look.is_some()), body == [3]);
    }
    let mut empty = connection(&[]).await;
    assert_eq!(read_and_log_configuration_packets(&mut empty, None).await.unwrap(), None);
}

#[test]
fn guarded_known_packs_accepts_only_the_exact_offered_subset() {
    let mut known = vec![1];
    for value in ["minecraft", "core", "26.2"] {
        known.extend(crate::network::varint::encode_string(value));
    }
    assert!(validate_probe_known_packs(&[0]).is_ok());
    assert!(validate_probe_known_packs(&known).is_ok());
    for end in 0..known.len() { assert!(validate_probe_known_packs(&known[..end]).is_err()); }
    for mut bad in [known.clone(), vec![0], vec![255, 255, 255, 255, 15], vec![2]] {
        bad.push(0); assert!(validate_probe_known_packs(&bad).is_err());
    }
    for fields in [["minecraft", "core", "26.3"], ["foreign", "core", "26.2"], ["minecraft", "other", "26.2"]] {
        let mut body = vec![1];
        for value in fields { body.extend(crate::network::varint::encode_string(value)); }
        assert!(validate_probe_known_packs(&body).is_err());
    }
}

#[tokio::test]
async fn guarded_known_pack_reader_rejects_tail_negative_count_and_wrong_tuple() {
    for payload in [vec![0], vec![0, 1], vec![255, 255, 255, 255, 15], vec![2]] {
        let body = packet(7, &payload).body;
        let mut stream = connection(&[body]).await;
        assert_eq!(read_known_packs(&mut stream).await.is_ok(), payload == [0]);
    }
}

#[test]
fn guarded_settings_are_complete_and_unknown_phase_packets_fail() {
    let mut settings = crate::network::varint::encode_string("en_us");
    settings.extend([2, 0, 1, 127, 1, 0, 1, 0]);
    assert!(validate_probe_settings(&settings).is_ok());
    for end in 0..settings.len() { assert!(validate_probe_settings(&settings[..end]).is_err()); }
    let mut tail = settings.clone(); tail.push(0);
    assert!(validate_probe_settings(&tail).is_err());
    let mut invalid = settings.clone(); invalid[8] = 2; // chatColors bool
    assert!(validate_probe_settings(&invalid).is_err());
    assert!(validate_probe_config_packet(&packet(0, &settings), false).is_ok());
    assert!(validate_probe_config_packet(&packet(3, &[]), false).is_err());
    assert!(validate_probe_config_packet(&packet(7, &[0]), true).is_err());
    assert!(validate_probe_config_packet(&packet(99, &[]), true).is_err());
}
