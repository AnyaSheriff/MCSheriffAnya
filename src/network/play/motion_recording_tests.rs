// Координаты из tmp/bb/vanilla-*-motion.json, официальный 26.1.2.
// Повторяем входные шаги, а не подгоняем такт под другой клиентский полёт.
use super::*;

#[test]
fn recorded_motion_replays_fall_damage_and_both_particle_counts() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!("../../../tools/blackbox/fixtures/fall-mace-motion-26.1.2.json")).unwrap();
    for (name, expected) in [
        ("fall-mode-teleport-motion", vec![(18,9.0,vec![100]),(73,1.0,vec![40]),(133,3.0,vec![50])]),
        ("pvp-mace-landing-motion", vec![(15,0.0,vec![105]),(77,0.0,vec![160]),(138,1.0,vec![200,40]),(199,2.0,vec![200,40]),(256,0.0,vec![134]),(317,0.0,vec![167])]),
    ] {
        let shared = Shared::new(World::in_memory(), World::in_memory(), Default::default(), Default::default(), Default::default(),
            crate::ops::Ops::open(std::path::Path::new("tmp/motion-ops.json")), crate::skins::Settings::default());
        let mut p = PlayerState::new([72;16],"Motion".into(),None,None,0x7f,0,(3.5,64.0,1.5));
        p.game_mode = if name.starts_with("fall-") { GAME_MODE_CREATIVE } else { GAME_MODE_SURVIVAL };
        p.entity_id = 72;
        shared.world_in(p.dimension).lock().unwrap().set_block_silently(3,63,1,blocks::state_by_name("stone").unwrap());
        let mut actual = Vec::new();
        for row in fixture[name].as_array().unwrap() {
            let tick = row["tick"].as_u64().unwrap();
            shared.ticks.send_replace(tick);
            match row["op"].as_str().unwrap() {
                "teleport" => { position_body(&mut p,Sync::Command); }
                "survival" => vitals::change_game_mode(&mut p,GAME_MODE_SURVIVAL),
                "smash" => {
                    assert!(pvp::may_smash(&p,true),"{name} {tick}: {}",p.vit.fall);
                    // Успешный удар прощает уже накопленный путь; полёт после
                    // него виден в следующих исходящих пакетах записи.
                    p.vit.fall = 0.0;
                    p.vit.smash_landing = true;
                }
                "move" => {
                    let from = (p.x,p.y,p.z);
                    if let Some(y) = row["y"].as_f64() { p.y = y + 123.0; }
                    p.on_ground = row["ground"].as_bool().unwrap();
                    vitals::moved(&mut p,&shared,from);
                    if !p.net.landings.is_empty() || !p.net.hits.is_empty() {
                        let damage: f32 = p.net.hits.drain(..).map(|h| h.amount).sum();
                        let counts: Vec<_> = p.net.landings.drain(..).map(|(_,count,_)| count).collect();
                        actual.push((tick,damage,counts));
                    }
                }
                _ => unreachable!(),
            }
        }
        assert_eq!(actual,expected,"{name}");
    }
}

#[test]
fn java_smash_clears_past_distance_without_immunity_to_future_falls() {
    // pvp-mace-heavy: остаётся 3,15552 блока — без урона; Wind Burst
    // отдельно сохраняет уже сверенную защиту (200 + 90 частиц).
    let mut vit = crate::survival::Vitals::new();
    vit.smash_landing = true;
    assert_eq!(vit.land_java(-3.15552175294311,true,false,1.0),None);
    assert!(!vit.smash_landing);
    assert_eq!(vit.land_java(-4.40739540236494,true,false,1.0),Some(1.0));
    vit.smash_landing = true;
    vit.smash_forgives = true;
    assert_eq!(vit.land_java(-9.5,true,false,1.0),None);
    assert!(!vit.smash_forgives);
    assert_eq!(vit.land_java(-5.40739540236494,true,false,1.0),Some(2.0));
}

#[test]
fn passive_census_keeps_two_spawn_windows_inside_the_recorder_budget() {
    let census: serde_json::Value = serde_json::from_str(include_str!("../../../tools/blackbox/scenarios/superflat-passive-census.json")).unwrap();
    let ticks = census["record_ticks"].as_u64().unwrap();
    assert!((ticks + 13) * 50 < 60_000);
    let actions = census["actions"].as_array().unwrap();
    assert!(actions.iter().all(|a| a["after"].as_u64().unwrap() <= ticks));
    for at in [468,868] {
        assert!(actions.iter().any(|a| a["after"] == at && a["cmd"].as_str().unwrap().starts_with("scoreboard players get #cycle")));
    }
    assert!(census["cleanup"].as_array().unwrap().iter().any(|c| c.as_str().unwrap().contains("fill $(-48,-1,-48) $(48,-1,48) minecraft:air")));
}

#[tokio::test]
async fn mode_change_landing_keeps_recorded_velocity_then_teleport_clears_it() {
    let fixture: serde_json::Value = serde_json::from_str(include_str!("../../../tools/blackbox/fixtures/fall-mace-motion-26.1.2.json")).unwrap();
    let shared = Shared::new(World::in_memory(), World::in_memory(), Default::default(), Default::default(), Default::default(),
        crate::ops::Ops::open(std::path::Path::new("tmp/motion-velocity-ops.json")), crate::skins::Settings::default());
    let mut p = PlayerState::new([73;16],"Velocity".into(),None,None,0x7f,0,(3.5,64.0,1.5));
    p.game_mode = GAME_MODE_CREATIVE;
    p.entity_id = 73;
    shared.world_in(p.dimension).lock().unwrap().set_block_silently(3,63,1,blocks::state_by_name("stone").unwrap());
    let rows = fixture["fall-mode-teleport-motion"].as_array().unwrap();
    let mut velocities = Vec::new();
    for tick in 0..=135 {
        shared.ticks.send_replace(tick);
        for row in rows.iter().filter(|r| r["tick"] == tick) {
            match row["op"].as_str().unwrap() {
                "teleport" => { position_body(&mut p,Sync::Command); }
                "survival" => vitals::change_game_mode(&mut p,GAME_MODE_SURVIVAL),
                "move" => {
                    let from = (p.x,p.y,p.z);
                    if let Some(y) = row["y"].as_f64() { p.y = y + 123.0; }
                    p.on_ground = row["ground"].as_bool().unwrap();
                    vitals::moved(&mut p,&shared,from);
                }
                _ => unreachable!(),
            }
        }
        let mut out = Outgoing::capture();
        vitals::tick(&mut out,&shared,&mut p).await.unwrap();
        let mut bytes = out.queued();
        while !bytes.is_empty() {
            let packet = read_packet(&mut bytes).await.unwrap();
            if packet.id == 0x65 {
                let mut c = Cursor::new(packet.payload());
                assert_eq!(c.varint(),Some(p.entity_id));
                let mut expected = Vec::new();
                types::push_velocity(&mut expected,0.0,if tick == 18 { -1.195 } else { -0.0784 },0.0);
                assert_eq!(c.bytes(expected.len()),Some(expected.as_slice()),"tick {tick}");
                velocities.push(tick);
            }
        }
    }
    // Выходные +74/+134 записи получены после клиентских +73/+133:
    // это граница приёма пакетов; самого второго урона не откладываем.
    assert_eq!(velocities,[18,73,133]);
}

#[test]
fn actual_mace_hits_reset_fall_for_players_and_mobs_without_blanket_immunity() {
    for player_target in [false,true] {
        for wind in [0,1] {
            let shared = Shared::new(World::in_memory(), World::in_memory(), Default::default(), Default::default(), Default::default(),
                crate::ops::Ops::open(std::path::Path::new("tmp/motion-combat-ops.json")), crate::skins::Settings::default());
            shared.ticks.send_replace(100);
            let mut p = PlayerState::new([74;16],"Smash".into(),None,None,0x7f,0,(3.5,66.40739540236494,1.5));
            p.game_mode = GAME_MODE_SURVIVAL;
            p.entity_id = 74;
            p.on_ground = false;
            p.vit.fall = 1.59260459763506;
            let mut data = crate::stackdata::Data::default();
            if wind > 0 { data.set_enchant(crate::stackdata::enchantment_number("wind_burst").unwrap(),wind); }
            p.inventory.set(p.inventory.selected_slot(),Some(Stack::new(blocks::item_named("mace").unwrap(),1).with_extras(data)));
            if player_target {
                let mut victim = PlayerState::new([75;16],"Target".into(),None,None,0x7f,0,(3.5,64.0,3.5));
                victim.entity_id = 75;
                victim.game_mode = GAME_MODE_SURVIVAL;
                victim.on_ground = true;
                shared.players.lock().unwrap().join(victim.member(),75).unwrap();
                publish_player(&shared,&victim);
            } else {
                let mut cow = crate::mob::Mob::new(75,crate::mob::Kind::Cow,(3.5,64.0,3.5));
                cow.no_ai = true;
                cow.on_ground = true;
                shared.items_in(p.dimension).lock().unwrap().add_mob(cow);
            }
            attack(75,&mut p,&shared);
            assert_eq!(p.vit.fall,0.0,"player={player_target}, wind={wind}");
            assert!(p.vit.smash_landing);
            assert_eq!(p.vit.smash_forgives,wind > 0);
            assert_eq!(p.net.velocity,Some((0.0,0.01,0.0)));
        }
    }
}
