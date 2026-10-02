// Бой игроков: удар по игроку, щит, броня, отдача, износ вещей.
//
// Нападающий (`strike_player`) считает свой удар — урон из атрибутов и
// зачарований, заряд руки, критический удар, рывок — и кладёт `PlayerStrike`
// в журнал мира. Жертва (`take_strike`) в свой черёд читает журнал и
// считает то, что видно только ей: щит, броню и зачарования защиты, износ,
// отдачу, смерть. Звуки и частицы удара ставит тоже жертва — от них зависит,
// прошёл ли удар (внутри неуязвимости он «пустой»), — и ставит в порядке
// оригинала.
//
// Что не сверено с оригиналом, помечено «[проверить]».

use super::*;

use crate::combat::{self, PlayerStrike};
use crate::survival::Source;
use crate::world::World;

/// Досягаемость удара по существу в выживании (атрибут
/// `entity_interaction_range`) и запас на задержку сети. [проверить]
const REACH_SLACK: f64 = 1.0;

/// Ближайшее расстояние от точки до ящика игрока (0,6 × 1,8; присев — 1,5).
fn distance_to_box(from: (f64, f64, f64), feet: (f64, f64, f64), sneaking: bool) -> f64 {
    let height = if sneaking { 1.5 } else { 1.8 };
    let gap = |value: f64, low: f64, high: f64| (low - value).max(value - high).max(0.0);

    let dx = gap(from.0, feet.0 - 0.3, feet.0 + 0.3);
    let dy = gap(from.1, feet.1, feet.1 + height);
    let dz = gap(from.2, feet.2 - 0.3, feet.2 + 0.3);

    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// Может ли игрок в этом состоянии бить с критом (вики, Critical hit): падает,
/// не на земле, не бежит, не в воде.
fn may_crit(player: &PlayerState) -> bool {
    player.vit.fall > 0.0 && !player.on_ground && !player.vit.sprinting && !player.net.in_water
}

/// Удар игрока по игроку `target`. Возвращает, был ли такой игрок в
/// досягаемости (тогда удар отдан жертве и рука разряжена).
pub(super) fn strike_player(shared: &Shared, player: &mut PlayerState, target: i32, now: u64) -> bool {
    let found = shared.players.lock().expect("список игроков захвачен другим потоком").by_entity(target);
    let Some((victim, gear)) = found else {
        return false;
    };

    if victim.uuid == player.uuid || victim.dimension != player.dimension {
        return false;
    }

    let eye = (player.x, player.y + if player.sneaking { 1.27 } else { 1.62 }, player.z);
    let range = if player.game_mode == GAME_MODE_CREATIVE { 5.0 } else { player.attrs.value(crate::attributes::ENTITY_INTERACTION_RANGE) };
    let feet = (victim.position.x, victim.position.y, victim.position.z);

    if distance_to_box(eye, feet, victim.sneaking) > range + REACH_SLACK {
        log_debug!("PvP: {} не дотянулся до {}: глаза {:.2} {:.2} {:.2}, ноги цели {:.2} {:.2} {:.2}", player.name, victim.name, eye.0, eye.1, eye.2, feet.0, feet.1, feet.2);

        return false;
    }

    vitals::update_equipment(player, now);

    let since = u32::try_from(now.saturating_sub(player.last_attack)).unwrap_or(u32::MAX);
    let held = player.inventory.held();
    let data = held.map(|stack| stack.extras()).unwrap_or_default();
    let charge = combat::charge(since, player.attrs.value(crate::attributes::ATTACK_SPEED));
    let enchant = combat::enchant_bonus(&data, false, false);
    // Сокрушающий удар булавы (вики, Mace): в пролёте от 1,5 блока прибавка
    // за каждый блок падения идёт в основу (и под крит), вид урона —
    // mace_smash, пролёт гасится (чёрный ящик, pvp-mace).
    let mace = held.and_then(|stack| combat::item_name(stack.item)) == Some("mace");
    let smash = combat::smash_ready(mace, player.vit.fall, player.on_ground);
    let bonus = if smash { combat::smash_bonus(player.vit.fall, data.level("density")) } else { 0.0 };
    let strike = combat::strike(player.attrs.value(crate::attributes::ATTACK_DAMAGE), bonus, enchant, charge, may_crit(player));
    let source = if smash { Source::MaceSmash } else { Source::PlayerAttack };
    let sprint_hit = player.vit.sprinting && strike.full;
    let extra = combat::extra_knockback(player.attrs.value(crate::attributes::ATTACK_KNOCKBACK), &data, sprint_hit);
    let sweep = strike.full && !strike.crit && !sprint_hit && player.on_ground && held.and_then(|stack| combat::item_name(stack.item)).is_some_and(|name| name.ends_with("_sword"));
    let info = held.and_then(|stack| combat::info(stack.item));
    let disables = info.map_or(0.0, |info| info.disable_blocking_seconds);

    player.last_attack = now;

    // Исход удара — по тому, что жертва опубликовала (броня, щит,
    // неуязвимость): от него зависят звуки и частицы, которые ставит нападающий
    // сразу за ударом, в том же такте (чёрный ящик, pvp-fist).
    let outcome = combat::outcome(&gear.view, strike.damage, source, now, (victim.position.x, victim.position.z, victim.position.yaw), (player.x, player.z));
    let lands = matches!(outcome, combat::Outcome::Taken(_));

    if lands {
        player.vit.add_exhaustion(0.1);
    }

    // Сокрушил: пролёт считается заново от этого места — падение дальше не
    // ранит, — и клиенту уходит скорость (0; 0,01; 0): вертикальный ход
    // гасится (чёрный ящик, pvp-mace: «b: Velocity 0,0.01,0», урона от
    // падения с 4 и 8 блоков нет).
    let smash_fall = player.vit.fall;

    if smash && lands {
        player.vit.fall = 0.0;
        player.vit.smash_forgives = true;
        player.net.velocity = Some((0.0, 0.01, 0.0));
    }

    if sprint_hit && lands {
        player.vit.sprinting = false;
    }

    let fire = data.level("fire_aspect");

    // Шипы жертвы (вики, Thorns): каждая деталь с «шипами» с шансом 15 % за
    // уровень ранит ударившего; урон приходит ему в этом же такте, со звуками
    // у него (чёрный ящик, pvp-thorns).
    let thorns_slots = if lands { thorns_hits(shared, player, &gear.view, (victim.position.x, victim.position.y, victim.position.z), target) } else { Vec::new() };

    // Рывок: звук отдачи звучит до удара и сверх обычного (чёрный ящик,
    // pvp-sprint-knockback).
    if sprint_hit {
        shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком").note_player_sound((player.x, player.y, player.z), "minecraft:entity.player.attack.knockback");
    }

    shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком").note_player_strike(PlayerStrike {
        victim: target,
        attacker: player.entity_id,
        attacker_uuid: player.uuid,
        attacker_name: player.name.clone(),
        from: (player.x, player.y, player.z),
        yaw: player.yaw,
        damage: strike.damage,
        source,
        extra_knockback: extra,
        weapon: held,
        disables_shield_ticks: (disables * 20.0) as u32,
        breach_scale: if smash { combat::breach_scale(data.level("breach")) } else { 1.0 },
        fire_ticks: fire * 80,
        thorns_slots,
        ignores_shield: false,
    });

    // Звуки и частицы удара — в порядке оригинала: ушиб жертвы, звук удара,
    // критическая анимация, взмах, частицы урона (чёрный ящик, pvp-fist).
    {
        let mut world = shared.world_in(player.dimension).lock().expect("мир захвачен другим потоком");
        let from = (player.x, player.y, player.z);
        let at = (victim.position.x, victim.position.y, victim.position.z);

        match outcome {
            combat::Outcome::Missed => world.note_player_sound(from, "minecraft:entity.player.attack.nodamage"),
            // Щит держит: звук щита (сломан топором — сперва звук поломки),
            // и нападающему — пустой удар (чёрный ящик, pvp-shield).
            combat::Outcome::Blocked => {
                let pitch = 0.8 + world.random_unit() * 0.4;

                if disables > 0.0 {
                    world.note_sound_player_at(at, "minecraft:item.shield.break", 0.8, pitch);
                }

                world.note_sound_player_at(at, "minecraft:item.shield.block", 1.0, pitch);
                world.note_player_sound(from, "minecraft:entity.player.attack.nodamage");
            }
            _ => {
                if let combat::Outcome::Taken(_) = outcome {
                    world.note_sound_player_except(at, "minecraft:entity.player.hurt", 1.0, 1.0, target);
                }

                let sound = if sweep {
                    "minecraft:entity.player.attack.sweep"
                } else if strike.crit {
                    "minecraft:entity.player.attack.crit"
                } else if strike.full {
                    "minecraft:entity.player.attack.strong"
                } else {
                    "minecraft:entity.player.attack.weak"
                };

                world.note_player_sound(from, sound);

                if sweep {
                    let main = MainTarget { entity_id: target, feet: at, size: (0.6, if victim.sneaking { 1.5 } else { 1.8 }) };
                    let damage = sweep_damage(player.attrs.value(crate::attributes::ATTACK_DAMAGE), &data);

                    sweep_players(shared, &mut world, player, &main, damage, now);
                    sweep_mobs(&mut shared.items_in(player.dimension).lock().expect("предметы захвачены другим потоком"), player, &main, damage);
                    shared.items_in(player.dimension).lock().expect("предметы захвачены другим потоком").flush_mob_events(&mut || shared.next_entity_id(), &mut crate::mob::Rng::new(u64::from(shared.next_entity_id() as u32)));

                    let rad = f64::from(player.yaw).to_radians();

                    world.note_particle(
                        crate::mob::Particle::SweepAttack.id(),
                        None,
                        (from.0 - rad.sin(), from.1 + 0.9, from.2 + rad.cos()),
                        ((-rad.sin()) as f32, 0.0, rad.cos() as f32),
                        0.0,
                        0,
                    );
                }

                if strike.crit {
                    world.note_animation(target, 4);
                }

                // Сокрушающий удар: звук у нападающего и частицы из блока под
                // жертвой (событие мира 2013) — после крит-анимации, до частиц
                // урона (чёрный ящик, pvp-mace).
                if smash {
                    world.note_sound_player_at(from, combat::smash_sound(smash_fall, victim.position.on_ground), 1.0, 1.0);

                    if victim.position.on_ground {
                        world.note_level_event(at.0.floor() as i32, at.1.floor() as i32 - 1, at.2.floor() as i32, 2013, combat::SMASH_PARTICLES);
                    }

                    // «Порыв ветра»: взрыв ветра у нападающего подбрасывает
                    // его (вики, Wind Burst) — за частицами удара о землю, до
                    // частиц урона (чёрный ящик, pvp-mace-wind-burst). Центр —
                    // на 0,1 ниже ног: удар и звук на высоте 2,5…2,625, взрыв
                    // на 2,407 [проверить].
                    let wind = data.level("wind_burst");

                    if wind > 0 && lands {
                        let targets: Vec<crate::tnt::Target> = shared
                            .players
                            .lock()
                            .expect("список игроков захвачен другим потоком")
                            .members()
                            .iter()
                            .filter(|member| member.dimension == player.dimension && member.game_mode != 3)
                            .map(|member| {
                                let me = member.entity_id == player.entity_id;

                                crate::tnt::Target {
                                    entity_id: member.entity_id,
                                    x: if me { player.x } else { member.position.x },
                                    y: if me { player.y } else { member.position.y },
                                    z: if me { player.z } else { member.position.z },
                                    sneaking: member.sneaking,
                                }
                            })
                            .collect();
                        let mut items = shared.items_in(player.dimension).lock().expect("предметы захвачены другим потоком");

                        crate::tnt::wind_burst(&mut world, &mut items, &targets, (from.0, from.1 - 0.1, from.2), crate::tnt::wind_burst_knockback(wind));
                    }
                }

                if extra_enchant_hit(&data) {
                    world.note_animation(target, 5);
                }

                // Частиц урона — половина снятого, но не больше, чем было
                // здоровья (чёрный ящик, pvp-mace-heavy: 32,5 урона по 20
                // здоровья — 10 частиц).
                if let combat::Outcome::Taken(taken) = outcome {
                    let count = (taken.min(gear.health) * 0.5) as i32;

                    if count > 0 {
                        world.note_particle(crate::mob::Particle::DamageIndicator.id(), None, (at.0, at.1 + 0.9, at.2), (0.1, 0.0, 0.1), 0.2, count);
                    }
                }
            }
        }
    }

    // Износ оружия за удар: `weapon.item_damage_per_attack` (меч 1, орудия
    // 2); у вещей без такого довеска удар их не изнашивает.
    if lands
        && let Some(stack) = held
        && let Some(info) = info
        && info.max_damage > 0
        && info.damage_per_attack > 0
        && player.game_mode != GAME_MODE_CREATIVE
    {
        wear_held(shared, player, stack, info.damage_per_attack, false);
    }

    true
}

/// Снимает прочность с предмета в руке (`armor` — броня, другой шанс
/// зачарования «прочность»); сломанный исчезает со звуком.
pub(super) fn wear_held(shared: &Shared, player: &mut PlayerState, stack: Stack, amount: i32, armor: bool) {
    let slot = player.inventory.selected_slot();

    wear_slot(shared, player, slot, stack, amount, armor);
}

/// То же для предмета в любом слоте.
pub(super) fn wear_slot(shared: &Shared, player: &mut PlayerState, slot: i32, stack: Stack, amount: i32, armor: bool) {
    let dimension = player.dimension;
    let mut roll = || vitals::world_random(shared, dimension);
    let worn = combat::wear(&stack, amount, armor, &mut roll);

    if worn > 0 {
        replace_worn(shared, player, slot, stack, worn);
    }
}

/// Добавляет износ вещи в слоте; сломавшаяся пропадает.
pub(super) fn replace_worn(shared: &Shared, player: &mut PlayerState, slot: i32, stack: Stack, worn: i32) {
    match combat::add_damage(stack, worn) {
        Some(next) => {
            player.inventory.set(slot, Some(next));
        }
        None => {
            player.inventory.set(slot, None);
            shared
                .world_in(player.dimension)
                .lock()
                .expect("мир захвачен другим потоком")
                .note_sound_player_at((player.x, player.y, player.z), "minecraft:entity.item.break", 0.8, 1.0);
        }
    }

    player.net.slot_changed(slot);
}

/// Жертва получает удар из журнала мира: щит, броня, зачарования, износ,
/// урон, отдача, звуки. Ничего не делает, если удар не ей.
pub(super) async fn take_strike(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState, strike: &PlayerStrike) -> io::Result<()> {
    if strike.victim != player.entity_id {
        return Ok(());
    }

    let mortal = player.game_mode == GAME_MODE_SURVIVAL || player.game_mode == GAME_MODE_ADVENTURE;
    let mut damage = strike.damage;
    let now = *shared.ticks.borrow();

    // Щит (вики, Shield): поднятый дольше 5 тактов держит удар из передней
    // полусферы — урона нет, щит изнашивается, а топор отключает его.
    if mortal
        && strike.source != Source::Thorns
        && !strike.ignores_shield
        && !player.vit.dead
        && damage > 0.0
        && let Some(hand) = vitals::shield_up(player, now)
        && combat::shield_covers(player.yaw, (player.x, player.z), (strike.from.0, strike.from.2))
    {
        return block_with_shield(stream, shared, player, strike, hand, now).await;
    }

    // Броня и зачарования защиты (вики, Armor).
    let (armor, toughness) = (player.attrs.value(crate::attributes::ARMOR) as f32, player.attrs.value(crate::attributes::ARMOR_TOUGHNESS) as f32);

    if mortal && !combat::bypasses_armor(strike.source) {
        damage = combat::after_armor_breached(damage, armor, toughness, strike.breach_scale);

        let points: i32 = combat::ARMOR_SLOTS.iter().filter_map(|(slot, _)| player.inventory.slot(*slot)).map(|stack| combat::protection_points(&stack.extras(), strike.source)).sum();

        damage = combat::after_protection(damage, points);
    }

    let killer = vitals::Killer { name: &strike.attacker_name, uuid: strike.attacker_uuid, weapon: strike.weapon };
    let at = (player.x, player.y, player.z);
    let from = strike.from;
    // Анимация ушиба: откуда пришёл удар относительно взгляда жертвы.
    let toward = (from.2 - at.2).atan2(from.0 - at.0).to_degrees() as f32;
    let yaw = (toward - player.yaw).rem_euclid(360.0);
    let result = if mortal {
        vitals::hit_by(stream, shared, player, damage, strike.source, strike.attacker, strike.attacker, Some(killer), Some(yaw)).await?
    } else {
        None
    };

    let at = (player.x, player.y, player.z);
    let from = strike.from;

    match result {
        None => {}
        Some(_) => {
            // Износ надетой брони: каждой детали 1 за каждые 4 единицы урона.
            if !combat::bypasses_armor(strike.source) {
                wear_armor(shared, player, strike.damage);
            }

            // Шипы (вики, Thorns): сработавшие детали теряют по 2 прочности;
            // сам урон ударившему он выдаёт себе в такт удара (`strike_player`).
            for slot in &strike.thorns_slots {
                if let Some(stack) = player.inventory.slot(*slot) {
                    replace_worn(shared, player, *slot, stack, 2);
                }
            }

            // Отдача: обычная 0,4 от нападающего, сверх неё — по взгляду.
            let away = (at.0 - from.0, at.2 - from.2);
            let length = away.0.hypot(away.1);
            let dir = if length > 1.0e-6 { (away.0 / length, away.1 / length) } else { crate::mob::look_direction(strike.yaw) };
            let resistance = player.attrs.value(crate::attributes::KNOCKBACK_RESISTANCE);

            vitals::knock_back(player, dir, crate::mob::BASE_KNOCKBACK, resistance);

            if strike.extra_knockback > 0.0 {
                let look = crate::mob::look_direction(strike.yaw);

                if strike.source == Source::Arrow {
                    // «Отбрасывание» лука не гасит скорость, а прибавляет:
                    // 0,6 на уровень вдоль полёта и 0,1 вверх (чёрный ящик,
                    // pvp-bow-punch: (0,874; 0,373) при отбрасывании II).
                    let push = f64::from(strike.extra_knockback) * 0.5 * (1.0 - resistance).max(0.0);

                    if let Some(velocity) = player.net.velocity.as_mut() {
                        *velocity = (velocity.0 + look.0 * push, velocity.1 + 0.1, velocity.2 + look.1 * push);
                    }
                } else {
                    vitals::knock_back(player, look, f64::from(strike.extra_knockback) * 0.5, resistance);
                }
            }

            if strike.fire_ticks > 0 {
                player.vit.ignite(strike.fire_ticks);
            }

            if let Some(velocity) = player.net.velocity.take() {
                // Взрыв ракеты — так же, как стрела (чёрный ящик, firework-crossbow:
                // скорость (−0,012; 0,275; 0,218) на такт позже, отдача от ракеты).
                if strike.source.is_projectile() || strike.source == Source::Fireworks {
                    // Стрела: скорость и свойства идут на такт позже, а скорость
                    // — уже после шага трения и тяжести (чёрный ящик, pvp-bow:
                    // 0,218 = 0,4·0,546; 0,275 = (0,361 − 0,08)·0,98); в теле
                    // застревает стрела.
                    let friction = if player.on_ground { 0.546 } else { 0.91 };

                    player.net.delayed_velocity = Some((now + 1, (velocity.0 * friction, (velocity.1 - 0.08) * 0.98, velocity.2 * friction)));
                    player.net.meta_hold = now + 1;
                    // Пробивающая стрела и трезубец в теле не застревают
                    // (чёрный ящик, crossbow-piercing: Metadata без 12=1).
                    player.net.arrows += i32::from(strike.source == Source::Arrow && !strike.ignores_shield);
                } else {
                    // Скорость уходит сразу, одним пакетом на обе отдачи (чёрный ящик,
                    // pvp-fist: сразу за Hurt Animation).
                    send_velocity(stream, player.entity_id, velocity).await?;
                }
            }
        }
    }

    Ok(())
}

/// Изнашивает надетую броню от удара силы `damage`.
fn wear_armor(shared: &Shared, player: &mut PlayerState, damage: f32) {
    let wear = combat::armor_wear(damage);

    for (slot, _) in combat::ARMOR_SLOTS {
        let Some(stack) = player.inventory.slot(slot) else { continue };
        let dimension = player.dimension;
        let mut roll = || vitals::world_random(shared, dimension);
        let worn = combat::wear(&stack, wear, true, &mut roll);

        if worn > 0 {
            replace_worn(shared, player, slot, stack, worn);
        }
    }
}

/// Щит принял удар: событие «щит принял» всем, износ щита, отключение
/// топором; нападающий слышит обычный звук удара. Урона жертве нет.
async fn block_with_shield(stream: &mut Outgoing<'_>, shared: &Shared, player: &mut PlayerState, strike: &PlayerStrike, hand: i32, now: u64) -> io::Result<()> {
    let slot = if hand == 1 { inventory::OFFHAND } else { player.inventory.selected_slot() };
    if let Some(shield) = player.inventory.slot(slot) {
        let wear = combat::shield_wear(strike.damage);

        if wear > 0 {
            let dimension = player.dimension;
            let mut roll = || vitals::world_random(shared, dimension);
            let worn = combat::wear(&shield, wear, false, &mut roll);

            if worn > 0 {
                replace_worn(shared, player, slot, shield, worn);
            }
        }
    }

    // Топор: щит отключён на 100 тактов, поднятая рука опущена.
    if strike.disables_shield_ticks > 0 {
        player.net.shield_disabled_until = now + u64::from(strike.disables_shield_ticks);
        player.net.using = None;
        vitals::send_cooldown(stream, "minecraft:shield", strike.disables_shield_ticks as i32).await?;
    }

    Ok(())
}

/// Зачарованное оружие даёт частицы «зачарованный удар» (анимация 5): у нас
/// пока острота, небесная кара и бич членистоногих (вики, Sharpness).
fn extra_enchant_hit(data: &crate::stackdata::Data) -> bool {
    data.level("sharpness") > 0 || data.level("smite") > 0 || data.level("bane_of_arthropods") > 0
}

/// Куда попал основной удар: место ног цели и размер её ящика.
#[derive(Clone, Copy)]
pub(super) struct MainTarget {
    pub entity_id: i32,
    pub feet: (f64, f64, f64),
    pub size: (f64, f64),
}

/// Урон взмаха меча по остальным (вики, Sweep attack): `1 + доля · урон`,
/// где доля у «разящего клинка» `L/(L+1)`; задеваются существа в ящике
/// цели, расширенном на (1; 0,25; 1), не дальше 3 блоков от нападающего.
pub(super) fn sweep_damage(attack_damage: f64, data: &crate::stackdata::Data) -> f32 {
    let level = data.level("sweeping_edge") as f32;
    let ratio = level / (level + 1.0);

    1.0 + ratio * attack_damage as f32
}

fn in_sweep_box(main: &MainTarget, feet: (f64, f64, f64), size: (f64, f64), attacker: (f64, f64, f64)) -> bool {
    let reach_squared = (feet.0 - attacker.0).powi(2) + (feet.1 - attacker.1).powi(2) + (feet.2 - attacker.2).powi(2);

    reach_squared < 9.0
        && (feet.0 - main.feet.0).abs() <= (size.0 + main.size.0) / 2.0 + 1.0
        && (feet.2 - main.feet.2).abs() <= (size.0 + main.size.0) / 2.0 + 1.0
        && feet.1 < main.feet.1 + main.size.1 + 0.25
        && feet.1 + size.1 > main.feet.1 - 0.25
}

/// Игрок рядом со взмахом: номер, место, приседает ли, режим, бой.
type Bystander = (i32, (f64, f64, f64), bool, i32, crate::players::CombatView);

/// Взмах по другим игрокам: удар с уроном взмаха и отдачей вдоль взгляда.
pub(super) fn sweep_players(shared: &Shared, world: &mut World, player: &PlayerState, main: &MainTarget, damage: f32, now: u64) {
    let victims: Vec<Bystander> = {
        let players = shared.players.lock().expect("список игроков захвачен другим потоком");

        players
            .members()
            .iter()
            .filter(|member| member.uuid != player.uuid && member.entity_id != main.entity_id && member.dimension == player.dimension)
            .map(|member| (member.entity_id, (member.position.x, member.position.y, member.position.z), member.sneaking, member.game_mode, players.gear(&member.uuid).view))
            .collect()
    };

    for (entity_id, feet, sneaking, mode, view) in victims {
        let size = (0.6, if sneaking { 1.5 } else { 1.8 });

        if mode == GAME_MODE_SPECTATOR || !in_sweep_box(main, feet, size, (player.x, player.y, player.z)) {
            continue;
        }

        if combat::outcome(&view, damage, Source::PlayerAttack, now, (feet.0, feet.2, 0.0), (player.x, player.z)) == combat::Outcome::Missed {
            continue;
        }

        world.note_sound_player_except(feet, "minecraft:entity.player.hurt", 1.0, 1.0, entity_id);
        world.note_player_strike(PlayerStrike {
            victim: entity_id,
            attacker: player.entity_id,
            attacker_uuid: player.uuid,
            attacker_name: player.name.clone(),
            from: (player.x, player.y, player.z),
            yaw: player.yaw,
            damage,
            source: Source::PlayerAttack,
            // Обычная отдача 0,4 от нападающего и ещё 0,4 вдоль взгляда:
            // жертва берёт половину `extra_knockback`.
            extra_knockback: 0.8,
            weapon: player.inventory.held(),
            disables_shield_ticks: 0,
            breach_scale: 1.0,
            fire_ticks: 0,
            thorns_slots: Vec::new(),
            ignores_shield: false,
        });
    }
}

/// Взмах по мобам: урон и отдача, как у обычного удара, и ещё толчок вдоль взгляда.
pub(super) fn sweep_mobs(items: &mut crate::items::Items, player: &PlayerState, main: &MainTarget, damage: f32) {
    let attacker = (player.x, player.y, player.z);
    let look = crate::mob::look_direction(player.yaw);

    for mob in items.mobs_mut() {
        if mob.entity_id == main.entity_id || mob.is_dead() || !in_sweep_box(main, (mob.x, mob.y, mob.z), mob.size(), attacker) {
            continue;
        }

        let away = (mob.x - attacker.0, mob.z - attacker.2);
        let length = away.0.hypot(away.1);
        let dir = if length > 1.0e-6 { (away.0 / length, away.1 / length) } else { look };
        let push = crate::mob::Push { dir, strength: crate::mob::BASE_KNOCKBACK, cause: player.entity_id };

        mob.credit_player();

        if mob.hurt(damage, Source::PlayerAttack, Some(push)).is_some() {
            mob.knockback(look.0, look.1, crate::mob::BASE_KNOCKBACK);
        }
    }
}

/// Бросает шипы жертвы по её опубликованным уровням: сработавшим кладёт урон
/// в очередь ударов нападающего; возвращает слоты сработавших.
fn thorns_hits(shared: &Shared, player: &mut PlayerState, view: &crate::players::CombatView, victim_at: (f64, f64, f64), victim: i32) -> Vec<i32> {
    let mut slots = Vec::new();
    let dimension = player.dimension;

    for ((slot, _), level) in combat::ARMOR_SLOTS.iter().zip(view.thorns) {
        if level <= 0 || vitals::world_random(shared, dimension) >= 0.15 * level as f32 {
            continue;
        }

        let amount = if level > 10 { (level - 10) as f32 } else { combat::thorns_damage(level, vitals::world_random(shared, dimension)) };
        let toward = (victim_at.2 - player.z).atan2(victim_at.0 - player.x).to_degrees() as f32;
        let at = (player.x, player.y, player.z);
        let mut world = shared.world_in(dimension).lock().expect("мир захвачен другим потоком");

        // Остальные игроки видят урон ударившего в такт удара, а не на такт
        // позже, когда он применится у него самого (чёрный ящик, pvp-thorns:
        // «other damage_event» и здоровье в одном сбросе с остальным): урон
        // заранее прикидывается на копии состояния.
        let mut probe = player.vit.clone();
        let immune = player.game_mode == GAME_MODE_CREATIVE || player.game_mode == GAME_MODE_SPECTATOR;
        let announced = probe.hurt(amount, Source::Thorns, immune).is_some();

        if announced {
            let (type_id, culprit, direct) = vitals::damage_fields(Source::Thorns, victim, 0);

            world.note_player_damaged(player.entity_id, type_id, culprit, direct);
            player.net.announced_health = Some(probe.health);
            shared.players.lock().expect("список игроков захвачен другим потоком").set_health(&player.uuid, probe.health);
        }

        // Звуки идут в журнал сразу: клиенты получат их в этом же такте.
        world.note_sound_player_except(at, "minecraft:entity.player.hurt", 1.0, 1.0, player.entity_id);
        world.note_sound_player_at(at, "minecraft:enchant.thorns.hit", 1.0, 1.0);
        drop(world);

        player.net.hits.push(vitals::Pending {
            amount,
            source: Source::Thorns,
            direct: victim,
            hurt_yaw: Some((toward - player.yaw).rem_euclid(360.0)),
            push: {
                let away = (player.x - victim_at.0, player.z - victim_at.2);
                let length = away.0.hypot(away.1);

                Some(if length > 1.0e-6 { (away.0 / length, away.1 / length) } else { crate::mob::look_direction(player.yaw) })
            },
            announced,
            killer: shared.players.lock().expect("список игроков захвачен другим потоком").by_entity(victim).map(|(member, _)| (member.name.clone(), member.uuid)),
        });
        slots.push(*slot);
    }

    slots
}
