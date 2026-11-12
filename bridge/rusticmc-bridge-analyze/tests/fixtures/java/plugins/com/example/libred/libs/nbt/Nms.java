package com.example.libred.libs.nbt;

// A "shaded library" reaching into CraftBukkit by reflection.
public final class Nms {
    public static Object version() {
        try {
            return Class.forName("org.bukkit.craftbukkit.CraftServer");
        } catch (ClassNotFoundException e) {
            return null;
        }
    }
}
