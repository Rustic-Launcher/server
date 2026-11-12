//! Try Bukkit plugin jars through the bridge without a game server: loads every jar given on the
//! command line into a scratch server folder, enables them, fires a join, a chat line and a quit,
//! runs `/<command>` for every plugin.yml command as the console, ticks for two seconds, then
//! prints each plugin's state. Everything the plugins log is printed (target `bukkit`).
//!
//! ```sh
//! bridge/java/build.sh
//! cargo run -p rusticmc-bridge --example try_jars -- path/to/Plugin.jar ...
//! ```

use rusticmc_bridge::{Bridge, BridgeConfig};
use rusticmc_core::model::*;
use rusticmc_core::{Backend, Rustic};
use std::path::{Path, PathBuf};
use std::time::Duration;

struct Game;
impl Backend for Game {
    fn console_message(&mut self, m: &Text) -> Result<(), HostError> {
        println!("  console <- {}", m.to_plain());
        Ok(())
    }
    fn players_send_message(&mut self, _p: Uuid, m: &Text) -> Result<(), HostError> {
        println!("  Steve <- {}", m.to_plain());
        Ok(())
    }
    fn server_broadcast(&mut self, m: &Text) -> Result<(), HostError> {
        println!("  broadcast <- {}", m.to_plain());
        Ok(())
    }
    fn players_online(&mut self) -> Result<Vec<Uuid>, HostError> {
        Ok(vec![STEVE])
    }
    fn players_name(&mut self, _p: Uuid) -> Result<Option<String>, HostError> {
        Ok(Some("Steve".into()))
    }
    fn worlds_all(&mut self) -> Result<Vec<String>, HostError> {
        Ok(vec!["minecraft:overworld".into()])
    }
    fn server_version(&mut self) -> Result<String, HostError> {
        Ok("try_jars".into())
    }
}

const STEVE: Uuid = Uuid::new(0x069a79f444e94726, 0xa5befca90e38aaf5);

struct Print;
impl log::Log for Print {
    fn enabled(&self, m: &log::Metadata<'_>) -> bool {
        m.level() <= log::Level::Info
    }
    fn log(&self, r: &log::Record<'_>) {
        if self.enabled(r.metadata()) {
            println!("{:5} {}: {}", r.level(), r.target(), r.args());
        }
    }
    fn flush(&self) {}
}
static LOG: Print = Print;

fn main() {
    let _ = log::set_logger(&LOG).map(|()| log::set_max_level(log::LevelFilter::Info));
    let jars: Vec<PathBuf> = std::env::args().skip(1).map(PathBuf::from).collect();
    let dir = tempfile::tempdir().unwrap();
    let bukkit = dir.path().join("plugins/bukkit");
    std::fs::create_dir_all(&bukkit).unwrap();
    for j in &jars {
        std::fs::copy(j, bukkit.join(j.file_name().unwrap())).unwrap();
    }
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let java = repo
        .join("tools/jdk25/bin")
        .join(if cfg!(windows) { "java.exe" } else { "java" });
    let bridge = Bridge::with_config(
        dir.path(),
        BridgeConfig {
            java: java.is_file().then_some(java),
            home: Some(repo.join("bridge/java/build")),
            ..BridgeConfig::default()
        },
    );
    let descriptors = bridge.scan().unwrap_or_default();
    let packages = bridge.packages();
    let rt = Rustic::new(dir.path(), Box::new(bridge.loader(None))).unwrap();
    rt.load_packages(packages, &mut Game);
    rt.enable_all(&mut Game);
    rt.fire_record(
        PlayerJoin {
            player: STEVE,
            name: "Steve".into(),
            join_message: Some(Text::plain("Steve joined the game")),
        },
        &mut Game,
    );
    let chat = rt.fire_record(
        PlayerChat {
            player: STEVE,
            name: "Steve".into(),
            message: "hello".into(),
            cancelled: false,
            display: None,
        },
        &mut Game,
    );
    println!("chat cancelled: {}", chat.cancelled);
    for d in &descriptors {
        for c in &d.commands {
            println!("> /{} (as the console)", c.name);
            println!(
                "  {:?}",
                rt.dispatch_command(CommandSender::Console, &c.name, &mut Game)
            );
        }
    }
    for _ in 0..40 {
        rt.tick(&mut Game);
        std::thread::sleep(Duration::from_millis(50));
    }
    rt.fire_record(
        PlayerQuit {
            player: STEVE,
            name: "Steve".into(),
            quit_message: None,
        },
        &mut Game,
    );
    for p in rt.plugins() {
        println!(
            "{:24} {:10} {}",
            p.id,
            p.state.to_string(),
            p.failure.unwrap_or_default()
        );
    }
    rt.shutdown(&mut Game);
}
