//! Внутренний каталог не является версией сетевого протокола.
//! Preview бетонных и шерстяных плит: выбранные pinned JSON, базовые ID775 неизменны.
use crate::{JavaVersion, ids::{self, RegistryKind}};

/// Неизменяемые внутренние идентичности; существующие строки нельзя перенумеровывать.
/// Источники: selected concrete-slabs-26.3.json / wool-slabs-26.3.json; не игровое наблюдение.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SlabFamily { Concrete, Wool }

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SlabDescriptor {
    pub family: SlabFamily,
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

/// Совместимое имя прежнего API; принадлежность задаётся явным family.
pub type ConcreteSlab = SlabDescriptor;

pub static CONCRETE_SLABS: [ConcreteSlab; 16] = [
    ConcreteSlab { family: SlabFamily::Concrete, name: "white_concrete_slab", item: 1506, block: 1168, first_state: 29873, last_state: 29878, default_state: 29876, wire_item: 721, wire_block: 800, wire_first_state: 18307, input_name: "white_concrete", input_item: 615 },
    ConcreteSlab { family: SlabFamily::Concrete, name: "orange_concrete_slab", item: 1507, block: 1169, first_state: 29879, last_state: 29884, default_state: 29882, wire_item: 722, wire_block: 801, wire_first_state: 18313, input_name: "orange_concrete", input_item: 616 },
    ConcreteSlab { family: SlabFamily::Concrete, name: "magenta_concrete_slab", item: 1508, block: 1170, first_state: 29885, last_state: 29890, default_state: 29888, wire_item: 723, wire_block: 802, wire_first_state: 18319, input_name: "magenta_concrete", input_item: 617 },
    ConcreteSlab { family: SlabFamily::Concrete, name: "light_blue_concrete_slab", item: 1509, block: 1171, first_state: 29891, last_state: 29896, default_state: 29894, wire_item: 724, wire_block: 803, wire_first_state: 18325, input_name: "light_blue_concrete", input_item: 618 },
    ConcreteSlab { family: SlabFamily::Concrete, name: "yellow_concrete_slab", item: 1510, block: 1172, first_state: 29897, last_state: 29902, default_state: 29900, wire_item: 725, wire_block: 804, wire_first_state: 18331, input_name: "yellow_concrete", input_item: 619 },
    ConcreteSlab { family: SlabFamily::Concrete, name: "lime_concrete_slab", item: 1511, block: 1173, first_state: 29903, last_state: 29908, default_state: 29906, wire_item: 726, wire_block: 805, wire_first_state: 18337, input_name: "lime_concrete", input_item: 620 },
    ConcreteSlab { family: SlabFamily::Concrete, name: "pink_concrete_slab", item: 1512, block: 1174, first_state: 29909, last_state: 29914, default_state: 29912, wire_item: 727, wire_block: 806, wire_first_state: 18343, input_name: "pink_concrete", input_item: 621 },
    ConcreteSlab { family: SlabFamily::Concrete, name: "gray_concrete_slab", item: 1513, block: 1175, first_state: 29915, last_state: 29920, default_state: 29918, wire_item: 728, wire_block: 807, wire_first_state: 18349, input_name: "gray_concrete", input_item: 622 },
    ConcreteSlab { family: SlabFamily::Concrete, name: "light_gray_concrete_slab", item: 1514, block: 1176, first_state: 29921, last_state: 29926, default_state: 29924, wire_item: 729, wire_block: 808, wire_first_state: 18355, input_name: "light_gray_concrete", input_item: 623 },
    ConcreteSlab { family: SlabFamily::Concrete, name: "cyan_concrete_slab", item: 1515, block: 1177, first_state: 29927, last_state: 29932, default_state: 29930, wire_item: 730, wire_block: 809, wire_first_state: 18361, input_name: "cyan_concrete", input_item: 624 },
    ConcreteSlab { family: SlabFamily::Concrete, name: "purple_concrete_slab", item: 1516, block: 1178, first_state: 29933, last_state: 29938, default_state: 29936, wire_item: 731, wire_block: 810, wire_first_state: 18367, input_name: "purple_concrete", input_item: 625 },
    ConcreteSlab { family: SlabFamily::Concrete, name: "blue_concrete_slab", item: 1517, block: 1179, first_state: 29939, last_state: 29944, default_state: 29942, wire_item: 732, wire_block: 811, wire_first_state: 18373, input_name: "blue_concrete", input_item: 626 },
    ConcreteSlab { family: SlabFamily::Concrete, name: "brown_concrete_slab", item: 1518, block: 1180, first_state: 29945, last_state: 29950, default_state: 29948, wire_item: 733, wire_block: 812, wire_first_state: 18379, input_name: "brown_concrete", input_item: 627 },
    ConcreteSlab { family: SlabFamily::Concrete, name: "green_concrete_slab", item: 1519, block: 1181, first_state: 29951, last_state: 29956, default_state: 29954, wire_item: 734, wire_block: 813, wire_first_state: 18385, input_name: "green_concrete", input_item: 628 },
    ConcreteSlab { family: SlabFamily::Concrete, name: "red_concrete_slab", item: 1520, block: 1182, first_state: 29957, last_state: 29962, default_state: 29960, wire_item: 735, wire_block: 814, wire_first_state: 18391, input_name: "red_concrete", input_item: 629 },
    ConcreteSlab { family: SlabFamily::Concrete, name: "black_concrete_slab", item: 1521, block: 1183, first_state: 29963, last_state: 29968, default_state: 29966, wire_item: 736, wire_block: 815, wire_first_state: 18397, input_name: "black_concrete", input_item: 630 },
];


/// Отдельное семейство: metadata не разрешает копировать правила добычи бетона.
pub static WOOL_SLABS: [SlabDescriptor; 16] = [
    SlabDescriptor { family: SlabFamily::Wool, name: "white_wool_slab", item: 1522, block: 1184, first_state: 29969, last_state: 29974, default_state: 29972, wire_item: 282, wire_block: 183, wire_first_state: 3705, input_name: "white_wool", input_item: 213 },
    SlabDescriptor { family: SlabFamily::Wool, name: "orange_wool_slab", item: 1523, block: 1185, first_state: 29975, last_state: 29980, default_state: 29978, wire_item: 283, wire_block: 184, wire_first_state: 3711, input_name: "orange_wool", input_item: 214 },
    SlabDescriptor { family: SlabFamily::Wool, name: "magenta_wool_slab", item: 1524, block: 1186, first_state: 29981, last_state: 29986, default_state: 29984, wire_item: 284, wire_block: 185, wire_first_state: 3717, input_name: "magenta_wool", input_item: 215 },
    SlabDescriptor { family: SlabFamily::Wool, name: "light_blue_wool_slab", item: 1525, block: 1187, first_state: 29987, last_state: 29992, default_state: 29990, wire_item: 285, wire_block: 186, wire_first_state: 3723, input_name: "light_blue_wool", input_item: 216 },
    SlabDescriptor { family: SlabFamily::Wool, name: "yellow_wool_slab", item: 1526, block: 1188, first_state: 29993, last_state: 29998, default_state: 29996, wire_item: 286, wire_block: 187, wire_first_state: 3729, input_name: "yellow_wool", input_item: 217 },
    SlabDescriptor { family: SlabFamily::Wool, name: "lime_wool_slab", item: 1527, block: 1189, first_state: 29999, last_state: 30004, default_state: 30002, wire_item: 287, wire_block: 188, wire_first_state: 3735, input_name: "lime_wool", input_item: 218 },
    SlabDescriptor { family: SlabFamily::Wool, name: "pink_wool_slab", item: 1528, block: 1190, first_state: 30005, last_state: 30010, default_state: 30008, wire_item: 288, wire_block: 189, wire_first_state: 3741, input_name: "pink_wool", input_item: 219 },
    SlabDescriptor { family: SlabFamily::Wool, name: "gray_wool_slab", item: 1529, block: 1191, first_state: 30011, last_state: 30016, default_state: 30014, wire_item: 289, wire_block: 190, wire_first_state: 3747, input_name: "gray_wool", input_item: 220 },
    SlabDescriptor { family: SlabFamily::Wool, name: "light_gray_wool_slab", item: 1530, block: 1192, first_state: 30017, last_state: 30022, default_state: 30020, wire_item: 290, wire_block: 191, wire_first_state: 3753, input_name: "light_gray_wool", input_item: 221 },
    SlabDescriptor { family: SlabFamily::Wool, name: "cyan_wool_slab", item: 1531, block: 1193, first_state: 30023, last_state: 30028, default_state: 30026, wire_item: 291, wire_block: 192, wire_first_state: 3759, input_name: "cyan_wool", input_item: 222 },
    SlabDescriptor { family: SlabFamily::Wool, name: "purple_wool_slab", item: 1532, block: 1194, first_state: 30029, last_state: 30034, default_state: 30032, wire_item: 292, wire_block: 193, wire_first_state: 3765, input_name: "purple_wool", input_item: 223 },
    SlabDescriptor { family: SlabFamily::Wool, name: "blue_wool_slab", item: 1533, block: 1195, first_state: 30035, last_state: 30040, default_state: 30038, wire_item: 293, wire_block: 194, wire_first_state: 3771, input_name: "blue_wool", input_item: 224 },
    SlabDescriptor { family: SlabFamily::Wool, name: "brown_wool_slab", item: 1534, block: 1196, first_state: 30041, last_state: 30046, default_state: 30044, wire_item: 294, wire_block: 195, wire_first_state: 3777, input_name: "brown_wool", input_item: 225 },
    SlabDescriptor { family: SlabFamily::Wool, name: "green_wool_slab", item: 1535, block: 1197, first_state: 30047, last_state: 30052, default_state: 30050, wire_item: 295, wire_block: 196, wire_first_state: 3783, input_name: "green_wool", input_item: 226 },
    SlabDescriptor { family: SlabFamily::Wool, name: "red_wool_slab", item: 1536, block: 1198, first_state: 30053, last_state: 30058, default_state: 30056, wire_item: 296, wire_block: 197, wire_first_state: 3789, input_name: "red_wool", input_item: 227 },
    SlabDescriptor { family: SlabFamily::Wool, name: "black_wool_slab", item: 1537, block: 1199, first_state: 30059, last_state: 30064, default_state: 30062, wire_item: 297, wire_block: 198, wire_first_state: 3795, input_name: "black_wool", input_item: 228 },
];

/// Дописываемый порядок canonical-каталога: старые бетонные ID не меняются.
pub fn slabs() -> impl Iterator<Item = &'static SlabDescriptor> {
    CONCRETE_SLABS.iter().chain(WOOL_SLABS.iter())
}

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
        slabs().find_map(|row| {
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
        for row in slabs() {
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
