//! WebAssembly plugins in the harness, through `rusticmc-host`.
//!
//! The host is created lazily, at the first `.wasm` load, so servers running only native
//! plugins start no wasmtime engine and no async pool. Compiled components are cached on disk
//! (`<tmp>/rusticmc-test-cache`), so the many servers of a test run compile a plugin once.
//!
//! Async tasks of WebAssembly plugins run in the host's async pool (one worker, so in task
//! order) on the fake clock; [`WasmSide::wait_idle`] waits until the pool has nothing due and
//! nothing running, which makes `tick()` deterministic for tests.

use crate::harness::{Harness, WasmSide};
use rusticmc_core::PluginId;
use rusticmc_core::clock::Clock;
use rusticmc_core::lifecycle::{LoadRequest, PluginInstance, PluginLoader};
use rusticmc_host::{HostConfig, WasmHost};
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

type Slot = Rc<RefCell<Option<(WasmHost, Box<dyn PluginLoader>)>>>;

struct LazyLoader {
    slot: Slot,
}

fn host_config() -> HostConfig {
    HostConfig {
        cache_dir: Some(std::env::temp_dir().join("rusticmc-test-cache")),
        async_workers: Some(1),
        async_poll_interval: Duration::from_millis(1),
        ..HostConfig::default()
    }
}

impl PluginLoader for LazyLoader {
    fn load(&mut self, req: &LoadRequest<'_>) -> Result<Box<dyn PluginInstance>, String> {
        let mut slot = self.slot.borrow_mut();
        if slot.is_none() {
            let host = WasmHost::new(host_config()).map_err(|e| e.to_string())?;
            let loader = host.loader();
            *slot = Some((host, loader));
        }
        let (_, loader) = slot.as_mut().expect("just created");
        loader.load(req)
    }

    fn unloaded(&mut self, plugin: &PluginId) {
        if let Some((_, l)) = self.slot.borrow_mut().as_mut() {
            l.unloaded(plugin);
        }
    }
}

struct Side {
    slot: Slot,
    /// (scheduled async tasks, tasks run) at the end of the last wait.
    last: Cell<(usize, u64)>,
}

impl WasmSide for Side {
    fn active(&self) -> bool {
        self.slot.borrow().is_some()
    }

    fn wait_idle(&self, h: &Harness, became_due: bool) -> usize {
        let Some(host) = self.slot.borrow().as_ref().map(|(h, _)| h.clone()) else {
            return 0;
        };
        let shared = h.rt.shared();
        let scheduled = || -> usize {
            h.rt.plugins()
                .iter()
                .map(|p| shared.scheduler().count(&p.id).1)
                .sum()
        };
        let due = || {
            shared
                .scheduler()
                .next_async_deadline()
                .is_some_and(|d| d <= h.clock.now())
        };
        let start_runs = host.stats().async_tasks_run;
        let (last_scheduled, last_runs) = self.last.get();
        let now_scheduled = scheduled();
        let busy =
            became_due || due() || now_scheduled != last_scheduled || start_runs != last_runs;
        if now_scheduled == 0 && start_runs == last_runs || !busy {
            self.last.set((now_scheduled, start_runs));
            return 0;
        }
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut last = start_runs;
        while Instant::now() < deadline {
            let runs = host.stats().async_tasks_run;
            if runs != last {
                last = runs;
            }
            // Idle: nothing due in the scheduler (read first) and no task taken by the pool left
            // to finish. Exact, not timing based: the pool counts a task from the moment it leaves
            // the scheduler until it has run (fault reported included).
            if !due() && host.stats().async_in_flight == 0 {
                break;
            }
            std::thread::sleep(Duration::from_micros(500));
        }
        self.last.set((scheduled(), host.stats().async_tasks_run));
        (last - start_runs) as usize
    }
}

/// The lazily created WebAssembly loader and its async side.
pub(crate) fn loader() -> (Box<dyn PluginLoader>, Rc<dyn WasmSide>) {
    let slot: Slot = Rc::default();
    (
        Box::new(LazyLoader { slot: slot.clone() }),
        Rc::new(Side {
            slot,
            last: Cell::new((0, 0)),
        }),
    )
}
