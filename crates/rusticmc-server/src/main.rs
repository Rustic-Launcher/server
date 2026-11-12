//! `rustic-server`: Horizon's dedicated engine with Rustic linked in as its one trait mod
//! (ADR 0001, docs/ARCHITECTURE.md §1).
//!
//! It is always a dedicated server. Run directly (`rustic-server --resources <prepared>`), it
//! starts itself again with `--dedicated --nogui` appended; run by Horizon's supervisor
//! (`horizon-server` with `engine.path` pointing here), it gets exactly the supervisor's command
//! line (`--resources R --control-socket P [--transfers T] --dedicated --nogui`) and passes it on
//! untouched to `horizon_client::entry::main_with`, which reads `std::env::args`.
#![forbid(unsafe_code)]

mod backend;
mod commands;
mod convert;
mod demo;
mod hooks;
mod ping;
mod server;
mod ui;

use std::process::{Command, ExitCode};
use std::sync::Arc;

use horizon_client::mod_api::Hooks;

/// The game's allocator, as Horizon's own executable has it (`crates/horizon-client/src/main.rs`).
#[global_allocator]
static ALLOCATOR: horizon_threads::Allocator = horizon_threads::ALLOCATOR;

/// The mod API level this engine serves, kept in the program for the launcher and the supervisor
/// to read without running it. `main` reads it, so the linker keeps it.
static MOD_API_LEVEL: [u8; 27] = horizon_mods::api_level::MARK;

/// The name Horizon's crash reports and `set_mod_on` give Rustic's hooks.
const MOD_NAME: &str = "rustic";

fn main() -> ExitCode {
    std::hint::black_box(&MOD_API_LEVEL);
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    if args.iter().any(|a| a == "--rustic-version") {
        println!("{}", server::version_line());
        return ExitCode::SUCCESS;
    }
    if !args.iter().any(|a| a == "--dedicated") {
        return run_dedicated_again(args);
    }
    horizon_client::mod_api::name_mods(&[MOD_NAME]);
    let hooks: Box<dyn Hooks> = Box::new(hooks::RusticHooks::new());
    horizon_client::entry::main_with(Arc::from([hooks]));
    ExitCode::SUCCESS
}

/// `rustic-server` is only ever a dedicated server: without `--dedicated` the game would open a
/// window. The game reads its arguments from the process, so the server starts itself again with
/// `--dedicated --nogui` after the arguments it was given (game arguments such as `--resources`
/// come before `--dedicated`), sharing this console, and exits with its code.
fn run_dedicated_again(args: Vec<std::ffi::OsString>) -> ExitCode {
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(e) => {
            eprintln!("rustic-server: cannot find its own executable: {e}");
            return ExitCode::FAILURE;
        }
    };
    let mut command = Command::new(&exe);
    command.args(&args);
    if !args.iter().any(|a| a == "--resources")
        && let Some(resources) = default_resources(&exe)
    {
        command.arg("--resources").arg(resources);
    }
    command.arg("--dedicated");
    if !args.iter().any(|a| a == "--nogui") {
        command.arg("--nogui");
    }
    match command.status() {
        Ok(status) => ExitCode::from(status.code().unwrap_or(1).clamp(0, 255) as u8),
        Err(e) => {
            eprintln!("rustic-server: cannot start the dedicated server: {e}");
            ExitCode::FAILURE
        }
    }
}

/// The prepared game resources when the command line names none: `RUSTIC_RESOURCES`, else what
/// Horizon itself looks for (`resources/` beside the executable, `.local/horizon-client/26.3` in
/// the working directory), else the checkout this binary was built from
/// (`horizon/.local/horizon-client/26.3`, where docs/horizon/run.md prepares them), so
/// `rustic run` works from a plugin's folder. None lets Horizon report what is missing.
fn default_resources(exe: &std::path::Path) -> Option<std::path::PathBuf> {
    if let Some(dir) = std::env::var_os("RUSTIC_RESOURCES") {
        return Some(dir.into());
    }
    let marker = "horizon-resources.json";
    let beside = exe.parent()?.join("resources");
    let local = std::path::Path::new(".local/horizon-client/26.3");
    if beside.join(marker).is_file() || local.join(marker).is_file() {
        return None;
    }
    let checkout = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../horizon/.local/horizon-client/26.3");
    checkout
        .join(marker)
        .is_file()
        .then(|| std::path::absolute(&checkout).unwrap_or(checkout))
}
