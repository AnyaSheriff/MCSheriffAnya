// Общая транзакция крафта: итог никогда не является обычной ячейкой.
// Вики, Crafting: «Shift-clicking the output slot»; остатки возвращаются
// в сетку, затем в инвентарь, затем на землю [проверить порядок].
use crate::inventory::{self, Inventory, Stack};

pub fn padded(grid: &[Option<Stack>]) -> [Option<Stack>; 9] {
    let mut out = [None; 9];
    let width = if grid.len() == 4 { 2 } else { 3 };
    for (i, s) in grid.iter().take(width * width).enumerate() { out[i / width * 3 + i % width] = *s; }
    out
}

/// Перенос слиянием, затем в пустые места; Shift-итог заполняет с конца.
pub fn insert(inventory: &mut Inventory, stack: Stack, reverse: bool) -> Option<Stack> {
    let mut slots: Vec<i32> = (9..=44).collect();
    if reverse { slots.reverse(); }
    let mut left = stack.count;
    for slot in &slots {
        if let Some(old) = inventory.slot(*slot).filter(|s| s.stacks_with(&stack)) {
            let moved = left.min((old.limit() - old.count).max(0));
            inventory.set(*slot, Some(old.with_count(old.count + moved))); left -= moved;
        }
    }
    for slot in slots {
        if left > 0 && inventory.slot(slot).is_none() {
            let moved = left.min(stack.limit()); inventory.set(slot, Some(stack.with_count(moved))); left -= moved;
        }
    }
    inventory::remainder(stack, left)
}

/// Один атомарный крафт. false — итог не помещается, входы не тронуты.
pub fn take(grid: &mut [Option<Stack>], inventory: &mut Inventory, result: Stack, mode: i32, button: i8, dropped: &mut Vec<Stack>) -> bool {
    match mode {
        0 if (0..=1).contains(&button) => {
            let held = inventory.cursor();
            if held.is_some_and(|h| !h.stacks_with(&result) || h.count + result.count > h.limit()) { return false; }
            inventory.set_cursor(Some(result.with_count(result.count + held.map_or(0, |s| s.count))));
        }
        1 => {
            // Не поместилось целиком — что влезло, то в рюкзаке, остальное игрок
            // бросает (чёрный ящик, craft-shift-partial: 62+2 доски, 2 на земле);
            // не влезло ничего — ничего не расходуется (craft-shift-full).
            let mut trial = inventory.clone();
            let left = insert(&mut trial, result, true);
            if left.is_some_and(|l| l.count == result.count) { return false; }
            *inventory = trial;
            dropped.extend(left);
        }
        2 if (0..=8).contains(&button) || button == 40 => {
            let slot = if button == 40 { 45 } else { 36 + i32::from(button) };
            let old = inventory.slot(slot);
            if old.is_some_and(|s| !s.stacks_with(&result) || s.count + result.count > s.limit()) { return false; }
            inventory.set(slot, Some(result.with_count(result.count + old.map_or(0, |s| s.count))));
        }
        4 if inventory.cursor().is_none() => dropped.push(result),
        _ => return false,
    }
    let remainders = super::remainders(grid);
    for (slot, rest) in grid.iter_mut().zip(remainders) {
        *slot = inventory::take_one(*slot);
        if let Some(rest) = rest {
            if slot.is_none() { *slot = Some(rest); }
            else if slot.is_some_and(|s| s.stacks_with(&rest) && s.count + rest.count <= s.limit()) { *slot = slot.map(|s| s.with_count(s.count + rest.count)); }
            else { dropped.extend(inventory.put(rest).1); }
        }
    }
    true
}

/// Автозаполнение книги: план целиком, без потери вещей при нехватке места.
pub fn place(recipe: &super::Recipe, grid: &mut [Option<Stack>], inventory: &mut Inventory, all: bool) -> bool {
    let width = if grid.len() == 4 { 2 } else { 3 };
    if recipe.shaped && (recipe.width as usize > width || recipe.height as usize > width) || recipe.cells.iter().filter(|c| !c.is_empty()).count() > grid.len() { return false; }
    let mut bag = inventory.clone();
    for stack in grid.iter().flatten() { if bag.put(*stack).1.is_some() { return false; } }
    let previous = grid.iter().flatten().map(|s| s.count).min().unwrap_or(0);
    let cap = if all { 64 } else if super::recipe_index(&padded(grid)).is_some_and(|i| std::ptr::eq(&super::recipes()[i], recipe)) { (previous + 1).min(64) } else { 1 };
    let mut best = None;
    for amount in 1..=cap {
        let mut trial = bag.clone(); let mut filled = vec![None; grid.len()];
        let cells: Vec<_> = recipe.cells.iter().enumerate().filter(|(_,c)| !c.is_empty()).collect();
        if !assign(&cells, 0, amount, &mut trial, &mut filled, recipe, width) { break; }
        best = Some((trial, filled));
    }
    if let Some((bag, filled)) = best { *inventory = bag; grid.copy_from_slice(&filled); true }
    else { *inventory = bag; grid.fill(None); false }
}

fn assign(cells: &[(usize, &&[i32])], at: usize, amount: i32, bag: &mut Inventory, grid: &mut [Option<Stack>], recipe: &super::Recipe, width: usize) -> bool {
    if at == cells.len() { return true; }
    let (index, allowed) = cells[at];
    let target = if recipe.shaped {
        let x = usize::from(usize::from(recipe.width) * 2 < width);
        let y = usize::from(usize::from(recipe.height) * 2 < width);
        (index / recipe.width as usize + y) * width + index % recipe.width as usize + x
    } else { at };
    let mut tried = std::collections::BTreeSet::new();
    for slot in 9..=44 {
        let Some(stack) = bag.slot(slot).filter(|s| allowed.contains(&s.item) && s.limit() >= amount && s.extras() == crate::stackdata::Data::default()) else { continue; };
        if !tried.insert(stack.item) { continue; }
        let mut trial = bag.clone(); let mut left = amount;
        for source in 9..=44 {
            if let Some(s) = trial.slot(source).filter(|s| s.stacks_with(&stack)) { let n = left.min(s.count); trial.set(source, inventory::remainder(s, s.count - n)); left -= n; }
        }
        if left != 0 { continue; }
        grid[target] = Some(stack.with_count(amount));
        if assign(cells, at + 1, amount, &mut trial, grid, recipe, width) { *bag = trial; return true; }
        grid[target] = None;
    }
    false
}
