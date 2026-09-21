// Модуль сетевого слоя сервера.
//
// Отвечает за приём подключений, чтение/запись пакетов протокола Minecraft
// и управление состояниями соединения (handshake, status, login, play).

/// Версия игры, под которую сделано ядро, и номер её протокола.
///
/// Лежат здесь, а не в одном только ответе списку серверов: по ним же
/// решается, пускать ли клиента, и по ним же объясняется отказ.
pub const PROTOCOL_VERSION: i32 = 775;
pub const VERSION_NAME: &str = "26.1.2";

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
pub mod play;
