//! `rustic-bridge-analyze`: analyze Bukkit/Spigot/Paper plugin jars for Rustic compatibility.

use std::path::PathBuf;
use std::process::ExitCode;

use rusticmc_bridge_analyze::{
    Coverage, analyze_all, render_panel_markdown, render_plugin_markdown,
};

const USAGE: &str = "\
rustic-bridge-analyze: static Bukkit/Paper API usage and Rustic coverage of plugin jars

USAGE:
    rustic-bridge-analyze [OPTIONS] <JAR_OR_DIR>...

OPTIONS:
    --json <FILE>       write the full report as JSON
    --markdown <FILE>   write the Markdown report to FILE instead of stdout
    --coverage <FILE>   use this coverage table instead of the built-in one
    --panel <FILE>      panel.json from bridge/panel/fetch.py (names, versions, licences)
    --details           append a section per plugin to the Markdown
    --compact <N>       in the JSON, keep per plugin only the N most used uncovered members
                        and types (the full report lists every member used)
    -h, --help          this help

The verdict of each jar is green (only covered API), yellow (uncovered API) or red (server
internals, packet libraries, instrumentation). Exit code 0 unless an argument is wrong.";

fn main() -> ExitCode {
    let mut args = std::env::args().skip(1);
    let mut inputs = Vec::new();
    let (mut json, mut markdown, mut coverage, mut panel) = (None, None, None, None);
    let mut details = false;
    let mut compact: Option<usize> = None;
    while let Some(a) = args.next() {
        let mut value = |name: &str| {
            args.next()
                .map(PathBuf::from)
                .ok_or_else(|| format!("{name} needs a value"))
        };
        let r = match a.as_str() {
            "-h" | "--help" => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            "--json" => value("--json").map(|v| json = Some(v)),
            "--markdown" => value("--markdown").map(|v| markdown = Some(v)),
            "--coverage" => value("--coverage").map(|v| coverage = Some(v)),
            "--panel" => value("--panel").map(|v| panel = Some(v)),
            "--compact" => value("--compact")
                .and_then(|v| {
                    v.to_string_lossy()
                        .parse::<usize>()
                        .map_err(|_| "--compact needs a number".to_owned())
                })
                .map(|n| compact = Some(n)),
            "--details" => {
                details = true;
                Ok(())
            }
            s if s.starts_with('-') => Err(format!("unknown option {s}")),
            s => {
                inputs.push(PathBuf::from(s));
                Ok(())
            }
        };
        if let Err(e) = r {
            eprintln!("error: {e}\n\n{USAGE}");
            return ExitCode::from(2);
        }
    }
    if inputs.is_empty() {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }
    let cov = match &coverage {
        Some(p) => match std::fs::read_to_string(p)
            .map_err(|e| e.to_string())
            .and_then(|s| Coverage::from_toml_str(&s))
        {
            Ok(c) => c,
            Err(e) => {
                eprintln!("error: {}: {e}", p.display());
                return ExitCode::from(2);
            }
        },
        None => Coverage::builtin(),
    };
    let report = match analyze_all(&inputs, &cov, panel.as_deref()) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::from(2);
        }
    };
    let mut md = render_panel_markdown(&report);
    if details {
        md.push_str("## Plugins in detail\n\n");
        for p in &report.plugins {
            md.push_str(&render_plugin_markdown(p));
        }
    }
    if let Some(path) = &json {
        let full;
        let r = match compact {
            Some(n) => {
                full = report.compact(n);
                &full
            }
            None => &report,
        };
        let text = serde_json::to_string_pretty(r).expect("report serializes");
        if let Err(e) = std::fs::write(path, text) {
            eprintln!("error: {}: {e}", path.display());
            return ExitCode::FAILURE;
        }
    }
    match &markdown {
        Some(path) => {
            if let Err(e) = std::fs::write(path, md) {
                eprintln!("error: {}: {e}", path.display());
                return ExitCode::FAILURE;
            }
        }
        None => print!("{md}"),
    }
    let s = &report.summary;
    eprintln!(
        "{} jars: {} green, {} yellow, {} red, {} unreadable",
        s.total, s.green, s.yellow, s.red, s.error
    );
    ExitCode::SUCCESS
}
