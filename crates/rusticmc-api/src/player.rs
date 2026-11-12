//! Players: a cheap, copyable handle keyed by UUID. Every call asks the host, so a handle never
//! goes stale; calls on an offline player return [`HostError::NoSuchPlayer`](crate::HostError)
//! (or are silently dropped for the fire-and-forget messaging helpers).

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::bindings::rustic::api::{messaging, players as wit, tablist};
use crate::error::Result;
use crate::host::call;
use crate::inventory::PlayerInventory;
use crate::text::Text;
use crate::types::{GameMode, Location, Uuid};
use crate::world::World;

/// An online (or recently online) player.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Player {
    uuid: Uuid,
}

impl Player {
    /// The handle for a UUID (no host call).
    pub const fn from_uuid(uuid: Uuid) -> Player {
        Player { uuid }
    }

    /// The UUID.
    pub const fn uuid(&self) -> Uuid {
        self.uuid
    }

    /// Everyone online.
    pub fn online() -> Vec<Player> {
        call!("players.online", wit::online())
            .into_iter()
            .map(Player::from_uuid)
            .collect()
    }

    /// The online player with this name (case-insensitive on the host).
    pub fn by_name(name: &str) -> Option<Player> {
        call!("players.by-name", wit::by_name(name)).map(Player::from_uuid)
    }

    /// Whether the player is online.
    pub fn is_online(&self) -> bool {
        call!("players.is-online", wit::is_online(self.uuid))
    }

    /// The player's name (`None` if unknown).
    pub fn name(&self) -> Option<String> {
        call!("players.name", wit::name(self.uuid))
    }

    /// Where the player is.
    pub fn location(&self) -> Result<Location> {
        Ok(call!("players.get-location", wit::get_location(self.uuid))?)
    }

    /// The world the player is in.
    pub fn world(&self) -> Result<World> {
        Ok(self.location()?.world())
    }

    /// The client's IP address.
    pub fn address(&self) -> Result<String> {
        Ok(call!("players.address", wit::address(self.uuid))?)
    }

    /// Round-trip latency in milliseconds.
    pub fn ping(&self) -> Result<u32> {
        Ok(call!("players.ping", wit::ping(self.uuid))?)
    }

    /// The client's language (`en_us`).
    pub fn locale(&self) -> Result<String> {
        Ok(call!("players.locale", wit::locale(self.uuid))?)
    }

    /// Sends a chat message; dropped if the player went offline.
    pub fn send_message(&self, message: impl Into<Text>) {
        let _ = self.try_send_message(message);
    }

    /// Sends a chat message, reporting failures.
    pub fn try_send_message(&self, message: impl Into<Text>) -> Result<()> {
        let json = message.into().to_json();
        Ok(call!(
            "players.send-message",
            wit::send_message(self.uuid, &json)
        )?)
    }

    /// Shows text above the hotbar; dropped if offline.
    pub fn action_bar(&self, message: impl Into<Text>) {
        let json = message.into().to_json();
        let _ = call!(
            "players.send-action-bar",
            wit::send_action_bar(self.uuid, &json)
        );
    }

    /// Shows a title and subtitle with default timings (10 / 70 / 20 ticks); dropped if offline.
    pub fn title(&self, title: impl Into<Text>, subtitle: impl Into<Text>) {
        let _ = self.title_with(title, subtitle, 10, 70, 20);
    }

    /// Shows a title with timings in ticks.
    pub fn title_with(
        &self,
        title: impl Into<Text>,
        subtitle: impl Into<Text>,
        fade_in: u32,
        stay: u32,
        fade_out: u32,
    ) -> Result<()> {
        let (t, s) = (title.into().to_json(), subtitle.into().to_json());
        Ok(call!(
            "players.send-title",
            wit::send_title(self.uuid, &t, &s, fade_in, stay, fade_out)
        )?)
    }

    /// Plays a sound event (`minecraft:entity.experience_orb.pickup`) to the player only;
    /// dropped if offline.
    pub fn sound(&self, sound: &str, volume: f32, pitch: f32) {
        let _ = call!(
            "players.play-sound",
            wit::play_sound(self.uuid, sound, volume, pitch)
        );
    }

    /// Teleports the player. Main instance only.
    pub fn teleport(&self, to: &Location) -> Result<()> {
        Ok(call!("players.teleport", wit::teleport(self.uuid, to))?)
    }

    /// The game mode.
    pub fn game_mode(&self) -> Result<GameMode> {
        Ok(call!(
            "players.get-game-mode",
            wit::get_game_mode(self.uuid)
        )?)
    }

    /// Changes the game mode.
    pub fn set_game_mode(&self, mode: GameMode) -> Result<()> {
        Ok(call!(
            "players.set-game-mode",
            wit::set_game_mode(self.uuid, mode)
        )?)
    }

    /// Health points (20 = ten hearts).
    pub fn health(&self) -> Result<f32> {
        Ok(call!("players.health", wit::health(self.uuid))?)
    }

    /// Sets health (clamped by the host to `0..=max_health`).
    pub fn set_health(&self, health: f32) -> Result<()> {
        Ok(call!(
            "players.set-health",
            wit::set_health(self.uuid, health)
        )?)
    }

    /// Maximum health.
    pub fn max_health(&self) -> Result<f32> {
        Ok(call!("players.max-health", wit::max_health(self.uuid))?)
    }

    /// Food level (0..=20).
    pub fn food_level(&self) -> Result<u32> {
        Ok(call!("players.food-level", wit::food_level(self.uuid))?)
    }

    /// Sets the food level.
    pub fn set_food_level(&self, food: u32) -> Result<()> {
        Ok(call!(
            "players.set-food-level",
            wit::set_food_level(self.uuid, food)
        )?)
    }

    /// Saturation.
    pub fn saturation(&self) -> Result<f32> {
        Ok(call!("players.saturation", wit::saturation(self.uuid))?)
    }

    /// Sets saturation.
    pub fn set_saturation(&self, saturation: f32) -> Result<()> {
        Ok(call!(
            "players.set-saturation",
            wit::set_saturation(self.uuid, saturation)
        )?)
    }

    /// Experience level.
    pub fn level(&self) -> Result<u32> {
        Ok(call!(
            "players.experience-level",
            wit::experience_level(self.uuid)
        )?)
    }

    /// Sets the experience level.
    pub fn set_level(&self, level: u32) -> Result<()> {
        Ok(call!(
            "players.set-experience-level",
            wit::set_experience_level(self.uuid, level)
        )?)
    }

    /// Allows flight and/or makes the player fly.
    pub fn set_flying(&self, allow: bool, flying: bool) -> Result<()> {
        Ok(call!(
            "players.set-flying",
            wit::set_flying(self.uuid, allow, flying)
        )?)
    }

    /// Disconnects the player with a reason.
    pub fn kick(&self, reason: impl Into<Text>) -> Result<()> {
        let json = reason.into().to_json();
        Ok(call!("players.kick", wit::kick(self.uuid, &json))?)
    }

    /// Whether the player is a server operator.
    pub fn is_op(&self) -> bool {
        call!("players.is-op", wit::is_op(self.uuid))
    }

    /// Grants or revokes operator status.
    pub fn set_op(&self, op: bool) -> Result<()> {
        Ok(call!("players.set-op", wit::set_op(self.uuid, op))?)
    }

    /// Whether the player has a permission node.
    pub fn has_permission(&self, node: &str) -> bool {
        crate::permissions::Permissions::has(crate::command::Sender::Player(*self), node)
    }

    /// This plugin's persistent per-player data (saved with the player).
    pub fn data(&self) -> PlayerData {
        PlayerData { uuid: self.uuid }
    }

    /// The player's inventory.
    pub fn inventory(&self) -> PlayerInventory {
        PlayerInventory::new(self.uuid)
    }

    /// Sets the tab-list header and footer for this player.
    pub fn set_tab_header_footer(
        &self,
        header: impl Into<Text>,
        footer: impl Into<Text>,
    ) -> Result<()> {
        let (h, f) = (header.into().to_json(), footer.into().to_json());
        Ok(call!(
            "tablist.set-header-footer",
            tablist::set_header_footer(self.uuid, &h, &f)
        )?)
    }

    /// The name shown for this player in everyone's tab list (`None` restores it).
    pub fn set_tab_name(&self, name: Option<Text>) -> Result<()> {
        let json = name.map(|t| t.to_json());
        Ok(call!(
            "tablist.set-display-name",
            tablist::set_display_name(self.uuid, json.as_deref())
        )?)
    }

    /// Sends a custom payload on a plugin channel.
    pub fn send_plugin_message(&self, channel: &str, data: &[u8]) -> Result<()> {
        Ok(call!(
            "messaging.send",
            messaging::send(self.uuid, channel, data)
        )?)
    }
}

/// Persistent per-player key-value data in this plugin's namespace, with serde helpers
/// (values are stored as JSON).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerData {
    uuid: Uuid,
}

impl PlayerData {
    /// A typed value.
    pub fn get<T: DeserializeOwned>(&self, key: &str) -> Result<Option<T>> {
        match self.get_raw(key) {
            Some(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            None => Ok(None),
        }
    }

    /// A typed value, or `T::default()`.
    pub fn get_or_default<T: DeserializeOwned + Default>(&self, key: &str) -> Result<T> {
        Ok(self.get(key)?.unwrap_or_default())
    }

    /// Stores a typed value.
    pub fn set<T: Serialize + ?Sized>(&self, key: &str, value: &T) -> Result<()> {
        let bytes = serde_json::to_vec(value)?;
        self.set_raw(key, &bytes);
        Ok(())
    }

    /// Raw bytes.
    pub fn get_raw(&self, key: &str) -> Option<Vec<u8>> {
        call!("players.get-data", wit::get_data(self.uuid, key))
    }

    /// Stores raw bytes.
    pub fn set_raw(&self, key: &str, value: &[u8]) {
        call!("players.set-data", wit::set_data(self.uuid, key, value));
    }

    /// Deletes a key; returns whether it existed.
    pub fn delete(&self, key: &str) -> bool {
        call!("players.delete-data", wit::delete_data(self.uuid, key))
    }
}
