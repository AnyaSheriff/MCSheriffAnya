// Вики, Armor: «equip» — звук смены надетого предмета, включая команды.
// Износ, снятие и перестановка числа предметов не надевают новую броню.
use super::*;

fn sound(item: i32) -> Option<&'static str> {
    let name = blocks::item_name(item)?;
    match name {
        "elytra" => Some("minecraft:item.armor.equip_elytra"),
        "turtle_helmet" => Some("minecraft:item.armor.equip_turtle"),
        "carved_pumpkin" => Some("minecraft:item.armor.equip_generic"),
        _ if name.ends_with("_head") || name.ends_with("_skull") => Some("minecraft:item.armor.equip_generic"),
        _ => crate::combat::info(item).and_then(|i| (!i.equip_sound.is_empty()).then_some(i.equip_sound)),
    }
}

/// Вики, Armor: «swapping with any armor already equipped». Головы и тыквы
/// надеваются через инвентарь/раздатчик, а не правым щелчком в воздух.
pub(super) fn use_item(shared: &Shared, player: &mut PlayerState, hand: i32, sequence: i32) -> Option<Reaction> {
    if player.game_mode == GAME_MODE_SPECTATOR || player.vit.dead { return None; }
    let held = if hand == 1 { inventory::OFFHAND } else { player.inventory.selected_slot() };
    let stack = player.inventory.slot(held)?;
    let name = blocks::item_name(stack.item)?;
    let slot = if name.ends_with("_helmet") { 5 } else if name.ends_with("_chestplate") || name == "elytra" { 6 }
        else if name.ends_with("_leggings") { 7 } else if name.ends_with("_boots") { 8 } else { return None; };
    let old = player.inventory.slot(slot);
    if old == Some(stack) || (player.game_mode != GAME_MODE_CREATIVE && old.is_some_and(|s| s.extras().level("binding_curse") > 0)) {
        return Some(Reaction::accepted(sequence));
    }
    player.inventory.set(slot, Some(stack));
    player.inventory.set(held, old);
    // Публикация озвучит смену типа; обмен двух экземпляров одного типа
    // требует отдельного звука, иначе сравнение экипировки его пропустит.
    if old.is_some_and(|s| s.item == stack.item) && let Some(sound) = sound(stack.item) {
        shared.world_in(player.dimension).lock().expect("мир").note_sound_player_at((player.x,player.y,player.z),sound,1.0,1.0);
    }
    Some(Reaction { inventory: true, ..Reaction::accepted(sequence) })
}

pub(super) fn changed(before: &[Option<Stack>; 5], after: &[Option<Stack>; 5]) -> Vec<&'static str> {
    before.iter().zip(after).skip(1).filter_map(|(old, new)| {
        let new = (*new)?;
        if old.is_some_and(|old| old.item == new.item) { return None; }
        sound(new.item)
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn special_head_and_wing_equipment_sounds() {
        for (name,suffix) in [("elytra","elytra"),("turtle_helmet","turtle"),("carved_pumpkin","generic"),("player_head","generic"),("skeleton_skull","generic")] {
            assert_eq!(sound(blocks::item_named(name).unwrap()),Some(format!("minecraft:item.armor.equip_{suffix}").as_str()));
        }
    }
    #[test]
    fn equipping_sounds_once_but_damage_and_removal_are_silent() {
        let empty = [None;5];
        let mut worn = empty;
        let armor = Stack::new(blocks::item_named("diamond_chestplate").unwrap(),1);
        worn[2] = Some(armor);
        assert_eq!(changed(&empty,&worn),vec!["minecraft:item.armor.equip_diamond"]);
        let mut damaged = worn; damaged[2] = crate::combat::add_damage(armor,1);
        assert!(changed(&worn,&damaged).is_empty());
        assert!(changed(&worn,&empty).is_empty());
        assert!(changed(&worn,&worn).is_empty());
    }
}
