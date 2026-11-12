//! The [`Rustic`] runtime façade, the host-facing [`PluginApi`] / [`AsyncApi`] handles and
//! the thread-safe [`SharedHandle`].
//!
//! `Rustic` is single-threaded (it lives on the server thread) and cheap to clone (`Rc`). Every
//! entry point that takes a `&mut dyn Backend` installs it in [`crate::context`] for the
//! duration of the call, so plugin imports reach it. No `RefCell` borrow is ever held across a
//! call into a plugin, so plugins can call back into the runtime freely; an attempt to
//! re-enter a plugin that is executing is detected and skipped.

use crate::archive::PluginPackage;
use crate::backend::Backend;
use crate::bus::{EventBus, ListenerInfo, ListenerList};
use crate::clock::{Clock, SystemClock};
use crate::commands::parse::Resolver;
use crate::commands::{CommandRegistry, CommandTreeExport, HorizonUsage, ParseOutcome};
use crate::config::ServerConfig;
use crate::context::{self, CallContext};
use crate::discovery::{DiscoveryError, ResolveError, discover, resolve, version_matches};
use crate::id::PluginId;
use crate::lifecycle::{
    LoadRequest, PluginFault, PluginInstance, PluginLoader, PluginSlot, PluginState,
};
use crate::manifest::Manifest;
use crate::model::*;
use crate::permission::{PermissionProvider, PermissionService, resolve as resolve_permission};
use crate::scheduler::{AsyncDue, Scheduler};
use crate::services::{PERMISSION_PROVIDER_IFACE, ServiceRegistry};
use crate::storage::{DataLayout, Storage, install_default_config};
use crate::watchdog::{CallKind, ExecutionTracker, Watchdog};
use rusticmc_permissions::PermissionEngine;
use rusticmc_timings::{SectionKind, Timings};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashSet};
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError, RwLock};
use std::time::{Duration, Instant};

/// Max nesting of event dispatches (custom events fired from handlers).
pub const MAX_DISPATCH_DEPTH: u32 = 8;

/// Version of this crate.
pub const CORE_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Errors of runtime operations.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    /// No such plugin.
    #[error("unknown plugin `{0}`")]
    UnknownPlugin(String),
    /// The operation does not apply in the plugin's state.
    #[error("plugin `{id}` is {state}: cannot {action}")]
    InvalidState {
        /// Plugin.
        id: String,
        /// Its state.
        state: PluginState,
        /// What was attempted.
        action: &'static str,
    },
    /// The plugin is executing right now (e.g. it asked to reload itself).
    #[error("plugin `{0}` is executing; try again after its call returns")]
    Busy(String),
    /// Dependency problem.
    #[error("plugin `{id}`: {error}")]
    Dependency {
        /// Plugin.
        id: String,
        /// Problem.
        error: ResolveError,
    },
    /// Loading (instantiation or `on-load`) failed.
    #[error("plugin `{id}` failed to load: {message}")]
    Load {
        /// Plugin.
        id: String,
        /// Why.
        message: String,
    },
    /// `on-enable` failed.
    #[error("plugin `{id}` failed to enable: {message}")]
    Enable {
        /// Plugin.
        id: String,
        /// Why.
        message: String,
    },
    /// Reading the package failed.
    #[error(transparent)]
    Archive(#[from] crate::archive::ArchiveError),
    /// Server config problem.
    #[error(transparent)]
    Config(#[from] crate::config::ConfigError),
    /// `permissions.toml` problem.
    #[error(transparent)]
    Permissions(#[from] rusticmc_permissions::PermissionsError),
    /// Storage problem.
    #[error(transparent)]
    Storage(#[from] crate::storage::StorageError),
    /// File system problem.
    #[error("{path}: {source}")]
    Io {
        /// Path.
        path: PathBuf,
        /// Cause.
        #[source]
        source: std::io::Error,
    },
}

/// Why a call into a plugin did not return a value.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum CallError {
    /// No such plugin, or not enabled.
    #[error("the plugin is not enabled")]
    NotEnabled,
    /// It is executing further up the stack (re-entry refused).
    #[error("the plugin is currently executing")]
    Busy,
    /// It faulted (and was disabled).
    #[error("the plugin faulted: {0}")]
    Fault(PluginFault),
}

/// Result of a command dispatch. Feedback for every outcome but `Executed` and
/// `UnknownCommand` has already been sent to the sender.
#[derive(Clone, Debug, PartialEq)]
pub enum CommandOutcome {
    /// The command ran.
    Executed,
    /// No Rustic command has this label (let the engine handle it).
    UnknownCommand,
    /// The sender lacks the permission.
    NoPermission,
    /// The line did not match any usage; the message (error + usage lines) was sent.
    Usage(String),
    /// Console sender for a players-only command.
    PlayerOnly,
    /// The plugin returned an error text (shown in red).
    PluginError(Text),
    /// The plugin faulted or could not be called.
    Fault(String),
}

/// What [`Rustic::load_dir`] did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct LoadReport {
    /// Plugins now loaded (in load order).
    pub loaded: Vec<PluginId>,
    /// Refused or failed: (id or path, reason).
    pub failed: Vec<(String, String)>,
    /// Non-fatal notes.
    pub warnings: Vec<String>,
}

/// What [`Rustic::tick`] did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TickReport {
    /// Tick number (starts at 1).
    pub tick: u64,
    /// Messages delivered to `on-message`.
    pub messages: usize,
    /// Tasks run.
    pub tasks_run: usize,
    /// Due tasks rolled over to the next tick (budget exceeded).
    pub tasks_rolled: usize,
}

/// Public view of a plugin.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PluginInfo {
    /// Id.
    pub id: PluginId,
    /// Display name.
    pub name: String,
    /// Version.
    pub version: String,
    /// State.
    pub state: PluginState,
    /// Why it failed / was disabled.
    pub failure: Option<String>,
    /// Authors.
    pub authors: Vec<String>,
    /// Description.
    pub description: Option<String>,
    /// Licence.
    pub license: String,
    /// Package path.
    pub source: Option<PathBuf>,
    /// Required dependencies.
    pub required: Vec<String>,
    /// Optional dependencies.
    pub optional: Vec<String>,
    /// Data folder.
    pub data_dir: Option<PathBuf>,
}

/// Thread-safe part of the runtime, shared with the host's async instance pool.
pub struct Shared {
    config: ServerConfig,
    layout: DataLayout,
    scheduler: Scheduler,
    storage: Storage,
    services: ServiceRegistry,
    timings: Timings,
    tracker: Arc<ExecutionTracker>,
    tick: AtomicU64,
    clock: Arc<dyn Clock>,
    enabled: RwLock<HashSet<PluginId>>,
    faults: Mutex<Vec<(PluginId, PluginFault)>>,
}

/// A cloneable, `Send + Sync` handle to the shared state.
#[derive(Clone)]
pub struct SharedHandle(Arc<Shared>);

impl std::fmt::Debug for SharedHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedHandle").finish_non_exhaustive()
    }
}

impl SharedHandle {
    /// Server config.
    pub fn config(&self) -> &ServerConfig {
        &self.0.config
    }
    /// Data layout.
    pub fn layout(&self) -> &DataLayout {
        &self.0.layout
    }
    /// Scheduler.
    pub fn scheduler(&self) -> &Scheduler {
        &self.0.scheduler
    }
    /// Storage.
    pub fn storage(&self) -> &Storage {
        &self.0.storage
    }
    /// Service registry.
    pub fn services(&self) -> &ServiceRegistry {
        &self.0.services
    }
    /// Timings.
    pub fn timings(&self) -> &Timings {
        &self.0.timings
    }
    /// Execution tracker (watchdog).
    pub fn tracker(&self) -> &Arc<ExecutionTracker> {
        &self.0.tracker
    }
    /// Ticks since start.
    pub fn current_tick(&self) -> u64 {
        self.0.tick.load(Ordering::Relaxed)
    }
    /// The runtime's clock.
    pub fn now(&self) -> Instant {
        self.0.clock.now()
    }
    /// Whether a plugin is enabled (async pool: skip tasks of disabled plugins).
    pub fn is_enabled(&self, plugin: &str) -> bool {
        self.0
            .enabled
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .contains(plugin)
    }
    /// Report a fault in a plugin's async instance; the plugin is disabled at the next tick.
    pub fn report_fault(&self, plugin: &PluginId, fault: PluginFault) {
        self.0
            .faults
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((plugin.clone(), fault));
    }
    /// Async tasks due now (marked running): run them, then call
    /// [`SharedHandle::async_task_finished`].
    pub fn due_async_tasks(&self, now: Instant) -> Vec<AsyncDue> {
        self.0
            .scheduler
            .due_async(now)
            .into_iter()
            .filter(|d| self.is_enabled(&d.plugin))
            .collect()
    }
    /// Completion bookkeeping of an async task.
    pub fn async_task_finished(&self, task: TaskId) {
        self.0.scheduler.async_finished(task);
    }
    /// The import surface of a plugin's async instance.
    pub fn async_api(&self, plugin: PluginId) -> AsyncApi {
        AsyncApi {
            shared: self.clone(),
            plugin,
        }
    }
}

fn log_plugin(plugin: &str, level: LogLevel, message: &str) {
    log::log!(target: "rustic", level.into(), "[{plugin}] {message}");
}

/// What a plugin's **async instance** can call (thread-safe; the host keeps one per async
/// instance). Calls touching the world are not here: they return `main-thread-only`.
#[derive(Clone, Debug)]
pub struct AsyncApi {
    shared: SharedHandle,
    plugin: PluginId,
}

impl AsyncApi {
    /// The plugin.
    pub fn id(&self) -> &PluginId {
        &self.plugin
    }
    /// `logging.log`.
    pub fn log(&self, level: LogLevel, message: &str) {
        log_plugin(&self.plugin, level, message);
    }
    /// `scheduler.run-later` (a main-thread task).
    pub fn run_later(&self, delay: u64) -> TaskId {
        self.shared
            .scheduler()
            .run_later(&self.plugin, self.shared.current_tick(), delay)
    }
    /// `scheduler.run-timer`.
    pub fn run_timer(&self, delay: u64, period: u64) -> TaskId {
        self.shared
            .scheduler()
            .run_timer(&self.plugin, self.shared.current_tick(), delay, period)
    }
    /// `scheduler.run-async`.
    pub fn run_async(&self, delay_ms: u64) -> TaskId {
        self.shared.scheduler().run_async(
            &self.plugin,
            self.shared.now(),
            Duration::from_millis(delay_ms),
        )
    }
    /// `scheduler.run-async-job`.
    pub fn run_async_job(&self, job: &str, payload: Vec<u8>, delay_ms: u64) -> TaskId {
        self.shared.scheduler().run_async_job(
            &self.plugin,
            self.shared.now(),
            job.to_owned(),
            payload,
            Duration::from_millis(delay_ms),
        )
    }
    /// `scheduler.run-async-timer`.
    pub fn run_async_timer(&self, delay_ms: u64, period_ms: u64) -> TaskId {
        self.shared.scheduler().run_async_timer(
            &self.plugin,
            self.shared.now(),
            Duration::from_millis(delay_ms),
            Duration::from_millis(period_ms),
        )
    }
    /// `scheduler.cancel`.
    pub fn cancel(&self, task: TaskId) {
        self.shared.scheduler().cancel(&self.plugin, task);
    }
    /// `scheduler.post-to-main`.
    pub fn post_to_main(&self, topic: &str, payload: Vec<u8>) {
        self.shared
            .scheduler()
            .post_to_main(&self.plugin, topic.to_owned(), payload);
    }
    /// `scheduler.current-tick`.
    pub fn current_tick(&self) -> u64 {
        self.shared.current_tick()
    }
    /// `scheduler.is-main`: false.
    pub fn is_main(&self) -> bool {
        false
    }
    /// `services.provider`.
    pub fn service_provider(&self, iface: &str) -> Option<String> {
        self.shared
            .services()
            .provider(iface)
            .map(|p| p.to_string())
    }
    /// `storage.get`.
    pub fn storage_get(&self, key: &str) -> Option<Vec<u8>> {
        self.shared
            .storage()
            .kv_get(&self.plugin, key)
            .unwrap_or_else(|e| {
                log_plugin(&self.plugin, LogLevel::Warn, &format!("storage.get: {e}"));
                None
            })
    }
    /// `storage.set` (errors are logged: the WIT function returns nothing).
    pub fn storage_set(&self, key: &str, value: Vec<u8>) {
        if let Err(e) = self.shared.storage().kv_set(&self.plugin, key, value) {
            log_plugin(
                &self.plugin,
                LogLevel::Warn,
                &format!("storage.set `{key}` refused: {e}"),
            );
        }
    }
    /// `storage.delete`.
    pub fn storage_delete(&self, key: &str) -> bool {
        self.shared
            .storage()
            .kv_delete(&self.plugin, key)
            .unwrap_or(false)
    }
    /// `storage.keys`.
    pub fn storage_keys(&self, prefix: &str) -> Vec<String> {
        self.shared
            .storage()
            .kv_keys(&self.plugin, prefix)
            .unwrap_or_default()
    }
    /// `players.get-data`.
    pub fn player_data_get(&self, player: Uuid, key: &str) -> Option<Vec<u8>> {
        self.shared
            .storage()
            .player_get(&self.plugin, player, key)
            .unwrap_or_else(|e| {
                log_plugin(
                    &self.plugin,
                    LogLevel::Warn,
                    &format!("players.get-data: {e}"),
                );
                None
            })
    }
    /// `players.set-data`.
    pub fn player_data_set(&self, player: Uuid, key: &str, value: Vec<u8>) {
        if let Err(e) = self
            .shared
            .storage()
            .player_set(&self.plugin, player, key, value)
        {
            log_plugin(
                &self.plugin,
                LogLevel::Warn,
                &format!("players.set-data `{key}` refused: {e}"),
            );
        }
    }
    /// `players.delete-data`.
    pub fn player_data_delete(&self, player: Uuid, key: &str) -> bool {
        self.shared
            .storage()
            .player_delete(&self.plugin, player, key)
            .unwrap_or(false)
    }
    /// The plugin's data folder.
    pub fn data_dir(&self) -> Option<PathBuf> {
        self.shared.storage().data_dir(&self.plugin)
    }
}

struct PluginRecord {
    slot: Rc<PluginSlot>,
    package: Option<PluginPackage>,
    failure: Option<String>,
}

#[derive(Default)]
struct PluginTable {
    records: BTreeMap<PluginId, PluginRecord>,
    order: Vec<PluginId>,
    orphan_errors: Vec<DiscoveryError>,
}

/// Teardown flavour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Teardown {
    /// Explicit disable, reload, dependency, shutdown: `on-disable` is called.
    Graceful,
    /// Fault or failed load/enable: no `on-disable`, state `Failed`.
    Failed,
}

pub(crate) struct Inner {
    shared: SharedHandle,
    loader: RefCell<Box<dyn PluginLoader>>,
    plugins: RefCell<PluginTable>,
    bus: RefCell<EventBus>,
    commands: RefCell<CommandRegistry>,
    permissions: RefCell<PermissionService>,
    perm_provider: RefCell<Option<Box<dyn PermissionProvider>>>,
    next_handler: Cell<u32>,
    next_command: Cell<u32>,
    dispatch_depth: Cell<u32>,
    deferred: RefCell<Vec<Rc<PluginSlot>>>,
    last_flush_tick: Cell<u64>,
    last_budget_warning: Cell<u64>,
    rng: Cell<u64>,
    commands_dirty: Cell<bool>,
    warned_reentry: RefCell<HashSet<(PluginId, String)>>,
    shut_down: Cell<bool>,
}

/// Builds a [`Rustic`].
pub struct RusticBuilder {
    server_dir: PathBuf,
    loader: Box<dyn PluginLoader>,
    config: Option<ServerConfig>,
    clock: Option<Arc<dyn Clock>>,
    permissions: Option<PermissionEngine>,
}

impl RusticBuilder {
    /// Use this config instead of loading `<server>/rustic.toml`.
    pub fn config(mut self, config: ServerConfig) -> Self {
        self.config = Some(config);
        self
    }
    /// Use this clock (tests: [`crate::clock::FakeClock`]).
    pub fn clock(mut self, clock: Arc<dyn Clock>) -> Self {
        self.clock = Some(clock);
        self
    }
    /// Use this permission engine instead of loading `<server>/permissions.toml`.
    pub fn permissions(mut self, engine: PermissionEngine) -> Self {
        self.permissions = Some(engine);
        self
    }

    /// Build: loads (creating if needed) `rustic.toml` and `permissions.toml` unless given.
    pub fn build(self) -> Result<Rustic, CoreError> {
        let layout = DataLayout::new(self.server_dir.clone());
        let config = match self.config {
            Some(c) => {
                c.validate()?;
                c
            }
            None => ServerConfig::load_or_create(&self.server_dir)?,
        };
        let engine = match self.permissions {
            Some(e) => e,
            None => PermissionEngine::load_or_create(&layout.permissions_file())?,
        };
        let shared = SharedHandle(Arc::new(Shared {
            timings: Timings::new(config.timings.enabled),
            storage: Storage::new(config.storage.max_kv_bytes),
            config,
            layout,
            scheduler: Scheduler::new(),
            services: ServiceRegistry::new(),
            tracker: Arc::new(ExecutionTracker::new()),
            tick: AtomicU64::new(0),
            clock: self.clock.unwrap_or_else(|| Arc::new(SystemClock)),
            enabled: RwLock::new(HashSet::new()),
            faults: Mutex::new(Vec::new()),
        }));
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0x9E37_79B9_7F4A_7C15)
            | 1;
        let rt = Rustic {
            inner: Rc::new(Inner {
                shared,
                loader: RefCell::new(self.loader),
                plugins: RefCell::new(PluginTable::default()),
                bus: RefCell::new(EventBus::default()),
                commands: RefCell::new(CommandRegistry::new()),
                permissions: RefCell::new(PermissionService::new(engine)),
                perm_provider: RefCell::new(None),
                next_handler: Cell::new(1),
                next_command: Cell::new(1),
                dispatch_depth: Cell::new(0),
                deferred: RefCell::new(Vec::new()),
                last_flush_tick: Cell::new(0),
                last_budget_warning: Cell::new(0),
                rng: Cell::new(seed),
                commands_dirty: Cell::new(false),
                warned_reentry: RefCell::new(HashSet::new()),
                shut_down: Cell::new(false),
            }),
        };
        rt.register_builtins();
        Ok(rt)
    }
}

/// The Rustic runtime. Lives on the server thread; clone it freely (it is an `Rc`).
#[derive(Clone)]
pub struct Rustic {
    pub(crate) inner: Rc<Inner>,
}

impl std::fmt::Debug for Rustic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Rustic").finish_non_exhaustive()
    }
}

/// Resolver over the current backend.
struct BackendResolver<'a> {
    rng: &'a Cell<u64>,
}

impl Resolver for BackendResolver<'_> {
    fn player_by_name(&mut self, name: &str) -> Option<Uuid> {
        context::backend_call(|b| b.players_by_name(name))
            .ok()
            .flatten()
    }
    fn online_players(&mut self) -> Vec<Uuid> {
        context::backend_call(|b| b.players_online()).unwrap_or_default()
    }
    fn player_name(&mut self, player: Uuid) -> Option<String> {
        context::backend_call(|b| b.players_name(player))
            .ok()
            .flatten()
    }
    fn is_online(&mut self, player: Uuid) -> bool {
        context::backend_call(|b| b.players_is_online(player)).unwrap_or(false)
    }
    fn location(&mut self, player: Uuid) -> Option<Location> {
        context::backend_call(|b| b.players_get_location(player)).ok()
    }
    fn world_spawn(&mut self, world: &str) -> Option<Location> {
        context::backend_call(|b| b.worlds_spawn(world)).ok()
    }
    fn worlds(&mut self) -> Option<Vec<String>> {
        context::backend_call(|b| b.worlds_all()).ok()
    }
    fn random_below(&mut self, n: usize) -> usize {
        // xorshift64*
        let mut x = self.rng.get();
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng.set(x);
        (x.wrapping_mul(0x2545_F491_4F6C_DD1D) % n.max(1) as u64) as usize
    }
}

fn menu_of(event: &Event) -> Option<MenuId> {
    match event {
        Event::InventoryClick(c) => c.menu,
        Event::InventoryClose(c) => c.menu,
        _ => None,
    }
}

fn without_menu(event: &Event) -> Event {
    let mut e = event.clone();
    match &mut e {
        Event::InventoryClick(c) => c.menu = None,
        Event::InventoryClose(c) => c.menu = None,
        _ => {}
    }
    e
}

fn red(text: &Text) -> Text {
    Text::from_json_unchecked(format!(
        "{{\"text\":\"\",\"color\":\"red\",\"extra\":[{}]}}",
        text.as_json()
    ))
}

macro_rules! fire_fns {
    ($($fn_name:ident => $ty:ident),* $(,)?) => {
        impl Rustic {
            $(
                #[doc = concat!("Fire a [`", stringify!($ty), "`] and get it back with the plugins' changes applied.")]
                pub fn $fn_name(&self, event: $ty, backend: &mut dyn Backend) -> $ty {
                    self.fire_record(event, backend)
                }
            )*
        }
    };
}

fire_fns! {
    fire_player_pre_login => PlayerPreLogin,
    fire_player_join => PlayerJoin,
    fire_player_quit => PlayerQuit,
    fire_player_chat => PlayerChat,
    fire_player_command => PlayerCommand,
    fire_server_command => ServerCommand,
    fire_player_move => PlayerMove,
    fire_player_teleport => PlayerTeleport,
    fire_player_change_world => PlayerChangeWorld,
    fire_block_break => BlockBreak,
    fire_block_place => BlockPlace,
    fire_player_interact => PlayerInteract,
    fire_player_interact_entity => PlayerInteractEntity,
    fire_entity_damage => EntityDamage,
    fire_player_death => PlayerDeath,
    fire_entity_death => EntityDeath,
    fire_player_respawn => PlayerRespawn,
    fire_inventory_click => InventoryClick,
    fire_inventory_close => InventoryClose,
    fire_player_game_mode_change => PlayerGameModeChange,
    fire_server_list_ping => ServerListPing,
    fire_plugin_message => PluginMessage,
    fire_chunk_load => ChunkLoad,
    fire_chunk_unload => ChunkUnload,
    fire_custom_event => CustomEvent,
}

impl Rustic {
    /// A builder for a server directory and a loader.
    pub fn builder(server_dir: impl Into<PathBuf>, loader: Box<dyn PluginLoader>) -> RusticBuilder {
        RusticBuilder {
            server_dir: server_dir.into(),
            loader,
            config: None,
            clock: None,
            permissions: None,
        }
    }

    /// A runtime for `server_dir` with config and permissions loaded from (or created in) it.
    pub fn new(
        server_dir: impl Into<PathBuf>,
        loader: Box<dyn PluginLoader>,
    ) -> Result<Rustic, CoreError> {
        Self::builder(server_dir, loader).build()
    }

    pub(crate) fn from_inner(inner: Rc<Inner>) -> Self {
        Rustic { inner }
    }

    fn sh(&self) -> &Shared {
        &self.inner.shared.0
    }

    /// The thread-safe shared state (give it to the async instance pool).
    pub fn shared(&self) -> SharedHandle {
        self.inner.shared.clone()
    }

    /// The server config.
    pub fn config(&self) -> &ServerConfig {
        &self.sh().config
    }

    /// The data layout.
    pub fn layout(&self) -> &DataLayout {
        &self.sh().layout
    }

    /// Timings.
    pub fn timings(&self) -> &Timings {
        &self.sh().timings
    }

    /// Ticks since start.
    pub fn current_tick(&self) -> u64 {
        self.sh().tick.load(Ordering::Relaxed)
    }

    /// The execution tracker (for a watchdog).
    pub fn tracker(&self) -> Arc<ExecutionTracker> {
        self.sh().tracker.clone()
    }

    /// Start a watchdog thread per the config (`None` when `watchdog.seconds = 0`). It logs
    /// stuck calls; the host's epoch deadline is what actually interrupts them.
    pub fn spawn_watchdog(&self) -> std::io::Result<Option<Watchdog>> {
        let secs = self.config().watchdog.seconds;
        if secs == 0 {
            return Ok(None);
        }
        Watchdog::spawn(
            self.tracker(),
            Duration::from_secs(secs),
            Duration::from_millis(250),
            Watchdog::log_stuck,
        )
        .map(Some)
    }

    /// The import surface of a plugin's main instance. Main-instance host functions normally
    /// get it from [`crate::context::api`] (the plugin currently executing).
    pub fn api(&self, plugin: impl Into<PluginId>) -> PluginApi {
        PluginApi {
            rt: self.clone(),
            plugin: plugin.into(),
        }
    }

    /// Install the host's bridge to permission-provider plugins.
    pub fn set_permission_provider(&self, provider: Box<dyn PermissionProvider>) {
        match self.inner.perm_provider.try_borrow_mut() {
            Ok(mut p) => *p = Some(provider),
            Err(_) => {
                log::error!(target: "rustic", "cannot install the permission provider during a permission check")
            }
        }
    }

    /// Run `f` on the permission service (declared nodes, default engine).
    pub fn with_permissions<R>(&self, f: impl FnOnce(&mut PermissionService) -> R) -> Option<R> {
        self.inner
            .permissions
            .try_borrow_mut()
            .ok()
            .map(|mut p| f(&mut p))
    }

    /// Re-read `<server>/permissions.toml` into the default engine.
    pub fn reload_permissions(&self) -> Result<(), CoreError> {
        let engine = PermissionEngine::load_or_create(&self.layout().permissions_file())?;
        self.with_permissions(|p| p.set_engine(engine));
        Ok(())
    }

    // ------------------------------------------------------------------ entry plumbing

    fn enter<R>(&self, backend: &mut dyn Backend, f: impl FnOnce() -> R) -> R {
        context::with_runtime(&self.inner, || {
            context::with_backend(backend, || {
                let r = f();
                self.settle();
                r
            })
        })
    }

    fn settle(&self) {
        if context::call_depth() == 0 {
            self.process_deferred();
        }
        if self.inner.commands_dirty.replace(false) {
            let _ = context::backend_mut(|b| b.commands_changed());
        }
    }

    fn slot(&self, id: &str) -> Option<Rc<PluginSlot>> {
        self.inner
            .plugins
            .borrow()
            .records
            .get(id)
            .map(|r| r.slot.clone())
    }

    fn manifest(&self, id: &str) -> Option<Manifest> {
        self.inner
            .plugins
            .borrow()
            .records
            .get(id)
            .and_then(|r| r.package.as_ref().map(|p| p.manifest.clone()))
    }

    /// Call into a plugin instance with the context, tracker, timings and fault handling set
    /// up. Faults disable the plugin (and its dependents) before this returns.
    fn call_slot<R>(
        &self,
        slot: &Rc<PluginSlot>,
        kind: CallKind,
        section: SectionKind,
        name: &str,
        f: impl FnOnce(&mut dyn PluginInstance) -> Result<R, PluginFault>,
    ) -> Result<R, CallError> {
        let res = {
            let Ok(mut guard) = slot.instance.try_borrow_mut() else {
                return Err(CallError::Busy);
            };
            let Some(inst) = guard.as_mut() else {
                return Err(CallError::NotEnabled);
            };
            let sh = self.sh();
            let prev = sh.tracker.enter(slot.index, kind, Instant::now());
            let t0 = sh.timings.start();
            let res = context::with_call(
                CallContext {
                    plugin_id: slot.id.clone(),
                    is_main: true,
                },
                || std::panic::catch_unwind(AssertUnwindSafe(|| f(inst.as_mut()))),
            );
            sh.timings.finish(t0, &slot.id, section, name);
            sh.tracker.exit(prev);
            res
        };
        match res.unwrap_or_else(|p| Err(PluginFault::from_panic(p))) {
            Ok(v) => Ok(v),
            Err(fault) => {
                self.handle_fault(&slot.id, kind, &fault);
                Err(CallError::Fault(fault))
            }
        }
    }

    /// Call into a plugin on behalf of another (service calls, permission provider). The host
    /// downcasts with [`PluginInstance::as_any_mut`]. Refused if the plugin is not enabled or is
    /// executing; a fault disables it.
    pub fn with_plugin_instance<R>(
        &self,
        plugin: &str,
        f: impl FnOnce(&mut dyn PluginInstance) -> Result<R, PluginFault>,
    ) -> Result<R, CallError> {
        let slot = self
            .slot(plugin)
            .filter(|s| s.is_enabled())
            .ok_or(CallError::NotEnabled)?;
        context::with_runtime(&self.inner, || {
            let r = self.call_slot(&slot, CallKind::Service, SectionKind::Service, "service", f);
            if context::call_depth() == 0 {
                self.process_deferred();
            }
            r
        })
    }

    fn handle_fault(&self, id: &PluginId, kind: CallKind, fault: &PluginFault) {
        match fault.backtrace() {
            Some(bt) => log::error!(
                target: "rustic",
                "[{id}] {} in {}: {} — the plugin is disabled\n{bt}",
                fault.kind(),
                kind.label(),
                fault.message()
            ),
            None => log::error!(
                target: "rustic",
                "[{id}] {} in {}: {} — the plugin is disabled",
                fault.kind(),
                kind.label(),
                fault.message()
            ),
        }
        self.disable_cascade(
            id,
            &format!("{} in {}: {}", fault.kind(), kind.label(), fault.message()),
            Teardown::Failed,
        );
    }

    /// Enabled or loaded plugins that require `id`, transitively, in reverse load order.
    fn active_dependents(&self, id: &str) -> Vec<PluginId> {
        let table = self.inner.plugins.borrow();
        let mut set: Vec<PluginId> = vec![PluginId::new_unchecked(id)];
        let mut changed = true;
        while changed {
            changed = false;
            for (pid, rec) in &table.records {
                if set.contains(pid) || !rec.slot.is_active() {
                    continue;
                }
                if let Some(pkg) = &rec.package
                    && pkg
                        .manifest
                        .dependencies
                        .required
                        .keys()
                        .any(|d| set.contains(d))
                {
                    set.push(pid.clone());
                    changed = true;
                }
            }
        }
        set.remove(0);
        let pos = |p: &PluginId| {
            table
                .order
                .iter()
                .position(|x| x == p)
                .unwrap_or(usize::MAX)
        };
        set.sort_by_key(|p| std::cmp::Reverse(pos(p)));
        set
    }

    fn disable_cascade(&self, id: &PluginId, reason: &str, mode: Teardown) -> Vec<PluginId> {
        let mut done = Vec::new();
        for d in self.active_dependents(id) {
            let why = format!("required dependency `{id}` was disabled ({reason})");
            log::warn!(target: "rustic", "[{d}] disabling: {why}");
            self.teardown(&d, &why, Teardown::Graceful);
            done.push(d);
        }
        self.teardown(id, reason, mode);
        done.push(id.clone());
        done
    }

    fn teardown(&self, id: &PluginId, reason: &str, mode: Teardown) {
        let Some(slot) = self.slot(id) else {
            return;
        };
        let state = slot.state.get();
        if !matches!(state, PluginState::Loaded | PluginState::Enabled) {
            return;
        }
        let was_enabled = state == PluginState::Enabled;
        slot.state.set(PluginState::Disabling);
        self.sh()
            .enabled
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(id);
        let call_on_disable = mode == Teardown::Graceful && was_enabled;
        let free = slot.instance.try_borrow_mut().is_ok();
        if call_on_disable && free {
            let _ = self.call_slot(
                &slot,
                CallKind::Disable,
                SectionKind::Lifecycle,
                "on-disable",
                |i| i.on_disable(),
            );
        }
        self.release(&slot);
        let final_state = match mode {
            Teardown::Graceful => PluginState::Disabled,
            Teardown::Failed => PluginState::Failed,
        };
        slot.state.set(final_state);
        if let Some(rec) = self.inner.plugins.borrow_mut().records.get_mut(id) {
            rec.failure = Some(reason.to_owned());
        }
        if free {
            self.drop_instance(&slot);
        } else {
            slot.pending_on_disable.set(call_on_disable);
            self.inner.deferred.borrow_mut().push(slot.clone());
        }
        match mode {
            Teardown::Graceful => log::info!(target: "rustic", "[{id}] disabled ({reason})"),
            Teardown::Failed => log::warn!(target: "rustic", "[{id}] failed ({reason})"),
        }
    }

    fn drop_instance(&self, slot: &Rc<PluginSlot>) {
        let inst = slot
            .instance
            .try_borrow_mut()
            .ok()
            .and_then(|mut i| i.take());
        if inst.is_some() {
            drop(inst);
            match self.inner.loader.try_borrow_mut() {
                Ok(mut l) => l.unloaded(&slot.id),
                Err(_) => {
                    log::warn!(target: "rustic", "[{}] loader busy: unloaded() not called", slot.id)
                }
            }
        }
    }

    fn process_deferred(&self) {
        let list: Vec<Rc<PluginSlot>> = std::mem::take(&mut *self.inner.deferred.borrow_mut());
        let mut keep = Vec::new();
        for slot in list {
            if slot.instance.try_borrow_mut().is_err() {
                keep.push(slot);
                continue;
            }
            if slot.pending_on_disable.replace(false) {
                let _ = self.call_slot(
                    &slot,
                    CallKind::Disable,
                    SectionKind::Lifecycle,
                    "on-disable",
                    |i| i.on_disable(),
                );
                self.release(&slot);
            }
            self.drop_instance(&slot);
        }
        self.inner.deferred.borrow_mut().extend(keep);
    }

    /// Remove everything a plugin registered and release what it owns in the game.
    fn release(&self, slot: &Rc<PluginSlot>) {
        let id = &slot.id;
        if let Ok(mut bus) = self.inner.bus.try_borrow_mut() {
            bus.remove_plugin(id);
        }
        if let Ok(mut cmds) = self.inner.commands.try_borrow_mut()
            && cmds.remove_plugin(id) > 0
        {
            self.inner.commands_dirty.set(true);
        }
        self.sh().scheduler.cancel_plugin(id);
        if let Ok(mut p) = self.inner.permissions.try_borrow_mut() {
            p.remove_plugin(id);
        }
        self.sh().services.remove_plugin(id);
        let res = std::mem::take(&mut *slot.resources.borrow_mut());
        let others_hold = |f: &dyn Fn(&crate::lifecycle::OwnedResources) -> bool| {
            self.inner.plugins.borrow().records.values().any(|r| {
                !Rc::ptr_eq(&r.slot, slot)
                    && r.slot
                        .resources
                        .try_borrow()
                        .map(|x| f(&x))
                        .unwrap_or(false)
            })
        };
        let channels: Vec<String> = res
            .channels
            .iter()
            .filter(|c| !others_hold(&|r| r.channels.contains(*c)))
            .cloned()
            .collect();
        let chunks: Vec<(String, i32, i32)> = res
            .chunks
            .iter()
            .filter(|c| !others_hold(&|r| r.chunks.contains(*c)))
            .cloned()
            .collect();
        let backend = context::backend_mut(|b| {
            for m in &res.menus {
                let _ = b.menus_close(*m);
            }
            for bar in &res.bars {
                let _ = b.bossbars_remove(*bar);
            }
            for c in &channels {
                let _ = b.messaging_unregister_incoming(c);
            }
            for (w, x, z) in &chunks {
                let _ = b.worlds_keep_chunk_loaded(w, *x, *z, false);
            }
            b.plugin_disabled(id);
        });
        if let Err(e) = backend {
            log::debug!(target: "rustic", "[{id}] no backend to release game resources: {e}");
        }
        for e in self.sh().storage.unload_plugin(id) {
            log::error!(target: "rustic", "[{id}] saving storage failed: {e}");
        }
    }

    // ------------------------------------------------------------------ loading

    /// Discover, validate, resolve and load (instantiate + `on-load`) every plugin of
    /// `<server>/plugins`.
    pub fn load_plugins(&self, backend: &mut dyn Backend) -> LoadReport {
        let dir = self.layout().plugins_dir();
        self.load_dir(&dir, backend)
    }

    /// Discover, validate, resolve and load (instantiate + `on-load`) every plugin of `dir`.
    /// Plugins already loaded or enabled are skipped. Call [`Rustic::enable_all`] next.
    pub fn load_dir(&self, dir: &Path, backend: &mut dyn Backend) -> LoadReport {
        let mut report = LoadReport::default();
        let discovery = match discover(dir) {
            Ok(d) => d,
            Err(e) => {
                log::error!(target: "rustic", "cannot read {}: {e}", dir.display());
                report
                    .failed
                    .push((dir.display().to_string(), e.to_string()));
                return report;
            }
        };
        for e in discovery.errors {
            log::error!(target: "rustic", "{}", e.message);
            report.failed.push((
                e.id.as_ref()
                    .map(|i| i.to_string())
                    .unwrap_or_else(|| e.path.display().to_string()),
                e.message.clone(),
            ));
            match &e.id {
                Some(id) => self.record_failure(id, None, &e.message),
                None => self.inner.plugins.borrow_mut().orphan_errors.push(e),
            }
        }
        let more = self.load_packages(discovery.packages, backend);
        report.loaded.extend(more.loaded);
        report.failed.extend(more.failed);
        report.warnings.extend(more.warnings);
        report
    }

    /// Validate, resolve and load packages built elsewhere (in memory, by the test harness).
    pub fn load_packages(
        &self,
        packages: Vec<PluginPackage>,
        backend: &mut dyn Backend,
    ) -> LoadReport {
        self.enter(backend, || self.load_packages_inner(packages))
    }

    fn record_failure(&self, id: &PluginId, package: Option<PluginPackage>, reason: &str) {
        let mut t = self.inner.plugins.borrow_mut();
        let rec = t.records.entry(id.clone()).or_insert_with(|| PluginRecord {
            slot: Rc::new(PluginSlot::new(id.clone(), u32::MAX)),
            package: None,
            failure: None,
        });
        if rec.slot.is_active() {
            return;
        }
        rec.slot.state.set(PluginState::Failed);
        rec.failure = Some(reason.to_owned());
        if package.is_some() {
            rec.package = package;
        }
        if !t.order.contains(id) {
            t.order.push(id.clone());
        }
    }

    fn load_packages_inner(&self, packages: Vec<PluginPackage>) -> LoadReport {
        let mut report = LoadReport::default();
        let mut fresh: BTreeMap<PluginId, PluginPackage> = BTreeMap::new();
        for pkg in packages {
            let id = pkg.manifest.id.clone();
            if self
                .slot(&id)
                .is_some_and(|s| s.is_active() || s.state.get() == PluginState::Disabling)
            {
                report
                    .failed
                    .push((id.to_string(), "already loaded".into()));
                continue;
            }
            if fresh.contains_key(&id) {
                report.failed.push((
                    id.to_string(),
                    "duplicate plugin id in the same load".into(),
                ));
                continue;
            }
            fresh.insert(id, pkg);
        }
        let active: Vec<Manifest> = {
            let t = self.inner.plugins.borrow();
            t.records
                .values()
                .filter(|r| r.slot.is_active() && !fresh.contains_key(&r.slot.id))
                .filter_map(|r| r.package.as_ref().map(|p| p.manifest.clone()))
                .collect()
        };
        let mut all: Vec<&Manifest> = fresh.values().map(|p| &p.manifest).collect();
        all.extend(active.iter());
        let res = resolve(&all);
        for w in &res.warnings {
            log::warn!(target: "rustic", "{w}");
            report.warnings.push(w.clone());
        }
        for (id, err) in &res.refused {
            if let Some(pkg) = fresh.get(id) {
                let msg = err.to_string();
                log::error!(target: "rustic", "[{id}] not loaded: {msg}");
                self.record_failure(id, Some(pkg.clone()), &msg);
                report.failed.push((id.to_string(), msg));
            }
        }
        for id in res.order {
            let Some(pkg) = fresh.remove(&id) else {
                continue;
            };
            match self.load_one(pkg) {
                Ok(()) => report.loaded.push(id),
                Err(e) => report.failed.push((id.to_string(), e.to_string())),
            }
        }
        report
    }

    /// Instantiate and run `on-load`.
    fn load_one(&self, pkg: PluginPackage) -> Result<(), CoreError> {
        let m = pkg.manifest.clone();
        let id = m.id.clone();
        for (dep, req) in &m.dependencies.required {
            let ok = self.slot(dep).is_some_and(|s| s.is_active())
                && self
                    .manifest(dep)
                    .is_some_and(|dm| version_matches(req, &dm.version));
            if !ok {
                let err = ResolveError::DependencyRefused {
                    dependency: dep.clone(),
                    reason: "it is not loaded".into(),
                };
                self.record_failure(&id, Some(pkg), &err.to_string());
                return Err(CoreError::Dependency {
                    id: id.to_string(),
                    error: err,
                });
            }
        }
        let index = self.sh().tracker.register(&id);
        let slot = Rc::new(PluginSlot::new(id.clone(), index));
        let source_dir = match &pkg.source {
            crate::archive::PluginSource::Directory(p) => Some(p.as_path()),
            _ => None,
        };
        let data_dir = self.layout().data_dir(&id, source_dir);
        if let Err(e) = self.sh().storage.register(&id, data_dir.clone()) {
            let msg = format!("cannot create data folder {}: {e}", data_dir.display());
            self.record_failure(&id, Some(pkg), &msg);
            return Err(CoreError::Load {
                id: id.to_string(),
                message: msg,
            });
        }
        {
            let mut t = self.inner.plugins.borrow_mut();
            t.records.insert(
                id.clone(),
                PluginRecord {
                    slot: slot.clone(),
                    package: Some(pkg.clone()),
                    failure: None,
                },
            );
            if !t.order.contains(&id) {
                t.order.push(id.clone());
            }
        }
        let req = LoadRequest {
            package: &pkg,
            limits: self.config().effective_limits(&m.limits),
            data_dir: &data_dir,
            shared: self.shared(),
        };
        let loaded = match self.inner.loader.try_borrow_mut() {
            Ok(mut l) => {
                let r = std::panic::catch_unwind(AssertUnwindSafe(|| l.load(&req)));
                r.unwrap_or_else(|p| {
                    Err(format!(
                        "loader panicked: {}",
                        PluginFault::from_panic(p).message()
                    ))
                })
            }
            Err(_) => Err("the loader is busy (re-entrant load)".into()),
        };
        let inst = match loaded {
            Ok(i) => i,
            Err(msg) => {
                log::error!(target: "rustic", "[{id}] cannot instantiate: {msg}");
                slot.state.set(PluginState::Failed);
                self.record_failure(&id, None, &msg);
                return Err(CoreError::Load {
                    id: id.to_string(),
                    message: msg,
                });
            }
        };
        *slot.instance.borrow_mut() = Some(inst);
        slot.state.set(PluginState::Loaded);
        match self.call_slot(
            &slot,
            CallKind::Load,
            SectionKind::Lifecycle,
            "on-load",
            |i| i.on_load(),
        ) {
            Ok(Ok(())) => {
                log::info!(target: "rustic", "[{id}] loaded {} v{}", m.name, m.version);
                Ok(())
            }
            Ok(Err(msg)) => {
                let reason = format!("on-load returned an error: {msg}");
                self.disable_cascade(&id, &reason, Teardown::Failed);
                Err(CoreError::Load {
                    id: id.to_string(),
                    message: reason,
                })
            }
            Err(e) => {
                self.disable_cascade(&id, &format!("on-load: {e}"), Teardown::Failed);
                Err(CoreError::Load {
                    id: id.to_string(),
                    message: e.to_string(),
                })
            }
        }
    }

    // ------------------------------------------------------------------ enabling

    /// Enable every loaded plugin in load order. Returns each result.
    pub fn enable_all(&self, backend: &mut dyn Backend) -> Vec<(PluginId, Result<(), CoreError>)> {
        self.enter(backend, || {
            let order = self.inner.plugins.borrow().order.clone();
            order
                .into_iter()
                .filter(|id| {
                    self.slot(id)
                        .is_some_and(|s| s.state.get() == PluginState::Loaded)
                })
                .map(|id| {
                    let r = self.enable_loaded(&id);
                    (id, r)
                })
                .collect()
        })
    }

    /// Enable one plugin (re-instantiating it if it was disabled or failed).
    pub fn enable(&self, id: &str, backend: &mut dyn Backend) -> Result<(), CoreError> {
        self.enter(backend, || self.enable_inner(id))
    }

    fn enable_inner(&self, id: &str) -> Result<(), CoreError> {
        let slot = self
            .slot(id)
            .ok_or_else(|| CoreError::UnknownPlugin(id.to_owned()))?;
        match slot.state.get() {
            PluginState::Enabled => Ok(()),
            PluginState::Loaded => self.enable_loaded(&slot.id),
            PluginState::Disabling => Err(CoreError::Busy(id.to_owned())),
            _ => {
                let pkg = self
                    .inner
                    .plugins
                    .borrow()
                    .records
                    .get(id)
                    .and_then(|r| r.package.clone())
                    .ok_or_else(|| CoreError::InvalidState {
                        id: id.to_owned(),
                        state: slot.state.get(),
                        action: "enable (its package could not be read)",
                    })?;
                self.load_one(pkg)?;
                self.enable_loaded(&slot.id)
            }
        }
    }

    fn enable_loaded(&self, id: &PluginId) -> Result<(), CoreError> {
        let slot = self
            .slot(id)
            .ok_or_else(|| CoreError::UnknownPlugin(id.to_string()))?;
        let pkg = self
            .inner
            .plugins
            .borrow()
            .records
            .get(id)
            .and_then(|r| r.package.clone())
            .ok_or_else(|| CoreError::UnknownPlugin(id.to_string()))?;
        for (dep, req) in &pkg.manifest.dependencies.required {
            let enabled = self.slot(dep).is_some_and(|s| s.is_enabled());
            let version_ok = self
                .manifest(dep)
                .is_some_and(|dm| version_matches(req, &dm.version));
            if !enabled || !version_ok {
                let error = ResolveError::DependencyNotEnabled(dep.clone());
                self.disable_cascade(id, &error.to_string(), Teardown::Failed);
                return Err(CoreError::Dependency {
                    id: id.to_string(),
                    error,
                });
            }
        }
        if let Some(dir) = self.sh().storage.data_dir(id) {
            match pkg.default_config() {
                Ok(cfg) => {
                    if let Err(e) = install_default_config(&dir, cfg.as_deref()) {
                        log::warn!(target: "rustic", "[{id}] cannot write default config.toml: {e}");
                    }
                }
                Err(e) => {
                    log::warn!(target: "rustic", "[{id}] cannot read default config.toml: {e}")
                }
            }
        }
        slot.state.set(PluginState::Enabled);
        self.sh()
            .enabled
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(id.clone());
        match self.call_slot(
            &slot,
            CallKind::Enable,
            SectionKind::Lifecycle,
            "on-enable",
            |i| i.on_enable(),
        ) {
            Ok(Ok(())) => {
                if let Some(rec) = self.inner.plugins.borrow_mut().records.get_mut(id) {
                    rec.failure = None;
                }
                log::info!(target: "rustic", "[{id}] enabled {} v{}", pkg.manifest.name, pkg.manifest.version);
                Ok(())
            }
            Ok(Err(msg)) => {
                let reason = format!("on-enable returned an error: {msg}");
                self.disable_cascade(id, &reason, Teardown::Failed);
                Err(CoreError::Enable {
                    id: id.to_string(),
                    message: reason,
                })
            }
            Err(e) => {
                self.disable_cascade(id, &format!("on-enable: {e}"), Teardown::Failed);
                Err(CoreError::Enable {
                    id: id.to_string(),
                    message: e.to_string(),
                })
            }
        }
    }

    /// Disable a plugin (its dependents first). Returns every plugin disabled, the requested
    /// one last.
    pub fn disable(&self, id: &str, backend: &mut dyn Backend) -> Result<Vec<PluginId>, CoreError> {
        self.enter(backend, || self.disable_inner(id, "disabled by request"))
    }

    fn disable_inner(&self, id: &str, reason: &str) -> Result<Vec<PluginId>, CoreError> {
        let slot = self
            .slot(id)
            .ok_or_else(|| CoreError::UnknownPlugin(id.to_owned()))?;
        if !slot.is_active() {
            return Err(CoreError::InvalidState {
                id: id.to_owned(),
                state: slot.state.get(),
                action: "disable",
            });
        }
        Ok(self.disable_cascade(&slot.id, reason, Teardown::Graceful))
    }

    /// Reload: disable (dependents first), re-read the package from disk, load, enable, then
    /// re-enable the dependents. Returns the plugins enabled again.
    pub fn reload(&self, id: &str, backend: &mut dyn Backend) -> Result<Vec<PluginId>, CoreError> {
        self.enter(backend, || self.reload_inner(id))
    }

    fn reload_inner(&self, id: &str) -> Result<Vec<PluginId>, CoreError> {
        let slot = self
            .slot(id)
            .ok_or_else(|| CoreError::UnknownPlugin(id.to_owned()))?;
        let pid = slot.id.clone();
        if context::is_executing(id) {
            return Err(CoreError::Busy(id.to_owned()));
        }
        let mut dependents = self.active_dependents(id);
        dependents.reverse(); // load order
        if slot.is_active() {
            self.disable_cascade(&pid, "reloading", Teardown::Graceful);
        }
        let old = self
            .inner
            .plugins
            .borrow()
            .records
            .get(id)
            .and_then(|r| r.package.clone())
            .ok_or_else(|| CoreError::InvalidState {
                id: id.to_owned(),
                state: slot.state.get(),
                action: "reload (no package known)",
            })?;
        let pkg = match old.reopen() {
            Ok(p) if p.manifest.id == pid => p,
            Ok(p) => {
                let msg = format!("the package now declares id `{}`", p.manifest.id);
                self.record_failure(&pid, None, &msg);
                return Err(CoreError::Load {
                    id: id.to_owned(),
                    message: msg,
                });
            }
            Err(e) => {
                self.record_failure(&pid, None, &e.to_string());
                return Err(e.into());
            }
        };
        self.load_one(pkg)?;
        self.enable_loaded(&pid)?;
        let mut done = vec![pid.clone()];
        for d in dependents {
            match self.enable_inner(&d) {
                Ok(()) => done.push(d),
                Err(e) => {
                    log::error!(target: "rustic", "[{d}] not re-enabled after reloading `{pid}`: {e}")
                }
            }
        }
        log::info!(target: "rustic", "reloaded {}", done.iter().map(|p| p.as_str()).collect::<Vec<_>>().join(", "));
        Ok(done)
    }

    /// Disable every plugin (reverse load order, `on-disable` called) and flush storage.
    pub fn shutdown(&self, backend: &mut dyn Backend) {
        self.enter(backend, || {
            self.inner.shut_down.set(true);
            let order = self.inner.plugins.borrow().order.clone();
            for id in order.iter().rev() {
                if self.slot(id).is_some_and(|s| s.is_active()) {
                    self.teardown(id, "server shutdown", Teardown::Graceful);
                }
            }
            for e in self.sh().storage.flush_all() {
                log::error!(target: "rustic", "saving storage failed: {e}");
            }
        });
    }

    /// Whether [`Rustic::shutdown`] ran.
    pub fn is_shut_down(&self) -> bool {
        self.inner.shut_down.get()
    }

    // ------------------------------------------------------------------ queries

    fn info_of(&self, id: &PluginId, rec: &PluginRecord) -> PluginInfo {
        let m = rec.package.as_ref().map(|p| &p.manifest);
        PluginInfo {
            id: id.clone(),
            name: m.map(|m| m.name.clone()).unwrap_or_else(|| id.to_string()),
            version: m.map(|m| m.version.to_string()).unwrap_or_default(),
            state: rec.slot.state.get(),
            failure: rec.failure.clone(),
            authors: m.map(|m| m.authors.clone()).unwrap_or_default(),
            description: m.and_then(|m| m.description.clone()),
            license: m.map(|m| m.license.clone()).unwrap_or_default(),
            source: rec.package.as_ref().map(|p| p.path().to_owned()),
            required: m
                .map(|m| {
                    m.dependencies
                        .required
                        .keys()
                        .map(|k| k.to_string())
                        .collect()
                })
                .unwrap_or_default(),
            optional: m
                .map(|m| {
                    m.dependencies
                        .optional
                        .keys()
                        .map(|k| k.to_string())
                        .collect()
                })
                .unwrap_or_default(),
            data_dir: self.sh().storage.data_dir(id),
        }
    }

    /// Every known plugin (loaded, enabled, disabled, failed), in load order.
    pub fn plugins(&self) -> Vec<PluginInfo> {
        let t = self.inner.plugins.borrow();
        t.order
            .iter()
            .filter_map(|id| t.records.get(id).map(|r| self.info_of(id, r)))
            .collect()
    }

    /// One plugin.
    pub fn plugin(&self, id: &str) -> Option<PluginInfo> {
        let t = self.inner.plugins.borrow();
        t.records.get_key_value(id).map(|(k, r)| self.info_of(k, r))
    }

    /// A plugin's state.
    pub fn state(&self, id: &str) -> Option<PluginState> {
        self.slot(id).map(|s| s.state.get())
    }

    /// Packages that could not even be read (no id known).
    pub fn discovery_errors(&self) -> Vec<DiscoveryError> {
        self.inner.plugins.borrow().orphan_errors.clone()
    }

    /// WIT `server.plugin-meta` of every plugin.
    pub fn plugin_metas(&self) -> Vec<PluginMeta> {
        self.plugins()
            .into_iter()
            .map(|p| PluginMeta {
                id: p.id.to_string(),
                name: p.name,
                version: p.version,
                enabled: p.state == PluginState::Enabled,
            })
            .collect()
    }

    // ------------------------------------------------------------------ events

    /// Whether anything listens to an event name ("player-move", "custom:<name>"). O(1):
    /// the engine adapter uses it to skip building events nobody listens to.
    pub fn has_listeners(&self, name: &str) -> bool {
        let Ok(bus) = self.inner.bus.try_borrow() else {
            return true;
        };
        match EventKind::from_name(name) {
            Some(k) => bus.has_listeners(k),
            None => name
                .strip_prefix(CUSTOM_PREFIX)
                .is_some_and(|n| bus.has_custom_listeners(n)),
        }
    }

    /// Whether anything listens to a built-in event (one array load).
    #[inline]
    pub fn has_listeners_for(&self, kind: EventKind) -> bool {
        self.inner
            .bus
            .try_borrow()
            .map(|b| b.has_listeners(kind))
            .unwrap_or(true)
    }

    /// The subscriptions of an event name.
    pub fn listeners(&self, name: &str) -> Vec<ListenerInfo> {
        match (EventKey::parse(name), self.inner.bus.try_borrow()) {
            (Ok(key), Ok(bus)) => bus.listeners(&key),
            _ => Vec::new(),
        }
    }

    /// Fire an event: handlers run by priority then registration order; returns the event with
    /// the mutable fields (and `cancelled`) as the handlers left them. With no listener this is
    /// a few nanoseconds.
    pub fn fire(&self, event: Event, backend: &mut dyn Backend) -> Event {
        match event.kind() {
            EventKind::PlayerQuit | EventKind::InventoryClick | EventKind::InventoryClose => {
                return self.enter(backend, || self.fire_with_bookkeeping(event));
            }
            _ => {}
        }
        let list = match self.snapshot(&event) {
            Some(l) => l,
            None => return event,
        };
        self.enter(backend, || self.dispatch(event, &list))
    }

    /// Typed [`Rustic::fire`].
    pub fn fire_record<E: EventRecord>(&self, event: E, backend: &mut dyn Backend) -> E {
        match E::from_event(self.fire(event.into(), backend)) {
            Ok(e) => e,
            // `fire` never changes the variant of the event it is given.
            Err(_) => unreachable!("fire returned another kind of event"),
        }
    }

    fn snapshot(&self, event: &Event) -> Option<ListenerList> {
        let bus = self.inner.bus.try_borrow().ok()?;
        let list = match event {
            Event::CustomEvent(c) => bus.custom(&c.name)?,
            other => bus.builtin(other.kind()),
        };
        if list.is_empty() {
            None
        } else {
            Some(list.clone())
        }
    }

    fn fire_with_bookkeeping(&self, mut event: Event) -> Event {
        if let Event::InventoryClick(c) = &mut event
            && let Some(m) = c.menu
            && self.menu_owner(m).is_some()
        {
            c.cancelled = true;
        }
        let event = match self.snapshot(&event) {
            Some(list) => self.dispatch(event, &list),
            None => event,
        };
        match &event {
            Event::InventoryClose(c) => {
                if let Some(m) = c.menu
                    && let Some(slot) = self.menu_owner(m)
                {
                    slot.resources.borrow_mut().menus.remove(&m);
                }
            }
            Event::PlayerQuit(q) => {
                for e in self.sh().storage.evict_player(q.player) {
                    log::error!(target: "rustic", "saving player data failed: {e}");
                }
            }
            _ => {}
        }
        event
    }

    fn menu_owner(&self, menu: MenuId) -> Option<Rc<PluginSlot>> {
        self.inner
            .plugins
            .borrow()
            .records
            .values()
            .find(|r| {
                r.slot
                    .resources
                    .try_borrow()
                    .is_ok_and(|x| x.menus.contains(&menu))
            })
            .map(|r| r.slot.clone())
    }

    pub(crate) fn dispatch(&self, mut event: Event, list: &ListenerList) -> Event {
        let depth = self.inner.dispatch_depth.get();
        if depth >= MAX_DISPATCH_DEPTH {
            log::warn!(
                target: "rustic",
                "event {} not dispatched: nested event depth limit ({MAX_DISPATCH_DEPTH}) reached",
                event.name()
            );
            return event;
        }
        struct Depth<'a>(&'a Cell<u32>);
        impl Drop for Depth<'_> {
            fn drop(&mut self) {
                self.0.set(self.0.get().saturating_sub(1));
            }
        }
        self.inner.dispatch_depth.set(depth + 1);
        let _depth = Depth(&self.inner.dispatch_depth);
        let custom_name;
        let name: &str = match &event {
            Event::CustomEvent(c) => {
                custom_name = format!("{CUSTOM_PREFIX}{}", c.name);
                &custom_name
            }
            other => other.name(),
        };
        for l in list.iter() {
            if !l.slot.is_enabled() {
                continue;
            }
            if l.ignore_cancelled && event.cancelled() {
                continue;
            }
            if let Event::PluginMessage(pm) = &event
                && !l
                    .slot
                    .resources
                    .try_borrow()
                    .is_ok_and(|r| r.channels.contains(&pm.channel))
            {
                continue;
            }
            // Menu ids are only shown to the plugin that owns the menu.
            let masked;
            let to_send: &Event = match menu_of(&event) {
                Some(m)
                    if !l
                        .slot
                        .resources
                        .try_borrow()
                        .is_ok_and(|r| r.menus.contains(&m)) =>
                {
                    masked = without_menu(&event);
                    &masked
                }
                _ => &event,
            };
            match self.call_slot(&l.slot, CallKind::Event, SectionKind::Event, name, |i| {
                i.handle_event(l.id, to_send)
            }) {
                Ok(ret) => {
                    if l.priority != Priority::Monitor && !event.apply_mutable_from(&ret) {
                        log::warn!(
                            target: "rustic",
                            "[{}] handler {} returned a `{}` for a `{name}` event; ignored",
                            l.slot.id,
                            l.id,
                            ret.name()
                        );
                    }
                }
                Err(CallError::Busy) => {
                    let key = (l.slot.id.clone(), name.to_owned());
                    if self.inner.warned_reentry.borrow_mut().insert(key) {
                        log::warn!(
                            target: "rustic",
                            "event {name} not delivered to `{}`: the plugin is currently executing (re-entrant dispatch; warned once)",
                            l.slot.id
                        );
                    }
                }
                Err(_) => {}
            }
        }
        event
    }

    // ------------------------------------------------------------------ ticking

    /// Run one tick: deliver async messages, process async faults, run due sync tasks within
    /// the budget, flush storage periodically.
    pub fn tick(&self, backend: &mut dyn Backend) -> TickReport {
        self.enter(backend, || self.tick_inner())
    }

    fn tick_inner(&self) -> TickReport {
        let sh = self.sh();
        let tick = sh.tick.fetch_add(1, Ordering::Relaxed) + 1;
        let start = sh.clock.now();
        let mut report = TickReport {
            tick,
            ..TickReport::default()
        };
        let faults: Vec<(PluginId, PluginFault)> =
            std::mem::take(&mut *sh.faults.lock().unwrap_or_else(PoisonError::into_inner));
        for (id, fault) in faults {
            if self.slot(&id).is_some_and(|s| s.is_active()) {
                self.handle_fault(&id, CallKind::Task, &fault);
            }
        }
        for msg in sh.scheduler.take_inbox() {
            let Some(slot) = self.slot(&msg.plugin).filter(|s| s.is_enabled()) else {
                continue;
            };
            report.messages += 1;
            let _ = self.call_slot(
                &slot,
                CallKind::Message,
                SectionKind::Message,
                &msg.topic,
                |i| i.on_message(&msg.topic, &msg.payload),
            );
        }
        let budget = Duration::from_millis(sh.config.scheduler.max_task_ms_per_tick);
        let due = sh.scheduler.due_sync(tick);
        for (i, d) in due.iter().enumerate() {
            if i > 0 && sh.clock.now().saturating_duration_since(start) > budget {
                report.tasks_rolled = due.len() - i;
                sh.scheduler.set_resume(d.task);
                if tick.saturating_sub(self.inner.last_budget_warning.get()) >= 100
                    || self.inner.last_budget_warning.get() == 0
                {
                    self.inner.last_budget_warning.set(tick);
                    log::warn!(
                        target: "rustic",
                        "tick {tick}: sync tasks exceeded the {} ms budget; {} task(s) rolled over to the next tick",
                        budget.as_millis(),
                        report.tasks_rolled
                    );
                }
                break;
            }
            let Some(plugin) = sh.scheduler.begin_run(d.task, tick) else {
                continue;
            };
            let Some(slot) = self.slot(&plugin).filter(|s| s.is_enabled()) else {
                sh.scheduler.cancel(&plugin, d.task);
                continue;
            };
            report.tasks_run += 1;
            let name = if d.repeating {
                format!("timer#{}", d.task)
            } else {
                "run-later".to_owned()
            };
            let task = d.task;
            let _ = self.call_slot(&slot, CallKind::Task, SectionKind::Task, &name, |i| {
                i.run_task(task)
            });
        }
        let interval = sh.config.storage.flush_interval_ticks.max(1);
        if tick.saturating_sub(self.inner.last_flush_tick.get()) >= interval {
            self.inner.last_flush_tick.set(tick);
            for e in sh.storage.flush_all() {
                log::error!(target: "rustic", "saving storage failed: {e}");
            }
        }
        report
    }

    /// Record the duration of a whole server tick (engine side) for timings' MSPT shares.
    pub fn record_server_tick(&self, took: Duration) {
        self.timings().record_tick(took);
    }

    /// Async tasks due now; see [`SharedHandle::due_async_tasks`].
    pub fn due_async_tasks(&self, now: Instant) -> Vec<AsyncDue> {
        self.inner.shared.due_async_tasks(now)
    }

    /// See [`SharedHandle::async_task_finished`].
    pub fn async_task_finished(&self, task: TaskId) {
        self.inner.shared.async_task_finished(task);
    }

    // ------------------------------------------------------------------ permissions

    /// Whether `sender` has `node` (see [`crate::permission`] for the resolution order).
    pub fn has_permission(
        &self,
        sender: CommandSender,
        node: &str,
        backend: &mut dyn Backend,
    ) -> bool {
        self.enter(backend, || self.check_permission(sender, node))
    }

    pub(crate) fn check_permission(&self, sender: CommandSender, node: &str) -> bool {
        let player = match sender {
            CommandSender::Console => return true,
            CommandSender::Player(u) => u,
        };
        let is_op = context::backend_call(|b| b.players_is_op(player)).unwrap_or(false);
        let Ok(table) = self.inner.permissions.try_borrow().map(|p| p.table()) else {
            return false;
        };
        let provider = self.sh().services.provider(PERMISSION_PROVIDER_IFACE);
        let mut explicit = |n: &str| -> Tristate {
            if let Some(pp) = &provider
                && let Ok(mut slot) = self.inner.perm_provider.try_borrow_mut()
                && let Some(p) = slot.as_mut()
            {
                match p.check(pp, player, n) {
                    Ok(t) => return t,
                    Err(e) => {
                        log::debug!(target: "rustic", "permission provider `{pp}` failed for {n}: {e}; using the default engine")
                    }
                }
            }
            self.inner
                .permissions
                .try_borrow()
                .map(|p| p.engine().check(player.as_u128(), n))
                .unwrap_or(Tristate::Undefined)
        };
        resolve_permission(&table, node, is_op, &mut explicit)
    }

    // ------------------------------------------------------------------ commands

    /// Changes whenever commands change (re-register usages with the engine then).
    pub fn commands_generation(&self) -> u64 {
        self.inner
            .commands
            .try_borrow()
            .map(|c| c.generation())
            .unwrap_or(0)
    }

    /// Neutral export of every command tree.
    pub fn command_tree(&self) -> Vec<CommandTreeExport> {
        self.inner
            .commands
            .try_borrow()
            .map(|c| c.export())
            .unwrap_or_default()
    }

    /// Usage lines for Horizon's `register_commands`.
    pub fn horizon_usages(&self) -> Vec<HorizonUsage> {
        self.inner
            .commands
            .try_borrow()
            .map(|c| c.horizon_usages())
            .unwrap_or_default()
    }

    fn reply(&self, sender: CommandSender, message: &Text) {
        let r = context::backend_call(|b| match sender {
            CommandSender::Console => b.console_message(message),
            CommandSender::Player(p) => b.players_send_message(p, message),
        });
        if let Err(e) = r {
            log::debug!(target: "rustic", "reply not delivered ({e}): {}", message.to_plain());
        }
    }

    /// Run a command line (with or without the leading slash). Error feedback is sent to the
    /// sender; `UnknownCommand` is silent (let the engine try its own commands).
    pub fn dispatch_command(
        &self,
        sender: CommandSender,
        line: &str,
        backend: &mut dyn Backend,
    ) -> CommandOutcome {
        self.enter(backend, || self.dispatch_command_inner(sender, line))
    }

    pub(crate) fn dispatch_command_inner(
        &self,
        sender: CommandSender,
        line: &str,
    ) -> CommandOutcome {
        let line = line.trim().trim_start_matches('/').trim();
        let parsed = {
            let Ok(reg) = self.inner.commands.try_borrow() else {
                return CommandOutcome::Fault("command registry busy".into());
            };
            let mut r = BackendResolver {
                rng: &self.inner.rng,
            };
            reg.parse(line, sender, &mut r)
        };
        let ParseOutcome::Command {
            id,
            label,
            matches,
            error,
        } = parsed
        else {
            return CommandOutcome::UnknownCommand;
        };
        let Some(cmd) = self
            .inner
            .commands
            .try_borrow()
            .ok()
            .and_then(|r| r.get(id).cloned())
        else {
            return CommandOutcome::UnknownCommand;
        };
        if cmd.spec.players_only && sender == CommandSender::Console {
            self.reply(
                sender,
                &Text::error("This command can only be run by a player."),
            );
            return CommandOutcome::PlayerOnly;
        }
        let usable: Vec<u32> = (0..cmd.spec.usages.len() as u32)
            .filter(|u| {
                cmd.usage_permission(*u)
                    .is_none_or(|p| self.check_permission(sender, p))
            })
            .collect();
        if usable.is_empty() {
            self.reply(
                sender,
                &Text::error("You do not have permission to use this command."),
            );
            return CommandOutcome::NoPermission;
        }
        if matches.is_empty() {
            let mut msg = error.unwrap_or_else(|| "invalid command".into());
            msg.push_str("\nUsage:");
            for u in &usable {
                msg.push_str("\n  ");
                msg.push_str(&cmd.usage_line(&label, *u));
            }
            self.reply(sender, &Text::error(&msg));
            return CommandOutcome::Usage(msg);
        }
        let Some(m) = matches.into_iter().find(|m| usable.contains(&m.usage)) else {
            self.reply(
                sender,
                &Text::error("You do not have permission to do that."),
            );
            return CommandOutcome::NoPermission;
        };
        if cmd.owner.as_str() == crate::id::CORE_OWNER {
            return self.builtin_execute(sender, &label, m);
        }
        let Some(slot) = self.slot(&cmd.owner).filter(|s| s.is_enabled()) else {
            let msg = format!("Plugin `{}` is not enabled.", cmd.owner);
            self.reply(sender, &Text::error(&msg));
            return CommandOutcome::Fault(msg);
        };
        let ctx = CommandContext {
            sender,
            label,
            usage: m.usage,
            args: m.args,
            raw: line.to_owned(),
        };
        match self.call_slot(
            &slot,
            CallKind::Command,
            SectionKind::Command,
            &cmd.spec.name,
            |i| i.execute_command(id, ctx),
        ) {
            Ok(Ok(())) => CommandOutcome::Executed,
            Ok(Err(text)) => {
                self.reply(sender, &red(&text));
                CommandOutcome::PluginError(text)
            }
            Err(e) => {
                self.reply(
                    sender,
                    &Text::error(format!(
                        "An internal error occurred in plugin `{}` while running this command.",
                        cmd.owner
                    )),
                );
                CommandOutcome::Fault(e.to_string())
            }
        }
    }

    /// Tab completion of a line (with or without slash): suggestions for the last token.
    pub fn complete(
        &self,
        sender: CommandSender,
        line: &str,
        backend: &mut dyn Backend,
    ) -> Vec<String> {
        self.enter(backend, || self.complete_inner(sender, line))
    }

    fn complete_inner(&self, sender: CommandSender, line: &str) -> Vec<String> {
        let line = line.trim_start().trim_start_matches('/');
        let plan = {
            let Ok(reg) = self.inner.commands.try_borrow() else {
                return Vec::new();
            };
            let mut r = BackendResolver {
                rng: &self.inner.rng,
            };
            reg.complete(line, sender, &mut r)
        };
        let Ok(reg) = self.inner.commands.try_borrow().map(|r| {
            let ids: Vec<CommandId> = plan
                .labels
                .iter()
                .map(|(_, id)| *id)
                .chain(plan.command)
                .collect();
            ids.into_iter()
                .filter_map(|id| r.get(id).cloned())
                .collect::<Vec<_>>()
        }) else {
            return Vec::new();
        };
        let permitted = |cmd: &crate::commands::RegisteredCommand, u: u32| {
            cmd.usage_permission(u)
                .is_none_or(|p| self.check_permission(sender, p))
        };
        let visible = |cmd: &crate::commands::RegisteredCommand| {
            !(cmd.spec.players_only && sender == CommandSender::Console)
                && (0..cmd.spec.usages.len() as u32).any(|u| permitted(cmd, u))
        };
        let mut out: Vec<String> = Vec::new();
        let mut push = |s: String| {
            if !out.contains(&s) {
                out.push(s);
            }
        };
        if plan.command.is_none() {
            for (label, id) in &plan.labels {
                if let Some(cmd) = reg.iter().find(|c| c.id == *id)
                    && visible(cmd)
                {
                    push(label.clone());
                }
            }
            return out;
        }
        let Some(cmd) = plan
            .command
            .and_then(|id| reg.into_iter().find(|c| c.id == id))
        else {
            return out;
        };
        if !visible(&cmd) {
            return out;
        }
        let partial_lower = plan.partial.to_ascii_lowercase();
        for c in &plan.candidates {
            if c.usages.iter().any(|u| permitted(&cmd, *u))
                && c.text.to_ascii_lowercase().starts_with(&partial_lower)
            {
                push(c.text.clone());
            }
        }
        for d in &plan.dynamic {
            if !d.usages.iter().any(|u| permitted(&cmd, *u)) {
                continue;
            }
            let ctx = CompleteContext {
                sender,
                raw: line.to_owned(),
                argument: d.argument.clone(),
                partial: plan.partial.clone(),
            };
            if cmd.owner.as_str() == crate::id::CORE_OWNER {
                for s in self.builtin_complete(&ctx) {
                    if s.to_ascii_lowercase().starts_with(&partial_lower) {
                        push(s);
                    }
                }
                continue;
            }
            let Some(slot) = self.slot(&cmd.owner).filter(|s| s.is_enabled()) else {
                continue;
            };
            if let Ok(list) = self.call_slot(
                &slot,
                CallKind::Completion,
                SectionKind::Completion,
                &cmd.spec.name,
                |i| i.complete_command(cmd.id, ctx),
            ) {
                for s in list.into_iter().take(1000) {
                    if s.to_ascii_lowercase().starts_with(&partial_lower) {
                        push(s);
                    }
                }
            }
        }
        out
    }

    pub(crate) fn next_command_id(&self) -> CommandId {
        let id = self.inner.next_command.get();
        self.inner.next_command.set(id.wrapping_add(1).max(1));
        id
    }

    pub(crate) fn mark_commands_dirty(&self) {
        self.inner.commands_dirty.set(true);
    }

    pub(crate) fn commands_mut<R>(&self, f: impl FnOnce(&mut CommandRegistry) -> R) -> Option<R> {
        self.inner
            .commands
            .try_borrow_mut()
            .ok()
            .map(|mut c| f(&mut c))
    }

    pub(crate) fn plugin_slots(&self) -> Vec<(PluginId, PluginState)> {
        let t = self.inner.plugins.borrow();
        t.order
            .iter()
            .filter_map(|id| t.records.get(id).map(|r| (id.clone(), r.slot.state.get())))
            .collect()
    }

    pub(crate) fn enable_by_command(&self, id: &str) -> Result<(), CoreError> {
        self.enable_inner(id)
    }

    pub(crate) fn disable_by_command(&self, id: &str) -> Result<Vec<PluginId>, CoreError> {
        if context::is_executing(id) {
            // Disabling oneself from a command works (deferred), but say so.
            log::info!(target: "rustic", "[{id}] disabling itself; on-disable runs when its call returns");
        }
        self.disable_inner(id, "disabled by command")
    }

    pub(crate) fn reload_by_command(&self, id: &str) -> Result<Vec<PluginId>, CoreError> {
        self.reload_inner(id)
    }

    pub(crate) fn counts_for(&self, id: &str) -> (usize, usize, (usize, usize)) {
        let listeners = self
            .inner
            .bus
            .try_borrow()
            .map(|b| b.count_for(id))
            .unwrap_or(0);
        let commands = self
            .inner
            .commands
            .try_borrow()
            .map(|c| c.all().filter(|x| x.owner.as_str() == id).count())
            .unwrap_or(0);
        (listeners, commands, self.sh().scheduler.count(id))
    }
}

/// What a plugin's **main instance** can call: the core-owned imports (`events`, `commands`,
/// `scheduler`, `permissions`, `services`, `storage`, `logging`, `server.plugins/self-meta`,
/// `players.*-data`) and the ownership-tracked game calls (menus, boss bars, plugin channels,
/// chunk tickets). Everything else goes straight to the backend via
/// [`crate::context::backend_call`].
#[derive(Clone, Debug)]
pub struct PluginApi {
    rt: Rustic,
    plugin: PluginId,
}

impl PluginApi {
    /// The plugin.
    pub fn id(&self) -> &PluginId {
        &self.plugin
    }

    /// The runtime.
    pub fn runtime(&self) -> &Rustic {
        &self.rt
    }

    /// The thread-safe subset (same as the async instance gets).
    pub fn async_api(&self) -> AsyncApi {
        self.rt.shared().async_api(self.plugin.clone())
    }

    fn active_slot(&self) -> Result<Rc<PluginSlot>, String> {
        self.rt
            .slot(&self.plugin)
            .filter(|s| s.is_active())
            .ok_or_else(|| format!("plugin `{}` is not loaded or enabled", self.plugin))
    }

    fn owned(
        &self,
        check: impl FnOnce(&crate::lifecycle::OwnedResources) -> bool,
        what: &str,
    ) -> Result<(), HostError> {
        let slot = self
            .rt
            .slot(&self.plugin)
            .ok_or(HostError::Other("plugin unknown".into()))?;
        let ok = slot
            .resources
            .try_borrow()
            .map(|r| check(&r))
            .unwrap_or(false);
        if ok {
            Ok(())
        } else {
            Err(HostError::Invalid(format!(
                "{what} is not owned by this plugin"
            )))
        }
    }

    // ---- logging
    /// `logging.log`.
    pub fn log(&self, level: LogLevel, message: &str) {
        log_plugin(&self.plugin, level, message);
    }

    // ---- events
    /// `events.subscribe`.
    pub fn subscribe(
        &self,
        event: &str,
        priority: Priority,
        ignore_cancelled: bool,
    ) -> Result<HandlerId, String> {
        let key = EventKey::parse(event)?;
        let slot = self.active_slot()?;
        let id = self.rt.inner.next_handler.get();
        self.rt.inner.next_handler.set(id.wrapping_add(1).max(1));
        self.rt
            .inner
            .bus
            .try_borrow_mut()
            .map_err(|_| "the event bus is busy".to_owned())?
            .subscribe(key, id, slot, priority, ignore_cancelled);
        Ok(id)
    }

    /// `events.unsubscribe` (own handlers only).
    pub fn unsubscribe(&self, handler: HandlerId) {
        if let Ok(mut bus) = self.rt.inner.bus.try_borrow_mut() {
            bus.unsubscribe(&self.plugin, handler);
        }
    }

    /// `events.call-custom`: `name` is `<plugin-id>:<name>` (the caller's id is prefixed when
    /// there is no namespace). Returns whether a handler cancelled it.
    pub fn call_custom(&self, name: &str, payload: Vec<u8>) -> bool {
        let full = if name.contains(':') {
            name.to_owned()
        } else {
            format!("{}:{name}", self.plugin)
        };
        if let Err(e) = validate_custom_name(&full) {
            self.log(LogLevel::Warn, &format!("events.call-custom: {e}"));
            return false;
        }
        let event = Event::CustomEvent(CustomEvent {
            name: full,
            source_plugin: self.plugin.to_string(),
            payload,
            cancelled: false,
        });
        match self.rt.snapshot(&event) {
            Some(list) => {
                context::with_runtime(&self.rt.inner, || self.rt.dispatch(event, &list)).cancelled()
            }
            None => false,
        }
    }

    // ---- commands
    /// `commands.register`.
    pub fn register_command(&self, spec: CommandSpec) -> Result<CommandId, String> {
        self.active_slot()?;
        let id = self.rt.next_command_id();
        self.rt
            .commands_mut(|c| c.register(&self.plugin, id, spec))
            .ok_or_else(|| "the command registry is busy".to_owned())??;
        self.rt.mark_commands_dirty();
        Ok(id)
    }

    /// `commands.unregister` (own commands only).
    pub fn unregister_command(&self, command: CommandId) {
        if self
            .rt
            .commands_mut(|c| c.unregister(&self.plugin, command))
            == Some(true)
        {
            self.rt.mark_commands_dirty();
        }
    }

    /// `commands.reply`.
    pub fn reply(&self, sender: CommandSender, message: &Text) {
        self.rt.reply(sender, message);
    }

    // ---- scheduler
    /// `scheduler.run-later`.
    pub fn run_later(&self, delay: u64) -> TaskId {
        self.async_api().run_later(delay)
    }
    /// `scheduler.run-timer`.
    pub fn run_timer(&self, delay: u64, period: u64) -> TaskId {
        self.async_api().run_timer(delay, period)
    }
    /// `scheduler.run-async`.
    pub fn run_async(&self, delay_ms: u64) -> TaskId {
        self.async_api().run_async(delay_ms)
    }
    /// `scheduler.run-async-timer`.
    pub fn run_async_timer(&self, delay_ms: u64, period_ms: u64) -> TaskId {
        self.async_api().run_async_timer(delay_ms, period_ms)
    }
    /// `scheduler.run-async-job`.
    pub fn run_async_job(&self, job: &str, payload: Vec<u8>, delay_ms: u64) -> TaskId {
        self.async_api().run_async_job(job, payload, delay_ms)
    }
    /// `scheduler.cancel`.
    pub fn cancel_task(&self, task: TaskId) {
        self.async_api().cancel(task);
    }
    /// `scheduler.post-to-main` (from the main instance it is delivered next tick too).
    pub fn post_to_main(&self, topic: &str, payload: Vec<u8>) {
        self.async_api().post_to_main(topic, payload);
    }
    /// `scheduler.current-tick`.
    pub fn current_tick(&self) -> u64 {
        self.rt.current_tick()
    }
    /// `scheduler.is-main`: true.
    pub fn is_main(&self) -> bool {
        true
    }

    // ---- permissions
    /// `permissions.declare` (errors are also logged: the WIT function returns nothing).
    pub fn declare_permission(
        &self,
        node: &str,
        default: PermissionDefault,
        description: &str,
        children: &[(String, bool)],
    ) -> Result<(), String> {
        self.active_slot()?;
        let r = self
            .rt
            .with_permissions(|p| p.declare(&self.plugin, node, default, description, children))
            .unwrap_or_else(|| Err("the permission service is busy".into()));
        if let Err(e) = &r {
            self.log(LogLevel::Warn, &format!("permissions.declare: {e}"));
        }
        r
    }

    /// `permissions.has`.
    pub fn has_permission(&self, subject: CommandSender, node: &str) -> bool {
        self.rt.check_permission(subject, node)
    }

    // ---- services
    /// `services.provide`.
    pub fn provide_service(&self, iface: &str, priority: Priority) -> Result<(), String> {
        self.active_slot()?;
        self.rt.sh().services.provide(&self.plugin, iface, priority)
    }

    /// `services.provider`.
    pub fn service_provider(&self, iface: &str) -> Option<String> {
        self.async_api().service_provider(iface)
    }

    // ---- storage and player data
    /// `storage.get`.
    pub fn storage_get(&self, key: &str) -> Option<Vec<u8>> {
        self.async_api().storage_get(key)
    }
    /// `storage.set`.
    pub fn storage_set(&self, key: &str, value: Vec<u8>) {
        self.async_api().storage_set(key, value)
    }
    /// `storage.delete`.
    pub fn storage_delete(&self, key: &str) -> bool {
        self.async_api().storage_delete(key)
    }
    /// `storage.keys`.
    pub fn storage_keys(&self, prefix: &str) -> Vec<String> {
        self.async_api().storage_keys(prefix)
    }
    /// `players.get-data`.
    pub fn player_data_get(&self, player: Uuid, key: &str) -> Option<Vec<u8>> {
        self.async_api().player_data_get(player, key)
    }
    /// `players.set-data`.
    pub fn player_data_set(&self, player: Uuid, key: &str, value: Vec<u8>) {
        self.async_api().player_data_set(player, key, value)
    }
    /// `players.delete-data`.
    pub fn player_data_delete(&self, player: Uuid, key: &str) -> bool {
        self.async_api().player_data_delete(player, key)
    }
    /// The plugin's data folder.
    pub fn data_dir(&self) -> Option<PathBuf> {
        self.async_api().data_dir()
    }

    // ---- server
    /// `server.plugins`.
    pub fn plugins(&self) -> Vec<PluginMeta> {
        self.rt.plugin_metas()
    }

    /// `server.self-meta`.
    pub fn self_meta(&self) -> PluginMeta {
        self.rt
            .plugin_metas()
            .into_iter()
            .find(|m| m.id == self.plugin.as_str())
            .unwrap_or_else(|| PluginMeta {
                id: self.plugin.to_string(),
                name: self.plugin.to_string(),
                version: String::new(),
                enabled: false,
            })
    }

    // ---- owned game resources
    /// `menus.open` (the menu is closed when the plugin is disabled).
    pub fn menus_open(&self, player: Uuid, spec: &MenuSpec) -> Result<MenuId, HostError> {
        if spec.rows == 0 || spec.rows > 6 {
            return Err(HostError::Invalid("menus have 1 to 6 rows".into()));
        }
        let slot = self.active_slot().map_err(HostError::Other)?;
        let id = context::backend_call(|b| b.menus_open(player, spec))?;
        slot.resources.borrow_mut().menus.insert(id);
        Ok(id)
    }
    /// `menus.set-item` (own menus only).
    pub fn menus_set_item(
        &self,
        menu: MenuId,
        slot: u32,
        item: &ItemStack,
    ) -> Result<(), HostError> {
        self.owned(|r| r.menus.contains(&menu), "menu")?;
        context::backend_call(|b| b.menus_set_item(menu, slot, item))
    }
    /// `menus.set-title` (own menus only).
    pub fn menus_set_title(&self, menu: MenuId, title: &Text) -> Result<(), HostError> {
        self.owned(|r| r.menus.contains(&menu), "menu")?;
        context::backend_call(|b| b.menus_set_title(menu, title))
    }
    /// `menus.close` (own menus only).
    pub fn menus_close(&self, menu: MenuId) {
        if self.owned(|r| r.menus.contains(&menu), "menu").is_ok() {
            if let Some(slot) = self.rt.slot(&self.plugin) {
                slot.resources.borrow_mut().menus.remove(&menu);
            }
            let _ = context::backend_call(|b| b.menus_close(menu));
        }
    }
    /// `menus.open-menu`: the menu `player` has open, if it is one of this plugin's.
    pub fn menus_open_menu(&self, player: Uuid) -> Option<MenuId> {
        let m = context::backend_call(|b| b.menus_open_menu(player))
            .ok()
            .flatten()?;
        self.owned(|r| r.menus.contains(&m), "menu").ok().map(|_| m)
    }
    /// `bossbars.create` (removed when the plugin is disabled).
    pub fn bossbars_create(
        &self,
        title: &Text,
        color: BarColor,
        style: BarStyle,
        progress: f32,
    ) -> Result<BarId, HostError> {
        let slot = self.active_slot().map_err(HostError::Other)?;
        let p = if progress.is_finite() {
            progress.clamp(0.0, 1.0)
        } else {
            0.0
        };
        let id = context::backend_call(|b| b.bossbars_create(title, color, style, p))?;
        slot.resources.borrow_mut().bars.insert(id);
        Ok(id)
    }
    /// `bossbars.set-title`.
    pub fn bossbars_set_title(&self, bar: BarId, title: &Text) -> Result<(), HostError> {
        self.owned(|r| r.bars.contains(&bar), "boss bar")?;
        context::backend_call(|b| b.bossbars_set_title(bar, title))
    }
    /// `bossbars.set-progress` (clamped to 0..=1).
    pub fn bossbars_set_progress(&self, bar: BarId, progress: f32) -> Result<(), HostError> {
        self.owned(|r| r.bars.contains(&bar), "boss bar")?;
        let p = if progress.is_finite() {
            progress.clamp(0.0, 1.0)
        } else {
            0.0
        };
        context::backend_call(|b| b.bossbars_set_progress(bar, p))
    }
    /// `bossbars.set-color`.
    pub fn bossbars_set_color(&self, bar: BarId, color: BarColor) -> Result<(), HostError> {
        self.owned(|r| r.bars.contains(&bar), "boss bar")?;
        context::backend_call(|b| b.bossbars_set_color(bar, color))
    }
    /// `bossbars.show`.
    pub fn bossbars_show(&self, bar: BarId, player: Uuid) -> Result<(), HostError> {
        self.owned(|r| r.bars.contains(&bar), "boss bar")?;
        context::backend_call(|b| b.bossbars_show(bar, player))
    }
    /// `bossbars.hide`.
    pub fn bossbars_hide(&self, bar: BarId, player: Uuid) -> Result<(), HostError> {
        self.owned(|r| r.bars.contains(&bar), "boss bar")?;
        context::backend_call(|b| b.bossbars_hide(bar, player))
    }
    /// `bossbars.remove`.
    pub fn bossbars_remove(&self, bar: BarId) {
        if self.owned(|r| r.bars.contains(&bar), "boss bar").is_ok() {
            if let Some(slot) = self.rt.slot(&self.plugin) {
                slot.resources.borrow_mut().bars.remove(&bar);
            }
            let _ = context::backend_call(|b| b.bossbars_remove(bar));
        }
    }

    fn holders_of_channel(&self, channel: &str) -> usize {
        self.rt
            .inner
            .plugins
            .borrow()
            .records
            .values()
            .filter(|r| {
                r.slot
                    .resources
                    .try_borrow()
                    .is_ok_and(|x| x.channels.contains(channel))
            })
            .count()
    }

    /// `messaging.register-incoming`: `plugin-message` events of `channel` reach this plugin.
    pub fn messaging_register_incoming(&self, channel: &str) -> Result<(), String> {
        let channel = channel.to_ascii_lowercase();
        let valid = channel.split_once(':').is_some_and(|(ns, path)| {
            !ns.is_empty()
                && !path.is_empty()
                && channel.len() <= 128
                && ns
                    .bytes()
                    .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'-'))
                && path
                    .bytes()
                    .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'-' | b'/'))
        });
        if !valid {
            return Err(format!("`{channel}` is not a channel id (namespace:path)"));
        }
        let slot = self.active_slot()?;
        if slot.resources.borrow().channels.contains(&channel) {
            return Ok(());
        }
        if self.holders_of_channel(&channel) == 0 {
            context::backend_call(|b| b.messaging_register_incoming(&channel))
                .map_err(|e| e.to_string())?;
        }
        slot.resources.borrow_mut().channels.insert(channel);
        Ok(())
    }

    /// `messaging.unregister-incoming`.
    pub fn messaging_unregister_incoming(&self, channel: &str) {
        let channel = channel.to_ascii_lowercase();
        let Some(slot) = self.rt.slot(&self.plugin) else {
            return;
        };
        if !slot.resources.borrow_mut().channels.remove(&channel) {
            return;
        }
        if self.holders_of_channel(&channel) == 0 {
            let _ = context::backend_call(|b| b.messaging_unregister_incoming(&channel));
        }
    }

    /// `worlds.keep-chunk-loaded` (reference-counted across plugins, released on disable).
    pub fn worlds_keep_chunk_loaded(
        &self,
        world: &str,
        cx: i32,
        cz: i32,
        keep: bool,
    ) -> Result<(), HostError> {
        let slot = self.active_slot().map_err(HostError::Other)?;
        let key = (world.to_owned(), cx, cz);
        let holders = || {
            self.rt
                .inner
                .plugins
                .borrow()
                .records
                .values()
                .filter(|r| {
                    r.slot
                        .resources
                        .try_borrow()
                        .is_ok_and(|x| x.chunks.contains(&key))
                })
                .count()
        };
        let mine = slot.resources.borrow().chunks.contains(&key);
        if keep && !mine {
            if holders() == 0 {
                context::backend_call(|b| b.worlds_keep_chunk_loaded(world, cx, cz, true))?;
            }
            slot.resources.borrow_mut().chunks.insert(key.clone());
        } else if !keep && mine {
            slot.resources.borrow_mut().chunks.remove(&key);
            if holders() == 0 {
                context::backend_call(|b| b.worlds_keep_chunk_loaded(world, cx, cz, false))?;
            }
        }
        Ok(())
    }
}

/// Information about the plugin currently executing, for host functions that need its
/// manifest (capabilities for `http`, limits).
impl Rustic {
    /// A plugin's manifest.
    pub fn plugin_manifest(&self, id: &str) -> Option<Manifest> {
        self.manifest(id)
    }
}
