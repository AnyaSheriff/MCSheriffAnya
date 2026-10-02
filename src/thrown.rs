// Брошенные зелья (взрывное и оседающее), огненный шар дракона и облака
// (`area_effect_cloud`): оседающего зелья и дыхания дракона.
//
// Правила — вики (Splash Potion, Lingering Potion, Area Effect Cloud, Dragon
// Fireball) и чёрный ящик (splash-potion*, lingering-potion*, dragon-fireball):
// зелье летит 0,5 блока за такт, задранное на 20° (поправка только к
// вертикали, вектор потом приводится к длине); такт полёта — тяжесть 0,05,
// сопротивление 0,99, ход (splash-potion: сдвиг за 10 тактов совпал только
// при таком порядке); поворот догоняет полёт на пятую часть за такт.
// Раскол — событие мира 2002 (2007, если есть мгновенный эффект) с цветом
// зелья; звук раскола играет сам клиент. Что не сверено, помечено «[проверить]».
//
// Всё хранится общим списком в `Shared::thrown` (у каждого своё измерение);
// такт двигает их из tick.rs, клиентам о них рассказывает своё слежение
// (network/play/thrown.rs) — в конце хода, после событий раскола.

use crate::dimension::Dimension;
use crate::inventory::Stack;
use crate::shared::Shared;
use crate::survival::{Effect, EffectKind, Source};
use crate::world::World;

/// Номера видов сущностей (minecraft-data 26.1, entities.json).
pub const SPLASH_TYPE: i32 = 105;
pub const LINGERING_TYPE: i32 = 106;
pub const CLOUD_TYPE: i32 = 3;
pub const FIREBALL_TYPE: i32 = 37;

/// Скорость броска зелья и поправка вверх (вики, Splash Potion: «about 0.5
/// blocks/tick», «–20°»).
pub const POTION_SPEED: f64 = 0.5;
pub const POTION_PITCH_OFFSET: f32 = -20.0;

/// Тяжесть и сопротивление брошенного зелья (вики, Entity: 0,05 и 0,99).
const GRAVITY: f64 = 0.05;
const DRAG: f64 = 0.99;

/// Огненный шар: сопротивление 0,95 и разгон 0,1 по направлению полёта
/// (dragon-fireball: из 4 блоков со скоростью −0,2 о пол на +7) [проверить
/// порядок разгона и хода].
const FIREBALL_DRAG: f64 = 0.95;
const FIREBALL_POWER: f64 = 0.1;

/// Полуширина зелья (вики, Splash Potion: «Width: 0.25»).
const POTION_HALF: f64 = 0.125;

/// Цвет воды без эффектов (вики, Potion: «#385DC6»).
pub const WATER_COLOR: i32 = 0x385DC6;

/// Что летит.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Splash,
    Lingering,
    DragonFireball,
}

/// Место, скорость и поворот снаряда.
pub type Snapshot = ((f64, f64, f64), (f64, f64, f64), (f32, f32));

/// Летящий снаряд: зелье или огненный шар.
#[derive(Clone, Debug)]
pub struct Flying {
    pub entity_id: i32,
    pub uuid: [u8; 16],
    pub dimension: Dimension,
    pub kind: Kind,
    /// Брошенное зелье (с составом); у огненного шара нет.
    pub stack: Option<Stack>,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub vx: f64,
    pub vy: f64,
    pub vz: f64,
    /// Поворот (угол, наклон): догоняет полёт на 0,2 за такт.
    pub rot: (f32, f32),
    /// Место, скорость и поворот до хода этого такта: слежение оригинала
    /// показывает их (splash-potion: на +10 — после десяти ходов).
    pub prev: Snapshot,
    /// Направление разгона огненного шара.
    pub push: (f64, f64, f64),
    pub owner: i32,
    /// Зелье ещё внутри бросившего: его не задевает (splash-potion-feet:
    /// брошенное под ноги летит сквозь робота на пол).
    pub in_owner: bool,
    pub age: u32,
    /// Место и скорость при появлении: клиенту сперва уходит оно.
    pub start: ((f64, f64, f64), (f64, f64, f64)),
}

impl Flying {
    pub fn entity_type(&self) -> i32 {
        match self.kind {
            Kind::Splash => SPLASH_TYPE,
            Kind::Lingering => LINGERING_TYPE,
            Kind::DragonFireball => FIREBALL_TYPE,
        }
    }

    /// Брошенное игроком зелье: из глаз (−0,1) по взгляду с поправкой.
    pub fn potion(entity_id: i32, dimension: Dimension, kind: Kind, stack: Stack, at: (f64, f64, f64), velocity: (f64, f64, f64), owner: i32) -> Self {
        let rot = angles(velocity);

        Self {
            entity_id,
            uuid: crate::items::uuid_for(entity_id),
            dimension,
            kind,
            stack: Some(stack),
            x: at.0,
            y: at.1,
            z: at.2,
            vx: velocity.0,
            vy: velocity.1,
            vz: velocity.2,
            rot,
            prev: (at, velocity, rot),
            push: (0.0, 0.0, 0.0),
            owner,
            in_owner: true,
            age: 0,
            start: (at, velocity),
        }
    }

    /// Огненный шар дракона: из точки `at` со скоростью `velocity`, разгон —
    /// по `direction` (длина приводится к единице).
    pub fn fireball(entity_id: i32, dimension: Dimension, at: (f64, f64, f64), velocity: (f64, f64, f64), direction: (f64, f64, f64), owner: i32) -> Self {
        let length = (direction.0 * direction.0 + direction.1 * direction.1 + direction.2 * direction.2).sqrt();
        let push = if length < 1.0e-9 { (0.0, 0.0, 0.0) } else { (direction.0 / length, direction.1 / length, direction.2 / length) };

        Self { kind: Kind::DragonFireball, stack: None, push, in_owner: false, rot: (0.0, 0.0), prev: (at, velocity, (0.0, 0.0)), ..Self::potion(entity_id, dimension, Kind::Splash, Stack::new(0, 1), at, velocity, owner) }
    }

    /// Эффекты зелья.
    pub fn effects(&self) -> Vec<Effect> {
        self.stack.map(|stack| crate::survival::potion_effects(stack.extras().potion.unwrap_or(0))).unwrap_or_default()
    }
}

/// Угол по сторонам света и наклон по скорости, как у стрел: `yaw =
/// atan2(vx, vz)` (splash-potion: на −z поворот 127 из 256).
pub fn angles((vx, vy, vz): (f64, f64, f64)) -> (f32, f32) {
    (vx.atan2(vz).to_degrees() as f32, vy.atan2(vx.hypot(vz)).to_degrees() as f32)
}

/// Скорость броска зелья по взгляду: поправка −20° только к вертикали, потом
/// вектор приводится к `speed` (splash-potion: взгляд прямо вверх — летит
/// прямо вверх; по горизонту — вверх на 19°).
pub fn throw_velocity(yaw: f32, pitch: f32, speed: f64, wobble: &mut dyn FnMut() -> f64) -> (f64, f64, f64) {
    // Наклон за пределами ±90 оригинал обрезает (splash-potion: робот прислал
    // взгляд «выше зенита» — зелье ушло прямо вверх).
    let pitch = pitch.clamp(-90.0, 90.0);
    let (yaw, pitch_r, lifted) = (f64::from(yaw).to_radians(), f64::from(pitch).to_radians(), f64::from(pitch + POTION_PITCH_OFFSET).to_radians());
    let (dx, dy, dz) = (-yaw.sin() * pitch_r.cos(), -lifted.sin(), yaw.cos() * pitch_r.cos());
    let length = (dx * dx + dy * dy + dz * dz).sqrt().max(1.0e-9);

    ((dx / length + wobble()) * speed, (dy / length + wobble()) * speed, (dz / length + wobble()) * speed)
}

/// Цвет зелья: среднее цветов эффектов с весом «уровень + 1»; без эффектов —
/// вода (вики, Potion: «Color») [проверить смешение].
pub fn potion_color(effects: &[Effect]) -> i32 {
    if effects.is_empty() {
        return WATER_COLOR;
    }

    let (mut r, mut g, mut b, mut weight) = (0.0f32, 0.0f32, 0.0f32, 0.0f32);

    for effect in effects {
        let color = effect.kind.color();
        let w = (effect.amplifier + 1) as f32;

        r += ((color >> 16) & 0xFF) as f32 / 255.0 * w;
        g += ((color >> 8) & 0xFF) as f32 / 255.0 * w;
        b += (color & 0xFF) as f32 / 255.0 * w;
        weight += w;
    }

    let channel = |value: f32| ((value / weight * 255.0) as i32).clamp(0, 255);

    (channel(r) << 16) | (channel(g) << 8) | channel(b)
}

/// Цвет с непрозрачностью: так он уходит в событии мира и частице облака
/// (splash-potion: 2002 / −13374465 у быстроты — 0xFF33EBFF).
pub fn opaque(color: i32) -> i32 {
    (color as u32 | 0xFF00_0000) as i32
}

/// Облако эффектов.
#[derive(Clone, Debug)]
pub struct Cloud {
    pub entity_id: i32,
    pub uuid: [u8; 16],
    pub dimension: Dimension,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub radius: f32,
    /// Как радиус меняется за такт и при каждом наложении.
    pub per_tick: f32,
    pub on_use: f32,
    /// Ожидание до первого действия, срок жизни после него, пауза перед
    /// повторным наложением на того же.
    pub wait: u32,
    pub duration: u32,
    pub delay: u32,
    pub age: u32,
    /// Ждёт (свойство 9).
    pub waiting: bool,
    pub effects: Vec<Effect>,
    /// Дыхание дракона (частица dragon_breath); иначе частица entity_effect цвета `color`.
    pub dragon: bool,
    pub color: i32,
    /// Чей (дыхание не трогает дракона).
    pub owner: i32,
    /// Кому и до какого возраста облака не накладывать снова.
    pub victims: Vec<(i32, u32)>,
    /// Слежение видит облако на такт позже: появившееся от удара о блок
    /// (lingering-potion-feet, dragon-fireball: первый сдвиг и смена свойств
    /// на такт позже, чем у разбитого о голову, lingering-potion).
    pub lag: bool,
    /// Радиус и ожидание до хода этого такта (для отстающего слежения).
    pub prev: (f32, bool),
}

impl Cloud {
    /// Облако оседающего зелья: радиус 3, ждёт 10 тактов, живёт 600, тает на
    /// 3/600 за такт и на 0,5 за наложение, повтор через 20 (lingering-potion-feet;
    /// вики, Lingering Potion).
    pub fn lingering(entity_id: i32, dimension: Dimension, at: (f64, f64, f64), effects: Vec<Effect>, owner: i32, lag: bool) -> Self {
        let color = potion_color(&effects);

        Self {
            entity_id,
            uuid: crate::items::uuid_for(entity_id),
            dimension,
            x: at.0,
            y: at.1,
            z: at.2,
            radius: 3.0,
            per_tick: -3.0 / 600.0,
            on_use: -0.5,
            wait: 10,
            duration: 600,
            delay: 20,
            age: 0,
            waiting: false,
            effects,
            dragon: false,
            color,
            owner,
            victims: Vec::new(),
            lag,
            prev: (3.0, false),
        }
    }

    /// Дыхание дракона: радиус 3 растёт до 7 за 600 тактов, ждёт 20 тактов,
    /// мгновенный урон II (dragon-fireball; вики, Dragon Fireball).
    pub fn dragon_breath(entity_id: i32, dimension: Dimension, at: (f64, f64, f64), owner: i32) -> Self {
        let effects = vec![Effect { kind: EffectKind::InstantDamage, amplifier: 1, ticks_left: 1, ambient: false, visible: true }];

        Self { per_tick: (7.0 - 3.0) / 600.0, on_use: 0.0, wait: 20, dragon: true, ..Self::lingering(entity_id, dimension, at, effects, owner, true) }
    }

    /// Ящик облака: радиус вширь, полблока ввысь (minecraft-data: высота 0,5).
    fn touches(&self, at: (f64, f64, f64), half: f64, height: f64) -> bool {
        let r = f64::from(self.radius);
        let dx = at.0 - self.x;
        let dz = at.2 - self.z;

        at.1 < self.y + 0.5 && at.1 + height > self.y && dx.abs() < r + half && dz.abs() < r + half && dx * dx + dz * dz <= r * r
    }
}

/// Раскол этого такта: слежение показывает его в порядке оригинала —
/// облако, событие мира, уход снаряда (lingering-potion).
#[derive(Clone, Debug)]
pub struct Impact {
    pub dimension: Dimension,
    pub flying: i32,
    pub cloud: Option<i32>,
    /// Событие мира: блок, номер, данные.
    pub event: ((i32, i32, i32), i32, i32),
}

/// Все снаряды и облака сервера.
#[derive(Default)]
pub struct Thrown {
    pub flying: Vec<Flying>,
    pub clouds: Vec<Cloud>,
    /// Расколы последнего такта по измерениям.
    pub impacts: Vec<Impact>,
    /// Облака, исчезнувшие в последнем такте: отстающее слежение ещё шлёт их
    /// последний радиус (lingering-potion-feet: 0,6 на +92, потом уход).
    pub faded: Vec<Cloud>,
    /// Ракеты фейерверка (firework.rs) и их взрывы последнего такта.
    pub rockets: Vec<crate::firework::Rocket>,
    pub blasts: Vec<crate::firework::Blast>,
    /// Брошенные оки Края (ender_eye.rs) и пропавшие в последнем такте.
    pub eyes: Vec<crate::ender_eye::Eye>,
    pub eyes_gone: Vec<crate::ender_eye::Gone>,
}

impl Thrown {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn any_in(&self, dimension: Dimension) -> bool {
        self.flying.iter().any(|f| f.dimension == dimension)
            || self.clouds.iter().any(|c| c.dimension == dimension)
            || self.impacts.iter().any(|i| i.dimension == dimension)
            || self.faded.iter().any(|c| c.dimension == dimension)
            || self.rockets.iter().any(|r| r.dimension == dimension)
            || self.blasts.iter().any(|b| b.dimension == dimension)
            || self.eyes.iter().any(|e| e.dimension == dimension)
            || self.eyes_gone.iter().any(|g| g.last.dimension == dimension)
    }
}

/// Кого задел снаряд.
#[derive(Clone, Copy, PartialEq)]
enum Struck {
    Player(i32),
    Mob(i32),
}

/// Ближайшая точка ящика (`low`…`low + height`, полуширина `half`) к `at`:
/// расстояние до неё (вики, Splash Potion: «closest point of the hitbox»).
pub fn box_distance(at: (f64, f64, f64), (cx, low, cz): (f64, f64, f64), half: f64, height: f64) -> f64 {
    let dx = at.0 - at.0.clamp(cx - half, cx + half);
    let dy = at.1 - at.1.clamp(low, low + height);
    let dz = at.2 - at.2.clamp(cz - half, cz + half);

    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// Сила взрывного зелья на расстоянии `distance`: 1 при прямом попадании,
/// дальше линейно до 0 на 4 блоках (вики, Splash Potion).
pub fn splash_scale(distance: f64, direct: bool) -> f64 {
    if direct { 1.0 } else { (1.0 - distance / 4.0).max(0.0) }
}

/// Срок эффекта с силой `scale`: округление до такта, «1 second or less» —
/// не накладывается (вики, Splash Potion). `None` — не накладывать.
pub fn scaled_ticks(ticks: i32, scale: f64) -> Option<i32> {
    let scaled = (scale * f64::from(ticks) + 0.5) as i32;

    (scaled > 20).then_some(scaled)
}

/// Сила мгновенного эффекта с множителем `scale`: лечение 4·2^уровень,
/// урон 6·2^уровень (вики, Instant Health / Instant Damage), округление
/// до целого [проверить].
pub fn instant_amount(kind: EffectKind, amplifier: i32, scale: f64) -> f32 {
    let base = if kind == EffectKind::InstantHealth { 4 << amplifier.clamp(0, 20) } else { 6 << amplifier.clamp(0, 20) };

    (scale * f64::from(base) + 0.5).floor() as f32
}

/// Первая точка отрезка `from → to` в твёрдом блоке: доля пути и блок (шаг по
/// клеткам вдоль луча — точка входа на грани).
pub(crate) fn first_block(world: &World, from: (f64, f64, f64), to: (f64, f64, f64)) -> Option<(f64, (i32, i32, i32))> {
    let solid = |x: i32, y: i32, z: i32| crate::mob::solid(world.get_block(x, y, z));
    let start = [from.0, from.1, from.2];
    let step = [to.0 - from.0, to.1 - from.1, to.2 - from.2];
    let mut cell = [from.0.floor() as i32, from.1.floor() as i32, from.2.floor() as i32];

    if solid(cell[0], cell[1], cell[2]) {
        return Some((0.0, (cell[0], cell[1], cell[2])));
    }

    let mut next = [0.0f64; 3];
    let mut delta = [f64::INFINITY; 3];
    let mut dir = [0i32; 3];

    for axis in 0..3 {
        if step[axis].abs() < 1.0e-12 {
            next[axis] = f64::INFINITY;
            continue;
        }

        dir[axis] = if step[axis] > 0.0 { 1 } else { -1 };
        delta[axis] = 1.0 / step[axis].abs();
        let edge = if step[axis] > 0.0 { f64::from(cell[axis]) + 1.0 } else { f64::from(cell[axis]) };

        next[axis] = (edge - start[axis]) / step[axis];
    }

    loop {
        let axis = (0..3).min_by(|a, b| next[*a].total_cmp(&next[*b])).unwrap_or(0);
        let part = next[axis];

        if part > 1.0 {
            return None;
        }

        cell[axis] += dir[axis];
        next[axis] += delta[axis];

        if solid(cell[0], cell[1], cell[2]) {
            return Some((part, (cell[0], cell[1], cell[2])));
        }
    }
}

/// Отрезок `from → to` против ящика: доля пути до входа.
pub(crate) fn sweep(from: (f64, f64, f64), to: (f64, f64, f64), (cx, low, cz): (f64, f64, f64), half: f64, height: f64) -> Option<f64> {
    let (min, max) = ([cx - half, low, cz - half], [cx + half, low + height, cz + half]);
    let (start, step) = ([from.0, from.1, from.2], [to.0 - from.0, to.1 - from.1, to.2 - from.2]);
    let (mut near, mut far) = (0.0f64, 1.0f64);

    for axis in 0..3 {
        if step[axis].abs() < 1.0e-12 {
            if start[axis] < min[axis] || start[axis] > max[axis] {
                return None;
            }
            continue;
        }

        let (a, b) = ((min[axis] - start[axis]) / step[axis], (max[axis] - start[axis]) / step[axis]);
        let (a, b) = if a < b { (a, b) } else { (b, a) };

        near = near.max(a);
        far = far.min(b);

        if near > far {
            return None;
        }
    }

    Some(near)
}

/// Пересекаются ли два ящика (центр низа, полуширина, высота).
pub(crate) fn overlap(a: (f64, f64, f64), a_half: f64, a_height: f64, b: (f64, f64, f64), b_half: f64, b_height: f64) -> bool {
    (a.0 - b.0).abs() < a_half + b_half && (a.2 - b.2).abs() < a_half + b_half && a.1 < b.1 + b_height && b.1 < a.1 + a_height
}

/// Игрок рядом: номер, место, приседает ли, режим, опознаватель.
type Bystander = (i32, (f64, f64, f64), bool, i32, [u8; 16]);

/// Что сделать с игроком: эффект и лечение — событием мира в порядке такта,
/// урон — поручением после хода (вне захвата снарядов).
enum Deed {
    Effect(i32, Effect),
    Heal(i32, f32),
    Hurt([u8; 16], f32),
}

/// Наложить эффекты `effects` с силой `scale` (срок × `duration_scale`,
/// мгновенные × `scale`) на игрока или моба.
fn apply(deeds: &mut Vec<Deed>, mobs: &mut [crate::mob::Mob], target: Struck, uuid: [u8; 16], effects: &[Effect], scale: f64, duration_scale: f64) {
    for effect in effects {
        if effect.kind.instant() {
            let amount = instant_amount(effect.kind, effect.amplifier, scale);

            if amount <= 0.0 {
                continue;
            }

            match target {
                Struck::Player(id) => deeds.push(if effect.kind == EffectKind::InstantHealth { Deed::Heal(id, amount) } else { Deed::Hurt(uuid, amount) }),
                Struck::Mob(id) => {
                    if let Some(mob) = mobs.iter_mut().find(|mob| mob.entity_id == id) {
                        // Нежить лечится уроном и ранится лечением (вики, Instant Health).
                        let undead = matches!(mob.kind, crate::mob::Kind::Zombie | crate::mob::Kind::Skeleton);

                        if (effect.kind == EffectKind::InstantDamage) != undead {
                            mob.hurt(amount, Source::Magic, None);
                        } else {
                            mob.vitals.health = (mob.vitals.health + amount).min(mob.kind.max_health());
                        }
                    }
                }
            }
            continue;
        }

        // Мобы обычных эффектов пока не носят [проверить].
        let Struck::Player(id) = target else { continue };
        let Some(ticks) = scaled_ticks(effect.ticks_left, scale * duration_scale) else { continue };

        deeds.push(Deed::Effect(id, Effect { ticks_left: ticks, ..*effect }));
    }
}

/// Ход снарядов и облаков одного измерения за такт.
pub fn step(shared: &Shared, dimension: Dimension, world: &mut World) {
    let members: Vec<Bystander> = shared
        .players
        .lock()
        .expect("список игроков захвачен другим потоком")
        .members()
        .iter()
        .filter(|member| member.dimension == dimension)
        .map(|member| (member.entity_id, (member.position.x, member.position.y, member.position.z), member.sneaking, member.game_mode, member.uuid))
        .collect();

    let mut items = shared.items_in(dimension).lock().expect("предметы захвачены другим потоком");
    let mut thrown = shared.thrown.lock().expect("снаряды захвачены другим потоком");

    thrown.impacts.retain(|impact| impact.dimension != dimension);
    thrown.blasts.retain(|blast| blast.dimension != dimension);
    thrown.eyes_gone.retain(|gone| gone.last.dimension != dimension);

    // Дракон стреляет и дышит (mob/dragon.rs ставит заказ).
    let mut shots = Vec::new();

    for mob in items.mobs_mut().iter_mut().filter(|mob| mob.kind == crate::mob::Kind::EnderDragon) {
        if let Some((from, direction)) = mob.dragon.fireball.take() {
            shots.push(Flying::fireball(shared.next_entity_id(), dimension, from, (0.0, 0.0, 0.0), direction, mob.entity_id));
        }

        if let Some(at) = mob.dragon.breath_at.take() {
            let mut cloud = Cloud::dragon_breath(shared.next_entity_id(), dimension, at, mob.entity_id);

            // Дыхание сидящего дракона: облако у пасти, не растёт [проверить].
            cloud.per_tick = 0.0;
            cloud.lag = false;
            thrown.clouds.push(cloud);
        }
    }

    thrown.flying.extend(shots);

    // Облака, исчезнувшие в прошлом такте, слежению больше не нужны.
    thrown.faded.retain(|cloud| cloud.dimension != dimension);

    if !thrown.any_in(dimension) {
        return;
    }

    // Отстающее слежение за облаками — до их хода (lingering-potion-feet:
    // радиус прошлого такта раньше пакета эффекта).
    world.note_thrown_tracker(true);

    let mut deeds: Vec<Deed> = Vec::new();
    let mut gone: Vec<i32> = Vec::new();
    let mut new_clouds: Vec<Cloud> = Vec::new();
    let mut impacts: Vec<Impact> = Vec::new();
    let height = |sneaking: bool| if sneaking { 1.5 } else { 1.8 };

    for flying in thrown.flying.iter_mut().filter(|f| f.dimension == dimension) {
        if !world.has_chunk((flying.x.floor() as i32).div_euclid(16), (flying.z.floor() as i32).div_euclid(16)) {
            continue;
        }

        flying.prev = ((flying.x, flying.y, flying.z), (flying.vx, flying.vy, flying.vz), flying.rot);
        flying.age += 1;

        let fireball = flying.kind == Kind::DragonFireball;

        // Такт полёта: у зелья тяжесть, сопротивление, ход; у шара — разгон,
        // сопротивление, ход.
        if fireball {
            flying.vx = (flying.vx + flying.push.0 * FIREBALL_POWER) * FIREBALL_DRAG;
            flying.vy = (flying.vy + flying.push.1 * FIREBALL_POWER) * FIREBALL_DRAG;
            flying.vz = (flying.vz + flying.push.2 * FIREBALL_POWER) * FIREBALL_DRAG;
        } else {
            flying.vy -= GRAVITY;
            flying.vx *= DRAG;
            flying.vy *= DRAG;
            flying.vz *= DRAG;
        }

        let (half, tall) = if fireball { (0.5, 1.0) } else { (POTION_HALF, 2.0 * POTION_HALF) };
        let from = (flying.x, flying.y, flying.z);
        let to = (flying.x + flying.vx, flying.y + flying.vy, flying.z + flying.vz);
        let centre = |p: (f64, f64, f64)| (p.0, p.1 + tall / 2.0, p.2);

        // Бросивший не задевается, пока зелье в нём.
        if flying.in_owner
            && !members.iter().any(|m| m.0 == flying.owner && overlap(from, half, tall, m.1, 0.3, height(m.2)))
        {
            flying.in_owner = false;
        }

        let mut best: Option<(f64, Struck)> = None;

        for (id, at, sneaking, mode, _) in &members {
            if *mode == 3 || (*id == flying.owner && flying.in_owner) {
                continue;
            }

            if let Some(part) = sweep(centre(from), centre(to), *at, 0.3 + half, height(*sneaking) + tall).map(|p| p.max(0.0))
                && best.is_none_or(|(known, _)| part < known)
            {
                best = Some((part, Struck::Player(*id)));
            }
        }

        for mob in items.mobs().iter() {
            if mob.is_dead() || mob.entity_id == flying.owner || (fireball && mob.kind == crate::mob::Kind::EnderDragon) {
                continue;
            }

            let (width, mob_height) = mob.size();

            if let Some(part) = sweep(centre(from), centre(to), (mob.x, mob.y - tall / 2.0, mob.z), width / 2.0 + half, mob_height + tall)
                && best.is_none_or(|(known, _)| part < known)
            {
                best = Some((part, Struck::Mob(mob.entity_id)));
            }
        }

        let block = first_block(world, from, to);
        let entity_first = match (best, block) {
            (Some((part, _)), Some((block_part, _))) => part <= block_part,
            (Some(_), None) => true,
            _ => false,
        };

        // Попадание в сущность: место раскола — где зелье было до хода
        // (splash-potion: событие на высоте 2 при ударе о голову с 2,13).
        let hit: Option<((f64, f64, f64), Option<Struck>)> = if entity_first {
            best.map(|(_, target)| (from, Some(target)))
        } else {
            block.map(|(part, _)| ((from.0 + (to.0 - from.0) * part, from.1 + (to.1 - from.1) * part, from.2 + (to.2 - from.2) * part), None))
        };

        let Some((at, struck)) = hit else {
            flying.x = to.0;
            flying.y = to.1;
            flying.z = to.2;

            // Поворот догоняет полёт на пятую часть (splash-potion: наклон
            // 38 из 256 на +10 при броске вверх).
            if !fireball {
                let (yaw, pitch) = angles((flying.vx, flying.vy, flying.vz));
                let turn = (yaw - flying.rot.0 + 540.0).rem_euclid(360.0) - 180.0;

                flying.rot = (flying.rot.0 + turn * 0.2, flying.rot.1 + (pitch - flying.rot.1) * 0.2);
            }

            if flying.y < -128.0 || flying.age > 1200 && fireball {
                gone.push(flying.entity_id);
            }

            continue;
        };

        gone.push(flying.entity_id);

        let block_at = (at.0.floor() as i32, at.1.floor() as i32, at.2.floor() as i32);

        if fireball {
            // Шар разбивается облаком дыхания (dragon-fireball: событие 2006/1
            // в блоке удара, облако в точке удара).
            let cloud = Cloud::dragon_breath(shared.next_entity_id(), dimension, at, flying.owner);

            impacts.push(Impact { dimension, flying: flying.entity_id, cloud: Some(cloud.entity_id), event: (block_at, 2006, 1) });
            new_clouds.push(cloud);
            continue;
        }

        let effects = flying.effects();
        let color = opaque(potion_color(&effects));
        let instant = effects.iter().any(|effect| effect.kind.instant());

        if flying.kind == Kind::Lingering {
            let cloud = Cloud::lingering(shared.next_entity_id(), dimension, at, effects, flying.owner, struck.is_none());

            impacts.push(Impact { dimension, flying: flying.entity_id, cloud: Some(cloud.entity_id), event: (block_at, 2002, color) });
            new_clouds.push(cloud);
            continue;
        }

        // Взрывное: всем в ящике 8,25 × 4,25 и в 4 блоках (вики, Splash Potion).
        if effects.is_empty() {
            // Вода гасит огонь в блоке удара и в четырёх вокруг (вики).
            if let Some(fire) = crate::blocks::state_by_name("fire") {
                let soul = crate::blocks::state_by_name("soul_fire").unwrap_or(fire);

                for (dx, dz) in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1)] {
                    for dy in [0, 1] {
                        let (x, y, z) = (block_at.0 + dx, block_at.1 + dy, block_at.2 + dz);
                        let state = world.get_block(x, y, z);

                        if state == fire || state == soul {
                            world.set_block(x, y, z, crate::world::AIR);
                        }
                    }
                }
            }
        } else {
            for (id, pos, sneaking, mode, uuid) in &members {
                if *mode == 3 || (pos.0 - at.0).abs() > 4.125 + 0.3 || pos.1 + height(*sneaking) < at.1 - 2.125 || pos.1 > at.1 + 2.125 || (pos.2 - at.2).abs() > 4.125 + 0.3 {
                    continue;
                }

                let distance = box_distance(at, *pos, 0.3, height(*sneaking));

                if distance > 4.0 {
                    continue;
                }

                let scale = splash_scale(distance, struck == Some(Struck::Player(*id)));

                apply(&mut deeds, &mut [], Struck::Player(*id), *uuid, &effects, scale, 1.0);
            }

            let targets: Vec<(i32, f64)> = items
                .mobs()
                .iter()
                .filter(|mob| !mob.is_dead() && !matches!(mob.kind, crate::mob::Kind::EndCrystal | crate::mob::Kind::EnderDragon))
                .filter_map(|mob| {
                    let (width, mob_height) = mob.size();
                    let distance = box_distance(at, (mob.x, mob.y, mob.z), width / 2.0, mob_height);

                    (distance <= 4.0).then_some((mob.entity_id, splash_scale(distance, struck == Some(Struck::Mob(mob.entity_id)))))
                })
                .collect();

            for (id, scale) in targets {
                apply(&mut deeds, items.mobs_mut(), Struck::Mob(id), [0; 16], &effects, scale, 1.0);
            }
        }

        impacts.push(Impact { dimension, flying: flying.entity_id, cloud: None, event: (block_at, if instant { 2007 } else { 2002 }, color) });
    }

    thrown.flying.retain(|f| !gone.contains(&f.entity_id));

    // Ракеты фейерверка: полёт и взрывы (firework.rs).
    let mut strikes = Vec::new();

    if thrown.rockets.iter().any(|r| r.dimension == dimension) {
        let bystanders: Vec<crate::firework::Bystander> = members.iter().map(|m| (m.0, m.1, m.2, m.3)).collect();
        let mut rockets = std::mem::take(&mut thrown.rockets);
        let (mut blasts, mut wounds) = (Vec::new(), Vec::new());

        crate::firework::step(world, &mut rockets, dimension, &bystanders, items.mobs(), &mut blasts, &mut wounds);
        thrown.rockets = rockets;

        // Событие 17 — раньше урона, уход ракеты — со слежением в конце хода.
        for blast in &blasts {
            world.note_rocket_blast(blast.rocket);
        }

        thrown.blasts.extend(blasts);

        for (rocket, wound) in wounds {
            match wound.target {
                crate::firework::Struck::Mob(id) => {
                    if let Some(mob) = items.mobs_mut().iter_mut().find(|mob| mob.entity_id == id) {
                        let away = (mob.x - wound.from.0, mob.z - wound.from.2);
                        let length = away.0.hypot(away.1);
                        let push = (length > 1.0e-6).then(|| crate::mob::Push { dir: (away.0 / length, away.1 / length), strength: crate::mob::BASE_KNOCKBACK, cause: rocket.owner });

                        if rocket.owner != 0 {
                            mob.credit_player();
                        }

                        mob.hurt(wound.damage, crate::firework::SOURCE, push);
                    }
                }
                crate::firework::Struck::Player(id) => strikes.push(crate::combat::PlayerStrike {
                    victim: id,
                    attacker: rocket.owner,
                    attacker_uuid: rocket.owner_uuid,
                    attacker_name: rocket.owner_name.clone(),
                    from: wound.from,
                    yaw: rocket.rot.0,
                    damage: wound.damage,
                    source: crate::firework::SOURCE,
                    extra_knockback: 0.0,
                    weapon: None,
                    disables_shield_ticks: 0,
                    breach_scale: 1.0,
                    fire_ticks: 0,
                    thorns_slots: Vec::new(),
                    ignores_shield: false,
                }),
            }
        }
    }

    for strike in strikes {
        world.note_player_strike(strike);
    }

    // Оки Края: полёт, через 80 тактов — предмет (80 %) или раскол (вики, Eye
    // of Ender; чёрный ящик, eye-of-ender-far).
    let mut eyes_gone = Vec::new();

    thrown.eyes.retain_mut(|eye| {
        if eye.dimension != dimension || !world.has_chunk((eye.x.floor() as i32).div_euclid(16), (eye.z.floor() as i32).div_euclid(16)) {
            return true;
        }

        if !eye.step() {
            return true;
        }

        let shattered = world.random_unit() >= crate::ender_eye::SURVIVE;

        eyes_gone.push(crate::ender_eye::Gone { last: eye.clone(), shattered });
        false
    });

    // Упавшее око — предмет с обычным толчком выпавшего: вбок до ±0,1,
    // вверх 0,2 (eye-of-ender-far: скорость 0,058/0,200/0,076).
    for gone in &eyes_gone {
        if !gone.shattered {
            let eye = &gone.last;
            let velocity = (f64::from(world.random_unit()) * 0.2 - 0.1, 0.2, f64::from(world.random_unit()) * 0.2 - 0.1);

            items.drop_item(shared.next_entity_id(), eye.stack, (eye.x, eye.y, eye.z), velocity, 0);
        }
    }

    thrown.eyes_gone.extend(eyes_gone);

    // Облака: ожидание, таяние, наложение раз в 5 тактов [проверить период].
    let mut faded = Vec::new();

    for cloud in thrown.clouds.iter_mut().filter(|c| c.dimension == dimension) {
        cloud.prev = (cloud.radius, cloud.waiting);
        cloud.age += 1;
        cloud.waiting = cloud.age < cloud.wait;

        if cloud.age >= cloud.wait + cloud.duration {
            faded.push(cloud.entity_id);
            continue;
        }

        if cloud.waiting {
            continue;
        }

        cloud.radius += cloud.per_tick;

        if cloud.radius < 0.5 {
            faded.push(cloud.entity_id);
            continue;
        }

        if !cloud.age.is_multiple_of(5) {
            continue;
        }

        let age = cloud.age;

        cloud.victims.retain(|(_, until)| *until > age);

        let mut touched: Vec<(Struck, [u8; 16])> = Vec::new();

        for (id, pos, sneaking, mode, uuid) in &members {
            if *mode != 3 && *mode != 1 && cloud.touches(*pos, 0.3, height(*sneaking)) && !cloud.victims.iter().any(|(v, _)| v == id) {
                touched.push((Struck::Player(*id), *uuid));
            }
        }

        for mob in items.mobs() {
            let (width, mob_height) = mob.size();

            if !mob.is_dead() && mob.entity_id != cloud.owner && cloud.touches((mob.x, mob.y, mob.z), width / 2.0, mob_height) && !cloud.victims.iter().any(|(v, _)| *v == mob.entity_id) {
                touched.push((Struck::Mob(mob.entity_id), [0; 16]));
            }
        }

        for (target, uuid) in touched {
            let id = match target {
                Struck::Player(id) | Struck::Mob(id) => id,
            };

            cloud.victims.push((id, age + cloud.delay));
            // Облако даёт четверть срока и половину мгновенного (вики,
            // Area Effect Cloud).
            apply(&mut deeds, items.mobs_mut(), target, uuid, &cloud.effects, 0.5, 0.5);

            if cloud.on_use != 0.0 {
                cloud.radius += cloud.on_use;

                if cloud.radius < 0.5 {
                    faded.push(cloud.entity_id);
                    break;
                }
            }
        }
    }

    let (kept, gone_clouds): (Vec<Cloud>, Vec<Cloud>) = std::mem::take(&mut thrown.clouds).into_iter().partition(|c| !faded.contains(&c.entity_id));

    thrown.clouds = kept;
    thrown.faded.extend(gone_clouds);
    thrown.clouds.extend(new_clouds);
    thrown.impacts.extend(impacts);

    drop(thrown);
    drop(items);

    let mut hurts = Vec::new();

    for deed in deeds {
        match deed {
            Deed::Effect(id, effect) => world.note_potion_effect(id, effect),
            Deed::Heal(id, amount) => world.note_potion_heal(id, amount),
            Deed::Hurt(uuid, amount) => hurts.push((uuid, amount)),
        }
    }

    // Слежение — в конце хода: раскол, облако и уход снаряда в порядке оригинала.
    world.note_thrown_tracker(false);

    if !hurts.is_empty() {
        let mut players = shared.players.lock().expect("список игроков захвачен другим потоком");

        for (uuid, amount) in hurts {
            players.order(&uuid, crate::players::Order::Hurt { amount, source: Source::Magic });
        }
    }
}

/// Стеклянный пузырёк набирает дыхание дракона: облако рядом с игроком
/// (ящик игрока, расширенный на 2) теряет 0,5 радиуса (вики, Dragon's
/// Breath: «right-clicking … with a glass bottle») [проверить числа].
pub fn bottle_breath(shared: &Shared, dimension: Dimension, at: (f64, f64, f64)) -> bool {
    let mut thrown = shared.thrown.lock().expect("снаряды захвачены другим потоком");
    let Some(cloud) = thrown.clouds.iter_mut().find(|c| c.dimension == dimension && c.dragon && !c.waiting && c.touches(at, 0.3 + 2.0, 1.8 + 2.0)) else {
        return false;
    };

    cloud.radius -= 0.5;

    if cloud.radius < 0.5 {
        let id = cloud.entity_id;

        thrown.clouds.retain(|c| c.entity_id != id);
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_potion_thrown_up_goes_straight_up_and_level_is_lifted_twenty_degrees() {
        let mut none = || 0.0;
        // Взгляд прямо вверх: поправка не наклоняет (splash-potion: 0; 0,501; 0).
        let up = throw_velocity(0.0, -90.0, POTION_SPEED, &mut none);

        assert!(up.0.abs() < 1e-9 && up.2.abs() < 1e-9 && (up.1 - 0.5).abs() < 1e-9);

        // По горизонту на −z (yaw 180): вверх на 19° (splash-potion-far: 0,164; −0,467).
        let level = throw_velocity(180.0, 0.0, POTION_SPEED, &mut none);

        assert!((level.1 - 0.1618).abs() < 0.002 && (level.2 + 0.4731).abs() < 0.002);
    }

    #[test]
    fn the_flight_matches_the_recorded_tenth_tick() {
        // splash-potion: из скорости 0,5014 вверх за 10 ходов сдвиг 8618/4096
        // и скорость −0,02 — порядок «тяжесть, сопротивление, ход».
        let (mut y, mut vy) = (0.0f64, 0.5014344137215405f64);

        for _ in 0..10 {
            vy -= GRAVITY;
            vy *= DRAG;
            y += vy;
        }

        assert!(((y * 4096.0).round() - 8619.0).abs() <= 1.0);
        assert!((vy + 0.0198).abs() < 1e-3);
    }

    #[test]
    fn splash_strength_falls_off_over_four_blocks() {
        assert_eq!(splash_scale(0.0, false), 1.0);
        assert_eq!(splash_scale(2.0, false), 0.5);
        assert_eq!(splash_scale(5.0, false), 0.0);
        assert_eq!(splash_scale(3.0, true), 1.0);
        // Быстрота 3600 на половине силы — 1800; на 1/200 — 18 тактов: не накладывается.
        assert_eq!(scaled_ticks(3600, 0.5), Some(1800));
        assert_eq!(scaled_ticks(3600, 0.005), None);
        // Лечение II целиком — 8 (splash-potion-heal: здоровье 6 → 14), вполсилы — 4.
        assert_eq!(instant_amount(EffectKind::InstantHealth, 1, 1.0), 8.0);
        assert_eq!(instant_amount(EffectKind::InstantHealth, 1, 0.5), 4.0);
        // Дыхание дракона: урон II вполсилы — 6 (вики, Dragon Fireball: «6HP»).
        assert_eq!(instant_amount(EffectKind::InstantDamage, 1, 0.5), 6.0);
        // Ближайшая точка ящика: под ногами — 0, сбоку — до грани.
        assert_eq!(box_distance((0.0, 0.0, 0.0), (0.0, 0.0, 0.0), 0.3, 1.8), 0.0);
        assert!((box_distance((2.3, 1.0, 0.0), (0.0, 0.0, 0.0), 0.3, 1.8) - 2.0).abs() < 1e-9);
    }

    #[test]
    fn potion_colours_are_opaque_effect_colours() {
        let speed = crate::survival::potion_effects(crate::survival::potion_number("swiftness").expect("есть"));

        // splash-potion: 2002 / −13374465.
        assert_eq!(opaque(potion_color(&speed)), -13374465);
        // lingering-potion: регенерация −3318613.
        let regen = crate::survival::potion_effects(crate::survival::potion_number("regeneration").expect("есть"));

        assert_eq!(opaque(potion_color(&regen)), -3318613);
        assert_eq!(potion_color(&[]), WATER_COLOR);
    }

    #[test]
    fn a_lingering_cloud_waits_then_shrinks_and_dragon_breath_grows() {
        let mut cloud = Cloud::lingering(1, Dimension::Overworld, (0.0, 0.0, 0.0), Vec::new(), 0, true);

        assert_eq!((cloud.radius, cloud.wait, cloud.duration, cloud.delay), (3.0, 10, 600, 20));
        assert!(cloud.touches((2.0, 0.0, 0.0), 0.3, 1.8));
        assert!(!cloud.touches((0.0, 2.1, 0.0), 0.3, 1.8), "над облаком (lingering-potion: облако на голове робота)");

        cloud.radius += cloud.per_tick;
        assert!((cloud.radius - 2.995).abs() < 1e-6);

        let breath = Cloud::dragon_breath(2, Dimension::Overworld, (0.0, 0.0, 0.0), 0);

        assert!(breath.dragon && breath.wait == 20 && (breath.per_tick - 4.0 / 600.0).abs() < 1e-7);
        assert_eq!(breath.effects[0].kind, EffectKind::InstantDamage);
    }

    #[test]
    fn a_ray_enters_a_block_on_its_face() {
        let mut world = World::in_memory();
        let stone = crate::blocks::state_by_name("stone").expect("камень есть");

        world.set_block(3, -1, -2, stone);

        // Сверху вниз на камень: вход на верхней грани, y = 0 (splash-potion-far:
        // событие в блоке (3, 0, −2)).
        let (part, cell) = first_block(&world, (3.5, 0.06, -1.5), (3.5, -0.36, -1.9)).expect("попал");

        assert_eq!(cell, (3, -1, -2));
        assert!((0.06 + (-0.42) * part).abs() < 1e-9);
    }
}
