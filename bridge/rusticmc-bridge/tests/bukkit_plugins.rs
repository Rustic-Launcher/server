//! The test plugins of `bridge/java/test-plugins` (ordinary Bukkit jars, built by
//! `bridge/java/build.sh`) run unmodified through a real JVM sidecar and the real Rustic core,
//! over a recording backend.
//!
//! Needs the Java build (`bridge/java/build/`) and a JDK 25 (`tools/jdk25`, `$JAVA_HOME` or
//! `java` on PATH); without the build the test is skipped with a message.

use rusticmc_bridge::{Bridge, BridgeConfig};
use rusticmc_core::model::*;
use rusticmc_core::{Backend, CommandOutcome, PluginState, Rustic};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// The game, recorded.
#[derive(Default)]
struct Rec {
    names: HashMap<Uuid, String>,
    ops: Vec<Uuid>,
    told: Vec<(Uuid, String)>,
    console: Vec<String>,
    broadcasts: Vec<String>,
    health: HashMap<Uuid, f32>,
    food: HashMap<Uuid, u32>,
    flying: HashMap<Uuid, (bool, bool)>,
}

impl Rec {
    fn told(&self, p: Uuid, needle: &str) -> bool {
        self.told.iter().any(|(u, m)| *u == p && m.contains(needle))
    }
}

impl Backend for Rec {
    fn server_broadcast(&mut self, message: &Text) -> Result<(), HostError> {
        self.broadcasts.push(message.to_plain());
        Ok(())
    }
    fn server_version(&mut self) -> Result<String, HostError> {
        Ok("Rustic test".into())
    }
    fn server_max_players(&mut self) -> Result<u32, HostError> {
        Ok(20)
    }
    fn server_motd(&mut self) -> Result<Text, HostError> {
        Ok(Text::plain("A test server"))
    }
    fn console_message(&mut self, message: &Text) -> Result<(), HostError> {
        self.console.push(message.to_plain());
        Ok(())
    }
    fn players_online(&mut self) -> Result<Vec<Uuid>, HostError> {
        Ok(self.names.keys().copied().collect())
    }
    fn players_name(&mut self, player: Uuid) -> Result<Option<String>, HostError> {
        Ok(self.names.get(&player).cloned())
    }
    fn players_is_online(&mut self, player: Uuid) -> Result<bool, HostError> {
        Ok(self.names.contains_key(&player))
    }
    fn players_send_message(&mut self, player: Uuid, message: &Text) -> Result<(), HostError> {
        self.told.push((player, message.to_plain()));
        Ok(())
    }
    fn players_is_op(&mut self, player: Uuid) -> Result<bool, HostError> {
        Ok(self.ops.contains(&player))
    }
    fn players_health(&mut self, player: Uuid) -> Result<f32, HostError> {
        Ok(*self.health.get(&player).unwrap_or(&7.0))
    }
    fn players_max_health(&mut self, _player: Uuid) -> Result<f32, HostError> {
        Ok(20.0)
    }
    fn players_set_health(&mut self, player: Uuid, health: f32) -> Result<(), HostError> {
        self.health.insert(player, health);
        Ok(())
    }
    fn players_set_food_level(&mut self, player: Uuid, food: u32) -> Result<(), HostError> {
        self.food.insert(player, food);
        Ok(())
    }
    fn players_set_flying(
        &mut self,
        player: Uuid,
        allow: bool,
        flying: bool,
    ) -> Result<(), HostError> {
        self.flying.insert(player, (allow, flying));
        Ok(())
    }
    fn players_get_location(&mut self, _player: Uuid) -> Result<Location, HostError> {
        Ok(Location {
            world: "minecraft:overworld".into(),
            x: 0.5,
            y: 64.0,
            z: 0.5,
            yaw: 0.0,
            pitch: 0.0,
        })
    }
    fn worlds_all(&mut self) -> Result<Vec<String>, HostError> {
        Ok(vec!["minecraft:overworld".into()])
    }
}

struct StderrLog;
impl log::Log for StderrLog {
    fn enabled(&self, m: &log::Metadata<'_>) -> bool {
        m.level() <= log::Level::Info
    }
    fn log(&self, r: &log::Record<'_>) {
        if self.enabled(r.metadata()) {
            eprintln!("{:5} {}: {}", r.level(), r.target(), r.args());
        }
    }
    fn flush(&self) {}
}
static LOGGER: StderrLog = StderrLog;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A server folder with the test plugin jars, or None when the Java build is missing.
fn server() -> Option<(tempfile::TempDir, BridgeConfig)> {
    let build = repo().join("bridge/java/build");
    let plugins = build.join("plugins");
    if !build.join("rustic-bridge.jar").is_file() || !plugins.is_dir() {
        eprintln!("SKIPPED: build the Java layer first (bridge/java/build.sh)");
        return None;
    }
    let dir = tempfile::tempdir().unwrap();
    let bukkit = dir.path().join("plugins/bukkit");
    std::fs::create_dir_all(&bukkit).unwrap();
    for e in std::fs::read_dir(&plugins).unwrap().flatten() {
        std::fs::copy(e.path(), bukkit.join(e.file_name())).unwrap();
    }
    let jdk = repo()
        .join("tools/jdk25/bin")
        .join(if cfg!(windows) { "java.exe" } else { "java" });
    let config = BridgeConfig {
        java: jdk.is_file().then_some(jdk),
        home: Some(build),
        call_timeout_ms: 4_000,
        watchdog_ms: 1_000,
        jvm_args: vec!["-Xmx256m".into()],
        ..BridgeConfig::default()
    };
    Some((dir, config))
}

fn steve() -> Uuid {
    "069a79f4-44e9-4726-a5be-fca90e38aaf5".parse().unwrap()
}

fn join(rt: &Rustic, rec: &mut Rec, who: Uuid, name: &str) -> PlayerJoin {
    rec.names.insert(who, name.into());
    rt.fire_record(
        PlayerJoin {
            player: who,
            name: name.into(),
            join_message: Some(Text::plain(format!("{name} joined the game"))),
        },
        rec,
    )
}

fn tick_until(rt: &Rustic, rec: &mut Rec, what: impl Fn(&Rec) -> bool) -> bool {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if what(rec) {
            return true;
        }
        rt.tick(rec);
        std::thread::sleep(Duration::from_millis(5));
    }
    what(rec)
}

#[test]
fn bukkit_plugins_run_unmodified() {
    let _ = log::set_logger(&LOGGER).map(|()| log::set_max_level(log::LevelFilter::Info));
    let Some((dir, config)) = server() else {
        return;
    };
    let bridge = Bridge::with_config(dir.path(), config);
    let packages = bridge.packages();
    assert_eq!(packages.len(), 4, "bridge + 3 plugins");
    let rt = Rustic::new(dir.path(), Box::new(bridge.loader(None))).unwrap();
    let mut rec = Rec {
        ops: vec![steve()],
        ..Rec::default()
    };
    let report = rt.load_packages(packages, &mut rec);
    assert!(report.failed.is_empty(), "{:?}", report.failed);
    for (id, r) in rt.enable_all(&mut rec) {
        assert!(r.is_ok(), "{id}: {r:?}");
    }
    let ids: Vec<String> = rt.plugins().iter().map(|p| p.id.to_string()).collect();
    for id in [
        "bukkit-bridge",
        "bukkit-welcome",
        "bukkit-rusticcommands",
        "bukkit-probe",
    ] {
        assert!(ids.contains(&id.to_string()), "{ids:?}");
        assert_eq!(rt.state(id), Some(PluginState::Enabled), "{id}");
    }
    assert!(
        dir.path()
            .join("plugins/bukkit-welcome/config.yml")
            .is_file(),
        "saveDefaultConfig"
    );

    // Join: the join line is the plugin's, it greets privately and again 40 ticks later.
    let out = join(&rt, &mut rec, steve(), "Steve");
    let line = out.join_message.map(|t| t.to_plain()).unwrap_or_default();
    assert_eq!(line, "Welcome, Steve!");
    assert!(rec.told(steve(), "you are visitor #1"), "{:?}", rec.told);
    assert!(tick_until(&rt, &mut rec, |r| r
        .told(steve(), "Enjoy your stay, Steve!")));

    // Chat: cancelled by the blocked word, at the plugin's priority.
    let chat = rt.fire_record(
        PlayerChat {
            player: steve(),
            name: "Steve".into(),
            message: "what a badword".into(),
            cancelled: false,
            display: None,
        },
        &mut rec,
    );
    assert!(chat.cancelled);
    assert!(rec.told(steve(), "Please do not say 'badword'."));

    // plugin.yml commands with their permissions.
    let me = CommandSender::Player(steve());
    assert_eq!(
        rt.dispatch_command(me, "welcome reload", &mut rec),
        CommandOutcome::Executed
    );
    assert!(rec.told(steve(), "Welcome configuration reloaded."));
    assert_eq!(
        rt.dispatch_command(me, "heal", &mut rec),
        CommandOutcome::Executed
    );
    assert_eq!(rec.health.get(&steve()), Some(&20.0));
    assert_eq!(rec.food.get(&steve()), Some(&20));
    assert_eq!(
        rt.dispatch_command(me, "fly", &mut rec),
        CommandOutcome::Executed
    );
    assert_eq!(rec.flying.get(&steve()), Some(&(true, false)));
    // An alias, the scheduler (async task, then a timer counting down on the main thread).
    assert_eq!(
        rt.dispatch_command(CommandSender::Console, "bc hello world", &mut rec),
        CommandOutcome::Executed
    );
    assert!(tick_until(&rt, &mut rec, |r| r
        .broadcasts
        .iter()
        .any(|b| b == "[CONSOLE] hello world")));

    // A shaded JDBC driver in the plugin jar.
    rt.dispatch_command(me, "probe sqlite", &mut rec);
    assert!(
        rec.told(steve(), "sqlite: 1 row(s) in probe.db"),
        "{:?}",
        rec.told
    );
    // An API call from a plugin thread goes through the main thread.
    rt.dispatch_command(me, "probe async", &mut rec);
    assert!(tick_until(&rt, &mut rec, |r| r
        .told(steve(), "async thread: 1 online, health 20.0")));

    // A plugin calling System.exit or throwing only fails its own command.
    rt.dispatch_command(me, "probe exit", &mut rec);
    rt.dispatch_command(me, "probe throw", &mut rec);
    assert!(rec.told(steve(), "An internal error occurred"));
    assert!(bridge.is_running());
    assert_eq!(rt.state("bukkit-probe"), Some(PluginState::Enabled));

    // A plugin stuck in a loop: the call times out, the JVM is killed, every Bukkit plugin is
    // disabled, the server ticks on; a reload of the bridge starts a new JVM.
    let t0 = Instant::now();
    let outcome = rt.dispatch_command(me, "probe loop", &mut rec);
    assert!(matches!(outcome, CommandOutcome::Fault(_)), "{outcome:?}");
    assert!(t0.elapsed() < Duration::from_secs(8));
    rt.tick(&mut rec);
    rt.tick(&mut rec);
    assert!(!bridge.is_running());
    assert_eq!(bridge.crashes(), 1);
    assert_ne!(rt.state("bukkit-welcome"), Some(PluginState::Enabled));
    assert_ne!(rt.state("bukkit-bridge"), Some(PluginState::Enabled));
    rt.reload("bukkit-bridge", &mut rec).unwrap();
    assert!(bridge.is_running());
    rt.enable("bukkit-welcome", &mut rec).unwrap();
    let out = join(
        &rt,
        &mut rec,
        "11111111-2222-3333-4444-555555555555".parse().unwrap(),
        "Alex",
    );
    assert_eq!(
        out.join_message.map(|t| t.to_plain()).as_deref(),
        Some("Welcome, Alex!")
    );

    // Shutdown runs onDisable (Welcome saves its visitor count) and stops the JVM.
    rt.shutdown(&mut rec);
    assert!(!bridge.is_running());
    let cfg =
        std::fs::read_to_string(dir.path().join("plugins/bukkit-welcome/config.yml")).unwrap();
    assert!(cfg.contains("visitors: 1"), "{cfg}");
}
