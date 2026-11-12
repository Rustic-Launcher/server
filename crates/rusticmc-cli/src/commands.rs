//! `rustic build`, `check`, `package`, `info`, `test`, `bridge analyze`.

use crate::component::{self, Report};
use crate::project::{self, Project};
use crate::ui;
use anyhow::{Context, Result, anyhow, bail};
use rusticmc_core::archive::PluginPackage;
use rusticmc_core::manifest::{FilesystemCapability, HOST_API, Manifest, NetworkCapability};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Print a component report; returns (errors, warnings).
pub fn print_report(r: &Report) -> (usize, usize) {
    for e in &r.errors {
        ui::error(e);
    }
    for w in &r.warnings {
        ui::warn(w);
    }
    for n in &r.notes {
        ui::note(n);
    }
    (r.errors.len(), r.warnings.len())
}

/// Build, validate and package; returns (rplugin path, component bytes).
pub fn build(
    path: Option<&Path>,
    release: bool,
    quiet: bool,
) -> Result<(Project, PathBuf, Vec<u8>)> {
    let project = Project::find(path)?;
    let m = &project.manifest;
    ui::status(
        "Building",
        format!(
            "{} v{} ({}, {})",
            m.id,
            m.version,
            project::TARGET,
            if release { "release" } else { "debug" }
        ),
    );
    let wasm_path = project.cargo_build(release, quiet)?;
    let wasm = std::fs::read(&wasm_path)
        .with_context(|| format!("cannot read {}", ui::rel(&wasm_path)))?;
    ui::status("Checking", format!("component {}", ui::rel(&wasm_path)));
    let report = component::inspect(&wasm, m);
    let (errors, _) = print_report(&report);
    if errors > 0 {
        bail!("the component is not a valid Rustic plugin ({errors} error(s))");
    }
    let out = project.rplugin_path();
    let size = project.package(&wasm, &out)?;
    ui::status(
        "Packaged",
        format!(
            "{} ({}, component {})",
            ui::rel(&out),
            ui::size(size),
            ui::size(wasm.len() as u64)
        ),
    );
    Ok((project, out, wasm))
}

/// `rustic package`: a release build, plus the SHA-256 to publish.
pub fn package(path: Option<&Path>, out: Option<&Path>) -> Result<()> {
    let (_, built, _) = build(path, true, false)?;
    let target = match out {
        Some(o) => {
            if let Some(p) = o.parent().filter(|p| !p.as_os_str().is_empty()) {
                std::fs::create_dir_all(p)?;
            }
            std::fs::copy(&built, o)?;
            o.to_path_buf()
        }
        None => built,
    };
    let bytes = std::fs::read(&target)?;
    ui::status("Finished", format!("{} ready to upload", ui::rel(&target)));
    anstream::println!("{}  {}", project::sha256(&bytes), ui::rel(&target));
    Ok(())
}

/// What `check` and `info` look at.
struct Subject {
    label: String,
    manifest: Manifest,
    wasm: Option<Vec<u8>>,
    archive_size: Option<u64>,
    config: Option<String>,
}

fn load_subject(path: Option<&Path>, profile: Option<&'static str>) -> Result<Subject> {
    if let Some(p) = path
        && p.is_file()
    {
        if p.extension().is_some_and(|e| e == "wasm") {
            bail!(
                "{} is a bare component: check its project directory or its .rplugin",
                ui::rel(p)
            );
        }
        let pkg = PluginPackage::open(p).map_err(|e| anyhow!("{e}"))?;
        let wasm = pkg.read_entry().map_err(|e| anyhow!("{e}"))?;
        return Ok(Subject {
            label: ui::rel(p),
            config: pkg.default_config().ok().flatten(),
            manifest: pkg.manifest,
            wasm: Some(wasm),
            archive_size: Some(std::fs::metadata(p)?.len()),
        });
    }
    if let Some(p) = path
        && p.is_dir()
        && !p.join("Cargo.toml").is_file()
    {
        let pkg = PluginPackage::open_dir(p).map_err(|e| anyhow!("{e}"))?;
        return Ok(Subject {
            label: ui::rel(p),
            config: pkg.default_config().ok().flatten(),
            wasm: Some(pkg.read_entry().map_err(|e| anyhow!("{e}"))?),
            manifest: pkg.manifest,
            archive_size: None,
        });
    }
    let project = Project::find(path)?;
    let rplugin = project.rplugin_path();
    let built = project.built_wasm(profile);
    if let Some((_, used)) = &built
        && profile.is_none()
    {
        let other = if *used == "release" {
            "debug"
        } else {
            "release"
        };
        let modified = |p: &Path| p.metadata().and_then(|m| m.modified()).ok();
        if let (Some(this), Some(that)) = (
            built.as_ref().and_then(|(w, _)| modified(w)),
            project.built_wasm_of(other).and_then(|w| modified(&w)),
        ) && that > this
        {
            ui::note(format!(
                "inspecting the {used} build; the {other} build is newer (`rustic check --{other}` inspects it)"
            ));
        }
    }
    if profile.is_some() && built.is_none() {
        bail!(
            "no {} build of {} yet: `rustic build{}` first",
            profile.unwrap_or_default(),
            project.manifest.id,
            if profile == Some("release") {
                " --release"
            } else {
                ""
            }
        );
    }
    let wasm = built.and_then(|(p, _)| std::fs::read(p).ok());
    Ok(Subject {
        label: ui::rel(&project.dir),
        config: project.default_config(),
        archive_size: std::fs::metadata(&rplugin).ok().map(|m| m.len()),
        manifest: project.manifest,
        wasm,
    })
}

/// The capabilities, shown like the prompt an admin sees.
fn print_capabilities(m: &Manifest) {
    use ui::{BOLD, DIM, GREEN, YELLOW};
    let authors = m.authors.join(", ");
    anstream::println!(
        "\n  {BOLD}{}{BOLD:#} {} by {authors} asks to:",
        m.name,
        m.version
    );
    let mut asked = false;
    if m.capabilities.filesystem == FilesystemCapability::DataFolder {
        asked = true;
        anstream::println!(
            "    {YELLOW}•{YELLOW:#} read and write files in its own data folder (plugins/{}/)",
            m.id
        );
    }
    if let NetworkCapability::Domains(d) = &m.capabilities.network {
        asked = true;
        anstream::println!(
            "    {YELLOW}•{YELLOW:#} send HTTP requests to {}",
            d.join(", ")
        );
    }
    if !asked {
        anstream::println!("    {GREEN}•{GREEN:#} nothing beyond the sandbox");
    }
    if let Some(f) = m.limits.fuel_per_call {
        anstream::println!("    {YELLOW}•{YELLOW:#} use up to {f} fuel per call");
    }
    if let Some(mb) = m.limits.memory_mb {
        anstream::println!("    {YELLOW}•{YELLOW:#} use up to {mb} MiB of memory");
    }
    anstream::println!(
        "  {DIM}Always sandboxed: no access to other files, sockets, processes or other plugins' data.{DIM:#}\n"
    );
}

fn print_manifest(s: &Subject) {
    let m = &s.manifest;
    use ui::{BOLD, DIM};
    anstream::println!("{BOLD}{}{BOLD:#} {DIM}({}){DIM:#}", m.id, s.label);
    anstream::println!("  name         {}", m.name);
    anstream::println!("  version      {}", m.version);
    let api_ok = m.api.is_compatible_with(HOST_API);
    anstream::println!(
        "  api          {} {}",
        m.api,
        if api_ok {
            format!(
                "{}(host implements {HOST_API}){}",
                ui::DIM,
                ui::DIM.render_reset()
            )
        } else {
            format!(
                "{}incompatible with {HOST_API}{}",
                ui::RED,
                ui::RED.render_reset()
            )
        }
    );
    anstream::println!("  authors      {}", m.authors.join(", "));
    anstream::println!("  license      {}", m.license);
    anstream::println!("  entry        {}", m.entry);
    if let Some(d) = &m.description {
        anstream::println!("  description  {d}");
    }
    let deps: Vec<String> = m
        .dependencies
        .required
        .iter()
        .map(|(k, v)| format!("{k} {v}"))
        .chain(
            m.dependencies
                .optional
                .iter()
                .map(|(k, v)| format!("{k} {v} (optional)")),
        )
        .collect();
    if !deps.is_empty() {
        anstream::println!("  depends on   {}", deps.join(", "));
    }
    if let Some(w) = &s.wasm {
        anstream::println!(
            "  component    {} (sha256 {})",
            ui::size(w.len() as u64),
            &project::sha256(w)[..16]
        );
    }
    if let Some(a) = s.archive_size {
        anstream::println!("  .rplugin     {}", ui::size(a));
    }
    if s.config.is_some() {
        anstream::println!("  config.toml  shipped");
    }
}

/// `rustic check`: exit code 1 on errors (or warnings with `deny_warnings`).
pub fn check(
    path: Option<&Path>,
    deny_warnings: bool,
    profile: Option<&'static str>,
) -> Result<()> {
    let s = load_subject(path, profile)?;
    print_manifest(&s);
    print_capabilities(&s.manifest);
    let mut errors = 0;
    let mut warnings = 0;
    if !s.manifest.api.is_compatible_with(HOST_API) {
        ui::error(format!(
            "api {} is not supported by this host (API {HOST_API}; 0.x minors must match)",
            s.manifest.api
        ));
        errors += 1;
    }
    if let Some(c) = &s.config
        && let Err(e) = toml::from_str::<toml::Table>(c)
    {
        ui::error(format!("config.toml is not valid TOML: {e}"));
        errors += 1;
    }
    match &s.wasm {
        Some(w) => {
            let r = component::inspect(w, &s.manifest);
            anstream::println!(
                "  imports      {}",
                if r.rustic_imports().is_empty() {
                    "(no rustic:api interface)".to_owned()
                } else {
                    r.rustic_imports().join(", ")
                }
            );
            let (e, wa) = print_report(&r);
            errors += e;
            warnings += wa;
        }
        None => ui::note(
            "not built yet: `rustic build` compiles it, then `rustic check` inspects the component",
        ),
    }
    if errors > 0 {
        bail!("{} has {errors} error(s)", s.manifest.id);
    }
    if deny_warnings && warnings > 0 {
        bail!(
            "{} has {warnings} warning(s) (--deny-warnings)",
            s.manifest.id
        );
    }
    ui::status(
        "Checked",
        format!(
            "{} v{}: {}",
            s.manifest.id,
            s.manifest.version,
            if warnings == 0 {
                "no problem found".to_owned()
            } else {
                format!("{warnings} warning(s)")
            }
        ),
    );
    Ok(())
}

/// `rustic info`: what is in a plugin.
pub fn info(path: Option<&Path>) -> Result<()> {
    let s = load_subject(path, None)?;
    print_manifest(&s);
    print_capabilities(&s.manifest);
    if let Some(w) = &s.wasm {
        let r = component::inspect(w, &s.manifest);
        anstream::println!("  imports:");
        for i in &r.imports {
            anstream::println!(
                "    {}{}",
                i.name,
                if i.functions.is_empty() || !i.name.starts_with("rustic:") {
                    String::new()
                } else {
                    format!("  {}", i.functions.join(" "))
                }
            );
        }
        anstream::println!("  exports:");
        for e in &r.exports {
            anstream::println!("    {} ({} functions)", e.name, e.functions.len());
        }
    }
    Ok(())
}

/// `rustic test`: `cargo test` in the plugin crate.
pub fn test(path: Option<&Path>, args: &[String]) -> Result<()> {
    let project = Project::find(path)?;
    ui::status(
        "Testing",
        format!(
            "{} (unit tests + the real .wasm in rusticmc-test's MockServer)",
            project.manifest.id
        ),
    );
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let status = Command::new(cargo)
        .current_dir(&project.dir)
        .arg("test")
        .arg("--manifest-path")
        .arg(project.dir.join("Cargo.toml"))
        .args(args)
        .status()
        .context("cannot run cargo test")?;
    if !status.success() {
        bail!("tests failed");
    }
    Ok(())
}

/// Options of `rustic bridge analyze`.
pub struct BridgeOptions {
    pub inputs: Vec<PathBuf>,
    pub json: Option<PathBuf>,
    pub markdown: Option<PathBuf>,
    pub coverage: Option<PathBuf>,
    pub panel: Option<PathBuf>,
    pub details: bool,
    pub compact: Option<usize>,
}

/// `rustic bridge analyze`.
pub fn bridge_analyze(o: BridgeOptions) -> Result<()> {
    use rusticmc_bridge_analyze::{
        Coverage, Verdict, analyze_all, render_panel_markdown, render_plugin_markdown,
    };
    let cov = match &o.coverage {
        Some(p) => Coverage::from_toml_str(
            &std::fs::read_to_string(p).with_context(|| format!("cannot read {}", ui::rel(p)))?,
        )
        .map_err(|e| anyhow!("{}: {e}", ui::rel(p)))?,
        None => Coverage::builtin(),
    };
    if let Some(missing) = o.inputs.iter().find(|p| !p.exists()) {
        bail!("{} does not exist", ui::rel(missing));
    }
    let report = analyze_all(&o.inputs, &cov, o.panel.as_deref()).map_err(|e| anyhow!(e))?;
    if report.plugins.is_empty() {
        bail!("no .jar found in the given paths");
    }
    let mut md = if report.plugins.len() == 1 && !o.details {
        render_plugin_markdown(&report.plugins[0])
    } else {
        render_panel_markdown(&report)
    };
    if o.details && report.plugins.len() > 1 {
        md.push_str("## Plugins in detail\n\n");
        for p in &report.plugins {
            md.push_str(&render_plugin_markdown(p));
        }
    }
    if let Some(path) = &o.json {
        let text = match o.compact {
            Some(n) => serde_json::to_string_pretty(&report.compact(n))?,
            None => serde_json::to_string_pretty(&report)?,
        };
        std::fs::write(path, text)?;
        ui::status("Wrote", ui::rel(path));
    }
    match &o.markdown {
        Some(path) => {
            std::fs::write(path, &md)?;
            ui::status("Wrote", ui::rel(path));
        }
        None => anstream::print!("{md}"),
    }
    for p in &report.plugins {
        let (style, word) = match p.verdict {
            Verdict::Green => (ui::GREEN, "green"),
            Verdict::Yellow => (ui::YELLOW, "yellow"),
            Verdict::Red => (ui::RED, "red"),
            Verdict::Error => (ui::RED, "unreadable"),
        };
        anstream::eprintln!("{style}{word:>12}{style:#} {}", p.file);
    }
    let s = &report.summary;
    ui::info(
        "Summary",
        format!(
            "{} jar(s): {} green, {} yellow, {} red, {} unreadable",
            s.total, s.green, s.yellow, s.red, s.error
        ),
    );
    Ok(())
}
