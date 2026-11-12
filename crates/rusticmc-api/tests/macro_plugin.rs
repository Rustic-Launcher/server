//! The attribute macros: `#[plugin]`, `#[handlers]` with `#[event_handler]`, `#[command]` and
//! `#[completer]` (compile-time check plus the generated registration count).
// With a provider feature on, every plugin must implement the provider traits.
#![cfg(not(any(feature = "provide-permissions", feature = "provide-economy")))]

use rusticmc_api::__rt::HandlerSet;
use rusticmc_api::prelude::*;

#[rusticmc_api::plugin]
#[derive(Default)]
struct Demo {
    seen: u32,
}

impl Plugin for Demo {
    fn enable(&mut self, _ctx: &mut Ctx) -> Result<()> {
        Ok(())
    }
}

#[rusticmc_api::handlers]
impl Demo {
    #[event_handler]
    fn on_join(&mut self, _event: &mut PlayerJoinEvent) {
        self.seen += 1;
    }

    #[event_handler(priority = "monitor")]
    fn on_quit(&self, _event: &PlayerQuitEvent) -> Result<()> {
        Ok(())
    }

    #[event_handler(custom = "demo:ping", ignore_cancelled)]
    fn on_ping(&mut self, event: &mut CustomEvent) {
        event.cancel();
    }

    #[command(name = "demo", aliases = ["d", "dm"], usage = ["", "<n:integer(1..9)>"], players_only)]
    fn demo(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        let _n: Option<i64> = cmd.opt_arg("n")?;
        Ok(())
    }

    #[command(usage = "<who:word*>")]
    fn wave(&mut self, _cmd: &mut CommandCtx) {}

    #[completer(command = "wave")]
    fn wave_names(&mut self, ctx: &CompleteCtx) -> Vec<String> {
        ctx.suggest(["alice", "bob"])
    }

    /// Not a handler: left alone.
    fn helper(&self) -> u32 {
        self.seen
    }
}

#[test]
fn handlers_are_collected() {
    assert_eq!(<Demo as HandlerSet>::COUNT, 5);
    assert_eq!(Demo::default().helper(), 0);
}
