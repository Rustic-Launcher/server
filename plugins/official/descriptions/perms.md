# Perms

Permission groups for Rustic servers: inheritance, weights, prefixes and suffixes, wildcard and
negated nodes, temporary nodes and groups, and promotion tracks. An official Rustic Server plugin,
written from scratch for Rustic.

Perms provides the standard services `permission-provider` and `chat-meta`: once it is
installed, every permission check of the server goes through it, and chat plugins (the official
**Chat Format**, `/msg` in **Essentials Lite**) take their prefixes and suffixes from it.

## How a check is decided

For a player and a node, the player's own nodes come first, then every group they inherit,
highest weight first. Inside one source the most specific entry wins (`a.b.c`, then `a.b.*`,
then `a.*`, then `*`). A node typed with a leading `-` is negated. Temporary entries count until
they expire. Players without a group are in the default group, created automatically.

## Commands

Everything is under `/perms` (alias `/perm`), permission `perms.admin` (op by default).
Durations look like `30s`, `10m`, `2h`, `7d` or `1h30m`; offline players who joined once work too.

- `/perms user <player> info`: groups, primary group, prefix and suffix, own nodes
- `/perms user <player> group add|remove|set <group>` and `group addtemp <group> <duration>`: memberships
- `/perms user <player> permission set|settemp|unset <node> ...`: the player's own nodes
- `/perms user <player> promote|demote <track>`: move along a track
- `/perms user <player> setprefix|setsuffix [text]`: a personal prefix or suffix
- `/perms group <group> create [weight]`, `delete`, `info`: groups
- `/perms group <group> permission set|settemp|unset <node> ...`: group nodes
- `/perms group <group> parent add|remove <group>`: inheritance (cycles are refused)
- `/perms group <group> setprefix|setsuffix [text]` and `setweight <weight>`: display and priority
- `/perms groups` and `/perms tracks`: every group and track, clickable
- `/perms track <track> create|delete|info|append|remove|insert ...`: promotion ladders
- `/perms check <player> <node>`: the verdict and where it comes from
- `/perms reload`, `/perms export`, `/perms import`: reload, write or read `perms.toml`

Prefixes accept `&` colour codes or MiniMessage tags.

## Permissions

- `perms.admin`: every `/perms` command (op)

## What it asks the server for

- Files: its own data folder only (`plugins/perms/`, for `config.toml` and the `perms.toml` export)
- Network: none

Groups, tracks and players are kept in Rustic Server's plugin storage and written on every change.
Imports are validated: a bad file changes nothing.

## Configuration

`plugins/perms/config.toml`: the language (`en` or `fr`), the default group, how often temporary
entries are checked, whether players are told when one expires, and the export file name.

## Requirements

Rustic Server (plugin API 0.1). Licence: MIT OR Apache-2.0.

## En français

Groupes de permissions pour les serveurs Rustic : héritage, poids, préfixes et suffixes, jokers et
permissions niées, permissions et groupes temporaires, échelles de promotion. Plugin officiel de
Rustic Server, écrit de zéro pour Rustic.

- Fournit les services standard `permission-provider` et `chat-meta` : toutes les vérifications de permissions du serveur passent par lui, et les plugins de chat lui prennent les préfixes.
- Tout est sous `/perms` (alias `/perm`), permission `perms.admin` (op) : utilisateurs, groupes, héritage, échelles, `/perms check`, export et import en TOML.
- Accès demandés : son dossier de données uniquement, aucun accès réseau.
- Configuration : `plugins/perms/config.toml` (langue `fr` ou `en`, groupe par défaut, fichier d'export).
