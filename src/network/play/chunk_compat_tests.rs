//! Проверка настоящего builder чанков через сетевую границу перевода.
use super::*;
use mcsa_protocol::{JavaVersion, Negotiated, wire};
use crate::network::connection::CryptoStream;

async fn packets(version: JavaVersion) -> Vec<Vec<u8>> {
    let mut world = World::in_memory();
    for z in -16..0 { for x in -16..0 { world.set_block(x, 64, z, blocks::state_by_name("stone").unwrap()); } }
    for (x, name) in [(-12,"chest"),(-11,"barrel"),(-10,"oak_sign"),(-9,"white_bed"),(-8,"beacon")] {
        world.set_block(x, 65, -8, blocks::state_by_name(name).unwrap());
    }
    world.with_sign(-10,65,-8, |sign| { sign.front.lines[0] = "MCSA chunk probe".into(); sign.front.color = 1; });
    let world = Mutex::new(world);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let peer = tokio::net::TcpStream::connect(listener.local_addr().unwrap()).await.unwrap();
    let mut peer = CryptoStream::new(peer);
    let mut stream = CryptoStream::new(listener.accept().await.unwrap().0);
    stream.protocol = Some(Negotiated { client: version, native: wire::INTERNAL });
    stream.enable_encryption(&[54;16]); peer.enable_encryption(&[54;16]);
    let reader = tokio::spawn(async move {
        let mut bodies = Vec::new();
        for _ in 0..14 { bodies.push(read_packet(&mut peer).await.unwrap().body); }
        bodies
    });
    let mut outgoing = Outgoing::new(&mut stream);
    send_login(&mut outgoing, Dimension::Overworld, GAME_MODE_CREATIVE, 1, 2, 2, false).await.unwrap();
    send_abilities(&mut outgoing, GAME_MODE_CREATIVE).await.unwrap();
    send_game_event_start_waiting(&mut outgoing).await.unwrap();
    send_center_chunk(&mut outgoing, (-1,-1)).await.unwrap();
    for z in -2..=0 { for x in -2..=0 { send_chunk(&mut outgoing, &world, x, z, &ChunkLight::open_sky()).await.unwrap(); } }
    let mut position = encode_varint(PLAYER_POSITION); position.push(1);
    for value in [-8.5,67.0,-8.5,0.0,0.0,0.0] { push_f64(&mut position,value); }
    for value in [0.0,0.0] { push_f32(&mut position,value); } push_i32(&mut position,0);
    outgoing.send(&position).await.unwrap(); outgoing.flush().await.unwrap();
    reader.await.unwrap()
}

#[tokio::test]
async fn real_chunk_builder_crosses_encrypted_transport_with_shared_biome_context() {
    let modern = packets(JavaVersion::V26_3).await;
    let native = packets(JavaVersion::V26_1_2).await;
    let count = configuration::REQUIRED_REGISTRIES.iter().find(|(name,_)| *name == "minecraft:worldgen/biome").unwrap().1.len();
    let context = wire::Context { chunk: Some(mcsa_protocol::chunk::Context {
        min_y: world::MIN_Y, sections: world::SECTIONS as usize, shared_biome_count: count as u32,
    }), ..Default::default() };
    assert_eq!(modern.len(),14);
    for (old,new) in native[4..13].iter().zip(&modern[4..13]) {
        assert_eq!(old[0],0x2d); assert_eq!(new[0],0x2e);
        assert_eq!(&old[1..9],&new[1..9]);
        let translated = wire::translate_with_context(mcsa_protocol::State::Play,mcsa_protocol::Direction::Clientbound,JavaVersion::V26_1_2,JavaVersion::V26_3,old,context).unwrap();
        assert_eq!(translated.as_ref(),new);
        // Обратный перевод восстанавливает legacy BE кроватей и не падает.
        assert!(wire::translate_with_context(mcsa_protocol::State::Play,mcsa_protocol::Direction::Clientbound,JavaVersion::V26_3,JavaVersion::V26_1_2,new,context).is_ok());
    }
}

#[tokio::test]
#[ignore = "экспорт только наших Play-пакетов для обычного клиента26.3"]
async fn export_chunk_client_probe() {
    let bodies: Vec<String> = packets(JavaVersion::V26_3).await.into_iter().map(|b| b.iter().map(|b| format!("{b:02x}")).collect()).collect();
    std::fs::create_dir_all("tmp").unwrap();
    std::fs::write("tmp/chunk-client-26.3.json",serde_json::to_vec_pretty(&bodies).unwrap()).unwrap();
}
