// Стрелы игроков: полёт, попадание, застревание, подбор.
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
// клиентам (network/play/bow.rs).

use crate::combat::PlayerStrike;
use crate::dimension::Dimension;
use crate::inventory::Stack;
use crate::shared::Shared;
use crate::survival::Source;
use crate::world::World;

/// Номер вида сущности «стрела» (minecraft-data 26.1, entities.json).
pub const ARROW_TYPE: i32 = 6;

/// Тяжесть и сопротивление стрелы (вики, Arrow).
const GRAVITY: f64 = 0.05;
const AIR_DRAG: f64 = 0.99;
const WATER_DRAG: f64 = 0.6;

/// Сколько тактов стрела лежит в блоке, прежде чем исчезнуть: минута.
const GROUND_LIFE: u32 = 1200;

/// Сколько тактов после появления стрела не задевает стрелка.
const OWNER_SAFE: u32 = 5;

/// Сколько тактов после попадания в блок стрела ещё нельзя подобрать. [проверить]
const PICKUP_DELAY: u32 = 5;

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
    /// Кто выпустил (номер сущности, 0 — никто), его имя и опознаватель.
    pub owner: i32,
    pub owner_uuid: [u8; 16],
    pub owner_name: String,
    /// Полностью натянутая стрела бьёт сильнее и летит с искрами.
    pub crit: bool,
    /// Уровни «силы» и «отдачи» лука, «горящая стрела».
    pub power: i32,
    pub punch: i32,
    pub flame: bool,
    pub pickup: Pickup,
    pub in_ground: bool,
    pub ground_ticks: u32,
    pub age: u32,
    /// Сколько скорость менялась: клиенты досылают скорость при каждом изменении.
    pub moves: u32,
    /// Как дошла до клиентов: попала в блок (для звука на следующей рассылке).
    pub landed: bool,
    /// Место и скорость при появлении: клиенту сперва уходит оно.
    pub start: ((f64, f64, f64), (f64, f64, f64)),
}

impl Arrow {
    /// Угол по сторонам света и наклон по скорости (для пакетов).
    pub fn angles(&self) -> (f32, f32) {
        let horizontal = self.vx.hypot(self.vz);
        let yaw = (-self.vx).atan2(self.vz).to_degrees() as f32;
        let pitch = (-self.vy).atan2(horizontal).to_degrees() as f32;

        (yaw, pitch)
    }
}

/// Все стрелы сервера.
#[derive(Default)]
pub struct Arrows {
    list: Vec<Arrow>,
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

/// Урон стрелы, влетевшей со скоростью `speed`: `ceil(скорость · базовый)`,
/// у критической прибавка (вики, Arrow).
pub fn damage_of(arrow: &Arrow, roll: f32) -> f32 {
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

    Some(near)
}

/// Кого стрела задела на пути: игрок или моб, и в какой доле пути.
enum Target {
    Player { entity_id: i32 },
    Mob { entity_id: i32 },
}

/// Игрок рядом со стрелой: номер, место, приседает ли, режим, опознаватель, имя.
type Bystander = (i32, (f64, f64, f64), bool, i32, [u8; 16], String);

/// Ход стрел одного измерения за такт. Мир нужен для блоков, игроки и
/// мобы — для целей.
pub fn step(shared: &Shared, dimension: Dimension, world: &mut World) {
    // Стрелы клиентам рассылает слежение: пока они есть (и в такт, когда
    // последняя пропала), оно должно идти — даже если в мире нет ни одного
    // моба (pvp-bow на пустой площадке: стрелу никто не видел).
    let any = shared.arrows.lock().expect("стрелы захвачены другим потоком").list.iter().any(|arrow| arrow.dimension == dimension);

    if any {
        world.note_tracker();
    }

    let members: Vec<Bystander> = shared
        .players
        .lock()
        .expect("список игроков захвачен другим потоком")
        .members()
        .iter()
        .filter(|member| member.dimension == dimension)
        .map(|member| (member.entity_id, (member.position.x, member.position.y, member.position.z), member.sneaking, member.game_mode, member.uuid, member.name.clone()))
        .collect();

    let mut items = shared.items_in(dimension).lock().expect("предметы захвачены другим потоком");
    let mut arrows = shared.arrows.lock().expect("стрелы захвачены другим потоком");
    let mut pickups: Vec<([u8; 16], Stack)> = Vec::new();
    let mut removed: Vec<i32> = Vec::new();

    for arrow in arrows.list.iter_mut().filter(|arrow| arrow.dimension == dimension) {
        arrow.age += 1;

        if arrow.in_ground {
            arrow.ground_ticks += 1;

            // Опору убрали — стрела снова летит.
            let state = world.get_block(arrow.x.floor() as i32, arrow.y.floor() as i32, arrow.z.floor() as i32);

            if !crate::mob::solid(state) {
                arrow.in_ground = false;
                arrow.ground_ticks = 0;
                arrow.moves += 1;
            } else if arrow.ground_ticks >= GROUND_LIFE {
                removed.push(arrow.entity_id);
                continue;
            } else if arrow.ground_ticks >= PICKUP_DELAY && arrow.pickup != Pickup::No {
                // Подбор: игрок рядом (ящик игрока расширен на блок вширь и
                // полблока ввысь и вниз). [проверить]
                let picker = members.iter().find(|(_, at, _, mode, _, _)| {
                    (*mode == 0 || *mode == 1 || *mode == 2)
                        && (arrow.pickup == Pickup::Yes || *mode == 1)
                        && (at.0 - arrow.x).abs() <= 1.3
                        && (arrow.y - at.1 + 0.5).abs() <= 2.0
                        && (at.2 - arrow.z).abs() <= 1.3
                });

                if let Some((_, _, _, _, uuid, _)) = picker {
                    pickups.push((*uuid, Stack::new(crate::blocks::item_named("arrow").unwrap_or(0), 1)));
                    removed.push(arrow.entity_id);
                }
            }

            continue;
        }

        let from = (arrow.x, arrow.y, arrow.z);
        let to = (arrow.x + arrow.vx, arrow.y + arrow.vy, arrow.z + arrow.vz);
        let mut best: Option<(f64, Target)> = None;

        // Игроки.
        for (entity_id, at, sneaking, mode, _, _) in &members {
            if *mode == 3 || (*entity_id == arrow.owner && arrow.age <= OWNER_SAFE) {
                continue;
            }

            let height = if *sneaking { 1.5 } else { 1.8 };

            if let Some(part) = sweep_box(from, to, (at.0, at.1, at.2), 0.3, height)
                && best.as_ref().is_none_or(|(known, _)| part < *known)
            {
                best = Some((part, Target::Player { entity_id: *entity_id }));
            }
        }

        // Мобы.
        for mob in items.mobs() {
            if mob.is_dead() {
                continue;
            }

            let (width, height) = mob.size();

            if let Some(part) = sweep_box(from, to, (mob.x, mob.y, mob.z), width / 2.0, height)
                && best.as_ref().is_none_or(|(known, _)| part < *known)
            {
                best = Some((part, Target::Mob { entity_id: mob.entity_id }));
            }
        }

        // Блоки: шагом в четверть блока вдоль пути.
        let length = ((to.0 - from.0).powi(2) + (to.1 - from.1).powi(2) + (to.2 - from.2).powi(2)).sqrt();
        let steps = ((length / 0.25).ceil() as usize).max(1);
        let mut block_part = None;

        for index in 1..=steps {
            let part = index as f64 / steps as f64;
            let at = (from.0 + (to.0 - from.0) * part, from.1 + (to.1 - from.1) * part, from.2 + (to.2 - from.2) * part);

            if crate::mob::solid(world.get_block(at.0.floor() as i32, at.1.floor() as i32, at.2.floor() as i32)) {
                block_part = Some(part);
                break;
            }
        }

        let entity_first = match (&best, block_part) {
            (Some((part, _)), Some(block)) => *part <= block,
            (Some(_), None) => true,
            _ => false,
        };

        if entity_first && let Some((part, target)) = best {
            arrow.x = from.0 + (to.0 - from.0) * part;
            arrow.y = from.1 + (to.1 - from.1) * part;
            arrow.z = from.2 + (to.2 - from.2) * part;

            let roll = world.random_unit();
            let damage = damage_of(arrow, roll);

            match target {
                Target::Player { entity_id } => {
                    world.note_player_strike(PlayerStrike {
                        victim: entity_id,
                        attacker: arrow.owner,
                        attacker_uuid: arrow.owner_uuid,
                        attacker_name: arrow.owner_name.clone(),
                        from: (arrow.x, arrow.y, arrow.z),
                        yaw: arrow.angles().0,
                        damage,
                        source: Source::Arrow,
                        extra_knockback: arrow.punch as f32 * 1.2,
                        weapon: None,
                        disables_shield_ticks: 0,
                        fire_ticks: if arrow.flame { 100 } else { 0 },
                    });
                }
                Target::Mob { entity_id } => {
                    let speed = arrow.vx.hypot(arrow.vz).max(1.0e-9);
                    let push = crate::mob::Push { dir: (arrow.vx / speed, arrow.vz / speed), strength: crate::mob::BASE_KNOCKBACK, cause: arrow.owner };

                    if let Some(mob) = items.mobs_mut().iter_mut().find(|mob| mob.entity_id == entity_id) {
                        mob.credit_player();
                        mob.hurt(damage, Source::Arrow, Some(push));
                    }
                }
            }

            removed.push(arrow.entity_id);
            world.note_sound_neutral_at((arrow.x, arrow.y, arrow.z), "minecraft:entity.arrow.hit", 1.0, 1.0);
            continue;
        }

        if let Some(part) = block_part {
            arrow.x = from.0 + (to.0 - from.0) * part;
            arrow.y = from.1 + (to.1 - from.1) * part;
            arrow.z = from.2 + (to.2 - from.2) * part;
            (arrow.vx, arrow.vy, arrow.vz) = (0.0, 0.0, 0.0);
            arrow.in_ground = true;
            arrow.ground_ticks = 0;
            arrow.landed = true;
            arrow.moves += 1;
            world.note_sound_neutral_at((arrow.x, arrow.y, arrow.z), "minecraft:entity.arrow.hit", 1.0, 1.0);
            continue;
        }

        arrow.x = to.0;
        arrow.y = to.1;
        arrow.z = to.2;

        let in_water = crate::mob::is_water(world.get_block(arrow.x.floor() as i32, arrow.y.floor() as i32, arrow.z.floor() as i32));
        let drag = if in_water { WATER_DRAG } else { AIR_DRAG };

        arrow.vx *= drag;
        arrow.vy = arrow.vy * drag - GRAVITY;
        arrow.vz *= drag;
        arrow.moves += 1;

        // Улетела за край мира.
        if arrow.y < -128.0 {
            removed.push(arrow.entity_id);
        }
    }

    if !removed.is_empty() {
        arrows.list.retain(|arrow| !removed.contains(&arrow.entity_id));
    }

    drop(arrows);
    drop(items);

    if !pickups.is_empty() {
        let mut players = shared.players.lock().expect("список игроков захвачен другим потоком");

        for (uuid, stack) in pickups {
            players.order(&uuid, crate::players::Order::Give { stack });
        }
    }
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
            owner: 0,
            owner_uuid: [0; 16],
            owner_name: String::new(),
            crit: false,
            power: 0,
            punch: 0,
            flame: false,
            pickup: Pickup::Yes,
            in_ground: false,
            ground_ticks: 0,
            age: 0,
            moves: 0,
            landed: false,
            start: ((0.0, 0.0, 0.0), (0.0, 0.0, 3.0)),
        };

        assert_eq!(arrow.angles(), (0.0, 0.0));

        arrow.vx = -3.0;
        arrow.vz = 0.0;

        assert!((arrow.angles().0 - 90.0).abs() < 1e-4);
        assert_eq!(damage_of(&arrow, 0.0), 6.0);
    }
}
