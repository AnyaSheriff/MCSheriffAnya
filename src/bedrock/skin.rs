// Скины для клиентов Bedrock.
//
// Bedrock получает скин целиком — картинкой в пакете, а не ссылкой, как Java.
// Поэтому:
// - игрок Bedrock присылает свой скин сам, в данных входа (ClientData); его
//   и показываем другим игрокам Bedrock;
// - у игрока Java есть только описание скина со ссылкой (свойство textures);
//   картинку сервер скачивает сам, разбирает PNG и шлёт как есть. Пока она
//   качается, показывается одноцветная заглушка, а после приходит Player Skin.
//
// Скины хранятся по UUID игрока и живут, пока работает сервер.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, OnceLock};

use super::codec::Out;
use super::png;
use super::session::base64_url;
use crate::players::Member;
use crate::log_debug;

/// Картинка: ширина, высота, точки RGBA.
#[derive(Clone, Default)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Кусок облика «персоны» (собранного в редакторе персонажа).
#[derive(Clone)]
pub struct Piece {
    id: String,
    kind: String,
    pack: String,
    default: bool,
    product: String,
}

/// Кадр анимации скина (моргание и подобное).
#[derive(Clone)]
pub struct Animation {
    image: Image,
    kind: i32,
    frames: f32,
    /// Вид анимации (0 — плавная, 1 — моргание): у Mojang u32, не число с точкой.
    expression: i32,
}

/// Скин в том виде, в каком его ждёт Bedrock.
#[derive(Clone, Default)]
pub struct Look {
    id: String,
    play_fab: String,
    resource_patch: String,
    image: Image,
    animations: Vec<Animation>,
    cape: Image,
    geometry: String,
    geometry_version: String,
    animation_data: String,
    cape_id: String,
    arm_size: String,
    color: String,
    pieces: Vec<Piece>,
    tints: Vec<(String, Vec<String>)>,
    premium: bool,
    persona: bool,
    cape_on_classic: bool,
}

/// Известные скины: UUID игрока → (номер версии, скин).
type Looks = HashMap<[u8; 16], (u64, Arc<Look>)>;

fn looks() -> &'static Mutex<Looks> {
    static LOOKS: OnceLock<Mutex<Looks>> = OnceLock::new();
    LOOKS.get_or_init(Default::default)
}

/// Чьи скины уже качаются или скачаны — чтобы не просить дважды.
fn requested() -> &'static Mutex<HashSet<[u8; 16]>> {
    static REQUESTED: OnceLock<Mutex<HashSet<[u8; 16]>>> = OnceLock::new();
    REQUESTED.get_or_init(Default::default)
}

/// Запоминает скин игрока.
pub fn remember(uuid: [u8; 16], look: Look) {
    let mut looks = looks().lock().expect("скины захвачены другим потоком");
    let version = looks.get(&uuid).map_or(1, |(version, _)| version + 1);
    looks.insert(uuid, (version, Arc::new(look)));
}

/// Забывает скин ушедшего игрока Bedrock: при следующем входе он пришлёт свой.
pub fn forget(uuid: &[u8; 16]) {
    looks().lock().expect("скины захвачены другим потоком").remove(uuid);
}

/// Версия скина игрока — без самого скина: чтобы каждый такт узнавать, не
/// сменился ли он. Для игрока Java без скачанной картинки — 0, а картинка
/// начинает качаться.
pub fn version_of(member: &Member) -> u64 {
    if let Some((version, _)) = looks().lock().expect("скины захвачены другим потоком").get(&member.uuid) {
        return *version;
    }

    start_download(member);
    0
}

/// Скин игрока и его версия; для игрока Java без скачанной картинки —
/// заглушка (версия 0), а картинка начинает качаться.
pub fn look_of(member: &Member) -> (u64, Arc<Look>) {
    if let Some((version, look)) = looks().lock().expect("скины захвачены другим потоком").get(&member.uuid) {
        return (*version, Arc::clone(look));
    }

    start_download(member);
    (0, Arc::new(placeholder(&member.name)))
}

/// Начинает качать скин игрока Java, если он ещё не качается.
fn start_download(member: &Member) {
    if let Some(skin) = &member.skin
        && requested().lock().expect("скины захвачены другим потоком").insert(member.uuid)
    {
        let uuid = member.uuid;
        let name = member.name.clone();
        let value = skin.value.clone();

        tokio::spawn(async move {
            match download_java(&value).await {
                Some(look) => remember(uuid, look),
                None => log_debug!("Bedrock: скин {} не скачался — остаётся заглушка", name),
            }
        });
    }
}

/// Одноцветный скин 64×64. Картинка у всех заглушек одна — считается
/// один раз.
fn placeholder(name: &str) -> Look {
    static RGBA: OnceLock<Vec<u8>> = OnceLock::new();
    let rgba = RGBA.get_or_init(|| [0x4a, 0x6f, 0x9e, 0xff].repeat(64 * 64)).clone();

    java_look(name, "placeholder", Image { width: 64, height: 64, rgba }, false)
}

/// Скин из картинки Java: обычная или тонкорукая модель. `tag` различает
/// картинки одного игрока: клиент помнит скины по имени.
fn java_look(name: &str, tag: &str, image: Image, slim: bool) -> Look {
    let geometry = match (slim, image.height) {
        (_, 32) => "geometry.humanoid",
        (true, _) => "geometry.humanoid.customSlim",
        (false, _) => "geometry.humanoid.custom",
    };
    let id = format!("mcsheriffanya-{}-{}", name, tag);

    Look {
        id: id.clone(),
        resource_patch: format!(r#"{{"geometry":{{"default":"{}"}}}}"#, geometry),
        image,
        geometry_version: "0.0.0".to_string(),
        arm_size: if slim { "slim" } else { "wide" }.to_string(),
        color: "#0".to_string(),
        ..Look::default()
    }
}

/// Качает и разбирает картинку по описанию скина Java.
async fn download_java(value: &str) -> Option<Look> {
    let description: serde_json::Value = serde_json::from_slice(&base64_url(value)?).ok()?;
    let skin = description.get("textures")?.get("SKIN")?;
    let url = skin.get("url")?.as_str()?;
    let slim = skin.get("metadata").and_then(|m| m.get("model")).and_then(|m| m.as_str()) == Some("slim");
    let name = description.get("profileName").and_then(|n| n.as_str()).unwrap_or("").to_string();

    let bytes = crate::skins::download(url).await?;
    let png = png::decode(&bytes)?;

    if png.width != 64 || (png.height != 64 && png.height != 32) {
        return None;
    }

    let mut image = Image { width: png.width, height: png.height, rgba: png.rgba };
    make_base_opaque(&mut image);

    // Имя картинки у Mojang — её отпечаток: новая картинка — новое имя.
    let tag = url.rsplit('/').next().unwrap_or("skin");
    let mut look = java_look(&name, tag, image, slim);

    // Плащ — отдельной картинкой по своей ссылке (64×32, как у Bedrock).
    // Не скачался — скин и без плаща.
    if let Some(cape_url) = description.pointer("/textures/CAPE/url").and_then(|u| u.as_str())
        && let Some(cape) = crate::skins::download(cape_url).await.as_deref().and_then(png::decode)
        && cape.width == 64
        && cape.height == 32
    {
        look.cape = Image { width: cape.width, height: cape.height, rgba: cape.rgba };
        look.cape_id = format!("mcsheriffanya-cape-{}", cape_url.rsplit('/').next().unwrap_or("cape"));
        look.cape_on_classic = true;
    }

    Some(look)
}

/// Плащ игрока Bedrock картинкой: ширина, высота, точки RGBA. Нет плаща —
/// None.
pub fn cape(look: &Look) -> Option<(u32, u32, &[u8])> {
    (!look.cape.rgba.is_empty()).then_some((look.cape.width, look.cape.height, look.cape.rgba.as_slice()))
}

/// Java рисует нижний слой скина непрозрачным, даже если в файле там
/// прозрачные точки (minecraft.wiki, «Skin»); Bedrock показал бы дыры.
fn make_base_opaque(image: &mut Image) {
    // Прямоугольники нижнего слоя: (x, y, ширина, высота).
    let mut parts = vec![(0, 0, 32, 16), (0, 16, 56, 16)];

    if image.height == 64 {
        parts.extend([(16, 48, 32, 16)]);
    }

    for (x0, y0, w, h) in parts {
        for y in y0..y0 + h {
            for x in x0..x0 + w {
                image.rgba[((y * image.width + x) * 4 + 3) as usize] = 255;
            }
        }
    }
}

/// Пределы того, что принимается из данных входа: сторона картинки, число
/// анимаций, кусков облика и цветов, длина строк и описаний геометрии.
/// Настоящие скины в разы меньше; больше — клиент подделан.
const MAX_SIDE: u32 = 512;
const MAX_ANIMATIONS: usize = 16;
const MAX_PIECES: usize = 128;
const MAX_TEXT: usize = 1024;
const MAX_DATA: usize = 1024 * 1024;

/// Скин игрока Bedrock из данных входа (второй JWT пакета Login).
pub fn from_client_data(json: &serde_json::Value) -> Option<Look> {
    let text = |key: &str| json.get(key).and_then(|v| v.as_str()).filter(|t| t.len() <= MAX_TEXT).unwrap_or("").to_string();
    let decoded = |key: &str| base64_url(json.get(key)?.as_str()?);
    let number = |value: Option<&serde_json::Value>| value.and_then(|v| v.as_f64()).unwrap_or(0.0);
    let image = |data: Option<Vec<u8>>, width: f64, height: f64| {
        let rgba = data.unwrap_or_default();
        // Дробные, отрицательные и огромные размеры не проходят; площадь
        // считается без переполнения.
        let side = |value: f64| (value.fract() == 0.0 && (0.0..=MAX_SIDE as f64).contains(&value)).then_some(value as u32);
        let size = side(width).zip(side(height)).and_then(|(w, h)| Some((w, h, w.checked_mul(h)?.checked_mul(4)? as usize)));

        match size {
            Some((width, height, bytes)) if bytes > 0 && rgba.len() == bytes => Image { width, height, rgba },
            _ => Image::default(),
        }
    };
    let flag = |key: &str| json.get(key).and_then(|v| v.as_bool()).unwrap_or(false);
    let utf8 = |bytes: Option<Vec<u8>>| {
        bytes.filter(|b| b.len() <= MAX_DATA).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default()
    };

    let skin = image(decoded("SkinData"), number(json.get("SkinImageWidth")), number(json.get("SkinImageHeight")));

    if skin.rgba.is_empty() {
        return None;
    }

    let animations = json
        .get("AnimatedImageData")
        .and_then(|a| a.as_array())
        .map(|list| {
            list.iter()
                .take(MAX_ANIMATIONS)
                .map(|entry| Animation {
                    image: image(
                        entry.get("Image").and_then(|i| i.as_str()).and_then(base64_url),
                        number(entry.get("ImageWidth")),
                        number(entry.get("ImageHeight")),
                    ),
                    kind: number(entry.get("Type")) as i32,
                    frames: number(entry.get("Frames")) as f32,
                    expression: number(entry.get("AnimationExpression")) as i32,
                })
                .collect()
        })
        .unwrap_or_default();

    let pieces = json
        .get("PersonaPieces")
        .and_then(|p| p.as_array())
        .map(|list| {
            list.iter()
                .take(MAX_PIECES)
                .map(|piece| {
                    let field = |key: &str| {
                        piece.get(key).and_then(|v| v.as_str()).filter(|t| t.len() <= MAX_TEXT).unwrap_or("").to_string()
                    };
                    Piece {
                        id: field("PieceId"),
                        kind: field("PieceType"),
                        pack: field("PackId"),
                        default: piece.get("IsDefault").and_then(|v| v.as_bool()).unwrap_or(false),
                        product: field("ProductId"),
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    let tints = json
        .get("PieceTintColors")
        .and_then(|p| p.as_array())
        .map(|list| {
            list.iter()
                .take(MAX_PIECES)
                .map(|tint| {
                    let kind = tint.get("PieceType").and_then(|v| v.as_str()).filter(|t| t.len() <= MAX_TEXT).unwrap_or("").to_string();
                    let colors = tint
                        .get("Colors")
                        .and_then(|c| c.as_array())
                        .map(|c| {
                            c.iter().take(MAX_PIECES).filter_map(|v| v.as_str().filter(|t| t.len() <= MAX_TEXT).map(str::to_string)).collect()
                        })
                        .unwrap_or_default();
                    (kind, colors)
                })
                .collect()
        })
        .unwrap_or_default();

    Some(Look {
        id: text("SkinId"),
        play_fab: text("PlayFabId"),
        resource_patch: utf8(decoded("SkinResourcePatch")),
        image: skin,
        animations,
        cape: image(decoded("CapeData"), number(json.get("CapeImageWidth")), number(json.get("CapeImageHeight"))),
        geometry: utf8(decoded("SkinGeometryData")),
        geometry_version: utf8(decoded("SkinGeometryDataEngineVersion")),
        animation_data: utf8(decoded("SkinAnimationData")),
        cape_id: text("CapeId"),
        arm_size: text("ArmSize"),
        color: text("SkinColor"),
        pieces,
        tints,
        premium: flag("PremiumSkin"),
        persona: flag("PersonaSkin"),
        cape_on_classic: flag("CapeOnClassicSkin"),
    })
}

/// Скин игрока Bedrock картинкой для Java: PNG 64×64 (или старый 64×32)
/// и тонкие ли руки. Только обычный скин на стандартной фигуре: у персонажа
/// из редактора и у скина со своей фигурой раскладка картинки другая, Java
/// так не нарисует. 128×128 ужимается вдвое — Mojang берёт только 64.
pub fn java_png(look: &Look) -> Option<(Vec<u8>, bool)> {
    let figure = serde_json::from_str::<serde_json::Value>(&look.resource_patch)
        .ok()
        .and_then(|patch| patch.pointer("/geometry/default").and_then(|v| v.as_str()).map(str::to_string))
        .unwrap_or_default();
    let slim = match figure.as_str() {
        "geometry.humanoid.custom" | "geometry.humanoid" => false,
        "geometry.humanoid.customSlim" => true,
        _ => {
            log_debug!("Bedrock: скин {} на своей фигуре «{}» — для Java не годится", look.id, figure);
            return None;
        }
    };

    if look.persona {
        log_debug!("Bedrock: скин {} — персонаж из редактора, для Java не годится", look.id);
        return None;
    }

    let image = &look.image;
    let rgba = match (image.width, image.height) {
        (64, 64) | (64, 32) => image.rgba.clone(),
        // Точка из каждого квадрата 2×2 — левая верхняя: так чёткие
        // пиксельные скины остаются чёткими.
        (128, 128) => (0..64 * 64)
            .flat_map(|at| {
                let (x, y) = (at % 64 * 2, at / 64 * 2);
                let from = (y * 128 + x) * 4;
                image.rgba[from..from + 4].to_vec()
            })
            .collect(),
        (width, height) => {
            log_debug!("Bedrock: скин {} размером {}×{} — для Java не годится", look.id, width, height);
            return None;
        }
    };
    let height = if image.height == 32 { 32 } else { 64 };

    Some((png::encode(64, height, &rgba), slim))
}

fn image(out: &mut Out, image: &Image) {
    out.li32(image.width as i32).li32(image.height as i32).varint(image.rgba.len() as u32).raw(&image.rgba);
}

/// Скин в пакете (тип Skin из описания протокола).
pub fn encode(out: &mut Out, look: &Look) {
    out.string(&look.id).string(&look.play_fab).string(&look.resource_patch);
    image(out, &look.image);
    out.li32(look.animations.len() as i32);

    for animation in &look.animations {
        image(out, &animation.image);
        out.li32(animation.kind).lf32(animation.frames).li32(animation.expression);
    }

    image(out, &look.cape);
    out.string(&look.geometry)
        .string(&look.geometry_version)
        .string(&look.animation_data)
        .string(&look.cape_id)
        .string(&format!("{}{}", look.id, look.cape_id))
        .string(&look.arm_size)
        .string(&look.color)
        .li32(look.pieces.len() as i32);

    for piece in &look.pieces {
        out.string(&piece.id).string(&piece.kind).string(&piece.pack).bool(piece.default).string(&piece.product);
    }

    out.li32(look.tints.len() as i32);

    for (kind, colors) in &look.tints {
        out.string(kind).li32(colors.len() as i32);

        for color in colors {
            out.string(color);
        }
    }

    out.bool(look.premium)
        .bool(look.persona)
        .bool(look.cape_on_classic)
        .bool(true) // primary_user
        .bool(false); // overriding_player_appearance
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_data_skin_reads() {
        let json = serde_json::json!({
            "SkinId": "custom",
            "SkinData": "AAAA/w==",
            "SkinImageWidth": 1,
            "SkinImageHeight": 1,
            "SkinResourcePatch": "e30=",
            "ArmSize": "slim",
            "PersonaSkin": false,
        });
        let look = from_client_data(&json).expect("скин");

        assert_eq!(look.image.rgba, vec![0, 0, 0, 255]);
        assert_eq!(look.resource_patch, "{}");
        assert_eq!(look.arm_size, "slim");
    }

    #[test]
    fn broken_client_skin_is_rejected() {
        let json = serde_json::json!({"SkinData": "AAAA", "SkinImageWidth": 64, "SkinImageHeight": 64});
        assert!(from_client_data(&json).is_none());
    }

    /// Огромные и дробные размеры не принимаются, и площадь не переполняется.
    #[test]
    fn oversized_client_skins_are_rejected() {
        for (width, height) in [(1024.0, 1024.0), (-1.0, -4.0), (65536.0, 65536.0), (0.5, 8.0)] {
            let json = serde_json::json!({"SkinData": "AAAA/w==", "SkinImageWidth": width, "SkinImageHeight": height});
            assert!(from_client_data(&json).is_none(), "{}×{}", width, height);
        }
    }

    /// Скин для Java: 128×128 ужимается до 64×64, тонкие руки узнаются по
    /// фигуре, персонаж из редактора и своя фигура не отдаются.
    #[test]
    fn bedrock_skins_turn_into_java_pictures() {
        let rgba: Vec<u8> = (0..128 * 128).flat_map(|at: u32| [(at % 128) as u8, (at / 128) as u8, 0, 255]).collect();
        let look = java_look("анна", "проба", Image { width: 128, height: 128, rgba }, true);

        let (bytes, slim) = java_png(&look).expect("картинка");
        let picture = png::decode(&bytes).expect("PNG");
        assert!(slim);
        assert_eq!((picture.width, picture.height), (64, 64));
        // Точка (10, 3) — это (20, 6) большой картинки.
        assert_eq!(&picture.rgba[(3 * 64 + 10) * 4..][..2], &[20, 6]);

        let persona = Look { persona: true, ..look.clone() };
        assert!(java_png(&persona).is_none());

        let odd = Look { resource_patch: r#"{"geometry":{"default":"geometry.dragon"}}"#.to_string(), ..look };
        assert!(java_png(&odd).is_none());
    }

    #[test]
    fn java_base_layer_becomes_opaque() {
        let mut image = Image { width: 64, height: 64, rgba: vec![0; 64 * 64 * 4] };
        make_base_opaque(&mut image);

        assert_eq!(image.rgba[3], 255); // голова
        assert_eq!(image.rgba[(40 * 4) + 3], 0); // верхний слой головы
        assert_eq!(image.rgba[((50 * 64 + 20) * 4 + 3) as usize], 255); // левая нога
    }
}
