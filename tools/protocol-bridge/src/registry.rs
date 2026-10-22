//! Registry Data: Wiki Registries oldid3749652 / Packets oldid3848030.
//! ID записи — её место в массиве. Формат содержимого задаёт версия клиента;
//! этот код проверяет границы NBT, но не выдаёт старый NBT за новый формат.
use std::collections::HashSet;
use crate::{JavaVersion, ids::{self, RegistryKind}};

const MAX_BYTES: usize = 8 * 1024 * 1024;
const MAX_ENTRIES: usize = 65_536;
const MAX_NODES: usize = 262_144;
const MAX_DEPTH: usize = 64;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry { pub name: String, pub data: Option<Vec<u8>> }

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Registry {
    /// Версия СОДЕРЖИМОГО записей, а не только заголовка пакета.
    pub version: JavaVersion,
    pub name: String,
    pub entries: Vec<Entry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error { UnsupportedVersion, Invalid, Truncated, TooLarge, TooDeep, DuplicateEntry, Id(ids::IdTranslationError) }

/// Размер одного анонимного NBT-тега; End разрешён для отсутствующих данных.
pub(crate) fn nbt_length(bytes: &[u8]) -> Result<usize, Error> {
    let (&kind, rest) = bytes.split_first().ok_or(Error::Truncated)?;
    Ok(1 + nbt_payload_length(kind, rest)?)
}

pub(crate) fn nbt_payload_length(kind: u8, bytes: &[u8]) -> Result<usize, Error> {
    if bytes.len() > MAX_BYTES { return Err(Error::TooLarge); }
    let mut input = Cursor(bytes);
    if kind != 0 { let mut budget = MAX_NODES; nbt_payload(&mut input, kind, 0, &mut budget)?; }
    Ok(bytes.len() - input.0.len())
}

fn supported(version: JavaVersion) -> Result<(), Error> {
    if !matches!(version, JavaVersion::V26_1_2 | JavaVersion::V26_2 | JavaVersion::V26_3) { Err(Error::UnsupportedVersion) } else { Ok(()) }
}

fn identifier(value: &str) -> bool {
    let Some((namespace, path)) = value.split_once(':') else { return false; };
    let common = |b: u8| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-' | b'.');
    !namespace.is_empty() && !path.is_empty() && value.len() <= 32_767
        && namespace.bytes().all(common) && path.bytes().all(|b| common(b) || b == b'/')
}

impl Registry {
    /// Полезная нагрузка без packet ID. Не сортировать записи: это сменит ID!
    pub fn decode(version: JavaVersion, payload: &[u8]) -> Result<Self, Error> {
        supported(version)?;
        if payload.len() > MAX_BYTES { return Err(Error::TooLarge); }
        let mut input = Cursor(payload);
        let name = input.identifier()?.to_owned();
        let count = input.uint()? as usize;
        if count > MAX_ENTRIES { return Err(Error::TooLarge); }
        if count > input.0.len() / 5 { return Err(Error::Truncated); }
        let mut entries = Vec::with_capacity(count);
        let mut seen = HashSet::with_capacity(count);
        let mut budget = MAX_NODES;
        for _ in 0..count {
            let name = input.identifier()?.to_owned();
            if !seen.insert(name.clone()) { return Err(Error::DuplicateEntry); }
            let data = match input.take(1)?[0] {
                0 => None,
                1 => {
                    let start = input.0;
                    let root = input.take(1)?[0];
                    nbt_payload(&mut input, root, 0, &mut budget)?;
                    Some(start[..start.len() - input.0.len()].to_vec())
                }
                _ => return Err(Error::Invalid),
            };
            entries.push(Entry { name, data });
        }
        if !input.0.is_empty() { return Err(Error::Invalid); }
        Ok(Self { version, name, entries })
    }

    /// Полное тело уже для нужного клиента. Никакого перевода вложенных
    /// схем здесь нет: подготовить их обязан вызывающий builder реестров.
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        supported(self.version)?;
        if self.entries.len() > MAX_ENTRIES { return Err(Error::TooLarge); }
        // Готовое содержимое builder-а; private lookup не открывает общий wire776.
        let id = if self.version == JavaVersion::V26_2 {
            let info = crate::packet_layouts::packet_info(RegistryKind::ConfigurationClientbound,
                JavaVersion::V26_1_2, self.version, 7).ok_or(Error::Invalid)?;
            if info.name != "registry_data" || !info.schema_equal { return Err(Error::Invalid); }
            info.target_id.ok_or(Error::Invalid)?
        } else {
            ids::translate(RegistryKind::ConfigurationClientbound, JavaVersion::V26_1_2, self.version, 7).map_err(Error::Id)?
        };
        let mut out = Vec::new();
        uint(&mut out, id);
        string(&mut out, &self.name)?;
        uint(&mut out, self.entries.len() as u32);
        let mut seen = HashSet::with_capacity(self.entries.len());
        let mut budget = MAX_NODES;
        for entry in &self.entries {
            if !seen.insert(&entry.name) { return Err(Error::DuplicateEntry); }
            string(&mut out, &entry.name)?;
            out.push(u8::from(entry.data.is_some()));
            if let Some(data) = &entry.data {
                if data.len() > MAX_BYTES || out.len() + data.len() > MAX_BYTES { return Err(Error::TooLarge); }
                let mut input = Cursor(data);
                let root = input.take(1)?[0];
                nbt_payload(&mut input, root, 0, &mut budget)?;
                if !input.0.is_empty() { return Err(Error::Invalid); }
                out.extend(data);
            }
        }
        if out.len() > MAX_BYTES { return Err(Error::TooLarge); }
        Ok(out)
    }
}

struct Cursor<'a>(&'a [u8]);
impl<'a> Cursor<'a> {
    fn take(&mut self, size: usize) -> Result<&'a [u8], Error> {
        let (part, rest) = self.0.split_at_checked(size).ok_or(Error::Truncated)?;
        self.0 = rest;
        Ok(part)
    }
    fn uint(&mut self) -> Result<u32, Error> {
        let mut value = 0u32;
        for shift in (0..35).step_by(7) {
            let byte = self.take(1)?[0];
            if shift == 28 && byte & 0xf8 != 0 { return Err(Error::Invalid); }
            value |= u32::from(byte & 127) << shift;
            if byte < 128 { return Ok(value); }
        }
        Err(Error::Invalid)
    }
    fn identifier(&mut self) -> Result<&'a str, Error> {
        let size = self.uint()? as usize;
        if size > 32_767 { return Err(Error::TooLarge); }
        let value = std::str::from_utf8(self.take(size)?).map_err(|_| Error::Invalid)?;
        if !identifier(value) { return Err(Error::Invalid); }
        Ok(value)
    }
    fn nbt_string(&mut self) -> Result<(), Error> {
        let length = u16::from_be_bytes(self.take(2)?.try_into().map_err(|_| Error::Invalid)?) as usize;
        self.take(length)?;
        Ok(())
    }
    fn nbt_count(&mut self) -> Result<usize, Error> {
        let n = i32::from_be_bytes(self.take(4)?.try_into().map_err(|_| Error::Invalid)?);
        usize::try_from(n).map_err(|_| Error::Invalid)
    }
}

/// Проверка без распаковки NBT в дерево и без выделения памяти по размеру
/// заявленного массива. Строки NBT — modified UTF-8, здесь считаем их байты.
fn nbt_payload(input: &mut Cursor<'_>, kind: u8, depth: usize, budget: &mut usize) -> Result<(), Error> {
    if depth > MAX_DEPTH { return Err(Error::TooDeep); }
    *budget = budget.checked_sub(1).ok_or(Error::TooLarge)?;
    match kind {
        1 => { input.take(1)?; }
        2 => { input.take(2)?; }
        3 | 5 => { input.take(4)?; }
        4 | 6 => { input.take(8)?; }
        7 | 11 | 12 => {
            let count = input.nbt_count()?;
            let width = match kind { 7 => 1, 11 => 4, _ => 8 };
            let size = count.checked_mul(width).ok_or(Error::TooLarge)?;
            if size > MAX_BYTES { return Err(Error::TooLarge); }
            input.take(size)?;
        }
        8 => input.nbt_string()?,
        9 => {
            let element = input.take(1)?[0];
            let count = input.nbt_count()?;
            if element > 12 || (element == 0 && count != 0) { return Err(Error::Invalid); }
            if count > *budget { return Err(Error::TooLarge); }
            for _ in 0..count { nbt_payload(input, element, depth + 1, budget)?; }
        }
        10 => loop {
            let element = input.take(1)?[0];
            if element == 0 { break; }
            if element > 12 { return Err(Error::Invalid); }
            input.nbt_string()?;
            nbt_payload(input, element, depth + 1, budget)?;
        }
        _ => return Err(Error::Invalid),
    }
    Ok(())
}

fn uint(out: &mut Vec<u8>, mut value: u32) {
    while value >= 128 { out.push((value as u8 & 127) | 128); value >>= 7; }
    out.push(value as u8);
}
fn string(out: &mut Vec<u8>, value: &str) -> Result<(), Error> {
    if !identifier(value) { return Err(Error::Invalid); }
    if out.len() + value.len() + 5 > MAX_BYTES { return Err(Error::TooLarge); }
    uint(out, value.len() as u32); out.extend(value.as_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> Registry {
        Registry { version: JavaVersion::V26_3, name: "minecraft:worldgen/biome".into(), entries: vec![
            Entry { name: "minecraft:plains".into(), data: Some(vec![10, 0]) },
            Entry { name: "mcsa:autumn".into(), data: Some(vec![10, 8, 0, 1, b'x', 0, 1, b'y', 0]) },
            Entry { name: "minecraft:forest".into(), data: None },
        ] }
    }
    #[test]
    fn round_trip_preserves_custom_entries_order_and_optional_nbt() {
        for version in [JavaVersion::V26_1_2, JavaVersion::V26_2, JavaVersion::V26_3] {
            let mut registry = sample(); registry.version = version;
            let bytes = registry.encode().unwrap();
            assert_eq!(bytes[0], 7);
            assert_eq!(Registry::decode(version, &bytes[1..]).unwrap(), registry);
            for n in 1..bytes.len() { assert!(Registry::decode(version, &bytes[1..n]).is_err()); }
            let mut trailing = bytes[1..].to_vec(); trailing.push(0);
            assert_eq!(Registry::decode(version, &trailing), Err(Error::Invalid));
        }
        let mut registry = sample(); registry.version = JavaVersion::V1_12_2;
        assert_eq!(registry.encode(), Err(Error::UnsupportedVersion));
        assert!(ids::translate(RegistryKind::ConfigurationClientbound,
            JavaVersion::V26_1_2, JavaVersion::V26_2, 7).is_err());
    }
    #[test]
    fn root_list_is_valid_for_block_transformers_but_must_be_bounded() {
        let mut registry = sample();
        registry.name = "minecraft:block_transformer".into();
        registry.entries = vec![Entry { name: "minecraft:shovel".into(), data: Some(vec![9, 10, 0, 0, 0, 1, 0]) }];
        let body = registry.encode().unwrap();
        assert_eq!(Registry::decode(registry.version, &body[1..]).unwrap(), registry);
        registry.entries[0].data = Some(vec![9, 10, 0, 0, 0, 2, 0]);
        assert_eq!(registry.encode(), Err(Error::Truncated));
        registry.entries[0].data = Some(vec![0]);
        assert_eq!(registry.encode(), Err(Error::Invalid));
    }
    #[test]
    fn duplicate_names_invalid_identifiers_and_nbt_are_rejected() {
        let mut registry = sample(); registry.entries.push(registry.entries[0].clone());
        assert_eq!(registry.encode(), Err(Error::DuplicateEntry));
        for name in ["plains", "minecraft:", ":plains", "Minecraft:plains", "mc:a:b", "mc:hello world"] {
            let mut registry = sample(); registry.entries[0].name = name.into();
            assert_eq!(registry.encode(), Err(Error::Invalid));
        }
        for data in [vec![0], vec![10, 99, 0], vec![10, 9, 0, 0, 0, 0, 0, 0, 1, 0], vec![10, 7, 0, 0, 255, 255, 255, 255, 0]] {
            let mut registry = sample(); registry.entries[0].data = Some(data);
            assert!(registry.encode().is_err());
        }
        let mut data = vec![10];
        for _ in 0..66 { data.extend([10, 0, 0]); }
        data.extend([0; 67]);
        let mut registry = sample(); registry.entries[0].data = Some(data);
        assert_eq!(registry.encode(), Err(Error::TooDeep));
    }
    #[test]
    fn declared_counts_do_not_allocate_or_loop_without_bounds() {
        let mut huge = Vec::new(); string(&mut huge, "minecraft:test").unwrap(); uint(&mut huge, 65_537);
        assert_eq!(Registry::decode(JavaVersion::V26_3, &huge), Err(Error::TooLarge));
        let mut registry = sample();
        registry.entries[0].data = Some(vec![10, 9, 0, 0, 10, 0x7f, 0xff, 0xff, 0xff, 0]);
        assert_eq!(registry.encode(), Err(Error::TooLarge));
        registry.entries[0].data = Some(vec![10, 12, 0, 0, 0x7f, 0xff, 0xff, 0xff, 0]);
        assert_eq!(registry.encode(), Err(Error::TooLarge));
    }
}
