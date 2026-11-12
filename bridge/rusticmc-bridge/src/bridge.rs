//! [`Bridge`]: the JVM's owner on the Rust side, and the core plugin instances it backs.
//!
//! The core sees one plugin per Bukkit jar (`bukkit-<name>`, a [`JavaPluginInstance`]) plus the
//! bridge itself (`bukkit-bridge`, a [`BridgeInstance`]), which every Bukkit plugin depends on.
//! The bridge instance owns the JVM's lifetime (started on load, stopped on disable), feeds the
//! Java scheduler one `tick` pulse per server tick, and keeps the JVM's view of online players.
//! When the JVM dies or stops answering, the call in flight fails with a fault, the process is
//! killed and the bridge plugin is reported faulty, so the core disables it and, through the
//! dependency, every Bukkit plugin; the server goes on. `rustic reload bukkit-bridge` starts a
//! new JVM.

use crate::config::BridgeConfig;
use crate::descriptor::{BRIDGE_ID, Descriptor, bridge_package};
use crate::link::{Link, LinkError};
use crate::sidecar::{self, Sidecar};
use rusticmc_core::archive::PluginPackage;
use rusticmc_core::context;
use rusticmc_core::lifecycle::{LoadRequest, PluginFault, PluginInstance, PluginLoader};
use rusticmc_core::model::{
    ArgValue, CommandContext, CommandId, CommandSender, CompleteContext, Event, HandlerId,
    Priority, TaskId, Text,
};
use rusticmc_core::{PluginId, Rustic};
use serde_json::{Value, json};
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

struct Jvm {
    sidecar: Sidecar,
    link: Rc<Link>,
}

#[derive(Default)]
struct State {
    descriptors: BTreeMap<String, Descriptor>,
    names: HashMap<String, PluginId>,
    handlers: HashMap<(String, HandlerId), String>,
    commands: HashMap<(String, CommandId), String>,
    pending_disable: Vec<PluginId>,
}

struct Inner {
    server_dir: PathBuf,
    config: BridgeConfig,
    jvm: RefCell<Option<Jvm>>,
    state: RefCell<State>,
    crashes: Cell<u32>,
    started_in: Cell<Option<Duration>>,
}

/// The bridge runtime (cheap to clone; lives on the server thread like the core).
#[derive(Clone)]
pub struct Bridge(Rc<Inner>);

impl std::fmt::Debug for Bridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bridge")
            .field("server_dir", &self.0.server_dir)
            .field("running", &self.is_running())
            .finish_non_exhaustive()
    }
}

impl Bridge {
    /// A bridge for the server in `server_dir` (`rustic-bridge.toml` read if present). Nothing
    /// starts yet.
    pub fn new(server_dir: impl Into<PathBuf>) -> Result<Self, String> {
        let server_dir = server_dir.into();
        let config = BridgeConfig::load(&server_dir)?;
        Ok(Self::with_config(server_dir, config))
    }

    /// With explicit settings.
    pub fn with_config(server_dir: impl Into<PathBuf>, config: BridgeConfig) -> Self {
        Self(Rc::new(Inner {
            server_dir: server_dir.into(),
            config,
            jvm: RefCell::new(None),
            state: RefCell::new(State::default()),
            crashes: Cell::new(0),
            started_in: Cell::new(None),
        }))
    }

    /// The settings.
    pub fn config(&self) -> &BridgeConfig {
        &self.0.config
    }

    /// The folder of the Bukkit jars.
    pub fn plugins_dir(&self) -> PathBuf {
        self.0.config.plugins_dir(&self.0.server_dir)
    }

    /// Whether the JVM runs.
    pub fn is_running(&self) -> bool {
        self.0
            .jvm
            .borrow_mut()
            .as_mut()
            .is_some_and(|j| j.sidecar.is_running())
    }

    /// The JVM's process id.
    pub fn pid(&self) -> Option<u32> {
        self.0.jvm.borrow().as_ref().map(|j| j.sidecar.pid)
    }

    /// How long the last start took (spawn to handshake).
    pub fn startup_time(&self) -> Option<Duration> {
        self.0.started_in.get()
    }

    /// Times the JVM was lost (crash, kill, stuck call).
    pub fn crashes(&self) -> u32 {
        self.0.crashes.get()
    }

    /// Round trips made on the current connection.
    pub fn round_trips(&self) -> u64 {
        self.0
            .jvm
            .borrow()
            .as_ref()
            .map(|j| j.link.calls.get())
            .unwrap_or(0)
    }

    /// Start the JVM if it does not run.
    pub fn start(&self) -> Result<(), String> {
        if self.is_running() {
            return Ok(());
        }
        let t0 = Instant::now();
        let (sidecar, link) = sidecar::spawn(&self.0.config, &self.0.server_dir)?;
        let took = t0.elapsed();
        self.0.started_in.set(Some(took));
        log::info!(target: "rustic", "[bukkit-bridge] JVM started (pid {}) in {} ms", sidecar.pid, took.as_millis());
        *self.0.jvm.borrow_mut() = Some(Jvm {
            sidecar,
            link: Rc::new(link),
        });
        Ok(())
    }

    /// Ask the JVM to disable its plugins and exit, then make sure it did.
    pub fn stop(&self) {
        if self.0.jvm.borrow().is_none() {
            return;
        }
        let _ = self.request(json!({"op": "shutdown"}));
        if let Some(mut j) = self.0.jvm.borrow_mut().take() {
            j.link.close();
            j.sidecar.stop(Duration::from_secs(5));
            log::info!(target: "rustic", "[bukkit-bridge] JVM stopped");
        }
    }

    /// One request to the JVM. `Ok(Err(message))`: Java answered with an exception.
    /// `Err(fault)`: the JVM is gone or stuck (it is killed, see the module docs).
    pub fn request(&self, body: Value) -> Result<Result<Value, String>, PluginFault> {
        let link = self.0.jvm.borrow().as_ref().map(|j| j.link.clone());
        let Some(link) = link else {
            return Err(PluginFault::other("the Rustic Bridge JVM is not running"));
        };
        match link.request(&body, &|req| self.serve(req)) {
            Ok(r) => Ok(r),
            Err(e) => Err(self.lost(&e)),
        }
    }

    /// The JVM failed: kill it and have the core disable the bridge (and so every Bukkit plugin).
    fn lost(&self, e: &LinkError) -> PluginFault {
        let fault = match e {
            LinkError::TimedOut(_) => PluginFault::timeout(format!("Rustic Bridge: {e}")),
            _ => PluginFault::other(format!("Rustic Bridge: {e}")),
        };
        if let Some(mut j) = self.0.jvm.borrow_mut().take() {
            self.0.crashes.set(self.0.crashes.get() + 1);
            log::error!(target: "rustic", "[bukkit-bridge] {e}; the JVM (pid {}) is stopped and the Bukkit plugins are disabled (`rustic reload {BRIDGE_ID}` starts it again)", j.sidecar.pid);
            j.link.close();
            j.sidecar.kill();
            if let Ok(rt) = context::runtime() {
                rt.shared()
                    .report_fault(&PluginId::new_unchecked(BRIDGE_ID), fault.clone());
            }
        }
        fault
    }

    /// Read the plugin jars (starts the JVM). Descriptors with an `error` are returned too.
    pub fn scan(&self) -> Result<Vec<Descriptor>, String> {
        self.start()?;
        let dir = self.plugins_dir();
        let v = self
            .request(json!({"op": "scan", "dir": dir}))
            .map_err(|f| f.to_string())??;
        let list: Vec<Descriptor> = serde_json::from_value(v).map_err(|e| e.to_string())?;
        let mut st = self.0.state.borrow_mut();
        for d in &list {
            if d.error.is_none()
                && d.id() != BRIDGE_ID
                && let Ok(id) = PluginId::new(&d.id())
            {
                st.names.insert(d.name.clone(), id.clone());
                st.descriptors.insert(id.to_string(), d.clone());
            }
        }
        Ok(list)
    }

    /// The packages to give the core: the bridge and every valid Bukkit jar of the plugin folder.
    /// Empty (and no JVM) when the folder has no jar or the bridge is disabled; problems are logged.
    pub fn packages(&self) -> Vec<PluginPackage> {
        if !self.0.config.enabled {
            return Vec::new();
        }
        let has_jars = std::fs::read_dir(self.plugins_dir())
            .map(|rd| {
                rd.flatten().any(|e| {
                    e.path()
                        .extension()
                        .is_some_and(|x| x.eq_ignore_ascii_case("jar"))
                })
            })
            .unwrap_or(false);
        if !has_jars {
            return Vec::new();
        }
        let list = match self.scan() {
            Ok(l) => l,
            Err(e) => {
                log::error!(target: "rustic", "[bukkit-bridge] Bukkit plugins not loaded: {e}");
                return Vec::new();
            }
        };
        let mut out = vec![bridge_package()];
        for d in list {
            if let Some(err) = &d.error {
                log::error!(target: "rustic", "[bukkit-bridge] {}: {err}", d.file.display());
                continue;
            }
            if d.id() == BRIDGE_ID {
                log::error!(target: "rustic", "[bukkit-bridge] {}: the name `{}` is reserved", d.file.display(), d.name);
                continue;
            }
            match d.package() {
                Ok(p) => out.push(p),
                Err(e) => {
                    log::error!(target: "rustic", "[bukkit-bridge] {}: {e}", d.file.display())
                }
            }
        }
        out
    }

    /// The loader to give the core: Bukkit plugins here, everything else to `fallback`.
    pub fn loader(&self, fallback: Option<Box<dyn PluginLoader>>) -> BridgeLoader {
        BridgeLoader {
            bridge: self.clone(),
            fallback,
        }
    }

    pub(crate) fn id_of(&self, name: &str) -> Option<PluginId> {
        self.0.state.borrow().names.get(name).cloned()
    }

    fn descriptor(&self, id: &str) -> Option<Descriptor> {
        self.0.state.borrow().descriptors.get(id).cloned()
    }

    pub(crate) fn remember_handler(&self, id: &PluginId, handler: HandlerId, prio: &str) {
        self.0
            .state
            .borrow_mut()
            .handlers
            .insert((id.to_string(), handler), prio.to_owned());
    }

    pub(crate) fn queue_disable(&self, id: PluginId) {
        self.0.state.borrow_mut().pending_disable.push(id);
    }

    fn take_pending_disables(&self) -> Vec<PluginId> {
        std::mem::take(&mut self.0.state.borrow_mut().pending_disable)
    }
}

fn runtime() -> Result<Rustic, PluginFault> {
    context::runtime().map_err(|e| PluginFault::other(format!("no runtime context: {e:?}")))
}

// ---------------------------------------------------------------------- the bridge plugin

/// The `bukkit-bridge` plugin: JVM lifetime, tick pulse, online-player mirror.
pub struct BridgeInstance {
    bridge: Bridge,
}

impl BridgeInstance {
    fn hello(&self) -> Result<Result<(), String>, PluginFault> {
        let info = context::backend_mut(|b| {
            let players: Vec<Value> = b
                .players_online()
                .unwrap_or_default()
                .into_iter()
                .map(|u| json!({"uuid": u, "name": b.players_name(u).ok().flatten()}))
                .collect();
            json!({
                "server": {
                    "version": b.server_version().unwrap_or_default(),
                    "max_players": b.server_max_players().unwrap_or(20),
                    "motd": b.server_motd().map(Text::into_json).unwrap_or_else(|_| "\"\"".into()),
                    "online_mode": self.bridge.0.config.online_mode,
                    "plugins_dir": self.bridge.0.server_dir.join("plugins"),
                },
                "players": players,
            })
        })
        .unwrap_or_else(|_| json!({"server": {}, "players": []}));
        let mut body = info;
        body["op"] = json!("hello");
        match self.bridge.request(body)? {
            Ok(v) => {
                log::info!(target: "rustic", "[bukkit-bridge] Java {} ready (bridge {})",
                    v["java"].as_str().unwrap_or("?"), v["bridge"].as_str().unwrap_or("?"));
                Ok(Ok(()))
            }
            Err(e) => Ok(Err(e)),
        }
    }
}

impl PluginInstance for BridgeInstance {
    fn on_load(&mut self) -> Result<Result<(), String>, PluginFault> {
        if let Err(e) = self.bridge.start() {
            return Ok(Err(e));
        }
        self.hello()
    }

    fn on_enable(&mut self) -> Result<Result<(), String>, PluginFault> {
        let api = context::api().map_err(|e| PluginFault::other(format!("{e:?}")))?;
        api.subscribe("player-join", Priority::Lowest, false)
            .map_err(PluginFault::other)?;
        api.subscribe("player-quit", Priority::Monitor, false)
            .map_err(PluginFault::other)?;
        api.run_timer(1, 1);
        Ok(Ok(()))
    }

    fn on_disable(&mut self) -> Result<(), PluginFault> {
        self.bridge.stop();
        Ok(())
    }

    fn run_task(&mut self, _task: TaskId) -> Result<(), PluginFault> {
        let rt = runtime()?;
        match self
            .bridge
            .request(json!({"op": "tick", "tick": rt.current_tick()}))?
        {
            Ok(_) => {}
            Err(e) => log::warn!(target: "rustic", "[bukkit-bridge] tick: {e}"),
        }
        for id in self.bridge.take_pending_disables() {
            let r = context::backend_mut(|b| rt.disable(&id, b));
            if let Ok(Err(e)) = r {
                log::warn!(target: "rustic", "[bukkit-bridge] {id} asked to be disabled: {e}");
            }
        }
        Ok(())
    }

    fn execute_command(
        &mut self,
        _command: CommandId,
        _ctx: CommandContext,
    ) -> Result<Result<(), Text>, PluginFault> {
        Ok(Ok(()))
    }

    fn complete_command(
        &mut self,
        _command: CommandId,
        _ctx: CompleteContext,
    ) -> Result<Vec<String>, PluginFault> {
        Ok(Vec::new())
    }

    fn on_message(&mut self, _topic: &str, _payload: &[u8]) -> Result<(), PluginFault> {
        Ok(())
    }

    fn handle_event(&mut self, _handler: HandlerId, event: &Event) -> Result<Event, PluginFault> {
        let ev = serde_json::to_value(event).map_err(|e| PluginFault::other(e.to_string()))?;
        if let Err(e) = self
            .bridge
            .request(json!({"op": "event", "plugin": "", "event": ev}))?
        {
            log::warn!(target: "rustic", "[bukkit-bridge] {}: {e}", event.kind().name());
        }
        Ok(event.clone())
    }
}

// ---------------------------------------------------------------------- one Bukkit plugin

/// One Bukkit plugin, as the core sees it.
pub struct JavaPluginInstance {
    bridge: Bridge,
    id: PluginId,
    desc: Descriptor,
    data_dir: PathBuf,
}

impl JavaPluginInstance {
    fn sender_json(sender: CommandSender) -> Value {
        serde_json::to_value(sender).unwrap_or(Value::Null)
    }

    fn command_name(&self, command: CommandId) -> Option<String> {
        self.bridge
            .0
            .state
            .borrow()
            .commands
            .get(&(self.id.to_string(), command))
            .cloned()
    }
}

impl PluginInstance for JavaPluginInstance {
    fn on_load(&mut self) -> Result<Result<(), String>, PluginFault> {
        Ok(self
            .bridge
            .request(json!({
                "op": "load",
                "plugin": self.desc.name,
                "file": self.desc.file,
                "data_dir": self.data_dir,
            }))?
            .map(|_| ()))
    }

    fn on_enable(&mut self) -> Result<Result<(), String>, PluginFault> {
        let rt = runtime()?;
        crate::host::declare_permissions(&rt, &self.id, &self.desc.permissions);
        let api = rt.api(self.id.clone());
        for spec in self.desc.command_specs() {
            let name = spec.name.clone();
            match api.register_command(spec) {
                Ok(cid) => {
                    self.bridge
                        .0
                        .state
                        .borrow_mut()
                        .commands
                        .insert((self.id.to_string(), cid), name);
                }
                Err(e) => {
                    log::warn!(target: "rustic", "[{}] command /{name} not registered: {e}", self.id)
                }
            }
        }
        match self
            .bridge
            .request(json!({"op": "enable", "plugin": self.desc.name}))?
        {
            Ok(v) if v["enabled"].as_bool() == Some(false) => Ok(Err(v["reason"]
                .as_str()
                .unwrap_or("not enabled")
                .to_owned())),
            Ok(_) => Ok(Ok(())),
            Err(e) => Ok(Err(e)),
        }
    }

    fn on_disable(&mut self) -> Result<(), PluginFault> {
        if self.bridge.is_running()
            && let Err(e) = self
                .bridge
                .request(json!({"op": "disable", "plugin": self.desc.name}))?
        {
            log::warn!(target: "rustic", "[{}] onDisable: {e}", self.id);
        }
        let id = self.id.to_string();
        let mut st = self.bridge.0.state.borrow_mut();
        st.handlers.retain(|(p, _), _| *p != id);
        st.commands.retain(|(p, _), _| *p != id);
        Ok(())
    }

    fn run_task(&mut self, _task: TaskId) -> Result<(), PluginFault> {
        Ok(())
    }

    fn execute_command(
        &mut self,
        command: CommandId,
        ctx: CommandContext,
    ) -> Result<Result<(), Text>, PluginFault> {
        let Some(name) = self.command_name(command) else {
            return Ok(Err(Text::plain("unknown command")));
        };
        let args = match ctx.arg("args") {
            Some(ArgValue::Greedy(s) | ArgValue::String(s) | ArgValue::Word(s)) => s.clone(),
            _ => String::new(),
        };
        match self.bridge.request(json!({
            "op": "command",
            "plugin": self.desc.name,
            "command": name,
            "label": ctx.label,
            "args": args,
            "sender": Self::sender_json(ctx.sender),
        }))? {
            Ok(_) => Ok(Ok(())),
            Err(e) => Ok(Err(Text::plain(e))),
        }
    }

    fn complete_command(
        &mut self,
        command: CommandId,
        ctx: CompleteContext,
    ) -> Result<Vec<String>, PluginFault> {
        let Some(name) = self.command_name(command) else {
            return Ok(Vec::new());
        };
        let raw = ctx.raw.trim_start_matches('/');
        let (label, args) = raw.split_once(' ').unwrap_or((raw, ""));
        match self.bridge.request(json!({
            "op": "complete",
            "plugin": self.desc.name,
            "command": name,
            "label": label,
            "args": args,
            "sender": Self::sender_json(ctx.sender),
        }))? {
            Ok(v) => Ok(serde_json::from_value(v).unwrap_or_default()),
            Err(_) => Ok(Vec::new()),
        }
    }

    fn on_message(&mut self, _topic: &str, _payload: &[u8]) -> Result<(), PluginFault> {
        Ok(())
    }

    fn handle_event(&mut self, handler: HandlerId, event: &Event) -> Result<Event, PluginFault> {
        let prio = self
            .bridge
            .0
            .state
            .borrow()
            .handlers
            .get(&(self.id.to_string(), handler))
            .cloned()
            .unwrap_or_else(|| "normal".into());
        let ev = serde_json::to_value(event).map_err(|e| PluginFault::other(e.to_string()))?;
        match self.bridge.request(json!({
            "op": "event",
            "plugin": self.desc.name,
            "priority": prio,
            "event": ev,
        }))? {
            Ok(v) => serde_json::from_value(v["event"].clone()).map_err(|e| {
                PluginFault::other(format!("the bridge returned a malformed event: {e}"))
            }),
            Err(e) => {
                log::warn!(target: "rustic", "[{}] {}: {e}", self.id, event.kind().name());
                Ok(event.clone())
            }
        }
    }
}

// ---------------------------------------------------------------------- loader

/// [`PluginLoader`] for Bukkit plugins, delegating every other package to `fallback` (the
/// WebAssembly host on rustic-server).
pub struct BridgeLoader {
    bridge: Bridge,
    fallback: Option<Box<dyn PluginLoader>>,
}

impl BridgeLoader {
    /// The bridge.
    pub fn bridge(&self) -> &Bridge {
        &self.bridge
    }
}

impl PluginLoader for BridgeLoader {
    fn load(&mut self, req: &LoadRequest<'_>) -> Result<Box<dyn PluginInstance>, String> {
        let id = req.package.manifest.id.to_string();
        if id == BRIDGE_ID {
            return Ok(Box::new(BridgeInstance {
                bridge: self.bridge.clone(),
            }));
        }
        if let Some(desc) = self.bridge.descriptor(&id) {
            return Ok(Box::new(JavaPluginInstance {
                bridge: self.bridge.clone(),
                id: req.package.manifest.id.clone(),
                desc,
                data_dir: req.data_dir.to_path_buf(),
            }));
        }
        match &mut self.fallback {
            Some(f) => f.load(req),
            None => Err(format!("`{id}` is not a Bukkit plugin known to the bridge")),
        }
    }

    fn unloaded(&mut self, plugin: &PluginId) {
        if plugin.as_str() == BRIDGE_ID {
            return;
        }
        if let Some(desc) = self.bridge.descriptor(plugin) {
            if self.bridge.is_running() {
                let _ = self
                    .bridge
                    .request(json!({"op": "unload", "plugin": desc.name}));
            }
        } else if let Some(f) = &mut self.fallback {
            f.unloaded(plugin);
        }
    }
}

/// Whether `dir` (the plugin folder) holds Bukkit jars.
pub fn has_bukkit_plugins(dir: &Path) -> bool {
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten().any(|e| {
                e.path()
                    .extension()
                    .is_some_and(|x| x.eq_ignore_ascii_case("jar"))
            })
        })
        .unwrap_or(false)
}
