//! Точный перевод ID 775↔777 по разрешённым JSON. Это не перевод пакетов.
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
    for version in [from, to] {
        if version == JavaVersion::V26_2 { return Err(IdTranslationError::UnsupportedVersion(version)); }
    }
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
    fn new_content_and_removed_attributes_never_get_silent_fallbacks() {
        // Четыре атрибута удалены из таблицы 777. Их значения нельзя просто
        // отправить под прежними ID: это уже другие атрибуты.
        for id in [20, 21, 23, 26] {
            assert_eq!(translate(RegistryKind::Attribute, V26_1_2, V26_3, id),
                Err(IdTranslationError::MissingTarget(id)));
        }
        assert_eq!(translate(RegistryKind::Item, V26_2, V26_3, 1),
            Err(IdTranslationError::UnsupportedVersion(V26_2)));
        assert_eq!(translate(RegistryKind::BlockState, V26_3, V26_1_2, u32::MAX),
            Err(IdTranslationError::UnknownId(u32::MAX)));
    }

    #[test]
    fn observed_configuration_packet_ids_match_vanilla() {
        assert_eq!(translate(RegistryKind::ConfigurationClientbound, V26_1_2, V26_3, 14), Ok(15));
        assert_eq!(translate(RegistryKind::ConfigurationClientbound, V26_1_2, V26_3, 13), Ok(14));
        assert_eq!(translate(RegistryKind::PlayClientbound, V26_1_2, V26_3, 49), Ok(50));
    }
}
