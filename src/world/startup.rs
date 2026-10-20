//! Проверка профиля до создания мира и сетевых слушателей.
//!
//! Внешний замок не заменяет session.lock измерений и не разрешает снимок
//! работающего мира: прежние сборки об этом замке не знают.

use std::fs::{self, File};
use std::io;
use std::path::{Path, PathBuf};

use super::profile::{self, StartupCheck};

/// Живёт до завершения runtime. Файл замка намеренно не удаляется при Drop:
/// удаление занятого inode позволило бы другому процессу создать новый замок.
pub(crate) struct StartupGuard {
    lock: Option<File>,
    pub(crate) check: StartupCheck,
}

impl StartupGuard {
    pub(crate) fn has_exclusive_lock(&self) -> bool { self.lock.is_some() }
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

/// Не создаёт world, не маркирует старые сохранения и не вызывает World::open.
/// На Unix создаётся только соседний lock-файл; на остальных платформах
/// выполняется читающая проверка без обещания исключительного доступа.
pub(crate) fn prepare(world: &Path) -> io::Result<StartupGuard> {
    let world = world_path(world)?;
    check_directory(&world)?;
    let lock = lock_world(&world)?;
    check_directory(&world)?;
    let check = profile::check_current_world(&world).map_err(|error|
        io::Error::new(io::ErrorKind::InvalidData, format!("запуск мира остановлен: {error}")))?;
    Ok(StartupGuard { lock, check })
}

#[cfg(test)]
mod tests;
