//! `rustic.plugin.toml`: parsing and strict validation (see `docs/PLUGIN_FORMAT.md`).

use crate::id::{PluginId, is_valid_id_syntax};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// File name of the manifest, at the root of an archive or plugin directory.
pub const MANIFEST_FILE: &str = "rustic.plugin.toml";

/// The plugin API version this runtime implements (`rustic:api@0.1.0`).
pub const HOST_API: ApiVersion = ApiVersion { major: 0, minor: 1 };

/// Hard ceiling on `limits.memory_mb` in a manifest (the server config caps it further).
pub const MAX_MEMORY_MB: u32 = 4096;

/// A plugin API version, `MAJOR.MINOR`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ApiVersion {
    /// Major.
    pub major: u64,
    /// Minor.
    pub minor: u64,
}

impl ApiVersion {
    /// Parse `"MAJOR.MINOR"` (exactly two numeric components).
    pub fn parse(s: &str) -> Result<Self, String> {
        let mut parts = s.split('.');
        let (Some(a), Some(b), None) = (parts.next(), parts.next(), parts.next()) else {
            return Err(format!("api `{s}` must be \"MAJOR.MINOR\", e.g. \"0.1\""));
        };
        let num = |p: &str| -> Result<u64, String> {
            if p.is_empty()
                || !p.bytes().all(|c| c.is_ascii_digit())
                || (p.len() > 1 && p.starts_with('0'))
            {
                return Err(format!("api `{s}` must be \"MAJOR.MINOR\", e.g. \"0.1\""));
            }
            p.parse::<u64>().map_err(|e| format!("api `{s}`: {e}"))
        };
        Ok(Self {
            major: num(a)?,
            minor: num(b)?,
        })
    }

    /// Whether a plugin built for `self` runs on a host implementing `host`, by semver rules:
    /// for `0.x` the minor must match exactly (every 0.x minor may break), from `1.0` on the
    /// major must match and the plugin's minor must not be newer than the host's.
    pub fn is_compatible_with(self, host: ApiVersion) -> bool {
        if self.major != host.major {
            return false;
        }
        if self.major == 0 {
            // 0.x policy (docs/API.md): a plugin built for 0.x runs on a host whose API is 0.y
            // with y >= x, as long as 0.x is still inside the host's support window.
            self.minor <= host.minor && self.minor >= OLDEST_SUPPORTED_0_MINOR
        } else {
            self.minor <= host.minor
        }
    }
}

/// Oldest 0.x API minor this host still runs (the 0.x support window starts here).
pub const OLDEST_SUPPORTED_0_MINOR: u64 = 1;

impl fmt::Display for ApiVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

/// Outbound network access.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum NetworkCapability {
    /// No HTTP at all (`network = false`, the default).
    #[default]
    None,
    /// HTTP only to these hosts (`network = ["api.example.com", "*.example.org"]`).
    Domains(Vec<String>),
}

/// File system access through WASI.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FilesystemCapability {
    /// No file system.
    #[default]
    None,
    /// The plugin's data folder, preopened as `/`.
    DataFolder,
}

/// What the sandbox grants the plugin.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Capabilities {
    /// HTTP.
    pub network: NetworkCapability,
    /// File system.
    pub filesystem: FilesystemCapability,
}

impl Capabilities {
    /// Whether HTTP to `host` is allowed. `*.example.com` matches subdomains of
    /// `example.com` (not `example.com` itself). Comparison is case-insensitive.
    pub fn network_allows(&self, host: &str) -> bool {
        let host = host.trim_end_matches('.').to_ascii_lowercase();
        match &self.network {
            NetworkCapability::None => false,
            NetworkCapability::Domains(list) => list.iter().any(|d| {
                if let Some(suffix) = d.strip_prefix("*.") {
                    host.len() > suffix.len() + 1
                        && host.ends_with(suffix)
                        && host.as_bytes()[host.len() - suffix.len() - 1] == b'.'
                } else {
                    *d == host
                }
            }),
        }
    }
}

/// Per-plugin overrides of the sandbox limits (capped by the server config).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginLimits {
    /// Fuel per call into the plugin.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fuel_per_call: Option<u64>,
    /// Linear memory limit in MiB.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_mb: Option<u32>,
}

/// Dependencies of a plugin.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Dependencies {
    /// Must be present, enabled first, with a matching version.
    pub required: BTreeMap<PluginId, semver::VersionReq>,
    /// Enabled first when present (and matching), ignored otherwise.
    pub optional: BTreeMap<PluginId, semver::VersionReq>,
    /// Plugins this one must be loaded before (when present).
    pub load_before: Vec<PluginId>,
}

/// A validated manifest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manifest {
    /// Id.
    pub id: PluginId,
    /// Display name.
    pub name: String,
    /// Version.
    pub version: semver::Version,
    /// Targeted API.
    pub api: ApiVersion,
    /// Authors.
    pub authors: Vec<String>,
    /// SPDX licence expression.
    pub license: String,
    /// Path of the component inside the archive / directory (forward slashes).
    pub entry: String,
    /// Description.
    pub description: Option<String>,
    /// Dependencies.
    pub dependencies: Dependencies,
    /// Sandbox capabilities.
    pub capabilities: Capabilities,
    /// Limit overrides.
    pub limits: PluginLimits,
}

/// Why a manifest was refused: every problem found, not just the first.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ManifestError {
    /// Not TOML, or not the expected structure (unknown keys are refused).
    #[error("{MANIFEST_FILE}: {0}")]
    Syntax(String),
    /// Structurally fine but invalid values.
    #[error("{MANIFEST_FILE}: {}", .0.join("; "))]
    Invalid(Vec<String>),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawManifest {
    plugin: RawPlugin,
    #[serde(default, skip_serializing_if = "RawDependencies::is_empty")]
    dependencies: RawDependencies,
    #[serde(default)]
    capabilities: RawCapabilities,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    limits: Option<PluginLimits>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPlugin {
    id: String,
    name: String,
    version: String,
    api: String,
    authors: Vec<String>,
    license: String,
    entry: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    description: Option<String>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDependencies {
    #[serde(default)]
    required: BTreeMap<String, String>,
    #[serde(default)]
    optional: BTreeMap<String, String>,
    #[serde(default)]
    load_before: Vec<String>,
}

impl RawDependencies {
    fn is_empty(&self) -> bool {
        self.required.is_empty() && self.optional.is_empty() && self.load_before.is_empty()
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
enum RawNetwork {
    Flag(bool),
    Domains(Vec<String>),
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawCapabilities {
    #[serde(default = "no_network")]
    network: RawNetwork,
    #[serde(default)]
    filesystem: FilesystemCapability,
}

fn no_network() -> RawNetwork {
    RawNetwork::Flag(false)
}

impl Default for RawCapabilities {
    fn default() -> Self {
        Self {
            network: no_network(),
            filesystem: FilesystemCapability::None,
        }
    }
}

fn valid_domain(d: &str) -> bool {
    let body = d.strip_prefix("*.").unwrap_or(d);
    !body.is_empty()
        && body.len() <= 253
        && body.contains('.')
        && body.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        })
}

/// Validate an entry path: relative, forward slashes, no `..`, ends with `.wasm`.
pub fn validate_entry_path(entry: &str) -> Result<(), String> {
    if entry.is_empty() {
        return Err("entry must not be empty".into());
    }
    if entry.starts_with('/') || entry.contains('\\') || entry.contains(':') {
        return Err(format!(
            "entry `{entry}` must be a relative path with forward slashes"
        ));
    }
    if entry
        .split('/')
        .any(|seg| seg.is_empty() || seg == "." || seg == "..")
    {
        return Err(format!(
            "entry `{entry}` must not contain empty, `.` or `..` segments"
        ));
    }
    if !entry.ends_with(".wasm") {
        return Err(format!("entry `{entry}` must be a `.wasm` file"));
    }
    Ok(())
}

impl Manifest {
    /// Parse and validate manifest text. Does not check that the entry file exists (the
    /// archive / directory readers do).
    pub fn parse(text: &str) -> Result<Self, ManifestError> {
        let raw: RawManifest = toml::from_str(text)
            .map_err(|e| ManifestError::Syntax(e.to_string().trim().to_owned()))?;
        Self::validate(raw)
    }

    fn validate(raw: RawManifest) -> Result<Self, ManifestError> {
        let mut errors = Vec::new();
        let p = raw.plugin;
        let id = match PluginId::new(&p.id) {
            Ok(id) => Some(id),
            Err(e) => {
                errors.push(e.to_string());
                None
            }
        };
        let name = p.name.trim().to_owned();
        if name.is_empty() || name.chars().count() > 64 {
            errors.push("name must be 1 to 64 characters".into());
        }
        let version = match semver::Version::parse(&p.version) {
            Ok(v) => Some(v),
            Err(e) => {
                errors.push(format!("version `{}` is not semver: {e}", p.version));
                None
            }
        };
        let api = match ApiVersion::parse(&p.api) {
            Ok(a) => {
                if !a.is_compatible_with(HOST_API) {
                    errors.push(format!(
                        "api {a} is not compatible with this server's plugin API {HOST_API}"
                    ));
                }
                Some(a)
            }
            Err(e) => {
                errors.push(e);
                None
            }
        };
        if p.authors.is_empty() || p.authors.iter().any(|a| a.trim().is_empty()) {
            errors.push("authors must list at least one non-empty name".into());
        }
        if p.license.trim().is_empty() {
            errors.push("license must not be empty (an SPDX expression like \"MIT\")".into());
        }
        if let Err(e) = validate_entry_path(&p.entry) {
            errors.push(e);
        }
        if let Some(d) = &p.description
            && d.chars().count() > 1024
        {
            errors.push("description must be at most 1024 characters".into());
        }

        let mut deps = Dependencies::default();
        let dep_id = |key: &str, what: &str, errors: &mut Vec<String>| -> Option<PluginId> {
            if !is_valid_id_syntax(key) {
                errors.push(format!(
                    "{what} dependency `{key}` is not a valid plugin id"
                ));
                return None;
            }
            if Some(key) == id.as_deref() || key == p.id {
                errors.push(format!(
                    "{what} dependency `{key}`: a plugin cannot depend on itself"
                ));
                return None;
            }
            Some(PluginId::new_unchecked(key))
        };
        for (k, req) in &raw.dependencies.required {
            if let Some(dep) = dep_id(k, "required", &mut errors) {
                match semver::VersionReq::parse(req) {
                    Ok(r) => {
                        deps.required.insert(dep, r);
                    }
                    Err(e) => errors.push(format!(
                        "required dependency `{k}`: bad version requirement `{req}`: {e}"
                    )),
                }
            }
        }
        for (k, req) in &raw.dependencies.optional {
            if raw.dependencies.required.contains_key(k) {
                errors.push(format!(
                    "`{k}` is both a required and an optional dependency"
                ));
                continue;
            }
            if let Some(dep) = dep_id(k, "optional", &mut errors) {
                match semver::VersionReq::parse(req) {
                    Ok(r) => {
                        deps.optional.insert(dep, r);
                    }
                    Err(e) => errors.push(format!(
                        "optional dependency `{k}`: bad version requirement `{req}`: {e}"
                    )),
                }
            }
        }
        for k in &raw.dependencies.load_before {
            if let Some(dep) = dep_id(k, "load_before", &mut errors) {
                if deps.required.contains_key(&dep) || deps.optional.contains_key(&dep) {
                    errors.push(format!(
                        "`{k}` is both a dependency and in load_before (contradictory order)"
                    ));
                } else if !deps.load_before.contains(&dep) {
                    deps.load_before.push(dep);
                }
            }
        }

        let network = match raw.capabilities.network {
            RawNetwork::Flag(false) => NetworkCapability::None,
            RawNetwork::Flag(true) => {
                errors.push(
                    "capabilities.network = true is not allowed: list the domains, e.g. [\"api.example.com\"]"
                        .into(),
                );
                NetworkCapability::None
            }
            RawNetwork::Domains(list) => {
                let mut out = Vec::new();
                for d in list {
                    let d = d.trim().trim_end_matches('.').to_ascii_lowercase();
                    if valid_domain(&d) {
                        if !out.contains(&d) {
                            out.push(d);
                        }
                    } else {
                        errors.push(format!(
                            "capabilities.network: `{d}` is not a host name (no scheme, port or path; `*.` prefix allowed)"
                        ));
                    }
                }
                if out.is_empty() {
                    NetworkCapability::None
                } else {
                    NetworkCapability::Domains(out)
                }
            }
        };

        let limits = raw.limits.unwrap_or_default();
        if limits.fuel_per_call == Some(0) {
            errors.push("limits.fuel_per_call must be positive".into());
        }
        if let Some(m) = limits.memory_mb
            && (m == 0 || m > MAX_MEMORY_MB)
        {
            errors.push(format!("limits.memory_mb must be 1 to {MAX_MEMORY_MB}"));
        }

        match (id, version, api) {
            (Some(id), Some(version), Some(api)) if errors.is_empty() => Ok(Manifest {
                id,
                name,
                version,
                api,
                authors: p.authors,
                license: p.license.trim().to_owned(),
                entry: p.entry,
                description: p.description.filter(|d| !d.trim().is_empty()),
                dependencies: deps,
                capabilities: Capabilities {
                    network,
                    filesystem: raw.capabilities.filesystem,
                },
                limits,
            }),
            _ => Err(ManifestError::Invalid(errors)),
        }
    }

    /// Render as manifest TOML (what [`Manifest::parse`] reads back).
    pub fn to_toml(&self) -> String {
        let raw = RawManifest {
            plugin: RawPlugin {
                id: self.id.to_string(),
                name: self.name.clone(),
                version: self.version.to_string(),
                api: self.api.to_string(),
                authors: self.authors.clone(),
                license: self.license.clone(),
                entry: self.entry.clone(),
                description: self.description.clone(),
            },
            dependencies: RawDependencies {
                required: self
                    .dependencies
                    .required
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
                optional: self
                    .dependencies
                    .optional
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.to_string()))
                    .collect(),
                load_before: self
                    .dependencies
                    .load_before
                    .iter()
                    .map(|d| d.to_string())
                    .collect(),
            },
            capabilities: RawCapabilities {
                network: match &self.capabilities.network {
                    NetworkCapability::None => RawNetwork::Flag(false),
                    NetworkCapability::Domains(d) => RawNetwork::Domains(d.clone()),
                },
                filesystem: self.capabilities.filesystem,
            },
            limits: if self.limits == PluginLimits::default() {
                None
            } else {
                Some(self.limits)
            },
        };
        // Serialising plain structs of strings, maps and integers cannot fail.
        toml::to_string_pretty(&raw).unwrap_or_default()
    }

    /// Every plugin this one references (required, optional, load_before).
    pub fn referenced_plugins(&self) -> impl Iterator<Item = &PluginId> {
        self.dependencies
            .required
            .keys()
            .chain(self.dependencies.optional.keys())
            .chain(self.dependencies.load_before.iter())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) const FULL: &str = r#"
[plugin]
id = "homes"
name = "Homes"
version = "1.2.0"
api = "0.1"
authors = ["Alice", "Bob"]
license = "MIT"
entry = "homes.wasm"
description = "Set and teleport to homes"

[dependencies]
required = { economy = "^1.0" }
optional = { placeholders = "*" }
load_before = ["chat"]

[capabilities]
network = ["api.example.com", "*.Example.org"]
filesystem = "data-folder"

[limits]
fuel_per_call = 50000000
memory_mb = 64
"#;

    #[test]
    fn full_manifest() {
        let m = Manifest::parse(FULL).unwrap();
        assert_eq!(m.id, "homes");
        assert_eq!(m.version, semver::Version::new(1, 2, 0));
        assert_eq!(m.api, ApiVersion { major: 0, minor: 1 });
        assert_eq!(m.dependencies.required.len(), 1);
        assert!(m.dependencies.optional.contains_key("placeholders"));
        assert_eq!(
            m.dependencies.load_before,
            vec![PluginId::new_unchecked("chat")]
        );
        assert_eq!(m.capabilities.filesystem, FilesystemCapability::DataFolder);
        assert!(m.capabilities.network_allows("api.example.com"));
        assert!(m.capabilities.network_allows("cdn.example.org"));
        assert!(!m.capabilities.network_allows("example.org"));
        assert!(!m.capabilities.network_allows("evil-example.org"));
        assert!(!m.capabilities.network_allows("other.com"));
        assert_eq!(m.limits.memory_mb, Some(64));
        // Round trip through the writer.
        let again = Manifest::parse(&m.to_toml()).unwrap();
        assert_eq!(again, m);
    }

    #[test]
    fn minimal_manifest_defaults() {
        let m = Manifest::parse(
            r#"[plugin]
id = "a"
name = "A"
version = "0.1.0-alpha.1"
api = "0.1"
authors = ["x"]
license = "Apache-2.0"
entry = "bin/a.wasm"
"#,
        )
        .unwrap();
        assert_eq!(m.capabilities, Capabilities::default());
        assert!(!m.capabilities.network_allows("example.com"));
        assert_eq!(m.limits, PluginLimits::default());
        assert!(m.description.is_none());
        assert_eq!(Manifest::parse(&m.to_toml()).unwrap(), m);
    }

    fn errors_of(text: &str) -> String {
        match Manifest::parse(text) {
            Ok(_) => panic!("expected an error for:\n{text}"),
            Err(e) => e.to_string(),
        }
    }

    fn with(replace: &str, by: &str) -> String {
        assert!(FULL.contains(replace), "{replace}");
        FULL.replace(replace, by)
    }

    #[test]
    fn rejects_bad_values() {
        assert!(errors_of(&with("id = \"homes\"", "id = \"Homes\"")).contains("[a-z0-9_-]"));
        assert!(errors_of(&with("id = \"homes\"", "id = \"rustic\"")).contains("reserved"));
        assert!(errors_of(&with("version = \"1.2.0\"", "version = \"1.2\"")).contains("semver"));
        assert!(errors_of(&with("api = \"0.1\"", "api = \"0.2\"")).contains("not compatible"));
        assert!(errors_of(&with("api = \"0.1\"", "api = \"0.1.0\"")).contains("MAJOR.MINOR"));
        assert!(errors_of(&with("api = \"0.1\"", "api = \"1.0\"")).contains("not compatible"));
        assert!(
            errors_of(&with("authors = [\"Alice\", \"Bob\"]", "authors = []")).contains("authors")
        );
        assert!(errors_of(&with("license = \"MIT\"", "license = \" \"")).contains("license"));
        assert!(
            errors_of(&with("entry = \"homes.wasm\"", "entry = \"../homes.wasm\""))
                .contains("entry")
        );
        assert!(
            errors_of(&with("entry = \"homes.wasm\"", "entry = \"/abs.wasm\"")).contains("entry")
        );
        assert!(
            errors_of(&with("entry = \"homes.wasm\"", "entry = \"homes.so\"")).contains(".wasm")
        );
        assert!(
            errors_of(&with("economy = \"^1.0\"", "economy = \"not a req\""))
                .contains("version requirement")
        );
        assert!(errors_of(&with("economy = \"^1.0\"", "homes = \"*\"")).contains("itself"));
        assert!(
            errors_of(&with(
                "load_before = [\"chat\"]",
                "load_before = [\"homes\"]"
            ))
            .contains("itself")
        );
        assert!(
            errors_of(&with(
                "load_before = [\"chat\"]",
                "load_before = [\"economy\"]"
            ))
            .contains("contradictory")
        );
        assert!(
            errors_of(&with("placeholders = \"*\"", "economy = \"*\""))
                .contains("both a required and an optional")
        );
        assert!(
            errors_of(&with(
                "network = [\"api.example.com\", \"*.Example.org\"]",
                "network = true"
            ))
            .contains("list the domains")
        );
        assert!(
            errors_of(&with(
                "network = [\"api.example.com\", \"*.Example.org\"]",
                "network = [\"https://x.com/\"]"
            ))
            .contains("host name")
        );
        assert!(
            errors_of(&with(
                "filesystem = \"data-folder\"",
                "filesystem = \"all\""
            ))
            .contains(MANIFEST_FILE)
        );
        assert!(errors_of(&with("memory_mb = 64", "memory_mb = 0")).contains("memory_mb"));
        assert!(
            errors_of(&with("fuel_per_call = 50000000", "fuel_per_call = 0"))
                .contains("fuel_per_call")
        );
        assert!(errors_of(&with("[limits]", "[limits]\nunknown = 1")).contains(MANIFEST_FILE));
        assert!(
            errors_of(&with("name = \"Homes\"", "name = \"Homes\"\nmain = \"x\""))
                .contains("unknown field")
        );
        assert!(errors_of("not toml at all [").contains(MANIFEST_FILE));
    }

    #[test]
    fn collects_every_error() {
        let text = with("id = \"homes\"", "id = \"BAD\"")
            .replace("version = \"1.2.0\"", "version = \"x\"");
        match Manifest::parse(&text) {
            Err(ManifestError::Invalid(list)) => assert!(list.len() >= 2, "{list:?}"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn api_compatibility() {
        let v = |a, b| ApiVersion { major: a, minor: b };
        assert!(v(0, 1).is_compatible_with(v(0, 1)));
        // 0.x window: older minors run on newer hosts, newer plugins never run on older hosts,
        // and minors below the window (0.0) are refused.
        assert!(v(0, 1).is_compatible_with(v(0, 2)));
        assert!(!v(0, 2).is_compatible_with(v(0, 1)));
        assert!(!v(0, 0).is_compatible_with(v(0, 2)));
        assert!(v(1, 0).is_compatible_with(v(1, 3)));
        assert!(!v(1, 4).is_compatible_with(v(1, 3)));
        assert!(!v(1, 0).is_compatible_with(v(2, 0)));
        assert!(ApiVersion::parse("01.1").is_err());
        assert!(ApiVersion::parse("1").is_err());
        assert!(ApiVersion::parse("a.b").is_err());
    }
}
