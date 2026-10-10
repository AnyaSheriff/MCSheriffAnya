use super::*;

/// Вики, Commands/give: «fake item entity», остаток — «Owner» и задержка 0.
/// [проверить] Порядок пакетов появления/подбора относительно слотов в 26.1.2.
pub(super) fn give(shared: &Shared, player: &mut PlayerState, stack: Stack, visual: bool) -> Vec<i32> {
    let mut slots = Vec::new();
    let mut count = stack.count;
    while count > 0 {
        let part = stack.with_count(count.min(stack.limit()));
        count -= part.count;
        let (changed, left) = player.inventory.put(part);
        slots.extend(changed);
        if !visual {
            if let Some(left) = left { throw_out(shared, player, left); }
            continue;
        }
        let at = (player.x, player.y + 1.32, player.z);
        let (lx, ly, lz) = looking(player.yaw, player.pitch);
        let velocity = (lx * items::THROW_SPEED, ly * items::THROW_SPEED + 0.1, lz * items::THROW_SPEED);
        let id = shared.next_entity_id();
        let mut items = shared.items_in(player.dimension).lock().expect("предметы");
        if let Some(left) = left {
            items.give_remainder(id, left, at, velocity, player.entity_id);
        } else {
            items.give_visual(id, part.with_count(1), at, velocity, player.entity_id);
        }
        drop(items);
        if left.is_none() {
            let mut world = shared.world_in(player.dimension).lock().expect("мир");
            let pitch = (world.random_unit() - world.random_unit()) * 0.7 + 1.0;
            world.note_sound_player_at((player.x, player.y, player.z), "minecraft:entity.item.pickup", 0.2, pitch * 2.0);
        }
    }
    slots.sort_unstable();
    slots.dedup();
    slots
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn give_adds_once_and_shows_pickup_and_overflow_is_owned() {
        let shared = Shared::new(World::in_memory(), World::in_memory(), Default::default(), Default::default(), Default::default(), crate::ops::Ops::open(std::path::Path::new("target/give-test-ops.json")), crate::skins::Settings::default());
        let mut player = PlayerState::new([19; 16], "GiveTest".into(), None, None, 0x7f, 1, (0.0, 64.0, 0.0));
        player.entity_id = 19;
        let stack = Stack::new(blocks::item_named("stone").unwrap(), 65);
        assert!(!give(&shared, &mut player, stack, true).is_empty());
        let total: i32 = (9..=44).filter_map(|slot| player.inventory.slot(slot)).map(|stack| stack.count).sum();
        assert_eq!(total, 65);
        {
            let mut items = shared.items_in(player.dimension).lock().unwrap();
            assert_eq!(items.items().len(), 2);
            assert!(items.take_near((0.0, 64.0, 0.0), 19, |stack| stack.count).is_empty());
            items.step(&|_, _, _| false, &|_, _, _| false, &|_, _| true);
            assert!(items.items().is_empty());
        }
        for slot in 9..=44 { player.inventory.set(slot, Some(stack.with_count(64))); }
        give(&shared, &mut player, Stack::new(blocks::item_named("diamond").unwrap(), 2), true);
        let mut items = shared.items_in(player.dimension).lock().unwrap();
        assert_eq!(items.items()[0].stack.count, 2);
        assert!(items.take_near((0.0, 64.0, 0.0), 20, |stack| stack.count).is_empty());
        assert_eq!(items.take_near((0.0, 64.0, 0.0), 19, |stack| stack.count).len(), 1);
    }
}
