//! Ограниченный хвост курсора для собственного playerdata v10.
use std::io;
use crate::inventory::Stack;

const MAX_PAYLOAD: usize = 1 + 4 + 4 + 2 + u16::MAX as usize;

fn valid(stack: &Stack) -> bool {
    stack.item > 0 && crate::blocks::item_name(stack.item).is_some()
        && (1..=stack.limit()).contains(&stack.count)
        && stack.tnt.is_none_or(|kind| kind.0 <= 0x7f && crate::custom_tnt::by_id(kind).is_some())
}

pub(super) fn append(stack: Option<Stack>, out: &mut Vec<u8>) -> io::Result<()> {
    let mut payload = Vec::new();
    match stack {
        None => payload.push(0),
        Some(stack) => {
            if !valid(&stack) {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "неверная стопка курсора игрока"));
            }
            payload.push(1);
            payload.extend_from_slice(&stack.stored_item().to_be_bytes());
            payload.extend_from_slice(&stack.count.to_be_bytes());
            if let Some(data) = stack.data {
                let data = crate::stackdata::get(data);
                let extras = crate::stackdata::to_bytes(&data);
                let length = u16::try_from(extras.len()).map_err(|_| io::Error::new(
                    io::ErrorKind::InvalidData, "компоненты курсора игрока превышают 65535 байт"))?;
                // Старый общий кодек отдельных полей использует min/take.
                // Общая длина меньше u16::MAX ещё не доказывает, что он
                // сохранил все символы/записи. Проверяем исходные данные до
                // добавления хвоста и до замены предыдущего файла игрока.
                if crate::stackdata::from_bytes(&extras).as_ref() != Some(&data) {
                    return Err(io::Error::new(io::ErrorKind::InvalidData,
                        "компоненты курсора нельзя сохранить без изменения"));
                }
                payload.extend_from_slice(&length.to_be_bytes());
                payload.extend_from_slice(&extras);
            }
        }
    }
    out.extend_from_slice(&payload);
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    Ok(())
}

/// Снимает хвост только после полного успешного разбора. Неизвестный предмет,
/// неверная длина/число и лишние байты не превращаются в пустой курсор.
pub(super) fn take(tail: &mut &[u8]) -> Option<Option<Stack>> {
    let length_at = tail.len().checked_sub(4)?;
    let length = u32::from_be_bytes(tail.get(length_at..)?.try_into().ok()?) as usize;
    if !(1..=MAX_PAYLOAD).contains(&length) { return None; }
    let start = length_at.checked_sub(length)?;
    let payload = tail.get(start..length_at)?;
    let stack = parse(payload)?;
    *tail = &tail[..start];
    Some(stack)
}

fn parse(payload: &[u8]) -> Option<Option<Stack>> {
    if payload == [0] { return Some(None); }
    if payload.first() != Some(&1) { return None; }
    let stored = i32::from_be_bytes(payload.get(1..5)?.try_into().ok()?);
    let count = i32::from_be_bytes(payload.get(5..9)?.try_into().ok()?);
    let mut stack = Stack::from_stored(stored, count);
    // from_stored старого формата заменяет неизвестный custom TNT обычным.
    // Новый хвост не должен молча менять семантику предмета.
    let custom = ((stored >> 24) & 0x7f) as u8;
    if stack.tnt.map_or(0, |kind| kind.0) != custom
        || stack.item <= 0 || crate::blocks::item_name(stack.item).is_none() { return None; }
    let mut offset: usize = 9;
    let extras = Stack::read_extras(stored, |length| {
        let bytes = payload.get(offset..offset.checked_add(length)?)?;
        offset += length;
        Some(bytes)
    })?;
    if offset != payload.len() { return None; }
    if let Some(extras) = extras {
        // Записи v10 создаются текущим кодеком: отклоняем хвост, который он
        // прочитал бы лишь частично, оставив неизвестные компоненты за кадром.
        if crate::stackdata::to_bytes(&extras) != payload.get(11..)? { return None; }
        stack = stack.with_extras(extras);
    }
    // Предел количества проверяется с восстановленными компонентами, чтобы
    // будущий компонент размера стопки не оценивался по пустому предмету.
    if !valid(&stack) { return None; }
    Some(Some(stack))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn malformed_cursor_tail_does_not_become_an_empty_stack() {
        let mut bytes = Vec::new();
        append(Some(Stack::new(crate::blocks::item_named("diamond").unwrap(), 7)), &mut bytes).unwrap();
        for truncated in 0..bytes.len() {
            let mut slice = &bytes[..truncated];
            assert!(take(&mut slice).is_none(), "обрезано до {truncated}");
        }
        for payload in [vec![2], vec![0, 0], vec![1], vec![1, 0, 0, 0, 0, 0, 0, 0, 1]] {
            let mut malformed = payload.clone();
            malformed.extend_from_slice(&(payload.len() as u32).to_be_bytes());
            let mut slice = malformed.as_slice();
            assert!(take(&mut slice).is_none());
            assert_eq!(slice, malformed);
        }
        for length in [0, (MAX_PAYLOAD + 1) as u32, u32::MAX] {
            let mut malformed = bytes.clone();
            let end = malformed.len();
            malformed[end - 4..].copy_from_slice(&length.to_be_bytes());
            assert!(take(&mut malformed.as_slice()).is_none());
        }
        let mut wrong_count = bytes.clone();
        wrong_count[5..9].copy_from_slice(&65i32.to_be_bytes());
        assert!(take(&mut wrong_count.as_slice()).is_none());
    }

    #[test]
    fn normal_component_representations_remain_lossless() {
        for spec in [
            "diamond_sword[damage=5,enchantments={sharpness:3},attribute_modifiers=[{type:attack_damage,id:mcsa:test,amount:2,operation:add_value,slot:mainhand}]]",
            "potion[potion_contents={}]",
            "potion[potion_contents={potion:healing,custom_effects:[{id:poison,duration:-1,ambient:true}]}]",
            "firework_rocket[fireworks={flight_duration:2,explosions:[]}]",
        ] {
            let (name, data) = crate::stackdata::parse_item_spec(spec).unwrap();
            let stack = Stack::new(crate::blocks::item_named(&name).unwrap(), 1).with_extras(data);
            let mut bytes = Vec::new();
            append(Some(stack), &mut bytes).unwrap();
            assert_eq!(take(&mut bytes.as_slice()), Some(Some(stack)), "{spec}");
        }
        // Смешанные заряды проходят тот же существующий самостоятельный
        // кодек; не сравниваем только bytes, которые могли уже потерять поле.
        let observed: serde_json::Value = serde_json::from_str(include_str!(
            "../../tools/protocol-bridge/testdata/crossbow-observations.json")).unwrap();
        for case in observed["cases"].as_array().unwrap() {
            let (name, data) = crate::stackdata::parse_item_spec(case["command"].as_str().unwrap()).unwrap();
            let stack = Stack::new(crate::blocks::item_named(&name).unwrap(), 1).with_extras(data);
            let mut bytes = Vec::new();
            append(Some(stack), &mut bytes).unwrap();
            assert_eq!(take(&mut bytes.as_slice()), Some(Some(stack)), "{}", case["name"]);
        }
    }

    #[test]
    fn cursor_tail_preserves_count_components_and_custom_tnt() {
        let diamond = Stack::new(crate::blocks::item_named("diamond").unwrap(), 37)
            .with_extras(crate::stackdata::Data { name: Some("Курсор после сбоя".into()), repair_cost: 17, ..Default::default() });
        let tnt = crate::custom_tnt::stack(crate::custom_tnt::by_name("cluster").unwrap(), 7).unwrap();
        for expected in [None, Some(diamond), Some(tnt)] {
            let mut bytes = vec![7, 8, 9];
            append(expected, &mut bytes).unwrap();
            let mut tail = bytes.as_slice();
            assert_eq!(take(&mut tail), Some(expected));
            assert_eq!(tail, [7, 8, 9]);
        }
    }
}
