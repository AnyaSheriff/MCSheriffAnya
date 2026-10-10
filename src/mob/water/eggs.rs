// Вики, Turtle Egg: «Between 21062 and 21904 ticks» каждый случайный такт трескает яйцо
// (чёрный ящик turtle-egg-hatch: трещина через 10 тактов после time set 21050); вне окна шанс 1/500.
// Frogspawn: «3–10 minutes», 2–5 головастиков. Луна не меняет шанс яйца.
use super::*;
use crate::world::{World,AIR};

fn number(state:i32,key:&str)->i32 {
    blocks::orientation_of(state).and_then(|o|o.value(state,key)).and_then(|s|s.parse().ok()).unwrap_or(0)
}
// Вики, Frogspawn: «water source block» либо «waterlogged block».
pub fn supports_frogspawn(state:i32)->bool {
    matches!(crate::fluids::fluid_at(state),Some((crate::fluids::Kind::Water,crate::fluids::SOURCE)))
        || (super::super::is_water(state) && blocks::block_at_state(state)!=Some("water"))
        || blocks::orientation_of(state).and_then(|o|o.value(state,"waterlogged"))==Some("true")
}
pub fn hatch_window(time:i64)->bool { (21062..=21904).contains(&time.rem_euclid(24000)) }
pub fn turtle_tick(world:&mut World,(x,y,z):(i32,i32,i32),state:i32) {
    if !matches!(blocks::block_at_state(world.get_block(x,y-1,z)),Some("sand" | "red_sand")) {return;}
    if !hatch_window(world.time_of_day()) && world.random_unit()>=1.0/500.0 {return;}
    let hatch=number(state,"hatch");
    if hatch<2 {
        if let Some(next)=blocks::orientation_of(state).and_then(|o|o.with(state,"hatch",&(hatch+1).to_string())) {
            // Звук, событие 2012/15, блок — в этом порядке (чёрный ящик turtle-egg-hatch);
            // высота звука 1,0…1,1 [проверить: замерены 1,05 и 1,02].
            let pitch=1.0+world.random_unit()*0.1;
            world.note_sound(x,y,z,"minecraft:entity.turtle.egg_crack",0.7,pitch);
            world.note_level_event(x,y,z,2012,15);
            world.set_block(x,y,z,next);
        }
    } else {
        // Звук 0,9…1,1 [проверить: замерена одна: 0,90]; на каждого малыша — событие 2001 с
        // состоянием яйца, потом сам малыш; блок убирается последним (чёрный ящик turtle-egg-hatch).
        let pitch=0.9+world.random_unit()*0.2;
        world.note_sound(x,y,z,"minecraft:entity.turtle.egg_hatch",0.7,pitch);
        world.set_block(x,y,z,AIR);
        for i in 0..number(state,"eggs").clamp(1,4) {
            world.note_level_event(x,y,z,2001,state);
            let mut baby=Mob::new(0,Kind::Turtle,(f64::from(x)+0.3+f64::from(i)*0.2,f64::from(y),f64::from(z)+0.3));
            baby.age=super::super::BABY_AGE;
            baby.water.home=(x,y,z);
            world.queue_water_hatch(baby);
        }
    }
}
pub fn frog_tick(world:&mut World,(x,y,z):(i32,i32,i32)) {
    if blocks::block_at_state(world.get_block(x,y,z))!=Some("frogspawn") {return;}
    world.set_block(x,y,z,AIR);
    if !supports_frogspawn(world.get_block(x,y-1,z)) {return;}
    world.note_sound(x,y,z,"minecraft:block.frogspawn.hatch",1.0,1.0);
    let count=2+(world.random_unit()*4.0) as i32;
    for _ in 0..count {
        let at=(f64::from(x)+0.2+f64::from(world.random_unit())*0.6,f64::from(y)-0.5,f64::from(z)+0.2+f64::from(world.random_unit())*0.6);
        let mut tadpole=Mob::new(0,Kind::Tadpole,at);
        tadpole.persistent_tag=true;
        world.queue_water_hatch(tadpole);
    }
}
pub fn break_one(world:&mut World,(x,y,z):(i32,i32,i32))->bool {
    let state=world.get_block(x,y,z);
    if blocks::block_at_state(state)!=Some("turtle_egg") {return false;}
    let eggs=number(state,"eggs");
    let next=if eggs>1 {blocks::orientation_of(state).and_then(|o|o.with(state,"eggs",&(eggs-1).to_string())).unwrap_or(AIR)} else {AIR};
    world.set_block(x,y,z,next);
    world.note_sound(x,y,z,"minecraft:entity.turtle.egg_break",0.7,1.0);
    true
}
pub fn trample(world:&mut World,pos:(i32,i32,i32),falling:bool)->bool {
    let odds=if falling {3.0} else {100.0};
    blocks::block_at_state(world.get_block(pos.0,pos.1,pos.2))==Some("turtle_egg") && world.random_unit()<1.0/odds && break_one(world,pos)
}

/// Зомби ищут яйца с двумя блоками воздуха сверху; черепахи яиц не топчут.
/// [проверить] Дальность поиска и пауза разрушения требуют чёрного ящика.
pub fn mob_steps(world:&mut World,mobs:&mut [Mob],griefing:bool) {
    if !griefing {return;}
    for mob in mobs.iter_mut().filter(|m|!m.is_dead() && !m.no_ai && m.kind!=Kind::Turtle) {
        let (x,y,z)=mob.cell();
        if mob.on_ground {trample(world,(x,y,z),false);}
        if matches!(mob.kind,Kind::Zombie | Kind::Husk | Kind::Drowned | Kind::ZombieVillager | Kind::ZombifiedPiglin) && mob.h.target.is_none() {
            if mob.ticks.is_multiple_of(20) {
                let mut nearest=None;
                for dx in -12..=12 {for dz in -12..=12 {for dy in -3..=3 {
                    let at=(x+dx,y+dy,z+dz);
                    if blocks::block_at_state(world.get_block(at.0,at.1,at.2))==Some("turtle_egg")
                        && world.get_block(at.0,at.1+1,at.2)==AIR && world.get_block(at.0,at.1+2,at.2)==AIR
                        && nearest.is_none_or(|(_,dist)| dx*dx+dy*dy+dz*dz<dist) { nearest=Some((at,dx*dx+dy*dy+dz*dz)); }
                }}}
                mob.water.egg_target=nearest.map(|(at,_)|at);
            }
            if let Some(at)=mob.water.egg_target {
                if blocks::block_at_state(world.get_block(at.0,at.1,at.2))!=Some("turtle_egg") {mob.water.egg_target=None;continue;}
                if (mob.x-f64::from(at.0)-0.5).abs()<1.0 && (mob.z-f64::from(at.2)-0.5).abs()<1.0 && (mob.y-f64::from(at.1)).abs()<1.5 {
                    mob.water.egg_ticks+=1;
                    if mob.water.egg_ticks>=60 {break_one(world,at);mob.water.egg_ticks=0;}
                } else {mob.water.egg_ticks=0;}
            }
        }
    }
}
