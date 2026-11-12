//! The plugin loader, and two built-in native test plugins.
//!
//! [`Loader`] gives the runtime WebAssembly plugins through `rusticmc-host`'s `WasmHost` (every
//! package of `plugins/`) and plugins registered in-process by id (`rusticmc_core::native`). With
//! `RUSTIC_DEMO_PLUGINS=1`, `rustic-server` also loads two built-in plugins made of closures, used
//! to check the whole path on a running server without building WebAssembly:
//!
//! * `rustic-demo` listens to joins, quits, chat, block breaks and places, deaths and server-list
//!   pings (logging each, changing the join line and the ping's motd, cancelling chat that says
//!   `cancelme`), registers `/demo [name]`, and runs a timer every 10 s;
//! * `rustic-buggy` panics in its join handler: the core disables it and the server goes on;
//! * `rustic-ui` shows what plugins put on screens: a boss bar and a sidebar objective for every
//!   player who joins, a one-row menu two seconds later (its clicks are cancelled by default and
//!   close it), and block breaks that drop nothing (`drop-items = false`).
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use rusticmc_core::PluginId;
use rusticmc_core::archive::PluginPackage;
use rusticmc_core::lifecycle::{LoadRequest, PluginInstance, PluginLoader};
use rusticmc_core::manifest::Manifest;
use rusticmc_core::model::{
    ArgKind, ArgSpec, CommandSpec, Event, LogLevel, NodeSpec, Priority, Text, Usage,
};
use rusticmc_core::native::{NativeLoader, NativePlugin};

/// Whether the built-in test plugins load (`RUSTIC_DEMO_PLUGINS=1`).
pub fn enabled() -> bool {
    std::env::var_os("RUSTIC_DEMO_PLUGINS").is_some_and(|v| v != "0" && !v.is_empty())
}

/// The loader `rustic-server` gives the runtime: native plugins by id, WebAssembly for the rest.
#[derive(Clone)]
pub struct Loader {
    native: NativeLoader,
    native_ids: Rc<RefCell<HashSet<String>>>,
    wasm: Rc<RefCell<Box<dyn PluginLoader>>>,
}

impl Loader {
    pub fn new(wasm: Box<dyn PluginLoader>) -> Self {
        Self {
            native: NativeLoader::new(),
            native_ids: Rc::default(),
            wasm: Rc::new(RefCell::new(wasm)),
        }
    }

    fn register(&self, id: &str, factory: fn() -> NativePlugin) {
        self.native_ids.borrow_mut().insert(id.to_owned());
        self.native.register(id, factory);
    }
}

impl PluginLoader for Loader {
    fn load(&mut self, req: &LoadRequest<'_>) -> Result<Box<dyn PluginInstance>, String> {
        let id = req.package.manifest.id.to_string();
        if self.native_ids.borrow().contains(&id) {
            self.native.load(req)
        } else {
            self.wasm.borrow_mut().load(req)
        }
    }

    fn unloaded(&mut self, plugin: &PluginId) {
        if !self.native_ids.borrow().contains(plugin.as_str()) {
            self.wasm.borrow_mut().unloaded(plugin);
        }
    }
}

fn package(id: &str) -> PluginPackage {
    let manifest = Manifest::parse(&format!(
        "[plugin]\nid = \"{id}\"\nname = \"{id}\"\nversion = \"0.1.0\"\napi = \"0.1\"\n\
         authors = [\"Rustic\"]\nlicense = \"MIT OR Apache-2.0\"\nentry = \"{id}.wasm\"\n"
    ))
    .expect("built-in manifest");
    PluginPackage::in_memory(manifest, Vec::new(), None, Default::default())
        .expect("built-in package")
}

/// Registers the built-in plugins with `loader` and returns their packages.
pub fn packages(loader: &Loader) -> Vec<PluginPackage> {
    loader.register("rustic-demo", demo);
    loader.register("rustic-buggy", buggy);
    loader.register("rustic-ui", ui_demo);
    vec![
        package("rustic-demo"),
        package("rustic-buggy"),
        package("rustic-ui"),
    ]
}

const EVENTS: [&str; 10] = [
    "entity-damage",
    "entity-death",
    "player-interact",
    "player-join",
    "player-quit",
    "player-chat",
    "block-break",
    "block-place",
    "player-death",
    "server-list-ping",
];

fn demo() -> NativePlugin {
    NativePlugin::new()
        .on_enable(|api| {
            for event in EVENTS {
                api.subscribe(event, Priority::Normal, false)?;
            }
            api.register_command(CommandSpec {
                name: "demo".into(),
                aliases: Vec::new(),
                description: "Rustic's demo greeting".into(),
                permission: None,
                players_only: false,
                usages: vec![
                    Usage {
                        nodes: vec![NodeSpec::Argument(ArgSpec::new("name", ArgKind::Word))],
                        permission: None,
                        description: String::new(),
                    },
                    Usage::default(),
                ],
            })?;
            api.run_timer(200, 200);
            api.log(LogLevel::Info, "rustic-demo enabled");
            Ok(())
        })
        .on_task(|api, task| {
            api.log(
                LogLevel::Info,
                &format!("demo timer {task:?} at tick {}", api.current_tick()),
            );
            Ok(())
        })
        .on_command(|api, _command, ctx| {
            let who = ctx
                .args
                .iter()
                .find(|(name, _)| name == "name")
                .map(|(_, v)| format!("{v:?}"))
                .unwrap_or_else(|| "world".into());
            api.log(
                LogLevel::Info,
                &format!("/demo from {:?}: {}", ctx.sender, ctx.raw),
            );
            api.reply(
                ctx.sender,
                &Text::colored(format!("Hello, {who}! (Rustic)"), "gold"),
            );
            Ok(Ok(()))
        })
        .on_event(|api, _handler, event| {
            api.log(
                LogLevel::Info,
                &format!("event {}: {}", event.name(), summary(event)),
            );
            let mut event = event.clone();
            match &mut event {
                Event::PlayerJoin(join) => {
                    join.join_message = Some(Text::colored(
                        format!("{} joined a Rustic server", join.name),
                        "aqua",
                    ));
                }
                Event::ServerListPing(ping) => {
                    ping.motd = Text::colored("A Rustic server (demo plugin)", "green");
                }
                Event::PlayerChat(chat) if chat.message.contains("cancelme") => {
                    chat.cancelled = true;
                }
                _ => {}
            }
            Ok(event)
        })
}

fn summary(event: &Event) -> String {
    match event {
        Event::PlayerJoin(e) => e.name.clone(),
        Event::PlayerQuit(e) => e.name.clone(),
        Event::PlayerChat(e) => format!("<{}> {}", e.name, e.message),
        Event::BlockBreak(e) => format!("{} at {:?} in {}", e.block.id, e.pos, e.world),
        Event::BlockPlace(e) => format!(
            "{}{:?} at {:?} with {:?}",
            e.block.id, e.block.properties, e.pos, e.hand
        ),
        Event::PlayerDeath(e) => format!("{:?} keep_inventory={}", e.player, e.keep_inventory),
        Event::ServerListPing(e) => format!("from {} ({} online)", e.address, e.online_players),
        Event::EntityDamage(e) => format!(
            "{} {} hit by {} for {} ({})",
            e.target.kind,
            e.target.uuid,
            e.attacker
                .as_ref()
                .map_or("nothing".to_owned(), |a| format!("{} {}", a.kind, a.uuid)),
            e.amount,
            e.cause
        ),
        Event::EntityDeath(e) => format!(
            "{} {} killed by {:?}, drops {:?}",
            e.entity.kind,
            e.entity.uuid,
            e.killer,
            e.drops
                .iter()
                .map(|d| format!("{}x{}", d.count, d.id))
                .collect::<Vec<_>>()
        ),
        Event::PlayerInteract(e) => format!(
            "{:?} {:?} {:?} with {}",
            e.action, e.block, e.face, e.item.id
        ),
        _ => String::new(),
    }
}

fn buggy() -> NativePlugin {
    NativePlugin::new()
        .on_enable(|api| {
            api.subscribe("player-join", Priority::Normal, false)?;
            Ok(())
        })
        .on_event(|_api, _handler, _event| panic!("rustic-buggy panics on purpose"))
}

fn ui_demo() -> NativePlugin {
    use rusticmc_core::context;
    use rusticmc_core::model::{BarColor, BarStyle, DisplaySlot, ItemStack, MenuSpec, Uuid};
    use std::cell::{Cell, RefCell};
    let bar = Rc::new(Cell::new(None));
    let waiting: Rc<RefCell<Vec<Uuid>>> = Rc::default();
    let (bar_e, bar_ev) = (bar.clone(), bar);
    let (waiting_ev, waiting_task) = (waiting.clone(), waiting);
    NativePlugin::new()
        .on_enable(move |api| {
            for event in [
                "player-join",
                "inventory-click",
                "inventory-close",
                "block-break",
            ] {
                api.subscribe(event, Priority::Normal, false)?;
            }
            let id = api
                .bossbars_create(
                    &Text::colored("Rustic boss bar", "gold"),
                    BarColor::Purple,
                    BarStyle::Notched10,
                    0.75,
                )
                .map_err(|e| e.to_string())?;
            bar_e.set(Some(id));
            let title = Text::colored("Rustic sidebar", "aqua");
            let made = context::backend_call(|b| {
                b.scoreboards_create_objective("rustic_ui", &title, "dummy")?;
                b.scoreboards_set_display(DisplaySlot::Sidebar, Some("rustic_ui"))
            });
            api.log(
                LogLevel::Info,
                &format!("rustic-ui enabled: bar {id}, sidebar {made:?}"),
            );
            Ok(())
        })
        .on_event(move |api, _handler, event| {
            let mut event = event.clone();
            match &mut event {
                Event::PlayerJoin(join) => {
                    if let Some(bar) = bar_ev.get() {
                        let shown = api.bossbars_show(bar, join.player);
                        api.log(
                            LogLevel::Info,
                            &format!("boss bar {bar} shown to {}: {shown:?}", join.name),
                        );
                    }
                    let name = join.name.clone();
                    let scored =
                        context::backend_call(|b| b.scoreboards_set_score("rustic_ui", &name, 42));
                    api.log(
                        LogLevel::Info,
                        &format!("sidebar score of {name}: {scored:?}"),
                    );
                    waiting_ev.borrow_mut().push(join.player);
                    api.run_later(40);
                }
                Event::InventoryClick(click) if click.menu.is_some() => {
                    api.log(
                        LogLevel::Info,
                        &format!(
                            "menu {:?} click slot {} {:?} on {} (cancelled by default: {})",
                            click.menu, click.slot, click.click, click.item.id, click.cancelled
                        ),
                    );
                    if let Some(menu) = click.menu {
                        api.menus_close(menu);
                    }
                }
                Event::InventoryClose(close) => {
                    api.log(LogLevel::Info, &format!("menu {:?} closed", close.menu));
                }
                Event::BlockBreak(b) => {
                    b.drop_items = false;
                    api.log(
                        LogLevel::Info,
                        &format!("{} at {:?} breaks without drops", b.block.id, b.pos),
                    );
                }
                _ => {}
            }
            Ok(event)
        })
        .on_task(move |api, _task| {
            for player in waiting_task.borrow_mut().drain(..) {
                let spec = MenuSpec {
                    rows: 1,
                    title: Text::colored("Rustic menu", "dark_purple"),
                    items: vec![
                        (4, ItemStack::new("minecraft:diamond", 1)),
                        (8, ItemStack::new("minecraft:barrier", 1)),
                    ],
                };
                let opened = api.menus_open(player, &spec);
                api.log(
                    LogLevel::Info,
                    &format!("menu opened for {player}: {opened:?}"),
                );
            }
            Ok(())
        })
}
