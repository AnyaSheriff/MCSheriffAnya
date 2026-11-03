// Модуль сетевого слоя сервера.
//
// Отвечает за приём подключений, чтение/запись пакетов протокола Minecraft
// и управление состояниями соединения (handshake, status, login, play).

/// Внутренний формат игровых пакетов для адаптеров разных клиентов.
/// Основная версия мира задаётся проверенными настройками и каталогом,
/// а не этой исторической константой протокола775.
pub const NATIVE_VERSION: mcsa_protocol::JavaVersion = mcsa_protocol::JavaVersion::V26_1_2;

pub mod listener;
pub mod varint;
pub mod types;
pub mod packet;
pub mod nbt;
pub mod handshake;
pub mod status;
pub mod login;
pub mod configuration;
pub mod registries;
pub mod chat_types;
pub mod play;
pub mod text_info;

pub mod keep_alive;

pub mod connection;
pub mod online;

mod inbound_runtime;
pub mod proxy_protocol;
