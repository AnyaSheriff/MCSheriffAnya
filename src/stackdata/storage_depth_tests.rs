//! Общий бюджет чтения не сбрасывается при переходе через улей и моба.
use super::*;
use crate::{inventory::Stack, mob::{Kind, Mob}};

fn wrap(data: Data) -> Data {
    Data { contents: vec![Content {
        slot: 0, item: crate::blocks::item_named("shulker_box").unwrap(), count: 1, data: intern(data),
    }], ..Default::default() }
}
fn chain(levels: usize) -> Data {
    assert!(levels > 0);
    let mut data = Data { name: Some("Граница глубины".into()), ..Default::default() };
    for _ in 1..levels { data = wrap(data); }
    data
}

#[test]
fn sixteen_component_levels_are_readable_seventeen_fail_and_failure_does_not_poison_next_decode() {
    let allowed = chain(16);
    assert_eq!(from_bytes(&to_bytes(&allowed)), Some(allowed.clone()));
    let forbidden = wrap(allowed.clone());
    let bytes = to_bytes(&forbidden);
    for _ in 0..3 {
        assert!(from_bytes(&bytes).is_none());
        assert_eq!(from_bytes(&to_bytes(&allowed)), Some(allowed.clone()));
        assert!(from_bytes(&[0]).is_none());
        assert_eq!(from_bytes(&to_bytes(&Data::default())), Some(Data::default()));
    }
}

#[test]
fn container_hive_mob_component_chain_shares_the_same_sixteen_level_parse_budget() {
    let gear_data = chain(2);
    let gear = Stack::new(crate::blocks::item_named("shulker_box").unwrap(), 1).with_extras(gear_data.clone());
    let mut bee = Mob::new(93, Kind::Bee, (0.0, 64.0, 0.0));
    bee.equipment = Some([Some(gear), None, None, None, None, None]);
    let mut hive = crate::hive::Data::default();
    assert!(hive.enter(&bee));
    let hive_bytes = hive.bytes();
    assert_eq!(crate::hive::Data::from_bytes(&hive_bytes), Some(hive.clone()));
    let mut allowed = Data { bees: hive_bytes.clone(), ..Default::default() };
    // 12 контейнеров + компоненты улья + чтение улья + 2 уровня у пчелы = 16.
    for _ in 0..12 { allowed = wrap(allowed); }
    assert_eq!(from_bytes(&to_bytes(&allowed)), Some(allowed.clone()));
    let forbidden = wrap(allowed.clone());
    let forbidden_bytes = to_bytes(&forbidden);
    for _ in 0..2 {
        assert!(from_bytes(&forbidden_bytes).is_none(), "улей не начинает новый бюджет");
        assert_eq!(from_bytes(&to_bytes(&gear_data)), Some(gear_data.clone()));
        assert_eq!(crate::hive::Data::from_bytes(&hive_bytes), Some(hive.clone()));
        assert_eq!(from_bytes(&to_bytes(&allowed)), Some(allowed.clone()));
    }
}
