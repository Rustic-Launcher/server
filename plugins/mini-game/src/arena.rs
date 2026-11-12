//! Arenas as stored (`arenas/<name>`), and their geometry (pure).

use serde::{Deserialize, Serialize};

/// Storage key prefix of arenas.
pub const KEY_PREFIX: &str = "arenas/";

/// The storage key of an arena.
pub fn key(name: &str) -> String {
    format!("{KEY_PREFIX}{name}")
}

/// Whether `name` is a valid arena name: `[a-z0-9_]{1,16}`.
pub fn valid_name(name: &str) -> bool {
    (1..=16).contains(&name.len())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// The game played in an arena.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Break the snow floor under the others; the last one standing wins.
    Spleef,
    /// Stand alone in the zone to score; first to the target (or best at the end) wins.
    Koth,
}

impl Mode {
    /// From a command argument.
    pub fn parse(s: &str) -> Option<Mode> {
        match s.to_ascii_lowercase().as_str() {
            "spleef" => Some(Mode::Spleef),
            "koth" | "kingofthehill" | "hill" => Some(Mode::Koth),
            _ => None,
        }
    }
}

/// A block position (serializable).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Pos {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl Pos {
    pub const fn new(x: i32, y: i32, z: i32) -> Pos {
        Pos { x, y, z }
    }
}

/// A location (serializable).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Loc {
    pub world: String,
    pub x: f64,
    pub y: f64,
    pub z: f64,
    #[serde(default)]
    pub yaw: f32,
    #[serde(default)]
    pub pitch: f32,
}

/// An arena.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Arena {
    pub name: String,
    pub mode: Mode,
    /// Where players start (spread on a small ring around it).
    pub spawn: Loc,
    /// Where players wait, and where they go when they leave or the game ends.
    pub lobby: Loc,
    /// Spleef: the floor centre (the floor is at `center.y`). KotH: the zone centre (the block
    /// players stand on).
    pub center: Pos,
    /// Floor / zone radius in blocks.
    pub radius: i32,
}

impl Arena {
    /// Whether `pos` is part of the spleef floor.
    pub fn in_floor(&self, pos: Pos) -> bool {
        pos.y == self.center.y && in_disc(self.center, self.radius, pos.x, pos.z)
    }

    /// Every floor block (a disc of `radius` around the centre).
    pub fn floor(&self) -> Vec<Pos> {
        let r = self.radius;
        let mut v = Vec::new();
        for dx in -r..=r {
            for dz in -r..=r {
                let (x, z) = (self.center.x + dx, self.center.z + dz);
                if in_disc(self.center, r, x, z) {
                    v.push(Pos::new(x, self.center.y, z));
                }
            }
        }
        v
    }

    /// Whether a player at `(x, y, z)` stands in the KotH zone (feet within 4 blocks above it).
    pub fn in_zone(&self, x: f64, y: f64, z: f64) -> bool {
        let (cx, cz) = (
            f64::from(self.center.x) + 0.5,
            f64::from(self.center.z) + 0.5,
        );
        let r = f64::from(self.radius) + 0.5;
        let dy = y - f64::from(self.center.y);
        (x - cx).powi(2) + (z - cz).powi(2) <= r * r && (0.0..=4.0).contains(&dy)
    }

    /// Whether a player at height `y` fell through the spleef floor.
    pub fn fell(&self, y: f64) -> bool {
        y < f64::from(self.center.y)
    }

    /// `n` points on the zone's border, at `(x, z)`.
    pub fn border(&self, n: usize) -> Vec<(f64, f64)> {
        let (cx, cz) = (
            f64::from(self.center.x) + 0.5,
            f64::from(self.center.z) + 0.5,
        );
        let r = f64::from(self.radius) + 0.5;
        (0..n)
            .map(|i| {
                let a = std::f64::consts::TAU * i as f64 / n as f64;
                (cx + r * a.cos(), cz + r * a.sin())
            })
            .collect()
    }

    /// The `i`-th of `n` start positions: a ring of radius up to 2 around the spawn.
    pub fn start_position(&self, i: usize, n: usize) -> Loc {
        let mut l = self.spawn.clone();
        if n > 1 {
            let ring = f64::from(self.radius.clamp(1, 3) - 1).max(1.0);
            let a = std::f64::consts::TAU * i as f64 / n as f64;
            l.x += ring * a.cos();
            l.z += ring * a.sin();
        }
        l
    }
}

fn in_disc(center: Pos, r: i32, x: i32, z: i32) -> bool {
    let (dx, dz) = (i64::from(x - center.x), i64::from(z - center.z));
    let r = i64::from(r);
    dx * dx + dz * dz <= r * r + r
}

/// `m:ss`.
pub fn clock(seconds: u32) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arena(mode: Mode, radius: i32) -> Arena {
        let loc = Loc {
            world: "minecraft:overworld".into(),
            x: 0.5,
            y: 65.0,
            z: 0.5,
            yaw: 0.0,
            pitch: 0.0,
        };
        Arena {
            name: "a".into(),
            mode,
            spawn: loc.clone(),
            lobby: loc,
            center: Pos::new(0, 64, 0),
            radius,
        }
    }

    #[test]
    fn names_and_modes() {
        assert!(valid_name("arena_1"));
        assert!(!valid_name("Arena"));
        assert!(!valid_name(""));
        assert!(!valid_name("a-b"));
        assert!(!valid_name("abcdefghijklmnopq"));
        assert_eq!(Mode::parse("KOTH"), Some(Mode::Koth));
        assert_eq!(Mode::parse("spleef"), Some(Mode::Spleef));
        assert_eq!(Mode::parse("x"), None);
        assert_eq!(key("a"), "arenas/a");
    }

    #[test]
    fn floor_disc() {
        let a = arena(Mode::Spleef, 2);
        let f = a.floor();
        assert_eq!(f.len(), 21);
        assert!(f.contains(&Pos::new(2, 64, 0)));
        assert!(f.contains(&Pos::new(1, 64, 1)));
        assert!(!f.contains(&Pos::new(2, 64, 2)));
        assert!(a.in_floor(Pos::new(-2, 64, 0)));
        assert!(!a.in_floor(Pos::new(0, 65, 0)));
        assert!(!a.in_floor(Pos::new(3, 64, 0)));
        assert!(a.fell(63.9));
        assert!(!a.fell(65.0));
        assert_eq!(arena(Mode::Spleef, 0).floor().len(), 1);
    }

    #[test]
    fn zone_and_border() {
        let a = arena(Mode::Koth, 3);
        assert!(a.in_zone(0.5, 65.0, 0.5));
        assert!(a.in_zone(3.5, 65.0, 0.5));
        assert!(!a.in_zone(5.0, 65.0, 0.5));
        assert!(!a.in_zone(0.5, 70.0, 0.5));
        assert!(!a.in_zone(0.5, 63.0, 0.5));
        let b = a.border(4);
        assert_eq!(b.len(), 4);
        assert!((b[0].0 - 4.0).abs() < 1e-9 && (b[0].1 - 0.5).abs() < 1e-9);
        let p = a.start_position(1, 4);
        assert!((p.x - 0.5).abs() < 1e-9 && (p.z - 2.5).abs() < 1e-9);
        assert_eq!(a.start_position(0, 1), a.spawn);
    }

    #[test]
    fn clocks() {
        assert_eq!(clock(0), "0:00");
        assert_eq!(clock(125), "2:05");
    }

    #[test]
    fn serde_roundtrip() {
        let a = arena(Mode::Koth, 3);
        let json = rustic::serde_json::to_string(&a).unwrap();
        assert!(json.contains("\"mode\":\"koth\""));
        assert_eq!(rustic::serde_json::from_str::<Arena>(&json).unwrap(), a);
    }
}
