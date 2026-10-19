//! Достижения775→340: JSON-текст, старый Slot и явный список criteria.
//! Фоны подтверждены обычной выдачей пяти корневых достижений на340.
use std::collections::BTreeSet;
use crate::{items, legacy_items, registry, wire::{Cursor, Error, uint}};
fn string(out: &mut Vec<u8>, value: &str) { uint(out,value.len() as u32); out.extend(value.as_bytes()); }
fn text(input: &mut Cursor<'_>, out: &mut Vec<u8>) -> Result<(),Error> {
    let n=registry::nbt_length(input.rest).map_err(|_|Error::Invalid)?;
    string(out,&crate::legacy_text::json(input.take(n)?)?); Ok(())
}
fn boolean(input: &mut Cursor<'_>, out: &mut Vec<u8>) -> Result<bool,Error> {
    let value=input.boolean()?; out.push(u8::from(value)); Ok(value)
}
pub fn translate(bytes: &[u8], enchantments: &[&str]) -> Result<Vec<u8>,Error> {
    if bytes.len()>8*1024*1024 { return Err(Error::TooLarge); }
    let mut input=Cursor::new(bytes); let mut out=vec![0x4d];
    boolean(&mut input,&mut out)?;
    let n=input.count(4096)?; uint(&mut out,n as u32); let mut keys=BTreeSet::new();
    for _ in 0..n {
        let key=input.string()?; if !keys.insert(key) { return Err(Error::Invalid); } string(&mut out,key);
        if boolean(&mut input,&mut out)? { string(&mut out,input.string()?); }
        if boolean(&mut input,&mut out)? {
            text(&mut input,&mut out)?; text(&mut input,&mut out)?;
            let (icon,n)=items::decode_template(crate::wire::INTERNAL,input.rest,items::Context {shared_enchantment_count:Some(enchantments.len() as u32)}).map_err(Error::Item)?;
            input.take(n)?; out.extend(legacy_items::encode(Some(&icon),legacy_items::Context {enchantments}).map_err(Error::LegacyItem)?);
            let frame=input.count(2)?; uint(&mut out,frame as u32);
            let flags=u32::from_be_bytes(input.take(4)?.try_into().map_err(|_|Error::Invalid)?);
            if flags&!7!=0 { return Err(Error::Invalid); } out.extend(flags.to_be_bytes());
            if flags&1!=0 {
                let source=input.string()?;
                let name=source.strip_prefix("minecraft:gui/advancements/backgrounds/")
                    .filter(|name|matches!(*name,"stone"|"nether"|"end"|"adventure"|"husbandry"))
                    .ok_or(Error::PayloadUnavailable("legacy advancement background"))?;
                string(&mut out,&format!("minecraft:textures/gui/advancements/backgrounds/{name}.png"));
            }
            for _ in 0..2 {
                let value=input.take(4)?;
                if !f32::from_be_bytes(value.try_into().map_err(|_|Error::Invalid)?).is_finite() { return Err(Error::Invalid); }
                out.extend(value);
            }
        }
        let n=input.count(4096)?; let mut groups=Vec::new(); uint(&mut groups,n as u32); let mut criteria=BTreeSet::new();
        for _ in 0..n {
            let n=input.count(4096)?; uint(&mut groups,n as u32);
            for _ in 0..n { let name=input.string()?; string(&mut groups,name); criteria.insert(name); }
        }
        input.boolean()?; // sends telemetry: у340 нет этого поля.
        uint(&mut out,criteria.len() as u32); for name in criteria { string(&mut out,name); }
        out.extend(groups);
    }
    let n=input.count(4096)?; uint(&mut out,n as u32);
    for _ in 0..n { string(&mut out,input.string()?); }
    let n=input.count(4096)?; uint(&mut out,n as u32);
    for _ in 0..n {
        string(&mut out,input.string()?); let n=input.count(4096)?; uint(&mut out,n as u32);
        let mut seen=BTreeSet::new();
        for _ in 0..n {
            let name=input.string()?; if !seen.insert(name) { return Err(Error::Invalid); } string(&mut out,name);
            if boolean(&mut input,&mut out)? { out.extend(input.take(8)?); }
        }
    }
    if !input.boolean()? { return Err(Error::PayloadUnavailable("legacy hidden advancements")); }
    if input.remaining()!=0 { return Err(Error::Invalid); }
    if out.len()>8*1024*1024 { return Err(Error::TooLarge); } Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_criteria_are_restored_from_requirement_groups() {
        let mut native=vec![1,1]; string(&mut native,"mcsa:test"); native.extend([0,0,2,2]);
        string(&mut native,"a"); string(&mut native,"b"); native.push(1); string(&mut native,"a");
        native.extend([0,0,0,1]);
        let mut expected=vec![0x4d,1,1]; string(&mut expected,"mcsa:test"); expected.extend([0,0,2]);
        string(&mut expected,"a"); string(&mut expected,"b"); expected.extend([2,2]);
        string(&mut expected,"a"); string(&mut expected,"b"); expected.push(1); string(&mut expected,"a"); expected.extend([0,0]);
        assert_eq!(translate(&native,&[]).unwrap(),expected);
        for n in 0..native.len() { assert!(translate(&native[..n],&[]).is_err()); }
        native.push(0); assert!(translate(&native,&[]).is_err());
    }
}
