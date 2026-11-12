//! [`MockServer`]: the in-memory server and its builder.

use crate::build::BuiltPlugin;
use crate::harness::{AsyncMode, CommandRun, Harness, HarnessLoader};
use crate::player::MockPlayer;
use crate::state::{BarState, GameState, NETHER, OVERWORLD, ObjectiveState, THE_END, TeamState};
use crate::world::MockWorld;
use rusticmc_core::archive::PluginPackage;
use rusticmc_core::clock::FakeClock;
use rusticmc_core::config::ServerConfig;
use rusticmc_core::manifest::Manifest;
use rusticmc_core::model::{BarId, CommandSender, EventRecord, ServerListPing, Text};
use rusticmc_core::native::{NativeLoader, NativePlugin};
use rusticmc_core::permissions_engine::PermissionEngine;
use rusticmc_core::{PluginId, PluginInfo, PluginState, Rustic, TickReport};
use serde::de::DeserializeOwned;
use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

/// The in-memory server: a real Rustic runtime over a simulated game. See the crate docs.
///
/// Cheap to clone (all clones are the same server). Not `Send`: a server lives on its test's
/// thread, like the real one lives on the server thread.
#[derive(Clone)]
pub struct MockServer {
    pub(crate) h: Rc<Harness>,
}

impl std::fmt::Debug for MockServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MockServer")
            .field("dir", &self.h.dir)
            .finish_non_exhaustive()
    }
}

impl Default for MockServer {
    fn default() -> Self {
        Self::new()
    }
}

/// Builds a [`MockServer`].
#[must_use]
pub struct MockServerBuilder {
    config: ServerConfig,
    dir: Option<PathBuf>,
    packages: Vec<PluginPackage>,
    natives: Vec<(String, Box<dyn FnMut() -> NativePlugin>)>,
    configs: Vec<(String, String)>,
    async_mode: AsyncMode,
    permissions: Option<PermissionEngine>,
    #[cfg(feature = "wasm")]
    wasm: bool,
}

impl Default for MockServerBuilder {
    fn default() -> Self {
        Self {
            config: harness_config(),
            dir: None,
            packages: Vec::new(),
            natives: Vec::new(),
            configs: Vec::new(),
            async_mode: AsyncMode::Inline,
            permissions: None,
            #[cfg(feature = "wasm")]
            wasm: true,
        }
    }
}

/// The default `rustic.toml` of a mock server: the server defaults, except a 2 s wall-clock
/// deadline per plugin call (instead of 50 ms) so debug builds and busy CI machines do not trip
/// it; runaway loops are still stopped by fuel and by the deadline.
pub fn harness_config() -> ServerConfig {
    ServerConfig {
        limits: rusticmc_core::config::LimitsConfig {
            epoch_deadline_ms: 2_000,
            ..Default::default()
        },
        ..Default::default()
    }
}

/// A minimal valid manifest for a native plugin id.
pub fn native_manifest(id: &str) -> Manifest {
    Manifest::parse(&format!(
        "[plugin]\nid = \"{id}\"\nname = \"{id}\"\nversion = \"0.1.0\"\napi = \"0.1\"\n\
         authors = [\"test\"]\nlicense = \"MIT\"\nentry = \"{id}.wasm\"\n\
         [capabilities]\nfilesystem = \"data-folder\"\n"
    ))
    .unwrap_or_else(|e| panic!("invalid native plugin id `{id}`: {e}"))
}

impl MockServerBuilder {
    /// Use this `rustic.toml` instead of the defaults.
    pub fn config(mut self, config: ServerConfig) -> Self {
        self.config = config;
        self
    }

    /// Use this server directory (kept after the test) instead of a temporary one.
    pub fn dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.dir = Some(dir.into());
        self
    }

    /// Start with this `permissions.toml` engine.
    pub fn permissions(mut self, engine: PermissionEngine) -> Self {
        self.permissions = Some(engine);
        self
    }

    /// How async tasks run (default: inline after each tick).
    pub fn async_mode(mut self, mode: AsyncMode) -> Self {
        self.async_mode = mode;
        self
    }

    /// Add a native (closure) plugin; the factory runs again on reload.
    pub fn native_plugin(
        mut self,
        id: &str,
        factory: impl FnMut() -> NativePlugin + 'static,
    ) -> Self {
        self.natives.push((id.to_owned(), Box::new(factory)));
        self
    }

    /// Add a plugin package (built in memory, read from a `.rplugin`...).
    pub fn package(mut self, package: PluginPackage) -> Self {
        self.packages.push(package);
        self
    }

    /// Add a plugin built with [`crate::build_plugin`].
    pub fn built(self, plugin: &BuiltPlugin) -> Self {
        self.package(plugin.package())
    }

    /// Add a WebAssembly plugin: a plugin crate directory (built first), a dev directory, a
    /// `.rplugin`, or a `.wasm` next to its `rustic.plugin.toml`.
    #[cfg(feature = "wasm")]
    pub fn wasm_plugin(self, path: impl AsRef<Path>) -> Self {
        let pkg = crate::build::package_from_path(path.as_ref()).unwrap_or_else(|e| panic!("{e}"));
        self.package(pkg)
    }

    /// Write `<data folder>/config.toml` of a plugin before it is enabled (what an admin edits).
    pub fn plugin_config(mut self, id: &str, toml: &str) -> Self {
        self.configs.push((id.to_owned(), toml.to_owned()));
        self
    }

    /// Build the server, then load and enable every plugin added (in dependency order). Panics
    /// if a plugin fails to load or enable, with the reason.
    pub fn build(self) -> MockServer {
        let server = self.build_unstarted();
        server.assert_all_enabled();
        server
    }

    /// Like [`MockServerBuilder::build`] but does not panic on plugin failures (assert them with
    /// [`MockServer::plugin_state`] / [`MockServer::plugin`]).
    pub fn build_unstarted(self) -> MockServer {
        let (dir, tmp) = match self.dir {
            Some(d) => {
                std::fs::create_dir_all(&d).expect("server dir");
                (d, None)
            }
            None => {
                let t = tempfile::Builder::new()
                    .prefix("rustic-mock-")
                    .tempdir()
                    .expect("temp dir");
                (t.path().to_path_buf(), Some(t))
            }
        };
        let native = NativeLoader::new();
        let native_ids: Rc<RefCell<BTreeSet<String>>> = Rc::default();
        #[cfg(feature = "wasm")]
        let wasm_loader = if self.wasm {
            Some(crate::wasm::loader())
        } else {
            None
        };
        #[cfg(not(feature = "wasm"))]
        let wasm_loader: Option<(
            Box<dyn rusticmc_core::PluginLoader>,
            Rc<dyn crate::harness::WasmSide>,
        )> = None;
        let (wasm_box, runner) = match wasm_loader {
            Some((l, r)) => (Some(l), Some(r)),
            None => (None, None),
        };
        let loader = HarnessLoader {
            native: native.clone(),
            native_ids: native_ids.clone(),
            wasm: wasm_box,
        };
        let clock = Arc::new(FakeClock::new());
        let rt = Rustic::builder(&dir, Box::new(loader))
            .config(self.config)
            .clock(clock.clone())
            .permissions(self.permissions.unwrap_or_default())
            .build()
            .unwrap_or_else(|e| panic!("cannot start the runtime: {e}"));
        let h = Rc::new(Harness {
            rt,
            state: RefCell::new(GameState::default()),
            clock,
            native,
            native_ids,
            wasm: runner,
            async_mode: Cell::new(self.async_mode),
            depth: Cell::new(0),
            dir,
            log: crate::logcap::register(),
            _tmp: tmp,
        });
        let server = MockServer { h };
        let mut packages = self.packages;
        for (id, factory) in self.natives {
            server.register_native(&id, factory);
            packages.push(
                PluginPackage::in_memory(
                    native_manifest(&id),
                    Vec::new(),
                    None,
                    Default::default(),
                )
                .expect("native package"),
            );
        }
        for (id, text) in &self.configs {
            let d = server.h.rt.layout().data_dir(id, None);
            std::fs::create_dir_all(&d).expect("data dir");
            std::fs::write(d.join("config.toml"), text).expect("write config.toml");
        }
        if !packages.is_empty() {
            server.h.op(|| {
                server.h.rt.load_packages(packages, &mut server.h.backend());
                server.h.rt.enable_all(&mut server.h.backend());
            });
        }
        server
    }
}

impl MockServer {
    /// An empty server (no plugin): temp directory, default `rustic.toml`, fake clock.
    pub fn new() -> Self {
        Self::builder().build()
    }

    /// Configure a server.
    pub fn builder() -> MockServerBuilder {
        MockServerBuilder::default()
    }

    /// A server running one native (closure) plugin.
    pub fn with_native_plugin(id: &str, factory: impl FnMut() -> NativePlugin + 'static) -> Self {
        Self::builder().native_plugin(id, factory).build()
    }

    /// A server running one WebAssembly plugin: a plugin crate directory (built with
    /// [`crate::build_plugin`]), a dev directory, a `.rplugin`, or a `.wasm` file next to its
    /// `rustic.plugin.toml`.
    #[cfg(feature = "wasm")]
    pub fn with_plugin_wasm(path: impl AsRef<Path>) -> Self {
        Self::builder().wasm_plugin(path).build()
    }

    /// A server running a plugin built with [`crate::build_plugin`].
    pub fn with_built(plugin: &BuiltPlugin) -> Self {
        Self::builder().built(plugin).build()
    }

    fn register_native(&self, id: &str, factory: impl FnMut() -> NativePlugin + 'static) {
        self.h.native_ids.borrow_mut().insert(id.to_owned());
        self.h.native.register(id, factory);
    }

    /// Load and enable a native plugin into the running server.
    pub fn load_native(
        &self,
        id: &str,
        factory: impl FnMut() -> NativePlugin + 'static,
    ) -> Result<PluginId, String> {
        self.register_native(id, factory);
        let pkg =
            PluginPackage::in_memory(native_manifest(id), Vec::new(), None, Default::default())
                .map_err(|e| e.to_string())?;
        self.load_package(pkg)
    }

    /// Load and enable a package into the running server.
    pub fn load_package(&self, package: PluginPackage) -> Result<PluginId, String> {
        let id = package.manifest.id.clone();
        self.h.op(|| {
            let report = self
                .h
                .rt
                .load_packages(vec![package], &mut self.h.backend());
            if let Some((_, why)) = report.failed.first() {
                return Err(why.clone());
            }
            self.h
                .rt
                .enable(&id, &mut self.h.backend())
                .map_err(|e| e.to_string())?;
            Ok(id)
        })
    }

    /// Load and enable a WebAssembly plugin (see [`MockServer::with_plugin_wasm`]).
    #[cfg(feature = "wasm")]
    pub fn load_plugin(&self, path: impl AsRef<Path>) -> Result<PluginId, String> {
        let pkg = crate::build::package_from_path(path.as_ref())?;
        self.load_package(pkg)
    }

    /// The Rustic runtime (for anything the harness does not wrap).
    pub fn runtime(&self) -> &Rustic {
        &self.h.rt
    }

    /// Run `f` with the runtime and the mock backend installed (to call `Rustic` entry points
    /// directly).
    pub fn with_backend<R>(
        &self,
        f: impl FnOnce(&Rustic, &mut dyn rusticmc_core::Backend) -> R,
    ) -> R {
        self.h.op(|| f(&self.h.rt, &mut self.h.backend()))
    }

    /// The server directory (`plugins/`, `rustic.toml`, `permissions.toml`).
    pub fn dir(&self) -> &Path {
        &self.h.dir
    }

    /// A plugin's data folder (`plugins/<id>/`).
    pub fn data_dir(&self, plugin: &str) -> PathBuf {
        self.h.rt.layout().data_dir(plugin, None)
    }

    // ------------------------------------------------------------------ plugins

    fn assert_all_enabled(&self) {
        let bad: Vec<String> = self
            .h
            .rt
            .plugins()
            .into_iter()
            .filter(|p| p.state != PluginState::Enabled)
            .map(|p| {
                format!(
                    "{} is {}: {}",
                    p.id,
                    p.state,
                    p.failure.unwrap_or_else(|| "no reason recorded".into())
                )
            })
            .collect();
        assert!(
            bad.is_empty(),
            "plugins did not start:\n  {}\nconsole:\n  {}",
            bad.join("\n  "),
            self.console().join("\n  ")
        );
    }

    /// A plugin's public info (state, failure reason...).
    pub fn plugin(&self, id: &str) -> Option<PluginInfo> {
        self.h.rt.plugin(id)
    }

    /// A plugin's state.
    pub fn plugin_state(&self, id: &str) -> Option<PluginState> {
        self.h.rt.state(id)
    }

    /// Panics unless the plugin is enabled (shows the failure reason).
    #[track_caller]
    pub fn assert_enabled(&self, id: &str) {
        let info = self.plugin(id);
        assert!(
            info.as_ref()
                .is_some_and(|p| p.state == PluginState::Enabled),
            "plugin {id} is not enabled: {info:?}"
        );
    }

    /// Disable a plugin (dependents first).
    pub fn disable(&self, id: &str) -> Result<(), String> {
        self.h
            .op(|| self.h.rt.disable(id, &mut self.h.backend()))
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// Enable a plugin again (re-instantiated).
    pub fn enable(&self, id: &str) -> Result<(), String> {
        self.h
            .op(|| self.h.rt.enable(id, &mut self.h.backend()))
            .map_err(|e| e.to_string())
    }

    /// Reload a plugin (`/rustic reload <id>`).
    pub fn reload(&self, id: &str) -> Result<(), String> {
        self.h
            .op(|| self.h.rt.reload(id, &mut self.h.backend()))
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// Shut the runtime down (disables every plugin, flushes storage).
    pub fn shutdown(&self) {
        self.h.op(|| self.h.rt.shutdown(&mut self.h.backend()));
    }

    /// Plugins disabled so far (in order), as the backend saw them.
    pub fn disabled_plugins(&self) -> Vec<String> {
        self.h.state.borrow().disabled_plugins.clone()
    }

    // ------------------------------------------------------------------ players

    /// Connect a player (fires `player-pre-login` then `player-join`). Panics if refused.
    pub fn add_player(&self, name: &str) -> MockPlayer {
        self.try_add_player(name)
            .unwrap_or_else(|reason| panic!("{name} could not join: {reason}"))
    }

    /// Connect an operator.
    pub fn add_op(&self, name: &str) -> MockPlayer {
        let uuid = crate::harness::offline_uuid(name);
        self.h.state.borrow_mut().ops.insert(uuid);
        self.add_player(name)
    }

    /// Connect a player: `Err(kick message)` if a plugin refused them or the server is full.
    pub fn try_add_player(&self, name: &str) -> Result<MockPlayer, String> {
        self.try_add_player_from(name, "127.0.0.1")
    }

    /// Connect a player from an address.
    pub fn try_add_player_from(&self, name: &str, address: &str) -> Result<MockPlayer, String> {
        let uuid = self.h.join(name, address).map_err(|t| t.to_plain())?;
        let name = self
            .h
            .player_state(uuid)
            .map(|p| p.name)
            .unwrap_or_default();
        Ok(MockPlayer {
            h: self.h.clone(),
            uuid,
            name,
        })
    }

    /// An online player by name (case-insensitive).
    pub fn player(&self, name: &str) -> Option<MockPlayer> {
        let uuid = self.h.state.borrow().by_name(name)?;
        Some(self.handle(uuid))
    }

    fn handle(&self, uuid: rusticmc_core::model::Uuid) -> MockPlayer {
        let name = self
            .h
            .player_state(uuid)
            .map(|p| p.name)
            .unwrap_or_default();
        MockPlayer {
            h: self.h.clone(),
            uuid,
            name,
        }
    }

    /// Online players in join order.
    pub fn online_players(&self) -> Vec<MockPlayer> {
        let uuids = self.h.state.borrow().online.clone();
        uuids.into_iter().map(|u| self.handle(u)).collect()
    }

    // ------------------------------------------------------------------ commands

    /// Type a command on the console (fires `server-command`, then runs it).
    pub fn console_command(&self, line: &str) -> CommandRun {
        self.h.command(CommandSender::Console, line)
    }

    /// Run a command as a player, as typed (fires `player-command`).
    pub fn dispatch_as(&self, player: &MockPlayer, line: &str) -> CommandRun {
        self.h.command(player.sender(), line)
    }

    /// Tab completion for a player: suggestions for the last word of `line`.
    pub fn complete_as(&self, player: &MockPlayer, line: &str) -> Vec<String> {
        player.complete(line)
    }

    /// Tab completion on the console.
    pub fn complete_console(&self, line: &str) -> Vec<String> {
        self.h.op(|| {
            self.h
                .rt
                .complete(CommandSender::Console, line, &mut self.h.backend())
        })
    }

    /// Whether `/label` is a Rustic command (a plugin's or `/rustic`).
    pub fn has_command(&self, label: &str) -> bool {
        self.h
            .rt
            .command_tree()
            .iter()
            .any(|c| c.labels.iter().any(|l| l == label))
    }

    // ------------------------------------------------------------------ time

    /// Run `n` ticks (50 ms of fake time each): scheduler, `post-to-main` messages and, in the
    /// default inline mode, due async tasks. Returns the last tick's report.
    pub fn tick(&self, n: u32) -> TickReport {
        let mut last = TickReport::default();
        for _ in 0..n {
            last = self.h.tick();
        }
        last
    }

    /// Run ticks for a duration of game time (`Duration::from_secs(5)` = 100 ticks).
    pub fn run_for(&self, d: Duration) -> TickReport {
        self.tick((d.as_millis() / 50) as u32)
    }

    /// The current tick.
    pub fn current_tick(&self) -> u64 {
        self.h.rt.current_tick()
    }

    /// Move the fake clock without ticking (async delays are in milliseconds).
    pub fn advance_time(&self, d: Duration) {
        self.h.clock.advance(d);
    }

    /// Run due async tasks now, inline: how many ran.
    pub fn run_async(&self) -> usize {
        self.h.op(|| self.h.run_async_due(true))
    }

    /// Change how async tasks run.
    pub fn set_async_mode(&self, mode: AsyncMode) {
        self.h.async_mode.set(mode);
    }

    // ------------------------------------------------------------------ events and game

    /// Fire any event and get it back with the plugins' changes.
    pub fn fire<E: EventRecord>(&self, event: E) -> E {
        self.h.op(|| self.h.fire(event))
    }

    /// A server-list ping from `address` (fires `server-list-ping`): what the client sees.
    pub fn ping(&self, address: &str) -> ServerListPing {
        let (motd, max, online) = {
            let st = self.h.state.borrow();
            (st.motd.clone(), st.max_players, st.online.len() as u32)
        };
        self.fire(ServerListPing {
            address: address.to_owned(),
            motd,
            max_players: max,
            online_players: online,
            show_players: true,
        })
    }

    /// A world by id (`"minecraft:overworld"`, or `"overworld"`).
    pub fn world(&self, id: &str) -> MockWorld {
        let id = crate::state::namespaced(id);
        assert!(
            self.h.state.borrow().worlds.contains_key(&id),
            "no world {id}"
        );
        MockWorld {
            h: self.h.clone(),
            id,
        }
    }

    /// The overworld.
    pub fn overworld(&self) -> MockWorld {
        self.world(OVERWORLD)
    }

    /// The nether.
    pub fn nether(&self) -> MockWorld {
        self.world(NETHER)
    }

    /// The end.
    pub fn the_end(&self) -> MockWorld {
        self.world(THE_END)
    }

    /// Max players (joins beyond it are refused, operators excepted).
    pub fn set_max_players(&self, n: u32) {
        self.h.state.borrow_mut().max_players = n;
    }

    /// The MOTD.
    pub fn set_motd(&self, motd: Text) {
        self.h.state.borrow_mut().motd = motd;
    }

    /// Whether `messaging.behind-proxy` answers true.
    pub fn set_behind_proxy(&self, yes: bool) {
        self.h.state.borrow_mut().behind_proxy = yes;
    }

    /// The `keepInventory` game rule (the default of `player-death.keep-inventory`).
    pub fn set_keep_inventory(&self, yes: bool) {
        self.h.state.borrow_mut().keep_inventory = yes;
    }

    // ------------------------------------------------------------------ inspection

    /// Console output so far: command replies to the console, broadcasts, and log lines
    /// (`[INFO] [hello] Hello enabled`).
    pub fn console(&self) -> Vec<String> {
        let mut out = self.h.state.borrow().console.clone();
        out.extend(self.h.log.borrow().iter().cloned());
        out
    }

    /// Console messages only (command replies to the console, broadcasts), without log lines.
    pub fn console_messages(&self) -> Vec<String> {
        self.h.state.borrow().console.clone()
    }

    /// Log lines only (`[WARN] ...`).
    pub fn log_lines(&self) -> Vec<String> {
        self.h.log.borrow().clone()
    }

    /// Panics unless some console line contains `needle`.
    #[track_caller]
    pub fn assert_console_contains(&self, needle: &str) {
        let c = self.console();
        assert!(
            c.iter().any(|l| l.contains(needle)),
            "no console line contains {needle:?}; console: {c:#?}"
        );
    }

    /// Forget console output and log lines.
    pub fn clear_console(&self) {
        self.h.state.borrow_mut().console.clear();
        self.h.log.borrow_mut().clear();
    }

    /// Everything broadcast (`server.broadcast`, join/quit/death lines, chat), plain.
    pub fn broadcasts(&self) -> Vec<String> {
        self.h
            .state
            .borrow()
            .broadcasts
            .iter()
            .map(Text::to_plain)
            .collect()
    }

    /// Commands run as the console (typed or dispatched by plugins), without slash.
    pub fn console_commands(&self) -> Vec<String> {
        self.h.state.borrow().console_commands.clone()
    }

    /// A scoreboard objective.
    pub fn objective(&self, name: &str) -> Option<ObjectiveState> {
        self.h.state.borrow().objectives.get(name).cloned()
    }

    /// Every objective name.
    pub fn objectives(&self) -> Vec<String> {
        self.h.state.borrow().objectives.keys().cloned().collect()
    }

    /// A score.
    pub fn score(&self, objective: &str, entry: &str) -> Option<i32> {
        self.objective(objective)?.scores.get(entry).copied()
    }

    /// The objective shown in a display slot (`"sidebar"`, `"list"`, `"below_name"`).
    pub fn displayed_objective(&self, slot: &str) -> Option<String> {
        self.h.state.borrow().display_slots.get(slot).cloned()
    }

    /// A team.
    pub fn team(&self, name: &str) -> Option<TeamState> {
        self.h.state.borrow().teams.get(name).cloned()
    }

    /// Boss bars that exist.
    pub fn boss_bars(&self) -> Vec<(BarId, BarState)> {
        self.h
            .state
            .borrow()
            .bars
            .iter()
            .map(|(k, v)| (*k, v.clone()))
            .collect()
    }

    /// Plugin channels some plugin listens on.
    pub fn incoming_channels(&self) -> Vec<String> {
        self.h
            .state
            .borrow()
            .incoming_channels
            .iter()
            .cloned()
            .collect()
    }

    /// How many times the command set changed (the real adapter re-sends commands then).
    pub fn commands_changed(&self) -> u32 {
        self.h.state.borrow().commands_changed
    }

    /// A plugin's raw storage value (`storage.get`).
    pub fn storage(&self, plugin: &str, key: &str) -> Option<Vec<u8>> {
        let id = PluginId::new(plugin).ok()?;
        self.h.rt.shared().async_api(id).storage_get(key)
    }

    /// A plugin's storage value as JSON (what the SDK's `Storage::set(key, &value)` writes).
    pub fn storage_json<T: DeserializeOwned>(&self, plugin: &str, key: &str) -> Option<T> {
        serde_json::from_slice(&self.storage(plugin, key)?).ok()
    }

    /// Set a plugin's storage value as JSON (test setup).
    pub fn set_storage_json<T: serde::Serialize>(&self, plugin: &str, key: &str, value: &T) {
        let id = PluginId::new(plugin).expect("plugin id");
        self.h
            .rt
            .shared()
            .async_api(id)
            .storage_set(key, serde_json::to_vec(value).expect("json"));
    }

    /// The raw game state, for assertions the helpers do not cover.
    pub fn with_state<R>(&self, f: impl FnOnce(&mut GameState) -> R) -> R {
        f(&mut self.h.state.borrow_mut())
    }
}
