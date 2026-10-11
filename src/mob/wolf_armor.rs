// Wiki, Wolf Armor: «all damage is absorbed anyway», 64 durability,
// scute «8 points». [проверить] Частицы трещин; [Bedrock: потом] body slot.
use super::*;

pub fn stack(mob: &Mob) -> Option<Stack> {
    let remaining = mob.tame.armor?;
    Some(Stack::new(crate::blocks::item_named("wolf_armor")?, 1).with_extras(crate::stackdata::Data {
        damage: (tame::ARMOR_DURABILITY - remaining).max(0), ..Default::default()
    }))
}

pub fn protects(mob: &Mob, source: Source) -> bool {
    mob.kind == Kind::Wolf && mob.tame.armor.is_some_and(|left| left > 0)
        && !matches!(source, Source::Drown | Source::InWall | Source::Magic | Source::IndirectMagic
            | Source::Thorns | Source::Wither | Source::OutOfWorld | Source::OutsideBorder | Source::GenericKill | Source::Starve)
}

pub fn damage(mob: &mut Mob, amount: f32) {
    let Some(before) = mob.tame.armor else { return; };
    if amount <= 0.0 { return; }
    let remaining = before.saturating_sub(amount.ceil() as i32);
    mob.tame.armor = (remaining > 0).then_some(remaining);
    let sound = if remaining <= 0 { "minecraft:item.wolf_armor.break" }
        else if [60, 44, 20].iter().any(|n| before > *n && remaining <= *n) { "minecraft:item.wolf_armor.crack" }
        else { "minecraft:item.wolf_armor.damage" };
    mob.events.push(Event::Sound { name: sound, volume: 1.0, pitch: 1.0 });
}

/// Только владелец снимает/чинит броню; результат (расход, износ ножниц).
pub fn interact(mob: &mut Mob, owner: [u8; 16], held: Stack) -> Option<(bool, bool)> {
    if mob.kind != Kind::Wolf || mob.is_dead() || mob.tame.owner != Some(owner) { return None; }
    match crate::blocks::item_name(held.item)? {
        "wolf_armor" if !mob.is_baby() && mob.tame.armor.is_none() => {
            mob.tame.armor = Some((tame::ARMOR_DURABILITY - held.extras().damage).max(1));
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
}
