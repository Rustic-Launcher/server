# ADR 0006: Licensing

- Status: accepted
- Date: 2026-11-09

## Decision

- Rustic (all `rusticmc-*` crates, the WIT contract, docs, official plugins, the Bridge
  analyzer): **MIT OR Apache-2.0**, the Rust ecosystem's usual pair. It is compatible with
  Horizon (MIT) and wasmtime (Apache-2.0 WITH LLVM-exception), and lets patches be offered
  upstream under MIT.
- Plugins built with the SDK may use any licence. The WIT is an interface and imposes none.
- `bridge/java` (the Java layer implementing `org.bukkit.*` interfaces) is **GPL-3.0-only**,
  because it compiles against and implements the Bukkit API (GPLv3). It is a separate,
  optional component that talks to `rustic-server` over IPC (ADR 0009), so the GPL does not reach the core.

## Rules

- No code from Bukkit, Spigot, Paper, CraftBukkit or Glowstone, under any licence, anywhere.
  Concepts are re-designed and listed in `docs/THIRD_PARTY.md`.
- No Mojang code or assets. Horizon's prepared files stay in gitignored `.local/` folders.
- Every new dependency is checked for licence compatibility (`cargo deny` in CI, `deny.toml`).
