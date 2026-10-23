//! Полная побайтовая копия папки мира; узкий storageupgrade публикуется под теми же замками.
//!
//! Снимок не преобразует формат и не разрешает downgrade. Для будущего перехода
//! наличие проверенного конвертера должно проверяться ДО вызова этого модуля.
#![allow(dead_code)] // Полные переносы игровых профилей ещё не реализованы.

use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use super::startup::StartupGuard;

#[derive(Debug)]
pub(crate) struct BackupError {
    /// Уже созданная папка сохраняется для диагностики, никогда не чистится.
    pub(crate) retained: Option<PathBuf>,
    /// Rename состоялся, но sync родителя не подтверждён. Не повторять вслепую.
    pub(crate) publication_uncertain: bool,
    cause: io::Error,
}
impl fmt::Display for BackupError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "резервная копия мира не подтверждена: {}", self.cause)?;
        if let Some(path) = &self.retained { write!(f, "; сохранено для проверки: {}", path.display())?; }
        if self.publication_uncertain { f.write_str("; публикация могла состояться, требуется повторная проверка")?; }
        Ok(())
    }
}
impl std::error::Error for BackupError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> { Some(&self.cause) }
}

/// Конструктор закрыт: это результат проверки bytes/hash/полноты, не имя папки.
#[derive(Debug)]
pub(crate) struct VerifiedBackup {
    directory: PathBuf,
    snapshot_id: String,
    file_count: u64,
    total_bytes: u64,
    entries_sha256: String,
}
impl VerifiedBackup {
    pub(crate) fn directory(&self) -> &Path { &self.directory }
    pub(crate) fn snapshot_id(&self) -> &str { &self.snapshot_id }
    pub(crate) fn file_count(&self) -> u64 { self.file_count }
    pub(crate) fn total_bytes(&self) -> u64 { self.total_bytes }
    pub(crate) fn entries_sha256(&self) -> &str { &self.entries_sha256 }
}

/// Пока только Linux: openat2 запрещает также bind-mount переходы. Нет fallback
/// к path-based open или копии без исключительных замков.
pub(crate) fn create_verified_backup(guard: &StartupGuard) -> Result<VerifiedBackup, BackupError> {
    #[cfg(target_os = "linux")]
    { linux::create(guard) }
    #[cfg(not(target_os = "linux"))]
    { let _ = guard; Err(unsupported()) }
}

/// Повторная полная проверка final backup после перезапуска. Partial никогда
/// не принимается. Восстановление файлов/смена текущего мира здесь отсутствуют.
pub(crate) fn verify_backup(path: &Path) -> Result<VerifiedBackup, BackupError> {
    #[cfg(target_os = "linux")]
    { linux::verify(path) }
    #[cfg(not(target_os = "linux"))]
    { let _ = path; Err(unsupported()) }
}

#[cfg(not(target_os = "linux"))]
fn unsupported() -> BackupError {
    BackupError { retained: None, publication_uncertain: false,
        cause: io::Error::new(io::ErrorKind::Unsupported, "безопасный снимок пока поддержан только на Linux с openat2") }
}

#[cfg(target_os = "linux")]
mod linux;

/// Только известный переход storage10→11. Манифест меняется после полной
/// проверки копии, пока session.lock всех измерений ещё удерживаются.
pub(crate) fn create_storage_upgrade(guard: &StartupGuard, expected: &super::profile::Manifest) -> Result<VerifiedBackup, BackupError> {
    #[cfg(target_os = "linux")]
    { linux::create_storage_upgrade(guard, expected) }
    #[cfg(not(target_os = "linux"))]
    { let _ = (guard, expected); Err(unsupported()) }
}

/// Не определяет игровой профиль: копия неизвестного мира и отдельный receipt
/// новых раскладок публикуются при непрерывном удержании session.lock.
pub(crate) fn create_unidentified_storage_receipt(guard: &StartupGuard) -> Result<VerifiedBackup, BackupError> {
    #[cfg(target_os = "linux")]
    { linux::create_unidentified_storage_receipt(guard) }
    #[cfg(not(target_os = "linux"))]
    { let _ = guard; Err(unsupported()) }
}

#[cfg(target_os = "linux")]
pub(crate) fn create_profile_upgrade(guard: &StartupGuard) -> Result<VerifiedBackup, BackupError> {
    linux::create_profile_upgrade(guard)
}
