use super::*;
use std::ffi::{CStr, CString, OsStr};
use std::fs::{File, Metadata, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, IntoRawFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};

use aws_lc_rs::{digest, rand};
use serde::{Deserialize, Serialize};

const MAX_ENTRIES: usize = 1_000_000;
const MAX_MEMORY: usize = 64 * 1024 * 1024;
const MAX_PATH: usize = 4095;
const MAX_DEPTH: usize = 128;
const MAX_LINE: usize = 16 * 1024;
const BUFFER: usize = 256 * 1024;

fn invalid(message: impl Into<String>) -> io::Error { io::Error::new(io::ErrorKind::InvalidData, message.into()) }
fn changed() -> io::Error { invalid("источник/папка копии изменились во время проверки; снимок не подтверждён") }
fn name_error(name: &[u8], reason: &str) -> io::Error { invalid(format!("{}: {reason}", OsStr::from_bytes(name).to_string_lossy())) }
fn hex(bytes: &[u8]) -> String { bytes.iter().map(|v| format!("{v:02x}")).collect() }
fn component(bytes: &[u8]) -> io::Result<CString> {
    if bytes.is_empty() || bytes == b"." || bytes == b".." || bytes.contains(&b'/') {
        return Err(invalid("недопустимый компонент относительного пути"));
    }
    CString::new(bytes).map_err(|_| invalid("NUL в имени файла"))
}

// Linux syscall ABI из системного linux/openat2.h, без заимствования кода игры.
#[repr(C)]
struct OpenHow { flags: u64, mode: u64, resolve: u64 }
fn open_at(parent: &File, path: &[u8], flags: i32, mode: u32) -> io::Result<File> {
    let path = CString::new(path).map_err(|_| invalid("NUL в пути"))?;
    let how = OpenHow {
        flags: (flags | libc::O_CLOEXEC | libc::O_NOFOLLOW | if flags & libc::O_PATH == 0 { libc::O_NONBLOCK | libc::O_NOATIME } else { 0 }) as u64,
        mode: u64::from(mode),
        resolve: 0x01 | 0x04 | 0x08, // NO_XDEV | NO_SYMLINKS | BENEATH
    };
    // SAFETY: все указатели живы, размер OpenHow соответствует syscall ABI.
    let fd = unsafe { libc::syscall(libc::SYS_openat2, parent.as_raw_fd(), path.as_ptr(), std::ptr::from_ref(&how), std::mem::size_of::<OpenHow>()) };
    if fd < 0 {
        let error = io::Error::last_os_error();
        if error.raw_os_error() == Some(libc::ENOSYS) {
            return Err(io::Error::new(io::ErrorKind::Unsupported, "для снимка нужен Linux с openat2; небезопасный fallback запрещён"));
        }
        return Err(error);
    }
    // SAFETY: syscall вернул новый принадлежащий нам descriptor.
    Ok(unsafe { File::from_raw_fd(fd as i32) })
}
fn open_directory(path: &Path) -> io::Result<File> {
    // Даже путь родителя раскрывается компонентами, без следования ссылкам.
    if !path.is_absolute() { return Err(invalid("нужен абсолютный путь")); }
    let mut current = OpenOptions::new().read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC).open("/")?;
    for part in path.components() {
        match part {
            std::path::Component::RootDir => {},
            std::path::Component::Normal(name) => {
                let name = component(name.as_bytes())?;
                // По пути К миру допускается смена filesystem; ВНУТРИ него — нет.
                // SAFETY: current и name живы; новый descriptor проверяется ниже.
                let fd = unsafe { libc::openat(current.as_raw_fd(), name.as_ptr(),
                    libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC) };
                if fd < 0 { return Err(io::Error::last_os_error()); }
                // SAFETY: новый fd принадлежит этому File.
                current = unsafe { File::from_raw_fd(fd) };
            }
            _ => return Err(invalid("путь содержит . или ..")),
        }
    }
    Ok(current)
}
fn checked_node(parent: &File, path: &[u8], file_only: bool) -> io::Result<File> {
    // O_PATH не читает устройство/FIFO. Известный special отказывается ещё до
    // data descriptor; повторная metadata сверка ловит подмену при открытии.
    let probe = open_at(parent, path, libc::O_PATH, 0)?;
    let before = probe.metadata()?;
    if !before.is_file() && (!before.is_dir() || file_only) {
        return Err(name_error(path, "нужен обычный файл/каталог, не ссылка или special"));
    }
    let file = open_at(parent, path, libc::O_RDONLY | if before.is_dir() { libc::O_DIRECTORY } else { 0 }, 0)?;
    if Stamp::of(&file.metadata()?) != Stamp::of(&before) { return Err(changed()); }
    Ok(file)
}
fn regular(parent: &File, path: &[u8]) -> io::Result<File> { checked_node(parent, path, true) }
fn directory(parent: &File, path: &[u8]) -> io::Result<File> { open_at(parent, path, libc::O_RDONLY | libc::O_DIRECTORY, 0) }
fn new_file(parent: &File, path: &[u8]) -> io::Result<File> { open_at(parent, path, libc::O_RDWR | libc::O_CREAT | libc::O_EXCL, 0o600) }
fn mkdir(parent: &File, name: &[u8]) -> io::Result<File> {
    let c = component(name)?;
    // SAFETY: parent и c живы, папка создаётся только под удерживаемым fd.
    if unsafe { libc::mkdirat(parent.as_raw_fd(), c.as_ptr(), 0o700) } != 0 { return Err(io::Error::last_os_error()); }
    directory(parent, name)
}
fn existing_or_mkdir(parent: &File, name: &[u8]) -> io::Result<File> {
    match directory(parent, name) {
        Ok(file) => Ok(file),
        Err(e) if e.kind() == io::ErrorKind::NotFound => { let file = mkdir(parent, name)?; parent.sync_all()?; Ok(file) },
        Err(e) => Err(e),
    }
}
fn same_inode(left: &Metadata, right: &Metadata) -> bool { left.dev() == right.dev() && left.ino() == right.ino() }
fn check_directory_name(parent: &File, name: &[u8], held: &File) -> io::Result<()> {
    if !same_inode(&directory(parent, name)?.metadata()?, &held.metadata()?) { return Err(changed()); }
    Ok(())
}
fn check_directory_path(path: &Path, held: &File) -> io::Result<()> {
    if !same_inode(&open_directory(path)?.metadata()?, &held.metadata()?) { return Err(changed()); }
    Ok(())
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Stamp {
    dev: u64, ino: u64, size: u64, mode: u32, uid: u32, gid: u32, links: u64,
    mtime: i64, mtime_ns: i64, ctime: i64, ctime_ns: i64,
}
impl Stamp {
    fn of(m: &Metadata) -> Self { Self { dev:m.dev(), ino:m.ino(), size:m.len(), mode:m.mode(), uid:m.uid(), gid:m.gid(), links:m.nlink(),
        mtime:m.mtime(), mtime_ns:m.mtime_nsec(), ctime:m.ctime(), ctime_ns:m.ctime_nsec() } }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum Kind { Directory, File }
#[derive(Clone, Debug, PartialEq, Eq)]
struct Entry { path: Vec<u8>, kind: Kind, stamp: Stamp, atime: i64, atime_ns: i64 }
#[derive(Debug, PartialEq, Eq)]
struct Inventory { entries: Vec<Entry>, files: u64, bytes: u64, memory: usize }
struct Budget { used: usize, maximum: usize }
impl Budget {
    fn reserve(&mut self, bytes: usize) -> io::Result<()> {
        self.used = self.used.checked_add(bytes).filter(|n| *n <= self.maximum)
            .ok_or_else(|| invalid("перечень мира превышает лимит памяти"))?;
        Ok(())
    }
}
fn metadata_supported(file: &File, path: &[u8]) -> io::Result<Metadata> {
    let m = file.metadata()?;
    if !m.is_file() && !m.is_dir() { return Err(name_error(path, "ссылка или необычный файл не поддерживается")); }
    if m.is_file() && m.nlink() != 1 { return Err(name_error(path, "hardlink не поддерживается")); }
    // ACL/xattrs/setid/чужое владение не копируются молча: этот узкий primitive
    // сохраняет bytes, mode0777, atime/mtime и совпадающие uid/gid.
    // SAFETY: geteuid/getegid не принимают указателей.
    if m.mode() & 0o7000 != 0 || m.uid() != unsafe { libc::geteuid() } || m.gid() != unsafe { libc::getegid() } {
        return Err(name_error(path, "особые permissions или другое владение пока не поддержаны"));
    }
    // SAFETY: запрос только размера списка xattrs, буфер отсутствует, size=0.
    let count = unsafe { libc::flistxattr(file.as_raw_fd(), std::ptr::null_mut(), 0) };
    if count < 0 { return Err(io::Error::last_os_error()); }
    if count != 0 { return Err(name_error(path, "xattrs/ACL пока не поддержаны")); }
    Ok(m)
}

struct Dir(*mut libc::DIR);
impl Drop for Dir {
    fn drop(&mut self) {
        // SAFETY: fdopendir передал этому объекту владение DIR.
        unsafe { libc::closedir(self.0); }
    }
}
fn names(parent: &File, budget: &mut Budget) -> io::Result<Vec<Vec<u8>>> {
    // Новый open, не dup: readdir не должен разделять offset с владельцем fd.
    let file = directory(parent, b".")?;
    let fd = file.into_raw_fd();
    // SAFETY: fd открыт как каталог и передаётся DIR при успехе.
    let ptr = unsafe { libc::fdopendir(fd) };
    if ptr.is_null() {
        let error = io::Error::last_os_error();
        // SAFETY: fdopendir не принял fd при ошибке.
        unsafe { libc::close(fd); }
        return Err(error);
    }
    let dir = Dir(ptr);
    let mut result = Vec::new();
    loop {
        // SAFETY: errno принадлежит текущему потоку; DIR действителен.
        unsafe { *libc::__errno_location() = 0; }
        let item = unsafe { libc::readdir(dir.0) };
        if item.is_null() {
            let error = io::Error::last_os_error();
            if error.raw_os_error() != Some(0) { return Err(error); }
            break;
        }
        // SAFETY: readdir вернул живую запись с завершающим NUL до следующего вызова.
        let name = unsafe { CStr::from_ptr((*item).d_name.as_ptr()) }.to_bytes();
        if name == b"." || name == b".." { continue; }
        component(name)?;
        budget.reserve(name.len().checked_add(128).ok_or_else(|| invalid("слишком длинное имя"))?)?;
        result.push(name.to_vec());
    }
    result.sort();
    Ok(result)
}
fn inventory(root: &File, maximum: usize) -> io::Result<Inventory> {
    let mut out = Inventory { entries:Vec::new(), files:0, bytes:0, memory:0 };
    let mut budget = Budget { used:0, maximum };
    walk(root, Vec::new(), 0, &mut budget, &mut |entry, _, budget| {
        if out.entries.len() >= MAX_ENTRIES { return Err(invalid("слишком много записей мира")); }
        budget.reserve(512usize.checked_add(entry.path.len().checked_mul(2).ok_or_else(|| invalid("размер пути"))?)
            .ok_or_else(|| invalid("размер перечня"))?)?;
        if entry.kind == Kind::File {
            out.files = out.files.checked_add(1).ok_or_else(|| invalid("слишком много файлов"))?;
            out.bytes = out.bytes.checked_add(entry.stamp.size).ok_or_else(|| invalid("размер мира не представим"))?;
        }
        out.entries.push(entry);
        Ok(())
    })?;
    out.memory = budget.used;
    Ok(out)
}
fn walk(file: &File, path: Vec<u8>, depth: usize, budget: &mut Budget,
    visit: &mut impl FnMut(Entry, &File, &mut Budget) -> io::Result<()>) -> io::Result<()> {
    if depth > MAX_DEPTH || path.len() > MAX_PATH {
        return Err(name_error(&path, "превышены пределы глубины/пути"));
    }
    let m = metadata_supported(file, &path)?;
    let kind = if m.is_dir() { Kind::Directory } else { Kind::File };
    visit(Entry { path:path.clone(), kind, stamp:Stamp::of(&m), atime:m.atime(), atime_ns:m.atime_nsec() }, file, budget)?;
    if kind == Kind::Directory {
        for name in names(file, budget)? {
            budget.used -= name.len() + 128;
            let mut child = path.clone();
            if !child.is_empty() { child.push(b'/'); }
            child.extend(&name);
            let entry = checked_node(file, &name, false)?;
            walk(&entry, child, depth + 1, budget, visit)?;
        }
    }
    if Stamp::of(&file.metadata()?) != Stamp::of(&m) { return Err(changed()); }
    Ok(())
}
fn check_inventory(root: &File, expected: &Inventory, maximum: usize) -> io::Result<()> {
    let mut at=0usize;
    let mut budget=Budget { used:expected.memory,maximum };
    walk(root,Vec::new(),0,&mut budget,&mut |entry,_,_| {
        if expected.entries.get(at)!=Some(&entry) { return Err(changed()); }
        at+=1;Ok(())
    })?;
    if at!=expected.entries.len() { return Err(changed()); }
    Ok(())
}
fn fingerprint(hash:&mut digest::Context,entry:&Entry) {
    hash.update(&(entry.path.len() as u64).to_be_bytes());hash.update(&entry.path);
    for n in [entry.stamp.dev,entry.stamp.ino,entry.stamp.size,entry.stamp.links] { hash.update(&n.to_be_bytes()); }
    for n in [entry.stamp.mode,entry.stamp.uid,entry.stamp.gid] { hash.update(&n.to_be_bytes()); }
    for n in [entry.stamp.mtime,entry.stamp.mtime_ns,entry.stamp.ctime,entry.stamp.ctime_ns,entry.atime,entry.atime_ns] { hash.update(&n.to_be_bytes()); }
}
fn tree_fingerprint(root:&File,held_memory:usize)->io::Result<String> {
    let mut hash=digest::Context::new(&digest::SHA256);
    let mut budget=Budget {used:held_memory,maximum:MAX_MEMORY};let mut count=0usize;
    walk(root,Vec::new(),0,&mut budget,&mut |entry,_,_| {
        count+=1;if count>MAX_ENTRIES {return Err(invalid("слишком много записей копии"));}
        fingerprint(&mut hash,&entry);Ok(())
    })?;
    Ok(hex(hash.finish().as_ref()))
}

struct LockedWorld<'a> { guard: &'a StartupGuard, parent: File, root: File, name: Vec<u8>, locks: Vec<(Vec<u8>, Option<File>)> }
impl<'a> LockedWorld<'a> {
    fn acquire(guard: &'a StartupGuard) -> io::Result<Self> {
        if !guard.has_exclusive_lock() { return Err(invalid("исключительный StartupGuard обязателен")); }
        let world = guard.world_path();
        let name = world.file_name().ok_or_else(|| invalid("нет имени мира"))?.as_bytes().to_vec();
        if name == b"backups" { return Err(invalid("мир с именем backups пересекается с местом резервных копий")); }
        let parent = open_directory(world.parent().ok_or_else(|| invalid("нет родителя мира"))?)?;
        let root = directory(&parent, &name)?;
        let identity = root.metadata()?;
        if guard.world_identity() != Some((identity.dev(), identity.ino())) { return Err(changed()); }
        let mut locks = Vec::new();
        for path in [b"session.lock".as_slice(), b"dimensions/minecraft/the_nether/session.lock", b"dimensions/minecraft/the_end/session.lock"] {
            let dir = path.rsplitn(2, |b| *b == b'/').nth(1);
            if let Some(parent_path) = dir {
                match directory(&root, parent_path) {
                    Ok(_) => {},
                    Err(e) if e.kind() == io::ErrorKind::NotFound => { locks.push((path.to_vec(), None)); continue; },
                    Err(e) => return Err(e),
                }
            }
            let lock = regular(&root, path).map_err(|e| io::Error::new(e.kind(), format!("нужен существующий session.lock {}: {e}", OsStr::from_bytes(path).to_string_lossy())))?;
            metadata_supported(&lock, path)?;
            // SAFETY: lock держится до конца LockedWorld; без create/truncate/PID write.
            if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 { return Err(io::Error::last_os_error()); }
            locks.push((path.to_vec(), Some(lock)));
        }
        let source = Self { guard, parent, root, name, locks };
        source.check_anchor()?;
        Ok(source)
    }
    fn check_anchor(&self) -> io::Result<()> {
        let current_parent = open_directory(self.guard.world_path().parent().ok_or_else(changed)?)?;
        if !same_inode(&current_parent.metadata()?, &self.parent.metadata()?) { return Err(changed()); }
        if !same_inode(&directory(&self.parent, &self.name)?.metadata()?, &self.root.metadata()?) { return Err(changed()); }
        let mut lock_name = b".mcsa-".to_vec(); lock_name.extend(&self.name); lock_name.extend(b".lock");
        let current = regular(&self.parent, &lock_name)?;
        let held = self.guard.snapshot_lock().ok_or_else(changed)?;
        if !same_inode(&current.metadata()?, &held.metadata()?) { return Err(changed()); }
        // Удержание старого inode не защищает новый session.lock по тому же имени.
        // Не появившиеся при acquire измерения также должны оставаться отсутствующими.
        for (path, held) in &self.locks {
            if let Some(held) = held {
                if !same_inode(&regular(&self.root, path)?.metadata()?, &held.metadata()?) { return Err(changed()); }
            } else {
                let parent = path.rsplitn(2, |b| *b == b'/').nth(1).ok_or_else(changed)?;
                match directory(&self.root, parent) {
                    Err(e) if e.kind() == io::ErrorKind::NotFound => {},
                    Err(e) => return Err(e),
                    Ok(_) => return Err(changed()),
                }
            }
        }
        Ok(())
    }
}
fn known_dimensions(files: &Inventory) -> io::Result<()> {
    for e in &files.entries {
        if e.kind != Kind::Directory { continue; }
        let parts: Vec<&[u8]> = e.path.split(|b| *b == b'/').collect();
        if parts.first() == Some(&b"dimensions".as_slice()) {
            if parts.len() == 2 && parts[1] != b"minecraft" {
                return Err(name_error(&e.path, "неизвестное измерение: схема session locks не поддержана"));
            }
            if parts.len() == 3 && parts[2] != b"the_nether" && parts[2] != b"the_end" {
                return Err(name_error(&e.path, "неизвестное измерение: схема session locks не поддержана"));
            }
        }
    }
    Ok(())
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Record {
    /// Каждый компонент — hex точных Unix bytes, без потери не-UTF8 имён.
    path_hex: Vec<String>, kind: Kind, size: u64, sha256: Option<String>,
    mode: u32, uid: u32, gid: u32, atime: i64, atime_ns: i64, mtime: i64, mtime_ns: i64,
}
impl Record {
    fn new(entry: &Entry, hash: Option<String>) -> Self {
        Self { path_hex: if entry.path.is_empty() { Vec::new() } else { entry.path.split(|b| *b == b'/').map(hex).collect() },
            kind:entry.kind, size:if entry.kind == Kind::File { entry.stamp.size } else { 0 }, sha256:hash,
            mode:entry.stamp.mode & 0o777, uid:entry.stamp.uid, gid:entry.stamp.gid,
            atime:entry.atime, atime_ns:entry.atime_ns, mtime:entry.stamp.mtime, mtime_ns:entry.stamp.mtime_ns }
    }
    fn path(&self) -> io::Result<Vec<u8>> {
        let mut path = Vec::new();
        for word in &self.path_hex {
            if word.is_empty() || word.len() % 2 != 0 || word.len() > 510 { return Err(invalid("неверный hex компонента пути")); }
            let mut bytes = Vec::with_capacity(word.len() / 2);
            for pair in word.as_bytes().as_chunks::<2>().0 {
                let digit = |v:u8| match v { b'0'..=b'9' => Some(v-b'0'), b'a'..=b'f' => Some(v-b'a'+10), _=>None };
                bytes.push(digit(pair[0]).ok_or_else(|| invalid("неверный hex"))? * 16 + digit(pair[1]).ok_or_else(|| invalid("неверный hex"))?);
            }
            component(&bytes)?;
            if !path.is_empty() { path.push(b'/'); }
            path.extend(bytes);
            if path.len() > MAX_PATH { return Err(invalid("слишком длинный manifest path")); }
        }
        Ok(path)
    }
    fn matches(&self, entry: &Entry) -> io::Result<()> {
        if self.path()? != entry.path || self.kind != entry.kind
            || self.size != if entry.kind == Kind::File { entry.stamp.size } else { 0 }
            || self.mode != entry.stamp.mode & 0o777 || self.uid != entry.stamp.uid || self.gid != entry.stamp.gid
            || self.mtime != entry.stamp.mtime || self.mtime_ns != entry.stamp.mtime_ns {
            return Err(invalid("перечень/метаданные копии не совпадают с files.jsonl"));
        }
        match (&self.sha256, self.kind) {
            (Some(hash), Kind::File) if digest_hex(hash) => Ok(()),
            (None, Kind::Directory) => Ok(()),
            _ => Err(invalid("неверная запись hash/kind")),
        }
    }
}
fn digest_hex(s: &str) -> bool { s.len() == 64 && s.bytes().all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v)) }
fn id_valid(s: &str) -> bool { s.len() == 32 && s.bytes().all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v)) }
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Marker {
    schema: u32, scope: String, status: String, snapshot_id: String,
    source_profile: Option<super::super::profile::Manifest>,
    entries_file: String, entries_sha256: String, entries: u64, files: u64, total_bytes: u64,
    metadata_policy: String,
}
const POLICY: &str = "bytes+mode0777+uidgid+mtime;atime-archival;no-xattrs-acl-hardlinks-special-mounts";

fn open_entry(root: &File, entry: &Entry) -> io::Result<File> {
    let file = if entry.path.is_empty() { directory(root, b".")? }
        else { checked_node(root, &entry.path, entry.kind == Kind::File)? };
    if Stamp::of(&metadata_supported(&file, &entry.path)?) != entry.stamp { return Err(changed()); }
    Ok(file)
}
fn destination_parent(root: &File, path: &[u8]) -> io::Result<(File, Vec<u8>)> {
    if let Some(at) = path.iter().rposition(|b| *b == b'/') {
        Ok((directory(root, &path[..at])?, path[at+1..].to_vec()))
    } else { Ok((directory(root, b".")?, path.to_vec())) }
}
fn restore_metadata(file: &File, entry: &Entry) -> io::Result<()> {
    let metadata = file.metadata()?;
    if metadata.uid() != entry.stamp.uid || metadata.gid() != entry.stamp.gid {
        return Err(invalid("назначение имеет другое владение; без chown не обещаем сохранить его"));
    }
    // SAFETY: fd жив, mode содержит только обычные permission bits.
    if unsafe { libc::fchmod(file.as_raw_fd(), entry.stamp.mode & 0o777) } != 0 { return Err(io::Error::last_os_error()); }
    let times = [libc::timespec { tv_sec:entry.atime, tv_nsec:entry.atime_ns },
        libc::timespec { tv_sec:entry.stamp.mtime, tv_nsec:entry.stamp.mtime_ns }];
    // SAFETY: передан массив из двух timespec, file жив.
    if unsafe { libc::futimens(file.as_raw_fd(), times.as_ptr()) } != 0 { return Err(io::Error::last_os_error()); }
    file.sync_all()
}
fn hash_file(mut file: &File, expected: u64, buffer: &mut [u8]) -> io::Result<String> {
    let mut total = 0u64;
    let mut hash = digest::Context::new(&digest::SHA256);
    loop {
        let count = file.read(buffer)?;
        if count == 0 { break; }
        total = total.checked_add(count as u64).filter(|n| *n <= expected).ok_or_else(changed)?;
        hash.update(&buffer[..count]);
    }
    if total != expected { return Err(changed()); }
    Ok(hex(hash.finish().as_ref()))
}
fn space_required(bytes:u128, entries:u128, block:u128)->io::Result<u128> {
    let per_entry=block.max(4096).checked_add(MAX_LINE as u128).ok_or_else(||invalid("оценка размера переполнена"))?;
    let overhead=entries.checked_mul(per_entry).ok_or_else(||invalid("оценка размера переполнена"))?;
    let data=bytes.checked_add(overhead).ok_or_else(||invalid("оценка размера переполнена"))?;
    data.checked_add((64*1024*1024u128).max(data/20)).ok_or_else(||invalid("оценка размера переполнена"))
}
fn available_space(parent: &File, files: &Inventory) -> io::Result<()> {
    let mut v = std::mem::MaybeUninit::<libc::statvfs>::uninit();
    // SAFETY: fd жив, syscall заполняет valid statvfs при успехе.
    if unsafe { libc::fstatvfs(parent.as_raw_fd(), v.as_mut_ptr()) } != 0 { return Err(io::Error::last_os_error()); }
    let v = unsafe { v.assume_init() };
    let available = u128::from(v.f_bavail) * u128::from(v.f_frsize);
    // Logical bytes, округление каждого файла/каталога до блока + JSON reserve.
    let needed=space_required(u128::from(files.bytes),files.entries.len() as u128,u128::from(v.f_frsize))?;
    if available < needed { return Err(io::Error::new(io::ErrorKind::StorageFull, "недостаточно свободного места для полной копии с резервом")); }
    Ok(())
}

fn bounded_line(reader: &mut impl BufRead) -> io::Result<Vec<u8>> {
    let mut line = Vec::new();
    loop {
        let buffer = reader.fill_buf()?;
        if buffer.is_empty() { return Ok(line); }
        let count = buffer.iter().position(|b| *b == b'\n').map_or(buffer.len(), |n| n+1);
        if line.len().checked_add(count).is_none_or(|n| n > MAX_LINE) { return Err(invalid("слишком длинная строка files.jsonl")); }
        let done = buffer[count-1] == b'\n';
        line.extend(&buffer[..count]);
        reader.consume(count);
        if done { return Ok(line); }
    }
}
fn read_marker(root: &File) -> io::Result<(Marker, Stamp)> {
    let file = regular(root, b"snapshot.json")?;
    let stamp = Stamp::of(&file.metadata()?);
    let mut bytes = Vec::new(); file.take((MAX_LINE + 1) as u64).read_to_end(&mut bytes)?;
    if bytes.len() > MAX_LINE { return Err(invalid("слишком большой snapshot.json")); }
    let marker = serde_json::from_slice(&bytes).map_err(|e| invalid(format!("неверный snapshot.json: {e}")))?;
    Ok((marker, stamp))
}
fn verify_open(root: &File, id: &str, path: PathBuf, held_memory: usize) -> io::Result<VerifiedBackup> {
    let (marker, marker_stamp) = read_marker(root)?;
    if marker.schema != 1 || marker.scope != "world-folder" || marker.status != "complete"
        || marker.snapshot_id != id || !id_valid(id) || marker.entries_file != "files.jsonl"
        || !digest_hex(&marker.entries_sha256) || marker.metadata_policy != POLICY {
        return Err(invalid("неверный complete marker резервной копии"));
    }
    let world = directory(root, b"world")?;
    let list = regular(root, b"files.jsonl")?;
    let before = Stamp::of(&list.metadata()?);
    let mut reader = BufReader::new(list);
    let mut hash = digest::Context::new(&digest::SHA256);
    let mut tree_hash = digest::Context::new(&digest::SHA256);
    let mut buffer = vec![0u8; BUFFER];
    let (mut entries,mut files,mut bytes)=(0u64,0u64,0u64);
    // Потоковая сверка полного дерева с полным списком: второй Inventory
    // не удваивает RAM; path из JSON никогда не используется для открытия.
    let mut budget=Budget {used:held_memory,maximum:MAX_MEMORY};
    walk(&world,Vec::new(),0,&mut budget,&mut |entry,file,_| {
        entries+=1;if entries>MAX_ENTRIES as u64 {return Err(invalid("слишком много записей копии"));}
        let line = bounded_line(&mut reader)?;
        if line.last() != Some(&b'\n') { return Err(invalid("обрезан files.jsonl")); }
        hash.update(&line);
        let record: Record = serde_json::from_slice(&line).map_err(|e| invalid(format!("неверный files.jsonl: {e}")))?;
        record.matches(&entry)?;
        if entry.kind == Kind::File {
            files+=1;bytes=bytes.checked_add(entry.stamp.size).ok_or_else(||invalid("размер копии переполнен"))?;
            let actual = hash_file(file, entry.stamp.size, &mut buffer)?;
            if record.sha256.as_deref() != Some(actual.as_str()) { return Err(invalid("SHA256 файла копии не совпал")); }
        }
        fingerprint(&mut tree_hash,&entry);Ok(())
    })?;
    if marker.entries!=entries || marker.files!=files || marker.total_bytes!=bytes {
        return Err(invalid("число записей/размер не совпали с complete marker"));
    }
    if !bounded_line(&mut reader)?.is_empty() { return Err(invalid("лишние записи в files.jsonl")); }
    if Stamp::of(&reader.get_ref().metadata()?) != before { return Err(changed()); }
    let entries_sha256 = hex(hash.finish().as_ref());
    if entries_sha256 != marker.entries_sha256 { return Err(invalid("SHA256 files.jsonl не совпал")); }
    if tree_fingerprint(&world,held_memory)?!=hex(tree_hash.finish().as_ref()) {return Err(changed());}
    let mut budget = Budget { used:held_memory, maximum:MAX_MEMORY };
    if names(root, &mut budget)? != [b"files.jsonl".to_vec(), b"snapshot.json".to_vec(), b"world".to_vec()] {
        return Err(invalid("неожиданное содержимое каталога backup"));
    }
    check_directory_name(root, b"world", &world)?;
    if !same_inode(&regular(root, b"files.jsonl")?.metadata()?, &reader.get_ref().metadata()?) { return Err(changed()); }
    let (marker_after, stamp_after) = read_marker(root)?;
    if marker_after != marker || stamp_after != marker_stamp { return Err(changed()); }
    Ok(VerifiedBackup { directory:path, snapshot_id:id.to_owned(), file_count:files, total_bytes:bytes, entries_sha256 })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Point { LocksAcquired, VerificationComplete, InventoryComplete, PartialCreated, BeforeFileOpen, BeforeWrite, BeforeFileSync, BeforeFileVerify, EntriesSynced, MarkerSynced, BeforeRename, Published, ParentSynced }
struct Options { memory:usize, id:Option<String> }
impl Default for Options { fn default() -> Self { Self { memory:MAX_MEMORY, id:None } } }

pub(super) fn create(guard: &StartupGuard) -> Result<VerifiedBackup, BackupError> {
    create_with(guard, Options::default(), &mut |_| Ok(()))
}
fn create_with(guard: &StartupGuard, options:Options, hook:&mut impl FnMut(Point)->io::Result<()>) -> Result<VerifiedBackup, BackupError> {
    create_with_completion(guard, options, hook, None, false, &mut |_, _, _| Ok(()))
}
fn create_with_completion(
    guard: &StartupGuard, options: Options, hook: &mut impl FnMut(Point)->io::Result<()>,
    expected: Option<&super::super::profile::Manifest>, unidentified: bool,
    completion: &mut impl FnMut(&LockedWorld<'_>, &VerifiedBackup, &mut bool)->io::Result<()>,
) -> Result<VerifiedBackup, BackupError> {
    let mut retained = None;
    let mut manifest_uncertain = false;
    let mut published = false;
    let mut parent_synced = false;
    let attempt = (|| -> io::Result<VerifiedBackup> {
        let source = LockedWorld::acquire(guard)?;
        hook(Point::LocksAcquired)?;
        source.check_anchor()?;
        if let Some(expected) = expected { checked_manifest(&source, expected)?; }
        if unidentified { require_unidentified_without_receipt(&source)?; }
        let files = inventory(&source.root, options.memory)?;
        known_dimensions(&files)?;
        // Все неизменяемые ограничения проверяются до создания backup ancestors.
        available_space(&source.parent, &files)?;
        match directory(&source.parent, b"backups") {
            Ok(backups) => { match directory(&backups, &source.name) { Ok(_) => {}, Err(e) if e.kind()==io::ErrorKind::NotFound=>{}, Err(e)=>return Err(e) } },
            Err(e) if e.kind()==io::ErrorKind::NotFound=>{}, Err(e)=>return Err(e),
        }
        source.check_anchor()?;
        hook(Point::InventoryComplete)?;
        check_inventory(&source.root,&files,options.memory)?;
        source.check_anchor()?;
        let id = if let Some(id) = options.id { if !id_valid(&id) { return Err(invalid("неверный backup id")); } id }
            else { let mut bytes=[0u8;16]; rand::fill(&mut bytes).map_err(|_| io::Error::other("нет случайных данных для backup id"))?; hex(&bytes) };
        let backups = existing_or_mkdir(&source.parent, b"backups")?;
        let collection = existing_or_mkdir(&backups, &source.name)?;
        let partial_name = format!("{id}.partial");
        // Не создаём partial при существующем final, даже если он повреждён.
        match open_at(&collection, id.as_bytes(), libc::O_PATH, 0) {
            Ok(_) => return Err(io::Error::new(io::ErrorKind::AlreadyExists, "backup id уже существует")),
            Err(e) if e.kind()==io::ErrorKind::NotFound=>{}, Err(e)=>return Err(e),
        }
        let base = guard.world_path().parent().ok_or_else(changed)?.join("backups").join(OsStr::from_bytes(&source.name));
        let partial = match mkdir(&collection, partial_name.as_bytes()) {
            Ok(file) => file,
            Err(error) => {
                // mkdir мог пройти, а open отказать (например, необычный umask).
                // Ничего не чистим, даже если это прежний collision artifact.
                if open_at(&collection,partial_name.as_bytes(),libc::O_PATH,0).is_ok() {
                    retained=Some(base.join(&partial_name));
                }
                return Err(error);
            }
        };
        retained = Some(base.join(&partial_name));
        hook(Point::PartialCreated)?;
        let target = mkdir(&partial, b"world")?;
        let mut list = new_file(&partial, b"files.jsonl")?;
        let mut list_hash = digest::Context::new(&digest::SHA256);
        let mut buffer = vec![0u8; BUFFER];
        for entry in &files.entries {
            let hash = if entry.kind == Kind::Directory {
                if !entry.path.is_empty() { let (parent,name)=destination_parent(&target,&entry.path)?; mkdir(&parent,&name)?; }
                None
            } else {
                hook(Point::BeforeFileOpen)?;
                let mut input = open_entry(&source.root, entry)?;
                let (parent,name)=destination_parent(&target,&entry.path)?;
                let mut output = new_file(&parent,&name)?;
                let mut hash=digest::Context::new(&digest::SHA256); let mut copied=0u64;
                loop {
                    let count=input.read(&mut buffer)?; if count==0 { break; }
                    copied=copied.checked_add(count as u64).filter(|n| *n<=entry.stamp.size).ok_or_else(changed)?;
                    hook(Point::BeforeWrite)?;
                    output.write_all(&buffer[..count])?; hash.update(&buffer[..count]);
                }
                if copied!=entry.stamp.size || Stamp::of(&input.metadata()?)!=entry.stamp { return Err(changed()); }
                hook(Point::BeforeFileSync)?; output.sync_all()?;
                hook(Point::BeforeFileVerify)?;
                let hash=hex(hash.finish().as_ref());
                let readback=regular(&parent,&name)?;
                if hash_file(&readback,copied,&mut buffer)?!=hash { return Err(invalid("записанная копия не совпала с исходным файлом")); }
                restore_metadata(&output,entry)?;
                Some(hash)
            };
            let mut line=serde_json::to_vec(&Record::new(entry,hash)).map_err(io::Error::other)?;
            line.push(b'\n'); if line.len()>MAX_LINE { return Err(invalid("слишком длинная строка перечня")); }
            list.write_all(&line)?; list_hash.update(&line);
        }
        // Дочерние изменения не должны затереть времена/permissions каталогов.
        for entry in files.entries.iter().rev().filter(|e| e.kind==Kind::Directory) {
            let dir=if entry.path.is_empty() { directory(&target,b".")? } else { directory(&target,&entry.path)? };
            restore_metadata(&dir,entry)?;
        }
        list.sync_all()?; hook(Point::EntriesSynced)?;
        check_inventory(&source.root,&files,options.memory)?;
        source.check_anchor()?;
        let source_profile = if !matches!(guard.check, super::super::profile::StartupCheck::LegacyUnidentified) {
            let file=regular(&source.root,super::super::profile::MANIFEST_NAME.as_bytes())?;
            let mut bytes=Vec::new();file.take((super::super::profile::MAX_MANIFEST_BYTES+1) as u64).read_to_end(&mut bytes)?;
            Some(super::super::profile::decode(&bytes).map_err(|e| invalid(e.to_string()))?)
        } else { None };
        let marker=Marker { schema:1,scope:"world-folder".into(),status:"complete".into(),snapshot_id:id.clone(),source_profile,
            entries_file:"files.jsonl".into(),entries_sha256:hex(list_hash.finish().as_ref()),entries:files.entries.len() as u64,
            files:files.files,total_bytes:files.bytes,metadata_policy:POLICY.into() };
        let mut marker_file=new_file(&partial,b"snapshot.json.tmp")?;
        marker_file.write_all(&serde_json::to_vec(&marker).map_err(io::Error::other)?)?;marker_file.sync_all()?;
        rename_no_replace(&partial,b"snapshot.json.tmp",&partial,b"snapshot.json")?;
        partial.sync_all()?;hook(Point::MarkerSynced)?;
        let mut verified=verify_open(&partial,&id,base.join(&id),files.memory)?;
        hook(Point::BeforeRename)?;
        check_inventory(&source.root,&files,options.memory)?;
        source.check_anchor()?;
        if !same_inode(&directory(&source.parent,b"backups")?.metadata()?,&backups.metadata()?)
            || !same_inode(&directory(&backups,&source.name)?.metadata()?,&collection.metadata()?) { return Err(changed()); }
        check_directory_name(&collection,partial_name.as_bytes(),&partial)?;
        rename_no_replace(&collection,partial_name.as_bytes(),&collection,id.as_bytes())?;
        published=true;retained=Some(base.join(&id));
        hook(Point::Published)?;
        check_directory_path(&base.join(&id),&partial)?;
        collection.sync_all()?;backups.sync_all()?;source.parent.sync_all()?;
        parent_synced=true;hook(Point::ParentSynced)?;
        check_directory_path(&base.join(&id),&partial)?;
        source.check_anchor()?;
        verified.directory=base.join(id);
        completion(&source, &verified, &mut manifest_uncertain)?;
        Ok(verified)
    })();
    attempt.map_err(|cause| BackupError { retained, publication_uncertain:manifest_uncertain || (published && !parent_synced), cause })
}
fn rename_no_replace(from:&File, old:&[u8], to:&File, new:&[u8]) -> io::Result<()> {
    let old=component(old)?;let new=component(new)?;
    // SAFETY: dirs/имена живы; RENAME_NOREPLACE никогда не заменяет существующий backup.
    if unsafe { libc::syscall(libc::SYS_renameat2,from.as_raw_fd(),old.as_ptr(),to.as_raw_fd(),new.as_ptr(),1u32) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
pub(super) fn verify(path:&Path) -> Result<VerifiedBackup,BackupError> {
    verify_with(path,&mut |_|Ok(()))
}
fn verify_with(path:&Path, hook:&mut impl FnMut(Point)->io::Result<()>) -> Result<VerifiedBackup,BackupError> {
    let result=(|| -> io::Result<VerifiedBackup> {
        let id=path.file_name().and_then(|v|v.to_str()).filter(|v|id_valid(v)).ok_or_else(|| invalid("требуется final backup id; partial не принимается"))?;
        let root=open_directory(path)?;
        let verified=verify_open(&root,id,path.to_path_buf(),0)?;
        hook(Point::VerificationComplete)?;
        check_directory_path(path,&root)?;
        Ok(verified)
    })();
    result.map_err(|cause|BackupError { retained:Some(path.to_path_buf()),publication_uncertain:false,cause })
}

#[cfg(test)]
mod tests;

// Снимок и публикация манифеста выполняются при непрерывно удержанных
// внешнем и всех существующих session.lock; прежние байты уже проверены.
pub(super) fn create_storage_upgrade(guard: &StartupGuard, expected: &super::super::profile::Manifest) -> Result<VerifiedBackup, BackupError> {
    use super::super::profile;
    let target = profile::current_profile();
    let plan = profile::plan_transition(Some(expected), &target.gameplay_version, std::slice::from_ref(&target))
        .map_err(|error| BackupError { retained: None, publication_uncertain: false, cause: invalid(error.to_string()) })?;
    if plan.kind != profile::TransitionKind::StorageUpgrade
        || !matches!(&guard.check, profile::StartupCheck::StorageUpgradePending { snapshot_id } if snapshot_id == &expected.snapshot_id) {
        return Err(BackupError { retained: None, publication_uncertain: false, cause: invalid("нет незавершённого плана storage10→11") });
    }
    create_with_completion(guard, Options::default(), &mut |_| Ok(()), Some(expected), false,
        &mut |source, backup, uncertain| publish_storage_manifest(source, expected, backup, uncertain))
}

fn checked_manifest(source: &LockedWorld<'_>, expected: &super::super::profile::Manifest) -> io::Result<(File, Stamp)> {
    let file = regular(&source.root, super::super::profile::MANIFEST_NAME.as_bytes())?;
    let stamp = Stamp::of(&metadata_supported(&file, b"mcsa-world.json")?);
    let mut bytes = Vec::new();
    (&file).take((super::super::profile::MAX_MANIFEST_BYTES + 1) as u64).read_to_end(&mut bytes)?;
    let actual = super::super::profile::decode(&bytes).map_err(|error| invalid(error.to_string()))?;
    if actual != *expected || Stamp::of(&file.metadata()?) != stamp { return Err(changed()); }
    Ok((file, stamp))
}

fn publish_storage_manifest(source: &LockedWorld<'_>, expected: &super::super::profile::Manifest,
    backup: &VerifiedBackup, uncertain: &mut bool) -> io::Result<()> {
    use super::super::profile;
    let target = profile::current_profile();
    let plan = profile::plan_transition(Some(expected), &target.gameplay_version, std::slice::from_ref(&target))
        .map_err(|error| invalid(error.to_string()))?;
    if plan.kind != profile::TransitionKind::StorageUpgrade { return Err(invalid("требуется точный переход playerdata10→11")); }
    source.check_anchor()?;
    let (previous, stamp) = checked_manifest(source, expected)?;
    let updated = profile::Manifest { save_format: target.save_format, snapshot_id: backup.snapshot_id().to_owned(), ..expected.clone() };
    let bytes = serde_json::to_vec_pretty(&updated).map_err(io::Error::other)?;
    profile::decode(&bytes).map_err(|error| invalid(error.to_string()))?;
    let temp_name = format!(".mcsa-manifest-{}.tmp", backup.snapshot_id());
    let mut temporary = new_file(&source.parent, temp_name.as_bytes())?;
    temporary.write_all(&bytes)?;
    // SAFETY: действующий наш fd; сохраняем обычные права исходного файла.
    if unsafe { libc::fchmod(temporary.as_raw_fd(), stamp.mode & 0o777) } != 0 { return Err(io::Error::last_os_error()); }
    temporary.sync_all()?;
    let mut readback = Vec::new();
    regular(&source.parent, temp_name.as_bytes())?.take((profile::MAX_MANIFEST_BYTES + 1) as u64).read_to_end(&mut readback)?;
    if readback != bytes { return Err(invalid("новый манифест не прошёл побайтовую проверку")); }
    source.check_anchor()?;
    let (current, current_stamp) = checked_manifest(source, expected)?;
    if current_stamp != stamp || !same_inode(&current.metadata()?, &previous.metadata()?)
        || !same_inode(&regular(&source.parent, temp_name.as_bytes())?.metadata()?, &temporary.metadata()?) { return Err(changed()); }
    let old = component(temp_name.as_bytes())?;
    let new = component(profile::MANIFEST_NAME.as_bytes())?;
    // SAFETY: fd-relative rename заменяет проверенный манифест атомарно.
    if unsafe { libc::renameat(source.parent.as_raw_fd(), old.as_ptr(), source.root.as_raw_fd(), new.as_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    *uncertain = true;
    source.root.sync_all()?;
    source.parent.sync_all()?;
    source.check_anchor()?;
    let (published, _) = checked_manifest(source, &updated)?;
    if !same_inode(&published.metadata()?, &temporary.metadata()?) { return Err(changed()); }
    *uncertain = false;
    Ok(())
}

fn require_absent(root: &File, name: &[u8]) -> io::Result<()> {
    match open_at(root, name, libc::O_PATH, 0) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
        Ok(_) => Err(io::Error::new(io::ErrorKind::AlreadyExists, "манифест/receipt появился во время подготовки снимка")),
    }
}
fn require_unidentified_without_receipt(source: &LockedWorld<'_>) -> io::Result<()> {
    require_absent(&source.root, super::super::profile::MANIFEST_NAME.as_bytes())?;
    require_absent(&source.root, super::super::startup::storage_receipt::NAME.as_bytes())
}

pub(super) fn create_unidentified_storage_receipt(guard: &StartupGuard) -> Result<VerifiedBackup, BackupError> {
    if !matches!(&guard.check, super::super::profile::StartupCheck::LegacyUnidentified) {
        return Err(BackupError { retained: None, publication_uncertain: false, cause: invalid("для receipt требуется неопознанный мир") });
    }
    create_with_completion(guard, Options::default(), &mut |_| Ok(()), None, true,
        &mut |source, backup, uncertain| publish_storage_receipt(source, backup, uncertain))
}

fn publish_storage_receipt(source: &LockedWorld<'_>, backup: &VerifiedBackup, uncertain: &mut bool) -> io::Result<()> {
    use super::super::startup::storage_receipt;
    source.check_anchor()?;
    require_unidentified_without_receipt(source)?;
    let receipt = storage_receipt::Receipt::for_backup(backup.snapshot_id());
    let bytes = serde_json::to_vec_pretty(&receipt).map_err(io::Error::other)?;
    storage_receipt::decode(&bytes)?;
    let temp_name = format!(".mcsa-storage-{}.tmp", backup.snapshot_id());
    let mut temporary = new_file(&source.parent, temp_name.as_bytes())?;
    temporary.write_all(&bytes)?;
    temporary.sync_all()?;
    let mut readback = Vec::new();
    regular(&source.parent, temp_name.as_bytes())?.take(16 * 1024 + 1).read_to_end(&mut readback)?;
    if readback != bytes { return Err(invalid("storage receipt не прошёл побайтовую проверку")); }
    source.check_anchor()?;
    require_unidentified_without_receipt(source)?;
    if !same_inode(&regular(&source.parent, temp_name.as_bytes())?.metadata()?, &temporary.metadata()?) { return Err(changed()); }
    // Никогда не заменяем даже подставленный за время копии receipt.
    rename_no_replace(&source.parent, temp_name.as_bytes(), &source.root, storage_receipt::NAME.as_bytes())?;
    *uncertain = true;
    source.root.sync_all()?; source.parent.sync_all()?;
    source.check_anchor()?;
    require_absent(&source.root, super::super::profile::MANIFEST_NAME.as_bytes())?;
    let published = regular(&source.root, storage_receipt::NAME.as_bytes())?;
    if !same_inode(&published.metadata()?, &temporary.metadata()?) { return Err(changed()); }
    let mut final_bytes = Vec::new();
    published.take(16 * 1024 + 1).read_to_end(&mut final_bytes)?;
    if final_bytes != bytes { return Err(changed()); }
    *uncertain = false;
    Ok(())
}

// Все session.lock живут до конца activate, включая переименования мира.
pub(super) fn create_profile_upgrade(guard: &StartupGuard) -> Result<VerifiedBackup, BackupError> {
    create_with_completion(guard, Options::default(), &mut |_| Ok(()), None, false,
        &mut |source, backup, uncertain| {
            source.check_anchor()?;
            super::super::migration::activate(guard, backup, uncertain)
        })
}
