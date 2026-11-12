//! # rusticmc-test: test Rustic plugins with `cargo test`
//!
//! A MockBukkit-style harness written for Rustic: [`MockServer`] is a **real** Rustic runtime
//! (`rusticmc-core`: events, commands, scheduler, permissions, storage) over an **in-memory
//! game** (worlds with block storage, players with inventories, menus, scoreboards, boss bars,
//! tab list, plugin channels). It implements every `Backend` method, so a plugin under test
//! cannot tell it from the real server, and it loads either the plugin's real `.wasm`
//! component (through `rusticmc-host`, feature `wasm`) or native closure plugins.
//!
//! ```no_run
//! use rusticmc_test::{MockServer, build_plugin};
//!
//! #[test]
//! fn welcomes_players() {
//!     let server = MockServer::with_built(&build_plugin("."));  // the plugin crate itself
//!     let steve = server.add_player("Steve");                   // fires player-join
//!     steve.assert_message_contains("Welcome");
//!     assert!(!steve.chat("what the heck"));                     // cancelled by the plugin
//!     server.tick(20);                                           // runs the scheduler
//! }
//! ```
//!
//! ## What the simulation does
//!
//! Every simulated action fires the event the real server fires and **applies the outcome**
//! the way the server does: a cancelled chat is not broadcast, a cancelled block break keeps
//! the block, a cancelled move puts the player back, `player-death` decides what drops and
//! what is broadcast, `player-respawn` decides where the player appears, a plugin menu click
//! stays cancelled unless the plugin allows it, and so on. Events caused by a plugin's own
//! calls (a teleport, a game-mode change) fire synchronously like on the server (the calling
//! plugin itself is skipped, as the runtime never re-enters a plugin); deaths, kicks and menu
//! closes caused by a plugin are processed when its call returns.
//!
//! Time is fake and deterministic: [`MockServer::tick`] advances 50 ms per tick, runs the
//! scheduler and, by default ([`AsyncMode::Inline`]), runs due async tasks inline on the test
//! thread.
//!
//! See `docs/guides/testing.md` for the full guide.

#![warn(missing_docs)]

mod build;
mod harness;
mod logcap;
mod player;
mod server;
pub mod state;
mod vanilla;
#[cfg(feature = "wasm")]
mod wasm;
mod world;

pub use build::{
    BuiltPlugin, WASM_TARGET, build_plugin, locate, package_from_path, try_build_plugin,
};
pub use harness::{AsyncMode, CommandRun, offline_uuid};
pub use player::{MenuView, MockPlayer};
pub use server::{MockServer, MockServerBuilder, harness_config, native_manifest};
pub use state::{ParticleRecord, Received, SoundRecord, TitleRecord};
pub use world::{IntoBlockPos, MockWorld};

/// The runtime crate, for types the harness does not re-export.
pub use rusticmc_core as core;
/// Event records, model types (`Uuid`, `Location`, `ItemStack`, `ClickKind`...).
pub use rusticmc_core::model;
/// Native (closure) plugins.
pub use rusticmc_core::native::NativePlugin;
pub use rusticmc_core::{CommandOutcome, PluginState};

/// Everything a plugin test usually needs.
pub mod prelude {
    pub use crate::model::{
        BlockFace, BlockPos, BlockState, ClickKind, GameMode, Hand, ItemStack, Location, Text,
        Uuid, Vec3, Weather,
    };
    pub use crate::{
        AsyncMode, CommandOutcome, CommandRun, MockPlayer, MockServer, MockWorld, NativePlugin,
        PluginState, build_plugin,
    };
}
