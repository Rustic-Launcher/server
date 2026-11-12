//! Rustic Bridge analyzer: reads Bukkit/Spigot/Paper plugin jars **statically** (no JVM, no
//! class loading) and tells how far each one is from running on Rustic.
//!
//! For each jar it reads `plugin.yml` / `paper-plugin.yml`, parses every class file (its own
//! compact parser, [`classfile`]), collects the classes, methods and fields referenced outside
//! the jar, and classifies the plugin:
//!
//! - 🟢 [`Verdict::Green`]: only Bukkit/Paper/Adventure API that Rustic covers
//!   ([`Coverage`], `data/coverage.toml`, built from the WIT contract);
//! - 🟡 [`Verdict::Yellow`]: API that Rustic does not cover yet, listed member by member;
//! - 🔴 [`Verdict::Red`]: NMS, CraftBukkit, other server internals, ProtocolLib/PacketEvents,
//!   reflection into internals, bytecode instrumentation or `Unsafe`, or a hard dependency on a
//!   plugin of that kind.
//!
//! Classes inside the jar (the plugin's own and shaded libraries) are never targets. The rules
//! are recorded in `docs/DECISIONS/0008-bridge-analyzer.md`.
//!
//! ```no_run
//! use rusticmc_bridge_analyze::{Coverage, analyze_path, build_panel, render_panel_markdown};
//! let cov = Coverage::builtin();
//! let report = analyze_path(std::path::Path::new("SomePlugin.jar"), &cov);
//! println!("{:?}: {:?}", report.verdict, report.reasons);
//! let panel = build_panel(vec![report], &cov.meta.api);
//! println!("{}", render_panel_markdown(&panel));
//! ```

pub mod analyze;
pub mod classfile;
pub mod coverage;
pub mod jar;
pub mod manifest;
pub mod report;

use std::path::{Path, PathBuf};

pub use analyze::{
    MemberUse, PanelInfo, PluginReport, RedKind, RedSignal, TypeUse, Verdict, analyze_bytes,
    analyze_path,
};
pub use coverage::{Coverage, Status};
pub use manifest::PluginMeta;
pub use report::{PanelReport, build_panel, render_panel_markdown, render_plugin_markdown};

/// Expand files and directories into the `.jar` files to analyze (directories: their direct
/// `*.jar` children, sorted).
pub fn collect_jars(inputs: &[PathBuf]) -> std::io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for p in inputs {
        if p.is_dir() {
            let mut v: Vec<PathBuf> = std::fs::read_dir(p)?
                .filter_map(|e| e.ok().map(|e| e.path()))
                .filter(|q| q.extension().is_some_and(|x| x.eq_ignore_ascii_case("jar")))
                .collect();
            v.sort();
            out.extend(v);
        } else {
            out.push(p.clone());
        }
    }
    Ok(out)
}

/// Read `bridge/panel/panel.json` (written by `bridge/panel/fetch.py`): entries keyed by jar
/// file name.
pub fn read_panel_index(
    path: &Path,
) -> Result<std::collections::HashMap<String, PanelInfo>, String> {
    #[derive(serde::Deserialize)]
    struct Index {
        plugins: Vec<PanelInfo>,
    }
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let idx: Index = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(idx
        .plugins
        .into_iter()
        .map(|p| (p.jar.clone(), p))
        .collect())
}

/// Analyze several jars (files or directories) and aggregate them.
pub fn analyze_all(
    inputs: &[PathBuf],
    cov: &Coverage,
    panel: Option<&Path>,
) -> Result<PanelReport, String> {
    let jars = collect_jars(inputs).map_err(|e| e.to_string())?;
    let index = match panel {
        Some(p) => read_panel_index(p)?,
        None => Default::default(),
    };
    let reports = jars
        .iter()
        .map(|j| {
            let mut r = analyze_path(j, cov);
            r.panel = index.get(&r.file).cloned();
            r
        })
        .collect();
    Ok(build_panel(reports, &cov.meta.api))
}

#[cfg(test)]
mod tests;
