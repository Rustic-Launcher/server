// SPDX-License-Identifier: GPL-3.0-only
package rustic.bridge;

import java.io.File;
import java.io.InputStream;
import java.lang.reflect.Constructor;
import java.util.ArrayList;
import java.util.Collection;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.UUID;
import java.util.concurrent.ConcurrentHashMap;
import java.util.jar.JarFile;
import java.util.zip.ZipEntry;
import org.bukkit.Bukkit;
import org.bukkit.Server;
import org.bukkit.command.PluginCommand;
import org.bukkit.entity.Player;
import org.bukkit.event.EventPriority;
import org.bukkit.event.HandlerList;
import org.bukkit.permissions.Permission;
import org.bukkit.plugin.Plugin;
import org.bukkit.plugin.PluginDescriptionFile;
import org.bukkit.plugin.java.JavaPlugin;

/**
 * The bridge's state and the host's requests: plugin loading, lifecycle, events, commands and
 * the tick pulse. One instance per JVM (Bukkit's {@code Server} is a singleton).
 */
public final class Bridge {
    public static final String VERSION = "0.1.0";
    /** The bridge of this JVM (for API services found with ServiceLoader). */
    static volatile Bridge current;

    final Host host;
    final ServerProxy serverHandler;
    final Server server;
    final Scheduler scheduler;
    final PluginManagerProxy pluginManager;
    /** Loaded plugins by name, in load order. */
    final Map<String, LoadedPlugin> plugins = new LinkedHashMap<>();
    /** Online players by UUID (maintained from the bridge's own join/quit subscriptions). */
    final Map<UUID, PlayerProxy> online = new ConcurrentHashMap<>();
    /** Every player object handed out (kept while the bridge runs, so identity is stable). */
    final Map<UUID, PlayerProxy> known = new ConcurrentHashMap<>();
    final ConsoleProxy console;
    volatile long tick;
    /** Server facts sent in `hello`. */
    volatile Map<String, Object> serverInfo = Map.of();

    Bridge(Host host) {
        this.host = host;
        this.scheduler = new Scheduler(this);
        this.pluginManager = new PluginManagerProxy(this);
        this.console = new ConsoleProxy(this);
        this.serverHandler = new ServerProxy(this);
        this.server = serverHandler.create(Server.class);
        current = this;
    }

    // ------------------------------------------------------------------ players

    PlayerProxy player(UUID id, String name) {
        PlayerProxy p = known.computeIfAbsent(id, k -> new PlayerProxy(this, k, name));
        if (name != null) p.name = name;
        return p;
    }

    Collection<Player> onlinePlayers() {
        List<Player> out = new ArrayList<>(online.size());
        for (PlayerProxy p : online.values()) out.add(p.proxy);
        return out;
    }

    // ------------------------------------------------------------------ requests

    Object handle(String op, Map<String, Object> req) throws Exception {
        return switch (op) {
            case "hello" -> hello(req);
            case "ping" -> Json.obj("tick", tick);
            case "scan" -> scan(new File(Json.str(req.get("dir"))));
            case "load" -> load(req);
            case "enable" -> enable(Json.str(req.get("plugin")));
            case "disable" -> disable(Json.str(req.get("plugin")));
            case "unload" -> unload(Json.str(req.get("plugin")));
            case "event" -> EventMapper.dispatch(this, req);
            case "command" -> CommandBridge.execute(this, req);
            case "complete" -> CommandBridge.complete(this, req);
            case "tick" -> {
                tick = Json.lng(req.get("tick"));
                scheduler.pulse(tick);
                yield null;
            }
            case "shutdown" -> {
                for (String n : new ArrayList<>(plugins.keySet())) disable(n);
                host.requestStop();
                yield null;
            }
            default -> throw new IllegalArgumentException("unknown request `" + op + "`");
        };
    }

    private Object hello(Map<String, Object> req) {
        serverInfo = Json.map(req.getOrDefault("server", Map.of()));
        for (Object o : Json.list(req.get("players"))) {
            Map<String, Object> p = Json.map(o);
            PlayerProxy pp = player(Json.uuid(p.get("uuid")), Json.str(p.get("name")));
            online.put(pp.id, pp);
        }
        if (Bukkit.getServer() == null) Bukkit.setServer(server);
        return Json.obj(
                "bridge", VERSION,
                "java", System.getProperty("java.version"),
                "pid", ProcessHandle.current().pid());
    }

    /** Read every `*.jar` of `dir`: its plugin.yml as a descriptor for the host. */
    private Object scan(File dir) {
        List<Object> out = new ArrayList<>();
        File[] files = dir.listFiles((d, n) -> n.toLowerCase(Locale.ROOT).endsWith(".jar"));
        if (files == null) return out;
        java.util.Arrays.sort(files);
        for (File f : files) {
            Map<String, Object> d = Json.obj("file", f.getAbsolutePath());
            try (JarFile jar = new JarFile(f)) {
                ZipEntry e = jar.getEntry("plugin.yml");
                if (e == null) {
                    d.put("error", jar.getEntry("paper-plugin.yml") != null
                            ? "Paper plugins (paper-plugin.yml only) are not supported yet"
                            : "no plugin.yml");
                } else {
                    try (InputStream in = jar.getInputStream(e)) {
                        describe(new PluginDescriptionFile(in), d);
                    }
                }
            } catch (Exception ex) {
                d.put("error", String.valueOf(ex.getMessage()));
            }
            out.add(d);
        }
        return out;
    }

    static void describe(PluginDescriptionFile pd, Map<String, Object> d) {
        d.put("name", pd.getName());
        d.put("version", pd.getVersion());
        d.put("main", pd.getMain());
        d.put("description", pd.getDescription());
        d.put("authors", pd.getAuthors());
        d.put("website", pd.getWebsite());
        d.put("depend", pd.getDepend());
        d.put("softdepend", pd.getSoftDepend());
        d.put("loadbefore", pd.getLoadBefore());
        d.put("api_version", pd.getAPIVersion());
        List<Object> cmds = new ArrayList<>();
        for (Map.Entry<String, Map<String, Object>> c : pd.getCommands().entrySet()) {
            Map<String, Object> v = c.getValue() == null ? Map.of() : c.getValue();
            List<String> aliases = new ArrayList<>();
            Object a = v.get("aliases");
            if (a instanceof List<?> l) for (Object x : l) aliases.add(String.valueOf(x));
            else if (a != null) aliases.add(String.valueOf(a));
            cmds.add(Json.obj(
                    "name", c.getKey(),
                    "description", v.get("description") == null ? "" : String.valueOf(v.get("description")),
                    "usage", v.get("usage") == null ? null : String.valueOf(v.get("usage")),
                    "aliases", aliases,
                    "permission", v.get("permission") == null ? null : String.valueOf(v.get("permission"))));
        }
        d.put("commands", cmds);
        List<Object> perms = new ArrayList<>();
        for (Permission p : pd.getPermissions()) {
            List<Object> children = new ArrayList<>();
            for (Map.Entry<String, Boolean> ch : p.getChildren().entrySet())
                children.add(List.of(ch.getKey(), ch.getValue()));
            perms.add(Json.obj(
                    "name", p.getName(),
                    "description", p.getDescription(),
                    "default", p.getDefault().name().toLowerCase(Locale.ROOT),
                    "children", children));
        }
        d.put("permissions", perms);
    }

    private Object load(Map<String, Object> req) throws Exception {
        File file = new File(Json.str(req.get("file")));
        File data = new File(Json.str(req.get("data_dir")));
        PluginDescriptionFile pd;
        try (JarFile jar = new JarFile(file); InputStream in = jar.getInputStream(jar.getEntry("plugin.yml"))) {
            pd = new PluginDescriptionFile(in);
        }
        if (plugins.containsKey(pd.getName())) throw new IllegalStateException(pd.getName() + " is already loaded");
        LoadedPlugin lp = new LoadedPlugin(this, pd, file, data);
        plugins.put(pd.getName(), lp);
        try {
            lp.loader = new PluginClassLoader(this, lp);
            createCommands(lp);
            Class<?> main = Class.forName(pd.getMain(), true, lp.loader);
            if (!JavaPlugin.class.isAssignableFrom(main))
                throw new IllegalStateException("main class " + pd.getMain() + " does not extend JavaPlugin");
            Thread.currentThread().setContextClassLoader(lp.loader);
            lp.plugin = (JavaPlugin) main.getDeclaredConstructor().newInstance();
            lp.plugin.getLogger().info("Loading " + pd.getFullName());
            lp.plugin.onLoad();
        } catch (Throwable t) {
            plugins.remove(pd.getName());
            if (lp.loader != null) lp.loader.close();
            throw t instanceof Exception e ? e : new RuntimeException(t);
        } finally {
            Thread.currentThread().setContextClassLoader(Bridge.class.getClassLoader());
        }
        return Json.obj("name", pd.getName());
    }

    private void createCommands(LoadedPlugin lp) throws ReflectiveOperationException {
        Constructor<PluginCommand> ctor = PluginCommand.class.getDeclaredConstructor(String.class, Plugin.class);
        ctor.setAccessible(true);
        for (Map.Entry<String, Map<String, Object>> c : lp.description.getCommands().entrySet()) {
            Map<String, Object> v = c.getValue() == null ? Map.of() : c.getValue();
            // The plugin instance does not exist yet: the command's owner is set at enable.
            lp.commandSpecs.put(c.getKey().toLowerCase(Locale.ROOT), v);
        }
        lp.commandCtor = ctor;
    }

    private Object enable(String name) throws Exception {
        LoadedPlugin lp = need(name);
        if (lp.plugin.isEnabled()) return Json.obj("enabled", true);
        lp.buildCommands();
        Thread.currentThread().setContextClassLoader(lp.loader);
        try {
            lp.plugin.getLogger().info("Enabling " + lp.description.getFullName());
            lp.plugin.setEnabled(true);
        } catch (Throwable t) {
            BridgeLog.error(name, "Error occurred while enabling " + lp.description.getFullName(), t);
            try {
                if (lp.plugin.isEnabled()) lp.plugin.setEnabled(false);
            } catch (Throwable t2) {
                BridgeLog.error(name, "error while disabling " + name, t2);
            }
            cleanup(lp);
            throw new IllegalStateException("onEnable threw " + t, t);
        } finally {
            Thread.currentThread().setContextClassLoader(Bridge.class.getClassLoader());
        }
        if (!lp.plugin.isEnabled()) {
            cleanup(lp);
            return Json.obj("enabled", false, "reason", "the plugin disabled itself in onEnable");
        }
        pluginManager.callEvent(new org.bukkit.event.server.PluginEnableEvent(lp.plugin));
        return Json.obj("enabled", true);
    }

    Object disable(String name) {
        LoadedPlugin lp = plugins.get(name);
        if (lp == null || lp.plugin == null) return null;
        if (lp.plugin.isEnabled()) {
            pluginManager.callEvent(new org.bukkit.event.server.PluginDisableEvent(lp.plugin));
            Thread.currentThread().setContextClassLoader(lp.loader);
            try {
                lp.plugin.getLogger().info("Disabling " + lp.description.getFullName());
                lp.plugin.setEnabled(false);
            } catch (Throwable t) {
                BridgeLog.error(name, "error while disabling " + name, t);
            } finally {
                Thread.currentThread().setContextClassLoader(Bridge.class.getClassLoader());
            }
        }
        cleanup(lp);
        return null;
    }

    void cleanup(LoadedPlugin lp) {
        scheduler.cancelTasks(lp.plugin);
        if (lp.plugin != null) HandlerList.unregisterAll(lp.plugin);
        lp.subscriptions.clear();
        serverHandler.servicesHandler.unregisterAll(lp.plugin);
    }

    private Object unload(String name) {
        disable(name);
        LoadedPlugin lp = plugins.remove(name);
        if (lp != null && lp.loader != null) lp.loader.close();
        return null;
    }

    LoadedPlugin need(String name) {
        LoadedPlugin lp = plugins.get(name);
        if (lp == null) throw new IllegalArgumentException("no plugin `" + name + "` is loaded");
        return lp;
    }

    LoadedPlugin of(Plugin p) {
        return p == null ? null : plugins.get(p.getName());
    }

    /** Subscribe `lp` to a Rustic event at a Bukkit priority, once. */
    void subscribe(LoadedPlugin lp, String rusticEvent, EventPriority prio) {
        String key = rusticEvent + "@" + prio;
        if (!lp.subscriptions.add(key)) return;
        host.call("subscribe", Json.obj(
                "plugin", lp.description.getName(),
                "event", rusticEvent,
                "priority", prio.name().toLowerCase(Locale.ROOT)));
    }
}
