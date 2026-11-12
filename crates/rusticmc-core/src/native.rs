//! In-process plugins built from closures: [`NativePlugin`] and [`NativeLoader`].
//!
//! Used by the core's own tests and benches and by `rusticmc-test` to drive the runtime
//! without WebAssembly. Each closure receives the plugin's [`PluginApi`] (the same surface a
//! WASM plugin's imports reach). Panics inside closures are caught by the runtime and become
//! [`PluginFault::Panic`].
//!
//! ```
//! use rusticmc_core::native::{NativeLoader, NativePlugin};
//! use rusticmc_core::model::{Event, Priority};
//!
//! let loader = NativeLoader::new();
//! loader.register("hello", || {
//!     NativePlugin::new()
//!         .on_enable(|api| {
//!             api.subscribe("player-join", Priority::Normal, false)?;
//!             Ok(())
//!         })
//!         .on_event(|_api, _handler, event| Ok(event.clone()))
//! });
//! ```

use crate::context;
use crate::lifecycle::{LoadRequest, PluginFault, PluginInstance, PluginLoader};
use crate::model::{CommandContext, CommandId, CompleteContext, Event, HandlerId, TaskId, Text};
use crate::runtime::PluginApi;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

type Lifecycle = Box<dyn FnMut(&PluginApi) -> Result<(), String>>;
type Fallible<T> = Box<dyn FnMut(&PluginApi) -> Result<T, PluginFault>>;
type TaskFn = Box<dyn FnMut(&PluginApi, TaskId) -> Result<(), PluginFault>>;
type CommandFn =
    Box<dyn FnMut(&PluginApi, CommandId, CommandContext) -> Result<Result<(), Text>, PluginFault>>;
type CompleteFn =
    Box<dyn FnMut(&PluginApi, CommandId, CompleteContext) -> Result<Vec<String>, PluginFault>>;
type JobFn = Box<dyn FnMut(TaskId, &str, &[u8]) -> Result<(), PluginFault>>;
type MessageFn = Box<dyn FnMut(&PluginApi, &str, &[u8]) -> Result<(), PluginFault>>;
type EventFn = Box<dyn FnMut(&PluginApi, HandlerId, &Event) -> Result<Event, PluginFault>>;

/// A plugin made of closures. Unset hooks do nothing (events are returned unchanged).
#[derive(Default)]
pub struct NativePlugin {
    on_load: Option<Lifecycle>,
    on_enable: Option<Lifecycle>,
    on_disable: Option<Fallible<()>>,
    run_task: Option<TaskFn>,
    run_job: Option<JobFn>,
    execute: Option<CommandFn>,
    complete: Option<CompleteFn>,
    on_message: Option<MessageFn>,
    handle_event: Option<EventFn>,
}

impl std::fmt::Debug for NativePlugin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativePlugin").finish_non_exhaustive()
    }
}

fn api() -> Result<PluginApi, PluginFault> {
    context::api().map_err(|e| PluginFault::other(format!("no runtime context: {e}")))
}

impl NativePlugin {
    /// A plugin that does nothing.
    pub fn new() -> Self {
        Self::default()
    }

    /// `on-load` (return `Err(msg)` to refuse loading).
    pub fn on_load(mut self, f: impl FnMut(&PluginApi) -> Result<(), String> + 'static) -> Self {
        self.on_load = Some(Box::new(f));
        self
    }

    /// `on-enable` (return `Err(msg)` to refuse enabling).
    pub fn on_enable(mut self, f: impl FnMut(&PluginApi) -> Result<(), String> + 'static) -> Self {
        self.on_enable = Some(Box::new(f));
        self
    }

    /// `on-disable`.
    pub fn on_disable(
        mut self,
        f: impl FnMut(&PluginApi) -> Result<(), PluginFault> + 'static,
    ) -> Self {
        self.on_disable = Some(Box::new(f));
        self
    }

    /// `run-task`.
    pub fn on_task(
        mut self,
        f: impl FnMut(&PluginApi, TaskId) -> Result<(), PluginFault> + 'static,
    ) -> Self {
        self.run_task = Some(Box::new(f));
        self
    }

    /// `run-job` (async instance: there is no runtime context there, so no [`PluginApi`]).
    pub fn on_job(
        mut self,
        f: impl FnMut(TaskId, &str, &[u8]) -> Result<(), PluginFault> + 'static,
    ) -> Self {
        self.run_job = Some(Box::new(f));
        self
    }

    /// `execute-command`.
    pub fn on_command(
        mut self,
        f: impl FnMut(&PluginApi, CommandId, CommandContext) -> Result<Result<(), Text>, PluginFault>
        + 'static,
    ) -> Self {
        self.execute = Some(Box::new(f));
        self
    }

    /// `complete-command`.
    pub fn on_complete(
        mut self,
        f: impl FnMut(&PluginApi, CommandId, CompleteContext) -> Result<Vec<String>, PluginFault>
        + 'static,
    ) -> Self {
        self.complete = Some(Box::new(f));
        self
    }

    /// `on-message`.
    pub fn on_message(
        mut self,
        f: impl FnMut(&PluginApi, &str, &[u8]) -> Result<(), PluginFault> + 'static,
    ) -> Self {
        self.on_message = Some(Box::new(f));
        self
    }

    /// `handle-<event>` for every subscribed event.
    pub fn on_event(
        mut self,
        f: impl FnMut(&PluginApi, HandlerId, &Event) -> Result<Event, PluginFault> + 'static,
    ) -> Self {
        self.handle_event = Some(Box::new(f));
        self
    }
}

impl PluginInstance for NativePlugin {
    fn on_load(&mut self) -> Result<Result<(), String>, PluginFault> {
        match &mut self.on_load {
            Some(f) => Ok(f(&api()?)),
            None => Ok(Ok(())),
        }
    }

    fn on_enable(&mut self) -> Result<Result<(), String>, PluginFault> {
        match &mut self.on_enable {
            Some(f) => Ok(f(&api()?)),
            None => Ok(Ok(())),
        }
    }

    fn on_disable(&mut self) -> Result<(), PluginFault> {
        match &mut self.on_disable {
            Some(f) => f(&api()?),
            None => Ok(()),
        }
    }

    fn run_task(&mut self, task: TaskId) -> Result<(), PluginFault> {
        match &mut self.run_task {
            Some(f) => f(&api()?, task),
            None => Ok(()),
        }
    }

    fn run_job(&mut self, task: TaskId, job: &str, payload: &[u8]) -> Result<(), PluginFault> {
        match &mut self.run_job {
            Some(f) => f(task, job, payload),
            None => Ok(()),
        }
    }

    fn execute_command(
        &mut self,
        command: CommandId,
        ctx: CommandContext,
    ) -> Result<Result<(), Text>, PluginFault> {
        match &mut self.execute {
            Some(f) => f(&api()?, command, ctx),
            None => Ok(Ok(())),
        }
    }

    fn complete_command(
        &mut self,
        command: CommandId,
        ctx: CompleteContext,
    ) -> Result<Vec<String>, PluginFault> {
        match &mut self.complete {
            Some(f) => f(&api()?, command, ctx),
            None => Ok(Vec::new()),
        }
    }

    fn on_message(&mut self, topic: &str, payload: &[u8]) -> Result<(), PluginFault> {
        match &mut self.on_message {
            Some(f) => f(&api()?, topic, payload),
            None => Ok(()),
        }
    }

    fn handle_event(&mut self, handler: HandlerId, event: &Event) -> Result<Event, PluginFault> {
        match &mut self.handle_event {
            Some(f) => f(&api()?, handler, event),
            None => Ok(event.clone()),
        }
    }

    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        Some(self)
    }
}

type Factory = Box<dyn FnMut(&LoadRequest<'_>) -> Result<Box<dyn PluginInstance>, String>>;

/// A loader that instantiates registered native plugins by id (each load calls the factory
/// again, so reloads get a fresh instance). Clones share the registry, so factories can be
/// added after the loader was given to the runtime.
#[derive(Clone, Default)]
pub struct NativeLoader {
    factories: Rc<RefCell<HashMap<String, Factory>>>,
    loads: Rc<RefCell<HashMap<String, usize>>>,
}

impl std::fmt::Debug for NativeLoader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("NativeLoader").finish_non_exhaustive()
    }
}

impl NativeLoader {
    /// Empty.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a factory for a plugin id.
    pub fn register(&self, id: &str, mut factory: impl FnMut() -> NativePlugin + 'static) {
        self.factories.borrow_mut().insert(
            id.to_owned(),
            Box::new(move |_| Ok(Box::new(factory()) as Box<dyn PluginInstance>)),
        );
    }

    /// Register a factory that sees the load request and may fail.
    pub fn register_with(
        &self,
        id: &str,
        factory: impl FnMut(&LoadRequest<'_>) -> Result<Box<dyn PluginInstance>, String> + 'static,
    ) {
        self.factories
            .borrow_mut()
            .insert(id.to_owned(), Box::new(factory));
    }

    /// How many times a plugin was instantiated.
    pub fn load_count(&self, id: &str) -> usize {
        self.loads.borrow().get(id).copied().unwrap_or(0)
    }

    /// A boxed clone, to give to [`crate::Rustic::new`].
    pub fn boxed(&self) -> Box<dyn PluginLoader> {
        Box::new(self.clone())
    }
}

impl PluginLoader for NativeLoader {
    fn load(&mut self, req: &LoadRequest<'_>) -> Result<Box<dyn PluginInstance>, String> {
        let id = req.package.manifest.id.to_string();
        let mut f = self.factories.borrow_mut();
        let factory = f
            .get_mut(&id)
            .ok_or_else(|| format!("no native implementation registered for `{id}`"))?;
        let inst = factory(req)?;
        *self.loads.borrow_mut().entry(id).or_insert(0) += 1;
        Ok(inst)
    }
}
