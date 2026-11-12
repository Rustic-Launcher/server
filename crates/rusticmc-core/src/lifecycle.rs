//! Plugin lifecycle: states, faults, and the [`PluginInstance`] / [`PluginLoader`] traits the
//! host implements.

use crate::archive::PluginPackage;
use crate::config::EffectiveLimits;
use crate::id::PluginId;
use crate::model::{
    BarId, CommandContext, CommandId, CompleteContext, Event, HandlerId, MenuId, TaskId, Text,
};
use crate::runtime::SharedHandle;
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::fmt;
use std::path::Path;

/// Where a plugin is in its life.
///
/// `Discovered → Validated → Loaded → Enabled → Disabling → Disabled`, or `Failed` from any
/// state (refused at validation/resolution, error in `on-load`/`on-enable`, or a fault).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PluginState {
    /// Found on disk, manifest not yet checked.
    Discovered,
    /// Manifest valid and dependencies resolved; not instantiated.
    Validated,
    /// Instantiated, `on-load` done.
    Loaded,
    /// `on-enable` done: receives events, commands and tasks.
    Enabled,
    /// Being disabled (its `on-disable` may still be pending).
    Disabling,
    /// Disabled (explicitly, for a reload, or because a dependency was disabled).
    Disabled,
    /// Refused or crashed; see the failure reason.
    Failed,
}

impl fmt::Display for PluginState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            PluginState::Discovered => "discovered",
            PluginState::Validated => "validated",
            PluginState::Loaded => "loaded",
            PluginState::Enabled => "enabled",
            PluginState::Disabling => "disabling",
            PluginState::Disabled => "disabled",
            PluginState::Failed => "failed",
        })
    }
}

/// A plugin misbehaved (the call did not complete). The plugin is disabled; the server goes on.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PluginFault {
    /// A WebAssembly trap (unreachable, out-of-bounds access...).
    #[error("trap: {message}")]
    Trap {
        /// What happened.
        message: String,
        /// Guest backtrace, if available.
        backtrace: Option<String>,
    },
    /// A panic (in native test plugins, or caught in host code).
    #[error("panic: {message}")]
    Panic {
        /// Panic message.
        message: String,
        /// Backtrace, if available.
        backtrace: Option<String>,
    },
    /// The call ran out of fuel.
    #[error("fuel exhausted: {message}")]
    FuelExhausted {
        /// Details.
        message: String,
        /// Backtrace, if available.
        backtrace: Option<String>,
    },
    /// The epoch deadline passed (infinite loop, too slow).
    #[error("timeout: {message}")]
    Timeout {
        /// Details.
        message: String,
        /// Backtrace, if available.
        backtrace: Option<String>,
    },
    /// The memory limit was hit.
    #[error("memory exceeded: {message}")]
    MemoryExceeded {
        /// Details.
        message: String,
        /// Backtrace, if available.
        backtrace: Option<String>,
    },
    /// Anything else (instantiation problem, host invariant broken...).
    #[error("{message}")]
    Other {
        /// Details.
        message: String,
        /// Backtrace, if available.
        backtrace: Option<String>,
    },
}

impl PluginFault {
    /// A trap.
    pub fn trap(message: impl Into<String>) -> Self {
        PluginFault::Trap {
            message: message.into(),
            backtrace: None,
        }
    }
    /// A panic.
    pub fn panic(message: impl Into<String>) -> Self {
        PluginFault::Panic {
            message: message.into(),
            backtrace: None,
        }
    }
    /// Out of fuel.
    pub fn fuel_exhausted(message: impl Into<String>) -> Self {
        PluginFault::FuelExhausted {
            message: message.into(),
            backtrace: None,
        }
    }
    /// Deadline passed.
    pub fn timeout(message: impl Into<String>) -> Self {
        PluginFault::Timeout {
            message: message.into(),
            backtrace: None,
        }
    }
    /// Memory limit.
    pub fn memory_exceeded(message: impl Into<String>) -> Self {
        PluginFault::MemoryExceeded {
            message: message.into(),
            backtrace: None,
        }
    }
    /// Anything else.
    pub fn other(message: impl Into<String>) -> Self {
        PluginFault::Other {
            message: message.into(),
            backtrace: None,
        }
    }

    /// Attach a backtrace.
    pub fn with_backtrace(mut self, bt: impl Into<String>) -> Self {
        let slot = match &mut self {
            PluginFault::Trap { backtrace, .. }
            | PluginFault::Panic { backtrace, .. }
            | PluginFault::FuelExhausted { backtrace, .. }
            | PluginFault::Timeout { backtrace, .. }
            | PluginFault::MemoryExceeded { backtrace, .. }
            | PluginFault::Other { backtrace, .. } => backtrace,
        };
        *slot = Some(bt.into());
        self
    }

    /// The message.
    pub fn message(&self) -> &str {
        match self {
            PluginFault::Trap { message, .. }
            | PluginFault::Panic { message, .. }
            | PluginFault::FuelExhausted { message, .. }
            | PluginFault::Timeout { message, .. }
            | PluginFault::MemoryExceeded { message, .. }
            | PluginFault::Other { message, .. } => message,
        }
    }

    /// The backtrace, if any.
    pub fn backtrace(&self) -> Option<&str> {
        match self {
            PluginFault::Trap { backtrace, .. }
            | PluginFault::Panic { backtrace, .. }
            | PluginFault::FuelExhausted { backtrace, .. }
            | PluginFault::Timeout { backtrace, .. }
            | PluginFault::MemoryExceeded { backtrace, .. }
            | PluginFault::Other { backtrace, .. } => backtrace.as_deref(),
        }
    }

    /// Short kind name ("trap", "panic", "fuel-exhausted", "timeout", "memory-exceeded", "other").
    pub fn kind(&self) -> &'static str {
        match self {
            PluginFault::Trap { .. } => "trap",
            PluginFault::Panic { .. } => "panic",
            PluginFault::FuelExhausted { .. } => "fuel-exhausted",
            PluginFault::Timeout { .. } => "timeout",
            PluginFault::MemoryExceeded { .. } => "memory-exceeded",
            PluginFault::Other { .. } => "other",
        }
    }

    /// Build a fault from a caught panic payload.
    pub fn from_panic(payload: Box<dyn Any + Send>) -> Self {
        let message = if let Some(s) = payload.downcast_ref::<&str>() {
            (*s).to_owned()
        } else if let Some(s) = payload.downcast_ref::<String>() {
            s.clone()
        } else {
            "unknown panic payload".to_owned()
        };
        PluginFault::panic(message)
    }
}

/// A running plugin, as the core sees it. Implemented by `rusticmc-host` for WASM components
/// (and by [`crate::native::NativePlugin`] for tests). Every method maps to an export of
/// `rustic:api/plugin` / `handlers`; `Err(PluginFault)` means the call did not complete and the
/// plugin is disabled.
///
/// The core guarantees: calls happen on the thread that owns the runtime, never re-entrantly on
/// the same instance, with [`crate::context`] set (backend, call context, runtime).
pub trait PluginInstance {
    /// `plugin.on-load`. `Ok(Err(msg))`: the plugin refused to load.
    fn on_load(&mut self) -> Result<Result<(), String>, PluginFault>;
    /// `plugin.on-enable`. `Ok(Err(msg))`: the plugin refused to enable.
    fn on_enable(&mut self) -> Result<Result<(), String>, PluginFault>;
    /// `plugin.on-disable`.
    fn on_disable(&mut self) -> Result<(), PluginFault>;
    /// `plugin.run-task` (main instance).
    fn run_task(&mut self, task: TaskId) -> Result<(), PluginFault>;
    /// `plugin.run-job` (async instance; the core never calls it on the main instance: the
    /// host's async pool runs jobs handed out by [`crate::SharedHandle::due_async_tasks`]).
    fn run_job(&mut self, task: TaskId, job: &str, payload: &[u8]) -> Result<(), PluginFault> {
        let _ = (task, payload);
        Err(PluginFault::other(format!(
            "run-job `{job}` is not implemented by this instance"
        )))
    }
    /// `plugin.execute-command`. `Ok(Err(text))` is shown to the sender in red.
    fn execute_command(
        &mut self,
        command: CommandId,
        ctx: CommandContext,
    ) -> Result<Result<(), Text>, PluginFault>;
    /// `plugin.complete-command`.
    fn complete_command(
        &mut self,
        command: CommandId,
        ctx: CompleteContext,
    ) -> Result<Vec<String>, PluginFault>;
    /// `plugin.on-message`.
    fn on_message(&mut self, topic: &str, payload: &[u8]) -> Result<(), PluginFault>;
    /// `handlers.handle-<event>`: returns the (possibly changed) event.
    fn handle_event(&mut self, handler: HandlerId, event: &Event) -> Result<Event, PluginFault>;
    /// Downcast hook for the host (service calls into its own instance type).
    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        None
    }
}

/// What a loader gets to instantiate a plugin.
pub struct LoadRequest<'a> {
    /// The validated package (read the component with [`PluginPackage::read_entry`]).
    pub package: &'a PluginPackage,
    /// Limits for this plugin (manifest overrides capped by the server config).
    pub limits: EffectiveLimits,
    /// The plugin's data folder (exists).
    pub data_dir: &'a Path,
    /// Thread-safe runtime state for the async instance (scheduler, storage, services...).
    pub shared: SharedHandle,
}

/// Creates instances from validated packages. Implemented by `rusticmc-host`; see
/// [`crate::native::NativeLoader`] for the in-process implementation used by tests.
pub trait PluginLoader {
    /// Instantiate (compile, link, instantiate the main instance). Do not call `on-load`: the
    /// core does.
    fn load(&mut self, req: &LoadRequest<'_>) -> Result<Box<dyn PluginInstance>, String>;
    /// The plugin's instance was dropped (disable, reload, shutdown): release async instances
    /// and caches.
    fn unloaded(&mut self, plugin: &PluginId) {
        let _ = plugin;
    }
}

/// Things a plugin owns in the game, released when it is disabled.
#[derive(Debug, Default)]
pub(crate) struct OwnedResources {
    pub menus: BTreeSet<MenuId>,
    pub bars: BTreeSet<BarId>,
    pub channels: BTreeSet<String>,
    pub chunks: BTreeSet<(String, i32, i32)>,
}

/// Runtime slot of one plugin incarnation (a reload creates a new slot).
pub(crate) struct PluginSlot {
    pub id: PluginId,
    pub index: u32,
    pub state: Cell<PluginState>,
    pub instance: RefCell<Option<Box<dyn PluginInstance>>>,
    pub resources: RefCell<OwnedResources>,
    /// `on-disable` still to call once the instance is free (deferred disable).
    pub pending_on_disable: Cell<bool>,
}

impl PluginSlot {
    pub fn new(id: PluginId, index: u32) -> Self {
        Self {
            id,
            index,
            state: Cell::new(PluginState::Validated),
            instance: RefCell::new(None),
            resources: RefCell::new(OwnedResources::default()),
            pending_on_disable: Cell::new(false),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.state.get() == PluginState::Enabled
    }

    /// Loaded or enabled: may register things.
    pub fn is_active(&self) -> bool {
        matches!(self.state.get(), PluginState::Loaded | PluginState::Enabled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn faults() {
        let f = PluginFault::trap("unreachable").with_backtrace("at f");
        assert_eq!(f.kind(), "trap");
        assert_eq!(f.message(), "unreachable");
        assert_eq!(f.backtrace(), Some("at f"));
        assert_eq!(f.to_string(), "trap: unreachable");
        for (f, k) in [
            (PluginFault::panic("x"), "panic"),
            (PluginFault::fuel_exhausted("x"), "fuel-exhausted"),
            (PluginFault::timeout("x"), "timeout"),
            (PluginFault::memory_exceeded("x"), "memory-exceeded"),
            (PluginFault::other("x"), "other"),
        ] {
            assert_eq!(f.kind(), k);
            assert!(f.backtrace().is_none());
        }
        let p = std::panic::catch_unwind(|| panic!("boom {}", 1)).unwrap_err();
        assert_eq!(PluginFault::from_panic(p).message(), "boom 1");
        let p = std::panic::catch_unwind(|| std::panic::panic_any(5u8)).unwrap_err();
        assert_eq!(
            PluginFault::from_panic(p).message(),
            "unknown panic payload"
        );
        assert_eq!(PluginState::Enabled.to_string(), "enabled");
    }
}
