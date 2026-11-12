// SPDX-License-Identifier: GPL-3.0-only
package rustic.bridge;

import java.lang.reflect.Method;
import org.bukkit.command.ConsoleCommandSender;

/** The server console as a {@link ConsoleCommandSender}: always op, every permission. */
final class ConsoleProxy extends ApiProxy {
    private final Bridge bridge;
    final ConsoleCommandSender proxy;

    ConsoleProxy(Bridge bridge) {
        super(ConsoleCommandSender.class);
        this.bridge = bridge;
        this.proxy = create(ConsoleCommandSender.class);
    }

    @Override
    protected Object handle(Object p, Method m, String n, Object[] a) {
        switch (n) {
            case "hasPermission":
            case "isOp":
            case "isPermissionSet":
                return true;
            case "setOp":
            case "recalculatePermissions":
                return null;
            case "getName":
                return "CONSOLE";
            case "name":
                return net.kyori.adventure.text.Component.text("CONSOLE");
            case "isConversing":
                return false;
            case "spigot":
                return new org.bukkit.command.CommandSender.Spigot() {};
            default:
                break;
        }
        Object r = Senders.common(bridge, "console", n, a, m);
        return r;
    }

    @Override
    public String toString() {
        return "RusticConsole";
    }
}
