// Модуль сетевого слоя сервера.
//
// Отвечает за приём подключений, чтение/запись пакетов протокола Minecraft
// и управление состояниями соединения (handshake, status, login, play).

/// Версия игры, под которую сделано ядро, и номер её протокола.
///
/// Лежат здесь, а не в одном только ответе списку серверов: по ним же
/// решается, пускать ли клиента, и по ним же объясняется отказ.
pub const NATIVE_VERSION: mcsa_protocol::JavaVersion = mcsa_protocol::JavaVersion::V26_1_2;
pub const PROTOCOL_VERSION: i32 = NATIVE_VERSION.protocol();
pub const VERSION_NAME: &str = NATIVE_VERSION.name();

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
