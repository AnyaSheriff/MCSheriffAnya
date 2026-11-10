//! Закрытый trusted Slot/SetSlot767↔768↔769: простые предметы, износ и plain text.
//! Схемы/источники: testdata/slot-adjacent-layouts.json. Не JavaVersion alias:
//! чужой payload не читается кодеком769; общие примитивы проверяет component_data.
//! Food/custom_model_data/fire_resistant/banner и остальные формы пока отказ.
use std::collections::BTreeSet;
use crate::{JavaVersion, ids::{self, RegistryKind, IdTranslationError}, wire};
use super::{Component, Context, Error, Reader, Stack, LIMIT, uint};

#[path="generated_slot_adjacent.rs"]
mod tables;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Layout { V767, V768, V769 }

impl Layout {
    fn protocol(self) -> u16 { match self { Self::V767 => 767, Self::V768 => 768, Self::V769 => 769 } }
}

fn source_count(kind: RegistryKind, protocol: u16) -> Result<u32, Error> {
    let neighbor = if protocol == 769 { 768 } else { protocol + 1 };
    tables::ranges(kind, protocol, neighbor).map(|value| value.0).ok_or(Error::Invalid)
}

fn map(kind: RegistryKind, from: Layout, to: Layout, mut value: u32) -> Result<u32, Error> {
    let mut source = from.protocol();
    let target = to.protocol();
    if value >= source_count(kind, source)? { return Err(Error::Id(IdTranslationError::UnknownId(value))); }
    while source != target {
        let next = if source < target { source + 1 } else { source - 1 };
        let (count, ranges) = tables::ranges(kind, source, next).ok_or(Error::Invalid)?;
        if value >= count { return Err(Error::Id(IdTranslationError::UnknownId(value))); }
        let &(_, _, offset) = ranges.iter().find(|&&(first,last,_)| (first..=last).contains(&value))
            .ok_or(Error::Id(IdTranslationError::UnknownId(value)))?;
        if offset == 65535 { return Err(Error::Id(IdTranslationError::MissingTarget(value))); }
        value = u32::try_from(i64::from(value) + i64::from(offset)).map_err(|_| Error::Invalid)?;
        source = next;
    }
    Ok(value)
}

fn simple(layout: Layout, kind: u32) -> Result<(), Error> {
    if kind >= source_count(RegistryKind::Component, layout.protocol())? {
        return Err(Error::Id(IdTranslationError::UnknownId(kind)));
    }
    if !tables::simple(layout.protocol()).ok_or(Error::Invalid)?.contains(&kind) {
        return Err(Error::UnsupportedComponent(kind));
    }
    Ok(())
}

fn canonical_component(layout: Layout, kind: u32) -> Result<u32, Error> {
    simple(layout, kind)?;
    // Только ID по имени: ни одна раскладка768/767 не объявляется payload769.
    let reference = map(RegistryKind::Component, layout, Layout::V769, kind)?;
    ids::translate(RegistryKind::Component, JavaVersion::V1_21_4, wire::INTERNAL, reference).map_err(Error::Id)
}

fn target_component(layout: Layout, kind: u32) -> Result<u32, Error> {
    let reference = ids::translate(RegistryKind::Component, wire::INTERNAL, JavaVersion::V1_21_4, kind).map_err(Error::Id)?;
    simple(Layout::V769, reference)?;
    map(RegistryKind::Component, Layout::V769, layout, reference)
}

/// Один trusted Slot, с числом прочитанных байтов для будущих массивов окон.
/// Результат использует настоящий canonical Stack; неизвестные вещи не воздух.
pub fn decode(layout: Layout, bytes: &[u8]) -> Result<(Option<Stack>, usize), Error> {
    if bytes.len() > LIMIT { return Err(Error::TooLarge); }
    let mut input = Reader(bytes);
    let count = input.count(127)? as u32;
    if count == 0 { return Ok((None, bytes.len() - input.0.len())); }
    let reference = map(RegistryKind::Item, layout, Layout::V769, input.uint()?)?;
    let item = ids::translate(RegistryKind::Item, JavaVersion::V1_21_4, wire::INTERNAL, reference).map_err(Error::Id)?;
    if item == 0 { return Err(Error::Invalid); }
    let added_count = input.count(256)?;
    let removed_count = input.count(256)?;
    let mut seen = BTreeSet::new();
    let mut added = Vec::new();
    let mut removed = Vec::new();
    for _ in 0..added_count {
        let kind = canonical_component(layout, input.uint()?)?;
        if !seen.insert(kind) { return Err(Error::Invalid); }
        let start = input.0;
        super::component_data(kind, &mut input, Context::default())?;
        added.push(Component { kind, data: start[..start.len() - input.0.len()].to_vec() });
    }
    for _ in 0..removed_count {
        let kind = canonical_component(layout, input.uint()?)?;
        if !seen.insert(kind) { return Err(Error::Invalid); }
        removed.push(kind);
    }
    Ok((Some(Stack { item, count, added, removed }), bytes.len() - input.0.len()))
}

pub fn encode(layout: Layout, stack: Option<&Stack>) -> Result<Vec<u8>, Error> {
    let Some(stack) = stack else { return Ok(vec![0]); };
    if !(1..=127).contains(&stack.count) || stack.item == 0 || stack.added.len() > 256 || stack.removed.len() > 256 {
        return Err(Error::Invalid);
    }
    let reference = ids::translate(RegistryKind::Item, wire::INTERNAL, JavaVersion::V1_21_4, stack.item).map_err(Error::Id)?;
    let item = map(RegistryKind::Item, Layout::V769, layout, reference)?;
    let mut out = Vec::new();
    uint(&mut out, stack.count); uint(&mut out, item);
    uint(&mut out, stack.added.len() as u32); uint(&mut out, stack.removed.len() as u32);
    let mut seen = BTreeSet::new();
    for component in &stack.added {
        let kind = target_component(layout, component.kind)?;
        if !seen.insert(component.kind) { return Err(Error::Invalid); }
        if component.data.len() > LIMIT { return Err(Error::TooLarge); }
        let mut input = Reader(&component.data);
        super::component_data(component.kind, &mut input, Context::default())?;
        if !input.0.is_empty() { return Err(Error::Invalid); }
        uint(&mut out, kind); out.extend(&component.data);
        if out.len() > LIMIT { return Err(Error::TooLarge); }
    }
    for &component in &stack.removed {
        let kind = target_component(layout, component)?;
        if !seen.insert(component) { return Err(Error::Invalid); }
        uint(&mut out, kind);
    }
    if out.len() > LIMIT { return Err(Error::TooLarge); }
    Ok(out)
}

pub fn translate_slot(from: Layout, to: Layout, payload: &[u8]) -> Result<Vec<u8>, Error> {
    let (stack, used) = decode(from, payload)?;
    if used != payload.len() { return Err(Error::Invalid); }
    encode(to, stack.as_ref())
}

fn read_window(input: &mut wire::Cursor<'_>, layout: Layout, reason: &'static str) -> Result<i32, wire::Error> {
    let window = if layout == Layout::V767 { i32::from(input.take(1)?[0]) } else { input.varint()? };
    if !(0..=127).contains(&window) { return Err(wire::Error::PayloadUnavailable(reason)); }
    Ok(window)
}

fn put_window(out: &mut Vec<u8>, layout: Layout, window: i32) {
    if layout == Layout::V767 { out.push(window as u8); } else { wire::uint(out, window as u32); }
}

/// Тело clientbound SetSlot без packet ID; только обычные окна0..127.
/// Старые255/254 и новые−1/−2 требуют отдельного контекста, а не приведения u8.
pub fn set_slot(from: Layout, to: Layout, payload: &[u8]) -> Result<Vec<u8>, wire::Error> {
    if payload.len() > LIMIT { return Err(wire::Error::TooLarge); }
    let mut input = wire::Cursor::new(payload);
    let window = read_window(&mut input, from, "adjacent SetSlot reserved window requires context")?;
    let revision = input.varint()?;
    let slot = input.take(2)?;
    let (stack, used) = decode(from, input.rest).map_err(wire::Error::Item)?;
    input.take(used)?;
    if input.remaining() != 0 { return Err(wire::Error::Invalid); }
    let mut out = Vec::new();
    put_window(&mut out, to, window);
    wire::uint(&mut out, revision as u32); out.extend(slot);
    out.extend(encode(to, stack.as_ref()).map_err(wire::Error::Item)?);
    if out.len() > LIMIT { return Err(wire::Error::TooLarge); }
    Ok(out)
}

/// Полный clientbound WindowItems с массивом и carriedSlot. Никаких изменений
/// серверного инвентаря: результат возвращается только после проверки всего тела.
/// Это всё тот же trusted subset, а не serverbound предсказание щелчка.
pub fn window_items(from: Layout, to: Layout, payload: &[u8]) -> Result<Vec<u8>, wire::Error> {
    if payload.len() > LIMIT { return Err(wire::Error::TooLarge); }
    let mut input = wire::Cursor::new(payload);
    let window = read_window(&mut input, from, "adjacent WindowItems reserved window requires context")?;
    let revision = input.varint()?;
    let count = input.count(4096)?;
    let mut out = Vec::with_capacity(payload.len());
    put_window(&mut out, to, window);
    wire::uint(&mut out, revision as u32);
    wire::uint(&mut out, count as u32);
    // Последний Slot — carriedItem, даже при пустом массиве.
    for _ in 0..=count {
        let (stack, used) = decode(from, input.rest).map_err(wire::Error::Item)?;
        input.take(used)?;
        let encoded = encode(to, stack.as_ref()).map_err(wire::Error::Item)?;
        if out.len().checked_add(encoded.len()).is_none_or(|length| length > LIMIT) {
            return Err(wire::Error::TooLarge);
        }
        out.extend(encoded);
    }
    if input.remaining() != 0 { return Err(wire::Error::Invalid); }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    const LAYOUTS: [Layout;3] = [Layout::V767,Layout::V768,Layout::V769];

    fn fixture() -> serde_json::Value {
        serde_json::from_str(include_str!("../testdata/slot-adjacent-layouts.json")).unwrap()
    }

    fn bytes(value: &str) -> Vec<u8> {
        value.as_bytes().as_chunks::<2>().0.iter().map(|part|
            u8::from_str_radix(std::str::from_utf8(part).unwrap(),16).unwrap()).collect()
    }

    fn slot(layout: Layout) -> Vec<u8> {
        bytes(fixture()["synthetic_slot_hex"][layout.protocol().to_string()].as_str().unwrap())
    }

    #[test]
    fn pinned_simple_payloads_are_equal_and_unsupported_names_are_explicit() {
        let data = fixture();
        for name in data["simple_names"].as_array().unwrap() {
            let name = name.as_str().unwrap();
            assert_eq!(data["versions"]["767"]["components"][name],data["versions"]["768"]["components"][name]);
            assert_eq!(data["versions"]["768"]["components"][name],data["versions"]["769"]["components"][name]);
        }
        assert_eq!(data["versions"]["767"]["container_id"], "u8");
        assert_eq!(data["versions"]["768"]["container_id"], "varint");
        assert_ne!(data["versions"]["767"]["components"]["food"], data["versions"]["768"]["components"]["food"]);
        assert_ne!(data["versions"]["767"]["components"]["custom_model_data"], data["versions"]["768"]["components"]["custom_model_data"]);
        assert_ne!(data["versions"]["768"]["banner_pattern_layer"], data["versions"]["769"]["banner_pattern_layer"]);
    }

    #[test]
    fn diamond_805_827_836_damage_names_lore_and_removed_components_roundtrip() {
        let reference = decode(Layout::V769,&slot(Layout::V769)).unwrap().0.unwrap();
        assert!(!reference.added.is_empty()); assert!(!reference.removed.is_empty());
        for from in LAYOUTS {
            let original = slot(from);
            let (stack,used) = decode(from,&original).unwrap();
            assert_eq!(used,original.len()); assert_eq!(stack.as_ref(),Some(&reference));
            for to in LAYOUTS {
                let translated = translate_slot(from,to,&original).unwrap();
                assert_eq!(translated,slot(to));
                assert_eq!(translate_slot(to,from,&translated).unwrap(),original);
            }
        }
    }

    #[test]
    fn every_exact_item_and_component_map_reverses_or_explicitly_refuses() {
        for (from,to) in [(Layout::V767,Layout::V768),(Layout::V768,Layout::V769)] {
            for (source,target) in [(from,to),(to,from)] {
                for kind in [RegistryKind::Item,RegistryKind::Component] {
                    let count = source_count(kind,source.protocol()).unwrap();
                    for value in 0..count {
                        match map(kind,source,target,value) {
                            Ok(mapped) => assert_eq!(map(kind,target,source,mapped).unwrap(),value),
                            Err(Error::Id(IdTranslationError::MissingTarget(missing))) => assert_eq!(missing,value),
                            other => panic!("unexpected mapping result: {other:?}"),
                        }
                    }
                    assert_eq!(map(kind,source,target,count),Err(Error::Id(IdTranslationError::UnknownId(count))));
                }
            }
        }
        let data = fixture();
        let missing = data["pale_oak_log_769"].as_u64().unwrap() as u32;
        assert!(matches!(map(RegistryKind::Item,Layout::V769,Layout::V767,missing),Err(Error::Id(IdTranslationError::MissingTarget(_)))));
    }

    #[test]
    fn empty_slots_prefix_arrays_and_normal_window_set_slot_keep_headers() {
        for from in LAYOUTS {
            for to in LAYOUTS {
                assert_eq!(translate_slot(from,to,&[0]).unwrap(),vec![0]);
                assert_eq!(decode(from,&[0,42]).unwrap(),(None,1));
                for revision in [i32::MIN,-1,0,128,i32::MAX] {
                    for window in [0u8,1,127] {
                        let mut body = vec![window]; uint(&mut body,revision as u32);
                        body.extend((-7i16).to_be_bytes()); body.extend(slot(from));
                        let converted = set_slot(from,to,&body).unwrap();
                        assert_eq!(set_slot(to,from,&converted).unwrap(),body);
                        assert_eq!(converted[0],window);
                    }
                }
            }
        }
    }

    #[test]
    fn special_windows_do_not_wrap_between_u8_and_signed_varint() {
        for from in LAYOUTS {
            for window in [-2i32,-1,128,254,255] {
                if from == Layout::V767 && window < 0 { continue; }
                let mut body = Vec::new();
                if from == Layout::V767 { body.push(window as u8); } else { uint(&mut body,window as u32); }
                body.extend([0,0,0,0]);
                assert_eq!(set_slot(from,Layout::V769,&body),Err(wire::Error::PayloadUnavailable("adjacent SetSlot reserved window requires context")));
            }
        }
    }

    #[test]
    fn unsupported_added_removed_duplicate_and_conflicting_components_refuse() {
        let data = fixture();
        for layout in LAYOUTS {
            let item = data["diamond_ids"][layout.protocol().to_string()].as_u64().unwrap() as u32;
            for kind in data["versions"][layout.protocol().to_string()]["unsupported_ids"].as_array().unwrap() {
                let kind = kind.as_u64().unwrap() as u32;
                for added in [false,true] {
                    let mut value = vec![1]; uint(&mut value,item);
                    value.extend(if added {[1,0]} else {[0,1]}); uint(&mut value,kind);
                    assert_eq!(decode(layout,&value),Err(Error::UnsupportedComponent(kind)));
                }
            }
            let mut duplicate = vec![1]; uint(&mut duplicate,item); duplicate.extend([2,0,3,1,3,2]);
            assert_eq!(decode(layout,&duplicate),Err(Error::Invalid));
            let mut conflict = vec![1]; uint(&mut conflict,item); conflict.extend([1,1,3,1,3]);
            assert_eq!(decode(layout,&conflict),Err(Error::Invalid));
            let mut duplicate_removed = vec![1]; uint(&mut duplicate_removed,item); duplicate_removed.extend([0,2,3,3]);
            assert_eq!(decode(layout,&duplicate_removed),Err(Error::Invalid));
        }
    }

    #[test]
    fn truncation_trailing_varint_counts_unknown_ids_and_limits_refuse() {
        for layout in LAYOUTS {
            let body = slot(layout);
            for length in 0..body.len() { assert!(translate_slot(layout,Layout::V769,&body[..length]).is_err()); }
            let mut trailing = body; trailing.push(0);
            assert_eq!(translate_slot(layout,Layout::V769,&trailing),Err(Error::Invalid));
            for malformed in [vec![128],vec![255;6],vec![255,255,255,255,16],vec![128,1]] {
                assert!(decode(layout,&malformed).is_err());
            }
            let mut unknown = vec![1]; uint(&mut unknown,u32::MAX); unknown.extend([0,0]);
            assert!(decode(layout,&unknown).is_err());
            assert_eq!(decode(layout,&vec![0;LIMIT+1]),Err(Error::TooLarge));
        }
    }

    #[test]
    fn set_slot_headers_and_complete_payload_have_strict_boundaries() {
        for from in LAYOUTS {
            let mut body = vec![0]; uint(&mut body,300); body.extend([0,5]); body.extend(slot(from));
            for length in 0..body.len() { assert!(set_slot(from,Layout::V769,&body[..length]).is_err()); }
            body.push(0);
            assert_eq!(set_slot(from,Layout::V769,&body),Err(wire::Error::Invalid));
            assert_eq!(set_slot(from,Layout::V769,&vec![0;LIMIT+1]),Err(wire::Error::TooLarge));
            let mut revision = vec![0]; revision.extend([255,255,255,255,16]);
            assert_eq!(set_slot(from,Layout::V769,&revision),Err(wire::Error::Invalid));
        }
    }

    #[test]
    fn canonical_encoder_and_text_validator_do_not_drop_unknown_or_extra_data() {
        let mut stack = decode(Layout::V769,&slot(Layout::V769)).unwrap().0.unwrap();
        let duplicate = stack.added[0].clone(); stack.added.push(duplicate);
        assert_eq!(encode(Layout::V768,Some(&stack)),Err(Error::Invalid));
        stack.added.pop();
        stack.removed.clear();
        stack.added.push(Component{kind:19,data:vec![0,0]});
        assert_eq!(encode(Layout::V768,Some(&stack)),Err(Error::Invalid));
        let mut raw = vec![1]; uint(&mut raw,836); raw.extend([1,0,5]);
        // Пустой compound не является plain text: неизвестные NBT не пролетают.
        raw.extend([10,0]);
        assert!(matches!(decode(Layout::V769,&raw),Err(Error::Invalid)));
        let hover = bytes(fixture()["unsupported_hover_slot_769_hex"].as_str().unwrap());
        assert_eq!(decode(Layout::V769,&hover),Err(Error::UnsupportedComponent(6)));
    }
    fn inventory(layout: Layout, revision: i32, window: u8) -> Vec<u8> {
        let mut out = vec![window]; uint(&mut out,revision as u32); uint(&mut out,46);
        for index in 0..46 {
            if index % 5 == 0 { out.extend(slot(layout)); } else { out.push(0); }
        }
        out.extend(slot(layout));
        out
    }

    #[test]
    fn pinned_window_items_fields_and_independent_initial_inventory_pairs_match() {
        let data = fixture();
        for protocol in ["767","768","769"] {
            let fields = &data["versions"][protocol]["window_items"];
            assert_eq!(fields[1][0]["name"], "windowId");
            assert_eq!(fields[1][1]["name"], "stateId");
            assert_eq!(fields[1][2]["name"], "items");
            assert_eq!(fields[1][2]["type"][1]["type"], "Slot");
            assert_eq!(fields[1][3]["name"], "carriedItem");
        }
        assert_eq!(data["versions"]["769"]["container_id"], "varint");
        for from in LAYOUTS {
            let source = bytes(data["synthetic_window_items_hex"][from.protocol().to_string()].as_str().unwrap());
            assert_eq!(source,inventory(from,300,0));
            for to in LAYOUTS {
                let target = bytes(data["synthetic_window_items_hex"][to.protocol().to_string()].as_str().unwrap());
                assert_eq!(window_items(from,to,&source).unwrap(),target);
                assert_eq!(window_items(to,from,&target).unwrap(),source);
            }
        }
    }

    #[test]
    fn window_items_preserves_46_slots_carried_item_and_signed_revisions() {
        for from in LAYOUTS {
            for to in LAYOUTS {
                for revision in [i32::MIN,-1,0,300,i32::MAX] {
                    for window in [0u8,1,127] {
                        let source = inventory(from,revision,window);
                        let target = window_items(from,to,&source).unwrap();
                        assert_eq!(target,inventory(to,revision,window));
                        assert_eq!(window_items(to,from,&target).unwrap(),source);
                    }
                }
                assert_eq!(window_items(from,to,&[0,0,0,0]).unwrap(),vec![0,0,0,0]);
            }
        }
    }

    #[test]
    fn bad_slot_or_carried_component_rejects_the_entire_window_items_packet() {
        for from in LAYOUTS {
            let data = fixture();
            let item = data["diamond_ids"][from.protocol().to_string()].as_u64().unwrap() as u32;
            let kind = data["versions"][from.protocol().to_string()]["unsupported_ids"][0].as_u64().unwrap() as u32;
            let mut unsupported = vec![1]; uint(&mut unsupported,item); unsupported.extend([1,0]); uint(&mut unsupported,kind);
            let mut carried = vec![0,0,1,0]; carried.extend(&unsupported);
            assert_eq!(window_items(from,Layout::V769,&carried),Err(wire::Error::Item(Error::UnsupportedComponent(kind))));
            let mut contents = vec![0,0,1]; contents.extend(&unsupported); contents.push(0);
            assert_eq!(window_items(from,Layout::V769,&contents),Err(wire::Error::Item(Error::UnsupportedComponent(kind))));
        }
    }

    #[test]
    fn window_items_truncation_tail_counts_limits_and_special_windows_refuse() {
        for from in LAYOUTS {
            let source = inventory(from,300,0);
            for length in 0..source.len() { assert!(window_items(from,Layout::V769,&source[..length]).is_err()); }
            let mut tail = source; tail.push(0);
            assert_eq!(window_items(from,Layout::V769,&tail),Err(wire::Error::Invalid));
            let mut count = vec![0,0]; uint(&mut count,4097);
            assert_eq!(window_items(from,Layout::V769,&count),Err(wire::Error::TooLarge));
            assert_eq!(window_items(from,Layout::V769,&vec![0;LIMIT+1]),Err(wire::Error::TooLarge));
            let mut reserved = Vec::new();
            if from == Layout::V767 { reserved.push(255); } else { uint(&mut reserved,(-1i32) as u32); }
            reserved.extend([0,0,0]);
            assert_eq!(window_items(from,Layout::V769,&reserved),Err(wire::Error::PayloadUnavailable("adjacent WindowItems reserved window requires context")));
        }
    }

}
