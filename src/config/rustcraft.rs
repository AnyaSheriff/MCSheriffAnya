// Настройки RustCraft, которых нет в server.properties.
//
// server.properties должен выглядеть как у обычного сервера, поэтому всё
// своё лежит отдельно — в config/rustcraft.toml. Пока здесь одно: подробный
// лог.

use std::fs;
use std::path::Path;

use serde::Deserialize;

use crate::log_warn;

/// Файл настроек.
pub const FILE: &str = "config/rustcraft.toml";

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
# Настройки RustCraft, которых нет в server.properties.

# Подробный лог: каждый пакет, каждое движение игрока, служебный обмен
# с клиентом. Нужен только для поиска неполадок; в обычной игре — false.
debug = false
";

impl Settings {
    /// Читает настройки; нет файла — кладёт образец и берёт значения по
    /// умолчанию. Испорченный файл — тоже значения по умолчанию,
    /// с предупреждением в логе.
    pub fn load(path: &Path) -> Self {
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Нет файла — подробный лог выключен и положен образец; есть файл —
    /// читается; испорчен — по умолчанию.
    #[test]
    fn debug_is_off_unless_asked() {
        let mut path = std::env::temp_dir();
        path.push(format!("rustcraft-settings-{}.toml", std::process::id()));
        let _ = fs::remove_file(&path);

        assert!(!Settings::load(&path).debug);
        assert!(path.exists(), "образец не положен");

        fs::write(&path, "debug = true\n").expect("пишется");
        assert!(Settings::load(&path).debug);

        fs::write(&path, "debug = maybe").expect("пишется");
        assert!(!Settings::load(&path).debug);

        let _ = fs::remove_file(&path);
    }
}
