// SPDX-License-Identifier: GPL-3.0-only
package rustic.bridge;

import java.io.File;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import java.util.function.Predicate;

/**
 * Unit tests of the Java layer, without the Rust host: a {@link FakeHost} transport answers the
 * bridge's host calls and records them, and the tests drive {@link Host#handle} with the requests
 * the Rust side sends (load the real test-plugin jars, enable, events, commands, tick pulses).
 * Run by build.sh / build.ps1 (no test framework needed).
 */
public final class TestMain {
    private static int passed;
    private static final List<String> failures = new ArrayList<>();

    public static void main(String[] args) throws Exception {
        run("json round trip and u64 uuids", TestMain::json);
        run("legacy colours to component JSON", TestMain::text);
        run("plugins through the bridge", TestMain::plugins);
        System.out.println("tests: " + passed + " passed, " + failures.size() + " failed");
        for (String f : failures) System.out.println("FAILED " + f);
        Runtime.getRuntime().halt(failures.isEmpty() ? 0 : 1);
    }

    interface Body {
        void run() throws Exception;
    }

    static void run(String name, Body b) {
        try {
            b.run();
            passed++;
            System.out.println("ok   " + name);
        } catch (Throwable t) {
            failures.add(name + ": " + t);
            System.out.println("FAIL " + name);
            t.printStackTrace(System.out);
        }
    }

    static void check(boolean cond, String what) {
        if (!cond) throw new AssertionError(what);
    }

    // ------------------------------------------------------------------ json / text

    static void json() {
        String src = "{\"a\":[1,2.5,\"x\\\"y\\n\",true,null],\"u\":{\"hi\":18446744073709551615,\"lo\":1}}";
        Map<String, Object> m = Json.map(Json.parse(src));
        List<Object> a = Json.list(m.get("a"));
        check(a.get(0).equals(1L) && a.get(1).equals(2.5) && a.get(2).equals("x\"y\n"), "values " + a);
        UUID u = Json.uuid(m.get("u"));
        check(u.getMostSignificantBits() == -1L && u.getLeastSignificantBits() == 1L, "uuid " + u);
        check(Json.write(Json.uuid(u)).equals("{\"hi\":18446744073709551615,\"lo\":1}"), "uuid back");
        check(Json.write(Json.obj("x", 3.0, "y", 7L)).equals("{\"x\":3.0,\"y\":7}"), "numbers");
        check(Json.write(m.get("u")).equals("{\"hi\":18446744073709551615,\"lo\":1}"), "u64 kept as read");
    }

    static void text() {
        String j = TextUtil.legacyToJson("§6Gold §lbold");
        check(j.contains("\"gold\"") && j.contains("Gold"), j);
        check(TextUtil.legacyToJson("plain").equals("\"plain\""), "plain");
        check(TextUtil.jsonToLegacy("{\"text\":\"hi\",\"color\":\"red\"}").equals("§chi"), "back");
        // The server's own join line: a modern show_entity hover event.
        String join = "{\"translate\":\"multiplayer.player.joined\",\"color\":\"yellow\",\"with\":[{\"text\":\"Bot\","
                + "\"hover_event\":{\"action\":\"show_entity\",\"id\":\"minecraft:player\",\"uuid\":[1,2,3,4],"
                + "\"name\":\"Bot\"},\"click_event\":{\"action\":\"suggest_command\",\"command\":\"/tell Bot \"}}]}";
        check(TextUtil.fromJson(join) != null, "modern component read");
    }

    // ------------------------------------------------------------------ the bridge, end to end in the JVM

    /** Answers host calls like the Rust side would, and records them. */
    static final class FakeHost implements Transport {
        final List<Map<String, Object>> calls = new ArrayList<>();
        Host host;

        @Override
        public synchronized byte[] call(byte[] request) {
            Map<String, Object> req = Json.map(Json.parse(new String(request, StandardCharsets.UTF_8)));
            calls.add(req);
            Object result = switch (Json.str(req.get("op"))) {
                case "subscribe" -> (long) calls.size();
                case "has_permission" -> true;
                case "dispatch_command" -> true;
                case "player" -> switch (Json.str(req.get("get"))) {
                    case "health", "max_health" -> 20.0;
                    case "op" -> true;
                    case "food" -> 20L;
                    case "game_mode" -> "survival";
                    default -> Json.obj("world", "minecraft:overworld", "x", 1.0, "y", 64.0, "z", 2.0,
                            "yaw", 0.0, "pitch", 0.0);
                };
                case "server" -> List.of("minecraft:overworld");
                default -> null;
            };
            byte[] json = Json.write(result).getBytes(StandardCharsets.UTF_8);
            byte[] r = new byte[json.length + 1];
            r[0] = OK;
            System.arraycopy(json, 0, r, 1, json.length);
            return r;
        }

        synchronized List<Map<String, Object>> find(String op, Predicate<String> text) {
            List<Map<String, Object>> out = new ArrayList<>();
            for (Map<String, Object> c : calls) {
                if (op.equals(c.get("op")) && text.test(Json.write(c))) out.add(c);
            }
            return out;
        }
    }

    static Object send(Host h, Object... kv) {
        byte[] r = h.handle(Json.write(Json.obj(kv)).getBytes(StandardCharsets.UTF_8));
        String json = new String(r, 1, r.length - 1, StandardCharsets.UTF_8);
        if (r[0] != Transport.OK) throw new AssertionError("host request failed: " + json);
        return Json.parse(json);
    }

    static final UUID STEVE = UUID.fromString("069a79f4-44e9-4726-a5be-fca90e38aaf5");

    static Map<String, Object> steve() {
        return Json.uuid(STEVE);
    }

    static void plugins() throws Exception {
        File dir = new File(System.getProperty("rustic.test.plugins", "build/plugins"));
        File data = java.nio.file.Files.createTempDirectory("rustic-bridge-test").toFile();
        FakeHost fake = new FakeHost();
        Host host = new Host();
        Bridge bridge = new Bridge(host);
        host.attach(fake, bridge, Thread.currentThread());

        send(host, "op", "hello", "server", Json.obj("version", "test", "max_players", 20L), "players", List.of());
        List<Object> scan = Json.list(send(host, "op", "scan", "dir", dir.getAbsolutePath()));
        check(scan.size() == 3, "3 plugins scanned: " + scan);
        for (Object o : scan) {
            Map<String, Object> d = Json.map(o);
            check(!d.containsKey("error"), "descriptor " + d);
            String name = Json.str(d.get("name"));
            send(host, "op", "load", "plugin", name, "file", d.get("file"),
                    "data_dir", new File(data, name).getAbsolutePath());
            Map<String, Object> en = Json.map(send(host, "op", "enable", "plugin", name));
            check(Json.bool(en.get("enabled")), name + " enabled: " + en);
        }
        check(new File(data, "Welcome/config.yml").isFile(), "saveDefaultConfig wrote config.yml");
        check(!fake.find("subscribe", s -> s.contains("player-join") && s.contains("Welcome")).isEmpty(),
                "Welcome subscribed to player-join");
        check(!fake.find("subscribe", s -> s.contains("player-quit") && s.contains("monitor")).isEmpty(),
                "quit at MONITOR");

        // Class isolation: each plugin sees its own bundled rustictest.shared.Lib.
        Object wl = bridge.plugins.get("Welcome").loader.loadClass("rustictest.shared.Lib").getField("VERSION").get(null);
        Object cl = bridge.plugins.get("RusticCommands").loader.loadClass("rustictest.shared.Lib").getField("VERSION").get(null);
        check(wl.equals("welcome-copy") && cl.equals("commands-copy"), "isolated libs " + wl + " " + cl);

        // Join: the plugin rewrites the join line, greets privately, schedules a delayed greeting.
        Map<String, Object> join = Json.obj("player-join", Json.obj("player", steve(), "name", "Steve",
                "join-message", "\"Steve joined the game\""));
        send(host, "op", "event", "plugin", "", "priority", "lowest", "event", join);
        Map<String, Object> out = Json.map(send(host, "op", "event", "plugin", "Welcome", "priority", "normal",
                "event", join));
        String line = Json.write(out);
        check(line.contains("Welcome, ") && line.contains("Steve"), "join message rewritten: " + line);
        check(!fake.find("send_message", s -> s.contains("visitor #1")).isEmpty(), "private greeting");
        for (int t = 1; t <= 41; t++) send(host, "op", "tick", "tick", (long) t);
        check(!fake.find("send_message", s -> s.contains("Enjoy your stay")).isEmpty(), "delayed greeting");

        // Chat: the blocked word cancels.
        Map<String, Object> chat = Json.obj("player-chat", Json.obj("player", steve(), "name", "Steve",
                "message", "this is a badword", "cancelled", false, "display", null));
        out = Json.map(send(host, "op", "event", "plugin", "Welcome", "priority", "normal", "event", chat));
        check(Json.bool(Json.map(Json.map(out.get("event")).get("player-chat")).get("cancelled")), "chat cancelled");

        // Commands.
        Map<String, Object> asSteve = Json.obj("player", steve());
        send(host, "op", "command", "plugin", "Welcome", "command", "welcome", "label", "welcome",
                "args", "reload", "sender", asSteve);
        check(!fake.find("send_message", s -> s.contains("reloaded")).isEmpty(), "/welcome reload");
        send(host, "op", "command", "plugin", "RusticCommands", "command", "heal", "label", "heal",
                "args", "", "sender", asSteve);
        check(!fake.find("player_set", s -> s.contains("health") && s.contains("20.0")).isEmpty(), "/heal set health");
        send(host, "op", "command", "plugin", "RusticCommands", "command", "fly", "label", "fly",
                "args", "", "sender", asSteve);
        check(!fake.find("player_set", s -> s.contains("flying") && s.contains("true")).isEmpty(), "/fly");
        send(host, "op", "command", "plugin", "RusticCommands", "command", "broadcast", "label", "bc",
                "args", "hello world", "sender", "console");
        long t = 42;
        for (int i = 0; i < 200 && fake.find("broadcast", s -> s.contains("hello world")).isEmpty(); i++) {
            Thread.sleep(5);
            send(host, "op", "tick", "tick", t++);
        }
        check(!fake.find("broadcast", s -> s.contains("hello world")).isEmpty(), "/broadcast after countdown");

        // Faults stay inside the plugin call.
        send(host, "op", "command", "plugin", "Probe", "command", "probe", "label", "probe", "args", "exit",
                "sender", "console");
        check(!fake.find("send_message", s -> s.contains("internal error")).isEmpty(), "System.exit refused");
        send(host, "op", "command", "plugin", "Probe", "command", "probe", "label", "probe", "args", "throw",
                "sender", "console");
        send(host, "op", "command", "plugin", "Probe", "command", "probe", "label", "probe", "args", "sqlite",
                "sender", "console");
        check(!fake.find("send_message", s -> s.contains("row(s)")).isEmpty(), "sqlite through a shaded driver");
        // An API call from a plugin thread is run by the main thread at the next pulse.
        send(host, "op", "command", "plugin", "Probe", "command", "probe", "label", "probe", "args", "async",
                "sender", asSteve);
        for (int i = 0; i < 200 && fake.find("send_message", s -> s.contains("async thread")).isEmpty(); i++) {
            Thread.sleep(5);
            send(host, "op", "tick", "tick", t++);
        }
        check(!fake.find("send_message", s -> s.contains("async thread")).isEmpty(), "async marshalling");

        // Disable: onDisable saved the config.
        send(host, "op", "disable", "plugin", "Welcome");
        String cfg = java.nio.file.Files.readString(new File(data, "Welcome/config.yml").toPath());
        check(cfg.contains("visitors: 1"), "config saved on disable: " + cfg);
    }
}
