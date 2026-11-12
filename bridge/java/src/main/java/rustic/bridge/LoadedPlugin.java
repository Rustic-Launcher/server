// SPDX-License-Identifier: GPL-3.0-only
package rustic.bridge;

import java.io.File;
import java.lang.reflect.Constructor;
import java.util.ArrayList;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Set;
import org.bukkit.command.PluginCommand;
import org.bukkit.plugin.Plugin;
import org.bukkit.plugin.PluginDescriptionFile;
import org.bukkit.plugin.java.JavaPlugin;

/** One Bukkit plugin in the bridge: its descriptor, class loader, instance and commands. */
final class LoadedPlugin {
    final Bridge bridge;
    final PluginDescriptionFile description;
    final File file;
    final File dataFolder;
    PluginClassLoader loader;
    JavaPlugin plugin;
    /** plugin.yml `commands:` entries by lower-case name. */
    final Map<String, Map<String, Object>> commandSpecs = new LinkedHashMap<>();
    Constructor<PluginCommand> commandCtor;
    /** Commands by lower-case name and alias. */
    final Map<String, PluginCommand> commands = new LinkedHashMap<>();
    /** `<rustic event>@<PRIORITY>` this plugin is subscribed to in the host. */
    final Set<String> subscriptions = new HashSet<>();

    LoadedPlugin(Bridge bridge, PluginDescriptionFile description, File file, File dataFolder) {
        this.bridge = bridge;
        this.description = description;
        this.file = file;
        this.dataFolder = dataFolder;
    }

    String name() {
        return description.getName();
    }

    /** Create the {@link PluginCommand} objects of plugin.yml (owner: the plugin instance). */
    void buildCommands() throws ReflectiveOperationException {
        if (!commands.isEmpty()) return;
        for (Map.Entry<String, Map<String, Object>> e : commandSpecs.entrySet()) {
            Map<String, Object> v = e.getValue();
            PluginCommand c = commandCtor.newInstance(e.getKey(), (Plugin) plugin);
            Object d = v.get("description");
            if (d != null) c.setDescription(String.valueOf(d));
            Object u = v.get("usage");
            if (u != null) c.setUsage(String.valueOf(u));
            Object p = v.get("permission");
            if (p != null) c.setPermission(String.valueOf(p));
            Object pm = v.get("permission-message");
            if (pm != null) c.setPermissionMessage(String.valueOf(pm));
            List<String> aliases = new ArrayList<>();
            Object a = v.get("aliases");
            if (a instanceof List<?> l) for (Object x : l) aliases.add(String.valueOf(x));
            else if (a != null) aliases.add(String.valueOf(a));
            c.setAliases(aliases);
            commands.put(e.getKey(), c);
            for (String al : aliases) commands.putIfAbsent(al.toLowerCase(Locale.ROOT), c);
        }
    }
}
