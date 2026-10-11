// Вики, Wind Charged/Weaving/Oozing/Infested, ревизии до 26.1.2.
// Реакции отделены от Vitals: нужны мир, новые entity id и правило mobGriefing.
// [проверить] Точное место в такте, специальные частицы и мощность порыва.
use crate::{blocks, items::Items, mob::{Kind, Mob, Rng}, survival::{EffectKind, Vitals}, world::{World, AIR}};

pub struct Reaction {
    pub at: (f64, f64, f64),
    pub centre: (f64, f64, f64),
    pub yaw: f32,
    pub player: bool,
    pub wind: bool,
    pub weaving: bool,
    pub oozing: bool,
    pub infested_hits: u32,
}

pub fn take(vit: &mut Vitals, dead: bool, at: (f64,f64,f64), height: f64, yaw: f32, player: bool) -> Reaction {
    let death = dead && !vit.death_effects_handled;
    if death { vit.death_effects_handled = true; }
    Reaction { at, centre: (at.0, at.1 + height / 2.0, at.2), yaw, player,
        wind: death && vit.has_effect(EffectKind::WindCharged),
        weaving: death && vit.has_effect(EffectKind::Weaving),
        oozing: death && vit.has_effect(EffectKind::Oozing),
        infested_hits: std::mem::take(&mut vit.infested_hits) }
}

pub fn infested_count(chance: u32, count: u32) -> usize {
    if chance == 0 { 1 + (count % 2) as usize } else { 0 }
}

/// Ограничение Oozing — соседние слизни, куб 5³ (вики, Oozing).
/// В этом сервере gamerule max_entity_cramming ещё отсутствует: default 24.
fn ooze_room(items: &Items, at: (f64,f64,f64)) -> usize {
    let nearby = items.mobs().iter().filter(|m| m.kind == Kind::Slime && !m.is_dead() && (m.x-at.0).abs() <= 2.5 && (m.y-at.1).abs() <= 2.5 && (m.z-at.2).abs() <= 2.5).count();
    24usize.saturating_sub(nearby).min(2)
}

pub fn apply(world: &mut World, items: &mut Items, r: Reaction, griefing: bool, targets: &[crate::tnt::Target], next_id: &mut dyn FnMut() -> i32, rng: &mut Rng) {
    for _ in 0..r.infested_hits {
        let count = infested_count(rng.below(10), rng.below(2));
        for _ in 0..count {
            let mut mob = Mob::new(next_id(), Kind::Silverfish, r.centre);
            let angle = (f64::from(r.yaw) + rng.unit() * 90.0 - 45.0).to_radians();
            mob.vx = -angle.sin() * 0.3;
            mob.vy = 0.3;
            mob.vz = angle.cos() * 0.3;
            items.add_mob_late(mob);
        }
    }
    if r.oozing {
        for _ in 0..ooze_room(items, r.at) {
            let mut mob = Mob::new(next_id(), Kind::Slime, r.at);
            crate::mob::slime::set_size(&mut mob, 2);
            items.add_mob_late(mob);
        }
    }
    if r.weaving && (r.player || griefing) {
        let (x,y,z) = (r.at.0.floor() as i32,r.at.1.floor() as i32,r.at.2.floor() as i32);
        let mut places = Vec::new();
        for dx in -1..=1 { for dy in -1..=1 { for dz in -1..=1 {
            let p = (x+dx,y+dy,z+dz);
            if world.get_block(p.0,p.1,p.2) == AIR && blocks::full_cube(world.get_block(p.0,p.1-1,p.2)) { places.push(p); }
        } } }
        let count = 2 + rng.below(2);
        for _ in 0..count {
            if places.is_empty() { break; }
            let p = places.swap_remove(rng.below(places.len() as u32) as usize);
            world.set_block(p.0,p.1,p.2,blocks::state_by_name("cobweb").unwrap_or(AIR));
        }
    }
    if r.wind {
        let mut doors = Vec::new();
        // Общая механика недеструктивного ветра; диапазон силы [проверить].
        let power = 3.0 + rng.unit() as f32 * 2.0;
        crate::tnt::wind_charge_burst(world, items, targets, r.centre, power, 1.0, crate::world::Gust::Breeze, &mut |world, pos| {
            crate::wind::interact(world, pos, &mut doors, r.player || griefing);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn effect(vit: &mut Vitals, kind: EffectKind) {
        vit.give_effect_shown(crate::survival::Effect { kind, amplifier: 3, ticks_left: 100, ambient: false, visible: true, show_icon: true });
    }
    fn reaction(kind: EffectKind) -> Reaction {
        let mut vit=Vitals::new(); effect(&mut vit,kind);
        take(&mut vit,true,(0.5,1.0,0.5),1.8,0.0,false)
    }
    #[test]
    fn weaving_places_two_or_three_supported_webs_and_obeys_griefing() {
        let mut w=World::in_memory(); let mut items=Items::new(); let mut id=100;
        for x in -2..=2 { for z in -2..=2 { w.set_block_silently(x,0,z,blocks::state_by_name("stone").unwrap()); } }
        apply(&mut w,&mut items,reaction(EffectKind::Weaving),false,&[],&mut || {id+=1;id},&mut Rng::new(1));
        assert_eq!(w.get_block(0,1,0),AIR);
        apply(&mut w,&mut items,reaction(EffectKind::Weaving),true,&[],&mut || {id+=1;id},&mut Rng::new(1));
        let count=(-1..=1).flat_map(|x|(-1..=1).map(move |z|(x,z))).filter(|&(x,z)|blocks::block_at_state(w.get_block(x,1,z))==Some("cobweb")).count();
        assert!((2..=3).contains(&count));
    }
    #[test]
    fn oozing_spawns_two_medium_slimes_once_and_caps_nearby() {
        let mut w=World::in_memory();let mut items=Items::new();let mut id=100;
        apply(&mut w,&mut items,reaction(EffectKind::Oozing),false,&[],&mut || {id+=1;id},&mut Rng::new(1));
        assert_eq!(items.mobs().len(),2);
        assert!(items.mobs().iter().all(|m|m.kind==Kind::Slime && m.nether.size==2 && m.vitals.health==4.0));
        for _ in 0..22 { id+=1;items.add_mob(Mob::new(id,Kind::Slime,(0.5,1.0,0.5))); }
        assert_eq!(ooze_room(&items,(0.5,1.0,0.5)),0);
        let mut v=Vitals::new();effect(&mut v,EffectKind::Oozing);
        assert!(take(&mut v,true,(0.0,0.0,0.0),1.8,0.0,false).oozing);
        assert!(!take(&mut v,true,(0.0,0.0,0.0),1.8,0.0,false).oozing);
    }
    #[test]
    fn infested_accepts_zero_damage_but_not_cooldown_or_immune_hits() {
        let mut v=Vitals::new();effect(&mut v,EffectKind::Infested);
        assert!(v.hurt(0.0,crate::survival::Source::Thrown,false).is_some());
        assert_eq!(v.infested_hits,1);
        assert!(v.hurt(0.0,crate::survival::Source::Thrown,false).is_none());
        assert_eq!(v.infested_hits,1);
        assert_eq!(infested_count(0,0),1);assert_eq!(infested_count(0,1),2);
        for roll in 1..10 {assert_eq!(infested_count(roll,0),0);}
        v.invulnerable=0;
        assert!(v.hurt(1.0,crate::survival::Source::Thrown,true).is_none());
    }
    #[test]
    fn wind_charged_only_triggers_on_death_and_does_not_break_blocks() {
        let mut v=Vitals::new();effect(&mut v,EffectKind::WindCharged);
        assert!(!take(&mut v,false,(0.0,0.0,0.0),1.8,0.0,false).wind);
        let r=take(&mut v,true,(0.5,1.0,0.5),1.8,0.0,false);assert!(r.wind);
        let mut w=World::in_memory();let mut items=Items::new();
        let stone=blocks::state_by_name("stone").unwrap();w.set_block_silently(0,0,0,stone);
        apply(&mut w,&mut items,r,true,&[],&mut || 500,&mut Rng::new(1));
        assert_eq!(w.get_block(0,0,0),stone);
    }
}
