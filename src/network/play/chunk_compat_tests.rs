//! Проверка настоящего builder чанков через сетевую границу перевода.
use super::*;

#[tokio::test]
async fn changed_torch_light_crosses_chunk_edge_and_removal_clears_all_client_masks() {
    use mcsa_protocol::{JavaVersion as V, light::Light, wire};
    let mut world = World::in_memory();
    // Факел у границы: проверяем и источник, и свет в соседнем чанке.
    world.set_block(15, 64, 0, blocks::state_by_name("stone").unwrap());
    world.set_block(16, 64, 0, blocks::state_by_name("stone").unwrap());
    for (state, levels) in [(blocks::state_by_name("torch").unwrap(), [14, 13]), (0, [0, 0])] {
        world.set_block(15, 65, 0, state);
        for (x, expected) in levels.into_iter().enumerate() {
            let (light, recounted) = world.light_job(x as i32, 0).unwrap().finish();
            world.store_light(recounted);
            let mut out = Outgoing::capture();
            send_light_update(&mut out, x as i32, 0, &light).await.unwrap();
            let (size, offset) = decode_varint(out.queued()).unwrap();
            let body = &out.queued()[offset..];
            assert_eq!(body.len(), size as usize);
            for version in [V::V26_1_2, V::V26_2, V::V26_3] {
                let mapped = if version == V::V26_2 {
                    mcsa_protocol::probe_776::clientbound(&mut Default::default(), body, wire::Context::default()).unwrap()
                } else { wire::translate(mcsa_protocol::State::Play, mcsa_protocol::Direction::Clientbound, V::V26_1_2, version, body).unwrap() };
                let (_, id_size) = decode_varint(&mapped).unwrap();
                // Оба положительных номера чанка занимают по одному VarInt.
                assert_eq!(&mapped[id_size..id_size + 2], &[x as u8, 0]);
                let decoded = Light::decode(version, &mapped[id_size + 2..]).unwrap();
                let bit = 9usize;
                let present = decoded.block.get(bit / 8).copied().unwrap_or(0) & (1 << (bit % 8)) != 0;
                let actual = if present {
                    let index: usize = (0..bit).map(|b| usize::from(decoded.block[b / 8] & (1 << (b % 8)) != 0)).sum();
                    let cell = 256 + if x == 0 { 15 } else { 0 };
                    (decoded.block_arrays[index][cell / 2] >> ((cell % 2) * 4)) & 15
                } else {
                    assert_ne!(decoded.empty_block[bit / 8] & (1 << (bit % 8)), 0);
                    0
                };
                assert_eq!(actual, expected, "{version:?}, chunk{x}");
            }
        }
    }
}
use mcsa_protocol::{JavaVersion, Negotiated, wire};
use crate::network::connection::CryptoStream;

#[test]
fn legacy_recipe_book_uses_actual_core_descriptors_and_returns_native_display_id() {
    use mcsa_protocol::{legacy_chunk::HeightWindow, legacy_play::Session};
    let mut state = Session::new(HeightWindow::new(-64).unwrap(),2).unwrap();
    let mut book = crate::recipe_book::Book::default();
    book.grant_initial();
    assert!(book.learn_named(&["oak_planks".into()]));
    state.clientbound(&book.settings_body(),&[]).unwrap();
    for packet in book.update() {
        let output = state.clientbound(&packet,&[]).unwrap();
        assert_eq!(output[0][0],0x31);
    }
    let mut request = vec![0x12,0]; request.extend(encode_varint(360)); request.push(0);
    let native = state.serverbound(&request).unwrap();
    assert_eq!(&native[..2],&[0x27,0]);
    let mut cursor = Cursor::new(&native[2..]);
    let display = cursor.varint().unwrap();
    assert_ne!(display,360);
    let take = book.take(&["crafting_table".into()]).unwrap();
    let mut removed = Cursor::new(&take[1..]);
    assert_eq!(removed.varint(),Some(1)); assert_eq!(removed.varint(),Some(display));
    state.clientbound(&take,&[]).unwrap(); assert!(state.serverbound(&request).is_err());
}

#[tokio::test]
async fn legacy_control_uses_the_actual_outgoing_and_inbound_transport() {
    use mcsa_protocol::{legacy_chunk::HeightWindow, legacy_control::Packet, legacy_play::Session, State};
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let peer = tokio::net::TcpStream::connect(listener.local_addr().unwrap()).await.unwrap();
    let mut peer = CryptoStream::new(peer);
    let mut server = CryptoStream::new(listener.accept().await.unwrap().0);
    server.protocol = Some(Negotiated { client: JavaVersion::V1_12_2, native: wire::INTERNAL });
    server.legacy = Some(Arc::new(Mutex::new(Session::new(HeightWindow::new(-64).unwrap(), 3).unwrap())));
    server.enable_encryption(&[41; 16]); peer.enable_encryption(&[41; 16]);
    let reading = tokio::spawn(async move {
        let join = read_packet(&mut peer).await.unwrap();
        let Packet::Join { dimension, difficulty, mode, entity, .. } = Packet::decode(State::Play, join.id as u32, join.payload()).unwrap() else { panic!() };
        assert_eq!((dimension, difficulty, mode, entity), (0, 3, 1, 73));
        let abilities = read_packet(&mut peer).await.unwrap();
        assert!(matches!(Packet::decode(State::Play, abilities.id as u32, abilities.payload()).unwrap(), Packet::Abilities { .. }));
        let teleport = read_packet(&mut peer).await.unwrap();
        let Packet::Teleport { pose, id, flags } = Packet::decode(State::Play, teleport.id as u32, teleport.payload()).unwrap() else { panic!() };
        assert_eq!((pose.x, pose.y, pose.z, flags), (0.5, 128.0, -1.5, 0));
        let chunk = read_packet(&mut peer).await.unwrap();
        assert_eq!(chunk.id, 0x20);
        let chunk = mcsa_protocol::legacy_chunk::Chunk::decode(chunk.payload(), true).unwrap();
        assert_eq!(chunk.sections[0].1.blocks[0], 16);
        assert_eq!(chunk.biomes, Some([1; 256]));
        let mut body = vec![0]; body.extend(encode_varint(id));
        let mut frame = Vec::new(); super::super::packet::frame(&mut frame, &body, super::super::packet::compression_threshold());
        super::super::packet::write_bytes(&mut peer, &frame).await.unwrap();
        let kick = read_packet(&mut peer).await.unwrap();
        assert_eq!(kick.id, 0x1a);
        assert_eq!(&kick.payload()[1..], br#""bye""#);
    });
    let legacy = server.legacy.clone();
    let mut outgoing = Outgoing::new(&mut server);
    send_login(&mut outgoing, Dimension::Overworld, GAME_MODE_CREATIVE, 73, 2, 2, false).await.unwrap();
    send_abilities(&mut outgoing, GAME_MODE_CREATIVE).await.unwrap();
    let mut body = vec![0x48, 9];
    for value in [0.5f64, 64.0, -1.5, 0.0, 0.0, 0.0] { body.extend(value.to_be_bytes()); }
    body.extend([0; 12]);
    outgoing.send(&body).await.unwrap();
    let mut world = World::in_memory(); world.set_block(0, -64, 0, blocks::state_by_name("stone").unwrap());
    let plains = world::terrain::Biome::ALL.iter().position(|biome| *biome == world::terrain::Biome::Plains).unwrap() as u16;
    assert_ne!(plains, 1); // Проверяем перевод имени, не совпадение ID.
    world.fill_biome((0, -64, 0), (15, 319, 15), plains, None);
    let snapshot = world.chunk_packet(0, 0, |sections,_| sections.to_vec()).unwrap();
    let light = crate::world::light::chunk_light(&snapshot);
    send_chunk(&mut outgoing, &Mutex::new(world), 0, 0, &light).await.unwrap();
    outgoing.flush().await.unwrap();
    let confirmation = super::super::packet::read_play_packet_with_legacy(outgoing.stream(), JavaVersion::V1_12_2, legacy.as_ref()).await.unwrap();
    assert_eq!(confirmation.teleport_confirmation().unwrap().id, 9);
    outgoing.disconnect(&[0x20, 8, 0, 3, b'b', b'y', b'e']).await;
    reading.await.unwrap();
}

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
        while let Ok(packet) = read_packet(&mut peer).await { bodies.push(packet.body); }
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
    outgoing.send(&position).await.unwrap();
    // Настоящий push_slot: износ, зачарование, название и карта.
    let mut extra = crate::stackdata::Data { damage: 7, unbreakable: true, name: Some("MCSA sword".into()), ..Default::default() };
    extra.set_enchant(crate::stackdata::enchantment_number("sharpness").unwrap(),5);
    let sword = Stack::new(blocks::item_named("diamond_sword").unwrap(),1).with_extras(extra);
    let mut content = encode_varint(SET_CONTAINER_CONTENT);
    for n in [0,1,46] { content.extend(encode_varint(n)); }
    for slot in 0..46 { push_slot(&mut content,if slot == 36 {Some(sword)} else {None}); }
    push_slot(&mut content,None); outgoing.send(&content).await.unwrap();
    let mut update = encode_varint(SET_CONTAINER_SLOT); update.extend([0,2,0,37]);
    push_slot(&mut update,Some(Stack::new(blocks::item_named("red_wool").unwrap(),5)));
    outgoing.send(&update).await.unwrap();
    let mut cursor = encode_varint(SET_CURSOR_ITEM); push_slot(&mut cursor,None);
    outgoing.send(&cursor).await.unwrap();
    let mut player_slot = encode_varint(0x6c); player_slot.push(2);
    let map = Stack::new(blocks::item_named("filled_map").unwrap(),1).with_extras(crate::stackdata::Data {map_id:Some(23),..Default::default()});
    push_slot(&mut player_slot,Some(map)); outgoing.send(&player_slot).await.unwrap();
    // Настоящий SpawnInfo-builder: два измерения, previousMode и точка смерти.
    for dimension in [Dimension::Nether, Dimension::Overworld] {
        let mut respawn = encode_varint(RESPAWN);
        push_spawn_info_ex(&mut respawn, dimension, GAME_MODE_CREATIVE, 0, None, 10);
        respawn.push(3); outgoing.send(&respawn).await.unwrap();
    }
    send_center_chunk(&mut outgoing, (-1,-1)).await.unwrap();
    for z in -2..=0 { for x in -2..=0 { send_chunk(&mut outgoing, &world, x, z, &ChunkLight::open_sky()).await.unwrap(); } }
    position[1] = 2; outgoing.send(&position).await.unwrap();
    for (i,kind) in [crate::mob::Kind::Cow,crate::mob::Kind::Slime,crate::mob::Kind::MagmaCube].into_iter().enumerate() {
        let mut mob = crate::mob::Mob::new(20+i as i32,kind,(-10.5+i as f64,65.0,-10.5));
        mob.uuid = [70+i as u8;16]; // одна синтетическая личность в обоих прогонах
        mob.no_ai = true;
        mob.vitals.give_effect(crate::survival::EffectKind::Speed,1,2000);
        mob.vitals.give_effect(crate::survival::EffectKind::Strength,1,2000);
        if kind == crate::mob::Kind::Slime { mob.nether.size = 4; }
        mobs::send_compatibility_probe(&mut outgoing,&mob).await.unwrap();
    }
    for (particle, data, count) in [(4,None,7),(4,None,0),(1,Some(blocks::state_by_name("bamboo_planks").unwrap()),7),
        (47,Some(blocks::item_named("red_wool").unwrap()),7)] {
        mobs::send_particles(&mut outgoing,particle,data,(-8.5,67.0,-8.5),(0.25,0.5,0.75),0.125,count).await.unwrap();
    }
    outgoing.flush().await.unwrap();
    drop(outgoing); drop(stream);
    reader.await.unwrap()
}

#[tokio::test]
async fn real_chunk_builder_crosses_encrypted_transport_with_shared_biome_context() {
    let modern = packets(JavaVersion::V26_3).await;
    let native = packets(JavaVersion::V26_1_2).await;
    let count = configuration::REQUIRED_REGISTRIES.iter().find(|(name,_)| *name == "minecraft:worldgen/biome").unwrap().1.len();
    static COUNTS: std::sync::LazyLock<Vec<(&'static str,u32)>> = std::sync::LazyLock::new(||
        configuration::REQUIRED_REGISTRIES.iter().map(|(name,entries)| (*name,entries.len() as u32)).collect());
    let context = wire::Context { metadata_registry_counts:&COUNTS, items: mcsa_protocol::items::Context { shared_enchantment_count: Some(crate::stackdata::ENCHANTMENTS.len() as u32), ..Default::default() }, chunk: Some(mcsa_protocol::chunk::Context {
        min_y: world::MIN_Y, sections: world::SECTIONS as usize, shared_biome_count: count as u32,
    }), ..Default::default() };
    assert_eq!(modern.len(),43);
    // Реальная проба775: ItemStackTemplate red_wool = e3 01 <count> 00 00.
    // Это ловит прежнюю перестановку ID/количества даже на родном маршруте.
    assert!(native.last().unwrap().ends_with(&[47,0xe3,1,1,0,0]));
    assert_eq!(native.iter().filter(|body| body[0] == 0x2f).count(),4);
    assert_eq!(modern.iter().filter(|body| body[0] == 0x30).count(),4);
    assert_eq!(modern.len(),native.len());
    for (old,new) in native[14..18].iter().zip(&modern[14..18]) {
        let mapped = wire::translate_with_context(mcsa_protocol::State::Play,mcsa_protocol::Direction::Clientbound,JavaVersion::V26_1_2,JavaVersion::V26_3,old,context).unwrap();
        assert_eq!(mapped.as_ref(),new);
        assert_eq!(wire::translate_with_context(mcsa_protocol::State::Play,mcsa_protocol::Direction::Clientbound,JavaVersion::V26_3,JavaVersion::V26_1_2,new,context).unwrap().as_ref(),old);
    }
    let mut tracker = mcsa_protocol::entities::Tracker::default();
    let context = wire::Context { online_mode:Some(false), ..context };
    for (old,new) in native.iter().zip(&modern) {
        assert_eq!(tracker.translate(JavaVersion::V26_1_2,JavaVersion::V26_3,old,context).unwrap().as_ref(),new);
    }
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

#[tokio::test]
async fn native_world_snapshot_preserves_blocks_light_and_negative_height_in_legacy_chunk() {
    use mcsa_protocol::{legacy_blocks::{self, SourceSection, View}, legacy_chunk::{Chunk, HeightWindow}};
    let mut world = World::in_memory();
    let stone = blocks::state_by_name("stone").unwrap();
    let wool = blocks::state_by_name("red_wool").unwrap();
    let glowstone = blocks::state_by_name("glowstone").unwrap();
    for (x,y,z,state) in [(-16,-64,-16,stone),(-15,-63,-16,glowstone),(-1,191,-1,wool),(-1,192,-1,stone)] {
        assert!(world.set_block(x,y,z,state));
    }
    // Тот же снимок Arc-секций, который берёт обычный send_chunk.
    let snapshot = world.chunk_packet(-1,-1, |sections,_| sections.to_vec()).unwrap();
    let light = crate::world::light::chunk_light(&snapshot);
    let block_light: Vec<_> = light.block.iter().map(|v| v.nibbles()).collect();
    let sky_light: Vec<_> = light.sky.iter().map(|v| v.nibbles()).collect();
    let source: Vec<_> = snapshot.iter().enumerate().map(|(i,blocks)| SourceSection {
        blocks: blocks.as_deref(), block_light: &block_light[i], sky_light: Some(&sky_light[i]),
    }).collect();
    let window = HeightWindow::new(-64).unwrap();
    let projected = legacy_blocks::project(View { version: JavaVersion::V26_1_2, x:-1,z:-1,min_y:world::MIN_Y,
        window, sections:&source, biomes:[1;256], block_entities:vec![] }).unwrap();
    assert_eq!(projected.sections[0].1.blocks[0], 16);
    assert_eq!(projected.sections[15].1.blocks[4095], 35*16+14);
    assert_eq!(projected.sections[0].1.block_light[128] >> 4, 15);
    assert_eq!(projected.sections.len(), 16);
    let body = projected.encode(true).unwrap();
    // Реальное framing и существующее шифрование потока; только порт0 для теста.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let peer = tokio::net::TcpStream::connect(listener.local_addr().unwrap()).await.unwrap();
    let mut peer = CryptoStream::new(peer);
    let mut writer = CryptoStream::new(listener.accept().await.unwrap().0);
    peer.enable_encryption(&[81;16]); writer.enable_encryption(&[81;16]);
    let reader = tokio::spawn(async move { read_packet(&mut peer).await.unwrap().body });
    let mut packet = vec![0x20]; packet.extend(body);
    let mut frame = Vec::new(); crate::network::packet::frame(&mut frame,&packet,-1);
    crate::network::packet::write_bytes(&mut writer,&frame).await.unwrap();
    let received = reader.await.unwrap();
    assert_eq!(received[0],0x20);
    assert_eq!(Chunk::decode(&received[1..],true).unwrap(),projected);
    // Снимок и перевод не переписывают серверную высоту или состояние.
    assert_eq!(world.get_block(-16,-64,-16),stone);
    assert_eq!(world.get_block(-1,191,-1),wool);
    assert_eq!(world.get_block(-1,192,-1),stone);
    let mut modified = snapshot.clone();
    let section = modified[0].as_mut().unwrap();
    std::sync::Arc::make_mut(section)[0] = blocks::state_by_name("copper_bulb").unwrap() as u16;
    let invalid: Vec<_> = modified.iter().enumerate().map(|(i,blocks)| SourceSection {
        blocks: blocks.as_deref(), block_light:&block_light[i], sky_light:Some(&sky_light[i]),
    }).collect();
    assert!(legacy_blocks::project(View {version:JavaVersion::V26_1_2,x:-1,z:-1,min_y:world::MIN_Y,
        window,sections:&invalid,biomes:[1;256],block_entities:vec![]}).is_err());
}

#[test]
#[ignore = "экспорт собственного мира и пакетов для непрозрачного клиента1.12.2"]
fn export_legacy_chunk_client_probe() {
    use mcsa_protocol::{legacy_blocks::{self, SourceSection, View}, legacy_chunk::HeightWindow,
        legacy_control::Packet as Legacy, teleport::Pose};
    let mut world = World::in_memory();
    for z in -16..16 { for x in -16..16 { world.set_block(x,0,z,blocks::state_by_name("stone").unwrap()); } }
    for (x,name) in [(2,"red_wool"),(3,"glowstone"),(4,"diamond_block")] {
        world.set_block(x,1,2,blocks::state_by_name(name).unwrap());
    }
    let hex = |bytes: &[u8]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let control = |value: Legacy| {
        let (_,id,bytes)=value.encode().unwrap(); let mut out=encode_varint(id as i32);out.extend(bytes);hex(&out)
    };
    let pose=Pose {x:0.5,y:65.0,z:0.5,yaw:0.0,pitch:0.0};
    let mut sky_chunks=Vec::new(); let mut dark_chunks=Vec::new();
    for z in -1..=0 { for x in -1..=0 {
        let snapshot=world.chunk_packet(x,z,|sections,_|sections.to_vec()).unwrap();
        let light=crate::world::light::chunk_light(&snapshot);
        let block:Vec<_>=light.block.iter().map(|s|s.nibbles()).collect();
        let sky:Vec<_>=light.sky.iter().map(|s|s.nibbles()).collect();
        for sky_enabled in [true,false] {
            let source:Vec<_>=snapshot.iter().enumerate().map(|(i,s)|SourceSection {blocks:s.as_deref(),block_light:&block[i],sky_light:sky_enabled.then_some(sky[i].as_slice())}).collect();
            let chunk=legacy_blocks::project(View {version:JavaVersion::V26_1_2,x,z,min_y:world::MIN_Y,
                window:HeightWindow::new(-64).unwrap(),sections:&source,biomes:[1;256],block_entities:vec![]}).unwrap();
            let mut out=vec![0x20];out.extend(chunk.encode(sky_enabled).unwrap());
            if sky_enabled {sky_chunks.push(hex(&out));} else {dark_chunks.push(hex(&out));}
        }
    } }
    let report=serde_json::json!({
        "protocol":340,"window_origin":-64,"position":[pose.x,pose.y,pose.z,pose.yaw,pose.pitch],
        "login":control(Legacy::LoginSuccess {uuid:crate::network::login::offline_uuid("McsaProbe"),username:"McsaProbe".into()}),
        "join":control(Legacy::Join {entity:1,mode:1,hardcore:false,dimension:0,difficulty:2,maximum:20,level_type:"flat".into(),reduced_debug:false}),
        "abilities":control(Legacy::Abilities {flags:7,flying:0.05,walking:0.1}),
        "teleports":[control(Legacy::Teleport {pose,flags:0,id:1}),control(Legacy::Teleport {pose,flags:0,id:2}),control(Legacy::Teleport {pose,flags:0,id:3})],
        "respawn_nether":control(Legacy::Respawn {dimension:-1,difficulty:2,mode:1,level_type:"flat".into()}),
        "respawn_overworld":control(Legacy::Respawn {dimension:0,difficulty:2,mode:1,level_type:"flat".into()}),
        "sky_chunks":sky_chunks,"dark_chunks":dark_chunks,
    });
    std::fs::create_dir_all("tmp").unwrap();
    std::fs::write("tmp/legacy-chunk-client.json",serde_json::to_vec_pretty(&report).unwrap()).unwrap();
}


#[tokio::test]
async fn generated_palette_ids_match_every_client_configuration() {
    check_generated_palettes(0, None).await;
}

#[tokio::test]
#[ignore = "полный обход четырёх семян, трёх измерений, радиус16; экспорт наших пакетов для recorder777"]
async fn generated_palette_radius16_recorder_export() {
    let export = std::env::var_os("MCSA_PALETTE_EXPORT").map(std::path::PathBuf::from);
    check_generated_palettes(16, export.as_deref()).await;
}

async fn check_generated_palettes(radius: i32, export: Option<&std::path::Path>) {
    use mcsa_protocol::{content::{ContentCodec, IdSpace}, ids::RegistryKind};
    use std::io::Write;
    let versions = [JavaVersion::V26_3, JavaVersion::V26_2, JavaVersion::V26_1_2,
        JavaVersion::V1_21_11, JavaVersion::V1_21_9, JavaVersion::V1_21_8,
        JavaVersion::V1_21_6, JavaVersion::V1_21_5, JavaVersion::V1_21_4];
    let content = ContentCodec::white_slab_preview();
    let registries: Vec<_> = versions.iter().map(|v| configuration::test_chunk_biomes(*v, true)).collect();
    let mut seen_blocks = HashSet::new();
    let mut seen_biomes = HashSet::new();
    let mut chunks = 0usize;
    let mut sections_checked = 0usize;
    if let Some(path) = export {
        std::fs::create_dir_all(path).unwrap();
        let mut file = std::fs::File::create(path.join("configuration.frames")).unwrap();
        for registry in configuration::registry_bundle_for(JavaVersion::V26_3, true).unwrap() {
            let body = registry.encode().unwrap();
            file.write_all(&encode_varint(body.len() as i32)).unwrap();
            file.write_all(&body).unwrap();
        }
    }
    for seed in [1792855987123998292i64, 1, -987654321, 8675309] {
        for dimension in 0..3 {
            let world = World::palette_test_world(seed, dimension);
            let spawn = world.spawn_position();
            let center = ((spawn.0.floor() as i32).div_euclid(16), (spawn.2.floor() as i32).div_euclid(16));
            let world = Mutex::new(world);
            let mut file = export.map(|path| flate2::write::GzEncoder::new(
                std::fs::File::create(path.join(format!("{seed}-{dimension}.frames.gz"))).unwrap(), flate2::Compression::fast()));
            if let Some(file) = &mut file {
                let mut startup = Outgoing::capture_content(JavaVersion::V26_3, content);
                let dimension = [Dimension::Overworld, Dimension::Nether, Dimension::End][dimension];
                send_login(&mut startup, dimension, GAME_MODE_CREATIVE, 1, 16, 2, false).await.unwrap();
                send_abilities(&mut startup, GAME_MODE_CREATIVE).await.unwrap();
                file.write_all(startup.queued()).unwrap();
            }
            let mut positions: Vec<_> = (-radius..=radius).flat_map(|z| (-radius..=radius).map(move |x| (center.0+x,center.1+z))).collect();
            if seed == 1792855987123998292 && dimension == 0 && !positions.contains(&(3,-3)) { positions.push((3,-3)); }
            for (x,z) in positions {
                {
                let mut world = world.lock().unwrap();
                world.palette_test_generate(x,z);
                world.chunk_packet(x,z, |sections,biomes| {
                    // Проверяем каждую ячейку, включая редкие состояния и все вертикальные биомы.
                    for section in sections.iter().flatten() {
                        for &id in section.iter() {
                            if seen_blocks.insert(id) {
                                for version in versions {
                                    let mapped = content.map(RegistryKind::BlockState, IdSpace::Canonical, IdSpace::Wire(version), u32::from(id))
                                        .unwrap_or_else(|e| panic!("seed{seed} dim{dimension} chunk{x},{z} block{id} client{version:?}: {e:?}"));
                                    content.map(RegistryKind::BlockState, IdSpace::Wire(version), IdSpace::Wire(version), mapped).unwrap();
                                }
                            }
                        }
                    }
                    for &id in biomes {
                        seen_biomes.insert(id);
                        for registry in &registries { assert!(usize::from(registry.map(id).unwrap()) < registry.count); }
                    }
                    if radius == 0 {
                        let registry = &registries[0];
                        let mut bytes = Vec::new();
                        for (section, biomes) in sections.iter().zip(biomes.chunks(world::SECTION_BIOMES)) {
                            let mapped: Vec<_> = biomes.iter().map(|id| registry.map(*id).unwrap()).collect();
                            push_section(&mut bytes, section.as_deref(), Some(&mapped), registry);
                        }
                        let canonical = mcsa_protocol::chunk_sections::decode_with_content(wire::INTERNAL,
                            &bytes, world::SECTIONS as usize, registry.count as u32, IdSpace::Canonical, content).unwrap();
                        for (version, registry) in versions.iter().zip(&registries) {
                            let mut translated = canonical.clone();
                            for section in &mut translated {
                                for id in &mut section.blocks {
                                    *id = content.map(RegistryKind::BlockState, IdSpace::Canonical, IdSpace::Wire(*version), *id).unwrap();
                                }
                            }
                            let bytes = mcsa_protocol::chunk_sections::encode(*version, &translated, registry.count as u32).unwrap();
                            assert_eq!(mcsa_protocol::chunk_sections::decode(*version, &bytes,
                                world::SECTIONS as usize, registry.count as u32).unwrap(), translated);
                        }
                    }
                    sections_checked += sections.len();
                }).unwrap();
                }
                let mut outgoing = Outgoing::capture_content(JavaVersion::V26_3, content);
                send_chunk(&mut outgoing, &world, x,z,&ChunkLight::open_sky()).await.unwrap();
                let (length, offset) = decode_varint(outgoing.queued()).unwrap();
                let body = &outgoing.queued()[offset..];
                assert_eq!(body.len(), length as usize);
                let (_, id_size) = decode_varint(body).unwrap();
                let mut cursor = Cursor::new(&body[id_size + 8..]);
                assert_eq!(cursor.varint(), Some(0));
                let size = cursor.varint().unwrap() as usize;
                let decoded = mcsa_protocol::chunk_sections::decode(JavaVersion::V26_3, cursor.bytes(size).unwrap(), world::SECTIONS as usize, registries[0].count as u32).unwrap();
                assert_eq!(decoded.len(), world::SECTIONS as usize);
                if let Some(file) = &mut file { file.write_all(outgoing.queued()).unwrap(); }
                chunks += 1;
            }
            if let Some(file) = file { file.finish().unwrap(); }
            eprintln!("palette seed={seed} dimension={dimension} radius={radius} chunks={chunks} sections={sections_checked} unique_blocks={} unique_biomes={}", seen_blocks.len(), seen_biomes.len());
        }
    }
    if radius == 16 { assert!(seen_biomes.contains(&66), "серные пещеры должны встретиться"); }
}


#[tokio::test]
async fn unknown_biome_is_rejected_before_any_chunk_bytes_are_queued() {
    let mut world = World::in_memory();
    world.set_block(0, 64, 0, 1);
    world.fill_biome((0,-64,0), (15,319,15), u16::MAX, None);
    let mut outgoing = Outgoing::capture();
    assert!(send_chunk(&mut outgoing, &Mutex::new(world), 0,0,&ChunkLight::open_sky()).await.is_err());
    assert!(outgoing.queued().is_empty());
}

#[tokio::test]
async fn preview_pot_chunk_and_update_use_observed_wire_without_erasing_saved_side() {
    use mcsa_protocol::{content::ContentCodec, ids::RegistryKind};
    let fixture: serde_json::Value = serde_json::from_str(include_str!(
        "../../../tools/protocol-bridge/testdata/decorated-pot-wire-stone-777.json")).unwrap();
    let hex = |text: &str| text.as_bytes().chunks_exact(2).map(|pair|
        u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap()).collect::<Vec<_>>();
    // Независимая ожидаемая проекция: список775 имеет четыре обычные стороны.
    let mut old_view = vec![10, 9, 0, 6];
    old_view.extend_from_slice(b"sherds");
    old_view.extend_from_slice(&[8, 0, 0, 0, 4]);
    for _ in 0..4 {
        old_view.extend_from_slice(&15u16.to_be_bytes());
        old_view.extend_from_slice(b"minecraft:brick");
    }
    old_view.push(0);
    for (case, item, name, input) in [
        ("named_front_brick", "brick", Some("wire-side"),
            r#"{sherds:{front:{id:"minecraft:brick",components:{"minecraft:custom_name":"wire-side"}}}}"#),
        ("front_stone", "stone", None, r#"{sherds:{front:{id:"minecraft:stone"}}}"#),
    ] {
        let catalog = crate::gameplay_catalog::Catalog::new(crate::gameplay_catalog::Profile::Preview26_3);
        let mut world = World::in_memory_with_catalog(catalog);
        let at = (-15, 65, -16);
        let state = blocks::state_by_name("decorated_pot").unwrap();
        world.set_block(at.0, at.1 - 1, at.2, blocks::state_by_name("stone").unwrap());
        world.set_block(at.0, at.1, at.2, state);
        crate::pottery::apply_snbt(&mut world, at, input);
        assert_eq!(world.pot(at), Some([0, 0, 0, blocks::item_named(item).unwrap()]));
        let authoritative = world.pot_details(at);
        let pots = world.chunk_pots(-1, -1);
        assert_eq!(pots.len(), 1);
        assert_eq!(pots[0].0, (1, 65, 0));
        let nbt = pots[0].1.clone();
        let kind = redstone::block_entity_type(state).unwrap();
        assert_eq!(kind, 42);
        let world = Mutex::new(world);
        for version in [JavaVersion::V26_3, JavaVersion::V26_1_2] {
            let mut outgoing = Outgoing::capture_content(version, ContentCodec::white_slab_preview());
            send_chunk(&mut outgoing, &world, -1, -1, &ChunkLight::open_sky()).await.unwrap();
            send_block_entity(&mut outgoing, at, kind, &nbt).await.unwrap();
            let mut bytes = outgoing.queued();
            for packet_name in ["map_chunk", "tile_entity_data"] {
                let packet = read_packet(&mut bytes).await.unwrap();
                let canonical_id = if packet_name == "map_chunk" { CHUNK_DATA } else { BLOCK_ENTITY_DATA };
                let expected_id = mcsa_protocol::ids::translate(RegistryKind::PlayClientbound,
                    wire::INTERNAL, version, canonical_id as u32).unwrap();
                assert_eq!(packet.id as u32, expected_id);
                let observed = fixture["observations"].as_array().unwrap().iter().find(|row|
                    row["case"] == case && row["packet"] == packet_name).unwrap();
                let expected = if version == JavaVersion::V26_3 {
                    hex(observed["nbt_hex"].as_str().unwrap())
                } else { old_view.clone() };
                let expected_kind = if version == JavaVersion::V26_3 { 41 } else { 42 };
                let mut cursor = Cursor::new(packet.payload());
                if packet_name == "map_chunk" {
                    assert_eq!(cursor.bytes(8).unwrap(), [(-1i32).to_be_bytes(), (-1i32).to_be_bytes()].concat());
                    assert_eq!(cursor.varint(), Some(0)); // Высотные карты builder не добавляет.
                    let sections = cursor.varint().unwrap() as usize;
                    cursor.bytes(sections).unwrap();
                    assert_eq!(cursor.varint(), Some(1));
                    assert_eq!(cursor.u8(), Some(0x10));
                    assert_eq!(cursor.u16(), Some(65));
                } else {
                    assert_eq!(cursor.position(), Some(at));
                }
                assert_eq!(cursor.varint(), Some(expected_kind));
                assert_eq!(cursor.bytes(expected.len()).unwrap(), expected, "{version:?}/{case}/{packet_name}");
                if packet_name == "tile_entity_data" { assert_eq!(cursor.offset, packet.payload().len()); }
            }
            assert!(bytes.is_empty());
            // Проекция775 не переписывает исходный мир или компоненты предмета.
            let world = world.lock().unwrap();
            assert_eq!(world.pot_details(at), authoritative);
            assert_eq!(world.chunk_pots(-1, -1)[0].1, nbt);
            let details = world.pot_details(at);
            let side = &details.pot_sides[0];
            assert_eq!(side.data.map(crate::stackdata::get).unwrap_or_default().name.as_deref(), name);
        }
    }
}
