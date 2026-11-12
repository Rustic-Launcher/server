//! The [`Backend`] trait: everything game-facing a plugin can import, implemented by the engine
//! adapter (Horizon) and by the test harness.
//!
//! One method per function of the game-facing WIT interfaces (`server`, `players`,
//! `inventories`, `menus`, `worlds`, `scoreboards`, `bossbars`, `tablist`, `messaging`), named
//! `<interface>_<function>`. Functions the core implements itself are absent: `server.plugins`,
//! `server.self-meta`, `players.get-data/set-data/delete-data` (core storage). Every method has
//! a default body returning [`HostError::Unsupported`], so adapters grow incrementally.
//!
//! Ownership-sensitive calls (`menus_open`, `bossbars_create`, `messaging_register_incoming`,
//! `worlds_keep_chunk_loaded`) should be made through [`crate::PluginApi`] by the host so the
//! core can release them when the plugin is disabled; the backend then only sees plain calls.
//! The core also calls [`Backend::plugin_disabled`] after cleaning up, for anything else an
//! adapter tracks per plugin.

use crate::model::{
    BarColor, BarId, BarStyle, BlockPos, BlockState, DisplaySlot, EntityRef, GameMode, HostError,
    ItemStack, Location, MenuId, MenuSpec, TeamColor, Text, TickStats, Uuid, Vec3, Weather,
};

macro_rules! backend_trait {
    ($( $(#[$m:meta])* fn $name:ident(&mut self $(, $arg:ident : $ty:ty)* $(,)?) -> $ret:ty; )*) => {
        /// The game, as seen by plugins. See the module docs.
        pub trait Backend {
            $(
                $(#[$m])*
                #[allow(clippy::too_many_arguments)]
                fn $name(&mut self $(, $arg: $ty)*) -> Result<$ret, HostError> {
                    $( let _ = $arg; )*
                    Err(HostError::Unsupported(stringify!($name).to_owned()))
                }
            )*

            /// Show a message to the console (command replies to the console). The default
            /// writes the plain text to the log.
            fn console_message(&mut self, message: &Text) -> Result<(), HostError> {
                log::info!(target: "rustic", "{}", message.to_plain());
                Ok(())
            }

            /// Called after the core disabled a plugin and released what it tracks.
            fn plugin_disabled(&mut self, plugin: &str) {
                let _ = plugin;
            }

            /// Called when the command set changed (register, unregister, plugin disabled):
            /// the Horizon adapter re-registers usages (`host.refresh_commands()`).
            fn commands_changed(&mut self) {}
        }
    };
}

backend_trait! {
    // ---- server ----
    /// `server.broadcast`: every online player and the console.
    fn server_broadcast(&mut self, message: &Text) -> ();
    /// `server.dispatch-command`: run as the console; whether it ran.
    fn server_dispatch_command(&mut self, command: &str) -> bool;
    /// `server.get-tick-stats`.
    fn server_get_tick_stats(&mut self) -> TickStats;
    /// `server.max-players`.
    fn server_max_players(&mut self) -> u32;
    /// `server.motd`.
    fn server_motd(&mut self) -> Text;
    /// `server.version`: "Rustic <version> on Horizon <commit> (Minecraft 26.3)".
    fn server_version(&mut self) -> String;
    /// `server.offline-player`: name to UUID from the user cache.
    fn server_offline_player(&mut self, name: &str) -> Option<Uuid>;
    /// `server.operators`.
    fn server_operators(&mut self) -> Vec<Uuid>;

    // ---- players ----
    /// `players.online`.
    fn players_online(&mut self) -> Vec<Uuid>;
    /// `players.name`.
    fn players_name(&mut self, player: Uuid) -> Option<String>;
    /// `players.by-name` (online players, case-insensitive).
    fn players_by_name(&mut self, name: &str) -> Option<Uuid>;
    /// `players.is-online`.
    fn players_is_online(&mut self, player: Uuid) -> bool;
    /// `players.get-location`.
    fn players_get_location(&mut self, player: Uuid) -> Location;
    /// `players.address`.
    fn players_address(&mut self, player: Uuid) -> String;
    /// `players.ping` (ms).
    fn players_ping(&mut self, player: Uuid) -> u32;
    /// `players.locale`.
    fn players_locale(&mut self, player: Uuid) -> String;
    /// `players.send-message`.
    fn players_send_message(&mut self, player: Uuid, message: &Text) -> ();
    /// `players.send-action-bar`.
    fn players_send_action_bar(&mut self, player: Uuid, message: &Text) -> ();
    /// `players.send-title` (ticks).
    fn players_send_title(&mut self, player: Uuid, title: &Text, subtitle: &Text, fade_in: u32, stay: u32, fade_out: u32) -> ();
    /// `players.play-sound`.
    fn players_play_sound(&mut self, player: Uuid, sound: &str, volume: f32, pitch: f32) -> ();
    /// `players.teleport`.
    fn players_teleport(&mut self, player: Uuid, to: &Location) -> ();
    /// `players.get-game-mode`.
    fn players_get_game_mode(&mut self, player: Uuid) -> GameMode;
    /// `players.set-game-mode`.
    fn players_set_game_mode(&mut self, player: Uuid, mode: GameMode) -> ();
    /// `players.health`.
    fn players_health(&mut self, player: Uuid) -> f32;
    /// `players.set-health`.
    fn players_set_health(&mut self, player: Uuid, health: f32) -> ();
    /// `players.max-health`.
    fn players_max_health(&mut self, player: Uuid) -> f32;
    /// `players.food-level`.
    fn players_food_level(&mut self, player: Uuid) -> u32;
    /// `players.set-food-level`.
    fn players_set_food_level(&mut self, player: Uuid, food: u32) -> ();
    /// `players.saturation`.
    fn players_saturation(&mut self, player: Uuid) -> f32;
    /// `players.set-saturation`.
    fn players_set_saturation(&mut self, player: Uuid, saturation: f32) -> ();
    /// `players.experience-level`.
    fn players_experience_level(&mut self, player: Uuid) -> u32;
    /// `players.set-experience-level`.
    fn players_set_experience_level(&mut self, player: Uuid, level: u32) -> ();
    /// `players.set-flying`.
    fn players_set_flying(&mut self, player: Uuid, allow: bool, flying: bool) -> ();
    /// `players.kick`.
    fn players_kick(&mut self, player: Uuid, reason: &Text) -> ();
    /// `players.is-op`.
    fn players_is_op(&mut self, player: Uuid) -> bool;
    /// `players.set-op`.
    fn players_set_op(&mut self, player: Uuid, op: bool) -> ();

    // ---- inventories ----
    /// `inventories.get-slot`.
    fn inventories_get_slot(&mut self, player: Uuid, slot: u32) -> ItemStack;
    /// `inventories.set-slot`.
    fn inventories_set_slot(&mut self, player: Uuid, slot: u32, item: &ItemStack) -> ();
    /// `inventories.contents` (41 slots).
    fn inventories_contents(&mut self, player: Uuid) -> Vec<ItemStack>;
    /// `inventories.give`: returns what did not fit.
    fn inventories_give(&mut self, player: Uuid, item: &ItemStack) -> ItemStack;
    /// `inventories.take`: returns how many were removed.
    fn inventories_take(&mut self, player: Uuid, id: &str, count: u32) -> u32;
    /// `inventories.clear`.
    fn inventories_clear(&mut self, player: Uuid) -> ();
    /// `inventories.held-slot`.
    fn inventories_held_slot(&mut self, player: Uuid) -> u32;
    /// `inventories.close-container`.
    fn inventories_close_container(&mut self, player: Uuid) -> ();

    // ---- menus ----
    /// `menus.open`.
    fn menus_open(&mut self, player: Uuid, spec: &MenuSpec) -> MenuId;
    /// `menus.set-item`.
    fn menus_set_item(&mut self, menu: MenuId, slot: u32, item: &ItemStack) -> ();
    /// `menus.set-title`.
    fn menus_set_title(&mut self, menu: MenuId, title: &Text) -> ();
    /// `menus.close`.
    fn menus_close(&mut self, menu: MenuId) -> ();
    /// The plugin menu `player` has open, whatever plugin owns it (the core filters by owner).
    fn menus_open_menu(&mut self, player: Uuid) -> Option<MenuId>;

    // ---- worlds ----
    /// `worlds.all`.
    fn worlds_all(&mut self) -> Vec<String>;
    /// `worlds.get-block`.
    fn worlds_get_block(&mut self, world: &str, pos: BlockPos) -> BlockState;
    /// `worlds.set-block`.
    fn worlds_set_block(&mut self, world: &str, pos: BlockPos, state: &BlockState, physics: bool) -> bool;
    /// `worlds.set-blocks`.
    fn worlds_set_blocks(&mut self, world: &str, blocks: &[(BlockPos, BlockState)], physics: bool) -> u32;
    /// `worlds.fill` (at most 32768 blocks).
    fn worlds_fill(&mut self, world: &str, from: BlockPos, to: BlockPos, state: &BlockState) -> u32;
    /// `worlds.highest-block-y`.
    fn worlds_highest_block_y(&mut self, world: &str, x: i32, z: i32) -> i32;
    /// `worlds.is-chunk-loaded`.
    fn worlds_is_chunk_loaded(&mut self, world: &str, cx: i32, cz: i32) -> bool;
    /// `worlds.keep-chunk-loaded` (the core reference-counts per plugin).
    fn worlds_keep_chunk_loaded(&mut self, world: &str, cx: i32, cz: i32, keep: bool) -> ();
    /// `worlds.time`.
    fn worlds_time(&mut self, world: &str) -> u64;
    /// `worlds.set-time`.
    fn worlds_set_time(&mut self, world: &str, time: u64) -> ();
    /// `worlds.get-weather`.
    fn worlds_get_weather(&mut self, world: &str) -> Weather;
    /// `worlds.set-weather`.
    fn worlds_set_weather(&mut self, world: &str, weather: Weather, duration_ticks: u32) -> ();
    /// `worlds.spawn`.
    fn worlds_spawn(&mut self, world: &str) -> Location;
    /// `worlds.set-spawn`.
    fn worlds_set_spawn(&mut self, world: &str, at: &Location) -> ();
    /// `worlds.spawn-entity`.
    fn worlds_spawn_entity(&mut self, world: &str, kind: &str, at: Vec3, nbt: &str) -> EntityRef;
    /// `worlds.remove-entity`.
    fn worlds_remove_entity(&mut self, world: &str, entity: u32) -> ();
    /// `worlds.entities-near`.
    fn worlds_entities_near(&mut self, world: &str, at: Vec3, radius: f64) -> Vec<EntityRef>;
    /// `worlds.entity-location`.
    fn worlds_entity_location(&mut self, world: &str, entity: u32) -> Location;
    /// `worlds.spawn-particle`.
    fn worlds_spawn_particle(&mut self, world: &str, particle: &str, options: &str, at: Vec3, count: u32, offset: Vec3, speed: f64) -> ();
    /// `worlds.play-sound`.
    fn worlds_play_sound(&mut self, world: &str, sound: &str, at: Vec3, volume: f32, pitch: f32) -> ();
    /// `worlds.players`.
    fn worlds_players(&mut self, world: &str) -> Vec<Uuid>;

    // ---- scoreboards ----
    /// `scoreboards.create-objective`.
    fn scoreboards_create_objective(&mut self, name: &str, display: &Text, criteria: &str) -> ();
    /// `scoreboards.remove-objective`.
    fn scoreboards_remove_objective(&mut self, name: &str) -> ();
    /// `scoreboards.set-display`.
    fn scoreboards_set_display(&mut self, slot: DisplaySlot, objective: Option<&str>) -> ();
    /// `scoreboards.set-objective-display`.
    fn scoreboards_set_objective_display(&mut self, name: &str, display: &Text) -> ();
    /// `scoreboards.set-score`.
    fn scoreboards_set_score(&mut self, objective: &str, entry: &str, score: i32) -> ();
    /// `scoreboards.reset-score`.
    fn scoreboards_reset_score(&mut self, objective: &str, entry: &str) -> ();
    /// `scoreboards.get-score`.
    fn scoreboards_get_score(&mut self, objective: &str, entry: &str) -> Option<i32>;
    /// `scoreboards.create-team`.
    fn scoreboards_create_team(&mut self, name: &str) -> ();
    /// `scoreboards.remove-team`.
    fn scoreboards_remove_team(&mut self, name: &str) -> ();
    /// `scoreboards.set-team-prefix`.
    fn scoreboards_set_team_prefix(&mut self, name: &str, prefix: &Text) -> ();
    /// `scoreboards.set-team-suffix`.
    fn scoreboards_set_team_suffix(&mut self, name: &str, suffix: &Text) -> ();
    /// `scoreboards.set-team-color`.
    fn scoreboards_set_team_color(&mut self, name: &str, color: TeamColor) -> ();
    /// `scoreboards.team-add`.
    fn scoreboards_team_add(&mut self, name: &str, entry: &str) -> ();
    /// `scoreboards.team-remove`.
    fn scoreboards_team_remove(&mut self, name: &str, entry: &str) -> ();

    // ---- bossbars ----
    /// `bossbars.create`.
    fn bossbars_create(&mut self, title: &Text, color: BarColor, style: BarStyle, progress: f32) -> BarId;
    /// `bossbars.set-title`.
    fn bossbars_set_title(&mut self, bar: BarId, title: &Text) -> ();
    /// `bossbars.set-progress`.
    fn bossbars_set_progress(&mut self, bar: BarId, progress: f32) -> ();
    /// `bossbars.set-color`.
    fn bossbars_set_color(&mut self, bar: BarId, color: BarColor) -> ();
    /// `bossbars.show`.
    fn bossbars_show(&mut self, bar: BarId, player: Uuid) -> ();
    /// `bossbars.hide`.
    fn bossbars_hide(&mut self, bar: BarId, player: Uuid) -> ();
    /// `bossbars.remove`.
    fn bossbars_remove(&mut self, bar: BarId) -> ();

    // ---- tablist ----
    /// `tablist.set-header-footer`.
    fn tablist_set_header_footer(&mut self, player: Uuid, header: &Text, footer: &Text) -> ();
    /// `tablist.set-display-name`.
    fn tablist_set_display_name(&mut self, player: Uuid, name: Option<&Text>) -> ();

    // ---- messaging ----
    /// Start forwarding custom payloads of `channel` to the core (reference-counted by the
    /// core: called once for the first plugin registering it).
    fn messaging_register_incoming(&mut self, channel: &str) -> ();
    /// Stop forwarding `channel` (called when the last plugin unregisters it).
    fn messaging_unregister_incoming(&mut self, channel: &str) -> ();
    /// `messaging.send`.
    fn messaging_send(&mut self, player: Uuid, channel: &str, data: &[u8]) -> ();
    /// `messaging.behind-proxy`.
    fn messaging_behind_proxy(&mut self) -> bool;
}

/// A backend that supports nothing (every call returns `Unsupported`). Useful for tests and
/// for code paths that need a backend before the engine is up.
#[derive(Clone, Copy, Debug, Default)]
pub struct NullBackend;

impl Backend for NullBackend {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_unsupported() {
        let mut b = NullBackend;
        assert_eq!(
            b.players_online(),
            Err(HostError::Unsupported("players_online".into()))
        );
        assert!(b.console_message(&Text::plain("hi")).is_ok());
        b.plugin_disabled("x");
        b.commands_changed();
        let dynb: &mut dyn Backend = &mut b;
        assert!(dynb.worlds_all().is_err());
    }
}
