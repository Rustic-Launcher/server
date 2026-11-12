//! Mirrors of `rustic:api/types`.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// A player's or entity's UUID as two 64-bit halves, most significant first (WIT `uuid`).
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
pub struct Uuid {
    /// Most significant 64 bits.
    pub hi: u64,
    /// Least significant 64 bits.
    pub lo: u64,
}

impl Uuid {
    /// The nil UUID.
    pub const NIL: Uuid = Uuid { hi: 0, lo: 0 };

    /// From the two halves.
    pub const fn new(hi: u64, lo: u64) -> Self {
        Self { hi, lo }
    }

    /// From a 128-bit integer.
    pub const fn from_u128(v: u128) -> Self {
        Self {
            hi: (v >> 64) as u64,
            lo: v as u64,
        }
    }

    /// As a 128-bit integer.
    pub const fn as_u128(self) -> u128 {
        ((self.hi as u128) << 64) | self.lo as u128
    }

    /// Whether this is the nil UUID.
    pub const fn is_nil(self) -> bool {
        self.hi == 0 && self.lo == 0
    }
}

impl fmt::Display for Uuid {
    /// Lowercase hyphenated form: `069a79f4-44e9-4726-a5be-fca90e38aaf5`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let v = self.as_u128();
        write!(
            f,
            "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
            (v >> 96) as u32,
            (v >> 80) as u16,
            (v >> 64) as u16,
            (v >> 48) as u16,
            v & 0xffff_ffff_ffff
        )
    }
}

/// Error parsing a [`Uuid`].
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[error("invalid UUID `{0}` (expected 32 hex digits, optionally hyphenated 8-4-4-4-12)")]
pub struct UuidParseError(pub String);

impl FromStr for Uuid {
    type Err = UuidParseError;

    /// Accepts the hyphenated form and the 32-digit simple form, any case.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        rusticmc_permissions::parse_uuid(s)
            .map(Uuid::from_u128)
            .ok_or_else(|| UuidParseError(s.to_owned()))
    }
}

impl From<u128> for Uuid {
    fn from(v: u128) -> Self {
        Uuid::from_u128(v)
    }
}

impl From<Uuid> for u128 {
    fn from(v: Uuid) -> Self {
        v.as_u128()
    }
}

#[cfg(feature = "uuid")]
impl From<uuid::Uuid> for Uuid {
    fn from(v: uuid::Uuid) -> Self {
        Uuid::from_u128(v.as_u128())
    }
}

#[cfg(feature = "uuid")]
impl From<Uuid> for uuid::Uuid {
    fn from(v: Uuid) -> Self {
        uuid::Uuid::from_u128(v.as_u128())
    }
}

/// WIT `vec3`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Vec3 {
    /// X.
    pub x: f64,
    /// Y.
    pub y: f64,
    /// Z.
    pub z: f64,
}

/// WIT `block-pos`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlockPos {
    /// X.
    pub x: i32,
    /// Y.
    pub y: i32,
    /// Z.
    pub z: i32,
}

/// A point in a world (WIT `location`). `world` is the dimension id ("minecraft:overworld").
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Location {
    /// Dimension id.
    pub world: String,
    /// X.
    pub x: f64,
    /// Y.
    pub y: f64,
    /// Z.
    pub z: f64,
    /// Yaw in degrees.
    pub yaw: f32,
    /// Pitch in degrees.
    pub pitch: f32,
}

impl Location {
    /// Whether every coordinate is finite.
    pub fn is_finite(&self) -> bool {
        self.x.is_finite()
            && self.y.is_finite()
            && self.z.is_finite()
            && self.yaw.is_finite()
            && self.pitch.is_finite()
    }

    /// The block containing this point.
    pub fn block_pos(&self) -> BlockPos {
        BlockPos {
            x: self.x.floor() as i32,
            y: self.y.floor() as i32,
            z: self.z.floor() as i32,
        }
    }

    /// The position as a vector.
    pub fn vec3(&self) -> Vec3 {
        Vec3 {
            x: self.x,
            y: self.y,
            z: self.z,
        }
    }
}

/// WIT `game-mode`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GameMode {
    /// Survival.
    Survival,
    /// Creative.
    Creative,
    /// Adventure.
    Adventure,
    /// Spectator.
    Spectator,
}

/// A text component in vanilla's JSON chat-component format (WIT `text`, a JSON string).
///
/// A plain string is JSON-quoted: `Text::plain("hi").as_json() == "\"hi\""`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Text(String);

impl Default for Text {
    fn default() -> Self {
        Text::plain("")
    }
}

impl fmt::Display for Text {
    /// Shows the plain text (see [`Text::to_plain`]).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_plain())
    }
}

/// Named colours of the chat format.
pub const TEXT_COLORS: [&str; 16] = [
    "black",
    "dark_blue",
    "dark_green",
    "dark_aqua",
    "dark_red",
    "dark_purple",
    "gold",
    "gray",
    "dark_gray",
    "blue",
    "green",
    "aqua",
    "red",
    "light_purple",
    "yellow",
    "white",
];

impl Text {
    /// A plain, unstyled text.
    pub fn plain(s: impl AsRef<str>) -> Self {
        Text(serde_json::Value::String(s.as_ref().to_owned()).to_string())
    }

    /// A text with one colour: a named colour (`"red"`, `"gold"`...) or `#RRGGBB`.
    pub fn colored(s: impl AsRef<str>, color: &str) -> Self {
        let mut map = serde_json::Map::new();
        map.insert(
            "text".into(),
            serde_json::Value::String(s.as_ref().to_owned()),
        );
        map.insert("color".into(), serde_json::Value::String(color.to_owned()));
        Text(serde_json::Value::Object(map).to_string())
    }

    /// A red text, as used for errors.
    pub fn error(s: impl AsRef<str>) -> Self {
        Self::colored(s, "red")
    }

    /// Wrap a JSON component without checking it.
    pub fn from_json_unchecked(json: impl Into<String>) -> Self {
        Text(json.into())
    }

    /// Wrap a JSON component after checking that it is a JSON string, array or object.
    pub fn from_json(json: impl Into<String>) -> Result<Self, String> {
        let json = json.into();
        match serde_json::from_str::<serde_json::Value>(&json) {
            Ok(serde_json::Value::String(_))
            | Ok(serde_json::Value::Array(_))
            | Ok(serde_json::Value::Object(_)) => Ok(Text(json)),
            Ok(_) => Err("a text component must be a JSON string, array or object".into()),
            Err(e) => Err(format!("invalid text component: {e}")),
        }
    }

    /// The JSON.
    pub fn as_json(&self) -> &str {
        &self.0
    }

    /// The JSON, owned.
    pub fn into_json(self) -> String {
        self.0
    }

    /// The text without styling: concatenation of `text` fields and `extra` children
    /// (translation keys are shown as is). Invalid JSON is returned verbatim.
    pub fn to_plain(&self) -> String {
        fn walk(v: &serde_json::Value, out: &mut String) {
            match v {
                serde_json::Value::String(s) => out.push_str(s),
                serde_json::Value::Array(items) => items.iter().for_each(|i| walk(i, out)),
                serde_json::Value::Object(map) => {
                    if let Some(t) = map.get("text") {
                        walk(t, out);
                    } else if let Some(serde_json::Value::String(k)) = map.get("translate") {
                        out.push_str(k);
                    } else if let Some(serde_json::Value::String(k)) = map.get("keybind") {
                        out.push_str(k);
                    }
                    if let Some(extra) = map.get("extra") {
                        walk(extra, out);
                    }
                }
                serde_json::Value::Number(n) => out.push_str(&n.to_string()),
                serde_json::Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
                serde_json::Value::Null => {}
            }
        }
        match serde_json::from_str::<serde_json::Value>(&self.0) {
            Ok(v) => {
                let mut out = String::new();
                walk(&v, &mut out);
                out
            }
            Err(_) => self.0.clone(),
        }
    }
}

impl From<&str> for Text {
    fn from(s: &str) -> Self {
        Text::plain(s)
    }
}

impl From<String> for Text {
    fn from(s: String) -> Self {
        Text::plain(s)
    }
}

/// Listener priority, called in this order (WIT `priority`). `Monitor` observes only.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum Priority {
    /// Called first.
    Lowest,
    /// Second.
    Low,
    /// The default.
    #[default]
    Normal,
    /// Fourth.
    High,
    /// Last that can change the event.
    Highest,
    /// Observes the final outcome; returned changes are ignored.
    Monitor,
}

impl Priority {
    /// All priorities in call order.
    pub const ALL: [Priority; 6] = [
        Priority::Lowest,
        Priority::Low,
        Priority::Normal,
        Priority::High,
        Priority::Highest,
        Priority::Monitor,
    ];
}

/// An item stack (WIT `item-stack`). `count` 0 means empty; `components` is SNBT.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ItemStack {
    /// Namespaced item id.
    pub id: String,
    /// Count (0 = empty).
    pub count: u32,
    /// Data-component patch as SNBT, empty for defaults.
    pub components: String,
}

impl ItemStack {
    /// A stack of `count` default items.
    pub fn new(id: impl Into<String>, count: u32) -> Self {
        Self {
            id: id.into(),
            count,
            components: String::new(),
        }
    }

    /// The empty stack.
    pub fn empty() -> Self {
        Self::new("minecraft:air", 0)
    }

    /// Whether it is empty.
    pub fn is_empty(&self) -> bool {
        self.count == 0 || self.id == "minecraft:air"
    }
}

/// A block state (WIT `block-state`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlockState {
    /// Namespaced block id.
    pub id: String,
    /// State properties.
    pub properties: Vec<(String, String)>,
}

/// WIT `log-level`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LogLevel {
    /// Trace.
    Trace,
    /// Debug.
    Debug,
    /// Info.
    Info,
    /// Warn.
    Warn,
    /// Error.
    Error,
}

impl From<LogLevel> for log::Level {
    fn from(l: LogLevel) -> Self {
        match l {
            LogLevel::Trace => log::Level::Trace,
            LogLevel::Debug => log::Level::Debug,
            LogLevel::Info => log::Level::Info,
            LogLevel::Warn => log::Level::Warn,
            LogLevel::Error => log::Level::Error,
        }
    }
}

/// Who runs a command or is asked for a permission (WIT `command-sender`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CommandSender {
    /// The server console (has every permission).
    Console,
    /// A player.
    Player(Uuid),
}

impl CommandSender {
    /// The player's UUID, if a player.
    pub fn player(&self) -> Option<Uuid> {
        match self {
            CommandSender::Console => None,
            CommandSender::Player(u) => Some(*u),
        }
    }
}

/// An entity in a world (WIT `entity-ref`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityRef {
    /// Network id (valid while it lives).
    pub id: u32,
    /// UUID.
    pub uuid: Uuid,
    /// Entity type id.
    pub kind: String,
}

/// WIT `hand`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Hand {
    /// Main hand.
    MainHand,
    /// Off hand.
    OffHand,
}

/// WIT `block-face`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BlockFace {
    /// -Y.
    Down,
    /// +Y.
    Up,
    /// -Z.
    North,
    /// +Z.
    South,
    /// -X.
    West,
    /// +X.
    East,
}

/// WIT `weather`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Weather {
    /// Clear.
    Clear,
    /// Rain.
    Rain,
    /// Thunder.
    Thunder,
}

/// Default value of a permission node (WIT `permission-default`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PermissionDefault {
    /// Everyone.
    Allow,
    /// Nobody.
    Deny,
    /// Operators only (the default for undeclared nodes).
    #[default]
    Op,
    /// Everyone but operators.
    NotOp,
}

impl PermissionDefault {
    /// Evaluate for someone who is (or is not) an operator.
    pub fn evaluate(self, is_op: bool) -> bool {
        match self {
            PermissionDefault::Allow => true,
            PermissionDefault::Deny => false,
            PermissionDefault::Op => is_op,
            PermissionDefault::NotOp => !is_op,
        }
    }
}

/// Errors returned by host calls (WIT `host-error`).
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "kebab-case")]
pub enum HostError {
    /// The player is offline or unknown.
    #[error("no such player")]
    NoSuchPlayer,
    /// The world is unknown or not loaded.
    #[error("no such world")]
    NoSuchWorld,
    /// The plugin's manifest did not grant the capability.
    #[error("capability denied: {0}")]
    CapabilityDenied(String),
    /// Main (tick) instance only.
    #[error("this call is only allowed on the main thread")]
    MainThreadOnly,
    /// Async instance only.
    #[error("this call is only allowed in an async task")]
    AsyncOnly,
    /// Bad argument.
    #[error("invalid argument: {0}")]
    Invalid(String),
    /// The engine cannot do this yet.
    #[error("unsupported: {0}")]
    Unsupported(String),
    /// Anything else.
    #[error("{0}")]
    Other(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_round_trip() {
        let s = "069a79f4-44e9-4726-a5be-fca90e38aaf5";
        let u: Uuid = s.parse().unwrap();
        assert_eq!(u.to_string(), s);
        assert_eq!(u.hi, 0x069a79f444e94726);
        assert_eq!(u.lo, 0xa5befca90e38aaf5);
        assert_eq!(
            "069A79F444E94726A5BEFCA90E38AAF5".parse::<Uuid>().unwrap(),
            u
        );
        assert!("nope".parse::<Uuid>().is_err());
        assert_eq!(Uuid::from_u128(u.as_u128()), u);
        assert_eq!(
            Uuid::NIL.to_string(),
            "00000000-0000-0000-0000-000000000000"
        );
    }

    #[test]
    fn text_constructors_and_plain() {
        assert_eq!(Text::plain("hi").as_json(), "\"hi\"");
        assert_eq!(Text::plain("a\"b").to_plain(), "a\"b");
        let c = Text::colored("Gold!", "gold");
        assert_eq!(c.to_plain(), "Gold!");
        assert!(c.as_json().contains("\"color\":\"gold\""));
        let nested = Text::from_json(
            r#"{"text":"a","extra":[{"text":"b","color":"red"},"c",{"translate":"k.x"}]}"#,
        )
        .unwrap();
        assert_eq!(nested.to_plain(), "abck.x");
        assert!(Text::from_json("42").is_err());
        assert!(Text::from_json("{").is_err());
        assert_eq!(Text::from_json_unchecked("not json").to_plain(), "not json");
        assert_eq!(Text::default().to_plain(), "");
        assert_eq!(format!("{}", Text::error("x")), "x");
    }

    #[test]
    fn permission_defaults() {
        assert!(PermissionDefault::Allow.evaluate(false));
        assert!(!PermissionDefault::Deny.evaluate(true));
        assert!(PermissionDefault::Op.evaluate(true));
        assert!(!PermissionDefault::Op.evaluate(false));
        assert!(PermissionDefault::NotOp.evaluate(false));
    }

    #[test]
    fn priorities_order() {
        let mut p = Priority::ALL.to_vec();
        p.reverse();
        p.sort();
        assert_eq!(p, Priority::ALL);
    }
}
