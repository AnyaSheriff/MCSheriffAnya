// Мешки всех цветов. Minecraft Wiki, Bundle: «64 items», вложенный
// мешок занимает 4 единицы плюс содержимое. [проверить] выбор прокруткой.
use crate::{blocks, inventory::{Stack, remainder}, stackdata::{Content, Data}};
pub fn is_bundle(item: i32) -> bool { blocks::item_name(item).is_some_and(|n|n=="bundle"||n.ends_with("_bundle")) }
fn stack(c: Content) -> Stack { let mut s=Stack::from_stored(c.item,c.count); s.data=c.data; s }
fn content(s: Stack) -> Content { Content {slot:0,item:s.stored_item(),count:s.count,data:s.data} }
pub fn weight(data: &Data) -> i32 { data.bundle.iter().map(|c|unit_weight(stack(*c)).saturating_mul(c.count)).fold(0,i32::saturating_add) }
fn unit_weight(s: Stack) -> i32 {
    if is_bundle(s.item) { return 4+weight(&s.extras()); }
    if !s.extras().bees.is_empty() { return 64; }
    64/s.limit().max(1)
}
pub fn insert(bundle: Stack, incoming: Stack) -> (Stack, Option<Stack>) {
    if !is_bundle(bundle.item) || bundle.count!=1 || crate::containers::is_shulker_item(incoming.item) { return (bundle,Some(incoming)); }
    let mut data=bundle.extras();
    let moved=((64-weight(&data)).max(0)/unit_weight(incoming).max(1)).min(incoming.count);
    if moved<=0 { return (bundle,Some(incoming)); }
    if let Some(i)=data.bundle.iter().position(|c|stack(*c).stacks_with(&incoming)) {
        let old=data.bundle.remove(i); data.bundle.insert(0,content(incoming.with_count(old.count+moved)));
    } else { data.bundle.insert(0,content(incoming.with_count(moved))); }
    data.bundle_selected=None;
    (bundle.with_extras(data),remainder(incoming,incoming.count-moved))
}
pub fn extract(bundle: Stack) -> (Stack, Option<Stack>) {
    let mut data=bundle.extras();
    if data.bundle.is_empty() { return (bundle,None); }
    let index=data.bundle_selected.map_or(0,usize::from).min(data.bundle.len()-1);
    let taken=stack(data.bundle.remove(index)); data.bundle_selected=None;
    (bundle.with_extras(data),Some(taken))
}
pub fn select(bundle: Stack, index: i32) -> Stack {
    let mut data=bundle.extras();
    if index == -1 { data.bundle_selected=None; }
    else if index>=0 && (index as usize)<data.bundle.len() { data.bundle_selected=Some(index as u8); }
    bundle.with_extras(data)
}
// Щелчки в окне (вики, Bundle; чёрный ящик bundle-chest-select): левая кнопка — мешок на курсоре
// забирает предмет слота, а предмет на курсоре уходит в мешок слота; правая — пустой курсор
// достаёт из мешка слота, мешок на курсоре кладёт стопку в пустую ячейку. Остальное — обычный
// щелчок (правая с предметом на курсоре над мешком — обмен).
pub fn click(cursor: &mut Option<Stack>, there: &mut Option<Stack>, button: i8) -> bool {
    match button {
        0 => {
            if let (Some(b),Some(s))=(cursor.filter(|s|is_bundle(s.item)),*there) {
                let (next,rest)=insert(b,s);
                if rest!=Some(s) { *cursor=Some(next); *there=rest; return true; }
            } else if let (Some(b),Some(s))=(there.filter(|s|is_bundle(s.item)),*cursor) {
                let (next,rest)=insert(b,s);
                if rest!=Some(s) { *there=Some(next); *cursor=rest; return true; }
            }
            false
        }
        1 => {
            if let (Some(b),None)=(there.filter(|s|is_bundle(s.item)),*cursor) {
                let (next,taken)=extract(b);
                if taken.is_some() { *there=Some(next); *cursor=taken; return true; }
            } else if let (Some(b),None)=(cursor.filter(|s|is_bundle(s.item)),*there) {
                let (next,taken)=extract(b);
                if taken.is_some() { *cursor=Some(next); *there=taken; return true; }
            }
            false
        }
        _ => false,
    }
}
#[cfg(test)]
pub fn empty(bundle: Stack) -> (Stack, Vec<Stack>) {
    let mut data=bundle.extras(); let items=data.bundle.drain(..).map(stack).collect(); data.bundle_selected=None;
    (bundle.with_extras(data),items)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn item(n:&str,c:i32)->Stack { Stack::new(blocks::item_named(n).unwrap(),c) }
    #[test]
    fn mixed_weight_overflow_selection_and_nested() {
        let (b,left)=insert(item("bundle",1),item("ender_pearl",10)); assert!(left.is_none());
        let (b,left)=insert(b,item("stone",40)); assert_eq!(left.unwrap().count,16); assert_eq!(weight(&b.extras()),64);
        let (b,taken)=extract(select(b,1)); assert_eq!(taken.unwrap(),item("ender_pearl",10));
        let (b,left)=insert(b,item("diamond_sword",1)); assert!(left.is_some());
        let (outer,left)=insert(item("red_bundle",1),b); assert!(left.is_none()); assert_eq!(weight(&outer.extras()),28);
        let (outer,left)=insert(outer,item("shulker_box",1)); assert!(left.is_some());
        let (_,taken)=empty(outer); assert_eq!(taken.len(),1);
    }
    #[test]
    fn contents_survive_disk_and_clicks() {
        let mut cursor=Some(item("bundle",1)); let mut slot=Some(item("stone",64));
        assert!(click(&mut cursor,&mut slot,0)); assert!(slot.is_none());
        let b=cursor.unwrap(); let data=b.extras();
        assert_eq!(crate::stackdata::from_bytes(&crate::stackdata::to_bytes(&data)),Some(data));
        // Правый щелчок мешком на курсоре над занятой ячейкой — обычный обмен, над пустой — стопка из мешка.
        let mut other=Some(item("dirt",3)); assert!(!click(&mut cursor,&mut other,1));
        assert!(click(&mut cursor,&mut slot,1)); assert_eq!(slot.unwrap().count,64);
    }
}
