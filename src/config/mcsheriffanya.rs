// Настройки MCSheriffAnya, которых нет в server.properties.
//
// server.properties должен выглядеть как у обычного сервера, поэтому всё
// своё лежит отдельно — в config/mcsa.properties (MCSheriffAnya.properties),
// в том же виде key=value: подробный лог, складывание мира впрок и порт
// для Bedrock.

use std::fs;
use std::path::Path;

use serde::Deserialize;

use super::server_properties::parse_properties;
use crate::{log_info, log_warn};

/// Файл настроек.
pub const FILE: &str = "config/mcsa.properties";

/// Где настройки лежали раньше: сперва `rustcraft.toml` (пока проект звался
/// RustCraft), потом `mcsheriffanya.toml`. Такой файл при первом запуске
/// переводится в новый — терять настройки из-за смены места незачем.
pub const OLD_FILES: [&str; 2] = ["config/mcsheriffanya.toml", "config/rustcraft.toml"];

/// Настройки.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// Подробный лог: каждый пакет, каждое движение игрока, служебный обмен
    /// с клиентом. Нужен только для поиска неполадок.
    pub debug: bool,

    /// На сколько чанков дальше прорисовки мир складывается заранее, пока
    /// игрок ходит: к тому мгновению, как чанк войдёт в обзор, он уже готов.
    /// Эти чанки держатся в памяти. 0 — не складывать впрок.
    pub prefetch_chunks: i32,

    /// Сколько чанков одного игрока складывается впрок одновременно.
    /// Немного: видимым чанкам нельзя уступать ядра.
    pub prefetch_at_once: usize,

    /// UDP-порт для игроков Bedrock Edition. 0 — не пускать Bedrock.
    pub bedrock_port: u16,

    /// Пускать игроков Bedrock только с подтверждённым входом Xbox Live
    /// (как `online-mode` у Java). Выключено — пускаются все, но права по
    /// ops.json получают только подтверждённые.
    pub bedrock_online_mode: bool,

    /// Редкие осенние пятна в обычном лесу — свой биом с осенней листвой.
    pub autumn_forests: bool,

    /// Розоватая трава и чуть больше лепестков в вишнёвых рощах.
    pub pink_cherry_groves: bool,

    /// Справка о сервере текстом тому, кто подключился не игрой
    /// (`telnet адрес порт`, браузер, curl).
    pub text_info: bool,

    /// Сколько тактов стоять в портале, чтобы перейти в другое измерение
    /// (в выживании; в творческом — такт, как у оригинала).
    pub portal_delay: u32,

    /// Отправлять ли анонимные отчёты об ошибках разработчикам (reports.rs).
    pub send_error_reports: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            debug: false,
            prefetch_chunks: 3,
            prefetch_at_once: 4,
            bedrock_port: 19132,
            bedrock_online_mode: false,
            autumn_forests: true,
            pink_cherry_groves: true,
            text_info: true,
            portal_delay: crate::portal::DELAY,
            send_error_reports: false,
        }
    }
}

impl Settings {
    /// Читает настройки; нет файла — кладёт его с пояснениями (перенеся
    /// значения из прежнего файла, если он был) и берёт что есть. Ключ,
    /// который не разобрался, берётся по умолчанию — с предупреждением
    /// в логе.
    pub fn load(path: &Path) -> Self {
        if !path.exists() {
            let settings = OLD_FILES
                .iter()
                .map(|old| path.with_file_name(file_name(old)))
                .find(|old| old.exists())
                .map_or_else(Settings::default, |old| move_old_file(&old, path));

            if let Err(error) = fs::write(path, settings.to_file()) {
                log_warn!("{} не записан: {}", path.display(), error);
            }

            return settings;
        }

        match fs::read_to_string(path) {
            Ok(text) => Settings::parse(&text, path),
            Err(error) => {
                log_warn!("{} не читается ({}) — беру всё по умолчанию", path.display(), error);
                Settings::default()
            }
        }
    }

    /// Разбирает файл: нет ключа — значение по умолчанию.
    fn parse(text: &str, path: &Path) -> Self {
        let values = parse_properties(text);
        let default = Settings::default();

        fn take<T: std::str::FromStr>(values: &std::collections::HashMap<String, String>, key: &str, default: T, path: &Path) -> T {
            match values.get(key) {
                None => default,
                Some(value) => value.parse().unwrap_or_else(|_| {
                    log_warn!("{}: {}={} не разобралось — беру по умолчанию", path.display(), key, value);
                    default
                }),
            }
        }

        Settings {
            debug: take(&values, "debug", default.debug, path),
            prefetch_chunks: take(&values, "prefetch-chunks", default.prefetch_chunks, path),
            prefetch_at_once: take(&values, "prefetch-at-once", default.prefetch_at_once, path),
            bedrock_port: take(&values, "bedrock-port", default.bedrock_port, path),
            bedrock_online_mode: take(&values, "bedrock-online-mode", default.bedrock_online_mode, path),
            autumn_forests: take(&values, "autumn-forests", default.autumn_forests, path),
            pink_cherry_groves: take(&values, "pink-cherry-groves", default.pink_cherry_groves, path),
            text_info: take(&values, "text-info", default.text_info, path),
            portal_delay: take(&values, "portal-delay", default.portal_delay, path),
            send_error_reports: take(&values, "send-error-reports", default.send_error_reports, path),
        }
    }

    /// Файл с пояснениями — чтобы было понятно, что править.
    fn to_file(self) -> String {
        format!(
            "\
# Настройки MCSheriffAnya, которых нет в server.properties.

# Подробный лог: каждый пакет, каждое движение игрока, служебный обмен
# с клиентом. Нужен только для поиска неполадок; в обычной игре — false.
debug={}

# Складывание мира впрок: на сколько чанков дальше прорисовки игрока мир
# готовится заранее (клиенту эти чанки не шлются, но держатся в памяти).
# Когда игрок бежит, новые чанки уже готовы и отправка не ждёт генерации.
# 0 — не складывать впрок.
prefetch-chunks={}

# Сколько чанков одного игрока складывается впрок одновременно.
prefetch-at-once={}

# UDP-порт для игроков Bedrock Edition (телефоны, консоли, Windows).
# 0 — не пускать Bedrock.
bedrock-port={}

# Проверка входа Xbox Live у игроков Bedrock. Проверяется всегда: кто вошёл
# с подтверждённой учётной записью, тот получает права по ops.json.
# true — пускать только подтверждённых (как online-mode у Java);
# false — пускать всех, неподтверждённым — только команды для всех.
bedrock-online-mode={}

# Дополнения к миру, которых нет у оригинала (false — мир как у оригинала).
# Редкие осенние пятна в обычном лесу: рыжая листва и пожухлая трава.
autumn-forests={}

# Вишнёвые рощи с розоватой травой и чуть большим числом лепестков.
pink-cherry-groves={}

# Справка о сервере для тех, кто подключился не игрой: `telnet адрес порт`,
# браузер или curl. Сервер ждёт секунду: клиент Minecraft за это время
# говорит первым, а молчащему отвечают справкой (версии, порты, игроки).
text-info={}

# Сколько тактов (20 в секунду) стоять в портале, чтобы перейти в другое
# измерение, — в выживании. У оригинала 80 (4 секунды); 1 — сразу.
# В творческом переход всегда сразу.
portal-delay={}

# Анонимные отчёты об ошибках разработчикам MCSheriffAnya (падения и строки
# ERROR с последними строками лога). Ники, адреса, UUID, XUID и чат из них
# вычищаются; вместо адреса сервера — случайный номер (config/install-id).
# Копия каждого отчёта остаётся в logs/reports. false — ничего не отправлять.
send-error-reports={}
",
            self.debug,
            self.prefetch_chunks,
            self.prefetch_at_once,
            self.bedrock_port,
            self.bedrock_online_mode,
            self.autumn_forests,
            self.pink_cherry_groves,
            self.text_info,
            self.portal_delay,
            self.send_error_reports
        )
    }
}

/// Имя файла — без папки: прежние файлы лежат там же, где и новый.
fn file_name(path: &str) -> &str {
    Path::new(path).file_name().and_then(|name| name.to_str()).unwrap_or(path)
}

/// Читает прежний toml-файл и убирает его: его значения уйдут в новый файл.
/// Не разобрался — значения по умолчанию, а старый файл остаётся лежать.
fn move_old_file(old: &Path, path: &Path) -> Settings {
    let text = fs::read_to_string(old).unwrap_or_default();

    match toml::from_str::<Settings>(&text) {
        Ok(settings) => {
            match fs::remove_file(old) {
                Ok(()) => log_info!("Настройки перенесены: {} → {}", old.display(), path.display()),
                Err(error) => log_warn!("{} перенесён в {}, но не удалён: {}", old.display(), path.display(), error),
            }

            settings
        }
        Err(error) => {
            log_warn!("{} не разобрался ({}) — беру всё по умолчанию", old.display(), error);
            Settings::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Нет файла — подробный лог выключен и положен файл с пояснениями;
    /// есть файл — читается; испорченное значение — по умолчанию.
    #[test]
    fn debug_is_off_unless_asked() {
        let directory = std::env::temp_dir().join(format!("mcsheriffanya-settings-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("папка");
        let path = directory.join("mcsa.properties");

        assert!(!Settings::load(&path).debug);
        assert!(path.exists(), "файл не положен");

        // Положенный файл читается в те же значения, что и по умолчанию.
        assert_eq!(Settings::load(&path), Settings::default());

        fs::write(&path, "debug=true\nprefetch-chunks=5\n").expect("пишется");
        let settings = Settings::load(&path);
        assert!(settings.debug);
        assert_eq!(settings.prefetch_chunks, 5);
        assert_eq!(settings.prefetch_at_once, Settings::default().prefetch_at_once);
        assert!(!settings.bedrock_online_mode, "проверка входа Bedrock включилась сама");

        fs::write(&path, "bedrock-online-mode=true\n").expect("пишется");
        assert!(Settings::load(&path).bedrock_online_mode);

        fs::write(&path, "debug=maybe").expect("пишется");
        assert!(!Settings::load(&path).debug);

        let _ = fs::remove_dir_all(&directory);
    }

    /// Настройки из прежних toml-файлов переезжают в новый файл; если новый
    /// уже есть, старый не трогается.
    #[test]
    fn old_settings_move_to_the_new_file() {
        for old_name in OLD_FILES {
            let directory = std::env::temp_dir().join(format!("mcsheriffanya-move-{}", std::process::id()));
            let _ = fs::remove_dir_all(&directory);
            fs::create_dir_all(&directory).expect("папка");

            let new = directory.join("mcsa.properties");
            let old = directory.join(file_name(old_name));

            fs::write(&old, "debug = true\nprefetch_chunks = 6\n").expect("старый файл");

            let settings = Settings::load(&new);
            assert!(settings.debug, "настройки из {} потерялись", old_name);
            assert_eq!(settings.prefetch_chunks, 6);
            assert!(new.exists() && !old.exists(), "файл {} не переехал", old_name);
            assert!(Settings::load(&new).debug, "перенесённое не читается из нового файла");

            // Новый уже есть — старый остаётся лежать как лежал.
            fs::write(&old, "debug = false\n").expect("старый файл снова");
            assert!(Settings::load(&new).debug, "старый файл затёр новый");
            assert!(old.exists());

            let _ = fs::remove_dir_all(&directory);
        }
    }
}
