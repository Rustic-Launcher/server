//! Mutes: duration parsing, the stored record and the in-memory table (pure; the plugin
//! persists records under `mutes/<uuid>`).

use std::collections::HashMap;

use rustic::Uuid;
use serde::{Deserialize, Serialize};

/// Storage key prefix of mute records.
pub const KEY_PREFIX: &str = "mutes/";

/// A mute, as stored.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MuteRecord {
    /// The muted player's name when muted.
    pub name: String,
    /// When it ends (unix ms); `None` = permanent.
    pub until: Option<u64>,
    /// Why.
    #[serde(default)]
    pub reason: String,
    /// Who muted them.
    pub by: String,
    /// When (unix ms).
    pub at: u64,
}

impl MuteRecord {
    /// Whether the mute is over at `now`.
    pub fn expired(&self, now: u64) -> bool {
        self.until.is_some_and(|u| now >= u)
    }

    /// Milliseconds left (`None` = permanent).
    pub fn remaining(&self, now: u64) -> Option<u64> {
        self.until.map(|u| u.saturating_sub(now))
    }
}

/// The storage key of a player's mute.
pub fn key(player: Uuid) -> String {
    format!("{KEY_PREFIX}{player}")
}

/// A parsed `/mute` duration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MuteLength {
    /// Until `/unmute`.
    Permanent,
    /// This many milliseconds.
    For(u64),
}

/// Parses `30s`, `10m`, `2h`, `1d`, `1w`, combinations (`1h30m`), a bare number (minutes), or
/// `perm`/`permanent`/`forever`. `None` when `s` is not a duration (it then starts the reason).
pub fn parse_duration(s: &str) -> Option<MuteLength> {
    let s = s.trim().to_ascii_lowercase();
    if matches!(s.as_str(), "perm" | "permanent" | "forever" | "inf") {
        return Some(MuteLength::Permanent);
    }
    if s.is_empty() {
        return None;
    }
    if let Ok(minutes) = s.parse::<u64>() {
        return (minutes > 0).then(|| MuteLength::For(minutes.saturating_mul(60_000)));
    }
    let mut total: u64 = 0;
    let mut num = String::new();
    for c in s.chars() {
        if c.is_ascii_digit() {
            num.push(c);
            continue;
        }
        let unit: u64 = match c {
            's' => 1_000,
            'm' => 60_000,
            'h' => 3_600_000,
            'd' => 86_400_000,
            'w' => 604_800_000,
            _ => return None,
        };
        let n: u64 = num.parse().ok()?;
        num.clear();
        total = total.saturating_add(n.saturating_mul(unit));
    }
    if !num.is_empty() || total == 0 {
        return None;
    }
    Some(MuteLength::For(total))
}

/// `1d 2h`, `5m 3s`, `12s`: the two largest non-zero units.
pub fn format_duration(ms: u64) -> String {
    let secs = ms.div_ceil(1000).max(1);
    let units = [(86_400, "d"), (3_600, "h"), (60, "m"), (1, "s")];
    let mut parts = Vec::new();
    let mut rest = secs;
    for (size, name) in units {
        let n = rest / size;
        rest %= size;
        if n > 0 {
            parts.push(format!("{n}{name}"));
        }
        if parts.len() == 2 {
            break;
        }
    }
    parts.join(" ")
}

/// Splits the optional `[duration] [reason...]` of `/mute`: when the first word is not a
/// duration it belongs to the reason. Default length: permanent.
pub fn split_duration_reason(duration: Option<&str>, reason: Option<&str>) -> (MuteLength, String) {
    let reason = reason.unwrap_or("").trim();
    match duration {
        None => (MuteLength::Permanent, reason.to_owned()),
        Some(d) => match parse_duration(d) {
            Some(len) => (len, reason.to_owned()),
            None => {
                let joined = if reason.is_empty() {
                    d.to_owned()
                } else {
                    format!("{d} {reason}")
                };
                (MuteLength::Permanent, joined)
            }
        },
    }
}

/// Mutes in memory, keyed by player.
#[derive(Debug, Default)]
pub struct Mutes {
    map: HashMap<Uuid, MuteRecord>,
}

impl Mutes {
    /// Adds or replaces a mute.
    pub fn insert(&mut self, player: Uuid, record: MuteRecord) {
        self.map.insert(player, record);
    }

    /// Removes a mute.
    pub fn remove(&mut self, player: Uuid) -> Option<MuteRecord> {
        self.map.remove(&player)
    }

    /// The active mute of a player, if any (expired ones are not returned).
    pub fn active(&self, player: Uuid, now: u64) -> Option<&MuteRecord> {
        self.map.get(&player).filter(|m| !m.expired(now))
    }

    /// Removes and returns every mute expired at `now`.
    pub fn take_expired(&mut self, now: u64) -> Vec<(Uuid, MuteRecord)> {
        let expired: Vec<Uuid> = self
            .map
            .iter()
            .filter(|(_, m)| m.expired(now))
            .map(|(u, _)| *u)
            .collect();
        expired
            .into_iter()
            .filter_map(|u| self.map.remove(&u).map(|m| (u, m)))
            .collect()
    }

    /// Active mutes, sorted by name.
    pub fn list(&self, now: u64) -> Vec<(Uuid, MuteRecord)> {
        let mut v: Vec<(Uuid, MuteRecord)> = self
            .map
            .iter()
            .filter(|(_, m)| !m.expired(now))
            .map(|(u, m)| (*u, m.clone()))
            .collect();
        v.sort_by_key(|a| a.1.name.to_lowercase());
        v
    }

    /// Number of mutes held (expired included until swept).
    pub fn len(&self) -> usize {
        self.map.len()
    }

    /// Whether no mute is held.
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations() {
        assert_eq!(parse_duration("30s"), Some(MuteLength::For(30_000)));
        assert_eq!(parse_duration("1h30m"), Some(MuteLength::For(5_400_000)));
        assert_eq!(parse_duration("2D"), Some(MuteLength::For(172_800_000)));
        assert_eq!(parse_duration("1w"), Some(MuteLength::For(604_800_000)));
        assert_eq!(parse_duration("15"), Some(MuteLength::For(900_000)));
        assert_eq!(parse_duration("perm"), Some(MuteLength::Permanent));
        assert_eq!(parse_duration("0"), None);
        assert_eq!(parse_duration("spam"), None);
        assert_eq!(parse_duration("10"), Some(MuteLength::For(600_000)));
        assert_eq!(parse_duration("10x"), None);
        assert_eq!(parse_duration("m"), None);
        assert_eq!(parse_duration("5m3"), None);
        assert_eq!(format_duration(5_400_000), "1h 30m");
        assert_eq!(format_duration(90_061_000), "1d 1h");
        assert_eq!(format_duration(1), "1s");
        assert_eq!(format_duration(61_000), "1m 1s");
    }

    #[test]
    fn duration_or_reason() {
        assert_eq!(
            split_duration_reason(Some("10m"), Some("spam")),
            (MuteLength::For(600_000), "spam".to_owned())
        );
        assert_eq!(
            split_duration_reason(Some("being"), Some("rude")),
            (MuteLength::Permanent, "being rude".to_owned())
        );
        assert_eq!(
            split_duration_reason(None, None),
            (MuteLength::Permanent, String::new())
        );
        assert_eq!(
            split_duration_reason(Some("rude"), None),
            (MuteLength::Permanent, "rude".to_owned())
        );
    }

    #[test]
    fn table() {
        let rec = |until| MuteRecord {
            name: "Bob".into(),
            until,
            reason: String::new(),
            by: "CONSOLE".into(),
            at: 0,
        };
        let (a, b) = (Uuid::from_u128(1), Uuid::from_u128(2));
        let mut m = Mutes::default();
        m.insert(a, rec(Some(1_000)));
        m.insert(b, rec(None));
        assert!(m.active(a, 999).is_some());
        assert!(m.active(a, 1_000).is_none());
        assert_eq!(m.active(b, u64::MAX).unwrap().remaining(5), None);
        assert_eq!(m.list(0).len(), 2);
        let expired = m.take_expired(2_000);
        assert_eq!(expired.len(), 1);
        assert_eq!(expired[0].0, a);
        assert_eq!(m.len(), 1);
        assert!(m.remove(b).is_some());
        assert!(m.is_empty());
        let json = rustic::serde_json::to_string(&rec(Some(3))).unwrap();
        assert_eq!(
            rustic::serde_json::from_str::<MuteRecord>(&json).unwrap(),
            rec(Some(3))
        );
        assert_eq!(key(a), "mutes/00000000-0000-0000-0000-000000000001");
    }
}
