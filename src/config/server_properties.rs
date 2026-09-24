// Загрузка и парсинг файла config/server.properties.
//
// Формат файла — key=value, по одной паре на строку.
// Строки, начинающиеся с '#', и пустые строки считаются комментариями
// и игнорируются (как в оригинальном server.properties).

use std::collections::HashMap;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use crate::{log_info, log_warn};

/// Настройки сервера, загружаемые из server.properties.
#[derive(Debug, Clone)]
pub struct ServerProperties {
    pub motd: String,
    pub max_players: i32,
    pub server_port: u16,

    /// Сколько чанков вокруг себя видит игрок. Считается в каждую сторону:
    /// при десяти игроку уходит квадрат 21×21.
    pub view_distance: i32,

    /// На сколько чанков вокруг игрока мир живёт: течёт вода, работает
    /// редстоун. Дальше него ничего не пересчитывается.
    pub simulation_distance: i32,

    /// Семя мира из настроек. Пусто — мир получит случайное семя, а число
    /// или слово задают его наверняка. У уже сложенного мира своё семя
    /// записано в level.dat, и настройка его не перебивает.
    pub level_seed: Option<i64>,

    /// Тип мира — настройка `level-type`, как у оригинала, плюс наш.
    pub world_kind: WorldKind,
}

impl Default for ServerProperties {
    fn default() -> Self {
        ServerProperties {
            motd: "My Rust Server".to_string(),
            max_players: 20,
            server_port: 25565,
            // Значения по умолчанию — те же, что у оригинала.
            view_distance: 10,
            simulation_distance: 10,
            level_seed: None,
            world_kind: WorldKind::Normal,
        }
    }
}

/// Загружает настройки из файла по указанному пути.
///
/// Если файл не существует, он создаётся со значениями по умолчанию,
/// после чего эти значения и возвращаются вызывающему коду.
pub fn load_server_properties(path: &str) -> io::Result<ServerProperties> {
    let file_path = Path::new(path);

    if !file_path.exists() {
        create_default_properties(file_path)?;
    }

    let content = fs::read_to_string(file_path)?;
    point_to_own_settings(file_path, &content);
    let values = parse_properties(&content);
    let defaults = ServerProperties::default();

    let motd = values.get("motd").cloned().unwrap_or(defaults.motd);

    let max_players = values
        .get("max-players")
        .and_then(|v| v.parse::<i32>().ok())
        .unwrap_or(defaults.max_players);

    let server_port = values
        .get("server-port")
        .and_then(|v| v.parse::<u16>().ok())
        .unwrap_or(defaults.server_port);

    // Допустимые значения дальностей — от 3 до 32, как у оригинала:
    // меньше трёх клиент не примет, больше тридцати двух незачем.
    let view_distance = distance(&values, "view-distance", defaults.view_distance);
    let simulation_distance = distance(
        &values,
        "simulation-distance",
        defaults.simulation_distance,
    );

    let level_seed = values.get("level-seed").and_then(|value| seed_of(value));

    let world_kind = values
        .get("level-type")
        .map(|value| WorldKind::from_setting(value))
        .unwrap_or(WorldKind::Normal);

    Ok(ServerProperties {
        motd,
        max_players,
        server_port,
        view_distance,
        simulation_distance,
        level_seed,
        world_kind,
    })
}

/// Тип мира.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum WorldKind {
    /// Обычный — `minecraft:normal`, рельеф как у оригинала.
    Normal,
    /// Суперплоский — `minecraft:flat`.
    Flat,
    /// «Супер плавность» — `mcsheriffanya:super_smooth`, наш мягкий рельеф.
    SuperSmooth,
}

impl WorldKind {
    /// Разбирает значение `level-type`. В файле двоеточие принято закрывать
    /// косой чертой, поэтому её убираем. Типы, которых у нас ещё нет
    /// (большие биомы, расширенный), пока складываются обычным миром.
    pub fn from_setting(value: &str) -> WorldKind {
        let value = value.replace('\\', "");
        let value = value.trim().to_ascii_lowercase();
        let name = value.strip_prefix("minecraft:").unwrap_or(&value);

        match name {
            "flat" => WorldKind::Flat,
            "mcsheriffanya:super_smooth" | "super_smooth" => WorldKind::SuperSmooth,
            _ => WorldKind::Normal,
        }
    }

    /// Как тип записывается в level.dat и в настройки.
    pub fn name(self) -> &'static str {
        match self {
            WorldKind::Normal => "minecraft:normal",
            WorldKind::Flat => "minecraft:flat",
            WorldKind::SuperSmooth => "mcsheriffanya:super_smooth",
        }
    }
}

/// Семя мира из настройки: число берётся как есть, слово превращается
/// в число — так же, как в игре, где семенем может быть любая строка.
/// Пустая настройка означает «любое»: мир получит случайное семя.
fn seed_of(value: &str) -> Option<i64> {
    let value = value.trim();

    if value.is_empty() {
        return None;
    }

    if let Ok(number) = value.parse::<i64>() {
        return Some(number);
    }

    // Простая свёртка строки в число: важно лишь, чтобы одно и то же слово
    // всегда давало один и тот же мир.
    let mut hash: i64 = 0;

    for byte in value.bytes() {
        hash = hash.wrapping_mul(31).wrapping_add(byte as i64);
    }

    Some(hash)
}

/// Дальность в чанках из настроек: число от 3 до 32.
///
/// Всё, что вне этих границ или вовсе не число, заменяется значением
/// по умолчанию: лучше работать, чем падать из-за опечатки в настройках.
fn distance(values: &HashMap<String, String>, key: &str, default: i32) -> i32 {
    const LEAST: i32 = 3;
    const MOST: i32 = 32;

    values
        .get(key)
        .and_then(|value| value.parse::<i32>().ok())
        .filter(|distance| (LEAST..=MOST).contains(distance))
        .unwrap_or(default)
}

/// Разбирает содержимое файла в формате key=value в HashMap.
/// Пустые строки и строки, начинающиеся с '#', пропускаются.
pub(crate) fn parse_properties(content: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if let Some((key, value)) = trimmed.split_once('=') {
            map.insert(key.trim().to_string(), value.trim().to_string());
        }
    }

    map
}

/// Все настройки файла по порядку — так же, как их пишет оригинальный
/// сервер: по алфавиту, каждая со значением по умолчанию.
///
/// Список взят с minecraft.wiki (страница Server.properties, раздел «Default
/// content»). Мы пишем его целиком, чтобы файл выглядел привычно и его можно
/// было править теми же средствами, что и обычный. Но честно: сервер пока
/// слушается лишь части настроек — какие именно, сказано в примечании
/// в самом файле.
const DEFAULT_PROPERTIES: &[(&str, &str)] = &[
    ("accepts-transfers", "false"),
    ("allow-flight", "false"),
    ("broadcast-console-to-ops", "true"),
    ("broadcast-rcon-to-ops", "true"),
    ("bug-report-link", ""),
    ("chat-spam-threshold-seconds", "10"),
    ("command-spam-threshold-seconds", "10"),
    ("difficulty", "easy"),
    ("enable-code-of-conduct", "false"),
    ("enable-jmx-monitoring", "false"),
    ("enable-query", "false"),
    ("enable-rcon", "false"),
    ("enable-status", "true"),
    ("enforce-secure-profile", "true"),
    ("enforce-whitelist", "false"),
    ("entity-broadcast-range-percentage", "100"),
    ("force-gamemode", "false"),
    ("function-permission-level", "2"),
    ("gamemode", "survival"),
    ("generate-structures", "true"),
    ("generator-settings", "{}"),
    ("hardcore", "false"),
    ("hide-online-players", "false"),
    ("initial-disabled-packs", ""),
    ("initial-enabled-packs", "vanilla"),
    ("level-name", "world"),
    ("level-seed", ""),
    ("level-type", "minecraft\\:normal"),
    ("log-ips", "true"),
    ("max-chained-neighbor-updates", "1000000"),
    ("max-players", "20"),
    ("max-tick-time", "60000"),
    ("max-world-size", "29999984"),
    // Приветствие своё: выдавать себя за чужой сервер незачем.
    ("motd", "MCSheriffAnya Server"),
    ("network-compression-threshold", "256"),
    ("online-mode", "true"),
    ("op-permission-level", "4"),
    ("pause-when-empty-seconds", "60"),
    ("player-idle-timeout", "0"),
    ("prevent-proxy-connections", "false"),
    ("query.port", "25565"),
    ("rate-limit", "0"),
    ("rcon.password", ""),
    ("rcon.port", "25575"),
    ("region-file-compression", "deflate"),
    ("require-resource-pack", "false"),
    ("resource-pack", ""),
    ("resource-pack-id", ""),
    ("resource-pack-prompt", ""),
    ("resource-pack-sha1", ""),
    ("server-ip", ""),
    ("server-port", "25565"),
    ("simulation-distance", "10"),
    ("spawn-protection", "16"),
    ("status-heartbeat-interval", "0"),
    ("sync-chunk-writes", "true"),
    ("text-filtering-config", ""),
    ("text-filtering-version", "0"),
    ("use-native-transport", "true"),
    ("view-distance", "10"),
    ("white-list", "false"),
];

/// Настройки, которые сервер сейчас действительно слушается.
///
/// Всё остальное в файле лежит для вида: имена и значения по умолчанию
/// настоящие, но поведение за ними пока не стоит.
const HONOURED: &[&str] = &[
    "motd",
    "max-players",
    "server-port",
    "view-distance",
    "simulation-distance",
];

/// Создаёт файл server.properties со значениями по умолчанию
/// (используется, если файл ещё не существует).
fn create_default_properties(path: &Path) -> io::Result<()> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)?;
    }

    let mut content = format!(
        "#MCSheriffAnya server properties\n\
         #{}\n\
         #Названия и значения по умолчанию — как у оригинального сервера.\n\
         #Сервер пока слушается этих настроек: {}.\n\
         #Остальные лежат для вида: поведения за ними ещё нет.\n\
         {}\n",
        written_at(),
        HONOURED.join(", "),
        OWN_SETTINGS_NOTE
    );

    for (key, value) in DEFAULT_PROPERTIES {
        content.push_str(&format!("{}={}\n", key, value));
    }

    let mut file = fs::File::create(path)?;
    file.write_all(content.as_bytes())?;

    log_info!("Создан файл настроек по умолчанию: {}", path.display());

    Ok(())
}

/// Строка-пояснение: где лежат настройки, которых у оригинала нет.
const OWN_SETTINGS_NOTE: &str = "#Дополнительные настройки MCSheriffAnya (которых нет у оригинала) — в mcsa.properties рядом.";

/// Дописывает пояснение про mcsa.properties в файл, созданный до того, как
/// оно появилось: сразу после вводных комментариев, ничего не меняя в самих
/// настройках. Не записалось — не беда, файл читается и так.
fn point_to_own_settings(path: &Path, content: &str) {
    if content.contains("mcsa.properties") {
        return;
    }

    let head = content.lines().take_while(|line| line.starts_with('#')).count();
    let mut lines: Vec<&str> = content.lines().collect();
    lines.insert(head, OWN_SETTINGS_NOTE);
    let mut text = lines.join("\n");
    text.push('\n');

    if let Err(error) = fs::write(path, text) {
        log_warn!("{}: пояснение про mcsa.properties не дописано: {}", path.display(), error);
    }
}

/// Время создания файла — в том же виде, в каком его пишет оригинальный
/// сервер: «Сб сен 20 10:05:00 MSK 2026», только по-английски.
fn written_at() -> String {
    /// Сколько байт хватит на строку времени.
    const ROOM: usize = 64;

    let seconds = match std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
    {
        Ok(passed) => passed.as_secs() as libc::time_t,
        Err(_) => return String::new(),
    };

    let mut broken_down: libc::tm = unsafe { std::mem::zeroed() };

    // SAFETY: система пишет в переданную структуру и нигде её не сохраняет.
    if unsafe { libc::localtime_r(&seconds, &mut broken_down) }.is_null() {
        return String::new();
    }

    let mut text = [0u8; ROOM];
    let format = c"%a %b %d %H:%M:%S %Z %Y";

    // SAFETY: система пишет не больше ROOM байт в наш же буфер.
    let written = unsafe {
        libc::strftime(
            text.as_mut_ptr() as *mut libc::c_char,
            ROOM,
            format.as_ptr(),
            &broken_down,
        )
    };

    String::from_utf8_lossy(&text[..written]).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// В новом файле и в старом, где пояснения ещё не было, есть строка про
    /// mcsa.properties; сами настройки при этом не меняются.
    #[test]
    fn the_file_points_to_our_own_settings() {
        let directory = std::env::temp_dir().join(format!("mcsheriffanya-props-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).expect("папка");

        let fresh = directory.join("fresh.properties");
        load_server_properties(fresh.to_str().expect("путь")).expect("читается");
        assert!(fs::read_to_string(&fresh).expect("есть").contains("mcsa.properties"));

        let old = directory.join("old.properties");
        fs::write(&old, "#старый заголовок\n#ещё строка\nmotd=Мой\nview-distance=12\n").expect("пишется");
        let properties = load_server_properties(old.to_str().expect("путь")).expect("читается");
        let text = fs::read_to_string(&old).expect("есть");

        assert_eq!(properties.motd, "Мой");
        assert_eq!(properties.view_distance, 12);
        assert_eq!(text.lines().nth(2), Some(OWN_SETTINGS_NOTE), "{}", text);
        assert!(text.ends_with("view-distance=12\n"), "{}", text);

        // Второй раз пояснение не дописывается.
        load_server_properties(old.to_str().expect("путь")).expect("читается");
        assert_eq!(fs::read_to_string(&old).expect("есть"), text);

        let _ = fs::remove_dir_all(&directory);
    }

    /// Дальности читаются из файла, а негодные значения заменяются
    /// значением по умолчанию: опечатка в настройках не должна ронять сервер.
    #[test]
    fn distances_come_from_the_file() {
        let read = |text: &str| {
            let values = parse_properties(text);

            (
                distance(&values, "view-distance", 10),
                distance(&values, "simulation-distance", 10),
            )
        };

        assert_eq!(read("view-distance=8\nsimulation-distance=6"), (8, 6));

        // Слишком мало, слишком много и вовсе не число.
        assert_eq!(read("view-distance=1"), (10, 10));
        assert_eq!(read("view-distance=100"), (10, 10));
        assert_eq!(read("view-distance=далеко"), (10, 10));

        // Настройки нет вовсе — берётся значение по умолчанию, как у игры.
        assert_eq!(read("motd=привет"), (10, 10));
    }

    /// Строки с решёткой — примечания, и они не попадают в настройки.
    #[test]
    fn comments_are_skipped() {
        let values = parse_properties("# view-distance=3\nview-distance=12\n");

        assert_eq!(distance(&values, "view-distance", 10), 12);
    }

    /// Файл по умолчанию выглядит как оригинальный: те же названия настроек,
    /// по алфавиту, и он читается обратно.
    #[test]
    fn the_default_file_looks_like_the_original() {
        let path = std::env::temp_dir().join("mcsheriffanya_props_default/server.properties");
        let _ = fs::remove_dir_all(path.parent().expect("есть директория"));

        let properties = load_server_properties(path.to_str().expect("путь из букв"))
            .expect("создать и прочитать");

        let text = fs::read_to_string(&path).expect("файл на месте");

        // Настройки идут по алфавиту — как их пишет оригинал.
        let keys: Vec<&str> = DEFAULT_PROPERTIES.iter().map(|(key, _)| *key).collect();
        let mut sorted = keys.clone();
        sorted.sort();

        assert_eq!(keys, sorted, "настройки не по алфавиту");

        // Все они попали в файл.
        for (key, _) in DEFAULT_PROPERTIES {
            assert!(text.contains(&format!("\n{}=", key)), "нет настройки {}", key);
        }

        // Первая строка — примечание, как и у оригинала.
        assert!(text.starts_with('#'));

        // И прочитанное совпадает со значениями по умолчанию из файла.
        assert_eq!(properties.max_players, 20);
        assert_eq!(properties.server_port, 25565);
        assert_eq!(properties.view_distance, 10);
        assert_eq!(properties.simulation_distance, 10);
    }

    /// Настройки, которые сервер слушается, есть и в самом файле: иначе
    /// в примечании стояло бы обещание, которого файл не выполняет.
    #[test]
    fn honoured_settings_are_in_the_file() {
        for key in HONOURED {
            assert!(
                DEFAULT_PROPERTIES.iter().any(|(name, _)| name == key),
                "настройки {} нет в файле",
                key
            );
        }
    }
}
