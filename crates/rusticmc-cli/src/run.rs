//! `rustic run`: try a plugin. With a `rustic-server` binary: a dev server with the plugin in
//! `plugins/`; without: a headless MockServer REPL. `--watch` rebuilds on save and hot-reloads.

use crate::commands;
use crate::project::Project;
use crate::ui;
use anyhow::{Context, Result, anyhow, bail};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

/// Options of `rustic run`.
pub struct RunOptions {
    pub path: Option<PathBuf>,
    pub server: Option<PathBuf>,
    pub server_bin: Option<PathBuf>,
    pub repl: bool,
    pub watch: bool,
    pub release: bool,
    pub accept_eula: bool,
    pub server_args: Vec<String>,
}

/// The dev server's `server.properties` when its folder has none: offline (bots and offline
/// clients join), on this machine only, a flat world, flying allowed (test bots fly).
pub const DEV_SERVER_PROPERTIES: &str = "# Written by `rustic run` for a local dev server: offline mode, reachable from this machine only.
online-mode=false
server-ip=127.0.0.1
server-port=25570
level-name=world
level-type=minecraft\\:flat
allow-flight=true
spawn-protection=0
view-distance=8
simulation-distance=8
motd=Rustic dev server
";

const EULA_URL: &str = "https://aka.ms/MinecraftEULA";

/// Gets the dev server folder ready: a default `server.properties` when it has none, and the
/// EULA, which is never accepted silently: `eula.txt` already says `eula=true`, or the user passed
/// `--accept-eula`, or answers `y` when asked on a terminal. Otherwise an error says what to do.
pub fn prepare_dev_server(
    dir: &Path,
    accept_eula: bool,
    ask: impl FnOnce() -> Option<bool>,
) -> Result<()> {
    std::fs::create_dir_all(dir)?;
    let properties = dir.join("server.properties");
    if !properties.exists() {
        std::fs::write(&properties, DEV_SERVER_PROPERTIES)?;
        ui::note(format!(
            "wrote a dev {} (offline, 127.0.0.1)",
            ui::rel(&properties)
        ));
    }
    let eula = dir.join("eula.txt");
    let accepted = std::fs::read_to_string(&eula)
        .map(|t| {
            t.lines()
                .any(|l| l.trim().eq_ignore_ascii_case("eula=true"))
        })
        .unwrap_or(false);
    if accepted {
        return Ok(());
    }
    if !(accept_eula || ask() == Some(true)) {
        bail!(
            "the dev server needs the Minecraft EULA ({EULA_URL}) accepted: rerun with \
             --accept-eula, answer `y` when asked, or write `eula=true` in {}",
            ui::rel(&eula)
        );
    }
    std::fs::write(
        &eula,
        format!("# Accepted through `rustic run` ({EULA_URL})\neula=true\n"),
    )?;
    Ok(())
}

/// Asks on the terminal; None when stdin is not one (no answer: not accepted).
fn ask_eula() -> Option<bool> {
    use std::io::IsTerminal;
    if !std::io::stdin().is_terminal() {
        return None;
    }
    print!("Do you accept the Minecraft EULA {EULA_URL}? [y/N] ");
    std::io::stdout().flush().ok()?;
    let mut answer = String::new();
    std::io::stdin().lock().read_line(&mut answer).ok()?;
    Some(matches!(answer.trim(), "y" | "Y" | "yes" | "Yes"))
}

fn exe(name: &str) -> String {
    format!("{name}{}", std::env::consts::EXE_SUFFIX)
}

/// Where a `rustic-server` binary may be: `--server-bin`, `RUSTIC_SERVER_BIN`, the build
/// outputs of `crates/rusticmc-server` in the checkout this CLI comes from, then `PATH`.
pub fn find_server(explicit: Option<&Path>) -> Option<PathBuf> {
    if let Some(p) = explicit {
        return Some(p.to_path_buf());
    }
    if let Some(p) = std::env::var_os("RUSTIC_SERVER_BIN") {
        return Some(PathBuf::from(p));
    }
    let name = exe("rustic-server");
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut candidates = Vec::new();
    for target in ["horizon/target", "crates/rusticmc-server/target", "target"] {
        for profile in ["fast", "release", "debug"] {
            candidates.push(repo.join(target).join(profile).join(&name));
        }
    }
    if let Some(found) = candidates
        .into_iter()
        .filter(|p| p.is_file())
        .max_by_key(|p| p.metadata().and_then(|m| m.modified()).ok())
    {
        return Some(found);
    }
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|d| d.join(&name))
            .find(|p| p.is_file())
    })
}

/// File changes of `src/`, `Cargo.toml`, `rustic.plugin.toml`, `config.toml`, `resources/`.
fn watch(project: &Project, tx: mpsc::Sender<Event>) -> Result<notify::RecommendedWatcher> {
    use notify::{RecursiveMode, Watcher};
    let target = project.target_dir.clone();
    let mut w = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(e) = res
            && matches!(
                e.kind,
                notify::EventKind::Modify(_)
                    | notify::EventKind::Create(_)
                    | notify::EventKind::Remove(_)
            )
            && e.paths.iter().any(|p| !p.starts_with(&target))
        {
            let _ = tx.send(Event::Changed);
        }
    })?;
    for p in ["src", "resources"] {
        let d = project.dir.join(p);
        if d.is_dir() {
            w.watch(&d, RecursiveMode::Recursive)?;
        }
    }
    for f in ["Cargo.toml", "rustic.plugin.toml", "config.toml"] {
        let p = project.dir.join(f);
        if p.is_file() {
            w.watch(&p, RecursiveMode::NonRecursive)?;
        }
    }
    Ok(w)
}

enum Event {
    Line(String),
    Eof,
    Changed,
}

fn stdin_lines(tx: mpsc::Sender<Event>) {
    std::thread::spawn(move || {
        let stdin = std::io::stdin();
        for line in stdin.lock().lines() {
            match line {
                Ok(l) => {
                    if tx.send(Event::Line(l)).is_err() {
                        return;
                    }
                }
                Err(_) => break,
            }
        }
        let _ = tx.send(Event::Eof);
    });
}

/// Wait for the next event, coalescing bursts of file changes.
fn next_event(rx: &mpsc::Receiver<Event>) -> Option<Event> {
    let e = rx.recv().ok()?;
    if matches!(e, Event::Changed) {
        while let Ok(more) = rx.recv_timeout(Duration::from_millis(300)) {
            if !matches!(more, Event::Changed) {
                // Keep the order roughly: a line typed during a save runs after the rebuild.
                return Some(more);
            }
        }
    }
    Some(e)
}

/// Rebuild into the dev directory; `Ok(false)` when the build failed (already reported).
fn rebuild(o: &RunOptions, dev_root: &Path) -> Result<Option<(Project, PathBuf)>> {
    match commands::build(o.path.as_deref(), o.release, false) {
        Ok((project, _, wasm)) => {
            let dir = project.write_dev_dir(&wasm, dev_root)?;
            Ok(Some((project, dir)))
        }
        Err(e) => {
            ui::error(format!("{e:#}"));
            Ok(None)
        }
    }
}

/// `rustic run`.
pub fn run(o: RunOptions) -> Result<()> {
    let project = Project::find(o.path.as_deref())?;
    let server_bin = if o.repl {
        None
    } else {
        find_server(o.server_bin.as_deref())
    };
    match server_bin {
        Some(bin) => run_server(&o, &project, &bin),
        None => {
            if !o.repl {
                ui::note(
                    "no rustic-server binary found (set RUSTIC_SERVER_BIN, pass --server-bin, or build \
                     crates/rusticmc-server); starting the headless REPL instead",
                );
            }
            run_repl(&o, &project)
        }
    }
}

fn run_server(o: &RunOptions, project: &Project, bin: &Path) -> Result<()> {
    let server_dir = o
        .server
        .clone()
        .unwrap_or_else(|| project.target_dir.join("rustic").join("dev-server"));
    prepare_dev_server(&server_dir, o.accept_eula, ask_eula)?;
    let plugins = server_dir.join("plugins");
    std::fs::create_dir_all(&plugins)?;
    let Some((project, _)) = rebuild(o, &plugins)? else {
        bail!("the first build failed");
    };
    ui::status(
        "Running",
        format!("{} in {}", ui::rel(bin), ui::rel(&server_dir)),
    );
    let mut child = Command::new(bin)
        .args(&o.server_args)
        .current_dir(&server_dir)
        .stdin(Stdio::piped())
        .spawn()
        .with_context(|| format!("cannot start {}", ui::rel(bin)))?;
    let mut stdin = child.stdin.take().ok_or_else(|| anyhow!("no stdin"))?;
    let (tx, rx) = mpsc::channel();
    stdin_lines(tx.clone());
    let _watcher = if o.watch {
        Some(watch(&project, tx)?)
    } else {
        None
    };
    loop {
        if let Some(status) = child.try_wait()? {
            ui::info("Stopped", format!("server exited ({status})"));
            return Ok(());
        }
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(Event::Line(l)) => {
                writeln!(stdin, "{l}")?;
                stdin.flush()?;
            }
            Ok(Event::Changed) => {
                while rx.recv_timeout(Duration::from_millis(300)).is_ok() {}
                if rebuild(o, &plugins)?.is_some() {
                    ui::status("Reloading", &project.manifest.id);
                    writeln!(stdin, "rustic reload {}", project.manifest.id)?;
                    stdin.flush()?;
                }
            }
            Ok(Event::Eof) => {
                writeln!(stdin, "stop")?;
                let _ = stdin.flush();
                drop(stdin);
                child.wait()?;
                return Ok(());
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                child.wait()?;
                return Ok(());
            }
        }
    }
}

#[cfg(not(feature = "wasm"))]
fn run_repl(_o: &RunOptions, _project: &Project) -> Result<()> {
    bail!("this rustic binary was built without the `wasm` feature: the REPL cannot load plugins")
}

#[cfg(feature = "wasm")]
fn run_repl(o: &RunOptions, project: &Project) -> Result<()> {
    let dev_root = project.target_dir.join("rustic").join("dev");
    let Some((project, dir)) = rebuild(o, &dev_root)? else {
        bail!("the first build failed");
    };
    let pkg = rusticmc_core::archive::PluginPackage::open_dir(&dir).map_err(|e| anyhow!("{e}"))?;
    let server = rusticmc_test::MockServer::builder()
        .package(pkg)
        .build_unstarted();
    let mut repl = crate::repl::Repl::new(server, project.manifest.id.to_string());
    repl.banner();
    let (tx, rx) = mpsc::channel();
    stdin_lines(tx.clone());
    let _watcher = if o.watch {
        Some(watch(&project, tx)?)
    } else {
        None
    };
    repl.prompt();
    while let Some(e) = next_event(&rx) {
        match e {
            Event::Line(l) => {
                if !repl.execute(&l) {
                    break;
                }
            }
            Event::Changed => {
                if rebuild(o, &dev_root)?.is_some() {
                    repl.reload();
                }
            }
            Event::Eof => break,
        }
        repl.prompt();
    }
    anstream::println!();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dev_server_gets_offline_properties_and_never_a_silent_eula() {
        let dir = tempfile::tempdir().unwrap();
        let server = dir.path().join("dev-server");
        // Not accepted, nobody to ask: an error, properties written, no eula.txt.
        let err = prepare_dev_server(&server, false, || None).unwrap_err();
        assert!(err.to_string().contains("--accept-eula"), "{err}");
        let props = std::fs::read_to_string(server.join("server.properties")).unwrap();
        assert!(props.contains("online-mode=false") && props.contains("server-ip=127.0.0.1"));
        assert!(!server.join("eula.txt").exists());
        // Refused when asked: still not written.
        assert!(prepare_dev_server(&server, false, || Some(false)).is_err());
        assert!(!server.join("eula.txt").exists());
        // Accepted when asked.
        prepare_dev_server(&server, false, || Some(true)).unwrap();
        assert!(
            std::fs::read_to_string(server.join("eula.txt"))
                .unwrap()
                .contains("eula=true")
        );
        // Already accepted: nobody is asked again; an admin's server.properties is kept.
        std::fs::write(server.join("server.properties"), "server-port=1\n").unwrap();
        prepare_dev_server(&server, false, || panic!("asked again")).unwrap();
        assert_eq!(
            std::fs::read_to_string(server.join("server.properties")).unwrap(),
            "server-port=1\n"
        );
        // --accept-eula.
        let other = dir.path().join("other");
        prepare_dev_server(&other, true, || panic!("asked")).unwrap();
        assert!(other.join("eula.txt").exists());
    }
}
