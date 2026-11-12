//! The plugin trait and the enable/load context.

use serde::de::DeserializeOwned;

use crate::__rt::{self, IntoHandlerResult};
use crate::command::{Command, CommandHandle};
use crate::error::Result;
use crate::event::{CustomEvent, Event, Events, Subscription};
use crate::types::{PluginMeta, Priority};

/// A plugin. The type also needs `Default` (the SDK builds the single instance with it) and the
/// `#[rustic::plugin]` attribute (which generates the component exports).
///
/// `load` and `enable` run in **both** the main and the async instance (see
/// [`Instance`]); the SDK's registration calls (`ctx.on`, `ctx.command`, permissions, timers)
/// only reach the host from the main instance, so the same code is correct in both.
pub trait Plugin: 'static {
    /// Called once after instantiation, before any plugin is enabled: declare permissions and
    /// services here.
    fn load(&mut self, ctx: &mut Ctx) -> Result<()> {
        let _ = ctx;
        Ok(())
    }

    /// Register listeners, commands, tasks and async jobs. An error disables the plugin.
    fn enable(&mut self, ctx: &mut Ctx) -> Result<()>;

    /// Called before the plugin is unloaded. The host removes its listeners, commands, tasks,
    /// menus, bars and channels afterwards anyway.
    fn disable(&mut self, ctx: &mut Ctx) {
        let _ = ctx;
    }
}

/// Which instance of the component this code runs in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Instance {
    /// On the server tick: events, commands, world access.
    Main,
    /// Off the tick: may block (HTTP, storage), no world access.
    Async,
}

/// Passed to `Plugin::load`, `enable` and `disable`: registration helpers.
#[derive(Debug)]
pub struct Ctx {
    instance: Instance,
}

impl Ctx {
    pub(crate) fn new() -> Ctx {
        Ctx {
            instance: if __rt::is_main() {
                Instance::Main
            } else {
                Instance::Async
            },
        }
    }

    /// Which instance this is.
    pub fn instance(&self) -> Instance {
        self.instance
    }

    /// Whether this is the main instance. Guard one-time side effects you make yourself
    /// (broadcasts, world edits) in `enable` with it.
    pub fn is_main(&self) -> bool {
        self.instance == Instance::Main
    }

    /// This plugin's metadata.
    pub fn meta(&self) -> PluginMeta {
        crate::server::Server::self_meta()
    }

    /// This plugin's id.
    pub fn plugin_id(&self) -> String {
        __rt::plugin_id()
    }

    /// Subscribes to `E` at normal priority, cancelled events included.
    pub fn on<E, R>(
        &mut self,
        mut handler: impl FnMut(&mut E) -> R + 'static,
    ) -> Result<Subscription>
    where
        E: Event,
        R: IntoHandlerResult,
    {
        Events::subscribe::<E>(Priority::Normal, false, move |e| handler(e).report(E::NAME))
    }

    /// Subscribes to `E` with a priority; `ignore_cancelled` skips events already cancelled.
    pub fn on_with<E, R>(
        &mut self,
        priority: Priority,
        ignore_cancelled: bool,
        mut handler: impl FnMut(&mut E) -> R + 'static,
    ) -> Result<Subscription>
    where
        E: Event,
        R: IntoHandlerResult,
    {
        Events::subscribe::<E>(priority, ignore_cancelled, move |e| {
            handler(e).report(E::NAME)
        })
    }

    /// Subscribes to a custom event (`<plugin-id>:<name>`).
    pub fn on_custom<R: IntoHandlerResult>(
        &mut self,
        name: &str,
        handler: impl FnMut(&mut CustomEvent) -> R + 'static,
    ) -> Result<Subscription> {
        self.on_custom_with(name, Priority::Normal, false, handler)
    }

    /// Subscribes to a custom event with a priority.
    pub fn on_custom_with<R: IntoHandlerResult>(
        &mut self,
        name: &str,
        priority: Priority,
        ignore_cancelled: bool,
        mut handler: impl FnMut(&mut CustomEvent) -> R + 'static,
    ) -> Result<Subscription> {
        Events::subscribe_custom(name, priority, ignore_cancelled, move |e| {
            handler(e).report("custom-event")
        })
    }

    /// Registers a command (no-op in the async instance).
    pub fn command(&mut self, command: Command) -> Result<CommandHandle> {
        command.register()
    }

    /// Registers the async job `name`: `f` runs in the async instance with the payload given to
    /// [`Scheduler::run_async`](crate::Scheduler::run_async), decoded from JSON. Register it in
    /// `enable` so both instances know it.
    pub fn async_task<T, R>(&mut self, name: &str, mut f: impl FnMut(T) -> R + 'static)
    where
        T: DeserializeOwned + 'static,
        R: IntoHandlerResult,
    {
        let label = name.to_owned();
        __rt::add_async_job(
            name,
            Box::new(
                move |payload: &[u8]| match serde_json::from_slice::<T>(payload) {
                    Ok(v) => f(v).report(&label),
                    Err(e) => crate::warn!("async job `{label}`: bad payload: {e}"),
                },
            ),
        );
    }

    /// Handles values the async instance sends with
    /// [`Scheduler::post_to_main`](crate::Scheduler::post_to_main) on `topic`.
    pub fn on_message<T, R>(&mut self, topic: &str, mut f: impl FnMut(T) -> R + 'static)
    where
        T: DeserializeOwned + 'static,
        R: IntoHandlerResult,
    {
        let label = topic.to_owned();
        __rt::add_message_handler(
            topic,
            Box::new(
                move |bytes: &[u8]| match serde_json::from_slice::<T>(bytes) {
                    Ok(v) => f(v).report(&label),
                    Err(e) => crate::warn!("message `{label}`: bad payload: {e}"),
                },
            ),
        );
    }

    /// Handles raw bytes posted on `topic`.
    pub fn on_message_raw(&mut self, topic: &str, f: impl FnMut(&[u8]) + 'static) {
        __rt::add_message_handler(topic, Box::new(f));
    }

    /// Loads `config.toml` from the data folder into `T` (defaults written when missing).
    /// For versions and migrations use [`Config`](crate::config::Config) directly.
    #[cfg(feature = "config")]
    pub fn config<T>(&mut self) -> Result<T>
    where
        T: DeserializeOwned + serde::Serialize + Default,
    {
        crate::config::Config::new().load()
    }
}

/// Runs `f` with the plugin instance (for closures registered with the builder APIs that need
/// the plugin's state; `#[event_handler]` / `#[command]` methods use it under the hood).
///
/// # Panics
///
/// If `P` is not the plugin type, or when called while the plugin is already borrowed (from
/// inside `enable`/`load`/`disable`, or from within another `with_plugin`).
pub fn with_plugin<P: 'static, R>(f: impl FnOnce(&mut P) -> R) -> R {
    __rt::with_plugin(f)
}
