//! Inspecting a plugin component: its imports and exports, checked against the WIT contract
//! the host implements (`rustic:api@0.1.0`, embedded at build time) and the manifest.

use anyhow::{Result, anyhow};
use rusticmc_core::manifest::{ApiVersion, HOST_API, Manifest, NetworkCapability};
use std::collections::{BTreeMap, BTreeSet};
use wit_parser::{Resolve, WorldItem, WorldKey};

const WIT_FILES: [(&str, &str); 6] = [
    (
        "types.wit",
        include_str!("../../rusticmc-api-wit/wit/types.wit"),
    ),
    (
        "events.wit",
        include_str!("../../rusticmc-api-wit/wit/events.wit"),
    ),
    (
        "core.wit",
        include_str!("../../rusticmc-api-wit/wit/core.wit"),
    ),
    (
        "game.wit",
        include_str!("../../rusticmc-api-wit/wit/game.wit"),
    ),
    (
        "plugin.wit",
        include_str!("../../rusticmc-api-wit/wit/plugin.wit"),
    ),
    (
        "deps/services/services.wit",
        include_str!("../../rusticmc-api-wit/wit/deps/services/services.wit"),
    ),
];

/// WASI interfaces the host links (WASI 0.2 from Rust's std).
const WASI_ALLOWED: [&str; 5] = [
    "wasi:cli/",
    "wasi:clocks/",
    "wasi:io/",
    "wasi:filesystem/",
    "wasi:random/",
];

/// Interfaces a provider plugin may export besides `plugin` and `handlers`.
const PROVIDER_EXPORTS: [&str; 3] = [
    "rustic:services/economy",
    "rustic:services/permission-provider",
    "rustic:services/chat-meta",
];

/// The host's contract: interface name without version → (version, function names).
fn host_contract() -> Result<BTreeMap<String, (String, BTreeSet<String>)>> {
    let dir = tempfile::tempdir()?;
    for (name, text) in WIT_FILES {
        let p = dir.path().join(name);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(p, text)?;
    }
    let mut resolve = Resolve::default();
    resolve
        .push_dir(dir.path())
        .map_err(|e| anyhow!("embedded WIT does not parse: {e:#}"))?;
    let mut out = BTreeMap::new();
    for (id, iface) in resolve.interfaces.iter() {
        let Some(full) = resolve.id_of(id) else {
            continue;
        };
        let (name, version) = split_version(&full);
        out.insert(
            name.to_owned(),
            (
                version.to_owned(),
                iface.functions.keys().cloned().collect(),
            ),
        );
    }
    Ok(out)
}

/// `rustic:api/players@0.1.0` → (`rustic:api/players`, `0.1.0`).
pub fn split_version(full: &str) -> (&str, &str) {
    full.split_once('@').unwrap_or((full, ""))
}

/// One imported or exported interface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Iface {
    /// `rustic:api/players@0.1.0`.
    pub name: String,
    /// Functions used (imports) or provided (exports).
    pub functions: Vec<String>,
}

/// What a component contains and what is wrong with it.
#[derive(Debug, Clone, Default)]
pub struct Report {
    pub imports: Vec<Iface>,
    pub exports: Vec<Iface>,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
    pub notes: Vec<String>,
}

impl Report {
    /// The `rustic:` interfaces imported, short names (`players`, `http`...).
    pub fn rustic_imports(&self) -> Vec<String> {
        self.imports
            .iter()
            .filter(|i| i.name.starts_with("rustic:"))
            .map(|i| {
                let (n, _) = split_version(&i.name);
                n.trim_start_matches("rustic:api/").to_owned()
            })
            .collect()
    }

    fn imports_iface(&self, name: &str) -> Option<&Iface> {
        self.imports
            .iter()
            .find(|i| split_version(&i.name).0 == name)
    }
}

fn items(resolve: &Resolve, map: &wit_parser::IndexMap<WorldKey, WorldItem>) -> Vec<Iface> {
    let mut out = Vec::new();
    for (key, item) in map {
        match item {
            WorldItem::Interface { id, .. } => {
                let name = resolve
                    .id_of(*id)
                    .unwrap_or_else(|| resolve.name_world_key(key));
                out.push(Iface {
                    name,
                    functions: resolve.interfaces[*id].functions.keys().cloned().collect(),
                });
            }
            WorldItem::Function(f) => out.push(Iface {
                name: f.name.clone(),
                functions: vec![f.name.clone()],
            }),
            WorldItem::Type { .. } => {}
        }
    }
    out
}

/// Inspect component bytes against the manifest.
pub fn inspect(wasm: &[u8], manifest: &Manifest) -> Report {
    let mut r = Report::default();
    if wasmparser::Parser::is_core_wasm(wasm) {
        r.errors.push(
            "this is a core WebAssembly module, not a component: build for wasm32-wasip2 \
             (`rustic build`)"
                .into(),
        );
        return r;
    }
    if !wasmparser::Parser::is_component(wasm) {
        r.errors.push("not a WebAssembly file".into());
        return r;
    }
    if let Err(e) =
        wasmparser::Validator::new_with_features(wasmparser::WasmFeatures::all()).validate_all(wasm)
    {
        r.errors.push(format!("invalid component: {e}"));
        return r;
    }
    let decoded = match wit_parser::decoding::decode(wasm) {
        Ok(d) => d,
        Err(e) => {
            r.errors
                .push(format!("cannot read the component's types: {e:#}"));
            return r;
        }
    };
    let wit_parser::decoding::DecodedWasm::Component(resolve, world) = decoded else {
        r.errors
            .push("this is a WIT package, not a plugin component".into());
        return r;
    };
    let w = &resolve.worlds[world];
    r.imports = items(&resolve, &w.imports);
    r.exports = items(&resolve, &w.exports);

    let contract = match host_contract() {
        Ok(c) => c,
        Err(e) => {
            r.errors.push(e.to_string());
            return r;
        }
    };
    let api_ok = manifest.api.is_compatible_with(HOST_API);
    if !api_ok {
        r.errors.push(format!(
            "the manifest targets api {} but this host implements {HOST_API}{}",
            manifest.api,
            if manifest.api.major == 0 {
                " (0.x minors must match until the compatibility window covers it)"
            } else {
                ""
            }
        ));
    }
    let check_version = |r: &mut Report, full: &str, kind: &str| {
        let (_, v) = split_version(full);
        let api = v
            .rsplit_once('.')
            .map(|(mm, _)| mm)
            .and_then(|mm| ApiVersion::parse(mm).ok());
        if let Some(api) = api
            && full.starts_with("rustic:api/")
            && api != manifest.api
        {
            r.errors.push(format!(
                "{kind} `{full}` is from API {api} but rustic.plugin.toml says api = \"{}\" \
                 (rebuild against the matching SDK or fix the manifest)",
                manifest.api
            ));
        }
    };
    for imp in r.imports.clone() {
        let (name, _) = split_version(&imp.name);
        if name.starts_with("rustic:") {
            check_version(&mut r, &imp.name, "import");
            match contract.get(name) {
                None => r.errors.push(format!(
                    "imports `{}`, which the host does not provide",
                    imp.name
                )),
                Some((_, funcs)) => {
                    for f in &imp.functions {
                        if !funcs.contains(f) {
                            r.errors.push(format!(
                                "imports `{name}.{f}`, which the host does not provide \
                                 (built against a newer SDK?)"
                            ));
                        }
                    }
                }
            }
        } else if name.starts_with("wasi:") {
            if !WASI_ALLOWED.iter().any(|p| name.starts_with(p)) {
                r.errors.push(format!(
                    "imports `{}`: the sandbox only offers wasi cli/clocks/io/filesystem/random",
                    imp.name
                ));
            }
        } else {
            r.errors.push(format!(
                "imports `{}`, which no Rustic host provides",
                imp.name
            ));
        }
    }
    for want in ["rustic:api/plugin", "rustic:api/handlers"] {
        let found = r
            .exports
            .iter()
            .find(|e| split_version(&e.name).0 == want)
            .cloned();
        match found {
            None => r.errors.push(format!(
                "does not export `{want}`: is the plugin type marked #[rustic::plugin]?"
            )),
            Some(e) => {
                check_version(&mut r, &e.name, "export");
                if let Some((_, funcs)) = contract.get(want) {
                    for f in funcs {
                        if want == "rustic:api/plugin" && !e.functions.contains(f) {
                            r.errors.push(format!("`{want}` lacks `{f}`"));
                        }
                    }
                }
            }
        }
    }
    for e in &r.exports {
        let (name, _) = split_version(&e.name);
        if !["rustic:api/plugin", "rustic:api/handlers"].contains(&name)
            && !PROVIDER_EXPORTS.contains(&name)
        {
            r.warnings
                .push(format!("exports `{}`, which the host never calls", e.name));
        }
    }
    // Capabilities vs imports.
    let http = r.imports_iface("rustic:api/http").is_some();
    match (&manifest.capabilities.network, http) {
        (NetworkCapability::None, true) => r.warnings.push(
            "the component imports `http` but [capabilities] network = false: every fetch \
             will fail with capability-denied (list the hosts it needs)"
                .into(),
        ),
        (NetworkCapability::Domains(d), false) => r.warnings.push(format!(
            "network access to {} is requested but the component never imports `http`: \
             ask only for what the plugin uses",
            d.join(", ")
        )),
        _ => {}
    }
    if manifest.capabilities.filesystem == rusticmc_core::manifest::FilesystemCapability::None
        && r.imports
            .iter()
            .any(|i| i.name.starts_with("wasi:filesystem/preopens"))
    {
        r.notes.push(
            "filesystem = \"none\": std::fs calls (and the SDK's Config) find no files".into(),
        );
    }
    for e in &r.exports {
        let (name, _) = split_version(&e.name);
        if PROVIDER_EXPORTS.contains(&name) {
            r.notes.push(format!("provides the service `{}`", e.name));
        }
    }
    if wasm.len() > 16 * 1024 * 1024 {
        r.warnings.push(format!(
            "the component is {}: build with --release (opt-level \"s\", lto, strip)",
            crate::ui::size(wasm.len() as u64)
        ));
    }
    r
}
