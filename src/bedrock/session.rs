// Сессия игрока Bedrock: от настроек сети до появления в мире и дальше.
//
// Порядок пакетов — по описанию протокола (minecraft-data, proto.yml, начало
// раздела «Login Sequence»): Request Network Settings → Network Settings →
// Login → Play Status (успех) → Resource Packs Info → Resource Pack Stack →
// Start Game → Item Registry → Creative Content → Biome Definition List →
// чанки → Play Status (появление).

use std::sync::Arc;

use tokio::sync::mpsc;

use super::codec::{self, In, Out};
use super::raknet::Sender;
use super::world::BedrockWorld;
use crate::shared::Shared;
use crate::{log_debug, log_info};

pub const LOGIN: u32 = 0x01;
pub const PLAY_STATUS: u32 = 0x02;
pub const DISCONNECT: u32 = 0x05;
pub const RESOURCE_PACKS_INFO: u32 = 0x06;
pub const RESOURCE_PACK_STACK: u32 = 0x07;
pub const RESOURCE_PACK_RESPONSE: u32 = 0x08;
pub const TEXT: u32 = 0x09;
pub const SET_TIME: u32 = 0x0a;
pub const START_GAME: u32 = 0x0b;
pub const ADD_PLAYER: u32 = 0x0c;
pub const REMOVE_ENTITY: u32 = 0x0e;
pub const MOVE_PLAYER: u32 = 0x13;
pub const UPDATE_BLOCK: u32 = 0x15;
pub const INVENTORY_TRANSACTION: u32 = 0x1e;
pub const UPDATE_ABILITIES: u32 = 0xbb;
pub const PLAYER_SKIN: u32 = 0x5d;
pub const ADD_ITEM_ENTITY: u32 = 0x0f;
pub const TAKE_ITEM_ENTITY: u32 = 0x11;
pub const MOB_EQUIPMENT: u32 = 0x1f;
pub const INVENTORY_CONTENT: u32 = 0x31;
pub const INVENTORY_SLOT: u32 = 0x32;
pub const ITEM_STACK_REQUEST: u32 = 0x93;
pub const AVAILABLE_COMMANDS: u32 = 0x4c;
pub const COMMAND_REQUEST: u32 = 0x4d;
pub const SET_PLAYER_GAME_TYPE: u32 = 0x3e;
pub const ITEM_STACK_RESPONSE: u32 = 0x94;
pub const PLAYER_LIST: u32 = 0x3f;
pub const LEVEL_CHUNK: u32 = 0x3a;
pub const REQUEST_CHUNK_RADIUS: u32 = 0x45;
pub const CHUNK_RADIUS_UPDATE: u32 = 0x46;
pub const SET_LOCAL_PLAYER_INITIALIZED: u32 = 0x71;
pub const CHUNK_PUBLISHER_UPDATE: u32 = 0x79;
pub const BIOME_DEFINITION_LIST: u32 = 0x7a;
pub const NETWORK_SETTINGS: u32 = 0x8f;
pub const PLAYER_AUTH_INPUT: u32 = 0x90;
pub const CREATIVE_CONTENT: u32 = 0x91;
pub const ITEM_REGISTRY: u32 = 0xa2;
pub const REQUEST_NETWORK_SETTINGS: u32 = 0xc1;

/// С какого размера пакеты-обёртки сжимаются.
const COMPRESSION_THRESHOLD: usize = 256;

/// Кто зашёл: из цепочки JWT пакета Login.
#[derive(Clone)]
pub struct Identity {
    pub name: String,
    pub uuid: [u8; 16],
    pub xuid: String,
    /// Скин из данных входа, если он там разборчивый.
    pub look: Option<super::skin::Look>,
}

/// Состояние соединения с клиентом Bedrock.
pub struct Connection {
    out: Sender,
    compressed: bool,
}

impl Connection {
    /// Отправить пакеты одной обёрткой.
    pub fn send(&self, packets: &[Vec<u8>]) {
        let body = codec::batch(packets, self.compressed, COMPRESSION_THRESHOLD);
        self.out.send(body);
    }

    pub fn send_one(&self, packet: Out) {
        self.send(&[packet.bytes]);
    }

    pub fn close(&self) {
        self.out.close();
    }
}

/// Ведёт одного игрока Bedrock от подключения до выхода.
pub async fn run(shared: Arc<Shared>, addr: std::net::SocketAddr, mut inbound: mpsc::UnboundedReceiver<Vec<u8>>, out: Sender) {
    let mut connection = Connection { out, compressed: false };
    let mut identity: Option<Identity> = None;
    let mut world: Option<BedrockWorld> = None;

    // Такт: пока игрок в мире, раз в 50 мс рассказываем ему, что изменилось.
    let mut tick = tokio::time::interval(std::time::Duration::from_millis(50));

    'session: loop {
        let body = tokio::select! {
            received = inbound.recv() => match received {
                Some(body) => body,
                None => break,
            },
            _ = tick.tick() => {
                if let Some(world) = world.as_mut() {
                    world.tick(&connection);
                }
                continue;
            }
        };

        let Some(packets) = codec::unbatch(&body, connection.compressed) else {
            log_debug!("Bedrock: {} прислал нечитаемую обёртку", addr);
            continue;
        };

        for packet in packets {
            let Some((id, payload)) = codec::packet_id(&packet) else {
                continue;
            };

            match id {
                REQUEST_NETWORK_SETTINGS => {
                    let protocol = In::new(payload).i32_be().unwrap_or(0);

                    if protocol != super::PROTOCOL {
                        log_info!("Bedrock: {} с протоколом {} (нужен {}) — не пускаем", addr, protocol, super::PROTOCOL);
                        let mut status = Out::packet(PLAY_STATUS);
                        status.i32_be(if protocol < super::PROTOCOL { 1 } else { 2 });
                        connection.send_one(status);
                        connection.close();
                        return;
                    }

                    let mut settings = Out::packet(NETWORK_SETTINGS);
                    settings
                        .lu16(COMPRESSION_THRESHOLD as u16)
                        .lu16(codec::DEFLATE as u16)
                        .bool(false)
                        .u8(0)
                        .lf32(0.0);
                    connection.send_one(settings);
                    connection.compressed = true;
                }
                LOGIN => {
                    let Some(who) = read_login(payload) else {
                        log_info!("Bedrock: {} прислал нечитаемый вход", addr);
                        log_debug!("Bedrock: вход начинается так: {}", describe_login(payload));
                        connection.close();
                        return;
                    };

                    log_info!("{}[/{}] вошёл с Bedrock", who.name, addr);
                    log_debug!("Bedrock: {} — XUID «{}» (пока не проверяется)", who.name, who.xuid);

                    let mut status = Out::packet(PLAY_STATUS);
                    status.i32_be(0);
                    let mut info = Out::packet(RESOURCE_PACKS_INFO);
                    info.bool(false).bool(false).bool(false).bool(false).uuid([0; 16]).string("").li16(0);
                    connection.send(&[status.bytes, info.bytes]);
                    identity = Some(who);
                }
                RESOURCE_PACK_RESPONSE => {
                    let status = In::new(payload).u8().unwrap_or(0);

                    match status {
                        // «Всё есть» — стопка наборов (пустая).
                        3 => {
                            let mut stack = Out::packet(RESOURCE_PACK_STACK);
                            stack.bool(false).varint(0).string(super::GAME_VERSION).li32(0).bool(false).bool(false);
                            connection.send_one(stack);
                        }
                        // «Готово» — в мир.
                        4 => {
                            let Some(who) = identity.clone() else {
                                connection.close();
                                return;
                            };

                            match BedrockWorld::enter(Arc::clone(&shared), &connection, who) {
                                Some(entered) => world = Some(entered),
                                None => {
                                    connection.close();
                                    return;
                                }
                            }
                        }
                        _ => {}
                    }
                }
                // Клиент уходит сам — не ждём тайм-аута RakNet.
                DISCONNECT => break 'session,
                _ => {
                    if let Some(world) = world.as_mut() {
                        world.handle(&connection, id, payload);
                    }
                }
            }
        }
    }

    if let Some(world) = world {
        world.leave();
    }
}

/// Разбирает пакет Login: имя, UUID и XUID из цепочки JWT.
///
/// С 1.21.90 цепочка лежит строкой в поле `Certificate` JSON-объекта
/// (proto.yml, LoginTokens). Подписи пока не проверяются — это вход
/// без проверки Xbox Live, как `online-mode=false` у Java.
fn read_login(payload: &[u8]) -> Option<Identity> {
    let mut reader = In::new(payload);
    let _protocol = reader.i32_be()?;
    let length = reader.varint()? as usize;
    let tokens = reader.take(length)?;
    let mut tokens = In::new(tokens);
    let identity = tokens.little_string()?;

    // Второй JWT — данные клиента, среди них скин.
    let look = tokens
        .little_string()
        .and_then(|client| client.split('.').nth(1).and_then(base64_url))
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|json| super::skin::from_client_data(&json));

    let outer: serde_json::Value = serde_json::from_str(&identity).ok()?;
    log_debug!("Bedrock: устройство входа: {}", describe_tokens(&outer));

    // Новый вид (клиенты 26.x): всё о игроке — в JWT поля `Token`:
    // `xname` — имя, `xid` — XUID.
    //
    // UUID, присланному клиентом (`identity`), не верим: подписи пока не
    // проверяются, и подделанный клиент назвался бы UUID владельца из
    // ops.json. UUID игрока Bedrock строится сервером из XUID (или ника) —
    // в пространстве, которое с UUID игроков Java не пересекается.
    if let Some(token) = outer.get("Token").and_then(|t| t.as_str())
        && let Some(json) = token
            .split('.')
            .nth(1)
            .and_then(base64_url)
            .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        && let Some(name) = json.get("xname").and_then(|n| n.as_str())
    {
        let xuid = json.get("xid").and_then(|x| x.as_str()).unwrap_or("").to_string();
        let name = java_safe_name(name);
        let uuid = uuid_for(&xuid, &name);

        return Some(Identity { name, uuid, xuid, look });
    }

    // Прежний вид: цепочка JWT, данные игрока — в `extraData`.
    let chain_holder: serde_json::Value = match outer.get("Certificate").and_then(|c| c.as_str()) {
        Some(text) => serde_json::from_str(text).ok()?,
        None => outer.clone(),
    };
    let chain = chain_holder.get("chain")?.as_array()?;

    for token in chain.iter().rev() {
        let token = token.as_str()?;
        let payload = token.split('.').nth(1)?;
        let json: serde_json::Value = serde_json::from_slice(&base64_url(payload)?).ok()?;

        if let Some(extra) = json.get("extraData") {
            let name = java_safe_name(extra.get("displayName")?.as_str()?);
            let xuid = extra.get("XUID").and_then(|x| x.as_str()).unwrap_or("").to_string();
            let uuid = uuid_for(&xuid, &name);

            return Some(Identity { name, uuid, xuid, look });
        }
    }

    None
}

/// Для отладки: как устроены жетоны входа — без личных данных. Заголовки
/// JWT, издатель, аудитория, срок и имена полей: по ним видно, как проверять
/// подпись, когда входит клиент с учётной записью Xbox.
fn describe_tokens(outer: &serde_json::Value) -> String {
    const PUBLIC: [&str; 5] = ["iss", "aud", "exp", "nbf", "iat"];

    let describe_jwt = |token: &str| {
        let mut parts = token.split('.');
        let header = parts.next().and_then(base64_url).map(|b| String::from_utf8_lossy(&b).into_owned()).unwrap_or_default();
        let claims: serde_json::Value = parts
            .next()
            .and_then(base64_url)
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        let fields: Vec<String> = claims
            .as_object()
            .map(|o| {
                o.iter()
                    .map(|(key, value)| if PUBLIC.contains(&key.as_str()) { format!("{}={}", key, value) } else { key.clone() })
                    .collect()
            })
            .unwrap_or_default();
        format!("заголовок {} поля [{}]", header, fields.join(", "))
    };

    let mut out = Vec::new();

    for (key, value) in outer.as_object().into_iter().flatten() {
        match (key.as_str(), value) {
            ("Token", serde_json::Value::String(token)) if !token.is_empty() => out.push(format!("Token: {}", describe_jwt(token))),
            ("Certificate", serde_json::Value::String(text)) => {
                let chain: Vec<String> = serde_json::from_str::<serde_json::Value>(text)
                    .ok()
                    .and_then(|c| c.get("chain").and_then(|c| c.as_array()).cloned())
                    .unwrap_or_default()
                    .iter()
                    .filter_map(|t| t.as_str().map(describe_jwt))
                    .collect();
                out.push(format!("Certificate: {} звеньев: {}", chain.len(), chain.join(" | ")));
            }
            (key, value) => out.push(format!("{}={}", key, value.to_string().chars().take(40).collect::<String>())),
        }
    }

    out.join("; ")
}

/// Для отладки: что лежит в пакете Login.
fn describe_login(payload: &[u8]) -> String {
    let mut reader = In::new(payload);
    let protocol = reader.i32_be();
    let length = reader.varint();
    let identity = reader.little_string();

    let token = identity
        .as_deref()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(text).ok())
        .and_then(|json| json.get("Token").and_then(|t| t.as_str()).map(str::to_string))
        .and_then(|token| token.split('.').nth(1).and_then(base64_url))
        .map(|bytes| String::from_utf8_lossy(&bytes).chars().take(1500).collect::<String>())
        .unwrap_or_default();

    format!("протокол {:?}, длина {:?}, Token: {}", protocol, length, token)
}

/// Ник, который примут клиенты Java: у gamertag бывают пробелы и суффикс
/// «#1234», а Java ждёт до 16 знаков из букв, цифр и подчёркивания.
fn java_safe_name(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '_' { c } else { '_' })
        .take(16)
        .collect();

    if cleaned.is_empty() { "Bedrock".to_string() } else { cleaned }
}

/// Постоянный UUID игрока Xbox: XUID в младших восьми байтах, старшие нули
/// (у Java-игроков так не бывает — UUID не спутать). Без XUID — по нику.
fn uuid_for(xuid: &str, name: &str) -> [u8; 16] {
    let mut uuid = [0u8; 16];

    match xuid.parse::<u64>() {
        Ok(number) if number != 0 => uuid[8..].copy_from_slice(&number.to_be_bytes()),
        _ => {
            // FNV-1a по нику двумя проходами — просто стабильные 16 байт.
            for (half, seed) in [(0usize, 0xcbf2_9ce4_8422_2325u64), (8, 0x6c62_272e_07bb_0142)] {
                let mut hash = seed;
                for byte in name.to_lowercase().bytes() {
                    hash = (hash ^ byte as u64).wrapping_mul(0x0000_0100_0000_01b3);
                }
                uuid[half..half + 8].copy_from_slice(&hash.to_be_bytes());
            }
            uuid[6] = (uuid[6] & 0x0f) | 0x30; // версия 3: «по имени»
            uuid[8] = (uuid[8] & 0x3f) | 0x80;
        }
    }

    uuid
}

/// Base64 — и в варианте для URL (как в JWT), и обычный.
pub(crate) fn base64_url(text: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(text.len() * 3 / 4);
    let mut buffer = 0u32;
    let mut bits = 0;

    for byte in text.bytes() {
        let value = match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'-' | b'+' => 62,
            b'_' | b'/' => 63,
            b'=' => break,
            _ => return None,
        } as u32;

        buffer = (buffer << 6) | value;
        bits += 6;

        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
        }
    }

    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_url_decodes() {
        assert_eq!(base64_url("eyJhIjoxfQ"), Some(br#"{"a":1}"#.to_vec()));
    }

    #[test]
    fn xbox_uuid_comes_from_xuid() {
        let uuid = uuid_for("2535425101461547", "Кто-то");
        assert_eq!(&uuid[..8], &[0; 8]);
        assert_eq!(u64::from_be_bytes(uuid[8..].try_into().expect("8 байт")), 2535425101461547);
        assert_eq!(uuid_for("", "Steve"), uuid_for("", "steve"));
        assert_ne!(uuid_for("0", "Steve"), uuid_for("0", "Alex"));
    }

    #[test]
    fn gamertags_become_java_names() {
        assert_eq!(java_safe_name("Sheriff Anya#1234"), "Sheriff_Anya_123");
        assert_eq!(java_safe_name("SheriffAnya6650"), "SheriffAnya6650");
        assert_eq!(java_safe_name("   "), "___");
        assert_eq!(java_safe_name(""), "Bedrock");
    }

}
