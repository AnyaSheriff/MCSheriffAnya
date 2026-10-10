// Костры и маяки: действия Java. [проверить] чёрным ящиком.
use super::*;

pub(super) fn campfire_use(player: &mut PlayerState, shared: &Shared, pos: (i32, i32, i32), face: i32, sequence: i32) -> Option<Reaction> {
    let stack = player.inventory.held()?;
    let name = blocks::item_name(stack.item)?;
    let mut world = shared.world_in(player.dimension).lock().expect("мир");
    let state = world.get_block(pos.0, pos.1, pos.2);
    if !crate::campfire::is_campfire(state) { return None; }
    let lighting = matches!(name, "flint_and_steel" | "fire_charge");
    let shovel = name.ends_with("_shovel");
    let water = name == "water_bucket";
    let cooked = if !lighting && !shovel && !water { crate::campfire::insert(&mut world, pos, stack) } else { false };
    let changed = if lighting || shovel { crate::campfire::set_lit(&mut world, pos, lighting) }
        else if water && world.is_nether() {
            let pitch = 2.6 + (world.random_unit() - world.random_unit()) * 0.8;
            world.note_sound(pos.0, pos.1, pos.2, "minecraft:block.fire.extinguish", 0.5, pitch);
            true
        } else if water {
            crate::campfire::set_lit(&mut world, pos, false);
            let s = world.get_block(pos.0, pos.1, pos.2);
            let new = blocks::orientation_of(s)?.with(s, "waterlogged", "true")?;
            world.set_block(pos.0, pos.1, pos.2, new)
        } else { cooked };
    if !changed { return None; }
    if lighting { world.note_sound(pos.0, pos.1, pos.2, if name == "fire_charge" { "minecraft:item.firecharge.use" } else { "minecraft:item.flintandsteel.use" }, 1.0, 1.0); }
    // Как у любого удавшегося щелчка оригинала (чёрный ящик, campfire-cook, как
    // кровать/якорь): два взмаха и повтор щёлкнутого блока и блока за гранью.
    world.note_animation(player.entity_id, 0);
    world.note_animation(player.entity_id, 0);
    let beyond = offset_towards(pos.0, pos.1, pos.2, face);
    world.tell_directly(pos.0, pos.1, pos.2);
    world.tell_directly(beyond.0, beyond.1, beyond.2);
    redstone::settle(&mut world);
    drop(world);
    if cooked { player.progress.custom("interact_with_campfire", 1); }
    let slots = if water { swap_held(player, blocks::item_named("bucket")?, shared) }
        else if cooked || name == "fire_charge" { spend_placed(player) }
        else {
            if player.game_mode != GAME_MODE_CREATIVE { pvp::wear_slot(shared, player, player.inventory.selected_slot(), stack, 1, false); }
            Vec::new()
        };
    Some(Reaction { slots, confirm: vec![pos], ..Reaction::accepted(sequence) })
}

pub(super) fn beacon_select(payload: &[u8], player: &mut PlayerState, shared: &Shared) {
    if player.game_mode == GAME_MODE_SPECTATOR { return; }
    let Some(window) = player.window.as_ref().filter(|w| w.kind == Some(containers::Kind::Beacon)) else { return; };
    let Some(&p) = window.halves.first() else { return; };
    if (player.x - p.0 as f64).powi(2) + (player.y - p.1 as f64).powi(2) + (player.z - p.2 as f64).powi(2) > 64.0 { return; }
    let mut cursor = Cursor::new(payload);
    let mut read = || { if cursor.u8()? == 0 { Some(-1) } else { cursor.varint() } };
    let (Some(primary), Some(secondary)) = (read(), read()) else { return; };
    let mut world = shared.world_in(player.dimension).lock().expect("мир");
    let levels = crate::beacon::level(&world, p);
    if !crate::beacon::valid_selection(levels, primary, secondary) { return; }
    if !player.window.as_ref().is_some_and(|window| window.shown[0].is_some_and(|s| crate::beacon::payment(s.item))) { return; }
    let done = world.with_container(p.0, p.1, p.2, |c| {
        if c.kind != containers::Kind::Beacon { return false; }
        c.beacon[1] = primary;
        c.beacon[2] = secondary;
        true
    }).unwrap_or(false);
    if done {
        player.state_id = player.state_id.wrapping_add(1);
        if let Some(window) = player.window.as_mut() {
            window.shown[0] = inventory::take_one(window.shown[0]);
            let mut body = encode_varint(SET_CONTAINER_SLOT);
            body.extend(encode_varint(window.id));
            body.extend(encode_varint(player.state_id));
            push_i16(&mut body, 0);
            push_slot(&mut body, window.shown[0]);
            world.note_private(player.entity_id, body);
        }
        let data = crate::beacon::nbt(world.container(p.0, p.1, p.2).unwrap());
        world.note_block_entity(p.0, p.1, p.2, containers::Kind::Beacon.block_entity_type(), data);
        world.note_sound(p.0, p.1, p.2, "minecraft:block.beacon.power_select", 1.0, 1.0);
    }
}
