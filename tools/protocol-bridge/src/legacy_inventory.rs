//! Транзакции340: предсказанный предмет — только проверка, не источник вещей.
//! Обычные щелчки/ACK наблюдены probe_legacy_inventory.py на официальном340.
use crate::{items, legacy_items, wire::{Cursor, Error, uint}, JavaVersion};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Click {
    pub window: u8,
    pub slot: i16,
    pub button: u8,
    pub transaction: i16,
    pub mode: u8,
    pub expected: Option<items::Stack>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Confirmation { pub window: u8, pub transaction: i16, pub accepted: bool }

impl Confirmation {
    pub fn packet(self) -> Vec<u8> {
        let mut out = vec![0x11, self.window];
        out.extend(self.transaction.to_be_bytes()); out.push(u8::from(self.accepted)); out
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Action { Click(Click), Acknowledge(Confirmation) }

fn word(input: &mut Cursor<'_>) -> Result<i16, Error> {
    Ok(i16::from_be_bytes(input.take(2)?.try_into().map_err(|_| Error::Invalid)?))
}

/// Неизвестные режимы/окна пока отклоняются явно, а не подтверждаются вслепую.
/// ACK не имеет современного пакета: обрабатывается типизированно в ядре.
pub fn decode(body: &[u8], context: legacy_items::Context<'_>) -> Result<(Vec<u8>, Option<Action>), Error> {
    if body.len() > 1_048_576 { return Err(Error::TooLarge); }
    let mut input = Cursor::new(body); let id = input.uint()?;
    let (out, action) = match id {
        0x05 => {
            let ack = Confirmation { window: input.take(1)?[0], transaction: word(&mut input)?, accepted: input.boolean()? };
            (Vec::new(), Some(Action::Acknowledge(ack)))
        }
        0x07 => {
            let window = input.take(1)?[0]; let slot = word(&mut input)?;
            let button = input.take(1)?[0]; let transaction = word(&mut input)?; let mode = input.uint()?;
            if window != 0 { return Err(Error::PayloadUnavailable("legacy inventory window")); }
            let valid_button = match mode { 0 | 1 | 4 => button <= 1, 2 => button <= 8, 3 => button == 2,
                5 => matches!(button, 0..=2 | 4..=6 | 8..=10), 6 => button == 0, _ => false };
            if !valid_button || !((0..=45).contains(&slot) || (slot == -999 && matches!(mode, 0 | 5))) { return Err(Error::Invalid); }
            let (expected, n) = legacy_items::decode(input.rest, context).map_err(Error::LegacyItem)?; input.take(n)?;
            // Без современных хэшей: ядро вычисляет действие само и сверяет
            // старое предсказание отдельно. transaction не становится state ID.
            let mut out = vec![0x12, window, 0]; out.extend(slot.to_be_bytes()); out.push(button); uint(&mut out, mode);
            (out, Some(Action::Click(Click { window, slot, button, transaction, mode: mode as u8, expected })))
        }
        0x1b => {
            let slot = word(&mut input)?;
            if !(slot == -1 || (1..=45).contains(&slot)) { return Err(Error::Invalid); }
            let (stack, n) = legacy_items::decode(input.rest, context).map_err(Error::LegacyItem)?; input.take(n)?;
            let mut out = vec![0x38]; out.extend(slot.to_be_bytes());
            out.extend(items::encode(JavaVersion::V26_1_2, stack.as_ref(), true, items::Context {
                shared_enchantment_count: Some(context.enchantments.len() as u32),
            }).map_err(|_| Error::Invalid)?);
            (out, None)
        }
        _ => return Err(Error::PayloadUnavailable("legacy inventory packet")),
    };
    if input.remaining() != 0 { return Err(Error::Invalid); }
    Ok((out, action))
}

/// Порядок независимых компонентов не меняет предмет. Значения и количество
/// сравниваются полностью; неизвестные данные не отбрасываются.
pub fn same_stack(left: &Option<items::Stack>, right: &Option<items::Stack>) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(a), Some(b)) => a.item == b.item && a.count == b.count
            && a.added.len() == b.added.len() && a.removed.len() == b.removed.len()
            && a.added.iter().all(|v| b.added.contains(v)) && a.removed.iter().all(|v| b.removed.contains(v)),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn context() -> legacy_items::Context<'static> { legacy_items::Context { enchantments: &[] } }
    #[test]
    fn observed_pickup_and_negative_ack_have_distinct_channels() {
        let body = [7, 0, 0, 36, 0, 0, 1, 0, 0, 1, 8, 0, 0, 0];
        let (native, action) = decode(&body, context()).unwrap();
        assert_eq!(native, [0x12, 0, 0, 0, 36, 0, 0]);
        let Some(Action::Click(click)) = action else { panic!() };
        assert_eq!(click.expected.unwrap().count, 8); assert_eq!(click.transaction, 1);
        let (native, action) = decode(&[5, 0, 0, 3, 0], context()).unwrap();
        assert!(native.is_empty());
        let Some(Action::Acknowledge(ack)) = action else { panic!() };
        assert_eq!(ack.packet(), [0x11, 0, 0, 3, 0]);
        for end in 0..body.len() { assert!(decode(&body[..end], context()).is_err()); }
        let mut extra = body.to_vec(); extra.push(0); assert!(decode(&extra, context()).is_err());
        let mut unsupported = body; unsupported[7] = 7; assert!(decode(&unsupported, context()).is_err());
        assert!(decode(&[5, 0, 0, 3, 2], context()).is_err());
    }
}
