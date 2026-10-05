// Сидение: кто на ком едет. Общая основа для лодок, вагонеток и (потом)
// лошадей, свиней с седлом и стрейдеров.
//
// Вики (Riding): у сущности-«транспорта» есть список седоков (passengers),
// у седока — не больше одного транспорта (vehicle). Клиентам список уходит
// пакетом Set Passengers (0x6B): номер транспорта и номера седоков по
// порядку — первый и есть «водитель» (у лодки им становится первый севший,
// чёрный ящик boat-ride: Passengers n=1, после выхода n=0).
//
// Здесь только учёт: кто с кем. Движение у каждого транспорта своё (лодка —
// minecart/boat.rs, вагонетка — minecart.rs, моб — mob/), а седок на каждом
// такте переносится в точку транспорта (`seat_place`). Номера — номера
// сущностей: игроки, мобы, лодки, вагонетки живут в одном пространстве.
//
// Как пользоваться мобам (лошадь, свинья, стрейдер):
//
// ```ignore
// let mut riding = shared.riding.lock().unwrap();
// if riding.mount(player.entity_id, horse.entity_id, 1) { /* сел */ }
// riding.dismount(player.entity_id);                 // слез
// riding.passengers(horse.entity_id);                // кто сидит
// riding.vehicle_of(player.entity_id);               // на чём едет
// ```
//
// Подключения сами заметят перемену (`version`) и перешлют Set Passengers
// тем клиентам, что видят транспорт (network/play/vehicle.rs).

use std::collections::HashMap;

/// Где сидит седок относительно низа ящика лодки: чёрный ящик (boat-ride,
/// boat-drive) — игрок на лодке с низом 1,4554 стоит в 1,043, то есть на
/// 0,4124 ниже [проверить для второго седока: у оригинала сидения разнесены
/// по длине лодки на ±0,2].
pub const BOAT_SEAT_DROP: f64 = -0.4124;

/// Учёт седоков всего сервера.
#[derive(Default)]
pub struct Riding {
    /// Транспорт → седоки по порядку посадки.
    seats: HashMap<i32, Vec<i32>>,
    /// Седок → транспорт.
    mounts: HashMap<i32, i32>,
    /// Растёт на каждую перемену: подключениям не надо сравнивать всё подряд.
    version: u64,
}

impl Riding {
    pub fn new() -> Self {
        Self::default()
    }

    /// Счётчик перемен.
    pub fn version(&self) -> u64 {
        self.version
    }

    /// Посадить `passenger` на `vehicle`, где `capacity` мест. Уже севшего
    /// на другой пересаживает. Возвращает, получилось ли (мест нет — нет).
    pub fn mount(&mut self, passenger: i32, vehicle: i32, capacity: usize) -> bool {
        if passenger == vehicle || self.mounts.get(&passenger) == Some(&vehicle) {
            return false;
        }

        if self.seats.get(&vehicle).is_some_and(|seats| seats.len() >= capacity) || self.is_below(vehicle, passenger) {
            return false;
        }

        self.dismount(passenger);
        self.seats.entry(vehicle).or_default().push(passenger);
        self.mounts.insert(passenger, vehicle);
        self.version += 1;

        true
    }

    /// Седок слез (или его сняли). Возвращает транспорт, с которого слез.
    pub fn dismount(&mut self, passenger: i32) -> Option<i32> {
        let vehicle = self.mounts.remove(&passenger)?;

        if let Some(seats) = self.seats.get_mut(&vehicle) {
            seats.retain(|id| *id != passenger);

            if seats.is_empty() {
                self.seats.remove(&vehicle);
            }
        }

        self.version += 1;

        Some(vehicle)
    }

    /// Транспорт пропал: все его седоки слезли. Возвращает их.
    pub fn clear_vehicle(&mut self, vehicle: i32) -> Vec<i32> {
        let riders = self.seats.remove(&vehicle).unwrap_or_default();

        for rider in &riders {
            self.mounts.remove(rider);
        }

        // Пропал и седок, у которого сами сидели (цепочка): он тоже слезает.
        if self.mounts.remove(&vehicle).is_some() {
            for seats in self.seats.values_mut() {
                seats.retain(|id| *id != vehicle);
            }
        }

        if !riders.is_empty() {
            self.version += 1;
        }

        riders
    }

    /// Седоки транспорта по порядку посадки (первый — водитель).
    pub fn passengers(&self, vehicle: i32) -> &[i32] {
        self.seats.get(&vehicle).map_or(&[], Vec::as_slice)
    }

    /// Водитель: первый севший.
    pub fn driver(&self, vehicle: i32) -> Option<i32> {
        self.passengers(vehicle).first().copied()
    }

    /// На чём едет седок.
    pub fn vehicle_of(&self, passenger: i32) -> Option<i32> {
        self.mounts.get(&passenger).copied()
    }

    /// Все транспорты с седоками (для рассылки).
    pub fn vehicles(&self) -> impl Iterator<Item = (i32, &[i32])> {
        self.seats.iter().map(|(vehicle, seats)| (*vehicle, seats.as_slice()))
    }

    /// Не сидит ли `vehicle` (прямо или через цепочку) на `passenger`:
    /// посадка по кругу запрещена.
    fn is_below(&self, vehicle: i32, passenger: i32) -> bool {
        let mut at = vehicle;

        for _ in 0..16 {
            match self.mounts.get(&at) {
                Some(&next) if next == passenger => return true,
                Some(&next) => at = next,
                None => return false,
            }
        }

        true
    }
}

/// Куда ставить седока на лодке: середина по x, z и низ ящика плюс просадка.
pub fn boat_seat(place: (f64, f64, f64)) -> (f64, f64, f64) {
    (place.0, place.1 + BOAT_SEAT_DROP, place.2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_seats_in_a_boat() {
        let mut riding = Riding::new();

        assert!(riding.mount(1, 100, 2));
        assert!(riding.mount(2, 100, 2));
        assert!(!riding.mount(3, 100, 2), "третьему места нет");
        assert_eq!(riding.passengers(100), &[1, 2]);
        assert_eq!(riding.driver(100), Some(1));
        assert_eq!(riding.vehicle_of(2), Some(100));
    }

    #[test]
    fn dismount_frees_the_seat_and_bumps_version() {
        let mut riding = Riding::new();

        riding.mount(1, 100, 2);

        let before = riding.version();

        assert_eq!(riding.dismount(1), Some(100));
        assert_eq!(riding.dismount(1), None);
        assert!(riding.passengers(100).is_empty());
        assert!(riding.version() > before);
    }

    #[test]
    fn remount_moves_the_rider() {
        let mut riding = Riding::new();

        riding.mount(1, 100, 1);
        assert!(riding.mount(1, 200, 1));
        assert!(riding.passengers(100).is_empty());
        assert_eq!(riding.vehicle_of(1), Some(200));
    }

    #[test]
    fn no_riding_in_circles() {
        let mut riding = Riding::new();

        riding.mount(1, 2, 1);
        assert!(!riding.mount(2, 1, 1));
        assert!(!riding.mount(3, 3, 1));
    }

    #[test]
    fn broken_vehicle_drops_everyone() {
        let mut riding = Riding::new();

        riding.mount(1, 100, 2);
        riding.mount(2, 100, 2);

        assert_eq!(riding.clear_vehicle(100), vec![1, 2]);
        assert_eq!(riding.vehicle_of(1), None);
    }
}
