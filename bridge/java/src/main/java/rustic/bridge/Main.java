// SPDX-License-Identifier: GPL-3.0-only
package rustic.bridge;

import java.io.IOException;
import java.io.OutputStream;
import java.net.InetAddress;
import java.net.Socket;
import java.nio.charset.StandardCharsets;

/**
 * The sidecar's entry point: {@code java -cp rustic-bridge.jar:libs/* rustic.bridge.Main --port N}
 * with the connection token in the {@code RUSTIC_BRIDGE_TOKEN} environment variable. Connects to
 * the Rust host on loopback, introduces itself with the token, then serves the host's requests on
 * this (the "main") thread until the host says {@code shutdown} or the connection drops.
 */
public final class Main {
    private Main() {}

    /** A host request running longer than this gets its stack logged (once). */
    static final long WATCHDOG_MS = Long.getLong("rustic.bridge.watchdogMs", 3_000);

    public static void main(String[] args) throws Exception {
        int port = -1;
        for (int i = 0; i < args.length; i++) {
            if (args[i].equals("--port") && i + 1 < args.length) port = Integer.parseInt(args[++i]);
        }
        if (port <= 0) {
            System.err.println("usage: rustic.bridge.Main --port <port>   (token in RUSTIC_BRIDGE_TOKEN)");
            System.exit(2);
        }
        String token = System.getenv().getOrDefault("RUSTIC_BRIDGE_TOKEN", "");
        BridgeLog.install();
        Thread.setDefaultUncaughtExceptionHandler((t, e) ->
                BridgeLog.error("Rustic Bridge", "uncaught exception in thread " + t.getName(), e));

        Socket socket = new Socket(InetAddress.getLoopbackAddress(), port);
        OutputStream raw = socket.getOutputStream();
        raw.write(("RUSTIC-BRIDGE/1 " + token + "\n").getBytes(StandardCharsets.US_ASCII));
        raw.flush();

        Host host = new Host();
        Bridge bridge = new Bridge(host);
        Transport.Socket_ transport = new Transport.Socket_(socket, host);
        Thread main = Thread.currentThread();
        main.setName("Rustic Bridge Main");
        host.attach(transport, bridge, main);
        startWatchdog(host, main);

        int code = 0;
        try {
            transport.serve();
            BridgeLog.info("Rustic Bridge", "the server asked the bridge to stop");
        } catch (IOException e) {
            BridgeLog.warn("Rustic Bridge", "connection to the server lost (" + e.getMessage() + "); stopping");
            code = 1;
        } finally {
            bridge.scheduler.shutdown();
            transport.close();
        }
        // halt, not exit: a plugin's shutdown hook or a non-daemon thread must not keep the JVM up.
        Runtime.getRuntime().halt(code);
    }

    /** Logs the main thread's stack when one host request runs too long (a stuck plugin). */
    static void startWatchdog(Host host, Thread main) {
        Thread w = new Thread(() -> {
            long reported = 0;
            while (true) {
                try {
                    Thread.sleep(500);
                } catch (InterruptedException e) {
                    return;
                }
                long since = host.busySince;
                if (since == 0 || since == reported) continue;
                long ms = (System.nanoTime() - since) / 1_000_000;
                if (ms < WATCHDOG_MS) continue;
                reported = since;
                StringBuilder b = new StringBuilder("the main thread has been busy for " + ms + " ms in `"
                        + host.busyWhat + "`; a plugin may be stuck. Stack:");
                for (StackTraceElement el : main.getStackTrace()) b.append("\n    at ").append(el);
                BridgeLog.line("WARN", "Rustic Bridge Watchdog", b.toString());
            }
        }, "Rustic Bridge Watchdog");
        w.setDaemon(true);
        w.start();
    }
}
