// Wiki, Wolf Armor: «all damage is absorbed anyway», 64 durability,
// scute «8 points». [проверить] Частицы трещин; [Bedrock: потом] body slot.
use super::*;

pub fn stack(mob: &Mob) -> Option<Stack> {
    let remaining = mob.tame.armor?;
    Some(Stack::new(crate::blocks::item_named("wolf_armor")?, 1).with_extras(crate::stackdata::Data {
        damage: (tame::ARMOR_DURABILITY - remaining).max(0), dyed_color: mob.tame.armor_color, ..Default::default()
    }))
}

pub fn protects(mob: &Mob, source: Source) -> bool {
    mob.kind == Kind::Wolf && mob.tame.armor.is_some_and(|left| left > 0)
        && !matches!(source, Source::Cramming | Source::DryOut | Source::Freeze | Source::Drown | Source::InWall | Source::Magic | Source::IndirectMagic
            | Source::Thorns | Source::Wither | Source::OutOfWorld | Source::OutsideBorder | Source::GenericKill | Source::Starve)
}

pub fn damage(mob: &mut Mob, amount: f32) {
    let Some(before) = mob.tame.armor else { return; };
    if amount <= 0.0 { return; }
    let remaining = before.saturating_sub(amount.ceil() as i32);
    mob.tame.armor = (remaining > 0).then_some(remaining);
    if remaining <= 0 { mob.tame.armor_color = None; }
    // Чёрный ящик, gaps2-wolf-armor: трещина — звук crack и 20 частиц «item» вокруг
    // волка; шкура ломается тем же crack (звука break нет) + событие 65; в ином
    // случае удар по броне — item.wolf_armor.damage громкостью 0,4.
    let cracked = remaining <= 0 || [60, 44, 20].iter().any(|n| before > *n && remaining <= *n);
    if remaining <= 0 { mob.events.push(Event::Status(65)); }
    if cracked {
        mob.events.push(Event::Sound { name: "minecraft:item.wolf_armor.crack", volume: 1.0, pitch: 1.0 });
        mob.events.push(Event::Particle { kind: Particle::ItemBreak, at: (mob.x, mob.y + 1.0, mob.z), offset: (0.2, 0.1, 0.2), speed: 0.1, count: 20 });
    }
    if remaining > 0 { mob.events.push(Event::Sound { name: "minecraft:item.wolf_armor.damage", volume: 0.4, pitch: 1.0 }); }
}

/// Только владелец снимает/чинит броню; результат (расход, износ ножниц).
pub fn interact(mob: &mut Mob, owner: [u8; 16], held: Stack) -> Option<(bool, bool)> {
    if mob.kind != Kind::Wolf || mob.is_dead() || mob.tame.owner != Some(owner) { return None; }
    match crate::blocks::item_name(held.item)? {
        "wolf_armor" if !mob.is_baby() && mob.tame.armor.is_none() => {
            mob.tame.armor = Some((tame::ARMOR_DURABILITY - held.extras().damage).max(1));
            mob.tame.armor_color = held.extras().dyed_color;
                    mob.events.push(Event::Sound { name: "minecraft:item.armor.equip_wolf", volume: 1.0, pitch: 1.0 });
            Some((true, false))
        }
        "armadillo_scute" if mob.tame.ordered_sit => {
            let remaining = mob.tame.armor?;
            if remaining >= tame::ARMOR_DURABILITY { return Some((false, false)); }
            mob.tame.armor = Some((remaining + 8).min(tame::ARMOR_DURABILITY));
                    mob.events.push(Event::Sound { name: "minecraft:item.wolf_armor.repair", volume: 1.0, pitch: 1.0 });
            Some((true, false))
        }
        "shears" => {
            let armor = stack(mob)?;
            mob.tame.armor = None;
            mob.tame.armor_color = None;
                    mob.events.push(Event::Loot(vec![armor]));
            mob.events.push(Event::Sound { name: "minecraft:item.armor.unequip_wolf", volume: 1.0, pitch: 1.0 });
            Some((false, true))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dye_survives_equipping_saving_repair_and_removal_and_bypass_is_complete() {
        let mut wolf=Mob::new(4,Kind::Wolf,(0.0,0.0,0.0));wolf.tame.owner=Some([2;16]);
        let armor=Stack::new(crate::blocks::item_named("wolf_armor").unwrap(),1).with_extras(crate::stackdata::Data {dyed_color:Some(0xabcdef),damage:5,..Default::default()});
        interact(&mut wolf,[2;16],armor).unwrap();
        let mut bytes=Vec::new();tame::write(&wolf.tame,&mut bytes);wolf.tame=tame::read(&mut bytes.as_slice()).unwrap();
        assert_eq!(stack(&wolf).unwrap().extras().dyed_color,Some(0xabcdef));
        for source in [Source::Cramming,Source::DryOut,Source::Freeze,Source::Drown,Source::Magic,Source::IndirectMagic,Source::InWall,Source::Wither,Source::Thorns,Source::OutOfWorld,Source::OutsideBorder,Source::GenericKill,Source::Starve] { assert!(!protects(&wolf,source),"{source:?}"); }
        for source in [Source::Fall,Source::Arrow,Source::Lava,Source::Explosion,Source::MobAttack] { assert!(protects(&wolf,source)); }
        interact(&mut wolf,[2;16],Stack::new(crate::blocks::item_named("shears").unwrap(),1)).unwrap();
        assert!(wolf.events.iter().any(|e|matches!(e,Event::Loot(s) if s[0].extras().dyed_color==Some(0xabcdef))));
    }

    #[test]
    fn wolf_armor_absorbs_overkill_repairs_removes_and_preserves_damage() {
        let mut wolf = Mob::new(4, Kind::Wolf, (0.0, 0.0, 0.0));
        wolf.tame.owner = Some([2; 16]);
        let item = |name| Stack::new(crate::blocks::item_named(name).unwrap(), 1);
        let armor = item("wolf_armor").with_extras(crate::stackdata::Data { damage: 14, ..Default::default() });
        assert!(interact(&mut wolf, [3;16], armor).is_none());
        assert_eq!(interact(&mut wolf, [2;16], armor), Some((true,false)));
        assert_eq!(wolf.tame.armor, Some(50));
        wolf.tame.ordered_sit = true;
        assert_eq!(interact(&mut wolf, [2;16], item("armadillo_scute")), Some((true,false)));
        assert_eq!(wolf.tame.armor, Some(58));
        let health = wolf.vitals.health;
        let hit = wolf.hurt(100.0, Source::Fall, None).unwrap();
        assert!(!hit.died);
        assert_eq!(wolf.vitals.health, health);
        assert_eq!(wolf.tame.armor, None);
        assert!(wolf.hurt(100.0, Source::Fall, None).is_none(), "cooldown не изнашивает броню повторно");
        wolf.vitals.invulnerable = 0;
        interact(&mut wolf, [2;16], armor);
        wolf.hurt(2.0, Source::Magic, None).unwrap();
        assert_eq!(wolf.vitals.health, health - 2.0);
        assert_eq!(wolf.tame.armor, Some(50));
        let mut bytes=Vec::new();tame::write(&wolf.tame,&mut bytes);
        let saved=tame::read(&mut bytes.as_slice()).unwrap();assert_eq!(saved.armor,Some(50));
        assert_eq!(interact(&mut wolf, [2;16], item("shears")), Some((false,true)));
        assert!(wolf.events.iter().any(|event| matches!(event, Event::Loot(stacks) if stacks[0].extras().damage == 14)));
    }

    #[test]
    fn crack_has_sound_and_particles_and_break_has_event_65_without_break_sound() {
        let mut wolf = Mob::new(4, Kind::Wolf, (0.0, 0.0, 0.0));
        wolf.tame.armor = Some(64);
        damage(&mut wolf, 20.0);
        let sounds = |wolf: &Mob| wolf.events.iter().filter_map(|e| match e { Event::Sound { name, volume, .. } => Some((*name, *volume)), _ => None }).collect::<Vec<_>>();
        assert_eq!(sounds(&wolf), [("minecraft:item.wolf_armor.crack", 1.0), ("minecraft:item.wolf_armor.damage", 0.4)]);
        assert!(wolf.events.iter().any(|e| matches!(e, Event::Particle { kind: Particle::ItemBreak, count: 20, .. })));
        wolf.events.clear();
        damage(&mut wolf, 100.0);
        assert!(wolf.events.contains(&Event::Status(65)));
        assert_eq!(sounds(&wolf), [("minecraft:item.wolf_armor.crack", 1.0)]);
        assert_eq!(wolf.tame.armor, None);
        // Малый удар без порога: только тихий звук.
        wolf.tame.armor = Some(64);
        wolf.events.clear();
        damage(&mut wolf, 2.0);
        assert_eq!(sounds(&wolf), [("minecraft:item.wolf_armor.damage", 0.4)]);
    }

    #[test]
    fn armored_wolf_hurt_sound_is_quiet_even_when_the_hit_bypasses_the_armor() {
        let mut wolf = Mob::new(4, Kind::Wolf, (0.0, 0.0, 0.0));
        wolf.tame.armor = Some(64);
        wolf.hurt(2.0, Source::Freeze, None).unwrap();
        assert!(wolf.events.iter().any(|e| matches!(e, Event::Sound { name, volume, .. } if name.ends_with("wolf.hurt") && (*volume - 0.4).abs() < 1e-6)));
    }
}
