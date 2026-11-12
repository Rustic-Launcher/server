//! Test fixture: a plugin that does nothing (idle memory and instantiation measurements).

use rustic::prelude::*;

#[rustic::plugin]
#[derive(Default)]
struct Noop;

impl Plugin for Noop {
    fn enable(&mut self, _ctx: &mut Ctx) -> Result<()> {
        Ok(())
    }
}
