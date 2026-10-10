// Вики, Ominous Item Spawner: висит над целью, затем выпускает предмет.
// [проверить] задержки, веса снарядов и выбор цели. [Bedrock: потом] показ.
use crate::{blocks,dimension::Dimension,inventory::Stack,mob::Rng,shared::Shared,world::World};
#[derive(Clone,Debug)]
pub struct Spawner {pub id:i32,pub dimension:Dimension,pub at:(f64,f64,f64),pub stack:Stack,pub due:u64}
pub const ENTITY_TYPE:i32=92; // minecraft-data 26.1 entities.json.
pub fn item(rng:&mut Rng)->Stack {
    let (item,potion)=match rng.below(12) {
        0=>("arrow",None),1=>("tipped_arrow",Some("poison")),2=>("tipped_arrow",Some("strong_slowness")),
        3=>("fire_charge",None),4=>("wind_charge",None),5=>("lingering_potion",Some("wind_charged")),
        6=>("lingering_potion",Some("weaving")),7=>("lingering_potion",Some("oozing")),8=>("lingering_potion",Some("infested")),
        9=>("lingering_potion",Some("strength")),10=>("lingering_potion",Some("swiftness")),_=>("lingering_potion",Some("slow_falling")),
    };
    Stack::new(blocks::item_named(item).unwrap(),1).with_extras(crate::stackdata::Data {potion:potion.and_then(crate::survival::potion_number),..Default::default()})
}
pub fn step(shared:&Shared,world:&mut World,dimension:Dimension,now:u64,rng:&mut Rng) {
    let mut ready=Vec::new();
    {
        let mut thrown=shared.thrown.lock().expect("снаряды");
        for (stack,at) in std::mem::take(&mut world.ominous_births) {
            thrown.ominous.push(Spawner {id:shared.next_entity_id(),dimension,at,stack,due:now+60+rng.below(61) as u64});
            world.note_sound(at.0 as i32,at.1 as i32,at.2 as i32,"minecraft:block.trial_spawner.spawn_item_begin",1.0,0.8+0.4*rng.unit() as f32);
        }
        for s in thrown.ominous.iter().filter(|s| s.dimension==dimension && now+36==s.due) {
            world.note_sound_neutral_at(s.at,"minecraft:block.trial_spawner.about_to_spawn_item",1.0,1.0);
        }
        thrown.ominous.retain(|s| {
            if s.dimension==dimension && now>=s.due {ready.push(s.clone());false} else {true}
        });
    }
    for s in ready {
        let id=shared.next_entity_id();
        let velocity=(0.0,-0.1,0.0);
        // Чёрный ящик, ominous-spawner: при выпуске — событие 3021, не звук; за 36 тактов — звук «about_to_spawn_item».
        world.note_level_event(s.at.0.floor() as i32,s.at.1.floor() as i32,s.at.2.floor() as i32,3021,1);
        match blocks::item_name(s.stack.item) {
            Some("arrow"|"tipped_arrow")=>{
                let mut arrow=crate::arrow::Arrow::from_mob(id,dimension,s.at,velocity,0,"entity.minecraft.ominous_item_spawner");
                arrow.potion=s.stack.extras().potion;arrow.effects=crate::arrow::tipped_effects(arrow.potion);
                shared.arrows.lock().expect("стрелы").spawn(arrow);
            }
            name=>{
                let flying=match name {
                    Some("fire_charge")=>crate::thrown::Flying::mob_fireball(id,dimension,crate::thrown::Kind::SmallFireball,s.at,(0.0,-1.0,0.0),0,1),
                    Some("wind_charge")=>crate::thrown::Flying::wind_charge(id,dimension,s.at,velocity,0,false),
                    _=>crate::thrown::Flying::potion(id,dimension,crate::thrown::Kind::Lingering,s.stack,s.at,velocity,0),
                };
                shared.thrown.lock().expect("снаряды").flying.push(flying);
            }
        }
    }
}
