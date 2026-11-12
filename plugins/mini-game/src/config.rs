//! `config.toml` of the mini-game.

use serde::{Deserialize, Serialize};

use crate::game::Rules;

/// The packaged default configuration.
pub const DEFAULT_CONFIG: &str = include_str!("../config.toml");

/// The configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Cfg {
    pub language: String,
    pub min_players: usize,
    pub max_players: usize,
    pub countdown_seconds: u32,
    pub round_seconds: u32,
    pub end_seconds: u32,
    pub default_radius: i32,
    pub spleef: SpleefCfg,
    pub koth: KothCfg,
    pub rewards: Rewards,
}

impl Default for Cfg {
    fn default() -> Self {
        Cfg {
            language: "auto".into(),
            min_players: 2,
            max_players: 8,
            countdown_seconds: 15,
            round_seconds: 180,
            end_seconds: 5,
            default_radius: 8,
            spleef: SpleefCfg::default(),
            koth: KothCfg::default(),
            rewards: Rewards::default(),
        }
    }
}

/// `[spleef]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SpleefCfg {
    pub floor_block: String,
    pub tool: String,
}

impl Default for SpleefCfg {
    fn default() -> Self {
        SpleefCfg {
            floor_block: "minecraft:snow_block".into(),
            tool: "minecraft:diamond_shovel".into(),
        }
    }
}

/// `[koth]`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct KothCfg {
    pub points_to_win: u32,
    pub particle: String,
}

impl Default for KothCfg {
    fn default() -> Self {
        KothCfg {
            points_to_win: 60,
            particle: "minecraft:dust".into(),
        }
    }
}

/// `[rewards]` (economy minor units).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Rewards {
    pub win: i64,
    pub play: i64,
}

impl Default for Rewards {
    fn default() -> Self {
        Rewards { win: 500, play: 50 }
    }
}

impl Cfg {
    /// The game rules.
    pub fn rules(&self) -> Rules {
        Rules {
            min_players: self.min_players.max(1),
            max_players: self.max_players.max(1),
            countdown: self.countdown_seconds.max(1),
            round: self.round_seconds.max(1),
            end: self.end_seconds.max(1),
            koth_points: self.koth.points_to_win.max(1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_config() {
        let (cfg, rewrite) = rustic::config::Config::new()
            .defaults(DEFAULT_CONFIG)
            .load_from_str::<Cfg>(Some(DEFAULT_CONFIG))
            .unwrap();
        assert_eq!(rewrite, None);
        assert_eq!(cfg.min_players, 2);
        assert_eq!(cfg.spleef.floor_block, "minecraft:snow_block");
        assert_eq!(cfg.koth.points_to_win, 60);
        assert_eq!(cfg.rewards.win, 500);
        let r = cfg.rules();
        assert_eq!((r.countdown, r.round, r.end), (15, 180, 5));
    }
}
