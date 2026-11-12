package com.example.green;

import java.util.List;
import org.bukkit.Bukkit;
import org.bukkit.entity.Player;
import org.bukkit.event.EventHandler;
import org.bukkit.event.Listener;
import org.bukkit.event.player.PlayerJoinEvent;
import org.bukkit.plugin.java.JavaPlugin;

public class GreenPlugin extends JavaPlugin implements Listener {
    @Override
    public void onEnable() {
        getLogger().info("enabled");
        Bukkit.getPluginManager().registerEvents(this, this);
        new Ticker().runTaskTimer(this, 0, 20);
        greet("x");
    }

    void greet(String s) {}

    @EventHandler
    public void onJoin(PlayerJoinEvent e) {
        e.getPlayer().sendMessage("welcome");
    }

    List<String> names() {
        return Bukkit.getOnlinePlayers().stream().map(Player::getName).toList();
    }
}
