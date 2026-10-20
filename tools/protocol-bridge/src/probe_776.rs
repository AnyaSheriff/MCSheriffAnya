//! Явная ограниченная проба клиента776 на внутреннем775. Не public admission.
//! Feature открывает только этот API; обычные wire/Tracker/packet-ID guards
//! остаются закрыты. Core вызывает API лишь из cfg(test) сессии.
use std::borrow::Cow;
use crate::{Direction, JavaVersion, State, entities::Tracker, wire::{self, Context, Error}};

pub fn configuration(body: &[u8]) -> Result<Cow<'_, [u8]>, Error> {
    wire::translate_inner(State::Configuration, Direction::Clientbound,
        JavaVersion::V26_1_2, JavaVersion::V26_2, body, Context::default(), true)
}

pub fn clientbound<'a>(tracker: &mut Tracker, body: &'a [u8], context: Context) -> Result<Cow<'a, [u8]>, Error> {
    tracker.translate_inner(JavaVersion::V26_1_2, JavaVersion::V26_2, body, context, true)
}

pub fn serverbound(body: &[u8], context: Context) -> Result<Cow<'_, [u8]>, Error> {
    wire::translate_inner(State::Play, Direction::Serverbound,
        JavaVersion::V26_2, JavaVersion::V26_1_2, body, context, true)
}

#[cfg(test)]
#[path = "probe_776_tests.rs"]
mod tests;
