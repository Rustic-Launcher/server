//! A plugin project on disk: the crate, its manifest, its cargo target directory, building it.

use crate::ui;
use anyhow::{Context, Result, anyhow, bail};
use rusticmc_core::archive::RpluginBuilder;
use rusticmc_core::manifest::{MANIFEST_FILE, Manifest, ManifestError};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// The wasm target.
pub const TARGET: &str = "wasm32-wasip2";

/// A plugin crate: `Cargo.toml` + `rustic.plugin.toml`.
#[derive(Debug, Clone)]
pub struct Project {
    pub dir: PathBuf,
    pub manifest: Manifest,
    pub target_dir: PathBuf,
    pub package_name: String,
}

/// Explain a manifest error, one problem per line.
pub fn manifest_error(path: &Path, e: &ManifestError) -> anyhow::Error {
    match e {
        ManifestError::Invalid(list) => {
            anyhow!("{} is invalid:\n  - {}", ui::rel(path), list.join("\n  - "))
        }
        ManifestError::Syntax(m) => anyhow!("{} cannot be read: {m}", ui::rel(path)),
    }
}

/// Parse a manifest file strictly.
pub fn read_manifest(path: &Path) -> Result<Manifest> {
    let text =
        std::fs::read_to_string(path).with_context(|| format!("cannot read {}", ui::rel(path)))?;
    Manifest::parse(&text).map_err(|e| manifest_error(path, &e))
}

fn cargo() -> std::ffi::OsString {
    std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into())
}

impl Project {
    /// The project containing `path` (or the current directory): the closest ancestor with
    /// both `Cargo.toml` and `rustic.plugin.toml`.
    pub fn find(path: Option<&Path>) -> Result<Project> {
        let start = match path {
            Some(p) => p.to_path_buf(),
            None => std::env::current_dir()?,
        };
        let start = if start.is_file() {
            start.parent().map(Path::to_path_buf).unwrap_or_default()
        } else {
            start
        };
        // Absolute, so cargo can run from the project directory (`current_dir`) with the same paths.
        let start = std::path::absolute(&start)?;
        let dir = start
            .ancestors()
            .find(|d| d.join(MANIFEST_FILE).is_file() && d.join("Cargo.toml").is_file())
            .ok_or_else(|| {
                anyhow!(
                    "no Rustic plugin project at {} (a directory with Cargo.toml and {MANIFEST_FILE}); \
                     create one with `rustic new <id>`",
                    ui::rel(&start)
                )
            })?
            .to_path_buf();
        let manifest = read_manifest(&dir.join(MANIFEST_FILE))?;
        let (target_dir, package_name) = metadata(&dir)?;
        Ok(Project {
            dir,
            manifest,
            target_dir,
            package_name,
        })
    }

    /// `target/rustic/<id>.rplugin`.
    pub fn rplugin_path(&self) -> PathBuf {
        self.target_dir
            .join("rustic")
            .join(format!("{}.rplugin", self.manifest.id))
    }

    /// The built component of `profile` (`"release"` or `"debug"`), if any.
    pub fn built_wasm_of(&self, profile: &str) -> Option<PathBuf> {
        let lib = self.package_name.replace('-', "_");
        let p = self
            .target_dir
            .join(TARGET)
            .join(profile)
            .join(format!("{lib}.wasm"));
        p.is_file().then_some(p)
    }

    /// The component `rustic check`/`info` inspect: `profile`'s when given, else the release
    /// build when there is one (what `rustic package` ships), else the debug build. Returns the
    /// profile used too.
    pub fn built_wasm(&self, profile: Option<&'static str>) -> Option<(PathBuf, &'static str)> {
        match profile {
            Some(p) => self.built_wasm_of(p).map(|w| (w, p)),
            None => ["release", "debug"]
                .into_iter()
                .find_map(|p| self.built_wasm_of(p).map(|w| (w, p))),
        }
    }

    /// `cargo build --target wasm32-wasip2`, cargo's progress shown; returns the `.wasm`.
    pub fn cargo_build(&self, release: bool, quiet: bool) -> Result<PathBuf> {
        check_target_installed()?;
        let mut cmd = Command::new(cargo());
        // Run from the project so its `.cargo/config.toml` (target dir, flags) applies whatever the
        // caller's cwd (ADR 0032).
        cmd.current_dir(&self.dir)
            .arg("build")
            .arg("--manifest-path")
            .arg(self.dir.join("Cargo.toml"))
            .args([
                "--target",
                TARGET,
                "--message-format=json-render-diagnostics",
            ])
            .stdout(Stdio::piped())
            .stderr(if quiet {
                Stdio::piped()
            } else {
                Stdio::inherit()
            });
        if release {
            cmd.arg("--release");
        }
        let out = cmd.output().context("cannot run cargo")?;
        if !out.status.success() {
            if quiet {
                eprint!("{}", String::from_utf8_lossy(&out.stderr));
            }
            bail!("cargo build failed for {}", self.manifest.id);
        }
        let stdout = String::from_utf8_lossy(&out.stdout);
        let mut wasm = None;
        for line in stdout.lines() {
            let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            if v["reason"] != "compiler-artifact"
                || v["package_id"]
                    .as_str()
                    .is_none_or(|p| !p.contains(&self.package_name))
            {
                continue;
            }
            for f in v["filenames"].as_array().into_iter().flatten() {
                if let Some(f) = f.as_str().filter(|f| f.ends_with(".wasm")) {
                    wasm = Some(PathBuf::from(f));
                }
            }
        }
        wasm.ok_or_else(|| {
            anyhow!(
                "cargo produced no .wasm for `{}`: is `[lib] crate-type = [\"cdylib\"]` set in Cargo.toml?",
                self.package_name
            )
        })
    }

    /// `config.toml` next to the manifest.
    pub fn default_config(&self) -> Option<String> {
        std::fs::read_to_string(self.dir.join("config.toml")).ok()
    }

    /// Package `wasm` with the manifest, config and `resources/` into `out`.
    pub fn package(&self, wasm: &[u8], out: &Path) -> Result<u64> {
        let mut b = RpluginBuilder::new(self.manifest.clone(), wasm.to_vec());
        if let Some(c) = self.default_config() {
            b = b
                .default_config(c)
                .map_err(|e| anyhow!("config.toml: {e}"))?;
        }
        for (name, bytes) in read_resources(&self.dir.join("resources")) {
            b = b.resource(&name, bytes).map_err(|e| anyhow!("{e}"))?;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent)?;
        }
        b.write_to(out).map_err(|e| anyhow!("{e}"))?;
        Ok(std::fs::metadata(out)?.len())
    }

    /// An unpacked dev directory (`<target>/rustic/dev/<id>-dev/`) the REPL and dev server
    /// load, so `/rustic reload` re-reads it.
    pub fn write_dev_dir(&self, wasm: &[u8], root: &Path) -> Result<PathBuf> {
        let dir = root.join(format!("{}-dev", self.manifest.id));
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join(MANIFEST_FILE), self.manifest.to_toml())?;
        let entry = dir.join(&self.manifest.entry);
        if let Some(p) = entry.parent() {
            std::fs::create_dir_all(p)?;
        }
        std::fs::write(entry, wasm)?;
        if let Some(c) = self.default_config() {
            std::fs::write(dir.join("config.toml"), c)?;
        }
        for (name, bytes) in read_resources(&self.dir.join("resources")) {
            let p = dir.join("resources").join(&name);
            if let Some(parent) = p.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(p, bytes)?;
        }
        Ok(dir)
    }
}

/// (target directory, package name) from `cargo metadata`.
fn metadata(dir: &Path) -> Result<(PathBuf, String)> {
    let out = Command::new(cargo())
        .current_dir(dir)
        .args([
            "metadata",
            "--format-version",
            "1",
            "--no-deps",
            "--manifest-path",
        ])
        .arg(dir.join("Cargo.toml"))
        .stderr(Stdio::piped())
        .output()
        .context("cannot run cargo metadata")?;
    if !out.status.success() {
        bail!(
            "cargo metadata failed:\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout)?;
    let target = v["target_directory"]
        .as_str()
        .map(PathBuf::from)
        .ok_or_else(|| anyhow!("cargo metadata: no target directory"))?;
    let manifest = dir.join("Cargo.toml").canonicalize()?;
    let name = v["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|p| {
            p["manifest_path"]
                .as_str()
                .and_then(|m| Path::new(m).canonicalize().ok())
                .is_some_and(|m| m == manifest)
        })
        .and_then(|p| p["name"].as_str())
        .ok_or_else(|| anyhow!("cargo metadata: package of {} not found", ui::rel(dir)))?;
    Ok((target, name.to_owned()))
}

/// Fail early with the fix when `wasm32-wasip2` is missing (only checked through rustup).
fn check_target_installed() -> Result<()> {
    let Ok(out) = Command::new("rustup")
        .args(["target", "list", "--installed"])
        .stderr(Stdio::null())
        .output()
    else {
        return Ok(()); // no rustup: let cargo report it
    };
    if out.status.success() && !String::from_utf8_lossy(&out.stdout).contains(TARGET) {
        bail!("the {TARGET} target is not installed\n  fix: rustup target add {TARGET}");
    }
    Ok(())
}

/// Files under `resources/`, by forward-slash relative name.
pub fn read_resources(root: &Path) -> BTreeMap<String, Vec<u8>> {
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

/// SHA-256 as hex.
pub fn sha256(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
