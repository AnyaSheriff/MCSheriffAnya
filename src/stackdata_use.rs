// Wiki, Data component format: debug_stick_state и recipes — anonymous NBT.
use super::*;

fn nbt_string(reader: &mut Reader) -> Option<String> {
    let len = u16::from_be_bytes(reader.take(2)?.try_into().ok()?) as usize;
    String::from_utf8(reader.take(len)?.to_vec()).ok()
}

pub(super) fn read_map(reader: &mut Reader) -> Option<Vec<(String, String)>> {
    if reader.take(1)? != [10] { return None; }
    let mut result = Vec::new();
    loop {
        match reader.take(1)?[0] {
            0 => return Some(result),
            8 if result.len() < 1024 => result.push((nbt_string(reader)?, nbt_string(reader)?)),
            _ => return None,
        }
    }
}
pub(super) fn read_recipes(reader: &mut Reader) -> Option<Vec<String>> {
    if reader.take(2)? != [9, 8] { return None; }
    let count = i32::from_be_bytes(reader.take(4)?.try_into().ok()?);
    if !(0..=4096).contains(&count) { return None; }
    (0..count).map(|_| nbt_string(reader)).collect()
}
pub(super) fn encode_map(values: &[(String, String)]) -> Vec<u8> {
    let mut compound = crate::network::nbt::Compound::new();
    for (key, value) in values { compound = compound.string(key, value); }
    compound.encode_network()
}
pub(super) fn encode_recipes(values: &[String]) -> Vec<u8> {
    let mut bytes = vec![9,8]; bytes.extend_from_slice(&(values.len() as i32).to_be_bytes());
    for value in values { bytes.extend_from_slice(&(value.len() as u16).to_be_bytes()); bytes.extend_from_slice(value.as_bytes()); }
    bytes
}
