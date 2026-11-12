//! Logging to the server log (prefixed with the plugin id by the host).
//!
//! Use the macros: `info!("{} joined", name)`, `warn!`, `error!`, `debug!`, `trace!`.
//! On the host target (unit tests) lines go to stderr instead.

use crate::types::LogLevel;

/// Writes a line at `level`.
pub fn log(level: LogLevel, message: &str) {
    #[cfg(target_family = "wasm")]
    crate::bindings::rustic::api::logging::log(level, message);
    #[cfg(not(target_family = "wasm"))]
    eprintln!("[{level:?}] {message}");
}

/// Logs at info level: `info!("{} joined", name)`.
#[macro_export]
macro_rules! info {
    ($($arg:tt)*) => { $crate::log::log($crate::LogLevel::Info, &::std::format!($($arg)*)) };
}

/// Logs at warn level.
#[macro_export]
macro_rules! warn {
    ($($arg:tt)*) => { $crate::log::log($crate::LogLevel::Warn, &::std::format!($($arg)*)) };
}

/// Logs at error level.
#[macro_export]
macro_rules! error {
    ($($arg:tt)*) => { $crate::log::log($crate::LogLevel::Error, &::std::format!($($arg)*)) };
}

/// Logs at debug level.
#[macro_export]
macro_rules! debug {
    ($($arg:tt)*) => { $crate::log::log($crate::LogLevel::Debug, &::std::format!($($arg)*)) };
}

/// Logs at trace level.
#[macro_export]
macro_rules! trace {
    ($($arg:tt)*) => { $crate::log::log($crate::LogLevel::Trace, &::std::format!($($arg)*)) };
}
