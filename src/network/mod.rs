// Модуль сетевого слоя сервера.
//
// Отвечает за приём подключений, чтение/запись пакетов протокола Minecraft
// и управление состояниями соединения (handshake, status, login, play).

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
