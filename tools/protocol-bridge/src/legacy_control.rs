//! Служебные пакеты 340: закреплённый protocol.json и записи оригинала.
//! Это кодек входа/телепорта/возрождения, не включённый маршрут старого клиента.
//! Строки ограничены; данные разбираются один раз без промежуточного JSON.
use crate::{State, teleport::Pose, wire::{Cursor, Error, uint}};

pub const PROTOCOL: i32 = 340;
const MAX_PACKET: usize = 65536;

#[derive(Clone, Debug, PartialEq)]
pub enum Packet {
    LoginSuccess { uuid: [u8; 16], username: String },
    Compression { threshold: i32 },
    Join { entity: i32, mode: u8, hardcore: bool, dimension: i32, difficulty: u8, maximum: u8, level_type: String, reduced_debug: bool },
    Respawn { dimension: i32, difficulty: u8, mode: u8, level_type: String },
    Teleport { pose: Pose, flags: u8, id: i32 },
    Abilities { flags: u8, flying: f32, walking: f32 },
    GameState { reason: u8, value: f32 },
}

fn word(input: &mut Cursor<'_>) -> Result<i32, Error> {
    Ok(i32::from_be_bytes(input.take(4)?.try_into().map_err(|_| Error::Invalid)?))
}
fn float(input: &mut Cursor<'_>) -> Result<f32, Error> {
    Ok(f32::from_be_bytes(input.take(4)?.try_into().map_err(|_| Error::Invalid)?))
}
fn double(input: &mut Cursor<'_>) -> Result<f64, Error> {
    Ok(f64::from_be_bytes(input.take(8)?.try_into().map_err(|_| Error::Invalid)?))
}
fn finite(pose: Pose) -> bool {
    [pose.x, pose.y, pose.z].into_iter().all(f64::is_finite) && pose.yaw.is_finite() && pose.pitch.is_finite()
}
fn string(out: &mut Vec<u8>, value: &str) {
    uint(out, value.len() as u32); out.extend(value.as_bytes());
}
fn uuid_from_text(value: &str) -> Result<[u8; 16], Error> {
    if value.len() != 36 { return Err(Error::Invalid); }
    let mut uuid = [0; 16];
    let mut digits = 0;
    for (index, byte) in value.bytes().enumerate() {
        if [8, 13, 18, 23].contains(&index) {
            if byte != b'-' { return Err(Error::Invalid); }
            continue;
        }
        let nibble = match byte { b'0'..=b'9' => byte - b'0', b'a'..=b'f' => byte - b'a' + 10, b'A'..=b'F' => byte - b'A' + 10, _ => return Err(Error::Invalid) };
        uuid[digits / 2] = (uuid[digits / 2] << 4) | nibble;
        digits += 1;
    }
    Ok(uuid)
}
fn uuid_text(out: &mut Vec<u8>, uuid: &[u8; 16]) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    out.push(36);
    for (index, byte) in uuid.iter().enumerate() {
        if [4, 6, 8, 10].contains(&index) { out.push(b'-'); }
        out.push(HEX[usize::from(byte >> 4)]); out.push(HEX[usize::from(byte & 15)]);
    }
}

impl Packet {
    pub fn decode(state: State, id: u32, payload: &[u8]) -> Result<Self, Error> {
        if payload.len() > MAX_PACKET { return Err(Error::TooLarge); }
        let mut input = Cursor::new(payload);
        let packet = match (state, id) {
            (State::Login, 2) => Self::LoginSuccess { uuid: uuid_from_text(input.string()?)?, username: input.string()?.to_string() },
            (State::Login, 3) => Self::Compression { threshold: input.varint()? },
            (State::Play, 0x23) => {
                let entity = word(&mut input)?;
                let mode = input.take(1)?[0];
                if mode & !0x0b != 0 { return Err(Error::Invalid); }
                Self::Join { entity, mode: mode & 3, hardcore: mode & 8 != 0, dimension: word(&mut input)?, difficulty: input.take(1)?[0], maximum: input.take(1)?[0], level_type: input.string()?.to_string(), reduced_debug: input.boolean()? }
            }
            (State::Play, 0x35) => Self::Respawn { dimension: word(&mut input)?, difficulty: input.take(1)?[0], mode: input.take(1)?[0], level_type: input.string()?.to_string() },
            (State::Play, 0x2f) => Self::Teleport {
                pose: Pose { x: double(&mut input)?, y: double(&mut input)?, z: double(&mut input)?, yaw: float(&mut input)?, pitch: float(&mut input)? },
                flags: input.take(1)?[0], id: input.varint()?,
            },
            (State::Play, 0x2c) => Self::Abilities { flags: input.take(1)?[0], flying: float(&mut input)?, walking: float(&mut input)? },
            (State::Play, 0x1e) => Self::GameState { reason: input.take(1)?[0], value: float(&mut input)? },
            _ => return Err(Error::PayloadUnavailable("legacy control")),
        };
        if !input.rest.is_empty() { return Err(Error::Invalid); }
        packet.validate()?;
        Ok(packet)
    }

    fn validate(&self) -> Result<(), Error> {
        let valid = match self {
            Self::LoginSuccess { username, .. } => (1..=16).contains(&username.len()) && username.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'),
            Self::Compression { threshold } => *threshold >= 0,
            Self::Join { mode, difficulty, level_type, .. } | Self::Respawn { mode, difficulty, level_type, .. } => *mode <= 3 && *difficulty <= 3 && level_type.len() <= 32767,
            Self::Teleport { pose, flags, .. } => finite(*pose) && flags & !31 == 0,
            Self::Abilities { flags, flying, walking } => flags & !15 == 0 && flying.is_finite() && walking.is_finite() && *flying >= 0.0 && *walking >= 0.0,
            Self::GameState { reason, value } => matches!(reason, 0..=8 | 10) && value.is_finite(),
        };
        if valid { Ok(()) } else { Err(Error::Invalid) }
    }

    /// Возвращает фазу, ID и тело. Внешнее сжатие/шифрование делает соединение.
    pub fn encode(&self) -> Result<(State, u32, Vec<u8>), Error> {
        self.validate()?;
        let mut out = Vec::with_capacity(64);
        let (state, id) = match self {
            Self::LoginSuccess { uuid, username } => {
                uuid_text(&mut out, uuid); string(&mut out, username); (State::Login, 2)
            }
            Self::Compression { threshold } => { uint(&mut out, *threshold as u32); (State::Login, 3) }
            Self::Join { entity, mode, hardcore, dimension, difficulty, maximum, level_type, reduced_debug } => {
                out.extend(entity.to_be_bytes()); out.push(*mode | if *hardcore { 8 } else { 0 });
                out.extend(dimension.to_be_bytes()); out.extend([*difficulty, *maximum]);
                string(&mut out, level_type); out.push(u8::from(*reduced_debug)); (State::Play, 0x23)
            }
            Self::Respawn { dimension, difficulty, mode, level_type } => {
                out.extend(dimension.to_be_bytes()); out.extend([*difficulty, *mode]); string(&mut out, level_type); (State::Play, 0x35)
            }
            Self::Teleport { pose, flags, id } => {
                for v in [pose.x, pose.y, pose.z] { out.extend(v.to_be_bytes()); }
                for v in [pose.yaw, pose.pitch] { out.extend(v.to_be_bytes()); }
                out.push(*flags); uint(&mut out, *id as u32); (State::Play, 0x2f)
            }
            Self::Abilities { flags, flying, walking } => {
                out.push(*flags); out.extend(flying.to_be_bytes()); out.extend(walking.to_be_bytes()); (State::Play, 0x2c)
            }
            Self::GameState { reason, value } => { out.push(*reason); out.extend(value.to_be_bytes()); (State::Play, 0x1e) }
        };
        Ok((state, id, out))
    }
}

/// В340 нет новой скорости/флагов777. Относительные координаты требуют
/// известного положения клиента; нельзя молча подставлять начало мира.
pub fn resolve_teleport(pose: Pose, flags: u8, previous: Option<Pose>) -> Result<Pose, Error> {
    if !finite(pose) || flags & !31 != 0 { return Err(Error::Invalid); }
    if flags == 0 { return Ok(pose); }
    let old = previous.filter(|p| finite(*p)).ok_or(Error::Invalid)?;
    let result = Pose {
        x: pose.x + if flags & 1 != 0 { old.x } else { 0.0 },
        y: pose.y + if flags & 2 != 0 { old.y } else { 0.0 },
        z: pose.z + if flags & 4 != 0 { old.z } else { 0.0 },
        yaw: pose.yaw + if flags & 8 != 0 { old.yaw } else { 0.0 },
        pitch: pose.pitch + if flags & 16 != 0 { old.pitch } else { 0.0 },
    };
    if finite(result) { Ok(result) } else { Err(Error::Invalid) }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn bytes(hex: &str) -> Vec<u8> {
        hex.as_bytes().as_chunks::<2>().0.iter().map(|v| u8::from_str_radix(std::str::from_utf8(v).unwrap(),16).unwrap()).collect()
    }

    #[test]
    fn official_plain_and_compressed_sessions_reencode_exact_control_packets() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!("../testdata/legacy-session-observations.json")).unwrap();
        let mut count = 0;
        for run in fixture["runs"].as_array().unwrap() {
            let mut previous = None;
            for record in run["packets"].as_array().unwrap() {
                let state = if record["phase"] == "login" { State::Login } else { State::Play };
                let id = record["id"].as_u64().unwrap() as u32;
                let payload = bytes(record["payload"].as_str().unwrap());
                let packet = Packet::decode(state, id, &payload).unwrap();
                assert_eq!(packet.encode().unwrap(), (state, id, payload.clone()));
                if let Packet::Teleport { pose, flags, .. } = packet {
                    let target = resolve_teleport(pose, flags, previous).unwrap();
                    let actual = serde_json::json!([target.x,target.y,target.z,target.yaw,target.pitch]);
                    assert_eq!(actual, record["values"]);
                    previous = Some(target);
                }
                for n in 0..payload.len() { assert!(Packet::decode(state,id,&payload[..n]).is_err()); }
                let mut trailing = payload; trailing.push(0);
                assert!(Packet::decode(state,id,&trailing).is_err());
                count += 1;
            }
        }
        assert!(count >= 20);
    }

    #[test]
    fn relative_teleport_requires_state_and_rejects_non_finite_or_unknown_flags() {
        let pose = Pose { x:1.0,y:2.0,z:3.0,yaw:4.0,pitch:5.0 };
        assert!(resolve_teleport(pose,1,None).is_err());
        assert!(resolve_teleport(pose,32,Some(pose)).is_err());
        let old = Pose { x:10.0,y:20.0,z:30.0,yaw:40.0,pitch:50.0 };
        let result = resolve_teleport(pose,1|4|16,Some(old)).unwrap();
        assert_eq!(result,Pose { x:11.0,y:2.0,z:33.0,yaw:4.0,pitch:55.0 });
        for value in [f64::NAN,f64::INFINITY,f64::NEG_INFINITY] {
            assert!(resolve_teleport(Pose { x:value,..pose },0,None).is_err());
            assert!(Packet::Teleport { pose:Pose { x:value,..pose },flags:0,id:1 }.encode().is_err());
        }
        let large = Pose { x:f64::MAX,..pose };
        assert!(resolve_teleport(large,1,Some(large)).is_err());
        for id in [i32::MIN,-1,0,127,128,i32::MAX] {
            let packet=Packet::Teleport { pose,flags:0,id }; let (state,kind,body)=packet.encode().unwrap();
            assert_eq!(Packet::decode(state,kind,&body).unwrap(),packet);
        }
    }

    #[test]
    fn invalid_profiles_flags_modes_and_compression_are_rejected() {
        for username in ["", "bad name", "non_ascii_я", "12345678901234567"] {
            assert!(Packet::LoginSuccess { uuid:[0;16],username:username.into() }.encode().is_err());
        }
        assert!(uuid_from_text("000000000000-0000-0000-000000000000").is_err());
        assert!(Packet::Compression { threshold:-1 }.encode().is_err());
        assert!(Packet::Abilities { flags:16,flying:0.05,walking:0.1 }.encode().is_err());
        assert!(Packet::Respawn { dimension:0,difficulty:4,mode:0,level_type:"flat".into() }.encode().is_err());
        assert!(Packet::Respawn { dimension:0,difficulty:2,mode:4,level_type:"flat".into() }.encode().is_err());
        assert!(Packet::decode(State::Configuration,2,&[]).is_err());
        assert!(Packet::decode(State::Login,2,&vec![0;MAX_PACKET+1]).is_err());
    }
}
