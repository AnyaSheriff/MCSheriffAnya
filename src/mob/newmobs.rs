// Новые мобы: правила minecraft.wiki (Endermite, Zoglin, Creaking,
// Happy Ghast, Copper Golem), номера — minecraft-data 26.1 JSON.
// [проверить] чёрный ящик для этой партии запускать запрещено.
use super::{ai::{self, Ctx, Plan, Watcher}, hostile, path, Event, Kind, Mob, Rng, View};
use crate::inventory::Stack;

pub const LIFETIME: i32 = 2400;
pub const ZOMBIFY: i32 = 300;
pub const COLOURS: [&str; 16] = ["white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray", "light_gray", "cyan", "purple", "blue", "brown", "green", "red", "black"];

#[derive(Clone, Debug, Default)]
pub struct State {
    pub lifetime: i32,
    pub outside: i32,
    pub heart: Option<(i32, i32, i32)>,
    pub frozen: bool,
    pub active: bool,
    pub resin_hit: bool,
    pub resin_wait: i32,
    pub harness: Option<u8>,
    pub ridden: bool,
    pub still: i32,
    pub home: Option<(f64,f64,f64)>,
    pub tempted: bool,
    pub was_leashed: bool,
    pub oxidation: u8,
    pub waxed: bool,
    pub oxidize_at: i64,
    pub cargo: Option<Stack>,
    pub chest: Option<(i32, i32, i32)>,
    pub visited: Vec<(i32, i32, i32)>,
    pub chest_wait: i32,
}

pub fn new_kind(kind: Kind) -> bool {
    matches!(kind, Kind::Endermite | Kind::Zoglin | Kind::Creaking | Kind::HappyGhast | Kind::CopperGolem)
}

pub fn lifecycle(mob: &mut Mob, view: &dyn View) {
    if new_kind(mob.kind) || mob.kind == Kind::ZombifiedPiglin {
        if mob.kind == Kind::Zoglin {mob.vitals.effects.retain(|e|e.kind!=crate::survival::EffectKind::Poison);}
        for (damage,source) in mob.vitals.effects_tick().hurt {mob.hurt(damage,source,None);}
    }
    if mob.kind == Kind::Endermite && !mob.no_ai && !mob.persistent_tag && mob.name.is_none() {
        mob.newmobs.lifetime += 1;
        mob.h.despawn |= mob.newmobs.lifetime >= LIFETIME;
    }
    if matches!(mob.kind, Kind::Hoglin | Kind::Piglin | Kind::PiglinBrute) {
        if view.piglin_safe() || mob.nether.immune || mob.no_ai {
            mob.newmobs.outside = 0;
        } else {
            mob.newmobs.outside += 1;
            // Чёрный ящик (hoglin-zombify): хоглин и пиглин — на 300-м такте, брут — на 301-м.
            if mob.newmobs.outside > ZOMBIFY + i32::from(mob.kind == Kind::PiglinBrute) {
                mob.h.converted = Some(if mob.kind == Kind::Hoglin { Kind::Zoglin } else { Kind::ZombifiedPiglin });
            }
        }
    }
    if let Some(at) = mob.newmobs.heart
        && view.loaded(at.0.div_euclid(16),at.2.div_euclid(16))
        && crate::blocks::block_at_state(view.block(at.0, at.1, at.2)) != Some("creaking_heart")
    {
        mob.hurt(1.0, crate::survival::Source::GenericKill, None);
    }
    mob.newmobs.resin_wait = (mob.newmobs.resin_wait - 1).max(0);
}

/// Вики: поле зрения 60° относительно взгляда; творческий и тыква не замораживают.
pub fn watched(mob: &Mob, view: &dyn View, player: &Watcher) -> bool {
    if !player.targetable || player.spectator || player.pumpkin { return false; }
    let from = (player.x, player.y + player.eye, player.z);
    let to = (mob.x, mob.y + mob.eye(), mob.z);
    let d = (to.0 - from.0, to.1 - from.1, to.2 - from.2);
    let length = (d.0*d.0 + d.1*d.1 + d.2*d.2).sqrt();
    if length > 32.0 { return false; }
    let (sy, cy) = f64::from(player.look.0).to_radians().sin_cos();
    let (sp, cp) = f64::from(player.look.1).to_radians().sin_cos();
    let dot = -sy*cp*d.0 - sp*d.1 + cy*cp*d.2;
    dot >= length * 0.5 && hostile::sees(view, from, to)
}

pub fn freeze(mob: &mut Mob, view: &dyn View, ctx: &Ctx) -> bool {
    let before=(mob.newmobs.active,mob.newmobs.frozen);
    mob.newmobs.active = ctx.players.iter().any(|p| p.targetable && !p.spectator && (p.x-mob.x).powi(2)+(p.y-mob.y).powi(2)+(p.z-mob.z).powi(2) <= 32.0*32.0 && hostile::sees(view,(mob.x,mob.y+mob.eye(),mob.z),(p.x,p.y+p.eye,p.z)));
    mob.newmobs.frozen = ctx.players.iter().any(|p| watched(mob, view, p));
    if before.0 != mob.newmobs.active {
        mob.events.push(Event::Sound{name:if mob.newmobs.active {"minecraft:entity.creaking.activate"}else{"minecraft:entity.creaking.deactivate"},volume:1.0,pitch:1.0});
    }
    if before.1 != mob.newmobs.frozen {
        mob.events.push(Event::Sound{name:if mob.newmobs.frozen {"minecraft:entity.creaking.freeze"}else{"minecraft:entity.creaking.unfreeze"},volume:1.0,pitch:1.0});
    }
    if mob.newmobs.frozen {
        mob.brain.stop();
        (mob.vx, mob.vy, mob.vz) = (0.0, 0.0, 0.0);
    }
    mob.newmobs.frozen
}

/// Эндермен охотится на эндермитов; зоглин — на живых, кроме криперов и зоглинов.
/// [проверить] очередность цели среди игроков и мобов.
pub fn hunt_mob(mob: &mut Mob, view: &dyn View, ctx: &Ctx, rng: &mut Rng) -> Option<Plan> {
    if !matches!(mob.kind, Kind::Enderman | Kind::Zoglin) || (mob.kind == Kind::Enderman && mob.h.provoked) { return None; }
    let was_hunting = mob.newmobs.active;
    if mob.kind == Kind::Enderman { mob.newmobs.active = false; }
    // endermite-pearl: эндермен берёт цель-эндермита через 12 тактов после появления.
    if mob.kind == Kind::Enderman && mob.ticks < 12 { return None; }
    let range: f64 = if mob.kind == Kind::Enderman { 64.0 } else { 16.0 };
    let target = ctx.mates.iter().filter(|m| m.id != mob.entity_id && if mob.kind == Kind::Enderman { m.kind == Kind::Endermite } else { !m.kind.object() && !matches!(m.kind, Kind::Zoglin | Kind::Creeper | Kind::Ghast) })
        .filter(|m| (m.x-mob.x).powi(2)+(m.y-mob.y).powi(2)+(m.z-mob.z).powi(2) <= range*range)
        .filter(|m| hostile::sees(view, (mob.x,mob.y+mob.eye(),mob.z), (m.x,m.y+m.kind.height()*0.5,m.z)))
        .min_by(|a,b| ((a.x-mob.x).powi(2)+(a.z-mob.z).powi(2)).total_cmp(&((b.x-mob.x).powi(2)+(b.z-mob.z).powi(2))))?;
    let watcher = Watcher { id: target.id, x: target.x, y: target.y, z: target.z, eye: target.kind.height()*0.85, holding:[None;2], spectator:false,targetable:true,look:(0.0,0.0),pumpkin:false,gold_armor:false,sprinting:false,riding:false };
    // endermite-pearl: первый удар — через 18 тактов после взятия цели, дальше раз в 18.
    if mob.kind == Kind::Enderman && !was_hunting { mob.h.cooldown = mob.h.cooldown.max(15); }
    if mob.kind == Kind::Enderman { mob.newmobs.active = true; }
    let dist = (target.x-mob.x).powi(2)+(target.y-mob.y).powi(2)+(target.z-mob.z).powi(2);
    let flat = (target.x-mob.x).hypot(target.z-mob.z);
    let mut plan = Plan::default();
    hostile::melee(mob, view, &watcher, &mut plan, dist, flat, rng, mob.on_ground);
    if let Some(hit) = plan.melee.take() {
        plan.bite = Some((hit.target,hit.damage,hit.push));
        if mob.kind == Kind::Enderman { mob.h.cooldown = 18; }
        if mob.kind == Kind::Zoglin {
            mob.h.cooldown = if mob.is_baby() { 20 } else { 40 };
        }
    }
    Some(plan)
}

/// Снежок ускоряет рост на 10 % оставшихся целых секунд (вики, Happy Ghast).
pub fn feed_snowball(mob: &mut Mob) -> bool {
    if mob.kind != Kind::HappyGhast || !mob.is_baby() { return false; }
    mob.age += ((-mob.age / 20) as f32 * 0.1) as i32 * 20;
    true
}

pub fn harness_colour(name: &str) -> Option<u8> {
    let colour = name.strip_suffix("_harness")?;
    COLOURS.iter().position(|c| *c == colour).map(|i| i as u8)
}

/// [проверить] полёт: общий ход летающих мобов, без боя и падения.
pub fn happy_tick(mob: &mut Mob, view: &dyn View, ctx: &Ctx, rng: &mut Rng) -> Plan {
    ai::voice(mob,rng);
    if mob.newmobs.ridden {mob.newmobs.still=0;} else if ctx.players.iter().any(|p| !p.spectator && (p.x-mob.x).abs() <= 2.0 && (p.z-mob.z).abs() <= 2.0 && p.y >= mob.y+mob.size().1 && p.y < mob.y+mob.size().1+2.0) {mob.newmobs.still=10;} else {mob.newmobs.still=(mob.newmobs.still-1).max(0);}
    if mob.newmobs.ridden || mob.newmobs.still>0 { (mob.vx,mob.vy,mob.vz)=(0.0,0.0,0.0); return Plan::default(); }
    if mob.newmobs.home.is_none() || (mob.newmobs.was_leashed && mob.leash.is_none()) {
        mob.newmobs.home=Some((mob.x,mob.y,mob.z));
    }
    mob.newmobs.was_leashed=mob.leash.is_some();
    let lure = ctx.players.iter().filter(|p| !p.spectator && p.holding.iter().flatten().any(|i| *i == "snowball" || (mob.newmobs.harness.is_none() && harness_colour(i).is_some())))
        .find(|p| (p.x-mob.x).powi(2)+(p.y-mob.y).powi(2)+(p.z-mob.z).powi(2) < 16.0*16.0);
    let target=lure.map(|p|(p.x,p.y+2.0,p.z)).or_else(|| {
        if !mob.is_baby() {return None;}
        ctx.players.iter().filter(|p| !p.spectator)
            .map(|p|(p.x,p.y+2.0,p.z))
            .find(|p| (p.0-mob.x).powi(2)+(p.1-mob.y).powi(2)+(p.2-mob.z).powi(2)<256.0)
            .or_else(||ctx.mates.iter().filter(|m|m.id!=mob.entity_id && !m.baby && matches!(m.kind,Kind::Cow|Kind::Pig|Kind::Sheep|Kind::Chicken|Kind::Cat|Kind::Wolf|Kind::Strider|Kind::Villager|Kind::HappyGhast))
                .map(|m|(m.x,m.y+m.kind.height(),m.z))
                .find(|p|(p.0-mob.x).powi(2)+(p.1-mob.y).powi(2)+(p.2-mob.z).powi(2)<256.0))
    });
    if mob.newmobs.tempted && target.is_none() {mob.newmobs.home=Some((mob.x,mob.y,mob.z));}
    mob.newmobs.tempted=target.is_some();
    if let Some(at)=target {
        let close=(at.0-mob.x).powi(2)+(at.1-mob.y).powi(2)+(at.2-mob.z).powi(2)<if mob.is_baby(){25.0}else{9.0};
        mob.nether.wander=(!close).then_some(at);
        if close {(mob.vx,mob.vy,mob.vz)=(mob.vx*0.8,mob.vy*0.8,mob.vz*0.8);}
    } else if mob.nether.wander.is_none() || rng.below(120) == 0 {
        let home=mob.newmobs.home.unwrap();
        let radius=if mob.is_baby() || mob.newmobs.harness.is_some() {32.0}else{64.0};
        let candidate=(mob.x+rng.unit()*16.0-8.0,mob.y+rng.unit()*8.0-4.0,mob.z+rng.unit()*16.0-8.0);
        mob.nether.wander=Some(if (candidate.0-home.0).powi(2)+(candidate.1-home.1).powi(2)+(candidate.2-home.2).powi(2)<radius*radius {candidate}else{home});
    }
    super::nether::drift(mob,view);
    mob.vitals.fall = 0.0;
    Plan::default()
}

pub fn copper_stage(name: &str) -> Option<u8> {
    Some(match name.strip_prefix("waxed_").unwrap_or(name) {
        "copper_block" => 0, "exposed_copper" => 1, "weathered_copper" => 2, "oxidized_copper" => 3, _ => return None,
    })
}

pub fn copper_use(mob: &mut Mob, item: &str) -> bool {
    if mob.kind != Kind::CopperGolem { return false; }
    let sound = if item == "honeycomb" && !mob.newmobs.waxed {
        mob.newmobs.waxed = true;
        "minecraft:item.honeycomb.wax_on"
    } else if item.ends_with("_axe") && mob.newmobs.waxed {
        mob.newmobs.waxed = false;
        mob.newmobs.oxidize_at = 0;
        "minecraft:item.axe.wax_off"
    } else if item.ends_with("_axe") && mob.newmobs.oxidation > 0 {
        mob.newmobs.oxidation -= 1;
        mob.newmobs.oxidize_at = 0;
        "minecraft:item.axe.scrape"
    } else { return false; };
    mob.events.push(Event::Sound{name:sound,volume:1.0,pitch:1.0});
    true
}

/// Высота сидений над точкой радостного гаста; в `Spot` транспорта играет роль метки.
pub const GHAST_SEAT_Y: f64 = 3.4;

/// Место сиденья на спине; порядок: перед, право, зад, лево [проверить].
pub fn seat(at: (f64,f64,f64), yaw: f32, index: usize) -> (f64,f64,f64) {
    let angle = f64::from(yaw).to_radians() + index as f64 * std::f64::consts::FRAC_PI_2;
    (at.0-angle.sin()*1.7,at.1+GHAST_SEAT_Y,at.2+angle.cos()*1.7)
}

/// Путь сортировщика использует ту же сетку, что у остальных наземных мобов.
pub fn walk_to(mob: &mut Mob, view: &dyn View, at: (i32,i32,i32)) {
    if mob.brain.repath <= 0 || !mob.brain.walking() {
        let route = path::find(view,mob.cell(),at,path::MAX_NODES);
        mob.brain.go(route,1.0);
        mob.brain.repath = 20;
    }
    mob.brain.repath -= 1;
}

/// Хвост записи новых видов: имя и поводок остаются в общем хвосте.
pub fn write(s: &State, out: &mut Vec<u8>) {
    out.extend_from_slice(&s.lifetime.to_be_bytes());
    out.extend_from_slice(&s.outside.to_be_bytes());
    out.push(u8::from(s.heart.is_some()));
    if let Some((x,y,z))=s.heart {for n in [x,y,z] {out.extend_from_slice(&n.to_be_bytes());}}
    out.push(u8::from(s.home.is_some()));
    if let Some((x,y,z))=s.home {for n in [x,y,z] {out.extend_from_slice(&n.to_be_bytes());}}
    out.push(s.harness.unwrap_or(255));
    out.push(s.oxidation); out.push(u8::from(s.waxed));
    out.extend_from_slice(&s.oxidize_at.to_be_bytes());
    out.push(u8::from(s.cargo.is_some()));
    if let Some(stack)=s.cargo {
        out.extend_from_slice(&stack.stored_item().to_be_bytes());
        out.extend_from_slice(&stack.count.to_be_bytes());
        stack.write_extras(out);
    }
}
pub fn read(bytes: &mut &[u8]) -> Option<State> {
    let mut take=|n| {let (a,b)=bytes.split_at_checked(n)?;*bytes=b;Some(a)};
    let lifetime=i32::from_be_bytes(take(4)?.try_into().ok()?);
    let outside=i32::from_be_bytes(take(4)?.try_into().ok()?);
    let heart=if take(1)?[0]!=0 {Some((i32::from_be_bytes(take(4)?.try_into().ok()?),i32::from_be_bytes(take(4)?.try_into().ok()?),i32::from_be_bytes(take(4)?.try_into().ok()?)))} else {None};
    let home=if take(1)?[0]!=0 {
        let (x,y,z)=(f64::from_be_bytes(take(8)?.try_into().ok()?),f64::from_be_bytes(take(8)?.try_into().ok()?),f64::from_be_bytes(take(8)?.try_into().ok()?));
        if !x.is_finite() || !y.is_finite() || !z.is_finite() {return None;}
        Some((x,y,z))
    }else{None};
    let colour=take(1)?[0];let harness=if colour<16 {Some(colour)} else if colour==255 {None} else {return None};
    let oxidation=take(1)?[0]; if oxidation>3 {return None;}
    let waxed=take(1)?[0]!=0;
    let oxidize_at=i64::from_be_bytes(take(8)?.try_into().ok()?);
    let cargo=if take(1)?[0]!=0 {
        let id=i32::from_be_bytes(take(4)?.try_into().ok()?);let count=i32::from_be_bytes(take(4)?.try_into().ok()?);
        if count<=0 || count>16 {return None;}
        let data=Stack::read_extras(id,&mut take)?;
        let stack=Stack::from_stored(id,count);
        Some(if let Some(data)=data {stack.with_extras(data)} else {stack})
    }else{None};
    Some(State{lifetime,outside,heart,home,harness,oxidation,waxed,oxidize_at,cargo,..State::default()})
}
