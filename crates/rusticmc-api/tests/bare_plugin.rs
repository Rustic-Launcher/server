//! A plugin without `#[rustic::handlers]`: `export_plugin!` must still compile (the probe falls
//! back to registering nothing).
// With a provider feature on, every plugin must implement the provider traits.
#![cfg(not(any(feature = "provide-permissions", feature = "provide-economy")))]

use rusticmc_api::prelude::*;

#[derive(Default)]
struct Bare;

impl Plugin for Bare {
    fn enable(&mut self, _ctx: &mut Ctx) -> Result<()> {
        Ok(())
    }
}

rusticmc_api::export_plugin!(Bare);

#[test]
fn exports_compile_without_handlers() {
    let _ = rusticmc_api::__rt::Probe::<Bare>::new();
}
