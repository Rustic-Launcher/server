//! # rusticmc-core
//!
//! The engine-agnostic Rustic runtime: everything between the game engine adapter (Horizon)
//! and the WebAssembly host (`rusticmc-host`), with no dependency on either.
//!
//! * [`model`]: Rust mirrors of every WIT type of `rustic:api@0.1.0` / `rustic:services@0.1.0`,
//!   and [`model::Event`].
//! * [`manifest`], [`archive`], [`discovery`]: the plugin format (`rustic.plugin.toml`,
//!   `.rplugin`, dev directories), discovery and dependency resolution.
//! * [`lifecycle`]: plugin states, [`PluginFault`], the [`PluginInstance`] and [`PluginLoader`]
//!   traits the host implements; [`native`] has closure-based implementations for tests.
//! * [`Rustic`] (in [`runtime`]): the runtime — event bus, scheduler, commands, permissions,
//!   services, storage — and [`PluginApi`] / [`AsyncApi`], what plugin imports call.
//! * [`backend`]: the [`Backend`] trait (the game, as seen by plugins).
//! * [`context`]: how host functions reach the runtime and the current backend (ADR 0004).
//! * [`config`], [`storage`], [`scheduler`], [`commands`], [`permission`], [`services`],
//!   [`watchdog`], [`clock`].
//!
//! Logging uses the `log` crate with target `rustic`.

pub mod archive;
pub mod backend;
mod builtin;
pub mod bus;
pub mod clock;
pub mod commands;
pub mod config;
pub mod context;
pub mod discovery;
pub mod id;
pub mod lifecycle;
pub mod manifest;
pub mod model;
pub mod native;
pub mod permission;
pub mod runtime;
pub mod scheduler;
pub mod services;
pub mod storage;
pub mod watchdog;

pub use backend::{Backend, NullBackend};
pub use builtin::ADMIN_PERMISSION;
pub use id::PluginId;
pub use lifecycle::{LoadRequest, PluginFault, PluginInstance, PluginLoader, PluginState};
pub use manifest::{HOST_API, Manifest};
pub use runtime::{
    AsyncApi, CallError, CommandOutcome, CoreError, LoadReport, PluginApi, PluginInfo, Rustic,
    RusticBuilder, SharedHandle, TickReport,
};
pub use rusticmc_permissions as permissions_engine;
pub use rusticmc_timings as timings;
