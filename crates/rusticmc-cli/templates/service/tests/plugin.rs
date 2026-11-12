//! Integration tests: this plugin's real `.wasm` runs in an in-memory server
//! (`rusticmc-test`). Run them with `rustic test` (or `cargo test`).

use rusticmc_test::prelude::*;
use std::collections::BTreeMap;

fn server() -> MockServer {
    MockServer::with_built(&build_plugin(env!("CARGO_MANIFEST_DIR")))
}

#[test]
fn new_accounts_get_the_starting_balance() {
    let server = server();
    let steve = server.add_player("Steve");
    assert!(steve.command("/balance").executed());
    steve.assert_message_contains("Balance: 100.00 coins");
}

#[test]
fn pay_moves_money_and_is_persisted() {
    let server = server();
    let steve = server.add_player("Steve");
    let alex = server.add_player("Alex");
    assert!(steve.command("/pay Alex 25.50").executed());
    steve.assert_message_contains("Paid 25.50 coins.");
    alex.assert_message_contains("You received 25.50 coins.");

    let balances: BTreeMap<String, i64> = server.storage_json("{{id}}", "balances").unwrap();
    assert_eq!(balances[&steve.uuid().to_string()], 7450);
    assert_eq!(balances[&alex.uuid().to_string()], 12550);
}

#[test]
fn cannot_overspend_or_pay_nonsense() {
    let server = server();
    let steve = server.add_player("Steve");
    server.add_player("Alex");
    assert!(!steve.command("/pay Alex 1000").executed());
    steve.assert_message_contains("You do not have enough money.");
    assert!(!steve.command("/pay Alex -3").executed());
    assert!(!steve.command("/pay Steve 1").executed());
    steve.assert_message_contains("You cannot pay yourself.");
}

#[test]
fn eco_give_is_for_operators() {
    let server = server();
    let steve = server.add_player("Steve");
    assert!(!steve.command("/eco give Steve 5").executed());
    assert!(server.console_command("eco give Steve 5").executed());
    server.assert_console_contains("New balance: 105.00 coins");
}

#[test]
fn the_plugin_provides_the_economy_service() {
    let server = server();
    let provider = server
        .runtime()
        .shared()
        .services()
        .provider("rustic:services/economy@0.1.0");
    assert_eq!(provider.as_deref(), Some("{{id}}"));
}
