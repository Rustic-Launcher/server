//! The Rustic runtime on Horizon's server thread: started at the first server hook that has a
//! host, entered by every hook, stopped with the server.
//!
//! `rusticmc_core::Rustic` is `!Send` (an `Rc`); Horizon calls every hook that has a host, and the
//! no-host chat and move hooks, on its server thread (docs/ARCHITECTURE.md §2.1), so the runtime
//! lives in a thread local of that thread. Hooks on other threads (the connection threads'
//! `status_response`) never touch it: they hand their event over ([`crate::ping`]).
//!
//! Re-entrancy: a plugin's call can make Horizon fire a hook again (`teleport_player` ->
//! `player_teleport`). The outer entry holds the adapter's [`World`]; a nested entry gets a fresh
//! one whose queued writes join the outer queue ([`PENDING`]), and reads the same snapshot.
use std::cell::{Cell, RefCell};
use std::mem::ManuallyDrop;
use std::path::PathBuf;
use std::rc::Rc;

use horizon_client::mod_api::{Host, NativeServerPlayer};
use rusticmc_core::{Backend, Rustic};

use crate::backend::{Deferred, HorizonBackend, World};

thread_local! {
    /// `ManuallyDrop`: never dropped by the thread-local destructors at process exit, where Windows has
    /// already ended the host's worker threads (joining them would panic); [`stop`] drops them.
    static RUNTIME: ManuallyDrop<RefCell<Option<Rustic>>> = const { ManuallyDrop::new(RefCell::new(None)) };
    /// The outer entry's world (None while an entry holds it).
    static WORLD: RefCell<Option<World>> = RefCell::new(Some(World::default()));
    /// The players as of the last refresh, shared by nested entries.
    static SNAPSHOT: RefCell<Rc<Vec<NativeServerPlayer>>> = RefCell::new(Rc::new(Vec::new()));
    /// Writes queued by nested entries without a host.
    static PENDING: RefCell<Vec<Deferred>> = const { RefCell::new(Vec::new()) };
    /// The runtime could not start: do not try again every hook.
    static FAILED: Cell<bool> = const { Cell::new(false) };
    /// The WebAssembly host (its engine, epoch ticker and async pool live as long as it does).
    static HOST: ManuallyDrop<RefCell<Option<rusticmc_host::WasmHost>>> = const { ManuallyDrop::new(RefCell::new(None)) };
    static WATCHDOG: RefCell<Option<rusticmc_core::watchdog::Watchdog>> = const { RefCell::new(None) };
}

/// "Rustic 0.1.0 on Horizon (Minecraft 26.3)", for `/version`-like answers.
pub fn version_line() -> String {
    format!(
        "Rustic {} (rustic:api {}) on Horizon (Minecraft 26.3)",
        env!("CARGO_PKG_VERSION"),
        rusticmc_core::HOST_API,
    )
}

/// The players as of the last refresh.
pub fn snapshot() -> Rc<Vec<NativeServerPlayer>> {
    SNAPSHOT.with(|s| s.borrow().clone())
}

/// Takes a fresh snapshot of the game (once a tick, and before the runtime starts).
pub fn refresh(host: &mut dyn Host) {
    let players = Rc::new(host.server_players());
    SNAPSHOT.with(|s| *s.borrow_mut() = players);
    WORLD.with(|w| {
        if let Some(world) = w.borrow_mut().as_mut() {
            world.motd = host.motd();
            world.max_players = host.max_players();
        }
    });
}

/// Whether the runtime runs.
pub fn running() -> bool {
    RUNTIME.with(|r| r.borrow().is_some())
}

/// The runtime handle (cheap clone), when it runs.
pub fn runtime() -> Option<Rustic> {
    RUNTIME.with(|r| r.borrow().clone())
}

/// Puts the outer world back even when the entry panics.
struct Held(Option<World>, bool);
impl Drop for Held {
    fn drop(&mut self) {
        let Some(mut world) = self.0.take() else {
            return;
        };
        if self.1 {
            let deferred: Vec<Deferred> = std::mem::take(&mut world.deferred);
            PENDING.with(|p| p.borrow_mut().extend(deferred));
            if world.commands_changed {
                WORLD.with(|w| {
                    // The outer entry will see it when it gets its world back; failing that, the next tick.
                    if let Some(outer) = w.borrow_mut().as_mut() {
                        outer.commands_changed = true;
                    }
                });
            }
        } else {
            WORLD.with(|w| *w.borrow_mut() = Some(world));
        }
    }
}

/// Runs `f` with the runtime and a backend over `host` (or the snapshot and the write queue when
/// there is none). None when the runtime does not run.
pub fn enter<R>(
    host: Option<&mut dyn Host>,
    f: impl FnOnce(&Rustic, &mut dyn Backend) -> R,
) -> Option<R> {
    let rustic = runtime()?;
    let outer = WORLD.with(|w| w.borrow_mut().take());
    let nested = outer.is_none();
    let mut held = Held(Some(outer.unwrap_or_default()), nested);
    let world = held.0.as_mut().expect("held");
    let mut backend = HorizonBackend::new(host, world);
    Some(f(&rustic, &mut backend))
}

/// Starts the runtime: `rustic.toml` and `plugins/` in the server folder (next to
/// `server.properties`), the plugins loaded and enabled. Once; a failure is logged and the server
/// goes on without plugins.
pub fn start(host: &mut dyn Host) {
    if running() || FAILED.with(Cell::get) {
        return;
    }
    let dir = host
        .server_dir()
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    let wasm = match rusticmc_host::WasmHost::new(rusticmc_host::HostConfig::for_server(&dir)) {
        Ok(wasm) => wasm,
        Err(e) => {
            log::error!(target: "rustic", "Rustic did not start: the WebAssembly host failed: {e}");
            FAILED.with(|f| f.set(true));
            return;
        }
    };
    let loader = crate::demo::Loader::new(wasm.loader());
    #[cfg(feature = "bridge")]
    let bridge = rusticmc_bridge::Bridge::new(&dir)
        .map_err(|e| log::error!(target: "rustic", "Rustic Bridge disabled: {e}"))
        .ok();
    #[cfg(feature = "bridge")]
    let runtime_loader: Box<dyn rusticmc_core::PluginLoader> = match &bridge {
        Some(b) => Box::new(b.loader(Some(Box::new(loader.clone())))),
        None => Box::new(loader.clone()),
    };
    #[cfg(not(feature = "bridge"))]
    let runtime_loader: Box<dyn rusticmc_core::PluginLoader> = Box::new(loader.clone());
    let rustic = match Rustic::new(&dir, runtime_loader) {
        Ok(rustic) => rustic,
        Err(e) => {
            log::error!(target: "rustic", "Rustic did not start ({}): {e}", dir.display());
            FAILED.with(|f| f.set(true));
            return;
        }
    };
    wasm.install(&rustic);
    HOST.with(|h| *h.borrow_mut() = Some(wasm));
    log::info!(target: "rustic", "{} starting in {}", version_line(), dir.display());
    crate::hooks::install_panic_filter();
    if let Err(e) = std::fs::create_dir_all(rustic.layout().plugins_dir()) {
        log::warn!(target: "rustic", "cannot create the plugins folder: {e}");
    }
    match rustic.spawn_watchdog() {
        Ok(watchdog) => WATCHDOG.with(|w| *w.borrow_mut() = watchdog),
        Err(e) => log::warn!(target: "rustic", "no watchdog: {e}"),
    }
    RUNTIME.with(|r| *r.borrow_mut() = Some(rustic));
    refresh(host);
    enter(Some(host), |rustic, backend| {
        let mut report = rustic.load_plugins(backend);
        if crate::demo::enabled() {
            let demo = rustic.load_packages(crate::demo::packages(&loader), backend);
            report.loaded.extend(demo.loaded);
            report.failed.extend(demo.failed);
        }
        #[cfg(feature = "bridge")]
        if let Some(bridge) = &bridge {
            let bukkit = rustic.load_packages(bridge.packages(), backend);
            report.loaded.extend(bukkit.loaded);
            report.failed.extend(bukkit.failed);
        }
        for (id, why) in &report.failed {
            log::error!(target: "rustic", "plugin {id} not loaded: {why}");
        }
        let enabled = rustic.enable_all(backend);
        let ok = enabled.iter().filter(|(_, r)| r.is_ok()).count();
        for (id, r) in &enabled {
            if let Err(e) = r {
                log::error!(target: "rustic", "plugin {id} not enabled: {e}");
            }
        }
        log::info!(
            target: "rustic",
            "Rustic ready: {ok} plugin(s) enabled ({} loaded, {} failed)",
            report.loaded.len(),
            report.failed.len()
        );
    });
}

/// Runs the writes queued without a host (by the outer world and by nested entries), then the
/// commands' re-registration they asked for.
pub fn flush(host: &mut dyn Host) {
    let pending: Vec<Deferred> = PENDING.with(|p| p.borrow_mut().drain(..).collect());
    for write in pending {
        write(host);
    }
    let (deferred, refresh_commands) = WORLD.with(|w| match w.borrow_mut().as_mut() {
        Some(world) => (
            std::mem::take(&mut world.deferred),
            std::mem::take(&mut world.commands_changed),
        ),
        None => (Vec::new(), false),
    });
    for write in deferred {
        write(host);
    }
    if refresh_commands {
        host.refresh_commands();
    }
}

/// Disables every plugin (the server is stopping).
pub fn stop(host: &mut dyn Host) {
    enter(Some(host), |rustic, backend| {
        if !rustic.is_shut_down() {
            log::info!(target: "rustic", "Rustic stopping: disabling plugins");
            rustic.shutdown(backend);
        }
    });
    WATCHDOG.with(|w| w.borrow_mut().take());
    // The runtime, then the host (its async workers, its epoch ticker), while the process runs.
    // An entry still running holds its own clone of the runtime (an `Rc`): it goes when that ends.
    drop(RUNTIME.with(|r| r.borrow_mut().take()));
    drop(HOST.with(|h| h.borrow_mut().take()));
}
