// Ровный мир: животные приходят циклом, а не вместе с каждым новым чанком.
// Вики, Superflat: «Mobs still spawn normally»; Mob spawning: «every 400
// game ticks», Creature cap 10. Отсутствие начальных стад [проверить:
// сценарий superflat-passive-census, запись оригинала ещё нужна].
use super::{Kind, Mob, Rng, View, fits, wild_colour};
use crate::{blocks, mob::ai::Watcher};
use std::collections::BTreeSet;

pub const INTERVAL: u64 = 400;
const CAP: usize = 10;
// Вики, Plains, «Creature category»: вес и размер одной группы.
const PLAINS: [(Kind, u32, u32, u32); 6] = [
    (Kind::Sheep, 12, 4, 4), (Kind::Pig, 10, 4, 4),
    (Kind::Chicken, 10, 4, 4), (Kind::Cow, 8, 4, 4),
    (Kind::Horse, 5, 2, 6), (Kind::Donkey, 1, 1, 3),
];

fn counted(m: &Mob) -> bool {
    !m.is_dead() && !m.persistent_tag && matches!(m.kind,
        Kind::Allay | Kind::Armadillo | Kind::Bee | Kind::Camel | Kind::Cat |
        Kind::Chicken | Kind::Cow | Kind::Donkey | Kind::Fox | Kind::Frog |
        Kind::Goat | Kind::HappyGhast | Kind::Horse | Kind::Llama | Kind::Mooshroom |
        Kind::Mule | Kind::Ocelot | Kind::Panda | Kind::Parrot | Kind::Pig |
        Kind::PolarBear | Kind::Rabbit | Kind::Sheep | Kind::SkeletonHorse |
        Kind::Sniffer | Kind::Strider | Kind::TraderLlama | Kind::Turtle |
        Kind::WanderingTrader | Kind::Wolf)
}

fn in_range(p: &Watcher, cx: i32, cz: i32) -> bool {
    (p.x - f64::from(cx * 16 + 8)).powi(2) + (p.z - f64::from(cz * 16 + 8)).powi(2) < 128.0 * 128.0
}

fn chunks(view: &dyn View, players: &[&Watcher]) -> BTreeSet<(i32, i32)> {
    let mut result = BTreeSet::new();
    for p in players {
        let (cx, cz) = ((p.x.floor() as i32).div_euclid(16), (p.z.floor() as i32).div_euclid(16));
        for x in cx-8..=cx+8 { for z in cz-8..=cz+8 {
            if view.loaded(x,z) { result.insert((x,z)); }
        }}
    }
    result
}

fn select(rng: &mut Rng) -> (Kind, u32) {
    let mut roll = rng.below(46);
    for (kind, weight, low, high) in PLAINS {
        if roll < weight { return (kind, low + rng.below(high-low+1)); }
        roll -= weight;
    }
    unreachable!()
}

/// Один цикл: общие чанки игроков считаются один раз; глобальный предел
/// масштабируется числом чанков, местный — 10 на игрока. Последняя группа
/// может превысить предел (вики, Mob spawning, «Global mob cap»).
/// [проверить] Точное число неудачных попыток и порядок обхода чанков.
pub fn cycle(view: &dyn View, watchers: &[Watcher], mobs: &[Mob], spawn: (f64,f64,f64), sky: i32, rng: &mut Rng, next: &mut dyn FnMut()->i32) -> Vec<Mob> {
    let players: Vec<_> = watchers.iter().filter(|p| !p.spectator).collect();
    let chunks = chunks(view, &players);
    let cap = CAP * chunks.len() / 289;
    let mut global = mobs.iter().filter(|m| counted(m)).count();
    let mut local: Vec<_> = players.iter().map(|p| mobs.iter().filter(|m| counted(m) && in_range(p, m.chunk().0, m.chunk().1)).count()).collect();
    let mut born = Vec::new();
    for (cx, cz) in chunks {
        if global >= cap { break; }
        if !players.iter().enumerate().any(|(i,p)| local[i] < CAP && in_range(p,cx,cz)) { continue; }
        let (base_x,base_z) = (cx*16 + rng.below(16) as i32, cz*16 + rng.below(16) as i32);
        let top = (-64..320).rev().find(|&y| view.block(base_x,y,base_z) != crate::world::AIR).unwrap_or(-64);
        let y = -64 + rng.below((top+66) as u32) as i32;
        if super::solid(view.block(base_x,y,base_z)) { continue; }
        for _ in 0..3 {
            if global >= cap || !players.iter().enumerate().any(|(i,p)| local[i] < CAP && in_range(p,cx,cz)) { break; }
            let (kind, want) = select(rng);
            let (mut x,mut z) = (base_x,base_z);
            let coat = rng.below(7) as i32;
            for _ in 0..want {
                x += rng.below(6) as i32 - rng.below(6) as i32;
                z += rng.below(6) as i32 - rng.below(6) as i32;
                let at = (f64::from(x)+0.5,f64::from(y),f64::from(z)+0.5);
                let dist = |p: (f64,f64,f64)| (p.0-at.0).powi(2)+(p.1-at.1).powi(2)+(p.2-at.2).powi(2);
                let nearest = players.iter().map(|p| dist((p.x,p.y,p.z))).fold(f64::INFINITY,f64::min);
                if nearest <= 24.0*24.0 || nearest > 128.0*128.0 || dist(spawn) <= 24.0*24.0 { continue; }
                if !view.loaded(x.div_euclid(16),z.div_euclid(16)) || view.biome(x,y,z) != "plains"
                    || blocks::block_at_state(view.block(x,y-1,z)) != Some("grass_block") { continue; }
                let light = view.light(x,y,z,sky);
                if light.0.max(light.1) < 9 { continue; }
                let mut m = Mob::new(next(),kind,at);
                if !fits(view,&m) { continue; }
                // Существующие и только что появившиеся животные не должны
                // занимать один ящик (вики, Mob spawning, «collision box»).
                if mobs.iter().chain(&born).filter(|other| !other.is_dead()).any(|other| {
                    let width = (m.size().0 + other.size().0) * 0.5;
                    (other.x-m.x).abs() < width && (other.z-m.z).abs() < width
                        && other.y < m.y+m.size().1 && m.y < other.y+other.size().1
                }) { continue; }
                m.face((rng.unit()*360.0) as f32);
                if kind == Kind::Sheep { m.color = wild_colour(rng); }
                if kind == Kind::Chicken { m.egg_timer = 6000+rng.below(6001) as i32; }
                if crate::mob::horse::equine(kind) {
                    crate::mob::horse::initialize(&mut m,rng);
                    m.horse.variant = coat | ((rng.below(5) as i32)<<8);
                }
                global += 1;
                for (i,p) in players.iter().enumerate() {
                    if in_range(p,m.chunk().0,m.chunk().1) { local[i] += 1; }
                }
                born.push(m);
            }
        }
    }
    born
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Pad { water: bool, loaded: bool }
    impl View for Pad {
        fn block(&self, _:i32,y:i32,_:i32)->i32 {
            blocks::state_by_name(if y==0 {"grass_block"} else if y<0 {"dirt"} else if y==1 && self.water {"water"} else {"air"}).unwrap()
        }
        fn loaded(&self,_:i32,_:i32)->bool { self.loaded }
        fn biome(&self,_:i32,_:i32,_:i32)->&'static str {"plains"}
        fn light(&self,_:i32,_:i32,_:i32,sky:i32)->(u8,u8) {(sky as u8,0)}
    }
    fn player() -> Watcher { Watcher { x: 1000.0, y: 1.0, z: 1000.0, id: 1, eye: 1.62, holding: [None; 2], spectator: false, targetable: true, look: (0.0,0.0), pumpkin: false, gold_armor: false, sprinting: false, riding: false } }
    #[test]
    fn plains_weights_groups_and_shared_chunks() {
        let mut rng=Rng::new(21); let mut counts=[0;6];
        for _ in 0..46000 { let (kind,n)=select(&mut rng); let i=PLAINS.iter().position(|v|v.0==kind).unwrap(); counts[i]+=1; assert!((PLAINS[i].2..=PLAINS[i].3).contains(&n)); }
        for (i,(_,w,_,_)) in PLAINS.iter().enumerate() { assert!((counts[i] as f64 / 1000.0-f64::from(*w)).abs()<0.6); }
        let p=player(); let pad=Pad{water:false,loaded:true};
        assert_eq!(chunks(&pad,&[&p,&p]).len(),289);
        assert_eq!(chunks(&pad,&[]).len(),0);
    }
    #[test]
    fn cap_distance_water_and_repopulation() {
        let p=player(); let pad=Pad{water:false,loaded:true}; let mut mobs=Vec::new(); let mut rng=Rng::new(42);
        for _ in 0..100 { mobs.extend(cycle(&pad,std::slice::from_ref(&p),&mobs,(0.0,1.0,0.0),15,&mut rng,&mut ||1)); }
        assert!((10..=15).contains(&mobs.len()),"{}",mobs.len());
        for m in &mobs { assert!(m.y==1.0); let d=(m.x-p.x).powi(2)+(m.z-p.z).powi(2); assert!(d>24.0*24.0 && d<=128.0*128.0); }
        assert!(cycle(&pad,std::slice::from_ref(&p),&mobs,(0.0,1.0,0.0),15,&mut rng,&mut ||1).is_empty());
        assert!(mobs.iter().all(counted));
        mobs[0].persistent_tag=true; assert!(!counted(&mobs[0]));
        for pad in [Pad{water:true,loaded:true},Pad{water:false,loaded:false}] {
            for _ in 0..40 { assert!(cycle(&pad,std::slice::from_ref(&p),&[],(0.0,1.0,0.0),15,&mut rng,&mut ||1).is_empty()); }
        }
        let mut spectator=p; spectator.spectator=true;
        assert!(cycle(&pad,&[spectator],&[],(0.0,1.0,0.0),15,&mut rng,&mut ||1).is_empty());
    }
}
