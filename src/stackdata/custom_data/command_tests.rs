//! Интеграционные тесты `custom_data` из SNBT-команд против сырого NBT.
//! Фикстура — реальный предмет из 26.3: `mcsa_layout:1` (int) и
//! `marker:"LAYOUT_FIXED"` (quoted строка); `raw_hex` — независимое ожидание.

use super::CustomData;
use crate::stackdata::parse_item_spec;

/// Строковое поле JSON-фикстуры через уже существующую зависимость.
fn fixture_field(fixture: &str, key: &str) -> String {
    serde_json::from_str::<serde_json::Value>(fixture)
        .expect("фикстура — валидный JSON")[key]
        .as_str()
        .unwrap_or_else(|| panic!("поле \"{key}\" фиксусты — не строка"))
        .to_owned()
}

/// Байты из hex-строки фикстуры.
fn hex_bytes(hex: &str) -> Vec<u8> {
    let trimmed: String = hex.chars().filter(|c| !c.is_whitespace()).collect();
    assert!(trimmed.len().is_multiple_of(2), "hex фикстуры нечётной длины");
    trimmed
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

/// Реальная фикстура: полная команда, snbt и сырые байты согласованы.
#[test]
fn fixture_original_command_matches_raw_hex() {
    let fixture = include_str!("../../../testdata/custom-data-command-26.3.json");
    let snbt = fixture_field(fixture, "snbt");
    let raw_hex = fixture_field(fixture, "raw_hex");
    let command = fixture_field(fixture, "command");

    let from_hex = CustomData::from_bytes(&hex_bytes(&raw_hex))
        .expect("raw_hex фикстуры должен читаться как NBT");
    let from_snbt = CustomData::from_snbt(&snbt).expect("snbt фикстуры должен разбираться");
    assert_eq!(from_hex, from_snbt);

    let (_, data) = parse_item_spec(&command)
        .expect("полная команда фикстуры должна разбираться");
    assert_eq!(data.custom_data.as_ref(), Some(&from_hex));
}

/// Известный оригинал: безымянный TAG_Compound, TAG_Int (ид 3),
/// TAG_String (ид 8), завершение TAG_End. Порядок полей в сыром NBT
/// исходный, поэтому сравниваем канонизацию через from_bytes.
#[test]
fn known_original_int_and_marker_independent_bytes() {
    let expected_raw: &[u8] = &[
        10,
        3, 0, 11, b'm', b'c', b's', b'a', b'_', b'l', b'a', b'y', b'o', b'u', b't', 0, 0, 0, 1,
        8, 0, 6, b'm', b'a', b'r', b'k', b'e', b'r', 0, 12,
        b'L', b'A', b'Y', b'O', b'U', b'T', b'_', b'F', b'I', b'X', b'E', b'D',
        0,
    ];
    let canonical = CustomData::from_bytes(expected_raw).expect("ожидание читается как NBT");
    assert_eq!(
        CustomData::from_snbt(r#"{mcsa_layout:1,marker:"LAYOUT_FIXED"}"#).as_ref(),
        Some(&canonical),
    );

    let empty = CustomData::from_snbt("{}").expect("пустой compound поддерживается");
    assert_eq!(CustomData::from_bytes(empty.as_bytes()), Some(empty.clone()));
    let (_, data) = parse_item_spec("stone[minecraft:custom_data={}]")
        .expect("пустой custom_data — положительный случай");
    assert_eq!(data.custom_data, Some(empty));
}

/// Оба алиаса компонента дают одинаковое custom_data.
#[test]
fn raw_aliases_give_same_custom_data() {
    let (_, bare) = parse_item_spec("stone[custom_data={mcsa_layout:1}]")
        .expect("алиас без префикса поддерживается");
    let (_, prefixed) = parse_item_spec("stone[minecraft:custom_data={mcsa_layout:1}]")
        .expect("алиас с префиксом поддерживается");
    assert_eq!(bare.custom_data, prefixed.custom_data);
}

/// Long 9007199254740993L = 2^53 + 1 (0x0020000000000001): точное BE-кодирование
/// с типом 4 ловит потерю точности через f64; данные переживают сериализацию.
#[test]
fn long_exact_tag4_binary_preserved_through_data() {
    let expected_raw: Vec<u8> = [vec![10u8, 4, 0, 1, b'x'], 9007199254740993i64.to_be_bytes().to_vec(), vec![0u8]]
        .concat();
    let canonical = CustomData::from_bytes(&expected_raw).expect("long-ожидание читается");
    assert_eq!(CustomData::from_snbt("{x:9007199254740993L}").as_ref(), Some(&canonical));
    assert_eq!(CustomData::from_snbt("{x:9007199254740993l}").as_ref(), Some(&canonical));
    let lesser = CustomData::from_snbt("{x:9007199254740992L}").expect("l64 поддерживается");
    assert_ne!(canonical, lesser);

    let (_, data) = parse_item_spec("stone[custom_data={x:9007199254740993L}]")
        .expect("long должен уходить в довески");
    let bytes = crate::stackdata::to_bytes(&data);
    let restored = crate::stackdata::from_bytes(&bytes).expect("довески переживают сериализацию");
    assert_eq!(restored.custom_data, data.custom_data);
}

/// Полный сбор стекла по первичному каталогу: crossbow, custom_data,
/// damage и чары через enchantments, а не неизвестный ключ unbreaking.
#[test]
fn catalog_stack_keeps_custom_data_damage_and_enchantments() {
    let catalog = crate::gameplay_catalog::Catalog::new(
        crate::gameplay_catalog::Profile::Preview26_3,
    );
    let spec = r#"crossbow[damage=7,enchantments={unbreaking:3},custom_data={mcsa_layout:1,marker:"LAYOUT_FIXED"}]"#;
    let stack = crate::commands::stack_from_spec_for_catalog(catalog, spec, 1)
        .ok()
        .flatten()
        .expect("стек должен собраться из спецификации");
    let expected = CustomData::from_snbt(r#"{mcsa_layout:1,marker:"LAYOUT_FIXED"}"#)
        .expect("int и quoted строка поддерживаются");
    assert_eq!(stack.extras().custom_data.as_ref(), Some(&expected));
    assert_eq!(stack.extras().damage, 7);
    assert_eq!(stack.extras().level("unbreaking"), 3);
}

/// Экранированные запятая и кавычка не рвут довесок; имя сохранено в байтах.
#[test]
fn escaped_commas_and_quotes_survive_splitting() {
    let (_, data) = parse_item_spec(r#"paper[custom_data={marker:"a,\"b"},custom_name="name,\"still"]"#)
        .expect("экранирования внутри кавычек поддерживаются");
    let expected_cd = CustomData::from_snbt(r#"{marker:"a,\"b"}"#)
        .expect("экранированная quoted строка поддерживается");
    assert_eq!(data.custom_data.as_ref(), Some(&expected_cd));
    assert_eq!(data.name.as_deref(), Some(r#"name,"still"#));
}

/// Дубликат алиаса отвергается до создания предмета; одиночный — ок.
#[test]
fn duplicate_custom_data_alias_rejected() {
    for spec in [
        "stone[custom_data={},minecraft:custom_data={x:1b}]",
        "stone[minecraft:custom_data={},custom_data={x:1b}]",
    ] {
        assert!(parse_item_spec(spec).is_err(), "{spec}");
    }
    assert!(parse_item_spec("stone[custom_data={x:1b}]").is_ok());
}

/// Float и списки, битый SNBT и неизвестное экранирование отвергаются.
#[test]
fn unsupported_types_and_broken_snbt_rejected() {
    for bad_snbt in [
        "{x:1.5}",
        "{x:1.5f}",
        "{x:1.5d}",
        "{x:1.0}",
        "{x:[1b,2b]}",
        "{x:",
        "{x:q",
        r#"{m:"a\qb"}"#,
    ] {
        assert!(CustomData::from_snbt(bad_snbt).is_none(), "{bad_snbt}");
    }
    for bad_spec in ["stone[custom_data={x:1.5}]", "stone[custom_data={x:[1b]}]"] {
        assert!(parse_item_spec(bad_spec).is_err(), "{bad_spec}");
    }
}

/// Алиасы custom_data в готовом SNBT-значении отвергаются до потери типов.
#[test]
fn untyped_nested_custom_data_aliases_rejected() {
    for text in [
        "{id:\"stone\",components:{custom_data:{x:1}}}",
        "{id:\"stone\",components:{\"minecraft:custom_data\":{x:1}}}",
    ] {
        let value = crate::snbt::parse(text).expect("каркас SNBT");
        assert!(crate::stackdata::stack_from_value(&value).is_err(), "{text}");
    }
    for text in [
        "{Item:{id:stone,components:{custom_data:{x:1}}}}",
        "{Item:{id:stone,components:{\"minecraft:custom_data\":{x:1}}}}",
    ] {
        assert!(crate::commands::item_from_text(text).is_none(), "{text}");
    }
}

/// Действует прежний предел целых довесков: 65535.
#[test]
fn whole_extras_limit_held_for_large_quoted_marker() {
    let marker = "B".repeat(70000);
    let spec = format!("stone[custom_data={{marker:\"{marker}\"}}]");
    assert!(parse_item_spec(&spec).is_err());
}
