# 0013. A widget skin seam and menu helpers for the Rustic edition (U15, U16)

- Status: accepted (2026-11-13)
- Patches: `patches/0013-rustic-ui-skin-a-look-for-the-shared-widgets.patch` (U15),
  `patches/0014-rustic-client-menu-helpers-languages-and-server-list.patch` (U16)

## Context

The Rustic edition client (launcher branch, `launcher/client-mod/rustic-companion`, launcher ADR
0160) draws every in-game menu in Rustic's design (dark rounded plates, the orange accent, smooth
text), adds its own language screen and an in-game server directory (Discover). Horizon's mod API
lets a trait mod draw over a screen (`screen_render_post`) and take its input (`screen_*_pre`), but:

- vanilla's and Horizon's screens draw their buttons, sliders, fields, check boxes, tabs, scroll
  bars, tooltips and backgrounds with `horizon_client_render::ui` (nine-sliced GUI sprites and the
  game's pixel font). Drawing over each of them per screen would mean knowing every screen's
  layout (dozens of screens, hundreds of widgets) and would break on any change upstream;
- no helper lists the game's languages or chooses one (the Language page is inside Options, its
  state `pub(super)`), reads the server list, joins a server or adds one to the list from a mod.

## Decision

Two small patches, each in Horizon's own style, with tests:

- **U15, a widget skin** (`horizon_client_render::ui::skin`, render crate, platform layer like
  `pack_hooks::install`): an executable installs one `Skin` (first wins). `ui::nine_slice`,
  `ui::blit_sprite` and `ui::tiled_sized` (hence `tiled`, `menu_background`, `list_background`,
  `separators`) call `Skin::sprite(batch, atlas, sprite, rect, color)` first and draw vanilla's
  sprite only when it returns false; `ui::scrolling_text_around_at` (widget labels) and
  `ui::centered_text` (titles, message lines) call `Skin::label(batch, atlas, text, LabelBox,
  color)`. `Game::hud` calls `Skin::begin(atlas)` once a frame so the skin knows the atlas (a new
  one comes with every resource reload). One seam covers every screen drawn with `ui`; nothing
  moves: positions, hit boxes, narration, controller targets and clicks stay the screens'.
- **U16, menu helpers** (mod API fragment `client_menus.json`, `game/native_client_menus.rs`):
  `languages()`, `set_language(code)`, `server_list()`, `join_server(address)`,
  `add_server(name, address)`, all `&self` (usable from the read-only screen hooks); what needs
  `&mut Game` or the event loop is queued and run in `idle` between frames, as U13's `title_open`.
  `set_language` does what the Language page's Done does (`options.txt` saved, `load_language`);
  `join_server` is the server list's Join (`title_action(Connect)`), keeping the address among the
  hidden servers as Direct Connection does; `add_server` is Add Server's Done (an open list
  reloads).

## Consequences

- With no skin installed and no mod calling the helpers, nothing changes (one `OnceLock` read per
  sprite; an empty `RefCell` per frame).
- Text drawn outside `ui` (list rows, some screens' own lines) keeps the game's font: the skin
  covers widgets and centred lines, not every string. A later, separate seam could cover more.
- The patches touch `ui.rs`, `game.rs` (one line and one field), `window_handler.rs` (one call):
  small to carry on a new Horizon pin.
- Upstream: U16 can be proposed to arismoko/horizon as mod-API helpers; U15 to horizon-rust/horizon
  as a platform seam (the official game could use it for a themed edition too).

## Alternatives

- **A Rustic resource pack** (our own textures for `gui/sprites/widget/*`, a TTF default font):
  no patch, but packs stretch textures with nearest sampling (rounded corners blur or step at every
  GUI scale), a TTF default font also changes chat, signs and books, and the player can turn the
  pack off; the in-world HUD would change too.
- **Per-screen overlays** from `screen_render_post`: drawing over vanilla widgets leaves them
  showing through, needs every screen's layout, and breaks with any upstream layout change.
- **A Horizon theme struct** (`menu_style`): it styles only Horizon's own overlays, not vanilla's
  screens (`menu_style.rs`: "Vanilla's screens never read it").
