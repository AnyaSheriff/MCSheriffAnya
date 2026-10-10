// Наземные животные. Правила: minecraft.wiki/w/Llama, Goat, Panda,
// Camel, Polar_Bear, Ocelot, Mooshroom. [проверить] Такты целей и анимаций
// пока не сверены чёрным ящиком; сценарии приготовлены отдельно.
use super::{ai::{self, Ctx, Plan}, path, Event, Kind, Mob, Rng, View};
use crate::{blocks, inventory::Stack};

#[derive(Clone, Debug, PartialEq)]
pub struct State {
    pub variant: u8,
    pub strength: u8,
    pub tamed: bool,
    pub temper: u8,
    pub chest: bool,
    pub decor: Option<u8>,
    pub cargo: Vec<Option<Stack>>,
    pub saddle: bool,
    pub sitting: bool,
    pub pose_tick: i64,
    pub dash: i32,
    pub rider: Option<i32>,
    pub mount_ticks: i32,
    pub screaming: bool,
    pub horns: [bool; 2],
    pub horn: u8,
    pub ram_wait: i32,
    pub ram_ticks: i32,
    pub ram_at: Option<(f64, f64, f64)>,
    pub ram_candidate: Option<(i32, (f64,f64,f64), i32)>,
    pub jump_wait: i32,
    pub main_gene: u8,
    pub hidden_gene: u8,
    pub sneeze: i32,
    pub eating: i32,
    pub held: Option<Stack>,
    pub rolling: i32,
    pub worried: bool,
    pub trusting: bool,
    pub brown: bool,
    pub flower: u8,
    pub target: Option<i32>,
    pub attack_wait: i32,
    pub caravan: Option<i32>,
    pub caravan_at: Option<(f64, f64, f64)>,
    pub trader: Option<i32>,
    pub despawn: i32,
    pub invisible: bool,
    pub drinking: i32,
    pub standing: bool,
    pub max_health: f32,
    pub lightning: Option<i32>,
    pub saved_uuid: Option<[u8;16]>,
}
impl State {
    pub fn new(kind: Kind) -> Self {
        Self { variant: 0, strength: 1, tamed: false, temper: 0, chest: false, decor: None,
            cargo: Vec::new(), saddle: false, sitting: false, pose_tick: 0, dash: 0,
            rider: None, mount_ticks: 0, screaming: false, horns: [true; 2], horn: 0,
            ram_wait: 600, ram_ticks: 0, ram_at: None, ram_candidate: None, jump_wait: 600,
            main_gene: 0, hidden_gene: 0, sneeze: 0, eating: 0, held: None, rolling: 0, worried: false,
            trusting: false, brown: false, flower: 0, target: None, attack_wait: 0,
            caravan: None, caravan_at: None, trader: None,
            despawn: if kind == Kind::TraderLlama { 47999 } else { 0 },
            invisible: false, drinking: 0, standing: false, max_health: kind.max_health(), lightning: None, saved_uuid: None }
    }
    pub fn slots(&self) -> usize { if self.chest { usize::from(self.strength.clamp(1, 5)) * 3 } else { 0 } }
    pub fn personality(&self) -> u8 {
        if matches!(self.main_gene, 4 | 5) && self.hidden_gene != self.main_gene { 0 } else { self.main_gene }
    }
}
pub fn animal(kind: Kind) -> bool {
    matches!(kind, Kind::Llama | Kind::TraderLlama | Kind::Camel | Kind::Goat | Kind::PolarBear | Kind::Panda | Kind::Ocelot | Kind::Mooshroom)
}
pub fn llama(kind: Kind) -> bool { matches!(kind, Kind::Llama | Kind::TraderLlama) }
pub fn rideable(kind: Kind) -> bool { llama(kind) || kind == Kind::Camel }
pub fn health(mob: &mut Mob) {
    if llama(mob.kind) { mob.vitals.max_health = mob.land.max_health; }
    if mob.kind == Kind::Panda { mob.vitals.max_health = if mob.land.personality() == 5 { 10.0 } else { 20.0 }; }
    // Чёрный ящик, panda-sneeze: у слабой панды здоровье остаётся 20 при наибольшем 10.
    if mob.kind != Kind::Panda { mob.vitals.health = mob.vitals.health.min(mob.vitals.max_health); }
}
fn gene(rng: &mut Rng) -> u8 {
    // Вики, Panda: «main gene and hidden gene»; распределение из 16 долей.
    match rng.below(16) { 0 => 1, 1 => 2, 2 => 3, 3 => 6, 4..=5 => 4, 6..=10 => 5, _ => 0 }
}
pub fn initialize(mob: &mut Mob, rng: &mut Rng) {
    if llama(mob.kind) {
        mob.land.variant = rng.below(4) as u8;
        let max = if rng.below(100) < 4 { 5 } else { 3 };
        mob.land.strength = (1 + rng.below(max)) as u8;
        mob.land.max_health = (15 + rng.below(8) + rng.below(9)) as f32;
    }
    if mob.kind == Kind::Goat {
        mob.land.screaming = rng.below(50) == 0;
        mob.land.horn = rng.below(4) as u8 + if mob.land.screaming { 4 } else { 0 };
        mob.land.ram_wait = ram_pause(mob.land.screaming, rng);
    }
    if mob.kind == Kind::Panda { mob.land.main_gene = gene(rng); mob.land.hidden_gene = gene(rng); }
    health(mob);
    mob.vitals.health = mob.vitals.max_health;
}
pub fn inherit(baby: &mut Mob, a: &Mob, b: &Mob, rng: &mut Rng) {
    initialize(baby, rng);
    if llama(baby.kind) {
        baby.kind = Kind::Llama;
        baby.land.variant = if rng.below(2) == 0 { a.land.variant } else { b.land.variant };
        baby.land.strength = (1 + rng.below(u32::from(a.land.strength.max(b.land.strength))) + u32::from(rng.below(100) < 3)).min(5) as u8;
    }
    if baby.kind==Kind::Goat { baby.land.screaming=(if rng.below(2)==0 { a.land.screaming } else { b.land.screaming }) || rng.below(50)==0; baby.land.horn=baby.land.horn%4 + if baby.land.screaming { 4 } else { 0 }; }
    if baby.kind == Kind::Panda {
        let pick = |p: &Mob, r: &mut Rng| if r.below(2) == 0 { p.land.main_gene } else { p.land.hidden_gene };
        baby.land.main_gene = pick(a, rng); baby.land.hidden_gene = pick(b, rng);
        if rng.below(2) == 0 { std::mem::swap(&mut baby.land.main_gene, &mut baby.land.hidden_gene); }
        if rng.below(32) == 0 { baby.land.main_gene = gene(rng); }
        if rng.below(32) == 0 { baby.land.hidden_gene = gene(rng); }
    }
    if baby.kind == Kind::Mooshroom {
        baby.land.brown = if a.land.brown == b.land.brown { a.land.brown ^ (rng.below(1024) == 0) } else if rng.below(2) == 0 { a.land.brown } else { b.land.brown };
    }
    health(baby);
    baby.vitals.health = baby.vitals.max_health;
}
fn sound(mob: &mut Mob, name: &'static str) { mob.events.push(Event::Sound { name, volume: 1.0, pitch: 1.0 }); }
fn drop_item(mob: &mut Mob, name: &str, count: i32) {
    if let Some(id) = blocks::item_named(name) { mob.events.push(Event::Loot(vec![Stack::new(id, count)])); }
}
pub fn on_hurt(mob: &mut Mob, cause: i32) {
    if llama(mob.kind) || matches!(mob.kind, Kind::PolarBear | Kind::Panda | Kind::WanderingTrader) {
        // Чёрный ящик, llama-spit: первый плевок — через 41 такт после удара
        // (RangedAttack: счётчик взводится на 40 тактов на следующем такте).
        if llama(mob.kind) && mob.land.target.is_none() && mob.land.attack_wait == 0 { mob.land.attack_wait = 42; }
        mob.land.target = Some(cause);
        if !mob.is_baby() && mob.kind != Kind::WanderingTrader { mob.brain = ai::Brain::new(mob.entity_id); }
    }
    if mob.kind == Kind::Camel { mob.land.sitting = false; }
}
pub fn tick(mob: &mut Mob, rng: &mut Rng) {
    if !animal(mob.kind) && mob.kind != Kind::WanderingTrader { return; }
    if mob.kind == Kind::WanderingTrader && mob.villager.trading.is_none() && mob.land.despawn > 0 {
        mob.land.despawn -= 1;
        mob.h.despawn |= mob.land.despawn == 0;
    }
    if mob.kind == Kind::TraderLlama && !mob.land.tamed && mob.land.rider.is_none() && mob.land.despawn > 0 {
        mob.land.despawn -= 1;
        mob.h.despawn |= mob.land.despawn == 0;
    }
    if mob.no_ai { return; }
    mob.land.attack_wait = (mob.land.attack_wait - 1).max(0);
    if mob.kind==Kind::Camel && mob.land.dash==1 { sound(mob,"minecraft:entity.camel.dash_ready"); }
    mob.land.dash = (mob.land.dash - 1).max(0);
    mob.land.jump_wait = (mob.land.jump_wait - 1).max(0);
    mob.land.ram_wait = (mob.land.ram_wait - 1).max(0);
    if llama(mob.kind) && !mob.land.tamed && mob.land.rider.is_some() {
        mob.land.mount_ticks += 1;
        if mob.land.mount_ticks >= 100 {
            mob.land.mount_ticks = 0;
            if rng.below(30) < u32::from(mob.land.temper) {
                mob.land.tamed = true; mob.events.push(Event::Status(7));
            } else {
                mob.land.temper = (mob.land.temper + 5).min(30);
                mob.land.rider = None; mob.events.push(Event::Status(6));
            }
        }
    }
    if mob.kind == Kind::Panda {
        if mob.land.sneeze > 0 {
            mob.land.sneeze += 1;
            if mob.land.sneeze > 20 {
                mob.land.sneeze = 0;
                sound(mob, "minecraft:entity.panda.sneeze");
                if rng.below(700) == 0 { drop_item(mob, "slime_ball", 1); }
            }
        } else if mob.is_baby() && rng.below(if mob.land.personality() == 5 { 500 } else { 6000 }) == 0 {
            mob.land.sneeze = 1; sound(mob, "minecraft:entity.panda.pre_sneeze");
        }
        if mob.land.eating > 0 {
            mob.land.eating -= 1;
            if mob.land.eating % 5 == 0 { sound(mob, "minecraft:entity.panda.eat"); }
            if mob.land.eating==0 { mob.land.held=None; mob.attr_changes=mob.attr_changes.wrapping_add(1); }
        }
        mob.land.rolling = (mob.land.rolling - 1).max(0);
        if mob.land.rolling == 0 && (mob.is_baby() || mob.land.personality() == 3) && rng.below(600) == 0 { mob.land.rolling = 32; }
    }
}
fn route(mob: &mut Mob, view: &dyn View, at: (f64, f64, f64), speed: f64) -> Plan {
    mob.brain.repath -= 1;
    if !mob.brain.walking() || mob.brain.repath <= 0 {
        mob.brain.repath = 10;
        let goal = (at.0.floor() as i32, at.1.floor() as i32, at.2.floor() as i32);
        mob.brain.go(path::find(view, mob.cell(), goal, path::MAX_NODES), speed);
    }
    ai::walk(mob, Plan::default())
}
fn ram_pause(screaming: bool, rng: &mut Rng) -> i32 {
    if screaming { 100 + rng.below(201) as i32 } else { 600 + rng.below(5401) as i32 }
}
pub fn horn_block(name: &str) -> bool {
    matches!(name, "stone" | "packed_ice" | "coal_ore" | "copper_ore" | "emerald_ore" | "iron_ore") || name.ends_with("_log")
}
pub fn think(mob: &mut Mob, view: &dyn View, ctx: &Ctx, rng: &mut Rng) -> Plan {
    if mob.kind == Kind::WanderingTrader {
        super::trader::think(mob, ctx.sun_up);
        if let Some(enemy)=ctx.mates.iter().find(|m| matches!(m.kind,Kind::Zombie|Kind::ZombieVillager|Kind::Drowned|Kind::Husk|Kind::ZombifiedPiglin) && (m.x-mob.x).hypot(m.z-mob.z)<8.0) {
            mob.fright=Some((enemy.x,enemy.z)); if !mob.brain.is_panicking() { mob.brain.panic(20); }
        }
        if mob.villager.trading.is_some() { mob.brain.stop(); return Plan::default(); }
    }
    if mob.kind == Kind::Camel && mob.land.rider.is_none() && mob.on_ground && rng.below(400) == 0 {
        mob.land.sitting = !mob.land.sitting;
        mob.land.pose_tick = if mob.land.sitting { -mob.tame.clock.abs().max(1) } else { mob.tame.clock.abs() };
    }
    if mob.kind==Kind::Panda {
        mob.land.worried=mob.land.personality()==2 && view.thunder();
        if mob.land.worried { mob.brain.stop(); return Plan::default(); }
        if mob.land.personality()==1 && rng.below(600)==0 { mob.land.sitting= !mob.land.sitting; }
        if mob.land.rolling>0 && mob.on_ground {
            let angle=f64::from(mob.yaw).to_radians(); mob.vx= -angle.sin()*0.15; mob.vz=angle.cos()*0.15;
            return Plan::default();
        }
    }
    if mob.kind==Kind::Camel && mob.land.rider.is_some() && mob.land.saddle { mob.brain.stop(); return Plan::default(); }
    if mob.land.sitting || (mob.kind == Kind::Panda && (mob.land.eating > 0 || mob.land.sneeze > 0)) {
        mob.brain.stop(); return Plan::default();
    }
    if mob.kind == Kind::Ocelot && !mob.land.trusting
        && let Some(p) = ctx.players.iter().find(|p| p.targetable && (p.x-mob.x).hypot(p.z-mob.z) < 8.0 && !p.holding.iter().flatten().any(|s| matches!(*s, "cod" | "salmon")))
    {
        mob.fright = Some((p.x, p.z)); if !mob.brain.is_panicking() { mob.brain.panic(20); }
    }
    if mob.kind == Kind::Goat {
        if mob.land.ram_ticks > 0 {
            mob.land.ram_ticks -= 1;
            if mob.land.ram_ticks==0 { mob.events.push(Event::Status(59)); }
            if let Some(at) = mob.land.ram_at {
                mob.face((-(at.0-mob.x).atan2(at.2-mob.z)).to_degrees() as f32);
                // Чёрный ящик, goat-ram: подготовка — событие 58 каждый такт (21 раз) и
                // звук prepare_ram у сущности на последнем; потом разбег.
                if mob.land.ram_ticks > 20 {
                    mob.events.push(Event::Status(58));
                    if mob.land.ram_ticks == 21 {
                        let name = if mob.land.screaming { "minecraft:entity.goat.screaming.prepare_ram" } else { "minecraft:entity.goat.prepare_ram" };
                        mob.events.push(Event::EntitySound { name, category: 6, volume: 1.0, pitch: 0.8 + rng.below(1000) as f32 / 1000.0 * 0.4 });
                    }
                    return Plan::default();
                }
                let ahead = (mob.x - f64::from(mob.yaw).to_radians().sin(), mob.y, mob.z + f64::from(mob.yaw).to_radians().cos());
                let block = blocks::block_at_state(view.block(ahead.0.floor() as i32, ahead.1.floor() as i32, ahead.2.floor() as i32));
                if block.is_some_and(horn_block) && !mob.is_baby() {
                    if let Some(horn) = mob.land.horns.iter_mut().find(|h| **h) { *horn = false;
                        if let Some(id)=blocks::item_named("goat_horn") {
                            let d=crate::stackdata::Data { instrument: Some(mob.land.horn), ..Default::default() };
                            mob.events.push(Event::Loot(vec![Stack::new(id,1).with_extras(d)]));
                        } }
                    mob.land.ram_ticks = 0; mob.events.push(Event::Status(59));
                    sound(mob, "minecraft:entity.goat.ram_impact");
                }
                let victim=mob.land.target.and_then(|id| {
                    ctx.mates.iter().find(|m| m.id==id).map(|m| (id,(m.x,m.y,m.z)))
                        .or_else(|| ctx.players.iter().find(|p| p.id==id && p.targetable).map(|p| (id,(p.x,p.y,p.z))))
                }).filter(|(_,p)| (p.0-mob.x).hypot(p.2-mob.z)<1.4 && (p.1-mob.y).abs()<1.5);
                if let Some((id,p))=victim {
                    let mut plan = Plan::default();
                    attack(&mut plan, ctx, id, if mob.is_baby() { 1.0 } else { 2.0 }, (p.0-mob.x, p.2-mob.z));
                    mob.land.target=None; mob.land.ram_ticks = 0; mob.events.push(Event::Status(59));
                    let name = if mob.land.screaming { "minecraft:entity.goat.screaming.ram_impact" } else { "minecraft:entity.goat.ram_impact" };
                    mob.events.push(Event::EntitySound { name, category: 6, volume: 1.0, pitch: 1.0 });
                    return plan;
                }
                if (at.0-mob.x).hypot(at.2-mob.z)<0.5 { mob.land.ram_ticks=0; mob.events.push(Event::Status(59)); return Plan::default(); }
                return Plan { speed: 3.0, ..Plan::default() };
            }
        }
        if mob.land.ram_wait == 0 {
            let target = ctx.mates.iter().filter(|m| m.id != mob.entity_id && m.kind != Kind::Goat).map(|m| (m.id,(m.x,m.y,m.z)))
                .chain(ctx.players.iter().filter(|p| p.targetable).map(|p| (p.id,(p.x,p.y,p.z))))
                .find(|(_,p)| (p.0-mob.x).hypot(p.2-mob.z) >= 4.0 && (p.0-mob.x).hypot(p.2-mob.z) <= 16.0);
            if let Some((id, at)) = target.filter(|(_,at)| super::hostile::sees(view,(mob.x,mob.y+mob.eye(),mob.z),(at.0,at.1+0.5,at.2))) {
                let still=mob.land.ram_candidate.filter(|(old,p,_)| *old==id && (p.0-at.0).abs()+(p.1-at.1).abs()+(p.2-at.2).abs()<0.01).map_or(1,|(_,_,n)| n+1);
                mob.land.ram_candidate=Some((id,at,still));
                // [проверить] Несколько секунд неподвижности перед подготовкой.
                if still>=17 {
                    let (dx,dz)=(at.0-mob.x,at.2-mob.z); let len=dx.hypot(dz).max(0.001);
                    mob.land.target = Some(id); mob.land.ram_at = Some((at.0+dx/len*2.0,at.1,at.2+dz/len*2.0)); mob.land.ram_ticks = 41;
                    mob.land.ram_wait = ram_pause(mob.land.screaming, rng); mob.events.push(Event::Status(58));
                    mob.land.ram_candidate=None;
                }
            } else { mob.land.ram_candidate=None; }
        }
        if mob.land.jump_wait == 0 && mob.on_ground && mob.land.ram_ticks == 0 {
            mob.land.jump_wait = 600 + rng.below(1201) as i32;
            mob.vy = 1.0; mob.on_ground = false; mob.impulses = mob.impulses.wrapping_add(1);
        }
    }

    if mob.kind == Kind::PolarBear && !mob.is_baby() && ctx.mates.iter().any(|m| m.kind == Kind::PolarBear && m.baby && (m.x-mob.x).hypot(m.z-mob.z) < 8.0) {
        mob.land.target = mob.land.target.or_else(|| ctx.players.iter().find(|p| p.targetable && (p.x-mob.x).hypot(p.z-mob.z) < 8.0).map(|p| p.id));
    }
    if ctx.difficulty > 0 && !mob.is_baby() && mob.kind != Kind::Goat && mob.kind != Kind::WanderingTrader
        && let Some(id) = mob.land.target
    {
        let at = ctx.mates.iter().find(|m| m.id == id).map(|m| (m.x,m.y,m.z)).or_else(|| ctx.players.iter().find(|p| p.id == id && p.targetable).map(|p| (p.x,p.y,p.z)));
        if let Some(at) = at.filter(|p| (p.0-mob.x).hypot(p.2-mob.z) < 32.0) {
            let distance = (at.0-mob.x).hypot(at.2-mob.z);
            ai::look_at_point(mob, at.0, at.1 + 0.5, at.2);
            if llama(mob.kind) && distance < 20.0 && mob.land.attack_wait == 0 && super::hostile::sees(view,(mob.x,mob.y+mob.eye(),mob.z),(at.0,at.1+0.5,at.2)) {
                // Чёрный ящик, llama-spit: плевок появляется на 0,95 впереди тела, на высоте 1,68.
                let body = f64::from(mob.yaw).to_radians();
                let from = (mob.x - 0.95 * body.sin(), mob.y + 1.68, mob.z + 0.95 * body.cos());
                let (dx,dy,dz) = (at.0-from.0, at.1+0.5-from.1+distance*0.2, at.2-from.2);
                let len = (dx*dx+dy*dy+dz*dz).sqrt().max(0.001);
                mob.land.attack_wait = 40; mob.land.target = None;
                sound(mob, "minecraft:entity.llama.spit");
                return Plan { spit: Some((from, (dx/len*1.5,dy/len*1.5,dz/len*1.5))), ..Plan::default() };
            }
            if !llama(mob.kind) && distance < 2.0 && mob.land.attack_wait == 0 && super::hostile::sees(view,(mob.x,mob.y+mob.eye(),mob.z),(at.0,at.1+0.5,at.2)) {
                let mut plan = Plan::default();
                attack(&mut plan, ctx, id, 6.0, (at.0-mob.x, at.2-mob.z));
                mob.land.attack_wait = 20;
                if mob.kind == Kind::Panda && mob.land.personality() != 6 { mob.land.target = None; }
                return plan;
            }
            mob.land.standing = mob.kind == Kind::PolarBear && distance < 4.0;
            return route(mob, view, at, 1.25);
        }
        mob.land.target = None;
    }
    if let Some(at) = mob.land.caravan_at && mob.leash.is_none() && (at.0-mob.x).hypot(at.2-mob.z) > 2.0 { return route(mob, view, at, 2.1); }
    let mut plan = ai::think(mob, view, ctx, rng);
    if mob.kind == Kind::Panda && mob.land.personality() == 1 { plan.speed *= 0.5; }
    plan
}
fn attack(plan: &mut Plan, ctx: &Ctx, id: i32, damage: f32, dir: (f64,f64)) {
    let len = dir.0.hypot(dir.1).max(0.001); let push = (dir.0/len, dir.1/len);
    if ctx.players.iter().any(|p| p.id == id && p.targetable) {
        plan.melee = Some(super::hostile::Melee { target: id, damage, push, wither: false, hunger: false });
    } else { plan.bite = Some((id,damage,push)); }
}
// Вики, Llama: «up to nine llamas». Цепь без циклов, один хвост у головы.
pub fn herd(mobs: &mut [Mob], _players: &[ai::Watcher], _view: &dyn View) {
    let wolves: Vec<_>=mobs.iter().filter(|m| m.kind==Kind::Wolf && !m.tame.tamed() && !m.is_dead()).map(|m| (m.entity_id,m.x,m.z)).collect();
    let hurt: Vec<_>=mobs.iter().filter_map(|m| m.land.target.map(|t| (m.kind,m.x,m.z,t))).collect();
    for m in mobs.iter_mut() {
        if llama(m.kind) && m.land.target.is_none() { m.land.target=wolves.iter().find(|w| (w.1-m.x).hypot(w.2-m.z)<10.0).map(|w| w.0); }
        if !m.is_baby() && (m.kind==Kind::PolarBear || m.kind==Kind::Panda && m.land.personality()==6) {
            m.land.target=m.land.target.or_else(|| hurt.iter().find(|h| h.0==m.kind && (h.1-m.x).hypot(h.2-m.z)<8.0).map(|h| h.3));
        }
    }
    let mut chains: Vec<(i32,(f64,f64,f64),usize)> = mobs.iter().filter(|m| llama(m.kind) && m.leash.is_some() && !m.is_dead()).map(|m| (m.entity_id,(m.x,m.y,m.z),0)).collect();
    for mob in mobs.iter_mut().filter(|m| llama(m.kind)) { mob.land.caravan = None; mob.land.caravan_at = None; }
    for _ in 0..9 {
        for chain in &mut chains {
            if chain.2 >= 9 { continue; }
            let next = mobs.iter().enumerate().filter(|(_,m)| llama(m.kind) && m.leash.is_none() && !m.no_ai && !m.is_dead() && m.land.caravan.is_none())
                .map(|(i,m)| (i,(m.x-chain.1.0).hypot(m.z-chain.1.2)))
                .filter(|(_,d)| *d <= 9.0).min_by(|a,b| a.1.total_cmp(&b.1));
            if let Some((i,_)) = next {
                mobs[i].land.caravan = Some(chain.0); mobs[i].land.caravan_at = Some(chain.1);
                *chain = (mobs[i].entity_id,(mobs[i].x,mobs[i].y,mobs[i].z),chain.2+1);
            }
        }
    }
    let traders: Vec<_> = mobs.iter().filter(|m| m.kind == Kind::WanderingTrader).map(|m| (m.entity_id,m.land.despawn,m.land.target,m.uuid,m.villager.trading.is_some(),!m.is_dead())).collect();
    for mob in mobs.iter_mut().filter(|m| m.kind == Kind::TraderLlama && !m.land.tamed) {
        if let Some((id,delay,target,uuid,trading,alive)) = traders.iter().find(|t| matches!(mob.leash,Some(super::leash::Holder::Player { uuid,.. }) if uuid==t.3)) {
            mob.land.trader=alive.then_some(*id);
            mob.leash=Some(super::leash::Holder::Player { id:*id,uuid:*uuid });
            if *alive { mob.land.despawn = if *trading { delay.saturating_add(1) } else { *delay }.max(1); }
            mob.land.target = mob.land.target.or(*target);
        } else { mob.land.trader=None; }
    }
}
// Вики, Panda: «pick up and eat bamboo and cake». Подбор — общим журналом.
// [проверить] Длительность еды и радиус поиска.
pub fn pickup_food(items: &mut crate::items::Items) {
    let idle: Vec<_>=items.mobs().iter().enumerate()
        .filter(|(_,m)| m.kind==Kind::Panda && !m.is_dead() && !m.no_ai && !m.is_baby() && m.land.eating==0 && m.land.target.is_none())
        .map(|(i,m)| (i,m.entity_id,(m.x,m.y,m.z))).collect();
    for (index,id,at) in idle {
        let mut selected=false;
        let taken=items.take_near(at,id,|s| {
            if !selected && matches!(blocks::item_name(s.item),Some("bamboo"|"cake")) { selected=true; 1 } else { 0 }
        });
        if let Some(food)=taken.first() {
            let mob=&mut items.mobs_mut()[index]; mob.land.held=Some(food.stack); mob.land.eating=32;
            mob.attr_changes=mob.attr_changes.wrapping_add(1); mob.brain.stop();
        }
    }
}
pub fn bamboo_near(view: &dyn View, mob: &Mob) -> bool {
    let (x,y,z) = mob.cell();
    (-7..=7).any(|dx| (-7..=7).any(|dz| (-3..=3).any(|dy| blocks::block_at_state(view.block(x+dx,y+dy,z+dz)) == Some("bamboo"))))
}
pub fn dash(mob: &mut Mob, charge: i32) -> bool {
    if mob.kind != Kind::Camel || !mob.land.saddle || mob.land.sitting || !mob.on_ground || mob.land.dash > 0 { return false; }
    let power = if charge >= 90 { 1.0 } else { 0.4 + 0.4 * f64::from(charge.clamp(0, 90)) / 90.0 };
    let angle = f64::from(mob.yaw).to_radians();
    mob.vx -= angle.sin() * 1.5 * power; mob.vz += angle.cos() * 1.5 * power; mob.vy = 0.6 * power;
    sound(mob,"minecraft:entity.camel.dash");
    mob.land.dash = 55; mob.on_ground = false; mob.impulses = mob.impulses.wrapping_add(1); true
}
pub fn apply_data(mob: &mut Mob, data: &str) {
    if !animal(mob.kind) && mob.kind != Kind::WanderingTrader { return; }
    let Some(v) = crate::snbt::parse(data) else { return; };
    let n = |s| v.get(s).and_then(|v| v.as_f64()).map(|n| n as i32);
    if let Some(x) = n("Variant") { mob.land.variant = x.clamp(0,3) as u8; }
    if let Some(x) = n("Strength") { mob.land.strength = x.clamp(1,5) as u8; }
    if let Some(x) = n("Temper") { mob.land.temper = x.clamp(0,30) as u8; }
    if let Some(x) = n("Saddle") { mob.land.saddle = x != 0; }
    if let Some(x) = v.get("LastPoseChangeTick").and_then(|v| v.as_f64()) { mob.land.pose_tick=x as i64; mob.land.sitting=x<0.0; }
    if let Some(x) = n("Tame") { mob.land.tamed = x != 0; }
    if let Some(x) = n("ChestedHorse") { mob.land.chest = x != 0; }
    if let Some(x) = n("IsScreamingGoat") { mob.land.screaming = x != 0; mob.land.horn = mob.land.horn%4 + if mob.land.screaming { 4 } else { 0 }; }
    if let Some(x) = n("HasLeftHorn") { mob.land.horns[0] = x != 0; }
    if let Some(x) = n("HasRightHorn") { mob.land.horns[1] = x != 0; }
    if let Some(x) = n("Trusting") { mob.land.trusting = x != 0; }
    if let Some(x) = n("DespawnDelay") { mob.land.despawn = x.max(0); }
    if let Some(x) = v.get("Type").and_then(|v| v.as_str()) { mob.land.brown = x == "brown"; }
    for (key, dest) in [("MainGene", &mut mob.land.main_gene),("HiddenGene", &mut mob.land.hidden_gene)] {
        if let Some(x) = v.get(key).and_then(|v| v.as_str()).and_then(|s| ["normal","lazy","worried","playful","brown","weak","aggressive"].iter().position(|x| *x == s)) { *dest = x as u8; }
    }
    if let Some(x) = v.get("Brain").and_then(|b| b.get("memories")).and_then(|m| m.get("minecraft:ram_cooldown_ticks")).and_then(|v| v.get("value")).and_then(|v| v.as_f64()) {
        mob.land.ram_wait = (x as i32).max(0);
    }
    mob.land.cargo.resize(mob.land.slots(), None);
    if mob.kind == Kind::WanderingTrader { crate::villager::apply_data(&mut mob.villager, &v); }
    health(mob);
}
// Формат хвоста только новых видов; старые записи животных не меняются.
pub fn write(mob: &Mob, out: &mut Vec<u8>) {
    let s = &mob.land;
    out.extend_from_slice(&mob.uuid);
    out.extend_from_slice(&[s.variant,s.strength,u8::from(s.tamed),s.temper,u8::from(s.chest),s.decor.unwrap_or(255),u8::from(s.saddle),u8::from(s.sitting),u8::from(s.screaming),u8::from(s.horns[0]),u8::from(s.horns[1]),s.horn,s.main_gene,s.hidden_gene,u8::from(s.trusting),u8::from(s.brown),s.flower]);
    out.extend_from_slice(&s.despawn.to_be_bytes()); out.extend_from_slice(&s.max_health.to_be_bytes());
    out.extend_from_slice(&s.pose_tick.to_be_bytes());
    for i in 0..s.slots() {
        let slot=s.cargo.get(i).copied().flatten();
        out.push(u8::from(slot.is_some()));
        if let Some(stack) = slot { out.extend_from_slice(&stack.stored_item().to_be_bytes()); out.extend_from_slice(&stack.count.to_be_bytes()); stack.write_extras(out); }
    }
    crate::villager::write(&mob.villager, out);
    // Старый формат жителя не хранит компоненты товара; новый хвост хранит.
    for offer in &mob.villager.offers {
        for stack in [Some(offer.buy),offer.buy_b,Some(offer.sell)] {
            let data=stack.map(|s| crate::stackdata::to_bytes(&s.extras())).unwrap_or_default();
            out.extend_from_slice(&(data.len() as u16).to_be_bytes()); out.extend_from_slice(&data);
        }
    }
}
pub fn read(bytes: &mut &[u8]) -> Option<(State, crate::villager::State)> {
    let mut take = |n| { let (h,t) = bytes.split_at_checked(n)?; *bytes=t; Some(h) };
    let uuid: [u8;16]=take(16)?.try_into().ok()?;
    let h = take(17)?;
    if h[0]>3 || !(1..=5).contains(&h[1]) || h[12]>6 || h[13]>6 { return None; }
    let mut s = State::new(Kind::Llama);
    s.saved_uuid=Some(uuid);
    s.variant=h[0]; s.strength=h[1]; s.tamed=h[2]!=0; s.temper=h[3].min(30); s.chest=h[4]!=0; s.decor=(h[5]<16).then_some(h[5]);
    s.saddle=h[6]!=0; s.sitting=h[7]!=0; s.screaming=h[8]!=0; s.horns=[h[9]!=0,h[10]!=0]; s.horn=h[11].min(7);
    s.main_gene=h[12]; s.hidden_gene=h[13]; s.trusting=h[14]!=0; s.brown=h[15]!=0; s.flower=h[16];
    s.despawn=i32::from_be_bytes(take(4)?.try_into().ok()?).max(0); s.max_health=f32::from_be_bytes(take(4)?.try_into().ok()?);
    if !s.max_health.is_finite() || s.max_health<=0.0 { return None; }
    s.pose_tick=i64::from_be_bytes(take(8)?.try_into().ok()?);
    for _ in 0..s.slots() {
        let stack = if take(1)?[0]!=0 {
            let item=i32::from_be_bytes(take(4)?.try_into().ok()?); let count=i32::from_be_bytes(take(4)?.try_into().ok()?);
            let extras=Stack::read_extras(item,&mut take)?; let mut stack=Stack::from_stored(item,count);
            if let Some(data)=extras { stack=stack.with_extras(data); } Some(stack)
        } else { None };
        s.cargo.push(stack);
    }
    let mut trader=crate::villager::read(bytes)?;
    for offer in &mut trader.offers {
        for which in 0..3 {
            let (length,rest)=bytes.split_at_checked(2)?; *bytes=rest;
            let n=u16::from_be_bytes(length.try_into().ok()?) as usize;
            let (data,rest)=bytes.split_at_checked(n)?; *bytes=rest;
            if !data.is_empty() {
                let extras=crate::stackdata::from_bytes(data)?;
                match which { 0=>offer.buy=offer.buy.with_extras(extras),1=>offer.buy_b=offer.buy_b.map(|s| s.with_extras(extras)),_=>offer.sell=offer.sell.with_extras(extras) }
            }
        }
    }
    Some((s,trader))
}

// Вики, Suspicious Stew, «Flowers»: эффекты и длительности Java.
pub fn flower(name: &str) -> Option<u8> {
    Some(match name { "dandelion"|"blue_orchid"=>1,"poppy"|"torchflower"=>2,"allium"=>3,"azure_bluet"=>4,
        "red_tulip"|"orange_tulip"|"white_tulip"|"pink_tulip"=>5,"oxeye_daisy"=>6,"cornflower"=>7,
        "lily_of_the_valley"=>8,"wither_rose"=>9,"open_eyeblossom"=>10,"closed_eyeblossom"=>11,_=>return None })
}
pub fn stew(code:u8)->Option<(u8,i32)> {
    use crate::survival::EffectKind::*;
    let (effect,ticks)=match code { 1=>(Saturation,7),2=>(NightVision,100),3=>(FireResistance,80),4=>(Blindness,160),5=>(Weakness,180),6=>(Regeneration,160),7=>(JumpBoost,120),8=>(Poison,240),9=>(Wither,160),10=>(Blindness,220),11=>(Nausea,140),_=>return None };
    Some((effect.id() as u8,ticks))
}
pub fn lightning(mob:&mut Mob, bolt:i32) {
    if mob.kind==Kind::Mooshroom && mob.land.lightning!=Some(bolt) { mob.land.brown= !mob.land.brown; mob.land.lightning=Some(bolt); }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Flat;
    impl View for Flat {
        fn block(&self,_:i32,y:i32,_:i32)->i32 { blocks::state_by_name(if y==0 { "stone" } else { "air" }).unwrap() }
        fn loaded(&self,_:i32,_:i32)->bool { true }
    }
    fn mob(kind:Kind)->Mob { Mob::new(42,kind,(0.5,1.0,0.5)) }
    #[test]
    fn every_land_kind_round_trips_without_eating_the_next_record() {
        let mut rng=Rng::new(1);
        for k in Kind::ALL.into_iter().filter(|k| animal(*k)||*k==Kind::WanderingTrader) {
            let mut m=mob(k); initialize(&mut m,&mut rng); m.land.brown=true; m.land.trusting=true; m.land.flower=7;
            m.land.tamed=true; m.land.chest=true; m.land.strength=5; m.land.cargo=vec![None;15];
            let d=crate::stackdata::Data { damage:7,..Default::default() };
            m.land.cargo[14]=Some(Stack::new(blocks::item_named("iron_pickaxe").unwrap(),1).with_extras(d));
            let mut bytes=Vec::new(); m.write(&mut bytes); mob(Kind::Cow).write(&mut bytes);
            let mut bytes=bytes.as_slice(); let restored=Mob::read(&mut bytes).unwrap();
            assert_eq!(restored.kind,k); assert_eq!(restored.uuid,m.uuid); assert_eq!(restored.land.cargo,m.land.cargo);
            assert_eq!(restored.land.flower,7); assert!(restored.land.brown);
            assert_eq!(Mob::read(&mut bytes).unwrap().kind,Kind::Cow); assert!(bytes.is_empty());
        }
    }
    #[test]
    fn trader_llama_does_not_steal_back_a_players_lead() {
        let trader=Mob::new(1,Kind::WanderingTrader,(0.0,1.0,0.0));
        let mut llama=Mob::new(2,Kind::TraderLlama,(1.0,1.0,0.0));
        llama.land.trader=Some(1); llama.leash=Some(super::super::leash::Holder::Player { id:100,uuid:[100;16] });
        let mut mobs=vec![trader,llama]; herd(&mut mobs,&[],&Flat);
        assert_eq!(mobs[1].leash.unwrap().id(),100); assert_eq!(mobs[1].land.trader,None);
    }
    #[test]
    fn trader_llama_retaliates_even_when_the_trader_dies_in_one_hit() {
        let mut trader=Mob::new(1,Kind::WanderingTrader,(0.0,1.0,0.0)); trader.land.target=Some(100); trader.vitals.dead=true;
        let mut llama=Mob::new(2,Kind::TraderLlama,(1.0,1.0,0.0)); llama.land.despawn=42;
        llama.leash=Some(super::super::leash::Holder::Player { id:1,uuid:trader.uuid });
        let mut mobs=vec![trader,llama]; herd(&mut mobs,&[],&Flat);
        assert_eq!(mobs[1].land.target,Some(100)); assert_eq!(mobs[1].land.despawn,42);
        super::super::leash::step_all(&mut mobs,&[]); assert!(mobs[1].leash.is_none());
    }
    #[test]
    fn llama_spits_at_a_visible_attacker_then_cools_down() {
        let mut llama=mob(Kind::Llama); llama.land.target=Some(100);
        let mates=[ai::Mate { id:100,kind:Kind::Zombie,x:6.5,y:1.0,z:0.5,love:0,baby:false,ready:false,tamed:false }];
        let ctx=Ctx { players:&[],mates:&mates,difficulty:2,sky:15,sun_up:true };
        let mut rng=Rng::new(10); let plan=think(&mut llama,&Flat,&ctx,&mut rng);
        assert!(plan.spit.is_some()); assert_eq!(llama.land.attack_wait,40);
        llama.land.target=Some(100); assert!(think(&mut llama,&Flat,&ctx,&mut rng).spit.is_none());
    }
    #[test]
    fn first_spit_comes_after_the_ranged_attack_countdown() {
        // Чёрный ящик, llama-spit: удар — плевок через 41 такт, не сразу.
        let mut llama=mob(Kind::Llama); on_hurt(&mut llama,100);
        assert_eq!(llama.land.attack_wait,42); assert_eq!(llama.land.target,Some(100));
    }
    #[test]
    fn ram_breaks_a_horn_and_preserves_its_instrument() {
        struct Wall;
        impl View for Wall {
            fn block(&self,_:i32,y:i32,z:i32)->i32 { blocks::state_by_name(if y==0 || z==1 { "stone" } else { "air" }).unwrap() }
            fn loaded(&self,_:i32,_:i32)->bool { true }
        }
        let mut goat=mob(Kind::Goat); goat.land.horn=7; goat.land.ram_ticks=20; goat.land.ram_at=Some((0.5,1.0,8.5));
        let ctx=Ctx { players:&[],mates:&[],difficulty:2,sky:15,sun_up:true };
        think(&mut goat,&Wall,&ctx,&mut Rng::new(12));
        assert_eq!(goat.land.horns,[false,true]); assert_eq!(goat.land.ram_ticks,0);
        assert!(goat.events.iter().any(|e| matches!(e,Event::Loot(stacks) if stacks.iter().any(|s| s.extras().instrument==Some(7)))));
    }
    #[test]
    fn trader_potion_components_and_stock_survive_reload() {
        let mut trader=mob(Kind::WanderingTrader); let mut rng=Rng::new(12);
        trader.villager.offers=super::super::trader::offers(&mut rng); trader.villager.offers[0].uses=1;
        let d=crate::stackdata::Data { potion:Some(7),..Default::default() };
        trader.villager.offers[2].sell=Stack::new(blocks::item_named("potion").unwrap(),1).with_extras(d);
        let mut bytes=Vec::new(); trader.write(&mut bytes); let restored=Mob::read(&mut bytes.as_slice()).unwrap();
        assert_eq!(restored.villager.offers[0].uses,1); assert_eq!(restored.villager.offers[2].sell.extras().potion,Some(7));
    }
    #[test]
    fn completed_baby_sneeze_can_drop_a_slime_ball() {
        let mut found=false;
        for seed in 1..20000 {
            let mut panda=mob(Kind::Panda); panda.age= -100; panda.land.sneeze=20;
            tick(&mut panda,&mut Rng::new(seed));
            assert_eq!(panda.land.sneeze,0);
            if panda.events.iter().any(|e| matches!(e,Event::Loot(stacks) if stacks.iter().any(|s| blocks::item_name(s.item)==Some("slime_ball")))) { found=true; break; }
        }
        assert!(found);
    }
    #[test]
    fn panda_picks_one_food_and_leaves_other_items() {
        let mut items=crate::items::Items::new(); items.add_mob(mob(Kind::Panda));
        let bamboo=blocks::item_named("bamboo").unwrap(); let stone=blocks::item_named("stone").unwrap();
        items.drop_item(100,Stack::new(bamboo,3),(0.5,1.0,0.5),(0.0,0.0,0.0),0);
        items.drop_item(101,Stack::new(stone,2),(0.5,1.0,0.5),(0.0,0.0,0.0),0);
        items.note_tick_start();
        pickup_food(&mut items); pickup_food(&mut items);
        assert_eq!(items.mobs()[0].land.held,Some(Stack::new(bamboo,1)));
        assert_eq!(items.items().iter().find(|d| d.stack.item==bamboo).unwrap().stack.count,2);
        assert_eq!(items.items().iter().find(|d| d.stack.item==stone).unwrap().stack.count,2);
        for _ in 0..32 { tick(&mut items.mobs_mut()[0],&mut Rng::new(5)); }
        assert!(items.mobs()[0].land.held.is_none());
    }
    #[test]
    fn incomplete_land_record_is_rejected() {
        let mut data=Vec::new(); mob(Kind::Goat).write(&mut data);
        for end in 0..data.len() { assert!(Mob::read(&mut &data[..end]).is_none()); }
    }
    #[test]
    fn recessive_genes_require_both_copies_and_weak_health_is_ten() {
        let mut p=mob(Kind::Panda);
        for main in 0..7 { for hidden in 0..7 {
            p.land.main_gene=main; p.land.hidden_gene=hidden; health(&mut p);
            let expected=if matches!(main,4|5)&&hidden!=main { 0 } else { main };
            assert_eq!(p.land.personality(),expected); assert_eq!(p.vitals.max_health,if expected==5 { 10.0 } else { 20.0 });
        } }
    }
    #[test]
    fn caravan_has_nine_followers_and_breaks_without_a_lead() {
        let mut mobs: Vec<_>=(0..12).map(|i| Mob::new(i,Kind::Llama,(f64::from(i),1.0,0.5))).collect();
        mobs[0].leash=Some(super::super::leash::Holder::Player { id:100,uuid:[1;16] });
        herd(&mut mobs,&[],&Flat);
        assert_eq!(mobs.iter().filter(|m| m.land.caravan.is_some()).count(),9);
        for m in &mobs { assert_ne!(m.land.caravan,Some(m.entity_id)); }
        mobs[0].leash=None; herd(&mut mobs,&[],&Flat); assert!(mobs.iter().all(|m| m.land.caravan.is_none()));
    }
    #[test]
    fn trader_timer_pauses_during_trade_even_without_ai() {
        let mut m=mob(Kind::WanderingTrader); m.no_ai=true; m.land.despawn=2; m.villager.trading=Some(7);
        let mut rng=Rng::new(4); tick(&mut m,&mut rng); assert_eq!(m.land.despawn,2);
        m.villager.trading=None; tick(&mut m,&mut rng); assert!(!m.h.despawn); tick(&mut m,&mut rng); assert!(m.h.despawn);
        let mut summoned=mob(Kind::WanderingTrader); tick(&mut summoned,&mut rng); assert!(!summoned.h.despawn);
    }
    #[test]
    fn camel_dash_requires_saddle_ground_and_cooldown() {
        let mut m=mob(Kind::Camel); assert!(!dash(&mut m,100)); m.land.saddle=true; m.on_ground=true;
        assert!(dash(&mut m,100)); assert_eq!(m.land.dash,55); assert!(!dash(&mut m,100));
        for _ in 0..55 { tick(&mut m,&mut Rng::new(1)); }
        assert!(!dash(&mut m,100)); m.on_ground=true; assert!(dash(&mut m,100));
    }
    #[test]
    fn nbt_parses_genes_horns_and_ram_memory() {
        let mut m=mob(Kind::Goat);
        apply_data(&mut m,"{IsScreamingGoat:1b,HasLeftHorn:0b,Brain:{memories:{\"minecraft:ram_cooldown_ticks\":{value:1}}}}");
        assert!(m.land.screaming); assert_eq!(m.land.horns,[false,true]); assert_eq!(m.land.ram_wait,1);
        let mut p=mob(Kind::Panda); apply_data(&mut p,"{MainGene:weak,HiddenGene:weak}"); assert_eq!(p.vitals.max_health,10.0);
    }
    #[test]
    fn horns_only_break_on_the_documented_blocks() {
        for name in ["oak_log","stone","packed_ice","coal_ore","copper_ore","emerald_ore","iron_ore"] { assert!(horn_block(name)); }
        for name in ["dirt","diamond_ore","gold_ore","ice","oak_planks","air"] { assert!(!horn_block(name)); }
    }
    #[test]
    fn lightning_changes_mooshroom_once_per_bolt() {
        let mut m=mob(Kind::Mooshroom); lightning(&mut m,4); assert!(m.land.brown); lightning(&mut m,4); assert!(m.land.brown);
        lightning(&mut m,5); assert!(!m.land.brown);
    }
    #[test]
    fn gene_inheritance_and_strength_stay_in_range() {
        let mut rng=Rng::new(999); let mut a=mob(Kind::Llama); let mut b=mob(Kind::Llama); a.land.strength=5; b.land.strength=1;
        for _ in 0..500 { let mut c=mob(Kind::Llama); inherit(&mut c,&a,&b,&mut rng); assert!((1..=5).contains(&c.land.strength)); assert!(!c.land.tamed); }
    }
    #[test]
    fn habitats_exclude_camel_and_trader_from_natural_animal_spawns() {
        for biome in ["jungle","bamboo_jungle","frozen_peaks","snowy_slopes","mushroom_fields","savanna"] {
            assert!(super::super::spawn::land_table(biome).iter().all(|(k,_,_,_)| !matches!(k,Kind::Camel|Kind::TraderLlama|Kind::WanderingTrader)));
        }
        assert_eq!(super::super::spawn::land_table("frozen_peaks")[0].0,Kind::Goat);
    }
    #[test]
    fn stew_effects_and_horns_keep_components_in_storage_and_wire() {
        let data=crate::stackdata::Data { stew:vec![stew(flower("cornflower").unwrap()).unwrap()],instrument:Some(7),..Default::default() };
        assert_eq!(crate::stackdata::from_bytes(&crate::stackdata::to_bytes(&data)),Some(data.clone()));
        assert_eq!(crate::stackdata::encode_components(&data).0,2);
    }
}
