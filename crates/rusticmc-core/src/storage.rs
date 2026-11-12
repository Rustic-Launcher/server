//! Per-plugin storage: data folders, the key-value store and per-player data.
//!
//! Layout under the server directory:
//!
//! ```text
//! <server>/plugins/<id>/                 data folder (WASI "/" with filesystem = "data-folder")
//! <server>/plugins/<id>/config.toml      copied from the package's default on first enable
//! <server>/plugins/<id>/.rustic/kv.rkv   the plugin's KV store (`storage` interface)
//! <server>/plugins/<id>/.rustic/players/<uuid>.rkv   per-player data (`players.get-data`...)
//! ```
//!
//! A dev-mode plugin directory named exactly `<id>` would collide with its data folder; its
//! data folder is then `<server>/plugins/<id>/.data/`.
//!
//! Stores live in memory and are written atomically (temporary file, fsync, rename) when
//! flushed: at the end of a tick at most every `storage.flush_interval_ticks`, on disable and
//! on shutdown. The file format (`RKV1`) is: magic, entry count (u32 LE), then per entry key
//! length (u32), key (UTF-8), value length (u32), value, and a trailing FNV-1a 64 checksum of
//! everything before it. A corrupt file is refused (never silently emptied).

use crate::id::PluginId;
use crate::model::Uuid;
use std::collections::{BTreeMap, HashMap};
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

/// Max key length (bytes).
pub const MAX_KEY_BYTES: usize = 1024;
/// Max value length (bytes).
pub const MAX_VALUE_BYTES: usize = 16 * 1024 * 1024;
/// Max bytes of one player's data for one plugin.
pub const MAX_PLAYER_DATA_BYTES: u64 = 1024 * 1024;
const MAGIC: &[u8; 4] = b"RKV1";

/// Write `bytes` to `path` atomically: `<path>.tmp`, fsync, rename. Creates parent dirs.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)?;
    }
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
    }
    std::fs::rename(&tmp, path)
}

/// Storage errors.
#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    /// File system error.
    #[error("{path}: {source}")]
    Io {
        /// Path.
        path: PathBuf,
        /// Cause.
        #[source]
        source: std::io::Error,
    },
    /// The file exists but is not a valid store.
    #[error("{0}: corrupt store file (left untouched)")]
    Corrupt(PathBuf),
    /// Bad key.
    #[error("invalid key: {0}")]
    InvalidKey(String),
    /// The value or the store would be too large.
    #[error("storage quota exceeded: {0}")]
    Quota(String),
    /// The plugin has no data folder registered.
    #[error("plugin `{0}` has no storage (not loaded)")]
    UnknownPlugin(String),
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// An in-memory map persisted to one file.
#[derive(Debug)]
pub struct KvStore {
    path: Option<PathBuf>,
    map: BTreeMap<String, Vec<u8>>,
    bytes: u64,
    max_bytes: u64,
    dirty: bool,
}

fn validate_key(key: &str) -> Result<(), StorageError> {
    if key.is_empty() || key.len() > MAX_KEY_BYTES {
        return Err(StorageError::InvalidKey(format!(
            "keys must be 1 to {MAX_KEY_BYTES} bytes"
        )));
    }
    if key.contains('\0') {
        return Err(StorageError::InvalidKey("keys must not contain NUL".into()));
    }
    Ok(())
}

impl KvStore {
    /// A store that is never written to disk.
    pub fn in_memory(max_bytes: u64) -> Self {
        Self {
            path: None,
            map: BTreeMap::new(),
            bytes: 0,
            max_bytes,
            dirty: false,
        }
    }

    /// Open (or start empty if the file does not exist).
    pub fn open(path: &Path, max_bytes: u64) -> Result<Self, StorageError> {
        let mut s = Self::in_memory(max_bytes);
        s.path = Some(path.to_owned());
        match std::fs::read(path) {
            Ok(bytes) => {
                s.map =
                    Self::decode(&bytes).ok_or_else(|| StorageError::Corrupt(path.to_owned()))?;
                s.bytes = s.map.iter().map(|(k, v)| (k.len() + v.len()) as u64).sum();
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => {
                return Err(StorageError::Io {
                    path: path.to_owned(),
                    source,
                });
            }
        }
        Ok(s)
    }

    fn decode(bytes: &[u8]) -> Option<BTreeMap<String, Vec<u8>>> {
        if bytes.len() < 16 || &bytes[..4] != MAGIC {
            return None;
        }
        let (body, sum) = bytes.split_at(bytes.len() - 8);
        if fnv1a(body) != u64::from_le_bytes(sum.try_into().ok()?) {
            return None;
        }
        let mut pos = 4;
        let u32_at = |pos: &mut usize| -> Option<usize> {
            let v = u32::from_le_bytes(body.get(*pos..*pos + 4)?.try_into().ok()?);
            *pos += 4;
            Some(v as usize)
        };
        let count = u32_at(&mut pos)?;
        let mut map = BTreeMap::new();
        for _ in 0..count {
            let kl = u32_at(&mut pos)?;
            let key = std::str::from_utf8(body.get(pos..pos + kl)?)
                .ok()?
                .to_owned();
            pos += kl;
            let vl = u32_at(&mut pos)?;
            let value = body.get(pos..pos + vl)?.to_vec();
            pos += vl;
            map.insert(key, value);
        }
        (pos == body.len()).then_some(map)
    }

    fn encode(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.bytes as usize + 16 + self.map.len() * 8);
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&(self.map.len() as u32).to_le_bytes());
        for (k, v) in &self.map {
            out.extend_from_slice(&(k.len() as u32).to_le_bytes());
            out.extend_from_slice(k.as_bytes());
            out.extend_from_slice(&(v.len() as u32).to_le_bytes());
            out.extend_from_slice(v);
        }
        let sum = fnv1a(&out);
        out.extend_from_slice(&sum.to_le_bytes());
        out
    }

    /// A value.
    pub fn get(&self, key: &str) -> Option<&[u8]> {
        self.map.get(key).map(Vec::as_slice)
    }

    /// Set a value (refused if the key is invalid or a quota would be exceeded).
    pub fn set(&mut self, key: &str, value: Vec<u8>) -> Result<(), StorageError> {
        validate_key(key)?;
        if value.len() > MAX_VALUE_BYTES {
            return Err(StorageError::Quota(format!(
                "a value is limited to {MAX_VALUE_BYTES} bytes"
            )));
        }
        let old = self
            .map
            .get(key)
            .map(|v| (key.len() + v.len()) as u64)
            .unwrap_or(0);
        let new_total = self.bytes - old + (key.len() + value.len()) as u64;
        if new_total > self.max_bytes {
            return Err(StorageError::Quota(format!(
                "the store would hold {new_total} bytes, the limit is {}",
                self.max_bytes
            )));
        }
        self.bytes = new_total;
        self.map.insert(key.to_owned(), value);
        self.dirty = true;
        Ok(())
    }

    /// Delete a key; returns whether it existed.
    pub fn delete(&mut self, key: &str) -> bool {
        match self.map.remove(key) {
            Some(v) => {
                self.bytes -= (key.len() + v.len()) as u64;
                self.dirty = true;
                true
            }
            None => false,
        }
    }

    /// Keys starting with `prefix`, sorted.
    pub fn keys(&self, prefix: &str) -> Vec<String> {
        self.map
            .range(prefix.to_owned()..)
            .take_while(|(k, _)| k.starts_with(prefix))
            .map(|(k, _)| k.clone())
            .collect()
    }

    /// Number of entries.
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Whether empty.
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// Bytes of keys + values.
    pub fn size_bytes(&self) -> u64 {
        self.bytes
    }

    /// Whether there are unsaved changes.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// Write to disk if dirty (an empty store deletes its file).
    pub fn flush(&mut self) -> Result<(), StorageError> {
        if !self.dirty {
            return Ok(());
        }
        let Some(path) = &self.path else {
            self.dirty = false;
            return Ok(());
        };
        let res = if self.map.is_empty() {
            match std::fs::remove_file(path) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
                _ => Ok(()),
            }
        } else {
            write_atomic(path, &self.encode())
        };
        res.map_err(|source| StorageError::Io {
            path: path.clone(),
            source,
        })?;
        self.dirty = false;
        Ok(())
    }
}

/// Data-folder layout helpers.
#[derive(Clone, Debug)]
pub struct DataLayout {
    server_dir: PathBuf,
}

impl DataLayout {
    /// For a server directory.
    pub fn new(server_dir: impl Into<PathBuf>) -> Self {
        Self {
            server_dir: server_dir.into(),
        }
    }

    /// The server directory.
    pub fn server_dir(&self) -> &Path {
        &self.server_dir
    }

    /// `<server>/plugins`.
    pub fn plugins_dir(&self) -> PathBuf {
        self.server_dir.join("plugins")
    }

    /// `<server>/timings`.
    pub fn timings_dir(&self) -> PathBuf {
        self.server_dir.join("timings")
    }

    /// `<server>/permissions.toml`.
    pub fn permissions_file(&self) -> PathBuf {
        self.server_dir.join("permissions.toml")
    }

    /// The data folder of a plugin loaded from `source` (see the module docs for the dev-mode
    /// special case).
    pub fn data_dir(&self, id: &str, source: Option<&Path>) -> PathBuf {
        let dir = self.plugins_dir().join(id);
        match source {
            Some(src) if same_path(src, &dir) => dir.join(".data"),
            _ => dir,
        }
    }
}

fn same_path(a: &Path, b: &Path) -> bool {
    match (a.canonicalize(), b.canonicalize()) {
        (Ok(x), Ok(y)) => x == y,
        _ => a == b,
    }
}

/// `.rustic/kv.rkv` inside a data folder.
pub fn kv_path(data_dir: &Path) -> PathBuf {
    data_dir.join(".rustic").join("kv.rkv")
}

/// `.rustic/players/<uuid>.rkv` inside a data folder.
pub fn player_data_path(data_dir: &Path, player: Uuid) -> PathBuf {
    data_dir
        .join(".rustic")
        .join("players")
        .join(format!("{player}.rkv"))
}

/// Copy the package's default `config.toml` into the data folder if there is none yet.
/// Returns whether it was written.
pub fn install_default_config(data_dir: &Path, default: Option<&str>) -> std::io::Result<bool> {
    let target = data_dir.join(crate::archive::DEFAULT_CONFIG_FILE);
    match default {
        Some(text) if !target.exists() => {
            write_atomic(&target, text.as_bytes())?;
            Ok(true)
        }
        _ => Ok(false),
    }
}

#[derive(Default)]
struct StorageInner {
    dirs: HashMap<PluginId, PathBuf>,
    kv: HashMap<PluginId, KvStore>,
    players: HashMap<(PluginId, Uuid), KvStore>,
}

/// Thread-safe storage of every plugin (the async instance uses it too).
pub struct Storage {
    max_kv_bytes: u64,
    inner: Mutex<StorageInner>,
}

impl std::fmt::Debug for Storage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Storage").finish_non_exhaustive()
    }
}

impl Storage {
    /// Empty storage with a per-plugin KV quota.
    pub fn new(max_kv_bytes: u64) -> Self {
        Self {
            max_kv_bytes,
            inner: Mutex::new(StorageInner::default()),
        }
    }

    fn lock(&self) -> MutexGuard<'_, StorageInner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Register a plugin's data folder (on load). Creates it.
    pub fn register(&self, id: &PluginId, data_dir: PathBuf) -> std::io::Result<()> {
        std::fs::create_dir_all(&data_dir)?;
        self.lock().dirs.insert(id.clone(), data_dir);
        Ok(())
    }

    /// A plugin's data folder.
    pub fn data_dir(&self, id: &str) -> Option<PathBuf> {
        self.lock().dirs.get(id).cloned()
    }

    fn with_kv<R>(&self, id: &str, f: impl FnOnce(&mut KvStore) -> R) -> Result<R, StorageError> {
        let mut g = self.lock();
        let inner = &mut *g;
        if let Some(s) = inner.kv.get_mut(id) {
            return Ok(f(s));
        }
        let (pid, dir) = inner
            .dirs
            .get_key_value(id)
            .map(|(k, v)| (k.clone(), v.clone()))
            .ok_or_else(|| StorageError::UnknownPlugin(id.to_owned()))?;
        let store = KvStore::open(&kv_path(&dir), self.max_kv_bytes)?;
        Ok(f(inner.kv.entry(pid).or_insert(store)))
    }

    fn with_player<R>(
        &self,
        id: &str,
        player: Uuid,
        f: impl FnOnce(&mut KvStore) -> R,
    ) -> Result<R, StorageError> {
        let mut g = self.lock();
        let inner = &mut *g;
        let (pid, dir) = inner
            .dirs
            .get_key_value(id)
            .map(|(k, v)| (k.clone(), v.clone()))
            .ok_or_else(|| StorageError::UnknownPlugin(id.to_owned()))?;
        let key = (pid, player);
        if !inner.players.contains_key(&key) {
            let store = KvStore::open(&player_data_path(&dir, player), MAX_PLAYER_DATA_BYTES)?;
            inner.players.insert(key.clone(), store);
        }
        match inner.players.get_mut(&key) {
            Some(s) => Ok(f(s)),
            None => Err(StorageError::UnknownPlugin(id.to_owned())),
        }
    }

    /// `storage.get`.
    pub fn kv_get(&self, id: &str, key: &str) -> Result<Option<Vec<u8>>, StorageError> {
        self.with_kv(id, |s| s.get(key).map(<[u8]>::to_vec))
    }

    /// `storage.set`.
    pub fn kv_set(&self, id: &str, key: &str, value: Vec<u8>) -> Result<(), StorageError> {
        self.with_kv(id, |s| s.set(key, value))?
    }

    /// `storage.delete`.
    pub fn kv_delete(&self, id: &str, key: &str) -> Result<bool, StorageError> {
        self.with_kv(id, |s| s.delete(key))
    }

    /// `storage.keys`.
    pub fn kv_keys(&self, id: &str, prefix: &str) -> Result<Vec<String>, StorageError> {
        self.with_kv(id, |s| s.keys(prefix))
    }

    /// `players.get-data`.
    pub fn player_get(
        &self,
        id: &str,
        player: Uuid,
        key: &str,
    ) -> Result<Option<Vec<u8>>, StorageError> {
        self.with_player(id, player, |s| s.get(key).map(<[u8]>::to_vec))
    }

    /// `players.set-data`.
    pub fn player_set(
        &self,
        id: &str,
        player: Uuid,
        key: &str,
        value: Vec<u8>,
    ) -> Result<(), StorageError> {
        self.with_player(id, player, |s| s.set(key, value))?
    }

    /// `players.delete-data`.
    pub fn player_delete(&self, id: &str, player: Uuid, key: &str) -> Result<bool, StorageError> {
        self.with_player(id, player, |s| s.delete(key))
    }

    /// Flush every dirty store; errors are returned (all stores are attempted).
    pub fn flush_all(&self) -> Vec<StorageError> {
        let mut g = self.lock();
        let inner = &mut *g;
        inner
            .kv
            .values_mut()
            .chain(inner.players.values_mut())
            .filter_map(|s| s.flush().err())
            .collect()
    }

    /// Flush and forget a player's stores (player quit).
    pub fn evict_player(&self, player: Uuid) -> Vec<StorageError> {
        let mut g = self.lock();
        let mut errors = Vec::new();
        let keys: Vec<_> = g
            .players
            .keys()
            .filter(|(_, p)| *p == player)
            .cloned()
            .collect();
        for k in keys {
            if let Some(mut s) = g.players.remove(&k)
                && let Err(e) = s.flush()
            {
                errors.push(e);
            }
        }
        errors
    }

    /// Flush and forget everything of a plugin (disable). The data folder stays registered
    /// so the plugin can be enabled again.
    pub fn unload_plugin(&self, id: &str) -> Vec<StorageError> {
        let mut g = self.lock();
        let mut errors = Vec::new();
        if let Some(mut s) = g.kv.remove(id)
            && let Err(e) = s.flush()
        {
            errors.push(e);
        }
        let keys: Vec<_> = g
            .players
            .keys()
            .filter(|(p, _)| &**p == id)
            .cloned()
            .collect();
        for k in keys {
            if let Some(mut s) = g.players.remove(&k)
                && let Err(e) = s.flush()
            {
                errors.push(e);
            }
        }
        errors
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kv_basic_and_persistence() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x").join("kv.rkv");
        let mut s = KvStore::open(&path, 1 << 20).unwrap();
        assert!(s.is_empty());
        s.set("home.a", b"1".to_vec()).unwrap();
        s.set("home.b", b"22".to_vec()).unwrap();
        s.set("other", vec![]).unwrap();
        assert_eq!(s.keys("home."), ["home.a", "home.b"]);
        assert_eq!(s.keys(""), ["home.a", "home.b", "other"]);
        assert_eq!(s.get("home.b"), Some(&b"22"[..]));
        assert!(s.delete("other"));
        assert!(!s.delete("other"));
        assert!(s.is_dirty());
        s.flush().unwrap();
        assert!(!s.is_dirty());
        let back = KvStore::open(&path, 1 << 20).unwrap();
        assert_eq!(back.len(), 2);
        assert_eq!(back.size_bytes(), s.size_bytes());
        assert_eq!(back.get("home.a"), Some(&b"1"[..]));
        // Emptying deletes the file.
        let mut back = back;
        back.delete("home.a");
        back.delete("home.b");
        back.flush().unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn kv_limits_and_corruption() {
        let mut s = KvStore::in_memory(20);
        assert!(s.set("", vec![]).is_err());
        assert!(s.set(&"k".repeat(MAX_KEY_BYTES + 1), vec![]).is_err());
        s.set("k", vec![0; 10]).unwrap();
        assert!(matches!(
            s.set("j", vec![0; 10]),
            Err(StorageError::Quota(_))
        ));
        s.set("k", vec![0; 19]).unwrap(); // replacing counts the old value out
        assert_eq!(s.size_bytes(), 20);
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kv.rkv");
        let mut s = KvStore::open(&path, 1000).unwrap();
        s.set("a", b"b".to_vec()).unwrap();
        s.flush().unwrap();
        let mut bytes = std::fs::read(&path).unwrap();
        let n = bytes.len();
        bytes[n - 10] ^= 0xff;
        std::fs::write(&path, &bytes).unwrap();
        assert!(matches!(
            KvStore::open(&path, 1000),
            Err(StorageError::Corrupt(_))
        ));
        std::fs::write(&path, b"RKV").unwrap();
        assert!(matches!(
            KvStore::open(&path, 1000),
            Err(StorageError::Corrupt(_))
        ));
    }

    #[test]
    fn storage_per_plugin_and_player() {
        let dir = tempfile::tempdir().unwrap();
        let layout = DataLayout::new(dir.path());
        let st = Storage::new(1 << 20);
        let a = PluginId::new("a").unwrap();
        st.register(&a, layout.data_dir("a", None)).unwrap();
        assert!(matches!(
            st.kv_get("b", "x"),
            Err(StorageError::UnknownPlugin(_))
        ));
        st.kv_set("a", "x", b"1".to_vec()).unwrap();
        assert_eq!(st.kv_get("a", "x").unwrap().unwrap(), b"1");
        let p = Uuid::new(1, 2);
        st.player_set("a", p, "coins", b"5".to_vec()).unwrap();
        assert_eq!(st.player_get("a", p, "coins").unwrap().unwrap(), b"5");
        assert!(st.flush_all().is_empty());
        let data = layout.data_dir("a", None);
        assert!(kv_path(&data).exists());
        assert!(player_data_path(&data, p).exists());
        assert!(st.evict_player(p).is_empty());
        assert_eq!(st.player_get("a", p, "coins").unwrap().unwrap(), b"5");
        assert!(st.player_delete("a", p, "coins").unwrap());
        assert!(st.unload_plugin("a").is_empty());
        assert!(!player_data_path(&data, p).exists());
        assert_eq!(st.kv_keys("a", "").unwrap(), ["x"]);
    }

    #[test]
    fn layout_and_default_config() {
        let dir = tempfile::tempdir().unwrap();
        let layout = DataLayout::new(dir.path());
        let d = layout.data_dir("homes", None);
        assert_eq!(d, dir.path().join("plugins").join("homes"));
        std::fs::create_dir_all(&d).unwrap();
        // Dev dir named like the id: data goes to .data.
        assert_eq!(layout.data_dir("homes", Some(&d)), d.join(".data"));
        assert!(install_default_config(&d, Some("a = 1\n")).unwrap());
        assert!(!install_default_config(&d, Some("a = 2\n")).unwrap());
        assert_eq!(
            std::fs::read_to_string(d.join("config.toml")).unwrap(),
            "a = 1\n"
        );
        assert!(!install_default_config(&layout.data_dir("x", None), None).unwrap());
    }
}
