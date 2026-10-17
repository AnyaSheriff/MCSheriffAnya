// Водные мобы Java 26.1.2. Источники: minecraft.wiki/w/{Cod,Salmon,
// Tropical_Fish,Pufferfish,Squid,Glow_Squid,Dolphin,Turtle,Axolotl,Frog,Tadpole}.
// Номера свойств — minecraft-data 26.1 entities.json/protocol.json.
// [проверить] Чёрный ящик по заданию не запускался: выбор пути, вероятности
// целей и порядок сетевых событий ещё не сверены потактово.

pub mod bucket;
pub mod eggs;
pub mod spawn;

use super::{ai, Event, Kind, Mob, Particle, Push, Rng, Source, View};
use crate::{blocks, inventory::Stack, survival::{Effect, EffectKind}};

pub const GROW_TADPOLE: i32 = 24_000;
pub const PLAY_DEAD: i32 = 200;
pub const DRY_AXOLOTL: i32 = 6000;
pub const DOLPHIN_AIR: i32 = 4800;
pub const DOLPHIN_MOISTURE: i32 = 2400;
pub type Sting = (i32, i32, (f64, f64, f64), u8, (f64, f64));

// Вики, Axolotl: пять секунд за помощь, суммарно не более двух минут.
pub fn assist_effect(vitals: &crate::survival::Vitals) -> Effect {
    let old=vitals.effects.iter().find(|effect|effect.kind==EffectKind::Regeneration && effect.amplifier==0);
    Effect {kind:EffectKind::Regeneration,amplifier:0,ticks_left:(old.map_or(0,|effect|effect.ticks_left)+100).min(2400),ambient:false,visible:true, show_icon:true}
}

#[derive(Clone, Debug)]
pub struct State {
    pub variant: i32,
    pub salmon_size: u8,
    pub from_bucket: bool,
    pub wet: bool,
    pub air: i32,
    pub moisture: i32,
    pub puff: u8,
    pub puff_ticks: i32,
    pub calm_ticks: i32,
    pub dark_ticks: i32,
    pub playing_dead: i32,
    pub growth: i32,
    pub home: (i32, i32, i32),
    pub has_egg: bool,
    pub laying: i32,
    pub got_fish: bool,
    pub swim_to: Option<(f64, f64, f64)>,
    pub target: Option<i32>,
    pub attack_cooldown: i32,
    pub hunt_cooldown: i32,
    pub tongue: i32,
    pub anger: i32,
    pub alert: bool,
    pub mate_ticks: i32,
    pub last_attacker: Option<i32>,
    pub egg_target: Option<(i32,i32,i32)>,
    pub egg_ticks: i32,
    pub boat: Option<(f64, f64, f64)>,
    pub panic_ticks: i32,
    // Щупальца кальмара: фаза и скорость; на обороте уходит событие 19 (чёрный ящик squid-ink).
    pub tentacle: f32,
    pub tentacle_speed: f32,
    // Лягушка: сколько тактов цель в виду и сколько нужно до языка (чёрный ящик frog-eat-slime).
    pub lock_ticks: i32,
    pub lock_need: i32,
    // Аксолотль: притворство начинается не в такте удара, а через такт-два (чёрный ящик axolotl-play-dead).
    pub dead_pending: i32,
}

impl State {
    pub fn new(kind: Kind, at: (f64, f64, f64)) -> Self {
        Self {
            variant: 0, salmon_size: 1, from_bucket: false, wet: false,
            air: if kind == Kind::Dolphin { DOLPHIN_AIR } else { 300 },
            moisture: if kind == Kind::Axolotl { DRY_AXOLOTL } else { DOLPHIN_MOISTURE },
            puff: 0, puff_ticks: 0, calm_ticks: 0, dark_ticks: 0,
            playing_dead: 0, growth: 0,
            home: (at.0.floor() as i32, at.1.floor() as i32, at.2.floor() as i32),
            has_egg: false, laying: 0, got_fish: false, swim_to: None,
            target: None, attack_cooldown: 0, hunt_cooldown: 0, tongue: 0,
            anger: 0, alert: false, mate_ticks: 0, last_attacker: None, egg_target: None, egg_ticks: 0,
            boat: None, panic_ticks: 0, tentacle: 0.0, tentacle_speed: 0.0, lock_ticks: 0, lock_need: 0, dead_pending: 0,
        }
    }
}

pub fn fish(kind: Kind) -> bool {
    matches!(kind, Kind::Cod | Kind::Salmon | Kind::TropicalFish | Kind::Pufferfish)
}
pub fn creature(kind: Kind) -> bool {
    fish(kind) || matches!(kind, Kind::Squid | Kind::GlowSquid | Kind::Dolphin | Kind::Turtle | Kind::Axolotl | Kind::Frog | Kind::Tadpole)
}
pub fn bucket(kind: Kind) -> Option<&'static str> {
    Some(match kind {
        Kind::Cod => "cod_bucket", Kind::Salmon => "salmon_bucket",
        Kind::TropicalFish => "tropical_fish_bucket", Kind::Pufferfish => "pufferfish_bucket",
        Kind::Axolotl => "axolotl_bucket", Kind::Tadpole => "tadpole_bucket", _ => return None,
    })
}
pub fn bucket_kind(item: &str) -> Option<Kind> {
    let kind = Kind::from_name(item.strip_suffix("_bucket")?)?;
    bucket(kind).map(|_| kind)
}

// Вики, Tropical Fish: 2 формы × 6 узоров × 16 цветов тела × 16 цветов узора.
pub fn pack_variant(shape: u8, pattern: u8, base: u8, overlay: u8) -> Option<i32> {
    (shape < 2 && pattern < 6 && base < 16 && overlay < 16)
        .then_some(i32::from(shape) | (i32::from(pattern) << 8) | (i32::from(base) << 16) | (i32::from(overlay) << 24))
}
pub fn unpack_variant(value: i32) -> Option<(u8, u8, u8, u8)> {
    let parts = (value as u8, (value >> 8) as u8, (value >> 16) as u8, (value >> 24) as u8);
    pack_variant(parts.0, parts.1, parts.2, parts.3).map(|_| parts)
}

// Вики, Tropical Fish, «Common varieties»: 90 % — один из 22 вариантов.
// fr.minecraft.wiki/w/Poisson_tropical, таблица «Données d’entité» (исправления 1.13.1).
pub const COMMON: [(&str, i32); 22] = [
    ("Anemone", 117506305), ("Black Tang", 117899265), ("Blue Dory", 185008129),
    ("Butterflyfish", 117441793), ("Cichlid", 118161664), ("Clownfish", 65536),
    ("Cotton Candy Betta", 50726144), ("Dottyback", 67764993),
    ("Emperor Red Snapper", 234882305), ("Goatfish", 67110144),
    ("Moorish Idol", 117441025), ("Ornate Butterflyfish", 16778497),
    ("Parrotfish", 101253888), ("Queen Angelfish", 50660352),
    ("Red Cichlid", 918529), ("Red Lipped Blenny", 235340288),
    ("Red Snapper", 918273), ("Threadfin", 67108865),
    ("Tomato Clownfish", 917504), ("Triggerfish", 459008),
    ("Yellowtail Parrotfish", 67699456), ("Yellow Tang", 67371009),
];

pub fn initialize(mob: &mut Mob, biome: &str, rng: &mut Rng) {
    match mob.kind {
        Kind::TropicalFish => {
            mob.water.variant = if rng.below(10) < 9 { COMMON[rng.below(22) as usize].1 }
                else { pack_variant(rng.below(2) as u8, rng.below(6) as u8, rng.below(16) as u8, rng.below(16) as u8).unwrap() };
        }
        Kind::Salmon => mob.water.salmon_size = match rng.below(4) { 0 => 0, 1 => 1, _ => 2 },
        Kind::Axolotl => mob.water.variant = rng.below(4) as i32,
        Kind::Frog => mob.water.variant = frog_variant(biome),
        _ => {}
    }
}

// Вики, Frog: вариант задаёт биом в момент превращения головастика.
pub fn frog_variant(biome: &str) -> i32 {
    match biome {
        "desert" | "badlands" | "eroded_badlands" | "wooded_badlands" | "savanna" | "savanna_plateau" | "windswept_savanna" | "jungle" | "bamboo_jungle" | "sparse_jungle" | "mangrove_swamp" | "warm_ocean" | "nether_wastes" | "soul_sand_valley" | "crimson_forest" | "warped_forest" | "basalt_deltas" => 1,
        "snowy_plains" | "ice_spikes" | "snowy_taiga" | "snowy_beach" | "frozen_river" | "frozen_ocean" | "deep_frozen_ocean" | "grove" | "snowy_slopes" | "frozen_peaks" | "jagged_peaks" | "stony_peaks" | "windswept_hills" | "windswept_forest" | "windswept_gravelly_hills" | "deep_dark" | "the_end" | "end_highlands" | "end_midlands" | "small_end_islands" | "end_barrens" => 2,
        _ => 0,
    }
}
pub fn froglight(variant: i32) -> &'static str {
    match variant { 1 => "pearlescent_froglight", 2 => "verdant_froglight", _ => "ochre_froglight" }
}
pub fn size(mob: &Mob) -> Option<(f64, f64)> {
    let scale = match mob.kind {
        Kind::Salmon => [0.5, 1.0, 1.5][usize::from(mob.water.salmon_size.min(2))],
        Kind::Pufferfish => [0.5, 0.7, 1.0][usize::from(mob.water.puff.min(2))],
        Kind::Turtle if mob.is_baby() => 0.3,
        Kind::Tadpole => 1.0,
        _ => return None,
    };
    Some((mob.kind.width() * scale, mob.kind.height() * scale))
}
pub fn sense(mob: &mut Mob, view: &dyn View) {
    let (x,y,z) = mob.cell();
    mob.water.wet = super::is_water(view.block(x,y,z));
}

pub fn vitals(mob: &mut Mob, view: &dyn View) {
    let wet = mob.water.wet;
    let (x,y,z) = mob.cell();
    let raining = view.rain(x,y,z);
    // Рост — возраст, а не цель ИИ: NoAI не останавливает превращение.
    if mob.kind == Kind::Tadpole {
        mob.water.growth = mob.water.growth.saturating_add(1).min(GROW_TADPOLE);
        if mob.water.growth >= GROW_TADPOLE { mob.h.converted = Some(Kind::Frog); }
    }
    if fish(mob.kind) || matches!(mob.kind, Kind::Squid | Kind::GlowSquid | Kind::Tadpole) {
        if wet { mob.water.air = 300; }
        else {
            mob.water.air -= 1;
            if mob.water.air <= -20 { mob.water.air = 0; mob.hurt(2.0, Source::Drown, None); }
        }
    } else if mob.kind == Kind::Axolotl {
        if wet || raining { mob.water.moisture = DRY_AXOLOTL; }
        else {
            mob.water.moisture -= 1;
            if mob.water.moisture <= -20 { mob.water.moisture = 0; mob.hurt(2.0, Source::DryOut, None); }
        }
    } else if mob.kind == Kind::Dolphin && !mob.no_ai {
        if super::is_water(view.block(x, (mob.y + mob.eye()).floor() as i32, z)) {
            mob.water.air -= 1;
            if mob.water.air <= -20 { mob.water.air = 0; mob.hurt(2.0, Source::Drown, None); }
        } else { mob.water.air = DOLPHIN_AIR; }
        if wet || raining { mob.water.moisture = DOLPHIN_MOISTURE; }
        else {
            mob.water.moisture -= 1;
            if mob.water.moisture <= 0 { mob.hurt(1.0, Source::DryOut, None); }
        }
    }
    // Кальмар: щупальца крутятся в воде; на каждом обороте — событие сущности 19,
    // скорость 1/(r+1)·0,2 берётся при появлении и с шансом 1/10 на обороте
    // (чёрный ящик squid-ink: обороты каждые 54–59 тактов, фаза с нуля у NoAI тоже).
    if matches!(mob.kind,Kind::Squid | Kind::GlowSquid) && wet {
        let mut rng = Rng::new(u64::from(mob.entity_id as u32).wrapping_mul(0x9E37_79B9) ^ mob.ticks.wrapping_mul(0x85EB_CA6B));
        if mob.water.tentacle_speed == 0.0 { mob.water.tentacle_speed = 1.0 / (rng.unit() as f32 + 1.0) * 0.2; }
        mob.water.tentacle += mob.water.tentacle_speed;
        if mob.water.tentacle > std::f32::consts::TAU {
            mob.water.tentacle -= std::f32::consts::TAU;
            if rng.below(10) == 0 { mob.water.tentacle_speed = 1.0 / (rng.unit() as f32 + 1.0) * 0.2; }
            mob.events.push(Event::Status(19));
        }
    }
    mob.vitals.air = mob.water.air;
    mob.water.dark_ticks = (mob.water.dark_ticks - 1).max(0);
    mob.water.attack_cooldown = (mob.water.attack_cooldown - 1).max(0);
    mob.water.hunt_cooldown = (mob.water.hunt_cooldown - 1).max(0);
    if mob.water.dead_pending > 0 {
        mob.water.dead_pending -= 1;
        // Метка — через такт после удара, Регенерация (с частицами) — ещё через такт.
        if mob.water.dead_pending == 1 { mob.water.playing_dead = PLAY_DEAD + 1; }
        if mob.water.dead_pending == 0 {
            mob.vitals.give_effect_shown(Effect { kind: EffectKind::Regeneration, amplifier: 0, ticks_left: PLAY_DEAD, ambient: false, visible: true, show_icon: true });
        }
    }
    if mob.water.playing_dead > 0 { mob.water.playing_dead -= 1; }
    if mob.water.anger > 0 { mob.water.anger -= 1; }
    mob.water.panic_ticks = (mob.water.panic_ticks - 1).max(0);
    for (amount, source) in mob.vitals.effects_tick().hurt { mob.hurt(amount, source, None); }
}

pub fn on_hurt(mob: &mut Mob, source: Source, push: Option<Push>, damage: f32, died: bool) {
    if !died && !mob.no_ai && (fish(mob.kind) || matches!(mob.kind,Kind::Squid | Kind::GlowSquid | Kind::Turtle | Kind::Tadpole)) {
        mob.water.panic_ticks = super::PANIC_TICKS;
        mob.water.swim_to = None;
    }
    if matches!(mob.kind, Kind::Squid | Kind::GlowSquid) {
        mob.water.dark_ticks = if mob.kind == Kind::GlowSquid { 100 } else { 0 };
        // 30 пакетов по одной частице: вниз от тела, разброс ±0,3 по бокам,
        // длина 0,3…2,3, скорость 0,1 при числе 0 (чёрный ящик squid-ink).
        let mut ink = Rng::new(mob.ticks.wrapping_mul(0xC2B2_AE35) ^ u64::from(mob.entity_id as u32));
        for _ in 0..30 {
            let (rx, rz) = (ink.unit() as f32 * 0.6 - 0.3, ink.unit() as f32 * 0.6 - 0.3);
            let length = 0.3 + ink.unit() as f32 * 2.0;
            mob.events.push(Event::Particle {
                kind: if mob.kind == Kind::GlowSquid { Particle::GlowSquidInk } else { Particle::SquidInk },
                at: (mob.x, mob.y - 0.5, mob.z), offset: (rx * length, -length, rz * length), speed: 0.1, count: 0,
            });
        }
        mob.events.push(Event::Sound { name: if mob.kind == Kind::GlowSquid { "minecraft:entity.glow_squid.squirt" } else { "minecraft:entity.squid.squirt" }, volume: 0.4, pitch: 1.0 });
    }
    let mut rng = Rng::new(mob.ticks.wrapping_mul(0x9e37) ^ mob.entity_id as u64 ^ damage.to_bits() as u64);
    // Притворяется мёртвым случайно: 1/3, и при этом либо до удара было меньше половины
    // здоровья, либо случайное 0…2 меньше урона. Двенадцать записей оригинала axolotl-play-dead:
    // первый удар (8→6 из 14) 1 раз из 12, второй (6→4) 6 раз из 12 — не «всегда» [проверить].
    if mob.kind == Kind::Axolotl && !died && !mob.no_ai && mob.water.wet && mob.water.playing_dead == 0 && mob.water.dead_pending == 0
        && matches!(source, Source::MobAttack | Source::PlayerAttack | Source::Arrow | Source::Trident | Source::IndirectMagic)
        && (rng.below(3) == 0 && (mob.vitals.health + damage < mob.vitals.max_health / 2.0 || damage > rng.below(3) as f32))
    {
        mob.water.dead_pending = 3;
        mob.water.swim_to = None;
    }
    if mob.kind == Kind::Dolphin && !mob.no_ai && let Some(push) = push.filter(|p| p.cause != 0) {
        mob.water.target = Some(push.cause);
        mob.water.anger = 400;
        mob.water.alert = true;
    }
}

pub fn scute(mob: &mut Mob) {
    if let Some(item) = blocks::item_named("turtle_scute") { mob.events.push(Event::Loot(vec![Stack::new(item,1)])); }
}
fn distance(mob: &Mob, at: (f64,f64,f64)) -> f64 {
    (mob.x-at.0).powi(2)+(mob.y-at.1).powi(2)+(mob.z-at.2).powi(2)
}
fn near_box(mob: &Mob,at: (f64,f64,f64),size: (f64,f64),margin: f64) -> bool {
    let own=mob.size();
    (mob.x-at.0).abs()<(own.0+size.0)*0.5+margin
        && (mob.z-at.2).abs()<(own.0+size.0)*0.5+margin
        && mob.y<at.1+size.1+margin && at.1<mob.y+own.1+margin
}
fn water_goal(mob: &Mob, view: &dyn View, rng: &mut Rng) -> Option<(f64,f64,f64)> {
    for _ in 0..8 {
        let at = (mob.x + f64::from(rng.below(13) as i32-6),mob.y + f64::from(rng.below(7) as i32-3),mob.z + f64::from(rng.below(13) as i32-6));
        if super::is_water(view.block(at.0.floor() as i32,at.1.floor() as i32,at.2.floor() as i32)) { return Some(at); }
    }
    None
}

// Вики, Axolotl: «seek out water». Путь использует общий поиск пассивных.
// [проверить] Радиус и частота поиска, а также обход береговых препятствий.
fn near_water(mob: &Mob, view: &dyn View) -> Option<(f64,f64,f64)> {
    let (x,y,z)=mob.cell();
    let mut best=None;
    let mut best_dist=i32::MAX;
    for dx in -8..=8 { for dz in -8..=8 { for dy in -3..=2 {
        let dist=dx*dx+dy*dy+dz*dz;
        if dist<best_dist && super::is_water(view.block(x+dx,y+dy,z+dz)) {
            best=Some((f64::from(x+dx)+0.5,f64::from(y+dy),f64::from(z+dz)+0.5));
            best_dist=dist;
        }
    }}}
    best
}

fn walk_to(mob: &mut Mob,view: &dyn View,at: (f64,f64,f64),speed: f64) -> ai::Plan {
    if !mob.brain.walking() || mob.ticks.is_multiple_of(20) {
        let goal=(at.0.floor() as i32,at.1.floor() as i32,at.2.floor() as i32);
        let route=super::path::find_for(view,mob.cell(),goal,super::path::MAX_NODES,mob.kind);
        mob.brain.go(route,speed);
    }
    ai::walk(mob,ai::Plan::default())
}

pub fn think(mob: &mut Mob, view: &dyn View, ctx: &ai::Ctx, rng: &mut Rng) -> ai::Plan {
    let mut plan = ai::Plan::default();
    if mob.water.playing_dead > 0 || mob.water.tongue > 0 { return plan; }
    if mob.kind == Kind::Pufferfish {
        let threat = ctx.players.iter().any(|p| p.targetable && !p.spectator && near_box(mob,(p.x,p.y,p.z),(0.6,1.8),2.0))
            || ctx.mates.iter().any(|m| m.id != mob.entity_id && (!creature(m.kind) || matches!(m.kind,Kind::Axolotl | Kind::Frog)) && !super::guardian::guardian(m.kind)
                && near_box(mob,(m.x,m.y,m.z),(m.kind.width(),m.kind.height()),2.0));
        let before = mob.water.puff;
        if threat {
            mob.water.calm_ticks = 0;
            mob.water.puff_ticks += 1;
            mob.water.puff = if mob.water.puff_ticks > 40 { 2 } else { 1 };
        } else {
            mob.water.puff_ticks = 0;
            mob.water.calm_ticks += 1;
            if mob.water.calm_ticks > 100 { mob.water.puff = 0; }
            else if mob.water.calm_ticks > 60 { mob.water.puff = mob.water.puff.min(1); }
        }
        if before != mob.water.puff {
            mob.events.push(Event::Sound { name: if before < mob.water.puff { "minecraft:entity.puffer_fish.blow_up" } else { "minecraft:entity.puffer_fish.blow_out" }, volume: if before<mob.water.puff {0.45} else {0.7}, pitch: 0.8+rng.unit() as f32*0.4 });
        }
    }
    if mob.water.wet && mob.water.panic_ticks > 0 {
        if mob.water.swim_to.is_none_or(|at|distance(mob,at)<0.5) || mob.ticks.is_multiple_of(20) {
            mob.water.swim_to=water_goal(mob,view,rng);
        }
        return plan;
    }
    if mob.kind == Kind::Dolphin {
        if mob.water.air < 140 && mob.water.wet {
            mob.water.swim_to = Some((mob.x,mob.y+2.0,mob.z));
            return plan;
        }
        if mob.water.anger > 0 && let Some(p) = ctx.players.iter().find(|p| Some(p.id) == mob.water.target && p.targetable) {
            mob.water.swim_to = Some((p.x,p.y,p.z));
            if distance(mob,(p.x,p.y,p.z)) < 2.0 && mob.water.attack_cooldown == 0 {
                let len = (p.x-mob.x).hypot(p.z-mob.z).max(1e-6);
                plan.melee = Some(super::hostile::Melee { target: p.id, damage: 3.0, push: ((p.x-mob.x)/len,(p.z-mob.z)/len), wither: false, hunger: false });
                mob.water.attack_cooldown = 20;
            }
            return plan;
        }
        if mob.water.anger > 0 && let Some(mate)=ctx.mates.iter().find(|m|Some(m.id)==mob.water.target) {
            mob.water.swim_to=Some((mate.x,mate.y,mate.z));
            if distance(mob,(mate.x,mate.y,mate.z))<2.0 && mob.water.attack_cooldown==0 {
                let dx=mate.x-mob.x; let dz=mate.z-mob.z; let len=dx.hypot(dz).max(1e-6);
                plan.bite=Some((mate.id,3.0,(dx/len,dz/len)));
                mob.water.attack_cooldown=20;
            }
            return plan;
        }
        mob.water.target = None;
        if let Some(at)=mob.water.boat {
            mob.water.swim_to=Some(at);
            return plan;
        }
        if let Some(p) = ctx.players.iter().filter(|p| !p.spectator && super::is_water(view.block(p.x.floor() as i32,p.y.floor() as i32,p.z.floor() as i32)))
            .filter(|p| distance(mob,(p.x,p.y,p.z)) < 100.0).min_by(|a,b| distance(mob,(a.x,a.y,a.z)).total_cmp(&distance(mob,(b.x,b.y,b.z)))) {
            mob.water.swim_to = Some((p.x,p.y,p.z));
            return plan;
        }
    }
    if mob.kind == Kind::Turtle && mob.water.has_egg {
        let (x,y,z) = mob.cell();
        let home = mob.water.home;
        if (x-home.0).abs() <= 9 && (z-home.2).abs() <= 9 && !mob.water.wet
            && matches!(blocks::block_at_state(view.block(x,y-1,z)),Some("sand" | "red_sand")) && view.block(x,y,z) == crate::world::AIR {
            mob.water.laying += 1;
            if mob.water.laying >= 200 {
                let count = 1+rng.below(4);
                if let Some(state) = blocks::state_from_text(&format!("turtle_egg[eggs={count},hatch=0]")) {
                    plan.put = Some(((x,y,z),state));
                    mob.water.has_egg = false;
                    mob.water.laying = 0;
                    mob.events.push(Event::Sound { name: "minecraft:entity.turtle.lay_egg", volume: 0.3, pitch: 1.0 });
                }
            }
            return plan;
        }
        mob.water.laying = 0;
        mob.water.swim_to = Some((f64::from(home.0)+0.5,f64::from(home.1),f64::from(home.2)+0.5));
        if mob.water.wet { return plan; }
        return walk_to(mob,view,mob.water.swim_to.unwrap(),0.25);
    }
    if mob.kind == Kind::Frog && mob.water.has_egg {
        let (x,y,z) = mob.cell();
        for (dx,dz) in [(0,0),(1,0),(-1,0),(0,1),(0,-1)] {
            if eggs::supports_frogspawn(view.block(x+dx,y-1,z+dz)) && view.block(x+dx,y,z+dz) == crate::world::AIR {
                if let Some(state) = blocks::state_by_name("frogspawn") { plan.put = Some(((x+dx,y,z+dz),state)); mob.water.has_egg=false; }
                return plan;
            }
        }
    }
    if mob.water.wet && matches!(mob.kind, Kind::Turtle | Kind::Axolotl | Kind::Frog) && mob.love > 0 && mob.age == 0 {
        if let Some(mate) = ctx.mates.iter().filter(|m| m.id != mob.entity_id && m.kind == mob.kind && m.love > 0 && m.ready)
            .find(|m| distance(mob,(m.x,m.y,m.z)) < 64.0) {
            mob.water.swim_to = Some((mate.x,mate.y,mate.z));
            if distance(mob,(mate.x,mate.y,mate.z)) < 9.0 {
                mob.water.mate_ticks += 1;
                if mob.water.mate_ticks >= 60 { plan.breed=Some(mate.id); mob.water.mate_ticks=0; }
            } else { mob.water.mate_ticks=0; }
            return plan;
        } else { mob.water.mate_ticks=0; }
    }
    if !mob.water.wet && matches!(mob.kind, Kind::Turtle | Kind::Axolotl | Kind::Frog) {
        let tempted=ctx.players.iter().any(|p| !p.spectator && distance(mob,(p.x,p.y,p.z))<100.0
            && p.holding.iter().flatten().any(|food| mob.kind.likes(food)));
        if !tempted && mob.love == 0 && (matches!(mob.kind,Kind::Turtle | Kind::Axolotl) || mob.water.has_egg) {
            if mob.ticks.is_multiple_of(20) || mob.water.swim_to.is_none() { mob.water.swim_to=near_water(mob,view); }
            if let Some(at)=mob.water.swim_to { return walk_to(mob,view,at,if mob.kind==Kind::Turtle {0.25} else {0.3}); }
        }
        let mut land = ai::think(mob,view,ctx,rng);
        if mob.kind == Kind::Turtle { land.speed *= 0.25; }
        else {land.speed *= 0.2;}
        // Лягушка по земле ходит, не прыгает: за 400 тактов оригинал не дал ни одного подскока
        // (чёрный ящик frog-eat-slime); длинный прыжок к цели — [проверить].
        if let Some(target) = mob.water.swim_to.filter(|_| mob.water.target.is_some() || mob.water.has_egg) {
            let dx=target.0-mob.x; let dz=target.2-mob.z;
            mob.face((-dx.atan2(dz).to_degrees()) as f32);
            land=walk_to(mob,view,target,0.3);
        }
        return land;
    }
    ai::voice(mob,rng);
    // Стаи: треска до 9, лосось до 7, тропические до 8. [проверить] Выбор лидера.
    if matches!(mob.kind, Kind::Cod | Kind::Salmon | Kind::TropicalFish) {
        let limit=if mob.kind==Kind::Cod {9} else if mob.kind==Kind::Salmon {7} else {8};
        let school: Vec<_> = ctx.mates.iter().filter(|m| m.kind==mob.kind && distance(mob,(m.x,m.y,m.z)) < 64.0).collect();
        if let Some(leader)=school.iter().min_by_key(|m| m.id).filter(|m| m.id != mob.entity_id && school.iter().filter(|s| s.id < mob.entity_id).count() < limit) {
            mob.water.swim_to=if distance(mob,(leader.x,leader.y,leader.z)) > 1.0 {Some((leader.x,leader.y,leader.z))} else {None};
            return plan;
        }
    }
    if mob.water.target.is_some() { return plan; }
    if let Some(p) = ctx.players.iter().find(|p| !p.spectator && p.holding.iter().flatten().any(|food| mob.kind.likes(food)) && distance(mob,(p.x,p.y,p.z)) < 100.0) {
        mob.water.swim_to=Some((p.x,p.y,p.z));
    } else if mob.water.swim_to.is_none_or(|at| distance(mob,at)<0.5) || rng.below(100)==0 {
        mob.water.swim_to=water_goal(mob,view,rng);
    }
    plan
}

pub fn physics(mob: &mut Mob, view: &dyn View, plan: &ai::Plan, rng: &mut Rng) {
    if mob.water.playing_dead > 0 { mob.vx=0.0; mob.vy=0.0; mob.vz=0.0; return; }
    if mob.water.tongue > 0 {
        mob.vx=0.0; mob.vz=0.0;
        mob.physics(view,0.0,false);
        return;
    }
    if mob.water.wet {
        if let Some((x,y,z))=mob.water.swim_to {
            let (dx,dy,dz)=(x-mob.x,y-mob.y,z-mob.z);
            let length=(dx*dx+dy*dy+dz*dz).sqrt().max(1e-6);
            mob.face((-dx.atan2(dz).to_degrees()) as f32);
            let accel=if mob.kind==Kind::Dolphin {0.025} else if mob.water.panic_ticks>0 {0.02} else {0.01};
            mob.vx+=dx/length*accel; mob.vy+=dy/length*accel; mob.vz+=dz/length*accel;
        }
        mob.vitals.fall=0.0;
        mob.step(view,mob.vx,mob.vy,mob.vz);
        mob.vx*=0.9; mob.vy*=0.9; mob.vz*=0.9;
    } else {
        if mob.on_ground && (fish(mob.kind) || matches!(mob.kind,Kind::Tadpole | Kind::Dolphin)) {
            mob.vx+=(rng.unit()*2.0-1.0)*0.05; mob.vz+=(rng.unit()*2.0-1.0)*0.05; mob.vy=0.4;
            mob.on_ground=false;
            let name=match mob.kind { Kind::Cod=>"minecraft:entity.cod.flop",Kind::Salmon=>"minecraft:entity.salmon.flop",Kind::TropicalFish=>"minecraft:entity.tropical_fish.flop",Kind::Pufferfish=>"minecraft:entity.puffer_fish.flop",Kind::Dolphin=>"minecraft:entity.dolphin.flop",_=>"minecraft:entity.tadpole.flop" };
            mob.events.push(Event::Sound{name,volume:0.3,pitch:1.0});
        }
        mob.physics(view,plan.speed,plan.jump);
    }
}

pub fn loot(mob: &Mob,rng: &mut Rng,give: &mut dyn FnMut(&str,i32,i32,&mut Rng)) {
    let cooked=mob.vitals.fire>0;
    match mob.kind {
        Kind::Cod=>give(if cooked {"cooked_cod"} else {"cod"},1,1,rng),
        Kind::Salmon=>give(if cooked {"cooked_salmon"} else {"salmon"},1,1,rng),
        Kind::TropicalFish=>give("tropical_fish",1,1,rng), Kind::Pufferfish=>give("pufferfish",1,1,rng),
        Kind::Squid=>give("ink_sac",1,3,rng), Kind::GlowSquid=>give("glow_ink_sac",1,3,rng),
        Kind::Dolphin=>give(if cooked {"cooked_cod"} else {"cod"},0,1,rng),
        Kind::Turtle=>give("seagrass",0,2,rng), _=>{}
    }
    if fish(mob.kind) && rng.below(20)==0 { give("bone_meal",1,1,rng); }
}

// Хвост новых видов: фиксированная версия, старых записей этих видов нет.
pub fn write(state: &State,out: &mut Vec<u8>) {
    for n in [state.variant,i32::from(state.salmon_size),state.air,state.moisture,state.growth,state.home.0,state.home.1,state.home.2,i32::from(state.puff),state.dark_ticks,state.hunt_cooldown] { out.extend_from_slice(&n.to_be_bytes()); }
    out.push(u8::from(state.from_bucket) | (u8::from(state.has_egg)<<1) | (u8::from(state.got_fish)<<2));
}
pub fn read(bytes: &mut &[u8]) -> Option<State> {
    let (head,tail)=bytes.split_at_checked(45)?;
    let mut values=head[..44].as_chunks::<4>().0.iter().map(|v| i32::from_be_bytes(*v));
    let mut state=State::new(Kind::Cod,(0.0,0.0,0.0));
    state.variant=values.next()?; state.salmon_size=values.next()?.clamp(0,2) as u8;
    state.air=values.next()?; state.moisture=values.next()?; state.growth=values.next()?.clamp(0,GROW_TADPOLE);
    state.home=(values.next()?,values.next()?,values.next()?); state.puff=values.next()?.clamp(0,2) as u8;
    state.dark_ticks=values.next()?.max(0); state.hunt_cooldown=values.next()?.max(0);
    state.from_bucket=head[44]&1!=0; state.has_egg=head[44]&2!=0; state.got_fish=head[44]&4!=0;
    *bytes=tail; Some(state)
}

pub fn apply_data(mob: &mut Mob,text: &str) {
    if !creature(mob.kind) { return; }
    let Some(data)=crate::snbt::parse(text) else { return; };
    let num=|key| data.get(key).and_then(crate::snbt::Value::as_f64).map(|v| v as i32);
    let string=|key| data.get(key).and_then(crate::snbt::Value::as_str).map(|s| s.strip_prefix("minecraft:").unwrap_or(s));
    if let Some(air)=num("Air") { mob.water.air=air; mob.vitals.air=air; }
    if let Some(bucket)=num("FromBucket") { mob.water.from_bucket=bucket!=0; mob.persistent_tag|=bucket!=0; }
    if let Some(health)=data.get("Health").and_then(crate::snbt::Value::as_f64).filter(|v| v.is_finite()) { mob.vitals.health=(health as f32).clamp(0.0,mob.vitals.max_health); }
    match mob.kind {
        Kind::TropicalFish => if let Some(v)=num("Variant").filter(|v| unpack_variant(*v).is_some()) { mob.water.variant=v; },
        Kind::Pufferfish => if let Some(puff)=num("PuffState") { mob.water.puff=puff.clamp(0,2) as u8; },
        Kind::Salmon => {
            if let Some(size)=string("type").or_else(||string("Size")).or_else(||string("size")) {
                mob.water.salmon_size=match size { "small"=>0,"large"=>2,_=>1 };
            }
        }
        Kind::GlowSquid => if let Some(ticks)=num("DarkTicksRemaining") { mob.water.dark_ticks=ticks.max(0); },
        Kind::Axolotl => {
            if let Some(v)=num("Variant") { mob.water.variant=v.clamp(0,4); }
            if let Some(v)=string("variant").and_then(|s| ["lucy","wild","gold","cyan","blue"].iter().position(|v| *v==s)) { mob.water.variant=v as i32; }
        }
        Kind::Turtle => {
            if let Some(v)=num("HasEgg") { mob.water.has_egg=v!=0; }
            if let (Some(x),Some(y),Some(z))=(num("HomePosX"),num("HomePosY"),num("HomePosZ")) { mob.water.home=(x,y,z); }
        }
        Kind::Tadpole => if let Some(age)=num("Age") { mob.water.growth=age.clamp(0,GROW_TADPOLE); },
        Kind::Frog => if let Some(v)=string("variant").and_then(|s| ["temperate","warm","cold"].iter().position(|v| *v==s)) { mob.water.variant=v as i32; },
        _=>{}
    }
}

/// Контакт и охота: общий список позволяет учитывать реальный размер жертвы.
pub fn interactions(mobs: &mut [Mob],view: &dyn View,players: &[ai::Watcher],outcome: &mut super::Outcome,rng: &mut Rng) {
    let alerts: Vec<_>=mobs.iter().filter(|m| m.kind==Kind::Dolphin && m.water.alert && !m.is_dead()).map(|m| ((m.x,m.y,m.z),m.water.target)).collect();
    for mob in mobs.iter_mut().filter(|m| m.kind==Kind::Dolphin) {
        mob.water.alert=false;
        for (at,target) in &alerts {
            if distance(mob,*at)<256.0 && !mob.no_ai { mob.water.target = *target; mob.water.anger=400; }
        }
    }
    for i in 0..mobs.len() {
        if mobs[i].is_dead() { continue; }
        let kind=mobs[i].kind;
        let at=(mobs[i].x,mobs[i].y,mobs[i].z);
        if kind==Kind::Pufferfish && mobs[i].water.puff>0 {
            let puff=mobs[i].water.puff;
            let radius=mobs[i].size().0/2.0;
            for p in players.iter().filter(|p| p.targetable && !p.spectator) {
                if (p.x-at.0).abs()<radius+0.6 && (p.z-at.2).abs()<radius+0.6 && p.y<at.1+mobs[i].size().1 && p.y+1.8>at.1 {
                    // Отбрасывание от рыбы к игроку единичным вектором (чёрный ящик pufferfish-sting).
                    let (dx,dz)=(p.x-at.0,p.z-at.2);
                    let len=(dx*dx+dz*dz).sqrt();
                    let push=if len>1.0e-4 {(dx/len,dz/len)} else {(0.0,0.0)};
                    outcome.stings.push((p.id,mobs[i].entity_id,at,puff,push));
                }
            }
            let attacker=mobs[i].entity_id;
            let mut stung=false;
            for victim in mobs.iter_mut().filter(|m| !m.is_dead() && (!creature(m.kind) || matches!(m.kind,Kind::Axolotl | Kind::Frog)) && !super::guardian::guardian(m.kind)) {
                if near_box(victim,at,(radius*2.0,radius*2.0),0.1)
                    && victim.hurt(f32::from(1+puff),Source::MobAttack,Some(Push{dir:(0.0,0.0),strength:0.0,cause:attacker})).is_some()
                {
                    stung=true;
                    if !matches!(victim.kind,Kind::Zombie | Kind::Drowned | Kind::Husk | Kind::ZombieVillager | Kind::Skeleton | Kind::WitherSkeleton | Kind::ZombifiedPiglin) {
                        victim.vitals.give_effect_shown(Effect{kind:EffectKind::Poison,amplifier:0,ticks_left:60*i32::from(puff),ambient:false,visible:true, show_icon:true});
                    }
                }
            }
            if stung {mobs[i].events.push(Event::Sound{name:"minecraft:entity.puffer_fish.sting",volume:1.0,pitch:1.0});}
        }
        if kind==Kind::Dolphin && !mobs[i].no_ai && mobs[i].water.wet && mobs[i].water.anger==0 {
            for player in players.iter().filter(|p| !p.spectator && distance(&mobs[i],(p.x,p.y,p.z))<81.0 && super::is_water(view.block(p.x.floor() as i32,p.y.floor() as i32,p.z.floor() as i32))) {
                outcome.water_effects.push((player.id,Effect{kind:EffectKind::DolphinsGrace,amplifier:0,ticks_left:100,ambient:false,visible:true, show_icon:true}));
            }
        }
        if !matches!(kind,Kind::Axolotl | Kind::Frog) || mobs[i].no_ai { continue; }
        if let Some(id)=mobs[i].water.target
            && let Some(victim)=mobs.iter().find(|m| m.entity_id==id && m.is_dead()) {
            if kind==Kind::Axolotl && let Some(player)=players.iter().find(|p|Some(p.id)==victim.water.last_attacker && distance(&mobs[i],(p.x,p.y,p.z))<400.0) {
                outcome.axolotl_assists.push(player.id);
            }
            mobs[i].water.target=None;
        }
        if mobs[i].water.playing_dead>0 {continue;}
        // Язык уже выброшен: после поедания ещё два такта до втягивания.
        if kind==Kind::Frog && mobs[i].water.tongue>=8 {
            mobs[i].water.tongue+=1;
            if mobs[i].water.tongue>=10 {
                mobs[i].water.tongue=0; mobs[i].water.target=None; mobs[i].water.swim_to=None;
                mobs[i].water.attack_cooldown=20; mobs[i].water.lock_ticks=0;
            }
            continue;
        }
        let prey=mobs.iter().enumerate().filter(|(j,m)| *j!=i && !m.is_dead() && !m.h.despawn)
            .filter(|(_,m)| if kind==Kind::Frog { super::slime::cube(m.kind) && m.nether.size==1 }
                else { m.water.wet || super::is_water(view.block(m.cell().0,m.cell().1,m.cell().2)) })
            .filter(|(_,m)| kind==Kind::Frog || matches!(m.kind,Kind::Drowned | Kind::Guardian | Kind::ElderGuardian)
                || (mobs[i].water.hunt_cooldown==0 && (fish(m.kind) || matches!(m.kind,Kind::Squid | Kind::GlowSquid))))
            .filter(|(_,m)| distance(m,at)<64.0 && super::hostile::sees(view,(at.0,at.1+mobs[i].eye(),at.2),(m.x,m.y+m.eye(),m.z)))
            .min_by(|(_,a),(_,b)| distance(a,at).total_cmp(&distance(b,at))).map(|(j,_)|j);
        let Some(j)=prey else {
            if mobs[i].water.target.take().is_some() {mobs[i].water.swim_to=None;}
            mobs[i].water.tongue=0; mobs[i].water.lock_ticks=0; mobs[i].water.lock_need=0;
            continue;
        };
        let target=(mobs[j].x,mobs[j].y,mobs[j].z);
        if mobs[i].water.target!=Some(mobs[j].entity_id) {mobs[i].water.tongue=0;}
        mobs[i].water.target=Some(mobs[j].entity_id);
        mobs[i].water.swim_to=Some(target);
        if mobs[i].water.attack_cooldown>0 { continue; }
        if distance(&mobs[i],target)>if kind==Kind::Frog {2.25} else {1.5} { mobs[i].water.tongue=0; continue; }
        if kind==Kind::Frog {
            // Цель видна 30…110 тактов, прежде чем выстрелит язык (чёрный ящик
            // frog-eat-slime: 60, 106, 34; случайная задержка у оригинала [проверить]).
            if mobs[i].water.tongue==0 {
                if mobs[i].water.lock_need==0 { mobs[i].water.lock_need=30+rng.below(81) as i32; }
                mobs[i].water.lock_ticks+=1;
                if mobs[i].water.lock_ticks<mobs[i].water.lock_need { continue; }
            }
            mobs[i].water.tongue+=1;
            let frog=mobs[i].entity_id;
            if mobs[i].water.tongue==1 { mobs[i].events.push(Event::EntitySound{name:"minecraft:entity.frog.tongue",category:6,volume:2.0,pitch:1.0}); }
            if mobs[i].water.tongue<8 { continue; }
            // Съеденный: удар от лягушки, добыча на его месте и тут же исчезает
            // (слизень — шарик, магмовый куб — светокамень по её виду).
            let item=if mobs[j].kind==Kind::MagmaCube {froglight(mobs[i].water.variant)} else {"slime_ball"};
            let place=(mobs[j].x,mobs[j].y,mobs[j].z);
            mobs[j].events.push(Event::Hurt{source:Source::MobAttack,cause:frog,at:place});
            if let Some(item)=blocks::item_named(item) { mobs[j].events.push(Event::Loot(vec![Stack::new(item,1)])); }
            mobs[j].events.push(Event::Death);
            mobs[j].h.despawn=true;
            mobs[i].water.lock_need=0;
            mobs[i].events.push(Event::EntitySound{name:"minecraft:entity.frog.eat",category:6,volume:2.0,pitch:1.0});
        } else {
            let attacker=mobs[i].entity_id;
            let dx=target.0-at.0; let dz=target.2-at.2; let len=dx.hypot(dz).max(1e-6);
            mobs[i].water.attack_cooldown=20;
            let hit=mobs[j].hurt(2.0,Source::MobAttack,Some(Push{dir:(dx/len,dz/len),strength:0.4,cause:attacker}));
            if hit.is_some_and(|h| h.died) && !mobs[j].kind.hostile() { mobs[i].water.hunt_cooldown=2400; }
            if super::guardian::spikes_out(&mobs[j]) { mobs[i].hurt(super::guardian::SPIKES,Source::Thorns,None); }
        }
    }
    // [проверить] Случайный гнев стаи и выбор цели ещё требуют записей оригинала.
    let _=rng;
}

// Вики, Guardian / Drowned: аксолотли — добыча, но «playing dead» снимает цель.
// Снимок перед ходом сохраняет существующие луч и плавание утопленника.
pub fn predator_targets(mobs:&[Mob],players:&[ai::Watcher],guardian:bool)->Vec<ai::Watcher> {
    let mut targets=players.to_vec();
    targets.extend(mobs.iter().filter(|m| !m.is_dead() && !m.h.despawn
        && ((m.kind==Kind::Axolotl && m.water.playing_dead==0)
            || (guardian && matches!(m.kind,Kind::Squid | Kind::GlowSquid))))
        .map(|m| ai::Watcher{id:m.entity_id,x:m.x,y:m.y,z:m.z,eye:m.eye(),holding:[None,None],
            spectator:false,targetable:true,look:(m.yaw,m.pitch),pumpkin: false, sprinting: false, riding: false, gold_armor: false}));
    targets
}

// Луч/удар по водному мобу не должен уйти в очередь урона игроку.
pub fn predator_strikes(mobs:&mut [Mob],outcome:&mut super::Outcome) {
    outcome.strikes.retain(|strike| {
        let Some(victim)=mobs.iter_mut().find(|m| m.entity_id==strike.melee.target && creature(m.kind)) else {return true;};
        let source=if super::guardian::guardian(strike.kind) {Source::IndirectMagic} else {Source::MobAttack};
        victim.hurt(strike.melee.damage,source,Some(Push{cause:strike.attacker,dir:strike.melee.push,strength:0.4}));
        false
    });
}

pub fn child_variant(a: i32,b: i32,rng: &mut Rng) -> i32 {
    if rng.below(1200)==0 {4} else if rng.below(2)==0 {a} else {b}
}

#[cfg(test)]
mod tests;
