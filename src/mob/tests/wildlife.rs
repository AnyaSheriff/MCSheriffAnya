use super::*;
use crate::mob::{self, ai::{Ctx, Mate}};
use std::collections::HashMap;

struct Garden { blocks: HashMap<Cell,i32>, griefing: bool, rain: bool }
impl Garden {
    fn new() -> Self { Self { blocks:HashMap::new(),griefing:true,rain:false } }
    fn put(&mut self,at: Cell,state: &str) { self.blocks.insert(at,blocks::state_from_text(state).unwrap()); }
}
impl View for Garden {
    fn block(&self,x:i32,y:i32,z:i32)->i32 { self.blocks.get(&(x,y,z)).copied().unwrap_or_else(||if y<0 {blocks::state_by_name("dirt").unwrap()}else{0}) }
    fn loaded(&self,_x:i32,_z:i32)->bool {true}
    fn open_sky(&self,x:i32,y:i32,z:i32)->bool { !(y+1..y+8).any(|above|mob::solid(self.block(x,above,z))) }
    fn griefing(&self)->bool {self.griefing}
    fn rain(&self,_x:i32,_y:i32,_z:i32)->bool {self.rain}
}
fn context()->Ctx<'static> {Ctx{players:&[],mates:&[],difficulty:2,sky:15,sun_up:true}}
fn animal(kind:Kind)->Mob {let mut m=Mob::new(42,kind,(0.5,0.0,0.5));m.on_ground=true;m}

#[test]
fn ids_foods_and_leads() {
    for (name,kind,id) in [("rabbit",Kind::Rabbit,108),("fox",Kind::Fox,54),("bee",Kind::Bee,11)] {
        assert_eq!(Kind::from_name(name),Some(kind)); assert_eq!(kind.java_type(),id);
        assert!(blocks::item_named(&format!("{name}_spawn_egg")).is_some());
        assert!(mob::leash::leashable(kind));
    }
    for food in ["carrot","golden_carrot","dandelion"] {assert!(Kind::Rabbit.likes(food));}
    for food in ["sweet_berries","glow_berries"] {assert!(Kind::Fox.likes(food));}
    assert!(Kind::Bee.likes("poppy")); assert!(!Kind::Bee.likes("wither_rose")); assert!(!Kind::Bee.likes("closed_eyeblossom"));
    assert!(!Kind::Rabbit.likes("wheat")); assert!(!Kind::Fox.likes("carrot"));
}
#[test]
fn variants_are_biome_specific_and_never_generate_killer() {
    let mut rng=Rng::new(71);
    for _ in 0..1000 {
        assert_eq!(rabbit_variant("desert",&mut rng),4);
        assert!(matches!(rabbit_variant("snowy_taiga",&mut rng),1|3));
        assert!(matches!(rabbit_variant("flower_forest",&mut rng),0|2|5));
    }
    assert_eq!(fox_variant("grove"),1);assert_eq!(fox_variant("taiga"),0);
}
#[test]
fn command_variants_and_toast_survive_save() {
    let mut rabbit=animal(Kind::Rabbit);
    apply_data(&mut rabbit,"{RabbitType:99}");
    assert_eq!(rabbit.wild.variant,99);assert_eq!(rabbit.armor(),8.0);
    assert_eq!(rabbit.name.as_deref(),Some("The Killer Bunny"));
    rabbit.name=Some("Toast".into());
    let mut bytes=Vec::new();rabbit.write(&mut bytes);
    let read=Mob::read(&mut bytes.as_slice()).unwrap();
    assert_eq!(read.name.as_deref(),Some("Toast"));assert_eq!(read.wild.variant,99);
}
#[test]
fn fox_trust_and_mouth_components_survive_save() {
    let mut fox=animal(Kind::Fox);
    apply_data(&mut fox,"{Type:snow,Sleeping:1b,Trusted:[[I;1,2,3,4],[I;5,6,7,8]]}");
    fox.wild.mouth=Some(Stack::new(blocks::item_named("diamond_sword").unwrap(),1).with_extras(crate::stackdata::Data { damage:37,name:Some("Blade".into()),..Default::default() }));
    let mut bytes=Vec::new();fox.write(&mut bytes);
    let read=Mob::read(&mut bytes.as_slice()).unwrap();
    assert_eq!(read.wild,fox.wild);
    assert!(!read.tame.tamed());
}
#[test]
fn breeding_fox_trusts_both_feeders_but_not_tames() {
    let mut a=animal(Kind::Fox);let mut b=animal(Kind::Fox);let mut baby=animal(Kind::Fox);
    a.wild.breeder=Some([1;16]);b.wild.breeder=Some([2;16]);b.wild.variant=1;
    inherit(&mut baby,&a,&b,&Garden::new(),&mut Rng::new(1));
    assert_eq!(baby.wild.trusted,[Some([1;16]),Some([2;16])]); assert!(!baby.tame.tamed());
    let mut mobs=vec![baby]; bind_trust(&mut mobs,&[([1;16],7),([2;16],8)]);
    assert_eq!(mobs[0].wild.trusted_ids,[7,8]);
}
#[test]
fn rabbits_eat_only_mature_carrots_and_obey_griefing() {
    let mut garden=Garden::new();garden.put((0,0,0),"carrots[age=7]");
    let mut rabbit=animal(Kind::Rabbit);rabbit.ticks=20;
    let plan=think(&mut rabbit,&garden,&context(),&mut Rng::new(1));
    assert_eq!(plan.put.map(|(_,state)|number(state,"age")),Some(6));
    assert_eq!(rabbit.wild.cooldown,40);
    garden.griefing=false;
    rabbit.wild.cooldown=0;rabbit.ticks=40;
    assert!(think(&mut rabbit,&garden,&context(),&mut Rng::new(1)).put.is_none());
    garden.griefing=true;garden.put((0,0,0),"carrots[age=6]");rabbit.ticks=60;
    assert!(think(&mut rabbit,&garden,&context(),&mut Rng::new(1)).put.is_none());
}
#[test]
fn sleeping_fox_wakes_at_night_or_without_shade() {
    let mut garden=Garden::new();garden.put((0,2,0),"stone");
    let mut fox=animal(Kind::Fox);fox.wild.sleeping=true;
    let mut ctx=context();
    assert_eq!(think(&mut fox,&garden,&ctx,&mut Rng::new(4)).speed,0.0);assert!(fox.wild.sleeping);
    ctx.sun_up=false;think(&mut fox,&garden,&ctx,&mut Rng::new(4));assert!(!fox.wild.sleeping);
    fox.wild.sleeping=true;ctx.sun_up=true;garden.blocks.clear();think(&mut fox,&garden,&ctx,&mut Rng::new(4));assert!(!fox.wild.sleeping);
}
#[test]
fn fox_harvests_berries_after_wait_and_holds_one() {
    let mut garden=Garden::new();garden.put((0,0,0),"sweet_berry_bush[age=3]");
    let mut fox=animal(Kind::Fox);fox.wild.target=Some((0,0,0));
    let mut ctx=context();ctx.sun_up=false;
    for _ in 0..41 {assert!(think(&mut fox,&garden,&ctx,&mut Rng::new(5)).put.is_none());}
    let plan=think(&mut fox,&garden,&ctx,&mut Rng::new(5));
    assert_eq!(plan.put.map(|(_,s)|number(s,"age")),Some(1));
    assert_eq!(blocks::item_name(fox.wild.mouth.unwrap().item),Some("sweet_berries"));
}
#[test]
fn bee_stings_only_once_then_dies() {
    let garden=Garden::new();let mut bee=animal(Kind::Bee);
    sting(&mut bee);assert!(bee.wild.stung);assert_eq!(bee.wild.anger,0);
    bee.wild.death_in=1;let mut rng=Rng::new(1);
    bee.tick(&garden,&context(),&mut rng);
    assert!(bee.is_dead());
    assert!(poison(1).is_none());assert_eq!(poison(2).unwrap().ticks_left,200);assert_eq!(poison(3).unwrap().ticks_left,360);
}
#[test]
fn bee_pollination_and_crop_limits() {
    let mut garden=Garden::new();garden.put((0,0,0),"poppy");
    let mut bee=animal(Kind::Bee);bee.y=0.8;bee.wild.flower=Some((0,0,0));
    for _ in 0..400 {think(&mut bee,&garden,&context(),&mut Rng::new(7));}
    assert!(bee.wild.nectar);
    assert!(crop_growth(blocks::state_from_text("wheat[age=7]").unwrap()).is_none());
    assert_eq!(number(crop_growth(blocks::state_from_text("wheat[age=2]").unwrap()).unwrap(),"age"),3);
}
#[test]
fn bee_breeding_works_in_midair() {
    let garden=Garden::new();let mut bee=animal(Kind::Bee);bee.on_ground=false;bee.y=5.0;bee.love=600;
    let mate=Mate{id:43,kind:Kind::Bee,x:1.0,y:5.0,z:0.5,love:600,baby:false,ready:true,tamed:false};
    let mates=[mate];let mut ctx=context();ctx.mates=&mates;
    let mut result=None;
    for _ in 0..60 {result=think(&mut bee,&garden,&ctx,&mut Rng::new(7)).breed.or(result);}
    assert_eq!(result,Some(43));
}
#[test]
fn bees_share_anger_but_spent_bees_do_not_attack() {
    let mut bees=vec![animal(Kind::Bee),animal(Kind::Bee),animal(Kind::Bee)];
    bees[1].x=10.0;bees[2].wild.stung=true;
    let garden=Garden::new();
    on_hurt(&mut bees[0],91);
    // Гнев приходит через 2 такта после удара (чёрный ящик, bee-sting).
    base_tick(&mut bees[0],&garden);assert_eq!(bees[0].wild.anger,0);
    base_tick(&mut bees[0],&garden);assert!((400..=780).contains(&bees[0].wild.anger));
    pack_anger(&mut bees);
    assert_eq!(bees[1].wild.attacker,Some(91));assert!(bees[1].wild.anger>0);assert_eq!(bees[2].wild.anger,0);
}
#[test]
fn bee_without_ai_is_never_angered() {
    let garden=Garden::new();let mut bee=animal(Kind::Bee);bee.no_ai=true;
    on_hurt(&mut bee,91);
    for _ in 0..5 {base_tick(&mut bee,&garden);}
    assert_eq!(bee.wild.anger,0);assert_eq!(bee.wild.attacker,None);
}
#[test]
fn angry_bee_starts_chase_on_twentieth_tick_check_and_calms_after_sting() {
    let garden=Garden::new();let mut bee=animal(Kind::Bee);bee.wild.anger=500;bee.wild.attacker=Some(91);
    let players=[mob::ai::Watcher{id:91,x:3.0,y:0.0,z:0.5,eye:1.62,holding:[None,None],spectator:false,targetable:true,look:(0.0,0.0),pumpkin: false, sprinting: false, riding: false, gold_armor: false}];
    let mut ctx=context();ctx.players=&players;
    bee.ticks=1;think(&mut bee,&garden,&ctx,&mut Rng::new(1));
    assert!(bee.wild.chase&&bee.wild.flying);
    sting(&mut bee);assert!(!bee.wild.roll&&bee.wild.calm>0);
    for _ in 0..3 {base_tick(&mut bee,&garden);}
    assert!(!bee.wild.chase);
}
#[test]
fn mouth_pickup_takes_one_and_drops_old_item() {
    let mut items=crate::items::Items::new();let mut fox=animal(Kind::Fox);
    fox.wild.mouth=Some(Stack::new(blocks::item_named("diamond").unwrap(),1));items.add_mob(fox);
    items.drop_item(80,Stack::new(blocks::item_named("sweet_berries").unwrap(),4),(0.5,0.0,0.5),(0.0,0.0,0.0),0);
    items.note_tick_start();
    pickup(&mut items,true);
    assert_eq!(items.mobs()[0].wild.mouth.unwrap().count,1);
    assert_eq!(items.items()[0].stack.count,3);
    assert!(items.mobs()[0].events.iter().any(|e|matches!(e,Event::Loot(_))));
}
#[test]
fn rabbit_drops_and_baby_fox_equipment() {
    let mut rabbit=animal(Kind::Rabbit);rabbit.by_player=true;rabbit.looting=3;
    let mut foot=false;let mut rng=Rng::new(72);
    for _ in 0..1000 {
        for stack in mob::loot(&rabbit,&mut rng) {
            assert!(stack.count<=4);
            if blocks::item_name(stack.item)==Some("rabbit_foot") {foot=true;assert_eq!(stack.count,1);}
        }
    }
    assert!(foot);rabbit.age= -100;assert!(mob::loot(&rabbit,&mut rng).is_empty());
    let mut fox=animal(Kind::Fox);fox.age= -100;fox.wild.mouth=Some(Stack::new(blocks::item_named("emerald").unwrap(),1));
    assert_eq!(mob::loot(&fox,&mut rng),vec![fox.wild.mouth.unwrap()]);
}
#[test]
fn spawn_tables_have_wildlife_only_in_their_biomes() {
    use crate::mob::spawn::passive_weights;
    assert_eq!(passive_weights("desert"),vec![(Kind::Rabbit,4)]);
    assert!(passive_weights("taiga").contains(&(Kind::Fox,8)));
    assert!(passive_weights("flower_forest").contains(&(Kind::Rabbit,4)));
    assert!(!passive_weights("plains").iter().any(|(k,_)|matches!(k,Kind::Fox|Kind::Rabbit|Kind::Bee)));
}

#[test]
fn fox_defends_only_trusted_players_and_allowed_attackers() {
    let mut fox=animal(Kind::Fox);fox.wild.trusted_ids=[7,0];fox.wild.sleeping=true;
    let mut mobs=vec![fox];let target=mob::tame::Target::Player(8);
    defend(&mut mobs,6,target,false);assert!(mobs[0].tame.target.is_none());
    defend(&mut mobs,7,target,true);assert!(mobs[0].tame.target.is_none());
    defend(&mut mobs,7,target,false);assert_eq!(mobs[0].tame.target,Some(target));assert!(!mobs[0].wild.sleeping);
}
#[test]
fn nectar_grows_at_most_ten_crops_and_obeys_griefing() {
    let mut garden=Garden::new();garden.put((0,0,0),"wheat[age=0]");garden.put((4,2,0),"beehive");
    let mut bee=animal(Kind::Bee);bee.y=2.0;bee.wild.nectar=true;bee.wild.hive=Some((4,2,0));
    let mut rng=Rng::new(501);let mut grew=0;
    for _ in 0..1000 {grew+=usize::from(think(&mut bee,&garden,&context(),&mut rng).put.is_some());}
    assert_eq!(grew,10);assert_eq!(bee.wild.crops,10);
    bee.wild.crops=0;garden.griefing=false;
    for _ in 0..100 {assert!(think(&mut bee,&garden,&context(),&mut rng).put.is_none());}
}

#[test]
fn fox_uses_weapon_attributes_and_totem() {
    let mut fox=animal(Kind::Fox);
    fox.wild.mouth=Some(Stack::new(blocks::item_named("diamond_sword").unwrap(),1));
    assert_eq!(fox_damage(&fox,Some(Kind::Rabbit)),8.0);
    fox.wild.mouth=Some(Stack::new(blocks::item_named("totem_of_undying").unwrap(),1));
    let hit=fox.hurt(100.0,crate::survival::Source::Generic,None).unwrap();
    assert!(!hit.died);assert!(!fox.is_dead());assert_eq!(fox.vitals.health,1.0);
    assert!(fox.wild.mouth.is_none());assert_eq!(fox.vitals.effects.len(),3);
    assert!(fox.events.iter().any(|e|matches!(e,Event::Status(35))));
}
#[test]
fn harmful_flowers_do_not_breed_bees() {
    assert_eq!(harmful_flower("wither_rose").unwrap().ticks_left,40);
    assert_eq!(harmful_flower("open_eyeblossom").unwrap().ticks_left,25);
    assert!(harmful_flower("closed_eyeblossom").is_none());
    assert!(!flower("wither_rose"));assert!(!flower("open_eyeblossom"));
}

#[test]
fn pollen_grows_vines_and_both_pitcher_halves() {
    let vine=blocks::state_from_text("cave_vines[berries=false]").unwrap();
    let grown=crop_growth(vine).unwrap();
    assert_eq!(blocks::orientation_of(grown).unwrap().value(grown,"berries"),Some("true"));
    assert!(crop_growth(grown).is_none());
    let pitcher=blocks::state_from_text("pitcher_crop[age=2,half=lower]").unwrap();
    let grown=crop_growth(pitcher).unwrap();
    let (at,upper)=crop_upper((0,0,0),grown).unwrap();
    assert_eq!(at,(0,1,0));assert_eq!(number(upper,"age"),3);assert!(crop_growth(upper).is_none());
}

#[test]
fn berry_harvest_keeps_existing_mouth_item_and_respects_griefing() {
    let mut garden=Garden::new();garden.put((0,0,0),"sweet_berry_bush[age=3]");
    let held=Stack::new(blocks::item_named("emerald").unwrap(),1);
    let mut fox=animal(Kind::Fox);fox.wild.target=Some((0,0,0));fox.wild.mouth=Some(held);
    let mut ctx=context();ctx.sun_up=false;
    garden.griefing=false;
    for _ in 0..50 {assert!(think(&mut fox,&garden,&ctx,&mut Rng::new(5)).put.is_none());}
    garden.griefing=true;fox.wild.target=Some((0,0,0));fox.wild.hop=41;fox.wild.cooldown=0;
    assert!(think(&mut fox,&garden,&ctx,&mut Rng::new(5)).put.is_some());
    assert_eq!(fox.wild.mouth,Some(held));
}
