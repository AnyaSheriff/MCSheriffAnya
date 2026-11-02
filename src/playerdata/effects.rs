//! Ограниченный хвост эффектов собственного MCPD13; wiki Effect: «hidden effect».
use crate::survival::{Effect, EffectKind, INFINITE};
use std::io;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SavedEffects {
    pub active: Vec<Effect>,
    pub hidden: Vec<Effect>,
    pub absorption: f32,
}

// 40 видов, 256 уровней: remember_effect хранит не более одного слоя уровня.
const MAX_ACTIVE: usize = EffectKind::ALL.len();
const MAX_HIDDEN: usize = MAX_ACTIVE * 255;
const RECORD_SIZE: usize = 10; // kind:u8, amplifier:i32, ticks:i32, flags:u8
const HEADER_SIZE: usize = 12; // absorption:f32, active:u32, hidden:u32
const MAX_PAYLOAD: usize = HEADER_SIZE + (MAX_ACTIVE + MAX_HIDDEN) * RECORD_SIZE;

impl SavedEffects {
    pub fn needs_save(&self, current: &Self) -> bool {
        fn changed(old: &[Effect], new: &[Effect]) -> bool {
            old.len() != new.len() || old.iter().zip(new).any(|(a, b)| {
                Effect { ticks_left: b.ticks_left, ..*a } != *b
                    || (a.ticks_left == INFINITE) != (b.ticks_left == INFINITE)
                    || a.ticks_left.abs_diff(b.ticks_left) >= 100
            })
        }
        self.absorption != current.absorption || changed(&self.active, &current.active)
            || changed(&self.hidden, &current.hidden)
    }

    fn valid(&self) -> bool {
        if !self.absorption.is_finite() || self.absorption < 0.0
            || self.active.len() > MAX_ACTIVE || self.hidden.len() > MAX_HIDDEN { return false; }
        let valid = |effect: &Effect| (0..=255).contains(&effect.amplifier)
            && (effect.ticks_left >= 0 || effect.ticks_left == INFINITE);
        let mut active = [None; MAX_ACTIVE];
        for effect in &self.active {
            let slot = &mut active[effect.kind.id() as usize];
            if !valid(effect) || slot.is_some() { return false; }
            *slot = Some(effect);
        }
        let mut levels = [[false; 256]; MAX_ACTIVE];
        for effect in &self.hidden {
            if !valid(effect) || effect.kind.instant() || effect.ticks_left == 0 { return false; }
            let Some(top) = active[effect.kind.id() as usize] else { return false; };
            if effect.amplifier >= top.amplifier { return false; }
            let level = &mut levels[effect.kind.id() as usize][effect.amplifier as usize];
            if *level { return false; }
            *level = true;
        }
        true
    }
}

pub(super) fn append(saved: &SavedEffects, out: &mut Vec<u8>) -> io::Result<()> {
    if !saved.valid() { return Err(io::Error::new(io::ErrorKind::InvalidData, "неверный снимок эффектов игрока")); }
    let start = out.len();
    out.extend_from_slice(&saved.absorption.to_be_bytes());
    out.extend_from_slice(&(saved.active.len() as u32).to_be_bytes());
    out.extend_from_slice(&(saved.hidden.len() as u32).to_be_bytes());
    for effect in saved.active.iter().chain(&saved.hidden) {
        out.push(effect.kind.id() as u8);
        out.extend_from_slice(&effect.amplifier.to_be_bytes());
        out.extend_from_slice(&effect.ticks_left.to_be_bytes());
        out.push(u8::from(effect.ambient) | (u8::from(effect.visible) << 1) | (u8::from(effect.show_icon) << 2));
    }
    let length = out.len() - start;
    out.extend_from_slice(&(length as u32).to_be_bytes());
    Ok(())
}

pub(super) fn take(tail: &mut &[u8]) -> Option<SavedEffects> {
    let end = tail.len().checked_sub(4)?;
    let length = u32::from_be_bytes(tail.get(end..)?.try_into().ok()?) as usize;
    if !(HEADER_SIZE..=MAX_PAYLOAD).contains(&length) { return None; }
    let start = end.checked_sub(length)?;
    let payload = tail.get(start..end)?;
    let active = u32::from_be_bytes(payload.get(4..8)?.try_into().ok()?) as usize;
    let hidden = u32::from_be_bytes(payload.get(8..12)?.try_into().ok()?) as usize;
    if active > MAX_ACTIVE || hidden > MAX_HIDDEN || length != HEADER_SIZE + (active + hidden) * RECORD_SIZE { return None; }
    let mut saved = SavedEffects { absorption: f32::from_be_bytes(payload.get(..4)?.try_into().ok()?),
        active: Vec::with_capacity(active), hidden: Vec::with_capacity(hidden) };
    for (index, record) in payload[HEADER_SIZE..].chunks_exact(RECORD_SIZE).enumerate() {
        if record[9] & !7 != 0 { return None; }
        let effect = Effect { kind: EffectKind::from_id(i32::from(record[0]))?,
            amplifier: i32::from_be_bytes(record[1..5].try_into().ok()?),
            ticks_left: i32::from_be_bytes(record[5..9].try_into().ok()?),
            ambient: record[9] & 1 != 0, visible: record[9] & 2 != 0, show_icon: record[9] & 4 != 0 };
        if index < active { saved.active.push(effect); } else { saved.hidden.push(effect); }
    }
    if !saved.valid() { return None; }
    *tail = &tail[..start];
    Some(saved)
}
