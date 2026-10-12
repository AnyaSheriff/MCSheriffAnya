use std::{cell::RefCell, collections::HashMap};
use mcsa_sdk::{self as sdk, Plugin, Player, Sender, CommandResult, ChatResult, Verdict, presentation as ui};
use serde_json::json;
use serde_yaml::Value;
use super::{config::{Config,DEFAULT},format,placeholders};

const BATCHES:usize=4;
const FEATURES:[&str;7]=["display-name","nametag","nametag.below-name","tablist","bossbar","actionbar","scoreboard"];
const COMMANDS:[&str;13]=["tablist","tablistreload","tablistplaceholders","tablisttest","tablistformat","tablistworldcolors","tablisttoggle","cprefix","csuffix","cprefixremove","csuffixremove","cnametag","chat"];
thread_local! { static STATE:RefCell<Option<State>>=const {RefCell::new(None)}; }
struct State {
    config:Config,
    last:[[Option<u64>;BATCHES];7],
    worlds:HashMap<String,String>,
    sidebar:HashMap<String,usize>,
    action_line:usize,
    last_chat:HashMap<String,f64>,
    muted:HashMap<String,f64>,
}
fn now()->f64 {std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs_f64()}
fn load()->Result<Config,String> {
    let data=match sdk::storage::read("config.yml")? {Some(data)=>data,None=>{sdk::storage::write("config.yml",DEFAULT.as_bytes())?;DEFAULT.as_bytes().to_vec()}};
    let mut c=Config::parse(std::str::from_utf8(&data).map_err(|e|e.to_string())?)?;
    c.defaults();Ok(c)
}
fn save(c:&Config)->Result<(),String> {sdk::storage::write("config.yml",serde_yaml::to_string(&c.value).map_err(|e|e.to_string())?.as_bytes())}
fn clear() {for p in sdk::server::online_players() {ui::reset_player(&p.uuid);}}
fn render(c:&Config,text:&str,values:&HashMap<String,String>)->String {format::parse(&placeholders::expand(text,values),&c.mode()).to_string()}
fn reply(success:bool,text:impl Into<String>)->CommandResult {CommandResult{success,message:Some(text.into())}}
fn period(c:&Config,feature:&str)->u64 {c.number(&format!("{feature}.update-interval"),20.0).clamp(1.0,72000.0) as u64}
impl State {
    fn new(config:Config)->Self {Self {config,last:[[None;BATCHES];7],worlds:HashMap::new(),sidebar:HashMap::new(),action_line:0,last_chat:HashMap::new(),muted:HashMap::new()}}
    fn reload(&mut self,config:Config) {clear(); self.config=config;self.last=[[None;BATCHES];7];self.worlds.clear();self.sidebar.clear();self.action_line=0;}
    fn update(&mut self,feature:usize,batch:usize) {
        if feature>=FEATURES.len() || batch>=BATCHES{return;}
        let tick=sdk::server::current_tick();
        if (feature>0 || batch>0) && (!self.config.flag(&format!("{}.enabled",FEATURES[feature])) || self.last[feature][batch].is_some_and(|last|tick.saturating_sub(last)<period(&self.config,FEATURES[feature]))) {return;}
        let server=ui::get_server_info();
        let time=now();self.muted.retain(|_,end|*end>time);
        let keep=self.config.number("chat.slow-mode",0.0).max(self.config.number("chat.anti-spam.cooldown",3.0));
        self.last_chat.retain(|_,last|time-*last<keep);
        let online:Vec<_>=sdk::server::online_players().into_iter().filter_map(|p| ui::get_player_info(&p.uuid).map(|i|(p,i))).collect();
        let world_change=online.iter().any(|(p,i)|self.worlds.get(&p.uuid)!=Some(&i.world));
        let force=world_change && self.config.flag("settings.update-on-world-change");
        if force {self.last=[[None;BATCHES];7];}
        for (p,i) in &online {self.worlds.insert(p.uuid.clone(),i.world.clone());}
        let mut due=[false;7];
        for (n,name) in FEATURES.iter().enumerate() {
            due[n]=n==feature && self.config.flag(&format!("{name}.enabled")) && (force || self.last[n][batch].is_none_or(|last|tick.saturating_sub(last)>=period(&self.config,name)));
            if due[n] {self.last[n][batch]=Some(tick);}
        }
        if !due.iter().any(|b|*b) {return;}
        let c=&self.config;
        let recipients:Vec<_>=online.iter().map(|(p,_)|p.uuid.clone()).collect();
        let mut all=Vec::new();
        for (index,(p,info)) in online.into_iter().enumerate() {
            if index%BATCHES!=batch {continue;}
            let mut values=placeholders::values(c,&p,&info,&server);
            if due[0] {
                // %displayname% в собственном формате не должен наращивать префикс каждый такт.
                values.insert("displayname".into(),p.name.clone());
                let display=render(c,&c.text("display-name.format"),&values);
                ui::set_display_name(&p.uuid,&display); ui::set_player_list_name(&p.uuid,&display);
                let plain=format::markup(&serde_json::from_str(&display).unwrap_or(json!(p.name)),&c.mode());
                values.insert("displayname".into(),plain);
            }
            all.push((p,info,values));
        }
        let action_lines=c.lines("actionbar.lines");
        if due[5] && batch==0 && !action_lines.is_empty() && c.flag("actionbar.cycle-lines") {self.action_line=(self.action_line+1)%action_lines.len();}
        // Имя и здоровье принадлежат игроку, а не зрителю. Разбираем их
        // один раз за обновление: рассылка N×N не должна повторять парсер N раз.
        let teams:Vec<_>=if due[1] {all.iter().map(|(p,_,v)| ui::Team {
            id:format!("name:{}",p.uuid), prefix:render(c,&c.text("nametag.prefix-format"),v),suffix:render(c,&c.text("nametag.suffix-format"),v),color:"reset".into(),visibility:if c.flag("nametag.visible-to-all") {"always"} else {"never"}.into(),members:vec![p.name.clone()]
        }).collect()}else{Vec::new()};
        let health:Vec<_>=if due[2] {all.iter().map(|(p,i,v)|ui::Score{
            objective:"health".into(),holder:p.name.clone(),value:i.health.ceil() as i32,display:None,number:Some(render(c,&c.text("nametag.below-name.format"),v))
        }).collect()}else{Vec::new()};
        for viewer in &recipients {
            for team in &teams {ui::set_team(viewer,team);}
            if due[2] {
                ui::set_objective(viewer,&ui::Objective{id:"health".into(),title:json!("").to_string(),hearts:false});
                ui::set_display_slot(viewer,"below_name",Some("health"));
                for score in &health {ui::set_score(viewer,score);}
            }
        }
        for (viewer,info,values) in &all {
            if due[3] {
                let lines=|key:&str| json!({"text":"","extra":c.lines(key).iter().enumerate().flat_map(|(n,line)| {let mut out=Vec::new();if n>0 {out.push(json!({"text":"\n"}));} out.push(serde_json::from_str::<serde_json::Value>(&render(c,line,values)).unwrap());out}).collect::<Vec<_>>()}).to_string();
                ui::set_tab_header_footer(&viewer.uuid,&lines("tablist.header"),&lines("tablist.footer"));
                if c.flag("tablist.hide-playerlist") { ui::set_player_list_name(&viewer.uuid,&json!({"text":" ","color":"black"}).to_string()); }
                else if c.flag("tablist.use-custom-playerlist") {ui::set_player_list_name(&viewer.uuid,&render(c,&c.text("tablist.player-format"),values));}
                else {ui::set_player_list_name(&viewer.uuid,&json!(viewer.name).to_string());}
            }
            if due[4] {
                let style=match c.text("bossbar.style").to_uppercase().as_str() {"SEGMENTED_6"=>"notched_6","SEGMENTED_10"=>"notched_10","SEGMENTED_12"=>"notched_12","SEGMENTED_20"=>"notched_20",_=>"progress"};
                ui::set_boss_bar(&viewer.uuid,&ui::BossBar{id:"main".into(),title:render(c,&c.text("bossbar.format"),values),progress:c.number("bossbar.progress",1.0).clamp(0.0,1.0) as f32,color:c.text("bossbar.color").to_lowercase(),style:style.into()});
            }
            if due[5] && !action_lines.is_empty() {ui::set_action_bar(&viewer.uuid,&render(c,&action_lines[self.action_line%action_lines.len()],values));}
            if due[6] {
                ui::set_objective(&viewer.uuid,&ui::Objective{id:"sidebar".into(),title:render(c,&c.text("scoreboard.title"),values),hearts:false});
                ui::set_display_slot(&viewer.uuid,"sidebar",Some("sidebar"));
                let world_key=format!("scoreboard.worlds.{}",info.world.to_lowercase());
                let lines=if c.flag("scoreboard.use-world-specific") && c.get(&world_key).is_sequence() {c.lines(&world_key)} else {c.lines("scoreboard.lines")};
                let lines:Vec<_>=lines.iter().filter(|s|!s.trim().is_empty()).take(15).collect();
                for (n,line) in lines.iter().enumerate() {ui::set_score(&viewer.uuid,&ui::Score{objective:"sidebar".into(),holder:format!("line{n}"),value:(lines.len()-n) as i32,display:Some(render(c,line,values)),number:Some(json!("").to_string())});}
                let old=self.sidebar.insert(viewer.uuid.clone(),lines.len()).unwrap_or(0);
                for n in lines.len()..old {ui::remove_score(&viewer.uuid,"sidebar",&format!("line{n}"));}
            }
        }
    }
    fn command(&mut self,caller:Sender,name:&str,args:Vec<String>)->Result<String,String> {
        let (name,args)=if name=="tablist" {let Some((sub,args))=args.split_first() else {return Ok("/tablist reload|placeholders|test|format|worldcolors|toggle; /cprefix, /csuffix, /cnametag, /chat".into());};(format!("tablist{}",sub.to_lowercase()),args.to_vec())} else {(name.into(),args)};
        let arg=|n|args.get(n).map(String::as_str).unwrap_or("");
        let mut c=self.config.clone(); let mut changed=true;
        match name.as_str() {
            "tablistreload"=>{let config=load()?;self.reload(config);return Ok("Конфигурация перезагружена".into());}
            "tablistplaceholders"=>return Ok("%player% %displayname% %ping% %world% %cprefix% %csuffix% %health% %health_int% %max_health% %max_health_int% %health_percent% %health_hearts% %health_bar% %online% %max_online% %max_players% %tps% %mspt% %level% %exp% %food% %gamemode% %time% %date% %world_color% %colored_world% и %customtablist_*%".into()),
            "tablisttest"=>{
                let uuid=caller.uuid.ok_or("Команда доступна игроку")?;
                let p=sdk::server::find_player(&caller.name).ok_or("Игрок не найден")?;
                let info=ui::get_player_info(&uuid).ok_or("Нужен Java-клиент")?;
                ui::send_component(&uuid,&render(&c,&args.join(" "),&placeholders::values(&c,&p,&info,&ui::get_server_info())));return Ok(String::new());
            }
            "tablistformat"=>{let mode=match c.text("format.mode").as_str(){"auto"=>"legacy","legacy"=>"minimessage",_=>"auto"};c.set("format.mode",Value::String(mode.into()));c.set("format.use-legacy-format",Value::Bool(false));}
            "tablistworldcolors"=>return serde_yaml::to_string(c.get("world-colors")).map_err(|e|e.to_string()),
            "tablisttoggle"=>{
                let part=match arg(0){"belowname"=>"nametag.below-name","displayname"=>"display-name",x=>x};
                if part!="all" && !FEATURES.contains(&part) && part!="chat" {return Err("/tablist toggle tablist|nametag|belowname|displayname|bossbar|actionbar|scoreboard|chat|all [on|off]".into());}
                if !matches!(arg(1),""|"on"|"off") {return Err("Нужно on или off".into());}
                let enabled=if arg(1).is_empty(){!c.flag(&format!("{}.enabled",if part=="all"{"tablist"}else{part}))}else{arg(1)=="on"};
                for feature in FEATURES.into_iter().chain(["chat"]) {if part=="all" || part==feature {c.set(&format!("{feature}.enabled"),Value::Bool(enabled));}}
            }
            "cprefix"|"csuffix"|"cprefixremove"|"csuffixremove"=>{
                let player=sdk::server::find_player(arg(0)).ok_or("Игрок не найден")?;
                let remove=name.ends_with("remove");if !remove && args.len()<2{return Err("Нужно имя игрока и текст".into());}
                let section=if name.starts_with("cprefix"){"player-prefixes"}else{"player-suffixes"};
                if let Some(map)=c.value[section].as_mapping_mut() {map.retain(|key,_|key.as_str().is_none_or(|key|key.to_lowercase()!=player.name.to_lowercase()));}
                c.set(&format!("{section}.{}",player.name.to_lowercase()),Value::String(if remove{String::new()}else{args[1..].join(" ")}));
            }
            "cnametag"=>match arg(0) {
                "toggle"=>c.set("nametag.enabled",Value::Bool(!c.flag("nametag.enabled"))),
                "prefix"|"suffix" if args.len()>1=>c.set(&format!("nametag.{}-format",arg(0)),Value::String(args[1..].join(" ").trim_matches('"').into())),
                "visibility" if matches!(arg(1),"all"|"self")=>c.set("nametag.visible-to-all",Value::Bool(arg(1)=="all")),
                "below" if arg(1)=="toggle"=>c.set("nametag.below-name.enabled",Value::Bool(!c.flag("nametag.below-name.enabled"))),
                "below" if arg(1)=="text" && args.len()>2=>c.set("nametag.below-name.format",Value::String(args[2..].join(" "))),
                "preview"=>{self.last=[[None;BATCHES];7];return Ok("Оформление обновится на ближайшем такте".into());}
                _=>return Err("/cnametag toggle|prefix|suffix|visibility all|self|below toggle|below text|preview".into())
            },
            "chat"=>match arg(0) {
                "toggle"=>c.set("chat.enabled",Value::Bool(!c.flag("chat.enabled"))),
                "slow"=>{let seconds=arg(1).parse::<u32>().map_err(|_|"Нужно число секунд")?;c.set("chat.slow-mode",Value::Number(seconds.into()));}
                "mute"|"unmute"=>{
                    let player=sdk::server::find_player(arg(1)).ok_or("Игрок не найден")?;
                    if arg(0)=="unmute"{self.muted.remove(&player.uuid);}else{let seconds=duration(arg(2)).ok_or("Время: 30s, 5m, 2h, 1d")?;self.muted.insert(player.uuid,now()+seconds);}
                    changed=false;
                }
                "clear"=>{for _ in 0..100 {ui::broadcast_component(&json!("").to_string());}changed=false;}
                _=>return Err("/chat toggle|slow <секунды>|mute <игрок> <время>|unmute <игрок>|clear".into())
            },
            _=>return Err("Неизвестная команда".into())
        }
        if changed {save(&c)?;self.reload(c);}
        Ok("Готово".into())
    }
    fn chat(&mut self,p:Player,message:String)->ChatResult {
        let c=&self.config;if !c.flag("chat.enabled"){return ChatResult::Pass;}
        let Some(info)=ui::get_player_info(&p.uuid) else{return ChatResult::Pass;};
        let time=now();let last=self.last_chat.get(&p.uuid).copied();
        let cooldown=c.number("chat.slow-mode",0.0).max(if c.flag("chat.anti-spam.enabled"){c.number("chat.anti-spam.cooldown",3.0)}else{0.0});
        let denial=if info.operator_level==0 && self.muted.get(&p.uuid).is_some_and(|end|*end>time){Some("Вы замьючены")}
        else if info.operator_level==0 && last.is_some_and(|last|time-last<cooldown){Some("Не спамьте! Подождите перед следующим сообщением")}
        else if blocked(c,&message){Some("Сообщение не прошло фильтр чата")}else{None};
        if let Some(why)=denial{sdk::player::send_message(&p.uuid,why);return ChatResult::Cancel;}
        self.last_chat.insert(p.uuid.clone(),time);
        let mut values=placeholders::values(c,&p,&info,&ui::get_server_info());
        values.insert("health".into(),(info.health as i32).to_string());
        values.insert("max_health".into(),(info.max_health as i32).to_string());
        values.insert("exp".into(),format!("{:.2}",info.exp));
        // Сообщение игрока вставляется литералом после разбора шаблона: чужие
        // MiniMessage/legacy-теги не меняют формат чата и не создают события click.
        let template=c.text("chat.format");
        let mut extra=Vec::new();for (n,part) in template.split("{message}").enumerate(){if n>0{extra.push(json!({"text":message}));}extra.push(format::parse(&placeholders::expand(part,&values),&c.mode()));}
        if !template.contains("{message}"){extra.push(json!({"text":message}));}
        ui::broadcast_component(&json!({"text":"","extra":extra}).to_string());ChatResult::Cancel
    }
}
fn duration(text:&str)->Option<f64>{let (digits,mult)=match text.chars().last()?{'s'=>(&text[..text.len()-1],1.0),'m'=>(&text[..text.len()-1],60.0),'h'=>(&text[..text.len()-1],3600.0),'d'=>(&text[..text.len()-1],86400.0),_ => (text,1.0)};digits.parse::<u32>().ok().map(|n|n as f64*mult)}
fn blocked(c:&Config,message:&str)->bool {
    let lower=message.to_lowercase();
    if c.lines("chat.blocked-words").iter().any(|word|!word.is_empty() && lower.contains(&word.to_lowercase())){return true;}
    if c.flag("chat.caps-filter.enabled") && message.chars().count()>=c.number("chat.caps-filter.min-length",5.0) as usize {
        let letters=message.chars().filter(|c|c.is_alphabetic()).count();let upper=message.chars().filter(|c|c.is_uppercase()).count();
        if letters>0 && upper as f64*100.0/letters as f64>c.number("chat.caps-filter.max-percent",50.0){return true;}
    }
    if c.flag("chat.ads-filter.enabled") {
        let regex=regex::Regex::new(r"(?:https?://)?(?:www\.)?[-a-z0-9@:%._+~#=]{2,256}\.[a-z]{2,6}\b[-a-z0-9@:%_+.~#?&/=]*").unwrap();
        let allowed=c.lines("chat.ads-filter.allowed-domains");
        if regex.find_iter(&lower).any(|m|!allowed.iter().any(|domain|m.as_str().contains(&domain.to_lowercase()))){return true;}
    }
    false
}
pub struct CustomTabList;
impl Plugin for CustomTabList {
    fn enable() {
        let config=load().unwrap_or_else(|e|{sdk::warn(&format!("CustomTabList: {e}"));panic!("не удалось прочитать config.yml")});
        for name in COMMANDS {sdk::command::register(&sdk::CommandSpec{name:name.into(),description:"Оформление списка игроков".into(),usage:format!("/{name}"),level:2,permission:None}).unwrap();}
        STATE.with(|s|*s.borrow_mut()=Some(State::new(config)));
        // Каждый вид оформления и группа игроков — отдельный бюджет WASM.
        // Первое обновление — ближайший такт, как задачи Bukkit с delay=0.
        for task in 0..FEATURES.len()*BATCHES{sdk::scheduler::run_repeating(1,1,task as u32); }
    }
    fn disable(){clear();STATE.with(|s|*s.borrow_mut()=None);}
    fn on_task(task:u32){STATE.with(|s|{if let Some(s)=s.borrow_mut().as_mut(){s.update(task as usize%FEATURES.len(),task as usize/FEATURES.len());}});}
    fn on_player_join(_:Player){STATE.with(|s|{if let Some(s)=s.borrow_mut().as_mut(){s.last=[[None;BATCHES];7];}});}
    fn on_player_quit(p:Player){STATE.with(|s|{if let Some(s)=s.borrow_mut().as_mut(){s.worlds.remove(&p.uuid);s.sidebar.remove(&p.uuid);for viewer in sdk::server::online_players(){ui::remove_team(&viewer.uuid,&format!("name:{}",p.uuid));ui::remove_score(&viewer.uuid,"health",&p.name);}}});}
    fn on_command(caller:Sender,name:String,args:Vec<String>)->CommandResult {STATE.with(|s|match s.borrow_mut().as_mut().unwrap().command(caller,&name,args){Ok(text)=>reply(true,text),Err(text)=>reply(false,text)})}
    fn on_chat(p:Player,message:String)->ChatResult {STATE.with(|s|s.borrow_mut().as_mut().unwrap().chat(p,message))}
    fn on_player_command(p:Player,line:String)->Verdict {
        let command=line.split_whitespace().next().unwrap_or("").to_ascii_lowercase();
        STATE.with(|s|{if let Some(s)=s.borrow().as_ref() && s.config.flag("chat.log-commands") && !["tell","msg","w","r","me","say"].contains(&command.as_str()){sdk::info(&format!("[Command] {}: {}",p.name,line));}});Verdict::Allow
    }
}
