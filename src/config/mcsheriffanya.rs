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

/// Проверка лицензий в Java offline-mode, независимо от парольной защиты.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(rename_all="lowercase")]
pub enum OfflineLicenseMode { Cosmetics, Disabled, Invalid }
impl std::str::FromStr for OfflineLicenseMode {
    type Err = std::convert::Infallible;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(match value { "cosmetics" => Self::Cosmetics, "disabled" => Self::Disabled, _ => Self::Invalid })
    }
}
impl std::fmt::Display for OfflineLicenseMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self { Self::Cosmetics=>"cosmetics", Self::Disabled=>"disabled", Self::Invalid=>"invalid" })
    }
}

/// Настройки.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// Проверяем лимит даже в сборках без запуска внешнего ViaProxy.
    pub viaproxy: bool,
    #[serde(deserialize_with="read_viaproxy_memory")]
    pub viaproxy_memory: u64,
    /// Неизвестное значение не должно незаметно менять версию существующего мира.
    #[serde(deserialize_with="read_java_version")]
    pub java_version: Option<mcsa_protocol::JavaVersion>,
    /// Парольный диалог для Java offline-mode до входа в мир.
    pub auth_enabled: bool,
    /// Пробовать лицензию; неудача разрешает cracked-повторный вход.
    pub auth_premium: bool,
    /// Имеет приоритет над устаревшим auth-premium.
    pub offline_license_mode: OfflineLicenseMode,
    /// Подробный лог: каждый пакет, каждое движение игрока, служебный обмен
    /// с клиентом. Нужен только для поиска неполадок.
    pub debug: bool,
    /// ANSI консоли: auto — терминал, always — также панель, never — чистый текст.
    pub console_colors: crate::colors::ConsoleColors,

    /// TPS и время такта внизу списка игроков Java.
    pub tab_tps: bool,

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

    /// Координаты в углу экрана у игроков Bedrock (правило мира
    /// `showcoordinates`). У Java они и так есть в F3.
    pub bedrock_show_coordinates: bool,

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

    /// Какие проёмы портала в Незер зажигаются: ширина и высота внутри
    /// рамки, от и до (у оригинала от 2×3 до 21×21).
    pub portal_min_width: i32,
    pub portal_min_height: i32,
    pub portal_max_width: i32,
    pub portal_max_height: i32,

    /// Отправлять ли анонимные отчёты об ошибках разработчикам (reports.rs).
    pub send_error_reports: bool,
}

impl Settings {
    /// Ошибка в режиме не должна молча включать обращения к Mojang.
    pub fn validate(&self) -> std::io::Result<()> {
        if self.viaproxy && self.viaproxy_memory < 320 * 1024 * 1024 {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput,
                "config/mcsa.properties: при viaproxy=true требуется viaproxy-memory не менее 320M (по умолчанию 512M)"));
        }
        if !matches!(self.java_version, Some(mcsa_protocol::JavaVersion::V26_1_2 | mcsa_protocol::JavaVersion::V26_3)) {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput,
                format!("config/mcsa.properties: эта сборка поддерживает java-version={}; также доступен новый мир26.3; перенос существующего мира ещё не завершён", crate::network::VERSION_NAME)));
        }
        if self.offline_license_mode == OfflineLicenseMode::Invalid {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput,
                "config/mcsa.properties: offline-license-mode должен быть cosmetics или disabled"));
        }
        Ok(())
    }

    pub fn verify_license(&self, online_mode: bool) -> bool {
        online_mode || self.offline_license_mode == OfflineLicenseMode::Cosmetics
    }

    /// Политика сервера имеет приоритет над источниками в skins.toml.
    pub fn apply_skin_policy(&self, online_mode: bool, skins: &mut crate::skins::Settings) {
        skins.mojang &= self.verify_license(online_mode);
    }

    /// Какие проёмы портала в Незер зажигаются.
    pub fn portal_sizes(&self) -> crate::portal::Sizes {
        crate::portal::Sizes {
            min_width: self.portal_min_width,
            min_height: self.portal_min_height,
            max_width: self.portal_max_width,
            max_height: self.portal_max_height,
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            viaproxy: false,
            viaproxy_memory: 512 * 1024 * 1024,
            java_version: Some(crate::network::NATIVE_VERSION),
            auth_enabled: true,
            auth_premium: true,
            offline_license_mode: OfflineLicenseMode::Cosmetics,
            debug: false,
            console_colors: crate::colors::ConsoleColors::Auto,
            tab_tps: true,
            prefetch_chunks: 3,
            prefetch_at_once: 4,
            bedrock_port: 19132,
            bedrock_online_mode: false,
            bedrock_show_coordinates: true,
            autumn_forests: true,
            pink_cherry_groves: true,
            text_info: true,
            portal_delay: crate::portal::DELAY,
            portal_min_width: crate::portal::VANILLA.min_width,
            portal_min_height: crate::portal::VANILLA.min_height,
            portal_max_width: crate::portal::VANILLA.max_width,
            portal_max_height: crate::portal::VANILLA.max_height,
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
            Ok(text) => {
                let settings = Settings::parse(&text, path);
                settings.add_missing(path, &text);
                settings
            }
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

        if values.contains_key("auth-provider") {
            log_info!("{}: auth-provider больше не нужен — пароли всегда хранятся через Argon2id; настройка игнорируется", path.display());
        }
        Settings {
            viaproxy: take(&values, "viaproxy", default.viaproxy, path),
            viaproxy_memory: values.get("viaproxy-memory").map_or(default.viaproxy_memory, |value| proxy_memory_bytes(value).unwrap_or(0)),
            java_version: values.get("java-version").map_or(default.java_version, |value| mcsa_protocol::JavaVersion::from_name(value)),
            auth_enabled: take(&values, "auth-enabled", default.auth_enabled, path),
            auth_premium: take(&values, "auth-premium", default.auth_premium, path),
            // Старые настройки не должны неожиданно включить проверку входа.
            offline_license_mode: take(&values, "offline-license-mode",
                if take(&values, "auth-premium", true, path) && take(&values, "auth-enabled", true, path) {
                    OfflineLicenseMode::Cosmetics
                } else { OfflineLicenseMode::Disabled }, path),
            debug: take(&values, "debug", default.debug, path),
            console_colors: take(&values, "console-colors", default.console_colors, path),
            tab_tps: take(&values, "tab-tps", default.tab_tps, path),
            prefetch_chunks: take(&values, "prefetch-chunks", default.prefetch_chunks, path),
            prefetch_at_once: take(&values, "prefetch-at-once", default.prefetch_at_once, path),
            bedrock_port: take(&values, "bedrock-port", default.bedrock_port, path),
            bedrock_online_mode: take(&values, "bedrock-online-mode", default.bedrock_online_mode, path),
            bedrock_show_coordinates: take(&values, "bedrock-show-coordinates", default.bedrock_show_coordinates, path),
            autumn_forests: take(&values, "autumn-forests", default.autumn_forests, path),
            pink_cherry_groves: take(&values, "pink-cherry-groves", default.pink_cherry_groves, path),
            text_info: take(&values, "text-info", default.text_info, path),
            portal_delay: take(&values, "portal-delay", default.portal_delay, path),
            portal_min_width: take(&values, "portal-min-width", default.portal_min_width, path),
            portal_min_height: take(&values, "portal-min-height", default.portal_min_height, path),
            portal_max_width: take(&values, "portal-max-width", default.portal_max_width, path),
            portal_max_height: take(&values, "portal-max-height", default.portal_max_height, path),
            send_error_reports: take(&values, "send-error-reports", default.send_error_reports, path),
        }
    }

    /// Файл с пояснениями — чтобы было понятно, что править.
    fn to_file(self) -> String {
        let mut text = String::from(HEADER);

        for (_, section) in self.sections() {
            text.push('\n');
            text.push_str(&section);
        }

        text
    }

    /// Каждая настройка с пояснением над ней — по порядку файла. Ключ идёт
    /// отдельно: по нему видно, какой настройки в файле ещё нет.
    fn sections(self) -> [(&'static str, String); 23] {
        [
            ("viaproxy", format!("# Настройка внешнего ViaProxy; запуск в этой сборке не реализован.\nviaproxy={}\n", self.viaproxy)),
            ("viaproxy-memory", format!("# Память ViaProxy: минимум 320M при viaproxy=true; по умолчанию 512M.\nviaproxy-memory={}\n", if self.viaproxy_memory.is_multiple_of(1024 * 1024) { format!("{}M", self.viaproxy_memory / (1024 * 1024)) } else { self.viaproxy_memory.to_string() })),
            ("java-version", format!("# Основная версия Java. Менять только на поддерживаемую этой сборкой версию.\n# Поддерживаются 26.1.2 и новые миры26.3. Перед сменой версии\n# сохраните резервную копию мира: новые данные могут не читаться старым ядром.\njava-version={}\n", self.java_version.map_or("invalid", mcsa_protocol::JavaVersion::name))),
            ("console-colors", format!("# Цвета консоли: auto — только терминал (учитывает NO_COLOR/TERM=dumb).\n# always — включить также в панели хостинга; never — отключить ANSI.\n# Цвета чата &a, &l, &r и &#RRGGBB работают независимо от этой настройки.\nconsole-colors={}\n", self.console_colors)),
            ("auth-enabled", format!("# Парольный диалог Java в offline-mode, до входа в мир; БД auth.db.\nauth-enabled={}\n", self.auth_enabled)),
            ("auth-premium", format!("# Устаревшая настройка; используется только при отсутствии offline-license-mode.\nauth-premium={}\n", self.auth_premium)),
            ("offline-license-mode", format!("# Только для online-mode=false: cosmetics — проверять лицензию для скинов/плащей;\n# после неудачи можно войти повторно без лицензии. disabled — не проверять\n# лицензию и не искать профили/скины через Mojang. Парольная защита AuthMe независима.\n# Имеет приоритет над auth-premium; действует и при auth-enabled=false.\noffline-license-mode={}\n", self.offline_license_mode)),
            ("tab-tps", format!("# TPS и время такта внизу списка игроков Java; оформление плагина имеет приоритет.\ntab-tps={}\n", self.tab_tps)),
            ("debug", format!("\
# Подробный лог: каждый пакет, каждое движение игрока, служебный обмен
# с клиентом. Нужен только для поиска неполадок; в обычной игре — false.
debug={}
", self.debug)),
            ("prefetch-chunks", format!("\
# Складывание мира впрок: на сколько чанков дальше прорисовки игрока мир
# готовится заранее (клиенту эти чанки не шлются, но держатся в памяти).
# Когда игрок бежит, новые чанки уже готовы и отправка не ждёт генерации.
# 0 — не складывать впрок.
prefetch-chunks={}
", self.prefetch_chunks)),
            ("prefetch-at-once", format!("\
# Сколько чанков одного игрока складывается впрок одновременно.
prefetch-at-once={}
", self.prefetch_at_once)),
            ("bedrock-port", format!("\
# UDP-порт для игроков Bedrock Edition (телефоны, консоли, Windows).
# 0 — не пускать Bedrock.
bedrock-port={}
", self.bedrock_port)),
            ("bedrock-online-mode", format!("\
# Проверка входа Xbox Live у игроков Bedrock. Проверяется всегда: кто вошёл
# с подтверждённой учётной записью, тот получает права по ops.json.
# true — пускать только подтверждённых (как online-mode у Java);
# false — пускать всех, неподтверждённым — только команды для всех.
bedrock-online-mode={}
", self.bedrock_online_mode)),
            ("bedrock-show-coordinates", format!("\
# Координаты в углу экрана у игроков Bedrock (у Java они есть в F3).
bedrock-show-coordinates={}
", self.bedrock_show_coordinates)),
            ("autumn-forests", format!("\
# Дополнения к миру, которых нет у оригинала (false — мир как у оригинала).
# Редкие осенние пятна в обычном лесу: рыжая листва и пожухлая трава.
autumn-forests={}
", self.autumn_forests)),
            ("pink-cherry-groves", format!("\
# Вишнёвые рощи с розоватой травой и чуть большим числом лепестков.
pink-cherry-groves={}
", self.pink_cherry_groves)),
            ("text-info", format!("\
# Справка о сервере для тех, кто подключился не игрой: `telnet адрес порт`,
# браузер или curl. Сервер ждёт секунду: клиент Minecraft за это время
# говорит первым, а молчащему отвечают справкой (версии, порты, игроки).
text-info={}
", self.text_info)),
            ("portal-delay", format!("\
# Сколько тактов (20 в секунду) стоять в портале, чтобы перейти в другое
# измерение, — в выживании. У оригинала 80 (4 секунды); 1 — сразу.
# В творческом переход всегда сразу.
portal-delay={}
", self.portal_delay)),
            ("portal-min-width", format!("\
# Какие проёмы портала в Незер зажигаются: ширина и высота внутри рамки из
# обсидиана, в блоках, от и до. У оригинала от 2×3 до 21×21. Каждая сторона
# — от 1 до 256; огромный проём заполняется порталом за один такт.
# Наименьшая ширина (у оригинала 2):
portal-min-width={}
", self.portal_min_width)),
            ("portal-min-height", format!("\
# Наименьшая высота проёма портала (у оригинала 3).
portal-min-height={}
", self.portal_min_height)),
            ("portal-max-width", format!("\
# Наибольшая ширина проёма портала (у оригинала 21).
portal-max-width={}
", self.portal_max_width)),
            ("portal-max-height", format!("\
# Наибольшая высота проёма портала (у оригинала 21).
portal-max-height={}
", self.portal_max_height)),
            ("send-error-reports", format!("\
# Анонимные отчёты об ошибках разработчикам MCSheriffAnya (падения и строки
# ERROR с последними строками лога). Ники, адреса, UUID, XUID и чат из них
# вычищаются; вместо адреса сервера — случайный номер (config/install-id).
# Копия каждого отчёта остаётся в logs/reports. false — ничего не отправлять.
send-error-reports={}
", self.send_error_reports)),
        ]
    }

    /// Дописывает в конец файла настройки, которых в нём нет (файл положен
    /// прежней версией сервера), — с пояснениями и значениями по умолчанию.
    /// Остальное в файле не трогается. Не записалось — не беда: настройки
    /// всё равно берутся по умолчанию.
    fn add_missing(self, path: &Path, text: &str) {
        let values = parse_properties(text);
        let missing: Vec<_> = self.sections().into_iter().filter(|(key, _)| !values.contains_key(*key)).collect();

        if missing.is_empty() {
            return;
        }

        let mut text = text.to_string();
        let keys: Vec<_> = missing.iter().map(|(key, _)| *key).collect();

        for (_, section) in missing {
            if !text.is_empty() && !text.ends_with("\n\n") {
                text.push_str(if text.ends_with('\n') { "\n" } else { "\n\n" });
            }

            text.push_str(&section);
        }

        match fs::write(path, text) {
            Ok(()) => log_info!("{}: дописаны новые настройки: {}", path.display(), keys.join(", ")),
            Err(error) => log_warn!("{}: новые настройки не дописаны: {}", path.display(), error),
        }
    }
}

fn proxy_memory_bytes(value: &str) -> Option<u64> {
    let value = value.trim();
    let (number, multiplier) = match value.as_bytes().last()? {
        b'k' | b'K' => (&value[..value.len()-1], 1024),
        b'm' | b'M' => (&value[..value.len()-1], 1024 * 1024),
        b'g' | b'G' => (&value[..value.len()-1], 1024 * 1024 * 1024),
        _ => (value, 1),
    };
    number.parse::<u64>().ok()?.checked_mul(multiplier)
}

fn read_viaproxy_memory<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
    let value = String::deserialize(deserializer)?;
    Ok(proxy_memory_bytes(&value).unwrap_or(0))
}

fn read_java_version<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<mcsa_protocol::JavaVersion>, D::Error> {
    let value = String::deserialize(deserializer)?;
    Ok(mcsa_protocol::JavaVersion::from_name(&value))
}

/// Шапка файла.
const HEADER: &str = "# Настройки MCSheriffAnya, которых нет в server.properties.\n";

/// Имя файла — без папки: прежние файлы лежат там же, где и новый.
fn file_name(path: &str) -> &str {
    Path::new(path).file_name().and_then(|name| name.to_str()).unwrap_or(path)
}

/// Читает прежний toml-файл и убирает его: его значения уйдут в новый файл.
/// Не разобрался — значения по умолчанию, а старый файл остаётся лежать.
fn move_old_file(old: &Path, path: &Path) -> Settings {
    let text = fs::read_to_string(old).unwrap_or_default();

    match toml::from_str::<Settings>(&text) {
        Ok(mut settings) => {
            if !text.lines().any(|line| line.trim_start().starts_with("offline_license_mode")) {
                settings.offline_license_mode = if settings.auth_enabled && settings.auth_premium {
                    OfflineLicenseMode::Cosmetics
                } else { OfflineLicenseMode::Disabled };
            }
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
    #[test]
    fn viaproxy_heap_minimum_is_checked_before_startup() {
        let path = std::path::Path::new("mcsa.properties");
        for value in ["0", "128M", "319m", "335544319", "bad", "18446744073709551615G"] {
            let settings = super::Settings::parse(&format!("viaproxy=true\nviaproxy-memory={value}\n"), path);
            let error = settings.validate().unwrap_err();
            assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
            assert!(error.to_string().contains("не менее 320M"));
            assert!(super::Settings { viaproxy: false, ..settings }.validate().is_ok());
        }
        for value in ["320M", "327680K", "335544320", "512M", "1g"] {
            let settings = super::Settings::parse(&format!("viaproxy=true\nviaproxy-memory={value}\n"), path);
            assert!(settings.validate().is_ok());
            assert_eq!(super::Settings::parse(&settings.to_file(), path), settings);
        }
        let defaults = super::Settings::parse("viaproxy=true\n", path);
        assert_eq!(defaults.viaproxy_memory, 512 * 1024 * 1024);
        assert!(defaults.validate().is_ok());
    }
    #[test]
    fn auth_settings_default_and_round_trip() {
        let defaults = super::Settings::default();
        assert!(defaults.auth_enabled && defaults.auth_premium);
        let path = std::path::Path::new("config/mcsa.properties");
        let changed = super::Settings::parse("auth-enabled=false\nauth-premium=false\n", path);
        assert!(!changed.auth_enabled && !changed.auth_premium);
        assert_eq!(super::Settings::parse(&changed.to_file(), path), changed);
        // Старая настройка auth-provider игнорируется и не попадает в файл.
        let old=super::Settings::parse("auth-provider=guard\n",path);
        assert_eq!(old,defaults);
        assert!(!old.to_file().contains("auth-provider"));
    }
    use super::*;

    #[test]
    fn primary_version_never_enables_an_unimplemented_adapter() {
        let path = Path::new("mcsa.properties");
        for version in ["1.21.11", "26.1.2", "26.2", "26.3", "unknown", ""] {
            let settings = Settings::parse(&format!("java-version={version}"), path);
            assert_eq!(settings.validate().is_ok(), matches!(version, "26.1.2" | "26.3"));
            assert_eq!(Settings::parse(&settings.to_file(), path), settings);
        }
        assert!(Settings::parse("", path).validate().is_ok());
    }

    #[test]
    fn offline_license_policy_and_legacy_migration() {
        let path = Path::new("mcsa.properties");
        for (text, expected) in [
            ("", OfflineLicenseMode::Cosmetics),
            ("auth-premium=false", OfflineLicenseMode::Disabled),
            ("auth-enabled=false", OfflineLicenseMode::Disabled),
            ("auth-premium=true\noffline-license-mode=disabled", OfflineLicenseMode::Disabled),
            ("auth-enabled=false\nauth-premium=false\noffline-license-mode=cosmetics", OfflineLicenseMode::Cosmetics),
        ] {
            let settings = Settings::parse(text, path);
            assert_eq!(settings.offline_license_mode, expected);
            assert_eq!(Settings::parse(&settings.to_file(), path), settings);
            assert!(settings.validate().is_ok());
            assert_eq!(settings.verify_license(false), expected == OfflineLicenseMode::Cosmetics);
            assert!(settings.verify_license(true), "online-mode всегда требует лицензию");
            let mut skins = crate::skins::Settings::default();
            settings.apply_skin_policy(false, &mut skins);
            assert_eq!(skins.mojang, expected == OfflineLicenseMode::Cosmetics);
            assert!(skins.ely, "Ely не проверяет лицензию Mojang");
            skins.mojang = false;
            settings.apply_skin_policy(true, &mut skins);
            assert!(!skins.mojang, "не включать отключённый владельцем источник");
        }
        let invalid = Settings::parse("offline-license-mode=disabeld", path);
        assert!(invalid.validate().is_err());
        assert!(!invalid.verify_license(false));
        let protected = Settings::parse("auth-enabled=true\noffline-license-mode=disabled", path);
        assert!(protected.auth_enabled);
    }

    #[test]
    fn offline_license_mode_survives_config_reload() {
        let directory = std::env::temp_dir().join(format!("mcsa-offline-mode-{}", std::process::id()));
        fs::create_dir_all(&directory).unwrap();
        let path = directory.join("mcsa.properties");
        fs::write(&path, "# Старый сервер\nauth-premium=false\n").unwrap();
        let settings = Settings::load(&path);
        assert_eq!(settings.offline_license_mode, OfflineLicenseMode::Disabled);
        let saved = fs::read_to_string(&path).unwrap();
        assert!(saved.starts_with("# Старый сервер\nauth-premium=false\n"));
        assert!(saved.contains("offline-license-mode=disabled"));
        assert_eq!(Settings::load(&path), settings);
        fs::remove_file(&path).unwrap();
        fs::write(directory.join("rustcraft.toml"), "auth_premium = false\n").unwrap();
        let migrated = Settings::load(&path);
        assert_eq!(migrated.offline_license_mode, OfflineLicenseMode::Disabled);
        assert_eq!(Settings::load(&path), migrated);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn console_color_modes_survive_properties_and_legacy_toml() {
        use crate::colors::ConsoleColors;
        let path = Path::new("config/mcsa.properties");
        assert_eq!(Settings::parse("", path).console_colors, ConsoleColors::Auto);
        assert_eq!(Settings::parse("console-colors=always", path).console_colors, ConsoleColors::Always);
        assert_eq!(Settings::parse("console-colors=never", path).console_colors, ConsoleColors::Never);
        assert_eq!(Settings::parse("console-colors=typo", path).console_colors, ConsoleColors::Auto);
        assert_eq!(toml::from_str::<Settings>("console_colors = 'always'").unwrap().console_colors, ConsoleColors::Always);
        assert!(Settings::default().to_file().contains("console-colors=auto"));
    }

    #[test]
    fn tab_tps_defaults_and_explicit_disable() {
        let path = Path::new("config/mcsa.properties");
        assert!(Settings::parse("", path).tab_tps);
        assert!(!Settings::parse("tab-tps=false", path).tab_tps);
        assert!(Settings::default().to_file().contains("tab-tps=true"));
    }

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

    /// Файл от прежней версии: недостающие настройки дописываются в конец,
    /// а свои значения и пометки остаются как были.
    #[test]
    fn missing_settings_are_added_to_an_old_file() {
        let directory = std::env::temp_dir().join(format!("mcsheriffanya-missing-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("папка");
        let path = directory.join("mcsa.properties");

        fs::write(&path, "# своя пометка\ndebug=true\nbedrock-port=19133\n").expect("пишется");
        let settings = Settings::load(&path);
        assert!(settings.debug);
        assert_eq!(settings.bedrock_port, 19133);

        let text = fs::read_to_string(&path).expect("читается");
        assert!(text.starts_with("# своя пометка\ndebug=true\nbedrock-port=19133\n"), "прежнее содержимое изменилось");
        let values = parse_properties(&text);
        for (key, _) in Settings::default().sections() {
            assert!(values.contains_key(key), "{} не дописан", key);
        }
        assert_eq!(text.matches("bedrock-port=").count(), 1, "имеющаяся настройка дописана второй раз");

        // Второй запуск уже ничего не меняет.
        Settings::load(&path);
        assert_eq!(fs::read_to_string(&path).expect("читается"), text);

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
