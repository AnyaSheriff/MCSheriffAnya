//! Явный перевод базовых компонентов сетевого NBT в JSON340.
//! Стили и вложенные extra/with сохраняются; неизвестные события не теряются молча.
use crate::{items, registry, wire::Error};

fn quoted(out: &mut String, value: &str) {
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""), '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"), '\r' => out.push_str("\\r"), '\t' => out.push_str("\\t"),
            ch if ch < ' ' => out.push_str(&format!("\\u{:04x}", u32::from(ch))),
            ch => out.push(ch),
        }
    }
    out.push('"');
}

// Палитра клиента до RGB. Для исходного именованного цвета результат точный;
// произвольный RGB неизбежно приближается только в отображении старого клиента.
const COLORS: [(&str, u32); 16] = [
    ("black",0x000000),("dark_blue",0x0000aa),("dark_green",0x00aa00),("dark_aqua",0x00aaaa),
    ("dark_red",0xaa0000),("dark_purple",0xaa00aa),("gold",0xffaa00),("gray",0xaaaaaa),
    ("dark_gray",0x555555),("blue",0x5555ff),("green",0x55ff55),("aqua",0x55ffff),
    ("red",0xff5555),("light_purple",0xff55ff),("yellow",0xffff55),("white",0xffffff),
];

fn color(value: &str) -> Result<&str, Error> {
    if COLORS.iter().any(|(name,_)| *name == value) || value == "reset" { return Ok(value); }
    let hex = value.strip_prefix('#').filter(|s| s.len() == 6 && s.bytes().all(|b| b.is_ascii_hexdigit())).ok_or(Error::Invalid)?;
    let rgb = u32::from_str_radix(hex,16).map_err(|_| Error::Invalid)?;
    Ok(COLORS.iter().min_by_key(|(_, candidate)| {
        [0,8,16].into_iter().map(|shift| {
            let delta = ((rgb >> shift) & 255) as i32 - ((*candidate >> shift) & 255) as i32;
            delta * delta
        }).sum::<i32>()
    }).ok_or(Error::Invalid)?.0)
}

fn string(bytes: &[u8]) -> Result<&str, Error> { items::nbt_string(bytes).map_err(Error::Item) }

fn node(kind: u8, bytes: &[u8], depth: usize, out: &mut String) -> Result<(), Error> {
    if depth > 32 || out.len() > 32767 { return Err(Error::TooLarge); }
    match kind {
        8 => quoted(out, string(bytes)?),
        9 => {
            if bytes.len() < 5 { return Err(Error::Truncated); }
            let kind = bytes[0];
            let count = i32::from_be_bytes(bytes[1..5].try_into().map_err(|_| Error::Invalid)?);
            if !(0..=1024).contains(&count) { return Err(Error::TooLarge); }
            let mut rest = &bytes[5..];
            out.push('[');
            for i in 0..count {
                if i != 0 { out.push(','); }
                let size = registry::nbt_payload_length(kind, rest).map_err(|_| Error::Invalid)?;
                node(kind, &rest[..size], depth + 1, out)?; rest = &rest[size..];
            }
            if !rest.is_empty() { return Err(Error::Invalid); }
            out.push(']');
        }
        10 => {
            out.push('{');
            for (index,(name,kind,payload)) in items::fields(bytes).map_err(Error::Item)?.into_iter().enumerate() {
                if index != 0 { out.push(','); }
                quoted(out,&name); out.push(':');
                match (name.as_str(),kind) {
                    ("text"|"translate"|"insertion",8) => quoted(out,string(payload)?),
                    ("color",8) => quoted(out,color(string(payload)?)?),
                    ("bold"|"italic"|"underlined"|"strikethrough"|"obfuscated",1) => {
                        match payload { [0] => out.push_str("false"), [1] => out.push_str("true"), _ => return Err(Error::Invalid) }
                    }
                    ("extra"|"with",9) => node(kind,payload,depth + 1,out)?,
                    _ => return Err(Error::PayloadUnavailable("legacy text component")),
                }
            }
            out.push('}');
        }
        _ => return Err(Error::PayloadUnavailable("legacy text tag")),
    }
    Ok(())
}

pub fn json(bytes: &[u8]) -> Result<String, Error> {
    if bytes.len() > 131072 { return Err(Error::TooLarge); }
    if registry::nbt_length(bytes).map_err(|_| Error::Invalid)? != bytes.len() { return Err(Error::Invalid); }
    let (&kind,payload) = bytes.split_first().ok_or(Error::Truncated)?;
    let mut out = String::new(); node(kind,payload,0,&mut out)?;
    if out.len() > 32767 { return Err(Error::TooLarge); }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn text(value: &str) -> Vec<u8> { let mut out=vec![8]; out.extend((value.len() as u16).to_be_bytes()); out.extend(value.as_bytes()); out }
    #[test]
    fn text_escaping_is_valid_json_and_colors_are_explicitly_projected() {
        let value = "Аня: \"да\"\\\n\0🌟";
        assert_eq!(serde_json::from_str::<String>(&json(&text(value)).unwrap()).unwrap(),value);
        for (name,rgb) in COLORS { assert_eq!(color(&format!("#{rgb:06x}")).unwrap(),name); }
        assert_eq!(color("#fe5656").unwrap(),"red");
        assert!(color("#ffffffx").is_err());
    }
    #[test]
    fn unknown_fields_and_trailing_nbt_are_rejected() {
        let mut bytes=text("abc");bytes.push(0);assert!(json(&bytes).is_err());
        let mut component=vec![10,8,0,4];component.extend(b"font");component.extend(&text("minecraft:default")[1..]);component.push(0);
        assert!(matches!(json(&component),Err(Error::PayloadUnavailable(_))));
    }
}
