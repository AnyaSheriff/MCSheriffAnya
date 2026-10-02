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

/// Перенесённый игрок ещё не подтвердил перенос (`Member::unsettled`).
pub const AWAITING_CONFIRM: u8 = u8::MAX;

/// Через сколько тактов после подтверждения переноса игрок «встаёт» на блок
/// под ногами. Чёрный ящик, sculk-shrieker: робот перенесён командой на 0,1
/// над крикуном (и висит там — летает, в пакетах «не на земле»); оригинал
/// вскрикивает через 2 такта после команды при любых пакетах робота: в такте
/// команды игрок перенесён, подтверждение приходит к следующему, и ещё через
/// такт он стоит. Признак «на земле» от клиента оригинал для этого не
/// спрашивает, и число пакетов о перемещении тоже не важно (второй пакет
/// после переноса робот шлёт через полсекунды, а крик — через 2 такта).
/// [проверить] на настоящем клиенте.
pub const SETTLE_TICKS: u8 = 2;

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

    /// Ведущая рука: 0 — левая, 1 — правая. Её настройка в клиенте, и
    /// остальные видят левшу левшой, только если сервер им это перескажет.
    pub main_hand: u8,

    /// Что игрок держит в руке. Остальные видят предмет в чужих руках
    /// только отсюда: в своём инвентаре чужие предметы им не показывают.
    pub held: Option<Stack>,

    /// В каком измерении игрок: другим он виден только в том же.
    pub dimension: crate::dimension::Dimension,

    /// Приседает ли: остальные видят его присевшим, только если сервер
    /// им это перескажет.
    pub sneaking: bool,

    /// Сколько раз взмахнул рукой (щелчок, удар): подключения сравнивают
    /// с показанным и досылают своим клиентам взмах.
    pub swings: u32,

    /// Насколько игрока сдвинули едущие блоки поршней и сколько раз, с тех
    /// пор как подключение это забрало (`take_shoved`): место игрока знает
    /// его подключение, такт лишь сообщает сдвиг.
    pub shoved: ((f64, f64, f64), u32),

    /// Только что перенесён: `AWAITING_CONFIRM` — перенос ещё не подтверждён,
    /// иначе — сколько тактов ждать, прежде чем игрок «встанет» на блок под
    /// ногами (крикун, руда). 0 — стоит.
    pub unsettled: u8,
}

/// Что о бое игрока знают остальные подключения: сколько ещё неуязвим,
/// каким был сильнейший удар за это время и жив ли. Нападающему это нужно,
/// чтобы понять, пройдёт ли его удар (иначе — «пустой» звук).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CombatView {
    pub invulnerable: i32,
    pub last_hurt: f32,
    pub dead: bool,
    /// Броня и твёрдость, очки защиты зачарований (общие и только от снарядов).
    pub armor: f32,
    pub toughness: f32,
    pub protection: i32,
    pub projectile_protection: i32,
    /// Уровень сопротивления (0 — нет).
    pub resistance: i32,
    /// Уровни «шипов» на ботинках…шлеме по `ARMOR_SLOTS`.
    pub thorns: [i32; 4],
    /// С какого такта поднят щит (None — не поднят) и до какого он отключён.
    pub blocking_since: Option<u64>,
    pub shield_disabled_until: u64,
}

/// Бит позы вращения «тягуна» в `Gear::living`: самих флагов живой сущности
/// три (1, 2, 4), поза едет рядом, чтобы уйти с ними одним пакетом.
pub const SPIN_POSE_BIT: u8 = 0x80;

/// Снаряжение и бой игрока, видимые остальным: вторая рука и броня (шлем,
/// нагрудник, штаны, ботинки) — их клиенты показывают на чужом игроке.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Gear {
    pub worn: [Option<Stack>; 5],
    pub view: CombatView,
    /// Скорость удара: её меняет оружие в руке, и остальным она уходит атрибутом.
    pub attack_speed: Option<crate::attributes::Attribute>,
    pub speed_touch: u32,
    /// Флаги живой сущности (использует предмет, рука): остальным — свойством 8.
    pub living: u8,
    /// Частицы эффектов (ARGB, нули — нет): остальным — свойством 10.
    pub effect_colors: [i32; crate::survival::EFFECT_PARTICLES],
    /// Здоровье: остальным — свойством 9.
    pub health: f32,
    /// Бежит ли и его скорость шага: остальным они уходят флагом и атрибутом.
    pub sprinting: bool,
    pub movement_speed: Option<crate::attributes::Attribute>,
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

    /// Игроку сменили режим: остальным надо поправить строку в списке
    /// игроков — наблюдателя там видно по режиму.
    GameMode { uuid: [u8; 16], mode: i32 },

    /// У игрока появился скин (у игрока Bedrock — когда его картинка
    /// загружена в Mojang): остальным надо заново показать его с новым.
    Skin(Member),
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

    /// Повернуть взгляд: поворот по сторонам и наклон в градусах, как в
    /// `tp x y z поворот наклон`. Идёт следом за переносом.
    Look { yaw: f32, pitch: f32 },

    /// Выдать предметы: они лягут в инвентарь, а что не поместится —
    /// упадёт на землю.
    Give { stack: crate::inventory::Stack },

    /// Положить в основную руку (выбранную ячейку панели) это — или
    /// опустошить её: `item replace entity <игрок> weapon.mainhand`.
    Hold { stack: Option<crate::inventory::Stack> },

    /// Очистить инвентарь.
    Clear,

    /// Положить в ячейку инвентаря (номер как в окне игрока: панель —
    /// 36–44) ровно это (`item replace entity`).
    SetSlot { slot: i32, stack: Option<crate::inventory::Stack> },

    /// Сменить режим игры: подключение само скажет об этом своему клиенту.
    GameMode { mode: i32 },

    /// Нанести урон (`kill`, `damage`): приходит как обычный удар своего вида.
    Hurt { amount: f32, source: crate::survival::Source },
    /// Команда `experience`: добавить или задать очки либо уровни.
    Experience { set: bool, amount: i32, levels: bool },
    /// Игрок подобрал шарики опыта на такте мира: очки прибавляются, клиент
    /// узнаёт после пакета подбора.
    XpTaken { points: i32 },

    /// Возродить умершего игрока (клиент нажал «возродиться»).
    Respawn,

    /// Наложить эффект на столько тактов (`effect give`).
    Effect(crate::survival::Effect),

    /// Снять все эффекты (`effect clear`).
    ClearEffects,
    /// Снять один эффект.
    ClearEffect(crate::survival::EffectKind),

    /// Перейти в другое измерение и встать в это место.
    ChangeDimension {
        dimension: crate::dimension::Dimension,
        x: f64,
        y: f64,
        z: f64,
    },
}

/// Список игроков на сервере и журнал изменений.
pub struct Players {
    members: Vec<Member>,
    changes: Journal<Change>,

    /// Поручения подключениям: кого выгнать, кого перенести.
    orders: HashMap<[u8; 16], Vec<Order>>,

    /// Броня, вторая рука и бой игроков (`Gear`).
    gear: HashMap<[u8; 16], Gear>,

    /// Такт последнего показанного взмаха каждого игрока.
    swung: HashMap<[u8; 16], u64>,

    /// Будильник подключения: поручение (перенос командой) будит его сразу, а
    /// не на следующем пакете клиента или конце такта — оригинал шлёт пакет
    /// переноса в миг команды (чёрный ящик, pvp-mace: у нас он приходил то на
    /// 0, то на 25 мс позже, и падающий робот успевал на тик меньше).
    wakes: HashMap<[u8; 16], std::sync::Arc<tokio::sync::Notify>>,
}

impl Players {
    pub fn new() -> Self {
        Self {
            members: Vec::new(),
            changes: Journal::new(),
            orders: HashMap::new(),
            gear: HashMap::new(),
            swung: HashMap::new(),
            wakes: HashMap::new(),
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
        // И по опознавателю: у Bedrock он не из ника, и двое с одним
        // опознавателем писали бы в один файл игрока.
        if self.name_taken(&member.name) || self.members.iter().any(|other| other.uuid == member.uuid) {
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
        self.gear.remove(uuid);
        self.swung.remove(uuid);
        self.wakes.remove(uuid);

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
        if let Some(member) = self.members.iter_mut().find(|member| &member.uuid == uuid)
            && member.game_mode != game_mode
        {
            member.game_mode = game_mode;
            self.changes.push(Change::GameMode {
                uuid: *uuid,
                mode: game_mode,
            });
        }
    }

    /// Запоминает скин игрока и сообщает о нём остальным.
    pub fn set_skin(&mut self, uuid: &[u8; 16], skin: Skin) {
        if let Some(member) = self.members.iter_mut().find(|member| &member.uuid == uuid)
            && member.skin.as_ref() != Some(&skin)
        {
            member.skin = Some(skin);
            self.changes.push(Change::Skin(member.clone()));
        }
    }

    /// Запоминает, в каком измерении игрок: остальные по этому решают,
    /// показывать ли его.
    pub fn set_dimension(&mut self, uuid: &[u8; 16], dimension: crate::dimension::Dimension) {
        if let Some(member) = self.members.iter_mut().find(|member| &member.uuid == uuid) {
            member.dimension = dimension;
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

    /// Игрока перенесли: на блок под ногами он встанет не сразу (`SETTLE_TICKS`).
    pub fn unsettle(&mut self, uuid: &[u8; 16]) {
        if let Some(member) = self.members.iter_mut().find(|member| &member.uuid == uuid) {
            member.unsettled = AWAITING_CONFIRM;
        }
    }

    /// Клиент подтвердил перенос: отсчёт тактов до того, как игрок встанет.
    pub fn confirm_teleport(&mut self, uuid: &[u8; 16]) {
        if let Some(member) = self.members.iter_mut().find(|member| &member.uuid == uuid && member.unsettled == AWAITING_CONFIRM) {
            member.unsettled = SETTLE_TICKS;
        }
    }

    /// Такт: подтвердившие перенос на такт ближе к тому, чтобы встать.
    pub fn settle(&mut self) {
        for member in &mut self.members {
            if member.unsettled != AWAITING_CONFIRM {
                member.unsettled = member.unsettled.saturating_sub(1);
            }
        }
    }

    /// Едущий блок поршня сдвинул игрока (tick.rs): место в списке — сразу,
    /// чтобы следующий едущий блок в этом же такте видел его уже сдвинутым.
    pub fn shove(&mut self, entity_id: i32, (dx, dy, dz): (f64, f64, f64)) {
        if let Some(member) = self.members.iter_mut().find(|member| member.entity_id == entity_id) {
            member.position.x += dx;
            member.position.y += dy;
            member.position.z += dz;
            let ((x, y, z), count) = member.shoved;
            member.shoved = ((x + dx, y + dy, z + dz), count + 1);
        }
    }

    /// Забирает накопленный сдвиг поршнями: None — никто не толкал.
    pub fn take_shoved(&mut self, uuid: &[u8; 16]) -> Option<((f64, f64, f64), u32)> {
        let member = self.members.iter_mut().find(|member| &member.uuid == uuid)?;

        if member.shoved.1 == 0 {
            return None;
        }

        Some(std::mem::replace(&mut member.shoved, ((0.0, 0.0, 0.0), 0)))
    }

    /// Запоминает, что у игрока в руке.
    ///
    /// Как и положение, это меняется часто и в журнал не пишется: остальные
    /// смотрят в список и замечают перемену сами.
    /// Игрок взмахнул рукой.
    pub fn swing(&mut self, uuid: &[u8; 16]) {
        if let Some(member) = self.members.iter_mut().find(|member| &member.uuid == uuid) {
            member.swings = member.swings.wrapping_add(1);
        }
    }

    /// Взмах на такте `now`: пока прошлый взмах не прошёл четыре такта, новый
    /// остальным не показывается (у оригинала взмах, идущий меньше половины
    /// срока, не рассылается заново; чёрный ящик, pvp-sword-spam: удары через
    /// 3 такта — взмахи на 0, 6, 12).
    pub fn swing_at(&mut self, uuid: &[u8; 16], now: u64) {
        if self.swung.get(uuid).is_some_and(|before| now.saturating_sub(*before) < 4) {
            return;
        }

        self.swung.insert(*uuid, now);
        self.swing(uuid);
    }

    /// Запоминает, приседает ли игрок.
    pub fn set_sneaking(&mut self, uuid: &[u8; 16], sneaking: bool) {
        if let Some(member) = self.members.iter_mut().find(|member| &member.uuid == uuid) {
            member.sneaking = sneaking;
        }
    }

    /// Запоминает броню, вторую руку и бой игрока.
    pub fn set_gear(&mut self, uuid: &[u8; 16], gear: Gear) {
        if self.members.iter().any(|member| &member.uuid == uuid) {
            self.gear.insert(*uuid, gear);
        }
    }

    /// Здоровье игрока сразу после удара: остальные увидят его в этом же такте.
    pub fn set_health(&mut self, uuid: &[u8; 16], health: f32) {
        if let Some(gear) = self.gear.get_mut(uuid) {
            gear.health = health;
        }
    }

    /// Броня и бой игрока (у неизвестного — пусто).
    pub fn gear(&self, uuid: &[u8; 16]) -> Gear {
        self.gear.get(uuid).cloned().unwrap_or_else(|| Gear { health: crate::survival::MAX_HEALTH, ..Gear::default() })
    }

    /// Игрок по номеру его сущности: сам и его броня с боем.
    pub fn by_entity(&self, entity_id: i32) -> Option<(Member, Gear)> {
        let member = self.members.iter().find(|member| member.entity_id == entity_id)?;

        Some((member.clone(), self.gear(&member.uuid)))
    }

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

    /// Игрок сменил ведущую руку в настройках — остальные должны это увидеть.
    pub fn set_main_hand(&mut self, uuid: &[u8; 16], main_hand: u8) {
        if let Some(member) = self.members.iter_mut().find(|member| &member.uuid == uuid) {
            member.main_hand = main_hand;
        }
    }

    /// Поручает подключению игрока что-то сделать.
    ///
    /// Тому, кого нет на сервере, поручение не кладётся: его некому забрать,
    /// и оно лежало бы вечно.
    pub fn order(&mut self, uuid: &[u8; 16], order: Order) {
        if !self.members.iter().any(|member| &member.uuid == uuid) {
            return;
        }

        // Будить сразу — только перенос в своём измерении: смену измерения
        // (порталы) оригинал делает на ходу игрока в такте, пакеты идут в конце
        // такта (чёрный ящик, end-portal-enter: Respawn на +1).
        let wake = matches!(order, Order::Teleport { .. });

        self.orders.entry(*uuid).or_default().push(order);

        if wake && let Some(wake) = self.wakes.get(uuid) {
            wake.notify_one();
        }
    }

    /// Будильник подключения этого игрока (см. `wakes`).
    pub fn waker(&mut self, uuid: &[u8; 16]) -> std::sync::Arc<tokio::sync::Notify> {
        self.wakes.entry(*uuid).or_default().clone()
    }

    /// Куда игрока перенесёт последнее ждущее поручение, если перенос в
    /// своём измерении и после него нет перехода в другое.
    pub fn pending_teleport(&self, uuid: &[u8; 16]) -> Option<(f64, f64, f64)> {
        let orders = self.orders.get(uuid)?;

        match orders.iter().rev().find(|order| matches!(order, Order::Teleport { .. } | Order::ChangeDimension { .. }))? {
            Order::Teleport { x, y, z } => Some((*x, *y, *z)),
            _ => None,
        }
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
            main_hand: 1,
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
            dimension: Default::default(),
            sneaking: false,
            swings: 0,
            shoved: ((0.0, 0.0, 0.0), 0),
            unsettled: 0,
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

    /// Толчки поршнем копятся, пока подключение их не заберёт: место в
    /// списке сдвигается сразу, забранный сдвиг обнуляется.
    #[test]
    fn shoves_add_up_until_taken() {
        let mut players = Players::new();
        let uuid = [1; 16];

        joined(&mut players, "Первый", 1);
        assert_eq!(players.take_shoved(&uuid), None);

        players.shove(1, (0.31, 0.0, 0.0));
        players.shove(1, (0.5, 0.0, 0.0));

        assert!((players.members()[0].position.x - 0.81).abs() < 1e-9);

        let ((dx, dy, dz), count) = players.take_shoved(&uuid).expect("толкали");
        assert!((dx - 0.81).abs() < 1e-9);
        assert_eq!((dy, dz, count), (0.0, 0.0, 2));
        assert_eq!(players.take_shoved(&uuid), None);
    }

    /// Ждущий перенос виден до того, как подключение заберёт поручение:
    /// последний, и только если после него нет перехода в другое измерение.
    #[test]
    /// Перенесённый игрок встаёт на блок под ногами только через
    /// `SETTLE_TICKS` тактов после подтверждения переноса.
    fn a_teleported_player_settles_after_the_confirmation() {
        let mut players = Players::new();
        let uuid = [3; 16];
        joined(&mut players, "Третий", 3);

        players.unsettle(&uuid);
        players.settle();
        players.settle();
        assert_eq!(players.members()[0].unsettled, AWAITING_CONFIRM, "без подтверждения стоять нельзя");

        players.confirm_teleport(&uuid);
        players.settle();
        assert_eq!(players.members()[0].unsettled, 1);
        players.settle();
        assert_eq!(players.members()[0].unsettled, 0);

        // Подтверждение обычной сверки места ничего не меняет.
        players.confirm_teleport(&uuid);
        assert_eq!(players.members()[0].unsettled, 0);
    }

    #[test]
    fn pending_teleport_is_the_last_one_in_this_dimension() {
        let mut players = Players::new();
        let uuid = [1; 16];

        joined(&mut players, "Первый", 1);
        assert_eq!(players.pending_teleport(&uuid), None);

        players.order(&uuid, Order::Teleport { x: 1.0, y: 2.0, z: 3.0 });
        players.order(&uuid, Order::Look { yaw: 0.0, pitch: 0.0 });
        players.order(&uuid, Order::Teleport { x: 4.0, y: 5.0, z: 6.0 });
        assert_eq!(players.pending_teleport(&uuid), Some((4.0, 5.0, 6.0)));

        players.order(&uuid, Order::ChangeDimension { dimension: Default::default(), x: 0.0, y: 0.0, z: 0.0 });
        assert_eq!(players.pending_teleport(&uuid), None);

        assert_eq!(players.take_orders(&uuid).len(), 4);
        assert_eq!(players.pending_teleport(&uuid), None);
    }

    /// Перенос командой будит подключение сразу (оригинал шлёт пакет места в
    /// миг команды, чёрный ящик pvp-mace); смена измерения — нет: она идёт в
    /// конце такта (end-portal-enter).
    #[test]
    fn a_teleport_order_wakes_the_connection_but_a_dimension_change_does_not() {
        let mut players = Players::new();
        let uuid = [1; 16];

        joined(&mut players, "Первый", 1);

        let wake = players.waker(&uuid);
        let runtime = tokio::runtime::Builder::new_current_thread().enable_time().build().expect("часы для проверки");
        let woken = |wake: &std::sync::Arc<tokio::sync::Notify>| {
            runtime.block_on(async { tokio::time::timeout(std::time::Duration::from_millis(30), wake.notified()).await.is_ok() })
        };

        players.order(&uuid, Order::ChangeDimension { dimension: Default::default(), x: 0.0, y: 0.0, z: 0.0 });
        assert!(!woken(&wake));

        players.order(&uuid, Order::Teleport { x: 1.0, y: 2.0, z: 3.0 });
        assert!(woken(&wake));
        assert!(!woken(&wake), "один сигнал — один разбор");
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
