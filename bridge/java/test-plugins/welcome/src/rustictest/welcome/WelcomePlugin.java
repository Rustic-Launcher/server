// SPDX-License-Identifier: GPL-3.0-only
package rustictest.welcome;

import org.bukkit.ChatColor;
import org.bukkit.command.Command;
import org.bukkit.command.CommandSender;
import org.bukkit.entity.Player;
import org.bukkit.event.EventHandler;
import org.bukkit.event.EventPriority;
import org.bukkit.event.Listener;
import org.bukkit.event.player.AsyncPlayerChatEvent;
import org.bukkit.event.player.PlayerJoinEvent;
import org.bukkit.event.player.PlayerQuitEvent;
import org.bukkit.plugin.java.JavaPlugin;
import rustictest.shared.Lib;

/** Greets players from config.yml, delays a second greeting with the scheduler, filters a word. */
public final class WelcomePlugin extends JavaPlugin implements Listener {
    private int visitors;

    @Override
    public void onEnable() {
        saveDefaultConfig();
        getServer().getPluginManager().registerEvents(this, this);
        getCommand("welcome").setExecutor(this);
        visitors = getConfig().getInt("visitors", 0);
        getLogger().info("Welcome enabled: join message '" + getConfig().getString("join-message")
                + "', bundled lib " + Lib.VERSION);
    }

    @Override
    public void onDisable() {
        getConfig().set("visitors", visitors);
        saveConfig();
        getLogger().info("Welcome disabled after " + visitors + " visitor(s)");
    }

    private String format(String key, Player p) {
        String raw = getConfig().getString(key, "");
        return ChatColor.translateAlternateColorCodes('&',
                raw.replace("{player}", p.getName()).replace("{count}", String.valueOf(visitors)));
    }

    @EventHandler
    public void onJoin(PlayerJoinEvent e) {
        Player p = e.getPlayer();
        visitors++;
        e.setJoinMessage(format("join-message", p));
        p.sendMessage(format("private-message", p));
        long delay = getConfig().getLong("delay-ticks", 40);
        getServer().getScheduler().runTaskLater(this, () -> {
            if (p.isOnline()) {
                p.sendMessage(format("delayed-message", p));
                getLogger().info("delayed greeting sent to " + p.getName());
            }
        }, delay);
    }

    @EventHandler(priority = EventPriority.MONITOR)
    public void onQuit(PlayerQuitEvent e) {
        e.setQuitMessage(format("quit-message", e.getPlayer()));
    }

    @EventHandler(ignoreCancelled = true)
    public void onChat(AsyncPlayerChatEvent e) {
        String word = getConfig().getString("blocked-word", "");
        if (!word.isEmpty() && e.getMessage().toLowerCase().contains(word)) {
            e.setCancelled(true);
            e.getPlayer().sendMessage(ChatColor.RED + "Please do not say '" + word + "'.");
            getLogger().info("blocked a message from " + e.getPlayer().getName());
        }
    }

    @Override
    public boolean onCommand(CommandSender sender, Command command, String label, String[] args) {
        if (args.length == 0) {
            sender.sendMessage(ChatColor.GOLD + "Join message: " + ChatColor.RESET
                    + ChatColor.translateAlternateColorCodes('&', getConfig().getString("join-message", "")));
            return true;
        }
        if (args.length == 1 && args[0].equalsIgnoreCase("reload")) {
            if (!sender.hasPermission("welcome.reload")) {
                sender.sendMessage(ChatColor.RED + "You may not reload Welcome.");
                return true;
            }
            reloadConfig();
            sender.sendMessage(ChatColor.GREEN + "Welcome configuration reloaded.");
            getLogger().info("configuration reloaded by " + sender.getName());
            return true;
        }
        return false;
    }
}
