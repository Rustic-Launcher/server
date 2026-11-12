// SPDX-License-Identifier: GPL-3.0-only
package rustic.bridge;

import java.lang.reflect.InvocationTargetException;
import java.lang.reflect.Method;
import java.lang.reflect.Modifier;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import org.bukkit.event.Event;
import org.bukkit.event.EventException;
import org.bukkit.event.EventHandler;
import org.bukkit.event.EventPriority;
import org.bukkit.event.HandlerList;
import org.bukkit.event.Listener;
import org.bukkit.permissions.Permission;
import org.bukkit.permissions.PermissionDefault;
import org.bukkit.plugin.EventExecutor;
import org.bukkit.plugin.IllegalPluginAccessException;
import org.bukkit.plugin.Plugin;
import org.bukkit.plugin.PluginManager;
import org.bukkit.plugin.RegisteredListener;

/**
 * {@link PluginManager}: listener registration ({@code @EventHandler} methods found by
 * reflection, into the event classes' own {@link HandlerList}s, with priority and
 * {@code ignoreCancelled}), {@code callEvent} for plugin events, plugin lookup and permissions.
 *
 * <p>Registering a listener for a built-in event also subscribes the plugin, in the Rustic core,
 * to the Rustic event it maps to at the same priority ({@link EventMapper}); the core then calls
 * the bridge once per (plugin, priority), so Bukkit and WebAssembly plugins interleave by priority.
 */
final class PluginManagerProxy extends ApiProxy {
    private final Bridge bridge;
    final PluginManager proxy;
    private final Map<String, Permission> permissions = new LinkedHashMap<>();
    private final Set<String> warned = new LinkedHashSet<>();

    PluginManagerProxy(Bridge bridge) {
        super(PluginManager.class);
        this.bridge = bridge;
        this.proxy = create(PluginManager.class);
    }

    @Override
    protected Object handle(Object p, Method m, String n, Object[] a) throws Throwable {
        switch (n) {
            case "registerEvents":
                registerEvents((Listener) a[0], (Plugin) a[1]);
                return null;
            case "registerEvent": {
                @SuppressWarnings("unchecked")
                Class<? extends Event> ec = (Class<? extends Event>) a[0];
                boolean ignore = a.length > 5 && (Boolean) a[5];
                register(ec, (Listener) a[1], (EventPriority) a[2], (EventExecutor) a[3], (Plugin) a[4], ignore);
                return null;
            }
            case "callEvent":
                callEvent((Event) a[0]);
                return null;
            case "getPlugin":
                for (LoadedPlugin lp : bridge.plugins.values())
                    if (lp.name().equalsIgnoreCase((String) a[0]) && lp.plugin != null) return lp.plugin;
                return null;
            case "getPlugins": {
                List<Plugin> out = new ArrayList<>();
                for (LoadedPlugin lp : bridge.plugins.values()) if (lp.plugin != null) out.add(lp.plugin);
                return out.toArray(new Plugin[0]);
            }
            case "isPluginEnabled":
                if (a[0] instanceof Plugin pl) return pl.isEnabled();
                for (LoadedPlugin lp : bridge.plugins.values())
                    if (lp.name().equalsIgnoreCase((String) a[0])) return lp.plugin != null && lp.plugin.isEnabled();
                return false;
            case "disablePlugin": {
                LoadedPlugin lp = bridge.of((Plugin) a[0]);
                if (lp != null && lp.plugin.isEnabled()) {
                    bridge.disable(lp.name());
                    bridge.host.post("disable_plugin", Json.obj("plugin", lp.name()));
                }
                return null;
            }
            case "enablePlugin":
                BridgeLog.warn("Rustic Bridge", "enablePlugin is refused: use `rustic enable` on the console");
                return null;
            case "getPermission":
                return permissions.get(((String) a[0]).toLowerCase(java.util.Locale.ROOT));
            case "addPermission": {
                Permission perm = (Permission) a[0];
                String key = perm.getName().toLowerCase(java.util.Locale.ROOT);
                if (permissions.containsKey(key))
                    throw new IllegalArgumentException("The permission " + key + " is already defined!");
                permissions.put(key, perm);
                List<Object> children = new ArrayList<>();
                for (Map.Entry<String, Boolean> ch : perm.getChildren().entrySet())
                    children.add(List.of(ch.getKey(), ch.getValue()));
                bridge.host.post("declare_permission", Json.obj(
                        "plugin", currentPluginName(),
                        "node", perm.getName(),
                        "default", perm.getDefault().name().toLowerCase(java.util.Locale.ROOT),
                        "description", perm.getDescription(),
                        "children", children));
                return null;
            }
            case "removePermission":
                permissions.remove((a[0] instanceof Permission pp ? pp.getName() : (String) a[0])
                        .toLowerCase(java.util.Locale.ROOT));
                return null;
            case "getPermissions":
                return new LinkedHashSet<>(permissions.values());
            case "getDefaultPermissions": {
                boolean op = (Boolean) a[0];
                Set<Permission> out = new LinkedHashSet<>();
                for (Permission perm : permissions.values()) {
                    PermissionDefault d = perm.getDefault();
                    if (d == PermissionDefault.TRUE || (op ? d == PermissionDefault.OP : d == PermissionDefault.NOT_OP))
                        out.add(perm);
                }
                return out;
            }
            case "recalculatePermissionDefaults":
            case "subscribeToPermission":
            case "unsubscribeFromPermission":
            case "subscribeToDefaultPerms":
            case "unsubscribeFromDefaultPerms":
                return null;
            case "getPermissionSubscriptions":
            case "getDefaultPermSubscriptions":
                return Set.of();
            case "useTimings":
                return false;
            case "clearPlugins":
            case "disablePlugins":
                return null;
            default:
                return UNHANDLED;
        }
    }

    /** The plugin whose code is running (the request's plugin), for calls without a plugin argument. */
    String current = "";

    private String currentPluginName() {
        return current;
    }

    void registerEvents(Listener listener, Plugin plugin) {
        if (!plugin.isEnabled())
            throw new IllegalPluginAccessException("Plugin attempted to register " + listener + " while not enabled");
        Set<Method> methods = new LinkedHashSet<>();
        for (Method mm : listener.getClass().getMethods()) methods.add(mm);
        for (Method mm : listener.getClass().getDeclaredMethods()) methods.add(mm);
        for (Method mm : methods) {
            EventHandler eh = mm.getAnnotation(EventHandler.class);
            if (eh == null || mm.isBridge() || mm.isSynthetic()) continue;
            Class<?>[] params = mm.getParameterTypes();
            if (params.length != 1 || !Event.class.isAssignableFrom(params[0])) {
                BridgeLog.warn(plugin.getName(), "invalid @EventHandler " + mm + " (needs one Event parameter)");
                continue;
            }
            @SuppressWarnings("unchecked")
            Class<? extends Event> ec = (Class<? extends Event>) params[0];
            mm.setAccessible(true);
            EventExecutor exec = (l, e) -> {
                if (!ec.isInstance(e)) return;
                try {
                    mm.invoke(Modifier.isStatic(mm.getModifiers()) ? null : l, e);
                } catch (InvocationTargetException ite) {
                    throw new EventException(ite.getCause());
                } catch (IllegalAccessException iae) {
                    throw new EventException(iae);
                }
            };
            register(ec, listener, eh.priority(), exec, plugin, eh.ignoreCancelled());
        }
    }

    void register(Class<? extends Event> ec, Listener listener, EventPriority prio, EventExecutor exec,
            Plugin plugin, boolean ignoreCancelled) {
        HandlerList hl = handlerList(ec);
        hl.register(new RegisteredListener(listener, exec, prio, plugin, ignoreCancelled));
        LoadedPlugin lp = bridge.of(plugin);
        List<String> rustic = EventMapper.rusticEvents(ec);
        if (lp != null) {
            for (String ev : rustic) bridge.subscribe(lp, ev, prio);
        }
        if (rustic.isEmpty() && !EventMapper.isPluginEvent(ec) && warned.add(ec.getName())) {
            BridgeLog.warn(plugin.getName(), "listens to " + ec.getSimpleName()
                    + ", which the Rustic Bridge does not fire yet; that handler will not be called");
        }
    }

    static HandlerList handlerList(Class<?> ec) {
        for (Class<?> c = ec; c != null && Event.class.isAssignableFrom(c); c = c.getSuperclass()) {
            try {
                Method get = c.getDeclaredMethod("getHandlerList");
                get.setAccessible(true);
                return (HandlerList) get.invoke(null);
            } catch (NoSuchMethodException e) {
                // try the superclass
            } catch (ReflectiveOperationException e) {
                throw new IllegalStateException("cannot read the handler list of " + ec.getName(), e);
            }
        }
        throw new IllegalPluginAccessException("Unable to find handler list for event " + ec.getName()
                + ". Static getHandlerList method required!");
    }

    /** Every listener, every priority (plugin events: {@code callEvent}). */
    void callEvent(Event e) {
        for (RegisteredListener rl : e.getHandlers().getRegisteredListeners()) {
            if (!rl.getPlugin().isEnabled()) continue;
            invoke(rl, e);
        }
    }

    /** One plugin's listeners at one priority (built-in events dispatched by the core). */
    void callEvent(Event e, Plugin plugin, EventPriority prio) {
        for (RegisteredListener rl : e.getHandlers().getRegisteredListeners()) {
            if (rl.getPlugin() != plugin || rl.getPriority() != prio) continue;
            invoke(rl, e);
        }
    }

    private void invoke(RegisteredListener rl, Event e) {
        String saved = current;
        current = rl.getPlugin().getName();
        try {
            rl.callEvent(e);
        } catch (Throwable t) {
            Throwable c = t instanceof EventException && t.getCause() != null ? t.getCause() : t;
            BridgeLog.error(rl.getPlugin().getName(), "Could not pass event " + e.getEventName() + " to "
                    + rl.getPlugin().getPluginMeta().getDisplayName(), c);
        } finally {
            current = saved;
        }
    }
}
