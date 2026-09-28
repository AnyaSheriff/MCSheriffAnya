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

use crate::{log_debug, log_info, log_warn};

/// Директория со скинами.
pub const DIRECTORY: &str = "skins";

/// Файл настроек: какие источники скинов включены.
pub const SETTINGS_FILE: &str = "config/skins.toml";

/// Откуда брать скины. Порядок опроса всегда один: сперва Mojang, потом
/// Ely.by — кто нашёл, того и берём.
#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(default)]
pub struct Settings {
    /// Спрашивать ли Mojang.
    pub mojang: bool,

    /// Спрашивать ли Ely.by, если у Mojang не нашлось.
    pub ely: bool,

    /// Загружать ли скины игроков Bedrock в Mojang через MineSkin, чтобы
    /// их видели игроки Java. Картинка уходит стороннему сервису — поэтому
    /// по умолчанию выключено.
    #[serde(rename = "bedrock-to-java")]
    pub bedrock_to_java: bool,

    /// Ключ API MineSkin (account.mineskin.org/keys): с ним лимит выше.
    /// Пусто — загрузка без ключа, с меньшим лимитом.
    #[serde(rename = "mineskin-key")]
    pub mineskin_key: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            mojang: true,
            ely: true,
            bedrock_to_java: false,
            mineskin_key: String::new(),
        }
    }
}

/// Образец файла настроек — с пояснениями, чтобы было понятно, что править.
/// Пишется вместе с [`BEDROCK_SECTION`].
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

/// Часть образца про скины Bedrock — её же дописываем в файл, положенный
/// до того, как она появилась.
const BEDROCK_SECTION: &str = "\
# Скины игроков Bedrock для игроков Java. Java загружает скины только
# с серверов Mojang, поэтому картинку скина Bedrock сервер отправляет
# сервису MineSkin (mineskin.org), тот загружает её в Mojang и возвращает
# подписанное описание. Скин уходит стороннему сервису — включайте, если
# согласны. Персонажи из редактора Bedrock так не показать: у Java нет
# таких фигур. Ключ — с account.mineskin.org/keys (без ключа лимит ниже).
bedrock-to-java = false
mineskin-key = \"\"
";

impl Settings {
    /// Читает настройки; нет файла — кладёт образец и берёт значения
    /// по умолчанию. Испорченный файл — тоже значения по умолчанию,
    /// с предупреждением в логе. В файл без настроек Bedrock они
    /// дописываются.
    pub fn load(path: &Path) -> Self {
        match fs::read_to_string(path) {
            Ok(text) => match toml::from_str::<Settings>(&text) {
                Ok(settings) => {
                    add_bedrock_section(path, &text);
                    settings
                }
                Err(error) => {
                    log_warn!("Скины: {} не разобрался ({}) — беру всё по умолчанию", path.display(), error);
                    Settings::default()
                }
            },
            Err(_) => {
                if let Err(error) = fs::write(path, format!("{}\n{}", SETTINGS_SAMPLE, BEDROCK_SECTION)) {
                    log_warn!("Скины: {} не записан: {}", path.display(), error);
                }

                Settings::default()
            }
        }
    }
}

/// Дописывает настройки Bedrock в файл, где их ещё нет; остальное в файле
/// не трогается.
fn add_bedrock_section(path: &Path, text: &str) {
    let has_key = |key: &str| text.lines().any(|line| line.trim_start().starts_with(key));

    if has_key("bedrock-to-java") || has_key("mineskin-key") {
        return;
    }

    let gap = if text.ends_with("\n\n") || text.is_empty() {
        ""
    } else if text.ends_with('\n') {
        "\n"
    } else {
        "\n\n"
    };

    match fs::write(path, format!("{}{}{}", text, gap, BEDROCK_SECTION)) {
        Ok(()) => log_info!("Скины: в {} дописаны настройки скинов Bedrock", path.display()),
        Err(error) => log_warn!("Скины: {} — настройки Bedrock не дописаны: {}", path.display(), error),
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

impl Skin {
    /// Требует ли описание подписи. У скинов Mojang внутри описания стоит
    /// `signatureRequired: true`, и клиент такой скин без подписи молча
    /// отбрасывает. Подпись заверяет само описание, а не опознаватель
    /// игрока, поэтому годится и игроку с другим опознавателем.
    pub fn signature_required(&self) -> bool {
        crate::bedrock::session::base64_url(&self.value)
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
            .and_then(|description| description.get("signatureRequired")?.as_bool())
            .unwrap_or(false)
    }
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
pub async fn look_up(dir: &Path, name: &str, settings: &Settings) -> Option<Skin> {
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

    parse_cache(&text, now(), FRESH)
}

/// Разбирает запомненное. `now` — нынешнее время: по нему и `fresh` видно,
/// не устарела ли запись.
fn parse_cache(text: &str, now: u64, fresh: u64) -> Option<Option<Skin>> {
    let mut lines = text.lines();

    let (magic, version) = lines.next()?.split_once(' ')?;

    if magic != MAGIC || version.parse::<u32>().ok()? != FORMAT_VERSION {
        return None;
    }

    let written: u64 = lines.next()?.parse().ok()?;

    if now.saturating_sub(written) > fresh {
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
async fn fetch(name: &str, settings: &Settings) -> Answer {
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

/// Сколько ждать задание MineSkin целиком и как часто о нём спрашивать
/// (их правила: не чаще раза в секунду).
const MINESKIN_WAIT: Duration = Duration::from_secs(90);
const MINESKIN_POLL: Duration = Duration::from_millis(1500);

/// Скин игрока Bedrock для игроков Java: картинка (PNG 64×64) загружается
/// в Mojang через MineSkin (документация: docs.mineskin.org, api.mineskin.org
/// /v2/openapi.json), в ответ — подписанное описание, как у любого скина
/// Mojang. Ответ запоминается по картинке навсегда: ссылка на картинку
/// у Mojang не устаревает, и тот же скин второй раз не загружается.
///
/// `cape` — плащ из списка MineSkin (`java_cape`), если плащ игрока Bedrock
/// есть и у Java.
pub async fn upload_bedrock(dir: &Path, settings: &Settings, png: Vec<u8>, slim: bool, cape: Option<&str>) -> Option<Skin> {
    // Плащи MineSkin прикладывает не на всяком тарифе: раз отказал — дальше
    // без плаща, скин важнее.
    let cape = cape.filter(|_| !CAPES_FORBIDDEN.load(std::sync::atomic::Ordering::Relaxed));

    let path = dir.join(format!(
        "bedrock-{:016x}{}{}.skin",
        fnv(&png),
        if slim { "-slim" } else { "" },
        cape.map(|cape| format!("-{}", cape)).unwrap_or_default()
    ));

    if let Some(Some(skin)) = fs::read_to_string(&path).ok().and_then(|text| parse_cache(&text, 0, u64::MAX)) {
        return Some(skin);
    }

    let client = mineskin_client()?;
    let with_headers = |request: reqwest::RequestBuilder| {
        let request = request.header("User-Agent", concat!("MCSheriffAnya/", env!("CARGO_PKG_VERSION")));
        match settings.mineskin_key.trim() {
            "" => request,
            key => request.bearer_auth(key),
        }
    };

    // Копия — на случай, если с плащом откажут и придётся без него.
    let png_again = cape.map(|_| png.clone());
    let form = reqwest::multipart::Form::new()
        .part("file", reqwest::multipart::Part::bytes(png).file_name("skin.png").mime_str("image/png").ok()?)
        .text("variant", if slim { "slim" } else { "classic" })
        .text("visibility", "unlisted");
    let form = match cape {
        Some(cape) => form.text("cape", cape.to_string()),
        None => form,
    };
    let Some(mut answer) = mineskin_answer(with_headers(client.post("https://api.mineskin.org/v2/queue").multipart(form))).await else {
        if cape.is_some() && CAPES_FORBIDDEN.load(std::sync::atomic::Ordering::Relaxed) {
            return Box::pin(upload_bedrock(dir, settings, png_again?, slim, None)).await;
        }
        return None;
    };

    let started = std::time::Instant::now();
    loop {
        if let Some(skin) = mineskin_skin(&answer) {
            write_cache(&path, Some(&skin));
            return Some(skin);
        }

        let job = answer.pointer("/job/id").and_then(|id| id.as_str())?.to_string();
        let status = answer.pointer("/job/status").and_then(|status| status.as_str()).unwrap_or("unknown");

        if status == "failed" || started.elapsed() > MINESKIN_WAIT {
            log_warn!("Скины: MineSkin не загрузил скин ({}): {}", status, mineskin_errors(&answer));
            return None;
        }

        tokio::time::sleep(MINESKIN_POLL).await;
        answer = mineskin_answer(with_headers(client.get(format!("https://api.mineskin.org/v2/queue/{}", job)))).await?;
    }
}

/// Плащ Java для плаща игрока Bedrock: те плащи, что есть в обеих играх
/// (юбилейные, праздничные), узнаются по картинке среди тех, что MineSkin
/// умеет прикладывать к скину (`GET /v2/capes`, поле `supported`). Свою
/// картинку плаща Mojang не принимает — плащ только из Bedrock Java не
/// покажет. Ответ — номер плаща у MineSkin.
pub async fn java_cape(width: u32, height: u32, rgba: &[u8]) -> Option<String> {
    let capes = mineskin_capes().await;
    let theirs = scaled_cape(width, height, rgba)?;

    let (distance, uuid, alias) = capes
        .iter()
        .map(|(uuid, alias, known)| (cape_distance(&theirs, known), uuid, alias))
        .min_by_key(|(distance, ..)| *distance)?;

    if distance > CAPE_LIKENESS {
        log_debug!("Скины: плащ Bedrock не похож ни на один плащ Java (ближе всех «{}», отличие {})", alias, distance);
        return None;
    }

    log_debug!("Скины: плащ Bedrock — это «{}» (отличие {})", alias, distance);
    Some(uuid.clone())
}

/// Насколько плащи могут отличаться (средняя разница канала на точку
/// лицевой стороны, ×100), чтобы считаться одним: у двух игр картинки
/// бывают пересохранены с мелкими отличиями.
const CAPE_LIKENESS: u32 = 600;

/// Плащи, которые MineSkin прикладывает к скину: номер, название и картинка
/// 64×32. Спрашиваются один раз за работу сервера.
async fn mineskin_capes() -> &'static [(String, String, Vec<u8>)] {
    static CAPES: tokio::sync::OnceCell<Vec<(String, String, Vec<u8>)>> = tokio::sync::OnceCell::const_new();

    CAPES
        .get_or_init(|| async {
            let Some(client) = mineskin_client() else {
                return Vec::new();
            };
            let request = client
                .get("https://api.mineskin.org/v2/capes")
                .header("User-Agent", concat!("MCSheriffAnya/", env!("CARGO_PKG_VERSION")));
            let Some(list) = mineskin_answer(request).await else {
                return Vec::new();
            };

            let mut capes = Vec::new();
            for cape in list.get("capes").and_then(|capes| capes.as_array()).into_iter().flatten() {
                let text = |key: &str| cape.get(key).and_then(|v| v.as_str()).map(str::to_string);
                let (Some(uuid), Some(alias), Some(url)) = (text("uuid"), text("alias"), text("url")) else {
                    continue;
                };
                if cape.get("supported").and_then(|v| v.as_bool()) != Some(true) {
                    continue;
                }
                if let Some(image) = download(&url).await.as_deref().and_then(crate::bedrock::png::decode)
                    && let Some(rgba) = scaled_cape(image.width, image.height, &image.rgba)
                {
                    capes.push((uuid, alias, rgba));
                }
            }

            log_debug!("Скины: MineSkin прикладывает {} плащей", capes.len());
            capes
        })
        .await
}

/// Плащ к размеру 64×32 (у высокой чёткости — 128×64 и больше, берётся
/// каждая n-я точка). Другие пропорции — не плащ.
fn scaled_cape(width: u32, height: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    if width < 64 || !width.is_multiple_of(64) || height * 2 != width || rgba.len() != (width * height * 4) as usize {
        return None;
    }

    let step = (width / 64) as usize;
    Some(
        (0..64 * 32usize)
            .flat_map(|at| {
                let (x, y) = (at % 64 * step, at / 64 * step);
                let from = (y * width as usize + x) * 4;
                rgba[from..from + 4].to_vec()
            })
            .collect(),
    )
}

/// Отличие двух плащей 64×32 — по их лицевой и задней стороне (область
/// 22×17 в левом верхнем углу картинки): средняя разница канала ×100.
fn cape_distance(a: &[u8], b: &[u8]) -> u32 {
    let mut total = 0u64;
    let mut count = 0u64;

    for y in 0..17 {
        for x in 0..22 {
            let at = (y * 64 + x) * 4;
            for channel in 0..4 {
                total += u64::from(a[at + channel].abs_diff(b[at + channel]));
                count += 1;
            }
        }
    }

    (total * 100 / count) as u32
}

/// MineSkin отказал прикладывать плащ (такое бывает не на всяком тарифе):
/// дальше скины загружаются без плащей.
static CAPES_FORBIDDEN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Отправляет запрос MineSkin и разбирает ответ; отказ — в лог и None.
async fn mineskin_answer(request: reqwest::RequestBuilder) -> Option<serde_json::Value> {
    let answer = match request.send().await {
        Ok(answer) => answer,
        Err(error) => {
            log_warn!("Скины: MineSkin не отвечает: {}", error);
            return None;
        }
    };
    let status = answer.status();
    let body: serde_json::Value = answer.json().await.unwrap_or_default();

    if !status.is_success() {
        let errors = mineskin_errors(&body);

        if status == reqwest::StatusCode::FORBIDDEN && errors.contains("cape") {
            log_info!("Скины: MineSkin не прикладывает плащи с этим ключом — скины пойдут без плащей");
            CAPES_FORBIDDEN.store(true, std::sync::atomic::Ordering::Relaxed);
        } else {
            log_warn!("Скины: MineSkin отказал ({}): {}", status.as_u16(), errors);
        }
        return None;
    }

    Some(body)
}

/// Готовый скин из ответа MineSkin, если он уже там.
fn mineskin_skin(answer: &serde_json::Value) -> Option<Skin> {
    let data = answer.pointer("/skin/texture/data")?;
    let text = |key: &str| data.get(key).and_then(|v| v.as_str()).filter(|t| !t.is_empty()).map(str::to_string);

    Some(Skin { value: text("value")?, signature: text("signature")? })
}

/// Причины отказа из ответа MineSkin — для лога.
fn mineskin_errors(answer: &serde_json::Value) -> String {
    answer
        .get("errors")
        .and_then(|errors| errors.as_array())
        .map(|errors| errors.iter().filter_map(|error| error.get("message").and_then(|m| m.as_str())).collect::<Vec<_>>().join("; "))
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| "без пояснений".to_string())
}

/// Клиент для MineSkin: загрузка и очередь идут дольше, чем ответы Mojang.
fn mineskin_client() -> Option<&'static reqwest::Client> {
    static CLIENT: std::sync::OnceLock<Option<reqwest::Client>> = std::sync::OnceLock::new();

    CLIENT
        .get_or_init(|| reqwest::Client::builder().timeout(Duration::from_secs(20)).build().ok())
        .as_ref()
}

/// Хеш FNV-1a — имя файла для запомненного скина по его картинке: один и
/// тот же на любой машине и версии.
fn fnv(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3))
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
    /// Загрузка в MineSkin по-настоящему (сеть, без ключа): пробная картинка
    /// возвращается подписанным описанием со ссылкой на textures.minecraft.net.
    /// `cargo test --release mineskin_upload_live -- --ignored --nocapture`.
    #[tokio::test]
    #[ignore]
    async fn mineskin_upload_live() {
        let rgba: Vec<u8> = (0..64 * 64u32).flat_map(|at| [(at % 64 * 4) as u8, (at / 64 * 4) as u8, 0x80, 255]).collect();
        let png = crate::bedrock::png::encode(64, 64, &rgba);
        let dir = std::env::temp_dir().join(format!("mcsheriffanya-mineskin-{}", std::process::id()));
        fs::create_dir_all(&dir).expect("папка");

        let skin = upload_bedrock(&dir, &Settings::default(), png.clone(), false, None).await.expect("MineSkin вернул скин");
        let described = crate::bedrock::session::base64_url(&skin.value).expect("base64");
        let text = String::from_utf8_lossy(&described);
        println!("{}", text);
        assert!(text.contains("textures.minecraft.net") && !skin.signature.is_empty());

        // Второй раз — с диска, без сети.
        assert_eq!(upload_bedrock(&dir, &Settings::default(), png, false, None).await, Some(skin));
        let _ = fs::remove_dir_all(&dir);
    }

    /// Плащи MineSkin по-настоящему (сеть): каждый узнаётся сам в себе,
    /// а плащ 128×64 — тот же, что 64×32.
    #[tokio::test]
    #[ignore]
    async fn mineskin_capes_match_themselves_live() {
        let capes = mineskin_capes().await;
        println!("плащей: {}", capes.len());
        assert!(capes.len() >= 5);

        for (uuid, alias, rgba) in capes {
            let big: Vec<u8> = (0..128 * 64usize).flat_map(|at| {
                let from = ((at / 128 / 2) * 64 + at % 128 / 2) * 4;
                rgba[from..from + 4].to_vec()
            }).collect();
            assert_eq!(java_cape(128, 64, &big).await.as_deref(), Some(uuid.as_str()), "{}", alias);
        }
    }

    #[test]
    fn capes_compare_by_their_front() {
        let plain = |value: u8| [value, value, value, 255].repeat(64 * 32);
        assert_eq!(cape_distance(&plain(10), &plain(10)), 0);
        assert_eq!(cape_distance(&plain(10), &plain(16)), 450); // три канала из четырёх на 6
        assert!(scaled_cape(64, 64, &[0; 64 * 64 * 4]).is_none());
        assert_eq!(scaled_cape(128, 64, &plain(3).repeat(4)).map(|rgba| rgba.len()), Some(64 * 32 * 4));
    }

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
        assert_eq!(Settings::load(&path), Settings { mojang: true, ely: false, ..Settings::default() });

        // В старый файл дописаны настройки Bedrock, прежнее не тронуто.
        let text = fs::read_to_string(&path).expect("читается");
        assert!(text.starts_with("mojang = true\nely = false\n\n") && text.contains("bedrock-to-java = false"));
        assert_eq!(Settings::load(&path), Settings { mojang: true, ely: false, ..Settings::default() });
        assert_eq!(fs::read_to_string(&path).expect("читается"), text, "дописано второй раз");

        // Загрузка скинов Bedrock и ключ.
        fs::write(&path, "bedrock-to-java = true\nmineskin-key = \"abc\"\n").expect("пишется");
        let settings = Settings::load(&path);
        assert!(settings.bedrock_to_java && settings.mineskin_key == "abc" && settings.mojang);

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
            parse_cache(&text, 1000, FRESH),
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

        assert_eq!(parse_cache(&text, 1000, FRESH), Some(None));
    }

    /// Запись старше суток не годится: игрок мог сменить скин. Не годится и
    /// чужой файл.
    #[test]
    fn a_stale_or_foreign_file_is_not_used() {
        let text = format!("{} {}\n1000\nописание\nподпись\n", MAGIC, FORMAT_VERSION);

        assert_eq!(parse_cache(&text, 1000 + FRESH, FRESH), parse_cache(&text, 1000, FRESH));
        assert_eq!(parse_cache(&text, 1000 + FRESH + 1, FRESH), None);

        assert_eq!(parse_cache("что-то чужое\n", 1000, FRESH), None);
        assert_eq!(parse_cache(&format!("{} 999\n1000\n\n\n", MAGIC), 1000, FRESH), None);
        assert_eq!(parse_cache("", 1000, FRESH), None);
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

    /// Описание Mojang с `signatureRequired` требует подписи, прочие — нет.
    #[test]
    fn mojang_skins_require_a_signature() {
        let with = |json: &str| Skin {
            value: {
                // base64 без внешних зависимостей: через тот же разбор, что
                // у входа Bedrock, проверяем только обратную сторону.
                const TABLE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
                let bytes = json.as_bytes();
                let mut out = String::new();
                for chunk in bytes.chunks(3) {
                    let n = chunk.iter().enumerate().fold(0u32, |n, (i, b)| n | (*b as u32) << (16 - 8 * i));
                    for i in 0..=chunk.len() {
                        out.push(TABLE[(n >> (18 - 6 * i) & 63) as usize] as char);
                    }
                }
                out
            },
            signature: String::new(),
        };

        assert!(with(r#"{"profileName":"A","signatureRequired":true}"#).signature_required());
        assert!(!with(r#"{"profileName":"A"}"#).signature_required());
    }
}
