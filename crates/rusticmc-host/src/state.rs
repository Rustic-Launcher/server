//! Per-store data ([`HostState`]): which plugin and instance a store belongs to, its WASI
//! context, resource limiter and call clock. Import implementations (`imports`) are methods on
//! it.

use crate::engine::HostCore;
use crate::output::LogSink;
use rusticmc_core::manifest::Capabilities;
use rusticmc_core::{AsyncApi, PluginId, SharedHandle};
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use wasmtime::component::ResourceTable;
use wasmtime::{ResourceLimiter, format_err};
use wasmtime_wasi::{WasiCtx, WasiCtxView, WasiView};

/// What a plugin is, shared by its two stores.
pub(crate) struct PluginMetaInfo {
    pub id: PluginId,
    pub name: String,
    pub version: String,
    pub capabilities: Capabilities,
}

/// Store data of one instance (main or async) of one plugin. `Send` (async stores move to
/// worker threads).
pub(crate) struct HostState {
    pub plugin: PluginId,
    pub is_main: bool,
    pub meta: Arc<PluginMetaInfo>,
    pub host: Arc<HostCore>,
    pub shared: SharedHandle,
    pub async_api: AsyncApi,
    pub wasi: WasiCtx,
    pub table: ResourceTable,
    pub limiter: Limiter,
    pub stderr: LogSink,
    pub clock: CallClock,
    /// Functions already warned about (once per store), e.g. world calls from async.
    pub warned: HashSet<&'static str>,
}

impl HostState {
    /// Log a warning once per store and function.
    pub(crate) fn warn_once(&mut self, what: &'static str, message: &str) {
        if self.warned.insert(what) {
            log::warn!(target: "rustic", "[{}] {what}: {message} (warned once)", self.plugin);
        }
    }
}

impl WasiView for HostState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

/// Wall-clock accounting of the current call for the epoch callback: the deadline counts time
/// spent in WebAssembly and in non-blocking host calls, not time blocked in HTTP.
pub(crate) struct CallClock {
    pub started: Instant,
    pub blocked: Duration,
    pub deadline: Duration,
    /// Set from another thread to stop the async instance's current call (plugin disabled).
    pub cancel: Arc<AtomicBool>,
    /// Why the last call was interrupted by the epoch callback, if it was.
    pub interrupted: Option<Interrupt>,
}

/// Why the epoch callback stopped a call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Interrupt {
    Deadline,
    Cancelled,
}

impl CallClock {
    pub(crate) fn new(deadline: Duration, cancel: Arc<AtomicBool>) -> Self {
        Self {
            started: Instant::now(),
            blocked: Duration::ZERO,
            deadline,
            cancel,
            interrupted: None,
        }
    }

    /// Reset at the start of a call.
    pub(crate) fn start(&mut self) {
        self.started = Instant::now();
        self.blocked = Duration::ZERO;
        self.interrupted = None;
    }

    /// The time the call has really been running.
    pub(crate) fn busy(&self) -> Duration {
        self.started.elapsed().saturating_sub(self.blocked)
    }

    /// Epoch callback decision: `None` to stop the call, or how much time is left.
    pub(crate) fn check(&mut self) -> Option<Duration> {
        if self.cancel.load(Ordering::Relaxed) {
            self.interrupted = Some(Interrupt::Cancelled);
            return None;
        }
        let busy = self.busy();
        if busy >= self.deadline {
            self.interrupted = Some(Interrupt::Deadline);
            None
        } else {
            Some(self.deadline - busy)
        }
    }
}

/// Memory, table and instance caps of one store. Exceeding memory traps at once (wasmtime
/// "trap on grow failure") and records why, so the fault is reported as `MemoryExceeded`.
pub(crate) struct Limiter {
    pub memory_limit: usize,
    pub memory_total: usize,
    pub max_instances: usize,
    pub max_tables: usize,
    pub max_memories: usize,
    pub max_table_elements: usize,
    pub exceeded: Option<String>,
}

impl ResourceLimiter for Limiter {
    fn memory_growing(
        &mut self,
        current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        let total = self
            .memory_total
            .saturating_sub(current)
            .saturating_add(desired);
        if total > self.memory_limit {
            let msg = format!(
                "linear memory limit of {} MiB exceeded (growing to {} MiB)",
                self.memory_limit >> 20,
                total.div_ceil(1 << 20)
            );
            self.exceeded = Some(msg.clone());
            return Err(format_err!("{msg}"));
        }
        self.memory_total = total;
        Ok(true)
    }

    fn table_growing(
        &mut self,
        _current: usize,
        desired: usize,
        _maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        if desired > self.max_table_elements {
            let msg = format!(
                "table limit of {} elements exceeded (growing to {desired})",
                self.max_table_elements
            );
            self.exceeded = Some(msg.clone());
            return Err(format_err!("{msg}"));
        }
        Ok(true)
    }

    fn instances(&self) -> usize {
        self.max_instances
    }

    fn tables(&self) -> usize {
        self.max_tables
    }

    fn memories(&self) -> usize {
        self.max_memories
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limiter(mb: usize) -> Limiter {
        Limiter {
            memory_limit: mb << 20,
            memory_total: 0,
            max_instances: 10,
            max_tables: 10,
            max_memories: 10,
            max_table_elements: 100,
            exceeded: None,
        }
    }

    #[test]
    fn memory_limit_counts_all_memories() {
        let mut l = limiter(2);
        assert!(l.memory_growing(0, 1 << 20, None).unwrap());
        assert!(l.memory_growing(0, 1 << 19, None).unwrap());
        assert!(l.memory_growing(1 << 19, 1 << 20, None).unwrap());
        assert!(l.memory_growing(1 << 20, 3 << 19, None).is_err());
        assert!(l.exceeded.as_deref().unwrap().contains("2 MiB"));
    }

    #[test]
    fn table_limit() {
        let mut l = limiter(1);
        assert!(l.table_growing(0, 100, None).unwrap());
        assert!(l.table_growing(100, 101, None).is_err());
    }

    #[test]
    fn call_clock_deadline_and_cancel() {
        let cancel = Arc::new(AtomicBool::new(false));
        let mut c = CallClock::new(Duration::from_millis(50), cancel.clone());
        c.start();
        assert!(c.check().is_some());
        c.blocked = Duration::from_secs(10); // blocked time does not count
        assert!(c.check().is_some());
        c.deadline = Duration::ZERO;
        assert_eq!(c.check(), None);
        assert_eq!(c.interrupted, Some(Interrupt::Deadline));
        c.deadline = Duration::from_secs(1);
        cancel.store(true, Ordering::Relaxed);
        assert_eq!(c.check(), None);
        assert_eq!(c.interrupted, Some(Interrupt::Cancelled));
    }
}
