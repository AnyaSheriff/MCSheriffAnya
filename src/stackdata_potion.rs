// Свои зелья: wiki, Data component format/potion_contents; разрешённый
// minecraft-data 26.1 ItemPotionEffect. [проверить] Порядок сетевых пакетов.
use super::*;
use crate::survival::{Effect, EffectKind};

#[derive(Clone, PartialEq, Eq, Hash, Debug, Default)]
pub struct PotionExtras {
    pub color: Option<i32>,
    pub name: Option<String>,
    pub effects: Vec<CustomEffect>,
}

#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct CustomEffect {
    pub id: u8,
    pub amplifier: i32,
    pub duration: i32,
    pub ambient: bool,
    pub particles: bool,
    pub icon: bool,
}

impl Data {
    pub fn potion_effects(&self) -> Vec<Effect> {
        let mut effects = self.potion.map(crate::survival::potion_effects).unwrap_or_default();
        if let Some(extra) = &self.potion_extra {
            effects.extend(extra.effects.iter().filter_map(|e| Some(Effect {
                kind: EffectKind::from_id(i32::from(e.id))?, amplifier: e.amplifier,
                ticks_left: e.duration, ambient: e.ambient, visible: e.particles, show_icon: e.icon,
            })));
        }
        effects
    }

    pub fn potion_color(&self) -> i32 {
        self.potion_extra.as_ref().and_then(|e| e.color).unwrap_or_else(|| crate::thrown::potion_color(&self.potion_effects()))
    }
}

pub(super) fn encode(out: &mut Vec<u8>, data: &Data) {
    out.push(u8::from(data.potion.is_some()));
    if let Some(id) = data.potion { out.extend_from_slice(&encode_varint(i32::from(id))); }
    let extra = data.potion_extra.clone().unwrap_or_default();
    out.push(u8::from(extra.color.is_some()));
    if let Some(color) = extra.color { out.extend_from_slice(&color.to_be_bytes()); }
    out.extend_from_slice(&encode_varint(extra.effects.len() as i32));
    for e in &extra.effects {
        for n in [i32::from(e.id), e.amplifier, e.duration] { out.extend_from_slice(&encode_varint(n)); }
        out.extend_from_slice(&[u8::from(e.ambient), u8::from(e.particles), u8::from(e.icon), 0]);
    }
    out.push(u8::from(extra.name.is_some()));
    if let Some(name) = &extra.name { out.extend_from_slice(&encode_string(name)); }
}

pub(super) fn read(reader: &mut Reader, data: &mut Data) -> Option<()> {
    let potion = if reader.take(1)?[0] != 0 {
        Some(reader.varint()?.try_into().ok().filter(|id: &u16| usize::from(*id) < crate::survival::POTIONS.len())?)
    } else { None };
    let color = if reader.take(1)?[0] != 0 { Some(i32::from_be_bytes(reader.take(4)?.try_into().ok()?)) } else { None };
    let count = reader.varint()?;
    if !(0..=256).contains(&count) { return None; }
    let mut effects = Vec::new();
    for _ in 0..count {
        let id = EffectKind::from_id(reader.varint()?)?.id() as u8;
        let amplifier = reader.varint()?.clamp(0, 255);
        let duration = reader.varint()?.max(-1);
        let flags = reader.take(4)?;
        // HiddenEffect не создаётся компонентом custom_effects; повреждённый
        // вход отвергаем целиком, не сдвигая следующую заряженную стрелу.
        if flags[3] != 0 { return None; }
        effects.push(CustomEffect { id, amplifier, duration, ambient: flags[0] != 0, particles: flags[1] != 0, icon: flags[2] != 0 });
    }
    let name = if reader.take(1)?[0] != 0 { Some(reader.string()?) } else { None };
    let extra = PotionExtras { color, name, effects };
    data.potion = potion;
    data.potion_extra = (extra != PotionExtras::default() || potion.is_none()).then_some(extra);
    Some(())
}

pub(super) fn parse(value: &Value, data: &mut Data) -> Result<(), String> {
    if let Some(name) = value.get("potion").and_then(Value::as_str).or_else(|| value.as_str()) {
        let name = name.strip_prefix("minecraft:").unwrap_or(name);
        data.potion = Some(crate::survival::potion_number(name).ok_or_else(|| format!("нет зелья {name}"))?);
    } else if !matches!(value, Value::Map(_)) { return Err("potion_contents — строка или составное значение".into()); }
    let mut extra = PotionExtras::default();
    if let Some(color) = value.get("custom_color") { extra.color = Some(integer(color, 0, 0xFFFFFF)?); }
    if let Some(name) = value.get("custom_name") { extra.name = Some(name.as_str().ok_or("custom_name — строка")?.to_string()); }
    if let Some(list) = value.get("custom_effects") {
        let Value::List(list) = list else { return Err("custom_effects — список".into()); };
        if list.len() > 256 { return Err("слишком много эффектов".into()); }
        for e in list {
            let name = e.get("id").and_then(Value::as_str).ok_or("у эффекта нет id")?;
            let kind = EffectKind::from_name(name.strip_prefix("minecraft:").unwrap_or(name)).ok_or("нет эффекта")?;
            let amplifier = e.get("amplifier").map(|v| integer(v, 0, 255)).transpose()?.unwrap_or(0);
            let duration = e.get("duration").map(|v| integer(v, -1, i32::MAX)).transpose()?.unwrap_or(0);
            let boolean = |key, fallback| -> Result<bool, String> {
                e.get(key).map(|v| match v {
                    Value::Str(s) if s == "true" => Ok(true),
                    Value::Str(s) if s == "false" => Ok(false),
                    _ => integer(v, 0, 1).map(|n| n != 0),
                }).unwrap_or(Ok(fallback))
            };
            let particles = boolean("show_particles", true)?;
            extra.effects.push(CustomEffect { id: kind.id() as u8, amplifier, duration,
                ambient: boolean("ambient", false)?, particles, icon: boolean("show_icon", particles)? });
        }
    }
    data.potion_extra = (extra != PotionExtras::default() || data.potion.is_none()).then_some(extra);
    Ok(())
}

fn integer(value: &Value, min: i32, max: i32) -> Result<i32, String> {
    let number = value.as_f64().ok_or("нужно целое число")?;
    if !number.is_finite() || number.fract() != 0.0 || number < f64::from(min) || number > f64::from(max) { return Err("число вне диапазона".into()); }
    Ok(number as i32)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn custom_potions_round_trip_and_reject_invalid_values() {
        for spec in [r#"potion[potion_contents={custom_effects:[{id:"strength",amplifier:2,duration:801}],custom_color:1193046}]"#,
            r#"potion[potion_contents={potion:"healing",custom_effects:[{id:"poison",duration:-1,ambient:true,show_particles:false,show_icon:true}]}]"#, "potion[potion_contents={}]"
        ] {
            let (_, data) = parse_item_spec(spec).unwrap();
            assert_eq!(from_bytes(&to_bytes(&data)), Some(data.clone()));
            let mut bytes = Vec::new(); encode(&mut bytes, &data);
            let mut decoded = Data::default();
            read(&mut Reader { bytes: &bytes, at: 0 }, &mut decoded).unwrap();
            assert_eq!(decoded, data);
        }
        for field in ["amplifier:-1", "amplifier:256", "amplifier:0.5", "duration:-2", "duration:1.5"] {
            assert!(parse_item_spec(&format!("potion[potion_contents={{custom_effects:[{{id:poison,{field}}}]}}]")).is_err());
        }
        assert!(parse_item_spec("potion[potion_contents={custom_effects:[{id:unknown}]}]").is_err());
    }
}
