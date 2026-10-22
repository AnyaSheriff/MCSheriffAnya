// Вики, Ominous Trial Spawner: шлем и нагрудник, каждый с шансом 50 %;
// chainmail/iron/diamond — 4:2:1. Экипировка не выпадает. [проверить] RNG.
use super::{Mob,Kind,Rng};
use crate::{blocks,combat,inventory::Stack,stackdata};
pub type Gear=[Option<Stack>;3]; // основная рука, нагрудник, шлем.
pub fn equip(mob:&mut Mob,rng:&mut Rng) {
    if !matches!(mob.kind,Kind::Zombie|Kind::Husk|Kind::Skeleton|Kind::Stray|Kind::Bogged) {return;}
    let (material,pattern)=match rng.below(7) {0..=3=>("chainmail","bolt"),4..=5=>("iron","flow"),_=>("diamond","flow")};
    let mut gear=[None;3];
    for (slot,part) in [(1,"chestplate"),(2,"helmet")] {
        if rng.below(2)==0 {continue;}
        let mut data=stackdata::Data {trim:Some((stackdata::TRIM_MATERIALS.iter().position(|s|*s=="copper").unwrap() as u8,stackdata::TRIM_PATTERNS.iter().position(|s|*s==pattern).unwrap() as u8)),..Default::default()};
        for name in ["protection","fire_protection","projectile_protection"] {data.set_enchant(stackdata::enchantment_number(name).unwrap(),4);}
        gear[slot]=Some(Stack::new(blocks::item_named(&format!("{material}_{part}")).unwrap(),1).with_extras(data));
    }
    let (weapon,enchant)=if mob.kind.skeleton() {
        mob.h.bow=true;
        match rng.below(4) {2=>("bow",Some("power")),3=>("bow",Some("punch")),_=>("bow",None)}
    } else {match rng.below(7) {4=>("iron_sword",Some("sharpness")),5=>("iron_sword",Some("knockback")),6=>("diamond_sword",None),_=>("iron_sword",None)}};
    let mut data=stackdata::Data::default();
    if let Some(enchant)=enchant {data.set_enchant(stackdata::enchantment_number(enchant).unwrap(),1);}
    gear[0]=Some(Stack::new(blocks::item_named(weapon).unwrap(),1).with_extras(data));
    mob.h.helmet=gear[2].is_some();mob.h.trial_gear=Some(gear);mob.attr_changes+=1;
}
pub fn bonus(gear:&Gear,attribute:i32)->f32 {
    gear.iter().zip([1,6,7]).filter_map(|(s,place)|s.map(|s|(s,place)))
        .flat_map(|(s,place)|combat::granted(&s,place)).filter(|(a,_)|*a==attribute).map(|(_,m)|m.amount as f32).sum()
}
pub fn write(gear:&Gear,out:&mut Vec<u8>) {
    for stack in gear {
        out.push(u8::from(stack.is_some()));
        if let Some(s)=stack {out.extend_from_slice(&s.stored_item().to_be_bytes());s.write_extras(out);}
    }
}
pub fn read(bytes:&mut &[u8])->Option<Gear> {
    let mut take=|n| {let (a,b)=bytes.split_at_checked(n)?;*bytes=b;Some(a)};
    let mut gear=[None;3];
    for slot in &mut gear {
        if take(1)?[0]!=0 {
            let item=i32::from_be_bytes(take(4)?.try_into().ok()?);
            let extra=Stack::read_extras_strict(item,&mut take)?;
            *slot=Some(Stack::from_stored_strict(item,1,extra)?);
        }
    }
    Some(gear)
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn trial_equipment_persists_and_reduces_damage() {
        let mut mob=Mob::new(8,Kind::Zombie,(0.0,64.0,0.0));
        let mut rng=Rng::new(44);
        for _ in 0..100 {equip(&mut mob,&mut rng);if mob.armor()>2.0 {break;}}
        assert!(mob.armor()>2.0);
        let mut bytes=Vec::new();mob.write(&mut bytes);
        let back=Mob::read(&mut bytes.as_slice()).unwrap();
        assert_eq!(back.h.trial_gear,mob.h.trial_gear);
        assert!(back.h.trial_gear.unwrap()[0].is_some());
        let before=mob.vitals.health;mob.hurt(4.0,crate::survival::Source::MobAttack,None);
        assert!(before-mob.vitals.health<4.0);
        let mut spider=Mob::new(9,Kind::Spider,(0.0,64.0,0.0));equip(&mut spider,&mut rng);assert!(spider.h.trial_gear.is_none());
    }
}
