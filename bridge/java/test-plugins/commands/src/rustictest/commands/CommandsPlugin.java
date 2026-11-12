// SPDX-License-Identifier: GPL-3.0-only
package rustictest.commands;

import org.bukkit.Bukkit;
import org.bukkit.ChatColor;
import org.bukkit.command.Command;
import org.bukkit.command.CommandSender;
import org.bukkit.entity.Player;
import org.bukkit.plugin.java.JavaPlugin;
import org.bukkit.scheduler.BukkitRunnable;
import rustictest.shared.Lib;

/** /heal, /fly and /broadcast (a countdown with BukkitRunnable, the text built off-thread). */
public final class CommandsPlugin extends JavaPlugin {
    @Override
    public void onEnable() {
        getLogger().info("RusticCommands enabled, bundled lib " + Lib.VERSION);
    }

    private Player target(CommandSender sender, String[] args) {
        if (args.length >= 1) {
            Player p = Bukkit.getPlayer(args[0]);
            if (p == null) sender.sendMessage(ChatColor.RED + "No player named " + args[0] + ".");
            return p;
        }
        if (sender instanceof Player p) return p;
        sender.sendMessage(ChatColor.RED + "Name a player.");
        return null;
    }

    @Override
    public boolean onCommand(CommandSender sender, Command command, String label, String[] args) {
        switch (command.getName()) {
            case "heal" -> {
                Player p = target(sender, args);
                if (p == null) return true;
                p.setHealth(p.getMaxHealth());
                p.setFoodLevel(20);
                p.sendMessage(ChatColor.GREEN + "You have been healed.");
                if (p != sender) sender.sendMessage(ChatColor.GREEN + "Healed " + p.getName() + ".");
                return true;
            }
            case "fly" -> {
                Player p = target(sender, args);
                if (p == null) return true;
                boolean on = !p.getAllowFlight();
                p.setAllowFlight(on);
                p.sendMessage(ChatColor.AQUA + "Flight " + (on ? "enabled" : "disabled") + ".");
                return true;
            }
            case "broadcast" -> {
                if (args.length == 0) return false;
                String text = String.join(" ", args);
                String from = sender.getName();
                // Build the line on an async thread, then count down 3..1 on the main thread.
                Bukkit.getScheduler().runTaskAsynchronously(this, () -> {
                    String line = ChatColor.LIGHT_PURPLE + "[" + from + "] " + ChatColor.WHITE + text;
                    new BukkitRunnable() {
                        int left = 3;

                        @Override
                        public void run() {
                            if (left > 0) {
                                getLogger().info("broadcast in " + left + "...");
                                left--;
                                return;
                            }
                            Bukkit.broadcastMessage(line);
                            cancel();
                        }
                    }.runTaskTimer(CommandsPlugin.this, 0L, 10L);
                });
                sender.sendMessage(ChatColor.GRAY + "Broadcast scheduled.");
                return true;
            }
            default -> {
                return false;
            }
        }
    }
}
