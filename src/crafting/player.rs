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
#[cfg(test)]
pub fn place(recipe: &super::Recipe, grid: &mut [Option<Stack>], inventory: &mut Inventory, all: bool) -> bool {
    place_for_catalog(crate::gameplay_catalog::Catalog::new(crate::gameplay_catalog::Profile::Java26_1_2), recipe, grid, inventory, all)
}

pub fn place_for_catalog(catalog: crate::gameplay_catalog::Catalog, recipe: &super::Recipe, grid: &mut [Option<Stack>], inventory: &mut Inventory, all: bool) -> bool {
    if recipe.result == crate::gameplay_catalog::WHITE_SLAB_ITEM
        && super::recipe_for_catalog(catalog, super::RecipeId::WhiteConcreteSlab).is_none()
    { return false; }
    let width = if grid.len() == 4 { 2 } else { 3 };
    if recipe.shaped && (recipe.width as usize > width || recipe.height as usize > width) || recipe.cells.iter().filter(|c| !c.is_empty()).count() > grid.len() { return false; }
    let mut bag = inventory.clone();
    for stack in grid.iter().flatten() { if bag.put(*stack).1.is_some() { return false; } }
    let previous = grid.iter().flatten().map(|s| s.count).min().unwrap_or(0);
    let cap = if all { 64 } else if super::recipe_identity_for_catalog(catalog, &padded(grid)).and_then(|id| super::recipe_for_catalog(catalog, id)).is_some_and(|known| std::ptr::eq(known, recipe)) { (previous + 1).min(64) } else { 1 };
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

#[cfg(test)]
mod catalog_tests {
    use super::*;
    use crate::{crafting::{recipe_for_catalog, recipe_identity_for_catalog, RecipeId}, gameplay_catalog::{Catalog, Profile}};

    #[test]
    fn slab_fill_repeats_then_fills_all_without_losing_ingredients() {
        let catalog = Catalog::new(Profile::Preview26_3);
        let recipe = recipe_for_catalog(catalog, RecipeId::WhiteConcreteSlab).unwrap();
        let mut inventory = Inventory::new();
        inventory.set(36, Some(Stack::new(615, 12)));
        let mut grid = vec![None; 9];
        assert!(place_for_catalog(catalog, recipe, &mut grid, &mut inventory, false));
        assert_eq!(grid[3..6], [Some(Stack::new(615, 1)); 3]);
        assert_eq!(inventory.slot(36), Some(Stack::new(615, 9)));
        assert!(place_for_catalog(catalog, recipe, &mut grid, &mut inventory, false));
        assert_eq!(grid[3..6], [Some(Stack::new(615, 2)); 3]);
        assert_eq!(inventory.slot(36), Some(Stack::new(615, 6)));
        assert!(place_for_catalog(catalog, recipe, &mut grid, &mut inventory, true));
        assert_eq!(grid[3..6], [Some(Stack::new(615, 4)); 3]);
        assert!(inventory.slot(36).is_none());
        assert_eq!(recipe_identity_for_catalog(catalog, &grid), Some(RecipeId::WhiteConcreteSlab));
        let base = Catalog::new(Profile::Java26_1_2);
        assert!(recipe_for_catalog(base, RecipeId::WhiteConcreteSlab).is_none());
        let before = grid.clone();
        assert!(!place_for_catalog(base, recipe, &mut grid, &mut inventory, true));
        assert_eq!(grid, before);
        assert!(recipe_identity_for_catalog(base, &grid).is_none());
        let mut small = vec![None; 4];
        inventory.set(36, Some(Stack::new(615, 3)));
        assert!(!place_for_catalog(catalog, recipe, &mut small, &mut inventory, false));
        assert_eq!(inventory.slot(36), Some(Stack::new(615, 3)));
        assert!(small.iter().all(Option::is_none));
    }
}
