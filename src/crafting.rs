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
pub enum RecipeId { Native(usize), WhiteConcreteSlab, ConcreteSlab(u8), WoolSlab(u8), ConcreteStairs(u8), WoolStairs(u8), Mineral(u8), Poplar(u8) }

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

/// Ступени 26.3: «X.. / XX. / XXX» из шести блоков своего цвета дают 4 ступени
/// (наблюдение официального 26.3: обе зеркальные формы принимаются).
fn stairs_recipes(rows: &'static [crate::gameplay_catalog::SlabDescriptor], cells: &'static std::sync::OnceLock<Vec<[&'static [i32]; 9]>>, recipes: &'static std::sync::OnceLock<Vec<Recipe>>) -> &'static [Recipe] {
    let cells = cells.get_or_init(|| rows.iter().map(|row| {
        let x: &'static [i32] = std::slice::from_ref(&row.input_item);
        [x, &[], &[], x, x, &[], x, x, x]
    }).collect());
    recipes.get_or_init(|| rows.iter().zip(cells).map(|(row, cells)| Recipe {
        shaped: true, width: 3, height: 3, result: row.item, count: 4, cells,
    }).collect())
}

pub fn concrete_stairs_recipes() -> &'static [Recipe] {
    static CELLS: std::sync::OnceLock<Vec<[&'static [i32]; 9]>> = std::sync::OnceLock::new();
    static RECIPES: std::sync::OnceLock<Vec<Recipe>> = std::sync::OnceLock::new();
    stairs_recipes(&crate::gameplay_catalog::CONCRETE_STAIRS, &CELLS, &RECIPES)
}

pub fn wool_stairs_recipes() -> &'static [Recipe] {
    static CELLS: std::sync::OnceLock<Vec<[&'static [i32]; 9]>> = std::sync::OnceLock::new();
    static RECIPES: std::sync::OnceLock<Vec<Recipe>> = std::sync::OnceLock::new();
    stairs_recipes(&crate::gameplay_catalog::WOOL_STAIRS, &CELLS, &RECIPES)
}

/// Рецепты серы и киновари 26.3 (разбор рецептов pinned recipes.json 777 + вики Sulfur/Cinnabar):
/// сера — 2×2 шипа → 1; мощная сера — 9 серы без формы; плита 3→6; ступени «X.. / XX. / XXX» 6→4;
/// стенка 2 ряда по 3 → 6; полированный 2×2 → 4; кирпичи 2×2 полированных → 4;
/// резной — две плиты столбиком → 1. У киновари и шипа рецепта крафта нет.
/// Возвращает пары (индекс строки MINERALS, рецепт).
pub fn mineral_recipes() -> &'static [(u8, Recipe)] {
    static ALL: std::sync::OnceLock<Vec<(u8, Recipe)>> = std::sync::OnceLock::new();
    ALL.get_or_init(|| {
        use crate::gameplay_catalog::{BlockForm, MINERALS};
        let leak = |item: i32| -> &'static [i32] { Box::leak(Box::new([item])) };
        let mut all = Vec::new();
        for (index, row) in MINERALS.iter().enumerate() {
            if row.input_name.is_empty() { continue; }
            let x = leak(row.input_item);
            let empty: &'static [i32] = &[];
            let (shaped, width, height, count, cells): (bool, u8, u8, i32, Vec<&'static [i32]>) = match row.form {
                BlockForm::PotentSulfur => (false, 9, 1, 1, vec![x; 9]),
                BlockForm::Slab => (true, 3, 1, 6, vec![x; 3]),
                BlockForm::Stairs => (true, 3, 3, 4, vec![x, empty, empty, x, x, empty, x, x, x]),
                BlockForm::Wall => (true, 3, 2, 6, vec![x; 6]),
                BlockForm::Full if row.name.starts_with("chiseled_") => (true, 1, 2, 1, vec![x; 2]),
                // Сера из шипов — один блок; полированные и кирпичи — по 4.
                BlockForm::Full if row.name == "sulfur" => (true, 2, 2, 1, vec![x; 4]),
                BlockForm::Full => (true, 2, 2, 4, vec![x; 4]),
                BlockForm::Spike => continue,
                BlockForm::Wood | BlockForm::Plant | BlockForm::Boat | BlockForm::Item => unreachable!("в MINERALS нет блоков тополя"),
            };
            all.push((index as u8, Recipe { shaped, width, height, result: row.item, count, cells: Box::leak(cells.into_boxed_slice()) }));
        }
        all
    })
}

/// Рецепт тополя с метаданными книги рецептов.
pub struct PoplarRecipe { pub row: u8, pub recipe: Recipe, pub group: u8, pub category: u8 }

/// Рецепты тополя 26.3: снято с официального сервера (`probe_poplar_777.py`, recipe_book_add):
/// доски — из любого ствола тополя (тег poplar_logs) 1→4; древесина 2×2 → 3 (и обтёсанная);
/// ступени 6→4, плита 3→6, забор 3→ (доски/палка) 3, калитка 1, дверь 2×3 → 3, люк 2×3 → 2,
/// кнопка 1, нажимная плита 2→1, табличка 3 (шесть досок и палка), подвесная 6 (две цепи и шесть
/// обтёсанных брёвен), полка 6 (обтёсанные брёвна, ряды 1 и 3). Лодки (poplar_boat, poplar_chest_boat) — в конце
/// таблицы (сущности: minecart/boat.rs). Группа/категория: категории сняты,
/// номера групп локальные (общая группа с дубом не выставляется, как у прежних записей).
pub fn poplar_recipes() -> &'static [PoplarRecipe] {
    use crate::gameplay_catalog::POPLAR;
    static ALL: std::sync::OnceLock<Vec<PoplarRecipe>> = std::sync::OnceLock::new();
    ALL.get_or_init(|| {
        let item = |name: &str| crate::blocks::item_named(name).unwrap_or_else(|| panic!("нет предмета {name}"));
        let one = |name: &str| -> &'static [i32] { Box::leak(Box::new([item(name)])) };
        let (planks, stick, chain) = (one("poplar_planks"), one("stick"), one("iron_chain"));
        let (log, stripped) = (one("poplar_log"), one("stripped_poplar_log"));
        let any_log: &'static [i32] = Box::leak(Box::new([item("poplar_log"), item("poplar_wood"), item("stripped_poplar_log"), item("stripped_poplar_wood")]));
        let empty: &'static [i32] = &[];
        let (p, s, c, k) = (planks, stick, chain, stripped);
        // (результат, форма: shaped, ширина, высота, количество, клетки, группа, категория)
        type Row = (&'static str, bool, u8, u8, i32, Vec<&'static [i32]>, u8, u8);
        let table: Vec<Row> = vec![
            ("poplar_planks", false, 1, 1, 4, vec![any_log], 8, 0),
            ("poplar_wood", true, 2, 2, 3, vec![log; 4], 15, 0),
            ("stripped_poplar_wood", true, 2, 2, 3, vec![stripped; 4], 15, 0),
            ("poplar_stairs", true, 3, 3, 4, vec![p, empty, empty, p, p, empty, p, p, p], 13, 0),
            ("poplar_slab", true, 3, 1, 6, vec![p; 3], 12, 0),
            ("poplar_fence", true, 3, 2, 3, vec![p, s, p, p, s, p], 5, 3),
            ("poplar_fence_gate", true, 3, 2, 1, vec![s, p, s, s, p, s], 6, 1),
            ("poplar_door", true, 2, 3, 3, vec![p; 6], 4, 1),
            ("poplar_trapdoor", true, 3, 2, 2, vec![p; 6], 14, 1),
            ("poplar_button", false, 1, 1, 1, vec![p], 2, 1),
            ("poplar_pressure_plate", true, 2, 1, 1, vec![p; 2], 9, 1),
            ("poplar_sign", true, 3, 3, 3, vec![p, p, p, p, p, p, empty, s, empty], 11, 3),
            ("poplar_hanging_sign", true, 3, 3, 6, vec![c, empty, c, k, k, k, k, k, k], 7, 3),
            ("poplar_shelf", true, 3, 3, 6, vec![k, k, k, empty, empty, empty, k, k, k], 10, 3),
            // Лодки: пять досок буквой «U» в 3×2 (наблюдение 26.3, как у дубовой лодки); с сундуком — лодка + сундук.
            ("poplar_boat", true, 3, 2, 1, vec![p, empty, p, p, p, p], 16, 3),
            ("poplar_chest_boat", false, 2, 1, 1, vec![one("chest"), one("poplar_boat")], 17, 3),
            // Соломенная кровать: три стога сена в ряд → 4 (наблюдение 26.3, probe_furnish_777.py; без группы, категория 3).
            ("straw_bed", true, 3, 1, 4, vec![one("hay_block"); 3], 0, 3),
        ];
        let mut table = table;
        // Подушки: три плиты шерсти своего цвета в ряд → 1 (наблюдение 26.3; группа общая, категория 3).
        // [проверить] перекраска подушки красителем: рецепт в книге не снят (вики: «dyed by combining with dye»).
        for color in ["white", "orange", "magenta", "light_blue", "yellow", "lime", "pink", "gray", "light_gray", "cyan", "purple", "blue", "brown", "green", "red", "black"] {
            let wool_slab = crate::gameplay_catalog::WOOL_SLABS.iter().find(|row| row.name == format!("{color}_wool_slab")).expect("плита шерсти").item;
            let (name, slab): (&'static str, &'static [i32]) = (Box::leak(format!("{color}_cushion").into_boxed_str()), Box::leak(Box::new([wool_slab])));
            table.push((name, true, 3, 1, 1, vec![slab; 3], 18, 3));
        }
        table.into_iter().map(|(name, shaped, width, height, count, cells, group, category)| {
            let row = POPLAR.iter().position(|row| row.name == name).expect("строка тополя") as u8;
            PoplarRecipe { row, recipe: Recipe { shaped, width, height, result: item(name), count, cells: Box::leak(cells.into_boxed_slice()) }, group, category }
        }).collect()
    })
}

/// Грибные супы 26.3: вместо «бурый + красный» годятся любые два гриба из трёх (бурый, красный, полочный),
/// в кроличье рагу — любой один. Снято с официального сервера 26.3 крафтером: миска + любые 2 гриба —
/// грибной суп; миска + цветок + любые 2 гриба — подозрительный суп; один полочный гриб в рагу — рагу;
/// миска и один гриб, три гриба — рецепта нет. Сводится к прежнему рецепту подменой: первый гриб бурый,
/// второй красный. Возвращает подменённую сетку только если она изменилась.
fn mushroom_swap(slots: &[Option<Stack>]) -> Option<Vec<Option<Stack>>> {
    let (shelf, brown, red) = (crate::blocks::item_named("shelf_mushroom")?, crate::blocks::item_named("brown_mushroom")?, crate::blocks::item_named("red_mushroom")?);
    let at: Vec<usize> = (0..slots.len()).filter(|&index| slots[index].is_some_and(|stack| [shelf, brown, red].contains(&stack.item))).collect();
    if at.is_empty() || at.len() > 2 { return None; }
    let mut swapped = slots.to_vec();
    for (order, &index) in at.iter().enumerate() {
        if let Some(stack) = swapped[index].as_mut() { stack.item = if order == 0 { brown } else { red }; }
    }
    (swapped != slots).then_some(swapped)
}

/// Результат грибного супа после подмены гриба (только суп: другие рецепты с грибом подменой не получаем).
fn mushroom_soup(slots: &[Option<Stack>]) -> Option<(Vec<Option<Stack>>, Stack)> {
    let swapped = mushroom_swap(slots)?;
    let result = craft(&swapped)?;
    matches!(crate::blocks::item_name(result.item), Some("mushroom_stew" | "suspicious_stew" | "rabbit_stew")).then_some((swapped, result))
}

/// Доски тополя годятся везде, где рецепт принимает любые доски (тег planks): подменяем их дубовыми,
/// ищем прежний рецепт, и принимаем лишь если все доски тополя легли в клетки-теги.
fn poplar_planks_swap(slots: &[Option<Stack>]) -> Option<(usize, Vec<Option<Stack>>)> {
    let (planks, oak) = (crate::blocks::item_named("poplar_planks")?, crate::blocks::item_named("oak_planks")?);
    let used = slots.iter().flatten().filter(|stack| stack.item == planks).count();
    if used == 0 { return None; }
    let swapped: Vec<Option<Stack>> = slots.iter().map(|slot| slot.map(|mut stack| { if stack.item == planks { stack.item = oak; } stack })).collect();
    let index = recipe_index(&swapped)?;
    let tag_cells = recipes()[index].cells.iter().filter(|cell| cell.len() >= 12 && cell.contains(&oak)).count();
    (used <= tag_cells).then_some((index, swapped))
}

pub fn recipe_for_catalog(catalog: crate::gameplay_catalog::Catalog, id: RecipeId) -> Option<&'static Recipe> {
    if let RecipeId::Native(index) = id { return recipes().get(index); }
    if catalog.profile() != crate::gameplay_catalog::Profile::Preview26_3 { return None; }
    match id {
        RecipeId::WoolSlab(index) => wool_slab_recipes().get(usize::from(index)),
        RecipeId::ConcreteStairs(index) => concrete_stairs_recipes().get(usize::from(index)),
        RecipeId::WoolStairs(index) => wool_stairs_recipes().get(usize::from(index)),
        RecipeId::Mineral(index) => mineral_recipes().iter().find(|(row, _)| *row == index).map(|(_, recipe)| recipe),
        RecipeId::Poplar(index) => poplar_recipes().get(usize::from(index)).map(|entry| &entry.recipe),
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
        if let Some(index) = concrete_stairs_recipes().iter().position(|recipe| recipe.matches(&grid)) {
            return Some(RecipeId::ConcreteStairs(index as u8));
        }
        if let Some(index) = wool_stairs_recipes().iter().position(|recipe| recipe.matches(&grid)) {
            return Some(RecipeId::WoolStairs(index as u8));
        }
        if let Some((row, _)) = mineral_recipes().iter().find(|(_, recipe)| recipe.matches(&grid)) {
            return Some(RecipeId::Mineral(*row));
        }
        if let Some(index) = poplar_recipes().iter().position(|entry| entry.recipe.matches(&grid)) {
            return Some(RecipeId::Poplar(index as u8));
        }
        if let Some((index, _)) = poplar_planks_swap(slots) { return Some(RecipeId::Native(index)); }
        if let Some((swapped, _)) = mushroom_soup(slots) && let Some(index) = recipe_index(&swapped) { return Some(RecipeId::Native(index)); }
    }
    recipe_index(slots).map(RecipeId::Native)
}

/// Рецепты каталога 26.3: плиты, ступени, затем сера и киноварь.
pub fn catalog_recipes() -> impl Iterator<Item = &'static Recipe> {
    concrete_slab_recipes().iter().chain(wool_slab_recipes()).chain(concrete_stairs_recipes()).chain(wool_stairs_recipes())
        .chain(mineral_recipes().iter().map(|(_, recipe)| recipe))
        .chain(poplar_recipes().iter().map(|entry| &entry.recipe))
}

/// Каталог основного профиля общий для верстака, книги и автокрафтера.
/// Базовые специальные рецепты сохраняют прежний порядок проверки.
pub fn craft_for_catalog(catalog: crate::gameplay_catalog::Catalog, slots: &[Option<Stack>]) -> Option<Stack> {
    if catalog.profile() == crate::gameplay_catalog::Profile::Preview26_3
        && let Some(grid) = Grid::new(slots)
        && let Some(recipe) = catalog_recipes().find(|recipe| recipe.matches(&grid))
    { return Some(Stack::new(recipe.result, recipe.count)); }
    if catalog.profile() == crate::gameplay_catalog::Profile::Preview26_3
        && let Some((_, swapped)) = poplar_planks_swap(slots)
    { return craft(&swapped); }
    if catalog.profile() == crate::gameplay_catalog::Profile::Preview26_3
        && craft(slots).is_none()
        && let Some((_, result)) = mushroom_soup(slots)
    { return Some(result); }
    let result = craft(slots)?;
    if catalog.changed_rules() && crate::blocks::item_name(result.item) == Some("decorated_pot") {
        let mut data = result.extras();
        // [проверить] Выбор simple/декорированного рецепта при одинаковой сетке кирпичей.
        if slots.iter().flatten().all(|s| crate::blocks::item_name(s.item) == Some("brick") && s.extras() == crate::stackdata::Data::default()) {
            data.pot_decorations = Some([0;4]);
        } else if slots.len() == 9 {
            data.pot_sides = [1,3,5,7].into_iter().enumerate().filter_map(|(slot,i)| {
                let stack = slots[i]?;
                Some(crate::stackdata::Content { slot: slot as u8, item: stack.stored_item(), count: 1, data: stack.data })
            }).collect();
        }
        return Some(result.with_extras(data));
    }
    Some(result)
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
