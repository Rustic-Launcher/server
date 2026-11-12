# menus

A chest-menu demo built on the SDK's `Menu` builder, and a **recipe book** of patterns to copy
into your own plugins. English and French.

## In game

`/menu` (alias `/gui`) opens a 3-row main menu:

| Slot | Button | Click |
|---|---|---|
| 10 | **Server info** (nether star): version, players, TPS, plugins | left: the summary in chat; right: refresh the item. The item and the menu title ("Main menu · 3 online") refresh every second while open. |
| 12 | **Warps** | opens the warp selector: one item per `[[warps]]`; left runs its console commands (`{player}` = the player), right shows the description; warps with `confirm = true` ask first |
| 14 | **Settings** | per-player toggles from `[[toggles]]`; name colour, "State: Enabled/Disabled" lore and the enchantment glint follow the state; left toggles, right explains, shift restores the default; the TNT asks before resetting everything |
| 16 | **Players** (your own head) | a 6-row list of player heads (`minecraft:player_head` with a `minecraft:profile` component), 45 per page; ◀ ▶ page in place (items and title "Players (51) · 2/2" updated, same menu); left on a head: where they are; right: a clickable `/msg` suggestion; shift: teleport (`menus.teleport`) after a confirmation |
| 22 | **Close** | |

Every submenu has **Back** (bottom-left) and **Close** (bottom-middle). `/menu warps`,
`/menu settings`, `/menu players [page]` open a submenu directly.

Nothing can be taken out of a menu: clicks are cancelled (SDK default, and a `highest` guard
handler cancels them again whatever other plugins do), including shift-clicks from the
player's inventory, number keys, drops and double clicks. Buttons only react to left/right
clicks with or without shift. A menu is closed and its state forgotten when the player
disconnects or the plugin is disabled.

## Commands and permissions

| Command | Permission | Default |
|---|---|---|
| `/menu`, `/menu warps`, `/menu settings`, `/menu players [page]` | `menus.use` | everyone |
| shift-click a head to teleport | `menus.teleport` | op |

Players only (the console is refused).

## Configuration (`plugins/menus/config.toml`)

See the shipped [`config.toml`](config.toml) (`config_version = 1`): `language` (`auto`, `en`,
`fr`), `refresh_ticks` (live refresh period), `[[warps]]` (`name`, `icon`, `description`,
`commands`, `confirm`) and `[[toggles]]` (`id`, `name`, `icon`, `description`, `default`).
Toggle states are stored per player (data key `settings`, only the values that differ from
the default).

## Recipe book

Each pattern is a few lines in `src/`; copy what you need.

1. **One state struct, closures that reach it.** Keep per-player sessions in the plugin
   (`HashMap<Player, Session { handle, view }>`). Wrap click handlers so they borrow the plugin
   and report errors to the player:
   ```rust
   fn on_click(mut f: impl FnMut(&mut Menus, &mut MenuClick) -> Result<()> + 'static)
       -> impl FnMut(&mut MenuClick) + 'static {
       move |click| {
           if !is_button_click(click.kind()) { return; }        // ignore drops, number keys...
           let player = click.player();
           if let Err(e) = with_plugin(|m: &mut Menus| f(m, click)) {
               player.send_message(Text::new(e.to_string()).color(NamedColor::Red));
           }
       }
   }
   ```
2. **Know which menu closed.** `on_close` is set before the menu id exists: share it through
   an `Rc<Cell<Option<u32>>>` filled after `open`, and forget the session only if it still
   points at that id (opening a submenu replaces the session first). See `Menus::show`.
3. **Navigation.** Back = open the parent menu (it replaces the current one); close =
   `click.close()`. Put both in one helper (`Menus::nav`) so every submenu has them at the same
   slots.
4. **Click types.** Branch on `click.kind()`: `Left`, `Right`, `ShiftLeft`/`ShiftRight`; treat
   everything else as "do nothing" (it stays cancelled).
5. **Toggle items.** Derive the item from the state (`ui::toggle_item`: green/red name, a state
   lore line, `.glint(on)`), persist with `player.data().set(..)`, then replace the clicked slot
   in place with `MenuHandle::set_item(slot, item, handler)` (re-register the handler).
6. **Pagination in place.** Open the frame once (navigation row), then fill a page with
   `set_item`/`set_display` (empty stack for unused slots) and `set_title`; previous/next
   buttons call the same fill function with `page ± 1`. Pure helpers: `ui::page_count`,
   `ui::page_range`, `ui::clamp_page`.
7. **Player heads.** `ItemStack::new("minecraft:player_head").component("minecraft:profile",
   "{name:\"Steve\",id:[I;a,b,c,d]}")` (`ui::profile_snbt` turns a UUID into the int array).
8. **Live items and titles.** One plugin timer (`Scheduler::run_timer`) walks the open sessions
   and calls `set_display`/`set_title`; skip work when nothing changed (the player list keeps
   the players it shows); a failing call means the menu is gone: drop the session.
9. **Confirmation dialog.** `Confirm::new(lang).question(item).on_yes(..).on_no(..).open(&p)`
   (`src/confirm.rs`): Yes at slot 11, the question at 13, No at 15; whichever answer comes
   first takes both callbacks (`Rc<RefCell<Option<..>>>`), so exactly one runs once; closing
   the dialog counts as No; `on_no` typically reopens the previous menu.
10. **Never let items move.** Keep the SDK default (clicks start cancelled; never call
    `click.allow()` in a GUI) and, if other plugins might un-cancel, add a `highest` handler
    that cancels clicks in your own menus again.
11. **Disconnects and disable.** On `player-quit` (`monitor`), close the player's menu and drop
    the session; in `disable`, close every open menu.
12. **Text.** Names and lore are `Text` (`Text::mini(..)`); lore lines are non-italic unless
    they say otherwise; translations are MiniMessage templates filled with escaped values
    (`src/lang.rs`).

## Build and test

```sh
cd plugins/menus
cargo build --target wasm32-wasip2 --release        # ../target/wasm32-wasip2/release/menus.wasm
cargo test                                          # unit tests + tests/plugin.rs on the real .wasm
../../target/debug/rustic build --release && ../../target/debug/rustic check ../target/rustic/menus.rplugin --deny-warnings
```

Release component 663.1 KiB, `.rplugin` 233.6 KiB.

## Live check (2026-11-11, rustic-server `--profile fast`, `horizon-bots`)

A bot ran `/menu`, clicked (raw `ContainerClick` packets) the server info, a filler pane, the
settings button and a toggle, then closed the window. Its client received:

```
 2.08s OpenScreen        [window 1] Main menu · 1 online
 3.08s ContainerSetSlot  [window 1]          <- the live server-info item, once a second
 6.08s SystemChat        Rustic 0.1.0 (rustic:api 0.1) on Horizon (Minecraft 26.3) · 1/20 players · 20.0 TPS · 6 plugins
 7.08s ContainerSetContent                   <- click on a filler pane: cancelled, the server resends the window
 8.08s OpenScreen        [window 2] Settings
10.08s SystemChat        Mention sounds: Disabled
10.08s ContainerSetSlot  [window 2]          <- the toggle replaced in place (glint off)
10.08s Sound             ui.button.click
```

Server log: `[rustic] [menus] enabled Menus v0.1.0`, `[rustic] reloaded menus` (`/rustic reload menus`
with the menu closed), no warning. The first live run found two problems, both fixed and covered by
tests since: a title refresh every second reopened the window on Horizon (titles and items are now
re-sent only when they change), and the refreshed info item lost its click handler
(`set_display` drops handlers; it now uses `set_item` with the handler).
