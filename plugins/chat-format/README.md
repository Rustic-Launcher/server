# chat-format

Chat formatting and moderation for Rustic servers: per-group line formats with prefixes from
the `chat-meta` service (the `perms` plugin) or the config, hoverable/clickable names, colours
per permission, `@mentions`, anti-spam (rate limit, duplicates, caps), a link filter with a
whitelist, a word filter, mutes with expiry, and join/quit lines. Messages in English and French.

## What it does

* **Line format**: every chat line is rebuilt from a template, for example
  `{prefix}{name}{suffix}<dark_gray>:</dark_gray> <white>{message}`. The event's `display` is
  replaced; the signed message is kept. Placeholders: `{prefix}`, `{suffix}`, `{name}`,
  `{message}`, `{group}`, `{world}`.
* **Groups**: `[[groups]]` give a group its own format and prefix/suffix. A player's group is the
  permission provider's primary group (or the first of their groups listed in the config) when
  a permissions plugin is installed, otherwise the first group whose permission
  `chatformat.group.<name>` they hold, otherwise `default_group`.
* **Prefixes and suffixes**: from the `chat-meta` service when a provider (e.g. `perms`) is
  installed, else the group's, else `default_prefix`/`default_suffix`. `&` legacy codes and
  MiniMessage-like tags both work. Without any provider the plugin works the same, with the
  config's prefixes (graceful degradation).
* **Name**: hovering shows `name_hover` (`{name} {group} {world} {uuid} {ping}`); clicking puts
  `name_click` (`/msg {name} `) in the chat box.
* **Colours**: players with `chatformat.color` may use colour and decoration tags (`<red>`,
  `<#ff8800>`, `<bold>`, `<!italic>`...) and `&` codes. Click, hover, insertion, font,
  translation, `<reset>` and `</>` stay literal even for them. Everyone else's text is shown
  exactly as typed.
* **Mentions**: `@name` of an online player is highlighted (`[mentions] format`); the mentioned
  player hears `sound` and sees an action-bar line. Played only once no other plugin cancelled
  the message (a `monitor` handler).
* **Anti-spam** (bypass `chatformat.bypass.spam`): at most `max_messages` per `per_seconds`;
  the same message (case, spaces and punctuation ignored) within `duplicate_seconds` is refused.
* **Caps** (bypass `chatformat.bypass.caps`): over `max_ratio` capitals in a message of at least
  `min_letters` letters is lower-cased or refused.
* **Links** (bypass `chatformat.bypass.links`): URLs, `www.` hosts, bare `host.tld` names and
  IPv4 addresses are refused or masked, except whitelisted domains and their subdomains.
* **Words** (bypass `chatformat.bypass.filter`): case-insensitive, masked letter by letter or
  refused.
* **Mutes**: stored under `mutes/<uuid>` (survive reloads and restarts), lifted on time (checked
  on chat and every 5 seconds, the player is told), listed by `/mutes`. A muted player can neither
  chat nor use `[mute] blocked_commands` (`/msg`, `/tell`, `/me`...).
* **Join/quit**: `[join] message`, `first_message` (the very first join, remembered per player)
  and `quit_message`; empty hides the line, `vanilla` keeps the server's. On Horizon these lines
  take effect with Rustic Server's patch U2.
* **Languages**: `language = "auto"` answers each player in their client's language (French for
  `fr_*`, English otherwise); `en`/`fr` force one.

## Commands

| Command | Permission | |
|---|---|---|
| `/mute <player> [duration] [reason]` (alias `/cmute`) | `chatformat.mute` | `30s`, `10m`, `2h`, `1d`, `1w`, `1h30m`, a bare number (minutes) or `perm`; without a duration (or when the first word is not one) the mute is permanent and the words are the reason. Works on offline players who joined before. |
| `/unmute <player>` (alias `/cunmute`) | `chatformat.mute` | |
| `/mutes` (alias `/cmutes`) | `chatformat.mute` | active mutes, time left, reason (click: `/unmute`) |
| `/chatformat reload` | `chatformat.admin` | re-reads `config.toml` (a broken file is reported, the old config kept) |

Player names complete with Tab (`/unmute` completes muted players).

**On Rustic Server (Horizon)** `/mute` and `/unmute` are Horizon's own chat-filter commands,
which run first: use `/cmute` and `/cunmute` there (the plugin's `/mutes` works as is).

## Permissions

| Node | Default | |
|---|---|---|
| `chatformat.color` | op | colours and decorations in messages |
| `chatformat.mention` | everyone | `@name` mentions |
| `chatformat.mute` | op | `/mute`, `/unmute`, `/mutes` |
| `chatformat.mute.exempt` | op | cannot be muted |
| `chatformat.mute.notify` | op | told when someone is muted |
| `chatformat.admin` | op | `/chatformat reload` |
| `chatformat.bypass.spam` / `.caps` / `.links` / `.filter` | op | skip a filter |
| `chatformat.group.<name>` | (undeclared: op) | picks a `[[groups]]` format when no permissions plugin is installed |

## Configuration (`plugins/chat-format/config.toml`)

The shipped [`config.toml`](config.toml) documents every key (`config_version = 1`). Sections:
`[format]` (`default`, `name_hover`, `name_click`, `default_prefix`, `default_suffix`,
`default_group`), `[[groups]]` (`name`, `format`, `prefix`, `suffix`), `[join]`, `[mentions]`
(`enabled`, `format`, `sound`, `volume`, `pitch`, `action_bar`), `[spam]` (`max_messages`,
`per_seconds`, `duplicate_seconds`), `[caps]` (`enabled`, `min_letters`, `max_ratio`, `action =
"lowercase" | "block"`), `[links]` (`enabled`, `action = "block" | "mask"`, `replacement`,
`whitelist`), `[words]` (`action = "mask" | "block"`, `mask_char`, `list`), `[mute]`
(`blocked_commands`).

Notes: command arguments are trimmed, so a prefix set with `/perms ... setprefix` cannot end with
a space; put the space in the format (`{prefix} {name}`) or in the config prefix. Mutes use the
wall clock (they persist), the rate limit and duplicate windows too.

## Build and test

```sh
cd plugins/chat-format
cargo build --target wasm32-wasip2 --release        # ../target/wasm32-wasip2/release/chat_format.wasm
cargo test                                          # unit tests + tests/plugin.rs (real .wasm, with perms.wasm)
../../target/debug/rustic build --release && ../../target/debug/rustic check ../target/rustic/chat-format.rplugin --deny-warnings
```

Release component 709.2 KiB, `.rplugin` 248.4 KiB.

## Live check (2026-11-11, rustic-server `--profile fast`, two `horizon-bots`)

chat-format, perms (chat-meta provider), menus and mini-game `.rplugin`s on
`scripts/rustic-server-test.sh`; admin `config.toml` with `[words] list = ["fudge"]` (hello, also
installed, cancels "heck" itself); console `perms group default setprefix &7[Member]`.

```
[06:15:28] [Server thread/INFO]: [rustic] [chat-format] chat-format enabled: 2 group formats, 0 active mutes, chat-meta provider: true
[06:16:15] [Server thread/INFO]: System chat: [Perms] Prefix of group default set to: [Member]
[06:16:15] [Server thread/INFO]: System chat: Welcome Alpha0 to the server!            <- first_message (patch U2)
[06:17:23] [Server thread/INFO]: [rustic] reloaded chat-format
[06:17:30] [Server thread/INFO]: System chat: [-] Beta0                                 <- quit_message
```

Chat lines received by Alpha0's client over two sessions (`--capture`; the displayed, unsigned content):

```
 3.10s PlayerChat  [Member] (gray) Alpha0 [suggest_command "/msg Alpha0 ", show_text "Alpha0 / Group: default / World: overworld / Ping: 0 ms / Click to send a private message"]: hello everyone @Beta0 (yellow)
 5.10s PlayerChat  [Member]Alpha0: what the *****                         <- word filter (mask)
 3.90s PlayerChat  [Member]Beta0: why is nobody here                      <- typed "WHY IS NOBODY HERE"
 7.91s PlayerChat  [Member]Beta0: hey @Alpha0
 7.91s ActionBar   Beta0 mentioned you        7.96s Sound (entity.experience_orb.pickup)
```

Beta0's own client (non-op) for the refused lines, `/cmute` by Alpha0 and the expiry/unmute:

```
 4.09s SystemChat  Links are not allowed in chat.                         <- "join play.evil.net now" never broadcast
11.30s SystemChat  You have been muted for 5m by Alpha0. spamming
15.09s SystemChat  You are muted for 4m 57s. spamming                     <- "can I talk" cancelled
17.30s SystemChat  You can chat again.                                    <- /cunmute Beta0
22.91s PlayerChat  [Member]Beta0: free again
```

Alpha0: `Muted Beta0 for 5m. spamming`, `/cmutes` → `Muted players (1):` / `- Beta0 (4m 59s) spamming`
(click: `/unmute Beta0`). The duplicate "buy my stuff" was refused (only the first reached the
others).
