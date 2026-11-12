package com.example.yellow;

import org.bukkit.Bukkit;
import org.bukkit.entity.Player;
import org.bukkit.entity.Zombie;
import org.bukkit.plugin.java.JavaPlugin;

public class YellowPlugin extends JavaPlugin {
    @Override
    public void onEnable() {
        for (Player p : Bukkit.getOnlinePlayers()) {
            p.setScoreboard(null);
        }
    }

    boolean isZombie(Object o) {
        return o instanceof Zombie;
    }
}
