//! Тело clientbound Player Position767/768, без ID пакета и без открытия маршрута.
//! Источник: client-chain-sources.json, pinned protocol.json: «packet_position»
//! и «PositionUpdateRelatives». JSON задаёт раскладку, не правила изменения скорости.
//! [проверить] Игровая эквивалентность требует контекста движения и живых записей.
use crate::{teleport::Pose, wire::{Cursor, Error, uint}};

const POSE_FLAGS: u8 = 0x1f;
const MOTION_FLAGS: u32 = 0x1e0;
const ALL_FLAGS: u32 = 0x1ff;
const MAX_767: usize = 38;
const MAX_768: usize = 65;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Position767 {
    pub id: i32,
    pub pose: Pose,
    pub relative: u8,
}

/// Дополнение768 приходит от вызывающего контекста, а не выдумывается кодеком.
/// Биты5..8: dx,dy,dz,yawDelta; младшие пять бит принадлежат Position767.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionExtension {
    pub delta: [f64; 3],
    pub relative: u32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Position768 {
    pub position: Position767,
    pub motion: MotionExtension,
}

fn double(input: &mut Cursor<'_>) -> Result<f64, Error> {
    Ok(f64::from_be_bytes(input.take(8)?.try_into().map_err(|_| Error::Invalid)?))
}

fn float(input: &mut Cursor<'_>) -> Result<f32, Error> {
    Ok(f32::from_be_bytes(input.take(4)?.try_into().map_err(|_| Error::Invalid)?))
}

fn id(input: &mut Cursor<'_>) -> Result<i32, Error> {
    let before = input.remaining();
    let value = input.varint()?;
    // Не принимаем несколько байтовых представлений одного номера.
    let mut canonical = Vec::with_capacity(5);
    uint(&mut canonical, value as u32);
    if before - input.remaining() != canonical.len() { return Err(Error::Invalid); }
    Ok(value)
}

fn pose(input: &mut Cursor<'_>) -> Result<Pose, Error> {
    Ok(Pose { x: double(input)?, y: double(input)?, z: double(input)?,
        yaw: float(input)?, pitch: float(input)? })
}

fn put_pose(out: &mut Vec<u8>, pose: Pose) {
    for value in [pose.x, pose.y, pose.z] { out.extend(value.to_be_bytes()); }
    for value in [pose.yaw, pose.pitch] { out.extend(value.to_be_bytes()); }
}

impl Position767 {
    fn validate(self) -> Result<(), Error> {
        if self.relative & !POSE_FLAGS != 0
            || ![self.pose.x, self.pose.y, self.pose.z].into_iter().all(f64::is_finite)
            || !self.pose.yaw.is_finite() || !self.pose.pitch.is_finite() {
            return Err(Error::Invalid);
        }
        Ok(())
    }

    pub fn decode(payload: &[u8]) -> Result<Self, Error> {
        if payload.len() > MAX_767 { return Err(Error::TooLarge); }
        let mut input = Cursor::new(payload);
        let pose = pose(&mut input)?;
        let relative = input.take(1)?[0];
        let value = Self { id: id(&mut input)?, pose, relative };
        if input.remaining() != 0 { return Err(Error::Invalid); }
        value.validate()?;
        Ok(value)
    }

    pub fn encode(self) -> Result<Vec<u8>, Error> {
        self.validate()?;
        let mut out = Vec::with_capacity(MAX_767);
        put_pose(&mut out, self.pose);
        out.push(self.relative);
        uint(&mut out, self.id as u32);
        Ok(out)
    }

    /// Только добавление явно заданных полей; сохранение скорости не угадывается.
    pub fn with_motion(self, motion: MotionExtension) -> Result<Position768, Error> {
        self.validate()?;
        motion.validate()?;
        Ok(Position768 { position: self, motion })
    }
}

impl MotionExtension {
    fn validate(self) -> Result<(), Error> {
        if self.relative & !MOTION_FLAGS != 0 || !self.delta.into_iter().all(f64::is_finite) {
            return Err(Error::Invalid);
        }
        Ok(())
    }
}

impl Position768 {
    pub fn decode(payload: &[u8]) -> Result<Self, Error> {
        if payload.len() > MAX_768 { return Err(Error::TooLarge); }
        let mut input = Cursor::new(payload);
        let id = id(&mut input)?;
        let x = double(&mut input)?;
        let y = double(&mut input)?;
        let z = double(&mut input)?;
        let delta = [double(&mut input)?, double(&mut input)?, double(&mut input)?];
        let pose = Pose { x, y, z, yaw: float(&mut input)?, pitch: float(&mut input)? };
        let flags = u32::from_be_bytes(input.take(4)?.try_into().map_err(|_| Error::Invalid)?);
        if flags & !ALL_FLAGS != 0 || input.remaining() != 0 { return Err(Error::Invalid); }
        Position767 { id, pose, relative: (flags & u32::from(POSE_FLAGS)) as u8 }
            .with_motion(MotionExtension { delta, relative: flags & MOTION_FLAGS })
    }

    pub fn encode(self) -> Result<Vec<u8>, Error> {
        self.position.validate()?;
        self.motion.validate()?;
        let mut out = Vec::with_capacity(MAX_768);
        uint(&mut out, self.position.id as u32);
        for value in [self.position.pose.x, self.position.pose.y, self.position.pose.z] {
            out.extend(value.to_be_bytes());
        }
        for value in self.motion.delta { out.extend(value.to_be_bytes()); }
        for value in [self.position.pose.yaw, self.position.pose.pitch] { out.extend(value.to_be_bytes()); }
        out.extend((u32::from(self.position.relative) | self.motion.relative).to_be_bytes());
        Ok(out)
    }

    /// Проверка только структурной представимости, не игрового действия.
    /// Ненулевое движение/новые относительные биты нельзя молча потерять.
    /// Даже знак нулевого дополнения сохраняется отказом. До связывания маршрута
    /// [проверить] эффект нулевого дополнения на старую скорость отдельно.
    pub fn try_without_motion(self) -> Result<Position767, Error> {
        self.position.validate()?;
        self.motion.validate()?;
        if self.motion.relative != 0 || self.motion.delta.into_iter().any(|v| v.to_bits() != 0) {
            return Err(Error::PayloadUnavailable("767 player position motion requires context"));
        }
        Ok(self.position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn position() -> Position767 {
        Position767 { id: 300, pose: Pose { x: 1.25, y: -64.0, z: -2.5,
            yaw: 450.0, pitch: -30.0 }, relative: 0x15 }
    }

    fn extension() -> MotionExtension {
        MotionExtension { delta: [0.125, -0.25, 1.5], relative: 0x160 }
    }

    fn bytes(hex: &str) -> Vec<u8> {
        hex.as_bytes().as_chunks::<2>().0.iter().map(|part|
            u8::from_str_radix(std::str::from_utf8(part).unwrap(), 16).unwrap()).collect()
    }

    #[test]
    fn independent_big_endian_fixtures_keep_different_layouts() {
        // Синтетические значения упакованы Python struct по pinned JSON;
        // это проверка раскладки, не запись игрового оригинала.
        let old = bytes("3ff4000000000000c050000000000000c00400000000000043e10000c1f0000015ac02");
        let new = bytes("ac023ff4000000000000c050000000000000c0040000000000003fc0000000000000bfd00000000000003ff800000000000043e10000c1f0000000000175");
        assert_eq!(Position767::decode(&old).unwrap(), position());
        assert_eq!(position().encode().unwrap(), old);
        let value = position().with_motion(extension()).unwrap();
        assert_eq!(Position768::decode(&new).unwrap(), value);
        assert_eq!(value.encode().unwrap(), new);
    }

    #[test]
    fn signed_ids_and_each_known_flag_roundtrip_without_normalizing_floats() {
        for id in [0, 1, 127, 128, i32::MAX, i32::MIN, -1] {
            for flags in 0..=ALL_FLAGS {
                let mut old = position();
                old.id = id;
                old.relative = (flags & 31) as u8;
                old.pose.x = -0.0;
                old.pose.pitch = -0.0;
                let old_bytes = old.encode().unwrap();
                assert_eq!(Position767::decode(&old_bytes).unwrap().encode().unwrap(), old_bytes);
                let new = old.with_motion(MotionExtension { relative: flags & MOTION_FLAGS,
                    delta: [-0.0, f64::MIN_POSITIVE, -1.0] }).unwrap();
                let new_bytes = new.encode().unwrap();
                assert_eq!(Position768::decode(&new_bytes).unwrap().encode().unwrap(), new_bytes);
            }
        }
    }

    #[test]
    fn unknown_flags_truncated_oversized_and_trailing_bodies_are_rejected() {
        let old = position().encode().unwrap();
        let new = position().with_motion(extension()).unwrap().encode().unwrap();
        for length in 0..old.len() { assert!(Position767::decode(&old[..length]).is_err()); }
        for length in 0..new.len() { assert!(Position768::decode(&new[..length]).is_err()); }
        let mut tail = old.clone(); tail.push(0);
        assert!(Position767::decode(&tail).is_err());
        let mut tail = new.clone(); tail.push(0);
        assert!(Position768::decode(&tail).is_err());
        assert_eq!(Position767::decode(&[0; MAX_767 + 1]), Err(Error::TooLarge));
        assert_eq!(Position768::decode(&[0; MAX_768 + 1]), Err(Error::TooLarge));
        let mut bad = old; bad[32] = 0x20;
        assert_eq!(Position767::decode(&bad), Err(Error::Invalid));
        let mut bad = new; let start = bad.len() - 4;
        bad[start..].copy_from_slice(&0x200u32.to_be_bytes());
        assert_eq!(Position768::decode(&bad), Err(Error::Invalid));
        let mut bad = position(); bad.relative = 0x80;
        assert_eq!(bad.encode(), Err(Error::Invalid));
        for relative in [1, 0x200, u32::MAX] {
            assert_eq!(position().with_motion(MotionExtension { relative, ..extension() }), Err(Error::Invalid));
        }
    }

    #[test]
    fn malformed_noncanonical_and_overflow_varints_are_rejected_in_both_positions() {
        let old = position().encode().unwrap();
        let new = position().with_motion(extension()).unwrap().encode().unwrap();
        for bad in [vec![0x80], vec![0x80, 0], vec![0xff, 0xff, 0xff, 0xff, 0x10],
            vec![0x80, 0x80, 0x80, 0x80, 0x80, 0]] {
            let mut old_body = old[..33].to_vec(); old_body.extend(&bad);
            assert!(Position767::decode(&old_body).is_err());
            let mut new_body = bad; new_body.extend(&new[2..]);
            assert!(Position768::decode(&new_body).is_err());
        }
    }

    #[test]
    fn every_nonfinite_position_angle_and_motion_component_is_rejected() {
        let old = position().encode().unwrap();
        let new = position().with_motion(extension()).unwrap().encode().unwrap();
        for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            for offset in [0, 8, 16] {
                let mut bad = old.clone(); bad[offset..offset+8].copy_from_slice(&value.to_be_bytes());
                assert_eq!(Position767::decode(&bad), Err(Error::Invalid));
            }
            for offset in [2, 10, 18, 26, 34, 42] {
                let mut bad = new.clone(); bad[offset..offset+8].copy_from_slice(&value.to_be_bytes());
                assert_eq!(Position768::decode(&bad), Err(Error::Invalid));
            }
            for axis in 0..3 {
                let mut motion = extension(); motion.delta[axis] = value;
                assert_eq!(position().with_motion(motion), Err(Error::Invalid));
                assert_eq!(Position768 { position: position(), motion }.encode(), Err(Error::Invalid));
                let mut pose = position();
                match axis { 0 => pose.pose.x = value, 1 => pose.pose.y = value, _ => pose.pose.z = value }
                assert_eq!(pose.encode(), Err(Error::Invalid));
            }
        }
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            for offset in [24, 28] {
                let mut bad = old.clone(); bad[offset..offset+4].copy_from_slice(&value.to_be_bytes());
                assert_eq!(Position767::decode(&bad), Err(Error::Invalid));
            }
            for offset in [50, 54] {
                let mut bad = new.clone(); bad[offset..offset+4].copy_from_slice(&value.to_be_bytes());
                assert_eq!(Position768::decode(&bad), Err(Error::Invalid));
            }
            for pitch in [false, true] {
                let mut bad = position();
                if pitch { bad.pose.pitch = value; } else { bad.pose.yaw = value; }
                assert_eq!(bad.encode(), Err(Error::Invalid));
            }
        }
    }

    #[test]
    fn explicit_motion_is_preserved_and_unrepresentable_downgrade_refuses() {
        let old = position();
        let neutral = MotionExtension { delta: [0.0; 3], relative: 0 };
        assert_eq!(old.with_motion(neutral).unwrap().try_without_motion().unwrap(), old);
        for motion in [extension(), MotionExtension { delta: [0.0; 3], relative: 0x100 },
            MotionExtension { delta: [-0.0, 0.0, 0.0], relative: 0 },
            MotionExtension { delta: [0.0, 1.0, 0.0], relative: 0 }] {
            let value = old.with_motion(motion).unwrap();
            assert_eq!(Position768::decode(&value.encode().unwrap()).unwrap().motion, motion);
            assert_eq!(value.try_without_motion(), Err(Error::PayloadUnavailable("767 player position motion requires context")));
        }
    }
}
