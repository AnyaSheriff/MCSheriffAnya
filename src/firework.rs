// Ракеты фейерверка: полёт, взрыв (событие сущности 17) и урон от него.
//
// Вики, Firework Rocket: пущенная с блока ракета летит вверх — скорость 0,05
// вверх и едва заметная вбок (нормальное распределение, σ 0,001), за такт
// вбок ×1,15, вверх +0,04; живёт 10·(полёт + 1) + 0…5 + 0…6 тактов, потом
// взрывается. Из арбалета (и раздатчика) летит прямо, скорость не меняется
// («ShotAtAngle»), о сущность взрывается сразу. Ракета со звёздами взрывается
// и о блок; без звёзд скользит по нему.
//
// Урон: только у ракеты со звёздами — 5 + 2·звёзды, всем живым в 5 блоках, до
// кого виден прямой путь (вики: «within 5 blocks and not obstructed by solid
// blocks»), со спадом по расстоянию √((5 − d)/5) [проверить чёрным ящиком].
//
// Ракеты лежат в `Shared::thrown` рядом с зельями; их ход — в такте снарядов
// (thrown.rs), рассказ клиентам — слежение снарядов (network/play/thrown.rs).

use crate::dimension::Dimension;
use crate::inventory::Stack;
use crate::stackdata::Fireworks;
use crate::survival::Source;
use crate::world::World;

/// Номер вида сущности (minecraft-data 26.1, entities.json).
pub const ROCKET_TYPE: i32 = 53;

/// Полуширина и высота ракеты (вики: 0,25 × 0,25).
const HALF: f64 = 0.125;
const HEIGHT: f64 = 0.25;

/// Ракета из арбалета: 1,6 блока за такт (вики, Crossbow: «32 m/s»).
pub const CROSSBOW_SPEED: f64 = 1.6;

/// Радиус урона (вики: «within 5 blocks»).
const BLAST_RADIUS: f64 = 5.0;

/// Летящая ракета.
#[derive(Clone, Debug)]
pub struct Rocket {
    pub entity_id: i32,
    pub uuid: [u8; 16],
    pub dimension: Dimension,
    /// Сама ракета (свойство 8).
    pub stack: Stack,
    pub fireworks: Fireworks,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub vx: f64,
    pub vy: f64,
    pub vz: f64,
    /// Сколько тактов летит и сколько проживёт.
    pub life: u32,
    pub lifetime: u32,
    /// Пущена из арбалета: летит прямо (свойство 10).
    pub angled: bool,
    /// Кто пустил (0 — никто).
    pub owner: i32,
    pub owner_uuid: [u8; 16],
    pub owner_name: String,
    /// Ракета ещё внутри пустившего: его не задевает.
    pub in_owner: bool,
    /// Поворот (угол, наклон) по полёту.
    pub rot: (f32, f32),
    /// Место, скорость и поворот до хода этого такта (для слежения).
    pub prev: crate::thrown::Snapshot,
    /// Место и скорость при появлении.
    pub start: ((f64, f64, f64), (f64, f64, f64)),
    /// Поворот при появлении.
    pub start_rot: (f32, f32),
}

/// Срок жизни ракеты с полётом `flight`: 10·(полёт + 1) + 0…5 + 0…6 (вики,
/// Firework Rocket: «LifeTime»). `a`, `b` — случайные доли [0, 1).
pub fn lifetime(flight: i8, a: f32, b: f32) -> u32 {
    let base = 10 * (i32::from(flight) + 1);

    (base + (a * 6.0) as i32 + (b * 7.0) as i32).max(0) as u32
}

/// Сила взрыва: 5 + 2 за звезду, без звёзд — 0 (вики, Firework Rocket).
pub fn blast_power(stars: usize) -> f32 {
    if stars == 0 { 0.0 } else { 5.0 + 2.0 * stars as f32 }
}

/// Урон на расстоянии `distance` (до места сущности): `power·√((5 − d)/5)`,
/// дальше 5 блоков — нет [проверить чёрным ящиком].
pub fn blast_damage(power: f32, distance: f64) -> Option<f32> {
    (distance <= BLAST_RADIUS).then(|| power * ((BLAST_RADIUS - distance) / BLAST_RADIUS).sqrt() as f32)
}

impl Rocket {
    /// Ракета `stack` в точке `at` со скоростью `velocity`.
    #[allow(clippy::too_many_arguments)]
    pub fn new(entity_id: i32, dimension: Dimension, stack: Stack, at: (f64, f64, f64), velocity: (f64, f64, f64), angled: bool, owner: (i32, [u8; 16], String), lifetime: u32) -> Self {
        // Пущенная из арбалета повёрнута по выстрелу, с блока — 0/0 (чёрный
        // ящик, firework-crossbow и firework-launch: поворот при появлении).
        let rot = if angled { crate::thrown::angles(velocity) } else { (0.0, 0.0) };

        Self {
            entity_id,
            uuid: crate::items::uuid_for(entity_id),
            dimension,
            stack,
            fireworks: stack.extras().rocket(),
            x: at.0,
            y: at.1,
            z: at.2,
            vx: velocity.0,
            vy: velocity.1,
            vz: velocity.2,
            life: 0,
            lifetime,
            angled,
            owner: owner.0,
            owner_uuid: owner.1,
            owner_name: owner.2,
            in_owner: owner.0 != 0,
            rot,
            prev: (at, velocity, rot),
            start: (at, velocity),
            start_rot: rot,
        }
    }

    pub fn power(&self) -> f32 {
        blast_power(self.fireworks.explosions.len())
    }
}

/// Взрыв ракеты в этом такте: событие 17 и уход (слежение).
#[derive(Clone, Debug)]
pub struct Blast {
    pub dimension: Dimension,
    pub rocket: i32,
    /// Ракета в миг взрыва: слежение ещё шлёт её сверку места этого такта
    /// (чёрный ящик, firework-blast: сверка на +10 и взрыв там же).
    pub last: Rocket,
}

/// Мир глазами ракеты (для упора о блоки).
struct View<'a>(&'a World);

impl crate::mob::View for View<'_> {
    fn block(&self, x: i32, y: i32, z: i32) -> i32 {
        self.0.get_block(x, y, z)
    }

    fn loaded(&self, chunk_x: i32, chunk_z: i32) -> bool {
        self.0.has_chunk(chunk_x, chunk_z)
    }
}

/// Сдвиг ящика ракеты с упором: вверх-вниз, потом по большей из осей.
/// Скорость упор не гасит: у потолка ракета по-прежнему рвётся вверх и на
/// следующем такте задевает его (чёрный ящик, firework-blast: на +10 у
/// прижатой к потолку скорость 0,45 и взрыв).
fn move_box(world: &World, rocket: &mut Rocket) {
    let view = View(world);
    let mut min = (rocket.x - HALF, rocket.y, rocket.z - HALF);

    min.1 += crate::mob::clip_y(&view, min, HALF, HEIGHT, rocket.vy);

    if rocket.vx.abs() >= rocket.vz.abs() {
        min.0 += crate::mob::clip_x(&view, min, HALF, HEIGHT, rocket.vx);
        min.2 += crate::mob::clip_z(&view, min, HALF, HEIGHT, rocket.vz);
    } else {
        min.2 += crate::mob::clip_z(&view, min, HALF, HEIGHT, rocket.vz);
        min.0 += crate::mob::clip_x(&view, min, HALF, HEIGHT, rocket.vx);
    }

    rocket.x = min.0 + HALF;
    rocket.y = min.1;
    rocket.z = min.2 + HALF;
}

/// Кого задела ракета.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Struck {
    Player(i32),
    Mob(i32),
}

/// Игрок рядом: номер, место, приседает ли, режим.
pub type Bystander = (i32, (f64, f64, f64), bool, i32);

/// Урон от взрыва: игроку — удар со стороны ракеты, мобу — сразу.
pub struct Wound {
    pub target: Struck,
    pub damage: f32,
    pub from: (f64, f64, f64),
}

/// Ход ракет измерения за такт: полёт, попадание, взрыв по сроку. Взрывы
/// уходят в `blasts`, раны — в `wounds` (их наносит thrown.rs после хода).
pub fn step(world: &mut World, rockets: &mut Vec<Rocket>, dimension: Dimension, members: &[Bystander], mobs: &[crate::mob::Mob], blasts: &mut Vec<Blast>, wounds: &mut Vec<(Rocket, Wound)>) {
    let height = |sneaking: bool| if sneaking { 1.5 } else { 1.8 };
    let mut gone = Vec::new();

    for rocket in rockets.iter_mut().filter(|r| r.dimension == dimension) {
        if !world.has_chunk((rocket.x.floor() as i32).div_euclid(16), (rocket.z.floor() as i32).div_euclid(16)) {
            continue;
        }

        rocket.prev = ((rocket.x, rocket.y, rocket.z), (rocket.vx, rocket.vy, rocket.vz), rocket.rot);

        // Пущенная с блока разгоняется: вбок ×1,15, вверх +0,04 (вики).
        if !rocket.angled {
            rocket.vx *= 1.15;
            rocket.vy += 0.04;
            rocket.vz *= 1.15;
        }

        let from = (rocket.x, rocket.y, rocket.z);
        let to = (rocket.x + rocket.vx, rocket.y + rocket.vy, rocket.z + rocket.vz);
        let centre = |p: (f64, f64, f64)| (p.0, p.1 + HEIGHT / 2.0, p.2);

        // Пустивший не задевается, пока ракета в нём.
        if rocket.in_owner && !members.iter().any(|m| m.0 == rocket.owner && crate::thrown::overlap(from, HALF, HEIGHT, m.1, 0.3, height(m.2))) {
            rocket.in_owner = false;
        }

        // Попадание ищется по пути этого такта до хода, а взрыв — уже после
        // хода, на новом месте (чёрный ящик, firework-crossbow: ракета прошла
        // 1,6 блока за такт и взорвалась на +2, урон 5,858 = 7·√((5 − 1,498)/5)
        // — расстояние от места после хода до ног жертвы).
        let mut best: Option<(f64, Struck)> = None;

        for (id, at, sneaking, mode) in members {
            if *mode == 3 || (*id == rocket.owner && rocket.in_owner) {
                continue;
            }

            if let Some(part) = crate::thrown::sweep(centre(from), centre(to), *at, 0.3 + HALF, height(*sneaking) + HEIGHT).map(|p| p.max(0.0))
                && best.is_none_or(|(known, _)| part < known)
            {
                best = Some((part, Struck::Player(*id)));
            }
        }

        for mob in mobs {
            if mob.is_dead() || mob.entity_id == rocket.owner || matches!(mob.kind, crate::mob::Kind::EndCrystal) {
                continue;
            }

            let (width, mob_height) = mob.size();

            if let Some(part) = crate::thrown::sweep(centre(from), centre(to), (mob.x, mob.y - HEIGHT / 2.0, mob.z), width / 2.0 + HALF, mob_height + HEIGHT)
                && best.is_none_or(|(known, _)| part < known)
            {
                best = Some((part, Struck::Mob(mob.entity_id)));
            }
        }

        let block = crate::thrown::first_block(world, from, to).map(|(part, _)| part);
        let entity_first = match (best, block) {
            (Some((part, _)), Some(block_part)) => part <= block_part,
            (Some(_), None) => true,
            _ => false,
        };

        move_box(world, rocket);

        // О сущность — взрыв сразу (вики, Crossbow: «explodes on contact»);
        // о блок — только со звёздами, без них ракета скользит (вики, Firework
        // Rocket; чёрный ящик, firework-blast: пущенная внутри камня взорвалась
        // в том же такте).
        let mut explode = entity_first || (block.is_some() && !rocket.fireworks.explosions.is_empty());

        // Поворот догоняет полёт на пятую часть за такт, как у зелий; у
        // пущенной с блока он начинается с 0/0 (чёрный ящик, firework-launch:
        // наклон 56 и 62 из 256 на +10 и +20).
        let (yaw, pitch) = crate::thrown::angles((rocket.vx, rocket.vy, rocket.vz));
        let turn = (yaw - rocket.rot.0 + 540.0).rem_euclid(360.0) - 180.0;

        rocket.rot = (rocket.rot.0 + turn * 0.2, rocket.rot.1 + (pitch - rocket.rot.1) * 0.2);

        // Звук запуска — в первом такте полёта, после хода и попадания (вики:
        // громкость 3, высота 1; чёрный ящик, firework-blast: после события 17).
        if rocket.life == 0 {
            world.note_sound_ambient_at((rocket.x, rocket.y, rocket.z), "minecraft:entity.firework_rocket.launch", 3.0, 1.0);
        }

        rocket.life += 1;

        if rocket.life > rocket.lifetime {
            explode = true;
        }

        if !explode {
            if rocket.y < -128.0 {
                gone.push(rocket.entity_id);
            }
            continue;
        }

        gone.push(rocket.entity_id);
        blasts.push(Blast { dimension, rocket: rocket.entity_id, last: rocket.clone() });

        let power = rocket.power();

        if power <= 0.0 {
            continue;
        }

        let at = (rocket.x, rocket.y, rocket.z);
        // Прямой путь от ракеты до ног или до середины тела (вики:
        // «not obstructed by solid blocks») [проверить точки].
        let seen = |pos: (f64, f64, f64), tall: f64| (0..2).any(|i| crate::thrown::first_block(world, at, (pos.0, pos.1 + tall * 0.5 * f64::from(i), pos.2)).is_none());
        let distance = |pos: (f64, f64, f64)| ((pos.0 - at.0).powi(2) + (pos.1 - at.1).powi(2) + (pos.2 - at.2).powi(2)).sqrt();

        for (id, pos, sneaking, mode) in members {
            if *mode == 3 {
                continue;
            }

            if let Some(damage) = blast_damage(power, distance(*pos))
                && seen(*pos, height(*sneaking))
            {
                wounds.push((rocket.clone(), Wound { target: Struck::Player(*id), damage, from: at }));
            }
        }

        for mob in mobs.iter().filter(|mob| !mob.is_dead() && !matches!(mob.kind, crate::mob::Kind::EndCrystal)) {
            let (_, mob_height) = mob.size();

            if let Some(damage) = blast_damage(power, distance((mob.x, mob.y, mob.z)))
                && seen((mob.x, mob.y, mob.z), mob_height)
            {
                wounds.push((rocket.clone(), Wound { target: Struck::Mob(mob.entity_id), damage, from: at }));
            }
        }
    }

    rockets.retain(|r| !gone.contains(&r.entity_id));
}

/// Источник урона ракеты.
pub const SOURCE: Source = Source::Fireworks;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifetime_is_ten_per_flight_plus_two_rolls() {
        assert_eq!(lifetime(1, 0.0, 0.0), 20);
        assert_eq!(lifetime(1, 0.999, 0.999), 31);
        assert_eq!(lifetime(0, 0.5, 0.5), 10 + 3 + 3);
        assert_eq!(lifetime(3, 0.0, 0.0), 40);
    }

    #[test]
    fn blast_damage_falls_off_over_five_blocks() {
        assert_eq!(blast_power(0), 0.0);
        assert_eq!(blast_power(1), 7.0);
        assert_eq!(blast_power(7), 19.0);
        assert_eq!(blast_damage(7.0, 0.0), Some(7.0));
        assert!(blast_damage(7.0, 5.01).is_none());
        // На 1,5 блока одна звезда — около 5,9 (вики, Crossbow: «5 to 6»).
        let near = blast_damage(7.0, 1.5).unwrap();

        assert!((5.0..6.0).contains(&near));
    }

    #[test]
    fn a_launched_rocket_climbs_and_a_ceiling_stops_it() {
        let mut world = World::in_memory();
        let stone = crate::blocks::state_by_name("stone").expect("камень есть");
        let item = crate::blocks::item_named("firework_rocket").expect("ракета есть");

        world.set_block(0, 3, 0, stone);

        let mut rockets = vec![Rocket::new(1, Dimension::Overworld, Stack::new(item, 1), (0.5, 0.15, 0.5), (0.0, 0.05, 0.0), false, (0, [0; 16], String::new()), 100)];
        let (mut blasts, mut wounds) = (Vec::new(), Vec::new());

        // Первый ход: 0,05 + 0,04 вверх.
        step(&mut world, &mut rockets, Dimension::Overworld, &[], &[], &mut blasts, &mut wounds);
        assert!((rockets[0].y - 0.24).abs() < 1e-9);

        // Без звёзд о потолок не взрывается: упирается под ним.
        for _ in 0..20 {
            step(&mut world, &mut rockets, Dimension::Overworld, &[], &[], &mut blasts, &mut wounds);
        }

        assert!(blasts.is_empty());
        assert!((rockets[0].y - (3.0 - HEIGHT)).abs() < 1e-9);

        // Со звездой: к потолку прижата после 10 ходов, на 11-м задевает его
        // и взрывается (чёрный ящик, firework-blast: событие 17 на +10).
        let (_, star) = crate::stackdata::parse_item_spec("firework_rocket[fireworks={flight_duration:3,explosions:[{shape:\"star\"}]}]").unwrap();
        let mut rockets = vec![Rocket::new(2, Dimension::Overworld, Stack::new(item, 1).with_extras(star), (0.5, 0.15, 0.5), (0.0, 0.05, 0.0), false, (0, [0; 16], String::new()), 100)];

        for _ in 0..10 {
            step(&mut world, &mut rockets, Dimension::Overworld, &[], &[], &mut blasts, &mut wounds);
        }

        assert!(blasts.is_empty());
        assert!((rockets[0].y - (3.0 - HEIGHT)).abs() < 1e-9 && rockets[0].vy > 0.4);

        step(&mut world, &mut rockets, Dimension::Overworld, &[], &[], &mut blasts, &mut wounds);
        assert_eq!(blasts.len(), 1);
        assert!(rockets.is_empty());
    }
}
