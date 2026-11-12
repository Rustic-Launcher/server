//! Integration tests: the real `essentials_lite.wasm` in `rusticmc-test`'s in-memory server.
//! Every command, the permission denials, warmup/cooldown, request expiry, `/back` on teleport
//! and death, the chat-meta prefix (with the real `perms.wasm`), the economy cost (with a test
//! economy provider) and the French messages.

use rusticmc_test::BuiltPlugin;
use rusticmc_test::prelude::*;

const ID: &str = "essentials-lite";
/// Instant teleports, no cooldown: most tests are about the commands themselves.
const FAST: &str =
    "config_version = 1\nteleport_warmup_seconds = 0\nteleport_cooldown_seconds = 0\n";

fn built() -> BuiltPlugin {
    build_plugin(env!("CARGO_MANIFEST_DIR"))
}

fn server_with(config: &str) -> MockServer {
    MockServer::builder()
        .plugin_config(ID, config)
        .built(&built())
        .build()
}

fn fast() -> MockServer {
    server_with(FAST)
}

fn run(p: &MockPlayer, line: &str) {
    let r = p.command(line);
    assert!(r.executed(), "{line}: {r:?}\nmessages: {:?}", p.messages());
}

fn refused(p: &MockPlayer, line: &str) {
    let r = p.command(line);
    assert!(
        matches!(r, CommandRun::Plugin(CommandOutcome::PluginError(_))),
        "{line}: {r:?}"
    );
}

fn denied(p: &MockPlayer, line: &str) {
    let r = p.command(line);
    assert!(
        matches!(r, CommandRun::Plugin(CommandOutcome::NoPermission)),
        "{line}: {r:?}"
    );
}

fn at(server: &MockServer, x: f64, y: f64, z: f64) -> Location {
    server.overworld().location(x, y, z)
}

fn assert_near(p: &MockPlayer, x: f64, y: f64, z: f64) {
    let l = p.location();
    assert!(
        (l.x - x).abs() < 1e-6 && (l.y - y).abs() < 1e-6 && (l.z - z).abs() < 1e-6,
        "{} is at {l:?}, expected ({x}, {y}, {z})",
        p.name()
    );
}

fn jsons(p: &MockPlayer) -> Vec<String> {
    p.messages_json()
        .iter()
        .map(|t| t.as_json().to_owned())
        .collect()
}

#[test]
fn enables_with_commands_and_declared_defaults() {
    let server = fast();
    server.assert_enabled(ID);
    server.assert_console_contains("essentials-lite enabled");
    for cmd in [
        "spawn",
        "setspawn",
        "home",
        "sethome",
        "delhome",
        "homes",
        "tpa",
        "tpahere",
        "tpaccept",
        "tpdeny",
        "back",
        "warp",
        "setwarp",
        "delwarp",
        "warps",
        "msg",
        "r",
        "broadcast",
    ] {
        assert!(server.has_command(cmd), "/{cmd}");
    }
    let steve = server.add_player("Steve");
    assert!(steve.has_permission("essentials.home"));
    assert!(steve.has_permission("essentials.tpa"));
    assert!(!steve.has_permission("essentials.setspawn"));
    assert!(!steve.has_permission("essentials.homes.3"));
    let op = server.add_op("Admin");
    assert!(op.has_permission("essentials.setwarp"));
    assert!(op.has_permission("essentials.broadcast"));
}

#[test]
fn spawn_and_setspawn() {
    let server = fast();
    let steve = server.add_player("Steve");
    let admin = server.add_op("Admin");
    steve.set_location(at(&server, 40.5, 65.0, 40.5));
    // No spawn set yet: the world's.
    run(&steve, "/spawn");
    steve.assert_message_contains("Teleported to spawn.");
    let spawn = server.overworld().spawn();
    assert_near(&steve, spawn.x, spawn.y, spawn.z);

    denied(&steve, "/setspawn");
    admin.set_location(at(&server, 100.5, 70.0, -20.5));
    run(&admin, "/setspawn");
    admin.assert_message_contains("Spawn set at 100 70 -21 in overworld.");
    let stored: serde_json::Value = server.storage_json(ID, "spawn").unwrap();
    assert_eq!(stored["x"], 100.5);
    run(&steve, "/spawn");
    assert_near(&steve, 100.5, 70.0, -20.5);
    // The console cannot teleport.
    assert!(!server.console_command("spawn").executed());
}

#[test]
fn homes_limits_list_and_completion() {
    let server = fast();
    let steve = server.add_player("Steve");
    refused(&steve, "/home");
    steve.assert_message_contains("You have no home yet.");
    run(&steve, "/homes");

    steve.set_location(at(&server, 10.5, 65.0, 10.5));
    run(&steve, "/sethome");
    steve.assert_message_contains("Home home set.");
    run(&steve, "/sethome");
    steve.assert_message_contains("Home home moved here.");
    refused(&steve, "/sethome base");
    steve.assert_message_contains("You can have at most 1 homes.");

    steve.set_permission("essentials.homes.3", true);
    steve.set_location(at(&server, -30.5, 66.0, 5.5));
    run(&steve, "/sethome Base");
    steve.assert_message_contains("Home base set.");
    refused(&steve, "/sethome bad.name");
    steve.assert_message_contains("Invalid name bad.name");
    let homes: serde_json::Value = steve.data_json(ID, "homes").unwrap();
    assert_eq!(homes["base"]["x"], -30.5);

    run(&steve, "/homes");
    steve.assert_message_contains("Your homes (2/3)");
    steve.assert_message_contains("[base] [home]");
    let json = jsons(&steve).join("\n");
    assert!(
        json.contains("/home base") && json.contains("/home home"),
        "{json}"
    );

    run(&steve, "/home");
    assert_near(&steve, 10.5, 65.0, 10.5);
    run(&steve, "/home base");
    steve.assert_message_contains("Teleported to home base.");
    assert_near(&steve, -30.5, 66.0, 5.5);
    refused(&steve, "/home nope");
    steve.assert_message_contains("You have no home named nope.");
    assert_eq!(steve.complete("home b"), vec!["base".to_owned()]);
    assert_eq!(steve.complete("delhome h"), vec!["home".to_owned()]);

    run(&steve, "/delhome home");
    steve.assert_message_contains("Home home deleted.");
    refused(&steve, "/delhome home");
    // One home left: a bare /home goes there.
    steve.set_location(at(&server, 0.5, 65.0, 0.5));
    run(&steve, "/home");
    assert_near(&steve, -30.5, 66.0, 5.5);

    steve.set_permission("essentials.homes.unlimited", true);
    run(&steve, "/homes");
    steve.assert_message_contains("Your homes (1/∞)");
    steve.set_permission("essentials.sethome", false);
    denied(&steve, "/sethome x");
    denied(&steve, "/delhome base");
    steve.set_permission("essentials.home", false);
    denied(&steve, "/home");
    denied(&steve, "/homes");
}

#[test]
fn warps() {
    let server = fast();
    let admin = server.add_op("Admin");
    let steve = server.add_player("Steve");
    run(&steve, "/warps");
    steve.assert_message_contains("There are no warps yet.");
    denied(&steve, "/setwarp shop");
    admin.set_location(at(&server, 7.5, 65.0, -7.5));
    run(&admin, "/setwarp Shop");
    admin.assert_message_contains("Warp shop set.");
    let warps: serde_json::Value = server.storage_json(ID, "warps").unwrap();
    assert_eq!(warps["shop"]["z"], -7.5);

    run(&steve, "/warps");
    steve.assert_message_contains("Warps (1)");
    assert!(jsons(&steve).join("\n").contains("/warp shop"));
    assert_eq!(steve.complete("warp s"), vec!["shop".to_owned()]);
    run(&steve, "/warp shop");
    steve.assert_message_contains("Teleported to warp shop.");
    assert_near(&steve, 7.5, 65.0, -7.5);
    refused(&steve, "/warp mine");
    steve.assert_message_contains("There is no warp named mine.");

    denied(&steve, "/delwarp shop");
    run(&admin, "/delwarp shop");
    admin.assert_message_contains("Warp shop deleted.");
    refused(&admin, "/delwarp shop");
    // The console may delete warps.
    run(&admin, "/setwarp mine");
    assert!(server.console_command("delwarp mine").executed());
    server.assert_console_contains("Warp mine deleted.");
}

#[test]
fn per_warp_permissions() {
    let server = server_with(&format!("{FAST}per_warp_permissions = true\n"));
    let admin = server.add_op("Admin");
    let steve = server.add_player("Steve");
    run(&admin, "/setwarp vip");
    run(&admin, "/setwarp pub");
    refused(&steve, "/warp vip");
    steve.assert_message_contains("You may not use warp vip.");
    run(&steve, "/warps");
    steve.assert_message_contains("There are no warps yet.");
    steve.set_permission("essentials.warps.pub", true);
    run(&steve, "/warps");
    steve.assert_message_contains("Warps (1)");
    run(&steve, "/warp pub");
}

#[test]
fn tpa_accept_moves_the_requester() {
    let server = fast();
    let steve = server.add_player("Steve");
    let alex = server.add_player("Alex");
    steve.set_location(at(&server, 1.5, 65.0, 1.5));
    alex.set_location(at(&server, 50.5, 65.0, 50.5));
    refused(&steve, "/tpa Steve");
    steve.assert_message_contains("You cannot send a request to yourself.");
    run(&steve, "/tpa Alex");
    steve.assert_message_contains("Request sent to Alex. It expires in 60s.");
    alex.assert_message_contains("Steve wants to teleport to you.");
    alex.assert_message_contains("[Accept] [Deny] (60s)");
    alex.assert_heard("block.note_block.pling");
    let json = jsons(&alex).join("\n");
    assert!(
        json.contains("/tpaccept Steve") && json.contains("/tpdeny Steve"),
        "{json}"
    );
    assert_eq!(alex.complete("tpaccept "), vec!["Steve".to_owned()]);

    run(&alex, "/tpaccept");
    alex.assert_message_contains("You accepted the request from Steve.");
    steve.assert_message_contains("Alex accepted your request.");
    steve.assert_message_contains("Teleported to Alex.");
    assert_near(&steve, 50.5, 65.0, 50.5);
    refused(&alex, "/tpaccept");
    alex.assert_message_contains("You have no pending teleport request.");
}

#[test]
fn tpahere_deny_names_and_expiry() {
    let server = fast();
    let steve = server.add_player("Steve");
    let alex = server.add_player("Alex");
    let bob = server.add_player("Bob");
    steve.set_location(at(&server, 1.5, 65.0, 1.5));
    alex.set_location(at(&server, 50.5, 65.0, 50.5));

    run(&steve, "/tpahere Alex");
    alex.assert_message_contains("Steve wants you to teleport to them.");
    run(&bob, "/tpa Alex");
    refused(&alex, "/tpaccept Nobody");
    alex.assert_message_contains("No pending request from Nobody.");
    run(&alex, "/tpdeny Bob");
    alex.assert_message_contains("You denied the request from Bob.");
    bob.assert_message_contains("Alex denied your request.");
    run(&alex, "/tpaccept Steve");
    // tpahere: the target (Alex) moves to the requester.
    assert_near(&alex, 1.5, 65.0, 1.5);

    // Requests expire after tpa_timeout_seconds (60 s = 1200 ticks).
    run(&steve, "/tpa Alex");
    server.tick(1199);
    alex.assert_no_message_contains("expired");
    server.tick(1);
    steve.assert_message_contains("Your teleport request to Alex expired.");
    alex.assert_message_contains("The teleport request from Steve expired.");
    refused(&alex, "/tpdeny");

    // A request is dropped when its sender leaves.
    run(&bob, "/tpa Alex");
    bob.disconnect();
    refused(&alex, "/tpaccept");

    alex.set_permission("essentials.tpa", false);
    denied(&alex, "/tpa Steve");
    alex.set_permission("essentials.tpahere", false);
    denied(&alex, "/tpahere Steve");
    alex.set_permission("essentials.tpaccept", false);
    denied(&alex, "/tpaccept");
    denied(&alex, "/tpdeny");
}

#[test]
fn back_after_teleports_and_death() {
    let server = fast();
    let steve = server.add_player("Steve");
    refused(&steve, "/back");
    steve.assert_message_contains("There is no previous location to return to.");

    steve.set_location(at(&server, 12.5, 65.0, 12.5));
    run(&steve, "/spawn");
    run(&steve, "/back");
    steve.assert_message_contains("Teleported to your previous location.");
    assert_near(&steve, 12.5, 65.0, 12.5);

    // Teleports by anything else (vanilla /tp, other plugins) count too.
    steve.teleport(at(&server, 80.5, 65.0, 80.5));
    steve.set_location(at(&server, 90.5, 65.0, 90.5));
    run(&steve, "/back");
    assert_near(&steve, 12.5, 65.0, 12.5);

    // Death: the hint, then /back to where they died.
    steve.set_location(at(&server, -60.5, 65.0, 33.5));
    steve.kill();
    steve.assert_message_contains("Use /back to return to where you died.");
    steve.respawn();
    run(&steve, "/back");
    assert_near(&steve, -60.5, 65.0, 33.5);
    let stored: serde_json::Value = steve.data_json(ID, "back").unwrap();
    assert!(stored.get("world").is_some());

    steve.set_permission("essentials.back", false);
    denied(&steve, "/back");
}

#[test]
fn warmup_waits_and_is_cancelled_by_moving() {
    let server = server_with(
        "config_version = 1\nteleport_warmup_seconds = 3\nteleport_cooldown_seconds = 0\n",
    );
    let steve = server.add_player("Steve");
    let admin = server.add_op("Admin");
    admin.set_location(at(&server, 20.5, 65.0, 20.5));
    run(&admin, "/setwarp w");
    steve.set_location(at(&server, 0.5, 65.0, 0.5));

    run(&steve, "/warp w");
    steve.assert_message_contains("Teleporting to warp w in 3s. Don't move!");
    server.tick(59);
    assert_near(&steve, 0.5, 65.0, 0.5);
    server.tick(1);
    assert_near(&steve, 20.5, 65.0, 20.5);

    steve.set_location(at(&server, 0.5, 65.0, 0.5));
    run(&steve, "/warp w");
    // Turning on the spot or a tiny step does not cancel; walking away does.
    steve.move_by(0.2, 0.0, 0.0);
    server.tick(10);
    steve.assert_no_message_contains("Teleport cancelled");
    steve.move_by(2.0, 0.0, 0.0);
    steve.assert_message_contains("Teleport cancelled: you moved.");
    server.tick(80);
    assert_near(&steve, 2.7, 65.0, 0.5);

    // A second request replaces the pending one.
    run(&steve, "/warp w");
    run(&steve, "/spawn");
    steve.assert_message_contains("Your previous pending teleport was cancelled.");

    // Ops bypass the warmup.
    run(&admin, "/spawn");
    admin.assert_message_contains("Teleported to spawn.");
}

#[test]
fn cooldown_between_teleports() {
    let server = server_with(
        "config_version = 1\nteleport_warmup_seconds = 0\nteleport_cooldown_seconds = 5\n",
    );
    let steve = server.add_player("Steve");
    run(&steve, "/spawn");
    refused(&steve, "/spawn");
    steve.assert_message_contains("Wait 5s before teleporting again.");
    server.tick(60);
    refused(&steve, "/spawn");
    steve.assert_message_contains("Wait 2s before teleporting again.");
    server.tick(40);
    run(&steve, "/spawn");
    steve.set_permission("essentials.cooldown.bypass", true);
    run(&steve, "/spawn");
}

#[test]
fn private_messages_and_replies() {
    let server = fast();
    let steve = server.add_player("Steve");
    let alex = server.add_player("Alex");
    refused(&steve, "/r hi");
    steve.assert_message_contains("Nobody to reply to.");
    refused(&steve, "/msg Steve hi");
    run(&steve, "/msg Alex hello <red>there");
    steve.assert_message_contains("[me → Alex] hello <red>there");
    alex.assert_message_contains("[Steve → me] hello <red>there");
    alex.assert_heard("entity.experience_orb.pickup");
    assert!(jsons(&alex).join("\n").contains("/m Steve "));
    run(&alex, "/r hi back");
    steve.assert_message_contains("[Alex → me] hi back");
    run(&steve, "/m Alex again");
    alex.assert_message_contains("[Steve → me] again");

    // The console can whisper too, and players can answer it.
    assert!(
        server
            .console_command("msg Alex from the console")
            .executed()
    );
    alex.assert_message_contains("[Console → me] from the console");
    run(&alex, "/r ok");
    server.assert_console_contains("[Alex → me] ok");

    alex.disconnect();
    refused(&steve, "/r still there?");
    steve.assert_message_contains("Alex is not online any more.");
    steve.set_permission("essentials.msg", false);
    denied(&steve, "/msg Alex x");
    denied(&steve, "/r x");
}

#[test]
fn broadcast_is_for_ops() {
    let server = fast();
    let steve = server.add_player("Steve");
    let admin = server.add_op("Admin");
    denied(&steve, "/broadcast hi");
    run(&admin, "/broadcast <green>Server restarts soon");
    steve.assert_message_contains("[Broadcast] Server restarts soon");
    assert!(
        server
            .broadcasts()
            .iter()
            .any(|b| b == "[Broadcast] Server restarts soon"),
        "{:?}",
        server.broadcasts()
    );
    assert!(server.console_command("bc from console").executed());
    steve.assert_message_contains("[Broadcast] from console");
}

#[test]
fn msg_shows_the_chat_meta_prefix_from_perms() {
    let server = MockServer::builder()
        .plugin_config(ID, FAST)
        .built(&build_plugin("plugins/perms"))
        .built(&built())
        .build();
    let admin = server.add_op("Admin");
    let steve = server.add_player("Steve");
    let alex = server.add_player("Alex");
    run(&admin, "/perms group default setprefix &7[Member] ");
    run(&steve, "/msg Alex hi");
    alex.assert_message_contains("[[Member] Steve → me] hi");
    steve.assert_message_contains("[me → [Member] Alex] hi");
    // perms is the permission provider: its nodes decide essentials' permissions.
    run(
        &admin,
        "/perms group default permission set -essentials.msg",
    );
    denied(&steve, "/msg Alex hi");
    run(
        &admin,
        "/perms user Steve permission set essentials.homes.5",
    );
    for name in ["a", "b", "c", "d", "e"] {
        run(&steve, &format!("/sethome {name}"));
    }
    refused(&steve, "/sethome f");
    steve.assert_message_contains("at most 5 homes");
}

#[test]
fn sethome_costs_money_when_an_economy_is_installed() {
    let config = format!("{FAST}sethome_cost = 100\ndefault_max_homes = 5\n");
    // Without an economy: free (graceful degradation).
    let server = server_with(&config);
    let steve = server.add_player("Steve");
    run(&steve, "/sethome a");
    steve.assert_no_message_contains("charged");

    let server = MockServer::builder()
        .plugin_config(ID, &config)
        .built(&build_plugin(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/economy"
        )))
        .built(&built())
        .build();
    let steve = server.add_player("Steve");
    run(&steve, "/sethome a"); // 150 → 50
    steve.assert_message_contains("100 coins was charged for this home.");
    run(&steve, "/sethome a"); // moving a home is free
    steve.assert_message_contains("Home a moved here.");
    refused(&steve, "/sethome b");
    steve.assert_message_contains("A new home costs 100 coins: not enough coins");
    steve.set_permission("essentials.sethome.free", true);
    run(&steve, "/sethome b");
}

#[test]
fn french_messages() {
    let server = server_with(&format!("{FAST}lang = \"fr\"\n"));
    let steve = server.add_player("Steve");
    run(&steve, "/sethome base");
    steve.assert_message_contains("Résidence base définie.");
    refused(&steve, "/warp x");
    steve.assert_message_contains("Il n'existe aucun point nommé x.");
    let alex = server.add_player("Alex");
    run(&steve, "/tpa Alex");
    alex.assert_message_contains("[Accepter] [Refuser] (60s)");
}
