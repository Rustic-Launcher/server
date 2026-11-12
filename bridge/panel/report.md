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

## Plugins in detail

## 🔴 AuthMe 5.7.0-FORK-b53

- file: `authmerereloadedfork--AuthMe-5.7.0-FORK-Universal.jar`
- main: `fr.xephi.authme.AuthMe`, api-version: 1.13
- softdepend: Vault, LuckPerms, PermissionsEx, bPermissions, zPermissions, Multiverse-Core, Essentials, EssentialsSpawn, ProtocolLib, floodgate, PlaceholderAPI
- classes: 10571 (441 own under `fr.xephi.authme`, 10130 bundled), reflection calls: 596
- ProtocolLib / PacketEvents: com.comphenix.protocol, com.comphenix.protocol.events, com.comphenix.protocol.reflect (31 refs, in own)
- reflection into internals: Craft* class name, getHandle(), net.minecraft. (16 refs, in fr.xephi.authme.libs.net.kyori, own)
- uses 95 uncovered API members and 37 uncovered API types (top: org.bukkit.entity.Player#setFlySpeed, org.bukkit.entity.Player#setWalkSpeed, org.bukkit.entity.Player#getFlySpeed, org.bukkit.entity.Player#getWalkSpeed, org.bukkit.Server#getWorldContainer)

| Uncovered member | Sites |
|---|---:|
| `org.bukkit.entity.Player#setFlySpeed` | 6 |
| `org.bukkit.entity.Player#setWalkSpeed` | 6 |
| `org.bukkit.entity.Player#getFlySpeed` | 4 |
| `org.bukkit.entity.Player#getWalkSpeed` | 4 |
| `org.bukkit.Server#getWorldContainer` | 3 |
| `org.bukkit.entity.Player#addPotionEffect` | 3 |
| `org.bukkit.World#getMaxHeight` | 2 |
| `org.bukkit.block.Block#breakNaturally` | 2 |
| `org.bukkit.entity.Player#getAllowFlight` | 2 |
| `org.bukkit.entity.Player#getVehicle` | 2 |
| `org.bukkit.entity.Player#leaveVehicle` | 2 |
| `org.bukkit.entity.Player#setNoDamageTicks` | 2 |
| `org.bukkit.event.entity.EntityInteractEvent#setCancelled` | 2 |
| `org.bukkit.event.player.AsyncPlayerChatEvent#getRecipients` | 2 |
| `org.bukkit.event.player.PlayerSwapHandItemsEvent#getPlayer` | 2 |
| `org.bukkit.event.player.PlayerSwapHandItemsEvent#setCancelled` | 2 |
| `org.bukkit.potion.PotionEffectType#BLINDNESS` | 2 |
| `com.destroystokyo.paper.Title#builder` | 1 |
| `com.destroystokyo.paper.Title$Builder#build` | 1 |
| `com.destroystokyo.paper.Title$Builder#fadeIn` | 1 |
| `com.destroystokyo.paper.Title$Builder#fadeOut` | 1 |
| `com.destroystokyo.paper.Title$Builder#stay` | 1 |
| `com.destroystokyo.paper.Title$Builder#subtitle` | 1 |
| `com.destroystokyo.paper.Title$Builder#title` | 1 |
| `net.milkbowl.vault.permission.Permission#hasGroupSupport` | 1 |
| `net.milkbowl.vault.permission.Permission#playerInGroup` | 1 |
| `org.bukkit.BanList#addBan` | 1 |
| `org.bukkit.BanList$Type#IP` | 1 |
| `org.bukkit.Bukkit#getBannedPlayers` | 1 |
| `org.bukkit.Bukkit#getIp` | 1 |
| `org.bukkit.Bukkit#getOfflinePlayers` | 1 |
| `org.bukkit.Bukkit#getOnlineMode` | 1 |
| `org.bukkit.Material#isBlock` | 1 |
| `org.bukkit.Material#isOccluding` | 1 |
| `org.bukkit.Server#banIP` | 1 |
| `org.bukkit.Server#getBanList` | 1 |
| `org.bukkit.Server#getIp` | 1 |
| `org.bukkit.Server#getOfflinePlayers` | 1 |
| `org.bukkit.Server#shutdown` | 1 |
| `org.bukkit.Server$Spigot#getConfig` | 1 |
| ... 55 more | |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| ProtocolLib / PacketEvents | `com.comphenix.protocol` | own | 15 | yes |
| ProtocolLib / PacketEvents | `com.comphenix.protocol.events` | own | 13 | yes |
| ProtocolLib / PacketEvents | `com.comphenix.protocol.reflect` | own | 3 | yes |
| reflection into internals | `Craft* class name` | fr.xephi.authme.libs.net.kyori | 9 | yes |
| reflection into internals | `getHandle()` | fr.xephi.authme.libs.net.kyori | 2 | yes |
| reflection into internals | `net.minecraft.` | fr.xephi.authme.libs.net.kyori | 1 | yes |
| reflection into internals | `net.minecraft.server` | fr.xephi.authme.libs.net.kyori | 1 | yes |
| reflection into internals | `org.bukkit.craftbukkit` | fr.xephi.authme.libs.net.kyori | 2 | yes |
| reflection into internals | `org.bukkit.craftbukkit` | own | 1 | yes |
| bytecode instrumentation | `java.lang.instrument` | fr.xephi.authme.libs.com.alessiodp | 3 | no |
| bytecode instrumentation | `javassist` | fr.xephi.authme.libs.com.zaxxer | 10 | no |
| bytecode instrumentation | `javassist.bytecode` | fr.xephi.authme.libs.com.zaxxer | 1 | no |
| Unsafe / jdk.internal | `sun.misc` | fr.xephi.authme.libs.com.alessiodp | 1 | no |
| Unsafe / jdk.internal | `sun.misc` | fr.xephi.authme.libs.com.github | 59 | no |
| Unsafe / jdk.internal | `sun.misc` | fr.xephi.authme.libs.com.google | 21 | no |

## 🔴 AxiomPaper 6.0.1+26.3

- file: `axiompaperplugin--AxiomPaper-6.0.1-for-MC26.3.jar`
- main: `com.moulberry.axiom.AxiomPaper`, api-version: 26.3
- softdepend: CoreProtect, ViaVersion, WorldGuard, PlotSquared
- classes: 191 (158 own under `com.moulberry.axiom`, 33 bundled), reflection calls: 5
- NMS (net.minecraft): net.minecraft, net.minecraft.commands.arguments, net.minecraft.core (602 refs, in own)
- CraftBukkit: org.bukkit.craftbukkit, org.bukkit.craftbukkit.block, org.bukkit.craftbukkit.entity (55 refs, in own)
- server internals (Mojang libs, Netty, Paper internals): com.mojang.datafixers, com.mojang.datafixers.util, com.mojang.serialization (84 refs, in own)
- uses 30 uncovered API members and 22 uncovered API types (top: io.papermc.paper.command.brigadier.CommandSourceStack#getSender, io.papermc.paper.command.brigadier.Commands#literal, org.bukkit.World#sendMessage, io.papermc.paper.event.player.PlayerFailMoveEvent#getPlayer, org.bukkit.GameRules#ADVANCE_WEATHER)

| Uncovered member | Sites |
|---|---:|
| `io.papermc.paper.command.brigadier.CommandSourceStack#getSender` | 16 |
| `io.papermc.paper.command.brigadier.Commands#literal` | 16 |
| `org.bukkit.World#sendMessage` | 4 |
| `io.papermc.paper.event.player.PlayerFailMoveEvent#getPlayer` | 3 |
| `org.bukkit.GameRules#ADVANCE_WEATHER` | 3 |
| `org.bukkit.World#getPersistentDataContainer` | 3 |
| `io.papermc.paper.command.brigadier.Commands#argument` | 2 |
| `io.papermc.paper.command.brigadier.Commands#register` | 2 |
| `io.papermc.paper.event.player.PlayerFailMoveEvent#getFailReason` | 2 |
| `io.papermc.paper.event.player.PlayerFailMoveEvent#getTo` | 2 |
| `io.papermc.paper.event.player.PlayerFailMoveEvent#setAllowed` | 2 |
| `io.papermc.paper.plugin.lifecycle.event.registrar.ReloadableRegistrarEvent#registrar` | 2 |
| `org.bukkit.entity.Display#getUniqueId` | 2 |
| `org.bukkit.event.server.PluginDisableEvent#getPlugin` | 2 |
| `io.papermc.paper.antixray.ChunkPacketBlockController#shouldModify` | 1 |
| `io.papermc.paper.event.player.PlayerFailMoveEvent$FailReason#MOVED_INTO_UNLOADED_CHUNK` | 1 |
| `io.papermc.paper.event.player.PlayerFailMoveEvent$FailReason#MOVED_TOO_QUICKLY` | 1 |
| `io.papermc.paper.event.world.WorldGameRuleChangeEvent#getGameRule` | 1 |
| `io.papermc.paper.event.world.WorldGameRuleChangeEvent#getValue` | 1 |
| `io.papermc.paper.event.world.WorldGameRuleChangeEvent#getWorld` | 1 |
| `io.papermc.paper.plugin.lifecycle.event.LifecycleEventManager#registerEventHandler` | 1 |
| `io.papermc.paper.plugin.lifecycle.event.types.LifecycleEvents#COMMANDS` | 1 |
| `org.bukkit.World#getGameRuleValue` | 1 |
| `org.bukkit.World#setGameRule` | 1 |
| `org.bukkit.entity.Player#getListeningPluginChannels` | 1 |
| `org.bukkit.entity.Player#isFlying` | 1 |
| `org.bukkit.event.block.Action#PHYSICAL` | 1 |
| `org.bukkit.event.world.GenericGameEvent#getEntity` | 1 |
| `org.bukkit.event.world.GenericGameEvent#setCancelled` | 1 |
| `org.bukkit.plugin.java.JavaPlugin#getLifecycleManager` | 1 |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| NMS (net.minecraft) | `net.minecraft` | own | 11 | yes |
| NMS (net.minecraft) | `net.minecraft.commands.arguments` | own | 4 | yes |
| NMS (net.minecraft) | `net.minecraft.core` | own | 97 | yes |
| NMS (net.minecraft) | `net.minecraft.core.component` | own | 2 | yes |
| NMS (net.minecraft) | `net.minecraft.core.registries` | own | 12 | yes |
| NMS (net.minecraft) | `net.minecraft.nbt` | own | 44 | yes |
| NMS (net.minecraft) | `net.minecraft.network` | own | 78 | yes |
| NMS (net.minecraft) | `net.minecraft.network.chat` | own | 6 | yes |
| NMS (net.minecraft) | `net.minecraft.network.codec` | own | 23 | yes |
| NMS (net.minecraft) | `net.minecraft.network.protocol` | own | 9 | yes |
| NMS (net.minecraft) | `net.minecraft.resources` | own | 25 | yes |
| NMS (net.minecraft) | `net.minecraft.server` | own | 12 | yes |
| NMS (net.minecraft) | `net.minecraft.server.level` | own | 56 | yes |
| NMS (net.minecraft) | `net.minecraft.server.network` | own | 4 | yes |
| NMS (net.minecraft) | `net.minecraft.server.players` | own | 5 | yes |
| NMS (net.minecraft) | `net.minecraft.util` | own | 7 | yes |
| NMS (net.minecraft) | `net.minecraft.util.datafix` | own | 4 | yes |
| NMS (net.minecraft) | `net.minecraft.util.valueproviders` | own | 1 | yes |
| NMS (net.minecraft) | `net.minecraft.world` | own | 1 | yes |
| NMS (net.minecraft) | `net.minecraft.world.entity` | own | 40 | yes |
| NMS (net.minecraft) | `net.minecraft.world.item` | own | 9 | yes |
| NMS (net.minecraft) | `net.minecraft.world.level` | own | 145 | yes |
| NMS (net.minecraft) | `net.minecraft.world.phys` | own | 7 | yes |
| CraftBukkit | `org.bukkit.craftbukkit` | own | 18 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.block` | own | 4 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.entity` | own | 28 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.event` | own | 1 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.inventory` | own | 2 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.util` | own | 2 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `com.mojang.datafixers` | own | 6 | yes |

## 🟡 BlueMap 5.29

- file: `bluemap--bluemap-5.29-spigot.jar`
- main: `de.bluecolored.bluemap.bukkit.BukkitPlugin`, api-version: 1.16
- classes: 2846 (640 own under `de.bluecolored.bluemap`, 2206 bundled), reflection calls: 589
- uses 14 uncovered API members and 3 uncovered API types (top: org.bukkit.World#save, org.bukkit.event.server.TabCompleteEvent#getCompletions, org.bukkit.Bukkit#getOnlineMode, org.bukkit.Material#isBlock, org.bukkit.World#getWorldFolder)

| Uncovered member | Sites |
|---|---:|
| `org.bukkit.World#save` | 2 |
| `org.bukkit.event.server.TabCompleteEvent#getCompletions` | 2 |
| `org.bukkit.Bukkit#getOnlineMode` | 1 |
| `org.bukkit.Material#isBlock` | 1 |
| `org.bukkit.World#getWorldFolder` | 1 |
| `org.bukkit.block.Block#getLightFromBlocks` | 1 |
| `org.bukkit.block.Block#getLightFromSky` | 1 |
| `org.bukkit.command.BlockCommandSender#getBlock` | 1 |
| `org.bukkit.entity.Player#hasPotionEffect` | 1 |
| `org.bukkit.entity.Player#isSneaking` | 1 |
| `org.bukkit.event.server.TabCompleteEvent#getBuffer` | 1 |
| `org.bukkit.event.server.TabCompleteEvent#getSender` | 1 |
| `org.bukkit.event.server.TabCompleteEvent#setCompletions` | 1 |
| `org.bukkit.potion.PotionEffectType#INVISIBILITY` | 1 |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| Unsafe / jdk.internal | `sun.misc` | de.bluecolored.shadow | 39 | no |

## 🟡 ChestSort 1.1

- file: `chestsort--ChestSort-1.1.jar`
- main: `com.ashkiano.chestsort.ChestSort`, api-version: 1.16
- classes: 15 (15 own under `com.ashkiano.chestsort`, 0 bundled), reflection calls: 2
- uses 1 uncovered API members and 0 uncovered API types (top: org.bukkit.Bukkit#getOnlineMode)

| Uncovered member | Sites |
|---|---:|
| `org.bukkit.Bukkit#getOnlineMode` | 1 |

## 🟡 Chunky 1.5.66

- file: `chunky--Chunky-Paper-1.5.66.jar`
- main: `org.popcraft.chunky.ChunkyPaper`, api-version: 1.21.8
- softdepend: dynmap, BlueMap, squaremap, WorldBorder
- classes: 174 (164 own under `org.popcraft.chunky`, 10 bundled), reflection calls: 9
- uses 37 uncovered API members and 22 uncovered API types (top: io.papermc.paper.command.brigadier.CommandSourceStack#getSender, org.bukkit.World#getWorldFolder, org.bukkit.Registry#SOUND_EVENT, org.bukkit.World#getLogicalHeight, org.bukkit.WorldBorder#getCenter)

| Uncovered member | Sites |
|---|---:|
| `io.papermc.paper.command.brigadier.CommandSourceStack#getSender` | 3 |
| `org.bukkit.World#getWorldFolder` | 3 |
| `org.bukkit.Registry#SOUND_EVENT` | 2 |
| `org.bukkit.World#getLogicalHeight` | 2 |
| `org.bukkit.WorldBorder#getCenter` | 2 |
| `org.bukkit.WorldBorder#getSize` | 2 |
| `io.papermc.paper.command.brigadier.CommandSourceStack#getExecutor` | 1 |
| `io.papermc.paper.command.brigadier.Commands#register` | 1 |
| `io.papermc.paper.command.brigadier.argument.ArgumentTypes#playerProfiles` | 1 |
| `io.papermc.paper.entity.TeleportFlag$EntityState#RETAIN_PASSENGERS` | 1 |
| `io.papermc.paper.entity.TeleportFlag$EntityState#RETAIN_VEHICLE` | 1 |
| `io.papermc.paper.plugin.lifecycle.event.LifecycleEventManager#registerEventHandler` | 1 |
| `io.papermc.paper.plugin.lifecycle.event.registrar.ReloadableRegistrarEvent#registrar` | 1 |
| `io.papermc.paper.plugin.lifecycle.event.types.LifecycleEvents#COMMANDS` | 1 |
| `io.papermc.paper.world.chunk.ChunkGenerationState#GENERATED` | 1 |
| `io.papermc.paper.world.chunk.ChunkGenerationState#OUTDATED` | 1 |
| `org.bukkit.Bukkit#getOnlineMode` | 1 |
| `org.bukkit.Effect#valueOf` | 1 |
| `org.bukkit.Material#isSolid` | 1 |
| `org.bukkit.Registry#get` | 1 |
| `org.bukkit.Registry#keyStream` | 1 |
| `org.bukkit.Server#getMaxWorldSize` | 1 |
| `org.bukkit.World#getChunkGenerationStateAsync` | 1 |
| `org.bukkit.World#getMinHeight` | 1 |
| `org.bukkit.World#getSeaLevel` | 1 |
| `org.bukkit.World#getUID` | 1 |
| `org.bukkit.World#getWorldBorder` | 1 |
| `org.bukkit.World#isChunkGenerated` | 1 |
| `org.bukkit.World#playEffect` | 1 |
| `org.bukkit.entity.Entity#addPassenger` | 1 |
| `org.bukkit.entity.Entity#getPassengers` | 1 |
| `org.bukkit.entity.Player#getVehicle` | 1 |
| `org.bukkit.entity.Player#hideEntity` | 1 |
| `org.bukkit.entity.Player#showEntity` | 1 |
| `org.bukkit.event.player.PlayerTeleportEvent$TeleportCause#PLUGIN` | 1 |
| `org.bukkit.event.world.WorldInitEvent#getWorld` | 1 |
| `org.bukkit.plugin.java.JavaPlugin#getLifecycleManager` | 1 |

## 🔴 CommandAPI 12.1.0

- file: `commandapi--CommandAPI-12.1.0-Paper.jar`
- main: `dev.jorel.commandapi.CommandAPIMain`, api-version: 1.20.6
- classes: 334 (334 own under `dev.jorel.commandapi`, 0 bundled), reflection calls: 22
- NMS (net.minecraft): net.minecraft.advancements, net.minecraft.advancements.critereon, net.minecraft.advancements.criterion (1167 refs, in own)
- CraftBukkit: org.bukkit.craftbukkit, org.bukkit.craftbukkit.block, org.bukkit.craftbukkit.command (164 refs, in own)
- server internals (Mojang libs, Netty, Paper internals): com.mojang.datafixers.util, com.mojang.logging, com.mojang.serialization (59 refs, in own)
- uses 111 uncovered API members and 69 uncovered API types (top: io.papermc.paper.command.brigadier.PaperCommands#INSTANCE, io.papermc.paper.command.brigadier.PaperCommands#getBuildContext, org.bukkit.Registry#get, org.bukkit.Bukkit#getHelpMap, org.bukkit.Particle$DustTransition#<init>)

| Uncovered member | Sites |
|---|---:|
| `io.papermc.paper.command.brigadier.PaperCommands#INSTANCE` | 11 |
| `io.papermc.paper.command.brigadier.PaperCommands#getBuildContext` | 11 |
| `org.bukkit.Registry#get` | 10 |
| `org.bukkit.Bukkit#getHelpMap` | 9 |
| `org.bukkit.Particle$DustTransition#<init>` | 9 |
| `org.bukkit.Registry#ENCHANTMENT` | 9 |
| `org.bukkit.Vibration#<init>` | 9 |
| `org.bukkit.Vibration$Destination$BlockDestination#<init>` | 9 |
| `org.bukkit.block.Biome#valueOf` | 9 |
| `org.bukkit.Particle$Trail#<init>` | 6 |
| `io.papermc.paper.command.brigadier.argument.ArgumentTypes#resource` | 4 |
| `io.papermc.paper.command.brigadier.argument.range.DoubleRangeProvider#range` | 4 |
| `io.papermc.paper.command.brigadier.argument.range.IntegerRangeProvider#range` | 4 |
| `org.bukkit.command.ProxiedCommandSender#getCallee` | 4 |
| `org.bukkit.plugin.InvalidPluginException#<init>` | 3 |
| `org.bukkit.plugin.InvalidPluginException#printStackTrace` | 3 |
| `com.destroystokyo.paper.profile.PlayerProfile#getName` | 2 |
| `io.papermc.paper.command.brigadier.MessageComponentSerializer#serialize` | 2 |
| `io.papermc.paper.command.brigadier.argument.resolvers.selector.EntitySelectorArgumentResolver#resolve` | 2 |
| `io.papermc.paper.command.brigadier.argument.resolvers.selector.PlayerSelectorArgumentResolver#resolve` | 2 |
| `io.papermc.paper.plugin.bootstrap.BootstrapContext#getLogger` | 2 |
| `io.papermc.paper.plugin.lifecycle.event.LifecycleEventManager#registerEventHandler` | 2 |
| `io.papermc.paper.plugin.lifecycle.event.LifecycleEventOwner#getPluginMeta` | 2 |
| `io.papermc.paper.plugin.lifecycle.event.handler.configuration.PrioritizedLifecycleEventHandlerConfiguration#priority` | 2 |
| `io.papermc.paper.plugin.lifecycle.event.types.LifecycleEventType$Prioritizable#newHandler` | 2 |
| `io.papermc.paper.plugin.lifecycle.event.types.LifecycleEvents#COMMANDS` | 2 |
| `org.bukkit.Bukkit#reloadData` | 2 |
| `org.bukkit.command.ProxiedCommandSender#hasPermission` | 2 |
| `org.bukkit.command.ProxiedCommandSender#isOp` | 2 |
| `org.bukkit.plugin.java.JavaPlugin#getLifecycleManager` | 2 |
| `org.bukkit.potion.PotionEffectType#getName` | 2 |
| `io.papermc.paper.ServerBuildInfo#asString` | 1 |
| `io.papermc.paper.ServerBuildInfo#buildInfo` | 1 |
| `io.papermc.paper.ServerBuildInfo#minecraftVersionId` | 1 |
| `io.papermc.paper.ServerBuildInfo$StringRepresentation#VERSION_SIMPLE` | 1 |
| `io.papermc.paper.command.brigadier.Commands#register` | 1 |
| `io.papermc.paper.command.brigadier.MessageComponentSerializer#message` | 1 |
| `io.papermc.paper.command.brigadier.argument.ArgumentTypes#angle` | 1 |
| `io.papermc.paper.command.brigadier.argument.ArgumentTypes#axes` | 1 |
| `io.papermc.paper.command.brigadier.argument.ArgumentTypes#blockInWorldPredicate` | 1 |
| ... 71 more | |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| NMS (net.minecraft) | `net.minecraft.advancements` | own | 9 | yes |
| NMS (net.minecraft) | `net.minecraft.advancements.critereon` | own | 21 | yes |
| NMS (net.minecraft) | `net.minecraft.advancements.criterion` | own | 6 | yes |
| NMS (net.minecraft) | `net.minecraft.advancements.predicates` | own | 6 | yes |
| NMS (net.minecraft) | `net.minecraft.commands` | own | 113 | yes |
| NMS (net.minecraft) | `net.minecraft.commands.arguments` | own | 286 | yes |
| NMS (net.minecraft) | `net.minecraft.commands.execution` | own | 9 | yes |
| NMS (net.minecraft) | `net.minecraft.commands.functions` | own | 18 | yes |
| NMS (net.minecraft) | `net.minecraft.commands.synchronization` | own | 7 | yes |
| NMS (net.minecraft) | `net.minecraft.core` | own | 143 | yes |
| NMS (net.minecraft) | `net.minecraft.core.component` | own | 21 | yes |
| NMS (net.minecraft) | `net.minecraft.core.particles` | own | 105 | yes |
| NMS (net.minecraft) | `net.minecraft.core.registries` | own | 21 | yes |
| NMS (net.minecraft) | `net.minecraft.nbt` | own | 25 | yes |
| NMS (net.minecraft) | `net.minecraft.network.chat` | own | 23 | yes |
| NMS (net.minecraft) | `net.minecraft.resources` | own | 29 | yes |
| NMS (net.minecraft) | `net.minecraft.server` | own | 56 | yes |
| NMS (net.minecraft) | `net.minecraft.server.dedicated` | own | 20 | yes |
| NMS (net.minecraft) | `net.minecraft.server.level` | own | 21 | yes |
| NMS (net.minecraft) | `net.minecraft.server.permissions` | own | 4 | yes |
| NMS (net.minecraft) | `net.minecraft.sounds` | own | 9 | yes |
| NMS (net.minecraft) | `net.minecraft.util.profiling` | own | 16 | yes |
| NMS (net.minecraft) | `net.minecraft.world.effect` | own | 9 | yes |
| NMS (net.minecraft) | `net.minecraft.world.entity` | own | 14 | yes |
| NMS (net.minecraft) | `net.minecraft.world.flag` | own | 15 | yes |
| NMS (net.minecraft) | `net.minecraft.world.item` | own | 35 | yes |
| NMS (net.minecraft) | `net.minecraft.world.level` | own | 51 | yes |
| NMS (net.minecraft) | `net.minecraft.world.phys` | own | 37 | yes |
| NMS (net.minecraft) | `net.minecraft.world.scores` | own | 38 | yes |
| CraftBukkit | `org.bukkit.craftbukkit` | own | 65 | yes |

## 🔴 CoreProtect 24.1

- file: `coreprotect--CoreProtect-CE-24.1.jar`
- main: `net.coreprotect.CoreProtect`, api-version: 1.16
- softdepend: WorldEdit
- classes: 429 (429 own under `net.coreprotect`, 0 bundled), reflection calls: 78
- reflection into internals: getHandle() (1 refs, in own)
- bytecode instrumentation: javassist, javassist.bytecode (11 refs, in own)
- uses 666 uncovered API members and 222 uncovered API types (top: org.bukkit.block.Block#getState, org.bukkit.block.BlockState#getType, org.bukkit.block.BlockState#getBlockData, org.bukkit.Tag#getValues, org.bukkit.block.BlockState#getLocation)

| Uncovered member | Sites |
|---|---:|
| `org.bukkit.block.Block#getState` | 131 |
| `org.bukkit.block.BlockState#getType` | 54 |
| `org.bukkit.block.BlockState#getBlockData` | 49 |
| `org.bukkit.Tag#getValues` | 28 |
| `org.bukkit.block.BlockState#getLocation` | 27 |
| `org.bukkit.block.BlockState#getY` | 18 |
| `org.bukkit.block.BlockState#getX` | 16 |
| `org.bukkit.block.BlockState#getZ` | 16 |
| `org.bukkit.block.BlockState#getWorld` | 14 |
| `org.bukkit.entity.Player#sendBlockChange` | 13 |
| `org.bukkit.Material#isBlock` | 12 |
| `org.bukkit.World#getUID` | 12 |
| `org.bukkit.Chunk#getEntities` | 11 |
| `org.bukkit.entity.ItemFrame#getFacing` | 11 |
| `org.bukkit.entity.ItemFrame#getItem` | 11 |
| `org.bukkit.World#getMaxHeight` | 10 |
| `org.bukkit.block.Sign#getSide` | 10 |
| `org.bukkit.inventory.Inventory#getLocation` | 10 |
| `org.bukkit.block.data.Bisected$Half#TOP` | 9 |
| `org.bukkit.block.sign.Side#FRONT` | 8 |
| `org.bukkit.Tag#isTagged` | 7 |
| `org.bukkit.block.sign.Side#BACK` | 7 |
| `org.bukkit.block.BlockState#getBlock` | 6 |
| `org.bukkit.entity.ItemFrame#getLocation` | 6 |
| `org.bukkit.event.hanging.HangingBreakEvent$RemoveCause#equals` | 6 |
| `org.bukkit.block.data.type.Bed#getPart` | 5 |
| `org.bukkit.entity.ArmorStand#getLocation` | 5 |
| `org.bukkit.entity.Horse#getInventory` | 5 |
| `org.bukkit.entity.Villager#getClass` | 5 |
| `org.bukkit.event.inventory.InventoryMoveItemEvent#getItem` | 5 |
| `org.bukkit.inventory.Inventory#getMaxStackSize` | 5 |
| `org.bukkit.inventory.ItemStack#getMaxStackSize` | 5 |
| `com.destroystokyo.paper.profile.ProfileProperty#getValue` | 4 |
| `org.bukkit.Material#hasGravity` | 4 |
| `org.bukkit.Server#getEntity` | 4 |
| `org.bukkit.Tag#SAPLINGS` | 4 |
| `org.bukkit.block.Skull#getProfile` | 4 |
| `org.bukkit.block.data.FaceAttachable$AttachedFace#ordinal` | 4 |
| `org.bukkit.block.data.type.Bed$Part#HEAD` | 4 |
| `org.bukkit.block.data.type.Bell$Attachment#ordinal` | 4 |
| ... 626 more | |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| reflection into internals | `getHandle()` | own | 1 | yes |
| bytecode instrumentation | `javassist` | own | 10 | yes |
| bytecode instrumentation | `javassist.bytecode` | own | 1 | yes |

## 🔴 CrazyCrates 5.2.0

- file: `crazycrates--CrazyCrates-5.2.0.jar`
- main: `com.badbones69.crazycrates.paper.CrazyCrates`, api-version: 26.1.2
- softdepend: DecentHolograms, FancyHolograms, CMI, PlaceholderAPI, CraftEngine, ItemsAdder, Oraxen, Nexo, HeadDatabase, HMCWraps, ExcellentCrates
- classes: 1309 (179 own under `com.badbones69.crazycrates`, 1130 bundled), reflection calls: 98
- NMS (net.minecraft): net.minecraft.commands.arguments, net.minecraft.core, net.minecraft.core.component (18 refs, in libs.com.ryderbelserion)
- CraftBukkit: org.bukkit.craftbukkit, org.bukkit.craftbukkit.entity, org.bukkit.craftbukkit.inventory (5 refs, in libs.com.ryderbelserion)
- reflection into internals: Craft* class name (2 refs, in libs.com.ryderbelserion)
- uses 395 uncovered API members and 85 uncovered API types (top: org.bukkit.inventory.ItemType$Typed#key, org.bukkit.inventory.ItemStack#setData, org.bukkit.inventory.ItemStack#hasData, io.papermc.paper.registry.RegistryAccess#getRegistry, org.bukkit.Registry#get)

| Uncovered member | Sites |
|---|---:|
| `org.bukkit.inventory.ItemType$Typed#key` | 81 |
| `org.bukkit.inventory.ItemStack#setData` | 26 |
| `org.bukkit.inventory.ItemStack#hasData` | 17 |
| `io.papermc.paper.registry.RegistryAccess#getRegistry` | 12 |
| `org.bukkit.Registry#get` | 12 |
| `org.bukkit.inventory.ItemStack#getData` | 12 |
| `io.papermc.paper.persistence.PersistentDataContainerView#has` | 10 |
| `io.papermc.paper.datacomponent.DataComponentTypes#CUSTOM_NAME` | 8 |
| `io.papermc.paper.datacomponent.DataComponentTypes#LORE` | 8 |
| `io.papermc.paper.datacomponent.DataComponentTypes#TOOLTIP_DISPLAY` | 8 |
| `org.bukkit.inventory.ItemStack#editPersistentDataContainer` | 8 |
| `org.bukkit.block.Block#getState` | 7 |
| `org.bukkit.event.inventory.InventoryCloseEvent$Reason#UNLOADED` | 7 |
| `org.bukkit.inventory.ItemType$Typed#createItemStack` | 7 |
| `io.papermc.paper.persistence.PersistentDataContainerView#getOrDefault` | 6 |
| `org.bukkit.FireworkEffect$Builder#withColor` | 6 |
| `org.bukkit.inventory.ItemType#BLACK_STAINED_GLASS_PANE` | 6 |
| `io.papermc.paper.datacomponent.DataComponentTypes#CUSTOM_MODEL_DATA` | 5 |
| `io.papermc.paper.persistence.PersistentDataContainerView#get` | 5 |
| `org.bukkit.block.BlockState#update` | 5 |
| `org.bukkit.event.inventory.InventoryClickEvent#getAction` | 5 |
| `io.papermc.paper.datacomponent.DataComponentTypes#ITEM_NAME` | 4 |
| `io.papermc.paper.datacomponent.DataComponentTypes#PROFILE` | 4 |
| `io.papermc.paper.datacomponent.DataComponentTypes#UNBREAKABLE` | 4 |
| `org.bukkit.entity.Player#getVehicle` | 4 |
| `org.bukkit.entity.Player#isSneaking` | 4 |
| `org.bukkit.inventory.ItemType#AIR` | 4 |
| `org.bukkit.inventory.ItemType#createItemStack` | 4 |
| `io.papermc.paper.datacomponent.DataComponentTypes#ENCHANTMENT_GLINT_OVERRIDE` | 3 |
| `io.papermc.paper.datacomponent.DataComponentTypes#STORED_ENCHANTMENTS` | 3 |
| `io.papermc.paper.datacomponent.item.CustomModelData#floats` | 3 |
| `io.papermc.paper.datacomponent.item.ItemLore#lines` | 3 |
| `io.papermc.paper.datacomponent.item.TooltipDisplay$Builder#hideTooltip` | 3 |
| `org.bukkit.FireworkEffect$Builder#build` | 3 |
| `org.bukkit.Material#asItemType` | 3 |
| `org.bukkit.block.Chest#isOpen` | 3 |
| `org.bukkit.block.EnderChest#isOpen` | 3 |
| `org.bukkit.event.inventory.InventoryAction#MOVE_TO_OTHER_INVENTORY` | 3 |
| `org.bukkit.inventory.ItemType#STONE` | 3 |
| `org.bukkit.inventory.ItemType#createItemStack` | 3 |
| ... 355 more | |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| NMS (net.minecraft) | `net.minecraft.commands.arguments` | libs.com.ryderbelserion | 2 | yes |
| NMS (net.minecraft) | `net.minecraft.core` | libs.com.ryderbelserion | 4 | yes |
| NMS (net.minecraft) | `net.minecraft.core.component` | libs.com.ryderbelserion | 1 | yes |
| NMS (net.minecraft) | `net.minecraft.network.chat` | libs.com.ryderbelserion | 1 | yes |
| NMS (net.minecraft) | `net.minecraft.network.protocol` | libs.com.ryderbelserion | 2 | yes |
| NMS (net.minecraft) | `net.minecraft.server.level` | libs.com.ryderbelserion | 1 | yes |
| NMS (net.minecraft) | `net.minecraft.server.network` | libs.com.ryderbelserion | 1 | yes |
| NMS (net.minecraft) | `net.minecraft.world.entity` | libs.com.ryderbelserion | 1 | yes |
| NMS (net.minecraft) | `net.minecraft.world.inventory` | libs.com.ryderbelserion | 2 | yes |
| NMS (net.minecraft) | `net.minecraft.world.item` | libs.com.ryderbelserion | 2 | yes |
| NMS (net.minecraft) | `net.minecraft.world.level` | libs.com.ryderbelserion | 1 | yes |
| CraftBukkit | `org.bukkit.craftbukkit` | libs.com.ryderbelserion | 1 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.entity` | libs.com.ryderbelserion | 1 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.inventory` | libs.com.ryderbelserion | 2 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.util` | libs.com.ryderbelserion | 1 | yes |
| reflection into internals | `Craft* class name` | libs.com.ryderbelserion | 2 | yes |
| bytecode instrumentation | `javassist` | libs.com.zaxxer | 10 | no |
| bytecode instrumentation | `javassist.bytecode` | libs.com.zaxxer | 1 | no |

## 🔴 DeathChest 3.0.1

- file: `deathchest--deathchest.jar`
- main: `com.github.devcyntrix.deathchest.DeathChestPlugin`, api-version: 1.17
- softdepend: WorldGuard, ProtocolLib, GriefPrevention, GriefDefender, PlaceholderAPI, RedProtect, minePlots, LocketteX, LWC
- classes: 149 (149 own under `com.github.devcyntrix`, 0 bundled), reflection calls: 19
- ProtocolLib / PacketEvents: com.comphenix.protocol, com.comphenix.protocol.events, com.comphenix.protocol.reflect (10 refs, in own)
- reflection into internals: getHandle(), net.minecraft.server, net.minecraft.world (4 refs, in own)
- uses 74 uncovered API members and 17 uncovered API types (top: org.bukkit.event.inventory.InventoryAction#ordinal, org.bukkit.event.entity.PlayerDeathEvent#getDrops, org.bukkit.World#getMinHeight, org.bukkit.block.Block#getState, org.bukkit.entity.ArmorStand#isValid)

| Uncovered member | Sites |
|---|---:|
| `org.bukkit.event.inventory.InventoryAction#ordinal` | 14 |
| `org.bukkit.event.entity.PlayerDeathEvent#getDrops` | 4 |
| `org.bukkit.World#getMinHeight` | 3 |
| `org.bukkit.block.Block#getState` | 3 |
| `org.bukkit.entity.ArmorStand#isValid` | 3 |
| `org.bukkit.Chunk#unload` | 2 |
| `org.bukkit.World#getMaxHeight` | 2 |
| `org.bukkit.command.BlockCommandSender#getBlock` | 2 |
| `org.bukkit.entity.ArmorStand#setCustomName` | 2 |
| `org.bukkit.inventory.InventoryView#getInventory` | 2 |
| `org.bukkit.plugin.PluginManager#removePermission` | 2 |
| `net.kyori.adventure.platform.bukkit.BukkitAudiences#all` | 1 |
| `net.kyori.adventure.platform.bukkit.BukkitAudiences#close` | 1 |
| `net.kyori.adventure.platform.bukkit.BukkitAudiences#create` | 1 |
| `net.kyori.adventure.platform.bukkit.BukkitAudiences#player` | 1 |
| `org.bukkit.Bukkit#getEntity` | 1 |
| `org.bukkit.Bukkit#getOnlineMode` | 1 |
| `org.bukkit.Bukkit#getSpawnRadius` | 1 |
| `org.bukkit.Bukkit#getUpdateFolderFile` | 1 |
| `org.bukkit.Instrument#BASS_DRUM` | 1 |
| `org.bukkit.Material#isSolid` | 1 |
| `org.bukkit.Note#flat` | 1 |
| `org.bukkit.Note$Tone#D` | 1 |
| `org.bukkit.Registry#SOUNDS` | 1 |
| `org.bukkit.Registry#match` | 1 |
| `org.bukkit.World#getWorldFolder` | 1 |
| `org.bukkit.block.BlockState#update` | 1 |
| `org.bukkit.block.Lidded#close` | 1 |
| `org.bukkit.block.Lidded#open` | 1 |
| `org.bukkit.entity.ArmorStand#getUniqueId` | 1 |
| `org.bukkit.entity.ArmorStand#remove` | 1 |
| `org.bukkit.entity.ArmorStand#setCustomNameVisible` | 1 |
| `org.bukkit.entity.ArmorStand#setInvisible` | 1 |
| `org.bukkit.entity.ArmorStand#setMarker` | 1 |
| `org.bukkit.entity.ArmorStand#setPersistent` | 1 |
| `org.bukkit.entity.ArmorStand#teleport` | 1 |
| `org.bukkit.entity.Player#isSneaking` | 1 |
| `org.bukkit.event.block.BlockExplodeEvent#blockList` | 1 |
| `org.bukkit.event.block.BlockExplodeEvent#getBlock` | 1 |
| `org.bukkit.event.entity.EntityChangeBlockEvent#getBlock` | 1 |
| ... 34 more | |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| ProtocolLib / PacketEvents | `com.comphenix.protocol` | own | 7 | yes |
| ProtocolLib / PacketEvents | `com.comphenix.protocol.events` | own | 1 | yes |
| ProtocolLib / PacketEvents | `com.comphenix.protocol.reflect` | own | 1 | yes |
| ProtocolLib / PacketEvents | `com.comphenix.protocol.wrappers` | own | 1 | yes |
| reflection into internals | `getHandle()` | own | 1 | yes |
| reflection into internals | `net.minecraft.server` | own | 1 | yes |
| reflection into internals | `net.minecraft.world` | own | 1 | yes |
| reflection into internals | `org.bukkit.craftbukkit` | own | 1 | yes |

## 🔴 DecentHolograms 2.10.2

- file: `decentholograms--DecentHolograms-2.10.2.jar`
- main: `eu.decentsoftware.holograms.plugin.DecentHologramsPlugin`, api-version: 1.13
- softdepend: PlaceholderAPI, HeadDatabase
- classes: 2197 (1252 own under `eu.decentsoftware.holograms`, 945 bundled), reflection calls: 358
- NMS (net.minecraft): net.minecraft, net.minecraft.core, net.minecraft.core.registries (1909 refs, in own)
- CraftBukkit: org.bukkit.craftbukkit, org.bukkit.craftbukkit.entity, org.bukkit.craftbukkit.inventory (192 refs, in own)
- server internals (Mojang libs, Netty, Paper internals): com.mojang.authlib, com.mojang.authlib.properties, com.mojang.datafixers (367 refs, in eu.decentsoftware.holograms.libs.nbtapi, eu.decentsoftware.holograms.libs.nbtapi.utils, eu.decentsoftware.holograms.libs.xseries.profiles)
- reflection into internals: Craft* class name, com.mojang.authlib, getHandle() (103 refs, in eu.decentsoftware.holograms.libs.nbtapi.utils, eu.decentsoftware.holograms.libs.xseries, eu.decentsoftware.holograms.libs.xseries.messages)
- uses 403 uncovered API members and 136 uncovered API types (top: org.bukkit.entity.Player#isSneaking, org.bukkit.block.Block#getState, org.bukkit.entity.EntityType#isAlive, org.bukkit.block.BlockState#getData, org.bukkit.SkullType#ordinal)

| Uncovered member | Sites |
|---|---:|
| `org.bukkit.entity.Player#isSneaking` | 70 |
| `org.bukkit.block.Block#getState` | 23 |
| `org.bukkit.entity.EntityType#isAlive` | 18 |
| `org.bukkit.block.BlockState#getData` | 14 |
| `org.bukkit.SkullType#ordinal` | 10 |
| `org.bukkit.block.BlockState#setRawData` | 8 |
| `org.bukkit.Registry#get` | 7 |
| `org.bukkit.TreeSpecies#ordinal` | 7 |
| `org.bukkit.block.BlockState#update` | 7 |
| `org.bukkit.SkullType#PLAYER` | 5 |
| `org.bukkit.WorldBorder#getCenter` | 5 |
| `org.bukkit.potion.PotionType#name` | 5 |
| `org.bukkit.Bukkit#getOnlineMode` | 4 |
| `org.bukkit.Material#isBlock` | 4 |
| `org.bukkit.World#getUID` | 4 |
| `org.bukkit.block.Block#breakNaturally` | 4 |
| `org.bukkit.block.BlockState#getX` | 4 |
| `org.bukkit.block.BlockState#getY` | 4 |
| `org.bukkit.block.BlockState#getZ` | 4 |
| `org.bukkit.potion.PotionEffect#getType` | 4 |
| `org.bukkit.potion.PotionEffectType#getByName` | 4 |
| `org.bukkit.potion.PotionType#valueOf` | 4 |
| `org.bukkit.Chunk#getPersistentDataContainer` | 3 |
| `org.bukkit.Instrument#BASS_DRUM` | 3 |
| `org.bukkit.Material#getMaxDurability` | 3 |
| `org.bukkit.Note#getId` | 3 |
| `org.bukkit.Particle$DustTransition#<init>` | 3 |
| `org.bukkit.TreeSpecies#getData` | 3 |
| `org.bukkit.WorldBorder#getDamageBuffer` | 3 |
| `org.bukkit.WorldBorder#getSize` | 3 |
| `org.bukkit.WorldBorder#getWarningDistance` | 3 |
| `org.bukkit.WorldBorder#getWarningTime` | 3 |
| `org.bukkit.block.BlockState#update` | 3 |
| `org.bukkit.entity.Player#stopSound` | 3 |
| `org.bukkit.inventory.ItemStack#getData` | 3 |
| `org.bukkit.inventory.ItemStack#getMaxStackSize` | 3 |
| `org.bukkit.inventory.meta.BlockStateMeta#setBlockState` | 3 |
| `org.bukkit.inventory.meta.components.CustomModelDataComponent#setColors` | 3 |
| `org.bukkit.inventory.meta.components.CustomModelDataComponent#setFloats` | 3 |
| `org.bukkit.inventory.meta.components.CustomModelDataComponent#setStrings` | 3 |
| ... 363 more | |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| NMS (net.minecraft) | `net.minecraft` | own | 42 | yes |
| NMS (net.minecraft) | `net.minecraft.core` | own | 23 | yes |
| NMS (net.minecraft) | `net.minecraft.core.registries` | own | 17 | yes |
| NMS (net.minecraft) | `net.minecraft.network` | own | 73 | yes |
| NMS (net.minecraft) | `net.minecraft.network.chat` | own | 166 | yes |
| NMS (net.minecraft) | `net.minecraft.network.codec` | own | 22 | yes |
| NMS (net.minecraft) | `net.minecraft.network.protocol` | own | 191 | yes |
| NMS (net.minecraft) | `net.minecraft.network.syncher` | own | 204 | yes |
| NMS (net.minecraft) | `net.minecraft.resources` | own | 17 | yes |
| NMS (net.minecraft) | `net.minecraft.server.level` | own | 43 | yes |
| NMS (net.minecraft) | `net.minecraft.server.network` | own | 58 | yes |
| NMS (net.minecraft) | `net.minecraft.server.v1_10_R1` | own | 44 | yes |
| NMS (net.minecraft) | `net.minecraft.server.v1_11_R1` | own | 44 | yes |
| NMS (net.minecraft) | `net.minecraft.server.v1_12_R1` | own | 44 | yes |
| NMS (net.minecraft) | `net.minecraft.server.v1_13_R1` | own | 48 | yes |
| NMS (net.minecraft) | `net.minecraft.server.v1_13_R2` | own | 48 | yes |
| NMS (net.minecraft) | `net.minecraft.server.v1_14_R1` | own | 50 | yes |
| NMS (net.minecraft) | `net.minecraft.server.v1_15_R1` | own | 43 | yes |
| NMS (net.minecraft) | `net.minecraft.server.v1_16_R1` | own | 50 | yes |
| NMS (net.minecraft) | `net.minecraft.server.v1_16_R2` | own | 50 | yes |
| NMS (net.minecraft) | `net.minecraft.server.v1_16_R3` | own | 50 | yes |
| NMS (net.minecraft) | `net.minecraft.server.v1_8_R1` | own | 38 | yes |
| NMS (net.minecraft) | `net.minecraft.server.v1_8_R2` | own | 40 | yes |
| NMS (net.minecraft) | `net.minecraft.server.v1_8_R3` | own | 40 | yes |
| NMS (net.minecraft) | `net.minecraft.server.v1_9_R1` | own | 45 | yes |
| NMS (net.minecraft) | `net.minecraft.server.v1_9_R2` | own | 45 | yes |
| NMS (net.minecraft) | `net.minecraft.util` | own | 3 | yes |
| NMS (net.minecraft) | `net.minecraft.world.entity` | own | 239 | yes |
| NMS (net.minecraft) | `net.minecraft.world.item` | own | 63 | yes |
| NMS (net.minecraft) | `net.minecraft.world.level` | own | 48 | yes |

## 🔴 DiscordSRV 1.30.5

- file: `discordsrv--DiscordSRV-Build-1.30.5.jar`
- main: `github.scarsz.discordsrv.DiscordSRV`, api-version: 1.13
- softdepend: Chatty, FancyChat, Herochat, Legendchat, LunaChat, nChat, TownyChat, VentureChat, Essentials, PhantomAdmin, SuperVanish, Multiverse-Core, Vault, PlaceholderAPI, mcMMO, LuckPerms, dynmap, Skript
- classes: 5543 (200 own under `github.scarsz.discordsrv`, 5343 bundled), reflection calls: 384
- server internals (Mojang libs, Netty, Paper internals): io.netty.buffer (5 refs, in github.scarsz.discordsrv.dependencies.kyori.adventure)
- ProtocolLib / PacketEvents: com.comphenix.protocol.events (1 refs, in own)
- reflection into internals: Craft* class name, com.mojang.authlib, getHandle() (32 refs, in github.scarsz.discordsrv.dependencies.kyori.adventure, own)
- uses 57 uncovered API members and 22 uncovered API types (top: org.bukkit.event.player.PlayerKickEvent#getPlayer, org.bukkit.advancement.Advancement#getKey, net.milkbowl.vault.permission.Permission#playerInGroup, org.bukkit.Bukkit#getOfflinePlayers, org.bukkit.Bukkit#getOnlineMode)

| Uncovered member | Sites |
|---|---:|
| `org.bukkit.event.player.PlayerKickEvent#getPlayer` | 7 |
| `org.bukkit.advancement.Advancement#getKey` | 4 |
| `net.milkbowl.vault.permission.Permission#playerInGroup` | 3 |
| `org.bukkit.Bukkit#getOfflinePlayers` | 3 |
| `org.bukkit.Bukkit#getOnlineMode` | 3 |
| `org.bukkit.OfflinePlayer#hasPlayedBefore` | 3 |
| `org.bukkit.scheduler.BukkitWorker#getOwner` | 3 |
| `org.bukkit.BanList$Type#NAME` | 2 |
| `org.bukkit.Bukkit#getBanList` | 2 |
| `org.bukkit.Server#getBannedPlayers` | 2 |
| `org.bukkit.Server#getIPBans` | 2 |
| `org.bukkit.Server#getWhitelistedPlayers` | 2 |
| `org.bukkit.entity.Player#hasPlayedBefore` | 2 |
| `org.bukkit.event.player.PlayerAdvancementDoneEvent#getAdvancement` | 2 |
| `org.bukkit.event.player.PlayerAdvancementDoneEvent#getPlayer` | 2 |
| `org.bukkit.scheduler.BukkitWorker#getThread` | 2 |
| `com.destroystokyo.paper.Title#builder` | 1 |
| `com.destroystokyo.paper.Title$Builder#build` | 1 |
| `com.destroystokyo.paper.Title$Builder#fadeIn` | 1 |
| `com.destroystokyo.paper.Title$Builder#fadeOut` | 1 |
| `com.destroystokyo.paper.Title$Builder#stay` | 1 |
| `com.destroystokyo.paper.Title$Builder#subtitle` | 1 |
| `com.destroystokyo.paper.Title$Builder#title` | 1 |
| `io.papermc.paper.advancement.AdvancementDisplay#doesAnnounceToChat` | 1 |
| `net.milkbowl.vault.permission.Permission#getGroups` | 1 |
| `net.milkbowl.vault.permission.Permission#getName` | 1 |
| `org.bukkit.BanList#addBan` | 1 |
| `org.bukkit.BanList#isBanned` | 1 |
| `org.bukkit.BanList#pardon` | 1 |
| `org.bukkit.Bukkit#getBannedPlayers` | 1 |
| `org.bukkit.Bukkit#getUnsafe` | 1 |
| `org.bukkit.Server#getWarningState` | 1 |
| `org.bukkit.Server#selectEntities` | 1 |
| `org.bukkit.UnsafeValues#getDataVersion` | 1 |
| `org.bukkit.Warning$WarningState#printFor` | 1 |
| `org.bukkit.World#getGameRuleValue` | 1 |
| `org.bukkit.World#getGameRuleValue` | 1 |
| `org.bukkit.World#getWorldFolder` | 1 |
| `org.bukkit.advancement.Advancement#getClass` | 1 |
| `org.bukkit.advancement.AdvancementDisplay#shouldAnnounceChat` | 1 |
| ... 17 more | |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| server internals (Mojang libs, Netty, Paper internals) | `io.netty.buffer` | github.scarsz.discordsrv.dependencies.kyori.adventure | 5 | yes |
| ProtocolLib / PacketEvents | `com.comphenix.protocol.events` | own | 1 | yes |
| reflection into internals | `Craft* class name` | github.scarsz.discordsrv.dependencies.kyori.adventure | 9 | yes |
| reflection into internals | `com.mojang.authlib` | own | 2 | yes |
| reflection into internals | `getHandle()` | github.scarsz.discordsrv.dependencies.kyori.adventure | 2 | yes |
| reflection into internals | `getHandle()` | own | 2 | yes |
| reflection into internals | `net.minecraft.` | github.scarsz.discordsrv.dependencies.kyori.adventure | 1 | yes |
| reflection into internals | `net.minecraft.advancements` | own | 1 | yes |
| reflection into internals | `net.minecraft.server` | github.scarsz.discordsrv.dependencies.kyori.adventure | 1 | yes |
| reflection into internals | `net.minecraft.server` | own | 6 | yes |
| reflection into internals | `net.minecraft.util` | own | 2 | yes |
| reflection into internals | `net.minecraft.world` | own | 1 | yes |
| reflection into internals | `org.bukkit.craftbukkit` | github.scarsz.discordsrv.dependencies.kyori.adventure | 2 | yes |
| reflection into internals | `org.bukkit.craftbukkit` | own | 3 | yes |

## 🔴 dynmap 3.5-873

- file: `dynmap--Dynmap-3.5-spigot.jar`
- main: `org.dynmap.bukkit.DynmapPlugin`, api-version: 1.13
- softdepend: Permissions, PermissionEx, bPermissions, PermissionsBukkit, GroupManager, LuckPerms, Vault, SkinsRestorer
- classes: 2586 (153 own under `org.dynmap.bukkit`, 2433 bundled), reflection calls: 260
- NMS (net.minecraft): net.minecraft.core, net.minecraft.core.registries, net.minecraft.nbt (513 refs, in own)
- CraftBukkit: org.bukkit.craftbukkit.v1_14_R1, org.bukkit.craftbukkit.v1_15_R1, org.bukkit.craftbukkit.v1_16_R1 (38 refs, in own)
- server internals (Mojang libs, Netty, Paper internals): com.mojang.authlib, com.mojang.authlib.properties (18 refs, in own)
- reflection into internals: com.mojang.authlib, getHandle(), net.minecraft.server (43 refs, in own)
- uses 78 uncovered API members and 29 uncovered API types (top: org.bukkit.World#getWorldBorder, org.bukkit.WorldBorder#getCenter, org.bukkit.WorldBorder#getSize, org.bukkit.plugin.PluginManager#enablePlugin, org.bukkit.OfflinePlayer#isBanned)

| Uncovered member | Sites |
|---|---:|
| `org.bukkit.World#getWorldBorder` | 13 |
| `org.bukkit.WorldBorder#getCenter` | 13 |
| `org.bukkit.WorldBorder#getSize` | 13 |
| `org.bukkit.plugin.PluginManager#enablePlugin` | 6 |
| `org.bukkit.OfflinePlayer#isBanned` | 3 |
| `net.milkbowl.vault.permission.Permission#getName` | 2 |
| `org.bukkit.ChunkSnapshot#getBiome` | 2 |
| `org.bukkit.ChunkSnapshot#getBlockEmittedLight` | 2 |
| `org.bukkit.ChunkSnapshot#getBlockSkyLight` | 2 |
| `org.bukkit.ChunkSnapshot#getHighestBlockYAt` | 2 |
| `org.bukkit.ChunkSnapshot#isSectionEmpty` | 2 |
| `org.bukkit.World#getMaxHeight` | 2 |
| `org.bukkit.World#isChunkInUse` | 2 |
| `org.bukkit.World#unloadChunk` | 2 |
| `org.bukkit.World#unloadChunk` | 2 |
| `org.bukkit.World#unloadChunkRequest` | 2 |
| `org.bukkit.block.Biome#PLAINS` | 2 |
| `org.bukkit.block.Biome#ordinal` | 2 |
| `org.bukkit.event.server.ServiceRegisterEvent#getProvider` | 2 |
| `org.bukkit.event.world.ChunkPopulateEvent#getWorld` | 2 |
| `org.bukkit.Bukkit#getOnlineMode` | 1 |
| `org.bukkit.Chunk#getChunkSnapshot` | 1 |
| `org.bukkit.ChunkSnapshot#getBlockData` | 1 |
| `org.bukkit.ChunkSnapshot#getBlockTypeId` | 1 |
| `org.bukkit.Material#getId` | 1 |
| `org.bukkit.OfflinePlayer#getBedSpawnLocation` | 1 |
| `org.bukkit.OfflinePlayer#getFirstPlayed` | 1 |
| `org.bukkit.OfflinePlayer#getLastPlayed` | 1 |
| `org.bukkit.Server#getIPBans` | 1 |
| `org.bukkit.Server#getIp` | 1 |
| `org.bukkit.Server#getServerName` | 1 |
| `org.bukkit.World#getEmptyChunkSnapshot` | 1 |
| `org.bukkit.World#getLoadedChunks` | 1 |
| `org.bukkit.World#getSeaLevel` | 1 |
| `org.bukkit.World#getWorldFolder` | 1 |
| `org.bukkit.World#isChunkGenerated` | 1 |
| `org.bukkit.attribute.Attribute#GENERIC_ARMOR` | 1 |
| `org.bukkit.attribute.AttributeInstance#getValue` | 1 |
| `org.bukkit.block.Biome#toString` | 1 |
| `org.bukkit.block.Biome#values` | 1 |
| ... 38 more | |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| NMS (net.minecraft) | `net.minecraft.core` | own | 35 | yes |
| NMS (net.minecraft) | `net.minecraft.core.registries` | own | 4 | yes |
| NMS (net.minecraft) | `net.minecraft.nbt` | own | 106 | yes |
| NMS (net.minecraft) | `net.minecraft.resources` | own | 12 | yes |
| NMS (net.minecraft) | `net.minecraft.server` | own | 10 | yes |
| NMS (net.minecraft) | `net.minecraft.server.dedicated` | own | 4 | yes |
| NMS (net.minecraft) | `net.minecraft.server.level` | own | 22 | yes |
| NMS (net.minecraft) | `net.minecraft.server.v1_13_R2` | own | 11 | yes |
| NMS (net.minecraft) | `net.minecraft.server.v1_14_R1` | own | 25 | yes |
| NMS (net.minecraft) | `net.minecraft.server.v1_15_R1` | own | 28 | yes |
| NMS (net.minecraft) | `net.minecraft.server.v1_16_R1` | own | 29 | yes |
| NMS (net.minecraft) | `net.minecraft.server.v1_16_R2` | own | 66 | yes |
| NMS (net.minecraft) | `net.minecraft.server.v1_16_R3` | own | 38 | yes |
| NMS (net.minecraft) | `net.minecraft.util` | own | 6 | yes |
| NMS (net.minecraft) | `net.minecraft.world.level` | own | 117 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_14_R1` | own | 1 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_15_R1` | own | 1 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_16_R1` | own | 1 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_16_R2` | own | 2 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_16_R3` | own | 1 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_17_R1` | own | 4 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_18_R1` | own | 4 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_18_R2` | own | 6 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_19_R1` | own | 6 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_19_R2` | own | 6 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_19_R3` | own | 6 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `com.mojang.authlib` | own | 6 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `com.mojang.authlib.properties` | own | 12 | yes |
| reflection into internals | `com.mojang.authlib` | own | 2 | yes |
| reflection into internals | `getHandle()` | own | 6 | yes |

## 🔴 Essentials 2.22.0

- file: `essentialsx--EssentialsX-2.22.0.jar`
- main: `com.earth2me.essentials.Essentials`, api-version: 1.13
- softdepend: Vault, LuckPerms
- classes: 2142 (471 own under `com.earth2me.essentials`, 1671 bundled), reflection calls: 248
- server internals (Mojang libs, Netty, Paper internals): io.netty.buffer (4 refs, in com.earth2me.essentials.libs.kyori.adventure)
- reflection into internals: Craft* class name, getHandle(), net.minecraft. (25 refs, in com.earth2me.essentials.libs.kyori.adventure, net.ess3.nms)
- uses 603 uncovered API members and 234 uncovered API types (top: net.milkbowl.vault.economy.EconomyResponse#<init>, org.bukkit.Achievement#ordinal, org.bukkit.event.player.PlayerTeleportEvent$TeleportCause#COMMAND, net.milkbowl.vault.economy.EconomyResponse$ResponseType#FAILURE, org.bukkit.block.Block#getState)

| Uncovered member | Sites |
|---|---:|
| `net.milkbowl.vault.economy.EconomyResponse#<init>` | 35 |
| `org.bukkit.Achievement#ordinal` | 35 |
| `org.bukkit.event.player.PlayerTeleportEvent$TeleportCause#COMMAND` | 34 |
| `net.milkbowl.vault.economy.EconomyResponse$ResponseType#FAILURE` | 20 |
| `org.bukkit.block.Block#getState` | 19 |
| `org.bukkit.Server#getBanList` | 15 |
| `net.milkbowl.vault.economy.EconomyResponse$ResponseType#NOT_IMPLEMENTED` | 11 |
| `org.bukkit.BanEntry#getReason` | 10 |
| `org.bukkit.BanList$Type#NAME` | 10 |
| `org.bukkit.Material#getId` | 9 |
| `org.bukkit.inventory.ItemStack#getMaxStackSize` | 9 |
| `org.bukkit.World#getWorldBorder` | 8 |
| `org.bukkit.command.PluginIdentifiableCommand#getPlugin` | 8 |
| `org.bukkit.entity.Player#getAllowFlight` | 8 |
| `org.bukkit.event.player.PlayerTeleportEvent$TeleportCause#PLUGIN` | 8 |
| `org.bukkit.Material#getMaxStackSize` | 7 |
| `org.bukkit.entity.Player#getStatistic` | 7 |
| `org.bukkit.potion.PotionEffectType#INVISIBILITY` | 7 |
| `org.bukkit.potion.PotionEffectType#WATER_BREATHING` | 7 |
| `org.bukkit.BanEntry#getExpiration` | 6 |
| `org.bukkit.BanList#addBan` | 6 |
| `org.bukkit.BanList$Type#IP` | 6 |
| `org.bukkit.block.Sign#getLine` | 6 |
| `org.bukkit.entity.Player#isFlying` | 6 |
| `org.bukkit.entity.Player#setFlySpeed` | 6 |
| `org.bukkit.inventory.ShapedRecipe#getShape` | 6 |
| `org.bukkit.potion.PotionEffectType#getName` | 6 |
| `org.bukkit.potion.PotionType#name` | 6 |
| `org.bukkit.potion.PotionType#valueOf` | 6 |
| `org.bukkit.enchantments.Enchantment#getMaxLevel` | 5 |
| `org.bukkit.entity.Player#chat` | 5 |
| `org.bukkit.entity.Player#setFireTicks` | 5 |
| `org.bukkit.entity.Player#setWalkSpeed` | 5 |
| `org.bukkit.event.block.SignChangeEvent#getLine` | 5 |
| `org.bukkit.inventory.EntityEquipment#setBoots` | 5 |
| `org.bukkit.inventory.EntityEquipment#setChestplate` | 5 |
| `org.bukkit.inventory.EntityEquipment#setHelmet` | 5 |
| `org.bukkit.inventory.EntityEquipment#setLeggings` | 5 |
| `org.bukkit.potion.PotionEffectType#SPEED` | 5 |
| `net.milkbowl.vault.economy.EconomyResponse$ResponseType#SUCCESS` | 4 |
| ... 563 more | |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| server internals (Mojang libs, Netty, Paper internals) | `io.netty.buffer` | com.earth2me.essentials.libs.kyori.adventure | 4 | yes |
| reflection into internals | `Craft* class name` | com.earth2me.essentials.libs.kyori.adventure | 9 | yes |
| reflection into internals | `Craft* class name` | net.ess3.nms | 5 | yes |
| reflection into internals | `getHandle()` | com.earth2me.essentials.libs.kyori.adventure | 2 | yes |
| reflection into internals | `net.minecraft.` | com.earth2me.essentials.libs.kyori.adventure | 1 | yes |
| reflection into internals | `net.minecraft.locale` | net.ess3.nms | 2 | yes |
| reflection into internals | `net.minecraft.server` | com.earth2me.essentials.libs.kyori.adventure | 1 | yes |
| reflection into internals | `net.minecraft.server` | net.ess3.nms | 1 | yes |
| reflection into internals | `org.bukkit.craftbukkit` | com.earth2me.essentials.libs.kyori.adventure | 2 | yes |
| reflection into internals | `org.bukkit.craftbukkit` | net.ess3.nms | 2 | yes |

## 🔴 FancyHolograms 2.12.0

- file: `fancyholograms--FancyHolograms-2.12.0.jar`
- main: `de.oliver.fancyholograms.FancyHolograms`, api-version: 1.19
- softdepend: FancyNpcs, MiniPlaceholders, PlaceholderAPI, floodgate
- classes: 818 (88 own under `de.oliver.fancyholograms`, 730 bundled), reflection calls: 63
- NMS (net.minecraft): net.minecraft, net.minecraft.core, net.minecraft.core.registries (1302 refs, in de.oliver.fancysitula)
- CraftBukkit: org.bukkit.craftbukkit, org.bukkit.craftbukkit.block, org.bukkit.craftbukkit.entity (48 refs, in de.oliver.fancysitula)
- server internals (Mojang libs, Netty, Paper internals): com.mojang.authlib, com.mojang.authlib.properties, com.mojang.datafixers.util (106 refs, in de.oliver.fancysitula)
- reflection into internals: net.minecraft.world (271 refs, in de.oliver.fancysitula)
- uses 38 uncovered API members and 18 uncovered API types (top: org.bukkit.entity.Display$Billboard#name, org.bukkit.Bukkit#isStopping, org.bukkit.entity.Display$Billboard#CENTER, org.bukkit.entity.TextDisplay$TextAlignment#CENTER, org.bukkit.entity.TextDisplay$TextAlignment#ordinal)

| Uncovered member | Sites |
|---|---:|
| `org.bukkit.entity.Display$Billboard#name` | 5 |
| `org.bukkit.Bukkit#isStopping` | 4 |
| `org.bukkit.entity.Display$Billboard#CENTER` | 4 |
| `org.bukkit.entity.TextDisplay$TextAlignment#CENTER` | 4 |
| `org.bukkit.entity.TextDisplay$TextAlignment#ordinal` | 4 |
| `org.bukkit.entity.Display$Billboard#FIXED` | 3 |
| `org.bukkit.entity.Display$Billboard#VERTICAL` | 3 |
| `org.bukkit.entity.Display$Brightness#<init>` | 3 |
| `org.bukkit.entity.Display$Brightness#getBlockLight` | 3 |
| `org.bukkit.entity.Display$Brightness#getSkyLight` | 3 |
| `org.bukkit.entity.TextDisplay$TextAlignment#name` | 3 |
| `org.bukkit.event.player.PlayerResourcePackStatusEvent#getPlayer` | 3 |
| `com.destroystokyo.paper.profile.ProfileProperty#getName` | 2 |
| `org.bukkit.Server#sendMessage` | 2 |
| `org.bukkit.entity.TextDisplay$TextAlignment#LEFT` | 2 |
| `org.bukkit.entity.TextDisplay$TextAlignment#RIGHT` | 2 |
| `org.bukkit.entity.TextDisplay$TextAlignment#values` | 2 |
| `org.bukkit.event.world.WorldLoadEvent#getWorld` | 2 |
| `org.bukkit.event.world.WorldUnloadEvent#getWorld` | 2 |
| `com.destroystokyo.paper.profile.PlayerProfile#getId` | 1 |
| `com.destroystokyo.paper.profile.PlayerProfile#getName` | 1 |
| `com.destroystokyo.paper.profile.PlayerProfile#getProperties` | 1 |
| `com.destroystokyo.paper.profile.ProfileProperty#getSignature` | 1 |
| `com.destroystokyo.paper.profile.ProfileProperty#getValue` | 1 |
| `io.papermc.paper.event.player.PlayerClientLoadedWorldEvent#getPlayer` | 1 |
| `io.papermc.paper.plugin.bootstrap.PluginBootstrap#createPlugin` | 1 |
| `org.bukkit.Bukkit#getOnlineMode` | 1 |
| `org.bukkit.Material#isBlock` | 1 |
| `org.bukkit.Server#getOnlineMode` | 1 |
| `org.bukkit.block.data.BlockData#createBlockState` | 1 |
| `org.bukkit.entity.Display$Billboard#HORIZONTAL` | 1 |
| `org.bukkit.entity.Display$Billboard#values` | 1 |
| `org.bukkit.entity.Player#getTargetBlockExact` | 1 |
| `org.bukkit.event.player.PlayerResourcePackStatusEvent#getID` | 1 |
| `org.bukkit.event.player.PlayerResourcePackStatusEvent#getStatus` | 1 |
| `org.bukkit.event.player.PlayerResourcePackStatusEvent$Status#ACCEPTED` | 1 |
| `org.bukkit.inventory.EquipmentSlot#BODY` | 1 |
| `org.bukkit.inventory.EquipmentSlot#SADDLE` | 1 |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| NMS (net.minecraft) | `net.minecraft` | de.oliver.fancysitula | 5 | yes |
| NMS (net.minecraft) | `net.minecraft.core` | de.oliver.fancysitula | 96 | yes |
| NMS (net.minecraft) | `net.minecraft.core.registries` | de.oliver.fancysitula | 28 | yes |
| NMS (net.minecraft) | `net.minecraft.nbt` | de.oliver.fancysitula | 36 | yes |
| NMS (net.minecraft) | `net.minecraft.network` | de.oliver.fancysitula | 6 | yes |
| NMS (net.minecraft) | `net.minecraft.network.chat` | de.oliver.fancysitula | 53 | yes |
| NMS (net.minecraft) | `net.minecraft.network.protocol` | de.oliver.fancysitula | 252 | yes |
| NMS (net.minecraft) | `net.minecraft.network.syncher` | de.oliver.fancysitula | 21 | yes |
| NMS (net.minecraft) | `net.minecraft.resources` | de.oliver.fancysitula | 40 | yes |
| NMS (net.minecraft) | `net.minecraft.server` | de.oliver.fancysitula | 7 | yes |
| NMS (net.minecraft) | `net.minecraft.server.dedicated` | de.oliver.fancysitula | 7 | yes |
| NMS (net.minecraft) | `net.minecraft.server.dialog` | de.oliver.fancysitula | 138 | yes |
| NMS (net.minecraft) | `net.minecraft.server.level` | de.oliver.fancysitula | 116 | yes |
| NMS (net.minecraft) | `net.minecraft.server.network` | de.oliver.fancysitula | 109 | yes |
| NMS (net.minecraft) | `net.minecraft.world.entity` | de.oliver.fancysitula | 292 | yes |
| NMS (net.minecraft) | `net.minecraft.world.item` | de.oliver.fancysitula | 31 | yes |
| NMS (net.minecraft) | `net.minecraft.world.level` | de.oliver.fancysitula | 14 | yes |
| NMS (net.minecraft) | `net.minecraft.world.phys` | de.oliver.fancysitula | 14 | yes |
| NMS (net.minecraft) | `net.minecraft.world.scores` | de.oliver.fancysitula | 37 | yes |
| CraftBukkit | `org.bukkit.craftbukkit` | de.oliver.fancysitula | 7 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.block` | de.oliver.fancysitula | 7 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.entity` | de.oliver.fancysitula | 13 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.inventory` | de.oliver.fancysitula | 14 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.util` | de.oliver.fancysitula | 7 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `com.mojang.authlib` | de.oliver.fancysitula | 18 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `com.mojang.authlib.properties` | de.oliver.fancysitula | 14 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `com.mojang.datafixers.util` | de.oliver.fancysitula | 11 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `io.netty.channel` | de.oliver.fancysitula | 24 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `io.netty.handler.codec` | de.oliver.fancysitula | 6 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `io.papermc.paper.adventure` | de.oliver.fancysitula | 33 | yes |

## 🔴 FancyNpcs 2.12.1

- file: `fancynpcs--FancyNpcs-2.12.1.jar`
- main: `de.oliver.fancynpcs.FancyNpcs`, api-version: 1.19
- softdepend: Citizens, MiniPlaceholders, PlaceholderAPI, PlotSquared
- classes: 4579 (430 own under `de.oliver.fancynpcs`, 4149 bundled), reflection calls: 215
- NMS (net.minecraft): net.minecraft, net.minecraft.core, net.minecraft.core.registries (3128 refs, in de.oliver.fancysitula, own)
- CraftBukkit: org.bukkit.craftbukkit, org.bukkit.craftbukkit.block, org.bukkit.craftbukkit.entity (83 refs, in de.oliver.fancysitula, own)
- server internals (Mojang libs, Netty, Paper internals): com.mojang.authlib, com.mojang.authlib.properties, com.mojang.datafixers.util (176 refs, in de.oliver.fancysitula, own)
- reflection into internals: Craft* class name, getHandle(), net.minecraft. (294 refs, in de.oliver.cloud, de.oliver.fancysitula)
- uses 76 uncovered API members and 41 uncovered API types (top: org.bukkit.entity.Player#performCommand, org.bukkit.Registry#MATERIAL, org.bukkit.Registry#stream, io.papermc.paper.command.brigadier.CommandSourceStack#getSender, org.bukkit.Material#isBlock)

| Uncovered member | Sites |
|---|---:|
| `org.bukkit.entity.Player#performCommand` | 23 |
| `org.bukkit.Registry#MATERIAL` | 9 |
| `org.bukkit.Registry#stream` | 8 |
| `io.papermc.paper.command.brigadier.CommandSourceStack#getSender` | 7 |
| `org.bukkit.Material#isBlock` | 7 |
| `org.bukkit.entity.Display$Billboard#values` | 7 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#getBuffer` | 4 |
| `com.destroystokyo.paper.event.brigadier.CommandRegisteredEvent#getCommandLabel` | 3 |
| `com.destroystokyo.paper.profile.PlayerProfile#getName` | 3 |
| `com.destroystokyo.paper.profile.ProfileProperty#getName` | 3 |
| `org.bukkit.Bukkit#isStopping` | 3 |
| `org.bukkit.Material#isItem` | 3 |
| `org.bukkit.command.BlockCommandSender#getBlock` | 3 |
| `org.bukkit.command.PluginIdentifiableCommand#getPlugin` | 3 |
| `org.bukkit.entity.Player#canSee` | 3 |
| `org.bukkit.util.RayTraceResult#getHitPosition` | 3 |
| `com.destroystokyo.paper.event.brigadier.CommandRegisteredEvent#getCommand` | 2 |
| `com.destroystokyo.paper.event.player.PlayerUseUnknownEntityEvent#isAttack` | 2 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent$Completion#completion` | 2 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent$Completion#completion` | 2 |
| `com.destroystokyo.paper.profile.PlayerProfile#getProperties` | 2 |
| `com.destroystokyo.paper.profile.ProfileProperty#getSignature` | 2 |
| `com.destroystokyo.paper.profile.ProfileProperty#getValue` | 2 |
| `io.papermc.paper.command.brigadier.Commands#getDispatcher` | 2 |
| `io.papermc.paper.plugin.lifecycle.event.LifecycleEventManager#registerEventHandler` | 2 |
| `io.papermc.paper.plugin.lifecycle.event.types.LifecycleEvents#COMMANDS` | 2 |
| `io.papermc.paper.registry.RegistryAccess#getRegistry` | 2 |
| `io.papermc.paper.registry.RegistryAccess#registryAccess` | 2 |
| `org.bukkit.Material#getMaxStackSize` | 2 |
| `org.bukkit.Registry#get` | 2 |
| `org.bukkit.inventory.ItemStack#asHoverEvent` | 2 |
| `com.destroystokyo.paper.brigadier.BukkitBrigadierCommandSource#getBukkitSender` | 1 |
| `com.destroystokyo.paper.event.brigadier.CommandRegisteredEvent#getLiteral` | 1 |
| `com.destroystokyo.paper.event.brigadier.CommandRegisteredEvent#setLiteral` | 1 |
| `com.destroystokyo.paper.event.player.PlayerUseUnknownEntityEvent#getClickedRelativePosition` | 1 |
| `com.destroystokyo.paper.event.player.PlayerUseUnknownEntityEvent#getEntityId` | 1 |
| `com.destroystokyo.paper.event.player.PlayerUseUnknownEntityEvent#getHand` | 1 |
| `com.destroystokyo.paper.event.player.PlayerUseUnknownEntityEvent#getPlayer` | 1 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#completions` | 1 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#getSender` | 1 |
| ... 36 more | |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| NMS (net.minecraft) | `net.minecraft` | de.oliver.fancysitula | 5 | yes |
| NMS (net.minecraft) | `net.minecraft` | own | 12 | yes |
| NMS (net.minecraft) | `net.minecraft.core` | de.oliver.fancysitula | 96 | yes |
| NMS (net.minecraft) | `net.minecraft.core` | own | 301 | yes |
| NMS (net.minecraft) | `net.minecraft.core.registries` | de.oliver.fancysitula | 28 | yes |
| NMS (net.minecraft) | `net.minecraft.core.registries` | own | 63 | yes |
| NMS (net.minecraft) | `net.minecraft.nbt` | de.oliver.fancysitula | 36 | yes |
| NMS (net.minecraft) | `net.minecraft.network` | de.oliver.fancysitula | 6 | yes |
| NMS (net.minecraft) | `net.minecraft.network.chat` | de.oliver.fancysitula | 53 | yes |
| NMS (net.minecraft) | `net.minecraft.network.chat` | own | 42 | yes |
| NMS (net.minecraft) | `net.minecraft.network.protocol` | de.oliver.fancysitula | 252 | yes |
| NMS (net.minecraft) | `net.minecraft.network.protocol` | own | 129 | yes |
| NMS (net.minecraft) | `net.minecraft.network.syncher` | de.oliver.fancysitula | 21 | yes |
| NMS (net.minecraft) | `net.minecraft.network.syncher` | own | 84 | yes |
| NMS (net.minecraft) | `net.minecraft.resources` | de.oliver.fancysitula | 40 | yes |
| NMS (net.minecraft) | `net.minecraft.resources` | own | 105 | yes |
| NMS (net.minecraft) | `net.minecraft.server` | de.oliver.fancysitula | 7 | yes |
| NMS (net.minecraft) | `net.minecraft.server` | own | 49 | yes |
| NMS (net.minecraft) | `net.minecraft.server.dedicated` | de.oliver.fancysitula | 7 | yes |
| NMS (net.minecraft) | `net.minecraft.server.dedicated` | own | 7 | yes |
| NMS (net.minecraft) | `net.minecraft.server.dialog` | de.oliver.fancysitula | 138 | yes |
| NMS (net.minecraft) | `net.minecraft.server.level` | de.oliver.fancysitula | 116 | yes |
| NMS (net.minecraft) | `net.minecraft.server.level` | own | 55 | yes |
| NMS (net.minecraft) | `net.minecraft.server.network` | de.oliver.fancysitula | 109 | yes |
| NMS (net.minecraft) | `net.minecraft.server.network` | own | 7 | yes |
| NMS (net.minecraft) | `net.minecraft.world` | own | 7 | yes |
| NMS (net.minecraft) | `net.minecraft.world.entity` | de.oliver.fancysitula | 292 | yes |
| NMS (net.minecraft) | `net.minecraft.world.entity` | own | 797 | yes |
| NMS (net.minecraft) | `net.minecraft.world.item` | de.oliver.fancysitula | 31 | yes |
| NMS (net.minecraft) | `net.minecraft.world.item` | own | 81 | yes |

## 🔴 FastAsyncWorldEdit 2.16.2+eef5a6f

- file: `fastasyncworldedit--FastAsyncWorldEdit-Paper-2.16.2.jar`
- main: `com.sk89q.worldedit.bukkit.WorldEditPlugin`, api-version: 1.21
- softdepend: Vault
- classes: 3747 (2476 own under `com.sk89q.worldedit`, 1271 bundled), reflection calls: 933
- NMS (net.minecraft): net.minecraft, net.minecraft.core, net.minecraft.core.component (6395 refs, in own)
- CraftBukkit: org.bukkit.craftbukkit, org.bukkit.craftbukkit.block, org.bukkit.craftbukkit.entity (337 refs, in own)
- server internals (Mojang libs, Netty, Paper internals): ca.spottedleaf.moonrise.patches, com.mojang.authlib, com.mojang.datafixers (476 refs, in own)
- reflection into internals: Craft* class name, getHandle(), net.minecraft.nbt (10 refs, in com.fastasyncworldedit.bukkit, com.fastasyncworldedit.core, own)
- Unsafe / jdk.internal: sun.misc (16 refs, in own)
- uses 130 uncovered API members and 81 uncovered API types (top: org.bukkit.event.entity.CreatureSpawnEvent$SpawnReason#CUSTOM, org.bukkit.World#getGenerator, org.bukkit.event.block.BlockPhysicsEvent#<init>, org.bukkit.event.block.BlockPhysicsEvent#isCancelled, org.bukkit.World#getBiomeProvider)

| Uncovered member | Sites |
|---|---:|
| `org.bukkit.event.entity.CreatureSpawnEvent$SpawnReason#CUSTOM` | 27 |
| `org.bukkit.World#getGenerator` | 18 |
| `org.bukkit.event.block.BlockPhysicsEvent#<init>` | 18 |
| `org.bukkit.event.block.BlockPhysicsEvent#isCancelled` | 18 |
| `org.bukkit.World#getBiomeProvider` | 10 |
| `io.papermc.paper.event.block.BeaconDeactivatedEvent#<init>` | 9 |
| `io.papermc.paper.event.block.BeaconDeactivatedEvent#callEvent` | 9 |
| `io.papermc.paper.util.MCUtil#MAIN_EXECUTOR` | 9 |
| `org.bukkit.event.block.BlockPhysicsEvent#setCancelled` | 9 |
| `org.spigotmc.WatchdogThread#tick` | 9 |
| `io.papermc.paper.world.PaperWorldLoader$LoadedWorldData#<init>` | 6 |
| `io.papermc.paper.world.saveddata.PaperWorldPDC#<init>` | 6 |
| `org.bukkit.World#getPersistentDataContainer` | 6 |
| `org.bukkit.Material#isBlock` | 5 |
| `org.bukkit.help.HelpTopic#fullText` | 5 |
| `org.bukkit.Tag#getKey` | 4 |
| `org.bukkit.event.block.BlockPhysicsEvent#getBlock` | 4 |
| `org.bukkit.Registry#get` | 3 |
| `org.bukkit.World#getEntities` | 3 |
| `org.bukkit.command.BlockCommandSender#getName` | 3 |
| `org.bukkit.event.entity.EntityChangeBlockEvent#setCancelled` | 3 |
| `org.bukkit.event.entity.ItemSpawnEvent#setCancelled` | 3 |
| `org.bukkit.help.HelpTopic#shortText` | 3 |
| `co.aikar.timings.Timings#setTimingsEnabled` | 2 |
| `org.bukkit.Bukkit#getTag` | 2 |
| `org.bukkit.Bukkit#getTags` | 2 |
| `org.bukkit.Material#isBurnable` | 2 |
| `org.bukkit.Material#isItem` | 2 |
| `org.bukkit.Registry#MATERIAL` | 2 |
| `org.bukkit.Tag#getValues` | 2 |
| `org.bukkit.TreeType#BROWN_MUSHROOM` | 2 |
| `org.bukkit.TreeType#CHORUS_PLANT` | 2 |
| `org.bukkit.TreeType#REDWOOD` | 2 |
| `org.bukkit.World#getMaxHeight` | 2 |
| `org.bukkit.World#getMinHeight` | 2 |
| `org.bukkit.World#refreshChunk` | 2 |
| `org.bukkit.block.Biome#name` | 2 |
| `org.bukkit.block.Biome#values` | 2 |
| `org.bukkit.command.BlockCommandSender#getBlock` | 2 |
| `org.bukkit.entity.Entity#isDead` | 2 |
| ... 90 more | |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| NMS (net.minecraft) | `net.minecraft` | own | 27 | yes |
| NMS (net.minecraft) | `net.minecraft.core` | own | 846 | yes |
| NMS (net.minecraft) | `net.minecraft.core.component` | own | 18 | yes |
| NMS (net.minecraft) | `net.minecraft.core.particles` | own | 5 | yes |
| NMS (net.minecraft) | `net.minecraft.core.registries` | own | 62 | yes |
| NMS (net.minecraft) | `net.minecraft.data.worldgen` | own | 63 | yes |
| NMS (net.minecraft) | `net.minecraft.nbt` | own | 1064 | yes |
| NMS (net.minecraft) | `net.minecraft.network.chat` | own | 95 | yes |
| NMS (net.minecraft) | `net.minecraft.network.protocol` | own | 81 | yes |
| NMS (net.minecraft) | `net.minecraft.resources` | own | 203 | yes |
| NMS (net.minecraft) | `net.minecraft.server` | own | 114 | yes |
| NMS (net.minecraft) | `net.minecraft.server.dedicated` | own | 60 | yes |
| NMS (net.minecraft) | `net.minecraft.server.level` | own | 458 | yes |
| NMS (net.minecraft) | `net.minecraft.server.network` | own | 27 | yes |
| NMS (net.minecraft) | `net.minecraft.sounds` | own | 28 | yes |
| NMS (net.minecraft) | `net.minecraft.stats` | own | 9 | yes |
| NMS (net.minecraft) | `net.minecraft.tags` | own | 27 | yes |
| NMS (net.minecraft) | `net.minecraft.util` | own | 267 | yes |
| NMS (net.minecraft) | `net.minecraft.util.datafix` | own | 27 | yes |
| NMS (net.minecraft) | `net.minecraft.util.thread` | own | 9 | yes |
| NMS (net.minecraft) | `net.minecraft.world` | own | 100 | yes |
| NMS (net.minecraft) | `net.minecraft.world.damagesource` | own | 9 | yes |
| NMS (net.minecraft) | `net.minecraft.world.entity` | own | 165 | yes |
| NMS (net.minecraft) | `net.minecraft.world.flag` | own | 9 | yes |
| NMS (net.minecraft) | `net.minecraft.world.item` | own | 72 | yes |
| NMS (net.minecraft) | `net.minecraft.world.level` | own | 2435 | yes |
| NMS (net.minecraft) | `net.minecraft.world.phys` | own | 108 | yes |
| NMS (net.minecraft) | `net.minecraft.world.ticks` | own | 7 | yes |
| CraftBukkit | `org.bukkit.craftbukkit` | own | 170 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.block` | own | 73 | yes |

## 🔴 Geyser-Spigot 2.12.1-SNAPSHOT

- file: `geyser--Geyser-Spigot.jar`
- main: `org.geysermc.geyser.platform.spigot.GeyserSpigotPlugin`, api-version: 1.13
- softdepend: ViaVersion, floodgate
- classes: 17788 (1733 own under `org.geysermc.geyser`, 16055 bundled), reflection calls: 349
- NMS (net.minecraft): net.minecraft.core, net.minecraft.core.component, net.minecraft.core.registries (226 refs, in org.geysermc.erosion, own)
- CraftBukkit: org.bukkit.craftbukkit, org.bukkit.craftbukkit.block, org.bukkit.craftbukkit.v1_17_R1 (18 refs, in org.geysermc.erosion, own)
- server internals (Mojang libs, Netty, Paper internals): io.netty.bootstrap, io.netty.buffer, io.netty.channel (3452 refs, in io.netty.handler, net.lenni0451.commons, org.cloudburstmc.netty)
- reflection into internals: Craft* class name, getHandle(), io.netty.channel (52 refs, in org.geysermc.erosion, org.geysermc.geyser.platform.spigot.shaded.me.lucko, org.geysermc.geyser.platform.spigot.shaded.org.incendo)
- uses 75 uncovered API members and 41 uncovered API types (top: org.bukkit.World#getMinHeight, io.papermc.paper.command.brigadier.CommandSourceStack#getSender, com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#getBuffer, com.destroystokyo.paper.event.brigadier.CommandRegisteredEvent#getCommandLabel, org.bukkit.Bukkit#getUnsafe)

| Uncovered member | Sites |
|---|---:|
| `org.bukkit.World#getMinHeight` | 32 |
| `io.papermc.paper.command.brigadier.CommandSourceStack#getSender` | 7 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#getBuffer` | 4 |
| `com.destroystokyo.paper.event.brigadier.CommandRegisteredEvent#getCommandLabel` | 3 |
| `org.bukkit.Bukkit#getUnsafe` | 3 |
| `org.bukkit.UnsafeValues#getProtocolVersion` | 3 |
| `org.bukkit.command.BlockCommandSender#getBlock` | 3 |
| `org.bukkit.command.PluginIdentifiableCommand#getPlugin` | 3 |
| `org.bukkit.entity.Player#canSee` | 3 |
| `org.bukkit.event.block.BlockPistonEvent#getBlock` | 3 |
| `com.destroystokyo.paper.event.brigadier.CommandRegisteredEvent#getCommand` | 2 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent$Completion#completion` | 2 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent$Completion#completion` | 2 |
| `io.papermc.paper.command.brigadier.Commands#getDispatcher` | 2 |
| `io.papermc.paper.plugin.lifecycle.event.LifecycleEventManager#registerEventHandler` | 2 |
| `io.papermc.paper.plugin.lifecycle.event.types.LifecycleEvents#COMMANDS` | 2 |
| `io.papermc.paper.registry.RegistryAccess#getRegistry` | 2 |
| `io.papermc.paper.registry.RegistryAccess#registryAccess` | 2 |
| `org.bukkit.Bukkit#getIp` | 2 |
| `org.bukkit.Bukkit#getPort` | 2 |
| `org.bukkit.Bukkit#getViewDistance` | 2 |
| `org.bukkit.Material#getMaxStackSize` | 2 |
| `org.bukkit.block.Banner#numberOfPatterns` | 2 |
| `org.bukkit.event.block.BlockPistonEvent#isCancelled` | 2 |
| `org.bukkit.event.player.PlayerCommandSendEvent#getCommands` | 2 |
| `com.destroystokyo.paper.brigadier.BukkitBrigadierCommandSource#getBukkitSender` | 1 |
| `com.destroystokyo.paper.event.brigadier.AsyncPlayerSendCommandsEvent#getCommandNode` | 1 |
| `com.destroystokyo.paper.event.brigadier.AsyncPlayerSendCommandsEvent#getPlayer` | 1 |
| `com.destroystokyo.paper.event.brigadier.AsyncPlayerSendCommandsEvent#hasFiredAsync` | 1 |
| `com.destroystokyo.paper.event.brigadier.AsyncPlayerSendCommandsEvent#isAsynchronous` | 1 |
| `com.destroystokyo.paper.event.brigadier.CommandRegisteredEvent#getLiteral` | 1 |
| `com.destroystokyo.paper.event.brigadier.CommandRegisteredEvent#setLiteral` | 1 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#completions` | 1 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#getSender` | 1 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#setCompletions` | 1 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#setHandled` | 1 |
| `com.destroystokyo.paper.event.server.PaperServerListPingEvent#isCancelled` | 1 |
| `io.papermc.paper.brigadier.PaperBrigadier#componentFromMessage` | 1 |
| `io.papermc.paper.command.brigadier.CommandRegistrationFlag#FLATTEN_ALIASES` | 1 |
| `io.papermc.paper.command.brigadier.CommandSourceStack#getExecutor` | 1 |
| ... 35 more | |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| NMS (net.minecraft) | `net.minecraft.core` | org.geysermc.erosion | 3 | yes |
| NMS (net.minecraft) | `net.minecraft.core` | own | 60 | yes |
| NMS (net.minecraft) | `net.minecraft.core.component` | org.geysermc.erosion | 3 | yes |
| NMS (net.minecraft) | `net.minecraft.core.registries` | org.geysermc.erosion | 1 | yes |
| NMS (net.minecraft) | `net.minecraft.core.registries` | own | 12 | yes |
| NMS (net.minecraft) | `net.minecraft.network` | org.geysermc.erosion | 1 | yes |
| NMS (net.minecraft) | `net.minecraft.network.codec` | org.geysermc.erosion | 1 | yes |
| NMS (net.minecraft) | `net.minecraft.resources` | own | 24 | yes |
| NMS (net.minecraft) | `net.minecraft.server` | own | 12 | yes |
| NMS (net.minecraft) | `net.minecraft.server.level` | org.geysermc.erosion | 1 | yes |
| NMS (net.minecraft) | `net.minecraft.server.level` | own | 16 | yes |
| NMS (net.minecraft) | `net.minecraft.tags` | own | 12 | yes |
| NMS (net.minecraft) | `net.minecraft.world.item` | org.geysermc.erosion | 1 | yes |
| NMS (net.minecraft) | `net.minecraft.world.level` | org.geysermc.erosion | 3 | yes |
| NMS (net.minecraft) | `net.minecraft.world.level` | own | 76 | yes |
| CraftBukkit | `org.bukkit.craftbukkit` | org.geysermc.erosion | 1 | yes |
| CraftBukkit | `org.bukkit.craftbukkit` | own | 3 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.block` | org.geysermc.erosion | 1 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_17_R1` | own | 1 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_18_R1` | own | 1 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_18_R2` | own | 1 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_19_R1` | own | 1 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_19_R2` | own | 1 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_19_R3` | own | 1 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_20_R1` | own | 1 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_20_R2` | own | 1 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_20_R3` | own | 1 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_20_R4` | own | 1 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_21_R1` | own | 1 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_21_R2` | own | 1 | yes |

## 🟡 GriefPrevention 16.18.8

- file: `griefprevention--GriefPrevention.jar`
- main: `me.ryanhamshire.GriefPrevention.GriefPrevention`, api-version: 26.3
- softdepend: Vault, Multiverse-Core, My_Worlds, MystCraft, Transporter, WorldGuard, WorldEdit, RoyalCommands, MultiWorld, Denizen, CommandHelper, Iris
- classes: 130 (117 own under `me.ryanhamshire.GriefPrevention`, 13 bundled), reflection calls: 11
- uses 363 uncovered API members and 138 uncovered API types (top: org.bukkit.Tag#getValues, org.bukkit.Tag#isTagged, org.bukkit.event.entity.EntityChangeBlockEvent#setCancelled, org.bukkit.event.entity.EntityCombustByEntityEvent#getEntity, org.bukkit.entity.Projectile#getShooter)

| Uncovered member | Sites |
|---|---:|
| `org.bukkit.Tag#getValues` | 38 |
| `org.bukkit.Tag#isTagged` | 30 |
| `org.bukkit.event.entity.EntityChangeBlockEvent#setCancelled` | 16 |
| `org.bukkit.event.entity.EntityCombustByEntityEvent#getEntity` | 15 |
| `org.bukkit.entity.Projectile#getShooter` | 12 |
| `org.bukkit.event.entity.EntityChangeBlockEvent#getBlock` | 10 |
| `org.bukkit.event.entity.EntityChangeBlockEvent#getEntity` | 10 |
| `org.bukkit.OfflinePlayer#getLastPlayed` | 9 |
| `org.bukkit.Tag#LEAVES` | 9 |
| `org.bukkit.event.entity.EntityCombustByEntityEvent#getCombuster` | 9 |
| `org.bukkit.World#getMaxHeight` | 8 |
| `org.bukkit.entity.Tameable#isTamed` | 8 |
| `org.bukkit.event.entity.EntityCombustByEntityEvent#setCancelled` | 8 |
| `org.bukkit.Tag#LOGS` | 7 |
| `org.bukkit.World#getMinHeight` | 7 |
| `org.bukkit.entity.Tameable#getOwner` | 7 |
| `org.bukkit.event.entity.EntityChangeBlockEvent#getEntityType` | 7 |
| `org.bukkit.Statistic#PICKUP` | 6 |
| `org.bukkit.entity.Player#getStatistic` | 6 |
| `org.bukkit.event.block.BlockIgniteEvent#getCause` | 6 |
| `org.bukkit.event.block.BlockIgniteEvent#getIgnitingEntity` | 6 |
| `io.papermc.paper.event.entity.EntityBreakEvent$RemoveCause#ordinal` | 5 |
| `org.bukkit.block.BlockState#getLocation` | 5 |
| `org.bukkit.block.BlockState#getType` | 5 |
| `org.bukkit.event.block.BlockIgniteEvent#getBlock` | 5 |
| `org.bukkit.event.block.BlockPistonEvent#setCancelled` | 5 |
| `org.bukkit.event.block.BlockSpreadEvent#getSource` | 5 |
| `org.bukkit.event.player.PlayerBucketEmptyEvent#getBucket` | 5 |
| `org.bukkit.entity.Projectile#remove` | 4 |
| `org.bukkit.event.block.BlockIgniteEvent#setCancelled` | 4 |
| `org.bukkit.event.hanging.HangingPlaceEvent#getPlayer` | 4 |
| `org.bukkit.event.player.PlayerBucketEmptyEvent#getBlockClicked` | 4 |
| `org.bukkit.event.player.PlayerSignOpenEvent#getSign` | 4 |
| `org.bukkit.Chunk#getEntities` | 3 |
| `org.bukkit.Tag#FENCE_GATES` | 3 |
| `org.bukkit.block.Biome#getKey` | 3 |
| `org.bukkit.entity.AnimalTamer#getUniqueId` | 3 |
| `org.bukkit.entity.Hanging#getLocation` | 3 |
| `org.bukkit.entity.Item#getMetadata` | 3 |
| `org.bukkit.entity.Player#sendBlockChange` | 3 |
| ... 323 more | |

## 🔴 GrimAC 2.3.74

- file: `grimanticheat--grimac-bukkit-2.3.74.jar`
- main: `ac.grim.grimac.platform.bukkit.GrimACBukkitLoaderPlugin`, api-version: 1.13
- softdepend: ProtocolLib, ProtocolSupport, Essentials, ViaVersion, ViaBackwards, ViaRewind, Geyser-Spigot, floodgate, FastLogin, PlaceholderAPI, LuckPerms, sqlite-jdbc, mysql-jdbc, postgresql-jdbc, mongodb-driver, jedis
- classes: 6021 (1279 own under `ac.grim.grimac`, 4742 bundled), reflection calls: 341
- server internals (Mojang libs, Netty, Paper internals): io.netty.buffer, io.netty.channel, io.netty.handler.codec (89 refs, in ac.grim.grimac.shaded.io.github, ac.grim.grimac.shaded.kyori.adventure, own)
- reflection into internals: Craft* class name, com.mojang.authlib, getHandle() (64 refs, in ac.grim.grimac.shaded.incendo.cloud, ac.grim.grimac.shaded.io.github, ac.grim.grimac.shaded.kyori.adventure)
- Unsafe / jdk.internal: sun.misc (1 refs, in own)
- uses 78 uncovered API members and 38 uncovered API types (top: com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#getBuffer, org.bukkit.entity.Player#canSee, com.destroystokyo.paper.event.brigadier.CommandRegisteredEvent#getCommandLabel, org.bukkit.Material#getId, org.bukkit.command.BlockCommandSender#getBlock)

| Uncovered member | Sites |
|---|---:|
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#getBuffer` | 4 |
| `org.bukkit.entity.Player#canSee` | 4 |
| `com.destroystokyo.paper.event.brigadier.CommandRegisteredEvent#getCommandLabel` | 3 |
| `org.bukkit.Material#getId` | 3 |
| `org.bukkit.command.BlockCommandSender#getBlock` | 3 |
| `org.bukkit.command.PluginIdentifiableCommand#getPlugin` | 3 |
| `com.destroystokyo.paper.event.brigadier.CommandRegisteredEvent#getCommand` | 2 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent$Completion#completion` | 2 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent$Completion#completion` | 2 |
| `io.papermc.paper.command.brigadier.Commands#getDispatcher` | 2 |
| `io.papermc.paper.event.connection.PlayerConnectionValidateLoginEvent#getConnection` | 2 |
| `io.papermc.paper.plugin.lifecycle.event.LifecycleEventManager#registerEventHandler` | 2 |
| `io.papermc.paper.plugin.lifecycle.event.types.LifecycleEvents#COMMANDS` | 2 |
| `org.bukkit.Material#getMaxStackSize` | 2 |
| `org.bukkit.World#getUID` | 2 |
| `org.bukkit.World$Environment#getId` | 2 |
| `org.bukkit.entity.EntityType#getTypeId` | 2 |
| `org.bukkit.entity.Player#getVehicle` | 2 |
| `com.destroystokyo.paper.Title#builder` | 1 |
| `com.destroystokyo.paper.Title$Builder#build` | 1 |
| `com.destroystokyo.paper.Title$Builder#fadeIn` | 1 |
| `com.destroystokyo.paper.Title$Builder#fadeOut` | 1 |
| `com.destroystokyo.paper.Title$Builder#stay` | 1 |
| `com.destroystokyo.paper.Title$Builder#subtitle` | 1 |
| `com.destroystokyo.paper.Title$Builder#title` | 1 |
| `com.destroystokyo.paper.brigadier.BukkitBrigadierCommandSource#getBukkitSender` | 1 |
| `com.destroystokyo.paper.event.brigadier.CommandRegisteredEvent#getLiteral` | 1 |
| `com.destroystokyo.paper.event.brigadier.CommandRegisteredEvent#setLiteral` | 1 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#completions` | 1 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#getSender` | 1 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#setCompletions` | 1 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#setHandled` | 1 |
| `io.papermc.paper.brigadier.PaperBrigadier#componentFromMessage` | 1 |
| `io.papermc.paper.command.brigadier.CommandRegistrationFlag#FLATTEN_ALIASES` | 1 |
| `io.papermc.paper.command.brigadier.CommandSourceStack#getSender` | 1 |
| `io.papermc.paper.command.brigadier.Commands#getClass` | 1 |
| `io.papermc.paper.command.brigadier.Commands#registerWithFlags` | 1 |
| `io.papermc.paper.command.brigadier.MessageComponentSerializer#deserializeOrNull` | 1 |
| `io.papermc.paper.command.brigadier.MessageComponentSerializer#message` | 1 |
| `io.papermc.paper.event.connection.PlayerConnectionValidateLoginEvent#isAllowed` | 1 |
| ... 38 more | |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| server internals (Mojang libs, Netty, Paper internals) | `io.netty.buffer` | ac.grim.grimac.shaded.io.github | 13 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `io.netty.buffer` | ac.grim.grimac.shaded.kyori.adventure | 5 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `io.netty.buffer` | own | 3 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `io.netty.channel` | ac.grim.grimac.shaded.io.github | 56 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `io.netty.channel` | own | 2 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `io.netty.handler.codec` | ac.grim.grimac.shaded.io.github | 1 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `io.netty.util` | ac.grim.grimac.shaded.io.github | 2 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `io.netty.util.concurrent` | ac.grim.grimac.shaded.io.github | 4 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `io.netty.util.concurrent` | own | 1 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `io.netty.util.internal` | ac.grim.grimac.shaded.io.github | 2 | yes |
| reflection into internals | `Craft* class name` | ac.grim.grimac.shaded.incendo.cloud | 5 | yes |
| reflection into internals | `Craft* class name` | ac.grim.grimac.shaded.io.github | 9 | yes |
| reflection into internals | `Craft* class name` | ac.grim.grimac.shaded.kyori.adventure | 9 | yes |
| reflection into internals | `Craft* class name` | own | 2 | yes |
| reflection into internals | `com.mojang.authlib` | ac.grim.grimac.shaded.io.github | 2 | yes |
| reflection into internals | `getHandle()` | ac.grim.grimac.shaded.incendo.cloud | 1 | yes |
| reflection into internals | `getHandle()` | ac.grim.grimac.shaded.io.github | 1 | yes |
| reflection into internals | `getHandle()` | ac.grim.grimac.shaded.kyori.adventure | 2 | yes |
| reflection into internals | `getHandle()` | own | 2 | yes |
| reflection into internals | `net.minecraft.` | ac.grim.grimac.shaded.incendo.cloud | 1 | yes |
| reflection into internals | `net.minecraft.` | ac.grim.grimac.shaded.io.github | 1 | yes |
| reflection into internals | `net.minecraft.` | ac.grim.grimac.shaded.kyori.adventure | 1 | yes |
| reflection into internals | `net.minecraft.SharedConstants` | ac.grim.grimac.shaded.incendo.cloud | 1 | yes |
| reflection into internals | `net.minecraft.commands` | ac.grim.grimac.shaded.incendo.cloud | 1 | yes |
| reflection into internals | `net.minecraft.nbt` | ac.grim.grimac.shaded.incendo.cloud | 2 | yes |
| reflection into internals | `net.minecraft.network` | ac.grim.grimac.shaded.io.github | 5 | yes |
| reflection into internals | `net.minecraft.resources` | ac.grim.grimac.shaded.incendo.cloud | 5 | yes |
| reflection into internals | `net.minecraft.server` | ac.grim.grimac.shaded.incendo.cloud | 3 | yes |
| reflection into internals | `net.minecraft.server` | ac.grim.grimac.shaded.io.github | 2 | yes |
| reflection into internals | `net.minecraft.server` | ac.grim.grimac.shaded.kyori.adventure | 1 | yes |

## 🔴 GSit 3.8.0

- file: `gsit--GSit-3.8.0.jar`
- main: `dev.geco.gsit.GSitMain`, api-version: 1.16
- softdepend: GriefPrevention, PlaceholderAPI, PlotSquared, WorldGuard
- classes: 351 (351 own under `dev.geco.gsit`, 0 bundled), reflection calls: 87
- NMS (net.minecraft): net.minecraft.core, net.minecraft.nbt, net.minecraft.network (2037 refs, in own)
- CraftBukkit: org.bukkit.craftbukkit, org.bukkit.craftbukkit.entity, org.bukkit.craftbukkit.v1_18_R1 (232 refs, in own)
- server internals (Mojang libs, Netty, Paper internals): ca.spottedleaf.moonrise.patches, com.mojang.authlib, com.mojang.authlib.properties (203 refs, in own)
- reflection into internals: getHandle() (1 refs, in own)
- uses 105 uncovered API members and 42 uncovered API types (top: org.bukkit.block.Block#getBoundingBox, org.bukkit.entity.Player#sendBlockChange, org.bukkit.entity.Pose#ordinal, org.bukkit.util.VoxelShape#getBoundingBoxes, org.bukkit.event.entity.EntityMountEvent#getEntity)

| Uncovered member | Sites |
|---|---:|
| `org.bukkit.block.Block#getBoundingBox` | 42 |
| `org.bukkit.entity.Player#sendBlockChange` | 42 |
| `org.bukkit.entity.Pose#ordinal` | 40 |
| `org.bukkit.util.VoxelShape#getBoundingBoxes` | 25 |
| `org.bukkit.event.entity.EntityMountEvent#getEntity` | 23 |
| `org.bukkit.event.entity.EntityMountEvent#getMount` | 23 |
| `org.bukkit.event.player.PlayerToggleSneakEvent#getPlayer` | 22 |
| `org.bukkit.event.player.PlayerToggleSneakEvent#isSneaking` | 22 |
| `org.bukkit.entity.Player#isFlying` | 21 |
| `org.bukkit.Statistic#TIME_SINCE_REST` | 20 |
| `org.bukkit.World#getMinHeight` | 20 |
| `org.bukkit.World#getSimulationDistance` | 20 |
| `org.bukkit.entity.Player#canSee` | 20 |
| `org.bukkit.entity.Player#getFallDistance` | 20 |
| `org.bukkit.entity.Player#getPlayerTime` | 20 |
| `org.bukkit.entity.Player#removeScoreboardTag` | 20 |
| `org.bukkit.entity.Player#setStatistic` | 20 |
| `org.bukkit.entity.Projectile#getShooter` | 20 |
| `org.bukkit.event.entity.EntityToggleSwimEvent#getEntity` | 20 |
| `org.bukkit.event.entity.EntityToggleSwimEvent#setCancelled` | 20 |
| `org.bukkit.event.entity.ProjectileLaunchEvent#getEntity` | 20 |
| `org.bukkit.event.entity.ProjectileLaunchEvent#setCancelled` | 20 |
| `org.bukkit.event.player.PlayerAnimationEvent#getAnimationType` | 20 |
| `org.bukkit.event.player.PlayerAnimationEvent#getPlayer` | 20 |
| `org.bukkit.event.player.PlayerAnimationType#ARM_SWING` | 20 |
| `org.bukkit.event.player.PlayerDropItemEvent#getPlayer` | 20 |
| `org.bukkit.event.player.PlayerDropItemEvent#setCancelled` | 20 |
| `org.bukkit.event.player.PlayerToggleSneakEvent#isAsynchronous` | 20 |
| `org.bukkit.block.Block#isPassable` | 19 |
| `org.spigotmc.event.entity.EntityMountEvent#getEntity` | 19 |
| `org.spigotmc.event.entity.EntityMountEvent#getMount` | 19 |
| `org.bukkit.event.player.PlayerTeleportEvent$TeleportCause#DISMOUNT` | 16 |
| `org.bukkit.event.entity.EntityDismountEvent#getDismounted` | 11 |
| `org.bukkit.event.entity.EntityDismountEvent#getEntity` | 11 |
| `org.spigotmc.event.entity.EntityDismountEvent#getDismounted` | 10 |
| `org.spigotmc.event.entity.EntityDismountEvent#getEntity` | 10 |
| `org.bukkit.entity.Player#getVehicle` | 8 |
| `org.bukkit.entity.Player#isOnGround` | 7 |
| `org.bukkit.entity.Player#isSleeping` | 7 |
| `org.bukkit.entity.Player#isSneaking` | 7 |
| ... 65 more | |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| NMS (net.minecraft) | `net.minecraft.core` | own | 120 | yes |
| NMS (net.minecraft) | `net.minecraft.nbt` | own | 15 | yes |
| NMS (net.minecraft) | `net.minecraft.network` | own | 20 | yes |
| NMS (net.minecraft) | `net.minecraft.network.chat` | own | 12 | yes |
| NMS (net.minecraft) | `net.minecraft.network.protocol` | own | 558 | yes |
| NMS (net.minecraft) | `net.minecraft.network.syncher` | own | 176 | yes |
| NMS (net.minecraft) | `net.minecraft.server` | own | 20 | yes |
| NMS (net.minecraft) | `net.minecraft.server.dedicated` | own | 20 | yes |
| NMS (net.minecraft) | `net.minecraft.server.level` | own | 193 | yes |
| NMS (net.minecraft) | `net.minecraft.server.network` | own | 83 | yes |
| NMS (net.minecraft) | `net.minecraft.world` | own | 2 | yes |
| NMS (net.minecraft) | `net.minecraft.world.damagesource` | own | 20 | yes |
| NMS (net.minecraft) | `net.minecraft.world.effect` | own | 29 | yes |
| NMS (net.minecraft) | `net.minecraft.world.entity` | own | 384 | yes |
| NMS (net.minecraft) | `net.minecraft.world.inventory` | own | 20 | yes |
| NMS (net.minecraft) | `net.minecraft.world.item` | own | 21 | yes |
| NMS (net.minecraft) | `net.minecraft.world.level` | own | 299 | yes |
| NMS (net.minecraft) | `net.minecraft.world.phys` | own | 45 | yes |
| CraftBukkit | `org.bukkit.craftbukkit` | own | 18 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.entity` | own | 18 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_18_R1` | own | 11 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_18_R2` | own | 11 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_19_R1` | own | 22 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_19_R2` | own | 11 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_19_R3` | own | 11 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_20_R1` | own | 11 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_20_R2` | own | 11 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_20_R3` | own | 12 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_20_R4` | own | 12 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_21_R1` | own | 12 | yes |

## 🔴 ImageFrame 2026.1.5.0

- file: `imageframe--ImageFrame-2026.1.5.0.jar`
- main: `com.loohp.imageframe.ImageFrame`, api-version: 1.13
- softdepend: ViaVersion, PlaceholderAPI
- classes: 2552 (226 own under `com.loohp.imageframe`, 2326 bundled), reflection calls: 455
- NMS (net.minecraft): net.minecraft, net.minecraft.core, net.minecraft.core.component (1208 refs, in own)
- CraftBukkit: org.bukkit.craftbukkit, org.bukkit.craftbukkit.entity, org.bukkit.craftbukkit.inventory (294 refs, in own)
- Unsafe / jdk.internal: sun.misc (8 refs, in own)
- uses 129 uncovered API members and 41 uncovered API types (top: org.bukkit.Bukkit#getMap, org.bukkit.map.MapCursor#getType, org.bukkit.map.MapCursor#getCaption, org.bukkit.map.MapCursor#getDirection, org.bukkit.map.MapCursor#getX)

| Uncovered member | Sites |
|---|---:|
| `org.bukkit.Bukkit#getMap` | 65 |
| `org.bukkit.map.MapCursor#getType` | 39 |
| `org.bukkit.map.MapCursor#getCaption` | 38 |
| `org.bukkit.map.MapCursor#getDirection` | 38 |
| `org.bukkit.map.MapCursor#getX` | 38 |
| `org.bukkit.map.MapCursor#getY` | 38 |
| `org.bukkit.entity.ItemFrame#getLocation` | 36 |
| `org.bukkit.map.MapView#getId` | 32 |
| `org.bukkit.entity.ItemFrame#getItem` | 17 |
| `org.bukkit.map.MapView#getRenderers` | 13 |
| `org.bukkit.map.MapCursor$Type#getValue` | 12 |
| `org.bukkit.Rotation#ordinal` | 11 |
| `org.bukkit.map.MapView#removeRenderer` | 11 |
| `org.bukkit.map.MapCursor#<init>` | 10 |
| `org.bukkit.map.MapView#addRenderer` | 9 |
| `org.bukkit.entity.ItemFrame#setItem` | 7 |
| `org.bukkit.map.MapCursor#isVisible` | 7 |
| `org.bukkit.entity.ItemFrame#getAttachedFace` | 6 |
| `org.bukkit.entity.ItemFrame#getFacing` | 6 |
| `org.bukkit.map.MapCursor$Type#name` | 6 |
| `org.bukkit.map.MapCursor$Type#valueOf` | 6 |
| `org.bukkit.entity.Item#getItemStack` | 5 |
| `org.bukkit.entity.Item#getLocation` | 5 |
| `org.bukkit.entity.Item#setItemStack` | 5 |
| `org.bukkit.entity.Item#setVelocity` | 5 |
| `org.bukkit.inventory.EntityEquipment#getItem` | 5 |
| `org.bukkit.inventory.EntityEquipment#getItemInOffHand` | 5 |
| `org.bukkit.Rotation#NONE` | 4 |
| `org.bukkit.Rotation#rotateCounterClockwise` | 4 |
| `org.bukkit.inventory.EntityEquipment#setItem` | 4 |
| `org.bukkit.util.RayTraceResult#getHitPosition` | 4 |
| `org.bukkit.World#getWorldFolder` | 3 |
| `org.bukkit.entity.Item#getWorld` | 3 |
| `org.bukkit.entity.ItemFrame#isValid` | 3 |
| `org.bukkit.map.MapCanvas#setPixel` | 3 |
| `org.bukkit.map.MapView#equals` | 3 |
| `org.bukkit.potion.PotionEffectType#INVISIBILITY` | 3 |
| `org.bukkit.util.RayTraceResult#getHitEntity` | 3 |
| `org.bukkit.Chunk#getEntities` | 2 |
| `org.bukkit.Rotation#name` | 2 |
| ... 89 more | |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| NMS (net.minecraft) | `net.minecraft` | own | 25 | yes |
| NMS (net.minecraft) | `net.minecraft.core` | own | 19 | yes |
| NMS (net.minecraft) | `net.minecraft.core.component` | own | 76 | yes |
| NMS (net.minecraft) | `net.minecraft.nbt` | own | 55 | yes |
| NMS (net.minecraft) | `net.minecraft.network.chat` | own | 78 | yes |
| NMS (net.minecraft) | `net.minecraft.network.protocol` | own | 84 | yes |
| NMS (net.minecraft) | `net.minecraft.network.syncher` | own | 84 | yes |
| NMS (net.minecraft) | `net.minecraft.resources` | own | 30 | yes |
| NMS (net.minecraft) | `net.minecraft.server` | own | 28 | yes |
| NMS (net.minecraft) | `net.minecraft.server.level` | own | 80 | yes |
| NMS (net.minecraft) | `net.minecraft.server.network` | own | 36 | yes |
| NMS (net.minecraft) | `net.minecraft.server.v1_16_R1` | own | 35 | yes |
| NMS (net.minecraft) | `net.minecraft.server.v1_16_R2` | own | 35 | yes |
| NMS (net.minecraft) | `net.minecraft.server.v1_16_R3` | own | 35 | yes |
| NMS (net.minecraft) | `net.minecraft.world.entity` | own | 112 | yes |
| NMS (net.minecraft) | `net.minecraft.world.inventory` | own | 28 | yes |
| NMS (net.minecraft) | `net.minecraft.world.item` | own | 76 | yes |
| NMS (net.minecraft) | `net.minecraft.world.level` | own | 292 | yes |
| CraftBukkit | `org.bukkit.craftbukkit` | own | 5 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.entity` | own | 15 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.inventory` | own | 5 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.libs` | own | 8 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.map` | own | 20 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.util` | own | 5 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_16_R1` | own | 8 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_16_R2` | own | 8 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_16_R3` | own | 8 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_17_R1` | own | 8 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_18_R1` | own | 8 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_18_R2` | own | 8 | yes |

## 🟡 InstantRestock 2.6.7

- file: `infinitevillagertrading--instantrestock_2.6.7.jar`
- main: `me.spartacus04.instantrestock.InstantRestock`, api-version: 1.14
- classes: 945 (19 own under `me.spartacus04.instantrestock`, 926 bundled), reflection calls: 56
- uses 10 uncovered API members and 5 uncovered API types (top: org.bukkit.entity.AbstractVillager#getPersistentDataContainer, org.bukkit.entity.AbstractVillager#getRecipes, org.bukkit.entity.AbstractVillager#getType, org.bukkit.entity.Villager#getPersistentDataContainer, org.bukkit.inventory.MerchantRecipe#setMaxUses)

| Uncovered member | Sites |
|---|---:|
| `org.bukkit.entity.AbstractVillager#getPersistentDataContainer` | 9 |
| `org.bukkit.entity.AbstractVillager#getRecipes` | 3 |
| `org.bukkit.entity.AbstractVillager#getType` | 3 |
| `org.bukkit.entity.Villager#getPersistentDataContainer` | 2 |
| `org.bukkit.inventory.MerchantRecipe#setMaxUses` | 2 |
| `org.bukkit.Bukkit#getOnlineMode` | 1 |
| `org.bukkit.event.entity.VillagerAcquireTradeEvent#getEntity` | 1 |
| `org.bukkit.inventory.MerchantRecipe#getMaxUses` | 1 |
| `org.bukkit.inventory.MerchantRecipe#setDemand` | 1 |
| `org.bukkit.inventory.MerchantRecipe#setUses` | 1 |

## 🔴 InvSeePlusPlus 0.31.22

- file: `invsee--InvSee++.jar`
- main: `com.janboerman.invsee.spigot.InvseePlusPlus`, api-version: 1.16
- softdepend: Vault, PerWorldInventory, Multiverse-Inventories, LuckPerms, GroupManager, BungeePerms, UltraPermissions
- classes: 1101 (1079 own under `com.janboerman.invsee`, 22 bundled), reflection calls: 171
- NMS (net.minecraft): net.minecraft, net.minecraft.core, net.minecraft.nbt (3670 refs, in own)
- CraftBukkit: org.bukkit.craftbukkit, org.bukkit.craftbukkit.entity, org.bukkit.craftbukkit.inventory (860 refs, in own)
- server internals (Mojang libs, Netty, Paper internals): com.mojang.authlib, com.mojang.datafixers.types, com.mojang.datafixers.util (144 refs, in own)
- reflection into internals: org.bukkit.craftbukkit (22 refs, in own)
- uses 119 uncovered API members and 72 uncovered API types (top: org.bukkit.inventory.ItemStack#getMaxStackSize, org.bukkit.block.banner.Pattern#<init>, org.bukkit.entity.HumanEntity#getItemOnCursor, org.bukkit.entity.HumanEntity#setItemOnCursor, org.bukkit.Registry#stream)

| Uncovered member | Sites |
|---|---:|
| `org.bukkit.inventory.ItemStack#getMaxStackSize` | 66 |
| `org.bukkit.block.banner.Pattern#<init>` | 46 |
| `org.bukkit.entity.HumanEntity#getItemOnCursor` | 28 |
| `org.bukkit.entity.HumanEntity#setItemOnCursor` | 28 |
| `org.bukkit.Registry#stream` | 27 |
| `org.bukkit.event.inventory.InventoryOpenEvent#isCancelled` | 25 |
| `org.bukkit.block.banner.PatternType#STRIPE_BOTTOM` | 23 |
| `org.bukkit.block.banner.PatternType#STRIPE_TOP` | 23 |
| `org.bukkit.event.inventory.InventoryOpenEvent#<init>` | 23 |
| `org.bukkit.block.Banner#setBaseColor` | 22 |
| `org.bukkit.block.Banner#setPatterns` | 22 |
| `org.bukkit.inventory.meta.BlockStateMeta#addItemFlags` | 22 |
| `org.bukkit.inventory.meta.BlockStateMeta#getBlockState` | 22 |
| `org.bukkit.inventory.meta.BlockStateMeta#setBlockState` | 22 |
| `org.bukkit.inventory.meta.BlockStateMeta#setDisplayName` | 22 |
| `org.bukkit.event.inventory.InventoryType#ANVIL` | 21 |
| `org.bukkit.event.inventory.InventoryType#CARTOGRAPHY` | 21 |
| `org.bukkit.event.inventory.InventoryType#GRINDSTONE` | 21 |
| `org.bukkit.event.inventory.InventoryType#LOOM` | 21 |
| `org.bukkit.event.inventory.InventoryType#SMITHING` | 21 |
| `org.bukkit.event.inventory.InventoryType#STONECUTTER` | 21 |
| `org.bukkit.event.inventory.InventoryType#SMITHING_NEW` | 18 |
| `org.bukkit.attribute.Attribute#ordinal` | 15 |
| `org.bukkit.Bukkit#getRegistry` | 14 |
| `org.bukkit.event.inventory.InventoryType#CRAFTER` | 14 |
| `org.bukkit.Material#isItem` | 13 |
| `org.bukkit.Registry#MATERIAL` | 13 |
| `org.bukkit.inventory.ItemStack#getTypeId` | 12 |
| `org.bukkit.inventory.Inventory#getViewers` | 10 |
| `org.bukkit.entity.HumanEntity#getEnderChest` | 8 |
| `org.bukkit.attribute.Attribute#GENERIC_MAX_HEALTH` | 5 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#setHandled` | 4 |
| `org.bukkit.attribute.Attribute#GENERIC_MOVEMENT_SPEED` | 4 |
| `org.bukkit.attribute.AttributeInstance#setBaseValue` | 4 |
| `org.bukkit.attribute.AttributeModifier$Operation#ordinal` | 4 |
| `org.bukkit.event.inventory.InventoryOpenEvent#getView` | 4 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#setCompletions` | 3 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#getSender` | 2 |
| `com.destroystokyo.paper.event.server.ServerExceptionEvent#getException` | 2 |
| `com.destroystokyo.paper.exception.ServerPluginException#getCause` | 2 |
| ... 79 more | |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| NMS (net.minecraft) | `net.minecraft` | own | 20 | yes |
| NMS (net.minecraft) | `net.minecraft.core` | own | 204 | yes |
| NMS (net.minecraft) | `net.minecraft.nbt` | own | 218 | yes |
| NMS (net.minecraft) | `net.minecraft.network.chat` | own | 60 | yes |
| NMS (net.minecraft) | `net.minecraft.network.protocol` | own | 80 | yes |
| NMS (net.minecraft) | `net.minecraft.resources` | own | 133 | yes |
| NMS (net.minecraft) | `net.minecraft.server` | own | 50 | yes |
| NMS (net.minecraft) | `net.minecraft.server.dedicated` | own | 120 | yes |
| NMS (net.minecraft) | `net.minecraft.server.level` | own | 202 | yes |
| NMS (net.minecraft) | `net.minecraft.server.network` | own | 20 | yes |
| NMS (net.minecraft) | `net.minecraft.server.players` | own | 9 | yes |
| NMS (net.minecraft) | `net.minecraft.server.v1_12_R1` | own | 133 | yes |
| NMS (net.minecraft) | `net.minecraft.server.v1_16_R3` | own | 141 | yes |
| NMS (net.minecraft) | `net.minecraft.server.v1_8_R3` | own | 123 | yes |
| NMS (net.minecraft) | `net.minecraft.util` | own | 74 | yes |
| NMS (net.minecraft) | `net.minecraft.world` | own | 288 | yes |
| NMS (net.minecraft) | `net.minecraft.world.entity` | own | 519 | yes |
| NMS (net.minecraft) | `net.minecraft.world.inventory` | own | 676 | yes |
| NMS (net.minecraft) | `net.minecraft.world.item` | own | 255 | yes |
| NMS (net.minecraft) | `net.minecraft.world.level` | own | 342 | yes |
| NMS (net.minecraft) | `net.minecraft.world.phys` | own | 3 | yes |
| CraftBukkit | `org.bukkit.craftbukkit` | own | 63 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.entity` | own | 85 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.inventory` | own | 106 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.util` | own | 21 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_12_R1` | own | 35 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_16_R3` | own | 34 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_17_R1` | own | 34 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_18_R2` | own | 34 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_19_R3` | own | 38 | yes |

## 🔴 LagFixer 1.7.2 [build 152]

- file: `lagfixer--LagFixer-1.7.2.jar`
- main: `xyz.lychee.lagfixer.LagFixer`, api-version: 1.13
- softdepend: PlaceholderAPI, spark, LevelledMobs, RoseStacker, UltimateStacker, WildStacker, ModelEngine, MythicMobs, StackMob, BetterModel, packetevents
- classes: 1197 (370 own under `xyz.lychee.lagfixer`, 827 bundled), reflection calls: 143
- NMS (net.minecraft): net.minecraft.core, net.minecraft.nbt, net.minecraft.server.dedicated (2290 refs, in own)
- CraftBukkit: org.bukkit.craftbukkit, org.bukkit.craftbukkit.entity, org.bukkit.craftbukkit.event (161 refs, in own)
- server internals (Mojang libs, Netty, Paper internals): com.mojang.authlib, com.mojang.authlib.properties, com.mojang.serialization (69 refs, in own, xyz.lychee.lagfixer.libs.kyori.adventure)
- ProtocolLib / PacketEvents: com.github.retrooper.packetevents (12 refs, in own)
- reflection into internals: Craft* class name, getHandle(), net.minecraft. (16 refs, in own, xyz.lychee.lagfixer.libs.kyori.adventure)
- uses 154 uncovered API members and 62 uncovered API types (top: org.spigotmc.SpigotWorldConfig#viewDistance, org.bukkit.event.world.EntitiesLoadEvent#getEntities, org.bukkit.event.entity.EntityTargetLivingEntityEvent#isCancelled, org.bukkit.event.world.EntitiesLoadEvent#getWorld, org.spigotmc.SpigotWorldConfig#simulationDistance)

| Uncovered member | Sites |
|---|---:|
| `org.spigotmc.SpigotWorldConfig#viewDistance` | 52 |
| `org.bukkit.event.world.EntitiesLoadEvent#getEntities` | 51 |
| `org.bukkit.event.entity.EntityTargetLivingEntityEvent#isCancelled` | 35 |
| `org.bukkit.event.world.EntitiesLoadEvent#getWorld` | 34 |
| `org.spigotmc.SpigotWorldConfig#simulationDistance` | 32 |
| `org.bukkit.event.entity.EntityTargetEvent$TargetReason#TEMPT` | 18 |
| `org.bukkit.event.entity.EntityTargetEvent$TargetReason#CUSTOM` | 17 |
| `org.bukkit.event.world.EntitiesLoadEvent#getChunk` | 17 |
| `org.bukkit.entity.Player#hideEntity` | 16 |
| `org.bukkit.entity.Player#showEntity` | 16 |
| `org.bukkit.entity.Item#getItemStack` | 7 |
| `org.bukkit.event.entity.ExplosionPrimeEvent#getEntity` | 7 |
| `org.bukkit.event.block.BlockExplodeEvent#getBlock` | 6 |
| `org.bukkit.Chunk#getEntities` | 5 |
| `org.bukkit.event.entity.CreatureSpawnEvent#getEntity` | 5 |
| `org.bukkit.World#getLivingEntities` | 4 |
| `org.bukkit.block.Hopper#isPlaced` | 4 |
| `org.bukkit.event.block.BlockRedstoneEvent#getBlock` | 4 |
| `org.bukkit.event.entity.ItemDespawnEvent#getEntity` | 4 |
| `org.bukkit.event.inventory.InventoryMoveItemEvent#getSource` | 4 |
| `org.bukkit.World#getEntitiesByClass` | 3 |
| `org.bukkit.World#getLoadedChunks` | 3 |
| `org.bukkit.block.Hopper#getChunk` | 3 |
| `org.bukkit.event.block.BlockExplodeEvent#setCancelled` | 3 |
| `org.bukkit.event.block.BlockPistonExtendEvent#getBlock` | 3 |
| `org.bukkit.event.entity.EntityExplodeEvent#getEntity` | 3 |
| `org.bukkit.event.entity.EntityExplodeEvent#getLocation` | 3 |
| `org.bukkit.event.entity.ProjectileLaunchEvent#getEntity` | 3 |
| `org.bukkit.event.inventory.InventoryType#HOPPER` | 3 |
| `org.bukkit.event.player.PlayerDropItemEvent#getPlayer` | 3 |
| `org.bukkit.event.player.PlayerRiptideEvent#getPlayer` | 3 |
| `org.bukkit.event.vehicle.VehicleCreateEvent#getVehicle` | 3 |
| `org.bukkit.inventory.Inventory#getViewers` | 3 |
| `org.bukkit.inventory.ItemStack#getMaxStackSize` | 3 |
| `org.bukkit.World#getEntities` | 2 |
| `org.bukkit.block.Block#getState` | 2 |
| `org.bukkit.block.Hopper#getInventory` | 2 |
| `org.bukkit.entity.Item#getLocation` | 2 |
| `org.bukkit.entity.Item#getPickupDelay` | 2 |
| `org.bukkit.entity.Player#setCooldown` | 2 |
| ... 114 more | |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| NMS (net.minecraft) | `net.minecraft.core` | own | 15 | yes |
| NMS (net.minecraft) | `net.minecraft.nbt` | own | 17 | yes |
| NMS (net.minecraft) | `net.minecraft.server.dedicated` | own | 17 | yes |
| NMS (net.minecraft) | `net.minecraft.server.level` | own | 69 | yes |
| NMS (net.minecraft) | `net.minecraft.server.v1_16_R3` | own | 85 | yes |
| NMS (net.minecraft) | `net.minecraft.tags` | own | 22 | yes |
| NMS (net.minecraft) | `net.minecraft.util` | own | 15 | yes |
| NMS (net.minecraft) | `net.minecraft.world.entity` | own | 1633 | yes |
| NMS (net.minecraft) | `net.minecraft.world.item` | own | 140 | yes |
| NMS (net.minecraft) | `net.minecraft.world.level` | own | 260 | yes |
| NMS (net.minecraft) | `net.minecraft.world.phys` | own | 17 | yes |
| CraftBukkit | `org.bukkit.craftbukkit` | own | 9 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.entity` | own | 12 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.event` | own | 6 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_16_R3` | own | 8 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_17_R1` | own | 9 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_18_R2` | own | 9 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_19_R3` | own | 9 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_20_R1` | own | 9 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_20_R2` | own | 9 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_20_R3` | own | 9 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_20_R4` | own | 9 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_21_R1` | own | 9 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_21_R2` | own | 9 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_21_R3` | own | 9 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_21_R4` | own | 9 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_21_R5` | own | 9 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_21_R6` | own | 9 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_21_R7` | own | 9 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `com.mojang.authlib` | own | 19 | yes |

## 🟡 LifeStealZ 2.21.1

- file: `lifestealz--lifestealz-2.21.1.jar`
- main: `com.zetaplugins.lifestealz.LifeStealZ`, api-version: 1.19
- softdepend: PlaceholderAPI, WorldGuard, floodgate
- classes: 760 (119 own under `com.zetaplugins.lifestealz`, 641 bundled), reflection calls: 115
- uses 101 uncovered API members and 46 uncovered API types (top: org.bukkit.entity.BlockDisplay#setTransformation, org.bukkit.util.Transformation#<init>, io.papermc.paper.datacomponent.DataComponentTypes#CUSTOM_MODEL_DATA, io.papermc.paper.datacomponent.item.CustomModelData#customModelData, io.papermc.paper.datacomponent.item.CustomModelData$Builder#addString)

| Uncovered member | Sites |
|---|---:|
| `org.bukkit.entity.BlockDisplay#setTransformation` | 7 |
| `org.bukkit.util.Transformation#<init>` | 7 |
| `io.papermc.paper.datacomponent.DataComponentTypes#CUSTOM_MODEL_DATA` | 5 |
| `io.papermc.paper.datacomponent.item.CustomModelData#customModelData` | 5 |
| `io.papermc.paper.datacomponent.item.CustomModelData$Builder#addString` | 5 |
| `io.papermc.paper.datacomponent.item.CustomModelData$Builder#build` | 5 |
| `org.bukkit.inventory.ItemStack#setData` | 5 |
| `org.bukkit.Bukkit#getTag` | 4 |
| `org.bukkit.entity.BlockDisplay#remove` | 4 |
| `org.bukkit.block.Block#getState` | 3 |
| `org.bukkit.entity.BlockDisplay#setBlock` | 3 |
| `org.bukkit.entity.BlockDisplay#setPersistent` | 3 |
| `org.bukkit.entity.Item#getItemStack` | 3 |
| `com.destroystokyo.paper.exception.ServerPluginException#getCause` | 2 |
| `org.bukkit.Bukkit#removeRecipe` | 2 |
| `org.bukkit.attribute.Attribute#MAX_HEALTH` | 2 |
| `org.bukkit.block.TileState#getPersistentDataContainer` | 2 |
| `org.bukkit.entity.BlockDisplay#getTransformation` | 2 |
| `org.bukkit.entity.Item#remove` | 2 |
| `org.bukkit.entity.Item#setInvulnerable` | 2 |
| `org.bukkit.entity.Item#setUnlimitedLifetime` | 2 |
| `org.bukkit.entity.Player#getAttribute` | 2 |
| `org.bukkit.entity.Player#getEffectivePermissions` | 2 |
| `org.bukkit.event.entity.EntityResurrectEvent#getEntity` | 2 |
| `org.bukkit.event.player.PlayerAttemptPickupItemEvent#getItem` | 2 |
| `org.bukkit.event.player.PlayerAttemptPickupItemEvent#setCancelled` | 2 |
| `org.bukkit.event.player.PlayerKickEvent$Cause#BANNED` | 2 |
| `org.bukkit.permissions.PermissionAttachmentInfo#getPermission` | 2 |
| `org.bukkit.util.Transformation#getScale` | 2 |
| `com.destroystokyo.paper.event.server.ServerExceptionEvent#getException` | 1 |
| `com.destroystokyo.paper.exception.ServerPluginException#getResponsiblePlugin` | 1 |
| `io.papermc.paper.configuration.ServerConfiguration#isProxyOnlineMode` | 1 |
| `io.papermc.paper.datacomponent.item.CustomModelData$Builder#addFloat` | 1 |
| `org.bukkit.Bukkit#addRecipe` | 1 |
| `org.bukkit.Bukkit#getOnlineMode` | 1 |
| `org.bukkit.EntityEffect#PROTECTED_FROM_DEATH` | 1 |
| `org.bukkit.Server#getOnlineMode` | 1 |
| `org.bukkit.Server#getPluginsFolder` | 1 |
| `org.bukkit.Server#getServerConfig` | 1 |
| `org.bukkit.Server$Spigot#getPaperConfig` | 1 |
| ... 61 more | |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| bytecode instrumentation | `java.lang.instrument` | com.zetaplugins.zetacore.libs.javassist.util | 3 | no |
| bytecode instrumentation | `javassist` | com.zaxxer.hikari | 10 | no |
| bytecode instrumentation | `javassist.bytecode` | com.zaxxer.hikari | 1 | no |

## 🔴 LuckPerms 5.5.71

- file: `luckperms--LuckPerms-Bukkit-5.5.71.jar`
- main: `me.lucko.luckperms.bukkit.loader.BukkitLoaderPlugin`, api-version: 1.13
- softdepend: LilyPad-Connect, ViaVersion
- classes: 905 (699 own under `me.lucko.luckperms`, 206 bundled), reflection calls: 79
- reflection into internals: Craft* class name (2 refs, in own)
- uses 37 uncovered API members and 13 uncovered API types (top: org.bukkit.permissions.PermissibleBase#<init>, org.bukkit.permissions.PermissibleBase#clearPermissions, org.bukkit.permissions.PermissionAttachment#getPlugin, org.bukkit.plugin.ServicesManager#unregister, com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#getBuffer)

| Uncovered member | Sites |
|---|---:|
| `org.bukkit.permissions.PermissibleBase#<init>` | 5 |
| `org.bukkit.permissions.PermissibleBase#clearPermissions` | 2 |
| `org.bukkit.permissions.PermissionAttachment#getPlugin` | 2 |
| `org.bukkit.plugin.ServicesManager#unregister` | 2 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#getBuffer` | 1 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#getSender` | 1 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#isCommand` | 1 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#setCompletions` | 1 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#setHandled` | 1 |
| `net.milkbowl.vault.permission.Permission#getGroups` | 1 |
| `net.milkbowl.vault.permission.Permission#plugin` | 1 |
| `org.bukkit.Server#getOnlineMode` | 1 |
| `org.bukkit.Server#selectEntities` | 1 |
| `org.bukkit.command.BlockCommandSender#getBlock` | 1 |
| `org.bukkit.command.ProxiedCommandSender#getCallee` | 1 |
| `org.bukkit.event.server.PluginDisableEvent#getPlugin` | 1 |
| `org.bukkit.event.server.PluginEnableEvent#getPlugin` | 1 |
| `org.bukkit.event.server.RemoteServerCommandEvent#getCommand` | 1 |
| `org.bukkit.event.server.RemoteServerCommandEvent#getSender` | 1 |
| `org.bukkit.event.server.ServerCommandEvent#setCommand` | 1 |
| `org.bukkit.permissions.PermissibleBase#addAttachment` | 1 |
| `org.bukkit.permissions.PermissibleBase#addAttachment` | 1 |
| `org.bukkit.permissions.PermissibleBase#addAttachment` | 1 |
| `org.bukkit.permissions.PermissibleBase#addAttachment` | 1 |
| `org.bukkit.permissions.PermissibleBase#getEffectivePermissions` | 1 |
| `org.bukkit.permissions.PermissibleBase#hasPermission` | 1 |
| `org.bukkit.permissions.PermissibleBase#hasPermission` | 1 |
| `org.bukkit.permissions.PermissibleBase#isOp` | 1 |
| `org.bukkit.permissions.PermissibleBase#isPermissionSet` | 1 |
| `org.bukkit.permissions.PermissibleBase#isPermissionSet` | 1 |
| `org.bukkit.permissions.PermissibleBase#recalculatePermissions` | 1 |
| `org.bukkit.permissions.PermissibleBase#removeAttachment` | 1 |
| `org.bukkit.permissions.PermissibleBase#setOp` | 1 |
| `org.bukkit.permissions.PermissionAttachment#getPermissions` | 1 |
| `org.bukkit.permissions.PermissionAttachmentInfo#<init>` | 1 |
| `org.bukkit.permissions.PermissionRemovedExecutor#attachmentRemoved` | 1 |
| `org.bukkit.plugin.PluginManager#removePermission` | 1 |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| reflection into internals | `Craft* class name` | own | 2 | yes |

## 🟡 Maintenance 5.1.1

- file: `maintenance--Maintenance-Paper-5.1.1.jar`
- main: `eu.kennytv.maintenance.paper.MaintenancePaperBase`, api-version: 1.20
- softdepend: ServerListPlus, ProtocolSupport, LuckPerms
- classes: 79 (65 own under `eu.kennytv.maintenance`, 14 bundled), reflection calls: 7
- uses 15 uncovered API members and 6 uncovered API types (top: com.destroystokyo.paper.event.server.PaperServerListPingEvent#getListedPlayers, com.destroystokyo.paper.event.server.PaperServerListPingEvent#getPlayerSample, com.destroystokyo.paper.event.server.PaperServerListPingEvent#setProtocolVersion, com.destroystokyo.paper.event.server.PaperServerListPingEvent#setServerIcon, com.destroystokyo.paper.event.server.PaperServerListPingEvent#setVersion)

| Uncovered member | Sites |
|---|---:|
| `com.destroystokyo.paper.event.server.PaperServerListPingEvent#getListedPlayers` | 1 |
| `com.destroystokyo.paper.event.server.PaperServerListPingEvent#getPlayerSample` | 1 |
| `com.destroystokyo.paper.event.server.PaperServerListPingEvent#setProtocolVersion` | 1 |
| `com.destroystokyo.paper.event.server.PaperServerListPingEvent#setServerIcon` | 1 |
| `com.destroystokyo.paper.event.server.PaperServerListPingEvent#setVersion` | 1 |
| `com.destroystokyo.paper.event.server.PaperServerListPingEvent$ListedPlayerInfo#<init>` | 1 |
| `com.destroystokyo.paper.profile.PlayerProfile#getId` | 1 |
| `com.destroystokyo.paper.profile.PlayerProfile#getName` | 1 |
| `io.papermc.paper.connection.PlayerConfigurationConnection#getProfile` | 1 |
| `io.papermc.paper.connection.PlayerConnection#getClass` | 1 |
| `io.papermc.paper.connection.PlayerLoginConnection#getAuthenticatedProfile` | 1 |
| `org.bukkit.Bukkit#createProfileExact` | 1 |
| `org.bukkit.Bukkit#getOnlineMode` | 1 |
| `org.bukkit.Server#loadServerIcon` | 1 |
| `org.bukkit.Server#sendMessage` | 1 |

## 🟡 MiniMOTD 2.2.5

- file: `minimotd--minimotd-paper-2.2.5.jar`
- main: `xyz.jpenilla.minimotd.paper.MiniMOTDPaper`, api-version: 1.21.8
- softdepend: MiniPlaceholders
- classes: 452 (37 own under `xyz.jpenilla.minimotd`, 415 bundled), reflection calls: 34
- uses 14 uncovered API members and 13 uncovered API types (top: io.papermc.paper.command.brigadier.CommandSourceStack#getSender, com.destroystokyo.paper.event.server.PaperServerListPingEvent#getClient, com.destroystokyo.paper.event.server.PaperServerListPingEvent#getListedPlayers, com.destroystokyo.paper.event.server.PaperServerListPingEvent#setNumPlayers, com.destroystokyo.paper.event.server.PaperServerListPingEvent#setServerIcon)

| Uncovered member | Sites |
|---|---:|
| `io.papermc.paper.command.brigadier.CommandSourceStack#getSender` | 2 |
| `com.destroystokyo.paper.event.server.PaperServerListPingEvent#getClient` | 1 |
| `com.destroystokyo.paper.event.server.PaperServerListPingEvent#getListedPlayers` | 1 |
| `com.destroystokyo.paper.event.server.PaperServerListPingEvent#setNumPlayers` | 1 |
| `com.destroystokyo.paper.event.server.PaperServerListPingEvent#setServerIcon` | 1 |
| `com.destroystokyo.paper.network.StatusClient#getProtocolVersion` | 1 |
| `io.papermc.paper.command.brigadier.CommandSourceStack#getExecutor` | 1 |
| `io.papermc.paper.command.brigadier.Commands#register` | 1 |
| `io.papermc.paper.plugin.lifecycle.event.LifecycleEventManager#registerEventHandler` | 1 |
| `io.papermc.paper.plugin.lifecycle.event.registrar.ReloadableRegistrarEvent#registrar` | 1 |
| `io.papermc.paper.plugin.lifecycle.event.types.LifecycleEvents#COMMANDS` | 1 |
| `org.bukkit.Bukkit#getOnlineMode` | 1 |
| `org.bukkit.Server#loadServerIcon` | 1 |
| `org.bukkit.plugin.java.JavaPlugin#getLifecycleManager` | 1 |

## 🔴 Multiverse-Core 5.8.2

- file: `multiversecore--multiverse-core-5.8.2.jar`
- main: `org.mvplugins.multiverse.core.MultiverseCore`, api-version: 1.13
- softdepend: Vault, PlaceholderAPI
- classes: 2323 (469 own under `org.mvplugins.multiverse`, 1854 bundled), reflection calls: 172
- reflection into internals: Craft* class name, net.minecraft.world, org.bukkit.craftbukkit (4 refs, in org.mvplugins.multiverse.external.acf.commands, own)
- uses 214 uncovered API members and 55 uncovered API types (top: org.bukkit.GameRule#getName, org.bukkit.plugin.PluginManager#removePermission, org.bukkit.WorldBorder#getCenter, org.bukkit.event.server.PluginDisableEvent#getPlugin, org.bukkit.GameRule#getByName)

| Uncovered member | Sites |
|---|---:|
| `org.bukkit.GameRule#getName` | 11 |
| `org.bukkit.plugin.PluginManager#removePermission` | 9 |
| `org.bukkit.WorldBorder#getCenter` | 6 |
| `org.bukkit.event.server.PluginDisableEvent#getPlugin` | 6 |
| `org.bukkit.GameRule#getByName` | 5 |
| `org.bukkit.PortalType#CUSTOM` | 5 |
| `org.bukkit.World#getWorldBorder` | 5 |
| `org.bukkit.WorldBorder#getSize` | 5 |
| `org.bukkit.event.server.PluginEnableEvent#getPlugin` | 5 |
| `org.bukkit.generator.WorldInfo#getName` | 5 |
| `org.bukkit.World#getGameRules` | 4 |
| `org.bukkit.World#setGameRule` | 4 |
| `org.bukkit.WorldCreator#name` | 4 |
| `org.bukkit.WorldType#NORMAL` | 4 |
| `org.bukkit.command.BlockCommandSender#getBlock` | 4 |
| `org.bukkit.entity.Player#getBedSpawnLocation` | 4 |
| `org.bukkit.event.player.PlayerPortalEvent#getPlayer` | 4 |
| `org.bukkit.event.world.PortalCreateEvent$CreateReason#ordinal` | 4 |
| `org.bukkit.GameRule#getType` | 3 |
| `org.bukkit.PortalType#ENDER` | 3 |
| `org.bukkit.PortalType#NETHER` | 3 |
| `org.bukkit.WorldBorder#getDamageAmount` | 3 |
| `org.bukkit.WorldBorder#getDamageBuffer` | 3 |
| `org.bukkit.WorldCreator#environment` | 3 |
| `org.bukkit.entity.Player#canSee` | 3 |
| `org.bukkit.entity.SpawnCategory#name` | 3 |
| `org.bukkit.entity.SpawnCategory#values` | 3 |
| `org.bukkit.event.entity.EntitySpawnEvent#getEntity` | 3 |
| `org.bukkit.event.world.PortalCreateEvent#getWorld` | 3 |
| `com.destroystokyo.paper.brigadier.BukkitBrigadierCommandSource#getBukkitSender` | 2 |
| `com.destroystokyo.paper.event.brigadier.CommandRegisteredEvent#getBrigadierCommand` | 2 |
| `com.destroystokyo.paper.event.entity.PreCreatureSpawnEvent#getType` | 2 |
| `com.destroystokyo.paper.event.player.PlayerAdvancementCriterionGrantEvent#getCriterion` | 2 |
| `org.bukkit.Bukkit#getWorldContainer` | 2 |
| `org.bukkit.Difficulty#toString` | 2 |
| `org.bukkit.Material#isSolid` | 2 |
| `org.bukkit.Registry#BIOME` | 2 |
| `org.bukkit.World#getGameRuleDefault` | 2 |
| `org.bukkit.World#getGameRuleValue` | 2 |
| `org.bukkit.World#getKeepSpawnInMemory` | 2 |
| ... 174 more | |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| reflection into internals | `Craft* class name` | org.mvplugins.multiverse.external.acf.commands | 1 | yes |
| reflection into internals | `net.minecraft.world` | own | 2 | yes |
| reflection into internals | `org.bukkit.craftbukkit` | own | 1 | yes |
| bytecode instrumentation | `java.lang.instrument` | org.mvplugins.multiverse.external.javassist.util | 3 | no |

## 🔴 Multiverse-Inventories 5.3.6

- file: `multiverseinventories--multiverse-inventories-5.3.6.jar`
- main: `org.mvplugins.multiverse.inventories.MultiverseInventories`, api-version: 1.13
- depend: Multiverse-Core
- softdepend: LuckPerms, PlaceholderAPI, MultiInv, WorldInventories, PerWorldInventory
- classes: 1131 (1080 own under `org.mvplugins.multiverse`, 51 bundled), reflection calls: 464
- reflection into internals: getHandle(), net.minecraft.server, org.bukkit.craftbukkit (4 refs, in own)
- bytecode instrumentation: org.objectweb.asm (7 refs, in own)
- uses 111 uncovered API members and 35 uncovered API types (top: org.bukkit.Statistic#name, org.bukkit.Statistic$Type#ordinal, org.bukkit.event.player.PlayerRespawnEvent#isBedSpawn, org.bukkit.event.inventory.InventoryClickEvent#getAction, org.bukkit.event.inventory.InventoryDragEvent#getInventory)

| Uncovered member | Sites |
|---|---:|
| `org.bukkit.Statistic#name` | 16 |
| `org.bukkit.Statistic$Type#ordinal` | 6 |
| `org.bukkit.event.player.PlayerRespawnEvent#isBedSpawn` | 6 |
| `org.bukkit.event.inventory.InventoryClickEvent#getAction` | 5 |
| `org.bukkit.event.inventory.InventoryDragEvent#getInventory` | 5 |
| `org.bukkit.attribute.Attributable#getAttribute` | 4 |
| `org.bukkit.conversations.Prompt#END_OF_CONVERSATION` | 4 |
| `org.bukkit.entity.Player#setBedSpawnLocation` | 4 |
| `org.bukkit.entity.Player#setExp` | 4 |
| `org.bukkit.entity.Player#setTotalExperience` | 4 |
| `org.bukkit.entity.Player#getBedSpawnLocation` | 3 |
| `org.bukkit.entity.Player#getExp` | 3 |
| `org.bukkit.entity.Player#setExhaustion` | 3 |
| `org.bukkit.entity.Player#setFallDistance` | 3 |
| `org.bukkit.entity.Player#setFireTicks` | 3 |
| `org.bukkit.event.inventory.InventoryDragEvent#setCancelled` | 3 |
| `org.bukkit.event.player.PlayerAdvancementDoneEvent#getPlayer` | 3 |
| `org.bukkit.event.server.PluginEnableEvent#getPlugin` | 3 |
| `com.destroystokyo.paper.event.player.PlayerSetSpawnEvent#getCause` | 2 |
| `org.bukkit.Bukkit#advancementIterator` | 2 |
| `org.bukkit.GameRule#getByName` | 2 |
| `org.bukkit.Material#isBlock` | 2 |
| `org.bukkit.Material#isItem` | 2 |
| `org.bukkit.Registry#ATTRIBUTE` | 2 |
| `org.bukkit.Registry#get` | 2 |
| `org.bukkit.Statistic#getType` | 2 |
| `org.bukkit.Statistic#values` | 2 |
| `org.bukkit.World#setGameRule` | 2 |
| `org.bukkit.entity.Player#getActivePotionEffects` | 2 |
| `org.bukkit.entity.Player#getAdvancementProgress` | 2 |
| `org.bukkit.entity.Player#getEnderChest` | 2 |
| `org.bukkit.entity.Player#getStatistic` | 2 |
| `org.bukkit.entity.Player#getTotalExperience` | 2 |
| `org.bukkit.entity.Player#setItemOnCursor` | 2 |
| `org.bukkit.entity.Player#setStatistic` | 2 |
| `org.bukkit.entity.Player#setStatistic` | 2 |
| `org.bukkit.event.inventory.InventoryDragEvent#getRawSlots` | 2 |
| `org.bukkit.event.player.PlayerRecipeDiscoverEvent#getPlayer` | 2 |
| `org.bukkit.event.player.PlayerSpawnChangeEvent#getCause` | 2 |
| `org.bukkit.event.player.PlayerSpawnChangeEvent#getNewSpawn` | 2 |
| ... 71 more | |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| reflection into internals | `getHandle()` | own | 1 | yes |
| reflection into internals | `net.minecraft.server` | own | 2 | yes |
| reflection into internals | `org.bukkit.craftbukkit` | own | 1 | yes |
| bytecode instrumentation | `org.objectweb.asm` | own | 7 | yes |

## 🟡 Multiverse-Portals 5.3.0

- file: `multiverseportals--multiverse-portals-5.3.0.jar`
- main: `org.mvplugins.multiverse.portals.MultiversePortals`, api-version: 1.13
- depend: Multiverse-Core
- softdepend: WorldEdit, PlaceholderAPI
- classes: 83 (83 own under `org.mvplugins.multiverse`, 0 bundled), reflection calls: 0
- uses 43 uncovered API members and 12 uncovered API types (top: org.bukkit.event.player.PlayerBucketEmptyEvent#getPlayer, org.bukkit.event.player.PlayerBucketFillEvent#getBlockClicked, org.bukkit.event.player.PlayerPortalEvent#setCancelled, org.bukkit.plugin.PluginManager#removePermission, org.bukkit.entity.Vehicle#getLocation)

| Uncovered member | Sites |
|---|---:|
| `org.bukkit.event.player.PlayerBucketEmptyEvent#getPlayer` | 6 |
| `org.bukkit.event.player.PlayerBucketFillEvent#getBlockClicked` | 4 |
| `org.bukkit.event.player.PlayerPortalEvent#setCancelled` | 4 |
| `org.bukkit.plugin.PluginManager#removePermission` | 4 |
| `org.bukkit.entity.Vehicle#getLocation` | 3 |
| `org.bukkit.event.player.PlayerBucketEmptyEvent#getBlockClicked` | 3 |
| `org.bukkit.event.player.PlayerBucketFillEvent#getPlayer` | 3 |
| `org.bukkit.event.block.BlockFromToEvent#getBlock` | 2 |
| `org.bukkit.event.block.BlockFromToEvent#getToBlock` | 2 |
| `org.bukkit.event.block.BlockFromToEvent#setCancelled` | 2 |
| `org.bukkit.event.block.BlockPhysicsEvent#getBlock` | 2 |
| `org.bukkit.event.entity.EntityPortalEvent#setCancelled` | 2 |
| `org.bukkit.event.player.PlayerBucketEmptyEvent#setCancelled` | 2 |
| `io.papermc.paper.event.entity.EntityMoveEvent#getEntity` | 1 |
| `io.papermc.paper.event.entity.EntityMoveEvent#getFrom` | 1 |
| `io.papermc.paper.event.entity.EntityMoveEvent#getTo` | 1 |
| `org.bukkit.entity.Player#getVehicle` | 1 |
| `org.bukkit.entity.Player#isInsideVehicle` | 1 |
| `org.bukkit.entity.Player#setFallDistance` | 1 |
| `org.bukkit.entity.Vehicle#getName` | 1 |
| `org.bukkit.entity.Vehicle#getPassengers` | 1 |
| `org.bukkit.entity.Vehicle#removePassenger` | 1 |
| `org.bukkit.event.block.BlockFromToEvent#getHandlerList` | 1 |
| `org.bukkit.event.block.BlockPhysicsEvent#getChangedType` | 1 |
| `org.bukkit.event.block.BlockPhysicsEvent#setCancelled` | 1 |
| `org.bukkit.event.entity.EntityPortalEvent#getEntity` | 1 |
| `org.bukkit.event.entity.EntityPortalEvent#getFrom` | 1 |
| `org.bukkit.event.player.PlayerBucketEmptyEvent#getBlockFace` | 1 |
| `org.bukkit.event.player.PlayerBucketEmptyEvent#getBucket` | 1 |
| `org.bukkit.event.player.PlayerBucketEmptyEvent#isCancelled` | 1 |
| `org.bukkit.event.player.PlayerBucketFillEvent#isCancelled` | 1 |
| `org.bukkit.event.player.PlayerBucketFillEvent#setCancelled` | 1 |
| `org.bukkit.event.player.PlayerMoveEvent#getHandlerList` | 1 |
| `org.bukkit.event.player.PlayerPortalEvent#getFrom` | 1 |
| `org.bukkit.event.player.PlayerPortalEvent#getPlayer` | 1 |
| `org.bukkit.event.server.PluginDisableEvent#getPlugin` | 1 |
| `org.bukkit.event.server.PluginEnableEvent#getPlugin` | 1 |
| `org.bukkit.event.vehicle.VehicleMoveEvent#getFrom` | 1 |
| `org.bukkit.event.vehicle.VehicleMoveEvent#getHandlerList` | 1 |
| `org.bukkit.event.vehicle.VehicleMoveEvent#getTo` | 1 |
| ... 3 more | |

## 🔴 NBTAPI 2.16.1

- file: `nbtapi--item-nbt-api-plugin-2.16.1.jar`
- main: `de.tr7zw.nbtapi.plugin.NBTAPI`, api-version: 1.13
- classes: 145 (145 own under `de.tr7zw.nbtapi`, 0 bundled), reflection calls: 63
- server internals (Mojang libs, Netty, Paper internals): com.mojang.authlib, com.mojang.authlib.properties, com.mojang.datafixers (20 refs, in own)
- reflection into internals: Craft* class name, com.mojang.authlib, getHandle() (83 refs, in own)
- uses 11 uncovered API members and 3 uncovered API types (top: org.bukkit.World#getEntitiesByClasses, org.bukkit.block.Block#getState, org.bukkit.block.BlockState#getX, org.bukkit.block.BlockState#getY, org.bukkit.block.BlockState#getZ)

| Uncovered member | Sites |
|---|---:|
| `org.bukkit.World#getEntitiesByClasses` | 4 |
| `org.bukkit.block.Block#getState` | 4 |
| `org.bukkit.block.BlockState#getX` | 4 |
| `org.bukkit.block.BlockState#getY` | 4 |
| `org.bukkit.block.BlockState#getZ` | 4 |
| `org.bukkit.Chunk#getPersistentDataContainer` | 3 |
| `org.bukkit.World#getLoadedChunks` | 2 |
| `org.bukkit.block.BlockState#getWorld` | 2 |
| `org.bukkit.block.BlockState#isPlaced` | 2 |
| `org.bukkit.Bukkit#getOnlineMode` | 1 |
| `org.bukkit.block.BlockState#getType` | 1 |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| server internals (Mojang libs, Netty, Paper internals) | `com.mojang.authlib` | own | 4 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `com.mojang.authlib.properties` | own | 2 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `com.mojang.datafixers` | own | 3 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `com.mojang.serialization` | own | 11 | yes |
| reflection into internals | `Craft* class name` | own | 6 | yes |
| reflection into internals | `com.mojang.authlib` | own | 1 | yes |
| reflection into internals | `getHandle()` | own | 1 | yes |
| reflection into internals | `net.minecraft.core` | own | 9 | yes |
| reflection into internals | `net.minecraft.entity` | own | 2 | yes |
| reflection into internals | `net.minecraft.item` | own | 1 | yes |
| reflection into internals | `net.minecraft.nbt` | own | 27 | yes |
| reflection into internals | `net.minecraft.resources` | own | 2 | yes |
| reflection into internals | `net.minecraft.server` | own | 5 | yes |
| reflection into internals | `net.minecraft.tileentity` | own | 1 | yes |
| reflection into internals | `net.minecraft.util` | own | 8 | yes |
| reflection into internals | `net.minecraft.world` | own | 20 | yes |

## 🔴 nightcore 2.16.7

- file: `nightcore--nightcore-2.16.7.jar`
- main: `su.nightexpress.nightcore.NightCore`, api-version: 1.21
- softdepend: Vault, LuckPerms, PlaceholderAPI, floodgate, CoinsEngine, ExcellentEconomy, PlayerPoints, VotingPlugin, ExcellentCrates, ExecutableItems, ItemsAdder, MMOItems, Nexo, Oraxen, CraftEngine
- classes: 852 (852 own under `su.nightexpress.nightcore`, 0 bundled), reflection calls: 21
- server internals (Mojang libs, Netty, Paper internals): com.mojang.datafixers, com.mojang.serialization (14 refs, in own)
- reflection into internals: Craft* class name, net.minecraft.core, net.minecraft.nbt (13 refs, in own)
- uses 301 uncovered API members and 136 uncovered API types (top: org.bukkit.Tag#getValues, org.bukkit.Tag#isTagged, org.bukkit.Material#isItem, org.bukkit.Registry#stream, net.kyori.adventure.bossbar.BossBar$Flag#ordinal)

| Uncovered member | Sites |
|---|---:|
| `org.bukkit.Tag#getValues` | 11 |
| `org.bukkit.Tag#isTagged` | 10 |
| `org.bukkit.Material#isItem` | 5 |
| `org.bukkit.Registry#stream` | 5 |
| `net.kyori.adventure.bossbar.BossBar$Flag#ordinal` | 4 |
| `net.milkbowl.vault.economy.EconomyResponse#transactionSuccess` | 4 |
| `org.bukkit.World#getUID` | 4 |
| `org.bukkit.boss.BarFlag#ordinal` | 4 |
| `org.bukkit.profile.PlayerTextures#getSkin` | 4 |
| `org.bukkit.profile.PlayerTextures#setSkin` | 4 |
| `io.papermc.paper.datacomponent.DataComponentTypes#TOOLTIP_DISPLAY` | 3 |
| `io.papermc.paper.event.player.AsyncChatEvent#viewers` | 3 |
| `org.bukkit.Material#isBlock` | 3 |
| `org.bukkit.Registry#get` | 3 |
| `org.bukkit.entity.LivingEntity#getAttribute` | 3 |
| `org.bukkit.entity.Player#isSleeping` | 3 |
| `org.bukkit.event.inventory.InventoryDragEvent#getWhoClicked` | 3 |
| `org.bukkit.event.inventory.InventoryDragEvent#setCancelled` | 3 |
| `org.bukkit.event.player.AsyncPlayerChatEvent#getRecipients` | 3 |
| `org.bukkit.inventory.MenuType#GENERIC_9X3` | 3 |
| `org.bukkit.profile.PlayerProfile#getTextures` | 3 |
| `io.papermc.paper.plugin.loader.library.impl.MavenLibraryResolver#addDependency` | 2 |
| `io.papermc.paper.plugin.loader.library.impl.MavenLibraryResolver#addRepository` | 2 |
| `net.kyori.adventure.bossbar.BossBar$Flag#CREATE_WORLD_FOG` | 2 |
| `net.kyori.adventure.bossbar.BossBar$Flag#DARKEN_SCREEN` | 2 |
| `net.kyori.adventure.bossbar.BossBar$Flag#PLAY_BOSS_MUSIC` | 2 |
| `net.milkbowl.vault.permission.Permission#hasGroupSupport` | 2 |
| `org.bukkit.Bukkit#createPlayerProfile` | 2 |
| `org.bukkit.Bukkit#getUnsafe` | 2 |
| `org.bukkit.Chunk#getPersistentDataContainer` | 2 |
| `org.bukkit.FireworkEffect#getColors` | 2 |
| `org.bukkit.Material#getMaxStackSize` | 2 |
| `org.bukkit.Particle$DustTransition#<init>` | 2 |
| `org.bukkit.Tag#BUTTONS` | 2 |
| `org.bukkit.Tag#DOORS` | 2 |
| `org.bukkit.Tag#FENCE_GATES` | 2 |
| `org.bukkit.Tag#PRESSURE_PLATES` | 2 |
| `org.bukkit.Tag#TRAPDOORS` | 2 |
| `org.bukkit.World#getLoadedChunks` | 2 |
| `org.bukkit.block.BlockState#getBlock` | 2 |
| ... 261 more | |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| server internals (Mojang libs, Netty, Paper internals) | `com.mojang.datafixers` | own | 3 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `com.mojang.serialization` | own | 11 | yes |
| reflection into internals | `Craft* class name` | own | 4 | yes |
| reflection into internals | `net.minecraft.core` | own | 1 | yes |
| reflection into internals | `net.minecraft.nbt` | own | 3 | yes |
| reflection into internals | `net.minecraft.util` | own | 2 | yes |
| reflection into internals | `net.minecraft.world` | own | 3 | yes |

## 🔴 packetevents 2.14.0

- file: `packetevents--packetevents-spigot-2.14.0.jar`
- main: `io.github.retrooper.packetevents.PacketEventsPlugin`, api-version: 1.13
- softdepend: ProtocolLib, ProtocolSupport, ViaVersion, ViaBackwards, ViaRewind, Geyser-Spigot
- classes: 2270 (178 own under `io.github.retrooper`, 2092 bundled), reflection calls: 88
- server internals (Mojang libs, Netty, Paper internals): io.netty.buffer, io.netty.channel, io.netty.handler.codec (89 refs, in own)
- reflection into internals: Craft* class name, com.mojang.authlib, getHandle() (20 refs, in own)
- uses 19 uncovered API members and 9 uncovered API types (top: io.papermc.paper.event.connection.PlayerConnectionValidateLoginEvent#getConnection, org.bukkit.World$Environment#getId, org.bukkit.entity.EntityType#getTypeId, io.papermc.paper.event.connection.PlayerConnectionValidateLoginEvent#isAllowed, org.bukkit.Bukkit#getOnlineMode)

| Uncovered member | Sites |
|---|---:|
| `io.papermc.paper.event.connection.PlayerConnectionValidateLoginEvent#getConnection` | 2 |
| `org.bukkit.World$Environment#getId` | 2 |
| `org.bukkit.entity.EntityType#getTypeId` | 2 |
| `io.papermc.paper.event.connection.PlayerConnectionValidateLoginEvent#isAllowed` | 1 |
| `org.bukkit.Bukkit#getOnlineMode` | 1 |
| `org.bukkit.Material#getId` | 1 |
| `org.bukkit.Registry#PARTICLE_TYPE` | 1 |
| `org.bukkit.Registry#get` | 1 |
| `org.bukkit.World#getEntities` | 1 |
| `org.bukkit.entity.EntityType#fromId` | 1 |
| `org.bukkit.entity.Pose#ordinal` | 1 |
| `org.bukkit.entity.Pose#values` | 1 |
| `org.bukkit.inventory.MainHand#values` | 1 |
| `org.bukkit.material.MaterialData#<init>` | 1 |
| `org.bukkit.material.MaterialData#getData` | 1 |
| `org.bukkit.material.MaterialData#getItemType` | 1 |
| `org.bukkit.potion.PotionEffectType#getById` | 1 |
| `org.bukkit.potion.PotionEffectType#getId` | 1 |
| `org.spigotmc.event.player.PlayerSpawnLocationEvent#getPlayer` | 1 |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| server internals (Mojang libs, Netty, Paper internals) | `io.netty.buffer` | own | 21 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `io.netty.channel` | own | 59 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `io.netty.handler.codec` | own | 1 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `io.netty.util` | own | 2 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `io.netty.util.concurrent` | own | 4 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `io.netty.util.internal` | own | 2 | yes |
| reflection into internals | `Craft* class name` | own | 9 | yes |
| reflection into internals | `com.mojang.authlib` | own | 2 | yes |
| reflection into internals | `getHandle()` | own | 1 | yes |
| reflection into internals | `net.minecraft.` | own | 1 | yes |
| reflection into internals | `net.minecraft.network` | own | 5 | yes |
| reflection into internals | `net.minecraft.server` | own | 2 | yes |

## 🔴 PlaceholderAPI 2.12.3

- file: `placeholderapi--PlaceholderAPI-2.12.3.jar`
- main: `me.clip.placeholderapi.PlaceholderAPIPlugin`, api-version: 1.13
- classes: 716 (92 own under `me.clip.placeholderapi`, 624 bundled), reflection calls: 86
- server internals (Mojang libs, Netty, Paper internals): io.netty.buffer (5 refs, in me.clip.placeholderapi.libs.kyori.adventure)
- reflection into internals: Craft* class name, getHandle(), net.minecraft. (15 refs, in me.clip.placeholderapi.libs.kyori.adventure)
- uses 28 uncovered API members and 10 uncovered API types (top: com.destroystokyo.paper.Title#builder, com.destroystokyo.paper.Title$Builder#build, com.destroystokyo.paper.Title$Builder#fadeIn, com.destroystokyo.paper.Title$Builder#fadeOut, com.destroystokyo.paper.Title$Builder#stay)

| Uncovered member | Sites |
|---|---:|
| `com.destroystokyo.paper.Title#builder` | 1 |
| `com.destroystokyo.paper.Title$Builder#build` | 1 |
| `com.destroystokyo.paper.Title$Builder#fadeIn` | 1 |
| `com.destroystokyo.paper.Title$Builder#fadeOut` | 1 |
| `com.destroystokyo.paper.Title$Builder#stay` | 1 |
| `com.destroystokyo.paper.Title$Builder#subtitle` | 1 |
| `com.destroystokyo.paper.Title$Builder#title` | 1 |
| `io.papermc.paper.ServerBuildInfo#buildInfo` | 1 |
| `io.papermc.paper.ServerBuildInfo#buildNumber` | 1 |
| `io.papermc.paper.ServerBuildInfo#minecraftVersionId` | 1 |
| `org.bukkit.Bukkit#getOnlineMode` | 1 |
| `org.bukkit.Bukkit#getUnsafe` | 1 |
| `org.bukkit.OfflinePlayer#hasPlayedBefore` | 1 |
| `org.bukkit.Server#shutdown` | 1 |
| `org.bukkit.UnsafeValues#getDataVersion` | 1 |
| `org.bukkit.boss.BarFlag#CREATE_FOG` | 1 |
| `org.bukkit.boss.BarFlag#DARKEN_SKY` | 1 |
| `org.bukkit.boss.BarFlag#PLAY_BOSS_MUSIC` | 1 |
| `org.bukkit.boss.BossBar#addFlag` | 1 |
| `org.bukkit.boss.BossBar#removeFlag` | 1 |
| `org.bukkit.boss.BossBar#setStyle` | 1 |
| `org.bukkit.command.ProxiedCommandSender#getCallee` | 1 |
| `org.bukkit.entity.Entity#setCustomName` | 1 |
| `org.bukkit.entity.Player#openBook` | 1 |
| `org.bukkit.entity.Player#stopSound` | 1 |
| `org.bukkit.entity.Player#stopSound` | 1 |
| `org.bukkit.event.server.PluginDisableEvent#getPlugin` | 1 |
| `org.bukkit.inventory.meta.BookMeta$Spigot#addPage` | 1 |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| server internals (Mojang libs, Netty, Paper internals) | `io.netty.buffer` | me.clip.placeholderapi.libs.kyori.adventure | 5 | yes |
| reflection into internals | `Craft* class name` | me.clip.placeholderapi.libs.kyori.adventure | 9 | yes |
| reflection into internals | `getHandle()` | me.clip.placeholderapi.libs.kyori.adventure | 2 | yes |
| reflection into internals | `net.minecraft.` | me.clip.placeholderapi.libs.kyori.adventure | 1 | yes |
| reflection into internals | `net.minecraft.server` | me.clip.placeholderapi.libs.kyori.adventure | 1 | yes |
| reflection into internals | `org.bukkit.craftbukkit` | me.clip.placeholderapi.libs.kyori.adventure | 2 | yes |

## 🔴 Plan 5.8 build 3638

- file: `planplayeranalytics--Plan-5.8-build-3638.jar`
- main: `com.djrapitops.plan.Plan`, api-version: 1.13
- softdepend: AAC, ASkyBlock, AdvancedAchievements, AdvancedBan, AuthMe, BanManager, BentoBox, CoreProtect, DiscordSRV, DKCoins, DKBans, Essentials, CMI, Factions, FastLogin, GriefPrevention, GriefPreventionPlus, Jobs, Kingdoms, LiteBans, LogBlock, LuckPerms, MarriageMaster, mcMMO, MinigamesLib, MyPet, nuVotifier, PlaceholderAPI, ProtectionStones, ProtocolSupport, Quests, React, RedProtect, SuperbVote, Towny, Vault, ViaVersion, Votifier
- classes: 8362 (2034 own under `com.djrapitops.plan`, 6328 bundled), reflection calls: 811
- NMS (net.minecraft): net.minecraft.server (1 refs, in own)
- reflection into internals: Craft* class name, getHandle(), net.minecraft.server (13 refs, in own)
- uses 24 uncovered API members and 31 uncovered API types (top: org.bukkit.Bukkit#getOnlineMode, org.bukkit.OfflinePlayer#hasPlayedBefore, org.bukkit.entity.Player#getFirstPlayed, net.milkbowl.vault.economy.Economy#hasAccount, org.bukkit.Bukkit#getBannedPlayers)

| Uncovered member | Sites |
|---|---:|
| `org.bukkit.Bukkit#getOnlineMode` | 2 |
| `org.bukkit.OfflinePlayer#hasPlayedBefore` | 2 |
| `org.bukkit.entity.Player#getFirstPlayed` | 2 |
| `net.milkbowl.vault.economy.Economy#hasAccount` | 1 |
| `org.bukkit.Bukkit#getBannedPlayers` | 1 |
| `org.bukkit.Bukkit#getOfflinePlayers` | 1 |
| `org.bukkit.OfflinePlayer#getFirstPlayed` | 1 |
| `org.bukkit.OfflinePlayer#isBanned` | 1 |
| `org.bukkit.Server#getIp` | 1 |
| `org.bukkit.Server#getPort` | 1 |
| `org.bukkit.World#getChunkCount` | 1 |
| `org.bukkit.World#getEntities` | 1 |
| `org.bukkit.World#getEntityCount` | 1 |
| `org.bukkit.World#getLoadedChunks` | 1 |
| `org.bukkit.entity.Player#isBanned` | 1 |
| `org.bukkit.entity.Player$Spigot#getPing` | 1 |
| `org.bukkit.entity.Projectile#getShooter` | 1 |
| `org.bukkit.entity.Tameable#getClass` | 1 |
| `org.bukkit.entity.Tameable#getOwner` | 1 |
| `org.bukkit.entity.Tameable#getType` | 1 |
| `org.bukkit.entity.Tameable#isTamed` | 1 |
| `org.bukkit.event.player.PlayerKickEvent#getPlayer` | 1 |
| `org.bukkit.event.player.PlayerKickEvent#isCancelled` | 1 |
| `org.bukkit.event.player.PlayerLoginEvent#getHostname` | 1 |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| NMS (net.minecraft) | `net.minecraft.server` | own | 1 | yes |
| reflection into internals | `Craft* class name` | own | 5 | yes |
| reflection into internals | `getHandle()` | own | 4 | yes |
| reflection into internals | `net.minecraft.server` | own | 3 | yes |
| reflection into internals | `org.bukkit.craftbukkit` | own | 1 | yes |
| bytecode instrumentation | `javassist` | plan.com.zaxxer | 10 | no |
| bytecode instrumentation | `javassist.bytecode` | plan.com.zaxxer | 1 | no |
| bytecode instrumentation | `org.objectweb.asm` | plan.org.apache | 44 | no |

## 🔴 PlayerKits2 1.24.2

- file: `playerkits2--PlayerKits2-1.24.2.jar`
- main: `pk.ajneb97.PlayerKits2`, api-version: 1.13
- softdepend: Vault, PlaceholderAPI
- classes: 244 (97 own under `pk.ajneb97`, 147 bundled), reflection calls: 140
- server internals (Mojang libs, Netty, Paper internals): com.mojang.authlib, com.mojang.authlib.properties (3 refs, in own)
- reflection into internals: Craft* class name, getHandle(), net.minecraft.nbt (16 refs, in own, pk.ajneb97.libs.actionbar, pk.ajneb97.libs.titles)
- uses 94 uncovered API members and 30 uncovered API types (top: org.bukkit.profile.PlayerProfile#getTextures, org.bukkit.FireworkEffect#builder, org.bukkit.FireworkEffect$Builder#build, org.bukkit.FireworkEffect$Builder#flicker, org.bukkit.FireworkEffect$Builder#with)

| Uncovered member | Sites |
|---|---:|
| `org.bukkit.profile.PlayerProfile#getTextures` | 3 |
| `org.bukkit.FireworkEffect#builder` | 2 |
| `org.bukkit.FireworkEffect$Builder#build` | 2 |
| `org.bukkit.FireworkEffect$Builder#flicker` | 2 |
| `org.bukkit.FireworkEffect$Builder#with` | 2 |
| `org.bukkit.FireworkEffect$Builder#withColor` | 2 |
| `org.bukkit.FireworkEffect$Builder#withFade` | 2 |
| `org.bukkit.FireworkEffect$Type#valueOf` | 2 |
| `org.bukkit.Registry#get` | 2 |
| `org.bukkit.attribute.AttributeModifier#<init>` | 2 |
| `org.bukkit.attribute.AttributeModifier#<init>` | 2 |
| `org.bukkit.attribute.AttributeModifier#getAmount` | 2 |
| `org.bukkit.attribute.AttributeModifier#getKey` | 2 |
| `org.bukkit.attribute.AttributeModifier#getOperation` | 2 |
| `org.bukkit.attribute.AttributeModifier#getSlot` | 2 |
| `org.bukkit.attribute.AttributeModifier$Operation#name` | 2 |
| `org.bukkit.block.Banner#setBaseColor` | 2 |
| `org.bukkit.block.banner.Pattern#<init>` | 2 |
| `org.bukkit.inventory.meta.BlockStateMeta#getBlockState` | 2 |
| `org.bukkit.potion.PotionType#name` | 2 |
| `org.bukkit.potion.PotionType#valueOf` | 2 |
| `org.bukkit.profile.PlayerProfile#getUniqueId` | 2 |
| `org.bukkit.Bukkit#createPlayerProfile` | 1 |
| `org.bukkit.Bukkit#getOnlineMode` | 1 |
| `org.bukkit.FireworkEffect#getColors` | 1 |
| `org.bukkit.FireworkEffect#getFadeColors` | 1 |
| `org.bukkit.FireworkEffect#getType` | 1 |
| `org.bukkit.FireworkEffect#hasFlicker` | 1 |
| `org.bukkit.FireworkEffect#hasTrail` | 1 |
| `org.bukkit.FireworkEffect$Builder#trail` | 1 |
| `org.bukkit.FireworkEffect$Type#name` | 1 |
| `org.bukkit.Registry#TRIM_MATERIAL` | 1 |
| `org.bukkit.Registry#TRIM_PATTERN` | 1 |
| `org.bukkit.attribute.Attribute#GRAVITY` | 1 |
| `org.bukkit.attribute.AttributeModifier#<init>` | 1 |
| `org.bukkit.attribute.AttributeModifier#getName` | 1 |
| `org.bukkit.attribute.AttributeModifier#getSlotGroup` | 1 |
| `org.bukkit.attribute.AttributeModifier#getUniqueId` | 1 |
| `org.bukkit.attribute.AttributeModifier$Operation#ADD_NUMBER` | 1 |
| `org.bukkit.attribute.AttributeModifier$Operation#valueOf` | 1 |
| ... 54 more | |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| server internals (Mojang libs, Netty, Paper internals) | `com.mojang.authlib` | own | 1 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `com.mojang.authlib.properties` | own | 2 | yes |
| reflection into internals | `Craft* class name` | own | 2 | yes |
| reflection into internals | `Craft* class name` | pk.ajneb97.libs.actionbar | 1 | yes |
| reflection into internals | `getHandle()` | pk.ajneb97.libs.actionbar | 1 | yes |
| reflection into internals | `getHandle()` | pk.ajneb97.libs.titles | 1 | yes |
| reflection into internals | `net.minecraft.nbt` | own | 4 | yes |
| reflection into internals | `net.minecraft.server` | own | 1 | yes |
| reflection into internals | `net.minecraft.server` | pk.ajneb97.libs.actionbar | 1 | yes |
| reflection into internals | `net.minecraft.server` | pk.ajneb97.libs.titles | 1 | yes |
| reflection into internals | `net.minecraft.world` | own | 1 | yes |
| reflection into internals | `org.bukkit.craftbukkit` | own | 2 | yes |
| reflection into internals | `org.bukkit.craftbukkit` | pk.ajneb97.libs.actionbar | 1 | yes |
| bytecode instrumentation | `javassist` | pk.ajneb97.libs.hikaricp.util | 10 | no |
| bytecode instrumentation | `javassist.bytecode` | pk.ajneb97.libs.hikaricp.util | 1 | no |

## 🔴 PluginPortal 3.8.7

- file: `pluginportal--PluginPortal-3.8.7.jar`
- main: `gg.flyte.pluginportal.plugin.PluginPortal`, api-version: 1.13
- classes: 3090 (238 own under `gg.flyte.pluginportal`, 2852 bundled), reflection calls: 225
- server internals (Mojang libs, Netty, Paper internals): io.netty.buffer (5 refs, in net.kyori.adventure)
- reflection into internals: Craft* class name, getHandle(), net.minecraft. (18 refs, in net.kyori.adventure, revxrsal.commands.bukkit)
- uses 48 uncovered API members and 23 uncovered API types (top: com.destroystokyo.paper.event.brigadier.CommandRegisteredEvent#getCommand, io.papermc.paper.command.brigadier.CommandSourceStack#getExecutor, org.bukkit.Bukkit#selectEntities, org.bukkit.event.command.UnknownCommandEvent#getCommandLine, org.bukkit.event.server.PluginDisableEvent#getPlugin)

| Uncovered member | Sites |
|---|---:|
| `com.destroystokyo.paper.event.brigadier.CommandRegisteredEvent#getCommand` | 2 |
| `io.papermc.paper.command.brigadier.CommandSourceStack#getExecutor` | 2 |
| `org.bukkit.Bukkit#selectEntities` | 2 |
| `org.bukkit.event.command.UnknownCommandEvent#getCommandLine` | 2 |
| `org.bukkit.event.server.PluginDisableEvent#getPlugin` | 2 |
| `com.destroystokyo.paper.Title#builder` | 1 |
| `com.destroystokyo.paper.Title$Builder#build` | 1 |
| `com.destroystokyo.paper.Title$Builder#fadeIn` | 1 |
| `com.destroystokyo.paper.Title$Builder#fadeOut` | 1 |
| `com.destroystokyo.paper.Title$Builder#stay` | 1 |
| `com.destroystokyo.paper.Title$Builder#subtitle` | 1 |
| `com.destroystokyo.paper.Title$Builder#title` | 1 |
| `com.destroystokyo.paper.event.brigadier.CommandRegisteredEvent#getCommandLabel` | 1 |
| `com.destroystokyo.paper.event.brigadier.CommandRegisteredEvent#setLiteral` | 1 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#getBuffer` | 1 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#getCompletions` | 1 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#getSender` | 1 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#isCommand` | 1 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#setHandled` | 1 |
| `io.papermc.paper.command.brigadier.CommandSourceStack#getSender` | 1 |
| `io.papermc.paper.command.brigadier.Commands#register` | 1 |
| `io.papermc.paper.plugin.lifecycle.event.LifecycleEventManager#registerEventHandler` | 1 |
| `io.papermc.paper.plugin.lifecycle.event.registrar.ReloadableRegistrarEvent#registrar` | 1 |
| `io.papermc.paper.plugin.lifecycle.event.types.LifecycleEvents#COMMANDS` | 1 |
| `org.bukkit.Bukkit#getOnlineMode` | 1 |
| `org.bukkit.Bukkit#getUnsafe` | 1 |
| `org.bukkit.Bukkit#getUpdateFolderFile` | 1 |
| `org.bukkit.OfflinePlayer#getFirstPlayed` | 1 |
| `org.bukkit.OfflinePlayer#hasPlayedBefore` | 1 |
| `org.bukkit.Server#getIp` | 1 |
| `org.bukkit.Server#getOnlineMode` | 1 |
| `org.bukkit.Server#selectEntities` | 1 |
| `org.bukkit.UnsafeValues#getDataVersion` | 1 |
| `org.bukkit.boss.BarFlag#CREATE_FOG` | 1 |
| `org.bukkit.boss.BarFlag#DARKEN_SKY` | 1 |
| `org.bukkit.boss.BarFlag#PLAY_BOSS_MUSIC` | 1 |
| `org.bukkit.boss.BossBar#addFlag` | 1 |
| `org.bukkit.boss.BossBar#removeFlag` | 1 |
| `org.bukkit.boss.BossBar#setStyle` | 1 |
| `org.bukkit.command.ProxiedCommandSender#getCallee` | 1 |
| ... 8 more | |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| server internals (Mojang libs, Netty, Paper internals) | `io.netty.buffer` | net.kyori.adventure | 5 | yes |
| reflection into internals | `Craft* class name` | net.kyori.adventure | 9 | yes |
| reflection into internals | `Craft* class name` | revxrsal.commands.bukkit | 1 | yes |
| reflection into internals | `getHandle()` | net.kyori.adventure | 2 | yes |
| reflection into internals | `net.minecraft.` | net.kyori.adventure | 1 | yes |
| reflection into internals | `net.minecraft.` | revxrsal.commands.bukkit | 1 | yes |
| reflection into internals | `net.minecraft.server` | net.kyori.adventure | 1 | yes |
| reflection into internals | `net.minecraft.server` | revxrsal.commands.bukkit | 1 | yes |
| reflection into internals | `org.bukkit.craftbukkit` | net.kyori.adventure | 2 | yes |

## 🔴 PowerRanks 1.10.10

- file: `powerranks--PowerRanks.jar`
- main: `nl.svenar.powerranks.bukkit.PowerRanks`, api-version: 1.13
- softdepend: Vault, PlaceholderAPI, TAB, DeluxeTags, NametagEdit
- classes: 2228 (187 own under `nl.svenar.powerranks`, 2041 bundled), reflection calls: 133
- reflection into internals: Craft* class name, net.minecraft.server, org.bukkit.craftbukkit (5 refs, in own)
- uses 67 uncovered API members and 21 uncovered API types (top: org.bukkit.event.block.SignChangeEvent#getPlayer, org.bukkit.block.banner.PatternType#BASE, org.bukkit.event.block.SignChangeEvent#setLine, org.bukkit.block.Sign#getSide, org.bukkit.block.sign.SignSide#getLine)

| Uncovered member | Sites |
|---|---:|
| `org.bukkit.event.block.SignChangeEvent#getPlayer` | 23 |
| `org.bukkit.block.banner.PatternType#BASE` | 22 |
| `org.bukkit.event.block.SignChangeEvent#setLine` | 8 |
| `org.bukkit.block.Sign#getSide` | 7 |
| `org.bukkit.block.sign.SignSide#getLine` | 7 |
| `org.bukkit.block.Sign#getLine` | 5 |
| `org.bukkit.event.block.SignChangeEvent#getLine` | 4 |
| `org.bukkit.block.banner.PatternType#GRADIENT` | 3 |
| `org.bukkit.Server#getWorldContainer` | 2 |
| `org.bukkit.block.Block#getState` | 2 |
| `org.bukkit.block.banner.PatternType#STRIPE_DOWNLEFT` | 2 |
| `org.bukkit.block.banner.PatternType#STRIPE_MIDDLE` | 2 |
| `org.bukkit.block.sign.Side#BACK` | 2 |
| `org.bukkit.block.sign.Side#FRONT` | 2 |
| `org.bukkit.event.block.BlockFertilizeEvent#getBlock` | 2 |
| `org.bukkit.event.block.BlockFertilizeEvent#getPlayer` | 2 |
| `org.bukkit.event.block.BlockFertilizeEvent#setCancelled` | 2 |
| `org.bukkit.permissions.PermissibleBase#isPermissionSet` | 2 |
| `org.bukkit.permissions.PermissibleBase#recalculatePermissions` | 2 |
| `org.bukkit.plugin.InvalidDescriptionException#printStackTrace` | 2 |
| `org.bukkit.plugin.ServicesManager#getKnownServices` | 2 |
| `net.milkbowl.vault.permission.Permission#playerAddTransient` | 1 |
| `net.milkbowl.vault.permission.Permission#playerRemoveTransient` | 1 |
| `org.bukkit.Bukkit#getOnlineMode` | 1 |
| `org.bukkit.Server#getUpdateFolderFile` | 1 |
| `org.bukkit.block.banner.Pattern#<init>` | 1 |
| `org.bukkit.block.banner.PatternType#BORDER` | 1 |
| `org.bukkit.block.banner.PatternType#CIRCLE` | 1 |
| `org.bukkit.block.banner.PatternType#HALF_HORIZONTAL_BOTTOM` | 1 |
| `org.bukkit.block.banner.PatternType#STRIPE_BOTTOM` | 1 |
| `org.bukkit.block.banner.PatternType#TRIANGLE_BOTTOM` | 1 |
| `org.bukkit.block.banner.PatternType#TRIANGLE_TOP` | 1 |
| `org.bukkit.command.ConsoleCommandSender#getEffectivePermissions` | 1 |
| `org.bukkit.entity.Player#getFirstPlayed` | 1 |
| `org.bukkit.entity.Player#getLastPlayed` | 1 |
| `org.bukkit.entity.Player#setScoreboard` | 1 |
| `org.bukkit.event.block.BlockBurnEvent#getBlock` | 1 |
| `org.bukkit.event.block.BlockBurnEvent#setCancelled` | 1 |
| `org.bukkit.event.block.BlockExplodeEvent#getBlock` | 1 |
| `org.bukkit.event.block.BlockExplodeEvent#setCancelled` | 1 |
| ... 27 more | |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| reflection into internals | `Craft* class name` | own | 2 | yes |
| reflection into internals | `net.minecraft.server` | own | 1 | yes |
| reflection into internals | `org.bukkit.craftbukkit` | own | 2 | yes |

## 🔴 ProtocolLib 5.1.0

- file: `protocollib--ProtocolLib.jar`
- main: `com.comphenix.protocol.ProtocolLib`, api-version: 1.13
- classes: 3396 (530 own under `com.comphenix.protocol`, 2866 bundled), reflection calls: 352
- server internals (Mojang libs, Netty, Paper internals): com.mojang.datafixers.util, io.netty.buffer, io.netty.channel (95 refs, in own)
- reflection into internals: Craft* class name, com.mojang.authlib, getHandle() (31 refs, in own)
- Unsafe / jdk.internal: sun.misc (1 refs, in own)
- uses 41 uncovered API members and 21 uncovered API types (top: org.bukkit.conversations.ConversationContext#setSessionData, org.bukkit.conversations.Conversable#sendRawMessage, org.bukkit.conversations.ConversationContext#getSessionData, org.bukkit.conversations.ConversationAbandonedEvent#getContext, org.bukkit.block.Block#getState)

| Uncovered member | Sites |
|---|---:|
| `org.bukkit.conversations.ConversationContext#setSessionData` | 5 |
| `org.bukkit.conversations.Conversable#sendRawMessage` | 4 |
| `org.bukkit.conversations.ConversationContext#getSessionData` | 4 |
| `org.bukkit.conversations.ConversationAbandonedEvent#getContext` | 3 |
| `org.bukkit.block.Block#getState` | 2 |
| `org.bukkit.conversations.StringPrompt#<init>` | 2 |
| `org.bukkit.event.server.PluginDisableEvent#getPlugin` | 2 |
| `org.bukkit.plugin.ServicesManager#getKnownServices` | 2 |
| `org.bukkit.potion.PotionEffectType#getById` | 2 |
| `org.bukkit.potion.PotionEffectType#getId` | 2 |
| `org.bukkit.Bukkit#getOnlineMode` | 1 |
| `org.bukkit.OfflinePlayer#getFirstPlayed` | 1 |
| `org.bukkit.OfflinePlayer#getLastPlayed` | 1 |
| `org.bukkit.OfflinePlayer#hasPlayedBefore` | 1 |
| `org.bukkit.OfflinePlayer#isBanned` | 1 |
| `org.bukkit.OfflinePlayer#isWhitelisted` | 1 |
| `org.bukkit.Particle$DustTransition#<init>` | 1 |
| `org.bukkit.WorldType#getByName` | 1 |
| `org.bukkit.WorldType#getName` | 1 |
| `org.bukkit.conversations.Conversation#begin` | 1 |
| `org.bukkit.conversations.ConversationAbandonedEvent#gracefulExit` | 1 |
| `org.bukkit.conversations.ConversationCanceller#cancelBasedOnInput` | 1 |
| `org.bukkit.conversations.ConversationCanceller#clone` | 1 |
| `org.bukkit.conversations.ConversationCanceller#setConversation` | 1 |
| `org.bukkit.conversations.ConversationContext#getForWhom` | 1 |
| `org.bukkit.conversations.ConversationFactory#<init>` | 1 |
| `org.bukkit.conversations.ConversationFactory#addConversationAbandonedListener` | 1 |
| `org.bukkit.conversations.ConversationFactory#buildConversation` | 1 |
| `org.bukkit.conversations.ConversationFactory#withEscapeSequence` | 1 |
| `org.bukkit.conversations.ConversationFactory#withFirstPrompt` | 1 |
| `org.bukkit.conversations.ConversationFactory#withLocalEcho` | 1 |
| `org.bukkit.conversations.ExactMatchConversationCanceller#<init>` | 1 |
| `org.bukkit.conversations.Prompt#END_OF_CONVERSATION` | 1 |
| `org.bukkit.event.Cancellable#source` | 1 |
| `org.bukkit.plugin.PluginLoadOrder#POSTWORLD` | 1 |
| `org.bukkit.plugin.PluginLoadOrder#STARTUP` | 1 |
| `org.bukkit.potion.PotionEffect#<init>` | 1 |
| `org.bukkit.potion.PotionEffect#getAmplifier` | 1 |
| `org.bukkit.potion.PotionEffect#getDuration` | 1 |
| `org.bukkit.potion.PotionEffect#getType` | 1 |
| ... 1 more | |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| server internals (Mojang libs, Netty, Paper internals) | `com.mojang.datafixers.util` | own | 3 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `io.netty.buffer` | own | 22 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `io.netty.channel` | own | 50 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `io.netty.handler.codec` | own | 3 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `io.netty.util` | own | 9 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `io.netty.util.concurrent` | own | 8 | yes |
| reflection into internals | `Craft* class name` | own | 15 | yes |
| reflection into internals | `com.mojang.authlib` | own | 2 | yes |
| reflection into internals | `getHandle()` | own | 4 | yes |
| reflection into internals | `net.minecraft.entity` | own | 1 | yes |
| reflection into internals | `net.minecraft.network` | own | 2 | yes |
| reflection into internals | `net.minecraft.server` | own | 1 | yes |
| reflection into internals | `net.minecraft.util` | own | 3 | yes |
| reflection into internals | `org.bukkit.craftbukkit` | own | 3 | yes |
| bytecode instrumentation | `java.lang.instrument` | com.comphenix.net | 107 | no |
| Unsafe / jdk.internal | `sun.misc` | own | 1 | yes |

## 🟡 SetHome 6.3

- file: `sethome--sethome-6.3-SNAPSHOT.jar`
- main: `se.wilmer.setHome.SetHome`, api-version: 26.2
- classes: 13 (13 own under `se.wilmer.setHome`, 0 bundled), reflection calls: 0
- uses 1 uncovered API members and 0 uncovered API types (top: org.bukkit.OfflinePlayer#hasPlayedBefore)

| Uncovered member | Sites |
|---|---:|
| `org.bukkit.OfflinePlayer#hasPlayedBefore` | 2 |

## 🔴 voicechat 2.6.25

- file: `simplevoicechat--voicechat-bukkit-2.6.25.jar`
- main: `de.maxhenkel.voicechat.Voicechat`, api-version: 1.13
- softdepend: PlaceholderAPI, ViaVersion
- classes: 512 (512 own under `de.maxhenkel.voicechat`, 0 bundled), reflection calls: 66
- server internals (Mojang libs, Netty, Paper internals): io.netty.buffer, io.netty.handler.codec, io.netty.util (31 refs, in own)
- reflection into internals: Craft* class name, getHandle(), net.minecraft. (61 refs, in own)
- uses 14 uncovered API members and 3 uncovered API types (top: org.bukkit.plugin.messaging.Messenger#getOutgoingChannels, org.bukkit.event.player.PlayerCommandSendEvent#getCommands, com.destroystokyo.paper.event.brigadier.AsyncPlayerSendCommandsEvent#getCommandNode, com.destroystokyo.paper.event.brigadier.AsyncPlayerSendCommandsEvent#getPlayer, com.destroystokyo.paper.event.brigadier.AsyncPlayerSendCommandsEvent#hasFiredAsync)

| Uncovered member | Sites |
|---|---:|
| `org.bukkit.plugin.messaging.Messenger#getOutgoingChannels` | 3 |
| `org.bukkit.event.player.PlayerCommandSendEvent#getCommands` | 2 |
| `com.destroystokyo.paper.event.brigadier.AsyncPlayerSendCommandsEvent#getCommandNode` | 1 |
| `com.destroystokyo.paper.event.brigadier.AsyncPlayerSendCommandsEvent#getPlayer` | 1 |
| `com.destroystokyo.paper.event.brigadier.AsyncPlayerSendCommandsEvent#hasFiredAsync` | 1 |
| `com.destroystokyo.paper.event.brigadier.AsyncPlayerSendCommandsEvent#isAsynchronous` | 1 |
| `org.bukkit.Bukkit#getIp` | 1 |
| `org.bukkit.Bukkit#getOnlineMode` | 1 |
| `org.bukkit.Bukkit#getPort` | 1 |
| `org.bukkit.Bukkit#shutdown` | 1 |
| `org.bukkit.entity.Player#canSee` | 1 |
| `org.bukkit.event.player.PlayerCommandSendEvent#getPlayer` | 1 |
| `org.bukkit.plugin.ServicesManager#unregister` | 1 |
| `org.bukkit.plugin.messaging.Messenger#getIncomingChannels` | 1 |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| server internals (Mojang libs, Netty, Paper internals) | `io.netty.buffer` | own | 26 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `io.netty.handler.codec` | own | 3 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `io.netty.util` | own | 2 | yes |
| reflection into internals | `Craft* class name` | own | 11 | yes |
| reflection into internals | `getHandle()` | own | 10 | yes |
| reflection into internals | `net.minecraft.` | own | 1 | yes |
| reflection into internals | `net.minecraft.commands` | own | 10 | yes |
| reflection into internals | `net.minecraft.core` | own | 2 | yes |
| reflection into internals | `net.minecraft.network` | own | 21 | yes |
| reflection into internals | `net.minecraft.resources` | own | 1 | yes |
| reflection into internals | `net.minecraft.server` | own | 3 | yes |
| reflection into internals | `org.bukkit.craftbukkit` | own | 2 | yes |

## 🔴 SkBee 3.26.0

- file: `skbee--SkBee-3.26.0.jar`
- main: `com.shanebeestudios.skbee.SkBee`, api-version: 1.21.11
- depend: Skript
- softdepend: SkQuery
- classes: 1029 (1029 own under `com.shanebeestudios.skbee`, 0 bundled), reflection calls: 140
- server internals (Mojang libs, Netty, Paper internals): com.mojang.authlib, com.mojang.authlib.properties, com.mojang.datafixers (19 refs, in own)
- reflection into internals: Craft* class name, com.mojang.authlib, getHandle() (108 refs, in own)
- uses 1263 uncovered API members and 418 uncovered API types (top: org.bukkit.block.Block#getState, org.bukkit.inventory.ItemStack#setData, org.bukkit.inventory.ItemStack#hasData, org.bukkit.inventory.ItemStack#getData, io.papermc.paper.registry.TypedKey#create)

| Uncovered member | Sites |
|---|---:|
| `org.bukkit.block.Block#getState` | 41 |
| `org.bukkit.inventory.ItemStack#setData` | 25 |
| `org.bukkit.inventory.ItemStack#hasData` | 22 |
| `org.bukkit.inventory.ItemStack#getData` | 21 |
| `io.papermc.paper.registry.TypedKey#create` | 20 |
| `io.papermc.paper.registry.RegistryKey#BLOCK` | 18 |
| `org.bukkit.Material#isItem` | 18 |
| `io.papermc.paper.registry.RegistryAccess#getRegistry` | 16 |
| `io.papermc.paper.registry.RegistryAccess#registryAccess` | 16 |
| `org.bukkit.Registry#getTag` | 13 |
| `io.papermc.paper.registry.RegistryKey#DAMAGE_TYPE` | 12 |
| `io.papermc.paper.registry.tag.TagKey#registryKey` | 12 |
| `org.bukkit.Material#isBlock` | 12 |
| `org.bukkit.Registry#get` | 12 |
| `org.bukkit.Bukkit#addRecipe` | 11 |
| `org.bukkit.inventory.ItemStack#unsetData` | 11 |
| `io.papermc.paper.connection.PlayerConfigurationConnection#getProfile` | 10 |
| `org.bukkit.block.CreatureSpawner#update` | 10 |
| `org.bukkit.inventory.ItemStack#resetData` | 10 |
| `io.papermc.paper.datacomponent.DataComponentTypes#REPAIR_COST` | 9 |
| `io.papermc.paper.registry.set.RegistrySet#keySet` | 9 |
| `org.bukkit.Bukkit#getServerTickManager` | 9 |
| `org.bukkit.Bukkit#removeRecipe` | 9 |
| `org.bukkit.attribute.AttributeModifier#getKey` | 9 |
| `com.destroystokyo.paper.profile.PlayerProfile#getId` | 8 |
| `io.papermc.paper.datacomponent.DataComponentTypes#CONTAINER` | 8 |
| `io.papermc.paper.registry.RegistryKey#ENTITY_TYPE` | 8 |
| `io.papermc.paper.registry.RegistryKey#ITEM` | 8 |
| `io.papermc.paper.registry.data.dialog.DialogBase$Builder#externalTitle` | 8 |
| `io.papermc.paper.registry.data.dialog.DialogBase$DialogAfterAction#CLOSE` | 8 |
| `io.papermc.paper.command.brigadier.Commands#literal` | 7 |
| `org.bukkit.Statistic#getType` | 7 |
| `org.bukkit.event.block.BlockExplodeEvent#blockList` | 7 |
| `io.papermc.paper.command.brigadier.CommandSourceStack#getSender` | 6 |
| `io.papermc.paper.event.player.AsyncChatEvent#viewers` | 6 |
| `io.papermc.paper.registry.RegistryKey#MOB_EFFECT` | 6 |
| `org.bukkit.inventory.RecipeChoice$ExactChoice#<init>` | 6 |
| `org.bukkit.inventory.recipe.CraftingBookCategory#name` | 6 |
| `com.destroystokyo.paper.event.player.PlayerPickupExperienceEvent#getExperienceOrb` | 5 |
| `io.papermc.paper.connection.PlayerConfigurationConnection#getAudience` | 5 |
| ... 1223 more | |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| server internals (Mojang libs, Netty, Paper internals) | `com.mojang.authlib` | own | 3 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `com.mojang.authlib.properties` | own | 2 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `com.mojang.datafixers` | own | 3 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `com.mojang.serialization` | own | 11 | yes |
| reflection into internals | `Craft* class name` | own | 13 | yes |
| reflection into internals | `com.mojang.authlib` | own | 1 | yes |
| reflection into internals | `getHandle()` | own | 4 | yes |
| reflection into internals | `net.minecraft.` | own | 1 | yes |
| reflection into internals | `net.minecraft.core` | own | 11 | yes |
| reflection into internals | `net.minecraft.entity` | own | 2 | yes |
| reflection into internals | `net.minecraft.item` | own | 1 | yes |
| reflection into internals | `net.minecraft.nbt` | own | 31 | yes |
| reflection into internals | `net.minecraft.network` | own | 3 | yes |
| reflection into internals | `net.minecraft.resources` | own | 2 | yes |
| reflection into internals | `net.minecraft.server` | own | 6 | yes |
| reflection into internals | `net.minecraft.tileentity` | own | 1 | yes |
| reflection into internals | `net.minecraft.util` | own | 8 | yes |
| reflection into internals | `net.minecraft.world` | own | 23 | yes |
| reflection into internals | `org.bukkit.craftbukkit` | own | 1 | yes |

## 🔴 SkinsRestorer 15.12.6

- file: `skinsrestorer--SkinsRestorer.jar`
- main: `net.skinsrestorer.bukkit.SRBukkitBootstrap`, api-version: 1.13
- softdepend: ProtocolLib, ViaVersion, ViaBackwards, ProtocolSupport, MundoSK, floodgate, PlaceholderAPI, MiniPlaceholders
- classes: 4410 (79 own under `net.skinsrestorer.bukkit`, 4331 bundled), reflection calls: 553
- NMS (net.minecraft): net.minecraft.core, net.minecraft.network.protocol, net.minecraft.resources (305 refs, in own)
- CraftBukkit: org.bukkit.craftbukkit.entity, org.bukkit.craftbukkit.v1_18_R1, org.bukkit.craftbukkit.v1_18_R2 (22 refs, in own)
- server internals (Mojang libs, Netty, Paper internals): com.mojang.authlib, com.mojang.authlib.minecraft, com.mojang.authlib.properties (36 refs, in net.skinsrestorer.shadow)
- reflection into internals: Craft* class name, com.mojang.authlib, getHandle() (55 refs, in net.skinsrestorer.shadow, own)
- uses 474 uncovered API members and 159 uncovered API types (top: org.bukkit.block.Block#getState, org.bukkit.Bukkit#getMultiPaperDataStorage, org.bukkit.block.BlockState#getData, org.bukkit.Bukkit#getMultiPaperNotificationManager, org.bukkit.Registry#get)

| Uncovered member | Sites |
|---|---:|
| `org.bukkit.block.Block#getState` | 23 |
| `org.bukkit.Bukkit#getMultiPaperDataStorage` | 18 |
| `org.bukkit.block.BlockState#getData` | 14 |
| `org.bukkit.Bukkit#getMultiPaperNotificationManager` | 12 |
| `org.bukkit.Registry#get` | 8 |
| `org.bukkit.block.BlockState#setRawData` | 8 |
| `io.papermc.paper.command.brigadier.CommandSourceStack#getSender` | 7 |
| `org.bukkit.SkullType#ordinal` | 7 |
| `org.bukkit.TreeSpecies#ordinal` | 7 |
| `org.bukkit.block.BlockState#update` | 7 |
| `org.bukkit.WorldBorder#getCenter` | 5 |
| `org.bukkit.entity.Player#canSee` | 5 |
| `org.bukkit.entity.Player#getLastPlayed` | 5 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#getBuffer` | 4 |
| `org.bukkit.block.Block#breakNaturally` | 4 |
| `org.bukkit.potion.PotionEffect#getType` | 4 |
| `org.bukkit.potion.PotionType#name` | 4 |
| `org.bukkit.potion.PotionType#valueOf` | 4 |
| `com.destroystokyo.paper.event.brigadier.CommandRegisteredEvent#getCommandLabel` | 3 |
| `com.destroystokyo.paper.profile.PlayerProfile#getProperties` | 3 |
| `org.bukkit.Instrument#BASS_DRUM` | 3 |
| `org.bukkit.Material#getMaxDurability` | 3 |
| `org.bukkit.Material#isBlock` | 3 |
| `org.bukkit.Material#isItem` | 3 |
| `org.bukkit.Note#getId` | 3 |
| `org.bukkit.TreeSpecies#getData` | 3 |
| `org.bukkit.WorldBorder#getDamageBuffer` | 3 |
| `org.bukkit.WorldBorder#getSize` | 3 |
| `org.bukkit.WorldBorder#getWarningDistance` | 3 |
| `org.bukkit.WorldBorder#getWarningTime` | 3 |
| `org.bukkit.block.BlockState#update` | 3 |
| `org.bukkit.command.BlockCommandSender#getBlock` | 3 |
| `org.bukkit.command.PluginIdentifiableCommand#getPlugin` | 3 |
| `org.bukkit.entity.Player#stopSound` | 3 |
| `org.bukkit.event.server.PluginDisableEvent#getPlugin` | 3 |
| `org.bukkit.inventory.ItemStack#getData` | 3 |
| `org.bukkit.inventory.ItemStack#getMaxStackSize` | 3 |
| `org.bukkit.inventory.meta.BlockStateMeta#setBlockState` | 3 |
| `org.bukkit.inventory.meta.SpawnEggMeta#getSpawnedType` | 3 |
| `org.bukkit.inventory.meta.components.CustomModelDataComponent#setColors` | 3 |
| ... 434 more | |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| NMS (net.minecraft) | `net.minecraft.core` | own | 1 | yes |
| NMS (net.minecraft) | `net.minecraft.network.protocol` | own | 125 | yes |
| NMS (net.minecraft) | `net.minecraft.resources` | own | 8 | yes |
| NMS (net.minecraft) | `net.minecraft.server` | own | 22 | yes |
| NMS (net.minecraft) | `net.minecraft.server.level` | own | 52 | yes |
| NMS (net.minecraft) | `net.minecraft.server.network` | own | 22 | yes |
| NMS (net.minecraft) | `net.minecraft.server.players` | own | 22 | yes |
| NMS (net.minecraft) | `net.minecraft.world.effect` | own | 22 | yes |
| NMS (net.minecraft) | `net.minecraft.world.level` | own | 31 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.entity` | own | 4 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_18_R1` | own | 1 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_18_R2` | own | 1 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_19_R1` | own | 3 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_19_R2` | own | 1 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_19_R3` | own | 1 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_20_R1` | own | 1 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_20_R2` | own | 1 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_20_R3` | own | 1 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_20_R4` | own | 1 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_21_R1` | own | 1 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_21_R2` | own | 1 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_21_R3` | own | 1 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_21_R4` | own | 1 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_21_R5` | own | 1 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_21_R6` | own | 1 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_21_R7` | own | 1 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `com.mojang.authlib` | net.skinsrestorer.shadow | 15 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `com.mojang.authlib.minecraft` | net.skinsrestorer.shadow | 1 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `com.mojang.authlib.properties` | net.skinsrestorer.shadow | 18 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `com.mojang.authlib.yggdrasil` | net.skinsrestorer.shadow | 2 | yes |

## 🔴 Skript 2.17.1

- file: `skript--Skript-2.17.1.jar`
- main: `ch.njol.skript.Skript`, api-version: 1.21.5
- softdepend: SQLibrary, Vault, WorldGuard, Residence, PreciousStones, GriefPrevention
- classes: 2617 (1629 own under `ch.njol.skript`, 988 bundled), reflection calls: 106
- reflection into internals: org.bukkit.craftbukkit (2 refs, in own)
- uses 1837 uncovered API members and 613 uncovered API types (top: org.bukkit.block.Block#getState, org.bukkit.block.BlockState#getBlock, org.bukkit.entity.Item#getItemStack, org.bukkit.potion.PotionEffect#getType, org.bukkit.Material#isBlock)

| Uncovered member | Sites |
|---|---:|
| `org.bukkit.block.Block#getState` | 73 |
| `org.bukkit.block.BlockState#getBlock` | 62 |
| `org.bukkit.entity.Item#getItemStack` | 26 |
| `org.bukkit.potion.PotionEffect#getType` | 17 |
| `org.bukkit.Material#isBlock` | 16 |
| `org.bukkit.event.player.PlayerFishEvent#getHook` | 15 |
| `org.bukkit.block.BlockState#getBlockData` | 12 |
| `org.bukkit.Material#isItem` | 11 |
| `org.bukkit.World#setGameRule` | 11 |
| `org.bukkit.entity.LivingEntity#removePotionEffect` | 11 |
| `org.bukkit.event.enchantment.PrepareItemEnchantEvent#getOffers` | 11 |
| `org.bukkit.Tag#getKey` | 10 |
| `org.bukkit.Tag#isTagged` | 10 |
| `com.destroystokyo.paper.ParticleBuilder#particle` | 9 |
| `io.papermc.paper.registry.set.RegistryKeySet#resolve` | 9 |
| `org.bukkit.block.sign.Side#FRONT` | 9 |
| `org.bukkit.help.HelpMap#getHelpTopic` | 9 |
| `org.bukkit.inventory.Inventory#getViewers` | 9 |
| `io.papermc.paper.event.player.AsyncChatEvent#viewers` | 8 |
| `org.bukkit.FireworkEffect#getColors` | 8 |
| `org.bukkit.Registry#PARTICLE_TYPE` | 8 |
| `org.bukkit.WorldBorder#getWarningDistance` | 8 |
| `org.bukkit.WorldBorder#setWarningDistance` | 8 |
| `org.bukkit.block.Sign#getSide` | 8 |
| `org.bukkit.boss.BarFlag#ordinal` | 8 |
| `org.bukkit.entity.Display#getTransformation` | 8 |
| `org.bukkit.entity.ExperienceOrb#getExperience` | 8 |
| `org.bukkit.entity.LivingEntity#addPotionEffect` | 8 |
| `org.bukkit.event.entity.EntityChangeBlockEvent#getEntity` | 8 |
| `org.bukkit.inventory.EquipmentSlot#BODY` | 8 |
| `org.bukkit.inventory.ItemFactory#getItemMeta` | 8 |
| `org.bukkit.persistence.ListPersistentDataTypeProvider#dataContainers` | 8 |
| `com.destroystokyo.paper.ParticleBuilder#count` | 7 |
| `io.papermc.paper.datacomponent.DataComponentTypes#EQUIPPABLE` | 7 |
| `org.bukkit.Bukkit#getHelpMap` | 7 |
| `org.bukkit.block.BlockState#setType` | 7 |
| `org.bukkit.entity.Display#setBrightness` | 7 |
| `org.bukkit.entity.Display$Brightness#<init>` | 7 |
| `org.bukkit.entity.Entity#getVehicle` | 7 |
| `org.bukkit.entity.Entity#setVelocity` | 7 |
| ... 1797 more | |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| reflection into internals | `org.bukkit.craftbukkit` | own | 2 | yes |

## 🔴 squaremap 1.4.3

- file: `squaremap--squaremap-paper-mc26.3-1.4.3.jar`
- main: `xyz.jpenilla.squaremap.paper.SquaremapPaperBootstrap`, api-version: 26.3
- classes: 4456 (258 own under `xyz.jpenilla.squaremap`, 4198 bundled), reflection calls: 263
- NMS (net.minecraft): net.minecraft, net.minecraft.client, net.minecraft.client.color (343 refs, in own)
- CraftBukkit: org.bukkit.craftbukkit, org.bukkit.craftbukkit.entity, org.bukkit.craftbukkit.persistence (8 refs, in own)
- server internals (Mojang libs, Netty, Paper internals): ca.spottedleaf.concurrentutil.executor, ca.spottedleaf.concurrentutil.lock, ca.spottedleaf.concurrentutil.util (48 refs, in own)
- reflection into internals: Craft* class name, getHandle(), net.minecraft. (24 refs, in own, squaremap.libraries.org.incendo)
- uses 42 uncovered API members and 40 uncovered API types (top: io.papermc.paper.command.brigadier.CommandSourceStack#getSender, io.papermc.paper.command.brigadier.CommandSourceStack#getExecutor, org.bukkit.command.BlockCommandSender#getBlock, org.bukkit.entity.Player#canSee, io.papermc.paper.command.brigadier.Commands#getDispatcher)

| Uncovered member | Sites |
|---|---:|
| `io.papermc.paper.command.brigadier.CommandSourceStack#getSender` | 11 |
| `io.papermc.paper.command.brigadier.CommandSourceStack#getExecutor` | 4 |
| `org.bukkit.command.BlockCommandSender#getBlock` | 4 |
| `org.bukkit.entity.Player#canSee` | 3 |
| `io.papermc.paper.command.brigadier.Commands#getDispatcher` | 2 |
| `io.papermc.paper.plugin.lifecycle.event.LifecycleEventManager#registerEventHandler` | 2 |
| `io.papermc.paper.plugin.lifecycle.event.types.LifecycleEvents#COMMANDS` | 2 |
| `org.bukkit.Material#getMaxStackSize` | 2 |
| `org.bukkit.command.PluginIdentifiableCommand#getPlugin` | 2 |
| `io.papermc.paper.command.brigadier.CommandRegistrationFlag#FLATTEN_ALIASES` | 1 |
| `io.papermc.paper.command.brigadier.Commands#getClass` | 1 |
| `io.papermc.paper.command.brigadier.Commands#registerWithFlags` | 1 |
| `io.papermc.paper.event.packet.PlayerChunkLoadEvent#getChunk` | 1 |
| `io.papermc.paper.plugin.bootstrap.BootstrapContext#getLifecycleManager` | 1 |
| `io.papermc.paper.plugin.bootstrap.BootstrapContext#getPluginMeta` | 1 |
| `io.papermc.paper.plugin.lifecycle.event.registrar.ReloadableRegistrarEvent#registrar` | 1 |
| `io.papermc.paper.text.PaperComponents#flattener` | 1 |
| `io.papermc.paper.world.saveddata.PaperLevelOverrides#getRespawnData` | 1 |
| `org.bukkit.Bukkit#getOnlineMode` | 1 |
| `org.bukkit.Material#isItem` | 1 |
| `org.bukkit.World#hasCeiling` | 1 |
| `org.bukkit.block.Block#getState` | 1 |
| `org.bukkit.block.BlockState#getLocation` | 1 |
| `org.bukkit.command.BlockCommandSender#getServer` | 1 |
| `org.bukkit.event.block.BlockFromToEvent#getToBlock` | 1 |
| `org.bukkit.event.block.BlockPistonExtendEvent#getBlock` | 1 |
| `org.bukkit.event.block.BlockPistonExtendEvent#getBlocks` | 1 |
| `org.bukkit.event.block.BlockPistonRetractEvent#getBlock` | 1 |
| `org.bukkit.event.block.BlockPistonRetractEvent#getBlocks` | 1 |
| `org.bukkit.event.block.FluidLevelChangeEvent#getBlock` | 1 |
| `org.bukkit.event.block.FluidLevelChangeEvent#getNewData` | 1 |
| `org.bukkit.event.entity.EntityChangeBlockEvent#getBlock` | 1 |
| `org.bukkit.event.entity.EntityExplodeEvent#blockList` | 1 |
| `org.bukkit.event.entity.EntityExplodeEvent#getLocation` | 1 |
| `org.bukkit.event.server.PluginDisableEvent#getPlugin` | 1 |
| `org.bukkit.event.world.ChunkPopulateEvent#getChunk` | 1 |
| `org.bukkit.event.world.StructureGrowEvent#getBlocks` | 1 |
| `org.bukkit.event.world.StructureGrowEvent#getWorld` | 1 |
| `org.bukkit.event.world.WorldLoadEvent#getWorld` | 1 |
| `org.bukkit.event.world.WorldUnloadEvent#getWorld` | 1 |
| ... 2 more | |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| NMS (net.minecraft) | `net.minecraft` | own | 5 | yes |
| NMS (net.minecraft) | `net.minecraft.client` | own | 1 | yes |
| NMS (net.minecraft) | `net.minecraft.client.color` | own | 1 | yes |
| NMS (net.minecraft) | `net.minecraft.client.renderer` | own | 4 | yes |
| NMS (net.minecraft) | `net.minecraft.client.resources` | own | 3 | yes |
| NMS (net.minecraft) | `net.minecraft.commands.arguments` | own | 1 | yes |
| NMS (net.minecraft) | `net.minecraft.core` | own | 43 | yes |
| NMS (net.minecraft) | `net.minecraft.core.registries` | own | 4 | yes |
| NMS (net.minecraft) | `net.minecraft.nbt` | own | 14 | yes |
| NMS (net.minecraft) | `net.minecraft.network.chat` | own | 3 | yes |
| NMS (net.minecraft) | `net.minecraft.resources` | own | 24 | yes |
| NMS (net.minecraft) | `net.minecraft.server` | own | 4 | yes |
| NMS (net.minecraft) | `net.minecraft.server.level` | own | 68 | yes |
| NMS (net.minecraft) | `net.minecraft.tags` | own | 2 | yes |
| NMS (net.minecraft) | `net.minecraft.util` | own | 13 | yes |
| NMS (net.minecraft) | `net.minecraft.world.entity` | own | 8 | yes |
| NMS (net.minecraft) | `net.minecraft.world.item` | own | 1 | yes |
| NMS (net.minecraft) | `net.minecraft.world.level` | own | 143 | yes |
| NMS (net.minecraft) | `net.minecraft.world.phys` | own | 1 | yes |
| CraftBukkit | `org.bukkit.craftbukkit` | own | 4 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.entity` | own | 3 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.persistence` | own | 1 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `ca.spottedleaf.concurrentutil.executor` | own | 6 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `ca.spottedleaf.concurrentutil.lock` | own | 2 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `ca.spottedleaf.concurrentutil.util` | own | 1 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `ca.spottedleaf.moonrise.common` | own | 2 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `ca.spottedleaf.moonrise.libs` | own | 9 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `ca.spottedleaf.moonrise.patches` | own | 13 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `com.mojang.authlib` | own | 2 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `com.mojang.serialization` | own | 13 | yes |

## 🔴 TAB 6.2.1

- file: `tab--TAB_v6.2.1.jar`
- main: `me.neznamy.tab.platforms.bukkit.BukkitTAB`, api-version: 1.13
- softdepend: PlaceholderAPI, LuckPerms, LibsDisguises, ViaVersion, ViaRewind, floodgate, Vault
- classes: 4090 (622 own under `me.neznamy.tab`, 3468 bundled), reflection calls: 218
- NMS (net.minecraft): net.minecraft, net.minecraft.commands, net.minecraft.core (1568 refs, in own)
- CraftBukkit: org.bukkit.craftbukkit.entity, org.bukkit.craftbukkit.v1_12_R1, org.bukkit.craftbukkit.v1_16_R3 (78 refs, in own)
- server internals (Mojang libs, Netty, Paper internals): com.mojang.authlib, com.mojang.authlib.properties, com.mojang.logging (209 refs, in own)
- uses 14 uncovered API members and 5 uncovered API types (top: net.milkbowl.vault.permission.Permission#getName, org.bukkit.Bukkit#getOnlineMode, org.bukkit.Statistic#DEATHS, org.bukkit.attribute.AttributeInstance#setBaseValue, org.bukkit.boss.BossBar#setStyle)

| Uncovered member | Sites |
|---|---:|
| `net.milkbowl.vault.permission.Permission#getName` | 2 |
| `org.bukkit.Bukkit#getOnlineMode` | 1 |
| `org.bukkit.Statistic#DEATHS` | 1 |
| `org.bukkit.attribute.AttributeInstance#setBaseValue` | 1 |
| `org.bukkit.boss.BossBar#setStyle` | 1 |
| `org.bukkit.entity.Player#getAttribute` | 1 |
| `org.bukkit.entity.Player#getStatistic` | 1 |
| `org.bukkit.entity.Player#hasLineOfSight` | 1 |
| `org.bukkit.entity.Player#hasPotionEffect` | 1 |
| `org.bukkit.entity.Player#hidePlayer` | 1 |
| `org.bukkit.entity.Player#hidePlayer` | 1 |
| `org.bukkit.entity.Player#showPlayer` | 1 |
| `org.bukkit.entity.Player#showPlayer` | 1 |
| `org.bukkit.potion.PotionEffectType#INVISIBILITY` | 1 |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| NMS (net.minecraft) | `net.minecraft` | own | 29 | yes |
| NMS (net.minecraft) | `net.minecraft.commands` | own | 25 | yes |
| NMS (net.minecraft) | `net.minecraft.core` | own | 3 | yes |
| NMS (net.minecraft) | `net.minecraft.network` | own | 28 | yes |
| NMS (net.minecraft) | `net.minecraft.network.chat` | own | 337 | yes |
| NMS (net.minecraft) | `net.minecraft.network.protocol` | own | 420 | yes |
| NMS (net.minecraft) | `net.minecraft.resources` | own | 35 | yes |
| NMS (net.minecraft) | `net.minecraft.server` | own | 17 | yes |
| NMS (net.minecraft) | `net.minecraft.server.level` | own | 105 | yes |
| NMS (net.minecraft) | `net.minecraft.server.network` | own | 93 | yes |
| NMS (net.minecraft) | `net.minecraft.server.permissions` | own | 12 | yes |
| NMS (net.minecraft) | `net.minecraft.server.players` | own | 3 | yes |
| NMS (net.minecraft) | `net.minecraft.server.v1_12_R1` | own | 36 | yes |
| NMS (net.minecraft) | `net.minecraft.server.v1_16_R3` | own | 39 | yes |
| NMS (net.minecraft) | `net.minecraft.server.v1_7_R4` | own | 21 | yes |
| NMS (net.minecraft) | `net.minecraft.server.v1_8_R3` | own | 37 | yes |
| NMS (net.minecraft) | `net.minecraft.stats` | own | 12 | yes |
| NMS (net.minecraft) | `net.minecraft.world` | own | 9 | yes |
| NMS (net.minecraft) | `net.minecraft.world.entity` | own | 20 | yes |
| NMS (net.minecraft) | `net.minecraft.world.item` | own | 10 | yes |
| NMS (net.minecraft) | `net.minecraft.world.level` | own | 43 | yes |
| NMS (net.minecraft) | `net.minecraft.world.scores` | own | 234 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.entity` | own | 24 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_12_R1` | own | 3 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_16_R3` | own | 3 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_17_R1` | own | 3 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_18_R2` | own | 3 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_19_R3` | own | 3 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_20_R1` | own | 3 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_20_R2` | own | 3 | yes |

## 🔴 TabTPS 1.4.3

- file: `tabtps--tabtps-paper-1.4.3.jar`
- main: `xyz.jpenilla.tabtps.paper.TabTPSPlugin`, api-version: 1.13
- softdepend: PlaceholderAPI, ViaVersion
- classes: 2023 (85 own under `xyz.jpenilla.tabtps`, 1938 bundled), reflection calls: 237
- server internals (Mojang libs, Netty, Paper internals): io.netty.buffer (5 refs, in xyz.jpenilla.tabtps.lib.net.kyori)
- reflection into internals: Craft* class name, getHandle(), net.minecraft. (56 refs, in own, xyz.jpenilla.tabtps.lib.net.kyori, xyz.jpenilla.tabtps.lib.org.incendo)
- uses 68 uncovered API members and 35 uncovered API types (top: io.papermc.paper.command.brigadier.CommandSourceStack#getSender, com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#getBuffer, com.destroystokyo.paper.event.brigadier.CommandRegisteredEvent#getCommandLabel, org.bukkit.command.BlockCommandSender#getBlock, org.bukkit.command.PluginIdentifiableCommand#getPlugin)

| Uncovered member | Sites |
|---|---:|
| `io.papermc.paper.command.brigadier.CommandSourceStack#getSender` | 7 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#getBuffer` | 4 |
| `com.destroystokyo.paper.event.brigadier.CommandRegisteredEvent#getCommandLabel` | 3 |
| `org.bukkit.command.BlockCommandSender#getBlock` | 3 |
| `org.bukkit.command.PluginIdentifiableCommand#getPlugin` | 3 |
| `org.bukkit.entity.Player#canSee` | 3 |
| `com.destroystokyo.paper.event.brigadier.CommandRegisteredEvent#getCommand` | 2 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent$Completion#completion` | 2 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent$Completion#completion` | 2 |
| `io.papermc.paper.command.brigadier.Commands#getDispatcher` | 2 |
| `io.papermc.paper.plugin.lifecycle.event.LifecycleEventManager#registerEventHandler` | 2 |
| `io.papermc.paper.plugin.lifecycle.event.types.LifecycleEvents#COMMANDS` | 2 |
| `io.papermc.paper.registry.RegistryAccess#getRegistry` | 2 |
| `io.papermc.paper.registry.RegistryAccess#registryAccess` | 2 |
| `org.bukkit.Material#getMaxStackSize` | 2 |
| `org.bukkit.World#isChunkGenerated` | 2 |
| `org.bukkit.block.Block#getState` | 2 |
| `org.bukkit.entity.Player#getBedSpawnLocation` | 2 |
| `com.destroystokyo.paper.Title#builder` | 1 |
| `com.destroystokyo.paper.Title$Builder#build` | 1 |
| `com.destroystokyo.paper.Title$Builder#fadeIn` | 1 |
| `com.destroystokyo.paper.Title$Builder#fadeOut` | 1 |
| `com.destroystokyo.paper.Title$Builder#stay` | 1 |
| `com.destroystokyo.paper.Title$Builder#subtitle` | 1 |
| `com.destroystokyo.paper.Title$Builder#title` | 1 |
| `com.destroystokyo.paper.brigadier.BukkitBrigadierCommandSource#getBukkitSender` | 1 |
| `com.destroystokyo.paper.event.brigadier.CommandRegisteredEvent#getLiteral` | 1 |
| `com.destroystokyo.paper.event.brigadier.CommandRegisteredEvent#setLiteral` | 1 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#completions` | 1 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#getSender` | 1 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#setCompletions` | 1 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#setHandled` | 1 |
| `io.papermc.paper.brigadier.PaperBrigadier#componentFromMessage` | 1 |
| `io.papermc.paper.command.brigadier.CommandRegistrationFlag#FLATTEN_ALIASES` | 1 |
| `io.papermc.paper.command.brigadier.CommandSourceStack#getExecutor` | 1 |
| `io.papermc.paper.command.brigadier.Commands#getClass` | 1 |
| `io.papermc.paper.command.brigadier.Commands#registerWithFlags` | 1 |
| `io.papermc.paper.command.brigadier.MessageComponentSerializer#deserializeOrNull` | 1 |
| `io.papermc.paper.command.brigadier.MessageComponentSerializer#message` | 1 |
| `io.papermc.paper.plugin.bootstrap.BootstrapContext#getLifecycleManager` | 1 |
| ... 28 more | |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| server internals (Mojang libs, Netty, Paper internals) | `io.netty.buffer` | xyz.jpenilla.tabtps.lib.net.kyori | 5 | yes |
| reflection into internals | `Craft* class name` | own | 2 | yes |
| reflection into internals | `Craft* class name` | xyz.jpenilla.tabtps.lib.net.kyori | 9 | yes |
| reflection into internals | `Craft* class name` | xyz.jpenilla.tabtps.lib.org.incendo | 5 | yes |
| reflection into internals | `Craft* class name` | xyz.jpenilla.tabtps.lib.xyz.jpenilla | 1 | yes |
| reflection into internals | `getHandle()` | own | 1 | yes |
| reflection into internals | `getHandle()` | xyz.jpenilla.tabtps.lib.net.kyori | 2 | yes |
| reflection into internals | `getHandle()` | xyz.jpenilla.tabtps.lib.org.incendo | 1 | yes |
| reflection into internals | `net.minecraft.` | xyz.jpenilla.tabtps.lib.net.kyori | 1 | yes |
| reflection into internals | `net.minecraft.` | xyz.jpenilla.tabtps.lib.org.incendo | 1 | yes |
| reflection into internals | `net.minecraft.SharedConstants` | xyz.jpenilla.tabtps.lib.org.incendo | 1 | yes |
| reflection into internals | `net.minecraft.SharedConstants` | xyz.jpenilla.tabtps.lib.xyz.jpenilla | 1 | yes |
| reflection into internals | `net.minecraft.commands` | xyz.jpenilla.tabtps.lib.org.incendo | 1 | yes |
| reflection into internals | `net.minecraft.nbt` | xyz.jpenilla.tabtps.lib.org.incendo | 2 | yes |
| reflection into internals | `net.minecraft.resources` | xyz.jpenilla.tabtps.lib.org.incendo | 5 | yes |
| reflection into internals | `net.minecraft.server` | own | 8 | yes |
| reflection into internals | `net.minecraft.server` | xyz.jpenilla.tabtps.lib.net.kyori | 1 | yes |
| reflection into internals | `net.minecraft.server` | xyz.jpenilla.tabtps.lib.org.incendo | 3 | yes |
| reflection into internals | `net.minecraft.server` | xyz.jpenilla.tabtps.lib.xyz.jpenilla | 1 | yes |
| reflection into internals | `net.minecraft.world` | xyz.jpenilla.tabtps.lib.org.incendo | 2 | yes |
| reflection into internals | `org.bukkit.craftbukkit` | own | 2 | yes |
| reflection into internals | `org.bukkit.craftbukkit` | xyz.jpenilla.tabtps.lib.net.kyori | 2 | yes |
| reflection into internals | `org.bukkit.craftbukkit` | xyz.jpenilla.tabtps.lib.org.incendo | 2 | yes |
| reflection into internals | `org.bukkit.craftbukkit` | xyz.jpenilla.tabtps.lib.xyz.jpenilla | 2 | yes |

## 🟡 ToolStats 2.0.7

- file: `toolstats--toolstats-2.0.7.jar`
- main: `lol.hyper.toolstats.ToolStats`, api-version: 26.1.1
- softdepend: RoseStacker
- classes: 48 (48 own under `lol.hyper.toolstats`, 0 bundled), reflection calls: 0
- uses 80 uncovered API members and 46 uncovered API types (top: org.bukkit.inventory.ShapedRecipe#setIngredient, org.bukkit.event.inventory.PrepareAnvilEvent#setResult, org.bukkit.inventory.ShapedRecipe#<init>, org.bukkit.inventory.ShapedRecipe#shape, org.bukkit.entity.Trident#getItemStack)

| Uncovered member | Sites |
|---|---:|
| `org.bukkit.inventory.ShapedRecipe#setIngredient` | 32 |
| `org.bukkit.event.inventory.PrepareAnvilEvent#setResult` | 20 |
| `org.bukkit.inventory.ShapedRecipe#<init>` | 17 |
| `org.bukkit.inventory.ShapedRecipe#shape` | 17 |
| `org.bukkit.entity.Trident#getItemStack` | 6 |
| `org.bukkit.entity.Trident#setItemStack` | 5 |
| `org.bukkit.entity.Item#getItemStack` | 4 |
| `org.bukkit.entity.Item#setItemStack` | 4 |
| `org.bukkit.entity.minecart.StorageMinecart#getInventory` | 3 |
| `org.bukkit.event.inventory.PrepareAnvilEvent#getView` | 3 |
| `org.bukkit.event.player.PlayerFishEvent#getCaught` | 3 |
| `org.bukkit.event.world.ChunkPopulateEvent#getChunk` | 3 |
| `org.bukkit.inventory.view.AnvilView#setRepairCost` | 3 |
| `io.papermc.paper.command.brigadier.CommandSourceStack#getSender` | 2 |
| `io.papermc.paper.datacomponent.item.CustomModelData#customModelData` | 2 |
| `io.papermc.paper.datacomponent.item.CustomModelData$Builder#build` | 2 |
| `org.bukkit.Bukkit#addRecipe` | 2 |
| `org.bukkit.block.Block#getState` | 2 |
| `org.bukkit.entity.ItemFrame#getItem` | 2 |
| `org.bukkit.event.entity.EntityPickupItemEvent#getEntity` | 2 |
| `org.bukkit.event.entity.EntityPickupItemEvent#getItem` | 2 |
| `org.bukkit.event.inventory.PrepareItemCraftEvent#getInventory` | 2 |
| `org.bukkit.inventory.AnvilInventory#getItem` | 2 |
| `org.bukkit.inventory.ItemStack#setData` | 2 |
| `com.destroystokyo.paper.event.player.PlayerLaunchProjectileEvent#getProjectile` | 1 |
| `io.papermc.paper.datacomponent.DataComponentTypes#CUSTOM_MODEL_DATA` | 1 |
| `io.papermc.paper.datacomponent.DataComponentTypes#GLIDER` | 1 |
| `io.papermc.paper.datacomponent.DataComponentTypes#ITEM_MODEL` | 1 |
| `io.papermc.paper.datacomponent.item.CustomModelData$Builder#addFloat` | 1 |
| `io.papermc.paper.datacomponent.item.CustomModelData$Builder#addString` | 1 |
| `io.papermc.paper.plugin.loader.PluginClasspathBuilder#addLibrary` | 1 |
| `io.papermc.paper.plugin.loader.library.impl.MavenLibraryResolver#<init>` | 1 |
| `io.papermc.paper.plugin.loader.library.impl.MavenLibraryResolver#addDependency` | 1 |
| `io.papermc.paper.plugin.loader.library.impl.MavenLibraryResolver#addRepository` | 1 |
| `org.bukkit.Bukkit#removeRecipe` | 1 |
| `org.bukkit.Chunk#getEntities` | 1 |
| `org.bukkit.entity.Arrow#getShooter` | 1 |
| `org.bukkit.entity.Item#getType` | 1 |
| `org.bukkit.entity.ItemFrame#setItem` | 1 |
| `org.bukkit.entity.Player#isBlocking` | 1 |
| ... 40 more | |

## 🟡 Vault 2.20.3

- file: `vaultunlocked--VaultUnlocked-2.20.3.jar`
- main: `net.milkbowl.vault.Vault`, api-version: 1.13
- softdepend: PlaceholderAPI
- classes: 63 (24 own under `net.milkbowl.vault`, 39 bundled), reflection calls: 5
- uses 16 uncovered API members and 8 uncovered API types (top: org.bukkit.permissions.PermissionAttachmentInfo#getAttachment, com.destroystokyo.paper.exception.ServerPluginException#getCause, org.bukkit.entity.Player#getEffectivePermissions, org.bukkit.permissions.PermissionAttachment#getPlugin, com.destroystokyo.paper.event.server.ServerExceptionEvent#getException)

| Uncovered member | Sites |
|---|---:|
| `org.bukkit.permissions.PermissionAttachmentInfo#getAttachment` | 6 |
| `com.destroystokyo.paper.exception.ServerPluginException#getCause` | 2 |
| `org.bukkit.entity.Player#getEffectivePermissions` | 2 |
| `org.bukkit.permissions.PermissionAttachment#getPlugin` | 2 |
| `com.destroystokyo.paper.event.server.ServerExceptionEvent#getException` | 1 |
| `com.destroystokyo.paper.exception.ServerPluginException#getResponsiblePlugin` | 1 |
| `io.papermc.paper.configuration.ServerConfiguration#isProxyOnlineMode` | 1 |
| `org.bukkit.Bukkit#getOnlineMode` | 1 |
| `org.bukkit.Server#getOfflinePlayers` | 1 |
| `org.bukkit.Server#getOnlineMode` | 1 |
| `org.bukkit.Server#getPluginsFolder` | 1 |
| `org.bukkit.Server#getServerConfig` | 1 |
| `org.bukkit.Server$Spigot#getPaperConfig` | 1 |
| `org.bukkit.Server$Spigot#getSpigotConfig` | 1 |
| `org.bukkit.event.server.PluginEnableEvent#getPlugin` | 1 |
| `org.bukkit.plugin.ServicesManager#unregisterAll` | 1 |

## 🔴 ViaBackwards 5.12.2

- file: `viabackwards--ViaBackwards-5.12.2.jar`
- main: `com.viaversion.viabackwards.BukkitPlugin`, api-version: 1.13
- depend: ViaVersion
- classes: 610 (610 own under `com.viaversion.viabackwards`, 0 bundled), reflection calls: 2
- server internals (Mojang libs, Netty, Paper internals): io.netty.buffer (5 refs, in own)
- uses 9 uncovered API members and 5 uncovered API types (top: org.bukkit.Bukkit#getAdvancement, org.bukkit.Material#getMaxDurability, org.bukkit.advancement.Advancement#getCriteria, org.bukkit.block.Block#getState, org.bukkit.block.Lectern#getInventory)

| Uncovered member | Sites |
|---|---:|
| `org.bukkit.Bukkit#getAdvancement` | 1 |
| `org.bukkit.Material#getMaxDurability` | 1 |
| `org.bukkit.advancement.Advancement#getCriteria` | 1 |
| `org.bukkit.block.Block#getState` | 1 |
| `org.bukkit.block.Lectern#getInventory` | 1 |
| `org.bukkit.entity.Player#openBook` | 1 |
| `org.bukkit.event.player.PlayerDropItemEvent#getPlayer` | 1 |
| `org.bukkit.plugin.PluginLoader#disablePlugin` | 1 |
| `org.bukkit.plugin.java.JavaPlugin#getPluginLoader` | 1 |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| server internals (Mojang libs, Netty, Paper internals) | `io.netty.buffer` | own | 5 | yes |

## 🔴 ViaRewind 4.2.0

- file: `viarewind--ViaRewind-4.2.0.jar`
- main: `com.viaversion.viarewind.BukkitPlugin`, api-version: 1.13
- depend: ViaVersion, ViaBackwards
- classes: 207 (207 own under `com.viaversion.viarewind`, 0 bundled), reflection calls: 1
- server internals (Mojang libs, Netty, Paper internals): io.netty.buffer, io.netty.channel, io.netty.handler.codec (38 refs, in own)

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| server internals (Mojang libs, Netty, Paper internals) | `io.netty.buffer` | own | 25 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `io.netty.channel` | own | 9 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `io.netty.handler.codec` | own | 4 | yes |

## 🔴 ViaVersion 5.12.2

- file: `viaversion--ViaVersion-5.12.2.jar`
- main: `com.viaversion.viaversion.ViaVersionPlugin`, api-version: 1.13
- classes: 3426 (1722 own under `com.viaversion.viaversion`, 1704 bundled), reflection calls: 176
- server internals (Mojang libs, Netty, Paper internals): io.netty.buffer, io.netty.channel, io.netty.handler.codec (404 refs, in own)
- reflection into internals: Craft* class name, getHandle(), io.netty.channel (30 refs, in own)
- uses 41 uncovered API members and 12 uncovered API types (top: org.bukkit.event.player.PlayerToggleSneakEvent#isSneaking, org.bukkit.block.Block#getState, org.bukkit.inventory.ItemStack#getTypeId, org.bukkit.inventory.PlayerInventory#setHeldItemSlot, org.bukkit.Bukkit#getItemFactory)

| Uncovered member | Sites |
|---|---:|
| `org.bukkit.event.player.PlayerToggleSneakEvent#isSneaking` | 5 |
| `org.bukkit.block.Block#getState` | 3 |
| `org.bukkit.inventory.ItemStack#getTypeId` | 3 |
| `org.bukkit.inventory.PlayerInventory#setHeldItemSlot` | 3 |
| `org.bukkit.Bukkit#getItemFactory` | 2 |
| `org.bukkit.Bukkit#getUnsafe` | 2 |
| `org.bukkit.Material#getId` | 2 |
| `org.bukkit.UnsafeValues#getProtocolVersion` | 2 |
| `io.papermc.paper.event.player.PlayerInventorySlotChangeEvent#getNewItemStack` | 1 |
| `io.papermc.paper.event.player.PlayerInventorySlotChangeEvent#getPlayer` | 1 |
| `io.papermc.paper.event.player.PlayerInventorySlotChangeEvent#getSlot` | 1 |
| `org.bukkit.Bukkit#isStopping` | 1 |
| `org.bukkit.Material#getEquipmentSlot` | 1 |
| `org.bukkit.Material#isBlock` | 1 |
| `org.bukkit.Material#isItem` | 1 |
| `org.bukkit.Material#isSolid` | 1 |
| `org.bukkit.World#getGameRuleValue` | 1 |
| `org.bukkit.block.BlockState#update` | 1 |
| `org.bukkit.block.data.BlockData#getPlacementMaterial` | 1 |
| `org.bukkit.entity.Player#getFireTicks` | 1 |
| `org.bukkit.entity.Player#hasPotionEffect` | 1 |
| `org.bukkit.entity.Player#isGlowing` | 1 |
| `org.bukkit.entity.Player#isSneaking` | 1 |
| `org.bukkit.entity.Player#isSprinting` | 1 |
| `org.bukkit.entity.Player#isSwimming` | 1 |
| `org.bukkit.event.block.BlockDamageEvent#getItemInHand` | 1 |
| `org.bukkit.event.block.BlockDamageEvent#getPlayer` | 1 |
| `org.bukkit.event.block.BlockPlaceEvent#getBlockAgainst` | 1 |
| `org.bukkit.event.entity.EntityToggleGlideEvent#getEntity` | 1 |
| `org.bukkit.event.entity.EntityToggleGlideEvent#isCancelled` | 1 |
| `org.bukkit.event.entity.EntityToggleGlideEvent#isGliding` | 1 |
| `org.bukkit.event.inventory.InventoryType#BREWING` | 1 |
| `org.bukkit.event.inventory.InventoryType#CRAFTING` | 1 |
| `org.bukkit.event.player.PlayerItemBreakEvent#getPlayer` | 1 |
| `org.bukkit.event.player.PlayerItemHeldEvent#getNewSlot` | 1 |
| `org.bukkit.event.player.PlayerItemHeldEvent#getPlayer` | 1 |
| `org.bukkit.event.player.PlayerToggleSneakEvent#getPlayer` | 1 |
| `org.bukkit.inventory.ItemFactory#getSpawnEgg` | 1 |
| `org.bukkit.inventory.meta.BlockStateMeta#setBlockState` | 1 |
| `org.bukkit.plugin.messaging.Messenger#dispatchIncomingMessage` | 1 |
| ... 1 more | |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| server internals (Mojang libs, Netty, Paper internals) | `io.netty.buffer` | own | 290 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `io.netty.channel` | own | 89 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `io.netty.handler.codec` | own | 19 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `io.netty.util` | own | 2 | yes |
| server internals (Mojang libs, Netty, Paper internals) | `io.netty.util.concurrent` | own | 4 | yes |
| reflection into internals | `Craft* class name` | own | 9 | yes |
| reflection into internals | `getHandle()` | own | 6 | yes |
| reflection into internals | `io.netty.channel` | own | 1 | yes |
| reflection into internals | `net.minecraft.SharedConstants` | own | 1 | yes |
| reflection into internals | `net.minecraft.WorldVersion` | own | 1 | yes |
| reflection into internals | `net.minecraft.network` | own | 4 | yes |
| reflection into internals | `net.minecraft.server` | own | 6 | yes |
| reflection into internals | `org.bukkit.craftbukkit` | own | 2 | yes |

## 🔴 WorldEdit 7.4.6+7648-00664b0

- file: `worldedit--worldedit-bukkit-7.4.6.jar`
- main: `com.sk89q.worldedit.bukkit.WorldEditPlugin`, api-version: 1.21.4
- softdepend: Vault
- classes: 3856 (3518 own under `com.sk89q.worldedit`, 338 bundled), reflection calls: 431
- NMS (net.minecraft): net.minecraft, net.minecraft.core, net.minecraft.core.component (4111 refs, in own)
- CraftBukkit: org.bukkit.craftbukkit, org.bukkit.craftbukkit.block, org.bukkit.craftbukkit.entity (133 refs, in own)
- server internals (Mojang libs, Netty, Paper internals): com.mojang.authlib, com.mojang.datafixers, com.mojang.datafixers.schemas (307 refs, in own)
- reflection into internals: Craft* class name, getHandle(), net.minecraft.server (7 refs, in own)
- uses 90 uncovered API members and 52 uncovered API types (top: org.bukkit.World#getBiomeProvider, org.bukkit.World#getGenerator, org.bukkit.event.block.BlockPhysicsEvent#<init>, org.bukkit.event.block.BlockPhysicsEvent#isCancelled, org.bukkit.event.entity.CreatureSpawnEvent$SpawnReason#CUSTOM)

| Uncovered member | Sites |
|---|---:|
| `org.bukkit.World#getBiomeProvider` | 13 |
| `org.bukkit.World#getGenerator` | 13 |
| `org.bukkit.event.block.BlockPhysicsEvent#<init>` | 13 |
| `org.bukkit.event.block.BlockPhysicsEvent#isCancelled` | 13 |
| `org.bukkit.event.entity.CreatureSpawnEvent$SpawnReason#CUSTOM` | 13 |
| `org.spigotmc.WatchdogThread#tick` | 13 |
| `org.bukkit.Registry#get` | 5 |
| `org.bukkit.help.HelpTopic#fullText` | 5 |
| `org.bukkit.command.BlockCommandSender#getBlock` | 4 |
| `io.papermc.paper.world.PaperWorldLoader$LoadedWorldData#<init>` | 3 |
| `io.papermc.paper.world.saveddata.PaperWorldPDC#<init>` | 3 |
| `org.bukkit.Registry#ENTITY_TYPE` | 3 |
| `org.bukkit.Registry#MATERIAL` | 3 |
| `org.bukkit.World#getPersistentDataContainer` | 3 |
| `org.bukkit.block.Block#getState` | 3 |
| `org.bukkit.command.BlockCommandSender#getName` | 3 |
| `org.bukkit.help.HelpTopic#shortText` | 3 |
| `org.bukkit.Bukkit#getTag` | 2 |
| `org.bukkit.Bukkit#getTags` | 2 |
| `org.bukkit.Material#isBlock` | 2 |
| `org.bukkit.Registry#forEach` | 2 |
| `org.bukkit.Tag#getKey` | 2 |
| `org.bukkit.Tag#getValues` | 2 |
| `org.bukkit.TreeType#BROWN_MUSHROOM` | 2 |
| `org.bukkit.TreeType#REDWOOD` | 2 |
| `org.bukkit.World#getEntities` | 2 |
| `org.bukkit.World#isChunkGenerated` | 2 |
| `org.bukkit.block.Block#getState` | 2 |
| `org.bukkit.entity.Player#getBedSpawnLocation` | 2 |
| `org.bukkit.entity.Player#sendBlockChange` | 2 |
| `org.bukkit.event.server.PluginDisableEvent#getPlugin` | 2 |
| `org.bukkit.help.HelpTopic#name` | 2 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#getBuffer` | 1 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#getSender` | 1 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#isCommand` | 1 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#setCompletions` | 1 |
| `com.destroystokyo.paper.event.server.AsyncTabCompleteEvent#setHandled` | 1 |
| `io.papermc.paper.ServerBuildInfo#buildInfo` | 1 |
| `io.papermc.paper.ServerBuildInfo#isBrandCompatible` | 1 |
| `net.milkbowl.vault.permission.Permission#playerInGroup` | 1 |
| ... 50 more | |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| NMS (net.minecraft) | `net.minecraft` | own | 36 | yes |
| NMS (net.minecraft) | `net.minecraft.core` | own | 445 | yes |
| NMS (net.minecraft) | `net.minecraft.core.component` | own | 13 | yes |
| NMS (net.minecraft) | `net.minecraft.core.registries` | own | 26 | yes |
| NMS (net.minecraft) | `net.minecraft.nbt` | own | 1415 | yes |
| NMS (net.minecraft) | `net.minecraft.network.chat` | own | 140 | yes |
| NMS (net.minecraft) | `net.minecraft.network.protocol` | own | 39 | yes |
| NMS (net.minecraft) | `net.minecraft.resources` | own | 178 | yes |
| NMS (net.minecraft) | `net.minecraft.server` | own | 65 | yes |
| NMS (net.minecraft) | `net.minecraft.server.dedicated` | own | 26 | yes |
| NMS (net.minecraft) | `net.minecraft.server.level` | own | 194 | yes |
| NMS (net.minecraft) | `net.minecraft.server.network` | own | 13 | yes |
| NMS (net.minecraft) | `net.minecraft.stats` | own | 13 | yes |
| NMS (net.minecraft) | `net.minecraft.tags` | own | 13 | yes |
| NMS (net.minecraft) | `net.minecraft.util` | own | 117 | yes |
| NMS (net.minecraft) | `net.minecraft.util.datafix` | own | 39 | yes |
| NMS (net.minecraft) | `net.minecraft.util.thread` | own | 13 | yes |
| NMS (net.minecraft) | `net.minecraft.world` | own | 98 | yes |
| NMS (net.minecraft) | `net.minecraft.world.damagesource` | own | 13 | yes |
| NMS (net.minecraft) | `net.minecraft.world.entity` | own | 109 | yes |
| NMS (net.minecraft) | `net.minecraft.world.item` | own | 52 | yes |
| NMS (net.minecraft) | `net.minecraft.world.level` | own | 989 | yes |
| NMS (net.minecraft) | `net.minecraft.world.phys` | own | 65 | yes |
| CraftBukkit | `org.bukkit.craftbukkit` | own | 40 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.block` | own | 16 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.entity` | own | 16 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.inventory` | own | 8 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.persistence` | own | 3 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_21_R3` | own | 10 | yes |
| CraftBukkit | `org.bukkit.craftbukkit.v1_21_R4` | own | 10 | yes |

## 🔴 WorldGuard 7.0.21+2424-da30a99

- file: `worldguard--worldguard-bukkit-7.0.21.jar`
- main: `com.sk89q.worldguard.bukkit.WorldGuardPlugin`, api-version: 26.3
- depend: WorldEdit
- classes: 673 (633 own under `com.sk89q.worldguard`, 40 bundled), reflection calls: 30
- reflection into internals: Craft* class name, getHandle() (3 refs, in own)
- uses 568 uncovered API members and 180 uncovered API types (top: org.bukkit.Tag#isTagged, org.bukkit.event.entity.EntityExplodeEvent#blockList, org.bukkit.event.block.BlockSpreadEvent#setCancelled, org.bukkit.block.BlockState#getType, org.bukkit.event.block.BlockFadeEvent#getBlock)

| Uncovered member | Sites |
|---|---:|
| `org.bukkit.Tag#isTagged` | 38 |
| `org.bukkit.event.entity.EntityExplodeEvent#blockList` | 16 |
| `org.bukkit.event.block.BlockSpreadEvent#setCancelled` | 13 |
| `org.bukkit.block.BlockState#getType` | 11 |
| `org.bukkit.event.block.BlockFadeEvent#getBlock` | 11 |
| `org.bukkit.event.block.BlockFormEvent#setCancelled` | 11 |
| `org.bukkit.event.block.BlockIgniteEvent#setCancelled` | 11 |
| `org.bukkit.event.block.SignChangeEvent#getBlock` | 11 |
| `org.bukkit.event.entity.EntityExplodeEvent#setCancelled` | 11 |
| `org.bukkit.event.block.BlockFadeEvent#setCancelled` | 9 |
| `org.bukkit.event.block.BlockSpreadEvent#getBlock` | 8 |
| `org.bukkit.event.block.BlockBurnEvent#setCancelled` | 7 |
| `org.bukkit.event.block.BlockFormEvent#getBlock` | 7 |
| `org.bukkit.event.entity.CreatureSpawnEvent$SpawnReason#ordinal` | 7 |
| `org.bukkit.event.entity.EntityDamageByBlockEvent#setCancelled` | 7 |
| `org.bukkit.event.entity.ExplosionPrimeEvent#getEntityType` | 7 |
| `org.bukkit.event.entity.ExplosionPrimeEvent#setCancelled` | 7 |
| `org.bukkit.event.player.PlayerFishEvent#getPlayer` | 7 |
| `org.bukkit.block.Block#breakNaturally` | 6 |
| `org.bukkit.entity.Projectile#getShooter` | 6 |
| `org.bukkit.event.block.BlockBurnEvent#getBlock` | 6 |
| `org.bukkit.event.block.BlockFromToEvent#setCancelled` | 6 |
| `org.bukkit.event.entity.CreatureSpawnEvent#getEntity` | 6 |
| `org.bukkit.event.entity.EntityChangeBlockEvent#setCancelled` | 6 |
| `org.bukkit.event.hanging.HangingBreakEvent#getEntity` | 6 |
| `org.bukkit.event.hanging.HangingBreakEvent#setCancelled` | 6 |
| `org.bukkit.event.inventory.InventoryAction#ordinal` | 6 |
| `org.bukkit.event.player.PlayerFishEvent#getHook` | 6 |
| `org.bukkit.Material#isBlock` | 5 |
| `org.bukkit.Tag#getValues` | 5 |
| `org.bukkit.block.Block#getState` | 5 |
| `org.bukkit.block.BlockState#getBlock` | 5 |
| `org.bukkit.event.block.BlockIgniteEvent#getPlayer` | 5 |
| `org.bukkit.event.block.BlockPhysicsEvent#setCancelled` | 5 |
| `org.bukkit.event.block.BlockPistonExtendEvent#getBlock` | 5 |
| `org.bukkit.event.block.SignChangeEvent#setCancelled` | 5 |
| `org.bukkit.event.entity.EntityCombustEvent#getEntity` | 5 |
| `org.bukkit.event.player.PlayerBucketEmptyEvent#getBucket` | 5 |
| `org.bukkit.potion.PotionEffect#getType` | 5 |
| `io.papermc.paper.event.entity.EntityBreakEvent#getEntity` | 4 |
| ... 528 more | |

| Red signal | Target | Origin | Refs | Decisive |
|---|---|---|---:|:---:|
| reflection into internals | `Craft* class name` | own | 2 | yes |
| reflection into internals | `getHandle()` | own | 1 | yes |

