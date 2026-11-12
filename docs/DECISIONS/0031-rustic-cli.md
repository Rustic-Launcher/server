# ADR 0031: The `rustic` command-line tool

- Status: accepted
- Date: 2026-11-11

## Context

Plugin authors need one tool to start a plugin, build the component, package the `.rplugin`
(`docs/PLUGIN_FORMAT.md`), catch contract mistakes before an admin installs the plugin, run the
tests and try the plugin; server owners need to judge Bukkit plugins (`rustic bridge analyze`).

## Decision

1. **A thin layer over cargo.** `rustic build` runs `cargo build --target wasm32-wasip2
   --message-format=json-render-diagnostics` (cargo's progress and diagnostics shown as is, the
   artifact path read from the JSON), then validates and packages with the core's
   `RpluginBuilder` into `<target>/rustic/<id>.rplugin`. `rustic test` is `cargo test`;
   the plugin's integration tests use `rusticmc-test` (ADR 0030). No custom build system, so
   IDEs, `cargo clippy` and `cargo doc` keep working on plugin crates.
2. **Validation against the contract the host implements.** The WIT of `rustic:api@0.1.0`
   (and `rustic:services`) is embedded in the binary; `check` decodes the component with
   `wit-parser` and verifies the exports (`plugin`, `handlers` of the manifest's API version),
   every imported `rustic:` interface **and function**, the WASI subset the sandbox offers, the
   API version window (`ApiVersion::is_compatible_with`), and imports against capabilities
   (`http` without `network`, `network` without `http`). The manifest is parsed by the core's
   strict parser, so the CLI and the server never disagree.
3. **Templates are real crates in the repository** (`crates/rusticmc-cli/templates/{common,basic,
   command,gui,service}`), embedded with `include_str!` and rendered with `{{placeholder}}`
   substitution. Each template ships integration tests; the CLI's end-to-end test generates a
   template, builds, checks, packages, tests and runs it, so templates cannot drift from the SDK.
   Generated crates are standalone workspaces (a `[workspace]` table) with a size-optimised
   release profile; the SDK dependency is a path into the checkout `rustic` was built from when
   it exists, else a registry version.
4. **`rustic run`** prefers a real `rustic-server` (dev directory `plugins/<id>-dev/`, console
   forwarded, `--watch` sends `rustic reload <id>`); without one it falls back to a headless
   REPL over `MockServer`, loading the plugin from a dev directory so a rebuild can be reloaded
   in place. Both modes are scriptable through standard input.
5. **cargo-like UX**: green status verbs on stderr, results on stdout, `anstream` colors that
   honour `NO_COLOR`/`CLICOLOR`, errors with the fix (`rustup target add wasm32-wasip2`,
   `rustic new <id>`).

## Consequences

* Plugin crates depend only on the SDK and, for tests, the harness; `rustic` is optional for
  building (plain `cargo build --target wasm32-wasip2` works) but is the packager.
* The embedded WIT ties a `rustic` binary to the API it was built with; a newer host API needs
  a newer CLI to validate plugins using it.
* The CLI depends on `rusticmc-test` (and so wasmtime) for the REPL; `--no-default-features`
  builds a lighter binary without it.

## Alternatives

* `cargo-component`: adds its own WIT resolution and registry model the SDK does not need (the
  SDK owns its bindings, ADR 0007), and would not package `.rplugin`s.
* A `cargo rustic` subcommand: same code, less discoverable for authors coming from Bukkit; a
  `rustic` binary can still be called by cargo aliases.
* Template engines (handlebars, cargo-generate): four placeholders do not justify a dependency.
