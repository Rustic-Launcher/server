# Essentials Lite

The everyday commands of a survival server in one small plugin: spawn, homes, warps, teleport
requests, `/back`, private messages and broadcasts, with a teleport warmup (cancelled when the
player moves) and a cooldown. Messages in English and French. An official Rustic Server plugin,
written from scratch for Rustic.

It works on its own and does more when other plugins are installed: with a chat-meta provider
(such as the official **Perms** plugin) private messages show prefixes, and with an economy
provider a new home can have a price. Without them it simply leaves those parts out.

## Commands

- `/spawn`: teleport to the spawn (`essentials.spawn`, everyone)
- `/setspawn`: set the spawn where you stand (`essentials.setspawn`, op)
- `/home [name]` (alias `/h`) and `/homes`: go home, list your homes (`essentials.home`, everyone)
- `/sethome [name]` and `/delhome <name>`: set, move or delete a home (`essentials.sethome`, everyone)
- `/warp <name>` and `/warps`: go to a warp, list the warps you may use (`essentials.warp`, everyone)
- `/setwarp <name>` and `/delwarp <name>`: create, move or delete a warp (`essentials.setwarp`, `essentials.delwarp`, op)
- `/tpa <player>` and `/tpahere <player>`: ask to teleport to a player, or ask them to come (`essentials.tpa`, `essentials.tpahere`, everyone)
- `/tpaccept [player]` and `/tpdeny [player]` (aliases `/tpyes`, `/tpno`): answer a request (`essentials.tpaccept`, everyone)
- `/back`: return to where you were before your last teleport or death (`essentials.back`, everyone)
- `/msg <player> <message>` (aliases `/m`, `/pm`, `/whisper`) and `/r <message>`: private messages (`essentials.msg`, everyone)
- `/broadcast <message>` (alias `/bc`): announce to everyone (`essentials.broadcast`, op)

Teleport requests show clickable **[Accept]** and **[Deny]** buttons and expire after a delay.
Homes, warps and player names complete with Tab.

## More permissions

- `essentials.homes.<n>`: up to `n` homes, for each tier of `home_tiers` (no one by default)
- `essentials.homes.unlimited`: no home limit (op)
- `essentials.sethome.free`: new homes cost nothing (op)
- `essentials.warps.<name>`: one warp, when `per_warp_permissions = true` (op)
- `essentials.warmup.bypass` and `essentials.cooldown.bypass`: teleport at once, no cooldown (op)

## What it asks the server for

- Files: its own data folder only (`plugins/essentials-lite/`, for `config.toml`)
- Network: none

Spawn, warps, homes and `/back` locations are kept in Rustic Server's plugin storage.

## Configuration

`plugins/essentials-lite/config.toml` is created on first start: the language (`en` or `fr`), warmup
and cooldown seconds, the request timeout, home limits and tiers, the price of a home, per-warp
permissions and the broadcast format. Missing keys take their defaults.

## Requirements

Rustic Server (plugin API 0.1). Optional: Perms or another chat-meta provider, an economy plugin.
Licence: MIT OR Apache-2.0.

## En français

Les commandes de tous les jours d'un serveur survie : spawn, homes, warps, demandes de
téléportation, `/back`, messages privés et annonces, avec un délai avant téléportation (annulé si
le joueur bouge) et un temps de recharge. Messages en français et en anglais. Plugin officiel de
Rustic Server, écrit de zéro pour Rustic.

- Avec un fournisseur chat-meta (comme le plugin officiel **Perms**), les messages privés affichent les préfixes ; avec un plugin d'économie, un home peut avoir un prix.
- Commandes : `/spawn`, `/setspawn`, `/home`, `/homes`, `/sethome`, `/delhome`, `/warp`, `/warps`, `/setwarp`, `/delwarp`, `/tpa`, `/tpahere`, `/tpaccept`, `/tpdeny`, `/back`, `/msg`, `/r`, `/broadcast` (permissions `essentials.*` ci-dessus).
- Accès demandés : son dossier de données uniquement, aucun accès réseau.
- Configuration : `plugins/essentials-lite/config.toml` (langue `fr` ou `en`, délais, limites de homes, prix d'un home, format des annonces).
