# Menus

Clickable chest menus for Rustic servers, and a recipe book of menu patterns for plugin authors.
Players open a main menu with server information, a warp selector, personal settings and a
paginated list of the players online. English and French. An official Rustic Server plugin,
written from scratch for Rustic.

## In game

`/menu` (alias `/gui`) opens the main menu:

- **Server info**: version, players, TPS and plugins, refreshed every second while the menu is open.
- **Warps**: one button per warp of the config; a click runs its commands, warps can ask for a confirmation first.
- **Settings**: per-player toggles from the config, saved with the player; shift-click restores the default.
- **Players**: the heads of every player online, 45 per page, with page buttons; shift-click teleports to a player after a confirmation (with `menus.teleport`).

Every submenu has Back and Close buttons. Items can never be taken out of a menu: every click is
cancelled, including shift-clicks, number keys, drops and double clicks. A menu closes cleanly when
its player leaves or the plugin is disabled.

## Commands and permissions

- `/menu`, `/menu warps`, `/menu settings`, `/menu players [page]`: open a menu (`menus.use`, everyone)
- shift-click on a head: teleport to that player (`menus.teleport`, op)

Players only (the console is refused).

## What it asks the server for

- Files: its own data folder only (`plugins/menus/`, for `config.toml`)
- Network: none

## Configuration

`plugins/menus/config.toml`: the language (`auto`, `en` or `fr`), the refresh period, the
`[[warps]]` (name, icon, description, commands, confirmation) and the `[[toggles]]` (id, name,
icon, description, default).

## For plugin authors

The source is a recipe book for the Rust SDK's `Menu` builder: navigation, click types, toggle
items, pagination in place, player heads, live titles, confirmation dialogs, and cleaning up on
disconnects. See the Rustic Server repository (`plugins/menus`).

## Requirements

Rustic Server (plugin API 0.1). Licence: MIT OR Apache-2.0.

## En français

Des menus coffre cliquables pour les serveurs Rustic, et un recueil d'exemples pour les auteurs de
plugins. Plugin officiel de Rustic Server, écrit de zéro pour Rustic.

- `/menu` (alias `/gui`) : informations du serveur, sélecteur de warps, réglages par joueur, liste paginée des joueurs en ligne (permission `menus.use`).
- Shift-clic sur une tête : téléportation vers le joueur après confirmation (permission `menus.teleport`, op).
- Impossible de sortir un objet d'un menu : tous les clics sont annulés.
- Accès demandés : son dossier de données uniquement, aucun accès réseau.
