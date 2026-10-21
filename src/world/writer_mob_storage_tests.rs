use super::*;
use crate::{inventory::Stack,mob::{Kind,Mob},gameplay_catalog::{Catalog,Profile}};

#[test]
fn pending_snapshot_and_disk_preserve_complete_mob_items_after_retry() {
    let directory = std::env::temp_dir().join(format!("mcsa-writer-mob-items-{}",std::process::id()));
    assert!(!directory.exists()); std::fs::write(&directory,b"blocked").unwrap();
    let mut chunk = Chunk::flat();
    let mut fox = Mob::new(1,Kind::Fox,(1.0,64.0,2.0)); fox.wild.mouth = Some(Stack::new(1,2));
    let mut panda = Mob::new(2,Kind::Panda,(1.0,64.0,2.0));
    panda.land.held = Some(Stack::new(crate::blocks::item_named("bamboo").unwrap(),2));
    let mut trader = Mob::read(&mut include_bytes!("../mob/item_storage_fixtures/villager-plain-offer-legacy.bin").as_slice()).unwrap();
    trader.villager.offers[0].sell = Stack::new(crate::blocks::item_named("diamond_sword").unwrap(),1).with_extras(crate::stackdata::Data {
        name: Some("Меч".into()), enchants: vec![(0,2)], ..Default::default()
    });
    chunk.mobs = vec![fox,panda,trader];
    let snapshot = Arc::new(Snapshot::new(chunk.clone()));
    let key = (directory.clone(),0,0); pending().lock().unwrap().insert(key,Arc::clone(&snapshot));
    let job = Job::Chunks { directory: directory.clone(), chunks: vec![(0,0,snapshot)] };
    assert!(attempt(&job).is_err());
    let bytes = waiting(&directory,0,0).unwrap();
    let loaded = Chunk::decode_for_load_with_catalog(&bytes,Catalog::new(Profile::Java26_1_2)).unwrap();
    assert_eq!(loaded.mobs[0].wild.mouth,chunk.mobs[0].wild.mouth);
    assert_eq!(loaded.mobs[1].land.held,chunk.mobs[1].land.held);
    assert_eq!(loaded.mobs[2].villager.offers,chunk.mobs[2].villager.offers);
    assert_eq!(loaded.to_bytes(),*bytes);
    std::fs::remove_file(&directory).unwrap(); attempt(&job).unwrap(); finish(job);
    assert!(waiting(&directory,0,0).is_none());
    let disk = region::read(&directory,0,0).unwrap().unwrap(); assert_eq!(disk,*bytes);
    let disk_loaded = Chunk::decode_for_load_with_catalog(&disk,Catalog::new(Profile::Java26_1_2)).unwrap();
    assert_eq!(disk_loaded.mobs[0].wild.mouth,chunk.mobs[0].wild.mouth);
    assert_eq!(disk_loaded.mobs[1].land.held,chunk.mobs[1].land.held);
    assert_eq!(disk_loaded.mobs[2].villager.offers,chunk.mobs[2].villager.offers);
    std::fs::remove_dir_all(directory).unwrap();
}
