// Фаза Play: игрок в мире.
//
// Мир пока пустой: блоки только те, что поставили игроки (в начале — один
// блок камня). Ни генерации, ни физики нет; сервер запоминает, кто где стоит
// и какие блоки изменились.
//
// Порядок входа (по документации протокола):
//   1. Login (Play) — сообщает клиенту измерение, режим игры и параметры мира
//   2. Synchronize Player Position — ставит игрока в точку; клиент отвечает
//      Confirm Teleportation
//   3. Player Info Update — запись игрока (именно отсюда клиент берёт режим игры)
//   4. Set Default Spawn Position — точка возрождения
//   5. Game Event 13 — «начинаю ждать чанки»: без него клиент не закроет
//      экран загрузки, даже если чанки пришли
//   6. Set Center Chunk — какой чанк клиент считает центральным
//   7. Chunk Data and Update Light — сами чанки
//
// Тип измерения берётся из реестра, отправленного в фазе Configuration:
// там ровно одна запись (minecraft:overworld), значит её ID = 0. Высоту мира
// клиент берёт из того же измерения (обычный overworld: от -64, высота 384),
// поэтому секций в чанке ровно 24.

use std::collections::{HashMap, HashSet};
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tokio::net::TcpStream;
use tokio::time::timeout;

use super::nbt;
use super::packet::{Outgoing, PacketInfo, decode_varint, is_client_disconnect, read_packet};
use super::types;
use super::configuration;
use super::varint::{encode_string, encode_varint, encode_varlong};
use crate::blocks;
use crate::combat;
use crate::fluids;
use crate::inventory::{self, Inventory, Stack};
use crate::journal;
use crate::items::{self, Dropped};
use crate::dimension::Dimension;
use crate::commands::{
    self, Effect, GAME_MODE_ADVENTURE, GAME_MODE_CREATIVE, GAME_MODE_SPECTATOR, GAME_MODE_SURVIVAL,
    Source,
};
use crate::hardness_table::UNBREAKABLE;
mod mining;
mod give;
use crate::playerdata::{self, PlayerData};
use crate::players::{Change, Member, Order, Position};
use crate::placing;
use crate::redstone;
use crate::containers;
use crate::window;
use crate::shared::Shared;
use crate::skins::{self, Skin};
use crate::world::light::{ChunkLight, LightJob, LightSection};
use crate::world::{self, World, WorldEvent, WorldSound};
use crate::{log_debug, log_error, log_info};

#[cfg(test)]
mod arthropod_tests;
#[cfg(test)]
mod skeleton_tests;
#[cfg(test)]
mod land_tests;
mod vitals;
mod mobs;
mod land;
mod water;
#[cfg(test)]
mod water_tests;
mod merchant;
mod pvp;
mod bow;
#[cfg(test)]
mod bow_tests;
mod thrown;
mod sleep;
mod farm;
mod debug_stick;
mod mob_effects;
mod buckets;
mod equipment_sound;
mod knowledge_book;
mod station;
mod body;
mod signs;
mod cauldron;
mod block_use;
mod vehicle;
mod horse;
mod glide;
mod progression;
mod tame;
mod gameplay;
mod fishing;
mod maps;
mod brush;
mod small;

pub(crate) use body::{Act, Body, Done, Stance, cart_contents, cart_info, release_villager, store_cart, trade_paid, villager_missing, villager_state};

/// Сторона чанка в блоках. По ней считается, в каком чанке стоит игрок.
const CHUNK_SIZE: i32 = 16;

/// Clientbound Login (Play).
const LOGIN: i32 = 0x31;

/// Clientbound Player Info Update — запись игрока в списке игроков.
const PLAYER_INFO_UPDATE: i32 = 0x46;

/// Что именно сообщается об игроке в Player Info Update: это биты одной маски,
/// и поля идут в порядке возрастания битов.
const PLAYER_INFO_ADD: u8 = 0x01;
const PLAYER_INFO_GAME_MODE: u8 = 0x04;
const PLAYER_INFO_LISTED: u8 = 0x08;

/// Clientbound Keep Alive — «сервер жив».
const KEEP_ALIVE: i32 = 0x2C;

/// Serverbound Keep Alive — ответ клиента на наш Keep Alive.
const SERVERBOUND_KEEP_ALIVE: i32 = 0x1C;

/// Serverbound Set Player Position — игрок сдвинулся.
const SET_PLAYER_POSITION: i32 = 0x1E;

/// На сколько место от клиента может разойтись с серверным после толчка
/// поршнем, чтобы его приняли без сверки: 0,25 (квадрат — 0,0625).
/// Чёрный ящик: сдвинутого на 0,81 оригинал сверяет (piston-push-player),
/// прижатого к стене после 0,2 — нет (piston-push-wall) [проверить].
const SHOVE_TOLERANCE: f64 = 0.25;

/// Serverbound Set Player Position and Rotation — игрок сдвинулся и повернулся.
const SET_PLAYER_POSITION_AND_ROTATION: i32 = 0x1F;

/// Serverbound Set Player Rotation — игрок только повернулся.
const SET_PLAYER_ROTATION: i32 = 0x20;

/// Serverbound Set Player On Ground — игрок встал на землю или оторвался от неё.
const SET_PLAYER_ON_GROUND: i32 = 0x21;

/// minecraft-data 26.1, abilities: бит 1 — творческий полёт.
const SERVERBOUND_ABILITIES: i32 = 0x28;

/// Serverbound Player Action — игрок копает блок или отменяет копание.
const PLAYER_ACTION: i32 = 0x29;

/// Serverbound Use Item On — игрок щёлкнул по блоку предметом в руке.
const USE_ITEM_ON: i32 = 0x42;

/// Program Command Block — кнопка «Готово» в окне командного блока
/// (minecraft-data 26.1, update_command_block): место, команда, режим,
/// признаки.
const PROGRAM_COMMAND_BLOCK: i32 = 0x36;
const USE_ITEM: i32 = 0x43;

/// Serverbound Set Carried Item — игрок выбрал другой слот панели быстрого
/// доступа.
const SET_CARRIED_ITEM: i32 = 0x35;

/// Serverbound Swing Arm — игрок взмахнул рукой (щелчок, удар).
const SWING_ARM: i32 = 0x3F;

/// Clientbound Entity Animation — показать чужой взмах; 0 — главной рукой
/// (minecraft-data, протокол 26.1).
const ENTITY_ANIMATION: i32 = 0x02;
const ANIMATION_SWING_MAIN_ARM: u8 = 0;

/// Clientbound Set Container Content — содержимое окна целиком.
/// Им сервер показывает игроку его инвентарь.
const SET_CONTAINER_CONTENT: i32 = 0x12;

/// Clientbound Set Container Slot — содержимое одного слота.
const SET_CONTAINER_SLOT: i32 = 0x14;

/// Clientbound Set Held Item — какой слот панели выбран.
const SET_HELD_ITEM: i32 = 0x69;

/// Serverbound Click Container — игрок щёлкнул по слоту в открытом окне.
const CLICK_CONTAINER: i32 = 0x12;

/// Clientbound Pickup Item — предмет летит к тому, кто его поднял.
///
/// Мир меняет не он, а Remove Entities следом: этот пакет нужен только
/// для того, чтобы подбор было видно.
const PICK_UP_ITEM: i32 = 0x7C;

/// Номер вида сущности «лежащий предмет» в списке видов, вшитом в клиент.
const ITEM_ENTITY_TYPE: i32 = 71;

/// Номер вида сущности «падающий блок». Он меньше номера предмета, а между
/// 26.1 и 26.2 сдвинулись только те, что идут после предмета, — значит для
/// нашей версии число то же.
const FALLING_BLOCK_ENTITY_TYPE: i32 = 51;

/// Номер вида сущности «подожжённый динамит» (minecraft-data 26.1,
/// entities.json: `tnt`, id 132).
const TNT_ENTITY_TYPE: i32 = 132;

/// Номер вида сущности «молния» (minecraft-data 26.1, entities.json:
/// `lightning_bolt`, id 77; тот же номер прочёл робот чёрного ящика
/// в пакете оригинала, сценарий lightning-rod).
const LIGHTNING_ENTITY_TYPE: i32 = 77;

/// Свойства подожжённого динамита (minecraft-data 26.1, entities.json,
/// metadataKeys): восьмое — запал (`fuse`), девятое — состояние блока
/// (`block_state`). Виды значений — из protocol.json (entityMetadataEntry):
/// 1 — int (VarInt), 14 — block_state (VarInt).
const TNT_FUSE_PROPERTY: u8 = 8;
const TNT_BLOCK_PROPERTY: u8 = 9;
const PROPERTY_INT: i32 = 1;
const PROPERTY_BLOCK_STATE: i32 = 14;

/// Clientbound Explosion (minecraft-data 26.1, protocol.json: 0x24,
/// `explosion`).
const EXPLOSION: i32 = 0x24;

/// Частицы из реестра частиц 26.1 (protocol.json, тип Particle):
/// большая вспышка взрыва, дымок и дым.
const PARTICLE_EXPLOSION_EMITTER: i32 = 22;
const PARTICLE_EXPLOSION: i32 = 23;
const PARTICLE_POOF: i32 = 59;
/// Вихрь взрыва ветра (minecraft-data 26.1, particles.json).
const PARTICLE_GUST_EMITTER_LARGE: i32 = 26;
/// Малый вихрь взрыва заряда ветра игрока (minecraft-data 26.1: gust_emitter_small 27).
const PARTICLE_GUST_EMITTER_SMALL: i32 = 27;
/// Звук взрыва «порыва ветра» (вики, Wind Burst).
const WIND_BURST_SOUND: &str = "minecraft:entity.wind_charge.wind_burst";
/// Взрыв заряда бриза (вики, Breeze, «Sounds»; чёрный ящик, breeze-charge-flight).
const BREEZE_WIND_BURST_SOUND: &str = "minecraft:entity.breeze.wind_burst";
const PARTICLE_SMOKE: i32 = 62;
const PARTICLE_VIBRATION: i32 = 48;

/// Номер частицы `note` (minecraft-data 26.1, particles.json).
const PARTICLE_NOTE: i32 = 58;

/// Clientbound Level Particles (protocol.json 26.1, `world_particles`).
const LEVEL_PARTICLES: i32 = 0x2F;

/// Дальше этого от взрыва пакет не шлётся: звук громкостью 4 слышен на
/// 4 · 16 = 64 блока (вики, TNT, Sounds: distance=16). [проверить].
const EXPLOSION_HEARD: f64 = 64.0;

/// Номер свойства «что это за предмет» у лежащего предмета.
///
/// У него восемь общих свойств сущности, а это — девятое.
const ITEM_PROPERTY: u8 = 8;

/// Вид значения «предмет» в списке видов свойств сущности.
const PROPERTY_SLOT: i32 = 7;

/// Serverbound Close Container — игрок закрыл окно инвентаря.
const CLOSE_CONTAINER: i32 = 0x13;

/// Click Container Button: кнопка в окне — у кафедры листание и «взять
/// книгу» (протокол 775, `enchant_item` в minecraft-data).
const CLICK_CONTAINER_BUTTON: i32 = 0x11;

/// Rename Item: имя в окне наковальни (протокол 775, `name_item`).
const RENAME_ITEM: i32 = 0x30;

/// Change Container Slot State: игрок включил или выключил ячейку крафтера
/// (протокол 775, `set_slot_state` в minecraft-data: номер ячейки, номер
/// окна, включена ли).
const CHANGE_CONTAINER_SLOT_STATE: i32 = 0x14;

/// Set Container Property: свойство окна — у кафедры открытая страница
/// (свойство 0).
const SET_CONTAINER_PROPERTY: i32 = 0x13;

/// Open Screen: открыть окно хранилища.
const OPEN_SCREEN: i32 = 0x3B;

/// Close Container от сервера: закрыть окно у клиента (хранилище сломали).
const CLOSE_CONTAINER_CLIENTBOUND: i32 = 0x11;

/// Set Cursor Item: что у игрока в курсоре.
const SET_CURSOR_ITEM: i32 = 0x60;

/// World Event: событие, которое клиент проигрывает сам (звук и дым
/// выбрасывателя).
const WORLD_EVENT: i32 = 0x2E;

/// Номер окна, при котором счётчик окон начинается сначала: у оригинала
/// окна нумеруются от 1 до 100 по кругу (вики, Java Edition protocol).
const WINDOW_IDS: i32 = 100;

/// Номер окна, которым обозначается собственный инвентарь игрока.
const PLAYER_WINDOW: i32 = 0;

/// Serverbound Set Creative Mode Slot — содержимое слота инвентаря в креативе.
const SET_CREATIVE_MODE_SLOT: i32 = 0x38;

/// Serverbound Chat Message — игрок написал в чат.
const SERVERBOUND_CHAT_MESSAGE: i32 = 0x09;

/// Serverbound Chat Command — игрок ввёл команду, начиная с косой черты.
/// Клиент присылает команду без самой черты, одним полем.
const SERVERBOUND_CHAT_COMMAND: i32 = 0x07;

/// Clientbound System Chat Message — сообщение в чат от сервера.
const SYSTEM_CHAT: i32 = 0x79;

/// Clientbound Commands — список команд, который клиент показывает в подсказке
/// при вводе косой черты.
const COMMANDS: i32 = 0x10;

/// Clientbound Player Info Remove — игрок пропал из списка игроков.
/// Это отдельный пакет, а не вид Player Info Update.
const PLAYER_INFO_REMOVE: i32 = 0x45;

/// Clientbound Add Entity — «вот сущность, поставь её в мир».
///
/// Без него чужого игрока в мире не видно, даже если он есть в списке
/// игроков: список и мир клиент ведёт отдельно.
const ADD_ENTITY: i32 = 0x01;

/// Clientbound Remove Entities — «этих сущностей в мире больше нет».
/// Удаляется сразу список, поэтому для одного игрока в нём одна запись.
const REMOVE_ENTITIES: i32 = 0x4D;

/// Clientbound Update Entity Position — сущность сдвинулась.
const ENTITY_POSITION: i32 = 0x35;

/// Clientbound Update Entity Position and Rotation — сущность сдвинулась
/// и повернулась.
const ENTITY_POSITION_AND_ROTATION: i32 = 0x36;

/// Clientbound Update Entity Rotation — сущность только повернулась.
const ENTITY_ROTATION: i32 = 0x38;

/// Clientbound Update Time — возраст мира и ход часов.
///
/// С 26.1 время устроено не как раньше: сервер сообщает возраст мира, а
/// дальше по одной записи на каждые «мировые часы» — сколько тактов они
/// показывают и с какой скоростью идут.
const UPDATE_TIME: i32 = 0x71;

/// Как часто сверять время с клиентом: каждый двадцатый такт мира, как в
/// оригинале. Не «раз в секунду по часам соединения» — от этого рассылка
/// съезжает с сетки тактов и приходит то через 20, то через 21 такт.
const TIME_EVERY: i64 = 20;

/// Clientbound Set Equipment — что у сущности в руках и на теле.
///
/// Без него предмет в чужой руке не виден: свой инвентарь клиент знает,
/// а чужой — нет, и показывает пустые руки, пока сервер не скажет иначе.
const SET_EQUIPMENT: i32 = 0x66;

/// Место предмета у сущности: основная рука.
const EQUIPMENT_MAIN_HAND: u8 = 0;

/// Clientbound Set Entity Data — свойства сущности.
///
/// Через него остальным рассказывается, какие части скина игрок показывает:
/// второй слой скина (шапка, куртка, рукава, штанины) виден только отсюда.
const SET_ENTITY_DATA: i32 = 0x63;

/// Номер свойства «показываемые части скина» у игрока.
///
/// Свойства сущности нумеруются подряд по всей её родословной: восемь общих
/// у любой сущности, семь у живой, затем главная рука и части скина. Промах
/// здесь означает, что клиент запишет байт в чужое свойство.
const SKIN_PARTS_PROPERTY: u8 = 16;

/// Сколько чанков в каждую сторону присылается сразу при входе и после
/// переноса — чтобы было на чём стоять. Остальное догружается порциями.
const NEAR_CHUNKS: i32 = 3;

/// Сколько чанков догружать за один круг основного цикла. Круг бывает на
/// каждом такте и на каждом пакете игрока, так что даль приходит быстро,
/// но не заслоняет чат и движение.
const CHUNKS_PER_STEP: usize = 64;

/// Номер свойства «ведущая рука» у игрока — прямо перед частями скина.
const MAIN_HAND_PROPERTY: u8 = 15;

/// Вид значения «рука» (Humanoid Arm): число-перечисление, 0 — левая,
/// 1 — правая. Номер вида в 26.1 — 42 (minecraft-data, протокол 26.1).
const PROPERTY_HUMANOID_ARM: i32 = 42;

/// Вид значения «байт» в списке видов свойств сущности.
const PROPERTY_BYTE: i32 = 0;

/// Общие флаги сущности (свойство 0, байт): 0x02 — приседает
/// (minecraft.wiki, Java Edition protocol/Entity metadata).
const FLAGS_PROPERTY: u8 = 0;
const LIVING_FLAGS_PROPERTY_ID: u8 = 8;
const FLAG_CROUCHING: u8 = 0x02;

/// Поза сущности (свойство 6): вид значения «поза» — 20 в 26.1
/// (minecraft-data, протокол 26.1); 0 — стоит, 5 — присел.
const POSE_PROPERTY: u8 = 6;
const PROPERTY_POSE: i32 = 20;
const POSE_STANDING: i32 = 0;
const POSE_CROUCHING: i32 = 5;
/// Поза «спит» и свойство 14 «место кровати» (вид 11 — необязательное место
/// блока): чужой спящий игрок (чёрный ящик, bed-sleep-night — у себя так же).
const POSE_SLEEPING: i32 = 2;
const BED_PROPERTY: u8 = 14;
const PROPERTY_OPTIONAL_BLOCK_POS: i32 = 11;

/// Конец списка свойств в пакете.
const PROPERTIES_END: u8 = 0xFF;

/// Serverbound Client Information — игрок поменял настройки, в том числе
/// набор показываемых частей скина.
const CLIENT_INFORMATION: i32 = 0x0E;

/// Serverbound Pick Item From Block — игрок навёл на блок и попросил такой же
/// предмет в руку («выбор блока», средняя кнопка мыши).
const PICK_ITEM_FROM_BLOCK: i32 = 0x24;

/// Serverbound Player Input — какие клавиши движения зажаты. Серверу отсюда
/// нужно одно: приседает ли игрок. Присев, щёлкают не по рычагу, а ставят
/// на него блок.
const PLAYER_INPUT: i32 = 0x2B;

/// Бит приседания в пакете ввода.
const INPUT_SNEAK: u8 = 0x20;

/// Clientbound Set Head Rotation — куда повёрнута голова сущности.
///
/// У чужого игрока клиент берёт поворот головы отсюда, а не из поворота тела.
const SET_HEAD_ROTATION: i32 = 0x53;

/// Clientbound Teleport Entity — сущность перенесена сразу на большое
/// расстояние, дельтой такое не рассказать.
const TELEPORT_ENTITY: i32 = 0x23;

/// Номер игрока в списке видов сущностей. Список вшит в клиент, и номера
/// в нём сервер не сообщает: промах на единицу — и сущность не появится.
///
/// Для версии 26.1 это 155 (в 26.2 — уже 156).
const PLAYER_ENTITY_TYPE: i32 = 155;

/// Сколько байт занимают координаты в пакете о перемещении: три double.
const POSITION_BYTES: usize = 24;

/// Длина углов в пакетах о перемещении: поворот и наклон, по f32.
const ROTATION_BYTES: usize = 8;

/// Флаг «на земле» в байте флагов пакетов о перемещении (Movement Flags:
/// 0x01 — на земле, 0x02 — упёрся в стену).
const ON_GROUND_FLAG: u8 = 0x01;

/// Clientbound Game Event.
const GAME_EVENT: i32 = 0x26;

/// Событие игры 10: появление древнего стража (вики, Java Edition protocol,
/// Game Event: «Play elder guardian mob appearance»; чёрный ящик, elder-fatigue).
const GAME_EVENT_ELDER_CURSE: u8 = 10;
const SET_HEALTH: i32 = 0x68;

/// Clientbound Set Center Chunk.
const SET_CENTER_CHUNK: i32 = 0x5E;

/// Clientbound Sound Effect — звук в точке мира.
const SOUND_EFFECT: i32 = 0x75;

/// Разряд звука: звук блока. Их одиннадцать, и от разряда зависит,
/// какой ползунок громкости на него влияет.
const SOUND_BLOCK: i32 = 4;

/// Раздел звуков «существа» — в нём звучат сущности (плеск предмета).
const SOUND_NEUTRAL: i32 = 6;

/// Раздел звуков «игроки» — удар игрока.
const SOUND_PLAYERS: i32 = 7;
/// Раздел «окружение» (ambient): запуск ракеты (вики, Firework Rocket).
const SOUND_AMBIENT: i32 = 8;

/// Entity Sound Effect: звук, привязанный к сущности.
const ENTITY_SOUND: i32 = 0x74;

/// Clientbound Block Action — то, что клиент проигрывает у себя сам:
/// ход поршня, открытие сундука, звук нотного блока.
/// Clientbound Block Entity Data — данные блок-сущности (вики, Java Edition
/// protocol/Packets: идёт прямо перед Block Action).
const BLOCK_ENTITY_DATA: i32 = 0x06;

/// Номер вида блок-сущности головы в реестре `minecraft:block_entity_type`
/// (`minecraft:skull`, tools/blackbox/data/registries.json).
const SKULL_BLOCK_ENTITY: i32 = 16;

const BLOCK_ACTION: i32 = 0x07;

/// Clientbound Block Update — «вот какое состояние у этого блока».
const BLOCK_UPDATE: i32 = 0x08;
const UPDATE_SECTION_BLOCKS: i32 = 0x54;

/// Clientbound Acknowledge Block Change — «твоё действие принято, показывай
/// то, что прислал сервер, а не своё предсказание».
const ACKNOWLEDGE_BLOCK_CHANGE: i32 = 0x04;

/// Serverbound Command Suggestions Request — клиент нажал Tab и просит
/// подсказки.
const TAB_COMPLETE_REQUEST: i32 = 0x0F;

/// Clientbound Command Suggestions Response — сами подсказки.
const TAB_COMPLETE_RESPONSE: i32 = 0x0F;

/// Clientbound Disconnect (Play) — «сервер тебя отключает, вот почему».
const DISCONNECT: i32 = 0x20;

/// Clientbound Unload Chunk — «этот чанк тебе больше не нужен».
///
/// Без него клиент забывает ушедшие далеко чанки сам, и когда игрок
/// возвращается, на их месте остаётся пустота: сервер-то считает, что уже
/// их отправлял.
const UNLOAD_CHUNK: i32 = 0x25;

/// Clientbound Chunk Data and Update Light.
const CHUNK_DATA: i32 = 0x2D;

/// Clientbound Synchronize Player Position.
const PLAYER_POSITION: i32 = 0x48;

/// Clientbound Set Default Spawn Position.
const SET_DEFAULT_SPAWN_POSITION: i32 = 0x61;

/// Serverbound Confirm Teleportation.
const CONFIRM_TELEPORTATION: i32 = 0x00;

/// Serverbound Interact — игрок щёлкнул по сущности (minecraft-data 26.1,
/// protocol.json: 0x1a, `use_entity`: номер сущности, рука, точка, присел).
const USE_ENTITY: i32 = 0x1A;

/// Serverbound Attack — игрок ударил сущность (minecraft-data 26.1,
/// protocol.json: 0x01, `attack`, одно поле — номер сущности).
const ATTACK: i32 = 0x01;

/// Clientbound Respawn: переход в другое измерение.
const RESPAWN: i32 = 0x52;

/// Boss Event (minecraft-data 26.1: boss_bar, 0x09): UUID полосы, действие
/// (0 — добавить: здоровье, заголовок, цвет, деления, флаги; 1 — убрать;
/// 2 — здоровье).
const BOSS_EVENT: i32 = 0x09;

/// UUID полосы дракона — постоянный [проверить: у оригинала случайный].
const DRAGON_BOSS_UUID: [u8; 16] = [0xD4, 0xA6, 0x0E, 0x4D, 0x1A, 0x7E, 0x4C, 0x0D, 0x9A, 0x33, 0x5E, 0x1C, 0xE2, 0xD1, 0x0B, 0xA1];

/// Дальность прорисовки, которую сервер объявляет клиенту. Клиент ждёт, пока
/// загрузятся все чанки в этом радиусе, поэтому она должна совпадать с числом
/// реально отправленных чанков.
/// Как часто напоминать клиенту о себе.
///
/// Клиент отключается, если не получал от сервера ни одного пакета дольше
/// 30 секунд, поэтому интервал должен быть заметно меньше.
const KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(5);

/// Сколько ждать ответа на Keep Alive, прежде чем отключить игрока.
const KEEP_ALIVE_TIMEOUT: Duration = Duration::from_secs(30);

// Режим игры при входе (для того, кто заходит впервые) — creative. Клиент
// берёт его не из Login (Play), а из Player Info Update, поэтому он указывается
// в обоих пакетах. Само значение — в модуле команд, вместе с остальными
// режимами. Кто уже заходил, входит в том режиме, в каком вышел.

/// Куда смотрит тот, кто появился впервые. Само место даёт мир: в обычном
/// мире в начале координат может оказаться море, и точку появления там ищут
/// по суше (`World::spawn_position`). Кто уже заходил — встаёт там, где вышел
/// (см. playerdata).
const SPAWN_YAW: f32 = 0.0;
const SPAWN_PITCH: f32 = 30.0;

/// На сколько от игрока появляется выброшенный предмет — чтобы он не вылетал
/// из самого игрока.
const THROW_DISTANCE: f64 = 0.4;

/// На какой высоте от ног игрока появляется выброшенный предмет: примерно
/// на уровне рук.
const THROW_HEIGHT: f64 = 1.2;

/// Насколько игрок должен повернуться, чтобы это стоило записать на диск.
///
/// Клиент шлёт поворот при каждом движении мыши, и записывать каждый градус
/// было бы незачем: после перезапуска сервера игрок всё равно не заметит
/// разницы в несколько градусов.
const SAVED_TURN: f32 = 15.0;

/// Насколько близко положения считаются одним и тем же.
///
/// Точного совпадения дробных чисел ждать не приходится, а миллиметр — это
/// заведомо меньше, чем игрок способен заметить.
const SAME_POSITION: f64 = 0.001;

/// Что этому клиенту уже показано про чужого игрока.
///
/// Смещения при движении считаются от того, что клиент уже знает, поэтому
/// помнить приходится про каждого чужого игрока отдельно, и у каждого
/// подключения этот список свой.
#[derive(Clone, Copy, PartialEq)]
struct Shown {
    position: Position,
    skin_parts: u8,
    main_hand: u8,
    held: Option<Stack>,
    /// Вторая рука и броня: шлем, нагрудник, штаны, ботинки.
    worn: [Option<Stack>; 5],
    /// Скорость удара (биты числа): чужим клиентам она уходит атрибутом.
    speed_bits: u64,
    /// Бежит ли: остальным — флаг сущности и атрибут скорости.
    sprinting: bool,
    living: u8,
    effect_colors: [i32; crate::survival::EFFECT_PARTICLES],
    /// Здоровье (биты числа): остальным — свойством 9.
    health_bits: u32,
    /// Воздух в лёгких: остальным — свойством 1 (чёрный ящик, trident-riptide-hit).
    air: i32,
    sneaking: bool,
    /// Сколько взмахов рукой уже показано.
    swings: u32,
    /// Спит: место кровати (поза «спит» и свойство 14).
    sleeping: Option<(i32, i32, i32)>,
}

type Seen = HashMap<i32, Shown>;

/// Проходы рассказа о чужих игроках за такт: движение и вид, взмахи,
/// экипировка.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    Moves,
    Swings,
    Gear,
}

/// Что этому клиенту в последний раз сказали про лежащий предмет: где он,
/// куда летит и лежит ли.
///
/// Скорость и «лежит ли» здесь не лишние: клиент между пакетами досчитывает
/// движение сам, и остановку ему надо сообщить отдельно — иначе он продолжит
/// тянуть предмет вниз.
#[derive(Clone, Copy, PartialEq)]
struct ShownItem {
    place: (f64, f64, f64),
    speed: (f64, f64, f64),
    on_ground: bool,

    /// Ушёл ли отчёт такта появления (`send_item_ticks`).
    first_sent: bool,
}

type SeenItems = HashMap<i32, ShownItem>;

/// Чанки, уже отправленные этому клиенту.
///
/// Мир бесконечен, а клиенту нужны только те чанки, до которых он дошёл.
/// Помнить отправленное приходится для каждого подключения своё.
type SentChunks = std::collections::HashSet<(i32, i32)>;

/// Сколько записей каждого из общих журналов клиент уже получил.
///
/// Запоминается в момент входа: всё, что случилось раньше, клиент получил
/// вместе с чанками, списком игроков и лентой чата, и повторять это ему
/// не нужно.
struct Entered {
    world_changes: usize,
    player_changes: usize,
    chat_lines: usize,
    item_changes: usize,

    /// Чужие игроки, уже показанные клиенту при входе.
    seen: Seen,

    /// Лежащие предметы, уже показанные клиенту при входе.
    seen_items: SeenItems,

    /// Чанки, уже отправленные клиенту.
    sent_chunks: SentChunks,
}

/// Проводит игрока через фазу Play: вводит в мир и обслуживает до выхода.
/// Проводит игрока через игру. Возвращает false, если игрок в мир так и не
/// попал — например, его ник успели занять, пока он проходил настройку.
pub async fn play_session(
    socket: &mut TcpStream,
    name: &str,
    uuid: &[u8; 16],
    look: configuration::ClientLook,
    shared: &Shared,
    (came_from, came_to): (&str, &str),
) -> io::Result<bool> {
    let skin_parts = look.skin_parts;
    // Что осталось от прошлого захода: место, поворот и режим игры. Если
    // игрок здесь впервые, этого нет — он появится в точке появления.
    let saved = playerdata::load(&shared.playerdata, uuid);

    // Скин узнаётся до входа в мир: он должен быть готов к тому мгновению,
    // когда сервер расскажет о новом игроке остальным.
    let skin = skins::look_up(&shared.skins, name, &shared.skin_settings).await;

    // Точка появления — всегда в обычном мире.
    let spawn = shared
        .world_in(crate::dimension::Dimension::Overworld)
        .lock()
        .expect("мир захвачен другим потоком")
        .spawn_position();

    let mut player = PlayerState::new(
        *uuid,
        name.to_string(),
        saved,
        skin,
        skin_parts,
        shared.next_reader(),
        spawn,
    );

    player.main_hand = look.main_hand;
    player.view = view_for(shared, &look);

    if player.saved.is_some() {
        log_debug!(
            "Play: игрок {} заходит туда, где вышел: {:.2} {:.2} {:.2}",
            player.name,
            player.x,
            player.y,
            player.z
        );
    }

    // Отписаться от журналов надо и когда вход оборвался ошибкой: иначе
    // журналы держали бы записи для этого читателя вечно.
    let mut out = Outgoing::new(socket);
    let stream = &mut out;

    let entered = enter_world(stream, &mut player, shared, (came_from, came_to)).await;

    // Что вход успел положить в очередь, уходит и при отказе: там может быть
    // сообщение об отключении.
    let entered = match stream.flush().await {
        Ok(()) => entered,
        Err(error) => entered.and(Err(error)),
    };

    let Ok(Some(entered)) = entered else {
        log_debug!("Play: игрок {} в мир не попал", player.name);

        forget_reader(shared, player.dimension, player.reader);

        // Номер сущности выдаётся вместе с появлением в списке: если он
        // есть, обрыв случился уже там, и из списка надо убрать самим —
        // вызывающий уберёт только вошедшего.
        if entered.is_err() && player.entity_id != 0 {
            player.progress.custom("leave_game", 1);
            player.save(&shared.playerdata);
            player_left(shared, &player.name, &player.uuid);
        }

        return entered.map(|_| false);
    };

    let result = read_and_log_play_packets(stream, shared, &mut player, entered).await;

    // Последнее (например, причина отключения) — клиенту, пока сокет жив.
    let result = result.and(stream.flush().await);

    // Как у обычного сервера: «потерял соединение» с причиной, а «вышел»
    // добавит общий список игроков следом.
    match &result {
        Ok(()) => log_info!("{} потерял соединение: отключился", player.name),
        Err(error) => log_info!("{} потерял соединение: {}", player.name, error.kind()),
    }

    // Открытое хранилище теряет смотрящего.
    close_own_window(shared, &mut player);

    // Курсор и сетка верстака при выходе возвращаются в инвентарь, а что
    // не влезло — на землю: в файл игрока они не пишутся.
    for stack in player.inventory.close_window().thrown {
        throw_out(shared, &player, stack);
    }

    // При выходе записываем положение в любом случае — в том числе когда
    // соединение оборвалось ошибкой. Иначе последние метры пути пропали бы.
    player.progress.custom("leave_game", 1);
    player.save(&shared.playerdata);
    log_debug!(
        "Play: положение игрока {} записано при выходе: {:.2} {:.2} {:.2}",
        player.name,
        player.x,
        player.y,
        player.z
    );

    log_debug!("Play: чтение пакетов клиента завершено");

    forget_reader(shared, player.dimension, player.reader);

    // Игрок в мире был — значит, его надо убрать из списка при любом
    // окончании, и оборванном тоже. Причина обрыва уже записана в лог выше.
    Ok(true)
}

/// Сколько чанков вокруг слать игроку с такими настройками: его дальность,
/// но не больше серверной и не меньше двух.
fn view_for(shared: &Shared, look: &configuration::ClientLook) -> i32 {
    let server = shared.properties.view_distance;

    i32::from(look.view_distance).clamp(2.min(server), server)
}

/// Игрок для плагинов: копия нужных сведений (plugins/mod.rs).
fn plugin_view(player: &PlayerState) -> crate::plugins::PlayerView {
    crate::plugins::PlayerView {
        uuid: player.uuid,
        name: player.name.clone(),
        game_mode: player.game_mode,
        dimension: player.dimension,
        x: player.x,
        y: player.y,
        z: player.z,
        yaw: player.yaw,
        pitch: player.pitch,
    }
}

/// Отписывает подключение от общих журналов.
///
/// Пока читатель записан, журналы держат для него все изменения. Забыть
/// отписаться — значит заставить их расти вечно.
fn forget_reader(shared: &Shared,
    dimension: crate::dimension::Dimension, reader: journal::Reader) {
    shared
        .world_in(dimension)
        .lock()
        .expect("мир захвачен другим потоком")
        .forget_reader(reader);

    shared
        .players
        .lock()
        .expect("список игроков захвачен другим потоком")
        .forget_reader(reader);

    shared
        .items_in(dimension)
        .lock()
        .expect("предметы захвачены другим потоком")
        .forget_reader(reader);
}

/// Убирает игрока из списка и сообщает об уходе в чат.
///
/// Зовётся при любом окончании сессии — и когда игрок вышел сам, и когда
/// соединение оборвалось ошибкой. Иначе он остался бы в списке навсегда.
pub fn player_left(shared: &Shared, name: &str, uuid: &[u8; 16]) {
    crate::commands_admin::forget_address(uuid);

    // Снимок — до ухода из списка; вызов плагина — после отпускания замков.
    let view = if crate::plugins::active() { crate::plugins::PlayerView::find(shared, name) } else { None };

    let removed = shared
        .players
        .lock()
        .expect("список игроков захвачен другим потоком")
        .leave(uuid);

    if removed {
        shared
            .chat
            .lock()
            .expect("чат захвачен другим потоком")
            .push(format!("{} вышел", name));

        if let Some(view) = view {
            crate::plugins::player_quit(&view);
        }
    }
}

/// Проводит игрока в мир, отправляет ему чанки вокруг точки появления и
/// заводит запись в списке игроков.
///
/// Возвращает None, если игрока не пустили: ник успели занять, пока он
/// проходил настройку. Отказ здесь — окончательный, потому что он выносится
/// тем же захватом списка, каким игрок в него добавляется.
async fn enter_world(
    stream: &mut Outgoing<'_>,
    player: &mut PlayerState,
    shared: &Shared,
    (came_from, came_to): (&str, &str),
) -> io::Result<Option<Entered>> {
    // Номера записей в общих журналах берём до всего остального: всё, что
    // случится после этого места, дойдёт до клиента обычной рассылкой. Если
    // взять их в конце, запись, появившаяся во время входа, окажется уже
    // «прошлой» и до клиента не дойдёт вовсе.
    //
    // Из-за этого своя же запись о входе тоже попадёт в рассылку — она
    // пропускается по опознавателю при отправке.
    let (chat_lines, player_changes, world_changes, item_changes) = {
        // Замки берутся в общем для всего сервера порядке: мир, игроки,
        // предметы, чат (см. Shared). Такт держит мир и берёт игроков; возьми
        // мы здесь игроков раньше мира — вход игрока, совпавший с тактом,
        // застыл бы вместе с тактом навсегда, а за ними и весь сервер.
        let mut world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");
        let mut players = shared
            .players
            .lock()
            .expect("список игроков захвачен другим потоком");
        let mut items = shared
            .items_in(player.dimension)
            .lock()
            .expect("предметы захвачены другим потоком");
        let chat = shared.chat.lock().expect("чат захвачен другим потоком");

        // Записываемся читателями: с этого места журналы держат для нас
        // всё новое, а прочитанное всеми выбрасывают.
        (
            chat.count(),
            players.watch_changes(player.reader),
            world.watch_events(player.reader),
            items.watch_changes(player.reader),
        )
    };

    // Заводим игрока в общем списке до Login (Play): оттуда берётся номер его
    // сущности, а его клиент должен узнать этот номер сразу — по нему он
    // отличает себя от остальных. Остальные узнают о нём из журнала.
    let member = player.member();

    let Some((member, others)) = shared
        .players
        .lock()
        .expect("список игроков захвачен другим потоком")
        .join(member, shared.next_entity_id())
    else {
        log_info!("{} не пущен: ник уже занят", player.name);
        return Ok(None);
    };

    // Строка входа как у обычного сервера, плюс адрес сервера после стрелки:
    // «X[/1.2.3.4:5555 → host:25565] вошёл с номером сущности N в (x, y, z)».
    log_info!(
        "{}[/{} → {}] вошёл с номером сущности {} в ({:.1}, {:.1}, {:.1})",
        player.name,
        came_from,
        came_to,
        member.entity_id,
        player.x,
        player.y,
        player.z
    );

    player.entity_id = member.entity_id;

    // Всё дальнейшее — после записи в список: если связь оборвётся здесь,
    // игрока надо из списка убрать, иначе его ник останется «занятым».
    let entered: io::Result<Option<Entered>> = async {

        send_login(
            stream,
            player.dimension,
            player.game_mode,
            player.entity_id,
            shared.properties.view_distance,
            shared.properties.simulation_distance,
        )
        .await?;
        send_abilities(stream, player.game_mode).await?;
        send_player_position(stream, player).await?;

        // Клиент подтверждает телепортацию — до подтверждения он игнорирует
        // пакеты о перемещении.
        let look = read_confirm_teleportation(stream).await?;
        player.awaiting_teleport = false;

        if let Some(look) = look {
            player.skin_parts = look.skin_parts;
            player.main_hand = look.main_hand;
            player.view = view_for(shared, &look);

            // Игрок уже в общем списке — остальные должны увидеть новое.
            let mut players = shared
                .players
                .lock()
                .expect("список игроков захвачен другим потоком");

            players.set_skin_parts(&player.uuid, look.skin_parts);
            players.set_main_hand(&player.uuid, look.main_hand);
        }

        // Дерево команд: без него клиент не знает, что можно вводить после косой
        // черты, и не подсказывает.
        let may_tnt = crate::permissions::allowed(shared, &player.uuid, crate::custom_tnt::GIVE_NODE);
        send_commands(stream, may_tnt).await?;

        // Список игроков: сперва те, кто уже на сервере, затем сам игрок.
        // Новичок о них узнаёт только отсюда — записи журнала он ещё не читал.
        for other in &others {
            send_player_info(stream, other).await?;
        }

        send_player_info(stream, &member).await?;

        // И про самого себя: свой второй слой скина клиент рисует не по своим
        // настройкам, а по тем же сведениям о сущности, что и чужой. Не сказать
        // ему — и в виде со стороны игрок будет без шапки и куртки.
        send_look(stream, member.entity_id, member.skin_parts, member.main_hand, member.sneaking)
        .await?;

        // Рамка мира и состояние такта (пробник cmdworld: у оригинала —
        // сразу за входом: Initialize World Border, Set Ticking State, Step Tick).
        write_body(stream, crate::commands_world::border_of(shared, player.dimension).init_packet()).await?;

        for body in crate::commands_world::ticking_packets(shared) {
            write_body(stream, body).await?;
        }

        send_default_spawn_position(stream, player.spawn_point).await?;
        vitals::send_health(stream, &player.vit).await?;
        vitals::send_experience_if_any(stream, &player.vit).await?;
        send_game_event_start_waiting(stream).await?;
        let (age, time, _) = world_age(shared, player.dimension);
        send_time(stream, age, Some(time)).await?;
        send_weather(stream, shared, player.dimension).await?;
        send_center_chunk(stream, player.chunk()).await?;

        // Клиент не закроет экран загрузки, пока не получит все чанки в объявленной
        // дальности прорисовки. Отправляем квадрат вокруг игрока: он может стоять
        // и не в начале координат — при следующем заходе он появляется там, где
        // вышел.
        let (center_x, center_z) = player.chunk();

        let mut sent = SentChunks::new();

        // Сперва только ближний квадрат — чтобы было на чём стоять. Остальное
        // догружается в основном цикле: иначе на большой дальности игрок ждал бы
        // тысячи чанков, прежде чем войти, а сообщение о его входе остальные
        // увидели бы только через десяток секунд. У оригинала так же: игрок
        // появляется сразу, даль дорисовывается потом.
        send_chunks_around(
            stream,
            shared, player.dimension,
            (center_x, center_z),
            &mut sent,
            (NEAR_CHUNKS, player.view),
            usize::MAX,
        )
        .await?;

        log_debug!("Play: отправлено ближних чанков — {}", sent.len());

        // Чужие игроки появляются в мире только теперь: сущность клиент создаёт
        // лишь после того, как получил запись об игроке в списке, а место в мире —
        // после чанков под ним.
        let mut seen = Seen::new();

        for other in others.iter().filter(|other| other.dimension == player.dimension) {
            send_add_entity(stream, other).await?;
            send_look(stream, other.entity_id, other.skin_parts, other.main_hand, other.sneaking).await?;
            send_equipment(stream, other.entity_id, &[(EQUIPMENT_MAIN_HAND, other.held)]).await?;

            seen.insert(
                other.entity_id,
                Shown {
                    position: other.position,
                    skin_parts: other.skin_parts,
                    main_hand: other.main_hand,
                    held: other.held,
                    worn: [None; 5],
                    speed_bits: 0,
                    sprinting: false,
                    living: 0,
                    effect_colors: [0; crate::survival::EFFECT_PARTICLES],
                    health_bits: crate::survival::MAX_HEALTH.to_bits(),
                    air: crate::survival::MAX_AIR,
                    sneaking: other.sneaking,
                    swings: other.swings,
                    sleeping: None,
                },
            );
        }

        // Предметы, уже лежащие в мире: о них новичок из журнала не узнает —
        // они попали туда до его прихода.
        let lying: Vec<Dropped> = shared
            .items_in(player.dimension)
            .lock()
            .expect("предметы захвачены другим потоком")
            .items()
            .to_vec();

        let mut seen_items = SeenItems::new();

        for item in &lying {
            send_dropped_item(stream, item).await?;
            seen_items.insert(item.entity_id, shown_item(item));
        }

        send_all_carts(stream, shared, player).await?;
        send_all_frames(stream, shared, player).await?;
        mobs::send_all_mobs(stream, shared, player).await?;

        // Кто на ком едет (`ride`).
        for body in crate::commands_world::passenger_packets(shared) {
            write_body(stream, body).await?;
        }

        // Инвентарь: то, с чем игрок вышел в прошлый раз. Без этой посылки клиент
        // показал бы пустой инвентарь, хотя на сервере вещи есть.
        send_container_content(stream, player).await?;
        send_held_slot(stream, player.inventory.selected()).await?;

        if !others.is_empty() {
            log_debug!("Play: показано чужих игроков — {}", others.len());
        }

        shared
            .chat
            .lock()
            .expect("чат захвачен другим потоком")
            .push(format!("{} зашёл на сервер", player.name));

        crate::plugins::player_join(shared, &player.name);

        Ok(Some(Entered {
            item_changes,
            seen_items,
            sent_chunks: sent,
            world_changes,
            player_changes,
            chat_lines,
            seen,
        }))
    }
    .await;

    if entered.is_err() {
        player_left(shared, &player.name, &player.uuid);
    }

    entered
}

/// Отправляет Login (Play) — первый пакет фазы Play.
///
/// `entity_id` — номер сущности самого игрока: по нему его клиент отличает
/// себя от других. Тот же номер сервер сообщает остальным, когда показывает
/// им этого игрока.
async fn send_login(
    stream: &mut Outgoing<'_>,
    dimension: Dimension,
    game_mode: i32,
    entity_id: i32,
    view: i32,
    simulation: i32,
) -> io::Result<()> {
    let mut body = encode_varint(LOGIN);

    push_i32(&mut body, entity_id); // Entity ID игрока
    body.push(0x00); // не хардкор

    // Список измерений сервера.
    body.extend_from_slice(&encode_varint(Dimension::ALL.len() as i32));
    for known in Dimension::ALL {
        body.extend_from_slice(&encode_string(known.name()));
    }

    body.extend_from_slice(&encode_varint(20)); // максимальное число игроков
    body.extend_from_slice(&encode_varint(view)); // дальность прорисовки
    body.extend_from_slice(&encode_varint(simulation)); // дальность симуляции
    body.push(0x00); // сокращённая отладочная информация
    body.push(0x01); // показывать экран возрождения
    body.push(0x00); // ограниченная крафтовка

    push_spawn_info(&mut body, dimension, game_mode);
    body.push(0x00); // обязательный защищённый чат

    write_body(stream, body).await
}

/// Сведения о месте появления — общая часть Login (Play) и Respawn: в каком
/// измерении игрок и в каком режиме.
fn push_spawn_info(body: &mut Vec<u8>, dimension: Dimension, game_mode: i32) {
    push_spawn_info_ex(body, dimension, game_mode, -1, None, 0);
}

/// То же с прежним режимом и точкой смерти (Respawn после гибели).
fn push_spawn_info_ex(
    body: &mut Vec<u8>,
    dimension: Dimension,
    game_mode: i32,
    previous_mode: i32,
    death: Option<(Dimension, (i32, i32, i32))>,
    portal_cooldown: i32,
) {
    body.extend_from_slice(&encode_varint(dimension.type_id()));
    body.extend_from_slice(&encode_string(dimension.name()));

    push_i64(body, 0); // hashed seed
    body.push(game_mode as u8); // режим игры
    body.push(previous_mode as u8); // предыдущий режим игры (−1 — нет)
    body.push(0x00); // отладочный мир
    body.push(0x00); // плоский мир

    match death {
        Some((dimension, (x, y, z))) => {
            body.push(0x01); // точка смерти есть
            body.extend_from_slice(&encode_string(dimension.name()));
            push_packed_position(body, x, y, z);
        }
        None => body.push(0x00),
    }
    body.extend_from_slice(&encode_varint(portal_cooldown)); // откат портала
    // Уровень моря: у Энда 0 (замер оригинала: Respawn при входе в Энд),
    // у прочих — как был.
    body.extend_from_slice(&encode_varint(if dimension == Dimension::End { 0 } else { 63 }));
}

/// Отправляет Respawn — игрок переходит в другое измерение. Клиент при этом
/// забывает все чанки и сущности: их надо прислать заново.
async fn send_respawn(stream: &mut Outgoing<'_>, dimension: Dimension, player: &PlayerState, portal_cooldown: i32) -> io::Result<()> {
    let mut body = encode_varint(RESPAWN);

    // Прежний режим и точка смерти — как у игрока (чёрный ящик,
    // dim-nether-tp: prev=0, death=место последней гибели).
    push_spawn_info_ex(&mut body, dimension, player.game_mode, player.prev_mode, player.net.died_at, portal_cooldown);
    body.push(0x03); // сохранить свойства и сведения о сущности

    write_body(stream, body).await
}

/// Полоса босса дракона: доля здоровья — добавить или обновить, None —
/// убрать. Заголовок — имя дракона, цвет розовый (0), без делений, флаги
/// 6 — музыка и туман (чёрный ящик, end-fight-enter).
async fn send_boss_bar(stream: &mut Outgoing<'_>, player: &mut PlayerState, share: Option<f32>) -> io::Result<()> {
    let mut body = encode_varint(BOSS_EVENT);

    body.extend_from_slice(&DRAGON_BOSS_UUID);

    match share {
        Some(share) if !player.boss_shown => {
            player.boss_shown = true;
            body.extend_from_slice(&encode_varint(0));
            body.extend_from_slice(&nbt::translated("entity.minecraft.ender_dragon"));
            body.extend_from_slice(&share.to_be_bytes());
            body.extend_from_slice(&encode_varint(0)); // цвет: розовый
            body.extend_from_slice(&encode_varint(0)); // без делений
            body.push(6); // флаги: музыка дракона и туман
        }
        Some(share) => {
            body.extend_from_slice(&encode_varint(2));
            body.extend_from_slice(&share.to_be_bytes());
        }
        None if player.boss_shown => {
            player.boss_shown = false;
            body.extend_from_slice(&encode_varint(1));
        }
        None => return Ok(()),
    }

    write_body(stream, body).await
}

/// Respawn после смерти: прежний режим и место гибели, ничего не сохраняется.
async fn send_respawn_after_death(stream: &mut Outgoing<'_>, dimension: Dimension, player: &PlayerState) -> io::Result<()> {
    let mut body = encode_varint(RESPAWN);
    let death = player.net.died_at;

    push_spawn_info_ex(&mut body, dimension, player.game_mode, player.prev_mode, death, 0);
    body.push(0x00); // ничего не сохранять

    write_body(stream, body).await
}

/// Отправляет Synchronize Player Position — ставит игрока в точку, где он
/// стоял в прошлый раз (или в точку появления, если он здесь впервые).
async fn send_player_position(stream: &mut Outgoing<'_>, player: &mut PlayerState) -> io::Result<()> {
    send_player_position_as(stream, player, Sync::Absolute).await
}

/// Как сверка места ложится в пакет: у оригинала это зависит от того, что
/// перенесло игрока (чёрный ящик, dim-nether-tp и end-portal-enter).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Sync {
    /// Всё абсолютное (вход, возрождение).
    Absolute,
    /// Команда `tp`: поворот относительный, сдвиг на 0 — флаги 24.
    Command,
    /// Портал Края: поворот по горизонтали абсолютный, наклон и скорость
    /// относительные — флаги 496.
    Portal,
    /// Портал Незера: и поворот относительный — флаги 504 (чёрный ящик,
    /// nether-portal-enter). Сдвиг поворота у нас пока 0 [проверить].
    NetherPortal,
}

async fn send_player_position_as(stream: &mut Outgoing<'_>, player: &mut PlayerState, sync: Sync) -> io::Result<()> {
    let body = position_body(player, sync);

    write_body(stream, body).await
}

/// Тело пакета сверки места (Synchronize Player Position) — с новым номером
/// переноса; игрок ждёт подтверждения.
fn position_body(player: &mut PlayerState, sync: Sync) -> Vec<u8> {
    let mut body = encode_varint(PLAYER_POSITION);

    // У каждого переноса свой номер: подтверждение старого не должно
    // сниматься за подтверждение нового.
    player.teleport = player.teleport.wrapping_add(1);
    player.awaiting_teleport = true;
    player.moved_since_tp = false;

    // Перенос обнуляет падение: скачок места не высота падения.
    player.vit.fall = 0.0;
    player.net.teleported = true;

    body.extend_from_slice(&encode_varint(player.teleport));
    push_f64(&mut body, player.x); // X
    push_f64(&mut body, player.y); // Y
    push_f64(&mut body, player.z); // Z
    push_f64(&mut body, 0.0); // скорость X
    push_f64(&mut body, 0.0); // скорость Y
    push_f64(&mut body, 0.0); // скорость Z

    match sync {
        Sync::Absolute => {
            push_f32(&mut body, player.yaw); // поворот
            push_f32(&mut body, player.pitch); // наклон
            push_i32(&mut body, 0); // все координаты абсолютные
        }
        Sync::Command => {
            push_f32(&mut body, 0.0); // поворот: прибавить 0
            push_f32(&mut body, 0.0); // наклон: прибавить 0
            push_i32(&mut body, 24); // относительные: поворот и наклон
        }
        Sync::Portal => {
            push_f32(&mut body, player.yaw);
            push_f32(&mut body, 0.0); // наклон: прибавить 0
            push_i32(&mut body, 496); // относительные: наклон, скорость, её поворот
        }
        Sync::NetherPortal => {
            push_f32(&mut body, 0.0); // поворот: прибавить 0
            push_f32(&mut body, 0.0); // наклон: прибавить 0
            push_i32(&mut body, 504); // относительные: поворот, наклон, скорость, её поворот
        }
    }

    body
}

/// Кладёт пакет в очередь подключения (см. `Outgoing`): он уйдёт вместе с
/// остальными, когда подключение соберётся ждать.
async fn write_body(stream: &mut Outgoing<'_>, body: Vec<u8>) -> io::Result<()> {
    stream.send(&body).await
}

/// Читает подтверждение телепортации, пропуская прочие пакеты клиента —
/// кроме настроек: их клиент шлёт сразу после входа, и выбросить их значило
/// бы потерять, например, его ведущую руку. Возвращает последние из них.
async fn read_confirm_teleportation(
    stream: &mut Outgoing<'_>,
) -> io::Result<Option<configuration::ClientLook>> {
    let mut look = None;

    // Клиент подтвердит только то, что получил.
    stream.flush().await?;

    loop {
        let packet = read_packet(stream.stream()).await?;

        if packet.id == CONFIRM_TELEPORTATION {
            log_debug!("Play: клиент подтвердил телепортацию");
            return Ok(look);
        }

        if packet.id == CLIENT_INFORMATION
            && let Some(now) = configuration::read_look(packet.payload())
        {
            look = Some(now);
        }

        log_debug!(
            "Play: пакет клиента 0x{:02X} — длина тела {} байт",
            packet.id, packet.length
        );
    }
}

/// Отправляет Player Info Update — запись игрока в списке игроков.
///
/// Нужна не только ради списка: клиент берёт режим игры именно отсюда, поле
/// режима в Login (Play) он не применяет.
///
/// «Показывать в списке» — отдельное действие со своим битом, и без него
/// клиент игрока знает, но в списке не показывает: список остаётся пустым.
async fn send_player_info(stream: &mut Outgoing<'_>, member: &Member) -> io::Result<()> {
    let mut body = encode_varint(PLAYER_INFO_UPDATE);

    body.push(PLAYER_INFO_ADD | PLAYER_INFO_GAME_MODE | PLAYER_INFO_LISTED);
    body.extend_from_slice(&encode_varint(1)); // один игрок
    body.extend_from_slice(&member.uuid);

    // Действие «добавить игрока»: имя и свойства профиля.
    body.extend_from_slice(&encode_string(&member.name));
    match &member.skin {
        // Скин — свойство профиля с именем textures: описание с ссылкой
        // на картинку.
        //
        // Подпись прикладывается, только когда описание её требует (у скинов
        // Mojang стоит `signatureRequired`): такой скин без подписи клиент
        // молча отбрасывает. Остальные (Ely.by) идут без подписи — чужую
        // подпись клиент счёл бы неверной и тоже отбросил бы скин.
        Some(skin) => {
            body.extend_from_slice(&encode_varint(1)); // одно свойство
            body.extend_from_slice(&encode_string("textures"));
            body.extend_from_slice(&encode_string(&skin.value));

            if skin.signature_required() && !skin.signature.is_empty() {
                body.push(1);
                body.extend_from_slice(&encode_string(&skin.signature));
            } else {
                body.push(0); // подписи нет
            }
        }
        None => body.extend_from_slice(&encode_varint(0)), // свойств профиля нет
    }

    // Действие «обновить режим игры».
    body.extend_from_slice(&encode_varint(member.game_mode));

    // Действие «показывать в списке».
    body.push(1);

    write_body(stream, body).await
}

/// Отправляет Player Info Update с одним лишь новым режимом игры.
///
/// Именно этим клиент переключает режим игры, а не текстом в чате: текст
/// только уведомляет игрока, а режим меняется отсюда.
async fn send_game_mode(stream: &mut Outgoing<'_>, uuid: &[u8; 16], game_mode: i32) -> io::Result<()> {
    let mut body = encode_varint(PLAYER_INFO_UPDATE);

    body.push(PLAYER_INFO_GAME_MODE);
    body.extend_from_slice(&encode_varint(1)); // один игрок
    body.extend_from_slice(uuid);
    body.extend_from_slice(&encode_varint(game_mode));

    write_body(stream, body).await
}

/// Отправляет Player Info Remove — игрок пропал из списка.
async fn send_player_info_remove(stream: &mut Outgoing<'_>, uuid: &[u8; 16]) -> io::Result<()> {
    let mut body = encode_varint(PLAYER_INFO_REMOVE);

    body.extend_from_slice(&encode_varint(1)); // один игрок
    body.extend_from_slice(uuid);

    write_body(stream, body).await
}

/// Отправляет Add Entity — сущность чужого игрока появляется в мире.
///
/// Обязательно после Player Info Update на этого же игрока: без записи
/// в списке клиент сущность не создаст, и игрок останется невидимым.
async fn send_add_entity(stream: &mut Outgoing<'_>, member: &Member) -> io::Result<()> {
    let mut body = encode_varint(ADD_ENTITY);
    let position = member.position;

    body.extend_from_slice(&encode_varint(member.entity_id));
    body.extend_from_slice(&member.uuid);
    body.extend_from_slice(&encode_varint(PLAYER_ENTITY_TYPE));

    push_f64(&mut body, position.x);
    push_f64(&mut body, position.y);
    push_f64(&mut body, position.z);

    // Скорость: сервер её не считает, поэтому игрок всегда появляется
    // стоящим на месте.
    types::push_velocity(&mut body, 0.0, 0.0, 0.0);

    body.push(types::angle(position.pitch));
    body.push(types::angle(position.yaw));
    body.push(types::angle(position.yaw)); // голова смотрит туда же, куда тело

    body.extend_from_slice(&encode_varint(0)); // данных о виде сущности нет

    write_body(stream, body).await
}

/// Кладёт предмет на землю рядом с игроком.
///
/// Летит он не по дуге, как в игре, — сервер сразу считает, где предмет ляжет:
/// от места броска вниз до первой опоры. Поднять его можно не сразу, иначе он
/// тут же вернулся бы в руки тому, кто бросил.
fn throw_out(shared: &Shared, player: &PlayerState, stack: Stack) {
    // Бросок идёт туда, куда игрок смотрит, — с наклоном: в небо предмет
    // летит вверх, под ноги падает сразу. Дальше он летит сам, по дуге.
    let (look_x, look_y, look_z) = looking(player.yaw, player.pitch);

    // Вылетает предмет от рук, чуть впереди игрока — по той же стороне,
    // куда брошен, только без наклона, иначе он выскакивал бы из головы.
    let flat = (look_x.hypot(look_z)).max(f64::EPSILON);

    let from = (
        player.x + look_x / flat * THROW_DISTANCE,
        player.y + THROW_HEIGHT,
        player.z + look_z / flat * THROW_DISTANCE,
    );

    let velocity = (
        look_x * items::THROW_SPEED,
        look_y * items::THROW_SPEED + items::THROW_LIFT,
        look_z * items::THROW_SPEED,
    );

    shared.items_in(player.dimension).lock().expect("предметы захвачены другим потоком").drop_from_player(shared.next_entity_id(), stack, from, velocity, player.uuid);

    log_debug!(
        "Play: {} выбросил предмет {} ({} шт.)",
        player.name,
        stack.item,
        stack.count
    );
}

/// Взгляд как у оригинала: синус и косинус по таблице на 65536 делений
/// одинарной точности (boat-ride: лодка на воде встаёт в z = 1,95264, а не
/// в 1,95257 точного счёта при наклоне 50°).
fn looking_table(yaw: f32, pitch: f32) -> (f64, f64, f64) {
    const RADIANS: f32 = std::f32::consts::PI / 180.0;
    const INDEX: f32 = 10430.378;

    let at = |index: i32| ((index & 65535) as f64 * std::f64::consts::TAU / 65536.0).sin() as f32;
    let sin = |angle: f32| at((angle * INDEX) as i32);
    let cos = |angle: f32| at((angle * INDEX + 16384.0) as i32);
    let (turn, tilt) = (-yaw * RADIANS, pitch * RADIANS);
    let flat = cos(tilt);

    (f64::from(sin(turn) * flat), f64::from(-sin(tilt)), f64::from(cos(turn) * flat))
}

/// Куда смотрит игрок — единичным направлением.
///
/// Север в игре — это минус Z, а наклон вниз — положительный, поэтому
/// у высоты знак обратный.
fn looking(yaw: f32, pitch: f32) -> (f64, f64, f64) {
    let yaw = (yaw as f64).to_radians();
    let pitch = (pitch as f64).to_radians();

    (
        -yaw.sin() * pitch.cos(),
        -pitch.sin(),
        yaw.cos() * pitch.cos(),
    )
}

/// Кладёт предмет в мир. Дальше он летит и падает сам — этим занимается такт.
fn drop_into_world(
    shared: &Shared,
    dimension: crate::dimension::Dimension,
    stack: Stack,
    at: (f64, f64, f64),
    velocity: (f64, f64, f64),
    delay: u64,
) {
    shared
        .items_in(dimension)
        .lock()
        .expect("предметы захвачены другим потоком")
        .drop_item(shared.next_entity_id(), stack, at, velocity, delay);
}

/// Подбирает то, до чего игрок дотянулся.
///
/// Возвращает слоты, которые изменились: их надо показать клиенту. Само
/// исчезновение предмета из мира рассылается всем по журналу.
fn pick_up_items(shared: &Shared, player: &mut PlayerState) -> Vec<i32> {
    let mut slots = Vec::new();

    // Поднимается столько, сколько влезает: остальное остаётся лежать.
    let taken = shared
        .items_in(player.dimension)
        .lock()
        .expect("предметы захвачены другим потоком")
        .take_near((player.x, player.y, player.z), player.entity_id, |stack| {
            let (changed, left) = player.inventory.put(stack);

            slots.extend(changed);
            stack.count - left.map_or(0, |left| left.count)
        });

    for item in taken {
        player.progress.picked_up(item.stack.item, item.stack.count);
        log_debug!(
            "Play: {} поднял предмет {} ({} шт.)",
            player.name,
            item.stack.item,
            item.stack.count
        );
    }

    slots
}

/// Отправляет сущность лежащего предмета.
///
/// Двумя пакетами: сперва сама сущность, потом её свойство — какой это
/// предмет. Без второго клиент нарисует пустоту: вид сущности он знает,
/// а что именно лежит — нет.
async fn send_dropped_item(stream: &mut Outgoing<'_>, item: &Dropped) -> io::Result<()> {
    let mut body = encode_varint(ADD_ENTITY);

    let kind = match (item.fuse, item.block) {
        (Some(_), _) => TNT_ENTITY_TYPE,
        (None, Some(_)) => FALLING_BLOCK_ENTITY_TYPE,
        (None, None) => ITEM_ENTITY_TYPE,
    };

    body.extend_from_slice(&encode_varint(item.entity_id));
    body.extend_from_slice(&item.uuid);
    body.extend_from_slice(&encode_varint(kind));
    push_f64(&mut body, item.x);
    push_f64(&mut body, item.y);
    push_f64(&mut body, item.z);

    // Динамит появляется уже с толчком — клиент ведёт полёт сам.
    match item.fuse {
        Some(_) => types::push_velocity(&mut body, item.vx, item.vy, item.vz),
        // Выпавший предмет — тоже: о нём рассказывается лишь раз в 20
        // тактов (`send_item_ticks`), и полёт до того клиент ведёт сам.
        None if item.block.is_none() => types::push_velocity(&mut body, item.vx, item.vy, item.vz),
        None => types::push_velocity(&mut body, 0.0, 0.0, 0.0),
    }

    // Лежащий предмет никуда не повёрнут.
    body.push(0);
    body.push(0);
    body.push(0);

    // Падающий блок говорит о себе прямо здесь: в этом числе у него
    // записано, какой он блок. Предмету и динамиту это поле ни к чему.
    let object_data = if item.fuse.is_some() { 0 } else { item.block.unwrap_or(0) };
    body.extend_from_slice(&encode_varint(object_data));

    write_body(stream, body).await?;

    // Динамит: запал и блок, которым он нарисован — как у оригинала, только
    // то, что отличается от обычного (80 тактов, блок динамита): при
    // поджоге рычагом пакета свойств нет вовсе (чёрный ящик, tnt-lever).
    if let Some(fuse) = item.fuse {
        let tnt = blocks::state_by_name("tnt").unwrap_or(0);
        let block = item.block.unwrap_or(tnt);

        if fuse == crate::tnt::FUSE && block == tnt {
            return Ok(());
        }

        let mut data = encode_varint(SET_ENTITY_DATA);

        data.extend_from_slice(&encode_varint(item.entity_id));

        if fuse != crate::tnt::FUSE {
            data.push(TNT_FUSE_PROPERTY);
            data.extend_from_slice(&encode_varint(PROPERTY_INT));
            data.extend_from_slice(&encode_varint(fuse));
        }

        if block != tnt {
            data.push(TNT_BLOCK_PROPERTY);
            data.extend_from_slice(&encode_varint(PROPERTY_BLOCK_STATE));
            data.extend_from_slice(&encode_varint(block));
        }

        data.push(PROPERTIES_END);

        return write_body(stream, data).await;
    }

    // Дальше — только для предмета: у падающего блока всё уже сказано.
    if item.block.is_some() {
        return Ok(());
    }

    send_item_properties(stream, item.entity_id, item.stack, item.no_gravity).await
}

/// Свойства выпавшего предмета: без тяжести (если так призван) и сам предмет.
/// У оригинала они уходят дважды — при появлении и ещё раз в конце такта
/// появления, как и у мобов (чёрный ящик, cow-physics, sheep-shear).
async fn send_item_properties(stream: &mut Outgoing<'_>, entity_id: i32, stack: Stack, no_gravity: bool) -> io::Result<()> {
    let mut data = encode_varint(SET_ENTITY_DATA);

    data.extend_from_slice(&encode_varint(entity_id));
    // Без тяжести (`NoGravity` в призыве) — свойство 5, логическое
    // (minecraft-data 26.1, entities.json: `no_gravity`), иначе клиент
    // тянул бы предмет вниз.
    if no_gravity {
        const NO_GRAVITY: u8 = 5;
        const PROPERTY_BOOLEAN: i32 = 8;

        data.push(NO_GRAVITY);
        data.extend_from_slice(&encode_varint(PROPERTY_BOOLEAN));
        data.push(1);
    }

    data.push(ITEM_PROPERTY);
    data.extend_from_slice(&encode_varint(PROPERTY_SLOT));
    push_slot(&mut data, Some(stack));
    data.push(PROPERTIES_END);

    write_body(stream, data).await
}

/// Молния: одна сущность без скорости и без свойств — как у оригинала
/// (чёрный ящик, lightning-rod: данные 0). Гром и треск клиент играет сам.
async fn send_lightning(stream: &mut Outgoing<'_>, bolt: &crate::lightning::Bolt) -> io::Result<()> {
    let mut body = encode_varint(ADD_ENTITY);

    body.extend_from_slice(&encode_varint(bolt.entity_id));
    body.extend_from_slice(&bolt.uuid);
    body.extend_from_slice(&encode_varint(LIGHTNING_ENTITY_TYPE));
    push_f64(&mut body, bolt.x);
    push_f64(&mut body, bolt.y);
    push_f64(&mut body, bolt.z);
    types::push_velocity(&mut body, 0.0, 0.0, 0.0);
    body.push(0);
    body.push(0);
    body.push(0);
    body.extend_from_slice(&encode_varint(0));

    write_body(stream, body).await
}

/// Вагонетка, как её знает клиент: последнее присланное место (точное и
/// в 1/4096 блока, от которых считаются шаги), скорость, поворот байтами,
/// «на земле», число ударов, толчков и переносов.
#[derive(Clone, Copy)]
struct ShownCart {
    place: (f64, f64, f64),
    grid: (i64, i64, i64),
    speed: (f64, f64, f64),
    yaw: u8,
    pitch: u8,
    on_ground: bool,
    hits: u32,
    impulses: u32,
    teleports: u32,
    primes: u32,
    burning: bool,
}

impl ShownCart {
    /// Вагонетка в том виде, в каком её показал пакет появления.
    fn of(cart: &crate::minecart::Minecart) -> Self {
        Self {
            place: (cart.x, cart.y, cart.z),
            grid: grid_of((cart.x, cart.y, cart.z)),
            speed: (cart.vx, cart.vy, cart.vz),
            yaw: cart_angle(cart.yaw),
            pitch: cart_angle(cart.pitch),
            on_ground: cart.on_ground,
            hits: cart.hits,
            impulses: cart.impulses,
            teleports: cart.teleports,
            primes: cart.primes,
            burning: cart.fuel > 0,
        }
    }
}

/// Место в шагах по 1/4096 блока — так его считает пакет сдвига.
fn grid_of((x, y, z): (f64, f64, f64)) -> (i64, i64, i64) {
    ((x * 4096.0).round() as i64, (y * 4096.0).round() as i64, (z * 4096.0).round() as i64)
}

/// Угол байтом, как его шлёт оригинал: с отбрасыванием дробной части
/// (зонд minecart-curve: 52,9° уходит как 37, а не 38).
fn cart_angle(degrees: f32) -> u8 {
    ((degrees * 256.0 / 360.0).floor() as i32 & 0xFF) as u8
}

/// Показывает клиенту все вагонетки мира, где он теперь: о них он из
/// журнала не узнает — они появились раньше.
async fn send_all_carts(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState) -> io::Result<()> {
    let carts: Vec<crate::minecart::Minecart> =
        shared.items_in(player.dimension).lock().expect("предметы захвачены другим потоком").carts().to_vec();

    for cart in &carts {
        send_cart(stream, cart).await?;
        player.seen_carts.insert(cart.entity_id, ShownCart::of(cart));
    }

    Ok(())
}

/// Досылает клиенту, что стало с вагонетками к началу этого такта (снято
/// с оригинала зондом, minecart-powered и minecart-hit):
///
/// - свойства удара (удар, сторона, урон) — каждый такт, только
///   изменившиеся; после удара — ещё и скорость;
/// - раз в 3 такта от появления — скорость, если она заметно изменилась
///   (квадрат разницы больше 1e-7 или стала нулём), и место: шагом, если
///   сдвинулась хотя бы на 1/362 блока, а раз в 60 тактов — в любом случае;
///   сменила «на земле» — сверкой места целиком; повернулась — с поворотом.
async fn send_cart_moves(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState) -> io::Result<()> {
    /// Раз во сколько тактов сообщается место и скорость, и раз во сколько —
    /// место, даже если оно не изменилось.
    const EVERY: u64 = 3;
    const FORCED: u64 = 60;

    /// Меньшие сдвиги и перемены скорости не сообщаются.
    const MOVED: f64 = 7.629_394_531_25e-6;
    const SPED: f64 = 1e-7;

    if player.seen_carts.is_empty() {
        return Ok(());
    }

    let carts: Vec<(i32, u64, u32, crate::minecart::Snapshot, u32)> = shared
        .items_in(player.dimension)
        .lock()
        .expect("предметы захвачены другим потоком")
        .carts()
        .iter()
        .map(|cart| (cart.entity_id, cart.age, cart.teleports, cart.last, cart.primes))
        .collect();

    for (entity_id, age, teleports, now, primes) in carts {
        let Some(mut shown) = player.seen_carts.get(&entity_id).copied() else {
            continue;
        };

        send_cart_data(stream, entity_id, &now).await?;

        // Вагонетку с динамитом подожгли: событие сущности 10 — клиент
        // рисует мигание (minecraft-data 26.1: 0x22, `entity_status`,
        // номер — i32, событие — байт). Уходит в такте поджога, не дожидаясь
        // рассылки (зонд t-act).
        if primes != shown.primes {
            const ENTITY_EVENT: i32 = 0x22;
            const PRIMED: u8 = 10;

            let mut body = encode_varint(ENTITY_EVENT);

            body.extend_from_slice(&entity_id.to_be_bytes());
            body.push(PRIMED);
            write_body(stream, body).await?;
            shown.primes = primes;
        }

        // Печь загорелась или погасла: свойство `fuel` (minecraft-data 26.1,
        // entities.json: четырнадцатое, логическое — вид 8).
        if now.burning != shown.burning {
            const FUEL: u8 = 13;
            const PROPERTY_BOOLEAN: i32 = 8;

            let mut data = encode_varint(SET_ENTITY_DATA);

            data.extend_from_slice(&encode_varint(entity_id));
            data.push(FUEL);
            data.extend_from_slice(&encode_varint(PROPERTY_BOOLEAN));
            data.push(u8::from(now.burning));
            data.push(PROPERTIES_END);
            write_body(stream, data).await?;
            shown.burning = now.burning;
        }

        let speed = (now.vx, now.vy, now.vz);

        // Толкнули или ударили — скорость уходит сразу, а с ней и место, как
        // в очередную рассылку (minecart-push: пока игрок толкает, оригинал
        // каждый такт шлёт скорость и сдвиг; minecart-collide: у ещё не
        // сдвинувшейся — одну скорость). Раз в 60 тактов место уходит только
        // в очередную рассылку. [проверить]: поворот вне очереди.
        let pushed = now.hits != shown.hits || now.impulses != shown.impulses;

        if pushed {
            (shown.hits, shown.impulses) = (now.hits, now.impulses);
            send_velocity(stream, entity_id, speed).await?;
            shown.speed = speed;
        }

        // Такт с появления: сразу после появления — нулевой. Изменились
        // свойства — место и скорость уходят вместе с ними вне очереди
        // (boat-drive: вёсла опущены на +20 — в том же такте и сдвиг, хотя
        // очередь на +18 и +21).
        let tick = age.saturating_sub(1);
        let due = tick % EVERY == 0 || now.dirty != 0;

        if due || pushed {
            let change = (speed.0 - shown.speed.0).powi(2) + (speed.1 - shown.speed.1).powi(2) + (speed.2 - shown.speed.2).powi(2);

            if change > SPED || (speed == (0.0, 0.0, 0.0) && shown.speed != (0.0, 0.0, 0.0)) {
                send_velocity(stream, entity_id, speed).await?;
                shown.speed = speed;
            }

            let place = (now.x, now.y, now.z);
            let grid = grid_of(place);
            let moved = (place.0 - shown.place.0).powi(2) + (place.1 - shown.place.1).powi(2) + (place.2 - shown.place.2).powi(2) >= MOVED;
            let (yaw, pitch) = (cart_angle(now.yaw), cart_angle(now.pitch));
            let turned = yaw != shown.yaw || pitch != shown.pitch;
            let step = (grid.0 - shown.grid.0, grid.1 - shown.grid.1, grid.2 - shown.grid.2);
            let fits = |value: i64| i16::try_from(value).is_ok();
            let whole = Position { x: now.x, y: now.y, z: now.z, yaw: now.yaw, pitch: now.pitch, on_ground: now.on_ground };

            if teleports != shown.teleports || now.on_ground != shown.on_ground || !(fits(step.0) && fits(step.1) && fits(step.2)) {
                send_teleport_with_speed(stream, entity_id, whole, speed).await?;
                (shown.place, shown.grid, shown.yaw, shown.pitch) = (place, grid, yaw, pitch);
                shown.on_ground = now.on_ground;
                shown.teleports = teleports;
            } else if moved || (due && tick % FORCED == 0) || turned {
                let kind = match (moved || (due && tick % FORCED == 0), turned) {
                    (true, true) => ENTITY_POSITION_AND_ROTATION,
                    (true, false) => ENTITY_POSITION,
                    (false, _) => ENTITY_ROTATION,
                };
                let mut body = encode_varint(kind);

                body.extend_from_slice(&encode_varint(entity_id));

                if kind != ENTITY_ROTATION {
                    push_i16(&mut body, step.0 as i16);
                    push_i16(&mut body, step.1 as i16);
                    push_i16(&mut body, step.2 as i16);
                    (shown.place, shown.grid) = (place, grid);
                }

                if kind != ENTITY_POSITION {
                    body.push(yaw);
                    body.push(pitch);
                    (shown.yaw, shown.pitch) = (yaw, pitch);
                }

                body.push(u8::from(now.on_ground));
                write_body(stream, body).await?;
            }
        }

        player.seen_carts.insert(entity_id, shown);
    }

    Ok(())
}

/// Свойства удара, записанные за такт (minecraft-data 26.1,
/// entities.json: восьмое — `hurt`, девятое — `hurtdir`, десятое — `damage`;
/// виды значений из protocol.json: 1 — int, 3 — float).
async fn send_cart_data(stream: &mut Outgoing<'_>, entity_id: i32, now: &crate::minecart::Snapshot) -> io::Result<()> {
    const HURT: u8 = 8;
    const HURT_DIR: u8 = 9;
    const DAMAGE: u8 = 10;
    const PROPERTY_FLOAT: i32 = 3;

    use crate::minecart::{DIRTY_DAMAGE, DIRTY_DIR, DIRTY_HURT, DIRTY_PADDLE};

    if now.dirty == 0 {
        return Ok(());
    }

    let mut data = encode_varint(SET_ENTITY_DATA);

    data.extend_from_slice(&encode_varint(entity_id));

    if now.dirty & DIRTY_HURT != 0 {
        data.push(HURT);
        data.extend_from_slice(&encode_varint(PROPERTY_INT));
        data.extend_from_slice(&encode_varint(now.hurt_time));
    }

    if now.dirty & DIRTY_DIR != 0 {
        data.push(HURT_DIR);
        data.extend_from_slice(&encode_varint(PROPERTY_INT));
        data.extend_from_slice(&encode_varint(now.hurt_dir));
    }

    if now.dirty & DIRTY_DAMAGE != 0 {
        data.push(DAMAGE);
        data.extend_from_slice(&encode_varint(PROPERTY_FLOAT));
        push_f32(&mut data, now.damage);
    }

    // Вёсла лодки (boat-drive: «11=true 12=true»).
    if now.dirty & DIRTY_PADDLE != 0 {
        vehicle::push_paddles(&mut data, now.paddle);
    }

    data.push(PROPERTIES_END);

    write_body(stream, data).await
}

/// Set Entity Motion — скорость сущности (minecraft-data 26.1: 0x65,
/// `entity_velocity`).
async fn send_velocity(stream: &mut Outgoing<'_>, entity_id: i32, (x, y, z): (f64, f64, f64)) -> io::Result<()> {
    const SET_ENTITY_MOTION: i32 = 0x65;

    let mut body = encode_varint(SET_ENTITY_MOTION);

    body.extend_from_slice(&encode_varint(entity_id));
    types::push_velocity(&mut body, x, y, z);

    write_body(stream, body).await
}

/// Add Entity для вагонетки: вид, место, скорость и поворот. Свойства
/// (удар, урон) у новой вагонетки обычные — пакета свойств нет.
async fn send_cart(stream: &mut Outgoing<'_>, cart: &crate::minecart::Minecart) -> io::Result<()> {
    let mut body = encode_varint(ADD_ENTITY);

    body.extend_from_slice(&encode_varint(cart.entity_id));
    body.extend_from_slice(&cart.uuid);
    body.extend_from_slice(&encode_varint(cart.kind.java_type()));
    push_f64(&mut body, cart.x);
    push_f64(&mut body, cart.y);
    push_f64(&mut body, cart.z);
    types::push_velocity(&mut body, cart.vx, cart.vy, cart.vz);
    body.push(cart_angle(cart.pitch));
    body.push(cart_angle(cart.yaw));
    body.push(0);
    body.extend_from_slice(&encode_varint(0));

    write_body(stream, body).await
}

/// Что клиент уже знает из журнала предметов, кроме самих предметов, и где он
/// стоит (для слышимости звуков).
struct Known<'a> {
    carts: &'a mut HashMap<i32, ShownCart>,
    frames: &'a mut HashMap<i32, ShownFrame>,
    mobs: &'a mut HashMap<i32, mobs::ShownMob>,
    listener: (f64, f64, f64),
}

/// Дальше этого звук рамки клиенту не шлётся. [проверить]
const FRAME_HEARD: f64 = 48.0;

/// Рамка, как её знает клиент: `pending` — сразу после появления в конце
/// такта оригинал ещё раз шлёт свойства и нулевой сдвиг (чёрный ящик,
/// item-frame-facing), `meta` — что слать (нет ничего — не шлётся).
struct ShownFrame {
    pending: bool,
    meta: Option<Vec<u8>>,
}

/// Свойства рамки, отличные от умолчаний: сторона (не юг), предмет, поворот.
/// У оригинала пустая рамка, смотрящая на юг, свойств не шлёт вовсе.
fn frame_meta(frame: &crate::frame::Frame) -> Option<Vec<u8>> {
    let mut data = encode_varint(SET_ENTITY_DATA);

    data.extend_from_slice(&encode_varint(frame.entity_id));

    let start = data.len();

    if frame.facing != crate::frame::DEFAULT_FACING {
        data.push(crate::frame::DIRECTION_PROPERTY);
        data.extend_from_slice(&encode_varint(crate::frame::PROPERTY_DIRECTION));
        data.extend_from_slice(&encode_varint(i32::from(frame.facing)));
    }

    if frame.item.is_some() {
        data.push(crate::frame::ITEM_PROPERTY);
        data.extend_from_slice(&encode_varint(PROPERTY_SLOT));
        push_slot(&mut data, frame.item);
    }

    if frame.rotation != 0 {
        data.push(crate::frame::ROTATION_PROPERTY);
        data.extend_from_slice(&encode_varint(PROPERTY_INT));
        data.extend_from_slice(&encode_varint(i32::from(frame.rotation)));
    }

    if data.len() == start {
        return None;
    }

    data.push(PROPERTIES_END);
    Some(data)
}

/// Появление рамки: клетка (не настоящее место), углы стороны, номер
/// стороны в данных; следом свойства, если есть.
async fn send_frame(stream: &mut Outgoing<'_>, frame: &crate::frame::Frame, meta: Option<&[u8]>) -> io::Result<()> {
    let mut body = encode_varint(ADD_ENTITY);
    let (pitch, yaw) = frame.angles();
    let (x, y, z) = frame.spawn_place();

    body.extend_from_slice(&encode_varint(frame.entity_id));
    body.extend_from_slice(&frame.uuid);
    body.extend_from_slice(&encode_varint(frame.java_type()));
    push_f64(&mut body, x);
    push_f64(&mut body, y);
    push_f64(&mut body, z);
    types::push_velocity(&mut body, 0.0, 0.0, 0.0);
    body.push(pitch);
    body.push(yaw);
    body.push(0);
    body.extend_from_slice(&encode_varint(i32::from(frame.facing)));
    write_body(stream, body).await?;

    if let Some(meta) = meta {
        write_body(stream, meta.to_vec()).await?;
    }

    Ok(())
}

/// Показывает клиенту все рамки мира — вошедшему о них журнал не скажет.
async fn send_all_frames(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState) -> io::Result<()> {
    let frames: Vec<crate::frame::Frame> =
        shared.items_in(player.dimension).lock().expect("предметы захвачены другим потоком").frames().to_vec();

    for frame in &frames {
        let meta = frame_meta(frame);

        send_frame(stream, frame, meta.as_deref()).await?;
        player.seen_frames.insert(frame.entity_id, ShownFrame { pending: false, meta });
    }

    Ok(())
}

/// Конец такта у только что появившихся рамок: свойства повторяются, следом —
/// сдвиг на ноль, не на земле (чёрный ящик: так после каждого появления).
async fn send_frame_intros(stream: &mut Outgoing<'_>, player: &mut PlayerState) -> io::Result<()> {
    let mut pending: Vec<i32> = player.seen_frames.iter().filter(|(_, shown)| shown.pending).map(|(id, _)| *id).collect();

    pending.sort_unstable();

    for entity_id in pending {
        let Some(shown) = player.seen_frames.get_mut(&entity_id) else {
            continue;
        };

        shown.pending = false;

        if let Some(meta) = shown.meta.clone() {
            write_body(stream, meta).await?;
        }

        let mut body = encode_varint(ENTITY_POSITION);

        body.extend_from_slice(&encode_varint(entity_id));
        push_i16(&mut body, 0);
        push_i16(&mut body, 0);
        push_i16(&mut body, 0);
        body.push(0);
        write_body(stream, body).await?;
    }

    Ok(())
}

/// Звук сущности в её месте, раздел «нейтральные»: место в восьмых долях,
/// отсекаемых к нулю (как `send_sound`).
async fn send_entity_sound(stream: &mut Outgoing<'_>, (x, y, z): (f64, f64, f64), name: &str) -> io::Result<()> {
    let mut body = encode_varint(SOUND_EFFECT);

    body.extend_from_slice(&encode_varint(0));
    body.extend_from_slice(&encode_string(name));
    body.push(0);
    body.extend_from_slice(&encode_varint(SOUND_NEUTRAL));
    push_i32(&mut body, (x * 8.0) as i32);
    push_i32(&mut body, (y * 8.0) as i32);
    push_i32(&mut body, (z * 8.0) as i32);
    body.extend_from_slice(&1.0f32.to_be_bytes());
    body.extend_from_slice(&1.0f32.to_be_bytes());
    body.extend_from_slice(&0i64.to_be_bytes());

    write_body(stream, body).await
}

/// Отправляет подбор предмета: клиент рисует, как предмет летит к тому,
/// кто его поднял.
async fn send_pick_up(
    stream: &mut Outgoing<'_>,
    entity_id: i32,
    by: i32,
    count: i32,
) -> io::Result<()> {
    let mut body = encode_varint(PICK_UP_ITEM);

    body.extend_from_slice(&encode_varint(entity_id));
    body.extend_from_slice(&encode_varint(by));
    body.extend_from_slice(&encode_varint(count));

    write_body(stream, body).await
}

/// Досылает клиенту всё, что случилось с лежащими предметами.
async fn send_pending_item_changes(
    stream: &mut Outgoing<'_>,
    shared: &Shared,
    dimension: crate::dimension::Dimension,
    from: &mut usize,
    seen: &mut SeenItems,
    known: Known<'_>,
    reader: journal::Reader,
) -> io::Result<()> {
    let Known { carts, frames, mobs, listener } = known;
    let orb_range = f64::from(shared.properties.view_distance.clamp(2, 10)) * 16.0;

    let changes = {
        let mut items = shared
            .items_in(dimension)
            .lock()
            .expect("предметы захвачены другим потоком");

        let (changes, count) = items.changes_since(*from, reader);
        *from = count;

        changes
    };

    for change in changes {
        match change {
            items::Change::Dropped(item) => {
                send_dropped_item(stream, &item).await?;
                seen.insert(item.entity_id, shown_item(&item));
            }
            items::Change::Stack { entity_id, stack, no_gravity } => {
                send_item_properties(stream, entity_id, stack, no_gravity).await?;
            }
            items::Change::Gone { entity_id } => {
                // Провалился в пустоту: просто убираем из мира, без показа
                // подбора. Полоса иссушителя уходит перед ним самим
                // (wither-death-drop: Boss Bar action=1, потом удаление).
                if mobs.get(&entity_id).is_some_and(|shown| shown.boss) {
                    mobs::send_boss_remove(stream, entity_id).await?;
                }

                send_remove_entity(stream, entity_id).await?;
                seen.remove(&entity_id);
                carts.remove(&entity_id);
                frames.remove(&entity_id);
                mobs.remove(&entity_id);
            }
            items::Change::Fuse { entity_id, fuse } => {
                if seen.contains_key(&entity_id) {
                    let mut data = encode_varint(SET_ENTITY_DATA);

                    data.extend_from_slice(&encode_varint(entity_id));
                    data.push(TNT_FUSE_PROPERTY);
                    data.extend_from_slice(&encode_varint(PROPERTY_INT));
                    data.extend_from_slice(&encode_varint(fuse));
                    data.push(PROPERTIES_END);
                    write_body(stream, data).await?;
                }
            }
            items::Change::Cart(cart) => {
                send_cart(stream, &cart).await?;
                carts.insert(cart.entity_id, ShownCart::of(&cart));
            }
            items::Change::Lightning(bolt) => send_lightning(stream, &bolt).await?,
            items::Change::Frame(frame) => {
                let meta = frame_meta(&frame);

                send_frame(stream, &frame, meta.as_deref()).await?;
                frames.insert(frame.entity_id, ShownFrame { pending: true, meta });
            }
            items::Change::FrameItem { entity_id, item } => {
                if frames.contains_key(&entity_id) {
                    let mut data = encode_varint(SET_ENTITY_DATA);

                    data.extend_from_slice(&encode_varint(entity_id));
                    data.push(crate::frame::ITEM_PROPERTY);
                    data.extend_from_slice(&encode_varint(PROPERTY_SLOT));
                    push_slot(&mut data, item);
                    data.push(PROPERTIES_END);
                    write_body(stream, data).await?;
                }
            }
            items::Change::FrameRotation { entity_id, rotation } => {
                if frames.contains_key(&entity_id) {
                    let mut data = encode_varint(SET_ENTITY_DATA);

                    data.extend_from_slice(&encode_varint(entity_id));
                    data.push(crate::frame::ROTATION_PROPERTY);
                    data.extend_from_slice(&encode_varint(PROPERTY_INT));
                    data.extend_from_slice(&encode_varint(i32::from(rotation)));
                    data.push(PROPERTIES_END);
                    write_body(stream, data).await?;
                }
            }
            items::Change::FrameSound { at, name } => {
                // Громкость 1 слышна на 16 блоков; с запасом.
                if (at.0 - listener.0).powi(2) + (at.1 - listener.1).powi(2) + (at.2 - listener.2).powi(2) <= FRAME_HEARD * FRAME_HEARD {
                    send_entity_sound(stream, at, name).await?;
                }
            }
            items::Change::MobEvent { entity_id, at, event } => {
                mobs::send_mob_event(stream, entity_id, at, &event, mobs, listener).await?;
            }
            items::Change::Orb(orb) => {
                // Слежение подхватит шарик само, если он не рядом.
                if (orb.x - listener.0).abs() <= orb_range && (orb.z - listener.2).abs() <= orb_range {
                    let shown = mobs::send_orb(stream, &orb).await?;

                    mobs.insert(orb.entity_id, shown);
                }
            }
            items::Change::Taken { entity_id, by, count, remaining } => {
                send_pick_up(stream, entity_id, by, count).await?;
                if !remaining {
                    send_remove_entity(stream, entity_id).await?;
                    seen.remove(&entity_id);
                    // Из `mobs` номер не убираем: слежение само уберёт шарик,
                    // иначе устаревший снимок шариков показал бы его заново
                    // (чёрный ящик zombie-hit/cow-hit: лишний spawn после подбора).
                }
            }
        }
    }

    Ok(())
}

/// Досылает клиенту движение динамита и падающих блоков: переносом целиком,
/// как только что-то изменилось, — в переносе есть и скорость, по ней
/// клиент досчитывает полёт сам. [проверить]: у оригинала и у них своя
/// очередь рассылки, как у предметов (`send_item_ticks`).
async fn send_item_moves(
    stream: &mut Outgoing<'_>,
    shared: &Shared,
    dimension: crate::dimension::Dimension,
    seen: &mut SeenItems,
) -> io::Result<()> {
    let items: Vec<Dropped> = shared
        .items_in(dimension)
        .lock()
        .expect("предметы захвачены другим потоком")
        .items()
        .iter()
        .filter(|item| item.block.is_some())
        .cloned()
        .collect();

    for item in items {
        // О только что появившемся расскажет журнал — там же он попадёт
        // и в список показанного.
        let Some(shown) = seen.get(&item.entity_id).copied() else {
            continue;
        };

        let now = shown_item(&item);

        if now == shown {
            continue;
        }

        send_teleport_with_speed(
            stream,
            item.entity_id,
            item_place(now.place, now.on_ground),
            now.speed,
        )
        .await?;

        seen.insert(item.entity_id, now);
    }

    Ok(())
}

/// В конце такта досылает клиенту, что стало с выпавшими предметами, —
/// в очередь, как оригинал (зонды probe-item-rest и piston-push-items):
///
/// - о предмете рассказывается раз в 20 тактов от появления и вне очереди —
///   в такте, когда его скорость резко изменилась (`Dropped::impulse`);
/// - тогда уходит скорость, если она заметно изменилась (квадрат разницы
///   больше 1e-7 или стала нулём), и место: шагом, если сдвинулся хотя бы
///   на 1/362 блока, а раз в 60 тактов — в любом случае; сменил «лежит ли»
///   — сверкой места целиком, со скоростью;
/// - в такте появления — только нулевой шаг с «лежит ли» из появления: этот
///   отчёт у оригинала идёт раньше первого такта предмета (у призванного
///   на полу — «не лежит», а через круг — сверка «лежит»).
async fn send_item_ticks(
    stream: &mut Outgoing<'_>,
    shared: &Shared,
    dimension: crate::dimension::Dimension,
    seen: &mut SeenItems,
) -> io::Result<()> {
    /// Раз во сколько тактов о предмете рассказывается в очередь и раз во
    /// сколько — место, даже если оно не изменилось.
    const EVERY: u64 = 20;
    const FORCED: u64 = 60;

    /// Меньшие сдвиги и перемены скорости не сообщаются.
    const MOVED: f64 = 7.629_394_531_25e-6;
    const SPED: f64 = 1e-7;

    let items: Vec<(i32, u64, bool, ShownItem, Stack, bool)> = shared
        .items_in(dimension)
        .lock()
        .expect("предметы захвачены другим потоком")
        .items()
        .iter()
        .filter(|item| item.block.is_none() && seen.contains_key(&item.entity_id))
        .map(|item| (item.entity_id, item.ticks, item.impulse, shown_item(item), item.stack, item.no_gravity))
        .collect();

    for (entity_id, age, impulse, now, stack, no_gravity) in items {
        let Some(mut shown) = seen.get(&entity_id).copied() else {
            continue;
        };

        // Такт с появления: в такте появления — нулевой.
        let tick = age.saturating_sub(1);

        if tick == 0 {
            if !shown.first_sent {
                let mut body = encode_varint(ENTITY_POSITION);

                body.extend_from_slice(&encode_varint(entity_id));
                push_i16(&mut body, 0);
                push_i16(&mut body, 0);
                push_i16(&mut body, 0);
                body.push(u8::from(shown.on_ground));
                write_body(stream, body).await?;
                send_item_properties(stream, entity_id, stack, no_gravity).await?;
                shown.first_sent = true;
                seen.insert(entity_id, shown);
            }
            continue;
        }

        let due = tick.is_multiple_of(EVERY);

        if !due && !impulse {
            continue;
        }

        let speed = now.speed;
        let change = (speed.0 - shown.speed.0).powi(2) + (speed.1 - shown.speed.1).powi(2) + (speed.2 - shown.speed.2).powi(2);

        if change > SPED || (speed == (0.0, 0.0, 0.0) && shown.speed != (0.0, 0.0, 0.0)) {
            send_velocity(stream, entity_id, speed).await?;
            shown.speed = speed;
        }

        let place = now.place;
        let grid = grid_of(place);
        let shown_grid = grid_of(shown.place);
        let moved = (place.0 - shown.place.0).powi(2) + (place.1 - shown.place.1).powi(2) + (place.2 - shown.place.2).powi(2) >= MOVED;
        let forced = due && tick.is_multiple_of(FORCED);
        let step = (grid.0 - shown_grid.0, grid.1 - shown_grid.1, grid.2 - shown_grid.2);
        let fits = |value: i64| i16::try_from(value).is_ok();

        if now.on_ground != shown.on_ground || !(fits(step.0) && fits(step.1) && fits(step.2)) {
            send_teleport_with_speed(stream, entity_id, item_place(place, now.on_ground), speed).await?;
            shown.place = place;
            shown.on_ground = now.on_ground;
        } else if moved || forced {
            let mut body = encode_varint(ENTITY_POSITION);

            body.extend_from_slice(&encode_varint(entity_id));
            push_i16(&mut body, step.0 as i16);
            push_i16(&mut body, step.1 as i16);
            push_i16(&mut body, step.2 as i16);
            body.push(u8::from(now.on_ground));
            write_body(stream, body).await?;
            shown.place = place;
        }

        seen.insert(entity_id, shown);
    }

    Ok(())
}

/// То, что о предмете надо рассказать клиенту.
fn shown_item(item: &Dropped) -> ShownItem {
    ShownItem {
        place: (item.x, item.y, item.z),
        speed: (item.vx, item.vy, item.vz),
        on_ground: item.on_ground,
        first_sent: false,
    }
}

/// Место предмета в том же виде, в каком сервер хранит место игрока.
///
/// Пакеты о движении сущностей одни и те же, а поворота у лежащего предмета
/// нет — отсюда нули.
fn item_place((x, y, z): (f64, f64, f64), on_ground: bool) -> Position {
    Position {
        x,
        y,
        z,
        yaw: 0.0,
        pitch: 0.0,
        on_ground,
    }
}

/// Дописывает «слот» протокола: сколько предметов и какой это предмет.
///
/// Довески передаются только у особого динамита (`slot_components`), у
/// прочих предметов их нет.
fn push_slot(body: &mut Vec<u8>, stack: Option<Stack>) {
    match stack {
        Some(stack) => {
            body.extend_from_slice(&encode_varint(stack.count));
            body.extend_from_slice(&encode_varint(stack.item));

            match stack.tnt.and_then(crate::custom_tnt::by_id) {
                Some(kind) => body.extend_from_slice(&slot_components(kind)),
                None => {
                    // Довески стопки (износ, зачарования…) — компонентами
                    // (stackdata.rs); ничего не убираем.
                    let (added, components) = crate::stackdata::encode_components(&stack.extras());

                    body.extend_from_slice(&encode_varint(added));
                    body.extend_from_slice(&encode_varint(0));
                    body.extend_from_slice(&components);
                }
            }
        }
        None => body.extend_from_slice(&encode_varint(0)),
    }
}

/// Номера довесков предмета в 775 (minecraft-data, protocol.json,
/// SlotComponentType): custom_data, custom_name, lore.
const COMPONENT_CUSTOM_DATA: i32 = 0;
const COMPONENT_CUSTOM_NAME: i32 = 6;
const COMPONENT_LORE: i32 = 11;

/// Довески особого динамита: три добавленных, ни одного убранного —
/// цветное название без курсива, строка описания серым и пометка
/// `{mcsheriffanya_tnt:"вид"}` в custom_data (её клиент в творческом
/// режиме присылает обратно — так вид и узнаётся).
fn slot_components(kind: &crate::custom_tnt::Kind) -> Vec<u8> {
    let text = |text: &str, colour: &str| {
        nbt::Compound::new().string("text", text).string("color", colour).boolean("italic", false).encode_network()
    };

    let mut out = encode_varint(3);

    out.extend_from_slice(&encode_varint(0));
    out.extend_from_slice(&encode_varint(COMPONENT_CUSTOM_NAME));
    out.extend_from_slice(&text(kind.title, kind.colour));
    out.extend_from_slice(&encode_varint(COMPONENT_LORE));
    out.extend_from_slice(&encode_varint(1));
    out.extend_from_slice(&text(kind.lore, "gray"));
    out.extend_from_slice(&encode_varint(COMPONENT_CUSTOM_DATA));
    out.extend_from_slice(&nbt::Compound::new().string(crate::custom_tnt::MARKER, kind.name).encode_network());
    out
}

/// Отправляет выбранный слот панели быстрого доступа.
///
/// Нужно при входе: игрок должен вернуться с тем же предметом в руке, с каким
/// вышел.
async fn send_held_slot(stream: &mut Outgoing<'_>, slot: i32) -> io::Result<()> {
    let mut body = encode_varint(SET_HELD_ITEM);

    body.extend_from_slice(&encode_varint(slot));

    write_body(stream, body).await
}

/// Отправляет инвентарь игрока целиком.
///
/// Этим же он и восстанавливается после перезахода: клиент показывает то,
/// что прислал сервер.
async fn send_container_content(
    stream: &mut Outgoing<'_>,
    player: &mut PlayerState,
) -> io::Result<()> {
    if player.window.is_some() {
        player.inventory_after_window = true;
        return send_window_content(stream, player).await;
    }

    player.state_id += 1;

    let mut body = encode_varint(SET_CONTAINER_CONTENT);

    body.extend_from_slice(&encode_varint(PLAYER_WINDOW));
    body.extend_from_slice(&encode_varint(player.state_id));
    body.extend_from_slice(&encode_varint(inventory::SLOTS as i32));

    for slot in player.inventory.slots() {
        push_slot(&mut body, *slot);
    }

    // Отдельно — то, что игрок держит мышкой.
    push_slot(&mut body, player.inventory.cursor());

    write_body(stream, body).await
}

/// Отправляет содержимое одного слота — когда изменился только он.
async fn send_container_slot(
    stream: &mut Outgoing<'_>,
    player: &mut PlayerState,
    slot: i32,
) -> io::Result<()> {
    // Пока открыто окно хранилища, оригинал окну 0 ничего не шлёт: рюкзак и
    // панель видны в самом окне — туда и уходит перемена (вики, Set
    // Container Slot; чёрный ящик, dropper-pulse: подобранный при открытом
    // сундуке предмет пришёл в ячейку окна 54). Броню и вторую руку клиент
    // узнает, когда окно закроется.
    if let Some(window) = &player.window {
        let shown = window.shown.len();
        let index = match slot {
            inventory::FIRST_MAIN..=inventory::LAST_MAIN => shown + (slot - inventory::FIRST_MAIN) as usize,
            inventory::FIRST_HOTBAR..=inventory::LAST_HOTBAR => shown + 27 + (slot - inventory::FIRST_HOTBAR) as usize,
            _ => {
                player.inventory_after_window = true;
                return Ok(());
            }
        };
        let stack = player.inventory.slot(slot);

        return send_window_slot(stream, player, index, stack).await;
    }

    player.state_id += 1;

    let mut body = encode_varint(SET_CONTAINER_SLOT);

    body.extend_from_slice(&encode_varint(PLAYER_WINDOW));
    body.extend_from_slice(&encode_varint(player.state_id));
    push_i16(&mut body, slot as i16);
    push_slot(&mut body, player.inventory.slot(slot));

    write_body(stream, body).await
}

/// Отправляет клиенту чанки вокруг этого места — те, которых у него ещё нет.
///
/// Мир ровный и бесконечный: чанк складывается, когда до него дошли, и
/// с этого мгновения хранится вместе с остальным миром.
async fn send_chunks_around(
    stream: &mut Outgoing<'_>,
    shared: &Shared,
    dimension: crate::dimension::Dimension,
    (center_x, center_z): (i32, i32),
    sent: &mut SentChunks,
    (radius, view): (i32, i32),
    limit: usize,
) -> io::Result<bool> {
    // Ушедшие из виду чанки клиент выбрасывает сам. Сервер должен об этом
    // знать, иначе, вернувшись, игрок увидит на их месте пустоту: сервер
    // считал бы, что они у клиента уже есть.

    let gone: Vec<(i32, i32)> = sent
        .iter()
        .filter(|(x, z)| (x - center_x).abs() > view || (z - center_z).abs() > view)
        .copied()
        .collect();

    for (chunk_x, chunk_z) in gone {
        send_unload_chunk(stream, chunk_x, chunk_z).await?;
        sent.remove(&(chunk_x, chunk_z));
    }

    // Что показать игроку: сперва ближние чанки, потом дальние — так земля
    // под ногами появляется сразу, а даль дорисовывается. За раз — не больше
    // `limit`: остальное дошлёт следующий вызов, а игрок тем временем уже
    // ходит и видит чат.
    let radius = radius.min(view);
    let mut wanted: Vec<(i32, i32)> = Vec::new();

    for chunk_x in (center_x - radius)..=(center_x + radius) {
        for chunk_z in (center_z - radius)..=(center_z + radius) {
            if !sent.contains(&(chunk_x, chunk_z)) {
                wanted.push((chunk_x, chunk_z));
            }
        }
    }

    wanted.sort_by_key(|(x, z)| (x - center_x).pow(2) + (z - center_z).pow(2));

    let all = wanted.len() <= limit;
    wanted.truncate(limit);

    for place in &wanted {
        sent.insert(*place);
    }

    // Готовим чанки пачками и сразу несколькими руками: складывание куска
    // мира — работа счётная, и на дальности в тридцать два чанка их тысячи.
    // Замок мира при этом не держим: такт не должен ждать землю.
    const AT_ONCE: usize = 16;

    for part in wanted.chunks(AT_ONCE) {
        // Свет у края чанка зависит от соседей: без них там остался бы шов.
        // Поэтому складываются и соседи отправляемых чанков — клиенту они
        // уйдут позже, когда до них дойдёт очередь.
        let mut needed: Vec<(i32, i32)> = Vec::new();

        for (chunk_x, chunk_z) in part.iter().copied() {
            for place in (-1..=1).flat_map(|dx| (-1..=1).map(move |dz| (chunk_x + dx, chunk_z + dz))) {
                if !needed.contains(&place) {
                    needed.push(place);
                }
            }
        }

        let source = {
            let world = shared.world_in(dimension).lock().expect("мир захвачен другим потоком");

            needed.retain(|(chunk_x, chunk_z)| !world.has_chunk(*chunk_x, *chunk_z));
            Arc::new(world.source())
        };

        // Пока чанки складываются и светятся, уже готовое (выгрузка чанков,
        // центр обзора) не должно ждать в очереди.
        stream.flush().await?;

        let mut coming = Vec::new();

        for (chunk_x, chunk_z) in needed {
            let source = Arc::clone(&source);

            coming.push(tokio::task::spawn_blocking(move || {
                (chunk_x, chunk_z, source.take(chunk_x, chunk_z))
            }));
        }

        for waiting in coming {
            let Ok((chunk_x, chunk_z, ready)) = waiting.await else {
                continue;
            };

            shared
                .world_in(dimension)
                .lock()
                .expect("мир захвачен другим потоком")
                .accept(chunk_x, chunk_z, ready);
        }

        // Свет — тоже вне захвата мира и сразу несколькими руками: миру
        // достаются только ссылки на секции, счёт идёт в стороне.
        let mut lighting = Vec::new();

        for (chunk_x, chunk_z) in part.iter().copied() {
            let job = shared
                .world_in(dimension)
                .lock()
                .expect("мир захвачен другим потоком")
                .light_job(chunk_x, chunk_z);

            lighting.push(tokio::task::spawn_blocking(move || job.map(LightJob::finish)));
        }

        for ((chunk_x, chunk_z), waiting) in part.iter().copied().zip(lighting) {
            let light = match waiting.await {
                Ok(Some((light, recounted))) => {
                    // Пересчитанный собственный свет — обратно в чанки,
                    // чтобы не считать его заново.
                    if !recounted.is_empty() {
                        shared
                            .world_in(dimension)
                            .lock()
                            .expect("мир захвачен другим потоком")
                            .store_light(recounted);
                    }

                    light
                }
                _ => ChunkLight::open_sky(),
            };

            send_chunk(stream, shared.world_in(dimension), chunk_x, chunk_z, &light).await?;
        }
    }

    Ok(all)
}

/// Чанки, которые складываются впрок за краем обзора одного игрока: к
/// тому мгновению, как чанк войдёт в обзор, он уже готов, и отправка не ждёт
/// генерации. Клиенту эти чанки не шлются. Насколько далеко и сколько за
/// раз — настройки `prefetch_chunks` и `prefetch_at_once`.
///
/// Складывание идёт в стороне, на пуле блокирующих задач; готовое приходит
/// по каналу и кладётся в мир на следующем круге цикла игрока. Замок мира на
/// время складывания не держится.
struct Prefetch {
    /// Что уже складывается и ещё не пришло.
    pending: std::collections::HashSet<(i32, i32)>,
    sender: std::sync::mpsc::Sender<(i32, i32, world::ReadyChunk)>,
    receiver: std::sync::mpsc::Receiver<(i32, i32, world::ReadyChunk)>,
    /// Вокруг какого чанка кольцо уже всё готово: пока игрок там, искать
    /// нечего.
    done_around: Option<(i32, i32, i32)>,
}

impl Prefetch {
    fn new() -> Prefetch {
        let (sender, receiver) = std::sync::mpsc::channel();

        Prefetch {
            pending: std::collections::HashSet::new(),
            sender,
            receiver,
            done_around: None,
        }
    }

    /// Кладёт в мир то, что успело сложиться. Чанк, который тем временем
    /// появился в мире другим путём (его прислали игроку, в нём уже что-то
    /// поменяли), не заменяется.
    fn collect(&mut self, shared: &Shared, dimension: crate::dimension::Dimension) {
        let arrived: Vec<_> = self.receiver.try_iter().collect();

        if arrived.is_empty() {
            return;
        }

        let mut world = shared.world_in(dimension).lock().expect("мир захвачен другим потоком");

        for (chunk_x, chunk_z, ready) in arrived {
            self.pending.remove(&(chunk_x, chunk_z));

            if !world.has_chunk(chunk_x, chunk_z) {
                world.accept(chunk_x, chunk_z, ready);
            }
        }
    }

    /// Запускает складывание ближайших недостающих чанков кольца от `view`
    /// до `view + prefetch_chunks` вокруг игрока.
    fn step(&mut self, shared: &Shared,
    dimension: crate::dimension::Dimension, (center_x, center_z): (i32, i32), view: i32) {
        let (ahead, at_once) = (shared.settings.prefetch_chunks, shared.settings.prefetch_at_once);

        if ahead <= 0 || self.done_around == Some((center_x, center_z, view)) || self.pending.len() >= at_once {
            return;
        }

        let reach = view + ahead;
        let (missing, source) = {
            let world = shared.world_in(dimension).lock().expect("мир захвачен другим потоком");
            let mut missing: Vec<(i32, i32)> = Vec::new();

            for chunk_x in (center_x - reach)..=(center_x + reach) {
                for chunk_z in (center_z - reach)..=(center_z + reach) {
                    let inside = (chunk_x - center_x).abs() <= view && (chunk_z - center_z).abs() <= view;

                    if !inside && !self.pending.contains(&(chunk_x, chunk_z)) && !world.has_chunk(chunk_x, chunk_z) {
                        missing.push((chunk_x, chunk_z));
                    }
                }
            }

            (missing, Arc::new(world.source()))
        };

        if missing.is_empty() {
            if self.pending.is_empty() && self.done_around.is_none() {
                log_debug!("Впрок: кольцо до {} чанков вокруг {} {} готово", reach, center_x, center_z);
            }

            if self.pending.is_empty() {
                self.done_around = Some((center_x, center_z, view));
            }

            return;
        }

        self.done_around = None;

        let mut missing = missing;
        missing.sort_by_key(|(x, z)| (x - center_x).pow(2) + (z - center_z).pow(2));

        for (chunk_x, chunk_z) in missing.into_iter().take(at_once - self.pending.len()) {
            let source = Arc::clone(&source);
            let sender = self.sender.clone();

            self.pending.insert((chunk_x, chunk_z));
            tokio::task::spawn_blocking(move || {
                // Игрок мог уйти, и приёмника уже нет — тогда чанк не нужен.
                let _ = sender.send((chunk_x, chunk_z, source.take(chunk_x, chunk_z)));
            });
        }
    }
}

/// Отправляет Unload Chunk — «этот чанк можно забыть».
///
/// Поля идут наоборот привычному: сперва Z, потом X.
async fn send_unload_chunk(stream: &mut Outgoing<'_>, chunk_x: i32, chunk_z: i32) -> io::Result<()> {
    let mut body = encode_varint(UNLOAD_CHUNK);

    push_i32(&mut body, chunk_z);
    push_i32(&mut body, chunk_x);

    write_body(stream, body).await
}

/// Отправляет Set Entity Data с тем, как игрок выглядит: ведущей рукой и
/// набором показываемых частей скина.
///
/// Без этого пакета клиент рисует чужого игрока в одном слое — сам скин у
/// него уже есть, но второй слой, шапка и куртка, считается выключенным, —
/// и любого игрока правшой.
async fn send_look(
    stream: &mut Outgoing<'_>,
    entity_id: i32,
    skin_parts: u8,
    main_hand: u8,
    sneaking: bool,
) -> io::Result<()> {
    let mut body = encode_varint(SET_ENTITY_DATA);

    body.extend_from_slice(&encode_varint(entity_id));

    // Свойства идут списком: номер, вид значения, само значение.
    // Присевшего клиент рисует по двум: флагу «приседает» среди общих
    // флагов сущности и позе «присел».
    body.push(FLAGS_PROPERTY);
    body.extend_from_slice(&encode_varint(PROPERTY_BYTE));
    body.push(if sneaking { FLAG_CROUCHING } else { 0 });

    body.push(POSE_PROPERTY);
    body.extend_from_slice(&encode_varint(PROPERTY_POSE));
    body.extend_from_slice(&encode_varint(if sneaking { POSE_CROUCHING } else { POSE_STANDING }));

    body.push(MAIN_HAND_PROPERTY);
    body.extend_from_slice(&encode_varint(PROPERTY_HUMANOID_ARM));
    body.extend_from_slice(&encode_varint(main_hand as i32));

    body.push(SKIN_PARTS_PROPERTY);
    body.extend_from_slice(&encode_varint(PROPERTY_BYTE));
    body.push(skin_parts);

    body.push(PROPERTIES_END);

    write_body(stream, body).await
}

/// Отправляет Entity Animation: чужой игрок взмахнул рукой.
async fn send_swing(stream: &mut Outgoing<'_>, entity_id: i32) -> io::Result<()> {
    let mut body = encode_varint(ENTITY_ANIMATION);

    body.extend_from_slice(&encode_varint(entity_id));
    body.push(ANIMATION_SWING_MAIN_ARM);

    write_body(stream, body).await
}

/// Кладёт в общий список то, что остальным нужно знать об игроке: где он
/// стоит и что держит в руке. Рассылают это уже их подключения.
fn publish_player(shared: &Shared, player: &PlayerState) {
    let _publication = shared.publication.lock().expect("публикация игрока");
    publish_player_inner(shared, player);
}

fn publish_player_inner(shared: &Shared, player: &PlayerState) {
    let gear = player.gear();
    let sounds = {
        let players = shared.players.lock().expect("игроки");
        equipment_sound::changed(&players.gear(&player.uuid).worn, &gear.worn)
    };
    if !sounds.is_empty() {
        let mut world = shared.world_in(player.dimension).lock().expect("мир");
        for sound in sounds {
            world.note_sound_player_at((player.x, player.y, player.z), sound, 1.0, 1.0);
        }
    }
    let mut players = shared.players.lock().expect("список игроков захвачен другим потоком");
    players.set_position(&player.uuid, player.position());
    players.set_held(&player.uuid, player.inventory.held());
    players.set_gear(&player.uuid, gear);
}

/// Сколько тактов миру от роду, который в нём час и сколько раз часы
/// переводили (`World::clock_changes`). Часы — у обычного мира: время
/// суток одно на сервер.
fn world_age(shared: &Shared, dimension: crate::dimension::Dimension) -> (i64, i64, (u64, i64)) {
    let age = shared.world_in(dimension).lock().expect("мир захвачен другим потоком").tick() as i64;
    let world = shared.world_in(Dimension::Overworld).lock().expect("мир захвачен другим потоком");

    (age, world.time_of_day(), world.clock_changes())
}

/// Отправляет Update Time: который час в мире.
///
/// Часы у нас одни — те, что объявлены в реестре мировых часов, поэтому
/// запись одна, с номером 0. Скорость обычная, доля такта нулевая:
/// время идёт ровно по тактам сервера. `time` None — ежесекундная сверка
/// без часов: так у оригинала, часы клиент ведёт сам, а запись о них
/// приходит при входе и после перевода (зонд 02.10, чёрный ящик
/// bed-sleep-night).
async fn send_time(stream: &mut Outgoing<'_>, age: i64, time: Option<i64>) -> io::Result<()> {
    let mut body = encode_varint(UPDATE_TIME);

    body.extend_from_slice(&age.to_be_bytes());

    match time {
        Some(time) => {
            body.extend_from_slice(&encode_varint(1)); // одни часы
            body.extend_from_slice(&encode_varint(0)); // их номер в реестре
            body.extend_from_slice(&encode_varlong(time));
            body.extend_from_slice(&0.0f32.to_be_bytes());
            body.extend_from_slice(&1.0f32.to_be_bytes());
        }
        None => body.extend_from_slice(&encode_varint(0)),
    }

    write_body(stream, body).await
}

/// Места экипировки протокола (EquipmentSlot): основная рука 0, вторая 1,
/// ботинки 2, штаны 3, нагрудник 4, шлем 5.
const EQUIPMENT_OFF_HAND: u8 = 1;
const EQUIPMENT_FEET: u8 = 2;
const EQUIPMENT_LEGS: u8 = 3;
const EQUIPMENT_CHEST: u8 = 4;
const EQUIPMENT_HEAD: u8 = 5;

/// Вторая рука и броня (`Gear::worn`: вторая рука, шлем, нагрудник, штаны,
/// ботинки) как пары «место протокола, стопка».
fn worn_slots(worn: &[Option<Stack>; 5]) -> [(u8, Option<Stack>); 5] {
    [
        (EQUIPMENT_OFF_HAND, worn[0]),
        (EQUIPMENT_HEAD, worn[1]),
        (EQUIPMENT_CHEST, worn[2]),
        (EQUIPMENT_LEGS, worn[3]),
        (EQUIPMENT_FEET, worn[4]),
    ]
}

/// Отправляет Set Equipment — что у сущности в этих местах.
///
/// Места предметов перечисляются подряд, и длина списка нигде не указана:
/// у каждого, кроме последнего, в номере места стоит старший бит.
async fn send_equipment(
    stream: &mut Outgoing<'_>,
    entity_id: i32,
    slots: &[(u8, Option<Stack>)],
) -> io::Result<()> {
    let mut body = encode_varint(SET_EQUIPMENT);

    body.extend_from_slice(&encode_varint(entity_id));

    for (index, (slot, stack)) in slots.iter().enumerate() {
        body.push(if index + 1 < slots.len() { slot | 0x80 } else { *slot });
        push_slot(&mut body, *stack);
    }

    write_body(stream, body).await
}

/// Отправляет Remove Entities — сущность игрока пропадает из мира.
async fn send_remove_entity(stream: &mut Outgoing<'_>, entity_id: i32) -> io::Result<()> {
    let mut body = encode_varint(REMOVE_ENTITIES);

    body.extend_from_slice(&encode_varint(1)); // одна сущность
    body.extend_from_slice(&encode_varint(entity_id));

    write_body(stream, body).await
}

/// Рассказывает клиенту, где теперь стоят чужие игроки.
///
/// Положения сравниваются с теми, что этому клиенту уже сообщили: смещение
/// считается от них, и если отправить его от другой точки, игрок на экране
/// окажется не там, где он на самом деле.
async fn send_other_moves(
    stream: &mut Outgoing<'_>,
    shared: &Shared,
    player: &PlayerState,
    seen: &mut Seen,
    phase: Phase,
) -> io::Result<()> {
    // Круг подключения идёт по нескольку раз за такт, и копировать каждый
    // раз всех игроков целиком (с ником и скином) незачем: сверке хватает
    // того, что видно другим. Целиком копируется только тот, кого клиенту
    // надо показать впервые.
    let others: Vec<(Shown, i32, bool, Option<Member>, crate::players::Gear)> = {
        let players = shared.players.lock().expect("список игроков захвачен другим потоком");

        let members = if phase == Phase::Moves {
            players.members().iter().map(|m| (m.clone(), players.gear(&m.uuid))).collect()
        } else { player.publication.as_ref().map_or_else(|| players.published().to_vec(), |p| p.players.clone()) };
        members.iter()
        .filter(|(member, _)| member.uuid != player.uuid)
        .map(|(member, gear)| {
            let now = Shown {
                position: member.position,
                skin_parts: member.skin_parts,
                main_hand: member.main_hand,
                held: member.held,
                worn: gear.worn,
                speed_bits: u64::from(gear.speed_touch),
                sprinting: gear.sprinting,
                living: gear.living,
                effect_colors: gear.effect_colors,
                health_bits: gear.health.to_bits(),
                air: gear.air,
                sneaking: member.sneaking,
                swings: member.swings,
                sleeping: gear.sleeping,
            };
            let here = member.dimension == player.dimension;
            let new = (here && !seen.contains_key(&member.entity_id)).then(|| member.clone());

            (now, member.entity_id, here, new, gear.clone())
        })
        .collect()
    };

    for (now, entity_id, here, new, gear) in others {
        // Видно только тех, кто в том же измерении: пришёл — показать,
        // ушёл в другое — убрать.
        let Some(shown) = seen.get(&entity_id).copied() else {
            if let Some(member) = new {
                send_add_entity(stream, &member).await?;
                send_look(stream, member.entity_id, member.skin_parts, member.main_hand, member.sneaking).await?;
                let mut slots = vec![(EQUIPMENT_MAIN_HAND, member.held)];

                slots.extend(worn_slots(&now.worn).into_iter().filter(|(_, stack)| stack.is_some()));
                send_equipment(stream, member.entity_id, &slots).await?;
                // Спящего покажет следующий проход (поза и кровать).
                seen.insert(entity_id, Shown { sleeping: None, ..now });
            }
            continue;
        };

        if !here {
            send_remove_entity(stream, entity_id).await?;
            seen.remove(&entity_id);
            continue;
        }

        // Взмах и экипировка идут отдельными проходами, после событий мира:
        // взмах — перед здоровьем, экипировка — после него (чёрный ящик,
        // pvp-fist, pvp-sword-charged).
        let mut now = now;

        if phase != Phase::Moves {
            let mut only = shown;

            if phase == Phase::Swings {
                only.swings = now.swings;
                only.sprinting = now.sprinting;
                only.living = now.living;
                only.effect_colors = now.effect_colors;
                only.health_bits = now.health_bits;
                only.air = now.air;
            } else {
                (only.held, only.worn, only.speed_bits) = (now.held, now.worn, now.speed_bits);
            }

            if only == shown {
                continue;
            }

            if only.swings != shown.swings {
                send_swing(stream, entity_id).await?;
            }

            // Бег чужого игрока: флаг сущности и атрибут скорости шага.
            if only.sprinting != shown.sprinting {
                let mut data = encode_varint(SET_ENTITY_DATA);

                data.extend_from_slice(&encode_varint(entity_id));
                data.push(FLAGS_PROPERTY);
                data.extend_from_slice(&encode_varint(PROPERTY_BYTE));
                data.push(if only.sprinting { 0x08 } else { 0 } | if now.sneaking { FLAG_CROUCHING } else { 0 });
                data.push(PROPERTIES_END);
                write_body(stream, data).await?;

                let movement = gear.movement_speed.clone();

                if let Some(attribute) = movement {
                    let mut body = encode_varint(0x83);

                    body.extend_from_slice(&crate::attributes::encode_update(entity_id, &[attribute]));
                    write_body(stream, body).await?;
                }
            }

            // Использует предмет (щит, лук), вращается «тягуном»: флаги
            // живой сущности и поза вращения одним пакетом (чёрный ящик,
            // trident-riptide: «6=0 8=0» на +20).
            // Воздух (свойство 1) идёт в одном пакете с позой и флагами: на +20
            // у оригинала `1=282 6=0 8=0` (чёрный ящик, trident-riptide-hit).
            if only.living != shown.living || only.air != shown.air {
                let mut data = encode_varint(SET_ENTITY_DATA);

                data.extend_from_slice(&encode_varint(entity_id));

                if only.air != shown.air {
                    data.push(1);
                    data.extend_from_slice(&encode_varint(PROPERTY_INT));
                    data.extend_from_slice(&encode_varint(only.air));
                }

                const POSE_BITS: u8 = crate::players::SPIN_POSE_BIT | crate::players::GLIDE_BIT;

                // Полёт на элитрах: флаг сущности 0x80 (с бегом и приседанием
                // в том же байте) и поза «летит» (1).
                if (only.living ^ shown.living) & crate::players::GLIDE_BIT != 0 {
                    data.push(FLAGS_PROPERTY);
                    data.extend_from_slice(&encode_varint(PROPERTY_BYTE));
                    data.push(if only.living & crate::players::GLIDE_BIT != 0 { 0x80 } else { 0 } | if only.sprinting { 0x08 } else { 0 } | if now.sneaking { FLAG_CROUCHING } else { 0 });
                }

                if (only.living ^ shown.living) & POSE_BITS != 0 {
                    data.push(POSE_PROPERTY);
                    data.extend_from_slice(&encode_varint(PROPERTY_POSE));
                    data.extend_from_slice(&encode_varint(if only.living & crate::players::SPIN_POSE_BIT != 0 {
                        vitals::POSE_SPIN_ATTACK
                    } else if only.living & crate::players::GLIDE_BIT != 0 {
                        1
                    } else if now.sneaking {
                        POSE_CROUCHING
                    } else {
                        POSE_STANDING
                    }));
                }

                if (only.living ^ shown.living) & !POSE_BITS != 0 {
                    data.push(LIVING_FLAGS_PROPERTY_ID);
                    data.extend_from_slice(&encode_varint(PROPERTY_BYTE));
                    data.push(only.living & !POSE_BITS);
                }

                data.push(PROPERTIES_END);
                write_body(stream, data).await?;
            }

            // Частицы эффектов: цвет в свойстве 10 (пустой список — сняты).
            if only.effect_colors != shown.effect_colors {
                let mut data = encode_varint(SET_ENTITY_DATA);

                data.extend_from_slice(&encode_varint(entity_id));
                vitals::push_effect_particles(&mut data, &only.effect_colors);
                data.push(PROPERTIES_END);
                write_body(stream, data).await?;
            }

            // Здоровье чужого игрока: свойство 9 с числом.
            if only.health_bits != shown.health_bits {
                let mut data = encode_varint(SET_ENTITY_DATA);

                data.extend_from_slice(&encode_varint(entity_id));
                data.push(9);
                data.extend_from_slice(&encode_varint(3));
                data.extend_from_slice(&f32::from_bits(only.health_bits).to_be_bytes());
                data.push(PROPERTIES_END);
                write_body(stream, data).await?;
            }

            let mut changed = Vec::new();

            if only.held != shown.held {
                changed.push((EQUIPMENT_MAIN_HAND, only.held));
            }

            for ((slot, stack), (_, before)) in worn_slots(&only.worn).into_iter().zip(worn_slots(&shown.worn)) {
                if stack != before {
                    changed.push((slot, stack));
                }
            }

            if !changed.is_empty() {
                send_equipment(stream, entity_id, &changed).await?;
            }

            // Скорость удара чужого игрока (оружие в руке) — атрибутом: у
            // оригинала он уходит следом за экипировкой.
            if only.speed_bits != shown.speed_bits
                && let Some(attribute) = gear.attack_speed.clone()
            {
                let mut body = encode_varint(0x83);

                body.extend_from_slice(&crate::attributes::encode_update(entity_id, &[attribute]));
                write_body(stream, body).await?;
            }

            seen.insert(entity_id, only);
            continue;
        }

        (now.swings, now.held, now.worn, now.speed_bits, now.sprinting, now.living, now.effect_colors, now.health_bits, now.air) = (shown.swings, shown.held, shown.worn, shown.speed_bits, shown.sprinting, shown.living, shown.effect_colors, shown.health_bits, shown.air);

        if now == shown {
            continue;
        }

        if now.skin_parts != shown.skin_parts || now.main_hand != shown.main_hand || now.sneaking != shown.sneaking {
            send_look(stream, entity_id, now.skin_parts, now.main_hand, now.sneaking).await?;
        }

        if now.position != shown.position {
            send_move(stream, entity_id, shown.position, now.position).await?;
        }

        // Лёг или встал: поза (спит — 2) и место кровати (свойство 14).
        if now.sleeping != shown.sleeping {
            let mut data = encode_varint(SET_ENTITY_DATA);

            data.extend_from_slice(&encode_varint(entity_id));
            data.push(POSE_PROPERTY);
            data.extend_from_slice(&encode_varint(PROPERTY_POSE));
            data.extend_from_slice(&encode_varint(if now.sleeping.is_some() { POSE_SLEEPING } else if now.sneaking { POSE_CROUCHING } else { POSE_STANDING }));
            data.push(BED_PROPERTY);
            data.extend_from_slice(&encode_varint(PROPERTY_OPTIONAL_BLOCK_POS));

            match now.sleeping {
                Some((x, y, z)) => {
                    data.push(1);
                    push_packed_position(&mut data, x, y, z);
                }
                None => data.push(0),
            }

            data.push(PROPERTIES_END);
            write_body(stream, data).await?;
        }

        seen.insert(entity_id, now);
    }

    Ok(())
}

/// Рассказывает клиенту о перемещении одной сущности — самыми дешёвыми
/// пакетами, каких хватает.
///
/// Смещение записывается коротким числом, поэтому большое перемещение так
/// не рассказать: тогда сообщаются координаты целиком.
async fn send_move(
    stream: &mut Outgoing<'_>,
    entity_id: i32,
    from: Position,
    to: Position,
) -> io::Result<()> {
    // Слишком далеко шагнул — дельтой не рассказать, переносим целиком.
    let Some(body) = move_body(entity_id, from, to) else {
        return send_teleport(stream, entity_id, to).await;
    };

    write_body(stream, body).await?;

    // Поворот головы у чужого игрока клиент берёт из отдельного пакета.
    if to.yaw != from.yaw || to.pitch != from.pitch {
        let mut head = encode_varint(SET_HEAD_ROTATION);
        head.extend_from_slice(&encode_varint(entity_id));
        head.push(types::angle(to.yaw));

        write_body(stream, head).await?;
    }

    Ok(())
}

/// Собирает пакет о перемещении сущности.
///
/// Пакетов три, и выбор между ними — по тому, что изменилось: сдвинулся,
/// повернулся или и то и другое. None означает, что шаг слишком велик и
/// рассказать о нём надо переносом.
///
/// Важно, что поля пакета зависят только от его вида, а не от того, что
/// изменилось. Игрок может, скажем, лишь оторваться от земли, не сдвинувшись
/// и не повернувшись, — но пакет всё равно берётся один из трёх, и углы в нём
/// должны стоять на своих местах. Иначе клиент читает поле, которого нет,
/// и обрывает соединение.
fn move_body(entity_id: i32, from: Position, to: Position) -> Option<Vec<u8>> {
    let moved = to.x != from.x || to.y != from.y || to.z != from.z;
    let turned = to.yaw != from.yaw || to.pitch != from.pitch;

    let (step_x, step_y, step_z) = (
        types::delta(to.x - from.x)?,
        types::delta(to.y - from.y)?,
        types::delta(to.z - from.z)?,
    );

    let kind = match (moved, turned) {
        (true, true) => ENTITY_POSITION_AND_ROTATION,
        (true, false) => ENTITY_POSITION,
        (false, _) => ENTITY_ROTATION,
    };

    let mut body = encode_varint(kind);

    body.extend_from_slice(&encode_varint(entity_id));

    if kind != ENTITY_ROTATION {
        push_i16(&mut body, step_x);
        push_i16(&mut body, step_y);
        push_i16(&mut body, step_z);
    }

    if kind != ENTITY_POSITION {
        body.push(types::angle(to.yaw));
        body.push(types::angle(to.pitch));
    }

    body.push(u8::from(to.on_ground));

    Some(body)
}

/// Переносит сущность на новое место целиком.
async fn send_teleport(stream: &mut Outgoing<'_>, entity_id: i32, to: Position) -> io::Result<()> {
    send_teleport_with_speed(stream, entity_id, to, (0.0, 0.0, 0.0)).await
}

/// То же, но со скоростью сущности.
///
/// Скорость здесь не украшение: клиент досчитывает движение сущности сам,
/// между пакетами сервера. Если ему не сказать, что предмет остановился, он
/// продолжит тянуть его вниз — и предмет уедет в пол.
async fn send_teleport_with_speed(
    stream: &mut Outgoing<'_>,
    entity_id: i32,
    to: Position,
    speed: (f64, f64, f64),
) -> io::Result<()> {
    let mut body = encode_varint(TELEPORT_ENTITY);

    body.extend_from_slice(&encode_varint(entity_id));
    push_f64(&mut body, to.x);
    push_f64(&mut body, to.y);
    push_f64(&mut body, to.z);

    // За координатами идут не смещения, а скорость.
    push_f64(&mut body, speed.0);
    push_f64(&mut body, speed.1);
    push_f64(&mut body, speed.2);

    // Углы в этом пакете — обычные дробные числа, а не байты.
    push_f32(&mut body, to.yaw);
    push_f32(&mut body, to.pitch);

    body.push(u8::from(to.on_ground));

    write_body(stream, body).await
}

/// Отправляет System Chat Message — сообщение в чат.
async fn send_system_chat(stream: &mut Outgoing<'_>, text: &str) -> io::Result<()> {
    let mut body = encode_varint(SYSTEM_CHAT);

    push_text_component(&mut body, text);
    body.push(0x00); // в чат, а не строкой над панелью предметов

    write_body(stream, body).await
}

/// Записывает текст сообщения так, как его ждёт клиент.
///
/// Кодирование живёт в общем сборщике данных движка: там же, где собираются
/// реестры. Своя копия тут уже один раз разошлась с ним — в сетевом виде имя
/// корневой метки не записывается, а здесь оно писалось.
fn push_text_component(out: &mut Vec<u8>, text: &str) {
    out.extend_from_slice(&nbt::text_component(text));
}

/// Отправляет Commands — дерево команд, по которому клиент подсказывает ввод
/// после косой черты.
///
/// Узлы дерева бывают трёх видов: корень, слово и аргумент. У нас пока только
/// слова, поэтому дерево простое. Слово, на котором команда считается
/// законченной, помечается отдельным битом — без него клиент не даст
/// отправить команду.
///
/// Ветка `tnt` (особый динамит) — только тем, кому она позволена
/// (`may_tnt`): остальным её незачем и видеть. Права, выданные позже,
/// откроют её при следующем входе.
async fn send_commands(stream: &mut Outgoing<'_>, may_tnt: bool) -> io::Result<()> {
    /// Виды узлов — младшие два бита флагов.
    const NODE_ROOT: u8 = 0x00;
    const NODE_LITERAL: u8 = 0x01;

    /// Бит «на этом узле команда закончена и её можно выполнить».
    const EXECUTABLE: u8 = 0x04;

    /// Вид узла «довесок»: то, что игрок дописывает своими словами.
    const NODE_ARGUMENT: u8 = 0x02;

    /// Разборщик «строка» и его поведение: одно слово или весь остаток.
    const PARSER_STRING: i32 = 5;
    const SINGLE_WORD: i32 = 0;
    const GREEDY: i32 = 2;

    /// Бит «подсказки к этому довеску спрашивать у сервера».
    const ASK_SERVER: u8 = 0x10;

    /// Узел-довесок: имя, разборщик и то, как он читает написанное.
    ///
    /// Подсказки к довеску клиент спрашивает у сервера: имена игроков знает
    /// только он.
    fn argument(name: &str, children: &[i32], behaviour: i32) -> Vec<u8> {
        let mut node = vec![NODE_ARGUMENT | EXECUTABLE | ASK_SERVER];

        node.extend_from_slice(&encode_varint(children.len() as i32));

        for child in children {
            node.extend_from_slice(&encode_varint(*child));
        }

        node.extend_from_slice(&encode_string(name));
        node.extend_from_slice(&encode_varint(PARSER_STRING));
        node.extend_from_slice(&encode_varint(behaviour));
        node.extend_from_slice(&encode_string("minecraft:ask_server"));
        node
    }

    /// Разборщик «целое число» и его флаг «есть нижняя граница».
    const PARSER_INTEGER: i32 = 3;
    const HAS_MIN: u8 = 0x01;

    /// Узел-число не меньше `min`: клиент сам проверит и подскажет.
    fn integer(name: &str, min: i32) -> Vec<u8> {
        let mut node = vec![NODE_ARGUMENT | EXECUTABLE];

        node.extend_from_slice(&encode_varint(0));
        node.extend_from_slice(&encode_string(name));
        node.extend_from_slice(&encode_varint(PARSER_INTEGER));
        node.push(HAS_MIN);
        node.extend_from_slice(&min.to_be_bytes());
        node
    }

    /// Узел-слово: имя и номера узлов-продолжений.
    fn literal(name: &str, children: &[i32], executable: bool) -> Vec<u8> {
        let mut node = vec![NODE_LITERAL | if executable { EXECUTABLE } else { 0 }];

        node.extend_from_slice(&encode_varint(children.len() as i32));

        for child in children {
            node.extend_from_slice(&encode_varint(*child));
        }

        node.extend_from_slice(&encode_string(name));
        node
    }

    // Номера узлов известны заранее, поэтому дерево описывается сразу целиком:
    // корень, слово «gamemode» и слова-продолжения к нему.
    let mut nodes: Vec<Vec<u8>> = Vec::new();

    nodes.push({
        let mut root = vec![NODE_ROOT];

        let mut children = vec![1, 6, 7, 8, 11, 13, 15, 17, 19, 21, 23, 25, 27, 29, 31];

        if may_tnt {
            children.push(TNT_NODE);
        }

        // Команды мира (commands_world.rs) — после ветки tnt.
        let world_base = TNT_NODE + if may_tnt { TNT_NODES } else { 0 };

        children.extend((0..WORLD_COMMANDS.len() as i32).map(|index| world_base + index * 2));

        root.extend_from_slice(&encode_varint(children.len() as i32));

        for child in children {
            root.extend_from_slice(&encode_varint(child));
        }

        root
    });

    nodes.push(literal("gamemode", &[2, 3, 4, 5], false));
    nodes.push(literal("creative", &[], true));
    nodes.push(literal("survival", &[], true));
    nodes.push(literal("adventure", &[], true));
    nodes.push(literal("spectator", &[], true));
    nodes.push(literal("list", &[], true));
    nodes.push(literal("help", &[], true));

    // kick <игрок> [причина]: имя одним словом, причина — весь остаток.
    nodes.push(literal("kick", &[9], false));
    nodes.push(argument("игрок", &[10], SINGLE_WORD));
    nodes.push(argument("причина", &[], GREEDY));

    // tp: и «куда», и «кого куда» читаются одним куском — разбирает сервер.
    nodes.push(literal("tp", &[12], false));
    nodes.push(argument("куда", &[], GREEDY));

    // give <игрок> <предмет> [сколько]: всё после имени разбирает сервер.
    nodes.push(literal("give", &[14], false));
    nodes.push(argument("игрок", &[], GREEDY));

    // clear [игрок]
    nodes.push(literal("clear", &[16], true));
    nodes.push(argument("игрок", &[], SINGLE_WORD));

    // op <игрок>
    nodes.push(literal("op", &[18], false));
    nodes.push(argument("игрок", &[], SINGLE_WORD));

    // deop <игрок>
    nodes.push(literal("deop", &[20], false));
    nodes.push(argument("игрок", &[], SINGLE_WORD));

    // time set|add|query
    nodes.push(literal("time", &[22], false));
    nodes.push(argument("как", &[], GREEDY));

    // setblock <x> <y> <z> <блок> и fill <x1> <y1> <z1> <x2> <y2> <z2> <блок>
    nodes.push(literal("setblock", &[24], false));
    nodes.push(argument("где и что", &[], GREEDY));
    nodes.push(literal("fill", &[26], false));
    nodes.push(argument("откуда докуда и что", &[], GREEDY));

    // item replace block <x> <y> <z> container.<N> with <предмет> [сколько]
    nodes.push(literal("item", &[28], false));
    nodes.push(argument("что куда", &[], GREEDY));

    // summon <вагонетка> <x> <y> <z> и kill @e[type=<вагонетка>]
    nodes.push(literal("summon", &[30], false));
    nodes.push(argument("что где", &[], GREEDY));
    nodes.push(literal("kill", &[32], false));
    nodes.push(argument("кого", &[], GREEDY));

    // tnt give <игрок> <вид> [сколько] и tnt list: игрока и вид подсказывает
    // сервер (виды — из таблицы custom_tnt.rs), число проверяет клиент.
    const TNT_NODE: i32 = 33;
    const TNT_NODES: i32 = 6;

    /// Команды мира (commands_world.rs).
    const WORLD_COMMANDS: [&str; 15] = [
        "worldborder", "tick", "clone", "attribute", "enchant", "locate", "forceload", "spreadplayers", "random", "ride", "rotate", "spectate",
        "fillbiome", "place", "loot",
    ];

    if may_tnt {
        nodes.push(literal("tnt", &[TNT_NODE + 1, TNT_NODE + 5], false));
        nodes.push(literal("give", &[TNT_NODE + 2], false));
        nodes.push(argument("игрок", &[TNT_NODE + 3], SINGLE_WORD));
        nodes.push(argument("вид", &[TNT_NODE + 4], SINGLE_WORD));
        nodes.push(integer("сколько", 1));
        nodes.push(literal("list", &[], true));
    }

    // Команды мира: слово и всё остальное одним куском — разбирает сервер.
    let world_base = nodes.len() as i32;

    for (index, name) in WORLD_COMMANDS.iter().enumerate() {
        let argument_node = world_base + index as i32 * 2 + 1;

        nodes.push(literal(name, &[argument_node], true));
        nodes.push(argument("что", &[], GREEDY));
    }

    // Команды табло и функций (commands_score.rs): слово и весь остаток —
    // разбирает сервер. `trigger` и `teammsg` — всем, как у оригинала.
    let mut extra_roots = Vec::new();

    for name in ["scoreboard", "team", "tag", "bossbar", "trigger", "teammsg", "tm", "function", "schedule", "datapack", "return", "execute"] {
        let at = nodes.len() as i32;

        extra_roots.push(at);
        nodes.push(literal(name, &[at + 1], false));
        nodes.push(argument("доводы", &[], GREEDY));
    }

    // Команды commands_admin.rs: свои узлы за остальными.
    let (admin_roots, admin_nodes) = crate::commands_admin::tree_nodes(nodes.len() as i32);

    nodes.extend(admin_nodes);

    // Корень — заново, с новыми словами.
    {
        let mut root = vec![NODE_ROOT];
        let mut children = vec![1, 6, 7, 8, 11, 13, 15, 17, 19, 21, 23, 25, 27, 29, 31];

        if may_tnt {
            children.push(TNT_NODE);
        }

        children.extend((0..WORLD_COMMANDS.len() as i32).map(|index| world_base + index * 2));
        children.extend(extra_roots);
        children.extend(admin_roots);
        root.extend_from_slice(&encode_varint(children.len() as i32));

        for child in children {
            root.extend_from_slice(&encode_varint(child));
        }

        nodes[0] = root;
    }

    let mut body = encode_varint(COMMANDS);

    body.extend_from_slice(&encode_varint(nodes.len() as i32));

    for node in &nodes {
        body.extend_from_slice(node);
    }

    body.extend_from_slice(&encode_varint(0)); // корень — узел номер 0

    write_body(stream, body).await
}

/// Собирает подсказки к набираемой команде.
///
/// Клиент присылает всё, что набрано, и ждёт, чем дописать последнее слово.
/// Сам он умеет дополнять только названия команд: имена игроков знает
/// только сервер.
fn suggestions(payload: &[u8], shared: &Shared, who: &[u8; 16]) -> Option<Suggestions> {
    let mut cursor = Cursor::new(payload);

    let id = cursor.varint()?;
    let text = cursor.string()?;

    // Дописывается последнее слово: от него и считаем место замены.
    let typed = text.rsplit(' ').next().unwrap_or("");

    // Место и длина считаются в знаках, а не в байтах: иначе на русских
    // именах клиент заменял бы не тот кусок строки.
    let length = text_length(typed);
    let start = text_length(&text) - length;

    Some(Suggestions {
        id,
        start,
        length,
        matches: hints_for(&text, typed, shared, who),
    })
}

/// Что подсказать: имена игроков, названия предметов или что-то ещё —
/// смотря какая команда и какое слово по счёту дописывается.
fn hints_for(text: &str, typed: &str, shared: &Shared, who: &[u8; 16]) -> Vec<String> {
    let line = text.strip_prefix('/').unwrap_or(text);
    let mut words = line.split(' ');

    let command = words.next().unwrap_or("");
    let first = words.clone().next().unwrap_or("");
    // Сколько слов уже написано после самой команды: дописываемое считается.
    let written = words.count();

    let starting = |list: &mut dyn Iterator<Item = &str>| -> Vec<String> {
        list.filter(|word| word.starts_with(typed)).map(|word| word.to_string()).collect()
    };

    // Особый динамит: подсказки — только тем, кому он позволен.
    if command == "tnt" && !crate::permissions::allowed(shared, who, crate::custom_tnt::GIVE_NODE) {
        return Vec::new();
    }

    match (command, written) {
        ("tnt", 1) => starting(&mut ["give", "list"].into_iter()),
        ("tnt", 2) if first == "give" => player_names(shared, typed),
        ("tnt", 3) if first == "give" => starting(&mut crate::custom_tnt::names()),
        ("tnt", 4) if first == "give" => starting(&mut ["1", "16", "64"].into_iter()),
        ("tnt", _) => Vec::new(),

        // give <игрок> <предмет>: на втором слове подсказываем предметы.
        ("give", 2) => item_names(typed),

        // deop <игрок>: подсказывать тех, у кого права есть, а не всех подряд.
        ("deop", 1) => {
            let ops = shared.ops.lock().expect("права захвачены другим потоком");
            let typed = typed.to_lowercase();

            ops.names()
                .into_iter()
                .filter(|name| typed.is_empty() || name.to_lowercase().starts_with(&typed))
                .map(|name| name.to_string())
                .collect()
        }

        ("time", 1) => ["set", "add", "query"]
            .into_iter()
            .filter(|word| word.starts_with(typed))
            .map(|word| word.to_string())
            .collect(),

        ("time", 2) => ["day", "noon", "night", "midnight"]
            .into_iter()
            .filter(|word| word.starts_with(typed))
            .map(|word| word.to_string())
            .collect(),

        // summon <вагонетка> и kill @e[type=<вид>]: из сущностей у нас
        // есть вагонетки, предметы, динамит и падающие блоки.
        ("summon", 1) => crate::minecart::Kind::ALL
            .into_iter()
            .chain(crate::minecart::boat_kinds())
            .map(|kind| kind.name())
            .chain(["lightning_bolt"])
            .map(|name| name.to_string())
            .filter(|name| name.starts_with(typed.strip_prefix("minecraft:").unwrap_or(typed)))
            .collect(),

        ("kill", 1) => ["item", "tnt", "falling_block", "lightning_bolt", "!player"]
            .into_iter()
            .chain(crate::minecart::Kind::ALL.into_iter().map(|kind| kind.name()))
            .map(|kind| format!("@e[type={}]", kind))
            .chain(["@e".to_string()])
            .filter(|selector| selector.starts_with(typed))
            .collect(),

        // setblock <x> <y> <z> <блок> и fill <x1> <y1> <z1> <x2> <y2> <z2> <блок>
        ("setblock", 4) | ("fill", 7) => item_names(typed),

        _ => player_names(shared, typed),
    }
}

/// Названия предметов, начинающиеся с написанного.
///
/// Предметов больше тысячи, поэтому без написанного начала не подсказываем
/// ничего: вывалить весь список бессмысленно.
fn item_names(typed: &str) -> Vec<String> {
    if typed.is_empty() {
        return Vec::new();
    }

    let typed = typed.strip_prefix("minecraft:").unwrap_or(typed).to_lowercase();

    blocks::item_names()
        .filter(|name| name.starts_with(&typed))
        .take(50)
        .map(|name| name.to_string())
        .collect()
}

/// Длина строки так, как её считает клиент.
///
/// Он считает не байты и даже не знаки, а «половинки»: знаки за пределами
/// основного набора занимают две. Для кириллицы это одна, для смайликов две.
fn text_length(text: &str) -> i32 {
    text.chars().map(|letter| letter.len_utf16() as i32).sum()
}

/// Имена игроков и значки выбора, начинающиеся на набранное.
fn player_names(shared: &Shared, typed: &str) -> Vec<String> {
    /// Значки выбора: сам, ближайший, случайный, все, все существа.
    const SELECTORS: [&str; 5] = ["@s", "@p", "@r", "@a", "@e"];

    let names = shared
        .players
        .lock()
        .expect("список игроков захвачен другим потоком")
        .names();

    let typed = typed.to_lowercase();

    names
        .into_iter()
        .chain(SELECTORS.iter().map(|selector| selector.to_string()))
        .filter(|name| typed.is_empty() || name.to_lowercase().starts_with(&typed))
        .collect()
}

/// Отправляет подсказки клиенту.
async fn send_suggestions(stream: &mut Outgoing<'_>, hints: &Suggestions) -> io::Result<()> {
    let mut body = encode_varint(TAB_COMPLETE_RESPONSE);

    body.extend_from_slice(&encode_varint(hints.id));
    body.extend_from_slice(&encode_varint(hints.start));
    body.extend_from_slice(&encode_varint(hints.length));
    body.extend_from_slice(&encode_varint(hints.matches.len() as i32));

    for hint in &hints.matches {
        body.extend_from_slice(&encode_string(hint));
        body.push(0); // пояснения к подсказке нет
    }

    write_body(stream, body).await
}

/// Забирает поручения для этого игрока.
fn take_orders(shared: &Shared, uuid: &[u8; 16]) -> Vec<Order> {
    shared
        .players
        .lock()
        .expect("список игроков захвачен другим потоком")
        .take_orders(uuid)
}

/// Отправляет Disconnect — сервер отключает игрока и называет причину.
async fn send_disconnect(stream: &mut Outgoing<'_>, reason: &str) -> io::Result<()> {
    let mut body = encode_varint(DISCONNECT);

    push_text_component(&mut body, reason);

    write_body(stream, body).await
}

/// Отправляет Keep Alive — без него клиент решает, что сервер пропал,
/// и отключается.
async fn send_keep_alive(stream: &mut Outgoing<'_>, id: i64) -> io::Result<()> {
    let mut body = encode_varint(KEEP_ALIVE);

    push_i64(&mut body, id);

    write_body(stream, body).await
}

/// Отправляет Set Default Spawn Position — точку возрождения.
async fn send_default_spawn_position(
    stream: &mut Outgoing<'_>,
    spawn: (f64, f64, f64),
) -> io::Result<()> {
    let mut body = encode_varint(SET_DEFAULT_SPAWN_POSITION);

    // Точка возрождения — всегда в обычном мире.
    body.extend_from_slice(&encode_string(Dimension::Overworld.name()));
    push_packed_position(&mut body, spawn.0.floor() as i32, spawn.1.floor() as i32, spawn.2.floor() as i32);
    push_f32(&mut body, 0.0); // поворот
    push_f32(&mut body, 0.0); // наклон

    write_body(stream, body).await
}

/// Отправляет Game Event 3 — «смена режима игры»: без него клиент остаётся
/// в прежнем режиме, как бы ни менялась строка в списке игроков.
async fn send_game_event_game_mode(stream: &mut Outgoing<'_>, game_mode: i32) -> io::Result<()> {
    let mut body = encode_varint(GAME_EVENT);

    body.push(3); // событие: смена режима игры
    push_f32(&mut body, game_mode as f32);

    write_body(stream, body).await
}

/// Clientbound Player Abilities.
const PLAYER_ABILITIES: i32 = 0x40;

/// Set Camera — смотреть глазами сущности (`spectate`).
const CAMERA: i32 = 0x5D;

/// Флаги способностей игрока по режиму игры (протокол, Player Abilities:
/// 0x01 — неуязвим, 0x02 — летит, 0x04 — может летать, 0x08 — творческий,
/// ломает мгновенно). Наблюдатель летает всегда; творческий может летать,
/// но после смены режима не летит; выживание и приключение — ничего.
fn ability_flags(game_mode: i32) -> u8 {
    match game_mode {
        GAME_MODE_CREATIVE => 0x01 | 0x04 | 0x08,
        GAME_MODE_SPECTATOR => 0x01 | 0x02 | 0x04,
        _ => 0,
    }
}

/// Отправляет Player Abilities — что игроку можно в его режиме. Без этого
/// клиент, переведённый из творческого в выживание, так и продолжал
/// висеть в воздухе: оригинал шлёт способности при входе и при смене
/// режима (sculk-shrieker: робот после `gamemode survival` должен упасть и
/// встать на крикуна).
async fn send_abilities(stream: &mut Outgoing<'_>, game_mode: i32) -> io::Result<()> {
    let mut body = encode_varint(PLAYER_ABILITIES);

    body.push(ability_flags(game_mode));
    push_f32(&mut body, 0.05); // скорость полёта
    push_f32(&mut body, 0.1); // поле зрения (скорость ходьбы)

    write_body(stream, body).await
}

/// Отправляет Game Event с этим поводом и значением.
async fn send_game_event(stream: &mut Outgoing<'_>, reason: u8, value: f32) -> io::Result<()> {
    let mut body = encode_varint(GAME_EVENT);

    body.push(reason);
    push_f32(&mut body, value);

    write_body(stream, body).await
}

/// Погода игроку, вошедшему в мир (weather.rs, `on_entry`): если идёт дождь
/// — «дождь пошёл», сила дождя и грома.
async fn send_weather(stream: &mut Outgoing<'_>, shared: &Shared, dimension: crate::dimension::Dimension) -> io::Result<()> {
    let events = shared.world_in(dimension).lock().expect("мир захвачен другим потоком").weather.on_entry();

    for (reason, value) in events {
        send_game_event(stream, reason, value).await?;
    }

    Ok(())
}

/// Отправляет Game Event 13 — «начать ждать чанки».
async fn send_game_event_start_waiting(stream: &mut Outgoing<'_>) -> io::Result<()> {
    let mut body = encode_varint(GAME_EVENT);

    body.push(13); // событие: начало ожидания чанков
    push_f32(&mut body, 0.0); // значение для этого события не используется

    write_body(stream, body).await
}

/// Отправляет Set Center Chunk — центральный чанк для клиента.
async fn send_center_chunk(stream: &mut Outgoing<'_>, chunk: (i32, i32)) -> io::Result<()> {
    let mut body = encode_varint(SET_CENTER_CHUNK);

    body.extend_from_slice(&encode_varint(chunk.0)); // чанк X
    body.extend_from_slice(&encode_varint(chunk.1)); // чанк Z

    write_body(stream, body).await
}

/// Такт игрока у портала: стоит в портале достаточно долго — поручение
/// перейти в другое измерение, к парному порталу (найденному или
/// построенному).
fn step_portal(shared: &Shared, player: &mut PlayerState) {
    let inside = crate::portal::standing_in(
        &shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком"),
        (player.x, player.y, player.z),
    );

    if !inside {
        player.in_portal = 0;
        player.portal_ready = true;
        return;
    }

    if !player.portal_ready {
        return;
    }

    // Портал замечается при движении: после переноса — с первым пакетом места.
    if !player.moved_since_tp {
        return;
    }

    player.in_portal += 1;
    let end_portal = crate::portal::standing_in_end_portal(
        &shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком"),
        (player.x, player.y, player.z),
    );
    // В портале Края ждать не надо.
    let needed = if end_portal { 1 } else { crate::portal::wait(&shared.settings, player.game_mode == GAME_MODE_CREATIVE) };

    if player.in_portal < needed {
        return;
    }

    player.in_portal = 0;
    player.portal_ready = false;
    player.portal_trip = true;

    let (mut dimension, (mut x, mut y, mut z)) = crate::portal::travel(shared, player.dimension, (player.x, player.y, player.z));

    // Из Энда — к точке возрождения игрока (кровать), а не к общей.
    if player.dimension == Dimension::End && dimension == Dimension::Overworld {
        dimension = Dimension::Overworld;
        (x, y, z) = on_the_ground(shared, player.spawn_point);
    }

    shared
        .players
        .lock()
        .expect("список игроков захвачен другим потоком")
        .order(&player.uuid, Order::ChangeDimension { dimension, x, y, z });
}

/// Конец такта для этого подключения: всё, что случилось в мире, и ответы
/// на собственные пакеты игрока.
async fn flush_tick(
    stream: &mut Outgoing<'_>,
    shared: &Shared,
    player: &mut PlayerState,
    world_changes: &mut usize,
    pending: &mut Vec<Reaction>,
) -> io::Result<bool> {
    let lagged = send_pending_world_events(stream, shared, player, world_changes).await?;

    for reaction in pending.drain(..) {
        apply_reaction(stream, shared, player, &reaction).await?;
    }

    Ok(lagged)
}

/// Отправляет клиенту события мира, накопившиеся с прошлого раза, в том
/// порядке, в каком они случились.
///
/// Порядок важен: действие поршня обязано уйти раньше изменения «поршень
/// выдвинут» (иначе клиент ход не нарисует — «Sending an extension action to
/// an already-extended piston has no effect»), а воздух на месте сломанного
/// блока — раньше действия, в которое клиент этот воздух заполнит.
///
/// Изменения блоков идут пачкой по секциям, как у оригинала: одно — Block
/// Update, несколько в одной секции — Update Section Blocks. Действие, звук
/// или прямое изменение (повтор блока после щелчка) — граница пачки.
async fn send_pending_world_events(
    stream: &mut Outgoing<'_>,
    shared: &Shared,
    player: &mut PlayerState,
    sent: &mut usize,
) -> io::Result<bool> {
    let (events, count, lagged) = shared
        .world_in(player.dimension)
        .lock()
        .expect("мир захвачен другим потоком")
        .published_events_until(*sent, player.reader, player.publication.as_ref().and_then(|p| p.events));

    *sent = count;

    // Изменения блоков копятся до метки рассылки — и между вызовами тоже:
    // сделанное после метки уйдёт со следующей.
    let mut pending = std::mem::take(&mut player.pending_changes);
    let mut entities = std::mem::take(&mut player.pending_entities);

    for event in &events {
        match event {
            WorldEvent::Change(change) if change.direct => {
                send_block_update(stream, change.x, change.y, change.z, change.state).await?;
            }
            WorldEvent::Change(change) => pending.push(*change),
            WorldEvent::Broadcast => {
                small::shoulders(stream, shared, player).await?;
                send_block_changes(stream, &mut pending).await?;

                // Данные блок-сущностей — после пачки, по одному разу на
                // место: последние, в порядке первого появления места.
                let mut order: Vec<PendingEntity> = Vec::new();

                for (x, y, z, kind, nbt) in entities.drain(..) {
                    match order.iter_mut().find(|there| (there.0, there.1, there.2) == (x, y, z)) {
                        Some(there) => *there = (x, y, z, kind, nbt),
                        None => order.push((x, y, z, kind, nbt)),
                    }
                }

                for (x, y, z, kind, nbt) in order {
                    send_block_entity(stream, (x, y, z), kind, &nbt).await?;
                }
            }
            // Ход поршня для Bedrock: у Java клиент рисует его по Block Action.
            WorldEvent::Piston(_) => {}
            WorldEvent::BlockEntity { x, y, z, kind, nbt } => entities.push((*x, *y, *z, *kind, nbt.clone())),
            WorldEvent::Action(action) => {
                let mut body = encode_varint(BLOCK_ACTION);

                push_packed_position(&mut body, action.x, action.y, action.z);
                body.push(action.action);
                body.push(action.param);
                body.extend_from_slice(&encode_varint(action.block));

                write_body(stream, body).await?;
            }
            WorldEvent::Sound(sound) if sound.except == Some(player.entity_id) => {}
            WorldEvent::Sound(sound) => send_sound(stream, sound).await?,
            WorldEvent::EntitySound { entity_id, name, volume, pitch } => {
                let mut body = encode_varint(ENTITY_SOUND);

                body.extend_from_slice(&encode_varint(0));
                body.extend_from_slice(&encode_string(name));
                body.push(0);
                body.extend_from_slice(&encode_varint(SOUND_PLAYERS));
                body.extend_from_slice(&encode_varint(*entity_id));
                push_f32(&mut body, *volume);
                push_f32(&mut body, *pitch);
                body.extend_from_slice(&0i64.to_be_bytes());
                write_body(stream, body).await?;
            }
            WorldEvent::Level { except: Some(id), .. } if *id == player.entity_id => {}
            WorldEvent::Level { x, y, z, effect, data, .. } => {
                let mut body = encode_varint(WORLD_EVENT);

                push_i32(&mut body, *effect);
                push_packed_position(&mut body, *x, *y, *z);
                push_i32(&mut body, *data);
                // Открытие портала Края, призыв иссушителя и смерть дракона слышно
                // на весь мир (глобальное событие).
                body.push(u8::from(matches!(*effect, 1023 | 1028 | 1038)));

                write_body(stream, body).await?;
            }
            WorldEvent::VibrationAt { from, to, ticks } => {
                let mut body = encode_varint(LEVEL_PARTICLES);

                body.push(0); // не издалека
                body.push(0); // не всегда
                for v in [from.0, from.1, from.2] {
                    body.extend_from_slice(&v.to_be_bytes());
                }
                for _ in 0..4 {
                    body.extend_from_slice(&0f32.to_be_bytes()); // разброс и скорость
                }
                push_i32(&mut body, 1);
                body.extend_from_slice(&encode_varint(PARTICLE_VIBRATION));
                body.extend_from_slice(&encode_varint(0)); // цель — блок
                push_packed_position(&mut body, to.0, to.1, to.2);
                body.extend_from_slice(&encode_varint(*ticks));

                write_body(stream, body).await?;
            }
            WorldEvent::Trail { from, to, color, ticks } => {
                let mut body=encode_varint(LEVEL_PARTICLES);
                body.extend_from_slice(&[0,0]);
                for v in [from.0,from.1,from.2] {body.extend_from_slice(&v.to_be_bytes());}
                for _ in 0..4 {body.extend_from_slice(&0f32.to_be_bytes());}
                push_i32(&mut body,1);
                body.extend_from_slice(&encode_varint(49));
                for v in [to.0,to.1,to.2] {body.extend_from_slice(&v.to_be_bytes());}
                push_i32(&mut body,*color);body.extend_from_slice(&encode_varint(*ticks));
                write_body(stream,body).await?;
            }
            WorldEvent::Vibration { from, to, ticks } => {
                let mut body = encode_varint(LEVEL_PARTICLES);

                body.push(0); // не издалека
                body.push(0); // не всегда
                for v in [from.0, from.1, from.2] {
                    body.extend_from_slice(&(f64::from(v) + 0.5).to_be_bytes());
                }
                for _ in 0..4 {
                    body.extend_from_slice(&0f32.to_be_bytes()); // разброс и скорость
                }
                push_i32(&mut body, 1);
                body.extend_from_slice(&encode_varint(PARTICLE_VIBRATION));
                body.extend_from_slice(&encode_varint(0)); // цель — блок
                push_packed_position(&mut body, to.0, to.1, to.2);
                body.extend_from_slice(&encode_varint(*ticks));

                write_body(stream, body).await?;
            }
            // Нота проигрывателя: число частиц 0 — тогда разброс по X
            // несёт цвет, а скорость — множитель (чёрный ящик, jukebox-disc:
            // разброс 0, 1/24, 2/24, 3/24). [проверить] скорость 1.
            WorldEvent::Note { at, color } => {
                let mut body = encode_varint(LEVEL_PARTICLES);

                body.push(0); // не издалека
                body.push(0); // не всегда
                for v in [at.0, at.1, at.2] {
                    body.extend_from_slice(&v.to_be_bytes());
                }
                for v in [*color, 0.0, 0.0, 1.0] {
                    body.extend_from_slice(&v.to_be_bytes());
                }
                push_i32(&mut body, 0);
                body.extend_from_slice(&encode_varint(PARTICLE_NOTE));

                write_body(stream, body).await?;
            }
            WorldEvent::OpenWindow { viewer, x, y, z } if *viewer == player.entity_id => {
                open_window(stream, shared, player, (*x, *y, *z)).await?;
            }
            WorldEvent::OpenWindow { .. } => {}
            WorldEvent::OpenSign { viewer, x, y, z, front } if *viewer == player.entity_id => {
                signs::send_editor(stream, (*x, *y, *z), *front).await?;
            }
            WorldEvent::OpenSign { .. } => {}
            WorldEvent::Contents => send_window_changes(stream, shared, player).await?,
            WorldEvent::Tracker => {
                if std::mem::take(&mut player.swing_pending) {
                    send_swing(stream, player.entity_id).await?;
                }

                mobs::send_mob_ticks(stream, shared, player).await?;
            }
            WorldEvent::MobSpawn(mob) => mobs::send_released_mob(stream, shared, player, mob).await?,
            WorldEvent::ArrowTracker => bow::send_arrow_ticks(stream, shared, player).await?,
            WorldEvent::ThrownTracker { early } => {
                thrown::send_thrown_ticks(stream, shared, player, *early).await?;
                if !early { fishing::send_ticks(stream, shared, player).await?; }
            }
            WorldEvent::RocketBlast { entity_id } => thrown::send_rocket_blast(stream, player, *entity_id).await?,
            WorldEvent::EyeLaunch { entity_id } => thrown::send_eye_launch(stream, shared, player, *entity_id).await?,
            // Эффект от зелья или облака: пакет эффекта сразу, частицы и
            // атрибуты — на такт позже, частицы даже без перемен (чёрный ящик,
            // splash-potion-feet: Effect на +2, Metadata 10 и Attributes на +3;
            // lingering-potion-feet: повторное наложение тоже шлёт Metadata 10).
            WorldEvent::PotionEffect { entity_id, effect } if *entity_id == player.entity_id => {
                vitals::give_effect(stream, player, *effect).await?;
                player.net.hold_meta = true;
                player.net.renew_effect_particles();

                if let Some((attribute, ..)) = effect.kind.modifier() {
                    player.attrs_touched.push((attribute, *shared.ticks.borrow() + 1));
                }
            }
            WorldEvent::PotionEffect { .. } => {}
            // Лечение: здоровье в свойствах — на такт позже (splash-potion-heal).
            WorldEvent::PotionHeal { entity_id, amount } if *entity_id == player.entity_id => {
                player.vit.heal(*amount);
                player.net.hold_meta = true;
            }
            WorldEvent::PotionHeal { .. } => {}
            WorldEvent::Hurt { entity_id, amount, source, killer } if *entity_id == player.entity_id => {
                let kind = if killer.is_empty() { crate::survival::Source::Explosion } else { crate::survival::Source::PlayerExplosion };

                player.net.killer = killer;

                let done = vitals::hit(stream, shared, player, *amount, kind, *source).await?;

                // Свойства и скорость после взрыва — на такт позже урона
                // (чёрный ящик: и у крипера, и у динамита).
                player.net.hold_meta = done.is_some();
            }
            WorldEvent::Hurt { .. } => {}
            WorldEvent::MobHurt { entity_id, amount, source, attacker, killer, push, at } if *entity_id == player.entity_id => {
                vitals::mob_hit(stream, shared, player, *amount, *source, *attacker, killer, *push, *at).await?;
            }
            WorldEvent::MobHurt { .. } => {}
            WorldEvent::MobEffect { entity_id, effect } if *entity_id == player.entity_id => {
                if (effect.kind != crate::survival::EffectKind::DolphinsGrace || (player.vit.sprinting && player.net.in_water)) && (effect.kind != crate::survival::EffectKind::Darkness || !player.vit.dead) {
                    vitals::give_effect(stream, player, *effect).await?;
                }
            }
            WorldEvent::MobEffect { .. } => {}
            WorldEvent::AxolotlAssist { entity_id } if *entity_id == player.entity_id => {
                let effect=crate::mob::water::assist_effect(&player.vit);
                vitals::clear_effects(stream,player,Some(crate::survival::EffectKind::MiningFatigue)).await?;
                vitals::give_effect(stream,player,effect).await?;
            }
            WorldEvent::AxolotlAssist { .. } => {}
            WorldEvent::MobHitEffect { entity_id, effect } if *entity_id == player.entity_id => {
                if std::mem::take(&mut player.net.mob_hit_landed) {
                    vitals::give_effect(stream, player, *effect).await?;

                    // Атрибуты уходят тактом позже и при повторном наложении
                    // того же срока (чёрный ящик, stray-slowness: Effect на +27,
                    // Attributes на +28; повтор — Effect +89, Attributes +90).
                    if let Some((attribute, ..)) = effect.kind.modifier() {
                        player.attrs_touched.push((attribute, *shared.ticks.borrow() + 1));
                    }
                }
            }
            WorldEvent::MobHitEffect { .. } => {}
            WorldEvent::PlayerStrike(strike) => pvp::take_strike(stream, shared, player, strike).await?,
            WorldEvent::Particle { kind, block, at, offset, speed, count, skip } if *skip != player.entity_id => {
                let near = (at.0 - player.x).powi(2) + (at.1 - player.y).powi(2) + (at.2 - player.z).powi(2) <= 32.0 * 32.0;

                if near {
                    mobs::send_particles(stream, *kind, *block, *at, *offset, *speed, *count).await?;
                }
            }
            WorldEvent::Particle { .. } => {}
            WorldEvent::PlayerDamaged { entity_id, type_id, culprit, direct, at } if *entity_id != player.entity_id => {
                let here = shared.players.lock().expect("список игроков захвачен другим потоком").by_entity(*entity_id).is_some_and(|(member, _)| member.dimension == player.dimension);

                if here {
                    let mut body = encode_varint(0x19);

                    body.extend_from_slice(&encode_varint(*entity_id));
                    body.extend_from_slice(&encode_varint(*type_id));
                    body.extend_from_slice(&encode_varint(*culprit));
                    body.extend_from_slice(&encode_varint(*direct));
                    body.push(u8::from(at.is_some()));
                    if let Some((x, y, z)) = at { push_f64(&mut body, *x); push_f64(&mut body, *y); push_f64(&mut body, *z); }
                    write_body(stream, body).await?;
                }
            }
            WorldEvent::PlayerDamaged { .. } => {}
            WorldEvent::GameEvent { entity_id, reason, value } if *entity_id == player.entity_id => {
                let mut body = encode_varint(GAME_EVENT);

                body.push(*reason);
                body.extend_from_slice(&value.to_be_bytes());
                write_body(stream, body).await?;
            }
            WorldEvent::GameEvent { .. } => {}
            // Древний страж: только выживание и приключение, и не тем, у кого
            // «Утомление» III ещё на минуту и дольше (вики, Elder Guardian);
            // эффект, потом событие 10 (чёрный ящик, elder-fatigue).
            WorldEvent::ElderCurse { entity_id, silent } if *entity_id == player.entity_id => {
                let fatigued = player.vit.effects.iter().any(|effect| {
                    effect.kind == crate::survival::EffectKind::MiningFatigue && effect.amplifier >= crate::mob::guardian::CURSE_AMPLIFIER && effect.ticks_left >= crate::mob::guardian::CURSE_EVERY as i32
                });

                if matches!(player.game_mode, GAME_MODE_SURVIVAL | GAME_MODE_ADVENTURE) && !fatigued && !player.vit.dead {
                    let curse = crate::survival::Effect {
                        kind: crate::survival::EffectKind::MiningFatigue,
                        amplifier: crate::mob::guardian::CURSE_AMPLIFIER,
                        ticks_left: crate::mob::guardian::CURSE_TICKS,
                        ambient: false,
                        visible: true, show_icon: true,
                    };

                    vitals::give_effect(stream, player, curse).await?;
                    // Скорость удара уходит на такт позже эффекта (elder-fatigue).
                    player.attrs_touched.push((crate::attributes::ATTACK_SPEED, *shared.ticks.borrow() + 1));
                    send_game_event(stream, GAME_EVENT_ELDER_CURSE, if *silent { 0.0 } else { 1.0 }).await?;
                }
            }
            WorldEvent::ElderCurse { .. } => {}
            WorldEvent::Weather { reason, value } => send_game_event(stream, *reason, *value).await?,
            WorldEvent::Struck { entity_id } if *entity_id == player.entity_id => vitals::struck(stream, shared, player).await?,
            WorldEvent::Struck { .. } => {}
            WorldEvent::ProgressNear { id, at, radius } => {
                if (player.x - at.0).abs() <= radius.0 && (player.y - at.1).abs() <= radius.1 && (player.z - at.2).abs() <= radius.2 {
                    player.progress.trigger(id);
                }
            }
            WorldEvent::BossBar(share) => send_boss_bar(stream, player, *share).await?,
            WorldEvent::WitherBar { entity_id, share } => {
                if player.seen_mobs.contains_key(entity_id) {
                    mobs::send_wither_share(stream, *entity_id, *share).await?;
                }
            }
            WorldEvent::Private { viewer, body } if *viewer == player.entity_id => write_body(stream, body.to_vec()).await?,
            WorldEvent::Private { .. } => {}
            WorldEvent::Collect { entity_id, by, count } => {
                if player.seen_bow_arrows.remove(entity_id).is_some() {
                    send_pick_up(stream, *entity_id, *by, *count).await?;
                    send_remove_entity(stream, *entity_id).await?;
                }
            }
            WorldEvent::EquipmentBreak { entity_id, status } => {
                let mut body = encode_varint(0x22);
                push_i32(&mut body, *entity_id);
                body.push(*status);
                write_body(stream, body).await?;
            }
            WorldEvent::Animation { entity_id, kind } => {
                let mut body = encode_varint(ENTITY_ANIMATION);

                body.extend_from_slice(&encode_varint(*entity_id));
                body.push(*kind);
                write_body(stream, body).await?;
            }
            WorldEvent::Explosion(blast) => {
                let (dx, dy, dz) = (player.x - blast.centre.0, player.y - blast.centre.1, player.z - blast.centre.2);

                if dx * dx + dy * dy + dz * dz < EXPLOSION_HEARD * EXPLOSION_HEARD {
                    let own = blast.pushed.iter().find(|(id, _)| *id == player.entity_id).map(|(_, push)| vitals::protected_blast_push(player, *push));

                    // Нападающему со «порывом ветра» скорость (0; 0,01; 0) от
                    // сокрушающего удара уходит сразу, до взрыва: иначе она
                    // затирает подброс (чёрный ящик, pvp-mace-wind-burst: у
                    // оригинала «Velocity» перед «Explosion»).
                    if own.is_some() && blast.wind && let Some(velocity) = player.net.velocity.take() {
                        send_velocity(stream, player.entity_id, velocity).await?;
                    }

                    send_explosion(stream, blast, own).await?;

                    // Взрыв ветра не ранит — и скорости следом не шлёт (чёрный
                    // ящик, pvp-mace-wind-burst: только толчок в пакете).
                    if let Some(push) = own.filter(|_| !blast.wind) {
                        vitals::blasted(player, push);
                    }
                }
            }
        }
    }

    player.pending_changes = pending;
    player.pending_entities = entities;

    Ok(lagged)
}

/// Данные блок-сущности, ждущие рассылки: место, вид, сетевой NBT.
type PendingEntity = (i32, i32, i32, i32, Arc<[u8]>);

/// Block Entity Data: место, вид и сетевой NBT.
async fn send_block_entity(stream: &mut Outgoing<'_>, (x, y, z): (i32, i32, i32), kind: i32, nbt: &[u8]) -> io::Result<()> {
    let mut body = encode_varint(BLOCK_ENTITY_DATA);

    push_packed_position(&mut body, x, y, z);
    body.extend_from_slice(&encode_varint(kind));
    body.extend_from_slice(nbt);

    write_body(stream, body).await
}

/// Открывает клиенту окно хранилища в этом месте: Open Screen и всё
/// содержимое окна — ячейки хранилища, рюкзак, панель и курсор.
async fn open_window(
    stream: &mut Outgoing<'_>,
    shared: &Shared,
    player: &mut PlayerState,
    at: (i32, i32, i32),
) -> io::Result<()> {
    let (halves, slots, kind) = {
        let world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");
        let kind = containers::Kind::of(world.get_block(at.0, at.1, at.2));
        // У эндер-сундука ячейки личные: в окне — хранилище игрока.
        let (halves, slots) = if kind == Some(containers::Kind::EnderChest) {
            (vec![at], player.ender.clone())
        } else if kind == Some(containers::Kind::Beacon) {
            (vec![at], vec![None])
        } else {
            let halves = containers::halves(&world, at);
            let slots = containers::window_slots(&world, &halves);

            (halves, slots)
        };

        (halves, slots, kind)
    };

    let Some(kind) = kind else {
        // Станок без хранилища: стол зачарования, наковальня, точило.
        let station = station::station_at(&shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком"), at);

        if let Some(station) = station {
            station::open_station(stream, shared, player, at, station).await?;
        }

        return Ok(());
    };

    // Статистика открытий (вики, Statistics).
    match kind {
        containers::Kind::Chest | containers::Kind::CopperChest => player.progress.custom("open_chest", 1),
        containers::Kind::TrappedChest => {
            player.progress.custom("open_chest", 1);
            player.progress.custom("trigger_trapped_chest", 1);
        }
        containers::Kind::Barrel => player.progress.custom("open_barrel", 1),
        containers::Kind::Hopper => player.progress.custom("inspect_hopper", 1),
        containers::Kind::Dropper => player.progress.custom("inspect_dropper", 1),
        containers::Kind::Dispenser => player.progress.custom("inspect_dispenser", 1),
        containers::Kind::Lectern => player.progress.custom("interact_with_lectern", 1),
        containers::Kind::EnderChest => player.progress.custom("open_enderchest", 1),
        containers::Kind::ShulkerBox => player.progress.custom("open_shulker_box", 1),
        containers::Kind::Furnace => player.progress.custom("interact_with_furnace", 1),
        containers::Kind::BlastFurnace => player.progress.custom("interact_with_blast_furnace", 1),
        containers::Kind::Smoker => player.progress.custom("interact_with_smoker", 1),
        containers::Kind::BrewingStand => player.progress.custom("interact_with_brewingstand", 1),
        containers::Kind::Beacon => player.progress.custom("interact_with_beacon", 1),
        _ => {}
    }

    player.window_counter = player.window_counter % WINDOW_IDS + 1;

    let id = player.window_counter;
    let (menu, title) = containers::menu(kind, halves.len());
    let mut body = encode_varint(OPEN_SCREEN);

    body.extend_from_slice(&encode_varint(id));
    body.extend_from_slice(&encode_varint(menu));
    body.extend_from_slice(&nbt::translated(title));
    write_body(stream, body).await?;

    let lectern = kind == containers::Kind::Lectern;
    let page = if lectern {
        let world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");

        redstone::lectern::page(&world, at).map_or(0, |(page, _)| page)
    } else {
        0
    };

    let crafter = kind == containers::Kind::Crafter;

    // Печь и стойка: свойства окна (горение, плавка, варка) — все сразу
    // после содержимого (чёрный ящик, furnace-smelt: 0, 1, 2, 3).
    let props = {
        let world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");

        world.container(at.0, at.1, at.2).map(crate::furnace::properties).unwrap_or_default()
    };

    player.window = Some(OpenWindow {
        id,
        halves,
        cart: None,
        shown: slots,
        lectern,
        crafter,
        kind: Some(kind),
        props: props.clone(),
        merchant: None,
        station: None,
        ender: kind == containers::Kind::EnderChest,
    });
    send_window_content(stream, player).await?;

    for (property, value) in props.into_iter().enumerate() {
        send_window_property(stream, id, property as i16, value).await?;
    }

    if lectern {
        send_window_property(stream, id, 0, page as i16).await?;
    }

    // Крафтер: выключенные ячейки и «запитан» — свойствами окна.
    if kind == containers::Kind::Crafter {
        let properties = {
            let world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");

            redstone::crafter::window_properties(&world, at)
        };

        for (property, value) in properties {
            send_window_property(stream, id, property, value).await?;
        }
    }

    Ok(())
}

/// Открывает клиенту окно вагонетки с сундуком или воронкой: как у
/// хранилищ-блоков, только ячейки — самой вагонетки (вики, Minecart with
/// Chest: «27 slots», как у сундука; Minecart with Hopper: 5 ячеек, окно
/// воронки). Заголовок — имя сущности. [проверить]: заголовок.
async fn open_cart_window(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState, entity_id: i32) -> io::Result<()> {
    let found = {
        let items=shared.items_in(player.dimension).lock().expect("предметы");
        if let Some(mob)=items.mobs().iter().find(|m| m.entity_id==entity_id && crate::mob::land::llama(m.kind) && m.land.tamed && !m.is_dead()) {
            Some((0,"entity.minecraft.llama",land::slots(mob),true,mob.land.strength))
        } else {
            items.carts().iter().find(|c| c.entity_id==entity_id).and_then(|c| cart_menu(c.kind).map(|(menu,title)| (menu,title,c.slots.clone(),false,0)))
        }
    };
    let Some((menu,title,slots,llama,strength))=found else { return horse::open(stream, shared, player, entity_id).await; };

    player.window_counter = player.window_counter % WINDOW_IDS + 1;

    let id = player.window_counter;
    let mut body = encode_varint(if llama { 0x29 } else { OPEN_SCREEN });
    body.extend_from_slice(&encode_varint(id));
    if llama { body.extend_from_slice(&encode_varint(if slots.len()>2 { i32::from(strength) } else { 0 })); body.extend_from_slice(&entity_id.to_be_bytes()); }
    else { body.extend_from_slice(&encode_varint(menu)); body.extend_from_slice(&nbt::translated(title)); }
    write_body(stream, body).await?;

    player.window = Some(OpenWindow { id, halves: Vec::new(), cart: Some(entity_id), shown: slots, lectern: false, crafter: false, kind: None, props: Vec::new(), merchant: None, station: None, ender: false });
    send_window_content(stream, player).await
}

/// Окно вагонетки-хранилища: вид меню и заголовок. Вид — тот же, что у
/// сундука в один блок и у воронки (`containers::menu`).
fn cart_menu(kind: crate::minecart::Kind) -> Option<(i32, &'static str)> {
    match kind {
        crate::minecart::Kind::Chest => Some((containers::menu(containers::Kind::Chest, 1).0, "entity.minecraft.chest_minecart")),
        crate::minecart::Kind::Hopper => Some((containers::menu(containers::Kind::Hopper, 1).0, "entity.minecraft.hopper_minecart")),
        crate::minecart::Kind::Boat(index) if crate::minecart::BOATS[index as usize].2 => Some((containers::menu(containers::Kind::Chest, 1).0, "entity.minecraft.chest_boat")),
        _ => None,
    }
}

/// Ячейки вагонетки с этим номером, если она ещё в мире.
pub(super) fn cart_slots(shared: &Shared, dimension: crate::dimension::Dimension, entity_id: i32) -> Option<Vec<Option<Stack>>> {
    let items=shared.items_in(dimension).lock().expect("предметы");
    items.carts().iter().find(|c| c.entity_id==entity_id).map(|c| c.slots.clone())
        .or_else(|| items.mobs().iter().find(|m| m.entity_id==entity_id && crate::mob::land::llama(m.kind) && !m.is_dead()).map(land::slots))
        .or_else(|| items.mobs().iter().find(|mob| mob.entity_id == entity_id && crate::mob::horse::equine(mob.kind) && !mob.is_dead()).map(|mob| mob.horse.inventory()))
}

/// Всё окно хранилища целиком: ячейки хранилища (как их знает клиент),
/// рюкзак и панель игрока, курсор.
async fn send_window_content(stream: &mut Outgoing<'_>, player: &mut PlayerState) -> io::Result<()> {
    let Some(window) = &player.window else {
        return Ok(());
    };

    player.state_id += 1;

    let mut body = encode_varint(SET_CONTAINER_CONTENT);

    body.extend_from_slice(&encode_varint(window.id));
    body.extend_from_slice(&encode_varint(player.state_id));
    // У кафедры в окне только книга — без рюкзака и панели игрока.
    let player_slots = if window.lectern { 0 } else { window::PLAYER_SLOTS };

    let result_slots = usize::from(window.crafter);

    body.extend_from_slice(&encode_varint((window.shown.len() + player_slots + result_slots) as i32));

    for stack in &window.shown {
        push_slot(&mut body, *stack);
    }

    if window.lectern {
        push_slot(&mut body, player.inventory.cursor());
        return write_body(stream, body).await;
    }

    for slot in (inventory::FIRST_MAIN..=inventory::LAST_MAIN).chain(inventory::FIRST_HOTBAR..=inventory::LAST_HOTBAR) {
        push_slot(&mut body, player.inventory.slot(slot));
    }

    if window.crafter {
        push_slot(&mut body, crate::crafting::craft(&window.shown));
    }

    push_slot(&mut body, player.inventory.cursor());
    write_body(stream, body).await
}

/// Одна ячейка открытого окна.
async fn send_window_slot(stream: &mut Outgoing<'_>, player: &mut PlayerState, slot: usize, stack: Option<Stack>) -> io::Result<()> {
    let Some(window) = &player.window else {
        return Ok(());
    };

    player.state_id += 1;

    let mut body = encode_varint(SET_CONTAINER_SLOT);

    body.extend_from_slice(&encode_varint(window.id));
    body.extend_from_slice(&encode_varint(player.state_id));
    push_i16(&mut body, slot as i16);
    push_slot(&mut body, stack);

    write_body(stream, body).await
}

/// Досылает открытому окну перемены содержимого, опубликованные к этому
/// мгновению такта. Хранилища больше нет (сломали) — окно закрывается.
async fn send_window_changes(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState) -> io::Result<()> {
    let Some(window) = &player.window else {
        return Ok(());
    };

    // Окно станка живёт, пока стоит станок.
    if window.station.is_some() {
        if !station::still_there(shared, player, window) {
            let mut body = encode_varint(CLOSE_CONTAINER_CLIENTBOUND);

            body.extend_from_slice(&encode_varint(window.id));
            write_body(stream, body).await?;
            close_own_window(shared, player);
        }

        return Ok(());
    }

    // Окно эндер-сундука: ячейки личные, со стороны мира они не меняются;
    // сломали блок — окно закрывается.
    if window.ender {
        let there = window.halves.first().is_some_and(|&(x, y, z)| {
            containers::Kind::of(shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком").get_block(x, y, z))
                == Some(containers::Kind::EnderChest)
        });

        if !there {
            let mut body = encode_varint(CLOSE_CONTAINER_CLIENTBOUND);

            body.extend_from_slice(&encode_varint(window.id));
            write_body(stream, body).await?;
            close_own_window(shared, player);
        }

        return Ok(());
    }

    // Окно торговли: ячейки живут у игрока; пропал житель — окно закрыто.
    if let Some(trade) = &window.merchant {
        if merchant::villager_gone(shared, player, trade.villager) {
            let mut body = encode_varint(CLOSE_CONTAINER_CLIENTBOUND);

            body.extend_from_slice(&encode_varint(window.id));
            write_body(stream, body).await?;
            close_own_window(shared, player);
        }

        return Ok(());
    }

    let now: Option<Vec<Option<Stack>>> = if window.kind == Some(containers::Kind::Beacon) {
        let world = shared.world_in(player.dimension).lock().expect("мир");
        window.halves.first().filter(|p| world.container(p.0, p.1, p.2).is_some_and(|c| c.kind == containers::Kind::Beacon)).map(|_| window.shown.clone())
    } else if let Some(cart) = window.cart {
        let equine = shared.items_in(player.dimension).lock().expect("предметы").mobs().iter().any(|m| m.entity_id == cart && crate::mob::horse::equine(m.kind));
        if equine { horse::slots(shared, player, cart) } else { shared.world_in(player.dimension).lock().expect("мир").published_carts.get(&cart).cloned() }
    } else {
        let world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");

        window
            .halves
            .iter()
            .map(|place| world.published.get(place).cloned())
            .collect::<Option<Vec<_>>>()
            .map(|parts| parts.concat())
    };

    let Some(now) = now.filter(|now| now.len() == window.shown.len()) else {
        // Хранилище пропало: закрыть окно у клиента.
        let mut body = encode_varint(CLOSE_CONTAINER_CLIENTBOUND);

        body.extend_from_slice(&encode_varint(window.id));
        write_body(stream, body).await?;
        close_own_window(shared, player);
        return Ok(());
    };

    let changed: Vec<usize> = (0..now.len()).filter(|&slot| now[slot] != window.shown[slot]).collect();
    let result = window.crafter.then(|| (window.result_slot(), crate::crafting::craft(&window.shown), crate::crafting::craft(&now)));

    // Свойства печи и стойки, снятые вместе с содержимым: уходят после
    // ячеек, только сменившиеся (чёрный ящик, furnace-smelt).
    let props: Vec<i16> = if window.props.is_empty() {
        Vec::new()
    } else {
        let world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");

        window.halves.first().and_then(|place| world.published_props.get(place).cloned()).unwrap_or_default()
    };

    for slot in changed {
        let stack = now[slot];

        if let Some(window) = player.window.as_mut() {
            window.shown[slot] = stack;
        }

        send_window_slot(stream, player, slot, stack).await?;
    }

    // У крафтера сменилось, что выйдет.
    if let Some((slot, before, after)) = result
        && before != after
    {
        send_window_slot(stream, player, slot, after).await?;
    }

    if let Some(window) = player.window.as_mut()
        && props.len() == window.props.len()
    {
        let id = window.id;
        let changed: Vec<(usize, i16)> = props.iter().enumerate().filter(|&(index, value)| window.props[index] != *value).map(|(index, value)| (index, *value)).collect();

        window.props = props;

        for (property, value) in changed {
            send_window_property(stream, id, property as i16, value).await?;
        }
    }

    Ok(())
}

/// Игрок закрыл окно хранилища (или оно закрылось само): хранилище
/// теряет смотрящего, курсор возвращается в инвентарь.
fn close_own_window(shared: &Shared, player: &mut PlayerState) {
    let Some(window) = player.window.take() else {
        return;
    };

    // Станок: предметы из его ячеек возвращаются игроку.
    if window.station.is_some() {
        if station::give_back(shared, player, &window) {
            player.inventory_after_window = true;
        }

        return;
    }

    if window.kind == Some(containers::Kind::Beacon) {
        for stack in window.shown.iter().flatten() {
            if let Some(rest) = player.inventory.put(*stack).1 { throw_out(shared, player, rest); }
        }
        player.inventory_after_window = true;
    }

    // Окно торговли: житель свободен; оплата, если осталась, — в инвентарь.
    if let Some(trade) = window.merchant {
        for stack in trade.slots[..2].iter().flatten() {
            player.inventory.put(*stack);
        }

        merchant::release(shared, player.dimension, trade.villager);
        return;
    }

    // У вагонетки смотрящих не считают: крышки нет.
    if window.cart.is_some() {
        return;
    }

    let mut world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");

    // Смотрящим он был, только пока хранилище стоит на месте.
    if window.halves.iter().all(|&(x, y, z)| world.container(x, y, z).is_some()) {
        let actor = world.actor.replace(player.entity_id);
        containers::close(&mut world, &window.halves);
        world.actor = actor;
        redstone::settle(&mut world);
    } else {
        for &(x, y, z) in &window.halves {
            world.add_viewers(x, y, z, -1);
        }
    }
}

/// Отправляет накопленные изменения блоков: по секциям, в порядке первого
/// появления секции; повторное изменение того же места в пачке — последнее
/// слово за последним.
async fn send_block_changes(
    stream: &mut Outgoing<'_>,
    pending: &mut Vec<world::BlockChange>,
) -> io::Result<()> {
    if pending.is_empty() {
        return Ok(());
    }

    let mut sections: Vec<((i32, i32, i32), Vec<world::BlockChange>)> = Vec::new();

    for change in pending.drain(..) {
        let key = (change.x >> 4, change.y >> 4, change.z >> 4);

        let group = match sections.iter_mut().find(|(section, _)| *section == key) {
            Some((_, group)) => group,
            None => {
                sections.push((key, Vec::new()));
                &mut sections.last_mut().expect("только что добавили").1
            }
        };

        match group
            .iter_mut()
            .find(|other| (other.x, other.y, other.z) == (change.x, change.y, change.z))
        {
            Some(other) => other.state = change.state,
            None => group.push(change),
        }
    }

    // У голов и табличек есть блок-сущность, и оригинал после пачки шлёт её
    // данные на каждую сменившуюся: у головы — пустой тег (сценарий
    // note-heads), у таблички — пустой текст (note-instruments).
    let mut heads = Vec::new();

    for ((sx, sy, sz), group) in sections {
        heads.extend(
            group
                .iter()
                .filter(|change| redstone::is_head(change.state) || redstone::is_sign(change.state))
                .map(|change| (change.x, change.y, change.z, change.state)),
        );

        if let [only] = group.as_slice() {
            send_block_update(stream, only.x, only.y, only.z, only.state).await?;
            continue;
        }

        let mut body = encode_varint(UPDATE_SECTION_BLOCKS);

        let section = ((sx as i64 & 0x3F_FFFF) << 42)
            | ((sz as i64 & 0x3F_FFFF) << 20)
            | (sy as i64 & 0xF_FFFF);
        body.extend_from_slice(&section.to_be_bytes());
        body.extend_from_slice(&encode_varint(group.len() as i32));

        for change in group {
            let local = ((change.x & 15) << 8) | ((change.z & 15) << 4) | (change.y & 15);
            let record = ((change.state as i64) << 12) | local as i64;
            body.extend_from_slice(&encode_varlong(record));
        }

        write_body(stream, body).await?;
    }

    for (x, y, z, state) in heads {
        let mut body = encode_varint(BLOCK_ENTITY_DATA);

        push_packed_position(&mut body, x, y, z);

        match redstone::block_entity_type(state).filter(|_| redstone::is_sign(state)) {
            Some(kind) => {
                body.extend_from_slice(&encode_varint(kind));
                body.extend_from_slice(&empty_sign_nbt());
            }
            None => {
                body.extend_from_slice(&encode_varint(SKULL_BLOCK_ENTITY));
                body.extend_from_slice(&[0x0A, 0x00]); // пустой безымянный тег-составной
            }
        }

        write_body(stream, body).await?;
    }

    Ok(())
}

/// Данные чистой таблички, как их шлёт оригинал (сценарий note-instruments):
/// с каждой стороны — без свечения, чёрный цвет, четыре пустые строки;
/// не вощёная. Безымянный корень — сетевой NBT.
fn empty_sign_nbt() -> Vec<u8> {
    fn name(out: &mut Vec<u8>, kind: u8, name: &str) {
        out.push(kind);
        out.extend_from_slice(&(name.len() as u16).to_be_bytes());
        out.extend_from_slice(name.as_bytes());
    }

    fn side(out: &mut Vec<u8>, side: &str) {
        name(out, 0x0A, side);
        name(out, 0x01, "has_glowing_text");
        out.push(0);
        name(out, 0x08, "color");
        out.extend_from_slice(&5u16.to_be_bytes());
        out.extend_from_slice(b"black");
        name(out, 0x09, "messages");
        out.push(0x08);
        out.extend_from_slice(&4i32.to_be_bytes());

        for _ in 0..4 {
            out.extend_from_slice(&0u16.to_be_bytes());
        }

        out.push(0x00);
    }

    let mut out = vec![0x0A];

    side(&mut out, "back_text");
    side(&mut out, "front_text");
    name(&mut out, 0x01, "is_waxed");
    out.push(0);
    out.push(0x00);
    out
}

/// Отправляет Sound Effect — звук в точке мира.
///
/// Звук называется словом, а не числом: у числа свой список, который
/// у клиента и у нас может не совпасть, а названия одинаковы всегда.
/// Место передаётся в восьмых долях блока — так в протоколе.
async fn send_sound(stream: &mut Outgoing<'_>, sound: &WorldSound) -> io::Result<()> {
    let mut body = encode_varint(SOUND_EFFECT);

    body.extend_from_slice(&encode_varint(0)); // звук назван словом, а не номером
    body.extend_from_slice(&encode_string(sound.name));
    body.push(0); // своей дальности слышимости у него нет

    // Звук сущности — в её точке и в разделе «существа»; звук блока — в
    // середине блока. Место — в восьмых долях.
    match sound.exact {
        Some((x, y, z)) => {
            let category = match (sound.of_block, sound.of_player) {
                // Оба признака — раздел «окружение» (запуск ракеты).
                (true, true) => SOUND_AMBIENT,
                (true, false) => SOUND_BLOCK,
                (false, true) => SOUND_PLAYERS,
                (false, false) => SOUND_NEUTRAL,
            };

            body.extend_from_slice(&encode_varint(category));

            // Восьмые доли отсекаются к нулю, а не вниз: поджог вагонетки на
            // высоте −59,9375 оригинал шлёт как −59,875 (чёрный ящик,
            // minecart-tnt-rail).
            push_i32(&mut body, (x * 8.0) as i32);
            push_i32(&mut body, (y * 8.0) as i32);
            push_i32(&mut body, (z * 8.0) as i32);
        }
        None => {
            body.extend_from_slice(&encode_varint(SOUND_BLOCK));
            push_i32(&mut body, sound.x * 8 + 4);
            push_i32(&mut body, sound.y * 8 + 4);
            push_i32(&mut body, sound.z * 8 + 4);
        }
    }

    body.extend_from_slice(&sound.volume.to_be_bytes());
    body.extend_from_slice(&sound.pitch.to_be_bytes());
    body.extend_from_slice(&0i64.to_be_bytes()); // случайность нам не нужна

    write_body(stream, body).await
}

/// Отправляет Explosion — раскладка 26.1 по minecraft-data
/// (data/pc/26.1/protocol.json, `packet_explosion`): центр (vec3f64), радиус
/// (f32), число разрушенных блоков (i32), толчок своего игрока (option
/// vec3f64), частица вспышки (Particle: VarInt номер и данные — у вспышек
/// данных нет), звук (ItemSoundHolder: 0 и имя словом, без своей
/// дальности — как в Sound Effect), частицы у разрушенных блоков (массив
/// ExplosionParticleEntry: частица, масштаб f32, скорость f32, вес VarInt).
///
/// Большая вспышка — у взрыва силы от 2 (вики, Explosion: «shockwave»).
/// [проверить]: какие частицы блоков шлёт оригинал (вики: с 25w33a
/// «Explosions now create smoke particles again»); у нас дымок и дым поровну.
/// Поля «Play Sound» из вики здесь нет: вики описывает протокол новее 26.1.
async fn send_explosion(stream: &mut Outgoing<'_>, blast: &world::Blast, own: Option<(f64, f64, f64)>) -> io::Result<()> {
    let mut body = encode_varint(EXPLOSION);

    push_f64(&mut body, blast.centre.0);
    push_f64(&mut body, blast.centre.1);
    push_f64(&mut body, blast.centre.2);
    push_f32(&mut body, blast.power);
    push_i32(&mut body, blast.blocks);

    match own {
        Some((x, y, z)) => {
            body.push(1);
            push_f64(&mut body, x);
            push_f64(&mut body, y);
            push_f64(&mut body, z);
        }
        None => body.push(0),
    }

    // Взрыв ветра: вихрь gust_emitter_large и звук wind_burst, дыма нет
    // (чёрный ящик, pvp-mace-wind-burst).
    let flash = if blast.wind && blast.gust == world::Gust::Charge {
        PARTICLE_GUST_EMITTER_SMALL
    } else if blast.wind {
        PARTICLE_GUST_EMITTER_LARGE
    } else if blast.power >= 2.0 {
        PARTICLE_EXPLOSION_EMITTER
    } else {
        PARTICLE_EXPLOSION
    };
    body.extend_from_slice(&encode_varint(flash));

    body.extend_from_slice(&encode_varint(0)); // звук назван словом, а не номером
    body.extend_from_slice(&encode_string(match (blast.wind, blast.gust) {
        (true, world::Gust::Breeze) => BREEZE_WIND_BURST_SOUND,
        (true, _) => WIND_BURST_SOUND,
        _ => crate::tnt::EXPLODE_SOUND,
    }));
    body.push(0); // своей дальности слышимости у него нет

    let smoke: &[(i32, f32, f32, i32)] = if blast.wind { &[] } else { &[(PARTICLE_POOF, 0.5, 1.0, 1), (PARTICLE_SMOKE, 1.0, 1.0, 1)] };
    body.extend_from_slice(&encode_varint(smoke.len() as i32));

    for &(particle, scaling, speed, weight) in smoke {
        body.extend_from_slice(&encode_varint(particle));
        push_f32(&mut body, scaling);
        push_f32(&mut body, speed);
        body.extend_from_slice(&encode_varint(weight));
    }

    write_body(stream, body).await
}

/// Отправляет Block Update — какое состояние блока клиент должен показать.
///
/// Это ответ на ломание и установку: клиент показывает изменение сразу, ещё до
/// ответа сервера (своё предсказание), а этим пакетом сервер сообщает, что
/// получилось на самом деле.
async fn send_block_update(
    stream: &mut Outgoing<'_>,
    x: i32,
    y: i32,
    z: i32,
    state: i32,
) -> io::Result<()> {
    let mut body = encode_varint(BLOCK_UPDATE);

    push_packed_position(&mut body, x, y, z);
    body.extend_from_slice(&encode_varint(state));

    write_body(stream, body).await
}

/// Отправляет Acknowledge Block Change — подтверждение действия игрока.
///
/// Клиент запоминает номер действия (`sequence`) и до подтверждения показывает
/// свой вариант, а не серверный. Подтверждение нужно отправлять на каждое
/// действие игрока с блоком, иначе клиент будет показывать не то, что в мире.
async fn send_acknowledge_block_change(stream: &mut Outgoing<'_>, sequence: i32) -> io::Result<()> {
    let mut body = encode_varint(ACKNOWLEDGE_BLOCK_CHANGE);

    body.extend_from_slice(&encode_varint(sequence));

    write_body(stream, body).await
}


/// Досылает клиенту изменения в списке игроков: кто зашёл, кто вышел.
///
/// Зашедшего мало показать в списке: остальные должны увидеть его в мире,
/// а вышедшего — перестать видеть. И то и другое делается здесь же, потому
/// что порядок пакетов важен: запись в списке обязана уйти раньше сущности.
///
/// Своя собственная запись пропускается: о себе клиент уже знает, а в списке
/// игроков ему не нужны две одинаковые строки.
async fn send_pending_player_changes(
    stream: &mut Outgoing<'_>,
    shared: &Shared,
    player: &PlayerState,
    sent: &mut usize,
    seen: &mut Seen,
) -> io::Result<()> {
    let (changes, count) = {
        let mut players = shared
            .players
            .lock()
            .expect("список игроков захвачен другим потоком");

        players.changes_since(*sent, player.reader)
    };

    for change in &changes {
        match change {
            Change::Joined(member) if member.uuid != player.uuid => {
                send_player_info(stream, member).await?;

                // Кто уже показан — при входе или раньше по этому же журналу —
                // второй раз в мир не добавляется: клиент заменил бы сущность
                // новой, а она и так на месте.
                let shown = Shown {
                    position: member.position,
                    skin_parts: member.skin_parts,
                    main_hand: member.main_hand,
                    held: member.held,
                    worn: [None; 5],
                    speed_bits: 0,
                    sprinting: false,
                    living: 0,
                    effect_colors: [0; crate::survival::EFFECT_PARTICLES],
                    health_bits: crate::survival::MAX_HEALTH.to_bits(),
                    air: crate::survival::MAX_AIR,
                    sneaking: member.sneaking,
                    swings: member.swings,
                    sleeping: None,
                };

                if member.dimension == player.dimension && seen.insert(member.entity_id, shown).is_none() {
                    send_add_entity(stream, member).await?;
                    send_look(stream, member.entity_id, member.skin_parts, member.main_hand, member.sneaking).await?;
                    send_equipment(stream, member.entity_id, &[(EQUIPMENT_MAIN_HAND, member.held)]).await?;
                }
            }
            Change::Joined(_) => {}
            Change::Left { uuid, entity_id } if *uuid != player.uuid => {
                send_remove_entity(stream, *entity_id).await?;
                send_player_info_remove(stream, uuid).await?;

                seen.remove(entity_id);
            }
            Change::Left { .. } => {}
            Change::GameMode { uuid, mode } if *uuid != player.uuid => {
                send_game_mode(stream, uuid, *mode).await?;
            }
            // О своём режиме подключение узнаёт из поручения.
            Change::GameMode { .. } => {}
            // Новый скин клиент берёт из записи в списке игроков: запись —
            // заново, и сущность — заново, иначе она осталась бы в старом.
            Change::Skin(member) if member.uuid != player.uuid => {
                let shown = seen.remove(&member.entity_id).is_some();

                if shown {
                    send_remove_entity(stream, member.entity_id).await?;
                }
                send_player_info_remove(stream, &member.uuid).await?;
                send_player_info(stream, member).await?;

                if shown {
                    let now = Shown {
                        position: member.position,
                        skin_parts: member.skin_parts,
                        main_hand: member.main_hand,
                        held: member.held,
                        worn: [None; 5],
                        speed_bits: 0,
                        sprinting: false,
                        living: 0,
                    effect_colors: [0; crate::survival::EFFECT_PARTICLES],
                    health_bits: crate::survival::MAX_HEALTH.to_bits(),
                    air: crate::survival::MAX_AIR,
                        sneaking: member.sneaking,
                        swings: member.swings,
                        sleeping: None,
                    };
                    seen.insert(member.entity_id, now);
                    send_add_entity(stream, member).await?;
                    send_look(stream, member.entity_id, member.skin_parts, member.main_hand, member.sneaking).await?;
                    send_equipment(stream, member.entity_id, &[(EQUIPMENT_MAIN_HAND, member.held)]).await?;
                }
            }
            Change::Skin(_) => {}
            Change::Armor { uuid, entity_id, entries } if *uuid != player.uuid && seen.contains_key(entity_id) => {
                let mut body = encode_varint(0x83);

                body.extend_from_slice(&crate::attributes::encode_update(*entity_id, entries));
                write_body(stream, body).await?;
            }
            Change::Armor { .. } => {}
        }
    }

    *sent = count;

    Ok(())
}

/// Досылает клиенту сообщения чата, которых он ещё не видел.
/// Табло, команды и полосы босса (scoreboard.rs): вошедшему — снимок, дальше
/// — лента пакетов.
async fn send_pending_score(stream: &mut Outgoing<'_>, shared: &Shared, uuid: &[u8; 16], sent: &mut Option<usize>) -> io::Result<()> {
    let (bodies, count) = {
        let board = shared.scoreboard.lock().expect("табло захвачено другим потоком");

        match *sent {
            None => (board.snapshot_for(uuid), board.count()),
            Some(from) if from == board.count() => return Ok(()),
            Some(from) => (board.since_for(from, uuid), board.count()),
        }
    };

    for body in bodies {
        write_body(stream, body).await?;
    }

    *sent = Some(count);
    Ok(())
}

/// Счёт по условиям, что сервер ведёт сам: здоровье, сытость, уровень,
/// опыт, воздух (вики, Scoreboard#Criteria). Здоровье — вверх до целого.
/// Как у оригинала, счёт пишется только при изменении значения (и в первый
/// раз после входа): цель, заведённая позже, получает счёт с первым
/// изменением [проверить].
fn note_criteria(shared: &Shared, player: &PlayerState, seen: &mut Option<[i32; 5]>) {
    const CRITERIA: [&str; 5] = ["health", "food", "air", "level", "xp"];
    let now = [player.vit.health.ceil() as i32, player.vit.food, player.vit.air, crate::survival::level_of(player.vit.xp).0, player.vit.xp];

    if *seen == Some(now) {
        return;
    }

    let before = seen.replace(now);
    let mut board = shared.scoreboard.lock().expect("табло захвачено другим потоком");

    for (index, criteria) in CRITERIA.iter().enumerate() {
        if before.is_none_or(|before| before[index] != now[index]) && board.watches(criteria) {
            board.set_criteria(&player.name, criteria, now[index]);
        }
    }
}

/// Раз в такт: счётчики времени, условия по вещам, табло по статистике,
/// ответ на запрос экрана статистики, достижения (пакет, объявление,
/// награда) и запись на диск раз в пять минут (progress.rs).
async fn progress_tick(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState) -> io::Result<()> {
    player.progress.tick_player(player.sneaking, !player.vit.dead, player.sleeping.is_some());
    if !player.vit.dead {
        // Фантомам нужно знать, сколько игрок не спал (src/mob/phantom.rs).
        let rest = player.progress.stats.get(crate::stats::Cat::Custom, "time_since_rest");

        shared.players.lock().expect("список игроков захвачен другим потоком").set_rest(&player.uuid, rest);
    }

    progression::effects(player);
    // Предметы у игрока: набор сверяется с таблицей «есть предмет».
    {
        let mut names: Vec<&str> = Vec::new();

        for slot in 5..inventory::SLOTS as i32 {
            if let Some(stack) = player.inventory.slot(slot)
                && let Some(name) = blocks::item_name(stack.item)
            {
                names.push(name);
            }
        }

        player.progress.scan_items(&names);
        player.progress.recipes.scan(&names, names.len(), player.net.in_water);
    }

    if !player.progress.adv.started() {
        write_body(stream, player.progress.recipes.settings_body()).await?;
    }
    if let Some(body) = player.progress.recipes.update() { write_body(stream, body).await?; }

    // Биом под ногами раз в секунду.
    if player.progress.ticks.is_multiple_of(20) {
        progression::location(shared, player);
    }

    {
        let mut board = shared.scoreboard.lock().expect("табло захвачено другим потоком");

        if !board.stat_criteria().is_empty() {
            player.progress.stats.sync_board(&player.name, &mut board);
        }
    }

    if player.progress.stats_requested && player.progress.stats.has_updates() {
        let body = player.progress.stats.award_body();

        write_body(stream, body).await?;
    }

    // Первый пакет — полный (сброс); позже — только разница.
    if !player.progress.adv.started()
        && let Some(body) = player.progress.adv.update_body(true)
    {
        write_body(stream, body).await?;
    }

    let done = player.progress.adv.take_completed();
    let announce = shared.announce_advancements.load(std::sync::atomic::Ordering::Relaxed);

    if let Some(body) = player.progress.adv.update_body(false) {
        write_body(stream, body).await?;
    }

    for def in done {
        // Корни не объявляются и окна не дают; опыт — у всех, у кого есть.
        if def.xp > 0 {
            vitals::command_xp(stream, player, false, def.xp, false).await?;
        }

        if announce && !def.is_root() {
            let component = crate::advancements::announcement(&player.name, def);
            let line = format!("{} has made the {} [{}]", player.name, match def.frame {
                1 => "challenge",
                2 => "goal",
                _ => "advancement",
            }, def.title);

            shared.chat.lock().expect("чат захвачен другим потоком").push_rich(line.clone(), None, Some(crate::scoreboard::encode_component(&component)));
            log_info!("{}", line);
        }
    }

    if player.progress.ticks.is_multiple_of(crate::progress::SAVE_EVERY) {
        player.progress.save(&player.uuid);
    }

    Ok(())
}

async fn send_pending_chat(
    stream: &mut Outgoing<'_>,
    shared: &Shared,
    uuid: &[u8; 16],
    sent: &mut usize,
) -> io::Result<()> {
    let (lines, count) = {
        let chat = shared.chat.lock().expect("чат захвачен другим потоком");

        (chat.since_for_rich(*sent, uuid), chat.count())
    };

    for (line, rich) in &lines {
        match rich {
            Some(component) => {
                let mut body = encode_varint(SYSTEM_CHAT);

                body.extend_from_slice(component);
                body.push(0x00);
                write_body(stream, body).await?;
            }
            None => send_system_chat(stream, line).await?,
        }
    }

    *sent = count;

    Ok(())
}

/// Отправляет чанк с указанными координатами — из текущего состояния мира.
/// Свет посчитан заранее, вне захвата мира.
async fn send_chunk(
    stream: &mut Outgoing<'_>,
    world: &Mutex<World>,
    chunk_x: i32,
    chunk_z: i32,
    light: &ChunkLight,
) -> io::Result<()> {
    let mut body = encode_varint(CHUNK_DATA);

    push_i32(&mut body, chunk_x);
    push_i32(&mut body, chunk_z);

    // Карты высот. Клиент умеет обходиться без них: недостающие он заполняет
    // минимальной высотой.
    body.extend_from_slice(&encode_varint(0));

    // Секции чанка — одним блоком байт с длиной впереди. Под захватом мира
    // берутся только ссылки на секции (они общие с миром) и биомы, а
    // складываются байты уже без него: складывание палитр — самое долгое
    // в пакете, и такт всё это время ждал бы мира. Хранилища — тем же
    // захватом.
    let (taken, entities, shelves) = {
        let world = world.lock().expect("мир захвачен другим потоком");
        let taken = world.chunk_packet(chunk_x, chunk_z, |sections, biomes| (sections.to_vec(), biomes.to_vec()));
        let entities = world.chunk_containers(chunk_x, chunk_z);

        // Полки показывают свои предметы всем — их данные идут и в пакете
        // чанка, иначе вошедший увидит пустые полки. [проверить] так ли у
        // оригинала.
        let mut shelves: HashMap<(i32, i32, i32), Vec<u8>> = entities
            .iter()
            .filter(|(_, _, _, kind)| *kind == containers::Kind::Shelf)
            .filter_map(|&(x, y, z, _)| {
                let (wx, wz) = (chunk_x * 16 + x, chunk_z * 16 + z);

                world.container(wx, y, wz).map(|c| ((x, y, z), redstone::shelf::nbt(&c.slots)))
            })
            .collect();

        for &(x, y, z, kind) in &entities {
            let data = world.container(chunk_x * 16 + x, y, chunk_z * 16 + z);
            if let Some(c) = data {
                match kind {
                    containers::Kind::Campfire => { shelves.insert((x, y, z), crate::campfire::nbt(c)); }
                    containers::Kind::Beacon => { shelves.insert((x, y, z), crate::beacon::nbt(c)); }
                    _ => {}
                }
            }
        }

        // Спавнеры несут вид моба: по нему клиент крутит зверька внутри.
        for ((x, y, z), nbt) in world.chunk_spawners(chunk_x, chunk_z) {
            shelves.insert((x, y, z), nbt);
        }

        // Таблички несут текст.
        for ((x, y, z), nbt) in world.chunk_signs(chunk_x, chunk_z) {
            shelves.insert((x, y, z), nbt);
        }

        // Испытательные спавнеры — моба, хранилища — подошедших (trial.rs).
        for ((x, y, z), nbt) in world.chunk_trials(chunk_x, chunk_z) {
            shelves.insert((x, y, z), nbt);
        }

        (taken, entities, shelves)
    };

    let sections = taken.map(|(sections, biomes)| {
        let mut out = Vec::new();

        // У каждой секции свои 64 клетки биома: пещерные биомы лежат
        // только в толще.
        for (section, biomes) in sections.iter().zip(biomes.chunks(world::SECTION_BIOMES)) {
            push_section(&mut out, section.as_deref(), Some(biomes));
        }

        // Самая высокая секция, где есть хоть один блок: свет выше неё
        // клиент считает полным сам.
        let top = sections.iter().rposition(Option::is_some);

        // Блок-сущности, которых нет среди хранилищ: головы, датчики
        // скалка, кафедры, проигрыватели — их находим по состоянию.
        let mut others = Vec::new();

        for (index, section) in sections.iter().enumerate() {
            let Some(states) = section.as_deref() else {
                continue;
            };

            for (at, state) in states.iter().enumerate() {
                if let Some(kind) = redstone::block_entity_type(i32::from(*state)) {
                    let (x, z, y) = (at as i32 & 15, (at as i32 >> 4) & 15, at as i32 >> 8);

                    others.push((x, world::MIN_Y + index as i32 * 16 + y, z, kind));
                }
            }
        }

        (out, top, others)
    });

    let top = sections.as_ref().and_then(|(_, top, _)| *top);
    let others = sections.as_ref().map(|(_, _, others)| others.clone()).unwrap_or_default();
    let sections = sections.map(|(ready, _, _)| ready);

    match sections {
        Some(ready) => {
            body.extend_from_slice(&encode_varint(ready.len() as i32));
            body.extend_from_slice(&ready);
        }
        // Чанка нет вовсе — шлём пустой: клиент увидит пустоту и не упадёт.
        None => {
            let mut empty = Vec::new();

            for _ in 0..world::SECTIONS {
                push_section(&mut empty, None, None);
            }

            body.extend_from_slice(&encode_varint(empty.len() as i32));
            body.extend_from_slice(&empty);
        }
    }

    // Блоки с дополнительными данными: хранилища. Оригинал шлёт их без
    // данных (чёрный ящик: у сундука, воронки, бочки, выбрасывателя в
    // пакете чанка только место и вид) — содержимое уходит только в окно.
    // Головы, датчики и прочие — так же, место и вид без данных.
    let mut entities: Vec<(i32, i32, i32, i32)> = entities
        .into_iter()
        .map(|(x, y, z, kind)| (x & 15, y, z & 15, kind.block_entity_type()))
        .chain(others)
        .collect();

    entities.sort_unstable_by_key(|&(x, y, z, _)| (y, z, x));
    body.extend_from_slice(&encode_varint(entities.len() as i32));

    for (x, y, z, kind) in entities {
        body.push(((x << 4) | z) as u8);
        push_i16(&mut body, y as i16);
        body.extend_from_slice(&encode_varint(kind));

        match shelves.get(&(x, y, z)) {
            Some(nbt) => body.extend_from_slice(nbt),
            None => body.push(0), // данных нет: пустой тег
        }
    }

    push_light(&mut body, top, light);

    write_body(stream, body).await
}

/// Добавляет одну секцию чанка.
///
/// `states` — состояния 4096 блоков секции в порядке пакета; None означает
/// чанк, которого в мире ещё нет (то есть полностью пустой).
///
/// Формат: количество непустых блоков, количество блоков с жидкостью, затем
/// состояния блоков и биомы. Состояния кодируются либо одним значением, либо
/// палитрой, либо напрямую — что короче.
fn push_section(out: &mut Vec<u8>, states: Option<&[u16]>, biomes: Option<&[u16]>) {
    /// Больше этого числа разных состояний (восемь битов на блок) палитра
    /// не бывает — дальше номера пишутся прямо, как и у оригинала.
    const MAX_PALETTE: usize = 256;

    // Состояния в памяти лежат в двух байтах, а в пакет идут числами
    // со знаком — разворачиваем по дороге.
    let empty = [world::AIR as u16; world::SECTION_BLOCKS];
    let states = states.unwrap_or(&empty);

    let mut palette: Vec<i32> = Vec::new();
    let mut known: HashSet<u16> = HashSet::new();
    let mut block_count: i32 = 0;

    for packed in states {
        let state = *packed as i32;

        if state != world::AIR {
            block_count += 1;
        }

        if known.insert(*packed) {
            palette.push(state);
        }
    }

    push_i16(out, block_count as i16);
    push_i16(out, 0); // жидкостей нет

    if palette.len() == 1 {
        push_single_value_container(out, palette[0]);
    } else if palette.len() <= MAX_PALETTE {
        push_palette_container(out, states, &palette);
    } else {
        push_direct_container(out, states);
    }

    push_biomes(out, biomes);
}

/// Добавляет биомы секции.
///
/// Биом задаётся на каждые четыре блока: в секции это 4×4×4 = 64 клетки,
/// в том же порядке, что у блоков: снизу вверх, внутри — по Z, потом по X.
///
/// Палитра у биомов — до трёх битов на клетку (до восьми разных биомов);
/// больше — и номера пишутся прямо, битов столько, сколько нужно на номер
/// любого биома реестра (страница протокола, «Chunk Format» → Paletted
/// Container).
fn push_biomes(out: &mut Vec<u8>, biomes: Option<&[u16]>) {
    /// Больше битов на клетку палитра не берёт.
    const MAX_PALETTE_BITS: usize = 3;

    let Some(biomes) = biomes else {
        // Чанка ещё нет — пусть будет равнина: она в реестре первой... а
        // точнее, под тем номером, под каким её объявил сервер.
        push_single_value_container(out, plains_number());
        return;
    };

    let mut palette: Vec<u16> = Vec::new();

    for biome in biomes {
        if !palette.contains(biome) {
            palette.push(*biome);
        }
    }

    if palette.len() == 1 {
        push_single_value_container(out, palette[0] as i32);
        return;
    }

    // Битов на запись — столько, чтобы хватило на всю палитру.
    let bits = bits_for(palette.len());

    if bits > MAX_PALETTE_BITS {
        let bits = bits_for(world::terrain::biome_registry_size());
        let numbers: Vec<u32> = biomes.iter().map(|biome| *biome as u32).collect();

        out.push(bits as u8);

        for long in pack_values(&numbers, bits) {
            push_i64(out, long);
        }

        return;
    }

    out.push(bits as u8);
    out.extend_from_slice(&encode_varint(palette.len() as i32));

    for biome in &palette {
        out.extend_from_slice(&encode_varint(*biome as i32));
    }

    let indexes: Vec<u32> = biomes
        .iter()
        .map(|cell| {
            palette
                .iter()
                .position(|biome| biome == cell)
                .expect("биом обязан быть в палитре") as u32
        })
        .collect();

    for long in pack_values(&indexes, bits) {
        push_i64(out, long);
    }
}

/// Сколько битов нужно, чтобы записать любое из `count` разных значений
/// (не меньше одного).
fn bits_for(count: usize) -> usize {
    (usize::BITS - count.saturating_sub(1).leading_zeros()).max(1) as usize
}

/// Номер равнины в реестре биомов: под ним клиенту уходят места, которых
/// сервер ещё не сложил.
fn plains_number() -> i32 {
    world::terrain::Biome::Plains.number()
}

/// Добавляет контейнер состояний блоков с палитрой: 4 бита на блок, палитра
/// из встреченных состояний, данные — номера состояний внутри палитры.
fn push_palette_container(out: &mut Vec<u8>, states: &[u16], palette: &[i32]) {
    // Битов на запись — сколько нужно на палитру, но не меньше четырёх:
    // меньше у состояний блоков не бывает.
    let bits = bits_for(palette.len()).max(4);

    out.push(bits as u8);
    out.extend_from_slice(&encode_varint(palette.len() as i32));

    for state in palette {
        out.extend_from_slice(&encode_varint(*state));
    }

    // Подряд идущие блоки обычно одинаковы, поэтому место в палитре ищется
    // заново только когда состояние сменилось.
    let places: HashMap<u16, u32> = palette
        .iter()
        .enumerate()
        .map(|(place, state)| (*state as u16, place as u32))
        .collect();

    let mut indexes: Vec<u32> = Vec::with_capacity(states.len());
    let mut last: Option<(u16, u32)> = None;

    for packed in states {
        let place = match last {
            Some((state, place)) if state == *packed => place,
            _ => {
                let found = places[packed];

                last = Some((*packed, found));
                found
            }
        };

        indexes.push(place);
    }

    for long in pack_values(&indexes, bits) {
        push_i64(out, long);
    }
}

/// Добавляет контейнер состояний блоков без палитры: 15 бит на блок, палитра
/// не передаётся.
fn push_direct_container(out: &mut Vec<u8>, states: &[u16]) {
    /// Битов на запись: размер, при котором палитра уже не помогает.
    const BITS: usize = 15;

    out.push(BITS as u8);

    let values: Vec<u32> = states.iter().map(|state| *state as u32).collect();

    for long in pack_values(&values, BITS) {
        push_i64(out, long);
    }
}

/// Упаковывает значения в long: по `bits` бит на значение, значения идут от
/// младших битов, одно значение не разрывается между двумя long.
fn pack_values(values: &[u32], bits: usize) -> Vec<i64> {
    let per_long = 64 / bits;
    let mut longs = vec![0i64; values.len().div_ceil(per_long)];

    for (index, value) in values.iter().enumerate() {
        let long = index / per_long;
        let offset = (index % per_long) * bits;

        longs[long] |= (*value as i64) << offset;
    }

    longs
}

/// Добавляет контейнер с единственным значением: битов на запись 0,
/// палитра из одного идентификатора, массив данных пустой.
fn push_single_value_container(out: &mut Vec<u8>, global_id: i32) {
    out.push(0);
    out.extend_from_slice(&encode_varint(global_id));
}

/// Добавляет данные освещения чанка (`top` — номер самой высокой непустой
/// секции среди секций мира, `None` — чанк пуст).
///
/// В масках на две секции больше, чем в мире: по одной снизу и сверху. Секция
/// попадает в «пустую» маску, если свет в ней всюду нулевой, — тогда массив
/// не шлётся. Небесный свет выше секции над самым высоким блоком не шлётся
/// вовсе: там только открытое небо, и клиент считает его полным сам — как
/// у оригинала, который тоже не хранит свет пустых секций над землёй. Слать
/// полные массивы на каждую секцию — это десятки килобайт на чанк.
fn push_light(out: &mut Vec<u8>, top: Option<usize>, light: &ChunkLight) {
    let sections = world::SECTIONS as usize;

    /// Бит маски для секции мира: нулевой бит — секция под миром.
    fn bit(section: usize) -> i64 {
        1i64 << (section + 1)
    }

    let mut sky = LightMasks::default();
    let mut block = LightMasks::default();

    // Под миром темно.
    sky.empty |= 1;
    block.empty |= 1;

    // Небо — до секции над самым высоким блоком включительно. Если самый
    // высокий блок в верхней секции мира, то и секция над миром — открытое
    // небо.
    let sky_sections = top.map_or(0, |top| top + 2);

    for section in 0..sky_sections.min(sections) {
        sky.add(bit(section), &light.sky[section]);
    }

    if sky_sections > sections {
        sky.add(bit(sections), &LightSection::Even(world::light::FULL));
    }

    for section in 0..sections {
        block.add(bit(section), &light.block[section]);
    }

    block.empty |= bit(sections);

    push_bitset(out, &sky.mask_longs());
    push_bitset(out, &block.mask_longs());
    push_bitset(out, &sky.empty_longs());
    push_bitset(out, &block.empty_longs());

    for arrays in [&sky.arrays, &block.arrays] {
        out.extend_from_slice(&encode_varint(arrays.len() as i32));

        for nibbles in arrays {
            out.extend_from_slice(&encode_varint(world::light::NIBBLES as i32));
            out.extend_from_slice(&nibbles[..]);
        }
    }
}

/// Маски и массивы света одного вида — пока пакет собирается.
#[derive(Default)]
struct LightMasks {
    /// Секции, для которых шлётся массив.
    mask: i64,

    /// Секции, где свет всюду нулевой.
    empty: i64,

    arrays: Vec<[u8; world::light::NIBBLES]>,
}

impl LightMasks {
    fn add(&mut self, bit: i64, section: &LightSection) {
        if *section == LightSection::Even(0) {
            self.empty |= bit;
            return;
        }

        self.mask |= bit;
        self.arrays.push(section.nibbles());
    }

    fn mask_longs(&self) -> Vec<i64> {
        Self::longs(self.mask)
    }

    fn empty_longs(&self) -> Vec<i64> {
        Self::longs(self.empty)
    }

    /// Маска без единого бита уходит пустым списком.
    fn longs(value: i64) -> Vec<i64> {
        if value == 0 { Vec::new() } else { vec![value] }
    }
}

/// Записывает BitSet: количество long, затем сами long.
fn push_bitset(out: &mut Vec<u8>, longs: &[i64]) {
    out.extend_from_slice(&encode_varint(longs.len() as i32));

    for value in longs {
        push_i64(out, *value);
    }
}

/// Записывает позицию блока: одно long, в котором упакованы X, Y и Z.
///
/// Раскладка битов: X занимает 26 старших бит, Z — следующие 26, Y — младшие
/// 12. Так позиция кодируется во всех пакетах, где нужны координаты блока.
fn push_packed_position(out: &mut Vec<u8>, x: i32, y: i32, z: i32) {
    let packed = ((x as i64 & 0x3FF_FFFF) << 38) | ((z as i64 & 0x3FF_FFFF) << 12) | (y as i64 & 0xFFF);

    push_i64(out, packed);
}

fn push_i16(out: &mut Vec<u8>, value: i16) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn push_i32(out: &mut Vec<u8>, value: i32) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn push_i64(out: &mut Vec<u8>, value: i64) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn push_f32(out: &mut Vec<u8>, value: f32) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn push_f64(out: &mut Vec<u8>, value: f64) {
    out.extend_from_slice(&value.to_be_bytes());
}

/// Читает и логирует пакеты клиента в фазе Play и поддерживает связь.
///
/// Keep Alive отправляется по часам, а не по молчанию клиента: клиент присылает
/// пакет каждый игровой тик, поэтому «дождаться тишины» невозможно — а без
/// пакетов от сервера клиент через 30 секунд рвёт соединение.
///
/// Ожидание начала пакета идёт через `peek`, который не забирает байты из
/// потока. Поэтому прерванное по таймауту ожидание ничего не теряет: когда
/// байты появятся, пакет будет прочитан целиком.
async fn read_and_log_play_packets(
    stream: &mut Outgoing<'_>,
    shared: &Shared,
    player: &mut PlayerState,
    entered: Entered,
) -> io::Result<()> {
    /// Serverbound Client Tick End — приходит каждый тик и только засоряет лог.
    const CLIENT_TICK_END: i32 = 0x0D;

    let mut keep_alive_id: i64 = 1;
    let mut last_keep_alive = Instant::now();
    let mut last_time_age: i64 = -1;
    let mut clock_seen: Option<u64> = None;

    let mut world_changes = entered.world_changes;
    let mut player_changes = entered.player_changes;
    let mut chat_lines = entered.chat_lines;
    // Табло: None — снимок ещё не прислан (шлётся на первом круге).
    let mut score_sent: Option<usize> = None;
    // Последние записанные в табло здоровье, сытость, воздух, уровень, опыт.
    let mut criteria_seen: Option<[i32; 5]> = None;
    let mut item_changes = entered.item_changes;
    let mut seen_items = entered.seen_items;
    let mut sent_chunks = entered.sent_chunks;
    let mut standing_in = player.chunk();
    let mut seen = entered.seen;

    // Весь ли обзор уже прислан. При входе прислан только ближний квадрат —
    // даль догружается здесь, порциями.
    let mut all_chunks_sent = false;
    let mut last_view = player.view;
    let mut prefetch = Prefetch::new();

    // Круг цикла начинается либо с пакета клиента, либо с конца такта: всё,
    // что случилось в мире за такт, должно уйти клиенту сразу, а не тогда,
    // когда он в следующий раз что-нибудь пришлёт.
    let mut ticks = shared.ticks.subscribe();
    let mut starts = shared.tick_starts.subscribe();
    let mut pending: Vec<Reaction> = Vec::new();
    let mut inbound = Inbound::start(stream.stream(), shared)?;
    let wake = shared.players.lock().expect("список игроков захвачен другим потоком").waker(&player.uuid);

    loop {
        // Всё, что прошлый круг положил в очередь, уходит одной записью,
        // прежде чем ждать клиента или конца такта.
        stream.flush().await?;

        let ready = tokio::select! {
            ready = timeout(KEEP_ALIVE_INTERVAL, inbound.packets.recv()) => Some(ready),
            _ = ticks.changed() => None,
            // Поручение (перенос командой) — обработать сразу, как у оригинала,
            // где пакет переноса уходит в миг команды.
            _ = wake.notified() => Some(timeout(Duration::ZERO, std::future::pending::<Option<io::Result<(PacketInfo, u64)>>>()).await),
        };

        // Связь пропала без закрытия соединения (Wi-Fi, NAT): TCP заметит это
        // через много минут, а до тех пор ник занят и вернуться нельзя. Как
        // у оригинала — отключаем, если на Keep Alive долго нет ответа.
        if player.answered.elapsed() >= KEEP_ALIVE_TIMEOUT {
            log_info!("{} не отвечает — отключён по сроку", player.name);
            send_disconnect(stream, "Время ожидания истекло").await?;
            return Ok(());
        }

        if let Some(limit) = crate::commands_admin::idle_limit_ticks()
            && player.idle.elapsed() >= Duration::from_millis(limit * 50)
        {
            log_info!("{} выгнан: слишком долго бездействовал", player.name);

            let mut body = encode_varint(DISCONNECT);

            body.extend_from_slice(&crate::commands_admin::idling());
            write_body(stream, body).await?;

            return Ok(());
        }

        if last_keep_alive.elapsed() >= KEEP_ALIVE_INTERVAL {
            send_keep_alive(stream, keep_alive_id).await?;
            log_debug!(
                "Play: Keep Alive отправлен (id {}, прошло {:?})",
                keep_alive_id,
                last_keep_alive.elapsed()
            );

            keep_alive_id += 1;
            last_keep_alive = Instant::now();
        }

        // Время суток клиент считает сам, но время от времени его надо
        // сверять с сервером — иначе часы разойдутся. Сверяем по возрасту
        // мира, а не по своим часам: так рассылка стоит ровно на сетке тактов.
        {
            let (age, _, (changes, set_to)) = world_age(shared, player.dimension);

            // Часы перевели (команда, сон) — сразу, с записью часов.
            if clock_seen != Some(changes) {
                if clock_seen.is_some() {
                    send_time(stream, age, Some(set_to)).await?;
                }

                clock_seen = Some(changes);
            } else if age % TIME_EVERY == 0 && age != last_time_age {
                send_time(stream, age, None).await?;
                last_time_age = age;
            }
        }

        // Всё, что случилось у других игроков, доходит до этого клиента на
        // ближайшем круге цикла — то есть почти сразу: клиент присылает пакет
        // каждый игровой тик, поэтому подолгу цикл не спит.
        // Игрок перешёл в другой чанк — досылаем то, что открылось впереди.
        let chunk = player.chunk();

        if chunk != standing_in {
            standing_in = chunk;

            send_center_chunk(stream, chunk).await?;
            all_chunks_sent = false;
        }

        // Игрок поменял дальность прорисовки — лишнее убрать, недостающее
        // дослать.
        if player.view != last_view {
            last_view = player.view;
            all_chunks_sent = false;
        }

        if !all_chunks_sent {
            all_chunks_sent = send_chunks_around(
                stream,
                shared, player.dimension,
                chunk,
                &mut sent_chunks,
                (player.view, player.view),
                CHUNKS_PER_STEP,
            )
            .await?;
        }

        // Весь обзор прислан — складываем впрок то, что за его краем.
        prefetch.collect(shared, player.dimension);

        if all_chunks_sent {
            prefetch.step(shared, player.dimension, chunk, player.view);
        }

        send_pending_player_changes(stream, shared, player, &mut player_changes, &mut seen).await?;
        vehicle::send_riding(stream, shared, player).await?;
        send_other_moves(stream, shared, player, &mut seen, Phase::Moves).await?;
        send_pending_chat(stream, shared, &player.uuid, &mut chat_lines).await?;
        note_criteria(shared, player, &mut criteria_seen);
        send_pending_score(stream, shared, &player.uuid, &mut score_sent).await?;

        // Поручения от команд: выгнать, перенести. Их отдаёт не то
        // подключение, которого они касаются, поэтому забираем их отсюда.
        let mut orders = take_orders(shared, &player.uuid);

        if std::mem::take(&mut player.net.respawn) && player.vit.dead {
            orders.insert(0, Order::Respawn);
        }

        // Перенос с поворотом (`tp <ник> x y z yaw pitch`) оригинал шлёт одним
        // пакетом сверки; у нас команда кладёт перенос и поворот двумя поручениями —
        // сливаем их, иначе клиент отвечает дважды (чёрный ящик, pvp-mace: у робота
        // на перенос с поворотом приходило два пакета места вместо одного).
        let mut merged: Vec<(Order, Option<(f32, f32)>)> = Vec::with_capacity(orders.len());
        let mut incoming = orders.into_iter().peekable();

        while let Some(order) = incoming.next() {
            let look = match (&order, incoming.peek()) {
                (Order::Teleport { .. }, Some(&Order::Look { yaw, pitch })) => Some((yaw, pitch)),
                _ => None,
            };

            if look.is_some() {
                incoming.next();
            }

            merged.push((order, look));
        }

        for (order, with_look) in merged {
            let pearl_teleport = matches!(&order, Order::PearlTeleport { .. });
            if let Order::PearlTeleport { dimension, .. } = &order
                && (*dimension != player.dimension || player.vit.dead) { continue; }
            // Возрождение — тот же переход, что и в другое измерение: клиент
            // забывает мир и получает его заново, только у точки возрождения.
            let respawning = matches!(order, Order::Respawn);
            // Куда возрождаться: кровать, якорь, `spawnpoint` или точка мира
            // (play/sleep.rs).
            let mut target = None;
            let order = if respawning {
                let found = sleep::respawn_target(shared, player);
                let (dimension, (x, y, z)) = (found.dimension, found.at);

                target = Some(found);
                Order::ChangeDimension { dimension, x, y, z }
            } else {
                order
            };

            let visual = matches!(order, Order::Give { .. });

            match order {
                Order::Respawn => {}
                Order::WakeUp => player.wake_due = true,
                Order::Message { component } => {
                    let mut body = encode_varint(SYSTEM_CHAT);

                    body.extend_from_slice(&component);
                    body.push(0x00);
                    write_body(stream, body).await?;
                }
                Order::CommandDamage { amount, source, origin } => {
                    vitals::command_hit(stream, shared, player, amount, source, origin).await?;
                }
                Order::Hurt { amount, source } => player.net.hits.push(vitals::Pending { amount, source, direct: 0, hurt_yaw: None, killer: None, push: None, announced: false }),
                Order::Effect(effect) => {
                    vitals::give_effect(stream, player, effect).await?;
                    // Маяк (фоновый эффект) обновляет частицы и атрибуты на такт
                    // позже каждый раз (чёрный ящик, beacon-effect: +158 эффект,
                    // +159 Metadata 10 и Attributes).
                    if effect.ambient {
                        player.net.hold_meta = true;
                        player.net.renew_effect_particles();

                        if let Some((attribute, ..)) = effect.kind.modifier() {
                            player.attrs_touched.push((attribute, *shared.ticks.borrow() + 1));
                        }
                    }
                }
                Order::ClearEffects => vitals::clear_effects(stream, player, None).await?,
                Order::ClearEffect(kind) => vitals::clear_effects(stream, player, Some(kind)).await?,
                Order::XpTaken { points } => player.net.xp_taken.push(points),
                Order::KilledPlayer => player.progress.killed_player(),
                // Брошенное зелье бьёт как indirect_magic: Damage Event с виновником и
                // снарядом, анимация ушиба и отдача от точки удара (чёрный ящик,
                // itemaudit2-instant-damage-undead: тип 23, скорость 0; 0,275; −0,218).
                Order::MagicHit { amount, killer, from: Some((potion, at)) } => {
                    let (dx, dz) = (player.x - at.0, player.z - at.2);
                    let length = dx.hypot(dz);
                    let yaw = ((at.2 - player.z).atan2(at.0 - player.x).to_degrees() - f64::from(player.yaw)) as f32;

                    player.net.hits.push(vitals::Pending {
                        amount,
                        source: crate::survival::Source::IndirectMagic,
                        direct: potion,
                        hurt_yaw: Some(yaw),
                        killer,
                        push: (length > 1.0e-4).then(|| (dx / length, dz / length)),
                        announced: false,
                    });
                }
                Order::MagicHit { amount, killer, from: None } => player.net.hits.push(vitals::Pending { amount, source: crate::survival::Source::Magic, direct: 0, hurt_yaw: None, killer, push: None, announced: false }),
                Order::Progress(event) => player.progress.event(event),
                Order::Advancement { grant, ids, criterion } => {
                    for id in ids {
                        let changed = match (grant, &criterion) {
                            (true, Some(criterion)) => player.progress.adv.grant(id, criterion),
                            (true, None) => player.progress.adv.grant_all(id),
                            (false, Some(criterion)) => player.progress.adv.revoke(id, criterion),
                            (false, None) => player.progress.adv.revoke_all(id),
                        };

                        let _ = changed;
                    }
                }
                Order::Experience { set, amount, levels } => vitals::command_xp(stream, player, set, amount, levels).await?,
                Order::Kick { reason } => {
                    log_info!("{} выгнан: {}", player.name, reason);
                    send_disconnect(stream, &reason).await?;

                    return Ok(());
                }
                Order::KickText { component, plain } => {
                    log_info!("{} выгнан: {}", player.name, plain);

                    let mut body = encode_varint(DISCONNECT);

                    body.extend_from_slice(&component);
                    write_body(stream, body).await?;

                    return Ok(());
                }
                Order::PacketText { id, body, .. } => {
                    let mut packet = encode_varint(id);

                    packet.extend_from_slice(&body);
                    write_body(stream, packet).await?;
                }
                Order::Spawn { x, y, z, body } => {
                    // Без пакета — `spawnpoint`: личная точка (play/sleep.rs);
                    // с пакетом — `setworldspawn`: точка мира.
                    if body.is_none() {
                        sleep::set_forced(player, (x, y, z));
                    } else {
                        player.spawn_point = (x, y, z);
                    }

                    if let Some(body) = body {
                        let mut packet = encode_varint(SET_DEFAULT_SPAWN_POSITION);

                        packet.extend_from_slice(&body);
                        write_body(stream, packet).await?;
                    }
                }
                // Переход в другое измерение: клиент получает Respawn и
                // забывает всё, что видел, — чанки, предметы и игроков
                // присылаем заново из нового мира.
                Order::ChangeDimension { dimension, x, y, z } => {
                    let reader = player.reader;

                    if respawning {
                        vitals::respawned(player);

                        if let Some(yaw) = target.as_ref().and_then(|target| target.yaw) {
                            player.yaw = yaw;
                        }
                    }

                    let from_dimension = player.dimension;

                    {
                        let old = player.dimension;
                        shared.world_in(old).lock().expect("мир захвачен другим потоком").forget_reader(reader);
                        shared.items_in(old).lock().expect("предметы захвачены другим потоком").forget_reader(reader);
                    }

                    progression::changed_dimension(player, dimension, (x, y, z));
                    player.dimension = dimension;
                    player.progress.entered(dimension);
                    player.x = x;
                    player.y = y;
                    player.z = z;

                    // Из портала в Энд игрок выходит лицом на запад (замер
                    // оригинала: yaw 90, наклон прежний); переход командой
                    // поворот не трогает.
                    if dimension == Dimension::End && !player.portal_ready {
                        player.yaw = 90.0;
                    }

                    shared
                        .players
                        .lock()
                        .expect("список игроков захвачен другим потоком")
                        .set_dimension(&player.uuid, dimension);

                    // Полосу босса оригинал убирает, когда игрок покидает бой.
                    if player.boss_shown {
                        send_boss_bar(stream, player, None).await?;
                    }

                    if respawning {
                        // После смерти сперва способности, потом Respawn.
                        send_abilities(stream, player.game_mode).await?;

                        if target.as_ref().is_some_and(|target| target.missing) {
                            sleep::send_no_respawn_block(stream).await?;
                        }

                        send_respawn_after_death(stream, dimension, player).await?;

                        // Состояние такта оригинал шлёт заново и после
                        // возрождения (void-death: Set Ticking State, Step Tick).
                        for body in crate::commands_world::ticking_packets(shared) {
                            write_body(stream, body).await?;
                        }
                    } else {
                        // Порядок оригинала (чёрный ящик, dim-nether-tp, end-portal-enter):
                        // Respawn, событие прав, место, способности, точка
                        // возрождения, «жди чанки», выбранный слот.
                        let portal = std::mem::take(&mut player.portal_trip);

                        send_respawn(stream, dimension, player, if portal { 10 } else { 0 }).await?;

                        // И после смены мира (end-fight-enter: Set Ticking State, Step Tick).
                        for body in crate::commands_world::ticking_packets(shared) {
                            write_body(stream, body).await?;
                        }
                        vitals::send_op_level(stream, shared, player).await?;
                        let sync = match (portal, dimension == Dimension::Nether || from_dimension == Dimension::Nether) {
                            (false, _) => Sync::Command,
                            (true, true) => Sync::NetherPortal,
                            (true, false) => Sync::Portal,
                        };

                        send_player_position_as(stream, player, sync).await?;
                        send_abilities(stream, player.game_mode).await?;
                        send_default_spawn_position(stream, player.spawn_point).await?;
                        send_weather(stream, shared, dimension).await?;
                        send_game_event_start_waiting(stream).await?;
                        send_held_slot(stream, player.inventory.selected()).await?;

                        if portal {
                            vitals::send_vitals(stream, player).await?;
                        } else {
                            player.vitals_due = 1;
                        }
                    }

                    world_changes = shared
                        .world_in(dimension)
                        .lock()
                        .expect("мир захвачен другим потоком")
                        .watch_events(reader);

                    let lying = {
                        let mut items = shared.items_in(dimension).lock().expect("предметы захвачены другим потоком");
                        item_changes = items.watch_changes(reader);
                        items.items().to_vec()
                    };

                    // Возрождение в том же измерении: клиент оставляет мир и всё, что
                    // в нём стоит, — оригинал чужих сущностей заново не шлёт
                    // (чёрный ящик, pvp-mace-heavy: чужой игрок не пересылался).
                    if from_dimension != dimension {
                        seen_items.clear();
                        for item in &lying {
                            send_dropped_item(stream, item).await?;
                            seen_items.insert(item.entity_id, shown_item(item));
                        }

                        player.seen_carts.clear();
                        send_all_carts(stream, shared, player).await?;
                        player.seen_frames.clear();
                        send_all_frames(stream, shared, player).await?;
                        mobs::send_all_mobs(stream, shared, player).await?;
                    }

                    // Рамка нового измерения (у каждого своя).
                    write_body(stream, crate::commands_world::border_of(shared, dimension).init_packet()).await?;

                    // Полоса босса: клиент после Respawn её забывает; в Крае с
                    // живым драконом — показать заново.
                    player.boss_shown = false;
                    let boss = (dimension == Dimension::End).then(|| shared.world_in(dimension).lock().expect("мир захвачен другим потоком").fight.boss).flatten();
                    if boss.is_some() {
                        send_boss_bar(stream, player, boss).await?;
                    }

                    // Чужих игроков покажет сверка видимости на этом же круге.
                    if from_dimension != dimension {
                        seen.clear();
                    }
                    sent_chunks.clear();
                    prefetch = Prefetch::new();

                    // Как у оригинала, всё о самом игроке уходит сразу, до чанков.
                    // (После перехода без смерти место и «жди чанки» ушли выше:
                    // без «жди чанки» клиент висит на экране загрузки и не шлёт
                    // движений — переход порталом в обычный мир 30.09.)
                    if respawning {
                        send_player_position(stream, player).await?;
                        publish_player(shared, player);
                        vitals::send_respawn_intro(stream, player).await?;
                        send_weather(stream, shared, dimension).await?;
                        send_game_event_start_waiting(stream).await?;
                        let deplete = target.as_mut().and_then(|target| target.deplete.take());

                        vitals::send_respawn_extras(stream, shared, player, deplete).await?;
                    } else {
                        publish_player(shared, player);
                    }

                    let chunk = player.chunk();
                    standing_in = chunk;
                    send_center_chunk(stream, chunk).await?;
                    send_chunks_around(
                        stream,
                        shared,
                        player.dimension,
                        chunk,
                        &mut sent_chunks,
                        (NEAR_CHUNKS, player.view),
                        usize::MAX,
                    )
                    .await?;
                    all_chunks_sent = false;

                    send_container_content(stream, player).await?;

                    player.save(&shared.playerdata);

                    log_info!("{} перешёл в {} ({:.1}, {:.1}, {:.1})", player.name, dimension.name(), x, y, z);
                }

                Order::Look { yaw, pitch } => {
                    player.yaw = yaw;
                    player.pitch = pitch;

                    send_player_position(stream, player).await?;
                    publish_player(shared, player);
                }

                Order::Teleport { x, y, z } | Order::PearlTeleport { x, y, z, .. } => {
                    if pearl_teleport {
                        vehicle::dismount(shared, player);
                        // Звук телепорта — в точке прибытия (чёрный ящик, itemaudit3-ender_pearl-near: (3,375; 0,375; 4,375)).
                        shared.world_in(player.dimension).lock().expect("мир").note_sound_player_at((x,y,z), "minecraft:entity.player.teleport", 1.0, 1.0);
                        player.vit.fall = 0.0;
                        player.vit.landed_from = 0.0;
                        player.net.last_delta = (0.0, 0.0, 0.0);
                        player.net.pin = None;
                    }
                    player.x = x;
                    player.y = y;
                    player.z = z;

                    // С поворотом — один пакет, взгляд абсолютный; без — относительный
                    // нулевой (флаги 24, чёрный ящик, dim-nether-tp).
                    if let Some((yaw, pitch)) = with_look {
                        player.yaw = yaw;
                        player.pitch = pitch;
                        send_player_position(stream, player).await?;
                    } else {
                        send_player_position_as(stream, player, Sync::Command).await?;
                    }

                    player.save(&shared.playerdata);

                    publish_player(shared, player);

                    // Перенесли далеко — под ногами может не быть ни одного
                    // присланного чанка.
                    let chunk = player.chunk();

                    standing_in = chunk;
                    send_center_chunk(stream, chunk).await?;

                    // Под ногами — сразу, даль — порциями в цикле.
                    send_chunks_around(
                        stream,
                        shared, player.dimension,
                        chunk,
                        &mut sent_chunks,
                        (NEAR_CHUNKS, player.view),
                        usize::MAX,
                    )
                    .await?;
                    all_chunks_sent = false;
                    if pearl_teleport {
                        vitals::hit(stream, shared, player, 5.0, crate::survival::Source::EnderPearl, 0).await?;
                    }

                    log_debug!(
                        "Play: игрок {} перенесён в {:.2} {:.2} {:.2}",
                        player.name,
                        x,
                        y,
                        z
                    );
                }

                // Выданные предметы кладутся в инвентарь; если места нет,
                // лишнее падает на землю рядом с игроком.
                Order::Give { stack } | Order::Pick { stack } => {
                    let (item, count) = (stack.item, stack.count);
                    let slots = give::give(shared, player, stack, visual);

                    let shown = slots.iter().any(|slot| *slot == player.inventory.selected_slot() || *slot == inventory::OFFHAND);

                    for slot in slots {
                        send_container_slot(stream, player, slot).await?;
                    }

                    // Легло в руку — соседи видят новую экипировку (чёрный ящик,
                    // trident-loyalty: вернувшийся трезубец в руке у соседей на
                    // такт позже ячейки).
                    if shown {
                        publish_player(shared, player);
                    }

                    log_debug!(
                        "Play: игроку {} выдано: предмет {} × {}",
                        player.name,
                        item,
                        count
                    );
                }

                // Режим сменила команда — своя или чужая (с Bedrock, из
                // консоли): клиенту нужен Game Event «смена режима», строка
                // в списке игроков — тоже.
                Order::GameMode { mode } => {
                    if player.game_mode != mode {
                        player.prev_mode = player.game_mode;
                    }

                    player.game_mode = mode;
                    player.creative_flying = false;
                    send_game_event_game_mode(stream, mode).await?;
                    send_abilities(stream, mode).await?;
                    send_game_mode(stream, &player.uuid, mode).await?;
                    player.save(&shared.playerdata);
                }

                Order::SetSlot { slot, stack } => {
                    // Команда подменила надетую вещь той же вещью с другими
                    // данными (чёрный ящик, tails-armor-equip-command:
                    // damage=5 — снова звук надевания); износ от ударов
                    // молчит, там вещь меняется не командой.
                    if (5..=8).contains(&slot)
                        && let (Some(old), Some(new)) = (player.inventory.slot(slot), stack)
                        && old.item == new.item && old != new
                        && let Some(info) = crate::combat::info(new.item)
                        && !info.equip_sound.is_empty()
                    {
                        shared.world_in(player.dimension).lock().expect("мир").note_sound_player_at((player.x, player.y, player.z), info.equip_sound, 1.0, 1.0);
                    }
                    player.inventory.set(slot, stack);
                    send_container_slot(stream, player, slot).await?;
                }

                Order::SetEnderSlot { slot, stack } => {
                    player.ender[slot] = stack;
                    player.save(&shared.playerdata);
                }

                // Готовый пакет команд мира (рамка, такт, наездники).
                Order::Packet(body) => write_body(stream, body).await?,

                // `attribute`: правка уходит клиенту обычной сверкой атрибутов
                // в конце такта.
                // Оригинал шлёт Update Attributes тактом позже (чёрный ящик,
                // cmd-attribute).
                Order::Attribute(edit) => {
                    let attribute = crate::commands_world::edited_attribute(&edit);

                    crate::commands_world::apply_attribute_edit(&mut player.attrs, &edit);
                    player.attrs_touched.push((attribute, *shared.ticks.borrow() + 1));
                }

                // `enchant`: зачарование предмету в основной руке.
                Order::Enchant { enchantment, level } => {
                    let slot = inventory::FIRST_HOTBAR + player.inventory.selected();

                    if let Some(stack) = player.inventory.slot(slot) {
                        let mut data = stack.extras();

                        data.set_enchant(enchantment as u16, level);
                        player.inventory.set(slot, Some(stack.with_extras(data)));
                        send_container_slot(stream, player, slot).await?;

                        // Следующая команда `enchant` должна видеть новые чары.
                        publish_player(shared, player);
                    }
                }

                // `spectate`: смотреть глазами сущности (Set Camera); своя —
                // вернуть взгляд себе.
                Order::Spectate { entity_id } => {
                    let mut body = encode_varint(CAMERA);

                    body.extend_from_slice(&encode_varint(entity_id.unwrap_or(player.entity_id)));
                    write_body(stream, body).await?;
                }

                Order::Hold { stack } => {
                    let slot = inventory::FIRST_HOTBAR + player.inventory.selected();

                    player.inventory.set(slot, stack);
                    send_container_slot(stream, player, slot).await?;
                }

                Order::Clear => {
                    for slot in 0..inventory::SLOTS as i32 {
                        player.inventory.set(slot, None);
                    }

                    send_container_content(stream, player).await?;
                    log_debug!("Play: инвентарь игрока {} очищен", player.name);
                }
            }
        }

        // Предмет поднимается не по команде клиента, а сам: сервер смотрит,
        // до чего игрок дотянулся.
        for slot in pick_up_items(shared, player) {
            send_container_slot(stream, player, slot).await?;
        }

        send_pending_item_changes(
            stream,
            shared, player.dimension,
            &mut item_changes,
            &mut seen_items,
            Known { carts: &mut player.seen_carts, frames: &mut player.seen_frames, mobs: &mut player.seen_mobs, listener: (player.x, player.y, player.z) },
            player.reader,
        )
        .await?;
        send_item_moves(stream, shared, player.dimension, &mut seen_items).await?;

        match ready {
            // Клиент молчит — на этом круге Keep Alive уже отправлен выше.
            // Такт закончился: всё, что за него случилось в мире, уходит
            // клиенту разом и в порядке, в каком случилось, — как у оригинала,
            // где пакеты копятся и уходят в конце такта. Следом — ответы на
            // его собственные действия: подтверждение должно идти после
            // изменения мира, иначе клиент на миг покажет пустое место.
            None => {
                mining::tick(shared, player);
                step_portal(shared, player);

                {
                    let _publication = shared.publication.lock().expect("снимок рассылки");
                    player.publication = Some(TrackingPublication {
                        tick: *shared.ticks.borrow(),
                        events: shared.world_in(player.dimension).lock().expect("мир").published_events,
                        players: shared.players.lock().expect("игроки").published().to_vec(),
                    });
                }

                let lagged =
                    flush_tick(stream, shared, player, &mut world_changes, &mut pending).await?;

                // Взмахи чужих игроков — после событий мира, перед здоровьем.
                send_other_moves(stream, shared, player, &mut seen, Phase::Swings).await?;

                // Здоровье, урон, еда — после событий мира и ответов клиенту.
                if player.vitals_due > 0 {
                    player.vitals_due -= 1;

                    if player.vitals_due == 0 {
                        vitals::send_vitals(stream, player).await?;
                    }
                }
                sleep::tick(shared, player);
                vitals::tick(stream, shared, player).await?;
                progress_tick(stream, shared, player).await?;
                maps::send_ticks(stream, shared, player).await?;
                glide::tick(shared, player);

                if let Some(speed) = glide::boost(player) {
                    send_velocity(stream, player.entity_id, speed).await?;
                }

                // Экипировка чужих игроков — следом за здоровьем.
                send_other_moves(stream, shared, player, &mut seen, Phase::Gear).await?;

                // Движение вагонеток — в конце такта, после изменений мира,
                // как у оригинала.
                send_cart_moves(stream, shared, player).await?;
                send_frame_intros(stream, player).await?;
                send_item_ticks(stream, shared, player.dimension, &mut seen_items).await?;

                // Едущие блоки поршней сдвинули игрока: по серверу он уже там.
                let shoved = shared.players.lock().expect("список игроков захвачен другим потоком").take_shoved(&player.uuid);

                if let Some(((dx, dy, dz), _)) = shoved {
                    player.x += dx;
                    player.y += dy;
                    player.z += dz;
                    player.shoved_to = Some((player.x, player.y, player.z));
                    publish_player(shared, player);
                }

                // Журнал выбросил события, не дождавшись этого клиента: его
                // картина мира разошлась с сервером. Обзор присылается заново.
                if lagged {
                    log_info!("{} отстал от событий мира — чанки присылаются заново", player.name);
                    sent_chunks.clear();
                    all_chunks_sent = false;
                }
            }
            Some(Err(_)) => {}
            // Читатель закончил: клиент закрыл соединение.
            Some(Ok(None)) => {
                log_debug!("Play: клиент закрыл соединение");
                return Ok(());
            }
            Some(Ok(Some(Ok((packet, arrived))))) => {
                if packet.id == CLIENT_TICK_END {
                    continue;
                }

                // Действия с миром применяются на границе тактов, как у
                // оригинала: щелчок, ломание, установка и команда, пришедшие
                // во время такта, ложатся в начало следующего. Иначе щелчок по
                // рычагу и ход поршня от него разъезжались бы по тактам как
                // попало. Такт `arrived` к приходу уже шёл — ждём следующего;
                // если он уже начался, он ждёт нас (`waiting_for_tick`).
                let gated = is_gated(packet.id);

                // Команда, пришедшая в простое (работа такта кончилась, следующий
                // не начался), исполняется сразу: оригинал берёт её между тактами
                // тут же, и ответ (пакет переноса `tp`) уходит через ~1 мс, а не
                // на начале следующего такта (чёрный ящик, pvp-mace: у оригинала
                // перенос на +1 мс при любой фазе, у нас на +27 — падающий робот
                // успевал на тик меньше). Мир при этом не меняется: до начала
                // такта всё равно ничего не идёт.
                let at_once = packet.id == SERVERBOUND_CHAT_COMMAND && *ticks.borrow() >= *starts.borrow();

                if gated && !at_once {
                    // Ждать начала такта с неотправленным нельзя: оно
                    // задержалось бы на такт.
                    stream.flush().await?;

                    while *starts.borrow_and_update() <= arrived {
                        if starts.changed().await.is_err() {
                            break;
                        }
                    }
                }

                let reaction = handle_client_packet(&packet, player, shared);

                // Действие в начале такта могло сменить то, что видят другие
                // (лук опущен, вещь износилась): им это должно быть известно
                // уже в этом такте, а не когда подключение дойдёт до конца
                // своего — иначе соседи видят перемену на такт позже, смотря
                // кто первым (pvp-bow, 30.09).
                if gated {
                    publish_player(shared, player);
                }

                // Команда перенесла самого игрока: в мире он на новом месте
                // уже в этом такте, до шага сущностей, как у оригинала
                // (minecart-push: вагонетку толкает в такте команды). Пакет
                // переноса клиенту уйдёт с поручением на ближайшем круге.
                let moved = if packet.id == SERVERBOUND_CHAT_COMMAND {
                    shared.players.lock().expect("список игроков захвачен другим потоком").pending_teleport(&player.uuid)
                } else {
                    None
                };

                if let Some((x, y, z)) = moved {
                    (player.x, player.y, player.z) = (x, y, z);
                    publish_player(shared, player);
                }

                if gated {
                    shared.waiting_for_tick[(arrived % 2) as usize].fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
                }

                // Положение записываем сразу, как только оно изменилось
                // заметно, — так же, как сохраняются блоки.
                player.save_if_needed(&shared.playerdata);

                // Остальные узнают, где игрок стоит, из общего списка:
                // туда это кладётся здесь, а рассылают его уже их
                // подключения.
                publish_player(shared, player);

                // Ответ клиенту уходит в конце такта, вместе с тем, что
                // за такт изменилось в мире.
                pending.push(reaction);
            }
            Some(Ok(Some(Err(e)))) if is_client_disconnect(&e) => {
                log_debug!("Play: клиент закрыл соединение ({})", e.kind());
                return Ok(());
            }
            Some(Ok(Some(Err(e)))) => return Err(e),
        }
    }
}

/// Действия с миром: их применяют только в начале такта.
fn is_gated(packet_id: i32) -> bool {
    matches!(
        packet_id,
        USE_ITEM_ON | PLAYER_ACTION | USE_ITEM | SERVERBOUND_CHAT_COMMAND | ATTACK
            | SET_PLAYER_POSITION | SET_PLAYER_POSITION_AND_ROTATION | SET_PLAYER_ON_GROUND
            // Взмах и смена ячейки тоже ждут начала такта: взмах сбрасывает
            // заряд руки и виден другим, смена ячейки меняет экипировку. Без
            // этого взмах, пришедший вслед за ударом, успевал или не успевал
            // к концу такта (он длится доли миллисекунды) — чужие видели его
            // то в такте удара, то в следующем (pvp-sword-charged, 30.09).
            | SWING_ARM | SET_CARRIED_ITEM
            // Окно командного блока меняет мир — тоже в начале такта.
            | PROGRAM_COMMAND_BLOCK
            // Правка таблички меняет мир.
            | signs::SIGN_UPDATE
    )
}

/// Пакеты клиента, прочитанные отдельной задачей.
///
/// Подключение и читает, и шлёт; пока оно отправляет клиенту итог такта,
/// читать некому, и щелчок, пришедший до начала следующего такта, ждал бы
/// ещё один (в чёрном ящике — «первый отклик 0 / 1»). Читатель работает
/// сам по себе, как сетевой поток у оригинала: каждое действие с миром он
/// сразу отмечает в `waiting_for_tick` и подписывает номером такта, во время
/// которого оно пришло, — такт его дождётся.
struct Inbound {
    packets: tokio::sync::mpsc::UnboundedReceiver<io::Result<(PacketInfo, u64)>>,
    reader: tokio::task::JoinHandle<()>,
    waiting: Arc<[std::sync::atomic::AtomicUsize; 2]>,
}

impl Inbound {
    fn start(stream: &TcpStream, shared: &Shared) -> io::Result<Self> {
        // Второй дескриптор того же сокета: читатель владеет своим, запись
        // остаётся за подключением.
        #[cfg(unix)]
        let own = {
            use std::os::fd::AsFd;

            std::net::TcpStream::from(stream.as_fd().try_clone_to_owned()?)
        };
        #[cfg(windows)]
        let own = {
            use std::os::windows::io::AsSocket;

            std::net::TcpStream::from(stream.as_socket().try_clone_to_owned()?)
        };
        own.set_nonblocking(true)?;

        // Чтение — через буфер: клиент шлёт по нескольку мелких пакетов за
        // такт, и без буфера каждый байт длины был бы своим системным вызовом.
        let mut own = tokio::io::BufReader::new(TcpStream::from_std(own)?);
        let (sender, packets) = tokio::sync::mpsc::unbounded_channel();
        let waiting = shared.waiting_for_tick.clone();
        let counted = waiting.clone();
        let starts = shared.tick_starts.subscribe();

        let reader = tokio::spawn(async move {
            loop {
                let read = read_packet(&mut own).await;
                let arrived = *starts.borrow();
                let failed = read.is_err();

                if let Ok(packet) = &read
                    && is_gated(packet.id)
                {
                    waiting[(arrived % 2) as usize].fetch_add(1, std::sync::atomic::Ordering::AcqRel);
                }

                if sender.send(read.map(|packet| (packet, arrived))).is_err() || failed {
                    return;
                }
            }
        });

        Ok(Self { packets, reader, waiting: counted })
    }
}

impl Drop for Inbound {
    fn drop(&mut self) {
        self.reader.abort();

        // Прочитанные, но не применённые действия такт больше не ждёт.
        while let Ok(read) = self.packets.try_recv() {
            if let Ok((packet, arrived)) = read
                && is_gated(packet.id)
            {
                self.waiting[(arrived % 2) as usize].fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
            }
        }
    }
}

/// Program Command Block: оператор в творческом режиме задаёт блоку
/// команду, режим и признаки; в ответ — «Command set: …» (чёрный ящик,
/// cmdblock-impulse). Прочим — ничего, как у оригинала.
fn program_command_block(payload: &[u8], player: &PlayerState, shared: &Shared, reaction: &mut Reaction) {
    let mut cursor = Cursor::new(payload);
    let (Some((x, y, z)), Some(command), Some(mode), Some(flags)) = (cursor.position(), cursor.string(), cursor.varint(), cursor.u8()) else {
        return;
    };

    if !crate::command_block::may_edit(shared, &player.uuid, player.game_mode) {
        log_debug!("Play: {} не может править командный блок", player.name);
        return;
    }

    let answer = {
        let mut world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");

        crate::command_block::program(&mut world, (x, y, z), &command, mode, flags)
    };

    if let Some(component) = answer {
        log_info!("{} задал команду блоку в {} {} {}: {}", player.name, x, y, z, command);

        let mut body = encode_varint(SYSTEM_CHAT);

        body.extend_from_slice(&component);
        body.push(0x00);
        reaction.packets.push(body);
    }
}

/// Отправляет клиенту ответ на его пакет: подтверждение, поправки блоков,
/// режим игры, текст, содержимое инвентаря, подсказки.
async fn apply_reaction(
    stream: &mut Outgoing<'_>,
    shared: &Shared,
    player: &mut PlayerState,
    reaction: &Reaction,
) -> io::Result<()> {
    for body in &reaction.packets {
        write_body(stream, body.clone()).await?;
    }

    if let Some(sequence) = reaction.acknowledge {
        send_acknowledge_block_change(stream, sequence).await?;
    }

    // Если блок не поставлен, клиенту надо сказать, что там
    // на самом деле: свой блок он уже показал, не дожидаясь
    // сервера, и без поправки тот остался бы призраком.
    for (x, y, z) in &reaction.confirm {
        let state = shared
            .world_in(player.dimension)
            .lock()
            .expect("мир захвачен другим потоком")
            .get_block(*x, *y, *z);

        send_block_update(stream, *x, *y, *z, state).await?;
    }

    if let Some(game_mode) = reaction.game_mode {
        send_game_mode(stream, &player.uuid, game_mode).await?;
    }

    if let Some(component) = &reaction.reply_component {
        // Пустой — ответ игроку уже ушёл рассылкой (`random roll`).
        for part in crate::commands_world::split_components(component).into_iter().filter(|part| !part.is_empty()) {
            let mut body = encode_varint(SYSTEM_CHAT);

            body.extend_from_slice(part);
            body.push(0x00);
            write_body(stream, body).await?;
        }
    } else if !reaction.reply_rich.is_empty() {
        for component in &reaction.reply_rich {
            let mut body = encode_varint(SYSTEM_CHAT);

            body.extend_from_slice(component);
            body.push(0x00);
            write_body(stream, body).await?;
        }
    } else if let Some(reply) = &reaction.reply {
        send_system_chat(stream, reply).await?;
    }

    if reaction.inventory {
        send_container_content(stream, player).await?;
    }

    for slot in reaction.slots.clone() {
        send_container_slot(stream, player, slot).await?;
    }

    if let Some(slot) = reaction.held {
        send_held_slot(stream, slot).await?;
    }

    if let Some(hints) = &reaction.suggestions {
        send_suggestions(stream, hints).await?;
    }

    for (slot, stack) in reaction.window_slots.clone() {
        send_window_slot(stream, player, slot, stack).await?;
    }

    if let Some(stack) = reaction.cursor {
        let mut body = encode_varint(SET_CURSOR_ITEM);

        push_slot(&mut body, stack);
        write_body(stream, body).await?;
    }

    if let Some(cart) = reaction.open_cart
        && player.window.is_none()
    {
        open_cart_window(stream, shared, player, cart).await?;
    }

    if let Some(villager) = reaction.open_merchant {
        if player.window.is_none() {
            merchant::open_merchant_window(stream, shared, player, villager).await?;
        } else {
            merchant::release(shared, player.dimension, villager);
        }
    }

    if let Some((window, property, value)) = reaction.window_property {
        send_window_property(stream, window, property, value).await?;
    }

    for &(window, property, value) in &reaction.window_properties {
        send_window_property(stream, window, property, value).await?;
    }

    if reaction.experience {
        vitals::send_experience(stream, &player.vit).await?;
    }

    if reaction.resync {
        send_player_position(stream, player).await?;
    }

    if let Some(window) = reaction.close_window {
        let mut body = encode_varint(CLOSE_CONTAINER_CLIENTBOUND);

        body.extend_from_slice(&encode_varint(window));
        write_body(stream, body).await?;
    }

    Ok(())
}

/// Set Container Property: одно свойство окна.
async fn send_window_property(stream: &mut Outgoing<'_>, window: i32, property: i16, value: i16) -> io::Result<()> {
    let mut body = encode_varint(SET_CONTAINER_PROPERTY);

    body.extend_from_slice(&encode_varint(window));
    push_i16(&mut body, property);
    push_i16(&mut body, value);
    write_body(stream, body).await
}

/// Что сервер должен сделать в ответ на пакет клиента.
///
/// Собирается в один набор, потому что на некоторые пакеты нужно ответить
/// сразу нескольким: например, команда смены режима и меняет режим, и требует
/// уведомить об этом клиента.
#[derive(Default)]
struct Reaction {
    /// Номер действия с блоком, которое нужно подтвердить клиенту.
    acknowledge: Option<i32>,

    /// Места, о настоящем состоянии которых надо сообщить клиенту.
    ///
    /// Клиент показывает поставленный блок сразу, не дожидаясь сервера. Если
    /// сервер блок не поставил, клиенту нужно сказать, что там на самом деле,
    /// иначе у него останется блок-призрак.
    confirm: Vec<(i32, i32, i32)>,

    /// Новый режим игры, о котором нужно сообщить клиенту.
    game_mode: Option<i32>,

    /// Ответ на команду — уходит только этому игроку.
    reply: Option<String>,

    /// Тот же ответ текстом игры (commands_world.rs): если есть, уходит он.
    reply_component: Option<Vec<u8>>,
    /// Ответ команды готовыми текстами Java (переводы оригинала); если есть —
    /// вместо `reply`.
    reply_rich: Vec<Vec<u8>>,

    /// Показать клиенту инвентарь целиком.
    ///
    /// Нужно после каждого щелчка по слотам: у клиента своя картинка
    /// инвентаря, и она должна совпасть с тем, что знает сервер.
    inventory: bool,

    /// Слоты, содержимое которых изменилось: их достаточно показать поодиночке.
    slots: Vec<i32>,

    /// Новый выбранный слот панели, о котором надо сказать клиенту.
    held: Option<i32>,

    /// Подсказки к набираемой команде: что дописать, с какого места и
    /// сколько знаков заменить.
    suggestions: Option<Suggestions>,

    /// Ячейки открытого окна хранилища, которые клиент предсказал не так.
    window_slots: Vec<(usize, Option<Stack>)>,

    /// Курсор, если клиент его предсказал не так.
    cursor: Option<Option<Stack>>,

    /// Открыть окно этой вагонетки.
    open_cart: Option<i32>,
    /// Открыть окно торговли с этим жителем.
    open_merchant: Option<i32>,

    /// Свойство открытого окна: номер окна, свойства и значение (страница
    /// кафедры).
    window_property: Option<(i32, i16, i16)>,

    /// Окно, которое надо закрыть у клиента (с кафедры забрали книгу).
    close_window: Option<i32>,

    /// Ещё свойства окна (стол зачарования, наковальня) — по порядку.
    window_properties: Vec<(i32, i16, i16)>,

    /// Опыт поменялся (зачарование, наковальня): прислать Set Experience.
    experience: bool,

    /// Вернуть клиента на место, где он по серверу (его сдвинул поршень, а
    /// клиент прислал прежнее место).
    resync: bool,

    /// Готовые пакеты (номер и тело) — уходят первыми, до подтверждения:
    /// данные командного блока при открытии окна.
    packets: Vec<Vec<u8>>,
}

/// Подсказки клиенту в ответ на Tab.
struct Suggestions {
    /// Номер запроса — клиент присылает его и ждёт обратно.
    id: i32,

    /// С какого знака в строке заменять и сколько знаков.
    start: i32,
    length: i32,

    /// Чем дописать.
    matches: Vec<String>,
}

impl Reaction {
    /// Отказ от действия с блоком: подтверждение и настоящее состояние тех
    /// мест, куда клиент мог поставить блок.
    fn refused(sequence: i32, places: &[(i32, i32, i32)]) -> Self {
        Self {
            acknowledge: Some(sequence),
            confirm: places.to_vec(),
            ..Self::default()
        }
    }

    /// Действие принято: подтверждение без поправок.
    fn accepted(sequence: i32) -> Self {
        Self {
            acknowledge: Some(sequence),
            ..Self::default()
        }
    }
}

/// Разбирает один пакет клиента: обновляет сведения об игроке, меняет мир и
/// пишет в лог то, что стоит показать.
fn handle_client_packet(
    packet: &PacketInfo,
    player: &mut PlayerState,
    shared: &Shared,
) -> Reaction {
    let before = (player.x, player.y, player.z, player.yaw, player.pitch);
    let reaction = handle_client_packet_inner(packet, player, shared);

    // Бездействие (`setidletimeout`): движение и повороты считаются делом,
    // только если место или взгляд правда изменились; ответы на Keep Alive и
    // конец такта — нет.
    let passive = matches!(
        packet.id,
        SERVERBOUND_KEEP_ALIVE | CONFIRM_TELEPORTATION | SET_PLAYER_POSITION | SET_PLAYER_POSITION_AND_ROTATION | SET_PLAYER_ROTATION | SET_PLAYER_ON_GROUND | 0x0D
    );

    if !passive || before != (player.x, player.y, player.z, player.yaw, player.pitch) {
        player.idle = Instant::now();
    }

    reaction
}

fn handle_client_packet_inner(
    packet: &PacketInfo,
    player: &mut PlayerState,
    shared: &Shared,
) -> Reaction {
    let payload = packet.payload();
    let mut reaction = Reaction::default();

    match packet.id {
        SERVERBOUND_ABILITIES => {
            if let Some(flags)=payload.first() {
                player.creative_flying=player.game_mode == GAME_MODE_CREATIVE && flags & 2 != 0;
            }
        }
        SERVERBOUND_KEEP_ALIVE => {
            log_debug!("Play: клиент ответил на Keep Alive");
            player.answered = Instant::now();
        }
        CONFIRM_TELEPORTATION => {
            if Cursor::new(payload).varint() == Some(player.teleport) {
                player.awaiting_teleport = false;

                // Перенесённый командой встанет на блок под ногами через
                // `players::SETTLE_TICKS` тактов.
                shared.players.lock().expect("список игроков захвачен другим потоком").confirm_teleport(&player.uuid);
            }
        }
        SET_PLAYER_POSITION | SET_PLAYER_POSITION_AND_ROTATION => {
            let Some((x, y, z)) = read_position(payload) else {
                log_debug!(
                    "Play: пакет о перемещении короче ожидаемого — длина тела {} байт",
                    packet.length
                );
                return reaction;
            };

            // Посланное до подтверждения переноса — со старого места.
            if player.awaiting_teleport {
                return reaction;
            }

            // Едущий сидит там, куда его посадили: место ведёт транспорт.
            if player.ride.vehicle.is_some() {
                return reaction;
            }

            // Как у оригинала: не число, бесконечность и место за границей
            // мира не принимаются — они ушли бы в файл игрока и всем
            // остальным, а огромные координаты переполняли бы счёт чанков.
            let sane = |value: f64, limit: f64| value.is_finite() && value.abs() <= limit;
            if !(sane(x, 30_000_000.0) && sane(y, 20_000_000.0) && sane(z, 30_000_000.0)) {
                log_debug!("Play: {} прислал недопустимое место {} {} {}", player.name, x, y, z);
                return reaction;
            }

            // В пакете «позиция и поворот» углы идут сразу за координатами.
            if packet.id == SET_PLAYER_POSITION_AND_ROTATION
                && let Some((yaw, pitch)) = read_rotation(payload, POSITION_BYTES)
            {
                player.set_rotation(yaw, pitch);
            }

            // Поршень сдвинул игрока, а клиент прислал место заметно
            // иное: оригинал на ближайший пакет места отвечает сверкой —
            // возвращает игрока туда, куда его сдвинули (piston-push-player:
            // робот, не знающий о толчке, получил сверку на (4,31; 0; 3,5)
            // в ответ на свой очередной пакет места). Сдвиг, который клиент
            // посчитал сам, принимается. Допуск 0,0625 — как у оригинала на
            // «moved wrongly» [проверить].
            if let Some(at) = player.shoved_to.take() {
                let off = (x - at.0).powi(2) + (y - at.1).powi(2) + (z - at.2).powi(2);

                if off > SHOVE_TOLERANCE * SHOVE_TOLERANCE {
                    reaction.resync = true;
                    return reaction;
                }
            }

            // Спящий стоит на месте кровати: его движение не принимается, а
            // ушедшего дальше блока (квадрат расстояния больше 1) оригинал
            // возвращает сверкой (чёрный ящик, bed-sleep-night: робот, не
            // знающий о сне, на свой ежесекундный пакет места получил сверку
            // на (3,5; 0,688; 4,5)).
            if player.sleeping.is_some() {
                let off = (x - player.x).powi(2) + (y - player.y).powi(2) + (z - player.z).powi(2);

                reaction.resync = off > 1.0;
                return reaction;
            }

            // Лодка для игрока — твёрдое тело: шаг, после которого его ящик
            // вошёл в лодку, где не был, не принимается — сверка обратно
            // (boat-ride: слезший стоит на верху лодки, робот, не знающий
            // о ней, падает в воду, а оригинал через такт возвращает его
            // на (3,5; 2,018; 1,953)). [проверить]: допуск и прочие тела.
            if vehicle::into_boat(shared, player, (x, y, z)) {
                reaction.resync = true;
                return reaction;
            }

            let from = (player.x, player.y, player.z);

            player.moved_since_tp = true;
            player.set_position(x, y, z);

            // Признак «на земле» — в байте флагов после координат (и углов).
            let flags = if packet.id == SET_PLAYER_POSITION { POSITION_BYTES } else { POSITION_BYTES + ROTATION_BYTES };
            if let Some(&flag) = payload.get(flags) {
                player.set_on_ground(flag & ON_GROUND_FLAG != 0);
                // Бит 2 — упёрся в стену (нужен элитрам: удар о стену).
                player.glide.wall = flag & 0x02 != 0;
            }

            // Прыжок, бег и приземление считает здоровье (vitals.rs).
            vitals::moved(player, shared, from);
        }
        SET_PLAYER_ROTATION => {
            if let Some((yaw, pitch)) = read_rotation(payload, 0) {
                player.set_rotation(yaw, pitch);
            }

            // До подтверждения переноса клиент ещё на старом месте.
            if !player.awaiting_teleport
                && let Some(&flag) = payload.get(ROTATION_BYTES)
            {
                player.set_on_ground(flag & ON_GROUND_FLAG != 0);
            }
        }
        SET_PLAYER_ON_GROUND => {
            if !player.awaiting_teleport
                && let Some(&flag) = payload.first()
            {
                player.set_on_ground(flag & ON_GROUND_FLAG != 0);
                vitals::moved(player, shared, (player.x, player.y, player.z));
            }
        }
        0x34 => { gameplay::beacon_select(payload, player, shared); }
        vitals::PLAYER_COMMAND => {
            sleep::player_command(payload, player, shared);
            vitals::player_command(payload, player);
            glide::command(payload, player, shared);
            vehicle::mount_command(payload, player, shared);
            let mut command=Cursor::new(payload);
            if let (Some(_),Some(5))=(command.varint(),command.varint()) { reaction.open_cart=player.ride.vehicle; }
            horse::command(payload, player, shared, &mut reaction);
        }
        vitals::CLIENT_COMMAND => {
            // Действие 0 — «возродиться»; живому возрождаться незачем.
            // Действие 1 — экран статистики: ответ — Award Statistics.
            match Cursor::new(payload).varint() {
                Some(0) if player.vit.dead => player.net.respawn = true,
                Some(1) => player.progress.stats_requested = true,
                _ => {}
            }
        }
        crate::recipe_book::CLIENT_SETTINGS => {
            let mut cursor = Cursor::new(payload);
            if let (Some(book), Some(open), Some(filter)) = (cursor.varint(), cursor.u8(), cursor.u8()) {
                player.progress.recipes.set_settings(book, open != 0, filter != 0);
            }
        }
        // Seen Advancements: 0 — открыта вкладка (с её названием), 1 — закрыто.
        crate::advancements::SEEN_ADVANCEMENTS => {
            let mut cursor = Cursor::new(payload);

            if cursor.varint() == Some(0)
                && let Some(tab) = cursor.string()
            {
                player.progress.adv.tab = Some(tab);
            }
        }
        // Разобрать пакет могло и не получиться: тогда отвечать не на что.
        PLAYER_ACTION => {
            // Статус 5 — «отпустил кнопку использования»: еда прервана.
            if Cursor::new(payload).varint() == Some(5) {
                vitals::stop_eating(player);
                vitals::release_use(player, shared);
            }

            if let Some(answer) = break_block(payload, player, shared) {
                reaction = answer;
            }
        }
        signs::SIGN_UPDATE => signs::update(payload, player, shared),
        USE_ITEM_ON => {
            if let Some(answer) = place_block(payload, player, shared) {
                reaction = answer;
            }
        }
        USE_ITEM => {
            if let Some(answer) = use_item(payload, player, shared) {
                reaction = answer;
            }
        }
        ATTACK => {
            if let Some(target) = Cursor::new(payload).varint() {
                attack(target, player, shared);
            }
        }
        USE_ENTITY => {
            let mut cursor = Cursor::new(payload);

            // Вторая рука пробует, только если основной делать нечего: у
            // вагонеток всё делает основная.
            if let Some(used) = horse::packet(payload, player, shared) {
                reaction = used;
            } else if let (Some(target), Some(0)) = (cursor.varint(), cursor.varint()) {
                reaction = use_entity(target, player, shared);
            }
        }
        SERVERBOUND_CHAT_MESSAGE => {
            // Поля после самого текста (время, подпись, счётчики) серверу пока
            // не нужны: подписи мы не проверяем, а текст стоит первым полем.
            let text = Cursor::new(payload).string();
            if let Some(text) = text.as_deref().and_then(crate::chat::allowed) {
                log_debug!("Play: {} пишет в чат: {}", player.name, text);

                // Плагины могут отменить сообщение или заменить текст
                // (plugins/): замков в этот миг мы не держим.
                let text = match crate::plugins::active().then(|| crate::plugins::chat(&plugin_view(player), text)) {
                    Some(crate::plugins::ChatOutcome::Cancel) => return reaction,
                    Some(crate::plugins::ChatOutcome::Replace(new)) => match crate::chat::allowed(&new) {
                        Some(new) => new.to_string(),
                        None => return reaction,
                    },
                    _ => text.to_string(),
                };

                shared
                    .chat
                    .lock()
                    .expect("чат захвачен другим потоком")
                    .push(format!("<{}> {}", player.name, text));
            }
        }
        SERVERBOUND_CHAT_COMMAND => {
            if let Some(command) = Cursor::new(payload).string() {
                log_debug!("{} ввёл команду: /{}", player.name, command);

                // Любая команда игрока — плагинам на отмену (plugins/).
                if crate::plugins::active() && crate::plugins::player_command(&plugin_view(player), &command) == crate::plugins::Verdict::Deny {
                    reaction.reply = Some("Команду отменил плагин".to_string());
                    return reaction;
                }

                let source = Source::Player {
                    uuid: player.uuid,
                    name: player.name.clone(),
                };
                let answer = commands::run(&command, &source, shared);

                if !answer.reply.is_empty() {
                    // Как у обычного сервера: «[Имя: что вышло]».
                    log_info!("[{}: {}]", player.name, answer.reply);
                    reaction.reply = Some(answer.reply);
                    reaction.reply_component = answer.component;
                }

                reaction.reply_rich = answer.rich;

                // Смена своего режима вступает в силу сразу, а клиенту о ней
                // скажет поручение Order::GameMode — одно, без повтора.
                if let Effect::GameMode { uuid, mode } = answer.effect
                    && uuid == player.uuid
                {
                    if player.game_mode != mode {
                        player.prev_mode = player.game_mode;
                    }

                    player.game_mode = mode;
                    player.creative_flying = false;
                }
            }
        }
        PROGRAM_COMMAND_BLOCK => program_command_block(payload, player, shared, &mut reaction),
        CHANGE_CONTAINER_SLOT_STATE => {
            let mut cursor = Cursor::new(payload);

            if let (Some(slot), Some(window), Some(enabled)) = (cursor.varint(), cursor.varint(), cursor.u8()) {
                crafter_slot_state(shared, player, window, slot, enabled != 0, &mut reaction);
            }
        }
        merchant::SELECT_TRADE => merchant::select_trade(shared, player, payload, &mut reaction),
        CLICK_CONTAINER_BUTTON => {
            let mut cursor = Cursor::new(payload);

            if let (Some(window), Some(button)) = (cursor.varint(), cursor.varint())
                && !station::button(shared, player, window, button, &mut reaction)
            {
                lectern_button(shared, player, window, button, &mut reaction);
            }
        }
        CLICK_CONTAINER if player.window.as_ref().is_some_and(|window| Cursor::new(payload).varint() == Some(window.id)) => {
            click_window(shared, player, payload, &mut reaction);
        }
        CLICK_CONTAINER => {
            // Что бы клиент ни нащёлкал, ответом идёт содержимое инвентаря
            // с сервера: если щелчок понят — новое, если нет — прежнее.
            // Так картинка у клиента не разойдётся с сервером.
            let click = read_click(payload, player);

            if !click.understood {
                log_debug!("Play: щелчок по инвентарю не разобран — вернули прежнее");
            }

            // Выброшенное падает на землю рядом с игроком.
            if let Some(stack) = click.thrown {
                player.progress.dropped(stack.item, stack.count);
                throw_out(shared, player, stack);
            }

            reaction.inventory = true;
        }
        RENAME_ITEM => {
            if let Some(name) = Cursor::new(payload).string() {
                station::rename(shared, player, name, &mut reaction);
            }
        }
        CLOSE_CONTAINER => {
            // Окно торговли: курсор и оплата — в инвентарь, поячеечно.
            let (slots, thrown) = merchant::close(shared, player);

            reaction.slots.extend(slots);

            for stack in thrown {
                throw_out(shared, player, stack);
            }

            // Закрыто окно хранилища — оно теряет смотрящего. Что менялось в
            // инвентаре мимо окна, клиент получит целиком.
            close_own_window(shared, player);

            if std::mem::take(&mut player.inventory_after_window) {
                reaction.inventory = true;
            }

            // Окно закрыто: то, что игрок держал мышкой и положил в сетку
            // верстака, надо вернуть в инвентарь, иначе предмет пропадёт.
            // Не влезло — падает на землю.
            let closed = player.inventory.close_window();

            for stack in closed.thrown {
                throw_out(shared, player, stack);
            }

            if !closed.slots.is_empty() {
                reaction.inventory = true;
            }
        }
        TAB_COMPLETE_REQUEST => {
            if let Some(answer) = suggestions(payload, shared, &player.uuid) {
                reaction.suggestions = Some(answer);
            }
        }
        PICK_ITEM_FROM_BLOCK => {
            if let Some(answer) = pick_block(payload, player, shared) {
                reaction = answer;
            }
        }
        vehicle::VEHICLE_MOVE => {
            if !horse::vehicle_move(payload, player, shared, &mut reaction) { vehicle::vehicle_move(payload, player, shared); }
        }
        vehicle::STEER_BOAT => vehicle::steer_boat(payload, player, shared),
        PLAYER_INPUT => {
            if let Some(flags) = payload.first() {
                vehicle::input(*flags,player,shared);
                horse::input(shared, player, *flags);
                let sneaking = flags & INPUT_SNEAK != 0;

                // Приседание в лодке или вагонетке — слезть (вики, Boat: «can
                // be exited by sneaking»).
                if sneaking && player.ride.vehicle.is_some() {
                    vehicle::dismount(shared, player);
                }

                // Остальным — через общий список: их подключения увидят
                // перемену и перескажут своим клиентам.
                if sneaking != player.sneaking {
                    player.sneaking = sneaking;
                    shared.players.lock().expect("список игроков захвачен другим потоком").set_sneaking(&player.uuid, sneaking);
                }
            }
        }
        CLIENT_INFORMATION => {
            // Игрок поменял настройки: возможно, включил или выключил
            // какую-то часть скина. Остальным это разошлётся само —
            // по сравнению с тем, что им уже показано.
            if let Some(look) = configuration::read_look(payload) {
                log_debug!(
                    "Play: {} сменил настройки: части скина {:#04X}, ведущая рука {}",
                    player.name,
                    look.skin_parts,
                    if look.main_hand == configuration::RIGHT_HAND {
                        "правая"
                    } else {
                        "левая"
                    }
                );

                player.skin_parts = look.skin_parts;
                player.main_hand = look.main_hand;
                player.view = view_for(shared, &look);

                let mut players = shared
                    .players
                    .lock()
                    .expect("список игроков захвачен другим потоком");

                players.set_skin_parts(&player.uuid, look.skin_parts);
                players.set_main_hand(&player.uuid, look.main_hand);
            }
        }
        SWING_ARM => {
            let now = *shared.ticks.borrow();

            shared.players.lock().expect("список игроков захвачен другим потоком").swing_at(&player.uuid, now);
        }
        SET_CARRIED_ITEM => {
            let mut cursor = Cursor::new(payload);

            if let Some(slot) = cursor.u16() {
                log_debug!(
                    "Play: игрок выбрал слот {} (байты: {})",
                    slot,
                    format_hex(payload)
                );
                player.inventory.select(slot as i32);

                // Сменил руку — щит опущен, лук спущен впустую.
                if player.net.using.is_some_and(|using| using.hand == 0) {
                    player.net.using = None;
                }
            }
        }
        SET_CREATIVE_MODE_SLOT => {
            // Слот −1 — предмет выброшен прямо из меню, мимо инвентаря.
            if let Some(thrown) = read_creative_slot(payload, player) {
                throw_out(shared, player, thrown);
            }
        }
        _ => {
            log_debug!(
                "Play: пакет клиента 0x{:02X} ({}) — длина тела {} байт, начало: {}",
                packet.id,
                packet_name(packet.id),
                packet.length,
                format_hex(&payload[..payload.len().min(24)])
            );
        }
    }

    reaction
}

/// Удар по сущности. Бить у нас можно только вагонетки: удар качает её,
/// а набравшая урона ломается. В выживании из сломанной выпадает предмет
/// вагонетки и её содержимое, в творческом — ничего (вики, Minecart:
/// «Breaking a minecart drops it as an item»). [проверить]: число ударов —
/// зонд minecart-hit.
fn attack(target: i32, player: &mut PlayerState, shared: &Shared) {
    if player.game_mode == GAME_MODE_SPECTATOR {
        return;
    }

    // Рамка: удар вынимает предмет или ломает её, без звука удара (чёрный
    // ящик, item-frame-rotate: только звуки рамки).
    if strike_frame(shared, player.dimension, target, (player.x, player.y, player.z), player.game_mode == GAME_MODE_CREATIVE) {
        return;
    }

    let now = *shared.ticks.borrow();
    let since = u32::try_from(now.saturating_sub(player.last_attack)).unwrap_or(u32::MAX);

    // Атрибуты (урон и скорость удара) — по вещам в руках и на теле сейчас.
    vitals::update_equipment(player, now);

    // Игрок: бой игроков (pvp.rs).
    if pvp::strike_player(shared, player, target, now) {
        return;
    }

    // Животное: удар ранит, толкает вперёд по взгляду и пугает.
    if strike_mob(shared, player, target, since) {
        player.last_attack = now;
        return;
    }

    // Мечом при полной руке, стоя и не на бегу — взмах, как по мобу
    // (чёрный ящик, boat-break).
    let sword = player.inventory.held().and_then(|stack| blocks::item_name(stack.item)).is_some_and(|name| name.ends_with("_sword"));
    let sweep = (sword && crate::minecart::hand_charge(since) > 0.9 && player.on_ground && !player.vit.sprinting).then_some(player.yaw);

    if strike_cart(shared, player.dimension, target, (player.x, player.y, player.z), player.game_mode == GAME_MODE_CREATIVE, since, player.attrs.value(crate::attributes::ATTACK_DAMAGE) as f32, sweep) {
        player.last_attack = now;
    }
}

/// Удар игрока по животному `target`. Урон — оружия в руке, ослабленный
/// неполным зарядом руки (`mob::charged`); толчок 0,4 вперёд по взгляду
/// (вики, Knockback); дальше 6 блоков не достаёт [проверить: у оригинала
/// 3 блока в выживании, 5 в творческом].
fn strike_mob(shared: &Shared, player: &mut PlayerState, target: i32, since: u32) -> bool {
    /// Дальше этого ударить нельзя (с запасом на задержку).
    const REACH: f64 = 6.0;

    // Удар по огненному шару гаста отбивает его по взгляду, и он становится
    // шаром ударившего (вики, Fireball: «can be deflected»).
    {
        let mut thrown = shared.thrown.lock().expect("снаряды захвачены другим потоком");

        if let Some(ball) = thrown.flying.iter_mut().find(|ball| ball.entity_id == target && ball.dimension == player.dimension && ball.kind == crate::thrown::Kind::GhastFireball) {
            let (yaw, pitch) = (f64::from(player.yaw).to_radians(), f64::from(player.pitch).to_radians());
            let dir = (-yaw.sin() * pitch.cos(), -pitch.sin(), yaw.cos() * pitch.cos());

            if (ball.x - player.x).powi(2) + (ball.y - player.y).powi(2) + (ball.z - player.z).powi(2) <= REACH * REACH {
                ball.push = dir;
                (ball.vx, ball.vy, ball.vz) = (dir.0 * 0.1, dir.1 * 0.1, dir.2 * 0.1);
                ball.owner = player.entity_id;

                return true;
            }
        }
    }

    let mut smash_event: Option<(i32, i32, i32)> = None;
    let mut items = shared.items_in(player.dimension).lock().expect("предметы захвачены другим потоком");

    // Части дракона: номера следом за ним, голова — первая (commands.rs,
    // summon_mob) [проверить]. Удар по самому дракону — как по телу.
    let is_part = |mob: &crate::mob::Mob| {
        mob.kind == crate::mob::Kind::EnderDragon && target > mob.entity_id && target <= mob.entity_id + crate::mob::DRAGON_PARTS
    };
    let Some(mob) = items.mobs_mut().iter_mut().find(|mob| mob.entity_id == target || is_part(mob)) else {
        return false;
    };
    let head = target == mob.entity_id + 1;
    let reach = REACH + mob.size().0 * 0.5;

    if (mob.x - player.x).powi(2) + (mob.y - player.y).powi(2) + (mob.z - player.z).powi(2) > reach * reach {
        return false;
    }

    // Кристалл Края: любой удар — взрыв силой 6 в его точке, сам он исчезает;
    // следом звук удара (чёрный ящик, end-crystal-explode: «убран», взрыв,
    // attack.strong — в один такт).
    if mob.kind == crate::mob::Kind::EndCrystal {
        let crystal_id = mob.entity_id;

        drop(items);
        detonate_crystal(shared, player.dimension, crystal_id, player.entity_id, (player.x, player.y, player.z));

        return true;
    }

    let held_stack = player.inventory.held();
    let held = held_stack.and_then(|stack| blocks::item_name(stack.item));
    // Урон и скорость удара — из атрибутов игрока (предмет в руке даёт им
    // модификаторы), зачарования — из довесков (вики, Melee attack).
    let data = held_stack.map(|stack| stack.extras()).unwrap_or_default();
    let charge = crate::combat::charge(since, player.attrs.value(crate::attributes::ATTACK_SPEED));
    // Критический удар (вики, Critical hit): полная рука, игрок падает —
    // не на земле, не бежит, не в воде; урон ×1,5 (чёрный ящик, cow-crit).
    let crit_ok = pvp::may_crit(player);
    let (undead, arthropod) = crate::combat::mob_traits(mob.kind.name());
    // Сокрушающий удар булавы — как по игроку (pvp::strike_player): прибавка
    // за пролёт в основу, вид урона mace_smash, пролёт гасится [проверить
    // по мобу: снято только по игроку, pvp-mace].
    let smash = pvp::may_smash(player, held == Some("mace"));
    let smash_fall = player.vit.fall;
    let bonus = if smash { crate::combat::smash_bonus(smash_fall, data.level("density")) } else { 0.0 };
    let source = if smash { crate::survival::Source::MaceSmash } else { crate::survival::Source::PlayerAttack };
    let enchanted = crate::combat::enchant_bonus(&data, undead, arthropod) + crate::combat::impaling_bonus(&data, mob.kind.name());
    let blow = crate::combat::strike(player.attrs.value(crate::attributes::ATTACK_DAMAGE), bonus, enchanted, charge, crit_ok);
    let (damage, crit) = (blow.damage, blow.crit);
    // Рывок (бег при полном заряде) и зачарование «отдача» толкают ещё, вдоль
    // взгляда; рывок сбрасывает бег (вики, Knockback).
    let sprint_hit = player.vit.sprinting && blow.full;
    let extra = crate::combat::extra_knockback(player.attrs.value(crate::attributes::ATTACK_KNOCKBACK), &data, sprint_hit);
    let look = crate::mob::look_direction(player.yaw);
    // Толчок — прочь от ударившего, по линии между ними (чёрный ящик,
    // cow-hit: корова в +z от игрока летит в +z, куда бы тот ни смотрел).
    let (away_x, away_z) = (mob.x - player.x, mob.z - player.z);
    let length = away_x.hypot(away_z);
    let dir = if length > 1.0e-6 { (away_x / length, away_z / length) } else { crate::mob::look_direction(player.yaw) };
    let push = crate::mob::Push { dir, strength: crate::mob::BASE_KNOCKBACK, cause: player.entity_id };
    mob.credit_player();
    mob.looting = data.level("looting");

    // Звук рывка — до удара (чёрный ящик, pvp-sprint-knockback).
    if sprint_hit {
        mob.events.push(crate::mob::Event::PlayerSound { name: "minecraft:entity.player.attack.knockback", at: (player.x, player.y, player.z) });
    }

    // Шипы стража: ударившего вплотную колет раньше, чем страж получит урон
    // (вики, Guardian, «Spikes defense»; чёрный ящик, guardian-spikes);
    // ставятся в очередь, когда склад мобов отпущен (замки мира — до склада).
    let spikes = crate::mob::guardian::spikes_out(mob).then_some(((mob.x, mob.y, mob.z), mob.entity_id));

    if spikes.is_some() {
        mob.events.push(crate::mob::Event::PlayerSound { name: "minecraft:enchant.thorns.hit", at: (player.x, player.y, player.z) });
    }

    let hit = if mob.kind == crate::mob::Kind::EnderDragon {
        mob.hurt_part(damage, head, source, Some(push))
    } else {
        mob.hurt_burning(damage, source, Some(push), if held == Some("mace") { crate::combat::breach_scale(data.level("breach")) } else { 1.0 }, data.level("fire_aspect") * 80)
    };
    let landed = hit.is_some();
    if landed && data.level("bane_of_arthropods") > 0 {
        let mut rng = crate::mob::Rng::new(mob.ticks ^ shared.next_entity_id() as u64);
        if let Some(effect) = crate::combat::bane_effect(&data, mob.kind.name(), rng.unit() as f32) {
            mob.vitals.give_effect_shown(effect);
        }
    }

    if let Some(hit) = &hit {
        player.progress.damage_dealt(*hit);

        mob.credit_to(player.uuid);
        if let Some(stack) = player.inventory.held() { player.progress.used_weapon(stack.item); }
        if smash && damage >= 100.0 { player.progress.trigger("adventure/overoverkill"); }
    }

    if smash && landed {
        player.vit.fall = 0.0;
        player.vit.smash_forgives = true;
        player.net.velocity = Some((0.0, 0.01, 0.0));
    }

    if hit.is_some() && extra > 0.0 {
        mob.knockback(look.0, look.1, f64::from(extra) * 0.5);
    }

    if hit.is_some() && sprint_hit {
        player.vit.sprinting = false;
    }

    let main = pvp::MainTarget { entity_id: mob.entity_id, feet: (mob.x, mob.y, mob.z), size: mob.size() };
    let mut sweeping = None;
    let center = (mob.x, mob.y + mob.size().1 * 0.5, mob.z);
    let from = (player.x, player.y, player.z);

    // Что слышно и видно от удара, идёт следом за уроном, в порядке журнала
    // (чёрный ящик, cow-hit): без урона — «пустой удар»; мечом при полной
    // руке — взмах (звук и частица перед игроком, по взгляду), иначе
    // сильный или слабый удар; при любом уроне — частицы «урон» (число —
    // половина снятого здоровья).
    match hit {
        None => mob.events.push(crate::mob::Event::PlayerSound { name: "minecraft:entity.player.attack.nodamage", at: from }),
        Some(hit) => {
            let sweep = !crit && !sprint_hit && blow.full && player.on_ground && held.is_some_and(|name| name.ends_with("_sword"));
            let rad = f64::from(player.yaw).to_radians();

            if crit {
                mob.events.push(crate::mob::Event::PlayerSound { name: "minecraft:entity.player.attack.crit", at: from });
                mob.events.push(crate::mob::Event::Animation(4));
            } else if sweep {
                sweeping = Some(pvp::sweep_damage(player.attrs.value(crate::attributes::ATTACK_DAMAGE), &data));
                mob.events.push(crate::mob::Event::PlayerSound { name: "minecraft:entity.player.attack.sweep", at: from });
                mob.events.push(crate::mob::Event::Particle {
                    kind: crate::mob::Particle::SweepAttack,
                    at: (from.0 - rad.sin(), from.1 + 0.9, from.2 + rad.cos()),
                    offset: ((-rad.sin()) as f32, 0.0, rad.cos() as f32),
                    speed: 0.0,
                    count: 0,
                });
            } else {
                let name = if charge > 0.9 { "minecraft:entity.player.attack.strong" } else { "minecraft:entity.player.attack.weak" };

                mob.events.push(crate::mob::Event::PlayerSound { name, at: from });
            }

            // Сокрушающий удар: звук у нападающего после крит-анимации
            // (чёрный ящик, pvp-mace). Частицы из блока под мобом (событие
            // мира 2013) — через журнал мира [проверить].
            if smash {
                mob.events.push(crate::mob::Event::PlayerSound { name: crate::combat::smash_sound(smash_fall, mob.on_ground), at: from });

                // Частицы из блока под мобом — событие мира 2013, как у игрока
                // (чёрный ящик, itemaudit4-density); шлётся после сброса мобов.
                if mob.on_ground {
                    smash_event = Some((mob.x.floor() as i32, mob.y.floor() as i32 - 1, mob.z.floor() as i32));
                }
            }

            if data.level("sharpness") > 0 || data.level("smite") > 0 || data.level("bane_of_arthropods") > 0 {
                mob.events.push(crate::mob::Event::Animation(5));
            }

            let count = (hit.taken * 0.5) as i32;

            if count > 0 {
                mob.events.push(crate::mob::Event::Particle {
                    kind: crate::mob::Particle::DamageIndicator,
                    at: center,
                    offset: (0.1, 0.0, 0.1),
                    speed: 0.2,
                    count,
                });
            }
        }
    }

    // Взмах меча задевает и других мобов рядом (вики, Sweep attack).
    if let Some(damage) = sweeping {
        pvp::sweep_mobs(&mut items, player, &main, damage);
    }

    items.flush_mob_events(&mut || shared.next_entity_id(), &mut crate::mob::Rng::new(u64::from(shared.next_entity_id() as u32)));
    drop(items);

    if let Some((x, y, z)) = smash_event {
        shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком").note_level_event(x, y, z, 2013, crate::combat::SMASH_PARTICLES);
    }

    if let Some((at, guardian)) = spikes {
        pvp::guardian_spikes(shared, player, at, guardian);
    }

    // Оружие изнашивается и об мобов, как об игроков (pvp::strike_player):
    // износ меняет вещь в руке, и её атрибуты уходят клиенту заново
    // (чёрный ящик, cow-xp: скорость удара на +1 после каждого удара).
    if landed
        && let Some(stack) = held_stack
        && let Some(info) = crate::combat::info(stack.item)
        && info.max_damage > 0
        && info.damage_per_attack > 0
        && player.game_mode != GAME_MODE_CREATIVE
    {
        pvp::wear_held(shared, player, stack, info.damage_per_attack, false);
    }

    true
}

/// Взрыв кристалла Края от удара игрока (tick::blow_crystal); звук удара —
/// после взрыва (чёрный ящик, end-crystal-explode). Виновник — ударивший.
pub(crate) fn detonate_crystal(shared: &Shared, dimension: crate::dimension::Dimension, crystal_id: i32, culprit: i32, from: (f64, f64, f64)) {
    let players = crate::tick::blast_targets(shared, dimension);
    let mut world = shared.world_in(dimension).lock().expect("мир захвачен другим потоком");
    let mut items = shared.items_in(dimension).lock().expect("предметы захвачены другим потоком");
    let mut next_id = || shared.next_entity_id();

    if !crate::tick::blow_crystal(&mut world, &mut items, &players, crystal_id, culprit, &mut next_id) {
        return;
    }

    world.note_player_sound(from, "minecraft:entity.player.attack.strong");
    items.flush_mob_events(&mut next_id, &mut crate::mob::Rng::new(u64::from(crystal_id as u32)));
}

/// Удар игрока в `from` по рамке `target`. Возвращает, была ли такая рамка
/// в досягаемости.
fn strike_frame(shared: &Shared, dimension: crate::dimension::Dimension, target: i32, from: (f64, f64, f64), creative: bool) -> bool {
    /// Дальше этого ударить нельзя (как у вагонетки).
    const REACH: f64 = 6.0;

    let mut world = shared.world_in(dimension).lock().expect("мир захвачен другим потоком");
    let mut items = shared.items_in(dimension).lock().expect("предметы захвачены другим потоком");

    let Some(frame) = items.frames().iter().find(|frame| frame.entity_id == target) else {
        return false;
    };
    let (x, y, z) = frame.place();

    if (x - from.0).powi(2) + (y - from.1).powi(2) + (z - from.2).powi(2) > REACH * REACH {
        return false;
    }

    crate::frame::strike(&mut world, &mut items, shared, target, creative)
}

/// Удар игрока в `from` по вагонетке `target`: `since` — сколько тактов
/// прошло с его прошлого удара. Общий для Java и Bedrock. Возвращает, был
/// ли удар (вагонетка есть и достаётся рукой). `sweep` — взгляд игрока,
/// если удар мечом размашистый.
#[allow(clippy::too_many_arguments)]
pub(crate) fn strike_cart(shared: &Shared, dimension: crate::dimension::Dimension, target: i32, from: (f64, f64, f64), creative: bool, since: u32, attack: f32, sweep: Option<f32>) -> bool {
    /// Дальше этого ударить нельзя (вики, Player: досягаемость сущностей 3
    /// блока, у творческого — 5; с запасом на задержку).
    const REACH: f64 = 6.0;

    let mut items = shared.items_in(dimension).lock().expect("предметы захвачены другим потоком");

    let Some(cart) = items.carts_mut().iter_mut().find(|cart| cart.entity_id == target) else {
        return false;
    };

    if (cart.x - from.0).powi(2) + (cart.y - from.1).powi(2) + (cart.z - from.2).powi(2) > REACH * REACH {
        return false;
    }

    let broken = cart.hit(creative, since, attack);

    // Удар слышен всем: сильный при полной руке, иначе слабый (зонд
    // minecart-hit: удар через 4 такта — уже слабый).
    let sound = if sweep.is_some() {
        "minecraft:entity.player.attack.sweep"
    } else if crate::minecart::hand_charge(since) > 0.9 {
        "minecraft:entity.player.attack.strong"
    } else {
        "minecraft:entity.player.attack.weak"
    };

    if broken {
        // Седоки слезают (riding.rs): подключения сами заметят и перенесут их.
        shared.riding.lock().expect("учёт седоков захвачен другим потоком").clear_vehicle(target);

        for broken in items.remove_carts(|cart| cart.entity_id == target) {
            if creative {
                continue;
            }

            let at = (broken.x, broken.y, broken.z);

            for stack in broken.drops() {
                items.drop_item(shared.next_entity_id(), stack, at, (0.0, 0.0, 0.0), items::MINED_DELAY);
            }
        }
    }

    drop(items);
    let mut world = shared.world_in(dimension).lock().expect("мир захвачен другим потоком");

    world.note_player_sound(from, sound);

    // Взмах: частица перед игроком по взгляду, как при ударе по мобу.
    // Задеть соседей взмахом пока не умеем. [проверить]
    if let Some(yaw) = sweep {
        let rad = f64::from(yaw).to_radians();

        world.note_particle(
            crate::mob::Particle::SweepAttack.id(),
            None,
            (from.0 - rad.sin(), from.1 + 0.9, from.2 + rad.cos()),
            ((-rad.sin()) as f32, 0.0, rad.cos() as f32),
            0.0,
            0,
        );
    }

    true
}

/// Щелчок по сущности. Щёлкать у нас можно только по вагонеткам: с
/// сундуком и воронкой открывают окно (вики, Minecart with Chest: «can be
/// accessed by pressing use»), печь заправляют углём.
fn use_entity(target: i32, player: &mut PlayerState, shared: &Shared) -> Reaction {
    /// Дальше этого щёлкнуть нельзя (как у удара).
    const REACH: f64 = 6.0;

    let mut reaction = Reaction::default();

    if player.game_mode == GAME_MODE_SPECTATOR {
        return reaction;
    }

    // Рамка: с предметом в руке кладёт его, по полной поворачивает.
    {
        let mut world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");
        let mut items = shared.items_in(player.dimension).lock().expect("предметы захвачены другим потоком");

        if let Some(frame) = items.frames().iter().find(|frame| frame.entity_id == target) {
            let (x, y, z) = frame.place();

            // В приключении рамку не тронуть. [проверить]
            if (x - player.x).powi(2) + (y - player.y).powi(2) + (z - player.z).powi(2) > REACH * REACH || player.game_mode == GAME_MODE_ADVENTURE {
                return reaction;
            }

            let used = crate::frame::use_on(&mut world, &mut items, target, player.inventory.held());

            if used == crate::frame::Used::Added
                && player.game_mode != GAME_MODE_CREATIVE
                && let Some(slot) = player.inventory.spend_held()
            {
                reaction.slots.push(slot);
            }

            return reaction;
        }
    }

    if use_mob(shared, player, target, &mut reaction) {
        return reaction;
    }

    // Лодка и пустая вагонетка: сесть (riding.rs).
    if vehicle::mount(shared, player, target) {
        return reaction;
    }

    let mut items = shared.items_in(player.dimension).lock().expect("предметы захвачены другим потоком");

    let Some(cart) = items.carts_mut().iter_mut().find(|cart| cart.entity_id == target) else {
        return reaction;
    };

    if (cart.x - player.x).powi(2) + (cart.y - player.y).powi(2) + (cart.z - player.z).powi(2) > REACH * REACH {
        return reaction;
    }

    match cart.kind {
        crate::minecart::Kind::Chest | crate::minecart::Kind::Hopper => reaction.open_cart = Some(target),
        crate::minecart::Kind::Boat(index) if crate::minecart::BOATS[index as usize].2 => reaction.open_cart = Some(target),
        crate::minecart::Kind::Furnace => {
            let coal = player.inventory.held().is_some_and(|stack| crate::minecart::is_furnace_fuel(stack.item));

            if cart.refuel((player.x, player.z), coal)
                && player.game_mode != GAME_MODE_CREATIVE
                && let Some(slot) = player.inventory.spend_held()
            {
                reaction.slots.push(slot);
            }
        }
        _ => {}
    }

    reaction
}

/// Щелчок по животному: ножницы стригут овцу, любимая еда вводит взрослого
/// в любовь (а детёныша подращивает на десятую долю оставшегося) — вики,
/// Sheep («Shearing»), Breeding. Возвращает, было ли это животное.
fn use_mob(shared: &Shared, player: &mut PlayerState, target: i32, reaction: &mut Reaction) -> bool {
    /// Дальше этого щёлкнуть нельзя (как у удара).
    const REACH: f64 = 6.0;

    let mut world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");
    let mut items = shared.items_in(player.dimension).lock().expect("предметы захвачены другим потоком");

    let Some(index) = items.mobs().iter().position(|mob| mob.entity_id == target) else {
        return false;
    };

    let mob = &items.mobs()[index];

    // Житель: торговля или мотание головой (merchant.rs); в приключении тоже.
    if matches!(mob.kind, crate::mob::Kind::Villager | crate::mob::Kind::WanderingTrader)
        && (mob.x - player.x).powi(2) + (mob.y - player.y).powi(2) + (mob.z - player.z).powi(2) <= REACH * REACH
        && !mob.is_dead()
    {
        player.progress.custom("talked_to_villager", 1);
        reaction.open_merchant = merchant::use_villager(&mut items, index, player.entity_id, player.uuid, shared);
        return true;
    }

    if (mob.x - player.x).powi(2) + (mob.y - player.y).powi(2) + (mob.z - player.z).powi(2) > REACH * REACH
        || (player.game_mode == GAME_MODE_ADVENTURE && !crate::mob::horse::equine(mob.kind))
        || mob.is_dead()
    {
        return true;
    }

    if water::use_on_mob(&mut items, index, player, &mut world, reaction) { return true; }
    // Вики, Golden Dandelion: переключает возраст только у растущих малышей.
    // [Bedrock: потом] use/частицы; offhand маршрутизация use_entity [проверить].
    if let Some(held) = player.inventory.held()
        && blocks::item_name(held.item) == Some("golden_dandelion")
        && crate::mob::growth::eligible(items.mobs()[index].kind)
        && items.mobs()[index].is_baby()
    {
        if crate::mob::growth::toggle(&mut items.mobs_mut()[index]) {
            player.progress.used(held.item);
            if player.game_mode != GAME_MODE_CREATIVE && let Some(slot) = player.inventory.spend_held() { reaction.slots.push(slot); }
            items.flush_mob_events(&mut || shared.next_entity_id(), &mut crate::mob::Rng::new(u64::from(world.random_bits())));
        }
        return true;
    }
    if items.mobs()[index].age_locked && player.inventory.held().and_then(|s| blocks::item_name(s.item)).is_some_and(|food| items.mobs()[index].kind.likes(food)) { return true; }

    // Бирка, поводок, волк и кошка (play/tame.rs).
    if tame::use_on_mob(&mut items, index, player, shared, reaction) {
        return true;
    }

    if use_new_mob(shared, player, &mut items, index, reaction) { return true; }
    if land::use_on(&mut items, index, &world, player, shared, reaction) { return true; }
    if horse::interact(&mut items, index, player, shared, reaction, 0) { return true; }

    // Големы: слиток лечит железного, ножницы снимают тыкву со снежного.
    if use_golem(shared, player, &mut items, index, reaction) {
        return true;
    }

    let mob = &items.mobs()[index];
    let held = player.inventory.held().and_then(|stack| blocks::item_name(stack.item));
    let survival = player.game_mode != GAME_MODE_CREATIVE;
    let at = (mob.x, mob.y, mob.z);

    // Вики, Bogged: два гриба, один раз; износ ножниц — 1 [проверить]; грибы падают на высоте 1,99 от ног (чёрный ящик, bogged-shear).
    if held == Some("shears") && mob.kind == crate::mob::Kind::Bogged {
        let mut rng = crate::mob::Rng::new(u64::from(shared.next_entity_id() as u32));
        if let Some(drops) = crate::mob::skeleton::shear(&mut items.mobs_mut()[index], &mut rng) {
            if let Some(stack) = player.inventory.held() { player.progress.used(stack.item); }
            items.flush_mob_events(&mut || shared.next_entity_id(), &mut rng);
            for stack in drops {
                let velocity = (rng.unit() * 0.2 - 0.1, 0.2, rng.unit() * 0.2 - 0.1);
                items.drop_item(shared.next_entity_id(), stack, (at.0, at.1 + 1.99, at.2), velocity, items::MINED_DELAY);
            }
            // wear_slot берёт мир: отпускаем оба замка до износа.
            drop(items);
            drop(world);
            if survival && let Some(shears) = player.inventory.held() {
                pvp::wear_slot(shared, player, player.inventory.selected_slot(), shears, 1, false);
            }
        }
        return true;
    }

    // Ножницы: овца теряет шерсть — 1–3 куска своего цвета (вики, Sheep,
    // «Shearing»); звук привязан к овце, куски падают выше её спины, как
    // выпавшее из блока: вверх 0,2, вбок до 0,1 (чёрный ящик, sheep-shear).
    if held == Some("shears") && mob.kind == crate::mob::Kind::Sheep && !mob.sheared && !mob.is_baby() {
        let color = mob.color;
        let count = 1 + (world.random_unit() * 3.0) as i32;
        let wool = blocks::item_named(crate::mob::WOOL[usize::from(color) % 16]);

        if let Some(stack) = player.inventory.held() { player.progress.used(stack.item); }
        items.mobs_mut()[index].sheared = true;
        items.mobs_mut()[index].events.push(crate::mob::Event::EntitySound {
            name: "minecraft:entity.sheep.shear",
            category: SOUND_PLAYERS,
            volume: 1.0,
            pitch: 1.0,
        });

        let mut rng = crate::mob::Rng::new(u64::from(shared.next_entity_id() as u32));

        items.flush_mob_events(&mut || shared.next_entity_id(), &mut rng);

        if let Some(wool) = wool {
            for _ in 0..count {
                let velocity = (rng.unit() * 0.2 - 0.1, 0.2, rng.unit() * 0.2 - 0.1);

                items.drop_item(shared.next_entity_id(), Stack::new(wool, 1), (at.0, at.1 + 1.0, at.2), velocity, items::MINED_DELAY);
            }
        }

        return true;
    }

    // Золотое яблоко зомби-жителю под слабостью: начинается лечение, 3600–6000
    // тактов, яблоко съедено, слабость снята, дана сила (вики, Zombie Villager,
    // «Curing»; чёрный ящик, zombie-villager-cure: событие 16, свойство 19).
    if held == Some("golden_apple")
        && mob.kind == crate::mob::Kind::ZombieVillager
        && mob.villager.conversion == 0
        && mob.vitals.effects.iter().any(|effect| effect.kind == crate::survival::EffectKind::Weakness)
    {
        let difficulty = shared.difficulty.load(std::sync::atomic::Ordering::Relaxed);
        let mut rng = crate::mob::Rng::new(u64::from(shared.next_entity_id() as u32) ^ mob.ticks.wrapping_mul(0x9E37));
        let mob = &mut items.mobs_mut()[index];

        if let Some(stack) = player.inventory.held() { player.progress.used(stack.item); }
        mob.villager.conversion = 3600 + rng.below(2401) as i32;
        mob.villager.converter = Some(player.uuid);
        mob.vitals.remove_effects(Some(crate::survival::EffectKind::Weakness));
        mob.vitals.give_effect_shown(crate::survival::Effect {
            kind: crate::survival::EffectKind::Strength,
            amplifier: i32::from(difficulty.saturating_sub(1)).min(2),
            ticks_left: mob.villager.conversion,
            ambient: false,
            visible: true, show_icon: true,
        });
        mob.events.push(crate::mob::Event::Status(16));
        items.flush_mob_events(&mut || shared.next_entity_id(), &mut rng);

        if survival && let Some(slot) = player.inventory.spend_held() {
            reaction.slots.push(slot);
        }

        return true;
    }

    // Огниво поджигает крипера: запал пошёл, пока не взорвётся; звук огнива
    // привязан к нему (вики, Creeper: «flint and steel»).
    if held == Some("flint_and_steel") && mob.kind == crate::mob::Kind::Creeper && !mob.h.lit {
        let mob = &mut items.mobs_mut()[index];

        if let Some(stack) = player.inventory.held() { player.progress.used(stack.item); }
        mob.h.lit = true;
        mob.events.push(crate::mob::Event::EntitySound { name: "minecraft:item.flintandsteel.use", category: SOUND_PLAYERS, volume: 1.0, pitch: 1.0 });
        items.flush_mob_events(&mut || shared.next_entity_id(), &mut crate::mob::Rng::new(u64::from(shared.next_entity_id() as u32)));

        return true;
    }

    if mob.kind==crate::mob::Kind::Bee && let Some(effect)=held.and_then(crate::mob::wildlife::harmful_flower) {
        items.mobs_mut()[index].vitals.give_effect_shown(effect);
        if survival && let Some(slot)=player.inventory.spend_held() {reaction.slots.push(slot);}
        return true;
    }

    // Любимая еда: любовь у взрослого, готового к разведению.
    if let Some(food) = held
        && mob.kind.likes(food)
    {
        let mob = &mut items.mobs_mut()[index];

        if mob.is_baby() {
            if let Some(stack) = player.inventory.held() { player.progress.used(stack.item); }
            // Детёныш: рост ускоряется, а сердечек сервер не шлёт — частицы
            // рисует клиент (чёрный ящик, rabbit-feed, fox-feed).
            mob.age = (mob.age + (-mob.age as f32 * 0.1) as i32).min(0);
        } else if mob.age == 0 && mob.love == 0 {
            if let Some(stack) = player.inventory.held() { player.progress.used(stack.item); }
            mob.love = crate::mob::LOVE_TICKS;
            mob.love_player = Some(player.uuid);
            mob.wild.breeder = Some(player.uuid);
            mob.events.push(crate::mob::Event::Hearts);
        } else {
            return true;
        }

        items.flush_mob_events(&mut || shared.next_entity_id(), &mut crate::mob::Rng::new(u64::from(shared.next_entity_id() as u32)));

        if survival && food == "tropical_fish_bucket" {
            let slot = inventory::FIRST_HOTBAR + player.inventory.selected();
            player.inventory.set(slot, blocks::item_named("water_bucket").map(|item| Stack::new(item, 1)));
            reaction.slots.push(slot);
        } else if survival && let Some(slot) = player.inventory.spend_held() {
            reaction.slots.push(slot);
        }

        return true;
    }

    true
}

/// Щелчок по голему. Железный слиток раненому железному — +25 здоровья, звук
/// починки, слиток тратится; здоровому — ничего (вики, Iron Golem; чёрный
/// ящик, iron-golem-heal). Ножницы снежному с тыквой — тыква падает с высоты
/// глаз 1,7, звук стрижки (snow-golem-shear).
fn use_golem(shared: &Shared, player: &mut PlayerState, items: &mut items::Items, index: usize, reaction: &mut Reaction) -> bool {
    let mob = &items.mobs()[index];
    let held = player.inventory.held().and_then(|stack| blocks::item_name(stack.item));
    let mut rng = crate::mob::Rng::new(u64::from(shared.next_entity_id() as u32) ^ mob.ticks.wrapping_mul(0x9E37));

    if mob.kind == crate::mob::Kind::IronGolem && held == Some("iron_ingot") {
        if mob.vitals.health >= mob.vitals.max_health {
            return false;
        }

        let mob = &mut items.mobs_mut()[index];
        let pitch = 1.0 + ((rng.unit() - rng.unit()) * 0.2) as f32;

        if let Some(stack) = player.inventory.held() { player.progress.used(stack.item); }
        mob.vitals.health = (mob.vitals.health + crate::mob::golem::INGOT_HEAL).min(mob.vitals.max_health);
        mob.events.push(crate::mob::Event::Sound { name: "minecraft:entity.iron_golem.repair", volume: 1.0, pitch });
        items.flush_mob_events(&mut || shared.next_entity_id(), &mut rng);

        if player.game_mode != GAME_MODE_CREATIVE && let Some(slot) = player.inventory.spend_held() {
            reaction.slots.push(slot);
        }

        return true;
    }

    if mob.kind == crate::mob::Kind::SnowGolem && held == Some("shears") && mob.golem.pumpkin {
        let at = (mob.x, mob.y + crate::mob::golem::SNOW_EYE, mob.z);
        let mob = &mut items.mobs_mut()[index];

        if let Some(stack) = player.inventory.held() { player.progress.used(stack.item); }
        mob.golem.pumpkin = false;
        mob.events.push(crate::mob::Event::EntitySound { name: "minecraft:entity.snow_golem.shear", category: SOUND_PLAYERS, volume: 1.0, pitch: 1.0 });
        items.flush_mob_events(&mut || shared.next_entity_id(), &mut rng);

        if let Some(pumpkin) = blocks::item_named("carved_pumpkin") {
            let velocity = (rng.unit() * 0.2 - 0.1, 0.2, rng.unit() * 0.2 - 0.1);

            items.drop_item(shared.next_entity_id(), Stack::new(pumpkin, 1), at, velocity, items::MINED_DELAY);
        }

        return true;
    }

    false
}

/// Обрабатывает Player Action: ломание блока.
///
/// Копание устроено по-разному в зависимости от режима игры. В творческом блок
/// исчезает мгновенно: клиент присылает только «начал копать» и сразу считает
/// блок сломанным, «докопал» он не присылает. В остальных режимах блок исчезает
/// только когда клиент докопал — время копания считает клиент, а «начал копать»
/// означает лишь начало.
///
/// Возвращает ответ клиенту: подтверждение, если блок действительно изменился.
fn break_block(payload: &[u8], player: &mut PlayerState, shared: &Shared) -> Option<Reaction> {
    let held = player.inventory.held();
    let mut mined = None;
    let result = break_block_inner(payload, player, shared, &mut mined);
    if player.game_mode == GAME_MODE_SURVIVAL && let Some(state) = mined {
        if let Some(stack) = held { pvp::wear_held(shared, player, stack, crate::mining::wear(state, stack), false); }
        player.vit.add_exhaustion(0.005);
    }
    result
}

fn break_block_inner(payload: &[u8], player: &mut PlayerState, shared: &Shared, mined: &mut Option<i32>) -> Option<Reaction> {
    /// Начал копать.
    const STARTED_DIGGING: i32 = 0;

    /// Выбросил всю стопку (клавиша выброса с Ctrl).
    const DROP_STACK: i32 = 3;

    /// Выбросил одну штуку (клавиша выброса, обычно Q).
    const DROP_ONE: i32 = 4;

    let mut cursor = Cursor::new(payload);

    let status = cursor.varint()?;
    let (x, y, z) = cursor.position()?;
    let face = cursor.u8()?;
    let sequence = cursor.varint()?;

    log_debug!(
        "Play: Player Action — статус {}, блок {} {} {} (игрок в {:.2} {:.2} {:.2}), грань {}, номер {}, байты: {}",
        status,
        x,
        y,
        z,
        player.x,
        player.y,
        player.z,
        face,
        sequence,
        format_hex(payload)
    );

    // Выброс предмета из руки приходит этим же пакетом: место в нём нулевое,
    // потому что бросают не в блок.
    if status == DROP_STACK || status == DROP_ONE {
        let Some(held) = player.inventory.held() else {
            return Some(Reaction::default());
        };

        let thrown = if status == DROP_STACK {
            player.inventory.set(
                inventory::FIRST_HOTBAR + player.inventory.selected(),
                None,
            );
            held
        } else {
            player.inventory.spend_held();
            held.with_count(1)
        };

        player.progress.dropped(thrown.item, thrown.count);
        throw_out(shared, player, thrown);

        return Some(Reaction {
            slots: vec![inventory::FIRST_HOTBAR + player.inventory.selected()],
            ..Reaction::default()
        });
    }

    // Подтверждение нужно на каждое действие с номером — и на «начал», и на
    // «отменил»: клиент держит у себя предсказанные блоки, пока не получит
    // его, и по нему же откатывает то, чего сервер не сделал.
    let answer = Reaction {
        confirm: vec![(x, y, z)],
        ..Reaction::accepted(sequence)
    };

    let creative = player.game_mode == GAME_MODE_CREATIVE;

    // В приключении и наблюдателем блоки не ломают.
    if !creative && player.game_mode != GAME_MODE_SURVIVAL {
        return Some(answer);
    }

    // Дальше, чем достаёт рука, и в незагруженных чанках — не ломаем.
    if !within_reach(player, (x, y, z)) {
        log_debug!("Play: {} тянется к {} {} {} слишком далеко", player.name, x, y, z);
        return Some(answer);
    }

    let state = {
        let world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");
        if !world.has_chunk(x.div_euclid(16), z.div_euclid(16)) {
            return Some(answer);
        }
        world.get_block(x, y, z)
    };

    if status == STARTED_DIGGING
        && let Some(reaction) = debug_stick::interact(shared, player, (x,y,z), 1, true, sequence)
    { return Some(reaction); }

    let breaks_now = mining::allows(player, shared, (x, y, z), state, status);

    // Удар по редстоуновой руде зажигает её (вики, Redstone Ore: «pressing
    // either the break or the use control»). В творческом блок ломается
    // сразу же, и зажигать нечего.
    if status == STARTED_DIGGING && !creative {
        let mut world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");

        redstone::punched(&mut world, (x, y, z));
        mining::punched(player, world.get_block(x, y, z));
        if blocks::block_at_state(state) == Some("note_block") { player.progress.custom("play_noteblock", 1); }
    }

    if !breaks_now {
        // В выживании оригинал на START/CANCEL и на преждевременный FINISH
        // блок не пересылает, только подтверждает (чёрный ящик mining-time-denied).
        return Some(if creative { answer } else { Reaction::accepted(sequence) });
    }

    // Бедрок, барьер, порталы в выживании не ломаются.
    if !creative && in_spans(UNBREAKABLE, state) {
        return Some(answer);
    }

    // Клиенту в этом доверять нельзя — он присылает «начал копать» и с мечом
    // тоже, поэтому решает сервер. Копьём блок не ломается ни в каком режиме,
    // а мечом, трезубцем, булавой и палкой отладки — только в творческом.
    if let Some(item) = player.inventory.held().map(|stack| stack.item)
        && !(blocks::can_break_at_all(item)
            && (player.game_mode != GAME_MODE_CREATIVE
                || blocks::can_break_in_creative(item)))
    {
        log_debug!(
            "Play: предмет {} блоки не ломает — блок в {} {} {} остаётся",
            item,
            x,
            y,
            z
        );
        return Some(Reaction::default());
    }

    // Плагины могут не дать сломать (plugins/): до яиц, чешуйниц,
    // износа инструмента и добычи, без замка мира. Подтверждение вернёт блок.
    if crate::plugins::active() {
        let block = format!("minecraft:{}", blocks::block_at_state(state).unwrap_or("air"));

        if crate::plugins::block_break(&plugin_view(player), (x, y, z), &block) == crate::plugins::Verdict::Deny {
            log_debug!("Play: плагин не дал {} сломать {} {} {}", player.name, x, y, z);
            return Some(answer);
        }
    }

    if blocks::block_at_state(state) == Some("turtle_egg") {
        let silk = player.inventory.held().is_some_and(|stack| stack.extras().level("silk_touch") > 0);
        let drop_egg = !creative && silk && shared.block_drops.load(std::sync::atomic::Ordering::Relaxed);
        let mut world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");
        if creative { world.set_block(x,y,z,world::AIR); }
        else { crate::mob::water::eggs::break_one(&mut world,(x,y,z)); }
        redstone::settle(&mut world);
        drop(world);
        if drop_egg && let Some(item) = blocks::item_named("turtle_egg") {
            drop_into_world(shared,player.dimension,Stack::new(item,1),(f64::from(x)+0.5,f64::from(y)+0.5,f64::from(z)+0.5),(0.0,0.2,0.0),items::MINED_DELAY);
        }
        return Some(answer);
    }

    // Заражённый блок: чешуйница выходит раньше, чем блок исчезает: у оригинала
    // сперва пакеты сущности, затем блок (чёрный ящик, silverfish-infested).
    // Silk Touch даёт обычный блок, в творческом и при doTileDrops=false не выходит никто.
    let infested = crate::mob::arthropod::host(state).is_some();
    let silk = infested && player.inventory.held().is_some_and(|stack| stack.extras().level("silk_touch") > 0);
    let drops = shared.block_drops.load(std::sync::atomic::Ordering::Relaxed);
    let release = infested && crate::mob::arthropod::releases(state, !creative, silk, drops);

    let released = release.then(|| crate::mob::arthropod::released(shared.next_entity_id(), (x, y, z)));

    if let Some(mob) = &released {
        shared.items_in(player.dimension).lock().expect("предметы захвачены другим потоком").add_mob_late(mob.clone());
    }

    let mut world = shared
        .world_in(player.dimension)
        .lock()
        .expect("мир захвачен другим потоком");

    if let Some(mob) = released {
        world.note_mob_spawn(mob);
    }

    // Растяжку, срезанную ножницами, сперва обезвреживают: крючки теряют
    // цепь без импульса (вики, String).
    if let Some(shears) = blocks::item_named("shears")
        && player.inventory.held().map(|stack| stack.item) == Some(shears)
    {
        redstone::disarm_tripwire(&mut world, (x, y, z));
    }

    // Воздух ломать нечего.
    // Особый динамит помнит свой вид до слома: выпадет он же.
    let custom = world.custom_tnt_at((x, y, z));

    // Кровать: что сказать о второй половине и откуда выпадет предмет
    // (play/sleep.rs).
    let bed = sleep::bed_break_plan(&world, (x, y, z), player.game_mode == GAME_MODE_CREATIVE);

    let hive_block = crate::hive::is_hive(state);
    let hive_silk = player.inventory.held().is_some_and(|stack| stack.extras().level("silk_touch") > 0);
    let hive_drop = if hive_block && hive_silk { crate::hive::silk_stack(&world, (x, y, z)) } else { None };
    if hive_block && !hive_silk {
        let mut items = shared.items_in(player.dimension).lock().expect("предметы захвачены другим потоком");
        crate::hive::disturb(&mut world, &mut items, (x, y, z), player.entity_id, &mut || shared.next_entity_id());
    }

    // Верхняя половина высокого растения: об исчезнувшей нижней оригинал в
    // выживании говорит событием слома 2001 (чёрный ящик, block3-large-fern,
    // block3-tall-seagrass); в творческом — нет (событие исключает самого игрока).
    let lower_half = {
        let here = world.get_block(x, y, z);
        let lower = world.get_block(x, y - 1, z);
        let tall = blocks::block_at_state(here).filter(|n| !n.ends_with("_door"))
            .is_some_and(|n| blocks::orientation_of(here).and_then(|b| b.value(here, "half")) == Some("upper")
                && blocks::block_at_state(lower) == Some(n));
        (tall && player.game_mode != GAME_MODE_CREATIVE).then_some(lower)
    };

    let actor = world.actor.replace(player.entity_id);
    let broken = placing::break_block(&mut world, x, y, z);
    world.actor = actor;
    let Some(broken) = broken else {
        log_debug!("Play: в {} {} {} и так пусто", x, y, z);
        // Оригинал на «начал копать» по воздуху в выживании блок не шлёт
        // (чёрный ящик mining-time-denied).
        return Some(if creative { answer } else { Reaction::accepted(sequence) });
    };

    *mined = Some(broken);

    if hive_block {
        let stack=if !creative && drops {
            hive_drop.or_else(|| (blocks::block_at_state(broken)==Some("beehive")).then(||blocks::item_named("beehive").map(|id|Stack::new(id,1))).flatten())
        } else { None };
        if !creative { player.progress.mined(broken); }
        let scattered=crate::tick::scattered((x,y,z),||f64::from(world.random_unit()));
        drop(world);
        if let Some(stack)=stack { drop_into_world(shared,player.dimension,stack,scattered.0,scattered.1,items::MINED_DELAY); }
        return Some(Reaction::accepted(sequence));
    }

    // Заражённый блок: что выпадает — Silk Touch даёт обычный блок.
    if crate::mob::arthropod::host(broken).is_some() {
        let stack = if !creative && silk && drops { crate::mob::arthropod::silk_drop(broken) } else { None };

        if !creative {
            player.progress.mined(broken);
        }

        // Предмет выпадает, как из любого блока: случайная точка и толчок
        // (чёрный ящик, silverfish-infested: предмет вылетает, робот его подбирает).
        let placed = stack.map(|stack| (stack, crate::tick::scattered((x, y, z), || f64::from(world.random_unit()))));

        drop(world);

        if let Some((stack, (at, velocity))) = placed {
            drop_into_world(shared, player.dimension, stack, at, velocity, items::MINED_DELAY);
        }

        return Some(Reaction::accepted(sequence));
    }

    if blocks::block_at_state(broken)==Some("creaking_heart") {
        if !creative && drops {
            let held=player.inventory.held();
            let silk=held.is_some_and(|stack|stack.extras().level("silk_touch")>0);
            let fortune=held.map_or(0,|stack|stack.extras().level("fortune"));
            let stack=crate::mob::newblocks::heart_drop(broken,silk,fortune,world.random_unit());
            if player.game_mode==GAME_MODE_SURVIVAL && blocks::orientation_of(broken).and_then(|b|b.value(broken,"natural"))==Some("true") {
                let xp=20+(world.random_unit()*5.0) as i32;
                player.net.new_orbs.push(((f64::from(x)+0.5,f64::from(y)+0.5,f64::from(z)+0.5),xp));
            }
            player.progress.mined(broken);
            drop(world);
            if let Some(stack)=stack {drop_into_world(shared,player.dimension,stack,(f64::from(x)+0.5,f64::from(y)+0.25,f64::from(z)+0.5),(0.0,0.0,0.0),items::MINED_DELAY);}
        }
        return Some(Reaction::accepted(sequence));
    }

    log_debug!("Play: сломан блок в {} {} {}", x, y, z);

    // Творческий ничего не «добывает» (вики, Statistics: BLOCK_MINED).
    if player.game_mode != GAME_MODE_CREATIVE {
        player.progress.mined(broken);
    }

    if let Some(lower) = lower_half {
        world.note_level_event(x, y - 1, z, 2001, lower);
    }

    if let Some(plan) = &bed {
        let (ex, ey, ez) = plan.event_at;

        world.note_level_event(ex, ey, ez, 2001, plan.event_state);
    }

    // Предмет кровати выпадает из изголовья, какую половину ни ломай
    // (чёрный ящик, bed-place-break).
    let (x, y, z) = bed.as_ref().map_or((x, y, z), |plan| plan.drop_at);

    // В выживании из сломанного блока выпадает предмет — он падает на землю
    // и лежит, пока его не подберут. В творческом режиме блоки просто
    // исчезают: предметы там берут из меню.
    if player.game_mode != GAME_MODE_CREATIVE {
        let tool = player.inventory.held().map(|stack| stack.item);
        let luck = world.random_unit();

        // Скалк с «шёлковым касанием» отдаёт блок вместо опыта (вики,
        // Sculk: «Silk Touch»). Прочие руды сохраняют прежний расчёт.
        let silk_sculk = player.inventory.held().is_some_and(|stack| stack.extras().level("silk_touch") > 0);
        let sculk_drop = redstone::sculk_family::mined_drop(broken, silk_sculk);
        let xp = if sculk_drop.is_some() && silk_sculk { 0 } else {
            blocks::block_at_state(broken).map_or(0, |name| crate::survival::ore_xp(name, world.random_unit()))
        };

        if xp > 0 && player.game_mode == GAME_MODE_SURVIVAL {
            player.net.new_orbs.push(((x as f64 + 0.5, y as f64 + 0.5, z as f64 + 0.5), xp));
        }

        // Посев даёт несколько стопок: урожай и семена (farming.rs).
        let fortune = player.inventory.held().map_or(0, |stack| stack.extras().level("fortune"));

        if let Some(drops) = crate::farming::harvest(&mut world, broken, fortune) {
            // Каждая стопка — в случайной точке блока со случайным толчком,
            // как у оригинала (чёрный ящик, farm-till-wheat).
            let placed: Vec<_> = drops.into_iter().map(|stack| (stack, crate::tick::scattered((x, y, z), || f64::from(world.random_unit())))).collect();

            drop(world);

            for (stack, (at, velocity)) in placed {
                drop_into_world(shared, player.dimension, stack, at, velocity, items::MINED_DELAY);
            }

            return Some(Reaction::accepted(sequence));
        }

        match sculk_drop.unwrap_or_else(|| blocks::drops_by_luck(broken, tool, luck)).map(|stack| crate::custom_tnt::tag_drop(stack, custom)) {
            Some(stack) => {
                let (at, velocity) = crate::tick::scattered((x, y, z), || f64::from(world.random_unit()));
                // Мир отпускаем: класть предмет будем под другим замком.
                drop(world);

                drop_into_world(
                    shared, player.dimension,
                    stack,
                    at,
                    velocity,
                    items::MINED_DELAY,
                );

                log_debug!("Play: из блока в {} {} {} выпал предмет {} × {}", x, y, z, stack.item, stack.count);
            }
            None => log_debug!("Play: из блока {} ничего не выпало", broken),
        }
    }

    Some(Reaction::accepted(sequence))
}

/// Лежит ли номер состояния в одном из промежутков таблицы.
fn in_spans(spans: &[(i32, i32)], state: i32) -> bool {
    spans.iter().any(|(low, high)| (*low..=*high).contains(&state))
}

/// Дотягивается ли игрок до блока: от глаз до ближайшей точки блока не
/// дальше дальности взаимодействия (4,5 блока, в творческом 5; вики,
/// Attribute: block_interaction_range) с запасом в блок. Мерка — до ящика
/// блока, а не до его середины: чёрный ящик, reach-edge — оригинал в
/// творческом принимает щелчок по рычагу с 5,96 блока до ящика (6,53 до
/// середины) и не принимает с 6,13 (6,69). [проверить] точную границу между
/// ними. Заодно отсекает места вне мира по высоте.
fn within_reach(player: &PlayerState, (x, y, z): (i32, i32, i32)) -> bool {
    const EYES: f64 = 1.62;

    if !(-64..320).contains(&y) {
        return false;
    }

    let reach = if player.game_mode == GAME_MODE_CREATIVE { 5.0 } else { 4.5 } + 1.0;

    // От глаз до ближайшей точки куба блока по каждой оси.
    let gap = |eye: f64, low: i32| (f64::from(low) - eye).max(eye - f64::from(low + 1)).max(0.0);
    let dx = gap(player.x, x);
    let dy = gap(player.y + EYES, y);
    let dz = gap(player.z, z);

    dx * dx + dy * dy + dz * dz < reach * reach
}

/// Use Item On второй рукой: клиент шлёт его, когда основной руке делать
/// нечего (факел во второй руке, пустая основная). Весь разбор написан для
/// основной руки, поэтому на время разбора вторая рука и выбранный слот
/// меняются местами, а номера изменённых слотов — обратно.
fn place_block(payload: &[u8], player: &mut PlayerState, shared: &Shared) -> Option<Reaction> {
    /// Вторая рука в поле `hand`.
    const OFF_HAND: i32 = 1;

    if Cursor::new(payload).varint()? != OFF_HAND {
        return place_with_main_hand(payload, player, shared);
    }

    let main = inventory::FIRST_HOTBAR + player.inventory.selected();
    let swap = |player: &mut PlayerState| {
        let held = player.inventory.slot(main);
        let off = player.inventory.slot(inventory::OFFHAND);
        player.inventory.set(main, off);
        player.inventory.set(inventory::OFFHAND, held);
    };

    swap(player);
    let mut reaction = place_with_main_hand(payload, player, shared);
    swap(player);

    if let Some(reaction) = &mut reaction {
        for slot in &mut reaction.slots {
            if *slot == main {
                *slot = inventory::OFFHAND;
            } else if *slot == inventory::OFFHAND {
                *slot = main;
            }
        }
    }

    reaction
}

/// Обрабатывает Use Item On: игрок щёлкнул по блоку предметом в руке.
///
/// Блок ставится не в тот блок, по которому щёлкнули, а в соседний — с той
/// стороны, по которой пришёлся щелчок. Каким состоянием — сервер считает сам:
/// клиент сообщает только грань и точку щелчка, а поворот блока зависит от
/// того, куда смотрит игрок. Правила — в placing.rs.
///
/// Возвращает ответ клиенту: подтверждение и, если блок не поставлен, —
/// настоящее состояние того места, куда он его ставил.
fn place_with_main_hand(payload: &[u8], player: &mut PlayerState, shared: &Shared) -> Option<Reaction> {
    let mut cursor = Cursor::new(payload);

    let hand = cursor.varint()?;
    let (clicked_x, clicked_y, clicked_z) = cursor.position()?;
    let face = cursor.varint()?;

    // Точка щелчка внутри блока: по высоте — какая половина блока
    // достанется ступени или плите, по всем трём — какая ячейка полки или
    // резного шкафа.
    let cursor_x = cursor.f32()?;
    let cursor_y = cursor.f32()?;
    let cursor_z = cursor.f32()?;
    let _inside_block = cursor.u8()?;

    let sequence = cursor.sequence()?;

    log_debug!(
        "Play: Use Item On — рука {}, блок {} {} {} (игрок в {:.2} {:.2} {:.2}), грань {}, номер {}, байты: {}",
        hand,
        clicked_x,
        clicked_y,
        clicked_z,
        player.x,
        player.y,
        player.z,
        face,
        sequence,
        format_hex(payload)
    );

    // Наблюдатель ничего не трогает; дальше руки и в незагруженных чанках
    // не ставим: иначе можно было бы менять мир издалека, а запись в
    // незагруженный чанк затёрла бы его на диске.
    let refused = Reaction {
        confirm: vec![(clicked_x, clicked_y, clicked_z)],
        ..Reaction::accepted(sequence)
    };
    let loaded = shared
        .world_in(player.dimension)
        .lock()
        .expect("мир захвачен другим потоком")
        .has_chunk(clicked_x.div_euclid(16), clicked_z.div_euclid(16));
    if player.game_mode == GAME_MODE_SPECTATOR || !loaded {
        log_debug!("Play: {} не может использовать {} {} {}", player.name, clicked_x, clicked_y, clicked_z);
        return Some(refused);
    }

    // Дальше руки оригинал щелчок молча пропускает: блок не повторяет
    // (чёрный ящик, rail-activator — рычаг в семи блоках).
    if !within_reach(player, (clicked_x, clicked_y, clicked_z)) {
        log_debug!("Play: {} тянется к {} {} {} слишком далеко", player.name, clicked_x, clicked_y, clicked_z);
        return Some(Reaction::accepted(sequence));
    }

    if let Some(reaction) = block_use::use_block(player, shared, (clicked_x, clicked_y, clicked_z), face, cursor_y, sequence) {
        return Some(reaction);
    }

    // Щелчок по рычагу, кнопке, двери, повторителю — это не установка, а
    // действие с ними, и рука тут ни при чём: щёлкают и пустой. Присев,
    // игрок всё-таки ставит блок.
    if !player.sneaking {
        let mut world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");

        // Командный блок: оператору в творческом режиме — его данные (окно
        // клиент открывает сам), потом повтор блока и за гранью (чёрный
        // ящик, cmdblock-impulse). Прочим щелчок идёт дальше, как по
        // обычному блоку.
        let clicked_state = world.get_block(clicked_x, clicked_y, clicked_z);

        if crate::command_block::mode_of(clicked_state).is_some()
            && crate::command_block::may_edit(shared, &player.uuid, player.game_mode)
            && let Some(data) = world.command_block(clicked_x, clicked_y, clicked_z)
        {
            let mut body = encode_varint(BLOCK_ENTITY_DATA);

            push_packed_position(&mut body, clicked_x, clicked_y, clicked_z);
            body.extend_from_slice(&encode_varint(crate::command_block::BLOCK_ENTITY));
            body.extend_from_slice(&data.network_nbt());

            let (bx, by, bz) = offset_towards(clicked_x, clicked_y, clicked_z, face);

            world.tell_directly(clicked_x, clicked_y, clicked_z);
            world.tell_directly(bx, by, bz);

            return Some(Reaction { packets: vec![body], ..Reaction::accepted(sequence) });
        }

        // Хранилище: окно открывается, если открыть можно. Порядок снят
        // с оригинала чёрным ящиком (chest-open): звук, окно, повтор самого
        // блока, повтор за гранью, и только потом — общая рассылка (бочка
        // «открыта», лампы у ловушки) и крышка в фазе блочных событий.
        let clicked = (clicked_x, clicked_y, clicked_z);
        let kind = containers::Kind::of(world.get_block(clicked_x, clicked_y, clicked_z));
        let (bx, by, bz) = offset_towards(clicked_x, clicked_y, clicked_z, face);

        // Око в рамку портала, светокамень в якорь, кусок торта. Порядок —
        // как у прочих щелчков (чёрный ящик, comparator-click): повтор
        // блока, повтор за гранью, потом общая рассылка.
        let held_name = player.inventory.held().and_then(|stack| blocks::item_name(stack.item));

        // Кровать: сон, точка возрождения, взрыв вне обычного мира; якорь:
        // точка возрождения в Незере, взрыв вне его (play/sleep.rs).
        if let Some(reaction) = sleep::use_bed(&mut world, shared, player, clicked, face, sequence) {
            return Some(reaction);
        }

        if let Some(reaction) = sleep::use_anchor(&mut world, shared, player, clicked, face, held_name, sequence) {
            return Some(reaction);
        }

        // Забор с мобами на поводке: узел (play/tame.rs).
        if let Some(reaction) = tame::use_fence(&mut world, shared, player, clicked, face, sequence) {
            return Some(reaction);
        }

        if let Some(spend) = redstone::use_with_item(&mut world, clicked, held_name, player.game_mode == GAME_MODE_CREATIVE) {
            if blocks::block_at_state(clicked_state) == Some("respawn_anchor") && spend
                && crate::bed::anchor_charges(world.get_block(clicked_x, clicked_y, clicked_z)) == Some(4)
            {
                player.progress.trigger("nether/charge_respawn_anchor");
            }
            if blocks::block_at_state(clicked_state) == Some("cake") { player.progress.custom("eat_cake_slice", 1); }
            world.tell_directly(clicked_x, clicked_y, clicked_z);
            world.tell_directly(bx, by, bz);
            redstone::settle(&mut world);
            drop(world);

            let slots = if spend { spend_placed(player) } else { Vec::new() };

            return Some(Reaction { slots, ..Reaction::accepted(sequence) });
        }

        // Проигрыватель: вставить пластинку из руки или выбросить ту, что
        // внутри. Окна у него нет; пустой и без пластинки в руке — щелчок
        // идёт дальше, как по обычному блоку.
        if kind == Some(containers::Kind::Jukebox)
            && let Some(took) = redstone::use_jukebox(&mut world, clicked, player.inventory.held())
        {
            world.tell_directly(clicked_x, clicked_y, clicked_z);
            world.tell_directly(bx, by, bz);
            drop(world);

            if took {
                player.progress.custom("play_record", 1);
                if shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком").mob_biome(clicked_x, clicked_y, clicked_z) == "meadow" {
                    player.progress.trigger("adventure/play_jukebox_in_meadows");
                }
            }
            let slots = if took { spend_placed(player) } else { Vec::new() };

            return Some(Reaction { slots, ..Reaction::accepted(sequence) });
        }

        // Яйцо призыва по спавнеру меняет его вид; яйцо тратится в выживании
        // (вики, Monster Spawner; чёрный ящик, spawner-egg: блок, блок за гранью,
        // блок, данные блока, ячейка).
        if let Some(entity) = player.inventory.held().and_then(|stack| blocks::item_name(stack.item)).and_then(crate::spawner::egg_entity).map(str::to_string)
            && blocks::block_at_state(world.get_block(clicked_x, clicked_y, clicked_z)) == Some("spawner")
            && crate::mob::Kind::from_name(&entity).is_some()
        {
            world.with_spawner(clicked_x, clicked_y, clicked_z, |data| data.entity = format!("minecraft:{entity}"));

            if let Some(nbt) = world.spawner(clicked_x, clicked_y, clicked_z).map(crate::spawner::Data::nbt) {
                world.tell_directly(clicked_x, clicked_y, clicked_z);
                world.tell_directly(bx, by, bz);
                world.tell_directly(clicked_x, clicked_y, clicked_z);
                world.note_block_entity(clicked_x, clicked_y, clicked_z, crate::spawner::BLOCK_ENTITY, nbt);
            }

            drop(world);

            let slots = spend_placed(player);

            return Some(Reaction { slots, ..Reaction::accepted(sequence) });
        }

        // Табличка: краситель, чернила, соты — или окно правки (signs.rs).
        if let Some(reaction) = signs::use_sign(&mut world, player, clicked, face, held_name, sequence) {
            redstone::settle(&mut world);

            return Some(reaction);
        }

        // Кафедра без книги: книга из руки ложится на неё. С книгой —
        // окно, как у хранилища; пустая и без книги в руке — щелчок идёт
        // дальше.
        let mut kind = kind.filter(|kind| !matches!(kind, containers::Kind::Jukebox | containers::Kind::Campfire));

        if kind == Some(containers::Kind::Lectern) {
            match redstone::lectern::place_book(&mut world, clicked, player.inventory.held(), 0) {
                Some(true) => {
                    world.tell_directly(clicked_x, clicked_y, clicked_z);
                    world.tell_directly(bx, by, bz);
                    drop(world);

                    let slots = spend_placed(player);

                    return Some(Reaction { slots, ..Reaction::accepted(sequence) });
                }
                Some(false) => {}
                None => kind = None,
            }
        }

        // Полка: ячейка по точке щелчка меняется с рукой, под током —
        // сцепка с правой частью панели (redstone/shelf.rs). Порядок как
        // у прочих щелчков: звук, повтор блока и за гранью, пачка, данные.
        if kind == Some(containers::Kind::Shelf) {
            let hotbar: Vec<Option<inventory::Stack>> =
                (0..9).map(|index| player.inventory.slot(inventory::FIRST_HOTBAR + index)).collect();
            let selected = player.inventory.selected() as usize;
            let used = redstone::shelf::use_shelf(
                &mut world,
                clicked,
                face,
                (cursor_x, cursor_y, cursor_z),
                &hotbar,
                selected,
                player.game_mode == GAME_MODE_CREATIVE,
            );

            match used {
                Some(used) => {
                    world.tell_directly(clicked_x, clicked_y, clicked_z);
                    world.tell_directly(bx, by, bz);
                    containers::after_change(&mut world);
                    redstone::settle(&mut world);
                    drop(world);

                    let changes = match used {
                        redstone::shelf::Used::Hand(stack) => vec![(selected, stack)],
                        redstone::shelf::Used::Hotbar(list) => list,
                    };
                    let mut slots = Vec::new();

                    for (index, stack) in changes {
                        let slot = inventory::FIRST_HOTBAR + index as i32;

                        if player.inventory.slot(slot) != stack {
                            player.inventory.set(slot, stack);
                            slots.push(slot);
                        }
                    }

                    return Some(Reaction { slots, ..Reaction::accepted(sequence) });
                }
                None => kind = None,
            }
        }

        // Резной книжный шкаф: книга из руки — в ячейку под точкой щелчка,
        // из занятой ячейки — игроку (redstone/bookshelf.rs).
        if kind == Some(containers::Kind::ChiseledBookshelf) {
            match redstone::bookshelf::use_bookshelf(&mut world, clicked, face, (cursor_x, cursor_y, cursor_z), player.inventory.held()) {
                Some(used) => {
                    world.tell_directly(clicked_x, clicked_y, clicked_z);
                    world.tell_directly(bx, by, bz);
                    containers::after_change(&mut world);
                    redstone::settle(&mut world);
                    drop(world);

                    let slots = match used {
                        redstone::bookshelf::Used::Placed => spend_placed(player),
                        // Не влезшее в инвентарь пропадает. [проверить] у
                        // оригинала оно выпадает под ноги.
                        redstone::bookshelf::Used::Taken(book) => player.inventory.put(book).0,
                    };

                    return Some(Reaction { slots, ..Reaction::accepted(sequence) });
                }
                None => kind = None,
            }
        }

        if kind.is_some() {
            let halves = if matches!(kind, Some(containers::Kind::EnderChest | containers::Kind::Beacon)) { vec![clicked] } else { containers::halves(&world, clicked) };

            if player.window.is_none() && containers::can_open(&world, &halves) {
                let actor = world.actor.replace(player.entity_id);
                let first = containers::open_begin(&mut world, &halves);
                world.actor = actor;

                world.note_open_window(player.entity_id, clicked_x, clicked_y, clicked_z);
                world.tell_directly(clicked_x, clicked_y, clicked_z);

                let (x, y, z) = offset_towards(clicked_x, clicked_y, clicked_z, face);

                world.tell_directly(x, y, z);
                containers::open_finish(&mut world, &halves, first);
                redstone::settle(&mut world);
            } else if player.window.is_none() {
                // Закрытый блоком сундук не открывается, но клиенту повторяют
                // сам блок и то, что за гранью (чёрный ящик, ender-chest-open).
                world.tell_directly(clicked_x, clicked_y, clicked_z);

                let (x, y, z) = offset_towards(clicked_x, clicked_y, clicked_z, face);

                world.tell_directly(x, y, z);
            }

            return Some(Reaction::accepted(sequence));
        }

        // Станок: окно открывается (station.rs). Порядок — как у хранилищ:
        // окно, повтор блока и за гранью.
        if station::station_at(&world, clicked).is_some() {
            if player.window.is_none() {
                world.note_open_window(player.entity_id, clicked_x, clicked_y, clicked_z);
            }

            world.tell_directly(clicked_x, clicked_y, clicked_z);
            world.tell_directly(bx, by, bz);

            return Some(Reaction::accepted(sequence));
        }

        // Куст ягод: сбор (farming.rs).
        if player.game_mode != GAME_MODE_SPECTATOR {
            drop(world);

            if let Some(reaction) = farm::pick_berries(player, shared, clicked, face, sequence) {
                return Some(reaction);
            }

            world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");
        }

        if let Some(held)=player.inventory.held()
            && let Some(name)=blocks::item_name(held.item)
            && crate::mob::newblocks::statue_use(&mut world,clicked,name)
        {
            let slot=player.inventory.selected_slot();
            if player.game_mode != GAME_MODE_CREATIVE {
                let next=if name=="honeycomb" {inventory::remainder(held,held.count-1)} else {combat::add_damage(held,combat::wear(&held,1,false,||world.random_unit()))};
                player.inventory.set(slot,next);
            }
            return Some(Reaction{slots:vec![slot],..Reaction::accepted(sequence)});
        }

        // Статуя медного голема: следующая поза (redstone/statue.rs).
        if redstone::statue::use_statue(&mut world, clicked) {
            world.tell_directly(clicked_x, clicked_y, clicked_z);
            world.tell_directly(bx, by, bz);
            redstone::settle(&mut world);

            return Some(Reaction::accepted(sequence));
        }

        // Хранилище: ключ испытаний открывает его (trial.rs; чёрный ящик,
        // vault-open: блок, блок за гранью, ключ из руки).
        if player.game_mode != GAME_MODE_SPECTATOR {
            let held = player.inventory.held().and_then(|stack| blocks::item_name(stack.item));
            let now = *shared.ticks.borrow();
            let seed = (u64::from(world.random_bits()) << 32) | u64::from(world.random_bits());

            if let Some(spend) = crate::trial::use_vault(&mut world, clicked, player.uuid, held, now, seed) {
                // Взмах рукой — дважды и самому игроку (vault-open: два
                // «Animate» до повтора блока).
                if spend {
                    world.note_animation(player.entity_id, ANIMATION_SWING_MAIN_ARM);
                    world.note_animation(player.entity_id, ANIMATION_SWING_MAIN_ARM);
                }

                world.tell_directly(clicked_x, clicked_y, clicked_z);
                world.tell_directly(bx, by, bz);

                let mut reaction = Reaction::accepted(sequence);

                if spend
                    && player.game_mode != GAME_MODE_CREATIVE
                    && let Some(slot) = player.inventory.spend_held()
                {
                    reaction.slots.push(slot);
                }

                return Some(reaction);
            }
        }

        // Звуки, вызванные рукой (дверь, люк, калитка), щёлкнувшему не
        // шлются: его клиент играет их сам.
        world.actor = Some(player.entity_id);
        let used = redstone::use_block_on(&mut world, (clicked_x, clicked_y, clicked_z), face);
        world.actor = None;

        if used {
            match blocks::block_at_state(clicked_state) {
                Some("note_block") => player.progress.custom("tune_noteblock", 1),
                Some("bell") => player.progress.custom("bell_ring", 1),
                _ => {}
            }
            log_debug!("Play: {} щёлкнул по {} {} {}", player.name, clicked_x, clicked_y, clicked_z);

            // Как у оригинала: после щелчка клиенту повторяют, что стоит
            // за той гранью, по которой щёлкнули, и в самом блоке — вдруг он
            // показал не то, что вышло на самом деле.
            let (dx, dy, dz) = match face {
                0 => (0, -1, 0),
                1 => (0, 1, 0),
                2 => (0, 0, -1),
                3 => (0, 0, 1),
                4 => (-1, 0, 0),
                _ => (1, 0, 0),
            };

            // Порядок снят с оригинала чёрным ящиком: повтор самого блока,
            // повтор за гранью — сразу, а перемены вокруг (провод
            // загорелся, лампа) — общей пачкой в точке рассылки такта.
            world.tell_directly(clicked_x, clicked_y, clicked_z);
            world.tell_directly(clicked_x + dx, clicked_y + dy, clicked_z + dz);
            redstone::settle(&mut world);

            // Что деталь делает после всего этого: нота музыкального блока
            // у оригинала звучит последней.
            redstone::after_use(&mut world, (clicked_x, clicked_y, clicked_z));

            return Some(Reaction::accepted(sequence));
        }
    }

    if let Some(reaction) = debug_stick::interact(shared, player, (clicked_x,clicked_y,clicked_z), face, false, sequence) {
        return Some(reaction);
    }
    if let Some(reaction) = gameplay::campfire_use(player, shared, (clicked_x, clicked_y, clicked_z), face, sequence) { return Some(reaction); }
    if let Some(reaction) = farm::use_hive(player, shared, (clicked_x, clicked_y, clicked_z), face, sequence) {
        return Some(reaction);
    }

    // Мотыга и костная мука (farming.rs).
    if brush::start(player,shared,hand) {
        // Оригинал повторяет блок и клетку за гранью (brush-sand, +10).
        let (bx, by, bz) = offset_towards(clicked_x, clicked_y, clicked_z, face);
        let mut world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");

        world.tell_directly(clicked_x, clicked_y, clicked_z);
        world.tell_directly(bx, by, bz);

        return Some(Reaction::accepted(sequence));
    }
    if let Some(reaction) = farm::use_on_block(player, shared, (clicked_x, clicked_y, clicked_z), face, sequence) {
        return Some(reaction);
    }

    // Огниво или огненный заряд по динамиту — поджог (вики, TNT: «Using a
    // flint and steel or a fire charge»), и в приключении тоже (вики, Flint
    // and Steel: «it can still ignite TNT»). Присевший вместо этого ставит
    // огонь рядом («results in the block catching fire») — это ниже, общим
    // путём огнива. Заряд тратится, огниво у нас износа пока не знает.
    let held = player.inventory.held().map(|stack| stack.item);
    let fire_charge = blocks::item_named("fire_charge");

    if !player.sneaking && held.is_some() && (held == blocks::item_named("flint_and_steel") || held == fire_charge) {
        let clicked = (clicked_x, clicked_y, clicked_z);
        let mut world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");

        if crate::tnt::ignite(&mut world, clicked) {
            redstone::settle(&mut world);
            drop(world);
            log_debug!("Play: {} поджёг динамит в {} {} {}", player.name, clicked_x, clicked_y, clicked_z);

            let slots = if held == fire_charge { spend_placed(player) } else {
                if let Some(item) = held { player.progress.used(item); }
                Vec::new()
            };

            return Some(Reaction {
                confirm: vec![clicked],
                slots,
                ..Reaction::accepted(sequence)
            });
        }
    }

    // Ракета фейерверка: щелчок по блоку пускает её из точки щелчка (вики,
    // Firework Rocket) [проверить в приключении].
    if thrown::launch_rocket(shared, player, (f64::from(clicked_x) + f64::from(cursor_x), f64::from(clicked_y) + f64::from(cursor_y), f64::from(clicked_z) + f64::from(cursor_z)), face) {
        // Повтор блока щелчка и клетки за гранью (чёрный ящик, firework-blast).
        let mut reaction = Reaction { confirm: vec![(clicked_x, clicked_y, clicked_z), offset_towards(clicked_x, clicked_y, clicked_z, face)], ..Reaction::accepted(sequence) };

        if player.game_mode != GAME_MODE_CREATIVE
            && let Some(slot) = player.inventory.spend_held()
        {
            reaction.slots.push(slot);
        }

        return Some(reaction);
    }

    // В приключении рычаги и двери трогать можно, а ставить блоки — нет.
    if player.game_mode == GAME_MODE_ADVENTURE {
        return Some(refused);
    }

    // Командный блок ставит только оператор в творческом режиме (вики,
    // Command Block: «cannot be obtained or placed» без прав).
    if player.inventory.held().and_then(|stack| blocks::item_name(stack.item)).is_some_and(|name| name.ends_with("command_block"))
        && !crate::command_block::may_edit(shared, &player.uuid, player.game_mode)
    {
        let (bx, by, bz) = offset_towards(clicked_x, clicked_y, clicked_z, face);

        return Some(Reaction {
            confirm: vec![(clicked_x, clicked_y, clicked_z), (bx, by, bz)],
            ..Reaction::accepted(sequence)
        });
    }

    // Рамка вешается на грань блока, по которой щёлкнули: клиентам уходит
    // звук и сама рамка, потом повторяются блок опоры и клетка рамки (чёрный
    // ящик, item-frame-rotate и item-frame-facing).
    if let Some(glow) = player
        .inventory
        .held()
        .and_then(|stack| blocks::item_name(stack.item))
        .and_then(|name| match name {
            "item_frame" => Some(false),
            "glow_item_frame" => Some(true),
            _ => None,
        })
    {
        let clicked = (clicked_x, clicked_y, clicked_z);
        let mut world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");
        let placed = {
            let mut items = shared.items_in(player.dimension).lock().expect("предметы захвачены другим потоком");

            crate::frame::place(&mut world, &mut items, shared, clicked, face as u8, glow)
        };

        if !placed {
            return Some(Reaction::accepted(sequence));
        }

        log_debug!("Play: {} повесил рамку в {} {} {}", player.name, clicked_x, clicked_y, clicked_z);

        let (x, y, z) = offset_towards(clicked_x, clicked_y, clicked_z, face);

        world.tell_directly(clicked_x, clicked_y, clicked_z);
        world.tell_directly(x, y, z);
        drop(world);

        return Some(Reaction {
            slots: spend_placed(player),
            ..Reaction::accepted(sequence)
        });
    }

    // Вагонетка ставится только на рельсу (вики, Minecart: «can be placed
    // on rails»); мимо рельсы щелчок ничего не делает.
    if let Some(kind) = player
        .inventory
        .held()
        .and_then(|stack| blocks::item_name(stack.item))
        .and_then(crate::minecart::Kind::from_name)
    {
        let clicked = (clicked_x, clicked_y, clicked_z);
        let state = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком").get_block(clicked_x, clicked_y, clicked_z);

        let Some(at) = crate::minecart::place_on_rail(state, clicked) else {
            return Some(Reaction::accepted(sequence));
        };

        shared
            .items_in(player.dimension)
            .lock()
            .expect("предметы захвачены другим потоком")
            .add_cart(crate::minecart::Minecart::new(shared.next_entity_id(), kind, at));
        log_debug!("Play: {} поставил вагонетку в {} {} {}", player.name, clicked_x, clicked_y, clicked_z);

        // Как у оригинала: после появления вагонетки клиенту повторяют
        // рельсу и то, что за гранью щелчка (зонд minecart-hit).
        {
            let (x, y, z) = offset_towards(clicked_x, clicked_y, clicked_z, face);
            let mut world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");

            world.tell_directly(clicked_x, clicked_y, clicked_z);
            world.tell_directly(x, y, z);
        }

        return Some(Reaction {
            slots: spend_placed(player),
            ..Reaction::accepted(sequence)
        });
    }

    // Какой блок ставить, решает предмет в руке: сервер ищет его в таблице
    // блоков. Если предмет блока не даёт, ставить нечего, но подтвердить
    // действие всё равно нужно — подтверждение означает «сервер здесь ничего
    // не менял», и клиент уберёт свой предсказанный блок, если он его показал.
    // Зажигалка: по обсидиану внутри рамки — портал, иначе огонь на месте
    // рядом с гранью, по которой щёлкнули.
    if player.inventory.held().and_then(|stack| blocks::item_name(stack.item)).is_some_and(|name| matches!(name, "flint_and_steel" | "fire_charge")) {
        let target = offset_towards(clicked_x, clicked_y, clicked_z, face);
        let mut world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");

        let mut ignited = false;
        if world.get_block(target.0, target.1, target.2) == world::AIR {
            if let Some(stack) = player.inventory.held() { player.progress.used(stack.item); }
            if crate::portal::light(&mut world, target, shared.settings.portal_sizes()) {
                log_info!("{} зажёг портал в {} {} {}", player.name, target.0, target.1, target.2);
                ignited = true;
            } else {
                ignited = crate::fire::ignite(&mut world, target);
            }

            redstone::settle(&mut world);
        }

        // Как пустой рукой: оригинал повторяет клиенту блок, по которому
        // щёлкнули, и блок за гранью — прежде общей пачки (чёрный ящик,
        // nether-portal).
        world.tell_directly(clicked_x, clicked_y, clicked_z);
        world.tell_directly(target.0, target.1, target.2);
        if ignited {
            let charge = player.inventory.held().and_then(|s| blocks::item_name(s.item)) == Some("fire_charge");
            let pitch = 0.8 + world.random_unit() * 0.4;
            // Звук зажигалки слышат все, кроме самого игрока: он играет его
            // у себя (чёрный ящик, nether-portal: бот-зажигатель звука не получает).
            let actor = world.actor.replace(player.entity_id);
            world.note_sound_by_hand(target.0, target.1, target.2, if charge { "minecraft:item.firecharge.use" } else { "minecraft:item.flintandsteel.use" }, 1.0, pitch);
            world.actor = actor;
        }

        drop(world);
        let mut slots = Vec::new();
        if ignited && let Some(stack) = player.inventory.held() {
            player.progress.used(stack.item);
            if blocks::item_name(stack.item) == Some("fire_charge") { slots = spend_placed(player); }
            else if player.game_mode != GAME_MODE_CREATIVE { pvp::wear_slot(shared, player, player.inventory.selected_slot(), stack, 1, false); }
        }
        return Some(Reaction { slots, ..Reaction::accepted(sequence) });
    }

    let Some(item) = player.inventory.held().map(|stack| stack.item) else {
        log_debug!("Play: содержимое слота неизвестно — ставить нечего");

        // Пустой рукой по блоку: оригинал повторяет клиенту сам блок и блок
        // за гранью щелчка — как после щелчка по рычагу, — а перемены
        // (загоревшаяся руда) уходят общей пачкой (сценарии
        // redstone-ore-click, bell-click).
        let mut world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");
        let (x, y, z) = offset_towards(clicked_x, clicked_y, clicked_z, face);

        world.tell_directly(clicked_x, clicked_y, clicked_z);
        world.tell_directly(x, y, z);
        redstone::settle(&mut world);

        return Some(Reaction::accepted(sequence));
    };

    // Котёл: ведро, бутылка, шулкер-ящик (cauldron.rs).
    if !player.sneaking
        && let Some(reaction) = cauldron::use_cauldron((clicked_x, clicked_y, clicked_z), face, sequence, player, shared)
    {
        return Some(reaction);
    }

    // Жидкие вёдра применяются только следующим Use Item (вики, Bucket).
    if buckets::liquid(item) { return Some(Reaction::accepted(sequence)); }
    if let Some(reaction) = use_bucket(
        item,
        (clicked_x, clicked_y, clicked_z),
        face,
        sequence,
        player,
        shared,
    ) {
        return Some(reaction);
    }

    if let Some(reaction) = use_spawn_egg(item, (clicked_x, clicked_y, clicked_z), face, sequence, player, shared) {
        return Some(reaction);
    }

    let Some((name, default_state)) = blocks::block_for_item(item) else {
        log_debug!("Play: предмет {} блоком не является — ставить нечего", item);

        // Оригинал и тут повторяет клиенту блок и блок за гранью (чёрный
        // ящик, axe-strip-logs, block3-pumpkin: второй щелчок инструментом).
        let mut world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");
        let (x, y, z) = offset_towards(clicked_x, clicked_y, clicked_z, face);

        world.tell_directly(clicked_x, clicked_y, clicked_z);
        world.tell_directly(x, y, z);
        redstone::settle(&mut world);

        return Some(Reaction::accepted(sequence));
    };

    // Табличка, поставленная на бок блока, — настенная (вики, Sign: «wall
    // sign»). Подвесная ставится по умолчанию. [проверить] подвесные.
    let wall_sign = (name.ends_with("_sign") && !name.contains("hanging") && !name.contains("wall") && (2..=5).contains(&face))
        .then(|| name.replace("_sign", "_wall_sign"))
        .and_then(|wall| Some((blocks::orientation(&wall)?.name, blocks::state_by_name(&wall)?)));
    let (name, default_state) = wall_sign.unwrap_or((name, default_state));

    let click = placing::Click {
        face,
        cursor_y,
        yaw: player.yaw,
        pitch: player.pitch,
    };

    // Плагины могут не дать поставить (plugins/): место — блок за гранью
    // щелчка. Замка мира мы тут не держим.
    if crate::plugins::active() {
        let target = offset_towards(clicked_x, clicked_y, clicked_z, face);

        if crate::plugins::block_place(&plugin_view(player), target, &format!("minecraft:{name}")) == crate::plugins::Verdict::Deny {
            log_debug!("Play: плагин не дал {} поставить блок в {} {} {}", player.name, target.0, target.1, target.2);
            return Some(Reaction::refused(sequence, &[(clicked_x, clicked_y, clicked_z), target]));
        }
    }

    let mut world = shared
        .world_in(player.dimension)
        .lock()
        .expect("мир захвачен другим потоком");

    let actor = world.actor.replace(player.entity_id);
    let placed = placing::place_tagged(
        &mut world,
        name,
        default_state,
        (clicked_x, clicked_y, clicked_z),
        &click,
        (player.x, player.y, player.z),
        player.inventory.held().and_then(|stack| stack.tnt),
    );
    world.actor = actor;

    // Тыква на столбе из снега или железа — голем (mob/golem.rs; чёрный ящик,
    // iron-golem-build, snow-golem-build).
    if matches!(placed, placing::Placement::Placed)
        && let Some(kind) = build_golem(shared, player.dimension, &mut world, offset_towards(clicked_x, clicked_y, clicked_z, face))
    {
        player.progress.summoned(kind.name());
    }

    // Как у оригинала после установки: сразу повтор блока, по которому
    // щёлкнули, и того, что встал за гранью, — а общая пачка потом
    // (чёрный ящик, chest-place).
    if matches!(placed, placing::Placement::Placed) {
        player.progress.planted(name);
        progression::placed(&world, player, name, offset_towards(clicked_x, clicked_y, clicked_z, face));
        let (x, y, z) = offset_towards(clicked_x, clicked_y, clicked_z, face);

        world.tell_directly(clicked_x, clicked_y, clicked_z);
        world.tell_directly(x, y, z);
    }

    // Поставленная табличка сразу открывает игроку окно правки передней
    // стороны (вики, Sign: «automatically opens the editing interface»).
    if matches!(placed, placing::Placement::Placed) {
        let near = offset_towards(clicked_x, clicked_y, clicked_z, face);

        if redstone::is_sign(world.get_block(near.0, near.1, near.2)) {
            player.sign_editor = Some(near);
            world.note_open_sign(player.entity_id, near.0, near.1, near.2, true);
        }
    }

    if matches!(placed, placing::Placement::Placed) && let Some(held)=player.inventory.held()
        && matches!(blocks::item_name(held.item),Some("beehive"|"bee_nest"))
    {
        let near=offset_towards(clicked_x,clicked_y,clicked_z,face);
        let at=if crate::hive::is_hive(world.get_block(near.0,near.1,near.2)) {near} else {(clicked_x,clicked_y,clicked_z)};
        crate::hive::place_contents(&mut world,at,&held);
    }

    // Шулкер-ящик с содержимым в предмете ставится с ним (вики, Shulker Box).
    if matches!(placed, placing::Placement::Placed)
        && containers::is_shulker_item(item)
        && let Some(held) = player.inventory.held()
        && !held.extras().contents.is_empty()
    {
        let contents = containers::contents_of(&held);
        let near = offset_towards(clicked_x, clicked_y, clicked_z, face);

        for (x, y, z) in [near, (clicked_x, clicked_y, clicked_z)] {
            if world.container(x, y, z).is_some_and(|found| found.kind == containers::Kind::ShulkerBox) {
                world.with_container(x, y, z, |found| found.slots.clone_from(&contents));
                break;
            }
        }
    }

    match placed {
        placing::Placement::Placed => Some(Reaction {
            slots: spend_placed(player),
            ..Reaction::accepted(sequence)
        }),
        placing::Placement::Merged | placing::Placement::Nothing => Some(Reaction::accepted(sequence)),
        placing::Placement::Refused(places) => Some(Reaction::refused(sequence, &places)),
    }
}

/// Голем из только что поставленной тыквы: события 2001 по клеткам узора,
/// голем, потом блоки уходят воздухом (чёрный ящик, iron-golem-build).
/// [проверить] тыква, поставленная командой или раздатчиком.
fn build_golem(shared: &Shared, dimension: crate::dimension::Dimension, world: &mut World, at: (i32, i32, i32)) -> Option<crate::mob::Kind> {
    let build = crate::mob::golem::find(&crate::tick::MobView(world), at)?;
    let kind = build.kind;

    for ((x, y, z), state) in &build.cells {
        world.note_level_event(*x, *y, *z, 2001, *state);
    }

    let mut mob = crate::mob::Mob::new(shared.next_entity_id(), build.kind, build.at);

    mob.golem.player_made = build.kind == crate::mob::Kind::IronGolem;
    let copper_body = if kind == crate::mob::Kind::CopperGolem {
        let body = build.cells.iter().find(|(_,state)| blocks::block_at_state(*state).and_then(crate::mob::newmobs::copper_stage).is_some()).copied();
        if let Some((_, state)) = body { mob.newmobs.oxidation = blocks::block_at_state(state).and_then(crate::mob::newmobs::copper_stage).unwrap_or(0); }
        (mob.x, mob.y, mob.z) = (f64::from(at.0)+0.5, f64::from(at.1)+0.05, f64::from(at.2)+0.5);
        // copper-golem-sort: первый осмотр сундуков — через ~90 тактов после рождения (copper-golem-sort).
        mob.newmobs.chest_wait = -90;
        body
    } else { None };
    let head_facing = blocks::orientation_of(world.get_block(at.0,at.1,at.2)).and_then(|b| b.value(world.get_block(at.0,at.1,at.2), "facing")).unwrap_or("north");

    // Иссушитель: треть здоровья, зарядка, поворот по оси Т (mob/wither.rs).
    if build.kind == crate::mob::Kind::Wither {
        let along_x = build.cells.first().zip(build.cells.last()).is_some_and(|(first, last)| first.0.0 != last.0.0);

        crate::mob::wither::built(&mut mob, along_x);

        // Голова с рождения чуть повёрнута — случайно, на 1–6° (wither-summon,
        // четыре записи оригинала: head 1, 3, 3, 4 из 256) [проверить: откуда].
        mob.head_yaw = 1.5 + world.random_unit() * 4.5;
    }
    shared.items_in(dimension).lock().expect("предметы захвачены другим потоком").add_mob_late(mob.clone());
    // Вики, Advancement: свидетели постройки тоже получают достижение.
    if kind == crate::mob::Kind::Wither {
        world.note_progress_near("nether/summon_wither", (mob.x, mob.y + 1.75, mob.z), (50.45, 51.75, 50.45));
    } else if kind == crate::mob::Kind::IronGolem {
        // [проверить] границы: объём голема, расширенный на 5 блоков.
        world.note_progress_near("adventure/summon_iron_golem", (mob.x, mob.y + 1.35, mob.z), (5.7, 6.35, 5.7));
    }
    let born = (mob.x, mob.y, mob.z);
    world.note_mob_spawn(mob);
    if kind == crate::mob::Kind::CopperGolem {
        world.note_sound_neutral_at(born, "minecraft:entity.copper_golem.spawn", 1.0, 1.0);
    }

    for (x, y, z) in build.clear {
        world.set_block(x, y, z, crate::world::AIR);
    }

    if let Some((p, state)) = copper_body {
        let stage = blocks::block_at_state(state).and_then(crate::mob::newmobs::copper_stage).unwrap_or(0);
        let name = ["copper_chest", "exposed_copper_chest", "weathered_copper_chest", "oxidized_copper_chest"][stage as usize];
        if let Some(state) = blocks::orientation(name).and_then(|b| b.state(&[("facing", head_facing)])) { world.set_block(p.0,p.1,p.2,state); }
    }
    Some(kind)
}

/// «Выбор блока»: игрок навёл на постройку и просит такой же предмет в руку.
///
/// Если предмет уже есть в панели — просто переключаем на него руку. Если
/// лежит в рюкзаке — меняем местами с тем, что в руке. А если его нет вовсе,
/// заводим — но только в творческом режиме: в выживании предметы берутся
/// из мира.
fn pick_block(payload: &[u8], player: &mut PlayerState, shared: &Shared) -> Option<Reaction> {
    let mut cursor = Cursor::new(payload);
    let (x, y, z) = cursor.position()?;

    let state = shared
        .world_in(player.dimension)
        .lock()
        .expect("мир захвачен другим потоком")
        .get_block(x, y, z);

    let name = blocks::block_at_state(state)?;

    let Some(item) = blocks::item_for_block(name) else {
        log_debug!("Play: для блока {} предмета нет — выбирать нечего", name);
        return Some(Reaction::default());
    };

    let found = player.inventory.find(item);

    // Заводить предмет из ничего можно только в творческом: в выживании
    // предметы берутся из мира.
    if found.is_none() && player.game_mode != GAME_MODE_CREATIVE {
        return Some(Reaction::default());
    }

    let picked = player.inventory.pick(item, found);

    log_debug!("Play: {} выбрал блок {} ({})", player.name, name, item);

    Some(Reaction {
        slots: picked.slots,
        held: picked.selected,
        ..Reaction::default()
    })
}

/// Ведро в руке: налить жидкость в мир или зачерпнуть её оттуда.
///
/// None означает, что в руке не ведро и надо разбираться дальше по-обычному.
///
/// Номера предметов здесь названы по именам из таблицы: ведро одно, а полных
/// два — с водой и с лавой.
/// Use Item: предмет применяют не к блоку, а в сторону взгляда. Нужен он
/// пустому ведру: воду прицел клиента не видит, и зачерпнуть её можно
/// только этим пакетом — сервер сам ведёт луч от глаз до первого источника
/// (текущая вода пропускается, твёрдый блок останавливает), как у
/// оригинала. Подтверждается всегда.
fn use_item(payload: &[u8], player: &mut PlayerState, shared: &Shared) -> Option<Reaction> {
    let mut cursor = Cursor::new(payload);
    let hand = cursor.varint()?;
    let sequence = cursor.varint()?;
    let yaw = cursor.f32().unwrap_or(player.yaw);
    let pitch = cursor.f32().unwrap_or(player.pitch);

    let answer = Reaction::accepted(sequence);

    // Пакет несёт и взгляд: сервер поворачивает игрока так же (чёрный ящик,
    // pvp-shield: от взгляда зависит, держит ли щит удар).
    if !player.awaiting_teleport {
        player.set_rotation(yaw, pitch);
    }

    if let Some(resync) = fishing::use_rod(shared, player, hand) {
        let mut answer = answer;
        answer.inventory = resync;
        return Some(answer);
    }
    if let Some(resync)=brush::idle_use(player,hand) {
        let mut answer = answer;
        answer.inventory = resync;
        return Some(answer);
    }
    if maps::fill(shared, player, hand).is_some() {
        let mut answer = answer;
        answer.inventory = true;
        return Some(answer);
    }

    // Ракета при полёте на элитрах: разгон (glide.rs).
    if glide::use_rocket(shared, player, hand) {
        return Some(answer);
    }

    if knowledge_book::use_book(player, hand) {
        let mut answer = answer;
        answer.slots.push(if hand == 1 { inventory::OFFHAND } else { player.inventory.selected_slot() });
        return Some(answer);
    }

    // Лодка или плот: на воду или землю по лучу взгляда (vehicle.rs).
    if vehicle::place_boat(shared, player, hand) {
        return Some(answer);
    }

    // Взрывное или оседающее зелье: бросок (thrown.rs).
    let pearl = (if hand == 1 { player.inventory.slot(inventory::OFFHAND) } else { player.inventory.held() })
        .is_some_and(|stack| blocks::item_name(stack.item) == Some("ender_pearl"));
    if thrown::throw_potion(shared, player, hand) {
        let mut answer = answer;
        if pearl {
            let mut packet = encode_varint(0x16);
            packet.extend_from_slice(&encode_string("minecraft:ender_pearl"));
            packet.extend_from_slice(&encode_varint(20));
            answer.packets.push(packet);
        }
        return Some(answer);
    }

    // Заряд ветра: бросок и откат 10 тактов (thrown.rs).
    if let Some(cooldown) = thrown::throw_wind_charge(shared, player, hand) {
        let mut answer = answer;

        answer.packets.push(cooldown);
        return Some(answer);
    }

    // Око Края: летит к крепости (ender_eye.rs); рука — «использует» и без
    // броска.
    let thrown_eye = thrown::throw_eye(shared, player, hand);

    vitals::start_using_eye(player, hand, *shared.ticks.borrow());

    if thrown_eye {
        return Some(answer);
    }

    // Пустой пузырёк у облака дыхания дракона: набрать.
    if thrown::fill_breath(shared, player, hand) {
        return Some(answer);
    }

    // Еда в руке: начать есть.
    if vitals::start_eating_at(player, hand, *shared.ticks.borrow()) {
        return Some(answer);
    }

    // Заряженный арбалет: выстрел.
    if bow::crossbow_fire(shared, player, hand) {
        return Some(answer);
    }

    // Щит, лук или арбалет: поднять, натянуть, зарядить.
    if vitals::start_using(shared, player, hand, *shared.ticks.borrow()) {
        return Some(answer);
    }

    buckets::use_item(shared, player, hand, sequence, yaw, pitch).or(Some(answer))
}

fn use_bucket(
    item: i32,
    clicked: (i32, i32, i32),
    face: i32,
    sequence: i32,
    player: &mut PlayerState,
    shared: &Shared,
) -> Option<Reaction> {
    let empty = blocks::item_named("bucket")?;
    let with_water = blocks::item_named("water_bucket")?;
    let with_lava = blocks::item_named("lava_bucket")?;
    let with_snow = blocks::item_named("powder_snow_bucket")?;

    let fish_kind = blocks::item_name(item).and_then(crate::mob::water::bucket_kind);
    let fish_data = player.inventory.held().map(|stack| stack.extras()).unwrap_or_default();
    let (x, y, z) = clicked;

    if item == empty || item == with_water {
        let mut world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");
        let here = world.get_block(x, y, z);
        if !(item == with_water && world.is_nether())
            && let Some(next) = crate::block_behaviour::bucket_water(here, item == with_water)
        {
            world.set_block(x, y, z, next);
            // Звук ведра не уходит самому игроку: его играет клиент (чёрный
            // ящик, tails-bucket-use-0: «лишний» item.bucket.empty у нас).
            let actor = world.actor.replace(player.entity_id);
            world.note_sound_by_hand(x, y, z, if item == empty { "minecraft:item.bucket.fill" } else { "minecraft:item.bucket.empty" }, 1.0, 1.0);
            world.actor = actor;
            if blocks::orientation_of(here).is_some_and(|b| b.value(here, "lit") == Some("true") && b.value(next, "lit") == Some("false")) {
                world.note_sound(x, y, z, if blocks::block_at_state(here).is_some_and(|n| n == "candle" || n.ends_with("_candle")) { "minecraft:block.candle.extinguish" } else { "minecraft:block.fire.extinguish" }, 1.0, 1.0);
            }
            // Ведро берётся из Use Item, у него нет щелчка по блоку, а значит и
            // повтора блока отдельным сообщением: изменение уходит со всеми
            // в конце такта (чёрный ящик, tails-bucket-use-0/1).
            redstone::settle(&mut world);
            drop(world);
            player.progress.used(item);
            return Some(Reaction { slots: swap_held(player, if item == empty { with_water } else { empty }, shared), ..Reaction::accepted(sequence) });
        }
    }

    // Зачерпнуть можно источник жидкости или рыхлый снег: из текущей воды
    // ведро не наполняется.
    if item == empty {
        let clicked_state = {
            let world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");

            world.get_block(x, y, z)
        };

        let filled = match fluids::fluid_at(clicked_state) {
            Some((fluids::Kind::Water, fluids::SOURCE)) => with_water,
            Some((fluids::Kind::Lava, fluids::SOURCE)) => with_lava,
            _ if fluids::is_powder_snow(clicked_state) => with_snow,
            _ => return Some(Reaction::accepted(sequence)),
        };

        {
            let mut world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");

            let replacement = if crate::campfire::is_campfire(clicked_state) {
                blocks::orientation_of(clicked_state).and_then(|o| o.with(clicked_state, "waterlogged", "false")).unwrap_or(clicked_state)
            } else { world::AIR };
            world.set_block(x, y, z, replacement);
            redstone::settle(&mut world);
        }

        player.progress.used(item);
        log_debug!("Play: {} набрал ведро в {} {} {}", player.name, x, y, z);

        return Some(Reaction {
            slots: swap_held(player, filled, shared),
            ..Reaction::accepted(sequence)
        });
    }

    // Что выльется из ведра. Рыхлый снег не жидкость — он просто встаёт
    // блоком, никуда не течёт и сам по себе не тает.
    let poured = if item == with_water || fish_kind.is_some() {
        fluids::state_of(fluids::Kind::Water, fluids::SOURCE)
    } else if item == with_lava {
        fluids::state_of(fluids::Kind::Lava, fluids::SOURCE)
    } else if item == with_snow {
        blocks::state_by_name("powder_snow")?
    } else {
        return None;
    };

    // Содержимое встаёт в тот блок, по которому щёлкнули, если он пустой,
    // иначе — в соседний с той стороны, куда щёлкнули.
    let here = {
        let world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");

        world.get_block(x, y, z)
    };

    let can_waterlog = |state| blocks::orientation_of(state).and_then(|o|o.value(state,"waterlogged"))==Some("false");
    let (x, y, z) = if here == world::AIR || (fish_kind.is_some() && can_waterlog(here)) {
        (x, y, z)
    } else {
        offset_towards(x, y, z, face)
    };

    if fish_kind.is_some() {
        let world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");
        let target = world.get_block(x,y,z);
        if !blocks::replaceable(target) && !crate::mob::is_water(target) && !can_waterlog(target) { return Some(Reaction::accepted(sequence)); }
    }

    {
        let mut world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");

        // В Незере вода из ведра испаряется с шипением (вики, Water).
        if world.is_nether() && (item == with_water || fish_kind.is_some()) {
            let pitch = 2.6 + (world.random_unit() - world.random_unit()) * 0.8;
            world.note_sound(x, y, z, "minecraft:block.fire.extinguish", 0.5, pitch);
        } else {
            let target=world.get_block(x,y,z);
            if !blocks::replaceable(target) && !crate::mob::is_water(target)
                && !((item == with_water || fish_kind.is_some()) && can_waterlog(target)) { return Some(Reaction::accepted(sequence)); }
            let placed=if fish_kind.is_some() && can_waterlog(target) {
                blocks::orientation_of(target).and_then(|o|o.with(target,"waterlogged","true")).unwrap_or(poured)
            } else {poured};
            world.set_block(x, y, z, placed);
            if let Some(kind)=fish_kind {
                let sound=match kind {
                    crate::mob::Kind::Axolotl=>"minecraft:item.bucket.empty_axolotl",
                    crate::mob::Kind::Tadpole=>"minecraft:item.bucket.empty_tadpole",
                    _=>"minecraft:item.bucket.empty_fish",
                };
                world.actor=Some(player.entity_id);
                world.note_sound_by_hand(x,y,z,sound,1.0,1.0);
                world.actor=None;
            }
            redstone::settle(&mut world);
        }
    }

    player.progress.used(item);
    log_debug!("Play: {} вылил ведро в {} {} {}", player.name, x, y, z);
    player.poured = Some(sequence);
    if let Some(kind) = fish_kind {
        let mut mob = crate::mob::Mob::new(shared.next_entity_id(),kind,(f64::from(x)+0.5,f64::from(y),f64::from(z)+0.5));
        crate::mob::water::bucket::release(&mut mob,&fish_data);
        shared.items_in(player.dimension).lock().expect("предметы захвачены другим потоком").add_mob_late(mob);
    }

    Some(Reaction {
        slots: swap_held(player, empty, shared),
        ..Reaction::accepted(sequence)
    })
}

/// Яйцо призыва по блоку: моб появляется в клетке за гранью щелчка (вики,
/// Spawn Egg: «spawns the mob on the face of the block clicked»), в
/// выживании яйцо тратится. Работает для тех видов, что есть в ядре; прочие
/// яйца ничего не делают.
fn use_spawn_egg(
    item: i32,
    clicked: (i32, i32, i32),
    face: i32,
    sequence: i32,
    player: &mut PlayerState,
    shared: &Shared,
) -> Option<Reaction> {
    let kind = crate::mob::Kind::from_name(blocks::item_name(item)?.strip_suffix("_spawn_egg")?)?;
    player.progress.used(item);
    let (x, y, z) = offset_towards(clicked.0, clicked.1, clicked.2, face);
    let mut mob = crate::mob::Mob::new(shared.next_entity_id(), kind, (f64::from(x) + 0.5, f64::from(y), f64::from(z) + 0.5));

    if kind == crate::mob::Kind::Slime {
        let mut rng = crate::mob::Rng::new(mob.entity_id as u64);
        let size = crate::mob::slime::roll_size(&mut rng);
        crate::mob::slime::set_size(&mut mob, size);
    }

    if crate::mob::land::animal(kind) { crate::mob::land::initialize(&mut mob, &mut crate::mob::Rng::new(shared.next_entity_id() as u64)); }
    if crate::mob::water::creature(kind) {
        let biome = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком").mob_biome(x,y,z);
        let mut rng = crate::mob::Rng::new(*shared.ticks.borrow() ^ mob.entity_id as u64);
        crate::mob::water::initialize(&mut mob,biome,&mut rng);
    }
    if crate::mob::horse::equine(kind) {
        let mut rng = crate::mob::Rng::new(mob.entity_id as u64 ^ (*shared.ticks.borrow() << 16));
        crate::mob::horse::initialize(&mut mob, &mut rng);
        if kind == crate::mob::Kind::Horse { mob.horse.variant = crate::mob::horse::wild_variant(&mut rng); }
        // Вики, Mule: «20% chance of spawning as foals». Нежить — взрослая.
        if !crate::mob::horse::undead(kind) && rng.below(5) == 0 { mob.age = crate::mob::BABY_AGE; }
    }
    if crate::mob::wildlife::kind(kind) {
        let biome = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком").mob_biome(x, y, z);
        crate::mob::wildlife::initialize(&mut mob, biome, &mut crate::mob::Rng::new(shared.next_entity_id() as u64));
    }

    // Смотрит туда же, куда игрок, как призванный с яйца у оригинала.
    mob.yaw = player.yaw;
    mob.head_yaw = player.yaw;

    if kind == crate::mob::Kind::Chicken {
        mob.egg_timer = 6000 + (mob.entity_id as u32 % 6001) as i32;
    }

    shared.items_in(player.dimension).lock().expect("предметы захвачены другим потоком").add_mob_late(mob);

    let mut reaction = Reaction::accepted(sequence);

    if player.game_mode != GAME_MODE_CREATIVE
        && let Some(slot) = player.inventory.spend_held()
    {
        reaction.slots.push(slot);
    }

    Some(reaction)
}

/// Меняет предмет в руке на другой — ведро наполняется и пустеет.
///
/// В творческом режиме ведро не меняется: запас там бесконечный.
fn swap_held(player: &mut PlayerState, item: i32, shared: &Shared) -> Vec<i32> {
    if player.game_mode == GAME_MODE_CREATIVE {
        return Vec::new();
    }

    let slot = inventory::FIRST_HOTBAR + player.inventory.selected();

    // Ведро было не одно — остальные остаются на месте, а полное кладётся
    // отдельно; не поместилось — падает на землю рядом с игроком.
    match player.inventory.held() {
        Some(held) if held.count > 1 => {
            player.inventory.spend_held();

            let mut slots = vec![slot];
            let (changed, left) = player.inventory.put(Stack::new(item, 1));

            slots.extend(changed);

            if let Some(left) = left {
                throw_out(shared, player, left);
            }

            slots
        }
        _ => {
            player.inventory.set(slot, Some(Stack::new(item, 1)));
            vec![slot]
        }
    }
}

/// Расходует поставленный блок: в выживании из руки уходит одна штука.
///
/// Возвращает слоты, о которых надо рассказать клиенту. В творческом режиме
/// не расходуется ничего — там запас бесконечный.
fn spend_placed(player: &mut PlayerState) -> Vec<i32> {
    // «Использовано» считается и в творческом (вики, Statistics: ITEM_USED).
    if let Some(held) = player.inventory.held() {
        player.progress.used(held.item);
    }

    if player.game_mode == GAME_MODE_CREATIVE {
        return Vec::new();
    }

    player.inventory.spend_held().into_iter().collect()
}

/// Соседний блок с той стороны, по которой пришёлся щелчок.
///
/// Номера сторон: 0 — низ, 1 — верх, 2 — север, 3 — юг, 4 — запад, 5 — восток.
fn offset_towards(x: i32, y: i32, z: i32, face: i32) -> (i32, i32, i32) {
    match face {
        0 => (x, y - 1, z),
        1 => (x, y + 1, z),
        2 => (x, y, z - 1),
        3 => (x, y, z + 1),
        4 => (x - 1, y, z),
        5 => (x + 1, y, z),
        _ => (x, y, z),
    }
}

/// Читает «слот» протокола: сколько предметов, какой предмет и есть ли у него
/// довески.
///
/// Довески (components) — это всё, чем предмет отличается от такого же
/// обычного: имя, зачарования, содержимое. Их длина сервером не разбирается,
/// поэтому дальше по пакету после довесков читать нельзя — об этом говорит
/// второе значение: false означает «дальше читать нечего».
#[cfg(test)]
fn read_slot(cursor: &mut Cursor) -> Option<(Option<Stack>, bool)> {
    let count = cursor.varint()?;

    if count <= 0 {
        return Some((None, true));
    }

    let item = cursor.varint()?;
    let added = cursor.varint()?;
    let removed = cursor.varint()?;

    let readable = added == 0 && removed == 0;

    Some((Some(Stack::new(item, count)), readable))
}

/// Читает слот от клиента в творческом режиме (Untrusted Slot): у каждого
/// довеска там своя длина, поэтому их можно пропускать. Из довесков
/// смотрится только custom_data — пометка особого динамита; прочие
/// отбрасываются, как и прежде.
fn read_untrusted_slot(cursor: &mut Cursor) -> Option<Option<Stack>> {
    let count = cursor.varint()?;

    if count <= 0 {
        return Some(None);
    }

    let mut stack = Stack::new(cursor.varint()?, count);
    let added = cursor.varint()?;
    let _removed = cursor.varint()?;

    let mut extras = crate::stackdata::Data::default();

    for _ in 0..added.clamp(0, 256) {
        let kind = cursor.varint()?;
        let length = usize::try_from(cursor.varint()?).ok()?;
        let data = cursor.bytes(length)?;

        if kind == COMPONENT_CUSTOM_DATA
            && let Some(name) = nbt::string_field(data, crate::custom_tnt::MARKER)
            && Some(stack.item) == blocks::item_named("tnt")
        {
            stack.tnt = crate::custom_tnt::by_name(&name).map(|kind| kind.id);
        }

        crate::stackdata::read_component(kind, data, &mut extras);
    }

    // Имя особого динамита — от его вида, своим именем оно не хранится.
    if let Some(kind) = stack.tnt.and_then(crate::custom_tnt::by_id)
        && extras.name.as_deref() == Some(kind.title)
    {
        extras.name = None;
    }

    Some(Some(stack.with_extras(extras)))
}

/// Разбирает сообщение о содержимом слота инвентаря в творческом режиме.
///
/// В творческом режиме клиент сам сообщает, что он положил в слот: меню
/// предметов есть только у него. Поэтому здесь сервер просто записывает
/// сказанное.
///
/// Возвращает предмет, который надо бросить на землю: его выбросили прямо
/// из меню (слот −1).
fn read_creative_slot(payload: &[u8], player: &mut PlayerState) -> Option<Stack> {
    // В выживании предметы берутся из мира, а не из меню. Клиент такого
    // сообщения там и не шлёт, но верить ему на слово нельзя: изменённый
    // клиент мог бы выдать себе что угодно.
    if player.game_mode != GAME_MODE_CREATIVE {
        log_debug!("Play: {} не в творческом режиме — слот не выдаём", player.name);
        return None;
    }

    let mut cursor = Cursor::new(payload);

    let slot = cursor.u16()? as i16;
    let stack = read_untrusted_slot(&mut cursor)?;

    // Номер предмета и число — тоже от клиента. Несуществующий предмет
    // в руке ушёл бы всем остальным и отключил бы их, а число больше стопки
    // ломало бы счёт при делении стопки.
    if let Some(stack) = stack
        && (!known_item(stack.item) || stack.count > crate::blocks::stack_size(stack.item))
    {
        log_debug!(
            "Play: {} прислал недопустимый предмет {} ({} шт.) — не берём",
            player.name,
            stack.item,
            stack.count
        );
        return None;
    }

    if slot == -1 {
        return stack;
    }

    player.inventory.set(i32::from(slot), stack);

    match stack {
        Some(stack) => log_debug!(
            "Play: в слоте {} предмет {} ({} шт.)",
            slot,
            stack.item,
            stack.count
        ),
        None => log_debug!("Play: слот {} опустел", slot),
    }

    None
}

/// Есть ли такой предмет в реестре (воздух — не предмет).
fn known_item(item: i32) -> bool {
    item > 0
        && crate::blocks_table::ITEM_NAMES
            .binary_search_by_key(&item, |(number, _)| *number)
            .is_ok()
}

/// Щелчок в окне хранилища. Как у оригинала (вики, Click Container):
/// щелчок выполняется на сервере, итог сравнивается с тем, что клиент
/// предсказал сам (присланные изменённые ячейки и курсор), и досылаются
/// только несовпавшие ячейки. Предсказание сверяется по предмету и числу:
/// своих довесков у наших предметов нет.
/// Кнопка в окне кафедры: листать (1 — назад, 2 — вперёд, 100 + N — на
/// страницу N) или взять книгу (3). Страница сменилась — клиенту новое
/// свойство окна; книгу взяли — она в инвентаре, окно закрывается.
fn lectern_button(shared: &Shared, player: &mut PlayerState, window: i32, button: i32, reaction: &mut Reaction) {
    let Some(open) = player.window.as_ref().filter(|open| open.id == window && open.lectern) else {
        return;
    };
    let Some(&pos) = open.halves.first() else {
        return;
    };
    let mut world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");

    if button == redstone::lectern::BUTTON_TAKE {
        let Some(book) = redstone::lectern::take_book(&mut world, pos) else {
            return;
        };

        drop(world);

        let (_, left) = player.inventory.put(book);

        if let Some(stack) = left {
            throw_out(shared, player, stack);
        }

        reaction.inventory = true;
        reaction.close_window = Some(window);
        close_own_window(shared, player);
        return;
    }

    if redstone::lectern::turn_page(&mut world, pos, button)
        && let Some((page, _)) = redstone::lectern::page(&world, pos)
    {
        reaction.window_property = Some((window, 0, page as i16));
    }
}

/// Ячейка крафтера включена или выключена щелчком в окне: свойство окна
/// с новым состоянием уходит обратно.
fn crafter_slot_state(shared: &Shared, player: &mut PlayerState, window: i32, slot: i32, enabled: bool, reaction: &mut Reaction) {
    let Some(&pos) = player.window.as_ref().filter(|open| open.id == window).and_then(|open| open.halves.first()) else {
        return;
    };
    let Ok(slot) = usize::try_from(slot) else {
        return;
    };
    let mut world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");

    if redstone::crafter::toggle_slot(&mut world, pos, slot, enabled) {
        reaction.window_property = Some((window, slot as i16, i16::from(!enabled)));
    }
}

fn click_window(shared: &Shared, player: &mut PlayerState, payload: &[u8], reaction: &mut Reaction) {
    // В окне кафедры ячейку не трогают — книгу берут кнопкой.
    if player.window.as_ref().is_some_and(|window| window.lectern) {
        return;
    }

    if player.window.as_ref().is_some_and(|window| window.station.is_some()) {
        station::click(shared, player, payload, reaction);
        return;
    }

    let mut cursor = Cursor::new(payload);

    let (Some(_window), Some(_state_id), Some(slot), Some(button), Some(mode)) =
        (cursor.varint(), cursor.varint(), cursor.u16(), cursor.u8(), cursor.varint())
    else {
        return;
    };

    // Что клиент думает о ячейках и курсоре после щелчка.
    let mut predicted: Vec<(usize, Option<(i32, i32)>)> = Vec::new();
    let mut predicted_cursor = None;
    let mut readable = true;

    if let Some(count) = cursor.varint() {
        for _ in 0..count.clamp(0, 128) {
            let (Some(place), Some(item)) = (cursor.u16(), read_hashed_slot(&mut cursor)) else {
                readable = false;
                break;
            };

            predicted.push((place as i16 as usize, item));
        }

        if readable {
            predicted_cursor = read_hashed_slot(&mut cursor);
        }
    }

    let Some(window) = player.window.as_ref() else {
        return;
    };

    if window.merchant.is_some() {
        merchant::click(shared, player, (slot as i16 as i32, button as i8, mode), &predicted, predicted_cursor, readable, reaction);
        return;
    }

    let halves = window.halves.clone();
    let cart = window.cart;
    let horse_kind = cart.and_then(|id| {
        shared.items_in(player.dimension).lock().expect("предметы").mobs().iter()
            .find(|mob| mob.entity_id == id && crate::mob::horse::equine(mob.kind)).map(|mob| mob.kind)
    });
    if horse_kind.is_some() && cart.and_then(|id| horse::slots(shared, player, id)).is_none() { return; }
    let ender = window.ender;
    let size = window.shown.len();
    let kind = window.kind;
    let feet = (player.x, player.y, player.z);
    let mut xp_points = 0;

    let (before, after, thrown) = {
        let mut world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");
        let mut slots = match cart {
            Some(cart) => cart_slots(shared, player.dimension, cart).unwrap_or_default(),
            None if kind == Some(containers::Kind::Beacon) => window.shown.clone(),
            None if ender => player.ender.clone(),
            None => containers::window_slots(&world, &halves),
        };

        if slots.len() != size {
            return;
        }

        let snapshot = |slots: &[Option<Stack>], inventory: &Inventory| -> Vec<Option<Stack>> {
            let mut all = slots.to_vec();

            for own in (inventory::FIRST_MAIN..=inventory::LAST_MAIN).chain(inventory::FIRST_HOTBAR..=inventory::LAST_HOTBAR) {
                all.push(inventory.slot(own));
            }

            all
        };

        let before = snapshot(&slots, &player.inventory);
        let (slots_before, inventory_before) = (slots.clone(), player.inventory.clone());
        let llama=cart.is_some_and(|id| shared.items_in(player.dimension).lock().expect("мобы").mobs().iter().any(|m| m.entity_id==id && crate::mob::land::llama(m.kind)));
        let mut click = if llama && mode==1 { land::quick_move(&mut slots,&mut player.inventory,slot as i16 as i32) }
            else { window::Window { container: &mut slots, inventory: &mut player.inventory, kind, horse: horse_kind }.click(slot as i16 as i32, button as i8, mode) };

        // В выключенную ячейку крафтера ничего не кладётся (вики, Crafter:
        // «Disabled slots cannot have items put in them») — такой щелчок
        // отменяется целиком.
        if let Some(crafter) = halves.first().and_then(|&(x, y, z)| world.container(x, y, z)).filter(|c| c.kind == containers::Kind::Crafter)
            && slots.iter().enumerate().any(|(index, stack)| stack.is_some() && crafter.is_disabled(index))
        {
            slots.clone_from(&slots_before);
            player.inventory = inventory_before.clone();
            click.thrown = None;
        }

        match cart {
            Some(cart) => {
                let mut items = shared.items_in(player.dimension).lock().expect("предметы захвачены другим потоком");

                if let Some(cart) = items.carts_mut().iter_mut().find(|found| found.entity_id == cart) {
                    cart.slots.clone_from(&slots);
                } else if let Some(mob)=items.mobs_mut().iter_mut().find(|m| m.entity_id==cart && crate::mob::land::llama(m.kind)) {
                    if land::valid(mob,&slots) { land::store(mob,&slots); }
                    else { slots.clone_from(&slots_before); player.inventory=inventory_before; click.thrown=None; }
                } else if horse_kind.is_some() {
                    let found = items.mobs_mut().iter_mut().find(|mob| mob.entity_id == cart && !mob.is_dead()
                        && mob.horse.tamed && !mob.is_baby() && mob.horse.inventory() == slots_before);
                    if let Some(mob) = found {
                        let old = mob.horse.slots;
                        mob.horse.slots[..slots.len()].copy_from_slice(&slots);
                        crate::mob::horse::equipment_changed(mob, old);
                        items.flush_mob_events(&mut || shared.next_entity_id(), &mut crate::mob::Rng::new(cart as u64));
                    } else {
                        // Между чтением и записью команда могла убить лошадь
                        // или сменить снаряжение: предмет не должен удвоиться.
                        slots.clone_from(&slots_before);
                        player.inventory = inventory_before;
                        click.thrown = None;
                    }
                }
            }
            None if kind == Some(containers::Kind::Beacon) => {},
            None if ender => player.ender.clone_from(&slots),
            None => containers::store_window_slots(&mut world, &halves, &slots),
        }

        // Из выхода печи забрали выплавленное — игроку весь накопленный
        // опыт.
        if kind.is_some_and(crate::furnace::is_oven)
            && let Some(&place) = halves.first()
        {
            let count = |slots: &[Option<Stack>]| slots.get(crate::furnace::OUTPUT).copied().flatten().map_or(0, |stack| stack.count);

            if count(&slots) < count(&slots_before) {
                if let Some(stack) = slots_before[crate::furnace::OUTPUT] {
                    player.progress.crafted(stack.item, count(&slots_before) - count(&slots));
                }
                xp_points = crate::furnace::take_experience(&mut world, place);
            }
        }

        if kind == Some(containers::Kind::BrewingStand) {
            for index in 0..3 {
                if let Some(stack) = slots_before[index]
                    && slots[index] != Some(stack)
                    && slots[index].is_none_or(|after| !after.stacks_with(&stack) || after.count < stack.count)
                {
                    player.progress.brewed(stack.item);
                }
            }
        }
        redstone::settle(&mut world);

        (before, snapshot(&slots, &player.inventory), click.thrown)
    };

    if let Some(stack) = thrown {
        player.progress.dropped(stack.item, stack.count);
        throw_out(shared, player, stack);
    }

    // Опыт из печи — шариками у ног сразу, ещё до ответа на щелчок
    // (чёрный ящик, furnace-smelt: появление шарика раньше ячеек окна).
    if xp_points > 0 {
        let mut rng = crate::mob::Rng::new(u64::from(shared.next_entity_id() as u32) ^ (*shared.ticks.borrow() << 20));

        shared
            .items_in(player.dimension)
            .lock()
            .expect("предметы захвачены другим потоком")
            .drop_experience(xp_points, feet, &mut || shared.next_entity_id(), &mut rng);
    }

    if let Some(window) = player.window.as_mut() {
        let crafted = window.crafter.then(|| (crate::crafting::craft(&window.shown), crate::crafting::craft(&after[..size])));

        window.shown = after[..size].to_vec();

        if let Some((was, now)) = crafted
            && was != now
        {
            reaction.window_slots.push((window.result_slot(), now));
        }
    }

    // Чего клиент ждёт в каждой ячейке: предсказанное — там, где он его
    // прислал, прежнее — в остальных.
    let as_pair = |stack: Option<Stack>| stack.map(|stack| (stack.item, stack.count));

    for (index, stack) in after.iter().enumerate() {
        let expected = predicted
            .iter()
            .rev()
            .find(|(place, _)| *place == index)
            .map_or(as_pair(before[index]), |(_, item)| *item);

        if !readable || expected != as_pair(*stack) {
            reaction.window_slots.push((index, *stack));
        }
    }

    if !readable || predicted_cursor != Some(as_pair(player.inventory.cursor())) {
        reaction.cursor = Some(player.inventory.cursor());
    }
}

/// Предмет в щелчке (Hashed Slot): есть ли, номер, число, довески с
/// контрольными суммами. Возвращает (номер, число) или None у пустого;
/// внешний None — пакет оборван.
fn read_hashed_slot(cursor: &mut Cursor) -> Option<Option<(i32, i32)>> {
    if cursor.u8()? == 0 {
        return Some(None);
    }

    let item = cursor.varint()?;
    let count = cursor.varint()?;

    for _ in 0..cursor.varint()?.clamp(0, 256) {
        cursor.varint()?;
        cursor.u32()?;
    }

    for _ in 0..cursor.varint()?.clamp(0, 256) {
        cursor.varint()?;
    }

    Some(Some((item, count)))
}

/// Разбирает щелчок по слоту в открытом окне.
///
/// Возвращает false, если щелчок такой, какого сервер не понимает: тогда
/// клиенту пересылается содержимое инвентаря с сервера.
fn read_click(payload: &[u8], player: &mut PlayerState) -> inventory::Click {
    let mut cursor = Cursor::new(payload);

    let Some(window) = cursor.varint() else {
        return inventory::Click::default();
    };

    // Чужие окна сервер пока не открывает, значит и щелчков в них быть не может.
    if window != PLAYER_WINDOW {
        return inventory::Click::default();
    }

    let (Some(_state_id), Some(slot), Some(button), Some(mode)) = (
        cursor.varint(),
        cursor.u16(),
        cursor.u8(),
        cursor.varint(),
    ) else {
        return inventory::Click::default();
    };

    if player.game_mode == GAME_MODE_CREATIVE {
        player.inventory.click_in_mode(slot as i16 as i32, button as i8, mode, true)
    } else {
        player.inventory.click(slot as i16 as i32, button as i8, mode)
    }
}

/// Записывает байты в виде шестнадцатеричных чисел — для разбора неизвестных
/// пакетов по логу.
fn format_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{:02X}", byte))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Читатель полей пакета по порядку: сам помнит, сколько уже прочитано.
///
/// Поля в пакете идут строго друг за другом, и разбирать их вручную со счётом
/// смещений — верный способ ошибиться на байт. Курсор делает это за нас.
struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn varint(&mut self) -> Option<i32> {
        let (value, read) = decode_varint(&self.bytes[self.offset..]).ok()?;

        self.offset += read;

        Some(value)
    }

    fn u8(&mut self) -> Option<u8> {
        let value = *self.bytes.get(self.offset)?;

        self.offset += 1;

        Some(value)
    }

    fn u16(&mut self) -> Option<u16> {
        let bytes: [u8; 2] = self.bytes.get(self.offset..self.offset + 2)?.try_into().ok()?;

        self.offset += 2;

        Some(u16::from_be_bytes(bytes))
    }

    fn u32(&mut self) -> Option<u32> {
        let bytes: [u8; 4] = self.bytes.get(self.offset..self.offset + 4)?.try_into().ok()?;

        self.offset += 4;

        Some(u32::from_be_bytes(bytes))
    }

    fn f32(&mut self) -> Option<f32> {
        let bytes: [u8; 4] = self.bytes.get(self.offset..self.offset + 4)?.try_into().ok()?;

        self.offset += 4;

        Some(f32::from_be_bytes(bytes))
    }

    /// Следующие `length` байт как есть.
    fn bytes(&mut self, length: usize) -> Option<&'a [u8]> {
        let value = self.bytes.get(self.offset..self.offset.checked_add(length)?)?;

        self.offset += length;

        Some(value)
    }

    /// Строка протокола: длина в байтах, затем сами байты в UTF-8.
    fn string(&mut self) -> Option<String> {
        let length = usize::try_from(self.varint()?).ok()?;
        let end = self.offset + length;

        let value = String::from_utf8_lossy(self.bytes.get(self.offset..end)?).into_owned();
        self.offset = end;

        Some(value)
    }

    /// Координаты блока: одно long, в котором упакованы X, Y и Z.
    ///
    /// Раскладка битов: X занимает 26 старших бит, Z — 26 средних, Y — 12
    /// младших. Все три поля знаковые, поэтому сдвиг влево-вправо растягивает
    /// знаковый бит обратно.
    fn position(&mut self) -> Option<(i32, i32, i32)> {
        let bytes: [u8; 8] = self.bytes.get(self.offset..self.offset + 8)?.try_into().ok()?;
        let packed = i64::from_be_bytes(bytes);

        self.offset += 8;

        // X лежит в старших битах: обычный знаковый сдвиг уже даёт верное число.
        let x = (packed >> 38) as i32;
        // Z — средние 26 бит: сдвигаем их в старшие, затем знаково возвращаем.
        let z = ((packed << 26) >> 38) as i32;
        // Y — младшие 12 бит.
        let y = ((packed << 52) >> 52) as i32;

        Some((x, y, z))
    }

    /// Номер действия игрока — последнее поле пакета.
    ///
    /// Перед ним в пакете установки блока стоит поле «щелчок по границе мира»,
    /// которого в нашей версии протокола может и не быть: документация описана
    /// для более новой версии. Отличить их можно по остатку — правильный разбор
    /// тот, после которого в пакете не остаётся лишних байт.
    fn sequence(&mut self) -> Option<i32> {
        let rest = &self.bytes[self.offset..];

        if let Ok((value, read)) = decode_varint(rest)
            && read == rest.len()
        {
            self.offset += read;
            return Some(value);
        }

        // Вариант с полем «щелчок по границе мира» перед номером действия.
        if rest.len() >= 2
            && let Ok((value, read)) = decode_varint(&rest[1..])
            && read + 1 == rest.len()
        {
            self.offset += read + 1;
            return Some(value);
        }

        None
    }
}

/// Что сервер знает о самом игроке: где он стоит и куда смотрит.
///
/// Физику сервер не считает — клиент сам присылает своё положение, а сервер
/// только запоминает присланное. Пока это нужно для логов; из этих же данных
/// вырастут и столкновения, и рассылка положения другим игрокам.
///
/// Положение записывается на диск сразу, как только игрок заметно сдвинулся,
/// — тем же способом, каким сохраняются блоки. Поэтому игрок после перезапуска
/// сервера оказывается там, где стоял, а не в точке появления.
// Одна рассылка читает одну границу, даже если запись большого пакета
// уступит исполнение следующему такту мира.
struct TrackingPublication {
    tick: u64,
    events: Option<usize>,
    players: Vec<(Member, crate::players::Gear)>,
}

struct PlayerState {
    publication: Option<TrackingPublication>,
    shoulders: [Option<Vec<u8>>; 2],
    shoulder_since: u64,
    shoulder_health: f32,
    shoulder_shown: HashMap<i32, [i32; 2]>,
    creative_flying: bool,
    /// Опознаватель игрока — по нему он отличается от остальных в общем списке
    /// и по нему же назван его файл на диске.
    uuid: [u8; 16],

    /// Имя игрока, как его показать в списке и в чате.
    name: String,

    /// Режим игры: от него зависит, например, как игрок ломает блоки.
    game_mode: i32,

    x: f64,
    y: f64,
    z: f64,
    yaw: f32,
    pitch: f32,
    on_ground: bool,
    mining: Option<mining::Session>,
    delayed: Option<mining::Session>,

    /// Номер сущности игрока. Его выдаёт список игроков при входе: тот же
    /// номер получают остальные, чтобы показать этого игрока.
    entity_id: i32,

    /// Номер этого подключения как читателя общих журналов.
    reader: journal::Reader,

    /// Приседает ли игрок сейчас. Присев, щёлкают не по рычагу и не по двери,
    /// а ставят блок рядом с ними.
    sneaking: bool,

    /// В каком измерении игрок.
    dimension: crate::dimension::Dimension,

    /// Сколько тактов подряд игрок стоит в портале, и можно ли порталу
    /// сработать: после перехода — только когда игрок из портала выйдет.
    in_portal: u32,
    portal_ready: bool,

    /// Полоса босса дракона показана клиенту.
    boss_shown: bool,

    /// Номер последнего переноса и ждём ли его подтверждения. Пока клиент
    /// не подтвердил перенос, его пакеты движения посланы ещё со старого
    /// места — принять их значило бы вернуть игрока назад.
    teleport: i32,
    awaiting_teleport: bool,
    /// Взмах руки самому игроку: уходит перед слежением за мобами (после звуков).
    swing_pending: bool,
    /// Такт последнего взмаха, показанного самому игроку (взмах не чаще раза в 3 такта).
    last_swing: u64,

    /// Присылал ли клиент пакет места после последнего переноса. Оригинал
    /// замечает портал, только когда игрок двигается (`move` → блоки
    /// внутри), а не сразу после `tp`: вход в портал Края — через два такта
    /// после переноса в него (чёрный ящик, end-portal-enter).
    moved_since_tp: bool,

    /// Переход идёт порталом, а не командой: у пакета места другие флаги, а в
    /// Respawn стоит откат портала (чёрный ящик, end-portal-enter).
    portal_trip: bool,

    /// После смены измерения оригинал досылает здоровье и опыт, когда игрок
    /// впервые тикает в новом мире: порталом — сразу за выбранным слотом,
    /// командой — в конце такта, после полосы босса (end-fight-enter); при
    /// долгой загрузке чанков у оригинала и на 1, 3, 30 тактов позже —
    /// случайное. Тут — сколько концов тактов ждать.
    vitals_due: u8,


    /// Где игрок по серверу после того, как его сдвинул поршень (tick.rs):
    /// ближайшее место от клиента с этим сверяется (`SET_PLAYER_POSITION`).
    shoved_to: Option<(f64, f64, f64)>,

    /// Номер последнего действия, которым из ведра вылили: тот же щелчок
    /// следом присылает Use Item, и вылитое не должно тут же зачерпнуться.
    poured: Option<i32>,

    /// Вагонетки, показанные клиенту, — какими он их знает (minecart.rs).
    seen_carts: HashMap<i32, ShownCart>,

    /// Рамки, показанные клиенту (frame.rs).
    seen_frames: HashMap<i32, ShownFrame>,

    /// Мобы, показанные клиенту (mob/mod.rs).
    seen_mobs: HashMap<i32, mobs::ShownMob>,

    /// Сидение: на чём едет игрок и каких седоков клиент уже видит (riding.rs).
    ride: vehicle::Ride,

    /// Полёт на элитрах (play/glide.rs).
    glide: glide::Glide,

    /// Стрелы, показанные клиенту (arrow.rs).
    seen_bow_arrows: HashMap<i32, bow::ShownArrow>,
    /// Брошенные зелья, огненные шары и облака, о которых клиент знает.
    seen_thrown: HashMap<i32, thrown::Shown>,
    /// Откат предметов: группа (имя предмета) → такт, до которого её нельзя
    /// использовать (пакет Set Cooldown; заряд ветра — thrown.rs).
    item_cooldowns: HashMap<&'static str, u64>,
    seen_fishing: HashMap<i32, fishing::Shown>,
    seen_maps: HashMap<i32, Vec<u8>>,
    /// Такт, когда карта впервые ушла клиенту: столбец 0 доходит через 15 тактов.
    map_first: HashMap<i32, u64>,
    seen_map_icons: HashMap<i32, Vec<(i32, i8, i8, u8)>>,

    /// Такт мира, когда игрок последний раз бил: сила удара зависит от
    /// того, сколько прошло.
    last_attack: u64,

    /// Когда клиент последний раз ответил на Keep Alive.
    answered: Instant,

    /// Когда игрок последний раз что-то делал (`setidletimeout`).
    idle: Instant,

    /// Инвентарь игрока: сервер ведёт его сам и показывает клиенту.
    inventory: Inventory,

    /// Счётчик состояний окна. Клиент возвращает его в щелчке, а сервер
    /// увеличивает при каждой посылке содержимого: по нему клиент понимает,
    /// что картинка у него свежая.
    state_id: i32,

    /// Последнее положение, о котором сервер уже написал в лог. Клиент шлёт
    /// пакет о перемещении каждый тик, поэтому без этой проверки лог
    /// превратился бы в поток одинаковых строк.
    reported: Option<(i32, i32, i32)>,

    /// Стоит ли игрок на месте. Пакет о перемещении приходит каждый тик, так
    /// что два одинаковых подряд означают остановку. Остановку стоит записать:
    /// иначе на диске останется место, где игрок перешёл в этот блок, а не
    /// там, где он встал.
    stopped: bool,

    /// Скин игрока: сервер узнаёт его один раз при заходе и дальше только
    /// пересказывает клиентам.
    skin: Option<Skin>,

    /// Какие части скина игрок показывает: это его настройка, и она нужна
    /// остальным, чтобы увидеть второй слой его скина.
    skin_parts: u8,

    /// Ведущая рука: 0 — левая, 1 — правая. Тоже настройка клиента, которую
    /// сервер пересказывает остальным.
    main_hand: u8,

    /// Сколько чанков вокруг слать этому игроку: меньшая из его дальности
    /// прорисовки и серверной, но не меньше двух — как у оригинала. Слать
    /// дальше, чем клиент рисует, — значит заставить его держать в памяти
    /// в разы больше чанков, чем он показывает (и падать от нехватки памяти).
    view: i32,

    /// Что об игроке уже лежит на диске: прочитанное при заходе или записанное
    /// после. Нынешнее положение сравнивается именно с этим — записывается
    /// только то, что от него отличается.
    saved: Option<PlayerData>,

    /// Точка появления мира: её сервер сообщает клиенту, и по ней клиент
    /// рисует стрелку компаса.
    spawn_point: (f64, f64, f64),

    /// Личная точка возрождения: кровать, якорь, `spawnpoint` (bed.rs).
    respawn: Option<crate::bed::RespawnPoint>,

    /// Табличка, чьё окно правки открыто у игрока (Update Sign принимается
    /// только для неё).
    sign_editor: Option<(i32, i32, i32)>,

    /// Эндер-сундук игрока: один на всех эндер-сундуков мира, лежит в файле
    /// игрока (вики, Ender Chest).
    ender: Vec<Option<Stack>>,

    /// Спит: изголовье кровати (play/sleep.rs).
    sleeping: Option<(i32, i32, i32)>,

    /// Ночь пропущена — встать в конце такта (play/sleep.rs, `tick`).
    wake_due: bool,

    /// Открытое окно хранилища, если есть.
    window: Option<OpenWindow>,

    /// Изменения блоков, ждущие метки рассылки (`WorldEvent::Broadcast`).
    pending_changes: Vec<world::BlockChange>,

    /// Данные блок-сущностей, которые уйдут после ближайшей пачки.
    pending_entities: Vec<PendingEntity>,

    /// Инвентарь менялся, пока было открыто окно хранилища, так, что окно
    /// этого не показало (броня, вторая рука): после закрытия прислать
    /// инвентарь целиком.
    inventory_after_window: bool,

    /// Номер последнего открытого окна — следующее получит номер на один
    /// больше.
    window_counter: i32,

    /// Здоровье, еда, воздух, огонь (survival.rs).
    vit: crate::survival::Vitals,

    /// Что из этого уже рассказано клиенту и что ждёт своего такта.
    net: vitals::Net,

    /// Режим игры до последней смены (нужен пакету Respawn); −1 — не было.
    prev_mode: i32,

    /// Атрибуты игрока: основа и модификаторы от предметов и бега
    /// (attributes.rs). Клиенту уходит то, что изменилось (`attrs_sent`).
    attrs: crate::attributes::Attributes,

    /// Какие модификаторы поставили предметы: их снимают перед новой расстановкой.
    attrs_applied: Vec<(i32, String)>,

    /// Что клиенту об атрибутах уже сообщено.
    attrs_sent: crate::attributes::Sent,

    /// Экипировка, по которой посчитаны модификаторы: рука, вторая рука, броня.
    equip_seen: Vec<Option<Stack>>,

    /// Атрибуты, чьи модификаторы от предметов снимались и ставились заново:
    /// клиенту они уходят, даже если значение то же.
    attrs_touched: Vec<(i32, u64)>,

    /// Такты, когда скорость удара повторится чужим клиентам.
    speed_due: Vec<u64>,

    /// Сколько раз перестанавливались модификаторы скорости удара: чужим
    /// клиентам это повод повторить атрибут.
    speed_touch: u32,

    /// Статистика и достижения игрока (progress.rs).
    progress: crate::progress::Progress,
    progress_location: progression::Location,
}

/// Окно хранилища, открытое у клиента.
struct OpenWindow {
    /// Номер окна у клиента.
    id: i32,

    /// Места хранилища (половины сундука) в порядке ячеек окна.
    halves: Vec<(i32, i32, i32)>,

    /// Транспорт (вагонетка или лошадь), чьё это окно (тогда мест нет).
    cart: Option<i32>,

    /// Ячейки хранилища, какими их знает клиент: по ним видно, что
    /// досылать.
    shown: Vec<Option<Stack>>,

    /// Окно кафедры: одна ячейка без инвентаря игрока и свойство
    /// «страница».
    lectern: bool,

    /// Окно крафтера: после рюкзака и панели — ячейка с тем, что выйдет
    /// (чёрный ящик, crafter-slots: ячейка 45).
    crafter: bool,

    /// Вид хранилища (у вагонетки — None): правила ячеек печи и стойки.
    kind: Option<containers::Kind>,

    /// Свойства окна печи или стойки, какими их знает клиент.
    props: Vec<i16>,

    /// Окно торговли с жителем: ячейки оплаты и выхода, выбранная сделка
    /// (merchant.rs). Тогда `shown` — эти три ячейки.
    merchant: Option<crate::villager::Merchant>,

    /// Окно станка (стол зачарования, наковальня, точило): предметы лежат
    /// в `shown`, хранилища нет (station.rs).
    station: Option<station::StationWindow>,

    /// Окно эндер-сундука: ячейки — личные, `PlayerState::ender`; `halves` —
    /// только сам блок (смотрящие и крышка).
    ender: bool,
}

impl OpenWindow {
    /// Номер ячейки «что выйдет» у окна крафтера.
    fn result_slot(&self) -> usize {
        self.shown.len() + window::PLAYER_SLOTS
    }
}

impl PlayerState {
    /// Заводит запись об игроке. `saved` — то, что осталось от прошлого
    /// захода: оттуда берутся место, поворот и режим игры. Если игрок здесь
    /// впервые, он появляется в точке появления.
    fn new(
        uuid: [u8; 16],
        name: String,
        saved: Option<PlayerData>,
        skin: Option<Skin>,
        skin_parts: u8,
        reader: journal::Reader,
        spawn: (f64, f64, f64),
    ) -> Self {
        // Игрок, сохранённый ниже дна мира, вернулся бы в пустоту и падал
        // вечно. Такое бывает после того, как мир поменялся, — ставим его
        // в точку появления.
        let saved = saved.filter(|data| {
            let in_the_void = data.y < world::MIN_Y as f64;

            if in_the_void {
                log_debug!(
                    "Play: место игрока ({:.2} {:.2} {:.2}) оказалось под миром — ставим в точку появления",
                    data.x,
                    data.y,
                    data.z
                );
            }

            !in_the_void
        });

        let (x, y, z, yaw, pitch, game_mode) = match &saved {
            Some(data) => (
                data.x,
                data.y,
                data.z,
                data.yaw,
                data.pitch,
                data.game_mode,
            ),
            None => (
                spawn.0,
                spawn.1,
                spawn.2,
                SPAWN_YAW,
                SPAWN_PITCH,
                crate::commands_admin::default_game_mode(),
            ),
        };

        // Здоровье, еда, воздух и огонь — как при выходе.
        let mut vit = saved.as_ref().map(|data| crate::survival::Vitals::restored(&data.vitals)).unwrap_or_default();

        // Семя зачарования 0 — не заведено: завести (вики, Enchanting
        // mechanics: «seed of 0 … will be regenerated»).
        if vit.xp_seed == 0 {
            vit.xp_seed = crate::enchanting::new_seed(u64::from_be_bytes(uuid[..8].try_into().unwrap_or_default()));
        }

        Self {
            spawn_point: spawn,
            respawn: saved.as_ref().and_then(|data| data.respawn),
            ender: saved.as_ref().map_or_else(playerdata::empty_ender, |data| data.ender.clone()),
            sign_editor: None,
            sleeping: None,
            wake_due: false,
            uuid,
            name,
            game_mode,
            x,
            y,
            z,
            yaw,
            pitch,
            on_ground: false,
            mining: None,
            delayed: None,
            entity_id: 0,
            // Инвентарь — тот, с которым игрок вышел в прошлый раз.
            reader,
            sneaking: false,
            teleport: 0,
            awaiting_teleport: false,
            swing_pending: false,
            last_swing: 0,
            moved_since_tp: true,
            portal_trip: false,
            vitals_due: 0,
            shoved_to: None,
            poured: None,
            seen_carts: HashMap::new(),
            seen_frames: HashMap::new(),
            seen_mobs: HashMap::new(),
            publication: None,
            shoulders: saved.as_ref().map(|s| s.shoulders.clone()).unwrap_or([None,None]),
            shoulder_since: 0,
            shoulder_health: vit.health,
            shoulder_shown: HashMap::new(),
            creative_flying: false,
            ride: vehicle::Ride::default(),
            glide: glide::Glide::default(),
            seen_bow_arrows: HashMap::new(),
            seen_thrown: HashMap::new(),
            item_cooldowns: HashMap::new(),
            seen_fishing: HashMap::new(),
            seen_maps: HashMap::new(),
            map_first: HashMap::new(),
            seen_map_icons: HashMap::new(),
            last_attack: 0,
            dimension: saved.as_ref().map(|data| data.dimension).unwrap_or_default(),
            in_portal: 0,
            // Зашёл, стоя в портале, — не переносить сразу.
            portal_ready: false,
            boss_shown: false,
            answered: Instant::now(),
            idle: Instant::now(),
            inventory: saved
                .as_ref()
                .map(|data| data.inventory.clone())
                .unwrap_or_else(Inventory::new),
            state_id: 0,
            window: None,
            pending_changes: Vec::new(),
            pending_entities: Vec::new(),
            inventory_after_window: false,
            window_counter: 0,
            net: vitals::Net::synced(&vit),
            vit,
            prev_mode: -1,
            attrs: crate::attributes::Attributes::player(),
            attrs_applied: Vec::new(),
            attrs_sent: crate::attributes::Sent::of(&crate::attributes::Attributes::player()),
            equip_seen: Vec::new(),
            attrs_touched: Vec::new(),
            speed_due: Vec::new(),
            speed_touch: 0,
            progress: crate::progress::Progress::load(&uuid),
            progress_location: progression::Location::default(),
            skin,
            skin_parts,
            main_hand: configuration::RIGHT_HAND,
            view: i32::MAX,
            reported: None,
            stopped: false,
            saved,
        }
    }

    /// Запись игрока для общего списка.
    fn member(&self) -> Member {
        Member {
            uuid: self.uuid,
            name: self.name.clone(),
            game_mode: self.game_mode,
            // Номер сущности выдаёт список игроков при входе.
            entity_id: 0,
            skin: self.skin.clone(),
            skin_parts: self.skin_parts,
            main_hand: self.main_hand,
            held: self.inventory.held(),
            position: self.position(),
            dimension: self.dimension,
            sneaking: self.sneaking,
            swings: 0,
            shoved: ((0.0, 0.0, 0.0), 0),
            unsettled: 0,
        }
    }

    /// Вторая рука, броня и бой — в том виде, в каком это видят остальные.
    fn gear(&self) -> crate::players::Gear {
        let slot = |number: i32| self.inventory.slot(number);
        let armor_data: Vec<crate::stackdata::Data> = crate::combat::ARMOR_SLOTS.iter().filter_map(|(number, _)| slot(*number)).map(|stack| stack.extras()).collect();
        let points = |source| armor_data.iter().map(|data| crate::combat::protection_points(data, source)).sum::<i32>();

        crate::players::Gear {
            worn: [slot(inventory::OFFHAND), slot(5), slot(6), slot(7), slot(8)],
            view: crate::players::CombatView {
                invulnerable: self.vit.invulnerable,
                last_hurt: self.vit.last_hurt(),
                dead: self.vit.dead,
                armor: self.attrs.value(crate::attributes::ARMOR) as f32,
                toughness: self.attrs.value(crate::attributes::ARMOR_TOUGHNESS) as f32,
                protection: points(crate::survival::Source::PlayerAttack),
                projectile_protection: points(crate::survival::Source::Arrow) - points(crate::survival::Source::PlayerAttack),
                resistance: self.vit.resistance_level(),
                thorns: {
                    let mut levels = [0; 4];

                    for (level, data) in levels.iter_mut().zip(crate::combat::ARMOR_SLOTS.iter().map(|(number, _)| slot(*number).map(|stack| stack.extras()))) {
                        *level = data.map_or(0, |data| data.level("thorns"));
                    }

                    levels
                },
                blocking_since: self.net.using.filter(|using| using.kind == vitals::UseKind::Shield).map(|using| using.since),
                shield_disabled_until: self.net.shield_disabled_until,
            },
            attack_speed: self.attrs.get(crate::attributes::ATTACK_SPEED).cloned(),
            speed_touch: self.speed_touch,
            living: vitals::living_flags(self) | if self.net.spin_pose { crate::players::SPIN_POSE_BIT } else { 0 } | if self.glide.active { crate::players::GLIDE_BIT } else { 0 },
            air: self.vit.air,
            sleeping: self.sleeping,
            effect_colors: self.vit.effect_colors(),
            health: self.net.announced_health.unwrap_or(self.vit.health),
            sprinting: self.vit.sprinting,
            movement_speed: self.attrs.get(crate::attributes::MOVEMENT_SPEED).cloned().map(|mut attribute| {
                // Модификатор бега — по текущему флагу, а не по последней рассылке.
                attribute.modifiers.retain(|modifier| modifier.id != "minecraft:sprinting");

                if self.vit.sprinting {
                    attribute.modifiers.push(crate::attributes::Modifier::new("minecraft:sprinting", f64::from(0.3f32), crate::attributes::Operation::AddMultipliedTotal));
                }

                attribute
            }),
        }
    }

    /// Где игрок стоит и куда смотрит — в том виде, в каком это видят остальные.
    fn position(&self) -> Position {
        Position {
            x: self.x,
            y: self.y,
            z: self.z,
            yaw: self.yaw,
            pitch: self.pitch,
            on_ground: self.on_ground,
        }
    }

    /// Номер чанка, в котором стоит игрок: вокруг него клиенту и нужны чанки.
    fn chunk(&self) -> (i32, i32) {
        let (x, _, z) = block_of(self.x, self.y, self.z);

        (x.div_euclid(CHUNK_SIZE), z.div_euclid(CHUNK_SIZE))
    }

    /// Запоминает положение и пишет в лог, если игрок заметно сдвинулся.
    fn set_position(&mut self, x: f64, y: f64, z: f64) {
        // Пакет приходит каждый тик, поэтому такое же положение, как в прошлом
        // пакете, означает, что игрок остановился.
        self.stopped = same_position((self.x, self.y, self.z), (x, y, z));

        self.x = x;
        self.y = y;
        self.z = z;

        // Сравниваем по десятым долям блока: мелкое дрожание при ходьбе
        // интереса не представляет.
        let rounded = ((x * 10.0) as i32, (y * 10.0) as i32, (z * 10.0) as i32);

        if self.reported != Some(rounded) {
            self.reported = Some(rounded);
            log_debug!("Play: игрок в {:.2} {:.2} {:.2}", x, y, z);
        }
    }

    /// Запоминает поворот и пишет в лог, если игрок заметно повернулся.
    fn set_rotation(&mut self, yaw: f32, pitch: f32) {
        // Не число ушло бы всем остальным в повороте головы.
        if !(yaw.is_finite() && pitch.is_finite()) {
            return;
        }

        let turned = (self.yaw - yaw).abs() >= 5.0 || (self.pitch - pitch).abs() >= 5.0;

        self.yaw = yaw;
        self.pitch = pitch;

        if turned {
            log_debug!(
                "Play: игрок смотрит (поворот {:.0}, наклон {:.0})",
                yaw, pitch
            );
        }
    }

    /// Запоминает, стоит игрок на земле или нет.
    fn set_on_ground(&mut self, on_ground: bool) {
        if self.on_ground != on_ground {
            log_debug!(
                "Play: игрок {}",
                if on_ground { "на земле" } else { "в воздухе" }
            );
        }

        self.on_ground = on_ground;
    }

    /// Стоит ли записывать положение на диск.
    ///
    /// Запись идёт не на каждое движение: клиент присылает положение двадцать
    /// раз в секунду, и записывать каждый тик было бы незачем. Записываем то,
    /// что игрок заметит, вернувшись: переход в другой блок, остановку,
    /// заметный поворот и смену режима игры.
    fn needs_save(&self) -> bool {
        // О игроке на диске ещё ничего нет — записываем.
        let Some(saved) = &self.saved else {
            return true;
        };

        // Игрок перешёл в другой блок.
        if block_of(saved.x, saved.y, saved.z) != block_of(self.x, self.y, self.z) {
            return true;
        }

        // Игрок остановился внутри блока: запоминаем, где именно. Иначе при
        // заходе он оказался бы у границы, через которую шёл.
        if self.stopped && !same_position((saved.x, saved.y, saved.z), (self.x, self.y, self.z)) {
            return true;
        }

        // Заметно повернулся, сменил режим игры или тронул инвентарь.
        turn_between(saved.yaw, self.yaw) >= SAVED_TURN
            || (saved.pitch - self.pitch).abs() >= SAVED_TURN
            || saved.game_mode != self.game_mode
            || saved.inventory != self.inventory
            || saved.ender != self.ender
            || saved.shoulders != self.shoulders
            || saved.vitals.health != self.vit.health
            || saved.vitals.food != self.vit.food
    }

    /// Записывает положение на диск, если оно отличается от того, что там уже
    /// лежит. Так же, как сохраняется мир: сразу, а не только при выходе, —
    /// сервер может и не успеть сохранить, если его выключат.
    fn save_if_needed(&mut self, directory: &Path) {
        if self.needs_save() {
            self.save(directory);
        }
    }

    /// Записывает положение на диск. Зовётся при выходе игрока — тогда
    /// записывается в любом случае, даже если ничего не изменилось.
    fn save(&mut self, directory: &Path) {
        let data = PlayerData {
            shoulders: self.shoulders.clone(),
            x: self.x,
            y: self.y,
            z: self.z,
            yaw: self.yaw,
            pitch: self.pitch,
            game_mode: self.game_mode,
            inventory: self.inventory.clone(),
            dimension: self.dimension,
            vitals: self.vit.saved(),
            respawn: self.respawn,
            ender: self.ender.clone(),
        };

        self.progress.save(&self.uuid);

        match playerdata::save(directory, &self.uuid, &data) {
            Ok(()) => self.saved = Some(data),
            Err(e) => log_error!(
                "Не удалось сохранить положение игрока {}: {}",
                self.name,
                e
            ),
        }
    }
}

/// Ставит точку на землю: с высоты `y` вниз до первого твёрдого места (у
/// плоского мира точка появления берётся по обычной высоте и висела бы над
/// землёй — выжившему это стоило бы жизни).
pub(crate) fn on_the_ground(shared: &Shared, (x, y, z): (f64, f64, f64)) -> (f64, f64, f64) {
    let mut world = shared.world_in(Dimension::Overworld).lock().expect("мир захвачен другим потоком");

    world.ensure((x.floor() as i32).div_euclid(CHUNK_SIZE), (z.floor() as i32).div_euclid(CHUNK_SIZE));

    let (bx, bz) = (x.floor() as i32, z.floor() as i32);
    let mut by = y.floor() as i32;

    // Начали внутри блока — поднимаемся из него.
    while by < world::MIN_Y + 384 && world.get_block(bx, by, bz) != world::AIR {
        by += 1;
    }

    // Иначе спускаемся, пока под ногами пусто.
    while by > world::MIN_Y && world.get_block(bx, by - 1, bz) == world::AIR {
        by -= 1;
    }

    (x, f64::from(by), z)
}

/// Номер блока, в котором находится точка.
fn block_of(x: f64, y: f64, z: f64) -> (i32, i32, i32) {
    (x.floor() as i32, y.floor() as i32, z.floor() as i32)
}

/// Одно ли это и то же место.
///
/// Точного совпадения дробных чисел ждать не приходится, а миллиметр заведомо
/// меньше, чем игрок способен заметить.
fn same_position(from: (f64, f64, f64), to: (f64, f64, f64)) -> bool {
    (from.0 - to.0).abs() < SAME_POSITION
        && (from.1 - to.1).abs() < SAME_POSITION
        && (from.2 - to.2).abs() < SAME_POSITION
}

/// Насколько игрок повернулся вокруг вертикали, в градусах.
///
/// Считается по короткой стороне: 350° и 10° отличаются на 20°, а не на 340°,
/// — иначе на каждом обороте через ноль положение записывалось бы зря.
fn turn_between(from: f32, to: f32) -> f32 {
    let turn = (to - from).rem_euclid(360.0);

    turn.min(360.0 - turn)
}

/// Читает координаты из пакета о перемещении: X, Y и Z идут первыми тремя
/// double. Остальное содержимое пакета серверу пока не нужно.
fn read_position(body: &[u8]) -> Option<(f64, f64, f64)> {
    Some((read_f64(body, 0)?, read_f64(body, 8)?, read_f64(body, 16)?))
}

/// Читает углы поворота: сначала поворот вокруг вертикали, затем наклон.
fn read_rotation(body: &[u8], offset: usize) -> Option<(f32, f32)> {
    Some((read_f32(body, offset)?, read_f32(body, offset + 4)?))
}

fn read_f32(body: &[u8], offset: usize) -> Option<f32> {
    let bytes: [u8; 4] = body.get(offset..offset + 4)?.try_into().ok()?;

    Some(f32::from_be_bytes(bytes))
}

fn read_f64(body: &[u8], offset: usize) -> Option<f64> {
    let bytes: [u8; 8] = body.get(offset..offset + 8)?.try_into().ok()?;

    Some(f64::from_be_bytes(bytes))
}

/// Человекочитаемое имя serverbound-пакета фазы Play (protocol 775).
/// Нужно только для понятных логов.
fn packet_name(packet_id: i32) -> &'static str {
    match packet_id {
        0x00 => "Confirm Teleportation",
        0x0D => "Client Tick End",
        0x0E => "Client Information",
        0x1C => "Keep Alive",
        0x1E => "Set Player Position",
        0x1F => "Set Player Position and Rotation",
        0x20 => "Set Player Rotation",
        0x21 => "Set Player On Ground",
        0x29 => "Player Action",
        0x2C => "Player Loaded",
        0x35 => "Set Carried Item",
        0x42 => "Use Item On",
        0x43 => "Use Item",
        _ => "неизвестный",
    }
}

/// Новые мобы: снежки, сбруя, воск и топор (minecraft.wiki) [проверить].
fn use_new_mob(shared: &Shared, player: &mut PlayerState, items: &mut crate::items::Items, index: usize, reaction: &mut Reaction) -> bool {
    let held = player.inventory.held();
    let name = held.and_then(|s| blocks::item_name(s.item));
    let mob = &mut items.mobs_mut()[index];
    let mut consumed = false;
    let mut used = false;
    let mut wear = false;
    if mob.kind == crate::mob::Kind::HappyGhast {
        if name == Some("snowball") && crate::mob::newmobs::feed_snowball(mob) { consumed=true; used=true; }
        else if !mob.is_baby() && mob.newmobs.harness.is_none() && let Some(c) = name.and_then(crate::mob::newmobs::harness_colour) {
            mob.newmobs.harness=Some(c); mob.newmobs.home=Some((mob.x,mob.y,mob.z)); consumed=true; used=true;
            mob.events.push(crate::mob::Event::Sound { name:"minecraft:entity.happy_ghast.equip",volume:1.0,pitch:1.0 });
        } else if name == Some("shears") && mob.newmobs.harness.is_some() && !mob.newmobs.ridden {
            let c=mob.newmobs.harness.take().unwrap();
            mob.newmobs.home=Some((mob.x,mob.y,mob.z));
            if let Some(item)=blocks::item_named(&format!("{}_harness",crate::mob::newmobs::COLOURS[c as usize])) {mob.events.push(crate::mob::Event::Loot(vec![Stack::new(item,1)]));}
            used=true; wear=true;
            mob.events.push(crate::mob::Event::Sound {name:"minecraft:entity.happy_ghast.unequip",volume:1.0,pitch:1.0});
        } else if !mob.is_baby() && mob.newmobs.harness.is_some() && !player.sneaking {
            shared.riding.lock().expect("учёт седоков").mount(player.entity_id,mob.entity_id,4);
            used=true;
        }
    } else if mob.kind == crate::mob::Kind::CopperGolem && let Some(name)=name {
        used=crate::mob::newmobs::copper_use(mob,name);
        consumed=used && name=="honeycomb";
        wear=used && name.ends_with("_axe");
    }
    if wear && player.game_mode != GAME_MODE_CREATIVE && let Some(stack)=held {
        let mut rng=crate::mob::Rng::new(mob.ticks ^ mob.entity_id as u64);
        let worn=combat::wear(&stack,1,false,||rng.unit() as f32);
        let slot=player.inventory.selected_slot();
        player.inventory.set(slot,combat::add_damage(stack,worn));reaction.slots.push(slot);
    }
    if consumed && player.game_mode != GAME_MODE_CREATIVE && let Some(stack)=held {
        let slot=player.inventory.selected_slot();
        player.inventory.set(slot,inventory::remainder(stack,stack.count-1));
        reaction.slots.push(slot);
    }
    used
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Чистая табличка: корень, сторона «back_text» первой, в ней свечение,
    /// цвет и четыре пустые строки — как у оригинала; тег закрыт.
    #[test]
    fn an_empty_sign_is_encoded_like_the_original() {
        let nbt = empty_sign_nbt();

        assert_eq!(&nbt[..4], &[0x0A, 0x0A, 0x00, 9]);
        assert_eq!(&nbt[4..13], b"back_text");
        assert_eq!(nbt.last(), Some(&0x00));
        // Корень; стороны — заголовок (12 и 13) и 60 байт тегов; is_waxed
        // (12); конец корня.
        assert_eq!(nbt.len(), 1 + (12 + 60) + (13 + 60) + 12 + 1);
    }

    /// Данные освещения по протоколу: массив у каждой секции из маски, тёмные
    /// секции — только в «пустой» маске, небо выше секции над верхним блоком
    /// не шлётся вовсе.
    #[test]
    fn light_data_follows_the_masks() {
        let mut light = ChunkLight::open_sky();

        // Чанк с землёй в секции 4: под ней темно, в ней свет разный.
        let mut mixed = [0u8; world::light::NIBBLES];
        mixed[..1024].fill(0xFF);

        for section in 0..4 {
            light.sky[section] = LightSection::Even(0);
        }

        light.sky[4] = LightSection::Mixed(Arc::new(mixed));
        light.block[2] = LightSection::Mixed(Arc::new(mixed));

        let mut out = Vec::new();
        push_light(&mut out, Some(4), &light);

        let mut at = 0;
        let varint = |out: &[u8], at: &mut usize| {
            let (value, size) = decode_varint(&out[*at..]).expect("число");
            *at += size;
            value
        };
        let mut masks = Vec::new();

        for _ in 0..4 {
            let longs = varint(&out, &mut at);
            let mut value = 0i64;

            for _ in 0..longs {
                value = i64::from_be_bytes(out[at..at + 8].try_into().expect("long"));
                at += 8;
            }

            masks.push(value);
        }

        let [sky, block, sky_empty, block_empty] = masks[..] else {
            panic!("масок четыре");
        };

        // Небо: секции 0–3 тёмные, 4 — массив, 5 — над верхним блоком, ещё
        // массивом; выше ничего. Нулевой бит — под миром, там темно.
        assert_eq!(sky, 0b11 << 5);
        assert_eq!(sky_empty, 0b11111);
        assert_eq!(sky & sky_empty, 0);

        // Блочный свет: массив только у секции 2, остальные — пустые, и так
        // все биты маски, включая секции под миром и над ним.
        let every = (1i64 << (world::SECTIONS + 2)) - 1;

        assert_eq!(block, 1 << 3);
        assert_eq!(block | block_empty, every);
        assert_eq!(block & block_empty, 0);

        let sky_arrays = varint(&out, &mut at);
        assert_eq!(sky_arrays, 2);

        for _ in 0..sky_arrays {
            assert_eq!(varint(&out, &mut at), 2048);
            at += 2048;
        }

        assert_eq!(varint(&out, &mut at), 1);
        assert_eq!(varint(&out, &mut at), 2048);
        assert_eq!(&out[at..at + 1024], &[0xFF; 1024][..]);
        assert_eq!(out.len(), at + 2048);
    }

    /// Биомы секции: один — одно число, до восьми — палитра до трёх битов,
    /// больше — номера прямо, по семь битов (в реестре 65 биомов).
    #[test]
    fn section_biomes_are_packed_by_the_protocol() {
        let one = [5u16; 64];
        let mut out = Vec::new();

        push_biomes(&mut out, Some(&one));
        assert_eq!(out, vec![0, 5]);

        let two: Vec<u16> = (0..64).map(|cell| if cell < 16 { 1 } else { 7 }).collect();
        let mut out = Vec::new();

        push_biomes(&mut out, Some(&two));
        // Один бит, палитра [1, 7], 64 записи — один long.
        assert_eq!(&out[..4], &[1, 2, 1, 7]);
        assert_eq!(out.len(), 4 + 8);
        assert_eq!(i64::from_be_bytes(out[4..].try_into().unwrap()), !0xFFFF);

        let many: Vec<u16> = (0..64).map(|cell| cell % 10).collect();
        let mut out = Vec::new();

        push_biomes(&mut out, Some(&many));
        // Прямо: в реестре 65 биомов — семь битов, девять записей на long,
        // восемь long'ов, палитры нет.
        assert_eq!(out[0], 7);
        assert_eq!(out.len(), 1 + 8 * 8);
        assert_eq!(i64::from_be_bytes(out[1..9].try_into().unwrap()) & 0x7F, 0);
        assert_eq!((i64::from_be_bytes(out[1..9].try_into().unwrap()) >> 7) & 0x7F, 1);
    }

    /// Точка появления в проверках: в них мира нет, поэтому берём любую.
    const SPAWN: (f64, f64, f64) = (0.5, 64.0, 0.5);

    /// Игрок, о котором на диске уже лежит ровно то, что он делает сейчас:
    /// так сервер видит игрока сразу после захода.
    fn settled_player() -> PlayerState {
        settled_player_with_yaw(SPAWN_YAW)
    }

    fn settled_player_with_yaw(yaw: f32) -> PlayerState {
        let saved = PlayerData {
            shoulders: [None,None],
            inventory: Inventory::new(),
            x: SPAWN.0,
            y: SPAWN.1,
            z: SPAWN.2,
            yaw,
            pitch: SPAWN_PITCH,
            game_mode: GAME_MODE_CREATIVE,
            dimension: Default::default(),
            vitals: Default::default(),
            respawn: None,
            ender: playerdata::empty_ender(),
        };

        PlayerState::new([7; 16], "Игрок".to_string(), Some(saved), None, 0x7F, 1, SPAWN)
    }

    /// Куда складывать файлы в тестах: своя директория, чтобы тесты не мешали
    /// друг другу.
    fn test_directory() -> std::path::PathBuf {
        let directory = std::env::temp_dir().join("mc_play_playerstate");
        std::fs::create_dir_all(&directory).expect("создать директорию для теста");

        directory
    }

    /// Новый игрок появляется в точке появления, и записать о нём есть что
    /// сразу: файла на диске ещё нет.
    #[test]
    fn a_new_player_starts_at_the_spawn_and_is_saved() {
        let player = PlayerState::new([1; 16], "Новичок".to_string(), None, None, 0x7F, 1, SPAWN);

        assert_eq!((player.x, player.y, player.z), SPAWN);
        assert_eq!(player.game_mode, GAME_MODE_CREATIVE);
        assert!(player.needs_save());
    }

    /// Кто уже заходил — встаёт там, где вышел, и смотрит туда же, куда
    /// смотрел: записывать это заново незачем.
    #[test]
    fn a_returning_player_resumes_where_he_left() {
        let saved = PlayerData {
            shoulders: [None,None],
            inventory: Inventory::new(),
            x: 100.5,
            y: 70.0,
            z: -20.5,
            yaw: 90.0,
            pitch: -10.0,
            game_mode: 0,
            dimension: Default::default(),
            vitals: Default::default(),
            respawn: None,
            ender: playerdata::empty_ender(),
        };

        let player = PlayerState::new([2; 16], "Игрок".to_string(), Some(saved), None, 0x7F, 1, SPAWN);

        assert_eq!((player.x, player.y, player.z), (100.5, 70.0, -20.5));
        assert_eq!((player.yaw, player.pitch), (90.0, -10.0));
        assert_eq!(player.game_mode, 0);
        assert!(!player.needs_save());
    }

    /// Здоровье, еда, воздух и огонь возвращаются такими, какими были при выходе,
    /// а клиенту не надо повторять то, что он и так получит при входе.
    #[test]
    fn a_returning_player_keeps_health_food_air_and_fire() {
        let saved = PlayerData {
            shoulders: [None,None],
            inventory: Inventory::new(),
            x: 0.5,
            y: 64.0,
            z: 0.5,
            yaw: 0.0,
            pitch: 0.0,
            game_mode: 0,
            dimension: Default::default(),
            vitals: crate::playerdata::SavedVitals { health: 6.0, food: 11, saturation: 2.0, exhaustion: 1.5, air: 200, fire: 80, fall: 0.0, experience: 21, xp_seed: 5 },
            respawn: None,
            ender: playerdata::empty_ender(),
        };

        let mut player = PlayerState::new([3; 16], "Игрок".to_string(), Some(saved), None, 0x7F, 1, SPAWN);

        assert_eq!((player.vit.health, player.vit.food, player.vit.saturation), (6.0, 11, 2.0));
        assert_eq!((player.vit.exhaustion, player.vit.air, player.vit.fire), (1.5, 200, 80));
        assert!(!player.needs_save());

        player.vit.health = 5.0;
        assert!(player.needs_save());
    }

    /// Шаг внутри одного блока записывать незачем: игрок вернётся в тот же
    /// блок. А вот остановку внутри блока записать стоит — иначе он окажется
    /// у границы, через которую шёл.
    #[test]
    fn a_step_inside_a_block_is_saved_only_when_the_player_stops() {
        let mut player = settled_player();

        player.set_position(SPAWN.0 + 0.3, SPAWN.1, SPAWN.2);
        assert!(!player.needs_save(), "шаг внутри блока — не повод писать файл");

        // Тот же самый пакет второй раз: игрок никуда не идёт.
        player.set_position(SPAWN.0 + 0.3, SPAWN.1, SPAWN.2);
        assert!(player.needs_save(), "остановку записываем");
    }

    /// Переход в другой блок записывается: игрок должен вернуться именно сюда.
    #[test]
    fn crossing_into_another_block_is_saved() {
        let mut player = settled_player();

        player.set_position(SPAWN.0 + 1.0, SPAWN.1, SPAWN.2);
        assert!(player.needs_save());

        player.set_position(SPAWN.0 + 1.0, SPAWN.1 - 1.0, SPAWN.2);
        assert!(player.needs_save());
    }

    /// Заметный поворот записывается, мелкий — нет.
    #[test]
    fn a_noticeable_turn_is_saved() {
        let mut player = settled_player();

        player.set_rotation(SPAWN_YAW + 3.0, SPAWN_PITCH);
        assert!(!player.needs_save(), "три градуса — не поворот");

        player.set_rotation(SPAWN_YAW + 20.0, SPAWN_PITCH);
        assert!(player.needs_save());
    }

    /// Поворот через ноль — не полный оборот: 355° и 5° отличаются на десять
    /// градусов, а не на триста пятьдесят.
    #[test]
    fn turning_past_zero_is_not_a_full_turn() {
        let mut player = settled_player_with_yaw(355.0);

        player.set_rotation(5.0, SPAWN_PITCH);
        assert!(!player.needs_save());
    }

    /// Смена режима игры — тоже повод записать: игрок должен вернуться в том
    /// режиме, в каком вышел.
    #[test]
    fn a_game_mode_change_is_saved() {
        let mut player = settled_player();

        player.game_mode = 0;
        assert!(player.needs_save());
    }

    /// Записанное положение читается обратно тем же самым — это и есть то,
    /// ради чего файл существует.
    #[test]
    fn what_is_written_can_be_read_back() {
        let directory = test_directory();

        let mut player = settled_player();

        player.set_position(123.5, 70.0, -45.25);
        player.set_rotation(90.0, 0.0);
        player.save_if_needed(&directory);

        let read = playerdata::load(&directory, &player.uuid).expect("файл игрока");

        assert_eq!(
            (read.x, read.y, read.z, read.yaw, read.game_mode),
            (123.5, 70.0, -45.25, 90.0, GAME_MODE_CREATIVE)
        );

        // Записанное совпадает с нынешним положением — второй раз писать нечего.
        assert!(!player.needs_save());
    }

    /// Чанк считается по тому, где игрок стоит: клиенту нужны чанки вокруг
    /// него, а не вокруг начала координат.
    #[test]
    fn the_chunk_is_taken_from_the_player_position() {
        let mut player = settled_player();

        player.set_position(0.5, SPAWN.1, 0.5);
        assert_eq!(player.chunk(), (0, 0));

        player.set_position(100.5, SPAWN.1, -1.5);
        assert_eq!(player.chunk(), (6, -1));
    }

    /// Координаты блока обязаны читаться ровно так, как записываются: если
    /// раскладка битов разойдётся, блоки будут ставиться и ломаться не там,
    /// где игрок щёлкнул.
    #[test]
    fn position_round_trip() {
        let positions = [
            (0, 64, 0),
            (63, 64, -1),
            (-2, 65, 5),
            (4161, 64, 1),
            (-33554432, -64, 33554431),
            (33554431, 2047, -33554432),
        ];

        for (x, y, z) in positions {
            let mut body = Vec::new();
            push_packed_position(&mut body, x, y, z);

            let mut cursor = Cursor::new(&body);

            assert_eq!(
                cursor.position(),
                Some((x, y, z)),
                "позиция {} {} {} прочиталась неправильно",
                x,
                y,
                z
            );
        }
    }

    /// Координаты игрока читаются из начала полезной нагрузки, а не из тела
    /// пакета: в теле первым идёт идентификатор, и без его пропуска числа
    /// получаются мусорными.
    #[test]
    fn player_position_comes_from_payload() {
        let mut body = encode_varint(SET_PLAYER_POSITION);
        push_f64(&mut body, 12.5);
        push_f64(&mut body, 65.0);
        push_f64(&mut body, -7.25);

        let packet = PacketInfo {
            id: SET_PLAYER_POSITION,
            length: body.len() as i32,
            body,
        };

        assert_eq!(packet.payload().len(), POSITION_BYTES);
        assert_eq!(read_position(packet.payload()), Some((12.5, 65.0, -7.25)));
    }

    /// Признак «на земле» приходит флагом в каждом пакете о перемещении, а
    /// не только в отдельном пакете: клиент шлёт его, лишь когда стоит на
    /// месте. До подтверждения переноса пакеты клиента — со старого места
    /// и не в счёт.
    #[test]
    fn on_ground_comes_with_every_move_packet() {
        let shared = crate::shared::Shared::new(
            crate::world::World::in_memory(),
            crate::world::World::in_memory(),
            std::path::PathBuf::new(),
            std::path::PathBuf::new(),
            Default::default(),
            crate::ops::Ops::open(std::path::Path::new("/tmp/mcsheriffanya-ground-ops.json")),
            crate::skins::Settings::default(),
        );
        let mut player = PlayerState::new([5; 16], "Игрок".to_string(), None, None, 0x7F, 1, SPAWN);
        let packet = |id: i32, fields: &[f64], angles: &[f32], flags: u8| {
            let mut body = encode_varint(id);
            for value in fields {
                push_f64(&mut body, *value);
            }
            for value in angles {
                push_f32(&mut body, *value);
            }
            body.push(flags);
            PacketInfo { id, length: body.len() as i32, body }
        };

        handle_client_packet(&packet(SET_PLAYER_POSITION, &[1.5, 2.0, 3.5], &[], 0x01), &mut player, &shared);
        assert!(player.on_ground);

        handle_client_packet(&packet(SET_PLAYER_POSITION_AND_ROTATION, &[1.5, 2.5, 3.5], &[90.0, 10.0], 0x00), &mut player, &shared);
        assert!(!player.on_ground);
        assert_eq!(player.y, 2.5);

        // Флаг «упёрся в стену» (0x02) — не земля.
        handle_client_packet(&packet(SET_PLAYER_ROTATION, &[], &[0.0, 0.0], 0x02), &mut player, &shared);
        assert!(!player.on_ground);

        handle_client_packet(&packet(SET_PLAYER_ON_GROUND, &[], &[], 0x03), &mut player, &shared);
        assert!(player.on_ground);

        // Ждём подтверждения переноса — старое место и признак не в счёт.
        player.awaiting_teleport = true;
        handle_client_packet(&packet(SET_PLAYER_POSITION, &[9.0, 9.0, 9.0], &[], 0x00), &mut player, &shared);
        handle_client_packet(&packet(SET_PLAYER_ON_GROUND, &[], &[], 0x00), &mut player, &shared);
        assert!(player.on_ground);
        assert_eq!(player.x, 1.5);

    }

    /// Дотягивание меряется до ящика блока: так у оригинала (чёрный ящик,
    /// reach-edge и target-conduct — робот в углу площадки).
    #[test]
    fn reach_is_measured_to_the_block_box() {
        let mut player = PlayerState::new([6; 16], "Игрок".to_string(), None, None, 0x7F, 1, SPAWN);
        player.game_mode = GAME_MODE_CREATIVE;
        let lever = (100, -59, 100);

        (player.x, player.y, player.z) = (106.93, -59.0, 100.5);
        assert!(within_reach(&player, lever));

        (player.x, player.y, player.z) = (107.1, -59.0, 100.5);
        assert!(!within_reach(&player, lever));

        // До середины 6,5 — на волос дальше прежней мерки, до ящика 5,73.
        (player.x, player.y, player.z) = (104.5, -59.0, 104.5);
        assert!(within_reach(&player, (100, -59, 99)));

        // В выживании рука короче.
        player.game_mode = GAME_MODE_SURVIVAL;
        (player.x, player.y, player.z) = (106.6, -59.0, 100.5);
        assert!(!within_reach(&player, lever));
        (player.x, player.y, player.z) = (106.3, -59.0, 100.5);
        assert!(within_reach(&player, lever));

        // Вне мира по высоте — никогда.
        assert!(!within_reach(&player, (105, -65, 100)));
    }

    /// Место, где стоит сущность, — для проверок пакетов о перемещении.
    fn at(x: f64, y: f64, z: f64, yaw: f32, on_ground: bool) -> Position {
        Position { x, y, z, yaw, pitch: 0.0, on_ground }
    }

    /// В пакете о перемещении поля стоят по виду пакета, а не по тому, что
    /// изменилось.
    ///
    /// Особенно важен последний случай: игрок оторвался от земли, не сдвинувшись
    /// и не повернувшись. Раньше сервер слал пакет поворота без самих углов,
    /// и клиент на нём обрывал соединение — вдвоём в игру было не зайти.
    #[test]
    fn a_move_packet_has_all_the_fields_of_its_kind() {
        let here = at(0.0, 64.0, 0.0, 0.0, true);

        // Номер сущности, три смещения, два угла, признак земли.
        let both = move_body(1, here, at(1.0, 64.0, 0.0, 90.0, true)).expect("шаг");
        assert_eq!(both[0], ENTITY_POSITION_AND_ROTATION as u8);
        assert_eq!(both.len(), 1 + 1 + 2 * 3 + 2 + 1);

        // Только сдвинулся: углов в этом пакете нет вовсе.
        let moved = move_body(1, here, at(1.0, 64.0, 0.0, 0.0, true)).expect("шаг");
        assert_eq!(moved[0], ENTITY_POSITION as u8);
        assert_eq!(moved.len(), 1 + 1 + 2 * 3 + 1);

        // Только повернулся: смещений нет, углы есть.
        let turned = move_body(1, here, at(0.0, 64.0, 0.0, 90.0, true)).expect("поворот");
        assert_eq!(turned[0], ENTITY_ROTATION as u8);
        assert_eq!(turned.len(), 1 + 1 + 2 + 1);

        // Только оторвался от земли: пакет всё равно с углами.
        let jumped = move_body(1, here, at(0.0, 64.0, 0.0, 0.0, false)).expect("прыжок");
        assert_eq!(jumped[0], ENTITY_ROTATION as u8);
        assert_eq!(jumped.len(), turned.len());
        assert_eq!(jumped[jumped.len() - 1], 0);
    }

    /// Слишком большой шаг смещением не рассказать: такой отдаётся переносом.
    #[test]
    fn a_long_step_is_not_a_move_packet() {
        let here = at(0.0, 64.0, 0.0, 0.0, true);

        assert!(move_body(1, here, at(7.9, 64.0, 0.0, 0.0, true)).is_some());
        assert!(move_body(1, here, at(100.0, 64.0, 0.0, 0.0, true)).is_none());
    }

    /// «Слот» протокола записывается так же, как читается: пустой слот — один
    /// нулевой байт, полный — количество, предмет и два нуля довесков.
    #[test]
    fn a_slot_is_written_the_way_it_is_read() {
        let mut empty = Vec::new();
        push_slot(&mut empty, None);
        assert_eq!(empty, vec![0x00]);

        let mut full = Vec::new();
        push_slot(&mut full, Some(Stack::new(5, 12)));
        assert_eq!(full, vec![12, 5, 0, 0]);

        let (read, readable) = read_slot(&mut Cursor::new(&full)).expect("прочитать");
        assert_eq!(read, Some(Stack::new(5, 12)));
        assert!(readable);

        let (read, _) = read_slot(&mut Cursor::new(&empty)).expect("прочитать пустой");
        assert_eq!(read, None);
    }

    /// Щелчок по инвентарю читается по полям и применяется к инвентарю
    /// Запись игрока для остальных несёт предмет из руки: без него чужие
    /// руки на экране пустые.
    #[test]
    fn the_shared_record_carries_the_held_item() {
        let mut player = PlayerState::new([9; 16], "Игрок".to_string(), None, None, 0x7F, 1, SPAWN);

        assert_eq!(player.member().held, None);

        player
            .inventory
            .set(inventory::FIRST_HOTBAR, Some(Stack::new(42, 3)));

        assert_eq!(player.member().held, Some(Stack::new(42, 3)));
    }

    /// сервера, а щелчок в чужое окно — нет: таких окон сервер не открывает.
    #[test]
    fn a_click_is_read_field_by_field() {
        let mut player = PlayerState::new([3; 16], "Игрок".to_string(), None, None, 0x7F, 1, SPAWN);

        player.inventory.set(inventory::FIRST_HOTBAR, Some(Stack::new(1, 5)));

        let mut click = encode_varint(PLAYER_WINDOW);
        click.extend_from_slice(&encode_varint(1)); // номер состояния окна
        click.extend_from_slice(&(inventory::FIRST_HOTBAR as i16).to_be_bytes());
        click.push(0); // левая кнопка
        click.extend_from_slice(&encode_varint(0)); // обычный щелчок
        push_slot(&mut click, Some(Stack::new(1, 5)));

        assert!(read_click(&click, &mut player).understood);
        assert_eq!(player.inventory.cursor(), Some(Stack::new(1, 5)));
        assert_eq!(player.inventory.slot(inventory::FIRST_HOTBAR), None);

        // Чужое окно: сервер таких не открывает и щелчок не применяет.
        let mut other = encode_varint(PLAYER_WINDOW + 1);
        other.extend_from_slice(&click[1..]);

        assert!(!read_click(&other, &mut player).understood);
    }

    /// Выдать себе предмет из творческого меню можно только в творческом
    /// режиме: в выживании сервер такое сообщение не применяет.
    #[test]
    fn items_are_handed_out_only_in_creative() {
        use crate::commands::GAME_MODE_SURVIVAL;

        let mut player = PlayerState::new([4; 16], "Игрок".to_string(), None, None, 0x7F, 1, SPAWN);

        let mut message = Vec::new();
        message.extend_from_slice(&(inventory::FIRST_HOTBAR as i16).to_be_bytes());
        push_slot(&mut message, Some(Stack::new(1, 64)));

        player.game_mode = GAME_MODE_SURVIVAL;
        read_creative_slot(&message, &mut player);
        assert_eq!(player.inventory.slot(inventory::FIRST_HOTBAR), None);

        player.game_mode = GAME_MODE_CREATIVE;
        read_creative_slot(&message, &mut player);
        assert_eq!(
            player.inventory.slot(inventory::FIRST_HOTBAR),
            Some(Stack::new(1, 64))
        );
    }

    /// Особый динамит уходит клиенту с тремя довесками (название, описание,
    /// пометка), а из творческого меню пометка приходит обратно — вид не
    /// теряется, когда игрок в творческом перекладывает стопку.
    #[test]
    fn custom_tnt_goes_out_with_components_and_comes_back() {
        let kind = crate::custom_tnt::by_name("nuke").expect("nuke");
        let stack = crate::custom_tnt::stack(kind, 2).expect("динамит");

        let mut sent = Vec::new();
        push_slot(&mut sent, Some(stack));
        assert_eq!(&sent[..2], &[2, stack.item as u8, 0][..2]);
        assert!(sent.windows(kind.title.len()).any(|window| window == kind.title.as_bytes()));

        // Творческий слот: у каждого довеска своя длина (Untrusted Slot).
        let data = nbt::Compound::new().string(crate::custom_tnt::MARKER, kind.name).encode_network();
        let name = nbt::text_component(kind.title);
        let mut message = Vec::new();
        message.extend_from_slice(&(inventory::FIRST_HOTBAR as i16).to_be_bytes());

        for value in [2, stack.item, 2, 0, COMPONENT_CUSTOM_NAME, name.len() as i32] {
            message.extend_from_slice(&encode_varint(value));
        }

        message.extend_from_slice(&name);
        message.extend_from_slice(&encode_varint(COMPONENT_CUSTOM_DATA));
        message.extend_from_slice(&encode_varint(data.len() as i32));
        message.extend_from_slice(&data);

        let mut player = PlayerState::new([4; 16], "Игрок".to_string(), None, None, 0x7F, 1, SPAWN);
        player.game_mode = GAME_MODE_CREATIVE;
        read_creative_slot(&message, &mut player);

        assert_eq!(player.inventory.slot(inventory::FIRST_HOTBAR), Some(stack));
    }

    /// Брошенный предмет летит туда, куда смотрит игрок, — с наклоном:
    /// в небо вверх, под ноги вниз.
    #[test]
    fn a_throw_follows_the_look_direction() {
        // Взгляд по сторонам света: юг при повороте 0, север при 180.
        let (x, y, z) = looking(0.0, 0.0);
        assert!(z > 0.9 && x.abs() < 0.01 && y.abs() < 0.01, "юг: {} {} {}", x, y, z);

        let (_, _, z) = looking(180.0, 0.0);
        assert!(z < -0.9, "север");

        let (x, _, _) = looking(90.0, 0.0);
        assert!(x < -0.9, "запад");

        // Взгляд в небо и под ноги.
        let (_, up, _) = looking(0.0, -90.0);
        assert!(up > 0.99, "в небо: {}", up);

        let (_, down, _) = looking(0.0, 90.0);
        assert!(down < -0.99, "под ноги: {}", down);

        // Наклон в сорок пять градусов: и вперёд, и вверх поровну.
        let (_, up, z) = looking(0.0, -45.0);
        assert!((up - z).abs() < 0.01, "наискосок: вверх {} вперёд {}", up, z);
    }

    /// Подсказки к команде: сервер дописывает имена игроков и говорит,
    /// какой кусок строки заменить.
    #[test]
    fn suggestions_complete_player_names() {
        let shared = crate::shared::Shared::new(
            crate::world::World::in_memory(),
            crate::world::World::in_memory(),
            std::path::PathBuf::new(),
            std::path::PathBuf::new(),
            Default::default(),
            crate::ops::Ops::open(std::path::Path::new("/tmp/mcsheriffanya-no-ops.json")),
            crate::skins::Settings::default(),
        );

        {
            let mut players = shared.players.lock().expect("список игроков");

            for (number, name) in [(1u8, "Вася"), (2, "Витя"), (3, "Петя")] {
                let mut member = PlayerState::new(
                    [number; 16],
                    name.to_string(),
                    None,
                    None,
                    0x7F,
                    number as u64,
                    SPAWN,
                )
                .member();

                member.uuid = [number; 16];
                players.join(member, number as i32).expect("ник свободен");
            }
        }

        let ask = |text: &str| {
            let mut payload = encode_varint(7); // номер запроса
            payload.extend_from_slice(&encode_string(text));

            suggestions(&payload, &shared, &[1; 16]).expect("подсказки собрались")
        };

        // Набрано начало имени: подходят двое из трёх.
        let hints = ask("kick В");

        assert_eq!(hints.id, 7);
        assert_eq!(hints.matches, vec!["Вася".to_string(), "Витя".to_string()]);
        // Значки выбора подсказываются тоже.
        assert!(ask("kick @").matches.contains(&"@a".to_string()));
        assert_eq!(hints.length, 1, "заменять надо одну набранную букву");
        assert_eq!(hints.start, "kick ".len() as i32);

        // Ничего не набрано — подходят все имена и все значки.
        assert_eq!(ask("tp ").matches.len(), 3 + 5);

        // Набрано то, чего нет.
        assert!(ask("kick Юра").matches.is_empty());

        // Особый динамит без прав не подсказывается вовсе.
        assert!(ask("tnt ").matches.is_empty());
        assert!(ask("tnt give Вася m").matches.is_empty());
    }

    /// Подсказки к `tnt`: слова, игроки, виды из таблицы, число — тому, у
    /// кого есть права.
    #[test]
    fn tnt_hints_come_from_the_kinds_table() {
        let ops = std::env::temp_dir().join(format!("mcsheriffanya-tnt-hints-{}.json", std::process::id()));
        let _ = std::fs::remove_file(&ops);
        let shared = crate::shared::Shared::new(
            crate::world::World::in_memory(),
            crate::world::World::in_memory(),
            std::path::PathBuf::new(),
            std::path::PathBuf::new(),
            Default::default(),
            crate::ops::Ops::open(&ops),
            crate::skins::Settings::default(),
        );

        shared.ops.lock().expect("права").give(&[1; 16], "Вася", crate::ops::HIGHEST_LEVEL);

        let hints = |text: &str| {
            let typed = text.rsplit(' ').next().unwrap_or("");
            hints_for(text, typed, &shared, &[1; 16])
        };

        assert_eq!(hints("/tnt "), vec!["give".to_string(), "list".to_string()]);
        assert_eq!(hints("/tnt give Вася "), crate::custom_tnt::names().map(str::to_string).collect::<Vec<_>>());
        assert_eq!(hints("/tnt give Вася n"), vec!["nuke".to_string()]);
        assert!(hints("/tnt give Вася mega ").contains(&"64".to_string()));
        assert!(hints("/tnt give ").contains(&"@a".to_string()));
        assert!(hints_for("/tnt ", "", &shared, &[2; 16]).is_empty(), "без прав подсказалось");

        let _ = std::fs::remove_file(&ops);
    }

    /// Подсказки зависят от команды: где ждут игрока — имена, где предмет —
    /// названия предметов.
    #[test]
    fn hints_depend_on_the_command() {
        let shared = crate::shared::Shared::new(
            crate::world::World::in_memory(),
            crate::world::World::in_memory(),
            std::path::PathBuf::new(),
            std::path::PathBuf::new(),
            Default::default(),
            crate::ops::Ops::open(std::path::Path::new("/tmp/mcsheriffanya-hints-ops.json")),
            crate::skins::Settings::default(),
        );

        // Предметы подсказываются по началу названия.
        let items = hints_for("/give Игрок sto", "sto", &shared, &[1; 16]);
        assert!(items.iter().any(|name| name == "stone"), "камня нет в подсказках");
        assert!(items.iter().all(|name| name.starts_with("sto")));

        // Без начала слова список предметов не вываливается.
        assert!(hints_for("/give Игрок ", "", &shared, &[1; 16]).is_empty());

        // У времени свои слова.
        assert_eq!(hints_for("/summon chest", "chest", &shared, &[1; 16]), vec!["chest_minecart".to_string()]);
        assert!(hints_for("/kill @e[type=i", "@e[type=i", &shared, &[1; 16]).contains(&"@e[type=item]".to_string()));
        assert!(hints_for("/setblock 1 2 3 redstone_l", "redstone_l", &shared, &[1; 16]).contains(&"redstone_lamp".to_string()));
        assert!(hints_for("/fill 1 2 3 4 5 6 redstone_l", "redstone_l", &shared, &[1; 16]).contains(&"redstone_lamp".to_string()));

        let time = hints_for("/time s", "s", &shared, &[1; 16]);
        assert_eq!(time, vec!["set".to_string()]);
    }

}

#[cfg(test)]
mod tails_tests;
