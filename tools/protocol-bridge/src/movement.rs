//! Wiki Java Edition protocol/Packets oldid=3848030, Update Entity Position:
//! «remaining bits store the step count», fixed point с 12 дробными битами.
//! Шаги подтверждены vanilla26.3; обычные i16 delta776 — тремя MoveLook
//! и RelativeMove в ordinary-payloads-26.2.json.
use crate::JavaVersion;

const MAX_STEPS: usize = 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Step { pub tick_offset: u32, pub delta: [i16; 3] }
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Motion { Linear([i16; 3]), Stepped(Vec<Step>) }
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Delta { pub on_ground: bool, pub motion: Motion }
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error { Truncated, Invalid, TooManySteps, RequiresTimedDelivery, UnsupportedVersion }

impl Delta {
    /// Число съеденных байтов нужно пакету Position+Rotation: поворот идёт
    /// после всего списка шагов. Хвост не считается частью Delta.
    pub fn decode_prefix(version: JavaVersion, bytes: &[u8]) -> Result<(Self, usize), Error> {
        let mut cursor = Cursor { bytes, offset: 0 };
        let value = match version {
            JavaVersion::V1_21_11 | JavaVersion::V26_1_2 | JavaVersion::V26_2 => {
                let delta = cursor.delta()?;
                let on_ground = match cursor.take(1)?[0] {
                    0 => false, 1 => true, _ => return Err(Error::Invalid),
                };
                Self { on_ground, motion: Motion::Linear(delta) }
            }
            JavaVersion::V26_3 => {
                let properties = cursor.varint()?;
                let count = (properties >> 1) as usize;
                if count > MAX_STEPS { return Err(Error::TooManySteps); }
                let motion = if count == 0 { Motion::Linear(cursor.delta()?) }
                else {
                    // Минимум: VarInt смещения и три Short на каждый шаг.
                    if bytes.len().saturating_sub(cursor.offset) < count * 7 { return Err(Error::Truncated); }
                    let mut steps = Vec::with_capacity(count);
                    for _ in 0..count {
                        steps.push(Step { tick_offset: cursor.varint()?, delta: cursor.delta()? });
                    }
                    Motion::Stepped(steps)
                };
                Self { on_ground: properties & 1 != 0, motion }
            }
            JavaVersion::V1_12_2 => return Err(Error::UnsupportedVersion),
        };
        Ok((value, cursor.offset))
    }

    pub fn encode(&self, version: JavaVersion) -> Result<Vec<u8>, Error> {
        let mut out = Vec::new();
        match version {
            JavaVersion::V1_21_11 | JavaVersion::V26_1_2 | JavaVersion::V26_2 => {
                let Motion::Linear(delta) = &self.motion else { return Err(Error::RequiresTimedDelivery); };
                push_delta(&mut out, delta);
                out.push(u8::from(self.on_ground));
            }
            JavaVersion::V26_3 => match &self.motion {
                Motion::Linear(delta) => {
                    varint(&mut out, u32::from(self.on_ground));
                    push_delta(&mut out, delta);
                }
                Motion::Stepped(steps) => {
                    if steps.is_empty() { return Err(Error::Invalid); }
                    if steps.len() > MAX_STEPS { return Err(Error::TooManySteps); }
                    varint(&mut out, ((steps.len() as u32) << 1) | u32::from(self.on_ground));
                    for step in steps {
                        if step.tick_offset > i32::MAX as u32 { return Err(Error::Invalid); }
                        varint(&mut out, step.tick_offset);
                        push_delta(&mut out, &step.delta);
                    }
                }
            },
            JavaVersion::V1_12_2 => return Err(Error::UnsupportedVersion),
        }
        Ok(out)
    }
}

struct Cursor<'a> { bytes: &'a [u8], offset: usize }
impl<'a> Cursor<'a> {
    fn take(&mut self, size: usize) -> Result<&'a [u8], Error> {
        let value = self.bytes.get(self.offset..self.offset + size).ok_or(Error::Truncated)?;
        self.offset += size;
        Ok(value)
    }
    fn delta(&mut self) -> Result<[i16; 3], Error> {
        let bytes = self.take(6)?;
        Ok([i16::from_be_bytes([bytes[0], bytes[1]]), i16::from_be_bytes([bytes[2], bytes[3]]),
            i16::from_be_bytes([bytes[4], bytes[5]])])
    }
    fn varint(&mut self) -> Result<u32, Error> {
        let mut value = 0;
        for shift in (0..35).step_by(7) {
            let byte = self.take(1)?[0];
            if shift == 28 && byte & 0xf8 != 0 { return Err(Error::Invalid); }
            value |= u32::from(byte & 127) << shift;
            if byte < 128 { return Ok(value); }
        }
        Err(Error::Invalid)
    }
}
fn varint(out: &mut Vec<u8>, mut value: u32) {
    while value >= 128 { out.push((value as u8 & 127) | 128); value >>= 7; }
    out.push(value as u8);
}
fn push_delta(out: &mut Vec<u8>, delta: &[i16; 3]) {
    for value in delta { out.extend(value.to_be_bytes()); }
}

#[cfg(test)]
mod tests {
    use super::*;
    use JavaVersion::{V26_1_2, V26_2, V26_3};

    #[test]
    fn actual_vanilla_steps_reencode_without_losing_timing() {
        let bytes = include_bytes!("../testdata/entity-delta-26.3.bin");
        let mut offset = 0;
        let mut sum = [0i32; 3];
        let mut samples = 0;
        while offset < bytes.len() {
            let (value, used) = Delta::decode_prefix(V26_3, &bytes[offset..]).unwrap();
            assert!(!value.on_ground);
            let Motion::Stepped(ref steps) = value.motion else { panic!("vanilla supplied steps"); };
            assert_eq!(steps.len(), 1);
            assert!((1..=3).contains(&steps[0].tick_offset));
            for (sum, delta) in sum.iter_mut().zip(steps[0].delta) { *sum += i32::from(delta); }
            assert_eq!(value.encode(V26_3).unwrap(), bytes[offset..offset + used]);
            // Сведение шагов в один пакет потеряло бы сроки их выполнения.
            assert_eq!(value.encode(V26_1_2), Err(Error::RequiresTimedDelivery));
            assert_eq!(value.encode(V26_2), Err(Error::RequiresTimedDelivery));
            offset += used;
            samples += 1;
        }
        assert_eq!(samples, 6);
        assert_eq!(sum, [-2048, 0, -2048]); // от центра 2.5/2.5 до 2.0/2.0
    }

    #[test]
    fn linear_delta_converts_both_directions_and_keeps_rotation_tail() {
        for ground in [false, true] {
            let value = Delta { on_ground: ground, motion: Motion::Linear([i16::MIN, 0, i16::MAX]) };
            for version in [V26_1_2, V26_2, V26_3] {
                let mut bytes = value.encode(version).unwrap();
                bytes.extend([42, 255]);
                let (decoded, used) = Delta::decode_prefix(version, &bytes).unwrap();
                assert_eq!(decoded, value);
                assert_eq!(&bytes[used..], &[42, 255]);
                for length in 0..used { assert!(Delta::decode_prefix(version, &bytes[..length]).is_err()); }
            }
        }
    }

    #[test]
    fn steps_have_bounded_lengths_and_preserve_offsets() {
        let value = Delta { on_ground: true, motion: Motion::Stepped(vec![
            Step { tick_offset: 1, delta: [1, 2, 3] }, Step { tick_offset: 130, delta: [-4, -5, -6] },
        ]) };
        let bytes = value.encode(V26_3).unwrap();
        assert_eq!(Delta::decode_prefix(V26_3, &bytes).unwrap(), (value, bytes.len()));
        assert_eq!(Delta::decode_prefix(V26_3, &[0x80, 0x10]), Err(Error::Truncated));
        assert_eq!(Delta::decode_prefix(V26_3, &[0x82, 0x10]), Err(Error::TooManySteps));
        assert_eq!(Delta::decode_prefix(V26_3, &[2]), Err(Error::Truncated));
        assert!(Delta::decode_prefix(V26_3, &[255; 5]).is_err());
        assert_eq!(Delta { on_ground: false, motion: Motion::Stepped(vec![]) }.encode(V26_3), Err(Error::Invalid));
    }
}
