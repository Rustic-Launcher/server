//! Integration tests: the real `menus.wasm` in `rusticmc-test`'s in-memory server.
//! `/menu` and its routes, every button of every menu, click kinds, cancelled clicks, the
//! confirmation dialog (yes, no, closed), toggles persisted per player, the paginated head list
//! updated in place, live title/item refresh, back/close, disconnects, permissions and French.

use rusticmc_test::BuiltPlugin;
use rusticmc_test::prelude::*;

const ID: &str = "menus";

fn built() -> BuiltPlugin {
    build_plugin(env!("CARGO_MANIFEST_DIR"))
}

fn server() -> MockServer {
    MockServer::with_built(&built())
}

fn run(p: &MockPlayer, line: &str) {
    let r = p.command(line);
    assert!(r.executed(), "{line}: {r:?}\nmessages: {:?}", p.messages());
}

fn menu(p: &MockPlayer) -> rusticmc_test::MenuView {
    p.open_menu()
        .unwrap_or_else(|| panic!("{} has no menu open", p.name()))
}

fn title(p: &MockPlayer) -> String {
    menu(p).title_plain()
}

fn item(p: &MockPlayer, slot: u32) -> ItemStack {
    menu(p).item(slot)
}

/// Clicks a slot; plugin menus never let an item move.
fn click(p: &MockPlayer, slot: u32, kind: ClickKind) {
    assert!(
        !p.click_menu_slot(slot, kind),
        "click on {slot} went through"
    );
}

// Slots (see src/lib.rs).
const INFO: u32 = 10;
const WARPS: u32 = 12;
const SETTINGS: u32 = 14;
const PLAYERS: u32 = 16;
const CLOSE: u32 = 22;
const YES: u32 = 11;
const QUESTION: u32 = 13;
const NO: u32 = 15;

#[test]
fn enables_with_command_and_permissions() {
    let server = server();
    server.assert_enabled(ID);
    assert!(server.has_command("menu"));
    assert!(server.has_command("gui"));
    let steve = server.add_player("Steve");
    assert!(steve.has_permission("menus.use"));
    assert!(!steve.has_permission("menus.teleport"));
    steve.set_permission("menus.use", false);
    assert!(matches!(
        steve.command("/menu"),
        CommandRun::Plugin(CommandOutcome::NoPermission)
    ));
    assert!(!server.console_command("menu").executed());
}

#[test]
fn main_menu_layout_and_cancelled_clicks() {
    let server = server();
    let steve = server.add_player("Steve");
    run(&steve, "/menu");
    let m = menu(&steve);
    assert_eq!(m.rows, 3);
    assert_eq!(m.title_plain(), "Main menu · 1 online");
    assert_eq!(m.item(INFO).id, "minecraft:nether_star");
    assert_eq!(m.item(WARPS).id, "minecraft:ender_pearl");
    assert_eq!(m.item(SETTINGS).id, "minecraft:comparator");
    let head = m.item(PLAYERS);
    assert_eq!(head.id, "minecraft:player_head");
    assert!(
        head.components.contains("minecraft:profile"),
        "{}",
        head.components
    );
    assert!(
        head.components.contains("name:\"Steve\""),
        "{}",
        head.components
    );
    assert!(head.components.contains("id:[I;"), "{}", head.components);
    assert_eq!(m.item(CLOSE).id, "minecraft:barrier");
    assert_eq!(m.item(0).id, "minecraft:gray_stained_glass_pane");
    assert_eq!(m.items.len(), 27);
    assert!(m.item(INFO).components.contains("Version"));

    // Nothing can be taken: buttons, filler, any click kind, the player's own inventory below.
    steve.give("diamond", 3);
    for (slot, kind) in [
        (0, ClickKind::Left),
        (0, ClickKind::ShiftLeft),
        (WARPS, ClickKind::Middle),
        (INFO, ClickKind::Drop),
        (27, ClickKind::ShiftLeft),
    ] {
        click(&steve, slot, kind);
    }
    assert!(!steve.number_key_menu_slot(0, 0));
    assert!(steve.cursor().count == 0);
    assert_eq!(steve.count("nether_star"), 0);
    assert_eq!(steve.count("gray_stained_glass_pane"), 0);
    assert_eq!(steve.count("diamond"), 3);
    assert_eq!(menu(&steve).items.len(), 27);

    click(&steve, CLOSE, ClickKind::Left);
    assert!(steve.open_menu().is_none());
}

#[test]
fn server_info_click_kinds_and_live_title() {
    let server = server();
    let steve = server.add_player("Steve");
    run(&steve, "/menu");
    click(&steve, INFO, ClickKind::Left);
    steve.assert_message_contains("TPS");
    steve.assert_message_contains("1/");
    let before = steve.messages().len();
    click(&steve, INFO, ClickKind::Right); // refresh in place, no chat
    assert_eq!(steve.messages().len(), before);

    let _alex = server.add_player("Alex");
    server.tick(20);
    // The refreshed item keeps its handler.
    steve.clear_messages();
    click(&steve, INFO, ClickKind::Left);
    steve.assert_message_contains("TPS");
    assert_eq!(title(&steve), "Main menu · 2 online");
    assert!(
        item(&steve, INFO).components.contains("2</white>")
            || item(&steve, INFO).components.contains("\"2\"")
    );
}

#[test]
fn warps_run_commands_with_and_without_confirmation() {
    let server = server();
    let steve = server.add_player("Steve");
    run(&steve, "/menu");
    click(&steve, WARPS, ClickKind::Left);
    let m = menu(&steve);
    assert_eq!(m.title_plain(), "Warps");
    assert_eq!(m.rows, 3);
    assert_eq!(m.item(10).id, "minecraft:grass_block");
    assert_eq!(m.item(11).id, "minecraft:emerald");
    assert_eq!(m.item(12).id, "minecraft:iron_sword");

    // Right-click: details only.
    click(&steve, 11, ClickKind::Right);
    steve.assert_message_contains("Market: Shops and trading hall.");
    assert!(steve.open_menu().is_some());

    // Left-click: runs the command from the console and closes.
    click(&steve, 10, ClickKind::Left);
    assert!(steve.open_menu().is_none());
    assert!(
        server
            .console_commands()
            .iter()
            .any(|c| c == "tp Steve 0 80 0"),
        "{:?}",
        server.console_commands()
    );
    let l = steve.location();
    assert_eq!((l.x, l.y, l.z), (0.0, 80.0, 0.0));
    steve.assert_message_contains("Warped to Spawn.");

    // A warp with confirm = true: No goes back to the warps.
    run(&steve, "/menu warps");
    click(&steve, 12, ClickKind::Left);
    let dialog = menu(&steve);
    assert_eq!(dialog.rows, 3);
    assert_eq!(dialog.item(YES).id, "minecraft:lime_concrete");
    assert_eq!(dialog.item(NO).id, "minecraft:red_concrete");
    assert_eq!(dialog.item(QUESTION).id, "minecraft:iron_sword");
    assert!(dialog.item(QUESTION).components.contains("Go to "));
    click(&steve, NO, ClickKind::Left);
    assert_eq!(title(&steve), "Warps");
    // Closing the dialog is a No too.
    click(&steve, 12, ClickKind::Left);
    assert!(steve.close_menu());
    assert_eq!(title(&steve), "Warps");
    assert!(
        !server
            .console_commands()
            .iter()
            .any(|c| c.starts_with("tp Steve -300"))
    );
    // Yes runs both commands.
    click(&steve, 12, ClickKind::Left);
    click(&steve, YES, ClickKind::Left);
    assert!(steve.open_menu().is_none());
    assert!(
        server
            .console_commands()
            .iter()
            .any(|c| c == "tp Steve -300 64 210")
    );
    assert!(
        server
            .console_commands()
            .iter()
            .any(|c| c.starts_with("title Steve actionbar"))
    );
    steve.assert_message_contains("Warped to Arena.");
}

#[test]
fn back_and_close_buttons() {
    let server = server();
    let steve = server.add_player("Steve");
    for (route, rows) in [("warps", 3u32), ("settings", 3)] {
        run(&steve, &format!("/menu {route}"));
        click(&steve, (rows - 1) * 9, ClickKind::Left); // back
        assert!(title(&steve).starts_with("Main menu"), "{route}");
        run(&steve, &format!("/menu {route}"));
        click(&steve, (rows - 1) * 9 + 4, ClickKind::Left); // close
        assert!(steve.open_menu().is_none(), "{route}");
    }
    run(&steve, "/menu players");
    click(&steve, 45, ClickKind::Left);
    assert!(title(&steve).starts_with("Main menu"));
    click(&steve, PLAYERS, ClickKind::Left);
    click(&steve, 53, ClickKind::Left);
    assert!(steve.open_menu().is_none());
}

fn settings(p: &MockPlayer) -> serde_json::Map<String, serde_json::Value> {
    p.data_json::<serde_json::Map<String, serde_json::Value>>(ID, "settings")
        .unwrap_or_default()
}

#[test]
fn toggles_reflect_and_persist_state() {
    let server = server();
    let steve = server.add_player("Steve");
    run(&steve, "/menu");
    click(&steve, SETTINGS, ClickKind::Left);
    assert_eq!(title(&steve), "Settings");
    let first = item(&steve, 10);
    assert_eq!(first.id, "minecraft:note_block");
    assert!(
        first
            .components
            .contains("enchantment_glint_override\":true")
    );
    assert!(first.components.contains("Enabled"));
    let particles = item(&steve, 13);
    assert!(
        particles
            .components
            .contains("enchantment_glint_override\":false")
    );

    // Left: toggle (item updated in place, same menu).
    let id = menu(&steve).id;
    click(&steve, 10, ClickKind::Left);
    assert_eq!(menu(&steve).id, id);
    let off = item(&steve, 10);
    assert!(
        off.components
            .contains("enchantment_glint_override\":false")
    );
    assert!(off.components.contains("Disabled"));
    assert_eq!(settings(&steve)["mention_sound"], false);
    steve.assert_message_contains("Mention sounds: Disabled");
    steve.assert_heard("ui.button.click");

    // Right: explains, changes nothing.
    click(&steve, 10, ClickKind::Right);
    steve.assert_message_contains("mentions you in chat.");
    assert_eq!(settings(&steve)["mention_sound"], false);

    // Persisted per player, across a reload.
    click(&steve, 13, ClickKind::Left);
    server.reload(ID).unwrap();
    run(&steve, "/menu settings");
    assert!(item(&steve, 10).components.contains("Disabled"));
    assert!(item(&steve, 13).components.contains("Enabled"));
    let alex = server.add_player("Alex");
    run(&alex, "/menu settings");
    assert!(item(&alex, 10).components.contains("Enabled"));

    // Shift: back to the default.
    click(&steve, 10, ClickKind::ShiftLeft);
    assert!(item(&steve, 10).components.contains("Enabled"));
    assert!(!settings(&steve).contains_key("mention_sound"));
}

#[test]
fn reset_all_settings_with_confirmation() {
    let server = server();
    let steve = server.add_player("Steve");
    run(&steve, "/menu settings");
    click(&steve, 10, ClickKind::Left);
    click(&steve, 26, ClickKind::Left); // reset all
    assert_eq!(item(&steve, QUESTION).id, "minecraft:tnt");
    click(&steve, NO, ClickKind::Left);
    steve.assert_message_contains("Cancelled.");
    assert_eq!(title(&steve), "Settings");
    assert!(item(&steve, 10).components.contains("Disabled"));
    click(&steve, 26, ClickKind::Left);
    click(&steve, YES, ClickKind::Left);
    steve.assert_message_contains("Your settings were reset.");
    assert_eq!(title(&steve), "Settings");
    assert!(item(&steve, 10).components.contains("Enabled"));
    assert_eq!(steve.data(ID, "settings"), None);
}

#[test]
fn paginated_player_heads_update_in_place() {
    let server = server();
    server.set_max_players(100);
    let names: Vec<String> = (0..50).map(|i| format!("P{i:02}")).collect();
    let others: Vec<MockPlayer> = names.iter().map(|n| server.add_player(n)).collect();
    let steve = server.add_player("Steve");
    run(&steve, "/menu players");
    let m = menu(&steve);
    assert_eq!(m.rows, 6);
    assert_eq!(m.title_plain(), "Players (51) · 1/2");
    assert!(m.item(0).components.contains("name:\"P00\""));
    assert!(m.item(44).components.contains("name:\"P44\""));
    assert_eq!(m.item(48).id, "minecraft:gray_stained_glass_pane"); // no previous page
    assert_eq!(m.item(50).id, "minecraft:spectral_arrow");
    assert_eq!(m.item(49).id, "minecraft:paper");

    click(&steve, 50, ClickKind::Left); // next
    let m2 = menu(&steve);
    assert_eq!(m2.id, m.id, "same menu, updated in place");
    assert_eq!(m2.title_plain(), "Players (51) · 2/2");
    assert!(m2.item(0).components.contains("name:\"P45\""));
    assert!(m2.item(5).components.contains("name:\"Steve\""));
    assert!(m2.item(6).count == 0 || m2.item(6).id == "minecraft:air");
    assert_eq!(m2.item(48).id, "minecraft:spectral_arrow");
    assert_eq!(m2.item(50).id, "minecraft:gray_stained_glass_pane");
    assert_eq!(m2.item(49).count, 2);

    // Live: players leaving shrink the list at the next refresh.
    for p in &others[..10] {
        p.disconnect();
    }
    server.tick(20);
    assert_eq!(title(&steve), "Players (41) · 1/1");
    assert!(item(&steve, 0).components.contains("name:\"P10\""));

    click(&steve, 48, ClickKind::Left); // filler now: nothing happens
    assert_eq!(title(&steve), "Players (41) · 1/1");
    run(&steve, "/menu players 2"); // clamped to the last page
    assert_eq!(title(&steve), "Players (41) · 1/1");
}

#[test]
fn head_click_kinds_and_teleport_confirmation() {
    let server = server();
    let alex = server.add_player("Alex");
    let steve = server.add_player("Steve");
    alex.set_location(server.overworld().location(10.0, 70.0, -5.0));
    run(&steve, "/menu players");
    // Left: info.
    click(&steve, 0, ClickKind::Left);
    steve.assert_message_contains("Alex is in overworld at 10 70 -5");
    // Right: clickable /msg suggestion.
    click(&steve, 0, ClickKind::Right);
    steve.assert_message_contains("[Click to message Alex]");
    let json = steve.messages_json().last().unwrap().as_json().to_owned();
    assert!(json.contains("/msg Alex "), "{json}");
    // Shift: teleport needs the permission.
    click(&steve, 0, ClickKind::ShiftLeft);
    steve.assert_message_contains("You may not teleport to players.");
    click(&steve, 1, ClickKind::ShiftLeft);
    steve.assert_message_contains("That is you!");

    steve.set_permission("menus.teleport", true);
    click(&steve, 0, ClickKind::ShiftLeft);
    assert!(item(&steve, QUESTION).components.contains("Teleport to "));
    click(&steve, NO, ClickKind::Left);
    assert!(title(&steve).starts_with("Players"));
    click(&steve, 0, ClickKind::ShiftLeft);
    click(&steve, YES, ClickKind::Left);
    steve.assert_message_contains("Teleported to Alex.");
    let l = steve.location();
    assert_eq!((l.x, l.y, l.z), (10.0, 70.0, -5.0));

    // A head of a player who left.
    run(&steve, "/menu players");
    alex.disconnect();
    click(&steve, 0, ClickKind::Left);
    steve.assert_message_contains("Alex is no longer online.");
}

#[test]
fn disconnecting_closes_the_menu() {
    let server = server();
    let steve = server.add_player("Steve");
    run(&steve, "/menu players");
    steve.disconnect();
    server.tick(40);
    let steve = server.add_player("Steve");
    assert!(steve.open_menu().is_none());
    run(&steve, "/menu");
    assert!(title(&steve).starts_with("Main menu"));
    assert!(
        !server
            .log_lines()
            .iter()
            .any(|l| l.contains("WARN") || l.contains("ERROR")),
        "{:?}",
        server.log_lines()
    );
}

#[test]
fn disabling_closes_open_menus() {
    let server = server();
    let steve = server.add_player("Steve");
    run(&steve, "/menu");
    server.disable(ID).unwrap();
    assert!(steve.open_menu().is_none());
}

#[test]
fn french_ui() {
    let server = server();
    let steve = server.add_player("Steve");
    steve.set_locale("fr_fr");
    run(&steve, "/menu");
    assert_eq!(title(&steve), "Menu principal · 1 en ligne");
    click(&steve, SETTINGS, ClickKind::Left);
    assert_eq!(title(&steve), "Paramètres");
    assert!(item(&steve, 10).components.contains("Activé"));
    click(&steve, 10, ClickKind::Left);
    steve.assert_message_contains("Mention sounds : Désactivé");
}
