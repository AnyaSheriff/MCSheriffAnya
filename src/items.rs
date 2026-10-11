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

use std::time::{SystemTime, UNIX_EPOCH};

use crate::inventory::Stack;
mod merging;
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

/// Сколько тактов предмет нельзя поднимать после того, как игрок его
/// выбросил: 40 (две секунды при 20 тактах в секунду), как в игре. Считается
/// в тактах, а не по часам: у отстающего сервера оригинал тоже ждёт 40
/// тактов, а не две секунды.
///
/// Иначе выброшенное тут же вернулось бы обратно: игрок стоит рядом с тем,
/// что бросил.
pub const THROWN_DELAY: u64 = 40;

/// Сколько тактов нельзя поднимать то, что выпало из сломанного блока: 10
/// (полсекунды), как в игре.
pub const MINED_DELAY: u64 = 10;

/// Предмет из выбрасывателя поднимается сразу: у оригинала робот подобрал
/// его через 4 такта после выброса (чёрный ящик, dropper-pulse).
pub const DISPENSED_DELAY: u64 = 0;

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

/// Сколько тактов предмет лежит, прежде чем пропасть: 6000 (пять минут), как
/// в игре; тоже в тактах, а не по часам.
///
/// Без этого брошенное копилось бы в памяти сервера до самой его остановки.
const LIFETIME: u64 = 6000;

/// Звезда Незера не пропадает от времени (вики, Nether Star: «does not
/// despawn»); от взрывов её бережёт tnt.rs.
fn never_despawns(item: &Dropped) -> bool {
    item.block.is_none() && Some(item.stack.item) == crate::blocks::item_named("nether_star")
}

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
    /// Бросивший игрок: для подбора золота пиглином (вики, Advancement).
    pub thrower: Option<[u8; 16]>,
    /// Получатель /give; чужие игроки остаток не поднимают.
    pub owner: Option<i32>,
    /// Такты сущности, отдельно от омолаживаемого слиянием Age.
    pub ticks: u64,

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

    /// Сколько тактов от появления предмет нельзя поднять (сравнивается с
    /// `age`).
    pickup_delay: u64,

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

    /// Размер стопки изменился при слиянии.
    Stack { entity_id: i32, stack: Stack, no_gravity: bool },

    /// У динамита убыл запал: оригинал сообщает его клиентам каждый такт.
    Fuse { entity_id: i32, fuse: i32 },

    /// Появилась вагонетка (minecart.rs). Пропадает она записью `Gone`.
    Cart(Box<crate::minecart::Minecart>),

    /// Ударила молния (lightning.rs). Пропадает она записью `Gone`.
    Lightning(crate::lightning::Bolt),

    /// Появилась рамка (frame.rs). Пропадает она записью `Gone`.
    Frame(crate::frame::Frame),

    /// В рамку положили предмет или вынули его (None): клиентам уходит
    /// свойство «предмет».
    FrameItem { entity_id: i32, item: Option<Stack> },

    /// Предмет в рамке повернули: свойство «поворот».
    FrameRotation { entity_id: i32, rotation: u8 },

    /// Звук рамки — в её месте, вперемешку с прочими событиями рамки в
    /// журнале, чтобы порядок пакетов был как у оригинала (чёрный ящик,
    /// item-frame-rotate: звук, потом свойство).
    FrameSound { at: (f64, f64, f64), name: &'static str },

    /// Что случилось с мобом: удар, смерть, звук, сердечки. Клиентам это
    /// уходит в порядке записи, вперемешку с прочими событиями.
    MobEvent { entity_id: i32, at: (f64, f64, f64), event: crate::mob::Event },

    /// Появился шарик опыта (xp.rs). Пропадает записью `Gone`, а подобранный —
    /// `Taken`.
    Orb(crate::xp::XpOrb),

    /// Предмет подняли.
    Taken {
        entity_id: i32,

        /// Кто поднял: клиент рисует, как предмет летит к этой сущности.
        by: i32,

        /// Сколько предметов в поднятой стопке — это тоже показывается.
        count: i32,
        /// Слитый шарик опыта остаётся до последнего подбора.
        remaining: bool,
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

    /// Рамки для предметов (frame.rs): висят на гранях блоков, журнал общий.
    frames: Vec<crate::frame::Frame>,

    /// Мобы (mob/mod.rs): своя физика и мысли, журнал общий.
    mobs: Vec<crate::mob::Mob>,
    /// Снимок катализаторов и очередь поглощённого опыта (redstone/sculk_family).
    pub sculk_catalysts: Vec<(i32, i32, i32)>,
    pub sculk_deaths: Vec<crate::redstone::sculk_family::DeathCharge>,
    pub vibrations: Vec<crate::redstone::sculk_family::GameEvent>,

    /// Мобы на начало такта — по ним слежение рассылает клиентам (`snapshot_mobs`).
    mob_view: Vec<crate::mob::Mob>,

    /// Шарики опыта (xp.rs) и их снимок на начало такта — как у мобов.
    orbs: Vec<crate::xp::XpOrb>,
    orb_view: Vec<crate::xp::XpOrb>,

    /// Сколько тактов игрок (по номеру сущности) ещё ждёт до следующего шарика.
    xp_delay: std::collections::HashMap<i32, u32>,

    changes: Journal<Change>,
    progress_kills: Vec<([u8; 16], &'static str)>,

    /// Эффекты «при смерти» убранных в этом такте трупов (effect_reactions.rs).
    pub death_reactions: Vec<crate::effect_reactions::Reaction>,
}

impl Items {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            carts: Vec::new(),
            bolts: Vec::new(),
            frames: Vec::new(),
            mobs: Vec::new(),
            sculk_catalysts: Vec::new(),
            sculk_deaths: Vec::new(),
            vibrations: Vec::new(),
            mob_view: Vec::new(),
            orbs: Vec::new(),
            orb_view: Vec::new(),
            xp_delay: std::collections::HashMap::new(),
            changes: Journal::new(),
            progress_kills: Vec::new(),
            death_reactions: Vec::new(),
        }
    }

    pub fn take_progress_kills(&mut self) -> Vec<([u8; 16], &'static str)> {
        std::mem::take(&mut self.progress_kills)
    }

    /// Ставит моба в мир: клиентам его покажет конец такта (`send_mob_ticks`).
    pub fn add_mob(&mut self, mob: crate::mob::Mob) {
        self.mobs.push(mob);
    }

    /// Переносит накопленные мобами события в журнал: в месте моба на
    /// этот миг. Добыча падает предметами прямо здесь, чтобы порядок журнала
    /// был как у оригинала: удар, звук, добыча, смерть. Зовётся после такта,
    /// удара игрока и команд.
    pub fn flush_mob_events(&mut self, next_id: &mut dyn FnMut() -> i32, rng: &mut crate::mob::Rng) {
        for index in 0..self.mobs.len() {
            let (entity_id, silent) = (self.mobs[index].entity_id, self.mobs[index].silent);
            let at = (self.mobs[index].x, self.mobs[index].y, self.mobs[index].z);

            for event in std::mem::take(&mut self.mobs[index].events) {
                if self.mobs[index].kind != crate::mob::Kind::Warden {
                    let frequency = match &event {
                        crate::mob::Event::Hurt { .. } => Some(7),
                        crate::mob::Event::Death => Some(15),
                        crate::mob::Event::Landing { .. } => Some(2),
                        crate::mob::Event::Sound { name, .. } if *name == self.mobs[index].kind.step_sound() => Some(1),
                        _ => None,
                    };
                    if let Some(frequency) = frequency {
                        self.vibrations.push(crate::redstone::sculk_family::GameEvent { at, source: Some(entity_id), frequency, projectile: false });
                    }
                }
                match event {
                    crate::mob::Event::Sound { .. } if silent => {}
                    crate::mob::Event::Loot(stacks) => {
                        for stack in stacks {
                            // Как у выпавшего из блока: скорость вверх 0,2, вбок
                            // случайная до 0,1 (чёрный ящик, cow-physics).
                            let velocity = (rng.unit() * 0.2 - 0.1, 0.2, rng.unit() * 0.2 - 0.1);

                            self.drop_item(next_id(), stack, at, velocity, crate::mob::DROP_DELAY);
                        }
                    }
                    event => {
                        // Опыт падает шариками после добычи, прямо перед
                        // событием смерти (чёрный ящик, cow-hit).
                        if matches!(event, crate::mob::Event::Death) {
                            let mob = &self.mobs[index];
                            if mob.xp_until > mob.ticks && let Some(uuid) = mob.kill_credit {
                                self.progress_kills.push((uuid, mob.kind.name()));
                            }
                            let points = std::mem::take(&mut self.mobs[index].pending_xp);

                            let catalyst = self.sculk_catalysts.iter().copied()
                                .filter(|pos| crate::mob::warden::distance(crate::redstone::sculk_family::center(*pos), at) <= 8.0)
                                .min_by(|a, b| crate::mob::warden::distance(crate::redstone::sculk_family::center(*a), at).total_cmp(&crate::mob::warden::distance(crate::redstone::sculk_family::center(*b), at)));
                            let charge = if catalyst.is_none() { 0 } else if points > 0 { points } else { self.mobs[index].sculk_xp(rng) };
                            if let Some(catalyst) = catalyst.filter(|_| charge > 0) {
                                self.sculk_deaths.push(crate::redstone::sculk_family::DeathCharge { catalyst, at, amount: charge });
                            } else { self.drop_experience(points, at, next_id, rng); }
                        }

                        self.changes.push(Change::MobEvent { entity_id, at, event });
                    }
                }
            }
        }
    }

    /// Снимок мобов на начало их такта: клиентам рассылка идёт по нему, как
    /// у оригинала, где слежение за сущностями стоит раньше их хода —
    /// конец такта показывает то, что было до хода мобов (чёрный ящик,
    /// cow-physics: скорость и сдвиг при падении отстают на один ход).
    pub fn snapshot_mobs(&mut self) {
        self.mob_view = self.mobs.clone();
        self.orb_view = self.orbs.clone();
    }

    /// Мобы такими, какими их видит слежение (`snapshot_mobs`).
    pub fn mob_view(&self) -> &[crate::mob::Mob] {
        &self.mob_view
    }

    /// Ставит моба, родившегося во время хода мобов: клиентам он виден уже
    /// в этом конце такта (появление), первая сверка — со следующего.
    pub fn add_mob_late(&mut self, mob: crate::mob::Mob) {
        self.mob_view.push(mob.clone());
        self.mobs.push(mob);
    }

    pub fn mobs(&self) -> &[crate::mob::Mob] {
        &self.mobs
    }

    pub fn mobs_mut(&mut self) -> &mut [crate::mob::Mob] {
        &mut self.mobs
    }

    /// Сам список мобов — чтобы на время вынуть его целиком.
    pub fn mobs_vec(&mut self) -> &mut Vec<crate::mob::Mob> {
        &mut self.mobs
    }

    /// Убирает мобов, которых выбирает `gone` (команда `kill`, пропавший труп,
    /// выгруженный чанк), и возвращает их. Клиентам уходит `Gone`.
    pub fn remove_mobs(&mut self, gone: impl FnMut(&mut crate::mob::Mob) -> bool) -> Vec<crate::mob::Mob> {
        let removed: Vec<crate::mob::Mob> = self.mobs.extract_if(.., gone).collect();

        for mob in &removed {
            self.changes.push(Change::Gone { entity_id: mob.entity_id });
            self.mob_view.retain(|seen| seen.entity_id != mob.entity_id);
        }

        removed
    }

    /// Ставит вагонетку в мир.
    pub fn add_cart(&mut self, cart: crate::minecart::Minecart) {
        self.changes.push(Change::Cart(Box::new(cart.clone())));
        self.carts.push(cart);
    }

    /// Ставит рамку в мир: клиенты узнают о ней сразу.
    pub fn add_frame(&mut self, frame: crate::frame::Frame) {
        self.changes.push(Change::Frame(frame.clone()));
        self.frames.push(frame);
    }

    /// Все рамки — в порядке появления.
    pub fn frames(&self) -> &[crate::frame::Frame] {
        &self.frames
    }

    pub fn frames_mut(&mut self) -> &mut [crate::frame::Frame] {
        &mut self.frames
    }

    /// Убирает рамки, которые выбирает `gone` (сломали, отвалились, убили
    /// командой), и возвращает их. Клиентам уходит `Gone`.
    pub fn remove_frames(&mut self, gone: impl FnMut(&mut crate::frame::Frame) -> bool) -> Vec<crate::frame::Frame> {
        let removed: Vec<crate::frame::Frame> = self.frames.extract_if(.., gone).collect();

        for frame in &removed {
            self.changes.push(Change::Gone { entity_id: frame.entity_id });
        }

        removed
    }

    /// Записывает в журнал событие рамки (свойство или звук).
    pub fn note_frame(&mut self, change: Change) {
        self.changes.push(change);
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
        let mut item = Self::new_item(entity_id, stack, at, (0.0, 0.0, 0.0), 0);

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
        delay: u64,
    ) {
        let dropped = Self::new_item(entity_id, stack, (x, y, z), velocity, delay);

        self.items.push(dropped.clone());
        self.changes.push(Change::Dropped(dropped));
    }

    /// Остаток /give: только получателю, без задержки подбора.
    pub fn give_remainder(&mut self, entity_id: i32, stack: Stack, at: (f64, f64, f64), velocity: (f64, f64, f64), owner: i32) {
        let mut item = Self::new_item(entity_id, stack, at, velocity, 0);
        item.owner = Some(owner);
        item.touchable = Some(at);
        self.items.push(item.clone());
        self.changes.push(Change::Dropped(item));
    }

    /// Визуальная сущность /give: в инвентаре уже есть выданное, повторно
    /// подбирать нельзя. [проверить] Анимация подбора при удалении в следующий такт.
    pub fn give_visual(&mut self, entity_id: i32, stack: Stack, at: (f64, f64, f64), velocity: (f64, f64, f64), owner: i32) {
        let mut item = Self::new_item(entity_id, stack, at, velocity, u64::MAX);
        item.age = LIFETIME - 1;
        item.owner = Some(owner);
        self.items.push(item.clone());
        self.changes.push(Change::Dropped(item));
    }

    pub fn drop_from_player(&mut self, entity_id: i32, stack: Stack, at: (f64, f64, f64), velocity: (f64, f64, f64), thrower: [u8; 16]) {
        let mut dropped = Self::new_item(entity_id, stack, at, velocity, THROWN_DELAY);
        dropped.thrower = Some(thrower);
        self.items.push(dropped.clone());
        self.changes.push(Change::Dropped(dropped));
    }

    /// Выпавший предмет, ещё не положенный в мир.
    fn new_item(
        entity_id: i32,
        stack: Stack,
        (x, y, z): (f64, f64, f64),
        velocity: (f64, f64, f64),
        delay: u64,
    ) -> Dropped {
        Dropped {
            entity_id,
            uuid: uuid_for(entity_id),
            stack,
            thrower: None,
            owner: None,
            ticks: 0,
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
            pickup_delay: delay,
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
            thrower: None,
            owner: None,
            ticks: 0,
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
            pickup_delay: 0,
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
            thrower: None,
            owner: None,
            ticks: 0,
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
            pickup_delay: 0,
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
        let mut merge_due = Vec::new();

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
            let cell = (item.x.floor(), item.y.floor(), item.z.floor());

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
                let cycle = (item.ticks as i64 + item.entity_id as i64).rem_euclid(ITEM_MOVE_EVERY) == 0;

                if let Some(splash) = item.step(solid, water, Some(cycle)) {
                    splashes.push(splash);
                }
            }

            let change = (item.vx - before.0).powi(2) + (item.vy - before.1).powi(2) + (item.vz - before.2).powi(2);

            item.impulse = change > IMPULSE;
            let moved_cell = cell != (item.x.floor(), item.y.floor(), item.z.floor());
            // Возраст считается уже с этого такта: первая проверка стоящего
            // предмета — на сороковом его такте (чёрный ящик item-merge:
            // предмет, возникший в такте 0, исчезает в такте 39).
            if (item.ticks + 1).is_multiple_of(if moved_cell { 2 } else { 40 }) {
                merge_due.push(item.entity_id);
            }
            item.ticks += 1;
            item.age += 1;
        }

        self.merge_items(&merge_due);

        // Пропадают провалившиеся в пустоту и пролежавшие слишком долго.
        let bottom = crate::world::MIN_Y as f64 - LOST_BELOW;
        let gone: Vec<Dropped> = self
            .items
            .extract_if(.., |item| item.y < bottom || (item.age >= LIFETIME && (item.pickup_delay == u64::MAX || !never_despawns(item))))
            .collect();

        for item in &gone {
            // Визуальная сущность /give пропадает без пакета подбора:
            // чёрный ящик give-pickup (оригинал шлёт только удаление).
            self.changes.push(Change::Gone { entity_id: item.entity_id });
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

    /// Кладёт в мир шарик опыта; клиентам он уходит по журналу.
    pub fn add_orb(&mut self, entity_id: i32, value: i32, at: (f64, f64, f64), velocity: (f64, f64, f64)) {
        let orb = crate::xp::XpOrb::new(entity_id, value, at, velocity);

        self.orbs.push(orb.clone());
        self.changes.push(Change::Orb(orb));
    }

    /// Роняет `points` очков опыта шариками (вики, Experience: жадно, от
    /// больших к малым) с толчком, как у падающих из моба (чёрный ящик,
    /// cow-xp): `x, z ∈ [−0,2; 0,2]`, `y ∈ [0; 0,4]`. `command` — призванный
    /// командой (без толчка).
    pub fn drop_experience(&mut self, points: i32, at: (f64, f64, f64), next_id: &mut dyn FnMut() -> i32, rng: &mut crate::mob::Rng) {
        for value in crate::survival::orb_values(points) {
            // Вики, Experience orb: новый шарик с вероятностью 1/40
            // добавляет подбор соседу той же стоимости. [проверить] Фаза RNG.
            let group = (rng.unit() * 40.0) as i32;
            if let Some(orb) = self.orbs.iter_mut().find(|orb| orb.value == value && !orb.expired()
                && orb.entity_id.rem_euclid(40) == group
                && (orb.x - at.0).abs() < 1.0 && (orb.y - at.1).abs() < 1.0 && (orb.z - at.2).abs() < 1.0)
            {
                orb.count = orb.count.saturating_add(1);
                orb.age = 0;
                continue;
            }
            let velocity = ((rng.unit() * 0.2 - 0.1) * 2.0, rng.unit() * 0.2 * 2.0, (rng.unit() * 0.2 - 0.1) * 2.0);

            self.add_orb(next_id(), value, at, velocity);
        }
    }

    pub fn orbs(&self) -> &[crate::xp::XpOrb] {
        &self.orbs
    }

    /// Шарики такими, какими их видит слежение (`snapshot_mobs`).
    pub fn orb_view(&self) -> &[crate::xp::XpOrb] {
        &self.orb_view
    }

    /// Убирает шарики, которые выбирает `gone` (команда `kill`).
    pub fn remove_orbs(&mut self, gone: impl FnMut(&mut crate::xp::XpOrb) -> bool) -> usize {
        let removed: Vec<crate::xp::XpOrb> = self.orbs.extract_if(.., gone).collect();

        for orb in &removed {
            self.changes.push(Change::Gone { entity_id: orb.entity_id });
        }

        removed.len()
    }

    /// Такт шариков: тяга к ближайшему из `players` (номер сущности и ноги),
    /// падение, старость, потом подбор: игрок берёт по одному шарику в
    /// 2 такта (`xp::TAKE_DELAY`), до которого дотянулся. Возвращает, кто
    /// сколько очков подобрал.
    pub fn step_orbs(&mut self, players: &[(i32, (f64, f64, f64))], solid: &dyn Fn(i32, i32, i32) -> bool) -> Vec<(i32, i32)> {
        let feet: Vec<(f64, f64, f64)> = players.iter().map(|&(_, feet)| feet).collect();

        for orb in &mut self.orbs {
            orb.step(crate::xp::nearest_pull((orb.x, orb.y, orb.z), &feet), solid);
        }

        self.merge_orbs();
        self.remove_orbs(|orb| orb.expired() || orb.y < -64.0);
        self.xp_delay.retain(|id, _| players.iter().any(|&(player, _)| player == *id));

        let mut gains = Vec::new();

        for &(id, at) in players {
            let delay = self.xp_delay.entry(id).or_insert(0);

            *delay = delay.saturating_sub(1);

            if *delay == 0
                && let Some(value) = self.take_orb(at, id)
            {
                self.xp_delay.insert(id, crate::xp::TAKE_DELAY);
                gains.push((id, value));
            }
        }

        gains
    }

    /// Игрок с ногами в `feet` подбирает один шарик, до которого дотянулся
    /// (по одному: паузу между ними ведёт игрок). Возвращает его очки.
    fn take_orb(&mut self, feet: (f64, f64, f64), by: i32) -> Option<i32> {
        let index = self.orbs.iter().position(|orb| orb.reached_by(feet))?;
        let value = self.orbs[index].value;
        let entity_id = self.orbs[index].entity_id;

        self.changes.push(Change::Taken { entity_id, by, count: 1, remaining: self.orbs[index].count > 1 });
        self.orbs[index].count -= 1;

        if self.orbs[index].count <= 0 {
            self.orbs.remove(index);
        }

        Some(value)
    }

    pub fn take_near<A>(&mut self, (x, y, z): (f64, f64, f64), by: i32, mut accept: A) -> Vec<Dropped>
    where
        A: FnMut(Stack) -> i32,
    {
        let mut taken = Vec::new();
        let mut index = 0;

        while index < self.items.len() {
            let item = &self.items[index];

            if item.block.is_some() || item.age < item.pickup_delay || item.owner.is_some_and(|owner| owner != by) || !item.reaches(x, y, z) {
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
                remaining: false,
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

                    if shove.bounce {
                        item.launch(shove.dir);
                    }
                } else if let Some(distance) = crate::redstone::drags(shove, item.on_ground, min, max) {
                    // Везомый мёдом так и лежит на нём: следующий шаг везёт
                    // его снова.
                    item.shove((shove.dir.0 * distance, shove.dir.1 * distance, shove.dir.2 * distance), solid);
                    item.on_ground = true;
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

    /// Выдвигаемая слизь подбросила: скорость по ходу — `LAUNCH`, прочие
    /// оси как были [проверить]. Клиентам вне очереди не сообщается: у
    /// оригинала подброшенный предмет шлёт скорость лишь в свой очередной
    /// отчёт раз в 20 тактов (slime-launch).
    fn launch(&mut self, dir: (f64, f64, f64)) {
        let launch = crate::redstone::LAUNCH;

        if dir.0 != 0.0 {
            self.vx = dir.0 * launch;
        } else if dir.1 != 0.0 {
            self.vy = dir.1 * launch;
        } else {
            self.vz = dir.2 * launch;
        }

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

        items.drop_item(1, stack(), at, velocity, 0);
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

        let head = crate::redstone::Shove { min: (0.5, 64.0, 0.0), max: (1.5, 65.0, 1.0), dir: (1.0, 0.0, 0.0), bounce: false, drag: false };
        items.shove(&[head], &floor);

        let item = &items.items()[0];
        assert!((item.x - 1.635).abs() < 1e-9, "до края головы и ещё 0,01: {}", item.x);
        assert!(!item.on_ground);
        assert_eq!((item.vx, item.vy, item.vz), (0.0, 0.0, 0.0));

        items.step(&floor, &floor, &loaded);
        assert!(items.items()[0].on_ground);
        assert_eq!(items.items()[0].y, 64.0);
    }

    /// Выдвигаемая слизь подбрасывает предмет со скоростью 1,0 по ходу, и
    /// это не толчок вне очереди; без тяжести скорость дальше гаснет по 0,98
    /// за такт — через 10 тактов 0,817, как у оригинала (slime-launch).
    #[test]
    fn a_pushed_slime_block_launches_an_item() {
        let mut items = Items::new();

        items.watch_changes(READER);
        items.summon_item(1, stack(), (0.5, 66.0, 0.5), true);

        let slime = crate::redstone::Shove { min: (0.0, 65.5, 0.0), max: (1.0, 66.5, 1.0), dir: (0.0, 1.0, 0.0), bounce: true, drag: false };
        items.shove(&[slime], &floor);

        let item = &items.items()[0];
        assert!((item.y - 66.51).abs() < 1e-9, "{}", item.y);
        assert_eq!(item.vy, 1.0);

        for _ in 0..10 {
            items.step(&floor, &floor, &loaded);
            assert!(!items.items()[0].impulse);
        }

        assert!((items.items()[0].vy - 0.98_f64.powi(10)).abs() < 1e-3, "{}", items.items()[0].vy);
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

        items.drop_item(7, stack(), (0.5, 64.0, 0.5), (0.0, 0.0, 0.0), 0);

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

        items.drop_item(1, stack(), (0.5, 64.0, 0.5), still, 0);
        items.drop_item(2, stack(), (10.0, 64.0, 0.5), still, 0);
        items.drop_item(3, stack(), (0.5, 70.0, 0.5), still, 0);

        items.note_tick_start();
        let taken = items.take_near((0.0, 64.0, 0.0), 42, anything);

        assert_eq!(taken.len(), 1);
        assert_eq!(taken[0].entity_id, 1);
        assert_eq!(items.items().len(), 2);

        // В журнале записано, кто поднял и сколько.
        let (changes, _) = items.changes_since(3, READER);

        assert!(matches!(
            changes.first(),
            Some(Change::Taken { entity_id: 1, by: 42, count: 3, .. })
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
        items.items[0].age = LIFETIME;

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

        items.drop_item(1, stack(), (0.5, 64.0, 0.5), (0.0, 0.0, 0.0), 0);

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

        items.drop_item(1, stack(), (0.5, 64.0, 0.5), (0.0, 0.0, 0.0), 0);

        items.note_tick_start();
        let taken = items.take_near((0.5, 64.0, 0.5), 42, |stack| stack.count.min(2));

        assert_eq!(taken.len(), 1);
        assert_eq!(taken[0].stack.count, 2);
        assert_eq!(items.items().len(), 1);
        assert_eq!(items.items()[0].stack.count, 1);
        assert_eq!(items.items()[0].entity_id, 1);

        // Клиентам: подбор двух штук и остаток, появившийся снова.
        let (changes, _) = items.changes_since(1, READER);

        assert!(matches!(changes.first(), Some(Change::Taken { entity_id: 1, by: 42, count: 2, .. })));
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

    fn one_id() -> impl FnMut() -> i32 {
        let mut next = 100;

        move || {
            next += 1;
            next
        }
    }

    #[test]
    fn experience_falls_as_greedy_orbs_in_the_journal() {
        let mut items = Items::new();
        let reader = 1;
        let from = items.watch_changes(reader);

        items.drop_experience(5, (0.5, 0.0, 0.5), &mut one_id(), &mut crate::mob::Rng::new(7));

        let values: Vec<i32> = items.orbs().iter().map(|orb| orb.value).collect();
        let (changes, _) = items.changes_since(from, reader);

        assert_eq!(values, vec![3, 1, 1]);
        assert_eq!(changes.iter().filter(|change| matches!(change, Change::Orb(_))).count(), 3);
        // Толчок как у оригинала: по сторонам до 0,2, вверх до 0,4.
        assert!(items.orbs().iter().all(|orb| orb.vx.abs() <= 0.2 && orb.vz.abs() <= 0.2 && (0.0..=0.4).contains(&orb.vy)));
    }

    #[test]
    fn a_player_takes_one_orb_per_two_ticks_and_everyone_sees_it() {
        let mut items = Items::new();
        let reader = 1;
        let from = items.watch_changes(reader);
        let solid = |_: i32, y: i32, _: i32| y < 0;

        for id in 1..=3 {
            items.add_orb(id, 1, (0.5, 0.0, 0.5), (0.0, 0.0, 0.0));
        }

        let players = [(50, (0.5, 0.0, 0.5))];
        let taken: Vec<usize> = (0..6).map(|_| items.step_orbs(&players, &solid).len()).collect();

        // Первый — сразу, потом через такт: паузы по 2 такта.
        assert_eq!(taken, vec![1, 0, 1, 0, 1, 0]);
        assert!(items.orbs().is_empty());

        let (changes, _) = items.changes_since(from, reader);
        let takes = changes.iter().filter(|change| matches!(change, Change::Taken { by: 50, count: 1, .. })).count();

        assert_eq!(takes, 3);
    }

    #[test]
    fn the_nearest_player_gets_the_orb_and_a_far_one_nothing() {
        let mut items = Items::new();
        let solid = |_: i32, y: i32, _: i32| y < 0;

        items.add_orb(1, 7, (0.5, 0.0, 0.5), (0.0, 0.0, 0.0));

        let gains = items.step_orbs(&[(10, (30.5, 0.0, 0.5)), (11, (0.5, 0.0, 0.5))], &solid);

        assert_eq!(gains, vec![(11, 7)]);
    }

    #[test]
    fn an_untouched_orb_disappears_after_6000_ticks() {
        let mut items = Items::new();
        let solid = |_: i32, y: i32, _: i32| y < 0;

        items.add_orb(1, 1, (0.5, 0.0, 0.5), (0.0, 0.0, 0.0));

        for _ in 0..5999 {
            items.step_orbs(&[], &solid);
        }

        assert_eq!(items.orbs().len(), 1);
        items.step_orbs(&[], &solid);
        assert!(items.orbs().is_empty());
    }

    #[test]
    fn a_kill_of_a_credited_mob_drops_orbs_before_its_death() {
        let mut items = Items::new();
        let reader = 1;
        let from = items.watch_changes(reader);
        let mut cow = crate::mob::Mob::new(1, crate::mob::Kind::Cow, (0.5, 0.0, 0.5));

        cow.credit_player();
        cow.hurt(f32::MAX, crate::survival::Source::PlayerAttack, None);
        items.add_mob(cow);
        items.flush_mob_events(&mut one_id(), &mut crate::mob::Rng::new(3));

        let (changes, _) = items.changes_since(from, reader);
        let order: Vec<char> = changes
            .iter()
            .map(|change| match change {
                Change::Dropped(_) => 'D',
                Change::Orb(_) => 'O',
                Change::MobEvent { event: crate::mob::Event::Death, .. } => 'X',
                _ => '.',
            })
            .collect();
        let points: i32 = items.orbs().iter().map(|orb| orb.value).sum();

        assert!((1..=3).contains(&points), "{points}");
        assert!(order.iter().position(|c| *c == 'O') > order.iter().position(|c| *c == 'D'));
        assert!(order.iter().position(|c| *c == 'O') < order.iter().position(|c| *c == 'X'));
    }

    #[test]
    fn closest_catalyst_absorbs_xp_even_without_player_credit() {
        for credited in [false, true] {
            let mut items = Items::new();
            items.sculk_catalysts = vec![(7, 0, 0), (2, 0, 0), (20, 0, 0)];
            let mut cow = crate::mob::Mob::new(1, crate::mob::Kind::Cow, (0.5, 0.5, 0.5));
            if credited { cow.credit_player(); }
            cow.hurt(f32::MAX, crate::survival::Source::GenericKill, None);
            items.add_mob(cow);
            items.flush_mob_events(&mut one_id(), &mut crate::mob::Rng::new(3));
            assert!(items.orbs().is_empty());
            assert_eq!(items.sculk_deaths.len(), 1);
            assert_eq!(items.sculk_deaths[0].catalyst, (2, 0, 0));
            assert!((1..=3).contains(&items.sculk_deaths[0].amount));
        }
    }

    #[test]
    fn catalyst_outside_eight_blocks_does_not_swallow_orbs() {
        let mut items = Items::new();
        items.sculk_catalysts = vec![(9, 0, 0)];
        let mut cow = crate::mob::Mob::new(1, crate::mob::Kind::Cow, (0.5, 0.5, 0.5));
        cow.credit_player();
        cow.hurt(f32::MAX, crate::survival::Source::GenericKill, None);
        items.add_mob(cow);
        items.flush_mob_events(&mut one_id(), &mut crate::mob::Rng::new(3));
        assert!(items.sculk_deaths.is_empty());
        assert!(!items.orbs().is_empty());
    }
}

#[cfg(test)]
mod progress_tests {
    use super::*;
    use crate::mob::{Kind, Mob, Rng};
    use crate::survival::Source;

    #[test]
    fn kills_are_credited_once_across_melee_projectiles_and_delayed_death() {
        for kind in [Kind::Cow, Kind::Zombie, Kind::Wither, Kind::EnderDragon] {
            for source in [Source::PlayerAttack, Source::Arrow, Source::Fall] {
                if kind == Kind::EnderDragon && source == Source::Fall { continue; }
                let mut mob = Mob::new(1, kind, (0.0, 64.0, 0.0));
                mob.credit_to([1; 16]);
                mob.hurt(10000.0, source, None);
                assert!(mob.is_dead(), "{kind:?} {source:?}");
                let mut items = Items::new();
                items.add_mob(mob);
                let mut id = 100;
                items.flush_mob_events(&mut || { id += 1; id }, &mut Rng::new(1));
                assert_eq!(items.take_progress_kills(), vec![([1; 16], kind.name())]);
                items.flush_mob_events(&mut || { id += 1; id }, &mut Rng::new(1));
                assert!(items.take_progress_kills().is_empty());
            }
        }
    }

    #[test]
    fn expired_credit_does_not_award_a_kill() {
        let mut mob = Mob::new(1, Kind::Cow, (0.0, 64.0, 0.0));
        mob.credit_to([1; 16]);
        mob.ticks = mob.xp_until;
        mob.hurt(100.0, Source::Fall, None);
        let mut items = Items::new();
        items.add_mob(mob);
        items.flush_mob_events(&mut || 100, &mut Rng::new(1));
        assert!(items.take_progress_kills().is_empty());
    }
}
