//! Атрибуты340: собственные наблюдения legacy-attributes.json.
//! Общие серверные правила не меняются. Неизвестная клиентская механика
//! не подменяется похожим атрибутом старой версии.
use std::collections::{BTreeMap, BTreeSet};
use crate::wire::{Cursor, Error, uint};

const NAMES: &[(u32, &str)] = &[(0,"generic.armor"),(1,"generic.armorToughness"),
    (4,"generic.attackSpeed"),(17,"generic.luck"),(19,"generic.maxHealth"),(22,"generic.movementSpeed")];
const KNOWN: &[(&str, u128)] = &[
    ("minecraft:sprinting",0x662a6b8dda3e4c1c881396ea6097278d),
    ("minecraft:base_attack_speed",0xfa233e1c41804865b01bbcce9785aca3),
    ("minecraft:effect.speed",0x91aeaa56376b4498935b2f7f68070635),
    ("minecraft:armor.chestplate",0x9f3d476dc1184544836564846904b48e),
    ("minecraft:effect.health_boost",0x5d6f0ba2118646acb896c61c5cee99cc),
    ("minecraft:effect.luck",0x03c3c89d70374b42869fb146bcb64d2e),
];

#[derive(Clone, Debug, Default)]
pub struct Modifiers(BTreeMap<String, u128>);
impl Modifiers {
    fn uuid(&mut self, name: &str) -> Result<u128, Error> {
        if let Some((_, id)) = KNOWN.iter().find(|row| row.0 == name) { return Ok(*id); }
        if let Some(id) = self.0.get(name) { return Ok(*id); }
        if self.0.len() >= 4096 { return Err(Error::TooLarge); }
        // Свои имена получают уникальный стабильный ID в пределах соединения.
        // Это не хеш: коллизий между такими именами и наблюдёнными UUID нет.
        let id = 0x4d435341000040008000000000000000u128 | (self.0.len() + 1) as u128;
        self.0.insert(name.into(), id); Ok(id)
    }

    pub fn translate(&mut self, bytes: &[u8]) -> Result<Option<Vec<u8>>, Error> {
        if bytes.len() > 1_048_576 { return Err(Error::TooLarge); }
        let mut next = self.clone();
        let mut input = Cursor::new(bytes);
        let entity = input.uint()?; let count = input.count(1024)?;
        let mut seen = BTreeSet::new(); let mut entries = Vec::new(); let mut kept = 0i32;
        for _ in 0..count {
            let id = input.uint()?;
            if !seen.insert(id) { return Err(Error::Invalid); }
            let base = number(&mut input)?; let n = input.count(1024)?;
            let mut mods = Vec::with_capacity(n); let mut names = BTreeSet::new();
            for _ in 0..n {
                let name = input.string()?;
                if name.is_empty() || name.len() > 256 || !names.insert(name) { return Err(Error::Invalid); }
                let amount = number(&mut input)?; let operation = input.take(1)?[0];
                if operation > 2 { return Err(Error::Invalid); }
                mods.push((name, amount, operation));
            }
            if let Some((_, name)) = NAMES.iter().find(|row| row.0 == id) {
                uint(&mut entries, name.len() as u32); entries.extend(name.as_bytes()); entries.extend(base.to_be_bytes());
                uint(&mut entries, n as u32);
                for (name, amount, operation) in mods { entries.extend(next.uuid(name)?.to_be_bytes()); entries.extend(amount.to_be_bytes()); entries.push(operation); }
                kept += 1;
            } else {
                let representable = match id {
                    // У340 дальность задаётся режимом игры. Произвольное
                    // увеличение требует другого представления, пока отказ.
                    6 => base == 4.5 && (mods.is_empty() || mods == [("minecraft:creative_mode_block_range",0.5,0)]),
                    10 => base == 3.0 && (mods.is_empty() || mods == [("minecraft:creative_mode_entity_range",2.0,0)]),
                    23 => base == 0.0 && mods.is_empty(),
                    // Урон соседям вычисляет сервер; у340 нет этого атрибута.
                    30 => true,
                    _ => false,
                };
                if !representable { return Err(Error::PayloadUnavailable("legacy attribute behaviour")); }
            }
        }
        if input.remaining() != 0 { return Err(Error::Invalid); }
        *self = next;
        if kept == 0 { return Ok(None); }
        let mut out = vec![0x4e]; uint(&mut out, entity); out.extend(kept.to_be_bytes()); out.extend(entries); Ok(Some(out))
    }
}
fn number(input: &mut Cursor<'_>) -> Result<f64, Error> {
    let value = f64::from_be_bytes(input.take(8)?.try_into().map_err(|_| Error::Invalid)?);
    if value.is_finite() { Ok(value) } else { Err(Error::Invalid) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn attributes_match_official_sprint_weapon_armor_and_effect_packets() {
        let source: serde_json::Value = serde_json::from_str(include_str!("../testdata/legacy-attributes.json")).unwrap();
        let mut state = Modifiers::default();
        for row in source["observations"].as_array().unwrap() {
            let mut body = vec![]; uint(&mut body,row["entity"].as_u64().unwrap() as u32);
            let entries = row["attributes"].as_array().unwrap(); uint(&mut body,entries.len() as u32);
            for entry in entries {
                uint(&mut body,NAMES.iter().find(|row| row.1 == entry["name"].as_str().unwrap()).unwrap().0);
                body.extend(entry["base"].as_f64().unwrap().to_be_bytes());
                let mods = entry["modifiers"].as_array().unwrap(); uint(&mut body,mods.len() as u32);
                for modifier in mods {
                    let id = u128::from_str_radix(&modifier["uuid"].as_str().unwrap().replace('-',""),16).unwrap();
                    let name = KNOWN.iter().find(|row| row.1 == id).unwrap().0;
                    uint(&mut body,name.len() as u32); body.extend(name.as_bytes());
                    body.extend(modifier["amount"].as_f64().unwrap().to_be_bytes()); body.push(modifier["operation"].as_u64().unwrap() as u8);
                }
            }
            let expected: Vec<_> = row["payload"].as_str().unwrap().as_bytes().as_chunks::<2>().0.iter()
                .map(|s| u8::from_str_radix(std::str::from_utf8(s).unwrap(),16).unwrap()).collect();
            assert_eq!(state.translate(&body).unwrap().unwrap()[1..],expected);
        }
    }
    #[test]
    fn custom_modifiers_have_distinct_stable_ids_and_failures_are_atomic() {
        let mut state = Modifiers::default();
        let first = state.uuid("mcsa:first").unwrap(); let second = state.uuid("mcsa:second").unwrap();
        assert_ne!(first,second); assert_eq!(state.uuid("mcsa:first").unwrap(),first);
        assert!(KNOWN.iter().all(|row| row.1 != first && row.1 != second));
        let mut reach = vec![1,1,6]; reach.extend(4.5f64.to_be_bytes()); reach.push(0);
        assert_eq!(state.translate(&reach),Ok(None));
        reach[3..11].copy_from_slice(&8f64.to_be_bytes()); assert!(state.translate(&reach).is_err());
        let before = state.0.clone();
        let mut bad = vec![1,1,22]; bad.extend(0.1f64.to_be_bytes()); bad.push(1);
        bad.extend([1,b'a']); bad.extend(1f64.to_be_bytes()); bad.extend([0,255]);
        assert!(state.translate(&bad).is_err()); assert_eq!(state.0,before);
    }
}
