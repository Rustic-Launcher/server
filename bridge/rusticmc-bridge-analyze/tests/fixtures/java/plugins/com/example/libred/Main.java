package com.example.libred;

import org.bukkit.plugin.java.JavaPlugin;

public class Main extends JavaPlugin {
    @Override
    public void onEnable() {
        getLogger().info(String.valueOf(com.example.libred.libs.nbt.Nms.version()));
    }
}
