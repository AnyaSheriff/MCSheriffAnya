use super::*;

fn named(name: &str, count: i32) -> Stack {
    Stack::new(crate::blocks::item_named(name).unwrap(), count).with_extras(crate::stackdata::Data {
        name: Some("Сохраняется полностью".into()), repair_cost: 7, ..Default::default()
    })
}

fn restored(mob: &Mob) -> Mob {
    let mut bytes = Vec::new();
    mob.write(&mut bytes);
    let next = include_bytes!("item_storage_fixtures/cow-legacy.bin");
    bytes.extend(next);
    let mut rest = bytes.as_slice();
    let restored = Mob::read(&mut rest).unwrap();
    assert_eq!(rest, next);
    assert_eq!(Mob::read(&mut rest).unwrap().kind, Kind::Cow);
    assert!(rest.is_empty());
    restored
}

#[test]
fn frozen_legacy_records_remain_readable_without_consuming_next_mob() {
    for (fixture, kind) in [
        (&include_bytes!("item_storage_fixtures/fox-stone-count1-legacy.bin")[..], Kind::Fox),
        (&include_bytes!("item_storage_fixtures/villager-plain-offer-legacy.bin")[..], Kind::Villager),
        (&include_bytes!("item_storage_fixtures/panda-legacy.bin")[..], Kind::Panda),
        (&include_bytes!("item_storage_fixtures/wandering-trader-legacy.bin")[..], Kind::WanderingTrader),
    ] {
        let mut bytes = fixture.to_vec();
        bytes.extend(include_bytes!("item_storage_fixtures/cow-legacy.bin"));
        let mut rest = bytes.as_slice();
        let mob = Mob::read(&mut rest).unwrap();
        assert_eq!(mob.kind, kind);
        if kind == Kind::Fox { assert_eq!(mob.wild.mouth, Some(Stack::new(1, 1))); }
        else {
            assert_eq!(mob.villager.offers.len(), 1);
            assert_eq!(mob.villager.offers[0].buy, Stack::new(1, 2));
            assert_eq!(mob.villager.offers[0].sell, Stack::new(2, 3));
        }
        assert_eq!(Mob::read(&mut rest).unwrap().kind, Kind::Cow);
        assert!(rest.is_empty());
    }
}

#[test]
fn complete_trade_stacks_survive_for_both_villagers_and_land_trader() {
    let fixture = include_bytes!("item_storage_fixtures/villager-plain-offer-legacy.bin");
    for kind in [Kind::Villager, Kind::ZombieVillager, Kind::WanderingTrader] {
        let mut mob = Mob::new(1, kind, (1.0, 64.0, 2.0));
        mob.villager.offers = Mob::read(&mut fixture.as_slice()).unwrap().villager.offers;
        let offer = &mut mob.villager.offers[0];
        offer.buy = named("emerald", 3);
        offer.buy_b = Some(named("paper", 2));
        offer.sell = named("diamond_sword", 1).with_extras(crate::stackdata::Data {
            name: Some("Меч".into()), enchants: vec![(0, 2)], ..Default::default()
        });
        let back = restored(&mob);
        assert_eq!(back.villager.offers, mob.villager.offers);
    }
}

#[test]
fn fox_count_panda_held_and_all_land_cargo_survive() {
    let mut fox = Mob::new(1, Kind::Fox, (1.0, 64.0, 2.0));
    fox.wild.mouth = Some(named("stone", 2));
    assert_eq!(restored(&fox).wild.mouth, fox.wild.mouth);
    let mut panda = Mob::new(2, Kind::Panda, (1.0, 64.0, 2.0));
    panda.land.held = Some(named("bamboo", 2));
    assert_eq!(restored(&panda).land.held, panda.land.held);
    let mut llama = Mob::new(3, Kind::Llama, (1.0, 64.0, 2.0));
    llama.land.chest = false;
    llama.land.cargo = vec![None, Some(named("stone", 3))];
    assert_eq!(restored(&llama).land.cargo, llama.land.cargo);
}

#[test]
fn annex_is_length_delimited_and_rejects_unknown_schema_truncation_and_trailing_data() {
    let mut mob = Mob::new(1, Kind::Fox, (1.0, 64.0, 2.0));
    mob.wild.mouth = Some(named("stone", 2));
    let record = encode(&mob).unwrap().unwrap();
    let mut bytes = (record.len() as u32).to_be_bytes().to_vec(); bytes.extend(&record);
    for end in 0..bytes.len() { assert!(read(&mut &bytes[..end]).is_none(), "обрезано {end}"); }
    let mut unknown = bytes.clone(); unknown[4] = 2;
    assert!(read(&mut unknown.as_slice()).is_none());
    let mut trailing = bytes.clone(); trailing.push(0); let length = (record.len() as u32 + 1).to_be_bytes(); trailing[..4].copy_from_slice(&length);
    assert!(read(&mut trailing.as_slice()).is_none());
    let mut wrong_kind = Mob::new(2, Kind::Panda, (1.0, 64.0, 2.0));
    assert!(apply(&mut wrong_kind, read(&mut bytes.as_slice()).unwrap()).is_none());
    let mut all = Vec::new(); mob.write(&mut all);
    for end in 0..all.len() { assert!(Mob::read(&mut &all[..end]).is_none(), "моб обрезан {end}"); }
}

#[test]
fn cardinality_and_nonpositive_count_cannot_be_silently_truncated() {
    let mut mob = Mob::new(1, Kind::Llama, (1.0, 64.0, 2.0));
    mob.land.cargo = vec![None; u16::MAX as usize + 1];
    assert!(mob.validate_item_storage().is_err());
    mob.land.cargo = vec![Some(Stack::new(1, 0))];
    assert!(mob.validate_item_storage().is_err());
}

#[test]
fn offer_count_mismatch_and_absent_required_stack_are_rejected() {
    let fixture = include_bytes!("item_storage_fixtures/villager-plain-offer-legacy.bin");
    let mob = Mob::read(&mut fixture.as_slice()).unwrap();
    let record = encode(&mob).unwrap().unwrap();
    let mut bytes = (record.len() as u32).to_be_bytes().to_vec(); bytes.extend(&record);
    let mut empty = Mob::new(1, Kind::Villager, (1.0, 64.0, 2.0));
    empty.villager.offers.clear();
    assert!(apply(&mut empty, read(&mut bytes.as_slice()).unwrap()).is_none());
    // schema/mask/u16count затем presence первого обязательного buy.
    let mut absent = bytes.clone(); absent[8] = 0;
    assert!(read(&mut absent.as_slice()).is_none());
    let mut mask = bytes; mask[5] |= 0x80;
    assert!(read(&mut mask.as_slice()).is_none());
}

#[test]
fn raw_annex_count_and_custom_tnt_tag_cannot_normalize_into_valid_stack() {
    let mut mob = Mob::new(1, Kind::Fox, (1.0, 64.0, 2.0));
    mob.wild.mouth = Some(Stack::new(1, 2));
    let record = encode(&mob).unwrap().unwrap();
    let mut bytes = (record.len() as u32).to_be_bytes().to_vec(); bytes.extend(record);
    // length4/schema1/mask1/presence1/stored4/count4.
    let mut zero = bytes.clone(); zero[11..15].copy_from_slice(&0i32.to_be_bytes());
    assert!(read(&mut zero.as_slice()).is_none());
    let mut wrong_tnt = bytes; wrong_tnt[7..11].copy_from_slice(&0x7f00_0001i32.to_be_bytes());
    assert!(read(&mut wrong_tnt.as_slice()).is_none());
}

#[test]
fn enderman_only_minus_one_is_empty_carried_state() {
    // Старый narrow Enderman47байт body + i32 carried, без common tail.
    let mut bytes = include_bytes!("item_storage_fixtures/cow-legacy.bin").to_vec();
    bytes[0] = 10;
    for raw in [-2i32, i32::MIN, -1, 0, 1] {
        let mut record = bytes.clone(); record.extend(raw.to_be_bytes());
        let loaded = Mob::read(&mut record.as_slice());
        if raw < -1 { assert!(loaded.is_none()); }
        else { assert_eq!(loaded.unwrap().h.carried, (raw >= 0).then_some(raw)); }
    }
}

#[test]
fn trial_gear_keeps_complete_counts_and_components_in_annex() {
    let mut mob = Mob::new(1, Kind::Skeleton, (1.0, 64.0, 2.0));
    mob.h.trial_gear = Some([Some(named("stone", 2)), None, Some(named("iron_helmet", 1))]);
    assert_eq!(restored(&mob).h.trial_gear, mob.h.trial_gear);
}

#[test]
fn preflight_checks_non_annex_equipment_and_ordinary_slot_families() {
    let bad=Stack::new(1,1).with_extras(crate::stackdata::Data { name:Some("я".repeat(40_000)),..Default::default() });
    for slot in 0..13 {
        let mut mob=Mob::new(1,Kind::Bee,(0.5,64.0,0.5));
        match slot {
            0 => mob.equipment=Some([Some(bad),None,None,None,None,None]),
            1 => mob.h.trial_gear=Some([Some(bad),None,None]),
            2 => mob.stand.equipment[0]=Some(bad),
            3 => mob.horse.slots[0]=Some(bad),
            4 => mob.land.cargo=vec![Some(bad)],
            5 => mob.small.held=Some(bad),
            6 => mob.small.cargo=Some(bad),
            7 => mob.wild.mouth=Some(bad),
            8 => mob.land.held=Some(bad),
            9 => mob.newmobs.cargo=Some(bad),
            10..=12 => {
                let fixture=include_bytes!("item_storage_fixtures/villager-plain-offer-legacy.bin");
                mob.villager.offers=Mob::read(&mut fixture.as_slice()).unwrap().villager.offers;
                if slot==10 { mob.villager.offers[0].buy=bad; } else if slot==11 { mob.villager.offers[0].buy_b=Some(bad); } else { mob.villager.offers[0].sell=bad; }
            }
            _ => unreachable!(),
        }
        assert!(mob.validate_item_storage().is_err(),"семейство {slot}");
    }
    // Существование предмета выбирает Catalog, а не проверка файлового кодирования.
    assert!(validate_stack_without_writing(&Stack::new(999_999,1)).is_ok());
}
