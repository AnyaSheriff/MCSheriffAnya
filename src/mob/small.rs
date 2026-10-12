// Броненосец, нюхач, попугай, летучая мышь и тихоня.
// Правила: minecraft.wiki/w/{Armadillo,Sniffer,Parrot,Bat,Allay}.
// [проверить] Времена анимаций, выбор целей и траектории ещё не сверены
// чёрным ящиком. Номера свойств — minecraft-data 26.1, entities/protocol.json.
use super::{ai::{self, Ctx, Plan}, tame, Event, Kind, Mob, Rng, View};
use crate::{blocks, inventory::Stack};
use serde::{Deserialize, Serialize};

pub const DUPLICATION_COOLDOWN: i32 = 6000;
pub const DIG_COOLDOWN: i32 = 9600;
pub const SNIFFLET_AGE: i32 = -48000;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct State {
    #[serde(default)]
    pub saved_uuid: Option<[u8;16]>,
    // 0 — открыт, 1 — сворачивается, 2 — свёрнут, 3 — разворачивается.
    pub roll: u8,
    pub roll_ticks: i32,
    pub threat: i32,
    // Датчик угрозы броненосца: раз в 20 тактов, фаза случайна (замер чёрного ящика).
    #[serde(default)]
    pub sense: i32,
    // Нюхач: после подъёма из земли одна пауза и «радость» (замер sniffer-dig).
    #[serde(default)]
    pub after_dig: bool,
    // «Silent» при сохранении (сход с плеча возвращает тихого попугая тихим).
    #[serde(default)]
    pub silent: bool,
    // Летучая мышь после пробуждения начинает лететь со следующего такта.
    #[serde(skip)]
    pub settle: i32,
    pub scute: i32,
    // Нюхач: idling, feeling_happy, scenting, sniffing, searching, digging, rising.
    pub sniff: u8,
    pub sniff_ticks: i32,
    pub dig_cooldown: i32,
    #[serde(default)]
    pub dig_target: Option<(i32,i32,i32)>,
    pub drop_seed_at: i32,
    pub explored: Vec<(i32, i32, i32)>,
    pub resting: bool,
    pub variant: u8,
    pub dancing: bool,
    pub duplicate: i32,
    pub note: Option<(i32, i32, i32)>,
    pub note_ticks: i32,
    #[serde(default)]
    pub pending_note: Option<((i32,i32,i32),i32)>,
    pub pickup_wait: i32,
    #[serde(skip)]
    pub held: Option<Stack>,
    #[serde(skip)]
    pub cargo: Option<Stack>,
    #[serde(skip)]
    pub goal: Option<(f64, f64, f64)>,
    #[serde(skip)]
    pub collecting: bool,
    #[serde(skip)]
    pub throwing: bool,
}

impl State {
    pub fn new(kind: Kind, seed: i32) -> Self {
        let mut rng = Rng::new(seed as u64 ^ 0xA11A7);
        Self {
            saved_uuid: None,
            roll: 0, roll_ticks: 0, threat: 0, sense: 2 + rng.below(20) as i32, after_dig: false, silent: false, settle: 0, scute: 6000 + rng.below(6001) as i32,
            sniff: 0, sniff_ticks: 5 + rng.below(80) as i32, dig_cooldown: 0, dig_target: None,
            drop_seed_at: 0, explored: Vec::new(), resting: kind == Kind::Bat,
            variant: rng.below(5) as u8, dancing: false, duplicate: 0,
            note: None, note_ticks: 0, pending_note: None, pickup_wait: 0, held: None, cargo: None, goal: None, collecting: false, throwing: false,
        }
    }
}

pub fn kind(k: Kind) -> bool {
    matches!(k, Kind::Armadillo | Kind::Sniffer | Kind::Parrot | Kind::Bat | Kind::Allay)
}
pub fn flying(k: Kind) -> bool { matches!(k, Kind::Bat | Kind::Parrot | Kind::Allay) }
pub fn drop(mob: &mut Mob, name: &str) {
    if let Some(id) = blocks::item_named(name) { mob.events.push(Event::Loot(vec![Stack::new(id, 1)])); }
}
fn sound(mob: &mut Mob, name: &'static str) {
    mob.events.push(Event::Sound { name, volume: 1.0, pitch: 1.0 });
}

pub fn tick(mob: &mut Mob, rng: &mut Rng) {
    if mob.kind == Kind::Armadillo && !mob.is_baby() {
        mob.small.scute -= 1;
        if mob.small.scute <= 0 {
            drop(mob, "armadillo_scute");
            sound(mob, "minecraft:entity.armadillo.scute_drop");
            mob.small.scute = 6000 + rng.below(6001) as i32;
        }
    }
    if mob.kind == Kind::Allay {
        mob.small.duplicate = (mob.small.duplicate - 1).max(0);
        mob.small.pickup_wait = (mob.small.pickup_wait - 1).max(0);
        mob.small.note_ticks = (mob.small.note_ticks - 1).max(0);
        if mob.small.note_ticks == 0 { mob.small.note = None; }
        if mob.ticks.is_multiple_of(20) {
            mob.vitals.health = (mob.vitals.health + 2.0).min(mob.vitals.max_health);
        }
    }
}

pub fn think(mob: &mut Mob, view: &dyn View, ctx: &Ctx, rng: &mut Rng) -> Plan {
    if flying(mob.kind) && mob.brain.panic_tick() {
        mob.small.resting = false;
        mob.tame.sitting = false;
        mob.small.dancing = false;
        let (from_x,from_z) = mob.fright.unwrap_or((mob.x-1.0,mob.z));
        let dx=mob.x-from_x; let dz=mob.z-from_z;
        let length=(dx*dx+dz*dz).sqrt().max(0.1);
        // [проверить] Уход по воздуху: используем общие столкновения.
        mob.small.goal=Some((mob.x+dx/length*6.0,mob.y+1.0,mob.z+dz/length*6.0));
        return Plan::default();
    }
    match mob.kind {
        Kind::Armadillo => armadillo(mob, view, ctx, rng),
        Kind::Sniffer => sniffer(mob, view, ctx, rng),
        Kind::Bat => bat(mob, view, ctx, rng),
        Kind::Parrot => parrot(mob, view, ctx, rng),
        Kind::Allay => {
            ai::voice(mob, rng);
            if !mob.small.dancing && !mob.small.collecting { wander(mob, view, rng); }
            Plan::default()
        }
        _ => Plan::default(),
    }
}

fn armadillo(mob: &mut Mob, view: &dyn View, ctx: &Ctx, rng: &mut Rng) -> Plan {
    let (x, y, z) = mob.cell();
    let wet = super::is_water(view.block(x, y, z)) || super::is_lava(view.block(x, y, z));
    let undead = ctx.mates.iter().any(|m| matches!(m.kind,
        Kind::Zombie | Kind::ZombieVillager | Kind::Drowned | Kind::Husk | Kind::Skeleton |
        Kind::WitherSkeleton | Kind::ZombifiedPiglin | Kind::Phantom)
        && (m.x-mob.x).abs() <= 7.0 && (m.y-mob.y).abs() <= 2.0 && (m.z-mob.z).abs() <= 7.0);
    let rushing = ctx.players.iter().any(|p| !p.spectator && (p.sprinting || p.riding)
        && (p.x-mob.x).abs() <= 7.0 && (p.y-mob.y).abs() <= 2.0 && (p.z-mob.z).abs() <= 7.0);
    // Замер чёрного ящика (armadillo-roll): сворачивается через 4–22 такта после
    // появления угрозы, разворачивается через 49–66 после убийства угрозы — это
    // датчик раз в 20 тактов со случайной фазой; нашёл угрозу — сворачивается сразу,
    // не нашёл — ещё ~46 тактов держится свёрнутым.
    mob.small.sense -= 1;
    if mob.small.sense <= 0 {
        mob.small.sense = 20;
        if undead || rushing { mob.small.threat = mob.small.threat.max(100); }
        else { mob.small.threat = mob.small.threat.min(46); }
    }
    let threatened = mob.small.threat > 0;
    mob.small.threat = (mob.small.threat - 1).max(0);
    if wet || (!mob.on_ground && mob.small.roll == 0) || mob.brain.is_panicking() {
        mob.small.roll = 0;
    } else {
        roll_step(mob, threatened);
    }
    if mob.small.roll != 0 {
        mob.brain.stop();
        mob.vx = 0.0; mob.vz = 0.0;
        return Plan::default();
    }
    ai::think(mob, view, ctx, rng)
}

fn roll_step(mob: &mut Mob, threatened: bool) {
    mob.small.roll_ticks += 1;
    let next = match mob.small.roll {
        0 if threatened => Some((1, "minecraft:entity.armadillo.roll")),
        1 if mob.small.roll_ticks >= 11 => Some((2, "")),
        2 if threatened => { mob.small.roll_ticks = 0; None }
        2 if !threatened => Some((3, "minecraft:entity.armadillo.unroll_start")),
        3 if threatened => Some((2, "minecraft:entity.armadillo.roll")),
        3 if mob.small.roll_ticks >= 30 => Some((0, "minecraft:entity.armadillo.unroll_finish")),
        _ => None,
    };
    if let Some((state, name)) = next {
        mob.small.roll = state;
        mob.small.roll_ticks = 0;
        if !name.is_empty() { sound(mob, name); }
    }
}

pub fn diggable(state: i32) -> bool {
    matches!(blocks::block_at_state(state), Some("grass_block" | "dirt" | "coarse_dirt" |
        "podzol" | "rooted_dirt" | "moss_block" | "mud" | "muddy_mangrove_roots"))
}
fn sniff_state(mob: &mut Mob, state: u8, ticks: i32, name: &'static str) {
    mob.small.sniff = state; mob.small.sniff_ticks = ticks;
    if state == 0 { mob.small.dig_target = None; }
    if !name.is_empty() { sound(mob, name); }
}
fn sniffer(mob: &mut Mob, view: &dyn View, ctx: &Ctx, rng: &mut Rng) -> Plan {
    mob.small.dig_cooldown = (mob.small.dig_cooldown - 1).max(0);
    let tempted = ctx.players.iter().any(|p| !p.spectator && p.holding.contains(&Some("torchflower_seeds"))
        && (p.x-mob.x).powi(2)+(p.y-mob.y).powi(2)+(p.z-mob.z).powi(2) < 100.0);
    let (x, y, z) = mob.cell();
    if mob.is_baby() || mob.love > 0 || mob.brain.is_panicking() || tempted || super::is_water(view.block(x,y,z)) {
        sniff_state(mob, 0, 100, "");
        mob.small.drop_seed_at = 0;
        return ai::think(mob, view, ctx, rng);
    }
    mob.small.sniff_ticks -= 1;
    let at = (x, y-1, z);
    let can_dig = mob.on_ground && diggable(view.block(x,y-1,z)) && !mob.small.explored.contains(&at);
    if mob.small.sniff == 5 && !can_dig {
        sniff_state(mob, 0, 100, ""); mob.small.drop_seed_at = 0;
    }
    if mob.small.sniff == 5 && mob.small.drop_seed_at > 0 && mob.ticks >= mob.small.drop_seed_at as u64 {
        drop(mob, if rng.below(2) == 0 { "torchflower_seeds" } else { "pitcher_pod" });
        mob.small.drop_seed_at = 0;
    }
    // Чёрный ящик sniffer-dig: статус 63 уходит за такт до состояния «копает».
    if mob.small.sniff == 4 && can_dig && mob.small.dig_target == Some((x,y,z)) && mob.small.sniff_ticks > 1 {
        mob.small.sniff_ticks = 1;
        mob.events.push(Event::Status(63));
    }
    if mob.small.sniff_ticks <= 0 {
        // Замер sniffer-dig (пять прогонов оригинала): цепочка «принюхивается» (2) идёт
        // подряд, между состояниями ровно один такт покоя; раз в ~5 решений вместо неё —
        // «нюхает» (3), затем «ищет» (4), «копает» (5, 170–181), «поднимается» (6, 41),
        // пауза, «радуется» (1, 50–71), потом ~54 тактов покоя.
        match mob.small.sniff {
            0 if mob.small.after_dig => {
                mob.small.after_dig = false;
                sniff_state(mob, 1, 50 + rng.below(22) as i32, "minecraft:entity.sniffer.happy");
            }
            0 if mob.small.dig_target.is_some() => {
                sniff_state(mob, 4, 600, "minecraft:entity.sniffer.searching");
            }
            0 if mob.small.dig_cooldown == 0 && rng.below(5) == 0 => sniff_state(mob, 3, 44 + rng.below(35) as i32, "minecraft:entity.sniffer.sniffing"),
            0 => sniff_state(mob, 2, 44 + rng.below(42) as i32, "minecraft:entity.sniffer.scenting"),
            2 => { mob.small.sniff = 0; mob.small.sniff_ticks = 1; }
            3 => {
                // [проверить] Ищем проходимую землю в пределах 10 блоков;
                // точная выборка целей и время поиска ждут чёрного ящика.
                let target = (0..30).find_map(|_| {
                    let tx=x+rng.below(21) as i32-10; let tz=z+rng.below(21) as i32-10;
                    (y-3..=y+3).find_map(|ty| {
                        let at=(tx,ty,tz);
                        if !diggable(view.block(tx,ty-1,tz)) || mob.small.explored.contains(&(tx,ty-1,tz))
                            || !super::path::standable(view,at) { return None; }
                        let path=super::path::find(view,(x,y,z),at,super::path::MAX_NODES);
                        if path.last().copied() == Some(at) || at == (x,y,z) { Some((at,path)) } else { None }
                    })
                });
                mob.small.sniff = 0; mob.small.sniff_ticks = 1;
                if let Some((target,path)) = target {
                    mob.small.dig_target=Some(target);
                    mob.brain.go(path,1.0);
                }
            }
            4 if can_dig => {
                sniff_state(mob, 5, 170 + rng.below(12) as i32, "minecraft:entity.sniffer.digging");
                mob.small.drop_seed_at = mob.ticks as i32 + 119;
            }
            5 => {
                mob.small.explored.push(at);
                if mob.small.explored.len() > 20 { mob.small.explored.remove(0); }
                mob.small.dig_cooldown = DIG_COOLDOWN;
                sniff_state(mob, 6, 41, "minecraft:entity.sniffer.digging_stop");
            }
            6 => { mob.small.sniff = 0; mob.small.sniff_ticks = 1; mob.small.after_dig = true; mob.small.dig_target = None; }
            1 => { mob.small.dig_target = None; sniff_state(mob, 0, 40 + rng.below(30) as i32, ""); }
            _ => sniff_state(mob, 0, 44 + rng.below(40) as i32, ""),
        }
    }
    if matches!(mob.small.sniff, 1 | 2 | 3 | 5 | 6) {
        mob.brain.stop(); mob.vx = 0.0; mob.vz = 0.0;
        Plan::default()
    } else if mob.small.sniff == 4 { ai::walk(mob,Plan::default()) }
    else { ai::think(mob, view, ctx, rng) }
}

fn wander(mob: &mut Mob, view: &dyn View, rng: &mut Rng) {
    // Чёрный ящик parrot-shoulder: попугай у хозяина стоит; цель назначается
    // редко (раз в ~120 тактов, как у WaterAvoidingRandomFlying), дошёл — снова ждёт.
    if mob.small.goal.is_some_and(|g| distance((mob.x,mob.y,mob.z),g) < 0.25) || mob.wall { mob.small.goal = None; }
    if mob.small.goal.is_none() && rng.below(120) == 0 {
        let at = (mob.x + rng.below(13) as f64 - 6.0, mob.y + rng.below(7) as f64 - 3.0, mob.z + rng.below(13) as f64 - 6.0);
        if at.1 > -64.0 && at.1 < 319.0 && view.block(at.0.floor() as i32,at.1.floor() as i32,at.2.floor() as i32) == 0 {
            mob.small.goal = Some(at);
        }
    }
}
/// Цель полёта мыши: новая точка рядом, когда старой нет, дошли до неё или раз в ~30 тактов.
/// [проверить] Размах цели (чёрный ящик bat-rest сверяет только скорость полёта).
fn bat_target(mob: &mut Mob, view: &dyn View, rng: &mut Rng) {
    let near = mob.small.goal.is_some_and(|g| distance((mob.x,mob.y,mob.z),g) < 4.0);
    if mob.small.goal.is_none() || near || rng.below(30) == 0 {
        let at = (mob.x + rng.below(7) as f64 - rng.below(7) as f64, mob.y + rng.below(6) as f64 - 2.0, mob.z + rng.below(7) as f64 - rng.below(7) as f64);
        if view.block(at.0.floor() as i32, at.1.floor() as i32, at.2.floor() as i32) == 0 {
            mob.small.goal = Some(at);
        }
    }
}
fn bat(mob: &mut Mob, view: &dyn View, ctx: &Ctx, rng: &mut Rng) -> Plan {
    let (x,y,z) = mob.cell();
    // Вики, Bat: «cannot hang on transparent or non-solid blocks».
    let roof=view.block(x,y+1,z);
    let ceiling = blocks::full_cube(roof) && crate::world::light::filter(roof) == 15;
    let near = ctx.players.iter().any(|p| !p.spectator && distance((p.x,p.y,p.z),(mob.x,mob.y,mob.z)) < 16.0);
    if mob.small.resting && (!ceiling || near) {
        mob.small.resting = false;
        mob.small.settle = 1;
        // Вики, Bat, Sounds: «0.05», «0.8–1.2».
        mob.events.push(Event::Sound { name:"minecraft:entity.bat.takeoff",volume:0.05,pitch:0.8+rng.unit() as f32*0.4 });
    } else if !mob.small.resting && ceiling && !near && rng.below(100) == 0 {
        mob.small.resting = true;
    }
    if mob.small.resting {
        mob.y = y as f64 + 1.0 - mob.kind.height();
        mob.small.goal = None;
    } else { bat_target(mob, view, rng); }
    if !mob.small.resting || rng.below(4) == 0 { ai::voice(mob, rng); }
    Plan::default()
}
fn parrot(mob: &mut Mob, view: &dyn View, ctx: &Ctx, rng: &mut Rng) -> Plan {
    mob.tame.sitting = mob.tame.ordered_sit && mob.tame.tamed();
    if mob.tame.sitting || mob.small.dancing {
        mob.small.goal = None;
    } else if let Some(owner) = ctx.players.iter().find(|p| p.id == mob.tame.owner_id && !p.spectator) {
        let d2 = distance((mob.x,mob.y,mob.z),(owner.x,owner.y,owner.z));
        let far = d2 >= tame::TELEPORT.powi(2);
        if far && mob.leash.is_none() {
            tame::teleport_near(mob,view,(owner.x,owner.y,owner.z),rng);
        }
        // Вики, Parrot: идёт к хозяину с 5 блоков, останавливается рядом
        // (чёрный ящик parrot-shoulder: у хозяина в 2 блоках попугай стоит).
        if d2 > 25.0 { mob.small.goal = Some((owner.x, owner.y + 0.5, owner.z)); }
        else if d2 < 1.0 { mob.small.goal = None; }
    } else { wander(mob, view, rng); }
    if rng.below(400) == 0 {
        let nearby: Vec<_> = ctx.mates.iter().filter(|m| imitation(m.kind).is_some() && distance((m.x,m.y,m.z),(mob.x,mob.y,mob.z)) <= 400.0).collect();
        if !nearby.is_empty() && let Some(name) = imitation(nearby[rng.below(nearby.len() as u32) as usize].kind) {
            mob.events.push(Event::Sound { name, volume: 0.7, pitch: 0.8 + rng.unit() as f32 * 0.4 });
        }
    }
    ai::voice(mob, rng);
    Plan::default()
}
fn imitation(kind: Kind) -> Option<&'static str> {
    Some(match kind {
        Kind::Zombie => "minecraft:entity.parrot.imitate.zombie",
        Kind::ZombieVillager => "minecraft:entity.parrot.imitate.zombie_villager",
        Kind::Skeleton => "minecraft:entity.parrot.imitate.skeleton",
        Kind::WitherSkeleton => "minecraft:entity.parrot.imitate.wither_skeleton",
        Kind::Creeper => "minecraft:entity.parrot.imitate.creeper",
        Kind::Spider | Kind::CaveSpider => "minecraft:entity.parrot.imitate.spider",
        Kind::Drowned => "minecraft:entity.parrot.imitate.drowned",
        Kind::Husk => "minecraft:entity.parrot.imitate.husk",
        Kind::Blaze => "minecraft:entity.parrot.imitate.blaze",
        Kind::Ghast => "minecraft:entity.parrot.imitate.ghast",
        Kind::Guardian => "minecraft:entity.parrot.imitate.guardian",
        Kind::ElderGuardian => "minecraft:entity.parrot.imitate.elder_guardian",
        Kind::Witch => "minecraft:entity.parrot.imitate.witch",
        Kind::Phantom => "minecraft:entity.parrot.imitate.phantom",
        Kind::Slime => "minecraft:entity.parrot.imitate.slime",
        Kind::MagmaCube => "minecraft:entity.parrot.imitate.magma_cube",
        Kind::Silverfish => "minecraft:entity.parrot.imitate.silverfish",
        Kind::Shulker => "minecraft:entity.parrot.imitate.shulker",
        Kind::EnderDragon => "minecraft:entity.parrot.imitate.ender_dragon",
        Kind::Hoglin => "minecraft:entity.parrot.imitate.hoglin",
        Kind::Piglin => "minecraft:entity.parrot.imitate.piglin",
        Kind::PiglinBrute => "minecraft:entity.parrot.imitate.piglin_brute",
        _ => return None,
    })
}

/// Полёт через общий коллидер. [проверить] Ускорение и торможение.
pub fn fly(mob: &mut Mob, view: &dyn View) {
    if (mob.kind == Kind::Bat && mob.small.resting) || (mob.kind == Kind::Parrot && mob.tame.sitting) || mob.small.dancing {
        mob.vx = 0.0; mob.vy = 0.0; mob.vz = 0.0;
        if mob.kind == Kind::Parrot { mob.physics(view, 0.0, false); }
        return;
    }
    if mob.kind == Kind::Bat {
        if mob.small.settle > 0 { mob.small.settle -= 1; return; }
        // Замер bat-rest: по горизонтали v += (±0,5 − v)·0,1, по вертикали
        // v += (±0,07 − v)·0,4, затем сопротивление ·0,91 — тот же предел 0,25 и 0,059 за такт.
        if let Some((x,y,z)) = mob.small.goal {
            let sign = |d: f64| if d > 0.0 { 1.0 } else if d < 0.0 { -1.0 } else { 0.0 };
            mob.vx += (sign(x-mob.x)*0.5-mob.vx)*0.1;
            mob.vy += (sign(y-mob.y)*0.07-mob.vy)*0.4;
            mob.vz += (sign(z-mob.z)*0.5-mob.vz)*0.1;
            mob.face((-mob.vx.atan2(mob.vz).to_degrees()) as f32);
        }
        mob.step(view,mob.vx,mob.vy,mob.vz);
        mob.vx *= 0.91; mob.vy *= 0.91; mob.vz *= 0.91;
        mob.vitals.fall = 0.0;
        return;
    }
    if let Some((x,y,z)) = mob.small.goal {
        let (dx,dy,dz) = (x-mob.x,y-mob.y,z-mob.z);
        let d = (dx*dx+dy*dy+dz*dz).sqrt().max(1.0);
        let speed = if mob.kind == Kind::Allay { 0.3 } else { 0.2 };
        mob.vx += (dx/d*speed-mob.vx)*0.1;
        mob.vy += (dy/d*speed-mob.vy)*0.1;
        mob.vz += (dz/d*speed-mob.vz)*0.1;
        mob.face((-dx.atan2(dz).to_degrees()) as f32);
    }
    mob.step(view,mob.vx,mob.vy,mob.vz);
    mob.vx *= 0.91; mob.vy *= 0.91; mob.vz *= 0.91;
    mob.vitals.fall = 0.0;
}
pub fn distance(a: (f64,f64,f64), b: (f64,f64,f64)) -> f64 {
    (a.0-b.0).powi(2)+(a.1-b.1).powi(2)+(a.2-b.2).powi(2)
}
pub fn seeds(name: &str) -> bool {
    matches!(name,"wheat_seeds"|"melon_seeds"|"pumpkin_seeds"|"beetroot_seeds"|"torchflower_seeds")
}
pub fn brush(mob: &mut Mob) -> bool {
    if mob.kind != Kind::Armadillo || mob.is_baby() || mob.is_dead() { return false; }
    drop(mob,"armadillo_scute"); sound(mob,"minecraft:entity.armadillo.brush");
    true
}

pub fn duplicate(mob: &mut Mob, id: i32) -> Option<Mob> {
    if mob.kind != Kind::Allay || mob.is_dead() || !mob.small.dancing || mob.small.duplicate != 0 { return None; }
    mob.small.duplicate = DUPLICATION_COOLDOWN;
    mob.events.push(Event::Hearts);
    mob.events.push(Event::Sound { name:"minecraft:block.amethyst_block.chime", volume:2.0, pitch:1.0 });
    let mut copy=Mob::new(id,Kind::Allay,(mob.x,mob.y,mob.z));
    copy.small.duplicate=DUPLICATION_COOLDOWN;
    Some(copy)
}

pub fn parrot_interact(mob: &mut Mob, who: ([u8;16],i32), held: Option<&str>, rng: &mut Rng) -> tame::Used {
    if mob.kind != Kind::Parrot || mob.is_dead() { return tame::Used::Pass; }
    if held == Some("cookie") {
        mob.hurt(f32::MAX, crate::survival::Source::GenericKill, None);
        return tame::Used::Spent;
    }
    if !mob.tame.tamed() && held.is_some_and(seeds) {
        if rng.below(10) == 0 {
            tame::tame(mob, who);
            mob.tame.ordered_sit = false;
            mob.events.push(Event::Status(7));
        } else { mob.events.push(Event::Status(6)); }
        return tame::Used::Spent;
    }
    if mob.tame.owner == Some(who.0) {
        mob.tame.ordered_sit = !mob.tame.ordered_sit;
        mob.tame.sitting = mob.tame.ordered_sit;
        return tame::Used::Kept;
    }
    tame::Used::Pass
}

pub fn apply_data(mob: &mut Mob, data: &str) {
    if !kind(mob.kind) { return; }
    let Some(value) = crate::snbt::parse(data) else { return; };
    let number = |key| value.get(key).and_then(|v|v.as_f64()).map(|n|n as i32);
    match mob.kind {
        Kind::Parrot => {
            if let Some(n)=number("Variant") { mob.small.variant=n.clamp(0,4) as u8; }
        }
        Kind::Bat => {
            if let Some(n)=number("BatFlags") { mob.small.resting=n & 1 != 0; }
        }
        Kind::Armadillo => {
            if let Some(n)=number("scute_time") { mob.small.scute=n.max(0); }
            if let Some(state)=value.get("state").and_then(|v|v.as_str()) {
                // Вики, Armadillo, Entity data; [проверить] возраст анимации
                // после загрузки не хранится в vanilla NBT.
                mob.small.roll=match state { "rolling"=>1,"scared"=>2,"unrolling"=>3,_=>0 };
                mob.small.roll_ticks=0;
            }
        }
        Kind::Allay => {
            if let Some(n)=number("DuplicationCooldown") { mob.small.duplicate=n.max(0); }
        }
        _ => {}
    }
}

// Хвост нового вида: JSON состояния и обычные записи стопок с компонентами.
// Старых видов этот формат не касается.
pub fn write(mob: &Mob, out: &mut Vec<u8>) {
    let mut state = mob.small.clone();
    state.saved_uuid = Some(mob.uuid);
    state.silent = mob.silent;
    // Возраст сущности при загрузке начинается заново: храним оставшееся
    // время до семени, иначе перезапуск отложит выпадение на прежний возраст.
    if state.drop_seed_at > 0 { state.drop_seed_at = (i64::from(state.drop_seed_at)-mob.ticks as i64).max(1) as i32; }
    let json = serde_json::to_vec(&state).expect("состояние малого моба");
    out.extend_from_slice(&(json.len() as u32).to_be_bytes()); out.extend(json);
    tame::write(&mob.tame, out);
    for stack in [mob.small.held, mob.small.cargo] {
        out.push(u8::from(stack.is_some()));
        if let Some(stack) = stack {
            out.extend_from_slice(&stack.stored_item().to_be_bytes()); out.extend_from_slice(&stack.count.to_be_bytes()); stack.write_extras(out);
        }
    }
}
pub fn read(bytes: &mut &[u8]) -> Option<(State,tame::State)> {
    fn take<'a>(bytes: &mut &'a [u8], n: usize) -> Option<&'a [u8]> {
        let (a,b) = bytes.split_at_checked(n)?; *bytes=b; Some(a)
    }
    let n = u32::from_be_bytes(take(bytes,4)?.try_into().ok()?) as usize;
    let mut state: State = serde_json::from_slice(take(bytes,n)?).ok()?;
    let tame = tame::read(bytes)?;
    for slot in [&mut state.held, &mut state.cargo] {
        if take(bytes,1)?[0] != 0 {
            let stored = i32::from_be_bytes(take(bytes,4)?.try_into().ok()?);
            let count = i32::from_be_bytes(take(bytes,4)?.try_into().ok()?);
            let extra = Stack::read_extras(stored, |n| take(bytes,n))?;
            let mut stack = Stack::from_stored(stored,count);
            stack.data = extra.and_then(crate::stackdata::intern);
            *slot = Some(stack);
        }
    }
    if state.variant > 4 || state.roll > 3 || state.sniff > 6 || state.explored.len() > 20
        || state.scute < 0 || state.duplicate < 0 || state.dig_cooldown < 0 || state.note_ticks < 0 || state.pickup_wait < 0
        || [state.held,state.cargo].into_iter().flatten().any(|stack| stack.count <= 0 || stack.count > stack.limit()) { return None; }
    Some((state,tame))
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Ground;
    impl View for Ground {
        fn block(&self,_x:i32,y:i32,_z:i32)->i32 { if y == 0 { blocks::state_by_name("moss_block").unwrap() } else { 0 } }
        fn loaded(&self,_x:i32,_z:i32)->bool { true }
    }
    fn context()->Ctx<'static> { Ctx { players:&[],mates:&[],difficulty:2,sky:15,sun_up:false } }
    fn mob(kind:Kind)->Mob { let mut m=Mob::new(1,kind,(0.5,1.0,0.5)); m.on_ground=true; m }

    #[test]
    fn brushing_has_no_cooldown_but_babies_do_not_drop_scutes() {
        let mut m=mob(Kind::Armadillo); m.small.roll=2;
        let timer=m.small.scute;
        for _ in 0..4 { assert!(brush(&mut m)); }
        assert_eq!(m.events.iter().filter(|e|matches!(e,Event::Loot(_))).count(),4);
        assert_eq!(m.small.scute,timer);
        m.age = -1; assert!(!brush(&mut m));
    }
    #[test]
    fn duplication_requires_dancing_and_cools_both_allays_without_copying_items() {
        let mut m=mob(Kind::Allay);
        m.small.held=Some(Stack::new(blocks::item_named("diamond").unwrap(),1));
        assert!(duplicate(&mut m,2).is_none());
        m.small.dancing=true;
        let copy=duplicate(&mut m,2).unwrap();
        assert_eq!(m.small.duplicate,DUPLICATION_COOLDOWN); assert_eq!(copy.small.duplicate,DUPLICATION_COOLDOWN);
        assert!(copy.small.held.is_none()); assert!(copy.tame.owner.is_none());
        assert!(m.small.held.is_some()); assert!(duplicate(&mut m,3).is_none());
        for _ in 0..DUPLICATION_COOLDOWN { tick(&mut m,&mut Rng::new(1)); }
        assert!(duplicate(&mut m,4).is_some());
    }
    #[test]
    fn dig_drop_deadline_survives_a_save_mid_animation() {
        let mut m=mob(Kind::Sniffer); m.ticks=5000; m.small.sniff=5; m.small.drop_seed_at=5060;
        let mut bytes=Vec::new(); m.write(&mut bytes);
        let restored=Mob::read(&mut bytes.as_slice()).unwrap();
        assert_eq!(restored.ticks,0); assert_eq!(restored.small.drop_seed_at,60);
    }

    #[test]
    fn new_kinds_have_names_eggs_and_distinct_protocol_ids() {
        let kinds=[Kind::Armadillo,Kind::Sniffer,Kind::Parrot,Kind::Bat,Kind::Allay];
        for k in kinds {
            assert_eq!(Kind::from_name(k.name()),Some(k));
            assert!(blocks::item_named(&format!("{}_spawn_egg",k.name())).is_some());
            assert!(kinds.iter().filter(|other| other.java_type()==k.java_type()).count()==1);
        }
    }
    fn watcher() -> ai::Watcher {
        ai::Watcher { id:2,x:2.0,y:1.0,z:0.5,eye:1.62,holding:[None,None],spectator:false,targetable:true,
            look:(0.0,0.0),pumpkin:false,gold_armor:false,sprinting:false,riding:false }
    }
    #[test]
    fn armadillo_rolls_for_sprinters_and_riders_but_ignores_spectators() {
        for riding in [false,true] {
            let mut m=mob(Kind::Armadillo); let mut player=watcher(); m.small.sense=1;
            player.riding=riding; player.sprinting = !riding;
            armadillo(&mut m,&Ground,&Ctx { players:&[player],..context() },&mut Rng::new(1));
            assert_eq!(m.small.roll,1);
            let mut m=mob(Kind::Armadillo); m.small.sense=1; player.spectator=true;
            armadillo(&mut m,&Ground,&Ctx { players:&[player],..context() },&mut Rng::new(1));
            assert_eq!(m.small.roll,0);
        }
    }
    #[test]
    fn an_attack_alerts_the_armadillo_without_overriding_its_roll_with_panic() {
        let mut m=mob(Kind::Armadillo);
        m.hurt(2.0,crate::survival::Source::PlayerAttack,Some(super::super::Push { dir:(1.0,0.0),strength:0.0,cause:2 }));
        assert!(!m.brain.is_panicking()); assert_eq!(m.small.threat,100);
        armadillo(&mut m,&Ground,&context(),&mut Rng::new(1)); assert_eq!(m.small.roll,1);
    }
    #[test]
    fn allay_is_protected_from_its_item_giver_but_not_other_players() {
        let mut m=mob(Kind::Allay); m.tame.owner_id=2;
        m.small.held=Some(Stack::new(blocks::item_named("diamond").unwrap(),1));
        let push=super::super::Push { dir:(1.0,0.0),strength:0.0,cause:2 };
        assert!(m.hurt(5.0,crate::survival::Source::PlayerAttack,Some(push)).is_none());
        assert_eq!(m.vitals.health,20.0);
        m.hurt(5.0,crate::survival::Source::PlayerAttack,Some(super::super::Push { cause:3,..push }));
        assert_eq!(m.vitals.health,15.0);
        let mut m=mob(Kind::Allay); m.tame.owner_id=2;
        assert!(m.hurt(5.0,crate::survival::Source::PlayerAttack,Some(push)).is_some());
    }
    #[test]
    fn only_requested_biomes_spawn_armadillos_or_parrots_with_correct_groups() {
        struct Grass;
        impl View for Grass {
            fn block(&self,_x:i32,y:i32,_z:i32)->i32 { if y == 0 { blocks::state_by_name("grass_block").unwrap() } else { 0 } }
            fn loaded(&self,_x:i32,_z:i32)->bool { true }
        }
        for (biome,kind,min,max) in [("savanna",Kind::Armadillo,2,3),("badlands",Kind::Armadillo,1,2),("jungle",Kind::Parrot,1,2)] {
            let group=(0..10000).find_map(|seed| {
                let mobs=super::super::spawn::for_new_chunk_in(&Grass,0,0,biome,&mut Rng::new(seed),&mut ||1);
                mobs.first().is_some_and(|m|m.kind==kind).then_some(mobs)
            }).unwrap();
            assert!((min..=max).contains(&group.len()));
        }
        for seed in 0..100 {
            let group=super::super::spawn::for_new_chunk_in(&Grass,0,0,"plains",&mut Rng::new(seed),&mut ||1);
            assert!(group.iter().all(|m| !matches!(m.kind,Kind::Armadillo|Kind::Parrot)));
        }
    }

    #[test]
    fn roll_finishes_and_a_renewed_threat_interrupts_unrolling() {
        let mut m=mob(Kind::Armadillo);
        roll_step(&mut m,true); assert_eq!(m.small.roll,1);
        for _ in 0..11 { roll_step(&mut m,true); }
        assert_eq!(m.small.roll,2);
        for _ in 0..100 { roll_step(&mut m,true); }
        assert_eq!(m.small.roll,2);
        roll_step(&mut m,false); assert_eq!(m.small.roll,3);
        roll_step(&mut m,true); assert_eq!(m.small.roll,2);
        roll_step(&mut m,false);
        for _ in 0..30 { roll_step(&mut m,false); }
        assert_eq!(m.small.roll,0);
    }
    #[test]
    fn armadillo_sensor_runs_every_twenty_ticks_and_holds_the_roll_for_46_more() {
        // Чёрный ящик armadillo-roll: угроза замечается только на тактах датчика (раз в 20).
        let mut m=mob(Kind::Armadillo); m.small.sense=5;
        let husk=Mob::new(9,Kind::Husk,(0.5,1.0,2.5));
        let mates=[ai::Mate { id:9,kind:Kind::Husk,x:husk.x,y:husk.y,z:husk.z,love:0,baby:false,ready:false,tamed:false }];
        let ctx=Ctx { mates:&mates,..context() };
        for _ in 0..4 { armadillo(&mut m,&Ground,&ctx,&mut Rng::new(1)); assert_eq!(m.small.roll,0); }
        armadillo(&mut m,&Ground,&ctx,&mut Rng::new(1));
        assert_eq!(m.small.roll,1);
        // Угроза ушла: на следующем датчике счёт сокращается до 46.
        m.small.sense=1;
        armadillo(&mut m,&Ground,&context(),&mut Rng::new(1));
        assert!(m.small.threat <= 46 && m.small.threat > 0);
    }
    #[test]
    fn bat_flight_reaches_the_measured_top_speed_and_waits_a_tick_after_waking() {
        let mut m=mob(Kind::Bat); m.small.resting=false; m.small.settle=1;
        m.small.goal=Some((100.0,m.y,m.z));
        let x0=m.x;
        fly(&mut m,&Ground); assert_eq!(m.x,x0);
        for _ in 0..200 { fly(&mut m,&Ground); }
        // Предел 0,25 блока за такт по горизонтали (bat-rest: 0,24…0,25).
        assert!((m.vx-0.2514).abs() < 0.005, "{}", m.vx);
    }
    #[test]
    fn a_tamed_parrot_follows_only_beyond_five_blocks() {
        let mut m=mob(Kind::Parrot); m.tame.owner=Some([1;16]); m.tame.owner_id=2; m.x=0.5; m.z=0.5;
        let near=watcher();
        parrot(&mut m,&Ground,&Ctx { players:&[near],..context() },&mut Rng::new(1));
        assert!(m.small.goal.is_none());
        let mut far=watcher(); far.x=9.0;
        parrot(&mut m,&Ground,&Ctx { players:&[far],..context() },&mut Rng::new(1));
        assert!(m.small.goal.is_some());
    }
    #[test]
    fn rolled_shell_reduces_physical_damage_but_not_kill() {
        let mut m=mob(Kind::Armadillo); m.small.roll=2;
        m.hurt(5.0,crate::survival::Source::PlayerAttack,None);
        assert_eq!(m.vitals.health,10.0);
        m.hurt(f32::MAX,crate::survival::Source::GenericKill,None);
        assert!(m.is_dead());
    }
    #[test]
    fn scutes_are_adult_only_and_timer_restarts_in_range() {
        let mut m=mob(Kind::Armadillo); let mut rng=Rng::new(5);
        m.small.scute=1; tick(&mut m,&mut rng);
        assert!(m.events.iter().any(|e| matches!(e,Event::Loot(v) if v[0].item==blocks::item_named("armadillo_scute").unwrap())));
        assert!((6000..=12000).contains(&m.small.scute));
        m.events.clear(); m.age = -100; m.small.scute=1; tick(&mut m,&mut rng);
        assert!(m.events.is_empty());
    }
    #[test]
    fn sniffer_drops_one_seed_then_remembers_ground_and_cools_down() {
        let mut m=mob(Kind::Sniffer); let mut rng=Rng::new(3);
        m.small.sniff=4; m.small.sniff_ticks=1;
        sniffer(&mut m,&Ground,&context(),&mut rng);
        assert_eq!(m.small.sniff,5);
        for _ in 0..220 { m.ticks+=1; sniffer(&mut m,&Ground,&context(),&mut rng); }
        assert_eq!(m.events.iter().filter(|e| matches!(e,Event::Loot(_))).count(),1);
        assert_eq!(m.small.explored,vec![(0,0,0)]);
        assert!(m.small.dig_cooldown>9300);
    }
    #[test]
    fn breeding_sniffers_drops_an_egg_while_armadillos_have_a_baby() {
        for kind in [Kind::Sniffer,Kind::Armadillo] {
            let mut items=crate::items::Items::new();
            for id in 1..=2 {
                let mut m=Mob::new(id,kind,(id as f64,1.0,0.5));
                m.on_ground=true; m.love=super::super::LOVE_TICKS; items.add_mob(m);
            }
            let mut next=10; let mut rng=Rng::new(1);
            for _ in 0..70 {
                super::super::step_all(&mut items,&Ground,&[],super::super::Env { difficulty:2,sky:15,sun_up:false,clock: 0, tick: 0 },&mut rng,&mut || {next+=1; next});
            }
            if kind == Kind::Sniffer {
                assert_eq!(items.mobs().len(),2);
                assert_eq!(items.items().iter().filter(|i|blocks::item_name(i.stack.item)==Some("sniffer_egg")).map(|i|i.stack.count).sum::<i32>(),1);
            } else {
                assert_eq!(items.mobs().len(),3); assert_eq!(items.mobs().iter().filter(|m|m.is_baby()).count(),1);
            }
            assert!(items.mobs().iter().filter(|m|!m.is_baby()).all(|m|m.age>0 && m.love==0));
        }
    }

    #[test]
    fn baby_sniffer_cannot_dig() {
        let mut m=mob(Kind::Sniffer); m.age = -100; m.small.sniff=5; m.small.drop_seed_at=1;
        for _ in 0..200 { m.ticks+=1; sniffer(&mut m,&Ground,&context(),&mut Rng::new(2)); }
        assert!(!m.events.iter().any(|e| matches!(e,Event::Loot(_))));
        assert_eq!(m.small.drop_seed_at,0);
    }
    #[test]
    fn parrot_seeds_never_breed_and_cookie_kills_tamed_bird() {
        let mut m=mob(Kind::Parrot); let who=([3;16],7); let mut rng=Rng::new(9);
        for _ in 0..200 { if m.tame.tamed() { break; } parrot_interact(&mut m,who,Some("wheat_seeds"),&mut rng); }
        assert_eq!(m.tame.owner,Some(who.0)); assert_eq!(m.love,0);
        assert!(!Kind::Parrot.likes("wheat_seeds"));
        parrot_interact(&mut m,who,Some("cookie"),&mut rng); assert!(m.is_dead());
    }
    #[test]
    fn bat_loses_ceiling_and_flying_mobs_ignore_fall_damage() {
        let mut m=mob(Kind::Bat); assert!(m.small.resting);
        bat(&mut m,&Ground,&context(),&mut Rng::new(4)); assert!(!m.small.resting);
        for k in [Kind::Bat,Kind::Parrot,Kind::Allay] {
            let mut m=mob(k); assert!(m.hurt(100.0,crate::survival::Source::Fall,None).is_none());
            assert!(!m.is_dead());
        }
    }
    #[test]
    fn bat_rests_under_stone_but_not_glass_or_leaves() {
        struct Roof(i32);
        impl View for Roof {
            fn block(&self,_x:i32,y:i32,_z:i32)->i32 { if y==2 { self.0 } else { 0 } }
            fn loaded(&self,_x:i32,_z:i32)->bool { true }
        }
        for (name,resting) in [("stone",true),("glass",false),("oak_leaves",false)] {
            let mut m=mob(Kind::Bat);
            bat(&mut m,&Roof(blocks::state_by_name(name).unwrap()),&context(),&mut Rng::new(4));
            assert_eq!(m.small.resting,resting,"{name}");
        }
    }
    #[test]
    fn allay_regenerates_and_cooldowns_expire_without_underflow() {
        let mut m=mob(Kind::Allay); m.vitals.health=10.0; m.small.duplicate=2; m.small.note_ticks=1; m.small.note=Some((1,2,3));
        m.ticks=20; tick(&mut m,&mut Rng::new(1)); assert_eq!(m.vitals.health,12.0); assert_eq!(m.small.note,None);
        m.ticks=21; tick(&mut m,&mut Rng::new(1)); assert_eq!(m.small.duplicate,0);
        tick(&mut m,&mut Rng::new(1)); assert_eq!(m.small.duplicate,0);
    }
    #[test]
    fn new_mob_records_round_trip_without_consuming_the_next_record() {
        let mut bytes=Vec::new();
        let kinds=[Kind::Armadillo,Kind::Sniffer,Kind::Parrot,Kind::Bat,Kind::Allay];
        for k in kinds {
            let mut m=mob(k); m.small.scute=123; m.small.duplicate=5900;
            m.small.explored=vec![(1,2,3)]; m.small.variant=4; m.tame.owner=Some([42;16]);
            let stack=Stack::new(blocks::item_named("diamond").unwrap(),3).with_extras(crate::stackdata::Data { name:Some("test".into()),..Default::default() });
            m.small.held=Some(Stack { count:1,..stack }); m.small.cargo=Some(stack); m.write(&mut bytes);
        }
        let mut input=bytes.as_slice();
        for k in kinds {
            let m=Mob::read(&mut input).unwrap(); assert_eq!(m.kind,k); assert_eq!(m.small.scute,123);
            assert_eq!(m.small.duplicate,5900); assert_eq!(m.small.explored,vec![(1,2,3)]);
            assert_eq!(m.small.variant,4); assert_eq!(m.tame.owner,Some([42;16]));
            assert_eq!(m.small.cargo.unwrap().count,3); assert_eq!(m.small.held.unwrap().extras().name.as_deref(),Some("test"));
        }
        assert!(input.is_empty());
    }
    #[test]
    fn wolf_armor_repair_requires_owner_and_sitting_and_caps_at_64() {
        let mut m=mob(Kind::Wolf); let who=([8;16],2); tame::tame(&mut m,who); m.tame.armor=Some(55);
        assert!(matches!(tame::interact(&mut m,who,Some("armadillo_scute"),&mut Rng::new(1)),tame::Used::Spent));
        assert_eq!(m.tame.armor,Some(63));
        m.tame.armor=Some(10);
        tame::interact(&mut m,([9;16],3),Some("armadillo_scute"),&mut Rng::new(1)); assert_eq!(m.tame.armor,Some(10));
    }
    #[test]
    fn halloween_boundaries_and_bat_light_limits() {
        assert!(!super::super::spawn::halloween(10,19)); assert!(super::super::spawn::halloween(10,20));
        assert!(super::super::spawn::halloween(11,3)); assert!(!super::super::spawn::halloween(11,4));
        let mut rng=Rng::new(6);
        for _ in 0..100 { assert!(!super::super::spawn::bat_light(7,true,&mut rng)); assert!(!super::super::spawn::bat_light(4,false,&mut rng)); }
    }
}
