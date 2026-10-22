//! Slot1.12.2 (protocol340): short ID, byte count, short Damage, named optionalNBT.
//! JSON и обычные пакеты официального сервера; никаких чужих реализаций.
use crate::{items::{self, Component, Stack, Reader, uint}, legacy_item_ids, registry};
pub const PROTOCOL: i32 = 340;
#[derive(Clone, Copy)]
pub struct Context<'a> { pub enchantments: &'a [&'a str] }
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    Item(items::Error), UnknownItem(u16, u16), MissingItem(u32), UnsupportedNbt(String),
    UnsupportedComponent(u32), UnknownEnchantment(u32), Invalid,
}
impl From<items::Error> for Error { fn from(e: items::Error) -> Self { Self::Item(e) } }

fn data(kind: u32, value: u32) -> Component { let mut out = Vec::new(); uint(&mut out,value); Component { kind, data: out } }
fn word(input: &mut Reader<'_>) -> Result<u16, Error> { Ok(u16::from_be_bytes(input.take(2)?.try_into().map_err(|_| Error::Invalid)?)) }
fn string(out: &mut Vec<u8>, value: &str) -> Result<(), Error> {
    if value.contains('§') || value.chars().any(|c| c == '\0' || u32::from(c) > 0xffff) { return Err(Error::UnsupportedComponent(6)); }
    let n = u16::try_from(value.len()).map_err(|_| Error::Invalid)?;
    out.extend(n.to_be_bytes()); out.extend(value.as_bytes()); Ok(())
}
fn field(out: &mut Vec<u8>, kind: u8, name: &str, payload: &[u8]) -> Result<(), Error> {
    out.push(kind); string(out,name)?; out.extend(payload); Ok(())
}
fn text(value: &str) -> Result<Vec<u8>, Error> { if value.contains('§') || value.chars().any(|c| c == '\0' || u32::from(c) > 0xffff) { return Err(Error::UnsupportedComponent(6)); } let mut out = vec![8]; string(&mut out,value)?; Ok(out) }
fn text_value(bytes: &[u8]) -> Result<&str, Error> {
    if bytes.first() == Some(&8) { return Ok(items::nbt_string(&bytes[1..])?); }
    if bytes.first() == Some(&10) {
        let fields = items::fields(&bytes[1..])?;
        if fields.len() == 1 && fields[0].0 == "text" && fields[0].1 == 8 { return Ok(items::nbt_string(fields[0].2)?); }
    }
    Err(Error::UnsupportedComponent(6))
}

pub fn decode(bytes: &[u8], context: Context<'_>) -> Result<(Option<Stack>, usize), Error> {
    if bytes.len() > items::LIMIT { return Err(items::Error::TooLarge.into()); }
    let mut input = Reader(bytes); let old = word(&mut input)?;
    if old == u16::MAX { return Ok((None,2)); }
    let count = input.take(1)?[0]; if !(1..=127).contains(&count) { return Err(Error::Invalid); }
    let damage = word(&mut input)?; if damage > i16::MAX as u16 { return Err(Error::Invalid); }
    // Для карты Damage — номер карты, для инструментов — износ, иначе вариант.
    let row = legacy_item_ids::ITEMS.iter().find(|r| r.0 == old && (r.3 || old == 358 || r.1 == damage))
        .ok_or(Error::UnknownItem(old,damage))?;
    let mut stack = Stack { item: row.2, count: u32::from(count), added: Vec::new(), removed: Vec::new() };
    if row.3 && damage != 0 { stack.added.push(data(3,u32::from(damage))); }
    if old == 358 { stack.added.push(data(46,u32::from(damage))); }
    let root = input.take(1)?[0];
    if root != 0 {
        if root != 10 { return Err(Error::Invalid); }
        let name_size = word(&mut input)? as usize; input.take(name_size)?;
        let n = registry::nbt_payload_length(10,input.0).map_err(items::Error::Nbt)?;
        for (name,kind,value) in items::fields(input.take(n)?)? {
            match (name.as_str(),kind) {
                ("Unbreakable",1) if matches!(value,[0] | [1]) => { if value == [1] { stack.added.push(Component { kind:4,data:vec![] }); } }
                ("RepairCost",3) => {
                    let n = i32::from_be_bytes(value.try_into().map_err(|_| Error::Invalid)?);
                    if n < 0 { return Err(Error::Invalid); } stack.added.push(data(19,n as u32));
                }
                ("ench" | "StoredEnchantments",9) => {
                    let mut src = Reader(value); let element = src.take(1)?[0];
                    let n = u32::from_be_bytes(src.take(4)?.try_into().map_err(|_| Error::Invalid)?) as usize;
                    if n > 256 || (element != 10 && !(element == 0 && n == 0)) { return Err(Error::Invalid); }
                    let mut out = Vec::new(); uint(&mut out,n as u32); let mut seen = std::collections::BTreeSet::new();
                    for _ in 0..n {
                        let size = registry::nbt_payload_length(10,src.0).map_err(items::Error::Nbt)?;
                        let fields = items::fields(src.take(size)?)?;
                        let mut id = None; let mut level = None;
                        for (name,kind,value) in fields {
                            if kind != 2 { return Err(Error::Invalid); }
                            let v = u16::from_be_bytes(value.try_into().map_err(|_| Error::Invalid)?);
                            match name.as_str() { "id" => id = Some(v), "lvl" => level = Some(v), _ => return Err(Error::UnsupportedNbt(name)) }
                        }
                        let id = id.ok_or(Error::Invalid)?; let level = level.ok_or(Error::Invalid)?;
                        let name = legacy_item_ids::ENCHANTMENTS.iter().find(|r| r.0 == id).ok_or(Error::UnknownEnchantment(u32::from(id)))?.1;
                        let modern = context.enchantments.iter().position(|&n| n == name).ok_or(Error::UnknownEnchantment(u32::from(id)))?;
                        if level > 255 || !seen.insert(modern) { return Err(Error::Invalid); }
                        uint(&mut out,modern as u32); uint(&mut out,u32::from(level));
                    }
                    if !src.0.is_empty() { return Err(Error::Invalid); }
                    stack.added.push(Component { kind:if name == "ench" {13} else {42}, data:out });
                }
                ("display",10) => {
                    for (name,kind,value) in items::fields(value)? {
                        match (name.as_str(),kind) {
                            ("Name",8) => stack.added.push(Component {kind:6,data:text(items::nbt_string(value)?)?}),
                            ("color",3) => stack.added.push(Component {kind:44,data:value.to_vec()}),
                            ("Lore",9) => {
                                let mut src = Reader(value); let element = src.take(1)?[0];
                                let n = u32::from_be_bytes(src.take(4)?.try_into().map_err(|_| Error::Invalid)?) as usize;
                                if n > 256 || (element != 8 && !(element == 0 && n == 0)) { return Err(Error::Invalid); }
                                let mut out = Vec::new(); uint(&mut out,n as u32);
                                for _ in 0..n {
                                    let size = registry::nbt_payload_length(8,src.0).map_err(items::Error::Nbt)?;
                                    let value = items::nbt_string(src.take(size)?)?;
                                    out.extend(text(value)?);
                                }
                                if !src.0.is_empty() { return Err(Error::Invalid); }
                                stack.added.push(Component {kind:11,data:out});
                            }
                            _ => return Err(Error::UnsupportedNbt(format!("display.{name}"))),
                        }
                    }
                }
                _ => return Err(Error::UnsupportedNbt(name)),
            }
        }
    }
    items::encode(crate::JavaVersion::V26_1_2,Some(&stack),false,items::Context {shared_enchantment_count:Some(context.enchantments.len() as u32), ..Default::default() })?;
    Ok((Some(stack),bytes.len()-input.0.len()))
}

pub fn encode(stack: Option<&Stack>, context: Context<'_>) -> Result<Vec<u8>, Error> {
    let Some(stack) = stack else { return Ok(vec![255,255]); };
    // Сначала валидируем общий формат, включая дубликаты и границы компонентов.
    items::encode(crate::JavaVersion::V26_1_2,Some(stack),false,items::Context {
        shared_enchantment_count: Some(context.enchantments.len() as u32), ..Default::default() })?;
    if !stack.removed.is_empty() { return Err(Error::Invalid); }
    let row = legacy_item_ids::ITEMS.iter().find(|r|r.2 == stack.item).ok_or(Error::MissingItem(stack.item))?;
    if row.0 == 358 && !stack.added.iter().any(|c| c.kind == 46) { return Err(Error::Invalid); }
    let mut damage = row.1; let mut root = Vec::new(); let mut display = Vec::new();
    for component in &stack.added {
        let mut src = Reader(&component.data);
        match component.kind {
            3 if row.3 => damage = u16::try_from(src.uint()?).map_err(|_| Error::Invalid)?,
            46 if row.0 == 358 => damage = u16::try_from(src.uint()?).map_err(|_| Error::Invalid)?,
            4 => field(&mut root,1,"Unbreakable",&[1])?,
            19 => field(&mut root,3,"RepairCost",&(src.uint()? as i32).to_be_bytes())?,
            6 => { let mut value = Vec::new(); string(&mut value,text_value(&component.data)?)?; field(&mut display,8,"Name",&value)?; }
            44 => field(&mut display,3,"color",&component.data)?,
            11 => {
                let n = src.count(256)?; let mut value = vec![8]; value.extend((n as i32).to_be_bytes());
                for _ in 0..n {
                    let size = registry::nbt_length(src.0).map_err(items::Error::Nbt)?;
                    string(&mut value,text_value(src.take(size)?)?)?;
                }
                field(&mut display,9,"Lore",&value)?;
            }
            13 | 42 => {
                let n = src.count(256)?; let mut value = vec![10]; value.extend((n as i32).to_be_bytes());
                for _ in 0..n {
                    let id = src.uint()?; let level = src.uint()?;
                    let name = context.enchantments.get(id as usize).ok_or(Error::UnknownEnchantment(id))?;
                    let old = legacy_item_ids::ENCHANTMENTS.iter().find(|r|r.1 == *name).ok_or(Error::UnknownEnchantment(id))?.0;
                    field(&mut value,2,"id",&old.to_be_bytes())?;
                    field(&mut value,2,"lvl",&(level as u16).to_be_bytes())?; value.push(0);
                }
                field(&mut root,9,if component.kind == 13 {"ench"} else {"StoredEnchantments"},&value)?;
            }
            _ => return Err(Error::UnsupportedComponent(component.kind)),
        }
    }
    if damage > i16::MAX as u16 { return Err(Error::Invalid); }
    if !display.is_empty() { display.push(0); field(&mut root,10,"display",&display)?; }
    let mut out = Vec::new(); out.extend(row.0.to_be_bytes()); out.push(stack.count as u8); out.extend(damage.to_be_bytes());
    if root.is_empty() { out.push(0); } else { out.extend([10,0,0]); out.extend(root); out.push(0); }
    if out.len() > items::LIMIT { return Err(items::Error::TooLarge.into()); }
    Ok(out)
}
