//! The server: broadcast, console commands, tick stats, plugin list.

use crate::bindings::rustic::api::server as wit;
use crate::host::call;
use crate::player::Player;
use crate::text::Text;
use crate::types::{PluginMeta, TickStats};

/// Server-wide calls (all associated functions).
#[derive(Debug, Clone, Copy)]
pub struct Server;

impl Server {
    /// Messages every online player and the console.
    pub fn broadcast(message: impl Into<Text>) {
        let json = message.into().to_json();
        call!("server.broadcast", wit::broadcast(&json));
    }

    /// Runs a command as the console (no slash); returns whether it ran. Main instance only.
    pub fn dispatch_command(command: &str) -> bool {
        let command = command.strip_prefix('/').unwrap_or(command);
        call!("server.dispatch-command", wit::dispatch_command(command))
    }

    /// Mean ms per tick and TPS.
    pub fn tick_stats() -> TickStats {
        call!("server.get-tick-stats", wit::get_tick_stats())
    }

    /// The player limit.
    pub fn max_players() -> u32 {
        call!("server.max-players", wit::max_players())
    }

    /// The message of the day.
    pub fn motd() -> Text {
        Text::from_json(call!("server.motd", wit::motd()))
    }

    /// `Rustic <version> on Horizon <commit> (...)`.
    pub fn version() -> String {
        call!("server.version", wit::version())
    }

    /// Every loaded plugin.
    pub fn plugins() -> Vec<PluginMeta> {
        call!("server.plugins", wit::plugins())
    }

    /// This plugin's metadata.
    pub fn self_meta() -> PluginMeta {
        call!("server.self-meta", wit::self_meta())
    }

    /// A player that ever joined, by name (from the user cache), online or not.
    pub fn offline_player(name: &str) -> Option<Player> {
        call!("server.offline-player", wit::offline_player(name)).map(Player::from_uuid)
    }

    /// Server operators.
    pub fn operators() -> Vec<Player> {
        call!("server.operators", wit::operators())
            .into_iter()
            .map(Player::from_uuid)
            .collect()
    }

    /// Everyone online (same as [`Player::online`]).
    pub fn online_players() -> Vec<Player> {
        Player::online()
    }
}
