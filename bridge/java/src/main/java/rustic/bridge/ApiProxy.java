// SPDX-License-Identifier: GPL-3.0-only
package rustic.bridge;

import java.lang.reflect.InvocationHandler;
import java.lang.reflect.Method;
import java.lang.reflect.Proxy;

/**
 * Base of the bridge's {@link Proxy}-based implementations of Bukkit interfaces ({@code Server},
 * {@code Player}, {@code PluginManager}...). The interfaces have hundreds of methods; a subclass
 * implements the ones Rustic supports in {@link #handle} and returns {@link #UNHANDLED} for the
 * rest. Unhandled calls run the interface's {@code default} body if it has one, and otherwise
 * throw {@code UnsupportedOperationException("Rustic Bridge: <Type>.<method> not supported yet")},
 * so a plugin loads and only fails where it touches what Rustic does not provide.
 */
public abstract class ApiProxy implements InvocationHandler {
    /** Returned by {@link #handle} for a method this handler does not implement. */
    protected static final Object UNHANDLED = new Object();

    protected final Class<?> type;

    protected ApiProxy(Class<?> type) {
        this.type = type;
    }

    /** Create the proxy object. */
    @SuppressWarnings("unchecked")
    public <T> T create(Class<T> iface, Class<?>... extra) {
        Class<?>[] all = new Class<?>[extra.length + 1];
        all[0] = iface;
        System.arraycopy(extra, 0, all, 1, extra.length);
        return (T) Proxy.newProxyInstance(Bridge.class.getClassLoader(), all, this);
    }

    /** Implemented methods; {@link #UNHANDLED} otherwise. */
    protected abstract Object handle(Object proxy, Method m, String name, Object[] a) throws Throwable;

    @Override
    public final Object invoke(Object proxy, Method m, Object[] args) throws Throwable {
        Object[] a = args == null ? new Object[0] : args;
        String name = m.getName();
        Object r = handle(proxy, m, name, a);
        if (r != UNHANDLED) return r;
        if (m.getDeclaringClass() == Object.class) {
            return switch (name) {
                case "equals" -> proxy == a[0];
                case "hashCode" -> System.identityHashCode(proxy);
                case "toString" -> toString();
                default -> throw unsupported(name);
            };
        }
        if (m.isDefault()) return InvocationHandler.invokeDefault(proxy, m, a);
        throw unsupported(name);
    }

    protected UnsupportedOperationException unsupported(String method) {
        return new UnsupportedOperationException(
                "Rustic Bridge: " + type.getSimpleName() + "." + method + " not supported yet");
    }

    /** A proxy of {@code iface} whose every abstract method is unsupported. */
    public static <T> T stub(Class<T> iface) {
        return new ApiProxy(iface) {
            @Override
            protected Object handle(Object proxy, Method m, String name, Object[] a) {
                return UNHANDLED;
            }

            @Override
            public String toString() {
                return "Rustic" + iface.getSimpleName() + "Stub";
            }
        }.create(iface);
    }

    /** Whether the call has exactly these parameter types. */
    protected static boolean sig(Method m, Class<?>... types) {
        Class<?>[] p = m.getParameterTypes();
        if (p.length != types.length) return false;
        for (int i = 0; i < p.length; i++) if (p[i] != types[i]) return false;
        return true;
    }

    /** A primitive default for a return type (used by no-op setters / unknown getters). */
    protected static Object zero(Class<?> t) {
        if (t == boolean.class) return false;
        if (t == int.class) return 0;
        if (t == long.class) return 0L;
        if (t == double.class) return 0.0;
        if (t == float.class) return 0.0f;
        if (t == short.class) return (short) 0;
        if (t == byte.class) return (byte) 0;
        if (t == char.class) return '\0';
        return null;
    }
}
