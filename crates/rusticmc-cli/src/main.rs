//! `rustic`: the command-line tool of the Rustic plugin platform.
//!
//! ```text
//! rustic new <id> [--template basic|command|gui|service]   a plugin crate from a template
//! rustic build [path] [--release]                         compile, validate, package .rplugin
//! rustic check [path|file.rplugin]                        manifest, API, capabilities, imports
//! rustic test [path] [-- cargo test args]                  the plugin's tests (rusticmc-test)
//! rustic run [path] [--watch] [--server DIR]               dev server, or a headless REPL
//! rustic package [path] [--out FILE]                       release build ready to upload
//! rustic info [path|file.rplugin]                          what a plugin contains
//! rustic bridge analyze <jar|dir>...                       Bukkit plugin compatibility report
//! ```
//!
//! Reference: `docs/guides/cli.md`.

mod commands;
mod component;
mod new;
mod project;
#[cfg(feature = "wasm")]
mod repl;
mod run;
mod ui;

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser)]
#[command(
    name = "rustic",
    version,
    about = "Create, build, check, test, run and package Rustic plugins",
    long_about = "Create, build, check, test, run and package Rustic plugins (WebAssembly \
                  components for the Rustic plugin platform on Horizon).\n\n\
                  Start with `rustic new my-plugin`, then `rustic build`, `rustic test` and `rustic run`.",
    after_help = "Guide: docs/guides/first-plugin.md · Reference: docs/guides/cli.md"
)]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Create a plugin crate from a template.
    New {
        /// Plugin id: [a-z0-9_-]{1,64} (also the crate and directory name).
        id: String,
        /// What to start from.
        #[arg(short, long, value_enum, default_value = "basic")]
        template: new::Template,
        /// Directory to create (default: ./<id>).
        #[arg(long)]
        dir: Option<PathBuf>,
        /// Display name (default: from the id).
        #[arg(long)]
        name: Option<String>,
        /// Author (default: $RUSTIC_AUTHOR, else git's user.name).
        #[arg(long)]
        author: Option<String>,
        /// One-line description.
        #[arg(long)]
        description: Option<String>,
        /// Use the SDK and test harness from this Rustic checkout's `crates/` directory.
        #[arg(long, conflicts_with = "sdk_version")]
        sdk_path: Option<PathBuf>,
        /// Use this registry version of the SDK and harness (e.g. 0.1).
        #[arg(long)]
        sdk_version: Option<String>,
        /// Do not add a `[workspace]` table (the crate joins an existing workspace).
        #[arg(long)]
        workspace_member: bool,
        /// Write into an existing non-empty directory.
        #[arg(long)]
        force: bool,
    },
    /// Compile to wasm32-wasip2, validate the component, package target/rustic/<id>.rplugin.
    Build {
        /// Plugin project (default: the current directory).
        path: Option<PathBuf>,
        /// Optimised build (what you ship).
        #[arg(short, long)]
        release: bool,
    },
    /// Validate a project or a .rplugin: manifest, API version, capabilities, imports, size.
    Check {
        /// Project directory, dev directory or .rplugin (default: the current directory).
        path: Option<PathBuf>,
        /// Fail on warnings too.
        #[arg(long)]
        deny_warnings: bool,
        /// Inspect the release build (the default when there is one: what `rustic package` ships).
        #[arg(long, conflicts_with = "debug")]
        release: bool,
        /// Inspect the debug build.
        #[arg(long)]
        debug: bool,
    },
    /// Run the plugin's tests (`cargo test`; integration tests use rusticmc-test).
    Test {
        /// Plugin project (default: the current directory).
        path: Option<PathBuf>,
        /// Arguments for `cargo test` (after `--`).
        #[arg(last = true)]
        args: Vec<String>,
    },
    /// Try the plugin: a local rustic-server if one is built, else a headless REPL.
    Run {
        /// Plugin project (default: the current directory).
        path: Option<PathBuf>,
        /// Server directory for the dev server (default: target/rustic/dev-server).
        #[arg(long)]
        server: Option<PathBuf>,
        /// The rustic-server executable (default: $RUSTIC_SERVER_BIN, the checkout's build, PATH).
        #[arg(long)]
        server_bin: Option<PathBuf>,
        /// Use the REPL even when a server binary exists.
        #[arg(long)]
        repl: bool,
        /// Rebuild on save and hot-reload the plugin.
        #[arg(short, long)]
        watch: bool,
        /// Optimised build.
        #[arg(long)]
        release: bool,
        /// Accept the Minecraft EULA (https://aka.ms/MinecraftEULA) for the dev server, which
        /// writes `eula=true` in its folder; without it, `rustic run` asks.
        #[arg(long)]
        accept_eula: bool,
        /// Arguments for rustic-server (after `--`).
        #[arg(last = true)]
        server_args: Vec<String>,
    },
    /// Release build, packaged and checksummed, ready to upload.
    Package {
        /// Plugin project (default: the current directory).
        path: Option<PathBuf>,
        /// Also copy the .rplugin here.
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
    /// Show what a plugin contains: manifest, capabilities, imports, exports, sizes.
    Info {
        /// Project directory, dev directory or .rplugin (default: the current directory).
        path: Option<PathBuf>,
    },
    /// Rustic Bridge tools for Bukkit/Spigot/Paper plugins.
    Bridge {
        #[command(subcommand)]
        command: BridgeCmd,
    },
}

#[derive(Subcommand)]
enum BridgeCmd {
    /// Static compatibility report of plugin jars (no JVM): green / yellow / red.
    Analyze {
        /// Jars or directories of jars.
        #[arg(required = true)]
        inputs: Vec<PathBuf>,
        /// Write the full report as JSON.
        #[arg(long)]
        json: Option<PathBuf>,
        /// Write the Markdown report to a file instead of stdout.
        #[arg(long)]
        markdown: Option<PathBuf>,
        /// Coverage table to use instead of the built-in one.
        #[arg(long)]
        coverage: Option<PathBuf>,
        /// panel.json (names, versions, licences) from bridge/panel/fetch.py.
        #[arg(long)]
        panel: Option<PathBuf>,
        /// A section per plugin.
        #[arg(long)]
        details: bool,
        /// In the JSON, keep only the N most used uncovered members per plugin.
        #[arg(long)]
        compact: Option<usize>,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Cmd::New {
            id,
            template,
            dir,
            name,
            author,
            description,
            sdk_path,
            sdk_version,
            workspace_member,
            force,
        } => new::run(new::NewOptions {
            id,
            template,
            dir,
            name,
            author,
            description,
            sdk: sdk_path
                .map(new::Sdk::Path)
                .or(sdk_version.map(new::Sdk::Version)),
            workspace_member,
            force,
        }),
        Cmd::Build { path, release } => {
            commands::build(path.as_deref(), release, false).map(|_| ())
        }
        Cmd::Check {
            path,
            deny_warnings,
            release,
            debug,
        } => commands::check(
            path.as_deref(),
            deny_warnings,
            match (release, debug) {
                (true, _) => Some("release"),
                (_, true) => Some("debug"),
                _ => None,
            },
        ),
        Cmd::Test { path, args } => commands::test(path.as_deref(), &args),
        Cmd::Run {
            path,
            server,
            server_bin,
            repl,
            watch,
            release,
            accept_eula,
            server_args,
        } => run::run(run::RunOptions {
            path,
            server,
            server_bin,
            repl,
            watch,
            release,
            accept_eula,
            server_args,
        }),
        Cmd::Package { path, out } => commands::package(path.as_deref(), out.as_deref()),
        Cmd::Info { path } => commands::info(path.as_deref()),
        Cmd::Bridge {
            command:
                BridgeCmd::Analyze {
                    inputs,
                    json,
                    markdown,
                    coverage,
                    panel,
                    details,
                    compact,
                },
        } => commands::bridge_analyze(commands::BridgeOptions {
            inputs,
            json,
            markdown,
            coverage,
            panel,
            details,
            compact,
        }),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            ui::error(format!("{e:#}"));
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn cli_definition_is_consistent() {
        use clap::CommandFactory;
        super::Cli::command().debug_assert();
    }
}
