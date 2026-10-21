// Лошади Java: щелчки, особое окно и пакеты езды. Minecraft-data 26.1:
// Open Horse Screen 0x29, Player Command 3/4/5, Move Vehicle 0x22.
// [проверить] поведение пакетов требует чёрного ящика.
use super::*;
use crate::mob::{horse, Kind, Mob};

pub(super) fn interact(items: &mut items::Items, index: usize, player: &mut PlayerState, shared: &Shared, reaction: &mut Reaction, hand: i32) -> bool {
    let mob = &mut items.mobs_mut()[index];
    if !horse::equine(mob.kind) { return false; }
    if (mob.x - player.x).powi(2) + (mob.y - player.y).powi(2) + (mob.z - player.z).powi(2) > 4.5 * 4.5 { return true; }
    let held_slot = if hand == 1 { inventory::OFFHAND } else { player.inventory.selected_slot() };
    let held = player.inventory.slot(held_slot);
    let name = held.and_then(|s| blocks::item_name(s.item));
    let mut spent = false;
    if player.sneaking && mob.horse.tamed && !mob.is_baby() {
        reaction.open_cart = Some(mob.entity_id);
    } else if name.is_some_and(|name| horse::feed(mob, name)) {
        spent = true;
        // Кормление — взмах руки и самому игроку (чёрный ящик, horse-tame: Animate).
        // Взмах не повторяется, пока идёт прежний (полпродолжительности — 3 такта;
        // horse-breed: второе кормление на +2 без Animate).
        let now = *shared.ticks.borrow();
        if player.last_swing == 0 || now >= player.last_swing + 3 {
            player.swing_pending = true;
            player.last_swing = now.max(1);
        }
    } else if mob.horse.tamed && !mob.is_baby() {
        let old = mob.horse.slots;
        if name == Some("chest") && horse::pack(mob.kind) && !mob.horse.chest {
            mob.horse.chest = true;
            horse::sound(mob, if mob.kind == Kind::Donkey { "minecraft:entity.donkey.chest" } else { "minecraft:entity.mule.chest" }, 1.0);
            spent = true;
        } else if let Some(stack) = held
            && let Some(slot) = (0..2).find(|&slot| mob.horse.slots[slot].is_none() && horse::may_place(mob.kind, slot, stack.item))
        {
            mob.horse.slots[slot] = Some(stack.with_count(1));
            horse::equipment_changed(mob, old);
            spent = true;
        } else {
            mount(mob, player, shared);
        }
    } else if !mob.is_baby() && !horse::undead(mob.kind) {
        if held.is_none() { mount(mob, player, shared); } else { horse::angry(mob); }
    }
    if spent && player.game_mode != GAME_MODE_CREATIVE && let Some(stack) = held {
        player.inventory.set(held_slot, inventory::remainder(stack, stack.count - 1));
        reaction.slots.push(held_slot);
    }
    items.flush_mob_events(&mut || shared.next_entity_id(), &mut crate::mob::Rng::new(u64::from(shared.next_entity_id() as u32)));
    true
}

/// Вторая рука и приседание из Use Entity (minecraft-data protocol.json).
/// Основную руку пропускаем через общий путь бирок и поводков.
pub(super) fn packet(payload: &[u8], player: &mut PlayerState, shared: &Shared) -> Option<Reaction> {
    let mut cursor = Cursor::new(payload);
    let (target, hand) = (cursor.varint()?, cursor.varint()?);
    if !(0..=1).contains(&hand) { return None; }
    let equine = shared.items_in(player.dimension).lock().expect("предметы").mobs().iter()
        .any(|mob| mob.entity_id == target && horse::equine(mob.kind));
    if !equine { return None; }
    let mut reaction = Reaction::default();
    if player.game_mode == GAME_MODE_SPECTATOR || player.vit.dead { return Some(reaction); }
    let old_sneaking = player.sneaking;
    player.sneaking = payload.last().is_some_and(|v| *v != 0);
    if hand == 0 { reaction = use_entity(target, player, shared); } else {
        let _world = shared.world_in(player.dimension).lock().expect("мир");
        let mut items = shared.items_in(player.dimension).lock().expect("предметы");
        if let Some(index) = items.mobs().iter().position(|mob| mob.entity_id == target && !mob.is_dead()) {
            interact(&mut items, index, player, shared, &mut reaction, hand);
        }
    }
    player.sneaking = old_sneaking;
    Some(reaction)
}

fn mount(mob: &mut Mob, player: &PlayerState, shared: &Shared) {
    if player.vit.dead || player.sneaking || player.ride.vehicle.is_some() { return; }
    if shared.riding.lock().expect("учёт седоков").mount(player.entity_id, mob.entity_id, 1) {
        mob.horse.rider = Some((player.uuid, player.entity_id));
        mob.horse.buck = false;
        mob.horse.last_move_tick = mob.ticks;
        mob.horse.move_budget = 1.0;
        mob.horse.gallop = 0;
        mob.move_dist = 0.0;
        mob.next_step = 1.0;
        mob.brain.stop();
    }
}

fn accessible(mob: &Mob, player: &PlayerState) -> bool {
    horse::equine(mob.kind) && mob.horse.tamed && !mob.is_baby() && !mob.is_dead()
        && (mob.x - player.x).powi(2) + (mob.y - player.y).powi(2) + (mob.z - player.z).powi(2) <= 64.0
}

pub(super) fn slots(shared: &Shared, player: &PlayerState, id: i32) -> Option<Vec<Option<Stack>>> {
    shared.items_in(player.dimension).lock().expect("предметы").mobs().iter()
        .find(|mob| mob.entity_id == id && accessible(mob, player)).map(|mob| mob.horse.inventory())
}

pub(super) async fn open(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState, entity_id: i32) -> io::Result<()> {
    let Some(slots) = slots(shared, player, entity_id) else { return Ok(()); };
    player.window_counter = player.window_counter % WINDOW_IDS + 1;
    let id = player.window_counter;
    let mut packet = encode_varint(0x29);
    packet.extend_from_slice(&encode_varint(id));
    // [проверить] современное поле — число столбцов сундука, 0 либо 5.
    packet.extend_from_slice(&encode_varint(if slots.len() > 2 { 5 } else { 0 }));
    push_i32(&mut packet, entity_id);
    write_body(stream, packet).await?;
    player.window = Some(OpenWindow { id, halves: Vec::new(), cart: Some(entity_id), shown: slots, lectern: false, crafter: false, kind: None, props: Vec::new(), merchant: None, station: None, ender: false });
    send_window_content(stream, player, shared.catalog()).await
}

pub(super) fn command(payload: &[u8], player: &mut PlayerState, shared: &Shared, reaction: &mut Reaction) {
    let mut cursor = Cursor::new(payload);
    let (Some(who), Some(action), Some(boost)) = (cursor.varint(), cursor.varint(), cursor.varint()) else { return; };
    if who != player.entity_id { return; }
    let Some(id) = shared.riding.lock().expect("учёт седоков").vehicle_of(player.entity_id) else { return; };
    let mut items = shared.items_in(player.dimension).lock().expect("предметы");
    let Some(mob) = items.mobs_mut().iter_mut().find(|m| m.entity_id == id && horse::equine(m.kind) && !m.is_dead()) else { return; };
    if action == 5 && mob.horse.tamed { reaction.open_cart = Some(id); }
    if mob.horse.rider.map(|(_, id)| id) != Some(player.entity_id) || !mob.horse.tamed || !mob.horse.saddled() { return; }
    if action == 3 && (1..=100).contains(&boost) && mob.on_ground {
        mob.horse.jump_charge = horse::charge(boost);
        mob.horse.rearing = 20;
        let name = match mob.kind { Kind::Donkey => "minecraft:entity.donkey.jump", Kind::Mule => "minecraft:entity.mule.jump", _ => "minecraft:entity.horse.jump" };
        horse::sound(mob, name, 0.4);
    }
    items.flush_mob_events(&mut || shared.next_entity_id(), &mut crate::mob::Rng::new(id as u64));
}

pub(super) fn input(shared: &Shared, player: &PlayerState, flags: u8) {
    let Some(id) = player.ride.vehicle else { return; };
    let mut items = shared.items_in(player.dimension).lock().expect("предметы");
    if let Some(mob) = items.mobs_mut().iter_mut().find(|m| m.entity_id == id && horse::equine(m.kind))
        && mob.horse.rider.map(|(_, id)| id) == Some(player.entity_id) { mob.horse.input = flags; }
}

struct View<'a>(&'a World);
impl crate::mob::View for View<'_> {
    fn catalog(&self) -> crate::gameplay_catalog::Catalog { self.0.catalog() }
    fn block(&self, x: i32, y: i32, z: i32) -> i32 { self.0.get_block(x, y, z) }
    fn loaded(&self, x: i32, z: i32) -> bool { self.0.has_chunk(x, z) }
}

/// Клиент ведёт осёдланную лошадь. Сервер сверяет водителя, скорость,
/// загруженность и столкновения, при отказе возвращает реальное место.
pub(super) fn vehicle_move(payload: &[u8], player: &mut PlayerState, shared: &Shared, reaction: &mut Reaction) -> bool {
    let Some(id) = player.ride.vehicle else { return false; };
    let world = shared.world_in(player.dimension).lock().expect("мир");
    let mut items = shared.items_in(player.dimension).lock().expect("предметы");
    let Some(mob) = items.mobs_mut().iter_mut().find(|m| m.entity_id == id && horse::equine(m.kind)) else { return false; };
    let (Some(to), Some((yaw, pitch))) = (read_position(payload), read_rotation(payload, 24)) else { return true; };
    let driver = shared.riding.lock().expect("учёт седоков").driver(id) == Some(player.entity_id);
    let accepted = driver && !player.vit.dead && horse::drive(mob, &View(&world), to, (yaw, pitch), payload.get(32).is_some_and(|v| *v != 0));
    if accepted {
        let at = horse::seat(mob);
        player.set_position(at.0, at.1, at.2);
        player.vit.fall = 0.0;
    } else {
        let mut packet = encode_varint(0x39);
        push_f64(&mut packet, mob.x); push_f64(&mut packet, mob.y); push_f64(&mut packet, mob.z);
        push_f32(&mut packet, mob.yaw); push_f32(&mut packet, mob.pitch);
        reaction.packets.push(packet);
    }
    items.flush_mob_events(&mut || shared.next_entity_id(), &mut crate::mob::Rng::new(id as u64));
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shared() -> Shared {
        Shared::new(World::in_memory(), World::in_memory(), Default::default(), Default::default(), Default::default(),
            crate::ops::Ops::open(std::path::Path::new("target/horse-test-ops.json")), crate::skins::Settings::default())
    }

    fn player() -> PlayerState {
        let mut p = PlayerState::new([13;16], "HorseTest".to_string(), None, None, 0x7f, 11, (0.5, 0.0, 0.5));
        p.game_mode = GAME_MODE_SURVIVAL;
        p
    }

    #[test]
    fn saddle_mount_inventory_and_dismount_use_existing_riding() {
        let shared = shared(); let mut p = player();
        let mut horse = Mob::new(100, Kind::Horse, (0.5, 0.0, 2.5));
        horse.horse.tamed = true; horse.on_ground = true;
        shared.items_in(p.dimension).lock().unwrap().add_mob(horse);
        p.inventory.set(inventory::FIRST_HOTBAR, Some(Stack::new(blocks::item_named("saddle").unwrap(), 1)));
        let equipped = use_entity(100, &mut p, &shared);
        assert_eq!(equipped.slots, vec![inventory::FIRST_HOTBAR]);
        assert!(p.inventory.held().is_none());
        assert!(shared.items_in(p.dimension).lock().unwrap().mobs()[0].horse.saddled());
        use_entity(100, &mut p, &shared);
        assert_eq!(shared.riding.lock().unwrap().vehicle_of(p.entity_id), Some(100));
        p.ride.vehicle = Some(100);
        let mut payload = encode_varint(p.entity_id);
        payload.extend_from_slice(&encode_varint(5)); payload.extend_from_slice(&encode_varint(0));
        let mut reaction = Reaction::default();
        command(&payload, &mut p, &shared, &mut reaction);
        assert_eq!(reaction.open_cart, Some(100));
        vehicle::dismount(&shared, &p);
        assert_eq!(shared.riding.lock().unwrap().vehicle_of(p.entity_id), None);
        assert!(shared.items_in(p.dimension).lock().unwrap().mobs()[0].horse.rider.is_none());
    }

    #[test]
    fn donkey_chest_is_consumed_and_window_updates_real_storage() {
        let shared = shared(); let mut p = player();
        let mut donkey = Mob::new(100, Kind::Donkey, (0.5, 0.0, 2.5)); donkey.horse.tamed = true;
        shared.items_in(p.dimension).lock().unwrap().add_mob(donkey);
        p.inventory.set(inventory::FIRST_HOTBAR, Some(Stack::new(blocks::item_named("chest").unwrap(), 1)));
        use_entity(100, &mut p, &shared);
        assert!(p.inventory.held().is_none());
        p.sneaking = true;
        assert_eq!(use_entity(100, &mut p, &shared).open_cart, Some(100));
        p.window = Some(OpenWindow { id: 1, halves: Vec::new(), cart: Some(100), shown: slots(&shared, &p, 100).unwrap(), lectern: false, crafter: false, kind: None, props: Vec::new(), merchant: None, station: None, ender: false });
        let diamonds = Stack::new(blocks::item_named("diamond").unwrap(), 10);
        p.inventory.set_cursor(Some(diamonds));
        let mut click = encode_varint(1); click.extend_from_slice(&encode_varint(0));
        push_i16(&mut click, 2); click.push(0); click.extend_from_slice(&encode_varint(0));
        click_window(&shared, &mut p, &click, &mut Reaction::default());
        assert_eq!(shared.items_in(p.dimension).lock().unwrap().mobs()[0].horse.slots[2], Some(diamonds));
        assert!(p.inventory.cursor().is_none());
    }

    #[test]
    fn jump_command_rejects_other_entity_and_unsaddled_mount() {
        let shared = shared(); let mut p = player();
        let mut horse = Mob::new(100, Kind::Horse, (0.5, 0.0, 2.5));
        horse.horse.tamed = true; horse.on_ground = true;
        shared.items_in(p.dimension).lock().unwrap().add_mob(horse);
        use_entity(100, &mut p, &shared);
        let command_bytes = |id| { let mut v = encode_varint(id); v.extend_from_slice(&encode_varint(3)); v.extend_from_slice(&encode_varint(90)); v };
        command(&command_bytes(p.entity_id), &mut p, &shared, &mut Reaction::default());
        assert_eq!(shared.items_in(p.dimension).lock().unwrap().mobs()[0].horse.jump_charge, 0.0);
        shared.items_in(p.dimension).lock().unwrap().mobs_mut()[0].horse.slots[0] = Some(Stack::new(blocks::item_named("saddle").unwrap(), 1));
        command(&command_bytes(p.entity_id + 1), &mut p, &shared, &mut Reaction::default());
        assert_eq!(shared.items_in(p.dimension).lock().unwrap().mobs()[0].horse.jump_charge, 0.0);
        command(&command_bytes(p.entity_id), &mut p, &shared, &mut Reaction::default());
        assert_eq!(shared.items_in(p.dimension).lock().unwrap().mobs()[0].horse.jump_charge, 1.0);
    }

    #[test]
    fn offhand_equipment_and_packet_sneaking_work_in_adventure() {
        let shared = shared(); let mut p = player(); p.game_mode = GAME_MODE_ADVENTURE;
        let mut horse = Mob::new(100, Kind::Horse, (0.5, 0.0, 2.5)); horse.horse.tamed = true;
        shared.items_in(p.dimension).lock().unwrap().add_mob(horse);
        p.inventory.set(inventory::OFFHAND, Some(Stack::new(blocks::item_named("saddle").unwrap(), 1)));
        let mut bytes = encode_varint(100); bytes.extend_from_slice(&encode_varint(1)); bytes.extend_from_slice(&[0, 0]);
        assert_eq!(packet(&bytes, &mut p, &shared).unwrap().slots, vec![inventory::OFFHAND]);
        assert!(p.inventory.slot(inventory::OFFHAND).is_none());
        assert!(shared.items_in(p.dimension).lock().unwrap().mobs()[0].horse.saddled());
        *bytes.last_mut().unwrap() = 1;
        assert_eq!(packet(&bytes, &mut p, &shared).unwrap().open_cart, Some(100));
        assert!(!p.sneaking);
    }
}
