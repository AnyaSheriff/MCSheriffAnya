// Изменение только через очередь подключения, без выполнения команд от консоли.
use super::api::{PluginCtx,parse_uuid,mcsa::api::attributes as api};
use crate::{attributes,commands_world::{self,AttributeEdit}};

fn target(uuid: &str, name: &str) -> Result<([u8;16],i32),String> {
    Ok((parse_uuid(uuid).ok_or("неверный UUID")?,attributes::by_name(name).ok_or("неизвестный атрибут")?))
}
fn modifier_id(owner: &str, id: &str) -> bool {
    id.strip_prefix(owner).and_then(|rest|rest.strip_prefix(':')).is_some_and(|local| !local.is_empty() && local.len()<=128 && local.bytes().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || b"._-/".contains(&c)))
}
impl api::Host for PluginCtx {
    fn get(&mut self, uuid: String, name: String) -> Result<api::Snapshot,String> {
        let (uuid,attribute)=target(&uuid,&name)?;
        let entry=commands_world::plugin_attribute(&self.shared,&uuid,attribute).ok_or("игрок или атрибут отсутствует")?;
        Ok(api::Snapshot {base:entry.base,value:entry.value(),modifiers:entry.modifiers.iter().map(|modifier| api::Modifier {id:modifier.id.clone(),amount:modifier.amount,operation:modifier.operation as u8}).collect()})
    }
    fn set_base(&mut self, uuid: String, name: String, value: f64) -> Result<(),String> {
        let (uuid,attribute)=target(&uuid,&name)?;
        if !value.is_finite() || value.abs()>1_000_000.0 {return Err("основа атрибута должна быть конечной, по модулю до 1000000".into());}
        commands_world::plugin_attribute_edit(&self.shared,&uuid,AttributeEdit::SetBase {attribute,value})
    }
    fn add(&mut self, uuid: String, name: String, modifier: api::Modifier) -> Result<(),String> {
        let (uuid,attribute)=target(&uuid,&name)?;
        if !modifier_id(&self.name,&modifier.id) || !modifier.amount.is_finite() || modifier.amount.abs()>1_000_000.0 {return Err("модификатор должен принадлежать плагину и иметь конечную величину".into());}
        let operation=attributes::Operation::from_number(i32::from(modifier.operation)).ok_or("неверная операция")?;
        let entry=commands_world::plugin_attribute(&self.shared,&uuid,attribute).ok_or("игрок или атрибут отсутствует")?;
        if entry.modifiers.len()>=256 || entry.modifiers.iter().any(|known|known.id==modifier.id) {return Err("слишком много модификаторов или ключ уже занят".into());}
        commands_world::plugin_attribute_edit(&self.shared,&uuid,AttributeEdit::AddModifier {attribute,modifier:attributes::Modifier::new(&modifier.id,modifier.amount,operation)})
    }
    fn remove(&mut self, uuid: String, name: String, id: String) -> Result<(),String> {
        let (uuid,attribute)=target(&uuid,&name)?;
        if !modifier_id(&self.name,&id) {return Err("модификатор принадлежит другому владельцу".into());}
        commands_world::plugin_attribute_edit(&self.shared,&uuid,AttributeEdit::RemoveModifier {attribute,id})
    }
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn modifier_keys_are_owned_and_bounded(){assert!(modifier_id("fourlegacy","fourlegacy:race/speed"));for key in ["minecraft:sprinting","fourlegacy-other:key","fourlegacy:","FourLegacy:key"]{assert!(!modifier_id("fourlegacy",key),"{key}");}}
}
