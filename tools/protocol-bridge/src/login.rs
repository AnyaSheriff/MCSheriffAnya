//! Login Success 775/776/777: разрешённые protocol.json и наблюдения vanilla.
//! В 776/777 после свойств профиля передаётся UUID sessionId. Это не UUID игрока.
use crate::JavaVersion;

const MAX_PACKET: usize = 2_097_151;
const MAX_STRING: usize = 32_767;
const MAX_PROPERTIES: usize = 1024;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Property {
    pub name: String,
    pub value: String,
    pub signature: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Success {
    pub uuid: [u8; 16],
    pub username: String,
    pub properties: Vec<Property>,
    pub session_id: Option<[u8; 16]>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error { UnsupportedVersion, Truncated, Invalid, TooLarge, MissingSessionId, UnexpectedSessionId }

fn supported(version: JavaVersion) -> Result<(), Error> {
    if !matches!(version, JavaVersion::V1_21_6 | JavaVersion::V1_21_8 | JavaVersion::V1_21_9 | JavaVersion::V1_21_11 | JavaVersion::V26_1_2 | JavaVersion::V26_2 | JavaVersion::V26_3) { Err(Error::UnsupportedVersion) } else { Ok(()) }
}

fn valid_name(name: &str) -> bool {
    (1..=16).contains(&name.len()) && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

impl Success {
    /// Разбирает полезную нагрузку, без ID пакета и внешнего кадра.
    pub fn decode(version: JavaVersion, payload: &[u8]) -> Result<Self, Error> {
        supported(version)?;
        if payload.len() > MAX_PACKET { return Err(Error::TooLarge); }
        let mut cursor = Cursor(payload);
        let uuid = cursor.take(16)?.try_into().map_err(|_| Error::Truncated)?;
        let username = cursor.string()?;
        if !valid_name(&username) { return Err(Error::Invalid); }
        let count = cursor.count(MAX_PROPERTIES)?;
        let mut properties = Vec::with_capacity(count);
        for _ in 0..count {
            let name = cursor.string()?;
            let value = cursor.string()?;
            let signature = match cursor.take(1)?[0] {
                0 => None,
                1 => Some(cursor.string()?),
                _ => return Err(Error::Invalid),
            };
            properties.push(Property { name, value, signature });
        }
        let session_id = if matches!(version, JavaVersion::V26_2 | JavaVersion::V26_3) {
            Some(cursor.take(16)?.try_into().map_err(|_| Error::Truncated)?)
        } else { None };
        if !cursor.0.is_empty() { return Err(Error::Invalid); }
        Ok(Self { uuid, username, properties, session_id })
    }

    /// Для 776/777 sessionId обязан выдать вызывающий сервер. Не подменяем его
    /// UUID игрока или нулями при отсутствии настоящей сессии.
    pub fn encode(&self, version: JavaVersion) -> Result<Vec<u8>, Error> {
        supported(version)?;
        if !valid_name(&self.username) { return Err(Error::Invalid); }
        if self.properties.len() > MAX_PROPERTIES { return Err(Error::TooLarge); }
        match (version, self.session_id) {
            (JavaVersion::V26_2 | JavaVersion::V26_3, None) => return Err(Error::MissingSessionId),
            (JavaVersion::V1_21_6 | JavaVersion::V1_21_8 | JavaVersion::V1_21_9 | JavaVersion::V1_21_11 | JavaVersion::V26_1_2, Some(_)) => return Err(Error::UnexpectedSessionId),
            _ => {}
        }
        let mut out = self.uuid.to_vec();
        string(&mut out, &self.username)?;
        varint(&mut out, self.properties.len() as u32);
        for property in &self.properties {
            string(&mut out, &property.name)?;
            string(&mut out, &property.value)?;
            out.push(u8::from(property.signature.is_some()));
            if let Some(signature) = &property.signature { string(&mut out, signature)?; }
        }
        if let Some(id) = self.session_id { out.extend(id); }
        if out.len() > MAX_PACKET { return Err(Error::TooLarge); }
        Ok(out)
    }
}

struct Cursor<'a>(&'a [u8]);
impl<'a> Cursor<'a> {
    fn take(&mut self, size: usize) -> Result<&'a [u8], Error> {
        let (value, rest) = self.0.split_at_checked(size).ok_or(Error::Truncated)?;
        self.0 = rest;
        Ok(value)
    }
    fn count(&mut self, maximum: usize) -> Result<usize, Error> {
        let mut value = 0u32;
        for shift in (0..35).step_by(7) {
            let byte = self.take(1)?[0];
            if shift == 28 && byte & 0xf0 != 0 { return Err(Error::Invalid); }
            value |= u32::from(byte & 127) << shift;
            if byte < 128 {
                let value = value as usize;
                return if value <= maximum { Ok(value) } else { Err(Error::TooLarge) };
            }
        }
        Err(Error::Invalid)
    }
    fn string(&mut self) -> Result<String, Error> {
        let size = self.count(MAX_STRING)?;
        String::from_utf8(self.take(size)?.to_vec()).map_err(|_| Error::Invalid)
    }
}
fn varint(out: &mut Vec<u8>, mut value: u32) {
    while value >= 128 {
        out.push((value as u8 & 127) | 128);
        value >>= 7;
    }
    out.push(value as u8);
}
fn string(out: &mut Vec<u8>, value: &str) -> Result<(), Error> {
    if value.len() > MAX_STRING || out.len() + value.len() + 5 > MAX_PACKET { return Err(Error::TooLarge); }
    varint(out, value.len() as u32);
    out.extend(value.as_bytes());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use JavaVersion::{V26_1_2, V26_2, V26_3};

    fn profile() -> Success {
        Success { uuid: [7; 16], username: "McsaProbe".into(), properties: vec![], session_id: None }
    }

    #[test]
    fn decodes_actual_offline_vanilla_frames() {
        let old_wire = include_bytes!("../testdata/login-success-26.1.2.bin");
        let old = Success::decode(V26_1_2, old_wire).unwrap();
        assert_eq!(old.username, "McsaProbe");
        assert!(old.properties.is_empty() && old.session_id.is_none());
        assert_eq!(old.encode(V26_1_2).unwrap(), old_wire);
        for (version, wire) in [
            (V26_2, include_bytes!("../testdata/login-success-26.2.bin")),
            (V26_3, include_bytes!("../testdata/login-success-26.3.bin")),
        ] {
            let new = Success::decode(version, wire).unwrap();
            assert_eq!(new.username, old.username);
            assert_eq!(new.uuid, old.uuid);
            assert!(new.properties.is_empty());
            let session = new.session_id.unwrap();
            assert_ne!(session, new.uuid);
            assert_ne!(session, [0; 16]);
            assert_eq!(new.encode(version).unwrap(), wire);
            for length in 0..wire.len() {
                assert!(Success::decode(version, &wire[..length]).is_err(), "{version:?}: truncation {length}");
            }
            let mut trailing = wire.to_vec();
            trailing.push(0);
            assert_eq!(Success::decode(version, &trailing), Err(Error::Invalid));
        }
    }

    #[test]
    fn session_uuid_is_explicit_and_preserves_profile_identity() {
        let old = profile();
        let old_wire = old.encode(V26_1_2).unwrap();
        assert_eq!(old_wire.len(), 27); // наблюдение vanilla26.1.2
        assert_eq!(Success::decode(V26_1_2, &old_wire), Ok(old.clone()));
        let mut new = old.clone();
        new.session_id = Some([9; 16]);
        for version in [V26_2, V26_3] {
            assert_eq!(old.encode(version), Err(Error::MissingSessionId));
            let new_wire = new.encode(version).unwrap();
            assert_eq!(new_wire.len(), 43); // наблюдения vanilla26.2/26.3
            assert_eq!(&new_wire[..old_wire.len()], &old_wire);
            assert_eq!(&new_wire[old_wire.len()..], &[9; 16]);
            assert_eq!(Success::decode(version, &new_wire), Ok(new.clone()));
            assert_eq!(Success::decode(version, &old_wire), Err(Error::Truncated));
            assert_eq!(Success::decode(V26_1_2, &new_wire), Err(Error::Invalid));
        }
        assert_eq!(new.encode(V26_1_2), Err(Error::UnexpectedSessionId));
        assert_eq!(new.encode(JavaVersion::V1_21_11), Err(Error::UnexpectedSessionId));
        assert_eq!(old.encode(JavaVersion::V1_21_11).unwrap(),old_wire);
        assert_eq!(Success::decode(JavaVersion::V1_21_11,&old_wire),Ok(old.clone()));
        assert_eq!(old.encode(JavaVersion::V1_12_2), Err(Error::UnsupportedVersion));
        assert_eq!(Success::decode(JavaVersion::V1_12_2, &old_wire), Err(Error::UnsupportedVersion));
    }

    #[test]
    fn signed_and_unsigned_properties_survive_both_formats() {
        let mut value = profile();
        value.properties = vec![
            Property { name: "textures".into(), value: "test-public-texture".into(), signature: Some("test-signature".into()) },
            Property { name: "other".into(), value: "тест".into(), signature: None },
        ];
        for version in [V26_1_2, V26_2, V26_3] {
            value.session_id = matches!(version, V26_2 | V26_3).then_some([3; 16]);
            let wire = value.encode(version).unwrap();
            assert_eq!(Success::decode(version, &wire), Ok(value.clone()));
            for length in 0..wire.len() {
                assert!(Success::decode(version, &wire[..length]).is_err(), "truncation {length}");
            }
            let mut trailing = wire.clone();
            trailing.push(0);
            assert_eq!(Success::decode(version, &trailing), Err(Error::Invalid));
        }
    }

    #[test]
    fn oversized_and_malformed_input_is_rejected_before_allocation() {
        let mut value = profile();
        value.username = "../escape".into();
        assert_eq!(value.encode(V26_1_2), Err(Error::Invalid));
        let mut payload = vec![0; 16];
        payload.extend([255, 255, 255, 255, 255, 0]);
        assert!(Success::decode(V26_3, &payload).is_err());
        let mut payload = profile().encode(V26_1_2).unwrap();
        payload.pop();
        payload.extend([255, 255, 127]);
        assert_eq!(Success::decode(V26_1_2, &payload), Err(Error::TooLarge));
    }
}
