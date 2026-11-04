//! Явная нормализация старых main-ульев после побайтовой сверки внешней записи.
//! Обычные читатели никогда не вызывают этот обход.
use super::{Data, DataId};
use crate::{inventory::Stack, mob::Mob};

fn nested(id: &mut Option<DataId>) -> Option<()> {
    if let Some(stored) = *id {
        let mut data = super::try_get(stored)?;
        components(&mut data)?;
        *id = super::intern(data);
    }
    Some(())
}

pub(crate) fn stack(stack: &mut Stack) -> Option<()> { nested(&mut stack.data) }

pub(crate) fn components(data: &mut Data) -> Option<()> {
    let _scope = super::storage_depth::enter()?;
    for child in data.contents.iter_mut().chain(data.bundle.iter_mut()).chain(data.pot_sides.iter_mut()) {
        nested(&mut child.data)?;
    }
    if let Some(child) = &mut data.charged_arrow { nested(&mut child.data)?; }
    for child in &mut data.charged_projectiles { nested(&mut child.content.data)?; }
    if !data.bees.is_empty() {
        let mut hive = crate::hive::Data::from_bytes_with_legacy_metadata(&data.bees, true)?;
        crate::hive::normalize_main_entities(&mut hive)?;
        data.bees = hive.bytes();
    }
    Some(())
}

pub(crate) fn mob(mob: &mut Mob) -> Option<()> {
    let _scope = super::storage_depth::enter()?;
    if let Some(gear) = &mut mob.equipment { for item in gear.iter_mut().flatten() { stack(item)?; } }
    if let Some(gear) = &mut mob.h.trial_gear { for item in gear.iter_mut().flatten() { stack(item)?; } }
    for item in mob.stand.equipment.iter_mut().flatten().chain(mob.horse.slots.iter_mut().flatten()).chain(mob.land.cargo.iter_mut().flatten()) {
        stack(item)?;
    }
    for item in [&mut mob.land.held, &mut mob.small.held, &mut mob.small.cargo, &mut mob.wild.mouth, &mut mob.newmobs.cargo].into_iter().flatten() {
        stack(item)?;
    }
    for offer in &mut mob.villager.offers {
        stack(&mut offer.buy)?;
        if let Some(item) = &mut offer.buy_b { stack(item)?; }
        stack(&mut offer.sell)?;
    }
    Some(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    pub(crate) const HIVE: &[u8] = include_bytes!("../../testdata/playerdata/main-beehive-plugin.json");
    pub(crate) const STACK: &[u8] = include_bytes!("../../testdata/playerdata/main-beehive-plugin.stackdata");
    pub(crate) fn catalog() -> crate::gameplay_catalog::Catalog {
        crate::gameplay_catalog::Catalog::new(crate::gameplay_catalog::Profile::Java26_1_2)
    }
    pub(crate) fn raw_stack() -> Stack {
        Stack::new(crate::blocks::item_named("beehive").unwrap(), 1)
            .with_extras(super::super::from_bytes_with_legacy_metadata(STACK, true).unwrap())
    }
    pub(crate) fn assert_hive(hive: &crate::hive::Data) {
        assert_eq!(hive.bees.len(), 1);
        let occupant = &hive.bees[0];
        assert_eq!((occupant.ticks, occupant.minimum, occupant.nectar, occupant.uuid), (120,2400,true,[71;16]));
        let bee = crate::hive::decode_occupant(occupant).unwrap();
        assert_eq!(bee.uuid, [71;16]); assert_eq!(bee.name.as_deref(), Some("Frozen Bee"));
        assert_eq!((bee.vitals.health, bee.vitals.max_health, bee.age), (29.0,40.0,-123));
        assert!(bee.wild.nectar); assert!(bee.silent);
        assert_eq!(bee.plugin_attributes.len(), 1);
        catalog().validate_hive(hive).unwrap();
    }
    pub(crate) fn assert_stack(stack: Stack) { assert_hive(&crate::hive::Data::from_bytes(&stack.extras().bees).unwrap()); }

    #[test]
    fn frozen_main_hive_requires_explicit_context_and_keeps_full_bee_state() {
        assert!(crate::hive::Data::from_bytes(HIVE).is_none());
        assert!(super::super::from_bytes(STACK).is_none());
        let mut raw = crate::hive::Data::from_bytes_with_legacy_metadata(HIVE, true).unwrap();
        assert_eq!(raw.bytes(), HIVE);
        let data = super::super::from_bytes_with_legacy_metadata(STACK, true).unwrap();
        assert_eq!(super::super::to_bytes_main_metadata(&data), STACK);
        crate::hive::normalize_main_entities(&mut raw).unwrap();
        assert_hive(&raw);
        assert_ne!(raw.bytes(), HIVE);
        let mut item = raw_stack(); stack(&mut item).unwrap(); assert_stack(item);
        let bytes = super::super::to_bytes(&item.extras());
        assert_eq!(super::super::from_bytes(&bytes), Some(item.extras()));
        for end in 0..STACK.len() {
            let mut framed = (end as u16).to_be_bytes().to_vec(); framed.extend(&STACK[..end]);
            let mut rest = framed.as_slice();
            assert!(Stack::read_extras_strict_with_legacy_metadata(i32::MIN | 1, |n| { let (head,tail)=rest.split_at_checked(n)?; rest=tail; Some(head) },true).is_none(), "обрезка{end}");
        }
        let mut invalid: crate::hive::Data = serde_json::from_slice(HIVE).unwrap();
        invalid.bees[0].entity.push(0);
        assert!(crate::hive::Data::from_bytes_with_legacy_metadata(&invalid.bytes(), true).is_none());
        let mut unknown:serde_json::Value=serde_json::from_slice(HIVE).unwrap();
        unknown["unknown"]=serde_json::json!(71);
        assert!(crate::hive::Data::from_bytes_with_legacy_metadata(&serde_json::to_vec(&unknown).unwrap(),true).is_none());
        let mut unknown:serde_json::Value=serde_json::from_slice(HIVE).unwrap();
        unknown["bees"][0]["unknown"]=serde_json::json!(71);
        assert!(crate::hive::Data::from_bytes_with_legacy_metadata(&serde_json::to_vec(&unknown).unwrap(),true).is_none());
        let mut whitespace=HIVE.to_vec();whitespace.push(b' ');
        assert!(crate::hive::Data::from_bytes_with_legacy_metadata(&whitespace,true).is_none());
    }

    #[test]
    fn main_hive_nested_inventory_checks_original_bytes_before_normalizing() {
        let mut inventory = crate::inventory::Inventory::new();
        let hive = raw_stack();
        let data = Data { contents: vec![super::super::Content { slot:2,item:hive.item,count:1,data:hive.data }], ..Default::default() };
        inventory.set(36, Some(Stack::new(crate::blocks::item_named("shulker_box").unwrap(), 1).with_extras(data)));
        let bytes = inventory.to_bytes_with_legacy_metadata(true);
        assert!(crate::inventory::Inventory::from_bytes_for_catalog(&bytes, catalog()).is_err());
        let current = crate::inventory::Inventory::from_bytes_for_catalog_with_legacy_metadata(&bytes, catalog(), true).unwrap();
        let child = current.slot(36).unwrap().extras().contents[0];
        assert_stack(Stack { item:child.item,count:child.count,tnt:None,data:child.data });
        let mut trailing = bytes.clone(); trailing.push(42);
        assert!(crate::inventory::Inventory::from_bytes_for_catalog_with_legacy_metadata(&trailing, catalog(), true).is_err());
        let mut malformed = bytes; malformed[1] += 1;
        assert!(crate::inventory::Inventory::from_bytes_for_catalog_with_legacy_metadata(&malformed, catalog(), true).is_err());
    }

    #[test]
    fn main_hive_mob_gear_uses_legacy_reader_then_shared_normalizer() {
        // Ровно main Zombie head/body + equipment tail64; пчела внутри предмета — frozen.
        let bee = include_bytes!("../../testdata/playerdata/main-bee-plugin.bin");
        let mut bytes = bee[..47].to_vec(); bytes[0] = 4 | 128;
        bytes.extend([64,0,0,0,1]);
        let hive = raw_stack(); bytes.extend(hive.stored_item().to_be_bytes()); bytes.extend(1i32.to_be_bytes());
        bytes.extend((STACK.len() as u16).to_be_bytes()); bytes.extend(STACK);
        bytes.extend([0;5]); for _ in 0..6 { bytes.extend(0.085f32.to_be_bytes()); }
        let mut rest = bytes.as_slice();
        assert!(Mob::read(&mut rest).is_none());
        let mut rest = bytes.as_slice();
        let mut entity = Mob::read_with_storage_context(&mut rest, true, true).unwrap(); assert!(rest.is_empty());
        mob(&mut entity).unwrap(); catalog().validate_mob(&entity).unwrap();
        assert_stack(entity.equipment.unwrap()[0].unwrap());
        let mut current = Vec::new(); entity.write(&mut current);
        let mut rest = current.as_slice(); let loaded = Mob::read(&mut rest).unwrap(); assert!(rest.is_empty());
        assert_stack(loaded.equipment.unwrap()[0].unwrap());
        let mut bad = bytes; let at = 47+5+8; bad[at..at+2].copy_from_slice(&((STACK.len()-1) as u16).to_be_bytes());
        assert!(Mob::read_with_storage_context(&mut bad.as_slice(),true,true).is_none());
    }
}
