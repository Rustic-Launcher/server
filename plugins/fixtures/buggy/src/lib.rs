//! Test fixture: misbehaves on demand, one fault per trigger, so the host's isolation can be
//! tested fault by fault (`crates/rusticmc-host/tests`).
//!
//! * chat message containing `boom`: the `player-chat` handler panics;
//! * `/buggy-panic`: panics in a command;
//! * `/buggy-loop`: loops forever in a command (deadline or fuel);
//! * `/buggy-oom`: schedules a task that allocates until the memory limit;
//! * `/buggy-abort`: aborts (a trap without a panic message);
//! * `/buggy-async-panic`: an async job that panics;
//! * `/buggy-arm`: the next `on-enable` (after a reload) aborts.

use rustic::prelude::*;

#[rustic::plugin]
#[derive(Default)]
struct Buggy;

impl Plugin for Buggy {
    fn enable(&mut self, ctx: &mut Ctx) -> Result<()> {
        if ctx.is_main() && Storage::get::<bool>("trap_on_enable")?.unwrap_or(false) {
            let _ = Storage::delete("trap_on_enable");
            std::process::abort();
        }
        ctx.on(|e: &mut PlayerChatEvent| {
            if e.message().contains("boom") {
                panic!("boom in the chat handler");
            }
        })?;
        ctx.async_task("explode", |_: ()| -> Result<()> {
            panic!("async job exploded")
        });
        ctx.command(Command::new("buggy-panic").executor(|_| panic!("buggy panicked on purpose")))?;
        ctx.command(Command::new("buggy-loop").executor(|_| {
            let mut x = 0u64;
            loop {
                x = std::hint::black_box(x.wrapping_add(1));
            }
        }))?;
        ctx.command(Command::new("buggy-oom").executor(|_| {
            Scheduler::run_later(0, || {
                let mut hoard: Vec<Vec<u8>> = Vec::new();
                loop {
                    hoard.push(std::hint::black_box(vec![7u8; 1 << 20]));
                }
            });
            Ok(())
        }))?;
        ctx.command(Command::new("buggy-abort").executor(|_| std::process::abort()))?;
        ctx.command(Command::new("buggy-async-panic").executor(|_| {
            Scheduler::run_async("explode", &())?;
            Ok(())
        }))?;
        ctx.command(Command::new("buggy-arm").executor(|_| {
            Storage::set("trap_on_enable", &true)?;
            Ok(())
        }))?;
        Ok(())
    }
}
