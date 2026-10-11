// Хранитель: minecraft.wiki/w/Warden (описание до 26.1.2).
// «anger ... 0 to 150», «decays ... 1 per second». Кода игры нет.
// [проверить] все тактовые границы, выбор пути и случайные паузы: чёрный
// ящик в этой задаче не запускается. [Bedrock: потом] позы и сердцебиение.

use std::collections::BTreeMap;
use super::{ai::{self, Ctx, Plan, Watcher}, Event, Kind, Mob, Particle, Rng, View};

pub const EMERGE: i32 = 134;
pub const DIG: i32 = 100;
pub const ROAR: i32 = 84;
pub const SNIFF: i32 = 84;
pub const CALM: i32 = 1200;
pub const ANGRY: u8 = 80;
pub const AGITATED: u8 = 40;
pub const MELEE_DAMAGE: f32 = 30.0;
pub const SONIC_DAMAGE: f32 = 10.0;
pub const SONIC_CHARGE: i32 = 37;
pub const SONIC_DURATION: i32 = 60;

#[derive(Clone, Copy, Debug)]
pub struct Boom {
    pub attacker: i32,
    pub target: i32,
    pub from: (f64, f64, f64),
    pub to: (f64, f64, f64),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Pose {
    #[default]
    Standing,
    Emerging,
    Digging,
    Roaring,
    Sniffing,
}

impl Pose {
    pub fn id(self) -> i32 {
        match self { Self::Standing => 0, Self::Roaring => 11, Self::Sniffing => 12, Self::Emerging => 13, Self::Digging => 14 }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Heard {
    pub source: Option<i32>,
    pub at: (f64, f64, f64),
    pub projectile: bool,
    pub left: i32,
}

#[derive(Clone, Debug)]
pub struct State {
    pub anger: BTreeMap<i32, u8>,
    pub pose: Pose,
    pub pose_left: i32,
    pub calm: i32,
    pub hearing_cooldown: i32,
    pub heard: Option<Heard>,
    pub sniff_cooldown: i32,
    pub touch_cooldown: i32,
    pub melee_cooldown: i32,
    pub sonic_cooldown: i32,
    pub sonic: i32,
    pub target: Option<i32>,
    pub disturbance: Option<(f64, f64, f64)>,
    pub projectiles: BTreeMap<i32, u64>,
    pub darkness: Vec<i32>,
    pub boom: Option<(i32, (f64, f64, f64))>,
    /// Появление по команде с памятью is_emerging: поза и звук — в первом такте ИИ
    /// (сценарий warden-emerge: звук +1, поза +2 от появления).
    pub pending_emerge: Option<i32>,
    /// Уровень злости, как его видит клиент (свойство 16): обновляется раз в 20 тактов
    /// возраста (сценарии warden-melee, warden-sonic-boom).
    pub client_anger: u8,
    /// Через сколько тактов после удара играет «listening_angry» (warden-melee: +4).
    pub listen_in: i32,
    pub emerge_sounded: bool,
    /// Фаза пульса «Тьмы» в 120-тактовом цикле: у призванного командой 109,
    /// у вышедшего из крикуна 115 (warden-sonic-boom: возраст 108, shrieker-warning: 114).
    pub darkness_phase: u64,
}

impl Default for State {
    fn default() -> Self {
        Self { anger: BTreeMap::new(), pose: Pose::Standing, pose_left: 0, calm: 0,
            hearing_cooldown: 0, heard: None, sniff_cooldown: 100, touch_cooldown: 0,
            melee_cooldown: 0, sonic_cooldown: 200, sonic: 0, target: None,
            disturbance: None, projectiles: BTreeMap::new(), darkness: Vec::new(), boom: None, pending_emerge: None, client_anger: 0, listen_in: 0, emerge_sounded: false, darkness_phase: 109 }
    }
}

impl State {
    pub fn angry_at(&mut self, id: i32, amount: u8) {
        if id == 0 { return; }
        let anger = self.anger.entry(id).or_default();
        *anger = anger.saturating_add(amount).min(150);
        self.calm = CALM;
    }

    pub fn level(&self) -> u8 { self.anger.values().copied().max().unwrap_or(0) }

    pub fn invulnerable(&self) -> bool { matches!(self.pose, Pose::Emerging | Pose::Digging) }

    pub fn hear(&mut self, event: Heard) {
        if self.invulnerable() || self.hearing_cooldown > 0 || self.heard.is_some() { return; }
        self.heard = Some(event);
    }
}

fn sound(mob: &mut Mob, name: &'static str, volume: f32) {
    mob.events.push(Event::Sound { name, volume, pitch: 1.0 });
}

/// Звук со случайной высотой, как у голосов хранителя (сценарии warden-melee и
/// warden-sonic-boom: 0,83…1,13).
fn voice(mob: &mut Mob, rng: &mut Rng, name: &'static str, volume: f32) {
    let pitch = 1.0 + ((rng.unit() - rng.unit()) * 0.2) as f32;
    mob.events.push(Event::Sound { name, volume, pitch });
}

pub fn emerge(mob: &mut Mob) {
    mob.warden.pose = Pose::Emerging;
    mob.warden.pose_left = EMERGE;
    if !mob.warden.emerge_sounded { sound(mob, "minecraft:entity.warden.emerge", 5.0); }
}

pub fn on_hurt(mob: &mut Mob, id: i32) {
    mob.warden.angry_at(id, 100);
    mob.warden.disturbance = None;
    // [проверить] ближний удар пропускает рёв; расстояние проверяет think.
    mob.h.provoked = true;
}

fn targets(ctx: &Ctx) -> Vec<Watcher> {
    let mut targets: Vec<_> = ctx.players.iter().filter(|p| p.targetable && !p.spectator).copied().collect();
    targets.extend(ctx.mates.iter().filter(|m| !m.kind.object() && m.kind != Kind::Warden).map(|m| Watcher {
        id: m.id, x: m.x, y: m.y, z: m.z, eye: m.kind.height() * 0.85,
        holding: [None; 2], spectator: false, targetable: true, look: (0.0, 0.0), pumpkin: false, sprinting: false, riding: false, gold_armor: false,
    }));
    targets
}

pub fn sonic_range(from: (f64, f64, f64), to: (f64, f64, f64)) -> bool {
    (from.0 - to.0).powi(2) + (from.2 - to.2).powi(2) <= 15.0 * 15.0 && (from.1 - to.1).abs() <= 20.0
}

pub fn think(mob: &mut Mob, view: &dyn View, ctx: &Ctx, rng: &mut Rng) -> Plan {
    let mut plan = Plan::default();
    if ctx.difficulty == crate::survival::PEACEFUL { mob.h.despawn = true; return plan; }
    let targets = targets(ctx);
    let at = (mob.x, mob.y, mob.z);
    mob.warden.anger.retain(|id, _| targets.iter().any(|p| p.id == *id));
    // Сценарий warden-emerge: звук на такт 1, поза — на такт 2.
    if let Some(left) = mob.warden.pending_emerge {
        if mob.warden.emerge_sounded {
            mob.warden.pending_emerge = None;
            emerge(mob);
            mob.warden.pose_left = left.max(1);
        } else {
            mob.warden.emerge_sounded = true;
            sound(mob, "minecraft:entity.warden.emerge", 5.0);
        }
        return plan;
    }
    if mob.ticks.is_multiple_of(20) {
        // Сценарии warden-melee и warden-sonic-boom: злость убывает на 1 раз в
        // 20 тактов и у цели тоже, клиенту она уходит в тот же такт.
        mob.warden.anger.retain(|_, anger| { *anger = anger.saturating_sub(1); *anger > 0 });
        mob.warden.client_anger = mob.warden.level();
    }
    mob.warden.projectiles.retain(|_, tick| mob.ticks.saturating_sub(*tick) <= 100);
    if mob.ticks % 120 == mob.warden.darkness_phase {
        mob.warden.darkness.extend(ctx.players.iter().filter(|p| p.targetable && distance(at, (p.x, p.y, p.z)) <= 20.0).map(|p| p.id));
    }
    // Сердцебиение клиент играет по client_anger_level (свойство 16).
    for cooldown in [&mut mob.warden.hearing_cooldown, &mut mob.warden.sniff_cooldown,
        &mut mob.warden.touch_cooldown, &mut mob.warden.melee_cooldown, &mut mob.warden.sonic_cooldown] {
        *cooldown = (*cooldown - 1).max(0);
    }
    if mob.warden.invulnerable() {
        mob.brain.stop();
        mob.warden.pose_left -= 1;
        if mob.warden.pose_left <= 0 {
            if mob.warden.pose == Pose::Digging { mob.h.despawn = true; return plan; }
            // Без памяти dig_cooldown хранитель зарывается в том же такте, не
            // показывая позу «стоит» (сценарий warden-emerge).
            mob.warden.pose = Pose::Standing;
            if mob.warden.calm > 0 || mob.name.is_some() { return plan; }
        } else { return plan; }
    }
    if let Some(mut heard) = mob.warden.heard.take() {
        heard.left -= 1;
        if heard.left > 0 { mob.warden.heard = Some(heard); }
        else {
            mob.warden.hearing_cooldown = 40;
            mob.warden.calm = CALM;
            mob.warden.disturbance = Some(heard.at);
            // Сценарии warden-melee, warden-sonic-boom: статус 61, щелчки усиков (5),
            // затем «слушающий» голос (10) — все со случайной высотой.
            mob.events.push(Event::Status(61));
            voice(mob, rng, "minecraft:entity.warden.tendril_clicks", 5.0);
            voice(mob, rng, if mob.warden.level() >= AGITATED { "minecraft:entity.warden.listening_angry" } else { "minecraft:entity.warden.listening" }, 10.0);
            if let Some(id) = heard.source.filter(|id| targets.iter().any(|p| p.id == *id)) {
                let mut amount = 35;
                if heard.projectile {
                    amount = if mob.warden.projectiles.insert(id, mob.ticks).is_some() { 35 } else { 10 };
                    if targets.iter().find(|p| p.id == id).is_none_or(|p| distance(at, (p.x, p.y, p.z)) > 30.0) { amount = 0; }
                }
                mob.warden.angry_at(id, amount);
            }
        }
    }
    if mob.warden.touch_cooldown == 0
        && let Some(p) = targets.iter().find(|p| (p.x - mob.x).abs() < 0.75 && (p.z - mob.z).abs() < 0.75 && p.y < mob.y + 2.9 && p.y + p.eye > mob.y)
    {
        mob.warden.angry_at(p.id, 35);
        mob.warden.touch_cooldown = 20;
    }
    if mob.warden.target.is_some_and(|id| !targets.iter().any(|p| p.id == id)) {
        mob.warden.target = None;
        mob.warden.sonic = 0;
        mob.warden.pose = Pose::Standing;
    }
    if mob.warden.target.is_none() {
        let suspect = targets.iter().filter(|p| mob.warden.anger.get(&p.id).copied().unwrap_or(0) >= ANGRY)
            .max_by_key(|p| (ctx.players.iter().any(|player| player.id == p.id), mob.warden.anger[&p.id]));
        if let Some(p) = suspect {
            mob.warden.target = Some(p.id);
            // Сценарий warden-sonic-boom: злость цели при захвате не растёт (100, 99, 98…).
            mob.warden.sonic_cooldown = 202;
            mob.brain.stop();
            // Сценарий warden-melee: рядом стоящий обидчик — первый удар через 7
            // тактов после урона, «listening_angry» через 4; рёва нет (sonic-boom
            // тоже без рёва).
            if mob.h.provoked && distance(at, (p.x, p.y, p.z)) <= 3.0 {
                mob.warden.melee_cooldown = mob.warden.melee_cooldown.max(7);
                mob.warden.listen_in = 6;
            }
            if !mob.h.provoked {
                mob.warden.pose = Pose::Roaring;
                mob.warden.pose_left = ROAR;
                sound(mob, "minecraft:entity.warden.roar", 3.0);
            } else { mob.warden.pose = Pose::Standing; }
            mob.h.provoked = false;
        }
    }
    if mob.warden.listen_in > 0 {
        mob.warden.listen_in -= 1;
        if mob.warden.listen_in == 0 { voice(mob, rng, "minecraft:entity.warden.listening_angry", 10.0); }
    }
    if mob.warden.pose == Pose::Roaring {
        mob.warden.pose_left -= 1;
        if mob.warden.pose_left <= 0 { mob.warden.pose = Pose::Standing; }
        return plan;
    }
    if let Some(p) = mob.warden.target.and_then(|id| targets.iter().find(|p| p.id == id)) {
        mob.warden.calm = CALM;
        mob.warden.pose = Pose::Standing;
        if mob.warden.sonic > 0 || ((p.x - mob.x).powi(2) + (p.z - mob.z).powi(2)).sqrt() <= 2.5 { ai::look_at_point(mob, p.x, p.y + p.eye, p.z); }
        if mob.warden.sonic > 0 {
            mob.brain.stop();
            mob.warden.sonic += 1;
            if mob.warden.sonic == SONIC_CHARGE && sonic_range(at, (p.x, p.y, p.z)) {
                sound(mob, "minecraft:entity.warden.sonic_boom", 3.0);
                mob.warden.boom = Some((p.id, (p.x, p.y + p.eye, p.z)));
                let from = (mob.x, mob.y + 1.6, mob.z);
                let len = distance(from, (p.x, p.y + p.eye, p.z)).max(0.001);
                for i in 1..=(len.floor() as i32 + 7) {
                    let t = f64::from(i) / len;
                    mob.events.push(Event::Particle { kind: Particle::SonicBoom,
                        at: (from.0 + (p.x - from.0) * t, from.1 + (p.y + p.eye - from.1) * t, from.2 + (p.z - from.2) * t),
                        offset: (0.0, 0.0, 0.0), speed: 0.0, count: 1 });
                }
            }
            // Сценарий warden-sonic-boom: от заряда до следующего заряда 102 такта.
            if mob.warden.sonic >= SONIC_DURATION { mob.warden.sonic = 0; mob.warden.sonic_cooldown = 43; }
            return plan;
        }
        let flat = ((p.x - mob.x).powi(2) + (p.z - mob.z).powi(2)).sqrt();
        if flat <= 2.5 && (p.y - mob.y).abs() <= 2.0 && super::hostile::sees(view, (mob.x, mob.y + mob.eye(), mob.z), (p.x, p.y + p.eye, p.z)) {
            if mob.warden.melee_cooldown == 0 {
                mob.warden.melee_cooldown = 19;
                mob.warden.sonic_cooldown = 100;
                let push = ((p.x - mob.x) / flat.max(0.001), (p.z - mob.z) / flat.max(0.001));
                if ctx.players.iter().any(|player| player.id == p.id) {
                    plan.melee = Some(super::hostile::Melee { target: p.id, damage: MELEE_DAMAGE, push, wither: false, hunger: false });
                } else { plan.bite = Some((p.id, MELEE_DAMAGE, push)); }
                mob.events.push(Event::Swing);
                mob.events.push(Event::Status(4));
                voice(mob, rng, "minecraft:entity.warden.attack_impact", 10.0);
            }
        } else if mob.warden.sonic_cooldown == 0 && sonic_range(at, (p.x, p.y, p.z)) {
            mob.warden.sonic = 1;
            mob.brain.stop();
            mob.events.push(Event::Status(62));
            sound(mob, "minecraft:entity.warden.sonic_charge", 3.0);
            return plan;
        }
        approach(mob,view,(p.x,p.y,p.z),1.2);
        return ai::walk(mob, plan);
    }
    mob.warden.calm -= 1;
    if mob.warden.pose == Pose::Sniffing {
        mob.warden.pose_left -= 1;
        if mob.warden.pose_left <= 0 {
            mob.warden.pose = Pose::Standing;
            mob.warden.sniff_cooldown = 100 + rng.below(101) as i32;
            if let Some(p) = targets.iter().filter(|p| (p.x - mob.x).powi(2) + (p.z - mob.z).powi(2) <= 36.0 && (p.y - mob.y).abs() <= 20.0)
                .min_by(|a, b| distance(at, (a.x, a.y, a.z)).total_cmp(&distance(at, (b.x, b.y, b.z)))) {
                mob.warden.angry_at(p.id, 35);
                mob.warden.disturbance = Some((p.x, p.y, p.z));
            }
        }
        return plan;
    }
    // Чёрный ящик (warden-sonic-boom): призванный командой хранитель без памяти
    // dig_cooldown зарывается сразу, даже с PersistenceRequired.
    if mob.warden.calm <= 0 && mob.name.is_none() {
        mob.brain.stop();
        if super::is_water(view.block(mob.cell().0, mob.cell().1, mob.cell().2)) || super::is_lava(view.block(mob.cell().0, mob.cell().1, mob.cell().2)) {
            mob.h.despawn = true;
        } else {
            mob.warden.pose = Pose::Digging;
            mob.warden.pose_left = DIG;
            sound(mob, "minecraft:entity.warden.dig", 5.0);
        }
        return plan;
    }
    if mob.warden.sniff_cooldown == 0 && targets.iter().any(|p| distance(at, (p.x, p.y, p.z)) <= 24.0) {
        mob.warden.pose = Pose::Sniffing;
        mob.warden.pose_left = SNIFF;
        mob.brain.stop();
        sound(mob, "minecraft:entity.warden.sniff", 5.0);
        return plan;
    }
    // Вики, Warden: «24 blocks» — запах ведёт к цели и вне радиуса
    // прибавки злости. [проверить] границы поиска и очередность со звуками.
    if mob.warden.disturbance.is_none()
        && let Some(p) = targets.iter().filter(|p| distance(at, (p.x, p.y, p.z)) <= 24.0)
            .min_by(|a, b| distance(at, (a.x, a.y, a.z)).total_cmp(&distance(at, (b.x, b.y, b.z)))) {
        mob.warden.disturbance = Some((p.x, p.y, p.z));
        mob.warden.calm = CALM;
    }
    if let Some(point) = mob.warden.disturbance {
        if distance(at, point) < 2.0 { mob.warden.disturbance = None; }
        else {
            approach(mob,view,point,1.0);
            return ai::walk(mob, plan);
        }
    }
    super::hostile::idle_walk(mob, view, ctx, rng, plan, mob.on_ground)
}

fn approach(mob:&mut Mob,view:&dyn View,point:(f64,f64,f64),speed:f64) {
    mob.brain.repath-=1;
    if mob.brain.walking() && mob.brain.repath>0 {return;}
    let goal=(point.0.floor() as i32,point.1.floor() as i32,point.2.floor() as i32);
    let route=super::path::find_height(view,mob.cell(),goal,super::path::MAX_NODES,3);
    mob.brain.go(route,speed);mob.brain.repath=10;
}

pub fn distance(a: (f64, f64, f64), b: (f64, f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2) + (a.2 - b.2).powi(2)).sqrt()
}

/// Вики, Warden: звук толкает в 3D, 2,5 по горизонтали и 0,5 по вертикали.
/// [проверить] скорость до/после трения в сетевом такте.
pub fn sonic_velocity(delta: (f64, f64, f64), resistance: f64) -> (f64, f64, f64) {
    let length = distance(delta, (0.0, 0.0, 0.0)).max(1.0e-9);
    let factor = (1.0 - resistance).clamp(0.0, 1.0) / length;
    (delta.0 * factor * 2.5, delta.1 * factor * 0.5, delta.2 * factor * 2.5)
}

/// «20 attempts» в 11×13×11 вокруг крикуна; свет и лимит монстров не нужны.
/// [проверить] порядок обхода высоты и препятствия в неполных блоках.
pub fn spawn_spot(view: &dyn View, origin: (i32, i32, i32), rng: &mut Rng) -> Option<(f64, f64, f64)> {
    for _ in 0..20 {
        let x = origin.0 + rng.below(11) as i32 - 5;
        let z = origin.2 + rng.below(11) as i32 - 5;
        for y in (origin.1 - 6..=origin.1 + 6).rev() {
            if !view.loaded(x.div_euclid(16), z.div_euclid(16)) { continue; }
            if !crate::blocks::solid_top(view.block(x, y - 1, z)) { continue; }
            if (0..3).all(|dy| {
                let block = view.block(x, y + dy, z);
                !super::solid(block) && !super::spawn::space::has_fluid(block)
            }) { return Some((f64::from(x) + 0.5, f64::from(y), f64::from(z) + 0.5)); }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn anger_is_per_suspect_and_capped() {
        let mut state = State::default();
        state.angry_at(1, 35); state.angry_at(2, 100); state.angry_at(2, 100);
        assert_eq!(state.anger[&1], 35); assert_eq!(state.anger[&2], 150);
        assert_eq!(state.level(), 150); assert_eq!(state.calm, CALM);
    }
    #[test]
    fn sonic_range_has_horizontal_and_vertical_limits() {
        assert!(sonic_range((0.0, 0.0, 0.0), (15.0, 20.0, 0.0)));
        assert!(!sonic_range((0.0, 0.0, 0.0), (15.01, 0.0, 0.0)));
        assert!(!sonic_range((0.0, 0.0, 0.0), (0.0, 20.01, 0.0)));
    }
    #[test]
    fn emerging_warden_ignores_damage_and_knockback() {
        let mut mob = Mob::new(1, Kind::Warden, (0.0, 0.0, 0.0));
        emerge(&mut mob);
        assert!(mob.hurt(50.0, crate::survival::Source::PlayerAttack, None).is_none());
        assert_eq!(mob.size(), (0.9, 1.0));
        mob.knockback(1.0, 0.0, 1.0);
        assert_eq!(mob.vx, 0.0);
        assert!(mob.hurt(1000.0, crate::survival::Source::GenericKill, None).unwrap().died);
    }

    struct Floor;
    impl View for Floor {
        fn block(&self, _x: i32, y: i32, _z: i32) -> i32 {
            if y < 0 { crate::blocks::state_by_name("stone").unwrap() } else { crate::world::AIR }
        }
        fn loaded(&self, _x: i32, _z: i32) -> bool { true }
    }

    fn watcher(id: i32, x: f64) -> Watcher {
        Watcher { id, x, y: 0.0, z: 0.0, eye: 1.62, holding: [None; 2], spectator: false, targetable: true,
            look: (0.0, 0.0), pumpkin: false, sprinting: false, riding: false, gold_armor: false }
    }

    #[test]
    fn emergence_and_quiet_dig_have_bounded_durations() {
        let mut mob = Mob::new(7, Kind::Warden, (0.0, 0.0, 0.0));
        let ctx = Ctx { players: &[], mates: &[], difficulty: 2, sky: 0, sun_up: false };
        let mut rng = Rng::new(1);
        emerge(&mut mob);
        mob.warden.calm = CALM;
        for tick in 1..EMERGE {
            mob.ticks = tick as u64;
            think(&mut mob, &Floor, &ctx, &mut rng);
            assert_eq!(mob.warden.pose, Pose::Emerging);
        }
        think(&mut mob, &Floor, &ctx, &mut rng);
        assert_eq!(mob.warden.pose, Pose::Standing);
        for _ in 0..CALM { think(&mut mob, &Floor, &ctx, &mut rng); }
        assert_eq!(mob.warden.pose, Pose::Digging);
        for _ in 0..DIG { think(&mut mob, &Floor, &ctx, &mut rng); }
        assert!(mob.h.despawn);
    }

    #[test]
    fn anger_hearing_delay_and_cooldown() {
        let mut mob = Mob::new(7, Kind::Warden, (0.0, 0.0, 0.0));
        let players = [watcher(1, 10.0)];
        let ctx = Ctx { players: &players, mates: &[], difficulty: 2, sky: 0, sun_up: false };
        let mut rng = Rng::new(1);
        mob.warden.calm = CALM;
        let event = Heard { source: Some(1), at: (10.0, 0.0, 0.0), projectile: false, left: 10 };
        mob.warden.hear(event);
        for tick in 1..10 {
            mob.ticks = tick;
            think(&mut mob, &Floor, &ctx, &mut rng);
            assert_eq!(mob.warden.level(), 0);
        }
        mob.ticks = 10;
        think(&mut mob, &Floor, &ctx, &mut rng);
        assert_eq!(mob.warden.level(), 35);
        mob.warden.hear(event);
        assert!(mob.warden.heard.is_none());
        mob.ticks = 20;
        think(&mut mob, &Floor, &ctx, &mut rng);
        assert_eq!(mob.warden.level(), 34);
    }

    #[test]
    fn melee_and_sonic_have_independent_cooldowns() {
        let mut mob = Mob::new(7, Kind::Warden, (0.0, 0.0, 0.0));
        let players = [watcher(1, 2.0)];
        let ctx = Ctx { players: &players, mates: &[], difficulty: 2, sky: 0, sun_up: false };
        let mut rng = Rng::new(1);
        mob.warden.calm = CALM;
        on_hurt(&mut mob, 1);
        // Сценарий warden-melee: первый удар — спустя несколько тактов после урона.
        let mut plan = think(&mut mob, &Floor, &ctx, &mut rng);
        let mut waited = 0;
        while plan.melee.is_none() && waited < 12 { plan = think(&mut mob, &Floor, &ctx, &mut rng); waited += 1; }
        assert!(waited >= 5);
        assert_eq!(plan.melee.unwrap().damage, 30.0);
        // Дальше удары идут каждые 19 тактов.
        for _ in 0..18 { assert!(think(&mut mob, &Floor, &ctx, &mut rng).melee.is_none()); }
        assert!(think(&mut mob, &Floor, &ctx, &mut rng).melee.is_some());
        let players = [watcher(1, 10.0)];
        let ctx = Ctx { players: &players, ..ctx };
        mob.warden.sonic_cooldown = 1;
        think(&mut mob, &Floor, &ctx, &mut rng);
        assert_eq!(mob.warden.sonic, 1);
        for _ in 1..SONIC_CHARGE - 1 { think(&mut mob, &Floor, &ctx, &mut rng); assert!(mob.warden.boom.is_none()); }
        think(&mut mob, &Floor, &ctx, &mut rng);
        assert_eq!(mob.warden.boom.unwrap().0, 1);
    }

    #[test]
    fn sonic_bypasses_armor_but_resistance_still_works() {
        assert!(crate::combat::bypasses_armor(crate::survival::Source::SonicBoom));
        let shielded = crate::players::CombatView { armor: 20.0, toughness: 12.0, protection: 20,
            blocking_since: Some(0), ..crate::players::CombatView::default() };
        assert_eq!(crate::combat::outcome(&shielded, SONIC_DAMAGE, crate::survival::Source::SonicBoom,
            20, (0.0, 0.0, 0.0), (0.0, 5.0)), crate::combat::Outcome::Taken(10.0));
        assert_eq!(crate::combat::outcome(&shielded, MELEE_DAMAGE, crate::survival::Source::MobAttack,
            20, (0.0, 0.0, 0.0), (0.0, 5.0)), crate::combat::Outcome::Blocked);
        assert_eq!(crate::survival::after_resistance(10.0, 1, crate::survival::Source::SonicBoom), 8.0);
        assert_eq!(sonic_velocity((10.0, 0.0, 0.0), 0.0), (2.5, 0.0, 0.0));
        assert_eq!(sonic_velocity((10.0, 5.0, 0.0), 1.0), (0.0, 0.0, 0.0));
    }

    #[test]
    fn spawn_requires_floor_and_three_clear_blocks() {
        let at = spawn_spot(&Floor, (0, 0, 0), &mut Rng::new(1)).unwrap();
        assert_eq!(at.1, 0.0);
        assert!(at.0.abs() <= 5.5 && at.2.abs() <= 5.5);
    }

    #[test]
    fn fire_and_lava_never_damage_warden_and_drop_is_fixed() {
        let mut mob = Mob::new(7, Kind::Warden, (0.0, 0.0, 0.0));
        for source in [crate::survival::Source::OnFire, crate::survival::Source::InFire, crate::survival::Source::Lava] {
            assert!(mob.hurt(50.0, source, None).is_none());
        }
        mob.credit_player();
        mob.hurt(1000.0, crate::survival::Source::GenericKill, None);
        assert_eq!(mob.pending_xp, 5);
        let loot = super::super::loot(&mob, &mut Rng::new(1));
        assert_eq!(loot.len(), 1);
        assert_eq!(crate::blocks::item_name(loot[0].item), Some("sculk_catalyst"));
    }

    #[test]
    fn warden_walks_to_sound_and_scent_with_a_speed_multiplier() {
        let mut m=Mob::new(7,Kind::Warden,(0.5,0.0,0.5));m.on_ground=true;
        m.warden.calm=CALM;m.warden.sniff_cooldown=200;m.warden.disturbance=Some((10.5,0.0,0.5));
        let mut rng=Rng::new(5);let ctx=Ctx {players:&[],mates:&[],difficulty:2,sky:0,sun_up:false};
        assert_eq!(think(&mut m,&Floor,&ctx,&mut rng).speed,1.0);
        for _ in 0..40 {m.tick(&Floor,&ctx,&mut rng);}
        assert!(m.x>4.0,"{}",m.x);
        m.warden.disturbance=None;m.brain.stop();
        let players=[watcher(1,20.0)];let ctx=Ctx {players:&players,..ctx};
        assert!(think(&mut m,&Floor,&ctx,&mut rng).speed>0.0);
        assert_eq!(m.warden.level(),0);
    }

    #[test]
    fn path_avoids_two_block_high_passages() {
        struct Tunnel;
        impl View for Tunnel {
            fn block(&self,x:i32,y:i32,z:i32)->i32 {
                if y<0 || (y==2 && x==2 && z==0) {crate::blocks::state_by_name("stone").unwrap()} else {0}
            }
            fn loaded(&self,_x:i32,_z:i32)->bool {true}
        }
        let path=super::super::path::find_height(&Tunnel,(0,0,0),(4,0,0),400,3);
        assert_eq!(path.last(),Some(&(4,0,0)));assert!(!path.contains(&(2,0,0)));
    }

}
