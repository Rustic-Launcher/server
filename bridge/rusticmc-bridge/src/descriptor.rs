//! A Bukkit plugin's `plugin.yml`, as the JVM read it (with the API's own descriptor parser), and
//! what the Rustic core needs from it: an id, a manifest, commands and permissions.

use rusticmc_core::PluginId;
use rusticmc_core::archive::PluginPackage;
use rusticmc_core::manifest::{Dependencies, HOST_API, Manifest};
use rusticmc_core::model::{ArgKind, ArgSpec, CommandSpec, NodeSpec, PermissionDefault, Usage};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Id of the bridge itself in the core (the plugin every Bukkit plugin depends on).
pub const BRIDGE_ID: &str = "bukkit-bridge";
/// Prefix of Bukkit plugin ids.
pub const ID_PREFIX: &str = "bukkit-";

/// One `commands:` entry.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct CommandDesc {
    /// Name.
    pub name: String,
    /// Description.
    pub description: String,
    /// Usage text (`/<command> ...`).
    pub usage: Option<String>,
    /// Aliases.
    pub aliases: Vec<String>,
    /// Permission node.
    pub permission: Option<String>,
}

/// One `permissions:` entry.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct PermissionDesc {
    /// Node.
    pub name: String,
    /// Description.
    pub description: String,
    /// `true`, `false`, `op` or `not_op`.
    pub default: String,
    /// Children and their value.
    pub children: Vec<(String, bool)>,
}

/// A scanned plugin jar.
#[derive(Clone, Debug, Default, Deserialize, PartialEq)]
#[serde(default)]
pub struct Descriptor {
    /// The jar.
    pub file: PathBuf,
    /// Why it cannot be loaded (no plugin.yml, invalid...).
    pub error: Option<String>,
    /// `name`.
    pub name: String,
    /// `version`.
    pub version: String,
    /// `main`.
    pub main: String,
    /// `description`.
    pub description: Option<String>,
    /// `authors` / `author`.
    pub authors: Vec<String>,
    /// `depend`.
    pub depend: Vec<String>,
    /// `softdepend`.
    pub softdepend: Vec<String>,
    /// `loadbefore`.
    pub loadbefore: Vec<String>,
    /// `api-version`.
    pub api_version: Option<String>,
    /// `commands`.
    pub commands: Vec<CommandDesc>,
    /// `permissions`.
    pub permissions: Vec<PermissionDesc>,
}

/// `bukkit-<name>`: lower case, other characters than `[a-z0-9_-]` become `-` (64 at most).
pub fn plugin_id(name: &str) -> String {
    let mut id = String::from(ID_PREFIX);
    for c in name.chars() {
        let c = c.to_ascii_lowercase();
        id.push(
            if c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || c == '-' {
                c
            } else {
                '-'
            },
        );
    }
    id.truncate(64);
    id
}

/// A Bukkit version string as semver: `1.0` → `1.0.0`, `v5.5.71-bukkit` → `5.5.71-bukkit`,
/// anything unparsable → `0.0.0` with the original kept as build metadata when possible.
pub fn semver_of(version: &str) -> semver::Version {
    let v = version.trim().trim_start_matches(['v', 'V']);
    if let Ok(s) = semver::Version::parse(v) {
        return s;
    }
    let (core, rest) = match v.find(|c: char| !(c.is_ascii_digit() || c == '.')) {
        Some(i) => (&v[..i], &v[i..]),
        None => (v, ""),
    };
    let mut nums: Vec<u64> = core
        .split('.')
        .filter(|p| !p.is_empty())
        .take(3)
        .filter_map(|p| p.parse().ok())
        .collect();
    nums.resize(3, 0);
    let pre: String = rest
        .trim_start_matches(['-', '+', '_', '.'])
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '.' })
        .collect::<String>()
        .split('.')
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(".");
    let text = if pre.is_empty() {
        format!("{}.{}.{}", nums[0], nums[1], nums[2])
    } else {
        format!("{}.{}.{}+{pre}", nums[0], nums[1], nums[2])
    };
    semver::Version::parse(&text)
        .unwrap_or_else(|_| semver::Version::new(nums[0], nums[1], nums[2]))
}

fn any_version() -> semver::VersionReq {
    semver::VersionReq::STAR
}

impl Descriptor {
    /// The core id.
    pub fn id(&self) -> String {
        plugin_id(&self.name)
    }

    /// The jar's file name (the package's `entry`, which is how the loader recognises it).
    pub fn entry(&self) -> String {
        self.file
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_else(|| format!("{}.jar", self.name))
    }

    /// The manifest the core validates and resolves dependencies with: the bridge is a required
    /// dependency, `depend` are required, `softdepend` optional, `loadbefore` load-before.
    pub fn manifest(&self) -> Result<Manifest, String> {
        let id = PluginId::new(&self.id()).map_err(|e| e.to_string())?;
        let mut deps = Dependencies::default();
        deps.required
            .insert(PluginId::new_unchecked(BRIDGE_ID), any_version());
        for d in &self.depend {
            let pid = PluginId::new(&plugin_id(d)).map_err(|e| e.to_string())?;
            deps.required.insert(pid, any_version());
        }
        for d in &self.softdepend {
            if let Ok(pid) = PluginId::new(&plugin_id(d)) {
                deps.optional.insert(pid, any_version());
            }
        }
        for d in &self.loadbefore {
            if let Ok(pid) = PluginId::new(&plugin_id(d)) {
                deps.load_before.push(pid);
            }
        }
        let mut name: String = self.name.chars().take(64).collect();
        if name.trim().is_empty() {
            name = id.to_string();
        }
        Ok(Manifest {
            id,
            name,
            version: semver_of(&self.version),
            api: HOST_API,
            authors: self.authors.clone(),
            license: "LicenseRef-Bukkit-plugin".into(),
            entry: self.entry(),
            description: self.description.clone(),
            dependencies: deps,
            capabilities: Default::default(),
            limits: Default::default(),
        })
    }

    /// An in-memory package for the core.
    pub fn package(&self) -> Result<PluginPackage, String> {
        PluginPackage::in_memory(self.manifest()?, Vec::new(), None, BTreeMap::new())
            .map_err(|e| e.to_string())
    }

    /// The Rustic command specs: Bukkit commands take raw arguments, so each gets a bare usage
    /// and a greedy `args` usage whose completions come from the plugin's tab completer.
    pub fn command_specs(&self) -> Vec<CommandSpec> {
        self.commands
            .iter()
            .map(|c| {
                let mut args = ArgSpec::new("args", ArgKind::Greedy);
                args.dynamic_suggestions = true;
                CommandSpec {
                    name: c.name.to_ascii_lowercase(),
                    aliases: c.aliases.iter().map(|a| a.to_ascii_lowercase()).collect(),
                    description: c.description.clone(),
                    permission: c.permission.clone(),
                    players_only: false,
                    usages: vec![
                        Usage {
                            nodes: Vec::new(),
                            permission: None,
                            description: c.description.clone(),
                        },
                        Usage {
                            nodes: vec![NodeSpec::Argument(args)],
                            permission: None,
                            description: c.usage.clone().unwrap_or_default(),
                        },
                    ],
                }
            })
            .collect()
    }
}

/// A Bukkit permission default as Rustic's.
pub fn permission_default(s: &str) -> PermissionDefault {
    match s.to_ascii_lowercase().replace([' ', '-'], "_").as_str() {
        "true" => PermissionDefault::Allow,
        "false" => PermissionDefault::Deny,
        "not_op" => PermissionDefault::NotOp,
        _ => PermissionDefault::Op,
    }
}

/// The bridge's own package (its instance owns the JVM and the tick pulse).
pub fn bridge_package() -> PluginPackage {
    let manifest = Manifest {
        id: PluginId::new_unchecked(BRIDGE_ID),
        name: "Rustic Bridge".into(),
        version: semver_of(env!("CARGO_PKG_VERSION")),
        api: HOST_API,
        authors: vec!["Rustic".into()],
        license: "MIT OR Apache-2.0 (bridge runtime); GPL-3.0-only (Java layer)".into(),
        entry: "rustic-bridge.jar".into(),
        description: Some("Runs Bukkit/Paper plugins in a supervised JVM".into()),
        dependencies: Dependencies::default(),
        capabilities: Default::default(),
        limits: Default::default(),
    };
    PluginPackage::in_memory(manifest, Vec::new(), None, BTreeMap::new())
        .expect("the bridge package is valid")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_and_versions() {
        assert_eq!(plugin_id("Welcome"), "bukkit-welcome");
        assert_eq!(plugin_id("Multiverse-Core"), "bukkit-multiverse-core");
        assert_eq!(plugin_id("My Plugin.v2"), "bukkit-my-plugin-v2");
        assert_eq!(semver_of("1.0").to_string(), "1.0.0");
        assert_eq!(semver_of("v5.5.71-bukkit").to_string(), "5.5.71-bukkit");
        assert_eq!(semver_of("2.12.1-b1298").to_string(), "2.12.1-b1298");
        assert_eq!(semver_of("5.8+build.3638").to_string(), "5.8.0+build.3638");
        assert_eq!(
            semver_of("bukkit-2.6.25").to_string(),
            "0.0.0+bukkit.2.6.25"
        );
        assert_eq!(semver_of("").to_string(), "0.0.0");
        assert_eq!(permission_default("not op"), PermissionDefault::NotOp);
        assert_eq!(permission_default("true"), PermissionDefault::Allow);
    }

    #[test]
    fn manifest_and_commands() {
        let d: Descriptor = serde_json::from_value(serde_json::json!({
            "file": "/srv/plugins/bukkit/Welcome.jar", "name": "Welcome", "version": "1.2",
            "main": "x.Y", "authors": ["A"], "depend": ["Vault"], "softdepend": ["PlaceholderAPI"],
            "commands": [{"name": "welcome", "description": "d", "usage": "/<command> [reload]",
                          "aliases": ["wel"], "permission": "welcome.use"}],
            "permissions": [{"name": "welcome.use", "description": "", "default": "true", "children": []}]
        }))
        .unwrap();
        let m = d.manifest().unwrap();
        assert_eq!(m.id, "bukkit-welcome");
        assert_eq!(m.entry, "Welcome.jar");
        assert!(m.dependencies.required.contains_key("bukkit-bridge"));
        assert!(m.dependencies.required.contains_key("bukkit-vault"));
        assert!(
            m.dependencies
                .optional
                .contains_key("bukkit-placeholderapi")
        );
        let specs = d.command_specs();
        assert_eq!(specs[0].name, "welcome");
        assert_eq!(specs[0].aliases, ["wel"]);
        assert_eq!(specs[0].usages.len(), 2);
        d.package().unwrap();
        assert_eq!(bridge_package().manifest.id, BRIDGE_ID);
    }
}
