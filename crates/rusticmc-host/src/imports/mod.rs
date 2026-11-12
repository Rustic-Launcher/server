//! Implementations of every `rustic:api` and `rustic:services` import on [`HostState`].
//!
//! * **Main instance**: core-owned interfaces go through [`rusticmc_core::context::api`] (the
//!   `PluginApi` of the plugin executing), game calls through
//!   [`rusticmc_core::context::backend_call`]. Thread-safe core state (scheduler, storage,
//!   logging) is reached through the store's own `AsyncApi`, which is the same thing.
//! * **Async instance**: only the thread-safe subset (`AsyncApi`); every world-touching call
//!   returns `host-error::main-thread-only` (functions without an error channel return an
//!   empty/default value and warn once), registrations are refused, `http.fetch` works.

mod core_ifaces;
mod game;
mod http;
pub(crate) mod services;

pub(crate) use http::agent as http_agent;

use crate::bindings::rustic::api::types as wt;
use crate::convert::Cv;
use crate::state::HostState;
use rusticmc_core::PluginApi;
use rusticmc_core::backend::Backend;
use rusticmc_core::context;
use rusticmc_core::model as m;

impl wt::Host for HostState {}
impl crate::bindings::rustic::api::event_types::Host for HostState {}
impl crate::bindings::rustic::services::common::Host for HostState {}

/// Convert a core result into the WIT one.
#[inline]
pub(crate) fn wit<T: Cv<U>, U>(r: Result<T, m::HostError>) -> Result<U, wt::HostError> {
    match r {
        Ok(v) => Ok(v.cv()),
        Err(e) => Err(e.cv()),
    }
}

impl HostState {
    /// The `PluginApi` of this plugin, from the main instance only.
    pub(crate) fn main_api(&self) -> Result<PluginApi, m::HostError> {
        if !self.is_main {
            return Err(m::HostError::MainThreadOnly);
        }
        let api = context::api()?;
        if *api.id() != self.plugin {
            // Never act as another plugin (would only happen if the host called an instance
            // outside the core's call frame).
            return Err(m::HostError::Other(format!(
                "call context belongs to `{}`, not `{}`",
                api.id(),
                self.plugin
            )));
        }
        Ok(api)
    }

    /// A game call (main instance only).
    #[inline]
    pub(crate) fn game<T>(
        &mut self,
        f: impl FnOnce(&mut dyn Backend) -> Result<T, m::HostError>,
    ) -> Result<T, m::HostError> {
        if !self.is_main {
            return Err(m::HostError::MainThreadOnly);
        }
        context::backend_call(f)
    }

    /// A game call whose WIT function has no error channel: errors give `default` (async
    /// instance: warned once, other errors at debug level).
    #[inline]
    pub(crate) fn game_or<T>(
        &mut self,
        what: &'static str,
        default: T,
        f: impl FnOnce(&mut dyn Backend) -> Result<T, m::HostError>,
    ) -> T {
        if !self.is_main {
            self.warn_once(
                what,
                "main instance only; returning an empty value in the async instance",
            );
            return default;
        }
        match context::backend_call(f) {
            Ok(v) => v,
            Err(e) => {
                log::debug!(target: "rustic", "[{}] {what}: {e}", self.plugin);
                default
            }
        }
    }

    /// A registration-style call (no result channel) from the async instance: ignored with a
    /// warning.
    pub(crate) fn main_only_ignored(&mut self, what: &'static str) {
        self.warn_once(what, "main instance only; ignored in the async instance");
    }
}
