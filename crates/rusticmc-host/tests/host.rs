//! Integration tests: real components (hello, the fixtures in `plugins/fixtures`, the guests in
//! `tests/guests`) loaded through `Rustic` with a recording backend.

mod common;

use common::{CAPS, STEVE, Server, test_config};
use rusticmc_core::model::*;
use rusticmc_core::{CommandOutcome, PluginState};
use std::time::{Duration, Instant};

const WAIT: Duration = Duration::from_secs(20);

fn block_break(id: &str) -> BlockBreak {
    BlockBreak {
        player: STEVE,
        world: "minecraft:overworld".into(),
        pos: BlockPos { x: 0, y: 64, z: 0 },
        block: BlockState {
            id: id.into(),
            properties: vec![],
        },
        cancelled: false,
        drop_items: true,
    }
}

fn chat(msg: &str) -> PlayerChat {
    PlayerChat {
        player: STEVE,
        name: "Steve".into(),
        message: msg.into(),
        cancelled: false,
        display: None,
    }
}

fn join() -> PlayerJoin {
    PlayerJoin {
        player: STEVE,
        name: "Steve".into(),
        join_message: None,
    }
}

fn failure(s: &Server, id: &str) -> String {
    s.rt.plugin(id).and_then(|p| p.failure).unwrap_or_default()
}

fn probe_server() -> Server {
    let mut s = Server::new(test_config());
    s.add(
        "probe",
        "guest_probe",
        CAPS,
        Some("greeting = \"from the data folder\"\n"),
    );
    s.start();
    s
}

#[test]
fn hello_end_to_end_with_config_from_the_data_folder() {
    let mut s = Server::new(test_config());
    s.add(
        "hello",
        "hello",
        "[capabilities]\nfilesystem = \"data-folder\"\n",
        Some("config_version = 1\nbanned_words = [\"kumquat\"]\n"),
    );
    s.start();
    assert_eq!(s.rt.state("hello"), Some(PluginState::Enabled));

    // Join: personal welcome + mutated join line; the counter lives in storage.
    let j = s.rt.fire_player_join(join(), &mut s.be);
    assert!(j.join_message.unwrap().to_plain().contains("Steve joined"));
    assert!(
        s.be.rec
            .messages
            .iter()
            .any(|(p, m)| *p == STEVE && m.contains("Steve"))
    );
    assert_eq!(s.stored("hello", "joins").as_deref(), Some("1"));

    // The banned word comes from config.toml read through the WASI preopen.
    assert!(
        s.rt.fire_player_chat(chat("I like KUMQUAT"), &mut s.be)
            .cancelled
    );
    assert!(
        !s.rt
            .fire_player_chat(chat("what the heck"), &mut s.be)
            .cancelled
    );

    // Command + completion.
    let (out, console) = s.console("hello");
    assert_eq!(out, CommandOutcome::Executed);
    assert!(console.iter().any(|l| l.contains("Hello, ")), "{console:?}");
    let sugg =
        s.rt.complete(CommandSender::Player(STEVE), "hello St", &mut s.be);
    assert_eq!(sugg, vec!["Steve".to_owned()]);

    // Timer (every 20 ticks) runs on the tick.
    for _ in 0..25 {
        s.rt.tick(&mut s.be);
    }
    assert_eq!(s.rt.state("hello"), Some(PluginState::Enabled));

    s.rt.disable("hello", &mut s.be).unwrap();
    assert_eq!(s.rt.state("hello"), Some(PluginState::Disabled));
}

#[test]
fn events_priorities_cancellation_and_monitor() {
    let mut s = probe_server();
    // Bedrock: LOW cancels, HIGH (ignore_cancelled) is skipped, MONITOR sees it cancelled and
    // its own changes are ignored.
    let e =
        s.rt.fire_block_break(block_break("minecraft:bedrock"), &mut s.be);
    assert!(e.cancelled);
    assert!(e.drop_items);
    assert_eq!(
        s.stored("probe", "monitor_saw_cancelled").as_deref(),
        Some("true")
    );
    // Stone: not cancelled, HIGH clears drops, MONITOR's un-cancel/re-enable ignored.
    let e =
        s.rt.fire_block_break(block_break("minecraft:stone"), &mut s.be);
    assert!(!e.cancelled);
    assert!(!e.drop_items);
    assert_eq!(
        s.stored("probe", "monitor_saw_cancelled").as_deref(),
        Some("false")
    );

    // Mutable field + an import during dispatch.
    let j = s.rt.fire_player_join(join(), &mut s.be);
    assert_eq!(j.join_message.unwrap().to_plain(), "welcome Steve");
    assert!(s.be.rec.messages.contains(&(STEVE, "hi Steve".to_owned())));

    // Custom events.
    let fire = |s: &mut Server, payload: &[u8]| {
        s.rt.fire_custom_event(
            CustomEvent {
                name: "probe:ping".into(),
                source_plugin: "test".into(),
                payload: payload.to_vec(),
                cancelled: false,
            },
            &mut s.be,
        )
        .cancelled
    };
    assert!(fire(&mut s, b"cancel"));
    assert!(!fire(&mut s, b"keep"));
}

#[test]
fn commands_and_completion() {
    let mut s = probe_server();
    let (out, console) = s.console("probe-echo hello there");
    assert_eq!(out, CommandOutcome::Executed);
    assert_eq!(console, vec!["echo hello there".to_owned()]);
    let (out, _) = s.console("/probe:probe-echo x");
    assert_eq!(out, CommandOutcome::Executed);
    let (out, _) = s.console("probe-fail");
    assert!(matches!(out, CommandOutcome::PluginError(t) if t.to_plain().contains("nope")));
    let (out, _) = s.console("no-such-command");
    assert_eq!(out, CommandOutcome::UnknownCommand);
    let sugg =
        s.rt.complete(CommandSender::Console, "probe-pick ap", &mut s.be);
    assert_eq!(sugg, vec!["apple".to_owned(), "apricot".to_owned()]);
    let (out, console) = s.console("probe-pick banana");
    assert_eq!(out, CommandOutcome::Executed);
    assert_eq!(console, vec!["picked banana".to_owned()]);
}

#[test]
fn sync_task_async_job_and_post_to_main() {
    let mut s = probe_server();
    let (out, _) = s.console("probe-later");
    assert_eq!(out, CommandOutcome::Executed);
    let start_tick = s.rt.current_tick();
    s.tick_until(WAIT, |s| s.stored("probe", "later_ran_at").is_some());
    let ran: u64 = s.stored("probe", "later_ran_at").unwrap().parse().unwrap();
    assert_eq!(ran, start_tick + 2);

    let (out, _) = s.console("probe-async 21");
    assert_eq!(out, CommandOutcome::Executed);
    s.tick_until(WAIT, |s| s.stored("probe", "doubled").is_some());
    assert_eq!(s.stored("probe", "doubled").as_deref(), Some("42"));
    let report = s.stored("probe", "async_report").unwrap();
    assert!(report.contains("is_main=false"), "{report}");
    assert!(
        report.contains("main"),
        "world access must be refused off-thread: {report}"
    );
    assert!(s.be.rec.teleports.is_empty());
    assert!(s.host.stats().async_tasks_run >= 1);
}

#[test]
fn http_is_async_only_and_allowlisted() {
    let mut s = probe_server();
    s.console("probe-http https://denied.example.com/x");
    s.tick_until(WAIT, |s| s.stored("probe", "fetched").is_some());
    let r = s.stored("probe", "fetched").unwrap();
    assert!(
        r.contains("capability") && r.contains("denied.example.com"),
        "{r}"
    );
    assert_eq!(s.rt.state("probe"), Some(PluginState::Enabled));
}

#[test]
fn storage_wasi_config_and_permissions() {
    let mut s = probe_server();
    s.console("probe-store color blue");
    let (_, out) = s.console("probe-get color");
    assert_eq!(out, vec!["value blue".to_owned()]);
    let (_, out) = s.console("probe-config");
    assert_eq!(
        out,
        vec!["config greeting = \"from the data folder\"".to_owned()]
    );
    let (_, out) = s.console("probe-perm");
    assert_eq!(
        out,
        vec!["secret true".to_owned()],
        "the console has every permission"
    );
    s.rt.dispatch_command(CommandSender::Player(STEVE), "probe-perm", &mut s.be);
    assert_eq!(
        s.be.rec.messages.last().map(|m| m.1.as_str()),
        Some("secret false"),
        "declared default deny applies even to ops"
    );
}

#[test]
fn hot_reload_keeps_state_and_hits_the_cache() {
    let mut s = probe_server();
    s.console("probe-store fruit kiwi");
    let before = s.host.stats();
    let done = s.rt.reload("probe", &mut s.be).unwrap();
    assert_eq!(done, vec![rusticmc_core::PluginId::new("probe").unwrap()]);
    let after = s.host.stats();
    assert_eq!(
        after.compiles, before.compiles,
        "unchanged component: no compile"
    );
    assert!(after.memory_cache_hits > before.memory_cache_hits);
    let (_, out) = s.console("probe-get fruit");
    assert_eq!(out, vec!["value kiwi".to_owned()]);
    // Events still flow after the reload (old listeners gone, new ones registered once).
    let e =
        s.rt.fire_block_break(block_break("minecraft:bedrock"), &mut s.be);
    assert!(e.cancelled);
    assert_eq!(s.rt.listeners("block-break").len(), 3);
}

#[test]
fn service_provider_and_consumer() {
    let mut s = Server::new(test_config());
    s.add("probe", "guest_probe", CAPS, None);
    s.start();
    let (_, out) = s.console("probe-pay 5");
    assert_eq!(
        out,
        vec!["service error ServiceError::NoProvider".to_owned()]
    );

    s.add("bank", "guest_economy", "", None);
    s.start();
    assert_eq!(
        s.rt.shared()
            .services()
            .provider(rusticmc_core::services::ECONOMY_IFACE)
            .as_deref(),
        Some("bank")
    );
    let (_, out) = s.console("probe-pay 5");
    assert_eq!(out, vec!["balance 5".to_owned()]);
    let (_, out) = s.console("probe-pay 7");
    assert_eq!(out, vec!["balance 12".to_owned()]);

    // The provider panics: it is disabled, the consumer gets provider-failed and goes on.
    let (_, out) = s.console("probe-pay -1");
    assert_eq!(
        out,
        vec!["service error ServiceError::ProviderFailed".to_owned()]
    );
    assert_eq!(s.rt.state("bank"), Some(PluginState::Failed));
    assert!(
        failure(&s, "bank").contains("negative deposit"),
        "{}",
        failure(&s, "bank")
    );
    assert_eq!(s.rt.state("probe"), Some(PluginState::Enabled));
    let (_, out) = s.console("probe-pay 1");
    assert_eq!(
        out,
        vec!["service error ServiceError::NoProvider".to_owned()]
    );
}

// ------------------------------------------------------------------------- faults

fn buggy_server(limits: &str, cfg: rusticmc_core::config::ServerConfig) -> Server {
    let mut s = Server::new(cfg);
    s.add("buggy", "fixture_buggy", limits, None);
    s.add("probe", "guest_probe", CAPS, None);
    s.start();
    s
}

fn assert_isolated(s: &mut Server) {
    assert_eq!(s.rt.state("buggy"), Some(PluginState::Failed));
    assert_eq!(s.rt.state("probe"), Some(PluginState::Enabled));
    // The other plugin still gets events and commands.
    assert!(
        s.rt.fire_block_break(block_break("minecraft:bedrock"), &mut s.be)
            .cancelled
    );
    assert_eq!(
        s.console("probe-echo still here").1,
        vec!["echo still here".to_owned()]
    );
    // The broken plugin's commands are gone.
    assert_eq!(s.console("buggy-panic").0, CommandOutcome::UnknownCommand);
}

#[test]
fn panic_in_a_handler_disables_only_that_plugin() {
    let mut s = buggy_server("", test_config());
    let e = s.rt.fire_player_chat(chat("this goes boom"), &mut s.be);
    assert!(!e.cancelled);
    let f = failure(&s, "buggy");
    assert!(f.starts_with("panic"), "{f}");
    assert!(
        f.contains("boom in the chat handler"),
        "panic message read from stderr: {f}"
    );
    assert_isolated(&mut s);
}

#[test]
fn panic_in_a_command() {
    let mut s = buggy_server("", test_config());
    let (out, _) = s.console("buggy-panic");
    assert!(matches!(out, CommandOutcome::Fault(_)), "{out:?}");
    assert!(failure(&s, "buggy").contains("buggy panicked on purpose"));
    assert_isolated(&mut s);
}

#[test]
fn infinite_loop_times_out_within_the_deadline() {
    let mut cfg = test_config();
    cfg.limits.epoch_deadline_ms = 100;
    cfg.limits.fuel_per_call = u64::MAX / 4;
    cfg.limits.max_fuel_per_call = u64::MAX / 4;
    let mut s = buggy_server("", cfg);
    let t0 = Instant::now();
    let (out, _) = s.console("buggy-loop");
    let took = t0.elapsed();
    assert!(matches!(out, CommandOutcome::Fault(_)), "{out:?}");
    assert!(
        failure(&s, "buggy").starts_with("timeout"),
        "{}",
        failure(&s, "buggy")
    );
    assert!(took >= Duration::from_millis(100), "{took:?}");
    assert!(
        took < Duration::from_millis(600),
        "interrupted late: {took:?}"
    );
    assert_isolated(&mut s);
}

#[test]
fn fuel_exhaustion() {
    let mut cfg = test_config();
    cfg.limits.epoch_deadline_ms = 30_000;
    let mut s = buggy_server("[limits]\nfuel_per_call = 20000000\n", cfg);
    let (out, _) = s.console("buggy-loop");
    assert!(matches!(out, CommandOutcome::Fault(_)), "{out:?}");
    assert!(
        failure(&s, "buggy").starts_with("fuel-exhausted"),
        "{}",
        failure(&s, "buggy")
    );
    assert_isolated(&mut s);
}

#[test]
fn out_of_memory_in_a_task() {
    let mut s = buggy_server("[limits]\nmemory_mb = 32\n", test_config());
    s.console("buggy-oom");
    s.tick_until(WAIT, |s| s.rt.state("buggy") != Some(PluginState::Enabled));
    let f = failure(&s, "buggy");
    assert!(f.starts_with("memory-exceeded"), "{f}");
    assert!(f.contains("32 MiB"), "{f}");
    assert_isolated(&mut s);
}

#[test]
fn abort_is_a_trap() {
    let mut s = buggy_server("", test_config());
    let (out, _) = s.console("buggy-abort");
    assert!(matches!(out, CommandOutcome::Fault(_)), "{out:?}");
    assert!(
        failure(&s, "buggy").starts_with("trap"),
        "{}",
        failure(&s, "buggy")
    );
    assert_isolated(&mut s);
}

#[test]
fn panic_in_an_async_job_disables_on_the_next_tick() {
    let mut s = buggy_server("", test_config());
    let (out, _) = s.console("buggy-async-panic");
    assert_eq!(out, CommandOutcome::Executed);
    s.tick_until(WAIT, |s| s.rt.state("buggy") != Some(PluginState::Enabled));
    let f = failure(&s, "buggy");
    assert!(f.contains("async job exploded"), "{f}");
    assert!(s.host.stats().async_faults >= 1);
    assert_isolated(&mut s);
}

#[test]
fn trap_in_on_enable_fails_the_reload() {
    let mut s = buggy_server("", test_config());
    s.console("buggy-arm");
    let r = s.rt.reload("buggy", &mut s.be);
    assert!(r.is_err());
    assert_eq!(s.rt.state("buggy"), Some(PluginState::Failed));
    assert!(
        failure(&s, "buggy").contains("on-enable"),
        "{}",
        failure(&s, "buggy")
    );
    assert_eq!(s.rt.state("probe"), Some(PluginState::Enabled));
    // The flag was consumed: enabling again works.
    s.rt.enable("buggy", &mut s.be).unwrap();
    assert_eq!(s.rt.state("buggy"), Some(PluginState::Enabled));
}

#[test]
fn bad_component_is_refused_cleanly() {
    let mut s = Server::new(test_config());
    rusticmc_core::archive::RpluginBuilder::from_manifest_text(
        &common::manifest("junk", ""),
        b"\0asm\x01\0\0\0garbage".to_vec(),
    )
    .unwrap()
    .write_to(&s.plugins_dir().join("junk.rplugin"))
    .unwrap();
    s.add("probe", "guest_probe", CAPS, None);
    let report = s.rt.load_plugins(&mut s.be);
    assert_eq!(report.failed.len(), 1, "{:?}", report.failed);
    assert_eq!(report.failed[0].0, "junk");
    assert_eq!(report.loaded.len(), 1);
}

#[test]
fn disabling_drops_instances_and_shutdown_is_clean() {
    let mut s = probe_server();
    s.console("probe-store a b");
    s.rt.shutdown(&mut s.be);
    assert_eq!(s.rt.state("probe"), Some(PluginState::Disabled));
    // Data was flushed to the data folder.
    assert!(s.dir.path().join("plugins/probe/.rustic/kv.rkv").exists());
}

#[test]
fn pooling_allocator_works_too() {
    let mut s = Server::with_host_config(test_config(), |h| {
        h.pooling = Some(rusticmc_host::PoolingConfig {
            total_component_instances: 16,
            total_core_instances: 256,
            total_memories: 32,
            total_tables: 128,
            max_memory_size: 64 << 20,
        });
    });
    s.add("probe", "guest_probe", CAPS, None);
    s.start();
    assert_eq!(
        s.console("probe-echo pooled").1,
        vec!["echo pooled".to_owned()]
    );
    s.rt.reload("probe", &mut s.be).unwrap();
    assert_eq!(
        s.console("probe-echo again").1,
        vec!["echo again".to_owned()]
    );
}

#[test]
fn idle_plugin_memory_is_reported_and_bounded() {
    let mut s = Server::new(test_config());
    s.add("noop", "fixture_noop", "", None);
    s.start();
    let bytes =
        s.rt.with_plugin_instance("noop", |i| {
            Ok(i.as_any_mut()
                .and_then(|a| a.downcast_mut::<rusticmc_host::WasmPlugin>())
                .map_or(0, |p| p.memory_bytes()))
        })
        .unwrap();
    assert!(bytes > 0 && bytes < 16 << 20, "{bytes}");
}
