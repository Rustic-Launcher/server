// SPDX-License-Identifier: GPL-3.0-only
package rustic.bridge;

import java.math.BigInteger;
import java.util.ArrayList;
import java.util.Iterator;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;

/**
 * Minimal JSON reader/writer for the bridge protocol (no dependency on the plugin-visible Gson,
 * so a plugin shading another Gson cannot interfere).
 *
 * <p>Objects are {@link LinkedHashMap}, arrays {@link ArrayList}, integers {@link Long} (values
 * above {@code Long.MAX_VALUE}, i.e. the u64 halves of a UUID, wrap to their two's-complement
 * long, which is what {@link UUID} wants), other numbers {@link Double}.
 */
public final class Json {
    private Json() {}

    /** A number written verbatim (e.g. an unsigned 64-bit value). */
    public record Raw(String text) {}

    /**
     * An integer above {@code Long.MAX_VALUE} (a u64 from the host): reads as its wrapped long,
     * writes back as the original digits.
     */
    public static final class U64 extends Number {
        private final long bits;
        private final String text;

        U64(String text) {
            this.text = text;
            this.bits = new BigInteger(text).longValue();
        }

        @Override
        public long longValue() {
            return bits;
        }

        @Override
        public int intValue() {
            return (int) bits;
        }

        @Override
        public double doubleValue() {
            return new BigInteger(text).doubleValue();
        }

        @Override
        public float floatValue() {
            return (float) doubleValue();
        }

        @Override
        public String toString() {
            return text;
        }
    }

    // ------------------------------------------------------------------ reading

    public static Object parse(String s) {
        Reader r = new Reader(s);
        r.ws();
        Object v = r.value();
        r.ws();
        if (r.i != s.length()) throw r.err("trailing characters");
        return v;
    }

    private static final class Reader {
        final String s;
        int i;

        Reader(String s) {
            this.s = s;
        }

        IllegalArgumentException err(String what) {
            return new IllegalArgumentException("JSON: " + what + " at " + i);
        }

        void ws() {
            while (i < s.length()) {
                char c = s.charAt(i);
                if (c == ' ' || c == '\n' || c == '\r' || c == '\t') i++;
                else break;
            }
        }

        Object value() {
            if (i >= s.length()) throw err("unexpected end");
            char c = s.charAt(i);
            switch (c) {
                case '{':
                    return object();
                case '[':
                    return array();
                case '"':
                    return string();
                case 't':
                    expect("true");
                    return Boolean.TRUE;
                case 'f':
                    expect("false");
                    return Boolean.FALSE;
                case 'n':
                    expect("null");
                    return null;
                default:
                    return number();
            }
        }

        void expect(String word) {
            if (!s.startsWith(word, i)) throw err("expected " + word);
            i += word.length();
        }

        Map<String, Object> object() {
            Map<String, Object> m = new LinkedHashMap<>();
            i++;
            ws();
            if (i < s.length() && s.charAt(i) == '}') {
                i++;
                return m;
            }
            while (true) {
                ws();
                if (i >= s.length() || s.charAt(i) != '"') throw err("expected key");
                String k = string();
                ws();
                if (i >= s.length() || s.charAt(i) != ':') throw err("expected ':'");
                i++;
                ws();
                m.put(k, value());
                ws();
                if (i >= s.length()) throw err("unexpected end");
                char c = s.charAt(i++);
                if (c == '}') return m;
                if (c != ',') throw err("expected ',' or '}'");
            }
        }

        List<Object> array() {
            List<Object> l = new ArrayList<>();
            i++;
            ws();
            if (i < s.length() && s.charAt(i) == ']') {
                i++;
                return l;
            }
            while (true) {
                ws();
                l.add(value());
                ws();
                if (i >= s.length()) throw err("unexpected end");
                char c = s.charAt(i++);
                if (c == ']') return l;
                if (c != ',') throw err("expected ',' or ']'");
            }
        }

        String string() {
            i++;
            StringBuilder b = null;
            int start = i;
            while (true) {
                if (i >= s.length()) throw err("unterminated string");
                char c = s.charAt(i);
                if (c == '"') {
                    String out = b == null ? s.substring(start, i) : b.append(s, start, i).toString();
                    i++;
                    return out;
                }
                if (c == '\\') {
                    if (b == null) b = new StringBuilder();
                    b.append(s, start, i);
                    i++;
                    char e = s.charAt(i++);
                    switch (e) {
                        case 'n' -> b.append('\n');
                        case 't' -> b.append('\t');
                        case 'r' -> b.append('\r');
                        case 'b' -> b.append('\b');
                        case 'f' -> b.append('\f');
                        case 'u' -> {
                            b.append((char) Integer.parseInt(s.substring(i, i + 4), 16));
                            i += 4;
                        }
                        default -> b.append(e);
                    }
                    start = i;
                } else {
                    i++;
                }
            }
        }

        Object number() {
            int start = i;
            boolean integral = true;
            while (i < s.length()) {
                char c = s.charAt(i);
                if ((c >= '0' && c <= '9') || c == '-' || c == '+') i++;
                else if (c == '.' || c == 'e' || c == 'E') {
                    integral = false;
                    i++;
                } else break;
            }
            if (start == i) throw err("unexpected character");
            String t = s.substring(start, i);
            if (integral) {
                if (t.length() < 19) return Long.parseLong(t);
                BigInteger big = new BigInteger(t);
                if (big.bitLength() < 64) return big.longValue();
                return new U64(t);
            }
            return Double.parseDouble(t);
        }
    }

    // ------------------------------------------------------------------ writing

    public static String write(Object v) {
        StringBuilder b = new StringBuilder(128);
        write(b, v);
        return b.toString();
    }

    @SuppressWarnings("unchecked")
    static void write(StringBuilder b, Object v) {
        if (v == null) {
            b.append("null");
        } else if (v instanceof String str) {
            quote(b, str);
        } else if (v instanceof Boolean || v instanceof Integer || v instanceof Long || v instanceof U64) {
            b.append(v);
        } else if (v instanceof Number n) {
            double d = n.doubleValue();
            if (Double.isFinite(d)) {
                if (d == Math.rint(d) && Math.abs(d) < 1e15) b.append((long) d).append(".0");
                else b.append(d);
            } else {
                b.append("null");
            }
        } else if (v instanceof Raw r) {
            b.append(r.text());
        } else if (v instanceof Map<?, ?> m) {
            b.append('{');
            Iterator<? extends Map.Entry<?, ?>> it = m.entrySet().iterator();
            while (it.hasNext()) {
                Map.Entry<?, ?> e = it.next();
                quote(b, String.valueOf(e.getKey()));
                b.append(':');
                write(b, e.getValue());
                if (it.hasNext()) b.append(',');
            }
            b.append('}');
        } else if (v instanceof Iterable<?> l) {
            b.append('[');
            boolean first = true;
            for (Object o : l) {
                if (!first) b.append(',');
                first = false;
                write(b, o);
            }
            b.append(']');
        } else if (v instanceof Object[] arr) {
            write(b, java.util.Arrays.asList(arr));
        } else {
            quote(b, v.toString());
        }
    }

    static void quote(StringBuilder b, String s) {
        b.append('"');
        for (int k = 0; k < s.length(); k++) {
            char c = s.charAt(k);
            switch (c) {
                case '"' -> b.append("\\\"");
                case '\\' -> b.append("\\\\");
                case '\n' -> b.append("\\n");
                case '\r' -> b.append("\\r");
                case '\t' -> b.append("\\t");
                default -> {
                    if (c < 0x20) b.append(String.format("\\u%04x", (int) c));
                    else b.append(c);
                }
            }
        }
        b.append('"');
    }

    // ------------------------------------------------------------------ helpers

    /** A fresh ordered object from key/value pairs. */
    public static Map<String, Object> obj(Object... kv) {
        Map<String, Object> m = new LinkedHashMap<>();
        for (int k = 0; k + 1 < kv.length; k += 2) m.put((String) kv[k], kv[k + 1]);
        return m;
    }

    @SuppressWarnings("unchecked")
    public static Map<String, Object> map(Object o) {
        return (Map<String, Object>) o;
    }

    @SuppressWarnings("unchecked")
    public static List<Object> list(Object o) {
        return o == null ? List.of() : (List<Object>) o;
    }

    public static String str(Object o) {
        return o == null ? null : o.toString();
    }

    public static long lng(Object o) {
        return o == null ? 0 : ((Number) o).longValue();
    }

    public static double dbl(Object o) {
        return o == null ? 0 : ((Number) o).doubleValue();
    }

    public static boolean bool(Object o) {
        return o != null && (Boolean) o;
    }

    /** A Rustic `uuid` record ({@code {"hi": u64, "lo": u64}}) as a {@link UUID}. */
    public static UUID uuid(Object o) {
        Map<String, Object> m = map(o);
        return new UUID(lng(m.get("hi")), lng(m.get("lo")));
    }

    /** A {@link UUID} as a Rustic `uuid` record. */
    public static Map<String, Object> uuid(UUID u) {
        return obj(
                "hi", new Raw(Long.toUnsignedString(u.getMostSignificantBits())),
                "lo", new Raw(Long.toUnsignedString(u.getLeastSignificantBits())));
    }
}
