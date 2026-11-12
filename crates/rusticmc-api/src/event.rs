//! Typed events.
//!
//! Each WIT event record has a wrapper with getters for every field and setters **only** for the
//! fields the host applies back (documented "mut" in `events.wit`), plus [`Cancellable`] on
//! cancellable events. A handler receives `&mut XxxEvent`; whatever it changes is returned to the
//! host, which applies the mutable fields and `cancelled` (and ignores changes made by `monitor`
//! handlers).
//!
//! Subscribe with `ctx.on(|e: &mut PlayerJoinEvent| ...)`, [`Events::subscribe`], or
//! `#[event_handler]` methods in a `#[rustic::handlers]` impl block.

use std::any::Any;
use std::cell::RefCell;
use std::rc::Rc;

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::__rt;
use crate::bindings::rustic::api::event_types as raw;
use crate::bindings::rustic::api::events as wit;
use crate::error::{Error, Result};
use crate::host::call;
use crate::item::ItemStack;
use crate::player::Player;
use crate::text::Text;
use crate::types::{
    BlockFace, BlockPos, BlockState, ClickKind, EntityRef, GameMode, Hand, InteractAction,
    Location, Priority, Uuid,
};
use crate::world::World;

/// The raw WIT event records, for code that needs them (tests, harnesses).
pub use crate::bindings::rustic::api::event_types as records;

/// An event type: its WIT name and its raw record.
pub trait Event: Sized + 'static {
    /// The kebab-case event name used to subscribe (`"player-join"`).
    const NAME: &'static str;
    /// The WIT record.
    type Raw;
    /// Wraps a raw record.
    fn from_raw(raw: Self::Raw) -> Self;
    /// Unwraps into the raw record handed back to the host.
    fn into_raw(self) -> Self::Raw;
}

/// An event a handler can cancel.
pub trait Cancellable: Event {
    /// Whether the event is cancelled (by this or an earlier handler).
    fn is_cancelled(&self) -> bool;
    /// Cancels or un-cancels it.
    fn set_cancelled(&mut self, cancelled: bool);
    /// Cancels it.
    fn cancel(&mut self) {
        self.set_cancelled(true);
    }
}

macro_rules! event {
    ($(#[$m:meta])* $name:ident, $raw:ident, $wit:literal) => {
        $(#[$m])*
        #[derive(Debug, Clone, PartialEq)]
        pub struct $name {
            raw: raw::$raw,
        }

        impl Event for $name {
            const NAME: &'static str = $wit;
            type Raw = raw::$raw;
            fn from_raw(raw: raw::$raw) -> Self {
                $name { raw }
            }
            fn into_raw(self) -> raw::$raw {
                self.raw
            }
        }

        impl $name {
            /// Wraps a raw record (for tests and harnesses; handlers receive events ready-made).
            pub fn new(raw: raw::$raw) -> Self {
                $name { raw }
            }

            /// The raw record.
            pub fn raw(&self) -> &raw::$raw {
                &self.raw
            }
        }
    };
}

macro_rules! cancellable {
    ($($name:ident),* $(,)?) => {$(
        impl Cancellable for $name {
            fn is_cancelled(&self) -> bool {
                self.raw.cancelled
            }
            fn set_cancelled(&mut self, cancelled: bool) {
                self.raw.cancelled = cancelled;
            }
        }
    )*};
}

/// Getter for the `player: uuid` field of online-player events.
macro_rules! player_getter {
    ($($name:ident),* $(,)?) => {$(
        impl $name {
            /// The player.
            pub fn player(&self) -> Player {
                Player::from_uuid(self.raw.player)
            }
        }
    )*};
}

/// Getter for the `name: string` field.
macro_rules! name_getter {
    ($($name:ident),* $(,)?) => {$(
        impl $name {
            /// The player's name.
            pub fn name(&self) -> &str {
                &self.raw.name
            }
        }
    )*};
}

fn opt_text(t: &Option<String>) -> Option<Text> {
    t.as_ref().map(|j| Text::from_json(j.clone()))
}

event!(
    /// Before a player is admitted (name, UUID and address known; nothing loaded yet).
    /// Cancel to refuse them with the kick message.
    PlayerPreLoginEvent, PlayerPreLogin, "player-pre-login"
);
event!(
    /// A player joined and is in the world.
    PlayerJoinEvent, PlayerJoin, "player-join"
);
event!(
    /// A player left.
    PlayerQuitEvent, PlayerQuit, "player-quit"
);
event!(
    /// A chat message. The typed message is read-only (signed by the client); the displayed
    /// line can be replaced.
    PlayerChatEvent, PlayerChat, "player-chat"
);
event!(
    /// A command a player typed (without slash), before it runs.
    PlayerCommandEvent, PlayerCommand, "player-command"
);
event!(
    /// A command typed on the console (without slash), before it runs.
    ServerCommandEvent, ServerCommand, "server-command"
);
event!(
    /// Movement past the server's threshold, at most once per tick per player. Cancelling puts
    /// the player back to `from`.
    PlayerMoveEvent, PlayerMove, "player-move"
);
event!(
    /// A player was teleported.
    PlayerTeleportEvent, PlayerTeleport, "player-teleport"
);
event!(
    /// A player changed world (dimension).
    PlayerChangeWorldEvent, PlayerChangeWorld, "player-change-world"
);
event!(
    /// A player broke a block. Cancel to keep the block.
    BlockBreakEvent, BlockBreak, "block-break"
);
event!(
    /// A player placed a block. Cancel to refuse the placement.
    BlockPlaceEvent, BlockPlace, "block-place"
);
event!(
    /// A player clicked a block or the air.
    PlayerInteractEvent, PlayerInteract, "player-interact"
);
event!(
    /// A player right-clicked an entity.
    PlayerInteractEntityEvent, PlayerInteractEntity, "player-interact-entity"
);
event!(
    /// A living entity (players included) is about to take damage.
    EntityDamageEvent, EntityDamage, "entity-damage"
);
event!(
    /// A player died.
    PlayerDeathEvent, PlayerDeath, "player-death"
);
event!(
    /// A non-player living entity died.
    EntityDeathEvent, EntityDeath, "entity-death"
);
event!(
    /// A player is respawning.
    PlayerRespawnEvent, PlayerRespawn, "player-respawn"
);
event!(
    /// A click in an open container; `menu()` is set for plugin menus (cancelled by default).
    InventoryClickEvent, InventoryClick, "inventory-click"
);
event!(
    /// A container was closed.
    InventoryCloseEvent, InventoryClose, "inventory-close"
);
event!(
    /// A player's game mode is changing.
    PlayerGameModeChangeEvent, PlayerGameModeChange, "player-game-mode-change"
);
event!(
    /// The server-list ping (status request).
    ServerListPingEvent, ServerListPing, "server-list-ping"
);
event!(
    /// A custom payload a client sent on a channel this plugin registered.
    PluginMessageEvent, PluginMessage, "plugin-message"
);
event!(
    /// A chunk was loaded.
    ChunkLoadEvent, ChunkLoad, "chunk-load"
);
event!(
    /// A chunk is being unloaded.
    ChunkUnloadEvent, ChunkUnload, "chunk-unload"
);
event!(
    /// An event a plugin fired with [`Events::call_custom`].
    CustomEvent, CustomEvent, "custom-event"
);

cancellable!(
    PlayerPreLoginEvent,
    PlayerChatEvent,
    PlayerCommandEvent,
    ServerCommandEvent,
    PlayerMoveEvent,
    BlockBreakEvent,
    BlockPlaceEvent,
    PlayerInteractEvent,
    PlayerInteractEntityEvent,
    EntityDamageEvent,
    InventoryClickEvent,
    PlayerGameModeChangeEvent,
    CustomEvent,
);

player_getter!(
    PlayerJoinEvent,
    PlayerQuitEvent,
    PlayerChatEvent,
    PlayerCommandEvent,
    PlayerMoveEvent,
    PlayerTeleportEvent,
    PlayerChangeWorldEvent,
    BlockBreakEvent,
    BlockPlaceEvent,
    PlayerInteractEvent,
    PlayerInteractEntityEvent,
    PlayerDeathEvent,
    PlayerRespawnEvent,
    InventoryClickEvent,
    InventoryCloseEvent,
    PlayerGameModeChangeEvent,
    PluginMessageEvent,
);

name_getter!(
    PlayerPreLoginEvent,
    PlayerJoinEvent,
    PlayerQuitEvent,
    PlayerChatEvent,
    PlayerCommandEvent,
);

impl PlayerPreLoginEvent {
    /// The joining player's UUID (not online yet, so no [`Player`] handle).
    pub fn uuid(&self) -> Uuid {
        self.raw.player
    }

    /// The client's address.
    pub fn address(&self) -> &str {
        &self.raw.address
    }

    /// The kick message shown when cancelled.
    pub fn kick_message(&self) -> Option<Text> {
        opt_text(&self.raw.kick_message)
    }

    /// Sets the kick message.
    pub fn set_kick_message(&mut self, message: impl Into<Text>) {
        self.raw.kick_message = Some(message.into().to_json());
    }

    /// Refuses the player with `message` (cancel + kick message).
    pub fn deny(&mut self, message: impl Into<Text>) {
        self.set_kick_message(message);
        self.cancel();
    }
}

impl PlayerJoinEvent {
    /// The broadcast join line (`None`: hidden).
    pub fn join_message(&self) -> Option<Text> {
        opt_text(&self.raw.join_message)
    }

    /// Replaces the broadcast join line.
    pub fn set_join_message(&mut self, message: impl Into<Text>) {
        self.raw.join_message = Some(message.into().to_json());
    }

    /// Hides the broadcast join line.
    pub fn hide_join_message(&mut self) {
        self.raw.join_message = None;
    }
}

impl PlayerQuitEvent {
    /// The broadcast quit line (`None`: hidden).
    pub fn quit_message(&self) -> Option<Text> {
        opt_text(&self.raw.quit_message)
    }

    /// Replaces the broadcast quit line.
    pub fn set_quit_message(&mut self, message: impl Into<Text>) {
        self.raw.quit_message = Some(message.into().to_json());
    }

    /// Hides the broadcast quit line.
    pub fn hide_quit_message(&mut self) {
        self.raw.quit_message = None;
    }
}

impl PlayerChatEvent {
    /// The message as typed (read-only).
    pub fn message(&self) -> &str {
        &self.raw.message
    }

    /// The replacement line everyone sees, if a handler set one.
    pub fn display(&self) -> Option<Text> {
        opt_text(&self.raw.display)
    }

    /// Replaces the displayed line (the signed body is kept).
    pub fn set_display(&mut self, line: impl Into<Text>) {
        self.raw.display = Some(line.into().to_json());
    }

    /// Goes back to the default `<name> message` line.
    pub fn reset_display(&mut self) {
        self.raw.display = None;
    }
}

impl PlayerCommandEvent {
    /// The command line, without slash.
    pub fn command(&self) -> &str {
        &self.raw.command
    }
}

impl ServerCommandEvent {
    /// The command line, without slash.
    pub fn command(&self) -> &str {
        &self.raw.command
    }
}

impl PlayerMoveEvent {
    /// Where the player was.
    pub fn from(&self) -> &Location {
        &self.raw.from
    }

    /// Where the player moved to.
    pub fn to(&self) -> &Location {
        &self.raw.to
    }

    /// Whether the player changed block (not just looked around or moved inside one).
    pub fn changed_block(&self) -> bool {
        self.raw.from.block() != self.raw.to.block() || self.raw.from.world != self.raw.to.world
    }
}

impl PlayerTeleportEvent {
    /// Where the player was.
    pub fn from(&self) -> &Location {
        &self.raw.from
    }

    /// Where the player went.
    pub fn to(&self) -> &Location {
        &self.raw.to
    }
}

impl PlayerChangeWorldEvent {
    /// The world left.
    pub fn from_world(&self) -> World {
        World::new(self.raw.from_world.clone())
    }

    /// The world entered.
    pub fn to_world(&self) -> World {
        World::new(self.raw.to_world.clone())
    }
}

impl BlockBreakEvent {
    /// The world.
    pub fn world(&self) -> World {
        World::new(self.raw.world.clone())
    }

    /// The block's position.
    pub fn pos(&self) -> BlockPos {
        self.raw.pos
    }

    /// The block being broken.
    pub fn block(&self) -> &BlockState {
        &self.raw.block
    }

    /// Whether the block drops items.
    pub fn drop_items(&self) -> bool {
        self.raw.drop_items
    }

    /// `false` drops nothing.
    pub fn set_drop_items(&mut self, drop: bool) {
        self.raw.drop_items = drop;
    }
}

impl BlockPlaceEvent {
    /// The world.
    pub fn world(&self) -> World {
        World::new(self.raw.world.clone())
    }

    /// The block's position.
    pub fn pos(&self) -> BlockPos {
        self.raw.pos
    }

    /// The block being placed.
    pub fn block(&self) -> &BlockState {
        &self.raw.block
    }

    /// The hand holding the item.
    pub fn hand(&self) -> Hand {
        self.raw.hand
    }
}

impl PlayerInteractEvent {
    /// What kind of click.
    pub fn action(&self) -> InteractAction {
        self.raw.action
    }

    /// The hand used.
    pub fn hand(&self) -> Hand {
        self.raw.hand
    }

    /// The item in that hand.
    pub fn item(&self) -> ItemStack {
        ItemStack::from_raw(self.raw.item.clone())
    }

    /// The world.
    pub fn world(&self) -> World {
        World::new(self.raw.world.clone())
    }

    /// The block clicked, if any.
    pub fn block(&self) -> Option<BlockPos> {
        self.raw.block
    }

    /// The face clicked, if a block was.
    pub fn face(&self) -> Option<BlockFace> {
        self.raw.face
    }
}

impl PlayerInteractEntityEvent {
    /// The entity clicked.
    pub fn target(&self) -> &EntityRef {
        &self.raw.target
    }

    /// The hand used.
    pub fn hand(&self) -> Hand {
        self.raw.hand
    }
}

impl EntityDamageEvent {
    /// The entity taking damage.
    pub fn target(&self) -> &EntityRef {
        &self.raw.target
    }

    /// The damage type id (`minecraft:fall`).
    pub fn cause(&self) -> &str {
        &self.raw.cause
    }

    /// The entity that dealt it (the shooter for projectiles).
    pub fn attacker(&self) -> Option<&EntityRef> {
        self.raw.attacker.as_ref()
    }

    /// The damage amount.
    pub fn amount(&self) -> f32 {
        self.raw.amount
    }

    /// Changes the damage amount.
    pub fn set_amount(&mut self, amount: f32) {
        self.raw.amount = amount.max(0.0);
    }
}

impl PlayerDeathEvent {
    /// The damage type id that killed the player.
    pub fn cause(&self) -> &str {
        &self.raw.cause
    }

    /// The broadcast death line (`None`: hidden).
    pub fn death_message(&self) -> Option<Text> {
        opt_text(&self.raw.death_message)
    }

    /// Replaces the death line.
    pub fn set_death_message(&mut self, message: impl Into<Text>) {
        self.raw.death_message = Some(message.into().to_json());
    }

    /// Hides the death line.
    pub fn hide_death_message(&mut self) {
        self.raw.death_message = None;
    }

    /// Whether the inventory is kept.
    pub fn keep_inventory(&self) -> bool {
        self.raw.keep_inventory
    }

    /// Keeps (or drops) the inventory.
    pub fn set_keep_inventory(&mut self, keep: bool) {
        self.raw.keep_inventory = keep;
    }

    /// Whether experience levels are kept.
    pub fn keep_level(&self) -> bool {
        self.raw.keep_level
    }

    /// Keeps (or drops) experience levels.
    pub fn set_keep_level(&mut self, keep: bool) {
        self.raw.keep_level = keep;
    }
}

impl EntityDeathEvent {
    /// The entity that died.
    pub fn entity(&self) -> &EntityRef {
        &self.raw.entity
    }

    /// The player who killed it, if any.
    pub fn killer(&self) -> Option<Player> {
        self.raw.killer.map(Player::from_uuid)
    }

    /// What it drops.
    pub fn drops(&self) -> Vec<ItemStack> {
        self.raw
            .drops
            .iter()
            .cloned()
            .map(ItemStack::from_raw)
            .collect()
    }

    /// Replaces the drops.
    pub fn set_drops(&mut self, drops: impl IntoIterator<Item = ItemStack>) {
        self.raw.drops = drops.into_iter().map(|i| i.to_raw()).collect();
    }

    /// Experience dropped.
    pub fn dropped_exp(&self) -> u32 {
        self.raw.dropped_exp
    }

    /// Changes the experience dropped.
    pub fn set_dropped_exp(&mut self, exp: u32) {
        self.raw.dropped_exp = exp;
    }
}

impl PlayerRespawnEvent {
    /// Where the player respawns.
    pub fn location(&self) -> &Location {
        &self.raw.location
    }

    /// Changes where the player respawns.
    pub fn set_location(&mut self, location: Location) {
        self.raw.location = location;
    }
}

impl InventoryClickEvent {
    /// The plugin menu clicked in, if it is one.
    pub fn menu(&self) -> Option<u32> {
        self.raw.menu
    }

    /// The slot index (negative outside the window).
    pub fn slot(&self) -> i32 {
        self.raw.slot
    }

    /// The kind of click.
    pub fn click(&self) -> ClickKind {
        self.raw.click
    }

    /// The hotbar number for number-key clicks.
    pub fn hotbar_button(&self) -> i32 {
        self.raw.hotbar_button
    }

    /// The item in the clicked slot.
    pub fn item(&self) -> ItemStack {
        ItemStack::from_raw(self.raw.item.clone())
    }

    /// The item on the cursor.
    pub fn cursor(&self) -> ItemStack {
        ItemStack::from_raw(self.raw.cursor.clone())
    }
}

impl InventoryCloseEvent {
    /// The plugin menu closed, if it is one.
    pub fn menu(&self) -> Option<u32> {
        self.raw.menu
    }
}

impl PlayerGameModeChangeEvent {
    /// The current mode.
    pub fn old_mode(&self) -> GameMode {
        self.raw.old_mode
    }

    /// The mode being switched to.
    pub fn new_mode(&self) -> GameMode {
        self.raw.new_mode
    }
}

impl ServerListPingEvent {
    /// The pinging client's address.
    pub fn address(&self) -> &str {
        &self.raw.address
    }

    /// The message of the day.
    pub fn motd(&self) -> Text {
        Text::from_json(self.raw.motd.clone())
    }

    /// Replaces the message of the day.
    pub fn set_motd(&mut self, motd: impl Into<Text>) {
        self.raw.motd = motd.into().to_json();
    }

    /// The advertised maximum player count.
    pub fn max_players(&self) -> u32 {
        self.raw.max_players
    }

    /// Changes the advertised maximum player count.
    pub fn set_max_players(&mut self, max: u32) {
        self.raw.max_players = max;
    }

    /// Players online.
    pub fn online_players(&self) -> u32 {
        self.raw.online_players
    }

    /// Whether the player sample is shown.
    pub fn show_players(&self) -> bool {
        self.raw.show_players
    }

    /// Shows or hides the player sample.
    pub fn set_show_players(&mut self, show: bool) {
        self.raw.show_players = show;
    }
}

impl PluginMessageEvent {
    /// The channel (`namespace:path`).
    pub fn channel(&self) -> &str {
        &self.raw.channel
    }

    /// The payload.
    pub fn data(&self) -> &[u8] {
        &self.raw.data
    }
}

impl ChunkLoadEvent {
    /// The world.
    pub fn world(&self) -> World {
        World::new(self.raw.world.clone())
    }

    /// Chunk coordinates `(x, z)`.
    pub fn chunk(&self) -> (i32, i32) {
        (self.raw.x, self.raw.z)
    }

    /// Whether the chunk was just generated.
    pub fn is_new_chunk(&self) -> bool {
        self.raw.new_chunk
    }
}

impl ChunkUnloadEvent {
    /// The world.
    pub fn world(&self) -> World {
        World::new(self.raw.world.clone())
    }

    /// Chunk coordinates `(x, z)`.
    pub fn chunk(&self) -> (i32, i32) {
        (self.raw.x, self.raw.z)
    }
}

impl CustomEvent {
    /// The event's full name (`<plugin-id>:<name>`).
    pub fn name(&self) -> &str {
        &self.raw.name
    }

    /// The plugin that fired it.
    pub fn source_plugin(&self) -> &str {
        &self.raw.source_plugin
    }

    /// The raw payload.
    pub fn payload(&self) -> &[u8] {
        &self.raw.payload
    }

    /// The payload decoded from JSON (what [`Events::call_custom`] sends).
    pub fn payload_json<T: DeserializeOwned>(&self) -> Result<T> {
        Ok(serde_json::from_slice(&self.raw.payload)?)
    }
}

// ---------------------------------------------------------------------------------------------
// Subscribing

pub(crate) type ErasedHandler = Rc<RefCell<dyn FnMut(&mut dyn Any)>>;

pub(crate) fn erase<E: Event>(mut f: impl FnMut(&mut E) + 'static) -> ErasedHandler {
    Rc::new(RefCell::new(move |any: &mut dyn Any| {
        if let Some(e) = any.downcast_mut::<E>() {
            f(e);
        }
    }))
}

/// A subscription; [`Subscription::unsubscribe`] ends it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Subscription {
    id: Option<u32>,
}

impl Subscription {
    /// The host's handler id (`None` in the async instance, where nothing subscribes).
    pub fn id(&self) -> Option<u32> {
        self.id
    }

    /// Stops receiving the event.
    pub fn unsubscribe(self) {
        if let Some(id) = self.id {
            call!("events.unsubscribe", wit::unsubscribe(id));
            __rt::remove_handler(id);
        }
    }
}

/// Event subscription and custom events.
pub struct Events;

impl Events {
    /// Subscribes `handler` to `E` at `priority`; with `ignore_cancelled`, it is skipped for events
    /// an earlier handler cancelled. In the async instance this does nothing (events go to the
    /// main instance).
    pub fn subscribe<E: Event>(
        priority: Priority,
        ignore_cancelled: bool,
        handler: impl FnMut(&mut E) + 'static,
    ) -> Result<Subscription> {
        Self::subscribe_name(E::NAME, priority, ignore_cancelled, erase(handler))
    }

    /// Subscribes to a custom event fired by any plugin as `<plugin-id>:<name>`.
    pub fn subscribe_custom(
        name: &str,
        priority: Priority,
        ignore_cancelled: bool,
        handler: impl FnMut(&mut CustomEvent) + 'static,
    ) -> Result<Subscription> {
        Self::subscribe_name(
            &format!("custom:{name}"),
            priority,
            ignore_cancelled,
            erase(handler),
        )
    }

    pub(crate) fn subscribe_name(
        event: &str,
        priority: Priority,
        ignore_cancelled: bool,
        handler: ErasedHandler,
    ) -> Result<Subscription> {
        if !__rt::is_main() {
            return Ok(Subscription { id: None });
        }
        let id = call!(
            "events.subscribe",
            wit::subscribe(event, priority, ignore_cancelled)
        )
        .map_err(|e| Error::Other(format!("cannot subscribe to `{event}`: {e}")))?;
        __rt::add_handler(id, handler);
        Ok(Subscription { id: Some(id) })
    }

    /// Fires the custom event `<this-plugin-id>:<name>` with a JSON payload to every subscriber;
    /// returns whether one cancelled it. Main instance only.
    pub fn call_custom<T: Serialize + ?Sized>(name: &str, payload: &T) -> Result<bool> {
        let bytes = serde_json::to_vec(payload)?;
        Ok(Self::call_custom_raw(name, &bytes))
    }

    /// Fires a custom event with raw bytes.
    pub fn call_custom_raw(name: &str, payload: &[u8]) -> bool {
        let full = if name.contains(':') {
            name.to_owned()
        } else {
            format!("{}:{name}", __rt::plugin_id())
        };
        call!("events.call-custom", wit::call_custom(&full, payload))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uuid() -> Uuid {
        Uuid::from_u128(1)
    }

    #[test]
    fn join_message_roundtrip() {
        let mut e = PlayerJoinEvent::new(raw::PlayerJoin {
            player: uuid(),
            name: "Steve".into(),
            join_message: Some(r#"{"text":"Steve joined"}"#.into()),
        });
        assert_eq!(e.name(), "Steve");
        assert_eq!(e.player().uuid(), uuid());
        assert_eq!(e.join_message().unwrap().plain(), "Steve joined");
        e.set_join_message(Text::new("Welcome"));
        assert_eq!(
            e.raw().join_message.as_deref(),
            Some(r#"{"text":"Welcome"}"#)
        );
        e.hide_join_message();
        assert_eq!(e.clone().into_raw().join_message, None);
        assert_eq!(PlayerJoinEvent::NAME, "player-join");
    }

    #[test]
    fn chat_cancel_and_display() {
        let mut e = PlayerChatEvent::from_raw(raw::PlayerChat {
            player: uuid(),
            name: "Alex".into(),
            message: "hello".into(),
            cancelled: false,
            display: None,
        });
        assert!(!e.is_cancelled());
        e.cancel();
        assert!(e.is_cancelled());
        e.set_display("x");
        assert_eq!(e.display().unwrap().plain(), "x");
        let r = e.into_raw();
        assert!(r.cancelled);
        assert_eq!(r.message, "hello");
    }

    #[test]
    fn damage_and_death_mutables() {
        let target = EntityRef {
            id: 3,
            uuid: uuid(),
            kind: "minecraft:player".into(),
        };
        let mut d = EntityDamageEvent::new(raw::EntityDamage {
            target: target.clone(),
            cause: "minecraft:fall".into(),
            attacker: None,
            amount: 4.0,
            cancelled: false,
        });
        d.set_amount(-1.0);
        assert_eq!(d.amount(), 0.0);
        assert!(d.target().as_player().is_some());

        let mut death = EntityDeathEvent::new(raw::EntityDeath {
            entity: target,
            killer: Some(uuid()),
            drops: vec![],
            dropped_exp: 5,
        });
        death.set_drops([ItemStack::new("bone").count(2)]);
        death.set_dropped_exp(0);
        assert_eq!(death.drops()[0].amount(), 2);
        assert_eq!(death.raw().dropped_exp, 0);
        assert_eq!(death.killer().unwrap().uuid(), uuid());
    }

    #[test]
    fn pre_login_deny() {
        let mut e = PlayerPreLoginEvent::new(raw::PlayerPreLogin {
            player: uuid(),
            name: "x".into(),
            address: "127.0.0.1".into(),
            cancelled: false,
            kick_message: None,
        });
        e.deny("Banned");
        assert!(e.is_cancelled());
        assert_eq!(e.kick_message().unwrap().plain(), "Banned");
    }

    #[test]
    fn erased_handler_downcasts() {
        let hits = Rc::new(RefCell::new(0));
        let h2 = hits.clone();
        let erased = erase(move |e: &mut ServerCommandEvent| {
            *h2.borrow_mut() += 1;
            e.cancel();
        });
        let mut ev = ServerCommandEvent::new(raw::ServerCommand {
            command: "stop".into(),
            cancelled: false,
        });
        (erased.borrow_mut())(&mut ev);
        assert!(ev.is_cancelled());
        // A different event type is ignored.
        let mut other = ChunkUnloadEvent::new(raw::ChunkUnload {
            world: "w".into(),
            x: 0,
            z: 0,
        });
        (erased.borrow_mut())(&mut other);
        assert_eq!(*hits.borrow(), 1);
    }
}
