// SPDX-License-Identifier: GPL-3.0-only
//
// The benchmark workload as an ordinary Bukkit plugin (docs/PERFORMANCE.md, "Plugin overhead"),
// original code compiled against the published Paper API. The same workload exists as a Rustic
// WASM plugin (bench/plugins/rustic-workload); keep the two equivalent:
//   - player move: per-player move count and distance travelled (from -> to);
//   - chat: lower-case the message, look for five banned words, cancel and warn on a match;
//   - /workload: replies with the counters;
//   - a task every tick: every online player's location, the farthest from (0, 0) kept.
package rustic.bench.workload;

import java.util.HashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.UUID;
import java.util.concurrent.atomic.AtomicLong;
import org.bukkit.Bukkit;
import org.bukkit.Location;
import org.bukkit.command.Command;
import org.bukkit.command.CommandSender;
import org.bukkit.entity.Player;
import org.bukkit.event.EventHandler;
import org.bukkit.event.Listener;
import org.bukkit.event.player.AsyncPlayerChatEvent;
import org.bukkit.event.player.PlayerMoveEvent;
import org.bukkit.plugin.java.JavaPlugin;

public final class WorkloadPlugin extends JavaPlugin implements Listener {
    private static final List<String> BANNED = List.of("badword", "spoiler", "griefer", "lagmachine", "dupe");

    /** Per player: moves seen and blocks travelled (main thread only). */
    private static final class Track {
        long moves;
        double distance;
    }

    private final Map<UUID, Track> tracks = new HashMap<>();
    private final AtomicLong chats = new AtomicLong();
    private final AtomicLong blocked = new AtomicLong();
    private long ticks;
    private double farthest;

    @Override
    public void onEnable() {
        getServer().getPluginManager().registerEvents(this, this);
        Bukkit.getScheduler().runTaskTimer(this, this::tick, 1L, 1L);
        getLogger().info("workload enabled");
    }

    private void tick() {
        ticks++;
        for (Player p : Bukkit.getOnlinePlayers()) {
            Location l = p.getLocation();
            double d = Math.hypot(l.getX(), l.getZ());
            if (d > farthest) {
                farthest = d;
            }
        }
    }

    @EventHandler
    public void onMove(PlayerMoveEvent e) {
        Location from = e.getFrom();
        Location to = e.getTo();
        Track t = tracks.computeIfAbsent(e.getPlayer().getUniqueId(), u -> new Track());
        t.moves++;
        double dx = to.getX() - from.getX(), dy = to.getY() - from.getY(), dz = to.getZ() - from.getZ();
        t.distance += Math.sqrt(dx * dx + dy * dy + dz * dz);
    }

    @EventHandler(ignoreCancelled = true)
    @SuppressWarnings("deprecation") // AsyncPlayerChatEvent: still fired by Paper and the bridge
    public void onChat(AsyncPlayerChatEvent e) {
        chats.incrementAndGet();
        String message = e.getMessage().toLowerCase(Locale.ROOT);
        for (String word : BANNED) {
            if (message.contains(word)) {
                e.setCancelled(true);
                blocked.incrementAndGet();
                e.getPlayer().sendMessage("Please do not say '" + word + "'.");
                return;
            }
        }
    }

    @Override
    public boolean onCommand(CommandSender sender, Command command, String label, String[] args) {
        long moves = 0;
        double distance = 0;
        for (Track t : tracks.values()) {
            moves += t.moves;
            distance += t.distance;
        }
        sender.sendMessage(String.format(Locale.ROOT,
                "workload: %d ticks, %d moves (%.1f blocks), %d chats (%d blocked), farthest %.1f",
                ticks, moves, distance, chats.get(), blocked.get(), farthest));
        return true;
    }
}
