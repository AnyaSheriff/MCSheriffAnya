// Кодек Bedrock: числа, строки и NBT так, как их пишет протокол Bedrock, и
// пакеты-обёртки (batch) со сжатием. Форматы — по описанию протокола в
// minecraft-data (data/bedrock/1.26.10/proto.yml, types.yml).

use std::io::{Read, Write};

/// Писатель тела пакета.
#[derive(Default)]
pub struct Out {
    pub bytes: Vec<u8>,
}

impl Out {
    /// Новый пакет с этим номером.
    pub fn packet(id: u32) -> Out {
        let mut out = Out::default();
        out.varint(id);
        out
    }

    pub fn u8(&mut self, value: u8) -> &mut Self {
        self.bytes.push(value);
        self
    }

    pub fn bool(&mut self, value: bool) -> &mut Self {
        self.u8(value as u8)
    }

    pub fn varint(&mut self, mut value: u32) -> &mut Self {
        loop {
            if value < 0x80 {
                self.bytes.push(value as u8);
                return self;
            }

            self.bytes.push((value as u8 & 0x7f) | 0x80);
            value >>= 7;
        }
    }

    pub fn varint64(&mut self, mut value: u64) -> &mut Self {
        loop {
            if value < 0x80 {
                self.bytes.push(value as u8);
                return self;
            }

            self.bytes.push((value as u8 & 0x7f) | 0x80);
            value >>= 7;
        }
    }

    pub fn zigzag32(&mut self, value: i32) -> &mut Self {
        self.varint(((value << 1) ^ (value >> 31)) as u32)
    }

    pub fn zigzag64(&mut self, value: i64) -> &mut Self {
        self.varint64(((value << 1) ^ (value >> 63)) as u64)
    }

    pub fn lu16(&mut self, value: u16) -> &mut Self {
        self.bytes.extend_from_slice(&value.to_le_bytes());
        self
    }

    pub fn li16(&mut self, value: i16) -> &mut Self {
        self.bytes.extend_from_slice(&value.to_le_bytes());
        self
    }

    pub fn li32(&mut self, value: i32) -> &mut Self {
        self.bytes.extend_from_slice(&value.to_le_bytes());
        self
    }

    pub fn lu32(&mut self, value: u32) -> &mut Self {
        self.bytes.extend_from_slice(&value.to_le_bytes());
        self
    }

    pub fn i32_be(&mut self, value: i32) -> &mut Self {
        self.bytes.extend_from_slice(&value.to_be_bytes());
        self
    }

    pub fn li64(&mut self, value: i64) -> &mut Self {
        self.bytes.extend_from_slice(&value.to_le_bytes());
        self
    }

    pub fn lu64(&mut self, value: u64) -> &mut Self {
        self.bytes.extend_from_slice(&value.to_le_bytes());
        self
    }

    pub fn lf32(&mut self, value: f32) -> &mut Self {
        self.bytes.extend_from_slice(&value.to_le_bytes());
        self
    }

    pub fn string(&mut self, value: &str) -> &mut Self {
        self.varint(value.len() as u32);
        self.bytes.extend_from_slice(value.as_bytes());
        self
    }

    /// UUID: старшая половина, потом младшая — каждая little-endian.
    pub fn uuid(&mut self, value: [u8; 16]) -> &mut Self {
        let high = u64::from_be_bytes(value[..8].try_into().expect("8 байт"));
        let low = u64::from_be_bytes(value[8..].try_into().expect("8 байт"));
        self.lu64(high).lu64(low)
    }

    pub fn vec3f(&mut self, x: f32, y: f32, z: f32) -> &mut Self {
        self.lf32(x).lf32(y).lf32(z)
    }

    pub fn block_position(&mut self, x: i32, y: i32, z: i32) -> &mut Self {
        self.zigzag32(x).zigzag32(y).zigzag32(z)
    }

    /// Пустой NBT-компаунд в сетевом виде.
    pub fn empty_nbt(&mut self) -> &mut Self {
        self.u8(10).varint(0).u8(0)
    }

    pub fn raw(&mut self, bytes: &[u8]) -> &mut Self {
        self.bytes.extend_from_slice(bytes);
        self
    }
}

/// Читатель тела пакета.
pub struct In<'a> {
    data: &'a [u8],
    at: usize,
}

impl<'a> In<'a> {
    pub fn new(data: &'a [u8]) -> In<'a> {
        In { data, at: 0 }
    }

    pub fn take(&mut self, count: usize) -> Option<&'a [u8]> {
        let slice = self.data.get(self.at..self.at.checked_add(count)?)?;
        self.at += count;
        Some(slice)
    }

    pub fn u8(&mut self) -> Option<u8> {
        self.take(1).map(|b| b[0])
    }

    pub fn bool(&mut self) -> Option<bool> {
        self.u8().map(|b| b != 0)
    }

    pub fn varint(&mut self) -> Option<u32> {
        let mut value: u32 = 0;

        for shift in (0..35).step_by(7) {
            let byte = self.u8()?;
            value |= ((byte & 0x7f) as u32) << shift;

            if byte & 0x80 == 0 {
                return Some(value);
            }
        }

        None
    }

    pub fn varint64(&mut self) -> Option<u64> {
        let mut value: u64 = 0;

        for shift in (0..70).step_by(7) {
            let byte = self.u8()?;
            value |= ((byte & 0x7f) as u64) << shift;

            if byte & 0x80 == 0 {
                return Some(value);
            }
        }

        None
    }

    pub fn zigzag32(&mut self) -> Option<i32> {
        let raw = self.varint()?;
        Some(((raw >> 1) as i32) ^ -((raw & 1) as i32))
    }

    #[cfg(test)]
    pub fn zigzag64(&mut self) -> Option<i64> {
        let raw = self.varint64()?;
        Some(((raw >> 1) as i64) ^ -((raw & 1) as i64))
    }

    pub fn i32_be(&mut self) -> Option<i32> {
        self.take(4).map(|b| i32::from_be_bytes(b.try_into().expect("4 байта")))
    }

    pub fn li32(&mut self) -> Option<i32> {
        self.take(4).map(|b| i32::from_le_bytes(b.try_into().expect("4 байта")))
    }

    pub fn lf32(&mut self) -> Option<f32> {
        self.take(4).map(|b| f32::from_le_bytes(b.try_into().expect("4 байта")))
    }

    pub fn string(&mut self) -> Option<String> {
        let length = self.varint()? as usize;
        self.take(length).map(|b| String::from_utf8_lossy(b).into_owned())
    }

    pub fn little_string(&mut self) -> Option<String> {
        let length = self.li32()?;

        if length < 0 {
            return None;
        }

        self.take(length as usize).map(|b| String::from_utf8_lossy(b).into_owned())
    }
}

/// Алгоритм сжатия, объявленный в Network Settings.
pub const DEFLATE: u8 = 0;

/// «Без сжатия» в заголовке пакета-обёртки.
pub const NO_COMPRESSION: u8 = 0xff;

/// Разбирает тело пакета-обёртки (то, что шло после 0xFE) на игровые
/// пакеты. `compressed` — включено ли уже сжатие (после Network Settings у
/// обёртки есть байт алгоритма).
pub fn unbatch(body: &[u8], compressed: bool) -> Option<Vec<Vec<u8>>> {
    let payload = if compressed {
        let (&algorithm, rest) = body.split_first()?;

        match algorithm {
            DEFLATE => {
                let mut out = Vec::new();
                flate2::read::DeflateDecoder::new(rest).take(64 * 1024 * 1024).read_to_end(&mut out).ok()?;
                out
            }
            NO_COMPRESSION => rest.to_vec(),
            _ => return None,
        }
    } else {
        body.to_vec()
    };

    let mut reader = In::new(&payload);
    let mut packets = Vec::new();

    while reader.at < payload.len() {
        let length = reader.varint()? as usize;
        packets.push(reader.take(length)?.to_vec());
    }

    Some(packets)
}

/// Собирает пакеты в тело обёртки. Маленькие — без сжатия.
pub fn batch(packets: &[Vec<u8>], compressed: bool, threshold: usize) -> Vec<u8> {
    let mut payload = Out::default();

    for packet in packets {
        payload.varint(packet.len() as u32);
        payload.raw(packet);
    }

    if !compressed {
        return payload.bytes;
    }

    if payload.bytes.len() < threshold {
        let mut out = vec![NO_COMPRESSION];
        out.extend_from_slice(&payload.bytes);
        return out;
    }

    let mut out = vec![DEFLATE];
    let mut encoder = flate2::write::DeflateEncoder::new(&mut out, flate2::Compression::fast());
    let _ = encoder.write_all(&payload.bytes);
    let _ = encoder.finish();
    out
}

/// Номер пакета из его заголовка (младшие 10 бит).
pub fn packet_id(packet: &[u8]) -> Option<(u32, &[u8])> {
    let mut reader = In::new(packet);
    let header = reader.varint()?;
    let at = reader.at;

    Some((header & 0x3ff, &packet[at..]))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Обёртка со сжатием и без читается обратно в те же пакеты.
    #[test]
    fn batches_round_trip() {
        let packets = vec![vec![1, 2, 3], vec![9; 3000]];

        for (compressed, threshold) in [(false, 0), (true, 1), (true, 100_000)] {
            let body = batch(&packets, compressed, threshold);
            assert_eq!(unbatch(&body, compressed), Some(packets.clone()));
        }
    }

    /// Числа переменной длины и зигзаг читаются обратно.
    #[test]
    fn numbers_round_trip() {
        let mut out = Out::default();
        out.varint(300).zigzag32(-5).zigzag64(-1_000_000_000_000).varint64(u64::MAX);
        let mut reader = In::new(&out.bytes);

        assert_eq!(reader.varint(), Some(300));
        assert_eq!(reader.zigzag32(), Some(-5));
        assert_eq!(reader.zigzag64(), Some(-1_000_000_000_000));
        assert_eq!(reader.varint64(), Some(u64::MAX));
    }
}
