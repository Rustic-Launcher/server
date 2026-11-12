# The Rustic plugin API

Rustic plugins are **WebAssembly components** (`wasm32-wasip2`) implementing the WIT world
`rustic:api/rustic-plugin@0.1.0` (source of truth: `crates/rusticmc-api-wit/wit/`). Plugins are
written in Rust with the guest SDK `rusticmc-api` (this document's "SDK guide"); any language with
component-model tooling can target the same world.

1. [Overview](#1-overview)
2. [Versioning policy](#2-versioning-policy)
3. [SDK guide](#3-sdk-guide)

---

## 1. Overview

### The contract

| WIT file | Interfaces |
|---|---|
| `types.wit` | `types`: shared records/enums (`uuid`, `location`, `item-stack`, `block-state`, `host-error`, ...) |
| `events.wit` | `event-types` (one record per event), `events` (subscribe / unsubscribe / call-custom) |
| `core.wit` | `logging`, `commands`, `scheduler`, `permissions`, `services`, `storage`, `http` |
| `game.wit` | `server`, `players`, `inventories`, `menus`, `worlds`, `scoreboards`, `bossbars`, `tablist`, `messaging` |
| `plugin.wit` | exports `plugin` (lifecycle, tasks, commands, messages) and `handlers` (one `handle-<event>` per event); the world |
| `deps/services` | `rustic:services@0.1.0`: standard service interfaces (economy, permission provider, chat meta) |

The host (`rusticmc-host`, wasmtime) instantiates each plugin, links every `rustic:api` import to
the runtime, and calls the two exported instances:

- `rustic:api/plugin@0.1.0`: `on-load`, `on-enable`, `on-disable`, `run-task`, `execute-command`,
  `complete-command`, `on-message`;
- `rustic:api/handlers@0.1.0`: `handle-player-join`, `handle-block-break`, ... (25 in 0.1).

A component imports only the interfaces and functions it actually uses (wit-component drops the
rest), so a host must provide all of them but a plugin pays only for what it calls.

### Concepts in one page

- **Sandbox.** A plugin sees nothing but its imports and a minimal WASI: clocks, random, stdio to
  the log, and its data folder preopened as `/` if the manifest grants it. A trap, panic, runaway
  loop (fuel/epoch) or OOM disables that plugin; the server goes on.
- **Two instances.** Each plugin runs as a *main* instance (on the server tick: events, commands,
  world access) and an *async* instance (off the tick: may block on HTTP and storage, no world
  access). They share no memory. See [3.4](#34-threading-model-main-and-async-instances).
- **Events are values.** A handler receives the event record and returns it; the host applies only
  the fields documented as mutable plus `cancelled`. See [3.5](#35-events).
- **Commands are typed trees** declared from usages (`<target:player> [amount:integer(1..64)]`),
  sent to the client for completion and parsed by the host before the plugin sees them.
- **Capabilities** in the manifest gate sensitive imports (filesystem, network domains).
- **Services** (economy, permission provider, chat meta) are imported by every plugin and
  exported by provider plugins, which target a provider world (`rustic-plugin-economy`, ...).
  See [3.12](#312-services).

---

## 2. Versioning policy

`rustic:api` follows **semantic versioning** on its WIT package version.

### What a minor version (0.x → 0.x+1 before 1.0, 1.x → 1.x+1 after) may do

- **Add interfaces** (a new `import` in the world).
- **Add functions** to an interface, never change or remove existing ones.
- **Add events**: a new record in `event-types` *and* a new `handle-<event>` function in
  `handlers`. The host looks handler exports up **by name** and only calls the ones for events the
  plugin subscribed to, so a plugin built against an older minor simply lacks the newer
  `handle-*` exports and keeps working.
- **Add types** (records, variants, enums) used by the new functions.

### What it may never do

- Change a record, variant, enum or flags **in place** (no new field, no new case, no reorder,
  no rename). The canonical ABI lays them out by position: changing one breaks every compiled
  plugin. If an event needs more data, the minor adds a *new* event record
  (`player-chat-2`) and handler; the old one stays.
- Change a function's parameters or results, or remove anything.

### Breaking changes

A breaking change requires a new **major** version (`rustic:api@2.0.0`). Before 1.0, a breaking
change goes to the **next 0.x minor** instead (0.1 → 0.2 may break), with a documented
**compatibility window**: the host keeps the bindings for the older world for **N releases**
(N = 3 for the 0.x line unless the release notes say otherwise), loads plugins built against it
through an adapter, and logs a deprecation warning naming the plugin. After the window, plugins
declaring the old version are refused with a clear message.

Within one 0.x line (0.1.0, 0.1.1, ...) only additive, compatible changes ship, exactly as for a
minor after 1.0.

### How a plugin declares its API version

The manifest (`rustic.plugin.toml`, format owned by `rusticmc-core`, see `docs/PLUGIN_FORMAT.md`)
says which API it was built against, as `major.minor`:

```toml
[plugin]
id = "hello"
version = "0.1.0"
api = "0.1"
```

The host accepts the plugin when it implements that version or a later compatible one (same
major, or the same 0.x minor line, or an older line still inside the compatibility window). The
SDK crate's version tracks the API: `rusticmc-api 0.1.*` builds plugins for `api = "0.1"`. The
component itself also carries the versioned interface names (`rustic:api/plugin@0.1.0`), which the
host checks against `api`. Today the server implements `0.1` and, per `docs/PLUGIN_FORMAT.md`,
accepts exactly `0.1` while the major is 0; the compatibility window above applies once a 0.2
exists.

The base world `rustic-plugin` has provider variants (`rustic-plugin-permissions`,
`rustic-plugin-economy`, `rustic-plugin-permissions-economy`) that include it and add service
exports; they version together with it.

### Types are frozen

`types.wit` says it explicitly: records and variants shared through `types` are frozen for the
whole 0.x line once 0.1.0 ships. New shared types are added next to them.

---

## 3. SDK guide

### 3.1 Getting started

A plugin is a `cdylib` crate built for `wasm32-wasip2` (`rustup target add wasm32-wasip2`).
Official plugins live in the `plugins/` workspace; `plugins/hello` is the reference example.

```toml
# Cargo.toml
[package]
name = "hello"
version = "0.1.0"
edition = "2024"

[lib]
crate-type = ["cdylib"]

[dependencies]
rustic = { package = "rusticmc-api", path = "../../crates/rusticmc-api" }  # or a registry version
serde = { version = "1", features = ["derive"] }

[profile.release]   # in the workspace root
opt-level = "s"
lto = true
codegen-units = 1
strip = true
panic = "abort"
```

```rust
use rustic::prelude::*;

#[rustic::plugin]
#[derive(Default)]
struct Hello;

impl Plugin for Hello {
    fn enable(&mut self, ctx: &mut Ctx) -> Result<()> {
        ctx.on(|e: &mut PlayerJoinEvent| {
            e.player().send_message(Text::mini("<gold>Welcome!"));
        })?;
        info!("Hello enabled");
        Ok(())
    }
}
```

Build: `cargo build --manifest-path plugins/Cargo.toml -p hello --target wasm32-wasip2 --release`
gives `target/wasm32-wasip2/release/hello.wasm`, a component (check with
`wasm-tools component wit hello.wasm`). The plugin crate never runs `wit-bindgen` itself: the SDK
generates the bindings once and owns the exports.

### 3.2 Anatomy of a plugin

- The **plugin type** is any `Default` struct (or enum) implementing [`Plugin`]:
  `load` (optional: declare permissions, provide services), `enable` (register everything),
  `disable` (optional cleanup). `#[rustic::plugin]` generates the component exports; the SDK keeps
  the single instance in a thread-local cell.
- **Handlers and commands** are registered either with closures (`ctx.on(..)`,
  `Command::new(..)..register()`) or as methods of an impl block marked `#[rustic::handlers]`:

```rust
#[rustic::handlers]
impl Hello {
    #[event_handler(priority = "high", ignore_cancelled = true)]
    fn on_chat(&mut self, event: &mut PlayerChatEvent) {
        if event.message().contains("heck") {
            event.cancel();
        }
    }

    #[command(name = "home", aliases = ["h"], permission = "homes.use", usage = "<name:word*>")]
    fn home(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        let name: String = cmd.arg("name")?;
        cmd.reply(format!("Going home to {name}"));
        Ok(())
    }

    #[completer("home")]
    fn home_names(&mut self, ctx: &CompleteCtx) -> Vec<String> {
        ctx.suggest(["base", "farm"])
    }
}
```

  Methods registered this way are subscribed right after `enable` returns. They get `&mut self`
  (the plugin state); a closure registered with the builders reaches it with
  `with_plugin(|p: &mut Hello| ..)`. Handlers may return `()` or `Result<(), E: Display>` (errors
  are logged); command methods return `CommandResult` or `()`.

- **Logging**: `trace!`, `debug!`, `info!`, `warn!`, `error!` (the host prefixes the plugin id).

### 3.3 The manifest and capabilities

```toml
# rustic.plugin.toml
[plugin]
id = "hello"
name = "Hello"
version = "0.1.0"
api = "0.1"
authors = ["Rustic contributors"]
license = "MIT OR Apache-2.0"
entry = "hello.wasm"

[capabilities]
filesystem = "data-folder"            # preopen the data folder as "/" (std::fs, config.toml)
network = ["api.example.org"]         # http.fetch allowlist (async instance only)
```

Without a capability, the matching calls fail with `host-error::capability-denied(<name>)`
(surfaced as `Error::Host`) and no WASI preopen exists. The full schema (dependencies, limits)
is `docs/PLUGIN_FORMAT.md`; validation is strict (unknown keys are errors).
`plugins/hello/tests/manifest.rs` checks the manifest with the server's own parser
(`rusticmc_core::manifest::Manifest::parse`, as a dev-dependency): do the same in your plugin.

### 3.4 Threading model: main and async instances

Each plugin is **instantiated twice** from the same component:

| | main instance | async instance |
|---|---|---|
| runs | on the server thread, inside the tick | on a worker, off the tick |
| receives | events, commands, completions, `run_later`/`run_timer` tasks, `on-message` | `run_async*` tasks |
| may | touch the world, players, menus, scoreboards | block: HTTP (`network` capability), storage, files |
| may not | block (no HTTP: `Error::AsyncOnly`) | touch the world (`host-error::main-thread-only`) |

They share **no memory**. Consequences, and how the SDK handles them:

1. **`load` and `enable` run in both instances.** The SDK's registration calls (`ctx.on`,
   `ctx.command`, `Command::register`, `Permissions::declare(..).register()`,
   `Messaging::register_incoming`, `Services::provide`, `Scheduler::run_later/run_timer`) reach the
   host **only from the main instance** and are silent no-ops in the async one, so the same
   `enable` is correct in both. Side effects you make yourself (broadcasting, editing the world,
   writing files) run twice unless you guard them with `ctx.is_main()`.
2. **Closures cannot cross instances.** A closure created in the main instance does not exist in
   the async one, so async work is registered **by a stable name** in `enable` (hence in both
   instances) and started by name with a serde payload:

```rust
fn enable(&mut self, ctx: &mut Ctx) -> Result<()> {
    // Runs in the async instance; registered in both so the name is known everywhere.
    ctx.async_task("fetch-motd", |url: String| -> Result<()> {
        let text = Request::get(url).send()?.error_for_status()?.text()?;
        Scheduler::post_to_main("motd", &text)?;   // back to the main instance
        Ok(())
    });
    // Runs in the main instance on the next tick after post_to_main.
    ctx.on_message("motd", |motd: String| Server::broadcast(Text::mini(&motd)));

    if ctx.is_main() {
        // Every minute (1200 ticks), paced by a main-instance timer.
        Scheduler::run_async_timer("fetch-motd", "https://example.org/motd", 0, 1200)?;
    }
    Ok(())
}
```

   Under the hood: `Scheduler::run_async(job, &payload)` is
   `scheduler.run-async-job(job, json-bytes, delay-ms)`; the host calls the
   `plugin.run-job(task, job, payload)` export in the async instance, where the SDK decodes the
   payload and runs the closure registered under `job`. `run_async_timer` is a main-instance
   timer (period in ticks) that starts the job each period; cancelling its task stops it. The
   SDK does not use the raw `run-async` / `run-async-timer` tasks.
3. **Results come back as messages**: `Scheduler::post_to_main(topic, &value)` (JSON) is delivered
   to the main instance's `on-message`, dispatched to the handler registered with
   `ctx.on_message(topic, |value: T| ..)`.
4. **State is per instance.** Fields of your plugin struct in the async instance are a separate
   copy, initialised by the same `Default` + `enable`. Share data through storage or messages.

Inside one instance everything is single-threaded: the host never re-enters a component while one
of its exports runs, so handlers never run concurrently with each other.

### 3.5 Events

Subscribe with `ctx.on(|e: &mut PlayerJoinEvent| ..)`, `ctx.on_with(priority, ignore_cancelled, ..)`,
`Events::subscribe::<E>(..)` (any time, main instance), or `#[event_handler]` methods. Only events
someone subscribed to cost anything: the host calls `handle-<event>` only for subscribed handler
ids, and the SDK dispatches by handler id to the closure.

**Priorities** run in order `lowest`, `low`, `normal`, `high`, `highest`, `monitor`. Use
`highest` to have the last word, `lowest` to go first, and `monitor` only to observe the final
outcome: whatever a monitor handler changes is **ignored** by the host.

**Cancellation.** Cancellable events implement [`Cancellable`]: `is_cancelled()`, `cancel()`,
`set_cancelled(false)` (un-cancel). With `ignore_cancelled = true` a handler is skipped for events
an earlier handler cancelled (Bukkit's `ignoreCancelled`). Plugin menu clicks arrive *cancelled by
default*.

**Mutable fields.** Each typed event has getters for every field and setters **only** for the
fields the host applies back (marked `mut` in `events.wit`): e.g. `PlayerJoinEvent::set_join_message`,
`PlayerChatEvent::set_display`, `BlockBreakEvent::set_drop_items`, `EntityDamageEvent::set_amount`,
`PlayerDeathEvent::set_keep_inventory`, `EntityDeathEvent::set_drops`, `PlayerRespawnEvent::set_location`,
`ServerListPingEvent::set_motd`. Read-only fields (a chat message's signed text, a block's position)
have no setter, so changing them is a compile error rather than a silent no-op.

**Custom events**: `Events::call_custom("my-event", &payload)` fires `<plugin-id>:my-event` to every
subscriber of `ctx.on_custom("<plugin-id>:my-event", ..)`; it returns whether a handler cancelled it.

### 3.6 Commands

Builder (always available):

```rust
ctx.command(
    Command::new("give-diamonds")
        .description("Gives diamonds")
        .alias("gd")
        .permission("example.give")
        .usage("<target:player> [amount:integer(1..64)]")
        .executor(|cmd| {
            let target: Player = cmd.arg("target")?;
            let amount: u32 = cmd.opt_arg("amount")?.unwrap_or(1);
            target.inventory().give(&ItemStack::new("diamond").count(amount))?;
            cmd.reply(Text::mini("<green>Done"));
            Ok(())
        }),
)?;
```

Sub-commands get their own executors with `.route("set <name:word>", |cmd| ..)`;
`.usage_permission(..)` overrides the permission of the last usage; `.completer(..)` answers
dynamic suggestions; `.players_only()` refuses the console.

**Usage syntax**

```text
node   := literal | <arg> | [arg]                     ([arg] = optional, trailing only)
arg    := name[:kind][(min..max)][{a|b|c}][*]
kind   := word (default) | string | greedy | integer | number | boolean | player | players
        | position | block-pos | item | world | duration
```

`(1..64)` bounds integers/numbers (inclusive); `{a|b}` are fixed client-side suggestions; `*` asks
the command's completer. A usage with `k` optional arguments becomes `k + 1` WIT usages. The host
parses arguments: the executor receives typed values through
`cmd.arg::<T>(name)` / `cmd.opt_arg::<T>(name)`, with `T` among `String`, integers, `f64`/`f32`,
`bool`, `Player`, `Vec<Player>`, `Uuid`, `Location`, `BlockPos`, `ItemStack`, `World` and
`Ticks` (durations). A wrong kind or a missing argument becomes a red error message for the
sender. Any `?` on SDK errors works in executors (`CommandError: From<Error>`); errors are shown
in red unless the text sets a color.

### 3.7 Menus

```rust
Menu::new(3, Text::mini("<dark_gray>Warps"))
    .item(11, ItemStack::new("grass_block").name("Spawn"), |click| {
        click.close();
        let _ = click.player().teleport(&spawn);
    })
    .display(13, ItemStack::new("paper").name("Pick one"))
    .fill(ItemStack::new("gray_stained_glass_pane").name(""))
    .on_close(|player| player.send_message("Bye"))
    .open(&player)?;
```

The SDK subscribes to `inventory-click` and `inventory-close` the first time a menu opens and
routes clicks by **(menu id, slot)** to the closures; they are dropped when the menu closes.
Clicks stay cancelled unless the handler calls `click.allow()`. `MenuHandle::set_item(slot, item,
handler)` changes a slot of an open menu.

### 3.8 Text

`Text` builds vanilla JSON chat components (what every WIT `text` parameter takes, via
`impl Into<Text>`; a `&str` is plain text):

```rust
Text::new("Click me")
    .color(NamedColor::Gold)            // or Color::Hex(0xFF8800), Color::parse("#f80")
    .bold()
    .hover(Text::new("Teleports you"))  // show_text
    .run_command("/spawn")              // also suggest_command, open_url, copy_to_clipboard
    .append(Text::translatable("item.minecraft.diamond"));
```

Two parsers produce the same structure:

- `Text::legacy("&aGreen &lbold &#FF8800hex")`: `&`/`§` codes (`0-9a-f`, `k l m n o r`,
  `&#RRGGBB`, `&x&R&R&G&G&B&B`); a color code resets decorations as in vanilla.
- `Text::mini("<red>Hi <bold>there</bold> <hover:show_text:'<gray>tip'><click:run_command:'/spawn'>[spawn]</click></hover>")`:
  tags `<color>`/`<#hex>`/`<color:..>`, `<bold>`/`<b>`, `<italic>`/`<i>`/`<em>`, `<underlined>`/`<u>`,
  `<strikethrough>`/`<st>`, `<obfuscated>`/`<obf>`, negation `<!italic>`, `<reset>`, `<hover:show_text:'..'>`,
  `<click:run_command|suggest_command|open_url|copy_to_clipboard|change_page:'..'>`,
  `<insert:'..'>`, `<font:..>`, `<lang:key:arg..>`, `<key:key.jump>`, `<newline>`/`<br>`;
  closing `</tag>` or `</>`. Unknown or malformed tags stay literal; `\<` escapes. Splice
  player-provided text with `text::escape_mini(name)`.

The JSON uses the current field names (`click_event`/`hover_event`, `command`/`url`/`value`).

### 3.9 Items, worlds, players

- `ItemStack::new("diamond_sword").count(1).name(..).lore([..]).enchant("sharpness", 5)
  .unbreakable().component("minecraft:rarity", "\"epic\"")` writes the components SNBT (text as
  inline compounds, enchantments as an id→level map). Custom names and lore are made non-italic
  unless the text sets italic. Stacks received from the host keep their components verbatim;
  builder components are appended.
- `World::new("minecraft:overworld")`: `get_block`, `set_block`, `fill`, `batch().set(..).apply()`,
  `time`/`set_time`, `weather`/`set_weather`, `spawn_entity`, `particle(..).count(..).spawn()`,
  `play_sound`, `players`. `BlockState` parses the command syntax (`"oak_log[axis=y]"`).
- `Player` is a copyable UUID handle: `name`, `send_message`, `action_bar`, `title`, `sound`,
  `teleport`, `game_mode`/`set_game_mode`, `health`/`set_health`, food, saturation, level, `kick`,
  `is_op`, `has_permission`, `inventory()`, `data()` (persistent per-player key-value data with
  serde helpers). Messaging helpers silently drop messages to offline players (`try_send_message`
  reports it); everything else returns `Result`.
- `Scoreboard`/`Objective`/`Team` (names prefixed with the plugin id), `BossBar`, `TabList`,
  `Messaging` (plugin channels), `Permissions`, `Services`, `Server` (broadcast, console commands,
  tick stats, plugin list).

### 3.10 Storage and config

- `Storage::get::<T>(key)`, `set(key, &value)`, `update(key, |v: &mut T| ..)`, `delete`, `keys`:
  host-persisted key-value storage, one namespace per plugin, values as JSON, shared by both
  instances.
- `player.data().get::<T>(key)` / `set`: the same per player, saved with the player.
- Config (`docs/PLUGIN_FORMAT.md` §5): ship a default `config.toml` that starts with
  `config_version = N`. The server copies it to the data folder on first enable and never
  overwrites the admin's file. Load it with:

  ```rust
  const DEFAULTS: &str = include_str!("../config.toml");
  let cfg: MyConfig = Config::new()
      .defaults(DEFAULTS)                 // target version = its config_version
      .migration(|from, table| {          // called once per step: from -> from + 1
          if from == 1 && let Some(v) = table.remove("homes") {
              table.insert("max_homes".into(), v);
          }
          Ok(())
      })
      .load()?;
  ```

  `load` reads `/config.toml` (needs `filesystem = "data-folder"`). It compares the file's
  `config_version` (absent = 1) with the target, runs the hook (and any per-step
  `.migrate(from, |table| ..)` closures) on the raw `toml::Table`, and writes the upgraded file
  back. It then deserializes into `MyConfig`; missing keys take its serde defaults (use
  `#[serde(default)]`). When the data folder has no file, the packaged text is written verbatim,
  comments included. `.version(n)` sets the target explicitly; `ctx.config::<T>()` is the short
  form without versioning.

### 3.11 Scheduler

| call | runs in | notes |
|---|---|---|
| `Scheduler::run_later(ticks, \|\| ..)` | main | once; 0 = next tick |
| `Scheduler::run_timer(delay, period, \|task\| ..)` | main | `task.cancel()` stops it |
| `Scheduler::run_async(job, &payload)` | async | `job` registered with `ctx.async_task`; WIT `run-async-job` → `run-job` |
| `Scheduler::run_async_later(job, &payload, delay_ms)` | async | |
| `Scheduler::run_async_timer(job, &payload, delay_ticks, period_ticks)` | async | started each period by a main-instance timer; main only |
| `Scheduler::post_to_main(topic, &value)` | from async → main | handled by `ctx.on_message` |

### 3.12 Services

Every plugin world imports the standard services of `rustic:services@0.1.0` (the Vault idea).
The host always defines them and returns `ServiceError::NoProvider` while no plugin provides one.

**Consuming** (any plugin):

```rust
let balance = Economy::balance(&player)?;                 // ServiceResult<i64>, minor units
let shown = Economy::format(balance)?;
Economy::withdraw(&player, 500)?;                         // Err(Refused(..)) when too low
let groups = PermissionProvider::groups(&player)?;
let prefix = ChatMeta::prefix(&player)?.unwrap_or_default();
let installed = Economy::is_available();
```

Functions take anything implementing `Account` (`Player`, `&Player`, `Uuid`). `ServiceError`
converts into `Error` and `CommandError`, so `?` works in handlers and commands.

**Providing** a service: turn on an SDK feature. It selects a provider world, so the component
also exports the interface. Then implement the matching trait on the plugin type:

| SDK feature | world | extra exports | implement |
|---|---|---|---|
| (none) | `rustic-plugin` | none | none |
| `provide-permissions` | `rustic-plugin-permissions` | `rustic:services/permission-provider@0.1.0`, `rustic:services/chat-meta@0.1.0` | `ProvidePermissions`, `ProvideChatMeta` |
| `provide-economy` | `rustic-plugin-economy` | `rustic:services/economy@0.1.0` | `ProvideEconomy` |
| both | `rustic-plugin-permissions-economy` | all three | all three |

```toml
rustic = { package = "rusticmc-api", path = "...", features = ["provide-economy"] }
```

```rust
impl ProvideEconomy for Bank {
    const PRIORITY: Priority = Priority::Normal;     // optional
    fn currency_name(&mut self, plural: bool) -> ServiceResult<String> { /* ... */ }
    fn format(&mut self, amount: i64) -> ServiceResult<String> { /* ... */ }
    fn balance(&mut self, account: Uuid) -> ServiceResult<i64> { /* ... */ }
    fn deposit(&mut self, account: Uuid, amount: i64) -> ServiceResult<i64> { /* ... */ }
    fn withdraw(&mut self, account: Uuid, amount: i64) -> ServiceResult<i64> { /* ... */ }
    // `has` defaults to `balance >= amount`
}
```

- The SDK declares each provided interface with `services.provide(iface, PRIORITY)` during
  `on-load`, in the main instance only.
- It routes the exported calls to the trait methods, with `&mut self` being the plugin.
- `ProvideChatMeta` methods all default to "no prefix/suffix", so
  `impl ProvideChatMeta for MyPerms {}` is enough.
- `ProvidePermissions::primary_group` defaults to the first group.
- With a feature on, a plugin type that lacks the trait impl fails to compile, with an error
  naming the trait.
- A provider must not call its own service through the consumer API: the call would re-enter
  its own instance.

Other interfaces: `Services::provide(iface, priority)` and `Services::provider(iface)` remain
available, but exporting a non-standard interface needs a custom world.

### 3.13 Testing

The pure parts of the SDK are tested on the host target with `cargo test -p rusticmc-api`. That
covers text, usages, items, config, event wrappers, dispatch tables and the provider glue (with
`--features provide-permissions,provide-economy`).

Calling a host import outside a component panics with a message naming the import. To test a
whole plugin, build it and drive it with the `rusticmc-test` harness (an in-memory server).

### 3.14 Notes for host implementers

- **Exported instances:**
  - `rustic:api/plugin@0.1.0`: `on-load`, `on-enable`, `on-disable`, `run-task`, `run-job`,
    `execute-command`, `complete-command`, `on-message`.
  - `rustic:api/handlers@0.1.0`: all 25 `handle-*` functions.
  - Core export names look like `rustic:api/plugin@0.1.0#run-job` and
    `rustic:api/handlers@0.1.0#handle-player-join`.
  - Provider plugins also export `rustic:services/economy@0.1.0`,
    `rustic:services/permission-provider@0.1.0` and/or `rustic:services/chat-meta@0.1.0`.
- **Imports:** a component imports only the `rustic:api/*@0.1.0` and `rustic:services/*@0.1.0`
  interfaces it uses. These include the types-only `rustic:api/types@0.1.0`,
  `rustic:api/event-types@0.1.0` and `rustic:services/common@0.1.0`. Rust's std adds WASI 0.2:
  `wasi:cli/*`, `wasi:clocks/*`, `wasi:io/*`, `wasi:filesystem/*`, `wasi:random/insecure-seed`.
- The SDK calls `scheduler.is-main` once per instance and `server.self-meta` once (for the
  plugin id).
- `on-load` and `on-enable` must run in both instances. The SDK itself skips host registrations
  in the async instance.
