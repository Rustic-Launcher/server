//! Test fixture for the host benchmarks (`crates/rusticmc-host/benches`):
//!
//! * no-op `player-join` and `player-move` handlers (event dispatch cost);
//! * custom event `<id>:names` whose payload is a little-endian `u32` n: calls `players.name`
//!   n times (host import round trip);
//! * `/bench-noop` (command dispatch cost).

use rustic::prelude::*;

#[rustic::plugin]
#[derive(Default)]
struct Bench;

impl Plugin for Bench {
    fn enable(&mut self, ctx: &mut Ctx) -> Result<()> {
        ctx.on(|_e: &mut PlayerJoinEvent| {})?;
        ctx.on(|_e: &mut PlayerMoveEvent| {})?;
        let names = format!("{}:names", ctx.plugin_id());
        ctx.on_custom(&names, |e: &mut CustomEvent| {
            let mut n = [0u8; 4];
            let p = e.payload();
            n[..p.len().min(4)].copy_from_slice(&p[..p.len().min(4)]);
            let player = Player::from_uuid(Uuid { hi: 0, lo: 1 });
            for _ in 0..u32::from_le_bytes(n) {
                std::hint::black_box(player.name());
            }
        })?;
        ctx.command(Command::new("bench-noop").executor(|_cmd| Ok(())))?;
        Ok(())
    }
}
