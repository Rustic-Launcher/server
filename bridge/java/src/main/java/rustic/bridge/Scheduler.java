// SPDX-License-Identifier: GPL-3.0-only
package rustic.bridge;

import java.lang.reflect.Method;
import java.util.ArrayList;
import java.util.Iterator;
import java.util.List;
import java.util.Map;
import java.util.concurrent.Callable;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.ConcurrentLinkedQueue;
import java.util.concurrent.Executor;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.ScheduledFuture;
import java.util.concurrent.ScheduledThreadPoolExecutor;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;
import java.util.function.Consumer;
import org.bukkit.plugin.IllegalPluginAccessException;
import org.bukkit.plugin.Plugin;
import org.bukkit.scheduler.BukkitScheduler;
import org.bukkit.scheduler.BukkitTask;
import org.bukkit.scheduler.BukkitWorker;
import io.papermc.paper.threadedregions.scheduler.AsyncScheduler;
import io.papermc.paper.threadedregions.scheduler.GlobalRegionScheduler;
import io.papermc.paper.threadedregions.scheduler.RegionScheduler;
import io.papermc.paper.threadedregions.scheduler.ScheduledTask;

/**
 * {@link BukkitScheduler} on the host's tick: the host sends one {@code tick} pulse per server
 * tick and the due synchronous tasks run in it, on the main thread, in scheduling order. Async
 * tasks run on the bridge's worker pool (delays in ticks are 50 ms each); what they do to the
 * game is marshalled back to the main thread by {@link Host}.
 */
final class Scheduler extends ApiProxy {
    private final Bridge bridge;
    final BukkitScheduler proxy;
    private final AtomicInteger ids = new AtomicInteger(1);
    private final Map<Integer, Task> tasks = new ConcurrentHashMap<>();
    private final ConcurrentLinkedQueue<Task> incoming = new ConcurrentLinkedQueue<>();
    private final List<Task> sync = new ArrayList<>();
    private final ScheduledExecutorService async;
    private volatile int running = -1;

    /** Paper's region/global/async schedulers (Folia API) on the same tick pulse. */
    final GlobalRegionScheduler global;
    final RegionScheduler region;
    final AsyncScheduler asyncScheduler;

    Scheduler(Bridge bridge) {
        super(BukkitScheduler.class);
        this.bridge = bridge;
        this.proxy = create(BukkitScheduler.class);
        this.global = new Folia(GlobalRegionScheduler.class, 0).create(GlobalRegionScheduler.class);
        this.region = new Folia(RegionScheduler.class, 3).create(RegionScheduler.class);
        this.asyncScheduler = new Folia(AsyncScheduler.class, -1).create(AsyncScheduler.class);
        AtomicInteger n = new AtomicInteger();
        ScheduledThreadPoolExecutor ex = new ScheduledThreadPoolExecutor(4, r -> {
            Thread t = new Thread(r, "Rustic Bridge Async Worker #" + n.incrementAndGet());
            t.setDaemon(true);
            return t;
        });
        ex.setRemoveOnCancelPolicy(true);
        this.async = ex;
    }

    /** A scheduled task (implements the published {@link BukkitTask} interface). */
    final class Task implements BukkitTask {
        final int id;
        final Plugin owner;
        final Runnable body;
        final boolean isSync;
        final long period;
        long next;
        volatile boolean cancelled;
        ScheduledFuture<?> future;
        /** Delay and period in milliseconds for async tasks (ticks * 50 unless given). */
        long asyncDelayMs = -1;
        long asyncPeriodMs;

        Task(Plugin owner, Runnable body, boolean isSync, long delay, long period) {
            this.id = ids.getAndIncrement();
            this.owner = owner;
            this.body = body;
            this.isSync = isSync;
            this.period = period;
            this.next = bridge.tick + Math.max(1, delay);
        }

        @Override
        public int getTaskId() {
            return id;
        }

        @Override
        public Plugin getOwner() {
            return owner;
        }

        @Override
        public boolean isSync() {
            return isSync;
        }

        @Override
        public boolean isCancelled() {
            return cancelled;
        }

        @Override
        public void cancel() {
            cancelled = true;
            tasks.remove(id);
            if (future != null) future.cancel(false);
        }

        void runOnce() {
            running = id;
            try {
                body.run();
            } catch (Throwable t) {
                BridgeLog.error(owner.getName(), "Plugin " + owner.getPluginMeta().getDisplayName()
                        + " generated an exception while executing task " + id, t);
            } finally {
                running = -1;
            }
        }
    }

    private Task schedule(Plugin plugin, Object body, boolean isSync, long delay, long period) {
        if (plugin == null) throw new IllegalArgumentException("Plugin cannot be null");
        if (!plugin.isEnabled()) throw new IllegalPluginAccessException("Plugin attempted to register task while disabled");
        Task[] self = new Task[1];
        Runnable r;
        if (body instanceof Runnable run) r = run;
        else if (body instanceof Consumer<?> c) {
            @SuppressWarnings("unchecked")
            Consumer<BukkitTask> cc = (Consumer<BukkitTask>) c;
            r = () -> cc.accept(self[0]);
        } else if (body instanceof Callable<?> call) {
            r = () -> {
                try {
                    call.call();
                } catch (Exception e) {
                    throw new RuntimeException(e);
                }
            };
        } else throw new IllegalArgumentException("not a task: " + body);
        Task t = new Task(plugin, r, isSync, delay, period);
        self[0] = t;
        tasks.put(t.id, t);
        if (isSync) {
            incoming.add(t);
        } else {
            long d = Math.max(0, delay) * 50;
            t.future = period > 0
                    ? async.scheduleAtFixedRate(t::runOnce, d, Math.max(1, period * 50), TimeUnit.MILLISECONDS)
                    : async.schedule(() -> {
                        t.runOnce();
                        tasks.remove(t.id);
                    }, d, TimeUnit.MILLISECONDS);
        }
        return t;
    }

    private final ConcurrentLinkedQueue<Runnable> afterTick = new ConcurrentLinkedQueue<>();

    /** Run once at the start of the next tick pulse (after the current event has reached everyone). */
    void afterTick(Runnable r) {
        afterTick.add(r);
    }

    /** One server tick: run the due synchronous tasks (main thread). */
    void pulse(long tick) {
        Runnable r;
        while ((r = afterTick.poll()) != null) r.run();
        Task t;
        while ((t = incoming.poll()) != null) sync.add(t);
        if (sync.isEmpty()) return;
        List<Task> due = new ArrayList<>();
        for (Iterator<Task> it = sync.iterator(); it.hasNext(); ) {
            Task x = it.next();
            if (x.cancelled) {
                it.remove();
            } else if (x.next <= tick) {
                due.add(x);
                if (x.period > 0) x.next = tick + x.period;
                else it.remove();
            }
        }
        for (Task x : due) {
            if (x.cancelled || !x.owner.isEnabled()) continue;
            x.runOnce();
            if (x.period <= 0) tasks.remove(x.id);
        }
    }

    /** An async task with a delay and period in milliseconds (Paper's AsyncScheduler). */
    private Task scheduleAsyncMs(Plugin plugin, Runnable r, long delayMs, long periodMs) {
        if (!plugin.isEnabled()) throw new IllegalPluginAccessException("Plugin attempted to register task while disabled");
        Task t = new Task(plugin, r, false, 0, periodMs > 0 ? 1 : 0);
        tasks.put(t.id, t);
        t.future = periodMs > 0
                ? async.scheduleAtFixedRate(t::runOnce, Math.max(0, delayMs), periodMs, TimeUnit.MILLISECONDS)
                : async.schedule(() -> {
                    t.runOnce();
                    tasks.remove(t.id);
                }, Math.max(0, delayMs), TimeUnit.MILLISECONDS);
        return t;
    }

    /** A {@link ScheduledTask} view of a task. */
    private ScheduledTask scheduled(Task t) {
        return new ApiProxy(ScheduledTask.class) {
            @Override
            protected Object handle(Object p, Method m, String n, Object[] a) {
                return switch (n) {
                    case "getOwningPlugin" -> t.owner;
                    case "isRepeatingTask" -> t.period > 0;
                    case "isCancelled" -> t.cancelled;
                    case "cancel" -> {
                        boolean was = t.cancelled;
                        t.cancel();
                        yield ScheduledTask.CancelledState.valueOf(was ? "CANCELLED_ALREADY" : "CANCELLED_BY_CALLER");
                    }
                    case "getExecutionState" -> ScheduledTask.ExecutionState.valueOf(
                            t.cancelled ? "CANCELLED" : running == t.id ? "RUNNING" : "IDLE");
                    default -> UNHANDLED;
                };
            }
        }.create(ScheduledTask.class);
    }

    /** Global (skip 0 location arguments), region (skip world+x+z or a location) or async (-1). */
    private final class Folia extends ApiProxy {
        private final int skip;

        Folia(Class<?> type, int skip) {
            super(type);
            this.skip = skip;
        }

        @Override
        protected Object handle(Object px, Method m, String n, Object[] a) {
            if (n.equals("cancelTasks")) {
                cancelTasks((Plugin) a[0]);
                return null;
            }
            if (a.length == 0 || !(a[0] instanceof Plugin plugin)) return UNHANDLED;
            int i = 1;
            if (skip > 0) i += (a.length > 1 && a[1] instanceof org.bukkit.Location) ? 1 : skip;
            if (i >= a.length) return UNHANDLED;
            Object body = a[i];
            ScheduledTask[] st = new ScheduledTask[1];
            Runnable r;
            if (body instanceof Runnable run) r = run;
            else if (body instanceof Consumer<?> c) {
                @SuppressWarnings("unchecked")
                Consumer<ScheduledTask> cc = (Consumer<ScheduledTask>) c;
                r = () -> cc.accept(st[0]);
            } else return UNHANDLED;
            long x = a.length > i + 1 && a[i + 1] instanceof Number v ? v.longValue() : 0;
            long y = a.length > i + 2 && a[i + 2] instanceof Number v ? v.longValue() : 0;
            Task t;
            if (skip < 0) {
                TimeUnit unit = a[a.length - 1] instanceof TimeUnit u ? u : TimeUnit.MILLISECONDS;
                t = switch (n) {
                    case "runNow" -> scheduleAsyncMs(plugin, r, 0, 0);
                    case "runDelayed" -> scheduleAsyncMs(plugin, r, unit.toMillis(x), 0);
                    case "runAtFixedRate" -> scheduleAsyncMs(plugin, r, unit.toMillis(x), Math.max(1, unit.toMillis(y)));
                    default -> null;
                };
            } else {
                t = switch (n) {
                    case "execute", "run" -> schedule(plugin, r, true, 0, 0);
                    case "runDelayed" -> schedule(plugin, r, true, x, 0);
                    case "runAtFixedRate" -> schedule(plugin, r, true, x, Math.max(1, y));
                    default -> null;
                };
            }
            if (t == null) return UNHANDLED;
            st[0] = scheduled(t);
            return m.getReturnType() == void.class ? null : st[0];
        }
    }

    void cancelTasks(Plugin p) {
        if (p == null) return;
        for (Task t : new ArrayList<>(tasks.values())) if (t.owner == p) t.cancel();
    }

    void shutdown() {
        async.shutdownNow();
    }

    @Override
    protected Object handle(Object px, Method m, String n, Object[] a) {
        switch (n) {
            case "runTask":
            case "runTaskLater":
            case "runTaskTimer":
            case "runTaskAsynchronously":
            case "runTaskLaterAsynchronously":
            case "runTaskTimerAsynchronously": {
                boolean isSync = !n.contains("Asynchronously");
                long delay = a.length > 2 ? ((Number) a[2]).longValue() : 0;
                long period = a.length > 3 ? ((Number) a[3]).longValue() : 0;
                Task t = schedule((Plugin) a[0], a[1], isSync, delay, period);
                return m.getReturnType() == void.class ? null : t;
            }
            case "scheduleSyncDelayedTask":
            case "scheduleAsyncDelayedTask":
                return schedule((Plugin) a[0], a[1], n.contains("Sync"),
                        a.length > 2 ? ((Number) a[2]).longValue() : 0, 0).id;
            case "scheduleSyncRepeatingTask":
            case "scheduleAsyncRepeatingTask":
                return schedule((Plugin) a[0], a[1], n.contains("Sync"),
                        ((Number) a[2]).longValue(), ((Number) a[3]).longValue()).id;
            case "callSyncMethod": {
                CompletableFuture<Object> f = new CompletableFuture<>();
                Callable<?> c = (Callable<?>) a[1];
                schedule((Plugin) a[0], (Runnable) () -> {
                    try {
                        f.complete(c.call());
                    } catch (Throwable t) {
                        f.completeExceptionally(t);
                    }
                }, true, 0, 0);
                return f;
            }
            case "cancelTask": {
                Task t = tasks.get((Integer) a[0]);
                if (t != null) t.cancel();
                return null;
            }
            case "cancelTasks":
                cancelTasks((Plugin) a[0]);
                return null;
            case "isCurrentlyRunning":
                return running == (Integer) a[0];
            case "isQueued":
                return tasks.containsKey((Integer) a[0]);
            case "getPendingTasks":
                return new ArrayList<BukkitTask>(tasks.values());
            case "getActiveWorkers":
                return new ArrayList<BukkitWorker>();
            case "getMainThreadExecutor": {
                Plugin pl = (Plugin) a[0];
                return (Executor) r -> schedule(pl, r, true, 0, 0);
            }
            default:
                return UNHANDLED;
        }
    }

    @Override
    public String toString() {
        return "RusticScheduler";
    }
}
