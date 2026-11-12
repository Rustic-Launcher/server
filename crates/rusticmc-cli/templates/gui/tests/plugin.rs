//! Integration tests: this plugin's real `.wasm` runs in an in-memory server
//! (`rusticmc-test`). Run them with `rustic test` (or `cargo test`).

use rusticmc_test::prelude::*;

fn server() -> MockServer {
    MockServer::with_built(&build_plugin(env!("CARGO_MANIFEST_DIR")))
}

#[test]
fn menu_opens_with_its_items() {
    let server = server();
    let steve = server.add_player("Steve");
    assert!(steve.command("/menu").executed());
    let menu = steve.open_menu().expect("a menu is open");
    assert_eq!(menu.rows, 3);
    assert_eq!(menu.title_plain(), "{{name}}");
    assert_eq!(menu.item(11).id, "minecraft:compass");
    assert_eq!(menu.item(0).id, "minecraft:gray_stained_glass_pane");
}

#[test]
fn decoration_cannot_be_taken() {
    let server = server();
    let steve = server.add_player("Steve");
    steve.command("/menu");
    assert!(!steve.click_menu_slot(0, ClickKind::ShiftLeft), "cancelled");
    assert_eq!(steve.count("gray_stained_glass_pane"), 0);
}

#[test]
fn free_bread_can_be_taken() {
    let server = server();
    let steve = server.add_player("Steve");
    steve.command("/menu");
    assert!(steve.click_menu_slot(15, ClickKind::ShiftLeft), "allowed");
    assert_eq!(steve.count("bread"), 3);
}

#[test]
fn spawn_button_teleports_and_closes() {
    let server = server();
    let steve = server.add_player("Steve");
    steve.teleport(server.overworld().location(500.0, 70.0, 500.0));
    steve.command("/menu");
    steve.click_menu_slot(11, ClickKind::Left);
    assert!(steve.open_menu().is_none());
    assert_eq!(steve.location().x, server.overworld().spawn().x);
    steve.assert_message_contains("Teleported to spawn.");
}

#[test]
fn daylight_is_for_operators() {
    let server = server();
    server.overworld().set_time(18000);
    let steve = server.add_player("Steve");
    steve.command("/menu");
    steve.click_menu_slot(13, ClickKind::Left);
    steve.assert_message_contains("Operators only.");
    assert_eq!(server.overworld().time(), 18000);

    let admin = server.add_op("Admin");
    admin.command("/menu");
    admin.click_menu_slot(13, ClickKind::Left);
    assert_eq!(server.overworld().time(), 1000);
}

#[test]
fn close_button_closes() {
    let server = server();
    let steve = server.add_player("Steve");
    steve.command("/menu");
    steve.click_menu_slot(26, ClickKind::Left);
    assert!(steve.open_menu().is_none());
}
