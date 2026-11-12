# Bridge analysis: what popular Bukkit/Paper plugins use, and what Rustic covers

Static analysis of **55 popular server plugins** (fetched 2026-11-09 from Modrinth and Hangar)
with `rustic-bridge-analyze` (`bridge/rusticmc-bridge-analyze`, rules in
[ADR 0008](DECISIONS/0008-bridge-analyzer.md)). It answers two questions for Rustic Bridge
(phase 4): how many existing plugins could run on Rustic through a Bukkit-API layer, and which
parts of the API Rustic's WIT (`rustic:api@0.1.0`) should cover next.

## Headline

- **0 🟢 / 12 🟡 / 43 🔴** out of 55. No popular plugin restricts itself to what Rustic 0.1
  covers today, and **78% touch server internals** (NMS, CraftBukkit, Netty, Mojang libraries,
  ProtocolLib/PacketEvents, or reflection into them) somewhere in their code or bundled libraries.
- The red figure is an upper bound on "will never run unchanged": 5 red plugins are red **only
  because of a bundled library** (EssentialsX, PlaceholderAPI and PluginPortal through the shaded
  `adventure-platform-bukkit`, which uses native Adventure when the server provides it;
  CrazyCrates and FancyHolograms through their authors' own NMS libraries), and 5 are red **only
  through reflection strings** (LuckPerms, Multiverse-Core, PowerRanks, Skript, WorldGuard),
  typically optional or guarded code paths. Generously, ~8 of the 43 could behave as yellow
  under a bridge that implements Paper's native Adventure API and tolerates failing reflection.
- On the API side the picture is better than the verdicts suggest: **79% of all bytecode call
  sites into the Bukkit/Paper API hit a member Rustic covers** (10% of them through a partial
  mapping), and the median plugin has 79% of the distinct members it uses covered (88% of its
  call sites). What blocks plugins is the **long tail**: the median plugin uses 335 distinct API
  members (min 6, max 3 263), and only 4 114 of the 9 039 distinct members seen in the panel are
  covered (46%).
- The single most common gap is trivial: `Bukkit.getOnlineMode()` (44 plugins, mostly through
  the bundled bStats metrics client). Next come block entities (`Block#getState`, 25 plugins),
  plugin enable/disable events (24), registries and item/block properties (`Registry`,
  `Material#isItem/isBlock`, 16 to 24), world height (17), and Paper's Brigadier command
  lifecycle (13 to 15).

## Methodology

1. **Panel** (`bridge/panel/fetch.py`, Python 3 stdlib only). Official public APIs only, no
   login, no SpigotMC scraping: the Modrinth search API (`/v2/search`, facets
   `categories:bukkit|spigot|paper` and `project_type:plugin`, sorted by downloads) and the Hangar
   API (`/api/v1/projects?sort=-downloads`). A list of well-known plugins is tried first
   (EssentialsX, LuckPerms, VaultUnlocked, WorldEdit, WorldGuard, ViaVersion, Chunky, DiscordSRV,
   PlaceholderAPI, CoreProtect, Multiverse, GriefPrevention, Geyser, TAB, ...), then the panel is
   filled from both most-downloaded lists, interleaved. For each project the script takes the
   release supporting the newest Minecraft version (26.3 for most), skips client mods whose
   Modrinth project also targets Fabric/Forge/NeoForge/Quilt (they would skew a server-plugin
   panel; only explicitly listed ones are kept), skips Hangar entries that only link to an
   external site (EssentialsX, Citizens) and jars over 60 MB, checks the API-provided SHA-256/512
   and records name, version, source URL, licence and SHA-256 in `bridge/panel/panel.json`.
   55 plugins, 267 MB, 122 784 classes; every class file parsed.
   Not available through these APIs as a Bukkit jar: Citizens, Floodgate, spark (Modrinth lists
   only its mod loaders), QuickShop-Hikari, NuVotifier; EssentialsX comes from Modrinth.
2. **Analysis** (`sh bridge/panel/analyze.sh`). For each jar: descriptors, every class (nested
   jars included), references to classes outside the jar, member references with call-site
   counts, attribution of inherited calls to the API superclass, coverage lookup in
   `bridge/rusticmc-bridge-analyze/data/coverage.toml`, and the verdict rules of ADR 0008.
   Full output: `bridge/panel/report.md` (with one section per plugin) and
   `bridge/panel/report.json` (compact: per plugin, the 15 most used uncovered members and types;
   run the binary with `--json` and no `--compact` for every member).
3. **Coverage table**. Built by hand from the WIT files. It counts as covered: WIT functions and
   the 25 events; pure-Java API a bridge implements in the JVM without any host call (marked
   `java-side`: configuration, Adventure and BungeeCord components, `Location`, `Vector`,
   colours, `NamespacedKey`...); `partial` mappings with a stated gap (worlds are dimension ids,
   not folder names; `ItemMeta` goes through SNBT data components; `callEvent` only for custom
   events...). Partial mappings are counted as covered and listed separately below. A test checks
   that every WIT function the table names exists.

Reading the tables: `o.b.` abbreviates `org.bukkit.`; members are `Owner#name` with overloads
merged (for coverage, the least covered overload wins); "Plugins" counts plugins referencing it,
"Sites" counts bytecode instructions (invocations, field accesses, method handles) over the panel.
Class "sites" are the member call sites on that class. A ✅ mapping means covered, ◐ partial, ❌
uncovered.

## Results

### Distribution

| Verdict | Plugins | Share |
|---|---:|---:|
| 🟢 green | 0 | 0% |
| 🟡 yellow | 12 | 22% |
| 🔴 red | 43 | 78% |
| **analyzed** | **55** | 100% |

Of the 43 red plugins, 5 are red only because of a bundled library (their own code is clean), and 5 only because of reflection with internal class names (often a guarded fallback).

API coverage over the panel: 4114 of 9039 distinct API members used (46%), and 79.4% of the 93964 bytecode sites that call the API (10.4% through a partial mapping).

### Red signals

| Signal | Red plugins | Of which in own code |
|---|---:|---:|
| NMS (net.minecraft) | 18 | 16 |
| CraftBukkit | 17 | 15 |
| server internals (Mojang libs, Netty, Paper internals) | 31 | 24 |
| ProtocolLib / PacketEvents | 4 | 4 |
| reflection into internals | 37 | 31 |
| bytecode instrumentation | 2 | 2 |
| Unsafe / jdk.internal | 4 | 4 |

### Per plugin

| Plugin | Version | Source | Licence | Verdict | API members (covered / used) | Uncovered members | Uncovered types | Why |
|---|---|---|---|:---:|---:|---:|---:|---|
| BlueMap | 5.29-spigot | modrinth | MIT | 🟡 | 82 / 96 | 14 | 3 | o.b.World#save, o.b.event.server.TabCompleteEvent#getCompletions, o.b.Bukkit#getOnlineMode |
| ChestSort | 1.1 | modrinth | MIT | 🟡 | 38 / 39 | 1 | 0 | o.b.Bukkit#getOnlineMode |
| Chunky | 1.5.66 | modrinth | GPL-3.0-only | 🟡 | 146 / 183 | 37 | 22 | io.papermc.paper.command.brigadier.CommandSourceStack#getSender, o.b.World#getWorldFolder, o.b.Registry#SOUND_EVENT |
| GriefPrevention | 16.18.8 | modrinth | GPL-3.0-only | 🟡 | 693 / 1056 | 363 | 138 | o.b.Tag#getValues, o.b.Tag#isTagged, o.b.event.entity.EntityChangeBlockEvent#setCancelled |
| InstantRestock | 2.6.7 | modrinth | MIT | 🟡 | 109 / 119 | 10 | 5 | o.b.entity.AbstractVillager#getPersistentDataContainer, o.b.entity.AbstractVillager#getRecipes, o.b.entity.AbstractVillager#getType |
| LifeStealZ | 2.21.1 | modrinth | GPL-3.0-only | 🟡 | 336 / 437 | 101 | 46 | o.b.entity.BlockDisplay#setTransformation, o.b.util.Transformation#<init>, io.papermc.paper.datacomponent.DataComponentTypes#CUSTOM_MODEL_DATA |
| Maintenance | 5.1.1 | modrinth | GPL-3.0-only | 🟡 | 107 / 122 | 15 | 6 | com.destroystokyo.paper.event.server.PaperServerListPingEvent#getListedPlayers, com.destroystokyo.paper.event.server.PaperServerListPingEvent#getPlayerSample, com.destroystokyo.paper.event.server.PaperServerListPingEvent#setProtocolVersion |
| MiniMOTD | 2.2.5 | hangar | MIT | 🟡 | 97 / 111 | 14 | 13 | io.papermc.paper.command.brigadier.CommandSourceStack#getSender, com.destroystokyo.paper.event.server.PaperServerListPingEvent#getClient, com.destroystokyo.paper.event.server.PaperServerListPingEvent#getListedPlayers |
| Multiverse-Portals | 5.3.0 | modrinth | BSD-3-Clause | 🟡 | 140 / 183 | 43 | 12 | o.b.event.player.PlayerBucketEmptyEvent#getPlayer, o.b.event.player.PlayerBucketFillEvent#getBlockClicked, o.b.event.player.PlayerPortalEvent#setCancelled |
| SetHome | 6.3 | modrinth | MIT | 🟡 | 65 / 66 | 1 | 0 | o.b.OfflinePlayer#hasPlayedBefore |
| ToolStats | 2.0.7 | hangar | GPL | 🟡 | 197 / 277 | 80 | 46 | o.b.inventory.ShapedRecipe#setIngredient, o.b.event.inventory.PrepareAnvilEvent#setResult, o.b.inventory.ShapedRecipe#<init> |
| Vault | 2.20.3 | modrinth | LGPL-3.0-or-later | 🟡 | 67 / 83 | 16 | 8 | o.b.permissions.PermissionAttachmentInfo#getAttachment, com.destroystokyo.paper.exception.ServerPluginException#getCause, o.b.entity.Player#getEffectivePermissions |
| AuthMe | 5.7.0-FORK-b53 | modrinth | AGPL-3.0-only | 🔴 | 349 / 444 | 95 | 37 | ProtocolLib / PacketEvents: com.comphenix.protocol, com.comphenix.protocol.events, com.comphenix.protocol.reflect (31 refs, in own); reflection into internals: Craft* class name, getHandle(), net.minecraft. (16 refs, in fr.xephi.authme.libs.net.kyori, own) |
| AxiomPaper | 6.0.1+26.3 | modrinth | MIT | 🔴 | 114 / 144 | 30 | 22 | NMS (net.minecraft): net.minecraft, net.minecraft.commands.arguments, net.minecraft.core (602 refs, in own); CraftBukkit: org.bukkit.craftbukkit, org.bukkit.craftbukkit.block, org.bukkit.craftbukkit.entity (55 refs, in own); server internals (Mojang libs, Netty, Paper internals): com.mojang.datafixers, com.mojang.datafixers.util, com.mojang.serialization (84 refs, in own) |
| CommandAPI | 12.1.0 | hangar | MIT | 🔴 | 254 / 365 | 111 | 69 | NMS (net.minecraft): net.minecraft.advancements, net.minecraft.advancements.critereon, net.minecraft.advancements.criterion (1167 refs, in own); CraftBukkit: org.bukkit.craftbukkit, org.bukkit.craftbukkit.block, org.bukkit.craftbukkit.command (164 refs, in own); server internals (Mojang libs, Netty, Paper internals): com.mojang.datafixers.util, com.mojang.logging, com.mojang.serialization (59 refs, in own) |
| CoreProtect | 24.1 | modrinth | Artistic-2.0 | 🔴 | 962 / 1628 | 666 | 222 | reflection into internals: getHandle() (1 refs, in own); bytecode instrumentation: javassist, javassist.bytecode (11 refs, in own) |
| CrazyCrates | 5.2.0 | modrinth | MIT | 🔴 | 410 / 805 | 395 | 85 | (bundled libraries only) NMS (net.minecraft): net.minecraft.commands.arguments, net.minecraft.core, net.minecraft.core.component (18 refs, in libs.com.ryderbelserion); CraftBukkit: org.bukkit.craftbukkit, org.bukkit.craftbukkit.entity, org.bukkit.craftbukkit.inventory (5 refs, in libs.com.ryderbelserion); reflection into internals: Craft* class name (2 refs, in libs.com.ryderbelserion) |
| DeathChest | 3.0.1 | hangar | MIT | 🔴 | 279 / 353 | 74 | 17 | ProtocolLib / PacketEvents: com.comphenix.protocol, com.comphenix.protocol.events, com.comphenix.protocol.reflect (10 refs, in own); reflection into internals: getHandle(), net.minecraft.server, net.minecraft.world (4 refs, in own) |
| DecentHolograms | 2.10.2 | modrinth | GPL-3.0-or-later | 🔴 | 600 / 1003 | 403 | 136 | NMS (net.minecraft): net.minecraft, net.minecraft.core, net.minecraft.core.registries (1909 refs, in own); CraftBukkit: org.bukkit.craftbukkit, org.bukkit.craftbukkit.entity, org.bukkit.craftbukkit.inventory (192 refs, in own); server internals (Mojang libs, Netty, Paper internals): com.mojang.authlib, com.mojang.authlib.properties, com.mojang.datafixers (367 refs, in eu.decentsoftware.holograms.libs.nbtapi, eu.decentsoftware.holograms.libs.nbtapi.utils, eu.decentsoftware.holograms.libs.xseries.profiles); reflection into internals: Craft* class name, com.mojang.authlib, getHandle() (103 refs, in eu.decentsoftware.holograms.libs.nbtapi.utils, eu.decentsoftware.holograms.libs.xseries, eu.decentsoftware.holograms.libs.xseries.messages) |
| DiscordSRV | 1.30.5 | modrinth | GPL-3.0-or-later | 🔴 | 286 / 343 | 57 | 22 | server internals (Mojang libs, Netty, Paper internals): io.netty.buffer (5 refs, in github.scarsz.discordsrv.dependencies.kyori.adventure); ProtocolLib / PacketEvents: com.comphenix.protocol.events (1 refs, in own); reflection into internals: Craft* class name, com.mojang.authlib, getHandle() (32 refs, in github.scarsz.discordsrv.dependencies.kyori.adventure, own) |
| dynmap | 3.5-spigot | modrinth | Apache-2.0 | 🔴 | 143 / 221 | 78 | 29 | NMS (net.minecraft): net.minecraft.core, net.minecraft.core.registries, net.minecraft.nbt (513 refs, in own); CraftBukkit: org.bukkit.craftbukkit.v1_14_R1, org.bukkit.craftbukkit.v1_15_R1, org.bukkit.craftbukkit.v1_16_R1 (38 refs, in own); server internals (Mojang libs, Netty, Paper internals): com.mojang.authlib, com.mojang.authlib.properties (18 refs, in own); reflection into internals: com.mojang.authlib, getHandle(), net.minecraft.server (43 refs, in own) |
| Essentials | 2.22.0 | modrinth | GPL-3.0-only | 🔴 | 837 / 1440 | 603 | 234 | (bundled libraries only) server internals (Mojang libs, Netty, Paper internals): io.netty.buffer (4 refs, in com.earth2me.essentials.libs.kyori.adventure); reflection into internals: Craft* class name, getHandle(), net.minecraft. (25 refs, in com.earth2me.essentials.libs.kyori.adventure, net.ess3.nms) |
| FancyHolograms | 2.12.0 | modrinth | MIT | 🔴 | 285 / 323 | 38 | 18 | (bundled libraries only) NMS (net.minecraft): net.minecraft, net.minecraft.core, net.minecraft.core.registries (1302 refs, in de.oliver.fancysitula); CraftBukkit: org.bukkit.craftbukkit, org.bukkit.craftbukkit.block, org.bukkit.craftbukkit.entity (48 refs, in de.oliver.fancysitula); server internals (Mojang libs, Netty, Paper internals): com.mojang.authlib, com.mojang.authlib.properties, com.mojang.datafixers.util (106 refs, in de.oliver.fancysitula); reflection into internals: net.minecraft.world (271 refs, in de.oliver.fancysitula) |
| FancyNpcs | 2.12.1 | modrinth | MIT | 🔴 | 487 / 563 | 76 | 41 | NMS (net.minecraft): net.minecraft, net.minecraft.core, net.minecraft.core.registries (3128 refs, in de.oliver.fancysitula, own); CraftBukkit: org.bukkit.craftbukkit, org.bukkit.craftbukkit.block, org.bukkit.craftbukkit.entity (83 refs, in de.oliver.fancysitula, own); server internals (Mojang libs, Netty, Paper internals): com.mojang.authlib, com.mojang.authlib.properties, com.mojang.datafixers.util (176 refs, in de.oliver.fancysitula, own); reflection into internals: Craft* class name, getHandle(), net.minecraft. (294 refs, in de.oliver.cloud, de.oliver.fancysitula) |
| FastAsyncWorldEdit | 2.16.2 | modrinth | GPL-3.0-only | 🔴 | 282 / 412 | 130 | 81 | NMS (net.minecraft): net.minecraft, net.minecraft.core, net.minecraft.core.component (6395 refs, in own); CraftBukkit: org.bukkit.craftbukkit, org.bukkit.craftbukkit.block, org.bukkit.craftbukkit.entity (337 refs, in own); server internals (Mojang libs, Netty, Paper internals): ca.spottedleaf.moonrise.patches, com.mojang.authlib, com.mojang.datafixers (476 refs, in own); reflection into internals: Craft* class name, getHandle(), net.minecraft.nbt (10 refs, in com.fastasyncworldedit.bukkit, com.fastasyncworldedit.core, own); Unsafe / jdk.internal: sun.misc (16 refs, in own) |
| Geyser-Spigot | 2.12.1-b1298 | modrinth | MIT | 🔴 | 251 / 326 | 75 | 41 | NMS (net.minecraft): net.minecraft.core, net.minecraft.core.component, net.minecraft.core.registries (226 refs, in org.geysermc.erosion, own); CraftBukkit: org.bukkit.craftbukkit, org.bukkit.craftbukkit.block, org.bukkit.craftbukkit.v1_17_R1 (18 refs, in org.geysermc.erosion, own); server internals (Mojang libs, Netty, Paper internals): io.netty.bootstrap, io.netty.buffer, io.netty.channel (3452 refs, in io.netty.handler, net.lenni0451.commons, org.cloudburstmc.netty); reflection into internals: Craft* class name, getHandle(), io.netty.channel (52 refs, in org.geysermc.erosion, org.geysermc.geyser.platform.spigot.shaded.me.lucko, org.geysermc.geyser.platform.spigot.shaded.org.incendo) |
| GrimAC | 2.3.74 | modrinth | GPL-3.0-or-later | 🔴 | 391 / 469 | 78 | 38 | server internals (Mojang libs, Netty, Paper internals): io.netty.buffer, io.netty.channel, io.netty.handler.codec (89 refs, in ac.grim.grimac.shaded.io.github, ac.grim.grimac.shaded.kyori.adventure, own); reflection into internals: Craft* class name, com.mojang.authlib, getHandle() (64 refs, in ac.grim.grimac.shaded.incendo.cloud, ac.grim.grimac.shaded.io.github, ac.grim.grimac.shaded.kyori.adventure); Unsafe / jdk.internal: sun.misc (1 refs, in own) |
| GSit | 3.8.0 | hangar | GPL | 🔴 | 258 / 363 | 105 | 42 | NMS (net.minecraft): net.minecraft.core, net.minecraft.nbt, net.minecraft.network (2037 refs, in own); CraftBukkit: org.bukkit.craftbukkit, org.bukkit.craftbukkit.entity, org.bukkit.craftbukkit.v1_18_R1 (232 refs, in own); server internals (Mojang libs, Netty, Paper internals): ca.spottedleaf.moonrise.patches, com.mojang.authlib, com.mojang.authlib.properties (203 refs, in own); reflection into internals: getHandle() (1 refs, in own) |
| ImageFrame | 2026.1.5 | hangar | GPL | 🔴 | 332 / 461 | 129 | 41 | NMS (net.minecraft): net.minecraft, net.minecraft.core, net.minecraft.core.component (1208 refs, in own); CraftBukkit: org.bukkit.craftbukkit, org.bukkit.craftbukkit.entity, org.bukkit.craftbukkit.inventory (294 refs, in own); Unsafe / jdk.internal: sun.misc (8 refs, in own) |
| InvSeePlusPlus | 0.31.22 | modrinth | LGPL-2.1-only | 🔴 | 335 / 454 | 119 | 72 | NMS (net.minecraft): net.minecraft, net.minecraft.core, net.minecraft.nbt (3670 refs, in own); CraftBukkit: org.bukkit.craftbukkit, org.bukkit.craftbukkit.entity, org.bukkit.craftbukkit.inventory (860 refs, in own); server internals (Mojang libs, Netty, Paper internals): com.mojang.authlib, com.mojang.datafixers.types, com.mojang.datafixers.util (144 refs, in own); reflection into internals: org.bukkit.craftbukkit (22 refs, in own) |
| LagFixer | 1.7.2 | modrinth | GPL-3.0-or-later | 🔴 | 347 / 501 | 154 | 62 | NMS (net.minecraft): net.minecraft.core, net.minecraft.nbt, net.minecraft.server.dedicated (2290 refs, in own); CraftBukkit: org.bukkit.craftbukkit, org.bukkit.craftbukkit.entity, org.bukkit.craftbukkit.event (161 refs, in own); server internals (Mojang libs, Netty, Paper internals): com.mojang.authlib, com.mojang.authlib.properties, com.mojang.serialization (69 refs, in own, xyz.lychee.lagfixer.libs.kyori.adventure); ProtocolLib / PacketEvents: com.github.retrooper.packetevents (12 refs, in own); reflection into internals: Craft* class name, getHandle(), net.minecraft. (16 refs, in own, xyz.lychee.lagfixer.libs.kyori.adventure) |
| LuckPerms | v5.5.71-bukkit | modrinth | MIT | 🔴 | 139 / 176 | 37 | 13 | reflection into internals: Craft* class name (2 refs, in own) |
| Multiverse-Core | 5.8.2 | modrinth | BSD-3-Clause | 🔴 | 383 / 597 | 214 | 55 | reflection into internals: Craft* class name, net.minecraft.world, org.bukkit.craftbukkit (4 refs, in org.mvplugins.multiverse.external.acf.commands, own) |
| Multiverse-Inventories | 5.3.6 | modrinth | BSD-3-Clause | 🔴 | 232 / 343 | 111 | 35 | reflection into internals: getHandle(), net.minecraft.server, org.bukkit.craftbukkit (4 refs, in own); bytecode instrumentation: org.objectweb.asm (7 refs, in own) |
| NBTAPI | 2.16.1 | modrinth | MIT | 🔴 | 76 / 87 | 11 | 3 | server internals (Mojang libs, Netty, Paper internals): com.mojang.authlib, com.mojang.authlib.properties, com.mojang.datafixers (20 refs, in own); reflection into internals: Craft* class name, com.mojang.authlib, getHandle() (83 refs, in own) |
| nightcore | 2.16.7 | modrinth | GPL-3.0-only | 🔴 | 703 / 1004 | 301 | 136 | server internals (Mojang libs, Netty, Paper internals): com.mojang.datafixers, com.mojang.serialization (14 refs, in own); reflection into internals: Craft* class name, net.minecraft.core, net.minecraft.nbt (13 refs, in own) |
| packetevents | 2.14.0+spigot | modrinth | GPL-3.0-only | 🔴 | 117 / 136 | 19 | 9 | server internals (Mojang libs, Netty, Paper internals): io.netty.buffer, io.netty.channel, io.netty.handler.codec (89 refs, in own); reflection into internals: Craft* class name, com.mojang.authlib, getHandle() (20 refs, in own) |
| PlaceholderAPI | 2.12.3 | hangar | GPL | 🔴 | 281 / 309 | 28 | 10 | (bundled libraries only) server internals (Mojang libs, Netty, Paper internals): io.netty.buffer (5 refs, in me.clip.placeholderapi.libs.kyori.adventure); reflection into internals: Craft* class name, getHandle(), net.minecraft. (15 refs, in me.clip.placeholderapi.libs.kyori.adventure) |
| Plan | 5.8+build.3638 | modrinth | LGPL-3.0-or-later | 🔴 | 188 / 212 | 24 | 31 | NMS (net.minecraft): net.minecraft.server (1 refs, in own); reflection into internals: Craft* class name, getHandle(), net.minecraft.server (13 refs, in own) |
| PlayerKits2 | 1.24.2 | modrinth | MIT | 🔴 | 304 / 398 | 94 | 30 | server internals (Mojang libs, Netty, Paper internals): com.mojang.authlib, com.mojang.authlib.properties (3 refs, in own); reflection into internals: Craft* class name, getHandle(), net.minecraft.nbt (16 refs, in own, pk.ajneb97.libs.actionbar, pk.ajneb97.libs.titles) |
| PluginPortal | 3.8.7 | hangar | Unspecified | 🔴 | 264 / 312 | 48 | 23 | (bundled libraries only) server internals (Mojang libs, Netty, Paper internals): io.netty.buffer (5 refs, in net.kyori.adventure); reflection into internals: Craft* class name, getHandle(), net.minecraft. (18 refs, in net.kyori.adventure, revxrsal.commands.bukkit) |
| PowerRanks | 1.10.10 | modrinth | MIT | 🔴 | 268 / 335 | 67 | 21 | reflection into internals: Craft* class name, net.minecraft.server, org.bukkit.craftbukkit (5 refs, in own) |
| ProtocolLib | 5.1.0 | hangar | GPL | 🔴 | 169 / 210 | 41 | 21 | server internals (Mojang libs, Netty, Paper internals): com.mojang.datafixers.util, io.netty.buffer, io.netty.channel (95 refs, in own); reflection into internals: Craft* class name, com.mojang.authlib, getHandle() (31 refs, in own); Unsafe / jdk.internal: sun.misc (1 refs, in own) |
| SkBee | 3.26.0 | modrinth | GPL-3.0-only | 🔴 | 805 / 2068 | 1263 | 418 | server internals (Mojang libs, Netty, Paper internals): com.mojang.authlib, com.mojang.authlib.properties, com.mojang.datafixers (19 refs, in own); reflection into internals: Craft* class name, com.mojang.authlib, getHandle() (108 refs, in own) |
| SkinsRestorer | 15.12.6 | modrinth | GPL-3.0-only | 🔴 | 662 / 1136 | 474 | 159 | NMS (net.minecraft): net.minecraft.core, net.minecraft.network.protocol, net.minecraft.resources (305 refs, in own); CraftBukkit: org.bukkit.craftbukkit.entity, org.bukkit.craftbukkit.v1_18_R1, org.bukkit.craftbukkit.v1_18_R2 (22 refs, in own); server internals (Mojang libs, Netty, Paper internals): com.mojang.authlib, com.mojang.authlib.minecraft, com.mojang.authlib.properties (36 refs, in net.skinsrestorer.shadow); reflection into internals: Craft* class name, com.mojang.authlib, getHandle() (55 refs, in net.skinsrestorer.shadow, own) |
| Skript | 2.17.1 | modrinth | GPL-3.0-or-later | 🔴 | 1426 / 3263 | 1837 | 613 | reflection into internals: org.bukkit.craftbukkit (2 refs, in own) |
| squaremap | 1.4.3 | modrinth | MIT | 🔴 | 334 / 376 | 42 | 40 | NMS (net.minecraft): net.minecraft, net.minecraft.client, net.minecraft.client.color (343 refs, in own); CraftBukkit: org.bukkit.craftbukkit, org.bukkit.craftbukkit.entity, org.bukkit.craftbukkit.persistence (8 refs, in own); server internals (Mojang libs, Netty, Paper internals): ca.spottedleaf.concurrentutil.executor, ca.spottedleaf.concurrentutil.lock, ca.spottedleaf.concurrentutil.util (48 refs, in own); reflection into internals: Craft* class name, getHandle(), net.minecraft. (24 refs, in own, squaremap.libraries.org.incendo) |
| TAB | 6.2.1 | modrinth | Apache-2.0 | 🔴 | 229 / 243 | 14 | 5 | NMS (net.minecraft): net.minecraft, net.minecraft.commands, net.minecraft.core (1568 refs, in own); CraftBukkit: org.bukkit.craftbukkit.entity, org.bukkit.craftbukkit.v1_12_R1, org.bukkit.craftbukkit.v1_16_R3 (78 refs, in own); server internals (Mojang libs, Netty, Paper internals): com.mojang.authlib, com.mojang.authlib.properties, com.mojang.logging (209 refs, in own) |
| TabTPS | 1.4.3 | hangar | MIT | 🔴 | 282 / 350 | 68 | 35 | server internals (Mojang libs, Netty, Paper internals): io.netty.buffer (5 refs, in xyz.jpenilla.tabtps.lib.net.kyori); reflection into internals: Craft* class name, getHandle(), net.minecraft. (56 refs, in own, xyz.jpenilla.tabtps.lib.net.kyori, xyz.jpenilla.tabtps.lib.org.incendo) |
| ViaBackwards | 5.12.2 | hangar | GPL | 🔴 | 47 / 56 | 9 | 5 | server internals (Mojang libs, Netty, Paper internals): io.netty.buffer (5 refs, in own) |
| ViaRewind | 4.2.0 | hangar | GPL | 🔴 | 6 / 6 | 0 | 0 | server internals (Mojang libs, Netty, Paper internals): io.netty.buffer, io.netty.channel, io.netty.handler.codec (38 refs, in own) |
| ViaVersion | 5.12.2 | hangar | GPL | 🔴 | 171 / 212 | 41 | 12 | server internals (Mojang libs, Netty, Paper internals): io.netty.buffer, io.netty.channel, io.netty.handler.codec (404 refs, in own); reflection into internals: Craft* class name, getHandle(), io.netty.channel (30 refs, in own) |
| voicechat | bukkit-2.6.25 | modrinth | LicenseRef-All-Rights-Reserved | 🔴 | 120 / 134 | 14 | 3 | server internals (Mojang libs, Netty, Paper internals): io.netty.buffer, io.netty.handler.codec, io.netty.util (31 refs, in own); reflection into internals: Craft* class name, getHandle(), net.minecraft. (61 refs, in own) |
| WorldEdit | 7.4.6 | modrinth | GPL-3.0-only | 🔴 | 225 / 315 | 90 | 52 | NMS (net.minecraft): net.minecraft, net.minecraft.core, net.minecraft.core.component (4111 refs, in own); CraftBukkit: org.bukkit.craftbukkit, org.bukkit.craftbukkit.block, org.bukkit.craftbukkit.entity (133 refs, in own); server internals (Mojang libs, Netty, Paper internals): com.mojang.authlib, com.mojang.datafixers, com.mojang.datafixers.schemas (307 refs, in own); reflection into internals: Craft* class name, getHandle(), net.minecraft.server (7 refs, in own) |
| WorldGuard | 7.0.21 | modrinth | LGPL-3.0-only | 🔴 | 1378 / 1946 | 568 | 180 | reflection into internals: Craft* class name, getHandle() (3 refs, in own) |

### Top 50 Bukkit/Paper classes (by plugins using them)

| # | Name | Plugins | Sites | Rustic |
|---:|---|---:|---:|---|
| 1 | `o.b.entity.Player` | 55 | 6639 | ✅ `types.uuid` |
| 2 | `o.b.Bukkit` | 55 | 3979 | ✅ `see members` |
| 3 | `o.b.plugin.Plugin` | 54 | 1777 | ✅ `plugin.on-enable` |
| 4 | `o.b.plugin.java.JavaPlugin` | 53 | 2156 | ✅ `plugin.on-enable` |
| 5 | `o.b.Server` | 52 | 1704 | ✅ `see members` |
| 6 | `o.b.plugin.PluginManager` | 52 | 1289 | ✅ `see members` |
| 7 | `o.b.event.EventHandler` | 52 | 0 | ✅ `events.subscribe` |
| 8 | `o.b.scheduler.BukkitScheduler` | 51 | 499 | ✅ `see members` |
| 9 | `o.b.event.Listener` | 51 | 12 | ✅ `events.subscribe` |
| 10 | `o.b.command.CommandSender` | 50 | 1839 | ✅ `types.command-sender` |
| 11 | `o.b.plugin.PluginDescriptionFile` | 50 | 459 | ✅ `see members` |
| 12 | `o.b.scheduler.BukkitTask` | 50 | 105 | ✅ `see members` |
| 13 | `o.b.Location` | 49 | 6991 | ✅ `types.location` |
| 14 | `o.b.World` | 49 | 1974 | ◐ a dimension id (minecraft:overworld...); no per-world UID or folder name |
| 15 | `o.b.configuration.file.YamlConfiguration` | 49 | 1825 | ✅ `java-side: YAML configuration, files in the plugin data folder (wasi:filesystem data-folder for native plugins)` |
| 16 | `o.b.event.EventPriority` | 48 | 86 | ✅ `types.priority` |
| 17 | `o.b.Material` | 47 | 7016 | ◐ namespaced item/block ids (types.item-stack id, types.block-state id); properties need registry data not in the WIT |
| 18 | `o.b.entity.Entity` | 47 | 962 | ✅ `types.entity-ref` |
| 19 | `o.b.inventory.ItemStack` | 45 | 5365 | ✅ `types.item-stack` |
| 20 | `o.b.block.Block` | 44 | 2851 | ◐ a (world, block-pos) handle over worlds.get-block / worlds.set-block |
| 21 | `o.b.event.player.PlayerJoinEvent` | 44 | 170 | ✅ `events.player-join` |
| 22 | `o.b.command.Command` | 43 | 244 | ◐ commands.register (string usages; Bukkit commands take raw string arguments) |
| 23 | `o.b.configuration.file.YamlConfigurationOptions` | 43 | 106 | ✅ `java-side: YAML configuration, files in the plugin data folder (wasi:filesystem data-folder for native plugins)` |
| 24 | `o.b.OfflinePlayer` | 40 | 841 | ✅ `see members` |
| 25 | `o.b.event.player.PlayerQuitEvent` | 40 | 121 | ✅ `events.player-quit` |
| 26 | `o.b.command.ConsoleCommandSender` | 40 | 115 | ✅ `types.command-sender` |
| 27 | `o.b.event.Event` | 37 | 225 | ◐ built-in events map to Rustic events; plugin-defined subclasses become events.call-custom payloads |
| 28 | `o.b.ChatColor` | 36 | 1323 | ✅ `java-side: legacy colour codes converted to types.text` |
| 29 | `o.b.inventory.PlayerInventory` | 36 | 466 | ✅ `inventories.contents` |
| 30 | `o.b.NamespacedKey` | 34 | 881 | ✅ `java-side: namespaced ids` |
| 31 | `o.b.event.HandlerList` | 34 | 385 | ✅ `events.subscribe` |
| 32 | `o.b.block.BlockState` | 32 | 521 | ❌ |
| 33 | `o.b.entity.EntityType` | 31 | 2471 | ✅ `types.entity-ref` |
| 34 | `o.b.inventory.Inventory` | 31 | 794 | ◐ the player inventory (inventories) or a plugin chest menu (menus); container blocks uncovered |
| 35 | `o.b.GameMode` | 31 | 206 | ✅ `types.game-mode` |
| 36 | `o.b.inventory.meta.ItemMeta` | 30 | 943 | ◐ types.item-stack components (SNBT data components converted java-side) |
| 37 | `net.kyori.adventure.text.Component` | 30 | 681 | ✅ `java-side: Adventure components and MiniMessage, serialized to types.text JSON` |
| 38 | `o.b.command.PluginCommand` | 30 | 128 | ✅ `see members` |
| 39 | `o.b.util.Vector` | 29 | 992 | ✅ `java-side: types.vec3` |
| 40 | `o.b.entity.LivingEntity` | 29 | 309 | ✅ `see members` |
| 41 | `o.b.entity.HumanEntity` | 29 | 267 | ✅ `see members` |
| 42 | `o.b.block.data.BlockData` | 29 | 178 | ✅ `types.block-state` |
| 43 | `io.papermc.paper.threadedregions.scheduler.ScheduledTask` | 28 | 63 | ✅ `see members` |
| 44 | `io.papermc.paper.threadedregions.scheduler.GlobalRegionScheduler` | 27 | 125 | ✅ `see members` |
| 45 | `o.b.event.Cancellable` | 27 | 51 | ✅ `see members` |
| 46 | `net.kyori.adventure.text.TextComponent` | 27 | 30 | ✅ `java-side: Adventure components and MiniMessage, serialized to types.text JSON` |
| 47 | `o.b.configuration.file.FileConfiguration` | 26 | 1241 | ✅ `java-side: YAML configuration, files in the plugin data folder (wasi:filesystem data-folder for native plugins)` |
| 48 | `o.b.block.BlockFace` | 26 | 471 | ✅ `types.block-face` |
| 49 | `o.b.plugin.ServicesManager` | 26 | 107 | ✅ `see members` |
| 50 | `net.md_5.bungee.api.chat.BaseComponent` | 26 | 44 | ✅ `java-side: BungeeCord chat components, serialized to types.text JSON` |

### Top 100 methods (by plugins using them)

| # | Name | Plugins | Sites | Rustic |
|---:|---|---:|---:|---|
| 1 | `o.b.plugin.java.JavaPlugin#<init>` | 53 | 57 | ✅ `plugin.on-enable` |
| 2 | `o.b.Bukkit#getServer` | 51 | 1069 | ✅ `java-side` |
| 3 | `o.b.plugin.PluginManager#registerEvents` | 51 | 526 | ✅ `events.subscribe` |
| 4 | `o.b.Server#getPluginManager` | 49 | 448 | ✅ `java-side: plugin manager facade` |
| 5 | `o.b.plugin.PluginDescriptionFile#getVersion` | 49 | 225 | ✅ `java-side: plugin descriptor values (server.self-meta)` |
| 6 | `o.b.Bukkit#getScheduler` | 48 | 313 | ✅ `java-side: scheduler facade` |
| 7 | `o.b.configuration.file.YamlConfiguration#loadConfiguration` | 48 | 132 | ✅ `java-side: YAML configuration, files in the plugin data folder (wasi:filesystem data-folder for native plugins)` |
| 8 | `o.b.configuration.file.YamlConfiguration#getBoolean` | 47 | 361 | ✅ `java-side: YAML configuration, files in the plugin data folder (wasi:filesystem data-folder for native plugins)` |
| 9 | `o.b.plugin.java.JavaPlugin#getDataFolder` | 47 | 253 | ✅ `java-side: plugin data folder (wasi:filesystem data-folder for native plugins)` |
| 10 | `o.b.Bukkit#getOnlinePlayers` | 47 | 230 | ✅ `players.online` |
| 11 | `o.b.plugin.Plugin#getDescription` | 47 | 210 | ✅ `server.self-meta` |
| 12 | `o.b.configuration.file.YamlConfiguration#getString` | 47 | 153 | ✅ `java-side: YAML configuration, files in the plugin data folder (wasi:filesystem data-folder for native plugins)` |
| 13 | `o.b.scheduler.BukkitScheduler#runTask` | 47 | 123 | ✅ `scheduler.run-later` |
| 14 | `o.b.entity.Player#getUniqueId` | 46 | 1116 | ✅ `types.uuid` |
| 15 | `o.b.entity.Player#getName` | 45 | 943 | ✅ `players.name` |
| 16 | `o.b.command.CommandSender#hasPermission` | 45 | 393 | ✅ `permissions.has` |
| 17 | `o.b.configuration.file.YamlConfiguration#save` | 45 | 103 | ✅ `java-side: YAML configuration, files in the plugin data folder (wasi:filesystem data-folder for native plugins)` |
| 18 | `o.b.Bukkit#getPluginManager` | 44 | 563 | ✅ `java-side: plugin manager facade` |
| 19 | `o.b.plugin.Plugin#getLogger` | 44 | 555 | ✅ `logging.log` |
| 20 | `o.b.plugin.PluginManager#getPlugin` | 44 | 180 | ◐ server.plugins (other bridged plugins are Java objects in the same JVM; native ones are metadata only) |
| 21 | `o.b.Bukkit#getVersion` | 44 | 80 | ✅ `server.version` |
| 22 | `o.b.configuration.file.YamlConfiguration#isSet` | 44 | 59 | ✅ `java-side: YAML configuration, files in the plugin data folder (wasi:filesystem data-folder for native plugins)` |
| 23 | `o.b.Bukkit#getOnlineMode` | 44 | 55 | ❌ |
| 24 | `o.b.command.CommandSender#sendMessage` | 43 | 1045 | ✅ `commands.reply` |
| 25 | `o.b.World#getName` | 43 | 627 | ◐ worlds.all (dimension id, not the folder name) |
| 26 | `o.b.entity.Player#getLocation` | 43 | 566 | ✅ `players.get-location` |
| 27 | `o.b.plugin.java.JavaPlugin#getServer` | 43 | 541 | ✅ `java-side: server facade` |
| 28 | `o.b.plugin.Plugin#getDataFolder` | 43 | 120 | ✅ `java-side: plugin data folder (wasi:filesystem data-folder for native plugins)` |
| 29 | `o.b.plugin.Plugin#isEnabled` | 43 | 96 | ✅ `server.self-meta` |
| 30 | `o.b.configuration.file.YamlConfiguration#options` | 43 | 54 | ✅ `java-side: YAML configuration, files in the plugin data folder (wasi:filesystem data-folder for native plugins)` |
| 31 | `o.b.Location#getWorld` | 42 | 1088 | ✅ `java-side: world id of types.location` |
| 32 | `o.b.entity.Player#getWorld` | 42 | 440 | ✅ `players.get-location` |
| 33 | `o.b.configuration.file.YamlConfiguration#addDefault` | 42 | 225 | ✅ `java-side: YAML configuration, files in the plugin data folder (wasi:filesystem data-folder for native plugins)` |
| 34 | `o.b.event.player.PlayerJoinEvent#getPlayer` | 42 | 147 | ✅ `java-side: player field of the event record` |
| 35 | `o.b.Bukkit#getName` | 42 | 66 | ✅ `server.version` |
| 36 | `o.b.configuration.file.YamlConfigurationOptions#copyDefaults` | 42 | 49 | ✅ `java-side: YAML configuration, files in the plugin data folder (wasi:filesystem data-folder for native plugins)` |
| 37 | `o.b.configuration.file.YamlConfigurationOptions#header` | 42 | 46 | ✅ `java-side: YAML configuration, files in the plugin data folder (wasi:filesystem data-folder for native plugins)` |
| 38 | `o.b.entity.Player#hasPermission` | 39 | 210 | ✅ `permissions.has` |
| 39 | `o.b.inventory.ItemStack#<init>` | 38 | 372 | ✅ `types.item-stack` |
| 40 | `o.b.event.player.PlayerQuitEvent#getPlayer` | 38 | 103 | ✅ `java-side: player field of the event record` |
| 41 | `o.b.Location#getX` | 37 | 638 | ✅ `java-side: types.location value maths` |
| 42 | `o.b.Location#getZ` | 37 | 630 | ✅ `java-side: types.location value maths` |
| 43 | `o.b.Location#getY` | 37 | 602 | ✅ `java-side: types.location value maths` |
| 44 | `o.b.plugin.java.JavaPlugin#getLogger` | 36 | 550 | ✅ `logging.log` |
| 45 | `o.b.entity.Player#getInventory` | 35 | 308 | ✅ `inventories.contents` |
| 46 | `o.b.Location#<init>` | 35 | 297 | ✅ `java-side: types.location value maths` |
| 47 | `o.b.plugin.Plugin#getName` | 35 | 250 | ✅ `server.self-meta` |
| 48 | `o.b.inventory.ItemStack#getType` | 34 | 948 | ✅ `types.item-stack` |
| 49 | `o.b.plugin.java.JavaPlugin#getDescription` | 34 | 167 | ✅ `server.self-meta` |
| 50 | `o.b.Bukkit#getPlayer` | 34 | 135 | ✅ `players.by-name` |
| 51 | `o.b.OfflinePlayer#getUniqueId` | 33 | 243 | ✅ `types.uuid` |
| 52 | `o.b.Server#getOnlinePlayers` | 33 | 111 | ✅ `players.online` |
| 53 | `o.b.Location#getBlockX` | 32 | 518 | ✅ `java-side: types.location value maths` |
| 54 | `o.b.Location#getBlockZ` | 32 | 514 | ✅ `java-side: types.location value maths` |
| 55 | `o.b.block.Block#getLocation` | 32 | 458 | ✅ `java-side: block position` |
| 56 | `o.b.Location#getYaw` | 32 | 393 | ✅ `java-side: types.location value maths` |
| 57 | `o.b.OfflinePlayer#getName` | 32 | 341 | ✅ `players.name` |
| 58 | `o.b.entity.Player#sendMessage` | 31 | 427 | ✅ `players.send-message` |
| 59 | `o.b.Location#getPitch` | 31 | 284 | ✅ `java-side: types.location value maths` |
| 60 | `o.b.entity.Entity#getLocation` | 30 | 172 | ✅ `worlds.entity-location` |
| 61 | `o.b.Bukkit#getWorlds` | 30 | 111 | ✅ `worlds.all` |
| 62 | `o.b.inventory.ItemStack#getItemMeta` | 29 | 555 | ◐ types.item-stack components (SNBT data components converted java-side) |
| 63 | `o.b.block.Block#getType` | 29 | 470 | ✅ `worlds.get-block` |
| 64 | `o.b.plugin.PluginDescriptionFile#getName` | 29 | 130 | ✅ `java-side: plugin descriptor values (server.self-meta)` |
| 65 | `o.b.Server#getScheduler` | 29 | 120 | ✅ `java-side: scheduler facade` |
| 66 | `o.b.scheduler.BukkitScheduler#runTaskAsynchronously` | 29 | 39 | ✅ `scheduler.run-async` |
| 67 | `o.b.plugin.Plugin#getServer` | 28 | 448 | ✅ `java-side: server facade` |
| 68 | `o.b.plugin.PluginManager#callEvent` | 28 | 293 | ◐ events.call-custom (custom events only; firing built-in events is not supported) |
| 69 | `o.b.Bukkit#getConsoleSender` | 28 | 161 | ✅ `types.command-sender` |
| 70 | `o.b.Bukkit#getOfflinePlayer` | 28 | 112 | ◐ server.offline-player (name to UUID); offline data beyond name, UUID and op is not exposed |
| 71 | `o.b.Server#getClass` | 28 | 73 | ✅ `java-side: java.lang.Object/Enum` |
| 72 | `o.b.inventory.ItemStack#setItemMeta` | 27 | 406 | ◐ types.item-stack components (SNBT data components converted java-side) |
| 73 | `o.b.block.Block#getY` | 27 | 134 | ✅ `java-side: block position` |
| 74 | `o.b.inventory.ItemStack#getAmount` | 26 | 1403 | ✅ `types.item-stack` |
| 75 | `o.b.Location#getBlockY` | 26 | 434 | ✅ `java-side: types.location value maths` |
| 76 | `o.b.command.CommandSender#getName` | 26 | 316 | ✅ `players.name` |
| 77 | `o.b.Material#name` | 26 | 234 | ✅ `java-side: java.lang.Object/Enum` |
| 78 | `o.b.Location#add` | 26 | 195 | ✅ `java-side: types.location value maths` |
| 79 | `o.b.entity.Player#isOnline` | 26 | 138 | ✅ `players.is-online` |
| 80 | `o.b.Bukkit#getWorld` | 26 | 125 | ◐ worlds.all (dimension ids such as minecraft:overworld, not folder names; lookup by UID uncovered) |
| 81 | `o.b.entity.Player#getGameMode` | 26 | 122 | ✅ `players.get-game-mode` |
| 82 | `o.b.scheduler.BukkitScheduler#runTaskLater` | 26 | 55 | ✅ `scheduler.run-later` |
| 83 | `o.b.Server#getBukkitVersion` | 26 | 54 | ✅ `server.version` |
| 84 | `o.b.block.Block#getState` | 25 | 374 | ❌ no block entity API (signs, chests, spawners) |
| 85 | `o.b.event.HandlerList#<init>` | 25 | 208 | ✅ `java-side` |
| 86 | `o.b.plugin.PluginManager#isPluginEnabled` | 25 | 79 | ✅ `server.plugins` |
| 87 | `o.b.plugin.PluginManager#disablePlugin` | 25 | 44 | ◐ an error from plugin.on-enable disables the plugin itself; disabling others is uncovered |
| 88 | `o.b.util.Vector#<init>` | 24 | 157 | ✅ `java-side: vector maths` |
| 89 | `o.b.event.Event#<init>` | 24 | 155 | ◐ events.call-custom (custom events) |
| 90 | `o.b.NamespacedKey#<init>` | 24 | 135 | ✅ `java-side: namespaced ids` |
| 91 | `o.b.plugin.java.JavaPlugin#getCommand` | 24 | 65 | ✅ `commands.register` |
| 92 | `o.b.plugin.PluginManager#getPlugins` | 24 | 48 | ✅ `server.plugins` |
| 93 | `o.b.Location#getBlock` | 23 | 313 | ✅ `worlds.get-block` |
| 94 | `o.b.block.Block#getBlockData` | 23 | 268 | ✅ `worlds.get-block` |
| 95 | `o.b.NamespacedKey#getKey` | 23 | 203 | ✅ `java-side: namespaced ids` |
| 96 | `o.b.block.Block#getX` | 23 | 129 | ✅ `java-side: block position` |
| 97 | `o.b.block.Block#getZ` | 23 | 129 | ✅ `java-side: block position` |
| 98 | `o.b.event.player.PlayerInteractEvent#getPlayer` | 23 | 105 | ✅ `java-side: player field of the event record` |
| 99 | `o.b.event.inventory.InventoryClickEvent#getWhoClicked` | 23 | 69 | ✅ `events.inventory-click` |
| 100 | `o.b.scheduler.BukkitTask#cancel` | 23 | 56 | ✅ `scheduler.cancel` |

### Top 50 methods by call sites

| # | Name | Plugins | Sites | Rustic |
|---:|---|---:|---:|---|
| 1 | `o.b.inventory.ItemStack#getAmount` | 26 | 1403 | ✅ `types.item-stack` |
| 2 | `o.b.entity.Player#getUniqueId` | 46 | 1116 | ✅ `types.uuid` |
| 3 | `o.b.Location#getWorld` | 42 | 1088 | ✅ `java-side: world id of types.location` |
| 4 | `o.b.Bukkit#getServer` | 51 | 1069 | ✅ `java-side` |
| 5 | `o.b.command.CommandSender#sendMessage` | 43 | 1045 | ✅ `commands.reply` |
| 6 | `o.b.inventory.ItemStack#getType` | 34 | 948 | ✅ `types.item-stack` |
| 7 | `o.b.entity.Player#getName` | 45 | 943 | ✅ `players.name` |
| 8 | `o.b.Location#getX` | 37 | 638 | ✅ `java-side: types.location value maths` |
| 9 | `o.b.Location#getZ` | 37 | 630 | ✅ `java-side: types.location value maths` |
| 10 | `o.b.World#getName` | 43 | 627 | ◐ worlds.all (dimension id, not the folder name) |
| 11 | `o.b.Location#getY` | 37 | 602 | ✅ `java-side: types.location value maths` |
| 12 | `o.b.entity.Player#getLocation` | 43 | 566 | ✅ `players.get-location` |
| 13 | `o.b.Bukkit#getPluginManager` | 44 | 563 | ✅ `java-side: plugin manager facade` |
| 14 | `o.b.plugin.Plugin#getLogger` | 44 | 555 | ✅ `logging.log` |
| 15 | `o.b.inventory.ItemStack#getItemMeta` | 29 | 555 | ◐ types.item-stack components (SNBT data components converted java-side) |
| 16 | `o.b.inventory.ItemStack#setAmount` | 20 | 555 | ✅ `types.item-stack` |
| 17 | `o.b.plugin.java.JavaPlugin#getLogger` | 36 | 550 | ✅ `logging.log` |
| 18 | `o.b.plugin.java.JavaPlugin#getServer` | 43 | 541 | ✅ `java-side: server facade` |
| 19 | `o.b.plugin.PluginManager#registerEvents` | 51 | 526 | ✅ `events.subscribe` |
| 20 | `o.b.Location#getBlockX` | 32 | 518 | ✅ `java-side: types.location value maths` |
| 21 | `o.b.Location#getBlockZ` | 32 | 514 | ✅ `java-side: types.location value maths` |
| 22 | `o.b.block.Block#getType` | 29 | 470 | ✅ `worlds.get-block` |
| 23 | `o.b.block.Block#getLocation` | 32 | 458 | ✅ `java-side: block position` |
| 24 | `o.b.Server#getPluginManager` | 49 | 448 | ✅ `java-side: plugin manager facade` |
| 25 | `o.b.plugin.Plugin#getServer` | 28 | 448 | ✅ `java-side: server facade` |
| 26 | `o.b.entity.Player#getWorld` | 42 | 440 | ✅ `players.get-location` |
| 27 | `o.b.Location#getBlockY` | 26 | 434 | ✅ `java-side: types.location value maths` |
| 28 | `o.b.entity.Player#sendMessage` | 31 | 427 | ✅ `players.send-message` |
| 29 | `net.kyori.adventure.text.Component#text` | 17 | 416 | ✅ `java-side: Adventure components and MiniMessage, serialized to types.text JSON` |
| 30 | `o.b.inventory.ItemStack#setItemMeta` | 27 | 406 | ◐ types.item-stack components (SNBT data components converted java-side) |
| 31 | `o.b.command.CommandSender#hasPermission` | 45 | 393 | ✅ `permissions.has` |
| 32 | `o.b.Location#getYaw` | 32 | 393 | ✅ `java-side: types.location value maths` |
| 33 | `o.b.block.Block#getState` | 25 | 374 | ❌ no block entity API (signs, chests, spawners) |
| 34 | `o.b.inventory.ItemStack#<init>` | 38 | 372 | ✅ `types.item-stack` |
| 35 | `o.b.configuration.file.YamlConfiguration#getBoolean` | 47 | 361 | ✅ `java-side: YAML configuration, files in the plugin data folder (wasi:filesystem data-folder for native plugins)` |
| 36 | `o.b.configuration.ConfigurationSection#getString` | 15 | 349 | ✅ `java-side: YAML configuration, files in the plugin data folder (wasi:filesystem data-folder for native plugins)` |
| 37 | `o.b.OfflinePlayer#getName` | 32 | 341 | ✅ `players.name` |
| 38 | `o.b.Material#ordinal` | 14 | 327 | ✅ `java-side: java.lang.Object/Enum` |
| 39 | `o.b.command.CommandSender#getName` | 26 | 316 | ✅ `players.name` |
| 40 | `o.b.Bukkit#getScheduler` | 48 | 313 | ✅ `java-side: scheduler facade` |
| 41 | `o.b.Location#getBlock` | 23 | 313 | ✅ `worlds.get-block` |
| 42 | `o.b.entity.Player#getInventory` | 35 | 308 | ✅ `inventories.contents` |
| 43 | `o.b.configuration.file.FileConfiguration#set` | 16 | 303 | ✅ `java-side: YAML configuration, files in the plugin data folder (wasi:filesystem data-folder for native plugins)` |
| 44 | `o.b.configuration.file.YamlConfiguration#set` | 16 | 300 | ✅ `java-side: YAML configuration, files in the plugin data folder (wasi:filesystem data-folder for native plugins)` |
| 45 | `o.b.Location#<init>` | 35 | 297 | ✅ `java-side: types.location value maths` |
| 46 | `o.b.configuration.ConfigurationSection#set` | 10 | 295 | ✅ `java-side: YAML configuration, files in the plugin data folder (wasi:filesystem data-folder for native plugins)` |
| 47 | `o.b.plugin.PluginManager#callEvent` | 28 | 293 | ◐ events.call-custom (custom events only; firing built-in events is not supported) |
| 48 | `o.b.Location#getPitch` | 31 | 284 | ✅ `java-side: types.location value maths` |
| 49 | `o.b.configuration.ConfigurationSection#getBoolean` | 13 | 276 | ✅ `java-side: YAML configuration, files in the plugin data folder (wasi:filesystem data-folder for native plugins)` |
| 50 | `o.b.inventory.ItemStack#clone` | 19 | 271 | ✅ `java-side` |

### Top 100 uncovered members (by plugins using them)

| # | Name | Plugins | Sites |
|---:|---|---:|---:|
| 1 | `o.b.Bukkit#getOnlineMode` | 44 | 55 |
| 2 | `o.b.block.Block#getState` | 25 | 374 |
| 3 | `o.b.event.server.PluginDisableEvent#getPlugin` | 23 | 37 |
| 4 | `o.b.Registry#get` | 21 | 85 |
| 5 | `o.b.Material#isItem` | 19 | 72 |
| 6 | `o.b.Bukkit#getUnsafe` | 18 | 24 |
| 7 | `o.b.World#getMinHeight` | 17 | 83 |
| 8 | `o.b.Material#isBlock` | 16 | 79 |
| 9 | `o.b.World#getMaxHeight` | 16 | 46 |
| 10 | `o.b.command.BlockCommandSender#getBlock` | 16 | 41 |
| 11 | `o.b.entity.Player#isSneaking` | 14 | 94 |
| 12 | `io.papermc.paper.command.brigadier.CommandSourceStack#getSender` | 14 | 76 |
| 13 | `o.b.entity.Player#canSee` | 14 | 54 |
| 14 | `o.b.boss.BossBar#setStyle` | 14 | 14 |
| 15 | `o.b.event.entity.EntityExplodeEvent#blockList` | 13 | 32 |
| 16 | `io.papermc.paper.plugin.lifecycle.event.LifecycleEventManager#registerEventHandler` | 13 | 20 |
| 17 | `io.papermc.paper.plugin.lifecycle.event.types.LifecycleEvents#COMMANDS` | 13 | 20 |
| 18 | `o.b.boss.BossBar#addFlag` | 13 | 15 |
| 19 | `o.b.event.block.BlockPistonExtendEvent#getBlocks` | 13 | 15 |
| 20 | `io.papermc.paper.plugin.lifecycle.event.registrar.ReloadableRegistrarEvent#registrar` | 13 | 14 |
| 21 | `o.b.boss.BossBar#removeFlag` | 13 | 14 |
| 22 | `o.b.World#getUID` | 12 | 34 |
| 23 | `o.b.command.PluginIdentifiableCommand#getPlugin` | 12 | 30 |
| 24 | `o.b.potion.PotionEffect#<init>` | 12 | 21 |
| 25 | `o.b.event.server.PluginEnableEvent#getPlugin` | 12 | 20 |
| 26 | `o.b.World#getEntities` | 12 | 18 |
| 27 | `o.b.event.world.WorldLoadEvent#getWorld` | 12 | 15 |
| 28 | `o.b.entity.Entity#setCustomName` | 12 | 13 |
| 29 | `o.b.inventory.ItemStack#getMaxStackSize` | 11 | 102 |
| 30 | `o.b.potion.PotionEffect#getType` | 11 | 40 |
| 31 | `o.b.entity.Player#stopSound` | 11 | 27 |
| 32 | `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#getBuffer` | 11 | 26 |
| 33 | `o.b.event.entity.EntityChangeBlockEvent#getBlock` | 11 | 26 |
| 34 | `o.b.Material#getMaxStackSize` | 11 | 25 |
| 35 | `o.b.OfflinePlayer#hasPlayedBefore` | 11 | 17 |
| 36 | `o.b.boss.BarFlag#CREATE_FOG` | 11 | 17 |
| 37 | `o.b.boss.BarFlag#DARKEN_SKY` | 11 | 17 |
| 38 | `o.b.boss.BarFlag#PLAY_BOSS_MUSIC` | 11 | 17 |
| 39 | `o.b.World#getWorldFolder` | 11 | 16 |
| 40 | `o.b.Material#isSolid` | 11 | 15 |
| 41 | `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#setHandled` | 11 | 14 |
| 42 | `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#getSender` | 11 | 12 |
| 43 | `o.b.Tag#getValues` | 10 | 95 |
| 44 | `o.b.entity.Item#getItemStack` | 10 | 56 |
| 45 | `o.b.Registry#stream` | 10 | 50 |
| 46 | `o.b.block.Block#breakNaturally` | 10 | 39 |
| 47 | `o.b.Chunk#getEntities` | 10 | 28 |
| 48 | `o.b.event.block.Action#PHYSICAL` | 10 | 14 |
| 49 | `o.b.command.ProxiedCommandSender#getCallee` | 10 | 13 |
| 50 | `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#setCompletions` | 10 | 12 |
| 51 | `o.b.Server#getOnlineMode` | 10 | 11 |
| 52 | `o.b.entity.Player#openBook` | 10 | 10 |
| 53 | `o.b.event.block.BlockPistonRetractEvent#getBlocks` | 10 | 10 |
| 54 | `o.b.block.BlockState#getType` | 9 | 79 |
| 55 | `o.b.block.BlockState#getBlockData` | 9 | 70 |
| 56 | `o.b.block.banner.Pattern#<init>` | 9 | 62 |
| 57 | `o.b.block.BlockState#getY` | 9 | 36 |
| 58 | `o.b.World#getWorldBorder` | 9 | 35 |
| 59 | `o.b.entity.Player#getVehicle` | 9 | 23 |
| 60 | `com.destroystokyo.paper.profile.PlayerProfile#getId` | 9 | 20 |
| 61 | `o.b.FireworkEffect$Builder#withColor` | 9 | 19 |
| 62 | `o.b.event.block.BlockExplodeEvent#blockList` | 9 | 18 |
| 63 | `com.destroystokyo.paper.profile.PlayerProfile#getName` | 9 | 15 |
| 64 | `o.b.FireworkEffect$Builder#build` | 9 | 15 |
| 65 | `io.papermc.paper.command.brigadier.CommandSourceStack#getExecutor` | 9 | 14 |
| 66 | `o.b.Bukkit#getItemFactory` | 9 | 13 |
| 67 | `o.b.FireworkEffect#builder` | 9 | 13 |
| 68 | `o.b.World#getGameRuleValue` | 9 | 13 |
| 69 | `o.b.event.world.WorldUnloadEvent#getWorld` | 9 | 11 |
| 70 | `o.b.UnsafeValues#getDataVersion` | 9 | 9 |
| 71 | `o.b.Tag#isTagged` | 8 | 103 |
| 72 | `o.b.inventory.ItemStack#setData` | 8 | 72 |
| 73 | `o.b.entity.Projectile#getShooter` | 8 | 44 |
| 74 | `o.b.block.BlockState#update` | 8 | 43 |
| 75 | `o.b.block.BlockState#getLocation` | 8 | 41 |
| 76 | `io.papermc.paper.registry.RegistryAccess#getRegistry` | 8 | 40 |
| 77 | `o.b.inventory.meta.BlockStateMeta#setBlockState` | 8 | 37 |
| 78 | `o.b.block.BlockState#getX` | 8 | 33 |
| 79 | `o.b.block.BlockState#getZ` | 8 | 33 |
| 80 | `o.b.event.block.SignChangeEvent#getPlayer` | 8 | 33 |
| 81 | `o.b.event.player.PlayerDropItemEvent#getPlayer` | 8 | 32 |
| 82 | `o.b.Registry#MATERIAL` | 8 | 31 |
| 83 | `io.papermc.paper.registry.RegistryAccess#registryAccess` | 8 | 29 |
| 84 | `o.b.entity.Tameable#isTamed` | 8 | 21 |
| 85 | `o.b.Material#getId` | 8 | 20 |
| 86 | `o.b.World#getLoadedChunks` | 8 | 16 |
| 87 | `o.b.attribute.AttributeInstance#setBaseValue` | 8 | 16 |
| 88 | `o.b.event.block.BlockBurnEvent#getBlock` | 8 | 16 |
| 89 | `o.b.FireworkEffect$Builder#withFade` | 8 | 13 |
| 90 | `o.b.FireworkEffect$Builder#flicker` | 8 | 11 |
| 91 | `o.b.FireworkEffect$Builder#with` | 8 | 11 |
| 92 | `o.b.FireworkEffect$Builder#trail` | 8 | 10 |
| 93 | `o.b.Registry#ATTRIBUTE` | 8 | 10 |
| 94 | `com.destroystokyo.paper.Title#builder` | 8 | 8 |
| 95 | `com.destroystokyo.paper.Title$Builder#build` | 8 | 8 |
| 96 | `com.destroystokyo.paper.Title$Builder#fadeIn` | 8 | 8 |
| 97 | `com.destroystokyo.paper.Title$Builder#fadeOut` | 8 | 8 |
| 98 | `com.destroystokyo.paper.Title$Builder#stay` | 8 | 8 |
| 99 | `com.destroystokyo.paper.Title$Builder#subtitle` | 8 | 8 |
| 100 | `com.destroystokyo.paper.Title$Builder#title` | 8 | 8 |

### Uncovered members grouped by class

| # | Name | Plugins | Sites |
|---:|---|---:|---:|
| 1 | `o.b.Bukkit` | 50 | 458 |
| 2 | `o.b.entity.Player` | 44 | 855 |
| 3 | `o.b.World` | 33 | 550 |
| 4 | `o.b.Material` | 30 | 268 |
| 5 | `o.b.block.Block` | 29 | 546 |
| 6 | `o.b.Server` | 28 | 135 |
| 7 | `o.b.Registry` | 24 | 334 |
| 8 | `o.b.entity.Entity` | 23 | 162 |
| 9 | `o.b.event.server.PluginDisableEvent` | 23 | 37 |
| 10 | `o.b.potion.PotionEffectType` | 20 | 162 |
| 11 | `o.b.block.BlockState` | 19 | 521 |
| 12 | `o.b.UnsafeValues` | 18 | 27 |
| 13 | `o.b.inventory.ItemStack` | 17 | 334 |
| 14 | `o.b.command.BlockCommandSender` | 17 | 59 |
| 15 | `o.b.Chunk` | 17 | 58 |
| 16 | `o.b.potion.PotionEffect` | 16 | 106 |
| 17 | `o.b.OfflinePlayer` | 15 | 112 |
| 18 | `io.papermc.paper.command.brigadier.CommandSourceStack` | 14 | 94 |
| 19 | `o.b.boss.BossBar` | 14 | 47 |
| 20 | `o.b.event.block.BlockPistonExtendEvent` | 14 | 43 |
| 21 | `o.b.event.entity.EntityExplodeEvent` | 13 | 89 |
| 22 | `o.b.boss.BarFlag` | 13 | 69 |
| 23 | `io.papermc.paper.command.brigadier.Commands` | 13 | 64 |
| 24 | `io.papermc.paper.plugin.lifecycle.event.LifecycleEventManager` | 13 | 20 |
| 25 | `io.papermc.paper.plugin.lifecycle.event.types.LifecycleEvents` | 13 | 20 |
| 26 | `io.papermc.paper.plugin.lifecycle.event.registrar.ReloadableRegistrarEvent` | 13 | 14 |
| 27 | `o.b.Tag` | 12 | 463 |
| 28 | `o.b.entity.Item` | 12 | 147 |
| 29 | `com.destroystokyo.paper.profile.PlayerProfile` | 12 | 61 |
| 30 | `o.b.attribute.AttributeInstance` | 12 | 50 |
| 31 | `o.b.event.block.BlockPistonRetractEvent` | 12 | 33 |
| 32 | `o.b.command.PluginIdentifiableCommand` | 12 | 30 |
| 33 | `net.milkbowl.vault.permission.Permission` | 12 | 28 |
| 34 | `o.b.event.server.PluginEnableEvent` | 12 | 20 |
| 35 | `o.b.plugin.ServicesManager` | 12 | 16 |
| 36 | `o.b.event.world.WorldLoadEvent` | 12 | 15 |
| 37 | `o.b.event.inventory.InventoryType` | 11 | 208 |
| 38 | `o.b.event.entity.EntityChangeBlockEvent` | 11 | 108 |
| 39 | `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent` | 11 | 77 |
| 40 | `o.b.event.player.PlayerDropItemEvent` | 11 | 71 |
| 41 | `o.b.plugin.PluginManager` | 11 | 30 |
| 42 | `o.b.block.banner.Pattern` | 10 | 76 |
| 43 | `o.b.event.player.PlayerTeleportEvent$TeleportCause` | 10 | 76 |
| 44 | `o.b.entity.Projectile` | 10 | 67 |
| 45 | `o.b.event.inventory.InventoryDragEvent` | 10 | 61 |
| 46 | `o.b.entity.Tameable` | 10 | 59 |
| 47 | `o.b.event.block.BlockExplodeEvent` | 10 | 53 |
| 48 | `o.b.command.ProxiedCommandSender` | 10 | 18 |
| 49 | `o.b.event.block.Action` | 10 | 14 |
| 50 | `o.b.plugin.java.JavaPlugin` | 10 | 11 |

### Top 50 uncovered types (plugins; sites = referencing classes)

| # | Name | Plugins | Sites |
|---:|---|---:|---:|
| 1 | `o.b.block.BlockState` | 32 | 367 |
| 2 | `o.b.Registry` | 24 | 132 |
| 3 | `o.b.potion.PotionEffectType` | 24 | 110 |
| 4 | `o.b.event.server.PluginDisableEvent` | 24 | 29 |
| 5 | `o.b.entity.Item` | 20 | 85 |
| 6 | `o.b.UnsafeValues` | 20 | 28 |
| 7 | `o.b.command.BlockCommandSender` | 19 | 55 |
| 8 | `o.b.potion.PotionEffect` | 17 | 75 |
| 9 | `o.b.attribute.Attribute` | 16 | 46 |
| 10 | `io.papermc.paper.command.brigadier.CommandSourceStack` | 15 | 80 |
| 11 | `o.b.entity.Projectile` | 15 | 66 |
| 12 | `o.b.boss.BarFlag` | 15 | 25 |
| 13 | `o.b.event.player.PlayerTeleportEvent$TeleportCause` | 14 | 98 |
| 14 | `o.b.attribute.AttributeInstance` | 14 | 22 |
| 15 | `o.b.event.block.BlockPistonExtendEvent` | 14 | 18 |
| 16 | `o.b.event.server.PluginEnableEvent` | 14 | 16 |
| 17 | `o.b.entity.Villager` | 13 | 34 |
| 18 | `o.b.command.PluginIdentifiableCommand` | 13 | 32 |
| 19 | `o.b.event.entity.EntityExplodeEvent` | 13 | 19 |
| 20 | `io.papermc.paper.command.brigadier.Commands` | 13 | 16 |
| 21 | `o.b.event.world.WorldLoadEvent` | 13 | 14 |
| 22 | `io.papermc.paper.plugin.lifecycle.event.LifecycleEvent` | 13 | 13 |
| 23 | `io.papermc.paper.plugin.lifecycle.event.LifecycleEventManager` | 13 | 13 |
| 24 | `io.papermc.paper.plugin.lifecycle.event.handler.LifecycleEventHandler` | 13 | 13 |
| 25 | `io.papermc.paper.plugin.lifecycle.event.registrar.Registrar` | 13 | 13 |
| 26 | `io.papermc.paper.plugin.lifecycle.event.registrar.ReloadableRegistrarEvent` | 13 | 13 |
| 27 | `io.papermc.paper.plugin.lifecycle.event.types.LifecycleEventType` | 13 | 13 |
| 28 | `io.papermc.paper.plugin.lifecycle.event.types.LifecycleEventType$Prioritizable` | 13 | 13 |
| 29 | `io.papermc.paper.plugin.lifecycle.event.types.LifecycleEvents` | 13 | 13 |
| 30 | `o.b.Tag` | 12 | 72 |
| 31 | `o.b.block.Biome` | 12 | 55 |
| 32 | `com.destroystokyo.paper.profile.PlayerProfile` | 12 | 30 |
| 33 | `o.b.event.block.BlockExplodeEvent` | 12 | 22 |
| 34 | `o.b.event.block.BlockPistonRetractEvent` | 12 | 16 |
| 35 | `o.b.event.entity.CreatureSpawnEvent` | 11 | 73 |
| 36 | `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent` | 11 | 41 |
| 37 | `o.b.potion.PotionType` | 11 | 38 |
| 38 | `o.b.event.player.PlayerDropItemEvent` | 11 | 34 |
| 39 | `o.b.entity.Tameable` | 11 | 26 |
| 40 | `o.b.entity.Wither` | 11 | 24 |
| 41 | `o.b.permissions.PermissionAttachmentInfo` | 11 | 19 |
| 42 | `o.b.event.entity.EntityChangeBlockEvent` | 11 | 17 |
| 43 | `o.b.event.block.BlockBurnEvent` | 11 | 13 |
| 44 | `o.b.event.entity.CreatureSpawnEvent$SpawnReason` | 10 | 71 |
| 45 | `o.b.Statistic` | 10 | 46 |
| 46 | `o.b.block.banner.PatternType` | 10 | 43 |
| 47 | `o.b.inventory.EntityEquipment` | 10 | 42 |
| 48 | `o.b.block.banner.Pattern` | 10 | 40 |
| 49 | `o.b.entity.Vehicle` | 10 | 32 |
| 50 | `o.b.entity.ArmorStand` | 10 | 21 |

### Most used partial mappings

| # | Name | Plugins | Sites | Rustic |
|---:|---|---:|---:|---|
| 1 | `o.b.plugin.PluginManager#getPlugin` | 44 | 180 | ◐ server.plugins (other bridged plugins are Java objects in the same JVM; native ones are metadata only) |
| 2 | `o.b.World#getName` | 43 | 627 | ◐ worlds.all (dimension id, not the folder name) |
| 3 | `o.b.inventory.ItemStack#getItemMeta` | 29 | 555 | ◐ types.item-stack components (SNBT data components converted java-side) |
| 4 | `o.b.plugin.PluginManager#callEvent` | 28 | 293 | ◐ events.call-custom (custom events only; firing built-in events is not supported) |
| 5 | `o.b.Bukkit#getOfflinePlayer` | 28 | 112 | ◐ server.offline-player (name to UUID); offline data beyond name, UUID and op is not exposed |
| 6 | `o.b.inventory.ItemStack#setItemMeta` | 27 | 406 | ◐ types.item-stack components (SNBT data components converted java-side) |
| 7 | `o.b.Bukkit#getWorld` | 26 | 125 | ◐ worlds.all (dimension ids such as minecraft:overworld, not folder names; lookup by UID uncovered) |
| 8 | `o.b.plugin.PluginManager#disablePlugin` | 25 | 44 | ◐ an error from plugin.on-enable disables the plugin itself; disabling others is uncovered |
| 9 | `o.b.event.Event#<init>` | 24 | 155 | ◐ events.call-custom (custom events) |
| 10 | `o.b.entity.Player#playSound` | 21 | 96 | ◐ players.play-sound at the player / worlds.play-sound at a location (no sound category) |
| 11 | `o.b.World#getEnvironment` | 20 | 89 | ◐ from the dimension id |
| 12 | `o.b.plugin.RegisteredServiceProvider#getProvider` | 18 | 71 | ◐ services.provider |
| 13 | `o.b.inventory.Inventory#setItem` | 17 | 202 | ◐ inventories.set-slot (player) / menus.set-item (menu) |
| 14 | `o.b.plugin.ServicesManager#getRegistration` | 17 | 49 | ◐ services.provider |
| 15 | `o.b.persistence.PersistentDataContainer#set` | 16 | 180 | ◐ players.get-data / players.set-data (players) and minecraft:custom_data (items) |
| 16 | `o.b.inventory.Inventory#getHolder` | 16 | 74 | ◐ menus (the plugin's InventoryHolder kept java-side) |
| 17 | `o.b.World$Environment#NETHER` | 16 | 71 | ◐ from the dimension id |
| 18 | `o.b.Server#getOfflinePlayer` | 16 | 63 | ◐ server.offline-player (name to UUID); offline data beyond name, UUID and op is not exposed |
| 19 | `o.b.entity.Player#openInventory` | 16 | 52 | ◐ menus.open (plugin chest menus only) |
| 20 | `o.b.Bukkit#dispatchCommand` | 16 | 39 | ◐ server.dispatch-command (console sender only; running as a player is uncovered) |
| 21 | `o.b.inventory.PlayerInventory#addItem` | 16 | 32 | ◐ inventories.give (player); java-side for menus |
| 22 | `com.mojang.brigadier.context.CommandContext#getSource` | 15 | 203 | ◐ java-side Brigadier tree, registered as commands.register usages (argument kinds limited to commands.arg-kind) |
| 23 | `o.b.inventory.InventoryView#getTopInventory` | 15 | 63 | ◐ menus |
| 24 | `o.b.World$Environment#THE_END` | 15 | 59 | ◐ from the dimension id |
| 25 | `com.mojang.brigadier.builder.LiteralArgumentBuilder#build` | 15 | 35 | ◐ java-side Brigadier tree, registered as commands.register usages (argument kinds limited to commands.arg-kind) |
| 26 | `o.b.World#getChunkAt` | 15 | 30 | ◐ chunk coordinates java-side |
| 27 | `o.b.inventory.meta.ItemMeta#setDisplayName` | 14 | 52 | ◐ types.item-stack components (SNBT data components converted java-side) |
| 28 | `o.b.inventory.Inventory#getType` | 14 | 48 | ◐ CHEST for menus, PLAYER for the player inventory |
| 29 | `com.mojang.brigadier.arguments.StringArgumentType#greedyString` | 14 | 38 | ◐ java-side Brigadier tree, registered as commands.register usages (argument kinds limited to commands.arg-kind) |
| 30 | `o.b.inventory.meta.ItemMeta#setLore` | 14 | 36 | ◐ types.item-stack components (SNBT data components converted java-side) |

### Coverage curve

If Rustic covered the K most needed missing members/types, this many of the 12 yellow plugins would turn green (ranking over yellow plugins), and this many of all 55 analyzed plugins would have every API member they use covered (ranking over all plugins; red ones stay red):

| K | Yellow → green | API-complete (all plugins) |
|---:|---:|---:|
| 10 | 1 | 2 |
| 25 | 1 | 2 |
| 50 | 2 | 2 |
| 100 | 2 | 3 |
| 200 | 2 | 3 |
| 300 | 2 | 3 |
| 500 | 2 | 3 |
| 1000 | 12 | 3 |
| 2000 | 12 | 5 |

## What Rustic's API should cover next

Derived from the rankings above (plugins using a missing member or type first, call sites
second), grouped into WIT-sized pieces. Numbers are plugins of the 55. Each item names the
blocking Bukkit/Paper members so the coverage table can be updated when the WIT grows.

### Tier 1: cheap getters and setters (one WIT function each, high reach)

1. **Server facts**: `server.online-mode` (`Bukkit/Server#getOnlineMode`: 44 + 10, almost every
   plugin through bStats), a data/protocol version getter (`Bukkit#getUnsafe` +
   `UnsafeValues#getDataVersion`: 18), `isStopping` (4). Alone, `getOnlineMode` is the only gap of
   ChestSort.
2. **World geometry and identity**: `worlds.min-height` / `max-height` (`World#getMinHeight` 17,
   `getMaxHeight` 16), a stable world id/UUID (`World#getUID` 12). Folder access
   (`World#getWorldFolder`, 11) stays out of the sandbox by design.
3. **Player state**: `isSneaking` (14), visibility `canSee`/`hidePlayer`/`showPlayer`/`hideEntity`
   (14), flight getters `isFlying`/`getAllowFlight` (5 to 6), `stopSound` (11), `openBook` (10),
   held-slot setter (`PlayerInventory#setHeldItemSlot`).
4. **Offline player data** from the user cache: `hasPlayedBefore` (11 + 6, the only gap of
   SetHome), first/last played (5 to 6), ban state (6).
5. **Boss bars**: `bossbars.set-style` (`BossBar#setStyle` 14) and flags (`addFlag`/`removeFlag`,
   `BarFlag`: 13).

Tier 1 alone turns ChestSort and SetHome green and removes the most frequent gap from 44
plugins.

### Tier 2: events (38 of the 329 event classes referenced are covered, base classes included)

In order of reach: `PluginEnableEvent`/`PluginDisableEvent` (14 / 24: soft-dependency
integrations hook them), `WorldLoadEvent`/`WorldUnloadEvent` (13 / 10), `EntityExplodeEvent`
and `BlockExplodeEvent` (13 / 12), `BlockPistonExtendEvent`/`RetractEvent` (14 / 12),
`BlockBurnEvent` (11), `EntityChangeBlockEvent` (11), `BlockFromToEvent` (10), `BlockFadeEvent`
(9), `PlayerDropItemEvent` (11), `EntityPickupItemEvent` (8), `CreatureSpawnEvent` (11, with
spawn reasons), `InventoryDragEvent` (10), `SignChangeEvent` (9), `AsyncTabCompleteEvent` (11),
`ServerLoadEvent` (8). Also enrich existing records: `player-teleport` cause and cancellation
(`TeleportCause` type in 14 plugins), `player-interact` physical action (`Action.PHYSICAL`, 10).
Protection and logging plugins (GriefPrevention, WorldGuard, CoreProtect) need the block-change
family (explode, piston, burn, fade, flow, change-block) as a group.

### Tier 3: data access (larger interfaces)

1. **Registries and game data**: a read-only `registries` interface for item/block properties and
   tags (`Material#isItem` 19, `isBlock` 16, `isSolid` 11, `getMaxStackSize` 11 + 11,
   `Tag#getValues`/`isTagged` 10 / 8) and registry listing/lookup (`Registry#get` 21,
   `Registry#stream` 10, Paper `RegistryAccess` 8). Many `Material` gaps disappear with it.
2. **Block entities** (`Block#getState` 25 plugins and 374 sites, `BlockState` type in 32):
   `worlds.get-block-entity` / `set-block-entity` as SNBT (signs, containers, spawners, banners;
   `BlockStateMeta` 8).
3. **Potion effects** (`PotionEffect`/`PotionEffectType` 16 to 24) and **attributes**
   (`AttributeInstance` 12 to 14): `players.add-effect/remove-effect/effects`,
   `attribute`/`set-attribute-base` for living entities.
4. **Entities beyond references**: custom name (`Entity#setCustomName` 12), passengers/vehicle
   (6 to 9), velocity (10), dropped item stack (`Item#getItemStack` 10), entity listing per world or
   chunk (`World#getEntities` 12, `Chunk#getEntities` 10), `isDead` (7), and teleporting
   non-player entities (`Entity#teleport` is partial today).
5. **Persistent data** on entities, chunks and blocks (`getPersistentDataContainer` in 19 plugins;
   players and items are covered, the rest is not).

### Tier 4: commands

Paper's Brigadier lifecycle (`LifecycleEvents.COMMANDS`, `Commands`, `CommandSourceStack`: 13
to 15 plugins) mapped onto `commands.register` (argument kinds are close to `commands.arg-kind`);
command-sender variants for command blocks and `execute as` (`BlockCommandSender` 16 to 19,
`ProxiedCommandSender` 10); running a command as a player (`Player#performCommand`, 3).

### Out of reach by design

Netty pipeline injection (ViaVersion family, Geyser, PacketEvents, ProtocolLib, Grim, voice
chat), NMS-level entities and packets (holograms, NPCs, map renderers, world editors), and
bytecode instrumentation will not be bridged. The Rustic answer for the most common reasons
behind them would be a small **client-side view API** (fake/display entities, client-only block
changes: `Player#sendBlockChange` in 6 plugins) rather than packet access.

## Limitations (static analysis blind spots)

- **Reference is not reachability.** A member referenced in a dead, version-guarded or optional
  branch counts like one on the hot path. Red means "touches internals somewhere"; many red
  plugins pick an NMS adapter per server version and might run with a degraded feature set.
- **Reflection is approximate.** Detected: reflection calls in the jar plus string constants
  naming internals (`net.minecraft.`, `org.bukkit.craftbukkit`, `com.mojang.authlib`,
  `io.netty.channel`, `getHandle`, `Craft*` class names). Missed: names assembled at run time
  from other pieces, encrypted/obfuscated strings, reflection on Bukkit API members (not counted
  as API usage), and `MethodHandle` lookups by computed names.
- **Overloads and supertypes.** Member coverage is by name within a class (descriptors only where
  the table says so); a covered name may hide an uncovered overload. Inheritance comes from the
  table's `inherits` lists, not from the real API jar; a call through an interface the table does
  not list is reported against that interface.
- **Runtime-loaded code is invisible**: Paper `libraries:` and LuckPerms-style dependency
  downloaders fetch jars at run time (e.g. LuckPerms' own Adventure copy), Skript scripts and
  addon jars, plugin-provided scripting (JavaScript, Groovy).
- **Shading heuristics.** Own code is the `main` class's first three package segments; projects
  spread over several roots (EssentialsX `net.ess3`) have part of their code counted as
  "bundled library", which only changes the origin label of red signals, not the verdict.
- **Namespace heuristics.** `io.papermc.paper.*` and `com.destroystokyo.paper.*` are treated as
  API, although a few server-internal classes live there too; `org.spigotmc.*` likewise.
- **Multi-platform jars**: BungeeCord/Velocity classes are separated, but NMS referenced only
  from another platform's module would still count.
- **Panel bias**: download-ranked lists favour big, feature-rich plugins (which use more API and
  more internals) over the many small single-purpose plugins of real servers; client-mod
  companions were excluded; some well-known plugins (Citizens, Floodgate, spark) are not
  distributed as Bukkit jars through these APIs.
- **Partial mappings count as covered.** 10% of covered call sites go through a `partial`
  mapping whose gap may matter for a given plugin (see "Most used partial mappings").

## Licensing context

The Bukkit API is GPLv3 (Paper's additions too); Adventure is MIT. This analysis reads
third-party jars **locally**, for interoperability research; the jars are downloaded to
`bridge/panel/jars/`, which is gitignored and must never be committed or redistributed. Only
metadata (`bridge/panel/panel.json`: name, version, source URL, licence, SHA-256) and derived
statistics (API member names and counts) are committed. The analyzer contains no Bukkit-family
code; its coverage table lists public API names only (ADR 0008), and its test fixtures compile
against minimal stand-in classes written for the tests. The GPLv3 Java bridge layer
(`bridge/java`, ADR 0009) is a separate component.

## Reproduce

```sh
python -I bridge/panel/fetch.py --count 55   # ~270 MB into bridge/panel/jars (gitignored)
sh bridge/panel/analyze.sh                   # bridge/panel/report.md + report.json
cargo run -p rusticmc-bridge-analyze --bin rustic-bridge-analyze -- --details SomePlugin.jar
```

Newer plugin versions on the APIs change the numbers; `panel.json` records exactly which builds
(SHA-256) this report is about.
