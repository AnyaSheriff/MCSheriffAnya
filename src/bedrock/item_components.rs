// Довески общей стопки для Bedrock 944. Вики, Arrow: «data value»;
// Map item format: «map_uuid». [проверить] отображение живым клиентом.
use crate::{blocks, inventory::Stack};
use super::nbt::{self, Style, Tag};

pub fn metadata(stack: &Stack, plain: u32) -> u32 {
    let data = stack.extras();
    match blocks::item_name(stack.item) {
        Some("potion" | "splash_potion" | "lingering_potion" | "tipped_arrow") => {
            let name = data.potion.and_then(|id| crate::survival::POTIONS.get(usize::from(id))).copied().unwrap_or("water");
            let potion = super::things::bedrock_potion(name) as u32;
            potion + u32::from(blocks::item_name(stack.item) == Some("tipped_arrow"))
        }
        Some("goat_horn") => u32::from(data.instrument.unwrap_or(0).min(7)),
        Some("ominous_bottle") => u32::from(data.ominous.unwrap_or(0).min(4)),
        _ => plain,
    }
}

pub fn augment(stack: &Stack, original: Option<Vec<u8>>) -> Option<Vec<u8>> {
    let data = stack.extras();
    let mut root = original.as_ref().and_then(|bytes| nbt::decode(bytes, Style::Little)).unwrap_or_else(Tag::compound);
    if let Some(id) = data.map_id {
        root.put("map_uuid", Tag::Long(i64::from(id)));
        root.int("map_name_index", id);
    }
    if data.unbreakable { root.put("Unbreakable", Tag::Byte(1)); }
    if data.ominous_banner { root.int("Type", 1); }
    if let Some(color) = data.potion_extra.as_ref().and_then(|extra| extra.color) {
        root.int("customColor", color);
    }
    if root == Tag::compound() { None } else { Some(root.encode(Style::Little)) }
}

/// Стопка внутри блок-актора: Bedrock item format, «Name», «Count», «Damage».
/// Довески предметов имеют тот же смысл, но другой порядок байтов в сети.
pub fn actor_item(stack: Stack) -> Tag {
    let mut item = Tag::compound();
    if let Some((id, aux, _)) = super::inventory::bedrock_item(stack.item) {
        let name = item_name(id);
        item.string("Name", &format!("minecraft:{name}"));
        item.put("Count", Tag::Byte(stack.count.clamp(0, 255) as u8));
        item.put("Damage", Tag::Short(metadata(&stack, aux) as i16));
        if let Some(tag) = super::stations::item_nbt(&stack).and_then(|bytes| nbt::decode(&bytes, Style::Little)) {
            item.put("tag", tag);
        }
    }
    item
}
fn item_name(id: i32) -> &'static str {
    use std::{collections::HashMap, sync::OnceLock};
    static NAMES: OnceLock<HashMap<i32, &'static str>> = OnceLock::new();
    NAMES.get_or_init(|| include_str!("item_names.txt").lines().filter_map(|line| {
        let (id, name) = line.split_once('\t')?; Some((id.parse().ok()?, name))
    }).collect()).get(&id).copied().unwrap_or("air")
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn components_roundtrip() {
        let stack = Stack::new(blocks::item_named("filled_map").unwrap(), 1).with_extras(crate::stackdata::Data {
            map_id: Some(123), unbreakable: true, ..Default::default()
        });
        let bytes = augment(&stack, None).unwrap();
        let tag = nbt::decode(&bytes, Style::Little).unwrap();
        assert_eq!(tag.get("map_uuid"), Some(&Tag::Long(123)));
        assert_eq!(tag.get("Unbreakable"), Some(&Tag::Byte(1)));
        let actor = actor_item(stack);
        assert_eq!(nbt::decode(&actor.encode(Style::Network), Style::Network), Some(actor));
    }
    #[test]
    fn potion_arrow_and_horn_aux() {
        for (item, value) in [("potion", 25), ("splash_potion", 25), ("lingering_potion", 25), ("tipped_arrow", 26)] {
            let stack = Stack::new(blocks::item_named(item).unwrap(), 1).with_extras(crate::stackdata::Data {
                potion: crate::survival::potion_number("poison"), ..Default::default()
            });
            assert_eq!(metadata(&stack, 0), value);
        }
    }
}
