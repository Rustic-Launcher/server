//! `rustic-bridge.toml` (next to `rustic.toml`, optional): where the JVM and the bridge jar are,
//! JVM options and the timeouts.
//!
//! ```toml
//! # rustic-bridge.toml (every key optional)
//! enabled = true                  # false: never start the JVM
//! java = "/opt/jdk-25/bin/java"   # default: $RUSTIC_BRIDGE_JAVA, $JAVA_HOME/bin/java, then `java`
//! home = "bridge"                 # rustic-bridge.jar + libs/; default: $RUSTIC_BRIDGE_HOME, <server>/bridge
//! plugins = "plugins/bukkit"      # the Bukkit plugin jars
//! jvm_args = ["-Xmx1G"]
//! call_timeout_ms = 30000         # a call into Java longer than this kills the JVM (stuck plugin)
//! startup_timeout_ms = 30000
//! watchdog_ms = 3000              # the JVM logs the stack of a call running longer than this
//! online_mode = true              # what Server#getOnlineMode answers
//! ```

use serde::Deserialize;
use std::path::{Path, PathBuf};

/// File name of the bridge's configuration, in the server folder.
pub const CONFIG_FILE: &str = "rustic-bridge.toml";

/// Bridge settings.
#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BridgeConfig {
    /// Start the bridge at all.
    pub enabled: bool,
    /// The `java` executable.
    pub java: Option<PathBuf>,
    /// Folder holding `rustic-bridge.jar` and `libs/`.
    pub home: Option<PathBuf>,
    /// Folder of the Bukkit plugin jars (relative to the server folder).
    pub plugins: PathBuf,
    /// Extra JVM options.
    pub jvm_args: Vec<String>,
    /// Longest wait for one answer of the JVM.
    pub call_timeout_ms: u64,
    /// Longest wait for the JVM to start and connect.
    pub startup_timeout_ms: u64,
    /// The JVM's own stuck-call reporter threshold.
    pub watchdog_ms: u64,
    /// `Server#getOnlineMode`.
    pub online_mode: bool,
}

impl Default for BridgeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            java: None,
            home: None,
            plugins: PathBuf::from("plugins").join("bukkit"),
            jvm_args: vec!["-Xmx1G".into()],
            call_timeout_ms: 30_000,
            startup_timeout_ms: 30_000,
            watchdog_ms: 3_000,
            online_mode: true,
        }
    }
}

impl BridgeConfig {
    /// `<server>/rustic-bridge.toml`, or the defaults when there is none.
    pub fn load(server_dir: &Path) -> Result<Self, String> {
        let path = server_dir.join(CONFIG_FILE);
        match std::fs::read_to_string(&path) {
            Ok(text) => toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(format!("{}: {e}", path.display())),
        }
    }

    /// The `java` to run.
    pub fn java(&self) -> PathBuf {
        if let Some(j) = &self.java {
            return j.clone();
        }
        if let Some(j) = std::env::var_os("RUSTIC_BRIDGE_JAVA") {
            return j.into();
        }
        if let Some(home) = std::env::var_os("JAVA_HOME") {
            let j =
                Path::new(&home)
                    .join("bin")
                    .join(if cfg!(windows) { "java.exe" } else { "java" });
            if j.is_file() {
                return j;
            }
        }
        PathBuf::from("java")
    }

    /// The folder holding `rustic-bridge.jar`: configured, `$RUSTIC_BRIDGE_HOME`,
    /// `<server>/bridge`, `bridge/` beside the executable, or this checkout's `bridge/java/build`.
    pub fn home(&self, server_dir: &Path) -> Result<PathBuf, String> {
        let candidates: Vec<PathBuf> = [
            self.home.as_ref().map(|h| server_dir.join(h)),
            std::env::var_os("RUSTIC_BRIDGE_HOME").map(PathBuf::from),
            Some(server_dir.join("bridge")),
            std::env::current_exe()
                .ok()
                .and_then(|e| e.parent().map(|p| p.join("bridge"))),
            Some(Path::new(env!("CARGO_MANIFEST_DIR")).join("../java/build")),
        ]
        .into_iter()
        .flatten()
        .collect();
        candidates
            .iter()
            .find(|c| c.join("rustic-bridge.jar").is_file())
            .cloned()
            .ok_or_else(|| {
                format!(
                    "rustic-bridge.jar not found in {}",
                    candidates
                        .iter()
                        .map(|c| c.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
    }

    /// The Bukkit plugin folder.
    pub fn plugins_dir(&self, server_dir: &Path) -> PathBuf {
        server_dir.join(&self.plugins)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_defaults() {
        let c: BridgeConfig =
            toml::from_str("jvm_args = [\"-Xmx256m\"]\ncall_timeout_ms = 5000\n").unwrap();
        assert_eq!(c.jvm_args, ["-Xmx256m"]);
        assert_eq!(c.call_timeout_ms, 5000);
        assert!(c.enabled);
        assert!(toml::from_str::<BridgeConfig>("nope = 1").is_err());
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(
            BridgeConfig::load(dir.path()).unwrap().startup_timeout_ms,
            30_000
        );
    }
}
