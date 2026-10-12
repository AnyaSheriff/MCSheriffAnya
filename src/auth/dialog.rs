// Вики (до 26.1.2): Dialog oldid=3512615, «dynamic/custom»;
// Java Edition protocol/Packets oldid=3501809, «Inline definition».
// Номера сверены с minecraft-data 26.1 protocol.json (protocol 775).
use std::{io, net::IpAddr, sync::Arc};
use tokio::io::{AsyncRead, AsyncWrite};
use crate::network::{nbt::Compound, packet, varint::encode_varint};
use super::{Identity, Service};

pub const SHOW: i32 = 0x12;
pub const CLEAR: i32 = 0x11;
pub const CLICK: i32 = 0x08;
pub const TIMEOUT: &str = "Время входа истекло (90 секунд). Зайди ещё раз.";

fn text(value: &str, colour: &str) -> Compound { Compound::new().string("text", value).string("color", colour) }
fn button(label: &str, colour: &str, id: &str) -> Compound {
    Compound::new().compound("label", text(label, colour)).int("width", 160)
        .compound("action", Compound::new().string("type", "minecraft:dynamic/custom").string("id", id))
}
pub fn encode(registered: bool, error: Option<&str>) -> Vec<u8> {
    let mut body = vec![Compound::new().string("type", "minecraft:plain_message")
        .compound("contents", text("Введите пароль", "gray")).int("width", 320)];
    if let Some(error) = error {
        body.push(Compound::new().string("type", "minecraft:plain_message")
            .compound("contents", text(error, "red")).int("width", 320));
    }
    let input = |key, label| Compound::new().string("type", "minecraft:text").string("key", key)
        .compound("label", text(label, "white")).int("max_length", 64).int("width", 320);
    let mut inputs = vec![input("password", "Пароль")];
    if !registered { inputs.push(input("repeat", "Повторите пароль")); }
    let title = if registered { "Вход" } else { "Регистрация" };
    Compound::new().string("type", "minecraft:confirmation")
        .compound("title", text(&format!("MCSheriffAnya • {title}"), "gold"))
        .boolean("can_close_with_escape", false).boolean("pause", false)
        .string("after_action", "wait_for_response").compounds("body", body).compounds("inputs", inputs)
        .compound("yes", button(if registered { "Войти" } else { "Зарегистрироваться" }, "green", if registered { "mcsheriffanya:login" } else { "mcsheriffanya:register" }))
        .compound("no", button("Выйти", "red", "mcsheriffanya:exit")).encode_network()
}

pub struct Click { pub id: String, pub password: String, pub repeat: Option<String> }
fn invalid() -> io::Error { io::Error::new(io::ErrorKind::InvalidData, "неверный Custom Click Action") }
// Строгий ограниченный разбор: только наши два строковых поля, без вложенных
// тегов, дубликатов и лишнего хвоста. NBT использует modified UTF-8.
fn string(rest: &mut &[u8]) -> io::Result<String> {
    let (size, tail) = rest.split_at_checked(2).ok_or_else(invalid)?;
    let size = u16::from_be_bytes([size[0], size[1]]) as usize;
    if size > 384 { return Err(invalid()); }
    let (bytes, tail) = tail.split_at_checked(size).ok_or_else(invalid)?;
    let mut units = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i]; i += 1;
        let unit = if (1..=127).contains(&b) { b as u16 }
        else if b & 0xe0 == 0xc0 {
            let c = *bytes.get(i).ok_or_else(invalid)?; i += 1;
            if c & 0xc0 != 0x80 { return Err(invalid()); }
            let u = ((b as u16 & 31) << 6) | (c as u16 & 63);
            if u < 128 && u != 0 { return Err(invalid()); } u
        } else if b & 0xf0 == 0xe0 {
            let c = *bytes.get(i).ok_or_else(invalid)?;
            let d = *bytes.get(i + 1).ok_or_else(invalid)?; i += 2;
            if c & 0xc0 != 0x80 || d & 0xc0 != 0x80 { return Err(invalid()); }
            let u = ((b as u16 & 15) << 12) | ((c as u16 & 63) << 6) | (d as u16 & 63);
            if u < 2048 { return Err(invalid()); } u
        } else { return Err(invalid()); };
        units.push(unit);
    }
    *rest = tail;
    String::from_utf16(&units).map_err(|_| invalid())
}
pub fn parse(payload: &[u8]) -> io::Result<Click> {
    if payload.len() > 1024 { return Err(invalid()); }
    let mut offset = 0;
    let id = packet::decode_string(payload, &mut offset)?;
    if !matches!(id.as_str(), "mcsheriffanya:login" | "mcsheriffanya:register" | "mcsheriffanya:exit") { return Err(invalid()); }
    let mut rest = &payload[offset..];
    // Перед NBT настоящий клиент шлёт его длину (VarInt, вики: «Payload
    // size»). Без длины — старая запись наших тестов, её тоже принимаем.
    if let Ok((size, used)) = packet::decode_varint(rest)
        && size >= 0 && rest.len() == used + size as usize
        && rest.get(used).is_some_and(|tag| matches!(tag, 0 | 10))
    {
        rest = &rest[used..];
    }
    if id == "mcsheriffanya:exit" && rest == [0] { return Ok(Click { id, password: String::new(), repeat: None }); }
    rest = rest.strip_prefix(&[10]).ok_or_else(invalid)?;
    let mut password = None;
    let mut repeat = None;
    loop {
        let (&tag, tail) = rest.split_first().ok_or_else(invalid)?; rest = tail;
        if tag == 0 { break; }
        if tag != 8 { return Err(invalid()); }
        let key = string(&mut rest)?;
        let value = string(&mut rest)?;
        if value.chars().count() > 64 { return Err(invalid()); }
        match key.as_str() {
            "password" if password.is_none() => password = Some(value),
            "repeat" if repeat.is_none() => repeat = Some(value),
            _ => return Err(invalid()),
        }
    }
    if !rest.is_empty() { return Err(invalid()); }
    let password = if id == "mcsheriffanya:exit" { password.unwrap_or_default() } else { password.ok_or_else(invalid)? };
    Ok(Click { id, password, repeat })
}
async fn send<W: AsyncWrite + Unpin>(stream: &mut W, id: i32, payload: &[u8]) -> io::Result<()> {
    let mut body = encode_varint(id); body.extend_from_slice(payload);
    let mut frame = Vec::new(); packet::frame(&mut frame, &body, packet::compression_threshold());
    packet::write_bytes(stream, &frame).await
}
pub async fn disconnect<W: AsyncWrite + Unpin>(stream: &mut W, reason: &str) -> io::Result<()> {
    send(stream, 2, &crate::network::nbt::text_component(reason)).await
}

pub async fn authenticate<S: AsyncRead + AsyncWrite + Unpin>(
    stream: &mut S, service: &Arc<Service>, identity: &Identity, ip: IpAddr,
    ops: &std::sync::Mutex<crate::ops::Ops>, verified_uuid: Option<[u8; 16]>, deadline: tokio::time::Instant,
) -> io::Result<bool> {
    let result = tokio::time::timeout_at(deadline, async {
        let mut errors = 0;
        let mut message = None;
        loop {
            if service.blocked(ip) { return Ok(Err(super::BLOCKED)); }
            let permitted = Service::registration_allowed(&ops.lock().expect("ops"), &identity.name, verified_uuid);
            if !identity.registered && !permitted { return Ok(Err(super::OP_PROTECTED)); }
            send(stream, SHOW, &encode(identity.registered, message.as_deref())).await?;
            let click = loop {
                let packet = packet::read_packet(stream).await?;
                match packet.id {
                    CLICK => break parse(packet.payload())?,
                    // Пакеты настроек/каналов не завершают парольный барьер.
                    0 | 1 | 2 | 4 | 5 | 6 | 7 => continue,
                    _ => return Ok(Err("Ожидался вход с паролем")),
                }
            };
            if click.id == "mcsheriffanya:exit" { return Ok(Err("Выход из MCSheriffAnya")); }
            if (click.id == "mcsheriffanya:login") != identity.registered { return Ok(Err("Неверное действие входа")); }
            let service = service.clone(); let identity = identity.clone();
            let outcome = tokio::task::spawn_blocking(move || {
                if tokio::time::Instant::now() >= deadline { return Err(TIMEOUT); }
                service.submit_before(&identity, ip, &click.password, click.repeat.as_deref(), permitted, Some(deadline))
            }).await.map_err(io::Error::other)?;
            match outcome {
                Ok(()) => { send(stream, CLEAR, &[]).await?; return Ok(Ok(())); }
                Err(error) => {
                    if error == "Неверный пароль" { errors += 1; }
                    if errors >= 3 { return Ok(Err("Три неверных пароля. Соединение закрыто.")); }
                    if matches!(error, super::BLOCKED | super::OP_PROTECTED | TIMEOUT | "Аккаунт изменился. Зайди ещё раз." | "Не удалось сохранить аккаунт" | "С этого IP уже зарегистрировано 3 аккаунта за сутки") { return Ok(Err(error)); }
                    message = Some(format!("{error}. Осталось попыток: {}", 3 - errors));
                }
            }
        }
    }).await;
    let reason = match result {
        Ok(Ok(Ok(()))) => return Ok(true),
        Ok(Ok(Err(reason))) => reason,
        Ok(Err(error)) => return Err(error),
        Err(_) => TIMEOUT,
    };
    disconnect(stream, reason).await?;
    Ok(false)
}
