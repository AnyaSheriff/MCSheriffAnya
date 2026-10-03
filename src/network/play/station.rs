// Окна станков без своего хранилища: стол зачарования, наковальня, точило,
// кузнечный стол.
//
// Предметы лежат в самом окне (`OpenWindow::shown`), пока оно открыто, и
// при закрытии возвращаются игроку. Расчёты — enchanting.rs. Порядок
// пакетов снят с оригинала чёрным ящиком (enchant-offers): после щелчка —
// изменённые ячейки, потом свойства окна; свойство уходит, только если
// значение поменялось.

use super::*;
use crate::enchanting::{self, Station};
use crate::smithing;

/// Станок, чьё это окно.
pub(super) struct StationWindow {
    pub kind: Station,
    pub pos: (i32, i32, i32),
    /// Свойства окна, какими их знает клиент.
    pub props: Vec<i16>,
    /// Имя, присланное для наковальни (Rename Item).
    pub name: Option<String>,
    /// Наковальня: цена в уровнях и сколько единиц материала уйдёт.
    pub cost: i32,
    pub repair_items: i32,
}

/// Станок в этом месте, если там станок.
pub(super) fn station_at(world: &World, (x, y, z): (i32, i32, i32)) -> Option<Station> {
    blocks::block_at_state(world.get_block(x, y, z)).and_then(Station::of)
}

/// Открывает окно станка: Open Screen, содержимое, все свойства.
pub(super) async fn open_station(
    stream: &mut Outgoing<'_>,
    shared: &Shared,
    player: &mut PlayerState,
    at: (i32, i32, i32),
    kind: Station,
) -> io::Result<()> {
    player.window_counter = player.window_counter % WINDOW_IDS + 1;

    let id = player.window_counter;
    let (menu, title) = kind.menu();
    let mut body = encode_varint(OPEN_SCREEN);

    body.extend_from_slice(&encode_varint(id));
    body.extend_from_slice(&encode_varint(menu));
    body.extend_from_slice(&nbt::translated(title));
    write_body(stream, body).await?;

    let station = StationWindow { kind, pos: at, props: Vec::new(), name: None, cost: 0, repair_items: 0 };

    player.window = Some(OpenWindow {
        id,
        halves: Vec::new(),
        cart: None,
        shown: vec![None; kind.slots()],
        lectern: false,
        crafter: false,
        kind: None,
        props: Vec::new(),
        merchant: None,
        station: Some(station),
    });
    send_window_content(stream, player).await?;

    // Свойства — все сразу, как у оригинала при открытии (у точила их нет).
    let (_, props) = compute(shared, player);

    if let Some(station) = player.window.as_mut().and_then(|window| window.station.as_mut()) {
        station.props.clone_from(&props);
    }

    for (property, value) in props.into_iter().enumerate() {
        send_window_property(stream, id, property as i16, value).await?;
    }

    Ok(())
}

/// Итог (у наковальни и точила) и свойства окна по нынешним ячейкам.
fn compute(shared: &Shared, player: &mut PlayerState) -> (Option<Stack>, Vec<i16>) {
    let creative = player.game_mode == GAME_MODE_CREATIVE;
    let seed = player.vit.xp_seed;
    let Some(window) = player.window.as_mut() else {
        return (None, Vec::new());
    };
    let Some(station) = window.station.as_mut() else {
        return (None, Vec::new());
    };

    match station.kind {
        Station::Enchanting => {
            let shelves = {
                let world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");

                enchanting::bookshelves(|(x, y, z)| world.get_block(x, y, z), station.pos)
            };

            (None, enchanting::properties(seed, shelves, window.shown[0]).to_vec())
        }
        Station::Anvil => {
            let outcome = enchanting::anvil(window.shown[0], window.shown[1], station.name.as_deref(), creative);

            station.cost = outcome.cost;
            station.repair_items = outcome.repair_items;

            (outcome.result, vec![outcome.cost as i16])
        }
        Station::Grindstone => (enchanting::grind(window.shown[0], window.shown[1]), Vec::new()),
        Station::Smithing => {
            let result = smithing::result(window.shown[0], window.shown[1], window.shown[2]);

            // Свойство 0 — «рецепта нет» (красный крест).
            (result, vec![i16::from(smithing::recipe_error(&window.shown, result))])
        }
    }
}

/// Ячейки станка поменялись: новый итог и свойства — клиенту то, что
/// изменилось.
fn refresh(shared: &Shared, player: &mut PlayerState, reaction: &mut Reaction) {
    let (result, props) = compute(shared, player);
    let Some(window) = player.window.as_mut() else {
        return;
    };
    let id = window.id;
    let Some(station) = window.station.as_mut() else {
        return;
    };

    let result_slot = station.kind.inputs();

    if station.kind != Station::Enchanting && window.shown[result_slot] != result {
        window.shown[result_slot] = result;
        reaction.window_slots.push((result_slot, result));
    }

    for (property, &value) in props.iter().enumerate() {
        if station.props.get(property) != Some(&value) {
            reaction.window_properties.push((id, property as i16, value));
        }
    }

    station.props = props;
}

/// Ячейки окна целиком: станок, рюкзак, панель.
fn snapshot(shown: &[Option<Stack>], inventory: &Inventory) -> Vec<Option<Stack>> {
    let mut all = shown.to_vec();

    for own in (inventory::FIRST_MAIN..=inventory::LAST_MAIN).chain(inventory::FIRST_HOTBAR..=inventory::LAST_HOTBAR) {
        all.push(inventory.slot(own));
    }

    all
}

/// Щелчок в окне станка. Как и у хранилищ, клиенту досылается только то,
/// что он предсказал не так.
pub(super) fn click(shared: &Shared, player: &mut PlayerState, payload: &[u8], reaction: &mut Reaction) {
    let mut cursor = Cursor::new(payload);

    let (Some(_window), Some(_state_id), Some(slot), Some(button), Some(mode)) =
        (cursor.varint(), cursor.varint(), cursor.u16(), cursor.u8(), cursor.varint())
    else {
        return;
    };

    let mut predicted: Vec<(usize, Option<(i32, i32)>)> = Vec::new();
    let mut predicted_cursor = None;
    let mut readable = true;

    if let Some(count) = cursor.varint() {
        for _ in 0..count.clamp(0, 128) {
            let (Some(place), Some(item)) = (cursor.u16(), read_hashed_slot(&mut cursor)) else {
                readable = false;
                break;
            };

            predicted.push((place as i16 as usize, item));
        }

        if readable {
            predicted_cursor = read_hashed_slot(&mut cursor);
        }
    }

    let Some(window) = player.window.as_ref() else {
        return;
    };
    let Some(station) = window.station.as_ref() else {
        return;
    };
    let kind = station.kind;
    let size = kind.slots();
    let before = snapshot(&window.shown, &player.inventory);
    let slot = slot as i16 as i32;
    let has_result = kind != Station::Enchanting;
    let result_slot = kind.inputs();
    let mut taken = false;

    if has_result && slot == result_slot as i32 {
        taken = take_result(shared, player, mode, reaction);
    } else if mode == 1 && slot >= size as i32 && (slot as usize) < size + window::PLAYER_SLOTS {
        quick_move_in(player, kind, slot as usize);
    } else if slot == inventory::OUTSIDE || (0..(size + window::PLAYER_SLOTS) as i32).contains(&slot) {
        generic_click(shared, player, kind, slot, button as i8, mode);
    }

    let inputs_changed = player.window.as_ref().is_some_and(|window| window.shown[..result_slot] != before[..result_slot]);

    if inputs_changed || taken {
        refresh(shared, player, reaction);
        // Итог уйдёт ниже, по порядку ячеек, как у оригинала (0, 2, 31).
        reaction.window_slots.retain(|(slot, _)| *slot != result_slot || !has_result);
    }

    let Some(window) = player.window.as_ref() else {
        return;
    };
    let after = snapshot(&window.shown, &player.inventory);
    let as_pair = |stack: Option<Stack>| stack.map(|stack| (stack.item, stack.count));

    for (index, stack) in after.iter().enumerate() {
        let expected = predicted
            .iter()
            .rev()
            .find(|(place, _)| *place == index)
            .map_or(as_pair(before[index]), |(_, item)| *item);

        // Довески (имя, зачарования) клиент не предсказывает: изменённая
        // ячейка уходит, даже если предмет и число совпали.
        if !readable || expected != as_pair(*stack) || (before[index] != *stack && stack.is_some()) {
            reaction.window_slots.push((index, *stack));
        }
    }

    if !readable || predicted_cursor != Some(as_pair(player.inventory.cursor())) {
        reaction.cursor = Some(player.inventory.cursor());
    }
}

/// Обычный щелчок по ячейкам станка и инвентаря: как у хранилищ, только
/// итог в общий разбор не входит, а ячейки станка строже (одна вещь на
/// столе, в ячейке лазурита — лазурит, на точиле — изнашиваемое или
/// зачарованное).
fn generic_click(shared: &Shared, player: &mut PlayerState, kind: Station, slot: i32, button: i8, mode: i32) {
    let Some(window) = player.window.as_mut() else {
        return;
    };
    let count = kind.inputs();
    let mut inputs = window.shown[..count].to_vec();
    let inventory_before = player.inventory.clone();
    // У наковальни, точила и кузнечного стола итог стоит между ячейками и
    // инвентарём.
    let slot = if kind != Station::Enchanting && slot > count as i32 { slot - 1 } else { slot };
    let mut view = window::Window { container: &mut inputs, inventory: &mut player.inventory, kind: None };
    let click = if mode == 1 && kind != Station::Enchanting && (0..count as i32).contains(&slot) {
        // Shift по входной ячейке: предмет идёт в инвентарь с первой ячейки
        // вперёд (рюкзак, потом панель), а не с конца, как из хранилищ
        // (чёрный ящик smithing-trim: 4, не 39) [проверить на точиле].
        let moved = view.get(slot as usize).is_some_and(|_| view.move_some(slot as usize, &(count..count + window::PLAYER_SLOTS).collect::<Vec<_>>()));

        inventory::Click { understood: moved, thrown: None }
    } else {
        view.click(slot, button, mode)
    };
    let lapis = blocks::item_named("lapis_lazuli");
    let mut allowed = match kind {
        Station::Enchanting => inputs[1].is_none_or(|stack| Some(stack.item) == lapis),
        Station::Grindstone => inputs.iter().flatten().all(enchanting::grindable),
        Station::Anvil => true,
        Station::Smithing => inputs.iter().enumerate().all(|(place, stack)| stack.is_none_or(|stack| smithing::may_place(place, &stack))),
    };

    // На стол — одна вещь: лишнее остаётся в курсоре.
    if kind == Station::Enchanting
        && let Some(item) = inputs[0].filter(|stack| stack.count > 1)
    {
        let cursor = player.inventory.cursor();

        match cursor {
            None => player.inventory.set_cursor(Some(item.with_count(item.count - 1))),
            Some(held) if held.stacks_with(&item) => player.inventory.set_cursor(Some(held.with_count(held.count + item.count - 1))),
            Some(_) => allowed = false,
        }

        inputs[0] = Some(item.with_count(1));
    }

    if !allowed {
        player.inventory = inventory_before;
        return;
    }

    if let Some(window) = player.window.as_mut() {
        window.shown[..count].copy_from_slice(&inputs);
    }

    if let Some(stack) = click.thrown {
        throw_out(shared, player, stack);
    }
}

/// Shift-щелчок из инвентаря в станок. Не легло в станок — стопка
/// переходит между рюкзаком и панелью, как у оригинала.
fn quick_move_in(player: &mut PlayerState, kind: Station, slot: usize) {
    let Some(window) = player.window.as_mut() else {
        return;
    };
    let size = kind.slots();
    let count = kind.inputs();
    let mut inputs = window.shown[..count].to_vec();
    // В общем разборе у станков итога нет: инвентарь — сразу за входами.
    let own = slot - size + count;
    let mut view = window::Window { container: &mut inputs, inventory: &mut player.inventory, kind: None };
    let Some(stack) = view.get(own) else {
        return;
    };
    let lapis = blocks::item_named("lapis_lazuli");

    let moved = match kind {
        Station::Enchanting if Some(stack.item) == lapis => view.move_some(own, &[1]),
        Station::Enchanting => {
            if view.get(0).is_none() {
                view.set(0, Some(stack.with_count(1)));
                view.set(own, inventory::remainder(stack, stack.count - 1));
                true
            } else {
                false
            }
        }
        Station::Grindstone if !enchanting::grindable(&stack) => false,
        Station::Anvil | Station::Grindstone => view.move_some(own, &[0, 1]),
        // В ту ячейку, куда предмет годится (шаблон, основа, добавка).
        Station::Smithing => (0..3).find(|&place| smithing::may_place(place, &stack)).is_some_and(|place| view.move_some(own, &[place])),
    };

    if !moved {
        // Рюкзак → панель, панель → рюкзак.
        let targets: Vec<usize> = if own < count + 27 { (count + 27..count + 36).collect() } else { (count..count + 27).collect() };

        view.move_some(own, &targets);
    }

    window.shown[..count].copy_from_slice(&inputs);
}

/// Забрать итог наковальни или точила: в курсор (обычный щелчок) или в
/// инвентарь (с Shift). true — забран.
fn take_result(shared: &Shared, player: &mut PlayerState, mode: i32, reaction: &mut Reaction) -> bool {
    let creative = player.game_mode == GAME_MODE_CREATIVE;
    let level = crate::survival::level_of(player.vit.xp).0;
    let Some(window) = player.window.as_mut() else {
        return false;
    };
    let Some(station) = window.station.as_ref() else {
        return false;
    };
    let result_slot = station.kind.inputs();
    let Some(result) = window.shown[result_slot] else {
        return false;
    };

    // Наковальня: нужна цена больше нуля и уровни на неё.
    if station.kind == Station::Anvil && (station.cost <= 0 || (!creative && level < station.cost)) {
        return false;
    }

    match mode {
        0 => match player.inventory.cursor() {
            None => player.inventory.set_cursor(Some(result)),
            Some(held) if held.stacks_with(&result) && held.count + result.count <= held.limit() => {
                player.inventory.set_cursor(Some(held.with_count(held.count + result.count)));
            }
            Some(_) => return false,
        },
        1 => {
            let mut only = [Some(result)];
            let inventory_before = player.inventory.clone();

            window::Window { container: &mut only, inventory: &mut player.inventory, kind: None }.click(0, 0, 1);

            if only[0].is_some() {
                player.inventory = inventory_before;
                return false;
            }
        }
        _ => return false,
    }

    let (kind, pos, cost, repair_items) = (station.kind, station.pos, station.cost, station.repair_items);
    let inputs = [window.shown[0], window.shown[1]];

    window.shown[result_slot] = None;

    match kind {
        Station::Anvil => {
            window.shown[0] = None;
            window.shown[1] = match inputs[1] {
                Some(right) if repair_items > 0 => inventory::remainder(right, right.count - repair_items),
                _ => None,
            };

            if !creative {
                player.vit.command_xp(false, -cost, true);
                reaction.experience = true;
            }

            anvil_used(shared, player, pos, creative);
        }
        Station::Grindstone => {
            window.shown[0] = None;
            window.shown[1] = None;

            let mut rng = crate::mob::Rng::new(u64::from(shared.next_entity_id() as u32) ^ (*shared.ticks.borrow() << 20));
            let points = enchanting::grind_xp(&inputs, |half| rng.below(half as u32) as i32);
            let at = (pos.0 as f64 + 0.5, pos.1 as f64 + 0.5, pos.2 as f64 + 0.5);

            if points > 0 {
                shared
                    .items_in(player.dimension)
                    .lock()
                    .expect("предметы захвачены другим потоком")
                    .drop_experience(points, at, &mut || shared.next_entity_id(), &mut rng);
            }

            // Звук точила — событием мира 1042 (вики, World Event).
            shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком").note_level_event(pos.0, pos.1, pos.2, 1042, 0);
        }
        Station::Smithing => {
            // Уходит по одному из каждой ячейки; звук — событием мира 1044
            // (чёрный ящик smithing-upgrade).
            for place in 0..3 {
                window.shown[place] = window.shown[place].and_then(|stack| inventory::remainder(stack, stack.count - 1));
            }

            shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком").note_level_event(pos.0, pos.1, pos.2, 1044, 0);
        }
        Station::Enchanting => {}
    }

    true
}

/// Наковальня отработала: 12 % — на ступень хуже (разрушенная исчезает,
/// событие 1029), иначе звук работы (событие 1030). В творческом режиме не
/// портится (вики, Anvil).
fn anvil_used(shared: &Shared, player: &mut PlayerState, (x, y, z): (i32, i32, i32), creative: bool) {
    let mut world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");
    let state = world.get_block(x, y, z);
    let name = blocks::block_at_state(state).unwrap_or("anvil");
    let mut rng = crate::mob::Rng::new(u64::from(shared.next_entity_id() as u32) ^ (*shared.ticks.borrow() << 20));

    if creative || rng.unit() >= enchanting::ANVIL_DAMAGE_CHANCE {
        world.note_level_event(x, y, z, 1030, 0);
        return;
    }

    match enchanting::anvil_damaged(name) {
        Some(next) => {
            let facing = blocks::orientation_of(state).and_then(|block| block.value(state, "facing")).unwrap_or("north");

            if let Some(worse) = blocks::state_from_text(&format!("minecraft:{next}[facing={facing}]")) {
                world.set_block(x, y, z, worse);
            }

            world.note_level_event(x, y, z, 1030, 0);
        }
        None => {
            world.set_block(x, y, z, world::AIR);
            world.note_level_event(x, y, z, 1029, 0);
        }
    }

    redstone::settle(&mut world);
}

/// Кнопка в окне стола: зачаровать предложением 0–2. Проверки — как у
/// оригинала: лазурита не меньше номера+1, уровней не меньше стоимости и
/// номера+1 (творческому — без проверок). Уходит номер+1 уровней и столько
/// же лазурита, семя меняется, стол звучит. true — кнопка этого окна.
pub(super) fn button(shared: &Shared, player: &mut PlayerState, window: i32, button: i32, reaction: &mut Reaction) -> bool {
    let creative = player.game_mode == GAME_MODE_CREATIVE;
    let level = crate::survival::level_of(player.vit.xp).0;
    let seed = player.vit.xp_seed;
    let Some(open) = player.window.as_ref().filter(|open| open.id == window) else {
        return false;
    };
    let Some(station) = open.station.as_ref().filter(|station| station.kind == Station::Enchanting) else {
        return false;
    };
    let Ok(slot) = usize::try_from(button) else {
        return true;
    };

    if slot > 2 {
        return true;
    }

    let need = slot as i32 + 1;
    let cost = i32::from(station.props.get(slot).copied().unwrap_or(0));
    let pos = station.pos;
    let (Some(item), lapis) = (open.shown[0], open.shown[1]) else {
        return true;
    };

    if !creative && lapis.is_none_or(|lapis| lapis.count < need) {
        return true;
    }

    if cost <= 0 || (!creative && (level < need || level < cost)) {
        return true;
    }

    let shelves = {
        let world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");

        enchanting::bookshelves(|(x, y, z)| world.get_block(x, y, z), pos)
    };
    let Some(enchanted) = enchanting::enchant(seed, shelves, item, slot) else {
        return true;
    };

    player.vit.command_xp(false, -need, true);
    reaction.experience = true;
    player.vit.xp_seed = enchanting::new_seed(u64::from(player.entity_id as u32));

    {
        let mut world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");
        let mut rng = crate::mob::Rng::new(u64::from(shared.next_entity_id() as u32));
        let pitch = rng.unit() as f32 * 0.1 + 0.9;

        world.note_sound(pos.0, pos.1, pos.2, "minecraft:block.enchantment_table.use", 1.0, pitch);
    }

    if let Some(open) = player.window.as_mut() {
        open.shown[0] = Some(enchanted);
        reaction.window_slots.push((0, Some(enchanted)));

        if !creative && let Some(lapis) = lapis {
            open.shown[1] = inventory::remainder(lapis, lapis.count - need);
            reaction.window_slots.push((1, open.shown[1]));
        }
    }

    refresh(shared, player, reaction);
    true
}

/// Rename Item: новое имя для наковальни (длиннее 50 знаков — не
/// принимается).
pub(super) fn rename(shared: &Shared, player: &mut PlayerState, name: String, reaction: &mut Reaction) {
    let Some(station) = player.window.as_mut().and_then(|window| window.station.as_mut()) else {
        return;
    };

    if station.kind != Station::Anvil || name.chars().count() > enchanting::MAX_NAME || station.name.as_deref() == Some(name.as_str()) {
        return;
    }

    station.name = Some(name);
    refresh(shared, player, reaction);
}

/// Окно станка закрыто: предметы из его ячеек (не итог) — игроку, не
/// влезшее падает рядом. true — инвентарь поменялся.
pub(super) fn give_back(shared: &Shared, player: &mut PlayerState, window: &OpenWindow) -> bool {
    let mut changed = false;

    let inputs = window.station.as_ref().map_or(2, |station| station.kind.inputs());

    for stack in window.shown.iter().take(inputs).flatten() {
        let (_, left) = player.inventory.put(*stack);

        changed = true;

        if let Some(left) = left {
            throw_out(shared, player, left);
        }
    }

    changed
}

/// Стоит ли станок окна на месте (сломали — окно закрывается).
pub(super) fn still_there(shared: &Shared, player: &PlayerState, window: &OpenWindow) -> bool {
    let Some(station) = window.station.as_ref() else {
        return true;
    };
    let world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");

    station_at(&world, station.pos) == Some(station.kind)
}
