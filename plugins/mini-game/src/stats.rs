//! Per-player statistics (`stats/<uuid>` in storage) and the leaderboard (pure).

use serde::{Deserialize, Serialize};

use rustic::Uuid;

/// Storage key prefix.
pub const KEY_PREFIX: &str = "stats/";

/// The storage key of a player's stats.
pub fn key(player: Uuid) -> String {
    format!("{KEY_PREFIX}{player}")
}

/// A player's record.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Stats {
    /// The name when last updated.
    pub name: String,
    pub wins: u32,
    pub games: u32,
    /// Blocks broken (spleef) and points (KotH), all games together.
    pub points: u64,
}

impl Stats {
    /// Wins per game, in percent (rounded down).
    pub fn win_rate(&self) -> u32 {
        (self.wins * 100).checked_div(self.games).unwrap_or(0)
    }
}

/// What `/mg top` sorts by.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stat {
    Wins,
    Games,
    Points,
}

impl Stat {
    /// From a command argument (default wins).
    pub fn parse(s: Option<&str>) -> Option<Stat> {
        match s.map(str::to_ascii_lowercase).as_deref() {
            None | Some("wins") => Some(Stat::Wins),
            Some("games") => Some(Stat::Games),
            Some("points") => Some(Stat::Points),
            _ => None,
        }
    }

    /// The argument name.
    pub fn name(self) -> &'static str {
        match self {
            Stat::Wins => "wins",
            Stat::Games => "games",
            Stat::Points => "points",
        }
    }
}

/// The best `n` records by `stat` (ties: more wins, then more games, then by name).
pub fn top(mut all: Vec<Stats>, stat: Stat, n: usize) -> Vec<Stats> {
    let value = |s: &Stats| match stat {
        Stat::Wins => u64::from(s.wins),
        Stat::Games => u64::from(s.games),
        Stat::Points => s.points,
    };
    all.retain(|s| s.games > 0);
    all.sort_by(|a, b| {
        value(b)
            .cmp(&value(a))
            .then(b.wins.cmp(&a.wins))
            .then(b.games.cmp(&a.games))
            .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    all.truncate(n);
    all
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(name: &str, wins: u32, games: u32, points: u64) -> Stats {
        Stats {
            name: name.into(),
            wins,
            games,
            points,
        }
    }

    #[test]
    fn ranking() {
        let all = vec![
            s("b", 2, 5, 10),
            s("a", 2, 5, 3),
            s("c", 5, 6, 1),
            s("z", 0, 0, 0),
            s("d", 0, 9, 50),
        ];
        let names = |v: Vec<Stats>| v.into_iter().map(|s| s.name).collect::<Vec<_>>();
        assert_eq!(
            names(top(all.clone(), Stat::Wins, 10)),
            ["c", "a", "b", "d"]
        );
        assert_eq!(names(top(all.clone(), Stat::Games, 2)), ["d", "c"]);
        assert_eq!(names(top(all, Stat::Points, 1)), ["d"]);
        assert_eq!(s("x", 1, 3, 0).win_rate(), 33);
        assert_eq!(Stats::default().win_rate(), 0);
        assert_eq!(Stat::parse(None), Some(Stat::Wins));
        assert_eq!(Stat::parse(Some("GAMES")), Some(Stat::Games));
        assert_eq!(Stat::parse(Some("x")), None);
        assert_eq!(
            key(Uuid::from_u128(1)),
            "stats/00000000-0000-0000-0000-000000000001"
        );
    }
}
