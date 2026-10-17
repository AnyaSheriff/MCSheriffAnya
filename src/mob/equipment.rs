// Общие шесть ячеек живой сущности. Номера Java: руки 0/1,
// обувь/штаны/нагрудник/шлем 2..5. Старые trial/stand данные читаются без миграции.
use super::{Kind, Mob};
use crate::{blocks, inventory::Stack};

pub type Gear = [Option<Stack>; 6];
pub type Chances = [f32; 6];
const NAMES: [&str; 6] = ["mainhand", "offhand", "feet", "legs", "chest", "head"];

fn item(name: &str) -> Option<Stack> { blocks::item_named(name).map(|id| Stack::new(id, 1)) }

pub fn slot(mob: &Mob, slot: u8) -> Option<Stack> {
    if slot >= 6 { return None; }
    if mob.kind == Kind::ArmorStand { return mob.stand.equipment[slot as usize]; }
    if slot == 0 {
        // Эти ячейки меняются обычными pickup/give/eat путями. Общая броня
        // не должна замораживать старую копию предмета в руке.
        match mob.kind {
            Kind::Fox => return mob.wild.mouth,
            Kind::Allay => return mob.small.held,
            Kind::Panda => return mob.land.held,
            Kind::CopperGolem => return mob.newmobs.cargo,
            _ => {}
        }
    }
    if let Some(gear) = &mob.equipment {
        return gear[slot as usize].map(|stack| {
            if slot == 0 && mob.kind == Kind::Pillager && blocks::item_name(stack.item) == Some("crossbow") {
                let mut data = stack.extras();
                data.damage = mob.illager.wear;
                data.charged = u8::from(mob.illager.charged);
                stack.with_extras(data)
            } else { stack }
        });
    }
    if let Some(gear) = &mob.h.trial_gear {
        return match slot { 0 => gear[0], 4 => gear[1], 5 => gear[2], _ => None };
    }
    match slot {
        0 => {
            if mob.kind == Kind::Fox { return mob.wild.mouth; }
            if mob.kind == Kind::Allay { return mob.small.held; }
            if mob.kind == Kind::Panda { return mob.land.held; }
            if mob.kind == Kind::CopperGolem { return mob.newmobs.cargo; }
            if let Some(name) = super::hostile::hand_item(mob.h.hand) { return item(name); }
            if mob.kind.skeleton() && mob.h.bow { return item("bow"); }
            if let Some(name) = super::illager::hand_item(mob.kind) {
                return item(name).map(|stack| if mob.kind == Kind::Pillager {
                    stack.with_extras(crate::stackdata::Data { damage: mob.illager.wear, charged: u8::from(mob.illager.charged), ..Default::default() })
                } else { stack });
            }
            None
        }
        1 if mob.h.offhand_shell => item("nautilus_shell"),
        5 if mob.h.helmet => item("iron_helmet"),
        5 if mob.illager.captain => item("white_banner").map(|stack| stack.with_extras(crate::stackdata::Data { ominous_banner: true, ..Default::default() })),
        _ => None,
    }
}

pub fn all(mob: &Mob) -> Gear { std::array::from_fn(|index| slot(mob, index as u8)) }

pub fn chances(mob: &Mob) -> Chances {
    if mob.equipment.is_some() { return mob.equipment_drop_chances; }
    let mut chances = [super::DEFAULT_DROP; 6];
    if let Some(gear) = mob.h.trial_gear {
        for (stack, slot) in gear.into_iter().zip([0, 4, 5]) { if stack.is_some() { chances[slot] = 0.0; } }
        return chances;
    }
    chances[0] = mob.h.hand_drop;
    if matches!(mob.kind, Kind::Fox | Kind::Allay | Kind::CopperGolem) { chances[0] = 2.0; }
    if mob.h.offhand_shell { chances[1] = 2.0; }
    if mob.illager.captain { chances[5] = 2.0; }
    chances
}

pub fn set_slot(mob: &mut Mob, slot_id: u8, stack: Option<Stack>) -> bool {
    if slot_id >= 6 || mob.kind.object() { return false; }
    if mob.kind == Kind::ArmorStand {
        mob.stand.equipment[slot_id as usize] = stack;
        mob.stand.armor_due = Some(mob.ticks + 1);
    } else {
        mob.equipment_drop_chances = chances(mob);
        let mut gear = all(mob);
        gear[slot_id as usize] = stack;
        mob.equipment = Some(gear);
        if slot_id == 0 { match mob.kind {
            Kind::Fox => mob.wild.mouth = stack,
            Kind::Allay => mob.small.held = stack,
            Kind::Panda => mob.land.held = stack,
            Kind::CopperGolem => mob.newmobs.cargo = stack,
            Kind::Pillager => {
                mob.illager.wear = stack.map_or(0, |s| s.extras().damage);
                mob.illager.charged = stack.is_some_and(|s| s.extras().charged > 0);
            }
            _ => {}
        } }
        if slot_id == 5 { mob.h.helmet = stack.is_some(); }
        if slot_id == 0 && mob.kind.skeleton() { mob.h.bow = stack.is_some_and(|s| blocks::item_name(s.item) == Some("bow")); }
        if slot_id == 0 && mob.kind == Kind::Drowned {
            mob.h.hand = match stack.and_then(|s| blocks::item_name(s.item)) {
                Some("trident") => super::hostile::HAND_TRIDENT,
                Some("nautilus_shell") => super::hostile::HAND_SHELL,
                Some("fishing_rod") => super::hostile::HAND_ROD,
                _ => super::hostile::HAND_NONE,
            };
        }
        if slot_id == 1 { mob.h.offhand_shell = stack.is_some_and(|s| blocks::item_name(s.item) == Some("nautilus_shell")); }
    }
    mob.attr_changes = mob.attr_changes.wrapping_add(1);
    true
}

pub fn apply(mob: &mut Mob, text: &str) {
    if mob.kind == Kind::ArmorStand { return; }
    let Some(value) = crate::snbt::parse(text) else { return; };
    if let Some(gear) = value.get("equipment") {
        for (index, name) in NAMES.into_iter().enumerate() {
            if let Some(value) = gear.get(name) {
                let stack = crate::commands::item_from_text(&format!("{{Item:{}}}", crate::commands::component_text(value)));
                set_slot(mob, index as u8, stack);
            }
        }
    }
    if let Some(values) = value.get("drop_chances") {
        let mut chances = chances(mob);
        for (index, name) in NAMES.into_iter().enumerate() {
            if let Some(chance) = values.get(name).and_then(|v| v.as_f64()).map(|v| v as f32).filter(|v| v.is_finite()) {
                chances[index] = chance;
            }
        }
        mob.equipment = Some(all(mob));
        mob.equipment_drop_chances = chances;
    }
}

/// Вики, Mob/Drop chances: «default (0.085f)»; Looting добавляет 1% за
/// уровень. Шанс >1 — подобранная вещь: без требования убийства игроком.
/// Vanishing действует до броска и сохраняет остальные компоненты добычи.
pub fn death_drops(mob: &Mob, rng: &mut super::Rng) -> Vec<Stack> {
    if mob.equipment.is_none() { return Vec::new(); }
    let gear = all(mob);
    let credited = mob.by_player || mob.xp_until > mob.ticks;
    gear.into_iter().zip(mob.equipment_drop_chances).filter_map(|(stack, chance)| {
        let stack = stack?;
        if stack.extras().level("vanishing_curse") > 0 || chance <= 0.0 { return None; }
        let guaranteed = chance > 1.0;
        if !guaranteed && (!credited || rng.unit() >= f64::from(chance) + 0.01 * f64::from(mob.looting.max(0))) { return None; }
        let maximum = crate::combat::max_damage(&stack);
        if !guaranteed && maximum > 0 {
            let mut data = stack.extras();
            data.damage = drop_damage(maximum, rng);
            Some(stack.with_extras(data))
        } else { Some(stack) }
    }).collect()
}

/// Статистическая модель vanilla enchant-mob-gear-durability (384 вещи).
/// Два вложенных равномерных броска согласуются с замером: max8 оставляет
/// 0..4 uses с частотами 54/32/27/6/9 из128; прежний damage заменяется.
/// [проверить] Точная формула/края больших max не доказаны конечной выборкой.
/// Гарантированные chance>1 сюда не попадают и сохраняют исходный damage.
fn drop_damage(maximum: i32, rng: &mut super::Rng) -> i32 {
    let upper = 1 + rng.below(maximum.saturating_sub(3).max(1) as u32);
    maximum - rng.below(upper) as i32
}

pub fn bonus(mob: &Mob, attribute: i32) -> f32 {
    if mob.equipment.is_none() && mob.kind != Kind::ArmorStand && let Some(gear) = &mob.h.trial_gear {
        return super::trial_gear::bonus(gear, attribute);
    }
    all(mob).into_iter().zip([1, 2, 4, 5, 6, 7]).filter_map(|(stack, place)| stack.map(|stack| (stack, place)))
        .flat_map(|(stack, place)| crate::combat::granted(&stack, place)).filter(|(attr, _)| *attr == attribute)
        .map(|(_, modifier)| modifier.amount as f32).sum()
}

pub fn write(gear: &Gear, out: &mut Vec<u8>) {
    for stack in gear {
        out.push(u8::from(stack.is_some()));
        if let Some(stack) = stack {
            out.extend(stack.stored_item().to_be_bytes());
            out.extend(stack.count.to_be_bytes());
            stack.write_extras(out);
        }
    }
}

pub fn read(bytes: &mut &[u8]) -> Option<Gear> {
    let mut take = |n| { let (head, rest) = bytes.split_at_checked(n)?; *bytes = rest; Some(head) };
    let mut gear = [None; 6];
    for slot in &mut gear {
        if take(1)?[0] != 0 {
            let raw = i32::from_be_bytes(take(4)?.try_into().ok()?);
            let count = i32::from_be_bytes(take(4)?.try_into().ok()?);
            if count <= 0 { return None; }
            let mut stack = Stack::from_stored(raw, count);
            if let Some(data) = Stack::read_extras(raw, &mut take)? { stack = stack.with_extras(data); }
            *slot = Some(stack);
        }
    }
    Some(gear)
}

pub fn read_chances(bytes: &mut &[u8]) -> Option<Chances> {
    let mut values = [super::DEFAULT_DROP; 6];
    for value in &mut values {
        let (head, rest) = bytes.split_at_checked(4)?;
        *bytes = rest;
        *value = f32::from_be_bytes(head.try_into().ok()?);
        if !value.is_finite() { return None; }
    }
    Some(values)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_trial_tail_stays_readable_and_new_slots_round_trip() {
        let legacy = [item("bow"), item("iron_chestplate"), item("iron_helmet")];
        // Старый хвост: flag8, пустое имя, нет поводка и ровно три вещи.
        let mut bytes = vec![8, 0, 0, 0];
        super::super::trial_gear::write(&legacy, &mut bytes);
        let mut input = bytes.as_slice();
        let tail = super::super::leash::read_tail(&mut input).unwrap();
        assert!(input.is_empty());
        let mut mob = Mob::new(5, Kind::Skeleton, (0.0, 64.0, 0.0));
        super::super::leash::apply_tail(&mut mob, tail);
        assert!(mob.equipment.is_none());
        assert_eq!(slot(&mob, 0), legacy[0]);
        assert_eq!(slot(&mob, 2), None);
        assert_eq!(slot(&mob, 4), legacy[1]);
        assert_eq!(slot(&mob, 5), legacy[2]);
        let mut back = Vec::new(); super::super::leash::write_tail(&mob, &mut back);
        assert_eq!(back, bytes);

        let mut data = crate::stackdata::Data::default();
        data.set_enchant(crate::stackdata::enchantment_number("frost_walker").unwrap(), 2);
        let boots = item("diamond_boots").unwrap().with_extras(data);
        assert!(set_slot(&mut mob, 2, Some(boots)));
        assert!(set_slot(&mut mob, 1, item("shield")));
        assert!(set_slot(&mut mob, 3, item("diamond_leggings")));
        mob.equipment_drop_chances = [0.0, 2.0, 1.0, 0.25, 0.0, 0.0];
        let expected = all(&mob);
        assert_eq!(expected[0], legacy[0]);
        let mut saved = Vec::new(); mob.write(&mut saved);
        let mut input = saved.as_slice();
        let loaded = Mob::read(&mut input).unwrap();
        assert!(input.is_empty());
        assert_eq!(loaded.equipment, Some(expected));
        assert_eq!(loaded.h.trial_gear, Some(legacy));
        assert_eq!(loaded.equipment_drop_chances, mob.equipment_drop_chances);
        assert_eq!(slot(&loaded, 2).unwrap().extras().level("frost_walker"), 2);
    }

    #[test]
    fn generic_equipment_accepts_components_and_explicit_empty_slots() {
        let mut mob = Mob::new(7, Kind::Zombie, (0.0, 64.0, 0.0));
        apply(&mut mob, r#"{equipment:{feet:{id:"minecraft:diamond_boots",count:1,components:{"minecraft:enchantments":{"minecraft:frost_walker":2}}}}}"#);
        assert_eq!(slot(&mob, 2).unwrap().extras().level("frost_walker"), 2);
        assert_eq!(mob.armor(), 5.0); // zombie 2 + diamond boots 3.
        assert!(set_slot(&mut mob, 2, None));
        assert_eq!(slot(&mob, 2), None);
        assert_eq!(mob.armor(), 2.0);
        assert!(!set_slot(&mut mob, 6, item("stone")));
        let mut stand = Mob::new(8, Kind::ArmorStand, (0.0, 64.0, 0.0));
        assert!(set_slot(&mut stand, 2, item("iron_boots")));
        assert_eq!(slot(&stand, 2), stand.stand.equipment[2]);
        assert!(stand.equipment.is_none());
    }

    #[test]
    fn death_drop_chances_credit_vanishing_and_components() {
        let mut mob = Mob::new(7, Kind::Zombie, (0.0, 64.0, 0.0));
        apply(&mut mob, r#"{equipment:{feet:{id:"minecraft:diamond_boots",components:{"minecraft:damage":17,"minecraft:enchantments":{"minecraft:frost_walker":2}}},chest:{id:"minecraft:diamond_chestplate",components:{"minecraft:enchantments":{"minecraft:vanishing_curse":1}}},head:{id:"minecraft:iron_helmet"},offhand:{id:"minecraft:shield"}},drop_chances:{feet:2.0f,chest:2.0f,head:0.0f,offhand:1.0f}}"#);
        let boots = slot(&mob, 2).unwrap();
        assert_eq!(death_drops(&mob, &mut super::super::Rng::new(10)), vec![boots]);
        mob.by_player = true;
        let drops = death_drops(&mob, &mut super::super::Rng::new(10));
        assert_eq!(drops.len(), 2);
        assert!(drops.contains(&boots));
        assert_eq!(boots.extras().damage, 17);
        assert_eq!(boots.extras().level("frost_walker"), 2);
        assert!(drops.iter().any(|s| blocks::item_name(s.item) == Some("shield")));
        set_slot(&mut mob, 2, item("golden_boots"));
        assert_eq!(mob.equipment_drop_chances[2], 2.0); // Износ/замена не сбрасывает заданный шанс.
    }

    #[test]
    fn generic_drops_replace_legacy_species_gear_without_duplication() {
        for kind in [Kind::Skeleton, Kind::Drowned, Kind::Pillager, Kind::Fox, Kind::Allay, Kind::CopperGolem] {
            let mut mob = Mob::new(7, kind, (0.0, 64.0, 0.0));
            mob.by_player = true;
            mob.looting = 100; // Старый случайный drop тоже сработал бы.
            mob.h.hand_drop = 2.0;
            mob.wild.mouth = item("diamond");
            mob.small.held = item("diamond");
            mob.newmobs.cargo = item("diamond");
            apply(&mut mob, r#"{equipment:{mainhand:{id:"minecraft:diamond"},offhand:{id:"minecraft:nautilus_shell"}},drop_chances:{mainhand:2.0f,offhand:2.0f}}"#);
            let drops = super::super::loot(&mob, &mut super::super::Rng::new(11));
            assert_eq!(drops.iter().filter(|s| blocks::item_name(s.item) == Some("diamond")).map(|s| s.count).sum::<i32>(), 1, "{kind:?}");
            assert_eq!(drops.iter().filter(|s| blocks::item_name(s.item) == Some("nautilus_shell")).map(|s| s.count).sum::<i32>(), 1, "{kind:?}");
            assert!(!drops.iter().any(|s| matches!(blocks::item_name(s.item), Some("bow" | "crossbow"))));
        }
    }

    #[test]
    fn trial_gear_stays_nondropping_after_boots_assignment() {
        let mut mob = Mob::new(7, Kind::Skeleton, (0.0, 64.0, 0.0));
        mob.h.trial_gear = Some([item("bow"), item("iron_chestplate"), item("iron_helmet")]);
        set_slot(&mut mob, 2, item("diamond_boots"));
        mob.by_player = true;
        mob.looting = 255;
        assert_eq!(mob.equipment_drop_chances, [0.0, super::super::DEFAULT_DROP, super::super::DEFAULT_DROP, super::super::DEFAULT_DROP, 0.0, 0.0]);
        let drops = death_drops(&mob, &mut super::super::Rng::new(42));
        assert_eq!(drops.len(), 1);
        assert_eq!(blocks::item_name(drops[0].item), Some("diamond_boots"));
        assert!(drops[0].extras().damage > 0);
        apply(&mut mob, "{drop_chances:{feet:2.0f}}");
        assert_eq!(death_drops(&mob, &mut super::super::Rng::new(42)), vec![item("diamond_boots").unwrap()]);
    }

    #[test]
    fn sampled_death_wear_is_biased_and_replaces_old_damage_without_losing_components() {
        // Малое max различает равномерную модель и измеренный сильный
        // перевес почти сломанного предмета; старый flat 85..100% не годится.
        let mut rng = super::super::Rng::new(7643);
        let mut remaining = [0u32; 5];
        for _ in 0..20_000 { remaining[(8 - drop_damage(8, &mut rng)) as usize] += 1; }
        assert!((0.42..0.49).contains(&(f64::from(remaining[0]) / 20_000.0)));
        assert!((0.02..0.06).contains(&(f64::from(remaining[4]) / 20_000.0)));

        let mut old = Mob::new(7, Kind::Zombie, (0.0, 64.0, 0.0));
        apply(&mut old, r#"{equipment:{feet:{id:"minecraft:iron_boots",components:{"minecraft:damage":194,"minecraft:custom_name":"sample","minecraft:enchantments":{"minecraft:frost_walker":2}}}},drop_chances:{feet:1.0f}}"#);
        old.by_player = true;
        let mut fresh = old.clone();
        let stack = slot(&fresh, 2).unwrap();
        let mut data = stack.extras(); data.damage = 0;
        set_slot(&mut fresh, 2, Some(stack.with_extras(data)));
        let mut saw_repair = false;
        for seed in 1..128 {
            let used = death_drops(&old, &mut super::super::Rng::new(seed));
            let new = death_drops(&fresh, &mut super::super::Rng::new(seed));
            assert_eq!(used, new);
            let extras = used[0].extras();
            assert!((4..=195).contains(&extras.damage));
            assert_eq!(extras.name.as_deref(), Some("sample"));
            assert_eq!(extras.level("frost_walker"), 2);
            saw_repair |= extras.damage < 194;
        }
        assert!(saw_repair);
        old.equipment_drop_chances[2] = 2.0;
        assert_eq!(death_drops(&old, &mut rng)[0].extras().damage, 194);
    }

    #[test]
    fn fox_pickup_and_crossbow_charge_do_not_leave_stale_generic_gear() {
        let mut fox = Mob::new(7, Kind::Fox, (0.5, 0.0, 0.5));
        set_slot(&mut fox, 0, item("diamond"));
        set_slot(&mut fox, 5, item("iron_helmet"));
        let mut items = crate::items::Items::new();
        items.add_mob(fox);
        items.drop_item(80, item("sweet_berries").unwrap().with_count(4), (0.5, 0.0, 0.5), (0.0, 0.0, 0.0), 0);
        items.note_tick_start();
        super::super::wildlife::pickup(&mut items, true);
        let fox = &items.mobs()[0];
        assert_eq!(slot(fox, 0), item("sweet_berries"));
        let mut saved = Vec::new(); fox.write(&mut saved);
        let loaded = Mob::read(&mut saved.as_slice()).unwrap();
        assert_eq!(slot(&loaded, 0), item("sweet_berries"));
        assert_eq!(loaded.equipment.unwrap()[0], item("sweet_berries"));

        let mut pillager = Mob::new(8, Kind::Pillager, (0.0, 64.0, 0.0));
        let mut data = crate::stackdata::Data::default();
        data.set_enchant(crate::stackdata::enchantment_number("quick_charge").unwrap(), 3);
        set_slot(&mut pillager, 0, Some(item("crossbow").unwrap().with_extras(data)));
        pillager.illager.charged = true;
        pillager.illager.wear = 3;
        let actual = slot(&pillager, 0).unwrap().extras();
        assert_eq!((actual.charged, actual.damage, actual.level("quick_charge")), (1, 3, 3));
    }

    #[test]
    fn vanilla_stand_vanishing_and_command_kill_drop_policies() {
        for cursed in [false, true] {
            let mut mob = Mob::new(7, Kind::ArmorStand, (0.0, 64.0, 0.0));
            let mut data = crate::stackdata::Data::default();
            if cursed { data.set_enchant(crate::stackdata::enchantment_number("vanishing_curse").unwrap(), 1); }
            let boots = item("netherite_boots").unwrap().with_extras(data);
            set_slot(&mut mob, 2, Some(boots));
            let first = super::super::armor_stand::hurt(&mut mob, 1000.0, crate::survival::Source::PlayerAttack).unwrap();
            assert!(!first.died);
            mob.ticks += 2;
            assert!(super::super::armor_stand::hurt(&mut mob, 1000.0, crate::survival::Source::PlayerAttack).unwrap().died);
            let loot: Vec<_> = mob.events.iter().filter_map(|event| match event { super::super::Event::Loot(stacks) => Some(stacks), _ => None }).flatten().copied().collect();
            assert_eq!(loot, if cursed { vec![item("armor_stand").unwrap()] } else { vec![item("armor_stand").unwrap(), boots] });
        }
        let mut mob = Mob::new(8, Kind::ArmorStand, (0.0, 64.0, 0.0));
        set_slot(&mut mob, 2, item("netherite_boots"));
        assert!(super::super::armor_stand::hurt(&mut mob, 1000.0, crate::survival::Source::GenericKill).unwrap().died);
        assert!(mob.events.is_empty());
        assert!(mob.h.despawn);
    }
}
