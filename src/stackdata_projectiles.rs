//! Список зарядов сохраняется целиком: тип, количество, довески и запрет подбора.
//! Формат ItemStackTemplate — protocol.json; поведение сверено обычными
//! командами на оригиналах 775/777 (verification-crossbow-projectiles).
use super::*;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ChargedProjectile {
    pub content: Content,
    pub intangible: bool,
}

pub fn loaded(data: &Data) -> Vec<ChargedProjectile> {
    if !data.charged_projectiles.is_empty() { return data.charged_projectiles.clone(); }
    let content = if let Some(rocket) = &data.charged_rocket {
        let mut extra = Data::default();
        extra.set_rocket(rocket.clone());
        Content { slot: 0, item: crate::blocks::item_named("firework_rocket").expect("ракета в таблице"), count: 1, data: intern(extra) }
    } else {
        data.charged_arrow.unwrap_or(Content { slot: 0, item: crate::blocks::item_named("arrow").expect("стрела в таблице"), count: 1, data: None })
    };
    (0..data.charged).map(|index| ChargedProjectile { content, intangible: index > 0 }).collect()
}

fn assign(data: &mut Data, projectiles: Vec<ChargedProjectile>) {
    data.charged = projectiles.len().min(255) as u8;
    data.charged_arrow = None;
    data.charged_rocket = None;
    data.charged_projectiles.clear();
    if let Some(first) = projectiles.first() {
        if crate::blocks::item_name(first.content.item) == Some("firework_rocket") {
            data.charged_rocket = Some(first.content.data.map(get).unwrap_or_default().rocket());
        } else { data.charged_arrow = Some(Content { count: 1, ..first.content }); }
    }
    // Старый однородный формат остаётся читаемым и не меняет сохранённые стопки.
    if loaded(data) != projectiles { data.charged_projectiles = projectiles; }
}

fn supported(item: i32) -> bool {
    matches!(crate::blocks::item_name(item), Some("arrow" | "tipped_arrow" | "spectral_arrow" | "firework_rocket"))
}

pub fn encode(data: &Data) -> Vec<u8> {
    let projectiles = loaded(data);
    let mut out = encode_varint(projectiles.len() as i32);
    for projectile in projectiles {
        let content = projectile.content;
        let extra = content.data.map(get).unwrap_or_default();
        let (added, bytes) = encode_components(&extra);
        out.extend(encode_varint(content.item));
        out.extend(encode_varint(content.count));
        out.extend(encode_varint(added + i32::from(projectile.intangible)));
        out.push(0);
        out.extend(bytes);
        if projectile.intangible {
            out.extend(encode_varint(COMPONENT_INTANGIBLE_PROJECTILE));
            out.extend([10, 0]);
        }
    }
    out
}

pub(super) fn read(reader: &mut Reader<'_>, data: &mut Data, depth: usize) -> Option<()> {
    if depth >= 16 { return None; }
    // Ограничение нашего декодера, не объявление лимита игры.
    let size = reader.varint().filter(|n| (0..=256).contains(n))?;
    let mut projectiles = Vec::new();
    for _ in 0..size {
        let (item, count, added, removed) = (reader.varint()?, reader.varint()?, reader.varint()?, reader.varint()?);
        if !supported(item) || !(1..=127).contains(&count) || !(0..=256).contains(&added) || removed != 0 { return None; }
        let mut extra = Data::default();
        let mut intangible = false;
        let mut seen = std::collections::BTreeSet::new();
        for _ in 0..added {
            let kind = reader.varint()?;
            if !seen.insert(kind) { return None; }
            if kind == COMPONENT_INTANGIBLE_PROJECTILE {
                if reader.take(2)? != [10, 0] { return None; }
                intangible = true;
            } else if kind == COMPONENT_POTION_CONTENTS {
                potion::read(reader, &mut extra)?;
            } else if kind == COMPONENT_FIREWORKS {
                extra.set_rocket(read_fireworks(reader)?);
            } else if !read_component_cursor(kind, reader, &mut extra, depth + 1) { return None; }
        }
        projectiles.push(ChargedProjectile { content: Content { slot: 0, item, count, data: intern(extra) }, intangible });
    }
    assign(data, projectiles);
    Some(())
}

pub fn read_body(body: &[u8], data: &mut Data) -> bool {
    let mut reader = Reader { bytes: body, at: 0, failed: false };
    let mut parsed = Data::default();
    if read(&mut reader, &mut parsed, 0).is_none() || reader.failed || reader.at != body.len()
        || to_bytes(&parsed).len() > u16::MAX as usize { return false; }
    data.charged = parsed.charged;
    data.charged_arrow = parsed.charged_arrow;
    data.charged_rocket = parsed.charged_rocket;
    data.charged_projectiles = parsed.charged_projectiles;
    true
}

pub fn parse(value: &Value, data: &mut Data) -> Result<(), String> {
    let Value::List(entries) = value else { return Err("заряды арбалета — список".into()); };
    if entries.len() > 256 { return Err("поддерживается не более 256 зарядов".into()); }
    let mut projectiles = Vec::new();
    for entry in entries {
        let mut entry = entry.clone();
        let mut intangible = false;
        if entry.get("components").is_some_and(|v| !matches!(v, Value::Map(_))) {
            return Err("компоненты заряда — объект".into());
        }
        if let Value::Map(fields) = &mut entry
            && let Some((_, Value::Map(components))) = fields.iter_mut().find(|(key, _)| key == "components") {
            let mut seen = std::collections::BTreeSet::new();
            for (key, value) in components.iter() {
                let key = key.strip_prefix("minecraft:").unwrap_or(key);
                if !seen.insert(key) { return Err("повтор компонента заряда".into()); }
                if key == "intangible_projectile" {
                    if !matches!(value, Value::Map(fields) if fields.is_empty()) { return Err("intangible_projectile — пустой объект".into()); }
                    intangible = true;
                } else if !matches!(key, "potion_contents" | "fireworks" | "custom_name" | "damage" | "unbreakable" | "enchantments" | "attribute_modifiers" | "charged_projectiles" | "container" | "bundle_contents") {
                    return Err(format!("неподдержанный компонент заряда: {key}"));
                }
            }
            components.retain(|(key, _)| key.strip_prefix("minecraft:").unwrap_or(key) != "intangible_projectile");
        }
        if let Some(count) = entry.get("count").or_else(|| entry.get("Count"))
            && !count.as_f64().is_some_and(|n| (1.0..=127.0).contains(&n) && n.fract() == 0.0) { return Err("количество снарядов — целое число 1..127".into()); }
        let text = format!("{{Item:{}}}", crate::commands::component_text(&entry));
        let stack = crate::commands::item_from_text(&text).ok_or("неверный заряд арбалета")?;
        if !supported(stack.item) { return Err("поддерживаются стрелы и ракеты".into()); }
        projectiles.push(ChargedProjectile { content: Content { slot: 0, item: stack.item, count: stack.count, data: stack.data }, intangible });
    }
    assign(data, projectiles);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observed_charges_survive_command_wire_and_file_without_collapsing() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!("../tools/protocol-bridge/testdata/crossbow-observations.json")).unwrap();
        for case in fixture["cases"].as_array().unwrap() {
            let data = parse_item_spec(case["command"].as_str().unwrap()).unwrap().1;
            assert_eq!(loaded(&data).len(), case["before"]["projectiles"].as_array().unwrap().len());
            let saved = to_bytes(&data);
            assert_eq!(from_bytes(&saved), Some(data.clone()), "{}", case["name"]);
            let body = encode(&data);
            let mut decoded = Data::default();
            assert!(read_body(&body, &mut decoded));
            assert_eq!(loaded(&decoded), loaded(&data), "{}", case["name"]);
            assert_eq!(encode(&decoded), body);
            for n in 0..body.len() {
                let mut existing = data.clone();
                assert!(!read_body(&body[..n], &mut existing));
                assert_eq!(existing, data, "неудачное чтение не меняет заряд");
            }
            let mut trailing = body.clone(); trailing.push(0);
            assert!(!read_body(&trailing, &mut decoded));
        }
    }

    #[test]
    fn charged_input_rejects_loss_duplicates_bad_counts_and_deep_nesting() {
        for item in [r#"{id:"arrow",count:0}"#, r#"{id:"arrow",count:1.5}"#, r#"{id:"stone"}"#,
            r#"{id:"arrow",components:1}"#, r#"{id:"arrow",components:{unknown:{}}}"#, r#"{id:"arrow",components:{intangible_projectile:1}}"#,
            r#"{id:"arrow",components:{intangible_projectile:{},"minecraft:intangible_projectile":{}}}"#] {
            assert!(parse_item_spec(&format!("crossbow[charged_projectiles=[{item}]]")).is_err(), "{item}");
        }
        let item = crate::blocks::item_named("arrow").unwrap();
        let template = |count, added, removed, components: &[u8]| {
            let mut body = vec![1]; body.extend(encode_varint(item));
            body.extend(encode_varint(count)); body.extend(encode_varint(added)); body.extend(encode_varint(removed)); body.extend(components); body
        };
        for body in [template(0,0,0,&[]), template(128,0,0,&[]), template(1,0,1,&[3]),
            template(1,2,0,&[22,10,0,22,10,0]), template(1,1,0,&[22,8,0]), template(1,1,0,&[51,1])] {
            assert!(!read_body(&body, &mut Data::default()));
        }
        let mut body = vec![0];
        for _ in 0..16 {
            let mut component = vec![49]; component.extend(body); body = template(1,1,0,&component);
        }
        assert!(!read_body(&body, &mut Data::default()));
        let mut data = parse_item_spec("crossbow[charged_projectiles=[{id:arrow}]]").unwrap().1;
        assert!(read_body(&[0], &mut data)); assert_eq!(data, Data::default());
    }
}
