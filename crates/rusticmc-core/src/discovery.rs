//! Discovery (scanning `plugins/`) and dependency resolution (load order).

use crate::archive::{ARCHIVE_EXTENSION, PluginPackage};
use crate::id::PluginId;
use crate::manifest::{MANIFEST_FILE, Manifest};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt;
use std::path::{Path, PathBuf};

/// A package that could not be used.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveryError {
    /// The file or directory.
    pub path: PathBuf,
    /// The plugin id, when the manifest could be read.
    pub id: Option<PluginId>,
    /// Why.
    pub message: String,
}

impl fmt::Display for DiscoveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

/// Result of scanning a directory.
#[derive(Debug, Default)]
pub struct DiscoveryReport {
    /// Valid packages with unique ids, sorted by id.
    pub packages: Vec<PluginPackage>,
    /// Refused packages.
    pub errors: Vec<DiscoveryError>,
}

/// Scan `dir` for `.rplugin` files and unpacked plugin directories (directories containing
/// `rustic.plugin.toml`). Other files and directories (plugin data folders) are ignored.
/// Packages sharing an id are all refused (which one to load would be ambiguous).
pub fn discover(dir: &Path) -> std::io::Result<DiscoveryReport> {
    let mut report = DiscoveryReport::default();
    if !dir.exists() {
        return Ok(report);
    }
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();
    entries.sort();
    let mut found: Vec<PluginPackage> = Vec::new();
    for path in entries {
        let candidate = if path.is_dir() {
            path.join(MANIFEST_FILE).is_file()
        } else {
            path.extension().and_then(|e| e.to_str()) == Some(ARCHIVE_EXTENSION)
        };
        if !candidate {
            continue;
        }
        match PluginPackage::open(&path) {
            Ok(pkg) => found.push(pkg),
            Err(e) => report.errors.push(DiscoveryError {
                path,
                id: None,
                message: e.to_string(),
            }),
        }
    }
    let mut by_id: BTreeMap<PluginId, Vec<PluginPackage>> = BTreeMap::new();
    for pkg in found {
        by_id.entry(pkg.manifest.id.clone()).or_default().push(pkg);
    }
    for (id, mut pkgs) in by_id {
        if pkgs.len() == 1 {
            report.packages.extend(pkgs.pop());
        } else {
            let paths: Vec<String> = pkgs
                .iter()
                .map(|p| p.path().display().to_string())
                .collect();
            for p in pkgs {
                report.errors.push(DiscoveryError {
                    path: p.path().to_owned(),
                    id: Some(id.clone()),
                    message: format!(
                        "duplicate plugin id `{id}` in {}: none of them is loaded, remove all but one",
                        paths.join(" and ")
                    ),
                });
            }
        }
    }
    Ok(report)
}

/// Why a plugin cannot be loaded.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ResolveError {
    /// A required dependency is not installed.
    #[error("missing required dependency `{dependency}` ({requirement})")]
    MissingDependency {
        /// The dependency.
        dependency: PluginId,
        /// The version requirement.
        requirement: String,
    },
    /// A required dependency has a version outside the requirement.
    #[error(
        "required dependency `{dependency}` is version {found}, which does not match {requirement}"
    )]
    VersionMismatch {
        /// The dependency.
        dependency: PluginId,
        /// The requirement.
        requirement: String,
        /// The installed version.
        found: String,
    },
    /// A required dependency is itself refused.
    #[error("required dependency `{dependency}` cannot be loaded: {reason}")]
    DependencyRefused {
        /// The dependency.
        dependency: PluginId,
        /// Its own error.
        reason: String,
    },
    /// Required dependencies form a cycle; every member is refused.
    #[error("required dependency cycle between {}", .0.iter().map(|p| p.as_str()).collect::<Vec<_>>().join(", "))]
    Cycle(Vec<PluginId>),
    /// A required dependency is installed but not enabled (single-plugin enable).
    #[error("required dependency `{0}` is not enabled")]
    DependencyNotEnabled(PluginId),
}

/// A load order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Resolution {
    /// Plugins to load, dependencies first.
    pub order: Vec<PluginId>,
    /// Plugins refused, with the reason.
    pub refused: BTreeMap<PluginId, ResolveError>,
    /// Non-fatal notes (optional dependency version mismatch, soft cycle broken...).
    pub warnings: Vec<String>,
}

/// Whether `version` satisfies `req`; `*` accepts every version, pre-releases included.
pub fn version_matches(req: &semver::VersionReq, version: &semver::Version) -> bool {
    *req == semver::VersionReq::STAR || req.matches(version)
}

/// Strongly connected components (Tarjan), nodes indexed `0..n`.
fn sccs(n: usize, edges: &[Vec<usize>]) -> Vec<Vec<usize>> {
    struct T<'a> {
        edges: &'a [Vec<usize>],
        index: Vec<Option<usize>>,
        low: Vec<usize>,
        on: Vec<bool>,
        stack: Vec<usize>,
        next: usize,
        out: Vec<Vec<usize>>,
    }
    fn strong(t: &mut T<'_>, v: usize) {
        t.index[v] = Some(t.next);
        t.low[v] = t.next;
        t.next += 1;
        t.stack.push(v);
        t.on[v] = true;
        for i in 0..t.edges[v].len() {
            let w = t.edges[v][i];
            match t.index[w] {
                None => {
                    strong(t, w);
                    t.low[v] = t.low[v].min(t.low[w]);
                }
                Some(iw) if t.on[w] => t.low[v] = t.low[v].min(iw),
                _ => {}
            }
        }
        if Some(t.low[v]) == t.index[v] {
            let mut comp = Vec::new();
            while let Some(w) = t.stack.pop() {
                t.on[w] = false;
                comp.push(w);
                if w == v {
                    break;
                }
            }
            t.out.push(comp);
        }
    }
    let mut t = T {
        edges,
        index: vec![None; n],
        low: vec![0; n],
        on: vec![false; n],
        stack: Vec::new(),
        next: 0,
        out: Vec::new(),
    };
    for v in 0..n {
        if t.index[v].is_none() {
            strong(&mut t, v);
        }
    }
    t.out
}

/// Compute a load order for `manifests` (ids must be unique).
///
/// * A plugin whose required dependency is missing, has a non-matching version or is refused
///   itself is refused (transitively).
/// * Plugins whose *required* dependencies form a cycle are all refused, named in the error.
/// * Optional dependencies and `load_before` only order plugins that are present; a cycle made
///   of such soft edges is broken (with a warning) rather than refused.
/// * Ties are broken by id, so the order is deterministic.
pub fn resolve(manifests: &[&Manifest]) -> Resolution {
    let mut res = Resolution::default();
    let by_id: BTreeMap<&PluginId, &Manifest> = manifests.iter().map(|m| (&m.id, *m)).collect();

    // 1. Missing / mismatching required dependencies, to a fixpoint.
    let propagate = |refused: &mut BTreeMap<PluginId, ResolveError>| loop {
        let mut changed = false;
        for (id, m) in &by_id {
            if refused.contains_key(*id) {
                continue;
            }
            for (dep, req) in &m.dependencies.required {
                let err = match by_id.get(dep) {
                    None => Some(ResolveError::MissingDependency {
                        dependency: dep.clone(),
                        requirement: req.to_string(),
                    }),
                    Some(dm) if !version_matches(req, &dm.version) => {
                        Some(ResolveError::VersionMismatch {
                            dependency: dep.clone(),
                            requirement: req.to_string(),
                            found: dm.version.to_string(),
                        })
                    }
                    Some(_) => refused.get(dep).map(|e| ResolveError::DependencyRefused {
                        dependency: dep.clone(),
                        reason: e.to_string(),
                    }),
                };
                if let Some(err) = err {
                    refused.insert((*id).clone(), err);
                    changed = true;
                    break;
                }
            }
        }
        if !changed {
            break;
        }
    };
    propagate(&mut res.refused);

    // 2. Cycles of required dependencies.
    let alive: Vec<&PluginId> = by_id
        .keys()
        .copied()
        .filter(|id| !res.refused.contains_key(*id))
        .collect();
    let idx: HashMap<&PluginId, usize> = alive.iter().enumerate().map(|(i, id)| (*id, i)).collect();
    let mut req_edges = vec![Vec::new(); alive.len()];
    for (i, id) in alive.iter().enumerate() {
        for dep in by_id[id].dependencies.required.keys() {
            if let Some(&j) = idx.get(dep) {
                req_edges[j].push(i); // dep before dependent
            }
        }
    }
    for comp in sccs(alive.len(), &req_edges) {
        if comp.len() > 1 {
            let mut members: Vec<PluginId> = comp.iter().map(|&i| alive[i].clone()).collect();
            members.sort();
            for m in &members {
                res.refused
                    .insert(m.clone(), ResolveError::Cycle(members.clone()));
            }
        }
    }
    propagate(&mut res.refused);

    // 3. Full graph with soft edges; soft edges inside a cycle are dropped.
    let alive: Vec<&PluginId> = by_id
        .keys()
        .copied()
        .filter(|id| !res.refused.contains_key(*id))
        .collect();
    let idx: HashMap<&PluginId, usize> = alive.iter().enumerate().map(|(i, id)| (*id, i)).collect();
    let mut edges: Vec<Vec<(usize, bool)>> = vec![Vec::new(); alive.len()]; // (to, soft)
    for (i, id) in alive.iter().enumerate() {
        let m = by_id[id];
        for dep in m.dependencies.required.keys() {
            if let Some(&j) = idx.get(dep) {
                edges[j].push((i, false));
            }
        }
        for (dep, req) in &m.dependencies.optional {
            match (idx.get(dep), by_id.get(dep)) {
                (Some(&j), Some(dm)) if version_matches(req, &dm.version) => edges[j].push((i, true)),
                (Some(_), Some(dm)) => res.warnings.push(format!(
                    "{id}: optional dependency `{dep}` is version {}, which does not match {req}; ignored",
                    dm.version
                )),
                _ => {}
            }
        }
        for target in &m.dependencies.load_before {
            if let Some(&j) = idx.get(target) {
                edges[i].push((j, true));
            }
        }
    }
    let plain: Vec<Vec<usize>> = edges
        .iter()
        .map(|e| e.iter().map(|(t, _)| *t).collect())
        .collect();
    let mut comp_of = vec![0usize; alive.len()];
    for (c, comp) in sccs(alive.len(), &plain).into_iter().enumerate() {
        if comp.len() > 1 {
            let mut names: Vec<&str> = comp.iter().map(|&i| alive[i].as_str()).collect();
            names.sort();
            res.warnings.push(format!(
                "optional dependencies / load_before form a cycle between {}; that ordering is ignored",
                names.join(", ")
            ));
        }
        for i in comp {
            comp_of[i] = c;
        }
    }
    let mut indeg = vec![0usize; alive.len()];
    let mut out: Vec<Vec<usize>> = vec![Vec::new(); alive.len()];
    for (from, list) in edges.iter().enumerate() {
        for &(to, soft) in list {
            if soft && comp_of[from] == comp_of[to] {
                continue;
            }
            if !out[from].contains(&to) {
                out[from].push(to);
                indeg[to] += 1;
            }
        }
    }
    let mut ready: BTreeSet<(&PluginId, usize)> = (0..alive.len())
        .filter(|&i| indeg[i] == 0)
        .map(|i| (alive[i], i))
        .collect();
    while let Some(first) = ready.iter().next().copied() {
        ready.remove(&first);
        let (_, i) = first;
        res.order.push(alive[i].clone());
        for &to in &out[i] {
            indeg[to] -= 1;
            if indeg[to] == 0 {
                ready.insert((alive[to], to));
            }
        }
    }
    // Cannot happen (required cycles were refused, soft ones broken), but never lose a plugin
    // silently.
    if res.order.len() != alive.len() {
        for id in alive {
            if !res.order.contains(id) {
                res.refused
                    .insert(id.clone(), ResolveError::Cycle(vec![id.clone()]));
            }
        }
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::archive::tests::manifest_text;

    fn m(id: &str, version: &str, deps: &str) -> Manifest {
        Manifest::parse(&manifest_text(id, version, deps)).unwrap()
    }

    fn ids(order: &[PluginId]) -> Vec<&str> {
        order.iter().map(|p| p.as_str()).collect()
    }

    fn run(list: &[Manifest]) -> Resolution {
        let refs: Vec<&Manifest> = list.iter().collect();
        resolve(&refs)
    }

    #[test]
    fn simple_order_and_ties() {
        let r = run(&[
            m("c", "1.0.0", "[dependencies]\nrequired = { b = \"^1\" }"),
            m("b", "1.2.0", "[dependencies]\nrequired = { a = \"*\" }"),
            m("a", "0.1.0-alpha.1", ""),
            m("z", "1.0.0", ""),
            m("d", "1.0.0", ""),
        ]);
        assert!(r.refused.is_empty(), "{:?}", r.refused);
        assert_eq!(ids(&r.order), ["a", "b", "c", "d", "z"]);
    }

    #[test]
    fn optional_and_load_before() {
        let r = run(&[
            m(
                "a",
                "1.0.0",
                "[dependencies]\noptional = { z = \"*\", missing = \"*\" }",
            ),
            m(
                "z",
                "1.0.0",
                "[dependencies]\nload_before = [\"m\", \"absent\"]",
            ),
            m("m", "1.0.0", ""),
        ]);
        assert!(r.refused.is_empty());
        assert_eq!(ids(&r.order), ["z", "a", "m"]);
    }

    #[test]
    fn optional_version_mismatch_is_ignored_with_warning() {
        let r = run(&[
            m("a", "1.0.0", "[dependencies]\noptional = { z = \"^2\" }"),
            m("z", "1.0.0", "[dependencies]\nload_before = []"),
        ]);
        assert_eq!(ids(&r.order), ["a", "z"]);
        assert!(r.warnings.iter().any(|w| w.contains("does not match")));
    }

    #[test]
    fn missing_and_mismatched_required() {
        let r = run(&[
            m(
                "a",
                "1.0.0",
                "[dependencies]\nrequired = { ghost = \"^1\" }",
            ),
            m("b", "1.0.0", "[dependencies]\nrequired = { c = \">=2\" }"),
            m("c", "1.5.0", ""),
            m("d", "1.0.0", "[dependencies]\nrequired = { a = \"*\" }"),
        ]);
        assert_eq!(ids(&r.order), ["c"]);
        assert!(matches!(
            r.refused["a"],
            ResolveError::MissingDependency { .. }
        ));
        assert!(matches!(
            r.refused["b"],
            ResolveError::VersionMismatch { .. }
        ));
        let d = r.refused["d"].to_string();
        assert!(
            d.contains("`a` cannot be loaded") && d.contains("ghost"),
            "{d}"
        );
    }

    #[test]
    fn required_cycle_refuses_every_member_and_dependents() {
        let r = run(&[
            m("a", "1.0.0", "[dependencies]\nrequired = { b = \"*\" }"),
            m("b", "1.0.0", "[dependencies]\nrequired = { c = \"*\" }"),
            m("c", "1.0.0", "[dependencies]\nrequired = { a = \"*\" }"),
            m("d", "1.0.0", "[dependencies]\nrequired = { a = \"*\" }"),
            m("e", "1.0.0", "[dependencies]\noptional = { a = \"*\" }"),
            m("f", "1.0.0", ""),
        ]);
        assert_eq!(ids(&r.order), ["e", "f"]);
        for x in ["a", "b", "c"] {
            let e = r.refused[x].to_string();
            assert!(e.contains("cycle between a, b, c"), "{e}");
        }
        assert!(matches!(
            r.refused["d"],
            ResolveError::DependencyRefused { .. }
        ));
    }

    #[test]
    fn soft_cycle_is_broken() {
        let r = run(&[
            m("a", "1.0.0", "[dependencies]\noptional = { b = \"*\" }"),
            m("b", "1.0.0", "[dependencies]\noptional = { a = \"*\" }"),
            m(
                "c",
                "1.0.0",
                "[dependencies]\nrequired = { b = \"*\" }\noptional = { a = \"*\" }",
            ),
        ]);
        assert!(r.refused.is_empty());
        assert_eq!(ids(&r.order), ["a", "b", "c"]);
        assert!(r.warnings.iter().any(|w| w.contains("cycle between a, b")));
        // A load_before contradicting a required dependency: required wins, warning.
        let r = run(&[
            m("a", "1.0.0", "[dependencies]\nrequired = { b = \"*\" }"),
            m("b", "1.0.0", "[dependencies]\nload_before = []"),
            m("x", "1.0.0", "[dependencies]\nload_before = [\"y\"]"),
            m("y", "1.0.0", "[dependencies]\nload_before = [\"x\"]"),
        ]);
        assert!(r.refused.is_empty());
        assert_eq!(ids(&r.order), ["b", "a", "x", "y"]);
    }

    #[test]
    fn discovery_scans_archives_and_dirs() {
        use crate::archive::RpluginBuilder;
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        RpluginBuilder::from_manifest_text(&manifest_text("a", "1.0.0", ""), vec![1])
            .unwrap()
            .write_to(&p.join("a.rplugin"))
            .unwrap();
        // Duplicate id "b": archive + dev dir.
        RpluginBuilder::from_manifest_text(&manifest_text("b", "1.0.0", ""), vec![1])
            .unwrap()
            .write_to(&p.join("b.rplugin"))
            .unwrap();
        std::fs::create_dir_all(p.join("b-dev")).unwrap();
        std::fs::write(
            p.join("b-dev").join(MANIFEST_FILE),
            manifest_text("b", "1.1.0", ""),
        )
        .unwrap();
        std::fs::write(p.join("b-dev").join("b.wasm"), b"x").unwrap();
        // A data folder (ignored), a broken archive (error), a stray file (ignored).
        std::fs::create_dir_all(p.join("a").join("players")).unwrap();
        std::fs::write(p.join("broken.rplugin"), b"nope").unwrap();
        std::fs::write(p.join("notes.txt"), b"hi").unwrap();
        let r = discover(p).unwrap();
        let found: Vec<&str> = r.packages.iter().map(|p| p.manifest.id.as_str()).collect();
        assert_eq!(found, ["a"]);
        assert_eq!(r.errors.len(), 3, "{:?}", r.errors);
        assert!(
            r.errors
                .iter()
                .filter(|e| e.message.contains("duplicate plugin id `b`"))
                .count()
                == 2
        );
        assert!(discover(&p.join("missing")).unwrap().packages.is_empty());
    }
}
