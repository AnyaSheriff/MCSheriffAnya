use super::*;
use crate::{gameplay_catalog::{Catalog, Profile, WHITE_SLAB_ITEM}, inventory::Stack, mob::{Kind, Mob}};

fn native() -> Catalog { Catalog::new(Profile::Java26_1_2) }
fn preview() -> Catalog { Catalog::new(Profile::Preview26_3) }

#[test]
fn mob_count_overflow_is_rejected_before_serialization() {
    let mut chunk = Chunk::new();
    chunk.mobs = vec![Mob::new(1, Kind::Cow, (0.0, 64.0, 0.0)); usize::from(u16::MAX) + 1];
    assert!(matches!(chunk.to_bytes_with_catalog(native()), Err(DecodeError::InvalidMob { .. })));
    assert_eq!(chunk.mobs.len(), usize::from(u16::MAX) + 1);
}

fn named(name: &str, count: i32) -> Stack {
    Stack::new(crate::blocks::item_named(name).unwrap(), count).with_extras(crate::stackdata::Data {
        name: Some("Данные сохраняются".into()), repair_cost: 7, ..Default::default()
    })
}

#[test]
fn version_four_frozen_mobs_and_version_five_lossless_overlays_both_load() {
    let mut old = Chunk::flat().to_bytes(); old[0] = 4;
    // Числа контейнеров/TNT/рамок/мобов; сами записи взяты из независимых старых fixture.
    old.extend([0,0,0,0,0,0,0,2]);
    old.extend(include_bytes!("../mob/item_storage_fixtures/fox-stone-count1-legacy.bin"));
    old.extend(include_bytes!("../mob/item_storage_fixtures/villager-plain-offer-legacy.bin"));
    let mut chunk = Chunk::decode_for_load_with_catalog(&old, native()).unwrap();
    assert_eq!(chunk.mobs.len(), 2);
    assert_eq!(chunk.mobs[0].wild.mouth, Some(Stack::new(1,1)));
    chunk.mobs[0].wild.mouth = Some(named("stone",2));
    chunk.mobs[1].villager.offers[0].sell = named("diamond_sword",1).with_extras(crate::stackdata::Data {
        name: Some("Меч".into()), enchants: vec![(0,2)], ..Default::default()
    });
    let mut panda = Mob::new(3,Kind::Panda,(1.0,64.0,2.0)); panda.land.held = Some(named("bamboo",2));
    chunk.mobs.push(panda);
    let bytes = chunk.to_bytes_with_catalog(native()).unwrap(); assert_eq!(bytes[0],5);
    let loaded = Chunk::decode_for_load_with_catalog(&bytes,native()).unwrap();
    assert_eq!(loaded.mobs[0].wild.mouth,chunk.mobs[0].wild.mouth);
    assert_eq!(loaded.mobs[1].villager.offers,chunk.mobs[1].villager.offers);
    assert_eq!(loaded.mobs[2].land.held,chunk.mobs[2].land.held);
    assert_eq!(loaded.to_bytes_with_catalog(native()).unwrap(),bytes);
}

#[test]
fn mob_and_occupied_hive_items_obey_primary_catalog_on_read_and_write() {
    let mut chunk = Chunk::flat();
    let mut mob = Mob::new(1,Kind::Fox,(1.0,64.0,2.0)); mob.wild.mouth = Some(Stack::new(WHITE_SLAB_ITEM,2));
    chunk.mobs.push(mob);
    assert!(matches!(chunk.validate_catalog(native()),Err(DecodeError::InvalidMob { .. })));
    let bytes = chunk.to_bytes_with_catalog(preview()).unwrap();
    assert!(matches!(Chunk::decode_for_load_with_catalog(&bytes,native()),Err(DecodeError::InvalidMob { .. })));
    assert_eq!(Chunk::decode_for_load_with_catalog(&bytes,preview()).unwrap().mobs[0].wild.mouth,chunk.mobs[0].wild.mouth);
    chunk.mobs.clear();
    let mut bee = Mob::new(2,Kind::Bee,(1.0,64.0,2.0));
    bee.equipment = Some([Some(Stack::new(WHITE_SLAB_ITEM,1)),None,None,None,None,None]);
    let mut hive = crate::hive::Data::default(); assert!(hive.enter(&bee));
    chunk.set(0,64,0,crate::blocks::state_by_name("beehive").unwrap()); chunk.hives.insert((0,64,0),hive);
    assert!(matches!(chunk.validate_catalog(native()),Err(DecodeError::InvalidHive { .. })));
    let bytes = chunk.to_bytes_with_catalog(preview()).unwrap();
    assert!(matches!(Chunk::decode_for_load_with_catalog(&bytes,native()),Err(DecodeError::InvalidHive { .. })));
    assert_eq!(Chunk::decode_for_load_with_catalog(&bytes,preview()).unwrap().hives,chunk.hives);
}

#[test]
fn tampered_annex_is_explicit_mob_incompatibility_never_damaged_terrain() {
    let mut chunk = Chunk::flat();
    let mut fox = Mob::new(1,Kind::Fox,(1.0,64.0,2.0)); fox.wild.mouth = Some(Stack::new(1,2));
    chunk.mobs.push(fox);
    let mut bytes = chunk.to_bytes_with_catalog(native()).unwrap();
    let suffix = [0,0,0,11,1,2,1,0,0,0,1,0,0,0,2];
    assert!(bytes.ends_with(&suffix));
    let schema = bytes.len()-11; bytes[schema] = 2;
    assert!(matches!(Chunk::decode_for_load_with_catalog(&bytes,native()),Err(DecodeError::InvalidMob { .. })));
    bytes.truncate(bytes.len()-1);
    assert!(matches!(Chunk::decode_for_load_with_catalog(&bytes,native()),Err(DecodeError::InvalidMob { .. })));
}

#[test]
fn invalid_preflight_retains_dirty_chunk_and_previous_region_bytes() {
    let path = std::env::temp_dir().join(format!("mcsa-mob-preflight-{}",std::process::id()));
    assert!(!path.exists());
    let mut world = World::open(&path,None,WorldKind::Flat).unwrap(); world.ensure(0,0); world.save_all();
    let regions = path.join(REGIONS); let before = region::read(&regions,0,0).unwrap().unwrap();
    let mut fox = Mob::new(1,Kind::Fox,(1.0,64.0,2.0)); fox.wild.mouth = Some(Stack::new(WHITE_SLAB_ITEM,1));
    world.chunks.get_mut(&(0,0)).unwrap().mobs.push(fox); world.dirty.insert((0,0));
    world.save_all();
    assert!(world.dirty.contains(&(0,0)));
    assert!(writer::waiting(&regions,0,0).is_none());
    assert_eq!(region::read(&regions,0,0).unwrap().unwrap(),before);
    world.chunks.get_mut(&(0,0)).unwrap().mobs.clear(); world.dirty.remove(&(0,0));
    drop(world); std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn unknown_mob_annex_on_disk_is_preserved_without_generation_or_quarantine() {
    let path = std::env::temp_dir().join(format!("mcsa-mob-layout-refusal-{}",std::process::id()));
    assert!(!path.exists());
    // Сначала создаём корректный обычный мир с level.dat. Проверяется отказ
    // конкретного моба в регионе, а не отсутствие исходного семени мира.
    let mut original = World::open(&path, None, WorldKind::Flat).unwrap();
    original.ensure(0, 0); original.save_all(); drop(original);
    let mut chunk = Chunk::flat();
    let mut fox = Mob::new(1,Kind::Fox,(1.0,64.0,2.0)); fox.wild.mouth = Some(Stack::new(1,2)); chunk.mobs.push(fox);
    let mut bytes = chunk.to_bytes_with_catalog(native()).unwrap(); let schema = bytes.len()-11; assert_eq!(bytes[schema],1); bytes[schema]=2;
    let regions = path.join(REGIONS); std::fs::create_dir_all(&regions).unwrap(); region::write(&regions,0,0,&bytes).unwrap();
    let mut world = World::open(&path,None,WorldKind::Flat).unwrap(); world.ensure(0,0);
    assert!(!world.chunks.contains_key(&(0,0)),"несовместимый чанк не заменяется новым");
    assert!(!world.dirty.contains(&(0,0)));
    assert_eq!(region::read(&regions,0,0).unwrap().unwrap(),bytes);
    assert!(!regions.join("corrupt").exists(),"неподдержанный формат не считается повреждением");
    drop(world); assert_eq!(region::read(&regions,0,0).unwrap().unwrap(),bytes);
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn malformed_occupied_entity_has_explicit_hive_error() {
    let bee = Mob::new(1,Kind::Bee,(1.0,64.0,2.0));
    let mut hive = crate::hive::Data::default(); assert!(hive.enter(&bee));
    hive.bees[0].entity.push(0);
    let mut chunk = Chunk::flat(); chunk.set(0,64,0,crate::blocks::state_by_name("beehive").unwrap()); chunk.hives.insert((0,64,0),hive);
    let bytes = chunk.to_bytes();
    assert!(matches!(Chunk::decode_for_load_with_catalog(&bytes,native()),Err(DecodeError::InvalidHive { .. })));
}

#[test]
fn idle_comparison_preserves_invalid_current_and_replaces_invalid_previous_without_panic() {
    let mut invalid = Mob::new(1,Kind::Fox,(0.5,64.0,0.5)); invalid.persistent=true;
    invalid.wild.mouth=Some(Stack::new(1,2).with_extras(crate::stackdata::Data {
        name:Some("я".repeat(40_000)), ..Default::default()
    }));
    assert!(invalid.validate_item_storage().is_err());
    let mut valid=invalid.clone(); valid.wild.mouth=Some(Stack::new(1,2));
    for (previous,now) in [(valid.clone(),invalid.clone()),(invalid.clone(),valid.clone()),(invalid.clone(),invalid.clone())] {
        let mut world=World::in_memory(); world.ensure(0,0);
        world.chunks.get_mut(&(0,0)).unwrap().mobs=vec![previous]; world.dirty.clear();
        world.store_mobs_if_changed(std::slice::from_ref(&now));
        assert!(world.dirty.contains(&(0,0)));
        let chunk=world.chunks.get(&(0,0)).unwrap();
        assert_eq!(chunk.mobs.len(),1);
        assert_eq!(chunk.mobs[0].wild.mouth,now.wild.mouth);
        assert_eq!(chunk.validate_catalog(native()).is_err(),now.validate_item_storage().is_err());
    }
}
