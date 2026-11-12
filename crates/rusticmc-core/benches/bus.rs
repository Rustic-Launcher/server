//! Event dispatch cost: 0 listeners (the fast path every engine hook takes), 1 and 10 no-op
//! native listeners, plus `has_listeners`.

use criterion::{Criterion, criterion_group, criterion_main};
use rusticmc_core::archive::PluginPackage;
use rusticmc_core::manifest::Manifest;
use rusticmc_core::model::{BlockBreak, BlockPos, BlockState, Event, EventKind, Priority, Uuid};
use rusticmc_core::native::{NativeLoader, NativePlugin};
use rusticmc_core::{NullBackend, Rustic};
use std::hint::black_box;

fn package(id: &str) -> PluginPackage {
    let m = Manifest::parse(&format!(
        "[plugin]\nid = \"{id}\"\nname = \"{id}\"\nversion = \"1.0.0\"\napi = \"0.1\"\n\
         authors = [\"bench\"]\nlicense = \"MIT\"\nentry = \"{id}.wasm\"\n"
    ))
    .expect("manifest");
    PluginPackage::in_memory(m, Vec::new(), None, Default::default()).expect("package")
}

fn runtime(listeners: usize) -> (Rustic, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let loader = NativeLoader::new();
    loader.register("bench", move || {
        NativePlugin::new()
            .on_enable(move |api| {
                for _ in 0..listeners {
                    api.subscribe("block-break", Priority::Normal, false)?;
                }
                Ok(())
            })
            .on_event(|_, _, e| Ok(e.clone()))
    });
    let rt = Rustic::new(dir.path(), loader.boxed()).expect("runtime");
    let mut b = NullBackend;
    rt.load_packages(vec![package("bench")], &mut b);
    rt.enable_all(&mut b);
    (rt, dir)
}

fn event() -> Event {
    Event::BlockBreak(BlockBreak {
        player: Uuid::new(1, 2),
        world: "minecraft:overworld".into(),
        pos: BlockPos { x: 1, y: 64, z: 2 },
        block: BlockState {
            id: "minecraft:stone".into(),
            properties: vec![],
        },
        cancelled: false,
        drop_items: true,
    })
}

fn bench(c: &mut Criterion) {
    let mut g = c.benchmark_group("dispatch");
    for n in [0usize, 1, 10] {
        let (rt, _dir) = runtime(n);
        let mut backend = NullBackend;
        let mut slot = Some(event());
        g.bench_function(format!("fire_block_break_{n}_listeners"), |b| {
            b.iter(|| {
                if let Some(e) = slot.take() {
                    slot = Some(rt.fire(black_box(e), &mut backend));
                }
            })
        });
        if n == 0 {
            g.bench_function("has_listeners_for", |b| {
                b.iter(|| rt.has_listeners_for(black_box(EventKind::PlayerMove)))
            });
            g.bench_function("has_listeners_by_name", |b| {
                b.iter(|| rt.has_listeners(black_box("player-move")))
            });
        }
    }
    g.finish();
}

criterion_group!(benches, bench);
criterion_main!(benches);
