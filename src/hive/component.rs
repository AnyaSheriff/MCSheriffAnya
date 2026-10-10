// Компонент bees: minecraft-data 26.1 protocol.json; NBT — minecraft.wiki/w/NBT_format.
// Ограничения длины и вложенности относятся к недоверенному творческому слоту.
use crate::snbt::Value;

struct Reader<'a> { rest: &'a [u8] }
impl Reader<'_> {
    fn take(&mut self, n: usize) -> Option<&[u8]> {
        let (head,tail)=self.rest.split_at_checked(n)?; self.rest=tail; Some(head)
    }
    fn byte(&mut self) -> Option<u8> { Some(self.take(1)?[0]) }
    fn varint(&mut self) -> Option<i32> {
        let mut value=0u32;
        for shift in (0..35).step_by(7) {
            let b=self.byte()?; value|=u32::from(b&127)<<shift;
            if b&128==0 { return Some(value as i32); }
        }
        None
    }
    fn string(&mut self) -> Option<String> {
        let n=u16::from_be_bytes(self.take(2)?.try_into().ok()?) as usize;
        String::from_utf8(self.take(n)?.to_vec()).ok()
    }
    fn length(&mut self) -> Option<usize> {
        let n=i32::from_be_bytes(self.take(4)?.try_into().ok()?);
        (0..=65536).contains(&n).then_some(n as usize)
    }
    fn value(&mut self, kind: u8, depth: u8) -> Option<Value> {
        if depth>16 { return None; }
        Some(match kind {
            1=>Value::Num(f64::from(self.byte()? as i8)),
            2=>Value::Num(f64::from(i16::from_be_bytes(self.take(2)?.try_into().ok()?))),
            3=>Value::Num(f64::from(i32::from_be_bytes(self.take(4)?.try_into().ok()?))),
            4=>Value::Num(i64::from_be_bytes(self.take(8)?.try_into().ok()?) as f64),
            5=>Value::Num(f64::from(f32::from_be_bytes(self.take(4)?.try_into().ok()?))),
            6=>Value::Num(f64::from_be_bytes(self.take(8)?.try_into().ok()?)),
            8=>Value::Str(self.string()?),
            7|9|11|12=> {
                let child=match kind {7=>1,11=>3,12=>4,_=>self.byte()?};
                let count=self.length()?;
                let mut values=Vec::new();
                for _ in 0..count { values.push(self.value(child,depth+1)?); }
                Value::List(values)
            }
            10=> {
                let mut fields=Vec::new();
                loop {
                    let child=self.byte()?;
                    if child==0 {break;}
                    if fields.len()>=256 {return None;}
                    let name=self.string()?;
                    fields.push((name,self.value(child,depth+1)?));
                }
                Value::Map(fields)
            }
            _=>return None,
        })
    }
}

pub fn decode(bytes: &[u8]) -> Option<super::Data> {
    let mut r=Reader {rest:bytes};
    let count=r.varint()?;
    if !(0..=3).contains(&count) {return None;}
    let mut entries=Vec::new();
    for _ in 0..count {
        let kind=r.byte()?;
        if kind!=10 {return None;}
        let entity=r.value(kind,0)?;
        let ticks=r.varint()?;let minimum=r.varint()?;
        entries.push(Value::Map(vec![("entity_data".into(),entity),("ticks_in_hive".into(),Value::Num(f64::from(ticks))),("min_ticks_in_hive".into(),Value::Num(f64::from(minimum)))]));
    }
    if !r.rest.is_empty() {return None;}
    super::from_value(&Value::Map(vec![("bees".into(),Value::List(entries))]))
}
