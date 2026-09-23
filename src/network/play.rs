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

use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tokio::net::TcpStream;
use tokio::time::timeout;

use super::nbt;
use super::packet::{PacketInfo, decode_varint, is_client_disconnect, read_packet, write_body};
use super::types;
use super::configuration;
use super::varint::{encode_string, encode_varint, encode_varlong};
use crate::blocks;
use crate::fluids;
use crate::inventory::{self, Inventory, Stack};
use crate::journal;
use crate::items::{self, Dropped};
use crate::commands::{self, Effect, GAME_MODE_CREATIVE, Source};
use crate::playerdata::{self, PlayerData};
use crate::players::{Change, Member, Order, Position};
use crate::placing;
use crate::redstone;
use crate::shared::Shared;
use crate::skins::{self, Skin};
use crate::world::{self, World, WorldEvent, WorldSound};
use crate::{log_debug, log_error, log_info};

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

/// Serverbound Set Player Position and Rotation — игрок сдвинулся и повернулся.
const SET_PLAYER_POSITION_AND_ROTATION: i32 = 0x1F;

/// Serverbound Set Player Rotation — игрок только повернулся.
const SET_PLAYER_ROTATION: i32 = 0x20;

/// Serverbound Set Player On Ground — игрок встал на землю или оторвался от неё.
const SET_PLAYER_ON_GROUND: i32 = 0x21;

/// Serverbound Player Action — игрок копает блок или отменяет копание.
const PLAYER_ACTION: i32 = 0x29;

/// Serverbound Use Item On — игрок щёлкнул по блоку предметом в руке.
const USE_ITEM_ON: i32 = 0x42;
const USE_ITEM: i32 = 0x43;

/// Serverbound Set Carried Item — игрок выбрал другой слот панели быстрого
/// доступа.
const SET_CARRIED_ITEM: i32 = 0x35;

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

/// Номер свойства «что это за предмет» у лежащего предмета.
///
/// У него восемь общих свойств сущности, а это — девятое.
const ITEM_PROPERTY: u8 = 8;

/// Вид значения «предмет» в списке видов свойств сущности.
const PROPERTY_SLOT: i32 = 7;

/// Serverbound Close Container — игрок закрыл окно инвентаря.
const CLOSE_CONTAINER: i32 = 0x13;

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

/// Clientbound Game Event.
const GAME_EVENT: i32 = 0x26;
const SET_HEALTH: i32 = 0x68;

/// Clientbound Set Center Chunk.
const SET_CENTER_CHUNK: i32 = 0x5E;

/// Clientbound Sound Effect — звук в точке мира.
const SOUND_EFFECT: i32 = 0x75;

/// Разряд звука: звук блока. Их одиннадцать, и от разряда зависит,
/// какой ползунок громкости на него влияет.
const SOUND_BLOCK: i32 = 4;

/// Clientbound Block Action — то, что клиент проигрывает у себя сам:
/// ход поршня, открытие сундука, звук нотного блока.
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

/// Идентификатор телепортации. Клиент возвращает его в подтверждении.
const TELEPORT_ID: i32 = 1;

/// Имя измерения. Должно совпадать с именем в списке измерений и с записью
/// реестра dimension_type.
const DIMENSION: &str = "minecraft:overworld";

/// ID измерения в реестре dimension_type: у нас там одна запись.
const DIMENSION_TYPE_ID: i32 = 0;

/// Дальность прорисовки, которую сервер объявляет клиенту. Клиент ждёт, пока
/// загрузятся все чанки в этом радиусе, поэтому она должна совпадать с числом
/// реально отправленных чанков.
/// Как часто напоминать клиенту о себе.
///
/// Клиент отключается, если не получал от сервера ни одного пакета дольше
/// 30 секунд, поэтому интервал должен быть заметно меньше.
const KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(5);

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

/// Где сервер в последний раз показывал этому клиенту чужого игрока.
///
/// Смещения при движении считаются от того, что клиент уже знает, поэтому
/// помнить приходится про каждого чужого игрока отдельно, и у каждого
/// подключения этот список свой.
/// Что этому клиенту уже показано про чужого игрока.
///
/// Смещения при движении считаются от того, что клиент уже знает, поэтому
/// помнить приходится для каждого подключения своё.
#[derive(Clone, Copy, PartialEq)]
struct Shown {
    position: Position,
    skin_parts: u8,
    main_hand: u8,
    held: Option<Stack>,
}

type Seen = HashMap<i32, Shown>;

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
    stream: &mut TcpStream,
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
    let skin = skins::look_up(&shared.skins, name, shared.skin_settings).await;

    let spawn = shared
        .world
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

    if player.saved.is_some() {
        log_debug!(
            "Play: игрок {} заходит туда, где вышел: {:.2} {:.2} {:.2}",
            player.name,
            player.x,
            player.y,
            player.z
        );
    }

    let Some(entered) = enter_world(stream, &mut player, shared, (came_from, came_to)).await? else {
        log_debug!("Play: игрок {} в мир не попал", player.name);

        forget_reader(shared, player.reader);
        return Ok(false);
    };

    let result = read_and_log_play_packets(stream, shared, &mut player, entered).await;

    // Как у обычного сервера: «потерял соединение» с причиной, а «вышел»
    // добавит общий список игроков следом.
    match &result {
        Ok(()) => log_info!("{} потерял соединение: отключился", player.name),
        Err(error) => log_info!("{} потерял соединение: {}", player.name, error.kind()),
    }

    // При выходе записываем положение в любом случае — в том числе когда
    // соединение оборвалось ошибкой. Иначе последние метры пути пропали бы.
    player.save(&shared.playerdata);
    log_debug!(
        "Play: положение игрока {} записано при выходе: {:.2} {:.2} {:.2}",
        player.name,
        player.x,
        player.y,
        player.z
    );

    log_debug!("Play: чтение пакетов клиента завершено");

    forget_reader(shared, player.reader);

    result.map(|()| true)
}

/// Отписывает подключение от общих журналов.
///
/// Пока читатель записан, журналы держат для него все изменения. Забыть
/// отписаться — значит заставить их расти вечно.
fn forget_reader(shared: &Shared, reader: journal::Reader) {
    shared
        .world
        .lock()
        .expect("мир захвачен другим потоком")
        .forget_reader(reader);

    shared
        .players
        .lock()
        .expect("список игроков захвачен другим потоком")
        .forget_reader(reader);

    shared
        .items
        .lock()
        .expect("предметы захвачены другим потоком")
        .forget_reader(reader);
}

/// Убирает игрока из списка и сообщает об уходе в чат.
///
/// Зовётся при любом окончании сессии — и когда игрок вышел сам, и когда
/// соединение оборвалось ошибкой. Иначе он остался бы в списке навсегда.
pub fn player_left(shared: &Shared, name: &str, uuid: &[u8; 16]) {
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
    }
}

/// Проводит игрока в мир, отправляет ему чанки вокруг точки появления и
/// заводит запись в списке игроков.
///
/// Возвращает None, если игрока не пустили: ник успели занять, пока он
/// проходил настройку. Отказ здесь — окончательный, потому что он выносится
/// тем же захватом списка, каким игрок в него добавляется.
async fn enter_world(
    stream: &mut TcpStream,
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
        let mut world = shared.world.lock().expect("мир захвачен другим потоком");
        let mut players = shared
            .players
            .lock()
            .expect("список игроков захвачен другим потоком");
        let mut items = shared
            .items
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

    send_login(
        stream,
        player.game_mode,
        player.entity_id,
        shared.properties.view_distance,
        shared.properties.simulation_distance,
    )
    .await?;
    send_player_position(stream, player).await?;

    // Клиент подтверждает телепортацию — до подтверждения он игнорирует
    // пакеты о перемещении.
    if let Some(look) = read_confirm_teleportation(stream).await? {
        player.skin_parts = look.skin_parts;
        player.main_hand = look.main_hand;

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
    send_commands(stream).await?;

    // Список игроков: сперва те, кто уже на сервере, затем сам игрок.
    // Новичок о них узнаёт только отсюда — записи журнала он ещё не читал.
    for other in &others {
        send_player_info(stream, other).await?;
    }

    send_player_info(stream, &member).await?;

    // И про самого себя: свой второй слой скина клиент рисует не по своим
    // настройкам, а по тем же сведениям о сущности, что и чужой. Не сказать
    // ему — и в виде со стороны игрок будет без шапки и куртки.
    send_look(
        stream,
        member.entity_id,
        member.skin_parts,
        member.main_hand,
    )
    .await?;

    send_default_spawn_position(stream, player.spawn_point).await?;
    send_health(stream).await?;
    send_game_event_start_waiting(stream).await?;
    let (age, time) = world_age(shared);
    send_time(stream, age, time).await?;
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
        shared,
        (center_x, center_z),
        &mut sent,
        NEAR_CHUNKS,
        usize::MAX,
    )
    .await?;

    log_debug!("Play: отправлено ближних чанков — {}", sent.len());

    // Чужие игроки появляются в мире только теперь: сущность клиент создаёт
    // лишь после того, как получил запись об игроке в списке, а место в мире —
    // после чанков под ним.
    let mut seen = Seen::new();

    for other in &others {
        send_add_entity(stream, other).await?;
        send_look(stream, other.entity_id, other.skin_parts, other.main_hand).await?;
        send_equipment(stream, other.entity_id, other.held).await?;

        seen.insert(
            other.entity_id,
            Shown {
                position: other.position,
                skin_parts: other.skin_parts,
                main_hand: other.main_hand,
                held: other.held,
            },
        );
    }

    // Предметы, уже лежащие в мире: о них новичок из журнала не узнает —
    // они попали туда до его прихода.
    let lying: Vec<Dropped> = shared
        .items
        .lock()
        .expect("предметы захвачены другим потоком")
        .items()
        .to_vec();

    let mut seen_items = SeenItems::new();

    for item in &lying {
        send_dropped_item(stream, item).await?;
        seen_items.insert(item.entity_id, shown_item(item));
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

/// Отправляет Login (Play) — первый пакет фазы Play.
///
/// `entity_id` — номер сущности самого игрока: по нему его клиент отличает
/// себя от других. Тот же номер сервер сообщает остальным, когда показывает
/// им этого игрока.
async fn send_login(
    stream: &mut TcpStream,
    game_mode: i32,
    entity_id: i32,
    view: i32,
    simulation: i32,
) -> io::Result<()> {
    let mut body = encode_varint(LOGIN);

    push_i32(&mut body, entity_id); // Entity ID игрока
    body.push(0x00); // не хардкор

    // Список измерений сервера.
    body.extend_from_slice(&encode_varint(1));
    body.extend_from_slice(&encode_string(DIMENSION));

    body.extend_from_slice(&encode_varint(20)); // максимальное число игроков
    body.extend_from_slice(&encode_varint(view)); // дальность прорисовки
    body.extend_from_slice(&encode_varint(simulation)); // дальность симуляции
    body.push(0x00); // сокращённая отладочная информация
    body.push(0x01); // показывать экран возрождения
    body.push(0x00); // ограниченная крафтовка

    body.extend_from_slice(&encode_varint(DIMENSION_TYPE_ID));
    body.extend_from_slice(&encode_string(DIMENSION));

    push_i64(&mut body, 0); // hashed seed
    body.push(game_mode as u8); // режим игры
    body.push(0xFF); // предыдущий режим игры: нет (-1)
    body.push(0x00); // отладочный мир
    body.push(0x00); // плоский мир
    body.push(0x00); // точки смерти нет
    body.extend_from_slice(&encode_varint(0)); // откат портала
    body.extend_from_slice(&encode_varint(63)); // уровень моря
    body.push(0x00); // обязательный защищённый чат

    write_body(stream, body).await
}

/// Отправляет Synchronize Player Position — ставит игрока в точку, где он
/// стоял в прошлый раз (или в точку появления, если он здесь впервые).
async fn send_player_position(stream: &mut TcpStream, player: &PlayerState) -> io::Result<()> {
    let mut body = encode_varint(PLAYER_POSITION);

    body.extend_from_slice(&encode_varint(TELEPORT_ID));
    push_f64(&mut body, player.x); // X
    push_f64(&mut body, player.y); // Y
    push_f64(&mut body, player.z); // Z
    push_f64(&mut body, 0.0); // скорость X
    push_f64(&mut body, 0.0); // скорость Y
    push_f64(&mut body, 0.0); // скорость Z
    push_f32(&mut body, player.yaw); // поворот
    push_f32(&mut body, player.pitch); // наклон
    push_i32(&mut body, 0); // все координаты абсолютные

    write_body(stream, body).await
}

/// Читает подтверждение телепортации, пропуская прочие пакеты клиента —
/// кроме настроек: их клиент шлёт сразу после входа, и выбросить их значило
/// бы потерять, например, его ведущую руку. Возвращает последние из них.
async fn read_confirm_teleportation(
    stream: &mut TcpStream,
) -> io::Result<Option<configuration::ClientLook>> {
    let mut look = None;

    loop {
        let packet = read_packet(stream).await?;

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
async fn send_player_info(stream: &mut TcpStream, member: &Member) -> io::Result<()> {
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
        // Подпись Mojang мы не прикладываем намеренно. Она выдана на
        // опознаватель владельца скина, а у наших игроков опознаватели
        // свои — клиент видит несовпадение, пишет «invalid signature for
        // textures property» и показывает стандартный скин. Без подписи
        // он просто берёт описание как есть.
        Some(skin) => {
            body.extend_from_slice(&encode_varint(1)); // одно свойство
            body.extend_from_slice(&encode_string("textures"));
            body.extend_from_slice(&encode_string(&skin.value));
            body.push(0); // подписи нет
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
async fn send_game_mode(stream: &mut TcpStream, uuid: &[u8; 16], game_mode: i32) -> io::Result<()> {
    let mut body = encode_varint(PLAYER_INFO_UPDATE);

    body.push(PLAYER_INFO_GAME_MODE);
    body.extend_from_slice(&encode_varint(1)); // один игрок
    body.extend_from_slice(uuid);
    body.extend_from_slice(&encode_varint(game_mode));

    write_body(stream, body).await
}

/// Отправляет Player Info Remove — игрок пропал из списка.
async fn send_player_info_remove(stream: &mut TcpStream, uuid: &[u8; 16]) -> io::Result<()> {
    let mut body = encode_varint(PLAYER_INFO_REMOVE);

    body.extend_from_slice(&encode_varint(1)); // один игрок
    body.extend_from_slice(uuid);

    write_body(stream, body).await
}

/// Отправляет Add Entity — сущность чужого игрока появляется в мире.
///
/// Обязательно после Player Info Update на этого же игрока: без записи
/// в списке клиент сущность не создаст, и игрок останется невидимым.
async fn send_add_entity(stream: &mut TcpStream, member: &Member) -> io::Result<()> {
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

    drop_into_world(shared, stack, from, velocity, items::THROWN_DELAY);

    log_debug!(
        "Play: {} выбросил предмет {} ({} шт.)",
        player.name,
        stack.item,
        stack.count
    );
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
    stack: Stack,
    at: (f64, f64, f64),
    velocity: (f64, f64, f64),
    delay: std::time::Duration,
) {
    shared
        .items
        .lock()
        .expect("предметы захвачены другим потоком")
        .drop_item(shared.next_entity_id(), stack, at, velocity, delay);
}

/// Подбирает то, до чего игрок дотянулся.
///
/// Возвращает слоты, которые изменились: их надо показать клиенту. Само
/// исчезновение предмета из мира рассылается всем по журналу.
fn pick_up_items(shared: &Shared, player: &mut PlayerState) -> Vec<i32> {
    let taken = shared
        .items
        .lock()
        .expect("предметы захвачены другим потоком")
        .take_near((player.x, player.y, player.z), player.entity_id);

    let mut slots = Vec::new();

    for item in taken {
        log_debug!(
            "Play: {} поднял предмет {} ({} шт.)",
            player.name,
            item.stack.item,
            item.stack.count
        );

        slots.extend(player.inventory.add(item.stack));
    }

    slots
}

/// Отправляет сущность лежащего предмета.
///
/// Двумя пакетами: сперва сама сущность, потом её свойство — какой это
/// предмет. Без второго клиент нарисует пустоту: вид сущности он знает,
/// а что именно лежит — нет.
async fn send_dropped_item(stream: &mut TcpStream, item: &Dropped) -> io::Result<()> {
    let mut body = encode_varint(ADD_ENTITY);

    let kind = match item.block {
        Some(_) => FALLING_BLOCK_ENTITY_TYPE,
        None => ITEM_ENTITY_TYPE,
    };

    body.extend_from_slice(&encode_varint(item.entity_id));
    body.extend_from_slice(&item.uuid);
    body.extend_from_slice(&encode_varint(kind));
    push_f64(&mut body, item.x);
    push_f64(&mut body, item.y);
    push_f64(&mut body, item.z);
    types::push_velocity(&mut body, 0.0, 0.0, 0.0);

    // Лежащий предмет никуда не повёрнут.
    body.push(0);
    body.push(0);
    body.push(0);

    // Падающий блок говорит о себе прямо здесь: в этом числе у него
    // записано, какой он блок. Предмету это поле ни к чему.
    body.extend_from_slice(&encode_varint(item.block.unwrap_or(0)));

    write_body(stream, body).await?;

    // Дальше — только для предмета: у падающего блока всё уже сказано.
    if item.block.is_some() {
        return Ok(());
    }

    let mut data = encode_varint(SET_ENTITY_DATA);

    data.extend_from_slice(&encode_varint(item.entity_id));
    data.push(ITEM_PROPERTY);
    data.extend_from_slice(&encode_varint(PROPERTY_SLOT));
    push_slot(&mut data, Some(item.stack));
    data.push(PROPERTIES_END);

    write_body(stream, data).await
}

/// Отправляет подбор предмета: клиент рисует, как предмет летит к тому,
/// кто его поднял.
async fn send_pick_up(
    stream: &mut TcpStream,
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
    stream: &mut TcpStream,
    shared: &Shared,
    from: &mut usize,
    seen: &mut SeenItems,
    reader: journal::Reader,
) -> io::Result<()> {
    let changes = {
        let mut items = shared
            .items
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
            items::Change::Gone { entity_id } => {
                // Провалился в пустоту: просто убираем из мира, без показа
                // подбора.
                send_remove_entity(stream, entity_id).await?;
                seen.remove(&entity_id);
            }
            items::Change::Taken { entity_id, by, count } => {
                send_pick_up(stream, entity_id, by, count).await?;
                send_remove_entity(stream, entity_id).await?;
                seen.remove(&entity_id);
            }
        }
    }

    Ok(())
}

/// Досылает клиенту движение летящих предметов.
///
/// Предметы переносятся целиком, а не смещениями: в пакете переноса есть
/// ещё и скорость, а она клиенту нужна — он досчитывает полёт сам.
async fn send_item_moves(
    stream: &mut TcpStream,
    shared: &Shared,
    seen: &mut SeenItems,
) -> io::Result<()> {
    let items: Vec<Dropped> = shared
        .items
        .lock()
        .expect("предметы захвачены другим потоком")
        .items()
        .to_vec();

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

/// То, что о предмете надо рассказать клиенту.
fn shown_item(item: &Dropped) -> ShownItem {
    ShownItem {
        place: (item.x, item.y, item.z),
        speed: (item.vx, item.vy, item.vz),
        on_ground: item.on_ground,
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
/// Довесков сервер не передаёт: предметы у нас пока обычные.
fn push_slot(body: &mut Vec<u8>, stack: Option<Stack>) {
    match stack {
        Some(stack) => {
            body.extend_from_slice(&encode_varint(stack.count));
            body.extend_from_slice(&encode_varint(stack.item));
            body.extend_from_slice(&encode_varint(0)); // довесков не добавляем
            body.extend_from_slice(&encode_varint(0)); // и не убираем
        }
        None => body.extend_from_slice(&encode_varint(0)),
    }
}

/// Отправляет выбранный слот панели быстрого доступа.
///
/// Нужно при входе: игрок должен вернуться с тем же предметом в руке, с каким
/// вышел.
async fn send_held_slot(stream: &mut TcpStream, slot: i32) -> io::Result<()> {
    let mut body = encode_varint(SET_HELD_ITEM);

    body.extend_from_slice(&encode_varint(slot));

    write_body(stream, body).await
}

/// Отправляет инвентарь игрока целиком.
///
/// Этим же он и восстанавливается после перезахода: клиент показывает то,
/// что прислал сервер.
async fn send_container_content(
    stream: &mut TcpStream,
    player: &mut PlayerState,
) -> io::Result<()> {
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
    stream: &mut TcpStream,
    player: &mut PlayerState,
    slot: i32,
) -> io::Result<()> {
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
    stream: &mut TcpStream,
    shared: &Shared,
    (center_x, center_z): (i32, i32),
    sent: &mut SentChunks,
    radius: i32,
    limit: usize,
) -> io::Result<bool> {
    // Ушедшие из виду чанки клиент выбрасывает сам. Сервер должен об этом
    // знать, иначе, вернувшись, игрок увидит на их месте пустоту: сервер
    // считал бы, что они у клиента уже есть.
    let view = shared.properties.view_distance;

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
        let source = {
            let world = shared.world.lock().expect("мир захвачен другим потоком");

            Arc::new(world.source())
        };

        let mut coming = Vec::new();

        for (chunk_x, chunk_z) in part.iter().copied() {
            if shared
                .world
                .lock()
                .expect("мир захвачен другим потоком")
                .has_chunk(chunk_x, chunk_z)
            {
                continue;
            }

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
                .world
                .lock()
                .expect("мир захвачен другим потоком")
                .accept(chunk_x, chunk_z, ready);
        }

        for (chunk_x, chunk_z) in part.iter().copied() {
            send_chunk(stream, &shared.world, chunk_x, chunk_z).await?;
        }
    }

    Ok(all)
}

/// Отправляет Unload Chunk — «этот чанк можно забыть».
///
/// Поля идут наоборот привычному: сперва Z, потом X.
async fn send_unload_chunk(stream: &mut TcpStream, chunk_x: i32, chunk_z: i32) -> io::Result<()> {
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
    stream: &mut TcpStream,
    entity_id: i32,
    skin_parts: u8,
    main_hand: u8,
) -> io::Result<()> {
    let mut body = encode_varint(SET_ENTITY_DATA);

    body.extend_from_slice(&encode_varint(entity_id));

    // Свойства идут списком: номер, вид значения, само значение.
    body.push(MAIN_HAND_PROPERTY);
    body.extend_from_slice(&encode_varint(PROPERTY_HUMANOID_ARM));
    body.extend_from_slice(&encode_varint(main_hand as i32));

    body.push(SKIN_PARTS_PROPERTY);
    body.extend_from_slice(&encode_varint(PROPERTY_BYTE));
    body.push(skin_parts);

    body.push(PROPERTIES_END);

    write_body(stream, body).await
}

/// Кладёт в общий список то, что остальным нужно знать об игроке: где он
/// стоит и что держит в руке. Рассылают это уже их подключения.
fn publish_player(shared: &Shared, player: &PlayerState) {
    let mut players = shared
        .players
        .lock()
        .expect("список игроков захвачен другим потоком");

    players.set_position(&player.uuid, player.position());
    players.set_held(&player.uuid, player.inventory.held());
}

/// Сколько тактов миру от роду и который в нём час.
fn world_age(shared: &Shared) -> (i64, i64) {
    let world = shared.world.lock().expect("мир захвачен другим потоком");

    (world.tick() as i64, world.time_of_day())
}

/// Отправляет Update Time: который час в мире.
///
/// Часы у нас одни — те, что объявлены в реестре мировых часов, поэтому
/// запись одна, с номером 0. Скорость обычная, доля такта нулевая:
/// время идёт ровно по тактам сервера.
async fn send_time(stream: &mut TcpStream, age: i64, time: i64) -> io::Result<()> {
    let mut body = encode_varint(UPDATE_TIME);

    body.extend_from_slice(&age.to_be_bytes());
    body.extend_from_slice(&encode_varint(1)); // одни часы
    body.extend_from_slice(&encode_varint(0)); // их номер в реестре
    body.extend_from_slice(&encode_varlong(time));
    body.extend_from_slice(&0.0f32.to_be_bytes());
    body.extend_from_slice(&1.0f32.to_be_bytes());

    write_body(stream, body).await
}

/// Отправляет Set Equipment — что у сущности в основной руке.
///
/// Места предметов перечисляются подряд, и длина списка нигде не указана:
/// конец помечается старшим битом номера места. Мы сообщаем одно место,
/// поэтому бит не ставим — список сразу и заканчивается.
async fn send_equipment(
    stream: &mut TcpStream,
    entity_id: i32,
    held: Option<Stack>,
) -> io::Result<()> {
    let mut body = encode_varint(SET_EQUIPMENT);

    body.extend_from_slice(&encode_varint(entity_id));
    body.push(EQUIPMENT_MAIN_HAND);
    push_slot(&mut body, held);

    write_body(stream, body).await
}

/// Отправляет Remove Entities — сущность игрока пропадает из мира.
async fn send_remove_entity(stream: &mut TcpStream, entity_id: i32) -> io::Result<()> {
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
    stream: &mut TcpStream,
    shared: &Shared,
    player: &PlayerState,
    seen: &mut Seen,
) -> io::Result<()> {
    let others: Vec<Member> = shared
        .players
        .lock()
        .expect("список игроков захвачен другим потоком")
        .members()
        .to_vec();

    for member in others {
        if member.uuid == player.uuid {
            continue;
        }

        // Про того, кто появился на этом же круге, уже рассказал Add Entity:
        // его запись в seen появилась там же.
        let Some(shown) = seen.get(&member.entity_id).copied() else {
            continue;
        };

        let now = Shown {
            position: member.position,
            skin_parts: member.skin_parts,
            main_hand: member.main_hand,
            held: member.held,
        };

        if now == shown {
            continue;
        }

        if now.skin_parts != shown.skin_parts || now.main_hand != shown.main_hand {
            send_look(stream, member.entity_id, now.skin_parts, now.main_hand).await?;
        }

        if now.held != shown.held {
            send_equipment(stream, member.entity_id, now.held).await?;
        }

        if now.position != shown.position {
            send_move(stream, member.entity_id, shown.position, now.position).await?;
        }

        seen.insert(member.entity_id, now);
    }

    Ok(())
}

/// Рассказывает клиенту о перемещении одной сущности — самыми дешёвыми
/// пакетами, каких хватает.
///
/// Смещение записывается коротким числом, поэтому большое перемещение так
/// не рассказать: тогда сообщаются координаты целиком.
async fn send_move(
    stream: &mut TcpStream,
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
async fn send_teleport(stream: &mut TcpStream, entity_id: i32, to: Position) -> io::Result<()> {
    send_teleport_with_speed(stream, entity_id, to, (0.0, 0.0, 0.0)).await
}

/// То же, но со скоростью сущности.
///
/// Скорость здесь не украшение: клиент досчитывает движение сущности сам,
/// между пакетами сервера. Если ему не сказать, что предмет остановился, он
/// продолжит тянуть его вниз — и предмет уедет в пол.
async fn send_teleport_with_speed(
    stream: &mut TcpStream,
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
async fn send_system_chat(stream: &mut TcpStream, text: &str) -> io::Result<()> {
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
async fn send_commands(stream: &mut TcpStream) -> io::Result<()> {
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

        let children = [1, 6, 7, 8, 11, 13, 15, 17, 19, 21, 23, 25];

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
fn suggestions(payload: &[u8], shared: &Shared) -> Option<Suggestions> {
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
        matches: hints_for(&text, typed, shared),
    })
}

/// Что подсказать: имена игроков, названия предметов или что-то ещё —
/// смотря какая команда и какое слово по счёту дописывается.
fn hints_for(text: &str, typed: &str, shared: &Shared) -> Vec<String> {
    let line = text.strip_prefix('/').unwrap_or(text);
    let mut words = line.split(' ');

    let command = words.next().unwrap_or("");
    // Сколько слов уже написано после самой команды: дописываемое считается.
    let written = words.count();

    match (command, written) {
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
async fn send_suggestions(stream: &mut TcpStream, hints: &Suggestions) -> io::Result<()> {
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
async fn send_disconnect(stream: &mut TcpStream, reason: &str) -> io::Result<()> {
    let mut body = encode_varint(DISCONNECT);

    push_text_component(&mut body, reason);

    write_body(stream, body).await
}

/// Отправляет Keep Alive — без него клиент решает, что сервер пропал,
/// и отключается.
async fn send_keep_alive(stream: &mut TcpStream, id: i64) -> io::Result<()> {
    let mut body = encode_varint(KEEP_ALIVE);

    push_i64(&mut body, id);

    write_body(stream, body).await
}

/// Отправляет Set Default Spawn Position — точку возрождения.
async fn send_default_spawn_position(
    stream: &mut TcpStream,
    spawn: (f64, f64, f64),
) -> io::Result<()> {
    let mut body = encode_varint(SET_DEFAULT_SPAWN_POSITION);

    body.extend_from_slice(&encode_string(DIMENSION));
    push_packed_position(&mut body, spawn.0 as i32, spawn.1 as i32, spawn.2 as i32);
    push_f32(&mut body, 0.0); // поворот
    push_f32(&mut body, 0.0); // наклон

    write_body(stream, body).await
}

/// Отправляет Game Event 13 — «начать ждать чанки».
async fn send_game_event_start_waiting(stream: &mut TcpStream) -> io::Result<()> {
    let mut body = encode_varint(GAME_EVENT);

    body.push(13); // событие: начало ожидания чанков
    push_f32(&mut body, 0.0); // значение для этого события не используется

    write_body(stream, body).await
}

/// Отправляет Set Health: здоровье, сытость и насыщение.
///
/// Пока урона нет, всё полное — но без этого пакета клиент не считает вход
/// законченным: полоски здоровья и еды остаются пустыми, а некоторые
/// клиенты не начинают действовать в мире.
async fn send_health(stream: &mut TcpStream) -> io::Result<()> {
    let mut body = encode_varint(SET_HEALTH);

    push_f32(&mut body, 20.0); // здоровье
    body.extend_from_slice(&encode_varint(20)); // сытость
    push_f32(&mut body, 5.0); // насыщение

    write_body(stream, body).await
}

/// Отправляет Set Center Chunk — центральный чанк для клиента.
async fn send_center_chunk(stream: &mut TcpStream, chunk: (i32, i32)) -> io::Result<()> {
    let mut body = encode_varint(SET_CENTER_CHUNK);

    body.extend_from_slice(&encode_varint(chunk.0)); // чанк X
    body.extend_from_slice(&encode_varint(chunk.1)); // чанк Z

    write_body(stream, body).await
}

/// Досылает клиенту события мира, которых он ещё не видел, — в том
/// порядке, в каком они случились.
///
/// Порядок важен: действие поршня обязано уйти раньше изменения «поршень
/// выдвинут» (иначе клиент ход не нарисует — «Sending an extension action to
/// an already-extended piston has no effect»), а воздух на месте сломанного
/// блока — раньше действия, в которое клиент этот воздух заполнит.
/// Конец такта для этого подключения: всё, что случилось в мире, и ответы
/// на собственные пакеты игрока.
async fn flush_tick(
    stream: &mut TcpStream,
    shared: &Shared,
    player: &mut PlayerState,
    world_changes: &mut usize,
    pending: &mut Vec<Reaction>,
) -> io::Result<()> {
    send_pending_world_events(stream, shared, world_changes, player.reader).await?;

    for reaction in pending.drain(..) {
        apply_reaction(stream, shared, player, &reaction).await?;
    }

    Ok(())
}

/// Отправляет клиенту события мира, накопившиеся с прошлого раза, в том
/// порядке, в каком они случились.
///
/// Изменения блоков идут пачкой по секциям, как у оригинала: одно — Block
/// Update, несколько в одной секции — Update Section Blocks. Действие, звук
/// или прямое изменение (повтор блока после щелчка) — граница пачки.
async fn send_pending_world_events(
    stream: &mut TcpStream,
    shared: &Shared,
    sent: &mut usize,
    reader: journal::Reader,
) -> io::Result<()> {
    let (events, count) = shared
        .world
        .lock()
        .expect("мир захвачен другим потоком")
        .events_since(*sent, reader);

    *sent = count;

    let mut pending: Vec<world::BlockChange> = Vec::new();

    for event in &events {
        match event {
            WorldEvent::Change(change) if change.direct => {
                send_block_changes(stream, &mut pending).await?;
                send_block_update(stream, change.x, change.y, change.z, change.state).await?;
            }
            WorldEvent::Change(change) => pending.push(*change),
            WorldEvent::Action(action) => {
                send_block_changes(stream, &mut pending).await?;

                let mut body = encode_varint(BLOCK_ACTION);

                push_packed_position(&mut body, action.x, action.y, action.z);
                body.push(action.action);
                body.push(action.param);
                body.extend_from_slice(&encode_varint(action.block));

                write_body(stream, body).await?;
            }
            WorldEvent::Sound(sound) => {
                send_block_changes(stream, &mut pending).await?;
                send_sound(stream, sound).await?;
            }
        }
    }

    send_block_changes(stream, &mut pending).await
}

/// Отправляет накопленные изменения блоков: по секциям, в порядке первого
/// появления секции; повторное изменение того же места в пачке — последнее
/// слово за последним.
async fn send_block_changes(
    stream: &mut TcpStream,
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

    for ((sx, sy, sz), group) in sections {
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

    Ok(())
}

/// Отправляет Sound Effect — звук в точке мира.
///
/// Звук называется словом, а не числом: у числа свой список, который
/// у клиента и у нас может не совпасть, а названия одинаковы всегда.
/// Место передаётся в восьмых долях блока — так в протоколе.
async fn send_sound(stream: &mut TcpStream, sound: &WorldSound) -> io::Result<()> {
    let mut body = encode_varint(SOUND_EFFECT);

    body.extend_from_slice(&encode_varint(0)); // звук назван словом, а не номером
    body.extend_from_slice(&encode_string(sound.name));
    body.push(0); // своей дальности слышимости у него нет

    body.extend_from_slice(&encode_varint(SOUND_BLOCK));

    // Середина блока, в восьмых долях.
    push_i32(&mut body, sound.x * 8 + 4);
    push_i32(&mut body, sound.y * 8 + 4);
    push_i32(&mut body, sound.z * 8 + 4);

    body.extend_from_slice(&sound.volume.to_be_bytes());
    body.extend_from_slice(&sound.pitch.to_be_bytes());
    body.extend_from_slice(&0i64.to_be_bytes()); // случайность нам не нужна

    write_body(stream, body).await
}

/// Отправляет Block Update — какое состояние блока клиент должен показать.
///
/// Это ответ на ломание и установку: клиент показывает изменение сразу, ещё до
/// ответа сервера (своё предсказание), а этим пакетом сервер сообщает, что
/// получилось на самом деле.
async fn send_block_update(
    stream: &mut TcpStream,
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
async fn send_acknowledge_block_change(stream: &mut TcpStream, sequence: i32) -> io::Result<()> {
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
    stream: &mut TcpStream,
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
                };

                if seen.insert(member.entity_id, shown).is_none() {
                    send_add_entity(stream, member).await?;
                    send_look(
                        stream,
                        member.entity_id,
                        member.skin_parts,
                        member.main_hand,
                    )
                    .await?;
                    send_equipment(stream, member.entity_id, member.held).await?;
                }
            }
            Change::Joined(_) => {}
            Change::Left { uuid, entity_id } if *uuid != player.uuid => {
                send_remove_entity(stream, *entity_id).await?;
                send_player_info_remove(stream, uuid).await?;

                seen.remove(entity_id);
            }
            Change::Left { .. } => {}
        }
    }

    *sent = count;

    Ok(())
}

/// Досылает клиенту сообщения чата, которых он ещё не видел.
async fn send_pending_chat(
    stream: &mut TcpStream,
    shared: &Shared,
    sent: &mut usize,
) -> io::Result<()> {
    let (lines, count) = {
        let chat = shared.chat.lock().expect("чат захвачен другим потоком");

        (chat.since(*sent).to_vec(), chat.count())
    };

    for line in &lines {
        send_system_chat(stream, line).await?;
    }

    *sent = count;

    Ok(())
}

/// Отправляет чанк с указанными координатами — из текущего состояния мира.
async fn send_chunk(
    stream: &mut TcpStream,
    world: &Mutex<World>,
    chunk_x: i32,
    chunk_z: i32,
) -> io::Result<()> {
    let mut body = encode_varint(CHUNK_DATA);

    push_i32(&mut body, chunk_x);
    push_i32(&mut body, chunk_z);

    // Карты высот. Клиент умеет обходиться без них: недостающие он заполняет
    // минимальной высотой.
    body.extend_from_slice(&encode_varint(0));

    // Секции чанка — одним блоком байт с длиной впереди. Собранное тело
    // хранится в самом чанке: пока в нём ничего не поставили, второму игроку
    // оно достаётся готовым.
    let sections = {
        let world = world.lock().expect("мир захвачен другим потоком");

        world.chunk_packet(chunk_x, chunk_z, |sections, biomes| {
            let mut out = Vec::new();

            for section in sections {
                push_section(&mut out, section.as_deref(), Some(biomes));
            }

            out
        })
    };

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

    // Блоков с дополнительными данными (сундуков, табличек) нет.
    body.extend_from_slice(&encode_varint(0));

    push_light(&mut body);

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
fn push_section(out: &mut Vec<u8>, states: Option<&[u16]>, biomes: Option<&[i32]>) {
    /// Больше этого числа разных состояний — и палитра перестаёт экономить.
    const MAX_PALETTE: usize = 16;

    // Состояния в памяти лежат в двух байтах, а в пакет идут числами
    // со знаком — разворачиваем по дороге.
    let empty = [world::AIR as u16; world::SECTION_BLOCKS];
    let states = states.unwrap_or(&empty);

    let mut palette: Vec<i32> = Vec::new();
    let mut block_count: i32 = 0;

    for packed in states {
        let state = *packed as i32;

        if state != world::AIR {
            block_count += 1;
        }

        if !palette.contains(&state) {
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
/// Биом задаётся на каждые четыре блока: в секции это 4×4×4 = 64 ячейки.
/// По высоте биом у нас один на весь столбец, поэтому четыре слоя ячеек
/// повторяют один и тот же рисунок 4×4.
fn push_biomes(out: &mut Vec<u8>, biomes: Option<&[i32]>) {
    /// Ячеек биома на сторону секции.
    const CELLS: usize = 4;

    let Some(biomes) = biomes else {
        // Чанка ещё нет — пусть будет равнина: она в реестре первой... а
        // точнее, под тем номером, под каким её объявил сервер.
        push_single_value_container(out, plains_number());
        return;
    };

    let mut palette: Vec<i32> = Vec::new();

    for biome in biomes {
        if !palette.contains(biome) {
            palette.push(*biome);
        }
    }

    if palette.len() == 1 {
        push_single_value_container(out, palette[0]);
        return;
    }

    // Битов на запись — столько, чтобы хватило на всю палитру.
    let bits = usize::BITS - (palette.len() - 1).leading_zeros();
    let bits = bits.max(1) as usize;

    out.push(bits as u8);
    out.extend_from_slice(&encode_varint(palette.len() as i32));

    for biome in &palette {
        out.extend_from_slice(&encode_varint(*biome));
    }

    // Порядок ячеек тот же, что у блоков: снизу вверх, внутри — по Z, потом
    // по X. Слои по высоте одинаковы.
    let mut indexes: Vec<u32> = Vec::with_capacity(CELLS * CELLS * CELLS);

    for _ in 0..CELLS {
        for cell in biomes {
            let place = palette
                .iter()
                .position(|biome| biome == cell)
                .expect("биом обязан быть в палитре");

            indexes.push(place as u32);
        }
    }

    for long in pack_values(&indexes, bits) {
        push_i64(out, long);
    }
}

/// Номер равнины в реестре биомов: под ним клиенту уходят места, которых
/// сервер ещё не сложил.
fn plains_number() -> i32 {
    world::terrain::Biome::Plains.number()
}

/// Добавляет контейнер состояний блоков с палитрой: 4 бита на блок, палитра
/// из встреченных состояний, данные — номера состояний внутри палитры.
fn push_palette_container(out: &mut Vec<u8>, states: &[u16], palette: &[i32]) {
    /// Битов на запись: минимум для состояний блоков.
    const BITS: usize = 4;

    out.push(BITS as u8);
    out.extend_from_slice(&encode_varint(palette.len() as i32));

    for state in palette {
        out.extend_from_slice(&encode_varint(*state));
    }

    // Подряд идущие блоки обычно одинаковы, поэтому место в палитре ищется
    // заново только когда состояние сменилось.
    let mut indexes: Vec<u32> = Vec::with_capacity(states.len());
    let mut last: Option<(u16, u32)> = None;

    for packed in states {
        let place = match last {
            Some((state, place)) if state == *packed => place,
            _ => {
                let found = palette
                    .iter()
                    .position(|state| *state == *packed as i32)
                    .expect("состояние обязано быть в палитре") as u32;

                last = Some((*packed, found));
                found
            }
        };

        indexes.push(place);
    }

    for long in pack_values(&indexes, BITS) {
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

/// Добавляет данные освещения: небо освещено во всех секциях.
///
/// Байты одинаковы для всех чанков и не меняются, поэтому собираются один
/// раз за всё время работы сервера: их около пятидесяти килобайт, и собирать
/// их заново на каждый чанк каждому игроку — впустую.
fn push_light(out: &mut Vec<u8>) {
    static LIGHT: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();

    out.extend_from_slice(LIGHT.get_or_init(build_light));
}

/// Собирает данные освещения.
fn build_light() -> Vec<u8> {
    let mut out = Vec::new();

    push_light_bytes(&mut out);
    out
}

/// Пишет данные освещения в готовый буфер.
fn push_light_bytes(out: &mut Vec<u8>) {
    /// В масках на две секции больше, чем в мире: по одной снизу и сверху.
    const MASK_BITS: i32 = world::SECTIONS + 2;

    /// Все биты до `MASK_BITS` включительно.
    const ALL_SECTIONS: i64 = (1i64 << MASK_BITS) - 1;

    // Маски: небесный свет есть у всех секций, блочного света нет ни у одной.
    // Пустые маски — обратные к ним.
    push_bitset(out, &[ALL_SECTIONS]);
    push_bitset(out, &[]);
    push_bitset(out, &[]);
    push_bitset(out, &[ALL_SECTIONS]);

    // Массивы небесного света: по одному на каждый установленный бит маски.
    // Каждый массив — половина байта на блок, 15 (ярко) во всех.
    out.extend_from_slice(&encode_varint(MASK_BITS));
    for _ in 0..MASK_BITS {
        out.extend_from_slice(&encode_varint(2048));
        out.extend_from_slice(&[0xFFu8; 2048]);
    }

    // Массивов блочного света нет.
    out.extend_from_slice(&encode_varint(0));
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
    stream: &mut TcpStream,
    shared: &Shared,
    player: &mut PlayerState,
    entered: Entered,
) -> io::Result<()> {
    /// Serverbound Client Tick End — приходит каждый тик и только засоряет лог.
    const CLIENT_TICK_END: i32 = 0x0D;

    let mut probe = [0u8; 1];
    let mut keep_alive_id: i64 = 1;
    let mut last_keep_alive = Instant::now();
    let mut last_time_age: i64 = -1;

    let mut world_changes = entered.world_changes;
    let mut player_changes = entered.player_changes;
    let mut chat_lines = entered.chat_lines;
    let mut item_changes = entered.item_changes;
    let mut seen_items = entered.seen_items;
    let mut sent_chunks = entered.sent_chunks;
    let mut standing_in = player.chunk();
    let mut seen = entered.seen;

    // Весь ли обзор уже прислан. При входе прислан только ближний квадрат —
    // даль догружается здесь, порциями.
    let mut all_chunks_sent = false;

    // Круг цикла начинается либо с пакета клиента, либо с конца такта: всё,
    // что случилось в мире за такт, должно уйти клиенту сразу, а не тогда,
    // когда он в следующий раз что-нибудь пришлёт.
    let mut ticks = shared.ticks.subscribe();
    let mut starts = shared.tick_starts.subscribe();
    let mut pending: Vec<Reaction> = Vec::new();

    loop {
        let ready = tokio::select! {
            ready = timeout(KEEP_ALIVE_INTERVAL, stream.peek(&mut probe)) => Some(ready),
            _ = ticks.changed() => None,
        };

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
            let (age, time) = world_age(shared);

            if age % TIME_EVERY == 0 && age != last_time_age {
                send_time(stream, age, time).await?;
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

        if !all_chunks_sent {
            let view = shared.properties.view_distance;

            all_chunks_sent = send_chunks_around(
                stream,
                shared,
                chunk,
                &mut sent_chunks,
                view,
                CHUNKS_PER_STEP,
            )
            .await?;
        }

        send_pending_player_changes(stream, shared, player, &mut player_changes, &mut seen).await?;
        send_other_moves(stream, shared, player, &mut seen).await?;
        send_pending_chat(stream, shared, &mut chat_lines).await?;

        // Поручения от команд: выгнать, перенести. Их отдаёт не то
        // подключение, которого они касаются, поэтому забираем их отсюда.
        for order in take_orders(shared, &player.uuid) {
            match order {
                Order::Kick { reason } => {
                    log_info!("{} выгнан: {}", player.name, reason);
                    send_disconnect(stream, &reason).await?;

                    return Ok(());
                }
                Order::Teleport { x, y, z } => {
                    player.x = x;
                    player.y = y;
                    player.z = z;

                    send_player_position(stream, player).await?;
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
                        shared,
                        chunk,
                        &mut sent_chunks,
                        NEAR_CHUNKS,
                        usize::MAX,
                    )
                    .await?;
                    all_chunks_sent = false;

                    log_debug!(
                        "Play: игрок {} перенесён в {:.2} {:.2} {:.2}",
                        player.name,
                        x,
                        y,
                        z
                    );
                }

                // Выданные предметы кладутся в инвентарь; если места нет,
                // лишнее просто не выдаётся — как в игре при полном рюкзаке.
                Order::Give { item, count } => {
                    let left = player.inventory.add(Stack::new(item, count));

                    for slot in left {
                        send_container_slot(stream, player, slot).await?;
                    }

                    log_debug!(
                        "Play: игроку {} выдано: предмет {} × {}",
                        player.name,
                        item,
                        count
                    );
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
            shared,
            &mut item_changes,
            &mut seen_items,
            player.reader,
        )
        .await?;
        send_item_moves(stream, shared, &mut seen_items).await?;

        match ready {
            // Клиент молчит — на этом круге Keep Alive уже отправлен выше.
            // Такт закончился: всё, что за него случилось в мире, уходит
            // клиенту разом и в порядке, в каком случилось, — как у оригинала,
            // где пакеты копятся и уходят в конце такта. Следом — ответы на
            // его собственные действия: подтверждение должно идти после
            // изменения мира, иначе клиент на миг покажет пустое место.
            None => {
                flush_tick(stream, shared, player, &mut world_changes, &mut pending).await?;
            }
            Some(Err(_)) => {}
            // Клиент закрыл соединение.
            Some(Ok(Ok(0))) => {
                log_debug!("Play: клиент закрыл соединение");
                return Ok(());
            }
            // Есть данные — читаем целый пакет.
            Some(Ok(Ok(_))) => match read_packet(stream).await {
                Ok(packet) => {
                    if packet.id == CLIENT_TICK_END {
                        continue;
                    }

                    // Действия с миром применяются на границе тактов, как у
                    // оригинала: щелчок, ломание, установка и команда ждут,
                    // пока закончится текущий такт, и ложатся в начало
                    // следующего. Иначе щелчок по рычагу и ход поршня от
                    // него разъезжались бы по тактам как попало.
                    let gated = matches!(
                        packet.id,
                        USE_ITEM_ON | PLAYER_ACTION | USE_ITEM | SERVERBOUND_CHAT_COMMAND
                    );

                    if gated {
                        shared
                            .waiting_for_tick
                            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
                        starts.borrow_and_update();
                        let _ = starts.changed().await;
                    }

                    let reaction = handle_client_packet(&packet, player, shared);

                    if gated {
                        shared
                            .waiting_for_tick
                            .fetch_sub(1, std::sync::atomic::Ordering::AcqRel);
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
                Err(e) if is_client_disconnect(&e) => {
                    log_debug!("Play: клиент закрыл соединение ({})", e.kind());
                    return Ok(());
                }
                Err(e) => return Err(e),
            },
            Some(Ok(Err(e))) if is_client_disconnect(&e) => {
                log_debug!("Play: клиент закрыл соединение ({})", e.kind());
                return Ok(());
            }
            Some(Ok(Err(e))) => return Err(e),
        }
    }
}

/// Отправляет клиенту ответ на его пакет: подтверждение, поправки блоков,
/// режим игры, текст, содержимое инвентаря, подсказки.
async fn apply_reaction(
    stream: &mut TcpStream,
    shared: &Shared,
    player: &mut PlayerState,
    reaction: &Reaction,
) -> io::Result<()> {
    if let Some(sequence) = reaction.acknowledge {
        send_acknowledge_block_change(stream, sequence).await?;
    }

    // Если блок не поставлен, клиенту надо сказать, что там
    // на самом деле: свой блок он уже показал, не дожидаясь
    // сервера, и без поправки тот остался бы призраком.
    for (x, y, z) in &reaction.confirm {
        let state = shared
            .world
            .lock()
            .expect("мир захвачен другим потоком")
            .get_block(*x, *y, *z);

        send_block_update(stream, *x, *y, *z, state).await?;
    }

    if let Some(game_mode) = reaction.game_mode {
        send_game_mode(stream, &player.uuid, game_mode).await?;
    }

    if let Some(reply) = &reaction.reply {
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

    Ok(())
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
    let payload = packet.payload();
    let mut reaction = Reaction::default();

    match packet.id {
        SERVERBOUND_KEEP_ALIVE => {
            log_debug!("Play: клиент ответил на Keep Alive");
        }
        SET_PLAYER_POSITION | SET_PLAYER_POSITION_AND_ROTATION => {
            let Some((x, y, z)) = read_position(payload) else {
                log_debug!(
                    "Play: пакет о перемещении короче ожидаемого — длина тела {} байт",
                    packet.length
                );
                return reaction;
            };

            // В пакете «позиция и поворот» углы идут сразу за координатами.
            if packet.id == SET_PLAYER_POSITION_AND_ROTATION
                && let Some((yaw, pitch)) = read_rotation(payload, POSITION_BYTES)
            {
                player.set_rotation(yaw, pitch);
            }

            player.set_position(x, y, z);
        }
        SET_PLAYER_ROTATION => {
            if let Some((yaw, pitch)) = read_rotation(payload, 0) {
                player.set_rotation(yaw, pitch);
            }
        }
        SET_PLAYER_ON_GROUND => {
            if let Some(&flag) = payload.first() {
                player.set_on_ground(flag != 0);
            }
        }
        // Разобрать пакет могло и не получиться: тогда отвечать не на что.
        PLAYER_ACTION => {
            if let Some(answer) = break_block(payload, player, shared) {
                reaction = answer;
            }
        }
        USE_ITEM_ON => {
            if let Some(answer) = place_block(payload, player, shared) {
                reaction = answer;
            }
        }
        SERVERBOUND_CHAT_MESSAGE => {
            // Поля после самого текста (время, подпись, счётчики) серверу пока
            // не нужны: подписи мы не проверяем, а текст стоит первым полем.
            if let Some(text) = Cursor::new(payload).string() {
                log_debug!("Play: {} пишет в чат: {}", player.name, text);

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

                let source = Source::Player {
                    uuid: player.uuid,
                    name: player.name.clone(),
                };
                let answer = commands::run(&command, &source, shared);

                if !answer.reply.is_empty() {
                    // Как у обычного сервера: «[Имя: что вышло]».
                    log_info!("[{}: {}]", player.name, answer.reply);
                    reaction.reply = Some(answer.reply);
                }

                // Смена режима — единственное, что разбор команды поручает
                // подключению: у него есть сведения о самом игроке.
                if let Effect::GameMode { uuid, mode } = answer.effect
                    && uuid == player.uuid
                {
                    player.game_mode = mode;
                    reaction.game_mode = Some(mode);
                }
            }
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
                throw_out(shared, player, stack);
            }

            reaction.inventory = true;
        }
        CLOSE_CONTAINER => {
            // Окно закрыто: то, что игрок держал мышкой, надо вернуть в
            // инвентарь, иначе предмет пропадёт.
            let changed = player.inventory.put_cursor_back();

            if !changed.is_empty() {
                reaction.inventory = true;
            }
        }
        TAB_COMPLETE_REQUEST => {
            if let Some(answer) = suggestions(payload, shared) {
                reaction.suggestions = Some(answer);
            }
        }
        PICK_ITEM_FROM_BLOCK => {
            if let Some(answer) = pick_block(payload, player, shared) {
                reaction = answer;
            }
        }
        PLAYER_INPUT => {
            if let Some(flags) = payload.first() {
                player.sneaking = flags & INPUT_SNEAK != 0;
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

                let mut players = shared
                    .players
                    .lock()
                    .expect("список игроков захвачен другим потоком");

                players.set_skin_parts(&player.uuid, look.skin_parts);
                players.set_main_hand(&player.uuid, look.main_hand);
            }
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
            }
        }
        SET_CREATIVE_MODE_SLOT => {
            read_creative_slot(payload, player);
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
    /// Начал копать.
    const STARTED_DIGGING: i32 = 0;

    /// Докопал.
    const FINISHED_DIGGING: i32 = 2;

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
            Stack::new(held.item, 1)
        };

        throw_out(shared, player, thrown);

        return Some(Reaction {
            slots: vec![inventory::FIRST_HOTBAR + player.inventory.selected()],
            ..Reaction::default()
        });
    }

    let breaks_now = if player.game_mode == GAME_MODE_CREATIVE {
        status == STARTED_DIGGING
    } else {
        status == FINISHED_DIGGING
    };

    if !breaks_now {
        // Начало копания, отмена копания и прочие действия мир не меняют.
        return Some(Reaction::default());
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

    let mut world = shared
        .world
        .lock()
        .expect("мир захвачен другим потоком");

    let broken = world.get_block(x, y, z);

    // Воздух ломать нечего: такое действие подтверждения не требует.
    if broken == world::AIR {
        log_debug!("Play: в {} {} {} и так пусто", x, y, z);
        return Some(Reaction::default());
    }

    world.set_block(x, y, z, world::AIR);

    // Дверь занимает два блока, и ломается тоже целиком: иначе от неё
    // осталась бы висеть в воздухе вторая половинка.
    if let Some(orientation) = blocks::orientation_of(broken)
        && placing::is_door(orientation)
        && let Some(half) = orientation.value(broken, "half")
    {
        let paired_y = if half == "lower" { y + 1 } else { y - 1 };

        if placing::is_same_door(orientation, world.get_block(x, paired_y, z)) {
            world.set_block(x, paired_y, z, world::AIR);
            placing::refresh_neighbours(&mut world, x, paired_y, z);
        }
    }

    // Поршень состоит из двух половин, и ломается целиком — с какой
    // стороны его ни ломай.
    if let Some((px, py, pz)) = redstone::piston_pair(&world, (x, y, z), broken) {
        world.set_block(px, py, pz, world::AIR);
    }

    // Соседи могли срастись с этим блоком — теперь им надо расцепиться.
    placing::refresh_neighbours(&mut world, x, y, z);

    // Редстоун должен узнать, что блока не стало: провод на нём пропадёт,
    // а запитанное им погаснет.
    redstone::settle(&mut world);

    // Сломанное записываем сразу — как и построенное.
    world.save_if_needed();

    log_debug!("Play: сломан блок в {} {} {}", x, y, z);

    // В выживании из сломанного блока выпадает предмет — он падает на землю
    // и лежит, пока его не подберут. В творческом режиме блоки просто
    // исчезают: предметы там берут из меню.
    if player.game_mode != GAME_MODE_CREATIVE {
        let tool = player.inventory.held().map(|stack| stack.item);
        let luck = world.random_unit();

        match blocks::drop_by_luck(broken, tool, luck) {
            Some(item) => {
                // Мир отпускаем: класть предмет будем под другим замком.
                drop(world);

                // Выпавшее из блока не бросают — оно просто падает с того
                // места, где был блок.
                drop_into_world(
                    shared,
                    Stack::new(item, 1),
                    (x as f64 + 0.5, y as f64 + 0.25, z as f64 + 0.5),
                    (0.0, 0.0, 0.0),
                    items::MINED_DELAY,
                );

                log_debug!("Play: из блока в {} {} {} выпал предмет {}", x, y, z, item);
            }
            None => log_debug!("Play: из блока {} ничего не выпало", broken),
        }
    }

    Some(Reaction::accepted(sequence))
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
fn place_block(payload: &[u8], player: &mut PlayerState, shared: &Shared) -> Option<Reaction> {
    let mut cursor = Cursor::new(payload);

    let hand = cursor.varint()?;
    let (clicked_x, clicked_y, clicked_z) = cursor.position()?;
    let face = cursor.varint()?;

    // Из точки щелчка серверу нужна только высота: от неё зависит, какая
    // половина блока достанется ступени или плите.
    let _cursor_x = cursor.f32()?;
    let cursor_y = cursor.f32()?;
    let _cursor_z = cursor.f32()?;
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

    // Щелчок по рычагу, кнопке, двери, повторителю — это не установка, а
    // действие с ними, и рука тут ни при чём: щёлкают и пустой. Присев,
    // игрок всё-таки ставит блок.
    if !player.sneaking {
        let mut world = shared.world.lock().expect("мир захвачен другим потоком");

        if redstone::use_block(&mut world, (clicked_x, clicked_y, clicked_z)) {
            world.save_if_needed();
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

            // Порядок снят с оригинала чёрным ящиком: сперва повтор за
            // гранью, потом перемены доходят до ближайших соседей (провод
            // у рычага загорается), потом повтор самого блока, и только
            // потом перемены идут дальше — к тому, что за проводом.
            world.tell_directly(clicked_x + dx, clicked_y + dy, clicked_z + dz);
            redstone::settle_once(&mut world);
            world.tell_directly(clicked_x, clicked_y, clicked_z);
            redstone::settle(&mut world);

            // Что деталь делает после всего этого: нота музыкального блока
            // у оригинала звучит последней.
            redstone::after_use(&mut world, (clicked_x, clicked_y, clicked_z));

            return Some(Reaction::accepted(sequence));
        }
    }

    // Какой блок ставить, решает предмет в руке: сервер ищет его в таблице
    // блоков. Если предмет блока не даёт, ставить нечего, но подтвердить
    // действие всё равно нужно — подтверждение означает «сервер здесь ничего
    // не менял», и клиент уберёт свой предсказанный блок, если он его показал.
    let Some(item) = player.inventory.held().map(|stack| stack.item) else {
        log_debug!("Play: содержимое слота неизвестно — ставить нечего");
        return Some(Reaction::accepted(sequence));
    };

    // Вёдра блоков не ставят, но именно ими в мире появляется вода и лава.
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

    let Some((name, default_state)) = blocks::block_for_item(item) else {
        log_debug!("Play: предмет {} блоком не является — ставить нечего", item);
        return Some(Reaction::accepted(sequence));
    };

    // Факел на стене — другой блок, настенный; на потолок факел не вешается.
    let Some((name, default_state)) = placing::block_for_face(name, default_state, face).or_else(|| {
        log_debug!("Play: {} на эту грань не ставится", name);
        None
    }) else {
        return Some(Reaction::accepted(sequence));
    };

    let click = placing::Click {
        face,
        cursor_y,
        yaw: player.yaw,
        pitch: player.pitch,
    };

    let orientation = blocks::orientation(name);

    let mut world = shared
        .world
        .lock()
        .expect("мир захвачен другим потоком");

    // Плита, поставленная на плиту того же вида, слипается с ней в двойную:
    // занимает место того же блока, а не соседнего.
    if let Some(orientation) = orientation
        && let Some(double) = placing::merge_into_double(
            orientation,
            world.get_block(clicked_x, clicked_y, clicked_z),
            &click,
        )
    {
        world.set_block(clicked_x, clicked_y, clicked_z, double);
        placing::refresh_neighbours(&mut world, clicked_x, clicked_y, clicked_z);

        log_debug!(
            "Play: плита {} стала двойной в {} {} {}",
            name,
            clicked_x,
            clicked_y,
            clicked_z
        );

        return Some(Reaction::accepted(sequence));
    }

    let (x, y, z) = offset_towards(clicked_x, clicked_y, clicked_z, face);

    // Ставить можно в пустое место или туда, где стоит жидкость: блок её
    // вытесняет, как в игре.
    if !placing::is_replaceable(world.get_block(x, y, z)) {
        return Some(Reaction::accepted(sequence));
    }

    // Состояние считается по тому, как игрок щёлкнул, и по тому, что стоит
    // вокруг: забор срастается с соседями, а у кнопки поворот зависит
    // от грани щелчка.
    let state = orientation
        .and_then(|orientation| placing::state_in_world(orientation, &click, &world, x, y, z))
        .unwrap_or(default_state);

    // Проводу, факелу, рычагу нужна опора: в воздухе они не держатся.
    if !placing::has_support(orientation, state, &world, x, y, z) {
        log_debug!("Play: {} в {} {} {} не на что опереться", name, x, y, z);
        return Some(Reaction::refused(sequence, &[(x, y, z)]));
    }

    // Дверь занимает два блока по высоте и ставится сразу целой: нижняя
    // половина в этот блок, верхняя — в тот, что над ним.
    if let Some(orientation) = orientation
        && placing::is_door(orientation)
    {
        if !placing::is_replaceable(world.get_block(x, y + 1, z)) {
            log_debug!("Play: над дверью не пусто — в {} {} {} она не встанет", x, y, z);
            return Some(Reaction::refused(sequence, &[(x, y, z), (x, y + 1, z)]));
        }

        // Двери нужна опора снизу: без неё она повисла бы в воздухе.
        if !blocks::solid_top(world.get_block(x, y - 1, z)) {
            log_debug!("Play: под дверью нет опоры — в {} {} {} она не встанет", x, y, z);
            return Some(Reaction::refused(sequence, &[(x, y, z), (x, y + 1, z)]));
        }

        let Some(upper) = placing::other_half(orientation, state, "upper") else {
            log_debug!("Play: не удалось собрать вторую половину двери {}", name);
            return Some(Reaction::accepted(sequence));
        };

        if placing::blocks_player(x, y, z, state, player.x, player.y, player.z)
            || placing::blocks_player(x, y + 1, z, upper, player.x, player.y, player.z)
        {
            log_debug!("Play: дверь в {} {} {} задела бы игрока — не ставим", x, y, z);
            return Some(Reaction::refused(sequence, &[(x, y, z), (x, y + 1, z)]));
        }

        world.set_block(x, y, z, state);
        world.set_block(x, y + 1, z, upper);
        placing::refresh_neighbours(&mut world, x, y, z);
        placing::refresh_neighbours(&mut world, x, y + 1, z);
        redstone::settle(&mut world);
        world.save_if_needed();

        log_debug!("Play: поставлена дверь {} ({}) в {} {} {}", name, state, x, y, z);

        return Some(Reaction {
            slots: spend_placed(player),
            ..Reaction::accepted(sequence)
        });
    }

    // Блок в игрока не ставится: он окажется замурован. Блоки без ящика —
    // трава, факел — никому не мешают.
    if placing::blocks_player(x, y, z, state, player.x, player.y, player.z) {
        log_debug!("Play: блок в {} {} {} задел бы игрока — не ставим", x, y, z);
        return Some(Reaction::refused(sequence, &[(x, y, z)]));
    }

    world.set_block(x, y, z, state);

    // Соседние заборы, стенки и панели должны увидеть новый блок и срастись
    // с ним, а редстоун — узнать о нём.
    placing::refresh_neighbours(&mut world, x, y, z);
    redstone::settle(&mut world);

    // Сделанное игроком записываем сразу: течение воды может и подождать
    // до общей записи, а постройки — нет.
    world.save_if_needed();

    log_debug!("Play: поставлен блок {} ({}) в {} {} {}", name, state, x, y, z);

    Some(Reaction {
        slots: spend_placed(player),
        ..Reaction::accepted(sequence)
    })
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
        .world
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

    let (x, y, z) = clicked;

    // Зачерпнуть можно источник жидкости или рыхлый снег: из текущей воды
    // ведро не наполняется.
    if item == empty {
        let clicked_state = {
            let world = shared.world.lock().expect("мир захвачен другим потоком");

            world.get_block(x, y, z)
        };

        let filled = match fluids::fluid_at(clicked_state) {
            Some((fluids::Kind::Water, fluids::SOURCE)) => with_water,
            Some((fluids::Kind::Lava, fluids::SOURCE)) => with_lava,
            _ if fluids::is_powder_snow(clicked_state) => with_snow,
            _ => return Some(Reaction::accepted(sequence)),
        };

        {
            let mut world = shared.world.lock().expect("мир захвачен другим потоком");

            world.set_block(x, y, z, world::AIR);
            redstone::settle(&mut world);
            world.save_if_needed();
        }

        log_debug!("Play: {} набрал ведро в {} {} {}", player.name, x, y, z);

        return Some(Reaction {
            slots: swap_held(player, filled),
            ..Reaction::accepted(sequence)
        });
    }

    // Что выльется из ведра. Рыхлый снег не жидкость — он просто встаёт
    // блоком, никуда не течёт и сам по себе не тает.
    let poured = if item == with_water {
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
        let world = shared.world.lock().expect("мир захвачен другим потоком");

        world.get_block(x, y, z)
    };

    let (x, y, z) = if here == world::AIR {
        (x, y, z)
    } else {
        offset_towards(x, y, z, face)
    };

    {
        let mut world = shared.world.lock().expect("мир захвачен другим потоком");

        world.set_block(x, y, z, poured);
        redstone::settle(&mut world);
        world.save_if_needed();
    }

    log_debug!("Play: {} вылил ведро в {} {} {}", player.name, x, y, z);

    Some(Reaction {
        slots: swap_held(player, empty),
        ..Reaction::accepted(sequence)
    })
}

/// Меняет предмет в руке на другой — ведро наполняется и пустеет.
///
/// В творческом режиме ведро не меняется: запас там бесконечный.
fn swap_held(player: &mut PlayerState, item: i32) -> Vec<i32> {
    if player.game_mode == GAME_MODE_CREATIVE {
        return Vec::new();
    }

    let slot = inventory::FIRST_HOTBAR + player.inventory.selected();

    // Ведро было не одно — остальные остаются на месте, а полное кладётся
    // отдельно; не поместилось, значит пропало: класть его некуда.
    match player.inventory.held() {
        Some(held) if held.count > 1 => {
            player.inventory.spend_held();

            let mut slots = vec![slot];
            slots.extend(player.inventory.add(Stack::new(item, 1)));
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

/// Разбирает сообщение о содержимом слота инвентаря в творческом режиме.
///
/// В творческом режиме клиент сам сообщает, что он положил в слот: меню
/// предметов есть только у него. Поэтому здесь сервер просто записывает
/// сказанное.
fn read_creative_slot(payload: &[u8], player: &mut PlayerState) {
    // В выживании предметы берутся из мира, а не из меню. Клиент такого
    // сообщения там и не шлёт, но верить ему на слово нельзя: изменённый
    // клиент мог бы выдать себе что угодно.
    if player.game_mode != GAME_MODE_CREATIVE {
        log_debug!("Play: {} не в творческом режиме — слот не выдаём", player.name);
        return;
    }

    let mut cursor = Cursor::new(payload);

    let Some(slot) = cursor.u16() else {
        return;
    };

    let Some((stack, _)) = read_slot(&mut cursor) else {
        return;
    };

    player.inventory.set(slot as i32, stack);

    match stack {
        Some(stack) => log_debug!(
            "Play: в слоте {} предмет {} ({} шт.)",
            slot,
            stack.item,
            stack.count
        ),
        None => log_debug!("Play: слот {} опустел", slot),
    }
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

    player.inventory.click(slot as i16 as i32, button as i8, mode)
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

    fn f32(&mut self) -> Option<f32> {
        let bytes: [u8; 4] = self.bytes.get(self.offset..self.offset + 4)?.try_into().ok()?;

        self.offset += 4;

        Some(f32::from_be_bytes(bytes))
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
struct PlayerState {
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

    /// Номер сущности игрока. Его выдаёт список игроков при входе: тот же
    /// номер получают остальные, чтобы показать этого игрока.
    entity_id: i32,

    /// Номер этого подключения как читателя общих журналов.
    reader: journal::Reader,

    /// Приседает ли игрок сейчас. Присев, щёлкают не по рычагу и не по двери,
    /// а ставят блок рядом с ними.
    sneaking: bool,

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

    /// Что об игроке уже лежит на диске: прочитанное при заходе или записанное
    /// после. Нынешнее положение сравнивается именно с этим — записывается
    /// только то, что от него отличается.
    saved: Option<PlayerData>,

    /// Точка появления мира: её сервер сообщает клиенту, и по ней клиент
    /// рисует стрелку компаса.
    spawn_point: (f64, f64, f64),
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
                GAME_MODE_CREATIVE,
            ),
        };

        Self {
            spawn_point: spawn,
            uuid,
            name,
            game_mode,
            x,
            y,
            z,
            yaw,
            pitch,
            on_ground: false,
            entity_id: 0,
            // Инвентарь — тот, с которым игрок вышел в прошлый раз.
            reader,
            sneaking: false,
            inventory: saved
                .as_ref()
                .map(|data| data.inventory.clone())
                .unwrap_or_else(Inventory::new),
            state_id: 0,
            skin,
            skin_parts,
            main_hand: configuration::RIGHT_HAND,
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
            x: self.x,
            y: self.y,
            z: self.z,
            yaw: self.yaw,
            pitch: self.pitch,
            game_mode: self.game_mode,
            inventory: self.inventory.clone(),
        };

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

#[cfg(test)]
mod tests {
    use super::*;

    /// Точка появления в проверках: в них мира нет, поэтому берём любую.
    const SPAWN: (f64, f64, f64) = (0.5, 64.0, 0.5);

    /// Игрок, о котором на диске уже лежит ровно то, что он делает сейчас:
    /// так сервер видит игрока сразу после захода.
    fn settled_player() -> PlayerState {
        settled_player_with_yaw(SPAWN_YAW)
    }

    fn settled_player_with_yaw(yaw: f32) -> PlayerState {
        let saved = PlayerData {
            inventory: Inventory::new(),
            x: SPAWN.0,
            y: SPAWN.1,
            z: SPAWN.2,
            yaw,
            pitch: SPAWN_PITCH,
            game_mode: GAME_MODE_CREATIVE,
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
            inventory: Inventory::new(),
            x: 100.5,
            y: 70.0,
            z: -20.5,
            yaw: 90.0,
            pitch: -10.0,
            game_mode: 0,
        };

        let player = PlayerState::new([2; 16], "Игрок".to_string(), Some(saved), None, 0x7F, 1, SPAWN);

        assert_eq!((player.x, player.y, player.z), (100.5, 70.0, -20.5));
        assert_eq!((player.yaw, player.pitch), (90.0, -10.0));
        assert_eq!(player.game_mode, 0);
        assert!(!player.needs_save());
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

            suggestions(&payload, &shared).expect("подсказки собрались")
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
    }

    /// Подсказки зависят от команды: где ждут игрока — имена, где предмет —
    /// названия предметов.
    #[test]
    fn hints_depend_on_the_command() {
        let shared = crate::shared::Shared::new(
            crate::world::World::in_memory(),
            std::path::PathBuf::new(),
            std::path::PathBuf::new(),
            Default::default(),
            crate::ops::Ops::open(std::path::Path::new("/tmp/mcsheriffanya-hints-ops.json")),
            crate::skins::Settings::default(),
        );

        // Предметы подсказываются по началу названия.
        let items = hints_for("/give Игрок sto", "sto", &shared);
        assert!(items.iter().any(|name| name == "stone"), "камня нет в подсказках");
        assert!(items.iter().all(|name| name.starts_with("sto")));

        // Без начала слова список предметов не вываливается.
        assert!(hints_for("/give Игрок ", "", &shared).is_empty());

        // У времени свои слова.
        let time = hints_for("/time s", "s", &shared);
        assert_eq!(time, vec!["set".to_string()]);
    }

}
