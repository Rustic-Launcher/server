# Your first plugin in 10 minutes

You will create a plugin that greets players, protects a block and adds a `/spawn` command,
test it against its real WebAssembly build, and try it in a REPL. No game server is needed.
(Every step below was run as written on 2026-11-11; outputs are abridged. Debug builds are
large; `rustic package` makes the small release build.)

## 0. Install (once)

You need Rust (1.95+) and the Rustic checkout.

```sh
rustup target add wasm32-wasip2
cargo install --path crates/rusticmc-cli     # from the Rustic checkout: installs `rustic`
rustic --version
```

## 1. Create the plugin (1 min)

```sh
rustic new greeter
cd greeter
```

```text
     Created plugin `greeter` (basic template) in greeter
```

What you got:

| File | What |
|---|---|
| `rustic.plugin.toml` | the manifest: id, version, API version (`0.1`), entry, capabilities (`filesystem = "data-folder"` so the plugin can read its config) |
| `config.toml` | the default configuration, copied to `plugins/greeter/config.toml` on first start |
| `src/lib.rs` | the plugin: a `Default` struct with `#[rustic::plugin]`, handlers in a `#[rustic::handlers]` impl |
| `tests/plugin.rs` | integration tests that run the real `.wasm` in an in-memory server |
| `Cargo.toml` | a `cdylib` depending on the SDK (`rustic` = `rusticmc-api`) and, for tests, `rusticmc-test` |

Open `src/lib.rs`: `enable` loads `config.toml` into a serde struct, `on_join` greets the
player, `on_break` cancels breaking protected blocks and counts broken blocks per player.

## 2. Build it (1 min)

```sh
rustic build
```

```text
    Building greeter v0.1.0 (wasm32-wasip2, debug)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 31.2s
    Checking component target/wasm32-wasip2/debug/greeter.wasm
    Packaged target/rustic/greeter.rplugin (3.3 MiB, component 14.4 MiB)
```

`rustic build` compiled the crate to a WebAssembly component, checked it against the API the
server implements, and packaged `target/rustic/greeter.rplugin`, the file a server installs.
`rustic check` shows what an admin will see:

```sh
rustic check
```

```text
  Greeter 0.1.0 by You asks to:
    • read and write files in its own data folder (plugins/greeter/)
  ...
     Checked greeter v0.1.0: no problem found
```

## 3. Add a command (2 min)

Add `/spawn` to the `#[rustic::handlers] impl Greeter` block of `src/lib.rs`, after `on_break`:

```rust
    #[command(name = "spawn", description = "Teleport to the spawn")]
    fn spawn(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        let player = cmd.player()?;
        let spawn = World::overworld().spawn_location()?;
        player.teleport(&spawn)?;
        cmd.reply(Text::mini("<green>Welcome back to spawn!"));
        Ok(())
    }
```

`cmd.player()?` refuses the console with a clear message; `?` on SDK calls turns errors into a
red message for the sender. The command is registered when the plugin is enabled and removed
when it is disabled; no manifest entry is needed.

## 4. Test it (2 min)

Add a test at the end of `tests/plugin.rs`:

```rust
#[test]
fn spawn_brings_players_back() {
    let server = server();
    let steve = server.add_player("Steve");
    steve.teleport(server.overworld().location(300.0, 70.0, -40.0));

    assert!(steve.command("/spawn").executed());
    assert_eq!(steve.location().x, server.overworld().spawn().x);
    steve.assert_message_contains("Welcome back to spawn!");
}
```

```sh
rustic test
```

```text
     Running tests\plugin.rs
running 5 tests
test greets_joining_players ... ok
test protected_blocks_stay ... ok
test counts_broken_blocks_per_player ... ok
test admins_can_change_the_greeting ... ok
test spawn_brings_players_back ... ok
test result: ok. 5 passed; 0 failed
```

The first run compiles the test harness and the WebAssembly runtime (a few minutes); later
runs take seconds. Each test builds a fresh in-memory server: `add_player` fires the join
events, `command` runs the command like a player typing it, and the assertions read what the
player received. `docs/guides/testing.md` lists everything the harness can do.

## 5. Try it (2 min)

```sh
rustic run
```

Without a `rustic-server` build, `rustic run` starts a REPL around an in-memory server with
your plugin loaded:

```text
note: no rustic-server binary found (...); starting the headless REPL instead
  [log] [INFO] [greeter] Greeter enabled (1 protected blocks)
Rustic REPL — `greeter` is enabled. In-memory server (rusticmc-test), fake clock. Type `help`.
> join Steve
  [console] Steve joined the game
  Steve chat Welcome to the server, Steve!
  Steve chat Steve joined the game
> tp Steve 100 70 100
> cmd Steve /spawn
  ok
  Steve chat Welcome back to spawn!
> break Steve 0 64 3
  done
> exit
```

Leave it running with `rustic run --watch`: every save rebuilds the plugin and reloads it in
place. With a server build (`crates/rusticmc-server`), `rustic run` starts that server with your
plugin instead, and `--watch` reloads it there with `/rustic reload greeter`.

## 6. Ship it (1 min)

```sh
rustic package
```

```text
    Packaged target/rustic/greeter.rplugin (167.0 KiB, component 439.4 KiB)
    Finished target/rustic/greeter.rplugin ready to upload
20f102d5...  target/rustic/greeter.rplugin
```

Copy the `.rplugin` into a server's `plugins/` folder. On first start the server copies
`config.toml` to `plugins/greeter/config.toml`; edit it there and `/rustic reload greeter`.

## Next

* `docs/API.md`: the SDK guide (events, commands, menus, text, storage, async tasks).
* `rustic new <id> --template command|gui|service` for commented examples of commands with
  permissions, chest menus, and providing a service.
* **Writing a service provider** (economy, permissions, chat meta; `docs/API.md` §3.12): turn on
  the SDK feature (`provide-economy`, `provide-permissions`) and keep the crate in **its own
  Cargo workspace** (a `[workspace]` table; if it sits under another workspace, list it in that
  workspace's `exclude`). The feature selects the component's world and cargo unifies features
  across a workspace, so sharing one with ordinary plugins would turn them all into providers
  (ADR 0032; `plugins/perms` is the example).
* `docs/BUKKIT_MAPPING.md` if you know Bukkit/Paper.
* `docs/guides/cli.md`: every `rustic` command and option.
