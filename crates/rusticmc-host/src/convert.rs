//! Conversions between the bindgen-generated WIT types and the core model
//! (`rusticmc_core::model`), both ways, exhaustively.
//!
//! Every record is converted field by field through a struct literal and every enum or variant
//! through an exhaustive `match`, so a WIT change that is not mirrored here (or in the core
//! model) is a compile error, not a silent mismatch. [`Cv`] is a local conversion trait (the
//! orphan rules allow `From` too, but `Option`/`Vec`/tuple lifting needs a trait of our own).

use crate::bindings::exports::rustic::services::permission_provider as pp_export;
use crate::bindings::rustic::api::{
    bossbars as wb, commands as wc, event_types as we, http as wh, menus as wm, scoreboards as ws,
    server as wsv, types as wt,
};
use crate::bindings::rustic::services::{common as wco, permission_provider as pp_import};
use rusticmc_core::model as m;

/// A conversion between a WIT type and its core mirror.
pub(crate) trait Cv<T> {
    /// Convert.
    fn cv(self) -> T;
}

macro_rules! identity {
    ($($t:ty),*) => {$(
        impl Cv<$t> for $t {
            #[inline]
            fn cv(self) -> $t { self }
        }
    )*};
}
identity!((), bool, u8, u16, u32, u64, i32, i64, f32, f64, String);

impl<A: Cv<B>, B> Cv<Option<B>> for Option<A> {
    #[inline]
    fn cv(self) -> Option<B> {
        self.map(Cv::cv)
    }
}

impl<A: Cv<B>, B> Cv<Vec<B>> for Vec<A> {
    #[inline]
    fn cv(self) -> Vec<B> {
        self.into_iter().map(Cv::cv).collect()
    }
}

impl<A: Cv<C>, B: Cv<D>, C, D> Cv<(C, D)> for (A, B) {
    #[inline]
    fn cv(self) -> (C, D) {
        (self.0.cv(), self.1.cv())
    }
}

// `text` is a JSON string on the wire; the core keeps it as `Text` (no re-validation here:
// the SDK builds valid JSON and backends must tolerate anything).
impl Cv<m::Text> for String {
    #[inline]
    fn cv(self) -> m::Text {
        m::Text::from_json_unchecked(self)
    }
}

impl Cv<String> for m::Text {
    #[inline]
    fn cv(self) -> String {
        self.into_json()
    }
}

/// Records with the same field names on both sides.
macro_rules! record {
    ($w:path, $c:path { $($f:ident),* $(,)? }) => {
        impl Cv<$c> for $w {
            #[inline]
            fn cv(self) -> $c {
                type C = $c;
                C { $($f: self.$f.cv()),* }
            }
        }
        impl Cv<$w> for $c {
            #[inline]
            fn cv(self) -> $w {
                type W = $w;
                W { $($f: self.$f.cv()),* }
            }
        }
    };
}

/// Enums with the same case names on both sides.
macro_rules! enums {
    ($w:path, $c:path { $($v:ident),* $(,)? }) => {
        impl Cv<$c> for $w {
            #[inline]
            fn cv(self) -> $c {
                type W = $w;
                type C = $c;
                match self { $(W::$v => C::$v),* }
            }
        }
        impl Cv<$w> for $c {
            #[inline]
            fn cv(self) -> $w {
                type W = $w;
                type C = $c;
                match self { $(C::$v => W::$v),* }
            }
        }
    };
}

// ---------------------------------------------------------------------------- types

record!(wt::Uuid, m::Uuid { hi, lo });
record!(wt::Vec3, m::Vec3 { x, y, z });
record!(wt::BlockPos, m::BlockPos { x, y, z });
record!(
    wt::Location,
    m::Location {
        world,
        x,
        y,
        z,
        yaw,
        pitch
    }
);
enums!(
    wt::GameMode,
    m::GameMode {
        Survival,
        Creative,
        Adventure,
        Spectator
    }
);
enums!(
    wt::Priority,
    m::Priority {
        Lowest,
        Low,
        Normal,
        High,
        Highest,
        Monitor
    }
);
record!(
    wt::ItemStack,
    m::ItemStack {
        id,
        count,
        components
    }
);
record!(wt::BlockState, m::BlockState { id, properties });
enums!(
    wt::LogLevel,
    m::LogLevel {
        Trace,
        Debug,
        Info,
        Warn,
        Error
    }
);
record!(wt::EntityRef, m::EntityRef { id, uuid, kind });
enums!(wt::Hand, m::Hand { MainHand, OffHand });
enums!(
    wt::BlockFace,
    m::BlockFace {
        Down,
        Up,
        North,
        South,
        West,
        East
    }
);
enums!(
    wt::Weather,
    m::Weather {
        Clear,
        Rain,
        Thunder
    }
);
enums!(
    wt::PermissionDefault,
    m::PermissionDefault {
        Allow,
        Deny,
        Op,
        NotOp
    }
);

impl Cv<m::CommandSender> for wt::CommandSender {
    fn cv(self) -> m::CommandSender {
        match self {
            wt::CommandSender::Console => m::CommandSender::Console,
            wt::CommandSender::Player(u) => m::CommandSender::Player(u.cv()),
        }
    }
}

impl Cv<wt::CommandSender> for m::CommandSender {
    fn cv(self) -> wt::CommandSender {
        match self {
            m::CommandSender::Console => wt::CommandSender::Console,
            m::CommandSender::Player(u) => wt::CommandSender::Player(u.cv()),
        }
    }
}

impl Cv<m::HostError> for wt::HostError {
    fn cv(self) -> m::HostError {
        match self {
            wt::HostError::NoSuchPlayer => m::HostError::NoSuchPlayer,
            wt::HostError::NoSuchWorld => m::HostError::NoSuchWorld,
            wt::HostError::CapabilityDenied(s) => m::HostError::CapabilityDenied(s),
            wt::HostError::MainThreadOnly => m::HostError::MainThreadOnly,
            wt::HostError::AsyncOnly => m::HostError::AsyncOnly,
            wt::HostError::Invalid(s) => m::HostError::Invalid(s),
            wt::HostError::Unsupported(s) => m::HostError::Unsupported(s),
            wt::HostError::Other(s) => m::HostError::Other(s),
        }
    }
}

impl Cv<wt::HostError> for m::HostError {
    fn cv(self) -> wt::HostError {
        match self {
            m::HostError::NoSuchPlayer => wt::HostError::NoSuchPlayer,
            m::HostError::NoSuchWorld => wt::HostError::NoSuchWorld,
            m::HostError::CapabilityDenied(s) => wt::HostError::CapabilityDenied(s),
            m::HostError::MainThreadOnly => wt::HostError::MainThreadOnly,
            m::HostError::AsyncOnly => wt::HostError::AsyncOnly,
            m::HostError::Invalid(s) => wt::HostError::Invalid(s),
            m::HostError::Unsupported(s) => wt::HostError::Unsupported(s),
            m::HostError::Other(s) => wt::HostError::Other(s),
        }
    }
}

// ---------------------------------------------------------------------------- commands

enums!(
    wc::ArgKind,
    m::ArgKind {
        Word,
        String,
        Greedy,
        Integer,
        Number,
        Boolean,
        Player,
        Players,
        Position,
        BlockPos,
        Item,
        World,
        Duration,
    }
);
record!(
    wc::ArgSpec,
    m::ArgSpec {
        name,
        kind,
        min,
        max,
        suggestions,
        dynamic_suggestions
    }
);
record!(
    wc::Usage,
    m::Usage {
        nodes,
        permission,
        description
    }
);
record!(
    wc::CommandSpec,
    m::CommandSpec {
        name,
        aliases,
        description,
        permission,
        players_only,
        usages,
    }
);
record!(
    wc::CommandContext,
    m::CommandContext {
        sender,
        label,
        usage,
        args,
        raw
    }
);
record!(
    wc::CompleteContext,
    m::CompleteContext {
        sender,
        raw,
        argument,
        partial
    }
);

impl Cv<m::NodeSpec> for wc::NodeSpec {
    fn cv(self) -> m::NodeSpec {
        match self {
            wc::NodeSpec::Literal(s) => m::NodeSpec::Literal(s),
            wc::NodeSpec::Argument(a) => m::NodeSpec::Argument(a.cv()),
        }
    }
}

impl Cv<wc::NodeSpec> for m::NodeSpec {
    fn cv(self) -> wc::NodeSpec {
        match self {
            m::NodeSpec::Literal(s) => wc::NodeSpec::Literal(s),
            m::NodeSpec::Argument(a) => wc::NodeSpec::Argument(a.cv()),
        }
    }
}

impl Cv<m::ArgValue> for wc::ArgValue {
    fn cv(self) -> m::ArgValue {
        match self {
            wc::ArgValue::Word(s) => m::ArgValue::Word(s),
            wc::ArgValue::String(s) => m::ArgValue::String(s),
            wc::ArgValue::Greedy(s) => m::ArgValue::Greedy(s),
            wc::ArgValue::Integer(v) => m::ArgValue::Integer(v),
            wc::ArgValue::Number(v) => m::ArgValue::Number(v),
            wc::ArgValue::Boolean(v) => m::ArgValue::Boolean(v),
            wc::ArgValue::Player(u) => m::ArgValue::Player(u.cv()),
            wc::ArgValue::Players(u) => m::ArgValue::Players(u.cv()),
            wc::ArgValue::Position(l) => m::ArgValue::Position(l.cv()),
            wc::ArgValue::BlockPos(p) => m::ArgValue::BlockPos(p.cv()),
            wc::ArgValue::Item(i) => m::ArgValue::Item(i.cv()),
            wc::ArgValue::World(s) => m::ArgValue::World(s),
            wc::ArgValue::Duration(v) => m::ArgValue::Duration(v),
        }
    }
}

impl Cv<wc::ArgValue> for m::ArgValue {
    fn cv(self) -> wc::ArgValue {
        match self {
            m::ArgValue::Word(s) => wc::ArgValue::Word(s),
            m::ArgValue::String(s) => wc::ArgValue::String(s),
            m::ArgValue::Greedy(s) => wc::ArgValue::Greedy(s),
            m::ArgValue::Integer(v) => wc::ArgValue::Integer(v),
            m::ArgValue::Number(v) => wc::ArgValue::Number(v),
            m::ArgValue::Boolean(v) => wc::ArgValue::Boolean(v),
            m::ArgValue::Player(u) => wc::ArgValue::Player(u.cv()),
            m::ArgValue::Players(u) => wc::ArgValue::Players(u.cv()),
            m::ArgValue::Position(l) => wc::ArgValue::Position(l.cv()),
            m::ArgValue::BlockPos(p) => wc::ArgValue::BlockPos(p.cv()),
            m::ArgValue::Item(i) => wc::ArgValue::Item(i.cv()),
            m::ArgValue::World(s) => wc::ArgValue::World(s),
            m::ArgValue::Duration(v) => wc::ArgValue::Duration(v),
        }
    }
}

// ---------------------------------------------------------------------------- game

record!(
    wsv::PluginMeta,
    m::PluginMeta {
        id,
        name,
        version,
        enabled
    }
);
record!(wsv::TickStats, m::TickStats { mspt, tps });
record!(wm::MenuSpec, m::MenuSpec { rows, title, items });
enums!(
    ws::DisplaySlot,
    m::DisplaySlot {
        Sidebar,
        PlayerList,
        BelowName
    }
);
enums!(
    ws::TeamColor,
    m::TeamColor {
        Black,
        DarkBlue,
        DarkGreen,
        DarkAqua,
        DarkRed,
        DarkPurple,
        Gold,
        Gray,
        DarkGray,
        Blue,
        Green,
        Aqua,
        Red,
        LightPurple,
        Yellow,
        White,
        Reset,
    }
);
enums!(
    wb::BarColor,
    m::BarColor {
        Pink,
        Blue,
        Red,
        Green,
        Yellow,
        Purple,
        White
    }
);
enums!(
    wb::BarStyle,
    m::BarStyle {
        Progress,
        Notched6,
        Notched10,
        Notched12,
        Notched20
    }
);
record!(
    wh::Request,
    m::HttpRequest {
        method,
        url,
        headers,
        body,
        timeout_ms
    }
);
record!(
    wh::Response,
    m::HttpResponse {
        status,
        headers,
        body
    }
);

// ---------------------------------------------------------------------------- services

record!(wco::Uuid, m::Uuid { hi, lo });

impl Cv<m::ServiceError> for wco::ServiceError {
    fn cv(self) -> m::ServiceError {
        match self {
            wco::ServiceError::NoProvider => m::ServiceError::NoProvider,
            wco::ServiceError::ProviderFailed => m::ServiceError::ProviderFailed,
            wco::ServiceError::Refused(s) => m::ServiceError::Refused(s),
        }
    }
}

impl Cv<wco::ServiceError> for m::ServiceError {
    fn cv(self) -> wco::ServiceError {
        match self {
            m::ServiceError::NoProvider => wco::ServiceError::NoProvider,
            m::ServiceError::ProviderFailed => wco::ServiceError::ProviderFailed,
            m::ServiceError::Refused(s) => wco::ServiceError::Refused(s),
        }
    }
}

enums!(
    pp_import::Tristate,
    m::Tristate {
        Allow,
        Deny,
        Undefined
    }
);
enums!(
    pp_export::Tristate,
    m::Tristate {
        Allow,
        Deny,
        Undefined
    }
);

/// The consumer-side and provider-side `tristate` are distinct generated types.
pub(crate) fn tristate_to_export(t: pp_import::Tristate) -> pp_export::Tristate {
    let c: m::Tristate = t.cv();
    c.cv()
}

/// See [`tristate_to_export`].
pub(crate) fn tristate_from_export(t: pp_export::Tristate) -> pp_import::Tristate {
    let c: m::Tristate = t.cv();
    c.cv()
}

// ---------------------------------------------------------------------------- events

enums!(
    we::InteractAction,
    m::InteractAction {
        LeftClickBlock,
        RightClickBlock,
        LeftClickAir,
        RightClickAir,
    }
);
enums!(
    we::ClickKind,
    m::ClickKind {
        Left,
        Right,
        ShiftLeft,
        ShiftRight,
        NumberKey,
        Drop,
        ControlDrop,
        DoubleClick,
        Middle,
        Other,
    }
);
record!(
    we::PlayerPreLogin,
    m::PlayerPreLogin {
        player,
        name,
        address,
        cancelled,
        kick_message
    }
);
record!(
    we::PlayerJoin,
    m::PlayerJoin {
        player,
        name,
        join_message
    }
);
record!(
    we::PlayerQuit,
    m::PlayerQuit {
        player,
        name,
        quit_message
    }
);
record!(
    we::PlayerChat,
    m::PlayerChat {
        player,
        name,
        message,
        cancelled,
        display
    }
);
record!(
    we::PlayerCommand,
    m::PlayerCommand {
        player,
        name,
        command,
        cancelled
    }
);
record!(we::ServerCommand, m::ServerCommand { command, cancelled });
record!(
    we::PlayerMove,
    m::PlayerMove {
        player,
        from,
        to,
        cancelled
    }
);
record!(we::PlayerTeleport, m::PlayerTeleport { player, from, to });
record!(
    we::PlayerChangeWorld,
    m::PlayerChangeWorld {
        player,
        from_world,
        to_world
    }
);
record!(
    we::BlockBreak,
    m::BlockBreak {
        player,
        world,
        pos,
        block,
        cancelled,
        drop_items
    }
);
record!(
    we::BlockPlace,
    m::BlockPlace {
        player,
        world,
        pos,
        block,
        hand,
        cancelled
    }
);
record!(
    we::PlayerInteract,
    m::PlayerInteract {
        player,
        action,
        hand,
        item,
        world,
        block,
        face,
        cancelled,
    }
);
record!(
    we::PlayerInteractEntity,
    m::PlayerInteractEntity {
        player,
        target,
        hand,
        cancelled
    }
);
record!(
    we::EntityDamage,
    m::EntityDamage {
        target,
        cause,
        attacker,
        amount,
        cancelled
    }
);
record!(
    we::PlayerDeath,
    m::PlayerDeath {
        player,
        cause,
        death_message,
        keep_inventory,
        keep_level,
    }
);
record!(
    we::EntityDeath,
    m::EntityDeath {
        entity,
        killer,
        drops,
        dropped_exp
    }
);
record!(we::PlayerRespawn, m::PlayerRespawn { player, location });
record!(
    we::InventoryClick,
    m::InventoryClick {
        player,
        menu,
        slot,
        click,
        hotbar_button,
        item,
        cursor,
        cancelled,
    }
);
record!(we::InventoryClose, m::InventoryClose { player, menu });
record!(
    we::PlayerGameModeChange,
    m::PlayerGameModeChange {
        player,
        old_mode,
        new_mode,
        cancelled,
    }
);
record!(
    we::ServerListPing,
    m::ServerListPing {
        address,
        motd,
        max_players,
        online_players,
        show_players,
    }
);
record!(
    we::PluginMessage,
    m::PluginMessage {
        player,
        channel,
        data
    }
);
record!(
    we::ChunkLoad,
    m::ChunkLoad {
        world,
        x,
        z,
        new_chunk
    }
);
record!(we::ChunkUnload, m::ChunkUnload { world, x, z });
record!(
    we::CustomEvent,
    m::CustomEvent {
        name,
        source_plugin,
        payload,
        cancelled
    }
);

#[cfg(test)]
mod tests;
