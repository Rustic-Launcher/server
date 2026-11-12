//! Mirrors of the records, enums and variants of `commands`, `server`, `menus`, `scoreboards`,
//! `bossbars`, `http` (as [`HttpRequest`] / [`HttpResponse`]) and `rustic:services/common`.

use super::types::{BlockPos, CommandSender, ItemStack, Location, Text, Uuid};
use serde::{Deserialize, Serialize};

/// WIT `commands.command-id`.
pub type CommandId = u32;
/// WIT `scheduler.task-id`.
pub type TaskId = u32;
/// WIT `events.handler-id`.
pub type HandlerId = u32;
/// WIT `menus.menu-id`.
pub type MenuId = u32;
/// WIT `bossbars.bar-id`.
pub type BarId = u32;

/// The kind of a command argument (WIT `arg-kind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArgKind {
    /// One word (`[A-Za-z0-9_.+-]`).
    Word,
    /// A quoted or unquoted string.
    String,
    /// The rest of the line; must be last.
    Greedy,
    /// A 64-bit integer.
    Integer,
    /// A finite floating-point number.
    Number,
    /// `true` / `false`.
    Boolean,
    /// One online player (name, UUID or selector resolving to exactly one).
    Player,
    /// One or more online players.
    Players,
    /// `x y z`, `~` relative to a player sender.
    Position,
    /// Integer `x y z`, `~` relative to a player sender.
    BlockPos,
    /// `namespace:id` with an optional count.
    Item,
    /// A world (dimension) id.
    World,
    /// A duration (`10s`, `5m`, `2h`, `20t`, `1d`), in ticks.
    Duration,
}

impl ArgKind {
    /// The kebab-case WIT name.
    pub fn name(self) -> &'static str {
        match self {
            ArgKind::Word => "word",
            ArgKind::String => "string",
            ArgKind::Greedy => "greedy",
            ArgKind::Integer => "integer",
            ArgKind::Number => "number",
            ArgKind::Boolean => "boolean",
            ArgKind::Player => "player",
            ArgKind::Players => "players",
            ArgKind::Position => "position",
            ArgKind::BlockPos => "block-pos",
            ArgKind::Item => "item",
            ArgKind::World => "world",
            ArgKind::Duration => "duration",
        }
    }
}

/// An argument of a usage (WIT `arg-spec`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct ArgSpec {
    /// Name (shown in usage messages, key in [`CommandContext::args`]).
    pub name: String,
    /// Kind.
    pub kind: ArgKind,
    /// Inclusive lower bound (integer and number).
    pub min: Option<f64>,
    /// Inclusive upper bound (integer and number).
    pub max: Option<f64>,
    /// Fixed suggestions.
    pub suggestions: Vec<String>,
    /// Ask the plugin's `complete-command` export.
    pub dynamic_suggestions: bool,
}

impl ArgSpec {
    /// An argument with no bounds and no suggestions.
    pub fn new(name: impl Into<String>, kind: ArgKind) -> Self {
        Self {
            name: name.into(),
            kind,
            min: None,
            max: None,
            suggestions: Vec::new(),
            dynamic_suggestions: false,
        }
    }
}

/// A node of a usage (WIT `node-spec`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NodeSpec {
    /// A fixed word.
    Literal(String),
    /// A typed argument.
    Argument(ArgSpec),
}

/// One path through a command (WIT `usage`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Usage {
    /// Nodes after the command name.
    pub nodes: Vec<NodeSpec>,
    /// Overrides the command's permission.
    pub permission: Option<String>,
    /// Description.
    pub description: String,
}

/// A command registration (WIT `command-spec`).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CommandSpec {
    /// Name.
    pub name: String,
    /// Aliases.
    pub aliases: Vec<String>,
    /// Description.
    pub description: String,
    /// Needed to see and run (none: everyone).
    pub permission: Option<String>,
    /// Refuse the console.
    pub players_only: bool,
    /// Usages.
    pub usages: Vec<Usage>,
}

/// A parsed argument value (WIT `arg-value`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArgValue {
    /// A word.
    Word(String),
    /// A string (unquoted and unescaped).
    String(String),
    /// The rest of the line.
    Greedy(String),
    /// An integer.
    Integer(i64),
    /// A number.
    Number(f64),
    /// A boolean.
    Boolean(bool),
    /// One player.
    Player(Uuid),
    /// Players.
    Players(Vec<Uuid>),
    /// A position (absolute, relative coordinates resolved).
    Position(Location),
    /// A block position.
    BlockPos(BlockPos),
    /// An item stack.
    Item(ItemStack),
    /// A world id.
    World(String),
    /// A duration in ticks.
    Duration(u64),
}

/// What a plugin's `execute-command` receives (WIT `command-context`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CommandContext {
    /// Who ran it.
    pub sender: CommandSender,
    /// The name or alias typed.
    pub label: String,
    /// Index into `CommandSpec::usages`.
    pub usage: u32,
    /// Parsed arguments, in usage order.
    pub args: Vec<(String, ArgValue)>,
    /// The whole line, without slash.
    pub raw: String,
}

impl CommandContext {
    /// The value of an argument by name.
    pub fn arg(&self, name: &str) -> Option<&ArgValue> {
        self.args.iter().find(|(n, _)| n == name).map(|(_, v)| v)
    }
}

/// What a plugin's `complete-command` receives (WIT `complete-context`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct CompleteContext {
    /// Who asks.
    pub sender: CommandSender,
    /// The line so far, without slash.
    pub raw: String,
    /// The argument being completed.
    pub argument: String,
    /// What is typed of it so far.
    pub partial: String,
}

/// WIT `server.plugin-meta`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PluginMeta {
    /// Id.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Version.
    pub version: String,
    /// Enabled.
    pub enabled: bool,
}

/// WIT `server.tick-stats`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct TickStats {
    /// Mean ms per tick over the last 100 ticks.
    pub mspt: f64,
    /// Ticks per second over the last minute (max 20).
    pub tps: f64,
}

/// WIT `menus.menu-spec`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct MenuSpec {
    /// 1 to 6 rows of 9.
    pub rows: u8,
    /// Title.
    pub title: Text,
    /// Items by slot.
    pub items: Vec<(u32, ItemStack)>,
}

/// WIT `scoreboards.display-slot`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DisplaySlot {
    /// Sidebar.
    Sidebar,
    /// Tab list.
    PlayerList,
    /// Below the name tag.
    BelowName,
}

/// WIT `scoreboards.team-color`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[allow(missing_docs)]
pub enum TeamColor {
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

/// WIT `bossbars.bar-color`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[allow(missing_docs)]
pub enum BarColor {
    Pink,
    Blue,
    Red,
    Green,
    Yellow,
    Purple,
    White,
}

/// WIT `bossbars.bar-style`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[allow(missing_docs)]
pub enum BarStyle {
    Progress,
    #[serde(rename = "notched-6")]
    Notched6,
    #[serde(rename = "notched-10")]
    Notched10,
    #[serde(rename = "notched-12")]
    Notched12,
    #[serde(rename = "notched-20")]
    Notched20,
}

/// WIT `http.request`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct HttpRequest {
    /// Method.
    pub method: String,
    /// URL.
    pub url: String,
    /// Headers.
    pub headers: Vec<(String, String)>,
    /// Body.
    pub body: Vec<u8>,
    /// Timeout.
    pub timeout_ms: u32,
}

/// WIT `http.response`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct HttpResponse {
    /// Status code.
    pub status: u16,
    /// Headers.
    pub headers: Vec<(String, String)>,
    /// Body.
    pub body: Vec<u8>,
}

/// `rustic:services/common.service-error`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "kebab-case")]
pub enum ServiceError {
    /// No provider is loaded.
    #[error("no provider")]
    NoProvider,
    /// The provider trapped or was disabled during the call.
    #[error("the provider failed")]
    ProviderFailed,
    /// The provider refused.
    #[error("refused: {0}")]
    Refused(String),
}

/// `rustic:services/permission-provider.tristate`.
pub use rusticmc_permissions::Tristate;
