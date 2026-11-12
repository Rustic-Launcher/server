//! The packaged manifest must pass the server's strict validation.

use rusticmc_core::manifest::Manifest;

#[test]
fn manifest_is_valid() {
    let text = include_str!("../rustic.plugin.toml");
    let m = Manifest::parse(text).unwrap_or_else(|e| panic!("rustic.plugin.toml refused: {e}"));
    assert_eq!(m.id.to_string(), "chat-format");
    assert_eq!(m.entry, "chat_format.wasm");
    assert_eq!(m.version.to_string(), env!("CARGO_PKG_VERSION"));
}
