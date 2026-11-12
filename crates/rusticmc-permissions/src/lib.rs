//! Rustic's default permission engine.
//!
//! The data lives in one TOML file, `permissions.toml`:
//!
//! ```toml
//! default_group = "default"
//!
//! [groups.default]
//! weight = 0
//! prefix = "[Member] "
//! nodes = { "homes.home" = true }
//!
//! [groups.admin]
//! weight = 100
//! inherits = ["default"]
//! prefix = "[Admin] "
//! nodes = { "*" = true, "homes.debug" = false }
//!
//! [users."069a79f4-44e9-4726-a5be-fca90e38aaf5"]
//! name = "Notch"              # informational only
//! groups = ["admin"]
//! nodes = { "essentials.fly" = true }
//! ```
//!
//! Resolution of `check(player, "a.b.c")`:
//!
//! 1. Candidate keys are tried from the most specific to the least: `a.b.c`, `a.b.*`, `a.*`,
//!    `*`. The first key that has a value decides (so an exact node beats any wildcard, and
//!    `a.b.*` beats `*`).
//! 2. For one key, a value set on the user beats any group value.
//! 3. Between groups, the group with the higher `weight` wins, then the one closer to the user
//!    in the inheritance graph (directly assigned = 0), then the group name (deterministic).
//! 4. Every player is implicitly a member of `default_group` (players absent from the file get
//!    exactly that group).
//! 5. A `false` value is a negation: it denies even if a less specific key allows.
//!
//! Nothing found gives [`Tristate::Undefined`]: the caller falls back to the node's declared
//! default. Inheritance cycles and references to unknown groups are refused at load time.
//!
//! Saving is atomic (temporary file, fsync, rename).

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, VecDeque};
use std::io::Write as _;
use std::path::{Path, PathBuf};

/// The result of a permission lookup.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Tristate {
    /// Explicitly granted.
    Allow,
    /// Explicitly denied.
    Deny,
    /// No rule: use the node's default.
    Undefined,
}

impl Tristate {
    /// `Some(bool)` for a decision, `None` for undefined.
    pub fn as_bool(self) -> Option<bool> {
        match self {
            Tristate::Allow => Some(true),
            Tristate::Deny => Some(false),
            Tristate::Undefined => None,
        }
    }

    /// From an optional decision.
    pub fn from_option(v: Option<bool>) -> Self {
        match v {
            Some(true) => Tristate::Allow,
            Some(false) => Tristate::Deny,
            None => Tristate::Undefined,
        }
    }
}

/// Errors of the permission engine.
#[derive(Debug, thiserror::Error)]
pub enum PermissionsError {
    /// Reading or writing the file failed.
    #[error("{path}: {source}")]
    Io {
        /// The file.
        path: PathBuf,
        /// The cause.
        #[source]
        source: std::io::Error,
    },
    /// The file is not valid TOML or does not match the schema.
    #[error("invalid permissions file: {0}")]
    Parse(String),
    /// The content is inconsistent (unknown group, bad node, bad UUID...).
    #[error("invalid permissions: {0}")]
    Invalid(String),
    /// Groups inherit from each other in a loop.
    #[error("group inheritance cycle: {}", .0.join(" -> "))]
    Cycle(Vec<String>),
    /// No file path to save to.
    #[error("the permission engine has no file path")]
    NoPath,
}

/// A group in the file.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GroupDef {
    /// Higher weight wins conflicts between groups.
    #[serde(default)]
    pub weight: i32,
    /// Parent groups whose nodes this group inherits.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inherits: Vec<String>,
    /// Chat prefix.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    /// Chat suffix.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suffix: Option<String>,
    /// Nodes (`true` grants, `false` negates), wildcards allowed.
    #[serde(default)]
    pub nodes: BTreeMap<String, bool>,
}

/// A user in the file, keyed by hyphenated UUID.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserDef {
    /// Last known name (informational).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Groups the user is a member of (the default group is implicit).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<String>,
    /// User-specific prefix (beats the groups').
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
    /// User-specific suffix.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suffix: Option<String>,
    /// User-specific nodes (beat the groups').
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub nodes: BTreeMap<String, bool>,
}

/// The whole file.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PermissionsFile {
    /// The group every player is a member of.
    #[serde(default = "default_group_name")]
    pub default_group: String,
    /// Groups by name.
    #[serde(default)]
    pub groups: BTreeMap<String, GroupDef>,
    /// Users by hyphenated UUID.
    #[serde(default)]
    pub users: BTreeMap<String, UserDef>,
}

fn default_group_name() -> String {
    "default".to_owned()
}

impl Default for PermissionsFile {
    fn default() -> Self {
        let mut groups = BTreeMap::new();
        groups.insert(default_group_name(), GroupDef::default());
        Self {
            default_group: default_group_name(),
            groups,
            users: BTreeMap::new(),
        }
    }
}

/// Validate a node: dot-separated segments of `[a-z0-9_:+-]`, the last one may be `*`;
/// `*` alone means everything.
pub fn validate_node(node: &str) -> Result<(), String> {
    if node.is_empty() || node.len() > 256 {
        return Err(format!("node `{node}` must be 1 to 256 characters"));
    }
    if node == "*" {
        return Ok(());
    }
    let segments: Vec<&str> = node.split('.').collect();
    for (i, seg) in segments.iter().enumerate() {
        if *seg == "*" {
            if i + 1 != segments.len() {
                return Err(format!("node `{node}`: `*` must be the last segment"));
            }
            continue;
        }
        if seg.is_empty()
            || !seg
                .bytes()
                .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' | b':' | b'+'))
        {
            return Err(format!(
                "node `{node}`: segment `{seg}` must be non-empty lowercase [a-z0-9_:+-]"
            ));
        }
    }
    Ok(())
}

/// The keys tried for `node`, most specific first: `a.b.c`, `a.b.*`, `a.*`, `*`.
pub fn candidate_keys(node: &str) -> Vec<String> {
    let mut out = vec![node.to_owned()];
    let base = node.strip_suffix(".*").unwrap_or(node);
    if base != "*" {
        let mut end = base.len();
        while let Some(dot) = base[..end].rfind('.') {
            out.push(format!("{}.*", &base[..dot]));
            end = dot;
        }
        if node != "*" {
            out.push("*".to_owned());
        }
    }
    out.dedup();
    out
}

/// Parse a UUID written as 32 hex digits, with or without hyphens.
pub fn parse_uuid(s: &str) -> Option<u128> {
    let hex: String = s.chars().filter(|c| *c != '-').collect();
    if hex.len() != 32 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    if s.contains('-') {
        let parts: Vec<&str> = s.split('-').collect();
        let lens: Vec<usize> = parts.iter().map(|p| p.len()).collect();
        if lens != [8, 4, 4, 4, 12] {
            return None;
        }
    }
    u128::from_str_radix(&hex, 16).ok()
}

/// Format a UUID as lowercase hyphenated hex.
pub fn format_uuid(v: u128) -> String {
    let h = format!("{v:032x}");
    format!(
        "{}-{}-{}-{}-{}",
        &h[0..8],
        &h[8..12],
        &h[12..16],
        &h[16..20],
        &h[20..32]
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Rank {
    user: bool,
    weight: i32,
    depth: u32,
}

impl Rank {
    /// Whether `self` beats `other` (the name tie-break is handled by iteration order).
    fn beats(&self, other: &Rank) -> bool {
        (self.user, self.weight, std::cmp::Reverse(self.depth))
            > (other.user, other.weight, std::cmp::Reverse(other.depth))
    }
}

type Flat = HashMap<String, bool>;

/// The default permission engine.
#[derive(Clone, Debug)]
pub struct PermissionEngine {
    file: PermissionsFile,
    path: Option<PathBuf>,
    users: HashMap<u128, Flat>,
    default_flat: Flat,
}

impl Default for PermissionEngine {
    fn default() -> Self {
        // The default file always validates.
        Self::from_file(PermissionsFile::default()).unwrap_or_else(|_| Self {
            file: PermissionsFile::default(),
            path: None,
            users: HashMap::new(),
            default_flat: HashMap::new(),
        })
    }
}

impl PermissionEngine {
    /// An engine over validated file content (not tied to a path).
    pub fn from_file(file: PermissionsFile) -> Result<Self, PermissionsError> {
        let file = normalize(file)?;
        validate(&file)?;
        let mut engine = Self {
            file,
            path: None,
            users: HashMap::new(),
            default_flat: HashMap::new(),
        };
        engine.rebuild()?;
        Ok(engine)
    }

    /// Parse TOML text.
    pub fn parse(text: &str) -> Result<Self, PermissionsError> {
        let file: PermissionsFile =
            toml::from_str(text).map_err(|e| PermissionsError::Parse(e.to_string()))?;
        Self::from_file(file)
    }

    /// Load `path`, which must exist.
    pub fn load(path: &Path) -> Result<Self, PermissionsError> {
        let text = std::fs::read_to_string(path).map_err(|source| PermissionsError::Io {
            path: path.to_owned(),
            source,
        })?;
        let mut engine = Self::parse(&text)?;
        engine.path = Some(path.to_owned());
        Ok(engine)
    }

    /// Load `path`, or create it with a default group when it does not exist.
    pub fn load_or_create(path: &Path) -> Result<Self, PermissionsError> {
        if path.exists() {
            return Self::load(path);
        }
        let mut engine = Self::default();
        engine.set_path(path);
        engine.save()?;
        Ok(engine)
    }

    /// The file this engine saves to.
    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// Set the file this engine saves to.
    pub fn set_path(&mut self, path: impl Into<PathBuf>) {
        self.path = Some(path.into());
    }

    /// The current content.
    pub fn file(&self) -> &PermissionsFile {
        &self.file
    }

    /// The content as TOML.
    pub fn to_toml(&self) -> Result<String, PermissionsError> {
        toml::to_string_pretty(&self.file).map_err(|e| PermissionsError::Parse(e.to_string()))
    }

    /// Save atomically to the engine's path.
    pub fn save(&self) -> Result<(), PermissionsError> {
        let path = self.path.clone().ok_or(PermissionsError::NoPath)?;
        self.save_to(&path)
    }

    /// Save atomically to `path` (write `<path>.tmp`, fsync, rename over `path`).
    pub fn save_to(&self, path: &Path) -> Result<(), PermissionsError> {
        let text = self.to_toml()?;
        let io = |source| PermissionsError::Io {
            path: path.to_owned(),
            source,
        };
        if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
            std::fs::create_dir_all(dir).map_err(io)?;
        }
        let mut tmp = path.as_os_str().to_owned();
        tmp.push(".tmp");
        let tmp = PathBuf::from(tmp);
        {
            let mut f = std::fs::File::create(&tmp).map_err(io)?;
            f.write_all(text.as_bytes()).map_err(io)?;
            f.sync_all().map_err(io)?;
        }
        std::fs::rename(&tmp, path).map_err(io)
    }

    /// Look `node` up for `player` (see the crate docs for the rules).
    pub fn check(&self, player: u128, node: &str) -> Tristate {
        let flat = self.users.get(&player).unwrap_or(&self.default_flat);
        let lowered;
        let node = if node.bytes().any(|b| b.is_ascii_uppercase()) {
            lowered = node.to_ascii_lowercase();
            lowered.as_str()
        } else {
            node
        };
        if let Some(v) = flat.get(node) {
            return Tristate::from_option(Some(*v));
        }
        for key in candidate_keys(node).iter().skip(1) {
            if let Some(v) = flat.get(key.as_str()) {
                return Tristate::from_option(Some(*v));
            }
        }
        Tristate::Undefined
    }

    /// The groups a player is directly a member of (the default group when none).
    pub fn groups_of(&self, player: u128) -> Vec<String> {
        let mut out = self
            .file
            .users
            .get(&format_uuid(player))
            .map(|u| u.groups.clone())
            .unwrap_or_default();
        if !out.contains(&self.file.default_group) {
            out.push(self.file.default_group.clone());
        }
        out
    }

    /// The highest-weight group of a player.
    pub fn primary_group(&self, player: u128) -> Option<String> {
        self.groups_of(player)
            .into_iter()
            .filter_map(|g| self.file.groups.get(&g).map(|d| (d.weight, g)))
            .max_by(|a, b| a.0.cmp(&b.0).then_with(|| b.1.cmp(&a.1)))
            .map(|(_, g)| g)
    }

    fn meta(
        &self,
        player: u128,
        pick: impl Fn(&GroupDef) -> Option<&String>,
        user_pick: impl Fn(&UserDef) -> Option<&String>,
    ) -> Option<String> {
        if let Some(v) = self
            .file
            .users
            .get(&format_uuid(player))
            .and_then(&user_pick)
        {
            return Some(v.clone());
        }
        self.expand(&self.groups_of(player))
            .into_iter()
            .filter_map(|(name, depth)| {
                let g = self.file.groups.get(&name)?;
                pick(g).map(|v| (g.weight, depth, name, v.clone()))
            })
            // Highest weight, then smallest depth, then smallest name.
            .max_by(|a, b| {
                a.0.cmp(&b.0)
                    .then_with(|| b.1.cmp(&a.1))
                    .then_with(|| b.2.cmp(&a.2))
            })
            .map(|(_, _, _, v)| v)
    }

    /// The player's prefix: their own, else the best group's (weight, then closeness).
    pub fn prefix(&self, player: u128) -> Option<String> {
        self.meta(player, |g| g.prefix.as_ref(), |u| u.prefix.as_ref())
    }

    /// The player's suffix, like [`PermissionEngine::prefix`].
    pub fn suffix(&self, player: u128) -> Option<String> {
        self.meta(player, |g| g.suffix.as_ref(), |u| u.suffix.as_ref())
    }

    /// A group's own prefix.
    pub fn group_prefix(&self, group: &str) -> Option<String> {
        self.file.groups.get(group).and_then(|g| g.prefix.clone())
    }

    /// Whether a group exists.
    pub fn has_group(&self, group: &str) -> bool {
        self.file.groups.contains_key(group)
    }

    /// Add a player to a group (which must exist).
    pub fn add_user_group(&mut self, player: u128, group: &str) -> Result<(), PermissionsError> {
        if !self.has_group(group) {
            return Err(PermissionsError::Invalid(format!("no group `{group}`")));
        }
        let user = self.file.users.entry(format_uuid(player)).or_default();
        if !user.groups.iter().any(|g| g == group) {
            user.groups.push(group.to_owned());
        }
        self.rebuild()
    }

    /// Remove a player from a group. Returns whether they were a member.
    pub fn remove_user_group(
        &mut self,
        player: u128,
        group: &str,
    ) -> Result<bool, PermissionsError> {
        let Some(user) = self.file.users.get_mut(&format_uuid(player)) else {
            return Ok(false);
        };
        let before = user.groups.len();
        user.groups.retain(|g| g != group);
        let removed = user.groups.len() != before;
        self.rebuild()?;
        Ok(removed)
    }

    /// Set (`Some`) or unset (`None`) a node on a player.
    pub fn set_user_node(
        &mut self,
        player: u128,
        node: &str,
        value: Option<bool>,
    ) -> Result<(), PermissionsError> {
        let node = node.to_ascii_lowercase();
        validate_node(&node).map_err(PermissionsError::Invalid)?;
        let user = self.file.users.entry(format_uuid(player)).or_default();
        match value {
            Some(v) => {
                user.nodes.insert(node, v);
            }
            None => {
                user.nodes.remove(&node);
            }
        }
        self.rebuild()
    }

    /// Set the informational name of a player.
    pub fn set_user_name(&mut self, player: u128, name: &str) {
        self.file.users.entry(format_uuid(player)).or_default().name = Some(name.to_owned());
    }

    /// Set (`Some`) or unset (`None`) a node on a group (which must exist).
    pub fn set_group_node(
        &mut self,
        group: &str,
        node: &str,
        value: Option<bool>,
    ) -> Result<(), PermissionsError> {
        let node = node.to_ascii_lowercase();
        validate_node(&node).map_err(PermissionsError::Invalid)?;
        let g = self
            .file
            .groups
            .get_mut(group)
            .ok_or_else(|| PermissionsError::Invalid(format!("no group `{group}`")))?;
        match value {
            Some(v) => {
                g.nodes.insert(node, v);
            }
            None => {
                g.nodes.remove(&node);
            }
        }
        self.rebuild()
    }

    /// Create or replace a group. Refused (and nothing changes) if it would create a cycle or
    /// reference unknown groups.
    pub fn put_group(&mut self, name: &str, def: GroupDef) -> Result<(), PermissionsError> {
        let mut file = self.file.clone();
        file.groups.insert(name.to_owned(), def);
        let file = normalize(file)?;
        validate(&file)?;
        self.file = file;
        self.rebuild()
    }

    /// Delete a group (not the default one); members lose it.
    pub fn delete_group(&mut self, name: &str) -> Result<bool, PermissionsError> {
        if name == self.file.default_group {
            return Err(PermissionsError::Invalid(
                "the default group cannot be deleted".into(),
            ));
        }
        if self.file.groups.remove(name).is_none() {
            return Ok(false);
        }
        for g in self.file.groups.values_mut() {
            g.inherits.retain(|p| p != name);
        }
        for u in self.file.users.values_mut() {
            u.groups.retain(|p| p != name);
        }
        self.rebuild()?;
        Ok(true)
    }

    /// Groups reachable from `start` with their minimal depth (BFS), in a deterministic order.
    fn expand(&self, start: &[String]) -> Vec<(String, u32)> {
        let mut seen: HashMap<String, u32> = HashMap::new();
        let mut order = Vec::new();
        let mut queue: VecDeque<(String, u32)> = start.iter().map(|g| (g.clone(), 0)).collect();
        while let Some((g, d)) = queue.pop_front() {
            if seen.contains_key(&g) {
                continue;
            }
            seen.insert(g.clone(), d);
            order.push((g.clone(), d));
            if let Some(def) = self.file.groups.get(&g) {
                for p in &def.inherits {
                    queue.push_back((p.clone(), d + 1));
                }
            }
        }
        order
    }

    fn flatten(&self, user: Option<&UserDef>, groups: &[String]) -> Flat {
        let mut best: HashMap<String, (Rank, String, bool)> = HashMap::new();
        let mut consider = |node: &str, value: bool, rank: Rank, source: &str| match best.get(node)
        {
            Some((r, s, _)) if !(rank.beats(r) || (rank == *r && source < s.as_str())) => {}
            _ => {
                best.insert(node.to_owned(), (rank, source.to_owned(), value));
            }
        };
        if let Some(u) = user {
            for (node, v) in &u.nodes {
                consider(
                    node,
                    *v,
                    Rank {
                        user: true,
                        weight: 0,
                        depth: 0,
                    },
                    "",
                );
            }
        }
        for (name, depth) in self.expand(groups) {
            if let Some(g) = self.file.groups.get(&name) {
                for (node, v) in &g.nodes {
                    consider(
                        node,
                        *v,
                        Rank {
                            user: false,
                            weight: g.weight,
                            depth,
                        },
                        &name,
                    );
                }
            }
        }
        best.into_iter().map(|(k, (_, _, v))| (k, v)).collect()
    }

    fn rebuild(&mut self) -> Result<(), PermissionsError> {
        let default = vec![self.file.default_group.clone()];
        self.default_flat = self.flatten(None, &default);
        let mut users = HashMap::new();
        for (key, def) in &self.file.users {
            let id = parse_uuid(key)
                .ok_or_else(|| PermissionsError::Invalid(format!("bad user UUID `{key}`")))?;
            let mut groups = def.groups.clone();
            if !groups.contains(&self.file.default_group) {
                groups.push(self.file.default_group.clone());
            }
            users.insert(id, self.flatten(Some(def), &groups));
        }
        self.users = users;
        Ok(())
    }
}

/// Lowercase nodes and canonicalise user UUID keys.
fn normalize(mut file: PermissionsFile) -> Result<PermissionsFile, PermissionsError> {
    for g in file.groups.values_mut() {
        g.nodes = std::mem::take(&mut g.nodes)
            .into_iter()
            .map(|(k, v)| (k.to_ascii_lowercase(), v))
            .collect();
    }
    let mut users = BTreeMap::new();
    for (k, mut u) in std::mem::take(&mut file.users) {
        let id = parse_uuid(&k)
            .ok_or_else(|| PermissionsError::Invalid(format!("bad user UUID `{k}`")))?;
        u.nodes = std::mem::take(&mut u.nodes)
            .into_iter()
            .map(|(k, v)| (k.to_ascii_lowercase(), v))
            .collect();
        if users.insert(format_uuid(id), u).is_some() {
            return Err(PermissionsError::Invalid(format!(
                "user {k} is listed twice"
            )));
        }
    }
    file.users = users;
    Ok(file)
}

fn validate(file: &PermissionsFile) -> Result<(), PermissionsError> {
    if !file.groups.contains_key(&file.default_group) {
        return Err(PermissionsError::Invalid(format!(
            "default group `{}` is not defined",
            file.default_group
        )));
    }
    for (name, g) in &file.groups {
        if name.is_empty()
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return Err(PermissionsError::Invalid(format!(
                "group name `{name}` must be [A-Za-z0-9_-]"
            )));
        }
        for p in &g.inherits {
            if !file.groups.contains_key(p) {
                return Err(PermissionsError::Invalid(format!(
                    "group `{name}` inherits unknown group `{p}`"
                )));
            }
        }
        for node in g.nodes.keys() {
            validate_node(node)
                .map_err(|e| PermissionsError::Invalid(format!("group `{name}`: {e}")))?;
        }
    }
    for (key, u) in &file.users {
        for gname in &u.groups {
            if !file.groups.contains_key(gname) {
                return Err(PermissionsError::Invalid(format!(
                    "user {key} is in unknown group `{gname}`"
                )));
            }
        }
        for node in u.nodes.keys() {
            validate_node(node)
                .map_err(|e| PermissionsError::Invalid(format!("user {key}: {e}")))?;
        }
    }
    // Cycle detection: DFS with colours, reporting the cycle path.
    let mut state: HashMap<&str, u8> = HashMap::new(); // 1 = in progress, 2 = done
    fn visit<'a>(
        file: &'a PermissionsFile,
        g: &'a str,
        state: &mut HashMap<&'a str, u8>,
        stack: &mut Vec<&'a str>,
    ) -> Result<(), Vec<String>> {
        match state.get(g) {
            Some(2) => return Ok(()),
            Some(1) => {
                let start = stack.iter().position(|s| *s == g).unwrap_or(0);
                let mut cycle: Vec<String> = stack[start..].iter().map(|s| s.to_string()).collect();
                cycle.push(g.to_owned());
                return Err(cycle);
            }
            _ => {}
        }
        state.insert(g, 1);
        stack.push(g);
        if let Some(def) = file.groups.get(g) {
            for p in &def.inherits {
                visit(file, p, state, stack)?;
            }
        }
        stack.pop();
        state.insert(g, 2);
        Ok(())
    }
    for name in file.groups.keys() {
        let mut stack = Vec::new();
        visit(file, name, &mut state, &mut stack).map_err(PermissionsError::Cycle)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOTCH: &str = "069a79f4-44e9-4726-a5be-fca90e38aaf5";

    fn notch() -> u128 {
        parse_uuid(NOTCH).unwrap()
    }

    const SAMPLE: &str = r#"
default_group = "default"

[groups.default]
weight = 0
prefix = "[Member] "
nodes = { "homes.home" = true, "chat.color" = false }

[groups.mod]
weight = 50
inherits = ["default"]
prefix = "[Mod] "
nodes = { "homes.*" = true, "chat.color" = true }

[groups.admin]
weight = 100
inherits = ["mod"]
prefix = "[Admin] "
nodes = { "*" = true, "homes.debug" = false }

[groups.muted]
weight = 200
nodes = { "chat.color" = false }

[users."069a79f4-44e9-4726-a5be-fca90e38aaf5"]
name = "Notch"
groups = ["admin"]
nodes = { "essentials.fly" = false, "Homes.Debug" = true }
"#;

    #[test]
    fn candidates() {
        assert_eq!(candidate_keys("a.b.c"), vec!["a.b.c", "a.b.*", "a.*", "*"]);
        assert_eq!(candidate_keys("a"), vec!["a", "*"]);
        assert_eq!(candidate_keys("a.*"), vec!["a.*", "*"]);
        assert_eq!(candidate_keys("*"), vec!["*"]);
    }

    #[test]
    fn node_validation() {
        assert!(validate_node("homes.home").is_ok());
        assert!(validate_node("homes.*").is_ok());
        assert!(validate_node("*").is_ok());
        assert!(validate_node("homes.*.x").is_err());
        assert!(validate_node("Homes").is_err());
        assert!(validate_node("a..b").is_err());
        assert!(validate_node("").is_err());
    }

    #[test]
    fn uuids() {
        let v = notch();
        assert_eq!(format_uuid(v), NOTCH);
        assert_eq!(parse_uuid(&NOTCH.replace('-', "")), Some(v));
        assert_eq!(parse_uuid("069a79f4-44e94726-a5be-fca90e38aaf5"), None);
        assert_eq!(parse_uuid("xyz"), None);
    }

    #[test]
    fn resolution_rules() {
        let e = PermissionEngine::parse(SAMPLE).unwrap();
        let stranger = 42u128;
        // Unknown player: default group only.
        assert_eq!(e.check(stranger, "homes.home"), Tristate::Allow);
        assert_eq!(e.check(stranger, "homes.set"), Tristate::Undefined);
        assert_eq!(e.check(stranger, "chat.color"), Tristate::Deny);
        // Notch: admin (inherits mod, default) + user nodes.
        let n = notch();
        assert_eq!(e.check(n, "anything.at.all"), Tristate::Allow, "admin *");
        assert_eq!(
            e.check(n, "essentials.fly"),
            Tristate::Deny,
            "user negation beats group *"
        );
        assert_eq!(
            e.check(n, "homes.debug"),
            Tristate::Allow,
            "user beats group (and is lowercased)"
        );
        assert_eq!(
            e.check(n, "homes.set"),
            Tristate::Allow,
            "mod homes.* beats admin *"
        );
        // Exact node from lower-weight group beats wildcard from higher one.
        assert_eq!(e.check(n, "homes.home"), Tristate::Allow);
        // Same key in several groups: higher weight wins (mod true over default false).
        assert_eq!(e.check(n, "chat.color"), Tristate::Allow);
        // Case-insensitive check.
        assert_eq!(e.check(n, "ESSENTIALS.FLY"), Tristate::Deny);
    }

    #[test]
    fn weight_beats_depth_and_membership_changes() {
        let mut e = PermissionEngine::parse(SAMPLE).unwrap();
        let n = notch();
        e.add_user_group(n, "muted").unwrap();
        assert_eq!(
            e.check(n, "chat.color"),
            Tristate::Deny,
            "muted has weight 200"
        );
        assert!(e.remove_user_group(n, "muted").unwrap());
        assert_eq!(e.check(n, "chat.color"), Tristate::Allow);
        assert!(e.add_user_group(n, "nope").is_err());
        e.set_user_node(n, "x.y", Some(false)).unwrap();
        assert_eq!(e.check(n, "x.y"), Tristate::Deny);
        e.set_user_node(n, "x.y", None).unwrap();
        assert_eq!(e.check(n, "x.y"), Tristate::Allow);
        e.set_group_node("default", "z.z", Some(true)).unwrap();
        assert_eq!(e.check(7, "z.z"), Tristate::Allow);
    }

    #[test]
    fn groups_and_meta() {
        let e = PermissionEngine::parse(SAMPLE).unwrap();
        let n = notch();
        assert_eq!(e.groups_of(n), vec!["admin", "default"]);
        assert_eq!(e.primary_group(n).as_deref(), Some("admin"));
        assert_eq!(e.prefix(n).as_deref(), Some("[Admin] "));
        assert_eq!(e.prefix(5).as_deref(), Some("[Member] "));
        assert_eq!(e.suffix(n), None);
        assert_eq!(e.group_prefix("mod").as_deref(), Some("[Mod] "));
    }

    #[test]
    fn inheritance_cycles_are_refused() {
        let text = r#"
[groups.default]
[groups.a]
inherits = ["b"]
[groups.b]
inherits = ["c"]
[groups.c]
inherits = ["a"]
"#;
        match PermissionEngine::parse(text) {
            Err(PermissionsError::Cycle(c)) => {
                assert_eq!(c.first(), c.last());
                assert!(c.contains(&"a".to_owned()) && c.contains(&"b".to_owned()));
            }
            other => panic!("expected cycle, got {other:?}"),
        }
        let self_loop = "[groups.default]\ninherits = [\"default\"]\n";
        assert!(matches!(
            PermissionEngine::parse(self_loop),
            Err(PermissionsError::Cycle(_))
        ));
        // put_group refuses a cycle and leaves the engine unchanged.
        let mut e = PermissionEngine::parse(SAMPLE).unwrap();
        let def = GroupDef {
            inherits: vec!["admin".into()],
            ..GroupDef::default()
        };
        assert!(e.put_group("default", def).is_err());
        assert!(e.file().groups["default"].inherits.is_empty());
    }

    #[test]
    fn invalid_files() {
        assert!(PermissionEngine::parse("default_group = \"x\"").is_err());
        assert!(PermissionEngine::parse("[groups.default]\ninherits = [\"ghost\"]").is_err());
        assert!(PermissionEngine::parse("[groups.default]\n[users.bad]\n").is_err());
        assert!(PermissionEngine::parse("[groups.default]\nnodes = { \"a..b\" = true }").is_err());
        assert!(PermissionEngine::parse("[groups.default]\nunknown = 1").is_err());
    }

    #[test]
    fn save_and_reload_atomically() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("permissions.toml");
        let mut e = PermissionEngine::load_or_create(&path).unwrap();
        assert!(path.exists());
        e.put_group(
            "vip",
            GroupDef {
                weight: 10,
                nodes: [("fly".to_owned(), true)].into_iter().collect(),
                ..GroupDef::default()
            },
        )
        .unwrap();
        e.add_user_group(notch(), "vip").unwrap();
        e.save().unwrap();
        assert!(!dir.path().join("permissions.toml.tmp").exists());
        let back = PermissionEngine::load(&path).unwrap();
        assert_eq!(back.check(notch(), "fly"), Tristate::Allow);
        assert_eq!(back.check(1, "fly"), Tristate::Undefined);
        assert!(back.delete_group_clone_ok());
    }

    impl PermissionEngine {
        fn delete_group_clone_ok(&self) -> bool {
            let mut c = self.clone();
            c.delete_group("vip").unwrap() && c.check(notch(), "fly") == Tristate::Undefined
        }
    }
}
