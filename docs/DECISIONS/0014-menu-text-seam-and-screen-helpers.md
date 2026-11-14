# 0014. A menu text seam, screen helpers and the options on the settings shell for the Rustic edition (U17, U18)

- Status: accepted (2026-11-13)
- Patches: `patches/0015-rustic-menu-text-seam-and-client-screen-helpers.patch` (U17),
  `patches/0016-rustic-vanilla-options-on-the-settings-shell.patch` (U18)
- Builds on ADR 0013 (U15 widget skin, U16 menu helpers)

## Context

The Rustic edition client (launcher branch, `launcher/client-mod/rustic-companion`, launcher ADR
0164) redoes the game's pages in the launcher's design: the world list and the server list as
cards with an action bar, the pause menu as a panel, the dialogs (confirmations, Add/Edit Server,
Direct Connection, Disconnected, connecting and loading screens) as cards. With U15 alone, only the
shared widgets and centred lines were Rustic's; the owner's screenshots showed the rest:

- the lists' rows, titles and descriptions in the game's pixel font (screens set them with
  `HudBatch::text_styled`, outside the shared label helpers), the vanilla row thumbnails and white
  selection frame, the "Scanning for games on your local network" line;
- the "My servers / Discover" switch drawn over the vanilla "Play Multiplayer" title;
- the bottom button grids laid out as vanilla's (skinned only).

A mod can draw a page of its own over a screen (`screen_render_post`, taking its input with the
`screen_*_pre` hooks), but it had no way to read the world list or the server list (with pings,
MOTDs, icons), to choose a row, or to press the screen's own buttons, and nothing changed the
font of the text the screens set themselves.

## Decision

One patch, U17, in Horizon's style, with tests:

- **A menu text seam** (render crate, `ui::skin`): `Skin::text(batch, atlas, &TextLine)`, asked by
  `HudBatch::text_styled` only inside `ui::skin::menu_text()` scopes, which the game opens around
  the screens it draws as menus: the title's screens, Options, Debug Options, the pause, death,
  stats, world options, friends, social, server links, confirm-link and dialog-warning screens,
  the connection/loading/reconfiguring screens (`remote_overlay`), and the server dialogs and
  prompts. Never the HUD, the chat, containers, signs, books or editors. The skin gets the line
  resolved and in visual order with **each character's advance in the game's font**, and the game
  goes on as if its font had drawn it (same advance returned): carets, selections and the text
  after a line stay where the screen put them. With no skin, or outside a scope, nothing changes
  (one thread-local read per line).
- **Client screen helpers** (mod API fragment `client_screens.json`,
  `game/native_client_screens.rs`), all `&self`, the writes queued and run in `idle` as U13/U16:
  - `world_entries() -> Vec<NativeWorldEntry>`: the open world list's worlds (folder, name, game
    mode, hardcore, cheats, last played, version, locked, corrupted, incompatible, experimental,
    needs a backup, selected); `world_icon(folder)`: a world's `icon.png` (plain folder names only);
  - `server_entries() -> Vec<NativeServerEntry>`: the open server list's rows, saved servers then
    LAN worlds, with their ping (`NativeServerStatus`: pinging, online with ping/players/version,
    incompatible with the server's version, unreachable, LAN), styled MOTD, player sample, 64x64
    RGBA icon and resource pack setting;
  - `title_select(index)`: selects a row of the open list as a click on it does;
  - `press_at(x, y)`: presses the open screen at a GUI point as the mouse would, the pointer put
    back after it, marked as made up (as the pad's clicks) so the screens' mouse hooks do not hear
    it: a mod that lays a screen's buttons out its own way presses the screen's own, so every
    action (Join, Play, Edit, Delete, Re-Create, Refresh, the pause menu's) does exactly what it
    does in the game, its confirmations and forms included.

- **U18, vanilla's Options on the settings shell** (render crate, `options_shell.rs`): the owner
  wants every settings page in the layout of Horizon's own settings screens (graphics, controller,
  extras: the mark and the screen's name, a search field, the page list, Done at the foot; the
  open page's groups of rows, label left and value right, sliders, switches, `< value >` choices;
  the strip that describes the hovered row). Those screens are data (`settings_shell::Screen`):
  `OptionsScreen` describes vanilla's Options page (FOV and the links to Video, Resource Packs,
  Horizon Settings, Mods, Credits), Music & Sounds, Controls (with Mouse Settings, links to Key
  Binds and Controller Settings), Chat, Accessibility (with the font settings), Skin
  Customization and Online as its pages, each row the vanilla option itself (`Opt::label`,
  `slider_position`, `slider_steps`, `set_slider`, `cycle`, `tooltip_key`; High Contrast's pack
  and the narrator's event as `cycle_option` does them; volumes told on release). An executable
  turns it on (`options_shell::install(on, language)`), may leave the Language page out (every
  way to it: the link, `push(Page::Language)`), and may name the screens' mark
  (`options_shell::set_brand`, "Horizon" by default). Off, nothing changes. `OptionsUi` hosts it
  as its other shells (`shell`/`shell_mut`, `show`, the framebuffer); four items of `options_ui`
  become `pub(crate)` (`horizon_extras`, `MODEL_PART_KEYS`, `tooltip_key`). The Mods page is one of its pages too (each
  installed mod's switch, greyed with its reason; its settings; Browse Mods; the mods folder). The
  shell gains one row kind, `Value::Open(text)`: a row that opens something, drawn as a value to
  read (its name, a `›` at its right) and pressed like a button. Two more executable switches: `set_pages_scroll(true)` keeps
  every settings screen's page list a column that scrolls (the wheel over it, the focused page in
  view) instead of collapsing into the `‹ Page ›` row, and `set_texts(f)` lets the executable word
  some of these screens' texts (the Mods page's empty line) in its own tables.

## Consequences

- The edition's pages are drawn by the companion, the game's screens still run underneath (state,
  sounds, narration of what Rustic registers, Escape). No game logic is duplicated in the mod.
- Every menu line set with `text_styled` can be smooth; the layout never moves (each glyph sits in
  the game's glyph's place, which slightly tracks the text).
- The patch touches `ui/skin.rs`, `hud.rs` (one block in `text_styled`), `game.rs` (one module,
  one field, scopes around the menu draws), `window_handler.rs` (one call), `mod_api.rs` (three
  re-exports) and adds `native_client_screens.rs` and a fragment: small to carry.
- Upstream: the helpers can be proposed to arismoko/horizon (mod API), the text seam with U15 to
  horizon-rust/horizon (platform seam).

## Alternatives

- **Helpers that do each action** (`join_selected`, `delete_world`, `edit_server(...)`): exact
  but many, each duplicating a screen's flow (backups, confirmations, upgrades); `press_at` keeps
  the game's flows with one helper. (A first draft had `edit_server`/`remove_server`: dropped, the
  game's own Edit Server and Delete screens are used, drawn as Rustic dialogs.)
- **Skinning every `text_styled` call**: the HUD, chat, signs and books would change too, and a
  per-screen allow-list would live in the skin, which cannot see screens.
- **Natural Inter advances** for the menu text: nicer spacing, but centred titles drift, columns
  of left-aligned text shift, carets and selections no longer match the text.
