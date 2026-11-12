package com.example.green.libs.util;

// A "shaded library" using Unsafe the way general-purpose libraries do: not decisive.
public final class Helper {
    static Object unsafe() throws Exception {
        java.lang.reflect.Field f = sun.misc.Unsafe.class.getDeclaredField("theUnsafe");
        f.setAccessible(true);
        return f.get(null);
    }
}
