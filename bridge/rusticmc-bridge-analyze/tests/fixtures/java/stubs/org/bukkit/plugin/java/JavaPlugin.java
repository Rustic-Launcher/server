// Minimal stand-in written for these tests (not Bukkit code): just enough to compile fixtures.
package org.bukkit.plugin.java;

public abstract class JavaPlugin {
    public java.util.logging.Logger getLogger() { return null; }
    public void onEnable() {}
}
