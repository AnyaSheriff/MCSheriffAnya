// Типизированные вызовы оформления. Произвольные пакеты гостю не доступны.
use serde_json::{Value, json};
use super::api::{PluginCtx, parse_uuid, mcsa::api::presentation as api};
use crate::presentation::{self as ui, Owned};
use crate::players::{Players, Order};
use crate::scoreboard::{self as board, NumberFormat};
use crate::network::{varint::{encode_varint, encode_string}};

fn component(text: &str) -> Option<Value> {
    if text.len() > 16384 { return None; }
    let mut value: Value = serde_json::from_str(text).ok()?;
    if !valid_component(&value) { return None; }
    drop_empty_lists(&mut value);
    Some(value)
}
/// Пустые `extra` и `with` клиент не принимает (список должен быть непустым)
/// и рвёт соединение — убираем их, а пустую строку конфига оставляем пустым текстом.
fn drop_empty_lists(value: &mut Value) {
    match value {
        Value::Array(values) => values.iter_mut().for_each(drop_empty_lists),
        Value::Object(map) => {
            map.retain(|key, inner| !(matches!(key.as_str(), "extra" | "with") && inner.as_array().is_some_and(Vec::is_empty)));
            map.values_mut().for_each(drop_empty_lists);
        }
        _ => {}
    }
}
fn valid_component(value: &Value) -> bool {
    match value {
        Value::String(_) => true,
        Value::Array(values) => !values.is_empty() && values.iter().all(valid_component),
        Value::Object(map) => {
            if !["text", "translate", "keybind"].iter().any(|key| map.get(*key).is_some_and(Value::is_string)) { return false; }
            map.iter().all(|(key, value)| match key.as_str() {
                "text" | "translate" | "fallback" | "keybind" | "font" | "insertion" => value.is_string(),
                "color" => value.as_str().is_some_and(|c| board::COLORS.contains(&c) || c == "reset" || (c.len() == 7 && c.starts_with('#') && c[1..].bytes().all(|b| b.is_ascii_hexdigit()))),
                "bold" | "italic" | "underlined" | "strikethrough" | "obfuscated" => value.is_boolean(),
                "shadow_color" => value.as_i64().is_some(),
                "extra" | "with" => value.as_array().is_some_and(|values| values.iter().all(valid_component)),
                "click_event" => value.as_object().is_some_and(|event| {
                    match event.get("action").and_then(Value::as_str) {
                        Some("open_url") => event.get("url").is_some_and(Value::is_string),
                        Some("run_command" | "suggest_command") => event.get("command").is_some_and(Value::is_string),
                        Some("copy_to_clipboard") => event.get("value").is_some_and(Value::is_string),
                        Some("change_page") => event.get("page").and_then(Value::as_i64).is_some_and(|page| page > 0),
                        _ => false,
                    }
                }),
                "hover_event" => value.get("action").and_then(Value::as_str) == Some("show_text") && value.get("value").is_some_and(valid_component),
                _ => false,
            })
        }
        _ => false,
    }
}
fn stable_uuid(seed: &str) -> [u8;16] {
    use md5::{Digest, Md5};
    let mut uuid: [u8;16] = Md5::digest(seed).into();
    uuid[6] = (uuid[6] & 0x0f) | 0x30;
    uuid[8] = (uuid[8] & 0x3f) | 0x80;
    uuid
}
fn id(owner: &str, local: &str) -> Option<String> {
    if local.is_empty() || local.len() > 128 || local.chars().any(char::is_control) { return None; }
    let uuid = stable_uuid(&format!("plugin:{owner}:{local}"));
    Some(uuid[..8].iter().map(|byte| format!("{byte:02x}")).collect())
}
fn send(players: &mut Players, uuid: [u8;16], body: Vec<u8>) { players.order(&uuid, Order::Packet(body)); }
fn cached(owned: &mut Owned, key: String, packet: Vec<u8>, out: &mut Vec<Vec<u8>>) -> bool {
    if owned.cache.get(&key) == Some(&packet) { return true; }
    if owned.cache.len() >= 2048 && !owned.cache.contains_key(&key) { return false; }
    owned.cache.insert(key, packet.clone()); out.push(packet); true
}
impl PluginCtx {
    fn ui_player(&self, text: &str) -> Option<[u8;16]> {
        let uuid = parse_uuid(text)?;
        self.shared.players.lock().expect("игроки").ui.info.contains_key(&uuid).then_some(uuid)
    }
    fn edit_ui(&mut self, text: &str, edit: impl FnOnce(&mut Owned, &mut Vec<Vec<u8>>) -> bool) -> bool {
        let Some(uuid) = self.ui_player(text) else { return false; };
        let mut players = self.shared.players.lock().expect("игроки");
        if !players.ui.info.contains_key(&uuid) { return false; }
        let owned = players.ui.owned.entry((self.name.clone(), uuid)).or_default();
        let mut out = Vec::new();
        let result = edit(owned, &mut out);
        for packet in out { send(&mut players, uuid, packet); }
        result
    }
}
impl api::Host for PluginCtx {
    fn get_player_info(&mut self, text: String) -> Option<api::PlayerInfo> {
        let uuid = parse_uuid(&text)?;
        let operator_level = self.shared.ops.lock().expect("операторы").level_of(&uuid) as u8;
        let players = self.shared.players.lock().expect("игроки");
        let member = players.members().iter().find(|m| m.uuid == uuid)?;
        let info = players.ui.info.get(&uuid)?;
        let world = match member.dimension { crate::dimension::Dimension::Overworld => "world", crate::dimension::Dimension::Nether => "world_nether", crate::dimension::Dimension::End => "world_the_end" };
        Some(api::PlayerInfo { ping: info.ping, health: info.health, max_health: info.max_health, world: world.into(), display_name: players.ui.displays.get(&uuid).map_or_else(|| json!(member.name).to_string(), |(_, value)| value.to_string()), food: info.food, level: info.level, exp: info.exp, operator_level })
    }
    fn get_server_info(&mut self) -> api::ServerInfo {
        let (tps, mspt) = self.shared.world_cmd.ticking.lock().expect("такт").metrics();
        api::ServerInfo { tps, mspt, online: self.shared.players.lock().expect("игроки").online() as u32, max_players: self.shared.properties.max_players.max(0) as u32 }
    }
    fn set_tab_header_footer(&mut self, uuid: String, header: String, footer: String) -> bool {
        let (Some(uuid), Some(header), Some(footer)) = (self.ui_player(&uuid), component(&header), component(&footer)) else { return false; };
        let mut players = self.shared.players.lock().expect("игроки");
        if !players.ui.info.contains_key(&uuid) { return false; }
        players.ui.headers.insert(uuid, self.name.clone());
        send(&mut players, uuid, ui::header_packet(&header, &footer)); true
    }
    fn reset_tab_header_footer(&mut self, uuid: String) -> bool {
        let Some(uuid) = self.ui_player(&uuid) else { return false; };
        let mut players = self.shared.players.lock().expect("игроки");
        if !players.ui.info.contains_key(&uuid) { return false; }
        if players.ui.headers.get(&uuid) != Some(&self.name) { return false; }
        players.ui.headers.remove(&uuid);
        send(&mut players, uuid, ui::header_packet(&json!(""), &json!(""))); true
    }
    fn set_display_name(&mut self, uuid: String, text: String) -> bool {
        let (Some(uuid), Some(value)) = (self.ui_player(&uuid), component(&text)) else { return false; };
        let mut players = self.shared.players.lock().expect("игроки");
        if !players.ui.info.contains_key(&uuid) { return false; }
        players.ui.displays.insert(uuid, (self.name.clone(), value)); true
    }
    fn set_player_list_name(&mut self, uuid: String, text: String) -> bool {
        let (Some(uuid), Some(value)) = (self.ui_player(&uuid), component(&text)) else { return false; };
        let mut players = self.shared.players.lock().expect("игроки");
        if !players.ui.info.contains_key(&uuid) { return false; }
        if players.ui.list_names.get(&uuid).is_some_and(|(owner, old)| owner == &self.name && old == &value) { return true; }
        players.ui.list_names.insert(uuid, (self.name.clone(), value.clone()));
        ui::broadcast_list(&mut players, &uuid, Some(&value)); true
    }
    fn set_team(&mut self, uuid: String, value: api::Team) -> bool {
        let (Some(name), Some(prefix), Some(suffix)) = (id(&self.name, &value.id), component(&value.prefix), component(&value.suffix)) else { return false; };
        let color = if value.color == "reset" { board::COLOR_RESET } else if let Some(index) = board::COLORS.iter().position(|c| *c == value.color) { index as i32 } else { return false; };
        let Some(nametag) = board::VISIBILITIES.iter().position(|v| *v == value.visibility) else { return false; };
        if value.members.len() > 1024 || value.members.iter().any(|m| m.len() > 128) { return false; }
        self.edit_ui(&uuid, |owned, out| {
            if owned.teams.len() >= 256 && !owned.teams.contains_key(&name) { return false; }
            let team = board::Team { name: name.clone(), display: json!(""), prefix, suffix, color, nametag, death_messages: 0, collision: 0, friendly_fire: true, see_invisible: false, members: value.members };
            let packet = board::team_packet(&team, 0, &team.members);
            if owned.cache.get(&format!("team:{name}")) == Some(&packet) { return true; }
            if let Some(old) = owned.teams.get(&name) {
                out.push(board::team_packet(&team, 2, &[]));
                let left: Vec<_> = old.members.iter().filter(|m| !team.members.contains(m)).cloned().collect();
                let joined: Vec<_> = team.members.iter().filter(|m| !old.members.contains(m)).cloned().collect();
                if !left.is_empty() { out.push(board::team_packet(&team, 4, &left)); }
                if !joined.is_empty() { out.push(board::team_packet(&team, 3, &joined)); }
            } else { out.push(packet.clone()); }
            owned.cache.insert(format!("team:{name}"), packet);
            owned.teams.insert(name, team); true
        })
    }
    fn remove_team(&mut self, uuid: String, local: String) -> bool {
        let Some(name) = id(&self.name, &local) else { return false; };
        self.edit_ui(&uuid, |owned, out| { if let Some(team) = owned.teams.remove(&name) { out.push(board::team_packet(&team, 1, &[])); } owned.cache.remove(&format!("team:{name}")); true })
    }
    fn set_objective(&mut self, uuid: String, value: api::Objective) -> bool {
        let (Some(name), Some(display)) = (id(&self.name, &value.id), component(&value.title)) else { return false; };
        self.edit_ui(&uuid, |owned, out| {
            if owned.objectives.len() >= 32 && !owned.objectives.contains_key(&name) { return false; }
            let objective = board::Objective { name: name.clone(), display, criteria: "dummy".into(), hearts: value.hearts, auto_update: false, format: None };
            let packet = board::objective_packet(&objective, 2);
            if owned.cache.get(&format!("objective:{name}")) != Some(&packet) {
                out.push(board::objective_packet(&objective, if owned.objectives.contains_key(&name) { 2 } else { 0 }));
                owned.cache.insert(format!("objective:{name}"), packet);
            }
            owned.objectives.insert(name, objective); true
        })
    }
    fn remove_objective(&mut self, uuid: String, local: String) -> bool {
        let Some(name) = id(&self.name, &local) else { return false; };
        let result = self.edit_ui(&uuid, |owned, out| {
            if let Some(objective) = owned.objectives.remove(&name) { out.push(board::objective_packet(&objective, 1)); }
            owned.cache.retain(|key, _| key != &format!("objective:{name}") && !key.starts_with(&format!("score:{name}:"))); true
        });
        if result && let Some(uuid) = parse_uuid(&uuid) {
            self.shared.players.lock().expect("игроки").ui.slots.retain(|(viewer, _), (owner, objective)| *viewer != uuid || owner != &self.name || objective != &name);
        }
        result
    }
    fn set_display_slot(&mut self, uuid: String, slot: String, objective: Option<String>) -> bool {
        let (Some(uuid), Some(slot)) = (self.ui_player(&uuid), board::slot_by_name(&slot)) else { return false; };
        let mut players = self.shared.players.lock().expect("игроки");
        if !players.ui.info.contains_key(&uuid) { return false; }
        let name = if let Some(local) = objective {
            let Some(name) = id(&self.name, &local) else { return false; };
            if !players.ui.owned.get(&(self.name.clone(), uuid)).is_some_and(|o| o.objectives.contains_key(&name)) { return false; }
            if players.ui.slots.get(&(uuid, slot)) == Some(&(self.name.clone(), name.clone())) { return true; }
            players.ui.slots.insert((uuid, slot), (self.name.clone(), name.clone())); name
        } else {
            if !players.ui.slots.get(&(uuid, slot)).is_some_and(|(owner, _)| owner == &self.name) { return false; }
            players.ui.slots.remove(&(uuid, slot)); String::new()
        };
        send(&mut players, uuid, board::display_packet(slot, &name)); true
    }
    fn set_score(&mut self, uuid: String, value: api::Score) -> bool {
        let Some(name) = id(&self.name, &value.objective) else { return false; };
        if value.holder.len() > 128 { return false; }
        let display = match value.display { Some(text) => { let Some(c) = component(&text) else { return false; }; Some(c) }, None => None };
        let format = match value.number { Some(text) => { let Some(c) = component(&text) else { return false; }; Some(NumberFormat::Fixed(c)) }, None => None };
        self.edit_ui(&uuid, |owned, out| {
            if !owned.objectives.contains_key(&name) { return false; }
            let score = board::Score { value: value.value, locked: true, display, format };
            cached(owned, format!("score:{name}:{}", value.holder), board::score_packet(&value.holder, &name, &score), out)
        })
    }
    fn remove_score(&mut self, uuid: String, objective: String, holder: String) -> bool {
        let Some(name) = id(&self.name, &objective) else { return false; };
        if holder.len() > 128 { return false; }
        self.edit_ui(&uuid, |owned, out| {
            if owned.cache.remove(&format!("score:{name}:{holder}")).is_some() {
                let mut packet = encode_varint(0x4f); packet.extend(encode_string(&holder)); packet.push(1); packet.extend(encode_string(&name)); out.push(packet);
            } true
        })
    }
    fn set_boss_bar(&mut self, uuid: String, value: api::BossBar) -> bool {
        let (Some(name), Some(title)) = (id(&self.name, &value.id), component(&value.title)) else { return false; };
        let (Some(color), Some(style)) = (board::BAR_COLORS.iter().position(|c| *c == value.color), board::BAR_STYLES.iter().position(|s| *s == value.style)) else { return false; };
        if !value.progress.is_finite() || !(0.0..=1.0).contains(&value.progress) { return false; }
        self.edit_ui(&uuid, |owned, out| {
            if owned.bars.len() >= 32 && !owned.bars.contains_key(&name) { return false; }
            let bar = board::BossBar { id: name.clone(), uuid: stable_uuid(&format!("ui:{name}")), name: title, color, style, value: (value.progress * 1000000.0) as i32, max: 1000000, visible: true, players: vec![] };
            let packet = bar.add_packet();
            if owned.cache.get(&format!("bar:{name}")) == Some(&packet) { return true; }
            if let Some(old) = owned.bars.get(&name) {
                for action in [2, 3, 4] {
                    if (action == 2 && old.value == bar.value) || (action == 3 && old.name == bar.name) || (action == 4 && old.color == bar.color && old.style == bar.style) { continue; }
                    let mut update = encode_varint(board::BOSS_PACKET); update.extend(bar.uuid); update.extend(encode_varint(action));
                    match action { 2 => update.extend(value.progress.to_be_bytes()), 3 => update.extend(board::encode_component(&bar.name)), _ => { update.extend(encode_varint(color as i32)); update.extend(encode_varint(style as i32)); } }
                    out.push(update);
                }
            } else { out.push(packet.clone()); }
            owned.cache.insert(format!("bar:{name}"), packet); owned.bars.insert(name, bar); true
        })
    }
    fn remove_boss_bar(&mut self, uuid: String, local: String) -> bool {
        let Some(name) = id(&self.name, &local) else { return false; };
        self.edit_ui(&uuid, |owned, out| { if let Some(bar) = owned.bars.remove(&name) { out.push(bar.remove_packet()); } owned.cache.remove(&format!("bar:{name}")); true })
    }
    fn set_action_bar(&mut self, uuid: String, text: String) -> bool {
        let (Some(uuid), Some(value)) = (self.ui_player(&uuid), component(&text)) else { return false; };
        let mut players = self.shared.players.lock().expect("игроки");
        if !players.ui.info.contains_key(&uuid) { return false; }
        players.ui.actions.insert(uuid, self.name.clone()); send(&mut players, uuid, ui::action_packet(&value)); true
    }
    fn send_component(&mut self, uuid: String, text: String) -> bool {
        let (Some(uuid), Some(value)) = (parse_uuid(&uuid), component(&text)) else { return false; };
        let mut players = self.shared.players.lock().expect("игроки");
        if !players.members().iter().any(|m| m.uuid == uuid) { return false; }
        players.order(&uuid, Order::Message { component: board::encode_component(&value) }); true
    }
    fn broadcast_component(&mut self, text: String) -> bool {
        let Some(value) = component(&text) else { return false; };
        self.shared.chat.lock().expect("чат").push_rich(board::plain_text(&value), None, Some(board::encode_component(&value))); true
    }
    fn reset_player(&mut self, uuid: String) -> bool {
        let Some(uuid) = self.ui_player(&uuid) else { return false; };
        ui::reset(&mut self.shared.players.lock().expect("игроки"), &self.name, uuid); true
    }
}

#[cfg(test)]
mod tests {
    use super::component;

    /// Пустая строка из конфига плагина даёт `extra: []` — такой текст
    /// клиент отвергает, поэтому пустые списки выбрасываются.
    #[test]
    fn empty_extra_lists_are_dropped() {
        let value = component(r#"{"text":"","extra":[{"text":"","extra":[]},"\n",{"text":"a","with":[]}]}"#).unwrap();
        assert_eq!(value, serde_json::json!({"text":"","extra":[{"text":""},"\n",{"text":"a"}]}));
    }
}
