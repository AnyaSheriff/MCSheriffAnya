//! Только допуск раскладок записи, без определения игрового профиля мира.
//! После первого проверенного снимка receipt — доверенная локальная запись,
//! как манифест: при повторном запуске наличие/целостность копии НЕ проверяется.
//! Новый/пустой мир не создаём здесь; при первом непустом перезапуске будет
//! сделана одна дополнительная копия, затем receipt исключает её повторение.
use std::fs::{self, OpenOptions};
use std::io::{self, Read};
use std::path::Path;
use serde::{Deserialize, Serialize};

pub(crate) const NAME: &str = "mcsa-storage.json";
const MAX_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Receipt {
    schema: u32,
    region_layout: u32,
    chunk_layout: u32,
    playerdata_layout: u32,
    pub(crate) snapshot_id: String,
}

impl Receipt {
    pub(crate) fn needs_upgrade(&self) -> bool { self.playerdata_layout != crate::playerdata::FORMAT_VERSION }

    pub(crate) fn for_backup(snapshot_id: &str) -> Self {
        Self { schema: 1, region_layout: 1, chunk_layout: u32::from(super::super::CHUNK_VERSION),
            playerdata_layout: crate::playerdata::FORMAT_VERSION, snapshot_id: snapshot_id.into() }
    }
}

pub(crate) fn decode(bytes: &[u8]) -> io::Result<Receipt> {
    if bytes.len() > MAX_BYTES { return Err(io::Error::new(io::ErrorKind::InvalidData, "storage receipt слишком велик")); }
    let receipt: Receipt = serde_json::from_slice(bytes).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let mut supported = Receipt::for_backup(&receipt.snapshot_id);
    if matches!(receipt.playerdata_layout, 8..=11) { supported.playerdata_layout = receipt.playerdata_layout; }
    if receipt != supported || receipt.snapshot_id.len() != 32
        || !receipt.snapshot_id.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)) {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "неподдержанная схема/раскладка или snapshot_id storage receipt"));
    }
    Ok(receipt)
}

pub(crate) fn read(world: &Path) -> io::Result<Option<Receipt>> {
    let path = world.join(NAME);
    match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
        Ok(metadata) if !metadata.is_file() => return Err(io::Error::new(io::ErrorKind::InvalidData, "storage receipt должен быть обычным файлом")),
        Ok(_) => {}
    }
    let mut options = OpenOptions::new(); options.read(true);
    #[cfg(unix)] {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options.open(path)?;
    if !file.metadata()?.is_file() { return Err(io::Error::new(io::ErrorKind::InvalidData, "storage receipt подменён")); }
    let mut bytes = Vec::new();
    file.take((MAX_BYTES + 1) as u64).read_to_end(&mut bytes)?;
    decode(&bytes).map(Some)
}
