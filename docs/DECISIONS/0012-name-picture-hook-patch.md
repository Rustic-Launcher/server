# 0012. A client hook for a picture in the place of a player's mark (U14)

- Status: accepted (2026-11-12)
- Patch: `patches/0012-rustic-name_picture-hook-a-picture-in-the-place-of-the-mark.patch`

## Context

The Rustic edition client (launcher branch, `launcher/client-mod/rustic-companion`, launcher ADR
0072) shows the Rustic logo left of the names of the players who use Rustic, in the player list
(Tab), over their heads and in chat. User decision (2026-11-12): for a Rustic user the Rustic logo
**takes the place** of Horizon's badge (one badge, Rustic's, in the same position); a Horizon-only
player keeps Horizon's badge exactly as today; players using neither get none.

What Horizon offers at the pin (a84f637d):

- `horizon_hooks::Hooks::player_mark(uuid) -> Option<Mark>` (player list) and
  `NameDecoration::mark` from `name_decoration` (name tag): the place before the name where
  Horizon's cosmetics draw Horizon's mark. A `Mark` is `&'static [(x, y, w, h, argb)]`: filled
  rectangles in text pixels, i.e. pixel art at the GUI's resolution, blocky at GUI scale 3 and more;
  the official logo cannot be shown smooth with it.
- Both are "first hook that answers is used". The edition registers Rustic's player hooks before
  Horizon's cosmetics, so answering them would also take `name_decoration`'s other fields from
  Horizon (its name tag style and the `top` lift that keeps tags above tall cosmetics), not only the
  mark, and Horizon's own answer could not be kept for non-Rustic players without reaching into
  Horizon's cosmetics state.
- Chat needs nothing: a client chat line is a `Text`, and Horizon draws an inline sprite
  (`InlineObject::Atlas`, any GUI atlas name, `hooks/<hex>` included) 8 GUI pixels square with the
  text's shadow.

## Decision

A minimal, generic platform hook in `horizon_hooks` (the layer executables register into, as
Horizon's cosmetics and Play with Friends do; not the mod API, so no fragment):

- `Hooks::name_picture(&self, uuid: u128) -> Option<u64>`, default `None`, dispatched by
  `engine::name_picture` (first answer wins): one of the crate's `players::Sprites` (the 64x64
  RGBA pictures it puts in the GUI atlas from `frame`), by id.
- **Player list** (`horizon-client-render/src/tab_list.rs`): `TabInputs::picture`. A player with a
  picture gets it drawn whole in the mark's 8x8 box (a `Blit` of `hooks/<hex>`), with the text's
  shadow (the same picture tinted `0xFF3F3F3F` a pixel right and down), and **no mark**. The mark
  column is there once anyone listed has a mark or a picture, so names stay aligned. The layout
  cache keys on the pictures too.
- **Name tags** (`game/names.rs`, `game/player_hooks.rs`): `Marks::picture`, filled from
  `name_picture` for the model's UUID when the sprite is in the atlas; drawn in the mark's place
  (8 text pixels square, a pixel left of the background), and the mark is then skipped. The rest of
  the decoration (Horizon's name tag style, its lift) is untouched.
- `game/scoreboard.rs` asks once per listed player per frame the list shows
  (`Game::hooks_name_picture`, which also keeps the sprite in its skin slot as drawn).

## Consequences

- With no hook answering, nothing changes: the closure returns `None`, the column and marks are as
  before (`a_picture_takes_the_place_of_the_mark_and_the_names_stay_aligned` checks both cases).
- Horizon's mark keeps working for everyone the picture hook does not answer for, which is the
  coexistence rule the user chose.
- The picture is drawn from the whole 64x64 sprite into 8 GUI pixels with nearest sampling; a crate
  that wants it crisp lays a render of the exact screen size into the sprite so that sampling gives
  it back (the companion does: `rusticmc_companion_core::badge::sprite_pixels`).
- Proposable upstream unchanged (to horizon-rust/horizon, which owns `horizon-hooks`); listed as U14
  in `docs/PATCHES.md` and `docs/ARCHITECTURE.md` §9.1.

## Alternatives

- A Rustic `Mark` (static rectangles) from `player_mark`: pixel art only, and it would hide
  Horizon's mark for everyone Rustic answers for, with no way to keep Horizon's for the others.
- Answering `name_decoration` for Rustic users: replaces Horizon's whole decoration (style, lift),
  not just the mark.
- Drawing over the player list from the trait mod's `hud` hook: the trait mod cannot know the rows'
  places (sorting, teams, columns, header lines) without re-implementing the layout, and could not
  reserve the column.
- Changing `Mark` into an enum with a picture variant: breaks every implementer of the public
  `player_mark`/`NameDecoration` API; a new hook with a default body breaks nobody.
