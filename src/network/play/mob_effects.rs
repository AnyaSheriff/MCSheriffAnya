// Вики, Attribute: «Modifiers»; Speed/Slowness меняют movement_speed.
// Используем общую таблицу EffectKind, чтобы снятие и скрытый эффект
// давали тот же пакет, что зелье. Порядок атрибутов [проверить].
use super::*;
use crate::attributes::{self, Attribute, Modifier};
use crate::mob::{Kind, Mob};
use crate::survival::EffectKind;

pub(super) const KINDS: [EffectKind; 4] = [EffectKind::Speed, EffectKind::Slowness, EffectKind::HealthBoost, EffectKind::Absorption];

pub(super) fn levels(mob: &Mob) -> [i32; 4] {
    KINDS.map(|kind| mob.vitals.effect_level(kind))
}

pub(super) fn modifiers(mob: &Mob, attribute: i32) -> Vec<Modifier> {
    KINDS.iter().filter_map(|kind| {
        let (id, name, amount, operation) = kind.modifier()?;
        let level = mob.vitals.effect_level(*kind);
        (id == attribute && level > 0).then(|| Modifier::new(name, f64::from(amount as f32) * f64::from(level), operation))
    }).collect()
}

pub(super) fn speed_base(mob: &Mob) -> f64 {
    if crate::mob::horse::equine(mob.kind) { mob.horse.speed }
    else if crate::mob::slime::cube(mob.kind) { crate::mob::slime::speed(mob.nether.size) }
    else if mob.kind == Kind::Wither { 0.6 }
    else { f64::from(super::mobs::attribute_speed(mob.kind) as f32) }
}

pub(super) fn speed_modifier(mob: &Mob) -> Option<(&'static str, f64, u8)> {
    match mob.kind {
        Kind::Witch if mob.h.drinking > 0 => Some(("minecraft:drinking", -0.25, 0)),
        Kind::Enderman if mob.h.target.is_some() || mob.newmobs.active => Some(("minecraft:attacking", crate::mob::hostile::ATTACKING_BONUS, 0)),
        Kind::Zombie | Kind::ZombieVillager | Kind::Drowned | Kind::Husk if mob.is_baby() => Some(("minecraft:baby", 0.5, 1)),
        _ => None,
    }
}

pub(super) fn changed(mob: &Mob, before: [i32; 4]) -> Vec<Attribute> {
    let now = levels(mob);
    let mut ids = Vec::new();
    for (i, kind) in KINDS.iter().enumerate() {
        let id = kind.modifier().unwrap().0;
        if now[i] != before[i] && !ids.contains(&id) { ids.push(id); }
    }
    ids.into_iter().map(|id| {
        let base = match id {
            attributes::MOVEMENT_SPEED => speed_base(mob),
            attributes::MAX_HEALTH => f64::from(mob.vitals.max_health),
            _ => 0.0,
        };
        let mut modifiers = modifiers(mob, id);
        // Усиленный или новый эффект снимает свой модификатор и ставит заново:
        // он встаёт в конец списка (чёрный ящик, tails-mob-attributes:
        // после повышения скорости `[slowness, speed]`) [проверить: порядок
        // прочих после нескольких правок].
        let moved = |modifier: &Modifier| KINDS.iter().enumerate().any(|(i, kind)| now[i] != before[i] && kind.modifier().is_some_and(|(_, name, _, _)| name == modifier.id));
        modifiers.sort_by_key(|modifier| moved(modifier));
        if id == attributes::MOVEMENT_SPEED && let Some((name, amount, op)) = speed_modifier(mob) {
            modifiers.insert(0, Modifier::new(name, amount, attributes::Operation::from_number(i32::from(op)).unwrap()));
        }
        Attribute { id, base, modifiers }
    }).collect()
}

pub(super) fn push(body: &mut Vec<u8>, mob: &Mob, attribute: i32, native: Option<(&str, f64, u8)>) {
    let modifiers = modifiers(mob, attribute);
    body.extend_from_slice(&encode_varint((modifiers.len() + usize::from(native.is_some())) as i32));
    if let Some((name, amount, op)) = native {
        body.extend_from_slice(&encode_string(name)); push_f64(body, amount); body.push(op);
    }
    for modifier in modifiers {
        body.extend_from_slice(&encode_string(&modifier.id)); push_f64(body, modifier.amount); body.push(modifier.operation as u8);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn potion_attributes_combine_upgrade_and_clear_without_dirtying_every_tick() {
        let mut mob = Mob::new(1, Kind::Cow, (0.0, 0.0, 0.0));
        let give = |mob: &mut Mob, kind, amplifier| { mob.vitals.give_effect_shown(crate::survival::Effect { kind, amplifier, ticks_left: 100, ambient: false, visible: true, show_icon: true }); };
        give(&mut mob, EffectKind::Speed, 0); give(&mut mob, EffectKind::Slowness, 1);
        let attrs = changed(&mob, [0;4]);
        assert_eq!(attrs.len(), 1); assert_eq!(attrs[0].modifiers.len(), 2);
        assert!((attrs[0].value() - speed_base(&mob) * 1.2 * 0.7).abs() < 1e-7);
        let old = levels(&mob); assert!(changed(&mob, old).is_empty());
        mob.vitals.effects.clear();
        let attrs = changed(&mob, old); assert_eq!(attrs.len(), 1); assert!(attrs[0].modifiers.is_empty());
        give(&mut mob, EffectKind::HealthBoost, 0); give(&mut mob, EffectKind::Absorption, 0);
        assert_eq!(changed(&mob, [0;4]).len(), 2);
    }
}
