//! Обычные Slot776 — восемь собственных official записей; nested — pinned shape.
use super::*;
use crate::JavaVersion::{V26_1_2 as OLD, V26_2 as MID, V26_3 as NEW};

fn bytes(hex: &str) -> Vec<u8> {
    assert_eq!(hex.len() % 2, 0);
    hex.as_bytes().as_chunks::<2>().0.iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap()).collect()
}
fn fixture() -> serde_json::Value {
    serde_json::from_str(include_str!("../testdata/item-26.2-observations.json")).unwrap()
}
fn context() -> Context { Context::default() }
fn slot(item: u32, kind: u32, data: &[u8]) -> Vec<u8> {
    let mut body = vec![1]; uint(&mut body, item); body.extend([1, 0]);
    uint(&mut body, kind); body.extend(data); body
}
fn template(raw: &[u8]) -> Vec<u8> {
    let mut input = Reader(raw);
    let count = input.uint().unwrap(); let item = input.uint().unwrap();
    assert_ne!(count, 0);
    let mut out = Vec::new(); uint(&mut out, item); uint(&mut out, count);
    out.extend(input.0); out
}

#[test]
fn eight_official_776_slots_preserve_exact_bytes_and_native_meaning() {
    let data = fixture();
    assert_eq!(data["protocol"], 776);
    let cases = data["cases"].as_array().unwrap(); assert_eq!(cases.len(), 8);
    for case in cases {
        let observed = bytes(case["slot_hex"].as_str().unwrap());
        let expected_native = bytes(case["native_slot_hex"].as_str().unwrap());
        let (stack, size) = decode(MID, &observed, false, context()).unwrap();
        assert_eq!(size, observed.len(), "{}", case["name"]);
        let (native, size) = decode(OLD, &expected_native, false, context()).unwrap();
        assert_eq!(size, expected_native.len()); assert_eq!(stack, native);
        if let Some(stack) = &stack { assert_eq!(u64::from(stack.item), case["native_item_id"].as_u64().unwrap()); }
        for (version, raw) in [(MID, observed), (OLD, expected_native)] {
            assert_eq!(encode(version, stack.as_ref(), false, context()).unwrap(), raw, "{}", case["name"]);
            for end in 0..raw.len() {
                assert!(decode(version, &raw[..end], false, context()).is_err(), "{} prefix{end}", case["name"]);
            }
            // Slot decoder возвращает длину одного элемента, не съедает соседний.
            let mut joined = raw.clone(); joined.push(0);
            assert_eq!(decode(version, &joined, false, context()).unwrap(), (stack.clone(), raw.len()));
        }
    }
}

#[test]
fn direct_775_776_components_do_not_detour_through_777() {
    for (kind, data, target) in [(40, vec![2, 7], 40), (45, vec![0, 0x12, 0x34, 0x56], 45), (84, vec![14], 85)] {
        let stack = Stack { item: 1, count: 1, added: vec![Component { kind, data: data.clone() }], removed: vec![] };
        let body = encode(MID, Some(&stack), false, context()).unwrap();
        assert_eq!(body, slot(1, target, &data));
        assert_eq!(decode(MID, &body, false, context()).unwrap().0, Some(stack));
    }
    // Removed ID тоже идёт напрямую: swing/map_color отсутствуют777.
    let stack = Stack { item: 1, count: 1, added: vec![], removed: vec![40, 45, 84] };
    assert_eq!(encode(MID, Some(&stack), false, context()).unwrap(), vec![1, 1, 0, 3, 40, 45, 85]);
    assert_eq!(decode(MID, &[1, 1, 0, 3, 40, 45, 85], false, context()).unwrap().0, Some(stack));
    // Старый775↔777 scope не расширяется одним фактом наблюдения776.
    for kind in [40, 84] {
        let raw = if kind == 40 { vec![2, 7] } else { vec![14] };
        assert_eq!(component_bytes(kind, &mut Reader(&raw), OLD, NEW, context()),
            Err(Error::UnsupportedComponent(kind)));
    }
}

#[test]
fn missing_native_content_and_unobserved_forms_still_fail_explicitly() {
    let missing = |id| Error::Id(ids::IdTranslationError::MissingTarget(id));
    let mut sulfur = vec![1]; uint(&mut sulfur, 1052); sulfur.extend([0, 0]);
    assert_eq!(decode(MID, &sulfur, false, context()), Err(missing(1052)));
    assert_eq!(decode(MID, &[1, 1, 1, 0, 78], false, context()), Err(missing(78)));
    assert_eq!(decode(MID, &[1, 1, 0, 1, 78], false, context()), Err(missing(78)));
    let mut unknown = vec![1]; uint(&mut unknown, 1537); unknown.extend([0, 0]);
    assert_eq!(decode(MID, &unknown, false, context()), Err(Error::Id(ids::IdTranslationError::UnknownId(1537))));
    assert_eq!(decode(MID, &[1, 1, 1, 0, 111], false, context()),
        Err(Error::Id(ids::IdTranslationError::UnknownId(111))));
    assert_eq!(decode(MID, &[1, 1, 1, 0, 4], false, context()), Err(Error::UnsupportedComponent(4)));
    let stack = Stack { item: 1, count: 1, added: vec![], removed: vec![] };
    let unsupported = Error::UnsupportedVersion(MID);
    for raw in [&[0][..], &[1, 1, 0, 0]] {
        assert_eq!(decode(MID, raw, true, context()), Err(unsupported.clone()));
        assert_eq!(decode_template(MID, raw, context()), Err(unsupported.clone()));
    }
    assert_eq!(encode(MID, None, true, context()), Err(unsupported.clone()));
    assert_eq!(encode(MID, Some(&stack), true, context()), Err(unsupported.clone()));
    assert_eq!(encode_template(MID, &stack, context()), Err(unsupported));
}

#[test]
fn malformed_776_values_counts_and_attribute_fields_are_rejected() {
    let mut invalid = vec![
        slot(1, 40, &[3, 7]), slot(1, 40, &[2, 255, 255, 255, 255, 15]),
        slot(1, 85, &[16]), slot(1, 45, &[1, 0, 0, 0]), slot(1, 6, &[10, 0]),
        vec![128, 1, 1, 0, 0], vec![1, 1, 129, 2, 0],
        vec![1, 1, 2, 0, 3, 1, 3, 2], vec![1, 1, 1, 1, 3, 7, 3],
    ];
    let data = fixture();
    let observed = bytes(data["cases"][3]["slot_hex"].as_str().unwrap());
    let mut input = Reader(&observed);
    for _ in 0..5 { input.uint().unwrap(); } // count,item,added,removed,kind
    let attributes = input.0;
    // count1,type3,string10,value f64,operation0,slot1,display0.
    let value_offset = 2 + 1 + 10;
    let mut nan = attributes.to_vec(); nan[value_offset..value_offset + 8].copy_from_slice(&f64::NAN.to_be_bytes());
    invalid.push(slot(1, 16, &nan));
    for (offset, value) in [(value_offset + 8, 3), (value_offset + 9, 11), (value_offset + 10, 3)] {
        let mut malformed = attributes.to_vec(); malformed[offset] = value;
        invalid.push(slot(1, 16, &malformed));
    }
    for body in invalid { assert!(decode(MID, &body, false, context()).is_err(), "{body:?}"); }
    assert_eq!(decode(MID, &vec![0; LIMIT + 1], false, context()), Err(Error::TooLarge));
}

#[test]
fn schema_only_776_nested_shapes_reuse_codec_and_preserve_inner_ids() {
    let data = fixture(); assert_eq!(data["nested_scope"]["live_776"], false);
    for case in data["cases"].as_array().unwrap().iter().take(7) {
        let native = template(&bytes(case["native_slot_hex"].as_str().unwrap()));
        let middle = template(&bytes(case["slot_hex"].as_str().unwrap()));
        for kind in [25, 49, 50, 75] {
            let wrap = |body: &[u8]| {
                let mut out = match kind { 25 => vec![], 75 => vec![2, 0, 1], _ => vec![1] };
                out.extend(body); slot(1, kind, &out)
            };
            let source = wrap(&middle); let expected = wrap(&native);
            let (stack, n) = decode(MID, &source, false, context()).unwrap(); assert_eq!(n, source.len());
            assert_eq!(encode(OLD, stack.as_ref(), false, context()).unwrap(), expected);
            assert_eq!(encode(MID, stack.as_ref(), false, context()).unwrap(), source);
            for end in 0..source.len() { assert!(decode(MID, &source[..end], false, context()).is_err()); }
        }
    }
}

#[test]
fn schema_only_776_nested_depth_counts_duplicates_and_missing_ids_are_bounded() {
    let mut inner = vec![0];
    for _ in 0..31 {
        let mut outer = vec![1, 1, 1, 1, 0, 50]; outer.extend(inner); inner = outer;
    }
    let raw = slot(1, 50, &inner);
    let (stack, n) = decode(MID, &raw, false, context()).unwrap(); assert_eq!(n, raw.len());
    assert_eq!(encode(MID, stack.as_ref(), false, context()).unwrap(), raw);
    let mut too_deep = vec![1, 1, 1, 1, 0, 50]; too_deep.extend(inner);
    assert_eq!(decode(MID, &slot(1, 50, &too_deep), false, context()), Err(Error::TooLarge));
    assert_eq!(decode(MID, &slot(1, 50, &[129, 2]), false, context()), Err(Error::TooLarge));
    for inner in [
        vec![1, 0, 1, 0, 0], vec![1, 1, 0, 0, 0],
        vec![1, 1, 1, 2, 0, 3, 0, 3, 1], vec![1, 1, 1, 1, 1, 3, 0, 3],
    ] {
        assert_eq!(decode(MID, &slot(1, 50, &inner), false, context()), Err(Error::Invalid));
    }
    assert_eq!(decode(MID, &slot(1, 75, &[1, 2]), false, context()), Err(Error::Invalid));
    let mut sulfur = vec![1]; uint(&mut sulfur, 1052); sulfur.extend([1, 0, 0]);
    assert_eq!(decode(MID, &slot(1, 50, &sulfur), false, context()),
        Err(Error::Id(ids::IdTranslationError::MissingTarget(1052))));
    assert_eq!(decode(MID, &slot(1, 50, &[1, 1, 1, 1, 0, 78]), false, context()),
        Err(Error::Id(ids::IdTranslationError::MissingTarget(78))));
    assert_eq!(decode(MID, &slot(1, 50, &[1, 1, 1, 1, 0, 4]), false, context()),
        Err(Error::UnsupportedComponent(4)));
}

#[test]
fn removed_components_cannot_extend_a_full_slot_past_the_byte_limit() {
    let bundle = |size: usize| {
        // 16 своих шаблонов с NBT-строками: count + 16*(8 байт заголовка + текст).
        let mut out = vec![16];
        let mut remaining = size - 1 - 16 * 8;
        for _ in 0..16 {
            let length = remaining.min(u16::MAX as usize);
            out.extend([1, 1, 1, 0, 6, 8]);
            out.extend((length as u16).to_be_bytes());
            out.resize(out.len() + length, b'x');
            remaining -= length;
        }
        assert_eq!(remaining, 0);
        assert_eq!(out.len(), size);
        out
    };
    for version in [OLD, MID, NEW] {
        // Заголовок Slot и ID bundle занимают 5 байт во всех трёх версиях.
        let mut stack = Stack { item: 1, count: 1,
            added: vec![Component { kind: 50, data: bundle(LIMIT - 5) }], removed: vec![] };
        let body = encode(version, Some(&stack), false, context()).unwrap();
        assert_eq!(body.len(), LIMIT);
        assert_eq!(decode(version, &body, false, context()).unwrap(), (Some(stack.clone()), LIMIT));
        stack.removed.push(3);
        assert_eq!(encode(version, Some(&stack), false, context()), Err(Error::TooLarge));
        // Ровно LIMIT, уже включая removed ID, остаётся допустимым.
        stack.added[0].data = bundle(LIMIT - 6);
        let body = encode(version, Some(&stack), false, context()).unwrap();
        assert_eq!(body.len(), LIMIT);
        assert_eq!(decode(version, &body, false, context()).unwrap(), (Some(stack), LIMIT));
    }
}
