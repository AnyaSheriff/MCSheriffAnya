// Мысли мобов: цели по порядку важности.
//
// Кода игры здесь нет; порядок и числа — со страниц minecraft.wiki (Mob AI,
// Pathfinding, Breeding, Cow, Pig, Sheep, Chicken). Цели выбираются по
// старшинству, как в игре: паника, разведение, приманка, следование за
// родителем, овца щиплет траву, прогулка; взгляд (на игрока, случайный) — сам
// по себе. Случайность у оригинала своя, повторить её один в один нельзя, —
// здесь она честная (Rng), а вероятности — с вики:
//
// - прогулка: раз в 120 тактов в среднем (`interval 120`), точка в 10 блоках
//   по горизонтали и 7 по вертикали;
// - взгляд на игрока: шанс 0,02 за такт, дальность 6 блоков, 40–80 тактов;
// - случайный взгляд: шанс 0,02 за такт, 20–40 тактов;
// - приманка: игрок с едой в руке в 10 блоках, идём со скоростью 1,25;
// - овца щиплет траву с шансом 1/1000 за такт (у детёныша 1/50), 40 тактов;
// - паника: от удара 100 тактов, скорость вдвое (у курицы 1,4).
//
// Что не сверено 1:1 с оригиналом (там всё случайно), помечено «[проверить]».

use super::path::{self, Cell};
use super::{Kind, Mob, Rng, View};

/// Сколько тактов подряд пара стоит рядом до детёныша: у оригинала он рождается
/// на 70-м такте, когда корм дали на 10-м и 12-м (чёрный ящик, cow-breed).
const BREED_TICKS: i32 = 59;

/// Игрок, которого мобы замечают.
#[derive(Clone, Copy, Debug)]
pub struct Watcher {
    /// Номер сущности игрока: по нему моб держит цель.
    pub id: i32,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub eye: f64,

    /// Что в основной и во второй руке (имя предмета).
    pub holding: [Option<&'static str>; 2],

    /// Наблюдателя мобы не видят.
    pub spectator: bool,

    /// Выживание или приключение: цель для враждебных (творческого они не бьют).
    pub targetable: bool,

    /// Куда смотрит (поворот и наклон, градусы) и не тыква ли на голове:
    /// взгляд злит эндермена, тыква — нет (вики, Enderman).
    pub look: (f32, f32),
    pub pumpkin: bool,

    /// Хотя бы одна вещь брони из золота на нём: пиглины такого не трогают
    /// (вики, Piglin: «wearing any piece of golden armor»).
    pub gold_armor: bool,
}

/// Сородич рядом: для разведения и следования за родителем.
#[derive(Clone, Copy, Debug)]
pub struct Mate {
    pub id: i32,
    pub kind: Kind,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub love: i32,
    pub baby: bool,
    pub ready: bool,
}

/// Что моб видит вокруг за такт.
pub struct Ctx<'a> {
    pub players: &'a [Watcher],
    pub mates: &'a [Mate],

    /// Сложность мира (0…3).
    pub difficulty: u8,

    /// Небесный свет под открытым небом в этот час: днём 15, ночью 4.
    pub sky: i32,

    /// Солнце жжёт нежить: от такта 23 460 до 12 540 суток (вики, Daylight
    /// cycle: мобы горят «27 seconds before the beginning of a new day»).
    pub sun_up: bool,
}

/// Что съела овца.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Eat {
    pub at: Cell,

    /// Съедена трава на земле (тогда блок под ногами станет землёй), иначе —
    /// трава в клетке (пропадает).
    pub block: bool,
}

/// Итог мысли за такт: как двигаться и что случилось с миром.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plan {
    /// Доля скорости шага (0 — стоять).
    pub speed: f64,
    pub jump: bool,

    /// Разведение состоялось: номер партнёра.
    pub breed: Option<i32>,

    /// Овца доела.
    pub eat: Option<Eat>,

    /// Курица снесла яйцо.
    pub lay_egg: bool,

    /// Враждебный: удар в ближнем бою, выстрел, взрыв (сила).
    pub melee: Option<super::hostile::Melee>,
    pub shot: Option<super::hostile::Shot>,
    pub blast: Option<f32>,

    /// Шалкер выпустил снаряд в этого игрока; снаряд попал в этого игрока.
    pub bullet: Option<i32>,
    pub bullet_hit: Option<i32>,

    /// Эндермен взял блок из клетки; поставил блок (клетка, состояние).
    pub take: Option<(i32, i32, i32)>,
    pub put: Option<((i32, i32, i32), i32)>,

    /// Огненный шар ифрита или гаста (mob/nether.rs).
    pub launch: Option<super::nether::Launch>,

    /// Укус волка по мобу: цель, урон, направление толчка (mob/tame.rs).
    pub bite: Option<(i32, f32, (f64, f64))>,

    /// Удар железного голема: цель подбрасывает (mob/golem.rs).
    pub fling: bool,

    /// Снежок снежного голема: откуда и с какой скоростью (mob/golem.rs).
    pub snowball: Option<super::golem::Throw>,
    pub spit: Option<super::golem::Throw>,

    /// Заряд ветра бриза: откуда и с какой скоростью (mob/breeze.rs).
    pub wind_charge: Option<super::golem::Throw>,
}

/// Состояние мыслей.
#[derive(Clone, Debug)]
pub struct Brain {
    /// Клетки пути, что остались, и множитель скорости.
    path: Vec<Cell>,
    speed: f64,

    /// Сколько тактов ещё паника.
    panic: i32,

    /// Куда смотрит голова (градусы) и сколько тактов ещё.
    look: Option<Look>,

    /// Овца жуёт: сколько тактов осталось (0 — нет).
    pub eating: i32,

    /// Сколько тактов подряд рядом с партнёром: 60 — детёныш.
    breed: i32,

    /// Тактов с последнего шага вперёд: застрял — путь бросается.
    stuck: i32,
    last: (f64, f64),

    /// Тактов до следующей ненужной проверки пути к приманке.
    pub repath: i32,

    /// Счёт для ambient-звука (вики, Sounds: раз в несколько секунд).
    ambient: i32,

    /// Ведёт ли сейчас цель (для отладки и проверок): что делает моб.
    pub doing: Doing,
}

/// Чем занят моб — для проверок.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Doing {
    #[default]
    Idle,
    Panic,
    Breed,
    Tempt,
    Follow,
    Eat,
    Stroll,
}

/// Куда и как долго смотрит голова.
#[derive(Clone, Copy, Debug)]
struct Look {
    yaw: f32,
    pitch: f32,
    ticks: i32,
    at_player: bool,
}

impl Brain {
    pub fn new(_seed: i32) -> Self {
        Self {
            path: Vec::new(),
            speed: 1.0,
            panic: 0,
            look: None,
            eating: 0,
            breed: 0,
            stuck: 0,
            last: (0.0, 0.0),
            repath: 0,
            ambient: 0,
            doing: Doing::Idle,
        }
    }

    /// Паника на столько тактов; путь прошлой цели бросается.
    pub fn panic(&mut self, ticks: i32) {
        self.panic = ticks;
        self.path.clear();
        self.eating = 0;
    }

    #[allow(dead_code)] // для проверок
    pub fn is_panicking(&self) -> bool {
        self.panic > 0
    }

    /// Идёт ли моб по пути.
    #[allow(dead_code)] // для проверок
    pub fn walking(&self) -> bool {
        !self.path.is_empty()
    }

    /// Бросить путь.
    pub fn stop(&mut self) {
        self.path.clear();
    }

    /// Дать путь напрямую (проверки и приманка).
    pub fn go(&mut self, path: Vec<Cell>, speed: f64) {
        self.path = path;
        self.speed = speed;
        self.stuck = 0;
    }
}

/// Скорость паники: у курицы 1,4, у остальных 2 (вики, Mob AI, «Panic»).
fn panic_speed(kind: Kind) -> f64 {
    if kind == Kind::Chicken { 1.4 } else { 2.0 }
}

/// Скорость к приманке (вики, Breeding/«Tempt»): корова 1,25, свинья 1,2,
/// овца 1,1, курица 1.
fn tempt_speed(kind: Kind) -> f64 {
    match kind {
        Kind::Cow => 1.25,
        Kind::Pig => 1.2,
        Kind::Sheep => 1.1,
        Kind::Chicken => 1.0,
        _ => 1.0,
    }
}

/// Держит ли игрок то, что животное любит.
fn tempting(kind: Kind, watcher: &Watcher) -> bool {
    watcher.holding.iter().flatten().any(|item| kind.likes(item))
}

/// Мысль за такт. Меняет моба (поворот, счётчики) и говорит, как ему
/// двигаться и что случилось вовне.
pub fn think(mob: &mut Mob, view: &dyn View, ctx: &Ctx, rng: &mut Rng) -> Plan {
    if mob.kind.hostile() {
        return super::hostile::think(mob, view, ctx, rng);
    }

    let mut plan = Plan::default();

    voice(mob, rng);

    // Житель торгует: стоит (вики, Trading: «pauses their pathfinding»). К
    // покупателю он не поворачивается: у оригинала за всю торговлю ни одного
    // поворота головы (чёрный ящик, villager-levelup-ai) [проверить: «makes
    // them face the trading player»].
    if mob.kind == Kind::Villager && mob.villager.trading.is_some() {
        mob.brain.stop();
        return plan;
    }

    // Курица несёт яйцо раз в 6000–12000 тактов (вики, Chicken: «Egg»).
    if mob.kind == Kind::Chicken && !mob.is_baby() {
        mob.egg_timer -= 1;

        if mob.egg_timer == 0 {
            plan.lay_egg = true;
        }

        if mob.egg_timer <= 0 {
            mob.egg_timer = 6000 + rng.below(6001) as i32;
        }
    }

    let here = mob.cell();

    // Овца жуёт: стоит на месте, голова вниз (вики, Sheep: «eat grass» —
    // 40 тактов, трава пропадает на последних).
    if mob.brain.eating > 0 {
        mob.brain.eating -= 1;
        mob.brain.doing = Doing::Eat;
        mob.pitch = 25.0;

        if mob.brain.eating == 4 {
            let below = (here.0, here.1 - 1, here.2);
            let grass = crate::blocks::block_at_state(view.block(here.0, here.1, here.2)) == Some("short_grass");
            let dirt = crate::blocks::block_at_state(view.block(below.0, below.1, below.2)) == Some("grass_block");

            if grass {
                plan.eat = Some(Eat { at: here, block: false });
            } else if dirt {
                plan.eat = Some(Eat { at: below, block: true });
            }
        }

        if mob.brain.eating == 0 {
            mob.pitch = 0.0;
        }

        return plan;
    }

    mob.brain.doing = Doing::Idle;

    // В воздухе моб не ходит: путь ведётся только с земли и из воды (вики,
    // Pathfinding; чёрный ящик, chicken-fall: падающая курица не идёт).
    let in_water = super::is_water(view.block(here.0, here.1, here.2));

    if !mob.on_ground && !in_water {
        // Начатый путь держится и в прыжке (руль работает, разгон в воздухе
        // слабый); новый в воздухе не строится.
        if !mob.brain.path.is_empty() {
            return walk(mob, plan);
        }

        look_around(mob, ctx, rng);
        return plan;
    }

    // 1. Паника.
    if mob.brain.panic > 0 {
        mob.brain.panic -= 1;
        mob.brain.doing = Doing::Panic;

        if mob.brain.path.is_empty() {
            let (ax, az) = mob.fright.unwrap_or((mob.x, mob.z));
            let (dx, dz) = (mob.x - ax, mob.z - az);
            let len = (dx * dx + dz * dz).sqrt().max(1.0e-6);
            let (ux, uz) = if len > 1.0e-3 && (dx.abs() > 1.0e-6 || dz.abs() > 1.0e-6) {
                (dx / len, dz / len)
            } else {
                let angle = rng.unit() * std::f64::consts::TAU;
                (angle.cos(), angle.sin())
            };
            let jitter = (rng.unit() - 0.5) * 4.0;
            let target = (here.0 + (ux * 6.0 + jitter).round() as i32, here.1, here.2 + (uz * 6.0 - jitter).round() as i32);
            let target = settle(view, target, here.1);

            let route = path::find(view, here, target, path::MAX_NODES);

            mob.brain.go(route, panic_speed(mob.kind));
        }

        return walk(mob, plan);
    }

    // 2. Разведение: влюблённый идёт к влюблённому сородичу.
    if mob.love > 0 && mob.age == 0 {
        let mate = ctx
            .mates
            .iter()
            .filter(|other| (other.kind == mob.kind || super::land::llama(other.kind) && super::land::llama(mob.kind)) && other.love > 0 && other.ready && other.id != mob.entity_id)
            .map(|other| (other, (other.x - mob.x).powi(2) + (other.y - mob.y).powi(2) + (other.z - mob.z).powi(2)))
            .filter(|(_, dist)| *dist <= 64.0)
            .min_by(|a, b| a.1.total_cmp(&b.1));

        if let Some((other, dist)) = mate {
            mob.brain.doing = Doing::Breed;
            look_at_point(mob, other.x, other.y + 0.5, other.z);

            if dist < 9.0 {
                mob.brain.breed += 1;
                mob.brain.stop();

                if mob.brain.breed >= BREED_TICKS {
                    mob.brain.breed = 0;
                    plan.breed = Some(other.id);
                }

                return plan;
            }

            mob.brain.breed = 0;
            mob.brain.repath -= 1;

            if mob.brain.path.is_empty() || mob.brain.repath <= 0 {
                let target = (other.x.floor() as i32, other.y.floor() as i32, other.z.floor() as i32);

                mob.brain.repath = 10;
                let route = path::find(view, here, target, path::MAX_NODES);

                mob.brain.go(route, 1.0);
            }

            return walk(mob, plan);
        }

        mob.brain.breed = 0;
    }

    // 3. Приманка: игрок с любимой едой в 10 блоках (взрослые и детёныши).
    let lured = ctx
        .players
        .iter()
        .filter(|player| !player.spectator && !super::land::llama(mob.kind) && tempting(mob.kind, player))
        .map(|player| (player, (player.x - mob.x).powi(2) + (player.y - mob.y).powi(2) + (player.z - mob.z).powi(2)))
        .filter(|(_, dist)| *dist <= 100.0)
        .min_by(|a, b| a.1.total_cmp(&b.1));

    if let Some((player, dist)) = lured {
        mob.brain.doing = Doing::Tempt;
        look_at_point(mob, player.x, player.y + player.eye, player.z);

        if dist < 6.25 {
            // Ближе 2,5 блока: остановиться (вики, Breeding: «follow»).
            mob.brain.stop();
            return plan;
        }

        mob.brain.repath -= 1;

        if mob.brain.path.is_empty() || mob.brain.repath <= 0 {
            mob.brain.repath = 10;
            let target = (player.x.floor() as i32, player.y.floor() as i32, player.z.floor() as i32);
            let route = path::find(view, here, target, path::MAX_NODES);

            mob.brain.go(route, tempt_speed(mob.kind));
        }

        return walk(mob, plan);
    }

    // 4. Детёныш идёт за взрослым того же вида.
    if mob.is_baby() {
        let parent = ctx
            .mates
            .iter()
            .filter(|other| other.kind == mob.kind && !other.baby)
            .map(|other| (other, (other.x - mob.x).powi(2) + (other.y - mob.y).powi(2) + (other.z - mob.z).powi(2)))
            .filter(|(_, dist)| *dist <= 64.0 && *dist >= 9.0)
            .min_by(|a, b| a.1.total_cmp(&b.1));

        if let Some((other, _)) = parent {
            mob.brain.doing = Doing::Follow;
            mob.brain.repath -= 1;

            if mob.brain.path.is_empty() || mob.brain.repath <= 0 {
                mob.brain.repath = 10;
                let target = (other.x.floor() as i32, other.y.floor() as i32, other.z.floor() as i32);
                let route = path::find(view, here, target, path::MAX_NODES);

                mob.brain.go(route, 1.25);
            }

            return walk(mob, plan);
        }
    }

    // 5. Овца щиплет траву: 1/1000 за такт, у детёныша 1/50.
    if mob.kind == Kind::Sheep && mob.brain.path.is_empty() {
        let chance = if mob.is_baby() { 50 } else { 1000 };

        if rng.below(chance) == 0 {
            let grass = crate::blocks::block_at_state(view.block(here.0, here.1, here.2)) == Some("short_grass");
            let dirt = crate::blocks::block_at_state(view.block(here.0, here.1 - 1, here.2)) == Some("grass_block");

            if grass || dirt {
                mob.brain.eating = 40;
                mob.brain.doing = Doing::Eat;
                mob.events.push(super::Event::Eating);
                return plan;
            }
        }
    }

    // 6. Прогулка: в среднем раз в 120 тактов (вики, Mob AI: «Wander»).
    if mob.brain.path.is_empty() && rng.below(120) == 0 {
        for _ in 0..10 {
            let dx = rng.below(21) as i32 - 10;
            let dz = rng.below(21) as i32 - 10;
            let dy = rng.below(15) as i32 - 7;
            let target = settle(view, (here.0 + dx, here.1 + dy, here.2 + dz), here.1 + dy);

            if !path::standable(view, target) {
                continue;
            }

            let route = path::find(view, here, target, path::MAX_NODES);

            if !route.is_empty() {
                mob.brain.go(route, 1.0);
                break;
            }
        }
    }

    if !mob.brain.path.is_empty() {
        mob.brain.doing = Doing::Stroll;
        return walk(mob, plan);
    }

    // Голова: на ближайшего игрока или случайно.
    look_around(mob, ctx, rng);
    plan
}

/// Опускает точку на землю: ближайшая клетка под ней, где можно стоять.
pub(super) fn settle(view: &dyn View, (x, y, z): Cell, start: i32) -> Cell {
    let mut at = (x, y.max(start - 7).min(start + 7), z);

    for _ in 0..16 {
        if path::standable(view, at) {
            return at;
        }

        at.1 -= 1;
    }

    (x, start, z)
}

/// Повернуть голову к точке.
pub(super) fn look_at_point(mob: &mut Mob, x: f64, y: f64, z: f64) {
    let (dx, dy, dz) = (x - mob.x, y - (mob.y + mob.eye()), z - mob.z);
    let yaw = (-dx.atan2(dz)).to_degrees() as f32;
    let flat = (dx * dx + dz * dz).sqrt();
    let pitch = (-dy.atan2(flat)).to_degrees() as f32;

    mob.head_yaw = turn(mob.head_yaw, yaw, 10.0);
    mob.pitch = pitch.clamp(-40.0, 40.0);
}

/// Поворот к цели не больше чем на `max` градусов за такт.
pub(super) fn turn(from: f32, to: f32, max: f32) -> f32 {
    let mut diff = (to - from) % 360.0;

    if diff > 180.0 {
        diff -= 360.0;
    }

    if diff < -180.0 {
        diff += 360.0;
    }

    from + diff.clamp(-max, max)
}

/// Взгляд на игрока (вики, Mob AI, «Look at player»: шанс 0,02, 6 блоков,
/// 40–80 тактов) и случайный взгляд (шанс 0,02, 20–40 тактов).
fn look_around(mob: &mut Mob, ctx: &Ctx, rng: &mut Rng) {
    // Житель по сторонам головой не водит: в клетке 1×1 за 400 тактов у
    // оригинала ни одного поворота головы (чёрный ящик, villager-claim) [проверить:
    // взгляд на игрока у колокола и на работе].
    if mob.kind == Kind::Villager {
        return;
    }

    if let Some(look) = &mut mob.brain.look {
        look.ticks -= 1;

        if look.ticks <= 0 {
            mob.brain.look = None;
        } else {
            let (yaw, pitch, at_player) = (look.yaw, look.pitch, look.at_player);

            if at_player {
                // Игрок мог сместиться: держим направление на него.
                if let Some(player) = ctx
                    .players
                    .iter()
                    .filter(|player| !player.spectator)
                    .min_by(|a, b| ((a.x - mob.x).powi(2) + (a.z - mob.z).powi(2)).total_cmp(&((b.x - mob.x).powi(2) + (b.z - mob.z).powi(2))))
                {
                    look_at_point(mob, player.x, player.y + player.eye, player.z);
                }
            } else {
                mob.head_yaw = turn(mob.head_yaw, yaw, 10.0);
                mob.pitch = pitch;
            }

            return;
        }
    }

    if rng.below(50) == 0 {
        let near = ctx
            .players
            .iter()
            .filter(|player| !player.spectator)
            .map(|player| (player, (player.x - mob.x).powi(2) + (player.y - mob.y).powi(2) + (player.z - mob.z).powi(2)))
            .filter(|(_, dist)| *dist <= 36.0)
            .min_by(|a, b| a.1.total_cmp(&b.1));

        if let Some((player, _)) = near {
            mob.brain.look = Some(Look { yaw: 0.0, pitch: 0.0, ticks: 40 + rng.below(40) as i32, at_player: true });
            look_at_point(mob, player.x, player.y + player.eye, player.z);
            return;
        }
    }

    if rng.below(50) == 0 {
        let angle = rng.unit() * std::f64::consts::TAU;

        mob.brain.look = Some(Look { yaw: (-angle.cos().atan2(angle.sin())).to_degrees() as f32, pitch: 0.0, ticks: 20 + rng.below(20) as i32, at_player: false });
    }
}

/// Один шаг по пути: повернуться к следующей клетке и идти. Пройденные
/// клетки снимаются; застрявший бросает путь.
pub(super) fn walk(mob: &mut Mob, mut plan: Plan) -> Plan {
    let Some(&(nx, ny, nz)) = mob.brain.path.first() else {
        return plan;
    };

    let (tx, tz) = (f64::from(nx) + 0.5, f64::from(nz) + 0.5);
    let (dx, dz) = (tx - mob.x, tz - mob.z);
    let flat = dx * dx + dz * dz;

    // Дошли до клетки: по горизонтали в пределах ширины, по вертикали
    // не выше ступеньки (вики, Pathfinding: узел достигнут, когда моб в нём).
    if flat < 0.25 * 0.25 * 4.0 && (mob.y - f64::from(ny)).abs() < 1.0 {
        mob.brain.path.remove(0);
        mob.brain.stuck = 0;

        if mob.brain.path.is_empty() {
            return plan;
        }

        return walk(mob, plan);
    }

    let want = (-dx.atan2(dz)).to_degrees() as f32;

    // Тело поворачивается к ходу не быстрее 90° за такт (вики, Mob AI).
    mob.yaw = turn(mob.yaw, want, 90.0);
    mob.head_yaw = turn(mob.head_yaw, mob.yaw, 10.0).max(-1.0e9);
    mob.head_yaw = mob.yaw;

    plan.speed = mob.brain.speed;

    // Клетка выше ног — прыжок (вики, Jumping: моб прыгает на блок выше).
    // Прыгает, пока до клетки меньше блока: с ходу, а не упёршись в стену
    // (вики, Jumping; так делает и оригинал).
    plan.jump = f64::from(ny) > mob.y + 0.1 && mob.on_ground && flat < 1.0;

    // Застрял: за 40 тактов не сдвинулся — путь брошен.
    let moved = (mob.x - mob.brain.last.0).powi(2) + (mob.z - mob.brain.last.1).powi(2);

    // Лезущий по стене паук стоит на месте по горизонтали, но не застрял.
    if moved < 1.0e-4 && !mob.h.climbing {
        mob.brain.stuck += 1;
    } else {
        mob.brain.stuck = 0;
    }

    mob.brain.last = (mob.x, mob.z);

    if mob.brain.stuck > 40 {
        mob.brain.path.clear();
        mob.brain.stuck = 0;
        plan.speed = 0.0;
    }

    plan
}

/// Голос: ambient-звук (вики, Sounds): шанс растёт с каждым тактом тишины.
/// Громкость — по виду (у дракона 5; чёрный ящик, dragon-summon).
/// Звук жителя «сделка обновилась» — тем же счётом, что голос в покое: не
/// чаще, чем через `TRADE_SOUND_GAP` тактов после прошлого (чёрный ящик,
/// villager-trade, villager-levelup). Можно — счёт сбрасывается.
pub fn trade_sound_due(mob: &mut Mob) -> bool {
    if mob.brain.ambient > -80 + crate::villager::TRADE_SOUND_GAP {
        mob.brain.ambient = -80;
        true
    } else {
        false
    }
}

/// Сделка совершилась: счёт голоса сбрасывается молча, как после голоса
/// (чёрный ящик, villager-trade: после Shift-сделок «да» на следующем щелчке
/// через 20 тактов не звучит).
pub fn hush(mob: &mut Mob) {
    mob.brain.ambient = -80;
}

pub fn voice(mob: &mut Mob, rng: &mut Rng) {
    mob.brain.ambient += 1;

    if rng.below(1000) < mob.brain.ambient.max(0) as u32 {
        let (ambient, _, _) = mob.sounds();
        // Торгующий житель вместо голоса в покое — «торгует» (чёрный ящик,
        // villager-trade-ai: entity.villager.trade при открытом окне).
        let ambient = if mob.villager.trading.is_some() { match mob.kind { Kind::Villager => "minecraft:entity.villager.trade", Kind::WanderingTrader => "minecraft:entity.wandering_trader.trade", _ => ambient } } else { ambient };

        if ambient.is_empty() {
            return;
        }

        mob.brain.ambient = -80;
        mob.events.push(super::Event::Sound { name: ambient, volume: mob.sound_volume(), pitch: mob.voice_pitch() });
    }
}
