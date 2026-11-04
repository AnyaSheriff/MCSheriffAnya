// Проверяем обработчики обеих рук, а не только флаг каталога.
use super::*;
use crate::gameplay_catalog::{Catalog, Profile};

#[test]
fn shield_priority_preserves_ground_and_tool_only_in_preview_without_crouching() {
    for profile in [Profile::Java26_1_2, Profile::Preview26_3] {
        for tool in ["iron_hoe", "iron_shovel"] {
            for sneaking in [false, true] {
                for hand in [0, 1] {
                    let catalog = Catalog::new(profile);
                    let shared = Shared::new(World::in_memory_with_catalog(catalog), World::in_memory_with_catalog(catalog),
                        Default::default(), Default::default(), Default::default(),
                        crate::ops::Ops::open(Path::new("tmp/changed-rules-ops.json")), crate::skins::Settings::default());
                    let mut p = PlayerState::new_for_catalog([231;16], "Rules".into(), None, None, 0, 231, (3.5,64.,5.5), catalog);
                    p.game_mode = GAME_MODE_SURVIVAL;
                    p.sneaking = sneaking;
                    let tool_slot = if hand == 0 { p.inventory.selected_slot() } else { inventory::OFFHAND };
                    let shield_slot = if hand == 0 { inventory::OFFHAND } else { p.inventory.selected_slot() };
                    let held = Stack::new(blocks::item_named(tool).unwrap(), 1);
                    p.inventory.set(tool_slot, Some(held));
                    p.inventory.set(shield_slot, Some(Stack::new(blocks::item_named("shield").unwrap(), 1)));
                    {
                        let mut w = shared.world_in(p.dimension).lock().unwrap();
                        w.ensure(0, 0);
                        w.set_block_silently(3,63,3, blocks::state_by_name("grass_block").unwrap());
                    }
                    let mut bytes = encode_varint(hand);
                    push_packed_position(&mut bytes, 3, 63, 3);
                    bytes.extend(encode_varint(1));
                    for _ in 0..3 { bytes.extend(0.5f32.to_be_bytes()); }
                    bytes.push(0);
                    bytes.extend(encode_varint(17));
                    let reaction = place_block(&bytes, &mut p, &shared).unwrap();
                    assert_eq!(reaction.acknowledge, Some(17));
                    let blocked = profile == Profile::Preview26_3 && !sneaking;
                    let w = shared.world_in(p.dimension).lock().unwrap();
                    assert_eq!(blocks::block_at_state(w.get_block(3,63,3)), Some(if blocked { "grass_block" } else if tool.ends_with("hoe") { "farmland" } else { "dirt_path" }));
                    assert_eq!(p.inventory.slot(tool_slot).unwrap().extras().damage, if blocked { 0 } else { 1 });
                    assert_eq!(blocks::item_name(p.inventory.slot(shield_slot).unwrap().item), Some("shield"));
                }
            }
        }
    }
}

fn rules_shared() -> Shared {
    let catalog = Catalog::new(Profile::Preview26_3);
    Shared::new(World::in_memory_with_catalog(catalog),World::in_memory_with_catalog(catalog),Default::default(),Default::default(),Default::default(),crate::ops::Ops::open(Path::new("tmp/changed-rules-ops.json")),crate::skins::Settings::default())
}

#[test]
fn gateway_commands_persist_destination_and_spectator_enters_immediately() {
    let shared = rules_shared();
    shared.world_in(Dimension::Overworld).lock().unwrap().ensure(0,0);
    let answer = crate::commands::run("setblock 3 64 5 end_gateway{exit_portal:[I;8,70,9],ExactTeleport:1b}",&crate::commands::Source::Console,&shared);
    assert!(answer.result.is_some());
    assert_eq!(crate::portal::travel(&shared,Dimension::Overworld,(3.5,64.,5.5)),(Dimension::Overworld,(8.5,70.5,9.5)));
    let mut p = PlayerState::new_for_catalog([239;16],"Gateway".into(),None,None,0,239,(3.5,64.,5.5),shared.catalog());
    p.game_mode = GAME_MODE_SPECTATOR; p.portal_ready = true; p.moved_since_tp = true;
    step_portal(&shared,&mut p);
    assert!(!p.portal_ready);
    assert!(p.progress.adv.is_done("end/enter_end_gateway"));
}

#[test]
fn vex_owner_is_command_executor_and_missing_owner_produces_no_execution() {
    let shared = rules_shared();
    let owner = crate::mob::Mob::new(234,crate::mob::Kind::Evoker,(2.,64.,2.));
    let mut vex = crate::mob::Mob::new(235,crate::mob::Kind::Vex,(2.,65.,2.)); vex.h.owner = owner.entity_id;
    { let mut items = shared.items_in(Dimension::Overworld).lock().unwrap(); items.add_mob(owner); items.add_mob(vex); }
    let answer = crate::commands::run("execute as @e[type=vex] on owner run rotate @s 37 12",&crate::commands::Source::Console,&shared);
    assert_eq!(answer.result,Some(1));
    {
        let items = shared.items_in(Dimension::Overworld).lock().unwrap();
        assert_eq!(items.mobs().iter().find(|m|m.kind==crate::mob::Kind::Evoker).unwrap().yaw,37.0);
        assert_eq!(items.mobs().iter().find(|m|m.kind==crate::mob::Kind::Vex).unwrap().yaw,0.0);
    }
    { let mut items = shared.items_in(Dimension::Overworld).lock().unwrap(); items.mobs_mut().iter_mut().find(|m|m.kind==crate::mob::Kind::Vex).unwrap().h.owner = 0; }
    assert_eq!(crate::commands::run("execute as @e[type=vex] on owner run rotate @s 37 12",&crate::commands::Source::Console,&shared).result,None);
}

#[test]
fn advancement_feedback_counts_actual_changes_before_network_orders_are_applied() {
    let shared = rules_shared();
    for id in [237,238] {
        shared.players.lock().unwrap().join(crate::players::Member {
            uuid:[id;16],name:format!("Rule{id}"),game_mode:0,entity_id:id as i32,skin:None,skin_parts:0x7f,main_hand:1,held:None,
            position:crate::players::Position{x:0.,y:64.,z:0.,yaw:0.,pitch:0.,on_ground:true},dimension:Dimension::Overworld,sneaking:false,swings:0,shoved:((0.,0.,0.),0),unsettled:0,
        },id as i32).unwrap();
        shared.advancement_states.lock().unwrap().insert([id;16],(crate::advancements::AdvState::default(),0));
    }
    let command = |s| crate::commands::run(s,&crate::commands::Source::Console,&shared);
    assert_eq!(command("advancement grant Rule237 only minecraft:story/root").result,Some(1));
    assert_eq!(command("advancement grant @a only minecraft:story/root").result,Some(1));
    assert_eq!(command("advancement grant @a only minecraft:story/root").result,None);
    assert_eq!(command("advancement revoke @a only minecraft:story/root").result,Some(2));
    assert_eq!(command("advancement revoke @a only minecraft:story/root").result,None);
}
