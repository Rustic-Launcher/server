# Mini-game

Arena mini-games for Rustic servers: **Spleef** and **King of the Hill**, with a lobby countdown
(boss bar, titles, sounds), timed rounds with a live sidebar, stats, a clickable leaderboard and
optional economy rewards. English and French. An official Rustic Server plugin, written from
scratch for Rustic.

## Modes

- **Spleef**: a snow floor is built under the arena; players dig it away with a special shovel and whoever falls is out. The last one standing wins; when time runs out, the best digger still standing wins.
- **King of the Hill**: stand alone in the zone to score a point per second, particles show its border and who holds it. The first to the target score wins, otherwise the best score when time runs out.

## How a game goes

- `/mg join` puts players in the lobby with a boss bar showing who is waiting.
- At the minimum number of players a countdown starts, with titles and sounds.
- During the round a sidebar shows every score and the time left.
- At the end the winner is announced, stats are saved and rewards are paid when an economy plugin is installed.
- Everyone goes back to the lobby with their game mode restored, the game's items taken back and the floor rebuilt.

Players take no damage and can't place blocks during a game. A player who leaves is taken out of
the game; disabling or reloading the plugin ends every game cleanly.

## Commands

All under `/mg` (alias `/minigame`):

- `/mg join [arena]`, `/mg leave`, `/mg list`: play (`minigame.play`, everyone)
- `/mg stats [player]`, `/mg top [wins|games|points]`: stats and the leaderboard (`minigame.play`)
- `/mg create <arena> [spleef|koth]`: create an arena where you stand (`minigame.admin`, op)
- `/mg setspawn`, `/mg setlobby`, `/mg setarea <arena> [radius]`: adjust an arena (`minigame.admin`)
- `/mg start <arena>`, `/mg stop <arena>`, `/mg delete <arena>`: run or remove an arena (`minigame.admin`)

## Permissions

- `minigame.play`: join games, stats and leaderboard (everyone)
- `minigame.admin`: create, set up, start, stop and delete arenas (op)

## What it asks the server for

- Files: its own data folder only (`plugins/mini-game/`, for `config.toml`)
- Network: none

Arenas and stats are kept in Rustic Server's plugin storage.

## Configuration

`plugins/mini-game/config.toml`: the language, minimum and maximum players, countdown, round and
end durations, the default arena radius, the Spleef floor block and tool, the King of the Hill
target score and particle, and the economy rewards.

## Requirements

Rustic Server (plugin API 0.1). Optional: an economy plugin for rewards. Licence: MIT OR Apache-2.0.

## En français

Des mini-jeux en arène pour les serveurs Rustic : **Spleef** et **Roi de la colline**, avec
compte à rebours dans le lobby, manches chronométrées, tableau des scores en direct, statistiques,
classement cliquable et récompenses facultatives. Plugin officiel de Rustic Server, écrit de zéro
pour Rustic.

- Jouer : `/mg join`, `/mg leave`, `/mg list`, `/mg stats`, `/mg top` (permission `minigame.play`).
- Administrer : `/mg create`, `/mg setspawn`, `/mg setlobby`, `/mg setarea`, `/mg start`, `/mg stop`, `/mg delete` (permission `minigame.admin`, op).
- Accès demandés : son dossier de données uniquement, aucun accès réseau.
