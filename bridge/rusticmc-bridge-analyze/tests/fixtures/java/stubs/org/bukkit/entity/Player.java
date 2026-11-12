// Minimal stand-in written for these tests (not Bukkit code).
package org.bukkit.entity;

public interface Player {
    void sendMessage(String message);
    String getName();
    void setScoreboard(Object board);
}
