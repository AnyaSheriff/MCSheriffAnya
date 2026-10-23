//! A private client view: no playerdata, session lease, player list or plugins.
use super::*;
use crate::auth::{self, Identity, Service};
use std::net::IpAddr;
use tokio::io::AsyncWrite;
async fn send<W: AsyncWrite + Unpin>(writer: &mut W, body: Vec<u8>) -> io::Result<()> {
    let mut frame = Vec::new();
    super::super::packet::frame(
        &mut frame,
        &body,
        super::super::packet::compression_threshold(),
    );
    super::super::packet::write_bytes(writer, &frame).await
}
fn message(text: &str) -> Vec<u8> {
    let mut body = encode_varint(SYSTEM_CHAT);
    body.extend(nbt::text_component(text));
    body.push(0);
    body
}
pub async fn authenticate(
    socket: &mut Connection,
    shared: &Shared,
    identity: &Identity,
    ip: IpAddr,
    verified: Option<[u8; 16]>,
) -> io::Result<Option<i32>> {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(90);
    let result = tokio::time::timeout_at(
        deadline,
        run(socket, shared, identity, ip, verified, deadline),
    )
    .await;
    let reason = match result {
        Ok(Ok(Ok(entity))) => return Ok(Some(entity)),
        Ok(Ok(Err(reason))) => reason,
        Ok(Err(e)) => return Err(e),
        Err(_) => auth::dialog::TIMEOUT,
    };
    let mut body = encode_varint(DISCONNECT);
    body.extend(nbt::text_component(reason));
    send(socket, body).await?;
    Ok(None)
}
async fn run(
    socket: &mut Connection,
    shared: &Shared,
    identity: &Identity,
    ip: IpAddr,
    verified: Option<[u8; 16]>,
    deadline: tokio::time::Instant,
) -> io::Result<Result<i32, &'static str>> {
    let service = shared.auth.as_ref().expect("auth");
    let permitted =
        Service::registration_allowed(&shared.ops.lock().expect("ops"), &identity.name, verified);
    if !identity.registered && !permitted {
        return Ok(Err(auth::OP_PROTECTED));
    }
    let hint = if identity.registered {
        "Войди: /login <пароль> (90 секунд)"
    } else {
        "Регистрация: /register <пароль> <пароль> (90 секунд)"
    };
    let mut player = PlayerState::new(
        identity.uuid,
        identity.name.clone(),
        None,
        None,
        0,
        shared.next_reader(),
        (0.5, 80.0, 0.5),
    );
    player.entity_id = shared.next_entity_id();
    player.x = 0.5;
    player.y = 80.0;
    player.z = 0.5;
    {
        let mut out = Outgoing::new(socket);
        send_login(
            &mut out,
            Dimension::Overworld,
            GAME_MODE_ADVENTURE,
            player.entity_id,
            2,
            2,
        )
        .await?;
        let mut ability = encode_varint(PLAYER_ABILITIES);
        ability.push(7);
        push_f32(&mut ability, 0.0);
        push_f32(&mut ability, 0.0);
        write_body(&mut out, ability).await?;
        send_player_position(&mut out, &mut player).await?;
        vitals::send_health(&mut out, &player.vit).await?;
        send_default_spawn_position(&mut out, (0.5, 80.0, 0.5)).await?;
        send_game_event_start_waiting(&mut out).await?;
        send_center_chunk(&mut out, (0, 0)).await?;
        send_chunks_around(
            &mut out,
            shared,
            Dimension::Overworld,
            (0, 0),
            &mut SentChunks::new(),
            (1, 2),
            usize::MAX,
        )
        .await?;
        let mut effect = encode_varint(0x84);
        for value in [
            player.entity_id,
            crate::survival::EffectKind::Blindness.id(),
            0,
            1800,
        ] {
            effect.extend(encode_varint(value));
        }
        effect.push(0);
        write_body(&mut out, effect).await?;
        let mut title = encode_varint(0x72);
        title.extend(nbt::text_component(hint));
        write_body(&mut out, title).await?;
        send_system_chat(&mut out, hint).await?;
        out.flush().await?;
    }
    let (mut reader, mut writer) = tokio::io::split(socket);
    let mut timer = tokio::time::interval(Duration::from_secs(2));
    let mut errors = 0;
    loop {
        // Keep the same read future alive across timer ticks: partial frames are preserved.
        let packet = {
            let read = read_packet(&mut reader);
            tokio::pin!(read);
            loop {
                tokio::select! {
                    result = &mut read => break result?,
                    _ = timer.tick() => {
                        let mut keep = encode_varint(KEEP_ALIVE); push_i64(&mut keep, auth::now() as i64); send(&mut writer, keep).await?;
                        send(&mut writer, position_body(&mut player, Sync::Absolute)).await?;
                    }
                }
            }
        };
        if !is_chat_command(packet.id) {
            if matches!(
                packet.id,
                SET_PLAYER_POSITION | SET_PLAYER_POSITION_AND_ROTATION
            ) && read_position(packet.payload()).is_some_and(|(x, y, z)| {
                (x - player.x).abs() > 0.000001
                    || (y - player.y).abs() > 0.000001
                    || (z - player.z).abs() > 0.000001
            }) {
                send(&mut writer, position_body(&mut player, Sync::Absolute)).await?;
            }
            continue;
        }
        let Some(command) = Cursor::new(packet.payload()).string() else {
            continue;
        };
        // Only these two commands are parsed; nothing reaches command dispatch or logging.
        let words: Vec<_> = command.split_whitespace().collect();
        let (password, repeat) = match words.as_slice() {
            ["login", password] if identity.registered => (*password, None),
            ["register", password, repeat] if !identity.registered => (*password, Some(*repeat)),
            _ => {
                send(&mut writer, message(hint)).await?;
                continue;
            }
        };
        let permitted = Service::registration_allowed(
            &shared.ops.lock().expect("ops"),
            &identity.name,
            verified,
        );
        let service = service.clone();
        let identity = identity.clone();
        let password = zeroize::Zeroizing::new(password.to_owned());
        let repeat = repeat.map(|s| zeroize::Zeroizing::new(s.to_owned()));
        let result = tokio::task::spawn_blocking(move || {
            service.submit_before(
                &identity,
                ip,
                &password,
                repeat.as_ref().map(|s| s.as_str()),
                permitted,
                Some(deadline),
            )
        })
        .await
        .map_err(io::Error::other)?;
        match result {
            Ok(()) => {
                let mut remove = encode_varint(0x4E);
                remove.extend(encode_varint(player.entity_id));
                remove.extend(encode_varint(crate::survival::EffectKind::Blindness.id()));
                send(&mut writer, remove).await?;
                let mut clear = encode_varint(0x0E);
                clear.push(1);
                send(&mut writer, clear).await?;
                send(&mut writer, message("Вход выполнен")).await?;
                return Ok(Ok(player.entity_id));
            }
            Err(error) => {
                if error == "Неверный пароль" {
                    errors += 1;
                }
                if errors >= 3 {
                    return Ok(Err("Три неверных пароля. Соединение закрыто."));
                }
                if matches!(
                    error,
                    auth::BLOCKED
                        | auth::OP_PROTECTED
                        | auth::dialog::TIMEOUT
                        | "Аккаунт изменился. Зайди ещё раз."
                        | "Не удалось сохранить аккаунт"
                        | "С этого IP уже зарегистрировано 3 аккаунта за сутки"
                ) {
                    return Ok(Err(error));
                }
                send(&mut writer, message(error)).await?;
            }
        }
    }
}
