//! The core's own `/rustic` command (permission `rustic.admin`, default op):
//!
//! ```text
//! /rustic                       help
//! /rustic plugins               list plugins with their state
//! /rustic info <id>             details of a plugin
//! /rustic enable <id>           enable (re-instantiating if needed)
//! /rustic disable <id>          disable (dependents first)
//! /rustic reload <id>           disable, re-read from disk, enable (dependents re-enabled)
//! /rustic timings [on|off|reset|report|export]
//! /rustic version
//! ```

use crate::commands::ParsedMatch;
use crate::commands::build::{command, dyn_arg, lit, usage};
use crate::id::PluginId;
use crate::lifecycle::PluginState;
use crate::manifest::HOST_API;
use crate::model::{ArgKind, ArgValue, CommandSender, CompleteContext, PermissionDefault, Text};
use crate::runtime::{CORE_VERSION, CommandOutcome, Rustic};

/// The admin permission.
pub const ADMIN_PERMISSION: &str = "rustic.admin";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Action {
    Help,
    Plugins,
    Info,
    Enable,
    Disable,
    Reload,
    Timings,
    TimingsOn,
    TimingsOff,
    TimingsReset,
    TimingsReport,
    TimingsExport,
    Version,
}

const ACTIONS: [Action; 13] = [
    Action::Help,
    Action::Plugins,
    Action::Info,
    Action::Enable,
    Action::Disable,
    Action::Reload,
    Action::Timings,
    Action::TimingsOn,
    Action::TimingsOff,
    Action::TimingsReset,
    Action::TimingsReport,
    Action::TimingsExport,
    Action::Version,
];

fn spec() -> crate::model::CommandSpec {
    let mut c = command(
        "rustic",
        vec![
            usage(vec![]),
            usage(vec![lit("plugins")]),
            usage(vec![lit("info"), dyn_arg("id", ArgKind::Word)]),
            usage(vec![lit("enable"), dyn_arg("id", ArgKind::Word)]),
            usage(vec![lit("disable"), dyn_arg("id", ArgKind::Word)]),
            usage(vec![lit("reload"), dyn_arg("id", ArgKind::Word)]),
            usage(vec![lit("timings")]),
            usage(vec![lit("timings"), lit("on")]),
            usage(vec![lit("timings"), lit("off")]),
            usage(vec![lit("timings"), lit("reset")]),
            usage(vec![lit("timings"), lit("report")]),
            usage(vec![lit("timings"), lit("export")]),
            usage(vec![lit("version")]),
        ],
    );
    c.description = "Manage Rustic plugins".into();
    c.permission = Some(ADMIN_PERMISSION.into());
    c
}

/// A text made of (text, colour) parts.
fn parts(list: &[(&str, Option<&str>)]) -> Text {
    let extra: Vec<serde_json::Value> = list
        .iter()
        .map(|(t, c)| {
            let mut m = serde_json::Map::new();
            m.insert("text".into(), serde_json::Value::String((*t).to_owned()));
            if let Some(c) = c {
                m.insert("color".into(), serde_json::Value::String((*c).to_owned()));
            }
            serde_json::Value::Object(m)
        })
        .collect();
    let mut root = serde_json::Map::new();
    root.insert("text".into(), serde_json::Value::String(String::new()));
    root.insert("extra".into(), serde_json::Value::Array(extra));
    Text::from_json_unchecked(serde_json::Value::Object(root).to_string())
}

fn state_color(s: PluginState) -> &'static str {
    match s {
        PluginState::Enabled => "green",
        PluginState::Loaded | PluginState::Validated | PluginState::Discovered => "yellow",
        PluginState::Disabling | PluginState::Disabled => "gray",
        PluginState::Failed => "red",
    }
}

impl Rustic {
    pub(crate) fn register_builtins(&self) {
        let core = PluginId::core();
        let id = self.next_command_id();
        if let Some(Err(e)) = self.commands_mut(|c| c.register(&core, id, spec())) {
            log::error!(target: "rustic", "cannot register /rustic: {e}");
        }
        self.with_permissions(|p| {
            if let Err(e) = p.declare(
                &core,
                ADMIN_PERMISSION,
                PermissionDefault::Op,
                "Use /rustic (manage plugins, timings)",
                &[],
            ) {
                log::error!(target: "rustic", "cannot declare {ADMIN_PERMISSION}: {e}");
            }
        });
    }

    fn say(&self, sender: CommandSender, text: &Text) {
        let r = crate::context::backend_call(|b| match sender {
            CommandSender::Console => b.console_message(text),
            CommandSender::Player(p) => b.players_send_message(p, text),
        });
        if r.is_err() {
            log::info!(target: "rustic", "{}", text.to_plain());
        }
    }

    pub(crate) fn builtin_execute(
        &self,
        sender: CommandSender,
        label: &str,
        m: ParsedMatch,
    ) -> CommandOutcome {
        let action = ACTIONS
            .get(m.usage as usize)
            .copied()
            .unwrap_or(Action::Help);
        let id = m.args.iter().find_map(|(n, v)| match (n.as_str(), v) {
            ("id", ArgValue::Word(w)) => Some(w.to_ascii_lowercase()),
            _ => None,
        });
        let ok = |s: &str| Text::colored(s, "green");
        let fail = |s: &str| {
            self.say(sender, &Text::error(s));
            CommandOutcome::PluginError(Text::plain(s))
        };
        match action {
            Action::Help => {
                let mut msg = format!("Rustic {CORE_VERSION}. Usage:");
                for line in [
                    "plugins",
                    "info <id>",
                    "enable <id>",
                    "disable <id>",
                    "reload <id>",
                    "timings [on|off|reset|report|export]",
                    "version",
                ] {
                    msg.push_str(&format!("\n  /{label} {line}"));
                }
                self.say(sender, &Text::colored(msg, "gold"));
            }
            Action::Plugins => {
                let list = self.plugin_slots();
                let mut p: Vec<(String, Option<String>)> =
                    vec![(format!("Plugins ({}): ", list.len()), None)];
                for (i, (pid, state)) in list.iter().enumerate() {
                    if i > 0 {
                        p.push((", ".into(), None));
                    }
                    p.push((pid.to_string(), Some(state_color(*state).into())));
                }
                if list.is_empty() {
                    p.push(("none".into(), Some("gray".into())));
                }
                let refs: Vec<(&str, Option<&str>)> =
                    p.iter().map(|(t, c)| (t.as_str(), c.as_deref())).collect();
                self.say(sender, &parts(&refs));
                for e in self.discovery_errors() {
                    self.say(
                        sender,
                        &Text::error(format!("{}: {}", e.path.display(), e.message)),
                    );
                }
            }
            Action::Info => {
                let Some(id) = id else {
                    return fail("missing plugin id");
                };
                let Some(info) = self.plugin(&id) else {
                    return fail(&format!("Unknown plugin `{id}`."));
                };
                let (listeners, commands, (tasks, async_tasks)) = self.counts_for(&id);
                let mut lines = vec![
                    format!(
                        "{} {} ({}) — {}",
                        info.name, info.version, info.id, info.state
                    ),
                    format!(
                        "Authors: {}; licence: {}",
                        info.authors.join(", "),
                        info.license
                    ),
                ];
                if let Some(d) = &info.description {
                    lines.push(d.clone());
                }
                if !info.required.is_empty() {
                    lines.push(format!("Requires: {}", info.required.join(", ")));
                }
                if !info.optional.is_empty() {
                    lines.push(format!("Optional: {}", info.optional.join(", ")));
                }
                if let Some(src) = &info.source {
                    lines.push(format!("Source: {}", src.display()));
                }
                lines.push(format!(
                    "Listeners: {listeners}, commands: {commands}, tasks: {tasks} sync / {async_tasks} async"
                ));
                if let Some(f) = &info.failure {
                    lines.push(format!("Last failure / reason: {f}"));
                }
                self.say(
                    sender,
                    &Text::colored(lines.join("\n"), state_color(info.state)),
                );
            }
            Action::Enable | Action::Disable | Action::Reload => {
                let Some(id) = id else {
                    return fail("missing plugin id");
                };
                let r = match action {
                    Action::Enable => self
                        .enable_by_command(&id)
                        .map(|()| vec![PluginId::new_unchecked(&id)]),
                    Action::Disable => self.disable_by_command(&id),
                    _ => self.reload_by_command(&id),
                };
                match r {
                    Ok(list) => {
                        let names: Vec<&str> = list.iter().map(|p| p.as_str()).collect();
                        let verb = match action {
                            Action::Enable => "Enabled",
                            Action::Disable => "Disabled",
                            _ => "Reloaded",
                        };
                        self.say(sender, &ok(&format!("{verb}: {}", names.join(", "))));
                    }
                    Err(e) => return fail(&e.to_string()),
                }
            }
            Action::Timings | Action::TimingsReport => {
                let report = self.timings().report();
                self.say(sender, &Text::plain(report.to_text(8)));
            }
            Action::TimingsOn => {
                self.timings().set_enabled(true);
                self.say(sender, &ok("Timings on."));
            }
            Action::TimingsOff => {
                self.timings().set_enabled(false);
                self.say(sender, &ok("Timings off."));
            }
            Action::TimingsReset => {
                self.timings().reset();
                self.say(sender, &ok("Timings reset."));
            }
            Action::TimingsExport => match self.timings().export_json(&self.layout().timings_dir())
            {
                Ok(path) => self.say(
                    sender,
                    &ok(&format!("Timings written to {}", path.display())),
                ),
                Err(e) => return fail(&format!("Cannot write timings: {e}")),
            },
            Action::Version => {
                let engine = crate::context::backend_call(|b| b.server_version()).ok();
                let mut msg = format!("Rustic {CORE_VERSION} (plugin API {HOST_API})");
                if let Some(e) = engine {
                    msg.push_str(&format!("\n{e}"));
                }
                self.say(sender, &Text::colored(msg, "gold"));
            }
        }
        CommandOutcome::Executed
    }

    pub(crate) fn builtin_complete(&self, ctx: &CompleteContext) -> Vec<String> {
        if ctx.argument != "id" {
            return Vec::new();
        }
        let partial = ctx.partial.to_ascii_lowercase();
        self.plugin_slots()
            .into_iter()
            .map(|(id, _)| id.to_string())
            .filter(|id| id.starts_with(&partial))
            .collect()
    }
}
