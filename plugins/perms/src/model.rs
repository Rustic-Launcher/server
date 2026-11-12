//! The permission model: groups, users, tracks and node resolution. Pure logic (no host calls),
//! tested on the host target.
//!
//! # Resolution
//!
//! A check of `node` for a user looks at **sources** in order and the first source holding a
//! matching entry decides:
//!
//! 1. the user's own nodes;
//! 2. every group the user inherits (direct groups, then their parents, transitively), ordered
//!    by **weight** (highest first), then by inheritance depth (closest first), then by name.
//!
//! Inside a source the most specific entry wins: `a.b.c`, then `a.b.*`, then `a.*`, then `*`.
//! Temporary entries count while they have not expired; a temporary entry shadows a permanent
//! one on the same node. `group.<name>` nodes are answered by membership: a user holds
//! `group.admin` while they inherit `admin`.

use std::collections::{BTreeMap, BTreeSet, VecDeque};

use rustic::Uuid;
use serde::{Deserialize, Serialize};

/// A temporary node value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TempNode {
    /// Granted (`true`) or negated (`false`).
    pub value: bool,
    /// Unix seconds after which the entry is gone.
    pub expires: u64,
}

/// A group.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Group {
    /// Higher weights win (node resolution, prefix and suffix).
    pub weight: i32,
    /// Raw prefix (legacy `&` codes or MiniMessage).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    /// Raw suffix.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suffix: Option<String>,
    /// Groups this one inherits.
    pub parents: BTreeSet<String>,
    /// Permanent nodes.
    pub nodes: BTreeMap<String, bool>,
    /// Temporary nodes.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub temp_nodes: BTreeMap<String, TempNode>,
}

/// A user's data.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct User {
    /// Last known name.
    pub name: String,
    /// Permanent groups.
    pub groups: BTreeSet<String>,
    /// Temporary groups: name → expiry (unix seconds).
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub temp_groups: BTreeMap<String, u64>,
    /// Permanent nodes.
    pub nodes: BTreeMap<String, bool>,
    /// Temporary nodes.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub temp_nodes: BTreeMap<String, TempNode>,
    /// Personal prefix (overrides the groups').
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    /// Personal suffix.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suffix: Option<String>,
}

impl User {
    /// A new user in `default_group`.
    pub fn new(name: &str, default_group: &str) -> User {
        User {
            name: name.to_owned(),
            groups: BTreeSet::from([default_group.to_owned()]),
            ..User::default()
        }
    }

    /// Groups the user is directly in (permanent and active temporary), sorted by name.
    pub fn direct_groups(&self, now: u64) -> BTreeSet<String> {
        let mut out = self.groups.clone();
        out.extend(
            self.temp_groups
                .iter()
                .filter(|(_, exp)| **exp > now)
                .map(|(g, _)| g.clone()),
        );
        out
    }
}

/// A promotion ladder: groups from lowest to highest.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Track {
    /// Groups, lowest first.
    pub groups: Vec<String>,
}

/// Everything the plugin persists (also the `perms.toml` export format).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PermData {
    /// Groups by name.
    pub groups: BTreeMap<String, Group>,
    /// Tracks by name.
    pub tracks: BTreeMap<String, Track>,
    /// Users by UUID.
    pub users: BTreeMap<Uuid, User>,
}

/// Why an edit was refused. The message keys are looked up in [`crate::lang`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelError {
    /// No such group.
    UnknownGroup(String),
    /// No such track.
    UnknownTrack(String),
    /// Bad node syntax.
    InvalidNode(String),
    /// Bad group or track name.
    InvalidName(String),
    /// Adding this parent would make a cycle.
    Cycle(String, String),
    /// The default group cannot be deleted.
    DefaultGroup(String),
    /// The user is in several groups of the track.
    AmbiguousTrack(String),
    /// The track is empty.
    EmptyTrack(String),
}

/// Validates a permission node (already lowercased): dot-separated segments of
/// `[a-z0-9_:+-]`; the last segment may be `*`, and `*` alone means every node.
pub fn validate_node(node: &str) -> Result<(), ModelError> {
    let bad = || ModelError::InvalidNode(node.to_owned());
    if node == "*" {
        return Ok(());
    }
    if node.is_empty() || node.len() > 256 {
        return Err(bad());
    }
    let segments: Vec<&str> = node.split('.').collect();
    let last = segments.len() - 1;
    for (i, seg) in segments.iter().enumerate() {
        if *seg == "*" {
            if i != last || i == 0 {
                return Err(bad());
            }
            continue;
        }
        if seg.is_empty()
            || !seg
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "_:+-".contains(c))
        {
            return Err(bad());
        }
    }
    Ok(())
}

/// Parses user input for a node: lowercases it, and a leading `-` negates it.
/// Returns `(node, value)` where `value` is `false` for negated input.
pub fn parse_node_input(input: &str) -> Result<(String, bool), ModelError> {
    let lower = input.trim().to_ascii_lowercase();
    let (node, value) = match lower.strip_prefix('-') {
        Some(rest) => (rest.to_owned(), false),
        None => (lower, true),
    };
    validate_node(&node)?;
    Ok((node, value))
}

/// Validates a group or track name: `[a-z0-9_-]{1,32}` (input is lowercased first).
pub fn normalize_name(input: &str) -> Result<String, ModelError> {
    let name = input.trim().to_ascii_lowercase();
    if name.is_empty()
        || name.len() > 32
        || !name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-')
    {
        return Err(ModelError::InvalidName(input.to_owned()));
    }
    Ok(name)
}

/// The keys that can match `node`, most specific first: `a.b.c`, `a.b.*`, `a.*`, `*`.
pub fn candidates(node: &str) -> Vec<String> {
    let mut out = vec![node.to_owned()];
    if node == "*" {
        return out;
    }
    // `a.b.c` → prefixes `a.b`, `a`; `a.b.*` (base `a.b`) → prefix `a`.
    let base = node.strip_suffix(".*").unwrap_or(node);
    let segments: Vec<&str> = base.split('.').collect();
    for len in (1..segments.len()).rev() {
        out.push(format!("{}.*", segments[..len].join(".")));
    }
    out.push("*".to_owned());
    out
}

/// Looks `node` up in one source (permanent + temporary maps); most specific key first.
fn lookup(
    nodes: &BTreeMap<String, bool>,
    temp: &BTreeMap<String, TempNode>,
    keys: &[String],
    now: u64,
) -> Option<(String, bool)> {
    for key in keys {
        if let Some(t) = temp.get(key)
            && t.expires > now
        {
            return Some((key.clone(), t.value));
        }
        if let Some(v) = nodes.get(key) {
            return Some((key.clone(), *v));
        }
    }
    None
}

/// Where a check's verdict came from (for `/perms check`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verdict {
    /// The decision (`None`: undefined, the node's default applies).
    pub value: Option<bool>,
    /// `user` or `group <name>`.
    pub source: Option<String>,
    /// The matching key (`essentials.*`).
    pub key: Option<String>,
}

/// The outcome of a promotion or demotion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackMove {
    /// The user entered the track at its first group.
    Joined(String),
    /// Moved from one group to another.
    Moved {
        /// Left group.
        from: String,
        /// New group.
        to: String,
    },
    /// Removed from the track's first group (demotion off the ladder).
    Left(String),
    /// Already at the top (promote) or not on the track (demote).
    Nothing,
}

/// The current unix time in seconds.
pub fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

impl PermData {
    /// Makes sure the default group exists.
    pub fn ensure_default(&mut self, default_group: &str) -> bool {
        if self.groups.contains_key(default_group) {
            return false;
        }
        self.groups
            .insert(default_group.to_owned(), Group::default());
        true
    }

    /// The groups a user inherits, with their depth, ordered for resolution (weight desc,
    /// depth asc, name). A user without any group gets `default_group`.
    pub fn inherited_groups(
        &self,
        user: Option<&User>,
        default_group: &str,
        now: u64,
    ) -> Vec<(String, u32)> {
        let mut direct: Vec<String> = user
            .map(|u| u.direct_groups(now).into_iter().collect())
            .unwrap_or_default();
        direct.retain(|g| self.groups.contains_key(g));
        if direct.is_empty() && self.groups.contains_key(default_group) {
            direct.push(default_group.to_owned());
        }
        let mut seen: BTreeMap<String, u32> = BTreeMap::new();
        let mut queue: VecDeque<(String, u32)> = direct.into_iter().map(|g| (g, 0)).collect();
        while let Some((g, depth)) = queue.pop_front() {
            if seen.contains_key(&g) {
                continue;
            }
            let Some(group) = self.groups.get(&g) else {
                continue;
            };
            seen.insert(g.clone(), depth);
            for p in &group.parents {
                if !seen.contains_key(p) {
                    queue.push_back((p.clone(), depth + 1));
                }
            }
        }
        let mut out: Vec<(String, u32)> = seen.into_iter().collect();
        out.sort_by(|(a, da), (b, db)| {
            let wa = self.groups.get(a).map_or(0, |g| g.weight);
            let wb = self.groups.get(b).map_or(0, |g| g.weight);
            wb.cmp(&wa).then(da.cmp(db)).then(a.cmp(b))
        });
        out
    }

    /// Resolves `node` for `uuid`, explaining where the verdict comes from.
    pub fn resolve(&self, uuid: Uuid, node: &str, default_group: &str, now: u64) -> Verdict {
        let node = node.trim().to_ascii_lowercase();
        let user = self.users.get(&uuid);
        let groups = self.inherited_groups(user, default_group, now);

        if let Some(g) = node.strip_prefix("group.")
            && !g.contains('.')
            && g != "*"
        {
            let member = groups.iter().any(|(name, _)| name == g);
            if member {
                return Verdict {
                    value: Some(true),
                    source: Some(format!("group {g}")),
                    key: Some(node.clone()),
                };
            }
        }

        let keys = candidates(&node);
        if let Some(u) = user
            && let Some((key, v)) = lookup(&u.nodes, &u.temp_nodes, &keys, now)
        {
            return Verdict {
                value: Some(v),
                source: Some("user".to_owned()),
                key: Some(key),
            };
        }
        for (name, _) in &groups {
            let g = &self.groups[name];
            if let Some((key, v)) = lookup(&g.nodes, &g.temp_nodes, &keys, now) {
                return Verdict {
                    value: Some(v),
                    source: Some(format!("group {name}")),
                    key: Some(key),
                };
            }
        }
        Verdict {
            value: None,
            source: None,
            key: None,
        }
    }

    /// A user's direct groups (active), highest weight first: the first is the primary group.
    pub fn user_groups(&self, uuid: Uuid, default_group: &str, now: u64) -> Vec<String> {
        let mut groups: Vec<String> = self
            .users
            .get(&uuid)
            .map(|u| u.direct_groups(now).into_iter().collect())
            .unwrap_or_default();
        groups.retain(|g| self.groups.contains_key(g));
        if groups.is_empty() && self.groups.contains_key(default_group) {
            groups.push(default_group.to_owned());
        }
        groups.sort_by(|a, b| {
            let wa = self.groups.get(a).map_or(0, |g| g.weight);
            let wb = self.groups.get(b).map_or(0, |g| g.weight);
            wb.cmp(&wa).then(a.cmp(b))
        });
        groups
    }

    /// The user's prefix (or suffix with `suffix = true`): personal, else the highest-weight
    /// inherited group that has one.
    pub fn meta(&self, uuid: Uuid, suffix: bool, default_group: &str, now: u64) -> Option<String> {
        let user = self.users.get(&uuid);
        if let Some(u) = user {
            let own = if suffix { &u.suffix } else { &u.prefix };
            if own.is_some() {
                return own.clone();
            }
        }
        self.inherited_groups(user, default_group, now)
            .into_iter()
            .find_map(|(name, _)| {
                let g = &self.groups[&name];
                if suffix {
                    g.suffix.clone()
                } else {
                    g.prefix.clone()
                }
            })
    }

    /// Adds `parent` to `group`'s parents, refusing cycles.
    pub fn add_parent(&mut self, group: &str, parent: &str) -> Result<bool, ModelError> {
        if !self.groups.contains_key(parent) {
            return Err(ModelError::UnknownGroup(parent.to_owned()));
        }
        if !self.groups.contains_key(group) {
            return Err(ModelError::UnknownGroup(group.to_owned()));
        }
        if group == parent || self.inherits(parent, group) {
            return Err(ModelError::Cycle(group.to_owned(), parent.to_owned()));
        }
        Ok(self
            .groups
            .get_mut(group)
            .is_some_and(|g| g.parents.insert(parent.to_owned())))
    }

    /// Whether `group` inherits `ancestor` (transitively).
    pub fn inherits(&self, group: &str, ancestor: &str) -> bool {
        let mut stack = vec![group.to_owned()];
        let mut seen = BTreeSet::new();
        while let Some(g) = stack.pop() {
            if !seen.insert(g.clone()) {
                continue;
            }
            if let Some(def) = self.groups.get(&g) {
                for p in &def.parents {
                    if p == ancestor {
                        return true;
                    }
                    stack.push(p.clone());
                }
            }
        }
        false
    }

    /// Deletes a group and every reference to it (parents, users, tracks).
    pub fn delete_group(&mut self, name: &str, default_group: &str) -> Result<(), ModelError> {
        if name == default_group {
            return Err(ModelError::DefaultGroup(name.to_owned()));
        }
        if self.groups.remove(name).is_none() {
            return Err(ModelError::UnknownGroup(name.to_owned()));
        }
        for g in self.groups.values_mut() {
            g.parents.remove(name);
        }
        for u in self.users.values_mut() {
            u.groups.remove(name);
            u.temp_groups.remove(name);
        }
        for t in self.tracks.values_mut() {
            t.groups.retain(|g| g != name);
        }
        Ok(())
    }

    /// Promotes (`up = true`) or demotes a user along a track.
    pub fn move_on_track(
        &mut self,
        uuid: Uuid,
        name_hint: &str,
        track: &str,
        up: bool,
        default_group: &str,
        now: u64,
    ) -> Result<TrackMove, ModelError> {
        let ladder = self
            .tracks
            .get(track)
            .ok_or_else(|| ModelError::UnknownTrack(track.to_owned()))?
            .groups
            .clone();
        if ladder.is_empty() {
            return Err(ModelError::EmptyTrack(track.to_owned()));
        }
        let user = self
            .users
            .entry(uuid)
            .or_insert_with(|| User::new(name_hint, default_group));
        let direct = user.direct_groups(now);
        let on: Vec<usize> = ladder
            .iter()
            .enumerate()
            .filter(|(_, g)| direct.contains(*g))
            .map(|(i, _)| i)
            .collect();
        if on.len() > 1 {
            return Err(ModelError::AmbiguousTrack(track.to_owned()));
        }
        let outcome = match (on.first().copied(), up) {
            (None, true) => {
                user.groups.insert(ladder[0].clone());
                TrackMove::Joined(ladder[0].clone())
            }
            (None, false) => TrackMove::Nothing,
            (Some(i), true) if i + 1 < ladder.len() => {
                user.groups.remove(&ladder[i]);
                user.temp_groups.remove(&ladder[i]);
                user.groups.insert(ladder[i + 1].clone());
                TrackMove::Moved {
                    from: ladder[i].clone(),
                    to: ladder[i + 1].clone(),
                }
            }
            (Some(_), true) => TrackMove::Nothing,
            (Some(0), false) => {
                user.groups.remove(&ladder[0]);
                user.temp_groups.remove(&ladder[0]);
                TrackMove::Left(ladder[0].clone())
            }
            (Some(i), false) => {
                user.groups.remove(&ladder[i]);
                user.temp_groups.remove(&ladder[i]);
                user.groups.insert(ladder[i - 1].clone());
                TrackMove::Moved {
                    from: ladder[i].clone(),
                    to: ladder[i - 1].clone(),
                }
            }
        };
        Ok(outcome)
    }

    /// Removes expired temporary entries. Returns the users and whether any group changed, plus
    /// one line per removed entry (`(user or None for a group, description)`).
    pub fn purge_expired(&mut self, now: u64) -> Purge {
        let mut purge = Purge::default();
        for (uuid, u) in &mut self.users {
            let before = purge.removed.len();
            u.temp_groups.retain(|g, exp| {
                let keep = *exp > now;
                if !keep {
                    purge.removed.push((Some(*uuid), Expired::Group(g.clone())));
                }
                keep
            });
            u.temp_nodes.retain(|n, t| {
                let keep = t.expires > now;
                if !keep {
                    purge.removed.push((Some(*uuid), Expired::Node(n.clone())));
                }
                keep
            });
            if purge.removed.len() > before {
                purge.users.push(*uuid);
            }
        }
        for (name, g) in &mut self.groups {
            g.temp_nodes.retain(|n, t| {
                let keep = t.expires > now;
                if !keep {
                    purge.groups_changed = true;
                    purge
                        .removed
                        .push((None, Expired::GroupNode(name.clone(), n.clone())));
                }
                keep
            });
        }
        purge
    }

    /// Checks imported data: valid names and nodes, parents/tracks/memberships naming existing
    /// groups, no inheritance cycle. Returns the first problem found.
    pub fn validate(&self) -> Result<(), String> {
        let node_ok = |n: &String| validate_node(n).map_err(|_| format!("invalid node `{n}`"));
        let name_ok = |n: &String| match normalize_name(n) {
            Ok(norm) if norm == *n => Ok(()),
            _ => Err(format!("invalid name `{n}` (lowercase a-z 0-9 _ -)")),
        };
        let group_ok = |g: &String, ctx: &str| {
            if self.groups.contains_key(g) {
                Ok(())
            } else {
                Err(format!("{ctx}: unknown group `{g}`"))
            }
        };
        for (name, g) in &self.groups {
            name_ok(name)?;
            g.nodes.keys().try_for_each(node_ok)?;
            g.temp_nodes.keys().try_for_each(node_ok)?;
            for p in &g.parents {
                group_ok(p, &format!("parents of `{name}`"))?;
            }
            if self.inherits(name, name) {
                return Err(format!("group `{name}` inherits itself"));
            }
        }
        for (name, t) in &self.tracks {
            name_ok(name)?;
            for g in &t.groups {
                group_ok(g, &format!("track `{name}`"))?;
            }
        }
        for (id, u) in &self.users {
            u.nodes.keys().try_for_each(node_ok)?;
            u.temp_nodes.keys().try_for_each(node_ok)?;
            for g in u.groups.iter().chain(u.temp_groups.keys()) {
                group_ok(g, &format!("player {id}"))?;
            }
        }
        Ok(())
    }

    /// Finds a known user by name (case-insensitive).
    pub fn user_by_name(&self, name: &str) -> Option<Uuid> {
        self.users
            .iter()
            .find(|(_, u)| u.name.eq_ignore_ascii_case(name))
            .map(|(id, _)| *id)
    }
}

/// What expired.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expired {
    /// A temporary group membership.
    Group(String),
    /// A user's temporary node.
    Node(String),
    /// A group's temporary node: (group, node).
    GroupNode(String, String),
}

/// The result of [`PermData::purge_expired`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Purge {
    /// Users whose data changed.
    pub users: Vec<Uuid>,
    /// Whether some group's data changed.
    pub groups_changed: bool,
    /// Every removed entry.
    pub removed: Vec<(Option<Uuid>, Expired)>,
}

/// Formats a duration in seconds as `1d 2h 3m 4s` (largest units first, zero parts omitted).
pub fn format_duration(mut secs: u64) -> String {
    if secs == 0 {
        return "0s".to_owned();
    }
    let mut parts = Vec::new();
    for (unit, size) in [("d", 86_400), ("h", 3_600), ("m", 60), ("s", 1)] {
        if secs >= size {
            parts.push(format!("{}{unit}", secs / size));
            secs %= size;
        }
    }
    parts.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEF: &str = "default";

    fn u(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    fn data() -> PermData {
        let mut d = PermData::default();
        d.ensure_default(DEF);
        d.groups.insert(
            "vip".into(),
            Group {
                weight: 10,
                prefix: Some("&6[VIP] ".into()),
                parents: BTreeSet::from([DEF.to_owned()]),
                ..Group::default()
            },
        );
        d.groups.insert(
            "admin".into(),
            Group {
                weight: 100,
                prefix: Some("<red>[Admin] ".into()),
                suffix: Some(" <gray>(staff)".into()),
                parents: BTreeSet::from(["vip".to_owned()]),
                nodes: BTreeMap::from([("*".to_owned(), true)]),
                ..Group::default()
            },
        );
        d
    }

    #[test]
    fn node_validation() {
        assert!(validate_node("a.b.c").is_ok());
        assert!(validate_node("a.b.*").is_ok());
        assert!(validate_node("*").is_ok());
        assert!(validate_node("essentials.homes.5").is_ok());
        assert!(validate_node("a.*.c").is_err());
        assert!(validate_node("a..b").is_err());
        assert!(validate_node("A.b").is_err());
        assert!(validate_node("").is_err());
        assert!(validate_node(".*").is_err());
        assert_eq!(
            parse_node_input("-Essentials.Home").unwrap(),
            ("essentials.home".into(), false)
        );
        assert_eq!(parse_node_input("x.y").unwrap(), ("x.y".into(), true));
        assert!(parse_node_input("-").is_err());
        assert_eq!(normalize_name("VIP").unwrap(), "vip");
        assert!(normalize_name("a b").is_err());
        assert!(normalize_name("").is_err());
    }

    #[test]
    fn candidate_order() {
        assert_eq!(candidates("a.b.c"), vec!["a.b.c", "a.b.*", "a.*", "*"]);
        assert_eq!(candidates("a"), vec!["a", "*"]);
        assert_eq!(candidates("a.b.*"), vec!["a.b.*", "a.*", "*"]);
        assert_eq!(candidates("a.*"), vec!["a.*", "*"]);
        assert_eq!(candidates("*"), vec!["*"]);
    }

    #[test]
    fn user_nodes_beat_groups_and_specific_beats_wildcard() {
        let mut d = data();
        let p = u(1);
        let mut user = User::new("Alice", DEF);
        user.nodes.insert("shop.*".into(), true);
        user.nodes.insert("shop.admin".into(), false);
        d.users.insert(p, user);
        d.groups
            .get_mut(DEF)
            .unwrap()
            .nodes
            .insert("build".into(), true);

        assert_eq!(d.resolve(p, "shop.buy", DEF, 0).value, Some(true));
        let v = d.resolve(p, "shop.admin", DEF, 0);
        assert_eq!(v.value, Some(false));
        assert_eq!(v.source.as_deref(), Some("user"));
        let v = d.resolve(p, "build", DEF, 0);
        assert_eq!(v.value, Some(true));
        assert_eq!(v.source.as_deref(), Some("group default"));
        assert_eq!(d.resolve(p, "fly", DEF, 0).value, None);
        // Unknown users get the default group.
        assert_eq!(d.resolve(u(99), "build", DEF, 0).value, Some(true));
    }

    #[test]
    fn weights_and_inheritance() {
        let mut d = data();
        let p = u(2);
        let mut user = User::new("Bob", DEF);
        user.groups.insert("admin".into());
        d.users.insert(p, user);
        d.groups
            .get_mut(DEF)
            .unwrap()
            .nodes
            .insert("essentials.fly".into(), false);
        // admin (weight 100) has `*`: it beats default's negation.
        let v = d.resolve(p, "essentials.fly", DEF, 0);
        assert_eq!(v.value, Some(true));
        assert_eq!(v.source.as_deref(), Some("group admin"));
        let order: Vec<String> = d
            .inherited_groups(d.users.get(&p), DEF, 0)
            .into_iter()
            .map(|(g, _)| g)
            .collect();
        assert_eq!(order, vec!["admin", "vip", "default"]);
        assert_eq!(d.resolve(p, "group.vip", DEF, 0).value, Some(true));
        assert_eq!(d.resolve(u(3), "group.vip", DEF, 0).value, None);
        assert_eq!(d.user_groups(p, DEF, 0), vec!["admin", "default"]);
    }

    #[test]
    fn temporary_entries_expire() {
        let mut d = data();
        let p = u(4);
        let mut user = User::new("Cleo", DEF);
        user.temp_nodes.insert(
            "fly".into(),
            TempNode {
                value: true,
                expires: 100,
            },
        );
        user.temp_groups.insert("vip".into(), 50);
        d.users.insert(p, user);
        assert_eq!(d.resolve(p, "fly", DEF, 99).value, Some(true));
        assert_eq!(d.resolve(p, "fly", DEF, 100).value, None);
        assert_eq!(d.resolve(p, "group.vip", DEF, 10).value, Some(true));
        assert_eq!(d.resolve(p, "group.vip", DEF, 60).value, None);
        assert_eq!(d.meta(p, false, DEF, 10).as_deref(), Some("&6[VIP] "));
        assert_eq!(d.meta(p, false, DEF, 60), None);

        let purge = d.purge_expired(60);
        assert_eq!(purge.users, vec![p]);
        assert_eq!(purge.removed, vec![(Some(p), Expired::Group("vip".into()))]);
        let purge = d.purge_expired(200);
        assert_eq!(purge.removed, vec![(Some(p), Expired::Node("fly".into()))]);
        assert!(d.purge_expired(300).removed.is_empty());
    }

    #[test]
    fn meta_prefers_user_then_weight() {
        let mut d = data();
        let p = u(5);
        let mut user = User::new("Dan", DEF);
        user.groups.insert("admin".into());
        d.users.insert(p, user);
        assert_eq!(d.meta(p, false, DEF, 0).as_deref(), Some("<red>[Admin] "));
        assert_eq!(d.meta(p, true, DEF, 0).as_deref(), Some(" <gray>(staff)"));
        d.users.get_mut(&p).unwrap().prefix = Some("&b[Me] ".into());
        assert_eq!(d.meta(p, false, DEF, 0).as_deref(), Some("&b[Me] "));
    }

    #[test]
    fn parents_refuse_cycles() {
        let mut d = data();
        assert_eq!(
            d.add_parent(DEF, "admin"),
            Err(ModelError::Cycle(DEF.into(), "admin".into()))
        );
        assert_eq!(
            d.add_parent("vip", "vip"),
            Err(ModelError::Cycle("vip".into(), "vip".into()))
        );
        assert!(matches!(
            d.add_parent("vip", "nope"),
            Err(ModelError::UnknownGroup(_))
        ));
        assert_eq!(d.add_parent("vip", DEF), Ok(false));
        assert!(d.inherits("admin", DEF));
    }

    #[test]
    fn delete_group_cleans_references() {
        let mut d = data();
        let mut user = User::new("Eve", DEF);
        user.groups.insert("vip".into());
        d.users.insert(u(6), user);
        d.tracks.insert(
            "staff".into(),
            Track {
                groups: vec![DEF.into(), "vip".into(), "admin".into()],
            },
        );
        assert_eq!(
            d.delete_group(DEF, DEF),
            Err(ModelError::DefaultGroup(DEF.into()))
        );
        d.delete_group("vip", DEF).unwrap();
        assert!(!d.groups["admin"].parents.contains("vip"));
        assert!(!d.users[&u(6)].groups.contains("vip"));
        assert_eq!(d.tracks["staff"].groups, vec![DEF, "admin"]);
        assert!(d.delete_group("vip", DEF).is_err());
    }

    #[test]
    fn tracks_promote_and_demote() {
        let mut d = data();
        d.tracks.insert(
            "staff".into(),
            Track {
                groups: vec!["vip".into(), "admin".into()],
            },
        );
        let p = u(7);
        let mv = |d: &mut PermData, up| d.move_on_track(p, "Finn", "staff", up, DEF, 0).unwrap();
        assert_eq!(mv(&mut d, false), TrackMove::Nothing);
        assert_eq!(mv(&mut d, true), TrackMove::Joined("vip".into()));
        assert_eq!(
            mv(&mut d, true),
            TrackMove::Moved {
                from: "vip".into(),
                to: "admin".into()
            }
        );
        assert_eq!(mv(&mut d, true), TrackMove::Nothing);
        assert_eq!(
            mv(&mut d, false),
            TrackMove::Moved {
                from: "admin".into(),
                to: "vip".into()
            }
        );
        assert_eq!(mv(&mut d, false), TrackMove::Left("vip".into()));
        assert_eq!(d.users[&p].groups, BTreeSet::from([DEF.to_owned()]));
        d.users
            .get_mut(&p)
            .unwrap()
            .groups
            .extend(["vip".to_owned(), "admin".to_owned()]);
        assert_eq!(
            d.move_on_track(p, "Finn", "staff", true, DEF, 0),
            Err(ModelError::AmbiguousTrack("staff".into()))
        );
        assert!(matches!(
            d.move_on_track(p, "Finn", "nope", true, DEF, 0),
            Err(ModelError::UnknownTrack(_))
        ));
    }

    #[test]
    fn json_and_toml_roundtrip() {
        let mut d = data();
        let mut user = User::new("Gus", DEF);
        user.temp_groups.insert("vip".into(), 123);
        user.temp_nodes.insert(
            "a.b".into(),
            TempNode {
                value: false,
                expires: 456,
            },
        );
        d.users.insert(u(8), user);
        d.tracks.insert(
            "t".into(),
            Track {
                groups: vec!["vip".into()],
            },
        );
        let json = serde_json::to_string(&d).unwrap();
        assert_eq!(serde_json::from_str::<PermData>(&json).unwrap(), d);
        let toml_text = rustic::toml::to_string(&d).unwrap();
        assert_eq!(rustic::toml::from_str::<PermData>(&toml_text).unwrap(), d);
    }

    #[test]
    fn validation_of_imports() {
        let mut d = data();
        assert_eq!(d.validate(), Ok(()));
        d.groups
            .get_mut("vip")
            .unwrap()
            .nodes
            .insert("Bad Node".into(), true);
        assert!(d.validate().unwrap_err().contains("invalid node"));
        let mut d = data();
        d.groups
            .get_mut(DEF)
            .unwrap()
            .parents
            .insert("admin".into());
        assert!(d.validate().unwrap_err().contains("inherits itself"));
        let mut d = data();
        d.users.insert(u(9), User::new("x", "ghost"));
        assert!(d.validate().unwrap_err().contains("unknown group `ghost`"));
        let mut d = data();
        d.groups.insert("Caps".into(), Group::default());
        assert!(d.validate().unwrap_err().contains("invalid name"));
    }

    #[test]
    fn durations() {
        assert_eq!(format_duration(0), "0s");
        assert_eq!(format_duration(59), "59s");
        assert_eq!(format_duration(3_661), "1h 1m 1s");
        assert_eq!(format_duration(90_000), "1d 1h");
    }
}
