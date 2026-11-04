//! Явный офлайн-переход775→777: проверенная копия, неизменные append-only ID,
//! отдельный результат и журнал. Неизвестные файлы/плагины не угадываются.
use std::{fs::{self, File, OpenOptions}, io::{self, Read, Write, Seek, SeekFrom}, path::{Path, PathBuf}};
use serde::{Deserialize, Serialize};
use super::{profile, startup, snapshot};
use crate::gameplay_catalog::{Catalog, Profile, Kind};

fn invalid(message: impl ToString) -> io::Error { io::Error::new(io::ErrorKind::InvalidData, message.to_string()) }
fn sync(path: &Path) -> io::Result<()> { File::open(path)?.sync_all() }
fn journal_path(world: &Path) -> PathBuf { world.with_file_name(format!(".{}-upgrade.json", world.file_name().unwrap().to_string_lossy())) }
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Journal { state: String, snapshot: String, #[serde(default)] source: Source }
#[derive(Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all="kebab-case")]
enum Source { #[default] Migration, #[serde(rename="main-26.1.2")] Main26_1_2 }
fn convert_main(bytes: &[u8]) -> io::Result<Vec<u8>> {
    crate::playerdata::convert_main_playerdata(bytes, Catalog::new(Profile::Java26_1_2))
        .map_err(|e| invalid(format!("main26.1.2 playerdata: {e:?}")))
}
fn write_journal(world: &Path, journal: &Journal) -> io::Result<()> {
    let path = journal_path(world);
    // Незавершённый temp после падения сохраняется для разбора, но не блокирует
    // следующий durable rename. Чужой/старый .json.new не удаляем и не заменяем.
    let mut created = None;
    for _ in 0..8 {
        let mut suffix=[0u8;16];
        aws_lc_rs::rand::fill(&mut suffix).map_err(|_|io::Error::other("нет случайных данных для журнала перехода"))?;
        let suffix=suffix.iter().map(|b|format!("{b:02x}")).collect::<String>();
        let temp=path.with_extension(format!("json.new-{suffix}"));
        match OpenOptions::new().write(true).create_new(true).open(&temp) {
            Ok(file)=>{created=Some((temp,file));break;}
            Err(error) if error.kind()==io::ErrorKind::AlreadyExists=>continue,
            Err(error)=>return Err(error),
        }
    }
    let (temp,mut file)=created.ok_or_else(||io::Error::new(io::ErrorKind::AlreadyExists,"не удалось выделить временный журнал перехода"))?;
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
        verify_result_for_source(&candidate, &backup, journal.source)?;
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
        verify_result_for_source(world,&backup,journal.source)?;
    }
    journal.state="committed".into(); write_journal(world,&journal)
}

/// Обход читает каждый чанк/игрока старым каталогом, включая вложенные вещи,
/// сущности, ульи, горшки и отложенные переносы поршней. World::open не вызывается.
fn validate(world: &Path) -> io::Result<(u64,u64)> { validate_source(world, Source::Migration) }
fn validate_source(world: &Path, source: Source) -> io::Result<(u64,u64)> {
    let catalog=Catalog::new(Profile::Java26_1_2);
    fn walk(root: &Path, path: &Path, catalog: Catalog, count: &mut (u64,u64), source: Source) -> io::Result<()> {
        for entry in fs::read_dir(path)? {
            let entry=entry?; let p=entry.path(); let meta=fs::symlink_metadata(&p)?;
            let rel=p.strip_prefix(root).unwrap();
            if meta.is_dir() {
                let text=rel.to_str().ok_or_else(||invalid("неверное имя каталога"))?;
                if !matches!(text,"region"|"playerdata"|"players"|"players/recipes"|"players/advancements"|"players/stats"|"dimensions"|"dimensions/minecraft"|"dimensions/minecraft/the_nether"|"dimensions/minecraft/the_end"|"dimensions/minecraft/the_nether/region"|"dimensions/minecraft/the_end/region") { return Err(invalid(format!("неподдержанный каталог {}",rel.display()))); }
                walk(root,&p,catalog,count,source)?; continue;
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
                                if source == Source::Main26_1_2 {
                                    super::Chunk::decode_for_main_import_with_catalog(&bytes,catalog)
                                } else { super::Chunk::decode_for_load_with_catalog(&bytes,catalog) }
                                    .map_err(|e|invalid(format!("{} slot{slot}: {e:?}",rel.display())))?;
                                count.0+=1;
                            }
                        }
                    }
                }
                Some("mcrw") if path.file_name().is_some_and(|v|v=="playerdata") => {
                    // Оба наших writer/path_for пишут ровно32 lowercase hex.
                    // Иначе runtime ищет другое имя и ошибочно создаст нового игрока.
                    let hex=name.strip_suffix(".mcrw").ok_or_else(||invalid("неверное имя playerdata"))?;
                    if hex.len()!=32 || !hex.bytes().all(|b|b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
                        return Err(invalid("неканоническое имя playerdata: нужны32строчные hex цифры без дефисов"));
                    }
                    let mut uuid=[0;16];
                    for (i,b) in uuid.iter_mut().enumerate() { *b=u8::from_str_radix(&hex[i*2..i*2+2],16).map_err(invalid)?; }
                    if source == Source::Main26_1_2 { convert_main(&fs::read(&p)?)?; }
                    else { crate::playerdata::load_checked_for_catalog(path,&uuid,catalog).map_err(|e|invalid(format!("{}: {e:?}",rel.display())))?.ok_or_else(||invalid("исчез playerdata"))?; }
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
    let mut count=(0,0);walk(world,world,catalog,&mut count,source)?;
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
    compare_transformed_tree(actual, original, ignore_manifest, Source::Migration)
}
fn compare_transformed_tree(actual: &Path, original: &Path, ignore_manifest: bool, source: Source) -> io::Result<()> {
    let mut a=fs::read_dir(actual)?.map(|e|e.map(|v|v.file_name())).collect::<io::Result<Vec<_>>>()?;
    let mut b=fs::read_dir(original)?.map(|e|e.map(|v|v.file_name())).collect::<io::Result<Vec<_>>>()?;
    if ignore_manifest { a.retain(|n|n!=profile::MANIFEST_NAME);b.retain(|n|n!=profile::MANIFEST_NAME); }
    a.sort();b.sort(); if a!=b{return Err(invalid("состав результата отличается от снимка"));}
    for name in a { let x=actual.join(&name);let y=original.join(name);let meta=fs::symlink_metadata(&x)?;
        if meta.is_dir(){compare_transformed_tree(&x,&y,false,source)?;}
        else {
            if !meta.is_file() || !fs::symlink_metadata(&y)?.is_file() { return Err(invalid("необычный файл результата")); }
            let equal = if source == Source::Main26_1_2 && original.file_name().is_some_and(|n|n=="region") && y.extension().is_some_and(|n|n=="mcs") {
                super::region::validate_for_migration(&x)?;
                main_region(&y, &mut File::open(&x)?, true)?;
                true
            } else if source == Source::Main26_1_2 && original.file_name().is_some_and(|n|n=="playerdata") && y.extension().is_some_and(|n|n=="mcrw") {
                fs::read(&x)? == convert_main(&fs::read(&y)?)?
            } else { same_bytes(&x,&y)? };
            if !equal {return Err(invalid(format!("байты {} отличаются",x.display())));}
        }
    } Ok(())
}
// Канонический MCS1: таблица1024×8, сектор256, CRC32+zstd. Прежние
// свободные сектора остаются только в снимке; результат сравнивается весь,
// включая header, padding и точную длину. В памяти — один чанк за раз.
fn main_region(source: &Path, output: &mut File, verify: bool) -> io::Result<()> {
    const UNIT: usize=256;
    const HEADER: usize=(8_usize+1024*8).next_multiple_of(UNIT);
    super::region::validate_for_migration(source)?;
    let mut source_head=[0;8];File::open(source)?.read_exact(&mut source_head)?;
    if source_head != [b'M',b'C',b'S',1,1,0,0,0] {return Err(invalid("неизвестный header main региона"));}
    let mut header=vec![0;HEADER];header[..8].copy_from_slice(&source_head);
    let mut offset=HEADER as u64;
    fn emit(output: &mut File, offset: u64, bytes: &[u8], verify: bool) -> io::Result<()> {
        output.seek(SeekFrom::Start(offset))?;
        if verify {
            let mut actual=vec![0;bytes.len()];output.read_exact(&mut actual)?;
            if actual!=bytes {return Err(invalid("байты main региона отличаются от точной конверсии снимка"));}
            Ok(())
        } else {output.write_all(bytes)}
    }
    for slot in 0..1024 {
        let bytes=match super::region::read_slot(source,slot)? {
            super::region::Stored::Missing=>continue,
            super::region::Stored::Damaged{why,..}=>return Err(invalid(why)),
            super::region::Stored::Found(bytes)=>bytes,
        };
        let chunk=super::Chunk::decode_for_main_import_with_catalog(&bytes,Catalog::new(Profile::Java26_1_2))
            .map_err(|e|invalid(format!("main slot{slot}: {e:?}")))?;
        let payload=chunk.to_bytes_with_catalog(Catalog::new(Profile::Java26_1_2))
            .map_err(|e|invalid(format!("main slot{slot} encode: {e:?}")))?;
        if payload.len()>64*1024*1024 {return Err(invalid("конвертированный чанк слишком велик"));}
        let body=super::region::compress(&payload)?;
        let length=body.len().checked_add(8).ok_or_else(||invalid("слишком большой чанк"))?;
        let length32=u32::try_from(length).map_err(invalid)?;
        let sector=u32::try_from(offset/UNIT as u64).map_err(invalid)?;
        header[8+slot*8..12+slot*8].copy_from_slice(&sector.to_be_bytes());
        header[12+slot*8..16+slot*8].copy_from_slice(&length32.to_be_bytes());
        let mut crc=flate2::Crc::new();crc.update(&body);
        let mut record=Vec::with_capacity(length.next_multiple_of(UNIT));
        record.extend_from_slice(&crc.sum().to_be_bytes());
        record.extend_from_slice(&u32::try_from(body.len()).map_err(invalid)?.to_be_bytes());
        record.extend_from_slice(&body);record.resize(length.next_multiple_of(UNIT),0);
        emit(output,offset,&record,verify)?;offset+=record.len() as u64;
    }
    emit(output,0,&header,verify)?;
    if verify {
        if output.metadata()?.len()!=offset {return Err(invalid("длина main региона отличается от точной конверсии снимка"));}
    } else {output.set_len(offset)?;output.sync_all()?;}
    Ok(())
}
fn convert_main_regions(source: &Path, candidate: &Path) -> io::Result<()> {
    for entry in fs::read_dir(source)? {
        let entry=entry?;let path=entry.path();let target=candidate.join(entry.file_name());
        if entry.file_type()?.is_dir() {convert_main_regions(&path,&target)?;}
        else if source.file_name().is_some_and(|n|n=="region") && path.extension().is_some_and(|n|n=="mcs") {
            main_region(&path,&mut File::create(target)?,false)?;
        }
    }
    sync(candidate)
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
#[cfg(test)]
fn verify_result(result: &Path, backup: &snapshot::VerifiedBackup) -> io::Result<()> {
    verify_result_for_source(result, backup, Source::Migration)
}
fn verify_result_for_source(result: &Path, backup: &snapshot::VerifiedBackup, source: Source) -> io::Result<()> {
    if !fs::symlink_metadata(result)?.is_dir() {return Err(invalid("результат перехода не обычный каталог"));}
    if profile::read_manifest(result).map_err(invalid)?!=Some(target_manifest(backup.snapshot_id())){return Err(invalid("неверный результат перехода"));}
    compare_transformed_tree(result,&backup.directory().join("world"),true,source)?;
    validate(result)?; Ok(())
}
/// Вызывается completion снимка: замки всех измерений удерживаются до commit.
pub(super) fn activate(guard: &startup::StartupGuard, backup: &snapshot::VerifiedBackup, uncertain: &mut bool) -> io::Result<()> {
    activate_for_source(guard, backup, uncertain, Source::Migration)
}
pub(super) fn activate_from_main(guard: &startup::StartupGuard, backup: &snapshot::VerifiedBackup, uncertain: &mut bool) -> io::Result<()> {
    activate_for_source(guard, backup, uncertain, Source::Main26_1_2)
}
fn activate_for_source(guard: &startup::StartupGuard, backup: &snapshot::VerifiedBackup, uncertain: &mut bool, kind: Source) -> io::Result<()> {
    let world=guard.world_path();let source=backup.directory().join("world");
    validate_source(&source, kind)?;
    let (candidate,previous)=paths(world,backup.snapshot_id());
    if candidate.exists() || previous.exists(){return Err(invalid("результат перехода уже существует"));}
    let mut journal=Journal{state:"prepared".into(),snapshot:backup.snapshot_id().into(),source:kind};
    *uncertain=true;write_journal(world,&journal)?;
    copy_tree(&source,&candidate)?;
    if kind == Source::Main26_1_2 { convert_main_regions(&source,&candidate)?; }
    if kind == Source::Main26_1_2 && candidate.join("playerdata").exists() {
        for entry in fs::read_dir(candidate.join("playerdata"))? {
            let path=entry?.path();
            let bytes=convert_main(&fs::read(&path)?)?;
            let mut file=File::create(&path)?; file.write_all(&bytes)?; file.sync_all()?;
        }
        sync(&candidate.join("playerdata"))?;
    }
    let bytes=serde_json::to_vec(&target_manifest(backup.snapshot_id())).map_err(invalid)?;
    let p=candidate.join(profile::MANIFEST_NAME);let mut file=File::create(p)?;file.write_all(&bytes)?;file.sync_all()?;sync(&candidate)?;
    verify_result_for_source(&candidate,backup,kind)?;
    journal.state="validated".into();write_journal(world,&journal)?;
    journal.state="activating".into();write_journal(world,&journal)?;
    recover(world)
}
pub(crate) fn upgrade(path: &Path) -> io::Result<()> { upgrade_source(path, Source::Migration) }
/// Явный импорт исторической main: номер MCPD10 здесь означает abilities,
/// а не cursor. Обычный upgrade никогда не угадывает эту раскладку.
pub(crate) fn upgrade_from_main_26_1_2(path: &Path) -> io::Result<()> { upgrade_source(path, Source::Main26_1_2) }
fn upgrade_source(path: &Path, source: Source) -> io::Result<()> {
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
    let backup=match source {
        Source::Migration=>snapshot::create_profile_upgrade(&guard),
        Source::Main26_1_2=>snapshot::create_main_profile_upgrade(&guard),
    }.map_err(invalid)?;
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
            let journal=Journal{state:"activating".into(),snapshot:m.snapshot_id,source:Source::Migration};write_journal(&f.world(),&journal).unwrap();
            let result=startup::prepare_for_version(&f.world(),mcsa_protocol::JavaVersion::V26_3);
            if stage==2 {assert!(result.is_err());assert!(!f.world().exists());}
            else {assert!(result.is_ok());assert_eq!(profile::read_manifest(&f.world()).unwrap().unwrap().gameplay_version,"26.3");}
        }
    }
    // Замороженная main26.1.2 раскладка10: хвост abilities, без cursor.
    fn main_player(f: &Fixture) -> (PathBuf, Vec<u8>) {
        // Историческая main пишет layout4; пустой хвост этой простой карты
        // одинаков, различается только явно замороженный номер раскладки.
        for dimension in ["", "dimensions/minecraft/the_nether", "dimensions/minecraft/the_end"] {
            let region=f.world().join(dimension).join("region/r.0.0.mcs");
            let mut slots=Vec::new();
            for slot in 0..1024 {
                if let super::super::region::Stored::Found(mut chunk)=super::super::region::read_slot(&region,slot).unwrap() {
                    chunk[0]=4;slots.push((slot,chunk));
                }
            }
            let borrowed=slots.iter().map(|(slot,bytes)|(*slot,bytes.as_slice())).collect::<Vec<_>>();
            super::super::region::write_slots(&region,&borrowed).unwrap();
        }
        let bytes=include_bytes!("../../testdata/playerdata/main10-abilities.mcrw").to_vec();
        let path=f.world().join("playerdata/00000000000000000000000000000001.mcrw");
        fs::create_dir(path.parent().unwrap()).unwrap();fs::write(&path,&bytes).unwrap();
        (path,bytes)
    }
    #[test]
    fn explicit_main_upgrade_preserves_snapshot_and_converts_only_candidate() {
        let f=Fixture::new();let (path,bytes)=main_player(&f);
        assert!(upgrade(&f.world()).is_err(), "обычный переход не угадывает main10");
        assert_eq!(fs::read(&path).unwrap(),bytes);
        assert!(profile::read_manifest(&f.world()).unwrap().is_none());
        upgrade_from_main_26_1_2(&f.world()).unwrap();
        let manifest=profile::read_manifest(&f.world()).unwrap().unwrap();
        let backup=snapshot::verify_backup(&f.0.join("backups/world").join(&manifest.snapshot_id)).unwrap();
        assert_eq!(fs::read(backup.directory().join("world/playerdata").join(path.file_name().unwrap())).unwrap(),bytes);
        let (_,previous)=paths(&f.world(),&manifest.snapshot_id);
        assert_eq!(fs::read(previous.join("playerdata").join(path.file_name().unwrap())).unwrap(),bytes);
        assert_eq!(fs::read(&path).unwrap(),convert_main(&bytes).unwrap());
        verify_result_for_source(&f.world(),&backup,Source::Main26_1_2).unwrap();
        assert!(verify_result(&f.world(),&backup).is_err());
        assert!(startup::prepare_for_version(&f.world(),mcsa_protocol::JavaVersion::V26_3).is_ok());
        assert!(upgrade_from_main_26_1_2(&f.world()).is_err());
    }
    #[test]
    fn main_import_recovery_checks_exact_conversion_at_all_rename_boundaries() {
        for stage in 0..6 {
            let f=Fixture::new();let (path,_)=main_player(&f);
            upgrade_from_main_26_1_2(&f.world()).unwrap();
            let manifest=profile::read_manifest(&f.world()).unwrap().unwrap();
            let (candidate,previous)=paths(&f.world(),&manifest.snapshot_id);
            if stage != 2 {fs::rename(f.world(),&candidate).unwrap();}
            if stage == 0 {fs::rename(&previous,f.world()).unwrap();}
            if stage == 3 {fs::write(candidate.join("playerdata").join(path.file_name().unwrap()),b"tampered").unwrap();}
            if stage == 4 {fs::write(previous.join("playerdata").join(path.file_name().unwrap()),b"tampered").unwrap();}
            if stage == 5 {fs::write(f.0.join("backups/world").join(&manifest.snapshot_id).join("world/playerdata").join(path.file_name().unwrap()),b"tampered").unwrap();}
            write_journal(&f.world(),&Journal{state:"activating".into(),snapshot:manifest.snapshot_id,source:Source::Main26_1_2}).unwrap();
            let result=startup::prepare_for_version(&f.world(),mcsa_protocol::JavaVersion::V26_3);
            assert_eq!(result.is_ok(),stage<3,"stage{stage}");
            if stage>=3 {assert!(!f.world().exists());}
        }
    }
    #[test]
    fn main_import_pre_activation_recovery_retains_original_bytes() {
        for state in ["prepared","validated"] {
            let f=Fixture::new();let (path,bytes)=main_player(&f);
            upgrade_from_main_26_1_2(&f.world()).unwrap();
            let manifest=profile::read_manifest(&f.world()).unwrap().unwrap();
            let (candidate,previous)=paths(&f.world(),&manifest.snapshot_id);
            fs::rename(f.world(),candidate).unwrap();fs::rename(previous,f.world()).unwrap();
            write_journal(&f.world(),&Journal{state:state.into(),snapshot:manifest.snapshot_id,source:Source::Main26_1_2}).unwrap();
            recover(&f.world()).unwrap();
            assert_eq!(fs::read(path).unwrap(),bytes);
            assert!(profile::read_manifest(&f.world()).unwrap().is_none());
            let journal:Journal=serde_json::from_slice(&fs::read(journal_path(&f.world())).unwrap()).unwrap();
            assert_eq!(journal.state,"aborted");assert!(journal.source==Source::Main26_1_2);
        }
    }
    #[test]
    fn journal_source_is_explicit_and_unknown_modes_fail_closed() {
        let old:Journal=serde_json::from_str(r#"{"state":"activating","snapshot":"01234567890123456789012345678901"}"#).unwrap();
        assert!(old.source==Source::Migration);
        let f=Fixture::new();let (path,bytes)=main_player(&f);
        fs::write(journal_path(&f.world()),r#"{"state":"activating","snapshot":"01234567890123456789012345678901","source":"guess"}"#).unwrap();
        assert!(recover(&f.world()).is_err());
        assert_eq!(fs::read(path).unwrap(),bytes);
    }
    #[test]
    fn invalid_main_playerdata_keeps_original_world_untouched() {
        let f=Fixture::new();let (path,mut bytes)=main_player(&f);bytes.pop();fs::write(&path,&bytes).unwrap();
        assert!(upgrade_from_main_26_1_2(&f.world()).is_err());
        assert_eq!(fs::read(path).unwrap(),bytes);
        assert!(profile::read_manifest(&f.world()).unwrap().is_none());
    }

    #[test]
    fn main_region_import_rewrites_metadata_and_preserves_verified_original() {
        use crate::{inventory::Stack, stackdata::Data};
        let f=Fixture::new();main_player(&f);
        let mut chunk=super::super::Chunk::flat();
        chunk.set(1,80,1,crate::blocks::state_by_name("chest").unwrap());
        let data=Data{plugin_meta:Some("{\"customModelData\":71}".into()),..Default::default()};
        let stack=Stack::new(crate::blocks::item_named("diamond").unwrap(),2).with_extras(data.clone());
        chunk.containers.get_mut(&(1,80,1)).unwrap().slots[0]=Some(stack);
        let mut legacy=chunk.to_bytes();legacy[0]=4;
        let current=crate::stackdata::to_bytes(&data);let main=crate::stackdata::to_bytes_main_metadata(&data);
        let offsets=legacy.windows(current.len()).enumerate().filter_map(|(i,w)|(w==current).then_some(i)).collect::<Vec<_>>();
        assert_eq!(offsets.len(),1);legacy[offsets[0]..offsets[0]+main.len()].copy_from_slice(&main);
        let region=f.world().join("region/r.0.0.mcs");
        super::super::region::write_slots(&region,&[(0,&legacy)]).unwrap();
        let source=fs::read(&region).unwrap();
        assert!(super::super::Chunk::decode_for_load_with_catalog(&legacy,Catalog::new(Profile::Java26_1_2)).is_err());
        upgrade_from_main_26_1_2(&f.world()).unwrap();
        let manifest=profile::read_manifest(&f.world()).unwrap().unwrap();
        let backup=snapshot::verify_backup(&f.0.join("backups/world").join(&manifest.snapshot_id)).unwrap();
        assert_eq!(fs::read(backup.directory().join("world/region/r.0.0.mcs")).unwrap(),source);
        let super::super::region::Stored::Found(rewritten)=super::super::region::read_slot(&region,0).unwrap() else {panic!("нет чанка")};
        assert_ne!(rewritten,legacy);
        let loaded=super::super::Chunk::decode_for_load_with_catalog(&rewritten,Catalog::new(Profile::Java26_1_2)).unwrap();
        assert_eq!(loaded.containers[&(1,80,1)].slots[0],Some(stack));
        verify_result_for_source(&f.world(),&backup,Source::Main26_1_2).unwrap();
        let canonical=fs::read(&region).unwrap();
        for offset in [5,8200,canonical.len()-1] {
            let mut tampered=canonical.clone();tampered[offset]^=1;fs::write(&region,tampered).unwrap();
            assert!(verify_result_for_source(&f.world(),&backup,Source::Main26_1_2).is_err(),"tamper{offset}");
            fs::write(&region,&canonical).unwrap();
        }
        super::super::region::write_slots(&region,&[(1,&rewritten)]).unwrap();
        assert!(verify_result_for_source(&f.world(),&backup,Source::Main26_1_2).is_err(),"лишний slot");
        fs::write(&region,&canonical).unwrap();
        let mut trailing=canonical;trailing.push(0);fs::write(&region,trailing).unwrap();
        assert!(verify_result_for_source(&f.world(),&backup,Source::Main26_1_2).is_err(),"лишний хвост");
    }

    #[test]
    fn main_plugin_shoulder_fixture_survives_full_world_import_and_restart() {
        let f=Fixture::new();let (path,_)=main_player(&f);
        let bytes: &[u8]=include_bytes!("../../testdata/playerdata/main10-plugin-shoulder.mcrw");
        fs::write(&path,bytes).unwrap();
        upgrade_from_main_26_1_2(&f.world()).unwrap();
        let manifest=profile::read_manifest(&f.world()).unwrap().unwrap();
        let backup=snapshot::verify_backup(&f.0.join("backups/world").join(&manifest.snapshot_id)).unwrap();
        assert_eq!(fs::read(backup.directory().join("world/playerdata").join(path.file_name().unwrap())).unwrap(),bytes);
        assert_eq!(fs::read(path).unwrap(),convert_main(bytes).unwrap());
        verify_result_for_source(&f.world(),&backup,Source::Main26_1_2).unwrap();
        assert!(startup::prepare_for_version(&f.world(),mcsa_protocol::JavaVersion::V26_3).is_ok());
    }

    #[test]
    fn frozen_main_hives_survive_world_import_verified_snapshot_and_restart() {
        use crate::stackdata::main_import::tests::{HIVE, assert_hive, assert_stack, catalog};
        let f=Fixture::new();main_player(&f);
        let source_chunk=super::super::codec::main_hive_import_tests::source_chunk();
        let region=f.world().join("region/r.0.0.mcs");
        super::super::region::write_slots(&region,&[(0,&source_chunk)]).unwrap();
        let source=fs::read(&region).unwrap();
        upgrade_from_main_26_1_2(&f.world()).unwrap();
        let manifest=profile::read_manifest(&f.world()).unwrap().unwrap();
        let backup=snapshot::verify_backup(&f.0.join("backups/world").join(&manifest.snapshot_id)).unwrap();
        assert_eq!(fs::read(backup.directory().join("world/region/r.0.0.mcs")).unwrap(),source);
        let (_,previous)=paths(&f.world(),&manifest.snapshot_id);
        assert_eq!(fs::read(previous.join("region/r.0.0.mcs")).unwrap(),source);
        let super::super::region::Stored::Found(bytes)=super::super::region::read_slot(&region,0).unwrap() else {panic!("нет чанка")};
        let chunk=super::super::Chunk::decode_for_load_with_catalog(&bytes,catalog()).unwrap();
        assert_hive(&chunk.hives[&(1,80,1)]);assert_stack(chunk.containers[&(2,80,1)].slots[0].unwrap());
        assert_ne!(chunk.hives[&(1,80,1)].bytes(),HIVE);
        verify_result_for_source(&f.world(),&backup,Source::Main26_1_2).unwrap();
        assert!(startup::prepare_for_version(&f.world(),mcsa_protocol::JavaVersion::V26_3).is_ok());
    }

    #[test]
    fn corrupt_main_plugin_writer_and_hive_keep_source_and_verified_backup() {
        for corruption in 0..3 {
            let f=Fixture::new();let (player,_)=main_player(&f);
            if corruption != 0 {
                let mut chunk=super::super::codec::main_hive_import_tests::source_chunk();
                let hive=crate::stackdata::main_import::tests::HIVE;
                if corruption == 1 { let at=chunk.windows(hive.len()).position(|w|w==hive).unwrap();chunk[at]=b'!'; }
                else { chunk.push(0x71); }
                super::super::region::write_slots(&f.world().join("region/r.0.0.mcs"),&[(0,&chunk)]).unwrap();
            } else { fs::write(&player,include_bytes!("../../testdata/playerdata/main10-plugin-only-writer-bug.mcrw")).unwrap(); }
            let error=upgrade_from_main_26_1_2(&f.world()).unwrap_err();assert_eq!(error.kind(),io::ErrorKind::InvalidData);
            assert!(profile::read_manifest(&f.world()).unwrap().is_none());
            let backups=fs::read_dir(f.0.join("backups/world")).unwrap().collect::<io::Result<Vec<_>>>().unwrap();
            assert_eq!(backups.len(),1);
            let backup=snapshot::verify_backup(&backups[0].path()).unwrap();
            compare_tree(&f.world(),&backup.directory().join("world"),false).unwrap();
            assert!(!journal_path(&f.world()).exists());
        }
    }

    #[test]
    fn stale_journal_temps_do_not_block_prepared_or_activating_recovery() {
        for state in ["prepared","activating"] {
            for partial in [false,true] {
                let f=Fixture::new();let (player,source)=main_player(&f);
                upgrade_from_main_26_1_2(&f.world()).unwrap();
                let manifest=profile::read_manifest(&f.world()).unwrap().unwrap();
                let backup=snapshot::verify_backup(&f.0.join("backups/world").join(&manifest.snapshot_id)).unwrap();
                let (candidate,previous)=paths(&f.world(),&manifest.snapshot_id);
                fs::rename(f.world(),&candidate).unwrap();fs::rename(previous,f.world()).unwrap();
                write_journal(&f.world(),&Journal{state:state.into(),snapshot:manifest.snapshot_id.clone(),source:Source::Main26_1_2}).unwrap();
                let stale=if partial {b"{\"state\":".to_vec()} else {
                    serde_json::to_vec(&Journal{state:state.into(),snapshot:manifest.snapshot_id.clone(),source:Source::Main26_1_2}).unwrap()
                };
                let temp=journal_path(&f.world()).with_extension("json.new");fs::write(&temp,&stale).unwrap();
                // Также сохраняем незавершённый temp новой схемы от прошлого процесса.
                let unique=journal_path(&f.world()).with_extension("json.new-00000000000000000000000000000000");fs::write(&unique,&stale).unwrap();
                recover(&f.world()).unwrap();
                assert_eq!(fs::read(&temp).unwrap(),stale);assert_eq!(fs::read(&unique).unwrap(),stale);
                snapshot::verify_backup(backup.directory()).unwrap();
                if state=="prepared" {
                    assert_eq!(fs::read(player).unwrap(),source);
                    assert!(profile::read_manifest(&f.world()).unwrap().is_none());
                    compare_tree(&f.world(),&backup.directory().join("world"),false).unwrap();
                } else { verify_result_for_source(&f.world(),&backup,Source::Main26_1_2).unwrap(); }
                let journal:Journal=serde_json::from_slice(&fs::read(journal_path(&f.world())).unwrap()).unwrap();
                assert_eq!(journal.state,if state=="prepared" {"aborted"} else {"committed"});
            }
        }
    }

    #[test]
    fn stale_journal_temp_does_not_bypass_source_or_snapshot_checks() {
        for tamper_snapshot in [false,true] {
            let f=Fixture::new();let (player,_)=main_player(&f);
            upgrade_from_main_26_1_2(&f.world()).unwrap();
            let manifest=profile::read_manifest(&f.world()).unwrap().unwrap();
            let (candidate,previous)=paths(&f.world(),&manifest.snapshot_id);
            fs::rename(f.world(),&candidate).unwrap();fs::rename(previous,f.world()).unwrap();
            write_journal(&f.world(),&Journal{state:"activating".into(),snapshot:manifest.snapshot_id.clone(),source:Source::Main26_1_2}).unwrap();
            let temp=journal_path(&f.world()).with_extension("json.new");fs::write(&temp,b"{").unwrap();
            let target=if tamper_snapshot {
                f.0.join("backups/world").join(&manifest.snapshot_id).join("world/playerdata").join(player.file_name().unwrap())
            } else {player.clone()};
            fs::write(&target,b"tampered").unwrap();
            assert!(recover(&f.world()).is_err());
            assert_eq!(fs::read(&target).unwrap(),b"tampered");assert!(candidate.is_dir());
            assert_eq!(fs::read(temp).unwrap(),b"{");
            let journal:Journal=serde_json::from_slice(&fs::read(journal_path(&f.world())).unwrap()).unwrap();assert_eq!(journal.state,"activating");
        }
    }

    #[test]
    fn main_import_target_loads_canonical_player_uuid_after_restart() {
        let f=Fixture::new();let (path,_)=main_player(&f);
        let source=include_bytes!("../../testdata/playerdata/main10-plugin-shoulder.mcrw");
        fs::write(&path,source).unwrap();
        upgrade_from_main_26_1_2(&f.world()).unwrap();
        let manifest=profile::read_manifest(&f.world()).unwrap().unwrap();
        let backup=snapshot::verify_backup(&f.0.join("backups/world").join(&manifest.snapshot_id)).unwrap();
        assert_eq!(fs::read(backup.directory().join("world/playerdata").join(path.file_name().unwrap())).unwrap(),source);
        let _guard=startup::prepare_for_version(&f.world(),mcsa_protocol::JavaVersion::V26_3).unwrap();
        let mut uuid=[0;16];uuid[15]=1;
        let loaded=crate::playerdata::load_checked_for_catalog(&f.world().join("playerdata"),&uuid,Catalog::new(Profile::Preview26_3)).unwrap().expect("UUID runtime обязан найти игрока");
        assert_eq!(crate::playerdata::path_for(&f.world().join("playerdata"),&uuid),path);
        let stone=loaded.inventory.slot(36).unwrap();
        assert_eq!((stone.item,stone.count),(1,17));
        assert_eq!(stone.extras().name.as_deref(),Some("Frozen stone"));
        assert_eq!(stone.extras().plugin_meta.as_deref(),Some(r#"{"pdc":{"fixture":"main"}}"#));
        assert_eq!(loaded.inventory.slot(37),Some(crate::inventory::Stack::new(10,5)));
        assert_eq!(loaded.ender[4],Some(crate::inventory::Stack::new(1,7)));
        assert_eq!(loaded.vitals.experience_total,42);
        assert!(loaded.shoulders[0].is_some());
    }

    #[test]
    fn noncanonical_player_filenames_refuse_import_without_hiding_or_replacing_data() {
        for name in ["00000000-0000-0000-0000-000000000001.mcrw", "0000000000000000000000000000000A.mcrw", "00000000000000000000000000000001.mcrw.mcrw"] {
            for keep_canonical in [false,true] {
                let f=Fixture::new();let (canonical,bytes)=main_player(&f);
                let foreign=canonical.with_file_name(name);
                fs::write(&foreign,&bytes).unwrap();
                if !keep_canonical { fs::remove_file(&canonical).unwrap(); }
                let error=upgrade_from_main_26_1_2(&f.world()).unwrap_err();assert!(error.to_string().contains("неканоническое имя playerdata"));
                assert_eq!(fs::read(&foreign).unwrap(),bytes);
                if keep_canonical {assert_eq!(fs::read(&canonical).unwrap(),bytes);} else {assert!(!canonical.exists());}
                assert!(profile::read_manifest(&f.world()).unwrap().is_none());assert!(!journal_path(&f.world()).exists());
                let backups=fs::read_dir(f.0.join("backups/world")).unwrap().collect::<io::Result<Vec<_>>>().unwrap();assert_eq!(backups.len(),1);
                let backup=snapshot::verify_backup(&backups[0].path()).unwrap();
                compare_tree(&f.world(),&backup.directory().join("world"),false).unwrap();
            }
        }
    }

}
