use super::*;
use crate::mob::{Kind,Mob};
fn shared()->Shared {
    Shared::new(World::in_memory(),World::in_memory(),Default::default(),Default::default(),Default::default(),
        crate::ops::Ops::open(std::path::Path::new("target/land-test-ops.json")),crate::skins::Settings::default())
}
fn player(id:i32)->PlayerState {
    let mut p=PlayerState::new([17;16],"LandTest".into(),None,None,0x7f,0,(3.5,64.0,5.5)); p.entity_id=id; p.game_mode=GAME_MODE_SURVIVAL; p
}
fn held(p:&mut PlayerState,name:&str,count:i32) { p.inventory.set(p.inventory.selected_slot(),Some(Stack::new(blocks::item_named(name).unwrap(),count))); }
fn add(s:&Shared,k:Kind)->i32 {
    let id=s.next_entity_id(); let mut m=Mob::new(id,k,(3.5,64.0,3.5)); m.no_ai=true;
    s.items_in(Default::default()).lock().unwrap().add_mob(m); id
}
#[test]
fn all_nine_land_eggs_and_summons_resolve() {
    for k in [Kind::Llama,Kind::TraderLlama,Kind::WanderingTrader,Kind::Camel,Kind::Goat,Kind::PolarBear,Kind::Panda,Kind::Ocelot,Kind::Mooshroom] {
        let s=shared(); let mut p=player(1000); let egg=format!("{}_spawn_egg",k.name()); held(&mut p,&egg,2);
        let item=p.inventory.held().unwrap().item;
        use_spawn_egg(item,(3,63,3),1,1,&mut p,&s).unwrap();
        assert_eq!(p.inventory.held().unwrap().count,1);
        assert_eq!(s.items_in(p.dimension).lock().unwrap().mobs()[0].kind,k);
        crate::commands::run(&format!("summon {} 3.5 64 3.5 {{NoAI:1b}}",k.name()),&crate::commands::Source::Console,&s);
        assert_eq!(s.items_in(p.dimension).lock().unwrap().mobs().len(),2);
    }
}
#[test]
fn mooshroom_shearing_preserves_name_health_and_drops_five_mushrooms() {
    let s=shared(); let mut p=player(1000); let id=add(&s,Kind::Mooshroom);
    { let mut items=s.items_in(p.dimension).lock().unwrap(); let m=&mut items.mobs_mut()[0]; m.land.brown=true; m.name=Some("Mush".into()); m.vitals.health=6.0; }
    held(&mut p,"shears",1); assert!(use_mob(&s,&mut p,id,&mut Reaction::default()));
    let items=s.items_in(p.dimension).lock().unwrap(); let cow=&items.mobs()[0];
    assert_eq!(cow.kind,Kind::Cow); assert_ne!(cow.entity_id,id); assert_eq!(cow.name.as_deref(),Some("Mush")); assert_eq!(cow.vitals.health,6.0);
    assert_eq!(items.items().iter().filter(|d| blocks::item_name(d.stack.item)==Some("brown_mushroom")).map(|d| d.stack.count).sum::<i32>(),5);
    assert_eq!(p.inventory.held().unwrap().extras().damage,1);
}
#[test]
fn baby_mooshroom_cannot_be_milked_or_sheared() {
    let s=shared(); let mut p=player(1000); let id=add(&s,Kind::Mooshroom);
    s.items_in(p.dimension).lock().unwrap().mobs_mut()[0].age= -100;
    for item in ["bucket","bowl","shears"] { held(&mut p,item,1); use_mob(&s,&mut p,id,&mut Reaction::default()); assert_eq!(blocks::item_name(p.inventory.held().unwrap().item),Some(item)); }
    assert_eq!(s.items_in(p.dimension).lock().unwrap().mobs()[0].kind,Kind::Mooshroom);
}
#[test]
fn brown_mooshroom_flower_is_consumed_once_then_stew_resets() {
    let s=shared(); let mut p=player(1000); let id=add(&s,Kind::Mooshroom);
    s.items_in(p.dimension).lock().unwrap().mobs_mut()[0].land.brown=true;
    held(&mut p,"cornflower",2); use_mob(&s,&mut p,id,&mut Reaction::default()); assert_eq!(p.inventory.held().unwrap().count,1);
    use_mob(&s,&mut p,id,&mut Reaction::default()); assert_eq!(p.inventory.held().unwrap().count,1);
    held(&mut p,"bowl",1); use_mob(&s,&mut p,id,&mut Reaction::default());
    let stew=p.inventory.slots().iter().flatten().find(|s| blocks::item_name(s.item)==Some("suspicious_stew")).unwrap(); assert_eq!(stew.extras().stew,vec![(7,100)]);
    assert_eq!(s.items_in(p.dimension).lock().unwrap().mobs()[0].land.flower,0);
}
#[test]
fn tamed_llama_chest_uses_strength_and_cannot_be_added_twice() {
    let s=shared(); let mut p=player(1000); let id=add(&s,Kind::Llama);
    { let mut items=s.items_in(p.dimension).lock().unwrap(); let m=&mut items.mobs_mut()[0]; m.land.tamed=true; m.land.strength=5; }
    held(&mut p,"chest",2); use_mob(&s,&mut p,id,&mut Reaction::default());
    assert_eq!(p.inventory.held().unwrap().count,1);
    use_mob(&s,&mut p,id,&mut Reaction::default()); assert_eq!(p.inventory.held().unwrap().count,1);
    let items=s.items_in(p.dimension).lock().unwrap(); let m=&items.mobs()[0]; assert_eq!(m.land.cargo.len(),15);
    let mut slots=land::slots(m); assert_eq!(slots.len(),17); slots[0]=Some(Stack::new(blocks::item_named("stone").unwrap(),1)); assert!(!land::valid(m,&slots));
}
#[test]
fn camel_mounts_two_players_and_rejects_a_third() {
    let s=shared(); let id=add(&s,Kind::Camel);
    for n in 1000..1003 { let mut p=player(n); use_mob(&s,&mut p,id,&mut Reaction::default()); }
    assert_eq!(s.riding.lock().unwrap().passengers(id),&[1000,1001]);
}
#[test]
fn wandering_trader_uses_merchant_offers_and_locks_one_customer() {
    let s=shared(); let mut p=player(1000); let id=add(&s,Kind::WanderingTrader); let mut r=Reaction::default();
    use_mob(&s,&mut p,id,&mut r); assert_eq!(r.open_merchant,Some(id));
    let mut second=player(1001); let mut r2=Reaction::default(); use_mob(&s,&mut second,id,&mut r2); assert!(r2.open_merchant.is_none());
    let items=s.items_in(p.dimension).lock().unwrap(); assert_eq!(items.mobs()[0].villager.offers.len(),9);
}

#[test]
fn milking_replaces_the_selected_bucket_and_creative_does_not_duplicate() {
    let s=shared(); let mut p=player(1000); let id=add(&s,Kind::Goat); p.inventory.select(5);
    held(&mut p,"bucket",1); use_mob(&s,&mut p,id,&mut Reaction::default());
    assert_eq!(blocks::item_name(p.inventory.held().unwrap().item),Some("milk_bucket"));
    p.game_mode=GAME_MODE_CREATIVE; held(&mut p,"bucket",1);
    for _ in 0..3 { use_mob(&s,&mut p,id,&mut Reaction::default()); }
    assert_eq!(blocks::item_name(p.inventory.held().unwrap().item),Some("bucket"));
    assert_eq!(p.inventory.slots().iter().flatten().filter(|s| blocks::item_name(s.item)==Some("milk_bucket")).count(),1);
}
