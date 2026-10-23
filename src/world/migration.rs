//! Явный офлайн-переход775→777: проверенная копия, неизменные append-only ID,
//! отдельный результат и журнал. Неизвестные файлы/плагины не угадываются.
use std::{fs::{self, File, OpenOptions}, io::{self, Read, Write}, path::{Path, PathBuf}};
use serde::{Deserialize, Serialize};
use super::{profile, startup, snapshot};
use crate::gameplay_catalog::{Catalog, Profile, Kind};

fn invalid(message: impl ToString) -> io::Error { io::Error::new(io::ErrorKind::InvalidData, message.to_string()) }
fn sync(path: &Path) -> io::Result<()> { File::open(path)?.sync_all() }
fn journal_path(world: &Path) -> PathBuf { world.with_file_name(format!(".{}-upgrade.json", world.file_name().unwrap().to_string_lossy())) }
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Journal { state: String, snapshot: String }
fn write_journal(world: &Path, journal: &Journal) -> io::Result<()> {
    let path = journal_path(world);
    let temp = path.with_extension("json.new");
    let mut file = OpenOptions::new().write(true).create_new(true).open(&temp)?;
    file.write_all(&serde_json::to_vec(journal).map_err(invalid)?)?; file.sync_all()?;
    fs::rename(temp, path)?; sync(world.parent().unwrap())
}
fn paths(world: &Path, id: &str) -> (PathBuf, PathBuf) {
    (world.with_extension(format!("migration-{id}")), world.with_extension(format!("previous-{id}")))
}

/// Вызывается с внешним замком до создания отсутствующего мира. Незавершённая
/// подготовка оставляет исходник; начатая активация завершается только после сверки.
pub(super) fn recover(world: &Path) -> io::Result<()> {
    let path = journal_path(world);
    if !path.exists() { return Ok(()); }
    let meta = fs::symlink_metadata(&path)?;
    if !meta.is_file() || meta.len() > 4096 { return Err(invalid("неверный журнал перехода")); }
    let mut journal: Journal = serde_json::from_slice(&fs::read(path)?).map_err(invalid)?;
    if journal.snapshot.len()!=32 || !journal.snapshot.bytes().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()) { return Err(invalid("неверный snapshot перехода")); }
    if matches!(journal.state.as_str(), "committed" | "aborted") { return Ok(()); }
    let (candidate, previous) = paths(world, &journal.snapshot);
    if matches!(journal.state.as_str(), "prepared" | "validated") {
        if !world.is_dir() || previous.exists() { return Err(invalid("неоднозначное восстановление перехода")); }
        journal.state="aborted".into(); return write_journal(world,&journal);
    }
    if journal.state != "activating" { return Err(invalid("неизвестный этап перехода")); }
    let backup_path = world.parent().unwrap().join("backups").join(world.file_name().unwrap()).join(&journal.snapshot);
    let backup = snapshot::verify_backup(&backup_path).map_err(invalid)?;
    if candidate.exists() {
        verify_result(&candidate, &backup)?;
        if world.exists() {
            if previous.exists() { return Err(invalid("обе исходные папки существуют")); }
            compare_tree(world, &backup.directory().join("world"), false)?;
            fs::rename(world, &previous)?; sync(world.parent().unwrap())?;
        }
        if !fs::symlink_metadata(&previous)?.is_dir() { return Err(invalid("нет исходной папки перехода")); }
        compare_tree(&previous,&backup.directory().join("world"),false)?;
        fs::rename(&candidate, world)?; sync(world.parent().unwrap())?;
    } else {
        if !previous.is_dir() { return Err(invalid("нет прежней папки перехода")); }
        if !fs::symlink_metadata(&previous)?.is_dir() {return Err(invalid("прежняя папка не обычный каталог"));}
        compare_tree(&previous,&backup.directory().join("world"),false)?;
        verify_result(world,&backup)?;
    }
    journal.state="committed".into(); write_journal(world,&journal)
}

/// Обход читает каждый чанк/игрока старым каталогом, включая вложенные вещи,
/// сущности, ульи, горшки и отложенные переносы поршней. World::open не вызывается.
fn validate(world: &Path) -> io::Result<(u64,u64)> {
    let catalog=Catalog::new(Profile::Java26_1_2);
    fn walk(root: &Path, path: &Path, catalog: Catalog, count: &mut (u64,u64)) -> io::Result<()> {
        for entry in fs::read_dir(path)? {
            let entry=entry?; let p=entry.path(); let meta=fs::symlink_metadata(&p)?;
            let rel=p.strip_prefix(root).unwrap();
            if meta.is_dir() {
                let text=rel.to_str().ok_or_else(||invalid("неверное имя каталога"))?;
                if !matches!(text,"region"|"playerdata"|"players"|"players/recipes"|"players/advancements"|"players/stats"|"dimensions"|"dimensions/minecraft"|"dimensions/minecraft/the_nether"|"dimensions/minecraft/the_end"|"dimensions/minecraft/the_nether/region"|"dimensions/minecraft/the_end/region") { return Err(invalid(format!("неподдержанный каталог {}",rel.display()))); }
                walk(root,&p,catalog,count)?; continue;
            }
            if !meta.is_file() { return Err(invalid(format!("необычный файл {}",rel.display()))); }
            let name=entry.file_name(); let name=name.to_str().ok_or_else(||invalid("имя файла не UTF-8"))?;
            match p.extension().and_then(|s|s.to_str()) {
                Some("mcs") if path.file_name().is_some_and(|v|v=="region") => {
                    super::region::validate_for_migration(&p)?;
                    for slot in 0..1024 {
                        match super::region::read_slot(&p,slot)? {
                            super::region::Stored::Missing=>(),
                            super::region::Stored::Damaged{why,..}=>return Err(invalid(format!("{} slot{slot}: {why}",rel.display()))),
                            super::region::Stored::Found(bytes)=> {
                                super::Chunk::decode_for_load_with_catalog(&bytes,catalog).map_err(|e|invalid(format!("{} slot{slot}: {e:?}",rel.display())))?;
                                count.0+=1;
                            }
                        }
                    }
                }
                Some("mcrw") if path.file_name().is_some_and(|v|v=="playerdata") => {
                    let hex=name.trim_end_matches(".mcrw").replace('-',"");
                    if hex.len()!=32 || !hex.bytes().all(|b|b.is_ascii_hexdigit()) { return Err(invalid("неверное имя playerdata")); }
                    let mut uuid=[0;16];
                    for (i,b) in uuid.iter_mut().enumerate() { *b=u8::from_str_radix(&hex[i*2..i*2+2],16).map_err(invalid)?; }
                    crate::playerdata::load_checked_for_catalog(path,&uuid,catalog).map_err(|e|invalid(format!("{}: {e:?}",rel.display())))?.ok_or_else(||invalid("исчез playerdata"))?;
                    count.1+=1;
                }
                Some("json") if rel.starts_with("players") => {
                    let text=fs::read_to_string(&p)?;
                    let value:serde_json::Value=serde_json::from_str(&text).map_err(invalid)?;
                    let valid=match path.file_name().and_then(|v|v.to_str()) {
                        Some("recipes")=>value.get("recipes").and_then(|v|v.as_array()).is_some_and(|a|a.iter().all(|v|v.is_string())) && value.get("settings").and_then(|v|v.as_array()).is_some_and(|a|a.len()==8 && a.iter().all(|v|v.is_boolean())),
                        Some("advancements")=>crate::advancements::AdvState::from_json(&text).is_some(),
                        Some("stats")=>crate::stats::Stats::from_json(&text).is_some(),
                        _=>false,
                    };
                    if !valid {return Err(invalid(format!("неверный JSON игрока {}",rel.display())));}
                }
                _ => match name {
                    "session.lock" | "mcsa-world.json" | "mcsa-storage.json" => (),
                    "level.dat" | "level.dat_old" => { super::read_level(&p).map_err(invalid)?.ok_or_else(||invalid("нет level.dat"))?; }
                    "portals.dat" => { for line in fs::read_to_string(&p)?.lines() { if !line.trim().is_empty() && line.split_whitespace().filter_map(|s|s.parse::<i32>().ok()).count()!=3 { return Err(invalid("неверные portals.dat")); } } }
                    "ticks.dat" => {
                        for line in fs::read_to_string(&p)?.lines().filter(|l|!l.trim().is_empty()) {
                            let (key,value)=line.split_once('=').ok_or_else(||invalid("неверная очередь тактов"))?;
                            let valid=match key.trim() {
                                "due"=>super::read_waiting(value.trim()).is_some(),
                                "moving"=>super::read_moving(value.trim()).is_some(),
                                "cargo"=>super::read_cargo(value.trim()).is_some_and(|(_,_,state)|catalog.name(Kind::State,state).is_ok()),
                                _=>false,
                            };
                            if !valid { return Err(invalid(format!("неподдержанный такт {}",rel.display()))); }
                        }
                    }
                    _=>return Err(invalid(format!("неподдержанные данные {} (включая данные плагинов)",rel.display()))),
                }
            }
        }
        Ok(())
    }
    let mut count=(0,0);walk(world,world,catalog,&mut count)?;
    if count.0==0 || !world.join("level.dat").is_file() { return Err(invalid("источник не распознан как сохранённый мир775")); }
    Ok(count)
}
fn copy_tree(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir(to)?;
    for entry in fs::read_dir(from)? { let e=entry?;let target=to.join(e.file_name());
        if e.file_type()?.is_dir(){copy_tree(&e.path(),&target)?;}else{fs::copy(e.path(),&target)?;sync(&target)?;}
    }
    sync(to)
}
fn compare_tree(actual: &Path, original: &Path, ignore_manifest: bool) -> io::Result<()> {
    let mut a=fs::read_dir(actual)?.map(|e|e.map(|v|v.file_name())).collect::<io::Result<Vec<_>>>()?;
    let mut b=fs::read_dir(original)?.map(|e|e.map(|v|v.file_name())).collect::<io::Result<Vec<_>>>()?;
    if ignore_manifest { a.retain(|n|n!=profile::MANIFEST_NAME);b.retain(|n|n!=profile::MANIFEST_NAME); }
    a.sort();b.sort(); if a!=b{return Err(invalid("состав результата отличается от снимка"));}
    for name in a { let x=actual.join(&name);let y=original.join(name);let meta=fs::symlink_metadata(&x)?;
        if meta.is_dir(){compare_tree(&x,&y,false)?;} else if !meta.is_file() || !same_bytes(&x,&y)?{return Err(invalid(format!("байты {} отличаются",x.display())));}
    } Ok(())
}
fn same_bytes(a: &Path, b: &Path) -> io::Result<bool> {
    let mut a=File::open(a)?;let mut b=File::open(b)?;
    if a.metadata()?.len()!=b.metadata()?.len(){return Ok(false);}
    let mut x=[0;65536];let mut y=[0;65536];
    loop {let n=a.read(&mut x)?;if n==0{return Ok(true);}b.read_exact(&mut y[..n])?;if x[..n]!=y[..n]{return Ok(false);}}
}
fn target_manifest(id: &str) -> profile::Manifest {
    profile::Manifest{schema:profile::MANIFEST_SCHEMA,save_format:profile::current_profile().save_format,
        registry_id:Catalog::new(Profile::Preview26_3).registry_id().into_owned(),gameplay_version:"26.3".into(),snapshot_id:id.into()}
}
fn verify_result(result: &Path, backup: &snapshot::VerifiedBackup) -> io::Result<()> {
    if !fs::symlink_metadata(result)?.is_dir() {return Err(invalid("результат перехода не обычный каталог"));}
    if profile::read_manifest(result).map_err(invalid)?!=Some(target_manifest(backup.snapshot_id())){return Err(invalid("неверный результат перехода"));}
    compare_tree(result,&backup.directory().join("world"),true)?;
    validate(result)?; Ok(())
}
/// Вызывается completion снимка: замки всех измерений удерживаются до commit.
pub(super) fn activate(guard: &startup::StartupGuard, backup: &snapshot::VerifiedBackup, uncertain: &mut bool) -> io::Result<()> {
    let world=guard.world_path();let source=backup.directory().join("world");
    validate(&source)?;
    let (candidate,previous)=paths(world,backup.snapshot_id());
    if candidate.exists() || previous.exists(){return Err(invalid("результат перехода уже существует"));}
    let mut journal=Journal{state:"prepared".into(),snapshot:backup.snapshot_id().into()};
    *uncertain=true;write_journal(world,&journal)?;
    copy_tree(&source,&candidate)?;
    let bytes=serde_json::to_vec(&target_manifest(backup.snapshot_id())).map_err(invalid)?;
    let p=candidate.join(profile::MANIFEST_NAME);let mut file=File::create(p)?;file.write_all(&bytes)?;file.sync_all()?;sync(&candidate)?;
    verify_result(&candidate,backup)?;
    journal.state="validated".into();write_journal(world,&journal)?;
    journal.state="activating".into();write_journal(world,&journal)?;
    recover(world)
}
pub(crate) fn upgrade(path: &Path) -> io::Result<()> {
    let guard=startup::lock_for_upgrade(path)?;
    // Внешние хранилища плагинов требуют отдельного согласованного перехода.
    for name in ["plugins","modules"] {
        let directory=guard.world_path().parent().unwrap().join(name);
        if directory.exists() && fs::read_dir(directory)?.next().transpose()?.is_some() { return Err(invalid("внешние данные плагинов: нет обработчика перехода")); }
    }
    if let Some(manifest)=profile::read_manifest(guard.world_path()).map_err(invalid)? {
        let source=profile::current_profile();
        if manifest.gameplay_version!=source.gameplay_version || manifest.registry_id!=source.registry_id || manifest.save_format!=source.save_format { return Err(invalid("нужен известный профиль26.1.2 текущей раскладки; понижение запрещено")); }
    }
    let backup=snapshot::create_profile_upgrade(&guard).map_err(invalid)?;
    println!("26.1.2 → 26.3: snapshot {} verified at {}",backup.snapshot_id(),backup.directory().display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Fixture(PathBuf);
    impl Fixture {
        fn new()->Self {
            let root=std::env::temp_dir().join(format!("mcsa-profile-upgrade-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
            fs::create_dir(&root).unwrap();
            fs::create_dir_all(root.join("world/dimensions/minecraft/the_nether")).unwrap();
            fs::create_dir_all(root.join("world/dimensions/minecraft/the_end")).unwrap();
            for dim in ["world","world/dimensions/minecraft/the_nether","world/dimensions/minecraft/the_end"] {
                let mut world=super::super::World::open(root.join(dim),Some(1),super::super::WorldKind::Flat).unwrap();
                world.ensure(0,0);world.set_block(1,65,1,crate::blocks::state_by_name("diamond_block").unwrap());world.save_all();
            }
            Self(root)
        }
        fn world(&self)->PathBuf{self.0.join("world")}
    }
    impl Drop for Fixture{fn drop(&mut self){fs::remove_dir_all(&self.0).unwrap();}}
    #[test]
    fn real_three_dimension_upgrade_verifies_backup_and_refuses_downgrade() {
        let f=Fixture::new();let before=validate(&f.world()).unwrap();assert!(before.0>=3);
        upgrade(&f.world()).unwrap();let manifest=profile::read_manifest(&f.world()).unwrap().unwrap();
        assert_eq!(manifest.gameplay_version,"26.3");assert_eq!(validate(&f.world()).unwrap(),before);
        let backup=snapshot::verify_backup(&f.0.join("backups/world").join(&manifest.snapshot_id)).unwrap();
        verify_result(&f.world(),&backup).unwrap();
        assert!(startup::prepare_for_version(&f.world(),mcsa_protocol::JavaVersion::V26_1_2).is_err());
        assert!(upgrade(&f.world()).is_err());
        assert!(startup::prepare_for_version(&f.world(),mcsa_protocol::JavaVersion::V26_3).is_ok());
    }
    #[test]
    fn unknown_plugin_file_refuses_activation_and_keeps_source_bytes() {
        let f=Fixture::new();fs::write(f.world().join("plugin.db"),b"opaque").unwrap();
        assert!(upgrade(&f.world()).is_err());assert_eq!(fs::read(f.world().join("plugin.db")).unwrap(),b"opaque");
        assert!(profile::read_manifest(&f.world()).unwrap().is_none());
    }
    #[test]
    fn occupied_world_and_corrupt_region_refuse_transition() {
        let f=Fixture::new();let guard=startup::lock_for_upgrade(&f.world()).unwrap();
        assert!(upgrade(&f.world()).is_err());drop(guard);
        fs::write(f.world().join("region/r.0.0.mcs"),b"unknown").unwrap();
        assert!(upgrade(&f.world()).is_err());assert!(profile::read_manifest(&f.world()).unwrap().is_none());
    }
    #[test]
    fn recovery_finishes_both_rename_boundaries_and_refuses_tampering() {
        for stage in [0,1,2] {
            let f=Fixture::new();upgrade(&f.world()).unwrap();let m=profile::read_manifest(&f.world()).unwrap().unwrap();
            let (candidate,previous)=paths(&f.world(),&m.snapshot_id);
            fs::rename(f.world(),&candidate).unwrap();
            if stage==0 { fs::rename(&previous,f.world()).unwrap(); }
            if stage==2 { fs::write(candidate.join("level.dat"),b"seed = 999\n").unwrap(); }
            let journal=Journal{state:"activating".into(),snapshot:m.snapshot_id};write_journal(&f.world(),&journal).unwrap();
            let result=startup::prepare_for_version(&f.world(),mcsa_protocol::JavaVersion::V26_3);
            if stage==2 {assert!(result.is_err());assert!(!f.world().exists());}
            else {assert!(result.is_ok());assert_eq!(profile::read_manifest(&f.world()).unwrap().unwrap().gameplay_version,"26.3");}
        }
    }
}
