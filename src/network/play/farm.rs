// Мотыга и костная мука по блоку (правила — в farming.rs). Порядок ответа —
// как у прочих щелчков (чёрный ящик, farm-till): повтор самого блока,
// повтор за гранью, потом общая пачка.

use super::*;
use crate::combat;

/// Щелчок по кусту ягод любой рукой (кроме присевшего): сбор ягод.
pub(super) fn pick_berries(player: &mut PlayerState, shared: &Shared, clicked: (i32, i32, i32), face: i32, sequence: i32) -> Option<Reaction> {
    let mut world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");
    let stack = crate::farming::pick_berries(&mut world, clicked)?;
    let beyond = offset_towards(clicked.0, clicked.1, clicked.2, face);
    let (at, velocity) = crate::tick::scattered(clicked, || f64::from(world.random_unit()));

    world.tell_directly(clicked.0, clicked.1, clicked.2);
    world.tell_directly(beyond.0, beyond.1, beyond.2);
    drop(world);
    drop_into_world(shared, player.dimension, stack, at, velocity, items::MINED_DELAY);

    Some(Reaction::accepted(sequence))
}

/// Щелчок мотыгой или костной мукой. None — в руке не они или блок к ним
/// глух: щелчок разбирается дальше по-обычному.
pub(super) fn use_on_block(player: &mut PlayerState, shared: &Shared, clicked: (i32, i32, i32), face: i32, sequence: i32) -> Option<Reaction> {
    let held = player.inventory.held()?;
    let name = blocks::item_name(held.item)?;
    let hoe = name.ends_with("_hoe");

    if !hoe && name != "bone_meal" || player.game_mode == GAME_MODE_ADVENTURE {
        return None;
    }

    let creative = player.game_mode == GAME_MODE_CREATIVE;
    let beyond = offset_towards(clicked.0, clicked.1, clicked.2, face);
    let mut world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");

    // Не вышло (мотыга по камню, мука по спелому): оригинал всё равно
    // повторяет блок и клетку за гранью (чёрный ящик, farm-till-wheat).
    let refuse = |world: &mut World| {
        world.tell_directly(clicked.0, clicked.1, clicked.2);
        world.tell_directly(beyond.0, beyond.1, beyond.2);

        Some(Reaction::accepted(sequence))
    };

    if hoe {
        // Звук вспашки щёлкнувшему не шлётся: его клиент играет сам.
        world.actor = Some(player.entity_id);
        let tilled = crate::farming::till(&mut world, clicked, face);
        world.actor = None;

        let Some(dropped) = tilled else {
            return refuse(&mut world);
        };

        world.tell_directly(clicked.0, clicked.1, clicked.2);
        world.tell_directly(beyond.0, beyond.1, beyond.2);
        redstone::settle(&mut world);

        // Мотыга изнашивается на единицу (вики, Hoe: «uses 1 durability»).
        let mut slots = Vec::new();

        if !creative {
            let worn = combat::wear(&held, 1, false, || world.random_unit());
            let slot = inventory::FIRST_HOTBAR + player.inventory.selected();

            player.inventory.set(slot, combat::add_damage(held, worn));
            slots.push(slot);
        }

        // Висячие корни из земли с корнями выскакивают из верхней грани:
        // точка на грани ±0,25 от середины, толчок 0,2 вверх и до 0,1 вбок
        // (чёрный ящик, farm-till-wheat: 2,32 / 0 / 2,72 у блока 2 −1 2).
        let popped = dropped.map(|stack| {
            let (x, y, z) = clicked;
            let mut random = || f64::from(world.random_unit());
            let at = (f64::from(x) + 0.25 + random() * 0.5, f64::from(y) + 1.0, f64::from(z) + 0.25 + random() * 0.5);
            let velocity = (random() * 0.2 - 0.1, 0.2, random() * 0.2 - 0.1);

            (stack, at, velocity)
        });

        drop(world);

        if let Some((stack, at, velocity)) = popped {
            drop_into_world(shared, player.dimension, stack, at, velocity, items::MINED_DELAY);
        }

        return Some(Reaction { slots, ..Reaction::accepted(sequence) });
    }

    if crate::farming::bone_meal(&mut world, clicked).is_none() {
        return refuse(&mut world);
    }

    world.tell_directly(clicked.0, clicked.1, clicked.2);
    world.tell_directly(beyond.0, beyond.1, beyond.2);
    redstone::settle(&mut world);
    drop(world);

    Some(Reaction { slots: spend_placed(player), ..Reaction::accepted(sequence) })
}
