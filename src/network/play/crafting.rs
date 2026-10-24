// Личная сетка верстака и инвентаря. Вики, Crafting Table: «3×3 crafting grid».
// Протокол: minecraft-data pc/26.1/protocol.json, crafting menu = 12.
use super::*;
use crate::{crafting::{self as recipes, player as grid_ops}, enchanting::Station};

/// Верстак сломали или игрок отошёл дальше 8 блоков: сервер закрывает окно сам, а вещи сетки
/// и курсора возвращает; ячейки уходят клиенту тактом позже (чёрный ящик, craft-table-break:
/// «окно закрыто» +0, ячейка 36 — +1; курсор не шлётся).
pub(super) async fn watch(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState) -> io::Result<()> {
    for slot in std::mem::take(&mut player.late_slots) { send_container_slot(stream, player, slot).await?; }
    let Some(window) = player.window.as_ref().filter(|_| is_table(player)) else { return Ok(()); };
    if station::still_there(shared, player, window) { return Ok(()); }
    let mut body = encode_varint(CLOSE_CONTAINER_CLIENTBOUND);
    body.extend_from_slice(&encode_varint(window.id));
    write_body(stream, body).await?;
    let before: Vec<_> = (0..=45).map(|s| player.inventory.slot(s)).collect();
    close_own_window(shared, player);
    player.inventory_after_window = false;
    player.late_slots = (0..=45).filter(|&s| player.inventory.slot(s) != before[s as usize]).collect();
    Ok(())
}

pub(super) fn is_table(player: &PlayerState) -> bool {
    player.window.as_ref().and_then(|w| w.station.as_ref()).is_some_and(|s| s.kind == Station::Crafting)
}
fn grid(player: &PlayerState) -> Vec<Option<Stack>> {
    if is_table(player) { player.window.as_ref().unwrap().shown[1..10].to_vec() }
    else { (1..=4).map(|s| player.inventory.slot(s)).collect() }
}
fn store(player: &mut PlayerState, slots: &[Option<Stack>]) {
    if is_table(player) { player.window.as_mut().unwrap().shown[1..10].copy_from_slice(slots); }
    else { for (i,s) in slots.iter().enumerate() { player.inventory.set(i as i32 + 1, *s); } }
}
fn result(shared: &Shared, player: &PlayerState, slots: &[Option<Stack>], commit: bool) -> Option<Stack> {
    let padded = grid_ops::padded(slots);
    let Some(result) = recipes::craft(&padded) else {return if crate::plugins::active() {crate::plugins::prepare_craft(&plugin_view(player),slots,None,false,is_table(player))} else {None};};
    if shared.limited_crafting.load(std::sync::atomic::Ordering::Relaxed)
        && player.game_mode != GAME_MODE_CREATIVE
        && let Some(id) = recipes::recipe_index(&padded)
        && !player.progress.recipes.knows_crafting(id)
    { return None; }
    let result=if result.extras().map_post == Some(1) {
        let mut data = result.extras(); data.map_post = None;
        shared.maps.lock().expect("карты").cartography(result.with_extras(data), Stack::new(blocks::item_named("paper")?, 1), commit)?
    } else {result};
    if crate::plugins::active() {
        let ingredients:Vec<_>=slots.iter().flatten().collect();
        let repair=ingredients.len()==2&&ingredients[0].item==ingredients[1].item&&ingredients.iter().all(|s|s.count==1)&&crate::combat::max_damage(ingredients[0])>0;
        crate::plugins::prepare_craft(&plugin_view(player),slots,Some(result),repair,is_table(player))
    } else {Some(result)}
}
/// Итог после каждой перемены сетки при расходе ингредиентов: по одной на
/// каждую ячейку, из которой убрали предмет, и на каждый остаток, положенный
/// на место (чёрный ящик craft-remainder-bucket: 9 расходов и 3 ведра — 12).
fn consumption_steps(shared: &Shared, player: &PlayerState, grid: &[Option<Stack>]) -> Vec<Option<Stack>> {
    let remainders = recipes::remainders(grid);
    let (mut now, mut steps) = (grid.to_vec(), Vec::new());
    for (i, rest) in remainders.into_iter().enumerate() {
        if now[i].is_none() { continue; }
        now[i] = inventory::take_one(now[i]);
        steps.push(result(shared, player, &now, false));
        if let Some(rest) = rest {
            if now[i].is_none() { now[i] = Some(rest); }
            else if now[i].is_some_and(|s| s.stacks_with(&rest) && s.count + rest.count <= s.limit()) { now[i] = now[i].map(|s| s.with_count(s.count + rest.count)); }
            else { continue; }
            steps.push(result(shared, player, &now, false));
        }
    }
    steps
}
pub(super) fn refresh(shared: &Shared, player: &mut PlayerState) {
    let result = result(shared, player, &grid(player), false);
    if is_table(player) { player.window.as_mut().unwrap().shown[0] = result; }
    else { player.inventory.set(0, result); }
}
// Номера ячеек — как в окне, которое видит клиент: у инвентаря 0..=45, у верстака
// 0 — итог, 1..=9 — сетка, дальше рюкзак и панель.
fn snapshot(player: &PlayerState) -> Vec<Option<Stack>> {
    if is_table(player) {
        let mut all = player.window.as_ref().unwrap().shown.clone();
        all.extend((9..=44).map(|s| player.inventory.slot(s)));
        all
    } else { (0..=45).map(|s| player.inventory.slot(s)).collect() }
}
/// Что клиент думает о ячейках (номер, предмет и число) и курсоре после щелчка.
type Predicted = Option<(Vec<(usize, Option<(i32, i32)>)>, Option<Option<(i32, i32)>>)>;
/// Клиенту уходит только то, что он не предсказал сам (чёрный ящик,
/// craft-inventory-planks: оригинал шлёт ячейки, где итог расходится с хэшем
/// клиента, и курсор). Без предсказаний (Place Recipe) — всё изменившееся.
fn sync(player: &PlayerState, before: &[Option<Stack>], cursor_before: Option<Stack>, predicted: &Predicted, mut trace: Vec<Option<Stack>>, reaction: &mut Reaction) {
    let pair = |s: Option<Stack>| s.map(|s| (s.item, s.count));
    let after = snapshot(player);
    let (list, cursor) = match predicted { Some((l, c)) => (l.as_slice(), *c), None => (&[][..], None) };
    // Итог уходит на каждую перемену сетки, до остальных ячеек (craft-shift-click:
    // 17 посылок на 17 расходов; craft-table-shaped: по одной на каждую доску).
    let grid_range = if is_table(player) { 1..10 } else { 1..5 };
    let changed = grid_range.into_iter().filter(|&i| before[i] != after[i]).count();
    if trace.is_empty() { trace.extend(std::iter::repeat_n(after[0], changed)); }
    reaction.result_sends.extend(trace);
    let grid_changed = !reaction.result_sends.is_empty();
    for (i, stack) in after.iter().enumerate() {
        if grid_changed && i == 0 { continue; }
        let expected = list.iter().rev().find(|(p, _)| *p == i).map_or(pair(before[i]), |(_, e)| *e);
        if expected != pair(*stack) || (before[i] != *stack && stack.is_some_and(|s| s.data.is_some())) {
            if is_table(player) { reaction.window_slots.push((i, *stack)); } else { reaction.slots.push(i as i32); }
        }
    }
    if cursor.unwrap_or(pair(cursor_before)) != pair(player.inventory.cursor())
        || (cursor_before != player.inventory.cursor() && player.inventory.cursor().is_some_and(|s| s.data.is_some())) { reaction.cursor = Some(player.inventory.cursor()); }
}

pub(super) fn click(shared: &Shared, player: &mut PlayerState, payload: &[u8], reaction: &mut Reaction) {
    let mut reader = Cursor::new(payload);
    let (Some(window), Some(_state), Some(slot), Some(button), Some(mode)) = (reader.varint(), reader.varint(), reader.u16(), reader.u8(), reader.varint()) else { return; };
    let table = is_table(player);
    if window != if table { player.window.as_ref().unwrap().id } else { 0 } { return; }
    if player.window.is_some() && !table { return; }
    if table && !station::still_there(shared, player, player.window.as_ref().unwrap()) {
        reaction.close_window = Some(window); close_own_window(shared, player); reaction.inventory = true; return;
    }
    if player.vit.dead || player.game_mode == GAME_MODE_SPECTATOR { return; }
    let mut predicted = None;
    if let Some(count) = reader.varint() {
        let (mut list, mut readable) = (Vec::new(), true);
        for _ in 0..count.clamp(0, 128) {
            let (Some(place), Some(item)) = (reader.u16(), read_hashed_slot(&mut reader)) else { readable = false; break; };
            list.push((place as i16 as usize, item));
        }
        if readable { predicted = Some((list, read_hashed_slot(&mut reader))); }
    }
    let inventory_before=crate::plugins::active().then(||player.inventory.clone());
    let original_grid=grid(player);
    let (before, cursor_before) = (snapshot(player), player.inventory.cursor());
    let slot = slot as i16 as i32; let button = button as i8;
    let mut slots = grid(player); let mut dropped = Vec::new(); let mut trace = Vec::new();
    if mode == 5 {
        drag(player, window, slot, button, &mut slots);
    } else if mode == 3 && player.game_mode == GAME_MODE_CREATIVE {
        let stack = if slot == 0 { result(shared, player, &slots, false) } else { get(player, &slots, slot) };
        if let Some(stack) = stack { player.inventory.set_cursor(Some(stack.with_count(stack.limit()))); }
    } else if slot == 0 {
        let initial = result(shared, player, &slots, false);
        // Ограничение цикла — число предметов сетки, а не доверенный пакет.
        for _ in 0..576 {
            let Some(preview) = result(shared, player, &slots, false).filter(|s| initial.is_some_and(|i| i.item == s.item)) else { break; };
            let mut trial = player.inventory.clone(); let mut trial_grid = slots.clone(); let mut trial_drops = Vec::new();
            if !grid_ops::take(&mut trial_grid, &mut trial, preview, mode, button, &mut trial_drops) { break; }
            let Some(actual) = result(shared, player, &slots, true) else { break; };
            let learned = recipes::recipe_index(&grid_ops::padded(&slots));
            let steps = consumption_steps(shared, player, &slots);
            if !grid_ops::take(&mut slots, &mut player.inventory, actual, mode, button, &mut dropped) { break; }
            trace.extend(steps);
            if let Some(id) = learned { player.progress.recipes.learn_crafting(id); }
            player.progress.crafted(actual.item, actual.count);
            if blocks::item_name(actual.item) == Some("crafting_table") { player.progress.trigger("story/root"); }
            if actual.extras().pot_decorations.is_some_and(|sides| sides.iter().all(|i| blocks::item_name(*i).is_some_and(|n| n.ends_with("_pottery_sherd")))) { player.progress.trigger("adventure/craft_decorated_pot_using_only_sherds"); }
            if mode != 1 { break; }
        }
    } else if table {
        let mapped = if slot > 0 { slot - 1 } else { slot };
        let mut view = window::Window { container: &mut slots, inventory: &mut player.inventory, kind: None, horse: None };
        if mode == 1 && (0..45).contains(&mapped) {
            let targets: Vec<usize> = if mapped < 9 { (9..45).collect() } else if mapped < 36 { (36..45).collect() } else { (9..36).collect() };
            view.move_some(mapped as usize, &targets);
        } else { dropped.extend(view.click(mapped, button, mode).thrown); }
    } else if mode == 6 {
        // Двойной щелчок собирает, только если ячейка под курсором пуста (craft-grid-clicks: иначе ничего).
        if let Some(mut held) = player.inventory.cursor().filter(|_| get(player, &slots, slot).is_none()) {
            for full in [false, true] { for source in (1..=4).chain(9..=45) {
                if let Some(stack) = get(player, &slots, source).filter(|s| s.stacks_with(&held) && (s.count == s.limit()) == full) {
                    let moved = stack.count.min(held.limit() - held.count); held.count += moved;
                    set(player, &mut slots, source, inventory::remainder(stack, stack.count - moved));
                }
            } }
            player.inventory.set_cursor(Some(held));
        }
    } else {
        // Итог убран из общего обработчика: двойной щелчок не может его
        // забрать без расхода ингредиентов.
        player.inventory.set(0, None);
        dropped.extend(player.inventory.click_in_mode(slot, button, mode, player.game_mode == GAME_MODE_CREATIVE).thrown);
        slots = grid(player);
    }
    if mode != 5 { player.crafting_drag = None; }
    // Итог уходит после каждой перемены сетки по порядку ячеек; полностью забранная щелчком или
    // броском ячейка шлёт его дважды (чёрный ящик, craft-grid-clicks: подбор стопки — 0, 0, ячейка;
    // раскладка по трём ячейкам — итог после каждой).
    if trace.is_empty() && slot > 0 {
        let n = if table { 9 } else { 4 };
        let after_grid = grid(player);
        let mut now: Vec<_> = before[1..=n].to_vec();
        let emptied_here = (mode == 0 && cursor_before.is_none() || mode == 4) && (1..=n as i32).contains(&slot) && now[slot as usize - 1].is_some() && after_grid[slot as usize - 1].is_none();
        for i in 0..n {
            if now[i] == after_grid[i] { continue; }
            now[i] = after_grid[i];
            trace.push(result(shared, player, &now, false));
            if emptied_here && slot as usize - 1 == i { trace.push(*trace.last().unwrap()); }
        }
    }
    if crate::plugins::active() {
        let mut staged=Vec::new();let mut denied=false;
        for stack in &dropped {let uuid=throw_menu(shared,player,*stack);staged.push(uuid);if super::cancel_thrown(shared,player,uuid) {denied=true;break;}}
        if denied {shared.items_in(player.dimension).lock().expect("предметы").remove_items(|i|staged.contains(&i.uuid));player.inventory=inventory_before.expect("снимок");store(player,&original_grid);refresh(shared,player);reaction.inventory=true;reaction.window_content=table;reaction.cursor=Some(player.inventory.cursor());return;}
    }
    store(player, &slots); refresh(shared, player); sync(player, &before, cursor_before, &predicted, trace, reaction);
    for stack in dropped { player.progress.dropped(stack.item, stack.count); if !crate::plugins::active() {throw_menu(shared, player, stack);} }
}

pub(super) fn place(shared: &Shared, player: &mut PlayerState, payload: &[u8], reaction: &mut Reaction) {
    let mut reader = Cursor::new(payload);
    let (Some(window), Some(id), Some(all)) = (reader.varint(), reader.varint(), reader.u8()) else { return; };
    if window != player.window.as_ref().map_or(0, |w| w.id) || player.window.is_some() && !is_table(player) { return; }
    if player.vit.dead || player.game_mode == GAME_MODE_SPECTATOR { return; }
    if is_table(player) && !station::still_there(shared, player, player.window.as_ref().unwrap()) { return; }
    let Some(recipe) = player.progress.recipes.crafting_display(id) else { return; };
    let (before, cursor_before) = (snapshot(player), player.inventory.cursor());
    let mut slots = grid(player);
    // Оригинал сперва опустошает сетку ячейка за ячейкой (по посылке итога на
    // каждую, даже пустую), затем раскладывает рецепт по одной посылке на
    // ячейку (recipe-book-place: пусто×4, кнопка, плита, пусто, верстак).
    let mut now = slots.clone();
    let mut trace = Vec::new();
    for i in 0..now.len() { now[i] = None; trace.push(result(shared, player, &now, false)); }
    if !grid_ops::place(recipe, &mut slots, &mut player.inventory, all != 0) {
        reaction.crafting_ghost = Some(crate::recipe_book::ghost(window, recipe));
    }
    for (i, filled) in slots.iter().enumerate() {
        if filled.is_some() { now[i] = *filled; trace.push(result(shared, player, &now, false)); }
    }
    store(player, &slots); refresh(shared, player); sync(player, &before, cursor_before, &None, trace, reaction);
}

/// Бросок из окна: предмет появляется у самых ног, на уровне глаз − 0,3, и уходит в случайную сторону
/// (чёрный ящик, craft-grid-clicks: появление в (0,5; 1,32; 2,5) при игроке в (0,5; 2,5)).
fn throw_menu(shared: &Shared, player: &PlayerState, stack: Stack)->[u8;16] {
    let (spread, angle) = { let mut world = shared.world_in(player.dimension).lock().expect("мир"); (f64::from(world.random_unit()) * 0.5, f64::from(world.random_unit()) * std::f64::consts::TAU) };
    let velocity = (-angle.sin() * spread, 0.2, angle.cos() * spread);
    shared.items_in(player.dimension).lock().expect("предметы").drop_from_player(shared.next_entity_id(), stack, (player.x, player.y + 1.32, player.z), velocity, player.uuid)
}
fn get(player: &PlayerState, slots: &[Option<Stack>], slot: i32) -> Option<Stack> {
    if (1..=slots.len() as i32).contains(&slot) { slots[slot as usize - 1] }
    else if is_table(player) && (10..=45).contains(&slot) { player.inventory.slot(slot - 1) }
    else if !is_table(player) && (9..=45).contains(&slot) { player.inventory.slot(slot) }
    else { None }
}
fn set(player: &mut PlayerState, slots: &mut [Option<Stack>], slot: i32, value: Option<Stack>) {
    if (1..=slots.len() as i32).contains(&slot) { slots[slot as usize - 1] = value; }
    else if is_table(player) && (10..=45).contains(&slot) { player.inventory.set(slot - 1, value); }
    else if !is_table(player) && (9..=45).contains(&slot) { player.inventory.set(slot, value); }
}
// Вики, Inventory: «Drag item stacks around to divide them up».
fn drag(player: &mut PlayerState, window: i32, slot: i32, button: i8, slots: &mut [Option<Stack>]) {
    let kind = button >> 2; let phase = button & 3;
    let Some(held) = player.inventory.cursor() else { player.crafting_drag = None; return; };
    if !(0..=2).contains(&kind) || kind == 2 && player.game_mode != GAME_MODE_CREATIVE { player.crafting_drag = None; return; }
    if phase == 0 { player.crafting_drag = Some((window,kind,Vec::new())); return; }
    let Some((id, started, mut selected)) = player.crafting_drag.take().filter(|(id,k,_)| *id == window && *k == kind) else { return; };
    if phase == 1 {
        let allowed = (1..=slots.len() as i32).contains(&slot) || if is_table(player) { (10..=45).contains(&slot) } else { (9..=45).contains(&slot) };
        if allowed && !selected.contains(&slot) && (kind == 2 || selected.len() < held.count as usize) && get(player,slots,slot).is_none_or(|s| s.stacks_with(&held) && s.count < s.limit()) { selected.push(slot); }
        player.crafting_drag = Some((id,started,selected));
    } else if phase == 2 && !selected.is_empty() {
        let each = if kind == 0 { held.count / selected.len() as i32 } else if kind == 1 { 1 } else { held.limit() };
        let mut left = held.count;
        for target in selected {
            let old = get(player,slots,target); if old.is_some_and(|s| !s.stacks_with(&held)) { continue; }
            let count = old.map_or(0,|s| s.count); let moved = each.min(held.limit() - count).min(if kind == 2 { held.limit() } else { left });
            if moved > 0 { set(player,slots,target,Some(held.with_count(count + moved))); if kind != 2 { left -= moved; } }
        }
        player.inventory.set_cursor(inventory::remainder(held,left));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn shared() -> Shared {
        Shared::new(World::in_memory(),World::in_memory(),Default::default(),Default::default(),Default::default(),crate::ops::Ops::open(std::path::Path::new("tmp/crafting-test-ops.json")),crate::skins::Settings::default())
    }
    fn player() -> PlayerState { let mut p=PlayerState::new([73;16],"Crafting".into(),None,None,0x7f,0,(0.5,64.0,0.5)); p.game_mode=GAME_MODE_SURVIVAL; p }
    fn item(n: &str, count: i32) -> Stack { Stack::new(blocks::item_named(n).unwrap(),count) }
    fn packet(window: i32, slot: i16, button: u8, mode: i32) -> Vec<u8> {
        let mut p=encode_varint(window);p.extend(encode_varint(0));p.extend(slot.to_be_bytes());p.push(button);p.extend(encode_varint(mode));p.extend([0,0]);p
    }
    fn table(p: &mut PlayerState) {
        p.window=Some(OpenWindow{id:1,halves:Vec::new(),cart:None,shown:vec![None;10],lectern:false,crafter:false,kind:None,props:Vec::new(),merchant:None,plugin_chest: None, ender: false,
            station:Some(station::StationWindow{kind:Station::Crafting,pos:(0,64,0),props:Vec::new(),name:None,cost:0,repair_items:0,selected:-1,input_item:None,sound_tick:None})});
    }
    #[test]
    fn inventory_clicks_compute_and_consume_without_trusting_client_output() {
        let shared=shared();let mut p=player();p.inventory.set(36,Some(item("oak_log",2)));let mut r=Reaction::default();
        click(&shared,&mut p,&packet(0,36,0,0),&mut r);click(&shared,&mut p,&packet(0,1,0,0),&mut r);
        assert_eq!(p.inventory.slot(0),Some(item("oak_planks",4)));
        click(&shared,&mut p,&packet(0,0,0,1),&mut r);
        assert_eq!(p.inventory.slot(44),Some(item("oak_planks",8)));assert!(p.inventory.slot(1).is_none());assert!(p.inventory.slot(0).is_none());
        click(&shared,&mut p,&packet(0,0,0,1),&mut r);assert_eq!(p.inventory.slot(44),Some(item("oak_planks",8)));
    }
    #[test]
    fn tables_are_private_and_close_on_break_or_distance() {
        let shared=shared();shared.world_in(Dimension::Overworld).lock().unwrap().set_block(0,64,0,blocks::state_by_name("crafting_table").unwrap());
        let mut a=player();let mut b=player();table(&mut a);table(&mut b);a.window.as_mut().unwrap().shown[1]=Some(item("oak_log",3));refresh(&shared,&mut a);
        assert!(b.window.as_ref().unwrap().shown.iter().all(Option::is_none));assert!(station::still_there(&shared,&a,a.window.as_ref().unwrap()));
        a.x=12.0;assert!(!station::still_there(&shared,&a,a.window.as_ref().unwrap()));
        close_own_window(&shared,&mut a);assert_eq!(a.inventory.slot(36),Some(item("oak_log",3)));assert!(!a.inventory.slots().contains(&Some(item("oak_planks",4))));
        shared.world_in(Dimension::Overworld).lock().unwrap().set_block(0,64,0,0);
        assert!(!station::still_there(&shared,&b,b.window.as_ref().unwrap()));
    }
    #[test]
    fn limited_crafting_and_place_recipe_require_unlock() {
        let shared=shared();let mut p=player();shared.limited_crafting.store(true,std::sync::atomic::Ordering::Relaxed);
        let slots=vec![Some(item("oak_log",1)),None,None,None];assert!(result(&shared,&p,&slots,false).is_none());
        p.progress.recipes.scan(&["oak_log"],1,false);assert_eq!(result(&shared,&p,&slots,false),Some(item("oak_planks",4)));
        let id=recipes::recipes().iter().position(|r| r.result==item("crafting_table",1).item).unwrap() as i32;
        p.progress.recipes.update();p.progress.recipes.update();p.inventory.set(36,Some(item("birch_planks",8)));
        let mut payload=encode_varint(0);payload.extend(encode_varint(id));payload.push(1);let mut r=Reaction::default();place(&shared,&mut p,&payload,&mut r);
        assert_eq!(p.inventory.slot(1),Some(item("birch_planks",2)));assert_eq!(p.inventory.slot(0),Some(item("crafting_table",1)));
    }
    #[test]
    fn map_scaling_preview_does_not_allocate_and_take_allocates_once() {
        let shared=shared();let mut p=player();let id=shared.maps.lock().unwrap().create(Dimension::Overworld,0,0,0);
        let map=item("filled_map",1).with_extras(crate::stackdata::Data{map_id:Some(id),..Default::default()});
        let mut slots=vec![Some(item("paper",1));9];slots[4]=Some(map);
        for _ in 0..3 { assert_eq!(result(&shared,&p,&slots,false).unwrap().extras().map_post,Some(1)); }
        assert_eq!(shared.maps.lock().unwrap().maps.len(),1);
        p.inventory.set_cursor(Some(item("stone",1)));let mut trial=slots.clone();let preview=result(&shared,&p,&slots,false).unwrap();
        assert!(!grid_ops::take(&mut trial,&mut p.inventory,preview,0,0,&mut Vec::new()));assert_eq!(shared.maps.lock().unwrap().maps.len(),1);
        let output=result(&shared,&p,&slots,true).unwrap();assert_eq!(output.extras().map_id,Some(id+1));assert_eq!(shared.maps.lock().unwrap().maps.len(),2);
    }
    #[test]
    fn dragging_distributes_only_real_inputs() {
        let mut p=player();p.inventory.set_cursor(Some(item("spruce_planks",8)));let mut slots=vec![None;4];
        drag(&mut p,0,-999,0,&mut slots);
        for i in 0..=4 {drag(&mut p,0,i,1,&mut slots);}
        drag(&mut p,0,-999,2,&mut slots);
        assert_eq!(slots,vec![Some(item("spruce_planks",2));4]);assert!(p.inventory.cursor().is_none());assert!(p.inventory.slot(0).is_none());
    }
    #[test]
    fn shift_craft_with_partial_room_drops_the_rest_and_with_none_changes_nothing() {
        let shared=shared();let mut p=player();
        for slot in 9..=44 { p.inventory.set(slot,Some(item("cobblestone",64))); }
        p.inventory.set(43,Some(item("oak_planks",62)));p.inventory.set(36,Some(item("oak_log",3)));
        let mut r=Reaction::default();
        click(&shared,&mut p,&packet(0,36,0,0),&mut r);click(&shared,&mut p,&packet(0,1,1,0),&mut r);click(&shared,&mut p,&packet(0,36,0,0),&mut r);
        click(&shared,&mut p,&packet(0,0,0,1),&mut r);
        assert_eq!(p.inventory.slot(43),Some(item("oak_planks",64)),"влезло две из четырёх");
        assert!(p.inventory.slot(1).is_none(),"бревно израсходовано один раз");
        // Ни одной доски поместить некуда: бревно остаётся.
        p.inventory.set(1,Some(item("oak_log",1)));refresh(&shared,&mut p);
        click(&shared,&mut p,&packet(0,0,0,1),&mut r);
        assert_eq!(p.inventory.slot(1),Some(item("oak_log",1)));
    }
    #[test]
    fn emptying_a_grid_slot_by_pickup_resends_the_result_twice() {
        let shared=shared();let mut p=player();p.inventory.set(36,Some(item("oak_log",1)));
        let mut r=Reaction::default();
        click(&shared,&mut p,&packet(0,36,0,0),&mut r);click(&shared,&mut p,&packet(0,1,0,0),&mut r);
        let mut r=Reaction::default();
        click(&shared,&mut p,&packet(0,1,0,0),&mut r);
        assert_eq!(r.result_sends,vec![None,None]);
    }
    #[test]
    fn map_component_corrections_reach_cursor_and_shift_destination() {
        for shift in [false,true] {
            let mut p = player(); let before = snapshot(&p); let mut reaction = Reaction::default();
            let map = item("filled_map",1).with_extras(crate::stackdata::Data { map_id: Some(10), ..Default::default() });
            let predicted = if shift {
                p.inventory.set(44,Some(map)); Some((vec![(44,Some((map.item,1)))],Some(None)))
            } else {
                p.inventory.set_cursor(Some(map)); Some((vec![],Some(Some((map.item,1)))))
            };
            sync(&p,&before,None,&predicted,Vec::new(),&mut reaction);
            if shift { assert!(reaction.slots.contains(&44)); } else { assert_eq!(reaction.cursor,Some(Some(map))); }
        }
    }

}
