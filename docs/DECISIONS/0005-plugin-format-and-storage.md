# ADR 0005: Plugin format and storage

- Status: accepted
- Date: 2026-11-10

## Context

Plugins need a package format that is easy to produce (`rustic build`), safe to read from
untrusted sources, convenient during development, and expressive enough for dependencies and
sandbox capabilities. They also need simple durable key-value storage and per-player data
without depending on an external database.

## Decision

**Manifest**: `rustic.plugin.toml` (TOML, like Cargo; `[plugin]`, `[dependencies]` with
semver requirements, `[capabilities]`, `[limits]`), validated strictly: unknown keys are
errors, every problem is reported at once, ids are `[a-z0-9_-]{1,64}`, `api` is
`MAJOR.MINOR` checked with semver-0.x rules against the host API. Full rules in
`docs/PLUGIN_FORMAT.md`.

**Package**: `.rplugin` = zip (manifest at the root, the `.wasm` entry, optional
`config.toml`, `resources/`). Zip because every OS and tool opens it, and per-entry
compression keeps `resources/` cheap to read. Reading rejects unsafe names and caps sizes during
decompression; writing is deterministic. Unpacked directories with the same layout are
accepted for development; in-memory packages exist for tests and the harness.

**Discovery and resolution**: duplicate ids refuse every copy; required-dependency cycles refuse
every member (named); soft cycles (optional / load_before) are broken with a warning; ties by id.

**Storage**: one `RKV1` file per plugin (`.rustic/kv.rkv` in its data folder) and one per
(plugin, player) (`.rustic/players/<uuid>.rkv`): an in-memory `BTreeMap` written atomically
(tmp + fsync + rename) with an FNV-1a checksum, flushed at most every
`storage.flush_interval_ticks`, on disable and at shutdown; per-plugin quota. A corrupt file is
refused, never truncated. Player stores are evicted on quit.

**Server config**: `rustic.toml` with defaults, `deny_unknown_fields`, range validation; plugin
limits are capped by `max_*` values.

## Consequences

* A plugin's whole state is its data folder: back it up or delete it as a unit.
* Writes are whole-file: fine for the expected sizes (KBs to a few MBs); a plugin storing
  hundreds of MB should use its data folder (capability `filesystem`) or an external service.
* Up to `flush_interval_ticks` of KV writes can be lost on a hard crash (power loss); a clean
  shutdown or disable loses nothing.

## Alternatives considered

* **redb / sled**: crash-safe B-trees with incremental writes, but a new storage engine and
  file format for data that is small and written rarely; revisit if plugins need large stores
  (the `Storage` API would not change).
* **SQLite**: C dependency, overkill.
* **TOML/JSON per player**: values are bytes (WIT `list<u8>`), which would need base64 and make
  files misleadingly "editable".
* **tar.gz packages**: no random access to entries; zip reads the manifest without
  decompressing the component.
* **YAML manifest (Bukkit's `plugin.yml`)**: TOML matches the Rust ecosystem and is less
  ambiguous.
