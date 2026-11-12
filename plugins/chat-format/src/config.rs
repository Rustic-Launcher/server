//! `config.toml` of chat-format.

use serde::{Deserialize, Serialize};

use crate::filter::{Action, SpamLimits};

/// The packaged default configuration.
pub const DEFAULT_CONFIG: &str = include_str!("../config.toml");

/// The whole configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ChatConfig {
    /// `auto` (client language), `en` or `fr`.
    pub language: String,
    pub format: FormatConfig,
    pub groups: Vec<GroupConfig>,
    pub join: JoinConfig,
    pub mentions: MentionConfig,
    pub spam: SpamConfig,
    pub caps: CapsConfig,
    pub links: LinkConfig,
    pub words: WordConfig,
    pub mute: MuteConfig,
}

impl Default for ChatConfig {
    fn default() -> Self {
        ChatConfig {
            language: "auto".into(),
            format: FormatConfig::default(),
            groups: Vec::new(),
            join: JoinConfig::default(),
            mentions: MentionConfig::default(),
            spam: SpamConfig::default(),
            caps: CapsConfig::default(),
            links: LinkConfig::default(),
            words: WordConfig::default(),
            mute: MuteConfig::default(),
        }
    }
}

/// `[format]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FormatConfig {
    /// Line format when no group format applies.
    pub default: String,
    /// Hover text of the sender's name.
    pub name_hover: String,
    /// Suggested command when the name is clicked (empty: no click action).
    pub name_click: String,
    /// Prefix/suffix when neither the ChatMeta service nor the group gives one.
    pub default_prefix: String,
    pub default_suffix: String,
    /// Group used for `{group}` when none applies.
    pub default_group: String,
}

impl Default for FormatConfig {
    fn default() -> Self {
        FormatConfig {
            default: "{prefix}{name}{suffix}<dark_gray>:</dark_gray> <white>{message}".into(),
            name_hover: "<gold>{name}</gold>".into(),
            name_click: "/msg {name} ".into(),
            default_prefix: String::new(),
            default_suffix: String::new(),
            default_group: "default".into(),
        }
    }
}

/// `[[groups]]`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct GroupConfig {
    pub name: String,
    /// Line format for this group (empty: `[format].default`).
    pub format: String,
    pub prefix: String,
    pub suffix: String,
}

/// `[join]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct JoinConfig {
    /// Join line (empty: hidden; `"vanilla"` keeps the server's).
    pub message: String,
    /// Join line the very first time a player joins.
    pub first_message: String,
    /// Quit line.
    pub quit_message: String,
}

impl Default for JoinConfig {
    fn default() -> Self {
        JoinConfig {
            message: "<dark_gray>[<green>+</green>]</dark_gray> <gray>{name}".into(),
            first_message: "<light_purple>Welcome <white>{name}</white> to the server!".into(),
            quit_message: "<dark_gray>[<red>-</red>]</dark_gray> <gray>{name}".into(),
        }
    }
}

/// `[mentions]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MentionConfig {
    pub enabled: bool,
    /// How `@name` is shown (`{name}` = the mentioned player).
    pub format: String,
    /// Sound played to the mentioned player (empty: none).
    pub sound: String,
    pub volume: f32,
    pub pitch: f32,
    /// Also tell the mentioned player in the action bar.
    pub action_bar: bool,
}

impl Default for MentionConfig {
    fn default() -> Self {
        MentionConfig {
            enabled: true,
            format: "<yellow>@{name}</yellow>".into(),
            sound: "minecraft:entity.experience_orb.pickup".into(),
            volume: 1.0,
            pitch: 1.2,
            action_bar: true,
        }
    }
}

/// `[spam]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SpamConfig {
    /// Messages allowed per window (0: no rate limit).
    pub max_messages: usize,
    pub per_seconds: u64,
    /// Repeating a message within this many seconds is refused (0: allowed).
    pub duplicate_seconds: u64,
}

impl Default for SpamConfig {
    fn default() -> Self {
        SpamConfig {
            max_messages: 5,
            per_seconds: 8,
            duplicate_seconds: 30,
        }
    }
}

impl SpamConfig {
    /// As limits in milliseconds.
    pub fn limits(&self) -> SpamLimits {
        SpamLimits {
            max_messages: self.max_messages,
            window_ms: self.per_seconds.saturating_mul(1000),
            duplicate_ms: self.duplicate_seconds.saturating_mul(1000),
        }
    }
}

/// `[caps]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CapsConfig {
    pub enabled: bool,
    /// Only judge messages with at least this many letters.
    pub min_letters: usize,
    /// Highest allowed share of capitals (0.0..1.0).
    pub max_ratio: f32,
    /// `lowercase` or `block`.
    pub action: Action,
}

impl Default for CapsConfig {
    fn default() -> Self {
        CapsConfig {
            enabled: true,
            min_letters: 6,
            max_ratio: 0.7,
            action: Action::Lowercase,
        }
    }
}

/// `[links]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LinkConfig {
    pub enabled: bool,
    /// `block` or `mask`.
    pub action: Action,
    /// Shown instead of a masked link.
    pub replacement: String,
    /// Allowed domains (subdomains included).
    pub whitelist: Vec<String>,
}

impl Default for LinkConfig {
    fn default() -> Self {
        LinkConfig {
            enabled: true,
            action: Action::Block,
            replacement: "[link removed]".into(),
            whitelist: Vec::new(),
        }
    }
}

/// `[words]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WordConfig {
    /// `mask` or `block`.
    pub action: Action,
    pub mask_char: char,
    pub list: Vec<String>,
}

impl Default for WordConfig {
    fn default() -> Self {
        WordConfig {
            action: Action::Mask,
            mask_char: '*',
            list: Vec::new(),
        }
    }
}

/// `[mute]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct MuteConfig {
    /// Commands a muted player may not use (first word, without slash).
    pub blocked_commands: Vec<String>,
}

impl Default for MuteConfig {
    fn default() -> Self {
        MuteConfig {
            blocked_commands: ["msg", "tell", "w", "whisper", "me", "r", "reply", "say"]
                .map(String::from)
                .to_vec(),
        }
    }
}

/// Parses a config text the way the plugin does (tests).
#[cfg(test)]
pub fn parse(text: Option<&str>) -> rustic::Result<ChatConfig> {
    let (cfg, _) = rustic::config::Config::new()
        .defaults(DEFAULT_CONFIG)
        .load_from_str::<ChatConfig>(text)?;
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_config_parses() {
        let cfg = parse(Some(DEFAULT_CONFIG)).unwrap();
        assert_eq!(cfg.language, "auto");
        assert!(cfg.groups.iter().any(|g| g.name == "admin"));
        assert_eq!(cfg.caps.action, Action::Lowercase);
        assert_eq!(cfg.links.action, Action::Block);
        assert_eq!(cfg.words.action, Action::Mask);
        assert!(cfg.format.default.contains("{message}"));
        assert!(DEFAULT_CONFIG.starts_with("config_version = 1"));
    }

    #[test]
    fn missing_keys_take_defaults() {
        let cfg = parse(Some("config_version = 1\n[spam]\nmax_messages = 2\n")).unwrap();
        assert_eq!(cfg.spam.max_messages, 2);
        assert_eq!(cfg.spam.per_seconds, 8);
        assert!(cfg.mentions.enabled);
    }
}
