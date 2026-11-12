//! # rusticmc-host
//!
//! The wasmtime component runtime of Rustic: it implements the core's
//! [`PluginLoader`](rusticmc_core::PluginLoader) / [`PluginInstance`](rusticmc_core::PluginInstance)
//! for plugins built as WebAssembly components against `rustic:api@0.1.0`.
//!
//! * One shared [`wasmtime::Engine`] (fuel, epoch interruption, Cranelift `speed`, parallel
//!   compilation, an on-disk compiled-module cache under `<server>/cache/rustic/`, optional
//!   pooling allocator), an epoch ticker thread, a [`Linker`](wasmtime::component::Linker) with
//!   WASI p2 and every `rustic:api` / `rustic:services` import.
//! * Per plugin a **main instance** (its own store, server thread only) and an **async
//!   instance** (another store, a worker of the async pool), see [`WasmHost`].
//! * Sandbox: fuel per call, a wall-clock deadline per call (epochs), memory/table/instance
//!   limits, traps/panics/timeouts/OOM turned into [`PluginFault`](rusticmc_core::PluginFault)s
//!   so the core disables the plugin and the server goes on. WASI gets clocks, random,
//!   stdout/stderr to the log, and the data folder preopened as `/` only with
//!   `filesystem = "data-folder"`; no env, args or sockets.
//!
//! ```no_run
//! use rusticmc_core::{NullBackend, Rustic};
//! use rusticmc_host::{HostConfig, WasmHost};
//!
//! let server = std::path::Path::new("server");
//! let host = WasmHost::new(HostConfig::for_server(server))?;
//! let rt = Rustic::new(server, host.loader())?;
//! host.install(&rt); // permission-provider bridge (also done at the first load)
//! let mut backend = NullBackend; // the engine adapter's Backend in production
//! rt.load_plugins(&mut backend);
//! rt.enable_all(&mut backend);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! Design notes: ADRs 0020 (engine configuration), 0021 (main/async instance model),
//! 0022 (HTTP client), 0023 (compiled-module cache); measurements in `docs/PERFORMANCE.md`.

#![warn(missing_docs)]

/// Host bindings generated from the WIT world (imports implemented by the host; exports
/// looked up by name at instantiation).
#[allow(missing_docs, clippy::all, clippy::pedantic)]
pub mod bindings {
    wasmtime::component::bindgen!({
        path: "../rusticmc-api-wit/wit",
        world: "rustic:api/rustic-plugin-permissions-economy",
    });
}

mod config;
mod convert;
mod engine;
mod imports;
mod instance;
mod loader;
mod output;
mod plugin;
mod pool;
mod state;

pub use config::{HostConfig, HttpConfig, OutputConfig, PoolingConfig};
pub use loader::{HostError, HostStats, WasmHost, WasmLoader};
pub use plugin::WasmPlugin;

/// Build a runtime for `server_dir` with a WASM host: `rustic.toml` / `permissions.toml` are
/// loaded (created if missing), the permission-provider bridge is installed.
pub fn runtime(
    server_dir: &std::path::Path,
    config: HostConfig,
) -> Result<(rusticmc_core::Rustic, WasmHost), Box<dyn std::error::Error + Send + Sync>> {
    let host = WasmHost::new(config)?;
    let rt = rusticmc_core::Rustic::new(server_dir, host.loader())?;
    host.install(&rt);
    Ok((rt, host))
}
