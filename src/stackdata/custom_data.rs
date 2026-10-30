//! Произвольный Java NBT без преобразования чисел и строк в JSON/SNBT.
//! Вики, NBT format: «TAG_Compound»; Java DataInput: «modified UTF-8».
//! Канонизируем только порядок полей compound. Строки остаются исходными
//! MUTF-8 байтами, float/double — исходными битами, списки — в исходном порядке.
//! Пределы — защита нашего формата, не лимиты игры. Дубликаты ключей
//! отвергаются до отдельной vanilla-пробы [проверить].
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct CustomData(Vec<u8>);

impl CustomData {
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        let (value, length) = Self::read_prefix(bytes)?;
        (length == bytes.len()).then_some(value)
    }

    pub(super) fn read_prefix(bytes: &[u8]) -> Option<(Self, usize)> {
        let mut reader = Nbt { bytes, at: 0, budget: 65536 };
        if reader.byte()? != 10 { return None; }
        let mut out = vec![10];
        reader.value(10, 0, &mut out)?;
        if out.len() > u16::MAX as usize { return None; }
        Some((Self(out), reader.at))
    }

    pub fn as_bytes(&self) -> &[u8] { &self.0 }

    /// Только ASCII-метка нашего TNT. Остальные строки не декодируем.
    pub(super) fn ascii_field(&self, key: &str) -> Option<&str> {
        let mut reader = Nbt { bytes: &self.0, at: 1, budget: 65536 };
        loop {
            let kind = reader.byte()?;
            if kind == 0 { return None; }
            let name = reader.text()?;
            if name == key.as_bytes() && kind == 8 {
                let value = reader.text()?;
                return value.is_ascii().then(|| std::str::from_utf8(value).ok()).flatten();
            }
            reader.value(kind, 0, &mut Vec::new())?;
        }
    }

    pub(super) fn has_field(&self, key: &str) -> bool {
        let mut reader = Nbt { bytes: &self.0, at: 1, budget: 65536 };
        loop {
            let Some(kind) = reader.byte() else { return false; };
            if kind == 0 { return false; }
            let Some(name) = reader.text() else { return false; };
            if name == key.as_bytes() { return true; }
            if reader.value(kind, 0, &mut Vec::new()).is_none() { return false; }
        }
    }

    pub(super) fn with_ascii_field(&self, key: &str, value: &str) -> Self {
        debug_assert!(key.is_ascii() && value.is_ascii());
        let mut reader = Nbt { bytes: &self.0, at: 1, budget: 65536 };
        let mut fields = Vec::new();
        while let Some(kind) = reader.byte() {
            if kind == 0 { break; }
            let start = reader.at - 1;
            let name = reader.text().expect("проверенное имя NBT");
            let keep = name != key.as_bytes();
            reader.value(kind, 0, &mut Vec::new()).expect("проверенный NBT");
            if keep { fields.push((name.to_vec(), self.0[start..reader.at].to_vec())); }
        }
        let mut field = vec![8];
        for text in [key, value] {
            field.extend_from_slice(&(text.len() as u16).to_be_bytes());
            field.extend_from_slice(text.as_bytes());
        }
        fields.push((key.as_bytes().to_vec(), field));
        fields.sort_unstable_by(|a,b| a.0.cmp(&b.0));
        let mut canonical = vec![10];
        for (_, bytes) in fields { canonical.extend(bytes); }
        canonical.push(0);
        Self(canonical)
    }
}

struct Nbt<'a> { bytes: &'a [u8], at: usize, budget: usize }
impl<'a> Nbt<'a> {
    fn take(&mut self, n: usize) -> Option<&'a [u8]> {
        let end = self.at.checked_add(n)?;
        // Предел исходного компонента, включая неканонический порядок полей.
        if end > u16::MAX as usize { return None; }
        let value = self.bytes.get(self.at..end)?;
        self.at = end;
        Some(value)
    }
    fn byte(&mut self) -> Option<u8> { Some(self.take(1)?[0]) }
    fn count(&mut self) -> Option<usize> {
        usize::try_from(i32::from_be_bytes(self.take(4)?.try_into().ok()?)).ok()
    }
    fn text(&mut self) -> Option<&'a [u8]> {
        let n = usize::from(u16::from_be_bytes(self.take(2)?.try_into().ok()?));
        let bytes = self.take(n)?;
        let mut at = 0;
        while at < bytes.len() {
            let first = bytes[at];
            let n = match first {
                1..=0x7f => 1,
                0xc0..=0xdf => {
                    let second = *bytes.get(at + 1)?;
                    if second & 0xc0 != 0x80 || (first < 0xc2 && (first != 0xc0 || second != 0x80)) { return None; }
                    2
                }
                0xe0..=0xef => {
                    let (second, third) = (*bytes.get(at + 1)?, *bytes.get(at + 2)?);
                    if second & 0xc0 != 0x80 || third & 0xc0 != 0x80 || (first == 0xe0 && second < 0xa0) { return None; }
                    3
                }
                _ => return None,
            };
            at += n;
        }
        Some(bytes)
    }
    fn value(&mut self, kind: u8, depth: usize, out: &mut Vec<u8>) -> Option<()> {
        if depth > 32 { return None; }
        self.budget = self.budget.checked_sub(1)?;
        let start = self.at;
        match kind {
            1..=6 => { self.take([0,1,2,4,8,4,8][kind as usize])?; }
            7 | 11 | 12 => {
                let n = self.count()?;
                self.take(n.checked_mul(match kind { 11 => 4, 12 => 8, _ => 1 })?)?;
            }
            8 => { self.text()?; }
            9 => {
                let element = self.byte()?;
                let n = self.count()?;
                if element > 12 || (element == 0 && n != 0) || n > self.budget { return None; }
                out.extend_from_slice(&self.bytes[start..self.at]);
                for _ in 0..n { self.value(element, depth + 1, out)?; }
                return Some(());
            }
            10 => {
                let mut fields = Vec::new();
                loop {
                    let kind = self.byte()?;
                    if kind == 0 { break; }
                    let name = self.text()?;
                    let mut value = Vec::new();
                    self.value(kind, depth + 1, &mut value)?;
                    fields.push((name, kind, value));
                }
                fields.sort_unstable_by(|a,b| a.0.cmp(b.0));
                if fields.windows(2).any(|pair| pair[0].0 == pair[1].0) { return None; }
                for (name, kind, value) in fields {
                    out.push(kind); out.extend_from_slice(&(name.len() as u16).to_be_bytes());
                    out.extend_from_slice(name); out.extend(value);
                }
                out.push(0);
                return Some(());
            }
            _ => return None,
        }
        out.extend_from_slice(&self.bytes[start..self.at]);
        Some(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_compounds_keep_types_bits_arrays_and_modified_utf8() {
        let mut fields = Vec::new();
        for (kind, name, body) in [
            (1,"b", vec![255]), (2,"s",(-123i16).to_be_bytes().to_vec()),
            (3,"i",i32::MAX.to_be_bytes().to_vec()), (4,"l",9007199254740993i64.to_be_bytes().to_vec()),
            (5,"f",0x7fc00001u32.to_be_bytes().to_vec()), (6,"d",(-0.0f64).to_be_bytes().to_vec()),
            (7,"ba",vec![0,0,0,2,0,255]), (8,"str",vec![0,8,0xc0,0x80,0xed,0xa0,0xbd,0xed,0xb8,0x80]),
            (9,"list",vec![10,0,0,0,1,0]), (10,"compound",vec![0]),
            (11,"ia",vec![0,0,0,1,0x80,0,0,0]), (12,"la",[1i32.to_be_bytes().as_slice(),i64::MIN.to_be_bytes().as_slice()].concat()),
        ] {
            let mut field = vec![kind];field.extend_from_slice(&(name.len() as u16).to_be_bytes());field.extend_from_slice(name.as_bytes());field.extend(body);fields.push(field);
        }
        let encode = |fields: &[Vec<u8>]| [vec![10],fields.concat(),vec![0]].concat();
        let a = CustomData::from_bytes(&encode(&fields)).unwrap();fields.reverse();
        let b = CustomData::from_bytes(&encode(&fields)).unwrap();assert_eq!(a,b);
        assert_eq!(CustomData::from_bytes(a.as_bytes()),Some(a.clone()));
        assert!(a.as_bytes().windows(8).any(|v|v==9007199254740993i64.to_be_bytes()));
        for n in 0..a.as_bytes().len() { assert!(CustomData::from_bytes(&a.as_bytes()[..n]).is_none()); }
    }
    #[test]
    fn rejects_ambiguous_or_malformed_compounds() {
        for bytes in [vec![0],vec![10,0,0],vec![10,1,0,1,b'a',1,1,0,1,b'a',2,0],
            vec![10,9,0,1,b'l',0,0,0,0,1,0],vec![10,8,0,1,b's',0,1,0,0],
            vec![10,8,0,1,b's',0,4,0xf0,0x9f,0x98,0x80,0]] {
            assert!(CustomData::from_bytes(&bytes).is_none(),"{bytes:?}");
        }
        assert!(CustomData::from_bytes(&[10,0]).is_some());
    }
    #[test]
    fn exact_depth_and_size_boundaries_do_not_panic_or_truncate() {
        let nested = |depth| {
            let mut bytes = vec![10];
            for _ in 0..depth { bytes.extend([10,0,1,b'x']); }
            bytes.extend(vec![0;depth+1]);bytes
        };
        assert!(CustomData::from_bytes(&nested(32)).is_some());
        assert!(CustomData::from_bytes(&nested(33)).is_none());
        let array = |n: usize| {
            let mut bytes = vec![10,7,0,1,b'x'];bytes.extend((n as i32).to_be_bytes());
            bytes.extend(vec![0;n]);bytes.push(0);bytes
        };
        let maximum = array(65525);assert_eq!(maximum.len(),65535);
        assert!(CustomData::from_bytes(&maximum).is_some());
        assert!(CustomData::from_bytes(&array(65526)).is_none());
        let data = crate::stackdata::Data {custom_data:CustomData::from_bytes(&array(65506)),..Default::default()};
        let bytes = crate::stackdata::to_bytes(&data);assert_eq!(bytes.len(),65535);
        assert_eq!(crate::stackdata::from_bytes(&bytes),Some(data));
        let data = crate::stackdata::Data {custom_data:CustomData::from_bytes(&array(65507)),..Default::default()};
        assert_eq!(crate::stackdata::to_bytes(&data).len(),65536);
        let stack = crate::inventory::Stack::new(crate::blocks::item_named("stone").unwrap(),1).with_extras(data);
        assert!(crate::gameplay_catalog::Catalog::new(crate::gameplay_catalog::Profile::Java26_1_2).validate_stack(&stack).is_err());
        // Редкий внутренний вызов с почти полным compound не паникует даже
        // при добавлении метки; каталог отвергнет выросшую стопку до сохранения.
        let custom = CustomData::from_bytes(&maximum).unwrap();
        assert!(custom.with_ascii_field(crate::custom_tnt::MARKER,"nuke").as_bytes().len()>65535);
        let kind = crate::custom_tnt::by_name("nuke").unwrap();
        let stack = crate::inventory::Stack {tnt:Some(kind.id),..crate::inventory::Stack::new(crate::blocks::item_named("tnt").unwrap(),1)
            .with_extras(crate::stackdata::Data {custom_data:Some(custom),..Default::default()})};
        assert!(crate::gameplay_catalog::Catalog::new(crate::gameplay_catalog::Profile::Java26_1_2).validate_stack(&stack).is_err());
    }

    #[test]
    fn modified_utf8_rejects_overlong_and_truncated_sequences_in_names_and_values() {
        for invalid in [vec![0xc1,0x81],vec![0xc0,0x81],vec![0xc0],vec![0xc2],
            vec![0xe0,0x80,0x80],vec![0xe0],vec![0xe1,0x80],vec![0xe1,0x41,0x80],vec![0x80]] {
            let length = (invalid.len() as u16).to_be_bytes();
            let name = [vec![10,1],length.to_vec(),invalid.clone(),vec![1,0]].concat();
            let value = [vec![10,8,0,1,b'x'],length.to_vec(),invalid,vec![0]].concat();
            assert!(CustomData::from_bytes(&name).is_none());
            assert!(CustomData::from_bytes(&value).is_none());
        }
    }

}
