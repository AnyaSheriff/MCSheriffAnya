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

    /// Строгий путь записей мобов: проверяем сырые данные до нормализации.
    /// Каталог и предел стопки проверяет вызывающий код после полного чтения.
    pub fn from_stored_strict(stored: i32, count: i32, extras: Option<crate::stackdata::Data>) -> Option<Self> {
        if count <= 0 || stored & 0x00ff_ffff == 0 || (stored < 0) != extras.is_some() { return None; }
        let tag = ((stored >> 24) & 0x7f) as u8;
        let mut stack = Self::from_stored(stored, count);
        if tag != 0 && (stack.tnt.map(|kind| kind.0) != Some(tag)
            || blocks::item_name(stack.item) != Some("tnt")) { return None; }
        if let Some(data) = extras { stack = stack.with_extras(data); }
        (stack.stored_item() == stored).then_some(stack)
    }

    /// Проверяет именно записанные байты компонентов, включая лишний хвост.
    pub fn read_extras_strict<'a>(stored: i32, mut take: impl FnMut(usize) -> Option<&'a [u8]>) -> Option<Option<crate::stackdata::Data>> {
        if stored >= 0 { return Some(None); }
        let length = usize::from(u16::from_be_bytes(take(2)?.try_into().ok()?));
        let bytes = take(length)?;
        let data = crate::stackdata::from_bytes(bytes)?;
        (crate::stackdata::to_bytes(&data) == bytes).then_some(Some(data))
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

        // Сперва доложить в начатые стопки, включая вторую руку.
        // Наблюдение official775: полный рюкзак + offhand pumpkin7,
        // give pumpkin1 → offhand8. Пустую вторую руку не заполняем.
        for slot in order.iter().chain(std::iter::once(&OFFHAND)) {
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
            2 => self.swap(slot, button),
            4 => self.throw_away(slot, button),
            _ => Click::default(),
        }
    }

    /// Броня принимает только свой тип экипировки и ровно одну вещь.
    /// Официальные340/775/777: bedrock в слот шлема не помещается.
    fn slot_limit(slot: i32, stack: Stack) -> i32 {
        if (5..=8).contains(&slot) {
            i32::from(crate::combat::info(stack.item).is_some_and(|info| i32::from(info.equip_slot) == 12 - slot))
        } else { stack.limit() }
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

        if self.cursor.is_some_and(|held| Self::slot_limit(slot, held) == 0) {
            return Click { understood: true, thrown: None };
        }
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
                let moved = held.count.min(Self::slot_limit(slot, held));
                self.slots[index] = Some(held.with_count(moved));
                self.cursor = remainder(held, held.count - moved);
            }
            (Some(held), None, _) => {
                self.slots[index] = Some(held.with_count(1));
                self.cursor = take_one(Some(held));
            }

            // Тот же предмет — досыпаем, сколько влезет.
            (Some(held), Some(there), button) if held.stacks_with(&there) => {
                let room = (Self::slot_limit(slot, there) - there.count).max(0);
                let moved = if button == 0 { room.min(held.count) } else { room.min(1) };

                self.slots[index] = Some(there.with_count(there.count.saturating_add(moved)));
                self.cursor = remainder(held, held.count - moved);
            }

            // Разные предметы — меняем местами.
            (Some(held), Some(there), _) => {
                if held.count > Self::slot_limit(slot, held) { return Click { understood: true, thrown: None }; }
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
    fn swap(&mut self, slot: i32, button: i8) -> Click {
        let other = match button {
            SWAP_OFFHAND => OFFHAND,
            0..=8 => FIRST_HOTBAR + button as i32,
            _ => return Click::default(),
        };

        let (Some(here), Some(there)) = (self.index(slot), self.index(other)) else {
            return Click::default();
        };
        let unchanged = Click { understood: true, thrown: None };
        if here == there { return unchanged; }

        if let Some(incoming) = self.slots[there] {
            let limit = Self::slot_limit(slot, incoming);
            if limit == 0 { return unchanged; }
            if incoming.count > limit {
                // Вики, Inventory: «hotbar keys». Официальные340/775:
                // из стопки тыкв надевается одна, прежний шлем возвращается
                // в рюкзак; при полном инвентаре выпадает ровно остаток.
                let displaced = self.slots[here].replace(incoming.with_count(limit));
                self.slots[there] = remainder(incoming, incoming.count - limit);
                return Click { understood: true, thrown: displaced.and_then(|stack| self.put(stack).1) };
            }
        }
        if self.slots[here].is_some_and(|stack| stack.count > Self::slot_limit(other, stack)) { return unchanged; }
        self.slots.swap(here, there);

        unchanged
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

    /// Строгий путь сохранений: неизвестное/усечённое не становится пустой ячейкой.
    pub fn from_bytes_for_catalog(bytes: &[u8], catalog: crate::gameplay_catalog::Catalog) -> Result<Self, crate::gameplay_catalog::StackError> {
        use crate::gameplay_catalog::{StackError, StackProblem};
        let malformed = || StackError { path: Vec::new(), problem: StackProblem::MalformedEncoding };
        let inventory = Self::from_bytes(bytes).ok_or_else(malformed)?;
        catalog.validate_inventory(&inventory)?;
        // Включая выбранный слот, custom TNT, компоненты и хвост атрибутов.
        // Старый декодер может обрезать/пропустить запись — это не успех.
        let canonical = inventory.to_bytes();
        // Прежний писатель мог явно записывать пустой хвост атрибутов.
        let legacy_empty_tail = inventory.seen_attributes.is_empty()
            && bytes.strip_suffix(&[0]) == Some(canonical.as_slice());
        if canonical != bytes && !legacy_empty_tail { return Err(malformed()); }
        Ok(inventory)
    }

    /// Явный профиль проверяет предел до установки; общий каталог775 не меняется.
    #[cfg(test)]
    pub fn set_for_catalog(&mut self, slot: usize, stack: Option<Stack>, catalog: crate::gameplay_catalog::Catalog) -> Result<(), crate::gameplay_catalog::StackError> {
        use crate::gameplay_catalog::{StackError, StackLocation, StackProblem};
        let target = self.slots.get_mut(slot).ok_or_else(|| StackError { path: vec![StackLocation::Slot(slot)], problem: StackProblem::MalformedEncoding })?;
        if let Some(stack) = &stack { catalog.validate_stack(stack)?; }
        *target = stack;
        Ok(())
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
    fn strict_inventory_accepts_legacy_optional_empty_attribute_tail() {
        let catalog = crate::gameplay_catalog::Catalog::new(crate::gameplay_catalog::Profile::Java26_1_2);
        let inventory = Inventory::new();
        let canonical = inventory.to_bytes();
        assert_eq!(Inventory::from_bytes_for_catalog(&canonical, catalog), Ok(inventory.clone()));
        let mut legacy = canonical;
        legacy.push(0);
        assert_eq!(Inventory::from_bytes_for_catalog(&legacy, catalog), Ok(inventory));
        legacy.push(0);
        assert!(Inventory::from_bytes_for_catalog(&legacy, catalog).is_err());
    }

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

    #[test]
    fn armor_slots_reject_wrong_items_and_limit_wearables() {
        let stone = Stack::new(crate::blocks::item_named("bedrock").unwrap(),8);
        let helmet = Stack::new(crate::blocks::item_named("diamond_helmet").unwrap(),1);
        for slot in 5..=8 {
            for creative in [false, true] {
                let mut inv = Inventory::new(); inv.set_cursor(Some(stone));
                inv.click_in_mode(slot, 0, 0, creative);
                assert_eq!(inv.slot(slot),None); assert_eq!(inv.cursor(),Some(stone));
                inv.set_cursor(None); inv.set(36,Some(stone));
                inv.click_in_mode(slot,0,2,creative);
                assert_eq!(inv.slot(slot),None); assert_eq!(inv.slot(36),Some(stone));
            }
        }
        let mut inv = Inventory::new(); inv.set_cursor(Some(helmet));
        inv.click(6,0,0); assert_eq!(inv.slot(6),None); assert_eq!(inv.cursor(),Some(helmet));
        inv.click(5,0,0); assert_eq!(inv.slot(5),Some(helmet)); assert_eq!(inv.cursor(),None);
        let pumpkin = Stack::new(crate::blocks::item_named("carved_pumpkin").unwrap(),8);
        for mode in [0,2] {
            let mut inv = Inventory::new();
            if mode == 0 { inv.set_cursor(Some(pumpkin)); } else { inv.set(36,Some(pumpkin)); }
            inv.click(5,0,mode); assert_eq!(inv.slot(5),Some(pumpkin.with_count(1)));
            assert_eq!(if mode == 0 {inv.cursor()} else {inv.slot(36)},Some(pumpkin.with_count(7)));
        }
    }

    #[test]
    fn put_merges_existing_offhand_without_filling_an_empty_one() {
        let pumpkin = Stack::new(crate::blocks::item_named("carved_pumpkin").unwrap(), 7);
        let bedrock = Stack::new(crate::blocks::item_named("bedrock").unwrap(), 64);
        let mut inventory = Inventory::new();
        for slot in FIRST_MAIN..=LAST_HOTBAR { inventory.set(slot, Some(bedrock)); }
        inventory.set(OFFHAND, Some(pumpkin));
        assert_eq!(inventory.put(pumpkin.with_count(1)), (vec![OFFHAND], None));
        assert_eq!(inventory.slot(OFFHAND), Some(pumpkin.with_count(8)));

        inventory.set(OFFHAND, Some(pumpkin.with_count(63)));
        assert_eq!(inventory.put(pumpkin.with_count(3)), (vec![OFFHAND], Some(pumpkin.with_count(2))));
        assert_eq!(inventory.slot(OFFHAND), Some(pumpkin.with_count(64)));

        inventory.set(OFFHAND, None);
        assert_eq!(inventory.put(pumpkin), (vec![], Some(pumpkin)));
        assert_eq!(inventory.slot(OFFHAND), None);

        let mut data = crate::stackdata::Data::default();
        data.set_enchant(crate::stackdata::enchantment_number("binding_curse").unwrap(), 1);
        let distinct = pumpkin.with_extras(data);
        inventory.set(OFFHAND, Some(distinct));
        assert_eq!(inventory.put(pumpkin), (vec![], Some(pumpkin)));
        assert_eq!(inventory.slot(OFFHAND), Some(distinct));
    }

    #[test]
    fn armor_number_keys_match_official_snapshots_and_conserve_every_item() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!("../tools/protocol-bridge/testdata/inventory-armor-edges.json")).unwrap();
        let stack = |value: &serde_json::Value| {
            (!value.is_null()).then(|| Stack::new(crate::blocks::item_named(value["item"].as_str().unwrap()).unwrap(), value["count"].as_i64().unwrap() as i32))
        };
        let totals = |inventory: &Inventory, dropped: &[Stack]| {
            let mut counts = std::collections::BTreeMap::<i32, i32>::new();
            for item in inventory.slots().iter().flatten().chain(inventory.cursor().as_ref()).chain(dropped) {
                *counts.entry(item.item).or_default() += item.count;
            }
            counts
        };
        for case in fixture["cases"].as_array().unwrap() {
            let name = case["name"].as_str().unwrap();
            let mut inventory = Inventory::new();
            for (index, value) in case["before"].as_array().unwrap().iter().enumerate() { inventory.set(index as i32, stack(value)); }
            let before = totals(&inventory, &[]);
            let button = case["button"].as_i64().unwrap() as i8;
            let click = inventory.click(5, button, 2);
            let dropped: Vec<_> = click.thrown.into_iter().collect();
            assert!(click.understood, "{name}");
            let expected: Vec<_> = case["after"].as_array().unwrap().iter().map(stack).collect();
            assert_eq!(inventory.slots(), expected, "{name}");
            assert_eq!(inventory.cursor(), stack(&case["cursor"]), "{name}");
            assert_eq!(dropped, case["dropped"].as_array().unwrap().iter().filter_map(stack).collect::<Vec<_>>(), "{name}");
            assert_eq!(totals(&inventory, &dropped), before, "{name}: потеря или дюп");
            // Повтор одного действия не должен повторно выбрасывать уже
            // вытесненный шлем, даже если клиент повторит запрос много раз.
            for _ in 0..16 {
                assert_eq!(inventory.click(5, button, 2).thrown, None, "{name}: повторный дроп");
                assert_eq!(totals(&inventory, &dropped), before, "{name}: повторный щелчок");
            }
        }
    }

    #[test]
    fn bound_armor_rolls_back_displacement_and_overflow_drop() {
        let mut inventory = Inventory::new();
        let bedrock = Stack::new(crate::blocks::item_named("bedrock").unwrap(), 64);
        for slot in 9..=44 { inventory.set(slot, Some(bedrock)); }
        let mut data = crate::stackdata::Data::default();
        data.set_enchant(crate::stackdata::enchantment_number("binding_curse").unwrap(), 1);
        inventory.set(5, Some(Stack::new(crate::blocks::item_named("iron_helmet").unwrap(), 1).with_extras(data)));
        inventory.set(36, Some(Stack::new(crate::blocks::item_named("carved_pumpkin").unwrap(), 8)));
        let before = inventory.clone();
        for _ in 0..16 {
            let click = inventory.click(5, 0, 2);
            assert!(click.understood); assert_eq!(click.thrown, None);
            assert_eq!(inventory.slots(), before.slots());
            assert_eq!(inventory.cursor(), before.cursor());
        }
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

#[cfg(test)]
mod strict_mob_storage_tests {
    use super::*;
    use crate::mob::{self, Kind, Mob};

    // Только сохранённая стопка сравнивается целиком, не нормализуемое тело моба.
    fn record(family: usize, stack: Stack) -> Vec<u8> {
        let mut bytes = Vec::new();
        let mut mob = Mob::new(71, Kind::Llama, (0.0, 64.0, 0.0));
        match family {
            0 => { let mut gear = [None; 6]; gear[0] = Some(stack); mob::equipment::write(&gear, &mut bytes); }
            1 => { let mut gear = [None; 3]; gear[0] = Some(stack); mob::trial_gear::write(&gear, &mut bytes); }
            2 => { mob.stand.equipment[0] = Some(stack); mob.stand.write(&mut bytes); }
            3 => { mob.horse.slots[0] = Some(stack); mob::horse::write(&mob, &mut bytes); }
            4 => { mob.land.chest = true; mob.land.strength = 1; mob.land.cargo = vec![Some(stack), None, None]; mob::land::write(&mob, &mut bytes); }
            5 => { mob.small.held = Some(stack); mob::small::write(&mob, &mut bytes); }
            6 => { mob.wild.mouth = Some(stack); mob::wildlife::write(&mob.wild, &mut bytes); }
            7 => { mob.newmobs.cargo = Some(stack); mob::newmobs::write(&mob.newmobs, &mut bytes); }
            _ => unreachable!(),
        }
        bytes
    }

    fn read(family: usize, bytes: &mut &[u8]) -> Option<Stack> {
        match family {
            0 => mob::equipment::read(bytes)?[0],
            1 => mob::trial_gear::read(bytes)?[0],
            2 => mob::armor_stand::State::read(bytes)?.equipment[0],
            3 => { let mut mob = Mob::new(72, Kind::Llama, (0.0, 64.0, 0.0)); mob::horse::read(&mut mob, bytes)?; mob.horse.slots[0] }
            4 => mob::land::read(bytes)?.0.cargo[0],
            5 => mob::small::read(bytes)?.0.held,
            6 => mob::wildlife::read(bytes)?.mouth,
            7 => mob::newmobs::read(bytes)?.cargo,
            _ => unreachable!(),
        }
    }

    fn change_item(bytes: &mut [u8], old: i32, new: i32) {
        let needle = old.to_be_bytes();
        let offsets: Vec<_> = bytes.windows(4).enumerate().filter_map(|(i, b)| (b == needle).then_some(i)).collect();
        assert_eq!(offsets.len(), 1, "одна выбранная запись предмета");
        bytes[offsets[0]..offsets[0]+4].copy_from_slice(&new.to_be_bytes());
    }

    #[test]
    fn strict_storage_decodes_writer_records_and_refuses_tnt_tag_normalization_in_every_family() {
        let stone = blocks::item_named("stone").unwrap();
        let tnt = blocks::item_named("tnt").unwrap();
        for family in 0..8 {
            let stack = Stack::new(stone, 1).with_extras(crate::stackdata::Data { name: Some("Сохранённая стопка".into()), ..Default::default() });
            let valid = record(family, stack);
            let mut concatenated = valid.clone(); concatenated.extend([0xab, 0xcd]);
            let mut tail = concatenated.as_slice();
            assert_eq!(read(family, &mut tail), Some(stack), "семейство{family}");
            assert_eq!(tail, &[0xab, 0xcd]);
            for raw in [stack.stored_item() | (127 << 24), stack.stored_item() | (1 << 24)] {
                let mut bad = valid.clone(); change_item(&mut bad, stack.stored_item(), raw);
                assert!(read(family, &mut bad.as_slice()).is_none(), "семейство{family}, raw{raw}");
            }
            let item_at = valid.windows(4).position(|b| b == stack.stored_item().to_be_bytes()).unwrap();
            let extra_at = item_at + if matches!(family, 1 | 6) { 4 } else { 8 };
            let length = u16::from_be_bytes(valid[extra_at..extra_at+2].try_into().unwrap());
            let mut bad_extra = valid.clone();
            bad_extra[extra_at..extra_at+2].copy_from_slice(&(length + 1).to_be_bytes());
            bad_extra.insert(extra_at + 2 + usize::from(length), 0);
            assert!(read(family, &mut bad_extra.as_slice()).is_none(), "лишний хвост компонентов семейство{family}");
            let custom = Stack { tnt: Some(crate::custom_tnt::KindId(1)), ..Stack::new(tnt, 1) };
            let bytes = record(family, custom);
            assert_eq!(read(family, &mut bytes.as_slice()), Some(custom), "известный TNT семейство{family}");
        }
    }

    #[test]
    fn present_zero_and_negative_counts_are_errors_not_empty_horse_or_cargo_slots() {
        let item = blocks::item_named("stone").unwrap();
        for family in [0, 2, 3, 4, 5, 7] {
            let valid = record(family, Stack::new(item, 7));
            for count in [0i32, -1] {
                let mut needle = item.to_be_bytes().to_vec(); needle.extend(7_i32.to_be_bytes());
                let offsets: Vec<_> = valid.windows(8).enumerate().filter_map(|(i, b)| (b == needle).then_some(i)).collect();
                assert_eq!(offsets.len(), 1);
                let mut bad = valid.clone();
                bad[offsets[0]+4..offsets[0]+8].copy_from_slice(&count.to_be_bytes());
                assert!(read(family, &mut bad.as_slice()).is_none(), "семейство{family}, count{count}");
            }
        }
    }

    #[test]
    fn strict_extras_refuse_trailing_component_bytes_and_empty_flag_normalization() {
        let stack = Stack::new(blocks::item_named("stone").unwrap(), 1).with_extras(crate::stackdata::Data { name: Some("Имя".into()), ..Default::default() });
        let mut extra = Vec::new(); stack.write_extras(&mut extra);
        let length = u16::from_be_bytes(extra[..2].try_into().unwrap());
        extra[..2].copy_from_slice(&(length + 1).to_be_bytes()); extra.push(0);
        let mut tail = extra.as_slice();
        assert!(Stack::read_extras_strict(stack.stored_item(), |n| { let (a,b) = tail.split_at_checked(n)?; tail = b; Some(a) }).is_none());
        assert!(Stack::from_stored_strict(stack.item | i32::MIN, 1, Some(Default::default())).is_none());
        assert!(Stack::from_stored_strict(stack.item, 1, Some(stack.extras())).is_none());
    }
}
