//! `config.toml` of the menus plugin.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// The packaged default configuration.
pub const DEFAULT_CONFIG: &str = include_str!("../config.toml");

/// The configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MenusConfig {
    /// `auto`, `en` or `fr`.
    pub language: String,
    /// Live refresh period of open menus, in ticks.
    pub refresh_ticks: u64,
    pub warps: Vec<Warp>,
    pub toggles: Vec<Toggle>,
}

impl Default for MenusConfig {
    fn default() -> Self {
        MenusConfig {
            language: "auto".into(),
            refresh_ticks: 20,
            warps: Vec::new(),
            toggles: Vec::new(),
        }
    }
}

/// `[[warps]]`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Warp {
    pub name: String,
    pub icon: String,
    pub description: String,
    pub commands: Vec<String>,
    pub confirm: bool,
}

/// `[[toggles]]`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Toggle {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub description: Vec<String>,
    pub default: bool,
}

/// A player's saved toggles (only the ones they changed), per-player data key `settings`.
pub type Settings = BTreeMap<String, bool>;

/// Per-player data key of [`Settings`].
pub const SETTINGS_KEY: &str = "settings";

impl MenusConfig {
    /// A toggle's effective value for saved settings.
    pub fn value(&self, settings: &Settings, id: &str) -> bool {
        settings.get(id).copied().unwrap_or_else(|| {
            self.toggles
                .iter()
                .find(|t| t.id == id)
                .is_some_and(|t| t.default)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_config() {
        let (cfg, rewrite) = rustic::config::Config::new()
            .defaults(DEFAULT_CONFIG)
            .load_from_str::<MenusConfig>(Some(DEFAULT_CONFIG))
            .unwrap();
        assert_eq!(rewrite, None);
        assert_eq!(cfg.warps.len(), 3);
        assert!(cfg.warps[2].confirm);
        assert_eq!(cfg.toggles.len(), 4);
        let mut s = Settings::new();
        assert!(cfg.value(&s, "mention_sound"));
        assert!(!cfg.value(&s, "particles"));
        s.insert("particles".into(), true);
        assert!(cfg.value(&s, "particles"));
        assert!(!cfg.value(&s, "unknown"));
    }
}
