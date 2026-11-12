# `rustic` command reference

`rustic` (crate `crates/rusticmc-cli`) creates, builds, checks, tests, runs and packages Rustic
plugins, and analyzes Bukkit plugin jars. Install it from a checkout:

```sh
cargo install --path crates/rusticmc-cli      # puts `rustic` in ~/.cargo/bin
rustup target add wasm32-wasip2               # plugins are wasm32-wasip2 components
```

Output follows cargo's style: green status verbs on stderr, results on stdout. Colors turn off
when the output is not a terminal, with `NO_COLOR=1` or `CLICOLOR=0` (`CLICOLOR_FORCE=1`
forces them). Every command exits with 0 on success and 1 on failure, after printing
`error: ...` with the fix when there is an obvious one.

A **project** is a directory with `Cargo.toml` and `rustic.plugin.toml`; commands taking
`[path]` use the closest such directory above `path` (default: the current directory).

## `rustic new <id>`

Create a plugin crate from a template.

| Option | Default | |
|---|---|---|
| `-t, --template basic\|command\|gui\|service` | `basic` | what to start from (below) |
| `--dir <DIR>` | `./<id>` | where to create it |
| `--name <NAME>` | from the id (`my-plugin` → `My Plugin`) | display name |
| `--author <NAME>` | `$RUSTIC_AUTHOR`, else `git config user.name` | |
| `--description <TEXT>` | the template's | |
| `--sdk-path <DIR>` | the checkout `rustic` was built from, if present | path dependencies on `<DIR>/rusticmc-api` and `<DIR>/rusticmc-test` (the repository's `crates/` directory) |
| `--sdk-version <VER>` | `0.1` when no checkout is found | registry dependencies |
| `--workspace-member` | off | omit the `[workspace]` table (the crate joins an enclosing workspace) |
| `--force` | off | write into a non-empty directory |

The id must be a valid plugin id (`[a-z0-9_-]{1,64}`, not `rustic`/`minecraft`/`horizon`).
Generated files: `Cargo.toml` (cdylib, SDK dependency, `rusticmc-test` dev-dependency, size
optimised release profile), `rustic.plugin.toml`, `config.toml`, `src/lib.rs`,
`tests/plugin.rs`, `.gitignore`, `README.md`.

| Template | Shows |
|---|---|
| `basic` | events and config: a join greeting, protected blocks (cancelling `block-break`), per-player data; a unit test that the shipped config matches the defaults |
| `command` | `/heal [target:player]` and `/feed`: declared permissions, a per-player cooldown, typed arguments, tab completion, console handling |
| `gui` | `/menu`: a chest menu with click handlers, an allowed click, teleport, close button |
| `service` | an economy provider (`rustic:services/economy`, SDK feature `provide-economy`): `/balance`, `/pay`, `/eco give`, balances persisted in storage |

Every template comes with integration tests that pass out of the box (`rustic test`).

## `rustic build [path] [-r|--release]`

1. Validates `rustic.plugin.toml` strictly (every problem listed).
2. Runs `cargo build --target wasm32-wasip2` (debug unless `--release`), showing cargo's
   progress; fails early with `rustup target add wasm32-wasip2` when the target is missing.
3. Checks the component (same checks as `rustic check`); errors stop the build, warnings are
   printed.
4. Packages `<target>/rustic/<id>.rplugin` (manifest, component at `entry`, `config.toml`,
   `resources/`; deterministic zip) and prints its size and the component's.

## `rustic check [path | file.rplugin | dev-dir] [--deny-warnings] [--release | --debug]`

Validates without building:

* **manifest**: strict schema (unknown keys are errors), id, semver version, entry path,
  dependency requirements, capability values, limits;
* **API version**: the manifest's `api` against the host's (`0.1`); in the 0.x line the minor
  must be inside the host's compatibility window;
* **capabilities**, shown the way an admin sees them ("Hello 0.1.0 by ... asks to: read and
  write files in its own data folder; send HTTP requests to api.example.com");
* **the component** (a project's release build when there is one, what `rustic package` ships,
  else its debug build; `--release`/`--debug` pick one and fail when it is not built; a note says
  when the other profile's build is newer; or the archive's entry): a component and not
  a core module; exports `rustic:api/plugin` and `rustic:api/handlers` of the manifest's API
  version; every imported `rustic:` interface **and function** exists in the host's WIT
  (embedded in `rustic`); only the WASI interfaces the sandbox offers; provider exports
  (`rustic:services/*`) noted;
* **imports vs capabilities**: importing `http` with `network = false` is a warning (every fetch
  would be refused), as is asking for network access without importing `http`;
* **sizes**: a component over 16 MiB is a warning (build with `--release`).

Exit code 1 on errors, and on warnings with `--deny-warnings` (for CI).

Every `rustic` command runs cargo from the plugin's own directory, so its `.cargo/config.toml`
(for example `plugins/`'s shared target directory) applies whatever the current directory is:
`rustic check plugins/perms` from the repository root finds the same build as from `plugins/perms`.

## `rustic test [path] [-- <cargo test args>]`

`cargo test` in the project: unit tests on the host target, then the integration tests in
`tests/`, which build the plugin for wasm32-wasip2 and run the real component in
`rusticmc-test`'s in-memory server (guide: `testing.md`). Arguments after `--` go to cargo
(`rustic test -- greets` runs matching tests).

## `rustic run [path] [--watch] [--server DIR] [--server-bin EXE] [--repl] [--release] [-- <server args>]`

Builds the plugin, then:

* **with a `rustic-server` binary** (`--server-bin`, `$RUSTIC_SERVER_BIN`, the build outputs
  of `crates/rusticmc-server` in the checkout — `horizon/target/{fast,release,debug}` — or
  `PATH`): writes the plugin as a dev directory `plugins/<id>-dev/` of the server directory
  (`--server`, default `<target>/rustic/dev-server`), starts the server there and forwards your
  terminal input to its console. With `--watch`, every save rebuilds and sends
  `rustic reload <id>` to the console. Closing the input (Ctrl+D / Ctrl+Z) sends `stop`.
* **otherwise** (or with `--repl`): says so and starts a **headless REPL**: a `MockServer` with
  the plugin loaded from `<target>/rustic/dev/<id>-dev/`. After each command it prints what the
  console logged and what each player received. With `--watch`, saves rebuild and reload the
  plugin in place (players stay connected). Commands:

```text
join <name> [op]        leave <name>            chat <name> <message>
cmd <name> /<command>   complete <name> <line>  op|deop <name>
perm <name> <node> <true|false|unset>          gm <name> <mode>
tp <name> x y z         move <name> dx dy dz    break <name> x y z
place <name> x y z <block>                      give <name> <item> [count]
damage <name> <n>       kill <name>             respawn <name>
menu <name>             click <name> <slot> [left|right|shift]       close <name>
status <name>           /<console command>      tick [n]
players                 plugins                 console          reload      help   exit
```

The REPL reads standard input, so it can be scripted:
`printf 'join Steve\ncmd Steve /hello\ntick 20\n' | rustic run --repl`.

## `rustic package [path] [-o|--out FILE]`

A release build (as `rustic build --release`), checked and packaged, then prints
`<sha256>  <file>` for publishing. `--out` also copies the archive there.

## `rustic info [path | file.rplugin | dev-dir]`

What a plugin contains: manifest fields, capabilities, sizes and component hash, every import
(with the `rustic:` functions used) and export. No verdict (use `check`).

## `rustic bridge analyze <jar|dir>... [options]`

Static compatibility report of Bukkit/Spigot/Paper plugin jars (no JVM is run), by the bridge
analyzer (`bridge/rusticmc-bridge-analyze`, ADR 0008): green (only API Rustic covers), yellow
(uncovered API, listed member by member), red (server internals, packet libraries,
instrumentation). One jar prints its detailed report; several print the panel summary.

| Option | |
|---|---|
| `--json FILE` | the full report as JSON |
| `--compact N` | in the JSON, only the N most used uncovered members per plugin |
| `--markdown FILE` | the Markdown report to a file instead of stdout |
| `--details` | a section per plugin in a multi-jar report |
| `--coverage FILE` | a coverage table instead of the built-in one |
| `--panel FILE` | `panel.json` from `bridge/panel/fetch.py` (names, versions, licences) |

The verdict of each jar and a summary go to stderr. Missing paths are errors.

## Environment

| Variable | Used by |
|---|---|
| `CARGO` | the cargo to run (set by cargo itself when `rustic` runs under it) |
| `CARGO_TARGET_DIR` | where builds go (honoured through cargo) |
| `RUSTIC_SERVER_BIN` | `run`: the server executable |
| `RUSTIC_AUTHOR` | `new`: the author |
| `NO_COLOR`, `CLICOLOR`, `CLICOLOR_FORCE` | colors |
