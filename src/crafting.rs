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
mod special;
mod tags;
pub mod player;
pub use special::remainders;

/// Один рецепт верстака.
#[derive(Clone)]
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
    tags::recipes()
}

impl Recipe {
    /// Пачка крафтов: поток от стопок к ингредиентам, без предположения,
    /// что каждый раз использована та же порода дерева (вики, Crafting).
    pub fn covered_counts(&self, items: &[(i32, i32)], times: i32) -> bool {
        if !(1..=64).contains(&times) || items.iter().any(|(_,n)| *n <= 0) { return false; }
        let cells: Vec<_> = self.cells.iter().filter(|c| !c.is_empty()).collect();
        let need = cells.len() as i64 * i64::from(times);
        if items.iter().map(|(_,n)| i64::from(*n)).sum::<i64>() != need { return false; }
        let end = items.len() + cells.len() + 1;
        let mut edges = vec![vec![0; end + 1]; end + 1];
        for (i,(item,count)) in items.iter().enumerate() {
            edges[0][i + 1] = *count;
            for (j,cell) in cells.iter().enumerate() { if cell.contains(item) { edges[i + 1][items.len() + j + 1] = times; } }
        }
        for j in 0..cells.len() { edges[items.len() + j + 1][end] = times; }
        let mut sent = 0;
        loop {
            let mut parent = vec![usize::MAX; end + 1]; parent[0] = 0;
            let mut queue = std::collections::VecDeque::from([0]);
            while let Some(from) = queue.pop_front() {
                for (to, prev) in parent.iter_mut().enumerate() { if *prev == usize::MAX && edges[from][to] > 0 { *prev = from; queue.push_back(to); } }
            }
            if parent[end] == usize::MAX { return i64::from(sent) == need; }
            let mut amount = i32::MAX; let mut at = end;
            while at != 0 { amount = amount.min(edges[parent[at]][at]); at = parent[at]; }
            at = end;
            while at != 0 { let from = parent[at]; edges[from][at] -= amount; edges[at][from] += amount; at = from; }
            sent += amount;
        }
    }

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

/// Идентичность рецепта сервера: сетевые display ID принадлежат книге отдельно.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecipeId { Native(usize), WhiteConcreteSlab, ConcreteSlab(u8), WoolSlab(u8) }

impl RecipeId {
    pub fn concrete_slab(index: usize) -> Option<Self> {
        if index >= crate::gameplay_catalog::CONCRETE_SLABS.len() { return None; }
        Some(if index == 0 { Self::WhiteConcreteSlab } else { Self::ConcreteSlab(index as u8) })
    }
    pub fn slab_index(self) -> Option<usize> {
        match self {
            Self::WhiteConcreteSlab => Some(0),
            Self::ConcreteSlab(index) if index > 0 && usize::from(index) < crate::gameplay_catalog::CONCRETE_SLABS.len() => Some(usize::from(index)),
            _ => None,
        }
    }
}

/// Статические ссылки на ингредиенты общей таблицы: без дублирования ID цветов.
fn slab_recipes(rows: &'static [crate::gameplay_catalog::SlabDescriptor], cells: &'static std::sync::OnceLock<Vec<[&'static [i32]; 3]>>, recipes: &'static std::sync::OnceLock<Vec<Recipe>>) -> &'static [Recipe] {
    let cells = cells.get_or_init(|| rows.iter().map(|row| [std::slice::from_ref(&row.input_item); 3]).collect());
    recipes.get_or_init(|| rows.iter().zip(cells).map(|(row, cells)| Recipe {
        shaped: true, width: 3, height: 1, result: row.item, count: 6, cells,
    }).collect())
}

pub fn concrete_slab_recipes() -> &'static [Recipe] {
    static CELLS: std::sync::OnceLock<Vec<[&'static [i32]; 3]>> = std::sync::OnceLock::new();
    static RECIPES: std::sync::OnceLock<Vec<Recipe>> = std::sync::OnceLock::new();
    slab_recipes(&crate::gameplay_catalog::CONCRETE_SLABS, &CELLS, &RECIPES)
}

pub fn wool_slab_recipes() -> &'static [Recipe] {
    static CELLS: std::sync::OnceLock<Vec<[&'static [i32]; 3]>> = std::sync::OnceLock::new();
    static RECIPES: std::sync::OnceLock<Vec<Recipe>> = std::sync::OnceLock::new();
    slab_recipes(&crate::gameplay_catalog::WOOL_SLABS, &CELLS, &RECIPES)
}

pub fn recipe_for_catalog(catalog: crate::gameplay_catalog::Catalog, id: RecipeId) -> Option<&'static Recipe> {
    if let RecipeId::Native(index) = id { return recipes().get(index); }
    if catalog.profile() != crate::gameplay_catalog::Profile::Preview26_3 { return None; }
    match id {
        RecipeId::WoolSlab(index) => wool_slab_recipes().get(usize::from(index)),
        _ => concrete_slab_recipes().get(id.slab_index()?),
    }
}

pub fn recipe_identity_for_catalog(catalog: crate::gameplay_catalog::Catalog, slots: &[Option<Stack>]) -> Option<RecipeId> {
    if catalog.profile() == crate::gameplay_catalog::Profile::Preview26_3
        && let Some(grid) = Grid::new(slots)
    {
        if let Some(index) = concrete_slab_recipes().iter().position(|recipe| recipe.matches(&grid)) {
            return RecipeId::concrete_slab(index);
        }
        if let Some(index) = wool_slab_recipes().iter().position(|recipe| recipe.matches(&grid)) {
            return Some(RecipeId::WoolSlab(index as u8));
        }
    }
    recipe_index(slots).map(RecipeId::Native)
}

/// Каталог основного профиля общий для верстака, книги и автокрафтера.
/// Базовые специальные рецепты сохраняют прежний порядок проверки.
pub fn craft_for_catalog(catalog: crate::gameplay_catalog::Catalog, slots: &[Option<Stack>]) -> Option<Stack> {
    if catalog.profile() == crate::gameplay_catalog::Profile::Preview26_3
        && let Some(grid) = Grid::new(slots)
        && let Some(recipe) = concrete_slab_recipes().iter().chain(wool_slab_recipes()).find(|recipe| recipe.matches(&grid))
    { return Some(Stack::new(recipe.result, recipe.count)); }
    craft(slots)
}

/// Что получится из сетки 3×3 (ячейки по рядам, слева направо): первый
/// подходящий рецепт. None — сетка пуста или рецепта нет.
pub fn craft(slots: &[Option<Stack>]) -> Option<Stack> {
    if let Some(result) = special::craft(slots) { return Some(result); }
    let ingredients:Vec<_>=slots.iter().flatten().copied().collect();
    if ingredients.len()==2 {
        let shield=ingredients.iter().find(|s|crate::blocks::item_name(s.item)==Some("shield"));
        let banner=ingredients.iter().find(|s|crate::loom::may_place(0,s));
        if let (Some(shield),Some(banner))=(shield,banner) {
            let mut data=shield.extras(); if data.base_color.is_some() || !data.banner_patterns.is_empty() {return None;}
            let color=crate::blocks::item_name(banner.item)?.strip_suffix("_banner")?;
            data.base_color=crate::sign::COLORS.iter().position(|c|*c==color).map(|c|c as u8);
            let layers=banner.extras();
            data.banner_patterns=if layers.ominous_banner {crate::stackdata::OMINOUS_BANNER_LAYERS.iter().map(|&(p,c)|(p as u8,c as u8)).collect()} else {layers.banner_patterns};return Some(shield.with_count(1).with_extras(data));
        }
    }
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

    recipes()
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

/// Номер обычного рецепта для книги и ограниченного крафта.
pub fn recipe_index(slots: &[Option<Stack>]) -> Option<usize> {
    let grid = Grid::new(slots)?;
    recipes().iter().position(|recipe| recipe.matches(&grid))
}

#[cfg(test)]
#[path = "crafting/tests.rs"]
mod player_tests;

#[cfg(test)]
mod concrete_slab_tests {
    use super::*;
    use crate::gameplay_catalog::{Catalog, Profile, CONCRETE_SLABS};

    #[test]
    fn all_colors_match_only_their_own_horizontal_recipe_and_profile() {
        let preview = Catalog::new(Profile::Preview26_3);
        let base = Catalog::new(Profile::Java26_1_2);
        assert_eq!(RecipeId::concrete_slab(0), Some(RecipeId::WhiteConcreteSlab));
        assert!(RecipeId::concrete_slab(16).is_none());
        assert!(recipe_for_catalog(preview, RecipeId::ConcreteSlab(0)).is_none());
        assert!(recipe_for_catalog(preview, RecipeId::ConcreteSlab(255)).is_none());
        for (index, row) in CONCRETE_SLABS.iter().enumerate() {
            let id = RecipeId::concrete_slab(index).unwrap();
            assert_eq!(id.slab_index(), Some(index));
            assert_eq!(recipe_for_catalog(preview, id).unwrap().result, row.item);
            assert!(recipe_for_catalog(base, id).is_none());
            for y in 0..3 {
                let mut slots = [None; 9];
                for x in 0..3 { slots[y * 3 + x] = Some(Stack::new(row.input_item, x as i32 + 1)); }
                assert_eq!(recipe_identity_for_catalog(preview, &slots), Some(id));
                assert_eq!(craft_for_catalog(preview, &slots), Some(Stack::new(row.item, 6)));
                assert!(craft_for_catalog(base, &slots).is_none());
                slots[y * 3 + 1] = Some(Stack::new(CONCRETE_SLABS[(index + 1) % 16].input_item, 1));
                assert!(craft_for_catalog(preview, &slots).is_none(), "смешанные цвета {}", row.name);
            }
            let mut vertical = [None; 9];
            for at in [0, 3, 6] { vertical[at] = Some(Stack::new(row.input_item, 1)); }
            assert!(craft_for_catalog(preview, &vertical).is_none());
        }
    }
}

#[cfg(test)]
mod wool_slab_tests {
    use super::*;
    use crate::gameplay_catalog::{Catalog, Profile, WOOL_SLABS, CONCRETE_SLABS};

    #[test]
    fn wool_recipes_require_one_color_in_a_row_and_preview_profile() {
        let preview = Catalog::new(Profile::Preview26_3);
        let base = Catalog::new(Profile::Java26_1_2);
        assert!(recipe_for_catalog(preview, RecipeId::WoolSlab(16)).is_none());
        assert!(recipe_for_catalog(preview, RecipeId::WoolSlab(255)).is_none());
        for (index, row) in WOOL_SLABS.iter().enumerate() {
            let id = RecipeId::WoolSlab(index as u8);
            assert_eq!(recipe_for_catalog(preview, id).unwrap().result, row.item);
            assert!(recipe_for_catalog(base, id).is_none());
            for y in 0..3 {
                let mut slots = [None; 9];
                for x in 0..3 { slots[y * 3 + x] = Some(Stack::new(row.input_item, x as i32 + 1)); }
                assert_eq!(recipe_identity_for_catalog(preview, &slots), Some(id));
                assert_eq!(craft_for_catalog(preview, &slots), Some(Stack::new(row.item, 6)));
                assert!(craft_for_catalog(base, &slots).is_none());
                for wrong in [WOOL_SLABS[(index + 1) % 16].input_item, CONCRETE_SLABS[index].input_item] {
                    slots[y * 3 + 1] = Some(Stack::new(wrong, 1));
                    assert!(craft_for_catalog(preview, &slots).is_none());
                }
            }
            let mut vertical = [None; 9];
            for at in [0, 3, 6] { vertical[at] = Some(Stack::new(row.input_item, 1)); }
            assert!(craft_for_catalog(preview, &vertical).is_none());
            assert!(craft_for_catalog(preview, &[Some(Stack::new(row.input_item, 3)); 4]).is_none());
        }
    }
}
