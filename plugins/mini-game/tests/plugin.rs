//! Integration tests: the real `mini_game.wasm` in `rusticmc-test`'s in-memory server.
//! Every `/mg` sub-command and its denials, full Spleef and King of the Hill rounds (lobby,
//! countdown with boss bar and titles, start teleports, live sidebar, world changes, win,
//! stats, rewards with an economy provider, back to the lobby), countdown abort, join errors,
//! disconnects and plugin disable mid-game, damage/place protection and French.

use rusticmc_test::BuiltPlugin;
use rusticmc_test::prelude::*;

const ID: &str = "mini-game";
/// Short timings: 3 s countdown, 10 s rounds, 2 s end; KotH to 3 points; 3 players at most.
const FAST: &str = "config_version = 1\nmin_players = 2\nmax_players = 3\ncountdown_seconds = 3\n\
round_seconds = 10\nend_seconds = 2\ndefault_radius = 4\n[koth]\npoints_to_win = 3\n";
const OBJECTIVE: &str = "mini-game.g_arena";

fn built() -> BuiltPlugin {
    build_plugin(env!("CARGO_MANIFEST_DIR"))
}

fn server() -> MockServer {
    MockServer::builder()
        .plugin_config(ID, FAST)
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

fn at(server: &MockServer, x: f64, y: f64, z: f64) -> Location {
    server.overworld().location(x, y, z)
}

fn near(p: &MockPlayer, x: f64, z: f64, within: f64) -> bool {
    let l = p.location();
    (l.x - x).abs() <= within && (l.z - z).abs() <= within
}

/// An admin creates `arena` at (0.5, 65, 0.5) with its lobby at (30.5, 65, 30.5).
fn setup(server: &MockServer, mode: &str) -> MockPlayer {
    let admin = server.add_op("Admin");
    admin.set_location(at(server, 0.5, 65.0, 0.5));
    run(&admin, &format!("/mg create arena {mode}"));
    admin.set_location(at(server, 30.5, 65.0, 30.5));
    run(&admin, "/mg setlobby arena");
    admin.clear_messages();
    admin
}

/// Two players join and the countdown runs out: the round has started.
fn start_two(server: &MockServer) -> (MockPlayer, MockPlayer) {
    let steve = server.add_player("Steve");
    let alex = server.add_player("Alex");
    run(&steve, "/mg join arena");
    run(&alex, "/mg join arena");
    server.tick(60);
    (steve, alex)
}

#[test]
fn enables_with_command_and_permissions() {
    let server = server();
    server.assert_enabled(ID);
    server.assert_console_contains("mini-game enabled: 0 arenas");
    assert!(server.has_command("mg"));
    assert!(server.has_command("minigame"));
    let steve = server.add_player("Steve");
    assert!(steve.has_permission("minigame.play"));
    assert!(!steve.has_permission("minigame.admin"));
    for line in [
        "/mg create a",
        "/mg setspawn a",
        "/mg setlobby a",
        "/mg setarea a 3",
        "/mg delete a",
        "/mg start a",
        "/mg stop a",
    ] {
        denied(&steve, line);
    }
    run(&steve, "/mg");
    steve.assert_message_contains("/mg join [arena]");
    steve.assert_no_message_contains("Admin:");
    steve.set_permission("minigame.play", false);
    denied(&steve, "/mg join");
}

#[test]
fn arena_admin_commands_and_floor() {
    let server = server();
    let admin = server.add_op("Admin");
    let w = server.overworld();
    admin.set_location(at(&server, 0.5, 65.0, 0.5));
    run(&admin, "/mg help");
    admin.assert_message_contains("Admin:");
    refused(&admin, "/mg create Bad-Name");
    admin.assert_message_contains("Arena names use a-z");
    refused(&admin, "/mg create arena dance");
    admin.assert_message_contains("Modes: spleef, koth.");
    run(&admin, "/mg create arena");
    admin.assert_message_contains("Arena arena (Spleef) created here, radius 4.");
    // The floor: a disc of snow at the block under the admin.
    assert_eq!(w.block_id((0, 64, 0)), "minecraft:snow_block");
    assert_eq!(w.block_id((4, 64, 0)), "minecraft:snow_block");
    assert_eq!(w.block_id((5, 64, 0)), "minecraft:grass_block");
    assert_eq!(w.block_id((4, 64, 4)), "minecraft:grass_block");
    refused(&admin, "/mg create arena");
    admin.assert_message_contains("The arena arena already exists.");

    admin.set_location(at(&server, 10.5, 65.0, 10.5));
    run(&admin, "/mg setspawn arena");
    admin.assert_message_contains("Start point of arena set here.");
    run(&admin, "/mg setlobby arena");
    admin.assert_message_contains("Lobby of arena set here.");
    run(&admin, "/mg setarea arena 2");
    admin.assert_message_contains("Area of arena: radius 2");
    assert_eq!(w.block_id((12, 64, 10)), "minecraft:snow_block");
    let stored: serde_json::Value = server.storage_json(ID, "arenas/arena").unwrap();
    assert_eq!(stored["mode"], "spleef");
    assert_eq!(stored["radius"], 2);
    assert_eq!(stored["center"]["x"], 10);
    assert_eq!(stored["lobby"]["x"], 10.5);
    refused(&admin, "/mg setspawn nope");
    admin.assert_message_contains("Unknown arena: nope");

    run(&admin, "/mg create hill koth");
    run(&admin, "/mg list");
    admin.assert_message_contains("Arenas:");
    admin.assert_message_contains("- arena Spleef · waiting · 0/3");
    admin.assert_message_contains("- hill King of the Hill · waiting · 0/3");
    let json = admin.messages_json().last().unwrap().as_json().to_owned();
    assert!(json.contains("/mg join hill"), "{json}");
    assert_eq!(
        admin.complete("mg join "),
        vec!["arena".to_owned(), "hill".to_owned()]
    );

    // Arenas survive a reload.
    server.reload(ID).unwrap();
    server.assert_console_contains("mini-game enabled: 2 arenas");
    run(&admin, "/mg delete hill");
    admin.assert_message_contains("Arena hill deleted.");
    assert_eq!(server.storage(ID, "arenas/hill"), None);
    assert!(!server.console_command("mg create x").executed());
}

#[test]
fn a_full_spleef_round() {
    let server = server();
    let _admin = setup(&server, "spleef");
    let steve = server.add_player("Steve");
    let alex = server.add_player("Alex");
    run(&steve, "/mg join arena");
    steve.assert_message_contains("You joined arena (1/3).");
    assert!(near(&steve, 30.5, 30.5, 0.01), "in the lobby");
    let bars = steve.boss_bars();
    assert_eq!(bars.len(), 1);
    assert!(
        bars[0].title.to_plain().contains("Waiting for players 1/2"),
        "{:?}",
        bars[0].title
    );

    run(&alex, "/mg join arena");
    steve.assert_message_contains("Alex joined (2/3).");
    server.tick(20);
    assert!(
        steve.boss_bars()[0]
            .title
            .to_plain()
            .contains("Starting in 2 s")
    );
    assert!(steve.titles().iter().any(|t| t.title.to_plain() == "2"));
    steve.assert_heard("block.note_block.hat");
    server.tick(40); // start

    assert!(steve.titles().iter().any(|t| t.title.to_plain() == "Go!"));
    assert!(
        near(&steve, 0.5, 0.5, 2.5) && near(&alex, 0.5, 0.5, 2.5),
        "at the start"
    );
    assert_eq!(steve.game_mode(), GameMode::Survival);
    assert_eq!(steve.count("diamond_shovel"), 1);
    assert_eq!(
        server.displayed_objective("sidebar").as_deref(),
        Some(OBJECTIVE)
    );
    assert_eq!(server.score(OBJECTIVE, "Time left"), Some(10));
    assert_eq!(server.score(OBJECTIVE, "Steve"), Some(0));
    assert!(
        steve.boss_bars()[0]
            .title
            .to_plain()
            .contains("Spleef · 0:10 left")
    );

    // Digging: floor blocks count, nothing drops; anything else is refused.
    assert!(steve.break_block((2, 64, 0)));
    assert!(steve.break_block((0, 64, 2)));
    assert_eq!(server.overworld().block_id((2, 64, 0)), "minecraft:air");
    assert!(server.overworld().dropped_items().is_empty());
    assert!(!steve.break_block((2, 63, 0)));
    assert!(!steve.place_block((6, 65, 6), "stone"));
    assert_eq!(server.score(OBJECTIVE, "Steve"), Some(2));
    server.tick(20);
    assert_eq!(server.score(OBJECTIVE, "Time left"), Some(9));
    // No damage in a game.
    let health = steve.health();
    steve.damage(5.0);
    assert_eq!(steve.health(), health);

    // Alex falls through: Steve is the last one standing.
    alex.set_location(at(&server, 2.5, 60.0, 0.5));
    server.tick(1);
    steve.assert_message_contains("Alex fell! 1 still standing.");
    assert!(
        alex.titles()
            .iter()
            .any(|t| t.title.to_plain() == "You fell!")
    );
    assert!(near(&alex, 30.5, 30.5, 0.01));
    assert_eq!(alex.count("diamond_shovel"), 0);
    assert!(
        steve
            .titles()
            .iter()
            .any(|t| t.title.to_plain() == "Victory!")
    );
    assert!(
        alex.titles()
            .iter()
            .any(|t| t.title.to_plain() == "Game over" && t.subtitle.to_plain() == "Winner: Steve")
    );
    steve.assert_message_contains("Steve won arena!");
    steve.assert_heard("ui.toast.challenge_complete");
    assert!(
        steve.boss_bars()[0]
            .title
            .to_plain()
            .contains("Winner: Steve")
    );
    let stats: serde_json::Value = server
        .storage_json(ID, &format!("stats/{}", steve.uuid()))
        .unwrap();
    assert_eq!(
        (
            stats["wins"].clone(),
            stats["games"].clone(),
            stats["points"].clone()
        ),
        (1.into(), 1.into(), 2.into())
    );
    // No economy installed: no reward line.
    steve.assert_no_message_contains("Reward");

    server.tick(40); // end_seconds
    assert!(near(&steve, 30.5, 30.5, 0.01), "back in the lobby");
    assert_eq!(steve.count("diamond_shovel"), 0);
    assert!(steve.boss_bars().is_empty());
    assert!(server.objective(OBJECTIVE).is_none());
    assert_eq!(
        server.overworld().block_id((2, 64, 0)),
        "minecraft:snow_block",
        "floor restored"
    );
    run(&steve, "/mg list");
    steve.assert_message_contains("- arena Spleef · waiting · 0/3");
}

#[test]
fn a_full_king_of_the_hill_round_with_rewards() {
    let server = MockServer::builder()
        .plugin_config(ID, FAST)
        .built(&build_plugin(
            "plugins/essentials-lite/tests/fixtures/economy",
        ))
        .built(&built())
        .build();
    let _admin = setup(&server, "koth");
    let (steve, alex) = start_two(&server);
    assert_eq!(steve.game_mode(), GameMode::Adventure);
    assert_eq!(steve.count("diamond_shovel"), 0);
    // Both on the hill: contested.
    steve.set_location(at(&server, 0.5, 65.0, 0.5));
    alex.set_location(at(&server, 1.5, 65.0, 0.5));
    server.tick(20);
    assert_eq!(steve.last_action_bar().as_deref(), Some("Contested!"));
    assert_eq!(server.score(OBJECTIVE, "Steve"), Some(0));
    assert!(
        !server.overworld().particles().is_empty(),
        "border particles"
    );
    // Steve alone: a point per second.
    alex.set_location(at(&server, 15.5, 65.0, 0.5));
    server.tick(20);
    assert_eq!(server.score(OBJECTIVE, "Steve"), Some(1));
    assert!(steve.last_action_bar().unwrap().starts_with("King: Steve"));
    server.tick(40);
    assert!(
        steve
            .titles()
            .iter()
            .any(|t| t.title.to_plain() == "Victory!")
    );
    alex.assert_message_contains("Steve won arena!");
    // Rewards through the economy service: play 50 + win 500 / play 50.
    steve.assert_message_contains("Reward: 550 coins");
    alex.assert_message_contains("Reward: 50 coins");
    server.tick(40);
    assert!(near(&steve, 30.5, 30.5, 0.01));
    assert_eq!(steve.game_mode(), GameMode::Survival);
}

#[test]
fn time_up_draw_and_leader() {
    let server = server();
    let _admin = setup(&server, "koth");
    let (steve, alex) = start_two(&server);
    steve.set_location(at(&server, 20.5, 65.0, 0.5));
    alex.set_location(at(&server, 20.5, 65.0, 5.5));
    server.tick(200);
    steve.assert_message_contains("arena ended in a draw.");
    assert!(
        steve
            .titles()
            .iter()
            .any(|t| t.subtitle.to_plain() == "Draw!")
    );
    server.tick(40);

    // Next round: one point is enough to lead when the time is up.
    run(&steve, "/mg join arena");
    run(&alex, "/mg join arena");
    server.tick(60);
    steve.set_location(at(&server, 0.5, 65.0, 0.5));
    alex.set_location(at(&server, 20.5, 65.0, 0.5));
    server.tick(30);
    steve.set_location(at(&server, 20.5, 65.0, 0.5));
    server.tick(200);
    steve.assert_message_contains("Steve won arena!");
}

#[test]
fn countdown_aborts_and_join_errors() {
    let server = server();
    let admin = setup(&server, "spleef");
    let steve = server.add_player("Steve");
    let alex = server.add_player("Alex");
    let bob = server.add_player("Bob");
    let eve = server.add_player("Eve");
    refused(&steve, "/mg leave");
    steve.assert_message_contains("You are not in a game.");
    refused(&steve, "/mg join nope");
    steve.assert_message_contains("Unknown arena: nope");
    run(&steve, "/mg join"); // the only open arena
    refused(&steve, "/mg join arena");
    steve.assert_message_contains("You are already in arena: /mg leave first.");
    run(&alex, "/mg join arena");
    server.tick(20);
    run(&alex, "/mg leave");
    alex.assert_message_contains("You left arena.");
    steve.assert_message_contains("Alex left (1/3).");
    server.tick(20);
    steve.assert_message_contains("Not enough players: the countdown stopped.");
    assert!(steve.boss_bars()[0].title.to_plain().contains("Waiting"));
    assert!(alex.boss_bars().is_empty());

    run(&alex, "/mg join arena");
    run(&bob, "/mg join arena");
    refused(&eve, "/mg join arena");
    eve.assert_message_contains("arena is full.");
    server.tick(60);
    run(&bob, "/mg leave");
    refused(&bob, "/mg join arena");
    bob.assert_message_contains("A game is being played in arena");
    refused(&eve, "/mg join");
    eve.assert_message_contains("No arena is open right now.");
    refused(&admin, "/mg delete arena");
    admin.assert_message_contains("arena has a game in progress.");
    refused(&admin, "/mg setarea arena 2");
    refused(&admin, "/mg start arena");
    admin.assert_message_contains("is already playing");
}

#[test]
fn forced_start_and_stop() {
    let server = server();
    let admin = setup(&server, "spleef");
    refused(&admin, "/mg start arena");
    refused(&admin, "/mg stop arena");
    admin.assert_message_contains("No game is being played in arena.");
    let steve = server.add_player("Steve");
    run(&steve, "/mg join arena");
    run(&admin, "/mg start arena");
    admin.assert_message_contains("Starting arena.");
    server.tick(60);
    assert_eq!(steve.count("diamond_shovel"), 1, "solo round started");
    assert!(steve.break_block((1, 64, 1)));
    run(&admin, "/mg stop arena");
    steve.assert_message_contains("The game in arena was stopped.");
    assert!(near(&steve, 30.5, 30.5, 0.01));
    assert_eq!(steve.count("diamond_shovel"), 0);
    assert!(server.objective(OBJECTIVE).is_none());
    assert_eq!(
        server.overworld().block_id((1, 64, 1)),
        "minecraft:snow_block"
    );
    // A stopped game counts for nobody.
    assert_eq!(server.storage(ID, &format!("stats/{}", steve.uuid())), None);
}

#[test]
fn disconnect_mid_game() {
    let server = server();
    let _admin = setup(&server, "spleef");
    let (steve, alex) = start_two(&server);
    assert_eq!(alex.count("diamond_shovel"), 1);
    alex.set_game_mode(GameMode::Survival);
    alex.disconnect();
    // The last one standing wins.
    steve.assert_message_contains("Alex left (1/3).");
    steve.assert_message_contains("Steve won arena!");
    server.tick(40);
    // Alex comes back in the lobby, without the game's shovel.
    let alex = server.add_player("Alex");
    assert!(near(&alex, 30.5, 30.5, 0.01), "{:?}", alex.location());
    assert_eq!(alex.count("diamond_shovel"), 0);
    assert_eq!(alex.data(ID, "return"), None);
    run(&alex, "/mg join arena");
}

#[test]
fn disabling_mid_game_cleans_up() {
    let server = server();
    let _admin = setup(&server, "spleef");
    let (steve, alex) = start_two(&server);
    assert!(steve.break_block((3, 64, 0)));
    alex.set_game_mode(GameMode::Creative);
    server.disable(ID).unwrap();
    steve.assert_message_contains("The mini-game was stopped by the server.");
    for p in [&steve, &alex] {
        assert!(near(p, 30.5, 30.5, 0.01), "{} to the lobby", p.name());
        assert_eq!(p.count("diamond_shovel"), 0);
        assert!(p.boss_bars().is_empty());
    }
    assert_eq!(alex.game_mode(), GameMode::Survival, "mode restored");
    assert!(server.objective(OBJECTIVE).is_none());
    assert!(server.boss_bars().is_empty());
    assert_eq!(
        server.overworld().block_id((3, 64, 0)),
        "minecraft:snow_block"
    );
    server.enable(ID).unwrap();
    server.assert_enabled(ID);
}

#[test]
fn stats_and_clickable_top() {
    let server = server();
    let _admin = setup(&server, "spleef");
    let (steve, alex) = start_two(&server);
    alex.set_location(at(&server, 0.5, 50.0, 0.5));
    server.tick(41);
    run(&steve, "/mg stats");
    steve.assert_message_contains("Stats of Steve:");
    steve.assert_message_contains("Wins 1 · games 1 · win rate 100% · points 0");
    run(&steve, "/mg stats Alex");
    steve.assert_message_contains("Wins 0 · games 1 · win rate 0%");
    let bob = server.add_player("Bob");
    run(&bob, "/mg stats");
    bob.assert_message_contains("Bob has not played yet.");
    refused(&bob, "/mg stats Nobody");
    bob.assert_message_contains("Unknown player: Nobody");
    run(&bob, "/mg top");
    bob.assert_message_contains("Best players (wins):");
    bob.assert_message_contains("#1 Steve 1 wins · 1 games · 0 points");
    bob.assert_message_contains("#2 Alex 0 wins · 1 games");
    let json: Vec<String> = bob
        .messages_json()
        .iter()
        .map(|t| t.as_json().to_owned())
        .collect();
    assert!(
        json.iter()
            .any(|j| j.contains("/mg stats Steve") && j.contains("run_command"))
    );
    run(&bob, "/mg top games");
    bob.assert_message_contains("Best players (games):");
    assert_eq!(bob.complete("mg stats St"), vec!["Steve".to_owned()]);

    let empty = MockServer::builder()
        .plugin_config(ID, FAST)
        .built(&built())
        .build();
    let p = empty.add_player("P");
    run(&p, "/mg top");
    p.assert_message_contains("Nobody has played yet.");
    run(&p, "/mg list");
    p.assert_message_contains("No arena yet.");
}

#[test]
fn french_messages() {
    let server = server();
    let _admin = setup(&server, "spleef");
    let steve = server.add_player("Steve");
    steve.set_locale("fr_fr");
    run(&steve, "/mg join arena");
    steve.assert_message_contains("Vous avez rejoint arena (1/3).");
    assert!(
        steve.boss_bars()[0]
            .title
            .to_plain()
            .contains("En attente de joueurs 1/2")
    );
    run(&steve, "/mg leave");
    steve.assert_message_contains("Vous avez quitté arena.");
}
