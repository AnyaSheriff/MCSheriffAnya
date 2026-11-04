//! Собственный ограниченный файл обычных предметов; не формат чанков Minecraft.
use super::*;
use sha2::{Digest, Sha256};
use std::{collections::HashSet, fs::{self, OpenOptions}, io::{self, Read, Write}, path::Path, sync::atomic::{AtomicU64, Ordering}};

const MAGIC: &[u8; 8] = b"MCSAGND\0";
const VERSION: u32 = 1;
const MAX_BYTES: usize = 64 * 1024 * 1024;
const MAX_ITEMS: usize = 100_000;
const MAX_RECORD: usize = 128 * 1024;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);
fn invalid() -> io::Error { io::Error::new(io::ErrorKind::InvalidData, "Повреждённый файл лежащих предметов") }
fn profile(catalog: crate::gameplay_catalog::Catalog) -> u8 {
    match catalog.profile() { crate::gameplay_catalog::Profile::Java26_1_2 => 0, crate::gameplay_catalog::Profile::Preview26_3 => 1 }
}
fn supported(item: &Dropped) -> bool { item.block.is_none() && item.fuse.is_none() && item.owner.is_none() }
fn check(item: &Dropped, catalog: crate::gameplay_catalog::Catalog) -> io::Result<()> {
    catalog.validate_stack(&item.stack).map_err(|_| invalid())?;
    if ![item.x,item.y,item.z,item.vx,item.vy,item.vz].iter().all(|v| v.is_finite()) || !item.health.is_finite() || item.health <= 0.0 || item.custom.is_some() { return Err(invalid()); }
    Ok(())
}
fn record(item: &Dropped, catalog: crate::gameplay_catalog::Catalog) -> io::Result<Vec<u8>> {
    check(item, catalog)?;
    let mut out = Vec::new();
    out.extend(item.uuid);
    out.extend(item.stack.stored_item().to_be_bytes()); out.extend(item.stack.count.to_be_bytes()); item.stack.write_extras(&mut out);
    out.push(u8::from(item.thrower.is_some())); if let Some(uuid) = item.thrower { out.extend(uuid); }
    for value in [item.x,item.y,item.z,item.vx,item.vy,item.vz] { out.extend(value.to_be_bytes()); }
    out.extend(item.health.to_be_bytes());
    for value in [item.age,item.ticks,item.pickup_delay] { out.extend(value.to_be_bytes()); }
    out.push(u8::from(item.on_ground)); out.push(u8::from(item.no_gravity));
    out.push(match item.in_water { None => 0, Some(false) => 1, Some(true) => 2 });
    Ok(out)
}
struct Input<'a>(&'a [u8]);
impl<'a> Input<'a> {
    fn take(&mut self, n: usize) -> io::Result<&'a [u8]> { let value = self.0.get(..n).ok_or_else(invalid)?; self.0 = &self.0[n..]; Ok(value) }
    fn u8(&mut self) -> io::Result<u8> { Ok(self.take(1)?[0]) }
    fn u32(&mut self) -> io::Result<u32> { Ok(u32::from_be_bytes(self.take(4)?.try_into().map_err(|_| invalid())?)) }
    fn u64(&mut self) -> io::Result<u64> { Ok(u64::from_be_bytes(self.take(8)?.try_into().map_err(|_| invalid())?)) }
    fn bool(&mut self) -> io::Result<bool> { match self.u8()? { 0 => Ok(false), 1 => Ok(true), _ => Err(invalid()) } }
    fn f64(&mut self) -> io::Result<f64> { Ok(f64::from_be_bytes(self.take(8)?.try_into().map_err(|_| invalid())?)) }
}
fn decode_record(bytes: &[u8], catalog: crate::gameplay_catalog::Catalog) -> io::Result<Dropped> {
    let mut input = Input(bytes);
    let uuid = input.take(16)?.try_into().map_err(|_| invalid())?;
    let stored = input.u32()? as i32; let count = input.u32()? as i32;
    let extras = Stack::read_extras(stored, |n| input.take(n).ok()).ok_or_else(invalid)?;
    let mut stack = Stack::from_stored(stored, count); if let Some(data) = extras { stack = stack.with_extras(data); }
    let thrower = if input.bool()? { Some(input.take(16)?.try_into().map_err(|_| invalid())?) } else { None };
    let at = (input.f64()?,input.f64()?,input.f64()?);
    let velocity = (input.f64()?,input.f64()?,input.f64()?);
    let mut item = Items::new_item(0, stack, at, velocity, 0);
    item.uuid = uuid; item.thrower = thrower;
    item.health = f32::from_be_bytes(input.take(4)?.try_into().map_err(|_| invalid())?);
    item.age = input.u64()?; item.ticks = input.u64()?; item.pickup_delay = input.u64()?;
    item.on_ground = input.bool()?; item.no_gravity = input.bool()?;
    item.in_water = match input.u8()? { 0 => None, 1 => Some(false), 2 => Some(true), _ => return Err(invalid()) };
    if !input.0.is_empty() || record(&item, catalog)? != bytes { return Err(invalid()); }
    Ok(item)
}
impl Items {
    /// Только ordinary owner=None: /give-визуализации и owner remainder намеренно исключены.
    pub(crate) fn save_ground_items(&self, path: &Path) -> io::Result<()> {
        let mut out = Vec::new(); out.extend(MAGIC); out.extend(VERSION.to_be_bytes()); out.push(profile(self.catalog));
        let count = self.items.iter().filter(|item| supported(item)).count();
        if count > MAX_ITEMS { return Err(invalid()); }
        out.extend((count as u32).to_be_bytes());
        let mut uuids = HashSet::new();
        for item in self.items.iter().filter(|item| supported(item)) {
            if !uuids.insert(item.uuid) { return Err(invalid()); }
            let bytes = record(item, self.catalog)?;
            if bytes.len() > MAX_RECORD || out.len() + 4 + bytes.len() + 32 > MAX_BYTES { return Err(invalid()); }
            out.extend((bytes.len() as u32).to_be_bytes()); out.extend(bytes);
        }
        let digest = Sha256::digest(&out); out.extend_from_slice(&digest);
        let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
        let nonce = SystemTime::now().duration_since(UNIX_EPOCH).map_err(io::Error::other)?.as_nanos();
        let temporary = parent.join(format!(".mcsa-ground-{}-{nonce}-{}.tmp", std::process::id(), TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)));
        let mut file = OpenOptions::new().write(true).create_new(true).open(&temporary)?;
        let result = (|| {
            file.write_all(&out)?; file.sync_all()?; fs::rename(&temporary, path)?;
            #[cfg(unix)]
            std::fs::File::open(parent)?.sync_all()?;
            Ok(())
        })();
        if result.is_err() { let _ = fs::remove_file(&temporary); }
        result
    }
    /// Запуск один раз до тиков. Полный разбор предшествует выдаче runtime ID и публикации.
    pub(crate) fn load_ground_items(&mut self, path: &Path, next_id: &mut dyn FnMut() -> i32) -> io::Result<()> {
        if self.ground_items_loaded || !self.items.is_empty() { return Err(io::Error::new(io::ErrorKind::AlreadyExists, "Лежащие предметы уже загружены")); }
        let file = match std::fs::File::open(path) { Ok(file) => file, Err(e) if e.kind() == io::ErrorKind::NotFound => { self.ground_items_loaded = true; return Ok(()); }, Err(e) => return Err(e) };
        let mut bytes = Vec::new(); file.take(MAX_BYTES as u64 + 1).read_to_end(&mut bytes)?;
        if bytes.len() > MAX_BYTES { return Err(invalid()); }
        if bytes.len() < 32 { return Err(invalid()); }
        let (payload, checksum) = bytes.split_at(bytes.len() - 32);
        if &Sha256::digest(payload)[..] != checksum { return Err(invalid()); }
        let mut input = Input(payload);
        if input.take(8)? != MAGIC || input.u32()? != VERSION || input.u8()? != profile(self.catalog) { return Err(invalid()); }
        let count = input.u32()? as usize;
        if count > MAX_ITEMS || count > input.0.len() / 4 { return Err(invalid()); }
        let mut loaded = Vec::with_capacity(count); let mut uuids = HashSet::new();
        for _ in 0..count {
            let length = input.u32()? as usize; if length > MAX_RECORD { return Err(invalid()); }
            let item = decode_record(input.take(length)?, self.catalog)?;
            if !uuids.insert(item.uuid) { return Err(invalid()); }
            loaded.push(item);
        }
        if !input.0.is_empty() { return Err(invalid()); }
        for mut item in loaded { item.entity_id = next_id(); self.changes.push(Change::Dropped(item.clone())); self.items.push(item); }
        self.ground_items_loaded = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gameplay_catalog::{Catalog, Profile, WHITE_SLAB_ITEM};
    fn root(label: &str) -> std::path::PathBuf {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tmp").join(format!("ground-{label}-{}-{}", std::process::id(), TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)));
        fs::create_dir_all(&path).unwrap(); path
    }
    fn fixture(catalog: Catalog) -> Items {
        let mut items = Items::with_catalog(catalog);
        let stack = Stack::new(crate::blocks::item_named("diamond_sword").unwrap(), 1).with_extras(crate::stackdata::Data {
            name: Some("Сохранённый меч".into()), damage: 19, enchants: vec![(0,2)], ..Default::default()
        });
        let mut item = Items::new_item(3, stack, (-0.5,71.25,4.0), (0.1,-0.04,0.03), u64::MAX);
        item.uuid = [33;16]; item.thrower = Some([44;16]); item.age = 5990; item.ticks = 8123;
        item.health = 2.5; item.no_gravity = true; item.on_ground = true; item.in_water = Some(true);
        item.impulse = true; item.touchable = Some((1.0,2.0,3.0)); items.items.push(item); items
    }
    #[test]
    fn ground_roundtrip_preserves_metadata_physics_uuid_and_resets_network_state() {
        for profile in [Profile::Java26_1_2, Profile::Preview26_3] {
            let catalog = Catalog::new(profile); let root = root("roundtrip"); let path = root.join("ground");
            let source = fixture(catalog); source.save_ground_items(&path).unwrap();
            let mut loaded = Items::with_catalog(catalog); loaded.watch_changes(1); let mut id = 100;
            loaded.load_ground_items(&path, &mut || { id += 1; id }).unwrap();
            assert_eq!(loaded.items.len(),1); let a = &source.items[0]; let b = &loaded.items[0];
            assert_eq!(record(a,catalog).unwrap(),record(b,catalog).unwrap()); assert_eq!(b.entity_id,101);
            assert!(!b.impulse); assert_eq!(b.touchable,None); assert_eq!(b.shown_fuse,0);
            assert_eq!(b.stack.extras(),a.stack.extras()); assert_eq!(b.pickup_delay,u64::MAX);
            assert!(matches!(loaded.changes_since(0,1).0.as_slice(), [Change::Dropped(_)]));
            assert!(loaded.load_ground_items(&path,&mut || 102).is_err()); assert_eq!(loaded.items.len(),1);
            loaded.save_ground_items(&path).unwrap();
            let mut second = Items::with_catalog(catalog); second.load_ground_items(&path,&mut || 201).unwrap();
            assert_eq!(record(&second.items[0],catalog).unwrap(),record(a,catalog).unwrap());
            fs::remove_dir_all(root).unwrap();
        }
    }
    #[test]
    fn ground_invalid_save_preserves_file_and_invalid_load_never_mutates_items() {
        let catalog = Catalog::new(Profile::Preview26_3); let root = root("invalid"); let path = root.join("ground");
        let mut source = fixture(catalog); source.save_ground_items(&path).unwrap(); let original = fs::read(&path).unwrap();
        let valid = source.items[0].clone();
        for invalid_stack in [Stack::new(999_999,1), valid.stack.with_extras(crate::stackdata::Data {
            contents: vec![crate::stackdata::Content { slot:0,item:999_999,count:1,data:None }], ..Default::default()
        }), valid.stack.with_extras(crate::stackdata::Data { name:Some("я".repeat(70_000)), ..Default::default() })] {
            source.items[0].stack = invalid_stack; assert!(source.save_ground_items(&path).is_err()); assert_eq!(fs::read(&path).unwrap(),original);
        }
        source.items[0] = valid.clone(); source.items[0].x = f64::NAN;
        assert!(source.save_ground_items(&path).is_err()); assert_eq!(fs::read(&path).unwrap(),original);
        source.items[0] = valid.clone(); source.items[0].health = f32::INFINITY;
        assert!(source.save_ground_items(&path).is_err()); assert_eq!(fs::read(&path).unwrap(),original);
        source.items[0] = valid.clone(); source.items.push(valid);
        assert!(source.save_ground_items(&path).is_err()); assert_eq!(fs::read(&path).unwrap(),original);
        let mut malformed = Vec::new();
        for n in 0..original.len() { malformed.push(original[..n].to_vec()); }
        let mut semantic = Vec::new();
        let mut corruption = original.clone(); corruption[21] ^= 1; malformed.push(corruption);
        let mut trailing = original.clone(); trailing.insert(trailing.len()-32,0); semantic.push(trailing);
        let mut nan = original.clone(); let at = nan.len() - 32 - 79; nan[at..at+8].copy_from_slice(&f64::NAN.to_be_bytes()); semantic.push(nan);
        let mut dead = original.clone(); let at = dead.len() - 32 - 31; dead[at..at+4].copy_from_slice(&0f32.to_be_bytes()); semantic.push(dead);
        let mut oversized = original.clone(); oversized[17..21].copy_from_slice(&((MAX_RECORD + 1) as u32).to_be_bytes()); semantic.push(oversized);
        let mut count = original.clone(); count[13..17].copy_from_slice(&((MAX_ITEMS + 1) as u32).to_be_bytes()); semantic.push(count);
        let mut duplicate = original[..original.len()-32].to_vec(); duplicate[13..17].copy_from_slice(&2u32.to_be_bytes()); duplicate.extend(&original[17..original.len()-32]); duplicate.extend([0;32]); semantic.push(duplicate);
        for mut bytes in semantic { bytes.truncate(bytes.len()-32); let digest=Sha256::digest(&bytes); bytes.extend_from_slice(&digest); malformed.push(bytes); }
        for bytes in malformed {
            fs::write(&path,bytes).unwrap(); let mut loaded = Items::with_catalog(catalog); let mut calls = 0;
            assert!(loaded.load_ground_items(&path,&mut || {calls+=1; 9}).is_err()); assert_eq!(calls,0); assert!(loaded.items.is_empty()); assert!(!loaded.ground_items_loaded);
        }
        fs::write(&path,&original).unwrap(); let mut incompatible = Items::new(); assert!(incompatible.load_ground_items(&path,&mut || 1).is_err());
        let mut old = fixture(Catalog::new(Profile::Java26_1_2)); old.items[0].stack = Stack::new(WHITE_SLAB_ITEM,1);
        assert!(old.save_ground_items(&path).is_err()); assert_eq!(fs::read(&path).unwrap(),original);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn ground_excludes_visual_owner_falling_and_primed_records_and_legacy_missing_is_once() {
        let catalog = Catalog::new(Profile::Java26_1_2); let root = root("scope"); let path = root.join("ground");
        let mut empty = Items::with_catalog(catalog); empty.load_ground_items(&path,&mut || 1).unwrap();
        assert!(empty.load_ground_items(&path,&mut || 1).is_err());
        let mut source = fixture(catalog); let base = source.items[0].clone();
        for index in 0..3 { let mut item=base.clone(); item.uuid=[index;16]; if index==0 {item.owner=Some(42);} else if index==1 {item.block=Some(1);} else {item.fuse=Some(20);} item.stack=Stack::new(999_999,1); source.items.push(item); }
        source.save_ground_items(&path).unwrap(); let mut loaded = Items::with_catalog(catalog); loaded.load_ground_items(&path,&mut || 100).unwrap();
        assert_eq!(loaded.items.len(),1); assert_eq!(loaded.items[0].uuid,base.uuid);
        let mut nonempty = fixture(catalog); assert!(nonempty.load_ground_items(&path,&mut || 1).is_err()); assert_eq!(nonempty.items.len(),1);
        fs::remove_dir_all(root).unwrap();
    }
}
