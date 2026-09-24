// Скины игроков.
//
// Скин — это картинка, натянутая на игрока. Сам файл картинки сервер не
// раздаёт: клиент забирает её по ссылке, а сервер лишь пересказывает, где она
// лежит. Этот пересказ — «свойство профиля» с именем textures: строка с
// ссылкой и подпись Mojang к ней. Клиент показывает скин, только если подпись
// на месте и сходится, поэтому выдумать своё описание скина нельзя — его
// можно только взять у Mojang.
//
// Отсюда и устройство: по нику сервер спрашивает, чей это игрок и какой у него
// скин, и запоминает ответ на диске. Источников два, и опрашиваются они по
// порядку: сперва Mojang, потом Ely.by — у кого нашлось, того и берём. Каждый
// можно выключить в config/skins.toml. Вторая половина — своя система по
// никам: в файле skins/names.txt рядом с ником пишется, чей скин ему выдать.
// Так скин получает и тот, кого нет ни у кого.
//
// Про Ely.by надо знать одно: ссылка на картинку там ведёт на их сервер, а
// обычный клиент загружает картинки только с серверов Mojang. Поэтому скин
// с Ely.by увидят те, у кого клиент это умеет (лаунчеры Ely.by и им подобные),
// остальные — стандартный. Скин с Mojang видят все.
//
// Ходить к Mojang за каждым заходом не нужно и вредно: у них стоит ограничение
// на число запросов. Поэтому ответ лежит в skins/<ник>.skin и перечитывается
// раз в сутки. Формат файла свой, простой: подпись, время и две строки.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Deserialize;

use crate::{log_info, log_warn};

/// Директория со скинами.
pub const DIRECTORY: &str = "skins";

/// Файл настроек: какие источники скинов включены.
pub const SETTINGS_FILE: &str = "config/skins.toml";

/// Откуда брать скины. Порядок опроса всегда один: сперва Mojang, потом
/// Ely.by — кто нашёл, того и берём.
#[derive(Clone, Copy, Debug, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// Спрашивать ли Mojang.
    pub mojang: bool,

    /// Спрашивать ли Ely.by, если у Mojang не нашлось.
    pub ely: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            mojang: true,
            ely: true,
        }
    }
}

/// Образец файла настроек — с пояснениями, чтобы было понятно, что править.
const SETTINGS_SAMPLE: &str = "\
# Откуда брать скины игроков.
#
# Источники опрашиваются по порядку: сперва Mojang, потом Ely.by. У кого
# скин нашёлся, того и берём. Каждый источник можно выключить.
#
# Про Ely.by: обычный клиент загружает картинки только с серверов Mojang,
# поэтому скин с Ely.by увидят лишь те, у кого клиент это умеет (лаунчеры
# Ely.by и подобные). Скин с Mojang видят все.

mojang = true
ely = true
";

impl Settings {
    /// Читает настройки; нет файла — кладёт образец и берёт значения
    /// по умолчанию. Испорченный файл — тоже значения по умолчанию,
    /// с предупреждением в логе.
    pub fn load(path: &Path) -> Self {
        match fs::read_to_string(path) {
            Ok(text) => match toml::from_str::<Settings>(&text) {
                Ok(settings) => settings,
                Err(error) => {
                    log_warn!("Скины: {} не разобрался ({}) — беру всё по умолчанию", path.display(), error);
                    Settings::default()
                }
            },
            Err(_) => {
                if let Err(error) = fs::write(path, SETTINGS_SAMPLE) {
                    log_warn!("Скины: {} не записан: {}", path.display(), error);
                }

                Settings::default()
            }
        }
    }
}

/// Файл своей системы: какому нику чей скин выдавать.
const NAMES_FILE: &str = "names.txt";

/// Подпись файла со скином: по ней сервер понимает, что файл его.
const MAGIC: &str = "MCSK";

/// Версия формата файла.
const FORMAT_VERSION: u32 = 1;

/// Сколько запомненный скин считается свежим: сутки. Игрок мог сменить скин,
/// и вечно показывать старый неправильно.
const FRESH: u64 = 24 * 60 * 60;

/// Сколько ждать ответа одного запроса. Дольше держать игрока на входе
/// нельзя: он ждёт появления в мире, а скин — дело десятое. У Mojang
/// запросов два подряд, так что худший случай — вдвое больше.
const WAIT: Duration = Duration::from_secs(2);

/// Описание скина в том виде, в каком его понимает клиент.
#[derive(Clone, PartialEq, Debug)]
pub struct Skin {
    /// Описание с ссылкой на картинку.
    pub value: String,

    /// Подпись Mojang к описанию.
    pub signature: String,
}

/// Кладёт образец таблицы ников, если её ещё нет.
///
/// Пустая директория ничего о себе не рассказывает, а так сразу видно, что
/// в неё писать.
pub fn prepare(dir: &Path) {
    let path = dir.join(NAMES_FILE);

    if path.exists() {
        return;
    }

    let sample = "\
# Кому какой скин выдавать.
#
# Слева ник игрока на этом сервере, справа — ник, чей скин ему показывать.
# Скин берётся у Mojang по правому нику, так что он должен быть настоящим.
# Строки с решёткой — примечания, они пропускаются.
#
# Пример:
# Вася = Notch
";

    if let Err(error) = fs::write(&path, sample) {
        log_warn!("Скины: {} не записан: {}", path.display(), error);
    }
}

/// Находит скин для ника: сперва смотрит в свою таблицу, потом в запомненное
/// на диске, и только затем спрашивает Mojang.
///
/// None означает «скина нет»: такого ника у Mojang не нашлось или до них не
/// достучаться. Игрок в этом случае заходит со стандартным скином.
pub async fn look_up(dir: &Path, name: &str, settings: Settings) -> Option<Skin> {
    // Чей скин выдать: по своей таблице — чужой, иначе — его собственный.
    let source = alias(dir, name).unwrap_or_else(|| name.to_string());

    if let Some(remembered) = read_cache(&cache_path(dir, &source)) {
        return remembered;
    }

    let fetched = match fetch(&source, settings).await {
        Answer::Found(skin) => Some(skin),
        Answer::Missing => {
            log_info!("Скины: скина {} нет ни в одном источнике", source);
            None
        }
        // Сбой сети или отказ от лишних запросов — это не «скина нет».
        // Запомнить такое на сутки значило бы сутки ходить без скина.
        Answer::Failed => {
            log_warn!("Скины: источники скина {} не ответили — спрошу при следующем входе", source);
            return None;
        }
    };

    write_cache(&cache_path(dir, &source), fetched.as_ref());

    fetched
}

/// Ответ источника скинов.
#[derive(Debug, PartialEq)]
enum Answer {
    Found(Skin),
    /// Источник ответил, что такого игрока или скина нет.
    Missing,
    /// Источник не ответил толком: сеть, отказ, ошибка на их стороне.
    Failed,
}

impl Answer {
    /// Ответ по коду HTTP, когда тела ответа не нужно: нет игрока — это
    /// 404 или 204 (так Mojang отвечал раньше), остальное — сбой.
    fn from_status(status: reqwest::StatusCode) -> Answer {
        if status == reqwest::StatusCode::NOT_FOUND || status == reqwest::StatusCode::NO_CONTENT {
            Answer::Missing
        } else {
            Answer::Failed
        }
    }
}

/// Итог двух источников: найденный скин — по порядку предпочтения; «нет»
/// — только если все опрошенные сказали «нет».
fn combine(mojang: Answer, ely: Answer) -> Answer {
    match (mojang, ely) {
        (Answer::Found(skin), _) | (_, Answer::Found(skin)) => Answer::Found(skin),
        (Answer::Missing, Answer::Missing) => Answer::Missing,
        _ => Answer::Failed,
    }
}

/// Чей скин выдать этому нику по своей таблице.
///
/// Таблица — обычный текстовый файл: в строке ник, знак равенства и ник, чей
/// скин брать. Пустые строки и строки, начинающиеся с решётки, пропускаются.
fn alias(dir: &Path, name: &str) -> Option<String> {
    let text = fs::read_to_string(dir.join(NAMES_FILE)).ok()?;

    find_alias(&text, name)
}

/// Разбирает таблицу и ищет в ней ник. Отдельно от чтения файла, чтобы
/// разбор можно было проверить.
fn find_alias(text: &str, name: &str) -> Option<String> {
    for line in text.lines() {
        let line = line.trim();

        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        let Some((left, right)) = line.split_once('=') else {
            continue;
        };

        if left.trim().eq_ignore_ascii_case(name) {
            let source = right.trim();

            if !source.is_empty() {
                return Some(source.to_string());
            }
        }
    }

    None
}

/// Где лежит запомненный скин этого ника.
///
/// Имя файла — ник в нижнем регистре: у Mojang ник не различает регистра,
/// и двух файлов на один и тот же скин быть не должно.
fn cache_path(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{}.skin", name.to_lowercase()))
}

/// Читает запомненный скин.
///
/// Внешнее None — «ничего не запомнено, надо спрашивать», внутреннее —
/// «спрашивали, скина нет»: второе тоже запоминается, иначе сервер ходил бы
/// к Mojang при каждом заходе игрока, которого там нет.
fn read_cache(path: &Path) -> Option<Option<Skin>> {
    let text = fs::read_to_string(path).ok()?;

    parse_cache(&text, now())
}

/// Разбирает запомненное. `now` — нынешнее время: по нему видно, не устарела
/// ли запись.
fn parse_cache(text: &str, now: u64) -> Option<Option<Skin>> {
    let mut lines = text.lines();

    let (magic, version) = lines.next()?.split_once(' ')?;

    if magic != MAGIC || version.parse::<u32>().ok()? != FORMAT_VERSION {
        return None;
    }

    let written: u64 = lines.next()?.parse().ok()?;

    if now.saturating_sub(written) > FRESH {
        return None;
    }

    let value = lines.next()?;
    let signature = lines.next().unwrap_or("");

    if value.is_empty() {
        return Some(None);
    }

    Some(Some(Skin {
        value: value.to_string(),
        signature: signature.to_string(),
    }))
}

/// Записывает скин на диск, чтобы не спрашивать его снова.
fn write_cache(path: &Path, skin: Option<&Skin>) {
    let (value, signature) = match skin {
        Some(skin) => (skin.value.as_str(), skin.signature.as_str()),
        None => ("", ""),
    };

    let text = format!("{} {}\n{}\n{}\n{}\n", MAGIC, FORMAT_VERSION, now(), value, signature);

    if let Err(error) = fs::write(path, text) {
        log_warn!("Скины: {} не записан: {}", path.display(), error);
    }
}

/// Нынешнее время в секундах.
fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|passed| passed.as_secs())
        .unwrap_or(0)
}

/// Спрашивает скин у Mojang.
///
/// Спрашивает источники по порядку: сперва Mojang, потом Ely.by.
async fn fetch(name: &str, settings: Settings) -> Answer {
    let Some(client) = client() else {
        return Answer::Failed;
    };

    // Оба источника спрашиваются сразу, а не по очереди: так игрок ждёт
    // самого медленного, а не их сумму. Предпочтение — прежнее: Mojang.
    let mojang = async {
        if settings.mojang {
            fetch_mojang(client, name).await
        } else {
            Answer::Missing
        }
    };
    let ely = async {
        if settings.ely {
            fetch_ely(client, name).await
        } else {
            Answer::Missing
        }
    };

    let (mojang, ely) = tokio::join!(mojang, ely);

    if matches!(mojang, Answer::Found(_)) {
        log_info!("Скины: скин {} получен от Mojang", name);
    } else if matches!(ely, Answer::Found(_)) {
        log_info!("Скины: скин {} получен от Ely.by", name);
    }

    combine(mojang, ely)
}

/// Скин у Mojang.
///
/// Делается это в два захода: сперва по нику узнаётся опознаватель игрока,
/// потом по опознавателю — его профиль со свойствами. Подпись отдают только
/// если её попросить отдельно, потому и unsigned=false.
async fn fetch_mojang(client: &reqwest::Client, name: &str) -> Answer {
    let uuid = match fetch_uuid(client, name).await {
        Ok(uuid) => uuid,
        Err(answer) => return answer,
    };
    let answer = client
        .get(format!(
            "https://sessionserver.mojang.com/session/minecraft/profile/{}?unsigned=false",
            uuid
        ))
        .send()
        .await;

    let answer = match answer {
        Ok(answer) if answer.status().is_success() => answer,
        Ok(answer) => return Answer::from_status(answer.status()),
        Err(error) => {
            log_warn!("Скины: до Mojang не достучаться: {}", error);
            return Answer::Failed;
        }
    };

    match answer.text().await {
        Ok(profile) => textures(&profile).map_or(Answer::Missing, Answer::Found),
        Err(_) => Answer::Failed,
    }
}

/// Скин у Ely.by.
///
/// У них профиль спрашивается сразу по нику, без опознавателя, и в том же
/// виде, что у Mojang: «This endpoint is an analog of the player profile
/// query in the Mojang's API, but instead of UUID user is queried by his
/// nickname» (docs.ely.by). Поэтому и разбирается тем же кодом.
async fn fetch_ely(client: &reqwest::Client, name: &str) -> Answer {
    let answer = client
        .get(format!("http://skinsystem.ely.by/profile/{}", name))
        .send()
        .await;

    let answer = match answer {
        Ok(answer) => answer,
        Err(error) => {
            log_warn!("Скины: до Ely.by не достучаться: {}", error);
            return Answer::Failed;
        }
    };

    if !answer.status().is_success() {
        return Answer::from_status(answer.status());
    }

    match answer.text().await {
        Ok(profile) => textures(&profile).map_or(Answer::Missing, Answer::Found),
        Err(_) => Answer::Failed,
    }
}

/// Тот, через кого сервер ходит к Mojang.
///
/// Он один на всё время работы: внутри у него сложенные соединения, и
/// заводить его заново на каждый запрос — значит каждый раз всё это
/// выбрасывать и собирать снова.
fn client() -> Option<&'static reqwest::Client> {
    static CLIENT: std::sync::OnceLock<Option<reqwest::Client>> = std::sync::OnceLock::new();

    CLIENT
        .get_or_init(|| reqwest::Client::builder().timeout(WAIT).build().ok())
        .as_ref()
}

/// Скачивает картинку скина по ссылке из его описания — для клиентов
/// Bedrock, которым картинка нужна целиком.
pub async fn download(url: &str) -> Option<Vec<u8>> {
    let answer = client()?.get(url).send().await.ok()?;

    if !answer.status().is_success() {
        return None;
    }

    answer.bytes().await.ok().map(|bytes| bytes.to_vec())
}

/// Узнаёт опознаватель игрока по нику; если не вышло — почему.
async fn fetch_uuid(client: &reqwest::Client, name: &str) -> Result<String, Answer> {
    let answer = client
        .get(format!(
            "https://api.mojang.com/users/profiles/minecraft/{}",
            name
        ))
        .send()
        .await;

    let answer = match answer {
        Ok(answer) => answer,
        Err(error) => {
            log_warn!("Скины: до Mojang не достучаться: {}", error);
            return Err(Answer::Failed);
        }
    };

    if !answer.status().is_success() {
        return Err(Answer::from_status(answer.status()));
    }

    let text = answer.text().await.map_err(|_| Answer::Failed)?;
    let parsed: serde_json::Value = serde_json::from_str(&text).map_err(|_| Answer::Failed)?;

    parsed.get("id").and_then(|id| id.as_str()).map(str::to_string).ok_or(Answer::Missing)
}

/// Достаёт из профиля свойство со скином.
fn textures(profile: &str) -> Option<Skin> {
    let parsed: serde_json::Value = serde_json::from_str(profile).ok()?;

    for property in parsed.get("properties")?.as_array()? {
        if property.get("name")?.as_str() != Some("textures") {
            continue;
        }

        return Some(Skin {
            value: property.get("value")?.as_str()?.to_string(),
            signature: property.get("signature")?.as_str()?.to_string(),
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Настройки читаются из файла; нет файла — всё включено и кладётся
    /// образец; испорченный файл — тоже всё включено.
    #[test]
    fn settings_are_read_and_default_to_everything_on() {
        let mut path = std::env::temp_dir();
        path.push(format!("mcsheriffanya-skins-{}.toml", std::process::id()));
        let _ = fs::remove_file(&path);

        // Нет файла — образец на месте, всё включено.
        assert_eq!(Settings::load(&path), Settings::default());
        assert!(path.exists(), "образец не положен");
        assert!(Settings::default().mojang && Settings::default().ely);

        // Выключили Ely — прочиталось.
        fs::write(&path, "mojang = true\nely = false\n").expect("пишется");
        assert_eq!(Settings::load(&path), Settings { mojang: true, ely: false });

        // Испорченный файл не роняет сервер.
        fs::write(&path, "mojang = ???").expect("пишется");
        assert_eq!(Settings::load(&path), Settings::default());

        let _ = fs::remove_file(&path);
    }

    /// Своя таблица: регистр ника не важен, пустые строки и примечания
    /// пропускаются.
    #[test]
    fn the_table_says_whose_skin_to_give() {
        let table = "\
# кому = чей
Лёня = Notch

  vasya  =  Dinnerbone
пустой =
";

        assert_eq!(find_alias(table, "Лёня"), Some("Notch".to_string()));
        assert_eq!(find_alias(table, "VASYA"), Some("Dinnerbone".to_string()));
        assert_eq!(find_alias(table, "пустой"), None);
        assert_eq!(find_alias(table, "кого-нет"), None);
        assert_eq!(find_alias("", "любой"), None);
    }

    /// Запомненный скин читается обратно тем же, чем был записан.
    #[test]
    fn a_remembered_skin_is_read_back() {
        let text = format!("{} {}\n1000\nописание\nподпись\n", MAGIC, FORMAT_VERSION);

        assert_eq!(
            parse_cache(&text, 1000),
            Some(Some(Skin {
                value: "описание".to_string(),
                signature: "подпись".to_string(),
            }))
        );
    }

    /// «Скина нет» тоже запоминается — пустой строкой вместо описания.
    #[test]
    fn the_absence_of_a_skin_is_remembered_too() {
        let text = format!("{} {}\n1000\n\n\n", MAGIC, FORMAT_VERSION);

        assert_eq!(parse_cache(&text, 1000), Some(None));
    }

    /// Запись старше суток не годится: игрок мог сменить скин. Не годится и
    /// чужой файл.
    #[test]
    fn a_stale_or_foreign_file_is_not_used() {
        let text = format!("{} {}\n1000\nописание\nподпись\n", MAGIC, FORMAT_VERSION);

        assert_eq!(parse_cache(&text, 1000 + FRESH), parse_cache(&text, 1000));
        assert_eq!(parse_cache(&text, 1000 + FRESH + 1), None);

        assert_eq!(parse_cache("что-то чужое\n", 1000), None);
        assert_eq!(parse_cache(&format!("{} 999\n1000\n\n\n", MAGIC), 1000), None);
        assert_eq!(parse_cache("", 1000), None);
    }

    /// Свойство со скином достаётся из профиля, а профиль без него ничего не
    /// Сбой источника — не «скина нет»: такое не запоминается на сутки.
    #[test]
    fn a_failed_source_is_not_an_absent_skin() {
        let skin = || Skin { value: "описание".to_string(), signature: String::new() };

        assert_eq!(combine(Answer::Missing, Answer::Failed), Answer::Failed);
        assert_eq!(combine(Answer::Missing, Answer::Missing), Answer::Missing);
        assert_eq!(combine(Answer::Failed, Answer::Found(skin())), Answer::Found(skin()));
        assert_eq!(Answer::from_status(reqwest::StatusCode::TOO_MANY_REQUESTS), Answer::Failed);
        assert_eq!(Answer::from_status(reqwest::StatusCode::NOT_FOUND), Answer::Missing);
    }

    /// даёт.
    #[test]
    fn the_skin_is_taken_from_the_profile() {
        let profile = r#"{"id":"0","name":"Кто-то","properties":[
            {"name":"что-то","value":"мимо"},
            {"name":"textures","value":"описание","signature":"подпись"}]}"#;

        assert_eq!(
            textures(profile),
            Some(Skin {
                value: "описание".to_string(),
                signature: "подпись".to_string(),
            })
        );

        assert_eq!(textures(r#"{"properties":[]}"#), None);
        assert_eq!(textures("не json"), None);
    }
}

