// Огонь: minecraft.wiki/w/Fire, «age 0–15», «30 to 39 game ticks».
// [проверить] вероятности/порядок обхода и влажность биомов: без измерений
// оригинала распределение случайного распространения ещё не сверено.
use crate::{blocks, containers::Pos, world::{World, AIR}};

/// (Поддержка огня, сгорание): таблица Burning blocks со страницы Fire.
pub fn flammability(name: &str) -> (u32, u32) {
    if name.starts_with("crimson_") || name.starts_with("warped_") { return (0, 0); }
    if name.ends_with("_leaves") || matches!(name, "azalea" | "flowering_azalea") { return (30, 60); }
    if name.ends_with("_wool") { return (30, 60); }
    if name.ends_with("_carpet") { return (60, 20); }
    if name.ends_with("_log") || name.ends_with("_wood") || matches!(name, "bamboo_block" | "stripped_bamboo_block") { return (5, 5); }
    if name.ends_with("_planks") || ((name.ends_with("_slab") || name.ends_with("_stairs") || name.ends_with("_fence") || name.ends_with("_fence_gate")) && ["oak", "spruce", "birch", "jungle", "acacia", "dark_oak", "mangrove", "cherry", "pale_oak", "bamboo"].iter().any(|w| name.starts_with(w))) { return (5, 20); }
    match name {
        "bookshelf" | "lectern" | "chiseled_bookshelf" => (30, 20),
        "tnt" => (15, 100),
        "hay_block" | "target" => (60, 20),
        "coal_block" => (5, 5),
        "vine" | "glow_lichen" => (15, 100),
        "bamboo" | "scaffolding" => (60, 60),
        "short_grass" | "tall_grass" | "fern" | "large_fern" | "dead_bush" | "sweet_berry_bush" | "dandelion" | "poppy" | "blue_orchid" | "allium" | "azure_bluet" | "oxeye_daisy" | "cornflower" | "lily_of_the_valley" | "wither_rose" | "sunflower" | "lilac" | "rose_bush" | "peony" | "pink_petals" => (60, 100),
        n if n.ends_with("_tulip") => (60, 100),
        "dried_kelp_block" => (30, 60),
        _ => (0, 0),
    }
}

fn fuel(w: &World, p: Pos) -> (u32, u32) {
    let s = w.get_block(p.0, p.1, p.2);
    if blocks::orientation_of(s).and_then(|o| o.value(s, "waterlogged")) == Some("true") { return (0, 0); }
    flammability(blocks::block_at_state(s).unwrap_or(""))
}
const SIDES: [Pos; 6] = [(1,0,0), (-1,0,0), (0,1,0), (0,-1,0), (0,0,1), (0,0,-1)];
fn add(a: Pos, b: Pos) -> Pos { (a.0+b.0, a.1+b.1, a.2+b.2) }
fn roll(w: &mut World, n: u32) -> u32 { w.random_bits() % n }
fn neighbours(w: &World, p: Pos) -> u32 { SIDES.iter().map(|&d| fuel(w, add(p,d)).0).max().unwrap_or(0) }

pub fn eternal(w: &World, below: i32) -> bool {
    matches!(blocks::block_at_state(below), Some("netherrack" | "magma_block")) || (w.is_end() && blocks::block_at_state(below) == Some("bedrock"))
}

pub fn state_at(w: &World, p: Pos, age: u32) -> i32 {
    let below = w.get_block(p.0,p.1-1,p.2);
    if matches!(blocks::block_at_state(below), Some("soul_sand" | "soul_soil")) { return blocks::state_by_name("soul_fire").unwrap(); }
    let mut s = blocks::state_by_name("fire").unwrap();
    let o = blocks::orientation_of(s).unwrap();
    s = o.with(s,"age",&age.min(15).to_string()).unwrap_or(s);
    if !blocks::full_cube(below) && fuel(w,(p.0,p.1-1,p.2)).0 == 0 {
        for (name,d) in [("east",(1,0,0)),("west",(-1,0,0)),("north",(0,0,-1)),("south",(0,0,1)),("up",(0,1,0))] {
            s = o.with(s,name,if fuel(w,add(p,d)).0 > 0 { "true" } else { "false" }).unwrap_or(s);
        }
    }
    s
}

pub fn ignite(w: &mut World, p: Pos) -> bool {
    if w.get_block(p.0,p.1,p.2) != AIR { return false; }
    let below = w.get_block(p.0, p.1 - 1, p.2);
    let soul = matches!(blocks::block_at_state(below), Some("soul_sand" | "soul_soil"));
    if !soul && !blocks::full_cube(below) && neighbours(w,p) == 0 { return false; }
    w.set_block(p.0,p.1,p.2,state_at(w,p,0))
}

pub fn tick(w: &mut World, p: Pos) {
    let s = w.get_block(p.0,p.1,p.2);
    if !matches!(blocks::block_at_state(s), Some("fire" | "soul_fire")) { return; }
    let delay = 30 + roll(w,10) as u64;
    w.schedule_block_once(p.0,p.1,p.2,delay);
    if !w.fire_tick { return; }
    let below = w.get_block(p.0,p.1-1,p.2);
    if blocks::block_at_state(s) == Some("soul_fire") {
        if !matches!(blocks::block_at_state(below),Some("soul_sand" | "soul_soil")) { w.set_block(p.0,p.1,p.2,AIR); }
        return;
    }
    let infinite = eternal(w,below);
    let age = blocks::orientation_of(s).and_then(|o| o.value(s,"age")).and_then(|v| v.parse::<u32>().ok()).unwrap_or(0);
    let wet = w.rain_at(p.0,p.1,p.2) || [(1,0,0),(-1,0,0),(0,0,1),(0,0,-1)].iter().any(|&d| { let q=add(p,d); w.rain_at(q.0,q.1,q.2) });
    if !infinite && ((wet && w.random_unit() < 0.2 + age as f32 * 0.03) || (neighbours(w,p)==0 && (!blocks::full_cube(below) || age>3)) || (age==15 && fuel(w,(p.0,p.1-1,p.2)).0==0 && roll(w,4)==0)) {
        w.set_block(p.0,p.1,p.2,AIR); return;
    }
    let next_age = (age + roll(w,3)/2).min(15);
    // Возраст огня клиентам не рассылается (чёрный ящик, fire-spread: у вечного
    // огня на нетерраке за 400 тактов ни одного обновления, а соседний огонь
    // появляется сразу с возрастом 3) — как запись с флагом 260.
    w.set_block_quietly(p.0,p.1,p.2,state_at(w,p,next_age));
    for d in SIDES {
        let q = add(p,d);
        let chance = fuel(w,q).1;
        if chance == 0 || roll(w,if d.1==0 {300} else {250}) >= chance { continue; }
        if crate::tnt::is_tnt(w.get_block(q.0,q.1,q.2)) { crate::tnt::ignite(w,q); continue; }
        let burn = roll(w,age+10)<5 && !w.rain_at(q.0,q.1,q.2);
        let s = if burn { let a=(age+roll(w,5)/4).min(15); state_at(w,q,a) } else { AIR };
        w.set_block(q.0,q.1,q.2,s);
    }
    for dx in -1..=1 { for dz in -1..=1 { for dy in -1..=4 {
        let q=add(p,(dx,dy,dz));
        if q==p || w.get_block(q.0,q.1,q.2)!=AIR || w.rain_at(q.0,q.1,q.2) { continue; }
        let encouragement=neighbours(w,q);
        if encouragement==0 { continue; }
        let chance=(encouragement+40+u32::from(w.fire_difficulty)*7)/(age+30);
        let bound=if dy>1 {100+(dy as u32-1)*100} else {100};
        if chance>0 && roll(w,bound)<=chance { let a=(age+roll(w,5)/4).min(15); w.set_block(q.0,q.1,q.2,state_at(w,q,a)); }
    } } }
}

pub fn lava_tick(w: &mut World, p: Pos) {
    if !w.fire_tick { return; }
    let steps=roll(w,3);
    let mut q=p;
    if steps==0 {
        for _ in 0..3 {
            let x=p.0+roll(w,3) as i32-1; let z=p.2+roll(w,3) as i32-1;
            if fuel(w,(x,p.1,z)).0>0 { ignite(w,(x,p.1+1,z)); }
        }
    } else {
        for _ in 0..steps {
            q=(q.0+roll(w,3) as i32-1,q.1+1,q.2+roll(w,3) as i32-1);
            let s=w.get_block(q.0,q.1,q.2);
            if s==AIR && neighbours(w,q)>0 { ignite(w,q); return; }
            if blocks::has_box(s) { return; }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fuels_exclude_nether_wood_and_stone() {
        assert_eq!(flammability("oak_planks"),(5,20));
        assert_eq!(flammability("oak_log"),(5,5));
        assert_eq!(flammability("white_wool"),(30,60));
        assert_eq!(flammability("crimson_planks"),(0,0));
        assert_eq!(flammability("stone_stairs"),(0,0));
    }
    #[test]
    fn eternal_fire_ages_but_soul_fire_never_spreads() {
        let mut w = World::in_memory();
        w.set_block(0, 69, 0, blocks::state_by_name("netherrack").unwrap());
        assert!(ignite(&mut w, (0, 70, 0)));
        for _ in 0..200 { tick(&mut w, (0, 70, 0)); }
        let s = w.get_block(0, 70, 0);
        assert_eq!(blocks::orientation_of(s).unwrap().value(s, "age"), Some("15"));
        w.set_block(0, 70, 0, AIR);
        w.set_block(0, 69, 0, blocks::state_by_name("soul_soil").unwrap());
        w.set_block(1, 70, 0, blocks::state_by_name("oak_planks").unwrap());
        assert!(ignite(&mut w, (0, 70, 0)));
        for _ in 0..200 { tick(&mut w, (0, 70, 0)); }
        assert_eq!(blocks::block_at_state(w.get_block(0, 70, 0)), Some("soul_fire"));
        assert_eq!(blocks::block_at_state(w.get_block(1, 70, 0)), Some("oak_planks"));
        w.set_block(0, 70, 0, AIR);
        w.set_block(0, 69, 0, blocks::state_by_name("soul_sand").unwrap());
        assert!(ignite(&mut w, (0, 70, 0)));
        assert_eq!(blocks::block_at_state(w.get_block(0, 70, 0)), Some("soul_fire"));
    }

    #[test]
    fn stopped_rule_preserves_fire_and_fuel() {
        let mut w=World::in_memory();
        w.set_block(0,69,0,blocks::state_by_name("netherrack").unwrap());
        w.set_block(1,70,0,blocks::state_by_name("oak_planks").unwrap());
        assert!(ignite(&mut w,(0,70,0)));
        w.fire_tick=false;
        for _ in 0..100 { tick(&mut w,(0,70,0)); }
        assert_eq!(blocks::block_at_state(w.get_block(0,70,0)),Some("fire"));
        assert_eq!(blocks::block_at_state(w.get_block(1,70,0)),Some("oak_planks"));
        assert!(eternal(&w,w.get_block(0,69,0)));
    }
}
