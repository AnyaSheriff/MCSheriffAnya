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
#[allow(unused_imports)]
pub use mcsa_protocol::content::{BlockForm, SlabDescriptor, SlabFamily, CONCRETE_SLABS, WOOL_SLABS, CONCRETE_STAIRS, WOOL_STAIRS, MINERALS, POPLAR, slabs, stairs, minerals, poplar, entries};
mod mob_validation;
mod hive_validation;
pub mod changed_rules;

#[cfg(test)]
mod hive_validation_tests;
pub use stack_validation::{StackError, StackLocation, StackProblem};

/// Это серверный профиль, а не согласованная версия сетевого соединения.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Profile {
    Java26_1_2,
    /// База775 +16бетонных и16шерстяных плит, НЕ полный каталог26.3.
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

/// Четыре стороны горизонта в порядке значений facing блока 777.
pub const STAIR_FACINGS: [&str; 4] = ["north", "south", "west", "east"];
pub const STAIR_SHAPES: [&str; 5] = ["straight", "inner_left", "inner_right", "outer_left", "outer_right"];

/// Свойства ступеней: индексы в порядке значений блока 777.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StairState { pub facing: u8, pub top: bool, pub shape: u8, pub waterlogged: bool }

impl StairState {
    pub fn from_offset(offset: u8) -> Self {
        let wet = offset.is_multiple_of(2);
        let rest = offset / 2;
        let shape = rest % 5;
        let rest = rest / 5;
        Self { facing: rest / 2, top: rest.is_multiple_of(2), shape, waterlogged: wet }
    }
    pub fn offset(self) -> u8 {
        (((self.facing * 2 + u8::from(!self.top)) * 5 + self.shape) * 2) + u8::from(!self.waterlogged)
    }
    pub fn facing_name(self) -> &'static str { STAIR_FACINGS[usize::from(self.facing)] }
    pub fn shape_name(self) -> &'static str { STAIR_SHAPES[usize::from(self.shape)] }
}

impl StairState {
    /// Ящики столкновений ступеней внутри клетки: (от, до) по x, y, z.
    /// Нижняя половина — плита; верхняя ступень смотрит в сторону facing; угол по shape.
    /// [проверить] геометрию физикой клиента; форма следует вики (Stairs): прямая — полблока,
    /// внешний угол — четверть, внутренний — три четверти верхнего уровня.
    pub fn boxes(self) -> Vec<([f64; 3], [f64; 3])> {
        let (base, step) = if self.top { ((0.5, 1.0), (0.0, 0.5)) } else { ((0.0, 0.5), (0.5, 1.0)) };
        // Половины клетки по (x, z): north — z 0..0.5, south — z 0.5..1, west — x 0..0.5, east — x 0.5..1.
        let half = |side: &str| -> ([f64; 2], [f64; 2]) {
            match side { "north" => ([0.0, 1.0], [0.0, 0.5]), "south" => ([0.0, 1.0], [0.5, 1.0]), "west" => ([0.0, 0.5], [0.0, 1.0]), _ => ([0.5, 1.0], [0.0, 1.0]) }
        };
        let facing = self.facing_name();
        // Слева от взгляда — против часовой стрелки сверху.
        let left = match facing { "north" => "west", "west" => "south", "south" => "east", _ => "north" };
        let right = match left { "north" => "south", "south" => "north", "west" => "east", _ => "west" };
        let back = match facing { "north" => "south", "south" => "north", "west" => "east", _ => "west" };
        let quarter = |a: &str, b: &str| -> ([f64; 2], [f64; 2]) {
            let (ax, az) = half(a); let (bx, bz) = half(b);
            ([ax[0].max(bx[0]), ax[1].min(bx[1])], [az[0].max(bz[0]), az[1].min(bz[1])])
        };
        let mut parts = vec![half(facing)];
        match self.shape_name() {
            "outer_left" => parts = vec![quarter(facing, left)],
            "outer_right" => parts = vec![quarter(facing, right)],
            "inner_left" => parts.push(quarter(left, back)),
            "inner_right" => parts.push(quarter(right, back)),
            _ => {}
        }
        let mut boxes = vec![([0.0, base.0, 0.0], [1.0, base.1, 1.0])];
        boxes.extend(parts.into_iter().map(|(x, z)| ([x[0], step.0, z[0]], [x[1], step.1, z[1]])));
        boxes
    }
}

/// Цельный непрозрачный куб каталога (сера, киноварь и их кирпичи, мощная сера): ведёт себя как камень.
/// Состояния каталога существуют только в профиле Preview26_3, поэтому проверка по номеру безопасна.
pub fn is_spike(state: i32) -> bool { crate::sulfur_spike::is_spike(state) }

pub fn opaque_state(state: i32) -> bool {
    // Все объекты серы лежат после ступеней: быстрый отказ для обычных блоков.
    state >= MINERALS[0].first_state
        && MINERALS.iter().any(|row| matches!(row.form, BlockForm::Full | BlockForm::PotentSulfur) && (row.first_state..=row.last_state).contains(&state))
}

/// Маленькое значение принадлежит серверу/миру и передаётся явно.
/// Здесь нет изменяемого глобального выбранного профиля или копии движка мира.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Catalog { profile: Profile }

impl Catalog {
    pub const fn new(profile: Profile) -> Self { Self { profile } }
    pub const fn profile(self) -> Profile { self.profile }

    pub fn for_version(version: mcsa_protocol::JavaVersion) -> Self {
        Self::new(if version == mcsa_protocol::JavaVersion::V26_3 { Profile::Preview26_3 } else { Profile::Java26_1_2 })
    }


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
            for value in ["mcsa-catalog-v1", base, "type:top,bottom,double", "waterlogged:true,false", "stairs:facing,half,shape,waterlogged"] {
                hash.update(&(value.len() as u64).to_be_bytes());
                hash.update(value.as_bytes());
            }
            for row in entries() {
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

    fn entry_named(name: &str) -> Option<&'static SlabDescriptor> { entries().find(|row| row.name == name) }

    fn entry_id(kind: Kind, id: i32) -> Option<&'static SlabDescriptor> {
        // Каталог расширяет базу775 только после этих границ. Проверка
        // сохранения обходит каждую ячейку под замком мира: обычные ID
        // не должны заново просматривать все расширения.
        let first = match kind { Kind::Item => WHITE_SLAB_ITEM, Kind::Block => WHITE_SLAB_BLOCK, Kind::State => WHITE_SLAB_FIRST_STATE };
        if (0..first).contains(&id) { return None; }
        entries().find(|row| match kind {
            Kind::Item => row.has_item() && row.item == id,
            Kind::Block => row.block == id,
            Kind::State => (row.first_state..=row.last_state).contains(&id),
        })
    }

    /// Только плиты: ступени проходят через `block_entry`/`stair_properties`.
    pub fn slab_entry(self, state: i32) -> Result<Option<&'static SlabDescriptor>, Error> {
        Ok(self.block_entry(state)?.filter(|row| row.form == BlockForm::Slab))
    }

    /// Любой объект каталога (плита или ступени) по внутреннему состоянию.
    pub fn block_entry(self, state: i32) -> Result<Option<&'static SlabDescriptor>, Error> {
        self.name(Kind::State, state)?;
        Ok(Self::entry_id(Kind::State, state))
    }

    /// Свойства ступеней каталога: (facing, half, shape, waterlogged) по порядку значений
    /// блока 777. Номер состояния = ((facing*2+half)*5+shape)*2+(не waterlogged).
    pub fn stair_properties(self, state: i32) -> Result<Option<StairState>, Error> {
        let Some(row) = self.block_entry(state)?.filter(|row| row.form == BlockForm::Stairs) else { return Ok(None); };
        Ok(Some(StairState::from_offset((state - row.first_state) as u8)))
    }

    /// Состояние ступеней каталога по свойствам; None, если строка не ступени.
    pub fn stair_state_for(self, row: &SlabDescriptor, value: StairState) -> Result<i32, Error> {
        self.extension_named(row.name)?;
        if row.form != BlockForm::Stairs { return Err(Error::UnknownInternal { kind: Kind::State, id: row.first_state }); }
        Ok(row.first_state + i32::from(value.offset()))
    }

    /// Налита ли вода в клетку: плиты и ступени каталога.
    pub fn waterlogged(self, state: i32) -> Result<Option<bool>, Error> {
        if let Some((_, wet)) = self.slab_properties(state)? { return Ok(Some(wet)); }
        if let Some(stair) = self.stair_properties(state)? { return Ok(Some(stair.waterlogged)); }
        // Тополь: свойства дубового двойника (src/poplar.rs), в том числе у забора, люка и листвы.
        if self.block_entry(state)?.is_some_and(|row| row.family == SlabFamily::Poplar) {
            return Ok(crate::blocks::orientation_of(state).and_then(|rule| rule.value(state, "waterlogged")).map(|value| value == "true"));
        }
        // Стенка и шип: порядок свойств как у стенки из камня / капельника, значит и смещение то же.
        let Some(equivalent) = self.base_equivalent(state)? else { return Ok(None); };
        Ok(crate::blocks::orientation_of(equivalent).and_then(|rule| rule.value(equivalent, "waterlogged")).map(|value| value == "true"))
    }

    /// Состояние базового блока с тем же порядком свойств: стенки — `cobblestone_wall`,
    /// шипы — `pointed_dripstone` (у серы свойства те же и в том же порядке, смещения равны).
    /// Нужно только там, где свойства или форма берутся у уже изученного базового блока.
    pub fn base_equivalent(self, state: i32) -> Result<Option<i32>, Error> {
        let Some(row) = self.block_entry(state)? else { return Ok(None); };
        let base = match row.form {
            BlockForm::Wall => "cobblestone_wall",
            BlockForm::Spike => "pointed_dripstone",
            _ => return Ok(None),
        };
        Ok(crate::blocks::states_of(base).map(|(first, _)| first + (state - row.first_state)))
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
        if let Some(row) = Self::entry_named(name) { self.extension_named(row.name)?; return Ok(row.has_item().then_some(row.item)); }
        Ok(crate::blocks::item_named(name))
    }

    pub fn default_state(self, name: &str) -> Result<Option<i32>, Error> {
        if let Some(row) = Self::entry_named(name) { self.extension_named(row.name)?; return Ok(Some(row.default_state)); }
        Ok(crate::blocks::state_by_name(name))
    }

    pub fn stack_size(self, item: i32) -> Result<i32, Error> {
        self.name(Kind::Item, item)?;
        // У тополя размер стопки как у дубового двойника (таблички по 16); у остального каталога — 64.
        Ok(if Self::entry_id(Kind::Item, item).is_some_and(|row| row.family != SlabFamily::Poplar) { 64 } else { crate::blocks::stack_size(item) })
    }

    pub fn slab_state(self, half: SlabHalf, waterlogged: bool) -> Result<i32, Error> {
        self.slab_state_for(&CONCRETE_SLABS[0], half, waterlogged)
    }

    pub fn slab_state_for(self, row: &SlabDescriptor, half: SlabHalf, waterlogged: bool) -> Result<i32, Error> {
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
            if client == JavaVersion::V26_1_2 {
                return self.content_codec().map(kind.registry(), mcsa_protocol::content::IdSpace::Canonical, mcsa_protocol::content::IdSpace::Wire(client), id as u32)
                    .map_err(|_| Error::Unrepresentable { client, name });
            }
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
            entries().find_map(|row| match kind {
                Kind::Item if row.has_item() && id == row.wire_item => Some(row.item),
                Kind::Block if id == row.wire_block => Some(row.block),
                Kind::State if (row.wire_first_state..row.wire_first_state + row.state_count()).contains(&id) => Some(row.first_state + (id - row.wire_first_state) as i32),
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

#[cfg(test)]
mod stairs_tests;

#[cfg(test)]
mod mineral_tests;

#[cfg(test)]
mod poplar_tests;
#[cfg(test)]
mod furnish_tests;

#[cfg(test)]
mod lookup_timing_tests {
    use super::*;

    #[test]
    fn base_fast_path_preserves_every_canonical_lookup() {
        for kind in [Kind::Item, Kind::Block, Kind::State] {
            for id in (-1..=65_536).chain([i32::MIN, i32::MAX]) {
                let expected = entries().find(|row| match kind {
                    Kind::Item => row.has_item() && row.item == id,
                    Kind::Block => row.block == id,
                    Kind::State => (row.first_state..=row.last_state).contains(&id),
                });
                assert_eq!(Catalog::entry_id(kind, id), expected, "{kind:?}/{id}");
            }
        }
    }
}
