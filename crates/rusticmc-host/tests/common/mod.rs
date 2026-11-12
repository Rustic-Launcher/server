//! Shared helpers for the integration tests and benches: building the guest components,
//! a recording [`Backend`], and a throwaway server directory with `.rplugin` packages.

#![allow(dead_code)]

use rusticmc_core::archive::RpluginBuilder;
use rusticmc_core::config::ServerConfig;
use rusticmc_core::model::{HostError, Location, Text, Uuid};
use rusticmc_core::{Backend, Rustic};
use rusticmc_host::{HostConfig, WasmHost};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::{Duration, Instant};

/// Repository root.
pub fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("repo root")
}

fn wasm_dir() -> PathBuf {
    repo().join("plugins/target/wasm32-wasip2/release")
}

/// Build every guest once per test binary (incremental: ~1 s when nothing changed). Set
/// `RUSTIC_SKIP_GUEST_BUILD=1` to use what is already built.
pub fn ensure_built() {
    static BUILT: OnceLock<()> = OnceLock::new();
    BUILT.get_or_init(|| {
        if std::env::var_os("RUSTIC_SKIP_GUEST_BUILD").is_some() {
            return;
        }
        let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
        let target = repo().join("plugins/target");
        let run = |manifest: &str, pkg: &str| {
            let out = Command::new(&cargo)
                .args([
                    "build",
                    "--release",
                    "--target",
                    "wasm32-wasip2",
                    "--manifest-path",
                ])
                .arg(repo().join(manifest))
                .args(["-p", pkg])
                .env("CARGO_TARGET_DIR", &target)
                .env_remove("RUSTFLAGS")
                .env_remove("CARGO_ENCODED_RUSTFLAGS")
                .output()
                .expect("run cargo");
            if !out.status.success() {
                let have = wasm_dir()
                    .join(format!("{}.wasm", pkg.replace('-', "_")))
                    .exists();
                let msg = String::from_utf8_lossy(&out.stderr);
                assert!(have, "building guest {pkg} failed:\n{msg}");
                eprintln!("warning: rebuilding {pkg} failed, using the existing build:\n{msg}");
            }
        };
        // Each package on its own: the provider feature of guest-economy must not unify.
        for p in ["hello", "fixture-noop", "fixture-buggy", "fixture-bench"] {
            run("plugins/Cargo.toml", p);
        }
        for p in ["guest-probe", "guest-economy"] {
            run("crates/rusticmc-host/tests/guests/Cargo.toml", p);
        }
    });
}

/// The bytes of a built guest (`hello`, `fixture_bench`, `guest_probe`...).
pub fn wasm(name: &str) -> Vec<u8> {
    ensure_built();
    let p = wasm_dir().join(format!("{name}.wasm"));
    std::fs::read(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
}

/// A manifest for a test package.
pub fn manifest(id: &str, extra: &str) -> String {
    format!(
        "[plugin]\nid = \"{id}\"\nname = \"{id}\"\nversion = \"0.1.0\"\napi = \"0.1\"\n\
         authors = [\"tests\"]\nlicense = \"MIT\"\nentry = \"plugin.wasm\"\n{extra}"
    )
}

/// Data-folder filesystem + an HTTP allowlist.
pub const CAPS: &str =
    "[capabilities]\nfilesystem = \"data-folder\"\nnetwork = [\"allowed.example.org\"]\n";

/// What the plugins did to the game.
#[derive(Debug, Default)]
pub struct Recorder {
    pub console: Vec<String>,
    pub messages: Vec<(Uuid, String)>,
    pub teleports: Vec<(Uuid, Location)>,
    pub broadcasts: Vec<String>,
    pub name_calls: u64,
}

/// A small in-memory game: one online player `Steve` (uuid 0:1), op.
#[derive(Debug, Default)]
pub struct TestBackend {
    pub rec: Recorder,
}

pub const STEVE: Uuid = Uuid::new(0, 1);

impl Backend for TestBackend {
    fn console_message(&mut self, message: &Text) -> Result<(), HostError> {
        self.rec.console.push(message.to_plain());
        Ok(())
    }
    fn server_broadcast(&mut self, message: &Text) -> Result<(), HostError> {
        self.rec.broadcasts.push(message.to_plain());
        Ok(())
    }
    fn players_online(&mut self) -> Result<Vec<Uuid>, HostError> {
        Ok(vec![STEVE])
    }
    fn players_name(&mut self, player: Uuid) -> Result<Option<String>, HostError> {
        self.rec.name_calls += 1;
        Ok((player == STEVE).then(|| "Steve".to_owned()))
    }
    fn players_by_name(&mut self, name: &str) -> Result<Option<Uuid>, HostError> {
        Ok(name.eq_ignore_ascii_case("steve").then_some(STEVE))
    }
    fn players_is_online(&mut self, player: Uuid) -> Result<bool, HostError> {
        Ok(player == STEVE)
    }
    fn players_is_op(&mut self, _player: Uuid) -> Result<bool, HostError> {
        Ok(true)
    }
    fn players_send_message(&mut self, player: Uuid, message: &Text) -> Result<(), HostError> {
        self.rec.messages.push((player, message.to_plain()));
        Ok(())
    }
    fn players_send_action_bar(&mut self, _p: Uuid, _m: &Text) -> Result<(), HostError> {
        Ok(())
    }
    fn players_teleport(&mut self, player: Uuid, to: &Location) -> Result<(), HostError> {
        self.rec.teleports.push((player, to.clone()));
        Ok(())
    }
    fn worlds_all(&mut self) -> Result<Vec<String>, HostError> {
        Ok(vec!["minecraft:overworld".into()])
    }
}

/// A server directory with a WASM host and a runtime.
pub struct Server {
    pub dir: tempfile::TempDir,
    pub host: WasmHost,
    pub rt: Rustic,
    pub be: TestBackend,
}

/// Default test limits: generous fuel, 500 ms deadline, 64 MiB.
pub fn test_config() -> ServerConfig {
    let mut c = ServerConfig::default();
    c.limits.epoch_deadline_ms = 500;
    c.storage.flush_interval_ticks = 1;
    c
}

impl Server {
    pub fn new(cfg: ServerConfig) -> Self {
        Self::with_host_config(cfg, |_| {})
    }

    pub fn with_host_config(cfg: ServerConfig, f: impl FnOnce(&mut HostConfig)) -> Self {
        let dir = tempfile::tempdir().expect("tempdir");
        // One compiled-module cache shared by every test run (compiling with a debug build
        // of Cranelift is slow); tests that need a cold cache set their own.
        let mut hc = HostConfig {
            cache_dir: Some(repo().join("target").join("rusticmc-host-test-cache")),
            async_workers: Some(2),
            ..HostConfig::default()
        };
        f(&mut hc);
        let host = WasmHost::new(hc).expect("host");
        let rt = Rustic::builder(dir.path(), host.loader())
            .config(cfg)
            .build()
            .expect("runtime");
        host.install(&rt);
        Self {
            dir,
            host,
            rt,
            be: TestBackend::default(),
        }
    }

    pub fn plugins_dir(&self) -> PathBuf {
        let p = self.dir.path().join("plugins");
        std::fs::create_dir_all(&p).expect("plugins dir");
        p
    }

    /// Write `<id>.rplugin` with the given guest component and manifest extras.
    pub fn add(&self, id: &str, guest: &str, extra: &str, config: Option<&str>) {
        let mut b =
            RpluginBuilder::from_manifest_text(&manifest(id, extra), wasm(guest)).expect("package");
        if let Some(c) = config {
            b = b.default_config(c).expect("config");
        }
        b.write_to(&self.plugins_dir().join(format!("{id}.rplugin")))
            .expect("write package");
    }

    /// Load and enable everything new in `plugins/`; panics on failure.
    pub fn start(&mut self) {
        let report = self.rt.load_plugins(&mut self.be);
        let failed: Vec<_> = report
            .failed
            .iter()
            .filter(|(_, why)| why != "already loaded")
            .collect();
        assert!(failed.is_empty(), "load failures: {failed:?}");
        for (id, r) in self.rt.enable_all(&mut self.be) {
            r.unwrap_or_else(|e| panic!("enable {id}: {e}"));
        }
    }

    /// Run a console command, returning the console output it produced.
    pub fn console(&mut self, line: &str) -> (rusticmc_core::CommandOutcome, Vec<String>) {
        let before = self.be.rec.console.len();
        let out = self.rt.dispatch_command(
            rusticmc_core::model::CommandSender::Console,
            line,
            &mut self.be,
        );
        (out, self.be.rec.console[before..].to_vec())
    }

    /// A stored value of a plugin (the SDK stores JSON).
    pub fn stored(&self, plugin: &str, key: &str) -> Option<String> {
        self.rt
            .shared()
            .storage()
            .kv_get(plugin, key)
            .ok()
            .flatten()
            .map(|v| String::from_utf8_lossy(&v).into_owned())
    }

    /// Tick until `f` holds (or panic after `timeout`).
    pub fn tick_until(&mut self, timeout: Duration, mut f: impl FnMut(&mut Self) -> bool) {
        let start = Instant::now();
        loop {
            self.rt.tick(&mut self.be);
            if f(self) {
                return;
            }
            assert!(
                start.elapsed() < timeout,
                "condition not met in {timeout:?}"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}
