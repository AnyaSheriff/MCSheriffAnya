// Полка (shelf, любое дерево): три ячейки на лицевой стороне.
//
// По страницам minecraft.wiki: Shelf. Кода игры и чужих ядер нет. Порядок
// пакетов, звуки, ячейка по точке щелчка, сцепка и обмен панели сверены
// чёрным ящиком (сценарий shelf).
//
// «Interacting with any of the 3 slots swaps the player's main hand item with
// the content of that slot if it is not powered by redstone». Под током полки
// сцепляются по три — «Powered shelves prioritize connecting to other shelves
// on the left», — и щелчок по любой из сцепки меняет её ячейки местами с
// правыми 3, 6 или 9 ячейками панели быстрого доступа. Сравнитель сзади
// читает занятые ячейки: левая — 1, средняя — 2, правая — 4.

use super::{Pos, any_power, set};
use crate::blocks;
use crate::containers::Kind;
use crate::inventory::Stack;
use crate::world::World;

/// Полка ли это — по имени блока.
pub fn is_shelf(name: &str) -> bool {
    name.ends_with("_shelf") && name != "chiseled_bookshelf"
}

/// Грань (номер в пакете Use Item On), на которую смотрит полка или шкаф.
pub fn front_face(facing: &str) -> Option<i32> {
    Some(match facing {
        "north" => 2,
        "south" => 3,
        "west" => 4,
        "east" => 5,
        _ => return None,
    })
}

/// Столбец лицевой стороны (0 — левый для смотрящего на неё, 2 — правый)
/// по точке щелчка внутри блока. Смотрящему на восточную грань слева юг:
/// у полки `facing=east` щелчок с Z у 1 — левая ячейка (сценарии shelf,
/// chiseled-bookshelf).
pub fn column(facing: &str, (x, _, z): (f32, f32, f32)) -> Option<usize> {
    let along = match facing {
        "north" => 1.0 - x,
        "south" => x,
        "west" => z,
        "east" => 1.0 - z,
        _ => return None,
    };

    Some(((along * 3.0).floor() as i32).clamp(0, 2) as usize)
}

/// Сторона, где у смотрящего на полку правая рука: у `facing=east` — север.
fn right_of(facing: &str) -> Option<(i32, i32)> {
    Some(match facing {
        "north" => (-1, 0),
        "south" => (1, 0),
        "west" => (0, 1),
        "east" => (0, -1),
        _ => return None,
    })
}

fn value(state: i32, name: &str) -> Option<&'static str> {
    blocks::orientation_of(state)?.value(state, name)
}

fn with(state: i32, name: &str, to: &str) -> i32 {
    blocks::orientation_of(state).and_then(|o| o.with(state, name, to)).unwrap_or(state)
}

fn shelf_at(world: &World, (x, y, z): Pos) -> Option<i32> {
    let state = world.get_block(x, y, z);

    blocks::block_at_state(state).filter(|name| is_shelf(name)).map(|_| state)
}

/// Сетевой NBT полки, как его шлёт оригинал: `align_items_to_bottom` и
/// ячейки (`count`, `Slot`, `id`); пустой список — тоже есть.
pub fn nbt(slots: &[Option<Stack>]) -> Vec<u8> {
    fn name(out: &mut Vec<u8>, kind: u8, name: &str) {
        out.push(kind);
        out.extend_from_slice(&(name.len() as u16).to_be_bytes());
        out.extend_from_slice(name.as_bytes());
    }

    let filled: Vec<(usize, Stack)> =
        slots.iter().enumerate().filter_map(|(slot, stack)| stack.map(|stack| (slot, stack))).collect();
    let mut out = vec![0x0A];

    name(&mut out, 0x01, "align_items_to_bottom");
    out.push(0);
    name(&mut out, 0x09, "Items");
    out.push(if filled.is_empty() { 0x00 } else { 0x0A });
    out.extend_from_slice(&(filled.len() as i32).to_be_bytes());

    for (slot, stack) in filled {
        let id = format!("minecraft:{}", blocks::item_name(stack.item).unwrap_or("air"));

        name(&mut out, 0x03, "count");
        out.extend_from_slice(&stack.count.to_be_bytes());
        name(&mut out, 0x01, "Slot");
        out.push(slot as u8);
        name(&mut out, 0x08, "id");
        out.extend_from_slice(&(id.len() as u16).to_be_bytes());
        out.extend_from_slice(id.as_bytes());
        out.push(0x00);
    }

    out.push(0x00);
    out
}

/// Разослать данные полки клиентам (после пачки изменений).
pub fn tell(world: &mut World, (x, y, z): Pos) {
    let Some(slots) = world.container(x, y, z).filter(|c| c.kind == Kind::Shelf).map(|c| c.slots.clone()) else {
        return;
    };

    world.note_block_entity(x, y, z, Kind::Shelf.block_entity_type(), nbt(&slots));
}

/// Полка «обновилась» для клиентов, хотя состояние то же: оригинал
/// повторяет её состояние в пачке и шлёт данные.
fn announce(world: &mut World, (x, y, z): Pos) {
    let state = world.get_block(x, y, z);

    world.announce_change(x, y, z, state);
    tell(world, (x, y, z));
}

/// Сила для сравнителя: левая ячейка — 1, средняя — 2, правая — 4.
/// [проверить] оригинал читает только сзади полки («Comparators only work
/// when the back of the shelf is facing it»).
pub fn reading(world: &World, (x, y, z): Pos) -> u8 {
    world
        .container(x, y, z)
        .filter(|c| c.kind == Kind::Shelf)
        .map_or(0, |c| c.slots.iter().take(3).enumerate().filter(|(_, s)| s.is_some()).map(|(i, _)| 1u8 << i).sum())
}

/// Сцепка полки слева направо (для смотрящего): по `side_chain`.
pub fn chain(world: &World, pos: Pos) -> Vec<Pos> {
    let Some(state) = shelf_at(world, pos) else {
        return Vec::new();
    };
    let Some((rx, rz)) = value(state, "facing").and_then(right_of) else {
        return vec![pos];
    };

    // Влево, пока текущая — не левый край.
    let mut left = pos;

    for _ in 0..2 {
        match shelf_at(world, left).and_then(|s| value(s, "side_chain")) {
            Some("center" | "right") => left = (left.0 - rx, left.1, left.2 - rz),
            _ => break,
        }
    }

    let mut members = vec![left];

    if shelf_at(world, left).and_then(|s| value(s, "side_chain")) != Some("left") {
        return members;
    }

    let mut next = left;

    for _ in 0..2 {
        next = (next.0 + rx, next.1, next.2 + rz);

        match shelf_at(world, next).and_then(|s| value(s, "side_chain")) {
            Some("center") => members.push(next),
            Some("right") => {
                members.push(next);
                break;
            }
            _ => break,
        }
    }

    members
}

/// Пересчитать сцепку ряда полок вокруг `pos`: запитанные подряд полки той
/// же стороны делятся на тройки, начиная слева (вики: «Powered shelves
/// connect with shelves on the left first»). [проверить] ряды длиннее трёх.
fn relink(world: &mut World, pos: Pos) {
    let Some(state) = shelf_at(world, pos) else {
        return;
    };
    let facing = value(state, "facing");
    let Some((rx, rz)) = facing.and_then(right_of) else {
        return;
    };
    let same = |world: &World, at: Pos| shelf_at(world, at).filter(|s| value(*s, "facing") == facing);
    let powered = |world: &World, at: Pos| same(world, at).is_some_and(|s| value(s, "powered") == Some("true"));

    // Весь ряд полок той же стороны, слева направо.
    let mut start = pos;

    while same(world, (start.0 - rx, start.1, start.2 - rz)).is_some() {
        start = (start.0 - rx, start.1, start.2 - rz);
    }

    let mut row = Vec::new();
    let mut at = start;

    while same(world, at).is_some() {
        row.push(at);
        at = (at.0 + rx, at.1, at.2 + rz);
    }

    let mut index = 0;

    while index < row.len() {
        if !powered(world, row[index]) {
            restate(world, row[index], "unconnected");
            index += 1;
            continue;
        }

        let mut group = vec![row[index]];

        while group.len() < 3 && index + group.len() < row.len() && powered(world, row[index + group.len()]) {
            group.push(row[index + group.len()]);
        }

        let names: &[&str] = match group.len() {
            1 => &["unconnected"],
            2 => &["left", "right"],
            _ => &["left", "center", "right"],
        };

        for (member, name) in group.iter().zip(names) {
            restate(world, *member, name);
        }

        index += group.len();
    }
}

/// Задать полке `side_chain`; сменилось — её данные тоже уходят.
fn restate(world: &mut World, pos: Pos, side_chain: &str) {
    let Some(state) = shelf_at(world, pos) else {
        return;
    };
    let next = with(state, "side_chain", side_chain);

    if next != state {
        set(world, pos, next);
        tell(world, pos);
    }
}

/// Сосед полки изменился: ток пришёл — `powered` и звук «activate», ушёл —
/// «deactivate»; потом пересчёт сцепки ряда.
pub fn react(world: &mut World, pos: Pos) {
    let Some(state) = shelf_at(world, pos) else {
        return;
    };
    let powered = any_power(world, pos);

    if powered == (value(state, "powered") == Some("true")) {
        return;
    }

    let sound = if powered { "minecraft:block.shelf.activate" } else { "minecraft:block.shelf.deactivate" };

    world.note_sound(pos.0, pos.1, pos.2, sound, 1.0, 1.0);

    let mut next = with(state, "powered", if powered { "true" } else { "false" });

    if !powered {
        next = with(next, "side_chain", "unconnected");
    }

    set(world, pos, next);
    tell(world, pos);
    relink(world, pos);
}

/// Что щелчок по полке сделал с руками игрока.
#[derive(Debug, PartialEq)]
pub enum Used {
    /// Предмет в руке стал таким.
    Hand(Option<Stack>),

    /// Ячейки панели быстрого доступа (0–8) стали такими.
    Hotbar(Vec<(usize, Option<Stack>)>),
}

/// Щелчок по полке в `pos` по грани `face` в точку `cursor`, в руке —
/// `held`, `hotbar` — вся панель. None — щелчок не по лицевой стороне
/// или менять нечего: он идёт дальше, как по обычному блоку.
///
/// В творческом режиме положенный предмет из руки не уходит (сценарий
/// shelf: рука с камнем после щелчка та же).
pub fn use_shelf(
    world: &mut World,
    pos: Pos,
    face: i32,
    cursor: (f32, f32, f32),
    hotbar: &[Option<Stack>],
    selected: usize,
    creative: bool,
) -> Option<Used> {
    let state = shelf_at(world, pos)?;
    let facing = value(state, "facing")?;

    if front_face(facing) != Some(face) {
        return None;
    }

    if value(state, "powered") == Some("true") {
        let members = chain(world, pos);
        let first = 9 - 3 * members.len();
        let mut changed = Vec::new();

        for (index, member) in members.iter().enumerate() {
            let slots = world.container(member.0, member.1, member.2).map(|c| c.slots.clone()).unwrap_or_default();
            let hand: Vec<Option<Stack>> = (0..3).map(|slot| hotbar.get(first + index * 3 + slot).copied().flatten()).collect();

            world.with_container(member.0, member.1, member.2, |shelf| {
                shelf.slots[..3].copy_from_slice(&hand);
            });

            for (slot, stack) in slots.into_iter().enumerate().take(3) {
                changed.push((first + index * 3 + slot, stack));
            }
        }

        world.note_sound(pos.0, pos.1, pos.2, "minecraft:block.shelf.multi_swap", 1.0, 1.0);

        for member in members {
            announce(world, member);
        }

        return Some(Used::Hotbar(changed));
    }

    let slot = column(facing, cursor)?;
    let there = world.container(pos.0, pos.1, pos.2)?.slots.get(slot).copied().flatten();
    let held = hotbar.get(selected).copied().flatten();

    let sound = match (held, there) {
        (None, None) => return None,
        (Some(_), None) => "minecraft:block.shelf.place_item",
        (None, Some(_)) => "minecraft:block.shelf.take_item",
        (Some(_), Some(_)) => "minecraft:block.shelf.single_swap",
    };

    world.with_container(pos.0, pos.1, pos.2, |shelf| shelf.slots[slot] = held);
    world.note_sound(pos.0, pos.1, pos.2, sound, 1.0, 1.0);
    announce(world, pos);

    // [проверить] обмен в творческом: рука, видимо, получает предмет полки.
    let hand = if creative && there.is_none() { held } else { there };

    Some(Used::Hand(hand))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_follows_the_viewer() {
        // Сценарий shelf: полка на восток, Z 0,85 — левая, 0,15 — правая.
        assert_eq!(column("east", (1.0, 0.5, 0.85)), Some(0));
        assert_eq!(column("east", (1.0, 0.5, 0.5)), Some(1));
        assert_eq!(column("east", (1.0, 0.5, 0.15)), Some(2));
        assert_eq!(column("south", (0.1, 0.5, 1.0)), Some(0));
        assert_eq!(column("north", (0.1, 0.5, 0.0)), Some(2));
        assert_eq!(column("west", (0.0, 0.5, 0.9)), Some(2));
    }

    #[test]
    fn nbt_lists_items_even_when_empty() {
        let empty = nbt(&[None, None, None]);

        // Корень, флаг выравнивания, пустой список `Items` из «end».
        assert_eq!(empty.first(), Some(&0x0A));
        assert!(empty.windows(5).any(|w| w == b"Items"));
        assert_eq!(empty.last(), Some(&0x00));

        let stone = blocks::item_named("stone").unwrap();
        let full = nbt(&[Some(Stack::new(stone, 5)), None, None]);

        assert!(full.windows(15).any(|w| w == b"minecraft:stone"));
        assert!(full.len() > empty.len());
    }
}
