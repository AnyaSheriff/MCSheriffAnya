//! Внутренний каталог не является версией сетевого протокола.
//! Узкий preview содержит только white_concrete_slab из pinned JSON fixture.
use crate::{JavaVersion, ids::{self, RegistryKind}};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdSpace { Canonical, Wire(JavaVersion) }

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ContentCodec { white_slab: bool }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Id(ids::IdTranslationError),
    Unrepresentable { kind: RegistryKind, id: u32, target: IdSpace },
}

impl ContentCodec {
    pub const fn is_preview(self) -> bool { self.white_slab }
    /// Только граница внутреннего packet builder: раскладка775 содержит
    /// canonical content. К настоящему входному Slot775 это неприменимо.
    pub(crate) fn packet_space(self, version: JavaVersion) -> IdSpace {
        if self.white_slab && version == JavaVersion::V26_1_2 { IdSpace::Canonical }
        else { IdSpace::Wire(version) }
    }
    pub(crate) fn packet_id(self, kind: RegistryKind, from: JavaVersion, to: JavaVersion, id: u32) -> Result<u32, Error> {
        self.map(kind,self.packet_space(from),self.packet_space(to),id)
    }

    /// Только вызывающий серверный каталог разрешает этот private preview.
    /// Версия подключившегося клиента сама по себе его не выбирает.
    pub const fn white_slab_preview() -> Self { Self { white_slab: true } }

    fn extension(kind: RegistryKind, space: IdSpace, id: u32) -> Option<u32> {
        let (first, last, wire) = match kind {
            RegistryKind::Item => (1506, 1506, 721),
            RegistryKind::Block => (1168, 1168, 800),
            RegistryKind::BlockState => (29873, 29878, 18307),
            _ => return None,
        };
        match space {
            IdSpace::Canonical if (first..=last).contains(&id) => Some(id),
            IdSpace::Wire(JavaVersion::V26_3) if (wire..=wire + last - first).contains(&id) => Some(first + id - wire),
            _ => None,
        }
    }

    pub fn map(self, kind: RegistryKind, from: IdSpace, to: IdSpace, id: u32) -> Result<u32, Error> {
        // Оба Wire обозначают настоящие протокольные пространства. Даже при
        // preview Canonical1506 не становится допустимым Wire775 item1506.
        if self.white_slab && let Some(canonical) = Self::extension(kind, from, id) {
                return match to {
                    IdSpace::Canonical => Ok(canonical),
                    IdSpace::Wire(JavaVersion::V26_3) => {
                        let (first, wire) = match kind {
                            RegistryKind::Item => (1506, 721),
                            RegistryKind::Block => (1168, 800),
                            RegistryKind::BlockState => (29873, 18307),
                            _ => unreachable!(),
                        };
                        Ok(wire + canonical - first)
                    }
                    _ => Err(Error::Unrepresentable { kind, id: canonical, target: to }),
                };
        }
        let version = |space| match space { IdSpace::Canonical => JavaVersion::V26_1_2, IdSpace::Wire(version) => version };
        ids::translate(kind, version(from), version(to), id).map_err(Error::Id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn extension_is_canonical_and_never_an_alias_for_wire775() {
        let codec = ContentCodec::white_slab_preview();
        for (kind, first, last, wire) in [(RegistryKind::Item,1506,1506,721),
            (RegistryKind::Block,1168,1168,800),(RegistryKind::BlockState,29873,29878,18307)] {
            for id in first..=last {
                let target = wire + id - first;
                assert_eq!(codec.map(kind,IdSpace::Canonical,IdSpace::Wire(JavaVersion::V26_3),id),Ok(target));
                assert_eq!(codec.map(kind,IdSpace::Wire(JavaVersion::V26_3),IdSpace::Canonical,target),Ok(id));
                assert!(ContentCodec::default().map(kind,IdSpace::Canonical,IdSpace::Wire(JavaVersion::V26_3),id).is_err());
                assert!(codec.map(kind,IdSpace::Wire(JavaVersion::V26_1_2),IdSpace::Canonical,id).is_err());
                for version in [JavaVersion::V26_1_2,JavaVersion::V26_2,JavaVersion::V1_12_2] {
                    assert!(matches!(codec.map(kind,IdSpace::Canonical,IdSpace::Wire(version),id),Err(Error::Unrepresentable { .. })));
                }
            }
        }
    }
    #[test]
    fn base_ids_and_errors_stay_equal_to_existing_tables() {
        for codec in [ContentCodec::default(),ContentCodec::white_slab_preview()] {
            for kind in [RegistryKind::Item,RegistryKind::Block,RegistryKind::BlockState] {
                for id in [0,1,2,500,u32::MAX] {
                    assert_eq!(codec.map(kind,IdSpace::Canonical,IdSpace::Wire(JavaVersion::V26_3),id),
                        ids::translate(kind,JavaVersion::V26_1_2,JavaVersion::V26_3,id).map_err(Error::Id));
                }
            }
        }
    }
}
