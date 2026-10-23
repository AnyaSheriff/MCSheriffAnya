//! Четыре отдельных ограниченных шага старых клиентов. Это не полный Play.
//! [проверить] Предметы/metadata/чанки/реестры и живой checklist каждого основного.
use crate::{Direction, JavaVersion, State, chain, wire};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Step {
    pub older: JavaVersion,
    pub newer: JavaVersion,
}

pub const STEPS: [Step; 4] = [
    Step {older:JavaVersion::V1_21_9, newer:JavaVersion::V1_21_11},
    Step {older:JavaVersion::V1_21_8, newer:JavaVersion::V1_21_9},
    Step {older:JavaVersion::V1_21_6, newer:JavaVersion::V1_21_8},
    Step {older:JavaVersion::V1_21_5, newer:JavaVersion::V1_21_6},
];

impl Step {
    /// Первый Play-шаг773↔774; остальные требуют собственных codec.
    pub fn translate_play(self, direction: Direction, from: JavaVersion, body: &[u8], context: wire::Context) -> Result<Vec<u8>, wire::Error> {
        if self == STEPS[1] {return crate::client_772::translate_play(direction,from,body,context);}
        if self != STEPS[0] { return Err(wire::Error::UnsupportedVersion(self.older)); }
        crate::client_773::translate_play(direction, from, body, context)
    }

    /// Каждый шаг проверяет свою схему и типизированный payload; допуск отдельно.
    pub fn translate_control(self, state: State, direction: Direction, from: JavaVersion, body: &[u8]) -> Result<Vec<u8>, wire::Error> {
        if !STEPS.contains(&self) {return Err(wire::Error::UnsupportedVersion(self.older));}
        let to = if from == self.older {self.newer}
            else if from == self.newer {self.older}
            else {return Err(wire::Error::UnsupportedVersion(from));};
        chain::translate_control(state,direction,from,to,body)
    }
}

#[cfg(test)]
#[path="adjacent_tests.rs"]
mod tests;
