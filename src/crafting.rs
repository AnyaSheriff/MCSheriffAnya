// Рецепты верстака: что получится из сетки 3×3.
//
// Таблица рецептов (crafting/table.rs) создана из minecraft-data
// (data/pc/26.1/recipes.json) — это данные, не код. Там есть рецепты с
// формой (shaped) и без формы (shapeless); особых рецептов (окраска брони,
// фейерверки, копирование карт и книг, узоры щитов, починка) в таблице нет.
// [проверить]: у рецептов с тегом в одной клетке (например, «любые брёвна
// дуба» у досок) minecraft-data даёт только первый предмет тега.
//
// Как сетка сверяется с рецептом (вики, Crafting): лишние пустые ряды и
// столбцы по краям не считаются, рецепт с формой подходит и зеркально
// отражённым слева направо, у рецепта без формы важен только набор
// предметов.

use crate::inventory::Stack;

mod table;

/// Один рецепт верстака.
pub struct Recipe {
    /// С формой (важно, где что лежит) или без (важен только набор).
    pub shaped: bool,

    /// Ширина и высота формы; у рецепта без формы ширина — число предметов,
    /// высота — 1.
    pub width: u8,
    pub height: u8,

    /// Номер предмета-результата и сколько его выходит.
    pub result: i32,
    pub count: i32,

    /// Клетки формы по рядам (у рецепта без формы — предметы по одному):
    /// какие предметы годятся; пустой список — клетка должна быть пустой.
    pub cells: &'static [&'static [i32]],
}

impl Recipe {
    /// Подходит ли сетка (уже обрезанная до занятых рядов и столбцов).
    fn matches(&self, grid: &Grid) -> bool {
        if self.shaped {
            if grid.width != self.width as usize || grid.height != self.height as usize {
                return false;
            }

            self.matches_shape(grid, false) || self.matches_shape(grid, true)
        } else {
            self.matches_set(grid)
        }
    }

    fn matches_shape(&self, grid: &Grid, mirrored: bool) -> bool {
        (0..grid.height).all(|row| {
            (0..grid.width).all(|column| {
                let from = if mirrored { grid.width - 1 - column } else { column };
                let wanted = self.cells[row * grid.width + from];

                match grid.at(row, column) {
                    None => wanted.is_empty(),
                    Some(item) => wanted.contains(&item),
                }
            })
        })
    }

    /// Рецепт без формы: каждому предмету сетки — своя клетка рецепта.
    fn matches_set(&self, grid: &Grid) -> bool {
        let items: Vec<i32> = grid.items().collect();

        if items.len() != self.cells.len() {
            return false;
        }

        let mut used = vec![false; self.cells.len()];

        assign(&items, self.cells, &mut used)
    }
}

/// Перебор: раздать предметы по клеткам рецепта так, чтобы каждый подошёл.
fn assign(items: &[i32], cells: &[&[i32]], used: &mut [bool]) -> bool {
    let Some((&item, rest)) = items.split_first() else {
        return true;
    };

    for (index, wanted) in cells.iter().enumerate() {
        if used[index] || !wanted.contains(&item) {
            continue;
        }

        used[index] = true;

        if assign(rest, cells, used) {
            return true;
        }

        used[index] = false;
    }

    false
}

/// Сетка 3×3, обрезанная до занятых рядов и столбцов.
struct Grid {
    cells: [Option<i32>; 9],
    left: usize,
    top: usize,
    width: usize,
    height: usize,
}

impl Grid {
    fn new(slots: &[Option<Stack>]) -> Option<Self> {
        let mut cells = [None; 9];

        for (index, slot) in slots.iter().take(9).enumerate() {
            cells[index] = slot.map(|stack| stack.item);
        }

        let filled = |index: usize| cells[index].is_some();
        let rows: Vec<usize> = (0..3).filter(|&row| (0..3).any(|column| filled(row * 3 + column))).collect();
        let columns: Vec<usize> = (0..3).filter(|&column| (0..3).any(|row| filled(row * 3 + column))).collect();
        let (top, bottom) = (*rows.first()?, *rows.last()?);
        let (left, right) = (*columns.first()?, *columns.last()?);

        Some(Self { cells, left, top, width: right - left + 1, height: bottom - top + 1 })
    }

    fn at(&self, row: usize, column: usize) -> Option<i32> {
        self.cells[(self.top + row) * 3 + self.left + column]
    }

    fn items(&self) -> impl Iterator<Item = i32> + '_ {
        self.cells.iter().flatten().copied()
    }
}

/// Все рецепты по порядку таблицы: Bedrock нумерует их для клиента
/// (bedrock/stations.rs).
pub fn recipes() -> &'static [Recipe] {
    table::RECIPES
}

impl Recipe {
    /// Хватает ли этих предметов (по одному на клетку) на рецепт, без
    /// учёта формы: у Bedrock клиент сам раскладывает сетку, сервер
    /// сверяет израсходованное.
    pub fn covered_by(&self, items: &[i32]) -> bool {
        let wanted: Vec<&[i32]> = self.cells.iter().copied().filter(|cell| !cell.is_empty()).collect();

        if items.len() != wanted.len() {
            return false;
        }

        let mut used = vec![false; wanted.len()];

        assign(items, &wanted, &mut used)
    }
}

/// Что получится из сетки 3×3 (ячейки по рядам, слева направо): первый
/// подходящий рецепт. None — сетка пуста или рецепта нет.
pub fn craft(slots: &[Option<Stack>]) -> Option<Stack> {
    // Вики, Suspicious Stew: «flower used to craft it». Рецепт особый:
    // результат хранит эффект, а не только номер предмета [проверить].
    let names: Vec<&str> = slots.iter().flatten().filter_map(|stack| crate::blocks::item_name(stack.item)).collect();
    if names.len() == 4 && ["bowl", "brown_mushroom", "red_mushroom"].iter().all(|name| names.contains(name))
        && let Some(effect) = names.iter().find_map(|name| crate::survival::stew_effect(name))
    {
        let data = crate::stackdata::Data { stew: vec![effect], ..Default::default() };
        return Some(Stack::new(crate::blocks::item_named("suspicious_stew")?, 1).with_extras(data));
    }
    let grid = Grid::new(slots)?;

    table::RECIPES
        .iter()
        .find(|recipe| recipe.matches(&grid))
        .map(|recipe| Stack::new(recipe.result, recipe.count))
}

/// Что остаётся от ингредиента после крафта: пустое ведро от ведра с
/// молоком, водой, лавой или снегом, пустая бутылка от мёда (вики,
/// Crafter: «byproducts (e.g. empty bottles for honey blocks or empty
/// buckets for cake)»).
pub fn remainder(item: i32) -> Option<i32> {
    let name = crate::blocks::item_name(item)?;

    let left = match name {
        "milk_bucket" | "water_bucket" | "lava_bucket" | "powder_snow_bucket" => "bucket",
        "honey_bottle" | "dragon_breath" => "glass_bottle",
        _ => return None,
    };

    crate::blocks::item_named(left)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::blocks::item_named;

    #[test]
    fn suspicious_stew_crafting_keeps_the_flower_effect() {
        for flower in ["allium", "azure_bluet", "dandelion", "golden_dandelion", "blue_orchid", "closed_eyeblossom", "cornflower", "lily_of_the_valley", "oxeye_daisy", "poppy", "torchflower", "red_tulip", "orange_tulip", "white_tulip", "pink_tulip", "open_eyeblossom", "wither_rose"] {
            let slots = grid(["bowl", "", "red_mushroom", "", flower, "", "brown_mushroom", "", ""]);
            let stew = craft(&slots).unwrap();
            assert_eq!(stew.item, item_named("suspicious_stew").unwrap());
            assert_eq!(stew.extras().stew, vec![crate::survival::stew_effect(flower).unwrap()]);
            let mut extra = slots.clone();
            extra[1] = Some(Stack::new(item_named("stone").unwrap(), 1));
            assert!(craft(&extra).is_none());
        }
        assert!(craft(&grid(["bowl", "red_mushroom", "brown_mushroom", "sunflower", "", "", "", "", ""])).is_none());
    }

    fn grid(names: [&str; 9]) -> Vec<Option<Stack>> {
        names.iter().map(|name| (!name.is_empty()).then(|| Stack::new(item_named(name).unwrap(), 1))).collect()
    }

    fn named(stack: Option<Stack>) -> Option<(&'static str, i32)> {
        stack.map(|stack| (crate::blocks::item_name(stack.item).unwrap(), stack.count))
    }

    #[test]
    fn shaped_recipes_match_anywhere_and_mirrored() {
        let p = "oak_planks";

        // Верстак 2×2 в любом углу сетки.
        assert_eq!(named(craft(&grid([p, p, "", p, p, "", "", "", ""]))), Some(("crafting_table", 1)));
        assert_eq!(named(craft(&grid(["", "", "", "", p, p, "", p, p]))), Some(("crafting_table", 1)));

        // Разные доски в одном сундуке (тег «доски»).
        let b = "birch_planks";
        assert_eq!(named(craft(&grid([p, b, p, p, "", b, p, p, p]))), Some(("chest", 1)));

        // Ступени и их зеркало.
        let s = "stone";
        assert_eq!(named(craft(&grid([s, "", "", s, s, "", s, s, s]))), Some(("stone_stairs", 4)));
        assert_eq!(named(craft(&grid(["", "", s, "", s, s, s, s, s]))), Some(("stone_stairs", 4)));

        // Не та форма — ничего.
        assert_eq!(craft(&grid([p, "", "", "", p, "", "", "", p])), None);
        assert_eq!(craft(&grid([p; 9])), None);
        assert_eq!(craft(&grid([""; 9])), None);
    }

    #[test]
    fn shapeless_recipes_ignore_places() {
        assert_eq!(named(craft(&grid(["", "", "", "", "", "", "", "", "oak_log"]))), Some(("oak_planks", 4)));
        assert_eq!(named(craft(&grid(["", "oak_planks", "", "", "", "", "", "", ""]))), Some(("oak_button", 1)));
        assert_eq!(
            named(craft(&grid(["", "", "gunpowder", "", "", "", "paper", "", ""]))),
            Some(("firework_rocket", 3))
        );
        // Лишний предмет ломает рецепт.
        assert_eq!(craft(&grid(["oak_log", "stone", "", "", "", "", "", "", ""])), None);
    }

    #[test]
    fn cake_leaves_buckets() {
        let m = "milk_bucket";
        let cake = grid([m, m, m, "sugar", "egg", "sugar", "wheat", "wheat", "wheat"]);

        assert_eq!(named(craft(&cake)), Some(("cake", 1)));
        assert_eq!(remainder(item_named(m).unwrap()), item_named("bucket"));
        assert_eq!(remainder(item_named("honey_bottle").unwrap()), item_named("glass_bottle"));
        assert_eq!(remainder(item_named("wheat").unwrap()), None);
    }
}
