//! Pure logic of essentials-lite (no host calls): stored locations, names, home limits,
//! teleport requests, warmup movement and cooldowns. Tested on the host target.

use rustic::{Location, Uuid};
use serde::{Deserialize, Serialize};

/// A stored location (homes, warps, spawn, `/back`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Loc {
    /// World id (`minecraft:overworld`).
    pub world: String,
    /// X.
    pub x: f64,
    /// Y.
    pub y: f64,
    /// Z.
    pub z: f64,
    /// Yaw.
    #[serde(default)]
    pub yaw: f32,
    /// Pitch.
    #[serde(default)]
    pub pitch: f32,
}

impl From<&Location> for Loc {
    fn from(l: &Location) -> Self {
        Loc {
            world: l.world.clone(),
            x: l.x,
            y: l.y,
            z: l.z,
            yaw: l.yaw,
            pitch: l.pitch,
        }
    }
}

impl Loc {
    /// The WIT location.
    pub fn to_location(&self) -> Location {
        Location::new(self.world.clone(), self.x, self.y, self.z).facing(self.yaw, self.pitch)
    }

    /// Whether `to` is more than `threshold` blocks away (or in another world).
    pub fn moved_beyond(&self, to: &Location, threshold: f64) -> bool {
        if to.world != self.world {
            return true;
        }
        let d2 = (to.x - self.x).powi(2) + (to.y - self.y).powi(2) + (to.z - self.z).powi(2);
        d2 > threshold * threshold
    }

    /// Block coordinates for display: `(x, y, z)`.
    pub fn block_coords(&self) -> (i64, i64, i64) {
        (
            self.x.floor() as i64,
            self.y.floor() as i64,
            self.z.floor() as i64,
        )
    }

    /// The world id without the `minecraft:` namespace, for display.
    pub fn world_short(&self) -> &str {
        self.world.strip_prefix("minecraft:").unwrap_or(&self.world)
    }
}

/// A home or warp name: lowercased, `[a-z0-9_-]{1,32}`.
pub fn normalize_name(input: &str) -> Option<String> {
    let name = input.trim().to_lowercase();
    let ok = !name.is_empty()
        && name.len() <= 32
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-');
    ok.then_some(name)
}

/// How many homes a player may have: `None` = unlimited (`essentials.homes.unlimited`), else
/// the largest tier `n` among the `essentials.homes.<n>` nodes they hold, at least `default`.
pub fn max_homes(default: u32, tiers: &[u32], has: impl Fn(&str) -> bool) -> Option<u32> {
    if has("essentials.homes.unlimited") {
        return None;
    }
    let mut sorted = tiers.to_vec();
    sorted.sort_unstable_by(|a, b| b.cmp(a));
    let best = sorted
        .into_iter()
        .find(|n| has(&format!("essentials.homes.{n}")))
        .unwrap_or(0);
    Some(best.max(default))
}

/// Seconds left before `until_tick`, rounded up (0 when passed).
pub fn seconds_left(until_tick: u64, now_tick: u64) -> u64 {
    until_tick.saturating_sub(now_tick).div_ceil(20)
}

/// A teleport request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Request {
    /// Unique id.
    pub id: u64,
    /// Who asked.
    pub from: Uuid,
    /// Who must accept.
    pub to: Uuid,
    /// `/tpahere`: `to` teleports to `from` (else `from` teleports to `to`).
    pub here: bool,
    /// Tick after which the request is gone.
    pub expires: u64,
}

impl Request {
    /// Who moves when the request is accepted.
    pub fn traveller(&self) -> Uuid {
        if self.here { self.to } else { self.from }
    }

    /// Where they go (the other party).
    pub fn destination(&self) -> Uuid {
        if self.here { self.from } else { self.to }
    }
}

/// Why no request could be taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoRequest {
    /// The player has no pending request at all.
    None,
    /// None from the named player.
    NoneFrom,
}

/// Pending teleport requests.
#[derive(Debug, Default)]
pub struct Requests {
    list: Vec<Request>,
    next_id: u64,
}

impl Requests {
    /// Adds a request, replacing an earlier one between the same two players. Returns its id.
    pub fn add(&mut self, from: Uuid, to: Uuid, here: bool, expires: u64) -> u64 {
        self.list.retain(|r| !(r.from == from && r.to == to));
        self.next_id += 1;
        let id = self.next_id;
        self.list.push(Request {
            id,
            from,
            to,
            here,
            expires,
        });
        id
    }

    /// Takes the newest active request addressed to `to` (from `from` when given).
    pub fn take(&mut self, to: Uuid, from: Option<Uuid>, now: u64) -> Result<Request, NoRequest> {
        let found = self
            .list
            .iter()
            .enumerate()
            .rev()
            .find(|(_, r)| r.to == to && r.expires > now && from.is_none_or(|f| r.from == f))
            .map(|(i, _)| i);
        match found {
            Some(i) => Ok(self.list.remove(i)),
            None if from.is_some() && self.list.iter().any(|r| r.to == to && r.expires > now) => {
                Err(NoRequest::NoneFrom)
            }
            None if from.is_some() => Err(NoRequest::NoneFrom),
            None => Err(NoRequest::None),
        }
    }

    /// Removes the request with this id if it is still pending.
    pub fn remove(&mut self, id: u64) -> Option<Request> {
        let i = self.list.iter().position(|r| r.id == id)?;
        Some(self.list.remove(i))
    }

    /// Removes and returns every expired request.
    pub fn expire(&mut self, now: u64) -> Vec<Request> {
        let (gone, keep): (Vec<Request>, Vec<Request>) =
            self.list.iter().partition(|r| r.expires <= now);
        self.list = keep;
        gone
    }

    /// Removes every request involving `who` (they left).
    pub fn drop_player(&mut self, who: Uuid) -> Vec<Request> {
        let (gone, keep): (Vec<Request>, Vec<Request>) =
            self.list.iter().partition(|r| r.from == who || r.to == who);
        self.list = keep;
        gone
    }

    /// Active requests addressed to `to`, oldest first.
    pub fn pending_for(&self, to: Uuid, now: u64) -> Vec<Request> {
        self.list
            .iter()
            .filter(|r| r.to == to && r.expires > now)
            .copied()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    #[test]
    fn locations_roundtrip_and_movement() {
        let l = Location::new("minecraft:overworld", 1.5, 64.0, -3.2).facing(90.0, 10.0);
        let loc = Loc::from(&l);
        assert_eq!(loc.to_location(), l);
        assert_eq!(loc.block_coords(), (1, 64, -4));
        assert_eq!(loc.world_short(), "overworld");
        assert!(!loc.moved_beyond(&Location::new("minecraft:overworld", 1.8, 64.0, -3.2), 0.5));
        assert!(loc.moved_beyond(&Location::new("minecraft:overworld", 2.1, 64.0, -3.2), 0.5));
        assert!(loc.moved_beyond(&Location::new("minecraft:the_nether", 1.5, 64.0, -3.2), 0.5));
        let json = serde_json::to_string(&loc).unwrap();
        assert_eq!(serde_json::from_str::<Loc>(&json).unwrap(), loc);
    }

    #[test]
    fn names() {
        assert_eq!(normalize_name("Base").as_deref(), Some("base"));
        assert_eq!(normalize_name("my_farm-2").as_deref(), Some("my_farm-2"));
        assert_eq!(normalize_name(""), None);
        assert_eq!(normalize_name("a.b"), None);
        assert_eq!(normalize_name(&"x".repeat(33)), None);
    }

    #[test]
    fn home_limits() {
        let tiers = [3, 10, 5];
        assert_eq!(max_homes(1, &tiers, |_| false), Some(1));
        assert_eq!(
            max_homes(1, &tiers, |n| n == "essentials.homes.5"
                || n == "essentials.homes.3"),
            Some(5)
        );
        assert_eq!(max_homes(4, &tiers, |n| n == "essentials.homes.3"), Some(4));
        assert_eq!(max_homes(1, &tiers, |n| n.ends_with("unlimited")), None);
        // A `*` wildcard grants every node, unlimited included.
        assert_eq!(max_homes(1, &tiers, |_| true), None);
    }

    #[test]
    fn cooldown_seconds() {
        assert_eq!(seconds_left(100, 100), 0);
        assert_eq!(seconds_left(100, 120), 0);
        assert_eq!(seconds_left(100, 99), 1);
        assert_eq!(seconds_left(140, 100), 2);
        assert_eq!(seconds_left(141, 100), 3);
    }

    #[test]
    fn requests_lifecycle() {
        let (a, b, c) = (u(1), u(2), u(3));
        let mut r = Requests::default();
        assert_eq!(r.take(b, None, 0), Err(NoRequest::None));
        let first = r.add(a, b, false, 100);
        let second = r.add(a, b, true, 120); // replaces the first
        assert_ne!(first, second);
        assert_eq!(r.pending_for(b, 0).len(), 1);
        r.add(c, b, false, 50);
        assert_eq!(r.take(b, Some(u(9)), 0), Err(NoRequest::NoneFrom));
        // Newest first: from c.
        let got = r.take(b, None, 10).unwrap();
        assert_eq!((got.from, got.traveller(), got.destination()), (c, c, b));
        let got = r.take(b, Some(a), 10).unwrap();
        assert!(got.here);
        assert_eq!((got.traveller(), got.destination()), (b, a));
        assert_eq!(r.take(b, None, 10), Err(NoRequest::None));

        let id = r.add(a, b, false, 100);
        assert!(r.expire(99).is_empty());
        assert_eq!(r.take(b, None, 100), Err(NoRequest::None));
        assert_eq!(r.expire(100).len(), 1);
        assert_eq!(r.remove(id), None);

        let id = r.add(a, c, false, 100);
        r.add(b, c, false, 100);
        assert_eq!(r.remove(id).map(|q| q.from), Some(a));
        assert_eq!(r.drop_player(b).len(), 1);
        assert!(r.pending_for(c, 0).is_empty());
    }
}
