use super::*;
use crate::combat::DamageOrigin;
use crate::survival::Source as DamageType;

struct Target {
    id: i32,
    at: (f64, f64, f64),
    player: Option<(String, [u8; 16])>,
}

fn entities(text: &str, source: &Source, shared: &Shared) -> Vec<Target> {
    let origin = origin_of(source, shared);
    let dimension = dimension_of(source, shared);
    let chosen = Selector::parse(text);
    let players = find_targets(text, source, shared);
    let mut found: Vec<Target> = {
        let all = shared.players.lock().expect("игроки");
        all.members().iter().filter(|p| p.dimension == dimension && players.iter().any(|(id, _)| *id == p.uuid))
            .map(|p| Target { id: p.entity_id, at: (p.position.x, p.position.y, p.position.z), player: Some((p.name.clone(), p.uuid)) }).collect()
    };
    if let Some(chosen) = &chosen {
        let items = shared.items_in(dimension).lock().expect("сущности");
        found.extend(items.mobs().iter().filter(|mob| !mob.is_dead() && chosen.takes(mob.kind.name()) && chosen.reaches(origin, (mob.x, mob.y, mob.z)))
            .map(|mob| Target { id: mob.entity_id, at: (mob.x, mob.y, mob.z), player: None }));
        if text.contains("sort=nearest") || text.starts_with("@n") {
            let distance = |p: &Target| (p.at.0 - origin.0).powi(2) + (p.at.1 - origin.1).powi(2) + (p.at.2 - origin.2).powi(2);
            found.sort_by(|a, b| distance(a).total_cmp(&distance(b)));
        }
        if let Some(limit) = chosen.limit { found.truncate(limit); }
    }
    found
}

fn single(text: &str, source: &Source, shared: &Shared) -> Option<Target> {
    let mut targets = entities(text, source, shared);
    (targets.len() == 1).then(|| targets.remove(0))
}

fn damage_type(name: &str) -> Option<DamageType> {
    Some(match name.strip_prefix("minecraft:").unwrap_or(name) {
        "generic" => DamageType::Generic, "generic_kill" => DamageType::GenericKill,
        "player_attack" => DamageType::PlayerAttack, "mob_attack" => DamageType::MobAttack,
        "mob_attack_no_aggro" => DamageType::MobAttackNoAggro,
        "explosion" => DamageType::Explosion, "player_explosion" => DamageType::PlayerExplosion,
        "arrow" => DamageType::Arrow, "trident" => DamageType::Trident, "thrown" => DamageType::Thrown,
        "fall" => DamageType::Fall, "drown" => DamageType::Drown, "cactus" => DamageType::Cactus,
        "sweet_berry_bush" => DamageType::SweetBerryBush, "starve" => DamageType::Starve,
        "in_fire" => DamageType::InFire, "on_fire" => DamageType::OnFire, "lava" => DamageType::Lava,
        "in_wall" => DamageType::InWall, "out_of_world" => DamageType::OutOfWorld,
        "outside_border" => DamageType::OutsideBorder, "magic" => DamageType::Magic,
        "indirect_magic" => DamageType::IndirectMagic, "wither" => DamageType::Wither,
        "wither_skull" => DamageType::WitherSkull, "thorns" => DamageType::Thorns,
        "fireball" => DamageType::Fireball, "fireworks" => DamageType::Fireworks,
        "lightning_bolt" => DamageType::Lightning, "mace_smash" => DamageType::MaceSmash,
        "fly_into_wall" => DamageType::FlyIntoWall, "sonic_boom" => DamageType::SonicBoom,
        _ => return None,
    })
}

fn origin(parts: &[&str], source: &Source, shared: &Shared) -> Option<DamageOrigin> {
    match parts {
        [] => Some(DamageOrigin::default()),
        ["at", x, y, z] => Some(DamageOrigin { at: Some(entity_place(&[x, y, z], origin_of(source, shared))?), ..Default::default() }),
        ["by", entity] | ["by", entity, "from", _] => {
            let direct = single(entity, source, shared)?;
            let cause = if parts.len() == 4 { single(parts[3], source, shared)? } else { Target { id: direct.id, at: direct.at, player: direct.player.clone() } };
            Some(DamageOrigin { direct: direct.id, cause: cause.id, at: None, entity_at: Some(direct.at), player: cause.player })
        }
        _ => None,
    }
}

pub(super) fn run(parts: &[&str], source: &Source, shared: &Shared) -> Answer {
    let fail = || Answer::saying("Нужно: damage <цель> <урон> [тип] [at <x> <y> <z> | by <сущность> [from <причина>]]; цель должна быть одна".into());
    let Some(amount) = parts.get(1).and_then(|v| v.parse::<f32>().ok()).filter(|v| v.is_finite() && *v >= 0.0) else { return fail(); };
    let Some(kind) = damage_type(parts.get(2).copied().unwrap_or("generic")) else { return fail(); };
    let Some(origin) = origin(parts.get(3..).unwrap_or(&[]), source, shared) else { return fail(); };
    let Some(target) = single(parts[0], source, shared) else { return fail(); };
    if let Some((name, uuid)) = target.player {
        shared.players.lock().expect("игроки").order(&uuid, Order::CommandDamage { amount, source: kind, origin });
        return Answer::saying(format!("Нанесён урон: {name}"));
    }
    let dimension = dimension_of(source, shared);
    let mut items = shared.items_in(dimension).lock().expect("сущности");
    let Some(mob) = items.mobs_mut().iter_mut().find(|mob| mob.entity_id == target.id) else { return fail(); };
    // Команда не выбирает части дракона (существующий проверенный путь).
    if mob.kind == crate::mob::Kind::EnderDragon { return Answer::saying("Цель не получила урон".into()); }
    let push = origin.entity_at.or(origin.at).map(|at| {
        let (dx, dz) = (mob.x - at.0, mob.z - at.2);
        let length = dx.hypot(dz).max(1e-6);
        crate::mob::Push { dir: (dx / length, dz / length), strength: crate::mob::BASE_KNOCKBACK, cause: origin.cause }
    });
    let before = mob.events.len();
    let hit = mob.hurt(amount, kind, push);
    if hit.is_some() {
        if let Some((_, uuid)) = origin.player { mob.credit_to(uuid); }
        for event in &mut mob.events[before..] {
            if matches!(event, crate::mob::Event::Hurt { .. }) {
                *event = crate::mob::Event::CommandHurt { source: kind, origin: origin.clone() };
            }
        }
    }
    items.flush_mob_events(&mut || shared.next_entity_id(), &mut crate::mob::Rng::new(u64::from(shared.next_entity_id() as u32)));
    Answer::saying(if hit.is_some() { "Нанесён урон сущности" } else { "Цель не получила урон" }.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn forms_validate_amount_type_and_keep_both_sources() {
        let shared = super::super::tests::server();
        assert!(run(&["Первый", "2", "player_attack", "by", "Второй"], &Source::Console, &shared).reply.contains("Нанесён"));
        let orders = shared.players.lock().unwrap().take_orders(&[1; 16]);
        assert!(matches!(&orders[0], Order::CommandDamage { origin, .. } if origin.direct == 2 && origin.cause == 2));
        let parsed = origin(&["by", "Второй", "from", "Первый"], &Source::Console, &shared).unwrap();
        assert_eq!((parsed.direct, parsed.cause), (2, 1));
        assert_eq!(origin(&["at", "~1", "~2", "~3"], &Source::Console, &shared).unwrap().at, Some((1.0, 2.0, 3.0)));
        for bad in ["NaN", "inf", "-1"] { assert!(run(&["Первый", bad], &Source::Console, &shared).reply.contains("Нужно")); }
        assert!(run(&["Первый", "1", "nonexistent"], &Source::Console, &shared).reply.contains("Нужно"));
        assert!(origin(&["by", "nobody"], &Source::Console, &shared).is_none());
        assert!(origin(&["at", "0", "0"], &Source::Console, &shared).is_none());
    }

    #[test]
    fn mob_damage_keeps_credit_and_explicit_location_in_the_journal() {
        let shared = super::super::tests::server();
        let dimension = Dimension::Overworld;
        let reader = 71;
        let from = shared.items_in(dimension).lock().unwrap().watch_changes(reader);
        shared.items_in(dimension).lock().unwrap().add_mob(crate::mob::Mob::new(100, crate::mob::Kind::Cow, (0.0, 64.0, 0.0)));
        assert!(run(&["@e[type=cow,limit=1]", "2", "player_attack", "by", "Первый"], &Source::Console, &shared).reply.contains("Нанесён"));
        let mut items = shared.items_in(dimension).lock().unwrap();
        assert_eq!(items.mobs()[0].vitals.health, 8.0);
        let (events, _) = items.changes_since(from, reader);
        assert!(events.iter().any(|event| matches!(event, crate::items::Change::MobEvent { event: crate::mob::Event::CommandHurt { origin, .. }, .. } if origin.cause == 1 && origin.direct == 1)));
        items.add_mob(crate::mob::Mob::new(101, crate::mob::Kind::Pig, (0.0, 64.0, 0.0)));
        drop(items);
        run(&["@e[type=pig,limit=1]", "1", "generic", "at", "1.0", "64", "2.0"], &Source::Console, &shared);
        let (events, _) = shared.items_in(dimension).lock().unwrap().changes_since(from, reader);
        assert!(events.iter().any(|event| matches!(event, crate::items::Change::MobEvent { event: crate::mob::Event::CommandHurt { origin, .. }, .. } if origin.at == Some((1.0, 64.0, 2.0)) && origin.direct == 0)));
    }
}
