// SPDX-License-Identifier: GPL-3.0-only
package rustictest.probe;

import java.io.File;
import java.sql.Connection;
import java.sql.DriverManager;
import java.sql.ResultSet;
import java.sql.Statement;
import java.util.concurrent.atomic.AtomicLong;
import org.bukkit.Bukkit;
import org.bukkit.command.Command;
import org.bukkit.command.CommandSender;
import org.bukkit.entity.Player;
import org.bukkit.event.EventHandler;
import org.bukkit.event.Listener;
import org.bukkit.event.player.AsyncPlayerChatEvent;
import org.bukkit.event.player.PlayerMoveEvent;
import org.bukkit.plugin.java.JavaPlugin;

/** Exercises what the bridge spike measures: faults, plugin threads, a shaded JDBC driver. */
public final class ProbePlugin extends JavaPlugin implements Listener {
    public static final AtomicLong MOVES = new AtomicLong();

    @Override
    public void onEnable() {
        getServer().getPluginManager().registerEvents(this, this);
        if (getConfig().getBoolean("tick-task", true)) {
            getServer().getScheduler().runTaskTimer(this, () -> MOVES.get(), 1L, 1L);
        }
    }

    @EventHandler
    public void onChat(AsyncPlayerChatEvent e) {
        if (e.getMessage().contains("cancelme")) e.setCancelled(true);
    }

    @EventHandler(ignoreCancelled = true)
    public void onMove(PlayerMoveEvent e) {
        MOVES.incrementAndGet();
        if (e.getTo() != null && e.getTo().getY() > 1000) e.setCancelled(true);
    }

    @Override
    public boolean onCommand(CommandSender sender, Command command, String label, String[] args) {
        String what = args.length > 0 ? args[0] : "";
        switch (what) {
            case "throw" -> throw new IllegalStateException("probe: thrown on purpose");
            case "exit" -> {
                System.exit(3);
                sender.sendMessage("still alive after System.exit");
            }
            case "halt" -> Runtime.getRuntime().halt(4);
            case "loop" -> {
                long n = 0;
                while (!Thread.currentThread().isInterrupted()) n++;
                sender.sendMessage("loop ended " + n);
            }
            case "async" -> {
                Player p = sender instanceof Player pl ? pl : null;
                new Thread(() -> {
                    long t0 = System.nanoTime();
                    int online = Bukkit.getOnlinePlayers().size();
                    double health = p != null ? p.getHealth() : -1;
                    long ms = (System.nanoTime() - t0) / 1_000_000;
                    sender.sendMessage("async thread: " + online + " online, health " + health
                            + ", round trip through the main thread " + ms + " ms");
                    getLogger().info("async probe done in " + ms + " ms");
                }, "probe-async").start();
            }
            case "sqlite" -> {
                File db = new File(getDataFolder(), "probe.db");
                getDataFolder().mkdirs();
                try {
                    // A shaded driver registers itself when loaded, as on Paper (no SPI scan of plugin jars).
                    Class.forName("org.sqlite.JDBC");
                } catch (ClassNotFoundException ex) {
                    sender.sendMessage("sqlite driver missing: " + ex);
                    return true;
                }
                try (Connection c = DriverManager.getConnection("jdbc:sqlite:" + db.getAbsolutePath());
                        Statement s = c.createStatement()) {
                    s.executeUpdate("create table if not exists hits (at integer)");
                    s.executeUpdate("insert into hits values (" + System.currentTimeMillis() + ")");
                    try (ResultSet rs = s.executeQuery("select count(*) from hits")) {
                        rs.next();
                        int rows = rs.getInt(1);
                        sender.sendMessage("sqlite: " + rows + " row(s) in " + db.getName()
                                + " (driver " + c.getMetaData().getDriverVersion() + ")");
                        getLogger().info("sqlite rows " + rows);
                    }
                } catch (Exception ex) {
                    sender.sendMessage("sqlite failed: " + ex);
                    getLogger().warning("sqlite failed: " + ex);
                }
            }
            default -> {
                return false;
            }
        }
        return true;
    }
}
