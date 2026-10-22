//! Частицы по закреплённым JSON и нашим записям обычных команд обоих серверов.
use crate::{JavaVersion, ids::{self, RegistryKind}, items, wire::{Cursor, Error, uint}};

fn floats(input: &mut Cursor<'_>, out: &mut Vec<u8>, count: usize, double: bool) -> Result<(),Error> {
    for _ in 0..count {
        let bytes = input.take(if double {8} else {4})?;
        let finite = if double { f64::from_be_bytes(bytes.try_into().map_err(|_| Error::Invalid)?).is_finite() }
            else { f32::from_be_bytes(bytes.try_into().map_err(|_| Error::Invalid)?).is_finite() };
        if !finite { return Err(Error::Invalid); }
        out.extend(bytes);
    }
    Ok(())
}

pub(crate) fn particle(input: &mut Cursor<'_>, out: &mut Vec<u8>, from: JavaVersion, to: JavaVersion, context: items::Context) -> Result<(),Error> {
    let source = input.uint()?;
    let kind = ids::translate(RegistryKind::Particle,from,JavaVersion::V26_1_2,source)?;
    uint(out,ids::translate(RegistryKind::Particle,from,to,source)?);
    match kind {
        1 | 2 | 29 | 111 | 115 => uint(out,context.content.packet_id(RegistryKind::BlockState,from,to,input.uint()?)?),
        14 | 15 => { out.extend(input.take(if kind == 14 {4} else {8})?); floats(input,out,1,false)?; }
        21 | 36 | 42 => out.extend(input.take(4)?),
        8 | 38 => floats(input,out,1,false)?,
        16 | 46 => { out.extend(input.take(4)?); floats(input,out,1,false)?; }
        47 => {
            let (stack,size) = items::decode_packet_template(from,input.rest,context).map_err(Error::Item)?;
            input.take(size)?;
            out.extend(items::encode_packet_template(to,&stack,context).map_err(Error::Item)?);
        }
        48 => {
            let source = input.count(1)?; uint(out,source as u32);
            if source == 0 { out.extend(input.take(8)?); }
            else { uint(out,input.uint()?); floats(input,out,1,false)?; }
            uint(out,input.uint()?);
        }
        49 => { floats(input,out,3,true)?; out.extend(input.take(4)?); uint(out,input.uint()?); }
        105 => uint(out,input.uint()?),
        // Остальные канонические частицы имеют void payload в pinned JSON.
        0..=116 => (),
        _ => return Err(Error::Invalid),
    }
    Ok(())
}

pub(crate) fn world(input: &mut Cursor<'_>, out: &mut Vec<u8>, from: JavaVersion, to: JavaVersion, context: items::Context) -> Result<(),Error> {
    let modern = from == JavaVersion::V26_3;
    let mut effect = Vec::new();
    if modern { particle(input,&mut effect,from,to,context)?; }
    let mut common = vec![u8::from(input.boolean()?),u8::from(input.boolean()?)];
    floats(input,&mut common,3,true)?; floats(input,&mut common,3,false)?;
    let mut speed = Vec::new(); floats(input,&mut speed,if modern {3} else {1},false)?;
    let count = if modern { input.uint()? } else {
        let count = i32::from_be_bytes(input.take(4)?.try_into().map_err(|_| Error::Invalid)?);
        u32::try_from(count).map_err(|_| Error::Invalid)?
    };
    if modern {
        // Новые распределения и разные скорости по осям не представимы
        // старым пакетом. Не подменяем их похожим, но другим эффектом.
        if input.uint()? != 0 || speed[..4] != speed[4..8] || speed[..4] != speed[8..] {
            return Err(Error::PayloadUnavailable("particle randomization or axis speeds"));
        }
        out.extend(common); out.extend(&speed[..4]); out.extend((count as i32).to_be_bytes()); out.extend(effect);
    } else {
        particle(input,&mut effect,from,to,context)?;
        out.extend(effect); out.extend(common);
        for _ in 0..3 { out.extend(&speed); }
        uint(out,count); uint(out,0);
    }
    Ok(())
}

#[cfg(test)]
#[path = "particle_tests.rs"]
mod tests;
