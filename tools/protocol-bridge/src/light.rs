//! Свет775/776/777: pinned protocol.json, Wiki Light Data; реальныеfixtures вtestdata.
//! В775/776 маски — массив BE-long, в777 — массив байтов с младшими битами впереди.
use crate::JavaVersion;

const MAX_SECTIONS: usize = 256;
const MAX_MASK_BYTES: usize = (MAX_SECTIONS + 2).div_ceil(8);
const MAX_BYTES: usize = 2 * (MAX_SECTIONS + 2) * 2053 + 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error { UnsupportedVersion, Invalid, Truncated, TooLarge }

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Light {
    /// Номера битов одинаковы в обоих форматах. Хвостовые нули удалены.
    pub sky: Vec<u8>,
    pub block: Vec<u8>,
    pub empty_sky: Vec<u8>,
    pub empty_block: Vec<u8>,
    pub sky_arrays: Vec<Vec<u8>>,
    pub block_arrays: Vec<Vec<u8>>,
}

#[derive(Clone, Copy)]
enum MaskLayout { Longs, Bytes }

fn mask_layout(version: JavaVersion) -> Result<MaskLayout, Error> {
    // 776: ordinary-payloads-26.2.json, два собственных MapChunk с факелом.
    match version {
        JavaVersion::V26_1_2 | JavaVersion::V26_2 => Ok(MaskLayout::Longs),
        JavaVersion::V26_3 => Ok(MaskLayout::Bytes),
        JavaVersion::V1_12_2 => Err(Error::UnsupportedVersion),
    }
}
fn trim(mask: &mut Vec<u8>) { while mask.last() == Some(&0) { mask.pop(); } }
fn population(mask: &[u8]) -> usize { mask.iter().map(|b| b.count_ones() as usize).sum() }

impl Light {
    /// Только LightData, без номера пакета и координат чанка.
    pub fn decode(version: JavaVersion, payload: &[u8]) -> Result<Self, Error> {
        let layout = mask_layout(version)?;
        if payload.len() > MAX_BYTES { return Err(Error::TooLarge); }
        let mut input = Cursor(payload);
        let sky = input.mask(layout)?;
        let block = input.mask(layout)?;
        let empty_sky = input.mask(layout)?;
        let empty_block = input.mask(layout)?;
        let sky_arrays = input.arrays(population(&sky))?;
        let block_arrays = input.arrays(population(&block))?;
        if !input.0.is_empty() { return Err(Error::Invalid); }
        let light = Self { sky, block, empty_sky, empty_block, sky_arrays, block_arrays };
        light.validate()?;
        Ok(light)
    }

    fn validate(&self) -> Result<(), Error> {
        for (mask, empty, arrays) in [(&self.sky, &self.empty_sky, &self.sky_arrays), (&self.block, &self.empty_block, &self.block_arrays)] {
            for mask in [mask, empty] {
                if mask.len() > MAX_MASK_BYTES { return Err(Error::TooLarge); }
                // Последний неполный байт: не разрешаем секции за пределом.
                if mask.len() == MAX_MASK_BYTES && mask[MAX_MASK_BYTES - 1] >> ((MAX_SECTIONS + 2) % 8) != 0 {
                    return Err(Error::Invalid);
                }
            }
            if mask.iter().zip(empty).any(|(a, b)| a & b != 0) { return Err(Error::Invalid); }
            if population(mask) != arrays.len() || arrays.iter().any(|a| a.len() != 2048) { return Err(Error::Invalid); }
        }
        Ok(())
    }

    pub fn encode(&self, version: JavaVersion) -> Result<Vec<u8>, Error> {
        let layout = mask_layout(version)?;
        self.validate()?;
        let mut out = Vec::new();
        for mask in [&self.sky, &self.block, &self.empty_sky, &self.empty_block] {
            let mut mask = mask.clone(); trim(&mut mask);
            if matches!(layout, MaskLayout::Longs) {
                uint(&mut out, mask.len().div_ceil(8) as u32);
                for bytes in mask.chunks(8) {
                    let mut word = [0; 8]; word[..bytes.len()].copy_from_slice(bytes);
                    out.extend(u64::from_le_bytes(word).to_be_bytes());
                }
            } else {
                uint(&mut out, mask.len() as u32); out.extend(mask);
            }
        }
        for arrays in [&self.sky_arrays, &self.block_arrays] {
            uint(&mut out, arrays.len() as u32);
            for array in arrays { uint(&mut out, 2048); out.extend(array); }
        }
        Ok(out)
    }
}

struct Cursor<'a>(&'a [u8]);
impl<'a> Cursor<'a> {
    fn take(&mut self, count: usize) -> Result<&'a [u8], Error> {
        let (value, rest) = self.0.split_at_checked(count).ok_or(Error::Truncated)?;
        self.0 = rest; Ok(value)
    }
    fn count(&mut self, limit: usize) -> Result<usize, Error> {
        let mut value = 0u32;
        for shift in (0..35).step_by(7) {
            let byte = self.take(1)?[0];
            if shift == 28 && byte & 0xf8 != 0 { return Err(Error::Invalid); }
            value |= u32::from(byte & 127) << shift;
            if byte < 128 { return if value as usize <= limit { Ok(value as usize) } else { Err(Error::TooLarge) }; }
        }
        Err(Error::Invalid)
    }
    fn mask(&mut self, layout: MaskLayout) -> Result<Vec<u8>, Error> {
        let mut mask = if matches!(layout, MaskLayout::Longs) {
            let count = self.count(MAX_MASK_BYTES.div_ceil(8))?;
            let mut out = Vec::with_capacity(count * 8);
            for word in self.take(count * 8)?.as_chunks::<8>().0 {
                out.extend(u64::from_be_bytes(*word).to_le_bytes());
            }
            out
        } else {
            let count = self.count(MAX_MASK_BYTES)?;
            self.take(count)?.to_vec()
        };
        trim(&mut mask); Ok(mask)
    }
    fn arrays(&mut self, expected: usize) -> Result<Vec<Vec<u8>>, Error> {
        let count = self.count(MAX_SECTIONS + 2)?;
        if count != expected { return Err(Error::Invalid); }
        if count > self.0.len() / 2050 { return Err(Error::Truncated); }
        let mut arrays = Vec::with_capacity(count);
        for _ in 0..count {
            if self.count(2048)? != 2048 { return Err(Error::Invalid); }
            arrays.push(self.take(2048)?.to_vec());
        }
        Ok(arrays)
    }
}
fn uint(out: &mut Vec<u8>, mut value: u32) {
    while value >= 128 { out.push((value as u8 & 127) | 128); value >>= 7; }
    out.push(value as u8);
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> Light {
        Light { sky: vec![1, 2, 0, 0, 0, 0, 0, 128, 1], block: vec![0, 4],
            empty_sky: vec![2], empty_block: vec![1],
            sky_arrays: (0..4).map(|i| vec![i; 2048]).collect(), block_arrays: vec![vec![0xed; 2048]] }
    }
    #[test]
    fn converted_bytes_equal_independent_official_observations() {
        let pairs: [(&[u8], &[u8]); 2] = [
            (include_bytes!("../testdata/light-26.1.2-placed.bin"), include_bytes!("../testdata/light-26.3-placed.bin")),
            (include_bytes!("../testdata/light-26.1.2-removed.bin"), include_bytes!("../testdata/light-26.3-removed.bin")),
        ];
        for (old, new) in pairs {
            let decoded_old = Light::decode(JavaVersion::V26_1_2, old).unwrap();
            let decoded_new = Light::decode(JavaVersion::V26_3, new).unwrap();
            assert_eq!(decoded_old, decoded_new);
            assert_eq!(decoded_old.encode(JavaVersion::V26_3).unwrap(), new);
            assert_eq!(decoded_new.encode(JavaVersion::V26_1_2).unwrap(), old);
        }
    }

    #[test]
    fn masks_cross_byte_and_long_boundaries_in_both_directions() {
        let light = sample();
        for version in [JavaVersion::V26_1_2, JavaVersion::V26_2, JavaVersion::V26_3] {
            let bytes = light.encode(version).unwrap();
            assert_eq!(Light::decode(version, &bytes).unwrap(), light);
            if version != JavaVersion::V26_3 { assert_eq!(&bytes[..9], &[2, 128, 0, 0, 0, 0, 0, 2, 1]); }
            else { assert_eq!(&bytes[..10], &[9, 1, 2, 0, 0, 0, 0, 0, 128, 1]); }
            for size in [0, 1, 10, bytes.len() - 1] { assert!(Light::decode(version, &bytes[..size]).is_err()); }
            let mut trailing = bytes; trailing.push(0);
            assert_eq!(Light::decode(version, &trailing), Err(Error::Invalid));
        }
    }
    #[test]
    fn conflicting_masks_bad_arrays_and_unbounded_lengths_fail() {
        let mut light = sample(); light.empty_sky[0] = 1;
        assert_eq!(light.encode(JavaVersion::V26_3), Err(Error::Invalid));
        let mut light = sample(); light.block_arrays[0].pop();
        assert_eq!(light.encode(JavaVersion::V26_3), Err(Error::Invalid));
        let mut light = sample(); light.sky_arrays.pop();
        assert_eq!(light.encode(JavaVersion::V26_3), Err(Error::Invalid));
        assert_eq!(Light::decode(JavaVersion::V26_3, &[255,255,255,255,7]), Err(Error::TooLarge));
        assert_eq!(Light::decode(JavaVersion::V1_12_2, &[]), Err(Error::UnsupportedVersion));
    }
}
