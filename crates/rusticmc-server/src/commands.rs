//! Rustic's commands in Horizon's command tree (docs/ARCHITECTURE.md §4).
//!
//! The core merges every plugin's usages and exports them as Horizon usage strings
//! (`Rustic::horizon_usages`): `register_commands` pushes one `NativeCommand` each, with its
//! permission node (which Horizon asks `permission_check`, answered by Rustic's engine). When the
//! core's commands change (a plugin registers, unregisters, is disabled or reloaded) the backend's
//! `commands_changed` calls `host.refresh_commands()`, and Horizon asks again at the next tick and
//! re-sends every player their tree. Execution: Horizon hands the raw line to `perform_command`
//! (players) or `perform_console_command` (console, RCON), which go to `Rustic::dispatch_command`;
//! the core parses the arguments itself and sends error feedback. A plugin command of a vanilla
//! name (`msg`, `tell`, `me`) replaces vanilla's (`NativeCommand::replace_vanilla`), as on Paper.
//! Tab completion of arguments that ask the server (`word?`, `text?`) comes through
//! `tab_complete_v2`.
use horizon_client::mod_api::NativeCommand;
use rusticmc_core::{CommandOutcome, Rustic};

/// One `NativeCommand` per usage the core exports.
pub fn native_commands(rustic: &Rustic) -> Vec<NativeCommand> {
    rustic
        .horizon_usages()
        .into_iter()
        // `<plugin>:<name>` labels: Horizon's command words take no `:` (it would refuse them with
        // a warning); the bare name reaches the same command.
        .filter(|usage| !usage.label.contains(':'))
        .map(|usage| {
            // A plugin's command replaces vanilla's of its name, as a Bukkit plugin's does on
            // Paper (essentials' `/msg`, chat-format's `/me`): Horizon then leaves vanilla's
            // root out of the players' trees and sends their lines of it to Rustic. Without
            // it Horizon would drop the plugin's usage and keep vanilla's.
            let command = NativeCommand::new(&usage.usage).replace_vanilla(true);
            match &usage.permission {
                Some(node) => command.permission(node),
                None => command,
            }
        })
        .collect()
}

/// Whether the core has a command named by `line`'s first word (with or without its slash).
pub fn owns(rustic: &Rustic, line: &str) -> bool {
    let label = line
        .trim_start()
        .trim_start_matches('/')
        .split(' ')
        .next()
        .unwrap_or_default();
    !label.is_empty()
        && rustic
            .horizon_usages()
            .iter()
            .any(|u| u.label.eq_ignore_ascii_case(label))
}

/// Whether Horizon should consider the line handled.
pub fn ran(outcome: &CommandOutcome) -> bool {
    !matches!(outcome, CommandOutcome::UnknownCommand)
}
