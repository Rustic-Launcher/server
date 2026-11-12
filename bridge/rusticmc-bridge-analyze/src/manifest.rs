//! `plugin.yml` (Bukkit) and `paper-plugin.yml` (Paper) descriptors, read leniently.

use serde::{Deserialize, Serialize};
use yaml_serde::Value;

/// Which descriptor a plugin ships.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DescriptorKind {
    /// `plugin.yml` only.
    Bukkit,
    /// `paper-plugin.yml` (possibly alongside a `plugin.yml`).
    Paper,
}

/// The fields of a plugin descriptor the analysis uses.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct PluginMeta {
    pub name: String,
    pub version: String,
    pub main: String,
    pub api_version: Option<String>,
    pub kind: Option<DescriptorKind>,
    /// Hard dependencies (`depend`, or Paper `dependencies.server.X.required: true`).
    pub depend: Vec<String>,
    pub softdepend: Vec<String>,
    pub loadbefore: Vec<String>,
    pub commands: Vec<String>,
    pub permissions: Vec<String>,
    /// Maven coordinates the server downloads for the plugin (`libraries`).
    pub libraries: Vec<String>,
    /// Paper bootstrapper / loader classes, if any.
    pub paper_bootstrapper: Option<String>,
    pub paper_loader: Option<String>,
    pub folia_supported: bool,
}

fn scalar(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// A scalar exactly as written (`api-version: 1.20` must not become `1.2`).
fn literal(raw: &str, key: &str, v: &Value) -> Option<String> {
    if let Value::Number(_) = v {
        for line in raw.lines() {
            if let Some(rest) = line
                .strip_prefix(key)
                .and_then(|r| r.trim_start().strip_prefix(':'))
            {
                let lit = rest
                    .split('#')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .trim_matches(['"', '\'']);
                if !lit.is_empty() {
                    return Some(lit.to_owned());
                }
            }
        }
    }
    scalar(v)
}

fn list(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::Sequence(s)) => s.iter().filter_map(scalar).collect(),
        Some(v) => scalar(v).into_iter().collect(),
        None => Vec::new(),
    }
}

fn keys(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::Mapping(m)) => m.keys().filter_map(scalar).collect(),
        _ => Vec::new(),
    }
}

fn get<'a>(v: &'a Value, key: &str) -> Option<&'a Value> {
    v.as_mapping().and_then(|m| m.get(key))
}

/// Parse a `plugin.yml`. Falls back to a line scan of top-level keys when the YAML is invalid.
pub fn parse_plugin_yml(raw: &str) -> PluginMeta {
    let mut meta = PluginMeta {
        kind: Some(DescriptorKind::Bukkit),
        ..PluginMeta::default()
    };
    let Ok(doc) = yaml_serde::from_str::<Value>(raw) else {
        line_fallback(raw, &mut meta);
        return meta;
    };
    fill_common(raw, &doc, &mut meta);
    meta.depend = list(get(&doc, "depend"));
    meta.softdepend = list(get(&doc, "softdepend"));
    meta.loadbefore = list(get(&doc, "loadbefore"));
    meta.commands = keys(get(&doc, "commands"));
    meta.permissions = keys(get(&doc, "permissions"));
    meta.libraries = list(get(&doc, "libraries"));
    meta
}

/// Parse a `paper-plugin.yml` (`dependencies.server.<Name>.{required,load}`).
pub fn parse_paper_plugin_yml(raw: &str) -> PluginMeta {
    let mut meta = PluginMeta {
        kind: Some(DescriptorKind::Paper),
        ..PluginMeta::default()
    };
    let Ok(doc) = yaml_serde::from_str::<Value>(raw) else {
        line_fallback(raw, &mut meta);
        return meta;
    };
    fill_common(raw, &doc, &mut meta);
    meta.paper_bootstrapper = get(&doc, "bootstrapper").and_then(scalar);
    meta.paper_loader = get(&doc, "loader").and_then(scalar);
    meta.permissions = keys(get(&doc, "permissions"));
    let deps = get(&doc, "dependencies");
    // Both the current (`dependencies.server`) and early (`dependencies` list) layouts.
    let server = deps
        .and_then(|d| get(d, "server"))
        .or(deps.filter(|d| d.is_sequence()));
    match server {
        Some(Value::Mapping(m)) => {
            for (k, v) in m {
                let Some(name) = scalar(k) else { continue };
                let required = get(v, "required").and_then(Value::as_bool).unwrap_or(true);
                let load = get(v, "load")
                    .and_then(scalar)
                    .unwrap_or_default()
                    .to_ascii_uppercase();
                if load == "AFTER" {
                    meta.loadbefore.push(name.clone());
                }
                if required {
                    meta.depend.push(name);
                } else {
                    meta.softdepend.push(name);
                }
            }
        }
        Some(Value::Sequence(s)) => {
            for d in s {
                if let Some(name) = get(d, "name").and_then(scalar) {
                    if get(d, "required").and_then(Value::as_bool).unwrap_or(true) {
                        meta.depend.push(name);
                    } else {
                        meta.softdepend.push(name);
                    }
                }
            }
        }
        _ => {}
    }
    meta
}

fn fill_common(raw: &str, doc: &Value, meta: &mut PluginMeta) {
    let field = |k: &str| get(doc, k).and_then(|v| literal(raw, k, v));
    meta.name = field("name").unwrap_or_default();
    meta.version = field("version").unwrap_or_default();
    meta.main = field("main").unwrap_or_default();
    meta.api_version = field("api-version");
    meta.folia_supported = get(doc, "folia-supported")
        .and_then(Value::as_bool)
        .unwrap_or(false);
}

fn line_fallback(raw: &str, meta: &mut PluginMeta) {
    for line in raw.lines() {
        if line.starts_with([' ', '\t', '#', '-']) {
            continue;
        }
        let Some((k, v)) = line.split_once(':') else {
            continue;
        };
        let v = v.trim().trim_matches(['"', '\'']).to_owned();
        match k.trim() {
            "name" => meta.name = v,
            "version" => meta.version = v,
            "main" => meta.main = v,
            "api-version" => meta.api_version = Some(v),
            _ => {}
        }
    }
}

/// Merge a `paper-plugin.yml` over a `plugin.yml` (Paper prefers the former when present).
pub fn merge(bukkit: Option<PluginMeta>, paper: Option<PluginMeta>) -> Option<PluginMeta> {
    match (bukkit, paper) {
        (Some(b), Some(mut p)) => {
            if p.commands.is_empty() {
                p.commands = b.commands;
            }
            if p.libraries.is_empty() {
                p.libraries = b.libraries;
            }
            Some(p)
        }
        (b, p) => p.or(b),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_yml_fields_and_literal_api_version() {
        let raw = "name: Demo\nversion: 1.0\nmain: com.example.Demo\napi-version: 1.20\n\
                   depend: [Vault]\nsoftdepend:\n  - PlaceholderAPI\nloadbefore: Essentials\n\
                   libraries:\n  - com.google.code.gson:gson:2.10\n\
                   commands:\n  demo:\n    description: x\n  other: {}\n\
                   permissions:\n  demo.use:\n    default: op\n";
        let m = parse_plugin_yml(raw);
        assert_eq!(m.name, "Demo");
        assert_eq!(m.version, "1.0");
        assert_eq!(m.api_version.as_deref(), Some("1.20"));
        assert_eq!(m.depend, ["Vault"]);
        assert_eq!(m.softdepend, ["PlaceholderAPI"]);
        assert_eq!(m.loadbefore, ["Essentials"]);
        assert_eq!(m.commands, ["demo", "other"]);
        assert_eq!(m.permissions, ["demo.use"]);
        assert_eq!(m.libraries, ["com.google.code.gson:gson:2.10"]);
    }

    #[test]
    fn paper_plugin_yml_dependencies() {
        let raw = "name: P\nversion: '2'\nmain: a.B\nbootstrapper: a.Boot\n\
                   dependencies:\n  server:\n    LuckPerms:\n      load: BEFORE\n      required: true\n\
                   \x20   Map:\n      load: AFTER\n      required: false\n";
        let m = parse_paper_plugin_yml(raw);
        assert_eq!(m.kind, Some(DescriptorKind::Paper));
        assert_eq!(m.depend, ["LuckPerms"]);
        assert_eq!(m.softdepend, ["Map"]);
        assert_eq!(m.loadbefore, ["Map"]);
        assert_eq!(m.paper_bootstrapper.as_deref(), Some("a.Boot"));
    }

    #[test]
    fn invalid_yaml_falls_back_to_lines() {
        let m = parse_plugin_yml("name: X\nmain: a.B\nversion: 3\n\tbroken: [\n");
        assert_eq!(
            (m.name.as_str(), m.main.as_str(), m.version.as_str()),
            ("X", "a.B", "3")
        );
    }
}
