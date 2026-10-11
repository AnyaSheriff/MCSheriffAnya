// Инвентарь игрока.
//
// Инвентарь — это набор слотов, и живёт он на сервере. Клиенту он показывается
// целиком при входе в мир и переписывается заново после каждого щелчка, чтобы
// картинка у игрока и то, что знает сервер, не разошлись.
//
// Слоты нумеруются так, как их нумерует протокол: 0 — итог верстака в окне
// инвентаря, 1–4 — его сетка, 5–8 — броня, 9–35 — рюкзак, 36–44 — панель
// быстрого доступа, 45 — вторая рука. Эта нумерация общая и для пакетов,
// и для файла игрока на диске.
//
// Отдельно от слотов лежит «предмет в курсоре» — то, что игрок держит мышкой
// в открытом окне инвентаря. Он тоже часть состояния: пока окно открыто,
// предмет не лежит ни в одном слоте.

use crate::blocks;

/// Сколько всего слотов у инвентаря игрока.
pub const SLOTS: usize = 46;

/// Первый и последний слот рюкзака.
pub const FIRST_MAIN: i32 = 9;
pub const LAST_MAIN: i32 = 35;

/// Первый и последний слот панели быстрого доступа.
pub const FIRST_HOTBAR: i32 = 36;
pub const LAST_HOTBAR: i32 = 44;

/// Слот второй руки.
pub const OFFHAND: i32 = 45;

/// Первый слот сетки верстака в окне инвентаря: сетка — слоты 1–4.
const CRAFTING_GRID: i32 = 1;

/// Номер слота, который означает «щелчок мимо окна» — за его пределами.
pub const OUTSIDE: i32 = -999;

/// Кнопка обмена со второй рукой при щелчке с цифрой.
pub const SWAP_OFFHAND: i8 = 40;

/// Чем кончился «выбор блока».
#[derive(Clone, PartialEq, Debug)]
pub struct Picked {
    /// Слоты, которые изменились: их надо показать клиенту.
    pub slots: Vec<i32>,

    /// Новый выбранный слот панели, если рука сменила слот.
    pub selected: Option<i32>,
}

/// Чем кончилось закрытие окна инвентаря.
#[derive(Clone, PartialEq, Debug, Default)]
pub struct Closed {
    /// Слоты, которые изменились: их надо показать клиенту.
    pub slots: Vec<i32>,

    /// Что не поместилось в инвентарь: это должно упасть на землю.
    pub thrown: Vec<Stack>,
}

/// Чем кончился щелчок по инвентарю.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Click {
    /// Понял ли сервер этот щелчок. False означает, что инвентарь не тронут:
    /// вызывающий просто перешлёт клиенту прежнее содержимое.
    pub understood: bool,

    /// Что игрок выбросил этим щелчком: это должно упасть на землю.
    pub thrown: Option<Stack>,
}

/// Стопка предметов в слоте.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Stack {
    /// Номер предмета.
    pub item: i32,

    /// Сколько их в стопке.
    pub count: i32,

    /// Особый динамит (custom_tnt.rs): какой именно. У обычных предметов —
    /// None. Стопки с разной пометкой не сливаются.
    pub tnt: Option<crate::custom_tnt::KindId>,

    /// Довески: износ, зачарования, свои модификаторы (stackdata.rs). У
    /// обычных предметов — None. Стопки с разными довесками не сливаются.
    pub data: Option<crate::stackdata::DataId>,
}

impl Stack {
    pub fn new(item: i32, count: i32) -> Self {
        Self { item, count, tnt: None, data: None }
    }

    /// Довески стопки (пустые, если их нет).
    pub fn extras(&self) -> crate::stackdata::Data {
        self.data.map(crate::stackdata::get).unwrap_or_default()
    }

    /// Та же стопка с другими довесками.
    pub fn with_extras(self, data: crate::stackdata::Data) -> Self {
        Self { data: crate::stackdata::intern(data), ..self }
    }

    /// Та же стопка другого размера — с теми же пометками.
    pub fn with_count(self, count: i32) -> Self {
        Self { count, ..self }
    }

    /// Сливаются ли стопки: тот же предмет и те же пометки.
    pub fn stacks_with(&self, other: &Stack) -> bool {
        self.item == other.item && self.tnt == other.tnt && self.data == other.data
    }

    /// Номер предмета для файла (игрока, хранилища): в старшем байте —
    /// номер особого динамита, у обычных предметов там ноль, и запись та же,
    /// что прежде.
    pub fn stored_item(&self) -> i32 {
        // Старший бит — «за номером идут довески» (`write_stored`); на
        // номер особого динамита остаются семь бит.
        let flag = if self.data.is_some() { i32::MIN } else { 0 };

        self.item | self.tnt.map_or(0, |kind| i32::from(kind.0 & 0x7f) << 24) | flag
    }

    /// Дописывает довески стопки, если они есть: длина и запись
    /// (`stackdata::to_bytes`). Читает их `read_stored`.
    pub fn write_extras(&self, out: &mut Vec<u8>) {
        if let Some(data) = self.data {
            let bytes = crate::stackdata::to_bytes(&crate::stackdata::get(data));

            out.extend_from_slice(&(bytes.len() as u16).to_be_bytes());
            out.extend_from_slice(&bytes);
        }
    }

    /// Довески к стопке из `from_stored`, если её номер их обещал: берёт их
    /// из `take` (длина и запись). None — запись испорчена.
    pub fn read_extras<'a>(stored: i32, mut take: impl FnMut(usize) -> Option<&'a [u8]>) -> Option<Option<crate::stackdata::Data>> {
        if stored >= 0 {
            return Some(None);
        }

        let length = usize::from(u16::from_be_bytes(take(2)?.try_into().ok()?));

        Some(Some(crate::stackdata::from_bytes(take(length)?)?))
    }

    /// Стопка из записи `stored_item`. Неизвестный номер особого динамита
    /// (вид убрали из таблицы) — обычный динамит.
    pub fn from_stored(stored: i32, count: i32) -> Self {
        let tnt = crate::custom_tnt::KindId(((stored >> 24) & 0x7f) as u8);

        Self {
            item: stored & 0x00ff_ffff,
            count,
            tnt: crate::custom_tnt::by_id(tnt).map(|kind| kind.id),
            data: None,
        }
    }

    /// Сколько таких предметов помещается в одну стопку.
    pub fn limit(&self) -> i32 {
        blocks::stack_size(self.item)
    }
}

/// Инвентарь игрока.
#[derive(Clone, PartialEq, Debug)]
pub struct Inventory {
    slots: [Option<Stack>; SLOTS],

    /// Выбранный слот панели быстрого доступа, от 0 до 8.
    selected: usize,

    /// Предмет в курсоре: то, что игрок держит мышкой.
    cursor: Option<Stack>,

    /// Атрибуты, которые у игрока когда-либо были с модификаторами: при
    /// возрождении оригинал шлёт их все (у оригинала они живут в файле
    /// игрока). Лежат хвостом записи инвентаря — своего места в файле игрока
    /// у них нет.
    seen_attributes: Vec<u8>,
}

impl Inventory {
    pub fn new() -> Self {
        Self {
            slots: [None; SLOTS],
            selected: 0,
            cursor: None,
            seen_attributes: Vec::new(),
        }
    }

    /// Атрибуты, что когда-либо имели модификаторы (по возрастанию номеров).
    pub fn seen_attributes(&self) -> &[u8] {
        &self.seen_attributes
    }

    /// Запоминает атрибут, у которого были модификаторы.
    pub fn note_attribute(&mut self, attribute: u8) {
        if let Err(at) = self.seen_attributes.binary_search(&attribute) {
            self.seen_attributes.insert(at, attribute);
        }
    }

    /// Все слоты по порядку — для отправки клиенту.
    pub fn slots(&self) -> &[Option<Stack>] {
        &self.slots
    }

    /// Что лежит в слоте.
    pub fn slot(&self, slot: i32) -> Option<Stack> {
        usize::try_from(slot)
            .ok()
            .and_then(|slot| self.slots.get(slot))
            .copied()
            .flatten()
    }

    /// Кладёт в слот то, что там должно лежать.
    pub fn set(&mut self, slot: i32, stack: Option<Stack>) {
        if let Ok(slot) = usize::try_from(slot)
            && slot < SLOTS
        {
            // Больше предела стопки в слоте не бывает: клиент или испорченный
            // файл могут прислать любое число, а дальше с ним считают.
            self.slots[slot] = stack
                .filter(|stack| stack.count > 0)
                .map(|stack| stack.with_count(stack.count.min(stack.limit())));
        }
    }

    /// Предмет в курсоре.
    pub fn cursor(&self) -> Option<Stack> {
        self.cursor
    }

    /// Кладёт предмет в курсор — для окон хранилищ (window.rs), где щелчок
    /// разбирается поверх двух наборов слотов сразу.
    pub fn set_cursor(&mut self, stack: Option<Stack>) {
        self.cursor = stack.filter(|stack| stack.count > 0);
    }

    /// Закрывает окно инвентаря: то, что игрок держал мышкой, и то, что
    /// лежало в сетке верстака (слоты 1–4), возвращается в инвентарь.
    ///
    /// Зовётся, когда игрок закрывает окно и когда уходит из игры: ни курсор,
    /// ни сетка не должны съедать предметы. Что не поместилось, отдаётся
    /// наружу — вызывающий бросает это на землю, как в игре при полном
    /// рюкзаке.
    pub fn close_window(&mut self) -> Closed {
        let mut closed = Closed::default();
        if self.slots[0].take().is_some() { closed.slots.push(0); }
        let mut returned: Vec<Stack> = self.cursor.take().into_iter().collect();

        for slot in CRAFTING_GRID..CRAFTING_GRID + 4 {
            if let Some(stack) = self.slots[slot as usize].take() {
                closed.slots.push(slot);
                returned.push(stack);
            }
        }

        for stack in returned {
            let (slots, left) = self.put(stack);

            closed.slots.extend(slots);
            closed.thrown.extend(left);
        }

        closed.slots.sort_unstable();
        closed.slots.dedup();
        closed
    }

    /// Запоминает выбранный слот панели.
    pub fn select(&mut self, slot: i32) {
        if let Ok(slot) = usize::try_from(slot)
            && slot < 9
        {
            self.selected = slot;
        }
    }

    /// Выбранный слот панели.
    pub fn selected(&self) -> i32 {
        self.selected as i32
    }

    /// Что у игрока в руке.
    pub fn held(&self) -> Option<Stack> {
        self.slots[FIRST_HOTBAR as usize + self.selected]
    }

    /// Номер слота, который выбран в руке (окно игрока: 36–44).
    pub fn selected_slot(&self) -> i32 {
        FIRST_HOTBAR + self.selected as i32
    }

    /// Убирает из руки одну штуку — так расходуется поставленный блок.
    ///
    /// Возвращает номер слота, если что-то изменилось: клиенту нужно послать
    /// именно этот слот.
    pub fn spend_held(&mut self) -> Option<i32> {
        let slot = FIRST_HOTBAR as usize + self.selected;
        let stack = self.slots[slot].as_mut()?;

        stack.count -= 1;

        if stack.count <= 0 {
            self.slots[slot] = None;
        }

        Some(slot as i32)
    }

    /// Кладёт предметы в инвентарь: сперва в начатые стопки того же предмета,
    /// потом в первое пустое место. Сначала панель, потом рюкзак — так вещь
    /// сразу попадает под руку.
    ///
    /// Возвращает слоты, которые изменились, — их надо показать клиенту, —
    /// и то, что не поместилось: его вызывающий оставляет лежать или бросает
    /// на землю.
    pub fn put(&mut self, stack: Stack) -> (Vec<i32>, Option<Stack>) {
        let mut left = stack.count;
        let mut changed = Vec::new();

        let order: Vec<i32> = (FIRST_HOTBAR..=LAST_HOTBAR)
            .chain(FIRST_MAIN..=LAST_MAIN)
            .collect();

        // Сперва доложить в начатые стопки.
        for slot in &order {
            if left <= 0 {
                break;
            }

            let index = *slot as usize;

            if let Some(there) = self.slots[index].as_mut()
                && there.stacks_with(&stack)
            {
                let room = (there.limit() - there.count).max(0);
                let moved = room.min(left);

                if moved > 0 {
                    there.count += moved;
                    left -= moved;
                    changed.push(*slot);
                }
            }
        }

        // Потом занять пустые слоты.
        for slot in &order {
            if left <= 0 {
                break;
            }

            let index = *slot as usize;

            if self.slots[index].is_none() {
                let moved = left.min(blocks::stack_size(stack.item));

                self.slots[index] = Some(stack.with_count(moved));
                left -= moved;
                changed.push(*slot);
            }
        }

        (changed, remainder(stack, left))
    }

    /// Находит предмет в инвентаре: сперва в панели, потом в рюкзаке.
    ///
    /// Возвращает номер слота. Нужно для «выбора блока»: если предмет уже
    /// есть, игроку дают его, а не заводят новый.
    pub fn find(&self, item: i32) -> Option<i32> {
        (FIRST_HOTBAR..=LAST_HOTBAR)
            .chain(FIRST_MAIN..=LAST_MAIN)
            .find(|slot| self.slot(*slot).is_some_and(|stack| stack.item == item))
    }

    /// «Выбор блока»: предмет попадает в руку.
    ///
    /// Правило с вики (страница Controls, Pick Block): если предмет есть
    /// в панели — рука просто переводится на этот слот; если он в рюкзаке —
    /// переезжает в панель; если свободного слота в панели нет, заменяется
    /// выбранный.
    ///
    /// Предмет из рюкзака меняется с содержимым слота местами, поэтому
    /// не теряется. А выданный в творческом заменяет то, что было в руке,
    /// — так и сказано на вики.
    ///
    /// `found` — где предмет уже лежит, если лежит.
    pub fn pick(&mut self, item: i32, found: Option<i32>) -> Picked {
        // Уже в панели — ничего не двигаем, только переводим руку.
        if let Some(slot) = found
            && (FIRST_HOTBAR..=LAST_HOTBAR).contains(&slot)
        {
            self.selected = (slot - FIRST_HOTBAR) as usize;

            return Picked {
                slots: Vec::new(),
                selected: Some(self.selected()),
            };
        }

        let target = self.free_hotbar_slot().unwrap_or(FIRST_HOTBAR + self.selected as i32);
        let mut slots = vec![target];

        match found {
            // Лежит в рюкзаке — меняем местами: так ничего не теряется.
            Some(slot) => {
                let taken = self.slot(slot);
                let held = self.slot(target);

                self.set(target, taken);
                self.set(slot, held);
                slots.push(slot);
            }
            // Нет вовсе — в творческом игра просто выдаёт предмет, заменяя
            // то, что лежало в этом слоте.
            None => self.set(target, Some(Stack::new(item, 1))),
        }

        self.selected = (target - FIRST_HOTBAR) as usize;

        Picked {
            slots,
            selected: Some(self.selected()),
        }
    }

    /// Пустой слот панели: сперва тот, что в руке, потом остальные по порядку.
    fn free_hotbar_slot(&self) -> Option<i32> {
        let held = FIRST_HOTBAR + self.selected as i32;

        std::iter::once(held)
            .chain(FIRST_HOTBAR..=LAST_HOTBAR)
            .find(|slot| self.slot(*slot).is_none())
    }

    /// Щелчок по инвентарю.
    ///
    /// `mode` — вид щелчка: 0 обычный, 1 с Shift, 2 с цифрой, 4 выброс.
    /// `button` — кнопка мыши или номер слота панели при щелчке с цифрой.
    ///
    /// false означает, что такой щелчок сервер разобрать не умеет. Тогда
    /// вызывающий просто перешлёт клиенту содержимое инвентаря, каким оно у
    /// сервера было: пусть картинка вернётся назад, но не разойдётся.
    pub fn click(&mut self, slot: i32, button: i8, mode: i32) -> Click {
        self.click_in_mode(slot, button, mode, false)
    }

    /// Вики, Curse of Binding: «cannot be removed from any armor slot».
    /// Проверяется итог всех способов переноса, включая цифру и выброс;
    /// неудачный перенос не меняет курсор и не создаёт выпавшую вещь.
    pub fn click_in_mode(&mut self, slot: i32, button: i8, mode: i32, creative: bool) -> Click {
        let bound: Vec<i32> = (5..=8).filter(|slot| !creative && self.slot(*slot).is_some_and(|stack| stack.extras().level("binding_curse") > 0)).collect();
        let before = (!bound.is_empty()).then(|| self.clone());
        let result = self.click_unchecked(slot, button, mode);
        if let Some(before) = before
            && bound.into_iter().any(|slot| self.slot(slot) != before.slot(slot))
        {
            *self = before;
            return Click { understood: true, thrown: None };
        }
        result
    }

    fn click_unchecked(&mut self, slot: i32, button: i8, mode: i32) -> Click {
        match mode {
            0 => self.pick_up(slot, button),
            1 => Click { understood: self.quick_move(slot), thrown: None },
            2 => Click { understood: self.swap(slot, button), thrown: None },
            4 => self.throw_away(slot, button),
            _ => Click::default(),
        }
    }

    /// Обычный щелчок: взять стопку в курсор, положить её или поменять местами.
    fn pick_up(&mut self, slot: i32, button: i8) -> Click {
        // Щелчок мимо окна: то, что в курсоре, летит на землю.
        if slot == OUTSIDE {
            let thrown = match (self.cursor, button) {
                (None, _) => None,
                (Some(held), 0) => {
                    self.cursor = None;
                    Some(held)
                }
                (Some(held), _) => {
                    self.cursor = take_one(self.cursor);
                    Some(held.with_count(1))
                }
            };

            return Click { understood: true, thrown };
        }

        let Some(index) = self.index(slot) else {
            return Click::default();
        };

        if index != 0 && crate::bundle::click(&mut self.cursor, &mut self.slots[index], button) {
            return Click { understood: true, thrown: None };
        }
        let there = self.slots[index];

        match (self.cursor, there, button) {
            // Курсор пуст — забираем стопку целиком или половину.
            (None, Some(stack), 0) => {
                self.cursor = Some(stack);
                self.slots[index] = None;
            }
            (None, Some(stack), _) => {
                // Половина с округлением вверх — без `count + 1`, которое
                // переполнилось бы на самом большом числе.
                let taken = stack.count - stack.count / 2;

                self.cursor = Some(stack.with_count(taken));
                self.slots[index] = remainder(stack, stack.count - taken);
            }
            (None, None, _) => {}

            // В курсоре что-то есть, слот пуст — кладём всё или одну штуку.
            (Some(held), None, 0) => {
                self.slots[index] = Some(held);
                self.cursor = None;
            }
            (Some(held), None, _) => {
                self.slots[index] = Some(held.with_count(1));
                self.cursor = take_one(Some(held));
            }

            // Тот же предмет — досыпаем, сколько влезет.
            (Some(held), Some(there), button) if held.stacks_with(&there) => {
                let room = (there.limit() - there.count).max(0);
                let moved = if button == 0 { room.min(held.count) } else { room.min(1) };

                self.slots[index] = Some(there.with_count(there.count.saturating_add(moved)));
                self.cursor = remainder(held, held.count - moved);
            }

            // Разные предметы — меняем местами.
            (Some(held), Some(there), _) => {
                self.slots[index] = Some(held);
                self.cursor = Some(there);
            }
        }

        Click { understood: true, thrown: None }
    }

    /// Щелчок с Shift: стопка перекладывается между рюкзаком и панелью.
    fn quick_move(&mut self, slot: i32) -> bool {
        let Some(index) = self.index(slot) else {
            return false;
        };

        let Some(stack) = self.slots[index] else {
            return true;
        };

        // Из панели вещь уходит в рюкзак, из рюкзака — в панель, из брони
        // и второй руки — куда придётся.
        let targets: Vec<i32> = match slot {
            FIRST_HOTBAR..=LAST_HOTBAR => (FIRST_MAIN..=LAST_MAIN).collect(),
            FIRST_MAIN..=LAST_MAIN => (FIRST_HOTBAR..=LAST_HOTBAR).collect(),
            _ => (FIRST_MAIN..=LAST_MAIN)
                .chain(FIRST_HOTBAR..=LAST_HOTBAR)
                .collect(),
        };

        self.slots[index] = None;

        let mut left = stack.count;

        // Сперва досыпаем в начатые стопки того же предмета.
        for target in &targets {
            if left <= 0 {
                break;
            }

            if let Some(there) = self.slots[*target as usize].as_mut()
                && there.stacks_with(&stack)
            {
                let moved = (there.limit() - there.count).max(0).min(left);

                there.count += moved;
                left -= moved;
            }
        }

        // Потом занимаем пустые слоты.
        for target in &targets {
            if left <= 0 {
                break;
            }

            let target = *target as usize;

            if self.slots[target].is_none() {
                let moved = left.min(blocks::stack_size(stack.item));

                self.slots[target] = Some(stack.with_count(moved));
                left -= moved;
            }
        }

        // Что не поместилось, остаётся на месте.
        self.slots[index] = remainder(stack, left);

        true
    }

    /// Щелчок с цифрой: слот меняется местами со слотом панели (или со второй
    /// рукой, если нажата клавиша второй руки).
    fn swap(&mut self, slot: i32, button: i8) -> bool {
        let other = match button {
            SWAP_OFFHAND => OFFHAND,
            0..=8 => FIRST_HOTBAR + button as i32,
            _ => return false,
        };

        let (Some(here), Some(there)) = (self.index(slot), self.index(other)) else {
            return false;
        };

        self.slots.swap(here, there);

        true
    }

    /// Выброс: предмет из слота летит на землю — одна штука или вся стопка.
    fn throw_away(&mut self, slot: i32, button: i8) -> Click {
        let Some(index) = self.index(slot) else {
            return Click::default();
        };

        let Some(there) = self.slots[index] else {
            return Click { understood: true, thrown: None };
        };

        let thrown = match button {
            0 => {
                self.slots[index] = take_one(Some(there));
                there.with_count(1)
            }
            _ => {
                self.slots[index] = None;
                there
            }
        };

        Click { understood: true, thrown: Some(thrown) }
    }

    /// Номер слота как место в списке. None — такого слота нет.
    fn index(&self, slot: i32) -> Option<usize> {
        usize::try_from(slot).ok().filter(|slot| *slot < SLOTS)
    }

    /// Записывает инвентарь для файла игрока.
    ///
    /// Пустые слоты не пишутся: их подавляющее большинство, и незачем занимать
    /// ими место. Поэтому запись — это список «слот, предмет, сколько».
    pub fn to_bytes(&self) -> Vec<u8> {
        let filled: Vec<(usize, Stack)> = self
            .slots
            .iter()
            .enumerate()
            .filter(|(slot, _)| *slot != 0)
            .filter_map(|(slot, stack)| stack.map(|stack| (slot, stack)))
            .collect();

        let mut out = Vec::with_capacity(2 + filled.len() * 9);

        out.push(self.selected as u8);
        out.push(filled.len() as u8);

        for (slot, stack) in filled {
            out.push(slot as u8);
            out.extend_from_slice(&stack.stored_item().to_be_bytes());
            out.extend_from_slice(&stack.count.to_be_bytes());
            stack.write_extras(&mut out);
        }

        // Хвост: сколько и какие атрибуты когда-либо имели модификаторы.
        if !self.seen_attributes.is_empty() {
            out.push(self.seen_attributes.len() as u8);
            out.extend_from_slice(&self.seen_attributes);
        }

        out
    }

    /// Читает инвентарь из файла игрока. None — запись испорчена.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let selected = *bytes.first()?;
        let filled = *bytes.get(1)? as usize;

        let mut inventory = Inventory::new();
        inventory.select(selected as i32);

        let mut at = 2;

        for _ in 0..filled {
            let mut take = |count: usize| -> Option<&[u8]> {
                let part = bytes.get(at..at + count)?;

                at += count;

                Some(part)
            };
            let slot = take(1)?[0] as i32;
            let item = i32::from_be_bytes(take(4)?.try_into().ok()?);
            let count = i32::from_be_bytes(take(4)?.try_into().ok()?);
            let extras = Stack::read_extras(item, &mut take)?;
            let mut stack = Stack::from_stored(item, count);

            if let Some(extras) = extras {
                stack = stack.with_extras(extras);
            }

            if count > 0 && slot != 0 {
                inventory.set(slot, Some(stack));
            }
        }

        // Хвост записи (у старых файлов его нет).
        if let Some(&number) = bytes.get(at) {
            for id in bytes.get(at + 1..at + 1 + usize::from(number)).unwrap_or_default() {
                inventory.note_attribute(*id);
            }
        }

        Some(inventory)
    }
}

/// Стопка, из которой убрали одну штуку. None — стопка кончилась.
pub fn take_one(stack: Option<Stack>) -> Option<Stack> {
    let stack = stack?;

    remainder(stack, stack.count - 1)
}

/// Та же стопка, но другого размера. None — в ней ничего не осталось.
pub fn remainder(stack: Stack, count: i32) -> Option<Stack> {
    (count > 0).then(|| stack.with_count(count))
}

#[cfg(test)]
mod tests {
    #[test]
    fn binding_blocks_every_supported_way_out_of_an_armor_slot() {
        let mut data = crate::stackdata::Data::default();
        data.set_enchant(crate::stackdata::enchantment_number("binding_curse").unwrap(), 1);
        let helmet = Stack::new(crate::blocks::item_named("diamond_helmet").unwrap(), 1).with_extras(data);
        for (button, mode) in [(0, 0), (1, 0), (0, 1), (0, 2), (40, 2), (0, 4), (1, 4)] {
            let mut inventory = Inventory::new();
            inventory.set(5, Some(helmet));
            let click = inventory.click(5, button, mode);
            assert!(click.understood);
            assert_eq!(click.thrown, None);
            assert_eq!(inventory.slot(5), Some(helmet));
            assert_eq!(inventory.cursor(), None);
            assert_eq!(inventory.held(), None);
            assert_eq!(inventory.slot(OFFHAND), None);
        }
        let mut inventory = Inventory::new();
        inventory.set(5, Some(helmet));
        inventory.click_in_mode(5, 0, 0, true);
        assert_eq!(inventory.slot(5), None);
        assert_eq!(inventory.cursor(), Some(helmet));

        // Проклятие не мешает надеть предмет или убрать его из рюкзака.
        let mut inventory = Inventory::new();
        inventory.set(FIRST_HOTBAR, Some(helmet));
        inventory.click(FIRST_HOTBAR, 0, 0);
        inventory.click(5, 0, 0);
        assert_eq!(inventory.slot(5), Some(helmet));
        assert_eq!(inventory.cursor(), None);
    }

    use super::*;

    /// Камень: обычная стопка на 64.
    const STONE: i32 = 1;

    /// Ведро: таких в стопку помещается только 16.
    fn small_item() -> i32 {
        (1..2000)
            .find(|item| blocks::stack_size(*item) == 16)
            .expect("предмет с маленькой стопкой")
    }

    fn with(slot: i32, stack: Stack) -> Inventory {
        let mut inventory = Inventory::new();

        inventory.set(slot, Some(stack));
        inventory
    }

    /// Обычный щелчок: стопка берётся в курсор и кладётся в другой слот.
    /// Износ и зачарования переживают файл игрока; стопки с разными довесками
    /// не сливаются.
    #[test]
    fn worn_and_enchanted_stacks_survive_the_player_file() {
        let sword = blocks::item_named("diamond_sword").expect("меч");
        let mut data = crate::stackdata::Data { damage: 17, ..Default::default() };

        data.set_enchant(crate::stackdata::enchantment_number("sharpness").unwrap(), 5);

        let special = Stack::new(sword, 1).with_extras(data);
        let mut inventory = Inventory::new();

        inventory.set(FIRST_HOTBAR, Some(special));
        inventory.set(FIRST_HOTBAR + 1, Some(Stack::new(STONE, 12)));

        let back = Inventory::from_bytes(&inventory.to_bytes()).expect("прочиталось");

        assert_eq!(back.slot(FIRST_HOTBAR), Some(special));
        assert_eq!(back.slot(FIRST_HOTBAR).unwrap().extras().damage, 17);
        assert_eq!(back.slot(FIRST_HOTBAR + 1), Some(Stack::new(STONE, 12)));
        assert!(!special.stacks_with(&Stack::new(sword, 1)));
    }

    #[test]
    fn a_click_moves_a_stack_through_the_cursor() {
        let mut inventory = with(FIRST_HOTBAR, Stack::new(STONE, 10));

        assert!(inventory.click(FIRST_HOTBAR, 0, 0).understood);
        assert_eq!(inventory.cursor(), Some(Stack::new(STONE, 10)));
        assert_eq!(inventory.slot(FIRST_HOTBAR), None);

        assert!(inventory.click(FIRST_MAIN, 0, 0).understood);
        assert_eq!(inventory.cursor(), None);
        assert_eq!(inventory.slot(FIRST_MAIN), Some(Stack::new(STONE, 10)));
    }

    /// Правая кнопка: берётся половина, кладётся по одной штуке.
    #[test]
    fn the_right_button_halves_and_places_one() {
        let mut inventory = with(FIRST_HOTBAR, Stack::new(STONE, 7));

        inventory.click(FIRST_HOTBAR, 1, 0);
        assert_eq!(inventory.cursor(), Some(Stack::new(STONE, 4)));
        assert_eq!(inventory.slot(FIRST_HOTBAR), Some(Stack::new(STONE, 3)));

        inventory.click(FIRST_MAIN, 1, 0);
        assert_eq!(inventory.cursor(), Some(Stack::new(STONE, 3)));
        assert_eq!(inventory.slot(FIRST_MAIN), Some(Stack::new(STONE, 1)));
    }

    /// Разные предметы меняются местами, одинаковые — сливаются, и не больше
    /// предела стопки.
    #[test]
    fn stacks_merge_up_to_their_limit() {
        let mut inventory = with(FIRST_MAIN, Stack::new(STONE, 60));

        inventory.set(FIRST_HOTBAR, Some(Stack::new(STONE, 10)));
        inventory.click(FIRST_HOTBAR, 0, 0);
        inventory.click(FIRST_MAIN, 0, 0);

        // Влезло только четыре: предел — 64.
        assert_eq!(inventory.slot(FIRST_MAIN), Some(Stack::new(STONE, 64)));
        assert_eq!(inventory.cursor(), Some(Stack::new(STONE, 6)));

        // Другой предмет в том же слоте просто меняется местами.
        let mut inventory = with(FIRST_MAIN, Stack::new(STONE, 1));
        inventory.set(FIRST_HOTBAR, Some(Stack::new(STONE + 1, 1)));
        inventory.click(FIRST_HOTBAR, 0, 0);
        inventory.click(FIRST_MAIN, 0, 0);

        assert_eq!(inventory.slot(FIRST_MAIN), Some(Stack::new(STONE + 1, 1)));
        assert_eq!(inventory.cursor(), Some(Stack::new(STONE, 1)));
    }

    /// Щелчок с Shift перекладывает стопку из панели в рюкзак и обратно.
    #[test]
    fn shift_moves_between_the_backpack_and_the_bar() {
        let mut inventory = with(FIRST_HOTBAR, Stack::new(STONE, 5));

        assert!(inventory.click(FIRST_HOTBAR, 0, 1).understood);
        assert_eq!(inventory.slot(FIRST_HOTBAR), None);
        assert_eq!(inventory.slot(FIRST_MAIN), Some(Stack::new(STONE, 5)));

        assert!(inventory.click(FIRST_MAIN, 0, 1).understood);
        assert_eq!(inventory.slot(FIRST_MAIN), None);
        assert_eq!(inventory.slot(FIRST_HOTBAR), Some(Stack::new(STONE, 5)));
    }

    /// Щелчок с цифрой меняет слот со слотом панели.
    #[test]
    fn a_number_key_swaps_with_the_bar() {
        let mut inventory = with(FIRST_MAIN, Stack::new(STONE, 3));

        assert!(inventory.click(FIRST_MAIN, 2, 2).understood);
        assert_eq!(inventory.slot(FIRST_MAIN), None);
        assert_eq!(inventory.slot(FIRST_HOTBAR + 2), Some(Stack::new(STONE, 3)));

        // Клавиша второй руки кладёт туда же.
        assert!(inventory.click(FIRST_HOTBAR + 2, SWAP_OFFHAND, 2).understood);
        assert_eq!(inventory.slot(OFFHAND), Some(Stack::new(STONE, 3)));
    }

    /// Выброс отдаёт то, что выброшено: это должно упасть на землю, а не
    /// пропасть.
    #[test]
    fn things_can_be_thrown_away() {
        let mut inventory = with(FIRST_HOTBAR, Stack::new(STONE, 3));

        let one = inventory.click(FIRST_HOTBAR, 0, 4);
        assert_eq!(one.thrown, Some(Stack::new(STONE, 1)));
        assert_eq!(inventory.slot(FIRST_HOTBAR), Some(Stack::new(STONE, 2)));

        let rest = inventory.click(FIRST_HOTBAR, 1, 4);
        assert_eq!(rest.thrown, Some(Stack::new(STONE, 2)));
        assert_eq!(inventory.slot(FIRST_HOTBAR), None);

        // Щелчок мимо окна выбрасывает то, что в курсоре.
        inventory.set(FIRST_MAIN, Some(Stack::new(STONE, 2)));
        inventory.click(FIRST_MAIN, 0, 0);

        let outside = inventory.click(OUTSIDE, 0, 0);
        assert_eq!(outside.thrown, Some(Stack::new(STONE, 2)));
        assert_eq!(inventory.cursor(), None);
    }

    /// Незнакомый вид щелчка сервер не применяет: пусть лучше клиенту вернётся
    /// прежняя картинка, чем инвентарь разойдётся.
    #[test]
    fn an_unknown_click_changes_nothing() {
        let mut inventory = with(FIRST_HOTBAR, Stack::new(STONE, 3));
        let before = inventory.clone();

        assert!(!inventory.click(FIRST_HOTBAR, 0, 5).understood);
        assert_eq!(inventory, before);
    }

    /// Поднятое кладётся в начатую стопку, а когда та полна — в пустой слот.
    /// Предел стопки у каждого предмета свой.
    #[test]
    fn picked_up_things_fill_stacks_first() {
        let mut inventory = with(FIRST_HOTBAR, Stack::new(STONE, 62));

        assert_eq!(inventory.put(Stack::new(STONE, 5)), (vec![FIRST_HOTBAR, FIRST_HOTBAR + 1], None));
        assert_eq!(inventory.slot(FIRST_HOTBAR), Some(Stack::new(STONE, 64)));
        assert_eq!(inventory.slot(FIRST_HOTBAR + 1), Some(Stack::new(STONE, 3)));

        let bucket = small_item();
        let mut inventory = Inventory::new();

        inventory.put(Stack::new(bucket, 20));
        assert_eq!(inventory.slot(FIRST_HOTBAR), Some(Stack::new(bucket, 16)));
        assert_eq!(inventory.slot(FIRST_HOTBAR + 1), Some(Stack::new(bucket, 4)));
    }

    /// Поставленный блок расходует одну штуку, и рука пустеет, когда стопка
    /// кончилась.
    #[test]
    fn placing_spends_one_from_the_hand() {
        let mut inventory = with(FIRST_HOTBAR, Stack::new(STONE, 2));

        assert_eq!(inventory.held(), Some(Stack::new(STONE, 2)));
        assert_eq!(inventory.spend_held(), Some(FIRST_HOTBAR));
        assert_eq!(inventory.spend_held(), Some(FIRST_HOTBAR));
        assert_eq!(inventory.held(), None);
        assert_eq!(inventory.spend_held(), None);
    }

    /// Записанный инвентарь читается обратно ровно таким же.
    #[test]
    fn an_inventory_is_read_back_from_its_bytes() {
        let mut inventory = with(FIRST_MAIN, Stack::new(STONE, 12));

        inventory.set(OFFHAND, Some(Stack::new(STONE + 1, 1)));
        inventory.select(4);

        let written = inventory.to_bytes();
        let read = Inventory::from_bytes(&written).expect("прочитать");

        assert_eq!(read, inventory);
        assert_eq!(read.selected(), 4);

        // Обрезанная запись не читается, а не читается наполовину.
        assert_eq!(Inventory::from_bytes(&written[..5]), None);
        assert_eq!(Inventory::from_bytes(&[]), None);
    }

    /// Закрытое окно не съедает предмет: то, что было в курсоре, возвращается
    /// в инвентарь.
    #[test]
    fn closing_the_window_returns_what_was_in_hand() {
        let mut inventory = with(FIRST_HOTBAR, Stack::new(STONE, 4));

        inventory.click(FIRST_HOTBAR, 0, 0);
        assert_eq!(inventory.cursor(), Some(Stack::new(STONE, 4)));

        let closed = inventory.close_window();

        assert_eq!(inventory.cursor(), None);
        assert_eq!(closed.slots, vec![FIRST_HOTBAR]);
        assert!(closed.thrown.is_empty());
        assert_eq!(inventory.slot(FIRST_HOTBAR), Some(Stack::new(STONE, 4)));
    }

    /// Сетка верстака при закрытии окна тоже возвращается в инвентарь, а то,
    /// что не влезло, отдаётся наружу — его бросают на землю.
    #[test]
    fn closing_the_window_empties_the_crafting_grid() {
        let mut inventory = Inventory::new();

        // Всё забито чужим, кроме одного слота.
        for slot in (FIRST_MAIN..=LAST_MAIN).chain(FIRST_HOTBAR..=LAST_HOTBAR) {
            inventory.set(slot, Some(Stack::new(STONE + 1, 64)));
        }
        inventory.set(LAST_MAIN, None);

        inventory.set(1, Some(Stack::new(STONE, 3)));
        inventory.set(4, Some(Stack::new(STONE, 64)));

        let closed = inventory.close_window();

        // Сетка пуста, влезло сколько влезло, остальное — наружу.
        for slot in 1..=4 {
            assert_eq!(inventory.slot(slot), None, "слот сетки {slot}");
        }

        assert_eq!(inventory.slot(LAST_MAIN), Some(Stack::new(STONE, 64)));
        assert_eq!(closed.thrown, vec![Stack::new(STONE, 3)]);
        assert!(closed.slots.contains(&1) && closed.slots.contains(&4) && closed.slots.contains(&LAST_MAIN));
    }

    /// Что не влезло в полный инвентарь, возвращается остатком, а не
    /// пропадает.
    #[test]
    fn what_does_not_fit_is_given_back() {
        let mut inventory = Inventory::new();

        for slot in (FIRST_MAIN..=LAST_MAIN).chain(FIRST_HOTBAR..=LAST_HOTBAR) {
            inventory.set(slot, Some(Stack::new(STONE, 60)));
        }

        // 36 слотов по 4 свободных места — 144; просим 150.
        let (slots, left) = inventory.put(Stack::new(STONE, 150));

        assert_eq!(slots.len(), 36);
        assert_eq!(left, Some(Stack::new(STONE, 6)));
    }

    /// Слишком большое число в стопке не ломает счёт: при приёме оно
    /// обрезается пределом стопки, а половина берётся без переполнения.
    #[test]
    fn huge_counts_are_safe() {
        let mut inventory = with(FIRST_HOTBAR, Stack::new(STONE, i32::MAX));

        assert_eq!(inventory.slot(FIRST_HOTBAR), Some(Stack::new(STONE, 64)));

        inventory.click(FIRST_HOTBAR, 1, 0);
        assert_eq!(inventory.cursor(), Some(Stack::new(STONE, 32)));
        assert_eq!(inventory.slot(FIRST_HOTBAR), Some(Stack::new(STONE, 32)));
    }

    /// Shift досыпает в начатую стопку, а не занимает первый пустой слот:
    /// иначе одинаковые вещи расползались бы по инвентарю.
    #[test]
    fn shift_fills_started_stacks_first() {
        let mut inventory = with(FIRST_HOTBAR, Stack::new(STONE, 10));

        // Пустой слот в рюкзаке идёт раньше, чем начатая стопка.
        inventory.set(FIRST_MAIN + 5, Some(Stack::new(STONE, 20)));

        inventory.click(FIRST_HOTBAR, 0, 1);

        assert_eq!(inventory.slot(FIRST_MAIN), None);
        assert_eq!(inventory.slot(FIRST_MAIN + 5), Some(Stack::new(STONE, 30)));
    }

    /// Выбор блока по правилу с вики: в панели — переключаемся, в рюкзаке —
    /// переезжает, панель занята — заменяется выбранный, но не пропадает.
    #[test]
    fn picking_a_block_follows_the_rules() {
        let held = Stack::new(STONE + 1, 5);

        // 1. Предмет уже в панели: только переводим руку, ничего не двигая.
        let mut inventory = with(FIRST_HOTBAR + 3, Stack::new(STONE, 2));
        inventory.set(FIRST_HOTBAR, Some(held));
        inventory.select(0);

        let picked = inventory.pick(STONE, inventory.find(STONE));

        assert_eq!(picked.slots, Vec::<i32>::new());
        assert_eq!(picked.selected, Some(3));
        assert_eq!(inventory.held(), Some(Stack::new(STONE, 2)));
        assert_eq!(inventory.slot(FIRST_HOTBAR), Some(held), "рука потеряла старое");

        // 2. Предмет в рюкзаке, в панели есть пустой слот — переезжает туда.
        let mut inventory = with(FIRST_MAIN + 4, Stack::new(STONE, 7));
        inventory.set(FIRST_HOTBAR, Some(held));
        inventory.select(0);

        let picked = inventory.pick(STONE, inventory.find(STONE));

        assert_eq!(inventory.held(), Some(Stack::new(STONE, 7)));
        assert_eq!(inventory.slot(FIRST_HOTBAR), Some(held), "старое из руки пропало");
        assert!(picked.selected.is_some());

        // 3. Панель забита, предмета нет: заменяется выбранный — так на вики.
        let mut inventory = Inventory::new();
        for slot in FIRST_HOTBAR..=LAST_HOTBAR {
            inventory.set(slot, Some(held));
        }
        inventory.select(2);

        let picked = inventory.pick(STONE, None);

        assert_eq!(inventory.held(), Some(Stack::new(STONE, 1)));
        assert_eq!(picked.selected, Some(2), "рука переехала на другой слот");

        // 4. Панель забита, но предмет лежит в рюкзаке — меняются местами,
        // и ничего не теряется.
        let mut inventory = Inventory::new();
        for slot in FIRST_HOTBAR..=LAST_HOTBAR {
            inventory.set(slot, Some(held));
        }
        inventory.set(FIRST_MAIN + 2, Some(Stack::new(STONE, 9)));
        inventory.select(1);

        inventory.pick(STONE, inventory.find(STONE));

        assert_eq!(inventory.held(), Some(Stack::new(STONE, 9)));
        assert_eq!(inventory.slot(FIRST_MAIN + 2), Some(held));
    }
}
