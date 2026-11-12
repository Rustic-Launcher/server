//! Server-level configuration: `<server>/rustic.toml`.
//!
//! ```toml
//! [limits]
//! fuel_per_call = 50_000_000      # default fuel per call into a plugin
//! max_fuel_per_call = 500_000_000 # cap for a plugin's own [limits] override
//! epoch_deadline_ms = 50          # wall-clock deadline of one call
//! memory_mb = 64                  # default linear memory limit
//! max_memory_mb = 512             # cap for a plugin's override
//!
//! [scheduler]
//! max_task_ms_per_tick = 25       # sync task budget per tick, the rest rolls over
//! async_workers = 2               # threads of the host's async instance pool
//!
//! [events]
//! move_threshold = 0.0625         # blocks a player must move before player-move fires
//!
//! [watchdog]
//! seconds = 10                    # a call running longer is reported as stuck (0 = off)
//!
//! [storage]
//! flush_interval_ticks = 20       # dirty KV stores are flushed at most this often
//! max_kv_bytes = 67108864         # per plugin
//!
//! [timings]
//! enabled = false
//! ```
//!
//! Every key is optional; unknown keys are refused (typos are errors, not silent defaults).

use crate::manifest::PluginLimits;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// File name of the server config.
pub const SERVER_CONFIG_FILE: &str = "rustic.toml";

/// Errors loading the server config.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    /// Reading or writing failed.
    #[error("{path}: {source}")]
    Io {
        /// Path.
        path: std::path::PathBuf,
        /// Cause.
        #[source]
        source: std::io::Error,
    },
    /// Not valid TOML or unknown keys.
    #[error("{SERVER_CONFIG_FILE}: {0}")]
    Syntax(String),
    /// Out-of-range values.
    #[error("{SERVER_CONFIG_FILE}: {}", .0.join("; "))]
    Invalid(Vec<String>),
}

/// `[limits]`: sandbox limits applied by the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct LimitsConfig {
    /// Default fuel per call.
    pub fuel_per_call: u64,
    /// Cap on a plugin's `limits.fuel_per_call`.
    pub max_fuel_per_call: u64,
    /// Epoch deadline of one call, in ms.
    pub epoch_deadline_ms: u64,
    /// Default memory limit (MiB).
    pub memory_mb: u32,
    /// Cap on a plugin's `limits.memory_mb`.
    pub max_memory_mb: u32,
}

impl Default for LimitsConfig {
    fn default() -> Self {
        Self {
            fuel_per_call: 50_000_000,
            max_fuel_per_call: 500_000_000,
            epoch_deadline_ms: 50,
            memory_mb: 64,
            max_memory_mb: 512,
        }
    }
}

/// `[scheduler]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SchedulerConfig {
    /// Budget of sync tasks per tick (ms); tasks over budget roll to the next tick.
    pub max_task_ms_per_tick: u64,
    /// Async instance pool threads (read by the host).
    pub async_workers: u32,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            max_task_ms_per_tick: 25,
            async_workers: 2,
        }
    }
}

/// `[events]`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct EventsConfig {
    /// Minimum movement (blocks) before `player-move` fires (used by the engine adapter).
    pub move_threshold: f64,
}

impl Default for EventsConfig {
    fn default() -> Self {
        Self {
            move_threshold: 0.0625,
        }
    }
}

/// `[watchdog]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WatchdogConfig {
    /// A call running longer than this is reported (0 disables the watchdog).
    pub seconds: u64,
}

impl Default for WatchdogConfig {
    fn default() -> Self {
        Self { seconds: 10 }
    }
}

/// `[storage]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct StorageConfig {
    /// Dirty stores are flushed at the end of a tick at most this often (also on disable and
    /// shutdown).
    pub flush_interval_ticks: u64,
    /// Total bytes (keys + values) a plugin's KV store may hold.
    pub max_kv_bytes: u64,
}

impl Default for StorageConfig {
    fn default() -> Self {
        Self {
            flush_interval_ticks: 20,
            max_kv_bytes: 64 * 1024 * 1024,
        }
    }
}

/// `[timings]`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct TimingsConfig {
    /// Collect timings from start.
    pub enabled: bool,
}

/// The whole `rustic.toml`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ServerConfig {
    /// Sandbox limits.
    pub limits: LimitsConfig,
    /// Scheduler.
    pub scheduler: SchedulerConfig,
    /// Events.
    pub events: EventsConfig,
    /// Watchdog.
    pub watchdog: WatchdogConfig,
    /// Storage.
    pub storage: StorageConfig,
    /// Timings.
    pub timings: TimingsConfig,
}

/// Limits for one plugin: its manifest overrides, capped by the server.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EffectiveLimits {
    /// Fuel per call.
    pub fuel_per_call: u64,
    /// Memory limit (MiB).
    pub memory_mb: u32,
    /// Epoch deadline (ms).
    pub epoch_deadline_ms: u64,
}

impl ServerConfig {
    /// Parse and validate TOML text.
    pub fn parse(text: &str) -> Result<Self, ConfigError> {
        let cfg: ServerConfig = toml::from_str(text)
            .map_err(|e| ConfigError::Syntax(e.to_string().trim().to_owned()))?;
        cfg.validate()?;
        Ok(cfg)
    }

    /// Check ranges.
    pub fn validate(&self) -> Result<(), ConfigError> {
        let mut e = Vec::new();
        let l = &self.limits;
        if l.fuel_per_call == 0 {
            e.push("limits.fuel_per_call must be positive".to_owned());
        }
        if l.max_fuel_per_call < l.fuel_per_call {
            e.push("limits.max_fuel_per_call must be >= limits.fuel_per_call".to_owned());
        }
        if l.epoch_deadline_ms == 0 || l.epoch_deadline_ms > 60_000 {
            e.push("limits.epoch_deadline_ms must be 1 to 60000".to_owned());
        }
        if l.memory_mb == 0 || l.memory_mb > crate::manifest::MAX_MEMORY_MB {
            e.push(format!(
                "limits.memory_mb must be 1 to {}",
                crate::manifest::MAX_MEMORY_MB
            ));
        }
        if l.max_memory_mb < l.memory_mb || l.max_memory_mb > crate::manifest::MAX_MEMORY_MB {
            e.push(format!(
                "limits.max_memory_mb must be between limits.memory_mb and {}",
                crate::manifest::MAX_MEMORY_MB
            ));
        }
        if self.scheduler.max_task_ms_per_tick == 0 || self.scheduler.max_task_ms_per_tick > 10_000
        {
            e.push("scheduler.max_task_ms_per_tick must be 1 to 10000".to_owned());
        }
        if self.scheduler.async_workers == 0 || self.scheduler.async_workers > 256 {
            e.push("scheduler.async_workers must be 1 to 256".to_owned());
        }
        if !self.events.move_threshold.is_finite() || self.events.move_threshold < 0.0 {
            e.push("events.move_threshold must be a finite number >= 0".to_owned());
        }
        if self.storage.flush_interval_ticks == 0 {
            e.push("storage.flush_interval_ticks must be positive".to_owned());
        }
        if self.storage.max_kv_bytes < 1024 {
            e.push("storage.max_kv_bytes must be at least 1024".to_owned());
        }
        if e.is_empty() {
            Ok(())
        } else {
            Err(ConfigError::Invalid(e))
        }
    }

    /// Load `<server_dir>/rustic.toml`, writing the defaults there first if it does not exist.
    pub fn load_or_create(server_dir: &Path) -> Result<Self, ConfigError> {
        let path = server_dir.join(SERVER_CONFIG_FILE);
        let io = |source| ConfigError::Io {
            path: path.clone(),
            source,
        };
        if !path.exists() {
            let cfg = ServerConfig::default();
            crate::storage::write_atomic(&path, cfg.to_toml().as_bytes()).map_err(io)?;
            return Ok(cfg);
        }
        let text = std::fs::read_to_string(&path).map_err(io)?;
        Self::parse(&text)
    }

    /// As TOML.
    pub fn to_toml(&self) -> String {
        let body = toml::to_string_pretty(self).unwrap_or_default();
        format!(
            "# Rustic server configuration (see docs/PLUGIN_FORMAT.md, \"Server limits\").\n\n{body}"
        )
    }

    /// A plugin's effective limits: its overrides, capped by `max_*`.
    pub fn effective_limits(&self, overrides: &PluginLimits) -> EffectiveLimits {
        let l = &self.limits;
        EffectiveLimits {
            fuel_per_call: overrides
                .fuel_per_call
                .unwrap_or(l.fuel_per_call)
                .min(l.max_fuel_per_call),
            memory_mb: overrides
                .memory_mb
                .unwrap_or(l.memory_mb)
                .min(l.max_memory_mb),
            epoch_deadline_ms: l.epoch_deadline_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_round_trip_and_create() {
        let dir = tempfile::tempdir().unwrap();
        let cfg = ServerConfig::load_or_create(dir.path()).unwrap();
        assert_eq!(cfg, ServerConfig::default());
        assert!(dir.path().join(SERVER_CONFIG_FILE).exists());
        let again = ServerConfig::load_or_create(dir.path()).unwrap();
        assert_eq!(again, cfg);
        assert_eq!(ServerConfig::parse("").unwrap(), cfg);
    }

    #[test]
    fn partial_and_invalid() {
        let c =
            ServerConfig::parse("[scheduler]\nmax_task_ms_per_tick = 5\n[timings]\nenabled = true")
                .unwrap();
        assert_eq!(c.scheduler.max_task_ms_per_tick, 5);
        assert!(c.timings.enabled);
        assert_eq!(c.limits, LimitsConfig::default());
        assert!(matches!(
            ServerConfig::parse("[limits]\nfuel = 1"),
            Err(ConfigError::Syntax(_))
        ));
        assert!(matches!(
            ServerConfig::parse("[nope]"),
            Err(ConfigError::Syntax(_))
        ));
        match ServerConfig::parse(
            "[limits]\nfuel_per_call = 0\nmemory_mb = 0\n[events]\nmove_threshold = -1.0",
        ) {
            Err(ConfigError::Invalid(v)) => assert!(v.len() >= 3, "{v:?}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn effective_limits_are_capped() {
        let c = ServerConfig::default();
        let e = c.effective_limits(&PluginLimits::default());
        assert_eq!(e.fuel_per_call, 50_000_000);
        assert_eq!(e.memory_mb, 64);
        let e = c.effective_limits(&PluginLimits {
            fuel_per_call: Some(u64::MAX),
            memory_mb: Some(4000),
        });
        assert_eq!(e.fuel_per_call, 500_000_000);
        assert_eq!(e.memory_mb, 512);
        let e = c.effective_limits(&PluginLimits {
            fuel_per_call: Some(10),
            memory_mb: Some(8),
        });
        assert_eq!((e.fuel_per_call, e.memory_mb), (10, 8));
    }
}
