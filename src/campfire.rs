// Костры: minecraft.wiki/w/Campfire — «up to four food items», «30 seconds».
// [проверить] порядок сетевых событий и дыма; сценарий campfire-cook ещё не запускался.
use crate::{blocks, containers::{Container, Kind, Pos}, inventory::Stack, world::World};

pub const COOK_TICKS: i32 = 600;

pub fn is_campfire(state: i32) -> bool {
    matches!(blocks::block_at_state(state), Some("campfire" | "soul_campfire"))
}

pub fn lit(state: i32) -> bool {
    is_campfire(state) && blocks::orientation_of(state).and_then(|o| o.value(state, "lit")) == Some("true")
}

pub fn damage(state: i32) -> f32 {
    if !lit(state) { 0.0 } else if blocks::block_at_state(state) == Some("soul_campfire") { 2.0 } else { 1.0 }
}

pub fn recipe(item: i32) -> Option<i32> {
    crate::furnace::smelt(Kind::Smoker, item).map(|(out, _)| out)
}

pub fn nbt(c: &Container) -> Vec<u8> {
    let items = c.slots.iter().enumerate().filter_map(|(i, stack)| {
        let stack = stack.as_ref()?;
        Some(crate::network::nbt::Compound::new().int("count", stack.count).byte("Slot", i as i8)
            .string("id", &format!("minecraft:{}", blocks::item_name(stack.item)?)))
    }).collect();
    // Чёрный ящик, campfire-cook: клиенту уходят только Items, без счётчиков готовки.
    crate::network::nbt::Compound::new().compounds("Items", items).encode_network()
}

pub fn publish(world: &mut World, p: Pos) {
    if let Some(c) = world.container(p.0, p.1, p.2) {
        let data = nbt(c);
        world.note_block_entity(p.0, p.1, p.2, Kind::Campfire.block_entity_type(), data);
    }
}

pub fn insert(world: &mut World, p: Pos, stack: Stack) -> bool {
    if recipe(stack.item).is_none() || !lit(world.get_block(p.0, p.1, p.2)) { return false; }
    let inserted = world.with_container(p.0, p.1, p.2, |c| {
        let Some(i) = c.slots.iter().position(Option::is_none) else { return false; };
        c.slots[i] = Some(stack.with_count(1));
        // Чёрный ящик, campfire-cook: блок-сущность отсчитывает и в такт
        // вставки (предмет готов на +599, не на +600).
        c.cooking[i] = 1;
        true
    }).unwrap_or(false);
    if inserted { publish(world, p); }
    inserted
}

pub fn set_lit(world: &mut World, p: Pos, on: bool) -> bool {
    let state = world.get_block(p.0, p.1, p.2);
    if !is_campfire(state) || lit(state) == on { return false; }
    let o = blocks::orientation_of(state).expect("состояния костра");
    if on && o.value(state, "waterlogged") == Some("true") { return false; }
    let new = o.with(state, "lit", if on { "true" } else { "false" }).unwrap_or(state);
    world.set_block(p.0, p.1, p.2, new);
    if !on {
        world.note_sound(p.0, p.1, p.2, "minecraft:block.fire.extinguish", 1.0, 1.0);
        world.note_particle(55, None, (p.0 as f64 + 0.5, p.1 as f64 + 0.5, p.2 as f64 + 0.5), (0.3, 0.3, 0.3), 0.0, 20);
    }
    true
}

pub fn tick(world: &mut World, p: Pos) {
    let mut state = world.get_block(p.0, p.1, p.2);
    if let Some(o) = blocks::orientation_of(state) {
        if o.value(state, "waterlogged") == Some("true") { set_lit(world, p, false); }
        state = world.get_block(p.0, p.1, p.2);
        let signal = blocks::block_at_state(world.get_block(p.0, p.1 - 1, p.2)) == Some("hay_block");
        if let Some(new) = o.with(state, "signal_fire", if signal { "true" } else { "false" })
            && new != state { world.set_block(p.0, p.1, p.2, new); state = new; }
    }
    let on = lit(state);
    if world.with_container(p.0, p.1, p.2, |c| std::mem::take(&mut c.publish_due)).unwrap_or(false) {
        // Чёрный ящик, campfire-cook: готовое блюдо — блок и данные уходят вместе (sendBlockUpdated).
        world.tell_later(p.0, p.1, p.2, 1);
        publish(world, p);
    }
    let mut drops = Vec::new();
    world.with_container(p.0, p.1, p.2, |c| {
        for i in 0..4 {
            let Some(stack) = c.slots[i] else { continue; };
            if !on { c.cooking[i] = (c.cooking[i] - 2).max(0); continue; }
            c.cooking[i] += 1;
            if c.cooking[i] >= COOK_TICKS && let Some(item) = recipe(stack.item) {
                drops.push(Stack::new(item, 1));
                c.slots[i] = None;
                c.cooking[i] = 0;
            }
        }
    });
    if !drops.is_empty() {
        // Чёрный ящик, campfire-cook: предмет появляется в случайной точке над
        // костром (x, z в ±0,4 от центра, y 0,23–0,63) с толчком (±0,11; 0,17–0,24).
        for stack in drops {
            let (a, b, c, d, e, f) = (world.random_unit(), world.random_unit(), world.random_unit(), world.random_unit(), world.random_unit(), world.random_unit());
            let at = (p.0 as f64 + 0.5 + f64::from(a - 0.5) * 0.8, p.1 as f64 + 0.23 + f64::from(b) * 0.4, p.2 as f64 + 0.5 + f64::from(c - 0.5) * 0.8);
            world.eject(at, (f64::from(d - 0.5) * 0.22, 0.17 + f64::from(e) * 0.07, f64::from(f - 0.5) * 0.22), stack);
        }
        world.with_container(p.0, p.1, p.2, |c| c.publish_due = true);
    }
    // Дым обычного и сигнального костра рисует Java-клиент по lit/signal_fire.
    // [Bedrock: потом] содержимое и CookingTimes блок-сущности.
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn food_only_and_damage() {
        assert_eq!(recipe(blocks::item_named("beef").unwrap()), blocks::item_named("cooked_beef"));
        assert_eq!(recipe(blocks::item_named("iron_ore").unwrap()), None);
        assert_eq!(damage(blocks::state_by_name("campfire").unwrap()), 1.0);
        assert_eq!(damage(blocks::state_by_name("soul_campfire").unwrap()), 2.0);
    }
    #[test]
    fn four_slots_finish_independently_and_cool_when_unlit() {
        let mut w = World::in_memory();
        let p = (0, 70, 0);
        w.set_block(p.0, p.1, p.2, blocks::state_by_name("campfire").unwrap());
        let food = Stack::new(blocks::item_named("beef").unwrap(), 4);
        assert!(insert(&mut w, p, food));
        for _ in 0..100 { tick(&mut w, p); }
        for _ in 0..3 { assert!(insert(&mut w, p, food)); }
        assert!(!insert(&mut w, p, food));
        for _ in 0..498 { tick(&mut w, p); }
        assert!(w.take_ejected().is_empty());
        tick(&mut w, p);
        assert_eq!(w.take_ejected().len(), 1);
        assert_eq!(w.container(p.0, p.1, p.2).unwrap().cooking, [0, 500, 500, 500]);
        assert!(set_lit(&mut w, p, false));
        for _ in 0..10 { tick(&mut w, p); }
        assert_eq!(w.container(p.0, p.1, p.2).unwrap().cooking, [0, 480, 480, 480]);
        assert!(set_lit(&mut w, p, true));
        for _ in 0..120 { tick(&mut w, p); }
        assert_eq!(w.take_ejected().len(), 3);
        assert!(w.container(p.0, p.1, p.2).unwrap().is_empty());
    }

    #[test]
    fn wet_campfire_cannot_light_and_hay_enables_signal() {
        let mut w = World::in_memory();
        let p = (0, 70, 0);
        let wet = blocks::state_from_text("campfire[waterlogged=true,lit=false]").unwrap();
        w.set_block(p.0, p.1, p.2, wet);
        assert!(!set_lit(&mut w, p, true));
        assert!(!insert(&mut w, p, Stack::new(blocks::item_named("cod").unwrap(), 1)));
        w.set_block(p.0, p.1 - 1, p.2, blocks::state_by_name("hay_block").unwrap());
        tick(&mut w, p);
        let state = w.get_block(p.0, p.1, p.2);
        assert_eq!(blocks::orientation_of(state).unwrap().value(state, "signal_fire"), Some("true"));
    }

    #[test]
    fn timers_survive_storage() {
        let mut c = Container::new(Kind::Campfire);
        c.cooking = [1, 20, 599, 0];
        c.slots[0] = Some(Stack::new(blocks::item_named("cod").unwrap(), 1));
        let mut bytes = Vec::new();
        c.write((0, 64, 0), &mut bytes);
        let (_, read) = Container::read(&mut bytes.as_slice(), 0, 0).unwrap();
        assert_eq!(read.cooking, c.cooking);
        assert_eq!(read.slots, c.slots);
    }
}
