//! Integration tests: the real `chat_format.wasm` in `rusticmc-test`'s in-memory server.
//! Formats (default, group, chat-meta prefixes with the real `perms.wasm`, degraded without it),
//! hover/click on names, colours per permission, mentions, every anti-spam filter in both
//! modes, the mute commands with persistence and expiry, join/quit lines, reload, completion,
//! permission denials and the French messages.

use std::time::Duration;

use rusticmc_test::BuiltPlugin;
use rusticmc_test::prelude::*;

const ID: &str = "chat-format";

fn built() -> BuiltPlugin {
    build_plugin(env!("CARGO_MANIFEST_DIR"))
}

fn server() -> MockServer {
    MockServer::with_built(&built())
}

/// The shipped config with some lines replaced (`key = value` under a table header).
fn server_with(edit: impl Fn(String) -> String) -> MockServer {
    let config = edit(include_str!("../config.toml").to_owned());
    MockServer::builder()
        .plugin_config(ID, &config)
        .built(&built())
        .build()
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

/// The last chat line `p` received, as JSON.
fn last_json(p: &MockPlayer) -> String {
    p.messages_json()
        .last()
        .map(|t| t.as_json().to_owned())
        .unwrap_or_default()
}

#[test]
fn enables_with_commands_and_declared_defaults() {
    let server = server();
    server.assert_enabled(ID);
    server.assert_console_contains("chat-format enabled");
    for cmd in [
        "mute",
        "unmute",
        "mutes",
        "cmute",
        "cunmute",
        "cmutes",
        "chatformat",
    ] {
        assert!(server.has_command(cmd), "/{cmd}");
    }
    let steve = server.add_player("Steve");
    assert!(steve.has_permission("chatformat.mention"));
    for node in [
        "chatformat.color",
        "chatformat.mute",
        "chatformat.admin",
        "chatformat.bypass.spam",
        "chatformat.bypass.links",
        "chatformat.bypass.filter",
        "chatformat.bypass.caps",
    ] {
        assert!(!steve.has_permission(node), "{node}");
    }
    let op = server.add_op("Admin");
    assert!(op.has_permission("chatformat.mute"));
    assert!(op.has_permission("chatformat.mute.exempt"));
}

#[test]
fn default_format_with_hover_and_click_on_the_name() {
    let server = server();
    let steve = server.add_player("Steve");
    let alex = server.add_player("Alex");
    steve.set_ping(42);
    assert!(steve.chat("hello there"));
    alex.assert_message_contains("Steve: hello there");
    let json = last_json(&alex);
    assert!(json.contains("suggest_command"), "{json}");
    assert!(json.contains("/msg Steve "), "{json}");
    assert!(json.contains("show_text"), "{json}");
    assert!(json.contains("42 ms"), "{json}");
    assert!(json.contains("overworld"), "{json}");
}

#[test]
fn colours_need_the_permission() {
    let server = server();
    let steve = server.add_player("Steve");
    let alex = server.add_player("Alex");
    steve.chat("<red>red &ahey");
    alex.assert_message_contains("Steve: <red>red &ahey");

    steve.set_permission("chatformat.color", true);
    steve.chat("<red>red &ahey <click:run_command:'/op Steve'>x");
    alex.assert_message_contains("Steve: red hey <click:run_command:'/op Steve'>x");
    let json = last_json(&alex);
    assert!(json.contains("\"color\":\"red\""), "{json}");
    assert!(json.contains("\"color\":\"green\""), "{json}");
    assert!(!json.contains("\"action\":\"run_command\""), "{json}");
}

#[test]
fn group_formats_and_prefixes_from_config() {
    let server = server();
    let steve = server.add_player("Steve");
    let alex = server.add_player("Alex");
    steve.set_permission("chatformat.group.admin", true);
    steve.chat("hi");
    alex.assert_message_contains("[Admin] Steve: hi");
    steve.set_permission("chatformat.group.admin", false);
    steve.set_permission("chatformat.group.vip", true);
    steve.chat("hey");
    alex.assert_message_contains("[VIP] Steve: hey");
    assert!(last_json(&alex).contains("\"color\":\"gold\""));
}

#[test]
fn prefixes_and_groups_come_from_perms_when_installed() {
    let server = MockServer::builder()
        .built(&build_plugin("plugins/perms"))
        .built(&built())
        .build();
    server.assert_enabled("perms");
    server.assert_enabled(ID);
    let admin = server.add_op("Admin");
    let steve = server.add_player("Steve");
    let alex = server.add_player("Alex");
    // (Command arguments are trimmed: a prefix cannot end with a space.)
    run(&admin, "/perms group default setprefix &7[Member]");
    steve.chat("hello");
    alex.assert_message_contains("[Member]Steve: hello");
    assert!(last_json(&alex).contains("\"color\":\"gray\""));

    // The provider's primary group picks the group format of config.toml.
    run(&admin, "/perms group admin create 100");
    run(&admin, "/perms user Steve group set admin");
    steve.chat("now admin");
    alex.assert_message_contains("[Admin] Steve: now admin");
    // A personal suffix from chat-meta.
    run(&admin, "/perms user Steve setsuffix &c*");
    steve.chat("with suffix");
    alex.assert_message_contains("Steve*: with suffix");
}

#[test]
fn mentions_highlight_and_ping() {
    let server = server();
    let steve = server.add_player("Steve");
    let alex = server.add_player("Alex");
    steve.chat("hey @alex look, and @nobody");
    alex.assert_message_contains("Steve: hey @Alex look, and @nobody");
    assert!(last_json(&alex).contains("\"color\":\"yellow\""));
    alex.assert_heard("entity.experience_orb.pickup");
    assert_eq!(
        alex.last_action_bar().as_deref(),
        Some("Steve mentioned you")
    );
    // Mentioning yourself plays nothing.
    steve.chat("me @Steve");
    assert!(steve.sounds().is_empty());
    // Without the permission, no highlight and no sound.
    let before = alex.sounds().len();
    steve.set_permission("chatformat.mention", false);
    steve.chat("again @Alex");
    assert!(!last_json(&alex).contains("yellow"));
    assert_eq!(alex.sounds().len(), before);
}

#[test]
fn rate_limit_and_bypass() {
    let server = server();
    let steve = server.add_player("Steve");
    for i in 0..5 {
        assert!(steve.chat(&format!("message number {i}")), "{i}");
    }
    assert!(!steve.chat("one too many"));
    steve.assert_message_contains("Slow down!");
    steve.set_permission("chatformat.bypass.spam", true);
    assert!(steve.chat("one too many"));
}

#[test]
fn duplicate_messages_are_refused() {
    let server = server();
    let steve = server.add_player("Steve");
    assert!(steve.chat("buy my stuff"));
    assert!(!steve.chat("Buy my stuff!!"));
    steve.assert_message_contains("Please do not repeat the same message.");
    assert!(steve.chat("something else"));
}

#[test]
fn caps_are_lowered_or_blocked() {
    let server = server();
    let steve = server.add_player("Steve");
    let alex = server.add_player("Alex");
    assert!(steve.chat("WHY IS NOBODY HERE"));
    alex.assert_message_contains("Steve: why is nobody here");
    assert!(steve.chat("OK"));
    alex.assert_message_contains("Steve: OK");

    let server = server_with(|c| c.replace("action = \"lowercase\"", "action = \"block\""));
    let steve = server.add_player("Steve");
    assert!(!steve.chat("WHY IS NOBODY HERE"));
    steve.assert_message_contains("Please do not shout");
    steve.set_permission("chatformat.bypass.caps", true);
    assert!(steve.chat("WHY IS NOBODY HERE"));
}

#[test]
fn links_are_blocked_or_masked_with_a_whitelist() {
    let server = server();
    let steve = server.add_player("Steve");
    let alex = server.add_player("Alex");
    assert!(!steve.chat("join play.evil.net now"));
    steve.assert_message_contains("Links are not allowed in chat.");
    assert!(steve.chat("wiki at https://wiki.rustic.example.org/start"));
    alex.assert_message_contains("https://wiki.rustic.example.org/start");
    steve.set_permission("chatformat.bypass.links", true);
    assert!(steve.chat("ok play.evil.net"));

    let server = server_with(|c| {
        c.replace(
            "action = \"block\"\nreplacement",
            "action = \"mask\"\nreplacement",
        )
    });
    let steve = server.add_player("Steve");
    let alex = server.add_player("Alex");
    assert!(steve.chat("join 10.0.0.1 now"));
    alex.assert_message_contains("Steve: join [link removed] now");
}

#[test]
fn words_are_masked_or_blocked() {
    let server = server();
    let steve = server.add_player("Steve");
    let alex = server.add_player("Alex");
    assert!(steve.chat("what the Heck"));
    alex.assert_message_contains("Steve: what the ****");
    steve.set_permission("chatformat.bypass.filter", true);
    assert!(steve.chat("darn it"));
    alex.assert_message_contains("Steve: darn it");

    let server = server_with(|c| c.replace("action = \"mask\"", "action = \"block\""));
    let steve = server.add_player("Steve");
    assert!(!steve.chat("darn"));
    steve.assert_message_contains("forbidden word");
}

#[test]
fn mute_unmute_and_mutes() {
    let server = server();
    let admin = server.add_op("Admin");
    let steve = server.add_player("Steve");
    let alex = server.add_player("Alex");
    denied(&steve, "/mute Alex");
    denied(&steve, "/unmute Alex");
    denied(&steve, "/mutes");

    run(&admin, "/mutes");
    admin.assert_message_contains("Nobody is muted.");
    run(&admin, "/mute Steve 10m spamming the chat");
    admin.assert_message_contains("Muted Steve for 10m. spamming the chat");
    steve.assert_message_contains("You have been muted for 10m by Admin. spamming the chat");
    assert!(!steve.chat("let me talk"));
    steve.assert_message_contains("You are muted for");
    alex.assert_no_message_contains("let me talk");
    // No whispering around it.
    assert!(matches!(
        steve.command("/msg Alex psst"),
        CommandRun::Cancelled
    ));
    steve.assert_message_contains("You cannot use this command while muted.");
    // Stored.
    let rec: serde_json::Value = server
        .storage_json(ID, &format!("mutes/{}", steve.uuid()))
        .expect("mute record");
    assert_eq!(rec["name"], "Steve");
    assert_eq!(rec["by"], "Admin");

    run(&admin, "/mutes");
    admin.assert_message_contains("Muted players (1):");
    admin.assert_message_contains("- Steve (10m) spamming the chat");

    // A reason without a duration is a permanent mute.
    run(&admin, "/mute Alex being rude");
    alex.assert_message_contains("You have been muted by Admin. being rude");
    run(&admin, "/mutes");
    admin.assert_message_contains("- Alex (permanent) being rude");

    run(&admin, "/unmute Steve");
    admin.assert_message_contains("Steve can chat again.");
    steve.assert_message_contains("You can chat again.");
    assert!(steve.chat("free at last"));
    refused(&admin, "/unmute Steve");
    admin.assert_message_contains("Steve is not muted.");
    refused(&admin, "/mute Nobody");
    admin.assert_message_contains("Unknown player: Nobody");
    // Ops are exempt by default.
    let other = server.add_op("Other");
    refused(&admin, "/mute Other");
    admin.assert_message_contains("Other cannot be muted.");
    drop(other);
    // The console can mute too.
    assert!(server.console_command("mute Steve 1h").executed());
    assert!(!steve.chat("again"));
}

#[test]
fn mutes_survive_a_reload_and_staff_is_notified() {
    let server = server();
    let admin = server.add_op("Admin");
    let mod_ = server.add_player("Mod");
    mod_.set_permission("chatformat.mute.notify", true);
    let steve = server.add_player("Steve");
    // The aliases (Horizon's own chat filter owns `/mute` and `/unmute` there).
    run(&admin, "/cmute Steve 2h");
    mod_.assert_message_contains("Admin muted Steve (2h).");
    run(&admin, "/cmutes");
    admin.assert_message_contains("- Steve (2h)");
    server.reload(ID).unwrap();
    assert!(!steve.chat("still muted?"));
    steve.assert_message_contains("You are muted for");
}

#[test]
fn mutes_expire() {
    let server = server();
    let admin = server.add_op("Admin");
    let steve = server.add_player("Steve");
    let alex = server.add_player("Alex");
    run(&admin, "/mute Steve 1s");
    run(&admin, "/mute Alex 1s");
    assert!(!steve.chat("muted"));
    // Mutes use the wall clock (they persist across restarts).
    std::thread::sleep(Duration::from_millis(1100));
    // The sweep every 5 seconds lifts it and tells the player...
    server.tick(100);
    alex.assert_message_contains("You can chat again.");
    assert_eq!(
        server.storage(ID, &format!("mutes/{}", alex.uuid())),
        None,
        "record deleted"
    );
    // ...and a chat line lifts it as well.
    assert!(steve.chat("unmuted"));
}

#[test]
fn join_first_join_and_quit_lines() {
    let server = server();
    let alex = server.add_player("Alex");
    let steve = server.add_player("Steve");
    alex.assert_message_contains("Welcome Steve to the server!");
    steve.disconnect();
    alex.assert_message_contains("[-] Steve");
    let _steve = server.add_player("Steve");
    alex.assert_message_contains("[+] Steve");
    assert_eq!(_steve.data_json::<bool>(ID, "seen"), Some(true));

    // "vanilla" keeps the server's line, empty hides it.
    let server = server_with(|c| {
        c.replace(
            "quit_message = \"<dark_gray>[<red>-</red>]</dark_gray> <gray>{name}\"",
            "quit_message = \"\"",
        )
        .replace(
            "first_message = \"<light_purple>Welcome <white>{name}</white> to the server!\"",
            "first_message = \"vanilla\"",
        )
    });
    let alex = server.add_player("Alex");
    let steve = server.add_player("Steve");
    alex.assert_no_message_contains("Welcome");
    alex.clear_messages();
    steve.disconnect();
    alex.assert_no_message_contains("Steve");
}

#[test]
fn reload_command() {
    let server = server();
    let admin = server.add_op("Admin");
    let steve = server.add_player("Steve");
    denied(&steve, "/chatformat reload");
    std::fs::write(
        server.data_dir(ID).join("config.toml"),
        include_str!("../config.toml").replace(
            "default = \"{prefix}{name}{suffix}<dark_gray>:</dark_gray> <white>{message}\"",
            "default = \"{name} says {message}\"",
        ),
    )
    .unwrap();
    run(&admin, "/chatformat reload");
    admin.assert_message_contains("chat-format configuration reloaded.");
    steve.chat("hi");
    admin.assert_message_contains("Steve says hi");
    // A broken file is reported and the old config kept.
    std::fs::write(server.data_dir(ID).join("config.toml"), "spam = [").unwrap();
    refused(&admin, "/chatformat reload");
    admin.assert_message_contains("Reload failed");
    steve.chat("still");
    admin.assert_message_contains("Steve says still");
}

#[test]
fn completion_of_player_names() {
    let server = server();
    let admin = server.add_op("Admin");
    let _alex = server.add_player("Alex");
    assert_eq!(admin.complete("mute Al"), vec!["Alex".to_owned()]);
    run(&admin, "/mute Alex");
    assert_eq!(admin.complete("unmute "), vec!["Alex".to_owned()]);
}

#[test]
fn french_messages() {
    let server = server();
    let admin = server.add_op("Admin");
    let steve = server.add_player("Steve");
    steve.set_locale("fr_fr");
    admin.set_locale("fr_ca");
    run(&admin, "/mute Steve 5m");
    admin.assert_message_contains("Steve est réduit au silence pour 5m.");
    steve.assert_message_contains("Admin vous a réduit au silence pour 5m.");
    assert!(!steve.chat("salut"));
    steve.assert_message_contains("Vous êtes réduit au silence pour");
    run(&admin, "/unmute Steve");
    steve.assert_message_contains("Vous pouvez de nouveau écrire.");
    assert!(steve.chat("salut"));
    assert!(!steve.chat("salut"));
    steve.assert_message_contains("Merci de ne pas répéter le même message.");
}
