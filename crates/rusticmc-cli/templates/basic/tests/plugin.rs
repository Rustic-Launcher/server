//! Integration tests: this plugin's real `.wasm` runs in an in-memory server
//! (`rusticmc-test`). Run them with `rustic test` (or `cargo test`).

use rusticmc_test::prelude::*;

/// A server running this plugin (built once per test run).
fn server() -> MockServer {
    MockServer::with_built(&build_plugin(env!("CARGO_MANIFEST_DIR")))
}

#[test]
fn greets_joining_players() {
    let server = server();
    let steve = server.add_player("Steve");
    steve.assert_message_contains("Welcome to the server, Steve!");
}

#[test]
fn protected_blocks_stay() {
    let server = server();
    let steve = server.add_player("Steve");
    let world = server.overworld();
    world.set_block((3, 64, 3), "diamond_ore");

    assert!(!steve.break_block((3, 64, 3)), "the plugin cancels the break");
    assert_eq!(world.block_id((3, 64, 3)), "minecraft:diamond_ore");
    steve.assert_message_contains("This block is protected.");

    assert!(steve.break_block((4, 64, 4)), "grass is not protected");
    assert_eq!(world.block_id((4, 64, 4)), "minecraft:air");
}

#[test]
fn counts_broken_blocks_per_player() {
    let server = server();
    let steve = server.add_player("Steve");
    for x in 0..3 {
        steve.break_block((x, 64, 10));
    }
    assert_eq!(steve.data_json::<u64>("{{id}}", "blocks_broken"), Some(3));
}

#[test]
fn admins_can_change_the_greeting() {
    let server = MockServer::builder()
        .plugin_config("{{id}}", "config_version = 1\ngreeting = \"Hi {name}\"\n")
        .built(&build_plugin(env!("CARGO_MANIFEST_DIR")))
        .build();
    server.add_player("Alex").assert_message_contains("Hi Alex");
}
