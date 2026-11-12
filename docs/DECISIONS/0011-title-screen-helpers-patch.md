# 0011. A client patch for mods that draw their own main menu (U13)

- Status: accepted (2026-11-12)
- Patch: `patches/0011-rustic-title_widgets_hidden-and-title_open-helpers.patch`

## Context

The Rustic edition client (launcher branch, `launcher/client-mod/rustic-companion`, launcher ADRs 0070
and 0148) replaces the game's main title (logo, splash, buttons, corner texts) with a Rustic menu
drawn over the vanilla panorama. Horizon's mod API lets a trait mod draw over the title
(`screen_render_post`, screen id `title`) and cancel its mouse and key presses
(`screen_mouse_button_pressed_pre`, `screen_key_pressed_pre`), but:

- nothing hides the title's own widgets: drawing over them leaves the logo and buttons visible
  around and through a translucent menu, and Tab/Enter, the game's keyboard focus and the agent
  `ui` list still reach buttons the player no longer sees;
- nothing opens what the title's buttons open: Select World / Create New World, the multiplayer
  safety screen or server list, Realms, Join a Friend, Options and its Language and Accessibility
  pages, the credits, quitting. Most of these are `Title`'s own state machine (`title.rs`) or
  `Game::title_action` with the event loop, which no helper reaches.

## Decision

A minimal, generic client-side mod-API addition in Horizon's own style (pending fragment
`tools/native-hook-fragments/title_screen.json`, two helpers, no hook):

- `title_widgets_hidden(hidden: bool)` (`native_title_widgets_hidden`, `&self`, a `Cell` on `Game`):
  hidden, the main title (`Mode::Main` only) draws nothing over the panorama, `Title::buttons`
  is empty for it (so no click, Tab, Enter, focus outline or narration of a hidden button) and its
  bottom notice takes no press. It holds for the window through every return to the title; the game
  copies it into the `Title` each frame (`Title::set_widgets_hidden`).
- `title_open(button: &str) -> bool` (`native_title_open`, `&self`): presses a main title button
  for the player — `singleplayer`, `multiplayer`, `realms`, `friends`, `options`, `language`,
  `accessibility`, `credits`, `quit` — exactly as a click on it: `Title::press_main` runs the same
  `click_action` arm as the click, and `Game::idle` runs the returned action with `title_action`
  (the event loop is there, for Quit), with the button's sound. The press is queued in a `Cell` and
  done between frames, so it is callable from the read-only screen hooks where a menu handles its
  clicks. False off the main title, for an unknown name, and for Multiplayer/Realms when the account
  may not play online (their buttons are off then).

Both are `&self` so they work from `screen_render_post` and the cancellable input hooks, which get a
read-only `Host` (the precedent is `apply_screen_blur`). Files: `title.rs` (`MainButton`,
`widgets_hidden`, `set_widgets_hidden`, `press_main`, a `Mode::Main` arm in `buttons` and in the
draw, tests), `game/native_title_screen.rs` (new: the two helpers and `take_title_open_request`),
`game.rs` (two fields, the module, the per-frame copy), `game/window_handler.rs` (the queued press
in `idle`), the fragment.

## Consequences

- With no mod calling them, nothing changes: the flag defaults to false and the queue to empty
  (zero cost but a `Cell` read a frame).
- The startup menu (the title the loading window shows before the game's resources are ready,
  `game/startup_menu.rs`) runs no trait mod: it shows the vanilla title until the game takes over.
- Proposable upstream unchanged (arismoko/horizon, where mod-API work goes); listed as U13 in
  `docs/PATCHES.md` and `docs/ARCHITECTURE.md` §9.1.

## Alternatives

- Cancel every title click and key in the input hooks and paint an opaque menu over the vanilla
  widgets: the widgets still show at the edges and through translucency, the game's keyboard focus
  and agent control still see them, and the menu still could not open Select World or Options.
- A screen-replacement hook (`screen_opening` returning a mod screen for the title): far larger, and
  the title's sub-screens are its own state machine, not separate screens a mod could reopen.
- Driving the title through the agent control socket (`horizon-ctl click-button`): needs the socket
  on, is label-based (breaks with the language) and is not a mod API.
