// Общие карты Java → ClientboundMapItemData (944, minecraft-data protocol.json).
// Вики, Map item format: «base colors», оттенки 180/220/255/135.
// Bedrock принимает ABGR, не индекс палитры Java. [проверить] цвета и значки
// живым клиентом; расчёт поверхности остаётся в crate::maps.
use super::codec::Out;
use crate::maps::{Map, Patch};

// Палитра Java до 26.1.2; неизвестные индексы прозрачны.
const COLORS: [u32; 62] = [
    0,0x7fb238,0xf7e9a3,0xc7c7c7,0xff0000,0xa0a0ff,0xa7a7a7,0x007c00,
    0xffffff,0xa4a8b8,0x976d4d,0x707070,0x4040ff,0x8f7748,0xfffcf5,0xd87f33,
    0xb24cd8,0x6699d8,0xe5e533,0x7fcc19,0xf27fa5,0x4c4c4c,0x999999,0x4c7f99,
    0x7f3fb2,0x334cb2,0x664c33,0x667f33,0x993333,0x191919,0xfaee4d,0x5cdbd5,
    0x4a80ff,0x00d93a,0x815631,0x700200,0xd1b1a1,0x9f5224,0x95576c,0x706c8a,
    0xba8524,0x677535,0xa04d4e,0x392923,0x876b62,0x575c5c,0x7a4958,0x4c3e5c,
    0x4c3223,0x4c522a,0x8e3c2e,0x251610,0xbd3031,0x943f61,0x5c191d,0x167e86,
    0x3a8e8c,0x562c3e,0x14b485,0x646464,0xd8af93,0x7fa796,
];
fn pixel(index: u8) -> u32 {
    let base = usize::from(index / 4);
    if base == 0 || base >= COLORS.len() { return 0; }
    let rgb = COLORS[base]; let shade = [180,220,255,135][usize::from(index % 4)];
    let r = (rgb >> 16) * shade / 255; let g = (rgb >> 8 & 255) * shade / 255; let b = (rgb & 255) * shade / 255;
    0xff000000 | b << 16 | g << 8 | r
}
fn icon(java: i32) -> Option<u8> {
    Some(match java {
        0 => 0, 1 => 1, 2 => 2, 3 => 3, 4 => 4, 5 => 5, 6 => 6, 7 => 13,
        8 => 14, 9 => 15, 26 => 4, // mansion, monument, treasure X
        27..=31 => (java - 10) as u8, 32 => 22, 33 => 23,
        _ => return None,
    })
}
/// Декорации: тип Java, x/z, поворот. Texture отсутствует, если пиксели прежние.
pub fn packet(map: &Map, patch: Option<&Patch>, icons: &[(i32, i8, i8, u8)], initial: bool) -> Vec<u8> {
    let mut out = Out::packet(67);
    let flags = 4 | if patch.is_some() { 2 } else { 0 } | if initial { 8 } else { 0 };
    let dimension = match map.dimension { 1 => 1, 2 => 2, _ => 0 };
    out.zigzag64(i64::from(map.id)).varint(flags).u8(dimension).bool(map.locked);
    out.block_position(map.x, 0, map.z);
    if initial { out.varint(0); }
    out.u8(map.scale).varint(0); // нет tracked objects: значки уже рассчитаны общим кодом
    let icons: Vec<_> = icons.iter().filter_map(|&(kind,x,z,rotation)| Some((icon(kind)?,x,z,rotation))).collect();
    out.varint(icons.len() as u32);
    for (kind,x,z,rotation) in icons { out.u8(kind).u8(rotation).u8(x as u8).u8(z as u8).string("").varint(u32::MAX); }
    if let Some((x,z,width,height,pixels)) = patch {
        out.zigzag32(i32::from(*width)).zigzag32(i32::from(*height)).zigzag32(i32::from(*x)).zigzag32(i32::from(*z));
        out.varint(pixels.len() as u32);
        for &index in pixels { out.varint(pixel(index)); }
    }
    out.bytes
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::bedrock::codec::In;
    #[test]
    fn map_texture_and_icons_decode_to_end() {
        let map = Map { id:123, scale:2, x:-64, z:128, dimension:1, locked:true, colors:vec![0;16384], destination:None };
        let patch = (3,4,2,1,vec![6,0]);
        let bytes = packet(&map, Some(&patch), &[(8,-2,3,4)], true); let mut r = In::new(&bytes);
        assert_eq!(r.varint(), Some(67)); assert_eq!(r.zigzag64(), Some(123)); assert_eq!(r.varint(), Some(14));
        assert_eq!(r.u8(), Some(1)); assert_eq!(r.bool(), Some(true));
        assert_eq!((r.zigzag32(),r.zigzag32(),r.zigzag32()), (Some(-64),Some(0),Some(128)));
        assert_eq!(r.varint(), Some(0)); assert_eq!(r.u8(), Some(2)); assert_eq!(r.varint(), Some(0)); assert_eq!(r.varint(), Some(1));
        assert_eq!(r.take(4), Some([14,4,254,3].as_slice())); assert_eq!(r.string().as_deref(), Some("")); assert_eq!(r.varint(), Some(u32::MAX));
        assert_eq!((r.zigzag32(),r.zigzag32(),r.zigzag32(),r.zigzag32()), (Some(2),Some(1),Some(3),Some(4)));
        assert_eq!(r.varint(), Some(2)); assert_eq!(r.varint(), Some(0xff38b27f)); assert_eq!(r.varint(), Some(0)); assert_eq!(r.remaining(), 0);
        let bytes = packet(&map, None, &[], false);
        assert_eq!(bytes, [67,246,1,4,1,1,127,0,128,2,2,0,0]);
    }
}
