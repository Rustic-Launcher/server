//! Aggregating plugin reports over a panel, and rendering Markdown.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fmt::Write as _;

use serde::{Deserialize, Serialize};

use crate::analyze::{PluginReport, RedKind, Verdict};
use crate::classfile::MemberKind;
use crate::coverage::Status;

/// Verdict counts.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Summary {
    pub total: u32,
    pub green: u32,
    pub yellow: u32,
    pub red: u32,
    /// Red plugins whose own code is clean (only bundled libraries are red).
    pub red_only_in_libraries: u32,
    pub error: u32,
    /// Jars without plugin.yml / paper-plugin.yml.
    pub without_descriptor: u32,
    /// Red plugins whose only decisive signal is reflection with internal names (often a
    /// guarded fallback path rather than a hard requirement).
    pub red_reflection_only: u32,
    /// Bytecode sites hitting Bukkit/Paper API members over the panel.
    pub api_sites: u64,
    /// Of which on covered members (partial included).
    pub api_sites_covered: u64,
    /// Of which on partially covered members.
    pub api_sites_partial: u64,
    /// Distinct API members (owner#name, overloads merged) used anywhere in the panel.
    pub distinct_members: u32,
    /// Of which covered (all overloads covered or partial).
    pub distinct_members_covered: u32,
}

/// A class or member ranked over the panel.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Ranked {
    pub name: String,
    /// Plugins using it.
    pub plugins: u32,
    /// Bytecode sites over the panel (members), or member sites on the class (classes).
    pub sites: u64,
    /// For members merged over overloads: the least covered overload.
    pub status: Status,
}

/// How many red plugins show each kind of red signal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RedKindCount {
    pub kind: RedKind,
    pub label: String,
    pub plugins: u32,
    /// Of which the signal is in the plugin's own code.
    pub plugins_own_code: u32,
}

/// "If Rustic covered the K most used missing members/types, N yellow plugins would turn green."
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverageStep {
    pub top_k: u32,
    /// Yellow plugins that would turn green (missing things ranked over yellow plugins).
    pub green_after: u32,
    /// Plugins of any verdict whose API needs would be fully covered (missing things ranked over
    /// all plugins); red plugins stay red, this only measures the API side.
    pub api_complete_after: u32,
}

/// Rank missing things by how many sets contain them, then count, for each K, the sets whose
/// every element is among the K first.
fn curve(sets: &[BTreeSet<String>], ks: &[u32]) -> Vec<u32> {
    let mut freq: HashMap<&str, u32> = HashMap::new();
    for b in sets {
        for k in b {
            *freq.entry(k.as_str()).or_default() += 1;
        }
    }
    let mut order: Vec<(&str, u32)> = freq.into_iter().collect();
    order.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(b.0)));
    let rank: HashMap<&str, usize> = order
        .iter()
        .enumerate()
        .map(|(i, (k, _))| (*k, i))
        .collect();
    ks.iter()
        .map(|&k| {
            sets.iter()
                .filter(|b| {
                    b.iter()
                        .all(|x| rank.get(x.as_str()).is_some_and(|r| *r < k as usize))
                })
                .count() as u32
        })
        .collect()
}

/// The analysis of a whole panel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PanelReport {
    pub analyzer_version: String,
    pub coverage_api: String,
    pub summary: Summary,
    pub top_classes: Vec<Ranked>,
    pub top_methods: Vec<Ranked>,
    pub top_methods_by_sites: Vec<Ranked>,
    pub top_uncovered_members: Vec<Ranked>,
    pub top_uncovered_types: Vec<Ranked>,
    pub uncovered_by_class: Vec<Ranked>,
    pub top_partial_members: Vec<Ranked>,
    pub red_kinds: Vec<RedKindCount>,
    pub coverage_curve: Vec<CoverageStep>,
    pub plugins: Vec<PluginReport>,
}

impl PanelReport {
    /// A smaller copy for committing or sharing: per plugin, only the `keep` most used uncovered
    /// members and types, the `keep` strongest red signals and the first `keep` commands and
    /// permissions are kept (covered members are dropped); rankings and summary are intact.
    pub fn compact(&self, keep: usize) -> PanelReport {
        let mut out = self.clone();
        for p in &mut out.plugins {
            p.api_members.retain(|m| !m.status.is_covered());
            p.api_members.truncate(keep);
            p.api_types.retain(|t| !t.status.is_covered());
            p.api_types.truncate(keep);
            p.platform_libraries.truncate(10);
            p.external_apis.truncate(10);
            p.other_platforms.truncate(10);
            p.bundled_libraries.truncate(10);
            p.red_signals
                .sort_by(|a, b| b.decisive.cmp(&a.decisive).then(b.refs.cmp(&a.refs)));
            p.red_signals.truncate(keep);
            if let Some(m) = &mut p.meta {
                m.permissions.truncate(keep);
                m.commands.truncate(keep);
            }
        }
        out
    }
}

fn severity(s: &Status) -> u8 {
    match s {
        Status::Covered(_) => 0,
        Status::Partial(_) => 1,
        Status::Uncovered(_) => 2,
    }
}

#[derive(Default, Clone)]
struct Acc {
    plugins: BTreeSet<usize>,
    sites: u64,
    status: Option<Status>,
}

impl Acc {
    fn add(&mut self, plugin: usize, sites: u64, status: &Status) {
        self.plugins.insert(plugin);
        self.sites += sites;
        if self
            .status
            .as_ref()
            .is_none_or(|s| severity(status) > severity(s))
        {
            self.status = Some(status.clone());
        }
    }
}

fn ranked(map: HashMap<String, Acc>, by_sites: bool, limit: usize) -> Vec<Ranked> {
    let mut v: Vec<Ranked> = map
        .into_iter()
        .map(|(name, a)| Ranked {
            name,
            plugins: a.plugins.len() as u32,
            sites: a.sites,
            status: a.status.unwrap_or(Status::Uncovered(None)),
        })
        .collect();
    if by_sites {
        v.sort_by(|a, b| {
            b.sites
                .cmp(&a.sites)
                .then(b.plugins.cmp(&a.plugins))
                .then_with(|| a.name.cmp(&b.name))
        });
    } else {
        v.sort_by(|a, b| {
            b.plugins
                .cmp(&a.plugins)
                .then(b.sites.cmp(&a.sites))
                .then_with(|| a.name.cmp(&b.name))
        });
    }
    v.truncate(limit);
    v
}

/// Aggregate plugin reports. `coverage_api` labels the table used (`[meta] api`).
pub fn build_panel(plugins: Vec<PluginReport>, coverage_api: &str) -> PanelReport {
    let mut summary = Summary {
        total: plugins.len() as u32,
        ..Summary::default()
    };
    let mut classes: HashMap<String, Acc> = HashMap::new();
    let mut methods: HashMap<String, Acc> = HashMap::new();
    let mut uncovered: HashMap<String, Acc> = HashMap::new();
    let mut uncovered_types: HashMap<String, Acc> = HashMap::new();
    let mut by_class: HashMap<String, Acc> = HashMap::new();
    let mut partial: HashMap<String, Acc> = HashMap::new();
    let mut red_kinds: BTreeMap<RedKind, (BTreeSet<usize>, BTreeSet<usize>)> = BTreeMap::new();
    // Per yellow plugin, and per analyzed plugin: the set of missing things blocking it.
    let mut blockers: Vec<BTreeSet<String>> = Vec::new();
    let mut api_needs: Vec<BTreeSet<String>> = Vec::new();

    for (i, p) in plugins.iter().enumerate() {
        match p.verdict {
            Verdict::Green => summary.green += 1,
            Verdict::Yellow => summary.yellow += 1,
            Verdict::Red => summary.red += 1,
            Verdict::Error => summary.error += 1,
        }
        if p.red_only_in_libraries {
            summary.red_only_in_libraries += 1;
        }
        if p.meta.is_none() && p.verdict != Verdict::Error {
            summary.without_descriptor += 1;
        }
        if p.verdict == Verdict::Red
            && p.red_signals
                .iter()
                .filter(|r| r.decisive)
                .all(|r| r.kind == RedKind::ReflectionIntoInternals)
        {
            summary.red_reflection_only += 1;
        }
        for t in &p.api_types {
            classes
                .entry(t.class.clone())
                .or_default()
                .add(i, 0, &t.status);
            if !t.status.is_covered() {
                uncovered_types.entry(t.class.clone()).or_default().add(
                    i,
                    u64::from(t.classes),
                    &t.status,
                );
            }
        }
        for m in &p.api_members {
            let sites = u64::from(m.sites);
            summary.api_sites += sites;
            if m.status.is_covered() {
                summary.api_sites_covered += sites;
            }
            if m.status.is_partial() {
                summary.api_sites_partial += sites;
            }
            let c = classes.entry(m.owner.clone()).or_default();
            c.plugins.insert(i);
            c.sites += sites;
            if m.kind == MemberKind::Method {
                methods.entry(m.key()).or_default().add(i, sites, &m.status);
            }
            if !m.status.is_covered() {
                uncovered
                    .entry(m.key())
                    .or_default()
                    .add(i, sites, &m.status);
                by_class
                    .entry(m.owner.clone())
                    .or_default()
                    .add(i, sites, &m.status);
            } else if m.status.is_partial() {
                partial.entry(m.key()).or_default().add(i, sites, &m.status);
            }
        }
        for r in p.red_signals.iter().filter(|r| r.decisive) {
            let e = red_kinds.entry(r.kind).or_default();
            e.0.insert(i);
            if r.origin == "own" {
                e.1.insert(i);
            }
        }
        if p.verdict != Verdict::Error {
            let mut set: BTreeSet<String> = p.uncovered_members().map(|m| m.key()).collect();
            set.extend(p.uncovered_types().map(|t| format!("type {}", t.class)));
            if p.verdict == Verdict::Yellow {
                blockers.push(set.clone());
            }
            api_needs.push(set);
        }
    }

    const KS: [u32; 9] = [10, 25, 50, 100, 200, 300, 500, 1000, 2000];
    let coverage_curve = curve(&blockers, &KS)
        .into_iter()
        .zip(curve(&api_needs, &KS))
        .zip(KS)
        .map(|((green_after, api_complete_after), top_k)| CoverageStep {
            top_k,
            green_after,
            api_complete_after,
        })
        .collect();

    let red_kinds = red_kinds
        .into_iter()
        .map(|(kind, (all, own))| RedKindCount {
            kind,
            label: kind.label().to_owned(),
            plugins: all.len() as u32,
            plugins_own_code: own.len() as u32,
        })
        .collect();
    let mut all_members: HashMap<String, bool> = HashMap::new();
    for p in &plugins {
        for m in &p.api_members {
            *all_members.entry(m.key()).or_insert(true) &= m.status.is_covered();
        }
    }
    summary.distinct_members = all_members.len() as u32;
    summary.distinct_members_covered = all_members.values().filter(|c| **c).count() as u32;
    let top_methods_by_sites = ranked(methods.clone(), true, 50);
    PanelReport {
        analyzer_version: env!("CARGO_PKG_VERSION").to_owned(),
        coverage_api: coverage_api.to_owned(),
        summary,
        top_classes: ranked(classes, false, 50),
        top_methods: ranked(methods, false, 100),
        top_methods_by_sites,
        top_uncovered_members: ranked(uncovered, false, 100),
        top_uncovered_types: ranked(uncovered_types, false, 50),
        uncovered_by_class: ranked(by_class, false, 50),
        top_partial_members: ranked(partial, false, 30),
        red_kinds,
        coverage_curve,
        plugins,
    }
}

fn pct(n: u32, total: u32) -> String {
    if total == 0 {
        "0%".into()
    } else {
        format!("{:.0}%", f64::from(n) * 100.0 / f64::from(total))
    }
}

fn status_cell(s: &Status) -> String {
    match s {
        Status::Covered(m) => format!("✅ `{}`", m.replace('|', "\\|")),
        Status::Partial(m) => format!("◐ {}", m.replace('|', "\\|")),
        Status::Uncovered(Some(m)) => format!(
            "❌ {}",
            m.trim_start_matches("uncovered")
                .trim_start_matches(':')
                .trim()
                .replace('|', "\\|")
        ),
        Status::Uncovered(None) => "❌".into(),
    }
}

fn short(name: &str) -> String {
    name.strip_prefix("org.bukkit.")
        .map(|s| format!("o.b.{s}"))
        .unwrap_or_else(|| name.to_owned())
}

fn ranked_table(out: &mut String, rows: &[Ranked], with_status: bool) {
    if with_status {
        out.push_str("| # | Name | Plugins | Sites | Rustic |\n|---:|---|---:|---:|---|\n");
    } else {
        out.push_str("| # | Name | Plugins | Sites |\n|---:|---|---:|---:|\n");
    }
    for (i, r) in rows.iter().enumerate() {
        let _ = write!(
            out,
            "| {} | `{}` | {} | {} |",
            i + 1,
            short(&r.name),
            r.plugins,
            r.sites
        );
        if with_status {
            let _ = write!(out, " {} |", status_cell(&r.status));
        }
        out.push('\n');
    }
    out.push('\n');
}

/// Render a panel as Markdown: summary, per-plugin table and the ranked tables.
/// `o.b.` abbreviates `org.bukkit.` in tables.
pub fn render_panel_markdown(p: &PanelReport) -> String {
    let s = &p.summary;
    let analyzed = s.total - s.error;
    let mut out = String::new();
    let _ = writeln!(out, "### Distribution\n");
    let _ = writeln!(out, "| Verdict | Plugins | Share |\n|---|---:|---:|");
    let _ = writeln!(
        out,
        "| 🟢 green | {} | {} |",
        s.green,
        pct(s.green, analyzed)
    );
    let _ = writeln!(
        out,
        "| 🟡 yellow | {} | {} |",
        s.yellow,
        pct(s.yellow, analyzed)
    );
    let _ = writeln!(out, "| 🔴 red | {} | {} |", s.red, pct(s.red, analyzed));
    let _ = writeln!(out, "| **analyzed** | **{analyzed}** | 100% |");
    if s.error > 0 {
        let _ = writeln!(out, "| ⚠️ unreadable | {} | |", s.error);
    }
    let _ = writeln!(
        out,
        "\nOf the {} red plugins, {} are red only because of a bundled library (their own code is clean), \
         and {} only because of reflection with internal class names (often a guarded fallback).\n",
        s.red, s.red_only_in_libraries, s.red_reflection_only
    );
    let site_pct = |n: u64| {
        if s.api_sites == 0 {
            0.0
        } else {
            n as f64 * 100.0 / s.api_sites as f64
        }
    };
    let _ = writeln!(
        out,
        "API coverage over the panel: {} of {} distinct API members used ({}), and {:.1}% of the {} \
         bytecode sites that call the API ({:.1}% through a partial mapping).\n",
        s.distinct_members_covered,
        s.distinct_members,
        pct(s.distinct_members_covered, s.distinct_members),
        site_pct(s.api_sites_covered),
        s.api_sites,
        site_pct(s.api_sites_partial)
    );

    let _ = writeln!(
        out,
        "### Red signals\n\n| Signal | Red plugins | Of which in own code |\n|---|---:|---:|"
    );
    for r in &p.red_kinds {
        let _ = writeln!(
            out,
            "| {} | {} | {} |",
            r.label, r.plugins, r.plugins_own_code
        );
    }
    out.push('\n');

    let _ = writeln!(out, "### Per plugin\n");
    out.push_str("| Plugin | Version | Source | Licence | Verdict | API members (covered / used) | Uncovered members | Uncovered types | Why |\n");
    out.push_str("|---|---|---|---|:---:|---:|---:|---:|---|\n");
    let mut rows: Vec<&PluginReport> = p.plugins.iter().collect();
    rows.sort_by_key(|r| (r.verdict, r.display_name().to_lowercase()));
    for r in rows {
        let used = r.api_members.len();
        let covered = r
            .api_members
            .iter()
            .filter(|m| m.status.is_covered())
            .count();
        let (version, source, licence) = match &r.panel {
            Some(pi) => (
                pi.version.clone(),
                pi.source.clone(),
                pi.license.clone().unwrap_or_default(),
            ),
            None => (
                r.meta
                    .as_ref()
                    .map(|m| m.version.clone())
                    .unwrap_or_default(),
                String::new(),
                String::new(),
            ),
        };
        let why = r
            .reasons
            .iter()
            .filter(|x| !x.starts_with("uses "))
            .cloned()
            .collect::<Vec<_>>()
            .join("; ");
        let why = if why.is_empty() {
            r.uncovered_members()
                .take(3)
                .map(|m| short(&m.key()))
                .collect::<Vec<_>>()
                .join(", ")
        } else if r.red_only_in_libraries {
            format!("(bundled libraries only) {why}")
        } else {
            why
        };
        let _ = writeln!(
            out,
            "| {} | {} | {} | {} | {} | {} / {} | {} | {} | {} |",
            r.display_name().replace('|', "/"),
            version.replace('|', "/"),
            source,
            licence.replace('|', "/"),
            r.verdict.emoji(),
            covered,
            used,
            r.uncovered_members().count(),
            r.uncovered_types().count(),
            why.replace('|', "/")
        );
    }
    out.push('\n');

    let _ = writeln!(
        out,
        "### Top {} Bukkit/Paper classes (by plugins using them)\n",
        p.top_classes.len()
    );
    ranked_table(&mut out, &p.top_classes, true);
    let _ = writeln!(
        out,
        "### Top {} methods (by plugins using them)\n",
        p.top_methods.len()
    );
    ranked_table(&mut out, &p.top_methods, true);
    let _ = writeln!(
        out,
        "### Top {} methods by call sites\n",
        p.top_methods_by_sites.len()
    );
    ranked_table(&mut out, &p.top_methods_by_sites, true);
    let _ = writeln!(
        out,
        "### Top {} uncovered members (by plugins using them)\n",
        p.top_uncovered_members.len()
    );
    ranked_table(&mut out, &p.top_uncovered_members, false);
    let _ = writeln!(out, "### Uncovered members grouped by class\n");
    ranked_table(&mut out, &p.uncovered_by_class, false);
    let _ = writeln!(
        out,
        "### Top {} uncovered types (plugins; sites = referencing classes)\n",
        p.top_uncovered_types.len()
    );
    ranked_table(&mut out, &p.top_uncovered_types, false);
    let _ = writeln!(out, "### Most used partial mappings\n");
    ranked_table(&mut out, &p.top_partial_members, true);
    let _ = writeln!(
        out,
        "### Coverage curve\n\nIf Rustic covered the K most needed missing members/types, this many of the {} \
         yellow plugins would turn green (ranking over yellow plugins), and this many of all {} analyzed \
         plugins would have every API member they use covered (ranking over all plugins; red ones stay red):\n",
        s.yellow, analyzed
    );
    out.push_str("| K | Yellow → green | API-complete (all plugins) |\n|---:|---:|---:|\n");
    for c in &p.coverage_curve {
        let _ = writeln!(
            out,
            "| {} | {} | {} |",
            c.top_k, c.green_after, c.api_complete_after
        );
    }
    out.push('\n');
    out
}

/// Render one plugin's report as Markdown.
pub fn render_plugin_markdown(r: &PluginReport) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "## {} {} {}\n",
        r.verdict.emoji(),
        r.display_name(),
        r.meta.as_ref().map(|m| m.version.as_str()).unwrap_or("")
    );
    let _ = writeln!(out, "- file: `{}`", r.file);
    if let Some(m) = &r.meta {
        let _ = writeln!(
            out,
            "- main: `{}`, api-version: {}",
            m.main,
            m.api_version.as_deref().unwrap_or("-")
        );
        if !m.depend.is_empty() {
            let _ = writeln!(out, "- depend: {}", m.depend.join(", "));
        }
        if !m.softdepend.is_empty() {
            let _ = writeln!(out, "- softdepend: {}", m.softdepend.join(", "));
        }
    }
    let st = &r.stats;
    let _ = writeln!(
        out,
        "- classes: {} ({} own under `{}`, {} bundled), reflection calls: {}",
        st.classes, st.own_classes, st.own_prefix, st.library_classes, st.reflection_calls
    );
    for reason in &r.reasons {
        let _ = writeln!(out, "- {reason}");
    }
    let unc: Vec<_> = r.uncovered_members().collect();
    if !unc.is_empty() {
        let _ = writeln!(out, "\n| Uncovered member | Sites |\n|---|---:|");
        for m in unc.iter().take(40) {
            let _ = writeln!(out, "| `{}` | {} |", m.key(), m.sites);
        }
        if unc.len() > 40 {
            let _ = writeln!(out, "| ... {} more | |", unc.len() - 40);
        }
    }
    if !r.red_signals.is_empty() {
        let _ = writeln!(
            out,
            "\n| Red signal | Target | Origin | Refs | Decisive |\n|---|---|---|---:|:---:|"
        );
        for s in r.red_signals.iter().take(30) {
            let _ = writeln!(
                out,
                "| {} | `{}` | {} | {} | {} |",
                s.kind.label(),
                s.target,
                s.origin,
                s.refs,
                if s.decisive { "yes" } else { "no" }
            );
        }
    }
    out.push('\n');
    out
}
