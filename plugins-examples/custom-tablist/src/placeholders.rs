use std::collections::HashMap;
use mcsa_sdk::{Player, GameMode, presentation::{PlayerInfo,ServerInfo}};
use super::{config::Config,format};

pub fn values(config:&Config,p:&Player,info:&PlayerInfo,server:&ServerInfo)->HashMap<String,String> {
    let prefix=config.player_text("player-prefixes",&p.name);
    let suffix=config.player_text("player-suffixes",&p.name);
    let mut world_color=config.player_text("world-colors",&info.world);
    if world_color.is_empty() {world_color=config.text("world-colors.default");}
    let ratio=if info.max_health>0.0 { (info.health/info.max_health).clamp(0.0,1.0) } else {0.0};
    let hearts=(info.health/2.0).ceil().clamp(0.0,512.0) as usize;
    let max_hearts=(info.max_health/2.0).ceil().clamp(0.0,512.0) as usize;
    let filled=(ratio*10.0).round() as usize;
    let color=if ratio>0.5 {"green"} else if ratio>0.25 {"yellow"} else {"red"};
    let display=serde_json::from_str(&info.display_name).map(|v|format::markup(&v,&config.mode())).unwrap_or_else(|_|p.name.clone());
    let now=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
    let date=chrono::DateTime::from_timestamp(now.as_secs() as i64,0).unwrap_or_default();
    let pairs=[
        ("player",p.name.clone()),("displayname",display),("ping",info.ping.to_string()),("world",info.world.clone()),
        ("cprefix",prefix.clone()),("csuffix",suffix.clone()),("world_color",world_color.clone()),("colored_world",format!("{world_color}{}",info.world)),
        ("health",format!("{:.1}",info.health)),("health_int",info.health.ceil().to_string()),("max_health",format!("{:.1}",info.max_health)),("max_health_int",info.max_health.ceil().to_string()),
        ("health_percent",((ratio*100.0) as i32).to_string()),("health_hearts",format!("{}{}","❤".repeat(hearts),"♡".repeat(max_hearts.saturating_sub(hearts)))),
        ("health_bar",format!("<{color}>{}<gray>{}","█".repeat(filled),"░".repeat(10-filled))),
        ("online",server.online.to_string()),("max_online",server.max_players.to_string()),("max_players",server.max_players.to_string()),("tps",format!("{:.1}",server.tps)),("mspt",format!("{:.1}",server.mspt)),
        ("food",info.food.to_string()),("level",info.level.to_string()),("exp",format!("{:.1}",info.exp)),
        ("gamemode",match p.game_mode {GameMode::Creative=>"CREATIVE",GameMode::Survival=>"SURVIVAL",GameMode::Adventure=>"ADVENTURE",GameMode::Spectator=>"SPECTATOR"}.into()),
        ("time",date.format("%H:%M").to_string()),("date",date.format("%d.%m.%Y").to_string()),
    ];
    let mut values:HashMap<String,String>=pairs.into_iter().map(|(k,v)|(k.into(),v)).collect();
    for (alias,key) in [("prefix","cprefix"),("suffix","csuffix"),("nick","player")] {values.insert(alias.into(),values[key].clone());}
    for key in ["player","nick","ping","world","health","health_int","max_health","max_health_int","health_percent"] { values.insert(format!("customtablist_{key}"),values[key].clone()); }
    values.insert("customtablist_display".into(),values["displayname"].clone());
    values.insert("customtablist_prefix".into(),prefix.clone()); values.insert("customtablist_suffix".into(),suffix.clone());
    values.insert("customtablist_full".into(),format!("{prefix}{}{suffix}",p.name)); values
}
// Один проход: неизвестные %...% остаются; значение не запускает новые замены.
pub fn expand(text:&str,values:&HashMap<String,String>)->String {
    let mut out=String::new(); let mut rest=text;
    while !rest.is_empty() {
        let first=rest.chars().next().unwrap();
        let closing=match first {'%'=>Some('%'),'{'=>Some('}'),_=>None};
        if let Some(closing)=closing && let Some(end)=rest[1..].find(closing).map(|i|i+1) && let Some(value)=values.get(&rest[1..end]) {
            out.push_str(value); rest=&rest[end+1..]; continue;
        }
        let mut replaced=false;
        for key in ["cprefix","csuffix","world_color"] {
            let open=format!("<{key}>"); let close=format!("</{key}>");
            if rest.starts_with(&open) { out.push_str(values.get(key).map(String::as_str).unwrap_or("")); rest=&rest[open.len()..]; replaced=true; break; }
            if rest.starts_with(&close) { rest=&rest[close.len()..]; replaced=true; break; }
        }
        if !replaced {out.push(first);rest=&rest[first.len_utf8()..];}
        if out.len()>32768 {break;}
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_preserved_values_are_not_expanded_again() {
        let values=HashMap::from([("player".into(),"Аня".into()),("cprefix".into(),"%player%".into())]);
        assert_eq!(expand("%cprefix% %player% %missing% {player}",&values),"%player% Аня %missing% Аня");
        assert_eq!(expand("<cprefix></cprefix>",&values),"%player%");
    }
    #[test]
    fn health_server_stats_and_original_aliases() {
        let config=Config::parse(super::super::config::DEFAULT).unwrap();
        let p=Player {uuid:"u".into(),name:"Аня".into(),game_mode:GameMode::Survival,dimension:mcsa_sdk::Dimension::Nether,x:0.0,y:64.0,z:0.0,yaw:0.0,pitch:0.0};
        let info=PlayerInfo {ping:42,health:7.5,max_health:24.0,world:"world_nether".into(),display_name:"\"Аня\"".into(),food:18,level:3,exp:0.5,operator_level:0};
        let server=ServerInfo {tps:18.7,mspt:53.2,online:2,max_players:20};
        let v=values(&config,&p,&info,&server);
        assert_eq!(expand("%health_int%/%max_health_int% %customtablist_health_int% %tps% %mspt% %online%/%max_players% %ping% %world%",&v),"8/24 8 18.7 53.2 2/20 42 world_nether");
        assert_eq!(v["world_color"],"<red>");assert_eq!(v["health_percent"],"31");assert_eq!(v["health_hearts"].chars().count(),12);
    }

}
