//! Проверка полного дерева предметов перед записью: без подмены неизвестного.
use super::{Catalog, Error, Kind};
use crate::{inventory::{Inventory, Stack}, stackdata::{self, Content}};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StackLocation {
    Slot(usize), Cursor, Container(usize), Bundle(usize), ChargedArrow,
    Projectile(usize), PotDecoration(usize), Ender(usize), HiveOccupant(usize), Shoulder(usize),
    MobEquipment(usize), MobTrialGear(usize), MobStandEquipment(usize), MobHorseSlot(usize),
    MobLandCargo(usize), MobLandHeld, MobSmallHeld, MobSmallCargo, MobWildMouth, MobNewCargo,
    MobTradeBuy(usize), MobTradeSecondBuy(usize), MobTradeSell(usize), MobCarriedState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StackProblem {
    Catalog(Error), EmptyItem, Count { count: i32, limit: i32 },
    MalformedEncoding, CustomTnt, MissingComponents, ComponentsTooLarge, LossyComponents, NestingLimit, InvalidHive,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StackError {
    pub path: Vec<StackLocation>,
    pub problem: StackProblem,
}

impl std::fmt::Display for StackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "неверная стопка {:?}: {:?}", self.path, self.problem)
    }
}
impl std::error::Error for StackError {}

impl Catalog {
    pub fn validate_stack(self, stack: &Stack) -> Result<(), StackError> {
        self.validate_stack_at(stack, &mut Vec::new(), 0)
    }

    /// Включает броню, вторую руку и курсор; не меняет ни одной ячейки.
    pub fn validate_inventory(self, inventory: &Inventory) -> Result<(), StackError> {
        for (slot, stack) in inventory.slots().iter().enumerate() {
            if let Some(stack) = stack {
                self.validate_stack_at(stack, &mut vec![StackLocation::Slot(slot)], 0)?;
            }
        }
        if let Some(stack) = inventory.cursor() {
            self.validate_stack_at(&stack, &mut vec![StackLocation::Cursor], 0)?;
        }
        Ok(())
    }

    pub(super) fn validate_stack_at(self, stack: &Stack, path: &mut Vec<StackLocation>, depth: usize) -> Result<(), StackError> {
        let current_path = path.clone();
        let fail = |problem| StackError { path: current_path.clone(), problem };
        // Предел нашего файлового декодера, не правило игры.
        if depth >= 16 { return Err(fail(StackProblem::NestingLimit)); }
        if stack.item <= 0 { return Err(fail(StackProblem::EmptyItem)); }
        let limit = self.stack_size(stack.item).map_err(|e| fail(StackProblem::Catalog(e)))?;
        if !(1..=limit).contains(&stack.count) { return Err(fail(StackProblem::Count { count: stack.count, limit })); }
        if let Some(kind) = stack.tnt
            && (kind.0 > 0x7f || crate::custom_tnt::by_id(kind).is_none()
                || crate::blocks::item_name(stack.item) != Some("tnt")) {
            return Err(fail(StackProblem::CustomTnt));
        }
        let Some(id) = stack.data else { return Ok(()); };
        let data = stackdata::try_get(id).ok_or_else(|| fail(StackProblem::MissingComponents))?;
        if !stackdata::valid_tnt_custom_data(stack, &data) { return Err(fail(StackProblem::CustomTnt)); }
        let mut nested = |content: &Content, location| {
            path.push(location);
            // Вложенные записи хранят stored_item, включая метку custom TNT.
            let mut child = Stack::from_stored(content.item, content.count);
            child.data = content.data;
            let custom = ((content.item >> 24) & 0x7f) as u8;
            let result = if child.tnt.map_or(0, |kind| kind.0) != custom {
                Err(StackError { path: path.clone(), problem: StackProblem::CustomTnt })
            } else { self.validate_stack_at(&child, path, depth + 1) };
            path.pop();
            result
        };
        for (i, child) in data.contents.iter().enumerate() { nested(child, StackLocation::Container(i))?; }
        for (i, child) in data.bundle.iter().enumerate() { nested(child, StackLocation::Bundle(i))?; }
        if let Some(child) = &data.charged_arrow { nested(child, StackLocation::ChargedArrow)?; }
        for (i, child) in data.charged_projectiles.iter().enumerate() { nested(&child.content, StackLocation::Projectile(i))?; }
        for side in &data.pot_sides { nested(side, StackLocation::PotDecoration(side.slot as usize))?; }
        if let Some(sides) = data.pot_decorations {
            for (i, item) in sides.into_iter().enumerate() {
                self.name(Kind::Item, item).map_err(|e| {
                    let mut path = path.clone(); path.push(StackLocation::PotDecoration(i));
                    StackError { path, problem: StackProblem::Catalog(e) }
                })?;
            }
        }
        if !data.bees.is_empty() {
            let hive = crate::hive::Data::from_bytes(&data.bees).ok_or_else(|| fail(StackProblem::InvalidHive))?;
            self.validate_hive_at(&hive, path, depth + 1)?;
        }
        let bytes = stackdata::to_bytes(&data);
        if bytes.len() > u16::MAX as usize { return Err(fail(StackProblem::ComponentsTooLarge)); }
        if stackdata::from_bytes(&bytes).as_ref() != Some(&data) { return Err(fail(StackProblem::LossyComponents)); }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay_catalog::{Profile, WHITE_SLAB_ITEM};
    fn preview() -> Catalog { Catalog::new(Profile::Preview26_3) }
    fn base() -> Catalog { Catalog::new(Profile::Java26_1_2) }
    fn child(item: i32) -> Content { Content { slot: 0, item, count: 3, data: None } }

    #[test]
    fn full_inventory_checks_every_slot_and_cursor_without_mutation() {
        for slot in 0..crate::inventory::SLOTS {
            let mut inventory = Inventory::new();
            inventory.set(slot as i32, Some(Stack::new(WHITE_SLAB_ITEM, 64)));
            let before = inventory.clone();
            assert!(preview().validate_inventory(&inventory).is_ok());
            let error = base().validate_inventory(&inventory).unwrap_err();
            assert_eq!(error.path, [StackLocation::Slot(slot)]);
            assert!(matches!(error.problem, StackProblem::Catalog(Error::Disabled { .. })));
            assert_eq!(inventory, before);
        }
        let mut inventory = Inventory::new();
        inventory.set_cursor(Some(Stack::new(WHITE_SLAB_ITEM, 1)));
        assert_eq!(base().validate_inventory(&inventory).unwrap_err().path, [StackLocation::Cursor]);
    }

    #[test]
    fn catalog_count_and_component_failures_are_explicit() {
        for item in [0, -1, 999_999] { assert!(preview().validate_stack(&Stack::new(item, 1)).is_err()); }
        for count in [0, -1, 65] {
            assert!(matches!(preview().validate_stack(&Stack::new(WHITE_SLAB_ITEM, count)).unwrap_err().problem, StackProblem::Count { .. }));
        }
        let normal = Stack::new(WHITE_SLAB_ITEM, 37).with_extras(stackdata::Data { name: Some("Плита".into()), repair_cost: 7, ..Default::default() });
        assert!(preview().validate_stack(&normal).is_ok());
        let lossy = Stack::new(WHITE_SLAB_ITEM, 1).with_extras(stackdata::Data { map_post: Some(1), ..Default::default() });
        assert_eq!(preview().validate_stack(&lossy).unwrap_err().problem, StackProblem::LossyComponents);
        let huge = Stack::new(WHITE_SLAB_ITEM, 1).with_extras(stackdata::Data { name: Some("я".repeat(70_000)), ..Default::default() });
        assert!(preview().validate_stack(&huge).is_err());
    }

    #[test]
    fn every_nested_item_tree_obeys_the_world_catalog() {
        let item = crate::blocks::item_named("shulker_box").unwrap();
        for data in [
            stackdata::Data { contents: vec![child(WHITE_SLAB_ITEM)], ..Default::default() },
            stackdata::Data { bundle: vec![child(WHITE_SLAB_ITEM)], ..Default::default() },
            stackdata::Data { charged_arrow: Some(child(WHITE_SLAB_ITEM)), ..Default::default() },
            stackdata::Data { charged_projectiles: vec![stackdata::ChargedProjectile { content: child(WHITE_SLAB_ITEM), intangible: true }], ..Default::default() },
        ] {
            let stack = Stack::new(item, 1).with_extras(data);
            assert!(matches!(base().validate_stack(&stack).unwrap_err().problem, StackProblem::Catalog(Error::Disabled { .. })));
            // Некоторые сочетания зарядов старый кодек не поддерживает:
            // разрешение ID не даёт права молча потерять компонент.
            if let Err(error) = preview().validate_stack(&stack) { assert_eq!(error.problem, StackProblem::LossyComponents); }
        }
        let inner = Stack::new(item, 1).with_extras(stackdata::Data { contents: vec![child(WHITE_SLAB_ITEM)], ..Default::default() });
        let outer = Stack::new(item, 1).with_extras(stackdata::Data { bundle: vec![Content { count: 1, data: inner.data, ..child(inner.stored_item()) }], ..Default::default() });
        assert_eq!(base().validate_stack(&outer).unwrap_err().path, [StackLocation::Bundle(0), StackLocation::Container(0)]);
        let unknown = Stack::new(item, 1).with_extras(stackdata::Data { contents: vec![child(999_999)], ..Default::default() });
        assert!(matches!(preview().validate_stack(&unknown).unwrap_err().problem, StackProblem::Catalog(Error::UnknownInternal { .. })));
    }
}
