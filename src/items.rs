// Предметы, лежащие в мире.
//
// Выброшенный предмет — это отдельная сущность: она видна всем, лежит на земле
// и поднимается тем, кто до неё дошёл. Устроено так же, как список игроков:
// склад общий, а каждое подключение помнит, сколько записей журнала оно уже
// разослало своему клиенту.
//
// Летит предмет по-настоящему: у него есть скорость, его тянет вниз, воздух
// его тормозит, а о блоки он останавливается. Числа взяты с minecraft.wiki
// (страница Entity, таблица тяжести и сопротивления): тяжесть −0.04 блока за
// такт, сопротивление 0.98, и порядок действий в такте именно такой —
// сперва ускорение, потом движение, потом сопротивление.
//
// Между запусками сервера лежащие предметы не сохраняются: мир хранит блоки,
// а не сущности. Поднимать их надо в ту же игру.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::inventory::Stack;
use crate::journal::{Journal, Reader};

/// Насколько близко надо подойти, чтобы предмет поднялся: примерно шаг
/// по горизонтали и полтора блока по высоте — чтобы поднять и то, что лежит
/// под ногами, и то, что на голову выше.
const PICK_UP_RANGE: f64 = 1.4;
const PICK_UP_HEIGHT: f64 = 1.5;

/// Сколько предмет нельзя поднимать после того, как игрок его выбросил:
/// две секунды, как в игре.
///
/// Иначе выброшенное тут же вернулось бы обратно: игрок стоит рядом с тем,
/// что бросил.
pub const THROWN_DELAY: Duration = Duration::from_secs(2);

/// Сколько нельзя поднимать то, что выпало из сломанного блока: полсекунды,
/// как в игре.
pub const MINED_DELAY: Duration = Duration::from_millis(500);

/// Насколько предмет тянет вниз за такт.
const GRAVITY: f64 = 0.04;

/// Сопротивление воздуха: на столько скорость умножается каждый такт.
const DRAG: f64 = 0.98;

/// Трение о землю: лежащий предмет останавливается быстро, а не скользит.
///
/// Число общее для трения о блоки; на вики оно указано для существ, стоящих
/// на земле, и здесь взято то же.
const GROUND_FRICTION: f64 = 0.6;

/// Скорость меньше этой считается нулевой: иначе предмет вечно ползал бы на
/// тысячные доли блока, а клиентам летели бы пакеты о его движении.
const STILL: f64 = 0.003;

/// С какой скоростью вылетает выброшенный предмет: вперёд и немного вверх.
///
/// Точных чисел на вики нет, поэтому взяты такие: бросок получается
/// на пару блоков вперёд, как в игре.
pub const THROW_SPEED: f64 = 0.3;
pub const THROW_LIFT: f64 = 0.12;

/// Ширина предмета: по ней он упирается в стены.
const SIZE: f64 = 0.25;

/// Насколько предмет сдвигается за один шаг проверки.
///
/// Шаг должен быть заметно меньше блока: иначе быстрое движение перескочило бы
/// через блок, и предмет прошёл бы сквозь пол.
const MAX_STEP: f64 = 0.2;

/// Сколько предмет лежит, прежде чем пропасть: пять минут, как в игре.
///
/// Без этого брошенное копилось бы в памяти сервера до самой его остановки.
const LIFETIME: Duration = Duration::from_secs(300);

/// Насколько ниже мира предмет считается пропавшим.
///
/// Упавший в пустоту предмет никуда не денется сам, и помнить его вечно
/// незачем.
const LOST_BELOW: f64 = 32.0;

/// Предмет, лежащий в мире.
#[derive(Clone, Debug)]
pub struct Dropped {
    /// Номер сущности: по нему клиенты отличают один предмет от другого.
    pub entity_id: i32,

    /// Опознаватель сущности — нужен клиенту при её появлении.
    pub uuid: [u8; 16],

    /// Что это за предмет и сколько его.
    pub stack: Stack,

    /// Если это падающий блок — какой именно. Обычный лежащий предмет
    /// здесь ничего не хранит.
    ///
    /// Падающий блок ведёт себя так же, как предмет: летит, тормозится
    /// воздухом, упирается в землю. Разница в двух вещах — его не поднять,
    /// и, долетев, он снова становится блоком.
    pub block: Option<i32>,

    pub x: f64,
    pub y: f64,
    pub z: f64,

    /// Куда и как быстро он летит: блоков за такт.
    pub vx: f64,
    pub vy: f64,
    pub vz: f64,

    /// Лежит ли он на чём-то. Лежащий тормозится трением, летящий — воздухом.
    pub on_ground: bool,

    /// С какого времени предмет можно поднять.
    ready_at: Instant,

    /// Когда он появился: полежав достаточно долго, предмет пропадает.
    born: Instant,
}

/// Что случилось с лежащими предметами — запись журнала.
#[derive(Clone, Debug)]
pub enum Change {
    /// Предмет появился в мире.
    Dropped(Dropped),

    /// Предмет пропал сам: провалился в пустоту.
    Gone { entity_id: i32 },

    /// Предмет подняли.
    Taken {
        entity_id: i32,

        /// Кто поднял: клиент рисует, как предмет летит к этой сущности.
        by: i32,

        /// Сколько предметов в поднятой стопке — это тоже показывается.
        count: i32,
    },
}

/// Все предметы, лежащие в мире, и журнал изменений.
pub struct Items {
    items: Vec<Dropped>,
    changes: Journal<Change>,
}

impl Items {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            changes: Journal::new(),
        }
    }

    /// Кладёт предмет в мир.
    pub fn drop_item(
        &mut self,
        entity_id: i32,
        stack: Stack,
        (x, y, z): (f64, f64, f64),
        velocity: (f64, f64, f64),
        delay: Duration,
    ) {
        let dropped = Dropped {
            entity_id,
            uuid: uuid_for(entity_id),
            stack,
            block: None,
            x,
            y,
            z,
            vx: velocity.0,
            vy: velocity.1,
            vz: velocity.2,
            on_ground: false,
            ready_at: Instant::now() + delay,
            born: Instant::now(),
        };

        self.items.push(dropped.clone());
        self.changes.push(Change::Dropped(dropped));
    }

    /// Пускает блок в свободное падение.
    ///
    /// Предмет при нём — то, что выпадет, если приземлиться будет некуда.
    pub fn drop_block(
        &mut self,
        entity_id: i32,
        state: i32,
        stack: Stack,
        (x, y, z): (f64, f64, f64),
    ) {
        let falling = Dropped {
            entity_id,
            uuid: uuid_for(entity_id),
            stack,
            block: Some(state),
            x,
            y,
            z,
            vx: 0.0,
            vy: 0.0,
            vz: 0.0,
            on_ground: false,
            ready_at: Instant::now(),
            born: Instant::now(),
        };

        self.items.push(falling.clone());
        self.changes.push(Change::Dropped(falling));
    }

    /// Забирает падающие блоки, которые уже приземлились.
    ///
    /// Вызывающий ставит их обратно в мир — или роняет предметом, если
    /// место занято.
    pub fn take_landed(&mut self) -> Vec<Dropped> {
        let (landed, left): (Vec<Dropped>, Vec<Dropped>) = self
            .items
            .drain(..)
            .partition(|item| item.block.is_some() && item.on_ground);

        self.items = left;

        for item in &landed {
            self.changes.push(Change::Gone {
                entity_id: item.entity_id,
            });
        }

        landed
    }

    /// Всё, что сейчас лежит в мире.
    pub fn items(&self) -> &[Dropped] {
        &self.items
    }

    /// Двигает все предметы на один такт.
    ///
    /// `solid` отвечает, можно ли на этом блоке стоять и упираться в него.
    pub fn step<F>(&mut self, solid: &F)
    where
        F: Fn(i32, i32, i32) -> bool,
    {
        for item in &mut self.items {
            item.step(solid);
        }

        // Пропадают провалившиеся в пустоту и пролежавшие слишком долго.
        let bottom = crate::world::MIN_Y as f64 - LOST_BELOW;
        let now = Instant::now();

        let gone: Vec<i32> = self
            .items
            .iter()
            .filter(|item| item.y < bottom || now.duration_since(item.born) >= LIFETIME)
            .map(|item| item.entity_id)
            .collect();

        for entity_id in &gone {
            self.changes.push(Change::Gone {
                entity_id: *entity_id,
            });
        }

        self.items
            .retain(|item| !gone.contains(&item.entity_id));
    }

    /// Забирает предметы, до которых дотянулся игрок.
    ///
    /// Возвращает поднятое, чтобы вызывающий разложил его по инвентарю.
    /// Слишком свежее — то, что только что выброшено, — остаётся лежать.
    pub fn take_near(&mut self, (x, y, z): (f64, f64, f64), by: i32) -> Vec<Dropped> {
        let now = Instant::now();

        let (taken, left): (Vec<Dropped>, Vec<Dropped>) = self
            .items
            .drain(..)
            .partition(|item| {
                item.block.is_none() && item.ready_at <= now && item.reaches(x, y, z)
            });

        self.items = left;

        for item in &taken {
            self.changes.push(Change::Taken {
                entity_id: item.entity_id,
                by,
                count: item.stack.count,
            });
        }

        taken
    }

    /// Изменения, начиная с записи `from`.
    pub fn changes_since(&mut self, from: usize, reader: Reader) -> (Vec<Change>, usize) {
        (self.changes.since(from, reader), self.changes.count())
    }

    /// Записывает читателя журнала.
    pub fn watch_changes(&mut self, reader: Reader) -> usize {
        self.changes.watch(reader)
    }

    /// Отписывает читателя: подключение закрылось.
    pub fn forget_reader(&mut self, reader: Reader) {
        self.changes.forget(reader);
    }
}

impl Dropped {
    /// Дотягивается ли игрок, стоящий здесь, до этого предмета.
    fn reaches(&self, x: f64, y: f64, z: f64) -> bool {
        let flat = ((self.x - x).powi(2) + (self.z - z).powi(2)).sqrt();

        flat <= PICK_UP_RANGE && (self.y - y).abs() <= PICK_UP_HEIGHT
    }

    /// Двигает предмет на один такт.
    ///
    /// Порядок в точности такой, как в игре: сперва к скорости добавляется
    /// тяжесть, потом предмет двигается, и лишь потом скорость уменьшается
    /// сопротивлением. От порядка зависит вид дуги, поэтому он важен.
    fn step<F>(&mut self, solid: &F)
    where
        F: Fn(i32, i32, i32) -> bool,
    {
        if self.resting() {
            return;
        }

        self.vy -= GRAVITY;

        self.move_across(solid);

        let sideways = if self.on_ground {
            DRAG * GROUND_FRICTION
        } else {
            DRAG
        };

        self.vx *= sideways;
        self.vy *= DRAG;
        self.vz *= sideways;

        // Совсем медленное движение прекращаем: предмет лежит.
        if self.vx.abs() < STILL {
            self.vx = 0.0;
        }
        if self.vz.abs() < STILL {
            self.vz = 0.0;
        }
        if self.vy.abs() < STILL && self.on_ground {
            self.vy = 0.0;
        }
    }

    /// Лежит ли предмет совсем неподвижно — такой такт можно и пропустить.
    fn resting(&self) -> bool {
        self.on_ground && self.vx == 0.0 && self.vy == 0.0 && self.vz == 0.0
    }

    /// Переносит предмет, упираясь в блоки.
    ///
    /// Движение дробится на короткие шаги, каждый меньше блока. Иначе
    /// быстро падающий предмет за один такт перескакивал бы через целый блок
    /// и проваливался сквозь пол: проверка смотрит, что в конце шага, а не
    /// что было по пути.
    ///
    /// Оси проверяются по отдельности, чтобы предмет, задевший стену, скользил
    /// вдоль неё, а не останавливался целиком.
    fn move_across<F>(&mut self, solid: &F)
    where
        F: Fn(i32, i32, i32) -> bool,
    {
        let longest = self.vx.abs().max(self.vy.abs()).max(self.vz.abs());
        let steps = (longest / MAX_STEP).ceil().max(1.0);

        let (step_x, step_y, step_z) = (
            self.vx / steps,
            self.vy / steps,
            self.vz / steps,
        );

        self.on_ground = false;

        for _ in 0..steps as i32 {
            self.step_sideways(step_x, 0.0, solid);
            self.step_sideways(0.0, step_z, solid);
            self.step_down_or_up(step_y, solid);
        }
    }

    /// Шаг вбок: упёрлись — остановились по этой оси.
    fn step_sideways<F>(&mut self, step_x: f64, step_z: f64, solid: &F)
    where
        F: Fn(i32, i32, i32) -> bool,
    {
        if step_x == 0.0 && step_z == 0.0 {
            return;
        }

        // Смотрим туда, куда придётся край предмета, а не его середина.
        let ahead_x = self.x + step_x + step_x.signum() * SIZE;
        let ahead_z = self.z + step_z + step_z.signum() * SIZE;

        let blocked = solid(
            ahead_x.floor() as i32,
            self.y.floor() as i32,
            ahead_z.floor() as i32,
        );

        if blocked {
            if step_x != 0.0 {
                self.vx = 0.0;
            } else {
                self.vz = 0.0;
            }

            return;
        }

        self.x += step_x;
        self.z += step_z;
    }

    /// Шаг вверх или вниз: снизу ложимся на блок, сверху упираемся в потолок.
    fn step_down_or_up<F>(&mut self, step_y: f64, solid: &F)
    where
        F: Fn(i32, i32, i32) -> bool,
    {
        let column = (self.x.floor() as i32, self.z.floor() as i32);
        let next_y = self.y + step_y;

        if step_y < 0.0 && solid(column.0, next_y.floor() as i32, column.1) {
            // Легли на верх блока.
            self.y = next_y.floor() + 1.0;
            self.vy = 0.0;
            self.on_ground = true;
            return;
        }

        if step_y > 0.0 && solid(column.0, (next_y + SIZE).floor() as i32, column.1) {
            self.vy = 0.0;
            return;
        }

        self.y = next_y;
    }
}

/// Опознаватель для сущности предмета.
///
/// Он должен быть разным у разных предметов и больше нигде не значит ничего:
/// клиент по нему только отличает сущности друг от друга. Поэтому составляем
/// его из номера сущности и времени.
fn uuid_for(entity_id: i32) -> [u8; 16] {
    let moment = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|passed| passed.as_nanos() as u64)
        .unwrap_or(0);

    let mut uuid = [0u8; 16];

    uuid[..8].copy_from_slice(&moment.to_be_bytes());
    uuid[8..12].copy_from_slice(&entity_id.to_be_bytes());
    uuid[12..].copy_from_slice(b"item");

    uuid
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Читатель журнала в проверках: настоящий номер выдаёт сервер.
    const READER: crate::journal::Reader = 1;

    fn stack() -> Stack {
        Stack::new(1, 3)
    }

    /// Пол на высоте 63: всё, что ниже 64, — твёрдое.
    fn floor(_x: i32, y: i32, _z: i32) -> bool {
        y <= 63
    }

    /// Предмет, брошенный отсюда с этой скоростью.
    fn thrown(at: (f64, f64, f64), velocity: (f64, f64, f64)) -> Items {
        let mut items = Items::new();

        // Журнал держит записи только для записанных читателей.
        items.watch_changes(READER);

        items.drop_item(1, stack(), at, velocity, Duration::ZERO);
        items
    }

    /// Выброшенный предмет появляется в мире и попадает в журнал.
    #[test]
    fn a_dropped_item_appears_in_the_world() {
        let mut items = Items::new();
        items.watch_changes(READER);

        items.drop_item(7, stack(), (0.5, 64.0, 0.5), (0.0, 0.0, 0.0), Duration::ZERO);

        assert_eq!(items.items().len(), 1);
        assert_eq!(items.items()[0].entity_id, 7);
        // Запись о появлении попала в журнал.
        let (changes, _) = items.changes_since(0, READER);

        assert!(matches!(changes.first(), Some(Change::Dropped(_))));
    }

    /// Поднимается то, до чего игрок дотянулся, — остальное остаётся лежать.
    #[test]
    fn only_what_is_within_reach_is_picked_up() {
        let mut items = Items::new();
        items.watch_changes(READER);

        let still = (0.0, 0.0, 0.0);

        items.drop_item(1, stack(), (0.5, 64.0, 0.5), still, Duration::ZERO);
        items.drop_item(2, stack(), (10.0, 64.0, 0.5), still, Duration::ZERO);
        items.drop_item(3, stack(), (0.5, 70.0, 0.5), still, Duration::ZERO);

        let taken = items.take_near((0.0, 64.0, 0.0), 42);

        assert_eq!(taken.len(), 1);
        assert_eq!(taken[0].entity_id, 1);
        assert_eq!(items.items().len(), 2);

        // В журнале записано, кто поднял и сколько.
        let (changes, _) = items.changes_since(3, READER);

        assert!(matches!(
            changes.first(),
            Some(Change::Taken { entity_id: 1, by: 42, count: 3 })
        ));
    }

    /// Только что выброшенное сразу не поднимается: иначе оно вернулось бы
    /// в руки тому, кто его бросил.
    #[test]
    fn a_fresh_drop_stays_on_the_ground() {
        let mut items = Items::new();

        items.drop_item(1, stack(), (0.0, 64.0, 0.0), (0.0, 0.0, 0.0), THROWN_DELAY);

        assert!(items.take_near((0.0, 64.0, 0.0), 1).is_empty());
        assert_eq!(items.items().len(), 1);
    }

    /// Брошенный предмет летит по дуге: сперва вверх и вперёд, потом вниз,
    /// и в конце ложится на пол впереди.
    #[test]
    fn a_thrown_item_flies_in_an_arc() {
        let mut items = thrown((0.0, 66.0, 0.0), (THROW_SPEED, THROW_LIFT, 0.0));

        // Записываем высоту такт за тактом: по ней и видно дугу.
        let mut heights = Vec::new();

        for _ in 0..30 {
            items.step(&floor);
            heights.push(items.items()[0].y);
        }

        let top = heights
            .iter()
            .cloned()
            .fold(f64::MIN, f64::max);

        // Поднялся выше места броска, а потом опустился ниже него.
        assert!(top > 66.0, "не поднялся: высшая точка {}", top);
        assert!(heights.last().copied().unwrap() < 66.0, "не опустился");

        // И в конце лежит на полу, улетев вперёд.
        for _ in 0..200 {
            items.step(&floor);
        }

        let landed = &items.items()[0];

        assert!(landed.on_ground);
        assert_eq!(landed.y, 64.0);
        assert!(landed.x > 0.5, "улетел всего на {} блока", landed.x);
        assert_eq!(landed.vx, 0.0, "лежащий предмет не ползёт");
    }

    /// Выпавший из блока предмет просто падает вниз, никуда не улетая.
    #[test]
    fn a_mined_item_just_falls() {
        let mut items = thrown((0.5, 70.0, 0.5), (0.0, 0.0, 0.0));

        for _ in 0..200 {
            items.step(&floor);
        }

        let landed = &items.items()[0];

        assert_eq!((landed.x, landed.z), (0.5, 0.5));
        assert_eq!(landed.y, 64.0);
        assert!(landed.on_ground);
    }

    /// Предмет не проходит сквозь стену: он упирается в неё и падает рядом.
    #[test]
    fn an_item_stops_at_a_wall() {
        // Стена на востоке: всё, что дальше x = 1, твёрдое.
        let wall = |x: i32, y: i32, _z: i32| y <= 63 || x >= 2;

        let mut items = thrown((0.5, 65.0, 0.5), (0.5, 0.0, 0.0));

        for _ in 0..100 {
            items.step(&wall);
        }

        let stopped = &items.items()[0];

        assert!(stopped.x < 2.0, "прошёл сквозь стену: {}", stopped.x);
        assert_eq!(stopped.y, 64.0);
    }

    /// У разных предметов разные опознаватели: иначе клиент принял бы их за
    /// одну и ту же сущность.
    #[test]
    fn dropped_items_differ_by_uuid() {
        assert_ne!(uuid_for(1), uuid_for(2));
    }

    /// Быстро падающий предмет не проваливается сквозь пол: движение
    /// проверяется короткими шагами.
    #[test]
    fn a_fast_item_does_not_fall_through_the_floor() {
        let mut items = thrown((0.5, 200.0, 0.5), (0.0, 0.0, 0.0));

        for _ in 0..500 {
            items.step(&floor);
        }

        let landed = &items.items()[0];

        assert!(landed.on_ground, "провалился: высота {}", landed.y);
        assert_eq!(landed.y, 64.0);
    }

    /// Предмет не пролетает сквозь одиночный блок, даже разогнавшись.
    #[test]
    fn a_falling_item_stops_on_a_single_block() {
        // Один блок на высоте 100, пола больше нет нигде.
        let ledge = |x: i32, y: i32, z: i32| (x, y, z) == (0, 100, 0);

        let mut items = thrown((0.5, 180.0, 0.5), (0.0, 0.0, 0.0));

        for _ in 0..300 {
            items.step(&ledge);
        }

        let landed = &items.items()[0];

        assert_eq!(landed.y, 101.0, "пролетел сквозь блок");
    }

    /// Упавший в пустоту предмет пропадает, и об этом есть запись.
    #[test]
    fn an_item_lost_in_the_void_disappears() {
        let mut items = thrown((0.5, 0.0, 0.5), (0.0, 0.0, 0.0));

        for _ in 0..400 {
            items.step(&|_, _, _| false);
        }

        assert!(items.items().is_empty());

        let (changes, _) = items.changes_since(0, READER);

        assert!(
            changes
                .iter()
                .any(|change| matches!(change, Change::Gone { .. }))
        );
    }

    /// Пролежавший слишком долго предмет пропадает: иначе брошенное копилось
    /// бы в памяти сервера до самой остановки.
    #[test]
    fn an_old_item_disappears() {
        let mut items = thrown((0.5, 64.0, 0.5), (0.0, 0.0, 0.0));

        items.step(&floor);
        assert_eq!(items.items().len(), 1);

        // Переводим часы предмета назад: столько он уже пролежал.
        items.items[0].born -= LIFETIME;

        items.step(&floor);

        assert!(items.items().is_empty());

        let (changes, _) = items.changes_since(0, READER);

        assert!(
            changes
                .iter()
                .any(|change| matches!(change, Change::Gone { .. }))
        );
    }

    /// Падающий блок ведёт себя как предмет, но его нельзя поднять,
    /// а долетев до земли, он отдаётся обратно миру.
    #[test]
    fn a_falling_block_is_not_picked_up_but_lands() {
        let mut items = Items::new();

        items.drop_block(1, 42, Stack::new(7, 1), (0.5, 5.0, 0.5));

        // Пол на нулевой высоте.
        let solid = |_: i32, y: i32, _: i32| y <= 0;

        // Подойти и поднять его нельзя.
        assert!(items.take_near((0.5, 5.0, 0.5), 99).is_empty(), "падающий блок подняли");

        for _ in 0..60 {
            items.step(&solid);
        }

        let landed = items.take_landed();

        assert_eq!(landed.len(), 1, "блок не приземлился");
        assert_eq!(landed[0].block, Some(42));
        assert!(landed[0].y >= 1.0 && landed[0].y < 1.5, "лёг не на пол: {}", landed[0].y);

        // И больше его в мире нет.
        assert!(items.items().is_empty());
    }

}
