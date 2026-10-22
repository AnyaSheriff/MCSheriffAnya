// Прямые справочники из тех же неизменяемых таблиц состояний.
use super::{BLOCK_NAMES, Orientation, orientation as by_name};
use std::sync::OnceLock;

struct State {
    name: Option<&'static str>,
    orientation: Option<&'static Orientation>,
}

fn table() -> &'static [State] {
    static TABLE: OnceLock<Vec<State>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let len = BLOCK_NAMES.last().map_or(0, |(_, high, _)| *high as usize + 1);
        let mut states: Vec<_> = (0..len).map(|_| State { name: None, orientation: None }).collect();
        for &(low, high, name) in BLOCK_NAMES {
            let orientation = by_name(name);
            for state in &mut states[low as usize..=high as usize] {
                *state = State { name: Some(name), orientation };
            }
        }
        states
    })
}

pub(super) fn name(state: i32) -> Option<&'static str> {
    match table().get(state as usize) {
        Some(entry) => entry.name,
        // Тополь 26.3 лежит за таблицей 775 (src/poplar.rs).
        #[cfg(not(seedmap_core))]
        None => crate::poplar::row_of_state(state).map(|row| row.name),
        #[cfg(seedmap_core)]
        None => None,
    }
}

pub(super) fn orientation(state: i32) -> Option<&'static Orientation> {
    match table().get(state as usize) {
        Some(entry) => entry.orientation,
        #[cfg(not(seedmap_core))]
        None => crate::poplar::orientation_of(state),
        #[cfg(seedmap_core)]
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_tables_match_binary_search_for_every_state() {
        // Тополь (за таблицей 775) проверяется в poplar::tests.
        let poplar = crate::gameplay_catalog::POPLAR[0].first_state..=crate::gameplay_catalog::POPLAR[crate::gameplay_catalog::POPLAR.len() - 1].last_state;
        for state in (-2..=i32::from(u16::MAX) + 1).chain([i32::MIN, i32::MAX]).filter(|state| !poplar.contains(state)) {
            let old = BLOCK_NAMES.partition_point(|(low, _, _)| *low <= state)
                .checked_sub(1).and_then(|index| {
                    let (low, high, name) = BLOCK_NAMES[index];
                    (state >= low && state <= high).then_some(name)
                });
            assert_eq!(name(state), old, "{state}");
            assert_eq!(orientation(state).map(std::ptr::from_ref),
                old.and_then(super::super::orientation).map(std::ptr::from_ref), "{state}");
        }
    }
}
