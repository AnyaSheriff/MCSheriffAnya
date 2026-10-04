// Табло, команды игроков, метки сущностей и полосы босса из команд.
//
// Всё это — общее состояние сервера, как у оригинала (вики, Scoreboard):
// цели (objectives) со счётом держателей, места показа (боковая панель,
// список игроков, под именем), команды (teams) с цветом, приставками и
// правилами, метки (`tag`) и полосы босса команды `bossbar`.
//
// Клиентам изменения уходят пакетами — Update Objectives, Display Objective,
// Update Score, Reset Score, Update Teams, Boss Event. Как и чат, состояние
// ведёт общую ленту готовых пакетов, а каждое подключение помнит, сколько их
// уже отправило: вошедший получает снимок всего, а дальше — ленту.
//
// Хранится в `world/data/scoreboard.dat` — имя как у оригинала, а содержимое
// своё (JSON): цели, счёт, места показа, команды, метки игроков и полосы.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::network::nbt;
use crate::network::varint::{encode_string, encode_varint};

/// Update Objectives (minecraft-data 26.1: scoreboard_objective).
const OBJECTIVE_PACKET: i32 = 0x6a;
/// Display Objective (scoreboard_display_objective).
const DISPLAY_PACKET: i32 = 0x62;
/// Update Score (scoreboard_score).
const SCORE_PACKET: i32 = 0x6e;
/// Reset Score (reset_score).
const RESET_PACKET: i32 = 0x4f;
/// Update Teams (teams).
const TEAMS_PACKET: i32 = 0x6d;
/// Boss Event (boss_bar).
pub const BOSS_PACKET: i32 = 0x09;

/// System Chat (system_chat).
const SYSTEM_CHAT_PACKET: i32 = 0x79;

/// Сколько последних пакетов держит лента.
const KEPT: usize = 4096;

/// Цвета по порядку оформления (ChatFormatting): номер — то, что уходит в
/// пакете команды, и место показа `sidebar.team.<цвет>` (3 + номер).
pub const COLORS: [&str; 16] = [
    "black", "dark_blue", "dark_green", "dark_aqua", "dark_red", "dark_purple", "gold", "gray",
    "dark_gray", "blue", "green", "aqua", "red", "light_purple", "yellow", "white",
];

/// Номер «reset» у оформления: команда без цвета.
pub const COLOR_RESET: i32 = 21;

/// Места показа: 0 — список игроков, 1 — боковая панель, 2 — под именем,
/// 3…18 — боковая панель для команды своего цвета.
pub const SLOTS: usize = 19;

/// Имя места показа по номеру — как его пишут в команде.
#[cfg(test)]
pub fn slot_name(slot: usize) -> String {
    match slot {
        0 => "list".to_string(),
        1 => "sidebar".to_string(),
        2 => "below_name".to_string(),
        n => format!("sidebar.team.{}", COLORS[n - 3]),
    }
}

/// Номер места показа по имени.
pub fn slot_by_name(name: &str) -> Option<usize> {
    match name {
        "list" => Some(0),
        "sidebar" => Some(1),
        "below_name" | "belowName" => Some(2),
        _ => name.strip_prefix("sidebar.team.").and_then(|color| COLORS.iter().position(|c| *c == color)).map(|n| n + 3),
    }
}

/// Вид чисел: показ чисел целиком у цели или у отдельного счёта (вики,
/// Scoreboard: «blank», «fixed», «styled»).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum NumberFormat {
    Blank,
    Styled(Value),
    Fixed(Value),
}

impl NumberFormat {
    fn write(format: &Option<NumberFormat>, out: &mut Vec<u8>) {
        match format {
            None => out.push(0),
            Some(format) => {
                out.push(1);
                match format {
                    NumberFormat::Blank => out.extend(encode_varint(0)),
                    NumberFormat::Styled(style) => {
                        out.extend(encode_varint(1));
                        out.extend(encode_component(style));
                    }
                    NumberFormat::Fixed(text) => {
                        out.extend(encode_varint(2));
                        out.extend(encode_component(text));
                    }
                }
            }
        }
    }
}

/// Цель табло.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Objective {
    pub name: String,
    pub criteria: String,
    pub display: Value,
    /// Сердечки вместо чисел (`rendertype hearts`).
    pub hearts: bool,
    pub auto_update: bool,
    pub format: Option<NumberFormat>,
}

/// Счёт держателя в цели.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Score {
    pub value: i32,
    /// Замок триггера: `trigger` работает, только пока он снят (`enable`).
    pub locked: bool,
    pub display: Option<Value>,
    pub format: Option<NumberFormat>,
}

/// Кто видит табличку с именем, кого толкают — как пишут в команде
/// (`always`, `never`, `hideForOtherTeams`, …): номер — порядок в пакете.
pub const VISIBILITIES: [&str; 4] = ["always", "never", "hideForOtherTeams", "hideForOwnTeam"];
pub const COLLISIONS: [&str; 4] = ["always", "never", "pushOtherTeams", "pushOwnTeam"];

/// Команда игроков.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Team {
    pub name: String,
    pub display: Value,
    pub color: i32,
    pub prefix: Value,
    pub suffix: Value,
    pub friendly_fire: bool,
    pub see_invisible: bool,
    pub nametag: usize,
    pub death_messages: usize,
    pub collision: usize,
    pub members: Vec<String>,
}

/// Цвета полосы босса по номеру пакета и деления (`notched_6`…).
pub const BAR_COLORS: [&str; 7] = ["pink", "blue", "red", "green", "yellow", "purple", "white"];
pub const BAR_STYLES: [&str; 5] = ["progress", "notched_6", "notched_10", "notched_12", "notched_20"];

/// Полоса босса из команды `bossbar`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BossBar {
    pub id: String,
    pub uuid: [u8; 16],
    pub name: Value,
    pub color: usize,
    pub style: usize,
    pub value: i32,
    pub max: i32,
    pub visible: bool,
    /// Кому показывать (игроки по UUID); видят только те, кто на сервере.
    pub players: Vec<[u8; 16]>,
}

impl BossBar {
    fn progress(&self) -> f32 {
        if self.max <= 0 { 0.0 } else { (self.value as f32 / self.max as f32).clamp(0.0, 1.0) }
    }

    /// Пакет «добавить» (действие 0): название, доля, цвет, деления, флаги.
    fn add_packet(&self) -> Vec<u8> {
        let mut body = encode_varint(BOSS_PACKET);
        body.extend_from_slice(&self.uuid);
        body.extend(encode_varint(0));
        body.extend(encode_component(&self.name));
        body.extend_from_slice(&self.progress().to_be_bytes());
        body.extend(encode_varint(self.color as i32));
        body.extend(encode_varint(self.style as i32));
        body.push(0);
        body
    }

    fn remove_packet(&self) -> Vec<u8> {
        let mut body = encode_varint(BOSS_PACKET);
        body.extend_from_slice(&self.uuid);
        body.extend(encode_varint(1));
        body
    }
}

/// Сохраняемое целиком.
#[derive(Default, Serialize, Deserialize)]
struct Stored {
    objectives: Vec<Objective>,
    scores: BTreeMap<String, BTreeMap<String, Score>>,
    display: Vec<Option<String>>,
    teams: Vec<Team>,
    tags: BTreeMap<String, Vec<String>>,
    bossbars: Vec<BossBar>,
}

/// Пакет ленты: кому (None — всем) и что.
type Entry = (Option<Vec<[u8; 16]>>, Vec<u8>);

/// Табло и всё, что с ним рядом.
pub struct Scoreboard {
    /// Цели по порядку появления.
    pub objectives: Vec<Objective>,
    /// Счёт: держатель → цель → счёт. Держатели — по порядку имён, как
    /// список у оригинала [проверить].
    pub scores: BTreeMap<String, BTreeMap<String, Score>>,
    /// Держатели в порядке появления — от него зависит порядок списка
    /// `players list` у оригинала (`open_hash_order`).
    pub arrival: Vec<String>,
    /// Что показано в каждом месте.
    pub display: [Option<String>; SLOTS],
    pub teams: Vec<Team>,
    /// Метки сущностей: ключ — UUID игрока шестнадцатеричной строкой.
    pub tags: BTreeMap<String, Vec<String>>,
    pub bossbars: Vec<BossBar>,

    /// Лента пакетов для подключений.
    log: Vec<Entry>,
    dropped: usize,

    /// Изменилось ли что-то с последней записи на диск.
    pub dirty: bool,
    file: Option<PathBuf>,
}

impl Scoreboard {
    pub fn new() -> Self {
        Self {
            objectives: Vec::new(),
            scores: BTreeMap::new(),
            arrival: Vec::new(),
            display: Default::default(),
            teams: Vec::new(),
            tags: BTreeMap::new(),
            bossbars: Vec::new(),
            log: Vec::new(),
            dropped: 0,
            dirty: false,
            file: None,
        }
    }

    /// Читает табло мира (или пустое, если файла нет).
    pub fn open(world: &Path) -> Self {
        let file = world.join("data").join("scoreboard.dat");
        let mut board = Self::new();

        if let Ok(text) = std::fs::read_to_string(&file) {
            match serde_json::from_str::<Stored>(&text) {
                Ok(stored) => {
                    board.objectives = stored.objectives;
                    board.arrival = stored.scores.keys().cloned().collect();
                    board.scores = stored.scores;
                    for (slot, name) in stored.display.into_iter().enumerate().take(SLOTS) {
                        board.display[slot] = name;
                    }
                    board.teams = stored.teams;
                    board.tags = stored.tags;
                    board.bossbars = stored.bossbars;
                }
                Err(error) => crate::log_warn!("Табло не прочитано ({}): {}", file.display(), error),
            }
        }

        board.file = Some(file);
        board
    }

    /// Что записать на диск, если есть что: путь и содержимое. Сама запись —
    /// вне замка, у вызвавшего.
    pub fn take_save(&mut self) -> Option<(PathBuf, String)> {
        if !self.dirty {
            return None;
        }

        self.dirty = false;
        let stored = Stored {
            objectives: self.objectives.clone(),
            scores: self.scores.clone(),
            display: self.display.to_vec(),
            teams: self.teams.clone(),
            tags: self.tags.clone(),
            bossbars: self.bossbars.clone(),
        };

        Some((self.file.clone()?, serde_json::to_string(&stored).ok()?))
    }

    // ---- лента ----

    fn push(&mut self, to: Option<Vec<[u8; 16]>>, body: Vec<u8>) {
        self.log.push((to, body));
        self.dirty = true;

        if self.log.len() > KEPT {
            let extra = self.log.len() - KEPT;
            self.log.drain(..extra);
            self.dropped += extra;
        }
    }

    /// Сколько пакетов всего было.
    pub fn count(&self) -> usize {
        self.dropped + self.log.len()
    }

    /// Пакеты с номера `from` для игрока `who`.
    pub fn since_for(&self, from: usize, who: &[u8; 16]) -> Vec<Vec<u8>> {
        let start = (from.max(self.dropped) - self.dropped).min(self.log.len());

        self.log[start..]
            .iter()
            .filter(|(to, _)| to.as_ref().is_none_or(|to| to.contains(who)))
            .map(|(_, body)| body.clone())
            .collect()
    }

    /// Всё табло для только что вошедшего: цели, счёт, места показа,
    /// команды и его полосы босса.
    pub fn snapshot_for(&self, who: &[u8; 16]) -> Vec<Vec<u8>> {
        let mut out = Vec::new();

        for objective in &self.objectives {
            if self.tracked(&objective.name) {
                out.extend(self.start_tracking(objective));
            }
        }

        for team in &self.teams {
            out.push(team_packet(team, 0, &team.members));
        }

        for bar in &self.bossbars {
            if bar.visible && bar.players.contains(who) {
                out.push(bar.add_packet());
            }
        }

        out
    }

    /// Показана ли цель хоть где-то: клиенты знают только о показанных
    /// (чёрный ящик, score-objectives: цель уходит клиенту при первом
    /// `setdisplay`, а убирается, когда её больше нигде не показывают).
    pub fn tracked(&self, name: &str) -> bool {
        self.display.iter().any(|shown| shown.as_deref() == Some(name))
    }

    /// Пакеты начала показа: цель, её места и весь её счёт.
    fn start_tracking(&self, objective: &Objective) -> Vec<Vec<u8>> {
        let mut out = vec![objective_packet(objective, 0)];

        for (slot, shown) in self.display.iter().enumerate() {
            if shown.as_deref() == Some(objective.name.as_str()) {
                out.push(display_packet(slot, &objective.name));
            }
        }

        for (holder, scores) in &self.scores {
            if let Some(score) = scores.get(&objective.name) {
                out.push(score_packet(holder, &objective.name, score));
            }
        }

        out
    }

    // ---- цели ----

    pub fn objective(&self, name: &str) -> Option<&Objective> {
        self.objectives.iter().find(|objective| objective.name == name)
    }

    pub fn add_objective(&mut self, objective: Objective) {
        self.objectives.push(objective);
        self.dirty = true;
    }

    pub fn remove_objective(&mut self, name: &str) {
        for slot in 0..SLOTS {
            if self.display[slot].as_deref() == Some(name) {
                self.set_display(slot, None);
            }
        }

        for scores in self.scores.values_mut() {
            scores.remove(name);
        }
        self.scores.retain(|_, scores| !scores.is_empty());
        let scores = &self.scores;
        self.arrival.retain(|holder| scores.contains_key(holder));

        self.objectives.retain(|objective| objective.name != name);
        self.dirty = true;
    }

    /// Изменить цель и разослать (действие 2).
    pub fn modify_objective(&mut self, name: &str, change: impl FnOnce(&mut Objective)) {
        if let Some(objective) = self.objectives.iter_mut().find(|objective| objective.name == name) {
            change(objective);
            let body = objective_packet(objective, 2);

            if self.tracked(name) {
                self.push(None, body);
            }
            self.dirty = true;
        }
    }

    /// Показать цель в месте (или очистить его) — с пакетами как у
    /// оригинала: прежняя цель, если её ещё где-то видно, уступает место
    /// пакетом показа, иначе убирается целиком; новая, если её уже знают, —
    /// пакетом показа, иначе приходит целиком (цель, места, счёт).
    pub fn set_display(&mut self, slot: usize, name: Option<String>) {
        let old = self.display[slot].clone();
        self.display[slot] = name.clone();
        self.dirty = true;

        if let Some(old) = old.filter(|old| Some(old) != name.as_ref()) {
            if self.tracked(&old) {
                self.push(None, display_packet(slot, name.as_deref().unwrap_or("")));
            } else if let Some(objective) = self.objective(&old).cloned() {
                self.push(None, objective_packet(&objective, 1));
            }
        }

        if let Some(name) = name {
            let others = self.display.iter().enumerate().any(|(other, shown)| other != slot && shown.as_deref() == Some(name.as_str()));

            if others {
                self.push(None, display_packet(slot, &name));
            } else if let Some(objective) = self.objective(&name).cloned() {
                for body in self.start_tracking(&objective) {
                    self.push(None, body);
                }
            }
        }
    }

    // ---- счёт ----

    pub fn score(&self, holder: &str, objective: &str) -> Option<&Score> {
        self.scores.get(holder).and_then(|scores| scores.get(objective))
    }

    /// Изменить счёт (заводя нулевой, если его не было) и разослать.
    pub fn change_score(&mut self, holder: &str, objective: &str, change: impl FnOnce(&mut Score)) -> i32 {
        self.note_arrival(holder);
        let score = self
            .scores
            .entry(holder.to_string())
            .or_default()
            .entry(objective.to_string())
            .or_insert(Score { value: 0, locked: true, display: None, format: None });

        change(score);
        let value = score.value;
        let body = score_packet(holder, objective, score);

        if self.tracked(objective) {
            self.push(None, body);
        }
        self.dirty = true;
        value
    }

    /// Завести нулевой счёт без рассылки, если его нет (у оригинала
    /// `operation` заводит счёт и источнику — чёрный ящик, score-players:
    /// `/= #zero` оставляет #zero в списке).
    fn note_arrival(&mut self, holder: &str) {
        if !self.scores.contains_key(holder) {
            self.arrival.push(holder.to_string());
        }
    }

    pub fn touch_score(&mut self, holder: &str, objective: &str) {
        self.note_arrival(holder);
        self.scores.entry(holder.to_string()).or_default().entry(objective.to_string()).or_insert(Score { value: 0, locked: true, display: None, format: None });
        self.dirty = true;
    }

    pub fn set_score(&mut self, holder: &str, objective: &str, value: i32) -> i32 {
        self.change_score(holder, objective, |score| score.value = value)
    }

    /// Сбросить счёт в одной цели (Some) или во всех (None).
    pub fn reset_score(&mut self, holder: &str, objective: Option<&str>) {
        let Some(scores) = self.scores.get_mut(holder) else {
            return;
        };

        let removed = match objective {
            Some(objective) => scores.remove(objective).is_some(),
            None => !scores.is_empty(),
        };
        scores.retain(|_, _| objective.is_some());

        // Последний счёт держателя ушёл — оригинал сбрасывает его целиком
        // (чёрный ящик, score-players: `reset #fake sbp_a` → Reset Score без цели).
        let gone = scores.is_empty();

        if gone {
            self.scores.remove(holder);
            self.arrival.retain(|known| known != holder);
        }

        if !removed {
            return;
        }

        let mut body = encode_varint(RESET_PACKET);
        body.extend(encode_string(holder));
        match objective.filter(|_| !gone) {
            Some(objective) => {
                if !self.tracked(objective) {
                    self.dirty = true;
                    return;
                }
                body.push(1);
                body.extend(encode_string(objective));
            }
            None => body.push(0),
        }
        self.push(None, body);
    }

    /// Прибавить ко всем счетам держателя по цели с таким условием (смерти,
    /// убийства): у оригинала такие счета заводятся сами, с нуля.
    pub fn bump_criteria(&mut self, holder: &str, criteria: &str, by: i32) {
        let names: Vec<String> = self.objectives.iter().filter(|objective| objective.criteria == criteria).map(|objective| objective.name.clone()).collect();

        for name in names {
            self.change_score(holder, &name, |score| score.value = score.value.wrapping_add(by));
        }
    }

    /// Задать счёт по условию (здоровье, сытость…), если он поменялся.
    pub fn set_criteria(&mut self, holder: &str, criteria: &str, value: i32) {
        let names: Vec<String> = self.objectives.iter().filter(|objective| objective.criteria == criteria).map(|objective| objective.name.clone()).collect();

        for name in names {
            if self.score(holder, &name).map(|score| score.value) != Some(value) {
                self.set_score(holder, &name, value);
            }
        }
    }

    /// Условия-статистики у целей (`minecraft.custom:minecraft.jump`).
    pub fn stat_criteria(&self) -> Vec<String> {
        let mut list: Vec<String> = self.objectives.iter().filter(|objective| objective.criteria.contains(':')).map(|objective| objective.criteria.clone()).collect();
        list.sort();
        list.dedup();
        list
    }

    /// Есть ли цели с таким условием — чтобы не считать зря.
    pub fn watches(&self, criteria: &str) -> bool {
        self.objectives.iter().any(|objective| objective.criteria == criteria)
    }

    // ---- команды ----

    pub fn team(&self, name: &str) -> Option<&Team> {
        self.teams.iter().find(|team| team.name == name)
    }

    /// В какой команде держатель.
    pub fn team_of(&self, holder: &str) -> Option<&Team> {
        self.teams.iter().find(|team| team.members.iter().any(|member| member == holder))
    }

    /// Новая команда: пакет «создать», а следом «изменить» — так у
    /// оригинала (чёрный ящик, team-basic).
    pub fn add_team(&mut self, team: Team) {
        // Создаётся с именем вместо названия; название приходит изменением.
        let named = Team { display: literal(&team.name), ..team.clone() };
        let body = team_packet(&named, 0, &[]);
        let change = team_packet(&team, 2, &[]);
        self.teams.push(team);
        self.push(None, body);
        self.push(None, change);
    }

    pub fn remove_team(&mut self, name: &str) {
        if let Some(at) = self.teams.iter().position(|team| team.name == name) {
            let team = self.teams.remove(at);
            self.push(None, team_packet(&team, 1, &[]));
        }
    }

    pub fn modify_team(&mut self, name: &str, change: impl FnOnce(&mut Team)) {
        if let Some(team) = self.teams.iter_mut().find(|team| team.name == name) {
            change(team);
            let body = team_packet(team, 2, &[]);
            self.push(None, body);
        }
    }

    /// Вступить в команду (выйдя из прежней).
    pub fn join_team(&mut self, name: &str, holders: &[String]) {
        for holder in holders {
            if self.team_of(holder).is_some_and(|team| team.name != name) {
                self.leave_team(std::slice::from_ref(holder));
            }
        }

        if let Some(team) = self.teams.iter_mut().find(|team| team.name == name) {
            let fresh: Vec<String> = holders.iter().filter(|holder| !team.members.contains(holder)).cloned().collect();
            team.members.extend(fresh.iter().cloned());
            let body = team_packet(team, 3, &fresh);
            self.push(None, body);
        }
    }

    /// Выйти из команд; возвращает, скольких это коснулось.
    pub fn leave_team(&mut self, holders: &[String]) -> usize {
        let mut left = 0;
        let mut bodies = Vec::new();

        for team in &mut self.teams {
            let gone: Vec<String> = holders.iter().filter(|holder| team.members.contains(holder)).cloned().collect();

            if !gone.is_empty() {
                team.members.retain(|member| !gone.contains(member));
                left += gone.len();
                bodies.push(team_packet(team, 4, &gone));
            }
        }

        for body in bodies {
            self.push(None, body);
        }

        left
    }

    /// Можно ли игроку `attacker` ранить `victim` (дружеский огонь, вики:
    /// Scoreboard, «friendlyFire»).
    pub fn may_hurt(&self, attacker: &str, victim: &str) -> bool {
        match (self.team_of(attacker), self.team_of(victim)) {
            (Some(one), Some(other)) if one.name == other.name => one.friendly_fire,
            _ => true,
        }
    }

    // ---- метки ----

    pub fn tags_of(&self, key: &str) -> &[String] {
        self.tags.get(key).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn add_tag(&mut self, key: &str, tag: &str) -> bool {
        let tags = self.tags.entry(key.to_string()).or_default();

        if tags.iter().any(|known| known == tag) || tags.len() >= 1024 {
            return false;
        }

        tags.push(tag.to_string());
        self.dirty = true;
        true
    }

    pub fn remove_tag(&mut self, key: &str, tag: &str) -> bool {
        let Some(tags) = self.tags.get_mut(key) else {
            return false;
        };
        let before = tags.len();
        tags.retain(|known| known != tag);
        let removed = tags.len() != before;

        if tags.is_empty() {
            self.tags.remove(key);
        }

        self.dirty |= removed;
        removed
    }

    /// Сообщение в чат только этим игрокам (`teammsg`).
    pub fn send_chat(&mut self, to: Vec<[u8; 16]>, component: Vec<u8>) {
        if to.is_empty() {
            return;
        }
        let mut body = encode_varint(SYSTEM_CHAT_PACKET);
        body.extend(component);
        body.push(0);
        self.log.push((Some(to), body));
    }

    // ---- полосы ----

    pub fn bossbar(&self, id: &str) -> Option<&BossBar> {
        self.bossbars.iter().find(|bar| bar.id == id)
    }

    pub fn add_bossbar(&mut self, bar: BossBar) {
        self.bossbars.push(bar);
        self.dirty = true;
    }

    pub fn remove_bossbar(&mut self, id: &str) {
        if let Some(at) = self.bossbars.iter().position(|bar| bar.id == id) {
            let bar = self.bossbars.remove(at);

            if bar.visible && !bar.players.is_empty() {
                self.push(Some(bar.players.clone()), bar.remove_packet());
            }
        }
    }

    /// Изменить полосу. `update` — что разослать тем, кто её видит:
    /// действие (2 — доля, 3 — название, 4 — вид, 5 — флаги).
    pub fn modify_bossbar(&mut self, id: &str, update: Option<i32>, change: impl FnOnce(&mut BossBar)) {
        let Some(bar) = self.bossbars.iter_mut().find(|bar| bar.id == id) else {
            return;
        };
        let was_visible = bar.visible;
        let was_players = bar.players.clone();

        change(bar);

        let bar = bar.clone();
        self.dirty = true;

        // Показ: кто перестал видеть — убрать, кто стал — добавить.
        let before: Vec<[u8; 16]> = if was_visible { was_players } else { Vec::new() };
        let after: Vec<[u8; 16]> = if bar.visible { bar.players.clone() } else { Vec::new() };
        let gone: Vec<[u8; 16]> = before.iter().filter(|who| !after.contains(who)).copied().collect();
        let came: Vec<[u8; 16]> = after.iter().filter(|who| !before.contains(who)).copied().collect();
        let stay: Vec<[u8; 16]> = after.iter().filter(|who| before.contains(who)).copied().collect();

        if !gone.is_empty() {
            self.push(Some(gone), bar.remove_packet());
        }

        if let (Some(action), false) = (update, stay.is_empty()) {
            let mut body = encode_varint(BOSS_PACKET);
            body.extend_from_slice(&bar.uuid);
            body.extend(encode_varint(action));
            match action {
                2 => body.extend_from_slice(&bar.progress().to_be_bytes()),
                3 => body.extend(encode_component(&bar.name)),
                4 => {
                    body.extend(encode_varint(bar.color as i32));
                    body.extend(encode_varint(bar.style as i32));
                }
                _ => body.push(0),
            }
            self.push(Some(stay), body);
        }

        if !came.is_empty() {
            self.push(Some(came), bar.add_packet());
        }
    }
}

impl Default for Scoreboard {
    fn default() -> Self {
        Self::new()
    }
}

/// Пакет цели: 0 — добавить, 1 — убрать, 2 — изменить.
fn objective_packet(objective: &Objective, action: u8) -> Vec<u8> {
    let mut body = encode_varint(OBJECTIVE_PACKET);
    body.extend(encode_string(&objective.name));
    body.push(action);

    if action != 1 {
        body.extend(encode_component(&objective.display));
        body.extend(encode_varint(i32::from(objective.hearts)));
        NumberFormat::write(&objective.format, &mut body);
    }

    body
}

fn display_packet(slot: usize, name: &str) -> Vec<u8> {
    let mut body = encode_varint(DISPLAY_PACKET);
    body.extend(encode_varint(slot as i32));
    body.extend(encode_string(name));
    body
}

fn score_packet(holder: &str, objective: &str, score: &Score) -> Vec<u8> {
    let mut body = encode_varint(SCORE_PACKET);
    body.extend(encode_string(holder));
    body.extend(encode_string(objective));
    body.extend(encode_varint(score.value));

    match &score.display {
        Some(display) => {
            body.push(1);
            body.extend(encode_component(display));
        }
        None => body.push(0),
    }

    NumberFormat::write(&score.format, &mut body);
    body
}

/// Пакет команды: 0 — создать (с участниками), 1 — убрать, 2 — изменить,
/// 3 — вступили, 4 — вышли.
fn team_packet(team: &Team, mode: u8, players: &[String]) -> Vec<u8> {
    let mut body = encode_varint(TEAMS_PACKET);
    body.extend(encode_string(&team.name));
    body.push(mode);

    if mode == 0 || mode == 2 {
        body.extend(encode_component(&team.display));
        body.push(u8::from(team.friendly_fire) | (u8::from(team.see_invisible) << 1));
        body.extend(encode_varint(team.nametag as i32));
        body.extend(encode_varint(team.collision as i32));
        body.extend(encode_varint(team.color));
        body.extend(encode_component(&team.prefix));
        body.extend(encode_component(&team.suffix));
    }

    if mode == 0 || mode == 3 || mode == 4 {
        body.extend(encode_varint(players.len() as i32));
        for player in players {
            body.extend(encode_string(player));
        }
    }

    body
}

// ---- текст ----

/// Поля оформления, у которых значение — признак.
const FLAGS: [&str; 6] = ["bold", "italic", "underlined", "strikethrough", "obfuscated", "interpret"];

/// Разбирает текст из команды: JSON или SNBT (`"x"`, `{text:"x",color:"red"}`,
/// `["a",{text:"b"}]`). None — не текст.
pub fn parse_component(input: &str) -> Option<Value> {
    let input = input.trim();

    if input.is_empty() {
        return None;
    }

    let parsed = crate::snbt::parse(input)?;
    Some(from_snbt(&parsed, ""))
}

fn from_snbt(value: &crate::snbt::Value, key: &str) -> Value {
    use crate::snbt::Value as S;

    match value {
        S::Num(number) if FLAGS.contains(&key) => Value::Bool(*number != 0.0),
        S::Num(number) if number.fract() == 0.0 && number.abs() < 2e9 => Value::from(*number as i64),
        S::Num(number) => Value::from(*number),
        S::Str(text) if FLAGS.contains(&key) && (text == "true" || text == "false") => Value::Bool(text == "true"),
        S::Str(text) => Value::String(text.clone()),
        S::List(items) => Value::Array(items.iter().map(|item| from_snbt(item, key)).collect()),
        S::Map(fields) => Value::Object(fields.iter().map(|(name, item)| (name.clone(), from_snbt(item, name))).collect()),
    }
}

/// Простой текст.
pub fn literal(text: &str) -> Value {
    Value::String(text.to_string())
}

/// Текст сетевым NBT, как его ждёт клиент.
///
/// Порядок полей — как у оригинала: его компаунд — хеш-таблица Java, и
/// поля идут по корзинам `hashCode` (чёрный ящик, score-objectives:
/// `{"color":…,"text":…,"hover_event":…}`, `{"with":…,"translate":…}`).
/// Простой текст без оформления — строкой; список с разными видами
/// значений — компаундами с пустым ключом (`{"": 5}`), как у оригинала.
pub fn encode_component(value: &Value) -> Vec<u8> {
    let value = normalize(value);
    let mut out = Vec::new();

    match &value {
        Value::String(text) => {
            out.push(TAG_STRING);
            nbt::write_string(&mut out, text);
        }
        Value::Object(map) => {
            out.push(TAG_COMPOUND);
            write_compound(&mut out, map);
        }
        other => {
            out.push(TAG_STRING);
            nbt::write_string(&mut out, &plain_text(other));
        }
    }

    out
}

const TAG_END: u8 = 0;
const TAG_BYTE: u8 = 1;
const TAG_INT: u8 = 3;
const TAG_DOUBLE: u8 = 6;
const TAG_STRING: u8 = 8;
const TAG_LIST: u8 = 9;
const TAG_COMPOUND: u8 = 10;

/// Приводит текст к виду оригинала: список — первый элемент с остальными
/// в `extra`; компаунд с одним `text` — строкой.
fn normalize(value: &Value) -> Value {
    match value {
        Value::Array(items) => {
            let mut items = items.iter();
            let Some(first) = items.next() else {
                return literal("");
            };
            let rest: Vec<Value> = items.map(normalize).collect();
            let first = normalize(first);

            if rest.is_empty() {
                return first;
            }

            let mut root = match first {
                Value::Object(map) => map,
                other => {
                    let mut map = serde_json::Map::new();
                    map.insert("text".to_string(), Value::String(plain_text(&other)));
                    map
                }
            };
            root.insert("extra".to_string(), Value::Array(rest));
            Value::Object(root)
        }
        Value::Object(map) => {
            if map.len() == 1 && let Some(Value::String(text)) = map.get("text") {
                return Value::String(text.clone());
            }

            Value::Object(
                map.iter()
                    .map(|(key, item)| {
                        let item = match (key.as_str(), item) {
                            ("extra" | "with", Value::Array(list)) => Value::Array(list.iter().map(normalize).collect()),
                            ("hover_event" | "click_event", other) => other.clone(),
                            (_, Value::Object(_)) => normalize(item),
                            _ => item.clone(),
                        };
                        (key.clone(), item)
                    })
                    .collect(),
            )
        }
        other => other.clone(),
    }
}

/// `String.hashCode()` Java — по знакам UTF-16.
fn java_hash(text: &str) -> u32 {
    text.encode_utf16().fold(0u32, |hash, unit| hash.wrapping_mul(31).wrapping_add(u32::from(unit)))
}

/// Порядок обхода открытой хеш-таблицы fastutil (ключи-строки, 32 ячейки,
/// перемешивание `HashCommon.mix`), по возрастанию ячеек: так оригинал
/// перечисляет держателей счёта, участников команды и метки (чёрный ящик:
/// «#fake, blackbox, #zero», «#fake, blackbox», «tg_one, tg_two»)
/// [проверить на больших списках].
pub fn open_hash_order<T: AsRef<str>>(keys: &[T]) -> Vec<usize> {
    let mut size = 32usize;
    while keys.len() * 4 > size * 3 {
        size *= 2;
    }
    let mut slots: Vec<Option<usize>> = vec![None; size];

    for (index, key) in keys.iter().enumerate() {
        let hash = java_hash(key.as_ref()).wrapping_mul(0x9E37_79B9);
        let mut at = (hash ^ (hash >> 16)) as usize & (size - 1);
        while slots[at].is_some() {
            at = (at + 1) & (size - 1);
        }
        slots[at] = Some(index);
    }

    slots.into_iter().flatten().collect()
}

/// Порядок обхода хеш-таблицы Java (HashMap/HashSet) с такими ключами,
/// добавленными по порядку: по корзинам, внутри корзины — по добавлению.
pub fn java_order<T: AsRef<str>>(keys: &[T]) -> Vec<usize> {
    let mut capacity = 16usize;
    while keys.len() * 4 > capacity * 3 {
        capacity *= 2;
    }
    let mut order: Vec<usize> = (0..keys.len()).collect();
    order.sort_by_key(|&index| {
        let hash = java_hash(keys[index].as_ref());
        ((hash ^ (hash >> 16)) as usize & (capacity - 1), index)
    });
    order
}

/// Порядок, в котором оригинал добавляет поля текста: содержимое, потом
/// оформление, потом `extra` — им решается порядок внутри одной корзины
/// (`{"text":…,"bold":…}` — чёрный ящик, score-objectives).
const FIELD_ORDER: [&str; 30] = [
    "action", "text", "translate", "fallback", "with", "score", "selector", "separator", "keybind", "nbt", "interpret", "block", "entity", "storage", "source", "color",
    "shadow_color", "bold", "italic", "underlined", "strikethrough", "obfuscated", "click_event", "hover_event", "insertion", "font", "extra", "id", "uuid", "name",
];

fn write_compound(out: &mut Vec<u8>, map: &serde_json::Map<String, Value>) {
    let mut keys: Vec<&String> = map.keys().collect();
    keys.sort_by_key(|key| FIELD_ORDER.iter().position(|known| known == key).unwrap_or(FIELD_ORDER.len()));

    for index in java_order(&keys) {
        let key = keys[index];
        write_named(out, key, &map[key.as_str()]);
    }

    out.push(TAG_END);
}

fn tag_of(value: &Value) -> u8 {
    match value {
        Value::Bool(_) => TAG_BYTE,
        Value::Number(number) if number.is_i64() => TAG_INT,
        Value::Number(_) => TAG_DOUBLE,
        Value::String(_) | Value::Null => TAG_STRING,
        Value::Array(_) => TAG_LIST,
        Value::Object(_) => TAG_COMPOUND,
    }
}

fn write_named(out: &mut Vec<u8>, key: &str, value: &Value) {
    // UUID в показе сущности — массивом целых, как у оригинала.
    if key == "uuid" && let Value::Array(items) = value {
        out.push(11);
        nbt::write_string(out, key);
        out.extend_from_slice(&(items.len() as i32).to_be_bytes());
        for item in items {
            out.extend_from_slice(&(item.as_i64().unwrap_or(0) as i32).to_be_bytes());
        }
        return;
    }

    out.push(tag_of(value));
    nbt::write_string(out, key);
    write_payload(out, value);
}

fn write_payload(out: &mut Vec<u8>, value: &Value) {
    match value {
        Value::Bool(flag) => out.push(u8::from(*flag)),
        Value::Number(number) => match number.as_i64() {
            Some(whole) => out.extend_from_slice(&(whole as i32).to_be_bytes()),
            None => out.extend_from_slice(&number.as_f64().unwrap_or(0.0).to_be_bytes()),
        },
        Value::String(text) => nbt::write_string(out, text),
        Value::Null => nbt::write_string(out, ""),
        Value::Object(map) => write_compound(out, map),
        Value::Array(items) => {
            let first = items.first().map_or(TAG_END, tag_of);
            let same = items.iter().all(|item| tag_of(item) == first);

            if same {
                out.push(first);
                out.extend_from_slice(&(items.len() as i32).to_be_bytes());
                for item in items {
                    write_payload(out, item);
                }
            } else {
                out.push(TAG_COMPOUND);
                out.extend_from_slice(&(items.len() as i32).to_be_bytes());
                for item in items {
                    match item {
                        Value::Object(map) => write_compound(out, map),
                        other => {
                            write_named(out, "", other);
                            out.push(TAG_END);
                        }
                    }
                }
            }
        }
    }
}

/// Текст без оформления — для консоли и Bedrock.
pub fn plain_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Number(number) => number.to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::Array(items) => items.iter().map(plain_text).collect(),
        Value::Object(map) => {
            let mut out = match (map.get("text"), map.get("translate")) {
                (Some(text), _) => plain_text(text),
                (None, Some(key)) => {
                    let args: Vec<String> = map.get("with").and_then(Value::as_array).map(|with| with.iter().map(plain_text).collect()).unwrap_or_default();
                    crate::commands_score::russian(key.as_str().unwrap_or(""), &args)
                }
                _ => String::new(),
            };

            if let Some(Value::Array(extra)) = map.get("extra") {
                for item in extra {
                    out.push_str(&plain_text(item));
                }
            }

            out
        }
        Value::Null => String::new(),
    }
}

/// Перевод с доводами: `{"translate": key, "with": [...]}`.
pub fn translate(key: &str, with: Vec<Value>) -> Value {
    let mut map = serde_json::Map::new();
    map.insert("translate".to_string(), Value::String(key.to_string()));

    if !with.is_empty() {
        map.insert("with".to_string(), Value::Array(with));
    }

    Value::Object(map)
}

/// Держатели, у которых есть хоть один счёт, — для `players list`.
/// В порядке появления (см. `arrival`).
pub fn holders(board: &Scoreboard) -> Vec<String> {
    board
        .arrival
        .iter()
        .filter(|holder| board.scores.get(holder.as_str()).is_some_and(|scores| scores.keys().any(|name| board.objective(name).is_some())))
        .cloned()
        .collect()
}

/// UUID случайный для новой полосы: от времени и счётчика.
pub fn fresh_uuid(seed: &str) -> [u8; 16] {
    use std::hash::{Hash, Hasher};
    let mut out = [0u8; 16];
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());

    for (half, chunk) in out.chunks_mut(8).enumerate() {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        (seed, nanos, half).hash(&mut hasher);
        chunk.copy_from_slice(&hasher.finish().to_be_bytes());
    }

    out[6] = (out[6] & 0x0f) | 0x40;
    out[8] = (out[8] & 0x3f) | 0x80;
    out
}

/// Ключ меток игрока — UUID строкой.
pub fn tag_key(uuid: &[u8; 16]) -> String {
    uuid.iter().map(|byte| format!("{byte:02x}")).collect()
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn components_are_parsed_from_json_and_snbt() {
        assert_eq!(parse_component("\"x\""), Some(literal("x")));
        assert_eq!(parse_component("{\"text\":\"a\",\"bold\":true}"), Some(serde_json::json!({"text":"a","bold":true})));
        assert_eq!(parse_component("{color:\"red\"}"), Some(serde_json::json!({"color":"red"})));
        assert_eq!(plain_text(&serde_json::json!(["a", {"text":"b"}])), "ab");
    }

    /// Порядок полей — как у хеш-таблицы Java (чёрный ящик, score-objectives).
    #[test]
    fn fields_follow_java_hash_order() {
        fn order(keys: &[&'static str]) -> Vec<&'static str> { java_order(keys).into_iter().map(|index| keys[index]).collect() }
        assert_eq!(order(&["text", "color", "hover_event"]), ["color", "text", "hover_event"]);
        assert_eq!(order(&["translate", "with"]), ["with", "translate"]);
        assert_eq!(order(&["text", "color", "extra", "click_event"]), ["click_event", "color", "extra", "text"]);
        // Простой текст — строкой; разнородный список — компаундами с пустым ключом.
        assert_eq!(encode_component(&serde_json::json!({"text": "a"})), [8, 0, 1, b'a']);
        let mixed = encode_component(&serde_json::json!({"translate": "k", "with": [1, "x"]}));
        assert!(mixed.windows(3).any(|w| w == [9, 0, 4]) || mixed.windows(2).any(|w| w == [10, 0]));
    }

    /// Порядок списков — как у оригинала (чёрный ящик, score-players, team-basic, tag-selectors).
    #[test]
    fn lists_follow_open_hash_order() {
        let order = |keys: &[&'static str]| open_hash_order(keys).into_iter().map(|index| keys[index]).collect::<Vec<&str>>();
        assert_eq!(order(&["blackbox", "#fake", "#zero"]), ["#fake", "blackbox", "#zero"]);
        assert_eq!(order(&["blackbox", "#fake"]), ["#fake", "blackbox"]);
        assert_eq!(order(&["tg_one", "tg_two"]), ["tg_one", "tg_two"]);
        // Поля одной корзины — в порядке оригинала: сперва текст, потом оформление.
        let bytes = encode_component(&serde_json::json!({"bold": true, "text": "a"}));
        assert_eq!(&bytes[..5], &[10, 8, 0, 4, b't']);
    }

    #[test]
    fn slots_round_trip() {
        for slot in 0..SLOTS {
            assert_eq!(slot_by_name(&slot_name(slot)), Some(slot));
        }
        assert_eq!(slot_by_name("sidebar.team.red"), Some(15));
    }

    #[test]
    fn scores_and_teams_change_and_reach_the_feed() {
        let mut board = Scoreboard::new();
        board.add_objective(Objective { name: "a".into(), criteria: "dummy".into(), display: literal("a"), hearts: false, auto_update: false, format: None });
        assert_eq!(board.set_score("p", "a", 5), 5);
        assert_eq!(board.change_score("p", "a", |score| score.value += 2), 7);
        board.add_team(Team { name: "t".into(), display: literal("t"), color: COLOR_RESET, prefix: literal(""), suffix: literal(""), friendly_fire: false, see_invisible: true, nametag: 0, death_messages: 0, collision: 0, members: vec![] });
        board.join_team("t", &["p".to_string(), "q".to_string()]);
        assert!(!board.may_hurt("p", "q"));
        assert!(board.may_hurt("p", "r"));
        assert_eq!(board.leave_team(&["p".to_string()]), 1);
        assert!(board.may_hurt("p", "q"));
        // Цель не показана — клиенты о ней не знают: в ленте только команды
        // (создать + изменить, вступили, вышли).
        assert_eq!(board.count(), 4);
        board.set_display(1, Some("a".into()));
        // Показ: цель, место, счёт.
        assert_eq!(board.count(), 7);
        board.remove_objective("a");
        assert!(board.score("p", "a").is_none());
        assert_eq!(board.count(), 8);
        assert_eq!(board.since_for(5, &[0; 16]).len(), 3);
    }
}
