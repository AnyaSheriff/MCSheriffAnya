// Куб серы 26.3: ведро, ножницы, слизь, поглощение блока рукой, огниво (mob/sulfur_cube.rs).
// Вики, Sulfur Cube (26.2/26.3). Источник чисел — одно наблюдение официального сервера
// (probe_sulfur_cube_777.py); звуки и износ ножниц `[проверить]`.
use super::*;
use crate::mob::{Kind, sulfur_cube as cube};

/// Взаимодействие предметом в руке с кубом серы; true — удар поглощён.
pub(super) fn use_on_mob(items: &mut items::Items, index: usize, player: &mut PlayerState, shared: &Shared, world: &mut World, reaction: &mut Reaction) -> bool {
    if items.mobs()[index].kind != Kind::SulfurCube { return false; }
    let Some(stack) = player.inventory.held() else { return false; };
    let Some(held) = blocks::item_name(stack.item) else { return false; };
    let survival = player.game_mode != GAME_MODE_CREATIVE;
    let slot = inventory::FIRST_HOTBAR + player.inventory.selected();
    let (id, at) = { let m = &items.mobs()[index]; (m.entity_id, (m.x, m.y, m.z)) };
    let mut rng = crate::mob::Rng::new(u64::from(shared.next_entity_id() as u32));

    match held {
        // Пустое ведро забирает большого куба без блока; малый не берётся [проверить: куб с блоком].
        "bucket" => {
            let mob = &items.mobs()[index];
            if mob.nether.size != cube::LARGE || mob.sulfur.body.is_some() || mob.is_dead() { return false; }
            let Some(filled) = blocks::item_named("sulfur_cube_bucket") else { return false; };
            let data = crate::stackdata::Data {
                name: mob.name.clone(),
                bucket: Some(crate::mob::water::bucket::BucketData {
                    health: mob.vitals.health.to_bits(), age: 0, cooldown: 0,
                    flags: u8::from(mob.no_ai) | (u8::from(mob.silent) << 1),
                }),
                ..Default::default()
            };
            let full = Stack::new(filled, 1).with_extras(data);
            items.remove_mobs(|m| m.entity_id == id);
            if survival {
                player.inventory.set(slot, Some(full)); reaction.slots.push(slot);
            } else {
                let (changed, _) = player.inventory.put(full);
                reaction.slots.extend(changed);
            }
            world.actor = Some(player.entity_id);
            world.note_sound_by_hand(at.0.floor() as i32, at.1.floor() as i32, at.2.floor() as i32, "minecraft:item.bucket.fill", 1.0, 1.0);
            world.actor = None;
            true
        }
        // Ножницы снимают блок: он падает предметом, 100 тактов куб ничего не подбирает.
        "shears" => {
            let Some(body) = cube::shear(&mut items.mobs_mut()[index]) else { return false; };
            items.flush_mob_events(&mut || shared.next_entity_id(), &mut rng);
            let velocity = (rng.unit() * 0.2 - 0.1, 0.2, rng.unit() * 0.2 - 0.1);
            items.drop_item(shared.next_entity_id(), Stack::new(body, 1), (at.0, at.1 + 0.5, at.2), velocity, items::MINED_DELAY);
            player.progress.used(stack.item);
            if survival {
                let worn = combat::wear(&stack, 1, false, || world.random_unit());
                if worn > 0 {
                    match combat::add_damage(stack, worn) {
                        Some(next) => player.inventory.set(slot, Some(next)),
                        None => {
                            player.progress.broken(stack.item);
                            player.inventory.set(slot, None);
                            world.note_equipment_break(player.entity_id, 47);
                        }
                    }
                    reaction.slots.push(slot);
                }
            }
            true
        }
        // Слизь ускоряет рост малого куба; выживание тратит предмет.
        "slime_ball" => {
            if !cube::feed_slimeball(&mut items.mobs_mut()[index]) { return false; }
            if survival && let Some(spent) = player.inventory.spend_held() { reaction.slots.push(spent); }
            true
        }
        // Огниво поджигает динамит внутри куба (запал 120 тактов).
        "flint_and_steel" => {
            if !cube::ignite(&mut items.mobs_mut()[index]) { return false; }
            true
        }
        // Блок из руки: большой куб без блока поглощает один.
        name => {
            if !cube::absorb(&mut items.mobs_mut()[index], stack.item, name) { return false; }
            player.progress.used(stack.item);
            if survival && let Some(spent) = player.inventory.spend_held() { reaction.slots.push(spent); }
            true
        }
    }
}

/// Ведро с кубом серы: выпускает большого куба в клетку за гранью щелчка, ведро становится пустым.
pub(super) fn use_bucket(item: i32, clicked: (i32, i32, i32), face: i32, sequence: i32, player: &mut PlayerState, shared: &Shared) -> Option<Reaction> {
    if blocks::item_name(item) != Some("sulfur_cube_bucket") { return None; }
    let empty = blocks::item_named("bucket")?;
    let data = player.inventory.held().map(|stack| stack.extras()).unwrap_or_default();
    let (x, y, z) = offset_towards(clicked.0, clicked.1, clicked.2, face);
    let mut mob = crate::mob::Mob::new(shared.next_entity_id(), Kind::SulfurCube, (f64::from(x) + 0.5, f64::from(y), f64::from(z) + 0.5));
    cube::initialize(&mut mob);
    mob.sulfur.from_bucket = true;
    mob.persistent_tag = true;
    mob.name = data.name.clone();
    if let Some(bucket) = &data.bucket {
        let health = f32::from_bits(bucket.health);
        if health.is_finite() && health > 0.0 { mob.vitals.health = health.min(mob.vitals.max_health); }
        mob.no_ai = bucket.flags & 1 != 0;
        mob.silent = bucket.flags & 2 != 0;
    }
    mob.yaw = player.yaw;
    mob.head_yaw = player.yaw;
    {
        let mut world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");
        world.actor = Some(player.entity_id);
        world.note_sound_by_hand(x, y, z, "minecraft:item.bucket.empty", 1.0, 1.0);
        world.actor = None;
    }
    shared.items_in(player.dimension).lock().expect("предметы захвачены другим потоком").add_mob_late(mob);
    player.progress.used(item);
    Some(Reaction { slots: swap_held(player, empty, shared), ..Reaction::accepted(sequence) })
}
