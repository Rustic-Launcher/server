//! Plugin identifiers.

use serde::{Deserialize, Serialize};
use std::borrow::Borrow;
use std::fmt;
use std::ops::Deref;
use std::sync::Arc;

/// Ids no plugin may use: the core owns `rustic` (built-in commands, permissions), and the
/// other two are namespaces of the game.
pub const RESERVED_IDS: [&str; 3] = ["rustic", "minecraft", "horizon"];

/// The owner id used for everything the core itself registers (the `/rustic` command, its
/// permissions).
pub const CORE_OWNER: &str = "rustic";

/// A plugin id: `[a-z0-9_-]{1,64}`. Cheap to clone (shared string).
#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PluginId(Arc<str>);

/// Why a string is not a valid plugin id.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PluginIdError {
    /// Wrong length or characters.
    #[error("plugin id `{0}` must be 1 to 64 characters of [a-z0-9_-]")]
    Syntax(String),
    /// A reserved id.
    #[error("plugin id `{0}` is reserved")]
    Reserved(String),
}

/// Whether `s` has plugin-id syntax (does not check reserved ids).
pub fn is_valid_id_syntax(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-'))
}

impl PluginId {
    /// Validate and wrap a plugin id (reserved ids are refused).
    pub fn new(s: &str) -> Result<Self, PluginIdError> {
        if !is_valid_id_syntax(s) {
            return Err(PluginIdError::Syntax(s.to_owned()));
        }
        if RESERVED_IDS.contains(&s) {
            return Err(PluginIdError::Reserved(s.to_owned()));
        }
        Ok(PluginId(Arc::from(s)))
    }

    /// Wrap without validation (used for the core's own owner id and in tests).
    pub fn new_unchecked(s: &str) -> Self {
        PluginId(Arc::from(s))
    }

    /// The core's owner id.
    pub fn core() -> Self {
        Self::new_unchecked(CORE_OWNER)
    }

    /// As a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Deref for PluginId {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl Borrow<str> for PluginId {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for PluginId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PluginId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl fmt::Debug for PluginId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&*self.0, f)
    }
}

impl PartialEq<str> for PluginId {
    fn eq(&self, other: &str) -> bool {
        &*self.0 == other
    }
}

impl PartialEq<&str> for PluginId {
    fn eq(&self, other: &&str) -> bool {
        &*self.0 == *other
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation() {
        assert!(PluginId::new("homes").is_ok());
        assert!(PluginId::new("a_b-9").is_ok());
        assert!(PluginId::new(&"a".repeat(64)).is_ok());
        assert!(PluginId::new(&"a".repeat(65)).is_err());
        assert!(PluginId::new("").is_err());
        assert!(PluginId::new("Homes").is_err());
        assert!(PluginId::new("a.b").is_err());
        assert_eq!(
            PluginId::new("rustic"),
            Err(PluginIdError::Reserved("rustic".into()))
        );
        let id = PluginId::new("x").unwrap();
        assert_eq!(id, "x");
        assert_eq!(format!("{id}"), "x");
    }
}
