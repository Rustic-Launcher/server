//! The fixture plugins of `plugins/fixtures` in the harness: a misbehaving plugin is disabled,
//! the server (and every other plugin) goes on.

#![cfg(feature = "wasm")]

use rusticmc_test::prelude::*;

fn with_buggy() -> MockServer {
    MockServer::builder()
        .built(&build_plugin("plugins/fixtures/buggy"))
        .built(&build_plugin("plugins/hello"))
        .build()
}

fn assert_failed(server: &MockServer, id: &str) {
    assert_eq!(
        server.plugin_state(id),
        Some(PluginState::Failed),
        "{:?}",
        server.plugin(id)
    );
    server.assert_enabled("hello");
}

#[test]
fn noop_loads_and_does_nothing() {
    let server = MockServer::with_built(&build_plugin("plugins/fixtures/noop"));
    server.assert_enabled("fixture-noop");
    let steve = server.add_player("Steve");
    server.tick(40);
    assert!(steve.messages().iter().all(|m| m.contains("joined")));
}

#[test]
fn a_panicking_chat_handler_disables_only_that_plugin() {
    let server = with_buggy();
    let steve = server.add_player("Steve");
    assert!(steve.chat("boom"), "the message still goes out");
    assert_failed(&server, "fixture-buggy");
    // hello still works.
    assert!(steve.command("/hello").executed());
}

#[test]
fn a_panicking_command_reports_an_internal_error() {
    let server = with_buggy();
    let steve = server.add_player("Steve");
    steve.set_op(true);
    assert!(!steve.command("/buggy-panic").executed());
    steve.assert_message_contains("internal error");
    assert_failed(&server, "fixture-buggy");
}

#[test]
fn an_infinite_loop_is_interrupted() {
    let server = with_buggy();
    assert!(!server.console_command("buggy-loop").executed());
    assert_failed(&server, "fixture-buggy");
}

#[test]
fn an_abort_is_a_trap() {
    let server = with_buggy();
    assert!(!server.console_command("buggy-abort").executed());
    assert_failed(&server, "fixture-buggy");
}

#[test]
fn running_out_of_memory_in_a_task_disables_the_plugin() {
    let server = with_buggy();
    assert!(server.console_command("buggy-oom").executed());
    server.tick(2);
    assert_failed(&server, "fixture-buggy");
}

#[test]
fn an_async_panic_disables_the_plugin_at_the_next_tick() {
    let server = with_buggy();
    assert!(server.console_command("buggy-async-panic").executed());
    server.tick(1);
    server.tick(1);
    assert_failed(&server, "fixture-buggy");
}

#[test]
fn bench_fixture_handles_events_and_commands() {
    let server = MockServer::with_built(&build_plugin("plugins/fixtures/bench"));
    let steve = server.add_player("Steve");
    assert!(steve.move_by(3.0, 0.0, 0.0));
    assert!(server.console_command("bench-noop").executed());
    server.assert_enabled("fixture-bench");
}
