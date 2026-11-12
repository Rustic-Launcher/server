//! The Rustic plugin API contract. The WIT files under `wit/` are the source of truth:
//! `rustic:api@0.1.0` (what a plugin imports and exports) and `rustic:services@0.1.0`
//! (standard service interfaces). See `docs/API.md` for the versioning policy.

/// Absolute path of the `wit/` directory (for `wasmtime::component::bindgen!` and `wit-bindgen`).
pub const WIT_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/wit");

/// The API version this crate describes.
pub const API_VERSION: &str = "0.1.0";

/// The world plugins target.
pub const WORLD: &str = "rustic:api/rustic-plugin@0.1.0";

#[cfg(test)]
mod tests {
    #[test]
    fn wit_parses_and_world_exists() {
        let mut resolve = wit_parser::Resolve::default();
        let (pkg, _) = resolve.push_dir(super::WIT_DIR).expect("WIT parses");
        let world = resolve
            .select_world(&[pkg], Some("rustic-plugin"))
            .expect("world");
        let w = &resolve.worlds[world];
        assert_eq!(w.exports.len(), 2, "plugin + handlers");
        assert!(w.imports.len() >= 18);
    }
}
