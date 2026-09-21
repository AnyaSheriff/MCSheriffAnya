// Кто сейчас на сервере.
//
// Список общий для всех подключений: игрок заходит — о нём должны узнать
// остальные, выходит — перестать его показывать. Подключения живут в разных
// задачах и своими потоками писать друг другу не могут, поэтому устроено так
// же, как рассылка изменений мира: сервер ведёт журнал «кто зашёл, кто вышел»,
// а каждое подключение помнит, сколько записей оно уже разослало своему
// клиенту, и досылает остальные.
//
// Список нужен ещё и для ответа на запрос в списке серверов: там показывается,
// сколько игроков онлайн.

use std::collections::HashMap;

use crate::journal::{Journal, Reader};
use crate::inventory::Stack;
use crate::skins::Skin;

/// Где игрок стоит и куда смотрит.
///
/// Это то, что о нём знают остальные: сервер сам физику не считает, поэтому
/// единственный источник сведений — пакеты о перемещении, которые присылает
/// клиент игрока.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Position {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,

    /// Стоит ли игрок на земле. Это видно и остальным: клиент рисует
    /// оторвавшегося от земли игрока иначе, чем стоящего.
    pub on_ground: bool,
}

/// Один игрок на сервере.
#[derive(Clone)]
pub struct Member {
    pub uuid: [u8; 16],
    pub name: String,
    pub game_mode: i32,

    /// Номер сущности игрока в мире. Клиенты отличают по нему одного игрока
    /// от другого, поэтому он должен быть одинаковым у всех, кто этого игрока
    /// видит, и не повторяться у тех, кто на сервере одновременно.
    pub entity_id: i32,

    /// Где игрок стоит и куда смотрит.
    pub position: Position,

    /// Скин игрока. None — скина нет, клиент покажет стандартный.
    pub skin: Option<Skin>,

    /// Какие части скина игрок показывает: плащ, куртка, рукава, штанины,
    /// шапка. Без этого остальные видят только первый слой скина.
    pub skin_parts: u8,

    /// Что игрок держит в руке. Остальные видят предмет в чужих руках
    /// только отсюда: в своём инвентаре чужие предметы им не показывают.
    pub held: Option<Stack>,
}

/// Что произошло с игроками — запись журнала.
#[derive(Clone)]
pub enum Change {
    /// Игрок зашёл.
    Joined(Member),

    /// Игрок вышел.
    ///
    /// Кроме опознавателя запоминается и номер его сущности: остальным нужно
    /// убрать этого игрока из мира, а ищут они его там именно по номеру.
    Left {
        uuid: [u8; 16],
        entity_id: i32,
    },
}

/// Поручение подключению игрока — то, что может сделать только оно само.
///
/// Команды выполняются не в том подключении, которого касаются: выгнать или
/// перенести игрока может и консоль, и другой игрок. Поэтому поручение
/// кладётся сюда, а подключение забирает его на ближайшем круге.
#[derive(Clone, Debug)]
pub enum Order {
    /// Выгнать с сервера, показав причину.
    Kick { reason: String },

    /// Перенести в другое место.
    Teleport { x: f64, y: f64, z: f64 },

    /// Выдать предметы: они лягут в инвентарь, а что не поместится —
    /// упадёт на землю.
    Give { item: i32, count: i32 },

    /// Очистить инвентарь.
    Clear,
}

/// Список игроков на сервере и журнал изменений.
pub struct Players {
    members: Vec<Member>,
    changes: Journal<Change>,

    /// Поручения подключениям: кого выгнать, кого перенести.
    orders: HashMap<[u8; 16], Vec<Order>>,

}

impl Players {
    pub fn new() -> Self {
        Self {
            members: Vec::new(),
            changes: Journal::new(),
            orders: HashMap::new(),
        }
    }

    /// Добавляет игрока и возвращает его самого (с выданным номером сущности)
    /// и тех, кто был на сервере до него. Номер сущности выдаётся снаружи:
    /// он общий для всех сущностей мира, а не только для игроков, — иначе
    /// игрок и лежащий предмет могли бы получить один и тот же номер.
    ///
    /// Возвращённый список нужен новичку: остальные о нём узнают из журнала,
    /// а он о них — только отсюда, потому что журнал он ещё не читал.
    ///
    /// None означает, что игрок с таким ником уже на сервере. Двоим с одним
    /// ником здесь делать нечего: их не отличить ни в чате, ни в списке, ни
    /// в файлах, где хранятся их положение и вещи, — они бы затирали друг
    /// друга. Решается это здесь, под общим замком, потому что только здесь
    /// проверка и появление в списке происходят разом: между проверкой при
    /// входе и входом в мир идёт обмен с клиентом, и за это время ник мог
    /// занять кто-то другой.
    pub fn join(&mut self, mut member: Member, entity_id: i32) -> Option<(Member, Vec<Member>)> {
        if self.name_taken(&member.name) {
            return None;
        }

        let others = self.members.clone();

        member.entity_id = entity_id;

        self.members.push(member.clone());
        self.changes.push(Change::Joined(member.clone()));

        Some((member, others))
    }

    /// Занят ли этот ник кем-то из тех, кто сейчас на сервере.
    pub fn name_taken(&self, name: &str) -> bool {
        self.members
            .iter()
            .any(|member| member.name.eq_ignore_ascii_case(name))
    }

    /// Убирает игрока из списка. Возвращает false, если его там не было.
    ///
    /// Убирать приходится по опознавателю: имя игрок мог и не сообщить, если
    /// отключился в неудачный момент.
    pub fn leave(&mut self, uuid: &[u8; 16]) -> bool {
        let Some(entity_id) = self
            .members
            .iter()
            .find(|member| &member.uuid == uuid)
            .map(|member| member.entity_id)
        else {
            return false;
        };

        self.members.retain(|member| &member.uuid != uuid);
        self.orders.remove(uuid);

        self.changes.push(Change::Left {
            uuid: *uuid,
            entity_id,
        });
        true
    }

    /// Сколько игроков сейчас на сервере.
    pub fn online(&self) -> usize {
        self.members.len()
    }

    /// Имена игроков по порядку входа — для ответа на команду «кто на сервере».
    pub fn names(&self) -> Vec<String> {
        self.members
            .iter()
            .map(|member| member.name.clone())
            .collect()
    }

    /// Находит игрока по имени — для команд, где игрока называют словами.
    ///
    /// Регистр не важен: в команде имя набирают руками, и требовать точного
    /// совпадения было бы придиркой. Сравнение при этом только по латинице —
    /// и этого достаточно: имя игрока в игре может состоять лишь из латинских
    /// букв, цифр и подчёркивания.
    pub fn find(&self, name: &str) -> Option<Member> {
        self.members
            .iter()
            .find(|member| member.name.eq_ignore_ascii_case(name))
            .cloned()
    }

    /// Запоминает новый режим игры игрока.
    ///
    /// В журнал это не пишется: режим нигде, кроме списка игроков, не виден,
    /// а самому игроку о смене сообщает не журнал, а ответ на его команду.
    /// Запомнить нужно для тех, кто зайдёт позже: новичок получает сведения
    /// об остальных из списка, и режим должен быть там уже новый.
    pub fn set_game_mode(&mut self, uuid: &[u8; 16], game_mode: i32) {
        if let Some(member) = self.members.iter_mut().find(|member| &member.uuid == uuid) {
            member.game_mode = game_mode;
        }
    }

    /// Запоминает, где игрок стоит и куда смотрит.
    ///
    /// Это нужно остальным: по этим сведениям их клиенты показывают сущность
    /// игрока там, где он на самом деле. В журнал положение не пишется —
    /// оно меняется по двадцать раз в секунду, и журнал бы рос без предела.
    /// Кто хочет его узнать, тот просто смотрит в список.
    pub fn set_position(&mut self, uuid: &[u8; 16], position: Position) {
        if let Some(member) = self.members.iter_mut().find(|member| &member.uuid == uuid) {
            member.position = position;
        }
    }

    /// Запоминает, что у игрока в руке.
    ///
    /// Как и положение, это меняется часто и в журнал не пишется: остальные
    /// смотрят в список и замечают перемену сами.
    pub fn set_held(&mut self, uuid: &[u8; 16], held: Option<Stack>) {
        if let Some(member) = self.members.iter_mut().find(|member| &member.uuid == uuid) {
            member.held = held;
        }
    }

    /// Запоминает новый набор показываемых частей скина.
    ///
    /// Игрок может поменять его прямо в игре, в настройках, — и остальные
    /// должны это увидеть.
    pub fn set_skin_parts(&mut self, uuid: &[u8; 16], skin_parts: u8) {
        if let Some(member) = self.members.iter_mut().find(|member| &member.uuid == uuid) {
            member.skin_parts = skin_parts;
        }
    }

    /// Поручает подключению игрока что-то сделать.
    pub fn order(&mut self, uuid: &[u8; 16], order: Order) {
        self.orders.entry(*uuid).or_default().push(order);
    }

    /// Забирает поручения для этого игрока.
    pub fn take_orders(&mut self, uuid: &[u8; 16]) -> Vec<Order> {
        self.orders.remove(uuid).unwrap_or_default()
    }

    /// Все, кто сейчас на сервере.
    pub fn members(&self) -> &[Member] {
        &self.members
    }

    /// Изменения, начиная с записи `from` — то, что осталось разослать.
    ///
    /// Возвращает копию, а не срез: рассылка — дело сетевое, и держать список
    /// захваченным, пока идёт отправка, нельзя.
    pub fn changes_since(&mut self, from: usize, reader: Reader) -> (Vec<Change>, usize) {
        (self.changes.since(from, reader), self.changes.count())
    }

    /// Записывает читателя журнала.
    pub fn watch_changes(&mut self, reader: Reader) -> usize {
        self.changes.watch(reader)
    }

    /// Отписывает читателя: подключение закрылось.
    pub fn forget_reader(&mut self, reader: Reader) {
        self.changes.forget(reader);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Читатель журнала в проверках: настоящий номер выдаёт сервер.
    const READER: crate::journal::Reader = 1;

    fn member(name: &str, uuid_byte: u8) -> Member {
        Member {
            skin: None,
            skin_parts: 0x7F,
            held: None,
            uuid: [uuid_byte; 16],
            name: name.to_string(),
            game_mode: 1,
            entity_id: 0,
            position: Position {
                x: 0.0,
                y: 0.0,
                z: 0.0,
                yaw: 0.0,
                pitch: 0.0,
                on_ground: false,
            },
        }
    }

    /// Пускает игрока в список и разворачивает ответ: в тестах ник ещё
    /// ни за кем не занят, поэтому отказ означал бы ошибку в самом тесте.
    fn joined(players: &mut Players, name: &str, uuid_byte: u8) -> (Member, Vec<Member>) {
        // Номер сущности в жизни выдаёт общий счётчик сервера; в проверках
        // достаточно, чтобы он был свой у каждого.
        let entity_id = players.online() as i32 + 1;

        players
            .join(member(name, uuid_byte), entity_id)
            .expect("ник должен быть свободен")
    }

    /// Новичок должен получить список тех, кто уже зашёл, — иначе он их не
    /// увидит: журнал он ещё не читал.
    #[test]
    fn newcomer_sees_existing_players() {
        let mut players = Players::new();

        assert!(joined(&mut players, "Первый", 1).1.is_empty());

        let (_, others) = joined(&mut players, "Второй", 2);
        assert_eq!(others.len(), 1);
        assert_eq!(others[0].name, "Первый");
        assert_eq!(players.online(), 2);
    }

    /// Номер сущности выдаёт сервер, и у двух игроков он не совпадает: иначе
    /// клиенты приняли бы одного за другого.
    #[test]
    fn every_player_gets_his_own_entity_number() {
        let mut players = Players::new();

        let (first, _) = joined(&mut players, "Первый", 1);
        let (second, _) = joined(&mut players, "Второй", 2);

        assert_ne!(first.entity_id, second.entity_id);
        assert_eq!(players.find("Первый").map(|m| m.entity_id), Some(first.entity_id));
    }

    /// Положение игрока видят остальные: из списка они берут, где его показать.
    #[test]
    fn a_position_is_kept_for_the_others() {
        let mut players = Players::new();

        joined(&mut players, "Первый", 1);

        let position = Position {
            x: 12.5,
            y: 65.0,
            z: -3.0,
            yaw: 90.0,
            pitch: 10.0,
            on_ground: true,
        };
        players.set_position(&[1; 16], position);

        assert_eq!(players.find("Первый").map(|m| m.position), Some(position));

        // Про того, кого на сервере нет, запоминать нечего.
        players.set_position(&[9; 16], position);
    }

    /// Выход попадает в журнал, а сам игрок пропадает из списка.
    ///
    /// В записи о выходе остаётся и номер сущности: по нему остальные убирают
    /// вышедшего из мира.
    #[test]
    fn leaving_is_recorded() {
        let mut players = Players::new();

        players.watch_changes(READER);

        let (first, _) = joined(&mut players, "Первый", 1);
        joined(&mut players, "Второй", 2);

        assert!(players.leave(&[1; 16]));
        assert_eq!(players.online(), 1);

        // Повторный выход того же игрока ничего не добавляет в журнал.
        assert!(!players.leave(&[1; 16]));

        let (changes, _) = players.changes_since(0, READER);
        assert_eq!(changes.len(), 3);

        assert!(matches!(
            changes[2],
            Change::Left { uuid, entity_id } if uuid == [1; 16] && entity_id == first.entity_id
        ));
    }

    /// Подключение досылает только то, чего ещё не рассылало.
    #[test]
    fn changes_are_read_from_the_remembered_position() {
        let mut players = Players::new();

        // Журнал держит записи только для записанных читателей: неслышащему
        // рассказывать нечего.
        players.watch_changes(READER);

        joined(&mut players, "Первый", 1);

        let (changes, read) = players.changes_since(0, READER);
        assert_eq!(changes.len(), 1);

        // Всё уже разослано — досылать нечего.
        assert!(players.changes_since(read, READER).0.is_empty());

        joined(&mut players, "Второй", 2);
        assert_eq!(players.changes_since(read, READER).0.len(), 1);
    }

    /// Игрока находят по имени, набранному руками: регистр не важен, а чужое
    /// имя не подходит.
    #[test]
    fn a_player_is_found_by_name_regardless_of_case() {
        let mut players = Players::new();

        joined(&mut players, "Steve", 1);

        assert_eq!(players.find("steve").map(|found| found.uuid), Some([1; 16]));
        assert_eq!(
            players.find("STEVE").map(|found| found.name),
            Some("Steve".to_string())
        );
        assert!(players.find("Alex").is_none());
    }

    /// Двоих с одним ником на сервере быть не должно: их не отличить ни
    /// в чате, ни в списке, ни в файлах, где хранятся их вещи и положение.
    #[test]
    fn a_second_player_with_the_same_name_is_not_let_in() {
        let mut players = Players::new();

        joined(&mut players, "Steve", 1);

        assert!(players.join(member("Steve", 2), 2).is_none());
        assert_eq!(players.online(), 1);

        // Регистр не важен: steve и Steve — один и тот же ник.
        assert!(players.join(member("steve", 3), 3).is_none());
        assert!(players.name_taken("STEVE"));

        // Отказанному номер сущности не выдаётся: он в мир не попал.
        let (next, _) = joined(&mut players, "Alex", 4);
        assert_eq!(next.entity_id, 2);
    }

    /// Ник освобождается, как только его хозяин вышел.
    #[test]
    fn a_name_is_free_again_after_leaving() {
        let mut players = Players::new();

        joined(&mut players, "Steve", 1);
        assert!(players.name_taken("Steve"));

        assert!(players.leave(&[1; 16]));
        assert!(!players.name_taken("Steve"));

        let (returned, _) = joined(&mut players, "Steve", 5);
        assert_eq!(returned.name, "Steve");
    }
}
