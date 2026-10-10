// Маяк: minecraft.wiki/w/Beacon — «every four seconds», радиус 20/30/40/50.
// [проверить] события, звуки и вертикальная граница в 26.1.2.
use crate::{blocks, containers::{Container, Kind, Pos}, dimension::Dimension, shared::Shared, survival::{Effect, EffectKind}, world::{self, World}};

pub fn payment(item: i32) -> bool {
    matches!(blocks::item_name(item), Some("iron_ingot" | "gold_ingot" | "emerald" | "diamond" | "netherite_ingot"))
}

pub fn level(world: &World, p: Pos) -> i32 {
    let mut level = 0;
    for n in 1..=4 {
        for x in p.0 - n..=p.0 + n {
            for z in p.2 - n..=p.2 + n {
                if !matches!(blocks::block_at_state(world.get_block(x, p.1 - n, z)),
                    Some("iron_block" | "gold_block" | "emerald_block" | "diamond_block" | "netherite_block")) { return level; }
            }
        }
        level = n;
    }
    level
}

pub fn clear_beam(world: &World, p: Pos) -> bool {
    (p.1 + 1..world::MIN_Y + world::WORLD_HEIGHT).all(|y| {
        let s = world.get_block(p.0, y, p.2);
        let name = blocks::block_at_state(s).unwrap_or("");
        name == "bedrock" || crate::world::light::filter(s) < 15
    })
}

pub fn valid_primary(level: i32, effect: i32) -> bool {
    match effect { 0 | 2 => level >= 1, 10 | 7 => level >= 2, 4 => level >= 3, _ => false }
}

pub fn valid_selection(level: i32, primary: i32, secondary: i32) -> bool {
    valid_primary(level, primary) && (secondary == -1 || (level == 4 && (secondary == 9 || secondary == primary)))
}

pub fn effects(level: i32, primary: i32, secondary: i32) -> Vec<Effect> {
    if !valid_primary(level, primary) { return Vec::new(); }
    let kind = EffectKind::ALL.into_iter().find(|k| k.id() == primary).expect("сила маяка");
    let first = Effect { kind, amplifier: i32::from(level == 4 && secondary == primary), ticks_left: (9 + 2 * level) * 20, ambient: true, visible: true };
    let mut result = vec![first];
    if level == 4 && secondary == 9 && primary != 9 { result.push(Effect { kind: EffectKind::Regeneration, amplifier: 0, ..first }); }
    result
}

pub fn nbt(c: &Container) -> Vec<u8> {
    let mut out = crate::network::nbt::Compound::new().int("Levels", c.beacon[0]);
    for (name, id) in [("primary_effect", c.beacon[1]), ("secondary_effect", c.beacon[2])] {
        if let Some(effect) = EffectKind::ALL.into_iter().find(|k| k.id() == id) {
            // Имя из таблицы эффектов, без новых реестров/дампов.
            let key = match effect { EffectKind::Speed => "speed", EffectKind::Haste => "haste", EffectKind::Resistance => "resistance", EffectKind::JumpBoost => "jump_boost", EffectKind::Strength => "strength", EffectKind::Regeneration => "regeneration", _ => continue };
            out = out.string(name, &format!("minecraft:{key}"));
        }
    }
    out.encode_network()
}

pub fn tick(shared: &Shared, dimension: Dimension, world: &mut World) {
    // Фаза как у датчика света: (такт + 1) кратно 80 (чёрный ящик, beacon-effect).
    if !(world.tick() + 1).is_multiple_of(80) { return; }
    let positions: Vec<_> = world.containers_in_order().into_iter().filter(|p| world.container(p.0, p.1, p.2).is_some_and(|c| c.kind == Kind::Beacon)).collect();
    for p in positions {
        let old = world.container(p.0, p.1, p.2).unwrap().beacon;
        // Уровень считается по пирамиде; закрытый луч лишь гасит силу и звук без
        // «deactivate» и без правки блок-сущности (чёрный ящик, beacon-effect:
        // камень над маяком на +170 — больше ни усилений, ни звуков, Levels прежний).
        let levels = level(world, p);
        let clear = clear_beam(world, p);
        world.with_container(p.0, p.1, p.2, |c| c.beacon[0] = levels);
        if old[0] != levels {
            let data = nbt(world.container(p.0, p.1, p.2).unwrap());
            world.note_block_entity(p.0, p.1, p.2, Kind::Beacon.block_entity_type(), data);
            world.note_sound(p.0, p.1, p.2, if levels > 0 { "minecraft:block.beacon.activate" } else { "minecraft:block.beacon.deactivate" }, 1.0, 1.0);
        }
        if levels == 0 || !clear { continue; }
        world.note_sound(p.0, p.1, p.2, "minecraft:block.beacon.ambient", 1.0, 1.0);
        let range = f64::from(10 + levels * 10);
        let mut players = shared.players.lock().expect("игроки");
        let targets: Vec<_> = players.members().iter().filter(|m| m.dimension == dimension && m.game_mode != 3
            && m.position.x >= p.0 as f64 - range && m.position.x < p.0 as f64 + 1.0 + range
            && m.position.z >= p.2 as f64 - range && m.position.z < p.2 as f64 + 1.0 + range
            && m.position.y >= p.1 as f64 - range && m.position.y < p.1 as f64 + 1.0 + range + world::WORLD_HEIGHT as f64).map(|m| m.uuid).collect();
        for uuid in targets {
            for effect in effects(levels, old[1], old[2]) { players.order(&uuid, crate::players::Order::Effect(effect)); }
        }
        if levels > old[0] {
            // Advancement: «20×20×14 cuboid centered on a beacon block».
            let nearby: Vec<_> = players.members().iter().filter(|m| m.dimension == dimension
                && (m.position.x - (p.0 as f64 + 0.5)).abs() <= 10.0
                && (m.position.y - (p.1 as f64 + 0.5)).abs() <= 7.0
                && (m.position.z - (p.2 as f64 + 0.5)).abs() <= 10.0).map(|m| m.uuid).collect();
            for uuid in nearby {
                let mut ids = vec!["nether/create_beacon"];
                if levels == 4 { ids.push("nether/create_full_beacon"); }
                players.order(&uuid, crate::players::Order::Advancement { grant: true, ids, criterion: Some("construct_beacon".to_owned()) });
            }
        }
    }
}

/// Поля блок-сущности для setblock/data merge (вики, Beacon, Block data).
pub fn apply_text(c: &mut Container, text: &str) -> bool {
    if c.kind != Kind::Beacon { return false; }
    let Some(value) = crate::snbt::parse(text) else { return false; };
    for (key, slot) in [("primary_effect", 1), ("secondary_effect", 2)] {
        if let Some(name) = value.get(key).and_then(crate::snbt::Value::as_str) {
            c.beacon[slot] = EffectKind::from_name(name.strip_prefix("minecraft:").unwrap_or(name)).map_or(-1, |e| e.id());
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_powers_survive_storage() {
        let mut c = Container::new(Kind::Beacon);
        c.beacon = [4, 4, 9];
        let mut bytes = Vec::new();
        c.write((0, 70, 0), &mut bytes);
        let (_, read) = Container::read(&mut bytes.as_slice(), 0, 0).unwrap();
        assert_eq!(read.beacon, c.beacon);
        assert!(read.is_empty());
    }

    #[test]
    fn selection_gates_and_duration() {
        assert!(!valid_selection(1, 4, -1));
        assert!(!valid_selection(3, 0, 9));
        assert!(!valid_selection(4, 18, 9));
        assert!(valid_selection(4, 4, 4));
        assert_eq!(effects(4, 4, 4)[0].amplifier, 1);
        assert_eq!(effects(4, 4, 9).len(), 2);
        assert_eq!(effects(1, 0, -1)[0].ticks_left, 220);
        assert_eq!(effects(4, 0, -1)[0].ticks_left, 340);
    }
    #[test]
    fn payment_slot_rejects_shifted_stone_and_accepts_one_ingot() {
        let mut inventory = crate::inventory::Inventory::new();
        let stone = crate::inventory::Stack::new(blocks::item_named("stone").unwrap(), 64);
        let iron = crate::inventory::Stack::new(blocks::item_named("iron_ingot").unwrap(), 64);
        let mut slots = vec![None];
        inventory.set(9, Some(stone));
        let mut window = crate::window::Window { container: &mut slots, inventory: &mut inventory, kind: Some(Kind::Beacon), horse: None };
        window.click(1, 0, 1);
        assert_eq!(window.container[0], None);
        assert_eq!(window.inventory.slot(9), None);
        assert_eq!(window.inventory.slot(crate::inventory::FIRST_HOTBAR), Some(stone));
        window.inventory.set(9, Some(iron));
        window.click(1, 0, 1);
        assert_eq!(window.container[0], Some(iron.with_count(1)));
        assert_eq!(window.inventory.slot(9), Some(iron.with_count(63)));
    }

    #[test]
    fn pyramid_requires_every_block() {
        let mut w = World::in_memory();
        let p = (6, 70, 6);
        for n in 1..=4 { for x in 6-n..=6+n { for z in 6-n..=6+n { w.set_block(x, 70-n, z, blocks::state_by_name("iron_block").unwrap()); } } }
        assert_eq!(level(&w, p), 4);
        w.set_block(2, 66, 2, world::AIR);
        assert_eq!(level(&w, p), 3);
        assert!(clear_beam(&w, p));
        w.set_block(6, 80, 6, blocks::state_by_name("red_stained_glass").unwrap());
        assert!(clear_beam(&w, p));
        w.set_block(6, 81, 6, blocks::state_by_name("stone").unwrap());
        assert!(!clear_beam(&w, p));
    }
}
