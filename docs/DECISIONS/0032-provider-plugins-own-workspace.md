# ADR 0032: Service-provider plugins build in their own Cargo workspace

- Status: accepted
- Date: 2026-11-11

## Context

A plugin that provides a standard service (`rustic:services/economy`, `permission-provider`,
`chat-meta`) exports extra interfaces, so its component targets a provider world
(`rustic-plugin-economy`, `rustic-plugin-permissions`, ...). ADR 0007 §9 makes the SDK
(`rusticmc-api`) choose the world with **cargo features** (`provide-economy`,
`provide-permissions`): the feature `cfg`-selects one `wit_bindgen::generate!` and the export glue
that requires the `Provide*` trait impls.

Cargo unifies the features of a dependency over every package of a build. In `plugins/`, `perms`
enabled `provide-permissions`, so `cargo build --workspace` (or `clippy`/`test --workspace`)
compiled one SDK with the feature for every member: hello, the fixtures, essentials-lite and the
others then failed with "the trait `ProvidePermissions` is not satisfied" (and would otherwise have
exported services they do not implement). Per-package builds (`-p perms`, `rustic build`,
`rusticmc_test::build_plugin`) worked, but a workspace that only builds one package at a time is a
trap for CI and contributors. `crates/rusticmc-host/tests/guests` already lived with the same
constraint (economy guest, per-package builds).

## Decision

1. **A service-provider plugin is its own Cargo workspace.** Its `Cargo.toml` has a `[workspace]`
   table, its own `Cargo.lock` and release profile, and no `*.workspace = true` inheritance. A
   provider sitting inside another workspace's directory is listed in that workspace's `exclude`.
   In this repository: `plugins/perms` (excluded from `plugins/Cargo.toml`) and the test fixture
   `plugins/essentials-lite/tests/fixtures/economy`.
2. **One target directory anyway.** `plugins/.cargo/config.toml` sets `build.target-dir =
   "target"` (relative to `plugins/`), so the plugins workspace and every standalone provider under
   `plugins/` build into `plugins/target`, and the SDK and dependencies are compiled once per
   feature set. Cargo reads that file from the current directory upwards: commands run from inside
   `plugins/` (including cargo's test processes, whose cwd is the package directory, and
   `rusticmc_test::build_plugin` / `rustic`, which inherit it) use it; from the repository root,
   pass `CARGO_TARGET_DIR=plugins/target`.
3. The ordinary plugins workspace must build, lint and test **as a whole**
   (`cargo build --workspace --target wasm32-wasip2`, `cargo clippy --workspace`,
   `cargo test --workspace` from `plugins/`); providers are gated on their own
   (`cd plugins/perms && cargo clippy/test`). Consumers of a provider load it in tests by path
   (`build_plugin("plugins/perms")`), which builds it in its own workspace.

## Consequences

* `--workspace` commands in `plugins/` work again for every member.
* A provider plugin has its own `Cargo.lock`; it is seeded from `plugins/Cargo.lock`, so the
  versions match and the shared target dir reuses artifacts. Keeping the two in step is manual
  (`cargo update` in both).
* One more command in the gates (one per provider workspace).
* The rule is mechanical and checkable: a crate enabling a `provide-*` feature must have a
  `[workspace]` table.

## Alternatives

* **Per-package gates** (`for p in members; cargo clippy -p $p`): keeps one workspace but every
  `--workspace` invocation stays broken; rejected.
* **World selection by macro** (to revisit): let the plugin say what it provides at the
  expansion site, e.g. `#[rustic::plugin(provides = [permissions, chat_meta])]` or
  `export_plugin!(Perms, provides: economy)`, with the SDK generating the bindings of every world
  in separate modules (or the macro expanding the matching `generate!`) instead of choosing one
  by feature. Features would then not leak between packages and providers could share a
  workspace. Costs: bindings for four worlds in the SDK (compile time; dead code is dropped by
  wit-component), or `wit-bindgen` running inside every plugin crate, which ADR 0007 rejected.
  Worth doing if the number of provider plugins grows or a standard service is added (2^n worlds
  under the feature scheme anyway).
* **A separate provider SDK crate** (`rusticmc-api-provider` re-exporting the SDK with its own
  `generate!`): avoids feature unification too, but two copies of the bindings types would meet
  in a plugin and its tests; deferred with the macro option.
