//! # rusticmc-bridge: Bukkit/Paper plugins on Rustic
//!
//! The Rust half of Rustic Bridge. It runs the Java layer (`bridge/java`, GPL-3.0-only, a
//! separate program) in a **supervised JVM sidecar**, talks to it over length-prefixed JSON
//! frames on a loopback socket ([`link`]), and plugs every Bukkit plugin jar of
//! `plugins/bukkit/` into `rusticmc-core` as an ordinary plugin (`bukkit-<name>` in
//! `rustic plugins`), with its plugin.yml commands and permissions, its event listeners at their
//! priorities, and the scheduler pulsed by the server tick. Why a sidecar rather than an embedded
//! JVM, and the measurements: `docs/BRIDGE.md`, ADRs 0009 and 0040-0042.
//!
//! ```no_run
//! # use rusticmc_core::{NullBackend, Rustic};
//! let bridge = rusticmc_bridge::Bridge::new("server").unwrap();
//! let packages = bridge.packages();            // starts the JVM, reads plugins/bukkit/*.jar
//! let loader = bridge.loader(None);            // or Some(the WebAssembly host's loader)
//! let rt = Rustic::new("server", Box::new(loader)).unwrap();
//! let mut backend = NullBackend;
//! rt.load_packages(packages, &mut backend);
//! rt.enable_all(&mut backend);
//! ```
//!
//! Nothing here implements Bukkit interfaces or links Java: the GPL code stays on the other side
//! of the process boundary (ADR 0009).

pub mod bridge;
pub mod config;
pub mod descriptor;
mod host;
pub mod link;
pub mod sidecar;

pub use bridge::{Bridge, BridgeInstance, BridgeLoader, JavaPluginInstance, has_bukkit_plugins};
pub use config::BridgeConfig;
pub use descriptor::{BRIDGE_ID, Descriptor};
