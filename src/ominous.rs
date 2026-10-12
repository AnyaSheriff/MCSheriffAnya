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
pub fn step(shared:&Shared,world:&mut World,items:&mut crate::items::Items,dimension:Dimension,now:u64,rng:&mut Rng) {
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
        // Чёрный ящик, gaps2-ominous-drops (две записи): скорость выпуска вниз —
        // стрела 1,08, заряд ветра 0,98, огненный шар 1,0 (дальше разгон), зелье
        // 1,42, пузырёк опыта 1,36; по каждой оси добавляется шум порядка 0,045.
        // [проверить] яйцо, снежок, жемчуг — взято 1,1 по аналогии.
        let name=blocks::item_name(s.stack.item);
        let speed=match name {
            Some("arrow"|"tipped_arrow"|"spectral_arrow")=>1.08,
            Some("wind_charge")=>0.98,
            Some("fire_charge")=>1.0,
            Some("lingering_potion"|"splash_potion")=>1.42,
            Some("experience_bottle")=>1.36,
            _=>1.1,
        };
        let mut gauss=||(0..12).map(|_|f64::from(world.random_unit())).sum::<f64>()-6.0;
        let velocity=(gauss()*0.045,-speed+gauss()*0.045,gauss()*0.045);
        // Чёрный ящик, ominous-spawner: при выпуске — событие 3021, не звук; за 36 тактов — звук «about_to_spawn_item».
        world.note_level_event(s.at.0.floor() as i32,s.at.1.floor() as i32,s.at.2.floor() as i32,3021,1);
        match blocks::item_name(s.stack.item) {
            Some("arrow"|"tipped_arrow")=>{
                let mut arrow=crate::arrow::Arrow::from_mob(id,dimension,s.at,velocity,0,"entity.minecraft.ominous_item_spawner");
                arrow.potion=s.stack.extras().potion;arrow.effects=crate::arrow::tipped_effects(arrow.potion);
                shared.arrows.lock().expect("стрелы").spawn(arrow);
            }
            name if name.is_some_and(projectile_item)=>{
                let flying=match name {
                    Some("fire_charge")=>{
                        let mut ball=crate::thrown::Flying::mob_fireball(id,dimension,crate::thrown::Kind::SmallFireball,s.at,(0.0,-1.0,0.0),0,1);
                        ball.vx=velocity.0;ball.vy=velocity.1;ball.vz=velocity.2;
                        ball.start=(s.at,velocity);ball.prev=(s.at,velocity,ball.rot);
                        ball
                    }
                    Some("wind_charge")=>crate::thrown::Flying::wind_charge(id,dimension,s.at,velocity,0,false),
                    _=>crate::thrown::Flying::potion(id,dimension,match name {
                        Some("splash_potion") => crate::thrown::Kind::Splash,
                        Some("experience_bottle") => crate::thrown::Kind::ExperienceBottle,
                        Some("egg") => crate::thrown::Kind::Egg,
                        Some("snowball") => crate::thrown::Kind::Snowball,
                        Some("ender_pearl") => crate::thrown::Kind::EnderPearl,
                        _ => crate::thrown::Kind::Lingering,
                    },s.stack,s.at,velocity,0),
                };
                shared.thrown.lock().expect("снаряды").flying.push(flying);
            }
            // Предмет падает как выброшенный: x, z в пределах ±0,1 и 0,2 вверх
            // (та же запись: (−0,094; +0,200; −0,010) у ominous_bottle).
            _ => {
                let toss=(f64::from(world.random_unit())*0.2-0.1,0.2,f64::from(world.random_unit())*0.2-0.1);
                items.drop_item(id,s.stack,s.at,toss,10)
            }
        }
    }
}

// Вики, Ominous Item Spawner: «Otherwise the item is dropped».
fn projectile_item(name: &str) -> bool {
    matches!(name, "fire_charge" | "wind_charge" | "lingering_potion" | "splash_potion"
        | "experience_bottle" | "snowball" | "egg" | "ender_pearl")
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bottles_are_items_unless_they_are_throwable() {
        assert!(!projectile_item("ominous_bottle"));
        assert!(!projectile_item("potion"));
        assert!(!projectile_item("diamond"));
        for name in ["lingering_potion","splash_potion","experience_bottle","wind_charge","fire_charge"] { assert!(projectile_item(name)); }
    }
}
