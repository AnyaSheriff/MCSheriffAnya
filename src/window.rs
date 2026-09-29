// Окно хранилища: сундук, бочка, воронка, выбрасыватель, раздатчик.
//
// В окне два набора слотов сразу: сверху — хранилище, снизу — рюкзак и
// панель игрока. Протокол нумерует их подряд: 0..n — хранилище, дальше
// 27 слотов рюкзака и 9 слотов панели. Щелчок разбирается поверх обоих,
// а предмет в курсоре — общий, он живёт в инвентаре игрока.
//
// Правила щелчков — со страницы Inventory на minecraft.wiki (разделы о
// щелчках мышью и Shift-щелчке) и страницы Java Edition protocol (Click
// Container, режимы 0–6). Порядок, куда уходит стопка при Shift-щелчке,
// сверен чёрным ящиком (сценарий chest-shift-click).

use crate::blocks;
use crate::inventory::{self, remainder, take_one, Click, Inventory, Stack};

/// Слотов рюкзака и панели внизу окна.
pub const PLAYER_SLOTS: usize = 36;

/// Окно: хранилище и инвентарь игрока.
pub struct Window<'a> {
    pub container: &'a mut [Option<Stack>],
    pub inventory: &'a mut Inventory,
}

impl Window<'_> {
    /// Сколько всего слотов в окне.
    pub fn len(&self) -> usize {
        self.container.len() + PLAYER_SLOTS
    }

    /// Слот инвентаря игрока, который показан в окне под этим номером.
    fn player_slot(&self, slot: usize) -> Option<i32> {
        let below = slot.checked_sub(self.container.len())?;

        match below {
            0..27 => Some(inventory::FIRST_MAIN + below as i32),
            27..36 => Some(inventory::FIRST_HOTBAR + (below - 27) as i32),
            _ => None,
        }
    }

    /// Что лежит в слоте окна.
    pub fn get(&self, slot: usize) -> Option<Stack> {
        match self.player_slot(slot) {
            Some(own) => self.inventory.slot(own),
            None => self.container.get(slot).copied().flatten(),
        }
    }

    fn set(&mut self, slot: usize, stack: Option<Stack>) {
        let stack = stack.filter(|stack| stack.count > 0);

        match self.player_slot(slot) {
            Some(own) => self.inventory.set(own, stack),
            None => {
                if let Some(place) = self.container.get_mut(slot) {
                    *place = stack;
                }
            }
        }
    }

    /// Щелчок по окну. `mode` и `button` — как в пакете Click Container:
    /// 0 — обычный, 1 — с Shift, 2 — с цифрой, 4 — выброс, 6 — двойной.
    pub fn click(&mut self, slot: i32, button: i8, mode: i32) -> Click {
        match mode {
            0 => self.pick_up(slot, button),
            1 => Click { understood: self.quick_move(slot), thrown: None },
            2 => Click { understood: self.swap(slot, button), thrown: None },
            4 => self.throw_away(slot, button),
            6 => Click { understood: self.collect(slot), thrown: None },
            _ => Click::default(),
        }
    }

    /// Номер слота окна как индекс; None — такого слота нет.
    fn index(&self, slot: i32) -> Option<usize> {
        usize::try_from(slot).ok().filter(|slot| *slot < self.len())
    }

    /// Обычный щелчок: взять стопку, положить, доложить, поменять местами.
    fn pick_up(&mut self, slot: i32, button: i8) -> Click {
        let cursor = self.inventory.cursor();

        if slot == inventory::OUTSIDE {
            let thrown = match (cursor, button) {
                (None, _) => None,
                (Some(held), 0) => {
                    self.inventory.set_cursor(None);
                    Some(held)
                }
                (Some(held), _) => {
                    self.inventory.set_cursor(take_one(cursor));
                    Some(held.with_count(1))
                }
            };

            return Click { understood: true, thrown };
        }

        let Some(index) = self.index(slot) else {
            return Click::default();
        };

        let there = self.get(index);

        match (cursor, there, button) {
            (None, Some(stack), 0) => {
                self.inventory.set_cursor(Some(stack));
                self.set(index, None);
            }
            (None, Some(stack), _) => {
                let taken = stack.count - stack.count / 2;

                self.inventory.set_cursor(Some(stack.with_count(taken)));
                self.set(index, remainder(stack, stack.count - taken));
            }
            (None, None, _) => {}
            (Some(held), None, 0) => {
                self.set(index, Some(held));
                self.inventory.set_cursor(None);
            }
            (Some(held), None, _) => {
                self.set(index, Some(held.with_count(1)));
                self.inventory.set_cursor(take_one(Some(held)));
            }
            (Some(held), Some(there), button) if held.stacks_with(&there) => {
                let room = (there.limit() - there.count).max(0);
                let moved = if button == 0 { room.min(held.count) } else { room.min(1) };

                self.set(index, Some(there.with_count(there.count.saturating_add(moved))));
                self.inventory.set_cursor(remainder(held, held.count - moved));
            }
            (Some(held), Some(there), _) => {
                self.set(index, Some(held));
                self.inventory.set_cursor(Some(there));
            }
        }

        Click { understood: true, thrown: None }
    }

    /// Shift-щелчок: из хранилища — в инвентарь игрока, начиная с правого
    /// края панели и назад; из инвентаря — в хранилище с первого слота.
    /// Сперва доливаются начатые стопки, потом занимаются пустые слоты.
    fn quick_move(&mut self, slot: i32) -> bool {
        let Some(index) = self.index(slot) else {
            return false;
        };

        let Some(stack) = self.get(index) else {
            return true;
        };

        let size = self.container.len();
        let targets: Vec<usize> = if index < size {
            (size..self.len()).rev().collect()
        } else {
            (0..size).collect()
        };

        let left = self.pour(stack, &targets);

        self.set(index, remainder(stack, left));
        true
    }

    /// Раскладывает стопку по слотам `targets` по порядку: сперва в начатые
    /// стопки того же предмета, потом в пустые. Возвращает, сколько не влезло.
    fn pour(&mut self, stack: Stack, targets: &[usize]) -> i32 {
        let mut left = stack.count;

        for &target in targets {
            if left <= 0 {
                break;
            }

            if let Some(there) = self.get(target)
                && there.stacks_with(&stack)
            {
                let moved = (there.limit() - there.count).max(0).min(left);

                if moved > 0 {
                    self.set(target, Some(there.with_count(there.count + moved)));
                    left -= moved;
                }
            }
        }

        for &target in targets {
            if left <= 0 {
                break;
            }

            if self.get(target).is_none() {
                let moved = left.min(blocks::stack_size(stack.item));

                self.set(target, Some(stack.with_count(moved)));
                left -= moved;
            }
        }

        left
    }

    /// Щелчок с цифрой: слот окна меняется местами со слотом панели (или со
    /// второй рукой).
    fn swap(&mut self, slot: i32, button: i8) -> bool {
        let Some(index) = self.index(slot) else {
            return false;
        };

        let own = match button {
            inventory::SWAP_OFFHAND => inventory::OFFHAND,
            0..=8 => inventory::FIRST_HOTBAR + button as i32,
            _ => return false,
        };

        let here = self.get(index);
        let there = self.inventory.slot(own);

        self.set(index, there);
        self.inventory.set(own, here);
        true
    }

    /// Выброс: одна штука или вся стопка.
    fn throw_away(&mut self, slot: i32, button: i8) -> Click {
        let Some(index) = self.index(slot) else {
            return Click::default();
        };

        let Some(there) = self.get(index) else {
            return Click { understood: true, thrown: None };
        };

        let thrown = if button == 0 {
            self.set(index, take_one(Some(there)));
            there.with_count(1)
        } else {
            self.set(index, None);
            there
        };

        Click { understood: true, thrown: Some(thrown) }
    }

    /// Двойной щелчок: в курсор собирается такой же предмет со всего окна,
    /// пока стопка не полна, — сперва из неполных стопок, потом из полных.
    fn collect(&mut self, slot: i32) -> bool {
        let Some(_) = self.index(slot) else {
            return false;
        };

        let Some(mut held) = self.inventory.cursor() else {
            return true;
        };

        for full_pass in [false, true] {
            for index in 0..self.len() {
                if held.count >= held.limit() {
                    break;
                }

                let Some(there) = self.get(index) else {
                    continue;
                };

                if !there.stacks_with(&held) || (there.count >= there.limit()) != full_pass {
                    continue;
                }

                let moved = (held.limit() - held.count).min(there.count);

                held.count += moved;
                self.set(index, remainder(there, there.count - moved));
            }
        }

        self.inventory.set_cursor(Some(held));
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STONE: i32 = 1;

    /// Shift-щелчок из сундука: камень уходит в правый край панели.
    #[test]
    fn shift_click_from_a_chest_fills_the_hotbar_from_the_right() {
        let mut chest = vec![None; 27];
        let mut inventory = Inventory::new();

        chest[0] = Some(Stack::new(STONE, 10));

        let mut window = Window { container: &mut chest, inventory: &mut inventory };

        assert!(window.click(0, 0, 1).understood);
        assert_eq!(window.get(0), None);
        assert_eq!(inventory.slot(inventory::LAST_HOTBAR), Some(Stack::new(STONE, 10)));
    }

    /// Shift-щелчок из инвентаря: в первый свободный слот сундука, а начатая
    /// стопка того же предмета доливается раньше.
    #[test]
    fn shift_click_into_a_chest_tops_up_first() {
        let mut chest = vec![None; 27];
        let mut inventory = Inventory::new();

        chest[5] = Some(Stack::new(STONE, 60));
        inventory.set(inventory::FIRST_HOTBAR, Some(Stack::new(STONE, 10)));

        let mut window = Window { container: &mut chest, inventory: &mut inventory };
        let hotbar_in_window = 27 + 27;

        assert!(window.click(hotbar_in_window, 0, 1).understood);
        assert_eq!(chest[5], Some(Stack::new(STONE, 64)));
        assert_eq!(chest[0], Some(Stack::new(STONE, 6)));
        assert_eq!(inventory.slot(inventory::FIRST_HOTBAR), None);
    }

    /// Обычные щелчки через курсор переносят стопку между сундуком и
    /// рюкзаком.
    #[test]
    fn a_stack_travels_through_the_cursor() {
        let mut hopper = vec![None; 5];
        let mut inventory = Inventory::new();

        hopper[2] = Some(Stack::new(STONE, 7));

        let mut window = Window { container: &mut hopper, inventory: &mut inventory };

        window.click(2, 1, 0);
        assert_eq!(window.get(2), Some(Stack::new(STONE, 3)));
        window.click(5, 0, 0);
        assert_eq!(inventory.slot(inventory::FIRST_MAIN), Some(Stack::new(STONE, 4)));
    }
}
