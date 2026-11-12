// SPDX-License-Identifier: GPL-3.0-only
package rustic.bridge;

import java.lang.reflect.Method;
import java.util.UUID;
import net.kyori.adventure.text.Component;
import net.kyori.adventure.text.ComponentLike;
import org.bukkit.command.CommandSender;
import org.bukkit.command.ConsoleCommandSender;
import org.bukkit.permissions.Permission;

/** What players and the console share as {@link CommandSender}s. */
final class Senders {
    private Senders() {}

    static void send(Bridge bridge, Object sender, String json) {
        bridge.host.post("send_message", Json.obj("to", sender, "text", json));
    }

    static Object common(Bridge bridge, Object sender, String n, Object[] a, Method m) {
        switch (n) {
            case "sendMessage": {
                for (Object o : a) {
                    if (o instanceof String s) send(bridge, sender, TextUtil.legacyToJson(s));
                    else if (o instanceof String[] arr) for (String s : arr) send(bridge, sender, TextUtil.legacyToJson(s));
                    else if (o instanceof ComponentLike c) {
                        send(bridge, sender, TextUtil.toJson(c.asComponent()));
                        break;
                    } else if (o instanceof net.md_5.bungee.api.chat.BaseComponent b) {
                        send(bridge, sender, TextUtil.legacyToJson(b.toLegacyText()));
                    } else if (o instanceof net.md_5.bungee.api.chat.BaseComponent[] bs) {
                        send(bridge, sender, TextUtil.legacyToJson(
                                net.md_5.bungee.api.chat.BaseComponent.toLegacyText(bs)));
                    } else if (o == null || o instanceof UUID) {
                        continue;
                    } else {
                        return ApiProxy.UNHANDLED;
                    }
                }
                return null;
            }
            case "sendRichMessage":
                send(bridge, sender, TextUtil.toJson(PlayerProxy.mini((String) a[0])));
                return null;
            case "sendPlainMessage":
                send(bridge, sender, TextUtil.toJson(Component.text((String) a[0])));
                return null;
            case "hasPermission": {
                String node = a[0] instanceof Permission p ? p.getName() : (String) a[0];
                return Json.bool(bridge.host.call("has_permission", Json.obj("sender", sender, "node", node)));
            }
            case "getServer":
                return bridge.server;
            default:
                return ApiProxy.UNHANDLED;
        }
    }

    /** The host's `command-sender` for a Bukkit sender. */
    static Object of(CommandSender s) {
        if (s instanceof ConsoleCommandSender) return "console";
        if (s instanceof org.bukkit.entity.Player p) return Json.obj("player", Json.uuid(p.getUniqueId()));
        return "console";
    }
}
