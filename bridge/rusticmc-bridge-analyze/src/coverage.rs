//! The coverage table: which Bukkit/Paper API classes and members Rustic can serve, and how.
//!
//! The built-in table is `data/coverage.toml`, built by hand from the WIT contract in
//! `crates/rusticmc-api-wit/wit/`. Mapping values are free text naming the Rustic equivalent
//! (`players.send-message`); their prefix gives the status:
//!
//! - `uncovered...`: explicitly not covered (a known gap, the text says why);
//! - `partial: ...`: covered with a semantic gap (counts as covered, reported separately);
//! - anything else: covered (`iface.function`, `events.<name>`, `java-side: ...` for pure-Java
//!   API the bridge implements without any host call).
//!
//! Lookup rules: a member is looked up on its owner class (`name(descriptor)` key, then
//! `name`, then `"*"`), then on the classes listed in `inherits` (breadth first). A member of a
//! class absent from the table is uncovered, even if a supertype maps a member of that name:
//! `PlayerKickEvent#getPlayer` is not covered just because `PlayerEvent#getPlayer` is for the
//! events Rustic has. `packages` entries cover every class under a package prefix.

use std::collections::{BTreeMap, HashSet, VecDeque};

use serde::{Deserialize, Serialize};

use crate::classfile::MemberKind;

/// The built-in table.
pub const BUILTIN_COVERAGE: &str = include_str!("../data/coverage.toml");

/// Coverage status of a class or member.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", content = "mapping", rename_all = "kebab-case")]
pub enum Status {
    Covered(String),
    Partial(String),
    Uncovered(Option<String>),
}

impl Status {
    fn from_value(v: &str) -> Self {
        let t = v.trim();
        if t.starts_with("uncovered") {
            Status::Uncovered(Some(t.to_owned()))
        } else if let Some(rest) = t.strip_prefix("partial:") {
            Status::Partial(rest.trim().to_owned())
        } else {
            Status::Covered(t.to_owned())
        }
    }
    /// Covered or partially covered.
    pub fn is_covered(&self) -> bool {
        !matches!(self, Status::Uncovered(_))
    }
    pub fn is_partial(&self) -> bool {
        matches!(self, Status::Partial(_))
    }
    /// The mapping text, if any.
    pub fn mapping(&self) -> Option<&str> {
        match self {
            Status::Covered(s) | Status::Partial(s) => Some(s),
            Status::Uncovered(s) => s.as_deref(),
        }
    }
}

/// `[meta]` of the table.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Meta {
    #[serde(default)]
    pub api: String,
    #[serde(default)]
    pub note: String,
}

/// One class entry.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ClassCoverage {
    /// How the type itself maps (absent: covered through its members).
    #[serde(default, rename = "type")]
    pub type_: Option<String>,
    /// Supertypes (or delegates, for `Bukkit` → `Server`) whose member mappings apply.
    #[serde(default)]
    pub inherits: Vec<String>,
    #[serde(default)]
    pub methods: BTreeMap<String, String>,
    #[serde(default)]
    pub fields: BTreeMap<String, String>,
}

/// The whole table. Class names use Java's binary form with dots (`org.bukkit.entity.Player`,
/// nested `org.bukkit.entity.Player$Spigot`).
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Coverage {
    #[serde(default)]
    pub meta: Meta,
    #[serde(default)]
    pub packages: BTreeMap<String, String>,
    #[serde(default)]
    pub classes: BTreeMap<String, ClassCoverage>,
}

/// Methods every class has (from `java.lang.Object` / `java.lang.Enum`): covered wherever the
/// class is.
const UNIVERSAL: &[&str] = &[
    "equals",
    "hashCode",
    "toString",
    "getClass",
    "name",
    "ordinal",
    "compareTo",
    "values",
    "valueOf",
    "getDeclaringClass",
];

impl Coverage {
    /// The table shipped with the analyzer.
    pub fn builtin() -> Self {
        Self::from_toml_str(BUILTIN_COVERAGE).expect("data/coverage.toml is valid (tested)")
    }

    pub fn from_toml_str(s: &str) -> Result<Self, String> {
        toml::from_str(s).map_err(|e| e.to_string())
    }

    fn package_status(&self, class: &str) -> Option<Status> {
        self.packages
            .iter()
            .filter(|(p, _)| class.starts_with(p.as_str()) && class[p.len()..].starts_with('.'))
            .max_by_key(|(p, _)| p.len())
            .map(|(_, v)| Status::from_value(v))
    }

    /// Whether the type itself is available (dotted class name).
    pub fn class_status(&self, class: &str) -> Status {
        if let Some(c) = self.classes.get(class) {
            return match &c.type_ {
                Some(t) => Status::from_value(t),
                None => Status::Covered("see members".into()),
            };
        }
        self.package_status(class)
            .unwrap_or(Status::Uncovered(None))
    }

    /// Whether a member is available (dotted owner class name).
    pub fn member_status(
        &self,
        kind: MemberKind,
        owner: &str,
        name: &str,
        descriptor: &str,
    ) -> Status {
        if !self.classes.contains_key(owner) {
            return self
                .package_status(owner)
                .unwrap_or(Status::Uncovered(None));
        }
        if !self.class_status(owner).is_covered() {
            return Status::Uncovered(None);
        }
        let mut queue = VecDeque::from([owner.to_owned()]);
        let mut seen = HashSet::new();
        while let Some(cls) = queue.pop_front() {
            if !seen.insert(cls.clone()) {
                continue;
            }
            match self.classes.get(&cls) {
                Some(c) => {
                    let table = match kind {
                        MemberKind::Method => &c.methods,
                        MemberKind::Field => &c.fields,
                    };
                    let keyed = format!("{name}{descriptor}");
                    if let Some(v) = table
                        .get(&keyed)
                        .or_else(|| table.get(name))
                        .or_else(|| table.get("*"))
                    {
                        return Status::from_value(v);
                    }
                    queue.extend(c.inherits.iter().cloned());
                }
                None => {
                    if let Some(s) = self.package_status(&cls) {
                        return s;
                    }
                }
            }
        }
        if kind == MemberKind::Method && name == "<init>" {
            // Constructing (or subclassing) a type the table covers.
            return self.class_status(owner);
        }
        if kind == MemberKind::Method && UNIVERSAL.contains(&name) {
            return Status::Covered("java-side: java.lang.Object/Enum".into());
        }
        Status::Uncovered(None)
    }

    /// Every mapping string in the table (for validation against the WIT).
    pub fn all_mappings(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for (p, v) in &self.packages {
            out.push((format!("package {p}"), v.clone()));
        }
        for (c, cc) in &self.classes {
            if let Some(t) = &cc.type_ {
                out.push((c.clone(), t.clone()));
            }
            for (m, v) in cc.methods.iter().chain(cc.fields.iter()) {
                out.push((format!("{c}#{m}"), v.clone()));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    const T: &str = r#"
[packages]
"org.bukkit.configuration" = "java-side: config"

[classes."a.Base"]
methods = { getPlayer = "players.online" }

[classes."a.Join"]
inherits = ["a.Base", "a.Cancellable"]
methods = { getJoinMessage = "events.player-join", "kick(Ljava/lang/String;)V" = "uncovered: no" }

[classes."a.Cancellable"]
methods = { setCancelled = "partial: some" }

[classes."a.Gone"]
type = "uncovered: not in WIT"
methods = { x = "y.z" }
"#;

    #[test]
    fn lookup_rules() {
        let c = Coverage::from_toml_str(T).unwrap();
        let m = |o: &str, n: &str, d: &str| c.member_status(MemberKind::Method, o, n, d);
        assert_eq!(
            m("a.Join", "getPlayer", "()V"),
            Status::Covered("players.online".into())
        );
        assert!(m("a.Join", "setCancelled", "(Z)V").is_partial());
        assert!(!m("a.Join", "kick", "(Ljava/lang/String;)V").is_covered());
        assert!(
            !m("a.Kick", "getPlayer", "()V").is_covered(),
            "absent class is uncovered"
        );
        assert!(
            !m("a.Gone", "x", "()V").is_covered(),
            "uncovered type hides its members"
        );
        assert!(m("a.Join", "hashCode", "()I").is_covered());
        assert!(
            m(
                "org.bukkit.configuration.file.YamlConfiguration",
                "getString",
                ""
            )
            .is_covered()
        );
        assert!(!m("org.bukkit.configurationx.Y", "a", "").is_covered());
        assert!(c.class_status("a.Join").is_covered());
        assert!(!c.class_status("a.Gone").is_covered());
    }

    /// `(interface, function)` pairs of the WIT contract, read with a small line scanner.
    fn wit_functions() -> BTreeSet<(String, String)> {
        let dir = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../crates/rusticmc-api-wit/wit"
        );
        let mut files = vec![];
        for d in [dir.to_owned(), format!("{dir}/deps/services")] {
            for e in std::fs::read_dir(d).expect("WIT dir") {
                let p = e.unwrap().path();
                if p.extension().is_some_and(|x| x == "wit") {
                    files.push(p);
                }
            }
        }
        let mut out = BTreeSet::new();
        for f in files {
            let text = std::fs::read_to_string(f).unwrap();
            let mut iface = String::new();
            for line in text.lines() {
                let l = line.trim();
                if let Some(rest) = l.strip_prefix("interface ") {
                    iface = rest.trim_end_matches('{').trim().to_owned();
                } else if let Some((name, _)) = l.split_once(": func(") {
                    out.insert((
                        iface.clone(),
                        name.trim().trim_start_matches('%').to_owned(),
                    ));
                } else if let Some(rest) = l.strip_prefix("record ") {
                    // Event records: `events.<record>` names an event handler.
                    let r = rest.trim_end_matches('{').trim().to_owned();
                    out.insert(("events".into(), r.clone()));
                    out.insert((iface.clone(), r));
                } else if let Some(rest) = l
                    .strip_prefix("enum ")
                    .or_else(|| l.strip_prefix("variant "))
                    .or_else(|| l.strip_prefix("type "))
                {
                    let name = rest
                        .split(|c: char| c == '{' || c == '=' || c.is_whitespace())
                        .next()
                        .unwrap_or("");
                    out.insert((iface.clone(), name.to_owned()));
                }
            }
        }
        out
    }

    /// Every `iface.function` token in the table names a real WIT function (or event record), so
    /// the table cannot claim coverage the contract does not have.
    #[test]
    fn builtin_table_parses_and_matches_the_wit() {
        let cov = Coverage::builtin();
        assert!(cov.classes.len() > 50);
        let wit = wit_functions();
        let ifaces: BTreeSet<&str> = wit.iter().map(|(i, _)| i.as_str()).collect();
        let mut bad = vec![];
        for (at, v) in cov.all_mappings() {
            for tok in v.split(|c: char| !(c.is_ascii_alphanumeric() || c == '-' || c == '.')) {
                let Some((i, f)) = tok.split_once('.') else {
                    continue;
                };
                if !ifaces.contains(i) || f.contains('.') {
                    continue; // not a WIT reference (java names, prose)
                }
                if !wit.contains(&(i.to_owned(), f.to_owned())) {
                    bad.push(format!("{at}: {tok}"));
                }
            }
        }
        assert!(
            bad.is_empty(),
            "coverage.toml names functions missing from the WIT:\n{}",
            bad.join("\n")
        );
    }
}
