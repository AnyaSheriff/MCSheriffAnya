//! Отдельный процесс на cohort; реальные worker и обработчик, без сервера.
use super::*;
use crate::plugins::place_baseline_worker as worker;
use serde_json::{Value, json};

fn hex(bytes: &[u8]) -> String { bytes.iter().map(|b| format!("{b:02x}")).collect() }
fn messages(shared: &Shared, uuid: &[u8;16]) -> Vec<String> {
    shared.players.lock().unwrap().take_orders(uuid).into_iter().filter_map(|o| match o {
        Order::Message { component } => Some(hex(&component)), _ => None,
    }).collect()
}
fn stacks(player: &PlayerState) -> Value {
    json!({"slots":player.inventory.slots().iter().map(|s| s.map(|s| json!({"item":s.item,"count":s.count,"tnt":format!("{:?}",s.tnt),"data_id":format!("{:?}",s.data),"extras_full_debug":format!("{:?}",s.extras())}))).collect::<Vec<_>>(),"cursor":format!("{:?}",player.inventory.cursor()),"selected":player.inventory.selected()})
}

#[tokio::test]
#[ignore = "только отдельные CI процессы с двумя собственными WASM"]
async fn place_baseline_cohort() {
    let id = std::env::var("MCSA_PLACE_CASE").expect("не задан cohort");
    let spec: Value = serde_json::from_str(include_str!("../../../tools/plugin-translator/fixtures/place-priority-expected.json")).unwrap();
    let (kind, name) = id.split_once(':').unwrap();
    let native = kind == "native";
    assert!(native || kind == "host");
    let suffix = name.strip_prefix("offhand-").or_else(|| name.strip_prefix("creative-")).unwrap_or(name);
    let special = suffix != name;
    let cohort_name = if special { format!("a-b-{suffix}") } else { name.to_string() };
    let reference = spec["cohorts"].as_array().unwrap().iter().find(|v| v["name"] == cohort_name).unwrap();
    let letters: Vec<String> = reference["load_order"].as_array().unwrap().iter().map(|s|s.as_str().unwrap().to_owned()).collect();
    let mode = reference["mode"].as_str().unwrap().to_string();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tmp/place-baseline/work").join(format!("{}-{}", name, std::process::id()));
    assert!(!root.exists()); std::fs::create_dir_all(&root).unwrap();
    let catalog = crate::gameplay_catalog::Catalog::new(crate::gameplay_catalog::Profile::Preview26_3);
    let shared = Arc::new(Shared::new(World::in_memory_with_catalog(catalog), World::in_memory_with_catalog(catalog), Default::default(), Default::default(), Default::default(), crate::ops::Ops::open(&root.join("ops.json")), crate::skins::Settings::default()));
    let mut player = PlayerState::new_for_catalog([219;16], "PlaceBaseline".into(), None, None, 0x7f, 219, (3.5,64.0,5.5),catalog);
    player.game_mode = if name.starts_with("creative-") {GAME_MODE_CREATIVE} else {GAME_MODE_SURVIVAL};
    player.on_ground = true;
    let stone = blocks::item_named("stone").unwrap();
    let used = Stack::new(stone,2).with_extras(crate::stackdata::Data {name:Some("used-place-stack".into()),damage:7,unbreakable:true,..Default::default()});
    let other = Stack::new(blocks::item_named("dirt").unwrap(),5).with_extras(crate::stackdata::Data {name:Some("opposite-stack".into()),..Default::default()});
    let hand = i32::from(name.starts_with("offhand-"));
    player.inventory.set(36,Some(if hand==0 {used} else {other}));
    player.inventory.set(45,Some(if hand==0 {other} else {used}));
    shared.players.lock().unwrap().join(crate::players::Member {uuid:player.uuid,name:player.name.clone(),game_mode:player.game_mode,entity_id:219,skin:None,skin_parts:0x7f,main_hand:1,held:player.inventory.held(),position:player.position(),dimension:player.dimension,sneaking:false,swings:0,shoved:((0.0,0.0,0.0),0),unsettled:0},1);
    {
        let mut world=shared.world_in(player.dimension).lock().unwrap();world.ensure(0,0);
        let stone=blocks::state_by_name("stone").unwrap();
        world.set_block_silently(3,63,3,stone);
        world.set_block_silently(3,64,3,world::AIR);
        // Отдельная опора и пустой объём игрока: terrain не определяет verdict.
        for x in 2..=4 { for z in 4..=6 { for y in 64..=66 {world.set_block_silently(x,y,z,world::AIR);} } }
        world.set_block_silently(3,63,5,stone);
        assert_eq!(world.get_block(3,63,5),stone);
        for x in 2..=4 { for z in 4..=6 { for y in 64..=66 {assert_eq!(world.get_block(x,y,z),world::AIR);} } }
        assert_eq!([world.get_block(3,63,3),world.get_block(3,64,3)],[stone,world::AIR]);
        assert!(player.z-0.3>=4.0,"AABB игрока пересекает target z=[3,4]");
    }
    worker::start(Arc::clone(&shared),root.clone());
    assert!(!crate::plugins::active());
    for letter in &letters {
        let plugin=format!("java-place-priority-{letter}");
        let folder=root.join("plugins").join(&plugin);std::fs::create_dir_all(folder.join("data")).unwrap();
        for file in ["plugin.toml","plugin.wasm"] {std::fs::copy(Path::new(env!("CARGO_MANIFEST_DIR")).join("tmp/place-baseline/plugins").join(&plugin).join(file),folder.join(file)).unwrap();}
        worker::load(plugin);
    }
    assert!(crate::plugins::active());
    assert_eq!(worker::count(),letters.len());
    publish_player(&shared,&player);publish_ui(&shared,&player);
    assert!(shared.players.lock().unwrap().ui.info.contains_key(&player.uuid));
    crate::plugins::player_join(&shared,&player.name);
    messages(&shared,&player.uuid);
    for letter in &letters {
        assert!(worker::command(plugin_view(&player),format!("placepriority{letter}"),mode.clone()));
        assert_eq!(messages(&shared,&player.uuid),[hex(&crate::network::nbt::text_component(&format!("mode {} {mode}",letter.to_ascii_uppercase())))]);
    }
    let before=stacks(&player);
    let stone_state=blocks::state_by_name("stone").unwrap();
    let before_cells={let w=shared.world_in(player.dimension).lock().unwrap();vec![w.get_block(3,63,3),w.get_block(3,64,3)]};
    assert_eq!(before_cells,[stone_state,world::AIR]);
    let mut input=encode_varint(hand);push_packed_position(&mut input,3,63,3);input.extend(encode_varint(1));for _ in 0..3 {input.extend(0.5f32.to_be_bytes());} input.push(0);input.extend(encode_varint(41));
    let (verdict,reaction,wire)=if native {
        let reaction=place_block(&input,&mut player,&shared).unwrap();
        assert_eq!(reaction.acknowledge,Some(41));
        let after=shared.world_in(player.dimension).lock().unwrap().get_block(3,64,3);
        let verdict=if after==stone_state {"allow"} else {assert_eq!(after,world::AIR);"deny"};
        let mut out=Outgoing::capture_content(mcsa_protocol::JavaVersion::V26_3,catalog.content_codec());
        apply_reaction(&mut out,&shared,&mut player,&reaction).await.unwrap();
        let bytes=out.queued().to_vec();
        (verdict,json!({"acknowledge":reaction.acknowledge,"confirm":reaction.confirm,"slots":reaction.slots,"wire_protocol":777}),hex(&bytes))
    } else {
        let view=plugin_view(&player);
        let verdict=worker::place(view);
        (if verdict==crate::plugins::Verdict::Allow {"allow"} else {"deny"},Value::Null,String::new())
    };
    let raw=messages(&shared,&player.uuid);
    assert_eq!(worker::count(),letters.len(),"fixture был выгружен при вызове");
    assert!(!raw.is_empty(),"fixture не вызван");
    let after=stacks(&player);
    let after_cells={let w=shared.world_in(player.dimension).lock().unwrap();vec![w.get_block(3,63,3),w.get_block(3,64,3)]};
    let output=json!({"id":id,"cohort":cohort_name,"kind":kind,"pid":std::process::id(),"load_order":letters,"mode":mode,"verdict":verdict,"raw_message_nbt_hex":raw,"uuid":crate::network::login::format_uuid(&player.uuid),"native_input_hex":hex(&input),"native_hand":hand,"event_hand_item_against_replaced":"UNSUPPORTED","before_stacks":before,"after_stacks":after,"before_cells":before_cells,"after_cells":after_cells,"reaction":reaction,"wire_hex":wire,"geometry":{"target_explicit_air":true,"actor_body_clear":[[2,64,4],[4,66,6]],"actor_support":[3,63,5],"actor_aabb":[[player.x-0.3,player.y,player.z-0.3],[player.x+0.3,player.y+1.8,player.z+0.3]],"target_aabb":[[3,64,3],[4,65,4]],"collision_free_asserted":true},"ready_java_view":true,"actual_worker":true,"sdk_canBuild_is_native_permission":false,"denial_claim":"commit prevention only"});
    let capture=Path::new(env!("CARGO_MANIFEST_DIR")).join("tmp/place-baseline/capture");std::fs::create_dir_all(&capture).unwrap();
    let path=capture.join(format!("{}-{}.json",kind,name));assert!(!path.exists());std::fs::write(path,serde_json::to_vec_pretty(&output).unwrap()).unwrap();println!("PLACE_BASELINE {output}");
    crate::plugins::shutdown();assert!(!crate::plugins::active());
    if native {
        assert_eq!(before_cells[0],after_cells[0]);
        if verdict=="deny" {assert_eq!(before,after);assert_eq!(before_cells,after_cells);} else {
            let used_slot=if hand==0 {36} else {45};let opposite=if hand==0 {45} else {36};
            assert_eq!(player.inventory.slot(opposite),Some(other));
            assert_eq!(player.inventory.slot(used_slot),Some(used.with_count(if player.game_mode==GAME_MODE_CREATIVE {2} else {1})));
        }
    } else {assert_eq!(before,after);assert_eq!(before_cells,after_cells);}
}
