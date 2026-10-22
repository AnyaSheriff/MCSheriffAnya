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

    let result = crate::crafting::craft_for_catalog(world.catalog(), &grid).and_then(|result| {
        if result.extras().map_post != Some(1) { return Some(result); }
        let mut data = result.extras(); data.map_post = None;
        world.maps.lock().expect("карты").cartography(result.with_extras(data), Stack::new(blocks::item_named("paper")?, 1), true)
    });
    let Some(result) = result else {
        world.note_level_event(x, y, z, EVENT_FAIL, 0);
        return;
    };

    if blocks::item_name(result.item) == Some("crafter") {
        // Вики, Advancement: находиться рядом с крафтером, делающим крафтер.
        // [проверить] границы радиуса в оригинале.
        world.note_progress_near("adventure/crafters_crafting_crafters", (x as f64 + 0.5, y as f64 + 0.5, z as f64 + 0.5), (8.0, 8.0, 8.0));
    }
    let face = front(state).unwrap_or("north");
    let Some(facing) = containers::facing_offset(face) else {
        return;
    };

    world.with_container(x, y, z, |crafter| crafter.crafting = CRAFTING_TICKS);
    set(world, pos, with(state, "crafting", true));

    // Тратится по одной штуке из каждой занятой ячейки; остатки копятся.
    let remainders: Vec<_> = crate::crafting::remainders(&grid).into_iter().flatten().collect();

    world.with_container(x, y, z, |crafter| {
        for slot in crafter.slots.iter_mut().take(GRID) {
            let Some(stack) = *slot else {
                continue;
            };

            *slot = (stack.count > 1).then(|| stack.with_count(stack.count - 1));

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
    #[test]
    fn map_expansion_commits_and_rejects_locked_or_max_scale() {
        for (scale,locked,works) in [(0,false,true),(4,false,false),(0,true,false)] {
            let mut world = World::in_memory(); let pos = (0,70,0);
            world.set_block(0,70,0,blocks::state_by_name("crafter").unwrap());
            let id = world.maps.lock().unwrap().create(crate::dimension::Dimension::Overworld,0,0,scale);
            world.maps.lock().unwrap().maps.get_mut(&id).unwrap().locked = locked;
            let map = Stack::new(blocks::item_named("filled_map").unwrap(),1).with_extras(crate::stackdata::Data { map_id: Some(id), ..Default::default() });
            world.with_container(0,70,0,|c| { c.slots.fill(Some(Stack::new(blocks::item_named("paper").unwrap(),1))); c.slots[4] = Some(map); });
            craft(&mut world,pos);
            let maps = world.maps.lock().unwrap(); assert_eq!(maps.maps.len(),if works {2} else {1});
            assert_eq!(world.container(0,70,0).unwrap().slots[0].is_none(),works);
            if works { assert_eq!(maps.maps[&(id+1)].scale,1); }
        }
    }

    fn slab_world(profile: crate::gameplay_catalog::Profile, slots: &[(usize, i32)]) -> World {
        let mut world = World::in_memory_with_catalog(crate::gameplay_catalog::Catalog::new(profile));
        world.set_block(0, 70, 0, blocks::state_by_name("crafter").unwrap());
        let concrete = blocks::item_named("white_concrete").unwrap();
        world.with_container(0, 70, 0, |container| {
            for &(slot, count) in slots { container.slots[slot] = Some(Stack::new(concrete, count)); }
        });
        world
    }

    #[test]
    fn profile_slab_crafter_consumes_each_slot_once_and_repeats() {
        use crate::gameplay_catalog::{Profile, WHITE_SLAB_ITEM};
        // Все три допустимых ряда; неодинаковые стопки выявляют лишний расход.
        for row in 0..3 {
            let mut world = slab_world(Profile::Preview26_3, &[(row * 3, 2), (row * 3 + 1, 3), (row * 3 + 2, 4)]);
            for (pass, expected) in [(1, [Some(1), Some(2), Some(3)]), (2, [None, Some(1), Some(2)])] {
                craft(&mut world, (0, 70, 0));
                let slots = &world.container(0, 70, 0).unwrap().slots;
                for (slot, stack) in slots.iter().enumerate() {
                    if (row * 3..row * 3 + 3).contains(&slot) {
                        assert_eq!(stack.map(|stack| stack.count), expected[slot - row * 3], "ряд{row}, проход{pass}, ячейка{slot}");
                    } else { assert!(stack.is_none()); }
                }
                let thrown = world.take_ejected();
                assert_eq!(thrown.len(), 1);
                assert_eq!(thrown[0].2, Stack::new(WHITE_SLAB_ITEM, 6));
                assert_eq!(world.container(0, 70, 0).unwrap().crafting, CRAFTING_TICKS);
            }
            // После исчерпания одной клетки рецепт исчезает: остатки не расходуются.
            let before = world.container(0, 70, 0).unwrap().slots.clone();
            craft(&mut world, (0, 70, 0));
            assert_eq!(world.container(0, 70, 0).unwrap().slots, before);
            assert!(world.take_ejected().is_empty());
        }
    }

    #[test]
    fn slab_crafter_refuses_vertical_grid_and_native_profile_without_consumption() {
        use crate::gameplay_catalog::Profile;
        for (profile, slots) in [
            (Profile::Preview26_3, vec![(0, 2), (3, 3), (6, 4)]),
            (Profile::Java26_1_2, vec![(3, 2), (4, 3), (5, 4)]),
        ] {
            let mut world = slab_world(profile, &slots);
            let before = world.container(0, 70, 0).unwrap().clone();
            craft(&mut world, (0, 70, 0));
            assert_eq!(world.container(0, 70, 0).unwrap(), &before);
            assert!(world.take_ejected().is_empty());
        }
    }

    #[test]
    fn slab_crafter_conserves_output_when_receiver_is_empty_partial_or_full() {
        use crate::gameplay_catalog::{Profile, WHITE_SLAB_ITEM};
        for capacity in [0, 2, 6] {
            let mut world = slab_world(Profile::Preview26_3, &[(0, 1), (1, 1), (2, 1)]);
            let state = world.get_block(0, 70, 0);
            let direction = containers::facing_offset(front(state).unwrap()).unwrap();
            let target = (direction.0, 70 + direction.1, direction.2);
            world.set_block(target.0, target.1, target.2, blocks::state_by_name("chest").unwrap());
            let stone = blocks::item_named("stone").unwrap();
            world.with_container(target.0, target.1, target.2, |container| {
                container.slots.fill(Some(Stack::new(stone, 64)));
                container.slots[0] = if capacity == 6 { None } else { Some(Stack::new(WHITE_SLAB_ITEM, 64 - capacity)) };
            });
            let before = world.container(target.0, target.1, target.2).unwrap().slots.iter().flatten().filter(|stack| stack.item == WHITE_SLAB_ITEM).map(|stack| stack.count).sum::<i32>();
            craft(&mut world, (0, 70, 0));
            assert!(world.container(0, 70, 0).unwrap().slots.iter().all(Option::is_none));
            let after = world.container(target.0, target.1, target.2).unwrap().slots.iter().flatten().filter(|stack| stack.item == WHITE_SLAB_ITEM).map(|stack| stack.count).sum::<i32>();
            let thrown = world.take_ejected();
            assert!(thrown.iter().all(|item| item.2.item == WHITE_SLAB_ITEM));
            let spilled = thrown.iter().map(|item| item.2.count).sum::<i32>();
            assert_eq!(after - before, capacity);
            assert_eq!(spilled, 6 - capacity);
            assert_eq!(after - before + spilled, 6);
        }
    }

    #[test]
    fn every_slab_color_autocrafts_all_rows_and_consumes_exactly_once() {
        use crate::gameplay_catalog::{Profile, SlabFamily, slabs};
        for color in slabs() {
            for row in 0..3 {
                let mut world = slab_world(Profile::Preview26_3, &[]);
                world.with_container(0, 70, 0, |container| {
                    for offset in 0..3 { container.slots[row * 3 + offset] = Some(Stack::new(color.input_item, offset as i32 + 1)); }
                });
                craft(&mut world, (0, 70, 0));
                let slots = &world.container(0, 70, 0).unwrap().slots;
                for (index, stack) in slots.iter().enumerate() {
                    let expected = if index == row * 3 + 1 { Some(Stack::new(color.input_item, 1)) }
                        else if index == row * 3 + 2 { Some(Stack::new(color.input_item, 2)) } else { None };
                    assert_eq!(*stack, expected, "{} ряд{row} слот{index}", color.name);
                }
                let thrown = world.take_ejected();
                assert_eq!(thrown.len(), 1);
                assert_eq!(thrown[0].2, Stack::new(color.item, 6), "{} ряд{row}", color.name);
                let before = world.container(0, 70, 0).unwrap().slots.clone();
                craft(&mut world, (0, 70, 0));
                match color.family {
                    SlabFamily::Concrete => {
                        assert_eq!(world.container(0, 70, 0).unwrap().slots, before);
                        assert!(world.take_ejected().is_empty());
                    }
                    SlabFamily::Mineral | SlabFamily::Poplar => unreachable!("slabs() не содержит серу и тополь"),
                    SlabFamily::Wool => {
                        // Оставшиеся две соседние клетки — прежний рецепт ковра,
                        // а не ещё одна плита: его нельзя запрещать новым семейством.
                        let carpet = blocks::item_named(&format!("{}_carpet", color.input_name.strip_suffix("_wool").unwrap())).unwrap();
                        let thrown = world.take_ejected();
                        assert_eq!(thrown.len(), 1);
                        assert_eq!(thrown[0].2, Stack::new(carpet, 3), "{} ряд{row}", color.name);
                        let mut remaining = vec![None; 9];
                        remaining[row * 3 + 2] = Some(Stack::new(color.input_item, 1));
                        assert_eq!(world.container(0, 70, 0).unwrap().slots, remaining);
                        craft(&mut world, (0, 70, 0));
                        assert_eq!(world.container(0, 70, 0).unwrap().slots, remaining);
                        assert!(world.take_ejected().is_empty());
                    }
                }
            }
        }
    }

    #[test]
    fn colored_output_never_merges_with_another_color_in_receiving_chest() {
        use crate::gameplay_catalog::{Profile, slabs};
        let colors: Vec<_> = slabs().collect();
        for (index, color) in colors.iter().enumerate() {
            for room in [false, true] {
                // Та же семья/другой цвет и другая семья/тот же цвет.
                for other in [colors[index / 16 * 16 + (index + 1) % 16], colors[(index + 16) % 32]] {
                let mut world = slab_world(Profile::Preview26_3, &[]);
                world.with_container(0, 70, 0, |container| {
                    container.slots[..3].fill(Some(Stack::new(color.input_item, 1)));
                });
                let direction = containers::facing_offset(front(world.get_block(0, 70, 0)).unwrap()).unwrap();
                let target = (direction.0, 70 + direction.1, direction.2);
                world.set_block(target.0, target.1, target.2, blocks::state_by_name("chest").unwrap());
                world.with_container(target.0, target.1, target.2, |container| {
                    container.slots.fill(Some(Stack::new(blocks::item_named("stone").unwrap(), 64)));
                    container.slots[0] = Some(Stack::new(other.item, 60));
                    if room { container.slots[1] = None; }
                });
                craft(&mut world, (0, 70, 0));
                assert!(world.container(0, 70, 0).unwrap().slots.iter().all(Option::is_none));
                let chest = &world.container(target.0, target.1, target.2).unwrap().slots;
                assert_eq!(chest[0], Some(Stack::new(other.item, 60)), "{} / {}", color.name, other.name);
                let inserted: i32 = chest.iter().flatten().filter(|stack| stack.item == color.item).map(|stack| stack.count).sum();
                assert_eq!(inserted, if room { 6 } else { 0 });
                let thrown = world.take_ejected();
                assert!(thrown.iter().all(|item| item.2.item == color.item));
                let spilled: i32 = thrown.iter().map(|item| item.2.count).sum();
                assert_eq!(inserted + spilled, 6);
                }
            }
        }
    }

    #[test]
    fn both_slab_families_refuse_native_vertical_and_mixed_family_grid_without_consumption() {
        use crate::gameplay_catalog::{Profile, slabs};
        let colors: Vec<_> = slabs().collect();
        for (index, color) in colors.iter().enumerate() {
            for case in 0..4 {
                let profile = if case == 0 { Profile::Java26_1_2 } else { Profile::Preview26_3 };
                let mut world = slab_world(profile, &[]);
                world.with_container(0, 70, 0, |container| {
                    let cells = if case == 1 { [0, 3, 6] } else { [0, 1, 2] };
                    for slot in cells { container.slots[slot] = Some(Stack::new(color.input_item, 2)); }
                    if case == 2 {
                        let other_family = colors[(index + 16) % 32];
                        container.slots[1] = Some(Stack::new(other_family.input_item, 2));
                    } else if case == 3 {
                        let other_color = colors[index / 16 * 16 + (index + 1) % 16];
                        container.slots[1] = Some(Stack::new(other_color.input_item, 2));
                    }
                });
                let before = world.container(0, 70, 0).unwrap().clone();
                craft(&mut world, (0, 70, 0));
                assert_eq!(world.container(0, 70, 0).unwrap(), &before, "{} case{case}", color.name);
                assert!(world.take_ejected().is_empty());
            }
        }
    }

}
