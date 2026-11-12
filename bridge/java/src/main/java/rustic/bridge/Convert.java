// SPDX-License-Identifier: GPL-3.0-only
package rustic.bridge;

import java.lang.reflect.Method;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.UUID;
import java.util.concurrent.ConcurrentHashMap;
import org.bukkit.GameMode;
import org.bukkit.Location;
import org.bukkit.Material;
import org.bukkit.NamespacedKey;
import org.bukkit.World;
import org.bukkit.block.Block;
import org.bukkit.block.BlockFace;
import org.bukkit.entity.Player;

/** Rustic records (JSON maps) to Bukkit values and back; worlds and blocks as host-backed proxies. */
final class Convert {
    private Convert() {}

    static GameMode gameMode(String s) {
        return s == null ? GameMode.SURVIVAL : GameMode.valueOf(s.toUpperCase(Locale.ROOT));
    }

    static String gameMode(GameMode g) {
        return g.name().toLowerCase(Locale.ROOT);
    }

    /** Bukkit world names for the three vanilla dimensions (what plugins compare against). */
    static String worldName(String id) {
        return switch (id) {
            case "minecraft:overworld" -> "world";
            case "minecraft:the_nether" -> "world_nether";
            case "minecraft:the_end" -> "world_the_end";
            default -> id;
        };
    }

    static String worldId(String name) {
        return switch (name) {
            case "world" -> "minecraft:overworld";
            case "world_nether" -> "minecraft:the_nether";
            case "world_the_end" -> "minecraft:the_end";
            default -> name.contains(":") ? name : "minecraft:" + name;
        };
    }

    private static final Map<String, World> WORLDS = new ConcurrentHashMap<>();

    static World world(Bridge bridge, String id) {
        return WORLDS.computeIfAbsent(id, k -> new WorldProxy(bridge, k).create(World.class));
    }

    static String worldIdOf(World w) {
        if (w == null) return "minecraft:overworld";
        return w.getKey().toString();
    }

    static Location location(Bridge bridge, Object json) {
        Map<String, Object> m = Json.map(json);
        return new Location(world(bridge, Json.str(m.get("world"))),
                Json.dbl(m.get("x")), Json.dbl(m.get("y")), Json.dbl(m.get("z")),
                (float) Json.dbl(m.get("yaw")), (float) Json.dbl(m.get("pitch")));
    }

    static Map<String, Object> location(Location l) {
        return Json.obj("world", worldIdOf(l.getWorld()),
                "x", l.getX(), "y", l.getY(), "z", l.getZ(),
                "yaw", (double) l.getYaw(), "pitch", (double) l.getPitch());
    }

    static Map<String, Object> pos(int x, int y, int z) {
        return Json.obj("x", x, "y", y, "z", z);
    }

    static Material material(String id) {
        if (id == null) return Material.AIR;
        Material m = Material.matchMaterial(id);
        return m == null ? Material.AIR : m;
    }

    /** A world (dimension) by its Rustic id. */
    static final class WorldProxy extends ApiProxy {
        final Bridge bridge;
        final String id;

        WorldProxy(Bridge bridge, String id) {
            super(World.class);
            this.bridge = bridge;
            this.id = id;
        }

        Object get(String what, Object... kv) {
            Map<String, Object> body = Json.obj(kv);
            body.put("world", id);
            body.put("get", what);
            return bridge.host.call("world", body);
        }

        @Override
        protected Object handle(Object proxy, Method m, String n, Object[] a) {
            switch (n) {
                case "getName":
                    return worldName(id);
                case "getKey":
                case "key":
                    return NamespacedKey.fromString(id);
                case "getUID":
                    return UUID.nameUUIDFromBytes(id.getBytes(StandardCharsets.UTF_8));
                case "getEnvironment":
                    return id.endsWith("the_nether") ? World.Environment.NETHER
                            : id.endsWith("the_end") ? World.Environment.THE_END : World.Environment.NORMAL;
                case "getMinHeight":
                    return id.endsWith("overworld") ? -64 : 0;
                case "getMaxHeight":
                    return id.endsWith("overworld") ? 320 : 256;
                case "getPlayers": {
                    List<Player> out = new ArrayList<>();
                    for (Object u : Json.list(get("players"))) {
                        PlayerProxy p = bridge.online.get(Json.uuid(u));
                        if (p != null) out.add(p.proxy);
                    }
                    return out;
                }
                case "getSpawnLocation":
                    return location(bridge, get("spawn"));
                case "getTime":
                    return Json.lng(get("time")) % 24000;
                case "getFullTime":
                    return Json.lng(get("time"));
                case "setTime":
                case "setFullTime":
                    bridge.host.post("world_set", Json.obj("world", id, "set", "time", "value", a[0]));
                    return null;
                case "getHighestBlockYAt":
                    if (a.length >= 2 && a[0] instanceof Integer x) {
                        return (int) Json.lng(get("highest_y", "x", x, "z", a[1]));
                    }
                    if (a.length >= 1 && a[0] instanceof Location l) {
                        return (int) Json.lng(get("highest_y", "x", l.getBlockX(), "z", l.getBlockZ()));
                    }
                    return UNHANDLED;
                case "save":
                    return null; // the server saves on its own schedule
                case "getBlockAt":
                    if (a.length == 3) return block(bridge, id, (Integer) a[0], (Integer) a[1], (Integer) a[2], null);
                    if (a.length == 1 && a[0] instanceof Location l)
                        return block(bridge, id, l.getBlockX(), l.getBlockY(), l.getBlockZ(), null);
                    return UNHANDLED;
                case "equals":
                    return a[0] instanceof World w && worldIdOf(w).equals(id);
                case "hashCode":
                    return id.hashCode();
                case "toString":
                    return "RusticWorld{name=" + worldName(id) + "}";
                default:
                    return UNHANDLED;
            }
        }
    }

    static Block block(Bridge bridge, String world, int x, int y, int z, String knownId) {
        return new BlockProxy(bridge, world, x, y, z, knownId).create(Block.class);
    }

    /** A block position; its type is read from the host when asked (or known from the event). */
    static final class BlockProxy extends ApiProxy {
        final Bridge bridge;
        final String world;
        final int x, y, z;
        String id;

        BlockProxy(Bridge bridge, String world, int x, int y, int z, String id) {
            super(Block.class);
            this.bridge = bridge;
            this.world = world;
            this.x = x;
            this.y = y;
            this.z = z;
            this.id = id;
        }

        String id() {
            if (id == null) {
                Map<String, Object> st = Json.map(bridge.host.call("block",
                        Json.obj("world", world, "pos", pos(x, y, z))));
                id = Json.str(st.get("id"));
            }
            return id;
        }

        @Override
        protected Object handle(Object proxy, Method m, String n, Object[] a) {
            switch (n) {
                case "getX":
                    return x;
                case "getY":
                    return y;
                case "getZ":
                    return z;
                case "getWorld":
                    return world(bridge, world);
                case "getLocation":
                    if (a.length == 1 && a[0] instanceof Location into) {
                        into.setWorld(world(bridge, world));
                        into.set(x, y, z);
                        return into;
                    }
                    return new Location(world(bridge, world), x, y, z);
                case "getType":
                    return material(id());
                case "isEmpty":
                    return material(id()).isAir();
                case "isLiquid":
                    return id().endsWith("water") || id().endsWith("lava");
                case "setType": {
                    Material mat = (Material) a[0];
                    String nid = mat.getKey().toString();
                    bridge.host.post("set_block", Json.obj("world", world, "pos", pos(x, y, z), "id", nid,
                            "physics", a.length < 2 || (Boolean) a[1]));
                    id = nid;
                    return null;
                }
                case "getRelative":
                    if (a.length == 1 && a[0] instanceof BlockFace f)
                        return block(bridge, world, x + f.getModX(), y + f.getModY(), z + f.getModZ(), null);
                    if (a.length == 2 && a[0] instanceof BlockFace f) {
                        int d = (Integer) a[1];
                        return block(bridge, world, x + f.getModX() * d, y + f.getModY() * d, z + f.getModZ() * d, null);
                    }
                    if (a.length == 3)
                        return block(bridge, world, x + (Integer) a[0], y + (Integer) a[1], z + (Integer) a[2], null);
                    return UNHANDLED;
                case "equals":
                    return a[0] instanceof Block b && b.getX() == x && b.getY() == y && b.getZ() == z
                            && worldIdOf(b.getWorld()).equals(world);
                case "hashCode":
                    return ((x * 31 + y) * 31 + z) * 31 + world.hashCode();
                case "toString":
                    return "RusticBlock{" + world + " " + x + "," + y + "," + z + "}";
                default:
                    return UNHANDLED;
            }
        }
    }
}
