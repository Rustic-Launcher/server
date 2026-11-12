# Rustic

**A plugin platform for [Horizon](https://github.com/horizon-rust/horizon) servers**: what Bukkit,
Spigot and Paper are to the Java server, rebuilt for Horizon's Rust server with sandboxed
WebAssembly plugins.

- **`rustic-server`**: Horizon's dedicated server with the Rustic layer built in (event bus,
  plugin loader, commands, permissions, scheduler, timings). Vanilla 26.3 clients and Horizon
  clients both join it.
- **The Rustic API** (`rustic:api`, WIT): a stable, versioned plugin API. Plugins are WebAssembly
  components, so a plugin that panics, loops forever or eats memory is disabled and the server
  keeps running. Rust is the first-class language (SDK + macros); any language that targets
  WASI 0.2 components can implement the same WIT.
- **Tooling**: `rustic new | build | check | test | run`, a MockBukkit-style test harness, docs.
- **Rustic Bridge** (optional, GPLv3): runs a subset of existing Bukkit/Paper plugins.

> Unofficial. Not affiliated with Mojang, Microsoft, the Bukkit/Spigot/Paper projects or Horizon.
> No game code or assets are included: Horizon prepares its files from your own copy of the game.

## A plugin in 20 lines

```rust
use rusticmc_api::prelude::*;

#[plugin]
#[derive(Default)]
struct Hello;

impl Plugin for Hello {
    fn enable(&mut self, ctx: &mut Ctx) -> Result<()> {
        ctx.command(Command::new("hello").usage("[player:player]").executor(|cmd| {
            let target = cmd.arg::<Player>("player").or_else(|_| cmd.sender_player())?;
            target.send_message(Text::parse("<gold>Hello, <bold>{}</bold>!", target.name()));
            Ok(())
        }))?;
        Ok(())
    }

    #[event_handler]
    fn on_join(&mut self, event: &mut PlayerJoinEvent) {
        event.set_join_message(Some(Text::parse("<green>+ {}", event.name())));
    }
}
```

See [docs/guides/first-plugin.md](docs/guides/first-plugin.md) ("your first plugin in 10 minutes").

## Repository

| Path | What |
|---|---|
| [`crates/rusticmc-api-wit/wit`](crates/rusticmc-api-wit/wit) | The API contract (`rustic:api@0.1.0`, `rustic:services@0.1.0`) |
| [`crates/rusticmc-core`](crates/rusticmc-core) | Runtime: plugin format, lifecycle, event bus, scheduler, commands, permissions, services, storage |
| [`crates/rusticmc-host`](crates/rusticmc-host) | Wasmtime component host: sandbox, limits, hot reload |
| [`crates/rusticmc-api`](crates/rusticmc-api) | Rust SDK for plugins |
| [`crates/rusticmc-test`](crates/rusticmc-test) | Test harness: simulated server and fake players |
| [`crates/rusticmc-cli`](crates/rusticmc-cli) | The `rustic` command |
| [`crates/rusticmc-server`](crates/rusticmc-server) | The `rustic-server` executable (Horizon + Rustic) |
| [`plugins/`](plugins) | Official plugins: essentials-lite, perms, chat-format, menus, mini-game |
| [`bridge/`](bridge) | Rustic Bridge: Bukkit plugin analyzer and runtime spike |
| [`patches/`](patches) | Minimal patches on Horizon (`scripts/patches.py`) |
| [`docs/`](docs) | Architecture, API, plugin format, performance, decisions (ADRs) |

## Build

Needs Rust 1.95+ with the `wasm32-wasip2` target.

```sh
git clone --recurse-submodules https://github.com/Rustic-Launcher/server rustic && cd rustic
cargo build --workspace                          # core, host, SDK, CLI, test harness
cargo run -p rusticmc-cli -- build plugins/hello # a plugin -> target/rustic/hello.rplugin
cargo test --workspace
```

The server links Horizon and is its own Cargo workspace (it must use Horizon's lockfile and
patched crates): see [crates/rusticmc-server/README.md](crates/rusticmc-server/README.md).

## Related repositories

- [Rustic-Launcher/launcher](https://github.com/Rustic-Launcher/launcher): Rustic Launcher, the desktop launcher that creates
  and manages Rustic Server servers and installs their `.rplugin` plugins, plus the Rustic edition
  client.
- The website, the Rustic Cloud services and the deployment are private repositories of the
  organisation.

## Horizon submodule

`horizon/` is a git submodule pinned to an upstream commit of
[horizon-rust/horizon](https://github.com/horizon-rust/horizon) (`git submodule update --init`).
Rustic's minimal changes to Horizon are not committed into the submodule: they live in
[`patches/`](patches) and are applied with `python scripts/patches.py apply` (`check` verifies they
apply, `rebuild` regenerates them from a patched checkout; see [docs/PATCHES.md](docs/PATCHES.md)).
Everything except `crates/rusticmc-server` (and the Rustic edition client in the launcher
repository) builds without the submodule.

## Status

Pre-release, under active development. This repository is published from the project's main
development tree; changes arrive as sync commits. Issues and pull requests are welcome.

## Licence

MIT OR Apache-2.0 ([ADR 0006](docs/DECISIONS/0006-licensing.md)); `bridge/java` is GPL-3.0-only.
