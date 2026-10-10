// Варианты скелета: правила minecraft.wiki, Stray и Bogged. ИИ и полёт
// стрел общие (hostile.rs, arrow.rs). [проверить] Такты и пакеты — сценариями
// stray-slowness, bogged-poison, bogged-shear, skeleton-to-stray; здесь не запускались.

use super::{Event, Kind, Mob, Rng, View};
use crate::blocks;
use crate::inventory::Stack;
use crate::survival::{Effect, EffectKind};

/// Вики: 140 тактов; у оригинала дрожь на 142-м такте после появления, превращение на 442-м
/// (чёрный ящик, skeleton-to-stray), отсюда 142.
pub const FREEZE_TICKS: i32 = 142;
pub const CONVERT_TICKS: i32 = 300;

/// Вики, Bogged: «cooldown is 3.5 seconds ... or 2.5 seconds on Hard».
/// Натяжка остаётся общей, пауза на 30 тактов длиннее скелета.
pub fn shoot_pause(kind: Kind, difficulty: u8) -> i32 {
    super::hostile::shoot_pause(difficulty) + if kind == Kind::Bogged { 30 } else { 0 }
}

/// Собственный эффект моба, не восьмая часть срока питьевого зелья: у зимогора
/// слабость 600, у болотника отрава 100 (чёрный ящик, bogged-poison: `dur=100`).
pub fn arrow_effect(kind: Kind) -> Option<Effect> {
    let (kind, ticks_left) = match kind {
        Kind::Stray => (EffectKind::Slowness, 600),
        Kind::Bogged => (EffectKind::Poison, 100),
        _ => return None,
    };
    Some(Effect { kind, amplifier: 0, ticks_left, ambient: false, visible: true })
}

/// Вики, Stray: «7 seconds (140 game ticks)», затем «15 seconds» дрожи.
/// Выход из снега сбрасывает и счёт, и начатую дрожь: у оригинала `16=false`
/// через 30 тактов после `16=true`, превращения нет (чёрный ящик, skeleton-to-stray).
pub fn snow_tick(mob: &mut Mob, view: &dyn View) {
    if mob.kind != Kind::Skeleton || mob.no_ai || mob.is_dead() {
        return;
    }
    let half = mob.kind.width() / 2.0;
    let mut snow = false;
    for x in (mob.x - half + 1.0e-7).floor() as i32..=(mob.x + half - 1.0e-7).floor() as i32 {
        for y in (mob.y + 1.0e-7).floor() as i32..=(mob.y + mob.kind.height() - 1.0e-7).floor() as i32 {
            for z in (mob.z - half + 1.0e-7).floor() as i32..=(mob.z + half - 1.0e-7).floor() as i32 {
                snow |= blocks::block_at_state(view.block(x, y, z)) == Some("powder_snow");
            }
        }
    }
    if !snow {
        // Чёрный ящик, skeleton-to-stray: `16=false` у оригинала на такт позже выхода из снега.
        if mob.h.converting > 0 && !mob.h.snow_gap {
            mob.h.snow_gap = true;
            return;
        }
        mob.h.snow_gap = false;
        mob.h.water_ticks = 0;
        mob.h.converting = 0;
        return;
    }
    mob.h.snow_gap = false;
    if mob.h.converting > 0 {
        mob.h.converting -= 1;
        if mob.h.converting == 0 {
            mob.h.converted = Some(Kind::Stray);
        }
        return;
    }
    mob.h.water_ticks += 1;
    if mob.h.water_ticks >= FREEZE_TICKS {
        mob.h.converting = CONVERT_TICKS;
    }
}

pub fn read_snow(bytes: &mut &[u8]) -> Option<(i32, i32)> {
    let (head, rest) = bytes.split_at_checked(8)?;
    *bytes = rest;
    Some((i32::from_be_bytes(head[..4].try_into().ok()?).clamp(0, FREEZE_TICKS), i32::from_be_bytes(head[4..].try_into().ok()?).max(0)))
}

/// Вики, Stray, «Spawning»: вес 80 вместо скелетных 100, скелету — 20.
/// Frozen River — только Bedrock; Java: равнины, пики и оба ледяных океана.
pub fn snowy_biome(biome: &str) -> bool {
    matches!(biome, "snowy_plains" | "ice_spikes" | "frozen_ocean" | "deep_frozen_ocean")
}

/// Вики, Bogged: «Each individual mushroom has an equal chance»; всего два.
/// Повторная стрижка ничего не даёт, грибы не отрастают.
pub fn shear(mob: &mut Mob, rng: &mut Rng) -> Option<Vec<Stack>> {
    if mob.kind != Kind::Bogged || mob.sheared || mob.is_dead() {
        return None;
    }
    mob.sheared = true;
    mob.events.push(Event::EntitySound { name: "minecraft:entity.bogged.shear", category: 7, volume: 1.0, pitch: 1.0 });
    Some((0..2).filter_map(|_| {
        let name = if rng.below(2) == 0 { "red_mushroom" } else { "brown_mushroom" };
        blocks::item_named(name).map(|item| Stack::new(item, 1))
    }).collect())
}

/// Общая добыча скелетов: кости, стрелы; Looting добавляет округлённое 0…L.
/// Вики, Bogged/Stray: особая стрела только от игрока, не больше одной.
/// [проверить] Распределение Looting и износ естественного лука — без замера.
pub fn loot(mob: &Mob, rng: &mut Rng) -> Vec<Stack> {
    let mut drops = Vec::new();
    let credited = mob.by_player || mob.xp_until > mob.ticks;
    let looting = if credited { mob.looting.max(0) } else { 0 };
    for name in ["bone", "arrow"] {
        let count = rng.below(3) as i32 + (rng.unit() * f64::from(looting)).round() as i32;
        if count > 0 && let Some(item) = blocks::item_named(name) {
            drops.push(Stack::new(item, count));
        }
    }
    if credited {
        let potion = match mob.kind {
            Kind::Stray => Some("slowness"),
            Kind::Bogged => Some("poison"),
            _ => None,
        };
        // Один успешный бросок из 1+Looting: 50%, 75%, 87,5%, 93,75%.
        if let Some(potion) = potion
            && rng.unit() < 1.0 - 0.5f64.powi(looting.saturating_add(1))
            && let Some(item) = blocks::item_named("tipped_arrow")
        {
            drops.push(Stack::new(item, 1).with_extras(crate::stackdata::Data { potion: crate::survival::potion_number(potion), ..Default::default() }));
        }
        if mob.h.bow && rng.unit() < f64::from(mob.h.hand_drop) + f64::from(looting) * 0.01
            && let Some(item) = blocks::item_named("bow")
        {
            drops.push(Stack::new(item, 1).with_extras(crate::stackdata::Data { damage: 326 + rng.below(58) as i32, ..Default::default() }));
        }
    }
    drops
}
