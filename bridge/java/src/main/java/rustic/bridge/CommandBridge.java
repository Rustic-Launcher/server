// SPDX-License-Identifier: GPL-3.0-only
package rustic.bridge;

import java.util.List;
import java.util.Map;
import org.bukkit.command.CommandSender;
import org.bukkit.command.PluginCommand;

/**
 * plugin.yml commands: the host registered them in the Rustic command tree (a bare usage and a
 * greedy {@code args} usage); a run arrives here as (label, raw argument string) and goes to the
 * plugin's {@link PluginCommand} (its executor, usage message and tab completer), as on Bukkit.
 */
final class CommandBridge {
    private CommandBridge() {}

    static CommandSender sender(Bridge b, Object json) {
        if (json instanceof Map<?, ?> m && m.containsKey("player")) {
            return b.player(Json.uuid(m.get("player")), null).proxy;
        }
        return b.console.proxy;
    }

    static String[] args(Object raw) {
        String s = raw == null ? "" : raw.toString();
        return s.isEmpty() ? new String[0] : s.split(" ", -1);
    }

    private static PluginCommand command(Bridge b, Map<String, Object> req) {
        LoadedPlugin lp = b.need(Json.str(req.get("plugin")));
        PluginCommand c = lp.commands.get(Json.str(req.get("command")));
        if (c == null) throw new IllegalArgumentException("no command " + req.get("command"));
        return c;
    }

    static Object execute(Bridge b, Map<String, Object> req) {
        PluginCommand c = command(b, req);
        CommandSender s = sender(b, req.get("sender"));
        String label = Json.str(req.get("label"));
        String saved = b.pluginManager.current;
        b.pluginManager.current = c.getPlugin().getName();
        try {
            c.execute(s, label, args(req.get("args")));
        } catch (Throwable t) {
            BridgeLog.error(c.getPlugin().getName(), "Unhandled exception executing command '" + label
                    + "' in plugin " + c.getPlugin().getPluginMeta().getDisplayName(), t);
            s.sendMessage("§cAn internal error occurred while attempting to perform this command");
        } finally {
            b.pluginManager.current = saved;
        }
        return Json.obj("ok", true);
    }

    static Object complete(Bridge b, Map<String, Object> req) {
        PluginCommand c = command(b, req);
        try {
            List<String> out = c.tabComplete(sender(b, req.get("sender")), Json.str(req.get("label")),
                    args(req.get("args")));
            return out == null ? List.of() : out;
        } catch (Throwable t) {
            BridgeLog.error(c.getPlugin().getName(), "tab completion failed", t);
            return List.of();
        }
    }
}
