// Инвентарь доступен по снимку подключения; записи идут через обычные приказы.
use serde_json::{Value, Map, json};
use super::api::{PluginCtx, parse_uuid, mcsa::api::inventory};
use crate::inventory::Stack;
use crate::players::Order;

// Гость получает только локальный токен уже виденных довесков, а не произвольный NBT.
#[derive(Default)]
pub struct NativeItems {
    next: u32,
    entries: std::collections::BTreeMap<u32, Stack>,
}
impl NativeItems {
    fn remember(&mut self, stack: Stack) -> u32 {
        if let Some((token, _)) = self.entries.iter().find(|(_, known)| known.data == stack.data && known.tnt == stack.tnt && known.item == stack.item) { return *token; }
        if self.entries.len() >= 1024 { self.entries.pop_first(); }
        self.next = self.next.wrapping_add(1).max(1);
        self.entries.insert(self.next, stack);
        self.next
    }
    fn restore(&self, text: &str) -> Result<Option<Stack>, String> {
        if text.len() > 16384 { return Err("ItemMeta: больше 16 КиБ".into()); }
        let value: Value = if text.is_empty() { json!({}) } else { serde_json::from_str(text).map_err(|_| "ItemMeta: неверный JSON")? };
        let Some(token) = value.get("_native") else { return Ok(None); };
        let token = token.as_u64().and_then(|value| u32::try_from(value).ok()).ok_or("ItemMeta: неверный токен")?;
        let id = self.entries.get(&token).ok_or("ItemMeta: токен довесков устарел или не принадлежит плагину")?;
        Ok(Some(*id))
    }
}

fn valid_pdc(value: &Value) -> bool {
    let Some(entries) = value.as_object().filter(|map| map.len() <= 256) else { return false; };
    entries.iter().all(|(key, entry)| {
        let Some((namespace, key)) = key.split_once(':') else { return false; };
        let valid = |s: &str, slash: bool| !s.is_empty() && s.len() <= 256 && s.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"._-".contains(&c) || slash && c == b'/');
        let Some(entry) = entry.as_object().filter(|map| map.len() == 2) else { return false; };
        let Some(kind) = entry.get("type").and_then(Value::as_str) else { return false; };
        let Some(value) = entry.get("value") else { return false; };
        fn scalar(kind: &str, value: &Value) -> bool {
            match kind {
                "byte" => value.as_i64().is_some_and(|v| (-128..=127).contains(&v)),
                "int" => value.as_i64().is_some_and(|v| i32::try_from(v).is_ok()),
                "long" => value.as_i64().is_some(), "double" => value.as_f64().is_some_and(f64::is_finite), "string" => value.is_string(), _ => false,
            }
        }
        valid(namespace, false) && valid(key, true) && if let Some(element) = kind.strip_prefix("list:") { value.as_array().is_some_and(|list| list.len() <= 1024 && list.iter().all(|value| scalar(element, value))) } else { scalar(kind, value) }
    })
}

fn apply_metadata(data: &mut crate::stackdata::Data, text: &str) -> Result<(), String> {
    if text.len() > 16384 { return Err("ItemMeta: больше 16 КиБ".into()); }
    let value: Value = if text.is_empty() { json!({}) } else { serde_json::from_str(text).map_err(|_| "ItemMeta: неверный JSON")? };
    let mut value = value;
    let map = value.as_object_mut().ok_or("ItemMeta: нужна карта")?;
    map.remove("_native");
    map.remove("_opaque");
    for (key, value) in map.iter() {
        match key.as_str() {
            "name" if super::presentation::component(&value.to_string()).is_some() => (),
            "lore" if value.as_array().is_some_and(|lines| lines.len() <= 256 && lines.iter().all(|line| super::presentation::component(&line.to_string()).is_some())) => (),
            "pdc" if valid_pdc(value) => (),
            "damage" if value.as_i64().is_some_and(|v| (0..=i32::MAX as i64).contains(&v)) => (),
            "maxDamage" if value.as_i64().is_some_and(|v| (1..=i32::MAX as i64).contains(&v)) => (),
            "flags" if value.as_array().is_some_and(|flags| flags.len() <= 32 && flags.iter().all(|flag| flag.as_str().is_some_and(|s| ["HIDE_ENCHANTS","HIDE_ATTRIBUTES","HIDE_UNBREAKABLE","HIDE_DESTROYS","HIDE_PLACED_ON","HIDE_ADDITIONAL_TOOLTIP","HIDE_DYE","HIDE_ARMOR_TRIM","HIDE_STORED_ENCHANTS"].contains(&s)))) => (),
            "enchants" | "stored" if value.as_object().is_some_and(|enchants| enchants.len() <= 64 && enchants.iter().all(|(name, level)| crate::stackdata::enchantment_number(name).is_some() && level.as_i64().is_some_and(|v| (1..=255).contains(&v)))) => (),
            _ => return Err(format!("ItemMeta: недопустимое поле {key}")),
        }
    }
    data.damage = map.get("damage").and_then(Value::as_i64).unwrap_or(0) as i32;
    for field in ["enchants", "stored"] {
        let enchants = if field == "enchants" { &mut data.enchants } else { &mut data.stored };
        enchants.clear();
        if let Some(map) = map.get(field).and_then(Value::as_object) { for (name, level) in map { enchants.push((crate::stackdata::enchantment_number(name).expect("проверено"), level.as_i64().expect("проверено") as i32)); } }
        enchants.sort_unstable();
    }
    data.name = map.get("name").map(crate::scoreboard::plain_text);
    data.plugin_meta = (!map.is_empty()).then(|| value.to_string());
    Ok(())
}

fn metadata(stack: Stack, token: Option<u32>) -> String {
    let data = stack.extras();
    let mut map: Map<String,Value> = data.plugin_meta.as_ref().and_then(|s| serde_json::from_str::<Value>(s).ok()).and_then(|v| v.as_object().cloned()).unwrap_or_default();
    if data.damage != 0 { map.insert("damage".into(), json!(data.damage)); } else { map.remove("damage"); }
    for (key, enchants) in [("enchants", data.enchants), ("stored", data.stored)] {
        map.remove(key);
        if !enchants.is_empty() { let values: Map<String,Value> = enchants.iter().filter_map(|(id, level)| crate::stackdata::ENCHANTMENTS.get(*id as usize).map(|(name, _)| (format!("minecraft:{name}"), json!(*level)))).collect(); map.insert(key.into(), Value::Object(values)); }
    }
    if let Some(name) = data.name {
        if !map.get("name").is_some_and(|value| crate::scoreboard::plain_text(value) == name) { map.insert("name".into(), json!({"text":name})); }
    } else { map.remove("name"); }
    if let Some(token) = token {
        let mut unknown = stack.extras();unknown.damage=0;unknown.enchants.clear();unknown.stored.clear();unknown.name=None;unknown.plugin_meta=None;
        if !unknown.is_default() || stack.tnt.is_some() {map.insert("_opaque".into(),json!(token));}
        map.insert("_native".into(),json!(token));
    }
    if map.is_empty() { String::new() } else { Value::Object(map).to_string() }
}

#[derive(Default)]
pub struct Chests {
    next: u32,
    entries: std::collections::BTreeMap<u32, std::sync::Arc<std::sync::Mutex<crate::players::PluginChest>>>,
    pub viewers: std::collections::BTreeMap<[u8; 16], u32>,
}
impl Drop for Chests {
    fn drop(&mut self) { for chest in self.entries.values() { chest.lock().expect("сундук").active = false; } }
}
impl PluginCtx {
    fn inventory_stack(&mut self, item: Option<inventory::Item>) -> Result<Option<Stack>, String> {
        let stack = match item {
            Some(item) => {
                let number = crate::blocks::item_named(&item.kind).ok_or("неизвестный предмет")?;
                if item.kind == "air" || item.count == 0 || item.count > crate::blocks::stack_size(number) as u32 { return Err("неверное количество предметов".into()); }
                let original = self.native_items.restore(&item.metadata)?;
                let mut data = original.map(|stack| stack.extras()).unwrap_or_default();
                apply_metadata(&mut data, &item.metadata)?;
                let stack = Stack::new(number, item.count as i32).with_extras(data);
                Some(Stack { tnt: original.filter(|old| old.item == number).and_then(|old| old.tnt), ..stack })
            }
            None => None,
        };
        Ok(stack)
    }
    fn inventory_items(&mut self, slots: &[Option<Stack>]) -> Vec<Option<inventory::Item>> {
        slots.iter().map(|slot| slot.and_then(|stack| crate::blocks::item_name(stack.item).map(|name| {
            let token = self.native_items.remember(stack);
            inventory::Item { kind: name.into(), count: stack.count as u32, metadata: metadata(stack, Some(token)) }
        }))).collect()
    }
}

impl inventory::Host for PluginCtx {
    fn chest_release(&mut self, handle: u32) -> Result<(), String> {
        if self.chests.viewers.values().any(|current| *current == handle) { return Err("Сундук ещё открыт".into()); }
        let chest = self.chests.entries.remove(&handle).ok_or("Чужой или закрытый сундук")?;
        chest.lock().expect("сундук").active = false;
        Ok(())
    }
    fn open_view(&mut self, text: String) -> Result<inventory::View, String> {
        let uuid = parse_uuid(&text).ok_or("Неверный UUID")?;
        let (kind, slots) = { let players = self.shared.players.lock().expect("игроки"); let info = players.ui.info.get(&uuid).ok_or("Игрок Java отсутствует")?; (info.open_kind.clone(), info.open_top.clone()) };
        Ok(inventory::View { kind, items: self.inventory_items(&slots) })
    }
    fn cursor(&mut self, text: String) -> Result<Option<inventory::Item>, String> {
        let uuid = parse_uuid(&text).ok_or("Неверный UUID")?;
        let stack = self.shared.players.lock().expect("игроки").ui.info.get(&uuid).ok_or("Игрок Java отсутствует")?.cursor;
        Ok(self.inventory_items(&[stack]).pop().flatten())
    }
    fn chest_create(&mut self, size: u32, title: String) -> Result<u32, String> {
        if !(9..=54).contains(&size) || !size.is_multiple_of(9) { return Err("Сундук: 9..54 ячейки, кратно 9".into()); }
        let title = super::presentation::component(&title).ok_or("Недопустимый заголовок сундука")?.to_string();
        if self.chests.entries.len() >= 512 { return Err("Лимит пользовательских сундуков: 512 на компонент".into()); }
        self.chests.next = self.chests.next.checked_add(1).ok_or("Исчерпаны номера сундуков")?;
        let handle = self.chests.next;
        self.chests.entries.insert(handle, std::sync::Arc::new(std::sync::Mutex::new(crate::players::PluginChest {
            owner: self.name.clone(), handle, title, slots: vec![None; size as usize], active: true,
        })));
        Ok(handle)
    }
    fn chest_set(&mut self, handle: u32, slot: u32, item: Option<inventory::Item>) -> Result<(), String> {
        let stack = self.inventory_stack(item)?;
        let chest = self.chests.entries.get(&handle).ok_or("Чужой или закрытый сундук")?;
        let mut chest = chest.lock().expect("сундук");
        *chest.slots.get_mut(slot as usize).ok_or("Ячейка вне сундука")? = stack;
        Ok(())
    }
    fn chest_snapshot(&mut self, handle: u32) -> Result<Vec<Option<inventory::Item>>, String> {
        let slots = self.chests.entries.get(&handle).ok_or("Чужой или закрытый сундук")?.lock().expect("сундук").slots.clone();
        Ok(self.inventory_items(&slots))
    }
    fn chest_open(&mut self, text: String, handle: u32) -> Result<(), String> {
        let uuid = parse_uuid(&text).ok_or("Неверный UUID")?;
        let chest = self.chests.entries.get(&handle).ok_or("Чужой или закрытый сундук")?.clone();
        let mut players = self.shared.players.lock().expect("игроки");
        if !players.ui.info.contains_key(&uuid) { return Err("Игрок Java отсутствует".into()); }
        self.chests.viewers.insert(uuid, handle);
        players.order(&uuid, Order::PluginOpen { chest });
        Ok(())
    }
    fn chest_close(&mut self, text: String) -> Result<(), String> {
        let uuid = parse_uuid(&text).ok_or("Неверный UUID")?;
        self.shared.players.lock().expect("игроки").order(&uuid, Order::PluginClose { owner: self.name.clone() });
        Ok(())
    }
    fn chest_current(&mut self, text: String) -> Option<u32> {
        parse_uuid(&text).and_then(|uuid| self.chests.viewers.get(&uuid).copied())
    }

    fn snapshot(&mut self, text: String) -> Result<Vec<Option<inventory::Item>>, String> {
        let uuid = parse_uuid(&text).ok_or("неверный UUID")?;
        let slots = self.shared.players.lock().expect("игроки").ui.info.get(&uuid).ok_or("нет снимка игрока")?.inventory.clone();
        Ok(slots.iter().map(|slot| slot.and_then(|stack| crate::blocks::item_name(stack.item).map(|name| {
            let token = self.native_items.remember(stack);
            inventory::Item { kind: name.into(), count: stack.count as u32, metadata: metadata(stack, Some(token)) }
        }))).collect())
    }
    fn selected(&mut self, text: String) -> Result<u32, String> {
        let uuid = parse_uuid(&text).ok_or("неверный UUID")?;
        self.shared.players.lock().expect("игроки").ui.info.get(&uuid).map(|info| info.selected).ok_or_else(|| "нет снимка игрока".into())
    }
    fn set(&mut self, text: String, slot: u32, item: Option<inventory::Item>) -> Result<(), String> {
        if !(5..=45).contains(&slot) { return Err("ячейка: от 5 до 45".into()); }
        let uuid = parse_uuid(&text).ok_or("неверный UUID")?;
        let stack = self.inventory_stack(item)?;
        let mut players = self.shared.players.lock().expect("игроки");
        let info = players.ui.info.get_mut(&uuid).ok_or("нет снимка игрока")?;
        if info.inventory.len() != 46 { info.inventory.resize(46,None); }
        info.inventory[slot as usize] = stack;
        players.order(&uuid, Order::SetSlot { slot: slot as i32, stack });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn item_metadata_is_checked_and_survives_the_file() {
        let mut data = crate::stackdata::Data::default();
        apply_metadata(&mut data, r#"{"damage":3,"name":{"text":"Blade","color":"red"},"lore":[{"text":"lore"}],"enchants":{"minecraft:sharpness":2},"pdc":{"test:ability":{"type":"string","value":"blink"}}}"#).unwrap();
        let loaded = crate::stackdata::from_bytes(&crate::stackdata::to_bytes(&data)).unwrap();
        assert_eq!(loaded,data);
        assert_eq!(crate::stackdata::encode_components(&loaded).0,4);
        let stack = Stack::new(crate::blocks::item_named("diamond_sword").unwrap(),1).with_extras(data);
        assert!(metadata(stack,None).contains("blink"));
        for invalid in [r#"{"pdc":{"bad":{"type":"string","value":"x"}}}"#, r#"{"damage":-1}"#, r#"{"enchants":{"minecraft:sharpness":999999}}"#, r#"{"name":{"nbt":"secret","storage":"auth"}}"#, r#"{"pdc":{"test:key":{"type":"int","value":"wrong"}}}"#] {
            assert!(apply_metadata(&mut crate::stackdata::Data::default(),invalid).is_err(),"{invalid}");
        }
        assert!(apply_metadata(&mut crate::stackdata::Data::default(), &"x".repeat(16385)).is_err());
    }
}
