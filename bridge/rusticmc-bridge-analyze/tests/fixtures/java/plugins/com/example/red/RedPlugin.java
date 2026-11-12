package com.example.red;

import org.bukkit.craftbukkit.entity.CraftPlayer;
import org.bukkit.entity.Player;
import org.bukkit.plugin.java.JavaPlugin;

public class RedPlugin extends JavaPlugin {
    Object handle(Player p) throws Exception {
        Class.forName("net.minecraft.server.MinecraftServer");
        return ((CraftPlayer) (Object) p).getHandle();
    }
}
