# If you come from Bukkit / Spigot / Paper

A concept-by-concept map from the Bukkit family of APIs to Rustic. It names concepts only (no
Bukkit code is reproduced); rows are ordered by how many popular plugins use the concept
(`docs/BRIDGE_ANALYSIS.md`, 55-plugin panel: `Player` 55 plugins, scheduler 51, `Location` 49,
`ItemStack` 45, join/quit events 44/40, services 26...).

Three differences shape everything else:

1. **A plugin is a sandboxed WebAssembly component**, not a jar in the server's JVM. It sees only
   the `rustic:api` imports and, if granted, its own data folder. A crash disables the plugin,
   never the server.
2. **Events are values.** A handler receives the event record and returns it; the server applies
   only the fields marked mutable plus `cancelled`. There are no live game objects to hold:
   `Player`, `World` are small handles (a UUID, a dimension id) re-resolved on every call.
3. **Two instances, no shared memory.** The main instance runs on the tick (events, commands,
   world); the async instance runs `run_async` jobs (HTTP, slow storage) and talks back with
   messages. See `docs/API.md` §3.4.

## Plugin lifecycle and metadata

| Bukkit / Paper | Rustic (SDK `rusticmc-api`, imported as `rustic`) |
|---|---|
| `JavaPlugin` subclass | a `Default` struct implementing `Plugin`, marked `#[rustic::plugin]` |
| `onLoad` / `onEnable` / `onDisable` | `Plugin::load` / `enable` / `disable` (both instances run `load`/`enable`; registrations happen in the main one) |
| `plugin.yml` (`name`, `version`, `main`, `depend`, `softdepend`, `loadbefore`, `api-version`) | `rustic.plugin.toml`: `[plugin] id/name/version/api/entry`, `[dependencies] required/optional/load_before` with semver ranges, `[capabilities]`, `[limits]` (`docs/PLUGIN_FORMAT.md`) |
| `getDataFolder()` | the data folder, preopened as `/` with `filesystem = "data-folder"` (`std::fs` works) |
| `getLogger().info(..)` | `info!(..)`, `warn!(..)`... (prefixed with the plugin id) |
| `PluginDescriptionFile` | `ctx.meta()`, `Server::plugins()` |
| `/reload`, PlugMan | `/rustic reload <id>` (disable, re-read the package, enable; dependents follow) |
| jar in `plugins/` | `.rplugin` (zip: manifest + component + `config.toml` + `resources/`) built by `rustic build` |

## Events

| Bukkit / Paper | Rustic |
|---|---|
| `implements Listener` + `@EventHandler` methods | methods with `#[event_handler]` in an `impl` marked `#[rustic::handlers]`, or `ctx.on(\|e: &mut PlayerJoinEvent\| ..)` |
| `EventPriority` LOWEST..MONITOR | `priority = "lowest" .. "monitor"`; changes made at `monitor` are ignored by the server |
| `ignoreCancelled = true` | `ignore_cancelled = true` |
| `event.setCancelled(true)` | `event.cancel()` (`Cancellable`); `set_cancelled(false)` un-cancels |
| `PlayerJoinEvent#setJoinMessage` | `PlayerJoinEvent::set_join_message` (`None` hides it) |
| `AsyncPlayerChatEvent` / Paper `AsyncChatEvent` renderer | `PlayerChatEvent`: cancel, or `set_display(text)` to change the line everyone sees (the signed message stays) |
| `BlockBreakEvent#setDropItems` | `BlockBreakEvent::set_drop_items` |
| `EntityDamageEvent#setDamage` | `EntityDamageEvent::set_amount` |
| `PlayerDeathEvent` keep inventory / level / death message | `PlayerDeathEvent::set_keep_inventory`, `set_keep_level`, `set_death_message` |
| `PlayerRespawnEvent#setRespawnLocation` | `PlayerRespawnEvent::set_location` |
| `ServerListPingEvent` | `ServerListPingEvent` (`set_motd`, max players, hide sample) |
| `PluginMessageListener` | `Messaging::register_incoming(channel)` + `PluginMessageEvent` |
| custom `Event` subclasses + `callEvent` | `Events::call_custom("my-event", &payload)` and `ctx.on_custom("<plugin>:my-event", ..)` |
| `HandlerList.unregisterAll(listener)` | automatic on disable; `unsubscribe()` on the handle `Events::subscribe` returns |

The 25 events of API 0.1 are listed in `crates/rusticmc-api-wit/wit/events.wit`.

## Commands

| Bukkit / Paper | Rustic |
|---|---|
| `getCommand("x").setExecutor(..)` + `plugin.yml` `commands:` | `#[command(name = "x", usage = "...")]` or `Command::new("x")...register()` (no manifest entry) |
| `CommandExecutor#onCommand(sender, cmd, label, String[] args)` | the executor gets a `CommandCtx` with **typed, already parsed** arguments: `cmd.arg::<Player>("target")?` |
| `TabCompleter` | client-side completion from the usage tree (`{a\|b}` suggestions, player names, numbers...) plus `#[completer("x")]` / `.completer(..)` for `*` arguments |
| `sender.sendMessage(..)` | `cmd.reply(..)` / `Sender::send_message` |
| `sender instanceof Player` | `cmd.player()?` (error message for the console) or `.players_only()` |
| `permission:` / `permission-message:` | `permission = "node"` (per command or per usage); the server sends the refusal |
| `usage:` and manual argument checks | the usage syntax `<target:player> [amount:integer(1..64)]`; wrong input gets a red error with the usage lines |
| Brigadier / Paper command API | built in: usages become a typed tree sent to the client |

## Scheduler

| Bukkit / Paper | Rustic |
|---|---|
| `runTask(plugin, r)` | `Scheduler::run_later(0, \|\| ..)` (next tick) |
| `runTaskLater(plugin, r, delay)` | `Scheduler::run_later(ticks, ..)` |
| `runTaskTimer(plugin, r, delay, period)` | `Scheduler::run_timer(delay, period, \|task\| ..)`; `task.cancel()` |
| `runTaskAsynchronously` / `runTaskTimerAsynchronously` | `ctx.async_task("job", \|payload\| ..)` registered in `enable`, started with `Scheduler::run_async("job", &payload)` / `run_async_timer` (milliseconds) |
| "go back to the main thread" (`runTask` from an async task) | `Scheduler::post_to_main(topic, &value)` + `ctx.on_message(topic, ..)` |
| Folia region schedulers | not needed: one main thread, plugins never run concurrently with themselves |
| `BukkitTask#cancel` | `Task::cancel`; everything is cancelled on disable |

## Players

| Bukkit / Paper | Rustic `Player` (a copyable UUID handle) |
|---|---|
| `getName`, `getUniqueId` | `name()`, `uuid()` |
| `Bukkit.getPlayer(name)`, `getOnlinePlayers()` | `Player::by_name`, `Player::online()` |
| `Bukkit.getOfflinePlayer(name)` | `Server::offline_player(name)` (UUID from the user cache) |
| `sendMessage`, `sendActionBar`, `sendTitle`, `playSound` | `send_message`, `action_bar`, `title` / `title_with`, `sound` |
| `teleport(Location)` | `teleport(&location)` |
| `getGameMode` / `setGameMode` | `game_mode()` / `set_game_mode()` |
| `getHealth` / `setHealth` / `getFoodLevel` / `setLevel` | `health`, `set_health`, `food_level`, `set_food_level`, `saturation`, `level`, `set_level` |
| `setAllowFlight` / `setFlying` | `set_flying(allow, flying)` |
| `kickPlayer` | `kick(text)` |
| `isOp` / `setOp`, `hasPermission` | `is_op`, `set_op`, `has_permission` |
| `getPing`, `getLocale`, `getAddress` | `ping`, `locale`, `address` |
| `PersistentDataContainer` on the player | `player.data().get/set` (JSON values, per plugin namespace, saved with the player) |
| `setPlayerListHeaderFooter`, `setPlayerListName` | `set_tab_header_footer`, `set_tab_name` |

## Worlds, blocks, entities

| Bukkit / Paper | Rustic |
|---|---|
| `World` object, `Bukkit.getWorld(name)` | `World::new("minecraft:overworld")`, `World::overworld()`, `World::all()` (dimension ids) |
| `Location` (world, x, y, z, yaw, pitch) | `Location` (same fields; `world` is the dimension id) |
| `Block` / `block.getType()` / `setType` | `world.get_block(pos)` / `world.set_block(pos, "oak_log[axis=y]")` |
| `BlockData` / `Material` | `BlockState` (namespaced id + properties), item ids as strings (`"minecraft:diamond"`) |
| bulk edits (WorldEdit-style) | `world.batch().set(..).apply()`, `world.fill(from, to, state)` (≤ 32768 blocks) |
| `getHighestBlockYAt` | `highest_block_y` |
| `getTime` / `setTime`, `setStorm` | `time`, `set_time`, `weather`, `set_weather` |
| `getSpawnLocation` / `setSpawnLocation` | `spawn_location`, `set_spawn_location` |
| `spawnEntity`, `entity.remove()`, `getNearbyEntities` | `spawn_entity`, `remove_entity`, `entities_near` |
| `spawnParticle`, `playSound` at a location | `world.particle(..).count(..).spawn()`, `world.play_sound` |
| chunk tickets (`addPluginChunkTicket`) | `keep_chunk_loaded` (released on disable) |

## Items and inventories

| Bukkit / Paper | Rustic |
|---|---|
| `new ItemStack(Material.DIAMOND, 5)` | `ItemStack::new("diamond").count(5)` |
| `ItemMeta#setDisplayName`, `setLore`, enchantments, unbreakable, custom model data | builder methods `name`, `lore`, `enchant`, `unbreakable`, `custom_model_data`, raw data components with `component(key, snbt)` |
| `player.getInventory().addItem(..)` (returns leftovers) | `player.inventory().give(&stack)?` (returns what did not fit) |
| `removeItem`, `contains` | `take(id, count)` (returns how many), `contents()` |
| `getItemInMainHand`, `getHeldItemSlot` | `main_hand()`, `held_slot()` |
| custom GUI: `Bukkit.createInventory(..)` + `InventoryClickEvent` + `event.setCancelled(true)` | `Menu::new(rows, title).item(slot, stack, \|click\| ..).open(&player)`: clicks are **cancelled by default**; `click.allow()` lets one through |
| `closeInventory()` | `MenuHandle::close()`, `player.inventory().close()` |

## Scoreboards, boss bars, tab list

| Bukkit / Paper | Rustic |
|---|---|
| `ScoreboardManager` main scoreboard, `Objective`, `Score` | `Scoreboard::create_objective`, `Objective` (names prefixed with the plugin id), `set_score`, display slots |
| `Team` prefix/suffix/color, `addEntry` | `Team` |
| `Bukkit.createBossBar(..)`, `addPlayer`, `setProgress` | `BossBar::create(..)`, `show(player)`, `set_progress`; removed on disable |
| tab list header/footer | `TabList` / `Player::set_tab_header_footer` |

## Configuration and storage

| Bukkit / Paper | Rustic |
|---|---|
| `config.yml`, `saveDefaultConfig`, `getConfig().getString(..)` | `config.toml` shipped in the package (copied once to the data folder) read into a serde struct: `Config::new().version(n).load::<MyConfig>()?` |
| `ConfigurationSection` | nested serde structs / `toml::Table` |
| config migrations by hand | `config_version` + `.migrate(from, \|table\| ..)` |
| flat files / SQLite for small data | `Storage::get/set/update` (key-value, JSON values, flushed by the server) |
| `PersistentDataContainer` (entities, items) | per-player data (`player.data()`); items carry data components (`component(..)`) |

## Services, permissions, integration

| Bukkit / Paper | Rustic |
|---|---|
| `ServicesManager` + Vault `Economy` | `Economy::balance(&player)` etc. (`rustic:services/economy`); a provider implements `ProvideEconomy` (SDK feature `provide-economy`) |
| Vault `Permission` / `Chat` (prefixes) | `PermissionProvider`, `ChatMeta`; providers implement `ProvidePermissions` / `ProvideChatMeta` |
| `plugin.yml` `permissions:` with defaults and children | `Permissions::declare(node).default(..).child(..).register()` in `load` |
| LuckPerms-style engine | the server's `permissions.toml`, or a provider plugin |
| plugin messaging (`registerOutgoingPluginChannel`, `sendPluginMessage`) | `Messaging::register_incoming`, `player.send_plugin_message` |
| BungeeCord / Velocity awareness | `Messaging::behind_proxy()` |
| HTTP with any library | `Request::get(url).send()` in the async instance, hosts listed in `capabilities.network` |

## Tooling

| Bukkit world | Rustic |
|---|---|
| Maven/Gradle archetype | `rustic new <id> --template basic\|command\|gui\|service` |
| `mvn package` | `rustic build` / `rustic package` |
| MockBukkit (`MockBukkit.mock()`, `server.addPlayer()`, `player.assertSaid(..)`) | `rusticmc-test`: `MockServer::with_built(&build_plugin(..))`, `server.add_player("Steve")`, `steve.assert_message_contains(..)` (guide: `docs/guides/testing.md`) |
| a local Paper server for trying things | `rustic run` (dev server, or a headless REPL), `--watch` for hot reload |
| "will this plugin run?" | `rustic bridge analyze plugin.jar` |

## Not available (by design or not yet)

* NMS / CraftBukkit internals, reflection, ProtocolLib-style packet access: no equivalent; the
  sandbox only exposes the WIT API.
* Arbitrary threads and blocking on the main thread: one main instance on the tick, one async
  instance; HTTP is async-only.
* What popular plugins use and API 0.1 lacks (sneaking/visibility getters, world height limits,
  plugin enable/disable events...) is ranked in `docs/BRIDGE_ANALYSIS.md` ("What Rustic's API
  should cover next").

## Running unchanged plugins: Rustic Bridge

Everything above is for porting a plugin to Rustic. To run a Bukkit/Paper jar **as it is**, drop it
in `plugins/bukkit/` of a `rustic-server` built with the bridge (`docs/BRIDGE.md`, guide:
`docs/guides/bukkit-bridge.md`). The bridge's Java layer (GPL-3.0-only, `bridge/java`) implements
the published API interfaces over Rustic; the API's own concrete classes (`JavaPlugin`,
`YamlConfiguration`, `ChatColor`, `Location`, `Vector`, `NamespacedKey`, `PluginCommand`,
`BukkitRunnable`, the event classes, Adventure) run unchanged from the Paper API jar.

| Bukkit / Paper API | Rustic Bridge 0.1 |
|---|---|
| `JavaPlugin` lifecycle, `plugin.yml` (name, version, main, depend/softdepend/loadbefore, commands, permissions) | ✅ one Rustic plugin `bukkit-<name>` per jar; depend → required dependency, softdepend → optional, loadbefore → load_before; `paper-plugin.yml`-only jars and `libraries:` are refused |
| `getConfig`, `saveDefaultConfig`, `reloadConfig`, `saveConfig`, `getResource`, `saveResource` | ✅ the API's own code, data folder `plugins/bukkit-<name>/` |
| `getLogger`, `System.out` | ✅ `logs/latest.log` as `[bukkit] [Name] ...` (SLF4J/`getComponentLogger` are no-ops) |
| `PluginManager.registerEvents` / `registerEvent`, `@EventHandler(priority, ignoreCancelled)`, `HandlerList` | ✅ priorities interleave with Rustic plugins; listeners for events not bridged are warned about |
| events: `PlayerJoinEvent`, `PlayerQuitEvent`, `AsyncPlayerChatEvent`, Paper `AsyncChatEvent`, `PlayerMoveEvent`, `PlayerTeleportEvent` (not cancellable), `PlayerCommandPreprocessEvent`, `ServerCommandEvent`, `BlockBreakEvent`, `PlayerGameModeChangeEvent`, `PlayerRespawnEvent`, `AsyncPlayerPreLoginEvent`, `PlayerChangedWorldEvent`, `PluginEnableEvent`/`PluginDisableEvent`, plugin-defined events with `callEvent` | ✅ mutable fields and cancellation go back to the server |
| other events (block place, interact, damage, death, inventory, entity...) | ❌ not fired yet |
| `CommandExecutor`, `TabCompleter`, `getCommand`, usage message on `false`, aliases, command permissions | ✅ registered in Rustic's command tree; raw string arguments |
| Paper Brigadier lifecycle commands (`registerCommand`, `LifecycleEvents.COMMANDS`) | ❌ registration ignored with a warning |
| `BukkitScheduler` (runTask, runTaskLater, runTaskTimer, the async variants, `scheduleSync*`, `callSyncMethod`, `cancelTask(s)`), `BukkitRunnable` | ✅ sync tasks in the server tick, async on a worker pool |
| Paper `GlobalRegionScheduler`, `RegionScheduler`, `AsyncScheduler`, `ScheduledTask` | ✅ on the same scheduler (no regions: one main thread) |
| API calls from async threads | ✅ marshalled to the main thread (at most one tick) |
| `Player`: name, UUID, display name, messages (legacy, Adventure, BungeeCord, MiniMessage), `hasPermission`, op, health/max health, food, saturation, level, allow-flight/flying, game mode, location, world, teleport, kick, address, ping, locale, action bar, titles, sounds, `performCommand` | ✅ |
| `Player`: inventory, items, potion effects, entities, metadata/PDC, scoreboard, boss bars, plugin messaging | ❌ `UnsupportedOperationException("Rustic Bridge: Player.<m> not supported yet")` |
| `Server`/`Bukkit`: online players, `getPlayer`, offline players, broadcast, console sender, `dispatchCommand`, plugin manager, schedulers, services manager, worlds, version strings, `getOnlineMode`, max players, motd, TPS, plugin commands, plugins folder | ✅ |
| `World` (name `world`/`world_nether`/`world_the_end` for the vanilla dimensions, key, UID, environment, height, time, spawn, players, highest block, `save` as a no-op), `Block` (position, type, `setType`, relatives) | ✅ (block data, block states, chunks, entities ❌) |
| `ServicesManager` (Vault-style providers between Java plugins) | ✅ inside the JVM; not bridged to Rustic services |
| NMS, CraftBukkit, ProtocolLib, packets | ❌ never |
| `System.exit` / `Runtime.halt` in a plugin | refused (`SecurityException`) |

Implemented share of the proxied interfaces (abstract method names handled, the rest throw; the
interfaces' default methods run as written): `Server` 52/171, `Player` 69/550 (with
`CommandSender`/`Audience`), `ConsoleCommandSender` 11/18, `PluginManager` 23/30,
`BukkitScheduler` 18/18, `ServicesManager` 8/8, `World` 15/172, `Block` 10/58; 206 of 1 025
(20%) by name, which is the long tail: weighted by use, the two demo plugins' 52 distinct API
members are all served.
