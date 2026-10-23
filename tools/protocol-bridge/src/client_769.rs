//! Клиентский маршрут769 через общие typed codec772/774/776 и отдельные таблицы769.
//! Злость переводится по часам снимка моба; неизвестные формы закрыты.
//! Источник: client-chain-sources.json; равные Slot envelopes не заменяют codec.
use crate::{Direction, JavaVersion, wire::{Context, Error}};

pub fn configuration(body: &[u8]) -> Result<std::borrow::Cow<'_, [u8]>, Error> {
    crate::wire::translate_inner(crate::State::Configuration,Direction::Clientbound,crate::wire::INTERNAL,JavaVersion::V1_21_4,body,Context::default(),true)
}
pub fn clientbound<'a>(tracker: &mut crate::entities::Tracker, body: &'a [u8], context: Context) -> Result<std::borrow::Cow<'a,[u8]>,Error> {
    tracker.translate_inner(crate::wire::INTERNAL,JavaVersion::V1_21_4,body,context,true)
}
pub fn serverbound(body: &[u8],context: Context) -> Result<std::borrow::Cow<'_,[u8]>,Error> {
    crate::wire::translate_inner(crate::State::Play,Direction::Serverbound,JavaVersion::V1_21_4,crate::wire::INTERNAL,body,context,true)
}


/// Отдельный соседний Play-шаг769↔770; содержимое разбирает общий typed codec.
pub fn translate_play(direction:Direction,from:JavaVersion,body:&[u8],context:Context)->Result<Vec<u8>,Error>{
    let to=match from {JavaVersion::V1_21_4=>JavaVersion::V1_21_5,JavaVersion::V1_21_5=>JavaVersion::V1_21_4,_=>return Err(Error::UnsupportedVersion(from))};
    crate::client_773::translate_selected(direction,from,to,body,context)
}




/// Прежние поля inline trim восстановлены только по именам закреплённых предметов.
pub(crate) fn trim(input:&mut crate::items::Reader<'_>,spaces:(crate::content::IdSpace,crate::content::IdSpace))->Result<Vec<u8>,crate::items::Error>{
    use crate::{items::{Error,uint,plain_text},content::IdSpace};
    let old=IdSpace::Wire(JavaVersion::V1_21_4);let mut out=Vec::new();
    for pattern in [false,true] {
        let holder=input.uint()?;uint(&mut out,holder);
        if holder!=0 {continue;}
        let len=input.count(32767)?;let name=std::str::from_utf8(input.take(len)?).map_err(|_|Error::Invalid)?;
        uint(&mut out,len as u32);out.extend(name.as_bytes());
        if spaces.0==old {input.uint()?;}
        if spaces.1==old {
            let stem=name.strip_prefix("minecraft:").unwrap_or(name);
            let item=if pattern {format!("{stem}_armor_trim_smithing_template")}else{match stem {"iron"=>"iron_ingot","gold"=>"gold_ingot","copper"=>"copper_ingot","netherite"=>"netherite_ingot","amethyst"=>"amethyst_shard","lapis"=>"lapis_lazuli","resin"=>"resin_brick",other=>other}.to_string()};
            let id=crate::generated_payload_769::ITEM_NAMES.iter().position(|n|*n==item).ok_or(Error::UnsupportedComponent(56))?;uint(&mut out,id as u32);
        }
        if !pattern {
            let n=input.count(256)?;uint(&mut out,n as u32);
            for _ in 0..n*2 {let n=input.count(32767)?;let value=input.take(n)?;std::str::from_utf8(value).map_err(|_|Error::Invalid)?;uint(&mut out,n as u32);out.extend(value);}
        }
        let nbt=input.nbt()?;plain_text(nbt)?;out.extend(nbt);
        if pattern {out.push(u8::from(input.boolean()?));}
    }
    if spaces.0==old {input.boolean()?;}
    if spaces.1==old {out.push(1);}
    Ok(out)
}

#[cfg(test)]
#[path="client_769_tests.rs"]
mod tests;
