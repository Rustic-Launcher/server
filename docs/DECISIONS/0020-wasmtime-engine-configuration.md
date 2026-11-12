# ADR 0020: wasmtime engine configuration and sandbox limits

- Status: accepted
- Date: 2026-11-10

## Context

`rusticmc-host` runs untrusted plugin components on the server thread (ADR 0002). A plugin must
never crash or stall the server: traps, panics, infinite loops and memory exhaustion have to end
the call, disable that plugin and leave everything else running. The engine is shared by every
plugin and both of each plugin's instances (ADR 0021).

## Decision

**wasmtime 50.0.1** (latest stable at the time; 51 was a release candidate), features
`runtime, cranelift, component-model, cache, parallel-compilation, pooling-allocator, backtrace,
addr2line, demangle`, plus `wasmtime-wasi` 50.0.1 with `p2` only (sync bindings). One `Engine`:

| setting | value | why |
|---|---|---|
| `consume_fuel` | on | deterministic CPU cap per call (`limits.fuel_per_call`, manifest override capped by `rustic.toml` `max_fuel_per_call`) |
| `epoch_interruption` | on, ticker thread every `HostConfig::epoch_tick` (1 ms) | wall-clock deadline per call (`limits.epoch_deadline_ms`); catches loops in code that burns little fuel per iteration and calls into slow host functions |
| `cranelift_opt_level` | `Speed` | handlers run on every tick |
| `parallel_compilation` | on | first load of a big component |
| `wasm_backtrace_details` | on (configurable) | file:line in fault reports when a plugin ships DWARF |
| cache | wasmtime's content-addressed cache in `<server>/cache/rustic/` (ADR 0023) | restarts and reloads skip Cranelift |
| allocator | on-demand by default, pooling optional (`HostConfig::pooling`) | see below |

**Per call** the host sets fresh fuel and an epoch deadline; an epoch callback compares the
call's real busy time (wall clock minus time blocked in HTTP, for async instances) with the
deadline and interrupts, or extends in steps of ≤ 10 ms on async instances so a disabled
plugin's async call can be cancelled promptly.

**Per store** a `ResourceLimiter` caps total linear memory (`limits.memory_mb`, capped by
`max_memory_mb`), table elements, core instances, tables and memories. Exceeding a cap fails the
growth *with a trap* and records why, so the fault is classified as `MemoryExceeded` rather than
the generic Rust "allocation failed" abort.

**Fault classification** of a failed call: limiter flag → `MemoryExceeded`; `Trap::OutOfFuel`
→ `FuelExhausted`; `Trap::Interrupt` → `Timeout` (or "cancelled" when the plugin is being
disabled); `unreachable` with a Rust panic message on the instance's stderr since the call
started → `Panic` with that message and location; any other trap or error → `Trap`. The wasm
backtrace (symbolicated with demangled names) is attached. The core disables the plugin.

**WASI** per store: no env, no args, closed stdin, stdout/stderr forwarded line by line to the
log with the plugin id (token bucket: 50 lines/s, burst 200; the last 32 stderr lines are kept
for panic messages), clocks and random, sockets denied (`allow_tcp/udp/ip_name_lookup(false)`
and a deny-all address check), and the plugin's data folder preopened read-write as `/` only with
`filesystem = "data-folder"`.

**Pooling allocator: off by default.** Instantiation happens only at load and reload (2 per
plugin), where on-demand allocation with copy-on-write memory images measures in hundreds of
microseconds (docs/PERFORMANCE.md). Pooling would fix the number of slots (plugins × 2 × core
instances per component) and reserve `max_memory_size` of address space per slot for the
largest `memory_mb` any plugin may ask for, for no measurable gain on the hot path. It stays
available (`HostConfig::pooling`) for servers that reload very often.

## Consequences

- An infinite loop is stopped after the deadline (measured: 100 ms deadline → fault after
  ~100-110 ms); fuel exhaustion and OOM are reported as such; a panic's message reaches the log.
- Fuel instrumentation costs some guest speed (typically 10-30 % on tight loops); handler code
  is short, the boundary dominates (docs/PERFORMANCE.md).
- The epoch ticker is one thread waking every millisecond.

## Alternatives considered

- **Fuel only**: no wall-clock bound when a plugin spends its time in host calls.
- **Epoch only**: not deterministic; a loop of cheap host calls could run long between checks.
- **`StoreLimits` (wasmtime's built-in limiter)**: same caps, but cannot tell "limit hit" from
  other traps without parsing messages.
- **Async wasmtime with fuel yielding**: needs an executor on the server thread; the plugin API
  is synchronous.
