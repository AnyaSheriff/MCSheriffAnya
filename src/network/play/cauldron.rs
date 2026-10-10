// Котёл: вёдра (вода, лава, рыхлый снег), бутылки и мытьё шулкер-ящиков.
//
// Как устроено у оригинала, взято с minecraft.wiki (Cauldron); звуки и
// порядок пакетов не сверены чёрным ящиком — [проверить]. Наполнение
// капельником реализовано в crate::dripstone; дождь, мытьё кожаной брони
// и знамён пока не сделаны.

use super::*;

/// Что лежит в котле: пустой, вода (уровень), лава, рыхлый снег (уровень).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Fill {
    Empty,
    Water(u8),
    Lava,
    Snow(u8),
}

fn fill_of(state: i32) -> Option<Fill> {
    let name = blocks::block_at_state(state)?;
    let level = || blocks::orientation_of(state)?.value(state, "level")?.parse::<u8>().ok();

    Some(match name {
        "cauldron" => Fill::Empty,
        "water_cauldron" => Fill::Water(level()?),
        "lava_cauldron" => Fill::Lava,
        "powder_snow_cauldron" => Fill::Snow(level()?),
        _ => return None,
    })
}

fn state_of(fill: Fill) -> Option<i32> {
    match fill {
        Fill::Empty => blocks::state_by_name("cauldron"),
        Fill::Lava => blocks::state_by_name("lava_cauldron"),
        Fill::Water(level) | Fill::Snow(level) => {
            let name = if matches!(fill, Fill::Water(_)) { "water_cauldron" } else { "powder_snow_cauldron" };
            let level = level.to_string();

            blocks::orientation(name)?.state(&[("level", level.as_str())])
        }
    }
}

/// Правый щелчок по котлу предметом в руке: ведро, бутылка, шулкер-ящик.
/// None — это не котёл или предмет здесь ничего не делает.
pub(super) fn use_cauldron(clicked: (i32, i32, i32), face: i32, sequence: i32, player: &mut PlayerState, shared: &Shared) -> Option<Reaction> {
    let (x, y, z) = clicked;
    let held = player.inventory.held()?;
    let item = blocks::item_name(held.item)?;
    let mut world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");
    let here = fill_of(world.get_block(x, y, z))?;
    let at = (f64::from(x) + 0.5, f64::from(y) + 0.5, f64::from(z) + 0.5);
    let creative = player.game_mode == GAME_MODE_CREATIVE;

    // Что станет с котлом, с рукой и каким звуком.
    enum Hand {
        Swap(&'static str),
        SwapStack(Stack),
    }

    let (next, hand, sound): (Fill, Hand, &'static str) = match (item, here) {
        ("water_bucket", Fill::Empty | Fill::Water(1..=2)) => (Fill::Water(3), Hand::Swap("bucket"), "minecraft:item.bucket.empty"),
        ("lava_bucket", Fill::Empty) => (Fill::Lava, Hand::Swap("bucket"), "minecraft:item.bucket.empty_lava"),
        ("powder_snow_bucket", Fill::Empty) => (Fill::Snow(3), Hand::Swap("bucket"), "minecraft:item.bucket.empty_powder_snow"),
        ("bucket", Fill::Water(3)) => (Fill::Empty, Hand::Swap("water_bucket"), "minecraft:item.bucket.fill"),
        ("bucket", Fill::Lava) => (Fill::Empty, Hand::Swap("lava_bucket"), "minecraft:item.bucket.fill_lava"),
        ("bucket", Fill::Snow(3)) => (Fill::Empty, Hand::Swap("powder_snow_bucket"), "minecraft:item.bucket.fill_powder_snow"),
        // Бутылка черпает уровень воды; вода из бутылки добавляет уровень.
        ("glass_bottle", Fill::Water(level)) => {
            let data = crate::stackdata::Data { potion: crate::survival::potion_number("water"), ..Default::default() };

            let bottle = Stack::new(blocks::item_named("potion")?, 1).with_extras(data);
            let next = if level <= 1 { Fill::Empty } else { Fill::Water(level - 1) };

            (next, Hand::SwapStack(bottle), "minecraft:item.bottle.fill")
        }
        ("potion", Fill::Empty | Fill::Water(1..=2)) if held.extras().potion == crate::survival::potion_number("water") => {
            let next = match here {
                Fill::Water(level) => Fill::Water(level + 1),
                _ => Fill::Water(1),
            };

            (next, Hand::Swap("glass_bottle"), "minecraft:item.bottle.empty")
        }
        // Цветной шулкер-ящик в воде становится простым, с тем же
        // содержимым, и стоит уровень воды (вики, Cauldron: «washing»).
        (name, Fill::Water(level)) if name.ends_with("_shulker_box") => {
            let plain = Stack { item: blocks::item_named("shulker_box")?, ..held };
            let next = if level <= 1 { Fill::Empty } else { Fill::Water(level - 1) };

            (next, Hand::SwapStack(plain), "")
        }
        _ => return None,
    };

    let state = state_of(next)?;

    player.progress.used(held.item);
    player.progress.custom(match item {
        "bucket" | "glass_bottle" => "use_cauldron",
        name if name.ends_with("_shulker_box") => "clean_shulker_box",
        _ => "fill_cauldron",
    }, 1);
    world.set_block(x, y, z, state);

    if !sound.is_empty() {
        world.note_sound_at_block(at, sound, 1.0, 1.0);
    }

    // После смены — повтор блока за гранью и самого блока (чёрный ящик,
    // cauldron-fill).
    let (fx, fy, fz) = offset_towards(x, y, z, face);

    world.tell_directly(fx, fy, fz);
    world.tell_directly(x, y, z);
    redstone::settle(&mut world);
    drop(world);

    let slots = if creative {
        Vec::new()
    } else {
        match hand {
            Hand::Swap(name) => swap_held(player, blocks::item_named(name)?, shared),
            Hand::SwapStack(stack) => {
                let slot = inventory::FIRST_HOTBAR + player.inventory.selected();

                if held.count > 1 {
                    player.inventory.spend_held();

                    let mut slots = vec![slot];
                    let (changed, left) = player.inventory.put(stack);

                    slots.extend(changed);

                    if let Some(left) = left {
                        throw_out(shared, player, left);
                    }

                    slots
                } else {
                    player.inventory.set(slot, Some(stack));
                    vec![slot]
                }
            }
        }
    };

    Some(Reaction { slots, ..Reaction::accepted(sequence) })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Котёл читается и собирается обратно: вода уровня 2, лава, пустой.
    #[test]
    fn fills_round_trip() {
        for fill in [Fill::Empty, Fill::Water(1), Fill::Water(3), Fill::Lava, Fill::Snow(2)] {
            assert_eq!(fill_of(state_of(fill).expect("состояние")), Some(fill));
        }
    }
}
