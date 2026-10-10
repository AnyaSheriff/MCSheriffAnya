// Таблицы биомов (вики, Ocean/River/Lush Caves/Swamp/Beach, «Spawning»).
// [проверить] Планировщик попыток и масштабирование лимитов по чанкам.
use super::*;
use crate::world::World;

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub struct Entry { pub kind:Kind,pub weight:u32,pub min:u32,pub max:u32 }
fn entry(kind:Kind,weight:u32,min:u32,max:u32)->Entry {Entry{kind,weight,min,max}}
pub fn table(biome:&str)->Vec<Entry> {
    use Kind::*;
    match biome {
        "river"=>vec![entry(Squid,2,1,4),entry(Salmon,5,1,5)],
        "frozen_river"=>vec![entry(Salmon,5,1,5)],
        "warm_ocean"=>vec![entry(TropicalFish,25,8,8),entry(Pufferfish,15,1,3),entry(Dolphin,2,1,2)],
        "lukewarm_ocean" | "deep_lukewarm_ocean"=>vec![entry(Squid,2,1,4),entry(Cod,15,3,6),entry(TropicalFish,25,8,8),entry(Pufferfish,5,1,3),entry(Dolphin,2,1,2)],
        "ocean" | "deep_ocean"=>vec![entry(Squid,1,1,4),entry(Cod,15,3,6),entry(Dolphin,1,1,2)],
        "cold_ocean" | "deep_cold_ocean"=>vec![entry(Squid,3,1,4),entry(Cod,15,3,6),entry(Salmon,15,1,5),entry(Dolphin,1,1,2)],
        "frozen_ocean" | "deep_frozen_ocean"=>vec![entry(Squid,1,1,4),entry(Salmon,15,1,5)],
        "lush_caves"=>vec![entry(Axolotl,10,4,6),entry(TropicalFish,25,8,8)],
        "beach"=>vec![entry(Turtle,5,2,5)],
        "swamp"=>vec![entry(Frog,10,2,5)],
        "mangrove_swamp"=>vec![entry(Frog,10,2,5),entry(TropicalFish,25,8,8)],
        _=>Vec::new()
    }
}
pub fn category(kind:Kind)->u8 {
    match kind {Kind::Squid|Kind::Dolphin=>1,Kind::GlowSquid=>2,Kind::Axolotl=>3,Kind::Turtle|Kind::Frog=>4,_=>0}
}
fn clay_below(view:&dyn View,(x,y,z):(i32,i32,i32))->bool {
    (1..=5).any(|dy| blocks::block_at_state(view.block(x,y-dy,z))==Some("clay"))
}
pub fn allowed(view:&dyn View,kind:Kind,at:(i32,i32,i32),biome:&str,light:(u8,u8))->bool {
    let (x,y,z)=at;
    if !view.loaded(x.div_euclid(16),z.div_euclid(16)) {return false;}
    if kind!=Kind::GlowSquid && !table(biome).iter().any(|entry|entry.kind==kind) {return false;}
    if kind==Kind::Turtle {
        return biome=="beach" && y>62 && matches!(blocks::block_at_state(view.block(x,y-1,z)),Some("sand"))
            && view.block(x,y,z)==crate::world::AIR && light.0.max(light.1)>8;
    }
    if kind==Kind::Frog {
        return matches!(biome,"swamp"|"mangrove_swamp") && matches!(blocks::block_at_state(view.block(x,y-1,z)),Some("grass_block"|"mud"|"mangrove_roots"|"muddy_mangrove_roots"))
            && view.block(x,y,z)==crate::world::AIR && light.0.max(light.1)>8;
    }
    // Вики, Glow Squid: Java «light level of 0», «below Y=30», кроме Deep Dark;
    // камень и второй слой воды не требуются (это не правило Bedrock).
    if kind==Kind::GlowSquid {
        return biome!="deep_dark" && y<30 && light==(0,0) && super::super::is_water(view.block(x,y,z));
    }
    if !super::super::is_water(view.block(x,y,z)) || !super::super::is_water(view.block(x,y-1,z)) {return false;}
    match kind {
        Kind::Axolotl=>biome=="lush_caves" && clay_below(view,at),
        Kind::TropicalFish if biome=="lush_caves"=>true,
        Kind::Squid=>(46..=62).contains(&y),
        _=>(50..=63).contains(&y),
    }
}
pub fn attempt(world:&mut World,players:&[ai::Watcher],mobs:&[Mob],rng:&mut Rng,next_id:&mut dyn FnMut()->i32)->Vec<Mob> {
    let mut born=Vec::new();
    if world.is_nether() || world.is_end() {return born;}
    let mut counts=[0usize;5];
    for mob in mobs.iter().filter(|m|creature(m.kind) && !m.is_dead() && !m.persistent_tag) {counts[category(mob.kind) as usize]+=1;}
    let caps=[20,5,5,5,10];
    for player in players.iter().filter(|p|!p.spectator) {
        for _ in 0..8 {
            let x=player.x.floor() as i32+rng.below(129) as i32-64;
            let z=player.z.floor() as i32+rng.below(129) as i32-64;
            if !world.has_chunk(x.div_euclid(16),z.div_euclid(16)) {continue;}
            let mut y=if rng.below(3)==0 { (player.y.floor() as i32+rng.below(65) as i32-32).clamp(-63,62) } else {50+rng.below(14) as i32};
            if matches!(world.surface_biome(x,z),"beach" | "swamp" | "mangrove_swamp")
                && let Some(ground)=(50..100).rev().find(|y| blocks::solid_top(world.get_block(x,*y,z))) { y=ground+1; }
            let biome=world.mob_biome(x,y,z);
            let mut choices=table(biome);
            if y<30 {choices.push(entry(Kind::GlowSquid,10,2,4));}
            choices.retain(|e|counts[category(e.kind) as usize]<caps[category(e.kind) as usize]);
            let total:u32=choices.iter().map(|e|e.weight).sum();
            if total==0 {continue;}
            let mut roll=rng.below(total);
            let Some(chosen)=choices.iter().find(|e| {if roll<e.weight {true} else {roll-=e.weight;false}}) else {continue;};
            let n=chosen.min+rng.below(chosen.max-chosen.min+1);
            let mut variant=None;
            for _ in 0..n {
                let (gx,gz)=(x+rng.below(5) as i32-2,z+rng.below(5) as i32-2);
                let at=(f64::from(gx)+0.5,f64::from(y),f64::from(gz)+0.5);
                let dist=players.iter().map(|p|(p.x-at.0).powi(2)+(p.y-at.1).powi(2)+(p.z-at.2).powi(2)).fold(f64::INFINITY,f64::min);
                if !(24.0*24.0..=128.0*128.0).contains(&dist) {continue;}
                let light=world.light_full(gx,y,gz);
                let biome=world.mob_biome(gx,y,gz);
                if chosen.kind!=Kind::GlowSquid && !table(biome).iter().any(|entry|entry.kind==chosen.kind) {continue;}
                if !allowed(&crate::tick::MobView(world),chosen.kind,(gx,y,gz),biome,light) {continue;}
                let mut mob=Mob::new(next_id(),chosen.kind,at);
                initialize(&mut mob,biome,rng);
                if mob.kind==Kind::TropicalFish {mob.water.variant = *variant.get_or_insert(mob.water.variant);}
                let cat=category(mob.kind) as usize;
                if counts[cat]>=caps[cat] {break;}
                counts[cat]+=1;
                born.push(mob);
            }
        }
    }
    born
}
