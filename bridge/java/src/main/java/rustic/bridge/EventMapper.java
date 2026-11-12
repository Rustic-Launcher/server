// SPDX-License-Identifier: GPL-3.0-only
package rustic.bridge;

import io.papermc.paper.chat.ChatRenderer;
import io.papermc.paper.event.player.AsyncChatEvent;
import java.net.InetAddress;
import java.net.UnknownHostException;
import java.util.HashSet;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;
import java.util.Set;
import java.util.UUID;
import net.kyori.adventure.audience.Audience;
import net.kyori.adventure.text.Component;
import org.bukkit.Location;
import org.bukkit.entity.Player;
import org.bukkit.event.Cancellable;
import org.bukkit.event.Event;
import org.bukkit.event.EventPriority;
import org.bukkit.event.block.BlockBreakEvent;
import org.bukkit.event.player.AsyncPlayerChatEvent;
import org.bukkit.event.player.AsyncPlayerPreLoginEvent;
import org.bukkit.event.player.PlayerChangedWorldEvent;
import org.bukkit.event.player.PlayerCommandPreprocessEvent;
import org.bukkit.event.player.PlayerGameModeChangeEvent;
import org.bukkit.event.player.PlayerJoinEvent;
import org.bukkit.event.player.PlayerMoveEvent;
import org.bukkit.event.player.PlayerQuitEvent;
import org.bukkit.event.player.PlayerRespawnEvent;
import org.bukkit.event.player.PlayerTeleportEvent;
import org.bukkit.event.server.ServerCommandEvent;

/**
 * Rustic events (records of {@code rustic:api/event-types}, as JSON) to Bukkit events and back.
 * For each (plugin, priority) call of the core, the Bukkit events that map to the Rustic event are
 * built, that plugin's listeners at that priority run, and the mutable fields and the
 * cancellation are written back into the record.
 */
final class EventMapper {
    private EventMapper() {}

    /** The Rustic events a Bukkit event class needs (empty: not bridged). */
    static List<String> rusticEvents(Class<?> ec) {
        if (ec == PlayerJoinEvent.class) return List.of("player-join");
        if (ec == PlayerQuitEvent.class) return List.of("player-quit");
        if (ec == AsyncPlayerChatEvent.class || ec == AsyncChatEvent.class) return List.of("player-chat");
        if (ec == PlayerMoveEvent.class) return List.of("player-move");
        if (ec == PlayerTeleportEvent.class) return List.of("player-teleport");
        if (ec == PlayerCommandPreprocessEvent.class) return List.of("player-command");
        if (ec == ServerCommandEvent.class) return List.of("server-command");
        if (ec == BlockBreakEvent.class) return List.of("block-break");
        if (ec == PlayerGameModeChangeEvent.class) return List.of("player-game-mode-change");
        if (ec == PlayerRespawnEvent.class) return List.of("player-respawn");
        if (ec == AsyncPlayerPreLoginEvent.class) return List.of("player-pre-login");
        if (ec == PlayerChangedWorldEvent.class) return List.of("player-change-world");
        return List.of();
    }

    /** The Bukkit event classes the bridge fires (for docs and the coverage report). */
    static final List<Class<? extends Event>> BRIDGED = List.of(
            PlayerJoinEvent.class, PlayerQuitEvent.class, AsyncPlayerChatEvent.class, AsyncChatEvent.class,
            PlayerMoveEvent.class, PlayerTeleportEvent.class, PlayerCommandPreprocessEvent.class,
            ServerCommandEvent.class, BlockBreakEvent.class, PlayerGameModeChangeEvent.class,
            PlayerRespawnEvent.class, AsyncPlayerPreLoginEvent.class, PlayerChangedWorldEvent.class);

    /** A plugin's own event class (fired with callEvent): no Rustic event needed. */
    static boolean isPluginEvent(Class<?> ec) {
        return ec.getClassLoader() instanceof PluginClassLoader
                || ec == org.bukkit.event.server.PluginEnableEvent.class
                || ec == org.bukkit.event.server.PluginDisableEvent.class;
    }

    static Object dispatch(Bridge b, Map<String, Object> req) {
        Map<String, Object> ev = Json.map(req.get("event"));
        String kind = ev.keySet().iterator().next();
        Map<String, Object> rec = Json.map(ev.get(kind));
        String plugin = Json.str(req.get("plugin"));
        if (plugin == null || plugin.isEmpty()) {
            mirror(b, kind, rec);
            return Json.obj("event", ev);
        }
        LoadedPlugin lp = b.plugins.get(plugin);
        if (lp == null || lp.plugin == null || !lp.plugin.isEnabled()) return Json.obj("event", ev);
        EventPriority prio = EventPriority.valueOf(Json.str(req.get("priority")).toUpperCase(Locale.ROOT));
        String saved = b.pluginManager.current;
        b.pluginManager.current = plugin;
        try {
            fire(b, lp, prio, kind, rec);
        } finally {
            b.pluginManager.current = saved;
        }
        return Json.obj("event", ev);
    }

    /** The bridge's own join/quit subscriptions keep the online-player mirror. */
    private static void mirror(Bridge b, String kind, Map<String, Object> rec) {
        if (kind.equals("player-join")) {
            PlayerProxy p = b.player(Json.uuid(rec.get("player")), Json.str(rec.get("name")));
            b.online.put(p.id, p);
        } else if (kind.equals("player-quit")) {
            UUID id = Json.uuid(rec.get("player"));
            b.scheduler.afterTick(() -> b.online.remove(id));
        }
    }

    private static Player player(Bridge b, Map<String, Object> rec) {
        PlayerProxy p = b.player(Json.uuid(rec.get("player")), Json.str(rec.get("name")));
        return p.proxy;
    }

    private static void fire(Bridge b, LoadedPlugin lp, EventPriority prio, String kind, Map<String, Object> rec) {
        PluginManagerProxy pm = b.pluginManager;
        switch (kind) {
            case "player-join": {
                PlayerProxy pp = b.player(Json.uuid(rec.get("player")), Json.str(rec.get("name")));
                b.online.put(pp.id, pp);
                PlayerJoinEvent e = new PlayerJoinEvent(pp.proxy, TextUtil.fromJson(Json.str(rec.get("join-message"))));
                pm.callEvent(e, lp.plugin, prio);
                rec.put("join-message", TextUtil.toJson(e.joinMessage()));
                break;
            }
            case "player-quit": {
                PlayerQuitEvent e = new PlayerQuitEvent(player(b, rec),
                        TextUtil.fromJson(Json.str(rec.get("quit-message"))), PlayerQuitEvent.QuitReason.DISCONNECTED);
                pm.callEvent(e, lp.plugin, prio);
                rec.put("quit-message", TextUtil.toJson(e.quitMessage()));
                break;
            }
            case "player-chat":
                chat(b, lp, prio, rec);
                break;
            case "player-move": {
                Location from = Convert.location(b, rec.get("from"));
                Location to = Convert.location(b, rec.get("to"));
                PlayerMoveEvent e = new PlayerMoveEvent(player(b, rec), from, to);
                e.setCancelled(Json.bool(rec.get("cancelled")));
                pm.callEvent(e, lp.plugin, prio);
                rec.put("cancelled", e.isCancelled());
                break;
            }
            case "player-teleport": {
                PlayerTeleportEvent e = new PlayerTeleportEvent(player(b, rec),
                        Convert.location(b, rec.get("from")), Convert.location(b, rec.get("to")));
                pm.callEvent(e, lp.plugin, prio);
                break; // not cancellable in rustic:api 0.1
            }
            case "player-command": {
                String cmd = Json.str(rec.get("command"));
                PlayerCommandPreprocessEvent e = new PlayerCommandPreprocessEvent(player(b, rec),
                        cmd.startsWith("/") ? cmd : "/" + cmd, new HashSet<>(b.onlinePlayers()));
                e.setCancelled(Json.bool(rec.get("cancelled")));
                pm.callEvent(e, lp.plugin, prio);
                rec.put("cancelled", e.isCancelled());
                break;
            }
            case "server-command": {
                ServerCommandEvent e = new ServerCommandEvent(b.console.proxy, Json.str(rec.get("command")));
                e.setCancelled(Json.bool(rec.get("cancelled")));
                pm.callEvent(e, lp.plugin, prio);
                rec.put("cancelled", e.isCancelled());
                break;
            }
            case "block-break": {
                Map<String, Object> pos = Json.map(rec.get("pos"));
                Map<String, Object> block = Json.map(rec.get("block"));
                BlockBreakEvent e = new BlockBreakEvent(Convert.block(b, Json.str(rec.get("world")),
                        (int) Json.lng(pos.get("x")), (int) Json.lng(pos.get("y")), (int) Json.lng(pos.get("z")),
                        Json.str(block.get("id"))), player(b, rec));
                e.setCancelled(Json.bool(rec.get("cancelled")));
                e.setDropItems(rec.get("drop-items") == null || Json.bool(rec.get("drop-items")));
                pm.callEvent(e, lp.plugin, prio);
                rec.put("cancelled", e.isCancelled());
                rec.put("drop-items", e.isDropItems());
                break;
            }
            case "player-game-mode-change": {
                PlayerGameModeChangeEvent e = new PlayerGameModeChangeEvent(player(b, rec),
                        Convert.gameMode(Json.str(rec.get("new-mode"))));
                e.setCancelled(Json.bool(rec.get("cancelled")));
                pm.callEvent(e, lp.plugin, prio);
                rec.put("cancelled", e.isCancelled());
                break;
            }
            case "player-respawn": {
                PlayerRespawnEvent e = new PlayerRespawnEvent(player(b, rec),
                        Convert.location(b, rec.get("location")), false, false, false,
                        PlayerRespawnEvent.RespawnReason.DEATH);
                pm.callEvent(e, lp.plugin, prio);
                rec.put("location", Convert.location(e.getRespawnLocation()));
                break;
            }
            case "player-pre-login": {
                InetAddress addr;
                try {
                    String a = Json.str(rec.get("address"));
                    int colon = a.lastIndexOf(':');
                    addr = InetAddress.getByName(colon > 0 && a.indexOf(':') == colon ? a.substring(0, colon) : a);
                } catch (UnknownHostException | RuntimeException ex) {
                    addr = InetAddress.getLoopbackAddress();
                }
                AsyncPlayerPreLoginEvent e = new AsyncPlayerPreLoginEvent(Json.str(rec.get("name")), addr,
                        Json.uuid(rec.get("player")));
                if (Json.bool(rec.get("cancelled")))
                    e.disallow(AsyncPlayerPreLoginEvent.Result.KICK_OTHER,
                            TextUtil.fromJson(Objects.requireNonNullElse(Json.str(rec.get("kick-message")), "\"\"")));
                pm.callEvent(e, lp.plugin, prio);
                boolean refused = e.getLoginResult() != AsyncPlayerPreLoginEvent.Result.ALLOWED;
                rec.put("cancelled", refused);
                if (refused) rec.put("kick-message", TextUtil.toJson(e.kickMessage()));
                break;
            }
            case "player-change-world": {
                PlayerChangedWorldEvent e = new PlayerChangedWorldEvent(player(b, rec),
                        Convert.world(b, Json.str(rec.get("from-world"))));
                pm.callEvent(e, lp.plugin, prio);
                break;
            }
            default:
                break;
        }
    }

    private static final String DEFAULT_FORMAT = "<%1$s> %2$s";

    private static void chat(Bridge b, LoadedPlugin lp, EventPriority prio, Map<String, Object> rec) {
        Player p = player(b, rec);
        String message = Json.str(rec.get("message"));
        boolean cancelled = Json.bool(rec.get("cancelled"));
        String display = null;
        Set<Player> recipients = new LinkedHashSet<>(b.onlinePlayers());
        AsyncPlayerChatEvent legacy = new AsyncPlayerChatEvent(true, p, message, recipients);
        legacy.setCancelled(cancelled);
        b.pluginManager.callEvent(legacy, lp.plugin, prio);
        cancelled = legacy.isCancelled();
        if (!legacy.getFormat().equals(DEFAULT_FORMAT) || !legacy.getMessage().equals(message)) {
            display = TextUtil.legacyToJson(String.format(legacy.getFormat(), p.getDisplayName(), legacy.getMessage()));
        }
        if (PluginManagerProxy.handlerList(AsyncChatEvent.class).getRegisteredListeners().length > 0) {
            Component msg = Component.text(legacy.getMessage());
            Set<Audience> viewers = new LinkedHashSet<>(recipients);
            AsyncChatEvent modern = new AsyncChatEvent(true, p, viewers, ChatRenderer.defaultRenderer(), msg, msg, null);
            modern.setCancelled(cancelled);
            b.pluginManager.callEvent(modern, lp.plugin, prio);
            cancelled = modern.isCancelled();
            if (modern.message() != msg || modern.renderer() != ChatRenderer.defaultRenderer()) {
                Component out = modern.renderer().render(p, p.displayName(), modern.message(), p);
                display = TextUtil.toJson(out);
            }
        }
        rec.put("cancelled", cancelled);
        if (display != null) rec.put("display", display);
    }

    static boolean isCancellable(Event e) {
        return e instanceof Cancellable;
    }
}
