//! Host configuration ([`HostConfig`]). Per-plugin sandbox limits (fuel, memory, epoch deadline)
//! come from `rustic.toml` `[limits]` and the manifest through the core
//! ([`rusticmc_core::config::EffectiveLimits`]); this is what the core does not know about:
//! the engine, the compiled-module cache, the async pool, HTTP and guest output.

use std::path::{Path, PathBuf};
use std::time::Duration;

/// How the wasmtime engine and the host runtime are set up.
#[derive(Clone, Debug)]
pub struct HostConfig {
    /// Directory of the on-disk compiled-module cache (`<server>/cache/rustic/`), keyed by the
    /// component's hash and the engine configuration. `None` disables it (every start compiles).
    pub cache_dir: Option<PathBuf>,
    /// Interval of the epoch ticker thread. Deadlines are measured in these ticks.
    pub epoch_tick: Duration,
    /// Wall-clock deadline of one call into an **async** instance, not counting the time the
    /// guest spends blocked in host calls (HTTP). Main-instance calls use
    /// `rustic.toml` `limits.epoch_deadline_ms`.
    pub async_deadline: Duration,
    /// How often the async dispatcher polls the scheduler for due async tasks.
    pub async_poll_interval: Duration,
    /// Threads running async tasks. `None`: `rustic.toml` `scheduler.async_workers`.
    pub async_workers: Option<usize>,
    /// Outgoing HTTP (`http.fetch`).
    pub http: HttpConfig,
    /// Guest stdout/stderr forwarding to the log.
    pub output: OutputConfig,
    /// Use wasmtime's pooling instance allocator instead of on-demand allocation.
    pub pooling: Option<PoolingConfig>,
    /// Parse DWARF (when a plugin ships it) to put file:line in trap backtraces.
    pub backtrace_details: bool,
    /// Store-level caps besides memory: core instances, tables and memories per store.
    pub max_instances: usize,
    /// See [`HostConfig::max_instances`].
    pub max_tables: usize,
    /// See [`HostConfig::max_instances`].
    pub max_memories: usize,
    /// Maximum elements of one table (function references).
    pub max_table_elements: usize,
}

impl Default for HostConfig {
    fn default() -> Self {
        Self {
            cache_dir: None,
            epoch_tick: Duration::from_millis(1),
            async_deadline: Duration::from_secs(30),
            async_poll_interval: Duration::from_millis(2),
            async_workers: None,
            http: HttpConfig::default(),
            output: OutputConfig::default(),
            pooling: None,
            backtrace_details: true,
            max_instances: 128,
            max_tables: 128,
            max_memories: 16,
            max_table_elements: 1_000_000,
        }
    }
}

impl HostConfig {
    /// Defaults for a server directory: the compiled-module cache lives in
    /// `<server>/cache/rustic/`.
    pub fn for_server(server_dir: &Path) -> Self {
        Self {
            cache_dir: Some(server_dir.join("cache").join("rustic")),
            ..Self::default()
        }
    }
}

/// `http.fetch` limits.
#[derive(Clone, Debug)]
pub struct HttpConfig {
    /// Timeout when the request says `timeout-ms = 0`.
    pub default_timeout: Duration,
    /// Upper bound of any request's timeout.
    pub max_timeout: Duration,
    /// Largest response body accepted (larger: `host-error::other`).
    pub max_response_bytes: u64,
    /// Largest request body accepted.
    pub max_request_bytes: usize,
    /// Allow plain `http://` URLs (the domain allowlist still applies).
    pub allow_plain_http: bool,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            default_timeout: Duration::from_secs(10),
            max_timeout: Duration::from_secs(60),
            max_response_bytes: 8 * 1024 * 1024,
            max_request_bytes: 8 * 1024 * 1024,
            allow_plain_http: true,
        }
    }
}

/// Guest stdout/stderr: forwarded to the log line by line with the plugin id, rate-limited
/// (token bucket) so a plugin printing in a loop cannot flood the console.
#[derive(Clone, Debug)]
pub struct OutputConfig {
    /// Lines per second allowed on average.
    pub lines_per_second: u32,
    /// Burst size.
    pub burst: u32,
    /// Longest line kept (longer lines are cut).
    pub max_line_bytes: usize,
}

impl Default for OutputConfig {
    fn default() -> Self {
        Self {
            lines_per_second: 50,
            burst: 200,
            max_line_bytes: 4096,
        }
    }
}

/// Pooling allocator sizing (see ADR 0020: off by default).
#[derive(Clone, Debug)]
pub struct PoolingConfig {
    /// Component instances (two per plugin: main and async).
    pub total_component_instances: u32,
    /// Core instances (a Rust component has a handful).
    pub total_core_instances: u32,
    /// Linear memories.
    pub total_memories: u32,
    /// Tables.
    pub total_tables: u32,
    /// Largest linear memory a slot holds (bytes); caps `limits.memory_mb`.
    pub max_memory_size: usize,
}

impl Default for PoolingConfig {
    fn default() -> Self {
        Self {
            total_component_instances: 256,
            total_core_instances: 2048,
            total_memories: 512,
            total_tables: 1024,
            max_memory_size: 512 << 20,
        }
    }
}
