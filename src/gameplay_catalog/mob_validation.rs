//! Каталог всех реальных предметных полей моба, включая скрытые старые копии.
//! equipment::all здесь не подходит: показанная экипировка маскирует другие поля.
use super::{Catalog, Kind, StackError, StackLocation, StackProblem};
use crate::mob::{Kind as MobKind, Mob};

impl Catalog {
    pub fn validate_mob(self, mob: &Mob) -> Result<(), StackError> {
        self.validate_mob_at(mob, &mut Vec::new(), 0)
    }

    /// Вложенный моб (например, пчела в предмете) продолжает общий счётчик глубины.
    pub(super) fn validate_mob_at(self, mob: &Mob, path: &mut Vec<StackLocation>, depth: usize) -> Result<(), StackError> {
        if depth >= 16 { return Err(StackError { path: path.clone(), problem: StackProblem::NestingLimit }); }
        let mut stack_at = |stack: &crate::inventory::Stack, location| {
            path.push(location);
            let result = self.validate_stack_at(stack, path, depth + 1);
            path.pop();
            result
        };
        if let Some(gear) = &mob.equipment {
            for (slot, stack) in gear.iter().enumerate() {
                if let Some(stack) = stack { stack_at(stack, StackLocation::MobEquipment(slot))?; }
            }
        }
        if let Some(gear) = &mob.h.trial_gear {
            for (slot, stack) in gear.iter().enumerate() {
                if let Some(stack) = stack { stack_at(stack, StackLocation::MobTrialGear(slot))?; }
            }
        }
        for (slot, stack) in mob.stand.equipment.iter().enumerate() {
            if let Some(stack) = stack { stack_at(stack, StackLocation::MobStandEquipment(slot))?; }
        }
        for (slot, stack) in mob.horse.slots.iter().enumerate() {
            if let Some(stack) = stack { stack_at(stack, StackLocation::MobHorseSlot(slot))?; }
        }
        // Проверяем весь вектор: текущие chest/strength не скрывают заполненный хвост.
        for (slot, stack) in mob.land.cargo.iter().enumerate() {
            if let Some(stack) = stack { stack_at(stack, StackLocation::MobLandCargo(slot))?; }
        }
        for (stack, location) in [
            (mob.land.held.as_ref(), StackLocation::MobLandHeld),
            (mob.small.held.as_ref(), StackLocation::MobSmallHeld),
            (mob.small.cargo.as_ref(), StackLocation::MobSmallCargo),
            (mob.wild.mouth.as_ref(), StackLocation::MobWildMouth),
            (mob.newmobs.cargo.as_ref(), StackLocation::MobNewCargo),
        ] {
            if let Some(stack) = stack { stack_at(stack, location)?; }
        }
        for (index, offer) in mob.villager.offers.iter().enumerate() {
            stack_at(&offer.buy, StackLocation::MobTradeBuy(index))?;
            if let Some(stack) = &offer.buy_b { stack_at(stack, StackLocation::MobTradeSecondBuy(index))?; }
            stack_at(&offer.sell, StackLocation::MobTradeSell(index))?;
        }
        if mob.kind == MobKind::Enderman && let Some(state) = mob.h.carried {
            path.push(StackLocation::MobCarriedState);
            let result = self.name(Kind::State, state).map_err(|error| StackError { path: path.clone(), problem: StackProblem::Catalog(error) });
            path.pop();
            result?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{gameplay_catalog::{Error, Profile, WHITE_SLAB_ITEM, WHITE_SLAB_FIRST_STATE}, inventory::Stack};

    fn mob() -> Mob { Mob::new(1, MobKind::Zombie, (0.0, 70.0, 0.0)) }
    fn base() -> Catalog { Catalog::new(Profile::Java26_1_2) }
    fn preview() -> Catalog { Catalog::new(Profile::Preview26_3) }
    fn locations() -> Vec<StackLocation> {
        vec![StackLocation::MobEquipment(5), StackLocation::MobTrialGear(2), StackLocation::MobStandEquipment(5),
            StackLocation::MobHorseSlot(16), StackLocation::MobLandCargo(19), StackLocation::MobLandHeld,
            StackLocation::MobSmallHeld, StackLocation::MobSmallCargo, StackLocation::MobWildMouth, StackLocation::MobNewCargo,
            StackLocation::MobTradeBuy(0), StackLocation::MobTradeSecondBuy(0), StackLocation::MobTradeSell(0)]
    }
    fn put(mob: &mut Mob, location: &StackLocation, stack: Stack) {
        match *location {
            StackLocation::MobEquipment(slot) => mob.equipment.get_or_insert([None; 6])[slot] = Some(stack),
            StackLocation::MobTrialGear(slot) => mob.h.trial_gear.get_or_insert([None; 3])[slot] = Some(stack),
            StackLocation::MobStandEquipment(slot) => mob.stand.equipment[slot] = Some(stack),
            StackLocation::MobHorseSlot(slot) => mob.horse.slots[slot] = Some(stack),
            StackLocation::MobLandCargo(slot) => { mob.land.cargo.resize(slot + 1, None); mob.land.cargo[slot] = Some(stack); }
            StackLocation::MobLandHeld => mob.land.held = Some(stack),
            StackLocation::MobSmallHeld => mob.small.held = Some(stack),
            StackLocation::MobSmallCargo => mob.small.cargo = Some(stack),
            StackLocation::MobWildMouth => mob.wild.mouth = Some(stack),
            StackLocation::MobNewCargo => mob.newmobs.cargo = Some(stack),
            StackLocation::MobTradeBuy(_) | StackLocation::MobTradeSecondBuy(_) | StackLocation::MobTradeSell(_) => {
                let normal = Stack::new(crate::blocks::item_named("stone").unwrap(), 1);
                mob.villager.offers = vec![crate::villager::Offer { buy: normal, buy_b: None, sell: normal, uses: 0, max_uses: 12, xp: 1, reward_exp: true, multiplier: 0.05, special: 0, demand: 0 }];
                match location {
                    StackLocation::MobTradeBuy(_) => mob.villager.offers[0].buy = stack,
                    StackLocation::MobTradeSecondBuy(_) => mob.villager.offers[0].buy_b = Some(stack),
                    _ => mob.villager.offers[0].sell = stack,
                }
            }
            _ => panic!("непредметное поле теста"),
        }
    }

    #[test]
    fn every_storage_family_checks_catalog_and_preserves_valid_stack() {
        for location in locations() {
            let mut entity = mob();
            let valid = Stack::new(WHITE_SLAB_ITEM, 2).with_extras(crate::stackdata::Data { name: Some("Проверка".into()), repair_cost: 7, ..Default::default() });
            put(&mut entity, &location, valid);
            assert!(preview().validate_mob(&entity).is_ok(), "{location:?}");
            let error = base().validate_mob(&entity).unwrap_err();
            assert_eq!(error.path.as_slice(), std::slice::from_ref(&location));
            assert!(matches!(error.problem, StackProblem::Catalog(Error::Disabled { .. })));
            put(&mut entity, &location, Stack::new(999_999, 1));
            let error = preview().validate_mob(&entity).unwrap_err();
            assert_eq!(error.path, [location]);
            assert!(matches!(error.problem, StackProblem::Catalog(Error::UnknownInternal { .. })));
        }
    }

    #[test]
    fn shadow_copies_and_tail_cargo_are_not_masked_by_visible_gear() {
        for hidden in [StackLocation::MobTrialGear(0), StackLocation::MobWildMouth, StackLocation::MobLandHeld, StackLocation::MobLandCargo(19)] {
            let mut entity = mob();
            entity.equipment = Some([Some(Stack::new(crate::blocks::item_named("stone").unwrap(), 1)); 6]);
            put(&mut entity, &hidden, Stack::new(WHITE_SLAB_ITEM, 1));
            assert_eq!(base().validate_mob(&entity).unwrap_err().path, [hidden]);
        }
    }

    #[test]
    fn enderman_carried_block_state_obeys_catalog() {
        let mut entity = Mob::new(2, MobKind::Enderman, (0.0, 70.0, 0.0));
        entity.h.carried = Some(WHITE_SLAB_FIRST_STATE);
        assert!(preview().validate_mob(&entity).is_ok());
        assert_eq!(base().validate_mob(&entity).unwrap_err().path, [StackLocation::MobCarriedState]);
        entity.h.carried = Some(999_999);
        assert!(matches!(preview().validate_mob(&entity).unwrap_err().problem, StackProblem::Catalog(Error::UnknownInternal { .. })));
        entity.h.carried = Some(crate::blocks::state_by_name("stone").unwrap());
        assert!(base().validate_mob(&entity).is_ok());
    }

    #[test]
    fn nested_stack_paths_and_depth_budget_continue_through_mob() {
        let mut entity = mob();
        let outer = Stack::new(crate::blocks::item_named("shulker_box").unwrap(), 1).with_extras(crate::stackdata::Data {
            contents: vec![crate::stackdata::Content { slot: 0, item: WHITE_SLAB_ITEM, count: 1, data: None }], ..Default::default()
        });
        entity.equipment = Some([Some(outer), None, None, None, None, None]);
        assert_eq!(base().validate_mob(&entity).unwrap_err().path, [StackLocation::MobEquipment(0), StackLocation::Container(0)]);
        let prefix = vec![StackLocation::Slot(9)]; let mut path = prefix.clone();
        let error = preview().validate_mob_at(&entity, &mut path, 15).unwrap_err();
        assert_eq!(error.problem, StackProblem::NestingLimit);
        assert_eq!(error.path, [StackLocation::Slot(9), StackLocation::MobEquipment(0)]);
        assert_eq!(path, prefix);
        assert_eq!(preview().validate_mob_at(&entity, &mut path, 16).unwrap_err().path, prefix);
    }
}
