// Самостоятельный разбор по публичной документации docs.advntr.dev/minimessage/format.html.
// «end tags aren’t outside of strict mode» — незакрытые теги допустимы.
use serde_json::{Map, Value, json};

pub const COLORS: [&str;16] = ["black","dark_blue","dark_green","dark_aqua","dark_red","dark_purple","gold","gray","dark_gray","blue","green","aqua","red","light_purple","yellow","white"];
const RGB: [u32;16] = [0,0x0000aa,0x00aa00,0x00aaaa,0xaa0000,0xaa00aa,0xffaa00,0xaaaaaa,0x555555,0x5555ff,0x55ff55,0x55ffff,0xff5555,0xff55ff,0xffff55,0xffffff];
fn color(text: &str) -> Option<String> {
    let text = text.to_lowercase().replace("grey","gray");
    if COLORS.contains(&text.as_str()) || (text.len()==7 && text.starts_with('#') && text[1..].bytes().all(|b| b.is_ascii_hexdigit())) { Some(text) } else { None }
}
fn rgb(text: &str) -> Option<u32> { color(text).and_then(|c| if let Some(hex)=c.strip_prefix('#') { u32::from_str_radix(hex,16).ok() } else { COLORS.iter().position(|n| *n==c).map(|i| RGB[i]) }) }
fn decoration(text: &str) -> Option<&'static str> {
    match text { "bold"|"b"=>Some("bold"), "italic"|"i"|"em"=>Some("italic"), "underlined"|"u"=>Some("underlined"), "strikethrough"|"st"=>Some("strikethrough"), "obfuscated"|"obf"=>Some("obfuscated"), _=>None }
}
#[derive(Clone,Default)]
struct Style { fields: Map<String,Value>, paint: Option<usize> }
struct Paint { colors: Vec<u32>, phase: f64, start: usize, end: usize }
fn mix(colors: &[u32], fraction: f64) -> String {
    let position = fraction.clamp(0.0,1.0) * (colors.len()-1) as f64;
    let a = position.floor() as usize;
    let b = (a+1).min(colors.len()-1);
    let part=position-a as f64;
    let mut result=0;
    for shift in [16,8,0] { let first=((colors[a]>>shift)&255) as f64; let last=((colors[b]>>shift)&255) as f64; result |= ((first+(last-first)*part).round() as u32)<<shift; }
    format!("#{result:06x}")
}
fn arguments(text: &str) -> Vec<String> {
    let mut parts=vec![String::new()]; let mut quote=None; let mut escape=false;
    for c in text.chars() {
        if escape { parts.last_mut().unwrap().push(c); escape=false; }
        else if c=='\\' && quote.is_some() { escape=true; }
        else if quote==Some(c) { quote=None; }
        else if quote.is_none() && (c=='\'' || c=='"') { quote=Some(c); }
        else if quote.is_none() && c==':' { parts.push(String::new()); }
        else { parts.last_mut().unwrap().push(c); }
    }
    parts
}
fn tag_end(chars: &[char], start: usize) -> Option<usize> {
    let mut quote=None; let mut escaped=false;
    for (i,&c) in chars.iter().enumerate().skip(start) {
        if escaped { escaped=false; continue; }
        if c=='\\' && quote.is_some() { escaped=true; continue; }
        if quote==Some(c) { quote=None; }
        else if quote.is_none() && (c=='\'' || c=='"') { quote=Some(c); }
        else if quote.is_none() && c=='>' { return Some(i); }
    }
    None
}
pub fn parse(text: &str, mode: &str) -> Value { parse_inner(text,mode,0) }
fn parse_inner(text: &str, mode: &str, depth: usize) -> Value {
    if depth>16 { return json!(text); }
    let chars:Vec<char>=text.chars().take(8192).collect();
    let mut glyphs:Vec<(Value,Style)>=Vec::new(); let mut style=Style::default();
    let mut stack:Vec<(String,Style)>=Vec::new(); let mut paints:Vec<Paint>=Vec::new(); let mut i=0;
    while i<chars.len() {
        let c=chars[i];
        if mode!="legacy" && c=='\\' && chars.get(i+1).is_some_and(|c| *c=='<' || *c=='\\') {
            i+=1; glyphs.push((json!({"text":chars[i].to_string()}),style.clone())); i+=1; continue;
        }
        if mode!="minimessage" && (c=='&' || c=='§') && i+1<chars.len() {
            let code=chars[i+1].to_ascii_lowercase(); let mut hex=None; let mut used=2;
            if code=='#' && i+8<=chars.len() { let value:String=chars[i+1..i+8].iter().collect(); hex=color(&value); used=8; }
            if code=='x' && i+14<=chars.len() && (0..6).all(|n| matches!(chars[i+2+n*2],'&'|'§') && chars[i+3+n*2].is_ascii_hexdigit()) {
                hex=Some(format!("#{}",(0..6).map(|n| chars[i+3+n*2]).collect::<String>())); used=14;
            }
            if let Some(value)=hex.or_else(|| code.to_digit(16).map(|n| COLORS[n as usize].into())) {
                style=Style::default(); style.fields.insert("color".into(),json!(value)); stack.clear(); i+=used; continue;
            }
            if code=='r' { style=Style::default(); stack.clear(); i+=2; continue; }
            if let Some(key)=match code { 'k'=>Some("obfuscated"),'l'=>Some("bold"),'m'=>Some("strikethrough"),'n'=>Some("underlined"),'o'=>Some("italic"),_=>None } { style.fields.insert(key.into(),json!(true)); i+=2; continue; }
        }
        if mode!="legacy" && c=='<' && let Some(end)=tag_end(&chars,i+1) {
            let raw:String=chars[i+1..end].iter().collect();
            let closing=raw.starts_with('/');
            let raw=raw.trim_start_matches('/').trim_end_matches('/');
            let args=arguments(raw); let tag=args[0].to_lowercase();
            let key=decoration(tag.trim_start_matches('!')).unwrap_or(&tag).to_string();
            if closing {
                if let Some(index)=stack.iter().rposition(|(name,_)| name==&key || name==&raw.to_lowercase()) {
                    style=stack[index].1.clone(); stack.truncate(index); i=end+1; continue;
                }
            } else {
                let previous=style.clone(); let mut recognized=true; let arg=|n| args.get(n).map(String::as_str).unwrap_or("");
                if tag=="reset" { style=Style::default(); stack.clear(); i=end+1; continue; }
                if matches!(tag.as_str(),"newline"|"br") { glyphs.push((json!({"text":"\n"}),style.clone())); i=end+1; continue; }
                if let Some(value)=color(&tag).or_else(|| if matches!(tag.as_str(),"color"|"colour"|"c") { color(arg(1)) } else { None }) {
                    style.fields.insert("color".into(),json!(value)); style.paint=None;
                } else if let Some(key)=decoration(tag.trim_start_matches('!')) {
                    style.fields.insert(key.into(), json!(!tag.starts_with('!') && arg(1)!="false"));
                } else if matches!(tag.as_str(),"gradient"|"transition"|"rainbow") {
                    let mut colors:Vec<_>=args.iter().skip(1).filter_map(|s| rgb(s)).collect();
                    let phase=args.last().and_then(|s| s.parse::<f64>().ok()).filter(|f| f.is_finite()).unwrap_or(0.0).clamp(-1.0,1.0);
                    if tag=="rainbow" { colors=vec![0xff0000,0xffff00,0x00ff00,0x00ffff,0x0000ff,0xff00ff,0xff0000]; }
                    if colors.is_empty() { colors=vec![0xffffff,0x000000]; }
                    if colors.len()==1 { colors.push(colors[0]); }
                    if tag=="transition" { style.fields.insert("color".into(),json!(mix(&colors,(phase+1.0)/2.0))); style.paint=None; }
                    else { style.paint=Some(paints.len()); paints.push(Paint { colors, phase, start:glyphs.len(), end:glyphs.len() }); }
                } else if matches!(tag.as_str(),"font"|"insertion") && args.len()>1 { style.fields.insert(tag.clone(),json!(arg(1))); }
                else if tag=="click" && args.len()>2 {
                    let field=match arg(1) { "open_url"=>"url", "run_command"|"suggest_command"=>"command", "copy_to_clipboard"=>"value", "change_page"=>"page", _=>"" };
                    if field.is_empty() { recognized=false; } else { let value=if field=="page" { json!(arg(2).parse::<i32>().unwrap_or(1)) } else { json!(arg(2)) }; style.fields.insert("click_event".into(),json!({"action":arg(1),field:value})); }
                } else if tag=="hover" && arg(1)=="show_text" { style.fields.insert("hover_event".into(),json!({"action":"show_text","value":parse_inner(arg(2),"minimessage",depth+1)})); }
                else if tag=="keybind" && args.len()>1 { glyphs.push((json!({"keybind":arg(1)}),style.clone())); i=end+1; continue; }
                else if matches!(tag.as_str(),"lang"|"translate"|"tr") && args.len()>1 {
                    let with:Vec<_>=args.iter().skip(2).map(|s| parse_inner(s,"minimessage",depth+1)).collect(); glyphs.push((json!({"translate":arg(1),"with":with}),style.clone())); i=end+1; continue;
                } else { recognized=false; }
                if recognized { stack.push((key,previous)); i=end+1; continue; }
            }
        }
        if let Some(paint)=style.paint { paints[paint].end=glyphs.len()+1; }
        glyphs.push((json!({"text":c.to_string()}),style.clone())); i+=1;
    }
    let mut result:Vec<Value>=Vec::new();
    for (index,(mut glyph,style)) in glyphs.into_iter().enumerate() {
        let map=glyph.as_object_mut().unwrap(); map.extend(style.fields);
        if let Some(paint)=style.paint {
            let paint=&paints[paint]; let position=(index-paint.start) as f64 / paint.end.saturating_sub(paint.start+1).max(1) as f64;
            let position=if paint.phase==0.0 { position } else { let p=position+paint.phase.abs(); if p>1.0 {2.0-p} else {p} };
            let colors:Vec<_>=if paint.phase<0.0 {paint.colors.iter().rev().copied().collect()} else {paint.colors.clone()};
            map.insert("color".into(),json!(mix(&colors,position)));
        }
        if let Some(last)=result.last_mut() {
            let mut a=last.as_object().unwrap().clone(); let mut b=map.clone();
            let old=a.remove("text"); let new=b.remove("text");
            if a==b && let (Some(Value::String(old)),Some(Value::String(new)))=(old,new) { last["text"]=json!(old+&new); continue; }
        }
        result.push(glyph);
    }
    json!({"text":"","extra":result})
}
#[cfg(test)]
pub fn plain(value: &Value) -> String {
    match value {
        Value::String(s)=>s.clone(), Value::Array(v)=>v.iter().map(plain).collect(),
        Value::Object(m)=>format!("{}{}",m.get("text").and_then(Value::as_str).unwrap_or(""),m.get("extra").map(plain).unwrap_or_default()), _=>String::new()
    }
}
// JSON-имя возвращается в синтаксис выбранного режима без потери оформления.
pub fn markup(value: &Value, mode: &str) -> String {
    fn visit(value: &Value, inherited: &Map<String,Value>, mode: &str, out: &mut String) {
        match value {
            Value::String(text) => visit(&json!({"text":text}), inherited, mode, out),
            Value::Array(values) => for value in values { visit(value,inherited,mode,out); },
            Value::Object(map) => {
                let mut style=inherited.clone();
                for (key,value) in map { if key!="text" && key!="extra" { style.insert(key.clone(),value.clone()); } }
                if let Some(text)=map.get("text").and_then(Value::as_str) && !text.is_empty() {
                    if mode=="legacy" {
                        out.push_str("§r");
                        if let Some(color)=style.get("color").and_then(Value::as_str) {
                            if let Some(index)=COLORS.iter().position(|c|*c==color) {out.push('§');out.push(char::from_digit(index as u32,16).unwrap());}
                            else if color.starts_with('#') {out.push_str("§x");for c in color.chars().skip(1){out.push('§');out.push(c);}}
                        }
                        for (key,code) in [("bold","§l"),("italic","§o"),("underlined","§n"),("strikethrough","§m"),("obfuscated","§k")] {if style.get(key).and_then(Value::as_bool)==Some(true){out.push_str(code);}}
                        out.push_str(text);out.push_str("§r");
                    } else {
                        out.push_str("<reset>");
                        if let Some(color)=style.get("color").and_then(Value::as_str){out.push_str(&format!("<{color}>"));}
                        for key in ["bold","italic","underlined","strikethrough","obfuscated"] {if let Some(flag)=style.get(key).and_then(Value::as_bool){out.push_str(&format!("<{key}:{flag}>"));}}
                        out.push_str(&text.replace('\\',"\\\\").replace('<',"\\<"));out.push_str("<reset>");
                    }
                }
                if let Some(extra)=map.get("extra"){visit(extra,&style,mode,out);}
            }
            _=>{}
        }
    }
    let mut out=String::new();visit(value,&Map::new(),mode,&mut out);out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nested_styles_restore_and_unicode_gradient_has_endpoints() {
        let p=parse("<red>А<b>Б</b>В</red>Г","auto");
        assert_eq!(plain(&p),"АБВГ"); assert_eq!(p["extra"][1]["bold"],true); assert!(p["extra"][2].get("bold").is_none()); assert!(p["extra"][3].get("color").is_none());
        let p=parse("<gradient:red:#0000ff>Я❤!</gradient>","auto");
        assert_eq!(p["extra"][0]["color"],"#ff5555"); assert_eq!(p["extra"][2]["color"],"#0000ff");
    }
    #[test]
    fn legacy_reset_hex_and_mode_boundaries() {
        let p=parse("&lА§cБ&#00ff00В§x§0§0§0§0§f§fГ&rД","legacy");
        assert_eq!(plain(&p),"АБВГД"); assert_eq!(p["extra"][0]["bold"],true); assert!(p["extra"][1].get("bold").is_none()); assert_eq!(p["extra"][2]["color"],"#00ff00"); assert_eq!(p["extra"][3]["color"],"#0000ff");
        assert_eq!(plain(&parse("<red>&aX","legacy")),"<red>X"); assert_eq!(plain(&parse("<red>&aX","minimessage")),"&aX");
    }
    #[test]
    fn display_name_roundtrip_keeps_decorations_and_gradient() {
        for mode in ["auto","legacy","minimessage"] {
            let value=parse("<gradient:red:blue><bold>Аня</bold></gradient>","auto");
            let again=parse(&markup(&value,mode),mode);
            assert_eq!(plain(&again),"Аня");
            assert_eq!(again["extra"][0]["color"],"#ff5555");
            assert_eq!(again["extra"][0]["bold"],true);
        }
    }
    #[test]
    fn escaped_unknown_and_quoted_tags() {
        assert_eq!(plain(&parse(r"\<red>X<unknown>Y","auto")),"<red>X<unknown>Y");
        let p=parse("<hover:show_text:'<red>A:B'>x</hover><!i>y","auto");
        assert_eq!(p["extra"][0]["hover_event"]["value"]["extra"][0]["text"],"A:B"); assert_eq!(p["extra"][1]["italic"],false);
    }
}
