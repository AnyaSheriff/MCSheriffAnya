//! Чанки340: закреплённый protocol.json и собственная запись построенных секций.
//! В отличие от современных секций, значения пересекают границу long.
//! Свет находится внутри секции; наличие sky light задаётся измерением.
use crate::{registry, wire::{Cursor, Error, uint}};

const MAX_BYTES: usize = 2 * 1024 * 1024;
const MAX_ENTITIES: usize = 4096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Section {
    /// Старый state = (числовой block ID << 4) | metadata. Не ID775/777!
    pub blocks: Vec<u16>,
    pub block_light: Vec<u8>,
    pub sky_light: Option<Vec<u8>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Chunk {
    pub x: i32,
    pub z: i32,
    /// Только присутствующие секции, по возрастанию Y; остальные не обновляются
    /// при partial-пакете и очищаются при полном пакете (biomes=Some).
    pub sections: Vec<(u8, Section)>,
    pub biomes: Option<[u8; 256]>,
    /// Именованный NBT340. Это не разрешение переносить NBT в другую версию.
    pub block_entities: Vec<Vec<u8>>,
}

fn word(input: &mut Cursor<'_>) -> Result<i32, Error> {
    Ok(i32::from_be_bytes(input.take(4)?.try_into().map_err(|_| Error::Invalid)?))
}

fn named_nbt_length(bytes: &[u8]) -> Result<usize, Error> {
    let mut input = Cursor::new(bytes);
    if input.take(1)?[0] != 10 { return Err(Error::Invalid); }
    let name = u16::from_be_bytes(input.take(2)?.try_into().map_err(|_| Error::Invalid)?);
    input.take(usize::from(name))?;
    let header = bytes.len() - input.remaining();
    let size = registry::nbt_payload_length(10, input.rest).map_err(|_| Error::Invalid)?;
    Ok(header + size)
}

pub(crate) fn validate_block_entities(entities: &[Vec<u8>]) -> Result<(), Error> {
    if entities.len() > MAX_ENTITIES { return Err(Error::TooLarge); }
    let mut total = 0usize;
    for data in entities {
        total = total.checked_add(data.len()).ok_or(Error::TooLarge)?;
        if total > MAX_BYTES { return Err(Error::TooLarge); }
        if named_nbt_length(data)? != data.len() { return Err(Error::Invalid); }
    }
    Ok(())
}

impl Chunk {
    /// Тело MapChunk без ID0x20, транспортного framing и сжатия.
    pub fn decode(bytes: &[u8], has_sky_light: bool) -> Result<Self, Error> {
        if bytes.len() > MAX_BYTES { return Err(Error::TooLarge); }
        let mut input = Cursor::new(bytes);
        let x = word(&mut input)?;
        let z = word(&mut input)?;
        let full = input.boolean()?;
        let mask = input.count(65535)?;
        let length = input.count(MAX_BYTES)?;
        let mut packed = Cursor::new(input.take(length)?);
        let mut sections = Vec::with_capacity(mask.count_ones() as usize);
        for y in 0..16 {
            if mask & (1 << y) == 0 { continue; }
            let width = packed.take(1)?[0];
            if !(4..=8).contains(&width) && width != 13 { return Err(Error::Invalid); }
            let count = packed.count(256)?;
            if (width <= 8 && (count == 0 || count > (1 << width))) || (width == 13 && count != 0) { return Err(Error::Invalid); }
            let mut palette = Vec::with_capacity(count);
            for _ in 0..count { palette.push(packed.count(8191)? as u16); }
            if packed.count(832)? != usize::from(width) * 64 { return Err(Error::Invalid); }
            let words = packed.take(usize::from(width) * 512)?.as_chunks::<8>().0;
            let mut blocks = Vec::with_capacity(4096);
            for i in 0..4096 {
                let bit = i * usize::from(width);
                let mut value = u64::from_be_bytes(words[bit / 64]) >> (bit % 64);
                if bit % 64 + usize::from(width) > 64 {
                    value |= u64::from_be_bytes(words[bit / 64 + 1]) << (64 - bit % 64);
                }
                let value = (value & ((1 << width) - 1)) as u16;
                blocks.push(if width == 13 { value } else { *palette.get(usize::from(value)).ok_or(Error::Invalid)? });
            }
            let block_light = packed.take(2048)?.to_vec();
            let sky_light = if has_sky_light { Some(packed.take(2048)?.to_vec()) } else { None };
            sections.push((y, Section { blocks, block_light, sky_light }));
        }
        let biomes = if full { Some(packed.take(256)?.try_into().map_err(|_| Error::Invalid)?) } else { None };
        if packed.remaining() != 0 { return Err(Error::Invalid); }
        let count = input.count(MAX_ENTITIES)?;
        if count > input.remaining() / 4 { return Err(Error::Truncated); }
        let mut block_entities = Vec::with_capacity(count);
        for _ in 0..count {
            let length = named_nbt_length(input.rest)?;
            block_entities.push(input.take(length)?.to_vec());
        }
        if input.remaining() != 0 { return Err(Error::Invalid); }
        Ok(Self { x, z, sections, biomes, block_entities })
    }

    /// Каноническая палитра по первому появлению. Сохраняется смысл, не порядок
    /// исходной палитры. Все16секций/свет/биомы валидируются до выдачи пакета.
    pub fn encode(&self, has_sky_light: bool) -> Result<Vec<u8>, Error> {
        if self.sections.len() > 16 || self.block_entities.len() > MAX_ENTITIES { return Err(Error::TooLarge); }
        validate_block_entities(&self.block_entities)?;
        let mut packed = Vec::new();
        let mut mask = 0u32;
        let mut previous = None;
        for (y, section) in &self.sections {
            if *y >= 16 || previous.is_some_and(|old| old >= *y) { return Err(Error::Invalid); }
            previous = Some(*y);
            mask |= 1 << y;
            if section.blocks.len() != 4096 || section.block_light.len() != 2048
                || section.sky_light.is_some() != has_sky_light
                || section.sky_light.as_ref().is_some_and(|v| v.len() != 2048) { return Err(Error::Invalid); }
            let mut indices = vec![u16::MAX; 8192];
            let mut entries = Vec::new();
            for &state in &section.blocks {
                let index = indices.get_mut(usize::from(state)).ok_or(Error::Invalid)?;
                if *index == u16::MAX { *index = entries.len() as u16; entries.push(state); }
            }
            let width = if entries.len() > 256 { 13 } else { (usize::BITS - (entries.len() - 1).leading_zeros()).max(4) as usize };
            packed.push(width as u8);
            if width == 13 { packed.push(0); } else {
                uint(&mut packed, entries.len() as u32);
                for state in entries { uint(&mut packed, u32::from(state)); }
            }
            uint(&mut packed, (width * 64) as u32);
            let mut words = vec![0u64; width * 64];
            for (i, &state) in section.blocks.iter().enumerate() {
                let value = u64::from(if width == 13 { state } else { indices[usize::from(state)] });
                let bit = i * width;
                words[bit / 64] |= value << (bit % 64);
                if bit % 64 + width > 64 { words[bit / 64 + 1] |= value >> (64 - bit % 64); }
            }
            for word in words { packed.extend(word.to_be_bytes()); }
            packed.extend(&section.block_light);
            if let Some(light) = &section.sky_light { packed.extend(light); }
        }
        if let Some(biomes) = &self.biomes { packed.extend(biomes); }
        let mut out = Vec::with_capacity(packed.len() + 32);
        out.extend(self.x.to_be_bytes()); out.extend(self.z.to_be_bytes());
        out.push(u8::from(self.biomes.is_some())); uint(&mut out, mask);
        uint(&mut out, packed.len() as u32); out.extend(packed);
        uint(&mut out, self.block_entities.len() as u32);
        for data in &self.block_entities {
            if data.len() > MAX_BYTES || out.len() + data.len() > MAX_BYTES { return Err(Error::TooLarge); }
            out.extend(data);
        }
        if out.len() > MAX_BYTES { return Err(Error::TooLarge); }
        Ok(out)
    }
}

/// Явное окно старого клиента. Смена окна требует полного обновления мира,
/// сущностей и телепорта; этот тип сам не обрезает/не переключает мир.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HeightWindow { origin: i32 }

impl HeightWindow {
    pub fn new(origin: i32) -> Result<Self, Error> {
        if origin % 16 != 0 || origin.checked_add(255).is_none() { return Err(Error::Invalid); }
        Ok(Self { origin })
    }
    pub fn origin(self) -> i32 { self.origin }
    pub fn client_y(self, server_y: i32) -> Result<u8, Error> {
        server_y.checked_sub(self.origin).and_then(|y| u8::try_from(y).ok()).ok_or(Error::Invalid)
    }
    pub fn server_y(self, client_y: u8) -> i32 { self.origin + i32::from(client_y) }
    pub fn client_position(self, position: crate::teleport::Pose) -> Result<crate::teleport::Pose, Error> {
        let mut position = crate::legacy_control::resolve_teleport(position, 0, None)?;
        position.y -= f64::from(self.origin);
        if !(0.0..256.0).contains(&position.y) { return Err(Error::Invalid); }
        Ok(position)
    }
    pub fn server_position(self, position: crate::teleport::Pose) -> Result<crate::teleport::Pose, Error> {
        let mut position = crate::legacy_control::resolve_teleport(position, 0, None)?;
        if !(0.0..256.0).contains(&position.y) { return Err(Error::Invalid); }
        position.y += f64::from(self.origin);
        Ok(position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes(hex: &str) -> Vec<u8> {
        assert_eq!(hex.len() % 2, 0);
        hex.as_bytes().as_chunks::<2>().0.iter().map(|v| u8::from_str_radix(std::str::from_utf8(v).unwrap(), 16).unwrap()).collect()
    }

    fn envelope(section: &[u8], y: u8, biomes: bool) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend((-23i32).to_be_bytes()); out.extend(19i32.to_be_bytes());
        out.push(u8::from(biomes)); uint(&mut out, 1 << y);
        uint(&mut out, (section.len() + if biomes { 256 } else { 0 }) as u32);
        out.extend(section);
        if biomes { out.extend([1; 256]); }
        out.push(0);
        out
    }

    #[test]
    fn official_authored_sections_decode_every_observed_palette_width() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!("../testdata/legacy-chunk-observations.json")).unwrap();
        let cases = fixture["cases"].as_array().unwrap();
        assert_eq!(cases.len(), 7);
        for case in cases {
            let raw = bytes(case["section_hex"].as_str().unwrap());
            assert_eq!(u64::from(raw[0]), case["width"].as_u64().unwrap());
            let body = envelope(&raw, case["section_y"].as_u64().unwrap() as u8, true);
            let chunk = Chunk::decode(&body, true).unwrap();
            assert_eq!((chunk.x, chunk.z), (-23, 19));
            let mut expected: Vec<u16> = case["expected_prefix"].as_array().unwrap().iter().map(|v| v.as_u64().unwrap() as u16).collect();
            expected.resize(4096, case["remaining_state"].as_u64().unwrap() as u16);
            assert_eq!(chunk.sections[0].1.blocks, expected);
            assert_eq!(chunk.sections[0].1.block_light.as_slice(), &raw[raw.len()-4096..raw.len()-2048]);
            assert_eq!(chunk.sections[0].1.sky_light.as_ref().unwrap().as_slice(), &raw[raw.len()-2048..]);
            let canonical = chunk.encode(true).unwrap();
            assert_eq!(Chunk::decode(&canonical, true).unwrap(), chunk);
            assert_eq!(Chunk::decode(&canonical, true).unwrap().encode(true).unwrap(), canonical);
            for n in (0..24).chain([body.len()/2, body.len()-1]) { assert!(Chunk::decode(&body[..n], true).is_err()); }
            assert!(Chunk::decode(&body, false).is_err());
            let mut trailing = body; trailing.push(0);
            assert!(Chunk::decode(&trailing, true).is_err());
        }
    }

    #[test]
    fn explicit_partial_chunks_no_sky_and_named_nbt_preserve_semantics() {
        // Nether-форма здесь синтетическая, не выдаётся за запись оригинала.
        let section = Section { blocks: (0..4096).map(|v| v % 8192).collect(), block_light: (0..2048).map(|v| v as u8).collect(), sky_light: None };
        let mut chunk = Chunk { x: i32::MIN, z: i32::MAX, sections: vec![(0, section.clone()), (15, section)], biomes: None,
            block_entities: vec![vec![10, 0, 0, 3, 0, 1, b'x', 0, 0, 0, 1, 0]] };
        let data = chunk.encode(false).unwrap();
        assert_eq!(Chunk::decode(&data, false).unwrap(), chunk);
        assert!(Chunk::decode(&data, true).is_err());
        chunk.biomes = Some([255; 256]);
        assert_eq!(Chunk::decode(&chunk.encode(false).unwrap(), false).unwrap(), chunk);
        chunk.sections.clear(); chunk.block_entities.clear();
        assert_eq!(Chunk::decode(&chunk.encode(false).unwrap(), false).unwrap(), chunk);
        chunk.biomes = None;
        assert_eq!(Chunk::decode(&chunk.encode(false).unwrap(), false).unwrap(), chunk);
    }

    #[test]
    fn invalid_palette_lights_masks_and_entity_lengths_fail_closed() {
        let section = Section { blocks: vec![0; 4096], block_light: vec![0; 2048], sky_light: None };
        let mut chunk = Chunk { x: 0, z: 0, sections: vec![(0, section)], biomes: None, block_entities: vec![] };
        let baseline = chunk.clone();
        chunk.sections[0].1.blocks[0] = 8192; assert!(chunk.encode(false).is_err());
        chunk = baseline.clone(); chunk.sections[0].1.blocks.pop(); assert!(chunk.encode(false).is_err());
        chunk = baseline.clone(); chunk.sections[0].1.block_light.pop(); assert!(chunk.encode(false).is_err());
        chunk = baseline.clone(); chunk.sections.push(chunk.sections[0].clone()); assert!(chunk.encode(false).is_err());
        chunk = baseline.clone(); chunk.sections[0].0 = 16; assert!(chunk.encode(false).is_err());
        chunk = baseline.clone(); chunk.block_entities = vec![vec![10, 0, 0]]; assert!(chunk.encode(false).is_err());
        chunk = baseline; chunk.block_entities = vec![vec![0]]; assert!(chunk.encode(false).is_err());
        for section in [vec![3], vec![9], vec![13, 1, 0], vec![4, 0], vec![4, 17], vec![4, 1, 0, 0]] {
            assert!(Chunk::decode(&envelope(&section, 0, false), false).is_err());
        }
        let mut out_of_palette = vec![4, 1, 0]; uint(&mut out_of_palette, 256);
        out_of_palette.extend(1u64.to_be_bytes()); out_of_palette.resize(4 + 1 + 2048 + 2048, 0);
        assert!(Chunk::decode(&envelope(&out_of_palette, 0, false), false).is_err());
        assert!(Chunk::decode(&vec![0; MAX_BYTES+1], false).is_err());
    }

    #[test]
    fn height_window_is_explicit_reversible_and_never_clamps_missing_world() {
        use crate::teleport::Pose;
        let window = HeightWindow::new(-64).unwrap();
        assert_eq!(window.origin(), -64);
        for y in 0..=255 { assert_eq!(window.client_y(window.server_y(y)), Ok(y)); }
        assert!(window.client_y(-65).is_err()); assert!(window.client_y(192).is_err());
        assert!(HeightWindow::new(1).is_err()); assert!(HeightWindow::new(i32::MAX-15).is_err());
        let position = Pose { x: 9.5, y: -63.75, z: -8.0, yaw: 180.0, pitch: -20.0 };
        let client = window.client_position(position).unwrap();
        assert_eq!(client.y, 0.25); assert_eq!(window.server_position(client).unwrap(), position);
        for y in [f64::NAN, f64::INFINITY, -64.01, 192.0] { assert!(window.client_position(Pose { y, ..position }).is_err()); }
        assert!(window.server_position(Pose { y: 256.0, ..client }).is_err());
    }
}
