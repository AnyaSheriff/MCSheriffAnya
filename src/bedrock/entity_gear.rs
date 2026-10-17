//! Снаряжение общего моба: MobEquipment и MobArmorEquipment (944).
//! Mojang bedrock-protocol-docs r/26_u1: «Body» — пятый слот брони.
use crate::{blocks, inventory::Stack, mob::{Kind, Mob}};
use super::{codec::Out, inventory::write_item};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Gear {
    pub hand: Option<Stack>,
    pub offhand: Option<Stack>,
    /// Голова, грудь, ноги, ступни, тело животного.
    pub armor: [Option<Stack>; 5],
}
fn item(name: &str) -> Option<Stack> { blocks::item_named(name).map(|id| Stack::new(id, 1)) }
impl Gear {
    pub fn of(m: &Mob) -> Self {
        let mut g = Self::default();
        if m.kind.skeleton() && m.h.bow { g.hand = item("bow"); }
        if let Some(name) = crate::mob::hostile::hand_item(m.h.hand) { g.hand = item(name); }
        if let Some(name) = crate::mob::illager::hand_item(m.kind) {
            g.hand = item(name).map(|s| if m.kind == Kind::Pillager {
                s.with_extras(crate::stackdata::Data { damage: m.illager.wear, charged: u8::from(m.illager.charged), ..Default::default() })
            } else { s });
        }
        // Оружие при появлении, как у BDS (запись br-cases.js, снаряжение при add_entity).
        if g.hand.is_none() {
            g.hand = match m.kind {
                Kind::WitherSkeleton => item("stone_sword"),
                Kind::ZombifiedPiglin => item("golden_sword"),
                Kind::PiglinBrute => item("golden_axe"),
                Kind::Piglin => item(if m.entity_id % 2 == 0 { "golden_sword" } else { "crossbow" }),
                _ => None,
            };
        }
        if m.h.offhand_shell { g.offhand = item("nautilus_shell"); }
        if m.illager.captain {
            g.armor[0] = item("white_banner").map(|s| s.with_extras(crate::stackdata::Data { ominous_banner: true, ..Default::default() }));
        }
        if m.h.helmet { g.armor[0] = item("iron_helmet"); }
        match m.kind {
            Kind::Fox => g.hand = m.wild.mouth,
            Kind::Allay => g.hand = m.small.held,
            Kind::Panda => g.hand = m.land.held,
            Kind::CopperGolem => g.hand = m.newmobs.cargo,
            Kind::Witch if m.h.drinking > 0 => g.hand = item("potion").map(|s| s.with_extras(crate::stackdata::Data { potion: Some(m.h.potion), ..Default::default() })),
            Kind::WanderingTrader if m.land.drinking > 0 => g.hand = item(if m.land.invisible { "milk_bucket" } else { "potion" }),
            Kind::Wolf => g.armor[4] = crate::mob::wolf_armor::stack(m),
            Kind::Llama | Kind::TraderLlama => g.armor[4] = m.land.decor.and_then(|c| crate::mob::WOOL.get(usize::from(c))).and_then(|name| item(&name.replace("_wool", "_carpet"))),
            Kind::HappyGhast => g.armor[4] = m.newmobs.harness.and_then(|c| crate::mob::newmobs::COLOURS.get(usize::from(c))).and_then(|c| item(&format!("{c}_harness"))),
            _ if crate::mob::horse::equine(m.kind) => g.armor[4] = m.horse.slots[1],
            _ => {}
        }
        if let Some(gear) = &m.h.trial_gear { g.hand = gear[0]; g.armor[1] = gear[1]; g.armor[0] = gear[2]; }
        if m.equipment.is_some() {
            let gear = crate::mob::equipment::all(m);
            g.hand = gear[0]; g.offhand = gear[1];
            for (target, source) in [5, 4, 3, 2].into_iter().enumerate() { g.armor[target] = gear[source]; }
        }
        g
    }

    /// Пустые слоты тоже уходят: снятая броня не должна оставаться у клиента.
    pub fn packets(&self, id: i32) -> Vec<Vec<u8>> {
        vec![hand_packet(id, self.hand, false), hand_packet(id, self.offhand, true), armor_packet(id, &self.armor)]
    }

    /// При появлении BDS пустые слоты не шлёт (запись d-*): уходят только занятые.
    pub fn spawn_packets(&self, id: i32) -> Vec<Vec<u8>> {
        let mut list = Vec::new();
        if self.hand.is_some() { list.push(hand_packet(id, self.hand, false)); }
        if self.offhand.is_some() { list.push(hand_packet(id, self.offhand, true)); }
        if self.armor.iter().any(Option::is_some) { list.push(armor_packet(id, &self.armor)); }
        list
    }
}
pub fn hand_packet(id: i32, stack: Option<Stack>, offhand: bool) -> Vec<u8> {
    let mut out = Out::packet(0x1f);
    out.varint64(id as u64);
    write_item(&mut out, stack, 0);
    out.u8(0).u8(0).u8(if offhand { 119 } else { 0 });
    out.bytes
}
pub fn armor_packet(id: i32, slots: &[Option<Stack>; 5]) -> Vec<u8> {
    let mut out = Out::packet(0x20);
    out.varint64(id as u64);
    for slot in slots { write_item(&mut out, *slot, 0); }
    out.bytes
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::codec::In;
    fn read_item(r: &mut In) -> Option<i32> {
        let id = r.zigzag32()?;
        if id == 0 { return Some(0); }
        assert_eq!(r.take(2)?, &[1, 0]);
        r.varint()?;
        assert_eq!(r.u8()?, 0);
        r.zigzag32()?;
        let len = r.varint()? as usize;
        r.take(len)?;
        Some(id)
    }
    #[test]
    fn equipment_packets_round_trip_including_body_and_clear() {
        for offhand in [false, true] {
            let p = hand_packet(301, item("bow"), offhand);
            let mut r = In::new(&p);
            assert_eq!(r.varint(), Some(0x1f));
            assert_eq!(r.varint64(), Some(301));
            assert_eq!(read_item(&mut r), item("bow").and_then(|s| super::super::inventory::bedrock_item(s.item)).map(|s| s.0));
            assert_eq!((r.u8(), r.u8(), r.u8()), (Some(0), Some(0), Some(if offhand { 119 } else { 0 })));
            assert_eq!(r.remaining(), 0);
        }
        let slots = [item("iron_helmet"), None, None, None, item("wolf_armor")];
        let p = armor_packet(9, &slots);
        let mut r = In::new(&p);
        assert_eq!((r.varint(), r.varint64()), (Some(0x20), Some(9)));
        for slot in slots { assert_eq!(read_item(&mut r), Some(slot.and_then(|s| super::super::inventory::bedrock_item(s.item)).map_or(0, |s| s.0))); }
        assert_eq!(r.remaining(), 0);
        assert_eq!(armor_packet(9, &[None; 5]), [0x20, 9, 0, 0, 0, 0, 0]);
        assert_eq!(hand_packet(9, None, true), [0x1f, 9, 0, 0, 0, 119]);
    }

    #[test]
    fn generic_mob_equipment_reaches_bedrock_and_clears_removed_boots() {
        let mut mob = Mob::new(3, Kind::Zombie, (0.0, 64.0, 0.0));
        let boots = item("diamond_boots");
        crate::mob::equipment::set_slot(&mut mob, 2, boots);
        let shown = Gear::of(&mob);
        assert_eq!(shown.armor[3], boots);
        assert_eq!(shown.spawn_packets(3).len(), 1);
        crate::mob::equipment::set_slot(&mut mob, 2, None);
        let changed = Gear::of(&mob);
        assert_ne!(shown, changed);
        assert_eq!(changed.packets(3)[2], armor_packet(3, &[None; 5]));
    }
}

/// UpdateAttributes: здоровье и скорость из общего состояния, без еды игрока.
pub fn attributes(id: i32, look: &super::mobdata::Look) -> Vec<u8> {
    let mut out = Out::packet(super::session::UPDATE_ATTRIBUTES);
    // У вида без движения (кристалл Края) BDS атрибутов не шлёт совсем.
    let none = super::bds_profile::of(look.kind).is_some_and(|p| p.speed.is_nan());
    let mut list = if none { vec![] } else { vec![
        ("minecraft:health", look.max_health, look.health, look.max_health),
        ("minecraft:movement", f32::MAX, look.speed, look.speed),
    ] };
    // У лошадиных BDS шлёт ещё силу прыжка (запись d-horse: horse.jump_strength).
    if let Some(jump) = look.jump {
        list.push(("minecraft:horse.jump_strength", f32::MAX, jump, jump));
    }
    out.varint64(id as u64).varint(list.len() as u32);
    for (name, max, current, default) in list {
        out.lf32(0.0).lf32(max).lf32(current).lf32(0.0).lf32(max).lf32(default).string(name).varint(0);
    }
    out.varint64(0);
    out.bytes
}

#[cfg(test)]
mod attribute_tests {
    use super::*;
    use super::super::{codec::In, mobdata::Look};
    #[test]
    fn crystals_have_no_attributes_and_equines_add_jump_strength() {
        let crystal = Look::of(&Mob::new(5, Kind::EndCrystal, (0.0, 0.0, 0.0)));

        assert_eq!(attributes(5, &crystal)[2..], [0, 0]); // id, число записей 0, такт 0

        let horse = Look::of(&Mob::new(6, Kind::Horse, (0.0, 0.0, 0.0)));
        let p = attributes(6, &horse);
        let mut r = In::new(&p);

        assert_eq!((r.varint(), r.varint64(), r.varint()), (Some(0x1d), Some(6), Some(3)));
        assert!(String::from_utf8_lossy(&p).contains("minecraft:horse.jump_strength"));
    }
    #[test]
    fn wither_skeletons_piglins_and_brutes_spawn_armed_like_bds() {
        for (kind, name) in [(Kind::WitherSkeleton, "stone_sword"), (Kind::ZombifiedPiglin, "golden_sword"), (Kind::PiglinBrute, "golden_axe")] {
            assert_eq!(Gear::of(&Mob::new(3, kind, (0.0, 0.0, 0.0))).hand, item(name), "{kind:?}");
        }
    }
    #[test]
    fn health_and_movement_round_trip_with_six_float_bounds() {
        let mut m = Mob::new(71, Kind::Wolf, (0.0, 0.0, 0.0));
        m.vitals.max_health = 40.0;
        m.vitals.health = 27.0;
        let look = Look::of(&m);
        let p = attributes(71, &look);
        let mut r = In::new(&p);
        assert_eq!((r.varint(), r.varint64(), r.varint()), (Some(0x1d), Some(71), Some(2)));
        for (name, max, current, default) in [("minecraft:health", 40.0, 27.0, 40.0), ("minecraft:movement", f32::MAX, look.speed, Kind::Wolf.speed() as f32)] {
            for f in [0.0, max, current, 0.0, max, default] { assert_eq!(r.lf32(), Some(f)); }
            assert_eq!(r.string().as_deref(), Some(name));
            assert_eq!(r.varint(), Some(0));
        }
        assert_eq!(r.varint64(), Some(0));
        assert_eq!(r.remaining(), 0);
    }
}
