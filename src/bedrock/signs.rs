// Bedrock: Open Sign (303), Block Actor Data (56), minecraft-data 1.26.10.
// Wiki, Bedrock block entity format: FrontText/BackText, Text, IsWaxed.
// [проверить] поля цвета/свечения живым клиентом.
use super::codec::{In,Out};
use crate::{sign,world::World,snbt::Value};
pub fn editor(pos:(i32,i32,i32),front:bool)->Vec<u8> {
    let mut out=Out::packet(303);out.block_position(pos.0,pos.1,pos.2).bool(front);out.bytes
}
pub fn actor(world:&World,(x,y,z):(i32,i32,i32))->Option<Vec<u8>> {
    let state=world.get_block(x,y,z);if !crate::redstone::is_sign(state){return None;}
    let data=world.sign(x,y,z).cloned().unwrap_or_default();
    let mut out=Out::packet(super::session::BLOCK_ACTOR_DATA);out.block_position(x,y,z).u8(10).string("");
    let id=if crate::blocks::block_at_state(state).is_some_and(|s|s.contains("hanging_sign")){"HangingSign"}else{"Sign"};
    out.u8(8).string("id").string(id);
    for (key,v) in [("x",x),("y",y),("z",z)] {out.u8(3).string(key).zigzag32(v);}
    out.u8(1).string("IsWaxed").bool(data.waxed);
    const COLORS:[u32;16]=[0xfff9fffe,0xfff9801d,0xffc74ebd,0xff3ab3da,0xfffed83d,0xff80c71f,0xfff38baa,0xff474f52,0xff9d9d97,0xff169c9c,0xff8932b8,0xff3c44aa,0xff835432,0xff5e7c16,0xffb02e26,0xff1d1d21];
    for (key,side) in [("FrontText",data.front),("BackText",data.back)] {
        out.u8(10).string(key).u8(8).string("Text").string(&side.lines.join("\n"));
        out.u8(3).string("SignTextColor").zigzag32(COLORS[usize::from(side.color.min(15))] as i32);
        out.u8(1).string("IgnoreLighting").bool(side.glowing).u8(0);
    }
    out.u8(0);Some(out.bytes)
}
pub fn front(world:&World,pos:(i32,i32,i32),player:(f32,f32,f32))->bool {
    let state=world.get_block(pos.0,pos.1,pos.2);
    let yaw=crate::blocks::orientation_of(state).and_then(|o|sign::front_yaw(o.value(state,"rotation"),o.value(state,"facing"))).unwrap_or(0.0);
    sign::is_front(yaw,(pos.0 as f64+0.5,pos.2 as f64+0.5),(player.0 as f64,player.2 as f64))
}
// Ограниченный сетевой NBT: клиент меняет только текст, остальные поля
// никогда не копируются в состояние сервера.
pub fn read(reader:&mut In<'_>)->Option<Value> {
    if reader.u8()?!=10{return None;} reader.string()?;
    value(reader,10,0,&mut 4096)
}
fn value(r:&mut In<'_>,kind:u8,depth:u8,budget:&mut usize)->Option<Value> {
    if depth>16||*budget==0{return None;} *budget-=1;
    Some(match kind {
        1=>Value::Num(f64::from(r.u8()? as i8)),
        2=>Value::Num(f64::from(i16::from_le_bytes(r.take(2)?.try_into().ok()?))),
        3=>Value::Num(f64::from(r.zigzag32()?)),4=>Value::Num(r.zigzag64()? as f64),
        5=>Value::Num(f64::from(r.lf32()?)),6=>Value::Num(f64::from_le_bytes(r.take(8)?.try_into().ok()?)),
        8=>Value::Str(r.string()?),
        9=>{let k=r.u8()?;let n=r.zigzag32()?;if n<0||n as usize>*budget{return None;}let mut list=Vec::new();for _ in 0..n{list.push(value(r,k,depth+1,budget)?);}Value::List(list)},
        10=>{let mut fields=Vec::new();loop{let k=r.u8()?;if k==0{break;}let name=r.string()?;fields.push((name,value(r,k,depth+1,budget)?));}Value::Map(fields)},
        _=>return None,
    })
}
pub fn lines(value:&Value,front:bool)->Option<[String;4]> {
    let text=value.get(if front {"FrontText"} else {"BackText"})?.get("Text")?.as_str()?;
    let lines:Vec<_>=text.split('\n').collect();if lines.len()>4||lines.iter().any(|s|s.chars().count()>sign::MAX_LINE){return None;}
    Some(std::array::from_fn(|i|lines.get(i).unwrap_or(&"").chars().filter(|c|!c.is_control()).collect()))
}
#[cfg(test)] mod tests {
    use super::*;
    #[test] fn text_update_never_accepts_extra_lines() {
        let v=Value::Map(vec![("FrontText".into(),Value::Map(vec![("Text".into(),Value::Str("one\ntwo".into()))]))]);
        assert_eq!(lines(&v,true).unwrap()[0],"one");assert!(lines(&v,false).is_none());
        let v=Value::Map(vec![("FrontText".into(),Value::Map(vec![("Text".into(),Value::Str("a\nb\nc\nd\ne".into()))]))]);assert!(lines(&v,true).is_none());
    }
}
