//! The shared wasmtime [`Engine`], the epoch ticker, the linker and the compiled-component
//! cache (in memory by content hash, on disk through wasmtime's cache). See ADRs 0020 and 0023.

use crate::bindings::RusticPluginPermissionsEconomy;
use crate::bindings::exports::rustic::services::{chat_meta, economy, permission_provider};
use crate::config::HostConfig;
use crate::state::HostState;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError, RwLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};
use wasmtime::component::{Component, ComponentExportIndex, HasSelf, InstancePre, Linker};
use wasmtime::{
    Cache, CacheConfig, Config, Engine, InstanceAllocationStrategy, OptLevel,
    PoolingAllocationConfig, WasmBacktraceDetails,
};

/// The exported instance every plugin has.
pub(crate) const PLUGIN_IFACE: &str = "rustic:api/plugin@0.1.0";
/// The event handler instance.
pub(crate) const HANDLERS_IFACE: &str = "rustic:api/handlers@0.1.0";

/// What the guest-facing parts of the host share (held by every store, so it must not own
/// stores itself).
pub(crate) struct HostCore {
    pub config: HostConfig,
    pub engine: Engine,
    pub http: ureq::Agent,
    pub snapshot: RwLock<GameSnapshot>,
}

/// Game facts the async instances may read without a backend, refreshed from the main thread.
#[derive(Clone, Debug, Default)]
pub(crate) struct GameSnapshot {
    pub worlds: Vec<String>,
    pub behind_proxy: bool,
}

impl HostCore {
    pub(crate) fn snapshot(&self) -> GameSnapshot {
        self.snapshot
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub(crate) fn update_snapshot(&self, f: impl FnOnce(&mut GameSnapshot)) {
        f(&mut self
            .snapshot
            .write()
            .unwrap_or_else(PoisonError::into_inner));
    }

    /// Epoch ticks for a duration (at least 1).
    pub(crate) fn ticks(&self, d: Duration) -> u64 {
        let tick = self.config.epoch_tick.as_nanos().max(1);
        (d.as_nanos().div_ceil(tick)).clamp(1, u64::MAX as u128) as u64
    }
}

/// Build the engine for a config.
pub(crate) fn build_engine(cfg: &HostConfig) -> Result<Engine, String> {
    let mut c = Config::new();
    c.wasm_component_model(true)
        .consume_fuel(true)
        .epoch_interruption(true)
        .cranelift_opt_level(OptLevel::Speed)
        .parallel_compilation(true)
        .wasm_backtrace_details(if cfg.backtrace_details {
            WasmBacktraceDetails::Enable
        } else {
            WasmBacktraceDetails::Disable
        });
    if let Some(dir) = &cfg.cache_dir {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("cannot create the module cache {}: {e}", dir.display()))?;
        let mut cc = CacheConfig::new();
        cc.with_directory(dir);
        let cache = Cache::new(cc).map_err(|e| format!("module cache: {e:#}"))?;
        c.cache(Some(cache));
    }
    if let Some(p) = &cfg.pooling {
        let mut pc = PoolingAllocationConfig::new();
        pc.total_component_instances(p.total_component_instances)
            .total_core_instances(p.total_core_instances)
            .total_memories(p.total_memories)
            .total_tables(p.total_tables)
            .max_memory_size(p.max_memory_size)
            .max_core_instances_per_component(64)
            .max_memories_per_component(4)
            .max_tables_per_component(64);
        c.allocation_strategy(InstanceAllocationStrategy::Pooling(pc));
    }
    Engine::new(&c).map_err(|e| format!("cannot create the wasmtime engine: {e:#}"))
}

/// Build the linker: WASI p2 (sync) and every `rustic:api` / `rustic:services` import.
pub(crate) fn build_linker(engine: &Engine) -> Result<Linker<HostState>, String> {
    let mut linker = Linker::<HostState>::new(engine);
    wasmtime_wasi::p2::add_to_linker_sync(&mut linker)
        .map_err(|e| format!("linking WASI: {e:#}"))?;
    RusticPluginPermissionsEconomy::add_to_linker::<_, HasSelf<_>>(&mut linker, |s| s)
        .map_err(|e| format!("linking rustic:api: {e:#}"))?;
    Ok(linker)
}

/// Background thread advancing the engine's epoch every `tick`.
pub(crate) struct EpochTicker {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl EpochTicker {
    pub(crate) fn start(engine: Engine, tick: Duration) -> std::io::Result<Self> {
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let thread = std::thread::Builder::new()
            .name("rustic-epoch".into())
            .spawn(move || {
                while !flag.load(Ordering::Relaxed) {
                    std::thread::sleep(tick);
                    engine.increment_epoch();
                }
            })?;
        Ok(Self {
            stop,
            thread: Some(thread),
        })
    }
}

impl Drop for EpochTicker {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

/// Export indices of a compiled component, computed once (lookups are semver-aware: a
/// `@0.1.x` export matches the host's `@0.1.0` name).
pub(crate) struct ExportIndices {
    pub plugin: HashMap<&'static str, ComponentExportIndex>,
    pub handlers: HashMap<&'static str, ComponentExportIndex>,
    pub economy: Option<economy::GuestIndices>,
    pub permission_provider: Option<permission_provider::GuestIndices>,
    pub chat_meta: Option<chat_meta::GuestIndices>,
}

/// Functions of `rustic:api/plugin` the host knows.
pub(crate) const PLUGIN_FUNCS: [&str; 8] = [
    "on-load",
    "on-enable",
    "on-disable",
    "run-task",
    "run-job",
    "execute-command",
    "complete-command",
    "on-message",
];

impl ExportIndices {
    fn resolve(pre: &InstancePre<HostState>) -> Result<Self, String> {
        let component = pre.component();
        let plugin_idx = component
            .get_export_index(None, PLUGIN_IFACE)
            .ok_or_else(|| format!("the component does not export `{PLUGIN_IFACE}`"))?;
        let mut plugin = HashMap::new();
        for f in PLUGIN_FUNCS {
            if let Some(i) = component.get_export_index(Some(&plugin_idx), f) {
                plugin.insert(f, i);
            }
        }
        for required in ["on-load", "on-enable"] {
            if !plugin.contains_key(required) {
                return Err(format!("`{PLUGIN_IFACE}` lacks `{required}`"));
            }
        }
        let mut handlers = HashMap::new();
        if let Some(h) = component.get_export_index(None, HANDLERS_IFACE) {
            for f in crate::instance::HANDLER_EXPORTS {
                if let Some(i) = component.get_export_index(Some(&h), f) {
                    handlers.insert(*f, i);
                }
            }
        }
        let has = |iface: &str| component.get_export_index(None, iface).is_some();
        let bad = |iface: &str, e: wasmtime::Error| {
            format!("the component exports `{iface}` with a wrong shape: {e:#}")
        };
        use rusticmc_core::services::{CHAT_META_IFACE, ECONOMY_IFACE, PERMISSION_PROVIDER_IFACE};
        let economy = match has(ECONOMY_IFACE) {
            true => Some(economy::GuestIndices::new(pre).map_err(|e| bad(ECONOMY_IFACE, e))?),
            false => None,
        };
        let permission_provider = match has(PERMISSION_PROVIDER_IFACE) {
            true => Some(
                permission_provider::GuestIndices::new(pre)
                    .map_err(|e| bad(PERMISSION_PROVIDER_IFACE, e))?,
            ),
            false => None,
        };
        let chat_meta = match has(CHAT_META_IFACE) {
            true => Some(chat_meta::GuestIndices::new(pre).map_err(|e| bad(CHAT_META_IFACE, e))?),
            false => None,
        };
        Ok(Self {
            plugin,
            handlers,
            economy,
            permission_provider,
            chat_meta,
        })
    }
}

/// A compiled, linked component ready to instantiate.
pub(crate) struct Compiled {
    pub pre: InstancePre<HostState>,
    pub exports: ExportIndices,
    pub hash: String,
}

/// Counters of the compile path.
#[derive(Debug, Default)]
pub(crate) struct CompileStats {
    pub memory_hits: AtomicU64,
    pub compiles: AtomicU64,
    pub compile_nanos: AtomicU64,
}

/// In-memory cache of compiled components by SHA-256 of their bytes (bounded, oldest out).
/// Entries by digest plus insertion order.
type CacheEntries = (HashMap<[u8; 32], Arc<Compiled>>, VecDeque<[u8; 32]>);

pub(crate) struct ComponentCache {
    entries: Mutex<CacheEntries>,
    capacity: usize,
    pub stats: CompileStats,
}

impl ComponentCache {
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            entries: Mutex::new((HashMap::new(), VecDeque::new())),
            capacity: capacity.max(1),
            stats: CompileStats::default(),
        }
    }

    /// The compiled form of `bytes`: from memory, else compiled (wasmtime's on-disk cache makes
    /// an unchanged component cheap to "compile" after a restart).
    pub(crate) fn get_or_compile(
        &self,
        engine: &Engine,
        linker: &Linker<HostState>,
        bytes: &[u8],
    ) -> Result<Arc<Compiled>, String> {
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        if let Some(c) = self.lock().0.get(&digest) {
            self.stats.memory_hits.fetch_add(1, Ordering::Relaxed);
            return Ok(c.clone());
        }
        let t0 = Instant::now();
        let component = Component::new(engine, bytes).map_err(|e| format!("{e:#}"))?;
        let pre = linker
            .instantiate_pre(&component)
            .map_err(|e| format!("cannot link the component: {e:#}"))?;
        let exports = ExportIndices::resolve(&pre)?;
        let hash = digest
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        self.stats.compiles.fetch_add(1, Ordering::Relaxed);
        self.stats
            .compile_nanos
            .fetch_add(t0.elapsed().as_nanos() as u64, Ordering::Relaxed);
        let compiled = Arc::new(Compiled { pre, exports, hash });
        let mut g = self.lock();
        let (map, order) = &mut *g;
        if map.insert(digest, compiled.clone()).is_none() {
            order.push_back(digest);
        }
        while order.len() > self.capacity {
            if let Some(old) = order.pop_front() {
                map.remove(&old);
            }
        }
        Ok(compiled)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, CacheEntries> {
        self.entries.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Forget every compiled component (tests, benches measuring cold loads).
    pub(crate) fn clear(&self) {
        let mut g = self.lock();
        g.0.clear();
        g.1.clear();
    }
}
