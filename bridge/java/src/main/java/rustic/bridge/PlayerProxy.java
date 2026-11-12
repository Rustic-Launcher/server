// SPDX-License-Identifier: GPL-3.0-only
package rustic.bridge;

import java.lang.reflect.Method;
import java.net.InetSocketAddress;
import java.util.Locale;
import java.util.Map;
import java.util.Set;
import java.util.UUID;
import net.kyori.adventure.text.Component;
import net.kyori.adventure.text.ComponentLike;
import net.kyori.adventure.text.minimessage.MiniMessage;
import net.kyori.adventure.title.Title;
import org.bukkit.GameMode;
import org.bukkit.Location;
import org.bukkit.OfflinePlayer;
import org.bukkit.Sound;
import org.bukkit.entity.EntityType;
import org.bukkit.entity.Player;
import org.bukkit.permissions.Permission;

/**
 * A {@link Player} backed by the host (players.* of rustic:api). Identity (UUID, name) and the
 * flight flags the host cannot read back are kept here; everything else is a host call.
 */
final class PlayerProxy extends ApiProxy {
    final Bridge bridge;
    final UUID id;
    volatile String name;
    volatile String displayName;
    volatile boolean allowFlight;
    volatile boolean flying;
    final Player proxy;

    PlayerProxy(Bridge bridge, UUID id, String name) {
        super(Player.class);
        this.bridge = bridge;
        this.id = id;
        this.name = name;
        this.proxy = create(Player.class);
    }

    Map<String, Object> sender() {
        return Json.obj("player", Json.uuid(id));
    }

    Object get(String what) {
        return bridge.host.call("player", Json.obj("uuid", Json.uuid(id), "get", what));
    }

    void set(String what, Object value) {
        bridge.host.post("player_set", Json.obj("uuid", Json.uuid(id), "set", what, "value", value));
    }

    boolean isOnline() {
        return bridge.online.containsKey(id);
    }

    @Override
    protected Object handle(Object proxy, Method m, String n, Object[] a) throws Throwable {
        Object r = Senders.common(bridge, sender(), n, a, m);
        if (r != UNHANDLED) return r;
        switch (n) {
            case "getName":
                return name;
            case "getUniqueId":
                return id;
            case "getDisplayName":
                return displayName != null ? displayName : name;
            case "setDisplayName":
                displayName = (String) a[0];
                return null;
            case "name":
                return Component.text(name);
            case "displayName":
                if (a.length == 0) return Component.text(displayName != null ? displayName : name);
                displayName = TextUtil.toLegacy((Component) a[0]);
                return null;
            case "getPlayerListName":
                return displayName != null ? displayName : name;
            case "playerListName":
                return a.length == 0 ? Component.text(name) : null;
            case "isOnline":
            case "isConnected":
            case "isValid":
                return isOnline();
            case "getPlayer":
                return isOnline() ? proxy : null;
            case "hasPlayedBefore":
                return true;
            case "getFirstPlayed":
            case "getLastPlayed":
            case "getLastLogin":
            case "getLastSeen":
                return 0L;
            case "isBanned":
            case "isWhitelisted":
            case "isSleeping":
            case "isConversing":
            case "isDead":
            case "isSneaking":
            case "isSprinting":
                return false;
            case "getType":
                return EntityType.PLAYER;
            case "isOp":
                return Json.bool(get("op"));
            case "setOp":
                set("op", a[0]);
                return null;
            case "getHealth":
                return Json.dbl(get("health"));
            case "setHealth":
                set("health", ((Number) a[0]).doubleValue());
                return null;
            case "getMaxHealth":
                return Json.dbl(get("max_health"));
            case "getFoodLevel":
                return (int) Json.lng(get("food"));
            case "setFoodLevel":
                set("food", ((Number) a[0]).intValue());
                return null;
            case "getSaturation":
                return (float) Json.dbl(get("saturation"));
            case "setSaturation":
                set("saturation", ((Number) a[0]).doubleValue());
                return null;
            case "getLevel":
                return (int) Json.lng(get("level"));
            case "setLevel":
                set("level", ((Number) a[0]).intValue());
                return null;
            case "getAllowFlight":
                return allowFlight;
            case "setAllowFlight":
                allowFlight = (Boolean) a[0];
                if (!allowFlight) flying = false;
                bridge.host.post("player_set", Json.obj("uuid", Json.uuid(id), "set", "flying",
                        "value", allowFlight, "flying", flying));
                return null;
            case "isFlying":
                return flying;
            case "setFlying":
                flying = (Boolean) a[0];
                if (flying && !allowFlight)
                    throw new IllegalArgumentException("Cannot make player fly if getAllowFlight() is false");
                bridge.host.post("player_set", Json.obj("uuid", Json.uuid(id), "set", "flying",
                        "value", allowFlight, "flying", flying));
                return null;
            case "getGameMode":
                return Convert.gameMode(Json.str(get("game_mode")));
            case "setGameMode":
                set("game_mode", ((GameMode) a[0]).name().toLowerCase(Locale.ROOT));
                return null;
            case "getLocation":
                if (a.length == 1 && a[0] instanceof Location into) {
                    Location l = Convert.location(bridge, get("location"));
                    into.setWorld(l.getWorld());
                    into.set(l.getX(), l.getY(), l.getZ());
                    into.setYaw(l.getYaw());
                    into.setPitch(l.getPitch());
                    return into;
                }
                return Convert.location(bridge, get("location"));
            case "getEyeLocation":
                return Convert.location(bridge, get("location")).add(0, 1.62, 0);
            case "getWorld":
                return Convert.location(bridge, get("location")).getWorld();
            case "teleport":
                if (a.length >= 1 && a[0] instanceof Location to) {
                    bridge.host.post("teleport", Json.obj("uuid", Json.uuid(id), "to", Convert.location(to)));
                    return true;
                }
                return UNHANDLED;
            case "teleportAsync":
                if (a.length >= 1 && a[0] instanceof Location to) {
                    bridge.host.post("teleport", Json.obj("uuid", Json.uuid(id), "to", Convert.location(to)));
                    return java.util.concurrent.CompletableFuture.completedFuture(true);
                }
                return UNHANDLED;
            case "kickPlayer":
                bridge.host.post("kick", Json.obj("uuid", Json.uuid(id), "text", TextUtil.legacyToJson((String) a[0])));
                return null;
            case "kick":
                bridge.host.post("kick", Json.obj("uuid", Json.uuid(id),
                        "text", a.length > 0 && a[0] instanceof Component c ? TextUtil.toJson(c) : "\"Kicked\""));
                return null;
            case "getAddress": {
                String addr = Json.str(get("address"));
                int colon = addr == null ? -1 : addr.lastIndexOf(':');
                String host = colon > 0 ? addr.substring(0, colon) : String.valueOf(addr);
                int port = 0;
                if (colon > 0) {
                    try {
                        port = Integer.parseInt(addr.substring(colon + 1));
                    } catch (NumberFormatException ignored) {
                        // no port
                    }
                }
                return InetSocketAddress.createUnresolved(host, port);
            }
            case "getPing":
                return (int) Json.lng(get("ping"));
            case "getLocale":
                return Json.str(get("locale"));
            case "locale":
                return Locale.forLanguageTag(String.valueOf(get("locale")).replace('_', '-'));
            case "sendActionBar":
                if (a.length == 1) {
                    String json = a[0] instanceof ComponentLike c ? TextUtil.toJson(c.asComponent())
                            : TextUtil.legacyToJson(String.valueOf(a[0]));
                    bridge.host.post("action_bar", Json.obj("uuid", Json.uuid(id), "text", json));
                    return null;
                }
                return UNHANDLED;
            case "sendTitle":
                if (a.length >= 2) {
                    int fi = a.length >= 5 ? (Integer) a[2] : 10;
                    int st = a.length >= 5 ? (Integer) a[3] : 70;
                    int fo = a.length >= 5 ? (Integer) a[4] : 20;
                    bridge.host.post("title", Json.obj("uuid", Json.uuid(id),
                            "title", TextUtil.legacyToJson(a[0] == null ? "" : (String) a[0]),
                            "subtitle", TextUtil.legacyToJson(a[1] == null ? "" : (String) a[1]),
                            "fade_in", fi, "stay", st, "fade_out", fo));
                    return null;
                }
                return UNHANDLED;
            case "showTitle":
                if (a.length == 1 && a[0] instanceof Title t) {
                    Title.Times times = t.times();
                    bridge.host.post("title", Json.obj("uuid", Json.uuid(id),
                            "title", TextUtil.toJson(t.title()), "subtitle", TextUtil.toJson(t.subtitle()),
                            "fade_in", times == null ? 10 : (int) (times.fadeIn().toMillis() / 50),
                            "stay", times == null ? 70 : (int) (times.stay().toMillis() / 50),
                            "fade_out", times == null ? 20 : (int) (times.fadeOut().toMillis() / 50)));
                    return null;
                }
                return UNHANDLED;
            case "playSound":
                if (a.length >= 4 && a[0] instanceof Location) {
                    String sound = a[1] instanceof Sound s ? s.key().asString() : String.valueOf(a[1]);
                    float vol = ((Number) a[a.length - 2]).floatValue();
                    float pitch = ((Number) a[a.length - 1]).floatValue();
                    bridge.host.post("sound", Json.obj("uuid", Json.uuid(id), "sound", sound,
                            "volume", (double) vol, "pitch", (double) pitch));
                    return null;
                }
                return UNHANDLED;
            case "performCommand":
                return Json.bool(bridge.host.call("dispatch_command", Json.obj("sender", sender(), "line", a[0])));
            case "chat":
                return UNHANDLED;
            case "spigot":
                return new Player.Spigot() {};
            case "recalculatePermissions":
            case "updateCommands":
            case "saveData":
            case "loadData":
                return null;
            case "isPermissionSet":
                return a[0] instanceof Permission || a[0] instanceof String;
            case "getListeningPluginChannels":
                return Set.of();
            case "equals":
                return a[0] instanceof OfflinePlayer o && o.getUniqueId().equals(id);
            case "hashCode":
                return id.hashCode();
            case "toString":
                return "RusticPlayer{name=" + name + "}";
            default:
                return UNHANDLED;
        }
    }

    @Override
    public String toString() {
        return "RusticPlayer{name=" + name + "}";
    }

    /** MiniMessage text (Paper's {@code sendRichMessage}). */
    static Component mini(String s) {
        return MiniMessage.miniMessage().deserialize(s);
    }
}
