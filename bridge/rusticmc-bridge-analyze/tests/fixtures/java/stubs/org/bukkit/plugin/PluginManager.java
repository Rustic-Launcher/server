// Minimal stand-in written for these tests (not Bukkit code).
package org.bukkit.plugin;

public interface PluginManager {
    void registerEvents(org.bukkit.event.Listener listener, Object plugin);
}
