//! [`WasmPlugin`]: the core's [`PluginInstance`] for a component — the main instance (called
//! on the server thread by the core) plus the plugin's async side in the pool.

use crate::engine::HostCore;
use crate::imports::services::ProviderCall;
use crate::instance::WasmInstance;
use crate::pool::{AsyncEntry, AsyncPool};
use rusticmc_core::model::{
    CommandContext, CommandId, CompleteContext, Event, EventKind, HandlerId, TaskId, Text,
};
use rusticmc_core::{PluginFault, PluginId, PluginInstance, context};
use std::any::Any;
use std::collections::HashSet;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Weak};

/// A loaded WebAssembly plugin.
pub struct WasmPlugin {
    id: PluginId,
    main: WasmInstance,
    entry: Arc<AsyncEntry>,
    pool: Weak<AsyncPool>,
    core: Arc<HostCore>,
    warned_handlers: HashSet<EventKind>,
}

impl std::fmt::Debug for WasmPlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WasmPlugin")
            .field("id", &self.id)
            .finish_non_exhaustive()
    }
}

/// Marks the main instance busy for the async workers (see `pool`).
struct Busy<'a>(&'a AsyncEntry);

impl<'a> Busy<'a> {
    fn new(e: &'a AsyncEntry) -> Self {
        e.main_busy.store(true, Ordering::Release);
        Busy(e)
    }
}

impl Drop for Busy<'_> {
    fn drop(&mut self) {
        self.0.main_busy.store(false, Ordering::Release);
    }
}

impl WasmPlugin {
    pub(crate) fn new(
        id: PluginId,
        main: WasmInstance,
        entry: Arc<AsyncEntry>,
        pool: Weak<AsyncPool>,
        core: Arc<HostCore>,
    ) -> Self {
        Self {
            id,
            main,
            entry,
            pool,
            core,
            warned_handlers: HashSet::new(),
        }
    }

    /// The plugin id.
    pub fn id(&self) -> &PluginId {
        &self.id
    }

    /// Fuel left after the last main-instance call (diagnostics, benches).
    pub fn fuel_left(&self) -> u64 {
        self.main.fuel_left()
    }

    /// Linear memory currently held by the main and async instances, in bytes.
    pub fn memory_bytes(&self) -> usize {
        let main = self.main.store.data().limiter.memory_total;
        let async_side = self
            .entry
            .instance
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|i| i.store.data().limiter.memory_total))
            .unwrap_or(0);
        main + async_side
    }

    /// Call a service interface this plugin exports (on the main instance). `Ok(None)`: the
    /// component does not export it.
    pub(crate) fn call_service<R>(
        &mut self,
        iface: &str,
        f: ProviderCall<'_, R>,
    ) -> Result<
        Option<Result<R, crate::bindings::rustic::services::common::ServiceError>>,
        PluginFault,
    > {
        let _busy = Busy::new(&self.entry);
        self.main
            .call(iface, move |store, svc| match f(svc, store) {
                Some(Ok(v)) => Ok(Some(v)),
                Some(Err(e)) => Err(e),
                None => Ok(None),
            })
    }

    fn refresh_snapshot(&self) {
        let worlds = context::backend_call(|b| b.worlds_all());
        let proxy = context::backend_call(|b| b.messaging_behind_proxy());
        self.core.update_snapshot(|s| {
            if let Ok(w) = worlds {
                s.worlds = w;
            }
            if let Ok(p) = proxy {
                s.behind_proxy = p;
            }
        });
    }
}

impl Drop for WasmPlugin {
    fn drop(&mut self) {
        self.entry.retire();
        if let Some(pool) = self.pool.upgrade() {
            pool.unregister(&self.entry);
        }
    }
}

impl PluginInstance for WasmPlugin {
    fn on_load(&mut self) -> Result<Result<(), String>, PluginFault> {
        let r = {
            let _busy = Busy::new(&self.entry);
            self.main.on_load()?
        };
        if r.is_err() {
            return Ok(r);
        }
        // The SDK relies on on-load / on-enable running in both instances.
        self.entry
            .with_instance(|i| i.on_load())
            .map_err(prefix_async)
            .map(|r| r.map_err(|e| format!("async instance: {e}")))
    }

    fn on_enable(&mut self) -> Result<Result<(), String>, PluginFault> {
        self.refresh_snapshot();
        let r = {
            let _busy = Busy::new(&self.entry);
            self.main.on_enable()?
        };
        if r.is_err() {
            return Ok(r);
        }
        let r = self
            .entry
            .with_instance(|i| i.on_enable())
            .map_err(prefix_async)?
            .map_err(|e| format!("async instance: {e}"));
        if r.is_ok() {
            self.entry.ready.store(true, Ordering::Release);
        }
        Ok(r)
    }

    fn on_disable(&mut self) -> Result<(), PluginFault> {
        // Stop async work first: the async instance is dropped with the plugin.
        self.entry.retire();
        let _busy = Busy::new(&self.entry);
        self.main.on_disable()
    }

    fn run_task(&mut self, task: TaskId) -> Result<(), PluginFault> {
        let _busy = Busy::new(&self.entry);
        self.main.run_task(task)
    }

    fn run_job(&mut self, task: TaskId, job: &str, payload: &[u8]) -> Result<(), PluginFault> {
        // The core never calls this on the main instance; jobs run in the async pool.
        self.entry
            .with_instance(|i| i.run_job(task, job, payload))
            .map_err(prefix_async)
    }

    fn execute_command(
        &mut self,
        command: CommandId,
        ctx: CommandContext,
    ) -> Result<Result<(), Text>, PluginFault> {
        let _busy = Busy::new(&self.entry);
        self.main.execute_command(command, ctx)
    }

    fn complete_command(
        &mut self,
        command: CommandId,
        ctx: CompleteContext,
    ) -> Result<Vec<String>, PluginFault> {
        let _busy = Busy::new(&self.entry);
        self.main.complete_command(command, ctx)
    }

    fn on_message(&mut self, topic: &str, payload: &[u8]) -> Result<(), PluginFault> {
        let _busy = Busy::new(&self.entry);
        self.main.on_message(topic, payload)
    }

    fn handle_event(&mut self, handler: HandlerId, event: &Event) -> Result<Event, PluginFault> {
        let _busy = Busy::new(&self.entry);
        match self.main.handle_event(handler, event) {
            Some(r) => r,
            None => {
                if self.warned_handlers.insert(event.kind()) {
                    log::warn!(
                        target: "rustic",
                        "[{}] subscribed to {} but its component has no `handle-{}` export \
                         (built against an older API); events are not delivered",
                        self.id,
                        event.name(),
                        event.name()
                    );
                }
                Ok(event.clone())
            }
        }
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(self)
    }
}

fn prefix_async(f: PluginFault) -> PluginFault {
    let msg = format!("async instance: {}", f.message());
    let bt = f.backtrace().map(str::to_owned);
    let g = match f {
        PluginFault::Trap { .. } => PluginFault::trap(msg),
        PluginFault::Panic { .. } => PluginFault::panic(msg),
        PluginFault::FuelExhausted { .. } => PluginFault::fuel_exhausted(msg),
        PluginFault::Timeout { .. } => PluginFault::timeout(msg),
        PluginFault::MemoryExceeded { .. } => PluginFault::memory_exceeded(msg),
        PluginFault::Other { .. } => PluginFault::other(msg),
    };
    match bt {
        Some(b) => g.with_backtrace(b),
        None => g,
    }
}
