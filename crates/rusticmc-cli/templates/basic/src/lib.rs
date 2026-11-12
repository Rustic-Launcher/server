//! {{name}}: {{description}}
//!
//! * greets joining players with the configurable `greeting`;
//! * protects the blocks listed in `protected_blocks` from being broken;
//! * counts the blocks each player broke (per-player data).

use rustic::config::Config;
use rustic::prelude::*;
use rustic::text::escape_mini;
use serde::{Deserialize, Serialize};

/// `config.toml` (the shipped file and these defaults must agree: a test checks it).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct Settings {
    /// MiniMessage-like text; `{name}` is the player's name.
    greeting: String,
    /// Block ids nobody may break.
    protected_blocks: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            greeting: "<green>Welcome to the server, <bold>{name}</bold>!".into(),
            protected_blocks: vec!["minecraft:diamond_ore".into()],
        }
    }
}

#[rustic::plugin]
#[derive(Default)]
struct {{struct_name}} {
    settings: Settings,
}

impl Plugin for {{struct_name}} {
    fn enable(&mut self, ctx: &mut Ctx) -> Result<()> {
        self.settings = Config::new().version(1).load()?;
        if ctx.is_main() {
            info!(
                "{{name}} enabled ({} protected blocks)",
                self.settings.protected_blocks.len()
            );
        }
        Ok(())
    }
}

#[rustic::handlers]
impl {{struct_name}} {
    #[event_handler]
    fn on_join(&mut self, event: &mut PlayerJoinEvent) {
        let line = self.settings.greeting.replace("{name}", &escape_mini(event.name()));
        event.player().send_message(Text::mini(&line));
    }

    #[event_handler(priority = "high", ignore_cancelled = true)]
    fn on_break(&mut self, event: &mut BlockBreakEvent) -> Result<()> {
        if self.settings.protected_blocks.contains(&event.block().id) {
            event.cancel();
            event
                .player()
                .send_message(Text::mini("<red>This block is protected."));
            return Ok(());
        }
        let data = event.player().data();
        let broken: u64 = data.get_or_default("blocks_broken")?;
        data.set("blocks_broken", &(broken + 1))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_config_matches_the_defaults() {
        let (settings, rewrite) = Config::new()
            .version(1)
            .load_from_str::<Settings>(Some(include_str!("../config.toml")))
            .unwrap();
        assert_eq!(rewrite, None);
        assert_eq!(settings.greeting, Settings::default().greeting);
        assert_eq!(settings.protected_blocks, Settings::default().protected_blocks);
    }
}
