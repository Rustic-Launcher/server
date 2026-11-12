//! [`WasmHost`] (the shared runtime: engine, linker, caches, epoch ticker, async pool) and
//! [`WasmLoader`], the core's [`PluginLoader`] for `.wasm` components.

use crate::config::HostConfig;
use crate::engine::{
    ComponentCache, EpochTicker, GameSnapshot, HostCore, build_engine, build_linker,
};
use crate::imports::services::PermissionBridge;
use crate::instance::WasmInstance;
use crate::output::LogSink;
use crate::plugin::WasmPlugin;
use crate::pool::{AsyncEntry, AsyncPool};
use crate::state::{CallClock, HostState, Limiter, PluginMetaInfo};
use rusticmc_core::manifest::FilesystemCapability;
use rusticmc_core::{LoadRequest, PluginId, PluginInstance, PluginLoader, Rustic, context};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError, RwLock};
use std::time::Duration;
use wasmtime::Engine;
use wasmtime::component::Linker;
use wasmtime_wasi::{FsPerms, WasiCtx};

/// Errors creating the host.
#[derive(Debug, thiserror::Error)]
pub enum HostError {
    /// Engine, cache or linker setup failed.
    #[error("{0}")]
    Setup(String),
    /// A thread could not be started.
    #[error("cannot start a host thread: {0}")]
    Thread(#[from] std::io::Error),
}

/// Statistics of a host (benches, diagnostics).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct HostStats {
    /// Components compiled (or fetched from wasmtime's disk cache) since start.
    pub compiles: u64,
    /// Loads served from the in-memory compiled-component cache.
    pub memory_cache_hits: u64,
    /// Total time spent in compilation (including disk-cache loads), in milliseconds.
    pub compile_ms: u64,
    /// Async tasks run.
    pub async_tasks_run: u64,
    /// Faults in async instances.
    pub async_faults: u64,
    /// Async tasks taken from the scheduler and not finished yet (0: the pool is idle).
    pub async_in_flight: u64,
}

pub(crate) struct HostShared {
    pub core: Arc<HostCore>,
    pub linker: Linker<HostState>,
    pub components: ComponentCache,
    pool: Mutex<Option<Arc<AsyncPool>>>,
    provider_installed: AtomicBool,
    _epoch: EpochTicker,
}

impl Drop for HostShared {
    fn drop(&mut self) {
        // Stop async workers before the engine goes away (the pool joins its threads).
        let pool = self
            .pool
            .get_mut()
            .unwrap_or_else(PoisonError::into_inner)
            .take();
        drop(pool);
    }
}

/// The WebAssembly plugin host. Cheap to clone (shared); `Send + Sync`.
///
/// ```no_run
/// use rusticmc_host::{HostConfig, WasmHost};
/// use rusticmc_core::{NullBackend, Rustic};
///
/// let dir = std::path::Path::new("server");
/// let host = WasmHost::new(HostConfig::for_server(dir))?;
/// let rt = Rustic::new(dir, host.loader())?;
/// host.install(&rt);
/// let mut backend = NullBackend;
/// rt.load_plugins(&mut backend);
/// rt.enable_all(&mut backend);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone)]
pub struct WasmHost {
    inner: Arc<HostShared>,
}

impl std::fmt::Debug for WasmHost {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WasmHost").finish_non_exhaustive()
    }
}

impl WasmHost {
    /// Create the engine (with the on-disk module cache when configured), the linker and the
    /// epoch ticker. The async pool starts with the first plugin load.
    pub fn new(config: HostConfig) -> Result<Self, HostError> {
        let engine = build_engine(&config).map_err(HostError::Setup)?;
        let linker = build_linker(&engine).map_err(HostError::Setup)?;
        let epoch = EpochTicker::start(engine.clone(), config.epoch_tick)?;
        let core = Arc::new(HostCore {
            http: crate::imports::http_agent(),
            engine,
            snapshot: RwLock::new(GameSnapshot::default()),
            config,
        });
        Ok(Self {
            inner: Arc::new(HostShared {
                core,
                linker,
                components: ComponentCache::new(64),
                pool: Mutex::new(None),
                provider_installed: AtomicBool::new(false),
                _epoch: epoch,
            }),
        })
    }

    /// A loader for [`Rustic::new`] / [`Rustic::builder`].
    pub fn loader(&self) -> Box<dyn PluginLoader> {
        Box::new(WasmLoader { host: self.clone() })
    }

    /// Install the permission-provider bridge on the runtime (a plugin providing
    /// `rustic:services/permission-provider` then answers every permission check). Done
    /// automatically at the first plugin load; calling it again is harmless.
    pub fn install(&self, rt: &Rustic) {
        rt.set_permission_provider(Box::new(PermissionBridge));
        self.inner.provider_installed.store(true, Ordering::Release);
    }

    /// The wasmtime engine.
    pub fn engine(&self) -> &Engine {
        &self.inner.core.engine
    }

    /// The host configuration.
    pub fn config(&self) -> &HostConfig {
        &self.inner.core.config
    }

    /// Counters.
    pub fn stats(&self) -> HostStats {
        let c = &self.inner.components.stats;
        let pool = self.pool();
        HostStats {
            compiles: c.compiles.load(Ordering::Relaxed),
            memory_cache_hits: c.memory_hits.load(Ordering::Relaxed),
            compile_ms: c.compile_nanos.load(Ordering::Relaxed) / 1_000_000,
            async_tasks_run: pool
                .as_ref()
                .map_or(0, |p| p.inner.stats.tasks_run.load(Ordering::Relaxed)),
            async_faults: pool
                .as_ref()
                .map_or(0, |p| p.inner.stats.faults.load(Ordering::Relaxed)),
            async_in_flight: pool
                .as_ref()
                .map_or(0, |p| p.inner.stats.in_flight.load(Ordering::SeqCst)),
        }
    }

    /// Drop every compiled component kept in memory (the next load compiles again, or reads
    /// wasmtime's disk cache).
    pub fn clear_memory_cache(&self) {
        self.inner.components.clear();
    }

    /// Compile (or fetch from the caches) a component and check it links, without
    /// instantiating it. Returns its SHA-256.
    pub fn precompile(&self, wasm: &[u8]) -> Result<String, String> {
        self.inner
            .components
            .get_or_compile(&self.inner.core.engine, &self.inner.linker, wasm)
            .map(|c| c.hash.clone())
    }

    fn pool(&self) -> Option<Arc<AsyncPool>> {
        self.inner
            .pool
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    fn pool_for(&self, req: &LoadRequest<'_>) -> Result<Arc<AsyncPool>, String> {
        let mut g = self
            .inner
            .pool
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(p) = g.as_ref() {
            return Ok(p.clone());
        }
        let cfg = &self.inner.core.config;
        let workers = cfg
            .async_workers
            .unwrap_or(req.shared.config().scheduler.async_workers as usize);
        let pool = Arc::new(
            AsyncPool::start(req.shared.clone(), workers, cfg.async_poll_interval)
                .map_err(|e| format!("cannot start the async pool: {e}"))?,
        );
        *g = Some(pool.clone());
        Ok(pool)
    }

    fn new_state(
        &self,
        req: &LoadRequest<'_>,
        meta: &Arc<PluginMetaInfo>,
        is_main: bool,
        cancel: Arc<AtomicBool>,
        deadline: Duration,
    ) -> Result<HostState, String> {
        let core = &self.inner.core;
        let id = &meta.id;
        let tag = if is_main {
            format!("[{id}]")
        } else {
            format!("[{id}/async]")
        };
        let stdout = LogSink::new(tag.clone(), log::Level::Info, core.config.output.clone());
        let stderr = LogSink::new(tag, log::Level::Warn, core.config.output.clone());
        let mut b = WasiCtx::builder();
        b.stdout(stdout).stderr(stderr.clone());
        b.allow_tcp(false)
            .allow_udp(false)
            .allow_ip_name_lookup(false)
            .socket_addr_check(|_, _| Box::pin(async { false }));
        if meta.capabilities.filesystem == FilesystemCapability::DataFolder {
            b.preopened_dir(req.data_dir, "/", FsPerms::ReadWrite)
                .map_err(|e| {
                    format!(
                        "cannot preopen the data folder {}: {e:#}",
                        req.data_dir.display()
                    )
                })?;
        }
        let limits = &req.limits;
        let cfg = &core.config;
        Ok(HostState {
            plugin: id.clone(),
            is_main,
            meta: meta.clone(),
            host: core.clone(),
            shared: req.shared.clone(),
            async_api: req.shared.async_api(id.clone()),
            wasi: b.build(),
            table: wasmtime::component::ResourceTable::new(),
            limiter: Limiter {
                memory_limit: (limits.memory_mb as usize) << 20,
                memory_total: 0,
                max_instances: cfg.max_instances,
                max_tables: cfg.max_tables,
                max_memories: cfg.max_memories,
                max_table_elements: cfg.max_table_elements,
                exceeded: None,
            },
            stderr,
            clock: CallClock::new(deadline, cancel),
            warned: Default::default(),
        })
    }

    fn load_plugin(&self, req: &LoadRequest<'_>) -> Result<WasmPlugin, String> {
        if !self.inner.provider_installed.load(Ordering::Acquire)
            && let Ok(rt) = context::runtime()
        {
            self.install(&rt);
        }
        let m = &req.package.manifest;
        let bytes = req.package.read_entry().map_err(|e| e.to_string())?;
        let core = &self.inner.core;
        let compiled =
            self.inner
                .components
                .get_or_compile(&core.engine, &self.inner.linker, &bytes)?;
        let pool = self.pool_for(req)?;
        let meta = Arc::new(PluginMetaInfo {
            id: m.id.clone(),
            name: m.name.clone(),
            version: m.version.to_string(),
            capabilities: m.capabilities.clone(),
        });
        let fuel = req.limits.fuel_per_call;
        let main_deadline = Duration::from_millis(req.limits.epoch_deadline_ms);
        let main_state = self.new_state(
            req,
            &meta,
            true,
            Arc::new(AtomicBool::new(false)),
            main_deadline,
        )?;
        let main = WasmInstance::instantiate(core, &compiled, main_state, fuel, main_deadline)
            .map_err(|e| format!("main instance: {e}"))?;
        let cancel = Arc::new(AtomicBool::new(false));
        let async_deadline = core.config.async_deadline;
        let async_state = self.new_state(req, &meta, false, cancel.clone(), async_deadline)?;
        let async_inst =
            WasmInstance::instantiate(core, &compiled, async_state, fuel, async_deadline)
                .map_err(|e| format!("async instance: {e}"))?;
        let entry = Arc::new(AsyncEntry::new(m.id.clone(), async_inst, cancel));
        pool.register(entry.clone());
        log::debug!(
            target: "rustic",
            "[{}] instantiated component {} ({} bytes)",
            m.id,
            &compiled.hash[..12],
            bytes.len()
        );
        Ok(WasmPlugin::new(
            m.id.clone(),
            main,
            entry,
            Arc::downgrade(&pool),
            core.clone(),
        ))
    }
}

/// The [`PluginLoader`] given to the core (see [`WasmHost::loader`]).
#[derive(Debug)]
pub struct WasmLoader {
    host: WasmHost,
}

impl WasmLoader {
    /// The host this loader belongs to.
    pub fn host(&self) -> &WasmHost {
        &self.host
    }
}

impl PluginLoader for WasmLoader {
    fn load(&mut self, req: &LoadRequest<'_>) -> Result<Box<dyn PluginInstance>, String> {
        self.host
            .load_plugin(req)
            .map(|p| Box::new(p) as Box<dyn PluginInstance>)
    }

    fn unloaded(&mut self, plugin: &PluginId) {
        // The instance's Drop already retired its async side; nothing else is cached per
        // plugin (compiled components stay in the content-addressed cache for reloads).
        log::debug!(target: "rustic", "[{plugin}] instances dropped");
    }
}
