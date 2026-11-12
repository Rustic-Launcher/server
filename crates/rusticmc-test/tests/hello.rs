//! The test suite of `plugins/hello`, run against its real `.wasm` (built for wasm32-wasip2 by
//! `build_plugin`, loaded through `rusticmc-host`).

#![cfg(feature = "wasm")]

use rusticmc_test::prelude::*;

fn server() -> MockServer {
    MockServer::with_built(&build_plugin("plugins/hello"))
}

#[test]
fn hello_is_enabled_and_logs() {
    let server = server();
    server.assert_enabled("hello");
    server.assert_console_contains("Hello enabled");
    assert!(server.has_command("hello") && server.has_command("hi"));
}

#[test]
fn joining_players_are_welcomed_and_counted() {
    let server = server();
    let steve = server.add_player("Steve");
    steve.assert_message_contains("Welcome, Steve!");
    steve.assert_message_contains("[say hi]");
    // The join line is replaced by the plugin's, with the join counter.
    assert!(
        server
            .broadcasts()
            .contains(&"Steve joined (join #1)".to_owned()),
        "{:?}",
        server.broadcasts()
    );
    let alex = server.add_player("Alex");
    steve.assert_message_contains("Alex joined (join #2)");
    alex.assert_message_contains("Welcome, Alex!");
    assert_eq!(server.storage_json::<u64>("hello", "joins"), Some(2));
}

#[test]
fn the_welcome_suggests_the_hello_command() {
    let server = server();
    let steve = server.add_player("Steve");
    let json = steve.messages_json()[0].as_json().to_owned();
    assert!(json.contains("suggest_command"), "{json}");
    assert!(json.contains("/hello "), "{json}");
}

#[test]
fn banned_words_are_cancelled() {
    let server = server();
    let steve = server.add_player("Steve");
    let alex = server.add_player("Alex");
    assert!(!steve.chat("what the HECK"));
    steve.assert_message_contains("Your message contains a banned word.");
    alex.assert_no_message_contains("HECK");
    assert!(steve.chat("hello everyone"));
    alex.assert_message_contains("<Steve> hello everyone");
}

#[test]
fn hello_command_greets_yourself_and_others() {
    let server = server();
    let steve = server.add_player("Steve");
    let alex = server.add_player("Alex");
    assert!(steve.command("/hello").executed());
    steve.assert_message_contains("Hello, Steve!");

    assert!(steve.command("/hi Alex").executed());
    alex.assert_message_contains("Steve says hello!");
    alex.assert_heard("entity.experience_orb.pickup");
    steve.assert_message_contains("Said hello to Alex.");

    assert!(!steve.command("/hello Nobody").executed());
    steve.assert_message_contains("That player is not online.");
}

#[test]
fn the_console_can_say_hello() {
    let server = server();
    server.add_player("Steve");
    assert!(server.console_command("hello Steve").executed());
    server
        .player("Steve")
        .unwrap()
        .assert_message_contains("says hello!");
}

#[test]
fn hello_completes_online_names() {
    let server = server();
    let steve = server.add_player("Steve");
    server.add_player("Alex");
    server.add_player("Alice");
    let mut names = server.complete_as(&steve, "hello Al");
    names.sort();
    assert_eq!(names, vec!["Alex", "Alice"]);
    assert!(
        server
            .complete_as(&steve, "hel")
            .contains(&"hello".to_owned())
    );
}

#[test]
fn hello_needs_its_permission() {
    let server = server();
    let steve = server.add_player("Steve");
    steve.set_permission("hello.use", false);
    assert_eq!(
        steve.command("/hello"),
        CommandRun::Plugin(CommandOutcome::NoPermission)
    );
}

#[test]
fn the_action_bar_ticks_every_second() {
    let server = server();
    let steve = server.add_player("Steve");
    server.tick(19);
    assert!(steve.action_bars().is_empty());
    server.tick(1);
    assert_eq!(
        steve.last_action_bar().as_deref(),
        Some("Online 1s · joins 1")
    );
    server.tick(40);
    assert_eq!(
        steve.last_action_bar().as_deref(),
        Some("Online 3s · joins 1")
    );
    assert_eq!(steve.action_bars().len(), 3);
}

#[test]
fn the_config_file_is_honoured() {
    let server = MockServer::builder()
        .plugin_config(
            "hello",
            "config_version = 1\nwelcome = \"Howdy {name}\"\nbanned_words = [\"pineapple\"]\n",
        )
        .built(&build_plugin("plugins/hello"))
        .build();
    let steve = server.add_player("Steve");
    steve.assert_message_contains("Howdy Steve");
    assert!(steve.chat("heck yes"), "heck is no longer banned");
    assert!(!steve.chat("pineapple pizza"));
}

#[test]
fn reload_keeps_the_join_counter() {
    let server = server();
    server.add_player("Steve");
    server.reload("hello").unwrap();
    server.assert_enabled("hello");
    let alex = server.add_player("Alex");
    alex.assert_message_contains("Alex joined (join #2)");
}

#[test]
fn disabling_hello_removes_its_command() {
    let server = server();
    let steve = server.add_player("Steve");
    server.disable("hello").unwrap();
    assert_eq!(steve.command("/hello"), CommandRun::Unknown);
    assert!(steve.chat("heck"), "no more chat filter");
}

#[test]
fn the_rplugin_archive_loads_too() {
    let hello = build_plugin("plugins/hello");
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("hello.rplugin");
    std::fs::write(&file, hello.rplugin()).unwrap();
    let server = MockServer::with_plugin_wasm(&file);
    server
        .add_player("Steve")
        .assert_message_contains("Welcome");
}
