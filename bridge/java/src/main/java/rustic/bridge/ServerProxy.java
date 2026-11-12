// SPDX-License-Identifier: GPL-3.0-only
package rustic.bridge;

import java.io.File;
import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.Collection;
import java.util.HashSet;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;
import java.util.UUID;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.CopyOnWriteArrayList;
import java.util.logging.Logger;
import net.kyori.adventure.audience.Audience;
import net.kyori.adventure.text.Component;
import org.bukkit.GameMode;
import org.bukkit.OfflinePlayer;
import org.bukkit.Server;
import org.bukkit.World;
import org.bukkit.command.CommandSender;
import org.bukkit.command.PluginCommand;
import org.bukkit.entity.Player;
import org.bukkit.plugin.Plugin;
import org.bukkit.plugin.RegisteredServiceProvider;
import org.bukkit.plugin.ServicePriority;
import org.bukkit.plugin.ServicesManager;

/** {@link Server}: the facade plugins reach through {@code Bukkit.getServer()}. */
final class ServerProxy extends ApiProxy {
    private final Bridge bridge;
    final ServicesHandler servicesHandler = new ServicesHandler();
    private final ServicesManager services;
    private final Logger logger = Logger.getLogger("Rustic Bridge");

    ServerProxy(Bridge bridge) {
        super(Server.class);
        this.bridge = bridge;
        this.services = servicesHandler.create(ServicesManager.class);
    }

    private Object info(String key, Object dflt) {
        Object v = bridge.serverInfo.get(key);
        return v == null ? dflt : v;
    }

    private List<Object> worlds() {
        return Json.list(bridge.host.call("server", Json.obj("get", "worlds")));
    }

    @Override
    protected Object handle(Object proxy, Method m, String n, Object[] a) throws Throwable {
        switch (n) {
            case "getName":
                return "Rustic";
            case "getVersion":
                return "Rustic Bridge " + Bridge.VERSION + " (" + info("version", "Rustic") + ")";
            case "getBukkitVersion":
                return "26.3-R0.1-SNAPSHOT";
            case "getMinecraftVersion":
                return "26.3";
            case "getOnlineMode":
                return Json.bool(info("online_mode", Boolean.TRUE));
            case "getMaxPlayers":
                return (int) Json.lng(info("max_players", 20L));
            case "getMotd":
                return TextUtil.jsonToLegacy(Json.str(info("motd", "\"A Rustic server\"")));
            case "motd":
                if (a.length == 0) return TextUtil.fromJson(Json.str(info("motd", "\"A Rustic server\"")));
                return UNHANDLED;
            case "getPort":
                return (int) Json.lng(info("port", 25565L));
            case "getIp":
                return "";
            case "getLogger":
                return logger;
            case "getPluginManager":
                return bridge.pluginManager.proxy;
            case "getScheduler":
                return bridge.scheduler.proxy;
            case "getServicesManager":
                return services;
            case "getGlobalRegionScheduler":
                return bridge.scheduler.global;
            case "getRegionScheduler":
                return bridge.scheduler.region;
            case "getAsyncScheduler":
                return bridge.scheduler.asyncScheduler;
            case "getPluginsFolder":
                return new File(String.valueOf(info("plugins_dir", "plugins")));
            case "isOwnedByCurrentRegion":
            case "isGlobalTickThread":
                return bridge.host.isMainThread();
            case "getConsoleSender":
                return bridge.console.proxy;
            case "isPrimaryThread":
                return bridge.host.isMainThread();
            case "isStopping":
                return false;
            case "getCurrentTick":
                return (int) bridge.tick;
            case "getTPS": {
                Map<String, Object> st = Json.map(bridge.host.call("server", Json.obj("get", "tick_stats")));
                double tps = Json.dbl(st.get("tps"));
                return new double[] {tps, tps, tps};
            }
            case "getAverageTickTime": {
                Map<String, Object> st = Json.map(bridge.host.call("server", Json.obj("get", "tick_stats")));
                return Json.dbl(st.get("mspt"));
            }
            case "getOnlinePlayers":
                return java.util.Collections.unmodifiableCollection(bridge.onlinePlayers());
            case "getPlayer":
                if (a[0] instanceof UUID u) {
                    PlayerProxy p = bridge.online.get(u);
                    return p == null ? null : p.proxy;
                }
                if (a[0] instanceof String s) {
                    Player best = null;
                    String low = s.toLowerCase(Locale.ROOT);
                    for (PlayerProxy p : bridge.online.values()) {
                        if (p.name.equalsIgnoreCase(s)) return p.proxy;
                        if (best == null && p.name.toLowerCase(Locale.ROOT).startsWith(low)) best = p.proxy;
                    }
                    return best;
                }
                return UNHANDLED;
            case "getPlayerExact":
                for (PlayerProxy p : bridge.online.values()) if (p.name.equalsIgnoreCase((String) a[0])) return p.proxy;
                return null;
            case "matchPlayer": {
                List<Player> out = new ArrayList<>();
                String low = ((String) a[0]).toLowerCase(Locale.ROOT);
                for (PlayerProxy p : bridge.online.values())
                    if (p.name.toLowerCase(Locale.ROOT).contains(low)) out.add(p.proxy);
                return out;
            }
            case "getOfflinePlayer":
                return offline(a[0]);
            case "getOperators": {
                Set<OfflinePlayer> out = new HashSet<>();
                for (Object u : Json.list(bridge.host.call("server", Json.obj("get", "operators"))))
                    out.add(bridge.player(Json.uuid(u), null).proxy);
                return out;
            }
            case "broadcastMessage":
                bridge.host.post("broadcast", Json.obj("text", TextUtil.legacyToJson((String) a[0])));
                return bridge.online.size();
            case "broadcast":
                if (a.length >= 1 && a[0] instanceof Component c) {
                    bridge.host.post("broadcast", Json.obj("text", TextUtil.toJson(c),
                            "permission", a.length > 1 ? a[1] : null));
                    return bridge.online.size();
                }
                if (a.length == 2 && a[0] instanceof String s) {
                    bridge.host.post("broadcast", Json.obj("text", TextUtil.legacyToJson(s), "permission", a[1]));
                    return bridge.online.size();
                }
                return UNHANDLED;
            case "sendMessage":
                if (a.length == 1 && a[0] instanceof Component c) {
                    bridge.host.post("broadcast", Json.obj("text", TextUtil.toJson(c)));
                    return null;
                }
                return UNHANDLED;
            case "audiences": {
                List<Audience> out = new ArrayList<>(bridge.onlinePlayers());
                out.add(bridge.console.proxy);
                return out;
            }
            case "dispatchCommand":
                return Json.bool(bridge.host.call("dispatch_command",
                        Json.obj("sender", Senders.of((CommandSender) a[0]), "line", a[1])));
            case "getPluginCommand": {
                String low = ((String) a[0]).toLowerCase(Locale.ROOT);
                int colon = low.indexOf(':');
                for (LoadedPlugin lp : bridge.plugins.values()) {
                    if (colon > 0 && !lp.name().equalsIgnoreCase(low.substring(0, colon))) continue;
                    PluginCommand c = lp.commands.get(colon > 0 ? low.substring(colon + 1) : low);
                    if (c != null) return c;
                }
                return null;
            }
            case "getWorlds": {
                List<World> out = new ArrayList<>();
                for (Object id : worlds()) out.add(Convert.world(bridge, Json.str(id)));
                return out;
            }
            case "getWorld":
                if (a[0] instanceof String s) {
                    String id = Convert.worldId(s);
                    for (Object w : worlds()) if (id.equals(w)) return Convert.world(bridge, id);
                    return null;
                }
                if (a[0] instanceof UUID u) {
                    for (Object w : worlds()) {
                        World world = Convert.world(bridge, Json.str(w));
                        if (world.getUID().equals(u)) return world;
                    }
                    return null;
                }
                if (a[0] instanceof net.kyori.adventure.key.Key k) return Convert.world(bridge, k.asString());
                return UNHANDLED;
            case "getWorldContainer":
                return new File(".");
            case "getUpdateFolder":
                return "update";
            case "getDefaultGameMode":
                return GameMode.SURVIVAL;
            case "getAllowFlight":
            case "getAllowEnd":
            case "getAllowNether":
                return true;
            case "hasWhitelist":
            case "isHardcore":
                return false;
            case "getViewDistance":
            case "getSimulationDistance":
                return 10;
            case "getServerName":
                return "Rustic";
            case "reload":
            case "reloadData":
            case "savePlayers":
                return null;
            case "shutdown":
                BridgeLog.warn("Rustic Bridge", "a plugin asked for Server.shutdown(); ignored (use the console)");
                return null;
            case "toString":
                return "RusticServer";
            default:
                return UNHANDLED;
        }
    }

    private static UUID uuidOfName(String name) {
        return UUID.nameUUIDFromBytes(("OfflinePlayer:" + name).getBytes(java.nio.charset.StandardCharsets.UTF_8));
    }

    private OfflinePlayer offline(Object key) {
        if (key instanceof UUID u) return bridge.player(u, null).proxy;
        String name = (String) key;
        for (PlayerProxy p : bridge.online.values()) if (p.name.equalsIgnoreCase(name)) return p.proxy;
        Object u = bridge.host.call("offline_player", Json.obj("name", name));
        UUID id = u == null ? uuidOfName(name) : Json.uuid(u);
        return bridge.player(id, name).proxy;
    }

    @Override
    public String toString() {
        return "RusticServer";
    }

    /** {@link ServicesManager}: an in-JVM registry (Vault-style services between Java plugins). */
    static final class ServicesHandler extends ApiProxy {
        record Reg(Class<?> service, Object provider, Plugin plugin, ServicePriority priority) {}

        private final Map<Class<?>, List<Reg>> regs = new ConcurrentHashMap<>();

        ServicesHandler() {
            super(ServicesManager.class);
        }

        void unregisterAll(Plugin p) {
            if (p == null) return;
            for (List<Reg> l : regs.values()) l.removeIf(r -> r.plugin() == p);
        }

        @SuppressWarnings({"unchecked", "rawtypes"})
        private RegisteredServiceProvider rsp(Reg r) {
            return new RegisteredServiceProvider(r.service(), r.provider(), r.priority(), r.plugin());
        }

        private Reg best(Class<?> c) {
            Reg best = null;
            for (Reg r : regs.getOrDefault(c, List.of()))
                if (best == null || r.priority().ordinal() > best.priority().ordinal()) best = r;
            return best;
        }

        @Override
        protected Object handle(Object proxy, Method m, String n, Object[] a) {
            switch (n) {
                case "register":
                    regs.computeIfAbsent((Class<?>) a[0], k -> new CopyOnWriteArrayList<>())
                            .add(new Reg((Class<?>) a[0], a[1], (Plugin) a[2], (ServicePriority) a[3]));
                    return null;
                case "unregisterAll":
                    unregisterAll((Plugin) a[0]);
                    return null;
                case "unregister":
                    for (List<Reg> l : regs.values())
                        l.removeIf(r -> a.length == 1 ? r.provider() == a[0]
                                : r.service() == a[0] && r.provider() == a[1]);
                    return null;
                case "load": {
                    Reg r = best((Class<?>) a[0]);
                    return r == null ? null : r.provider();
                }
                case "getRegistration": {
                    Reg r = best((Class<?>) a[0]);
                    return r == null ? null : rsp(r);
                }
                case "getRegistrations": {
                    List<RegisteredServiceProvider<?>> out = new ArrayList<>();
                    if (a[0] instanceof Class<?> c) {
                        for (Reg r : regs.getOrDefault(c, List.of())) out.add(rsp(r));
                    } else {
                        for (List<Reg> l : regs.values()) for (Reg r : l) if (r.plugin() == a[0]) out.add(rsp(r));
                    }
                    return out;
                }
                case "isProvidedFor":
                    return best((Class<?>) a[0]) != null;
                case "getKnownServices": {
                    Collection<Class<?>> out = new ArrayList<>();
                    for (Map.Entry<Class<?>, List<Reg>> e : regs.entrySet())
                        if (!e.getValue().isEmpty()) out.add(e.getKey());
                    return out;
                }
                default:
                    return UNHANDLED;
            }
        }
    }
}
