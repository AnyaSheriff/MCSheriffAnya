use super::*;
use crate::gameplay_catalog::{Catalog, Profile};
use crate::inventory::Stack;

fn state(name: &str) -> i32 { blocks::state_by_name(name).unwrap() }

#[test]
fn cached_geometry_is_exact_with_caps_and_reuses_cells_between_boxes() {
    for profile in [Profile::Java26_1_2, Profile::Preview26_3] {
        let mut world = World::in_memory_with_catalog(Catalog::new(profile));
        for y in 62..=66 { for z in -2..=1 { world.set_block_silently(-1,y,z,state("stone")); } }
        world.set_block_silently(1,64,1,state("water"));
        let mut baseline = ExposureCells::with_limit(&world, 0);
        let mut cached = ExposureCells::new(&world);
        let mut saturated = ExposureCells::with_limit(&world, 1);
        // Нулевой отрезок, отрицательные клетки, оси, границы и равные ties.
        for (from,to) in [((0.5,64.0,0.5),(0.5,64.0,0.5)),
            ((-2.5,64.5,0.5),(2.5,64.5,0.5)), ((0.0,64.0,0.0),(2.0,66.0,2.0)),
            ((-1.0,64.0,0.0),(-1.0,65.0,0.0)), ((40.0,64.0,40.0),(41.0,64.0,40.0)),
            ((0.5,-100.0,0.5),(0.5,-99.0,0.5)), ((1.5,64.5,1.5),(1.5,65.5,1.5))] {
            let expected = ray_blocked_with_cells(&mut baseline,from,to);
            assert_eq!(ray_blocked_with_cells(&mut cached,from,to),expected);
            assert_eq!(ray_blocked_with_cells(&mut saturated,from,to),expected);
        }
        for _ in 0..2 {
            for feet in [(-2.5,64.0,0.5),(2.5,64.0,0.5),(0.5,65.0,1.5)] {
                for size in [(0.25,0.25),(SIZE,SIZE),(0.6,1.8),(0.6,1.5)] {
                    let expected = exposure_with_cells(&mut baseline,(0.5,64.0,0.5),feet,size);
                    assert_eq!(exposure_with_cells(&mut cached,(0.5,64.0,0.5),feet,size).to_bits(),expected.to_bits());
                    assert_eq!(exposure_with_cells(&mut saturated,(0.5,64.0,0.5),feet,size).to_bits(),expected.to_bits());
                }
            }
        }
        assert!(cached.misses < baseline.misses, "уменьшены настоящие uncached чтения, не timing");
        assert!(saturated.cells.len() <= 1);
        assert!(baseline.cells.is_empty());
    }
}

#[test]
fn cache_is_lazy_and_bounded_with_hits_and_fallback_after_4096_cells() {
    let world = World::in_memory();
    let mut cells = ExposureCells::new(&world);
    assert_eq!(cells.cells.capacity(),0);
    for x in 0..4100 { assert!(!cells.block_has_box((x,64,0))); }
    assert_eq!(cells.cells.len(),4096); assert_eq!(cells.misses,4100);
    assert!(!cells.block_has_box((0,64,0))); assert_eq!(cells.misses,4100);
    assert!(!cells.block_has_box((4099,64,0))); assert_eq!(cells.misses,4101);
    assert_eq!(cells.cells.len(),4096);
}

#[test]
fn classification_keeps_base_extension_and_fail_closed_policy() {
    for profile in [Profile::Java26_1_2, Profile::Preview26_3] {
        let catalog = Catalog::new(profile);
        for name in ["air","water","torch","short_grass"] {
            assert!(!exposure_block_has_box(catalog,state(name)),"{name}");
        }
        for name in ["stone","sand","oak_slab"] {
            assert!(exposure_block_has_box(catalog,state(name)),"{name}");
        }
        for row in crate::gameplay_catalog::entries() {
            for value in [row.first_state,row.default_state,row.last_state] {
                // Включая plant-like extension: прежняя политика считает
                // ЛЮБОЙ разрешённый entry полным кубом, disabled тоже solid.
                assert!(exposure_block_has_box(catalog,value),"{} {value}",row.name);
            }
        }
        for invalid in [-1,i32::MIN,i32::MAX] { assert!(exposure_block_has_box(catalog,invalid)); }
    }
}

#[test]
fn next_phase_observes_changed_world_in_both_directions() {
    let mut world = World::in_memory();
    let from = (0.5,64.5,0.5); let to = (2.5,64.5,0.5);
    for blocked in [true,false,true,false] {
        world.set_block_silently(1,64,0,if blocked {state("stone")} else {AIR});
        let mut cells = ExposureCells::new(&world);
        assert_eq!(ray_blocked_with_cells(&mut cells,from,to),blocked);
        let misses = cells.misses;
        assert_eq!(ray_blocked_with_cells(&mut cells,from,to),blocked);
        assert_eq!(cells.misses,misses);
    }
}

fn entity_scene(catalog: Catalog) -> Items {
    let mut items = Items::with_catalog(catalog);
    items.prime(1,state("tnt"),(2.5,64.0,0.5),(0.1,0.2,0.0),37,None);
    items.drop_block(2,state("sand"),Stack::new(blocks::item_named("sand").unwrap(),1),(2.5,64.0,0.8));
    items.drop_item(3,Stack::new(blocks::item_named("stone").unwrap(),1),(2.5,64.0,0.5),(0.0,0.0,0.0),0);
    items.drop_item(4,Stack::new(blocks::item_named("nether_star").unwrap(),1),(2.5,64.0,0.5),(0.0,0.0,0.0),0);
    items.add_cart(crate::minecart::Minecart::new(5,crate::minecart::Kind::Plain,(3.5,64.0,0.5)));
    for (id,kind) in [(10,crate::mob::Kind::Cow),(11,crate::mob::Kind::Warden),(12,crate::mob::Kind::EnderDragon)] {
        items.add_mob(crate::mob::Mob::new(id,kind,(4.5,64.0,0.5)));
    }
    // UUID основан на часах; одинаковая начальная сцена должна отличаться
    // только cache limit, а не моментом создания.
    for item in items.items_mut() { item.uuid = [item.entity_id as u8;16]; }
    for mob in items.mobs_mut() { mob.uuid = [mob.entity_id as u8;16]; }
    for cart in items.carts_mut() { cart.uuid = [cart.entity_id as u8;16]; }
    items
}

#[test]
fn cached_push_matches_all_families_physics_damage_survivors_and_event_order() {
    for profile in [Profile::Java26_1_2,Profile::Preview26_3] {
        let catalog = Catalog::new(profile);
        let mut world = World::in_memory_with_catalog(catalog);
        for x in -2..=8 { for z in -2..=2 { world.set_block_silently(x,63,z,state("stone")); } }
        let mut baseline = entity_scene(catalog); let mut actual = entity_scene(catalog);
        let from_a = baseline.watch_changes(7); let from_b = actual.watch_changes(7);
        let grounded = Target { entity_id:40,x:3.5,y:64.0,z:0.5,sneaking:false,flying_creative:false };
        let players = [grounded,Target { entity_id:41,flying_creative:true,..grounded },
            Target { entity_id:42,sneaking:true,..grounded },Target { entity_id:43,x:8.5,..grounded },
            Target { entity_id:44,x:0.5,..grounded }];
        let mut disabled = ExposureCells::with_limit(&world,0);
        let mut cached = ExposureCells::new(&world);
        let expected = push_entities_with_cells(&mut disabled,&mut baseline,&players,(0.5,64.0,0.5),4.0);
        let result = push_entities_with_cells(&mut cached,&mut actual,&players,(0.5,64.0,0.5),4.0);
        assert_eq!(result,expected);
        assert_eq!(format!("{:?}",actual.items()),format!("{:?}",baseline.items()));
        assert_eq!(format!("{:?}",actual.carts()),format!("{:?}",baseline.carts()));
        assert_eq!(format!("{:?}",actual.mobs()),format!("{:?}",baseline.mobs()));
        let (expected_changes,_) = baseline.changes_since(from_a,7);
        let (changes,_) = actual.changes_since(from_b,7);
        assert_eq!(format!("{changes:?}"),format!("{expected_changes:?}"));
        assert!(matches!(changes.as_slice(),[items::Change::Gone { entity_id:3 }]));
        assert_eq!(actual.items().iter().map(|item|item.entity_id).collect::<Vec<_>>(),[1,2,4]);
        assert_eq!(actual.items()[0].fuse,Some(37));
        assert!(actual.mobs()[0].vitals.health < 10.0);
        assert_eq!((actual.mobs()[1].vx,actual.mobs()[1].vy,actual.mobs()[1].vz),(0.0,0.0,0.0));
        assert!(!result.0.iter().any(|(id,_)|*id==41));
        assert_eq!(result.1.iter().map(|(id,_)|*id).collect::<Vec<_>>(),[40,41,42,43,44]);
        assert!(cached.misses < disabled.misses);
        // Кэш не имеет доступа к mutable World RNG; прежние фазы не сдвинуты.
        let mut control = World::in_memory_with_catalog(catalog);
        assert_eq!(world.random_bits(),control.random_bits());
    }
}
