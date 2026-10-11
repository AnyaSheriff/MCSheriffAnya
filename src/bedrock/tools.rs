// Предметы в руке при щелчке по блоку у игрока Bedrock: костная мука,
// мотыга, вёдра, яйца призыва. Правила — те же, что у Java
// (network/play/farm.rs, use_bucket и use_spawn_egg в network/play.rs):
// вызывается общий код (farming.rs, fluids.rs), здесь — только то, что
// Java берёт из своего состояния игрока. Что делать с ответом (слоты,
// выпавшее, моб) — решает вызывающий уже без замка мира.

use crate::blocks;
use crate::combat;
use crate::commands::{GAME_MODE_ADVENTURE, GAME_MODE_CREATIVE};
use crate::fluids;
use crate::inventory::Stack;
use crate::world::World;

use super::inventory::Bag;

/// Точка и скорость выпавшего предмета.
pub type Drop = (Stack, (f64, f64, f64), (f64, f64, f64));

/// Что вышло из щелчка предметом.
#[derive(Default)]
pub struct ToolUse {
    /// Слоты инвентаря, которые изменились.
    pub slots: Vec<i32>,
    /// Что выпало в мир: стопка, точка, скорость.
    pub drops: Vec<Drop>,
    /// Что не влезло в инвентарь — выбросить под ноги.
    pub thrown: Vec<Stack>,
    /// Призванный с яйца моб.
    pub mob: Option<crate::mob::Mob>,
    /// Предмет к блоку глух — клиент мог показать своё, вернуть ему правду.
    pub refused: bool,
}

/// Игрок, от которого зависит щелчок.
pub struct Who {
    pub game_mode: i32,
    pub yaw: f32,
}

/// Предмет меняет поверхность до открытия окна медного сундука.
/// Вики, Copper: «waxing», Campfire: «up to four food items».
pub fn surface(bag: &mut Bag, who: &Who, world: &mut World, pos: (i32,i32,i32), face: i32) -> Option<ToolUse> {
    let stack = bag.inventory.held()?;
    let name = blocks::item_name(stack.item)?;
    let state = world.get_block(pos.0,pos.1,pos.2);
    let creative = who.game_mode == GAME_MODE_CREATIVE;
    let mut used = ToolUse::default();
    let mut consume = false;
    let mut wear = false;
    if let Some((next, action)) = crate::copper::interaction(state, name) {
        crate::copper::replace(world, pos, next);
        let (event, sound) = match action {
            crate::copper::Use::Wax => (3003, "minecraft:item.honeycomb.wax_on"),
            crate::copper::Use::Unwax => (3004, "minecraft:item.axe.wax_off"),
            crate::copper::Use::Scrape => (3005, "minecraft:item.axe.scrape"),
        };
        world.note_level_event(pos.0,pos.1,pos.2,event,0);
        world.note_sound(pos.0,pos.1,pos.2,sound,1.0,1.0);
        consume = action == crate::copper::Use::Wax; wear = !consume;
    } else if crate::campfire::is_campfire(state) {
        let lighting = matches!(name, "flint_and_steel" | "fire_charge");
        if lighting || name.ends_with("_shovel") {
            if !crate::campfire::set_lit(world, pos, lighting) { return None; }
            consume = name == "fire_charge"; wear = !consume;
            if lighting { world.note_sound(pos.0,pos.1,pos.2,if consume { "minecraft:item.firecharge.use" } else { "minecraft:item.flintandsteel.use" },1.0,1.0); }
        } else if name == "water_bucket" {
            if world.is_nether() {
                world.note_sound(pos.0,pos.1,pos.2,"minecraft:block.fire.extinguish",0.5,2.6);
            } else {
                crate::campfire::set_lit(world,pos,false);
                let state = world.get_block(pos.0,pos.1,pos.2);
                let wet = blocks::orientation_of(state)?.with(state,"waterlogged","true")?;
                world.set_block(pos.0,pos.1,pos.2,wet);
            }
            swap_held(bag,creative,blocks::item_named("bucket")?,&mut used);
        } else if crate::campfire::insert(world,pos,stack) { consume = true; } else { return None; }
    } else { return None; }
    if consume { used.slots.extend(spend(bag,creative)); }
    if wear && !creative {
        let worn = combat::wear(&stack,1,false,||world.random_unit());
        let slot = bag.inventory.selected_slot(); bag.inventory.set(slot,combat::add_damage(stack,worn)); used.slots.push(slot);
    }
    let beyond = super::world::offset(pos,face);
    world.tell_directly(pos.0,pos.1,pos.2); world.tell_directly(beyond.0,beyond.1,beyond.2);
    crate::redstone::settle(world); Some(used)
}

/// Щелчок предметом `item` по блоку `clicked`, грань `face`. None — это не
/// наш предмет: щелчок идёт дальше по-обычному (ставится блок).
pub fn use_on_block(
    bag: &mut Bag,
    who: &Who,
    world: &mut World,
    item: i32,
    clicked: (i32, i32, i32),
    face: i32,
    new_mob_id: impl FnOnce() -> i32,
) -> Option<ToolUse> {
    let name = blocks::item_name(item)?;
    let creative = who.game_mode == GAME_MODE_CREATIVE;
    let beyond = super::world::offset(clicked, face);

    // Мотыга и костная мука — не в приключении (farm.rs).
    if (name == "bone_meal" || name.ends_with("_hoe")) && who.game_mode != GAME_MODE_ADVENTURE {
        // Не вышло — оригинал всё равно повторяет блок и клетку за гранью.
        let refuse = |world: &mut World| {
            world.tell_directly(clicked.0, clicked.1, clicked.2);
            world.tell_directly(beyond.0, beyond.1, beyond.2);
            Some(ToolUse { refused: true, ..ToolUse::default() })
        };

        if name == "bone_meal" {
            let Some(drops) = crate::farming::bone_meal(world, clicked) else {
                return refuse(world);
            };
            let drops = drops.into_iter().map(|stack| {
                let (at, velocity) = crate::tick::scattered(clicked, || f64::from(world.random_unit()));
                (stack, at, velocity)
            }).collect();

            world.tell_directly(clicked.0, clicked.1, clicked.2);
            world.tell_directly(beyond.0, beyond.1, beyond.2);
            crate::redstone::settle(world);

            return Some(ToolUse { slots: spend(bag, creative), drops, ..ToolUse::default() });
        }

        let tilled = crate::farming::till(world, clicked, face);
        let Some(dropped) = tilled else {
            return refuse(world);
        };

        world.tell_directly(clicked.0, clicked.1, clicked.2);
        world.tell_directly(beyond.0, beyond.1, beyond.2);
        crate::redstone::settle(world);

        // Мотыга изнашивается на единицу (вики, Hoe: «uses 1 durability»).
        let mut result = ToolUse::default();

        if !creative && let Some(held) = bag.inventory.held() {
            let worn = combat::wear(&held, 1, false, || world.random_unit());
            let slot = bag.inventory.selected_slot();

            bag.inventory.set(slot, combat::add_damage(held, worn));
            result.slots.push(slot);
        }

        // Висячие корни выскакивают из верхней грани (как в farm.rs).
        if let Some(stack) = dropped {
            let (x, y, z) = clicked;
            let mut random = || f64::from(world.random_unit());
            let at = (f64::from(x) + 0.25 + random() * 0.5, f64::from(y) + 1.0, f64::from(z) + 0.25 + random() * 0.5);
            let velocity = (random() * 0.2 - 0.1, 0.2, random() * 0.2 - 0.1);

            result.drops.push((stack, at, velocity));
        }

        return Some(result);
    }

    if let Some(result) = bucket(bag, who, world, item, clicked, face) {
        return Some(result);
    }

    spawn_egg(bag, who, name, clicked, face, new_mob_id)
}

/// Расходует предмет из руки: в творческом ничего не уходит.
fn spend(bag: &mut Bag, creative: bool) -> Vec<i32> {
    if creative {
        return Vec::new();
    }

    bag.inventory.spend_held().into_iter().collect()
}

/// Ведро: зачерпнуть источник (или рыхлый снег) в блоке, по которому
/// щёлкнули, или вылить содержимое туда, как в `use_bucket` у Java. None —
/// в руке не ведро.
fn bucket(bag: &mut Bag, who: &Who, world: &mut World, item: i32, clicked: (i32, i32, i32), face: i32) -> Option<ToolUse> {
    let empty = blocks::item_named("bucket")?;
    let with_water = blocks::item_named("water_bucket")?;
    let with_lava = blocks::item_named("lava_bucket")?;
    let with_snow = blocks::item_named("powder_snow_bucket")?;
    let (x, y, z) = clicked;
    let creative = who.game_mode == GAME_MODE_CREATIVE;

    if who.game_mode == GAME_MODE_ADVENTURE {
        return None;
    }

    if item == empty {
        let state = world.get_block(x, y, z);
        let filled = match fluids::fluid_at(state) {
            Some((fluids::Kind::Water, fluids::SOURCE)) => with_water,
            Some((fluids::Kind::Lava, fluids::SOURCE)) => with_lava,
            _ if fluids::is_powder_snow(state) => with_snow,
            _ => return None,
        };

        world.set_block(x, y, z, crate::world::AIR);
        crate::redstone::settle(world);

        let mut result = ToolUse::default();

        swap_held(bag, creative, filled, &mut result);
        return Some(result);
    }

    let poured = if item == with_water {
        fluids::state_of(fluids::Kind::Water, fluids::SOURCE)
    } else if item == with_lava {
        fluids::state_of(fluids::Kind::Lava, fluids::SOURCE)
    } else if item == with_snow {
        blocks::state_by_name("powder_snow")?
    } else {
        return None;
    };

    // Встаёт в тот блок, по которому щёлкнули, если он пустой, иначе — в
    // соседний с той стороны, куда щёлкнули.
    let (x, y, z) = if world.get_block(x, y, z) == crate::world::AIR { (x, y, z) } else { super::world::offset(clicked, face) };

    // В Незере вода из ведра испаряется с шипением (вики, Water).
    if world.is_nether() && item == with_water {
        let pitch = 2.6 + (world.random_unit() - world.random_unit()) * 0.8;

        world.note_sound(x, y, z, "minecraft:block.fire.extinguish", 0.5, pitch);
    } else {
        world.set_block(x, y, z, poured);
        crate::redstone::settle(world);
    }

    let mut result = ToolUse::default();

    swap_held(bag, creative, empty, &mut result);
    Some(result)
}

/// Меняет ведро в руке на другое; лишнее — в инвентарь или под ноги.
fn swap_held(bag: &mut Bag, creative: bool, item: i32, result: &mut ToolUse) {
    if creative {
        return;
    }

    let slot = bag.inventory.selected_slot();

    match bag.inventory.held() {
        Some(held) if held.count > 1 => {
            bag.inventory.spend_held();
            result.slots.push(slot);

            let (changed, left) = bag.inventory.put(Stack::new(item, 1));

            result.slots.extend(changed);
            result.thrown.extend(left);
        }
        _ => {
            bag.inventory.set(slot, Some(Stack::new(item, 1)));
            result.slots.push(slot);
        }
    }
}

/// Яйцо призыва: моб появляется в клетке за гранью щелчка (вики, Spawn Egg),
/// в выживании яйцо тратится. Как `use_spawn_egg` у Java.
fn spawn_egg(
    bag: &mut Bag,
    who: &Who,
    name: &str,
    clicked: (i32, i32, i32),
    face: i32,
    new_mob_id: impl FnOnce() -> i32,
) -> Option<ToolUse> {
    let kind = crate::mob::Kind::from_name(name.strip_suffix("_spawn_egg")?)?;

    if who.game_mode == GAME_MODE_ADVENTURE {
        return None;
    }

    let (x, y, z) = super::world::offset(clicked, face);
    let mut mob = crate::mob::Mob::new(new_mob_id(), kind, (f64::from(x) + 0.5, f64::from(y), f64::from(z) + 0.5));

    mob.yaw = who.yaw;
    mob.head_yaw = who.yaw;

    if kind == crate::mob::Kind::Chicken {
        mob.egg_timer = 6000 + (mob.entity_id as u32 % 6001) as i32;
    }

    Some(ToolUse { slots: spend(bag, who.game_mode == GAME_MODE_CREATIVE), mob: Some(mob), ..ToolUse::default() })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::GAME_MODE_SURVIVAL;
    use crate::inventory::Inventory;

    fn bag_with(item: &str, count: i32) -> Bag {
        let mut inventory = Inventory::new();

        inventory.put(Stack::new(blocks::item_named(item).expect("предмет"), count));
        Bag::new(inventory)
    }

    fn world() -> World {
        let mut world = World::in_memory();

        for x in -3..=3 {
            for z in -3..=3 {
                world.set_block_silently(x, 0, z, blocks::state_by_name("farmland").expect("грядка"));
            }
        }

        world
    }

    fn survival() -> Who {
        Who { game_mode: GAME_MODE_SURVIVAL, yaw: 0.0 }
    }

    #[test]
    fn surface_actions_change_shared_state_and_charge_the_hand() {
        let mut world = World::in_memory(); let pos = (0,70,0);
        let mut bag = Bag::new(crate::inventory::Inventory::new());
        let who = Who { game_mode:crate::commands::GAME_MODE_SURVIVAL,yaw:0.0 };
        world.set_block(0,70,0,blocks::state_by_name("copper_block").unwrap());
        bag.inventory.set(crate::inventory::FIRST_HOTBAR,Some(Stack::new(blocks::item_named("honeycomb").unwrap(),2)));
        assert!(surface(&mut bag,&who,&mut world,pos,1).is_some());
        assert_eq!(blocks::block_at_state(world.get_block(0,70,0)),Some("waxed_copper_block"));
        assert_eq!(bag.inventory.held().unwrap().count,1);
        bag.inventory.set(crate::inventory::FIRST_HOTBAR,Some(Stack::new(blocks::item_named("iron_axe").unwrap(),1)));
        assert!(surface(&mut bag,&who,&mut world,pos,1).is_some());
        assert_eq!(blocks::block_at_state(world.get_block(0,70,0)),Some("copper_block"));
        assert_eq!(bag.inventory.held().unwrap().extras().damage,1);
        world.set_block(0,70,0,blocks::state_by_name("campfire").unwrap());
        bag.inventory.set(crate::inventory::FIRST_HOTBAR,Some(Stack::new(blocks::item_named("cod").unwrap(),2)));
        assert!(surface(&mut bag,&who,&mut world,pos,1).is_some());
        assert_eq!(world.container(0,70,0).unwrap().slots[0].unwrap().count,1);
        assert_eq!(bag.inventory.held().unwrap().count,1);
    }

    #[test]
    fn bone_meal_grows_wheat_and_is_spent() {
        let mut world = world();
        let mut bag = bag_with("bone_meal", 2);
        let wheat = blocks::state_by_name("wheat").expect("пшеница");

        world.set_block_silently(0, 1, 0, wheat);

        let item = blocks::item_named("bone_meal").expect("мука");
        let used = use_on_block(&mut bag, &survival(), &mut world, item, (0, 1, 0), 1, || 1).expect("мука");

        assert!(!used.refused);
        assert_eq!(used.slots, vec![bag.inventory.selected_slot()]);
        assert_eq!(bag.inventory.held().map(|stack| stack.count), Some(1));
        assert_ne!(world.get_block(0, 1, 0), wheat, "пшеница выросла");
    }

    #[test]
    fn bone_meal_on_stone_is_kept() {
        let mut world = world();
        let mut bag = bag_with("bone_meal", 1);
        let item = blocks::item_named("bone_meal").expect("мука");
        let used = use_on_block(&mut bag, &survival(), &mut world, item, (0, 0, 0), 1, || 1).expect("мука");

        assert!(used.refused);
        assert!(used.slots.is_empty());
        assert_eq!(bag.inventory.held().map(|stack| stack.count), Some(1));
    }

    #[test]
    fn hoe_tills_dirt_and_wears() {
        let mut world = world();

        world.set_block_silently(1, 0, 0, blocks::state_by_name("dirt").expect("земля"));

        let mut bag = bag_with("stone_hoe", 1);
        let item = blocks::item_named("stone_hoe").expect("мотыга");
        let used = use_on_block(&mut bag, &survival(), &mut world, item, (1, 0, 0), 1, || 1).expect("мотыга");

        assert!(!used.refused);
        assert_eq!(used.slots.len(), 1, "мотыга износилась");
        assert_eq!(world.get_block(1, 0, 0), blocks::state_by_name("farmland").expect("грядка"));
    }

    #[test]
    fn water_bucket_pours_next_to_the_clicked_block() {
        let mut world = world();
        let mut bag = bag_with("water_bucket", 1);
        let item = blocks::item_named("water_bucket").expect("ведро");
        let used = use_on_block(&mut bag, &survival(), &mut world, item, (0, 0, 0), 1, || 1).expect("ведро");

        assert!(!used.refused);
        assert!(fluids::fluid_at(world.get_block(0, 1, 0)).is_some(), "вода встала над блоком");
        assert_eq!(bag.inventory.held().map(|stack| stack.item), blocks::item_named("bucket"));
    }

    #[test]
    fn spawn_egg_makes_a_mob_in_the_clicked_cell() {
        let mut world = world();
        let mut bag = bag_with("cow_spawn_egg", 1);
        let item = blocks::item_named("cow_spawn_egg").expect("яйцо");
        let used = use_on_block(&mut bag, &Who { yaw: 90.0, ..survival() }, &mut world, item, (0, 0, 0), 1, || 77).expect("яйцо");
        let mob = used.mob.expect("моб");

        assert_eq!((mob.entity_id, mob.yaw), (77, 90.0));
        assert!(bag.inventory.held().is_none(), "яйцо потрачено");
    }

    #[test]
    fn other_items_are_left_alone() {
        let mut world = world();
        let mut bag = bag_with("stone", 1);
        let item = blocks::item_named("stone").expect("камень");

        assert!(use_on_block(&mut bag, &survival(), &mut world, item, (0, 0, 0), 1, || 1).is_none());
    }
}
