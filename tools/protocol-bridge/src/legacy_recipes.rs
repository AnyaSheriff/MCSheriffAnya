//! Первые проверенные рецепты340. ID получены обычной /recipe, не порядком
//! современных таблиц. Неподдержанная форма не выбирает рецепт по одному выходу.
use std::collections::{BTreeMap, BTreeSet};
use crate::{items, wire::{Cursor, Error, uint}};

// tools/data/items.json (775); формы — собственная crafting/table.rs.
const PLANKS: &[u32] = &[36,37,38,39,40,41,42,43,44,45,46,47];
const OLD_PLANKS: &[u32] = &[36,37,38,39,40,42];
const TABLE: u32 = 333;
const STICK: u32 = 947;

#[derive(Clone, Debug, Default)]
pub struct Book {
    settings: [bool;2],
    /// display ID775 -> (ID340, подсветка).
    known: BTreeMap<u32,(u32,bool)>,
}

fn display(input: &mut Cursor<'_>, depth: usize, budget: &mut usize) -> Result<Vec<(u32,u32)>,Error> {
    if depth > 16 || *budget == 0 { return Err(Error::TooLarge); } *budget -= 1;
    match input.uint()? {
        0 => Ok(vec![]),
        4 => Ok(vec![(input.uint()?,1)]),
        5 => {
            let (stack,n) = items::decode_template(crate::wire::INTERNAL,input.rest,items::Context::default()).map_err(Error::Item)?;
            input.take(n)?;
            if !stack.added.is_empty() || !stack.removed.is_empty() { return Err(Error::PayloadUnavailable("legacy recipe components")); }
            Ok(vec![(stack.item,stack.count)])
        }
        10 => {
            let n = input.count(256)?; let mut out = Vec::new();
            for _ in 0..n { out.extend(display(input,depth+1,budget)?); }
            if out.len() > 256 { return Err(Error::TooLarge); } Ok(out)
        }
        _ => Err(Error::PayloadUnavailable("legacy recipe display")),
    }
}
fn plank_cell(cell: &[(u32,u32)]) -> bool {
    cell.iter().all(|(id,n)| *n == 1 && PLANKS.contains(id))
        && OLD_PLANKS.iter().all(|id| cell.contains(&(*id,1)))
}
fn recipe(input: &mut Cursor<'_>, budget: &mut usize) -> Result<u32,Error> {
    let kind = input.count(4)?;
    let (w,h) = if kind == 1 { (input.count(3)?,input.count(3)?) }
        else if kind == 0 { (0,0) } else { return Err(Error::PayloadUnavailable("legacy recipe kind")); };
    let n = input.count(9)?;
    if n == 0 || (kind == 1 && (w == 0 || h == 0 || w*h != n)) { return Err(Error::Invalid); }
    let mut cells = Vec::new();
    for _ in 0..n { cells.push(display(input,0,budget)?); }
    let result = display(input,0,budget)?; let station = display(input,0,budget)?;
    if station != [(TABLE,1)] { return Err(Error::PayloadUnavailable("legacy recipe station")); }
    let old = match (kind,w,h,result.as_slice()) {
        (1,2,2,[(TABLE,1)]) if cells.iter().all(|c| plank_cell(c)) => 360,
        (1,1,2,[(STICK,4)]) if cells.iter().all(|c| plank_cell(c)) => 60,
        (0,0,0,[(36,4)]) if cells.len()==1
            && cells[0].iter().all(|(id,n)| *n==1 && [134,148,159,171].contains(id))
            && [134,171].iter().all(|id| cells[0].contains(&(*id,1))) => 163,
        _ => return Err(Error::PayloadUnavailable("legacy recipe shape")),
    };
    input.uint()?; input.count(12)?; // group/category: старые категории заданы самим клиентом.
    if input.boolean()? {
        let count = input.count(9)?;
        if count != cells.len() { return Err(Error::Invalid); }
        for cell in cells {
            let size = input.count(257)?;
            if size == 0 { return Err(Error::PayloadUnavailable("legacy tagged recipe requirement")); }
            let mut ids = BTreeSet::new();
            for _ in 1..size { if !ids.insert(input.uint()?) { return Err(Error::Invalid); } }
            if ids != cell.iter().map(|(id,_)| *id).collect() { return Err(Error::Invalid); }
        }
    }
    Ok(old)
}
impl Book {
    fn packet(&self, action: u8, recipes: &[u32], highlights: &[u32]) -> Vec<u8> {
        let mut out = vec![0x31,action,u8::from(self.settings[0]),u8::from(self.settings[1])];
        uint(&mut out,recipes.len() as u32); for id in recipes { uint(&mut out,*id); }
        if action == 0 { uint(&mut out,highlights.len() as u32); for id in highlights { uint(&mut out,*id); } }
        out
    }
    fn snapshot(&self) -> Vec<u8> {
        self.packet(0,&self.known.values().map(|r|r.0).collect::<Vec<_>>(),
            &self.known.values().filter(|r|r.1).map(|r|r.0).collect::<Vec<_>>())
    }
    pub fn clientbound(&mut self, id: u32, bytes: &[u8]) -> Result<Vec<u8>,Error> {
        if bytes.len() > 1_048_576 { return Err(Error::TooLarge); }
        let mut next = self.clone(); let mut input = Cursor::new(bytes); let mut budget = 16384;
        let out = match id {
            0x4c => {
                next.settings = [input.boolean()?,input.boolean()?];
                // У340 книги печи/коптильни/плавильни отсутствуют.
                for _ in 0..6 { input.boolean()?; }
                next.snapshot()
            }
            0x4a => {
                let count = input.count(4096)?; let mut added = BTreeMap::new();
                for _ in 0..count {
                    let native = input.uint()?; let old = recipe(&mut input,&mut budget)?;
                    let flags = input.take(1)?[0];
                    if flags & !3 != 0 { return Err(Error::Invalid); }
                    if !matches!(flags,0|3) { return Err(Error::PayloadUnavailable("legacy recipe notification flags")); }
                    if added.insert(native,(old,flags & 2 != 0)).is_some() { return Err(Error::Invalid); }
                }
                let replace = input.boolean()?;
                if replace { next.known.clear(); }
                for (native,value) in &added {
                    if next.known.iter().any(|(key,row)| key != native && row.0 == value.0) { return Err(Error::PayloadUnavailable("ambiguous legacy recipe")); }
                    next.known.insert(*native,*value);
                }
                if next.known.len() > 4096 { return Err(Error::TooLarge); }
                if replace || added.values().any(|row| !row.1) { next.snapshot() }
                else { next.packet(1,&added.values().map(|row|row.0).collect::<Vec<_>>(),&[]) }
            }
            0x4b => {
                let count = input.count(4096)?; let mut removed = Vec::new(); let mut seen = BTreeSet::new();
                for _ in 0..count {
                    let native = input.uint()?; if !seen.insert(native) { return Err(Error::Invalid); }
                    if let Some((old,_)) = next.known.remove(&native) { removed.push(old); }
                }
                next.packet(2,&removed,&[])
            }
            _ => return Err(Error::Invalid),
        };
        if input.remaining() != 0 { return Err(Error::Invalid); }
        *self = next; Ok(out)
    }
    pub fn serverbound(&mut self, id: u32, bytes: &[u8]) -> Result<Vec<u8>,Error> {
        let mut input = Cursor::new(bytes); let mut next = self.clone(); let mut out = Vec::new();
        let lookup = |old| self.known.iter().find(|(_,row)| row.0 == old).map(|(key,_)| *key)
            .ok_or(Error::PayloadUnavailable("unknown legacy recipe request"));
        match id {
            0x12 => {
                let window = input.take(1)?[0]; if window > 127 { return Err(Error::Invalid); }
                out.extend([0x27,window]); uint(&mut out,lookup(input.uint()?)?); out.push(u8::from(input.boolean()?));
            }
            0x17 => match input.uint()? {
                0 => {
                    let old = u32::from_be_bytes(input.take(4)?.try_into().map_err(|_|Error::Invalid)?);
                    let native = lookup(old)?; next.known.get_mut(&native).ok_or(Error::Invalid)?.1 = false;
                    out.push(0x2f); uint(&mut out,native);
                }
                1 => {
                    next.settings = [input.boolean()?,input.boolean()?];
                    out.extend([0x2e,0,u8::from(next.settings[0]),u8::from(next.settings[1])]);
                }
                _ => return Err(Error::Invalid),
            },
            _ => return Err(Error::Invalid),
        }
        if input.remaining() != 0 { return Err(Error::Invalid); } *self = next; Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    pub(crate) fn table(id: u32, highlight: bool, replace: bool) -> Vec<u8> {
        let mut out = vec![1]; uint(&mut out,id); out.extend([1,2,2,4]);
        for _ in 0..4 { out.extend([10,PLANKS.len() as u8]); for id in PLANKS { out.push(4); uint(&mut out,*id); } }
        out.push(5); uint(&mut out,TABLE); out.extend([1,0,0,4]); uint(&mut out,TABLE);
        out.extend([0,3,1,4]);
        for _ in 0..4 { uint(&mut out,PLANKS.len() as u32+1); for id in PLANKS { uint(&mut out,*id); } }
        out.extend([if highlight {3} else {0},u8::from(replace)]); out
    }
    #[test]
    fn observed_recipe_number_is_not_the_server_display_number() {
        let source: serde_json::Value = serde_json::from_str(include_str!("../testdata/legacy-recipes.json")).unwrap();
        assert_eq!(source["recipe_ids"]["crafting_table"],360);
        let mut book = Book::default();
        assert_eq!(book.clientbound(0x4c,&[0;8]).unwrap(),[0x31,0,0,0,0,0]);
        let result = book.clientbound(0x4a,&table(777,true,false)).unwrap();
        let observed = source["observations"].as_array().unwrap().iter().find(|r|r["action"]==1 && r["recipes"]==serde_json::json!([360])).unwrap();
        let expected: Vec<_> = observed["payload"].as_str().unwrap().as_bytes().as_chunks::<2>().0.iter().map(|s|u8::from_str_radix(std::str::from_utf8(s).unwrap(),16).unwrap()).collect();
        assert_eq!(result[1..],expected);
        let mut request = vec![0]; uint(&mut request,360); request.push(1);
        let mut expected = vec![0x27,0]; uint(&mut expected,777); expected.push(1);
        assert_eq!(book.serverbound(0x12,&request).unwrap(),expected);
        let mut seen = vec![0]; seen.extend(360u32.to_be_bytes());
        assert_eq!(book.serverbound(0x17,&seen).unwrap(),vec![0x2f,0x89,0x06]);
        assert_eq!(book.serverbound(0x17,&[1,1,0]).unwrap(),[0x2e,0,1,0]);
        let mut remove = vec![1]; uint(&mut remove,777); book.clientbound(0x4b,&remove).unwrap();
        assert!(book.serverbound(0x12,&request).is_err());
    }
    #[test]
    fn malformed_recipe_never_changes_existing_book_or_accepts_ambiguous_ids() {
        let mut book = Book::default(); book.clientbound(0x4a,&table(42,false,true)).unwrap();
        let before = book.snapshot();
        for n in 0..table(43,false,false).len() { assert!(book.clientbound(0x4a,&table(43,false,false)[..n]).is_err()); assert_eq!(book.snapshot(),before); }
        assert!(book.clientbound(0x4a,&table(43,false,false)).is_err());
        let mut bad = table(44,false,true); bad[2]=4; assert!(book.clientbound(0x4a,&bad).is_err()); assert_eq!(book.snapshot(),before);
    }
}
