// Общие эффекты оружия и шипов живых мобов. Значения чар берутся из
// фактической экипировки, включая команды и сохранённый мир.
use super::{equipment, Event, Kind, Mob, Push, Rng, Source};
use crate::{combat, inventory::Stack, world::ProjectileEnchantments};

#[derive(Clone, Debug)]
pub struct Retaliation {
    pub attacker: i32,
    pub target: i32,
    pub damage: f32,
    pub at: (f64, f64, f64),
    pub kind: Kind,
    pub looting: i32,
}

pub fn weapon(mob: &Mob) -> Option<Stack> { equipment::slot(mob, 0) }

pub fn bonus(mob: &Mob, target: Option<Kind>) -> f32 {
    let Some(stack) = weapon(mob) else { return 0.0; };
    let data = stack.extras();
    let (undead, arthropod) = target.map_or((false, false), |kind| combat::mob_traits(kind.name()));
    combat::enchant_bonus(&data, undead, arthropod)
        + target.map_or(0.0, |kind| combat::impaling_bonus(&data, kind.name()))
}

pub fn effects(mob: &Mob) -> ProjectileEnchantments {
    let Some(stack) = weapon(mob) else { return ProjectileEnchantments::default(); };
    let data = stack.extras();
    ProjectileEnchantments {
        fire_ticks: data.level("fire_aspect").saturating_mul(80),
        melee_knockback: data.level("knockback") as f32,
        ..Default::default()
    }
}

pub fn wear(mob: &mut Mob, slot: u8, amount: i32, rng: &mut Rng) {
    let Some(stack) = equipment::slot(mob, slot) else { return; };
    let amount = combat::wear(&stack, amount, slot >= 2, || rng.unit() as f32);
    let replacement = combat::add_damage(stack, amount);
    if replacement == Some(stack) { return; }
    equipment::set_slot(mob, slot, replacement);
    if replacement.is_none() {
        mob.events.push(Event::Status(match slot { 0 => 47, 1 => 48, _ => 54 - slot }));
        mob.events.push(Event::Sound { name: "minecraft:entity.item.break", volume: 0.8, pitch: 0.8 });
    }
}

/// Шипы срабатывают только после принятого попадания и не отражают шипы.
/// Очередь сохраняет отражение и при смертельном ударе по носителю брони.
pub fn on_hurt(mob: &mut Mob, source: Source, push: Option<Push>) {
    let Some(target) = push.map(|p| p.cause).filter(|id| *id != 0 && *id != mob.entity_id) else { return; };
    if !(source.is_projectile() || matches!(source, Source::PlayerAttack | Source::MaceSmash | Source::MobAttack | Source::MobAttackNoAggro)) { return; }
    let mut rng = Rng::new(u64::from(mob.entity_id as u32) ^ mob.ticks.wrapping_mul(0x9e3779b9) ^ u64::from(mob.impulses.wrapping_mul(0x51)));
    for (slot, stack) in equipment::all(mob).into_iter().enumerate() {
        let Some(stack) = stack else { continue; };
        let level = stack.extras().level("thorns");
        if level <= 0 || rng.unit() >= 0.15 * f64::from(level) { continue; }
        mob.enchant_retaliation.push(Retaliation {
            attacker: mob.entity_id, target, damage: combat::thorns_damage(level, rng.unit() as f32),
            at: (mob.x, mob.y, mob.z), kind: mob.kind,
            looting: weapon(mob).map_or(0, |stack| stack.extras().level("looting")),
        });
        // Отдельный износ именно вещи, давшей шипы. Обычный урон броне
        // моба сюда не добавляется без подтверждения чёрным ящиком.
        wear(mob, slot as u8, 2, &mut rng);
    }
}

pub fn reflect(mobs: &mut [Mob]) -> Vec<Retaliation> {
    let pending: Vec<_> = mobs.iter_mut().flat_map(|mob| std::mem::take(&mut mob.enchant_retaliation)).collect();
    let mut players = Vec::new();
    for hit in pending {
        if let Some(target) = mobs.iter_mut().find(|mob| mob.entity_id == hit.target) {
            let dir = (target.x - hit.at.0, target.z - hit.at.2);
            let length = dir.0.hypot(dir.1).max(1e-9);
            let before = target.looting;
            target.looting = hit.looting;
            if target.hurt(hit.damage, Source::Thorns, Some(Push { dir: (dir.0 / length, dir.1 / length), strength: super::BASE_KNOCKBACK, cause: hit.attacker })).is_none() {
                target.looting = before;
            } else {
                target.events.push(Event::Sound { name: "minecraft:enchant.thorns.hit", volume: 1.0, pitch: 1.0 });
            }
        } else {
            players.push(hit);
        }
    }
    players
}

#[cfg(test)]
mod tests {
    use super::*;
    fn enchanted(name: &str, enchants: &[(&str, i32)]) -> Stack {
        let mut data = crate::stackdata::Data::default();
        for (name, level) in enchants { data.set_enchant(crate::stackdata::enchantment_number(name).unwrap(), *level); }
        Stack::new(crate::blocks::item_named(name).unwrap(), 1).with_extras(data)
    }

    #[test]
    fn assigned_weapon_uses_target_traits_and_replacement() {
        let mut mob = Mob::new(101, Kind::Zombie, (0.0, 64.0, 0.0));
        equipment::set_slot(&mut mob, 0, Some(enchanted("diamond_sword", &[("sharpness", 5), ("smite", 2), ("bane_of_arthropods", 3), ("fire_aspect", 2), ("knockback", 2)])));
        assert_eq!(bonus(&mob, None), 3.0);
        assert_eq!(bonus(&mob, Some(Kind::Skeleton)), 8.0);
        assert_eq!(bonus(&mob, Some(Kind::Spider)), 10.5);
        assert_eq!(effects(&mob).fire_ticks, 160);
        assert_eq!(effects(&mob).melee_knockback, 2.0);
        equipment::set_slot(&mut mob, 0, None);
        assert_eq!(bonus(&mob, Some(Kind::Skeleton)), 0.0);
        assert_eq!(effects(&mob).fire_ticks, 0);
    }

    #[test]
    fn equipped_zombie_attack_matches_measured_damage_fire_and_weapon_wear() {
        struct Floor;
        impl super::super::View for Floor {
            fn block(&self, _: i32, y: i32, _: i32) -> i32 { if y < 64 { 1 } else { 0 } }
            fn loaded(&self, _: i32, _: i32) -> bool { true }
        }
        let mut zombie = Mob::new(101, Kind::Zombie, (0.5, 64.0, 0.5));
        zombie.h.target = Some(2); zombie.h.provoked = true;
        equipment::set_slot(&mut zombie, 0, Some(enchanted("diamond_sword", &[("sharpness", 5), ("fire_aspect", 2), ("knockback", 2)])));
        let mut items = crate::items::Items::new(); items.add_mob(zombie);
        let target = super::super::ai::Watcher { id: 2, x: 0.5, y: 64.0, z: 1.5, eye: 1.62,
            holding: [None, None], spectator: false, targetable: true, look: (0.0, 0.0), pumpkin: false,
            sprinting: false, riding: false, gold_armor: false };
        let mut rng = Rng::new(2);
        let mut attack = None;
        for tick in 0..30 {
            let outcome = super::super::step_all(&mut items, &Floor, &[target], super::super::Env {
                difficulty: 2, sky: 0, sun_up: false, clock: 18_000, tick }, &mut rng, &mut || 200);
            if let Some(strike) = outcome.strikes.into_iter().next() { attack = Some(strike); break; }
        }
        let strike = attack.expect("зомби должен ударить соседнего игрока");
        // enchant-mob-melee-effects: health64→52, Fire137 спустя23ticks.
        assert_eq!(strike.melee.damage, 12.0);
        assert_eq!(strike.enchantments.fire_ticks, 160);
        assert_eq!(strike.enchantments.melee_knockback, 2.0);
        assert_eq!(equipment::slot(&items.mobs()[0], 0).unwrap().extras().damage, 0);
    }

    #[test]
    fn accepted_lethal_hit_still_reflects_thorns_and_wears_its_slot() {
        let mut victim = Mob::new(102, Kind::Zombie, (0.0, 64.0, 0.0));
        equipment::set_slot(&mut victim, 4, Some(enchanted("diamond_chestplate", &[("thorns", 7)])));
        victim.vitals.health = 1.0;
        let push = Push { dir: (0.0, 1.0), strength: 0.4, cause: 103 };
        assert!(victim.hurt(20.0, Source::PlayerAttack, Some(push)).unwrap().died);
        assert_eq!(equipment::slot(&victim, 4).unwrap().extras().damage, 2);
        assert_eq!(victim.enchant_retaliation.len(), 1);
        let mut mobs = vec![victim];
        let reflected = reflect(&mut mobs);
        assert_eq!(reflected.len(), 1);
        assert_eq!(reflected[0].target, 103);
        assert!((1.0..5.0).contains(&reflected[0].damage));
        assert!(reflect(&mut mobs).is_empty());
    }

    #[test]
    fn mob_reflection_neither_recurses_nor_repeats_rejected_hits() {
        let mut victim = Mob::new(104, Kind::Zombie, (0.0, 64.0, 0.0));
        equipment::set_slot(&mut victim, 4, Some(enchanted("diamond_chestplate", &[("thorns", 7)])));
        let mut attacker = Mob::new(105, Kind::Zombie, (0.0, 64.0, -2.0));
        equipment::set_slot(&mut attacker, 4, Some(enchanted("diamond_chestplate", &[("thorns", 7)])));
        let push = Push { dir: (0.0, 1.0), strength: 0.4, cause: 105 };
        assert!(victim.hurt(1.0, Source::MobAttack, Some(push)).is_some());
        assert!(victim.hurt(1.0, Source::MobAttack, Some(push)).is_none());
        assert_eq!(victim.enchant_retaliation.len(), 1);
        let mut mobs = vec![victim, attacker];
        assert!(reflect(&mut mobs).is_empty());
        assert!(mobs[1].vitals.health < 20.0);
        assert!(mobs[1].enchant_retaliation.is_empty());
        assert_eq!(equipment::slot(&mobs[1], 4).unwrap().extras().damage, 0);
    }

    #[test]
    fn wear_respects_unbreakable_and_unbreaking_and_breaks_correct_slot() {
        let mut mob = Mob::new(106, Kind::Zombie, (0.0, 64.0, 0.0));
        let mut boots = enchanted("diamond_boots", &[("unbreaking", 3)]);
        equipment::set_slot(&mut mob, 2, Some(boots));
        wear(&mut mob, 2, 100, &mut Rng::new(42));
        let worn = equipment::slot(&mob, 2).unwrap().extras().damage;
        assert!((45..90).contains(&worn));
        let mut data = boots.extras(); data.unbreakable = true; boots = boots.with_extras(data);
        equipment::set_slot(&mut mob, 2, Some(boots));
        wear(&mut mob, 2, 100, &mut Rng::new(42));
        assert_eq!(equipment::slot(&mob, 2).unwrap().extras().damage, 0);
        let plain = enchanted("diamond_boots", &[]);
        let mut data = plain.extras(); data.damage = combat::max_damage(&plain) - 1;
        equipment::set_slot(&mut mob, 2, Some(plain.with_extras(data)));
        wear(&mut mob, 2, 1, &mut Rng::new(42));
        assert!(equipment::slot(&mob, 2).is_none());
        assert!(mob.events.iter().any(|event| matches!(event, Event::Status(52))));
    }
}
