// Кафедра (lectern): книга в блок-сущности, открытая страница для
// сравнителя и импульс при перелистывании.
//
// По разбору tools/research/components-rest.md, §10, redstone-components.md,
// §6.1 и container-protocol.md (кнопки окна). Страницы minecraft.wiki:
// Lectern, Redstone components. Кода игры и чужих ядер нет. Порядок пакетов
// сверен чёрным ящиком (сценарий lectern-page).
//
// «Lecterns holding a book emit a full-strength redstone pulse that is 2 game
// ticks long when a page is turned»; пока идёт импульс, кафедра питает
// соседей и сильно — блок под собой («It powers the conductive block beneath
// it»).

use super::{PRIORITY_NORMAL, Pos, set, settle};
use crate::blocks;
use crate::containers::Kind;
use crate::inventory::Stack;
use crate::world::World;

/// Сколько тактов держится импульс.
pub const PULSE: u64 = 2;

/// Кнопки окна кафедры (Click Container Button): назад, вперёд, взять книгу;
/// 100 и больше — «открыть страницу номер − 100».
const BUTTON_PREVIOUS: i32 = 1;
const BUTTON_NEXT: i32 = 2;
pub const BUTTON_TAKE: i32 = 3;
const BUTTON_JUMP: i32 = 100;

/// Книги, которые кладутся на кафедру: книга с пером и подписанная
/// (вики, Lectern: обычная и зачарованная — нет).
pub fn is_book(item: i32) -> bool {
    matches!(blocks::item_name(item), Some("writable_book" | "written_book"))
}

/// Состояние блока со свойством `name`, равным `value`.
fn with(state: i32, name: &str, value: bool) -> i32 {
    blocks::orientation_of(state)
        .and_then(|o| o.with(state, name, if value { "true" } else { "false" }))
        .unwrap_or(state)
}

/// Сила для сравнителя: `floor(1 + 14 × (P − 1) / (M − 1))`, где P —
/// страница с единицы, M — всего страниц; книга из одной страницы — 15,
/// без книги — 0 (вики, Lectern).
pub fn lectern_reading(world: &World, (x, y, z): Pos) -> u8 {
    let Some(lectern) = world.container(x, y, z).filter(|c| c.kind == Kind::Lectern) else {
        return 0;
    };

    if lectern.slots[0].is_none() {
        return 0;
    }

    reading(lectern.page, lectern.pages)
}

/// Та же формула, без мира.
fn reading(page: i32, pages: i32) -> u8 {
    // Книга без страниц: подписанная книга без содержимого, положенная
    // щелчком, у оригинала даёт 14 (чёрный ящик, lectern-page). [проверить]
    // почему не 15 и не 1.
    if pages == 0 {
        return 14;
    }

    if pages <= 1 {
        return 15;
    }

    let share = page.clamp(0, pages - 1) as f32 / (pages - 1) as f32;

    (share * 14.0).floor() as u8 + 1
}

/// Открытая страница и число страниц кафедры в этом месте.
pub fn page(world: &World, (x, y, z): Pos) -> Option<(i32, i32)> {
    world.container(x, y, z).filter(|c| c.kind == Kind::Lectern).map(|c| (c.page, c.pages))
}

/// Щелчок игрока по кафедре с `held` в руке. Пустая кафедра и книга в руке —
/// книга ложится (Some(true): её надо потратить), `has_book=true`, звук
/// `item.book.put`. Кафедра с книгой — Some(false): надо открыть окно. None —
/// щелчку тут делать нечего, он идёт дальше. `pages` — сколько страниц в
/// книге без компонента; при наличии компонента берётся его список страниц.
pub fn place_book(world: &mut World, pos: Pos, held: Option<Stack>, pages: i32) -> Option<bool> {
    let (x, y, z) = pos;

    if Kind::of(world.get_block(x, y, z)) != Some(Kind::Lectern) {
        return None;
    }

    if world.container(x, y, z).is_some_and(|c| c.slots[0].is_some()) {
        return Some(false);
    }

    let stack = held.filter(|stack| is_book(stack.item))?;

    world.with_container(x, y, z, |lectern| {
        lectern.slots[0] = Some(stack.with_count(1));
        lectern.page = 0;
        lectern.pages = stack.extras().book.as_ref().map_or(pages,|b|b.pages.len() as i32);
    });
    set(world, pos, with(world.get_block(x, y, z), "has_book", true));
    world.note_sound(x, y, z, "minecraft:item.book.put", 1.0, 1.0);
    crate::containers::after_change(world);
    settle(world);
    Some(true)
}

/// Кнопка в окне кафедры: листает страницы. Страница сменилась —
/// сравнитель пересчитывается, кафедра даёт импульс. Возвращает, сменилась
/// ли страница. Кнопку «взять книгу» разбирает окно (ему нужен инвентарь
/// игрока) — через `take_book`.
pub fn turn_page(world: &mut World, pos: Pos, button: i32) -> bool {
    let (x, y, z) = pos;
    let Some((page, pages)) = page(world, pos) else {
        return false;
    };

    let wanted = match button {
        BUTTON_PREVIOUS => page - 1,
        BUTTON_NEXT => page + 1,
        _ if button >= BUTTON_JUMP => button - BUTTON_JUMP,
        _ => return false,
    };
    let wanted = wanted.clamp(0, (pages - 1).max(0));

    if wanted == page {
        return false;
    }

    // Звук перелистывания — событием мира 1043, раньше свойства окна
    // (чёрный ящик, lectern-page).
    world.with_container(x, y, z, |lectern| lectern.page = wanted);
    world.note_level_event(x, y, z, 1043, 0);
    pulse(world, pos);
    crate::redstone::comparator_update(world, pos);
    settle(world);
    true
}

/// Импульс: `powered=true` и такт через 2, который его снимет. Уже идущий
/// импульс продлевается.
fn pulse(world: &mut World, pos: Pos) {
    let (x, y, z) = pos;
    let state = world.get_block(x, y, z);

    set(world, pos, with(state, "powered", true));
    world.schedule_once(x, y, z, PULSE, PRIORITY_NORMAL);
}

/// Такт кафедры: импульс кончился.
pub fn pulse_over(world: &mut World, pos: Pos) {
    let (x, y, z) = pos;
    let state = world.get_block(x, y, z);

    set(world, pos, with(state, "powered", false));
}

/// Книгу забирают с кафедры (кнопка окна): она уходит в руки игроку,
/// кафедра пустеет — `has_book=false`, `powered=false`. Возвращает книгу.
pub fn take_book(world: &mut World, pos: Pos) -> Option<Stack> {
    let (x, y, z) = pos;
    let book = world.container(x, y, z).filter(|c| c.kind == Kind::Lectern)?.slots[0]?;

    world.with_container(x, y, z, |lectern| {
        lectern.slots[0] = None;
        lectern.page = 0;
        lectern.pages = 0;
    });

    let state = with(with(world.get_block(x, y, z), "has_book", false), "powered", false);

    set(world, pos, state);
    crate::containers::after_change(world);
    settle(world);
    Some(book)
}

/// Сколько страниц у книги из данных блока в команде: `pages:[…]` внутри
/// `Book` — считаются элементы верхнего уровня списка. Нет списка — 0.
pub fn pages_from_text(text: &str) -> i32 {
    let Some(start) = text.find("pages:[") else {
        return 0;
    };
    let mut depth = 0;
    let mut count = 0;
    let mut quoted = None;
    let mut any = false;
    let mut escaped = false;

    for c in text[start + "pages:[".len()..].chars() {
        if let Some(quote) = quoted {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == quote {
                quoted = None;
            }

            continue;
        }

        match c {
            '"' | '\'' => {
                quoted = Some(c);
                any = true;
            }
            '[' | '{' => {
                depth += 1;
                any = true;
            }
            ']' | '}' if depth > 0 => depth -= 1,
            ']' => break,
            ',' if depth == 0 => count += 1,
            c if !c.is_whitespace() => any = true,
            _ => {}
        }
    }

    if any { count + 1 } else { 0 }
}

/// Открытая страница из данных блока в команде: `Page:N`.
pub fn page_from_text(text: &str) -> i32 {
    let Some(start) = text.find("Page:") else {
        return 0;
    };

    text[start + "Page:".len()..]
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '-')
        .collect::<String>()
        .parse()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Формула сравнителя — таблица вики: книга из 15 страниц даёт номер
    /// страницы, из 2 — 1 и 15, из одной — всегда 15.
    #[test]
    fn comparator_follows_the_page() {
        for page in 0..15 {
            assert_eq!(reading(page, 15), page as u8 + 1);
        }

        assert_eq!(reading(0, 2), 1);
        assert_eq!(reading(1, 2), 15);
        assert_eq!(reading(0, 1), 15);
        assert_eq!(reading(0, 0), 14);
        assert_eq!(reading(0, 5), 1);
        assert_eq!(reading(1, 5), 4);
        assert_eq!(reading(2, 5), 8);
        assert_eq!(reading(3, 5), 11);
        assert_eq!(reading(4, 5), 15);
    }

    /// Страницы считаются по данным блока из команды.
    #[test]
    fn pages_come_from_command_text() {
        let text = r#"{Book:{id:"minecraft:writable_book",count:1,components:{"minecraft:writable_book_content":{pages:["a","b,c",{raw:"d"}]}}},Page:2}"#;

        assert_eq!(pages_from_text(text), 3);
        assert_eq!(page_from_text(text), 2);
        assert_eq!(pages_from_text("{pages:[]}"), 0);
        assert_eq!(pages_from_text("{}"), 0);
    }
}
