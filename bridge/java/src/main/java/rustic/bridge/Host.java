// SPDX-License-Identifier: GPL-3.0-only
package rustic.bridge;

import java.io.IOException;
import java.io.UncheckedIOException;
import java.nio.charset.StandardCharsets;
import java.util.Map;
import java.util.concurrent.ConcurrentLinkedQueue;
import java.util.concurrent.ExecutionException;
import java.util.concurrent.FutureTask;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.TimeoutException;

/**
 * The Java end of the host connection: calls into Rust, the main-thread queue, and request
 * dispatch into {@link Bridge}.
 *
 * <p><b>Threading.</b> The "main thread" is the thread that serves host requests (the sidecar's
 * main thread, or the server thread itself with JNI). Only it may talk to the host. A call made
 * from another thread (a plugin's async task, its own thread) is marshalled: it is queued and
 * run by the main thread at the next tick pulse (or right after the current host request), and
 * the calling thread waits for the result. Calls without a result ({@link #post}) do not wait.
 * This is the BukkitScheduler contract: the game is only touched from the server tick.
 */
public final class Host {
    /** How long an off-main-thread call waits for the main thread. */
    static final long OFF_MAIN_TIMEOUT_MS = Long.getLong("rustic.bridge.offMainTimeoutMs", 10_000);

    private Transport transport;
    private volatile Thread mainThread;
    private final ConcurrentLinkedQueue<Runnable> mainQueue = new ConcurrentLinkedQueue<>();
    private Bridge bridge;
    private volatile boolean stop;
    /** Nanos when the current host request started on the main thread (0: idle), for the watchdog. */
    volatile long busySince;
    volatile String busyWhat = "";

    void attach(Transport transport, Bridge bridge, Thread main) {
        this.transport = transport;
        this.bridge = bridge;
        this.mainThread = main;
    }

    public boolean isMainThread() {
        return Thread.currentThread() == mainThread;
    }

    Thread mainThread() {
        return mainThread;
    }

    boolean stopRequested() {
        return stop;
    }

    void requestStop() {
        stop = true;
    }

    // ------------------------------------------------------------------ outgoing

    /** Call the host and return the decoded reply. Any thread (see the class docs). */
    public Object call(String op, Map<String, Object> body) {
        if (isMainThread()) return callNow(op, body);
        FutureTask<Object> task = new FutureTask<>(() -> callNow(op, body));
        mainQueue.add(task);
        try {
            return task.get(OFF_MAIN_TIMEOUT_MS, TimeUnit.MILLISECONDS);
        } catch (InterruptedException e) {
            Thread.currentThread().interrupt();
            throw new IllegalStateException("Rustic Bridge: interrupted while waiting for the server thread", e);
        } catch (TimeoutException e) {
            task.cancel(false);
            throw new IllegalStateException("Rustic Bridge: the server thread did not run `" + op + "` within "
                    + OFF_MAIN_TIMEOUT_MS + " ms");
        } catch (ExecutionException e) {
            Throwable c = e.getCause();
            if (c instanceof RuntimeException r) throw r;
            if (c instanceof Error er) throw er;
            throw new IllegalStateException(c);
        }
    }

    /** Call the host for its side effect only: off the main thread it is queued, not awaited. */
    public void post(String op, Map<String, Object> body) {
        if (isMainThread()) {
            callNow(op, body);
        } else {
            mainQueue.add(() -> {
                try {
                    callNow(op, body);
                } catch (RuntimeException e) {
                    BridgeLog.warn("Rustic Bridge", "queued `" + op + "` failed: " + e.getMessage());
                }
            });
        }
    }

    /** Run something on the main thread (queued if called elsewhere). */
    public void runOnMain(Runnable r) {
        if (isMainThread()) r.run();
        else mainQueue.add(r);
    }

    void enqueue(Runnable r) {
        mainQueue.add(r);
    }

    private Object callNow(String op, Map<String, Object> body) {
        body.put("op", op);
        byte[] reply;
        try {
            reply = transport.call(Json.write(body).getBytes(StandardCharsets.UTF_8));
        } catch (IOException e) {
            throw new UncheckedIOException("Rustic Bridge: lost the server connection", e);
        }
        String json = new String(reply, 1, reply.length - 1, StandardCharsets.UTF_8);
        if (reply[0] == Transport.OK) return json.isEmpty() ? null : Json.parse(json);
        throw new HostException(op, String.valueOf(Json.parse(json)));
    }

    /** The host refused a call. */
    public static final class HostException extends RuntimeException {
        HostException(String op, String message) {
            super("Rustic Bridge: `" + op + "` failed: " + message);
        }
    }

    /** Run the queued off-main-thread work (main thread only). */
    void drainMainQueue() {
        Runnable r;
        int n = 0;
        while (n++ < 10_000 && (r = mainQueue.poll()) != null) {
            try {
                r.run();
            } catch (Throwable t) {
                BridgeLog.error("Rustic Bridge", "a queued main-thread task failed", t);
            }
        }
    }

    // ------------------------------------------------------------------ incoming

    /** Serve one host request. Returns the reply: kind byte + JSON. */
    public byte[] handle(byte[] request) {
        long outer = busySince;
        String outerWhat = busyWhat;
        try {
            Map<String, Object> req = Json.map(Json.parse(new String(request, StandardCharsets.UTF_8)));
            String op = Json.str(req.get("op"));
            if (outer == 0) {
                busySince = System.nanoTime();
                busyWhat = op + describe(req);
            }
            Object result = bridge.handle(op, req);
            if (outer == 0) drainMainQueue();
            return reply(Transport.OK, result);
        } catch (Throwable t) {
            BridgeLog.error("Rustic Bridge", "request failed", t);
            return reply(Transport.ERR, String.valueOf(t));
        } finally {
            busySince = outer;
            busyWhat = outerWhat;
        }
    }

    private static String describe(Map<String, Object> req) {
        Object p = req.get("plugin");
        return p == null ? "" : " (" + p + ")";
    }

    private static byte[] reply(byte kind, Object v) {
        byte[] json = Json.write(v).getBytes(StandardCharsets.UTF_8);
        byte[] r = new byte[json.length + 1];
        r[0] = kind;
        System.arraycopy(json, 0, r, 1, json.length);
        return r;
    }
}
