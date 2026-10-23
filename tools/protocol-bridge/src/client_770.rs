//! Клиентский маршрут770 через общие typed codec772/774/776 и отдельные таблицы770.
//! Злость переводится по часам снимка моба; неизвестные формы закрыты.
//! Источник: client-chain-sources.json; равные Slot envelopes не заменяют codec.
use crate::{Direction, JavaVersion, wire::{Context, Error}};

pub fn configuration(body: &[u8]) -> Result<std::borrow::Cow<'_, [u8]>, Error> {
    crate::wire::translate_inner(crate::State::Configuration,Direction::Clientbound,crate::wire::INTERNAL,JavaVersion::V1_21_5,body,Context::default(),true)
}
pub fn clientbound<'a>(tracker: &mut crate::entities::Tracker, body: &'a [u8], context: Context) -> Result<std::borrow::Cow<'a,[u8]>,Error> {
    tracker.translate_inner(crate::wire::INTERNAL,JavaVersion::V1_21_5,body,context,true)
}
pub fn serverbound(body: &[u8],context: Context) -> Result<std::borrow::Cow<'_,[u8]>,Error> {
    crate::wire::translate_inner(crate::State::Play,Direction::Serverbound,JavaVersion::V1_21_5,crate::wire::INTERNAL,body,context,true)
}


/// Отдельный соседний Play-шаг770↔771; содержимое разбирает общий typed codec.
pub fn translate_play(direction:Direction,from:JavaVersion,body:&[u8],context:Context)->Result<Vec<u8>,Error>{
    let to=match from {JavaVersion::V1_21_5=>JavaVersion::V1_21_6,JavaVersion::V1_21_6=>JavaVersion::V1_21_5,_=>return Err(Error::UnsupportedVersion(from))};
    crate::client_773::translate_selected(direction,from,to,body,context)
}

#[cfg(test)]
#[path="client_770_tests.rs"]
mod tests;

/// Дерево собственных команд ядра: integer/string, без непрочитанных parser ID.
pub(crate) fn commands(input:&mut crate::wire::Cursor<'_>,out:&mut Vec<u8>)->Result<(),Error>{
    use crate::wire::uint;
    let count=input.count(65536)?;if count==0{return Err(Error::Invalid);}uint(out,count as u32);
    for _ in 0..count {
        let flags=input.take(1)?[0];if flags & !31!=0 || flags&3==3{return Err(Error::Invalid);}out.push(flags);
        let children=input.count(count)?;uint(out,children as u32);
        for _ in 0..children {uint(out,input.count(count-1)? as u32);}
        if flags&8!=0 {uint(out,input.count(count-1)? as u32);}
        if flags&3!=0 {let name=input.string()?;uint(out,name.len() as u32);out.extend(name.as_bytes());}
        if flags&3==2 {
            let parser=input.uint()?;uint(out,parser);
            match parser {
                3=>{let mask=input.take(1)?[0];if mask>3{return Err(Error::Invalid);}out.push(mask);if mask&1!=0 {out.extend(input.take(4)?);}
                    if mask&2!=0 {out.extend(input.take(4)?);}}
                5=>uint(out,input.count(2)? as u32),
                _=>return Err(Error::PayloadUnavailable("770 command parser outside core writer")),
            }
            if flags&16!=0 {let name=input.string()?;uint(out,name.len() as u32);out.extend(name.as_bytes());}
        }else if flags&16!=0 {return Err(Error::Invalid);}
    }
    uint(out,input.count(count-1)? as u32);Ok(())
}
