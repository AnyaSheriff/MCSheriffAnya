//! Седоки: общий Riding → SetActorLink (Mojang r/26_u1, protocol 944).
//! Вики, Riding: «passengers»; порядок сохраняется, первый — водитель.
use std::collections::BTreeMap;
use super::codec::Out;

#[derive(Default)]
pub struct Links {
    shown: BTreeMap<(i32, i32), u8>,
}
impl Links {
    /// После RemoveActor/AddActor с прежним номером связи нужно повторить.
    pub fn forget(&mut self, id: i32) { self.shown.retain(|(vehicle, rider), _| *vehicle != id && *rider != id); }

    /// Перестановка мест требует сменить Riding на Passenger и наоборот.
    pub fn update(&mut self, vehicles: &[(i32, Vec<i32>)]) -> Vec<Vec<u8>> {
        let now: BTreeMap<_, _> = vehicles.iter().flat_map(|(vehicle, seats)| seats.iter().enumerate().map(move |(seat, rider)| ((*vehicle, *rider), if seat == 0 { 1 } else { 2 }))).collect();
        let mut packets = Vec::new();
        for (&(vehicle, rider), kind) in &self.shown {
            if now.get(&(vehicle, rider)) != Some(kind) { packets.push(packet(vehicle, rider, 0)); }
        }
        for (&(vehicle, rider), &kind) in &now {
            if self.shown.get(&(vehicle, rider)) != Some(&kind) { packets.push(packet(vehicle, rider, kind)); }
        }
        self.shown = now;
        packets
    }
}
/// Unique IDs (signed zigzag), не runtime IDs. У нас они равны по значению.
pub fn packet(vehicle: i32, rider: i32, kind: u8) -> Vec<u8> {
    let mut out = Out::packet(0x29);
    out.zigzag64(i64::from(vehicle)).zigzag64(i64::from(rider)).u8(kind).bool(false).bool(false).lf32(0.0);
    out.bytes
}
/// Серверная точка седока: те же общие размеры, что у Java vehicle.rs.
/// [проверить] Клиентская камера Bedrock использует собственную высоту глаз.
pub fn seat(shared: &crate::shared::Shared, dimension: crate::dimension::Dimension, rider: i32) -> Option<(f64, f64, f64)> {
    let (vehicle, index) = {
        let riding = shared.riding.lock().expect("учёт седоков");
        let vehicle = riding.vehicle_of(rider)?;
        (vehicle, riding.passengers(vehicle).iter().position(|id| *id == rider)?)
    };
    let items = shared.items_in(dimension).lock().expect("предметы");
    if let Some(cart) = items.carts().iter().find(|c| c.entity_id == vehicle) {
        return Some(crate::riding::boat_seat((cart.x, cart.y, cart.z)));
    }
    let m = items.mobs().iter().find(|m| m.entity_id == vehicle && !m.is_dead())?;
    Some(if crate::mob::horse::equine(m.kind) { crate::mob::horse::seat(m) }
        else if m.kind == crate::mob::Kind::HappyGhast { crate::mob::newmobs::seat((m.x, m.y, m.z), m.yaw, index) }
        else { (m.x, m.y + if m.kind == crate::mob::Kind::Camel { if m.land.sitting { 0.8 } else { 1.85 } } else { 1.37 }, m.z) })
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::codec::In;
    #[test]
    fn links_round_trip_and_detach_before_reseat() {
        for kind in 0..=2 {
            let p = packet(300, 127, kind);
            let mut r = In::new(&p);
            assert_eq!((r.varint(), r.zigzag64(), r.zigzag64(), r.u8()), (Some(0x29), Some(300), Some(127), Some(kind)));
            assert_eq!((r.bool(), r.bool(), r.lf32(), r.remaining()), (Some(false), Some(false), Some(0.0), 0));
        }
        let mut l = Links::default();
        assert_eq!(l.update(&[(1, vec![2, 3, 4, 5])]), vec![packet(1, 2, 1), packet(1, 3, 2), packet(1, 4, 2), packet(1, 5, 2)]);
        assert!(l.update(&[(1, vec![2, 3, 4, 5])]).is_empty());
        let p = l.update(&[(1, vec![3, 4, 5])]);
        assert_eq!(p, vec![packet(1, 2, 0), packet(1, 3, 0), packet(1, 3, 1)]);
        l.forget(1);
        assert_eq!(l.update(&[(1, vec![3, 4, 5])]).len(), 3);
        assert_eq!(l.update(&[]).len(), 3);
    }
}

