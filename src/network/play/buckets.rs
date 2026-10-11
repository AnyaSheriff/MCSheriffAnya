// Вики, Bucket: «use item»; жидкость выбирается лучом самого сервера.
// Use Item On оставлен котлам и ведру рыхлого снега. [проверить] Тонкие формы.
use super::*;

pub(super) fn liquid(item: i32) -> bool {
    matches!(blocks::item_name(item), Some("bucket" | "water_bucket" | "lava_bucket"))
        || blocks::item_name(item).is_some_and(|name| crate::mob::water::bucket_kind(name).is_some())
}

fn target(world: &World, eye: (f64,f64,f64), direction: (f64,f64,f64), reach: f64, empty: bool) -> Option<((i32,i32,i32),i32)> {
    let cell = |t: f64| ((eye.0+direction.0*t).floor() as i32,(eye.1+direction.1*t).floor() as i32,(eye.2+direction.2*t).floor() as i32);
    let mut previous = cell(0.0);
    let mut t = 0.0;
    while t <= reach {
        let at = cell(t);
        t += 0.01;
        let state = world.get_block(at.0,at.1,at.2);
        let fluid = fluids::fluid_at(state);
        let source = fluid.is_some_and(|(_, level)| level == fluids::SOURCE);
        let solid = state != world::AIR && fluid.is_none();
        // Полное ведро пропускает жидкость; пустое ищет только источники.
        if solid || (empty && source) {
            if empty && !source && !fluids::is_powder_snow(state)
                && crate::block_behaviour::bucket_water(state, false).is_none() { return None; }
            let delta = (previous.0-at.0,previous.1-at.1,previous.2-at.2);
            let face = match delta { (_, -1, _) => 0, (_, 1, _) => 1, (_, _, -1) => 2, (_, _, 1) => 3, (-1, _, _) => 4, (1, _, _) => 5, _ => 1 };
            return Some((at,face));
        }
        previous = at;
    }
    None
}

pub(super) fn use_item(shared: &Shared, player: &mut PlayerState, hand: i32, sequence: i32, yaw: f32, pitch: f32) -> Option<Reaction> {
    let main = player.inventory.selected_slot();
    let slot = if hand == 1 { inventory::OFFHAND } else { main };
    let item = player.inventory.slot(slot)?.item;
    if !liquid(item) { return None; }
    if matches!(player.game_mode, GAME_MODE_SPECTATOR | GAME_MODE_ADVENTURE) { return Some(Reaction::accepted(sequence)); }
    let hit = {
        let world = shared.world_in(player.dimension).lock().expect("мир");
        target(&world, (player.x,player.y+if player.sneaking {1.27} else {1.62},player.z), looking(yaw,pitch), 5.0, blocks::item_name(item)==Some("bucket"))
    };
    let Some((at, face)) = hit else { return Some(Reaction::accepted(sequence)); };
    // Повторно используем обмен ведра, включая полные стопки и выпадение.
    let swap = |player: &mut PlayerState| {
        let a = player.inventory.slot(main); let b = player.inventory.slot(inventory::OFFHAND);
        player.inventory.set(main,b); player.inventory.set(inventory::OFFHAND,a);
    };
    if hand == 1 { swap(player); }
    let mut answer = super::use_bucket(item, at, face, sequence, player, shared);
    if hand == 1 {
        swap(player);
        if let Some(answer) = &mut answer {
            for slot in &mut answer.slots {
                if *slot == main { *slot = inventory::OFFHAND; } else if *slot == inventory::OFFHAND { *slot = main; }
            }
        }
    }
    answer
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn full_bucket_raycast_hits_waterloggable_block_and_empty_bucket_stops_at_wall() {
        let mut world = World::in_memory();
        world.set_block_silently(0,1,2,blocks::state_from_text("oak_slab[type=bottom,waterlogged=false]").unwrap());
        assert_eq!(target(&world,(0.5,1.2,0.5),(0.0,0.0,1.0),5.0,false),Some(((0,1,2),2)));
        assert!(target(&world,(0.5,1.2,0.5),(0.0,0.0,1.0),5.0,true).is_none());
        world.set_block_silently(0,1,1,blocks::state_by_name("water").unwrap());
        assert_eq!(target(&world,(0.5,1.2,0.5),(0.0,0.0,1.0),5.0,true),Some(((0,1,1),2)));
    }
}
