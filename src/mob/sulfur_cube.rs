//! Куб серы 26.3 (sulfur_cube): прыгающий пассивный моб серных пещер.
//!
//! Источники: minecraft.wiki/w/Sulfur_Cube (26.2/26.3) и один прогон официального
//! 26.3 (tools/protocol-bridge/probe_sulfur_cube_777.py, /summon, /data get, /attribute, /kill).
//! Наблюдено: большой куб — Size 1, здоровье 8, скорость 0,4, прыжок 0,42, тяжесть 0,08;
//! малый — Size 0, здоровье 4, скорость 0,3, Age −24000 (metadata baby=true); у призванного
//! с NBT без `Size` куб малый, без NBT — большой; `/kill` большого даёт двух малых,
//! малые добычи не дают, поглощённый блок выпадает предметом; брошенный предмет
//! поглощается за секунды (сбор + звук); `equipment.body` — слот тела (6).
//! Metadata (777): baby 16, age_locked 17, size 18 (= Size+1), max_fuse 19, from_bucket 20.
//! Прыжки переиспользуют общий путь кубов (`slime::cube`).
//! `[проверить]`: архетипы (скольжение/отскок/трение) и звуки — клиент считает шар сам;
//! сила запуска ударом, список блоков по тегам, ускорение роста слизью, вес спавна в
//! серных пещерах (биома в генерации ещё нет).
use super::ai::{Ctx, Plan};
use super::{Event, Kind, Mob, Rng, View};
use crate::inventory::Stack;

pub const SMALL: i32 = 1;
pub const LARGE: i32 = 2;
/// Малый куб растёт 20 минут (вики: «grow into large ones after 20 minutes»).
pub const GROW_TICKS: i32 = 24_000;
/// Запал динамита при ручном поджоге: 120 тактов (вики, «Manual ignition sets a 120-tick fuse»).
pub const TNT_FUSE: i32 = 120;
/// После снятия блока ножницами куб 100 тактов ничего не подбирает (вики, `pickup_timer`).
pub const PICKUP_PAUSE: i32 = 100;
/// Естественный спавн: вес 100 в серных пещерах, группы 2–4, большие, любой свет (вики).
/// `[проверить]` биом `sulfur_caves` в нашей генерации ещё не существует — правило не подключено.
#[allow(dead_code)] // правило ждёт биома серных пещер в генерации
pub const SPAWN_BIOME: &str = "sulfur_caves";
#[allow(dead_code)]
pub const SPAWN_WEIGHT: u32 = 100;
#[allow(dead_code)]
pub const SPAWN_GROUP: (u32, u32) = (2, 4);

/// Архетип по поглощённому блоку (вики, Sulfur Cube, «Archetypes»).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Archetype {
    Regular,
    Light,
    FastSliding,
    SlowSliding,
    SlowFlat,
    SlowBouncy,
    HighResistance,
    Sticky,
    Explosive,
    Hot,
}

/// Состояние куба серы (хранится в `Mob::sulfur`).
#[derive(Clone, Debug, PartialEq)]
pub struct State {
    /// Поглощённый блок (номер предмета) — `equipment.body`.
    pub body: Option<i32>,
    /// Тактов до взрыва; −1 — не подожжён.
    pub fuse: i32,
    /// Тактов до следующего подбора предмета.
    pub pickup_timer: i32,
    pub from_bucket: bool,
    /// Счётчик прыжков до следующего.
    pub hop: i32,
}

impl Default for State {
    fn default() -> Self { State { body: None, fuse: -1, pickup_timer: 0, from_bucket: false, hop: 20 } }
}

/// Архетип по имени предмета; None — блок поглотить нельзя (плиты и ступени не годятся).
/// Список — по таблице вики; теги блоков целиком не выгружены `[проверить]`.
pub fn archetype(item: &str) -> Option<Archetype> {
    if item.ends_with("_slab") || item.ends_with("_stairs") { return None; }
    // Песок, красный песок, гравий и блок красного камня поглощать нельзя (вики, снимок 26.2-6).
    if matches!(item, "sand" | "red_sand" | "gravel" | "redstone_block") { return None; }
    Some(match item {
        "tnt" => Archetype::Explosive,
        "magma_block" => Archetype::Hot,
        "honeycomb_block" => Archetype::Sticky,
        "soul_sand" | "soul_soil" => Archetype::HighResistance,
        "blue_ice" | "packed_ice" | "snow_block" => Archetype::FastSliding,
        "brown_mushroom_block" | "red_mushroom_block" | "mushroom_stem" | "mycelium" | "nether_wart_block"
        | "warped_wart_block" | "shroomlight" => Archetype::SlowSliding,
        "iron_block" | "gold_block" | "netherite_block" | "ancient_debris" | "raw_iron_block" | "raw_gold_block"
        | "raw_copper_block" | "iron_ore" | "gold_ore" | "copper_ore" | "nether_gold_ore" | "copper_block" | "copper_bulb"
        | "cut_copper" | "chiseled_copper" => Archetype::SlowFlat,
        "mud" | "muddy_mangrove_roots" | "packed_mud" | "coal_block" | "dirt" | "coarse_dirt" | "rooted_dirt" | "podzol"
        | "grass_block" | "clay" | "bone_block" => Archetype::Regular,
        n if n.ends_with("_wool") => Archetype::Light,
        n if n.ends_with("_concrete_powder") => Archetype::Regular,
        "stone" | "andesite" | "diorite" | "granite" | "bricks" | "deepslate" | "tuff" | "sandstone" | "quartz_block"
        | "coal_ore" | "lapis_ore" | "redstone_ore" | "diamond_ore" | "emerald_ore" | "redstone_lamp" | "observer"
        | "cobblestone" => Archetype::SlowBouncy,
        n if n.ends_with("_concrete") || n.ends_with("_terracotta") => Archetype::SlowBouncy,
        _ => return None,
    })
}

pub fn absorbable(item: &str) -> bool { archetype(item).is_some() }

/// Размер: 1 — малый, 2 — большой. Здоровье 4·размер, малый — «детёныш» (Age −24000).
pub fn set_size(mob: &mut Mob, size: i32) {
    mob.nether.size = size.clamp(SMALL, LARGE);
    mob.vitals.max_health = 4.0 * mob.nether.size as f32;
    mob.vitals.health = mob.vitals.max_health;
    mob.age = if mob.nether.size == SMALL { -GROW_TICKS } else { 0 };
    mob.attr_changes += 1;
}

/// Естественно появившийся и призванный без NBT куб — большой (наблюдение: /summon без данных → Size 1).
pub fn initialize(mob: &mut Mob) { set_size(mob, LARGE); }

/// Размер ящика: 0,49 · размер (малый 0,49, большой 0,98).
pub fn width(size: i32) -> f64 { 0.49 * f64::from(size.clamp(SMALL, LARGE)) }

#[allow(dead_code)]
pub fn can_spawn_in(biome: &str) -> bool { biome == SPAWN_BIOME }

/// Малый куб вырос (Age дошёл до 0).
pub fn grow(mob: &mut Mob) {
    if mob.kind == Kind::SulfurCube && mob.nether.size == SMALL {
        let missing = mob.vitals.max_health - mob.vitals.health;
        set_size(mob, LARGE);
        mob.vitals.health = (mob.vitals.max_health - missing).max(1.0);
    }
}

/// Слизь ускоряет рост малого куба [проверить: на сколько тактов].
pub fn feed_slimeball(mob: &mut Mob) -> bool {
    if mob.kind != Kind::SulfurCube || mob.nether.size != SMALL || mob.age_locked { return false; }
    mob.age = (mob.age + 2_400).min(0);
    true
}

/// Поглотить блок (по одному); только большой куб без блока и без паузы подбора.
pub fn absorb(mob: &mut Mob, item: i32, name: &str) -> bool {
    if mob.kind != Kind::SulfurCube || mob.nether.size != LARGE || mob.sulfur.body.is_some()
        || mob.sulfur.pickup_timer > 0 || mob.is_dead() || !absorbable(name) { return false; }
    mob.sulfur.body = Some(item);
    mob.attr_changes += 1;
    mob.events.push(Event::Sound { name: "minecraft:entity.sulfur_cube.absorb", volume: 1.0, pitch: 1.0 });
    true
}

/// Ножницы снимают блок и дают паузу подбора; с подожжённым динамитом нельзя.
pub fn shear(mob: &mut Mob) -> Option<i32> {
    if mob.sulfur.fuse >= 0 { return None; }
    let body = mob.sulfur.body.take()?;
    mob.sulfur.pickup_timer = PICKUP_PAUSE;
    mob.attr_changes += 1;
    mob.events.push(Event::Sound { name: "minecraft:entity.sulfur_cube.eject", volume: 1.0, pitch: 1.0 });
    Some(body)
}

/// Подожжённый динамит: запал 120 тактов.
pub fn ignite(mob: &mut Mob) -> bool {
    let tnt = mob.sulfur.body.and_then(crate::blocks::item_name) == Some("tnt");
    if !tnt || mob.sulfur.fuse >= 0 { return false; }
    mob.sulfur.fuse = TNT_FUSE;
    true
}

/// С блоком куб устойчив к ударам, снарядам, падающим блокам, чужим взрывам, падению,
/// холоду, кактусу и ягодным кустам (вики); огонь, удушье и эффекты действуют.
pub fn resists(mob: &Mob, source: crate::survival::Source) -> bool {
    use crate::survival::Source as S;
    mob.sulfur.body.is_some() && matches!(source, S::PlayerAttack | S::MobAttack | S::MobAttackNoAggro | S::Arrow | S::Thrown
        | S::MobProjectile | S::Spear | S::MaceSmash | S::FallingBlock | S::FallingAnvil | S::FallingStalactite
        | S::Explosion | S::PlayerExplosion | S::Fall | S::Freeze | S::Cactus | S::SweetBerryBush)
}

/// Удар по куту с блоком запускает его как мяч: сила растёт с уроном `[проверить числа]`.
pub fn launch(mob: &mut Mob, amount: f32, push: Option<super::Push>) {
    if let Some(push) = push {
        mob.knockback(push.dir.0, push.dir.1, push.strength + 0.1 * f64::from(amount));
        mob.vy = mob.vy.max(0.3);
    }
}

/// Добыча: поглощённый блок; опыт 1–2 у большого (считается в `Mob::die`). Малый не даёт ничего.
pub fn loot(mob: &Mob) -> Vec<Stack> {
    if mob.nether.size != LARGE { return Vec::new(); }
    mob.sulfur.body.map(|item| vec![Stack::new(item, 1)]).unwrap_or_default()
}

/// Опыт: 1–2 у большого, 0 у малого.
pub fn experience(mob: &Mob, rng: &mut Rng) -> i32 {
    if mob.nether.size == LARGE { 1 + rng.below(2) as i32 } else { 0 }
}

/// Мысль: пассивный; блуждает и прыгает как слизень, но без цели; с блоком стоит, пока не запущен.
pub fn think(mob: &mut Mob, view: &dyn View, _ctx: &Ctx, rng: &mut Rng) -> Plan {
    let mut plan = Plan::default();
    if mob.sulfur.pickup_timer > 0 { mob.sulfur.pickup_timer -= 1; }
    if mob.sulfur.fuse >= 0 {
        mob.sulfur.fuse -= 1;
        if mob.sulfur.fuse <= 0 {
            plan.blast = Some(4.0); // сила динамита; [проверить] у куба на сложностях
            return plan;
        }
    }
    let (x, y, z) = mob.cell();
    let fluid = super::is_water(view.block(x, y, z)) || super::is_lava(view.block(x, y, z));
    if mob.on_ground || fluid {
        mob.nether.wander_wait -= 1;
        if mob.nether.wander_wait <= 0 {
            mob.nether.wander_wait = 40 + rng.below(60) as i32;
            mob.face((rng.unit() * 360.0) as f32);
        }
    }
    if mob.sulfur.body.is_some() { return plan; }
    if fluid {
        mob.vy += 0.04;
        plan.speed = 1.2;
    } else if mob.on_ground {
        mob.sulfur.hop -= 1;
        if mob.sulfur.hop <= 0 {
            mob.sulfur.hop = 10 + rng.below(20) as i32; // [проверить] задержка как у слизня
            plan.jump = true;
            plan.speed = 1.0;
            let name = if mob.nether.size == SMALL { "minecraft:entity.sulfur_cube_small.jump" } else { "minecraft:entity.sulfur_cube.jump" };
            mob.events.push(Event::SoundAt { name, volume: 1.0, pitch: 1.0, at: (mob.x, mob.y, mob.z) });
        }
    } else {
        plan.speed = 1.0;
    }
    plan
}

/// Приземление: звук (общая часть прыжка кубов).
pub fn landed(mob: &mut Mob) {
    let name = if mob.nether.size == SMALL { "minecraft:entity.sulfur_cube_small.land" } else { "minecraft:entity.sulfur_cube.land" };
    mob.events.push(Event::Sound { name, volume: 1.0, pitch: 1.0 });
}

/// Поглощение брошенных блоков: большой куб без блока подбирает один подходящий предмет.
pub fn absorb_items(items: &mut crate::items::Items, griefing: bool) {
    if !griefing { return; }
    let idle: Vec<(usize, i32, (f64, f64, f64))> = items.mobs().iter().enumerate()
        .filter(|(_, m)| m.kind == Kind::SulfurCube && !m.is_dead() && !m.no_ai && m.nether.size == LARGE
            && m.sulfur.body.is_none() && m.sulfur.pickup_timer <= 0)
        .map(|(i, m)| (i, m.entity_id, (m.x, m.y, m.z))).collect();
    for (index, id, at) in idle {
        let mut taken = None;
        let got = items.take_near(at, id, |stack| {
            if taken.is_none() && crate::blocks::item_name(stack.item).is_some_and(absorbable) { taken = Some(stack.item); 1 } else { 0 }
        });
        if let (Some(item), false) = (taken, got.is_empty()) {
            let name = crate::blocks::item_name(item).unwrap_or("");
            absorb(&mut items.mobs_mut()[index], item, name);
        }
    }
}

/// Делится ли куб при гибели: большой — на двух малых (кроме собственного взрыва).
pub fn split(parent: &Mob, rng: &mut Rng, next_id: &mut dyn FnMut() -> i32) -> Vec<Mob> {
    if parent.nether.size != LARGE || parent.h.despawn || !parent.gone() { return Vec::new(); }
    (0..2).map(|i| {
        let at = (parent.x + (f64::from(i) - 0.5) * 0.5, parent.y + 0.5, parent.z);
        let mut child = Mob::new(next_id(), Kind::SulfurCube, at);
        set_size(&mut child, SMALL);
        child.no_ai = parent.no_ai;
        child.silent = parent.silent;
        child.face((rng.unit() * 360.0) as f32);
        child
    }).collect()
}

/// Данные из NBT команд: `Size` (0/1), `fuse`, `pickup_timer`, `from_bucket`, `equipment.body.id`.
pub fn apply_data(mob: &mut Mob, data: &str) {
    if mob.kind != Kind::SulfurCube { return; }
    let Some(v) = crate::snbt::parse(data) else { return; };
    if let Some(size) = v.get("Size").and_then(|v| v.as_f64()) { set_size(mob, size as i32 + 1); }
    if let Some(x) = v.get("fuse").and_then(|v| v.as_f64()) { mob.sulfur.fuse = x as i32; }
    if let Some(x) = v.get("pickup_timer").and_then(|v| v.as_f64()) { mob.sulfur.pickup_timer = x as i32; }
    if let Some(x) = v.get("from_bucket").and_then(|v| v.as_f64()) { mob.sulfur.from_bucket = x != 0.0; }
    if let Some(id) = v.get("equipment").and_then(|e| e.get("body")).and_then(|b| b.get("id")).and_then(|i| i.as_str()) {
        mob.sulfur.body = crate::blocks::item_named(id.trim_start_matches("minecraft:"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mob::slime;

    #[test]
    fn sizes_health_speed_and_box_match_observation() {
        let mut cube = Mob::new(1, Kind::SulfurCube, (0.5, 64.0, 0.5));
        initialize(&mut cube);
        assert_eq!((cube.nether.size, cube.vitals.max_health, cube.age), (LARGE, 8.0, 0));
        assert_eq!(slime::speed(cube.nether.size) as f32, 0.4);
        assert!((cube.size().0 - 0.98).abs() < 1e-9);
        set_size(&mut cube, SMALL);
        assert_eq!((cube.vitals.max_health, cube.age), (4.0, super::super::BABY_AGE));
        assert_eq!(slime::speed(SMALL) as f32, 0.3);
        assert!((cube.size().0 - 0.49).abs() < 1e-9);
    }

    #[test]
    fn large_cube_splits_into_two_small_but_not_after_own_blast() {
        let mut parent = Mob::new(1, Kind::SulfurCube, (0.5, 64.0, 0.5));
        initialize(&mut parent);
        parent.vitals.health = 0.0; parent.vitals.dead = true; parent.dead_for = 100;
        let mut n = 10;
        let kids = split(&parent, &mut Rng::new(1), &mut || { n += 1; n });
        assert_eq!(kids.len(), 2);
        assert!(kids.iter().all(|k| k.nether.size == SMALL && k.vitals.health == 4.0 && k.is_baby()));
        parent.h.despawn = true;
        assert!(split(&parent, &mut Rng::new(1), &mut || 5).is_empty());
        assert!(kids.iter().all(|k| split(k, &mut Rng::new(1), &mut || 5).is_empty()));
    }

    #[test]
    fn absorption_rules_shear_pause_and_drops() {
        assert!(absorbable("stone") && absorbable("white_wool") && absorbable("tnt"));
        assert!(!absorbable("sand") && !absorbable("stone_slab") && !absorbable("oak_stairs") && !absorbable("stick"));
        assert_eq!(archetype("magma_block"), Some(Archetype::Hot));
        assert_eq!(archetype("packed_ice"), Some(Archetype::FastSliding));
        let stone = crate::blocks::item_named("stone").unwrap();
        let mut cube = Mob::new(1, Kind::SulfurCube, (0.5, 64.0, 0.5));
        set_size(&mut cube, SMALL);
        assert!(!absorb(&mut cube, stone, "stone"), "малый не поглощает");
        initialize(&mut cube);
        assert!(absorb(&mut cube, stone, "stone"));
        assert!(!absorb(&mut cube, stone, "stone"), "второй блок не влезает");
        assert_eq!(loot(&cube), vec![Stack::new(stone, 1)]);
        assert_eq!(shear(&mut cube), Some(stone));
        assert_eq!(cube.sulfur.pickup_timer, PICKUP_PAUSE);
        assert!(!absorb(&mut cube, stone, "stone"), "пауза подбора");
    }

    #[test]
    fn tnt_fuse_blast_and_resistance() {
        let tnt = crate::blocks::item_named("tnt").unwrap();
        let mut cube = Mob::new(1, Kind::SulfurCube, (0.5, 64.0, 0.5));
        initialize(&mut cube);
        assert!(!ignite(&mut cube));
        absorb(&mut cube, tnt, "tnt");
        assert!(ignite(&mut cube) && cube.sulfur.fuse == TNT_FUSE);
        assert_eq!(shear(&mut cube), None, "с подожжённым динамитом блок не снять");
        assert!(resists(&cube, crate::survival::Source::PlayerAttack));
        assert!(!resists(&cube, crate::survival::Source::OnFire));
    }

    #[test]
    fn data_roundtrip_and_spawn_rule() {
        let mut cube = Mob::new(1, Kind::SulfurCube, (0.5, 64.0, 0.5));
        apply_data(&mut cube, "{Size:1,equipment:{body:{id:\"minecraft:tnt\",count:1}},from_bucket:1b}");
        assert_eq!((cube.nether.size, cube.sulfur.from_bucket), (LARGE, true));
        assert_eq!(cube.sulfur.body.and_then(crate::blocks::item_name), Some("tnt"));
        assert!(can_spawn_in("sulfur_caves") && !can_spawn_in("plains"));
        let mut small = Mob::new(2, Kind::SulfurCube, (0.5, 64.0, 0.5));
        apply_data(&mut small, "{Size:0}");
        small.age = -1;
        let before = small.age;
        small.age += 1;
        assert!(before < 0 && small.age == 0);
        grow(&mut small);
        assert_eq!(small.nether.size, LARGE);
    }

    #[test]
    fn save_roundtrip_keeps_size_block_fuse_and_flags() {
        let tnt = crate::blocks::item_named("tnt").unwrap();
        let mut cube = Mob::new(7, Kind::SulfurCube, (1.5, 64.0, -2.5));
        initialize(&mut cube);
        absorb(&mut cube, tnt, "tnt");
        cube.sulfur.fuse = 55;
        cube.sulfur.from_bucket = true;
        let mut bytes = Vec::new();
        cube.write(&mut bytes);
        let back = Mob::read(&mut bytes.as_slice()).expect("запись читается");
        assert_eq!((back.kind, back.nether.size, back.sulfur.body, back.sulfur.fuse, back.sulfur.from_bucket), (Kind::SulfurCube, LARGE, Some(tnt), 55, true));
        let mut small = Mob::new(8, Kind::SulfurCube, (0.5, 64.0, 0.5));
        set_size(&mut small, SMALL);
        let mut bytes = Vec::new();
        small.write(&mut bytes);
        let back = Mob::read(&mut bytes.as_slice()).unwrap();
        assert_eq!((back.nether.size, back.age, back.vitals.max_health), (SMALL, -GROW_TICKS, 4.0));
    }
}
