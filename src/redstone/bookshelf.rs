// Резной книжный шкаф (chiseled bookshelf): шесть ячеек под книги.
//
// По странице minecraft.wiki: Chiseled Bookshelf. Кода игры и чужих ядер нет.
// Ячейка по точке щелчка, звуки, порядок пакетов, сравнитель и воронка
// сверены чёрным ящиком (сценарий chiseled-bookshelf).
//
// Окна у шкафа нет: «books are instead added into the bookshelf by using on a
// slot with a book … Using on an occupied slot removes the book from that
// slot». Занятые ячейки видны в состоянии (`slot_N_occupied`), сравнитель
// читает последнюю тронутую ячейку: «a signal (1–6) indicating the last slot
// interacted with. When no slot has been interacted with yet, it outputs 0».

use super::{Pos, set};
use crate::blocks;
use crate::containers::Kind;
use crate::inventory::Stack;
use crate::world::World;

/// Что кладётся в шкаф: «books, books and quills, written books, enchanted
/// books, and/or knowledge books».
pub fn is_book(item: i32) -> bool {
    matches!(
        blocks::item_name(item),
        Some("book" | "writable_book" | "written_book" | "enchanted_book" | "knowledge_book")
    )
}

/// Ячейка шкафа по точке щелчка: верхний ряд — 0, 1, 2 слева направо для
/// смотрящего, нижний — 3, 4, 5 (сценарий chiseled-bookshelf).
pub fn slot_at(facing: &str, cursor: (f32, f32, f32)) -> Option<usize> {
    let column = super::shelf::column(facing, cursor)?;
    let row = usize::from(cursor.1 < 0.5);

    Some(row * 3 + column)
}

/// Сила для сравнителя: последняя тронутая ячейка плюс один; не трогали —
/// 0.
pub fn reading(world: &World, (x, y, z): Pos) -> u8 {
    world
        .container(x, y, z)
        .filter(|c| c.kind == Kind::ChiseledBookshelf)
        .map_or(0, |c| u8::try_from(c.page + 1).unwrap_or(0).min(6))
}

/// Состояние шкафа с `slot_N_occupied` по содержимому.
fn state_for(state: i32, slots: &[Option<Stack>]) -> i32 {
    let Some(orientation) = blocks::orientation_of(state) else {
        return state;
    };
    let mut next = state;

    for (slot, stack) in slots.iter().enumerate().take(6) {
        let name = format!("slot_{slot}_occupied");

        next = orientation.with(next, &name, if stack.is_some() { "true" } else { "false" }).unwrap_or(next);
    }

    next
}

/// Содержимое шкафа сменилось (воронкой, щелчком): состояние блока
/// догоняет его.
pub fn contents_changed(world: &mut World, (x, y, z): Pos) {
    let Some(slots) = world.container(x, y, z).filter(|c| c.kind == Kind::ChiseledBookshelf).map(|c| c.slots.clone()) else {
        return;
    };
    let state = world.get_block(x, y, z);
    let next = state_for(state, &slots);

    if next != state {
        set(world, (x, y, z), next);
    }
}

/// Что щелчок по шкафу сделал.
#[derive(Debug, PartialEq)]
pub enum Used {
    /// Книга из руки легла в ячейку.
    Placed,

    /// Книга из ячейки — игроку.
    Taken(Stack),
}

/// Щелчок по шкафу в `pos` по грани `face` в точку `cursor` с `held` в
/// руке. None — не по лицевой стороне или в пустую ячейку не книгой:
/// щелчок идёт дальше, как по обычному блоку (сценарий: камень по верху
/// шкафа ставится на него).
pub fn use_bookshelf(world: &mut World, pos: Pos, face: i32, cursor: (f32, f32, f32), held: Option<Stack>) -> Option<Used> {
    let state = world.get_block(pos.0, pos.1, pos.2);
    let facing = blocks::orientation_of(state)?.value(state, "facing")?;

    if super::shelf::front_face(facing) != Some(face) {
        return None;
    }

    let slot = slot_at(facing, cursor)?;
    let there = world.container(pos.0, pos.1, pos.2)?.slots.get(slot).copied().flatten();

    let used = match (there, held) {
        (Some(book), _) => {
            world.with_container(pos.0, pos.1, pos.2, |shelf| {
                shelf.slots[slot] = None;
                shelf.page = slot as i32;
            });
            let enchanted = blocks::item_name(book.item) == Some("enchanted_book");
            let sound = if enchanted {
                "minecraft:block.chiseled_bookshelf.pickup.enchanted"
            } else {
                "minecraft:block.chiseled_bookshelf.pickup"
            };

            world.note_sound(pos.0, pos.1, pos.2, sound, 1.0, 1.0);
            Used::Taken(book)
        }
        (None, Some(book)) if is_book(book.item) => {
            world.with_container(pos.0, pos.1, pos.2, |shelf| {
                shelf.slots[slot] = Some(book.with_count(1));
                shelf.page = slot as i32;
            });
            let enchanted = blocks::item_name(book.item) == Some("enchanted_book");
            let sound = if enchanted {
                "minecraft:block.chiseled_bookshelf.insert.enchanted"
            } else {
                "minecraft:block.chiseled_bookshelf.insert"
            };

            world.note_sound(pos.0, pos.1, pos.2, sound, 1.0, 1.0);
            Used::Placed
        }
        _ => return None,
    };

    contents_changed(world, pos);

    Some(used)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slots_by_cursor() {
        // Сценарий chiseled-bookshelf: шкаф на восток.
        assert_eq!(slot_at("east", (1.0, 0.75, 0.85)), Some(0));
        assert_eq!(slot_at("east", (1.0, 0.75, 0.5)), Some(1));
        assert_eq!(slot_at("east", (1.0, 0.25, 0.85)), Some(3));
        assert_eq!(slot_at("east", (1.0, 0.25, 0.5)), Some(4));
        assert_eq!(slot_at("east", (1.0, 0.25, 0.1)), Some(5));
    }

    #[test]
    fn books_only() {
        assert!(is_book(blocks::item_named("book").unwrap()));
        assert!(is_book(blocks::item_named("enchanted_book").unwrap()));
        assert!(!is_book(blocks::item_named("stone").unwrap()));
    }

    #[test]
    fn state_shows_occupied_slots() {
        let state = blocks::state_by_name("chiseled_bookshelf").unwrap();
        let book = Stack::new(blocks::item_named("book").unwrap(), 1);
        let next = state_for(state, &[None, Some(book), None, None, None, Some(book)]);
        let o = blocks::orientation_of(next).unwrap();

        assert_eq!(o.value(next, "slot_1_occupied"), Some("true"));
        assert_eq!(o.value(next, "slot_5_occupied"), Some("true"));
        assert_eq!(o.value(next, "slot_0_occupied"), Some("false"));
    }
}
