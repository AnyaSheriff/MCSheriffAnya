// Атрибуты экземпляра: пустой список оставляет прежнюю физику без изменений.
use super::{Kind,Mob};
use crate::{attributes::{self,Attribute,Modifier,Operation},commands_world::AttributeEdit};

impl Mob {
    fn native_attack_damage(&self) -> f64 {
        f64::from(match self.kind {
            Kind::Spider|Kind::CaveSpider|Kind::Skeleton|Kind::Stray|Kind::Bogged|Kind::Endermite => 2.0,
            Kind::Silverfish => 1.0,
            Kind::Enderman => 7.0,
            Kind::WitherSkeleton|Kind::ZombifiedPiglin|Kind::Piglin|Kind::PiglinBrute|Kind::Hoglin|Kind::Blaze|Kind::MagmaCube => super::nether::melee_damage(self),
            Kind::Vindicator|Kind::Vex|Kind::Ravager => super::illager::melee_damage(self.kind,2),
            Kind::Warden => super::warden::MELEE_DAMAGE,
            Kind::Slime => self.nether.size as f32,
            _ => 3.0,
        })
    }
    pub fn plugin_attribute(&self,id:i32) -> Option<Attribute> {
        let base=match id {
            attributes::MAX_HEALTH => f64::from(self.vitals.max_health),
            attributes::ATTACK_DAMAGE => self.native_attack_damage(),
            attributes::MOVEMENT_SPEED => if super::horse::equine(self.kind) {self.horse.speed} else {self.kind.speed()},
            _ => return None,
        };
        Some(self.plugin_attributes.iter().find(|a|a.id==id).cloned().unwrap_or(Attribute {id,base,modifiers:Vec::new()}))
    }
    pub fn plugin_attribute_value(&self,id:i32,native:f64) -> f64 {
        self.plugin_attributes.iter().find(|a|a.id==id).map_or(native,Attribute::value)
    }
    // Случайный разброс урона и сложность сохраняются; меняется атрибутная часть.
    pub fn plugin_attack_damage(&self,native:f32) -> f32 {
        let Some(attribute)=self.plugin_attributes.iter().find(|a|a.id==attributes::ATTACK_DAMAGE) else {return native;};
        let base=self.native_attack_damage();
        if base>0.0 {native*(attribute.value()/base) as f32} else {attribute.value() as f32}
    }
    pub fn edit_plugin_attribute(&mut self,edit:&AttributeEdit) -> Result<(),String> {
        let id=crate::commands_world::edited_attribute(edit);
        let mut attribute=self.plugin_attribute(id).ok_or("Атрибут отсутствует у моба")?;
        match edit {
            AttributeEdit::SetBase {value,..} => attribute.base= *value,
            AttributeEdit::ResetBase {..} => return Err("Сброс основы моба не поддерживается".into()),
            AttributeEdit::AddModifier {modifier,..} => {attribute.modifiers.retain(|m|m.id!=modifier.id);attribute.modifiers.push(modifier.clone());},
            AttributeEdit::RemoveModifier {id,..} => attribute.modifiers.retain(|m|m.id!=*id),
        }
        if id==attributes::MAX_HEALTH {self.vitals.max_health=attribute.value() as f32;self.vitals.health=self.vitals.health.min(self.vitals.max_health);}
        self.plugin_attributes.retain(|a|a.id!=id);self.plugin_attributes.push(attribute);
        self.attr_changes=self.attr_changes.wrapping_add(1);
        Ok(())
    }
}

pub(super) fn write(mob:&Mob,out:&mut Vec<u8>) {
    out.extend(mob.uuid);out.push(mob.plugin_attributes.len() as u8);
    for a in &mob.plugin_attributes {
        out.extend(a.id.to_be_bytes());out.extend(a.base.to_be_bytes());out.extend((a.modifiers.len() as u16).to_be_bytes());
        for m in &a.modifiers {out.extend((m.id.len() as u16).to_be_bytes());out.extend(m.id.as_bytes());out.extend(m.amount.to_be_bytes());out.push(m.operation as u8);}
    }
}
pub(super) fn read(bytes:&mut &[u8]) -> Option<Vec<Attribute>> {
    fn take<'a>(bytes:&mut &'a [u8],n:usize)->Option<&'a [u8]> {let (head,rest)=bytes.split_at_checked(n)?;*bytes=rest;Some(head)}
    // UUID читает обёртка хвоста, до этого списка.
    let count=take(bytes,1)?[0];if count>3 {return None;}
    let mut result=Vec::new();
    for _ in 0..count {
        let id=i32::from_be_bytes(take(bytes,4)?.try_into().ok()?);
        if !matches!(id,attributes::MAX_HEALTH|attributes::ATTACK_DAMAGE|attributes::MOVEMENT_SPEED)||result.iter().any(|a:&Attribute|a.id==id) {return None;}
        let base=f64::from_be_bytes(take(bytes,8)?.try_into().ok()?);if !base.is_finite() {return None;}
        let count=u16::from_be_bytes(take(bytes,2)?.try_into().ok()?);if count>256 {return None;}
        let mut modifiers=Vec::new();
        for _ in 0..count {
            let len=u16::from_be_bytes(take(bytes,2)?.try_into().ok()?) as usize;if len>256 {return None;}
            let key=String::from_utf8(take(bytes,len)?.to_vec()).ok()?;
            let amount=f64::from_be_bytes(take(bytes,8)?.try_into().ok()?);if !amount.is_finite()||modifiers.iter().any(|m:&Modifier|m.id==key) {return None;}
            let operation=Operation::from_number(i32::from(take(bytes,1)?[0]))?;modifiers.push(Modifier::new(&key,amount,operation));
        }
        result.push(Attribute {id,base,modifiers});
    }
    Some(result)
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn buffs_are_per_mob_and_survive_region_roundtrip() {
        let mut mob=Mob::new(1,Kind::Zombie,(0.0,64.0,0.0));let vanilla=mob.clone();
        for id in [attributes::MAX_HEALTH,attributes::ATTACK_DAMAGE] {mob.edit_plugin_attribute(&AttributeEdit::AddModifier {attribute:id,modifier:Modifier::new("fourlegacy:world_buff",0.5,Operation::AddMultipliedBase)}).unwrap();}
        assert_eq!(mob.vitals.max_health,30.0);mob.vitals.health=30.0;assert_eq!(mob.effect_melee_damage(3.0),4.5);
        let mut bytes=Vec::new();mob.write(&mut bytes);let mut input=bytes.as_slice();let saved=Mob::read(&mut input).unwrap();assert!(input.is_empty());
        assert_eq!(saved.uuid,mob.uuid);assert_eq!(saved.vitals.health,30.0);assert_eq!(saved.vitals.max_health,30.0);assert_eq!(saved.effect_melee_damage(3.0),4.5);assert_eq!(saved.plugin_attributes,mob.plugin_attributes);
        assert_eq!(vanilla.vitals.max_health,20.0);assert_eq!(vanilla.effect_melee_damage(3.0),3.0);assert!(vanilla.plugin_attributes.is_empty());
    }
    #[test] fn movement_modifier_controls_native_speed_and_removal_restores_it() {
        let mut mob=Mob::new(1,Kind::Horse,(0.0,64.0,0.0));let speed=mob.movement_speed();
        mob.edit_plugin_attribute(&AttributeEdit::AddModifier {attribute:attributes::MOVEMENT_SPEED,modifier:Modifier::new("fourlegacy:rider",0.5,Operation::AddMultipliedTotal)}).unwrap();assert_eq!(mob.movement_speed(),speed*1.5);
        mob.edit_plugin_attribute(&AttributeEdit::RemoveModifier {attribute:attributes::MOVEMENT_SPEED,id:"fourlegacy:rider".into()}).unwrap();assert_eq!(mob.movement_speed(),speed);
    }
    #[test] fn malformed_attribute_tail_refuses_without_panicking() {for bytes in [&[][..],&[4],&[1,0,0,0,19]] {assert!(read(&mut &*bytes).is_none());}}
}
