//! The boundary to host imports.
//!
//! Every wrapper calls [`guard`] before a raw import. Inside a component it is a no-op; on the
//! host target (unit tests, `cargo check` without `--target wasm32-wasip2`) it panics with a clear
//! message instead of wit-bindgen's bare `unreachable!()` stubs.

/// Panics outside WebAssembly, naming the import that was called.
#[inline(always)]
#[track_caller]
pub(crate) fn guard(import: &'static str) {
    #[cfg(not(target_family = "wasm"))]
    panic!(
        "rusticmc-api: `{import}` is a host import and only exists inside a Rustic plugin \
         (wasm32-wasip2 component). On the host target, test pure logic directly or load the \
         built plugin with the rusticmc-test harness."
    );
    #[cfg(target_family = "wasm")]
    let _ = import;
}

/// Calls a raw import after the [`guard`]: `call!("players.name", players::name(u))`.
macro_rules! call {
    ($name:literal, $e:expr) => {{
        $crate::host::guard($name);
        $e
    }};
}
pub(crate) use call;
