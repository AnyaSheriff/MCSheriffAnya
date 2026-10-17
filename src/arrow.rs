// Стрелы: одна сущность на всех — и выпущенные игроками из лука, и выпущенные
// скелетами. Полёт, попадание, застревание, подбор.
//
// Правила — вики (Arrow, Bow, Power, Punch, Flame): тяжесть 0,05 за такт,
// сопротивление воздуха 0,99 (воды 0,6), урон — `ceil(скорость · базовый)`,
// базовый 2, «сила» прибавляет 0,5 за уровень и 0,5, критическая
// (полностью натянутая) стрела добавляет случайные 0…(урон/2 + 1); в
// блоке стрела живёт минуту. Числа, которых на вики нет, сняты чёрным
// ящиком (pvp-bow). Что не сверено, помечено «[проверить]».
//
// Стрелы хранятся общим списком в `Shared::arrows` (у каждой своё
// измерение); такт двигает их из tick.rs, подключения рассказывают о них
// клиентам (network/play/bow.rs), Bedrock — bedrock/world.rs. Стрела скелета
// отличается тем, кто стрелок (`Shooter::Mob`): она ранит
// их как моб (пакеты урона уходят из `vitals::mob_hit`) и не подбирается;
// выпущенная в такт мобов, она начинает полёт со следующего такта (`fresh`).

use crate::combat::PlayerStrike;
use crate::dimension::Dimension;
use crate::inventory::Stack;
use crate::shared::Shared;
use crate::survival::Source;
use crate::world::World;

/// Номер вида сущности «стрела» (minecraft-data 26.1, entities.json).
pub const ARROW_TYPE: i32 = 6;

/// Перелом полёта для клиентов: возраст, место, скорость, флаги, поворот.
pub type Resync = (u32, (f64, f64, f64), (f64, f64, f64), u8, (f32, f32));

/// Номер вида сущности «трезубец» (minecraft-data 26.1, entities.json).
pub const TRIDENT_TYPE: i32 = 135;

/// Урон брошенного трезубца: 8, от скорости не зависит (вики, Trident).
pub const TRIDENT_DAMAGE: f32 = 8.0;

/// Скорость броска трезубца: 2,5 блока за такт (вики, Projectile) [проверить].
pub const TRIDENT_SPEED: f64 = 2.5;

/// Кто выпустил стрелу.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shooter {
    Player,
    /// Моб (скелет): ключ его имени для сообщения о смерти.
    Mob { killer: &'static str },
}

/// Тяжесть и сопротивление стрелы (вики, Arrow).
const GRAVITY: f64 = 0.05;
const AIR_DRAG: f64 = 0.99;
const WATER_DRAG: f64 = 0.6;

/// Сколько тактов стрела лежит в блоке, прежде чем исчезнуть: минута.
const GROUND_LIFE: u32 = 1200;

/// Сколько тактов после появления стрела не задевает стрелка.
const OWNER_SAFE: u32 = 5;

/// Сколько тактов после попадания в блок стрела ещё нельзя подобрать. [проверить]
const PICKUP_DELAY: u32 = 7;

/// Насколько по горизонтали от оси игрока стрела ещё подбирается: полуширина
/// игрока 0,3 + запас подбора 1,0 + полуширина стрелы 0,25 [проверить].
const PICKUP_REACH: f64 = 1.55;

/// Сколько тактов трезубец с «верностью» лежит в блоке, прежде чем полететь
/// к хозяину [проверить].
const LOYALTY_GROUND_WAIT: u32 = 4;

/// Можно ли подобрать стрелу (вики, Arrow: «Pickup»).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pickup {
    Yes,
    /// Выпущена в творческом режиме: подбирают только творческие игроки.
    CreativeOnly,
    No,
}

/// Стрела в полёте или в блоке.
#[derive(Clone, Debug)]
pub struct Arrow {
    pub entity_id: i32,
    pub uuid: [u8; 16],
    pub dimension: Dimension,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub vx: f64,
    pub vy: f64,
    pub vz: f64,
    pub no_gravity: bool,
    /// Кто выпустил (номер сущности, 0 — никто), его имя и опознаватель.
    pub owner: i32,
    pub owner_uuid: [u8; 16],
    pub owner_name: String,
    pub shooter: Shooter,
    /// Эффекты наконечника: готовый срок попадания (вики, Tipped Arrow).
    /// [Bedrock: потом] Цвет стрелы и частицы ещё не передаются Bedrock.
    pub effects: Vec<crate::survival::Effect>,
    /// Исходное зелье для подбора стрелы игрока; собственный эффект моба — None.
    pub potion: Option<u16>,
    /// Предмет наконечника для подбора и spectral entity (вики, Arrow) [проверить].
    pub ammo: Option<Stack>,
    /// Полностью натянутая стрела бьёт сильнее и летит с искрами.
    pub crit: bool,
    /// Уровни «силы» и «отдачи» лука, «горящая стрела».
    pub power: i32,
    pub punch: i32,
    pub flame: bool,
    /// «Пробивание» арбалета: сквозь стольких ещё пройдёт; кого уже задела
    /// (второй раз не бьёт) (вики, Piercing).
    pub pierce_left: i32,
    pub pierced: Vec<i32>,
    pub killed: Vec<&'static str>,
    /// Выпущена из арбалета: попадание звучит иначе (чёрный ящик,
    /// crossbow-shoot: item.crossbow.hit).
    pub crossbow: bool,
    /// Брошенный трезубец: сам предмет (с износом и зачарованиями) — он и
    /// вернётся в инвентарь. `None` — обычная стрела.
    pub trident: Option<Stack>,
    /// Уровень «верности» трезубца.
    pub loyalty: i32,
    /// Трезубец уже ранил: сущностей больше не задевает (вики, Trident:
    /// «only damage one entity»).
    pub dealt: bool,
    /// Трезубец с «верностью» летит к хозяину сквозь блоки.
    pub returning: bool,
    /// Подобрана: исчезнет в следующем такте (клиентам её убирает сам подбор).
    pub collected: bool,
    /// В этом такте полёт переломился (отскок, пробитая цель, возврат):
    /// слежение оригинала сообщает об этом на такт позже.
    pub impulse: bool,
    /// Что сообщить клиентам о переломе: в каком возрасте, место, скорость и
    /// флаги к концу такта перелома.
    pub resync: Option<Resync>,
    /// Поворот трезубца (угол, наклон): после отскока он не следует за
    /// скоростью, а догоняет её на пятую часть за такт [проверить].
    pub rot: Option<(f32, f32)>,
    /// Блок, в котором сидит вставшая стрела (трезубец отступает от него).
    pub ground_block: (i32, i32, i32),
    pub pickup: Pickup,
    pub in_ground: bool,
    pub ground_ticks: u32,
    pub age: u32,
    /// Только что выпущена мобом: первый такт пропускает (выпущенная в такт
    /// мобов начинает полёт со следующего).
    pub fresh: bool,
    /// Призвана командой `summon`: звука броска у трезубца нет (gaps2-projectiles-no-gravity).
    pub summoned: bool,
    /// Как дошла до клиентов: попала в блок (для звука на следующей рассылке).
    pub landed: bool,
    /// Место и скорость при появлении: клиенту сперва уходит оно.
    pub start: ((f64, f64, f64), (f64, f64, f64)),
}

/// Зелье на стреле игрока: длительность в восемь раз меньше питьевой
/// (вики, Tipped Arrow: «1/8 the time»). Стрелы мобов задают срок напрямую.
pub fn tipped_effects(potion: Option<u16>) -> Vec<crate::survival::Effect> {
    potion.map(crate::survival::potion_effects).unwrap_or_default().into_iter().map(|mut effect| {
        if effect.ticks_left != crate::survival::INFINITE { effect.ticks_left = (effect.ticks_left / 8).max(1); }
        effect
    }).collect()
}

/// Вики, Spectral Arrow: «Glowing ... 10 seconds». [Bedrock: потом]
pub fn ammunition_effects(ammo: Option<Stack>, potion: Option<u16>) -> Vec<crate::survival::Effect> {
    if ammo.is_some_and(|stack| crate::blocks::item_name(stack.item) == Some("spectral_arrow")) {
        vec![crate::survival::Effect { kind: crate::survival::EffectKind::Glowing, amplifier: 0, ticks_left: 200, ambient: false, visible: true, show_icon: true }]
    } else if let Some(stack) = ammo {
        stack.extras().potion_effects().into_iter().map(|mut effect| {
            if effect.ticks_left != crate::survival::INFINITE { effect.ticks_left = (effect.ticks_left / 8).max(1); }
            effect
        }).collect()
    } else { tipped_effects(potion) }
}

/// Эффекты стрелы на мобе; нежить не принимает яд/регенерацию (вики, Undead).
pub fn apply_effects(mob: &mut crate::mob::Mob, effects: &[crate::survival::Effect]) {
    use crate::survival::EffectKind;
    let undead = crate::combat::mob_traits(mob.kind.name()).0;
    for effect in effects {
        if !mob.accepts_effect(effect.kind) { continue; }
        if effect.kind.instant() {
            let kind = crate::thrown::instant_kind(effect.kind, undead);
            let amount = crate::thrown::instant_amount(kind, effect.amplifier, 1.0);
            if kind == EffectKind::InstantDamage {
                mob.hurt(amount, Source::Magic, None);
            } else {
                mob.vitals.health = (mob.vitals.health + amount).min(mob.vitals.max_health);
            }
        } else {
            mob.vitals.give_effect_shown(*effect);
        }
    }
}

impl Arrow {
    /// Подбирается тот же наконечник, что был выпущен.
    pub fn pickup_stack(&self) -> Stack {
        if let Some(ammo) = self.ammo { return ammo.with_count(1); }
        let name = if self.potion.is_some() { "tipped_arrow" } else { "arrow" };
        Stack::new(crate::blocks::item_named(name).unwrap_or(0), 1).with_extras(crate::stackdata::Data { potion: self.potion, ..Default::default() })
    }

    /// Стрела выпущена мобом.
    pub fn is_mob_shot(&self) -> bool {
        matches!(self.shooter, Shooter::Mob { .. })
    }

    /// Зачарования лука остаются со стрелой после смены оружия стрелком.
    pub fn enchant_from_bow(&mut self, data: &crate::stackdata::Data) {
        self.power = data.level("power");
        self.punch = data.level("punch");
        self.flame = data.level("flame") > 0;
    }

    /// Стрела скелета: из точки `at` со скоростью `velocity`.
    pub fn from_mob(entity_id: i32, dimension: Dimension, at: (f64, f64, f64), velocity: (f64, f64, f64), owner: i32, killer: &'static str) -> Self {
        Self {
            entity_id,
            uuid: crate::items::uuid_for(entity_id),
            dimension,
            x: at.0,
            y: at.1,
            z: at.2,
            vx: velocity.0,
            vy: velocity.1,
            vz: velocity.2,
            no_gravity: false,
            owner,
            owner_uuid: [0; 16],
            owner_name: String::new(),
            shooter: Shooter::Mob { killer },
            effects: Vec::new(),
            potion: None,
            ammo: None,
            crit: false,
            power: 0,
            punch: 0,
            flame: false,
            pierce_left: 0,
            pierced: Vec::new(),
            killed: Vec::new(),
            crossbow: false,
            trident: None,
            loyalty: 0,
            dealt: false,
            returning: false,
            collected: false,
            impulse: false,
            resync: None,
            rot: None,
            ground_block: (0, 0, 0),
            pickup: Pickup::No,
            in_ground: false,
            ground_ticks: 0,
            age: 0,
            fresh: true,
            summoned: false,
            landed: false,
            start: (at, velocity),
        }
    }

    /// Звук попадания в сущность: у стрелы арбалета и трезубца свои.
    pub fn hit_sound(&self) -> &'static str {
        if self.trident.is_some() {
            "minecraft:item.trident.hit"
        } else if self.crossbow {
            "minecraft:item.crossbow.hit"
        } else {
            "minecraft:entity.arrow.hit"
        }
    }

    /// Звук попадания в блок: у трезубца свой (вики, Trident: hit_ground)
    /// [проверить для арбалета].
    pub fn ground_sound(&self) -> &'static str {
        if self.trident.is_some() { "minecraft:item.trident.hit_ground" } else { self.hit_sound() }
    }

    /// Флаги сущности (свойство 8): 1 — критическая, 2 — летит сквозь блоки.
    pub fn flags(&self) -> u8 {
        u8::from(self.crit) | if self.returning { 2 } else { 0 }
    }

    /// Вики, Flame/Punch: поджог на 5 секунд и отдача 0,6 за уровень.
    /// Огонь ставится до расчёта loot в hurt_burning, здесь отдача [проверить].
    pub fn hit_effects(&self, mob: &mut crate::mob::Mob) {
        let length = self.vx.hypot(self.vz);
        if self.punch > 0 && length > 1.0e-9 && !matches!(mob.kind, crate::mob::Kind::IronGolem | crate::mob::Kind::Shulker) {
            let resistance = if mob.kind == crate::mob::Kind::Hoglin { 0.6 } else { 0.0 };
            let push = 0.6 * f64::from(self.punch) * (1.0 - resistance);
            mob.vx += self.vx / length * push;
            mob.vy += 0.1 * (1.0 - resistance);
            mob.vz += self.vz / length * push;
        }
    }

    pub fn spectral(&self) -> bool {
        self.ammo.is_some_and(|stack| crate::blocks::item_name(stack.item) == Some("spectral_arrow"))
    }

    /// Вид сущности для клиента.
    pub fn entity_type(&self) -> i32 {
        if self.trident.is_some() { TRIDENT_TYPE } else if self.spectral() { 123 } else { ARROW_TYPE }
    }

    /// Куда смотрит стрела: по скорости, а у вставшей — по скорости выстрела.
    pub fn facing(&self) -> (f32, f32) {
        if let Some(rot) = self.rot {
            return rot;
        }

        if self.vx == 0.0 && self.vy == 0.0 && self.vz == 0.0 {
            let (svx, svy, svz) = self.start.1;

            return Arrow { vx: svx, vy: svy, vz: svz, ..self.clone() }.angles();
        }

        self.angles()
    }

    /// Угол по сторонам света и наклон по скорости (для пакетов). У снарядов
    /// угол без знака минус: `yaw = atan2(vx, vz)` (чёрный ящик, skeleton-arrow:
    /// стрела на −x — поворот −65).
    pub fn angles(&self) -> (f32, f32) {
        angles_of((self.vx, self.vy, self.vz))
    }

    /// Место и скорость до последнего хода: сверка слежения каждые 20 тактов
    /// у оригинала видит стрелу без хода этого такта (чёрный ящик,
    /// tails-arrow-no-gravity: сдвиг за 20 тактов — 20 ходов, а не 21).
    pub fn before_step(&self) -> ((f64, f64, f64), (f64, f64, f64)) {
        let gravity = if self.no_gravity { 0.0 } else { GRAVITY };
        let before = (self.vx / AIR_DRAG, (self.vy + gravity) / AIR_DRAG, self.vz / AIR_DRAG);

        ((self.x - before.0, self.y - before.1, self.z - before.2), before)
    }

    /// Сколько тактов прошло с появления: стрела игрока делает первый ход в
    /// такт выстрела (возраст 1), стрела моба — со следующего (возраст 0).
    pub fn lived(&self) -> u32 {
        if self.is_mob_shot() { self.age } else { self.age.saturating_sub(1) }
    }
}

/// Угол и наклон по скорости: `yaw = atan2(vx, vz)`, `pitch = atan2(vy, по
/// горизонтали)`, градусы.
pub fn angles_of((vx, vy, vz): (f64, f64, f64)) -> (f32, f32) {
    let horizontal = vx.hypot(vz);

    (vx.atan2(vz).to_degrees() as f32, vy.atan2(horizontal).to_degrees() as f32)
}

/// Поворот `from` на пятую часть к `to` (угол — кратчайшим путём): так
/// трезубец догоняет свой полёт (чёрный ящик, trident-throw: после отскока
/// угол 169° → 177° за 8 тактов, наклон 0 → −70°).
pub fn turn_toward((yaw, pitch): (f32, f32), (to_yaw, to_pitch): (f32, f32)) -> (f32, f32) {
    let turn = (to_yaw - yaw + 540.0).rem_euclid(360.0) - 180.0;

    (yaw + turn * 0.2, pitch + (to_pitch - pitch) * 0.2)
}

/// Отступ вставшего трезубца от точки удара: 0,05 назад по каждой оси, по
/// знаку скорости, а не по её направлению (чёрный ящик, trident-throw: после
/// отскока при скорости (≈0; −0,4; −0,024) сел на 0,05 выше пола, на 0,05
/// дальше по z и на 0,05 в сторону по x, хотя по x почти не летел).
pub fn ground_backoff((vx, vy, vz): (f64, f64, f64)) -> (f64, f64, f64) {
    let sign = |value: f64| if value > 0.0 { 1.0 } else if value < 0.0 { -1.0 } else { 0.0 };

    (-sign(vx) * 0.05, -sign(vy) * 0.05, -sign(vz) * 0.05)
}

/// Все стрелы сервера.
#[derive(Default, Clone)]
pub struct Arrows {
    pub list: Vec<Arrow>,
    /// Попавшие в первый ход: показываем появление и уход в одной рассылке.
    /// [проверить] Порядок относительно damage_event.
    pub retired: Vec<Arrow>,
}

impl Arrows {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn spawn(&mut self, arrow: Arrow) {
        self.list.push(arrow);
    }

    /// Стрелы измерения — для рассылки клиентам.
    pub fn in_dimension(&self, dimension: Dimension) -> Vec<Arrow> {
        self.list.iter().filter(|arrow| arrow.dimension == dimension).cloned().collect()
    }

}

/// Скорость по направлению взгляда и силе: `(-sin yaw · cos pitch, -sin
/// pitch, cos yaw · cos pitch) · скорость`. Углы — градусы.
pub fn aim(yaw: f32, pitch: f32, speed: f64) -> (f64, f64, f64) {
    let (yaw, pitch) = (f64::from(yaw).to_radians(), f64::from(pitch).to_radians());

    (-yaw.sin() * pitch.cos() * speed, -pitch.sin() * speed, yaw.cos() * pitch.cos() * speed)
}

/// Начальная скорость выстрела скелета: от точки `from` в точку `aim`, с
/// поправкой вверх на дальность (`+ дальность · 0,2` по высоте — так целятся
/// стрелки, чтобы стрела не упала раньше цели) и разбросом: каждая ось
/// получает нормальную добавку `error · 0,0075`; длина вектора приводится к
/// `speed`. `gauss` выдаёт нормальные числа с нулевым средним и единичным
/// разбросом.
pub fn launch(from: (f64, f64, f64), aim: (f64, f64, f64), speed: f64, error: f64, gauss: &mut dyn FnMut() -> f64) -> (f64, f64, f64) {
    let (dx, mut dy, dz) = (aim.0 - from.0, aim.1 - from.1, aim.2 - from.2);
    let flat = (dx * dx + dz * dz).sqrt();

    dy += flat * 0.2;

    let len = (dx * dx + dy * dy + dz * dz).sqrt().max(1.0e-9);
    let (nx, ny, nz) = (dx / len, dy / len, dz / len);
    let spread = 0.0075 * error;
    let (vx, vy, vz) = (nx + gauss() * spread, ny + gauss() * spread, nz + gauss() * spread);

    (vx * speed, vy * speed, vz * speed)
}

/// Урон стрелы, влетевшей со скоростью `speed`: `ceil(скорость · базовый)`,
/// у критической прибавка (вики, Arrow).
pub fn damage_of(arrow: &Arrow, roll: f32) -> f32 {
    if arrow.trident.is_some() {
        return TRIDENT_DAMAGE;
    }

    crate::combat::arrow_damage((arrow.vx * arrow.vx + arrow.vy * arrow.vy + arrow.vz * arrow.vz).sqrt(), arrow.power, arrow.crit, roll)
}

/// Место, где отрезок `from → to` первый раз входит в ящик (полуширина
/// `half`, от `low` до `high` по высоте). Возвращает долю пути от 0 до 1.
fn sweep_box(from: (f64, f64, f64), to: (f64, f64, f64), (cx, low, cz): (f64, f64, f64), half: f64, height: f64) -> Option<f64> {
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

    // Отрезок лишь касается ящика в своём начале (стрела, отскочившая от
    // грани, летит прочь): попадания нет (чёрный ящик, wither-half-armor).
    if far <= 1.0e-7 {
        return None;
    }

    Some(near)
}

/// То же для живой цели: если начало пути уже внутри её ящика, попадания нет
/// (чёрный ящик, tails-arrow-pointblank: стрела, созданная внутри коровы,
/// пролетела её насквозь и упала в землю; у оригинала отрезок из ящика грани
/// не пересекает) [проверить: только эта запись].
fn sweep_living(from: (f64, f64, f64), to: (f64, f64, f64), (cx, low, cz): (f64, f64, f64), half: f64, height: f64) -> Option<f64> {
    let inside = from.0 > cx - half && from.0 < cx + half && from.1 > low && from.1 < low + height && from.2 > cz - half && from.2 < cz + half;

    if inside { None } else { sweep_box(from, to, (cx, low, cz), half, height) }
}

/// Кого стрела задела на пути: игрок или моб, и в какой доле пути.
enum Target {
    Player { entity_id: i32 },
    Mob { entity_id: i32 },
}

/// Игрок рядом со стрелой: номер, место, приседает ли, режим, опознаватель,
/// имя и «добыча» в основной руке в текущем такте, а не в момент выстрела.
type Bystander = (i32, (f64, f64, f64), bool, i32, [u8; 16], String, i32);

/// Ход стрел одного измерения за такт. Мир нужен для блоков, игроки и
/// мобы — для целей.
/// Куда трезубец с «громовержцем» зовёт молнию: в клетку раненого (низ,
/// середина), если идёт гроза и над клеткой открыто небо (вики, Channeling).
fn channeling_bolt(arrow: &Arrow, world: &World, target: Option<(f64, f64, f64)>) -> Option<(f64, f64, f64)> {
    let trident = arrow.trident?;

    if trident.extras().level("channeling") == 0 || !world.weather.is_thundering() {
        return None;
    }

    let (x, y, z) = target?;
    let cell = (x.floor() as i32, y.floor() as i32, z.floor() as i32);

    world.open_sky(cell.0, cell.1, cell.2).then_some((f64::from(cell.0) + 0.5, f64::from(cell.1), f64::from(cell.2) + 0.5))
}

/// Вики, Channeling/Lightning Rod: «a lightning rod ... during a thunderstorm».
/// Чёрный ящик enchant-channeling-lightning-rod: молния на Y самого
/// громоотвода; целевая клетка отдельно передаётся Bolt::on_rod.
fn channeling_rod(arrow: &Arrow, world: &World, cell: (i32, i32, i32)) -> Option<(f64, f64, f64)> {
    let name = crate::blocks::block_at_state(world.get_block(cell.0, cell.1, cell.2))?;
    if !name.ends_with("lightning_rod") || !world.open_sky(cell.0, cell.1, cell.2) { return None; }
    channeling_bolt(arrow, world, Some((f64::from(cell.0), f64::from(cell.1), f64::from(cell.2))))
}

pub fn step(shared: &Shared, dimension: Dimension, world: &mut World) {
    // Стрелы клиентам рассылает своё слежение (после хода стрел): пока они есть
    // (и в такт, когда последняя пропала), оно должно идти — даже без мобов
    // (pvp-bow на пустой площадке: стрелу никто не видел).
    let any = shared.arrows.lock().expect("стрелы захвачены другим потоком").list.iter().any(|arrow| arrow.dimension == dimension);

    let tracker = any.then(|| world.begin_arrow_snapshot());

    let members: Vec<Bystander> = shared
        .players
        .lock()
        .expect("список игроков захвачен другим потоком")
        .members()
        .iter()
        .filter(|member| member.dimension == dimension)
        .map(|member| (member.entity_id, (member.position.x, member.position.y, member.position.z), member.sneaking, member.game_mode, member.uuid, member.name.clone(), member.held.map_or(0, |stack| stack.extras().level("looting"))))
        .collect();

    let mut items = shared.items_in(dimension).lock().expect("предметы захвачены другим потоком");
    let mut arrows = shared.arrows.lock().expect("стрелы захвачены другим потоком");
    arrows.retired.retain(|arrow| arrow.dimension != dimension);
    let mut pickups: Vec<([u8; 16], Stack)> = Vec::new();
    let mut removed: Vec<i32> = Vec::new();
    let mut progress = Vec::new();

    for arrow in arrows.list.iter_mut().filter(|arrow| arrow.dimension == dimension) {
        if arrow.fresh {
            arrow.fresh = false;
            continue;
        }

        // Стрела в незагруженном чанке стоит.
        if !world.has_chunk((arrow.x.floor() as i32).div_euclid(16), (arrow.z.floor() as i32).div_euclid(16)) {
            continue;
        }

        if arrow.collected {
            removed.push(arrow.entity_id);
            continue;
        }

        arrow.age += 1;

        // Перелом прошлого такта: место и скорость к его концу уйдут
        // клиентам в этом (чёрный ящик, trident-throw: отскок на +1, скорость
        // и сдвиг до точки удара — на +2).
        if std::mem::take(&mut arrow.impulse) {
            arrow.resync = Some((arrow.age, (arrow.x, arrow.y, arrow.z), (arrow.vx, arrow.vy, arrow.vz), arrow.flags(), arrow.facing()));
        }

        // «Верность»: ранивший или вонзившийся трезубец летит к хозяину
        // (вики, Loyalty: «return to the thrower after hitting an entity or
        // directly hitting a block»); хозяина нет — лежит, как лежал.
        if arrow.loyalty > 0 && !arrow.returning && (arrow.dealt || (arrow.in_ground && arrow.ground_ticks >= LOYALTY_GROUND_WAIT)) && members.iter().any(|member| member.0 == arrow.owner && member.3 != 3) {
            arrow.returning = true;
            arrow.in_ground = false;
            arrow.impulse = true;
            world.note_sound_neutral_at((arrow.x, arrow.y, arrow.z), "minecraft:item.trident.return", 10.0, 1.0);
        }

        if arrow.returning {
            let Some((owner_id, at, _, mode, uuid, _, _)) = members.iter().find(|member| member.0 == arrow.owner && member.3 != 3) else {
                // Хозяин ушёл: падает, где летел [проверить].
                arrow.returning = false;
                continue;
            };
            let level = f64::from(arrow.loyalty);
            let eye = (at.0, at.1 + 1.62, at.2);

            // Сквозь блоки к глазам хозяина: скорость гаснет на 5 % и
            // набирает 0,05 за уровень в его сторону, по высоте ещё
            // подтягивается на 0,015 за уровень [проверить].
            arrow.y += (eye.1 - arrow.y) * 0.015 * level;

            let (dx, dy, dz) = (eye.0 - arrow.x, eye.1 - arrow.y, eye.2 - arrow.z);
            let length = (dx * dx + dy * dy + dz * dz).sqrt().max(1.0e-9);
            let pull = 0.05 * level;

            arrow.vx = arrow.vx * 0.95 + dx / length * pull;
            arrow.vy = arrow.vy * 0.95 + dy / length * pull;
            arrow.vz = arrow.vz * 0.95 + dz / length * pull;
            arrow.x += arrow.vx;
            arrow.y += arrow.vy;
            arrow.z += arrow.vz;

            // Возвращаясь, трезубец доворачивается против своего полёта —
            // остриём от хозяина (чёрный ящик, trident-loyalty: угол −174° →
            // −139° и 172° → 138° за такт, т. е. на пятую часть к 0 при полёте
            // на −z; наклон — по полёту, 0 → −4°).
            if let Some(rot) = arrow.rot {
                let (_, pitch) = angles_of((arrow.vx, arrow.vy, arrow.vz));
                let (yaw, _) = angles_of((-arrow.vx, -arrow.vy, -arrow.vz));

                arrow.rot = Some(turn_toward(rot, (yaw, pitch)));
            }

            // Сопротивление воздуха — после хода, тяжести нет (чёрный ящик,
            // trident-loyalty: сдвиг −0,1233 по z, а скорость следом −0,122).
            arrow.vx *= AIR_DRAG;
            arrow.vy *= AIR_DRAG;
            arrow.vz *= AIR_DRAG;

            if (at.0 - arrow.x).abs() <= PICKUP_REACH && arrow.y >= at.1 - 1.0 && arrow.y <= at.1 + 2.3 && (at.2 - arrow.z).abs() <= PICKUP_REACH {
                if let Some(stack) = arrow.trident.filter(|_| *mode != 1 || arrow.pickup == Pickup::Yes) {
                    pickups.push((*uuid, stack));
                }

                world.note_collect(arrow.entity_id, *owner_id, 1);
                arrow.collected = true;
            }

            continue;
        }

        if arrow.in_ground {
            arrow.ground_ticks += 1;

            // Опору убрали — стрела снова летит.
            let (bx, by, bz) = arrow.ground_block;
            let state = world.get_block(bx, by, bz);

            if !crate::mob::solid(state) {
                arrow.in_ground = false;
                arrow.ground_ticks = 0;
            } else if arrow.ground_ticks >= GROUND_LIFE {
                removed.push(arrow.entity_id);
                continue;
            } else if arrow.ground_ticks >= PICKUP_DELAY && arrow.pickup != Pickup::No {
                // Подбор: игрок рядом (ящик игрока расширен на блок вширь и
                // полблока ввысь и вниз). [проверить]
                // Трезубец берёт только хозяин: у оригинала раненый, стоя рядом
                // с упавшим трезубцем, его не поднял (чёрный ящик,
                // trident-throw: лежит до конца записи, 60 тактов).
                let owner_only = arrow.trident.is_some();
                let picker = members.iter().find(|(entity_id, at, _, mode, _, _, _)| {
                    (*mode == 0 || *mode == 1 || *mode == 2)
                        && (arrow.pickup == Pickup::Yes || *mode == 1)
                        && (!owner_only || *entity_id == arrow.owner)
                        && (at.0 - arrow.x).abs() <= PICKUP_REACH
                        && arrow.y >= at.1 - 1.0
                        && arrow.y <= at.1 + 2.3
                        && (at.2 - arrow.z).abs() <= PICKUP_REACH
                });

                if let Some((entity_id, _, _, _, uuid, _, _)) = picker {
                    // Творческому трезубец не возвращается в руки: он и не уходил.
                    match arrow.trident {
                        Some(stack) if arrow.pickup == Pickup::Yes => pickups.push((*uuid, stack)),
                        Some(_) => {}
                        None => pickups.push((*uuid, arrow.pickup_stack())),
                    }
                    world.note_collect(arrow.entity_id, *entity_id, 1);
                    // Стрела исчезнет в следующем такте: клиентам её убирает сам
                    // подбор (Collect), чтобы шёл раньше удаления.
                    arrow.pickup = Pickup::No;
                    arrow.collected = true;
                }
            }

            continue;
        }

        // Трезубец доворачивается к полёту на 0,2 за такт — к скорости, с
        // которой идёт этот такт (чёрный ящик, trident-throw: после отскока
        // сел через 8 тактов с наклоном −50 в четырёх записях оригинала;
        // к скорости после тяжести вышло бы −51).
        if let Some(rot) = arrow.rot {
            arrow.rot = Some(turn_toward(rot, angles_of((arrow.vx, arrow.vy, arrow.vz))));
        }

        let from = (arrow.x, arrow.y, arrow.z);
        let to = (arrow.x + arrow.vx, arrow.y + arrow.vy, arrow.z + arrow.vz);
        let mut best: Option<(f64, Target)> = None;

        // Игроки.
        for (entity_id, at, sneaking, mode, _, _, _) in &members {
            if arrow.dealt || *mode == 3 || (*entity_id == arrow.owner && arrow.age <= OWNER_SAFE) || arrow.pierced.contains(entity_id) {
                continue;
            }

            let height = if *sneaking { 1.5 } else { 1.8 };

            if let Some(part) = sweep_living(from, to, (at.0, at.1, at.2), 0.3, height)
                && best.as_ref().is_none_or(|(known, _)| part < *known)
            {
                best = Some((part, Target::Player { entity_id: *entity_id }));
            }
        }

        // Мобы тоже принимают стрелы скелетов. Эндермена стрела не берёт: он
        // прыгает прочь прежде попадания, а стрела летит дальше (вики,
        // Enderman: «immune to most projectiles»).
        let mut dodgers = Vec::new();

        for mob in items.mobs().iter() {
            if arrow.dealt || mob.is_dead() || arrow.pierced.contains(&mob.entity_id) || (mob.entity_id == arrow.owner && arrow.age < OWNER_SAFE) {
                continue;
            }

            let (width, height) = mob.size();

            if let Some(part) = sweep_living(from, to, (mob.x, mob.y, mob.z), width / 2.0, height)
                && best.as_ref().is_none_or(|(known, _)| part < *known)
            {
                if mob.kind == crate::mob::Kind::Enderman {
                    dodgers.push(mob.entity_id);
                    continue;
                }

                best = Some((part, Target::Mob { entity_id: mob.entity_id }));
            }
        }

        for mob in items.mobs_mut().iter_mut().filter(|mob| dodgers.contains(&mob.entity_id) && !mob.no_ai) {
            mob.h.want_teleport = true;
        }

        // Блоки: шагом в четверть блока вдоль пути.
        let length = ((to.0 - from.0).powi(2) + (to.1 - from.1).powi(2) + (to.2 - from.2).powi(2)).sqrt();
        let steps = ((length / 0.25).ceil() as usize).max(1);
        let mut block_part = None;
        let mut block_cell = (0, 0, 0);

        for index in 1..=steps {
            let part = index as f64 / steps as f64;
            let at = (from.0 + (to.0 - from.0) * part, from.1 + (to.1 - from.1) * part, from.2 + (to.2 - from.2) * part);

            let cell = (at.0.floor() as i32, at.1.floor() as i32, at.2.floor() as i32);

            if crate::mob::solid(world.get_block(cell.0, cell.1, cell.2)) {
                // Точка входа в блок — на его грани, а не там, где шаг его
                // нащупал (чёрный ящик, arrow-pickup и trident-throw: у
                // оригинала встала ровно на 0,05 над полом).
                let face = sweep_box(from, to, (f64::from(cell.0) + 0.5, f64::from(cell.1), f64::from(cell.2) + 0.5), 0.5, 1.0);

                block_part = Some(face.unwrap_or(part));
                block_cell = cell;
                break;
            }
        }

        let entity_first = match (&best, block_part) {
            (Some((part, _)), Some(block)) => *part <= block,
            (Some(_), None) => true,
            _ => false,
        };

        let best_part = best.as_ref().map(|(part, _)| *part);

        if entity_first && let Some((part, target)) = best {
            arrow.x = from.0 + (to.0 - from.0) * part;
            arrow.y = from.1 + (to.1 - from.1) * part;
            arrow.z = from.2 + (to.2 - from.2) * part;

            crate::redstone::game_event_from(world, (arrow.x, arrow.y, arrow.z), 2, Some(arrow.owner), true);
            let roll = world.random_unit();
            let damage = damage_of(arrow, roll);

            let on_player = matches!(target, Target::Player { .. });

            match target {
                Target::Player { entity_id } if arrow.is_mob_shot() => {
                    let Shooter::Mob { killer } = arrow.shooter else { unreachable!() };
                    let flat = arrow.vx.hypot(arrow.vz).max(1.0e-9);

                    world.note_enchanted_mob_hurt(entity_id, damage, if arrow.trident.is_some() { Source::Trident } else { Source::Arrow }, arrow.owner, killer, Some((arrow.vx / flat, arrow.vz / flat)), (arrow.x, arrow.y, arrow.z),
                        crate::world::ProjectileEnchantments { punch: arrow.punch, fire_ticks: if arrow.flame { 100 } else { 0 },
                            ignores_shield: arrow.pierce_left > 0 || !arrow.pierced.is_empty(), ..Default::default() });
                    for effect in &arrow.effects {
                        world.note_mob_hit_effect(entity_id, *effect);
                    }
                }
                Target::Player { entity_id } => {
                    world.note_game_event(arrow.owner, 6, 0.0);
                    world.note_player_strike(PlayerStrike {
                        victim: entity_id,
                        attacker: arrow.owner,
                        attacker_uuid: arrow.owner_uuid,
                        attacker_name: arrow.owner_name.clone(),
                        from: (arrow.x, arrow.y, arrow.z),
                        yaw: -arrow.angles().0,
                        damage,
                        source: if arrow.trident.is_some() { Source::Trident } else { Source::Arrow },
                        extra_knockback: arrow.punch as f32 * 1.2,
                        weapon: None,
                        disables_shield_ticks: 0,
                        breach_scale: 1.0,
                        fire_ticks: if arrow.flame { 100 } else { 0 },
                        thorns_slots: Vec::new(),
                        effects: arrow.effects.clone(),
                        ignores_shield: arrow.pierce_left > 0 || !arrow.pierced.is_empty(),
                    });
                }
                Target::Mob { entity_id } => {
                    let speed = arrow.vx.hypot(arrow.vz).max(1.0e-9);
                    let push = crate::mob::Push { dir: (arrow.vx / speed, arrow.vz / speed), strength: crate::mob::BASE_KNOCKBACK, cause: arrow.owner };

                    let mut bounced = false;
                    let mut burn = false;

                    if let Some(mob) = items.mobs_mut().iter_mut().find(|mob| mob.entity_id == entity_id) {
                        let previous_credit = (mob.kill_credit, mob.xp_until, mob.looting, mob.by_player);
                        let damage = damage + arrow.trident.map_or(0.0, |stack| crate::combat::impaling_bonus(&stack.extras(), mob.kind.name()));
                        let mut damage_result = None;
                        // Стрела без стрелка (призванная командой) опыта за
                        // убийство не сулит (wither-half-armor).
                        if arrow.owner != 0 && !arrow.is_mob_shot() {
                            mob.credit_player();
                        }
                        // Вики, Tutorial:Dual wielding: «mobs killed by the
                        // bow ... affected by the sword's Looting». Смена
                        // руки во время полёта меняет добычу. Установить до
                        // hurt: смертельный удар сразу создаёт Event::Loot.
                        mob.looting = if !arrow.is_mob_shot() && arrow.owner != 0 {
                            members.iter().find(|m| m.0 == arrow.owner).map_or(0, |m| m.6)
                        } else { 0 };
                        // Иссушитель в броне (половина здоровья и меньше): стрела
                        // отскакивает без урона (вики, Wither; wither-half-armor).
                        if crate::mob::wither::powered(mob) {
                            bounced = true;
                        } else if mob.kind == crate::mob::Kind::EnderDragon {
                            // Сидящий дракон стрелам не по зубам: «they catch
                            // fire and bounce off» (вики, Ender Dragon); в полёте
                            // стрела бьёт в тело — четверть урона [проверить].
                            if matches!(mob.dragon.phase, crate::mob::dragon::SCANNING | crate::mob::dragon::ROAR | crate::mob::dragon::BREATH) {
                                bounced = true;
                                burn = true;
                            } else {
                                damage_result = mob.hurt_part(damage, false, Source::Arrow, Some(push));
                            }
                        } else if let Some(hit) = mob.hurt_burning(damage, if arrow.trident.is_some() { Source::Trident } else { Source::Arrow }, Some(push), 1.0, if arrow.flame { 100 } else { 0 }) {
                            damage_result = Some(hit);
                            // Без стрелка виновник — сама стрела (wither-half-armor:
                            // Damage Event с причиной-сущностью).
                            if arrow.owner == 0
                                && let Some(crate::mob::Event::Hurt { cause, .. }) = mob.events.iter_mut().rev().find(|event| matches!(event, crate::mob::Event::Hurt { .. }))
                            {
                                *cause = arrow.entity_id;
                            }

                            // Стрела, не трезубец, остаётся в теле (свойство 12;
                            // wither-half-armor: 12=1 вместе с уроном).
                            if arrow.trident.is_none() && arrow.pierce_left == 0 {
                                mob.stick_arrow();
                            }

                            arrow.hit_effects(mob);
                            apply_effects(mob, &arrow.effects);
                        }
                        // Отбитая стрела и непринятый урон не продлевают
                        // кредит убийства и не заменяют чары прошлого удара.
                        if damage_result.is_none() {
                            (mob.kill_credit, mob.xp_until, mob.looting, mob.by_player) = previous_credit;
                        }
                        if !arrow.is_mob_shot() && arrow.owner != 0 && let Some(hit) = damage_result {
                            use crate::progress::Event;
                            let distance = members.iter().find(|m| m.0 == arrow.owner).map_or(0.0, |m| (m.1.0 - mob.x).hypot(m.1.2 - mob.z));
                            progress.push((arrow.owner_uuid, Event::ProjectileHit { trident: arrow.trident.is_some(), kind: mob.kind.name(), killed: mob.is_dead(), distance, crossbow: arrow.crossbow }));
                            progress.push((arrow.owner_uuid, Event::DamageDealt(hit)));
                            mob.credit_to(arrow.owner_uuid);
                            if mob.is_dead() {
                                arrow.killed.push(mob.kind.name());
                                if arrow.crossbow {
                                    if arrow.killed.iter().filter(|k| **k == "phantom").count() >= 2 {
                                        progress.push((arrow.owner_uuid, Event::Trigger("adventure/two_birds_one_arrow")));
                                    }
                                    if arrow.killed.iter().collect::<std::collections::HashSet<_>>().len() >= 5 {
                                        progress.push((arrow.owner_uuid, Event::Trigger("adventure/arbalistic")));
                                    }
                                }
                            }
                        }
                    }

                    // Отскок: стрела встаёт в точку удара, скорость ×(−0,1),
                    // разворот почти на 180° при прежнем наклоне, сопротивление и
                    // тяжесть — в том же такте; клиентам — как перелом, на такт
                    // позже (wither-half-armor: скорость (0; −0,045; −0,147) и
                    // поворот −128/−1 через такт после удара).
                    if bounced {
                        let (yaw, pitch) = arrow.facing();

                        // Разворот — на 170…190°, как у трезубца (wither-half-armor:
                        // −128 и −125 из 256 в двух записях оригинала).
                        arrow.rot = Some((yaw + 170.0 + world.random_unit() * 20.0, pitch));
                        arrow.vx *= -0.1 * AIR_DRAG;
                        arrow.vy = arrow.vy * -0.1 * AIR_DRAG - GRAVITY;
                        arrow.vz *= -0.1 * AIR_DRAG;
                        arrow.impulse = true;
                        arrow.flame |= burn;
                        continue;
                    }
                }
            }

            // «Громовержец»: в грозу трезубец зовёт молнию в клетку раненого,
            // если над ним открытое небо (вики, Channeling: «There must not be
            // opaque or partially transparent blocks at any position above
            // the target entity»). Гром трезубца (`item.trident.thunder`,
            // громкость 5) — в месте молнии, раньше обычного звука попадания
            // (чёрный ящик, trident-channeling).
            let target_place = match target {
                Target::Player { entity_id } => members.iter().find(|member| member.0 == entity_id).map(|member| member.1),
                Target::Mob { entity_id } => items.mobs().iter().find(|mob| mob.entity_id == entity_id).map(|mob| (mob.x, mob.y, mob.z)),
            };
            let bolt = channeling_bolt(arrow, world, target_place);

            if let Some(place) = bolt {
                let mut bolt = crate::lightning::Bolt::new(shared.next_entity_id(), place);
                if !arrow.is_mob_shot() && arrow.owner != 0 { bolt.channeler = Some(arrow.owner_uuid); }
                items.add_bolt(bolt);
                world.note_sound_neutral_at(place, "minecraft:item.trident.thunder", 5.0, 1.0);
            }

            // Игроку, раненому мобом, звук попадания уходит из `vitals::mob_hit`.
            // Трезубец утопленника звучит сам: item.trident.hit (чёрный ящик, drowned-trident).
            if !(arrow.is_mob_shot() && on_player && arrow.trident.is_none()) {
                world.note_sound_neutral_at((arrow.x, arrow.y, arrow.z), arrow.hit_sound(), 1.0, 1.0);
            }

            // Трезубец отскакивает от раненого и падает рядом (вики, Trident:
            // «bounces off the entity and lands nearby») [проверить множители].
            // Скорость гаснет и тянет вниз в том же такте (чёрный ящик,
            // trident-throw: после удара скорость (0; −0,044; −0,025)).
            if arrow.trident.is_some() {
                arrow.dealt = true;
                arrow.impulse = true;
                // Разворот на 170…190° со случайной добавкой, наклон прежний
                // (чёрный ящик: после удара угол 169,5°, −175°, 172°, −174° в
                // четырёх записях оригинала при броске почти ровно на +z;
                // наклон как в полёте, 0 или −1).
                let (yaw, pitch) = arrow.facing();

                arrow.rot = Some((yaw + 170.0 + world.random_unit() * 20.0, pitch));
                arrow.vx *= -0.01 * AIR_DRAG;
                arrow.vy = arrow.vy * -0.1 * AIR_DRAG - GRAVITY;
                arrow.vz *= -0.01 * AIR_DRAG;
                continue;
            }

            // «Пробивание»: стрела летит дальше сквозь задетого (вики,
            // Piercing: «pass through the number of entities equal to the
            // enchantment level») и в такт удара проходит весь свой ход
            // (чёрный ящик, crossbow-piercing: через такт после удара сдвиг на
            // два полных хода).
            if arrow.pierce_left > 0 {
                arrow.pierce_left -= 1;
                arrow.pierced.push(match target {
                    Target::Player { entity_id } | Target::Mob { entity_id } => entity_id,
                });
                arrow.impulse = true;
                (arrow.x, arrow.y, arrow.z) = from;
            } else {
                removed.push(arrow.entity_id);
                continue;
            }
        }

        // Пробившая цель стрела летит дальше: блок после цели ещё может её
        // остановить.
        let block_part = if entity_first { block_part.filter(|part| best_part.is_some_and(|hit| *part >= hit)) } else { block_part };

        if let Some(part) = block_part {
            arrow.x = from.0 + (to.0 - from.0) * part;
            arrow.y = from.1 + (to.1 - from.1) * part;
            arrow.z = from.2 + (to.2 - from.2) * part;
            arrow.ground_block = block_cell;
            if let Some(place) = channeling_rod(arrow, world, block_cell) {
                let mut bolt = crate::lightning::Bolt::new(shared.next_entity_id(), place).on_rod(block_cell);
                if !arrow.is_mob_shot() && arrow.owner != 0 { bolt.channeler = Some(arrow.owner_uuid); }
                items.add_bolt(bolt);
                world.note_sound_neutral_at(place, "minecraft:item.trident.thunder", 5.0, 1.0);
            }
            if arrow.flame { ignite_block(world, block_cell); }
            if let Some(power) = crate::redstone::projectile_target(world, block_cell, (arrow.x, arrow.y, arrow.z), true)
                && !arrow.is_mob_shot() && arrow.owner != 0
            {
                progress.push((arrow.owner_uuid, crate::progress::Event::Stat(crate::stats::Cat::Custom, "target_hit", 1)));
                if power == 15 && members.iter().any(|m| m.0 == arrow.owner && (m.1.0 - arrow.x).hypot(m.1.2 - arrow.z) >= 30.0) {
                    progress.push((arrow.owner_uuid, crate::progress::Event::Trigger("adventure/bullseye")));
                }
            }

            // Стрела и трезубец отступают от точки удара на 0,05 по каждой оси
            // (чёрный ящик, trident-throw и arrow-pickup: сели на высоте 0,05
            // над полом).
            let (bx, by, bz) = ground_backoff((arrow.vx, arrow.vy, arrow.vz));

            arrow.x += bx;
            arrow.y += by;
            arrow.z += bz;

            (arrow.vx, arrow.vy, arrow.vz) = (0.0, 0.0, 0.0);
            crate::redstone::game_event_from(world, (arrow.x, arrow.y, arrow.z), 2, Some(arrow.owner), true);
            arrow.in_ground = true;
            // enchant-pillager-crossbow-enchanted: при попадании в блок
            // Piercing сбрасывается (metadata9=0 вместе с10=true).
            arrow.pierce_left = 0;
            arrow.pierced.clear();
            arrow.killed.clear();
            arrow.ground_ticks = 0;
            arrow.landed = true;
            world.note_sound_neutral_at((arrow.x, arrow.y, arrow.z), arrow.ground_sound(), 1.0, 1.0);
            continue;
        }

        arrow.x = to.0;
        arrow.y = to.1;
        arrow.z = to.2;

        let in_water = crate::mob::is_water(world.get_block(arrow.x.floor() as i32, arrow.y.floor() as i32, arrow.z.floor() as i32));
        if in_water || world.in_rain(arrow.x, arrow.y, arrow.z, 0.5) { arrow.flame = false; }
        // Трезубец в воде не тормозит (вики, Trident: «does not slow down»).
        let drag = if in_water && arrow.trident.is_none() { WATER_DRAG } else { AIR_DRAG };

        arrow.vx *= drag;
        arrow.vy = arrow.vy * drag - if arrow.no_gravity { 0.0 } else { GRAVITY };
        arrow.vz *= drag;

        // Улетела за край мира.
        if arrow.y < -128.0 {
            removed.push(arrow.entity_id);
        }
    }

    if !removed.is_empty() {
        let retired: Vec<_> = arrows.list.iter().filter(|arrow| arrow.age <= 1 && removed.contains(&arrow.entity_id)).cloned().collect();
        arrows.retired.extend(retired);
        arrows.list.retain(|arrow| !removed.contains(&arrow.entity_id));
    }

    // Раненные стрелами мобы: событие урона и звук уходят в такт попадания,
    // а не со следующим тактом мобов (wither-half-armor: damage_event вместе
    // со звуком попадания стрелы).
    if items.mobs().iter().any(|mob| !mob.events.is_empty()) {
        items.flush_mob_events(&mut || shared.next_entity_id(), &mut crate::mob::Rng::new(u64::from(shared.next_entity_id() as u32)));
    }

    // Снимок относится к метке этого такта, даже если подключение отстало.
    if let Some(tracker) = tracker {
        let _ = tracker.set(Arrows {
            list: arrows.in_dimension(dimension),
            retired: arrows.retired.iter().filter(|a| a.dimension == dimension).cloned().collect(),
        });
    }
    drop(arrows);
    drop(items);

    if !progress.is_empty() {
        let mut players = shared.players.lock().expect("список игроков захвачен другим потоком");
        for (uuid, event) in progress { players.order(&uuid, crate::players::Order::Progress(event)); }
    }
    if !pickups.is_empty() {
        let mut players = shared.players.lock().expect("список игроков захвачен другим потоком");

        for (uuid, stack) in pickups {
            if let Some(name) = crate::blocks::item_name(stack.item) {
                players.order(&uuid, crate::players::Order::Progress(crate::progress::Event::Stat(crate::stats::Cat::PickedUp, name, stack.count)));
            }
            players.order(&uuid, crate::players::Order::Pick { stack });
        }
    }
}

/// Wiki, Flame: только TNT, campfire и candle, без огня на обычных блоках.
/// [проверить] Сетевой порядок зажигания при попадании.
fn ignite_block(world: &mut World, at: (i32,i32,i32)) -> bool {
    if crate::tnt::ignite(world, at) { return true; }
    let state=world.get_block(at.0,at.1,at.2);
    let Some(block)=crate::blocks::orientation_of(state) else { return false; };
    if !(matches!(block.name,"campfire"|"soul_campfire"|"candle"|"candle_cake") || block.name.ends_with("_candle") || block.name.ends_with("_candle_cake"))
        || block.value(state,"waterlogged")==Some("true") || block.value(state,"lit")!=Some("false") { return false; }
    let Some(lit)=block.with(state,"lit","true") else { return false; };
    world.set_block(at.0,at.1,at.2,lit);
    true
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aim_follows_the_look_direction() {
        // yaw 0 — на +z, наклон 0
        let (x, y, z) = aim(0.0, 0.0, 3.0);

        assert!(x.abs() < 1e-9 && y.abs() < 1e-9 && (z - 3.0).abs() < 1e-9);

        // yaw 90 — на −x; наклон вверх (−90) — вверх
        let (x, _, _) = aim(90.0, 0.0, 3.0);

        assert!((x + 3.0).abs() < 1e-9);
        assert!((aim(0.0, -90.0, 1.0).1 - 1.0).abs() < 1e-9);
    }

    #[test]
    fn a_ray_leaving_a_box_face_misses_it() {
        // отскочившая от иссушителя стрела на его грани летит прочь
        assert_eq!(sweep_box((3.5, 2.46, 4.05), (3.5, 2.42, 3.903), (3.5, 0.0, 4.5), 0.45, 3.5), None);
        assert!(sweep_box((3.5, 2.46, 3.0), (3.5, 2.42, 4.5), (3.5, 0.0, 4.5), 0.45, 3.5).is_some());
    }

    #[test]
    fn the_state_before_the_last_step_undoes_drag_and_gravity() {
        let mut arrow = Arrow::from_mob(1, Dimension::Overworld, (0.0, 10.0, 0.0), (1.0, 0.5, 0.0), 0, "");
        let (place, velocity) = ((arrow.x, arrow.y, arrow.z), (arrow.vx, arrow.vy, arrow.vz));

        arrow.x += arrow.vx;
        arrow.y += arrow.vy;
        arrow.vx *= AIR_DRAG;
        arrow.vy = arrow.vy * AIR_DRAG - GRAVITY;

        let (back_place, back_velocity) = arrow.before_step();

        assert!((back_place.0 - place.0).abs() < 1e-9 && (back_place.1 - place.1).abs() < 1e-9);
        assert!((back_velocity.0 - velocity.0).abs() < 1e-9 && (back_velocity.1 - velocity.1).abs() < 1e-9);
    }

    #[test]
    fn an_arrow_starting_inside_a_living_box_passes_through() {
        assert_eq!(sweep_living((0.5, 1.0, 3.1), (0.5, 1.0, 4.1), (0.5, 0.0, 3.5), 0.45, 1.4), None);
        assert!(sweep_living((0.5, 1.0, 2.0), (0.5, 1.0, 3.2), (0.5, 0.0, 3.5), 0.45, 1.4).is_some());
    }

    #[test]
    fn a_swept_box_is_hit_at_its_near_face() {
        // из x=0 в x=10, ящик вокруг x=5 шириной 1: вход на x=4,5 — доля 0,45
        let part = sweep_box((0.0, 1.0, 0.0), (10.0, 1.0, 0.0), (5.0, 0.0, 0.0), 0.5, 2.0).expect("попала");

        assert!((part - 0.45).abs() < 1e-9);
        // мимо: выше ящика
        assert!(sweep_box((0.0, 5.0, 0.0), (10.0, 5.0, 0.0), (5.0, 0.0, 0.0), 0.5, 2.0).is_none());
        // не долетела
        assert!(sweep_box((0.0, 1.0, 0.0), (3.0, 1.0, 0.0), (5.0, 0.0, 0.0), 0.5, 2.0).is_none());
    }

    #[test]
    fn angles_point_along_the_velocity() {
        let mut arrow = Arrow {
            entity_id: 1,
            uuid: [0; 16],
            dimension: Dimension::Overworld,
            x: 0.0,
            y: 0.0,
            z: 0.0,
            vx: 0.0,
            vy: 0.0,
            vz: 3.0,
            no_gravity: false,
            owner: 0,
            owner_uuid: [0; 16],
            owner_name: String::new(),
            shooter: Shooter::Player,
            effects: Vec::new(),
            potion: None,
            ammo: None,
            crit: false,
            power: 0,
            punch: 0,
            flame: false,
            pierce_left: 0,
            pierced: Vec::new(),
            killed: Vec::new(),
            crossbow: false,
            trident: None,
            loyalty: 0,
            dealt: false,
            returning: false,
            collected: false,
            impulse: false,
            resync: None,
            rot: None,
            ground_block: (0, 0, 0),
            pickup: Pickup::Yes,
            in_ground: false,
            ground_ticks: 0,
            age: 0,
            fresh: false,
            summoned: false,
            landed: false,
            start: ((0.0, 0.0, 0.0), (0.0, 0.0, 3.0)),
        };

        assert_eq!(arrow.angles(), (0.0, 0.0));

        arrow.vx = -3.0;
        arrow.vz = 0.0;

        assert!((arrow.angles().0 + 90.0).abs() < 1e-4);
        assert_eq!(damage_of(&arrow, 0.0), 6.0);
    }

    #[test]
    fn a_piercing_arrow_remembers_whom_it_passed() {
        let mut arrow = Arrow::from_mob(7, Dimension::Overworld, (0.0, 0.0, 0.0), (3.15, 0.0, 0.0), 4, "entity.minecraft.skeleton");

        arrow.pierce_left = 2;
        arrow.pierced.push(11);

        // Пробитый второй раз не задевается, запас пробиваний убывает.
        assert!(arrow.pierced.contains(&11));
        assert_eq!(arrow.pierce_left, 2);
        // Урон стрелы арбалета: ceil(3,15 · 2) = 7 (вики, Crossbow).
        assert_eq!(damage_of(&arrow, 0.0), 7.0);
    }

    #[test]
    fn a_thrown_trident_hits_for_eight_and_sounds_like_a_trident() {
        let mut trident = Arrow::from_mob(7, Dimension::Overworld, (0.0, 0.0, 0.0), (2.5, 0.0, 0.0), 4, "entity.minecraft.skeleton");

        trident.trident = Some(Stack::new(crate::blocks::item_named("trident").expect("трезубец есть"), 1));

        // Урон 8 при любой скорости (вики, Trident).
        assert_eq!(damage_of(&trident, 0.9), TRIDENT_DAMAGE);
        assert_eq!(trident.entity_type(), TRIDENT_TYPE);
        assert_eq!(trident.hit_sound(), "minecraft:item.trident.hit");
        assert_eq!(trident.ground_sound(), "minecraft:item.trident.hit_ground");
    }

    #[test]
    fn a_landed_trident_backs_off_by_the_sign_of_each_axis() {
        // trident-throw: падает почти отвесно, чуть на −z и +x — отступ 0,05
        // по каждой оси целиком; нулевая скорость по оси — без отступа.
        let (bx, by, bz) = ground_backoff((0.0003, -0.4, -0.024));

        assert_eq!((bx, by, bz), (-0.05, 0.05, 0.05));
        assert_eq!(ground_backoff((0.0, -1.0, 0.0)), (0.0, 0.05, 0.0));
    }

    #[test]
    fn a_trident_turns_a_fifth_of_the_way_by_the_short_side() {
        // trident-loyalty: −174° к 0 → −139°; через ±180 — кратчайшим путём.
        let (yaw, pitch) = turn_toward((-174.4, 0.0), (0.0, -19.4));

        assert!((yaw + 139.52).abs() < 1e-3 && (pitch + 3.88).abs() < 1e-3);

        let (yaw, _) = turn_toward((175.0, 0.0), (-175.0, 0.0));

        assert!((yaw - 177.0).abs() < 1e-3);
    }

    #[test]
    fn the_life_of_an_arrow_counts_from_its_spawn_tick() {
        let mut mob = Arrow::from_mob(7, Dimension::Overworld, (0.0, 0.0, 0.0), (1.0, 0.0, 0.0), 4, "entity.minecraft.skeleton");
        let mut player = Arrow { shooter: Shooter::Player, ..mob.clone() };

        // Стрела игрока ходит уже в такт выстрела (возраст 1), моба — нет.
        mob.age = 20;
        player.age = 21;

        assert_eq!(mob.lived(), 20);
        assert_eq!(player.lived(), 20);
    }

    #[test]
    fn a_skeleton_arrow_waits_one_tick_and_cannot_be_picked_up() {
        let arrow = Arrow::from_mob(7, Dimension::Overworld, (1.0, 2.0, 3.0), (1.6, 0.0, 0.0), 4, "entity.minecraft.skeleton");

        assert!(arrow.fresh && arrow.is_mob_shot());
        assert_eq!(arrow.pickup, Pickup::No);
        assert_eq!(arrow.start, ((1.0, 2.0, 3.0), (1.6, 0.0, 0.0)));
        // Урон по скорости: 2 · 1,6 = 3,2 → 4 (вики, Arrow).
        assert_eq!(damage_of(&arrow, 0.0), 4.0);
    }

    #[test]
    fn the_launch_aims_up_by_distance_and_keeps_the_speed() {
        let mut none = || 0.0;
        let v = launch((0.0, 1.0, 0.0), (10.0, 1.0, 0.0), 1.6, 6.0, &mut none);

        assert!(v.1 > 0.0, "наводка вверх на дальность");
        assert!((v.0 * v.0 + v.1 * v.1 + v.2 * v.2).sqrt() - 1.6 < 1.0e-9);

        // Разброс сдвигает вектор: направление в пределах error · 0,0075 на ось.
        let mut one = || 1.0;
        let off = launch((0.0, 1.0, 0.0), (10.0, 1.0, 0.0), 1.6, 6.0, &mut one);

        assert!((off.2 / 1.6 - 0.045).abs() < 0.005);
    }
}

#[cfg(test)]
mod itemaudit2_tests {
    use super::*;
    #[test]
    fn rejected_arrows_preserve_kill_credit_looting_and_existing_fire() {
        for kind in [crate::mob::Kind::Wither, crate::mob::Kind::Cow] {
            for player_shot in [false, true] {
                let shared = Shared::new(World::in_memory(), World::in_memory(), Default::default(), Default::default(), Default::default(),
                    crate::ops::Ops::open(std::path::Path::new("tmp/enchant-arrow-rejected/ops.json")), crate::skins::Settings::default());
                let mut world = shared.world_in(Dimension::Overworld).lock().unwrap();
                for y in 64..=67 { for z in 0..=4 { world.set_block_silently(0, y, z, crate::world::AIR); } }
                let mut mob = crate::mob::Mob::new(30, kind, (0.5, 64.0, 2.5));
                mob.no_ai = true;
                mob.ticks = 100;
                mob.xp_until = 107;
                mob.kill_credit = Some([8; 16]);
                mob.looting = 2;
                mob.by_player = true;
                mob.vitals.fire = 17;
                if kind == crate::mob::Kind::Wither {
                    mob.vitals.health = 100.0; // Броня босса отбивает стрелу до hurt.
                } else {
                    // Обычный hurt отклоняет стрелу слабее предыдущего удара.
                    assert!(mob.vitals.hurt(7.0, Source::Arrow, false).is_some());
                }
                let health = mob.vitals.health;
                shared.items_in(Dimension::Overworld).lock().unwrap().add_mob(mob);
                let mut arrow = Arrow::from_mob(40, Dimension::Overworld, (0.5, 64.5, 0.5), (0.0, 0.0, 3.0), 31, "skeleton");
                arrow.fresh = false;
                arrow.flame = true;
                if player_shot { arrow.shooter = Shooter::Player; arrow.owner_uuid = [31; 16]; }
                shared.arrows.lock().unwrap().spawn(arrow);
                step(&shared, Dimension::Overworld, &mut world);
                let items = shared.items_in(Dimension::Overworld).lock().unwrap();
                let victim = items.mobs().iter().find(|mob| mob.entity_id == 30).unwrap();
                assert_eq!(victim.vitals.health, health);
                assert_eq!((victim.kill_credit, victim.xp_until, victim.looting, victim.by_player),
                    (Some([8; 16]), 107, 2, true), "{kind:?}, player={player_shot}");
                assert_eq!(victim.vitals.fire, 17, "отклонённый Flame не продлевает прежний огонь");
                drop(items);
                let arrows = shared.arrows.lock().unwrap();
                if kind == crate::mob::Kind::Wither {
                    assert!(arrows.list.iter().any(|arrow| arrow.entity_id == 40 && arrow.vz < 0.0),
                        "стрела действительно столкнулась с боссом и отскочила");
                } else {
                    assert!(arrows.list.iter().all(|arrow| arrow.entity_id != 40),
                        "стрела действительно достигла цели с непринятым уроном");
                }
            }
        }
    }

    #[test]
    fn channeling_rod_requires_trident_enchantment_thunder_and_open_sky() {
        for (enchanted, thunder, roof, rod) in [
            (true, true, false, true), (false, true, false, true),
            (true, false, false, true), (true, true, true, true), (true, true, false, false),
        ] {
            let shared = Shared::new(World::in_memory(), World::in_memory(), Default::default(), Default::default(), Default::default(),
                crate::ops::Ops::open(std::path::Path::new("tmp/enchant-channeling/ops.json")), crate::skins::Settings::default());
            let mut world = shared.world_in(Dimension::Overworld).lock().unwrap();
            let cell = (0, 65, 2);
            world.set_block_silently(cell.0, cell.1, cell.2,
                crate::blocks::state_by_name(if rod { "lightning_rod" } else { "stone" }).unwrap());
            if roof { world.set_block_silently(0, 66, 2, crate::blocks::state_by_name("stone").unwrap()); }
            world.weather.rain_level = 1.0;
            world.weather.thunder_level = if thunder { 1.0 } else { 0.0 };
            let mut arrow = Arrow::from_mob(40, Dimension::Overworld, (0.5, 65.5, 0.5), (0.0, 0.0, 3.0), 31, "");
            arrow.fresh = false;
            arrow.shooter = Shooter::Player;
            arrow.owner_uuid = [31; 16];
            let mut data = crate::stackdata::Data::default();
            data.set_enchant(crate::stackdata::enchantment_number("channeling").unwrap(), i32::from(enchanted));
            arrow.trident = Some(Stack::new(crate::blocks::item_named("trident").unwrap(), 1).with_extras(data));
            let expected = enchanted && thunder && !roof && rod;
            assert_eq!(channeling_rod(&arrow, &world, cell).is_some(), expected);
            shared.arrows.lock().unwrap().spawn(arrow);
            step(&shared, Dimension::Overworld, &mut world);
            let mut count = 0;
            shared.items_in(Dimension::Overworld).lock().unwrap().step_bolts(|bolt| {
                count += 1;
                assert_eq!(bolt.struck_block(), cell);
                assert_eq!((bolt.x, bolt.y, bolt.z), (0.5, 65.0, 2.5));
                assert_eq!(bolt.channeler, Some([31; 16]));
                true
            });
            assert_eq!(count, i32::from(expected));
        }
    }

    #[test]
    fn only_player_owned_projectiles_unlock_player_kill_drops() {
        for (owner, player_shot) in [(31, true), (31, false), (0, true)] {
            let shared = Shared::new(World::in_memory(), World::in_memory(), Default::default(), Default::default(), Default::default(),
                crate::ops::Ops::open(std::path::Path::new("tmp/enchant-arrow-credit/ops.json")), crate::skins::Settings::default());
            let mut world = shared.world_in(Dimension::Overworld).lock().unwrap();
            world.set_block_silently(0, 63, 0, crate::blocks::state_by_name("stone").unwrap());
            let mut blaze = crate::mob::Mob::new(30, crate::mob::Kind::Blaze, (0.5, 64.0, 2.5));
            blaze.no_ai = true;
            blaze.vitals.health = 1.0;
            shared.items_in(Dimension::Overworld).lock().unwrap().add_mob(blaze);
            let mut arrow = Arrow::from_mob(40, Dimension::Overworld, (0.5, 64.5, 0.5), (0.0, 0.0, 3.0), owner, "skeleton");
            arrow.fresh = false;
            if player_shot { arrow.shooter = Shooter::Player; }
            shared.arrows.lock().unwrap().spawn(arrow);
            step(&shared, Dimension::Overworld, &mut world);
            let items = shared.items_in(Dimension::Overworld).lock().unwrap();
            let victim = items.mobs().iter().find(|mob| mob.entity_id == 30).unwrap();
            assert!(victim.is_dead());
            let has_rod = items.items().iter().any(|item| crate::blocks::item_name(item.stack.item) == Some("blaze_rod"));
            // Фиксированный RNG id30/ticks0 выбирает один стержень, если
            // выполнено условие убийства игроком, до начисления достижений.
            assert_eq!(has_rod, owner != 0 && player_shot, "owner={owner}, player={player_shot}");
        }
    }

    #[test]
    fn mob_bow_copies_power_punch_and_flame_into_the_projectile() {
        let mut arrow = Arrow::from_mob(1, Dimension::Overworld, (0.0, 64.0, 0.0), (0.0, 0.0, 3.0), 2, "skeleton");
        let mut data = crate::stackdata::Data::default();
        for (name, level) in [("power", 5), ("punch", 2), ("flame", 1)] {
            data.set_enchant(crate::stackdata::enchantment_number(name).unwrap(), level);
        }
        arrow.enchant_from_bow(&data);
        assert_eq!((arrow.power, arrow.punch, arrow.flame), (5, 2, true));
        data.set_enchant(crate::stackdata::enchantment_number("flame").unwrap(), 0);
        assert!(arrow.flame, "стрела хранит чары своего выстрела");
        assert_eq!(damage_of(&arrow, 0.0), 15.0);
    }

    #[test]
    fn projectile_looting_uses_the_shooters_current_main_hand() {
        let sword = |level| {
            let mut data = crate::stackdata::Data::default();
            data.set_enchant(crate::stackdata::enchantment_number("looting").unwrap(), level);
            Stack::new(crate::blocks::item_named("diamond_sword").unwrap(), 1).with_extras(data)
        };
        for (initial, final_level, mob_shot, expected) in [(0, 3, false, 3), (3, 0, false, 0), (3, 3, true, 0)] {
            let shared = Shared::new(World::in_memory(), World::in_memory(), Default::default(), Default::default(), Default::default(),
                crate::ops::Ops::open(std::path::Path::new("tmp/enchant-looting/ops.json")), crate::skins::Settings::default());
            let uuid = [31; 16];
            let member = crate::players::Member {
                uuid, name: "Лучник".into(), entity_id: 31, game_mode: 0,
                position: crate::players::Position { x: 5.0, y: 64.0, z: 0.5, yaw: 0.0, pitch: 0.0, on_ground: true },
                skin: None, skin_parts: 0x7f, main_hand: 1, held: Some(sword(initial)),
                dimension: Dimension::Overworld, sneaking: false, swings: 0, shoved: ((0.0, 0.0, 0.0), 0), unsettled: 0,
            };
            shared.players.lock().unwrap().join(member, 31).unwrap();
            let mut world = shared.world_in(Dimension::Overworld).lock().unwrap();
            world.set_block_silently(0, 63, 0, crate::blocks::state_by_name("stone").unwrap());
            let mut mob = crate::mob::Mob::new(30, crate::mob::Kind::Llama, (0.5, 64.0, 2.5));
            mob.no_ai = true;
            mob.vitals.health = 1.0;
            shared.items_in(Dimension::Overworld).lock().unwrap().add_mob(mob);
            let mut arrow = Arrow::from_mob(40, Dimension::Overworld, (0.5, 64.5, 0.5), (0.0, 0.0, 3.0), 31, "skeleton");
            arrow.owner_uuid = uuid;
            if !mob_shot { arrow.shooter = Shooter::Player; }
            shared.arrows.lock().unwrap().spawn(arrow);
            // Первый такт — появление; до столкновения игрок меняет руку.
            step(&shared, Dimension::Overworld, &mut world);
            shared.players.lock().unwrap().set_held(&uuid, (final_level > 0).then(|| sword(final_level)));
            step(&shared, Dimension::Overworld, &mut world);
            let items = shared.items_in(Dimension::Overworld).lock().unwrap();
            let victim = items.mobs().iter().find(|mob| mob.entity_id == 30).unwrap();
            assert!(victim.is_dead());
            assert_eq!(victim.looting, expected, "{initial} -> {final_level}, моб={mob_shot}");
            let mut reference = victim.clone();
            reference.looting = expected;
            let expected_loot = crate::mob::loot(&reference, &mut crate::mob::Rng::new(30 ^ reference.ticks ^ 0x5EED));
            // step уже перенёс Loot из моба в настоящие сущности предметов.
            let dropped: Vec<_> = items.items().iter().map(|item| item.stack).collect();
            assert_eq!(dropped, expected_loot, "добыча рассчитана уже внутри смертельного попадания");
        }
    }

    #[test]
    fn thrown_impaling_adds_one_bonus_per_level_only_to_aquatic_targets() {
        // Вики, Impaling: «2.5 ... extra damage per level». Проверяем весь
        // путь столкновения: два отдельных helper раньше удваивали прибавку.
        for kind in [crate::mob::Kind::Guardian, crate::mob::Kind::Cow] {
            for level in [0, 1, 5] {
                let shared = Shared::new(World::in_memory(), World::in_memory(), Default::default(), Default::default(), Default::default(),
                    crate::ops::Ops::open(std::path::Path::new("tmp/enchant-impaling/ops.json")), crate::skins::Settings::default());
                let mut world = shared.world_in(Dimension::Overworld).lock().unwrap();
                world.set_block_silently(0, 63, 0, crate::blocks::state_by_name("stone").unwrap());
                let mut mob = crate::mob::Mob::new(30, kind, (0.5, 64.0, 2.5));
                mob.no_ai = true;
                let before = mob.vitals.health;
                shared.items_in(Dimension::Overworld).lock().unwrap().add_mob(mob);
                let mut data = crate::stackdata::Data::default();
                data.set_enchant(crate::stackdata::enchantment_number("impaling").unwrap(), level);
                let mut arrow = Arrow::from_mob(40, Dimension::Overworld, (0.5, 64.5, 0.5), (0.0, 0.0, 3.0), 0, "");
                arrow.fresh = false;
                arrow.trident = Some(Stack::new(crate::blocks::item_named("trident").unwrap(), 1).with_extras(data));
                shared.arrows.lock().unwrap().spawn(arrow);
                step(&shared, Dimension::Overworld, &mut world);
                let items = shared.items_in(Dimension::Overworld).lock().unwrap();
                let hit = items.mobs().iter().find(|mob| mob.entity_id == 30).unwrap();
                let expected = 8.0 + if kind == crate::mob::Kind::Guardian { 2.5 * level as f32 } else { 0.0 };
                assert_eq!(before - hit.vitals.health, expected, "{kind:?}, Impaling {level}");
            }
        }
    }

    #[test]
    fn lethal_flame_arrow_cooks_drops_before_death_is_processed() {
        for flame in [false, true] {
            let shared = Shared::new(World::in_memory(), World::in_memory(), Default::default(), Default::default(), Default::default(),
                crate::ops::Ops::open(std::path::Path::new("tmp/enchant-flame-loot/ops.json")), crate::skins::Settings::default());
            let mut world = shared.world_in(Dimension::Overworld).lock().unwrap();
            // Исключаем случайный рельеф in_memory на пути снаряда.
            for y in 64..=66 { for z in 0..=4 { world.set_block_silently(0, y, z, 0); } }
            let mut cow = crate::mob::Mob::new(30, crate::mob::Kind::Cow, (0.5, 64.0, 2.5));
            cow.no_ai = true;
            cow.vitals.health = 1.0;
            shared.items_in(Dimension::Overworld).lock().unwrap().add_mob(cow);
            let mut arrow = Arrow::from_mob(40, Dimension::Overworld, (0.5, 64.5, 0.5), (0.0, 0.0, 3.0), 31, "skeleton");
            arrow.flame = flame;
            arrow.fresh = false;
            shared.arrows.lock().unwrap().spawn(arrow);
            step(&shared, Dimension::Overworld, &mut world);
            let items = shared.items_in(Dimension::Overworld).lock().unwrap();
            let victim = items.mobs().iter().find(|mob| mob.entity_id == 30).unwrap();
            assert!(victim.is_dead());
            let drops: Vec<_> = items.items().iter().map(|item| item.stack).collect();
            let expected = if flame { "cooked_beef" } else { "beef" };
            let forbidden = if flame { "beef" } else { "cooked_beef" };
            assert!(drops.iter().any(|stack| crate::blocks::item_name(stack.item) == Some(expected)));
            assert!(drops.iter().all(|stack| crate::blocks::item_name(stack.item) != Some(forbidden)));
        }
    }

    #[test]
    fn flame_and_punch_apply_to_mobs_after_a_hit() {
        let mut arrow = Arrow::from_mob(1, Dimension::Overworld, (0.0, 0.0, 0.0), (0.0, 0.0, 3.0), 2, "skeleton");
        arrow.flame = true;
        arrow.punch = 2;
        let mut mob = crate::mob::Mob::new(3, crate::mob::Kind::Cow, (0.0, 0.0, 1.0));
        mob.hurt_burning(1.0, Source::Arrow, None, 1.0, 100).unwrap();
        arrow.hit_effects(&mut mob);
        assert_eq!(mob.vitals.fire, 100);
        assert!((mob.vz - 1.2).abs() < 1e-6);
        assert_eq!(mob.vy, 0.1);
    }
    #[test]
    fn healing_arrow_hurts_undead_for_harming_amount() {
        let mut mob = crate::mob::Mob::new(3, crate::mob::Kind::Zombie, (0.0, 0.0, 1.0));
        apply_effects(&mut mob, &tipped_effects(crate::survival::potion_number("strong_healing")));
        assert_eq!(mob.vitals.health, 8.0);
    }
}

#[cfg(test)]
mod itemaudit4_flame_tests {
    use super::*;
    #[test]
    fn flame_lights_only_dry_ignitable_blocks() {
        let mut world=World::in_memory();
        for name in ["campfire","soul_campfire","candle","red_candle","candle_cake","blue_candle_cake"] {
            let state=crate::blocks::state_from_text(&format!("{name}[lit=false]")).unwrap();
            world.set_block(0,64,0,state);assert!(ignite_block(&mut world,(0,64,0)),"{name}");
            assert_eq!(crate::blocks::orientation_of(state).unwrap().value(world.get_block(0,64,0),"lit"),Some("true"));
        }
        for spec in ["oak_log","candle[waterlogged=true]","campfire[waterlogged=true,lit=false]"] {
            let state=crate::blocks::state_from_text(spec).unwrap();world.set_block(0,64,0,state);
            assert!(!ignite_block(&mut world,(0,64,0)));assert_eq!(world.get_block(0,64,0),state);
        }
        world.set_block(0,64,0,crate::blocks::state_by_name("tnt").unwrap());assert!(ignite_block(&mut world,(0,64,0)));assert_eq!(world.get_block(0,64,0),crate::world::AIR);
    }
}

#[cfg(test)]
mod itemaudit4_punch_tests {
    use super::*;
    #[test]
    fn punch_levels_scale_horizontal_impulse_and_respect_immunity() {
        for level in 1..=2 {
            let mut arrow=Arrow::from_mob(1,Dimension::Overworld,(0.0,0.0,0.0),(0.0,0.0,2.0),0,"skeleton");arrow.punch=level;
            let mut cow=crate::mob::Mob::new(2,crate::mob::Kind::Cow,(0.0,0.0,0.0));arrow.hit_effects(&mut cow);
            assert!((cow.vz-0.6*f64::from(level)).abs()<1e-6);assert!((cow.vy-0.1).abs()<1e-6);
            let mut golem=crate::mob::Mob::new(3,crate::mob::Kind::IronGolem,(0.0,0.0,0.0));arrow.hit_effects(&mut golem);assert_eq!(golem.vz,0.0);
        }
    }
}
