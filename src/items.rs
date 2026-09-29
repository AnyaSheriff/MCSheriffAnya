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
//
// Здесь же живёт подожжённый динамит: он летит и падает так же (вики,
// Entity: у падающих блоков и TNT тяжесть 0.04 и сопротивление 0.98 после
// шага), только у него есть запал, а поднять его нельзя.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::inventory::Stack;
use crate::journal::{Journal, Reader};

/// Докуда дотягивается игрок: его рамка (0.6 × 1.8), раздвинутая на блок
/// по сторонам и на полблока вверх и вниз, должна задеть рамку предмета
/// (0.25 × 0.25) — вики, Item (entity), «Pickup». По сторонам это
/// 0.3 + 1 + 0.125 от оси игрока до оси предмета — квадрат, а не круг:
/// наискось игрок дотягивается дальше (чёрный ящик, dropper-pulse — с кругом
/// в 1.4 камень поднимался на 2 такта позже).
const PICK_UP_REACH: f64 = 0.3 + 1.0 + 0.125;

/// По высоте: низ предмета ниже макушки игрока + 0.5, верх предмета выше
/// его ног − 0.5.
const PICK_UP_ABOVE: f64 = 1.8 + 0.5;
const PICK_UP_BELOW: f64 = 0.5 + 0.25;

/// Сколько предмет нельзя поднимать после того, как игрок его выбросил:
/// две секунды, как в игре.
///
/// Иначе выброшенное тут же вернулось бы обратно: игрок стоит рядом с тем,
/// что бросил.
pub const THROWN_DELAY: Duration = Duration::from_secs(2);

/// Сколько нельзя поднимать то, что выпало из сломанного блока: полсекунды,
/// как в игре.
pub const MINED_DELAY: Duration = Duration::from_millis(500);

/// Предмет из выбрасывателя поднимается сразу: у оригинала робот подобрал
/// его через 4 такта после выброса (чёрный ящик, dropper-pulse).
pub const DISPENSED_DELAY: Duration = Duration::ZERO;

/// Насколько предмет тянет вниз за такт.
const GRAVITY: f64 = 0.04;

/// Сопротивление воздуха: на столько скорость умножается каждый такт.
const DRAG: f64 = 0.98;

/// Трение о землю: лежащий предмет останавливается быстро, а не скользит.
///
/// Число общее для трения о блоки; на вики оно указано для существ, стоящих
/// на земле, и здесь взято то же.
const GROUND_FRICTION: f64 = 0.6;

/// Толчок течения за такт — снято с оригинала: скорость по течению растёт
/// как v·0,9702 + 0,02758 (трение 0,99 в воде и 0,98 после шага).
const WATER_PUSH: f64 = 0.028_423;

/// Трение в воде до шага.
const WATER_DRAG: f64 = 0.99;

/// Подъём в воде за такт, пока всплывает медленнее 0,06.
const BUOYANCY: f64 = 0.0005;

/// Скорость меньше этой считается нулевой: иначе предмет вечно ползал бы на
/// тысячные доли блока, а клиентам летели бы пакеты о его движении.
const STILL: f64 = 0.003;

/// С какой скоростью вылетает выброшенный предмет: вперёд и немного вверх.
///
/// Точных чисел на вики нет, поэтому взяты такие: бросок получается
/// на пару блоков вперёд, как в игре.
pub const THROW_SPEED: f64 = 0.3;
pub const THROW_LIFT: f64 = 0.12;

/// Раз во сколько тактов лежащий предмет всё же сдвигается (probe-item-rest).
const ITEM_MOVE_EVERY: i64 = 4;

/// Скользящий по земле предмет (квадрат скорости вбок больше этого)
/// двигается каждый такт [проверить: порог не замерен, взят как у STILL].
const SLIDING: f64 = 1.0e-5;

/// Перемена скорости за такт (квадрат), после которой оригинал рассказывает
/// о предмете вне очереди: лежащий предмет, накопивший −0,12, в свой такт
/// сдвига шлёт ноль сразу, а накопивший −0,08 — нет (probe-item-rest).
const IMPULSE: f64 = 0.01;

/// Ширина предмета: по ней он упирается в стены.
const SIZE: f64 = 0.25;

/// Насколько ниже лежащего предмета ищется опора: он лежит ровно на верху
/// блока, и сам этот блок — чуть ниже.
const SUPPORT_PROBE: f64 = 0.01;

/// Насколько предмет сдвигается за один шаг проверки.
///
/// Шаг должен быть заметно меньше блока: иначе быстрое движение перескочило бы
/// через блок, и предмет прошёл бы сквозь пол.
const MAX_STEP: f64 = 0.2;

/// Сколько предмет лежит, прежде чем пропасть: пять минут, как в игре.
///
/// Без этого брошенное копилось бы в памяти сервера до самой его остановки.
const LIFETIME: Duration = Duration::from_secs(300);

/// Здоровье лежащего предмета: 5 (вики, Item (entity): «Health: The health
/// of the item, which starts at 5»). Его отнимает взрыв.
pub const ITEM_HEALTH: f32 = 5.0;

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

    /// Подожжённый динамит — сколько тактов осталось до взрыва; `block` у
    /// него — состояние блока динамита (его клиент и рисует). У прочих —
    /// None.
    pub fuse: Option<i32>,

    /// Особый динамит (custom_tnt.rs) — какой; у обычного и у прочих
    /// сущностей — None.
    pub custom: Option<crate::custom_tnt::KindId>,

    /// Здоровье: у предмета его отнимает взрыв (`ITEM_HEALTH`).
    pub health: f32,

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

    /// Касался ли воды в прошлый такт (None — такта ещё не было): вход в
    /// воду даёт плеск.
    in_water: Option<bool>,

    /// Запал, который клиенты уже знают (у динамита).
    shown_fuse: i32,

    /// Без тяжести (`NoGravity` в призыве): не падает, лежит где висит.
    pub no_gravity: bool,

    /// Сколько тактов сущности прожил предмет — по нему идут и такт
    /// «лежащий предмет всё же сдвигается», и рассылка (network/play.rs).
    pub age: u64,

    /// Скорость за этот такт изменилась резко (квадрат разницы больше 0,01):
    /// оригинал в таком такте рассказывает о предмете вне очереди.
    pub impulse: bool,

    /// Где предмет был к началу такта; у появившегося в этом такте — None.
    /// Игрок подбирает по этому месту: у оригинала подбор идёт в начале
    /// такта, до шага предметов (чёрный ящик, dropper-pulse — у нас на такт
    /// раньше, пока брали место после шага).
    touchable: Option<(f64, f64, f64)>,
}

/// Вода в клетке для предмета: высота её поверхности над низом клетки
/// (0..=1) и направление течения по горизонтали (единичное или нулевое).
pub struct ItemWater {
    pub height: f64,
    pub flow: (f64, f64),
}

/// Плеск: предмет вошёл в воду — где и насколько громко.
pub struct Splash {
    pub at: (f64, f64, f64),
    pub volume: f32,
}

/// Что случилось с лежащими предметами — запись журнала.
#[derive(Clone, Debug)]
pub enum Change {
    /// Предмет появился в мире.
    Dropped(Dropped),

    /// Предмет пропал сам: провалился в пустоту.
    Gone { entity_id: i32 },

    /// У динамита убыл запал: оригинал сообщает его клиентам каждый такт.
    Fuse { entity_id: i32, fuse: i32 },

    /// Появилась вагонетка (minecart.rs). Пропадает она записью `Gone`.
    Cart(crate::minecart::Minecart),

    /// Ударила молния (lightning.rs). Пропадает она записью `Gone`.
    Lightning(crate::lightning::Bolt),

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

    /// Вагонетки в порядке появления: у них своя физика (minecart.rs), но
    /// журнал общий с предметами — клиенты узнают о них тем же путём.
    carts: Vec<crate::minecart::Minecart>,

    /// Молнии: живут несколько тактов, журнал тоже общий.
    bolts: Vec<crate::lightning::Bolt>,

    changes: Journal<Change>,
}

impl Items {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            carts: Vec::new(),
            bolts: Vec::new(),
            changes: Journal::new(),
        }
    }

    /// Ставит вагонетку в мир.
    pub fn add_cart(&mut self, cart: crate::minecart::Minecart) {
        self.changes.push(Change::Cart(cart.clone()));
        self.carts.push(cart);
    }

    /// Ставит молнию в мир: клиенты узнают о ней сразу.
    pub fn add_bolt(&mut self, bolt: crate::lightning::Bolt) {
        self.changes.push(Change::Lightning(bolt.clone()));
        self.bolts.push(bolt);
    }

    /// Такт молний: `step` возвращает false у погасшей — она убирается.
    pub fn step_bolts(&mut self, mut step: impl FnMut(&mut crate::lightning::Bolt) -> bool) {
        let gone: Vec<crate::lightning::Bolt> = self.bolts.extract_if(.., |bolt| !step(bolt)).collect();

        for bolt in &gone {
            self.changes.push(Change::Gone { entity_id: bolt.entity_id });
        }
    }

    /// Убирает молнии, которые выбирает `gone` (команда `kill`); сколько убрано.
    pub fn remove_bolts(&mut self, gone: impl FnMut(&mut crate::lightning::Bolt) -> bool) -> usize {
        let removed: Vec<crate::lightning::Bolt> = self.bolts.extract_if(.., gone).collect();

        for bolt in &removed {
            self.changes.push(Change::Gone { entity_id: bolt.entity_id });
        }

        removed.len()
    }

    /// Все вагонетки — в порядке появления.
    pub fn carts(&self) -> &[crate::minecart::Minecart] {
        &self.carts
    }

    pub fn carts_mut(&mut self) -> &mut [crate::minecart::Minecart] {
        &mut self.carts
    }

    /// Сам список вагонеток — чтобы на время вынуть его целиком.
    pub fn carts_vec(&mut self) -> &mut Vec<crate::minecart::Minecart> {
        &mut self.carts
    }

    /// Убирает вагонетки, которые выбирает `gone` (сломали, убили
    /// командой, провалились в пустоту), и возвращает их.
    pub fn remove_carts(&mut self, gone: impl FnMut(&mut crate::minecart::Minecart) -> bool) -> Vec<crate::minecart::Minecart> {
        let removed: Vec<crate::minecart::Minecart> = self.carts.extract_if(.., gone).collect();

        for cart in &removed {
            self.changes.push(Change::Gone { entity_id: cart.entity_id });
        }

        removed
    }

    /// Призванный командой предмет: как выпавший, только может быть без
    /// тяжести (`NoGravity:1b`).
    pub fn summon_item(&mut self, entity_id: i32, stack: Stack, at: (f64, f64, f64), no_gravity: bool) {
        let mut item = Self::new_item(entity_id, stack, at, (0.0, 0.0, 0.0), Duration::ZERO);

        item.no_gravity = no_gravity;
        self.items.push(item.clone());
        self.changes.push(Change::Dropped(item));
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
        let dropped = Self::new_item(entity_id, stack, (x, y, z), velocity, delay);

        self.items.push(dropped.clone());
        self.changes.push(Change::Dropped(dropped));
    }

    /// Выпавший предмет, ещё не положенный в мир.
    fn new_item(
        entity_id: i32,
        stack: Stack,
        (x, y, z): (f64, f64, f64),
        velocity: (f64, f64, f64),
        delay: Duration,
    ) -> Dropped {
        Dropped {
            entity_id,
            uuid: uuid_for(entity_id),
            stack,
            block: None,
            fuse: None,
            custom: None,
            health: ITEM_HEALTH,
            x,
            y,
            z,
            vx: velocity.0,
            vy: velocity.1,
            vz: velocity.2,
            on_ground: false,
            ready_at: Instant::now() + delay,
            born: Instant::now(),
            in_water: None,
            shown_fuse: 0,
            touchable: None,
            age: 0,
            impulse: false,
            no_gravity: false,
        }
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
            fuse: None,
            custom: None,
            health: ITEM_HEALTH,
            x,
            y,
            z,
            vx: 0.0,
            vy: 0.0,
            vz: 0.0,
            on_ground: false,
            ready_at: Instant::now(),
            born: Instant::now(),
            in_water: None,
            shown_fuse: 0,
            touchable: None,
            age: 0,
            impulse: false,
            no_gravity: false,
        };

        self.items.push(falling.clone());
        self.changes.push(Change::Dropped(falling));
    }

    /// Подожжённый динамит: сущность с запалом `fuse` тактов. `state` —
    /// состояние блока динамита, `velocity` — начальный толчок, `custom` —
    /// вид особого динамита.
    pub fn prime(
        &mut self,
        entity_id: i32,
        state: i32,
        (x, y, z): (f64, f64, f64),
        velocity: (f64, f64, f64),
        fuse: i32,
        custom: Option<crate::custom_tnt::KindId>,
    ) {
        let primed = Dropped {
            entity_id,
            uuid: uuid_for(entity_id),
            stack: Stack::new(0, 0),
            block: Some(state),
            fuse: Some(fuse),
            custom,
            health: ITEM_HEALTH,
            x,
            y,
            z,
            vx: velocity.0,
            vy: velocity.1,
            vz: velocity.2,
            on_ground: false,
            ready_at: Instant::now(),
            born: Instant::now(),
            in_water: None,
            // Необычный запал (от взрыва) оригинал в следующем такте шлёт
            // ещё раз, хотя тот не изменился (чёрный ящик, tnt-chain).
            shown_fuse: if fuse == crate::tnt::FUSE { fuse } else { -1 },
            touchable: None,
            age: 0,
            impulse: false,
            no_gravity: false,
        };

        self.items.push(primed.clone());
        self.changes.push(Change::Dropped(primed));
    }

    /// Запал убывает на единицу за такт; на нуле динамит взрывается (вики,
    /// TNT: «The timer decreases by 1 every game tick, and the Primed TNT
    /// explodes when it reaches 0»). Сперва шаг, потом запал — зовётся после
    /// `step_with_water`. Возвращает взорвавшиеся: они уже убраны из мира,
    /// взрыв устраивает вызывающий. В выгруженном чанке запал стоит, как и
    /// всё там.
    pub fn burn_fuses<L>(&mut self, loaded: &L) -> Vec<Dropped>
    where
        L: Fn(i32, i32) -> bool,
    {
        for item in &mut self.items {
            let chunk = ((item.x.floor() as i32).div_euclid(16), (item.z.floor() as i32).div_euclid(16));

            if let Some(fuse) = &mut item.fuse
                && loaded(chunk.0, chunk.1)
            {
                // Клиентам уходит запал, каким он стал к концу прошлого
                // такта: в такте взрыва ещё видна единица (чёрный ящик,
                // tnt-lever).
                if *fuse != item.shown_fuse {
                    item.shown_fuse = *fuse;
                    self.changes.push(Change::Fuse { entity_id: item.entity_id, fuse: *fuse });
                }

                *fuse -= 1;
            }
        }

        let exploded: Vec<Dropped> = self.items.extract_if(.., |item| item.fuse.is_some_and(|fuse| fuse <= 0)).collect();

        for item in &exploded {
            self.changes.push(Change::Gone { entity_id: item.entity_id });
        }

        exploded
    }

    /// Всё, что лежит в мире, — для взрыва: он толкает и ранит.
    pub fn items_mut(&mut self) -> &mut [Dropped] {
        &mut self.items
    }

    /// Убирает предметы, которые выбирает `gone` (команда `kill`), и
    /// возвращает их.
    pub fn remove_items(&mut self, gone: impl FnMut(&mut Dropped) -> bool) -> Vec<Dropped> {
        let removed: Vec<Dropped> = self.items.extract_if(.., gone).collect();

        for item in &removed {
            self.changes.push(Change::Gone { entity_id: item.entity_id });
        }

        removed
    }

    /// Убирает предметы, у которых не осталось здоровья, — их уничтожил
    /// взрыв.
    pub fn remove_broken(&mut self) {
        let broken: Vec<Dropped> = self.items.extract_if(.., |item| item.health <= 0.0).collect();

        for item in &broken {
            self.changes.push(Change::Gone { entity_id: item.entity_id });
        }
    }

    /// Забирает падающие блоки, которые уже приземлились.
    ///
    /// Вызывающий ставит их обратно в мир — или роняет предметом, если
    /// место занято.
    pub fn take_landed(&mut self) -> Vec<Dropped> {
        // Вынимаем на месте: остальные предметы не переезжают в новый
        // список каждый такт.
        let landed: Vec<Dropped> = self
            .items
            .extract_if(.., |item| item.block.is_some() && item.fuse.is_none() && item.on_ground)
            .collect();

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
    /// `solid` отвечает, можно ли на этом блоке лежать предмету и упираться
    /// в него, `holds_block` — то же для падающего блока: у них правила
    /// разные (падающий блок проходит в невысокие блоки и разбивается там,
    /// а предмет ложится сверху). `loaded` — лежит ли в памяти чанк с такими
    /// номерами: в выгруженном чанке мира нет, и предмет провалился бы
    /// в пустоту, поэтому там он замирает, пока чанк не вернётся.
    #[cfg(test)]
    pub fn step<F, B, L>(&mut self, solid: &F, holds_block: &B, loaded: &L)
    where
        F: Fn(i32, i32, i32) -> bool,
        B: Fn(i32, i32, i32) -> bool,
        L: Fn(i32, i32) -> bool,
    {
        self.step_with_water(solid, holds_block, loaded, &|_, _, _| None);
    }

    /// Такт предметов с водой: течение несёт выпавшие предметы, в воде они
    /// всплывают, а входя в воду — плещут. Возвращает плески.
    pub fn step_with_water<F, B, L, W>(&mut self, solid: &F, holds_block: &B, loaded: &L, water: &W) -> Vec<Splash>
    where
        F: Fn(i32, i32, i32) -> bool,
        B: Fn(i32, i32, i32) -> bool,
        L: Fn(i32, i32) -> bool,
        W: Fn(i32, i32, i32) -> Option<ItemWater>,
    {
        let mut splashes = Vec::new();

        for item in &mut self.items {
            item.impulse = false;

            let chunk = (
                (item.x.floor() as i32).div_euclid(16),
                (item.z.floor() as i32).div_euclid(16),
            );

            if !loaded(chunk.0, chunk.1) {
                continue;
            }

            // Динамит ложится на всё, у чего есть ящик, как предмет; воду
            // у него пока не считаем. [проверить]: течение сносит динамит
            // (вики, TNT: «can be pushed by flowing water»).
            let before = (item.vx, item.vy, item.vz);

            if item.fuse.is_some() {
                item.step(solid, &|_, _, _| None, None);
            } else if item.block.is_some() {
                item.step(holds_block, &|_, _, _| None, None);
            } else {
                // Лежащий предмет сдвигается (и замечает, что опоры нет) не
                // каждый такт, а раз в 4 — по номеру и возрасту; тяжесть же
                // копится каждый такт. Снято чёрным ящиком (probe-item-rest):
                // лежащий предмет раз в 20 тактов сообщает скорость −0,04,
                // −0,08 или −0,12 — смотря по фазе, — и сразу за ней ноль в
                // такте, когда он «упал» на свой же пол.
                let cycle = (item.age as i64 + item.entity_id as i64).rem_euclid(ITEM_MOVE_EVERY) == 0;

                if let Some(splash) = item.step(solid, water, Some(cycle)) {
                    splashes.push(splash);
                }
            }

            let change = (item.vx - before.0).powi(2) + (item.vy - before.1).powi(2) + (item.vz - before.2).powi(2);

            item.impulse = change > IMPULSE;
            item.age += 1;
        }

        // Пропадают провалившиеся в пустоту и пролежавшие слишком долго.
        let bottom = crate::world::MIN_Y as f64 - LOST_BELOW;
        let now = Instant::now();

        let gone: Vec<Dropped> = self
            .items
            .extract_if(.., |item| item.y < bottom || now.duration_since(item.born) >= LIFETIME)
            .collect();

        for item in &gone {
            self.changes.push(Change::Gone {
                entity_id: item.entity_id,
            });
        }

        splashes
    }

    /// Забирает предметы, до которых дотянулся игрок, — столько, сколько
    /// он может унести.
    ///
    /// `accept` получает стопку и кладёт в инвентарь, сколько влезет;
    /// отвечает, сколько штук взято. Что не влезло, остаётся лежать тем же
    /// предметом, только стопка меньше: в полный инвентарь не поднимается
    /// ничего, и ничего не пропадает.
    ///
    /// Возвращает поднятое — со взятым числом. Слишком свежее — то, что
    /// только что выброшено, — остаётся лежать.
    /// Первый лежащий предмет (не падающий блок), чья точка внутри
    /// области, — первым в мир попавший. Для воронки: она подбирает не
    /// глядя на задержку подбора.
    pub fn first_inside(&self, inside: &dyn Fn((f64, f64, f64)) -> bool) -> Option<Stack> {
        self.items
            .iter()
            .find(|item| item.block.is_none() && inside((item.x, item.y, item.z)))
            .map(|item| item.stack)
    }

    /// Забирает `count` штук у первого предмета внутри области: целиком —
    /// предмет пропадает, частью — остаток лежит дальше.
    pub fn take_from_first_inside(&mut self, inside: &dyn Fn((f64, f64, f64)) -> bool, count: i32) {
        let Some(index) = self.items.iter().position(|item| item.block.is_none() && inside((item.x, item.y, item.z))) else {
            return;
        };

        if count >= self.items[index].stack.count {
            let gone = self.items.remove(index);

            self.changes.push(Change::Gone { entity_id: gone.entity_id });
            return;
        }

        let left = &mut self.items[index];

        left.stack.count -= count;
        left.uuid = uuid_for(left.entity_id);
        self.changes.push(Change::Gone { entity_id: left.entity_id });
        self.changes.push(Change::Dropped(left.clone()));
    }

    /// Начало такта: запомнить, где лежит каждый предмет, — по этим местам
    /// игроки подбирают до конца такта.
    pub fn note_tick_start(&mut self) {
        for item in &mut self.items {
            item.touchable = Some((item.x, item.y, item.z));
        }
    }

    pub fn take_near<A>(&mut self, (x, y, z): (f64, f64, f64), by: i32, mut accept: A) -> Vec<Dropped>
    where
        A: FnMut(Stack) -> i32,
    {
        let now = Instant::now();
        let mut taken = Vec::new();
        let mut index = 0;

        while index < self.items.len() {
            let item = &self.items[index];

            if item.block.is_some() || item.ready_at > now || !item.reaches(x, y, z) {
                index += 1;
                continue;
            }

            let count = accept(item.stack).clamp(0, item.stack.count);

            if count == 0 {
                index += 1;
                continue;
            }

            self.changes.push(Change::Taken {
                entity_id: item.entity_id,
                by,
                count,
            });

            if count == item.stack.count {
                taken.push(self.items.remove(index));
                continue;
            }

            // Взята часть: клиенту подбор показывается как у целого предмета
            // (он исчезает), а остаток появляется снова на том же месте с тем
            // же номером — сказать клиенту «стопка стала меньше» журналу
            // нечем.
            let left = &mut self.items[index];
            let mut picked = left.clone();

            picked.stack.count = count;
            left.stack.count -= count;
            left.uuid = uuid_for(left.entity_id);

            self.changes.push(Change::Dropped(left.clone()));
            taken.push(picked);
            index += 1;
        }

        taken
    }

    /// Едущие блоки поршней толкают всё, что лежит у них на пути
    /// (redstone::shoves): предмет уезжает до края блока и ещё на зазор.
    /// Скорость толчок не меняет — у оригинала сдвинутый предмет шлёт лишь
    /// сдвиг места, без скорости (piston-push-items).
    pub fn shove<F>(&mut self, shoves: &[crate::redstone::Shove], solid: &F)
    where
        F: Fn(i32, i32, i32) -> bool,
    {
        if shoves.is_empty() {
            return;
        }

        for item in &mut self.items {
            for shove in shoves {
                let (half, height) = item.size();
                let min = (item.x - half, item.y, item.z - half);
                let max = (item.x + half, item.y + height, item.z + half);

                if let Some(distance) = crate::redstone::shove_distance(shove, min, max) {
                    item.shove((shove.dir.0 * distance, shove.dir.1 * distance, shove.dir.2 * distance), solid);
                }
            }
        }
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
    /// Имя вида сущности, как в командах: подожжённый динамит, падающий
    /// блок или предмет.
    pub fn type_name(&self) -> &'static str {
        if self.fuse.is_some() {
            "tnt"
        } else if self.block.is_some() {
            "falling_block"
        } else {
            "item"
        }
    }

    /// Половина ширины и высота рамки: у предмета 0,25 × 0,25, у динамита
    /// и падающего блока 0,98 × 0,98 (вики, Item (entity), TNT, Falling
    /// Block).
    fn size(&self) -> (f64, f64) {
        if self.block.is_some() { (0.49, 0.98) } else { (0.125, 0.25) }
    }

    /// Сдвиг поршнем: предмет переносится, упираясь в блоки, а скорость у
    /// него остаётся прежней. Сдвинутый вбок уже не «лежит» — в следующем
    /// такте он двигается сам и снова ложится (у оригинала так: сдвинутый
    /// лежащий предмет следующий свой отчёт шлёт лишь через полный круг из
    /// 4 тактов, piston-push-items).
    fn shove<F>(&mut self, by: (f64, f64, f64), solid: &F)
    where
        F: Fn(i32, i32, i32) -> bool,
    {
        let speed = (self.vx, self.vy, self.vz);
        let longest = by.0.abs().max(by.1.abs()).max(by.2.abs());
        let steps = (longest / MAX_STEP).ceil().max(1.0);

        for _ in 0..steps as i32 {
            self.step_sideways(by.0 / steps, 0.0, solid);
            self.step_sideways(0.0, by.2 / steps, solid);
            self.step_down_or_up(by.1 / steps, solid);
        }

        (self.vx, self.vy, self.vz) = speed;
        self.on_ground = false;
    }

    /// Дотягивается ли игрок, стоящий здесь, до этого предмета.
    fn reaches(&self, x: f64, y: f64, z: f64) -> bool {
        let Some((at_x, at_y, at_z)) = self.touchable else {
            return false;
        };
        (at_x - x).abs() < PICK_UP_REACH
            && (at_z - z).abs() < PICK_UP_REACH
            && at_y < y + PICK_UP_ABOVE
            && at_y > y - PICK_UP_BELOW
    }

    /// Двигает предмет на один такт.
    ///
    /// Порядок в точности такой, как в игре: сперва к скорости добавляется
    /// тяжесть, потом предмет двигается, и лишь потом скорость уменьшается
    /// сопротивлением. От порядка зависит вид дуги, поэтому он важен.
    fn step<F, W>(&mut self, solid: &F, water: &W, cycle: Option<bool>) -> Option<Splash>
    where
        F: Fn(i32, i32, i32) -> bool,
        W: Fn(i32, i32, i32) -> Option<ItemWater>,
    {
        // Вода вокруг: касается ли её предмет и насколько глубоко сидит.
        // Снято с оригинала чёрным ящиком (tools/blackbox/itemflow.js).
        let cell = (self.x.floor() as i32, self.y.floor() as i32, self.z.floor() as i32);
        let here = water(cell.0, cell.1, cell.2);
        let depth = here.as_ref().map(|water| cell.1 as f64 + water.height - self.y);
        let touching = depth.is_some_and(|depth| depth > 0.0);

        // Вошёл в воду — плеск: громче, чем быстрее шёл (вертикаль весит
        // впятеро больше). Первый такт после появления не считается.
        let splash = (self.in_water == Some(false) && touching).then(|| Splash {
            at: (self.x, self.y, self.z),
            volume: ((self.vx * self.vx * 0.2 + self.vy * self.vy + self.vz * self.vz * 0.2).sqrt() * 0.2).min(1.0) as f32,
        });
        self.in_water = Some(touching);

        if touching {
            // Течение подталкивает: предмет в воде уже не «лежит».
            if let Some(water) = &here
                && water.flow != (0.0, 0.0)
            {
                self.vx += water.flow.0 * WATER_PUSH;
                self.vz += water.flow.1 * WATER_PUSH;
                self.on_ground = false;
            }

            // Глубже десятой блока — плывёт: всплывает, без тяжести, вязко.
            if depth.is_some_and(|depth| depth > 0.1) {
                if self.vy < 0.06 {
                    self.vy += BUOYANCY;
                }
                self.vx *= WATER_DRAG;
                self.vz *= WATER_DRAG;
                self.on_ground = false;

                self.move_across(solid);
                self.slow_down();
                return splash;
            }
        }

        self.fall(solid, cycle);
        splash
    }

    /// Обычный такт без воды: тяжесть, шаг, трение.
    ///
    /// `cycle` — у выпавшего предмета: Some(такт ли сдвигаться лежащему).
    /// Лежащий и не скользящий предмет в прочие такты не двигается вовсе:
    /// тяжесть копится в скорости без трения (у оригинала лежащий шлёт
    /// ровно −0,08 и −0,12, а не −0,0792), а опору он проверяет в свой
    /// такт. У динамита и падающего блока (None) — как раньше: лежит, пока
    /// есть опора.
    fn fall<F>(&mut self, solid: &F, cycle: Option<bool>)
    where
        F: Fn(i32, i32, i32) -> bool,
    {
        if let Some(cycle) = cycle {
            if !self.no_gravity {
                self.vy -= GRAVITY;
            }

            let sliding = self.vx * self.vx + self.vz * self.vz > SLIDING;

            if self.on_ground && !sliding && !cycle {
                return;
            }

            self.move_across(solid);
            self.slow_down();
            return;
        }

        if self.resting() {
            // Лежит — пока под ним есть на чём лежать. Опору сломали —
            // предмет снова падает, а не висит в воздухе.
            let below = (self.y - SUPPORT_PROBE).floor() as i32;

            if solid(self.x.floor() as i32, below, self.z.floor() as i32) {
                return;
            }

            self.on_ground = false;
        }

        self.vy -= GRAVITY;

        self.move_across(solid);
        self.slow_down();
    }

    /// Трение после шага: в воздухе и на земле.
    fn slow_down(&mut self) {
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
pub(crate) fn uuid_for(entity_id: i32) -> [u8; 16] {
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

    /// Весь мир в памяти.
    fn loaded(_: i32, _: i32) -> bool {
        true
    }

    /// Поднимающий, у которого место есть для всего.
    fn anything(stack: Stack) -> i32 {
        stack.count
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

    /// Лежащий предмет сдвигается раз в 4 такта (по возрасту и номеру), а
    /// тяжесть копится каждый такт без трения: −0,04, −0,08, −0,12, потом
    /// сдвиг и ноль. Резкая перемена (с −0,12 до нуля) — отчёт вне очереди,
    /// с −0,08 — нет (probe-item-rest).
    #[test]
    fn a_lying_item_moves_every_fourth_tick() {
        // Номер 1: сдвиг в такты, где возраст + 1 делится на 4.
        let mut items = thrown((0.5, 64.0, 0.5), (0.0, 0.0, 0.0));
        let mut seen = Vec::new();

        for _ in 0..12 {
            items.step(&floor, &floor, &loaded);
            let item = &items.items()[0];
            seen.push(((item.vy * 100.0).round() as i32, item.impulse));
        }

        // Такт 0 — первый шаг, падать некуда; такт 3 — свой такт сдвига.
        assert_eq!(
            seen,
            [
                (0, false), (-4, false), (-8, false), (0, false),
                (-4, false), (-8, false), (-12, false), (0, true),
                (-4, false), (-8, false), (-12, false), (0, true),
            ]
        );
        assert_eq!(items.items()[0].y, 64.0);
    }

    /// Поршень сдвигает предмет, не меняя скорости; сдвинутый вбок уже не
    /// лежит — в следующем такте он двигается сам и ложится снова.
    #[test]
    fn a_shoved_item_keeps_its_speed_and_settles_again() {
        let mut items = thrown((1.5, 64.0, 0.5), (0.0, 0.0, 0.0));

        items.step(&floor, &floor, &loaded);
        assert!(items.items()[0].on_ground);

        let head = crate::redstone::Shove { min: (0.5, 64.0, 0.0), max: (1.5, 65.0, 1.0), dir: (1.0, 0.0, 0.0) };
        items.shove(&[head], &floor);

        let item = &items.items()[0];
        assert!((item.x - 1.635).abs() < 1e-9, "до края головы и ещё 0,01: {}", item.x);
        assert!(!item.on_ground);
        assert_eq!((item.vx, item.vy, item.vz), (0.0, 0.0, 0.0));

        items.step(&floor, &floor, &loaded);
        assert!(items.items()[0].on_ground);
        assert_eq!(items.items()[0].y, 64.0);
    }

    /// Призванный без тяжести предмет висит, где его поставили.
    #[test]
    fn an_item_without_gravity_stays_in_the_air() {
        let mut items = Items::new();

        items.watch_changes(READER);
        items.summon_item(1, stack(), (0.5, 70.0, 0.5), true);

        for _ in 0..40 {
            items.step(&floor, &floor, &loaded);
        }

        let item = &items.items()[0];
        assert_eq!((item.y, item.vy), (70.0, 0.0));
        assert!(!item.impulse);
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

        items.note_tick_start();
        let taken = items.take_near((0.0, 64.0, 0.0), 42, anything);

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

        items.note_tick_start();
        assert!(items.take_near((0.0, 64.0, 0.0), 1, anything).is_empty());
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
            items.step(&floor, &floor, &loaded);
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
            items.step(&floor, &floor, &loaded);
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
            items.step(&floor, &floor, &loaded);
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
            items.step(&wall, &wall, &loaded);
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
            items.step(&floor, &floor, &loaded);
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
            items.step(&ledge, &ledge, &loaded);
        }

        let landed = &items.items()[0];

        assert_eq!(landed.y, 101.0, "пролетел сквозь блок");
    }

    /// Упавший в пустоту предмет пропадает, и об этом есть запись.
    #[test]
    fn an_item_lost_in_the_void_disappears() {
        let mut items = thrown((0.5, 0.0, 0.5), (0.0, 0.0, 0.0));

        for _ in 0..400 {
            items.step(&|_, _, _| false, &|_, _, _| false, &loaded);
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

        items.step(&floor, &floor, &loaded);
        assert_eq!(items.items().len(), 1);

        // Переводим часы предмета назад: столько он уже пролежал.
        items.items[0].born -= LIFETIME;

        items.step(&floor, &floor, &loaded);

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
        items.note_tick_start();
        assert!(items.take_near((0.5, 5.0, 0.5), 99, anything).is_empty(), "падающий блок подняли");

        for _ in 0..60 {
            items.step(&solid, &solid, &loaded);
        }

        let landed = items.take_landed();

        assert_eq!(landed.len(), 1, "блок не приземлился");
        assert_eq!(landed[0].block, Some(42));
        assert!(landed[0].y >= 1.0 && landed[0].y < 1.5, "лёг не на пол: {}", landed[0].y);

        // И больше его в мире нет.
        assert!(items.items().is_empty());
    }

    /// В полный инвентарь не поднимается ничего: предмет лежит дальше,
    /// а не пропадает.
    #[test]
    fn a_full_inventory_leaves_the_item_on_the_ground() {
        let mut items = Items::new();
        items.watch_changes(READER);

        items.drop_item(1, stack(), (0.5, 64.0, 0.5), (0.0, 0.0, 0.0), Duration::ZERO);

        items.note_tick_start();
        let taken = items.take_near((0.5, 64.0, 0.5), 42, |_| 0);

        assert!(taken.is_empty());
        assert_eq!(items.items().len(), 1);
        assert_eq!(items.items()[0].stack.count, 3);

        // О подборе в журнале ничего нет.
        let (changes, _) = items.changes_since(1, READER);
        assert!(changes.is_empty());
    }

    /// Влезла часть — поднимается часть, остаток лежит тем же предметом.
    #[test]
    fn part_of_a_stack_is_picked_up() {
        let mut items = Items::new();
        items.watch_changes(READER);

        items.drop_item(1, stack(), (0.5, 64.0, 0.5), (0.0, 0.0, 0.0), Duration::ZERO);

        items.note_tick_start();
        let taken = items.take_near((0.5, 64.0, 0.5), 42, |stack| stack.count.min(2));

        assert_eq!(taken.len(), 1);
        assert_eq!(taken[0].stack.count, 2);
        assert_eq!(items.items().len(), 1);
        assert_eq!(items.items()[0].stack.count, 1);
        assert_eq!(items.items()[0].entity_id, 1);

        // Клиентам: подбор двух штук и остаток, появившийся снова.
        let (changes, _) = items.changes_since(1, READER);

        assert!(matches!(changes.first(), Some(Change::Taken { entity_id: 1, by: 42, count: 2 })));
        assert!(matches!(changes.get(1), Some(Change::Dropped(left)) if left.stack.count == 1));
    }

    /// Лежащий предмет падает, когда из-под него убрали опору.
    #[test]
    fn a_resting_item_falls_when_its_support_goes() {
        let mut items = thrown((0.5, 70.0, 0.5), (0.0, 0.0, 0.0));

        for _ in 0..200 {
            items.step(&floor, &floor, &loaded);
        }

        assert_eq!(items.items()[0].y, 64.0);

        // Пол опустился на пять блоков.
        let lower = |_: i32, y: i32, _: i32| y <= 58;

        for _ in 0..200 {
            items.step(&lower, &lower, &loaded);
        }

        let landed = &items.items()[0];

        assert!(landed.on_ground);
        assert_eq!(landed.y, 59.0, "остался висеть");
    }

    /// В выгруженном чанке предмет не двигается: мира под ним нет, и он
    /// провалился бы в пустоту.
    #[test]
    fn an_item_in_an_unloaded_chunk_stays_put() {
        let mut items = thrown((0.5, 70.0, 0.5), (0.0, 0.0, 0.0));

        for _ in 0..100 {
            items.step(&|_, _, _| false, &|_, _, _| false, &|_, _| false);
        }

        assert_eq!(items.items().len(), 1);
        assert_eq!(items.items()[0].y, 70.0);
    }
}
