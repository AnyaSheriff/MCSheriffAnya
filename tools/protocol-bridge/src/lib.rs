//! Собственная основа перевода Java-протокола. Готовых межверсионных маршрутов пока нет.
//! Номера версий: minecraft-data/data/pc/common/protocolVersions.json.
use std::collections::{BTreeMap, BTreeSet};

mod generated_ids;
pub mod ids;
pub mod entities;
mod entity_layouts;
pub mod items;
mod item_components;
mod particles;
mod recipes;
mod actions;
pub mod entity_sync;
pub mod legacy_items;
pub mod legacy_inventory;
pub mod legacy_control;
pub mod legacy_actions;
pub mod legacy_play;
pub mod legacy_attributes;
pub mod legacy_recipes;
pub mod legacy_advancements;
pub mod legacy_text;
pub mod legacy_biomes;
pub mod legacy_chunk;
mod legacy_block_ids;
pub mod legacy_blocks;
mod legacy_item_ids;
pub mod login;
pub mod light;
pub mod chunk_sections;
pub mod chunk;
pub mod block_entities;
mod block_entity_ids;
pub mod movement;
pub mod teleport;
pub mod registry;
pub mod wire;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JavaVersion { V1_12_2, V26_1_2, V26_2, V26_3 }

impl JavaVersion {
    pub const fn protocol(self) -> i32 {
        match self { Self::V1_12_2 => 340, Self::V26_1_2 => 775, Self::V26_2 => 776, Self::V26_3 => 777 }
    }
    pub const fn name(self) -> &'static str {
        match self { Self::V1_12_2 => "1.12.2", Self::V26_1_2 => "26.1.2", Self::V26_2 => "26.2", Self::V26_3 => "26.3" }
    }
    pub fn from_name(name: &str) -> Option<Self> {
        match name { "1.12.2" => Some(Self::V1_12_2), "26.1.2" => Some(Self::V26_1_2), "26.2" => Some(Self::V26_2), "26.3" => Some(Self::V26_3), _ => None }
    }
    pub const fn from_protocol(protocol: i32) -> Option<Self> {
        match protocol { 340 => Some(Self::V1_12_2), 775 => Some(Self::V26_1_2), 776 => Some(Self::V26_2), 777 => Some(Self::V26_3), _ => None }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Negotiated {
    pub client: JavaVersion,
    pub native: JavaVersion,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdmissionError { UnknownProtocol(i32), AdapterUnavailable { client: JavaVersion, native: JavaVersion } }

impl Negotiated {
    /// Один известный номер Handshake ещё не означает поддержку версии.
    pub fn native_only(client_protocol: i32, native: JavaVersion) -> Result<Self, AdmissionError> {
        let client = JavaVersion::from_protocol(client_protocol).ok_or(AdmissionError::UnknownProtocol(client_protocol))?;
        if client != native { return Err(AdmissionError::AdapterUnavailable { client, native }); }
        Ok(Self { client, native })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MappingError {
    DuplicateId(u32), DuplicateName(String), UnknownId(u32), MissingTarget(String),
    ChangedLayout(String), InvalidSchema(String),
}

/// Реестр строится из таблицы одной версии/одного соединения. Пространство имён
/// входит в строку целиком; числовое совпадение ID между версиями ничего не значит.
#[derive(Clone, Debug)]
pub struct Registry {
    by_id: BTreeMap<u32, String>,
    by_name: BTreeMap<String, u32>,
}

impl Registry {
    pub fn new(entries: impl IntoIterator<Item = (u32, String)>) -> Result<Self, MappingError> {
        let mut this = Self { by_id: BTreeMap::new(), by_name: BTreeMap::new() };
        for (id, name) in entries {
            if this.by_id.contains_key(&id) { return Err(MappingError::DuplicateId(id)); }
            if this.by_name.contains_key(&name) { return Err(MappingError::DuplicateName(name)); }
            this.by_id.insert(id, name.clone());
            this.by_name.insert(name, id);
        }
        Ok(this)
    }
    pub fn translate(&self, id: u32, target: &Self) -> Result<u32, MappingError> {
        let name = self.by_id.get(&id).ok_or(MappingError::UnknownId(id))?;
        target.by_name.get(name).copied().ok_or_else(|| MappingError::MissingTarget(name.clone()))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum State { Login, Configuration, Play }
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction { Serverbound, Clientbound }

/// Полная, разрешённая схема полезной нагрузки после разрешения ссылок на типы.
/// Сравнивать только имя внешнего типа (например, packet_chunk) НЕЛЬЗЯ.
/// Адаптер отдельно обязан переводить вложенные ID реестров и компоненты.
#[derive(Clone, Debug)]
pub struct PacketDefinition {
    pub id: u32,
    pub name: String,
    pub resolved_layout: String,
}

#[derive(Clone, Debug)]
pub struct PacketTable {
    pub state: State,
    pub direction: Direction,
    registry: Registry,
    layouts: BTreeMap<u32, String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PacketError { WrongContext, Mapping(MappingError) }

impl From<MappingError> for PacketError {
    fn from(value: MappingError) -> Self { Self::Mapping(value) }
}

impl PacketTable {
    pub fn new(state: State, direction: Direction, entries: Vec<PacketDefinition>) -> Result<Self, MappingError> {
        let registry = Registry::new(entries.iter().map(|e| (e.id, e.name.clone())))?;
        let mut layouts = BTreeMap::new();
        for entry in entries {
            if entry.resolved_layout.trim().is_empty() { return Err(MappingError::InvalidSchema(entry.name)); }
            layouts.insert(entry.id, entry.resolved_layout);
        }
        Ok(Self { state, direction, registry, layouts })
    }
    /// Только номер пакета: это НЕ разрешение слепо пересылать его payload.
    /// Работает после расшифровки/распаковки; новый ID кодируется перед сжатием.
    pub fn translate_id(&self, id: u32, target: &Self) -> Result<u32, PacketError> {
        if self.state != target.state || self.direction != target.direction { return Err(PacketError::WrongContext); }
        let mapped = self.registry.translate(id, &target.registry)?;
        if self.layouts.get(&id) != target.layouts.get(&mapped) {
            return Err(MappingError::ChangedLayout(self.registry.by_id[&id].clone()).into());
        }
        Ok(mapped)
    }
    /// Полнота таблицы не означает полноту реализации адаптера.
    pub fn names_missing_from(&self, target: &Self) -> BTreeSet<&str> {
        self.registry.by_name.keys().filter(|name| !target.registry.by_name.contains_key(*name)).map(String::as_str).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn registry(entries: &[(u32, &str)]) -> Registry {
        Registry::new(entries.iter().map(|(id, name)| (*id, name.to_string()))).unwrap()
    }
    fn packet(id: u32, name: &str, layout: &str) -> PacketDefinition {
        PacketDefinition { id, name: name.into(), resolved_layout: layout.into() }
    }
    fn table(entries: Vec<PacketDefinition>) -> PacketTable {
        PacketTable::new(State::Play, Direction::Clientbound, entries).unwrap()
    }
    #[test]
    fn admission_never_enables_a_version_just_because_its_number_is_known() {
        let native = JavaVersion::V26_1_2;
        assert_eq!(Negotiated::native_only(775, native).unwrap().client, native);
        for client in [JavaVersion::V1_12_2, JavaVersion::V26_2, JavaVersion::V26_3] {
            assert_eq!(Negotiated::native_only(client.protocol(), native), Err(AdmissionError::AdapterUnavailable { client, native }));
            assert_eq!(JavaVersion::from_protocol(client.protocol()), Some(client));
        }
        for protocol in [-1, 0, 774, 778, i32::MAX] {
            assert_eq!(Negotiated::native_only(protocol, native), Err(AdmissionError::UnknownProtocol(protocol)));
        }
    }
    #[test]
    fn recognized_legacy_version_cannot_use_modern_registry_or_payload_tables() {
        let old = JavaVersion::V1_12_2;
        let modern = JavaVersion::V26_1_2;
        for registry in generated_ids::ALL {
            for (from, to) in [(old, modern), (modern, old), (old, old)] {
                assert_eq!(ids::translate(*registry, from, to, 0), Err(ids::IdTranslationError::UnsupportedVersion(old)));
                assert!(wire::translate(State::Play, Direction::Clientbound, from, to, &[0]).is_err());
            }
        }
        assert_eq!(light::Light::decode(old, &[]), Err(light::Error::UnsupportedVersion));
        assert_eq!(login::Success::decode(old, &[]), Err(login::Error::UnsupportedVersion));
        assert_eq!(teleport::Confirmation::decode(old, &[0]), Err(teleport::Error::UnsupportedVersion));
        assert!(items::decode(old, &[0], false, items::Context::default()).is_err());
    }
    #[test]
    fn registries_translate_both_directions_by_name_not_numeric_coincidence() {
        let old = registry(&[(0,"minecraft:air"),(1,"minecraft:stone"),(500,"example:stone")]);
        let new = registry(&[(0,"minecraft:stone"),(9,"minecraft:air"),(1,"minecraft:new")]);
        assert_eq!(old.translate(1,&new), Ok(0));
        assert_eq!(new.translate(0,&old), Ok(1));
        assert_eq!(old.translate(0,&new), Ok(9));
        assert_eq!(old.translate(500,&new), Err(MappingError::MissingTarget("example:stone".into())));
        assert_eq!(new.translate(1,&old), Err(MappingError::MissingTarget("minecraft:new".into())));
        assert_eq!(old.translate(3,&new), Err(MappingError::UnknownId(3)));
    }
    #[test]
    fn ambiguous_tables_are_rejected() {
        assert!(matches!(Registry::new([(0,"a".into()),(0,"b".into())]),Err(MappingError::DuplicateId(0))));
        assert!(matches!(Registry::new([(0,"a".into()),(1,"a".into())]),Err(MappingError::DuplicateName(_))));
        assert!(matches!(PacketTable::new(State::Login,Direction::Serverbound,vec![packet(0,"a"," ")]),Err(MappingError::InvalidSchema(_))));
    }
    #[test]
    fn packet_context_schema_and_missing_packets_fail_closed() {
        let old = table(vec![packet(1,"ping","value:i64"),packet(2,"changed","x:i32")]);
        let mut new = table(vec![packet(8,"ping","value:i64"),packet(2,"changed","x:i64"),packet(3,"added","flag:bool")]);
        assert_eq!(old.translate_id(1,&new),Ok(8));
        assert_eq!(new.translate_id(8,&old),Ok(1));
        assert!(matches!(old.translate_id(2,&new),Err(PacketError::Mapping(MappingError::ChangedLayout(_)))));
        assert!(matches!(new.translate_id(3,&old),Err(PacketError::Mapping(MappingError::MissingTarget(_)))));
        assert_eq!(new.names_missing_from(&old),BTreeSet::from(["added"]));
        new.state=State::Configuration;
        assert_eq!(old.translate_id(1,&new),Err(PacketError::WrongContext));
        new.state=State::Play;new.direction=Direction::Serverbound;
        assert_eq!(old.translate_id(1,&new),Err(PacketError::WrongContext));
    }
}

#[cfg(test)]
mod item_observation_tests;

mod trades;
mod nested_items;
