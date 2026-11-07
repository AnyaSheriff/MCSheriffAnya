//! Локальный API приложения: только снимок статистики, без команд и пробуждения мира.
//! Unix socket лежит в приватной папке приложения; по умолчанию API выключен.
use std::{io, path::PathBuf, sync::{Arc, atomic::Ordering}, time::{Duration, Instant}};
use serde_json::{Value, json};
use tokio::{io::{AsyncReadExt, AsyncWriteExt}, net::{UnixListener, UnixStream}, sync::{Semaphore, watch}};
use crate::shared::Shared;

pub fn start(shared: Arc<Shared>, shutdown: watch::Receiver<bool>) {
    let (Some(path),Some(session))=(std::env::var_os("MCSA_APP_API_SOCKET"),std::env::var_os("MCSA_APP_API_SESSION")) else { return; };
    let Some(session)=session.to_str().filter(|s| !s.is_empty()&&s.len()<=64&&s.bytes().all(|b| b.is_ascii_alphanumeric()||b==b'-')) else { return; };
    if let Err(error)=serve(PathBuf::from(path),session.to_owned(),shared,shutdown) {
        crate::log_warn!("Локальный API приложения недоступен: {}",error);
    }
}

fn serve(path: PathBuf,session: String,shared: Arc<Shared>,mut shutdown: watch::Receiver<bool>) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    // Старый путь удаляет родительское приложение после завершения предыдущего процесса.
    // Здесь нельзя удалять произвольный файл из переменной окружения.
    let listener=UnixListener::bind(&path)?;
    if let Err(error)=std::fs::set_permissions(&path,std::fs::Permissions::from_mode(0o600)) { drop(listener);return Err(error); }
    let permits=Arc::new(Semaphore::new(4));
    let began=Instant::now();
    let presence=shared.players.lock().expect("игроки").presence();
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _=shutdown.changed()=>break,
                incoming=listener.accept()=>{
                    let Ok((stream,_))=incoming else { break; };
                    let Ok(permit)=Arc::clone(&permits).try_acquire_owned() else { continue; };
                    let shared=Arc::clone(&shared);let session=session.clone();let presence=presence.clone();
                    tokio::spawn(async move {
                        let _permit=permit;
                        let _=tokio::time::timeout(Duration::from_secs(1),async {
                            handle(stream,|| snapshot(&shared,&session,began.elapsed(),presence.borrow().online)).await
                        }).await;
                    });
                }
            }
        }
        drop(listener);
        let _=std::fs::remove_file(path);
    });
    Ok(())
}

async fn handle(mut stream: UnixStream,data: impl FnOnce()->Value) -> io::Result<()> {
    let mut request=[0u8;7];stream.read_exact(&mut request).await?;
    if &request!=b"status\n" { return Err(io::Error::new(io::ErrorKind::InvalidInput,"неизвестный запрос API")); }
    let mut bytes=serde_json::to_vec(&data())?;bytes.push(b'\n');
    stream.write_all(&bytes).await?;stream.shutdown().await
}

fn snapshot(shared: &Shared,session: &str,uptime: Duration,online: usize) -> Value {
    let paused=shared.idle_paused.load(Ordering::Acquire);
    let timing=shared.world_cmd.ticking.try_lock().ok().map(|t| (t.metrics(),t.frozen));
    let tps=if paused { Some(0.0) } else { timing.map(|((tps,_),_)| tps) };
    let mspt=if paused { Some(0.0) } else { timing.map(|((_,mspt),_)| mspt) };
    json!({"version":1,"session":session,"pid":std::process::id(),"uptime_ms":uptime.as_millis().min(u128::from(u64::MAX)) as u64,
        "online":online,"max_players":shared.properties.max_players,"tps":tps,"mspt":mspt,"paused":paused,
        "frozen":timing.map(|(_,frozen)| frozen),"tick":*shared.ticks.borrow(),"cpu_time_us":cpu_time_us(),
        "logical_cpus":crate::tick::cores(),"rss_bytes":resident_bytes()})
}

fn cpu_time_us() -> Option<u64> {
    // getrusage(2): RUSAGE_SELF — сумма CPU-времени всех потоков своего процесса.
    let mut usage=std::mem::MaybeUninit::<libc::rusage>::zeroed();
    if unsafe { libc::getrusage(libc::RUSAGE_SELF,usage.as_mut_ptr()) }!=0 { return None; }
    let usage=unsafe { usage.assume_init() };
    let user=u64::try_from(usage.ru_utime.tv_sec).ok()?.checked_mul(1_000_000)?.checked_add(u64::try_from(usage.ru_utime.tv_usec).ok()?)?;
    let system=u64::try_from(usage.ru_stime.tv_sec).ok()?.checked_mul(1_000_000)?.checked_add(u64::try_from(usage.ru_stime.tv_usec).ok()?)?;
    user.checked_add(system)
}

fn resident_bytes() -> Option<u64> {
    // proc_pid_statm(5): resident дан в страницах; читаем только собственный процесс.
    // Это оценка RSS ОС, а не точный размер выделенных Rust-объектов.
    #[cfg(any(target_os="linux",target_os="android"))]
    {
        let text=std::fs::read_to_string("/proc/self/statm").ok()?;
        let pages=text.split_whitespace().nth(1)?.parse::<u64>().ok()?;
        let size=u64::try_from(unsafe { libc::sysconf(libc::_SC_PAGESIZE) }).ok()?;
        pages.checked_mul(size)
    }
    #[cfg(not(any(target_os="linux",target_os="android")))]
    { None }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn shared()->Arc<Shared> {
        Arc::new(Shared::new(crate::world::World::in_memory(),crate::world::World::in_memory(),Default::default(),Default::default(),Default::default(),
            crate::ops::Ops::open(&std::env::temp_dir().join(format!("mcsa-api-ops-{}",std::process::id()))),Default::default()))
    }
    #[tokio::test]
    async fn snapshot_is_read_only_and_has_real_process_resources() {
        let shared=shared();let presence=shared.players.lock().unwrap().presence();let before=*presence.borrow();
        shared.idle_paused.store(true,Ordering::Release);
        let value=snapshot(&shared,"own-session",Duration::from_secs(3),0);
        assert_eq!(value["version"],1);assert_eq!(value["session"],"own-session");assert_eq!(value["pid"],std::process::id());
        assert_eq!(value["paused"],true);assert_eq!(value["tps"],0.0);assert_eq!(value["mspt"],0.0);assert_eq!(value["uptime_ms"],3000);
        assert!(value["cpu_time_us"].as_u64().is_some());assert!(value["logical_cpus"].as_u64().unwrap()>0);
        #[cfg(any(target_os="linux",target_os="android"))]
        assert!(value["rss_bytes"].as_u64().unwrap()>0);
        assert_eq!(presence.borrow().console_until,before.console_until);assert_eq!(presence.borrow().changed,before.changed);
        assert_eq!(*shared.ticks.borrow(),0);assert!(shared.idle_paused.load(Ordering::Acquire));
    }
    #[tokio::test]
    async fn wire_returns_json_and_rejects_commands() {
        let (mut client,server)=UnixStream::pair().unwrap();
        let task=tokio::spawn(handle(server,||json!({"version":1,"online":3})));
        client.write_all(b"status\n").await.unwrap();let mut bytes=Vec::new();client.read_to_end(&mut bytes).await.unwrap();
        assert_eq!(serde_json::from_slice::<Value>(&bytes).unwrap()["online"],3);assert_eq!(bytes.last(),Some(&b'\n'));task.await.unwrap().unwrap();
        let (mut client,server)=UnixStream::pair().unwrap();
        let task=tokio::spawn(handle(server,||panic!("команда не должна вызывать снимок")));
        client.write_all(b"stop!!\n").await.unwrap();assert!(task.await.unwrap().is_err());
    }
    #[tokio::test]
    async fn socket_is_private_and_shuts_down_without_waking_world() {
        use std::os::unix::fs::PermissionsExt;
        let shared=shared();let dir=std::env::temp_dir().join(format!("mcsa-app-api-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir(&dir).unwrap();let path=dir.join("stats.sock");let (stop,receiver)=watch::channel(false);
        serve(path.clone(),"own-session".into(),Arc::clone(&shared),receiver).unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode()&0o777,0o600);
        let mut client=UnixStream::connect(&path).await.unwrap();client.write_all(b"status\n").await.unwrap();let mut bytes=Vec::new();client.read_to_end(&mut bytes).await.unwrap();
        assert_eq!(serde_json::from_slice::<Value>(&bytes).unwrap()["session"],"own-session");assert_eq!(*shared.ticks.borrow(),0);
        assert!(shared.players.lock().unwrap().presence().borrow().console_until.is_none());
        stop.send(true).unwrap();for _ in 0..100 { if !path.exists() { break; } tokio::time::sleep(Duration::from_millis(5)).await; }
        assert!(!path.exists());std::fs::remove_dir(dir).unwrap();
    }
}
