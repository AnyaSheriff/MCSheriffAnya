//! Обычная команда через run: права, single-selector, снимок и execute store.
use super::*;
use crate::players::{Member,Position};
use crate::world::World;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize,Ordering};
struct Fixture { shared:Shared, root:PathBuf }
impl Drop for Fixture { fn drop(&mut self) { let _=std::fs::remove_dir_all(&self.root); } }
fn fixture(count:u8)->Fixture {
    static NEXT:AtomicUsize=AtomicUsize::new(0);
    let root=std::env::temp_dir().join(format!("mcsa-data-get-{}-{}",std::process::id(),NEXT.fetch_add(1,Ordering::Relaxed)));
    std::fs::create_dir(&root).unwrap();
    let shared=Shared::new(World::in_memory(),World::in_memory(),root.join("players"),root.join("skins"),Default::default(),ops::Ops::open(&root.join("ops.json")),crate::skins::Settings::default());
    for id in 1..=count {
        shared.players.lock().unwrap().join(Member {
            uuid:[id;16], name:format!("Reader{id}"),game_mode:0,entity_id:i32::from(id),skin:None,skin_parts:0x7f,main_hand:1,held:None,
            position:Position{x:-1.25,y:65.0625,z:-0.0,yaw:10.,pitch:-2.,on_ground:false},dimension:Dimension::Overworld,sneaking:false,swings:0,shoved:((0.,0.,0.),0),unsettled:0,
        },i32::from(id)).unwrap();
    }
    Fixture{shared,root}
}
fn player()->Source { Source::Player {uuid:[1;16],name:"Reader1".into()} }
#[test]
fn named_readback_formats_actual_values_and_preserves_state() {
    let f=fixture(1);let before=f.shared.players.lock().unwrap().find("Reader1").unwrap();
    let pos=run("data get entity reader1 Pos",&Source::Console,&f.shared);
    assert_eq!(pos.reply,"Reader1 has the following entity data: [-1.25d, 65.0625d, -0.0d]");
    assert_eq!(pos.result,Some(3));assert!(pos.component.is_some());assert!(matches!(pos.effect,Effect::None));
    assert!(pos.component.as_ref().unwrap().windows(b"commands.data.entity.query".len()).any(|w|w==b"commands.data.entity.query"));
    let off=run("data get entity Reader1 OnGround",&Source::Console,&f.shared);
    assert_eq!(off.reply,"Reader1 has the following entity data: 0b");assert_eq!(off.result,Some(0));
    let after=f.shared.players.lock().unwrap().find("Reader1").unwrap();assert_eq!(before.position,after.position);assert_eq!(before.game_mode,after.game_mode);assert_eq!(before.held,after.held);
    let mut accepted=after.position;accepted.x=2.5;accepted.on_ground=true;f.shared.players.lock().unwrap().set_position(&[1;16],accepted);
    let now=run("data get entity Reader1 OnGround",&Source::Console,&f.shared);assert_eq!(now.result,Some(1));assert!(now.reply.ends_with("1b"));
    assert!(run("data get entity Reader1 Pos",&Source::Console,&f.shared).reply.contains("2.5d"));

}
#[test]
fn selector_syntax_is_single_even_when_only_one_player_online() {
    let f=fixture(1);
    for target in ["@a","@a[name=Reader1]","@a[limit=2]","@p[limit=2]","@s[limit=2]","@e[type=player]","@e[limit=1]","@e[type=cow,limit=1]","@e[type=!player,limit=1]","@a[limit=1,limit=1]","@a[limit=0]","@a[limit=-1]","@r[limit=1]","@a[limit=1,sort=furthest]","@a[limit=1,sort=random]","@a[limit=1,sort=arbitrary,sort=arbitrary]","@e[type=player,type=cow,limit=1]"] {
        let answer=run(&format!("data get entity {target} Pos"),&Source::Console,&f.shared);assert_eq!(answer.result,None,"{target}");assert!(answer.component.is_some());
    }
    for target in ["Reader1","@p","@r","@a[name=Reader1,limit=1]","@e[type=player,limit=1]","@e[type=minecraft:player,limit=1]"] {
        assert_eq!(run(&format!("data get entity {target} Pos"),&Source::Console,&f.shared).result,Some(3),"{target}");
    }
    assert_eq!(run("data get entity @s Pos",&Source::Console,&f.shared).result,None);
    let two=fixture(2);
    assert_eq!(run("data get entity @a Pos",&Source::Console,&two.shared).result,None);
    assert_eq!(run("data get entity @a[name=Reader2,limit=1] OnGround",&Source::Console,&two.shared).reply,"Reader2 has the following entity data: 0b");
}
#[test]
fn normal_permission_gate_and_unsupported_forms_fail_without_mutation() {
    let f=fixture(1);let before=f.shared.players.lock().unwrap().find("Reader1").unwrap().position;
    assert_eq!(run("data get entity Reader1 Pos",&player(),&f.shared).result,None);
    f.shared.ops.lock().unwrap().give(&[1;16],"Reader1",ops::HIGHEST_LEVEL-1);
    assert_eq!(run("data get entity Reader1 Pos",&player(),&f.shared).result,None,"прежний уровень data не понижен");
    f.shared.ops.lock().unwrap().give(&[1;16],"Reader1",ops::HIGHEST_LEVEL);
    assert_eq!(run("data get entity @s Pos",&player(),&f.shared).result,Some(3));
    for command in ["data get entity Reader1","data get entity Reader1 Pos 2","data get entity Reader1 Pos extra","data get entity Reader1 Pos[0]","data get entity Reader1 Health","data get block 0 64 0 Pos","data get entity Missing Pos","data get entity @a[limit=1,name=Missing] Pos","data get entity @p[bogus=1] Pos"] {
        let answer=run(command,&Source::Console,&f.shared);assert_eq!(answer.result,None,"{command}");assert!(matches!(answer.effect,Effect::None));assert!(answer.component.is_some());
    }
    assert_eq!(f.shared.players.lock().unwrap().find("Reader1").unwrap().position,before);
}
#[test]
fn zero_byte_is_success_and_execute_store_distinguishes_result_from_success() {
    let f=fixture(1);
    assert!(run("scoreboard objectives add probe dummy",&Source::Console,&f.shared).result.is_some());
    f.shared.ops.lock().unwrap().give(&[1;16],"Reader1",2);
    let denied=run("execute store success score denied probe run data get entity @s Pos",&player(),&f.shared);
    assert_eq!(denied.result,None);
    assert_eq!(f.shared.scoreboard.lock().unwrap().score("denied","probe").unwrap().value,0);
    for (holder,kind,path,expected) in [("ground_result","result","OnGround",0),("ground_success","success","OnGround",1),("pos_result","result","Pos",3),("bad_success","success","Missing",0),("bad_result","result","Missing",0)] {
        let answer=run(&format!("execute store {kind} score {holder} probe run data get entity Reader1 {path}"),&Source::Console,&f.shared);
        assert_eq!(answer.result.is_some(),path!="Missing");
        assert_eq!(f.shared.scoreboard.lock().unwrap().score(holder,"probe").unwrap().value,expected);
    }
}
