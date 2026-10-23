//! Точный перевод статических ID775/776/777; Attribute776 подтверждены отдельно
//! наблюдениями. Доступность таблиц ID не означает поддержку форматов пакетов.
use crate::JavaVersion;
use crate::generated_ids::tables;
pub use crate::generated_ids::RegistryKind;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdTranslationError {
    UnsupportedVersion(JavaVersion),
    UnknownId(u32),
    MissingTarget(u32),
}

/// Неизвестный объект не превращается в воздух/пустой слот. Вызывающий код
/// обязан явно обработать отсутствие объекта в целевой версии.
pub fn translate(
    registry: RegistryKind, from: JavaVersion, to: JavaVersion, id: u32,
) -> Result<u32, IdTranslationError> {
    if from == JavaVersion::V1_21_11 || to == JavaVersion::V1_21_11 {
        if from != JavaVersion::V1_21_11 && from != JavaVersion::V26_1_2 {
            let middle = if from == JavaVersion::V26_3 {
                let adjacent = translate(registry, from, JavaVersion::V26_2, id)?;
                translate(registry, JavaVersion::V26_2, JavaVersion::V26_1_2, adjacent)?
            } else { translate(registry, from, JavaVersion::V26_1_2, id)? };
            return translate(registry, JavaVersion::V26_1_2, to, middle);
        }
        if to != JavaVersion::V1_21_11 && to != JavaVersion::V26_1_2 {
            let middle = translate(registry, from, JavaVersion::V26_1_2, id)?;
            return if to == JavaVersion::V26_3 {
                let adjacent = translate(registry, JavaVersion::V26_1_2, JavaVersion::V26_2, middle)?;
                translate(registry, JavaVersion::V26_2, to, adjacent)
            } else { translate(registry, JavaVersion::V26_1_2, to, middle) };
        }
        if registry == RegistryKind::Attribute {
            let values = if from == JavaVersion::V1_21_11 {crate::generated_payload_774::ATTRIBUTES_FROM} else {crate::generated_payload_774::ATTRIBUTES_TO};
            let mapped=*values.get(id as usize).ok_or(IdTranslationError::UnknownId(id))?;
            return if from==to {Ok(id)} else if mapped==u16::MAX {Err(IdTranslationError::MissingTarget(id))} else {Ok(u32::from(mapped))};
        }
        let (count, ranges) = crate::generated_step1::ranges(registry, from == JavaVersion::V1_21_11)
            .ok_or(IdTranslationError::UnsupportedVersion(JavaVersion::V1_21_11))?;
        if id >= count { return Err(IdTranslationError::UnknownId(id)); }
        if from == to { return Ok(id); }
        let &(_, _, offset) = ranges.iter().find(|&&(first,last,_)| (first..=last).contains(&id))
            .ok_or(IdTranslationError::UnknownId(id))?;
        return if offset == 65535 { Err(IdTranslationError::MissingTarget(id)) }
            else { Ok((i64::from(id) + i64::from(offset)) as u32) };
    }
    if (from == JavaVersion::V26_2 || to == JavaVersion::V26_2)
        && crate::generated_static_776::source_count(registry, JavaVersion::V26_2).is_some() {
        // Плотность source IDs доказана генератором. Сначала проверяем наличие
        // исходного ID;65535 означает лишь отсутствие цели у существующего объекта.
        let count = crate::generated_static_776::source_count(registry, from)
            .ok_or(IdTranslationError::UnsupportedVersion(from))?;
        crate::generated_static_776::source_count(registry, to)
            .ok_or(IdTranslationError::UnsupportedVersion(to))?;
        if id >= count { return Err(IdTranslationError::UnknownId(id)); }
        if from == to { return Ok(id); }
        let table = crate::generated_static_776::table(registry, from, to)
            .ok_or(IdTranslationError::UnsupportedVersion(to))?;
        let mapped = table[id as usize];
        return if mapped == crate::generated_static_776::MISSING { Err(IdTranslationError::MissingTarget(id)) }
            else { Ok(u32::from(mapped)) };
    }
    for version in [from, to] {
        if !matches!(version, JavaVersion::V26_1_2 | JavaVersion::V26_3)
            && !(registry == RegistryKind::Attribute && version == JavaVersion::V26_2) {
            return Err(IdTranslationError::UnsupportedVersion(version));
        }
    }
    // Только Attribute: все40 имён/ID776 совпали с официальными777 через
    // Tab + отдельные item modifier packets, testdata/protocol-26.2-observations.json.
    // Это НЕ общий alias776→777: другие реестры и packet payload сюда не допускаются.
    let (from, to) = if registry == RegistryKind::Attribute {
        (if from == JavaVersion::V26_2 { JavaVersion::V26_3 } else { from },
         if to == JavaVersion::V26_2 { JavaVersion::V26_3 } else { to })
    } else { (from, to) };
    let (forward, backward) = tables(registry);
    let source = if from == JavaVersion::V26_1_2 { forward } else { backward };
    let mapped = *source.get(id as usize).ok_or(IdTranslationError::UnknownId(id))?;
    if from == to { return Ok(id); }
    u32::try_from(mapped).map_err(|_| IdTranslationError::MissingTarget(id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::JavaVersion::{V26_1_2, V26_2, V26_3};

    #[test]
    fn every_mapped_id_round_trips_in_both_directions() {
        for &registry in crate::generated_ids::ALL {
            let (forward, backward) = tables(registry);
            for (from, to, source) in [(V26_1_2, V26_3, forward), (V26_3, V26_1_2, backward)] {
                for (id, &target) in source.iter().enumerate() {
                    let id = id as u32;
                    assert_eq!(translate(registry, from, from, id), Ok(id));
                    if target < 0 {
                        assert_eq!(translate(registry, from, to, id), Err(IdTranslationError::MissingTarget(id)));
                    } else {
                        assert_eq!(translate(registry, from, to, id), Ok(target as u32));
                        assert_eq!(translate(registry, to, from, target as u32), Ok(id));
                    }
                }
                let outside = source.len() as u32;
                assert_eq!(translate(registry, from, to, outside), Err(IdTranslationError::UnknownId(outside)));
            }
        }
    }

    #[test]
    fn new_attributes_never_get_silent_fallbacks() {
        // Официальные команды подтвердили35старых+5новых, а не удаление
        // четырёх атрибутов, ошибочно указанное в JSON-кандидате777.
        for id in [0, 6, 9, 17, 27] {
            assert_eq!(translate(RegistryKind::Attribute, V26_3, V26_1_2, id),
                Err(IdTranslationError::MissingTarget(id)));
        }
        assert_eq!(translate(RegistryKind::Attribute,V26_1_2,V26_3,2),Ok(3));
        assert_eq!(translate(RegistryKind::Attribute,V26_1_2,V26_3,22),Ok(26));
        assert_eq!(translate(RegistryKind::PlayClientbound, V26_2, V26_3, 1),
            Err(IdTranslationError::UnsupportedVersion(V26_2)));
        assert_eq!(translate(RegistryKind::BlockState, V26_3, V26_1_2, u32::MAX),
            Err(IdTranslationError::UnknownId(u32::MAX)));
    }

    fn observed_attribute_ids(sample: &serde_json::Value) -> std::collections::BTreeMap<String, u32> {
        assert_eq!(sample["exit_code"], 0);
        let mut ids = std::collections::BTreeMap::new();
        for case in sample["cases"].as_array().unwrap() {
            let hex = case["slot_hex"].as_str().unwrap();
            assert_eq!(hex.len() % 2, 0);
            let bytes: Vec<u8> = hex.as_bytes().as_chunks::<2>().0.iter()
                .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap()).collect();
            let mut input = crate::wire::Cursor::new(&bytes);
            assert_eq!(input.uint().unwrap(), 1);
            input.uint().unwrap(); // ID stick отличается между клиентскими версиями.
            assert_eq!([input.uint().unwrap(), input.uint().unwrap(), input.uint().unwrap(), input.uint().unwrap()], [1, 0, 16, 1]);
            let id = input.uint().unwrap();
            assert_eq!(input.string().unwrap(), "mcsa:probe");
            assert_eq!(input.take(8).unwrap(), 1f64.to_be_bytes());
            assert_eq!([input.uint().unwrap(), input.uint().unwrap(), input.uint().unwrap()], [0, 1, 0]);
            assert_eq!(input.remaining(), 0);
            assert!(ids.insert(case["name"].as_str().unwrap().to_owned(), id).is_none());
        }
        let suggestions: std::collections::BTreeSet<_> = sample["attribute_suggestions"].as_array().unwrap()
            .iter().map(|name| name.as_str().unwrap()).collect();
        assert_eq!(ids.keys().map(String::as_str).collect::<std::collections::BTreeSet<_>>(), suggestions);
        assert_eq!(ids.values().copied().collect::<std::collections::BTreeSet<_>>(),
            (0..ids.len() as u32).collect());
        ids
    }

    #[test]
    fn all_observed_776_attributes_match_777_and_translate_by_775_names() {
        let previous: serde_json::Value = serde_json::from_str(include_str!("../testdata/attribute-id-observations.json")).unwrap();
        let fresh: serde_json::Value = serde_json::from_str(include_str!("../testdata/protocol-26.2-observations.json")).unwrap();
        assert_eq!(fresh["attributes"]["protocol"], 776);
        assert_eq!(previous["old"]["protocol"], 775);
        assert_eq!(previous["new"]["protocol"], 777);
        let native = observed_attribute_ids(&previous["old"]);
        let modern = observed_attribute_ids(&previous["new"]);
        let middle = observed_attribute_ids(&fresh["attributes"]);
        assert_eq!(native.len(), 35);
        assert_eq!(middle.len(), 40);
        assert_eq!(middle, modern);
        let mut missing = 0;
        for (name, &id) in &middle {
            for (from, to) in [(V26_2, V26_2), (V26_2, V26_3), (V26_3, V26_2)] {
                assert_eq!(translate(RegistryKind::Attribute, from, to, id), Ok(id), "{name}");
            }
            if let Some(&old) = native.get(name) {
                assert_eq!(translate(RegistryKind::Attribute, V26_1_2, V26_2, old), Ok(id), "{name}");
                assert_eq!(translate(RegistryKind::Attribute, V26_2, V26_1_2, id), Ok(old), "{name}");
                assert_eq!(translate(RegistryKind::Attribute, V26_1_2, V26_3, old), Ok(id), "{name}");
            } else {
                missing += 1;
                assert_eq!(translate(RegistryKind::Attribute, V26_2, V26_1_2, id),
                    Err(IdTranslationError::MissingTarget(id)), "{name}");
            }
        }
        assert_eq!(missing, 5);
    }

    #[test]
    fn attribute_776_keeps_bounds_and_does_not_enable_packet_registries() {
        for id in [40, u32::MAX] {
            for (from, to) in [(V26_2, V26_2), (V26_2, V26_1_2), (V26_2, V26_3), (V26_3, V26_2)] {
                assert_eq!(translate(RegistryKind::Attribute, from, to, id), Err(IdTranslationError::UnknownId(id)));
            }
        }
        for id in [35, u32::MAX] {
            assert_eq!(translate(RegistryKind::Attribute, V26_1_2, V26_2, id), Err(IdTranslationError::UnknownId(id)));
        }
        for &registry in crate::generated_ids::ALL {
            if registry == RegistryKind::Attribute || crate::generated_static_776::source_count(registry, V26_2).is_some() { continue; }
            for (from, to) in [(V26_2, V26_2), (V26_2, V26_1_2), (V26_1_2, V26_2), (V26_2, V26_3), (V26_3, V26_2)] {
                assert_eq!(translate(registry, from, to, 0), Err(IdTranslationError::UnsupportedVersion(V26_2)));
            }
        }
    }

    #[test]
    fn observed_configuration_packet_ids_match_vanilla() {
        assert_eq!(translate(RegistryKind::ConfigurationClientbound, V26_1_2, V26_3, 14), Ok(15));
        assert_eq!(translate(RegistryKind::ConfigurationClientbound, V26_1_2, V26_3, 13), Ok(14));
        assert_eq!(translate(RegistryKind::PlayClientbound, V26_1_2, V26_3, 49), Ok(50));
    }
}
