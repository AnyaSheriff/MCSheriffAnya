// Загрузчик MCSA Guard. В ядре только протокол и проверка подписи;
// закрытая реализация и ключ подписи сюда не входят.
use std::{fs, io::{self, Read, Write}, path::{Path, PathBuf}, process::{Command, Stdio}, time::{Duration, Instant}};
use aws_lc_rs::{digest, signature};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

const PUBLIC_KEY: &[u8; 32] = include_bytes!("guard-public-key.bin");
const RELEASES: &str = "https://github.com/AnyaSheriff/MCSA-Guard/releases/latest/download";
const MAX_BINARY: usize = 32 * 1024 * 1024;
const MIN_SEQUENCE: u64 = 1;

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    product: String,
    version: String,
    sequence: u64,
    protocol: u32,
    target: String,
    sha256: String,
    size: usize,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Bundle { manifest: Vec<u8>, signature: Vec<u8> }

fn error(message: &'static str) -> io::Error { io::Error::other(message) }
fn sha256(bytes: &[u8]) -> String {
    digest::digest(&digest::SHA256, bytes).as_ref().iter().map(|b| format!("{b:02x}")).collect()
}
fn target() -> io::Result<&'static str> {
    if cfg!(all(target_os="linux",target_arch="x86_64")) { Ok("linux-x86_64") }
    else { Err(error("MCSA Guard: для этой платформы подписанный модуль пока не выпущен")) }
}
fn verify_manifest(bytes: &[u8], sig: &[u8], public: &[u8], platform: &str, floor: u64) -> io::Result<Manifest> {
    if bytes.len() > 4096 || sig.len() != 64 { return Err(error("MCSA Guard: неверный размер подписи/описания")); }
    signature::UnparsedPublicKey::new(&signature::ED25519, public).verify(bytes, sig)
        .map_err(|_| error("MCSA Guard: подпись не подтверждена"))?;
    let m: Manifest = serde_json::from_slice(bytes).map_err(|_| error("MCSA Guard: неверное описание выпуска"))?;
    if m.product != "MCSA-Guard" || m.protocol != 1 || m.target != platform || m.sequence < floor.max(MIN_SEQUENCE)
        || m.size == 0 || m.size > MAX_BINARY || m.version.len() > 64 || m.sha256.len() != 64
        || !m.sha256.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
        return Err(error("MCSA Guard: несовместимый выпуск или запрещённый откат"));
    }
    Ok(m)
}
fn verify_binary(m: &Manifest, data: &[u8]) -> io::Result<()> {
    if data.len() != m.size || sha256(data) != m.sha256 { return Err(error("MCSA Guard: файл модуля повреждён или подменён")); }
    Ok(())
}

fn private_dir(path: &Path) -> io::Result<()> {
    if !path.exists() { fs::create_dir_all(path)?; }
    if !fs::symlink_metadata(path)?.is_dir() { return Err(error("MCSA Guard: каталог не должен быть ссылкой")); }
    #[cfg(unix)] {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}
fn read_limited(path: &Path, limit: usize) -> io::Result<Vec<u8>> {
    if !fs::symlink_metadata(path)?.is_file() { return Err(error("MCSA Guard: ожидался обычный файл")); }
    let mut bytes=Vec::new();
    fs::File::open(path)?.take((limit+1) as u64).read_to_end(&mut bytes)?;
    if bytes.len()>limit { return Err(error("MCSA Guard: слишком большой файл")); }
    Ok(bytes)
}
fn atomic_write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut suffix=[0u8;8];
    aws_lc_rs::rand::fill(&mut suffix).map_err(|_|error("MCSA Guard: нет случайных данных"))?;
    let temporary=path.with_extension(format!("{:016x}.new",u64::from_le_bytes(suffix)));
    let mut options=fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; options.mode(0o600); }
    let mut f=options.open(&temporary)?;
    let result=(|| { f.write_all(bytes)?; f.sync_all()?; fs::rename(&temporary,path) })();
    if result.is_err() { let _=fs::remove_file(&temporary); }
    result
}
async fn download(client: &reqwest::Client, url: &str, limit: usize) -> io::Result<Vec<u8>> {
    let mut response=client.get(url).send().await.and_then(|r|r.error_for_status())
        .map_err(|_|error("MCSA Guard: загрузка с GitHub не удалась"))?;
    let mut data=Vec::new();
    while let Some(chunk)=response.chunk().await.map_err(|_|error("MCSA Guard: загрузка прервана"))? {
        if data.len()+chunk.len()>limit { return Err(error("MCSA Guard: превышен размер загрузки")); }
        data.extend_from_slice(&chunk);
    }
    Ok(data)
}

pub struct Guard {
    #[cfg(target_os="linux")]
    pepper: PathBuf,
    // На Linux исполняем запечатанную копию проверенных байтов: замена файла
    // после проверки не меняет запускаемый код (нет окна check→exec).
    #[cfg(target_os="linux")]
    sealed: fs::File,
}
impl Guard {
    pub async fn prepare(directory: &Path, protected_accounts: bool) -> io::Result<Self> {
        let platform=target()?;
        private_dir(directory)?;
        let directory=fs::canonicalize(directory)?;
        let active=directory.join("active.bundle");
        let cached=if active.exists() {
            let bundle: Bundle=serde_json::from_slice(&read_limited(&active,20000)?)
                .map_err(|_|error("MCSA Guard: повреждён кэш подписи"))?;
            let bytes=bundle.manifest;
            let sig=bundle.signature;
            let manifest=verify_manifest(&bytes,&sig,PUBLIC_KEY,platform,MIN_SEQUENCE)?;
            let data=read_limited(&directory.join(&manifest.sha256),MAX_BINARY)?;
            verify_binary(&manifest,&data)?;
            Some((manifest,data))
        } else { None };
        let floor=cached.as_ref().map_or(MIN_SEQUENCE,|(m,_)|m.sequence);
        let client=reqwest::Client::builder().https_only(true).timeout(Duration::from_secs(20)).build()
            .map_err(|_|error("MCSA Guard: не удалось создать HTTPS клиент"))?;
        let remote=async {
            let bytes=download(&client,&format!("{RELEASES}/{platform}.json"),4096).await?;
            let sig=download(&client,&format!("{RELEASES}/{platform}.sig"),64).await?;
            let m=verify_manifest(&bytes,&sig,PUBLIC_KEY,platform,floor)?;
            if let Some((old,data))=&cached && m.sequence==old.sequence {
                if m.sha256!=old.sha256 { return Err(error("MCSA Guard: один номер выпуска имеет разные файлы")); }
                return Ok((m,data.clone()));
            }
            let data=download(&client,&format!("{RELEASES}/mcsa-guard-{platform}"),MAX_BINARY).await?;
            verify_binary(&m,&data)?;
            let file=directory.join(&m.sha256);
            atomic_write(&file,&data)?;
            // Подпись и manifest переключаются одним атомарным rename.
            let bundle=serde_json::to_vec(&Bundle { manifest:bytes,signature:sig })
                .map_err(|_|error("MCSA Guard: ошибка упаковки выпуска"))?;
            atomic_write(&active,&bundle)?;
            Ok((m,data))
        }.await;
        let (m,data)=match remote {
            Ok(pair)=>pair,
            Err(e)=>if let Some(old)=cached {
                crate::log_warn!("{}; использую ранее проверенный MCSA Guard",e);
                old
            } else { return Err(e); },
        };
        let guard=Self::from_verified(&data,directory.join("pepper"))?;
        let marker=directory.join("initialized");
        let create=!protected_accounts && !marker.exists();
        let init=guard.request(&serde_json::json!({"protocol":1,"op":"init","create":create}))?;
        let key_id=init.get("key_id").and_then(|v|v.as_str()).ok_or_else(||error("MCSA Guard: нет идентификатора секрета"))?;
        if marker.exists() {
            if read_limited(&marker,64)? != key_id.as_bytes() { return Err(error("MCSA Guard: секрет установки был заменён")); }
        } else { atomic_write(&marker,key_id.as_bytes())?; }
        crate::log_info!("Auth: MCSA Guard {}, подпись подтверждена",m.version);
        Ok(guard)
    }

    fn from_verified(data: &[u8], pepper: PathBuf) -> io::Result<Self> {
        #[cfg(target_os="linux")] {
            use std::os::fd::FromRawFd;
            let fd=unsafe { libc::memfd_create(c"mcsa-guard".as_ptr(),libc::MFD_ALLOW_SEALING|libc::MFD_CLOEXEC) };
            if fd<0 { return Err(io::Error::last_os_error()); }
            let mut file=unsafe { fs::File::from_raw_fd(fd) };
            file.write_all(data)?;
            if unsafe { libc::fcntl(fd,libc::F_ADD_SEALS,libc::F_SEAL_WRITE|libc::F_SEAL_GROW|libc::F_SEAL_SHRINK|libc::F_SEAL_SEAL) }<0 {
                return Err(io::Error::last_os_error());
            }
            Ok(Self { pepper,sealed:file })
        }
        #[cfg(not(target_os="linux"))] { let _=(data,pepper); Err(error("MCSA Guard: платформа пока не поддерживается")) }
    }
    fn command(&self) -> io::Result<Command> {
        #[cfg(target_os="linux")] {
            use std::{ffi::CString, os::{fd::AsRawFd, unix::{ffi::OsStrExt, process::CommandExt}}};
            let fd=self.sealed.as_raw_fd();
            let pepper=CString::new(self.pepper.as_os_str().as_bytes())
                .map_err(|_|error("MCSA Guard: недопустимый путь секрета"))?;
            // man7 execveat(2): "on systems that do not have the /proc filesystem mounted".
            // Имя — только метка: pre_exec либо заменяет процесс проверенным ELF,
            // либо возвращает ошибку. Файл с таким именем никогда не запускается.
            let mut command=Command::new("mcsa-guard-from-sealed-memory");
            command.env_clear();
            // После fork допустимы только async-signal-safe операции. CString
            // создаётся выше, в родителе; в потомке нет аллокаций и блокировок.
            unsafe {
                command.pre_exec(move || {
                    let argv=[c"mcsa-guard".as_ptr(),pepper.as_ptr(),std::ptr::null()];
                    let envp=[std::ptr::null::<libc::c_char>()];
                    libc::syscall(libc::SYS_execveat,fd,c"".as_ptr(),argv.as_ptr(),envp.as_ptr(),libc::AT_EMPTY_PATH);
                    Err(io::Error::last_os_error())
                });
            }
            Ok(command)
        }
        #[cfg(not(target_os="linux"))] { Err(error("MCSA Guard: платформа пока не поддерживается")) }
    }

    fn request(&self, value: &serde_json::Value) -> io::Result<serde_json::Value> {
        let bytes=Zeroizing::new(serde_json::to_vec(value).map_err(|_|error("MCSA Guard: неверный запрос"))?);
        if bytes.len()>4096 { return Err(error("MCSA Guard: запрос слишком большой")); }
        let mut child=self.command()?
            .stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::null()).spawn()
            .map_err(|_|error("MCSA Guard: не удалось запустить модуль; хостинг должен разрешать memfd и execveat"))?;
        let result=(|| {
            child.stdin.take().ok_or_else(||error("MCSA Guard: нет канала"))?.write_all(&bytes)?;
            let deadline=Instant::now()+Duration::from_secs(10);
            loop {
                if let Some(status)=child.try_wait()? {
                    if !status.success() { return Err(error("MCSA Guard: ошибка модуля или секрета установки")); }
                    break;
                }
                if Instant::now()>=deadline { return Err(error("MCSA Guard: превышено время проверки")); }
                std::thread::sleep(Duration::from_millis(5));
            }
            let mut output=Vec::new();
            child.stdout.take().ok_or_else(||error("MCSA Guard: нет ответа"))?.take(2049).read_to_end(&mut output)?;
            if output.len()>2048 { return Err(error("MCSA Guard: ответ слишком большой")); }
            let response:serde_json::Value=serde_json::from_slice(&output).map_err(|_|error("MCSA Guard: неверный ответ"))?;
            if response["protocol"]!=1 || response["ok"]!=true { return Err(error("MCSA Guard: операция отклонена")); }
            Ok(response)
        })();
        if result.is_err() { let _=child.kill(); }
        let _=child.wait();
        result
    }
    #[cfg(test)]
    pub(super) fn test_fixture(directory:&Path,pepper:PathBuf)->io::Result<Self> {
        let bytes=read_limited(&directory.join("linux-x86_64.json"),4096)?;
        let sig=read_limited(&directory.join("linux-x86_64.sig"),64)?;
        let manifest=verify_manifest(&bytes,&sig,PUBLIC_KEY,"linux-x86_64",MIN_SEQUENCE)?;
        let data=read_limited(&directory.join("mcsa-guard-linux-x86_64"),MAX_BINARY)?;
        verify_binary(&manifest,&data)?;
        let guard=Self::from_verified(&data,pepper)?;
        guard.request(&serde_json::json!({"protocol":1,"op":"init","create":true}))?;
        Ok(guard)
    }
    pub fn hash(&self,name:&str,password:&str)->io::Result<String> {
        let mut request=serde_json::json!({"protocol":1,"op":"hash","name":name,"password":password});
        let result=self.request(&request);
        wipe_password(&mut request);
        let response=result?;
        let record=response["record"].as_str().filter(|v| !v.is_empty() && v.len()<=1024)
            .ok_or_else(||error("MCSA Guard: неверная запись пароля"))?;
        Ok(record.into())
    }
    pub fn verify(&self,name:&str,password:&str,record:&str)->io::Result<bool> {
        let mut request=serde_json::json!({"protocol":1,"op":"verify","name":name,"password":password,"record":record});
        let result=self.request(&request);
        wipe_password(&mut request);
        result?["valid"].as_bool().ok_or_else(||error("MCSA Guard: неверный результат проверки"))
    }
}
fn wipe_password(request:&mut serde_json::Value) {
    use zeroize::Zeroize;
    if let Some(serde_json::Value::String(value))=request.get_mut("password") { value.zeroize(); }
}

#[cfg(test)]
mod tests {
    use super::*;
    use signature::KeyPair;
    #[cfg(target_os="linux")]
    #[test]
    fn sealed_copy_cannot_change_and_invalid_executable_fails_closed() {
        use std::os::unix::fs::FileExt;
        let mut data=b"not an ELF executable".to_vec();
        let guard=Guard::from_verified(&data,PathBuf::from("unused-test-pepper")).unwrap();
        data.fill(0);
        let mut snapshot=vec![0;guard.sealed.metadata().unwrap().len() as usize];
        guard.sealed.read_exact_at(&mut snapshot,0).unwrap();
        assert_eq!(snapshot,b"not an ELF executable");
        assert_eq!(guard.sealed.write_at(b"x",0).unwrap_err().raw_os_error(),Some(libc::EPERM));
        assert_eq!(guard.sealed.set_len(0).unwrap_err().raw_os_error(),Some(libc::EPERM));
        let error=guard.command().unwrap().spawn().unwrap_err();
        assert_eq!(error.raw_os_error(),Some(libc::ENOEXEC));
    }

    #[test]
    fn rejects_tampering_wrong_key_platform_protocol_and_rollback() {
        let key=signature::Ed25519KeyPair::generate().unwrap();
        let other=signature::Ed25519KeyPair::generate().unwrap();
        let data=b"fixture executable";
        let m=Manifest { product:"MCSA-Guard".into(),version:"0.1.0".into(),sequence:2,protocol:1,target:"linux-x86_64".into(),sha256:sha256(data),size:data.len() };
        let bytes=serde_json::to_vec(&m).unwrap();
        let sig=key.sign(&bytes);
        let verified=verify_manifest(&bytes,sig.as_ref(),key.public_key().as_ref(),"linux-x86_64",2).unwrap();
        verify_binary(&verified,data).unwrap();
        assert!(verify_binary(&verified,b"fixture executablE").is_err());
        assert!(verify_manifest(&bytes,sig.as_ref(),other.public_key().as_ref(),"linux-x86_64",1).is_err());
        assert!(verify_manifest(&bytes,sig.as_ref(),key.public_key().as_ref(),"other",1).is_err());
        assert!(verify_manifest(&bytes,sig.as_ref(),key.public_key().as_ref(),"linux-x86_64",3).is_err());
        let mut changed=bytes.clone(); changed[0]^=1;
        assert!(verify_manifest(&changed,sig.as_ref(),key.public_key().as_ref(),"linux-x86_64",1).is_err());
        let mut m=m; m.protocol=2;
        let bytes=serde_json::to_vec(&m).unwrap();
        assert!(verify_manifest(&bytes,key.sign(&bytes).as_ref(),key.public_key().as_ref(),"linux-x86_64",1).is_err());
    }
}
