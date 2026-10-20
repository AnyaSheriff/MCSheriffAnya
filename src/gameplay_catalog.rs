//! Явный серверный каталог: прежние ID775 и дописываемые внутренние объекты.
//!
//! Клиентская версия не выбирает игровые правила. Экспериментальный каталог
//! пока не подключён к Shared, сохранениям или production admission.
//! Источник первого объекта — выбранные pinned JSON в white-concrete-slab.json;
//! свойства поведения ещё требуют обычного наблюдения [проверить].
#![allow(dead_code)] // Основа следующего среза; production-путь остаётся775.

use mcsa_protocol::{JavaVersion, ids::{self, RegistryKind}};

/// Это серверный профиль, а не согласованная версия сетевого соединения.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Profile {
    Java26_1_2,
    /// Только база775 + white_concrete_slab, НЕ полный каталог26.3.
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
            for value in ["mcsa-catalog-v1", base, WHITE_SLAB_NAME, "type:top,bottom,double", "waterlogged:true,false"] {
                hash.update(&(value.len() as u64).to_be_bytes());
                hash.update(value.as_bytes());
            }
            for value in [WHITE_SLAB_ITEM, WHITE_SLAB_BLOCK, WHITE_SLAB_FIRST_STATE, WHITE_SLAB_LAST_STATE, WHITE_SLAB_DEFAULT_STATE] {
                hash.update(&value.to_be_bytes());
            }
            let hex: String = hash.finish().as_ref().iter().map(|byte| format!("{byte:02x}")).collect();
            format!("mcsa-catalog-v1:sha256:{hex}")
        }))
    }

    fn extension(self) -> Result<(), Error> {
        match self.profile {
            Profile::Preview26_3 => Ok(()),
            profile => Err(Error::Disabled { profile, name: WHITE_SLAB_NAME }),
        }
    }

    /// Проверяет существование и доступность ДО выдачи предмета или записи мира.
    /// Профиль775 не принимает extension даже при физически допустимом u16/i32.
    pub fn name(self, kind: Kind, id: i32) -> Result<&'static str, Error> {
        let extension = match kind {
            Kind::Item => id == WHITE_SLAB_ITEM,
            Kind::Block => id == WHITE_SLAB_BLOCK,
            Kind::State => (WHITE_SLAB_FIRST_STATE..=WHITE_SLAB_LAST_STATE).contains(&id),
        };
        if extension { self.extension()?; return Ok(WHITE_SLAB_NAME); }
        let name = match kind {
            Kind::Item => crate::blocks::item_name(id),
            Kind::Block => crate::blocks_table::BLOCK_IDS.iter().find(|(_, value)| *value == id).map(|(name, _)| *name),
            Kind::State => crate::blocks::block_at_state(id),
        };
        name.ok_or(Error::UnknownInternal { kind, id })
    }

    pub fn item_named(self, name: &str) -> Result<Option<i32>, Error> {
        if name == WHITE_SLAB_NAME { self.extension()?; return Ok(Some(WHITE_SLAB_ITEM)); }
        Ok(crate::blocks::item_named(name))
    }

    pub fn default_state(self, name: &str) -> Result<Option<i32>, Error> {
        if name == WHITE_SLAB_NAME { self.extension()?; return Ok(Some(WHITE_SLAB_DEFAULT_STATE)); }
        Ok(crate::blocks::state_by_name(name))
    }

    pub fn stack_size(self, item: i32) -> Result<i32, Error> {
        self.name(Kind::Item, item)?;
        Ok(if item == WHITE_SLAB_ITEM { 64 } else { crate::blocks::stack_size(item) })
    }

    pub fn slab_state(self, half: SlabHalf, waterlogged: bool) -> Result<i32, Error> {
        self.extension()?;
        Ok(WHITE_SLAB_FIRST_STATE + half.offset() + i32::from(!waterlogged))
    }

    pub fn slab_properties(self, state: i32) -> Result<Option<(SlabHalf, bool)>, Error> {
        self.name(Kind::State, state)?;
        if !(WHITE_SLAB_FIRST_STATE..=WHITE_SLAB_LAST_STATE).contains(&state) { return Ok(None); }
        let offset = state - WHITE_SLAB_FIRST_STATE;
        let half = match offset / 2 { 0 => SlabHalf::Top, 1 => SlabHalf::Bottom, _ => SlabHalf::Double };
        Ok(Some((half, offset % 2 == 0)))
    }

    /// Старые объекты используют прежние точные таблицы775→client.
    /// Extension преобразуется отдельно; его ID никогда не объявляется ID775.
    pub fn to_client(self, kind: Kind, id: i32, client: JavaVersion) -> Result<u32, Error> {
        let name = self.name(kind, id)?;
        if name == WHITE_SLAB_NAME {
            if client != JavaVersion::V26_3 { return Err(Error::Unrepresentable { client, name }); }
            return Ok(match kind {
                Kind::Item => WHITE_SLAB_WIRE_ITEM,
                Kind::Block => WHITE_SLAB_WIRE_BLOCK,
                Kind::State => WHITE_SLAB_WIRE_FIRST_STATE + (id - WHITE_SLAB_FIRST_STATE) as u32,
            });
        }
        ids::translate(kind.registry(), JavaVersion::V26_1_2, client, id as u32).map_err(Error::Wire)
    }

    pub fn decode_client_id(self, kind: Kind, id: u32, client: JavaVersion) -> Result<i32, Error> {
        let extension = if client == JavaVersion::V26_3 {
            match kind {
                Kind::Item if id == WHITE_SLAB_WIRE_ITEM => Some(WHITE_SLAB_ITEM),
                Kind::Block if id == WHITE_SLAB_WIRE_BLOCK => Some(WHITE_SLAB_BLOCK),
                Kind::State if (WHITE_SLAB_WIRE_FIRST_STATE..WHITE_SLAB_WIRE_FIRST_STATE + 6).contains(&id) =>
                    Some(WHITE_SLAB_FIRST_STATE + (id - WHITE_SLAB_WIRE_FIRST_STATE) as i32),
                _ => None,
            }
        } else { None };
        if let Some(internal) = extension { self.extension()?; return Ok(internal); }
        let internal = ids::translate(kind.registry(), client, JavaVersion::V26_1_2, id).map_err(Error::Wire)?;
        let internal = i32::try_from(internal).map_err(|_| Error::Wire(ids::IdTranslationError::UnknownId(id)))?;
        self.name(kind, internal)?;
        Ok(internal)
    }
}

#[cfg(test)]
#[path = "gameplay_catalog/tests.rs"]
mod tests;
