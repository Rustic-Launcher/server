//! The SDK's error type.

use std::fmt;

use crate::types::HostError;

/// Errors of SDK calls. `?` works on host errors, serde/TOML errors, I/O errors and strings,
/// so `Plugin::enable` and handlers can propagate almost anything.
#[derive(Debug, Clone, PartialEq)]
pub enum Error {
    /// The host refused or failed a call.
    Host(HostError),
    /// (De)serialization of stored or transferred data failed.
    Serde(String),
    /// Reading or writing a file of the data folder failed.
    Io(String),
    /// The configuration file is invalid.
    Config(String),
    /// The call needs the main instance; this code runs in the async instance.
    MainThreadOnly(&'static str),
    /// The call needs the async instance (blocking I/O such as HTTP).
    AsyncOnly(&'static str),
    /// A `rustic:services` call failed (no provider, provider failed, refused).
    Service(crate::services::ServiceError),
    /// Anything else, with a message.
    Other(String),
}

/// `Result` with [`Error`] as the default error type.
pub type Result<T, E = Error> = std::result::Result<T, E>;

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Host(e) => write!(f, "{}", describe_host_error(e)),
            Error::Serde(m) => write!(f, "serialization error: {m}"),
            Error::Io(m) => write!(f, "I/O error: {m}"),
            Error::Config(m) => write!(f, "config error: {m}"),
            Error::MainThreadOnly(what) => {
                write!(
                    f,
                    "`{what}` only works on the main instance, not in an async task"
                )
            }
            Error::AsyncOnly(what) => {
                write!(
                    f,
                    "`{what}` only works in an async task (Scheduler::run_async)"
                )
            }
            Error::Service(e) => f.write_str(&crate::services::describe_service_error(e)),
            Error::Other(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for Error {}

/// Human-readable text for a host error.
pub(crate) fn describe_host_error(e: &HostError) -> String {
    match e {
        HostError::NoSuchPlayer => "that player is not online".to_owned(),
        HostError::NoSuchWorld => "that world is not loaded".to_owned(),
        HostError::CapabilityDenied(c) => {
            format!("the plugin manifest does not grant capability `{c}`")
        }
        HostError::MainThreadOnly => "only allowed on the main instance".to_owned(),
        HostError::AsyncOnly => "only allowed in an async task".to_owned(),
        HostError::Invalid(m) => format!("invalid argument: {m}"),
        HostError::Unsupported(m) => format!("not supported by this server yet: {m}"),
        HostError::Other(m) => m.clone(),
    }
}

impl From<HostError> for Error {
    fn from(e: HostError) -> Self {
        Error::Host(e)
    }
}

impl From<crate::services::ServiceError> for Error {
    fn from(e: crate::services::ServiceError) -> Self {
        Error::Service(e)
    }
}

impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Error::Serde(e.to_string())
    }
}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e.to_string())
    }
}

#[cfg(feature = "config")]
impl From<toml::de::Error> for Error {
    fn from(e: toml::de::Error) -> Self {
        Error::Config(e.to_string())
    }
}

#[cfg(feature = "config")]
impl From<toml::ser::Error> for Error {
    fn from(e: toml::ser::Error) -> Self {
        Error::Config(e.to_string())
    }
}

impl From<String> for Error {
    fn from(m: String) -> Self {
        Error::Other(m)
    }
}

impl From<&str> for Error {
    fn from(m: &str) -> Self {
        Error::Other(m.to_owned())
    }
}

impl From<std::fmt::Error> for Error {
    fn from(e: std::fmt::Error) -> Self {
        Error::Other(e.to_string())
    }
}
