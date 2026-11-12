// SPDX-License-Identifier: GPL-3.0-only
package rustic.bridge;

import java.io.File;
import java.io.InputStream;
import java.util.Map;
import java.util.Set;
import java.util.jar.JarFile;
import java.util.regex.Pattern;
import org.bukkit.event.Event;
import org.bukkit.event.Listener;
import org.bukkit.plugin.InvalidDescriptionException;
import org.bukkit.plugin.InvalidPluginException;
import org.bukkit.plugin.Plugin;
import org.bukkit.plugin.PluginDescriptionFile;
import org.bukkit.plugin.PluginLoader;
import org.bukkit.plugin.RegisteredListener;

/**
 * The {@link PluginLoader} service the API looks up ({@code JavaPlugin#getPluginLoader}). Loading
 * is the host's job (plugins come from {@code plugins/bukkit/} through the Rustic core), so only
 * the descriptor reader and enable/disable are provided.
 */
public final class BukkitLoader implements PluginLoader {
    @Override
    public Plugin loadPlugin(File file) throws InvalidPluginException {
        throw new InvalidPluginException("Rustic Bridge: plugins are loaded by the server (plugins/bukkit/)");
    }

    @Override
    public PluginDescriptionFile getPluginDescription(File file) throws InvalidDescriptionException {
        try (JarFile jar = new JarFile(file)) {
            var e = jar.getEntry("plugin.yml");
            if (e == null) throw new InvalidDescriptionException("no plugin.yml in " + file);
            try (InputStream in = jar.getInputStream(e)) {
                return new PluginDescriptionFile(in);
            }
        } catch (java.io.IOException ex) {
            throw new InvalidDescriptionException(ex);
        }
    }

    @Override
    public Pattern[] getPluginFileFilters() {
        return new Pattern[] {Pattern.compile("\\.jar$")};
    }

    @Override
    public Map<Class<? extends Event>, Set<RegisteredListener>> createRegisteredListeners(Listener l, Plugin p) {
        throw new UnsupportedOperationException("Rustic Bridge: PluginLoader.createRegisteredListeners not supported yet");
    }

    @Override
    public void enablePlugin(Plugin plugin) {
        throw new UnsupportedOperationException("Rustic Bridge: enable plugins with `rustic enable` on the console");
    }

    @Override
    public void disablePlugin(Plugin plugin) {
        Bridge b = Bridge.current;
        if (b != null) b.pluginManager.proxy.disablePlugin(plugin);
    }
}
