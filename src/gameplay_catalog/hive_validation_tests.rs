//! Предметы пчелы проверяются в том же каталоге даже внутри переносимого улья.
use super::*;
use crate::{inventory::Stack, mob::{Kind as MobKind, Mob}, stackdata::{Content, Data}};

fn nested(stack: Stack) -> Stack {
    Stack::new(crate::blocks::item_named("shulker_box").unwrap(), 1).with_extras(Data {
        contents: vec![Content { slot: 0, item: stack.stored_item(), count: stack.count, data: stack.data }],
        ..Default::default()
    })
}
fn hive(stack: Stack) -> crate::hive::Data {
    let mut bee = Mob::new(91, MobKind::Bee, (0.0, 64.0, 0.0));
    bee.equipment = Some([Some(stack), None, None, None, None, None]);
    let mut data = crate::hive::Data::default();
    assert!(data.enter(&bee)); data
}

#[test]
fn hive_and_nested_carried_hive_keep_selected_stack_and_reject_wrong_catalog_at_full_path() {
    let base = Catalog::new(Profile::Java26_1_2);
    let preview = Catalog::new(Profile::Preview26_3);
    let slab = Stack::new(WHITE_SLAB_ITEM, 37).with_extras(Data { name: Some("Пчелиная плита".into()), ..Default::default() });
    let gear = nested(slab);
    let hive = hive(gear);
    let bytes = hive.bytes();
    let decoded = crate::hive::Data::from_bytes(&bytes).unwrap();
    assert_eq!(decoded, hive);
    assert_eq!(crate::hive::decode_occupant(&decoded.bees[0]).unwrap().equipment.unwrap()[0], Some(gear));
    assert!(preview.validate_hive(&decoded).is_ok());
    let error = base.validate_hive(&decoded).unwrap_err();
    assert_eq!(error.path, [StackLocation::HiveOccupant(0), StackLocation::MobEquipment(0), StackLocation::Container(0)]);
    assert!(matches!(error.problem, StackProblem::Catalog(Error::Disabled { .. })));
    let carried = Stack::new(crate::blocks::item_named("beehive").unwrap(), 1).with_extras(Data { bees: bytes, ..Default::default() });
    let outer = nested(carried);
    assert!(preview.validate_stack(&outer).is_ok());
    let error = base.validate_stack(&outer).unwrap_err();
    assert_eq!(error.path, [StackLocation::Container(0), StackLocation::HiveOccupant(0), StackLocation::MobEquipment(0), StackLocation::Container(0)]);
    assert!(matches!(error.problem, StackProblem::Catalog(Error::Disabled { .. })));
    assert_eq!(crate::stackdata::from_bytes(&crate::stackdata::to_bytes(&outer.extras())), Some(outer.extras()));
}

#[test]
fn ordinary_hive_preserves_bytes_but_non_bee_or_extra_entity_bytes_are_rejected() {
    let plain = Stack::new(crate::blocks::item_named("stone").unwrap(), 1);
    let original = hive(plain);
    let bytes = original.bytes();
    assert_eq!(crate::hive::Data::from_bytes(&bytes).unwrap().bytes(), bytes);
    assert!(Catalog::new(Profile::Java26_1_2).validate_hive(&original).is_ok());
    for non_bee in [false, true] {
        let mut bad = original.clone();
        if non_bee {
            bad.bees[0].entity.clear();
            Mob::new(92, MobKind::Cow, (0.0, 64.0, 0.0)).write(&mut bad.bees[0].entity);
        } else { bad.bees[0].entity.push(0); }
        assert!(crate::hive::decode_occupant(&bad.bees[0]).is_none());
        assert!(crate::hive::Data::from_bytes(&bad.bytes()).is_none());
        let error = Catalog::new(Profile::Preview26_3).validate_hive(&bad).unwrap_err();
        assert_eq!(error.path, [StackLocation::HiveOccupant(0)]);
        assert_eq!(error.problem, StackProblem::InvalidHive);
        assert_eq!(crate::hive::Data::from_bytes(&bytes), Some(original.clone()));
    }
}

#[test]
fn unknown_hive_or_occupant_fields_refuse_instead_of_disappearing_on_resave() {
    let original = hive(Stack::new(crate::blocks::item_named("stone").unwrap(), 1));
    let valid = original.bytes();
    for occupant in [false, true] {
        let mut value: serde_json::Value = serde_json::from_slice(&valid).unwrap();
        let target = if occupant { &mut value["bees"][0] } else { &mut value };
        target.as_object_mut().unwrap().insert("unsupported_future_storage".into(), serde_json::json!({"item": 999999}));
        let bytes = serde_json::to_vec(&value).unwrap();
        assert!(crate::hive::Data::from_bytes(&bytes).is_none());
        let carried = Stack::new(crate::blocks::item_named("beehive").unwrap(), 1)
            .with_extras(Data { bees: bytes, ..Default::default() });
        assert_eq!(Catalog::new(Profile::Java26_1_2).validate_stack(&carried).unwrap_err().problem, StackProblem::InvalidHive);
        assert_eq!(crate::hive::Data::from_bytes(&valid), Some(original.clone()));
    }
}
