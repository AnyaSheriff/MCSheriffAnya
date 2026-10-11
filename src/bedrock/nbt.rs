// NBT Bedrock: числа little-endian, в сети int/long и длины — varint.
// minecraft.wiki, NBT format: «TAG_Compound»; protocol 944, BlockActorData.
// Один переводчик для акторов и довесков предметов; Java-кодек не меняется.
use super::codec::{In, Out};

#[derive(Clone, Debug, PartialEq)]
pub enum Tag {
    Byte(u8), Short(i16), Int(i32), Long(i64), Float(f32), Double(f64),
    Bytes(Vec<u8>), String(String), List(u8, Vec<Tag>), Compound(Vec<(String, Tag)>),
    Ints(Vec<i32>), Longs(Vec<i64>),
}
#[derive(Clone, Copy, PartialEq)]
pub enum Style { Network, Little }
impl Tag {
    pub fn compound() -> Self { Self::Compound(Vec::new()) }
    pub fn put(&mut self, name: &str, tag: Tag) {
        if let Self::Compound(fields) = self {
            fields.retain(|(key, _)| key != name);
            fields.push((name.into(), tag));
        }
    }
    pub fn string(&mut self, name: &str, value: &str) { self.put(name, Self::String(value.into())); }
    pub fn int(&mut self, name: &str, value: i32) { self.put(name, Self::Int(value)); }
    #[cfg(test)]
    pub fn get(&self, name: &str) -> Option<&Tag> {
        match self { Self::Compound(fields) => fields.iter().find(|(key, _)| key == name).map(|(_, v)| v), _ => None }
    }
    fn kind(&self) -> u8 {
        match self { Self::Byte(_) => 1, Self::Short(_) => 2, Self::Int(_) => 3, Self::Long(_) => 4,
            Self::Float(_) => 5, Self::Double(_) => 6, Self::Bytes(_) => 7, Self::String(_) => 8,
            Self::List(..) => 9, Self::Compound(_) => 10, Self::Ints(_) => 11, Self::Longs(_) => 12 }
    }
    pub fn encode(&self, style: Style) -> Vec<u8> {
        let mut out = Out::default(); out.u8(self.kind()); text(&mut out, "", style);
        self.write(&mut out, style); out.bytes
    }
    fn write(&self, out: &mut Out, style: Style) {
        match self {
            Self::Byte(v) => { out.u8(*v); }
            Self::Short(v) => { out.li16(*v); }
            Self::Int(v) => number(out, *v, style),
            Self::Long(v) => long(out, *v, style),
            Self::Float(v) => { out.lf32(*v); }
            Self::Double(v) => { out.raw(&v.to_le_bytes()); }
            Self::Bytes(v) => { number(out, v.len() as i32, style); out.raw(v); }
            Self::String(v) => text(out, v, style),
            Self::List(kind, v) => {
                out.u8(*kind); number(out, v.len() as i32, style);
                for tag in v { assert_eq!(tag.kind(), *kind); tag.write(out, style); }
            }
            Self::Compound(v) => {
                for (key, tag) in v { out.u8(tag.kind()); text(out, key, style); tag.write(out, style); }
                out.u8(0);
            }
            Self::Ints(v) => { number(out, v.len() as i32, style); for n in v { number(out, *n, style); } }
            Self::Longs(v) => { number(out, v.len() as i32, style); for n in v { long(out, *n, style); } }
        }
    }
}
fn number(out: &mut Out, n: i32, style: Style) {
    if style == Style::Network { out.zigzag32(n); } else { out.li32(n); }
}
fn long(out: &mut Out, n: i64, style: Style) {
    if style == Style::Network { out.zigzag64(n); } else { out.li64(n); }
}
fn text(out: &mut Out, s: &str, style: Style) {
    if style == Style::Network { out.string(s); } else { out.lu16(s.len() as u16).raw(s.as_bytes()); }
}
fn read_number(input: &mut In, style: Style) -> Option<i32> {
    if style == Style::Network { input.zigzag32() } else { input.li32() }
}
fn read_long(input: &mut In, style: Style) -> Option<i64> {
    if style == Style::Network { { let n = input.varint64()?; Some(((n >> 1) as i64) ^ -((n & 1) as i64)) } } else { input.take(8).map(|b| i64::from_le_bytes(b.try_into().unwrap())) }
}
fn read_text(input: &mut In, style: Style) -> Option<String> {
    if style == Style::Network { input.string() } else {
        let len = usize::from(u16::from_le_bytes(input.take(2)?.try_into().ok()?)); String::from_utf8(input.take(len)?.to_vec()).ok()
    }
}
fn count(input: &mut In, style: Style) -> Option<usize> {
    let n = usize::try_from(read_number(input, style)?).ok()?;
    (n <= input.remaining()).then_some(n)
}
fn read(input: &mut In, kind: u8, style: Style, depth: usize) -> Option<Tag> {
    if depth > 32 { return None; }
    Some(match kind {
        1 => Tag::Byte(input.u8()?), 2 => Tag::Short(i16::from_le_bytes(input.take(2)?.try_into().ok()?)),
        3 => Tag::Int(read_number(input, style)?), 4 => Tag::Long(read_long(input, style)?),
        5 => Tag::Float(input.lf32()?), 6 => Tag::Double(f64::from_le_bytes(input.take(8)?.try_into().ok()?)),
        7 => { let n = count(input, style)?; Tag::Bytes(input.take(n)?.to_vec()) }
        8 => Tag::String(read_text(input, style)?),
        9 => {
            let kind = input.u8()?; let n = count(input, style)?;
            Tag::List(kind, (0..n).map(|_| read(input, kind, style, depth + 1)).collect::<Option<_>>()?)
        }
        10 => {
            let mut fields = Vec::new();
            loop { let kind = input.u8()?; if kind == 0 { break; }
                let name = read_text(input, style)?; fields.push((name, read(input, kind, style, depth + 1)?)); }
            Tag::Compound(fields)
        }
        11 => { let n = count(input, style)?; Tag::Ints((0..n).map(|_| read_number(input, style)).collect::<Option<_>>()?) }
        12 => { let n = count(input, style)?; Tag::Longs((0..n).map(|_| read_long(input, style)).collect::<Option<_>>()?) }
        _ => return None,
    })
}
pub fn decode(bytes: &[u8], style: Style) -> Option<Tag> {
    let mut input = In::new(bytes); let kind = input.u8()?; read_text(&mut input, style)?;
    let tag = read(&mut input, kind, style, 0)?; (input.remaining() == 0).then_some(tag)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn both_formats_roundtrip_and_network_golden() {
        let mut root = Tag::compound(); root.int("x", -64);
        assert_eq!(root.encode(Style::Network), [10, 0, 3, 1, b'x', 127, 0]);
        for (i, tag) in [Tag::Byte(255), Tag::Short(-12), Tag::Long(i64::MIN), Tag::Float(1.5),
            Tag::Double(-2.5), Tag::Bytes(vec![0, 255]), Tag::String("пчела".into()),
            Tag::List(10, vec![Tag::compound()]), Tag::Ints(vec![-1, i32::MAX]), Tag::Longs(vec![i64::MAX])].into_iter().enumerate() {
            root.put(&i.to_string(), tag);
        }
        for style in [Style::Network, Style::Little] {
            let bytes = root.encode(style); assert_eq!(decode(&bytes, style), Some(root.clone()));
            for n in 0..bytes.len() { assert!(decode(&bytes[..n], style).is_none()); }
        }
    }
}
