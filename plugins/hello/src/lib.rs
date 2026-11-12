//! Hello: the minimal Rustic example plugin.
//!
//! * welcomes joining players with a hoverable, clickable message and counts joins in storage;
//! * cancels chat messages containing a banned word (from `config.toml`);
//! * `/hello [target]` greets you or another player, with tab completion of online names;
//! * every second (20 ticks) shows an uptime / join counter in everyone's action bar.

use rustic::config::Config;
use rustic::permissions::Permissions;
use rustic::prelude::*;
use rustic::text::escape_mini;
use serde::{Deserialize, Serialize};

/// `config.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
struct HelloConfig {
    welcome: String,
    banned_words: Vec<String>,
    action_bar: String,
}

impl Default for HelloConfig {
    fn default() -> Self {
        HelloConfig {
            welcome: "<gold>Welcome, <bold>{name}</bold>!</gold>".into(),
            banned_words: vec!["heck".into(), "darn".into()],
            action_bar: "<gray>Online <white>{seconds}s</white> · joins <white>{joins}</white>"
                .into(),
        }
    }
}

const JOINS_KEY: &str = "joins";
const DEFAULT_CONFIG: &str = include_str!("../config.toml");

#[rustic::plugin]
#[derive(Default)]
struct Hello {
    config: HelloConfig,
    seconds: u64,
    joins: u64,
}

impl Plugin for Hello {
    fn load(&mut self, _ctx: &mut Ctx) -> Result<()> {
        Permissions::declare("hello.use")
            .default(PermissionDefault::Allow)
            .description("Use /hello")
            .register();
        Ok(())
    }

    fn enable(&mut self, ctx: &mut Ctx) -> Result<()> {
        // The packaged config.toml gives the defaults and the current `config_version`;
        // `.migration(|from, table| ..)` would upgrade older admin files.
        self.config = Config::new().defaults(DEFAULT_CONFIG).load()?;
        self.joins = Storage::get_or_default(JOINS_KEY)?;

        // A repeating main-thread task: the closure reaches the plugin state with `with_plugin`.
        Scheduler::run_timer(20, 20, |_task| with_plugin(Hello::tick));

        if ctx.is_main() {
            info!(
                "Hello enabled: {} banned words, {} joins so far",
                self.config.banned_words.len(),
                self.joins
            );
        }
        Ok(())
    }

    fn disable(&mut self, _ctx: &mut Ctx) {
        info!("Hello disabled after {}s", self.seconds);
    }
}

impl Hello {
    fn tick(&mut self) {
        self.seconds += 1;
        let line = self
            .config
            .action_bar
            .replace("{seconds}", &self.seconds.to_string())
            .replace("{joins}", &self.joins.to_string());
        let text = Text::mini(&line);
        for player in Player::online() {
            player.action_bar(text.clone());
        }
    }

    fn is_banned(&self, message: &str) -> bool {
        let message = message.to_lowercase();
        self.config
            .banned_words
            .iter()
            .any(|w| !w.is_empty() && message.contains(&w.to_lowercase()))
    }
}

#[rustic::handlers]
impl Hello {
    #[event_handler]
    fn on_join(&mut self, event: &mut PlayerJoinEvent) -> Result<()> {
        self.joins = Storage::update(JOINS_KEY, |n: &mut u64| *n += 1)?;
        let name = escape_mini(event.name());

        let welcome = Text::mini(&self.config.welcome.replace("{name}", &name))
            .append(" ")
            .append(
                Text::new("[say hi]")
                    .color(NamedColor::Aqua)
                    .hover(Text::mini(
                        "<gray>Click to greet someone with <white>/hello",
                    ))
                    .suggest_command("/hello "),
            );
        event.player().send_message(welcome);
        event.set_join_message(Text::mini(&format!(
            "<yellow>{name} joined <dark_gray>(join #{})",
            self.joins
        )));
        Ok(())
    }

    #[event_handler(priority = "high", ignore_cancelled = true)]
    fn on_chat(&mut self, event: &mut PlayerChatEvent) {
        if self.is_banned(event.message()) {
            event.cancel();
            event
                .player()
                .send_message(Text::mini("<red>Your message contains a banned word."));
        }
    }

    #[command(
        name = "hello",
        aliases = ["hi"],
        permission = "hello.use",
        usage = "[target:word*]",
        description = "Say hello to yourself or to another player"
    )]
    fn hello(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        let from = cmd.sender().name();
        match cmd.opt_arg::<String>("target")? {
            None => cmd.reply(Text::mini(&format!(
                "<green>Hello, {}!",
                escape_mini(&from)
            ))),
            Some(target) => {
                let player = Player::by_name(&target).ok_or("That player is not online.")?;
                player.send_message(
                    Text::mini(&format!("<aqua>{} says hello!", escape_mini(&from)))
                        .hover(Text::new("Click to answer"))
                        .suggest_command(format!("/hello {from}")),
                );
                player.sound("minecraft:entity.experience_orb.pickup", 1.0, 1.2);
                cmd.reply(Text::mini(&format!(
                    "<green>Said hello to {}.",
                    escape_mini(&target)
                )));
            }
        }
        Ok(())
    }

    #[completer("hello")]
    fn complete_hello(&mut self, ctx: &CompleteCtx) -> Vec<String> {
        ctx.suggest(Player::online().iter().filter_map(Player::name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banned_words_are_case_insensitive() {
        let h = Hello::default();
        assert!(h.is_banned("what the HECK"));
        assert!(!h.is_banned("hello there"));
    }

    #[test]
    fn shipped_config_matches_defaults() {
        let (cfg, rewrite) = Config::new()
            .defaults(DEFAULT_CONFIG)
            .load_from_str::<HelloConfig>(Some(DEFAULT_CONFIG))
            .unwrap();
        assert_eq!(rewrite, None);
        assert_eq!(cfg.banned_words, HelloConfig::default().banned_words);
        assert_eq!(cfg.welcome, HelloConfig::default().welcome);
        assert_eq!(cfg.action_bar, HelloConfig::default().action_bar);
    }
}
