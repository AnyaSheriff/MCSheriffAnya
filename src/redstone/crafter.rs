// Крафтер (crafter): сетка 3×3 в блок-сущности, выключенные ячейки,
// крафт по фронту сигнала и выброс результата.
//
// По страницам minecraft.wiki: Crafter, Tutorial:Crafter, Dropper
// («Container interactions»), Java Edition protocol (окно `crafter_3x3`,
// свойства окна 0–8 — выключенные ячейки, World Event 1049/1050/2010,
// пакет Change Container Slot State). Разбор — tools/research/containers.md,
// §9. Кода игры и чужих ядер нет. Порядок и такты сверены чёрным ящиком
// (сценарии crafter-pulse, crafter-hopper, crafter-slots).
//
// «When a crafter receives a redstone signal, it waits for 4 game ticks
// before ejecting one crafted item using the ingredients from the nine
// inventory slots»; квазисвязности нет («crafters aren't affected by
// quasi-connectivity»).

use super::{PRIORITY_NORMAL, Pos, set, settle};
use crate::blocks;
use crate::containers::{self, Container, Kind};
use crate::inventory::Stack;
use crate::world::World;

/// Задержка крафта после фронта сигнала.
pub const DELAY: u64 = 4;

/// Сколько тактов после крафта держится `crafting=true`
/// (`crafting_ticks_remaining`: «Set to 6 when the crafter crafts
/// something»).
pub const CRAFTING_TICKS: i32 = 6;

/// Ячеек в сетке.
const GRID: usize = 9;

/// World Event: крафт удался, не удался, дым крафтера.
const EVENT_CRAFT: i32 = 1049;
const EVENT_FAIL: i32 = 1050;
const EVENT_SMOKE: i32 = 2010;

/// Состояние блока со свойством `name`, равным `value`.
fn with(state: i32, name: &str, value: bool) -> i32 {
    blocks::orientation_of(state)
        .and_then(|o| o.with(state, name, if value { "true" } else { "false" }))
        .unwrap_or(state)
}

fn property(state: i32, name: &str) -> Option<&'static str> {
    blocks::orientation_of(state)?.value(state, name)
}

/// Куда смотрит лицо крафтера: первая часть `orientation` (`east_up` —
/// восток).
fn front(state: i32) -> Option<&'static str> {
    property(state, "orientation")?.split('_').next()
}

/// Сила для сравнителя: сколько ячеек заняты или выключены; размер стопки
/// не важен (вики, Crafter: «The signal strength is equal to the number of
/// crafting slots that are either disabled or occupied by an item»).
pub fn reading(world: &World, (x, y, z): Pos) -> u8 {
    world.container(x, y, z).filter(|c| c.kind == Kind::Crafter).map_or(0, filled_or_disabled)
}

fn filled_or_disabled(crafter: &Container) -> u8 {
    (0..GRID).filter(|&slot| crafter.slots[slot].is_some() || crafter.is_disabled(slot)).count() as u8
}

/// Куда крафтер кладёт предмет снаружи (воронка, выбрасыватель, другой
/// крафтер): первая пустая включённая ячейка по рядам слева направо; если
/// пустых нет — самая маленькая стопка того же предмета, при равенстве —
/// первая по тому же порядку (вики, Crafter: «the item is added to the
/// lowest count item stack of the same type»).
pub fn slot_for(crafter: &Container, item: i32) -> Option<usize> {
    let enabled = |slot: &usize| !crafter.is_disabled(*slot);

    if let Some(empty) = (0..GRID).filter(enabled).find(|&slot| crafter.slots[slot].is_none()) {
        return Some(empty);
    }

    (0..GRID)
        .filter(enabled)
        .filter_map(|slot| crafter.slots[slot].map(|stack| (slot, stack)))
        .filter(|(_, stack)| stack.item == item && stack.count < blocks::stack_size(item))
        .min_by_key(|&(slot, stack)| (stack.count, slot))
        .map(|(slot, _)| slot)
}

/// Выключенные ячейки из данных блока в команде:
/// `{disabled_slots:[I;2,5,6]}`. Нет поля — ни одной.
pub fn disabled_from_text(text: &str) -> u16 {
    let Some(at) = text.find("disabled_slots:[") else {
        return 0;
    };
    let list = &text[at + "disabled_slots:[".len()..];
    let list = &list[..list.find(']').unwrap_or(list.len())];
    let list = list.strip_prefix("I;").unwrap_or(list);

    list.split(',')
        .filter_map(|number| number.trim().parse::<usize>().ok())
        .filter(|&slot| slot < GRID)
        .fold(0, |mask, slot| mask | 1 << slot)
}

/// Соседи сменились: по фронту сигнала — `triggered=true` и крафт через 4
/// такта; сигнал пропал — `triggered=false`. Смотрит только на своё место.
pub fn neighbour_changed(world: &mut World, pos: Pos, powered: bool) {
    let state = world.get_block(pos.0, pos.1, pos.2);
    let triggered = property(state, "triggered") == Some("true");

    if powered && !triggered {
        set(world, pos, with(state, "triggered", true));
        world.schedule_once(pos.0, pos.1, pos.2, DELAY, PRIORITY_NORMAL);
    } else if !powered && triggered {
        set(world, pos, with(state, "triggered", false));
    }
}

/// Прошли 4 такта от фронта: крафт. Рецепта нет — звук неудачи. Есть —
/// результат уходит в хранилище перед лицом или вылетает, по одной штуке
/// каждого ингредиента тратится, остатки (вёдра, бутылки) вылетают следом
/// (вики, Crafter: «those are ejected after the crafted item»).
pub fn craft(world: &mut World, pos: Pos) {
    let (x, y, z) = pos;
    let state = world.get_block(x, y, z);
    let Some(grid) = world.container(x, y, z).filter(|c| c.kind == Kind::Crafter).map(|c| c.slots.clone()) else {
        return;
    };

    let Some(result) = crate::crafting::craft(&grid) else {
        world.note_level_event(x, y, z, EVENT_FAIL, 0);
        return;
    };

    let face = front(state).unwrap_or("north");
    let Some(facing) = containers::facing_offset(face) else {
        return;
    };

    world.with_container(x, y, z, |crafter| crafter.crafting = CRAFTING_TICKS);
    set(world, pos, with(state, "crafting", true));

    // Тратится по одной штуке из каждой занятой ячейки; остатки копятся.
    let mut remainders = Vec::new();

    world.with_container(x, y, z, |crafter| {
        for slot in crafter.slots.iter_mut().take(GRID) {
            let Some(stack) = *slot else {
                continue;
            };

            *slot = (stack.count > 1).then(|| stack.with_count(stack.count - 1));

            if let Some(left) = crate::crafting::remainder(stack.item) {
                remainders.push(Stack::new(left, 1));
            }
        }
    });

    // Каждая вылетевшая стопка — со своим звуком крафта и дымом; ушедшая в
    // хранилище — молча (чёрный ящик: crafter-pulse — торт и три ведра,
    // четыре раза 1049 и 2010; crafter-hopper — в сундук без событий).
    for stack in std::iter::once(result).chain(remainders) {
        if out(world, pos, facing, stack) {
            world.note_level_event(x, y, z, EVENT_CRAFT, 0);
            world.note_level_event(x, y, z, EVENT_SMOKE, containers::smoke_direction(face));
        }
    }

    containers::after_change(world);
    settle(world);
}

/// Стопка уходит в хранилище перед лицом (как у выбрасывателя), а что не
/// влезло — вылетает вперёд. Возвращает, вылетело ли что-нибудь.
fn out(world: &mut World, pos: Pos, facing: (i32, i32, i32), stack: Stack) -> bool {
    let front = (pos.0 + facing.0, pos.1 + facing.1, pos.2 + facing.2);
    let target = containers::halves(world, front);
    let left = if !target.is_empty() && containers::can_open(world, &target) {
        containers::insert_stack(world, &target, stack, facing)
    } else {
        Some(stack)
    };

    let Some(left) = left else {
        return false;
    };

    containers::throw_forward(world, pos, facing, left, SPREAD);
    true
}

/// Разброс скорости вылета: у крафтера вбок до ±0,1 (чёрный ящик,
/// crafter-pulse: у стопок, вылетевших на восток, по Z −0,076 и −0,034, по
/// Y 0,23–0,26; на север по X до −0,0996) — больше, чем у выбрасывателя.
/// [проверить] по большему числу вылетов.
const SPREAD: f64 = 0.0172275 * 6.0;

/// Такт блок-сущности: отсчёт `crafting_ticks_remaining`; дошёл до нуля —
/// `crafting=false`.
pub fn tick(world: &mut World, pos: Pos) {
    let (x, y, z) = pos;
    let Some(left) = world.container(x, y, z).map(|c| c.crafting).filter(|&left| left > 0) else {
        return;
    };

    world.with_container(x, y, z, |crafter| crafter.crafting = left - 1);

    if left - 1 == 0 {
        let state = world.get_block(x, y, z);

        if property(state, "crafting") == Some("true") {
            set(world, pos, with(state, "crafting", false));
            settle(world);
        }
    }
}

/// Игрок щёлкнул по ячейке в окне крафтера: включить или выключить её.
/// Переключается только пустая ячейка (вики, Crafter: «A slot can be
/// enabled or disabled, which can be toggled by clicking on it when
/// empty»). Возвращает, сменилось ли что-нибудь.
pub fn toggle_slot(world: &mut World, pos: Pos, slot: usize, enabled: bool) -> bool {
    let (x, y, z) = pos;

    if slot >= GRID {
        return false;
    }

    let changed = world
        .with_container(x, y, z, |crafter| {
            if crafter.kind != Kind::Crafter || crafter.slots[slot].is_some() || crafter.is_disabled(slot) != enabled {
                return false;
            }

            crafter.disabled ^= 1 << slot;
            true
        })
        .unwrap_or(false);

    if changed {
        super::comparator_update(world, pos);
        settle(world);
    }

    changed
}

/// Свойства окна крафтера: 0–8 — выключена ли ячейка (1 — да), 9 — запитан
/// ли крафтер. [проверить] свойство 9 — по чёрному ящику (crafter-slots).
pub fn window_properties(world: &World, (x, y, z): Pos) -> Vec<(i16, i16)> {
    let Some(crafter) = world.container(x, y, z).filter(|c| c.kind == Kind::Crafter) else {
        return Vec::new();
    };
    let triggered = property(world.get_block(x, y, z), "triggered") == Some("true");

    (0..GRID)
        .map(|slot| (slot as i16, i16::from(crafter.is_disabled(slot))))
        .chain(std::iter::once((GRID as i16, i16::from(triggered))))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn crafter(slots: &[(usize, &str, i32)], disabled: &[usize]) -> Container {
        let mut crafter = Container::new(Kind::Crafter);

        for &(slot, name, count) in slots {
            crafter.slots[slot] = Some(Stack::new(blocks::item_named(name).unwrap(), count));
        }

        crafter.disabled = disabled.iter().fold(0, |mask, slot| mask | 1 << slot);
        crafter
    }

    #[test]
    fn comparator_counts_filled_and_disabled_slots() {
        assert_eq!(filled_or_disabled(&crafter(&[], &[])), 0);
        assert_eq!(filled_or_disabled(&crafter(&[(0, "stick", 1), (4, "stick", 64)], &[])), 2);
        assert_eq!(filled_or_disabled(&crafter(&[(0, "stick", 1)], &[2, 5, 6, 7, 8])), 6);
        assert_eq!(filled_or_disabled(&crafter(&[], &[0, 1, 2, 3, 4, 5, 6, 7, 8])), 9);
    }

    #[test]
    fn hopper_fills_empty_slots_then_the_smallest_stack() {
        let planks = blocks::item_named("oak_planks").unwrap();

        // Пустая включённая — первая по порядку, выключенные пропускаются.
        assert_eq!(slot_for(&crafter(&[], &[0, 1]), planks), Some(2));
        assert_eq!(slot_for(&crafter(&[(0, "oak_planks", 5)], &[]), planks), Some(1));

        // Все включённые заняты: самая маленькая стопка того же предмета.
        let full = crafter(&[(0, "oak_planks", 3), (1, "oak_planks", 2), (3, "oak_planks", 2), (4, "stone", 1)], &[2, 5, 6, 7, 8]);
        assert_eq!(slot_for(&full, planks), Some(1));

        // Некуда: всё выключено или чужие предметы.
        assert_eq!(slot_for(&crafter(&[], &[0, 1, 2, 3, 4, 5, 6, 7, 8]), planks), None);
        assert_eq!(slot_for(&crafter(&[(0, "stone", 1)], &[1, 2, 3, 4, 5, 6, 7, 8]), planks), None);
    }

    #[test]
    fn disabled_slots_come_from_command_text() {
        assert_eq!(disabled_from_text("{disabled_slots:[I;2,5,6,7,8]}"), 0b1_1110_0100);
        assert_eq!(disabled_from_text("{Items:[],disabled_slots:[I; 0, 9]}"), 1);
        assert_eq!(disabled_from_text("{}"), 0);
    }

    #[test]
    fn crafter_survives_the_disk() {
        let mut stored = crafter(&[(4, "oak_log", 2)], &[0, 8]);
        stored.crafting = 3;

        let mut bytes = Vec::new();
        stored.write((5, 70, 9), &mut bytes);

        let (place, read) = Container::read(&mut bytes.as_slice(), 0, 0).expect("запись крафтера");

        assert_eq!(place, (5, 70, 9));
        assert_eq!(read, stored);
        assert!(read.is_disabled(0) && read.is_disabled(8) && !read.is_disabled(4));
    }
}
