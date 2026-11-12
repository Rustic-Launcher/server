# ADR 0023: Compiled-component cache

- Status: accepted
- Date: 2026-11-10

## Context

Compiling a plugin component with Cranelift takes from a few hundred milliseconds to seconds
(docs/PERFORMANCE.md). A server restart or `/rustic reload` of an unchanged plugin should not pay
it again, and the loaded artifact must be trustworthy (deserializing compiled code is `unsafe`
in wasmtime and the workspace denies `unsafe`).

## Decision

Two levels, both keyed by content:

1. **In memory** (`ComponentCache`): SHA-256 of the component bytes → compiled `Component`,
   `InstancePre` (imports resolved against the linker once) and export indices; 64 entries,
   oldest out. A reload of an unchanged plugin (the core re-reads the package) instantiates
   straight from it.
2. **On disk**: wasmtime's own cache (`Config::cache`, `CacheConfig::with_directory`), in
   `<server>/cache/rustic/` (`HostConfig::for_server`). Its key covers the component bytes,
   the wasmtime version and the compiler configuration, so a changed engine setting or upgrade
   misses instead of loading incompatible code; entries are compressed and cleaned up by
   wasmtime's worker. This keeps the code free of `unsafe` (`Component::deserialize`).

## Consequences

- Restart: unchanged plugins load from disk (milliseconds); reload: from memory.
- The cache directory can be deleted at any time.
- Memory: compiled code of up to 64 distinct components stays mapped; plugin ids are not part of
  the key, so the same component under several ids is compiled once.

## Alternatives considered

- **Own `.cwasm` files** (`Engine::precompile_component` + `Component::deserialize_file`):
  equivalent speed but `unsafe` and our own invalidation logic.
- **No cache**: every restart recompiles every plugin.
