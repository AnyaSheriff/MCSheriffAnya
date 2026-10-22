//! В777 абсолютная позиция больше не несёт скорость: нужны два пакета.
//! Схема — pinned protocol.json; упаковка скорости перенесена из нашего types.rs.
use crate::{JavaVersion,ids::{self,RegistryKind},wire::{Cursor,Error,uint}};

/// Та же ограниченная точность LPVec3, которую использует родной формат775.
pub fn push_velocity(out:&mut Vec<u8>, x:f64, y:f64, z:f64) {
    let largest=x.abs().max(y.abs()).max(z.abs());
    if !largest.is_finite() || largest<1.0/32766.0 {out.push(0);return;}
    let scale=largest.ceil();let continued=scale>3.0;
    let mut packed=u64::from(scale as u32&3)|if continued{4}else{0};
    for (index,value) in [x,y,z].into_iter().enumerate(){
        let shifted=((value/scale*0.5+0.5)*32766.0).round() as u64;
        packed|=(shifted&0x7fff)<<(3+15*index as u32);
    }
    out.push(packed as u8);out.push((packed>>8) as u8);out.extend(((packed>>16) as u32).to_be_bytes());
    if continued{uint(out,((scale as i32)>>2) as u32);}
}

/// Один775 sync нельзя заменить только777 position: клиент продолжит старую
/// скорость. Возвращаем позицию и обязательный motion, в том числе нулевой.
pub(crate) fn upgrade(body:&[u8])->Result<Vec<Vec<u8>>,Error> {
    let from=JavaVersion::V26_1_2;let to=JavaVersion::V26_3;
    let mut input=Cursor::new(body);if input.uint()?!=0x23{return Err(Error::Invalid);}
    let entity=input.uint()?;let mut position=Vec::new();
    uint(&mut position,ids::translate(RegistryKind::PlayClientbound,from,to,0x23)?);
    uint(&mut position,entity);uint(&mut position,0); // linear
    for _ in 0..3 {
        let bytes=input.take(8)?;if !f64::from_be_bytes(bytes.try_into().map_err(|_|Error::Invalid)?).is_finite(){return Err(Error::Invalid);}position.extend(bytes);
    }
    let mut velocity=[0.0;3];
    for value in &mut velocity {
        *value=f64::from_be_bytes(input.take(8)?.try_into().map_err(|_|Error::Invalid)?);
        if !value.is_finite() || value.abs()>i32::MAX as f64{return Err(Error::Invalid);}
    }
    for _ in 0..2 {
        let bytes=input.take(4)?;if !f32::from_be_bytes(bytes.try_into().map_err(|_|Error::Invalid)?).is_finite(){return Err(Error::Invalid);}position.extend(bytes);
    }
    position.push(u8::from(input.boolean()?));if !input.rest.is_empty(){return Err(Error::Invalid);}
    let mut motion=Vec::new();uint(&mut motion,ids::translate(RegistryKind::PlayClientbound,from,to,0x65)?);uint(&mut motion,entity);
    push_velocity(&mut motion,velocity[0],velocity[1],velocity[2]);
    Ok(vec![position,motion])
}

#[cfg(test)]
mod tests {
    use super::*;
    fn body(speed:[f64;3])->Vec<u8>{
        let mut body=vec![0x23,7];for v in [10.0,64.0,-2.0].into_iter().chain(speed){body.extend(v.to_be_bytes());}
        body.extend(90f32.to_be_bytes());body.extend((-15f32).to_be_bytes());body.push(1);body
    }
    #[test]
    fn position_and_speed_are_both_sent_even_when_stopping() {
        let moving=upgrade(&body([0.5,-0.25,0.0])).unwrap();let stopped=upgrade(&body([0.0;3])).unwrap();
        assert_eq!(moving.len(),2);assert_eq!(moving[0],stopped[0]);assert_eq!(&moving[0][1..3],&[7,0]);
        assert_eq!(&stopped[1][1..],&[7,0]);assert!(moving[1].len()>stopped[1].len());
        let packet=body([0.0;3]);for n in 0..packet.len(){assert!(upgrade(&packet[..n]).is_err());}
        assert!(upgrade(&body([f64::NAN,0.0,0.0])).is_err());
    }
}
