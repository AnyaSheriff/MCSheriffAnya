//! Внутренний каталог не является версией сетевого протокола.
//! Preview бетонных плит: выбранные pinned JSON, базовые ID775 неизменны.
use crate::{JavaVersion, ids::{self, RegistryKind}};

/// Неизменяемые внутренние идентичности; существующие строки нельзя перенумеровывать.
/// Источник: selected fixture concrete-slabs-26.3.json, не игровые наблюдения.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ConcreteSlab {
    pub name: &'static str,
    pub item: i32,
    pub block: i32,
    pub first_state: i32,
    pub last_state: i32,
    pub default_state: i32,
    pub wire_item: u32,
    pub wire_block: u32,
    pub wire_first_state: u32,
    pub input_name: &'static str,
    pub input_item: i32,
}

pub static CONCRETE_SLABS: [ConcreteSlab; 16] = [
    ConcreteSlab { name: "white_concrete_slab", item: 1506, block: 1168, first_state: 29873, last_state: 29878, default_state: 29876, wire_item: 721, wire_block: 800, wire_first_state: 18307, input_name: "white_concrete", input_item: 615 },
    ConcreteSlab { name: "orange_concrete_slab", item: 1507, block: 1169, first_state: 29879, last_state: 29884, default_state: 29882, wire_item: 722, wire_block: 801, wire_first_state: 18313, input_name: "orange_concrete", input_item: 616 },
    ConcreteSlab { name: "magenta_concrete_slab", item: 1508, block: 1170, first_state: 29885, last_state: 29890, default_state: 29888, wire_item: 723, wire_block: 802, wire_first_state: 18319, input_name: "magenta_concrete", input_item: 617 },
    ConcreteSlab { name: "light_blue_concrete_slab", item: 1509, block: 1171, first_state: 29891, last_state: 29896, default_state: 29894, wire_item: 724, wire_block: 803, wire_first_state: 18325, input_name: "light_blue_concrete", input_item: 618 },
    ConcreteSlab { name: "yellow_concrete_slab", item: 1510, block: 1172, first_state: 29897, last_state: 29902, default_state: 29900, wire_item: 725, wire_block: 804, wire_first_state: 18331, input_name: "yellow_concrete", input_item: 619 },
    ConcreteSlab { name: "lime_concrete_slab", item: 1511, block: 1173, first_state: 29903, last_state: 29908, default_state: 29906, wire_item: 726, wire_block: 805, wire_first_state: 18337, input_name: "lime_concrete", input_item: 620 },
    ConcreteSlab { name: "pink_concrete_slab", item: 1512, block: 1174, first_state: 29909, last_state: 29914, default_state: 29912, wire_item: 727, wire_block: 806, wire_first_state: 18343, input_name: "pink_concrete", input_item: 621 },
    ConcreteSlab { name: "gray_concrete_slab", item: 1513, block: 1175, first_state: 29915, last_state: 29920, default_state: 29918, wire_item: 728, wire_block: 807, wire_first_state: 18349, input_name: "gray_concrete", input_item: 622 },
    ConcreteSlab { name: "light_gray_concrete_slab", item: 1514, block: 1176, first_state: 29921, last_state: 29926, default_state: 29924, wire_item: 729, wire_block: 808, wire_first_state: 18355, input_name: "light_gray_concrete", input_item: 623 },
    ConcreteSlab { name: "cyan_concrete_slab", item: 1515, block: 1177, first_state: 29927, last_state: 29932, default_state: 29930, wire_item: 730, wire_block: 809, wire_first_state: 18361, input_name: "cyan_concrete", input_item: 624 },
    ConcreteSlab { name: "purple_concrete_slab", item: 1516, block: 1178, first_state: 29933, last_state: 29938, default_state: 29936, wire_item: 731, wire_block: 810, wire_first_state: 18367, input_name: "purple_concrete", input_item: 625 },
    ConcreteSlab { name: "blue_concrete_slab", item: 1517, block: 1179, first_state: 29939, last_state: 29944, default_state: 29942, wire_item: 732, wire_block: 811, wire_first_state: 18373, input_name: "blue_concrete", input_item: 626 },
    ConcreteSlab { name: "brown_concrete_slab", item: 1518, block: 1180, first_state: 29945, last_state: 29950, default_state: 29948, wire_item: 733, wire_block: 812, wire_first_state: 18379, input_name: "brown_concrete", input_item: 627 },
    ConcreteSlab { name: "green_concrete_slab", item: 1519, block: 1181, first_state: 29951, last_state: 29956, default_state: 29954, wire_item: 734, wire_block: 813, wire_first_state: 18385, input_name: "green_concrete", input_item: 628 },
    ConcreteSlab { name: "red_concrete_slab", item: 1520, block: 1182, first_state: 29957, last_state: 29962, default_state: 29960, wire_item: 735, wire_block: 814, wire_first_state: 18391, input_name: "red_concrete", input_item: 629 },
    ConcreteSlab { name: "black_concrete_slab", item: 1521, block: 1183, first_state: 29963, last_state: 29968, default_state: 29966, wire_item: 736, wire_block: 815, wire_first_state: 18397, input_name: "black_concrete", input_item: 630 },
];

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

    fn extension(kind: RegistryKind, space: IdSpace, id: u32) -> Option<(u32, u32, u32)> {
        CONCRETE_SLABS.iter().find_map(|row| {
            let (first, last, wire) = match kind {
                RegistryKind::Item => (row.item as u32, row.item as u32, row.wire_item),
                RegistryKind::Block => (row.block as u32, row.block as u32, row.wire_block),
                RegistryKind::BlockState => (row.first_state as u32, row.last_state as u32, row.wire_first_state),
                _ => return None,
            };
            match space {
                IdSpace::Canonical if (first..=last).contains(&id) => Some((id, first, wire)),
                IdSpace::Wire(JavaVersion::V26_3) if (wire..=wire + last - first).contains(&id) => Some((first + id - wire, first, wire)),
                _ => None,
            }
        })
    }

    pub fn map(self, kind: RegistryKind, from: IdSpace, to: IdSpace, id: u32) -> Result<u32, Error> {
        // Оба Wire обозначают настоящие протокольные пространства. Даже при
        // preview Canonical1506 не становится допустимым Wire775 item1506.
        if self.white_slab && let Some((canonical, first, wire)) = Self::extension(kind, from, id) {
                return match to {
                    IdSpace::Canonical => Ok(canonical),
                    IdSpace::Wire(JavaVersion::V26_3) => {
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
        for row in &CONCRETE_SLABS {
            for (kind, first, last, wire) in [(RegistryKind::Item,row.item as u32,row.item as u32,row.wire_item),
                (RegistryKind::Block,row.block as u32,row.block as u32,row.wire_block),(RegistryKind::BlockState,row.first_state as u32,row.last_state as u32,row.wire_first_state)] {
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
