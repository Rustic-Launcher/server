// SPDX-License-Identifier: GPL-3.0-only
package rustic.bridge;

import java.nio.charset.StandardCharsets;
import java.util.Map;

/**
 * The embedded-JVM spike (docs/BRIDGE.md, execution model A): the same bridge, driven in-process
 * through JNI instead of the socket. {@link #start} is called once on the host thread, which
 * becomes the bridge's main thread; {@link #handle} serves one host request (the bytes the
 * sidecar would get in a frame). Host calls are answered locally (the spike measures the
 * dispatch, the plugins' listeners in the measured events make no host call).
 */
public final class JniEntry {
    private JniEntry() {}

    private static Host host;

    /** Answers the bridge's host calls in-process (subscriptions get ids, the rest is empty). */
    static final class LocalTransport implements Transport {
        private long next = 1;

        @Override
        public byte[] call(byte[] request) {
            Map<String, Object> req = Json.map(Json.parse(new String(request, StandardCharsets.UTF_8)));
            Object out = switch (Json.str(req.get("op"))) {
                case "subscribe" -> next++;
                case "has_permission" -> true;
                default -> null;
            };
            byte[] json = Json.write(out).getBytes(StandardCharsets.UTF_8);
            byte[] r = new byte[json.length + 1];
            r[0] = OK;
            System.arraycopy(json, 0, r, 1, json.length);
            return r;
        }
    }

    public static String start() {
        BridgeLog.install();
        host = new Host();
        Bridge bridge = new Bridge(host);
        host.attach(new LocalTransport(), bridge, Thread.currentThread());
        return "ok";
    }

    public static byte[] handle(byte[] request) {
        return host.handle(request);
    }
}
