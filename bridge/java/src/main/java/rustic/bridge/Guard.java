// SPDX-License-Identifier: GPL-3.0-only
package rustic.bridge;

/**
 * Where plugin calls to {@code System.exit}, {@code Runtime.exit} and {@code Runtime.halt} land
 * (rewritten by {@link PluginClassLoader}). A plugin may not end the bridge's JVM: the call throws
 * {@link SecurityException} in the plugin, like the SecurityManager did on old Java.
 */
public final class Guard {
    private Guard() {}

    public static void exit(int status) {
        throw refuse("System.exit(" + status + ")");
    }

    public static void runtimeExit(Runtime rt, int status) {
        throw refuse("Runtime.exit(" + status + ")");
    }

    public static void halt(Runtime rt, int status) {
        throw refuse("Runtime.halt(" + status + ")");
    }

    private static SecurityException refuse(String what) {
        SecurityException e = new SecurityException("Rustic Bridge: a plugin called " + what + "; refused");
        BridgeLog.error("Rustic Bridge", "a plugin called " + what + "; refused", e);
        return e;
    }
}
