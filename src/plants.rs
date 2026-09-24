// На чём растут растения и что с ними, когда опору убрали.
//
// Правила — по страницам блоков на вики (раздел «Placement»/«Usage»):
// трава, цветы, саженцы и кусты растут на земле; тростник — на земле или
// песке у воды; кактус — на песке и без соседей по бокам; сухие кусты —
// на песке, терракоте и земле; кувшинка — на воде; грибы — на полном блоке;
// у высоких растений нижняя половина держит верхнюю и наоборот.
//
// Проверка общая для установки игроком и для соседских обновлений: убрали
// блок под цветком — цветок ломается и выпадает, как у оригинала.

use crate::blocks;
use crate::fluids;
use crate::world::{World, AIR};

/// Что нужно растению, чтобы стоять на месте.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Needs {
    /// Земля: дёрн, земля, подзол, мох, грязь и прочее «почвенное».
    Soil,
    /// Земля, песок, терракота — сухие кусты и сухая трава.
    DrySoil,
    /// Песок (или кактус под ним), и по бокам ничего твёрдого.
    Cactus,
    /// Тростник под ним или почва/песок с водой рядом.
    SugarCane,
    /// Вода под ним — кувшинка.
    Water,
    /// Грядка — посевы.
    Farmland,
    /// Песок душ — адский нарост.
    SoulSand,
    /// Полный блок снизу — грибы, снежный покров.
    Solid,
    /// Незерская почва: нилий, песок душ, земля — грибы и корни Незера.
    NetherSoil,
    /// Под водой на твёрдом — морская трава, ламинария.
    Underwater,
}

/// Правило для блока с этим именем; None — блок держится сам.
fn needs(name: &str) -> Option<Needs> {
    Some(match name {
        "short_grass" | "fern" | "tall_grass" | "large_fern" | "dandelion" | "poppy" | "blue_orchid" | "allium"
        | "azure_bluet" | "red_tulip" | "orange_tulip" | "white_tulip" | "pink_tulip" | "oxeye_daisy" | "cornflower"
        | "lily_of_the_valley" | "wither_rose" | "torchflower" | "golden_dandelion" | "sunflower" | "lilac"
        | "rose_bush" | "peony" | "pitcher_plant" | "sweet_berry_bush" | "bush" | "firefly_bush" | "pink_petals"
        | "wildflowers" | "leaf_litter" | "open_eyeblossom" | "closed_eyeblossom" | "azalea" | "flowering_azalea"
        | "bamboo_sapling" | "pale_moss_carpet" => Needs::Soil,
        name if name.ends_with("_sapling") => Needs::Soil,
        "dead_bush" | "short_dry_grass" | "tall_dry_grass" => Needs::DrySoil,
        "cactus" => Needs::Cactus,
        "sugar_cane" => Needs::SugarCane,
        "lily_pad" => Needs::Water,
        "wheat" | "carrots" | "potatoes" | "beetroots" | "melon_stem" | "pumpkin_stem" | "torchflower_crop"
        | "pitcher_crop" => Needs::Farmland,
        "nether_wart" => Needs::SoulSand,
        "brown_mushroom" | "red_mushroom" | "snow" => Needs::Solid,
        "crimson_fungus" | "warped_fungus" | "crimson_roots" | "warped_roots" | "nether_sprouts" => Needs::NetherSoil,
        "seagrass" | "tall_seagrass" | "kelp" | "kelp_plant" => Needs::Underwater,
        _ => return None,
    })
}

/// Почва: на ней растут трава, цветы, саженцы.
fn soil(name: &str) -> bool {
    matches!(
        name,
        "grass_block"
            | "dirt"
            | "coarse_dirt"
            | "podzol"
            | "rooted_dirt"
            | "farmland"
            | "mycelium"
            | "moss_block"
            | "pale_moss_block"
            | "mud"
            | "muddy_mangrove_roots"
    )
}

fn sand(name: &str) -> bool {
    matches!(name, "sand" | "red_sand" | "suspicious_sand")
}

/// Может ли растение этого состояния стоять в этом месте.
///
/// Блоки, которым опора не нужна, всегда «могут».
pub fn survives(world: &World, state: i32, x: i32, y: i32, z: i32) -> bool {
    let Some(name) = blocks::block_at_state(state) else {
        return true;
    };
    if needs(name).is_none() {
        return true;
    }

    // Верхняя половина высокого растения держится на нижней того же вида.
    if half(name, state) == Some("upper") {
        return blocks::block_at_state(world.get_block(x, y - 1, z)) == Some(name);
    }

    // Нижняя половина без верхней тоже не живёт: сломали верх — ломается и низ.
    if half(name, state) == Some("lower") && blocks::block_at_state(world.get_block(x, y + 1, z)) != Some(name) {
        return false;
    }

    ground_ok(world, name, world.get_block(x, y - 1, z), x, y, z)
}

/// Держит ли то, что снизу (`below_state`), растение с этим именем.
fn ground_ok(world: &World, name: &str, below_state: i32, x: i32, y: i32, z: i32) -> bool {
    let Some(needs) = needs(name) else {
        return true;
    };
    let below = blocks::block_at_state(below_state).unwrap_or("air");

    match needs {
        Needs::Soil => soil(below),
        Needs::DrySoil => soil(below) || sand(below) || below == "terracotta" || below.ends_with("_terracotta"),
        Needs::Cactus => {
            (sand(below) || below == "cactus")
                && [(1, 0), (-1, 0), (0, 1), (0, -1)]
                    .iter()
                    .all(|(dx, dz)| !blocks::has_box(world.get_block(x + dx, y, z + dz)) || is_fluid(world.get_block(x + dx, y, z + dz)))
        }
        Needs::SugarCane => {
            below == "sugar_cane"
                || ((soil(below) || sand(below))
                    && [(1, 0), (-1, 0), (0, 1), (0, -1)]
                        .iter()
                        .any(|(dx, dz)| wet(world.get_block(x + dx, y - 1, z + dz))))
        }
        Needs::Water => matches!(fluids::fluid_at(below_state), Some((fluids::Kind::Water, _))) || below == "ice",
        Needs::Farmland => below == "farmland",
        Needs::SoulSand => below == "soul_sand",
        Needs::Solid => {
            if name == "snow" {
                blocks::solid_top(below_state) && !matches!(below, "ice" | "packed_ice" | "barrier")
            } else {
                blocks::solid_top(below_state) || matches!(below, "mycelium" | "podzol")
            }
        }
        Needs::NetherSoil => {
            soil(below) || matches!(below, "crimson_nylium" | "warped_nylium" | "soul_soil" | "mycelium")
        }
        Needs::Underwater => {
            below == "kelp" || below == "kelp_plant" || (blocks::solid_top(below_state) && below != "magma_block")
        }
    }
}

/// Значение свойства «половина» у высоких растений.
fn half(name: &str, state: i32) -> Option<&'static str> {
    blocks::orientation(name).and_then(|rule| rule.value(state, "half"))
}

fn is_fluid(state: i32) -> bool {
    fluids::fluid_at(state).is_some()
}

/// Вода или то, что стоит в воде (обводнённый блок, лёд для тростника не
/// считается — у оригинала тростнику нужна вода, а не лёд, но замёрзшая
/// вода «frosted_ice» считается).
fn wet(state: i32) -> bool {
    if matches!(fluids::fluid_at(state), Some((fluids::Kind::Water, _))) {
        return true;
    }

    let Some(name) = blocks::block_at_state(state) else {
        return false;
    };

    name == "frosted_ice"
        || blocks::orientation(name).and_then(|rule| rule.value(state, "waterlogged")) == Some("true")
}

/// Можно ли поставить растение сюда: как `survives`, но у нижней половины
/// высокого растения верхней ещё нет — её ставят следом, поэтому смотрится
/// только опора снизу.
pub fn can_place(world: &World, state: i32, x: i32, y: i32, z: i32) -> bool {
    match blocks::block_at_state(state) {
        Some(name) if half(name, state) == Some("lower") => ground_ok(world, name, world.get_block(x, y - 1, z), x, y, z),
        _ => survives(world, state, x, y, z),
    }
}

/// Проверяет растение в этом месте и, если опоры нет, ломает его — с
/// выпадением, как у оригинала. Возвращает, сломано ли.
pub fn check(world: &mut World, x: i32, y: i32, z: i32) -> bool {
    let state = world.get_block(x, y, z);

    if state == AIR || survives(world, state, x, y, z) {
        return false;
    }

    world.note_destroyed(x, y, z);
    world.set_block(x, y, z, AIR);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::redstone::settle;

    fn state(text: &str) -> i32 {
        blocks::state_from_text(text).unwrap_or_else(|| panic!("нет состояния {}", text))
    }

    /// Мир с полом из земли на высоте 0 и камнем в стороне.
    fn world() -> World {
        let mut world = World::in_memory();

        for x in -4..=4 {
            for z in -4..=4 {
                world.set_block_silently(x, 0, z, state("dirt"));
            }
        }

        world.set_block_silently(3, 0, 3, state("stone"));
        world
    }

    fn put(world: &mut World, (x, y, z): (i32, i32, i32), block: i32) {
        world.set_block(x, y, z, block);
        settle(world);
    }

    /// Цветок и трава ставятся на землю, но не на камень и не в воздух.
    #[test]
    fn flowers_need_soil() {
        let world = world();

        assert!(can_place(&world, state("poppy"), 0, 1, 0));
        assert!(!can_place(&world, state("poppy"), 3, 1, 3));
        assert!(!can_place(&world, state("short_grass"), 0, 3, 0));
    }

    /// Трава, поставленная командой на камень, стоит, пока не изменится
    /// сосед, — как у оригинала (чёрный ящик, grass-crush).
    #[test]
    fn a_misplaced_plant_waits_for_a_neighbour() {
        let mut world = world();
        let grass = state("short_grass");

        put(&mut world, (3, 1, 3), grass);
        assert_eq!(world.get_block(3, 1, 3), grass);

        put(&mut world, (4, 1, 3), state("stone"));
        assert_eq!(world.get_block(3, 1, 3), AIR);
    }

    /// Убрали землю из-под цветка — цветок сломался.
    #[test]
    fn a_flower_breaks_without_ground() {
        let mut world = world();

        put(&mut world, (0, 1, 0), state("dandelion"));
        assert_eq!(blocks::block_at_state(world.get_block(0, 1, 0)), Some("dandelion"));

        put(&mut world, (0, 0, 0), AIR);
        assert_eq!(world.get_block(0, 1, 0), AIR);
    }

    /// Высокая трава ломается целиком: и снизу, и сверху.
    #[test]
    fn tall_plants_break_as_a_whole() {
        let mut world = world();

        put(&mut world, (1, 1, 1), state("tall_grass[half=lower]"));
        put(&mut world, (1, 2, 1), state("tall_grass[half=upper]"));
        put(&mut world, (1, 2, 1), AIR);
        assert_eq!(world.get_block(1, 1, 1), AIR, "нижняя половина осталась без верхней");

        put(&mut world, (2, 1, 2), state("tall_grass[half=lower]"));
        put(&mut world, (2, 2, 2), state("tall_grass[half=upper]"));
        put(&mut world, (2, 0, 2), AIR);
        assert_eq!(world.get_block(2, 1, 2), AIR);
        assert_eq!(world.get_block(2, 2, 2), AIR, "верхняя половина висит в воздухе");
    }

    /// Тростник растёт только у воды, а сломанный снизу — падает весь.
    #[test]
    fn sugar_cane_needs_water_and_falls_as_a_stack() {
        let mut world = world();

        assert!(!can_place(&world, state("sugar_cane"), 0, 1, 0));

        world.set_block_silently(-1, 0, 0, state("water"));
        assert!(can_place(&world, state("sugar_cane"), 0, 1, 0));

        put(&mut world, (0, 1, 0), state("sugar_cane"));
        put(&mut world, (0, 2, 0), state("sugar_cane"));
        put(&mut world, (0, 3, 0), state("sugar_cane"));
        put(&mut world, (0, 1, 0), AIR);

        assert_eq!(world.get_block(0, 2, 0), AIR);
        assert_eq!(world.get_block(0, 3, 0), AIR);
    }
}
