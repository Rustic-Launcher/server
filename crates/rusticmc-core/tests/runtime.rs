//! End-to-end tests of the runtime with native plugins and a recording backend.

use rusticmc_core::archive::{PluginPackage, RpluginBuilder};
use rusticmc_core::clock::{Clock, FakeClock};
use rusticmc_core::commands::build::{arg, command, dyn_arg, lit, usage, usage_perm};
use rusticmc_core::config::ServerConfig;
use rusticmc_core::context;
use rusticmc_core::lifecycle::PluginFault;
use rusticmc_core::manifest::{MANIFEST_FILE, Manifest};
use rusticmc_core::model::*;
use rusticmc_core::native::{NativeLoader, NativePlugin};
use rusticmc_core::permission::PermissionProvider;
use rusticmc_core::permissions_engine::PermissionEngine;
use rusticmc_core::services::{ECONOMY_IFACE, PERMISSION_PROVIDER_IFACE};
use rusticmc_core::{Backend, CommandOutcome, PluginId, PluginState, Rustic};

use std::cell::RefCell;
use std::collections::BTreeSet;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

// ---------------------------------------------------------------- helpers

#[derive(Default)]
struct TestBackend {
    players: Vec<(Uuid, String, bool, Location)>,
    messages: Vec<(Option<Uuid>, String)>,
    next_id: u32,
    open_menus: BTreeSet<MenuId>,
    bars: BTreeSet<BarId>,
    channels: BTreeSet<String>,
    chunks: BTreeSet<(String, i32, i32)>,
    disabled: Vec<String>,
    commands_changed: usize,
}

impl TestBackend {
    fn with_players() -> Self {
        let loc = |x: f64| Location {
            world: "minecraft:overworld".into(),
            x,
            y: 64.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
        };
        Self {
            players: vec![
                (ALICE, "Alice".into(), true, loc(0.0)),
                (BOB, "Bob".into(), false, loc(5.0)),
            ],
            ..Self::default()
        }
    }

    fn said(&self, who: Option<Uuid>) -> Vec<String> {
        self.messages
            .iter()
            .filter(|m| m.0 == who)
            .map(|m| m.1.clone())
            .collect()
    }

    fn last(&self, who: Option<Uuid>) -> String {
        self.said(who).last().cloned().unwrap_or_default()
    }
}

const ALICE: Uuid = Uuid { hi: 0, lo: 1 }; // op
const BOB: Uuid = Uuid { hi: 0, lo: 2 }; // not op

impl Backend for TestBackend {
    fn console_message(&mut self, message: &Text) -> Result<(), HostError> {
        self.messages.push((None, message.to_plain()));
        Ok(())
    }
    fn players_send_message(&mut self, player: Uuid, message: &Text) -> Result<(), HostError> {
        self.messages.push((Some(player), message.to_plain()));
        Ok(())
    }
    fn players_online(&mut self) -> Result<Vec<Uuid>, HostError> {
        Ok(self.players.iter().map(|p| p.0).collect())
    }
    fn players_by_name(&mut self, name: &str) -> Result<Option<Uuid>, HostError> {
        Ok(self
            .players
            .iter()
            .find(|p| p.1.eq_ignore_ascii_case(name))
            .map(|p| p.0))
    }
    fn players_name(&mut self, player: Uuid) -> Result<Option<String>, HostError> {
        Ok(self
            .players
            .iter()
            .find(|p| p.0 == player)
            .map(|p| p.1.clone()))
    }
    fn players_is_online(&mut self, player: Uuid) -> Result<bool, HostError> {
        Ok(self.players.iter().any(|p| p.0 == player))
    }
    fn players_is_op(&mut self, player: Uuid) -> Result<bool, HostError> {
        Ok(self.players.iter().any(|p| p.0 == player && p.2))
    }
    fn players_get_location(&mut self, player: Uuid) -> Result<Location, HostError> {
        self.players
            .iter()
            .find(|p| p.0 == player)
            .map(|p| p.3.clone())
            .ok_or(HostError::NoSuchPlayer)
    }
    fn worlds_all(&mut self) -> Result<Vec<String>, HostError> {
        Ok(vec!["minecraft:overworld".into()])
    }
    fn server_version(&mut self) -> Result<String, HostError> {
        Ok("Test engine".into())
    }
    fn menus_open(&mut self, _player: Uuid, _spec: &MenuSpec) -> Result<MenuId, HostError> {
        self.next_id += 1;
        self.open_menus.insert(self.next_id);
        Ok(self.next_id)
    }
    fn menus_close(&mut self, menu: MenuId) -> Result<(), HostError> {
        self.open_menus.remove(&menu);
        Ok(())
    }
    fn bossbars_create(
        &mut self,
        _t: &Text,
        _c: BarColor,
        _s: BarStyle,
        _p: f32,
    ) -> Result<BarId, HostError> {
        self.next_id += 1;
        self.bars.insert(self.next_id);
        Ok(self.next_id)
    }
    fn bossbars_remove(&mut self, bar: BarId) -> Result<(), HostError> {
        self.bars.remove(&bar);
        Ok(())
    }
    fn messaging_register_incoming(&mut self, channel: &str) -> Result<(), HostError> {
        self.channels.insert(channel.to_owned());
        Ok(())
    }
    fn messaging_unregister_incoming(&mut self, channel: &str) -> Result<(), HostError> {
        self.channels.remove(channel);
        Ok(())
    }
    fn worlds_keep_chunk_loaded(
        &mut self,
        world: &str,
        cx: i32,
        cz: i32,
        keep: bool,
    ) -> Result<(), HostError> {
        if keep {
            self.chunks.insert((world.to_owned(), cx, cz));
        } else {
            self.chunks.remove(&(world.to_owned(), cx, cz));
        }
        Ok(())
    }
    fn plugin_disabled(&mut self, plugin: &str) {
        self.disabled.push(plugin.to_owned());
    }
    fn commands_changed(&mut self) {
        self.commands_changed += 1;
    }
}

fn manifest_text(id: &str, version: &str, extra: &str) -> String {
    format!(
        "[plugin]\nid = \"{id}\"\nname = \"{id}\"\nversion = \"{version}\"\napi = \"0.1\"\n\
         authors = [\"t\"]\nlicense = \"MIT\"\nentry = \"{id}.wasm\"\n{extra}"
    )
}

fn pkg(id: &str, extra: &str) -> PluginPackage {
    let m = Manifest::parse(&manifest_text(id, "1.0.0", extra)).unwrap();
    PluginPackage::in_memory(m, vec![], None, Default::default()).unwrap()
}

struct Env {
    rt: Rustic,
    loader: NativeLoader,
    b: TestBackend,
    clock: Arc<FakeClock>,
    _dir: tempfile::TempDir,
}

fn env_with(config: ServerConfig) -> Env {
    let dir = tempfile::tempdir().unwrap();
    let loader = NativeLoader::new();
    let clock = Arc::new(FakeClock::new());
    let rt = Rustic::builder(dir.path(), loader.boxed())
        .config(config)
        .clock(clock.clone())
        .permissions(PermissionEngine::default())
        .build()
        .unwrap();
    Env {
        rt,
        loader,
        b: TestBackend::with_players(),
        clock,
        _dir: dir,
    }
}

fn env() -> Env {
    env_with(ServerConfig::default())
}

impl Env {
    fn start(&mut self, packages: Vec<PluginPackage>) {
        let r = self.rt.load_packages(packages, &mut self.b);
        assert!(r.failed.is_empty(), "{:?}", r.failed);
        for (id, res) in self.rt.enable_all(&mut self.b) {
            res.unwrap_or_else(|e| panic!("{id}: {e}"));
        }
    }

    fn state(&self, id: &str) -> PluginState {
        self.rt.state(id).unwrap()
    }

    fn cmd(&mut self, sender: CommandSender, line: &str) -> CommandOutcome {
        self.rt.dispatch_command(sender, line, &mut self.b)
    }
}

type Log = Rc<RefCell<Vec<String>>>;

fn join() -> PlayerJoin {
    PlayerJoin {
        player: ALICE,
        name: "Alice".into(),
        join_message: Some(Text::plain("Alice joined")),
    }
}

fn damage(amount: f32) -> EntityDamage {
    EntityDamage {
        target: EntityRef {
            id: 7,
            uuid: Uuid::new(9, 9),
            kind: "minecraft:zombie".into(),
        },
        cause: "minecraft:player_attack".into(),
        attacker: None,
        amount,
        cancelled: false,
    }
}

// ---------------------------------------------------------------- lifecycle

#[test]
fn lifecycle_order_and_dependencies() {
    let mut e = env();
    let log: Log = Rc::default();
    for id in ["economy", "homes", "chat", "lonely"] {
        let l = log.clone();
        let l2 = log.clone();
        let l3 = log.clone();
        e.loader.register(id, move || {
            let (a, b, c) = (l.clone(), l2.clone(), l3.clone());
            NativePlugin::new()
                .on_load(move |api| {
                    a.borrow_mut().push(format!("load {}", api.id()));
                    Ok(())
                })
                .on_enable(move |api| {
                    b.borrow_mut().push(format!("enable {}", api.id()));
                    Ok(())
                })
                .on_disable(move |api| {
                    c.borrow_mut().push(format!("disable {}", api.id()));
                    Ok(())
                })
        });
    }
    let r = e.rt.load_packages(
        vec![
            pkg(
                "homes",
                "[dependencies]\nrequired = { economy = \"^1\" }\noptional = { chat = \"*\" }",
            ),
            pkg("economy", ""),
            pkg("chat", "[dependencies]\nload_before = [\"economy\"]"),
            pkg("broken", "[dependencies]\nrequired = { ghost = \"*\" }"),
        ],
        &mut e.b,
    );
    assert_eq!(
        r.loaded.iter().map(|p| p.as_str()).collect::<Vec<_>>(),
        ["chat", "economy", "homes"]
    );
    assert_eq!(r.failed.len(), 1);
    assert!(
        r.failed[0]
            .1
            .contains("missing required dependency `ghost`")
    );
    assert_eq!(e.state("broken"), PluginState::Failed);
    assert_eq!(e.state("homes"), PluginState::Loaded);
    e.rt.enable_all(&mut e.b);
    assert_eq!(e.state("homes"), PluginState::Enabled);
    assert_eq!(
        log.borrow().as_slice(),
        [
            "load chat",
            "load economy",
            "load homes",
            "enable chat",
            "enable economy",
            "enable homes"
        ]
    );
    // Disabling economy disables homes first, with a reason.
    log.borrow_mut().clear();
    let disabled = e.rt.disable("economy", &mut e.b).unwrap();
    assert_eq!(
        disabled.iter().map(|p| p.as_str()).collect::<Vec<_>>(),
        ["homes", "economy"]
    );
    assert_eq!(
        log.borrow().as_slice(),
        ["disable homes", "disable economy"]
    );
    let info = e.rt.plugin("homes").unwrap();
    assert_eq!(info.state, PluginState::Disabled);
    assert!(
        info.failure
            .unwrap()
            .contains("required dependency `economy` was disabled")
    );
    // Enabling homes alone fails (dependency not enabled); enabling economy then homes works.
    assert!(e.rt.enable("homes", &mut e.b).is_err());
    e.rt.enable("economy", &mut e.b).unwrap();
    e.rt.enable("homes", &mut e.b).unwrap();
    assert_eq!(e.state("homes"), PluginState::Enabled);
    assert_eq!(
        e.loader.load_count("homes"),
        2,
        "the failed attempt stopped before instantiating"
    );
    // Shutdown disables everything in reverse order.
    log.borrow_mut().clear();
    e.rt.shutdown(&mut e.b);
    assert_eq!(
        log.borrow().as_slice(),
        ["disable homes", "disable economy", "disable chat"]
    );
    assert!(e.rt.is_shut_down());
}

#[test]
fn load_and_enable_errors() {
    let mut e = env();
    e.loader.register("noload", || {
        NativePlugin::new().on_load(|_| Err("nope".into()))
    });
    e.loader.register("noenable", || {
        NativePlugin::new().on_enable(|api| {
            api.subscribe("player-join", Priority::Normal, false)?;
            Err("config invalid".into())
        })
    });
    e.loader.register("needs", NativePlugin::new);
    let r = e.rt.load_packages(
        vec![
            pkg("noload", ""),
            pkg("noenable", ""),
            pkg("unregistered", ""),
            pkg("needs", "[dependencies]\nrequired = { noenable = \"*\" }"),
        ],
        &mut e.b,
    );
    assert_eq!(r.failed.len(), 2, "{:?}", r.failed);
    assert_eq!(e.state("noload"), PluginState::Failed);
    assert!(
        e.rt.plugin("unregistered")
            .unwrap()
            .failure
            .unwrap()
            .contains("no native implementation")
    );
    let res = e.rt.enable_all(&mut e.b);
    assert!(res.iter().all(|(_, r)| r.is_err()), "{res:?}");
    assert_eq!(e.state("noenable"), PluginState::Failed);
    assert!(
        e.rt.plugin("noenable")
            .unwrap()
            .failure
            .unwrap()
            .contains("config invalid")
    );
    assert!(
        !e.rt.has_listeners("player-join"),
        "registrations of a failed enable are removed"
    );
    assert_eq!(
        e.state("needs"),
        PluginState::Disabled,
        "dependent of a failed plugin"
    );
}

#[test]
fn discovery_from_directory_and_reload_rereads_disk() {
    let mut e = env();
    let plugins = e.rt.layout().plugins_dir();
    std::fs::create_dir_all(&plugins).unwrap();
    RpluginBuilder::from_manifest_text(&manifest_text("economy", "1.0.0", ""), vec![0])
        .unwrap()
        .default_config("currency = \"coins\"\n")
        .unwrap()
        .write_to(&plugins.join("economy.rplugin"))
        .unwrap();
    let dev = plugins.join("homes-dev");
    std::fs::create_dir_all(&dev).unwrap();
    std::fs::write(
        dev.join(MANIFEST_FILE),
        manifest_text(
            "homes",
            "1.0.0",
            "[dependencies]\nrequired = { economy = \"*\" }",
        ),
    )
    .unwrap();
    std::fs::write(dev.join("homes.wasm"), b"x").unwrap();
    std::fs::write(plugins.join("bad.rplugin"), b"garbage").unwrap();
    e.loader.register("economy", NativePlugin::new);
    let versions: Rc<RefCell<Vec<String>>> = Rc::default();
    let v2 = versions.clone();
    e.loader.register_with("homes", move |req| {
        v2.borrow_mut()
            .push(req.package.manifest.version.to_string());
        Ok(Box::new(NativePlugin::new()))
    });
    let report = e.rt.load_plugins(&mut e.b);
    assert_eq!(report.loaded.len(), 2, "{report:?}");
    assert_eq!(report.failed.len(), 1);
    assert_eq!(e.rt.discovery_errors().len(), 1);
    e.rt.enable_all(&mut e.b);
    // Default config installed in the data folder on first enable.
    let data = e.rt.plugin("economy").unwrap().data_dir.unwrap();
    assert_eq!(
        std::fs::read_to_string(data.join("config.toml")).unwrap(),
        "currency = \"coins\"\n"
    );
    // Bump the dev plugin's version on disk, reload economy: homes (dependent) comes back.
    std::fs::write(
        dev.join(MANIFEST_FILE),
        manifest_text(
            "homes",
            "1.1.0",
            "[dependencies]\nrequired = { economy = \"*\" }",
        ),
    )
    .unwrap();
    let back = e.rt.reload("economy", &mut e.b).unwrap();
    assert_eq!(
        back.iter().map(|p| p.as_str()).collect::<Vec<_>>(),
        ["economy", "homes"]
    );
    assert_eq!(e.state("homes"), PluginState::Enabled);
    // Reloading homes itself re-reads its manifest.
    e.rt.reload("homes", &mut e.b).unwrap();
    assert_eq!(e.rt.plugin("homes").unwrap().version, "1.1.0");
    assert_eq!(versions.borrow().as_slice(), ["1.0.0", "1.0.0", "1.1.0"]);
    // A broken package on reload leaves the plugin failed with the reason.
    std::fs::write(dev.join(MANIFEST_FILE), "nonsense").unwrap();
    assert!(e.rt.reload("homes", &mut e.b).is_err());
    assert_eq!(e.state("homes"), PluginState::Failed);
}

// ---------------------------------------------------------------- events

#[test]
fn event_priorities_mutations_and_monitor() {
    let mut e = env();
    let order: Log = Rc::default();
    let o = order.clone();
    e.loader.register("a", move || {
        let o = o.clone();
        let handlers: Rc<RefCell<Vec<(HandlerId, &'static str)>>> = Rc::default();
        let h2 = handlers.clone();
        NativePlugin::new()
            .on_enable(move |api| {
                for (p, name) in [
                    (Priority::Monitor, "monitor"),
                    (Priority::High, "high"),
                    (Priority::Lowest, "lowest"),
                    (Priority::Normal, "normal"),
                ] {
                    let id = api.subscribe("entity-damage", p, false)?;
                    h2.borrow_mut().push((id, name));
                }
                api.subscribe("entity-damage", Priority::Highest, true)?; // ignore_cancelled
                Ok(())
            })
            .on_event(move |_, h, ev| {
                let name = handlers
                    .borrow()
                    .iter()
                    .find(|x| x.0 == h)
                    .map(|x| x.1)
                    .unwrap_or("highest-ic");
                o.borrow_mut().push(name.to_owned());
                let Event::EntityDamage(d) = ev else {
                    return Ok(ev.clone());
                };
                let mut d = d.clone();
                match name {
                    "lowest" => d.amount *= 2.0,
                    "normal" => d.amount += 1.0,
                    "high" => {
                        d.cancelled = true;
                        d.cause = "forged".into(); // read-only: ignored
                    }
                    "monitor" => {
                        d.cancelled = false; // monitor: ignored
                        d.amount = 0.0;
                    }
                    _ => {}
                }
                Ok(Event::EntityDamage(d))
            })
    });
    e.start(vec![pkg("a", "")]);
    let out = e.rt.fire_entity_damage(damage(3.0), &mut e.b);
    assert_eq!(
        order.borrow().as_slice(),
        ["lowest", "normal", "high", "monitor"]
    );
    assert_eq!(out.amount, 7.0);
    assert!(out.cancelled);
    assert_eq!(out.cause, "minecraft:player_attack");
    assert!(e.rt.has_listeners("entity-damage"));
    assert!(e.rt.has_listeners_for(EventKind::EntityDamage));
    assert!(!e.rt.has_listeners("player-join"));
    assert!(!e.rt.has_listeners("custom:x"));
    assert_eq!(e.rt.listeners("entity-damage").len(), 5);
}

#[test]
fn subscribe_during_dispatch_applies_next_time() {
    let mut e = env();
    let calls: Rc<RefCell<usize>> = Rc::default();
    let c = calls.clone();
    e.loader.register("a", move || {
        let c = c.clone();
        NativePlugin::new()
            .on_enable(|api| {
                api.subscribe("player-join", Priority::Normal, false)?;
                Ok(())
            })
            .on_event(move |api, h, ev| {
                *c.borrow_mut() += 1;
                // Each call adds a subscription and drops the current one.
                api.subscribe("player-join", Priority::Normal, false)
                    .map_err(PluginFault::other)?;
                api.subscribe("player-join", Priority::Normal, false)
                    .map_err(PluginFault::other)?;
                api.unsubscribe(h);
                Ok(ev.clone())
            })
    });
    e.start(vec![pkg("a", "")]);
    e.rt.fire_player_join(join(), &mut e.b);
    assert_eq!(
        *calls.borrow(),
        1,
        "new subscriptions do not run in the same dispatch"
    );
    e.rt.fire_player_join(join(), &mut e.b);
    assert_eq!(*calls.borrow(), 3);
    assert_eq!(e.rt.listeners("player-join").len(), 4);
}

#[test]
fn custom_events_nesting_depth_and_reentry() {
    let mut e = env();
    let seen: Log = Rc::default();
    let s = seen.clone();
    e.loader.register("relay", move || {
        let s = s.clone();
        NativePlugin::new()
            .on_enable(|api| {
                api.subscribe("custom:emitter:ping", Priority::Normal, false)?;
                api.subscribe("custom:relay:echo", Priority::Normal, false)?;
                Ok(())
            })
            .on_event(move |api, _, ev| {
                let Event::CustomEvent(c) = ev else {
                    return Ok(ev.clone());
                };
                s.borrow_mut().push(format!(
                    "relay got {} from {} {:?}",
                    c.name, c.source_plugin, c.payload
                ));
                if c.name == "emitter:ping" {
                    // Nested dispatch; the emitter is executing so it must be skipped.
                    let cancelled = api.call_custom("echo", vec![9]);
                    s.borrow_mut().push(format!("echo cancelled = {cancelled}"));
                }
                let mut c = c.clone();
                c.cancelled = true;
                Ok(Event::CustomEvent(c))
            })
    });
    let s = seen.clone();
    e.loader.register("emitter", move || {
        let s = s.clone();
        NativePlugin::new()
            .on_enable(|api| {
                api.subscribe("custom:relay:echo", Priority::Normal, false)?;
                api.register_command(command("ping", vec![]))?;
                Ok(())
            })
            .on_command(move |api, _, _| {
                let c = api.call_custom("ping", vec![1, 2]);
                s.borrow_mut().push(format!("ping cancelled = {c}"));
                Ok(Ok(()))
            })
    });
    e.start(vec![pkg("relay", ""), pkg("emitter", "")]);
    assert_eq!(
        e.cmd(CommandSender::Console, "ping"),
        CommandOutcome::Executed
    );
    assert_eq!(
        seen.borrow().as_slice(),
        [
            "relay got emitter:ping from emitter [1, 2]",
            "echo cancelled = false", // relay itself and emitter are executing: skipped
            "ping cancelled = true",
        ]
    );
    // Depth limit: a plugin re-firing a custom event from its handler stops at depth 8.
    let depth: Rc<RefCell<usize>> = Rc::default();
    let d = depth.clone();
    e.loader.register("loop1", move || {
        let d = d.clone();
        NativePlugin::new()
            .on_enable(|api| {
                api.subscribe("custom:loop2:x", Priority::Normal, false)?;
                Ok(())
            })
            .on_event(move |api, _, ev| {
                *d.borrow_mut() += 1;
                api.call_custom("x", vec![]);
                Ok(ev.clone())
            })
    });
    let d = depth.clone();
    e.loader.register("loop2", move || {
        let d = d.clone();
        NativePlugin::new()
            .on_enable(|api| {
                api.subscribe("custom:loop1:x", Priority::Normal, false)?;
                Ok(())
            })
            .on_event(move |api, _, ev| {
                *d.borrow_mut() += 1;
                api.call_custom("x", vec![]);
                Ok(ev.clone())
            })
    });
    e.start(vec![pkg("loop1", ""), pkg("loop2", "")]);
    // Fire loop1:x from outside: loop2 handles it, fires loop2:x, loop1 handles it...
    // but loop1/loop2 are then executing, so the chain stops at the first re-entry.
    let ev = CustomEvent {
        name: "loop1:x".into(),
        source_plugin: "test".into(),
        payload: vec![],
        cancelled: false,
    };
    e.rt.fire_custom_event(ev, &mut e.b);
    assert_eq!(*depth.borrow(), 2);
    assert_eq!(e.state("loop1"), PluginState::Enabled);
}

#[test]
fn nested_dispatch_depth_limit() {
    let mut e = env();
    let depth: Rc<RefCell<u32>> = Rc::default();
    // Ten plugins each relaying custom:chain:<n> to custom:chain:<n+1>.
    let mut packages = Vec::new();
    for n in 0..10u32 {
        let id = format!("p{n}");
        let d = depth.clone();
        e.loader.register(&id, move || {
            let d = d.clone();
            NativePlugin::new()
                .on_enable(move |api| {
                    api.subscribe(&format!("custom:chain:{n}"), Priority::Normal, false)?;
                    Ok(())
                })
                .on_event(move |api, _, ev| {
                    *d.borrow_mut() += 1;
                    api.call_custom(&format!("chain:{}", n + 1), vec![]);
                    Ok(ev.clone())
                })
        });
        packages.push(pkg(&id, ""));
    }
    e.start(packages);
    e.rt.fire_custom_event(
        CustomEvent {
            name: "chain:0".into(),
            source_plugin: "t".into(),
            payload: vec![],
            cancelled: false,
        },
        &mut e.b,
    );
    assert_eq!(*depth.borrow(), 8, "MAX_DISPATCH_DEPTH");
}

// ---------------------------------------------------------------- faults

#[test]
fn faults_disable_only_that_plugin_and_clean_up() {
    let mut e = env();
    e.loader.register("bad", || {
        NativePlugin::new()
            .on_enable(|api| {
                api.subscribe("player-join", Priority::Normal, false)?;
                api.register_command(command("boom", vec![]))?;
                api.run_timer(1, 1);
                api.declare_permission("bad.use", PermissionDefault::Allow, "", &[])?;
                api.provide_service(ECONOMY_IFACE, Priority::Normal)?;
                api.menus_open(
                    ALICE,
                    &MenuSpec {
                        rows: 1,
                        title: Text::plain("m"),
                        items: vec![],
                    },
                )
                .map_err(|e| e.to_string())?;
                api.bossbars_create(&Text::plain("b"), BarColor::Red, BarStyle::Progress, 0.5)
                    .map_err(|e| e.to_string())?;
                api.messaging_register_incoming("bad:chan")?;
                api.worlds_keep_chunk_loaded("minecraft:overworld", 1, 2, true)
                    .map_err(|e| e.to_string())?;
                Ok(())
            })
            .on_event(|_, _, _| {
                Err(PluginFault::trap("unreachable executed").with_backtrace("wasm backtrace: f0"))
            })
    });
    e.loader.register("dependent", NativePlugin::new);
    let good_calls: Rc<RefCell<usize>> = Rc::default();
    let g = good_calls.clone();
    e.loader.register("good", move || {
        let g = g.clone();
        NativePlugin::new()
            .on_enable(|api| {
                api.subscribe("player-join", Priority::Monitor, false)?;
                api.messaging_register_incoming("bad:chan")?;
                Ok(())
            })
            .on_event(move |_, _, ev| {
                *g.borrow_mut() += 1;
                Ok(ev.clone())
            })
    });
    e.start(vec![
        pkg("bad", ""),
        pkg("dependent", "[dependencies]\nrequired = { bad = \"*\" }"),
        pkg("good", ""),
    ]);
    assert_eq!(e.b.open_menus.len(), 1);
    assert_eq!(e.b.bars.len(), 1);
    assert_eq!(e.b.chunks.len(), 1);
    assert!(e.b.channels.contains("bad:chan"));
    assert_eq!(
        e.rt.shared().services().provider(ECONOMY_IFACE).as_deref(),
        Some("bad")
    );
    let gen_before = e.rt.commands_generation();
    let out = e.rt.fire_player_join(join(), &mut e.b);
    assert_eq!(out.join_message, Some(Text::plain("Alice joined")));
    assert_eq!(
        *good_calls.borrow(),
        1,
        "the good plugin still got the event"
    );
    assert_eq!(e.state("bad"), PluginState::Failed);
    assert!(
        e.rt.plugin("bad")
            .unwrap()
            .failure
            .unwrap()
            .contains("trap in event: unreachable executed")
    );
    assert_eq!(e.state("dependent"), PluginState::Disabled);
    assert_eq!(e.state("good"), PluginState::Enabled);
    // Everything of `bad` is gone.
    assert_eq!(e.rt.listeners("player-join").len(), 1);
    assert_eq!(
        e.cmd(CommandSender::Console, "boom"),
        CommandOutcome::UnknownCommand
    );
    assert!(e.rt.commands_generation() > gen_before);
    assert!(e.b.commands_changed > 0);
    assert_eq!(e.rt.shared().scheduler().count("bad"), (0, 0));
    assert!(
        e.rt.with_permissions(|p| p.declared("bad.use"))
            .unwrap()
            .is_none()
    );
    assert_eq!(e.rt.shared().services().provider(ECONOMY_IFACE), None);
    assert!(e.b.open_menus.is_empty());
    assert!(e.b.bars.is_empty());
    assert!(e.b.chunks.is_empty());
    assert!(
        e.b.channels.contains("bad:chan"),
        "still registered by `good`"
    );
    assert!(
        e.b.disabled.contains(&"bad".to_owned()) && e.b.disabled.contains(&"dependent".to_owned())
    );
    // The runtime goes on.
    e.rt.fire_player_join(join(), &mut e.b);
    assert_eq!(*good_calls.borrow(), 2);
    e.rt.disable("good", &mut e.b).unwrap();
    assert!(e.b.channels.is_empty(), "last holder gone");
}

#[test]
fn panics_become_faults() {
    let mut e = env();
    e.loader.register("p", || {
        NativePlugin::new()
            .on_enable(|api| {
                api.register_command(command("crash", vec![]))?;
                Ok(())
            })
            .on_command(|_, _, _| panic!("guest panicked"))
    });
    e.start(vec![pkg("p", "")]);
    match e.cmd(CommandSender::Console, "crash") {
        CommandOutcome::Fault(msg) => assert!(msg.contains("guest panicked"), "{msg}"),
        other => panic!("{other:?}"),
    }
    assert!(e.b.last(None).contains("internal error"));
    assert_eq!(e.state("p"), PluginState::Failed);
    assert!(
        e.rt.plugin("p")
            .unwrap()
            .failure
            .unwrap()
            .contains("panic in command")
    );
}

#[test]
fn self_disable_is_deferred_until_the_call_returns() {
    let mut e = env();
    let log: Log = Rc::default();
    let l = log.clone();
    e.loader.register("selfish", move || {
        let (a, b) = (l.clone(), l.clone());
        NativePlugin::new()
            .on_enable(|api| {
                api.register_command(command("quit", vec![]))?;
                Ok(())
            })
            .on_command(move |api, _, _| {
                let r = api.runtime().clone();
                let mut nb = rusticmc_core::NullBackend;
                // Disabling oneself through the façade from inside one's own call.
                let res = r.disable("selfish", &mut nb);
                a.borrow_mut()
                    .push(format!("disable returned ok={}", res.is_ok()));
                Ok(Ok(()))
            })
            .on_disable(move |_| {
                b.borrow_mut().push("on-disable".into());
                Ok(())
            })
    });
    e.start(vec![pkg("selfish", "")]);
    assert_eq!(
        e.cmd(CommandSender::Console, "quit"),
        CommandOutcome::Executed
    );
    assert_eq!(
        log.borrow().as_slice(),
        ["disable returned ok=true", "on-disable"]
    );
    assert_eq!(e.state("selfish"), PluginState::Disabled);
    // Reloading oneself is refused while executing.
    assert!(e.rt.enable("selfish", &mut e.b).is_ok());
}

// ---------------------------------------------------------------- scheduler

#[test]
fn scheduler_ticks_budget_messages_and_async() {
    let mut cfg = ServerConfig::default();
    cfg.scheduler.max_task_ms_per_tick = 10;
    let mut e = env_with(cfg);
    let ran: Log = Rc::default();
    let r = ran.clone();
    let ids: Rc<RefCell<Vec<TaskId>>> = Rc::default();
    let ids2 = ids.clone();
    e.loader.register("tasks", move || {
        let (r, r2, ids) = (r.clone(), r.clone(), ids2.clone());
        NativePlugin::new()
            .on_enable(move |api| {
                let mut v = ids.borrow_mut();
                v.push(api.run_later(0)); // tick 1
                v.push(api.run_timer(1, 2)); // 1, 3, 5
                v.push(api.run_later(2)); // tick 2
                v.push(api.run_async(100));
                api.post_to_main("hello", b"x".to_vec());
                Ok(())
            })
            .on_task(move |api, t| {
                r.borrow_mut()
                    .push(format!("task {t} at {}", api.current_tick()));
                Ok(())
            })
            .on_message(move |api, topic, payload| {
                r2.borrow_mut()
                    .push(format!("msg {topic} {payload:?} at {}", api.current_tick()));
                Ok(())
            })
    });
    e.start(vec![pkg("tasks", "")]);
    let ids = ids.borrow().clone();
    let (a, t, b, asy) = (ids[0], ids[1], ids[2], ids[3]);
    let r1 = e.rt.tick(&mut e.b);
    assert_eq!((r1.tick, r1.tasks_run, r1.messages), (1, 2, 1));
    e.rt.tick(&mut e.b);
    e.rt.tick(&mut e.b);
    assert_eq!(
        ran.borrow().as_slice(),
        [
            "msg hello [120] at 1".to_owned(),
            format!("task {a} at 1"),
            format!("task {t} at 1"),
            format!("task {b} at 2"),
            format!("task {t} at 3"),
        ]
    );
    // Async: tracked only.
    assert!(e.rt.due_async_tasks(e.clock.now()).is_empty());
    e.clock.advance(Duration::from_millis(100));
    let due = e.rt.due_async_tasks(e.clock.now());
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].task, asy);
    assert_eq!(due[0].job, None);
    e.rt.async_task_finished(asy);
    assert!(!e.rt.shared().scheduler().is_scheduled(asy));
    // Jobs carry a payload from the main instance to the async one.
    let job =
        e.rt.api(PluginId::new("tasks").unwrap())
            .run_async_job("export", vec![7], 0);
    let due = e.rt.due_async_tasks(e.clock.now());
    assert_eq!(due[0].task, job);
    let j = due[0].job.clone().unwrap();
    assert_eq!((j.name.as_str(), j.payload), ("export", vec![7]));
    e.rt.async_task_finished(job);
    // Async api posts to main.
    let async_api = e.rt.shared().async_api(PluginId::new("tasks").unwrap());
    async_api.post_to_main("from-async", vec![1]);
    assert!(!async_api.is_main());
    e.rt.tick(&mut e.b);
    assert!(ran.borrow().iter().any(|l| l == "msg from-async [1] at 4"));
    // Async fault reported by the host disables the plugin at the next tick.
    e.rt.shared().report_fault(
        &PluginId::new("tasks").unwrap(),
        PluginFault::fuel_exhausted("loop"),
    );
    e.rt.tick(&mut e.b);
    assert_eq!(e.state("tasks"), PluginState::Failed);
    assert_eq!(e.rt.shared().scheduler().count("tasks"), (0, 0));
}

#[test]
fn budget_rolls_tasks_over() {
    let mut cfg = ServerConfig::default();
    cfg.scheduler.max_task_ms_per_tick = 10;
    let mut e = env_with(cfg);
    let ran: Rc<RefCell<Vec<(u64, TaskId)>>> = Rc::default();
    let r = ran.clone();
    let clock = e.clock.clone();
    e.loader.register("slow", move || {
        let (r, clock) = (r.clone(), clock.clone());
        NativePlugin::new()
            .on_enable(|api| {
                for _ in 0..4 {
                    api.run_later(1);
                }
                Ok(())
            })
            .on_task(move |api, t| {
                r.borrow_mut().push((api.current_tick(), t));
                clock.advance(Duration::from_millis(6));
                Ok(())
            })
    });
    e.start(vec![pkg("slow", "")]);
    let t1 = e.rt.tick(&mut e.b);
    assert_eq!(
        (t1.tasks_run, t1.tasks_rolled),
        (2, 2),
        "6 ms each, 10 ms budget: third one rolls"
    );
    let t2 = e.rt.tick(&mut e.b);
    assert_eq!((t2.tasks_run, t2.tasks_rolled), (2, 0));
    let ticks: Vec<u64> = ran.borrow().iter().map(|x| x.0).collect();
    assert_eq!(ticks, [1, 1, 2, 2]);
    let order: Vec<TaskId> = ran.borrow().iter().map(|x| x.1).collect();
    let mut sorted = order.clone();
    sorted.sort();
    assert_eq!(order, sorted, "task-id order preserved across the rollover");
}

// ---------------------------------------------------------------- commands

fn homes_plugin(e: &Env, log: Log) {
    e.loader.register("homes", move || {
        let l = log.clone();
        NativePlugin::new()
            .on_enable(|api| {
                api.declare_permission(
                    "homes.admin",
                    PermissionDefault::Op,
                    "admin",
                    &[("homes.other".into(), true)],
                )?;
                let mut c = command(
                    "home",
                    vec![
                        usage(vec![]),
                        usage(vec![dyn_arg("name", ArgKind::Word)]),
                        usage_perm(
                            vec![lit("of"), arg("player", ArgKind::Player)],
                            "homes.other",
                        ),
                        usage(vec![lit("fail")]),
                    ],
                );
                c.aliases = vec!["h".into()];
                c.permission = Some("homes.use".into());
                api.declare_permission("homes.use", PermissionDefault::Allow, "", &[])?;
                api.register_command(c)?;
                let mut s = command("sethome", vec![usage(vec![arg("name", ArgKind::Word)])]);
                s.players_only = true;
                api.register_command(s)?;
                Ok(())
            })
            .on_command(move |api, _id, ctx| {
                l.borrow_mut()
                    .push(format!("{} u{} {:?}", ctx.label, ctx.usage, ctx.args));
                if ctx.usage == 3 {
                    return Ok(Err(Text::plain("no such home")));
                }
                api.reply(ctx.sender, &Text::plain("ok"));
                Ok(Ok(()))
            })
            .on_complete(|_, _, ctx| Ok(vec![format!("{}-base", ctx.partial), "farm".into()]))
    });
}

#[test]
fn command_dispatch_outcomes() {
    let mut e = env();
    let log: Log = Rc::default();
    homes_plugin(&e, log.clone());
    e.start(vec![pkg("homes", "")]);
    let alice = CommandSender::Player(ALICE);
    let bob = CommandSender::Player(BOB);
    assert_eq!(e.cmd(bob, "/home"), CommandOutcome::Executed);
    assert_eq!(e.b.last(Some(BOB)), "ok");
    assert_eq!(e.cmd(bob, "h base"), CommandOutcome::Executed);
    assert_eq!(
        log.borrow().last().unwrap(),
        "h u1 [(\"name\", Word(\"base\"))]"
    );
    // Usage permission: Bob (not op) lacks homes.other (child of homes.admin, op default).
    assert_eq!(e.cmd(bob, "home of Alice"), CommandOutcome::NoPermission);
    assert_eq!(e.cmd(alice, "home of bob"), CommandOutcome::Executed);
    assert!(
        log.borrow()
            .last()
            .unwrap()
            .contains("Player(Uuid { hi: 0, lo: 2 })")
    );
    match e.cmd(bob, "home a b") {
        CommandOutcome::Usage(msg) => {
            assert!(msg.contains("too many arguments"), "{msg}");
            assert!(msg.contains("/home <name>"));
            assert!(
                !msg.contains("/home of <player>"),
                "usages the sender cannot use are hidden"
            );
        }
        other => panic!("{other:?}"),
    }
    match e.cmd(alice, "home fail") {
        CommandOutcome::PluginError(t) => assert_eq!(t.to_plain(), "no such home"),
        other => panic!("{other:?}"),
    }
    assert_eq!(e.b.last(Some(ALICE)), "no such home");
    assert_eq!(
        e.cmd(CommandSender::Console, "sethome x"),
        CommandOutcome::PlayerOnly
    );
    assert_eq!(e.cmd(alice, "sethome x"), CommandOutcome::Executed);
    assert_eq!(e.cmd(alice, "homes:home"), CommandOutcome::Executed);
    assert_eq!(e.cmd(alice, "unknown"), CommandOutcome::UnknownCommand);
    // Deny homes.use explicitly for Bob in the default engine.
    e.rt.with_permissions(|p| {
        p.engine_mut()
            .set_user_node(BOB.as_u128(), "homes.use", Some(false))
            .unwrap()
    });
    assert_eq!(e.cmd(bob, "home"), CommandOutcome::NoPermission);
    assert!(e.b.last(Some(BOB)).contains("permission"));
}

#[test]
fn completion() {
    let mut e = env();
    homes_plugin(&e, Log::default());
    e.start(vec![pkg("homes", "")]);
    let bob = CommandSender::Player(BOB);
    let alice = CommandSender::Player(ALICE);
    let labels = e.rt.complete(bob, "h", &mut e.b);
    assert_eq!(
        labels,
        ["h", "home", "homes:h", "homes:home", "homes:sethome"]
    );
    assert!(
        e.rt.complete(bob, "rus", &mut e.b).is_empty(),
        "/rustic needs rustic.admin"
    );
    assert_eq!(
        e.rt.complete(alice, "/rus", &mut e.b),
        ["rustic", "rustic:rustic"]
    );
    // Bob cannot see `of` (needs homes.other); dynamic suggestions come from the plugin.
    assert_eq!(
        e.rt.complete(bob, "home ", &mut e.b),
        ["fail", "-base", "farm"]
    );
    assert_eq!(e.rt.complete(alice, "home o", &mut e.b), ["of", "o-base"]);
    assert_eq!(e.rt.complete(alice, "home of ", &mut e.b), ["Alice", "Bob"]);
    assert_eq!(e.rt.complete(alice, "home of b", &mut e.b), ["Bob"]);
    assert_eq!(e.rt.complete(alice, "rustic info h", &mut e.b), ["homes"]);
    assert_eq!(e.rt.complete(alice, "rustic tim", &mut e.b), ["timings"]);
}

#[test]
fn command_conflicts_and_exports() {
    let mut e = env();
    for id in ["first", "second"] {
        e.loader.register(id, || {
            NativePlugin::new().on_enable(|api| {
                let mut c = command("warp", vec![usage(vec![arg("to", ArgKind::Word)])]);
                c.aliases = vec!["w".into()];
                api.register_command(c)?;
                Ok(())
            })
        });
    }
    e.start(vec![pkg("first", ""), pkg("second", "")]);
    let tree = e.rt.command_tree();
    let second = tree.iter().find(|c| c.owner == "second").unwrap();
    assert_eq!(second.labels, ["second:w", "second:warp"]);
    let usages = e.rt.horizon_usages();
    assert!(usages.iter().any(|u| u.usage == "warp <to:word>"));
    assert!(usages.iter().any(|u| u.usage == "second:warp <to:word>"));
    assert!(
        usages.iter().any(|u| u.usage == "rustic info <id:word?>"
            && u.permission.as_deref() == Some("rustic.admin"))
    );
    e.rt.disable("first", &mut e.b).unwrap();
    let tree = e.rt.command_tree();
    let second = tree.iter().find(|c| c.owner == "second").unwrap();
    assert_eq!(second.labels, ["second:w", "second:warp", "w", "warp"]);
}

#[test]
fn builtin_rustic_command() {
    let mut e = env();
    e.loader.register("demo", NativePlugin::new);
    e.start(vec![pkg("demo", "")]);
    let alice = CommandSender::Player(ALICE);
    let console = CommandSender::Console;
    assert_eq!(
        e.cmd(CommandSender::Player(BOB), "rustic plugins"),
        CommandOutcome::NoPermission
    );
    assert_eq!(e.cmd(alice, "rustic plugins"), CommandOutcome::Executed);
    assert_eq!(e.b.last(Some(ALICE)), "Plugins (1): demo");
    assert_eq!(e.cmd(console, "rustic info demo"), CommandOutcome::Executed);
    assert!(e.b.last(None).contains("demo 1.0.0 (demo) — enabled"));
    assert_eq!(
        e.cmd(console, "rustic disable demo"),
        CommandOutcome::Executed
    );
    assert_eq!(e.state("demo"), PluginState::Disabled);
    assert_eq!(
        e.cmd(console, "rustic enable demo"),
        CommandOutcome::Executed
    );
    assert_eq!(e.state("demo"), PluginState::Enabled);
    assert_eq!(
        e.cmd(console, "rustic reload demo"),
        CommandOutcome::Executed
    );
    assert_eq!(e.b.last(None), "Reloaded: demo");
    assert!(matches!(
        e.cmd(console, "rustic info nope"),
        CommandOutcome::PluginError(_)
    ));
    assert_eq!(
        e.cmd(console, "rustic timings on"),
        CommandOutcome::Executed
    );
    assert!(e.rt.timings().is_enabled());
    e.cmd(console, "rustic reload demo");
    assert_eq!(e.cmd(console, "rustic timings"), CommandOutcome::Executed);
    assert!(e.b.last(None).contains("demo"), "{}", e.b.last(None));
    assert_eq!(
        e.cmd(console, "rustic timings export"),
        CommandOutcome::Executed
    );
    assert!(e.b.last(None).contains("Timings written to"));
    assert_eq!(
        e.cmd(console, "rustic timings off"),
        CommandOutcome::Executed
    );
    assert_eq!(
        e.cmd(console, "rustic timings reset"),
        CommandOutcome::Executed
    );
    assert_eq!(e.cmd(console, "rustic version"), CommandOutcome::Executed);
    assert!(e.b.last(None).contains("Test engine"));
    assert_eq!(e.cmd(console, "rustic"), CommandOutcome::Executed);
    assert!(e.b.last(None).contains("/rustic plugins"));
    assert!(matches!(
        e.cmd(console, "rustic bogus"),
        CommandOutcome::Usage(_)
    ));
}

// ---------------------------------------------------------------- permissions, services, storage

struct Provider(Rc<RefCell<Vec<String>>>);

impl PermissionProvider for Provider {
    fn check(
        &mut self,
        provider: &PluginId,
        _player: Uuid,
        node: &str,
    ) -> Result<Tristate, ServiceError> {
        self.0.borrow_mut().push(format!("{provider}:{node}"));
        match node {
            "x.allowed" => Ok(Tristate::Allow),
            "x.denied" => Ok(Tristate::Deny),
            "x.broken" => Err(ServiceError::ProviderFailed),
            _ => Ok(Tristate::Undefined),
        }
    }
}

#[test]
fn permission_resolution_with_provider() {
    let mut e = env();
    let calls: Rc<RefCell<Vec<String>>> = Rc::default();
    e.loader.register("perms", || {
        NativePlugin::new().on_load(|api| {
            api.provide_service(PERMISSION_PROVIDER_IFACE, Priority::Normal)?;
            Ok(())
        })
    });
    e.rt.with_permissions(|p| {
        let mut engine = PermissionEngine::default();
        engine
            .set_user_node(BOB.as_u128(), "x.broken", Some(true))
            .unwrap();
        engine
            .set_user_node(BOB.as_u128(), "x.engine", Some(true))
            .unwrap();
        p.set_engine(engine);
    });
    let bob = CommandSender::Player(BOB);
    // Without a provider: default engine, then op default.
    assert!(e.rt.has_permission(bob, "x.engine", &mut e.b));
    assert!(!e.rt.has_permission(bob, "x.allowed", &mut e.b));
    assert!(e.rt.has_permission(CommandSender::Console, "anything", &mut e.b));
    assert!(
        e.rt.has_permission(CommandSender::Player(ALICE), "undeclared.node", &mut e.b),
        "op default"
    );
    e.rt.set_permission_provider(Box::new(Provider(calls.clone())));
    e.start(vec![pkg("perms", "")]);
    assert!(e.rt.has_permission(bob, "x.allowed", &mut e.b));
    assert!(!e.rt.has_permission(bob, "x.denied", &mut e.b));
    assert!(
        !e.rt.has_permission(bob, "x.engine", &mut e.b),
        "the provider replaces the engine"
    );
    assert!(
        e.rt.has_permission(bob, "x.broken", &mut e.b),
        "provider failure falls back to the engine"
    );
    assert!(calls.borrow().iter().all(|c| c.starts_with("perms:")));
    e.rt.disable("perms", &mut e.b).unwrap();
    assert!(e.rt.has_permission(bob, "x.engine", &mut e.b));
}

#[test]
fn services_and_change_notifications() {
    let mut e = env();
    let changes: Arc<Mutex<Vec<String>>> = Arc::default();
    let c2 = changes.clone();
    e.rt.shared().services().on_change(move |c| {
        c2.lock()
            .unwrap()
            .push(format!("{:?}->{:?}", c.old.as_deref(), c.new.as_deref()));
    });
    for (id, p) in [("eco1", Priority::Normal), ("eco2", Priority::High)] {
        e.loader.register(id, move || {
            NativePlugin::new().on_load(move |api| {
                api.provide_service(ECONOMY_IFACE, p)?;
                assert!(api.provide_service("not-an-iface", p).is_err());
                Ok(())
            })
        });
    }
    e.loader.register("shop", || {
        NativePlugin::new().on_enable(|api| {
            assert_eq!(api.service_provider(ECONOMY_IFACE).as_deref(), Some("eco2"));
            Ok(())
        })
    });
    e.start(vec![pkg("eco1", ""), pkg("eco2", ""), pkg("shop", "")]);
    e.rt.disable("eco2", &mut e.b).unwrap();
    assert_eq!(
        e.rt.shared().services().provider(ECONOMY_IFACE).as_deref(),
        Some("eco1")
    );
    assert_eq!(
        changes.lock().unwrap().as_slice(),
        [
            "None->Some(\"eco1\")",
            "Some(\"eco1\")->Some(\"eco2\")",
            "Some(\"eco2\")->Some(\"eco1\")"
        ]
    );
}

#[test]
fn storage_survives_reload_and_player_data_is_namespaced() {
    let mut e = env();
    e.loader.register("bank", || {
        NativePlugin::new()
            .on_enable(|api| {
                let n = api.storage_get("boots").map(|v| v[0]).unwrap_or(0);
                api.storage_set("boots", vec![n + 1]);
                api.player_data_set(ALICE, "coins", vec![42]);
                Ok(())
            })
            .on_load(|_| Ok(()))
    });
    e.loader.register("other", || {
        NativePlugin::new().on_enable(|api| {
            assert!(
                api.player_data_get(ALICE, "coins").is_none(),
                "namespaced per plugin"
            );
            assert!(api.storage_get("boots").is_none());
            Ok(())
        })
    });
    e.start(vec![pkg("bank", ""), pkg("other", "")]);
    e.rt.reload("bank", &mut e.b).unwrap();
    let api = e.rt.api(PluginId::new("bank").unwrap());
    assert_eq!(api.storage_get("boots"), Some(vec![2]));
    assert_eq!(api.storage_keys(""), ["boots"]);
    assert_eq!(api.player_data_get(ALICE, "coins"), Some(vec![42]));
    // Player quit flushes and evicts the player's data; it is read back from disk.
    e.rt.fire_player_quit(
        PlayerQuit {
            player: ALICE,
            name: "Alice".into(),
            quit_message: None,
        },
        &mut e.b,
    );
    let dir = e.rt.plugin("bank").unwrap().data_dir.unwrap();
    assert!(
        dir.join(".rustic")
            .join("players")
            .join(format!("{ALICE}.rkv"))
            .exists()
    );
    assert_eq!(api.player_data_get(ALICE, "coins"), Some(vec![42]));
    assert!(api.player_data_delete(ALICE, "coins"));
    assert!(api.storage_delete("boots"));
}

// ---------------------------------------------------------------- context, menus, channels, watchdog

#[test]
fn context_inside_and_outside_calls() {
    let mut e = env();
    let seen: Log = Rc::default();
    let s = seen.clone();
    e.loader.register("ctx", move || {
        let s = s.clone();
        NativePlugin::new()
            .on_enable(|api| {
                api.subscribe("player-join", Priority::Normal, false)?;
                Ok(())
            })
            .on_event(move |api, _, ev| {
                let call = context::current_call().unwrap();
                let names = context::backend_call(|b| b.players_online()).unwrap();
                let tracked = api.runtime().tracker().current().unwrap();
                s.borrow_mut().push(format!(
                    "{} main={} online={} executing={} {}",
                    call.plugin_id,
                    call.is_main,
                    names.len(),
                    tracked.plugin,
                    tracked.kind.label()
                ));
                // Nested backend access is refused, not aliased.
                let nested = context::backend_mut(|_| context::backend_mut(|_| ()));
                s.borrow_mut().push(format!("nested={nested:?}"));
                Ok(ev.clone())
            })
    });
    e.start(vec![pkg("ctx", "")]);
    e.rt.fire_player_join(join(), &mut e.b);
    assert_eq!(
        seen.borrow()[0],
        "ctx main=true online=2 executing=ctx event"
    );
    assert!(seen.borrow()[1].starts_with("nested=Ok(Err(Other("));
    assert!(context::api().is_err());
    assert!(context::backend_mut(|_| ()).is_err());
    assert!(e.rt.tracker().current().is_none());
}

#[test]
fn menus_and_plugin_messages() {
    let mut e = env();
    let got: Log = Rc::default();
    let menu: Rc<RefCell<MenuId>> = Rc::default();
    let (g, m) = (got.clone(), menu.clone());
    e.loader.register("menus", move || {
        let (g, m) = (g.clone(), m.clone());
        NativePlugin::new()
            .on_enable(move |api| {
                api.subscribe("inventory-click", Priority::Normal, false)?;
                api.subscribe("plugin-message", Priority::Normal, false)?;
                api.messaging_register_incoming("menus:sync")?;
                assert!(api.messaging_register_incoming("bad channel").is_err());
                *m.borrow_mut() = api
                    .menus_open(
                        ALICE,
                        &MenuSpec {
                            rows: 3,
                            title: Text::plain("Shop"),
                            items: vec![],
                        },
                    )
                    .map_err(|e| e.to_string())?;
                assert!(
                    api.menus_open(
                        ALICE,
                        &MenuSpec {
                            rows: 7,
                            title: Text::plain("x"),
                            items: vec![]
                        }
                    )
                    .is_err()
                );
                Ok(())
            })
            .on_event(move |_, _, ev| {
                match ev {
                    Event::InventoryClick(c) => g
                        .borrow_mut()
                        .push(format!("click cancelled={}", c.cancelled)),
                    Event::PluginMessage(p) => g.borrow_mut().push(format!("msg {}", p.channel)),
                    _ => {}
                }
                Ok(ev.clone())
            })
    });
    let g = got.clone();
    e.loader.register("intruder", move || {
        let g = g.clone();
        NativePlugin::new()
            .on_enable(|api| {
                assert!(
                    api.menus_set_title(1, &Text::plain("hijack")).is_err(),
                    "not its menu"
                );
                api.subscribe("inventory-click", Priority::Monitor, false)?;
                Ok(())
            })
            .on_event(move |_, _, ev| {
                if let Event::InventoryClick(c) = ev {
                    g.borrow_mut()
                        .push(format!("intruder sees menu {:?}", c.menu));
                }
                Ok(ev.clone())
            })
    });
    e.start(vec![pkg("menus", ""), pkg("intruder", "")]);
    let menu_id = *menu.borrow();
    let click = InventoryClick {
        player: ALICE,
        menu: Some(menu_id),
        slot: 0,
        click: ClickKind::Left,
        hotbar_button: -1,
        item: ItemStack::empty(),
        cursor: ItemStack::empty(),
        cancelled: false,
    };
    let out = e.rt.fire_inventory_click(click.clone(), &mut e.b);
    assert!(out.cancelled, "plugin menus are cancelled by default");
    let out = e.rt.fire_inventory_click(
        InventoryClick {
            menu: None,
            ..click
        },
        &mut e.b,
    );
    assert!(!out.cancelled);
    for channel in ["menus:sync", "other:chan"] {
        e.rt.fire_plugin_message(
            PluginMessage {
                player: ALICE,
                channel: channel.into(),
                data: vec![],
            },
            &mut e.b,
        );
    }
    assert_eq!(
        got.borrow().as_slice(),
        [
            "click cancelled=true",
            "intruder sees menu None",
            "click cancelled=false",
            "intruder sees menu None",
            "msg menus:sync"
        ]
    );
    // Closing the menu (player side) untracks it: disabling no longer closes it.
    e.rt.fire_inventory_close(
        InventoryClose {
            player: ALICE,
            menu: Some(menu_id),
        },
        &mut e.b,
    );
    e.b.open_menus.insert(menu_id);
    e.rt.disable("menus", &mut e.b).unwrap();
    assert!(e.b.open_menus.contains(&menu_id));
}

#[test]
fn timings_are_collected_per_plugin() {
    let mut cfg = ServerConfig::default();
    cfg.timings.enabled = true;
    let mut e = env_with(cfg);
    e.loader.register("t", || {
        NativePlugin::new()
            .on_enable(|api| {
                api.subscribe("player-join", Priority::Normal, false)?;
                api.run_timer(1, 1);
                Ok(())
            })
            .on_event(|_, _, ev| Ok(ev.clone()))
    });
    e.start(vec![pkg("t", "")]);
    for _ in 0..3 {
        e.rt.fire_player_join(join(), &mut e.b);
        e.rt.tick(&mut e.b);
        e.rt.record_server_tick(Duration::from_millis(50));
    }
    let report = e.rt.timings().report();
    let p = &report.plugins[0];
    assert_eq!(p.plugin, "t");
    let kinds: Vec<String> = p
        .sections
        .iter()
        .map(|s| format!("{}:{}:{}", s.kind.label(), s.name, s.count))
        .collect();
    assert!(
        kinds.contains(&"event:player-join:3".to_owned()),
        "{kinds:?}"
    );
    assert!(
        kinds
            .iter()
            .any(|k| k.starts_with("task:timer#") && k.ends_with(":3")),
        "{kinds:?}"
    );
    assert!(kinds.contains(&"lifecycle:on-enable:1".to_owned()));
    assert_eq!(report.ticks, 3);
}

#[test]
fn api_refuses_registration_when_not_active() {
    let mut e = env();
    e.loader.register("x", NativePlugin::new);
    e.start(vec![pkg("x", "")]);
    e.rt.disable("x", &mut e.b).unwrap();
    let api = e.rt.api(PluginId::new("x").unwrap());
    assert!(
        api.subscribe("player-join", Priority::Normal, false)
            .is_err()
    );
    assert!(api.register_command(command("x", vec![])).is_err());
    assert!(
        api.provide_service(ECONOMY_IFACE, Priority::Normal)
            .is_err()
    );
    assert!(api.subscribe("nope", Priority::Normal, false).is_err());
    assert!(!api.self_meta().enabled);
    assert_eq!(api.plugins().len(), 1);
}
