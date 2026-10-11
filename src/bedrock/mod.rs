// Вход для игроков Bedrock Edition — свой, прямо в ядре, без прокси.
//
// Версия — парная к Java 26.1.2: обновление Tiny Takeover вышло 24 марта 2026
// сразу как Java 26.1 и Bedrock 26.10 (minecraft.wiki). У Bedrock 26.10–26.13
// сетевой протокол 944. Устройство:
// - raknet — транспорт поверх UDP;
// - codec — числа, строки, пакеты-обёртки со сжатием;
// - session — вход: настройки сети, Login, наборы ресурсов;
// - auth — проверка входа Xbox Live (подписи жетонов из Login);
// - world — появление в мире, чанки, движение, стройка;
// - inventory — инвентарь, которым распоряжается сервер (Item Stack Request);
// - containers — окна хранилищ, блок-акторы и крышка сундука;
// - pistons — основания поршней: блок-актор рычага;
// - effects — звуки, частицы, подожжённый динамит;
// - tools — костная мука, мотыга, вёдра, яйца призыва;
// - frames — рамки для предметов: блоки с блок-актором вместо сущностей;
// - skin и png — скины: свои у игроков Bedrock, скачанные у игроков Java.

pub mod auth;
pub mod bds_profile;
pub mod codec;
pub mod containers;
pub mod effects;
pub mod frames;
pub mod health;
pub mod inventory;
pub mod vehicle_data;
pub mod entity_events;
pub mod entity_state;
pub mod entity_gear;
pub mod entity_links;
pub mod mobdata;
pub mod mobprops;
pub mod pistons;
pub mod png;
pub mod player_state;
pub mod raknet;
pub mod session;
pub mod skin;
pub mod stations;
pub mod tables;
pub mod things;
pub mod projectiles;
pub mod trade;
pub mod tools;
pub mod world;

use std::sync::Arc;

use tokio::sync::mpsc;

use crate::shared::Shared;
use crate::{log_error, log_info};

/// Сетевой протокол Bedrock 26.10–26.13.
pub const PROTOCOL: i32 = 944;

/// Какие версии Bedrock пускаются — для справок.
pub const VERSIONS: &str = "26.10–26.13";

/// Версия игры в строке.
pub const GAME_VERSION: &str = "1.26.10";

/// Запускает приём Bedrock на порту из настроек (`bedrock-port`); 0 — выключено.
pub fn start(shared: Arc<Shared>) {
    let port = shared.settings.bedrock_port;

    if port == 0 {
        return;
    }

    let (events_tx, mut events_rx) = mpsc::unbounded_channel();
    let status_shared = Arc::clone(&shared);
    let status = move || raknet::Status {
        motd: status_shared.properties.motd.clone(),
        world: "MCSheriffAnya".to_string(),
        online: status_shared.players.lock().map(|players| players.online()).unwrap_or(0),
        max: status_shared.properties.max_players.max(0) as usize,
        port,
    };

    tokio::spawn(async move {
        if let Err(error) = raknet::serve(port, status, events_tx).await {
            log_error!("Bedrock: не удалось открыть UDP-порт {}: {}", port, error);
        }
    });

    tokio::spawn(async move {
        while let Some(event) = events_rx.recv().await {
            match event {
                raknet::Event::Connected { addr, inbound, outbound } => {
                    let shared = Arc::clone(&shared);
                    tokio::spawn(session::run(shared, addr, inbound, outbound));
                }
            }
        }
    });

    log_info!("Bedrock: приём на *:{} (UDP), версия {} (протокол {})", port, GAME_VERSION, PROTOCOL);
}

mod signs;
mod lectern;
