# Chat Format

Chat formatting and moderation for Rustic servers: per-group line formats with prefixes,
hoverable and clickable names, colours by permission, `@mentions`, anti-spam, a link filter with
a whitelist, a word filter, mutes with an expiry, and join and quit lines. Messages in English and
French. An official Rustic Server plugin, written from scratch for Rustic.

## What it does

- Line format: every chat line is rebuilt from a template such as `{prefix}{name}{suffix}: {message}` (placeholders `{prefix}`, `{suffix}`, `{name}`, `{message}`, `{group}`, `{world}`).
- Groups: each `[[groups]]` entry of the config gets its own format, prefix and suffix.
- Prefixes: from the chat-meta service when a provider such as the official **Perms** plugin is installed, otherwise from the config.
- Names: hovering shows details (group, world, ping), clicking prepares a `/msg`.
- Colours: players with `chatformat.color` may use colour tags and `&` codes; everyone else's text is shown as typed.
- Mentions: `@name` of an online player is highlighted, with a sound and an action-bar line.
- Anti-spam: a rate limit and repeated-message detection; capital letters lower-cased or refused.
- Links: addresses refused or masked, except whitelisted domains.
- Words: a list of words masked or refused.
- Mutes: kept across restarts, lifted on time, listed by `/mutes`; muted players can't use the commands you list either (`/msg`, `/me`...).
- Join and quit lines, and a different line for a player's very first join.
- Languages: each player is answered in their client's language (French or English), or one language for everyone.

## Commands

- `/mute <player> [duration] [reason]` (alias `/cmute`): mute, for `30s`, `10m`, `2h`, `1d`, `1w` or permanently (`chatformat.mute`)
- `/unmute <player>` (alias `/cunmute`): lift a mute (`chatformat.mute`)
- `/mutes` (alias `/cmutes`): the active mutes with the time left and the reason (`chatformat.mute`)
- `/chatformat reload`: re-read `config.toml`; a broken file is reported and the old config kept (`chatformat.admin`)

On Rustic Server, `/mute` and `/unmute` are the game's own commands and run first: use `/cmute`
and `/cunmute` there.

## Permissions

- `chatformat.color`: colours and decorations in messages (op)
- `chatformat.mention`: `@name` mentions (everyone)
- `chatformat.mute`, `chatformat.mute.exempt`, `chatformat.mute.notify`: mute others, can't be muted, told about mutes (op)
- `chatformat.admin`: `/chatformat reload` (op)
- `chatformat.bypass.spam`, `.caps`, `.links`, `.filter`: skip one filter (op)
- `chatformat.group.<name>`: picks a group format when no permissions plugin is installed

## What it asks the server for

- Files: its own data folder only (`plugins/chat-format/`, for `config.toml`)
- Network: none

## Configuration

The shipped `config.toml` documents every key: `[format]`, `[[groups]]`, `[join]`, `[mentions]`,
`[spam]`, `[caps]`, `[links]`, `[words]` and `[mute]`.

## Requirements

Rustic Server (plugin API 0.1). Optional: Perms or another chat-meta provider. Join and quit lines
need a recent Rustic Server. Licence: MIT OR Apache-2.0.

## En français

Mise en forme et modération du chat pour les serveurs Rustic : formats par groupe avec préfixes,
noms survolables et cliquables, couleurs selon la permission, mentions `@pseudo`, anti-spam, filtre
de liens avec liste blanche, filtre de mots, mutes avec durée, messages d'arrivée et de départ.
Plugin officiel de Rustic Server, écrit de zéro pour Rustic.

- Préfixes fournis par le service chat-meta (le plugin officiel **Perms**) s'il est installé, sinon par la configuration.
- Commandes : `/mute` (`/cmute`), `/unmute` (`/cunmute`), `/mutes`, `/chatformat reload` (permissions `chatformat.*`).
- Chaque joueur reçoit les messages dans la langue de son jeu (français ou anglais).
- Accès demandés : son dossier de données uniquement, aucun accès réseau.
