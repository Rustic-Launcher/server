//! # rusticmc-api: the Rustic plugin SDK
//!
//! Rustic plugins are WebAssembly components (`wasm32-wasip2`) implementing the WIT world
//! `rustic:api/rustic-plugin@0.1.0`. This crate owns the generated bindings and the component
//! exports, and wraps the raw imports in idiomatic types: [`Server`], [`Player`], [`World`],
//! [`ItemStack`], [`Menu`], [`Text`], [`Scheduler`], [`Storage`], [`Command`] and friends.
//!
//! A plugin is a `Default` struct implementing [`Plugin`], marked `#[rustic::plugin]`:
//!
//! ```ignore
//! use rustic::prelude::*;   // Cargo.toml: rustic = { package = "rusticmc-api", ... }
//!
//! #[rustic::plugin]
//! #[derive(Default)]
//! struct Hello;
//!
//! impl Plugin for Hello {
//!     fn enable(&mut self, ctx: &mut Ctx) -> Result<()> {
//!         ctx.on(|e: &mut PlayerJoinEvent| e.player().send_message(Text::mini("<gold>Hi!")))?;
//!         Ok(())
//!     }
//! }
//! ```
//!
//! See `docs/API.md` for the full guide: the main/async instance model, events, commands,
//! menus, text, storage and config, capabilities and the versioning policy.
//!
//! ## Host target
//!
//! The pure parts (text, usages, items, config migration, event wrappers) compile and are tested
//! on the host target. Every function that calls a host import panics there with a message naming
//! the import: imports only exist inside a Rustic server (or the `rusticmc-test` harness).

#![warn(missing_docs)]

/// Generated bindings for the `rustic:api` world (wit-bindgen). Prefer the wrappers; the raw
/// records are re-exported where they are already convenient ([`Uuid`], [`Location`],
/// [`BlockPos`], [`Vec3`], [`BlockState`], ...).
#[allow(
    unsafe_code,
    missing_docs,
    clippy::all,
    clippy::pedantic,
    rustdoc::all,
    unused_qualifications
)]
pub mod bindings {
    // One world per combination of provided services (cargo features `provide-permissions`,
    // `provide-economy`): a component only exports the service interfaces it implements.
    #[cfg(not(any(feature = "provide-permissions", feature = "provide-economy")))]
    wit_bindgen::generate!({
        path: "../rusticmc-api-wit/wit",
        world: "rustic-plugin",
        pub_export_macro: true,
        default_bindings_module: "rusticmc_api::bindings",
        additional_derives: [PartialEq],
        generate_all,
    });
    #[cfg(all(feature = "provide-permissions", not(feature = "provide-economy")))]
    wit_bindgen::generate!({
        path: "../rusticmc-api-wit/wit",
        world: "rustic-plugin-permissions",
        pub_export_macro: true,
        default_bindings_module: "rusticmc_api::bindings",
        additional_derives: [PartialEq],
        generate_all,
    });
    #[cfg(all(feature = "provide-economy", not(feature = "provide-permissions")))]
    wit_bindgen::generate!({
        path: "../rusticmc-api-wit/wit",
        world: "rustic-plugin-economy",
        pub_export_macro: true,
        default_bindings_module: "rusticmc_api::bindings",
        additional_derives: [PartialEq],
        generate_all,
    });
    #[cfg(all(feature = "provide-permissions", feature = "provide-economy"))]
    wit_bindgen::generate!({
        path: "../rusticmc-api-wit/wit",
        world: "rustic-plugin-permissions-economy",
        pub_export_macro: true,
        default_bindings_module: "rusticmc_api::bindings",
        additional_derives: [PartialEq],
        generate_all,
    });
}

mod error;
mod host;
pub mod log;
mod types;

pub mod command;
pub mod config;
pub mod event;
pub mod http;
pub mod inventory;
pub mod item;
pub mod menu;
pub mod messaging;
pub mod permissions;
pub mod player;
mod plugin;
pub mod scheduler;
pub mod scoreboard;
pub mod server;
pub mod services;
pub mod storage;
pub mod text;
pub mod world;

#[doc(hidden)]
pub mod __rt;

pub use command::{Command, CommandCtx, CommandError, CommandResult, CompleteCtx, Sender};
pub use error::{Error, Result};
pub use event::{Cancellable, Event};
pub use http::{Request, Response};
pub use item::ItemStack;
pub use menu::{Menu, MenuClick, MenuHandle};
pub use messaging::Messaging;
pub use permissions::Permissions;
pub use player::Player;
pub use plugin::{Ctx, Instance, Plugin, with_plugin};
pub use scheduler::{Scheduler, Task};
pub use scoreboard::{BossBar, Objective, Scoreboard, TabList, Team};
pub use server::Server;
pub use services::{
    ChatMeta, Economy, PermissionProvider, ProvideChatMeta, ProvideEconomy, ProvidePermissions,
    ServiceError, ServiceResult, Services, Tristate,
};
pub use storage::Storage;
pub use text::{Color, Text};
pub use types::*;
pub use world::World;

pub use rusticmc_api_macros::{command, completer, event_handler, handlers, plugin};

/// Re-exported for plugins that use the typed helpers without depending on serde themselves.
pub use serde;
/// Re-exported for JSON payloads (custom events, messages, HTTP bodies).
pub use serde_json;
/// Re-exported for config migrations (`toml::Table`).
#[cfg(feature = "config")]
pub use toml;

/// Everything a typical plugin needs: `use rusticmc_api::prelude::*;`.
pub mod prelude {
    pub use crate::command::{
        Command, CommandCtx, CommandError, CommandResult, CompleteCtx, Sender,
    };
    pub use crate::error::{Error, Result};
    pub use crate::event::*;
    pub use crate::item::ItemStack;
    pub use crate::menu::{Menu, MenuClick, MenuHandle};
    pub use crate::player::Player;
    pub use crate::plugin::{Ctx, Instance, Plugin, with_plugin};
    pub use crate::scheduler::{Scheduler, Task};
    pub use crate::scoreboard::{BossBar, Objective, Scoreboard, TabList, Team};
    pub use crate::server::Server;
    pub use crate::services::{
        ChatMeta, Economy, PermissionProvider, ProvideChatMeta, ProvideEconomy, ProvidePermissions,
        ServiceError, ServiceResult, Tristate,
    };
    pub use crate::storage::Storage;
    pub use crate::text::{Color, NamedColor, Text};
    pub use crate::types::*;
    pub use crate::world::World;
    pub use crate::{debug, error, info, trace, warn};
}

/// Generates the component exports for a plugin type. `#[rustic::plugin]` expands to this; call
/// it yourself only if you do not use the attribute.
///
/// The type must be `Default + Plugin`. If an `impl` block of the type is marked
/// `#[rustic::handlers]`, its handlers and commands are registered after `Plugin::enable`.
#[macro_export]
macro_rules! export_plugin {
    ($ty:ty) => {
        const _: () = {
            #[doc(hidden)]
            struct __RusticGuest;

            fn __rustic_register(ctx: &mut $crate::Ctx) -> $crate::Result<()> {
                #[allow(unused_imports)]
                use $crate::__rt::{ViaHandlers as _, ViaNothing as _};
                (&$crate::__rt::Probe::<$ty>::new()).__rustic_register(ctx)
            }

            impl $crate::bindings::exports::rustic::api::plugin::Guest for __RusticGuest {
                fn on_load() -> ::core::result::Result<(), ::std::string::String> {
                    $crate::__rt::on_load::<$ty>()?;
                    $crate::__provide_permissions!($ty);
                    $crate::__provide_economy!($ty);
                    Ok(())
                }
                fn on_enable() -> ::core::result::Result<(), ::std::string::String> {
                    $crate::__rt::on_enable::<$ty>(__rustic_register)
                }
                fn on_disable() {
                    $crate::__rt::on_disable::<$ty>()
                }
                fn run_task(task: u32) {
                    $crate::__rt::run_task(task)
                }
                fn execute_command(
                    command: u32,
                    ctx: $crate::bindings::rustic::api::commands::CommandContext,
                ) -> ::core::result::Result<(), ::std::string::String> {
                    $crate::__rt::execute_command(command, ctx)
                }
                fn complete_command(
                    command: u32,
                    ctx: $crate::bindings::rustic::api::commands::CompleteContext,
                ) -> ::std::vec::Vec<::std::string::String> {
                    $crate::__rt::complete_command(command, ctx)
                }
                fn run_job(task: u32, job: ::std::string::String, payload: ::std::vec::Vec<u8>) {
                    $crate::__rt::run_job(task, job, payload)
                }
                fn on_message(topic: ::std::string::String, payload: ::std::vec::Vec<u8>) {
                    $crate::__rt::on_message(topic, payload)
                }
            }

            $crate::__impl_handlers_guest!(__RusticGuest);
            $crate::__impl_permissions_guest!(__RusticGuest, $ty);
            $crate::__impl_economy_guest!(__RusticGuest, $ty);

    $crate::bindings::export!(__RusticGuest with_types_in $crate::bindings);
        };
    };
}
