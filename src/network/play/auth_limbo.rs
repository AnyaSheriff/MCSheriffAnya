// Отдельный замороженный мир: ни playerdata, ни общей сущности, ни плагинов.
use super::*;
use zeroize::{Zeroize, Zeroizing};

pub(crate) async fn chat_login(socket: &mut Connection, shared: &Shared,
    identity: &crate::auth::Identity, ip: std::net::IpAddr, verified: Option<[u8; 16]>,
    deadline: tokio::time::Instant,
) -> io::Result<bool> {
    let service = shared.auth.as_ref().ok_or_else(|| io::Error::other("Нет парольной службы"))?;
    let version = socket.client_version();
    let probe = socket.probe_776_enabled();
    socket.content = shared.catalog().content_codec();
    let content = socket.content;
    let mut out = Outgoing::new(socket);
    let mut player = PlayerState::new(identity.uuid, identity.name.clone(), None, None, 0, 0, (0.5, 65.0, 0.5));
    player.entity_id = 1;
    player.game_mode = GAME_MODE_SPECTATOR;
    let world = Mutex::new(World::in_memory_with_catalog(shared.catalog()));
    send_login(&mut out, Dimension::Overworld, GAME_MODE_SPECTATOR, 1, 2, 2, false).await?;
    send_abilities(&mut out, GAME_MODE_SPECTATOR).await?;
    send_game_event_start_waiting(&mut out).await?;
    send_center_chunk(&mut out, (0, 0)).await?;
    for z in -1..=1 { for x in -1..=1 {
        send_chunk(&mut out, &world, x, z, &ChunkLight::open_sky()).await?;
    } }
    send_player_position(&mut out, &mut player).await?;
    let prompt = if identity.registered { "Вход: /login <пароль>" } else { "Регистрация: /register <пароль> <повтор>" };
    send_system_chat(&mut out, prompt).await?;
    out.flush().await?;
    let result = tokio::time::timeout_at(deadline, async {
        let mut errors = 0;
        loop {
            if service.blocked(ip) { return Ok(Err(crate::auth::BLOCKED)); }
            let permitted = crate::auth::Service::registration_allowed(&shared.ops.lock().expect("ops"), &identity.name, verified);
            if !identity.registered && !permitted { return Ok(Err(crate::auth::OP_PROTECTED)); }
            let mut packet = super::super::packet::read_play_packet_with_content(out.stream(), version, None, probe, content).await?;
            // Никаких обычных обработчиков чата/команд; тело стирается до проверки.
            let command = if matches!(packet.id, SERVERBOUND_CHAT_COMMAND | SERVERBOUND_CHAT_MESSAGE) {
                Cursor::new(packet.payload()).string().map(Zeroizing::new)
            } else { None };
            packet.body.zeroize();
            let Some(command) = command else {
                if matches!(packet.id, SET_PLAYER_POSITION | SET_PLAYER_POSITION_AND_ROTATION | SET_PLAYER_ROTATION | SET_PLAYER_ON_GROUND) {
                    send_player_position(&mut out, &mut player).await?;
                    out.flush().await?;
                }
                continue;
            };
            let words: Vec<&str> = command.trim_start_matches('/').split_whitespace().collect();
            let credentials = match words.as_slice() {
                ["login", password] if identity.registered => Some((*password, None)),
                ["register", password, repeat] if !identity.registered => Some((*password, Some(*repeat))),
                _ => None,
            };
            let Some((password, repeat)) = credentials else {
                send_system_chat(&mut out, prompt).await?; out.flush().await?; continue;
            };
            let password = Zeroizing::new(password.to_string());
            let repeat = repeat.map(|s| Zeroizing::new(s.to_string()));
            let service = service.clone(); let identity = identity.clone();
            let outcome = tokio::task::spawn_blocking(move || service.submit_before(&identity, ip, &password,
                repeat.as_ref().map(|s| s.as_str()), permitted, Some(deadline))).await.map_err(io::Error::other)?;
            match outcome {
                Ok(()) => return Ok(Ok(())),
                Err(error) => {
                    if error == "Неверный пароль" { errors += 1; }
                    if errors >= 3 { return Ok(Err("Три неверных пароля. Соединение закрыто.")); }
                    if matches!(error, crate::auth::BLOCKED | crate::auth::OP_PROTECTED | crate::auth::dialog::TIMEOUT |
                        "Аккаунт изменился. Зайди ещё раз." | "Не удалось сохранить аккаунт" | "С этого IP уже зарегистрировано 3 аккаунта за сутки") {
                        return Ok(Err(error));
                    }
                    send_system_chat(&mut out, error).await?; out.flush().await?;
                }
            }
        }
    }).await;
    let reason = match result {
        Ok(Ok(Ok(()))) => {
            // Возврат в Configuration сбрасывает клиентский мир лимбо до настоящего Login.
            out.send(&encode_varint(0x76)).await?; out.flush().await?;
            loop {
                let packet = tokio::time::timeout_at(deadline,
                    super::super::packet::read_play_packet_with_content(out.stream(), version, None, probe, content)).await
                    .map_err(|_| io::Error::new(io::ErrorKind::TimedOut, crate::auth::dialog::TIMEOUT))??;
                if packet.id == 0x10 { return Ok(true); }
            }
        }
        Ok(Ok(Err(reason))) => reason,
        Ok(Err(error)) => return Err(error),
        Err(_) => crate::auth::dialog::TIMEOUT,
    };
    send_disconnect(&mut out, reason).await?; out.flush().await?;
    Ok(false)
}
