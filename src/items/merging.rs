use super::*;

fn mergeable(item: &Dropped) -> bool {
    item.block.is_none() && item.fuse.is_none() && item.health > 0.0
        && item.age < LIFETIME && item.pickup_delay != u64::MAX
        && item.stack.count > 0 && item.stack.count < item.stack.limit()
}

impl Items {
    /// Вики, Item (entity), «Merging»: только целые стопки, большая остаётся,
    /// меньшая (при равенстве — начавшая проверку) исчезает. [проверить] Фаза
    /// проверки 2/40 тактов и расширение рамки X/Z на 0,5 без расширения Y.
    pub(super) fn merge_items(&mut self, due: &[i32]) {
        for id in due {
            let Some(mut index) = self.items.iter().position(|item| item.entity_id == *id) else { continue; };
            if !mergeable(&self.items[index]) { continue; }
            let candidates: Vec<i32> = self.items.iter().filter(|other| other.entity_id != *id).map(|item| item.entity_id).collect();
            for other_id in candidates {
                let Some(other) = self.items.iter().position(|item| item.entity_id == other_id) else { continue; };
                let a = &self.items[index];
                let b = &self.items[other];
                if !mergeable(a) || !mergeable(b) || !a.stack.stacks_with(&b.stack) || a.owner != b.owner
                    || a.stack.count + b.stack.count > a.stack.limit()
                    || (a.x - b.x).abs() >= SIZE + 0.5 || (a.z - b.z).abs() >= SIZE + 0.5 || (a.y - b.y).abs() >= SIZE
                { continue; }
                let (keep, remove) = if a.stack.count > b.stack.count { (index, other) } else { (other, index) };
                let age = a.age.min(b.age);
                let delay = a.pickup_delay.saturating_sub(a.age).max(b.pickup_delay.saturating_sub(b.age));
                let count = a.stack.count + b.stack.count;
                let kept = &mut self.items[keep];
                kept.stack.count = count;
                kept.age = age;
                kept.pickup_delay = age.saturating_add(delay);
                self.changes.push(Change::Stack { entity_id: kept.entity_id, stack: kept.stack, no_gravity: kept.no_gravity });
                let gone = self.items.remove(remove);
                self.changes.push(Change::Gone { entity_id: gone.entity_id });
                if remove == index { break; }
                if remove < index { index -= 1; }
            }
        }
    }

    /// Вики, Experience orb (zh.minecraft.wiki/w/经验球): «20游戏刻»,
    /// одинаковая стоимость и разность номеров кратна 40. Сохраняются число
    /// подборов и младший возраст, остаётся меньший номер. [проверить] Фаза.
    pub(super) fn merge_orbs(&mut self) {
        let due: Vec<i32> = self.orbs.iter().filter(|orb| orb.ticks.is_multiple_of(20)).map(|orb| orb.entity_id).collect();
        for id in due {
            let Some(mut index) = self.orbs.iter().position(|orb| orb.entity_id == id) else { continue; };
            let mut other = 0;
            while other < self.orbs.len() {
                let a = &self.orbs[index];
                let b = &self.orbs[other];
                if index == other || a.expired() || b.expired() || a.value != b.value
                    || (i64::from(a.entity_id) - i64::from(b.entity_id)).rem_euclid(40) != 0
                    || (a.x - b.x).abs() >= 1.0 || (a.y - b.y).abs() >= 1.0 || (a.z - b.z).abs() >= 1.0
                { other += 1; continue; }
                let (keep, remove) = if a.entity_id < b.entity_id { (index, other) } else { (other, index) };
                let count = a.count.saturating_add(b.count);
                let age = a.age.min(b.age);
                self.orbs[keep].count = count;
                self.orbs[keep].age = age;
                let gone = self.orbs.remove(remove);
                self.changes.push(Change::Gone { entity_id: gone.entity_id });
                if remove == index { break; }
                if remove < index { index -= 1; }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn stack(count: i32) -> Stack { Stack::new(crate::blocks::item_named("stone").unwrap(), count) }
    fn pair(a: Stack, b: Stack, at: (f64, f64, f64)) -> Items {
        let mut items = Items::new();
        items.watch_changes(1);
        items.drop_item(1, a, (0.0, 64.0, 0.0), (0.0, 0.0, 0.0), 10);
        items.drop_item(2, b, at, (0.0, 0.0, 0.0), 40);
        items
    }

    #[test]
    fn larger_stack_keeps_youngest_age_and_longest_remaining_delay() {
        let mut items = pair(stack(12), stack(20), (0.7, 64.0, 0.0));
        items.items[0].age = 5;
        items.items[1].age = 30;
        items.merge_items(&[1]);
        assert_eq!(items.items.len(), 1);
        let item = &items.items[0];
        assert_eq!((item.entity_id, item.stack.count, item.age, item.pickup_delay), (2, 32, 5, 15));
        let (changes, _) = items.changes_since(0, 1);
        assert!(changes.iter().any(|e| matches!(e, Change::Stack { stack, .. } if stack.count == 32)));
        assert!(changes.iter().any(|e| matches!(e, Change::Gone { entity_id: 1 })));
    }

    #[test]
    fn limits_components_distance_and_expiry_block_merging() {
        let modified = stack(1).with_extras(crate::stackdata::Data { damage: 1, ..Default::default() });
        for (a, b, at) in [(stack(40), stack(40), (0.0, 64.0, 0.0)), (stack(1), modified, (0.0, 64.0, 0.0)),
            (stack(1), stack(1), (0.75, 64.0, 0.0)), (stack(1), stack(1), (0.0, 64.25, 0.0))] {
            let mut items = pair(a, b, at); items.merge_items(&[1]); assert_eq!(items.items.len(), 2);
        }
        let mut items = pair(stack(1), stack(1), (0.0, 64.0, 0.0));
        items.items[0].age = LIFETIME;
        items.merge_items(&[1]); assert_eq!(items.items.len(), 2);
    }

    #[test]
    fn custom_data_controls_inventory_and_ground_merging_without_losing_types() {
        let tagged = |bytes: &[u8]| stack(2).with_extras(crate::stackdata::Data {
            custom_data: Some(crate::stackdata::CustomData::from_bytes(bytes).unwrap()), ..Default::default()
        });
        let first = tagged(&[10,1,0,1,b'a',1,3,0,1,b'b',0,0,0,2,0]);
        let same = tagged(&[10,3,0,1,b'b',0,0,0,2,1,0,1,b'a',1,0]);
        let different = tagged(&[10,2,0,1,b'a',0,1,3,0,1,b'b',0,0,0,2,0]);
        assert_eq!(first.data,same.data);
        assert!(!first.stacks_with(&different));
        let mut inventory = crate::inventory::Inventory::new();
        inventory.put(first); inventory.put(different); inventory.put(same);
        let counts: Vec<_> = inventory.slots().iter().flatten().map(|s|s.count).collect();
        assert_eq!(counts,vec![4,2]);
        let mut items = pair(first,different,(0.0,64.0,0.0));
        items.merge_items(&[1]);assert_eq!(items.items.len(),2);
        let mut items = pair(first,same,(0.0,64.0,0.0));
        items.merge_items(&[1]);assert_eq!(items.items.len(),1);
        assert_eq!(items.items[0].stack,first.with_count(4));
    }

    #[test]
    fn stationary_items_check_every_forty_ticks_and_equal_initiator_disappears() {
        let mut items = pair(stack(1), stack(1), (0.0, 64.0, 0.0));
        for item in &mut items.items { item.no_gravity = true; item.ticks = 0; }
        for _ in 0..39 { items.step(&|_, _, _| false, &|_, _, _| false, &|_, _| true); }
        assert_eq!(items.items.len(), 2);
        items.step(&|_, _, _| false, &|_, _, _| false, &|_, _| true);
        assert_eq!(items.items.len(), 1); assert_eq!(items.items[0].entity_id, 2);
    }

    #[test]
    fn xp_merges_equal_value_and_matching_ids_without_losing_pickups() {
        let mut items = Items::new();
        for (id, value) in [(1, 3), (41, 3), (2, 3), (81, 7)] {
            items.add_orb(id, value, (0.0, 64.0, 0.0), (0.0, 0.0, 0.0));
        }
        items.orbs[0].age = 100;
        items.merge_orbs();
        assert_eq!(items.orbs.len(), 3);
        assert_eq!((items.orbs[0].count, items.orbs[0].age), (2, 0));
        assert_eq!(items.take_orb((0.0, 64.0, 0.0), 10), Some(3));
        assert_eq!(items.orbs[0].count, 1);
        assert_eq!(items.take_orb((0.0, 64.0, 0.0), 10), Some(3));
        assert!(items.orbs.iter().all(|orb| orb.entity_id != 1));
    }

    #[test]
    fn crossing_blocks_checks_on_even_ticks_and_unloaded_items_wait() {
        let mut items = pair(stack(1), stack(1), (1.2, 64.0, 0.0));
        for item in &mut items.items { item.no_gravity = true; item.ticks = 1; }
        items.items[0].x = 0.9;
        items.items[0].vx = 0.2;
        items.step(&|_, _, _| false, &|_, _, _| false, &|_, _| false);
        assert_eq!(items.items.len(), 2);
        assert_eq!(items.items[0].ticks, 1);
        items.step(&|_, _, _| false, &|_, _, _| false, &|_, _| true);
        assert_eq!(items.items.len(), 1);
    }

    #[test]
    fn xp_period_is_twenty_ticks_and_awarding_preserves_total_value() {
        let mut items = Items::new();
        for id in [1, 41] { items.add_orb(id, 1, (0.0, 64.0, 0.0), (0.0, 0.0, 0.0)); }
        for _ in 0..19 { items.step_orbs(&[], &|_, _, _| false); }
        assert_eq!(items.orbs.len(), 2);
        items.step_orbs(&[], &|_, _, _| false);
        assert_eq!(items.orbs.len(), 1);
        let mut next = 100;
        let mut rng = crate::mob::Rng::new(17);
        for _ in 0..100 { items.drop_experience(1, (0.0, 64.0, 0.0), &mut || { next += 1; next }, &mut rng); }
        assert_eq!(items.orbs.iter().map(|orb| orb.count * orb.value).sum::<i32>(), 102);
        assert!(items.orbs.len() < 101, "есть слияние при выдаче опыта");
    }
}
