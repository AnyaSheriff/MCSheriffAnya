// Вики, Bone Meal (oldid=3516009): дополнительные цели Java 26.1.2.
// Геометрия случайного распространения и порядок пакетов [проверить].
use super::*;

type Pos = (i32, i32, i32);

fn put(world: &mut World, at: Pos, state: i32) {
    world.set_block(at.0, at.1, at.2, state);
}

fn get(world: &World, at: Pos) -> i32 { world.get_block(at.0, at.1, at.2) }

fn empty(world: &World, at: Pos) -> bool { get(world, at) == AIR }

fn water(state: i32) -> bool {
    matches!(fluids::fluid_at(state), Some((fluids::Kind::Water, fluids::SOURCE)))
}

fn item(name: &str) -> Vec<Stack> {
    blocks::item_named(name).map(|id| vec![Stack::new(id, 1)]).unwrap_or_default()
}

fn dirt(state: i32) -> bool {
    matches!(name_of(state), "dirt" | "grass_block" | "coarse_dirt" | "rooted_dirt" | "podzol" | "mycelium" | "moss_block" | "pale_moss_block" | "mud" | "farmland")
}

fn spread(world: &mut World, at: Pos, plant: &str, dry: bool) -> bool {
    let mut candidates = Vec::new();
    for dx in -1..=1 {
        for dz in -1..=1 {
            if dx == 0 && dz == 0 { continue; }
            let p = (at.0 + dx, at.1, at.2 + dz);
            let ground = get(world, (p.0, p.1 - 1, p.2));
            if empty(world, p) && (dirt(ground) || (dry && matches!(name_of(ground), "sand" | "red_sand" | "terracotta"))) {
                candidates.push(p);
            }
        }
    }
    if candidates.is_empty() { return false; }
    let p = candidates[roll(world, candidates.len() as i32) as usize];
    put(world, p, state_of(plant));
    true
}

/// None — не поддерживается или нет места; успешное применение возвращает
/// предметы отдельно, чтобы общий мир не захватывал замок Items.
pub(super) fn apply(world: &mut World, at: Pos) -> Option<Vec<Stack>> {
    let (x, y, z) = at;
    let state = get(world, at);
    let name = name_of(state);
    let above = (x, y + 1, z);
    let below = (x, y - 1, z);
    let mut drops = Vec::new();
    match name {
        "brown_mushroom" | "red_mushroom" => {
            if !dirt(get(world, below)) { return None; }
            if chance(world, 0.4) {
                let shape = crate::world::vegetation::grown_mushroom(u64::from(world.random_bits()), name == "red_mushroom");
                if shape.iter().all(|&(dx,dy,dz,_)| {
                    let p=(x+dx,y+dy-1,z+dz);
                    (-64..320).contains(&p.1) && (p==at || empty(world,p) || name_of(get(world,p)).ends_with("_leaves"))
                }) {
                    for (dx,dy,dz,block) in shape { put(world,(x+dx,y+dy-1,z+dz),block); }
                }
            }
        }
        "crimson_nylium" | "warped_nylium" => {
            if !empty(world, above) { return None; }
            // Вики, Bone Meal: «in a 5 × 5 total area». Точные веса [проверить].
            let warped = name == "warped_nylium";
            let root = if warped { "warped_roots" } else { "crimson_roots" };
            put(world, above, state_of(root));
            for dx in -2..=2 { for dz in -2..=2 {
                let p=(x+dx,y+1,z+dz);
                let floor=get(world,(p.0,p.1-1,p.2));
                if !empty(world,p) || !(dirt(floor) || matches!(name_of(floor),"crimson_nylium"|"warped_nylium"|"netherrack")) || roll(world,2)!=0 {continue;}
                let target=match roll(world,8) {
                    0 => if warped { "warped_fungus" } else { "crimson_fungus" },
                    1 => if warped { "crimson_fungus" } else { "warped_fungus" },
                    2 if warped => "nether_sprouts",
                    3 if warped => "twisting_vines",
                    _ => root,
                };
                put(world,p,state_of(target));
            } }
        }
        "short_grass" | "fern" | "seagrass" => {
            let aquatic = name == "seagrass";
            if !(if aquatic { water(get(world, above)) } else { empty(world, above) }) { return None; }
            let target = state_of(match name { "fern" => "large_fern", "seagrass" => "tall_seagrass", _ => "tall_grass" });
            put(world, at, with(target, "half", "lower")?);
            put(world, above, with(target, "half", "upper")?);
        }
        "sunflower" | "lilac" | "rose_bush" | "peony" => drops = item(name),
        "pink_petals" | "wildflowers" => {
            let count = number(state, "flower_amount");
            if count < 4 { put(world, at, with(state, "flower_amount", count + 1)?); }
            else { drops = item(name); }
        }
        "firefly_bush" | "bush" => { if !spread(world, at, name, false) { return None; } }
        "short_dry_grass" => put(world, at, state_of("tall_dry_grass")),
        "tall_dry_grass" => { if !spread(world, at, "short_dry_grass", true) { return None; } }
        "rooted_dirt" => {
            if !empty(world, below) { return None; }
            put(world, below, state_of("hanging_roots"));
        }
        "pale_hanging_moss" => {
            let mut tip = at;
            while name_of(get(world, (x, tip.1 - 1, z))) == name && tip.1 > -64 { tip.1 -= 1; }
            let next = (x, tip.1 - 1, z);
            if !empty(world, next) || next.1 < -64 { return None; }
            put(world, tip, with(get(world, tip), "tip", false)?);
            put(world, next, with(state_of(name), "tip", true)?);
        }
        "cave_vines" | "cave_vines_plant" => {
            if value(state, "berries") == Some("true") { return None; }
            put(world, at, with(state, "berries", true)?);
        }
        "weeping_vines" | "weeping_vines_plant" | "twisting_vines" | "twisting_vines_plant" | "bamboo" => {
            let down = name.starts_with("weeping");
            let step = if down { -1 } else { 1 };
            let head = if down { "weeping_vines" } else if name == "bamboo" { "bamboo" } else { "twisting_vines" };
            let stem = if down { "weeping_vines_plant" } else if name == "bamboo" { "bamboo" } else { "twisting_vines_plant" };
            let mut top = y;
            while (-63..319).contains(&top) && matches!(name_of(world.get_block(x, top + step, z)), n if n == head || n == stem) { top += step; }
            if head == "bamboo" {
                let mut bottom = y;
                while bottom > -64 && name_of(world.get_block(x,bottom-1,z)) == "bamboo" { bottom -= 1; }
                if top - bottom + 1 >= 16 || number(world.get_block(x,top,z),"stage") == 1 { return None; }
            }
            if !empty(world, (x, top + step, z)) { return None; }
            let count = if head == "bamboo" {
                let mut bottom = y;
                while bottom > -64 && name_of(world.get_block(x,bottom-1,z)) == "bamboo" { bottom -= 1; }
                (1 + roll(world, 2)).min(16 - (top - bottom + 1))
            } else { 1 + roll(world, 3) };
            for _ in 0..count {
                let next = (x, top + step, z);
                if !(-64..320).contains(&next.1) || !empty(world, next) { break; }
                put(world, (x, top, z), state_of(stem));
                let target = if head == "bamboo" { with(state_of(head), "leaves", "large").unwrap_or(state_of(head)) } else { state_of(head) };
                put(world, next, target);
                top += step;
            }
        }
        "mangrove_leaves" => {
            if !empty(world, below) { return None; }
            let target = with(state_of("mangrove_propagule"), "hanging", true)?;
            put(world, below, with(target, "age", 0)?);
        }
        "mangrove_propagule" if value(state, "hanging") == Some("true") => {
            if number(state, "age") >= 4 { return None; }
            put(world, at, with(state, "age", number(state, "age") + 1)?);
        }
        "netherrack" => {
            if crate::world::light::filter(get(world, above)) == 15 { return None; }
            let mut crimson = false;
            let mut warped = false;
            for dx in -1..=1 { for dy in -1..=1 { for dz in -1..=1 {
                match name_of(world.get_block(x + dx, y + dy, z + dz)) {
                    "crimson_nylium" => crimson = true,
                    "warped_nylium" => warped = true,
                    _ => {}
                }
            } } }
            let target = match (crimson, warped) {
                (false, false) => return None,
                (true, false) => "crimson_nylium",
                (false, true) => "warped_nylium",
                (true, true) => if roll(world, 2) == 0 { "crimson_nylium" } else { "warped_nylium" },
            };
            put(world, at, state_of(target));
        }
        "big_dripleaf" | "big_dripleaf_stem" => {
            let mut top = y;
            while name_of(world.get_block(x, top + 1, z)) == "big_dripleaf_stem" && top < 319 { top += 1; }
            if name_of(world.get_block(x, top + 1, z)) == "big_dripleaf" { top += 1; }
            let tip = world.get_block(x, top, z);
            let next = world.get_block(x, top + 1, z);
            if top >= 319 || (next != AIR && !water(next)) { return None; }
            let facing = value(tip, "facing").unwrap_or("north");
            let stem = with(state_of("big_dripleaf_stem"), "facing", facing)?;
            let stem = with(stem, "waterlogged", value(tip, "waterlogged") == Some("true"))?;
            put(world, (x, top, z), stem);
            let leaf = with(state_of("big_dripleaf"), "facing", facing)?;
            put(world, (x, top + 1, z), with(leaf, "waterlogged", water(next))?);
        }
        "small_dripleaf" => {
            let bottom = if value(state, "half") == Some("upper") { below } else { at };
            let facing = value(state, "facing").unwrap_or("north");
            let stem = with(state_of("big_dripleaf_stem"), "facing", facing)?;
            let leaf = with(state_of("big_dripleaf"), "facing", facing)?;
            let upper = (bottom.0, bottom.1 + 1, bottom.2);
            let wet = |s| value(s, "waterlogged") == Some("true");
            put(world, bottom, with(stem, "waterlogged", wet(get(world, bottom)))?);
            put(world, upper, with(leaf, "waterlogged", wet(get(world, upper)))?);
        }
        "glow_lichen" => {
            // Перенос по той же плоскости; огибание углов [проверить].
            let faces = [("down", (0,-1,0)), ("up", (0,1,0)), ("north", (0,0,-1)), ("south", (0,0,1)), ("west", (-1,0,0)), ("east", (1,0,0))];
            let mut candidates = Vec::new();
            for (face, normal) in faces {
                if value(state, face) != Some("true") { continue; }
                for (_, step) in faces {
                    if step.0 * normal.0 + step.1 * normal.1 + step.2 * normal.2 != 0 { continue; }
                    let next = (x + step.0, y + step.1, z + step.2);
                    let target = get(world, next);
                    let support = get(world, (next.0 + normal.0, next.1 + normal.1, next.2 + normal.2));
                    if blocks::full_cube(support) && (target == AIR || water(target) || (name_of(target) == name && value(target, face) != Some("true"))) {
                        candidates.push((next, face, target));
                    }
                }
            }
            if candidates.is_empty() { return None; }
            let (next, face, target) = candidates[roll(world, candidates.len() as i32) as usize];
            let base = if name_of(target) == name { target } else { with(state_of(name), "waterlogged", water(target))? };
            put(world, next, with(base, face, true)?);
        }
        "sea_pickle" if value(state, "waterlogged") == Some("true") && water(get(world, above)) && name_of(get(world, below)).ends_with("_coral_block") && !name_of(get(world, below)).starts_with("dead_") => {
            put(world, at, with(state, "pickles", 4)?);
            // Размножение на соседних живых кораллах; форма patch [проверить].
            for dx in -2..=2 { for dz in -2..=2 {
                let p = (x + dx, y, z + dz);
                let floor = name_of(get(world, (p.0, p.1 - 1, p.2)));
                if water(get(world, p)) && floor.ends_with("_coral_block") && !floor.starts_with("dead_") && roll(world, 2) == 0 {
                    let count = 1 + roll(world, 4);
                    put(world, p, with(state_of("sea_pickle"), "pickles", count)?);
                }
            } }
        }
        _ => return None,
    }
    world.note_level_event(x, y, z, BONE_MEAL_EVENT, 15);
    Some(drops)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn world(plant: &str) -> World {
        let mut w = World::in_memory();
        for x in -3..=3 { for z in -3..=3 { w.set_block_silently(x, 0, z, state_of("dirt")); } }
        w.set_block_silently(0, 1, 0, blocks::state_from_text(plant).unwrap());
        w
    }
    #[test]
    fn mushrooms_grow_with_room_and_do_not_overwrite_obstacles() {
        for name in ["brown_mushroom","red_mushroom"] {
            let mut w=world(name);
            for _ in 0..100 { if name_of(w.get_block(0,1,0))=="mushroom_stem" {break;} assert!(bone_meal(&mut w,(0,1,0)).is_some()); }
            assert_eq!(name_of(w.get_block(0,1,0)),"mushroom_stem");
            let mut w=world(name);w.set_block_silently(0,2,0,state_of("stone"));
            for _ in 0..100 { assert!(bone_meal(&mut w,(0,1,0)).is_some()); }
            assert_eq!(name_of(w.get_block(0,1,0)),name);assert_eq!(name_of(w.get_block(0,2,0)),"stone");
        }
    }
    #[test]
    fn nylium_grows_matching_vegetation_and_refuses_covered_surface() {
        for (base,root) in [("crimson_nylium","crimson_roots"),("warped_nylium","warped_roots")] {
            let mut w=world(base);
            assert!(bone_meal(&mut w,(0,1,0)).is_some());
            assert_eq!(name_of(w.get_block(0,2,0)),root);
            assert!(bone_meal(&mut w,(0,1,0)).is_none());
        }
    }
    #[test]
    fn double_plants_require_room_and_preserve_water() {
        for (a,b) in [("short_grass","tall_grass"),("fern","large_fern"),("seagrass","tall_seagrass")] {
            let mut w = world(a);
            w.set_block_silently(0,2,0,state_of("stone"));
            assert!(bone_meal(&mut w,(0,1,0)).is_none());
            w.set_block_silently(0,2,0,if a=="seagrass" { state_of("water") } else { AIR });
            assert!(bone_meal(&mut w,(0,1,0)).is_some());
            assert_eq!(name_of(w.get_block(0,1,0)),b);
            assert_eq!(value(w.get_block(0,2,0),"half"),Some("upper"));
        }
    }
    #[test]
    fn double_flowers_duplicate_without_destroying_either_half() {
        for name in ["sunflower","lilac","rose_bush","peony"] {
            let mut w=world(&format!("{name}[half=lower]"));
            w.set_block_silently(0,2,0,blocks::state_from_text(&format!("{name}[half=upper]")).unwrap());
            let s=w.get_block(0,1,0);
            // Цветок выпадает сразу в мир (vegetation::bone_meal, drop_one).
            assert!(bone_meal(&mut w,(0,1,0)).is_some());
            assert_eq!(w.get_block(0,1,0),s);
        }
    }
    #[test]
    fn petals_fill_then_drop_and_bushes_spread() {
        for name in ["pink_petals","wildflowers"] {
            let mut w=world(&format!("{name}[flower_amount=3]"));
            assert!(bone_meal(&mut w,(0,1,0)).unwrap().is_empty());
            assert_eq!(number(w.get_block(0,1,0),"flower_amount"),4);
            assert!(bone_meal(&mut w,(0,1,0)).is_some());
            assert_eq!(number(w.get_block(0,1,0),"flower_amount"),4);
        }
        for name in ["bush","firefly_bush"] {
            let mut w=world(name);
            assert!(bone_meal(&mut w,(0,1,0)).is_some());
            assert_eq!((-1..=1).flat_map(|x|(-1..=1).map(move |z|(x,z))).filter(|&(x,z)|name_of(w.get_block(x,1,z))==name).count(),2);
        }
    }
    #[test]
    fn dry_grass_grows_then_spreads_without_removing_parent() {
        let mut w=world("short_dry_grass");
        assert!(bone_meal(&mut w,(0,1,0)).is_some());
        assert_eq!(name_of(w.get_block(0,1,0)),"tall_dry_grass");
        assert!(bone_meal(&mut w,(0,1,0)).is_some());
        assert_eq!(name_of(w.get_block(0,1,0)),"tall_dry_grass");
    }
    #[test]
    fn roots_and_pale_moss_require_free_space() {
        let mut w=world("rooted_dirt");
        assert!(bone_meal(&mut w,(0,1,0)).is_none());
        w.set_block_silently(0,0,0,AIR);
        assert!(bone_meal(&mut w,(0,1,0)).is_some());
        assert_eq!(name_of(w.get_block(0,0,0)),"hanging_roots");
        let mut w=world("pale_hanging_moss");
        w.set_block_silently(0,0,0,AIR);
        assert!(bone_meal(&mut w,(0,1,0)).is_some());
        assert_eq!(value(w.get_block(0,1,0),"tip"),Some("false"));
        assert_eq!(value(w.get_block(0,0,0),"tip"),Some("true"));
    }
    #[test]
    fn vines_fruit_or_extend_and_bamboo_extends() {
        for name in ["cave_vines","cave_vines_plant"] {
            let mut w=world(name);
            assert!(bone_meal(&mut w,(0,1,0)).is_some());
            assert_eq!(value(w.get_block(0,1,0),"berries"),Some("true"));
            assert!(bone_meal(&mut w,(0,1,0)).is_none());
        }
        for name in ["twisting_vines","bamboo","weeping_vines"] {
            let mut w=world(name);
            let y=if name=="weeping_vines" { w.set_block_silently(0,0,0,AIR); 0 } else { 2 };
            assert!(bone_meal(&mut w,(0,1,0)).is_some());
            assert_ne!(w.get_block(0,y,0),AIR);
        }
    }
    #[test]
    fn mangrove_hanging_propagule_grows_instead_of_becoming_tree() {
        let mut w=world("mangrove_leaves");
        w.set_block_silently(0,0,0,AIR);
        assert!(bone_meal(&mut w,(0,1,0)).is_some());
        assert_eq!(value(w.get_block(0,0,0),"hanging"),Some("true"));
        for n in 1..=4 {
            assert!(bone_meal(&mut w,(0,0,0)).is_some());
            assert_eq!(number(w.get_block(0,0,0),"age"),n);
        }
        assert!(bone_meal(&mut w,(0,0,0)).is_none());
    }
    #[test]
    fn netherrack_requires_neighbor_and_uncovered_top() {
        let mut w=world("netherrack");
        assert!(bone_meal(&mut w,(0,1,0)).is_none());
        w.set_block_silently(1,1,0,state_of("warped_nylium"));
        w.set_block_silently(0,2,0,state_of("stone"));
        assert!(bone_meal(&mut w,(0,1,0)).is_none());
        w.set_block_silently(0,2,0,state_of("glass"));
        assert!(bone_meal(&mut w,(0,1,0)).is_some());
        assert_eq!(name_of(w.get_block(0,1,0)),"warped_nylium");
    }
    #[test]
    fn dripleaf_moves_leaf_up_and_retains_facing() {
        let mut w=world("big_dripleaf[facing=east]");
        assert!(bone_meal(&mut w,(0,1,0)).is_some());
        assert_eq!(name_of(w.get_block(0,1,0)),"big_dripleaf_stem");
        assert_eq!(value(w.get_block(0,2,0),"facing"),Some("east"));
        let mut w=world("small_dripleaf[half=lower,facing=south]");
        w.set_block_silently(0,2,0,blocks::state_from_text("small_dripleaf[half=upper,facing=south]").unwrap());
        assert!(bone_meal(&mut w,(0,2,0)).is_some());
        assert_eq!(name_of(w.get_block(0,2,0)),"big_dripleaf");
    }
    #[test]
    fn lichen_spreads_only_to_supported_face() {
        let mut w=world("glow_lichen[down=true]");
        assert!(bone_meal(&mut w,(0,1,0)).is_some());
        assert_eq!([(-1,0),(1,0),(0,-1),(0,1)].into_iter().filter(|&(x,z)| value(w.get_block(x,1,z),"down")==Some("true")).count(),1);
    }
    #[test]
    fn pickle_colony_needs_live_coral_and_water() {
        let mut w=world("sea_pickle[waterlogged=true,pickles=1]");
        assert!(bone_meal(&mut w,(0,1,0)).is_none());
        w.set_block_silently(0,0,0,state_of("tube_coral_block"));
        assert!(bone_meal(&mut w,(0,1,0)).is_none());
        w.set_block_silently(0,2,0,state_of("water"));
        assert!(bone_meal(&mut w,(0,1,0)).is_some());
        assert_eq!(number(w.get_block(0,1,0),"pickles"),4);
    }
}
