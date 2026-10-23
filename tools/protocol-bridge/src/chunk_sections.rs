//! Собственные палитры секций775/776/777. Wiki Chunk format oldid3661930:
//! «length ... no longer sent»; значения не пересекают границу long.
//! Динамические ID биомов должны совпадать у обеих сторон (контракт вызывающего).
use std::collections::BTreeMap;
use crate::content::{ContentCodec, IdSpace};
use crate::{JavaVersion, ids::{self, RegistryKind}};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error { UnsupportedVersion, Invalid, Truncated, TooLarge, Id(ids::IdTranslationError), Content(crate::content::Error) }

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Section {
    pub blocks_present: u16,
    pub fluids_present: u16,
    pub blocks: Vec<u32>,
    pub biomes: Vec<u32>,
}

fn content_error(error: crate::content::Error) -> Error {
    match error { crate::content::Error::Id(error) => Error::Id(error), error => Error::Content(error) }
}

fn block_bits(version: JavaVersion) -> Result<u8, Error> {
    // 776:300 авторских состояний, direct width15 по обычному MapChunk.
    // Полный chunk/admission этим низкоуровневым helper не включается.
    match version {
        JavaVersion::V1_21_11 | JavaVersion::V26_1_2 | JavaVersion::V26_2 => Ok(15),
        JavaVersion::V26_3 => Ok(16),
        JavaVersion::V1_21_5 | JavaVersion::V1_21_6 | JavaVersion::V1_21_8 | JavaVersion::V1_21_9 | JavaVersion::V1_12_2 => Err(Error::UnsupportedVersion),
    }
}
fn bits(count: u32) -> u8 { (32 - count.saturating_sub(1).leading_zeros()).max(1) as u8 }
fn uint(out: &mut Vec<u8>, mut value: u32) {
    while value >= 128 { out.push((value as u8 & 127) | 128); value >>= 7; }
    out.push(value as u8);
}

pub fn decode(version: JavaVersion, bytes: &[u8], sections: usize, biome_count: u32) -> Result<Vec<Section>, Error> {
    decode_with_content(version,bytes,sections,biome_count,IdSpace::Wire(version),ContentCodec::default())
}
pub fn decode_with_content(version: JavaVersion, bytes: &[u8], sections: usize, biome_count: u32, space: IdSpace, content: ContentCodec) -> Result<Vec<Section>, Error> {
    let direct = block_bits(version)?;
    if bytes.len() > 8 * 1024 * 1024 || sections > 256 { return Err(Error::TooLarge); }
    if sections == 0 || biome_count == 0 || biome_count > 65_536 { return Err(Error::Invalid); }
    let mut input = Cursor(bytes);
    let mut out = Vec::with_capacity(sections);
    for _ in 0..sections {
        let blocks_present = u16::from_be_bytes(input.take(2)?.try_into().map_err(|_| Error::Invalid)?);
        // 774-client775-v6: клиент принял секции только с одним счётчиком.
        let fluids_present = if version==JavaVersion::V1_21_11 {0} else {u16::from_be_bytes(input.take(2)?.try_into().map_err(|_| Error::Invalid)?)};
        if blocks_present > 4096 || fluids_present > 4096 { return Err(Error::Invalid); }
        let blocks = input.palette(4096, 4, 8, direct)?;
        for &id in &blocks { content.map(RegistryKind::BlockState, space, space, id).map_err(content_error)?; }
        let biomes = input.palette(64, 1, 3, bits(biome_count))?;
        if biomes.iter().any(|id| *id >= biome_count) { return Err(Error::Invalid); }
        out.push(Section { blocks_present, fluids_present, blocks, biomes });
    }
    if !input.0.is_empty() { return Err(Error::Invalid); }
    Ok(out)
}

pub fn translate(from: JavaVersion, to: JavaVersion, bytes: &[u8], sections: usize, shared_biome_count: u32) -> Result<Vec<u8>, Error> {
    block_bits(to)?;
    let mut decoded = decode(from, bytes, sections, shared_biome_count)?;
    for section in &mut decoded {
        for id in &mut section.blocks { *id = ids::translate(RegistryKind::BlockState, from, to, *id).map_err(Error::Id)?; }
    }
    encode(to, &decoded, shared_biome_count)
}

pub fn encode(version: JavaVersion, sections: &[Section], biome_count: u32) -> Result<Vec<u8>, Error> {
    encode_with_content(version,sections,biome_count,IdSpace::Wire(version),ContentCodec::default())
}
pub fn encode_with_content(version: JavaVersion, sections: &[Section], biome_count: u32, space: IdSpace, content: ContentCodec) -> Result<Vec<u8>, Error> {
    let direct = block_bits(version)?;
    if sections.is_empty() || sections.len() > 256 || biome_count == 0 || biome_count > 65_536 { return Err(Error::Invalid); }
    let mut out = Vec::new();
    for section in sections {
        if section.blocks.len() != 4096 || section.biomes.len() != 64 || section.blocks_present > 4096 || section.fluids_present > 4096 {
            return Err(Error::Invalid);
        }
        for &id in &section.blocks {
            content.map(RegistryKind::BlockState, space, space, id).map_err(content_error)?;
            if id >= (1u32 << direct) { return Err(Error::Invalid); }
        }
        if section.biomes.iter().any(|id| *id >= biome_count) { return Err(Error::Invalid); }
        out.extend(section.blocks_present.to_be_bytes()); if version!=JavaVersion::V1_21_11 {out.extend(section.fluids_present.to_be_bytes());}
        palette(&mut out, &section.blocks, 4, 8, direct);
        palette(&mut out, &section.biomes, 1, 3, bits(biome_count));
    }
    Ok(out)
}

fn palette(out: &mut Vec<u8>, values: &[u32], minimum: u8, maximum: u8, direct: u8) {
    let mut ids = BTreeMap::new(); let mut entries = Vec::new();
    for &id in values {
        if let std::collections::btree_map::Entry::Vacant(entry) = ids.entry(id) {
            entry.insert(entries.len() as u32); entries.push(id);
        }
    }
    if entries.len() == 1 { out.push(0); uint(out, entries[0]); return; }
    let width = bits(entries.len() as u32).max(minimum);
    let indirect = width <= maximum;
    let width = if indirect { width } else { direct };
    out.push(width);
    if indirect { uint(out, entries.len() as u32); for id in entries { uint(out, id); } }
    for group in values.chunks(64 / usize::from(width)) {
        let mut word = 0u64;
        for (index, id) in group.iter().enumerate() {
            let value = if indirect { ids[id] } else { *id };
            word |= u64::from(value) << (index * usize::from(width));
        }
        out.extend(word.to_be_bytes());
    }
}

struct Cursor<'a>(&'a [u8]);
impl<'a> Cursor<'a> {
    fn take(&mut self, size: usize) -> Result<&'a [u8], Error> {
        let (value, rest) = self.0.split_at_checked(size).ok_or(Error::Truncated)?; self.0 = rest; Ok(value)
    }
    fn uint(&mut self) -> Result<u32, Error> {
        let mut value = 0u32;
        for shift in (0..35).step_by(7) {
            let byte = self.take(1)?[0];
            if shift == 28 && byte & 0xf8 != 0 { return Err(Error::Invalid); }
            value |= u32::from(byte & 127) << shift;
            if byte < 128 { return Ok(value); }
        }
        Err(Error::Invalid)
    }
    fn palette(&mut self, count: usize, minimum: u8, maximum: u8, direct: u8) -> Result<Vec<u32>, Error> {
        let width = self.take(1)?[0];
        if width == 0 { return Ok(vec![self.uint()?; count]); }
        let indirect = (minimum..=maximum).contains(&width);
        if !indirect && (width != direct || width <= maximum) { return Err(Error::Invalid); }
        let mut entries = Vec::new();
        if indirect {
            let size = self.uint()? as usize;
            if size == 0 || size > (1 << width) { return Err(Error::Invalid); }
            for _ in 0..size { entries.push(self.uint()?); }
        }
        let per_word = 64 / usize::from(width);
        let bytes = self.take(count.div_ceil(per_word) * 8)?;
        let mask = (1u64 << width) - 1;
        let mut values = Vec::with_capacity(count);
        for bytes in bytes.as_chunks::<8>().0 {
            let word = u64::from_be_bytes(*bytes);
            for index in 0..per_word.min(count - values.len()) {
                let value = ((word >> (index * usize::from(width))) & mask) as u32;
                values.push(if indirect { *entries.get(value as usize).ok_or(Error::Invalid)? } else { value });
            }
        }
        Ok(values)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use JavaVersion::{V26_1_2 as OLD, V26_3 as NEW};
    fn section(values: Vec<u32>) -> Section { Section { blocks_present: 4096, fluids_present: 0, blocks: values, biomes: (0..64).map(|i| i % 9).collect() } }
    #[test]
    fn single_indirect_and_direct_palettes_round_trip_with_id_translation() {
        for count in [1, 2, 16, 17, 256, 257, 4096] {
            let entries: Vec<u32> = (0..29_873).rev().filter(|id| ids::translate(RegistryKind::BlockState, OLD, NEW, *id).is_ok()).take(count).collect();
            let source = vec![section((0..4096).map(|i| entries[i % count]).collect())];
            let bytes = encode(OLD, &source, 59).unwrap();
            let translated = translate(OLD, NEW, &bytes, 1, 59).unwrap();
            let result = decode(NEW, &translated, 1, 59).unwrap();
            for (old, new) in source[0].blocks.iter().zip(&result[0].blocks) {
                assert_eq!(ids::translate(RegistryKind::BlockState, OLD, NEW, *old), Ok(*new));
            }
            assert_eq!(result[0].biomes, source[0].biomes);
            if count > 256 { assert_eq!(bytes[4], 15); assert_eq!(translated[4], 16); }
            let reversed = translate(NEW, OLD, &translated, 1, 59).unwrap();
            assert_eq!(decode(OLD, &reversed, 1, 59).unwrap(), source);
        }
    }
    #[test]
    fn independently_packed_words_preserve_voxel_order_and_padding() {
        // Документированный порядок x,z,y; значения внутри BE-long идут от LSB.
        // Здесь байты заданы напрямую, без вызова нашего encoder.
        for (version, width, word) in [(OLD, 15, 0x0000_6000_8000_8000u64), (NEW, 16, 0x0003_0002_0001_0000)] {
            let mut bytes = vec![0x0f, 0xff, 0, 0, width];
            bytes.extend(word.to_be_bytes());
            bytes.resize(5 + 1024 * 8, 0);
            bytes.extend([0, 7]); // Один биом с ID7; длины массива long нет.
            let decoded = decode(version, &bytes, 1, 59).unwrap();
            assert_eq!(&decoded[0].blocks[..5], &[0, 1, 2, 3, 0]);
            assert_eq!(decoded[0].biomes, vec![7; 64]);
        }
        // Пять бит:12 значений в long, верхние4 бита — padding, не начало13-го.
        let mut bytes = vec![0, 2, 0, 0, 5, 17];
        bytes.extend(0..17);
        bytes.extend(0xf000_0000_0000_0001u64.to_be_bytes());
        bytes.extend(2u64.to_be_bytes());
        bytes.resize(6 + 17 + 4096usize.div_ceil(12) * 8, 0);
        bytes.extend([0, 0]);
        let decoded = decode(OLD, &bytes, 1, 59).unwrap();
        assert_eq!(decoded[0].blocks[0], 1);
        assert_eq!(decoded[0].blocks[11], 0);
        assert_eq!(decoded[0].blocks[12], 2);
    }

    #[test]
    fn malformed_data_and_content_missing_in_old_client_fail() {
        let bytes = encode(OLD, &[section(vec![0; 4096])], 59).unwrap();
        for n in 0..bytes.len() { assert!(decode(OLD, &bytes[..n], 1, 59).is_err()); }
        let mut extra = bytes.clone(); extra.push(0); assert!(decode(OLD, &extra, 1, 59).is_err());
        let mut bad_width = bytes; bad_width[4] = 9; assert!(decode(OLD, &bad_width, 1, 59).is_err());
        let missing = (0..35_723).find(|id| matches!(ids::translate(RegistryKind::BlockState, NEW, OLD, *id), Err(ids::IdTranslationError::MissingTarget(_)))).unwrap();
        let bytes = encode(NEW, &[section(vec![missing; 4096])], 59).unwrap();
        assert!(matches!(translate(NEW, OLD, &bytes, 1, 59), Err(Error::Id(ids::IdTranslationError::MissingTarget(_)))));
        assert!(decode(NEW, &[], usize::MAX, 59).is_err());
    }
}
