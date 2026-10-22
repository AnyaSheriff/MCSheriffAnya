//! Проверка профиля до создания мира и сетевых слушателей.
//!
//! Внешний замок не заменяет session.lock измерений и не разрешает снимок
//! работающего мира: прежние сборки об этом замке не знают.

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use super::profile::{self, StartupCheck};

pub(super) mod storage_receipt;

/// Живёт до завершения runtime. Файл замка намеренно не удаляется при Drop:
/// удаление занятого inode позволило бы другому процессу создать новый замок.
pub(crate) struct StartupGuard {
    lock: Option<File>,
    #[cfg(target_os = "linux")]
    world: PathBuf,
    #[cfg(target_os = "linux")]
    identity: Option<(u64, u64)>,
    pub(crate) check: StartupCheck,
}

impl StartupGuard {
    pub(crate) fn has_exclusive_lock(&self) -> bool { self.lock.is_some() }
    #[cfg(target_os = "linux")]
    pub(crate) fn world_path(&self) -> &Path { &self.world }
    #[cfg(target_os = "linux")]
    pub(crate) fn world_identity(&self) -> Option<(u64, u64)> { self.identity }
    #[cfg(target_os = "linux")]
    pub(crate) fn snapshot_lock(&self) -> Option<&File> { self.lock.as_ref() }
}

fn world_path(world: &Path) -> io::Result<PathBuf> {
    let name = world.file_name().ok_or_else(|| io::Error::new(
        io::ErrorKind::InvalidInput, "путь мира должен содержать имя каталога",
    ))?;
    let parent = world.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or(Path::new("."));
    Ok(parent.canonicalize()?.join(name))
}

fn check_directory(world: &Path) -> io::Result<()> {
    match fs::symlink_metadata(world) {
        Ok(metadata) if metadata.file_type().is_dir() => Ok(()),
        Ok(_) => Err(io::Error::new(io::ErrorKind::InvalidInput,
            format!("мир {} должен быть обычным каталогом, не ссылкой", world.display()))),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(unix)]
fn lock_world(world: &Path) -> io::Result<Option<File>> {
    use std::os::fd::AsRawFd;
    use std::os::unix::fs::OpenOptionsExt;

    let mut name = std::ffi::OsString::from(".mcsa-");
    name.push(world.file_name().expect("проверенное имя мира"));
    name.push(".lock");
    let path = world.with_file_name(name);
    let file = fs::OpenOptions::new().read(true).write(true).create(true).truncate(false)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK).open(&path)?;
    if !file.metadata()?.file_type().is_file() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput,
            format!("замок мира {} должен быть обычным файлом", path.display())));
    }
    // SAFETY: file владеет действующим дескриптором и хранится в StartupGuard.
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        let error = io::Error::last_os_error();
        if error.kind() == io::ErrorKind::WouldBlock {
            return Err(io::Error::new(io::ErrorKind::WouldBlock,
                format!("папка мира {} уже занята другим процессом", world.display())));
        }
        return Err(error);
    }
    Ok(Some(file))
}

#[cfg(not(unix))]
fn lock_world(_world: &Path) -> io::Result<Option<File>> {
    // Не выдаём открытый файл за межпроцессный замок. Переносы и снимки
    // пока не реализованы; обычный запуск сохраняет прежние ограничения.
    Ok(None)
}

/// Не создаёт world и не маркирует неизвестные сохранения. Единственное
/// изменение до World::open — допуск новых раскладок после полной копии.
/// Неопознанный непустой мир получает только storage receipt, не игровой профиль.
/// На остальных платформах этот перенос явно не поддержан.
pub(crate) fn prepare(world: &Path) -> io::Result<StartupGuard> {
    prepare_with_upgrade(world, |guard, manifest| {
        super::snapshot::create_storage_upgrade(guard, manifest).map(|_| ()).map_err(io::Error::other)
    })
}

fn prepare_with_upgrade(world: &Path,
    upgrade: impl FnOnce(&StartupGuard, &profile::Manifest) -> io::Result<()>,
) -> io::Result<StartupGuard> {
    let world = world_path(world)?;
    check_directory(&world)?;
    let lock = lock_world(&world)?;
    check_directory(&world)?;
    let manifest = profile::read_manifest(&world).map_err(profile_error)?;
    let receipt = storage_receipt::read(&world)?;
    let unidentified_backup = manifest.is_none() && receipt.is_none() && nonempty_world(&world)?;
    let current = profile::current_profile();
    let pending = if let Some(manifest) = &manifest {
        let plan = profile::plan_transition(Some(manifest), &current.gameplay_version, std::slice::from_ref(&current))
            .map_err(profile_error)?;
        match plan.kind {
            profile::TransitionKind::NoChange => false,
            profile::TransitionKind::StorageUpgrade => true,
            _ => return Err(profile_error(profile::Error::TransitionRequired)),
        }
    } else { false };
    let check = if pending {
        StartupCheck::StorageUpgradePending { snapshot_id: manifest.as_ref().expect("известный исходный профиль").snapshot_id.clone() }
    } else { profile::check_current_world(&world).map_err(profile_error)? };
    #[cfg(target_os = "linux")]
    let identity = {
        use std::os::unix::fs::MetadataExt;
        match fs::symlink_metadata(&world) {
            Ok(metadata) if metadata.is_dir() => Some((metadata.dev(), metadata.ino())),
            Ok(_) => return Err(io::Error::new(io::ErrorKind::InvalidInput, "путь мира изменился во время запуска")),
            Err(error) if error.kind() == io::ErrorKind::NotFound => None,
            Err(error) => return Err(error),
        }
    };
    let mut guard = StartupGuard { lock, #[cfg(target_os = "linux")] world: world.clone(), #[cfg(target_os = "linux")] identity, check };
    if pending {
        upgrade(&guard, manifest.as_ref().expect("известный исходный профиль"))?;
        guard.check = profile::check_current_world(&world).map_err(profile_error)?;
    } else if unidentified_backup {
        super::snapshot::create_unidentified_storage_receipt(&guard).map_err(io::Error::other)?;
        // Раскладка записи известна, игровой профиль по-прежнему НЕ установлен.
        guard.check = profile::check_current_world(&world).map_err(profile_error)?;
    }
    Ok(guard)
}

fn nonempty_world(world: &Path) -> io::Result<bool> {
    match fs::read_dir(world) {
        Ok(mut entries) => entries.next().transpose().map(|entry| entry.is_some()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

fn profile_error(error: profile::Error) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, format!("запуск мира остановлен: {error}"))
}

#[cfg(test)]
mod tests;
