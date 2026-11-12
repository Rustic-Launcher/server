//! Shared value types: the WIT records that are already pleasant to use, re-exported with a few
//! constructors and conversions added.

use std::fmt;
use std::hash::{Hash, Hasher};
use std::str::FromStr;

pub use crate::bindings::rustic::api::bossbars::{BarColor, BarStyle};
pub use crate::bindings::rustic::api::event_types::{ClickKind, InteractAction};
pub use crate::bindings::rustic::api::scoreboards::{DisplaySlot, TeamColor};
pub use crate::bindings::rustic::api::server::{PluginMeta, TickStats};
pub use crate::bindings::rustic::api::types::{
    BlockFace, BlockPos, BlockState, EntityRef, GameMode, Hand, HostError, Location, LogLevel,
    PermissionDefault, Priority, Uuid, Vec3, Weather,
};

// ---------------------------------------------------------------------------------------------
// Uuid

impl Uuid {
    /// The nil UUID (all zeros).
    pub const NIL: Uuid = Uuid { hi: 0, lo: 0 };

    /// From the 128-bit value, most significant half first.
    pub const fn from_u128(v: u128) -> Self {
        Uuid {
            hi: (v >> 64) as u64,
            lo: v as u64,
        }
    }

    /// The 128-bit value.
    pub const fn as_u128(self) -> u128 {
        ((self.hi as u128) << 64) | self.lo as u128
    }
}

impl Eq for Uuid {}

impl Hash for Uuid {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hi.hash(state);
        self.lo.hash(state);
    }
}

impl PartialOrd for Uuid {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Uuid {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_u128().cmp(&other.as_u128())
    }
}

impl fmt::Display for Uuid {
    /// Hyphenated lowercase form: `069a79f4-44e9-4726-a5be-fca90e38aaf5`.
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

/// Error parsing a [`Uuid`] from text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseUuidError;

impl fmt::Display for ParseUuidError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid UUID (expected 32 hex digits, hyphens optional)")
    }
}

impl std::error::Error for ParseUuidError {}

impl FromStr for Uuid {
    type Err = ParseUuidError;

    /// Accepts the hyphenated and the plain 32-digit forms.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let hex: String = s.chars().filter(|c| *c != '-').collect();
        if hex.len() != 32 || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(ParseUuidError);
        }
        u128::from_str_radix(&hex, 16)
            .map(Uuid::from_u128)
            .map_err(|_| ParseUuidError)
    }
}

impl serde::Serialize for Uuid {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.collect_str(self)
    }
}

impl<'de> serde::Deserialize<'de> for Uuid {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

// ---------------------------------------------------------------------------------------------
// Positions

impl Vec3 {
    /// A vector.
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Vec3 { x, y, z }
    }

    /// Euclidean distance.
    pub fn distance(&self, other: &Vec3) -> f64 {
        ((self.x - other.x).powi(2) + (self.y - other.y).powi(2) + (self.z - other.z).powi(2))
            .sqrt()
    }

    /// The block containing this point.
    pub fn block(&self) -> BlockPos {
        BlockPos::new(
            self.x.floor() as i32,
            self.y.floor() as i32,
            self.z.floor() as i32,
        )
    }
}

impl BlockPos {
    /// A block position.
    pub const fn new(x: i32, y: i32, z: i32) -> Self {
        BlockPos { x, y, z }
    }

    /// This position moved by `(dx, dy, dz)`.
    pub const fn offset(&self, dx: i32, dy: i32, dz: i32) -> Self {
        BlockPos::new(self.x + dx, self.y + dy, self.z + dz)
    }

    /// The centre of the block.
    pub fn center(&self) -> Vec3 {
        Vec3::new(
            f64::from(self.x) + 0.5,
            f64::from(self.y) + 0.5,
            f64::from(self.z) + 0.5,
        )
    }

    /// Chunk coordinates `(cx, cz)` containing this block.
    pub const fn chunk(&self) -> (i32, i32) {
        (self.x >> 4, self.z >> 4)
    }
}

impl Eq for BlockPos {}

impl Hash for BlockPos {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (self.x, self.y, self.z).hash(state);
    }
}

impl Location {
    /// A location facing yaw 0, pitch 0.
    pub fn new(world: impl Into<String>, x: f64, y: f64, z: f64) -> Self {
        Location {
            world: world.into(),
            x,
            y,
            z,
            yaw: 0.0,
            pitch: 0.0,
        }
    }

    /// Sets the facing.
    pub fn facing(mut self, yaw: f32, pitch: f32) -> Self {
        self.yaw = yaw;
        self.pitch = pitch;
        self
    }

    /// The position without world and facing.
    pub fn position(&self) -> Vec3 {
        Vec3::new(self.x, self.y, self.z)
    }

    /// The block containing this location.
    pub fn block(&self) -> BlockPos {
        self.position().block()
    }

    /// The world, as a handle.
    pub fn world(&self) -> crate::World {
        crate::World::new(self.world.clone())
    }
}

// ---------------------------------------------------------------------------------------------
// Block states

impl BlockState {
    /// A block with default properties: `BlockState::new("minecraft:stone")`. An id without
    /// namespace gets `minecraft:`.
    pub fn new(id: impl Into<String>) -> Self {
        BlockState {
            id: namespaced(id.into()),
            properties: Vec::new(),
        }
    }

    /// Sets a state property: `BlockState::new("minecraft:oak_log").with("axis", "y")`.
    pub fn with(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        let key = key.into();
        let value = value.into();
        match self.properties.iter_mut().find(|(k, _)| *k == key) {
            Some(slot) => slot.1 = value,
            None => self.properties.push((key, value)),
        }
        self
    }

    /// A property's value.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.properties
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }

    /// Whether this is air (any of the air blocks).
    pub fn is_air(&self) -> bool {
        matches!(
            self.id.as_str(),
            "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
        )
    }
}

impl fmt::Display for BlockState {
    /// `minecraft:oak_log[axis=y]`, the command syntax.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.id)?;
        if !self.properties.is_empty() {
            f.write_str("[")?;
            for (i, (k, v)) in self.properties.iter().enumerate() {
                if i > 0 {
                    f.write_str(",")?;
                }
                write!(f, "{k}={v}")?;
            }
            f.write_str("]")?;
        }
        Ok(())
    }
}

/// Error parsing a [`BlockState`] from `id[key=value,...]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseBlockStateError(pub String);

impl fmt::Display for ParseBlockStateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid block state: {}", self.0)
    }
}

impl std::error::Error for ParseBlockStateError {}

impl FromStr for BlockState {
    type Err = ParseBlockStateError;

    /// Parses the command syntax `minecraft:oak_log[axis=y]` (namespace optional).
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        let (id, props) = match s.find('[') {
            Some(i) => {
                let rest = &s[i + 1..];
                let Some(inner) = rest.strip_suffix(']') else {
                    return Err(ParseBlockStateError(format!("missing `]` in `{s}`")));
                };
                (&s[..i], Some(inner))
            }
            None => (s, None),
        };
        if id.is_empty() || id.contains(char::is_whitespace) {
            return Err(ParseBlockStateError(format!("bad block id in `{s}`")));
        }
        let mut state = BlockState::new(id);
        if let Some(props) = props {
            for pair in props.split(',').filter(|p| !p.trim().is_empty()) {
                let Some((k, v)) = pair.split_once('=') else {
                    return Err(ParseBlockStateError(format!(
                        "expected key=value, got `{pair}`"
                    )));
                };
                state = state.with(k.trim(), v.trim());
            }
        }
        Ok(state)
    }
}

impl From<&str> for BlockState {
    /// Parses `id[props]`; on a syntax error the whole string is used as the id (the host then
    /// reports the unknown block).
    fn from(s: &str) -> Self {
        s.parse().unwrap_or_else(|_| BlockState {
            id: s.to_owned(),
            properties: Vec::new(),
        })
    }
}

impl EntityRef {
    /// The entity's world-wide UUID.
    pub fn uuid(&self) -> Uuid {
        self.uuid
    }

    /// Whether this entity is a player.
    pub fn is_player(&self) -> bool {
        self.kind == "minecraft:player"
    }

    /// The player behind this entity, if it is one.
    pub fn as_player(&self) -> Option<crate::Player> {
        self.is_player()
            .then(|| crate::Player::from_uuid(self.uuid))
    }
}

/// Adds `minecraft:` to an id that has no namespace.
pub(crate) fn namespaced(id: String) -> String {
    if id.contains(':') {
        id
    } else {
        format!("minecraft:{id}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uuid_roundtrip() {
        let s = "069a79f4-44e9-4726-a5be-fca90e38aaf5";
        let u: Uuid = s.parse().unwrap();
        assert_eq!(u.to_string(), s);
        assert_eq!(u.hi, 0x069a79f444e94726);
        assert_eq!(u.lo, 0xa5befca90e38aaf5);
        assert_eq!(
            "069a79f444e94726a5befca90e38aaf5".parse::<Uuid>().unwrap(),
            u
        );
        assert!("nope".parse::<Uuid>().is_err());
        assert_eq!(Uuid::from_u128(u.as_u128()), u);
        let json = serde_json::to_string(&u).unwrap();
        assert_eq!(json, format!("\"{s}\""));
        assert_eq!(serde_json::from_str::<Uuid>(&json).unwrap(), u);
    }

    #[test]
    fn block_state_parse_and_display() {
        let b: BlockState = "oak_log[axis=y, waterlogged=false]".parse().unwrap();
        assert_eq!(b.id, "minecraft:oak_log");
        assert_eq!(b.get("axis"), Some("y"));
        assert_eq!(b.get("waterlogged"), Some("false"));
        assert_eq!(b.to_string(), "minecraft:oak_log[axis=y,waterlogged=false]");
        assert_eq!(BlockState::new("stone").to_string(), "minecraft:stone");
        assert!("stone[axis".parse::<BlockState>().is_err());
        assert!("stone[axis]".parse::<BlockState>().is_err());
        assert!(BlockState::new("air").is_air());
        let b = BlockState::new("x:y").with("a", "1").with("a", "2");
        assert_eq!(b.properties, vec![("a".to_owned(), "2".to_owned())]);
    }

    #[test]
    fn positions() {
        let p = Vec3::new(-0.5, 64.2, 15.9);
        assert_eq!(p.block(), BlockPos::new(-1, 64, 15));
        assert_eq!(BlockPos::new(-1, 0, 17).chunk(), (-1, 1));
        assert_eq!(
            BlockPos::new(1, 2, 3).offset(1, -2, 0),
            BlockPos::new(2, 0, 3)
        );
        assert!((Vec3::new(0.0, 0.0, 0.0).distance(&Vec3::new(3.0, 4.0, 0.0)) - 5.0).abs() < 1e-9);
    }
}
