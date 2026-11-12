//! Rust mirrors of every WIT type of `rustic:api@0.1.0` and `rustic:services@0.1.0`.
//!
//! Names are the WIT names in UpperCamelCase, fields are the same in snake_case; serde uses the
//! kebab-case WIT names so JSON dumps read like the contract. WIT `http.request` /
//! `http.response` are [`HttpRequest`] / [`HttpResponse`] (to avoid two types called `Request`).

mod api;
mod events;
mod types;

pub use api::*;
pub use events::*;
pub use types::*;
