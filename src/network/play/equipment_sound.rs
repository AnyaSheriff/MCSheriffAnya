// Вики, Armor: «equip» — звук смены надетого предмета, включая команды.
// Износ, снятие и перестановка числа предметов не надевают новую броню.
use super::*;

pub(super) fn changed(before: &[Option<Stack>; 5], after: &[Option<Stack>; 5]) -> Vec<&'static str> {
    before.iter().zip(after).skip(1).filter_map(|(old, new)| {
        let new = (*new)?;
        if old.is_some_and(|old| old.item == new.item) { return None; }
        let info = crate::combat::info(new.item)?;
        (!info.equip_sound.is_empty()).then_some(info.equip_sound)
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
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
