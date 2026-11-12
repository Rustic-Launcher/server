//! Plugin messaging (custom payload channels) and proxy information. Incoming messages arrive as
//! [`PluginMessageEvent`](crate::event::PluginMessageEvent)s on registered channels.

use crate::__rt;
use crate::bindings::rustic::api::messaging as wit;
use crate::error::{Error, Result};
use crate::host::call;
use crate::player::Player;

/// Plugin channels.
#[derive(Debug, Clone, Copy)]
pub struct Messaging;

impl Messaging {
    /// Receives `plugin-message` events for `channel` (`namespace:path`). No-op in the async
    /// instance.
    pub fn register_incoming(channel: &str) -> Result<()> {
        if !__rt::is_main() {
            return Ok(());
        }
        call!(
            "messaging.register-incoming",
            wit::register_incoming(channel)
        )
        .map_err(Error::Other)
    }

    /// Stops receiving a channel.
    pub fn unregister_incoming(channel: &str) {
        call!(
            "messaging.unregister-incoming",
            wit::unregister_incoming(channel)
        );
    }

    /// Sends a payload to a player's client.
    pub fn send(player: &Player, channel: &str, data: &[u8]) -> Result<()> {
        player.send_plugin_message(channel, data)
    }

    /// Whether the server sits behind a proxy with modern forwarding.
    pub fn behind_proxy() -> bool {
        call!("messaging.behind-proxy", wit::behind_proxy())
    }
}
