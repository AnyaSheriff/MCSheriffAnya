// Настройки MCSheriffAnya, которых нет в server.properties.
//
// server.properties должен выглядеть как у обычного сервера, поэтому всё
// своё лежит отдельно — в config/mcsheriffanya.toml. Пока здесь одно: подробный
// лог.

use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::{log_info, log_warn};

/// Файл настроек.
pub const FILE: &str = "config/mcsheriffanya.toml";

/// Где настройки лежали, пока проект звался RustCraft. Такой файл при
/// первом запуске переезжает на новое место — терять настройки из-за
/// смены имени незачем.
pub const OLD_FILE: &str = "config/rustcraft.toml";

/// Настройки.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// Подробный лог: каждый пакет, каждое движение игрока, служебный обмен
    /// с клиентом. Нужен только для поиска неполадок.
    pub debug: bool,
}

/// Образец файла — с пояснениями, чтобы было понятно, что править.
const SAMPLE: &str = "\
# Настройки MCSheriffAnya, которых нет в server.properties.

# Подробный лог: каждый пакет, каждое движение игрока, служебный обмен
# с клиентом. Нужен только для поиска неполадок; в обычной игре — false.
debug = false
";

impl Settings {
    /// Читает настройки; нет файла — кладёт образец и берёт значения по
    /// умолчанию. Испорченный файл — тоже значения по умолчанию,
    /// с предупреждением в логе.
    pub fn load(path: &Path) -> Self {
        move_old_file(path, &path.with_file_name(old_file_name()));

        match fs::read_to_string(path) {
            Ok(text) => match toml::from_str::<Settings>(&text) {
                Ok(settings) => settings,
                Err(error) => {
                    log_warn!("{} не разобрался ({}) — беру всё по умолчанию", path.display(), error);
                    Settings::default()
                }
            },
            Err(_) => {
                if let Err(error) = fs::write(path, SAMPLE) {
                    log_warn!("{} не записан: {}", path.display(), error);
                }

                Settings::default()
            }
        }
    }
}

/// Имя прежнего файла — без папки: он лежит там же, где и новый.
fn old_file_name() -> &'static str {
    Path::new(OLD_FILE).file_name().and_then(|name| name.to_str()).unwrap_or(OLD_FILE)
}

/// Переносит настройки со старого места, если на новом их ещё нет.
fn move_old_file(path: &Path, old: &Path) {
    if path.exists() || !old.exists() {
        return;
    }

    match fs::rename(old, path) {
        Ok(()) => log_info!("Настройки перенесены: {} → {}", old.display(), path.display()),
        Err(error) => log_warn!("{} не перенесён в {}: {}", old.display(), path.display(), error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Нет файла — подробный лог выключен и положен образец; есть файл —
    /// читается; испорчен — по умолчанию.
    #[test]
    fn debug_is_off_unless_asked() {
        let mut path = std::env::temp_dir();
        path.push(format!("mcsheriffanya-settings-{}.toml", std::process::id()));
        let _ = fs::remove_file(&path);

        assert!(!Settings::load(&path).debug);
        assert!(path.exists(), "образец не положен");

        fs::write(&path, "debug = true\n").expect("пишется");
        assert!(Settings::load(&path).debug);

        fs::write(&path, "debug = maybe").expect("пишется");
        assert!(!Settings::load(&path).debug);

        let _ = fs::remove_file(&path);
    }

    /// Настройки, лежавшие под прежним именем, переезжают на новое место
    /// и читаются; если новый файл уже есть, старый не трогается.
    #[test]
    fn old_settings_move_to_the_new_name() {
        let directory = std::env::temp_dir().join(format!("mcsheriffanya-move-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("папка");

        let new = directory.join("mcsheriffanya.toml");
        let old = directory.join(old_file_name());

        fs::write(&old, "debug = true\n").expect("старый файл");

        assert!(Settings::load(&new).debug, "настройки со старого места потерялись");
        assert!(new.exists() && !old.exists(), "файл не переехал");

        // Новый уже есть — старый остаётся лежать как лежал.
        fs::write(&old, "debug = false\n").expect("старый файл снова");
        assert!(Settings::load(&new).debug, "старый файл затёр новый");
        assert!(old.exists());

        let _ = fs::remove_dir_all(&directory);
    }
}
