// Minimal stand-in written for these tests (not Bukkit code).
package org.bukkit.scheduler;

public abstract class BukkitRunnable implements Runnable {
    public Object runTaskTimer(Object plugin, long delay, long period) { return null; }
}
