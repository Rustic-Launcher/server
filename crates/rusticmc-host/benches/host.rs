//! WASM boundary costs (results in docs/PERFORMANCE.md):
//!
//! * `dispatch/player-move/{0,1,10}`: `Rustic::fire` of a player-move into 0, 1 or 10 plugins
//!   (the `fixture-bench` component loaded under 10 ids) with a no-op handler;
//! * `dispatch/player-join/1`;
//! * `import/players.name/{0,1000}`: a custom event whose handler calls `players.name` n times
//!   (per-import round trip = difference / 1000);
//! * `command/bench-noop`: console command dispatch into a plugin;
//! * `lifecycle/reload-hello`: disable + re-read + instantiate (main + async) + on-load/on-enable
//!   in both instances, compiled component from the in-memory cache.
//!
//! Also printed once: cold compile of hello, compile from wasmtime's disk cache, and linear
//! memory of an idle plugin (main + async instance).

#[path = "../tests/common/mod.rs"]
mod common;

use common::{STEVE, Server, test_config};
use criterion::{Criterion, criterion_group, criterion_main};
use rusticmc_core::model::*;
use rusticmc_host::{HostConfig, WasmHost, WasmPlugin};
use std::hint::black_box;
use std::time::Instant;

fn mv() -> PlayerMove {
    let at = Location {
        world: "minecraft:overworld".into(),
        x: 1.0,
        y: 64.0,
        z: 1.0,
        yaw: 0.0,
        pitch: 0.0,
    };
    PlayerMove {
        player: STEVE,
        from: at.clone(),
        to: Location { x: 1.5, ..at },
        cancelled: false,
    }
}

fn server_with_bench(n: usize) -> Server {
    let mut s = Server::new(test_config());
    for i in 0..n {
        s.add(&format!("bench-{i}"), "fixture_bench", "", None);
    }
    s.start();
    s
}

fn dispatch(c: &mut Criterion) {
    let mut g = c.benchmark_group("dispatch");
    for n in [0usize, 1, 10] {
        let mut s = server_with_bench(n);
        g.bench_function(format!("player-move/{n}"), |b| {
            b.iter(|| black_box(s.rt.fire_player_move(black_box(mv()), &mut s.be)))
        });
    }
    let mut s = server_with_bench(1);
    g.bench_function("player-join/1", |b| {
        b.iter(|| {
            black_box(s.rt.fire_player_join(
                PlayerJoin {
                    player: STEVE,
                    name: "Steve".into(),
                    join_message: None,
                },
                &mut s.be,
            ))
        })
    });
    g.finish();
}

fn imports(c: &mut Criterion) {
    let mut g = c.benchmark_group("import");
    let mut s = server_with_bench(1);
    for n in [0u32, 1000] {
        g.bench_function(format!("players.name/{n}"), |b| {
            b.iter(|| {
                s.rt.fire_custom_event(
                    CustomEvent {
                        name: "bench-0:names".into(),
                        source_plugin: "bench".into(),
                        payload: n.to_le_bytes().to_vec(),
                        cancelled: false,
                    },
                    &mut s.be,
                )
            })
        });
    }
    g.finish();
}

fn commands(c: &mut Criterion) {
    let mut s = server_with_bench(1);
    c.bench_function("command/bench-noop", |b| {
        b.iter(|| black_box(s.rt.dispatch_command(CommandSender::Console, "bench-noop", &mut s.be)))
    });
}

fn lifecycle(c: &mut Criterion) {
    let mut s = Server::new(test_config());
    s.add(
        "hello",
        "hello",
        "[capabilities]\nfilesystem = \"data-folder\"\n",
        None,
    );
    s.add("noop", "fixture_noop", "", None);
    s.start();
    let mem = |s: &Server, id: &str| {
        s.rt.with_plugin_instance(id, |i| {
            Ok(i.as_any_mut()
                .and_then(|a| a.downcast_mut::<WasmPlugin>())
                .map_or(0, |p| p.memory_bytes()))
        })
        .unwrap_or(0)
    };
    eprintln!(
        "linear memory, main + async instance: noop {} KiB, hello {} KiB",
        mem(&s, "noop") / 1024,
        mem(&s, "hello") / 1024
    );
    c.bench_function("lifecycle/reload-hello", |b| {
        b.iter(|| s.rt.reload("hello", &mut s.be).expect("reload"))
    });
}

fn compile_times() {
    let wasm = common::wasm("hello");
    let dir = tempfile::tempdir().expect("tempdir");
    let cfg = || HostConfig {
        cache_dir: Some(dir.path().to_owned()),
        ..HostConfig::default()
    };
    let host = WasmHost::new(cfg()).expect("host");
    let t = Instant::now();
    host.precompile(&wasm).expect("compile");
    let cold = t.elapsed();
    drop(host);
    let host = WasmHost::new(cfg()).expect("host");
    let t = Instant::now();
    host.precompile(&wasm).expect("compile");
    let disk = t.elapsed();
    let t = Instant::now();
    host.precompile(&wasm).expect("compile");
    let mem = t.elapsed();
    eprintln!(
        "hello.wasm ({} KiB): cold compile {cold:?}, from disk cache {disk:?}, in-memory hit {mem:?}",
        wasm.len() / 1024
    );
}

fn all(c: &mut Criterion) {
    compile_times();
    dispatch(c);
    imports(c);
    commands(c);
    lifecycle(c);
}

criterion_group!(benches, all);
criterion_main!(benches);
