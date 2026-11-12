// SPDX-License-Identifier: GPL-3.0-only
//
// Tick recorder for the Paper side of the benchmark (docs/PERFORMANCE.md), original code compiled
// against the published Paper API. Console:
//   benchprobe start            starts a window
//   benchprobe stop <file>      ends it and writes <file> (JSON): every tick's duration in ms as
//                               Paper's ServerTickEndEvent reports it, the main thread's CPU time,
//                               GC counts and time, and the heap.
// Rustic's side uses Horizon's own `horizon-report` (per-tick wall time); bench/summarize.py
// computes the percentiles of both the same way.
package rustic.bench.probe;

import com.destroystokyo.paper.event.server.ServerTickEndEvent;
import java.io.IOException;
import java.lang.management.GarbageCollectorMXBean;
import java.lang.management.ManagementFactory;
import java.lang.management.ThreadMXBean;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Locale;
import org.bukkit.Bukkit;
import org.bukkit.command.Command;
import org.bukkit.command.CommandSender;
import org.bukkit.event.EventHandler;
import org.bukkit.event.EventPriority;
import org.bukkit.event.Listener;
import org.bukkit.plugin.java.JavaPlugin;

public final class ProbePlugin extends JavaPlugin implements Listener {
    private final ThreadMXBean threads = ManagementFactory.getThreadMXBean();
    private long mainThreadId;
    private boolean recording;
    private double[] ticks = new double[1 << 14];
    private int count;
    private long startNanos;
    private long startCpu;
    private long startGcCount;
    private long startGcMs;

    @Override
    public void onEnable() {
        mainThreadId = Thread.currentThread().threadId();
        getServer().getPluginManager().registerEvents(this, this);
    }

    @EventHandler(priority = EventPriority.MONITOR)
    public void onTickEnd(ServerTickEndEvent e) {
        if (!recording) {
            return;
        }
        if (count == ticks.length) {
            ticks = java.util.Arrays.copyOf(ticks, count * 2);
        }
        ticks[count++] = e.getTickDuration();
    }

    private static long[] gc() {
        long n = 0, ms = 0;
        for (GarbageCollectorMXBean b : ManagementFactory.getGarbageCollectorMXBeans()) {
            n += Math.max(0, b.getCollectionCount());
            ms += Math.max(0, b.getCollectionTime());
        }
        return new long[] {n, ms};
    }

    @Override
    public boolean onCommand(CommandSender sender, Command command, String label, String[] args) {
        if (args.length >= 1 && args[0].equals("start")) {
            count = 0;
            long[] g = gc();
            startGcCount = g[0];
            startGcMs = g[1];
            startCpu = threads.getThreadCpuTime(mainThreadId);
            startNanos = System.nanoTime();
            recording = true;
            sender.sendMessage("benchprobe: recording");
            return true;
        }
        if (args.length >= 2 && args[0].equals("stop")) {
            recording = false;
            double wall = (System.nanoTime() - startNanos) / 1e9;
            double cpuMs = (threads.getThreadCpuTime(mainThreadId) - startCpu) / 1e6;
            long[] g = gc();
            Runtime rt = Runtime.getRuntime();
            StringBuilder sb = new StringBuilder(count * 8 + 512);
            sb.append(String.format(Locale.ROOT,
                    "{\"window_s\":%.3f,\"ticks\":%d,\"main_thread_cpu_ms\":%.3f,\"gc_count\":%d,\"gc_ms\":%d,"
                            + "\"heap_used_mb\":%d,\"heap_committed_mb\":%d,\"heap_max_mb\":%d,\"tps\":%.3f,"
                            + "\"players\":%d,\"tick_ms\":[",
                    wall, count, cpuMs, g[0] - startGcCount, g[1] - startGcMs,
                    (rt.totalMemory() - rt.freeMemory()) >> 20, rt.totalMemory() >> 20, rt.maxMemory() >> 20,
                    Bukkit.getTPS()[0], Bukkit.getOnlinePlayers().size()));
            for (int i = 0; i < count; i++) {
                if (i > 0) {
                    sb.append(',');
                }
                sb.append(String.format(Locale.ROOT, "%.4f", ticks[i]));
            }
            sb.append("]}\n");
            try {
                Files.writeString(Path.of(args[1]), sb, StandardCharsets.UTF_8);
            } catch (IOException ex) {
                sender.sendMessage("benchprobe: " + ex);
                return true;
            }
            sender.sendMessage("benchprobe: wrote " + count + " ticks to " + args[1]);
            return true;
        }
        sender.sendMessage("benchprobe start | stop <file>");
        return true;
    }
}
