//! Mirrors of `rustic:api/event-types` and the [`Event`] enum.
//!
//! Each record is what a handler receives and returns. The runtime applies only the fields the
//! WIT documents as mutable ("mut") and `cancelled` (on cancellable events) with
//! [`Event::apply_mutable_from`]; everything else a handler returns is ignored.

use super::types::{
    BlockFace, BlockPos, BlockState, EntityRef, GameMode, Hand, ItemStack, Location, Text, Uuid,
};
use serde::{Deserialize, Serialize};

/// Before the player is admitted. Cancellable: refuses them with `kick_message`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PlayerPreLogin {
    /// Player UUID.
    pub player: Uuid,
    /// Player name.
    pub name: String,
    /// Client address.
    pub address: String,
    /// Cancelled.
    pub cancelled: bool,
    /// mut: the kick message.
    pub kick_message: Option<Text>,
}

/// The player joined and is in the world.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PlayerJoin {
    /// Player UUID.
    pub player: Uuid,
    /// Player name.
    pub name: String,
    /// mut: the broadcast join line; `None` hides it.
    pub join_message: Option<Text>,
}

/// The player left.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PlayerQuit {
    /// Player UUID.
    pub player: Uuid,
    /// Player name.
    pub name: String,
    /// mut: the broadcast quit line; `None` hides it.
    pub quit_message: Option<Text>,
}

/// A chat message. Cancellable.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PlayerChat {
    /// Player UUID.
    pub player: Uuid,
    /// Player name.
    pub name: String,
    /// The message as typed (read-only: signed by the client).
    pub message: String,
    /// Cancelled.
    pub cancelled: bool,
    /// mut: what everyone sees instead of the default line.
    pub display: Option<Text>,
}

/// A command a player typed, without slash. Cancellable.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PlayerCommand {
    /// Player UUID.
    pub player: Uuid,
    /// Player name.
    pub name: String,
    /// The command line.
    pub command: String,
    /// Cancelled.
    pub cancelled: bool,
}

/// A console command, without slash. Cancellable.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ServerCommand {
    /// The command line.
    pub command: String,
    /// Cancelled.
    pub cancelled: bool,
}

/// Movement above the server's threshold. Cancellable: the player is put back to `from`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PlayerMove {
    /// Player UUID.
    pub player: Uuid,
    /// Previous location.
    pub from: Location,
    /// New location.
    pub to: Location,
    /// Cancelled.
    pub cancelled: bool,
}

/// A teleport.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PlayerTeleport {
    /// Player UUID.
    pub player: Uuid,
    /// Previous location.
    pub from: Location,
    /// Destination.
    pub to: Location,
}

/// A change of dimension.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PlayerChangeWorld {
    /// Player UUID.
    pub player: Uuid,
    /// Previous dimension.
    pub from_world: String,
    /// New dimension.
    pub to_world: String,
}

/// A player broke a block. Cancellable: the block stays.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct BlockBreak {
    /// Player UUID.
    pub player: Uuid,
    /// Dimension.
    pub world: String,
    /// Position.
    pub pos: BlockPos,
    /// The block broken.
    pub block: BlockState,
    /// Cancelled.
    pub cancelled: bool,
    /// mut: false drops nothing.
    pub drop_items: bool,
}

/// A player placed a block. Cancellable.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct BlockPlace {
    /// Player UUID.
    pub player: Uuid,
    /// Dimension.
    pub world: String,
    /// Position.
    pub pos: BlockPos,
    /// The block placed.
    pub block: BlockState,
    /// Hand used.
    pub hand: Hand,
    /// Cancelled.
    pub cancelled: bool,
}

/// WIT `interact-action`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InteractAction {
    /// Left click on a block.
    LeftClickBlock,
    /// Right click on a block.
    RightClickBlock,
    /// Left click in the air.
    LeftClickAir,
    /// Right click in the air.
    RightClickAir,
}

/// A player clicked a block or the air. Cancellable.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PlayerInteract {
    /// Player UUID.
    pub player: Uuid,
    /// What kind of click.
    pub action: InteractAction,
    /// Hand.
    pub hand: Hand,
    /// Item in that hand.
    pub item: ItemStack,
    /// Dimension.
    pub world: String,
    /// Clicked block.
    pub block: Option<BlockPos>,
    /// Clicked face.
    pub face: Option<BlockFace>,
    /// Cancelled.
    pub cancelled: bool,
}

/// A player right-clicked an entity. Cancellable.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PlayerInteractEntity {
    /// Player UUID.
    pub player: Uuid,
    /// The entity.
    pub target: EntityRef,
    /// Hand.
    pub hand: Hand,
    /// Cancelled.
    pub cancelled: bool,
}

/// A living entity is about to take damage. Cancellable.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct EntityDamage {
    /// Victim.
    pub target: EntityRef,
    /// Damage type id.
    pub cause: String,
    /// Who dealt it.
    pub attacker: Option<EntityRef>,
    /// mut: the amount (non-finite or negative values are ignored).
    pub amount: f32,
    /// Cancelled.
    pub cancelled: bool,
}

/// A player died.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PlayerDeath {
    /// Player UUID.
    pub player: Uuid,
    /// Damage type id.
    pub cause: String,
    /// mut: the broadcast death line; `None` hides it.
    pub death_message: Option<Text>,
    /// mut.
    pub keep_inventory: bool,
    /// mut.
    pub keep_level: bool,
}

/// A non-player living entity died.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct EntityDeath {
    /// The entity.
    pub entity: EntityRef,
    /// The killer, if a player.
    pub killer: Option<Uuid>,
    /// mut: what it drops.
    pub drops: Vec<ItemStack>,
    /// mut.
    pub dropped_exp: u32,
}

/// A player respawns.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PlayerRespawn {
    /// Player UUID.
    pub player: Uuid,
    /// mut: where (non-finite coordinates are ignored).
    pub location: Location,
}

/// WIT `click-kind`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ClickKind {
    /// Left click.
    Left,
    /// Right click.
    Right,
    /// Shift + left.
    ShiftLeft,
    /// Shift + right.
    ShiftRight,
    /// Hotbar number key.
    NumberKey,
    /// Drop key.
    Drop,
    /// Control + drop.
    ControlDrop,
    /// Double click.
    DoubleClick,
    /// Middle click.
    Middle,
    /// Anything else.
    Other,
}

/// A click in an open container. Cancellable; plugin menus cancel by default.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct InventoryClick {
    /// Player UUID.
    pub player: Uuid,
    /// The plugin menu, if one.
    pub menu: Option<u32>,
    /// Slot.
    pub slot: i32,
    /// Kind of click.
    pub click: ClickKind,
    /// Hotbar number for `NumberKey`.
    pub hotbar_button: i32,
    /// Item in the slot.
    pub item: ItemStack,
    /// Item on the cursor.
    pub cursor: ItemStack,
    /// Cancelled.
    pub cancelled: bool,
}

/// A container was closed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct InventoryClose {
    /// Player UUID.
    pub player: Uuid,
    /// The plugin menu, if one.
    pub menu: Option<u32>,
}

/// A game mode change. Cancellable.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PlayerGameModeChange {
    /// Player UUID.
    pub player: Uuid,
    /// Before.
    pub old_mode: GameMode,
    /// After.
    pub new_mode: GameMode,
    /// Cancelled.
    pub cancelled: bool,
}

/// The server-list ping.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ServerListPing {
    /// Client address.
    pub address: String,
    /// mut.
    pub motd: Text,
    /// mut.
    pub max_players: u32,
    /// Players online.
    pub online_players: u32,
    /// mut: false hides the player sample.
    pub show_players: bool,
}

/// A custom payload from a client, on a channel the plugin registered.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PluginMessage {
    /// Player UUID.
    pub player: Uuid,
    /// Channel ("namespace:path").
    pub channel: String,
    /// Payload.
    pub data: Vec<u8>,
}

/// A chunk was loaded.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ChunkLoad {
    /// Dimension.
    pub world: String,
    /// Chunk X.
    pub x: i32,
    /// Chunk Z.
    pub z: i32,
    /// Freshly generated.
    pub new_chunk: bool,
}

/// A chunk was unloaded.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ChunkUnload {
    /// Dimension.
    pub world: String,
    /// Chunk X.
    pub x: i32,
    /// Chunk Z.
    pub z: i32,
}

/// An event a plugin fired with `events.call-custom`. Cancellable.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CustomEvent {
    /// Event name ("<plugin-id>:<name>").
    pub name: String,
    /// Who fired it.
    pub source_plugin: String,
    /// Opaque payload.
    pub payload: Vec<u8>,
    /// Cancelled.
    pub cancelled: bool,
}

/// Implemented by every event record.
pub trait EventRecord: Sized + Into<Event> {
    /// The kind.
    const KIND: EventKind;
    /// Take the record back out of an [`Event`] (gives the event back if it is another kind).
    #[allow(clippy::result_large_err)]
    fn from_event(event: Event) -> Result<Self, Event>;
}

macro_rules! events {
    ($( $variant:ident = $name:literal ),* $(,)?) => {
        /// One variant per `event-types` record. Serialised with the kebab-case record name as
        /// tag (`{"player-join": {...}}`).
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "kebab-case")]
        pub enum Event {
            $(
                #[doc = concat!("`", $name, "`")]
                $variant($variant),
            )*
        }

        /// The kind of an [`Event`], usable as a dense index.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
        #[repr(u8)]
        pub enum EventKind {
            $(
                #[doc = concat!("`", $name, "`")]
                $variant,
            )*
        }

        impl EventKind {
            /// Every kind, in WIT order.
            pub const ALL: &'static [EventKind] = &[$(EventKind::$variant),*];
            /// Number of kinds.
            pub const COUNT: usize = Self::ALL.len();

            /// The kebab-case WIT record name ("player-join").
            pub const fn name(self) -> &'static str {
                match self {
                    $(EventKind::$variant => $name,)*
                }
            }

            /// Look a kind up by its WIT name.
            pub fn from_name(name: &str) -> Option<Self> {
                match name {
                    $($name => Some(EventKind::$variant),)*
                    _ => None,
                }
            }

            /// Dense index in `0..COUNT`.
            #[inline]
            pub const fn index(self) -> usize {
                self as usize
            }
        }

        impl Event {
            /// The kind.
            #[inline]
            pub fn kind(&self) -> EventKind {
                match self {
                    $(Event::$variant(_) => EventKind::$variant,)*
                }
            }
        }

        $(
            impl From<$variant> for Event {
                fn from(e: $variant) -> Self {
                    Event::$variant(e)
                }
            }

            impl EventRecord for $variant {
                const KIND: EventKind = EventKind::$variant;
                #[allow(clippy::result_large_err)]
                fn from_event(event: Event) -> Result<Self, Event> {
                    match event {
                        Event::$variant(e) => Ok(e),
                        other => Err(other),
                    }
                }
            }
        )*
    };
}

events! {
    PlayerPreLogin = "player-pre-login",
    PlayerJoin = "player-join",
    PlayerQuit = "player-quit",
    PlayerChat = "player-chat",
    PlayerCommand = "player-command",
    ServerCommand = "server-command",
    PlayerMove = "player-move",
    PlayerTeleport = "player-teleport",
    PlayerChangeWorld = "player-change-world",
    BlockBreak = "block-break",
    BlockPlace = "block-place",
    PlayerInteract = "player-interact",
    PlayerInteractEntity = "player-interact-entity",
    EntityDamage = "entity-damage",
    PlayerDeath = "player-death",
    EntityDeath = "entity-death",
    PlayerRespawn = "player-respawn",
    InventoryClick = "inventory-click",
    InventoryClose = "inventory-close",
    PlayerGameModeChange = "player-game-mode-change",
    ServerListPing = "server-list-ping",
    PluginMessage = "plugin-message",
    ChunkLoad = "chunk-load",
    ChunkUnload = "chunk-unload",
    CustomEvent = "custom-event",
}

impl EventKind {
    /// Whether the record has a `cancelled` field.
    pub const fn is_cancellable(self) -> bool {
        matches!(
            self,
            EventKind::PlayerPreLogin
                | EventKind::PlayerChat
                | EventKind::PlayerCommand
                | EventKind::ServerCommand
                | EventKind::PlayerMove
                | EventKind::BlockBreak
                | EventKind::BlockPlace
                | EventKind::PlayerInteract
                | EventKind::PlayerInteractEntity
                | EventKind::EntityDamage
                | EventKind::InventoryClick
                | EventKind::PlayerGameModeChange
                | EventKind::CustomEvent
        )
    }
}

/// Prefix of custom event subscriptions ("custom:<name>").
pub const CUSTOM_PREFIX: &str = "custom:";

/// What a subscription names: a built-in event or a custom event.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum EventKey {
    /// A built-in event.
    Builtin(EventKind),
    /// `custom:<name>`: the name after the prefix.
    Custom(String),
}

impl EventKey {
    /// Parse and validate a subscription name: a known kebab-case event name or
    /// `custom:<name>` (name 1-128 chars of `[a-z0-9_.:/-]`).
    pub fn parse(name: &str) -> Result<Self, String> {
        if let Some(kind) = EventKind::from_name(name) {
            return Ok(EventKey::Builtin(kind));
        }
        if let Some(rest) = name.strip_prefix(CUSTOM_PREFIX) {
            validate_custom_name(rest)?;
            return Ok(EventKey::Custom(rest.to_owned()));
        }
        Err(format!(
            "unknown event `{name}`: expected one of the event-types record names \
             (\"player-join\", \"block-break\"...) or \"custom:<name>\""
        ))
    }

    /// The subscription name.
    pub fn name(&self) -> String {
        match self {
            EventKey::Builtin(k) => k.name().to_owned(),
            EventKey::Custom(n) => format!("{CUSTOM_PREFIX}{n}"),
        }
    }
}

/// Validate a custom event name (without the `custom:` prefix).
pub fn validate_custom_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.len() > 128 {
        return Err("custom event names must be 1 to 128 characters".into());
    }
    if !name
        .bytes()
        .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b':' | b'/' | b'-'))
    {
        return Err(format!(
            "custom event name `{name}` must use lowercase [a-z0-9_.:/-]"
        ));
    }
    Ok(())
}

impl Event {
    /// The kebab-case WIT name ("player-join").
    pub fn name(&self) -> &'static str {
        self.kind().name()
    }

    /// The bus key: the built-in kind, or `Custom(name)` for a custom event.
    pub fn key(&self) -> EventKey {
        match self {
            Event::CustomEvent(c) => EventKey::Custom(c.name.clone()),
            other => EventKey::Builtin(other.kind()),
        }
    }

    /// Whether the record has a `cancelled` field.
    pub fn is_cancellable(&self) -> bool {
        self.kind().is_cancellable()
    }

    /// The `cancelled` field (false for events that cannot be cancelled).
    pub fn cancelled(&self) -> bool {
        match self {
            Event::PlayerPreLogin(e) => e.cancelled,
            Event::PlayerChat(e) => e.cancelled,
            Event::PlayerCommand(e) => e.cancelled,
            Event::ServerCommand(e) => e.cancelled,
            Event::PlayerMove(e) => e.cancelled,
            Event::BlockBreak(e) => e.cancelled,
            Event::BlockPlace(e) => e.cancelled,
            Event::PlayerInteract(e) => e.cancelled,
            Event::PlayerInteractEntity(e) => e.cancelled,
            Event::EntityDamage(e) => e.cancelled,
            Event::InventoryClick(e) => e.cancelled,
            Event::PlayerGameModeChange(e) => e.cancelled,
            Event::CustomEvent(e) => e.cancelled,
            _ => false,
        }
    }

    /// Set `cancelled`; returns false (and does nothing) if the event cannot be cancelled.
    pub fn set_cancelled(&mut self, cancelled: bool) -> bool {
        let slot = match self {
            Event::PlayerPreLogin(e) => &mut e.cancelled,
            Event::PlayerChat(e) => &mut e.cancelled,
            Event::PlayerCommand(e) => &mut e.cancelled,
            Event::ServerCommand(e) => &mut e.cancelled,
            Event::PlayerMove(e) => &mut e.cancelled,
            Event::BlockBreak(e) => &mut e.cancelled,
            Event::BlockPlace(e) => &mut e.cancelled,
            Event::PlayerInteract(e) => &mut e.cancelled,
            Event::PlayerInteractEntity(e) => &mut e.cancelled,
            Event::EntityDamage(e) => &mut e.cancelled,
            Event::InventoryClick(e) => &mut e.cancelled,
            Event::PlayerGameModeChange(e) => &mut e.cancelled,
            Event::CustomEvent(e) => &mut e.cancelled,
            _ => return false,
        };
        *slot = cancelled;
        true
    }

    /// Copy from `returned` only the fields the WIT documents as mutable, plus `cancelled` on
    /// cancellable events. Read-only fields of `self` are never touched. Values that would be
    /// invalid (a non-finite or negative damage amount, a non-finite respawn location) are
    /// ignored. Returns false (and changes nothing) when `returned` is another kind of event
    /// or a custom event of another name.
    pub fn apply_mutable_from(&mut self, returned: &Event) -> bool {
        match (self, returned) {
            (Event::PlayerPreLogin(a), Event::PlayerPreLogin(b)) => {
                a.cancelled = b.cancelled;
                a.kick_message = b.kick_message.clone();
            }
            (Event::PlayerJoin(a), Event::PlayerJoin(b)) => {
                a.join_message = b.join_message.clone();
            }
            (Event::PlayerQuit(a), Event::PlayerQuit(b)) => {
                a.quit_message = b.quit_message.clone();
            }
            (Event::PlayerChat(a), Event::PlayerChat(b)) => {
                a.cancelled = b.cancelled;
                a.display = b.display.clone();
            }
            (Event::PlayerCommand(a), Event::PlayerCommand(b)) => a.cancelled = b.cancelled,
            (Event::ServerCommand(a), Event::ServerCommand(b)) => a.cancelled = b.cancelled,
            (Event::PlayerMove(a), Event::PlayerMove(b)) => a.cancelled = b.cancelled,
            (Event::PlayerTeleport(_), Event::PlayerTeleport(_)) => {}
            (Event::PlayerChangeWorld(_), Event::PlayerChangeWorld(_)) => {}
            (Event::BlockBreak(a), Event::BlockBreak(b)) => {
                a.cancelled = b.cancelled;
                a.drop_items = b.drop_items;
            }
            (Event::BlockPlace(a), Event::BlockPlace(b)) => a.cancelled = b.cancelled,
            (Event::PlayerInteract(a), Event::PlayerInteract(b)) => a.cancelled = b.cancelled,
            (Event::PlayerInteractEntity(a), Event::PlayerInteractEntity(b)) => {
                a.cancelled = b.cancelled
            }
            (Event::EntityDamage(a), Event::EntityDamage(b)) => {
                a.cancelled = b.cancelled;
                if b.amount.is_finite() && b.amount >= 0.0 {
                    a.amount = b.amount;
                } else {
                    log::warn!(target: "rustic", "ignored invalid entity-damage amount {}", b.amount);
                }
            }
            (Event::PlayerDeath(a), Event::PlayerDeath(b)) => {
                a.death_message = b.death_message.clone();
                a.keep_inventory = b.keep_inventory;
                a.keep_level = b.keep_level;
            }
            (Event::EntityDeath(a), Event::EntityDeath(b)) => {
                a.drops = b.drops.clone();
                a.dropped_exp = b.dropped_exp;
            }
            (Event::PlayerRespawn(a), Event::PlayerRespawn(b)) => {
                if b.location.is_finite() && !b.location.world.is_empty() {
                    a.location = b.location.clone();
                } else {
                    log::warn!(target: "rustic", "ignored invalid player-respawn location");
                }
            }
            (Event::InventoryClick(a), Event::InventoryClick(b)) => a.cancelled = b.cancelled,
            (Event::InventoryClose(_), Event::InventoryClose(_)) => {}
            (Event::PlayerGameModeChange(a), Event::PlayerGameModeChange(b)) => {
                a.cancelled = b.cancelled
            }
            (Event::ServerListPing(a), Event::ServerListPing(b)) => {
                a.motd = b.motd.clone();
                a.max_players = b.max_players;
                a.show_players = b.show_players;
            }
            (Event::PluginMessage(_), Event::PluginMessage(_)) => {}
            (Event::ChunkLoad(_), Event::ChunkLoad(_)) => {}
            (Event::ChunkUnload(_), Event::ChunkUnload(_)) => {}
            (Event::CustomEvent(a), Event::CustomEvent(b)) => {
                if a.name != b.name {
                    return false;
                }
                a.cancelled = b.cancelled;
            }
            _ => return false,
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(n: u64) -> Uuid {
        Uuid::new(n, n)
    }

    fn loc(w: &str, x: f64) -> Location {
        Location {
            world: w.into(),
            x,
            y: 64.0,
            z: 0.0,
            yaw: 0.0,
            pitch: 0.0,
        }
    }

    fn entity(id: u32) -> EntityRef {
        EntityRef {
            id,
            uuid: u(id as u64),
            kind: "minecraft:zombie".into(),
        }
    }

    fn item(id: &str) -> ItemStack {
        ItemStack::new(id, 1)
    }

    /// Apply `returned` onto `original` and check that the result equals `expected`.
    fn check(original: Event, returned: Event, expected: Event) {
        let mut e = original;
        assert!(e.apply_mutable_from(&returned));
        assert_eq!(e, expected);
    }

    #[test]
    fn names_and_lookup() {
        assert_eq!(EventKind::COUNT, 25);
        for k in EventKind::ALL {
            assert_eq!(EventKind::from_name(k.name()), Some(*k));
        }
        assert_eq!(
            EventKind::PlayerGameModeChange.name(),
            "player-game-mode-change"
        );
        assert!(EventKind::from_name("player_join").is_none());
        assert_eq!(
            EventKey::parse("block-break"),
            Ok(EventKey::Builtin(EventKind::BlockBreak))
        );
        assert_eq!(
            EventKey::parse("custom:homes:teleport"),
            Ok(EventKey::Custom("homes:teleport".into()))
        );
        assert!(EventKey::parse("custom:").is_err());
        assert!(EventKey::parse("custom:Bad Name").is_err());
        assert!(EventKey::parse("nope").is_err());
        assert_eq!(EventKey::Custom("a:b".into()).name(), "custom:a:b");
    }

    #[test]
    fn cancellation() {
        let mut e = Event::PlayerJoin(PlayerJoin {
            player: u(1),
            name: "a".into(),
            join_message: None,
        });
        assert!(!e.is_cancellable());
        assert!(!e.set_cancelled(true));
        assert!(!e.cancelled());
        let mut c = Event::ServerCommand(ServerCommand {
            command: "stop".into(),
            cancelled: false,
        });
        assert!(c.is_cancellable());
        assert!(c.set_cancelled(true));
        assert!(c.cancelled());
        let cancellable: Vec<_> = EventKind::ALL
            .iter()
            .filter(|k| k.is_cancellable())
            .collect();
        assert_eq!(cancellable.len(), 13);
    }

    #[test]
    fn wrong_kind_is_ignored() {
        let mut e = Event::ServerCommand(ServerCommand {
            command: "x".into(),
            cancelled: false,
        });
        let other = Event::PlayerCommand(PlayerCommand {
            player: u(1),
            name: "n".into(),
            command: "y".into(),
            cancelled: true,
        });
        assert!(!e.apply_mutable_from(&other));
        assert!(!e.cancelled());
        let mut c1 = Event::CustomEvent(CustomEvent {
            name: "a:b".into(),
            source_plugin: "a".into(),
            payload: vec![],
            cancelled: false,
        });
        let c2 = Event::CustomEvent(CustomEvent {
            name: "a:c".into(),
            source_plugin: "a".into(),
            payload: vec![],
            cancelled: true,
        });
        assert!(!c1.apply_mutable_from(&c2));
        assert!(!c1.cancelled());
    }

    #[test]
    fn player_pre_login() {
        let o = PlayerPreLogin {
            player: u(1),
            name: "a".into(),
            address: "1.2.3.4".into(),
            cancelled: false,
            kick_message: None,
        };
        let r = PlayerPreLogin {
            player: u(2),
            name: "b".into(),
            address: "5.6.7.8".into(),
            cancelled: true,
            kick_message: Some(Text::plain("go")),
        };
        let x = PlayerPreLogin {
            cancelled: true,
            kick_message: Some(Text::plain("go")),
            ..o.clone()
        };
        check(o.into(), r.into(), x.into());
    }

    #[test]
    fn player_join_quit() {
        let o = PlayerJoin {
            player: u(1),
            name: "a".into(),
            join_message: Some(Text::plain("hi")),
        };
        let r = PlayerJoin {
            player: u(2),
            name: "evil".into(),
            join_message: None,
        };
        let x = PlayerJoin {
            join_message: None,
            ..o.clone()
        };
        check(o.into(), r.into(), x.into());
        let o = PlayerQuit {
            player: u(1),
            name: "a".into(),
            quit_message: None,
        };
        let r = PlayerQuit {
            player: u(2),
            name: "evil".into(),
            quit_message: Some(Text::plain("bye")),
        };
        let x = PlayerQuit {
            quit_message: Some(Text::plain("bye")),
            ..o.clone()
        };
        check(o.into(), r.into(), x.into());
    }

    #[test]
    fn player_chat() {
        let o = PlayerChat {
            player: u(1),
            name: "a".into(),
            message: "signed".into(),
            cancelled: false,
            display: None,
        };
        let r = PlayerChat {
            player: u(2),
            name: "b".into(),
            message: "forged".into(),
            cancelled: true,
            display: Some(Text::plain("<a> hi")),
        };
        let x = PlayerChat {
            cancelled: true,
            display: Some(Text::plain("<a> hi")),
            ..o.clone()
        };
        check(o.into(), r.into(), x.into());
    }

    #[test]
    fn commands() {
        let o = PlayerCommand {
            player: u(1),
            name: "a".into(),
            command: "home".into(),
            cancelled: false,
        };
        let r = PlayerCommand {
            player: u(2),
            name: "b".into(),
            command: "op b".into(),
            cancelled: true,
        };
        let x = PlayerCommand {
            cancelled: true,
            ..o.clone()
        };
        check(o.into(), r.into(), x.into());
        let o = ServerCommand {
            command: "list".into(),
            cancelled: false,
        };
        let r = ServerCommand {
            command: "stop".into(),
            cancelled: true,
        };
        let x = ServerCommand {
            cancelled: true,
            ..o.clone()
        };
        check(o.into(), r.into(), x.into());
    }

    #[test]
    fn movement() {
        let o = PlayerMove {
            player: u(1),
            from: loc("w", 0.0),
            to: loc("w", 1.0),
            cancelled: false,
        };
        let r = PlayerMove {
            player: u(2),
            from: loc("x", 5.0),
            to: loc("x", 100.0),
            cancelled: true,
        };
        let x = PlayerMove {
            cancelled: true,
            ..o.clone()
        };
        check(o.into(), r.into(), x.into());
        let o = PlayerTeleport {
            player: u(1),
            from: loc("w", 0.0),
            to: loc("w", 1.0),
        };
        let r = PlayerTeleport {
            player: u(2),
            from: loc("x", 0.0),
            to: loc("x", 9.0),
        };
        check(o.clone().into(), r.into(), o.into());
        let o = PlayerChangeWorld {
            player: u(1),
            from_world: "a".into(),
            to_world: "b".into(),
        };
        let r = PlayerChangeWorld {
            player: u(2),
            from_world: "c".into(),
            to_world: "d".into(),
        };
        check(o.clone().into(), r.into(), o.into());
    }

    #[test]
    fn blocks() {
        let state = BlockState {
            id: "minecraft:stone".into(),
            properties: vec![],
        };
        let o = BlockBreak {
            player: u(1),
            world: "w".into(),
            pos: BlockPos { x: 1, y: 2, z: 3 },
            block: state.clone(),
            cancelled: false,
            drop_items: true,
        };
        let r = BlockBreak {
            player: u(2),
            world: "x".into(),
            pos: BlockPos { x: 9, y: 9, z: 9 },
            block: BlockState {
                id: "minecraft:diamond_block".into(),
                properties: vec![],
            },
            cancelled: true,
            drop_items: false,
        };
        let x = BlockBreak {
            cancelled: true,
            drop_items: false,
            ..o.clone()
        };
        check(o.into(), r.into(), x.into());
        let o = BlockPlace {
            player: u(1),
            world: "w".into(),
            pos: BlockPos { x: 1, y: 2, z: 3 },
            block: state.clone(),
            hand: Hand::MainHand,
            cancelled: false,
        };
        let r = BlockPlace {
            player: u(2),
            world: "x".into(),
            pos: BlockPos { x: 0, y: 0, z: 0 },
            block: BlockState::default(),
            hand: Hand::OffHand,
            cancelled: true,
        };
        let x = BlockPlace {
            cancelled: true,
            ..o.clone()
        };
        check(o.into(), r.into(), x.into());
    }

    #[test]
    fn interactions() {
        let o = PlayerInteract {
            player: u(1),
            action: InteractAction::RightClickBlock,
            hand: Hand::MainHand,
            item: item("minecraft:stick"),
            world: "w".into(),
            block: Some(BlockPos { x: 1, y: 1, z: 1 }),
            face: Some(BlockFace::Up),
            cancelled: false,
        };
        let r = PlayerInteract {
            player: u(2),
            action: InteractAction::LeftClickAir,
            hand: Hand::OffHand,
            item: item("minecraft:tnt"),
            world: "x".into(),
            block: None,
            face: None,
            cancelled: true,
        };
        let x = PlayerInteract {
            cancelled: true,
            ..o.clone()
        };
        check(o.into(), r.into(), x.into());
        let o = PlayerInteractEntity {
            player: u(1),
            target: entity(1),
            hand: Hand::MainHand,
            cancelled: false,
        };
        let r = PlayerInteractEntity {
            player: u(2),
            target: entity(2),
            hand: Hand::OffHand,
            cancelled: true,
        };
        let x = PlayerInteractEntity {
            cancelled: true,
            ..o.clone()
        };
        check(o.into(), r.into(), x.into());
    }

    #[test]
    fn entity_damage() {
        let o = EntityDamage {
            target: entity(1),
            cause: "minecraft:fall".into(),
            attacker: None,
            amount: 4.0,
            cancelled: false,
        };
        let r = EntityDamage {
            target: entity(2),
            cause: "minecraft:magic".into(),
            attacker: Some(entity(3)),
            amount: 1.5,
            cancelled: true,
        };
        let x = EntityDamage {
            amount: 1.5,
            cancelled: true,
            ..o.clone()
        };
        check(o.clone().into(), r.clone().into(), x.into());
        // Invalid amounts are ignored, cancelled still applies.
        for bad in [f32::NAN, f32::INFINITY, -1.0] {
            let r = EntityDamage {
                amount: bad,
                ..r.clone()
            };
            let x = EntityDamage {
                cancelled: true,
                ..o.clone()
            };
            check(o.clone().into(), r.into(), x.into());
        }
    }

    #[test]
    fn deaths_and_respawn() {
        let o = PlayerDeath {
            player: u(1),
            cause: "minecraft:lava".into(),
            death_message: Some(Text::plain("a burned")),
            keep_inventory: false,
            keep_level: false,
        };
        let r = PlayerDeath {
            player: u(2),
            cause: "x".into(),
            death_message: None,
            keep_inventory: true,
            keep_level: true,
        };
        let x = PlayerDeath {
            death_message: None,
            keep_inventory: true,
            keep_level: true,
            ..o.clone()
        };
        check(o.into(), r.into(), x.into());
        let o = EntityDeath {
            entity: entity(1),
            killer: Some(u(1)),
            drops: vec![item("minecraft:rotten_flesh")],
            dropped_exp: 5,
        };
        let r = EntityDeath {
            entity: entity(9),
            killer: None,
            drops: vec![],
            dropped_exp: 50,
        };
        let x = EntityDeath {
            drops: vec![],
            dropped_exp: 50,
            ..o.clone()
        };
        check(o.into(), r.into(), x.into());
        let o = PlayerRespawn {
            player: u(1),
            location: loc("w", 0.0),
        };
        let r = PlayerRespawn {
            player: u(2),
            location: loc("w", 10.0),
        };
        let x = PlayerRespawn {
            location: loc("w", 10.0),
            ..o.clone()
        };
        check(o.clone().into(), r.into(), x.into());
        let bad = PlayerRespawn {
            player: u(1),
            location: loc("w", f64::NAN),
        };
        check(o.clone().into(), bad.into(), o.into());
    }

    #[test]
    fn inventories() {
        let o = InventoryClick {
            player: u(1),
            menu: Some(3),
            slot: 4,
            click: ClickKind::Left,
            hotbar_button: -1,
            item: item("minecraft:apple"),
            cursor: ItemStack::empty(),
            cancelled: true,
        };
        let r = InventoryClick {
            player: u(2),
            menu: None,
            slot: 9,
            click: ClickKind::Drop,
            hotbar_button: 2,
            item: item("minecraft:diamond"),
            cursor: item("minecraft:diamond"),
            cancelled: false,
        };
        let x = InventoryClick {
            cancelled: false,
            ..o.clone()
        };
        check(o.into(), r.into(), x.into());
        let o = InventoryClose {
            player: u(1),
            menu: Some(1),
        };
        let r = InventoryClose {
            player: u(2),
            menu: None,
        };
        check(o.clone().into(), r.into(), o.into());
        let o = PlayerGameModeChange {
            player: u(1),
            old_mode: GameMode::Survival,
            new_mode: GameMode::Creative,
            cancelled: false,
        };
        let r = PlayerGameModeChange {
            player: u(2),
            old_mode: GameMode::Spectator,
            new_mode: GameMode::Adventure,
            cancelled: true,
        };
        let x = PlayerGameModeChange {
            cancelled: true,
            ..o.clone()
        };
        check(o.into(), r.into(), x.into());
    }

    #[test]
    fn server_list_ping() {
        let o = ServerListPing {
            address: "1.1.1.1".into(),
            motd: Text::plain("A server"),
            max_players: 20,
            online_players: 3,
            show_players: true,
        };
        let r = ServerListPing {
            address: "6.6.6.6".into(),
            motd: Text::plain("Hacked"),
            max_players: 100,
            online_players: 999,
            show_players: false,
        };
        let x = ServerListPing {
            motd: Text::plain("Hacked"),
            max_players: 100,
            show_players: false,
            ..o.clone()
        };
        check(o.into(), r.into(), x.into());
    }

    #[test]
    fn read_only_events() {
        let o = PluginMessage {
            player: u(1),
            channel: "a:b".into(),
            data: vec![1, 2],
        };
        let r = PluginMessage {
            player: u(2),
            channel: "c:d".into(),
            data: vec![9],
        };
        check(o.clone().into(), r.into(), o.into());
        let o = ChunkLoad {
            world: "w".into(),
            x: 1,
            z: 2,
            new_chunk: true,
        };
        let r = ChunkLoad {
            world: "x".into(),
            x: 5,
            z: 5,
            new_chunk: false,
        };
        check(o.clone().into(), r.into(), o.into());
        let o = ChunkUnload {
            world: "w".into(),
            x: 1,
            z: 2,
        };
        let r = ChunkUnload {
            world: "x".into(),
            x: 7,
            z: 7,
        };
        check(o.clone().into(), r.into(), o.into());
    }

    #[test]
    fn custom_event() {
        let o = CustomEvent {
            name: "homes:tp".into(),
            source_plugin: "homes".into(),
            payload: vec![1],
            cancelled: false,
        };
        let r = CustomEvent {
            name: "homes:tp".into(),
            source_plugin: "evil".into(),
            payload: vec![6, 6, 6],
            cancelled: true,
        };
        let x = CustomEvent {
            cancelled: true,
            ..o.clone()
        };
        check(o.into(), r.into(), x.into());
    }

    #[test]
    fn serde_tags_are_kebab_case() {
        let e = Event::PlayerJoin(PlayerJoin {
            player: u(1),
            name: "a".into(),
            join_message: None,
        });
        let json = serde_json::to_string(&e).unwrap();
        assert!(json.starts_with("{\"player-join\""), "{json}");
        assert!(json.contains("join-message"));
        let back: Event = serde_json::from_str(&json).unwrap();
        assert_eq!(back, e);
        assert_eq!(PlayerJoin::from_event(back).unwrap().name, "a");
    }
}
