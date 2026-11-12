//! Host-provided key-value storage, one namespace per plugin, persisted by the server. Shared by
//! the main and async instances. Typed helpers store values as JSON.
//!
//! Keys starting with `__rustic/` are reserved for the SDK and hidden from [`Storage::keys`].

use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::bindings::rustic::api::storage as wit;
use crate::error::{Error, Result};
use crate::host::call;

/// Prefix of the SDK's own keys.
pub(crate) const RESERVED_PREFIX: &str = "__rustic/";

/// The plugin's key-value store.
#[derive(Debug, Clone, Copy)]
pub struct Storage;

fn check_key(key: &str) -> Result<()> {
    if key.starts_with(RESERVED_PREFIX) {
        return Err(Error::Other(format!(
            "storage keys starting with `{RESERVED_PREFIX}` are reserved for the SDK"
        )));
    }
    Ok(())
}

impl Storage {
    /// A typed value.
    pub fn get<T: DeserializeOwned>(key: &str) -> Result<Option<T>> {
        match Self::get_raw(key) {
            Some(bytes) => Ok(Some(serde_json::from_slice(&bytes)?)),
            None => Ok(None),
        }
    }

    /// A typed value, or `T::default()`.
    pub fn get_or_default<T: DeserializeOwned + Default>(key: &str) -> Result<T> {
        Ok(Self::get(key)?.unwrap_or_default())
    }

    /// Stores a typed value.
    pub fn set<T: Serialize + ?Sized>(key: &str, value: &T) -> Result<()> {
        check_key(key)?;
        let bytes = serde_json::to_vec(value)?;
        call!("storage.set", wit::set(key, &bytes));
        Ok(())
    }

    /// Reads, changes and writes back a value (`T::default()` when absent); returns the new value.
    pub fn update<T, F>(key: &str, f: F) -> Result<T>
    where
        T: Serialize + DeserializeOwned + Default,
        F: FnOnce(&mut T),
    {
        let mut v: T = Self::get_or_default(key)?;
        f(&mut v);
        Self::set(key, &v)?;
        Ok(v)
    }

    /// Raw bytes.
    pub fn get_raw(key: &str) -> Option<Vec<u8>> {
        call!("storage.get", wit::get(key))
    }

    /// Stores raw bytes.
    pub fn set_raw(key: &str, value: &[u8]) -> Result<()> {
        check_key(key)?;
        call!("storage.set", wit::set(key, value));
        Ok(())
    }

    /// Deletes a key; returns whether it existed.
    pub fn delete(key: &str) -> bool {
        call!("storage.delete", wit::delete(key))
    }

    /// Keys starting with `prefix` (SDK-reserved keys excluded).
    pub fn keys(prefix: &str) -> Vec<String> {
        call!("storage.keys", wit::keys(prefix))
            .into_iter()
            .filter(|k| !k.starts_with(RESERVED_PREFIX))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reserved_keys_rejected() {
        assert!(check_key("__rustic/x").is_err());
        assert!(check_key("joins").is_ok());
    }
}
