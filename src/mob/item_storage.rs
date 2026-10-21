//! Версия1 дополнения к старому телу моба: полные предметы без потери довесков.
use super::{Kind, Mob};
use crate::inventory::Stack;

const SCHEMA: u8 = 1;
const MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Debug)]
pub(super) struct Overlay {
    offers: Option<Vec<[Option<Stack>; 3]>>,
    trial_gear: Option<super::trial_gear::Gear>,
    mouth: Option<Option<Stack>>,
    held: Option<Option<Stack>>,
    cargo: Option<Vec<Option<Stack>>>,
}

fn needs_trial(mob: &Mob) -> bool {
    mob.h.trial_gear.as_ref().is_some_and(|gear| gear.iter().flatten().any(|stack| stack.count != 1))
}

pub(super) fn needed(mob: &Mob) -> bool {
    matches!(mob.kind, Kind::Villager | Kind::ZombieVillager | Kind::WanderingTrader | Kind::Fox)
        || super::land::animal(mob.kind) || needs_trial(mob)
}

fn flags(kind: Kind) -> u8 {
    u8::from(matches!(kind, Kind::Villager | Kind::ZombieVillager | Kind::WanderingTrader) || super::land::animal(kind))
        | (u8::from(kind == Kind::Fox) << 1)
        | (u8::from(super::land::animal(kind)) << 2)
        | (u8::from(super::land::animal(kind)) << 3)
}

/// Проверка представимости без выбранного игрового каталога и без записи байтов стопки.
fn validate_stack_without_writing(stack: &Stack) -> Result<(), &'static str> {
    let data = stack.data.map(|id| crate::stackdata::try_get(id).ok_or("неизвестные довески предмета")).transpose()?;
    if Stack::from_stored_strict(stack.stored_item(), stack.count, data.clone()) != Some(*stack) {
        return Err("предмет не кодируется без потерь");
    }
    if let Some(data) = &data {
        let encoded = crate::stackdata::to_bytes(data);
        if encoded.len() > u16::MAX as usize || crate::stackdata::from_bytes(&encoded).as_ref() != Some(data) {
            return Err("довески предмета не кодируются без потерь");
        }
    }
    Ok(())
}

pub(super) fn validate(mob: &Mob) -> Result<(), &'static str> {
    if let Some(gear) = &mob.equipment {
        for stack in gear.iter().flatten() { validate_stack_without_writing(stack)?; }
        // Писатель оборудования использует актуальную руку и состояние арбалета.
        for stack in super::equipment::all(mob).iter().flatten() { validate_stack_without_writing(stack)?; }
    }
    if let Some(gear) = &mob.h.trial_gear {
        for stack in gear.iter().flatten() { validate_stack_without_writing(stack)?; }
    }
    for stack in mob.stand.equipment.iter().flatten() { validate_stack_without_writing(stack)?; }
    for stack in mob.horse.slots.iter().flatten() { validate_stack_without_writing(stack)?; }
    for stack in mob.land.cargo.iter().flatten() { validate_stack_without_writing(stack)?; }
    for stack in [mob.small.held, mob.small.cargo, mob.wild.mouth, mob.land.held, mob.newmobs.cargo].iter().flatten() {
        validate_stack_without_writing(stack)?;
    }
    for offer in &mob.villager.offers {
        for stack in [Some(offer.buy), offer.buy_b, Some(offer.sell)].iter().flatten() {
            validate_stack_without_writing(stack)?;
        }
    }
    encode(mob).map(|_| ())
}

fn write_stack(stack: Option<Stack>, out: &mut Vec<u8>) -> Result<(), &'static str> {
    out.push(u8::from(stack.is_some()));
    if let Some(stack) = stack {
        validate_stack_without_writing(&stack)?;
        out.extend(stack.stored_item().to_be_bytes());
        out.extend(stack.count.to_be_bytes());
        stack.write_extras(out);
    }
    Ok(())
}

pub(super) fn encode(mob: &Mob) -> Result<Option<Vec<u8>>, &'static str> {
    if !needed(mob) { return Ok(None); }
    let mask = flags(mob.kind) | (u8::from(needs_trial(mob)) << 4);
    let mut out = vec![SCHEMA, mask];
    if mask & 1 != 0 {
        let count = u16::try_from(mob.villager.offers.len()).map_err(|_| "слишком много сделок")?;
        out.extend(count.to_be_bytes());
        for offer in &mob.villager.offers {
            for stack in [Some(offer.buy), offer.buy_b, Some(offer.sell)] { write_stack(stack, &mut out)?; }
            if out.len() > MAX_BYTES { return Err("слишком большое дополнение предметов моба"); }
        }
    }
    if mask & 2 != 0 { write_stack(mob.wild.mouth, &mut out)?; }
    if mask & 4 != 0 { write_stack(mob.land.held, &mut out)?; }
    if mask & 8 != 0 {
        let count = u16::try_from(mob.land.cargo.len()).map_err(|_| "слишком много ячеек груза")?;
        out.extend(count.to_be_bytes());
        for &stack in &mob.land.cargo {
            write_stack(stack, &mut out)?;
            if out.len() > MAX_BYTES { return Err("слишком большое дополнение предметов моба"); }
        }
    }
    if mask & 16 != 0 {
        for stack in mob.h.trial_gear.expect("полное снаряжение испытания") { write_stack(stack, &mut out)?; }
    }
    if out.len() > MAX_BYTES { return Err("слишком большое дополнение предметов моба"); }
    Ok(Some(out))
}

fn take<'a>(bytes: &mut &'a [u8], n: usize) -> Option<&'a [u8]> {
    let (head, tail) = bytes.split_at_checked(n)?;
    *bytes = tail;
    Some(head)
}
fn count(bytes: &mut &[u8]) -> Option<usize> { Some(usize::from(u16::from_be_bytes(take(bytes, 2)?.try_into().ok()?))) }
fn read_stack(bytes: &mut &[u8]) -> Option<Option<Stack>> {
    match take(bytes, 1)?[0] {
        0 => Some(None),
        1 => {
            let stored = i32::from_be_bytes(take(bytes, 4)?.try_into().ok()?);
            let count = i32::from_be_bytes(take(bytes, 4)?.try_into().ok()?);
            let extras = Stack::read_extras_strict(stored, |n| take(bytes, n))?;
            Some(Some(Stack::from_stored_strict(stored, count, extras)?))
        }
        _ => None,
    }
}

pub(super) fn read(bytes: &mut &[u8]) -> Option<Overlay> {
    let size = u32::from_be_bytes(take(bytes, 4)?.try_into().ok()?) as usize;
    if size > MAX_BYTES { return None; }
    let mut record = take(bytes, size)?;
    if take(&mut record, 1)?[0] != SCHEMA { return None; }
    let mask = take(&mut record, 1)?[0];
    if mask == 0 || mask & !31 != 0 { return None; }
    let offers = if mask & 1 != 0 {
        let n = count(&mut record)?;
        if n > record.len() / 3 { return None; }
        let mut offers = Vec::with_capacity(n);
        for _ in 0..n {
            let buy = read_stack(&mut record)?;
            let buy_b = read_stack(&mut record)?;
            let sell = read_stack(&mut record)?;
            if buy.is_none() || sell.is_none() { return None; }
            offers.push([buy, buy_b, sell]);
        }
        Some(offers)
    } else { None };
    let mouth = if mask & 2 != 0 { Some(read_stack(&mut record)?) } else { None };
    let held = if mask & 4 != 0 { Some(read_stack(&mut record)?) } else { None };
    let cargo = if mask & 8 != 0 {
        let n = count(&mut record)?;
        if n > record.len() { return None; }
        let mut cargo = Vec::with_capacity(n);
        for _ in 0..n { cargo.push(read_stack(&mut record)?); }
        Some(cargo)
    } else { None };
    let trial_gear = if mask & 16 != 0 {
        Some([read_stack(&mut record)?, read_stack(&mut record)?, read_stack(&mut record)?])
    } else { None };
    if !record.is_empty() { return None; }
    Some(Overlay { offers, mouth, held, cargo, trial_gear })
}

pub(super) fn apply(mob: &mut Mob, overlay: Overlay) -> Option<()> {
    let mask = u8::from(overlay.offers.is_some()) | (u8::from(overlay.mouth.is_some()) << 1)
        | (u8::from(overlay.held.is_some()) << 2) | (u8::from(overlay.cargo.is_some()) << 3)
        | (u8::from(overlay.trial_gear.is_some()) << 4);
    if mask & 15 != flags(mob.kind) || (overlay.trial_gear.is_some() && mob.h.trial_gear.is_none()) { return None; }
    if let Some(offers) = overlay.offers {
        if offers.len() != mob.villager.offers.len() { return None; }
        for (offer, [buy, buy_b, sell]) in mob.villager.offers.iter_mut().zip(offers) {
            offer.buy = buy?; offer.buy_b = buy_b; offer.sell = sell?;
        }
    }
    if let Some(mouth) = overlay.mouth { mob.wild.mouth = mouth; }
    if let Some(held) = overlay.held { mob.land.held = held; }
    if let Some(cargo) = overlay.cargo { mob.land.cargo = cargo; }
    if let Some(gear) = overlay.trial_gear { mob.h.trial_gear = Some(gear); }
    Some(())
}

#[cfg(test)]
#[path = "item_storage_tests.rs"]
mod tests;
