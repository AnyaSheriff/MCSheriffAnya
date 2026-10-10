// Вики, Brush / Suspicious Block / Archaeology: десять взмахов, четыре
// состояния dusted. [проверить] границы тактов и частицы; [Bedrock: потом].
use crate::{blocks,inventory::Stack,loot::{self,Pool},world::World};
use serde::{Serialize,Deserialize};

#[derive(Clone,Debug,Default,PartialEq,Serialize,Deserialize)]
pub struct Data {
    pub table:String,
    pub seed:u64,
    #[serde(skip)] pub strokes:u8,
    #[serde(skip)] pub last:Option<u64>,
    /// Предмет, который клиент видит выступающим из блока (сыпется первым же взмахом).
    #[serde(skip)] pub shown:Option<&'static str>,
    /// Грань, с которой чистят (0..5); сбрасывается, когда пыль осыпалась целиком.
    #[serde(skip)] pub face:Option<u8>,
}
/// Вид блок-сущности «подозрительный блок» (minecraft-data 26.1, `brushable_block`).
pub const BLOCK_ENTITY:i32=41;
/// Сетевой NBT блок-сущности: показанный предмет и грань взмаха (чёрный ящик, brush-sand).
pub fn network_nbt(data:&Data)->Vec<u8> {
    let mut out=vec![0x0A];
    if let Some(item)=data.shown {
        out.extend_from_slice(&[0x0A,0,4]);out.extend_from_slice(b"item");
        out.extend_from_slice(&[0x03,0,5]);out.extend_from_slice(b"count");out.extend_from_slice(&1i32.to_be_bytes());
        let id=format!("minecraft:{item}");
        out.extend_from_slice(&[0x08,0,2]);out.extend_from_slice(b"id");out.extend_from_slice(&(id.len() as u16).to_be_bytes());out.extend_from_slice(id.as_bytes());
        out.push(0x00);
    }
    if let Some(face)=data.face {
        out.extend_from_slice(&[0x03,0,13]);out.extend_from_slice(b"hit_direction");out.extend_from_slice(&i32::from(face).to_be_bytes());
    }
    out.push(0x00);
    out
}
fn announce(world:&mut World,at:(i32,i32,i32)) {
    if let Some(data)=world.with_archaeology(at,|data|data.clone()) {world.note_block_entity(at.0,at.1,at.2,BLOCK_ENTITY,network_nbt(&data));}
}
pub fn suspicious(state:i32)->bool {matches!(blocks::block_at_state(state),Some("suspicious_sand"|"suspicious_gravel"))}
pub fn generated(table:&str,seed:u64)->Data {Data {table:table.into(),seed,..Default::default()}}
pub fn from_snbt(text:&str)->Option<Data> {
    let n=crate::snbt::parse(text)?;
    Some(generated(n.get("LootTable").and_then(|v|v.as_str()).unwrap_or(""),n.get("LootTableSeed").and_then(|v|v.as_f64()).unwrap_or(0.0) as u64))
}
const DESERT_PYRAMID:[Pool;1]=[Pool {rolls:(1,1),empty:0,entries:&[
    ("archer_pottery_sherd",1,1,1),("prize_pottery_sherd",1,1,1),("skull_pottery_sherd",1,1,1),("miner_pottery_sherd",1,1,1),
    ("diamond",1,1,1),("emerald",1,1,1),("tnt",1,1,1),("gunpowder",1,1,1)]}];
const DESERT_WELL:[Pool;1]=[Pool {rolls:(1,1),empty:0,entries:&[
    ("arms_up_pottery_sherd",1,1,2),("brewer_pottery_sherd",1,1,2),("brick",1,1,1),("emerald",1,1,1),("stick",1,1,1),("suspicious_stew",1,1,1)]}];
pub fn pools(table:&str)->&'static [Pool] {
    match table.trim_start_matches("minecraft:").trim_start_matches("archaeology/") {
        "desert_pyramid"=>&DESERT_PYRAMID,"desert_well"=>&DESERT_WELL,
        "ocean_ruin_cold"=>&loot::ARCHAEOLOGY_OCEAN_RUIN_COLD,"ocean_ruin_warm"=>&loot::ARCHAEOLOGY_OCEAN_RUIN_WARM,
        "trail_ruins_common"=>&loot::ARCHAEOLOGY_TRAIL_RUINS_COMMON,"trail_ruins_rare"=>&loot::ARCHAEOLOGY_TRAIL_RUINS_RARE,
        _=>&[],
    }
}
pub fn dusted(strokes:u8)->&'static str {match strokes {0=>"0",1..=2=>"1",3..=5=>"2",_=>"3"}}
fn state(world:&mut World,at:(i32,i32,i32),strokes:u8) {
    let old=world.get_block(at.0,at.1,at.2);
    if let Some(shape)=blocks::orientation_of(old) && let Some(new)=shape.with(old,"dusted",dusted(strokes)) {world.set_block(at.0,at.1,at.2,new);}
}
/// Возвращает добычу только после завершения; пустой поставленный блок
/// также очищается, но предмет из него не появляется.
pub fn stroke(world:&mut World,at:(i32,i32,i32),now:u64,face:u8)->Option<Vec<Stack>> {
    let block=world.get_block(at.0,at.1,at.2);
    if !suspicious(block) {return None;}
    let data=world.with_archaeology(at,|data| {
        if data.last.is_some_and(|last|now<last+10) {return None;}
        data.last=Some(now);data.strokes+=1;data.face=Some(face);
        // Добыча выбирается на первом взмахе и сразу видна (чёрный ящик, brush-sand:
        // данные блока с предметом приходят вместе с каждым новым состоянием).
        if data.shown.is_none() {
            data.shown=loot::roll(pools(&data.table),&mut loot::Dice::new(data.seed)).first().and_then(|stack|blocks::item_name(stack.item));
        }
        Some(data.clone())
    })??;
    if data.strokes<10 {
        state(world,at,data.strokes);
        if dusted(data.strokes)!=dusted(data.strokes-1) {announce(world,at);}
        return None;
    }
    let clean=if blocks::block_at_state(block)==Some("suspicious_sand") {"sand"} else {"gravel"};
    world.set_block(at.0,at.1,at.2,blocks::state_by_name(clean).unwrap());
    Some(loot::roll(pools(&data.table),&mut loot::Dice::new(data.seed)))
}
const RESET:u64=38;
pub fn decay(world:&mut World,now:u64) {
    for at in world.archaeology_places() {
        let changed=world.with_archaeology(at,|data| {
            let last=data.last?;
            // Чёрный ящик, brush-sand: последний взмах на +45, осыпание на +84 и +88
            // (оригинал сбрасывает счёт через 40 тактов от взмаха и дальше каждые 4).
            if data.strokes==0 || now<last+RESET {return None;}
            let old=data.strokes;
            data.strokes=data.strokes.saturating_sub(2);
            if data.strokes==0 {data.face=None;}
            data.last=Some(now.saturating_sub(RESET-4));Some((old,data.strokes))
        }).flatten();
        if let Some((old,strokes))=changed {state(world,at,strokes);if dusted(old)!=dusted(strokes) {announce(world,at);}}
    }
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn brushing_completes_once_and_rejects_fast_packets() {
        let mut world=World::in_memory();let at=(0,70,0);
        world.set_block(0,70,0,blocks::state_by_name("suspicious_sand").unwrap());
        world.with_archaeology(at,|d|*d=generated("desert_pyramid",5));
        for n in 0..9 {assert!(stroke(&mut world,at,n*10,2).is_none());assert!(stroke(&mut world,at,n*10+1,2).is_none());}
        assert_eq!(stroke(&mut world,at,90,2).unwrap().len(),1);
        assert_eq!(blocks::block_at_state(world.get_block(0,70,0)),Some("sand"));
        assert!(stroke(&mut world,at,100,2).is_none());
    }
    #[test] fn block_entity_nbt_carries_item_and_direction() {
        let mut data=Data {shown:Some("tnt"),face:Some(2),..Default::default()};
        let with=network_nbt(&data);
        assert!(with.windows(13).any(|w|w==b"minecraft:tnt"));
        assert!(with.windows(13).any(|w|w==b"hit_direction"));
        data.face=None;
        assert!(!network_nbt(&data).windows(13).any(|w|w==b"hit_direction"));
        assert_eq!(network_nbt(&Data::default()),vec![0x0A,0x00]);
    }
    #[test] fn interrupted_progress_decays_and_all_tables_have_items() {
        let mut world=World::in_memory();let at=(0,70,0);
        world.set_block(0,70,0,blocks::state_by_name("suspicious_gravel").unwrap());
        for t in [0,10,20] {stroke(&mut world,at,t,2);}
        decay(&mut world,60);assert_eq!(world.with_archaeology(at,|d|d.strokes),Some(1));
        decay(&mut world,64);assert_eq!(world.with_archaeology(at,|d|d.strokes),Some(0));
        for table in ["desert_pyramid","desert_well","ocean_ruin_cold","ocean_ruin_warm","trail_ruins_common","trail_ruins_rare"] {
            for p in pools(table) {for &(item,_,_,_) in p.entries {assert!(blocks::item_named(item).is_some(),"{item}");}}
        }
    }
}
