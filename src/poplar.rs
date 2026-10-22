// Тополь 26.3 (poplar): блоки каталога Preview26_3 с дубовым двойником.
//
// Свойства, порядок состояний и форма каждого блока тополя совпадают с дубовыми (проверено по
// pinned blocks.json 777 при построении таблицы: имена и значения свойств равны, число состояний
// равно). Поэтому таблицы по номеру состояния (твёрдость, свет, форма, выпадение, взрыв) берутся
// у дубового двойника со смещением, а имя и правила установки — свои. Так поведение,
// завязанное на имя («_door», «_log», «_leaves»), работает для тополя без отдельной копии кода.
// Источник набора блоков — вики 26.3 (Poplar); значения проверены наблюдением `probe_poplar_777.py`.
use crate::blocks::{self, Family, Orientation, Property};
use crate::gameplay_catalog::{BlockForm, SlabDescriptor, POPLAR};

/// Первое состояние тополя: всё, что меньше, быстро отсекается.
fn first_state() -> i32 { POPLAR[0].first_state }

fn last_state() -> i32 { POPLAR[POPLAR.len() - 1].last_state }

/// Строка тополя по номеру состояния (строки идут по возрастанию состояний).
pub fn row_of_state(state: i32) -> Option<&'static SlabDescriptor> {
    if state < first_state() || state > last_state() { return None; }
    let index = POPLAR.partition_point(|row| row.last_state < state);
    POPLAR.get(index).filter(|row| row.first_state <= state)
}

pub fn row_named(name: &str) -> Option<&'static SlabDescriptor> {
    POPLAR.iter().find(|row| row.name == name)
}

/// То же, но только блоки (у лодок блока нет).
pub fn block_row_named(name: &str) -> Option<&'static SlabDescriptor> {
    row_named(name).filter(|row| row.has_block())
}

/// Свойства полочного гриба 26.3 (blocks.json 777): age 0–1, facing north/south/west/east.
const SHELF_PROPERTIES: &[Property] = &[
    Property { name: "age", values: &["0", "1"], default: "0" },
    Property { name: "facing", values: &["north", "south", "west", "east"], default: "north" },
];

/// Первые состояния дубовых двойников, по порядку строк `POPLAR`.
fn twin_firsts() -> &'static [i32] {
    static FIRSTS: std::sync::OnceLock<Vec<i32>> = std::sync::OnceLock::new();
    FIRSTS.get_or_init(|| POPLAR.iter().map(|row| if row.form == BlockForm::Boat { 0 } else { blocks::states_of(row.input_name).expect("дубовый двойник").0 }).collect())
}

/// Дубовое состояние с тем же смещением; для остальных состояний — без изменений.
pub fn twin_state(state: i32) -> i32 {
    match row_of_state(state) {
        Some(row) => {
            let index = POPLAR.iter().position(|candidate| std::ptr::eq(candidate, row)).expect("строка");
            // Растения без дубового двойника делят одно состояние красного гриба.
            let offset = if row.form == BlockForm::Plant { 0 } else { state - row.first_state };
            twin_firsts()[index] + offset
        }
        None => state,
    }
}

/// Превращает предмет дубового двойника в предмет тополя: «oak_sapling» → «poplar_sapling».
/// Предметы без дубовой пары (палка, цепь) остаются прежними.
pub fn retarget_item(item: i32) -> i32 {
    let Some(name) = blocks::item_name(item) else { return item; };
    if !name.contains("oak") { return item; }
    let poplar = name.replace("oak", "poplar");
    row_named(&poplar).filter(|row| row.has_item()).map_or(item, |row| row.item)
}

/// Предмет выпадения блока каталога: растения 26.3 выпадают собой, остальное — как дубовый двойник.
pub fn drop_item(state: i32, twin_item: i32) -> i32 {
    match row_of_state(state) {
        Some(row) if row.form == BlockForm::Plant => row.item,
        _ => retarget_item(twin_item),
    }
}

pub fn name_of_item(item: i32) -> Option<&'static str> {
    POPLAR.iter().find(|row| row.has_item() && row.item == item).map(|row| row.name)
}

/// Блок для предмета тополя: имя и состояние по умолчанию.
pub fn block_for_item(item: i32) -> Option<(&'static str, i32)> {
    POPLAR.iter().find(|row| row.has_item() && row.has_block() && row.item == item).map(|row| (row.name, row.default_state))
}

/// Правила установки тополя: свойства дубового двойника под своим именем и номерами состояний.
pub fn orientation(name: &str) -> Option<&'static Orientation> {
    static RULES: std::sync::OnceLock<Vec<Option<Orientation>>> = std::sync::OnceLock::new();
    let rules = RULES.get_or_init(|| POPLAR.iter().map(|row| {
        if row.name == "shelf_mushroom" {
            return Some(Orientation { name: row.name, min: row.first_state, max: row.last_state, family: Family::Other, properties: SHELF_PROPERTIES });
        }
        if matches!(row.form, BlockForm::Plant | BlockForm::Boat) { return None; }
        blocks::orientation(row.input_name).map(|twin| Orientation {
            name: row.name, min: row.first_state, max: row.last_state,
            family: twin.family, properties: twin.properties,
        })
    }).collect());
    let index = POPLAR.iter().position(|row| row.name == name)?;
    rules[index].as_ref()
}

pub fn orientation_of(state: i32) -> Option<&'static Orientation> {
    orientation(row_of_state(state)?.name)
}

/// Предмет таблички тополя несёт по 16 штук в стопке, остальные — по 64 (как у дубовых).
pub fn stack_size(item: i32) -> Option<i32> {
    let row = POPLAR.iter().find(|row| row.has_item() && row.item == item)?;
    match row.form {
        BlockForm::Boat => return Some(1),
        BlockForm::Plant => return Some(64),
        _ => {}
    }
    let twin = blocks::item_named(row.input_name)?;
    Some(blocks::stack_size(twin))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_poplar_state_has_a_twin_with_the_same_properties() {
        for row in POPLAR.iter().filter(|row| !matches!(row.form, BlockForm::Plant | BlockForm::Boat)) {
            let (first, last) = blocks::states_of(row.input_name).expect("двойник");
            assert_eq!(last - first, row.last_state - row.first_state, "{}", row.name);
            for state in row.first_state..=row.last_state {
                let twin = twin_state(state);
                assert_eq!(twin, first + (state - row.first_state));
                assert_eq!(blocks::block_at_state(state), Some(row.name));
                let (poplar_rule, oak_rule) = (blocks::orientation_of(state), blocks::orientation_of(twin));
                assert_eq!(poplar_rule.is_some(), oak_rule.is_some(), "{}", row.name);
                if let (Some(poplar), Some(oak)) = (poplar_rule, oak_rule) {
                    for property in oak.properties {
                        assert_eq!(poplar.value(state, property.name), oak.value(twin, property.name), "{} {}", row.name, property.name);
                    }
                }
            }
        }
        assert_eq!(twin_state(0), 0);
    }

    #[test]
    fn items_blocks_and_stacks_follow_the_oak_twins() {
        assert_eq!(blocks::item_named("poplar_planks"), Some(POPLAR[0].item));
        assert_eq!(blocks::item_name(POPLAR[0].item), Some("poplar_planks"));
        assert_eq!(blocks::state_by_name("poplar_planks"), Some(POPLAR[0].default_state));
        assert_eq!(blocks::item_for_block("poplar_planks"), Some(POPLAR[0].item));
        assert_eq!(blocks::block_for_item(POPLAR[0].item), Some(("poplar_planks", POPLAR[0].default_state)));
        assert_eq!(blocks::stack_size(blocks::item_named("poplar_sign").unwrap()), 16);
        assert_eq!(blocks::stack_size(blocks::item_named("poplar_hanging_sign").unwrap()), 16);
        assert_eq!(blocks::stack_size(blocks::item_named("poplar_door").unwrap()), 64);
        // У настенной таблички и горшка предмета нет.
        assert_eq!(blocks::item_named("poplar_wall_sign"), None);
        assert_eq!(blocks::item_for_block("potted_poplar_sapling"), None);
        assert_eq!(retarget_item(blocks::item_named("oak_sapling").unwrap()), blocks::item_named("poplar_sapling").unwrap());
        assert_eq!(retarget_item(blocks::item_named("stick").unwrap()), blocks::item_named("stick").unwrap());
    }
}
