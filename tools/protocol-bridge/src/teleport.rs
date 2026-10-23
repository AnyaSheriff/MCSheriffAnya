//! Confirm Teleportation: Wiki oldid3848030, «resulting position and rotation».
//! 777 добавляет координаты и углы к номеру. Не теряем эти поля при переводе.
//! Короткий ACK776: закреплённый JSON и принятие собственного probe сервером;
//! [проверить] исходящий ACK настоящего GUI-клиента776 ещё не записан.
use crate::JavaVersion;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    pub yaw: f32,
    pub pitch: f32,
}

impl Pose {
    fn finite(self) -> bool {
        [self.x, self.y, self.z].into_iter().all(f64::is_finite)
            && self.yaw.is_finite() && self.pitch.is_finite()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Confirmation {
    pub id: i32,
    pub pose: Option<Pose>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error { UnsupportedVersion, Truncated, Invalid, MissingPose, UnexpectedPose }

impl Confirmation {
    pub fn decode(version: JavaVersion, payload: &[u8]) -> Result<Self, Error> {
        if !matches!(version, JavaVersion::V1_21_5 | JavaVersion::V1_21_6 | JavaVersion::V1_21_8 | JavaVersion::V1_21_9 | JavaVersion::V1_21_11 | JavaVersion::V26_1_2 | JavaVersion::V26_2 | JavaVersion::V26_3) { return Err(Error::UnsupportedVersion); }
        let mut rest = payload;
        let mut value = 0u32;
        let mut complete = false;
        for shift in (0..35).step_by(7) {
            let (&byte, tail) = rest.split_first().ok_or(Error::Truncated)?;
            rest = tail;
            if shift == 28 && byte & 0xf0 != 0 { return Err(Error::Invalid); }
            value |= u32::from(byte & 127) << shift;
            if byte < 128 { complete = true; break; }
        }
        if !complete { return Err(Error::Invalid); }
        let pose = if version == JavaVersion::V26_3 {
            if rest.len() < 32 { return Err(Error::Truncated); }
            if rest.len() != 32 { return Err(Error::Invalid); }
            let pose = Pose {
                x: f64::from_be_bytes(rest[..8].try_into().map_err(|_| Error::Invalid)?),
                y: f64::from_be_bytes(rest[8..16].try_into().map_err(|_| Error::Invalid)?),
                z: f64::from_be_bytes(rest[16..24].try_into().map_err(|_| Error::Invalid)?),
                yaw: f32::from_be_bytes(rest[24..28].try_into().map_err(|_| Error::Invalid)?),
                pitch: f32::from_be_bytes(rest[28..32].try_into().map_err(|_| Error::Invalid)?),
            };
            if !pose.finite() { return Err(Error::Invalid); }
            Some(pose)
        } else {
            if !rest.is_empty() { return Err(Error::Invalid); }
            None
        };
        Ok(Self { id: value as i32, pose })
    }

    pub fn encode(self, version: JavaVersion) -> Result<Vec<u8>, Error> {
        match (version, self.pose) {
            (JavaVersion::V1_12_2, _) => return Err(Error::UnsupportedVersion),
            (JavaVersion::V26_3, None) => return Err(Error::MissingPose),
            (JavaVersion::V1_21_5 | JavaVersion::V1_21_6 | JavaVersion::V1_21_8 | JavaVersion::V1_21_9 | JavaVersion::V1_21_11 | JavaVersion::V26_1_2 | JavaVersion::V26_2, Some(_)) => return Err(Error::UnexpectedPose),
            _ => {}
        }
        let mut out = Vec::with_capacity(37);
        let mut id = self.id as u32;
        while id >= 128 { out.push((id as u8 & 127) | 128); id >>= 7; }
        out.push(id as u8);
        if let Some(pose) = self.pose {
            if !pose.finite() { return Err(Error::Invalid); }
            for value in [pose.x, pose.y, pose.z] { out.extend(value.to_be_bytes()); }
            for value in [pose.yaw, pose.pitch] { out.extend(value.to_be_bytes()); }
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use JavaVersion::{V26_1_2 as OLD, V26_3 as NEW};

    #[test]
    fn actual_confirmation_has_the_complete_resulting_pose() {
        let bytes = include_bytes!("../testdata/teleport-confirm-26.3.bin");
        let value = Confirmation::decode(NEW, bytes).unwrap();
        assert_eq!(value.id, 1);
        assert_eq!(value.pose, Some(Pose { x: -9.5, y: -60.0, z: -1.5, yaw: 0.0, pitch: 0.0 }));
        assert_eq!(value.encode(NEW).unwrap(), bytes);
    }

    #[test]
    fn signed_ids_and_all_pose_fields_survive_without_implicit_conversion() {
        for id in [0, 1, 127, 128, i32::MAX, i32::MIN, -1] {
            let mut value = Confirmation { id, pose: None };
            assert_eq!(Confirmation::decode(OLD, &value.encode(OLD).unwrap()), Ok(value));
            assert_eq!(value.encode(NEW), Err(Error::MissingPose));
            value.pose = Some(Pose { x: -1.25, y: -64.0, z: 1234.5, yaw: 370.0, pitch: -20.0 });
            assert_eq!(Confirmation::decode(NEW, &value.encode(NEW).unwrap()), Ok(value));
            assert_eq!(value.encode(OLD), Err(Error::UnexpectedPose));
        }
    }

    #[test]
    fn truncated_nonfinite_trailing_and_overflow_payloads_are_rejected() {
        let value = Confirmation { id: 128, pose: Some(Pose { x: 1.0, y: 64.0, z: -1.0, yaw: 45.0, pitch: 20.0 }) };
        let bytes = value.encode(NEW).unwrap();
        for len in 0..bytes.len() { assert!(Confirmation::decode(NEW, &bytes[..len]).is_err()); }
        let mut extra = bytes.clone(); extra.push(0);
        assert_eq!(Confirmation::decode(NEW, &extra), Err(Error::Invalid));
        for offset in [2, 10, 18, 26, 30] {
            let mut broken = bytes.clone();
            if offset < 26 { broken[offset..offset + 8].copy_from_slice(&f64::INFINITY.to_be_bytes()); }
            else { broken[offset..offset + 4].copy_from_slice(&f32::NAN.to_be_bytes()); }
            assert_eq!(Confirmation::decode(NEW, &broken), Err(Error::Invalid));
        }
        for bytes in [&[255, 255, 255, 255, 31][..], &[128, 128, 128, 128, 128, 0], &[1, 0]] {
            assert!(Confirmation::decode(OLD, bytes).is_err());
        }
        assert_eq!(Confirmation::decode(JavaVersion::V1_12_2, &[0]), Err(Error::UnsupportedVersion));
    }
}
