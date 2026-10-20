//! Читающая проверка заявленного профиля сохранения до открытия мира.
//!
//! Манифест — собственный формат MCSA, не формат Mojang. Совпадение его полей
//! не заменяет проверку файлов мира. Этот модуль ничего не создаёт и не меняет;
//! классификация перехода не разрешает преобразование или запуск писателей.
#![allow(dead_code)] // Проверка запуска подключена; полные переносы ещё не реализованы.

use std::cmp::Ordering;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, Read};
use std::path::Path;
use std::sync::OnceLock;

use aws_lc_rs::digest;
use serde::{Deserialize, Serialize};

pub const MANIFEST_NAME: &str = "mcsa-world.json";
pub const MAX_MANIFEST_BYTES: usize = 16 * 1024;
pub const MANIFEST_SCHEMA: u32 = 1;

/// Версии раскладки не являются версией игры или доказательством происхождения.
/// Здесь заявлены только существующие заголовки региона и playerdata.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SaveFormat {
    pub region_layout: u32,
    pub playerdata_layout: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: u32,
    pub save_format: SaveFormat,
    pub registry_id: String,
    pub gameplay_version: String,
    /// Идентификатор полного снимка, 16 байт в нижнем шестнадцатеричном регистре.
    /// Здесь проверяется форма, а не подлинность или содержимое снимка.
    pub snapshot_id: String,
}

/// Явно известный профиль; никакой порядок версий из строк не выводится.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileDescriptor {
    pub gameplay_version: String,
    pub release_order: u32,
    pub save_format: SaveFormat,
    pub registry_id: String,
}

#[derive(Debug)]
pub enum Error {
    Io(io::Error),
    Malformed(serde_json::Error),
    TooLarge,
    UnknownSchema(u32),
    InvalidField(&'static str),
    UnknownSource,
    UnsupportedProfile(String),
    InvalidCatalog,
    FormatMismatch,
    RegistryMismatch,
    TransitionRequired,
    NotRegularFile,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "не удалось прочитать манифест мира: {error}"),
            Self::Malformed(error) => write!(f, "неверный JSON манифеста мира: {error}"),
            Self::TooLarge => write!(f, "манифест мира превышает {MAX_MANIFEST_BYTES} байт"),
            Self::UnknownSchema(schema) => write!(f, "неизвестная схема манифеста мира: {schema}"),
            Self::InvalidField(field) => write!(f, "неверное поле манифеста мира: {field}"),
            Self::UnknownSource => f.write_str("исходный профиль мира неизвестен; перенос запрещён"),
            Self::UnsupportedProfile(version) => write!(f, "профиль мира не поддержан: {version}"),
            Self::InvalidCatalog => f.write_str("неоднозначный или неверный каталог профилей мира"),
            Self::FormatMismatch => f.write_str("раскладка сохранения не соответствует профилю мира"),
            Self::RegistryMismatch => f.write_str("реестр сохранённых ID не соответствует профилю мира"),
            Self::TransitionRequired => f.write_str("нужен ещё не реализованный перенос профиля мира"),
            Self::NotRegularFile => f.write_str("манифест мира должен быть обычным файлом, не ссылкой"),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Malformed(error) => Some(error),
            _ => None,
        }
    }
}

fn identifier(value: &str) -> bool {
    !value.is_empty() && value.len() <= 160
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || b".-_:".contains(&byte))
}

fn validate(manifest: &Manifest) -> Result<(), Error> {
    if manifest.schema != MANIFEST_SCHEMA { return Err(Error::UnknownSchema(manifest.schema)); }
    if !identifier(&manifest.gameplay_version) { return Err(Error::InvalidField("gameplay_version")); }
    if !identifier(&manifest.registry_id) { return Err(Error::InvalidField("registry_id")); }
    if manifest.snapshot_id.len() != 32
        || !manifest.snapshot_id.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)) {
        return Err(Error::InvalidField("snapshot_id"));
    }
    if manifest.save_format.region_layout == 0 || manifest.save_format.playerdata_layout == 0 {
        return Err(Error::InvalidField("save_format"));
    }
    Ok(())
}

/// Лимит применяется до JSON-разбора, в том числе к пробелам после документа.
pub fn decode(bytes: &[u8]) -> Result<Manifest, Error> {
    if bytes.len() > MAX_MANIFEST_BYTES { return Err(Error::TooLarge); }
    let manifest: Manifest = serde_json::from_slice(bytes).map_err(Error::Malformed)?;
    validate(&manifest)?;
    Ok(manifest)
}

fn read_bounded(reader: impl Read) -> Result<Manifest, Error> {
    let mut bytes = Vec::new();
    reader.take((MAX_MANIFEST_BYTES + 1) as u64).read_to_end(&mut bytes).map_err(Error::Io)?;
    decode(&bytes)
}

fn open_regular(path: &Path) -> Result<Option<File>, Error> {
    match path.symlink_metadata() {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(Error::Io(error)),
        Ok(metadata) if !metadata.file_type().is_file() => return Err(Error::NotRegularFile),
        Ok(_) => {}
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        // Не следовать подменённой ссылке и не зависать на подменённом FIFO.
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    // Исчезновение уже замеченного манифеста — ошибка, не старый мир.
    let file = options.open(path).map_err(Error::Io)?;
    if !file.metadata().map_err(Error::Io)?.is_file() { return Err(Error::NotRegularFile); }
    Ok(Some(file))
}

/// Не открывает World, не создаёт каталог и не выводит профиль из конфига.
/// Вызывать под общей блокировкой мира: эта проверка сама блокировку не берёт.
pub fn read_manifest(world: &Path) -> Result<Option<Manifest>, Error> {
    open_regular(&world.join(MANIFEST_NAME))?.map(read_bounded).transpose()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransitionKind { NoChange, Upgrade, Downgrade }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TransitionPlan {
    pub source: ProfileDescriptor,
    pub target: ProfileDescriptor,
    pub kind: TransitionKind,
    pub snapshot_id: String,
}

impl TransitionPlan {
    /// Все изменения требуют полной проверки и пока отсутствующего конвертера.
    pub fn requires_migration(&self) -> bool { self.kind != TransitionKind::NoChange }

    /// Даже повышение требует копии до преобразования сохранения.
    pub fn requires_full_backup(&self) -> bool { self.requires_migration() }
}

fn check_catalog(catalog: &[ProfileDescriptor]) -> Result<(), Error> {
    for (index, profile) in catalog.iter().enumerate() {
        if !identifier(&profile.gameplay_version) || !identifier(&profile.registry_id)
            || profile.save_format.region_layout == 0 || profile.save_format.playerdata_layout == 0
            || catalog[..index].iter().any(|other| other.gameplay_version == profile.gameplay_version
                || other.release_order == profile.release_order) {
            return Err(Error::InvalidCatalog);
        }
    }
    Ok(())
}

/// Каталог передаётся явно, поэтому известный номер сетевого протокола сам по
/// себе не включает игровой профиль. Отсутствующий манифест всегда запрещает
/// планирование, даже если в конфиге указана текущая версия.
pub fn plan_transition(
    manifest: Option<&Manifest>, target: &str, catalog: &[ProfileDescriptor],
) -> Result<TransitionPlan, Error> {
    let manifest = manifest.ok_or(Error::UnknownSource)?;
    validate(manifest)?;
    check_catalog(catalog)?;
    let find = |version: &str| catalog.iter().find(|profile| profile.gameplay_version == version)
        .ok_or_else(|| Error::UnsupportedProfile(version.to_string()));
    let source = find(&manifest.gameplay_version)?;
    let target = find(target)?;
    if source.save_format != manifest.save_format { return Err(Error::FormatMismatch); }
    if source.registry_id != manifest.registry_id { return Err(Error::RegistryMismatch); }
    let kind = match source.release_order.cmp(&target.release_order) {
        Ordering::Equal => TransitionKind::NoChange,
        Ordering::Less => TransitionKind::Upgrade,
        Ordering::Greater => TransitionKind::Downgrade,
    };
    Ok(TransitionPlan { source: source.clone(), target: target.clone(), kind, snapshot_id: manifest.snapshot_id.clone() })
}

/// Отпечаток ТОЛЬКО числовых блоков/состояний/предметов. Он не покрывает биомы,
/// сущности, компоненты, данные плагинов или всё содержимое сохранения.
/// Хешируются скомпилированные таблицы, без копий исходного текста в бинарнике.
pub fn block_item_registry_id() -> &'static str {
    static ID: OnceLock<String> = OnceLock::new();
    ID.get_or_init(|| {
        use crate::blocks_table::{BLOCK_NAMES, ITEM_NAMES, ORIENTED};
        registry_fingerprint(BLOCK_NAMES, ORIENTED, ITEM_NAMES)
    })
}

fn registry_fingerprint(
    blocks: &[(i32, i32, &str)], orientations: &[crate::blocks::Orientation], items: &[(i32, &str)],
) -> String {
    let mut hash = digest::Context::new(&digest::SHA256);
    fn text(hash: &mut digest::Context, value: &str) {
        hash.update(&(value.len() as u64).to_be_bytes());
        hash.update(value.as_bytes());
    }
    text(&mut hash, "block-item-registry-v1");
    hash.update(&(blocks.len() as u64).to_be_bytes());
    for &(low, high, name) in blocks {
        hash.update(&low.to_be_bytes());
        hash.update(&high.to_be_bytes());
        text(&mut hash, name);
    }
    hash.update(&(orientations.len() as u64).to_be_bytes());
    for block in orientations {
        text(&mut hash, block.name);
        hash.update(&block.min.to_be_bytes());
        hash.update(&block.max.to_be_bytes());
        hash.update(&(block.properties.len() as u64).to_be_bytes());
        for property in block.properties {
            text(&mut hash, property.name);
            text(&mut hash, property.default);
            hash.update(&(property.values.len() as u64).to_be_bytes());
            for value in property.values { text(&mut hash, value); }
        }
    }
    hash.update(&(items.len() as u64).to_be_bytes());
    for &(id, name) in items {
        hash.update(&id.to_be_bytes());
        text(&mut hash, name);
    }
    let hex: String = hash.finish().as_ref().iter().map(|byte| format!("{byte:02x}")).collect();
    format!("block-item-registry-v1:sha256:{hex}")
}

/// Единственный текущий основной профиль. 26.3 и старые клиенты не становятся
/// поддержанными игровыми профилями из-за наличия сетевого переводчика.
pub fn current_profile() -> ProfileDescriptor {
    ProfileDescriptor {
        gameplay_version: "26.1.2".into(),
        release_order: 1,
        save_format: SaveFormat { region_layout: 1, playerdata_layout: crate::playerdata::FORMAT_VERSION },
        registry_id: block_item_registry_id().into(),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StartupCheck {
    /// Разрешён прежний путь запуска, но источник всё ещё не установлен.
    LegacyUnidentified,
    /// Совпали только заявленные поля; это не проверка всех файлов мира.
    ManifestMatches { snapshot_id: String },
}

/// Минимальная проверка перед World::open, без автоматической маркировки
/// старого мира. Никакие переносы эта функция не выполняет и не разрешает.
pub fn check_current_world(world: &Path) -> Result<StartupCheck, Error> {
    let Some(manifest) = read_manifest(world)? else { return Ok(StartupCheck::LegacyUnidentified); };
    let current = current_profile();
    let plan = plan_transition(Some(&manifest), &current.gameplay_version, std::slice::from_ref(&current))?;
    if plan.requires_migration() { return Err(Error::TransitionRequired); }
    Ok(StartupCheck::ManifestMatches { snapshot_id: manifest.snapshot_id })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(version: &str, order: u32) -> ProfileDescriptor {
        ProfileDescriptor {
            gameplay_version: version.into(), release_order: order,
            save_format: SaveFormat { region_layout: 1, playerdata_layout: 9 },
            registry_id: format!("fixture-registry-{order}"),
        }
    }

    fn manifest(profile: &ProfileDescriptor) -> Manifest {
        Manifest {
            schema: MANIFEST_SCHEMA, save_format: profile.save_format,
            registry_id: profile.registry_id.clone(), gameplay_version: profile.gameplay_version.clone(),
            snapshot_id: "0123456789abcdef0123456789abcdef".into(),
        }
    }

    #[test]
    fn strict_json_rejects_ambiguous_and_future_documents() {
        let valid = manifest(&fixture("fixture-old", 1));
        let bytes = serde_json::to_vec(&valid).unwrap();
        assert_eq!(decode(&bytes).unwrap(), valid);
        let mut future = valid.clone();
        future.schema = MANIFEST_SCHEMA + 1;
        assert!(matches!(decode(&serde_json::to_vec(&future).unwrap()), Err(Error::UnknownSchema(2))));
        let original = String::from_utf8(bytes).unwrap();
        for invalid in [
            original.replacen('{', "{\"schema\":1,", 1),
            original.replacen('{', "{\"protocol\":775,", 1),
            original.replace("\"region_layout\":1", "\"region_layout\":1,\"extra\":2"),
            original.replace("\"schema\":1", "\"schema\":-1"),
            original.replace("\"schema\":1", "\"schema\":1.0"),
            format!("{original} {{}}"),
            "{}".into(), "{".into(), "null".into(),
        ] {
            assert!(matches!(decode(invalid.as_bytes()), Err(Error::Malformed(_))), "{invalid}");
        }
        for invalid_id in ["", "../../world", "ABCDEF0123456789ABCDEF0123456789AB"] {
            let mut invalid = valid.clone();
            invalid.snapshot_id = invalid_id.into();
            assert!(matches!(decode(&serde_json::to_vec(&invalid).unwrap()), Err(Error::InvalidField("snapshot_id"))));
        }
    }

    #[test]
    fn reading_is_bounded_even_if_reader_has_unlimited_data() {
        struct Endless { bytes: usize }
        impl Read for Endless {
            fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
                self.bytes += bytes.len();
                bytes.fill(b' ');
                Ok(bytes.len())
            }
        }
        let mut reader = Endless { bytes: 0 };
        assert!(matches!(read_bounded(&mut reader), Err(Error::TooLarge)));
        assert_eq!(reader.bytes, MAX_MANIFEST_BYTES + 1);
        let mut bytes = serde_json::to_vec(&manifest(&fixture("fixture", 1))).unwrap();
        bytes.resize(MAX_MANIFEST_BYTES, b' ');
        assert!(decode(&bytes).is_ok());
        bytes.push(b' ');
        assert!(matches!(decode(&bytes), Err(Error::TooLarge)));
    }

    #[test]
    fn transitions_use_explicit_order_and_require_backup_for_any_change() {
        // Имена намеренно в обратном строковом порядке; это НЕ готовые версии.
        let old = fixture("fixture-z", 10);
        let new = fixture("fixture-a", 20);
        let catalog = [new.clone(), old.clone()];
        for (source, target, expected) in [
            (&old, &old, TransitionKind::NoChange),
            (&old, &new, TransitionKind::Upgrade),
            (&new, &old, TransitionKind::Downgrade),
        ] {
            let plan = plan_transition(Some(&manifest(source)), &target.gameplay_version, &catalog).unwrap();
            assert_eq!(plan.kind, expected);
            assert_eq!(plan.requires_full_backup(), expected != TransitionKind::NoChange);
            assert_eq!(plan.requires_migration(), expected != TransitionKind::NoChange);
        }
    }

    #[test]
    fn unknown_source_and_mismatched_identity_fail_closed() {
        let profile = fixture("fixture", 1);
        let catalog = [profile.clone()];
        let original = manifest(&profile);
        assert!(matches!(plan_transition(None, "fixture", &catalog), Err(Error::UnknownSource)));
        assert!(matches!(plan_transition(Some(&original), "26.3", &catalog), Err(Error::UnsupportedProfile(_))));
        let mut bad = original.clone();
        bad.gameplay_version = "unknown-source".into();
        assert!(matches!(plan_transition(Some(&bad), "fixture", &catalog), Err(Error::UnsupportedProfile(_))));
        bad = original.clone();
        bad.registry_id = "another-registry".into();
        assert!(matches!(plan_transition(Some(&bad), "fixture", &catalog), Err(Error::RegistryMismatch)));
        bad = original.clone();
        bad.save_format.playerdata_layout += 1;
        assert!(matches!(plan_transition(Some(&bad), "fixture", &catalog), Err(Error::FormatMismatch)));
        bad = original.clone();
        bad.schema += 1;
        assert!(matches!(plan_transition(Some(&bad), "fixture", &catalog), Err(Error::UnknownSchema(_))));
        let mut ambiguous = profile.clone();
        ambiguous.gameplay_version = "other".into();
        assert!(matches!(plan_transition(Some(&original), "fixture", &[profile, ambiguous]), Err(Error::InvalidCatalog)));
    }

    #[test]
    fn current_descriptor_has_scoped_registry_identity_without_enabling_future_profiles() {
        let current = current_profile();
        assert_eq!(current.gameplay_version, "26.1.2");
        let hex = current.registry_id.strip_prefix("block-item-registry-v1:sha256:").unwrap();
        assert_eq!(hex.len(), 64);
        assert!(hex.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_eq!(current.registry_id, block_item_registry_id());
        assert!(matches!(plan_transition(Some(&manifest(&current)), "26.3", &[current]), Err(Error::UnsupportedProfile(_))));
    }

    #[test]
    fn registry_fingerprint_tracks_ids_names_and_property_order() {
        use crate::blocks::{Family, Orientation, Property};
        let blocks = [(1, 2, "fixture_block")];
        let items = [(3, "fixture_item")];
        let mut orientations = [Orientation {
            name: "fixture_block", min: 1, max: 2, family: Family::Pillar,
            properties: &[Property { name: "axis", values: &["x", "y"], default: "x" }],
        }];
        let initial = registry_fingerprint(&blocks, &orientations, &items);
        assert_ne!(initial, registry_fingerprint(&[(2, 3, "fixture_block")], &orientations, &items));
        assert_ne!(initial, registry_fingerprint(&blocks, &orientations, &[(4, "fixture_item")]));
        assert_ne!(initial, registry_fingerprint(&blocks, &orientations, &[(3, "renamed_item")]));
        orientations[0].properties = &[Property { name: "axis", values: &["y", "x"], default: "x" }];
        assert_ne!(initial, registry_fingerprint(&blocks, &orientations, &items));
        assert_ne!(registry_fingerprint(&[], &[], &[(1, "ab"), (2, "c")]),
            registry_fingerprint(&[], &[], &[(1, "a"), (2, "bc")]));
    }

    #[test]
    fn readonly_guard_does_not_mark_or_create_old_worlds() {
        let directory = std::env::temp_dir().join(format!("mcsa-profile-test-{}-{}", std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        assert_eq!(check_current_world(&directory).unwrap(), StartupCheck::LegacyUnidentified);
        assert!(!directory.exists());
        std::fs::create_dir(&directory).unwrap();
        let path = directory.join(MANIFEST_NAME);
        let current = manifest(&current_profile());
        let bytes = serde_json::to_vec(&current).unwrap();
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(check_current_world(&directory).unwrap(), StartupCheck::ManifestMatches { snapshot_id: current.snapshot_id });
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        std::fs::write(&path, b"{").unwrap();
        assert!(matches!(check_current_world(&directory), Err(Error::Malformed(_))));
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        assert!(matches!(check_current_world(&directory), Err(Error::NotRegularFile)));
        std::fs::remove_dir(&path).unwrap();
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink("absent-target", &path).unwrap();
            assert!(matches!(check_current_world(&directory), Err(Error::NotRegularFile)));
            std::fs::remove_file(&path).unwrap();
        }
        std::fs::remove_dir(&directory).unwrap();
    }
}
