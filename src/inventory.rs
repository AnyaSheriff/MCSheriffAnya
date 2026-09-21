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

/// Номер слота, который означает «щелчок мимо окна» — за его пределами.
const OUTSIDE: i32 = -999;

/// Кнопка обмена со второй рукой при щелчке с цифрой.
const SWAP_OFFHAND: i8 = 40;

/// Чем кончился «выбор блока».
#[derive(Clone, PartialEq, Debug)]
pub struct Picked {
    /// Слоты, которые изменились: их надо показать клиенту.
    pub slots: Vec<i32>,

    /// Новый выбранный слот панели, если рука сменила слот.
    pub selected: Option<i32>,
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
}

impl Stack {
    pub fn new(item: i32, count: i32) -> Self {
        Self { item, count }
    }

    /// Сколько таких предметов помещается в одну стопку.
    fn limit(&self) -> i32 {
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
}

impl Inventory {
    pub fn new() -> Self {
        Self {
            slots: [None; SLOTS],
            selected: 0,
            cursor: None,
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
            self.slots[slot] = stack.filter(|stack| stack.count > 0);
        }
    }

    /// Предмет в курсоре.
    pub fn cursor(&self) -> Option<Stack> {
        self.cursor
    }

    /// Забирает предмет из курсора и раскладывает его обратно по слотам.
    ///
    /// Зовётся, когда игрок закрывает окно инвентаря: то, что он держал
    /// мышкой, не должно пропасть.
    pub fn put_cursor_back(&mut self) -> Vec<i32> {
        let Some(stack) = self.cursor.take() else {
            return Vec::new();
        };

        self.add(stack)
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
    /// Возвращает слоты, которые изменились, — их надо показать клиенту.
    /// Что не поместилось, пропадает: земли под ногами, куда это положить,
    /// у сервера пока нет.
    pub fn add(&mut self, stack: Stack) -> Vec<i32> {
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
                && there.item == stack.item
            {
                let room = there.limit() - there.count;
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

                self.slots[index] = Some(Stack::new(stack.item, moved));
                left -= moved;
                changed.push(*slot);
            }
        }

        changed
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
                    Some(Stack::new(held.item, 1))
                }
            };

            return Click { understood: true, thrown };
        }

        let Some(index) = self.index(slot) else {
            return Click::default();
        };

        let there = self.slots[index];

        match (self.cursor, there, button) {
            // Курсор пуст — забираем стопку целиком или половину.
            (None, Some(stack), 0) => {
                self.cursor = Some(stack);
                self.slots[index] = None;
            }
            (None, Some(stack), _) => {
                let taken = (stack.count + 1) / 2;

                self.cursor = Some(Stack::new(stack.item, taken));
                self.slots[index] = remainder(stack, stack.count - taken);
            }
            (None, None, _) => {}

            // В курсоре что-то есть, слот пуст — кладём всё или одну штуку.
            (Some(held), None, 0) => {
                self.slots[index] = Some(held);
                self.cursor = None;
            }
            (Some(held), None, _) => {
                self.slots[index] = Some(Stack::new(held.item, 1));
                self.cursor = take_one(Some(held));
            }

            // Тот же предмет — досыпаем, сколько влезет.
            (Some(held), Some(there), button) if held.item == there.item => {
                let room = there.limit() - there.count;
                let moved = if button == 0 { room.min(held.count) } else { room.min(1) };

                self.slots[index] = Some(Stack::new(there.item, there.count + moved));
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
                && there.item == stack.item
            {
                let moved = (there.limit() - there.count).min(left);

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

                self.slots[target] = Some(Stack::new(stack.item, moved));
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
                Stack::new(there.item, 1)
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
            .filter_map(|(slot, stack)| stack.map(|stack| (slot, stack)))
            .collect();

        let mut out = Vec::with_capacity(2 + filled.len() * 9);

        out.push(self.selected as u8);
        out.push(filled.len() as u8);

        for (slot, stack) in filled {
            out.push(slot as u8);
            out.extend_from_slice(&stack.item.to_be_bytes());
            out.extend_from_slice(&stack.count.to_be_bytes());
        }

        out
    }

    /// Читает инвентарь из файла игрока. None — запись испорчена.
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let selected = *bytes.first()?;
        let filled = *bytes.get(1)? as usize;

        let mut inventory = Inventory::new();
        inventory.select(selected as i32);

        for number in 0..filled {
            let start = 2 + number * 9;
            let slot = *bytes.get(start)? as i32;

            let item = i32::from_be_bytes(bytes.get(start + 1..start + 5)?.try_into().ok()?);
            let count = i32::from_be_bytes(bytes.get(start + 5..start + 9)?.try_into().ok()?);

            if count > 0 {
                inventory.set(slot, Some(Stack::new(item, count)));
            }
        }

        Some(inventory)
    }
}

/// Стопка, из которой убрали одну штуку. None — стопка кончилась.
fn take_one(stack: Option<Stack>) -> Option<Stack> {
    let stack = stack?;

    remainder(stack, stack.count - 1)
}

/// Та же стопка, но другого размера. None — в ней ничего не осталось.
fn remainder(stack: Stack, count: i32) -> Option<Stack> {
    (count > 0).then(|| Stack::new(stack.item, count))
}

#[cfg(test)]
mod tests {
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

        assert_eq!(inventory.add(Stack::new(STONE, 5)), vec![FIRST_HOTBAR, FIRST_HOTBAR + 1]);
        assert_eq!(inventory.slot(FIRST_HOTBAR), Some(Stack::new(STONE, 64)));
        assert_eq!(inventory.slot(FIRST_HOTBAR + 1), Some(Stack::new(STONE, 3)));

        let bucket = small_item();
        let mut inventory = Inventory::new();

        inventory.add(Stack::new(bucket, 20));
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

        let changed = inventory.put_cursor_back();

        assert_eq!(inventory.cursor(), None);
        assert_eq!(changed, vec![FIRST_HOTBAR]);
        assert_eq!(inventory.slot(FIRST_HOTBAR), Some(Stack::new(STONE, 4)));
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
