//! Building a plugin crate to a `wasm32-wasip2` component from a test.

use rusticmc_core::archive::{PluginPackage, RpluginBuilder};
use rusticmc_core::manifest::{MANIFEST_FILE, Manifest};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock, PoisonError};

/// The wasm target plugins are built for.
pub const WASM_TARGET: &str = "wasm32-wasip2";

/// A plugin crate built to a component, with its manifest and default config.
#[derive(Clone, Debug)]
pub struct BuiltPlugin {
    /// The plugin crate directory.
    pub dir: PathBuf,
    /// The built component.
    pub wasm: PathBuf,
    /// The validated `rustic.plugin.toml`.
    pub manifest: Manifest,
    /// `config.toml` next to the manifest, if any.
    pub config: Option<String>,
}

impl BuiltPlugin {
    /// The plugin id.
    pub fn id(&self) -> &str {
        self.manifest.id.as_str()
    }

    /// The component bytes.
    pub fn wasm_bytes(&self) -> Vec<u8> {
        std::fs::read(&self.wasm)
            .unwrap_or_else(|e| panic!("cannot read {}: {e}", self.wasm.display()))
    }

    /// An in-memory package (manifest, component, default config, `resources/`) as the server
    /// would read it from a `.rplugin`.
    pub fn package(&self) -> PluginPackage {
        PluginPackage::in_memory(
            self.manifest.clone(),
            self.wasm_bytes(),
            self.config.clone(),
            read_resources(&self.dir.join("resources")),
        )
        .unwrap_or_else(|e| panic!("cannot package {}: {e}", self.dir.display()))
    }

    /// The `.rplugin` archive bytes.
    pub fn rplugin(&self) -> Vec<u8> {
        let mut b = RpluginBuilder::new(self.manifest.clone(), self.wasm_bytes());
        if let Some(c) = &self.config {
            b = b
                .default_config(c.clone())
                .expect("config.toml is valid TOML");
        }
        for (name, bytes) in read_resources(&self.dir.join("resources")) {
            b = b.resource(&name, bytes).expect("resource name");
        }
        b.to_bytes().expect("archive")
    }
}

fn read_resources(root: &Path) -> BTreeMap<String, Vec<u8>> {
    fn walk(root: &Path, dir: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
        let Ok(rd) = std::fs::read_dir(dir) else {
            return;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(root, &p, out);
            } else if let (Ok(rel), Ok(bytes)) = (p.strip_prefix(root), std::fs::read(&p)) {
                let name = rel
                    .components()
                    .map(|c| c.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<_>>()
                    .join("/");
                out.insert(name, bytes);
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(root, root, &mut out);
    out
}

/// Canonical path without the Windows verbatim prefix (`\\?\`), which linkers refuse.
fn clean_path(p: &Path) -> PathBuf {
    let c = p.canonicalize().unwrap_or_else(|_| p.to_path_buf());
    match c.to_str().and_then(|s| s.strip_prefix(r"\\?\")) {
        Some(rest) if !rest.starts_with("UNC") => PathBuf::from(rest),
        _ => c,
    }
}

/// Find `path`: as given (relative to the current directory, which `cargo test` sets to the
/// package being tested), else relative to each ancestor of the current directory, so
/// `"plugins/hello"` works from any crate of the repository.
pub fn locate(path: impl AsRef<Path>) -> Option<PathBuf> {
    let path = path.as_ref();
    if path.is_absolute() || path.exists() {
        return path.exists().then(|| path.to_path_buf());
    }
    let cwd = std::env::current_dir().ok()?;
    cwd.ancestors().map(|a| a.join(path)).find(|p| p.exists())
}

/// Build a plugin crate (a directory with `Cargo.toml` and `rustic.plugin.toml`; without a
/// manifest one is made from the package name, as for test fixtures) for
/// `wasm32-wasip2` and return the component. Builds are cached per process (and by cargo
/// across runs), so every test can call it.
///
/// The release profile is used (what ships); set `RUSTIC_TEST_PROFILE=dev` for faster debug
/// builds. Panics with cargo's output when the build fails: this is meant for tests.
///
/// ```no_run
/// let hello = rusticmc_test::build_plugin("plugins/hello");
/// let server = rusticmc_test::MockServer::with_built(&hello);
/// ```
pub fn build_plugin(path: impl AsRef<Path>) -> BuiltPlugin {
    try_build_plugin(path).unwrap_or_else(|e| panic!("{e}"))
}

/// [`build_plugin`] returning the error instead of panicking.
pub fn try_build_plugin(path: impl AsRef<Path>) -> Result<BuiltPlugin, String> {
    let given = path.as_ref();
    let dir =
        locate(given).ok_or_else(|| format!("plugin directory `{}` not found", given.display()))?;
    let dir = clean_path(&dir);
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, BuiltPlugin>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    let mut guard = cache.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(b) = guard.get(&dir) {
        return Ok(b.clone());
    }
    let built = build_uncached(&dir)?;
    guard.insert(dir, built.clone());
    Ok(built)
}

fn build_uncached(dir: &Path) -> Result<BuiltPlugin, String> {
    let cargo_toml = dir.join("Cargo.toml");
    if !cargo_toml.is_file() {
        return Err(format!("{}: no Cargo.toml", dir.display()));
    }
    let manifest_path = dir.join(MANIFEST_FILE);
    let manifest = if manifest_path.is_file() {
        let text = std::fs::read_to_string(&manifest_path)
            .map_err(|e| format!("{}: {e}", manifest_path.display()))?;
        Manifest::parse(&text).map_err(|e| format!("{}: {e}", dir.display()))?
    } else {
        synthesized_manifest(&cargo_toml)?
    };
    let profile = std::env::var("RUSTIC_TEST_PROFILE").unwrap_or_else(|_| "release".into());
    let wasm = cargo_build(&cargo_toml, &profile)?;
    let config = std::fs::read_to_string(dir.join("config.toml")).ok();
    Ok(BuiltPlugin {
        dir: dir.to_path_buf(),
        wasm,
        manifest,
        config,
    })
}

/// Read a plugin from disk in any of its forms: a plugin crate (directory with `Cargo.toml`
/// and `rustic.plugin.toml`: built with [`build_plugin`]), a dev directory (manifest + entry),
/// a `.rplugin` archive, or a `.wasm` file with `rustic.plugin.toml` next to it or in a parent
/// directory (the crate of a `target/.../x.wasm`).
pub fn package_from_path(path: &Path) -> Result<PluginPackage, String> {
    let p = locate(path).ok_or_else(|| format!("`{}` not found", path.display()))?;
    if p.is_dir() {
        if p.join("Cargo.toml").is_file() {
            let entry_present = std::fs::read_to_string(p.join(MANIFEST_FILE))
                .ok()
                .and_then(|t| Manifest::parse(&t).ok())
                .is_some_and(|m| p.join(&m.entry).is_file());
            if !entry_present {
                return Ok(try_build_plugin(&p)?.package());
            }
        }
        return PluginPackage::open_dir(&p).map_err(|e| e.to_string());
    }
    if p.extension().is_some_and(|e| e == "wasm") {
        let manifest_dir = p
            .ancestors()
            .skip(1)
            .find(|a| a.join(MANIFEST_FILE).is_file())
            .ok_or_else(|| {
                format!(
                    "no {MANIFEST_FILE} next to {} or in a parent directory",
                    p.display()
                )
            })?;
        let text =
            std::fs::read_to_string(manifest_dir.join(MANIFEST_FILE)).map_err(|e| e.to_string())?;
        let manifest = Manifest::parse(&text).map_err(|e| e.to_string())?;
        let bytes = std::fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
        let config = std::fs::read_to_string(manifest_dir.join("config.toml")).ok();
        return PluginPackage::in_memory(
            manifest,
            bytes,
            config,
            read_resources(&manifest_dir.join("resources")),
        )
        .map_err(|e| e.to_string());
    }
    PluginPackage::open(&p).map_err(|e| e.to_string())
}

/// A manifest for a crate without `rustic.plugin.toml` (test fixtures): id = package name,
/// entry = the library's `.wasm`, data folder granted.
fn synthesized_manifest(cargo_toml: &Path) -> Result<Manifest, String> {
    let text = std::fs::read_to_string(cargo_toml)
        .map_err(|e| format!("{}: {e}", cargo_toml.display()))?;
    let table: toml::Table =
        toml::from_str(&text).map_err(|e| format!("{}: {e}", cargo_toml.display()))?;
    let name = table
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(|n| n.as_str())
        .ok_or_else(|| format!("{}: no package name", cargo_toml.display()))?;
    let lib = table
        .get("lib")
        .and_then(|l| l.get("name"))
        .and_then(|n| n.as_str())
        .map(str::to_owned)
        .unwrap_or_else(|| name.replace('-', "_"));
    Manifest::parse(&format!(
        r#"[plugin]
id = "{name}"
name = "{name}"
version = "0.1.0"
api = "0.1"
authors = ["test"]
license = "MIT"
entry = "{lib}.wasm"

[capabilities]
filesystem = "data-folder"
"#
    ))
    .map_err(|e| {
        format!(
            "{}: cannot make a manifest for `{name}`: {e}",
            cargo_toml.display()
        )
    })
}

/// Run `cargo build --target wasm32-wasip2` on a manifest and return the `.wasm` it produced.
pub(crate) fn cargo_build(cargo_toml: &Path, profile: &str) -> Result<PathBuf, String> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut cmd = Command::new(cargo);
    cmd.arg("build")
        .arg("--manifest-path")
        .arg(cargo_toml)
        .arg("--target")
        .arg(WASM_TARGET)
        .arg("--profile")
        .arg(profile)
        .arg("--message-format=json-render-diagnostics")
        // The test's own flags (coverage, sanitizers) are not meant for the wasm guest.
        .env_remove("RUSTFLAGS")
        .env_remove("CARGO_ENCODED_RUSTFLAGS")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let out = cmd.output().map_err(|e| format!("cannot run cargo: {e}"))?;
    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    if !out.status.success() {
        let hint =
            if stderr.contains("wasm32-wasip2") && stderr.contains("target may not be installed") {
                "\nhint: rustup target add wasm32-wasip2"
            } else {
                ""
            };
        return Err(format!(
            "building {} for {WASM_TARGET} failed:\n{stderr}{hint}",
            cargo_toml.display()
        ));
    }
    let mut wasm = None;
    for line in stdout.lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if v["reason"] != "compiler-artifact" {
            continue;
        }
        let is_cdylib = v["target"]["kind"]
            .as_array()
            .is_some_and(|k| k.iter().any(|x| x == "cdylib"));
        if !is_cdylib {
            continue;
        }
        if let Some(files) = v["filenames"].as_array() {
            for f in files.iter().filter_map(|f| f.as_str()) {
                if f.ends_with(".wasm") {
                    wasm = Some(PathBuf::from(f));
                }
            }
        }
    }
    wasm.ok_or_else(|| {
        format!(
            "cargo built {} but produced no .wasm (is the crate a `cdylib`?)",
            cargo_toml.display()
        )
    })
}
