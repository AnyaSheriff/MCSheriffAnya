use super::*;
use crate::mob::{Kind,Mob};
fn shared()->Shared {
    Shared::new(World::in_memory(),World::in_memory(),Default::default(),Default::default(),Default::default(),crate::ops::Ops::open(std::path::Path::new("target/water-test-ops.json")),crate::skins::Settings::default())
}
fn player()->PlayerState {
    let mut p=PlayerState::new([22;16],"WaterTest".into(),None,None,0x7f,11,(3.5,64.0,5.5));
    p.game_mode=GAME_MODE_SURVIVAL; p
}

#[test]
fn every_water_mob_has_working_spawn_egg_and_summon() {
    for kind in Kind::ALL.into_iter().filter(|k|crate::mob::water::creature(*k)) {
        let shared=shared(); let mut p=player();
        let egg=blocks::item_named(&format!("{}_spawn_egg",kind.name())).unwrap();
        p.inventory.set(inventory::FIRST_HOTBAR,Some(Stack::new(egg,2)));
        use_spawn_egg(egg,(3,63,3),1,1,&mut p,&shared).unwrap();
        assert_eq!(shared.items_in(p.dimension).lock().unwrap().mobs()[0].kind,kind);
        assert_eq!(p.inventory.held().unwrap().count,1);
        let answer=crate::commands::run(&format!("summon {} 3.5 64 3.5 {{NoAI:1b}}",kind.name()),&crate::commands::Source::Console,&shared);
        assert!(answer.reply.contains(kind.name()),"{}",answer.reply);
        assert_eq!(shared.items_in(p.dimension).lock().unwrap().mobs().len(),2);
    }
}
#[test]
fn capture_and_release_preserve_fish_data_in_actual_handlers() {
    for kind in [Kind::Cod,Kind::Salmon,Kind::TropicalFish,Kind::Pufferfish,Kind::Axolotl,Kind::Tadpole] {
        let shared=shared(); let mut p=player();
        let mut mob=Mob::new(100,kind,(3.5,64.0,3.5)); mob.no_ai=true;
        mob.name=Some("Fin".into()); mob.water.variant=if kind==Kind::TropicalFish {185008129} else {4}; mob.water.salmon_size=2;
        shared.items_in(p.dimension).lock().unwrap().add_mob(mob);
        p.inventory.set(inventory::FIRST_HOTBAR,Some(Stack::new(blocks::item_named("water_bucket").unwrap(),1)));
        let mut reaction=Reaction::default(); assert!(use_mob(&shared,&mut p,100,&mut reaction));
        let filled=p.inventory.held().unwrap();
        assert_eq!(blocks::item_name(filled.item),crate::mob::water::bucket(kind));
        assert!(shared.items_in(p.dimension).lock().unwrap().mobs().is_empty());
        use_bucket(filled.item,(3,63,3),1,1,&mut p,&shared).unwrap();
        let items=shared.items_in(p.dimension).lock().unwrap();
        assert_eq!(items.mobs().len(),1);
        let released=&items.mobs()[0]; assert_eq!(released.kind,kind); assert!(released.water.from_bucket && released.no_ai);
        assert_eq!(released.name.as_deref(),Some("Fin"));
        if kind==Kind::Salmon {assert_eq!(released.water.salmon_size,2);}
        if kind==Kind::TropicalFish {assert_eq!(released.water.variant,185008129);}
        assert_eq!(blocks::item_name(p.inventory.held().unwrap().item),Some("bucket"));
    }
}
#[test]
fn fish_bucket_does_not_replace_a_solid_target_or_consume_itself() {
    let shared=shared(); let mut p=player();
    let item=blocks::item_named("cod_bucket").unwrap(); p.inventory.set(inventory::FIRST_HOTBAR,Some(Stack::new(item,1)));
    let stone=blocks::state_by_name("stone").unwrap();
    {let mut world=shared.world_in(p.dimension).lock().unwrap(); world.set_block_silently(3,63,3,stone);world.set_block_silently(3,64,3,stone);}
    use_bucket(item,(3,63,3),1,1,&mut p,&shared).unwrap();
    assert!(shared.items_in(p.dimension).lock().unwrap().mobs().is_empty());
    assert_eq!(p.inventory.held().unwrap().item,item);
    assert_eq!(shared.world_in(p.dimension).lock().unwrap().get_block(3,64,3),stone);
}
#[test]
fn axolotl_food_leaves_water_bucket_and_tadpole_food_accelerates_growth() {
    let shared=shared(); let mut p=player();
    shared.items_in(p.dimension).lock().unwrap().add_mob(Mob::new(100,Kind::Axolotl,(3.5,64.0,3.5)));
    p.inventory.set(inventory::FIRST_HOTBAR,Some(Stack::new(blocks::item_named("tropical_fish_bucket").unwrap(),1)));
    use_mob(&shared,&mut p,100,&mut Reaction::default());
    assert_eq!(blocks::item_name(p.inventory.held().unwrap().item),Some("water_bucket"));
    assert_eq!(shared.items_in(p.dimension).lock().unwrap().mobs()[0].love,crate::mob::LOVE_TICKS);
    shared.items_in(p.dimension).lock().unwrap().add_mob(Mob::new(101,Kind::Tadpole,(3.5,64.0,3.5)));
    p.inventory.set(inventory::FIRST_HOTBAR,Some(Stack::new(blocks::item_named("slime_ball").unwrap(),1)));
    use_mob(&shared,&mut p,101,&mut Reaction::default());
    let items=shared.items_in(p.dimension).lock().unwrap();
    assert_eq!(items.mobs()[1].water.growth,2400); assert_eq!(items.mobs()[1].love,0);
}
#[test]
fn frog_variants_have_registered_metadata_and_bucket_components_are_encoded() {
    let mut frog=Mob::new(1,Kind::Frog,(0.0,64.0,0.0));
    for (variant,registry) in [(0,1),(1,2),(2,0)] {
        frog.water.variant=variant;
        let props=mobs::water_test_properties(&frog);
        assert!(props.iter().any(|(key,ty,value)|*key==18 && *ty==27 && *value==encode_varint(registry)));
    }
    let mut salmon=Mob::new(1,Kind::Salmon,(0.0,64.0,0.0)); salmon.water.salmon_size=2;
    let data=crate::mob::water::bucket::capture(&salmon);
    let (count,bytes)=crate::stackdata::encode_components(&data);
    assert_eq!(count,2); assert_eq!(bytes[0],59);
    let mut decoded=crate::stackdata::Data::default();
    crate::stackdata::read_component(59,&data.bucket.unwrap().nbt(),&mut decoded);
    crate::stackdata::read_component(86,&encode_varint(2),&mut decoded);
    let mut again=Mob::new(2,Kind::Salmon,(0.0,64.0,0.0)); crate::mob::water::bucket::release(&mut again,&decoded);
    assert_eq!(again.water.salmon_size,2);
}

#[test]
fn initial_metadata_does_not_hide_nondefault_water_variants() {
    for (kind,key,value) in [(Kind::Axolotl,18,4),(Kind::Frog,18,2),(Kind::GlowSquid,18,75),(Kind::Turtle,18,1)] {
        let mut mob=Mob::new(1,kind,(0.0,64.0,0.0));
        mob.water.variant=if kind==Kind::Frog {1} else {4};
        mob.water.dark_ticks=75; mob.water.has_egg=true;
        let props=mobs::water_test_spawn_properties(&mob);
        assert!(props.iter().any(|(k,_,v)|*k==key && *v==encode_varint(value)),"{kind:?}");
    }
}

#[test]
fn named_bucket_components_roundtrip_to_tropical_fish() {
    let (_,data)=crate::stackdata::parse_item_spec("tropical_fish_bucket[tropical_fish/pattern=clayfish,tropical_fish/base_color=red,tropical_fish/pattern_color=light_blue]").unwrap();
    let mut mob=Mob::new(1,Kind::TropicalFish,(0.0,64.0,0.0));
    crate::mob::water::bucket::release(&mut mob,&data);
    assert_eq!(crate::mob::water::unpack_variant(mob.water.variant),Some((1,5,14,3)));
    assert!(crate::stackdata::parse_item_spec("salmon_bucket[salmon/size=huge]").is_err());
    assert!(crate::stackdata::parse_item_spec("axolotl_bucket[axolotl/variant=unknown]").is_err());
    let mut invalid=crate::stackdata::Data::default();
    crate::stackdata::read_component(88,&encode_varint(255),&mut invalid);
    assert!(invalid.mob_variants.is_empty());
}

#[test]
fn fish_bucket_waterlogs_slab_and_releases_fish() {
    let shared=shared(); let mut p=player();
    let item=blocks::item_named("cod_bucket").unwrap();
    p.inventory.set(inventory::FIRST_HOTBAR,Some(Stack::new(item,1)));
    let slab=blocks::state_from_text("oak_slab[type=bottom,waterlogged=false]").unwrap();
    shared.world_in(p.dimension).lock().unwrap().set_block_silently(3,63,3,slab);
    use_bucket(item,(3,63,3),1,1,&mut p,&shared).unwrap();
    let state=shared.world_in(p.dimension).lock().unwrap().get_block(3,63,3);
    assert_eq!(blocks::block_at_state(state),Some("oak_slab"));
    assert_eq!(blocks::orientation_of(state).unwrap().value(state,"waterlogged"),Some("true"));
    assert_eq!(shared.items_in(p.dimension).lock().unwrap().mobs().len(),1);
}
