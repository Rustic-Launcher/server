//! Integration tests: the real `perms.wasm` (built for wasm32-wasip2 by `build_plugin`) in
//! `rusticmc-test`'s in-memory server. Every `/perms` command, the permission denial, the
//! join event, the permission-provider and chat-meta services, expiry, persistence, completion
//! and the French messages.

use std::time::Duration;

use rusticmc_test::prelude::*;

fn built() -> rusticmc_test::BuiltPlugin {
    build_plugin(env!("CARGO_MANIFEST_DIR"))
}

fn server() -> MockServer {
    MockServer::with_built(&built())
}

/// Runs a command as `p` and asserts it executed.
fn run(p: &MockPlayer, line: &str) {
    let r = p.command(line);
    assert!(
        r.executed(),
        "{line}: {r:?}\nlast messages: {:?}",
        p.messages()
    );
}

/// Runs a command that the plugin refuses (an error message in red).
fn refused(p: &MockPlayer, line: &str) {
    let r = p.command(line);
    assert!(
        matches!(r, CommandRun::Plugin(CommandOutcome::PluginError(_))),
        "{line}: {r:?}"
    );
}

#[test]
fn enables_creates_the_default_group_and_registers_perms() {
    let server = server();
    server.assert_enabled("perms");
    server.assert_console_contains("Perms enabled: 1 groups");
    assert!(server.has_command("perms") && server.has_command("perm"));
    let groups: serde_json::Value = server.storage_json("perms", "groups").unwrap();
    assert!(groups.get("default").is_some(), "{groups}");
}

#[test]
fn non_admins_are_refused_and_ops_allowed() {
    let server = server();
    let steve = server.add_player("Steve");
    assert!(matches!(
        steve.command("/perms"),
        CommandRun::Plugin(CommandOutcome::NoPermission)
    ));
    assert!(!steve.has_permission("perms.admin"));
    let admin = server.add_op("Admin");
    assert!(admin.has_permission("perms.admin"));
    run(&admin, "/perms");
    admin.assert_message_contains("[Perms] Commands");
    admin.assert_message_contains("/perms user <player> info");
    run(&admin, "/perms help");
    // The console has every permission.
    assert!(server.console_command("perms groups").executed());
    server.assert_console_contains("Groups (1)");
}

#[test]
fn joining_players_get_the_default_group() {
    let server = server();
    let steve = server.add_player("Steve");
    let key = format!("users/{}", steve.uuid());
    let user: serde_json::Value = server.storage_json("perms", &key).unwrap();
    assert_eq!(user["name"], "Steve");
    assert_eq!(user["groups"], serde_json::json!(["default"]));
}

#[test]
fn group_nodes_wildcards_negation_and_weights_drive_permission_checks() {
    let server = server();
    let admin = server.add_op("Admin");
    let steve = server.add_player("Steve");
    assert!(!steve.has_permission("shop.buy"));

    run(&admin, "/perms group default permission set shop.*");
    admin.assert_message_contains("Set shop.* to true for group default.");
    assert!(steve.has_permission("shop.buy"));
    assert!(steve.has_permission("shop.sell.all"));

    run(&admin, "/perms group default permission set -shop.admin");
    assert!(!steve.has_permission("shop.admin"));
    run(&admin, "/perms group default permission set shop.vip false");
    assert!(!steve.has_permission("shop.vip"));

    // A heavier group overrides the default group's negation.
    run(&admin, "/perms group vip create 10");
    admin.assert_message_contains("Created group vip (weight 10).");
    run(&admin, "/perms group vip permission set shop.vip true");
    run(&admin, "/perms group vip parent add default");
    admin.assert_message_contains("vip now inherits default.");
    run(&admin, "/perms user Steve group add vip");
    admin.assert_message_contains("Added Steve to group vip.");
    steve.assert_message_contains("Your rank is now vip.");
    assert!(steve.has_permission("shop.vip"));
    assert!(steve.has_permission("group.vip"));
    // Weight 10 → -5: default (0) now wins on the conflicting node.
    run(&admin, "/perms group vip setweight -5");
    admin.assert_message_contains("Weight of vip set to -5.");
    assert!(!steve.has_permission("shop.vip"));

    // User nodes beat every group; `*` grants everything.
    run(&admin, "/perms user Steve permission set shop.vip");
    assert!(steve.has_permission("shop.vip"));
    run(&admin, "/perms user Steve permission set *");
    assert!(steve.has_permission("anything.at.all"));
    run(&admin, "/perms user Steve permission unset *");
    admin.assert_message_contains("Unset * for Steve.");
    assert!(!steve.has_permission("anything.at.all"));
    run(&admin, "/perms user Steve permission unset nothing.here");
    admin.assert_message_contains("nothing.here is not set for Steve.");

    run(&admin, "/perms check Steve shop.buy");
    admin.assert_message_contains("Steve → shop.buy = true (from group default, key shop.*)");
    run(&admin, "/perms check Steve fly");
    admin.assert_message_contains("is undefined here");

    refused(&admin, "/perms user Steve permission set Bad..Node");
    admin.assert_message_contains("Invalid permission node");
}

#[test]
fn user_commands_info_groups_and_meta() {
    let server = server();
    let admin = server.add_op("Admin");
    let steve = server.add_player("Steve");
    run(&admin, "/perms group mod create 50");
    run(&admin, "/perms group mod setprefix &9[Mod] ");
    admin.assert_message_contains("Prefix of group mod set to: [Mod]");
    run(&admin, "/perms group mod setsuffix <gray>*");

    run(&admin, "/perms user Steve group add mod");
    run(&admin, "/perms user Steve group add mod");
    admin.assert_message_contains("Steve is already in group mod.");
    run(&admin, "/perms user Steve info");
    admin.assert_message_contains(&format!("Player Steve ({})", steve.uuid()));
    admin.assert_message_contains("Primary group: mod");
    admin.assert_message_contains("Groups: mod, default");
    admin.assert_message_contains("Prefix: [Mod]");
    admin.assert_message_contains("Suffix: *");

    run(&admin, "/perms user Steve setprefix <gold>[Steve] ");
    run(&admin, "/perms user Steve info");
    admin.assert_message_contains("Prefix: [Steve]");
    run(&admin, "/perms user Steve setprefix");
    admin.assert_message_contains("Prefix of Steve removed.");
    run(&admin, "/perms user Steve setsuffix !");
    admin.assert_message_contains("Suffix of Steve set to: !");

    run(&admin, "/perms user Steve group remove mod");
    admin.assert_message_contains("Removed Steve from group mod.");
    run(&admin, "/perms user Steve group remove mod");
    admin.assert_message_contains("Steve is not in group mod.");
    run(&admin, "/perms user Steve group set mod");
    admin.assert_message_contains("Steve is now only in group mod.");
    let user: serde_json::Value = server
        .storage_json("perms", &format!("users/{}", steve.uuid()))
        .unwrap();
    assert_eq!(user["groups"], serde_json::json!(["mod"]));

    refused(&admin, "/perms user Steve group add ghost");
    admin.assert_message_contains("The group ghost does not exist.");
    refused(&admin, "/perms user Nobody info");
    admin.assert_message_contains("Unknown player Nobody");

    // Known offline players are still found by name.
    steve.disconnect();
    run(&admin, "/perms user steve info");
    admin.assert_message_contains("Primary group: mod");
}

#[test]
fn group_commands() {
    let server = server();
    let admin = server.add_op("Admin");
    run(&admin, "/perms group builder create");
    refused(&admin, "/perms group builder create");
    admin.assert_message_contains("The group builder already exists.");
    refused(&admin, "/perms group bad.name create");
    admin.assert_message_contains("Invalid name bad.name");

    run(&admin, "/perms group builder permission set worldedit.*");
    run(
        &admin,
        "/perms group builder permission settemp fly true 1h",
    );
    admin.assert_message_contains("Set fly to true for group builder for 1h.");
    run(&admin, "/perms group builder parent add default");
    run(&admin, "/perms group builder parent add default");
    admin.assert_message_contains("builder already inherits default.");
    refused(&admin, "/perms group default parent add builder");
    admin.assert_message_contains("would make a loop");
    run(&admin, "/perms group builder info");
    admin.assert_message_contains("Group builder");
    admin.assert_message_contains("Weight: 0");
    admin.assert_message_contains("Parents: default");
    admin.assert_message_contains("worldedit.* = true");
    admin.assert_message_contains("fly = true (expires in ");
    run(&admin, "/perms group builder permission unset fly");
    admin.assert_message_contains("Unset fly for group builder.");
    run(&admin, "/perms group builder parent remove default");
    admin.assert_message_contains("builder no longer inherits default.");
    run(&admin, "/perms group builder parent remove default");
    admin.assert_message_contains("builder does not inherit default.");
    run(&admin, "/perms group builder setprefix <green>[B]");
    run(&admin, "/perms group builder setprefix");
    admin.assert_message_contains("Prefix of group builder removed.");
    run(&admin, "/perms group builder setsuffix");
    admin.assert_message_contains("Suffix of group builder removed.");

    run(&admin, "/perms groups");
    admin.assert_message_contains("Groups (2)");
    let json: Vec<String> = admin
        .messages_json()
        .iter()
        .map(|t| t.as_json().to_owned())
        .collect();
    assert!(
        json.iter().any(|j| j.contains("/perms group builder info")),
        "{json:?}"
    );

    refused(&admin, "/perms group default delete");
    admin.assert_message_contains("The default group default cannot be deleted.");
    run(&admin, "/perms group builder delete");
    admin.assert_message_contains("Deleted group builder.");
    refused(&admin, "/perms group builder info");
}

#[test]
fn tracks_promote_and_demote() {
    let server = server();
    let admin = server.add_op("Admin");
    let steve = server.add_player("Steve");
    for (g, w) in [("helper", 10), ("mod", 20), ("admin", 30)] {
        run(&admin, &format!("/perms group {g} create {w}"));
    }
    run(&admin, "/perms track staff create");
    admin.assert_message_contains("Created track staff.");
    refused(&admin, "/perms track staff create");
    refused(&admin, "/perms user Steve promote staff");
    admin.assert_message_contains("The track staff has no group.");
    run(&admin, "/perms track staff append helper");
    run(&admin, "/perms track staff append admin");
    run(&admin, "/perms track staff insert mod 2");
    admin.assert_message_contains("Inserted mod at position 2 of track staff.");
    refused(&admin, "/perms track staff append mod");
    admin.assert_message_contains("mod is already on track staff.");
    run(&admin, "/perms track staff info");
    admin.assert_message_contains("Track staff: helper → mod → admin");

    run(&admin, "/perms user Steve promote staff");
    admin.assert_message_contains("Steve joins track staff at helper.");
    run(&admin, "/perms user Steve promote staff");
    admin.assert_message_contains("Promoted Steve from helper to mod on staff.");
    steve.assert_message_contains("Your rank is now mod.");
    assert!(steve.has_permission("group.mod") && !steve.has_permission("group.helper"));
    run(&admin, "/perms user Steve promote staff");
    run(&admin, "/perms user Steve promote staff");
    admin.assert_message_contains("Steve is already at the top of staff.");
    run(&admin, "/perms user Steve demote staff");
    admin.assert_message_contains("Demoted Steve from admin to mod on staff.");
    run(&admin, "/perms user Steve demote staff");
    run(&admin, "/perms user Steve demote staff");
    admin.assert_message_contains("Steve left track staff (removed from helper).");
    run(&admin, "/perms user Steve demote staff");
    admin.assert_message_contains("Steve is not on track staff.");

    run(&admin, "/perms tracks");
    admin.assert_message_contains("Tracks (1)");
    run(&admin, "/perms track staff remove mod");
    admin.assert_message_contains("Removed mod from track staff.");
    refused(&admin, "/perms track staff remove mod");
    run(&admin, "/perms track staff delete");
    admin.assert_message_contains("Deleted track staff.");
    refused(&admin, "/perms track staff info");
    admin.assert_message_contains("The track staff does not exist.");
}

#[test]
fn temporary_nodes_and_groups_expire_on_the_timer() {
    let server = server();
    let admin = server.add_op("Admin");
    let steve = server.add_player("Steve");
    run(&admin, "/perms group vip create");
    run(
        &admin,
        "/perms user Steve permission settemp kit.daily true 1s",
    );
    admin.assert_message_contains("for Steve for 1s.");
    run(&admin, "/perms user Steve group addtemp vip 1s");
    admin.assert_message_contains("Added Steve to group vip for 1s.");
    assert!(steve.has_permission("kit.daily"));
    assert!(steve.has_permission("group.vip"));
    run(&admin, "/perms user Steve info");
    admin.assert_message_contains("(expires in");

    // Expiry is wall-clock time (it survives restarts); the check runs on a tick timer.
    std::thread::sleep(Duration::from_millis(2100));
    assert!(!steve.has_permission("kit.daily"));
    server.tick(20);
    steve.assert_message_contains("Your temporary permission kit.daily has expired.");
    steve.assert_message_contains("Your temporary group vip has expired.");
    let user: serde_json::Value = server
        .storage_json("perms", &format!("users/{}", steve.uuid()))
        .unwrap();
    assert!(user.get("temp_nodes").is_none(), "{user}");
    assert!(user.get("temp_groups").is_none(), "{user}");
}

#[test]
fn data_survives_a_reload_and_reload_command_rereads_storage() {
    let server = server();
    let admin = server.add_op("Admin");
    let steve = server.add_player("Steve");
    run(&admin, "/perms group default permission set home.use");
    server.reload("perms").unwrap();
    assert!(steve.has_permission("home.use"));
    run(&admin, "/perms reload");
    admin.assert_message_contains("Reloaded: 1 groups, 2 players, 0 tracks.");
}

#[test]
fn export_and_import_round_trip_through_perms_toml() {
    let server = server();
    let admin = server.add_op("Admin");
    let steve = server.add_player("Steve");
    run(&admin, "/perms group default permission set build");
    run(&admin, "/perms export");
    admin.assert_message_contains("Exported to perms.toml (1 groups, 2 players, 0 tracks).");
    let file = server.data_dir("perms").join("perms.toml");
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(text.contains("[groups.default]"), "{text}");

    // An admin edits the file, then imports it.
    let edited = text.replace("build = true", "build = false");
    let edited = format!("{edited}\n[groups.guest]\nweight = -1\n");
    std::fs::write(&file, edited).unwrap();
    run(&admin, "/perms import");
    admin.assert_message_contains("Imported perms.toml: 2 groups");
    assert!(!steve.has_permission("build"));
    let groups: serde_json::Value = server.storage_json("perms", "groups").unwrap();
    assert!(groups.get("guest").is_some());

    // A broken file is refused and nothing changes.
    std::fs::write(&file, "[groups.x]\nparents = [\"nope\"]\n").unwrap();
    refused(&admin, "/perms import");
    admin.assert_message_contains("unknown group `nope`");
    assert!(
        server
            .storage_json::<serde_json::Value>("perms", "groups")
            .unwrap()
            .get("guest")
            .is_some()
    );
}

#[test]
fn completion_of_players_groups_tracks_and_nodes() {
    let server = server();
    let admin = server.add_op("Admin");
    server.add_player("Steve");
    run(&admin, "/perms group vip create");
    run(&admin, "/perms track staff create");
    run(&admin, "/perms group vip permission set essentials.home");
    let users = admin.complete("perms user St");
    assert!(users.contains(&"Steve".to_owned()), "{users:?}");
    let groups = admin.complete("perms group v");
    assert!(groups.contains(&"vip".to_owned()), "{groups:?}");
    let tracks = admin.complete("perms track ");
    assert!(tracks.contains(&"staff".to_owned()), "{tracks:?}");
    let nodes = admin.complete("perms group vip permission unset ess");
    assert!(nodes.contains(&"essentials.home".to_owned()), "{nodes:?}");
}

#[test]
fn chat_meta_and_provider_services_answer_other_plugins() {
    let server = server();
    let admin = server.add_op("Admin");
    let steve = server.add_player("Steve");
    run(&admin, "/perms group default setprefix &7[Member] ");
    for iface in [
        "rustic:services/permission-provider@0.1.0",
        "rustic:services/chat-meta@0.1.0",
    ] {
        assert_eq!(
            server
                .runtime()
                .shared()
                .services()
                .provider(iface)
                .map(|p| p.to_string())
                .as_deref(),
            Some("perms"),
            "{iface}"
        );
    }
    // The server's own checks go through the provider.
    run(&admin, "/perms user Steve permission set -perms.admin");
    assert!(!steve.has_permission("perms.admin"));
}

#[test]
fn french_messages() {
    let server = MockServer::builder()
        .plugin_config("perms", "config_version = 1\nlang = \"fr\"\n")
        .built(&built())
        .build();
    let admin = server.add_op("Admin");
    run(&admin, "/perms group vip create 5");
    admin.assert_message_contains("Groupe vip créé (poids 5).");
    refused(&admin, "/perms group ghost info");
    admin.assert_message_contains("Le groupe ghost n'existe pas.");
}
