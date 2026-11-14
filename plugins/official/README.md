# Official plugins on the Rustic marketplace

The five official Rustic Server plugins (essentials-lite, perms, chat-format, menus, mini-game) as
the official **Rustic** account publishes them on the Rustic marketplace, with the Official and
Verified marks (launcher repository, ADR 0166).

| File | What |
|---|---|
| `plugins.json` | the manifest: id, name, summary, categories, tags, licence, links, icon, `.rplugin` file and the `rustic.plugin.toml` the version is read from |
| `descriptions/<id>.md` | the marketplace description (English, then a French section; the marketplace keeps one description per project). Headings, paragraphs and one-line `-` lists only: the launcher's and the website's renderers show nothing else |
| `icons/<id>.svg`, `icons/<id>.png` | original icons (simple geometry, no game assets); `python icons/render.py` rewrites both from the shapes in the script (needs Pillow) |

## Publish

1. Build the packages (from the repository root):

   ```sh
   for p in essentials-lite perms chat-format menus mini-game; do ./target/debug/rustic build plugins/$p --release; done
   ```

   They land in `plugins/target/rustic/<id>.rplugin` (`rustic build` is deterministic).

2. Publish with the launcher repository's script (an admin account; the password and 2FA code are
   asked, never passed on the command line):

   ```sh
   cd <launcher checkout>/launcher
   RUSTIC_API_URL=https://api.rustic-launcher.com node scripts/official/publish.mjs \
     --manifest <this repository>/plugins/official/plugins.json --login <admin email> [--dry-run]
   ```

   The script creates missing projects, uploads only versions the marketplace does not have yet,
   edits page fields that changed and gives the Verified badge. Running it again changes nothing.

A new release of a plugin: bump `version` in its `rustic.plugin.toml`, rebuild, run the same
command (set `changelog` in `plugins.json` first). The API checks that the uploaded file's manifest
says the same id and version, and refuses re-uploading a published version with other bytes.
