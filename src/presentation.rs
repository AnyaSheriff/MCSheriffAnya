// Оформление Java: minecraft.wiki/w/Java_Edition_protocol/Packets,
// «Player List Header And Footer». Номера и поля: minecraft-data, 26.1.
// [проверить] Внешний вид нового оформления требует проверки клиентом.
use std::collections::HashMap;
use serde_json::{Value, json};
use crate::network::varint::encode_varint;
use crate::players::{Players, Order};
use crate::scoreboard::{self, Objective, Team, BossBar};

type Uuid = [u8; 16];

#[derive(Clone, Default)]
pub struct Info {
    pub plugin_state: PlayerState,
    pub inventory: Vec<Option<crate::inventory::Stack>>,
    pub selected: u32,
    pub cursor: Option<crate::inventory::Stack>,
    pub open_top: Vec<Option<crate::inventory::Stack>>,
    pub open_kind: String,
    pub ping: u32,
    pub health: f32,
    pub max_health: f32,
    pub food: i32,
    pub level: i32,
    pub exp: f32,
}

#[derive(Clone, Default)]
pub struct PlayerState {
    pub flight_owners: [Option<String>;3],
    pub flight_allowed: bool,
    pub flying: bool,
    pub flight_fall_damage: Option<bool>,
    pub velocity: (f64, f64, f64),
    pub fire_ticks: i32,
    pub in_water: bool,
    pub in_lava: bool,
    pub in_rain: bool,
    pub eye_height: f64,
    pub effects: Vec<crate::survival::Effect>,
}

#[derive(Default)]
pub struct Owned {
    pub teams: HashMap<String, Team>,
    pub objectives: HashMap<String, Objective>,
    pub bars: HashMap<String, BossBar>,
    pub cache: HashMap<String, Vec<u8>>,
}

#[derive(Default)]
pub struct State {
    // Наличие снимка означает Java-подключение; Bedrock пакеты не получает.
    pub info: HashMap<Uuid, Info>,
    pub headers: HashMap<Uuid, String>,
    pub displays: HashMap<Uuid, (String, Value)>,
    pub list_names: HashMap<Uuid, (String, Value)>,
    pub actions: HashMap<Uuid, String>,
    pub slots: HashMap<(Uuid, usize), (String, String)>,
    pub owned: HashMap<(String, Uuid), Owned>,
}

impl State {
    pub fn leave(&mut self, uuid: &Uuid) {
        self.info.remove(uuid);
        self.headers.remove(uuid);
        self.displays.remove(uuid);
        self.list_names.remove(uuid);
        self.actions.remove(uuid);
        self.slots.retain(|(viewer, _), _| viewer != uuid);
        self.owned.retain(|(_, viewer), _| viewer != uuid);
    }
}

pub fn header_packet(header: &Value, footer: &Value) -> Vec<u8> {
    let mut body = encode_varint(0x7a);
    body.extend(scoreboard::encode_component(header));
    body.extend(scoreboard::encode_component(footer));
    body
}

pub fn list_packet(uuid: &Uuid, name: Option<&Value>) -> Vec<u8> {
    let mut body = encode_varint(0x46);
    body.extend([0x20, 1]);
    body.extend(uuid);
    body.push(u8::from(name.is_some()));
    if let Some(name) = name { body.extend(scoreboard::encode_component(name)); }
    body
}

pub fn action_packet(text: &Value) -> Vec<u8> {
    let mut body = encode_varint(0x57);
    body.extend(scoreboard::encode_component(text));
    body
}

pub fn footer(tps: f32, mspt: f32) -> Value {
    let tps_color = if tps >= 19.5 { "green" } else if tps >= 15.0 { "yellow" } else { "red" };
    let mspt_color = if mspt <= 50.0 { "green" } else if mspt <= 1000.0 / 15.0 { "yellow" } else { "red" };
    json!({"text":"", "extra":[{"text":format!("TPS {tps:.1}"), "color":tps_color},
        {"text":" · ","color":"gray"}, {"text":format!("MSPT {mspt:.1} мс"),"color":mspt_color}]})
}

pub fn tick_footer(shared: &crate::shared::Shared) {
    if !shared.settings.tab_tps { return; }
    let (tps, mspt) = shared.world_cmd.ticking.lock().expect("такт").metrics();
    let packet = header_packet(&json!(""), &footer(tps, mspt));
    let mut players = shared.players.lock().expect("игроки");
    let viewers: Vec<_> = players.ui.info.keys().filter(|id| !players.ui.headers.contains_key(*id)).copied().collect();
    for uuid in viewers { players.order(&uuid, Order::Packet(packet.clone())); }
}

pub fn broadcast_list(players: &mut Players, uuid: &Uuid, value: Option<&Value>) {
    let packet = list_packet(uuid, value);
    let viewers: Vec<_> = players.ui.info.keys().copied().collect();
    for viewer in viewers { players.order(&viewer, Order::Packet(packet.clone())); }
}

// Освобождаем только то, что ещё принадлежит этому плагину. Чужой заголовок
// или слот, установленный позже, при выгрузке не трогаем.
pub fn reset(players: &mut Players, owner: &str, uuid: Uuid) {
    if let Some(owned) = players.ui.owned.remove(&(owner.to_string(), uuid)) {
        for team in owned.teams.values() { players.order(&uuid, Order::Packet(scoreboard::team_packet(team, 1, &[]))); }
        for objective in owned.objectives.values() { players.order(&uuid, Order::Packet(scoreboard::objective_packet(objective, 1))); }
        for bar in owned.bars.values() { players.order(&uuid, Order::Packet(bar.remove_packet())); }
    }
    let slots: Vec<_> = players.ui.slots.iter().filter(|((viewer, _), (plugin, _))| *viewer == uuid && plugin == owner).map(|((_, slot), _)| *slot).collect();
    for slot in slots {
        players.ui.slots.remove(&(uuid, slot));
        players.order(&uuid, Order::Packet(scoreboard::display_packet(slot, "")));
    }
    if players.ui.headers.get(&uuid).is_some_and(|plugin| plugin == owner) {
        players.ui.headers.remove(&uuid);
        players.order(&uuid, Order::Packet(header_packet(&json!(""), &json!(""))));
    }
    if players.ui.actions.get(&uuid).is_some_and(|plugin| plugin == owner) {
        players.ui.actions.remove(&uuid);
        players.order(&uuid, Order::Packet(action_packet(&json!(""))));
    }
    if players.ui.displays.get(&uuid).is_some_and(|(plugin, _)| plugin == owner) { players.ui.displays.remove(&uuid); }
    if players.ui.list_names.get(&uuid).is_some_and(|(plugin, _)| plugin == owner) {
        players.ui.list_names.remove(&uuid);
        broadcast_list(players, &uuid, None);
    }
}

pub fn release(shared: &crate::shared::Shared, owner: &str) {
    let mut players = shared.players.lock().expect("игроки");
    let uuids: Vec<_> = players.ui.info.keys().copied().collect();
    for uuid in uuids { reset(&mut players, owner, uuid); }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn footer_boundaries_and_nbt() {
        for (tps, color) in [(20.0,"green"),(19.5,"green"),(19.49,"yellow"),(15.0,"yellow"),(14.9,"red")] {
            let value = footer(tps, 67.0);
            assert_eq!(value["extra"][0]["color"], color);
            assert_eq!(value["extra"][2]["color"], "red");
        }
        let packet = header_packet(&json!(""), &json!({"text":"TPS 20.0","color":"green"}));
        assert_eq!(&packet[..4], &[0x7a, 8, 0, 0]);
        assert_eq!(packet[4], 10);
        assert_eq!(list_packet(&[1;16], None)[1], 0x20);
    }
    #[test]
    fn actual_intervals_report_lag_and_window_expires() {
        let mut ticking = crate::commands_world::Ticking::default();
        let start = std::time::Instant::now();
        ticking.note_start(start);
        ticking.note_start(start + std::time::Duration::from_millis(100));
        assert!((ticking.metrics().0 - 10.0).abs() < 0.001);
        for i in 1..=100 { ticking.note_start(start + std::time::Duration::from_millis(100 + i * 50)); }
        assert_eq!(ticking.metrics().0, 20.0);
    }
}
