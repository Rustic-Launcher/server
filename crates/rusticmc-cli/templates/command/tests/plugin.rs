//! Integration tests: this plugin's real `.wasm` runs in an in-memory server
//! (`rusticmc-test`). Run them with `rustic test` (or `cargo test`).

use rusticmc_test::prelude::*;

fn server() -> MockServer {
    MockServer::with_built(&build_plugin(env!("CARGO_MANIFEST_DIR")))
}

#[test]
fn operators_heal_themselves() {
    let server = server();
    let admin = server.add_op("Admin");
    admin.damage(12.0);
    admin.set_food(3);
    assert!(admin.command("/heal").executed());
    assert_eq!(admin.health(), 20.0);
    assert_eq!(admin.food(), 20);
    admin.assert_message_contains("You have been healed.");
}

#[test]
fn players_need_the_permission() {
    let server = server();
    let steve = server.add_player("Steve");
    assert_eq!(
        steve.command("/heal"),
        CommandRun::Plugin(CommandOutcome::NoPermission)
    );
    steve.set_permission("{{id}}.heal", true);
    assert!(steve.command("/heal").executed());
    // Healing others needs another node.
    server.add_player("Alex");
    assert!(!steve.command("/heal Alex").executed());
    steve.assert_message_contains("You may only heal yourself.");
}

#[test]
fn heal_others_and_cooldown() {
    let server = server();
    let admin = server.add_op("Admin");
    let alex = server.add_player("Alex");
    alex.damage(5.0);
    assert!(admin.command("/heal Alex").executed());
    assert_eq!(alex.health(), 20.0);
    admin.assert_message_contains("Healed Alex.");

    alex.damage(5.0);
    assert!(!admin.command("/heal Alex").executed(), "cooldown");
    admin.assert_message_contains("before healing again");
    server.tick(600);
    assert!(admin.command("/heal Alex").executed());
}

#[test]
fn feed_is_for_everyone_and_completion_lists_players() {
    let server = server();
    let steve = server.add_player("Steve");
    steve.set_food(1);
    assert!(steve.command("/feed").executed());
    assert_eq!(steve.food(), 20);

    let admin = server.add_op("Admin");
    assert!(server.complete_as(&admin, "heal S").contains(&"Steve".to_owned()));
}

#[test]
fn the_console_must_name_a_target() {
    let server = server();
    assert!(!server.console_command("heal").executed());
}
