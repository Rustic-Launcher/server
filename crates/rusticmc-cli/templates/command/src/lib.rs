//! {{name}}: {{description}}
//!
//! * `/heal [target]` restores health and food (`{{id}}.heal`, `{{id}}.heal.others`);
//! * `/feed` restores food only (`{{id}}.feed`);
//! * a cooldown (from `config.toml`) between two heals of the same player.

use rustic::config::Config;
use rustic::permissions::Permissions;
use rustic::prelude::*;
use rustic::text::escape_mini;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct Settings {
    /// Ticks between two heals of the same player (20 ticks = 1 second).
    cooldown_ticks: u64,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { cooldown_ticks: 600 }
    }
}

#[rustic::plugin]
#[derive(Default)]
struct {{struct_name}} {
    settings: Settings,
    /// Player → tick of their last heal.
    last_heal: HashMap<Uuid, u64>,
}

impl Plugin for {{struct_name}} {
    fn load(&mut self, _ctx: &mut Ctx) -> Result<()> {
        Permissions::declare("{{id}}.heal")
            .default(PermissionDefault::Op)
            .description("Use /heal on yourself")
            .register();
        Permissions::declare("{{id}}.heal.others")
            .default(PermissionDefault::Op)
            .description("Use /heal on other players")
            .register();
        Permissions::declare("{{id}}.feed")
            .default(PermissionDefault::Allow)
            .description("Use /feed")
            .register();
        Ok(())
    }

    fn enable(&mut self, _ctx: &mut Ctx) -> Result<()> {
        self.settings = Config::new().version(1).load()?;
        Ok(())
    }
}

#[rustic::handlers]
impl {{struct_name}} {
    #[command(
        name = "heal",
        permission = "{{id}}.heal",
        usage = "[target:player]",
        description = "Restore health and food"
    )]
    fn heal(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        let target: Player = match cmd.opt_arg::<Player>("target")? {
            Some(other) => {
                if !cmd.sender().has_permission("{{id}}.heal.others") {
                    return Err("You may only heal yourself.".into());
                }
                other
            }
            None => cmd.player()?,
        };
        let now = Scheduler::current_tick();
        if let Some(last) = self.last_heal.get(&target.uuid())
            && now < last + self.settings.cooldown_ticks
        {
            let seconds = (last + self.settings.cooldown_ticks - now).div_ceil(20);
            return Err(format!("Wait {seconds}s before healing again.").into());
        }
        target.set_health(target.max_health()?)?;
        target.set_food_level(20)?;
        self.last_heal.insert(target.uuid(), now);
        let name = escape_mini(&target.name().unwrap_or_default());
        target.send_message(Text::mini("<green>You have been healed."));
        if cmd.sender().as_player() != Some(target) {
            cmd.reply(Text::mini(&format!("<green>Healed {name}.")));
        }
        Ok(())
    }

    #[command(name = "feed", permission = "{{id}}.feed", description = "Restore food")]
    fn feed(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        let player = cmd.player()?;
        player.set_food_level(20)?;
        cmd.reply(Text::mini("<green>Your hunger is gone."));
        Ok(())
    }
}
