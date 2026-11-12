//! Measurements of the sidecar model (docs/BRIDGE.md): startup, event round trips (a cancellable
//! chat event, a high-frequency move event), per-tick overhead with 0/1/10 Bukkit plugins, memory.
//!
//! ```sh
//! bridge/java/build.sh
//! cargo run --release -p rusticmc-bridge --example measure
//! ```
//!
//! Each Bukkit plugin is a copy of the Probe test plugin (a chat listener that cancels
//! "cancelme", a move listener, a 1-tick repeating task), renamed with the JDK's `jar` tool.

use rusticmc_bridge::{Bridge, BridgeConfig};
use rusticmc_core::model::*;
use rusticmc_core::{Backend, Rustic};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

struct Game;
impl Backend for Game {
    fn players_online(&mut self) -> Result<Vec<Uuid>, HostError> {
        Ok(vec![STEVE])
    }
    fn players_name(&mut self, _p: Uuid) -> Result<Option<String>, HostError> {
        Ok(Some("Steve".into()))
    }
}

const STEVE: Uuid = Uuid::new(0x069a79f444e94726, 0xa5befca90e38aaf5);

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn jdk_tool(name: &str) -> PathBuf {
    let p = repo().join("tools/jdk25/bin").join(if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.into()
    });
    if p.is_file() { p } else { PathBuf::from(name) }
}

/// `n` renamed copies of the Probe plugin in `<dir>/plugins/bukkit`.
fn plugins(dir: &Path, n: usize) {
    let bukkit = dir.join("plugins/bukkit");
    std::fs::create_dir_all(&bukkit).unwrap();
    let probe = repo().join("bridge/java/build/plugins/Probe.jar");
    for i in 0..n {
        let jar = bukkit.join(format!("Probe{i}.jar"));
        std::fs::copy(&probe, &jar).unwrap();
        let yml = dir.join(format!("yml{i}"));
        std::fs::create_dir_all(&yml).unwrap();
        std::fs::write(
            yml.join("plugin.yml"),
            format!("name: Probe{i}\nversion: 0.1.0\nmain: rustictest.probe.ProbePlugin\ncommands:\n  probe{i}: {{}}\n"),
        )
        .unwrap();
        let ok = Command::new(jdk_tool("jar"))
            .arg("--update")
            .arg("--file")
            .arg(&jar)
            .arg("-C")
            .arg(&yml)
            .arg("plugin.yml")
            .status()
            .unwrap()
            .success();
        assert!(ok, "jar --update failed");
    }
}

fn config() -> BridgeConfig {
    BridgeConfig {
        java: Some(jdk_tool("java")),
        home: Some(repo().join("bridge/java/build")),
        jvm_args: vec!["-Xmx512m".into()],
        ..BridgeConfig::default()
    }
}

fn pct(v: &mut [Duration], p: f64) -> f64 {
    v.sort();
    let i = ((v.len() as f64 - 1.0) * p).round() as usize;
    v[i].as_secs_f64() * 1e6
}

/// Resident set size of a process, in MiB.
fn rss_mib(pid: u32) -> f64 {
    if cfg!(windows) {
        let out = Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                &format!("(Get-Process -Id {pid}).WorkingSet64"),
            ])
            .output()
            .unwrap();
        String::from_utf8_lossy(&out.stdout)
            .trim()
            .parse::<f64>()
            .unwrap_or(0.0)
            / 1048576.0
    } else {
        std::fs::read_to_string(format!("/proc/{pid}/status"))
            .ok()
            .and_then(|s| {
                s.lines()
                    .find(|l| l.starts_with("VmRSS:"))
                    .and_then(|l| l.split_whitespace().nth(1)?.parse::<f64>().ok())
            })
            .unwrap_or(0.0)
            / 1024.0
    }
}

struct Setup {
    _dir: tempfile::TempDir,
    rt: Rustic,
    bridge: Option<Bridge>,
    startup: Duration,
    enabled: Duration,
}

fn setup(n: usize) -> Setup {
    let dir = tempfile::tempdir().unwrap();
    plugins(dir.path(), n);
    let t0 = Instant::now();
    let bridge = (n > 0).then(|| Bridge::with_config(dir.path(), config()));
    let packages = bridge.as_ref().map(|b| b.packages()).unwrap_or_default();
    let startup = bridge
        .as_ref()
        .and_then(Bridge::startup_time)
        .unwrap_or_default();
    let loader: Box<dyn rusticmc_core::PluginLoader> = match &bridge {
        Some(b) => Box::new(b.loader(None)),
        None => Box::new(rusticmc_core::native::NativeLoader::new()),
    };
    let rt = Rustic::new(dir.path(), loader).unwrap();
    rt.load_packages(packages, &mut Game);
    for (id, r) in rt.enable_all(&mut Game) {
        assert!(r.is_ok(), "{id}: {r:?}");
    }
    let enabled = t0.elapsed();
    rt.fire_record(
        PlayerJoin {
            player: STEVE,
            name: "Steve".into(),
            join_message: None,
        },
        &mut Game,
    );
    Setup {
        _dir: dir,
        rt,
        bridge,
        startup,
        enabled,
    }
}

fn main() {
    let iters: usize = std::env::var("ITERS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(20_000);
    println!("# Rustic Bridge sidecar measurements ({iters} events, 2000 ticks)\n");
    println!(
        "rust process before the JVM: {:.1} MiB RSS",
        rss_mib(std::process::id())
    );
    for n in [0usize, 1, 10] {
        let s = setup(n);
        // Per tick: Rustic::tick (the bridge's pulse + each plugin's 1-tick task in the JVM).
        for _ in 0..200 {
            s.rt.tick(&mut Game);
        }
        let mut ticks: Vec<Duration> = (0..2000)
            .map(|_| {
                let t = Instant::now();
                s.rt.tick(&mut Game);
                t.elapsed()
            })
            .collect();
        let (t50, t99) = (pct(&mut ticks, 0.5), pct(&mut ticks, 0.99));
        println!("\n## {n} Bukkit plugin(s)");
        if let Some(b) = &s.bridge {
            println!(
                "startup: JVM spawn to handshake {} ms; to all {n} enabled {} ms",
                s.startup.as_millis(),
                s.enabled.as_millis()
            );
            println!(
                "memory: JVM {:.1} MiB RSS, rust process {:.1} MiB",
                rss_mib(b.pid().unwrap()),
                rss_mib(std::process::id())
            );
        }
        println!("tick: p50 {t50:.1} us, p99 {t99:.1} us");
        if n == 0 {
            continue;
        }
        let chat = |msg: &str| PlayerChat {
            player: STEVE,
            name: "Steve".into(),
            message: msg.into(),
            cancelled: false,
            display: None,
        };
        let mut v = Vec::with_capacity(iters);
        let mut cancelled = 0;
        for i in 0..iters {
            let t = Instant::now();
            let out = s.rt.fire_record(
                chat(if i % 2 == 0 { "hello" } else { "cancelme" }),
                &mut Game,
            );
            v.push(t.elapsed());
            cancelled += usize::from(out.cancelled);
        }
        assert_eq!(
            cancelled,
            iters / 2,
            "every other chat is cancelled by the plugin"
        );
        println!(
            "chat (cancellable, {n} listener plugin(s)): p50 {:.1} us, p99 {:.1} us",
            pct(&mut v, 0.5),
            pct(&mut v, 0.99)
        );
        let mut v = Vec::with_capacity(iters);
        for i in 0..iters {
            let at = |x: f64| Location {
                world: "minecraft:overworld".into(),
                x,
                y: 64.0,
                z: 0.0,
                yaw: 0.0,
                pitch: 0.0,
            };
            let t = Instant::now();
            s.rt.fire_record(
                PlayerMove {
                    player: STEVE,
                    from: at(i as f64 * 0.1),
                    to: at(i as f64 * 0.1 + 0.1),
                    cancelled: false,
                },
                &mut Game,
            );
            v.push(t.elapsed());
        }
        println!(
            "move ({n} listener plugin(s)): p50 {:.1} us, p99 {:.1} us",
            pct(&mut v, 0.5),
            pct(&mut v, 0.99)
        );
        if let Some(b) = &s.bridge {
            let mut v = Vec::with_capacity(iters);
            for _ in 0..iters {
                let t = Instant::now();
                let _ = b.request(serde_json::json!({"op": "ping"}));
                v.push(t.elapsed());
            }
            println!(
                "raw round trip (ping): p50 {:.1} us, p99 {:.1} us",
                pct(&mut v, 0.5),
                pct(&mut v, 0.99)
            );
        }
        s.rt.shutdown(&mut Game);
    }
}
