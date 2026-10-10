// Кролик, лиса, пчела: minecraft.wiki/w/Rabbit, /Fox, /Bee, /Beehive.
// Кода игры здесь нет. [проверить] ИИ, сроки целей и движение требуют
// чёрного ящика; сценарии написаны, но по заданию не запускались.
use super::{ai, Kind, Mob, Rng, View, Event};
use crate::{blocks, inventory::Stack};
use serde::{Deserialize, Serialize};

type Cell = (i32, i32, i32);

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct State {
    pub variant: u8,
    pub silent: bool,
    pub sleeping: bool,
    pub crouching: bool,
    pub pouncing: bool,
    pub trusted: [Option<[u8; 16]>; 2],
    #[serde(skip)]
    pub trusted_ids: [i32; 2],
    pub breeder: Option<[u8; 16]>,
    #[serde(skip)]
    pub mouth: Option<Stack>,
    pub eating: i32,
    pub cooldown: i32,
    pub hop: i32,
    pub target: Option<Cell>,
    #[serde(skip)]
    pub item_target: Option<Cell>,
    pub nectar: bool,
    pub stung: bool,
    pub death_in: i32,
    pub anger: i32,
    pub angry_at: Option<[u8;16]>,
    #[serde(skip)]
    pub attacker: Option<i32>,
    #[serde(skip)]
    pub alert: bool,
    pub hive: Option<Cell>,
    pub flower: Option<Cell>,
    pub pollination: i32,
    pub crops: u8,
    pub cannot_enter: i32,
    pub enter: bool,
    /// Через сколько тактов после удара включится гнев: цель обидчика берёт
    /// ИИ, а не сам удар (чёрный ящик, bee-sting: 19 приходит на +2).
    /// Такты до следующего поиска цели («идти к блоку»).
    pub search: i32,
    /// Только что вышла из улья: в этот же такт не ходит (у оригинала улей
    /// тикает после сущностей), слежение увидит её со следующего.
    #[serde(skip)]
    pub fresh: bool,
    pub anger_delay: i32,
    /// Флаг «с нектаром» ещё один такт числится у клиента (см. hive.rs).
    #[serde(skip)]
    pub nectar_ghost: bool,
    /// «Катается» (флаг 2) — за три такта до жала (чёрный ящик, bee-sting: 18=2 на +42, жало на +45).
    pub roll: bool,
    /// Свойство «агрессивен» (15=4): цель взята в погоню (+12 после удара).
    pub chase: bool,
    /// Через сколько тактов после жала «агрессивен» гаснет (15=0 на +48 при жале на +45).
    pub calm: i32,
    /// Свойство «без гравитации» (5=true): пчела пошла в полёт и больше не садится.
    pub flying: bool,
}

pub fn kind(kind: Kind) -> bool { matches!(kind, Kind::Rabbit | Kind::Fox | Kind::Bee) }

/// Вики, Bee: «flowers»; ядовитые цветы не дают любви.
pub fn flower(name: &str) -> bool {
    matches!(name, "dandelion" | "poppy" | "blue_orchid" | "allium" | "azure_bluet" | "red_tulip" | "orange_tulip" | "white_tulip" | "pink_tulip" | "oxeye_daisy" | "cornflower" | "lily_of_the_valley" | "sunflower" | "lilac" | "rose_bush" | "peony" | "torchflower" | "pitcher_plant" | "pink_petals" | "wildflowers" | "cactus_flower" | "flowering_azalea" | "flowering_azalea_leaves" | "cherry_leaves" | "mangrove_propagule" | "spore_blossom" | "chorus_flower")
}

/// Вики, Bee: wither rose — 40 тактов иссушения, open eyeblossom — 25 яда.
pub fn harmful_flower(name: &str) -> Option<crate::survival::Effect> {
    let (kind,ticks_left)=match name {
        "wither_rose"=>(crate::survival::EffectKind::Wither,40),
        "open_eyeblossom"=>(crate::survival::EffectKind::Poison,25),
        _=>return None,
    };
    Some(crate::survival::Effect {kind,amplifier:0,ticks_left,ambient:false,visible:true})
}

pub fn pollinatable(state: i32) -> bool {
    let name = blocks::block_at_state(state).unwrap_or("");
    flower(name) || matches!(name, "flowering_azalea" | "flowering_azalea_leaves" | "mangrove_propagule" | "wither_rose" | "open_eyeblossom")
}

/// Вики, Rabbit: «80% white», пустыня — золотой; прочие 50/40/10.
/// 0–5 — обычные шкурки, 99 — killer bunny; Toast задаётся именем.
pub fn rabbit_variant(biome: &str, rng: &mut Rng) -> u8 {
    let biome = biome.trim_start_matches("minecraft:");
    if biome == "desert" { 4 }
    else if matches!(biome, "snowy_plains" | "ice_spikes" | "snowy_taiga" | "grove" | "snowy_slopes") { if rng.below(100) < 80 { 1 } else { 3 } }
    else { match rng.below(100) { 0..50 => 0, 50..90 => 5, _ => 2 } }
}

pub fn fox_variant(biome: &str) -> u8 {
    u8::from(matches!(biome.trim_start_matches("minecraft:"), "snowy_taiga" | "grove"))
}

pub fn initialize(mob: &mut Mob, biome: &str, rng: &mut Rng) {
    if mob.kind == Kind::Rabbit { mob.wild.variant = rabbit_variant(biome, rng); }
    if mob.kind == Kind::Fox {
        mob.wild.variant = fox_variant(biome);
        // Вики, Fox: «20% chance» держать вещь при появлении.
        if rng.below(5) == 0 {
            let name = match rng.below(100) { 0..5 => "emerald", 5..15 => "rabbit_foot", 15..25 => "rabbit_hide", 25..40 => "egg", 40..60 => "wheat", 60..80 => "leather", _ => "feather" };
            mob.wild.mouth = blocks::item_named(name).map(|id| Stack::new(id, 1));
        }
    }
}

pub fn inherit(baby: &mut Mob, a: &Mob, b: &Mob, view: &dyn View, rng: &mut Rng) {
    if matches!(baby.kind, Kind::Rabbit | Kind::Fox) {
        baby.wild.variant = if rng.below(2) == 0 { a.wild.variant } else { b.wild.variant };
    }
    if baby.kind == Kind::Rabbit && rng.below(20) == 0 {
        let (x, y, z) = baby.cell();
        baby.wild.variant = rabbit_variant(view.biome(x, y, z), rng);
    }
    if baby.kind == Kind::Fox {
        baby.wild.trusted[0] = a.wild.breeder.or(b.wild.breeder);
        baby.wild.trusted[1] = b.wild.breeder.filter(|who| Some(*who) != baby.wild.trusted[0]);
    }
}

pub fn bind_trust(mobs: &mut [Mob], players: &[([u8; 16], i32)]) {
    for bee in mobs.iter_mut().filter(|m|m.kind==Kind::Bee && m.wild.anger>0) {
        if let Some(id)=bee.wild.attacker && let Some((uuid,_))=players.iter().find(|(_,n)|*n==id) {bee.wild.angry_at=Some(*uuid);}
        if bee.wild.attacker.is_none() {bee.wild.attacker=players.iter().find(|(uuid,_)|Some(*uuid)==bee.wild.angry_at).map(|(_,id)|*id);}
    }
    for mob in mobs.iter_mut().filter(|mob| mob.kind == Kind::Fox) {
        for n in 0..2 {
            mob.wild.trusted_ids[n] = players.iter().find(|(uuid, _)| Some(*uuid) == mob.wild.trusted[n]).map_or(0, |(_, id)| *id);
        }
    }
}

pub fn on_hurt(mob: &mut Mob, cause: i32) {
    mob.wild.sleeping = false;
    // Без ИИ пчела не злится вовсе (чёрный ящик, bee-hit: после ударов только
    // здоровье 9=…); с ИИ гнев приходит через 2 такта (bee-sting).
    if mob.kind == Kind::Bee && !mob.wild.stung && !mob.no_ai {
        mob.wild.attacker = Some(cause);
        if mob.wild.anger == 0 && mob.wild.anger_delay == 0 { mob.wild.anger_delay = 2; }
    }
}

pub fn pack_anger(mobs: &mut [Mob]) {
    let calls: Vec<_> = mobs.iter().filter(|m| m.kind == Kind::Bee && m.wild.alert).map(|m| ((m.x, m.y, m.z), m.wild.attacker, m.wild.anger)).collect();
    for mob in mobs.iter_mut().filter(|m| m.kind == Kind::Bee) {
        mob.wild.alert = false;
        if mob.wild.stung || mob.no_ai { continue; }
        for (at, target, anger) in &calls {
            // Вики, Bee: «16 blocks» по горизонтали и 10 по вертикали.
            if (mob.x - at.0).abs() <= 16.0 && (mob.y - at.1).abs() <= 10.0 && (mob.z - at.2).abs() <= 16.0 {
                mob.wild.attacker = *target;
                mob.wild.anger = *anger;
            }
        }
    }
}

/// Поиск блока «идти к блоку»: первый раз сразу, дальше раз в 200–400 тактов
/// (вики, Fox: ягоды; чёрный ящик, fox-berries — куст найден в первом такте,
/// ягоды сорваны через 40 тактов ожидания, на +42).
fn search_due(mob: &mut Mob, rng: &mut Rng) -> bool {
    if mob.wild.search>0 { mob.wild.search-=1; return false; }
    mob.wild.search=200+rng.below(200) as i32;
    true
}
fn distance(mob: &Mob, at: (f64, f64, f64)) -> f64 { (mob.x-at.0).powi(2)+(mob.y-at.1).powi(2)+(mob.z-at.2).powi(2) }
fn center(at: Cell) -> (f64, f64, f64) { (f64::from(at.0)+0.5, f64::from(at.1), f64::from(at.2)+0.5) }
fn seek(mob: &Mob, view: &dyn View, radius: i32, test: impl Fn(i32) -> bool) -> Option<Cell> {
    let (x,y,z) = mob.cell();
    let mut best = None;
    let mut best_dist = f64::INFINITY;
    for dy in -3..=3 { for dx in -radius..=radius { for dz in -radius..=radius {
        let at = (x+dx,y+dy,z+dz);
        if !view.loaded(at.0.div_euclid(16),at.2.div_euclid(16)) { continue; }
        let dist = distance(mob, center(at));
        if dist < best_dist && test(view.block(at.0,at.1,at.2)) { best=Some(at); best_dist=dist; }
    } } }
    best
}
fn go(mob: &mut Mob, view: &dyn View, at: Cell, speed: f64) -> ai::Plan {
    if mob.ticks.is_multiple_of(10) || !mob.brain.walking() {
        let route = super::path::find(view, mob.cell(), at, super::path::MAX_NODES);
        mob.brain.go(route, speed);
    }
    ai::walk(mob, ai::Plan::default())
}
fn sound(mob: &mut Mob, name: &'static str) { mob.events.push(Event::Sound { name, volume: 1.0, pitch: 1.0 }); }

pub fn think(mob: &mut Mob, view: &dyn View, ctx: &ai::Ctx, rng: &mut Rng) -> ai::Plan {
    mob.wild.cooldown = (mob.wild.cooldown - 1).max(0);
    match mob.kind {
        Kind::Rabbit => rabbit(mob, view, ctx, rng),
        Kind::Fox => fox(mob, view, ctx, rng),
        Kind::Bee => bee(mob, view, ctx, rng),
        _ => ai::Plan::default(),
    }
}

fn rabbit(mob: &mut Mob, view: &dyn View, ctx: &ai::Ctx, rng: &mut Rng) -> ai::Plan {
    let killer = mob.wild.variant == 99;
    let mut plan = if killer {
        if let Some(player) = ctx.players.iter().filter(|p| p.targetable && ctx.difficulty!=0).filter(|p| distance(mob,(p.x,p.y,p.z)) < 256.0).min_by(|a,b| distance(mob,(a.x,a.y,a.z)).total_cmp(&distance(mob,(b.x,b.y,b.z)))) {
            let mut plan = go(mob,view,(player.x.floor() as i32,player.y.floor() as i32,player.z.floor() as i32),1.4);
            if distance(mob,(player.x,player.y,player.z)) < 2.25 && mob.wild.cooldown == 0 {
                let len = ((player.x-mob.x).powi(2)+(player.z-mob.z).powi(2)).sqrt().max(0.001);
                plan.melee = Some(super::hostile::Melee { target: player.id, damage: 8.0, push: ((player.x-mob.x)/len,(player.z-mob.z)/len), wither: false, hunger: false });
                mob.wild.cooldown=20;
                sound(mob,"minecraft:entity.rabbit.attack");
            }
            plan
        } else if let Some(wolf)=ctx.mates.iter().find(|m|m.kind==Kind::Wolf && distance(mob,(m.x,m.y,m.z))<256.0) {
            let mut plan=go(mob,view,(wolf.x.floor() as i32,wolf.y.floor() as i32,wolf.z.floor() as i32),1.4);
            if distance(mob,(wolf.x,wolf.y,wolf.z))<2.25 && mob.wild.cooldown==0 {
                let len=((wolf.x-mob.x).powi(2)+(wolf.z-mob.z).powi(2)).sqrt().max(0.001);
                plan.bite=Some((wolf.id,8.0,((wolf.x-mob.x)/len,(wolf.z-mob.z)/len)));
                mob.wild.cooldown=20;sound(mob,"minecraft:entity.rabbit.attack");
            }
            plan
        } else { ai::think(mob,view,ctx,rng) }
    } else {
        if mob.love == 0 {
            let threat = ctx.players.iter().find(|p| p.targetable && !p.holding.iter().flatten().any(|food| mob.kind.likes(food)) && distance(mob,(p.x,p.y,p.z)) < 64.0).map(|p| (p.x,p.z))
                .or_else(|| ctx.mates.iter().find(|m| matches!(m.kind,Kind::Wolf|Kind::Fox) && distance(mob,(m.x,m.y,m.z))<100.0).map(|m| (m.x,m.z)));
            if let Some(at)=threat { mob.fright=Some(at); mob.brain.panic(20); }
        }
        ai::think(mob,view,ctx,rng)
    };
    if !killer && view.griefing() && mob.love == 0 && !mob.brain.is_panicking() && mob.wild.cooldown == 0 {
        if mob.wild.target.is_none() && search_due(mob,rng) {
            mob.wild.target=seek(mob,view,8,|state| blocks::block_at_state(state)==Some("carrots") && number(state,"age")==7);
        }
        if let Some(at)=mob.wild.target {
            let state=view.block(at.0,at.1,at.2);
            if blocks::block_at_state(state)!=Some("carrots") || number(state,"age")!=7 { mob.wild.target=None; }
            else if distance(mob,center(at)) < 2.0 {
                plan.put=with(state,"age","6").map(|state| (at,state));
                mob.wild.target=None; mob.wild.cooldown=40;
            } else { plan=go(mob,view,at,0.7); }
        }
    }
    // Вики, Rabbit: движение прыжками. [проверить] высота и пауза.
    if mob.on_ground {
        mob.wild.hop=(mob.wild.hop-1).max(0);
        if plan.speed>0.0 && mob.wild.hop==0 {
            mob.vy=if plan.jump {0.5} else {0.3};
            mob.events.push(Event::Status(1));
            sound(mob,"minecraft:entity.rabbit.jump");
            mob.wild.hop=if mob.brain.is_panicking() {1} else {10};
            plan.jump=false;
        } else { plan.speed=0.0; plan.jump=false; }
    }
    plan
}

fn fox(mob: &mut Mob, view: &dyn View, ctx: &ai::Ctx, rng: &mut Rng) -> ai::Plan {
    mob.tame.cooldown=(mob.tame.cooldown-1).max(0);
    if let Some(target)=mob.tame.target {
        let at=match target {
            super::tame::Target::Mob(id)=>ctx.mates.iter().find(|m|m.id==id).map(|m|(m.x,m.y,m.z,m.kind.width())),
            super::tame::Target::Player(id)=>ctx.players.iter().find(|p|p.id==id && p.targetable).map(|p|(p.x,p.y,p.z,0.6)),
        };
        if let Some(at)=at && ctx.difficulty!=0 {
            mob.wild.sleeping=false;
            return super::tame::attack(mob,view,target,at,ai::Plan::default(),rng);
        }
        mob.tame.target=None;
    }
    if let Some(held)=mob.wild.mouth && edible(held) {
        mob.wild.eating+=1;
        if mob.wild.eating>=600 {
            mob.wild.mouth=None; mob.wild.eating=0;
            let name=blocks::item_name(held.item).unwrap_or("");
            for effect in crate::survival::food_effects(name) {mob.vitals.give_effect_shown(effect);}
            if matches!(name,"beetroot_soup"|"mushroom_stew"|"rabbit_stew"|"suspicious_stew")
                && let Some(id)=blocks::item_named("bowl") {mob.events.push(Event::Loot(vec![Stack::new(id,1)]));}
            sound(mob,"minecraft:entity.fox.eat");
        }
    }
    let threat=ctx.players.iter().find(|p| p.targetable && !mob.wild.trusted_ids.contains(&p.id) && (p.eye>1.3 || distance(mob,(p.x,p.y,p.z))<2.25) && distance(mob,(p.x,p.y,p.z))<256.0).map(|p|(p.x,p.z))
        .or_else(||ctx.mates.iter().find(|m|m.kind==Kind::Wolf && !m.tamed && distance(mob,(m.x,m.y,m.z))<64.0).map(|m|(m.x,m.z)));
    if let Some(at)=threat {
        mob.wild.sleeping=false; mob.fright=Some(at); mob.brain.panic(20);
        return ai::think(mob,view,ctx,rng);
    }
    if mob.love>0 || mob.brain.is_panicking() { mob.wild.sleeping=false; return ai::think(mob,view,ctx,rng); }
    let (x,y,z)=mob.cell();
    let safe_sleep=ctx.sun_up && !view.rain(x,y,z) && !view.open_sky(x,y,z) && mob.on_ground;
    if !safe_sleep { mob.wild.sleeping=false; }
    if safe_sleep && (mob.wild.sleeping || rng.below(140)==0) { mob.wild.sleeping=true; mob.brain.stop(); ai::voice(mob,rng); return ai::Plan::default(); }
    if mob.wild.sleeping { return ai::Plan::default(); }
    // Вики, Fox: «chickens, rabbits»; рыбы добавятся с их видами мобов.
    if let Some(prey)=ctx.mates.iter().filter(|m|matches!(m.kind,Kind::Chicken|Kind::Rabbit)).filter(|m|distance(mob,(m.x,m.y,m.z))<256.0).min_by(|a,b|distance(mob,(a.x,a.y,a.z)).total_cmp(&distance(mob,(b.x,b.y,b.z)))) {
        let dist=distance(mob,(prey.x,prey.y,prey.z));
        let mut plan=go(mob,view,(prey.x.floor() as i32,prey.y.floor() as i32,prey.z.floor() as i32),1.2);
        mob.wild.crouching=dist>4.0 && dist<36.0;
        if mob.wild.crouching && mob.on_ground && rng.below(10)==0 {
            let len=((prey.x-mob.x).powi(2)+(prey.z-mob.z).powi(2)).sqrt().max(0.001);
            mob.vx=(prey.x-mob.x)/len*0.4; mob.vz=(prey.z-mob.z)/len*0.4; mob.vy=0.8;
            mob.wild.pouncing=true; mob.wild.crouching=false;
        } else if mob.on_ground { mob.wild.pouncing=false; }
        if dist<2.0 && mob.wild.cooldown==0 {
            let len=((prey.x-mob.x).powi(2)+(prey.z-mob.z).powi(2)).sqrt().max(0.001);
            plan.bite=Some((prey.id,fox_damage(mob,Some(prey.kind)),((prey.x-mob.x)/len,(prey.z-mob.z)/len)));
            mob.wild.cooldown=20; sound(mob,"minecraft:entity.fox.bite");
        }
        return plan;
    }
    mob.wild.crouching=false; mob.wild.pouncing=false;
    if view.griefing() && mob.wild.cooldown==0 {
        if mob.wild.target.is_none() && search_due(mob,rng) {
            mob.wild.target=seek(mob,view,16,berry_ready);
        }
        if let Some(at)=mob.wild.target {
            let state=view.block(at.0,at.1,at.2);
            if !berry_ready(state) { mob.wild.target=None; mob.wild.hop=0; }
            else if distance(mob,center(at))<2.0 {
                if blocks::block_at_state(state)!=Some("sweet_berry_bush") {
                    if let Some(old)=mob.wild.mouth.take() {mob.events.push(Event::Loot(vec![old]));sound(mob,"minecraft:entity.fox.spit");}
                    mob.wild.mouth=blocks::item_named("glow_berries").map(|id|Stack::new(id,1));
                    mob.wild.eating=0;mob.wild.target=None;mob.wild.cooldown=100;
                    sound(mob,"minecraft:block.cave_vines.pick_berries");
                    return ai::Plan {put:with(state,"berries","false").map(|s|(at,s)),..Default::default()};
                }
                mob.wild.hop+=1;
                // Ожидание у куста 40 тактов после подхода (чёрный ящик, fox-berries: ягоды на +42).
                if mob.wild.hop>=42 {
                    mob.wild.hop=0; mob.wild.target=None; mob.wild.cooldown=100;
                    let mut count=1+rng.below(2) as i32+i32::from(number(state,"age")==3);
                    // Вики, Fox: ягода во рту, только если рот пуст; остальное падает.
                    if mob.wild.mouth.is_none() {
                        mob.wild.mouth=blocks::item_named("sweet_berries").map(|id|Stack::new(id,1)); mob.wild.eating=0;count-=1;
                    }
                    if count>0 && let Some(id)=blocks::item_named("sweet_berries") { mob.events.push(Event::Loot(vec![Stack::new(id,count)])); }
                    sound(mob,"minecraft:block.sweet_berry_bush.pick_berries");
                    return ai::Plan { put: with(state,"age","1").map(|state|(at,state)), ..Default::default() };
                }
                return ai::Plan::default();
            } else { return go(mob,view,at,1.2); }
        }
    }
    if let Some(at)=mob.wild.item_target { return go(mob,view,at,1.2); }
    if ctx.sun_up && mob.ticks.is_multiple_of(40) {
        let (x,y,z)=mob.cell();
        let shade=(-8..=8).flat_map(|dx|(-8..=8).map(move |dz|(x+dx,y,z+dz)))
            .find(|&(x,y,z)|!view.open_sky(x,y,z) && super::path::standable(view,(x,y,z)));
        if let Some(at)=shade { return go(mob,view,at,1.0); }
    }
    ai::think(mob,view,ctx,rng)
}

/// Лиса держит вещь в основной руке: атрибуты и зачарования общие с боем.
pub fn fox_damage(mob: &Mob, target: Option<Kind>) -> f32 {
    let mut attrs=crate::attributes::Attributes::player();
    attrs.set_base(crate::attributes::ATTACK_DAMAGE,2.0);
    let mut bonus=0.0;
    if let Some(stack)=mob.wild.mouth {
        for (id,modifier) in crate::combat::granted(&stack,1) {attrs.set_modifier(id,modifier);}
        let (undead,arthropod)=target.map_or((false,false),|k|crate::combat::mob_traits(k.name()));
        bonus=crate::combat::enchant_bonus(&stack.extras(),undead,arthropod);
    }
    attrs.value(crate::attributes::ATTACK_DAMAGE) as f32+bonus
}

pub fn totem(mob: &mut Mob, source: crate::survival::Source) -> bool {
    if mob.kind!=Kind::Fox || source.bypasses_invulnerability() || mob.wild.mouth.is_none_or(|s|blocks::item_name(s.item)!=Some("totem_of_undying")) {return false;}
    mob.wild.mouth=None;mob.vitals.dead=false;mob.vitals.health=1.0;
    mob.vitals.effects.clear();mob.vitals.absorption=0.0;
    for (kind,amplifier,ticks_left) in [(crate::survival::EffectKind::Regeneration,1,900),(crate::survival::EffectKind::Absorption,1,100),(crate::survival::EffectKind::FireResistance,0,800)] {
        mob.vitals.give_effect_shown(crate::survival::Effect {kind,amplifier,ticks_left,ambient:false,visible:true});
    }
    mob.events.push(Event::Status(35));true
}

pub fn defend(mobs: &mut [Mob], player: i32, attacker: super::tame::Target, excluded: bool) {
    if excluded { return; }
    for fox in mobs.iter_mut().filter(|m|m.kind==Kind::Fox && !m.no_ai && !m.is_dead() && m.wild.trusted_ids.contains(&player)) {
        fox.tame.target=Some(attacker); fox.wild.sleeping=false;
    }
}

fn berry_ready(state: i32) -> bool {
    match blocks::block_at_state(state) {
        Some("sweet_berry_bush")=>number(state,"age")>=2,
        Some("cave_vines"|"cave_vines_plant")=>blocks::orientation_of(state).and_then(|b|b.value(state,"berries"))==Some("true"),
        _=>false,
    }
}

pub fn number(state: i32, key: &str) -> i32 { blocks::orientation_of(state).and_then(|block| block.value(state,key)).and_then(|n|n.parse().ok()).unwrap_or(0) }
fn edible(stack: Stack) -> bool { blocks::item_name(stack.item).is_some_and(|name| matches!(name, "apple" | "baked_potato" | "beef" | "beetroot" | "beetroot_soup" | "bread" | "carrot" | "chicken" | "cod" | "cooked_beef" | "cooked_chicken" | "cooked_cod" | "cooked_mutton" | "cooked_porkchop" | "cooked_rabbit" | "cooked_salmon" | "cookie" | "dried_kelp" | "glow_berries" | "golden_apple" | "enchanted_golden_apple" | "golden_carrot" | "melon_slice" | "mushroom_stew" | "mutton" | "porkchop" | "potato" | "pumpkin_pie" | "rabbit" | "rabbit_stew" | "suspicious_stew" | "honey_bottle" | "rotten_flesh" | "salmon" | "spider_eye" | "sweet_berries" | "tropical_fish" | "chorus_fruit" | "poisonous_potato" | "pufferfish")) }

pub fn pickup(items: &mut crate::items::Items, griefing: bool) {
    if !griefing { return; }
    let mut mobs=std::mem::take(items.mobs_vec());
    for fox in mobs.iter_mut().filter(|m|m.kind==Kind::Fox && !m.is_dead() && !m.no_ai) {
        fox.wild.item_target=items.items().iter().filter(|item| item.block.is_none() && (fox.wild.mouth.is_none() || (edible(item.stack) && fox.wild.mouth.is_some_and(|held|held.item!=item.stack.item))))
            .filter(|item|distance(fox,(item.x,item.y,item.z))<64.0)
            .min_by(|a,b|distance(fox,(a.x,a.y,a.z)).total_cmp(&distance(fox,(b.x,b.y,b.z))))
            .map(|item|(item.x.floor() as i32,item.y.floor() as i32,item.z.floor() as i32));
        let mut mouth=fox.wild.mouth;
        let mut old=None;
        let mut picked=false;
        let taken=items.take_near((fox.x,fox.y,fox.z),fox.entity_id,|stack| {
            if !picked && (mouth.is_none() || (edible(stack) && mouth.is_some_and(|held|held.item!=stack.item))) {
                picked=true; old=mouth; mouth=Some(stack.with_count(1)); 1
            } else { 0 }
        });
        if !taken.is_empty() {
            fox.wild.mouth=mouth; fox.wild.eating=0; fox.wild.item_target=None;
            if let Some(stack)=old { fox.events.push(Event::Loot(vec![stack])); sound(fox,"minecraft:entity.fox.spit"); }
        }
    }
    *items.mobs_vec()=mobs;
}

fn aim(mob: &mut Mob, at: (f64,f64,f64), speed: f64) {
    mob.wild.flying=true;
    ai::look_at_point(mob,at.0,at.1,at.2);
    let len=distance(mob,at).sqrt().max(0.001);
    // `speed` задан для трения 0,8 (предел 4·speed); пересчитан на 0,91 (предел ~10·a).
    let speed=speed*0.396;
    mob.vx+=(at.0-mob.x)/len*speed; mob.vy+=(at.1-mob.y)/len*speed; mob.vz+=(at.2-mob.z)/len*speed;
}

fn bee(mob: &mut Mob, view: &dyn View, ctx: &ai::Ctx, rng: &mut Rng) -> ai::Plan {
    mob.wild.cannot_enter=(mob.wild.cannot_enter-1).max(0);
    mob.wild.enter=false;

    if mob.wild.anger>0 && !mob.wild.stung && ctx.difficulty!=0 {
        mob.wild.anger-=1;
        // Цель берётся не каждый такт: проверка раз в 20 тактов жизни
        // (чёрный ящик, bee-sting: погоня на +11 после удара на +0).
        if !mob.wild.chase && mob.ticks%20==1 { mob.wild.chase=true; mob.wild.flying=true; }
        if !mob.wild.chase { return ai::Plan::default(); }
        if let Some(p)=ctx.players.iter().find(|p|Some(p.id)==mob.wild.attacker && p.targetable) {
            // Цель — у ног (чёрный ящик, bee-sting: жалит у земли, y=0 у игрока на y=0).
            let spot=(p.x,p.y+0.2,p.z);
            aim(mob,spot,0.0325);
            // Расстояния в квадрате: дотягивается до (2·ширина пчелы)² + ширина цели = 2,56,
            // катается за три такта до того (~2,0 блока).
            mob.wild.roll=distance(mob,spot)<4.0;
            if distance(mob,spot)<2.56 && mob.wild.cooldown==0 {
                mob.wild.cooldown=20;
                mob.events.push(Event::Swing);
                let len=((p.x-mob.x).powi(2)+(p.z-mob.z).powi(2)).sqrt().max(0.001);
                return ai::Plan { melee: Some(super::hostile::Melee { target:p.id, damage:2.0,push:((p.x-mob.x)/len,(p.z-mob.z)/len),wither:false,hunger:false }), ..Default::default() };
            }
            return ai::Plan::default();
        }
        if let Some(target)=ctx.mates.iter().find(|m|Some(m.id)==mob.wild.attacker) {
            aim(mob,(target.x,target.y+target.kind.height()*0.5,target.z),0.06);
            if distance(mob,(target.x,target.y,target.z))<2.0 && mob.wild.cooldown==0 {
                mob.wild.cooldown=20;
                let len=((target.x-mob.x).powi(2)+(target.z-mob.z).powi(2)).sqrt().max(0.001);
                return ai::Plan {bite:Some((target.id,2.0,((target.x-mob.x)/len,(target.z-mob.z)/len))),..Default::default()};
            }
            return ai::Plan::default();
        }
    } else if mob.wild.anger_delay==0 { mob.wild.anger=0; mob.wild.attacker=None; mob.wild.angry_at=None; }
    // Общая цель разведения сохраняет сроки пассивных; полёт — к партнёру.
    if mob.love>0 {
        if let Some(mate)=ctx.mates.iter().find(|m|m.kind==Kind::Bee && m.id!=mob.entity_id && m.love>0 && m.ready) {
            if distance(mob,(mate.x,mate.y,mate.z))<9.0 {
                mob.wild.hop+=1;
                if mob.wild.hop>=60 { mob.wild.hop=0; return ai::Plan { breed:Some(mate.id),..Default::default() }; }
            } else { aim(mob,(mate.x,mate.y,mate.z),0.03); }
            return ai::Plan::default();
        }
    } else { mob.wild.hop=0; }
    if let Some(p)=ctx.players.iter().find(|p|!p.spectator && p.holding.iter().flatten().any(|food|flower(food) || harmful_flower(food).is_some()) && distance(mob,(p.x,p.y,p.z))<100.0) {
        if distance(mob,(p.x,p.y+1.0,p.z))>4.0 { aim(mob,(p.x,p.y+1.0,p.z),0.03); }
        return ai::Plan::default();
    }
    let here=mob.cell();
    let day=ctx.sun_up || view.always_day();
    if let Some(at)=mob.wild.hive && !crate::hive::is_hive(view.block(at.0,at.1,at.2)) { mob.wild.hive=None; }
    if mob.wild.hive.is_none() && mob.ticks.is_multiple_of(40) { mob.wild.hive=seek(mob,view,8,crate::hive::is_hive); }
    if mob.wild.nectar && view.griefing() && mob.wild.crops<10 && rng.below(20)==0 && mob.wild.hive.is_some() {
        for dy in 1..=2 {
            let at=(here.0,here.1-dy,here.2);
            if let Some(state)=crop_growth(view.block(at.0,at.1,at.2)) {
                if blocks::block_at_state(state)==Some("pitcher_crop") && number(state,"age")>=3 {
                    let above=view.block(at.0,at.1+1,at.2);
                    if above!=0 && blocks::block_at_state(above)!=Some("pitcher_crop") {continue;}
                }
                mob.wild.crops+=1;
                return ai::Plan { put:Some((at,state)),..Default::default() };
            }
        }
    }
    if (mob.wild.nectar || !day || view.raining() || view.rain(here.0,here.1,here.2)) && mob.leash.is_none() && mob.wild.cannot_enter==0 && mob.wild.anger==0 && !mob.wild.stung
        && let Some(at)=mob.wild.hive {
            let entrance=crate::hive::entrance(at,view.block(at.0,at.1,at.2));
            aim(mob,entrance,0.035);
            mob.wild.enter=distance(mob,center(at))<2.0;
            return ai::Plan::default();
    }
    if !mob.wild.nectar && day && !view.raining() && !view.rain(here.0,here.1,here.2) && mob.wild.cooldown==0 {
        if mob.wild.flower.is_none() && mob.ticks.is_multiple_of(20) { mob.wild.flower=seek(mob,view,5,pollinatable); }
        if let Some(at)=mob.wild.flower {
            if !pollinatable(view.block(at.0,at.1,at.2)) { mob.wild.flower=None; mob.wild.pollination=0; }
            else {
                let target=(f64::from(at.0)+0.5,f64::from(at.1)+0.8,f64::from(at.2)+0.5);
                aim(mob,target,0.02);
                if distance(mob,target)<0.5 {
                    mob.wild.pollination+=1;
                    if mob.wild.pollination%60==0 { sound(mob,"minecraft:entity.bee.pollinate"); }
                    if mob.wild.pollination>=400 { mob.wild.nectar=true; mob.wild.crops=0; mob.wild.pollination=0; }
                }
                return ai::Plan::default();
            }
        }
    }
    if mob.ticks.is_multiple_of(40) { mob.wild.target=Some((here.0+rng.below(7) as i32-3,here.1+rng.below(3) as i32-1,here.2+rng.below(7) as i32-3)); }
    if let Some(at)=mob.wild.target { aim(mob,center(at),0.015); }
    ai::Plan::default()
}

pub fn base_tick(mob: &mut Mob, view: &dyn View) {
    for (amount,source) in mob.vitals.effects_tick().hurt { mob.hurt(amount,source,None); }
    if mob.is_dead() { return; }
    if mob.kind==Kind::Bee {
        if mob.wild.anger_delay>0 {
            mob.wild.anger_delay-=1;
            if mob.wild.anger_delay==0 && !mob.wild.stung {
                // Вики, Bee: гнев 20–39 с, то есть 400–780 тактов.
                let mut rng=Rng::new(u64::from(mob.entity_id as u32)^mob.ticks.wrapping_mul(0x9E37));
                mob.wild.anger=400+rng.below(381) as i32;
                mob.wild.alert=true;
            }
        }
        if mob.ticks>=3 { mob.wild.nectar_ghost=false; }
        if mob.wild.calm>0 { mob.wild.calm-=1; if mob.wild.calm==0 { mob.wild.chase=false; } }
        if mob.wild.stung {
            mob.wild.death_in-=1;
            if mob.wild.death_in<=0 { mob.vitals.health=0.0; mob.die(); return; }
        }
        let (x,y,z)=mob.cell();
        if super::is_water(view.block(x,y,z)) && mob.ticks.is_multiple_of(20) { mob.hurt(1.0,crate::survival::Source::Drown,None); }
    }
}

pub fn fly(mob: &mut Mob, view: &dyn View) {
    // Вики, Bee: «do not take fall damage». [проверить] разгон и трение.
    mob.vitals.fall=0.0;
    mob.step(view,mob.vx,mob.vy,mob.vz);
    // Трение 0,91, как у летящего: разгон погони до 0,13 за такт с постоянной ~11 тактов
    // (чёрный ящик, bee-sting: скорость к игроку 0,047 → 0,129).
    mob.vx*=0.91; mob.vy*=0.91; mob.vz*=0.91;
}

pub fn sting(mob: &mut Mob) {
    if mob.kind!=Kind::Bee || mob.wild.stung { return; }
    mob.wild.stung=true; mob.wild.anger=0; mob.wild.attacker=None; mob.wild.angry_at=None;
    mob.wild.roll=false; mob.wild.calm=3;
    // Вики, Bee: «within approximately one minute». [проверить] распределение.
    mob.wild.death_in=1200;
    sound(mob,"minecraft:entity.bee.sting");
}

pub fn poison(difficulty: u8) -> Option<crate::survival::Effect> {
    let ticks=match difficulty {2=>200,3=>360,_=>return None};
    Some(crate::survival::Effect { kind:crate::survival::EffectKind::Poison,amplifier:0,ticks_left:ticks,ambient:false,visible:true })
}

pub fn crop_growth(state: i32) -> Option<i32> {
    let name=blocks::block_at_state(state)?;
    if matches!(name,"cave_vines"|"cave_vines_plant") {
        return (blocks::orientation_of(state)?.value(state,"berries")==Some("false")).then(||with(state,"berries","true")).flatten();
    }
    if name=="pitcher_crop" && blocks::orientation_of(state)?.value(state,"half")==Some("upper") {return None;}
    let max=match blocks::block_at_state(state)? { "wheat"|"carrots"|"potatoes"|"melon_stem"|"pumpkin_stem"=>7,"beetroots"|"sweet_berry_bush"=>3,"torchflower_crop"=>1,"pitcher_crop"=>4,_=>return None };
    let age=number(state,"age");
    (age<max).then(||with(state,"age",&(age+1).to_string())).flatten()
}

/// Кувшинница выше двух стадий занимает две клетки (вики, Pitcher Pod).
pub fn crop_upper(at: Cell, state: i32) -> Option<(Cell,i32)> {
    if blocks::block_at_state(state)==Some("pitcher_crop") && number(state,"age")>=3 {
        return with(state,"half","upper").map(|s|((at.0,at.1+1,at.2),s));
    }
    None
}

pub fn write(state: &State, out: &mut Vec<u8>) {
    let data=serde_json::to_vec(state).expect("состояние животного");
    out.extend_from_slice(&(data.len() as u32).to_be_bytes()); out.extend_from_slice(&data);
    out.push(u8::from(state.mouth.is_some()));
    if let Some(stack)=state.mouth { out.extend_from_slice(&stack.stored_item().to_be_bytes()); stack.write_extras(out); }
}
pub fn read(bytes: &mut &[u8]) -> Option<State> {
    fn take<'a>(bytes: &mut &'a [u8], n: usize) -> Option<&'a [u8]> { let (a,b)=bytes.split_at_checked(n)?; *bytes=b; Some(a) }
    let len=u32::from_be_bytes(take(bytes,4)?.try_into().ok()?) as usize;
    if len>65536 { return None; }
    let mut state: State=serde_json::from_slice(take(bytes,len)?).ok()?;
    if take(bytes,1)?[0]!=0 {
        let id=i32::from_be_bytes(take(bytes,4)?.try_into().ok()?);
        let extras=Stack::read_extras(id,|n|take(bytes,n))?;
        let mut stack=Stack::from_stored(id,1);
        stack.data=extras.and_then(crate::stackdata::intern); state.mouth=Some(stack);
    }
    Some(state)
}

pub fn apply_data(mob: &mut Mob, text: &str) {
    if !kind(mob.kind) { return; }
    let Some(data)=crate::snbt::parse(text) else { return; };
    let num=|key|data.get(key).and_then(|v|v.as_f64()).map(|v|v as i32);
    if mob.kind==Kind::Rabbit && let Some(n)=num("RabbitType") {
        if (0..=5).contains(&n) || n==99 { mob.wild.variant=n as u8; }
        if n==99 && mob.name.is_none() { mob.name=Some("The Killer Bunny".into()); }
    }
    if mob.kind==Kind::Fox {
        if let Some(name)=data.get("Type").and_then(|v|v.as_str()) { mob.wild.variant=u8::from(name=="snow"); }
        if let Some(n)=num("Sleeping") { mob.wild.sleeping=n!=0; }
        if let Some(crate::snbt::Value::List(list))=data.get("Trusted") {
            mob.wild.trusted=[None,None];
            for (i,v) in list.iter().take(2).enumerate() {
                if let crate::snbt::Value::List(parts)=v && parts.len()==4 {
                    let mut uuid=[0;16];
                    for (j,p) in parts.iter().enumerate() { uuid[j*4..j*4+4].copy_from_slice(&(p.as_f64().unwrap_or(0.0) as i32).to_be_bytes()); }
                    mob.wild.trusted[i]=Some(uuid);
                }
            }
        }
    }
    if mob.kind==Kind::Bee {
        if let Some(n)=num("HasNectar") { mob.wild.nectar=n!=0; }
        if let Some(n)=num("HasStung") { mob.wild.stung=n!=0; mob.wild.death_in=1200; }
        if let Some(n)=num("AngerTime") { mob.wild.anger=n.max(0); }
        if let Some(crate::snbt::Value::List(parts))=data.get("AngryAt") && parts.len()==4 {
            let mut uuid=[0;16];
            for (j,p) in parts.iter().enumerate() {uuid[j*4..j*4+4].copy_from_slice(&(p.as_f64().unwrap_or(0.0) as i32).to_be_bytes());}
            mob.wild.angry_at=Some(uuid);mob.wild.attacker=None;
        }
        if let Some(n)=num("CropsGrownSincePollination") {mob.wild.crops=n.clamp(0,10) as u8;}
        if let Some(n)=num("CannotEnterHiveTicks") { mob.wild.cannot_enter=n.max(0); }
        for (key,target) in [("hive_pos",&mut mob.wild.hive),("flower_pos",&mut mob.wild.flower)] {
            if let Some(crate::snbt::Value::List(list))=data.get(key) && list.len()==3 {
                *target=Some((list[0].as_f64().unwrap_or(0.0) as i32,list[1].as_f64().unwrap_or(0.0) as i32,list[2].as_f64().unwrap_or(0.0) as i32));
            }
        }
    }
}

pub fn with(state: i32, key: &str, value: &str) -> Option<i32> { blocks::orientation_of(state).and_then(|block| block.with(state,key,value)) }

#[cfg(test)]
#[path = "tests/wildlife.rs"]
mod tests;
