//! One instantiated component in its own [`Store`]: typed exports looked up by name, the
//! per-call sandbox setup (fuel, epoch deadline) and the translation of wasmtime errors into
//! [`PluginFault`]s.

use crate::bindings::exports::rustic::services::{chat_meta, economy, permission_provider};
use crate::bindings::rustic::api::{commands as wc, event_types as we};
use crate::convert::Cv;
use crate::engine::{Compiled, HostCore};
use crate::output::panic_message;
use crate::state::{HostState, Interrupt};
use rusticmc_core::PluginFault;
use rusticmc_core::model::{
    CommandContext, CommandId, CompleteContext, Event, EventKind, HandlerId, TaskId, Text,
};
use std::sync::Arc;
use std::time::Duration;
use wasmtime::component::{Instance, TypedFunc};
use wasmtime::{Store, Trap, UpdateDeadline, WasmBacktrace};

type LifecycleFn = TypedFunc<(), (Result<(), String>,)>;
type RunJobFn = TypedFunc<(u32, String, Vec<u8>), ()>;
type ExecuteFn = TypedFunc<(u32, wc::CommandContext), (Result<(), String>,)>;
type CompleteFn = TypedFunc<(u32, wc::CompleteContext), (Vec<String>,)>;

/// The `rustic:api/plugin` exports present in a component (only `on-load` and `on-enable`
/// are required; older components may lack newer functions such as `run-job`).
pub(crate) struct PluginFuncs {
    on_load: LifecycleFn,
    on_enable: LifecycleFn,
    on_disable: Option<TypedFunc<(), ()>>,
    run_task: Option<TypedFunc<(u32,), ()>>,
    run_job: Option<RunJobFn>,
    execute_command: Option<ExecuteFn>,
    complete_command: Option<CompleteFn>,
    on_message: Option<TypedFunc<(String, Vec<u8>), ()>>,
}

macro_rules! handlers {
    ($($field:ident: $variant:ident = $export:literal),* $(,)?) => {
        /// Names of every `handlers.handle-*` export the host knows.
        pub(crate) const HANDLER_EXPORTS: &[&str] = &[$($export),*];

        /// The `handle-*` exports present in a component.
        #[derive(Default)]
        pub(crate) struct HandlerFuncs {
            $($field: Option<TypedFunc<(u32, we::$variant), (we::$variant,)>>,)*
        }

        impl HandlerFuncs {
            fn load(
                store: &mut Store<HostState>,
                instance: &Instance,
                compiled: &Compiled,
            ) -> Result<Self, String> {
                let mut h = HandlerFuncs::default();
                $(
                    if let Some(idx) = compiled.exports.handlers.get($export) {
                        h.$field = Some(
                            instance
                                .get_typed_func(&mut *store, idx)
                                .map_err(|e| format!("export `{}` has the wrong type: {e:#}", $export))?,
                        );
                    }
                )*
                Ok(h)
            }

            /// Whether the component handles this event kind.
            pub(crate) fn has(&self, kind: EventKind) -> bool {
                match kind {
                    $(EventKind::$variant => self.$field.is_some(),)*
                }
            }

            fn call(
                &self,
                store: &mut Store<HostState>,
                handler: HandlerId,
                event: &Event,
            ) -> Option<wasmtime::Result<Event>> {
                match event {
                    $(
                        Event::$variant(e) => {
                            let f = self.$field?;
                            Some(f.call(store, (handler, e.clone().cv())).map(|(r,)| Event::$variant(r.cv())))
                        }
                    )*
                }
            }
        }
    };
}

handlers! {
    player_pre_login: PlayerPreLogin = "handle-player-pre-login",
    player_join: PlayerJoin = "handle-player-join",
    player_quit: PlayerQuit = "handle-player-quit",
    player_chat: PlayerChat = "handle-player-chat",
    player_command: PlayerCommand = "handle-player-command",
    server_command: ServerCommand = "handle-server-command",
    player_move: PlayerMove = "handle-player-move",
    player_teleport: PlayerTeleport = "handle-player-teleport",
    player_change_world: PlayerChangeWorld = "handle-player-change-world",
    block_break: BlockBreak = "handle-block-break",
    block_place: BlockPlace = "handle-block-place",
    player_interact: PlayerInteract = "handle-player-interact",
    player_interact_entity: PlayerInteractEntity = "handle-player-interact-entity",
    entity_damage: EntityDamage = "handle-entity-damage",
    player_death: PlayerDeath = "handle-player-death",
    entity_death: EntityDeath = "handle-entity-death",
    player_respawn: PlayerRespawn = "handle-player-respawn",
    inventory_click: InventoryClick = "handle-inventory-click",
    inventory_close: InventoryClose = "handle-inventory-close",
    player_game_mode_change: PlayerGameModeChange = "handle-player-game-mode-change",
    server_list_ping: ServerListPing = "handle-server-list-ping",
    plugin_message: PluginMessage = "handle-plugin-message",
    chunk_load: ChunkLoad = "handle-chunk-load",
    chunk_unload: ChunkUnload = "handle-chunk-unload",
    custom_event: CustomEvent = "handle-custom-event",
}

/// The service interfaces a provider component exports.
#[derive(Default)]
pub(crate) struct ServiceFuncs {
    pub economy: Option<economy::Guest>,
    pub permission_provider: Option<permission_provider::Guest>,
    pub chat_meta: Option<chat_meta::Guest>,
}

/// One instance of a plugin component and its store.
pub(crate) struct WasmInstance {
    pub store: Store<HostState>,
    plugin: PluginFuncs,
    pub handlers: HandlerFuncs,
    pub services: ServiceFuncs,
    fuel: u64,
    deadline_ticks: u64,
    deadline: Duration,
    /// Epoch ticks between two checks of the call clock (async instances: cancellation and
    /// blocked-time accounting).
    check_ticks: u64,
}

fn typed<P, R>(
    store: &mut Store<HostState>,
    instance: &Instance,
    compiled: &Compiled,
    name: &str,
) -> Result<Option<TypedFunc<P, R>>, String>
where
    P: wasmtime::component::ComponentNamedList + wasmtime::component::Lower,
    R: wasmtime::component::ComponentNamedList + wasmtime::component::Lift,
{
    match compiled.exports.plugin.get(name) {
        None => Ok(None),
        Some(idx) => instance
            .get_typed_func::<P, R>(&mut *store, idx)
            .map(Some)
            .map_err(|e| format!("export `{name}` has the wrong type: {e:#}")),
    }
}

impl WasmInstance {
    /// Instantiate `compiled` in a fresh store holding `state`.
    pub(crate) fn instantiate(
        core: &Arc<HostCore>,
        compiled: &Compiled,
        state: HostState,
        fuel: u64,
        deadline: Duration,
    ) -> Result<Self, String> {
        let is_main = state.is_main;
        let mut store = Store::new(&core.engine, state);
        store.limiter(|s| &mut s.limiter);
        let check_ticks = if is_main {
            core.ticks(deadline)
        } else {
            core.ticks(Duration::from_millis(10))
        };
        let tick = core.config.epoch_tick;
        store.epoch_deadline_callback(move |mut s| match s.data_mut().clock.check() {
            None => Ok(UpdateDeadline::Interrupt),
            Some(left) => {
                let ticks = (left.as_nanos() / tick.as_nanos().max(1)) as u64;
                Ok(UpdateDeadline::Continue(ticks.clamp(1, check_ticks)))
            }
        });
        // Instantiation runs start functions: bound it like a call.
        store.set_fuel(fuel).map_err(|e| format!("fuel: {e:#}"))?;
        store.set_epoch_deadline(check_ticks);
        store.data_mut().clock.deadline = deadline;
        store.data_mut().clock.start();
        let instance = compiled.pre.instantiate(&mut store).map_err(|e| {
            let f = classify(e, &mut store, 0, "instantiation", fuel, deadline);
            f.to_string()
        })?;
        let plugin = PluginFuncs {
            on_load: typed(&mut store, &instance, compiled, "on-load")?.ok_or("missing on-load")?,
            on_enable: typed(&mut store, &instance, compiled, "on-enable")?
                .ok_or("missing on-enable")?,
            on_disable: typed(&mut store, &instance, compiled, "on-disable")?,
            run_task: typed(&mut store, &instance, compiled, "run-task")?,
            run_job: typed(&mut store, &instance, compiled, "run-job")?,
            execute_command: typed(&mut store, &instance, compiled, "execute-command")?,
            complete_command: typed(&mut store, &instance, compiled, "complete-command")?,
            on_message: typed(&mut store, &instance, compiled, "on-message")?,
        };
        let handlers = HandlerFuncs::load(&mut store, &instance, compiled)?;
        let e = &compiled.exports;
        let services = ServiceFuncs {
            economy: match &e.economy {
                Some(i) => Some(
                    i.load(&mut store, &instance)
                        .map_err(|e| format!("{e:#}"))?,
                ),
                None => None,
            },
            permission_provider: match &e.permission_provider {
                Some(i) => Some(
                    i.load(&mut store, &instance)
                        .map_err(|e| format!("{e:#}"))?,
                ),
                None => None,
            },
            chat_meta: match &e.chat_meta {
                Some(i) => Some(
                    i.load(&mut store, &instance)
                        .map_err(|e| format!("{e:#}"))?,
                ),
                None => None,
            },
        };
        Ok(Self {
            store,
            plugin,
            handlers,
            services,
            fuel,
            deadline_ticks: core.ticks(deadline),
            deadline,
            check_ticks,
        })
    }

    /// Run one call into the guest with fresh fuel and deadline; errors become faults.
    pub(crate) fn call<R>(
        &mut self,
        what: &str,
        f: impl FnOnce(&mut Store<HostState>, &ServiceFuncs) -> wasmtime::Result<R>,
    ) -> Result<R, PluginFault> {
        // Setting fuel only fails when fuel is disabled, which the engine never is.
        let _ = self.store.set_fuel(self.fuel);
        self.store
            .set_epoch_deadline(self.check_ticks.min(self.deadline_ticks));
        let mark = {
            let data = self.store.data_mut();
            data.clock.deadline = self.deadline;
            data.clock.start();
            data.stderr.mark()
        };
        match f(&mut self.store, &self.services) {
            Ok(r) => Ok(r),
            Err(e) => Err(classify(
                e,
                &mut self.store,
                mark,
                what,
                self.fuel,
                self.deadline,
            )),
        }
    }

    /// Fuel left after the last call (benches, tests).
    pub(crate) fn fuel_left(&self) -> u64 {
        self.store.get_fuel().unwrap_or(0)
    }

    pub(crate) fn on_load(&mut self) -> Result<Result<(), String>, PluginFault> {
        let f = self.plugin.on_load;
        self.call("on-load", |s, _| f.call(s, ()).map(|(r,)| r))
    }

    pub(crate) fn on_enable(&mut self) -> Result<Result<(), String>, PluginFault> {
        let f = self.plugin.on_enable;
        self.call("on-enable", |s, _| f.call(s, ()).map(|(r,)| r))
    }

    pub(crate) fn on_disable(&mut self) -> Result<(), PluginFault> {
        match self.plugin.on_disable {
            Some(f) => self.call("on-disable", |s, _| f.call(s, ())),
            None => Ok(()),
        }
    }

    pub(crate) fn run_task(&mut self, task: TaskId) -> Result<(), PluginFault> {
        match self.plugin.run_task {
            Some(f) => self.call("run-task", |s, _| f.call(s, (task,))),
            None => Ok(()),
        }
    }

    pub(crate) fn run_job(
        &mut self,
        task: TaskId,
        job: &str,
        payload: &[u8],
    ) -> Result<(), PluginFault> {
        match self.plugin.run_job {
            Some(f) => self.call("run-job", |s, _| {
                f.call(s, (task, job.to_owned(), payload.to_vec()))
            }),
            None => Err(PluginFault::other(format!(
                "async job `{job}`: the component has no `run-job` export (built against an older API)"
            ))),
        }
    }

    pub(crate) fn execute_command(
        &mut self,
        command: CommandId,
        ctx: CommandContext,
    ) -> Result<Result<(), Text>, PluginFault> {
        let Some(f) = self.plugin.execute_command else {
            return Ok(Err(Text::plain("this plugin cannot run commands")));
        };
        let ctx: wc::CommandContext = ctx.cv();
        self.call("execute-command", |s, _| f.call(s, (command, ctx)))
            .map(|(r,)| r.map_err(Cv::cv))
    }

    pub(crate) fn complete_command(
        &mut self,
        command: CommandId,
        ctx: CompleteContext,
    ) -> Result<Vec<String>, PluginFault> {
        let Some(f) = self.plugin.complete_command else {
            return Ok(Vec::new());
        };
        let ctx: wc::CompleteContext = ctx.cv();
        self.call("complete-command", |s, _| f.call(s, (command, ctx)))
            .map(|(r,)| r)
    }

    pub(crate) fn on_message(&mut self, topic: &str, payload: &[u8]) -> Result<(), PluginFault> {
        match self.plugin.on_message {
            Some(f) => self.call("on-message", |s, _| {
                f.call(s, (topic.to_owned(), payload.to_vec()))
            }),
            None => Ok(()),
        }
    }

    /// `None` when the component has no handler export for this event.
    pub(crate) fn handle_event(
        &mut self,
        handler: HandlerId,
        event: &Event,
    ) -> Option<Result<Event, PluginFault>> {
        if !self.handlers.has(event.kind()) {
            return None;
        }
        let handlers = std::mem::take(&mut self.handlers);
        let r = self.call(event.name(), |s, _| {
            handlers
                .call(s, handler, event)
                .unwrap_or_else(|| Ok(event.clone()))
        });
        self.handlers = handlers;
        Some(r)
    }
}

/// Turn a wasmtime error from a call into a fault: memory limit, fuel, deadline, Rust panic
/// (the message read back from stderr), other traps; with the guest backtrace.
pub(crate) fn classify(
    err: wasmtime::Error,
    store: &mut Store<HostState>,
    stderr_mark: u64,
    what: &str,
    fuel: u64,
    deadline: Duration,
) -> PluginFault {
    let backtrace = err.downcast_ref::<WasmBacktrace>().map(|bt| bt.to_string());
    let data = store.data_mut();
    let fault = if let Some(msg) = data.limiter.exceeded.take() {
        PluginFault::memory_exceeded(format!("{what}: {msg}"))
    } else {
        match err.downcast_ref::<Trap>() {
            Some(Trap::OutOfFuel) => PluginFault::fuel_exhausted(format!(
                "{what} used all of its fuel ({fuel} units per call)"
            )),
            Some(Trap::Interrupt) => match data.clock.interrupted {
                Some(Interrupt::Cancelled) => PluginFault::other(format!(
                    "{what} was cancelled (the plugin is being disabled)"
                )),
                _ => PluginFault::timeout(format!(
                    "{what} ran longer than its {} ms deadline",
                    deadline.as_millis()
                )),
            },
            Some(Trap::UnreachableCodeReached) => {
                let lines = data.stderr.lines_since(stderr_mark);
                match panic_message(&lines) {
                    Some(msg) => PluginFault::panic(format!("{what}: {msg}")),
                    None => {
                        let tail = lines
                            .iter()
                            .rev()
                            .find(|l| !l.trim().is_empty())
                            .map(|l| format!(" (last stderr line: {l})"))
                            .unwrap_or_default();
                        PluginFault::trap(format!(
                            "{what}: wasm `unreachable` executed (abort){tail}"
                        ))
                    }
                }
            }
            Some(t) => PluginFault::trap(format!("{what}: {t}")),
            None => PluginFault::trap(format!("{what}: {}", root_message(&err))),
        }
    };
    match backtrace {
        Some(bt) if !bt.trim().is_empty() => fault.with_backtrace(bt),
        _ => fault,
    }
}

fn root_message(err: &wasmtime::Error) -> String {
    let mut parts: Vec<String> = Vec::new();
    for c in err.chain() {
        let s = c.to_string();
        if !parts.contains(&s) {
            parts.push(s);
        }
    }
    parts.join(": ")
}
