# 0015. Pack screen, mod browser and Create World helpers, mod tags and controller presses for the Rustic edition (U19)

- Status: accepted (2026-11-14)
- Patch: `patches/0017-rustic-pack-mod-and-create-world-helpers.patch` (U19)
- Builds on ADR 0014 (U17 screen helpers and `press_at`, U18 options on the settings shell)

## Context

The Rustic edition client (launcher branch, `launcher/client-mod/rustic-companion`, launcher ADR
0165) finishes redoing the game's pages in the launcher's design: Resource Packs (and Create
World's Data Packs, the same screen) as two columns of cards, Browse Mods as a grid of cards with
a sidebar and a details panel, Create World as one form, and origin tags (Horizon, Rustic, Native,
Modrinth, Local) on the settings' Mods page. With U17/U18 a mod can draw a page over a screen, but:

- it could not tell which options page is open (`screen_render_post` says `options` for all), read
  the pack screen's two lists, move a pack, restore the opening selection or open the pack folder;
- it could not read Browse Mods' listings, the install running, its outcome, or the installed
  mods, nor install, switch a mod or open a project's page;
- Create World's choices are the title's `Mode::Create` state, its buttons are ids handled by a
  private `click_action`, and its tabs hide two thirds of them: `press_at` cannot reach a button
  of another tab, nor type a field's text;
- the settings shell's rows (U18) have no room for a tag, its note rows are one line (the Mods
  page's empty line was cut in narrow windows), and nothing lets an executable add a row there;
- a controller's A over a mod's own widget is a click the pad makes up, which Horizon keeps from
  the screens' mouse hooks ("not the mouse"): focus and narration reached Rustic's pieces, the
  press went to the screen underneath.

## Decision

One patch, U19, in Horizon's style, with tests:

- **Client helpers** (`game/native_client_options.rs`, fragment `client_options.json`):
  - `options_page() -> Option<&'static str>`: one stable id per options page (`packs`,
    `data_packs` for Create World's, `mods`, `mod_browser`, `mod_project`, `main`...), tested to be
    distinct;
  - `options_packs() -> Option<NativePacks>`: the pack screen's selected packs (top first) and
    available ones (`NativePack`: id, title, description, icon RGBA, compatibility, source,
    required, fixed, what can be done to it, Polytone options), whether its incompatible-pack
    confirmation is open, whether the selection changed;
  - `options_mods() -> Option<NativeMods>`: the installed mods (`NativeInstalledMod`: id, title,
    description, on, native, problem, dev) and Browse Mods' `ModBrowser` (status, listings,
    install running, last outcome);
  - `options_act(NativeOptionsAct)`: a pack card's arrows (`Pack(id, NativePackAct)`, an
    incompatible pack opening the game's confirmation first), `PackRestore` (back to the opening
    selection: then `Leave` is a Cancel), `PackFolder`, `Leave` (the page's Done/Back: leaving the
    pack screen commits and reloads as before), `InstallMod(id)`, `ToggleMod(id)`, `ModPage(id)`
    (through the game's link screen), `BrowseMods`, `ModFolder` — each turned into the options'
    own events and run by `handle_option_events`;
  - `create_world() -> Option<NativeCreateWorld>` (name, seed, game mode, difficulty, commands,
    world type key, customizable, structures, bonus chest, data packs, `covered` while Game
    Rules/Experiments are over it) and `create_world_act(NativeCreateAct)`: `Name`/`Seed` text,
    `Focus(field)` (the keyboard to the game's own name or seed field: typing, paste, selection),
    `Press(id, step)` through the new `Title::press_create(id, step)` (the screen's own
    `click_action` for any tab's button, refused for an off button; `step` -1 steps a cycle
    button back) whose action runs through `title_action` as after a click (Data Packs, Create);
  - `pad_clicks_to_hooks(on)`: the clicks a controller or the keyboard's focus makes up reach the
    screens' mouse hooks too (opt-in, off by default); a mod's own `press_at` never does (a
    `mod_press` flag set while it runs).
  All writes queue (`RefCell`) and run in `idle`, as U13/U16/U17.
- **Render crate**: `PackScreen::act(id, PackAct)`, `restore()`, `can(id)` (and the selection it
  opened with); `OptionsUi::data_packs_open()`, `pack_folder_event()`; the settings shell's rows
  get `tags` (small labels right-aligned before the control, each in its colour, searched with the
  name; as many as fit with 48 px of name kept) and note rows wrap on two lines of their row; `options_shell`
  gains `set_mod_tags(fn(id, native) -> Vec<(label, rgb)>)` (asked for each installed mod's row)
  and `set_find_mods(label, press)` (a row before Browse Mods), and the Mods page's empty text
  gives one note row per line.

## Consequences

- Off by default and zero cost with no mod: no helper is called, no tag function or find row is
  set, `pad_clicks_to_hooks` stays off; the pack screen, mod browser and Create World behave
  exactly as before (their tests pass unchanged).
- The edition's pages read and press the game's own models: applying packs reloads the resources
  as the game's Done does, installs are checked by the browser's worker, the world is created by
  the title. No game logic is duplicated in the mod.
- The patch touches `pack_screen.rs` (one field, three methods, a test), `options_ui.rs` (two
  methods), `settings_shell.rs` (row tags, wrapped notes), `options_shell.rs` (two setters, one
  row kind), `pack_options_ui.rs` (one field), `title.rs` (three methods), `game.rs` (one module,
  three fields, the mouse-hook gate), `window_handler.rs` (one call), `native_client_screens.rs`
  (the `mod_press` flag), `mod_api.rs` (re-exports) and adds `native_client_options.rs` and a
  fragment; and `hud.rs` (one `&*visual` of U17 that newer clippy flags as an explicit auto-deref).
- Upstream: the helpers and `pad_clicks_to_hooks` can be proposed to arismoko/horizon (mod API),
  the shell's tags and setters with U18.

## Alternatives

- **`press_at` on the pack screen's own arrows**: they show only under the pointer, positions
  move with scrolling and searching, and Create World's buttons of other tabs aren't drawn at all.
- **A Rustic Mods page over the settings shell**: it would hide the shell's page list (the Mods
  page is one of its pages, not a pushed screen); tags on the shell's own rows keep its keyboard,
  controller, search and narrator.
- **Firing the mouse hooks for every made-up click**: changes what every mod hears; opt-in keeps
  Horizon's rule for the others.
- **A seam for every list's selection frame** (stats, game rules, experiments...): each screen
  draws its own inline; left for a later patch.
