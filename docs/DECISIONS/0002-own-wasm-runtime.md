# ADR 0002: Rustic runs its own WebAssembly component runtime

- Status: accepted
- Date: 2026-11-09

## Context

The mission assumed Horizon ships a WASM mod system (WIT `horizon:mods@0.1.0`, wasmtime,
capabilities) that Rustic could build on. Reading Horizon at `a84f637d` shows otherwise:

- `crates/horizon-mods` still parses a `mod.wasm` and WIT capability names in manifests, but no
  wasmtime dependency is left in `Cargo.lock`. `docs/mods.md` states "code is native now":
  `cargo horizon-mod build` refuses a folder with a `Cargo.toml`.
- Code mods are **native mods** (`.hnmod`): horizon-mixins bundles that patch Horizon's source
  before compilation, or **trait mods** implementing `mod_api::Hooks`, linked into the
  executable. Both are unsandboxed ("Native mods run unsandboxed: only build code you have read and trust").
- The WASM mod work continues on an unofficial fork (arismoko/horizon, see issue #7386) that
  upstream has not adopted.

Rustic's requirements are the opposite of native mods: a plugin is untrusted, hot-reloadable,
must never crash the server and must run on a stable, versioned API.

## Decision

Rustic embeds **wasmtime** (Bytecode Alliance, Apache-2.0 WITH LLVM-exception) with the
**component model**. The API is the WIT package `rustic:api` (source of truth in
`crates/rusticmc-api-wit/wit/`). Plugins are components targeting `wasm32-wasip2`.

Horizon is reached through **one** native trait mod (ADR 0001): Rustic's runtime lives inside
that trait mod and fans Horizon's hooks out to plugins.

- Isolation: one `Store` per plugin instance, fuel per call, epoch-based deadline, memory limits
  (`StoreLimits`), traps converted to `PluginFault` → that plugin is disabled.
- WASI: only clocks, random, stdio-to-log and the plugin's data folder preopened as `/` when
  `capabilities.filesystem = "data-folder"`. No sockets; HTTP goes through Rustic's own
  `http` import with a domain allowlist.
- Async work runs in a second instance of the same component (no shared memory), so the main
  instance is only ever touched from the server thread.

## Consequences

- Rustic does not depend on the fate of Horizon's mod runtime; the trait-mod seam is small and
  versioned upstream (`native-hooks.json`, `NATIVE_HOOKS_API`).
- The WASM boundary costs per call (measured in `docs/PERFORMANCE.md`); the event bus only crosses
  it for events a plugin subscribed to.
- If upstream adopts WASM mods later, Rustic can keep its runtime (different goals: untrusted
  server plugins vs. player-installed client mods) or share low-level pieces.

## Alternatives considered

- **Reuse Horizon's mods as the foundation**: impossible today (no runtime upstream).
- **Native Rust plugins (dylib/abi_stable)**: no sandbox, a panic or UB kills the server, ABI
  tied to compiler versions. Rejected by the "a plugin can never crash the server" rule.
- **Wrap the fork's WASM runtime**: unofficial, client-oriented, not versioned for servers.
- **Other engines (wasmer, wasm3/WAMR)**: wasmtime has the most complete component model
  support, fuel + epoch interruption and pooling allocation, and is actively maintained.
