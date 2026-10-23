//! Клиентский маршрут771 через общие typed codec772/774/776 и отдельные таблицы771.
//! Злость переводится по часам снимка моба; неизвестные формы закрыты.
//! Источник: client-chain-sources.json; равные Slot envelopes не заменяют codec.
use crate::{Direction, JavaVersion, wire::{Context, Error}};

pub fn configuration(body: &[u8]) -> Result<std::borrow::Cow<'_, [u8]>, Error> {
    crate::wire::translate_inner(crate::State::Configuration,Direction::Clientbound,crate::wire::INTERNAL,JavaVersion::V1_21_6,body,Context::default(),true)
}
pub fn clientbound<'a>(tracker: &mut crate::entities::Tracker, body: &'a [u8], context: Context) -> Result<std::borrow::Cow<'a,[u8]>,Error> {
    tracker.translate_inner(crate::wire::INTERNAL,JavaVersion::V1_21_6,body,context,true)
}
pub fn serverbound(body: &[u8],context: Context) -> Result<std::borrow::Cow<'_,[u8]>,Error> {
    crate::wire::translate_inner(crate::State::Play,Direction::Serverbound,JavaVersion::V1_21_6,crate::wire::INTERNAL,body,context,true)
}


/// Отдельный соседний Play-шаг771↔772; содержимое разбирает общий typed codec.
pub fn translate_play(direction:Direction,from:JavaVersion,body:&[u8],context:Context)->Result<Vec<u8>,Error>{
    let to=match from {JavaVersion::V1_21_6=>JavaVersion::V1_21_8,JavaVersion::V1_21_8=>JavaVersion::V1_21_6,_=>return Err(Error::UnsupportedVersion(from))};
    crate::client_773::translate_selected(direction,from,to,body,context)
}

#[cfg(test)]
#[path="client_771_tests.rs"]
mod tests;
