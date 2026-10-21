//! Явный серверный каталог: прежние ID775 и дописываемые внутренние объекты.
//!
//! Клиентская версия не выбирает игровые правила. Каталог принадлежит World/Shared,
//! проходит через игровые обработчики, сеть и проверки сохранений. Preview доступен
//! только отдельной пробе с миром в памяти; production admission остаётся775.
//! Источник первого объекта — pinned JSON white-concrete-slab.json и обычные
//! наблюдения установки/добычи. Полная механика26.3 ещё не реализована.
#![allow(dead_code)] // Основа следующего среза; production-путь остаётся775.

use mcsa_protocol::{JavaVersion, ids::{self, RegistryKind}};

mod runtime;
mod stack_validation;
pub use mcsa_protocol::content::{ConcreteSlab, CONCRETE_SLABS};
pub use stack_validation::{StackError, StackLocation, StackProblem};

/// Это серверный профиль, а не согласованная версия сетевого соединения.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Profile {
    Java26_1_2,
    /// База775 +16бетонных плит, НЕ полный каталог26.3.
    Preview26_3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind { Item, Block, State }

impl Kind {
    fn registry(self) -> RegistryKind {
        match self { Self::Item => RegistryKind::Item, Self::Block => RegistryKind::Block, Self::State => RegistryKind::BlockState }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    UnknownInternal { kind: Kind, id: i32 },
    Disabled { profile: Profile, name: &'static str },
    Unrepresentable { client: JavaVersion, name: &'static str },
    Wire(ids::IdTranslationError),
}

/// Записи этой таблицы после появления сохранений нельзя перенумеровывать.
/// Новые объекты дописываются в конец соответствующего пространства.
/// Это НЕ номера из какого-либо Minecraft protocol.
pub const WHITE_SLAB_ITEM: i32 = 1506;
pub const WHITE_SLAB_BLOCK: i32 = 1168;
pub const WHITE_SLAB_FIRST_STATE: i32 = 29873;
pub const WHITE_SLAB_LAST_STATE: i32 = 29878;
pub const WHITE_SLAB_DEFAULT_STATE: i32 = 29876;
pub const WHITE_SLAB_NAME: &str = "white_concrete_slab";

const WHITE_SLAB_WIRE_ITEM: u32 = 721;
const WHITE_SLAB_WIRE_BLOCK: u32 = 800;
const WHITE_SLAB_WIRE_FIRST_STATE: u32 = 18307;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SlabHalf { Top, Bottom, Double }

impl SlabHalf {
    fn offset(self) -> i32 {
        match self { Self::Top => 0, Self::Bottom => 2, Self::Double => 4 }
    }
}

/// Маленькое значение принадлежит серверу/миру и передаётся явно.
/// Здесь нет изменяемого глобального выбранного профиля или копии движка мира.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Catalog { profile: Profile }

impl Catalog {
    pub const fn new(profile: Profile) -> Self { Self { profile } }
    pub const fn profile(self) -> Profile { self.profile }

    pub fn content_codec(self) -> mcsa_protocol::content::ContentCodec {
        match self.profile {
            Profile::Java26_1_2 => Default::default(),
            Profile::Preview26_3 => mcsa_protocol::content::ContentCodec::white_slab_preview(),
        }
    }

    /// Отпечаток внутренней идентичности блоков/предметов, не полного gameplay.
    /// Для775 возвращается ТОЧНО прежний fingerprint, без extension в базе.
    /// Наличие этого значения не разрешает запись/перемаркировку старого мира.
    pub fn registry_id(self) -> std::borrow::Cow<'static, str> {
        let base = crate::world::profile::block_item_registry_id();
        if self.profile == Profile::Java26_1_2 { return std::borrow::Cow::Borrowed(base); }
        static PREVIEW: std::sync::OnceLock<String> = std::sync::OnceLock::new();
        std::borrow::Cow::Borrowed(PREVIEW.get_or_init(|| {
            use aws_lc_rs::digest;
            let mut hash = digest::Context::new(&digest::SHA256);
            for value in ["mcsa-catalog-v1", base, "type:top,bottom,double", "waterlogged:true,false"] {
                hash.update(&(value.len() as u64).to_be_bytes());
                hash.update(value.as_bytes());
            }
            for row in &CONCRETE_SLABS {
                hash.update(&(row.name.len() as u64).to_be_bytes());
                hash.update(row.name.as_bytes());
                for value in [row.item, row.block, row.first_state, row.last_state, row.default_state] { hash.update(&value.to_be_bytes()); }
            }
            let hex: String = hash.finish().as_ref().iter().map(|byte| format!("{byte:02x}")).collect();
            format!("mcsa-catalog-v1:sha256:{hex}")
        }))
    }

    fn extension_named(self, name: &'static str) -> Result<(), Error> {
        match self.profile {
            Profile::Preview26_3 => Ok(()),
            profile => Err(Error::Disabled { profile, name }),
        }
    }

    fn extension(self) -> Result<(), Error> { self.extension_named(WHITE_SLAB_NAME) }

    fn entry_named(name: &str) -> Option<&'static ConcreteSlab> { CONCRETE_SLABS.iter().find(|row| row.name == name) }

    fn entry_id(kind: Kind, id: i32) -> Option<&'static ConcreteSlab> {
        CONCRETE_SLABS.iter().find(|row| match kind {
            Kind::Item => row.item == id,
            Kind::Block => row.block == id,
            Kind::State => (row.first_state..=row.last_state).contains(&id),
        })
    }

    pub fn slab_entry(self, state: i32) -> Result<Option<&'static ConcreteSlab>, Error> {
        self.name(Kind::State, state)?;
        Ok(Self::entry_id(Kind::State, state))
    }

    /// Проверяет существование и доступность ДО выдачи предмета или записи мира.
    /// Профиль775 не принимает extension даже при физически допустимом u16/i32.
    pub fn name(self, kind: Kind, id: i32) -> Result<&'static str, Error> {
        if let Some(row) = Self::entry_id(kind, id) { self.extension_named(row.name)?; return Ok(row.name); }
        let name = match kind {
            Kind::Item => crate::blocks::item_name(id),
            Kind::Block => crate::blocks_table::BLOCK_IDS.iter().find(|(_, value)| *value == id).map(|(name, _)| *name),
            Kind::State => crate::blocks::block_at_state(id),
        };
        name.ok_or(Error::UnknownInternal { kind, id })
    }

    pub fn item_named(self, name: &str) -> Result<Option<i32>, Error> {
        if let Some(row) = Self::entry_named(name) { self.extension_named(row.name)?; return Ok(Some(row.item)); }
        Ok(crate::blocks::item_named(name))
    }

    pub fn default_state(self, name: &str) -> Result<Option<i32>, Error> {
        if let Some(row) = Self::entry_named(name) { self.extension_named(row.name)?; return Ok(Some(row.default_state)); }
        Ok(crate::blocks::state_by_name(name))
    }

    pub fn stack_size(self, item: i32) -> Result<i32, Error> {
        self.name(Kind::Item, item)?;
        Ok(if Self::entry_id(Kind::Item, item).is_some() { 64 } else { crate::blocks::stack_size(item) })
    }

    pub fn slab_state(self, half: SlabHalf, waterlogged: bool) -> Result<i32, Error> {
        self.slab_state_for(&CONCRETE_SLABS[0], half, waterlogged)
    }

    pub fn slab_state_for(self, row: &ConcreteSlab, half: SlabHalf, waterlogged: bool) -> Result<i32, Error> {
        let actual = Self::entry_id(Kind::State, row.first_state).filter(|actual| *actual == row)
            .ok_or(Error::UnknownInternal { kind: Kind::State, id: row.first_state })?;
        self.extension_named(actual.name)?;
        Ok(actual.first_state + half.offset() + i32::from(!waterlogged))
    }

    pub fn slab_properties(self, state: i32) -> Result<Option<(SlabHalf, bool)>, Error> {
        let Some(row) = self.slab_entry(state)? else { return Ok(None); };
        let offset = state - row.first_state;
        let half = match offset / 2 { 0 => SlabHalf::Top, 1 => SlabHalf::Bottom, _ => SlabHalf::Double };
        Ok(Some((half, offset % 2 == 0)))
    }

    /// Старые объекты используют прежние точные таблицы775→client.
    /// Extension преобразуется отдельно; его ID никогда не объявляется ID775.
    pub fn to_client(self, kind: Kind, id: i32, client: JavaVersion) -> Result<u32, Error> {
        let name = self.name(kind, id)?;
        if let Some(row) = Self::entry_id(kind, id) {
            if client != JavaVersion::V26_3 { return Err(Error::Unrepresentable { client, name }); }
            return Ok(match kind {
                Kind::Item => row.wire_item,
                Kind::Block => row.wire_block,
                Kind::State => row.wire_first_state + (id - row.first_state) as u32,
            });
        }
        ids::translate(kind.registry(), JavaVersion::V26_1_2, client, id as u32).map_err(Error::Wire)
    }

    pub fn decode_client_id(self, kind: Kind, id: u32, client: JavaVersion) -> Result<i32, Error> {
        let extension = if client == JavaVersion::V26_3 {
            CONCRETE_SLABS.iter().find_map(|row| match kind {
                Kind::Item if id == row.wire_item => Some(row.item),
                Kind::Block if id == row.wire_block => Some(row.block),
                Kind::State if (row.wire_first_state..row.wire_first_state + 6).contains(&id) => Some(row.first_state + (id - row.wire_first_state) as i32),
                _ => None,
            })
        } else { None };
        if let Some(internal) = extension { self.name(kind, internal)?; return Ok(internal); }
        let internal = ids::translate(kind.registry(), client, JavaVersion::V26_1_2, id).map_err(Error::Wire)?;
        let internal = i32::try_from(internal).map_err(|_| Error::Wire(ids::IdTranslationError::UnknownId(id)))?;
        self.name(kind, internal)?;
        Ok(internal)
    }
}

#[cfg(test)]
#[path = "gameplay_catalog/tests.rs"]
mod tests;
