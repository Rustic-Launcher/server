// SPDX-License-Identifier: GPL-3.0-only
package rustic.bridge;

import net.kyori.adventure.text.Component;
import net.kyori.adventure.text.serializer.gson.GsonComponentSerializer;
import net.kyori.adventure.text.serializer.json.JSONOptions;
import net.kyori.adventure.text.serializer.legacy.LegacyComponentSerializer;
import net.kyori.adventure.text.serializer.plain.PlainTextComponentSerializer;

/**
 * Conversions between what plugins use (legacy {@code §}-coded strings, Adventure components)
 * and Rustic's {@code text} (a JSON chat component), through Adventure's own serializers.
 */
public final class TextUtil {
    private TextUtil() {}

    private static final LegacyComponentSerializer LEGACY = LegacyComponentSerializer.legacySection();
    /** The newest component JSON format (snake_case events, as the 26.3 server writes). */
    private static final GsonComponentSerializer GSON = GsonComponentSerializer.builder()
            .options(JSONOptions.byDataVersion().at(Integer.MAX_VALUE))
            .build();

    /** A legacy string ({@code §} codes, as {@code ChatColor} produces) as component JSON. */
    public static String legacyToJson(String legacy) {
        if (legacy == null) return null;
        if (legacy.indexOf('§') < 0) {
            StringBuilder b = new StringBuilder();
            Json.quote(b, legacy);
            return b.toString();
        }
        return GSON.serialize(LEGACY.deserialize(legacy));
    }

    public static String toJson(Component c) {
        return c == null ? null : GSON.serialize(c);
    }

    public static Component fromJson(String json) {
        if (json == null) return null;
        try {
            return GSON.deserialize(json);
        } catch (RuntimeException e) {
            // A component Adventure does not read (a newer event format): keep its text.
            return Component.text(plainOf(Json.parse(json)));
        }
    }

    /** The visible text of component JSON (text, translate keys' arguments, extra). */
    static String plainOf(Object v) {
        StringBuilder b = new StringBuilder();
        plain(v, b);
        return b.toString();
    }

    private static void plain(Object v, StringBuilder b) {
        if (v instanceof String s) {
            b.append(s);
        } else if (v instanceof java.util.List<?> l) {
            for (Object o : l) plain(o, b);
        } else if (v instanceof java.util.Map<?, ?> m) {
            Object t = m.get("text");
            if (t != null) b.append(t);
            Object with = m.get("with");
            if (m.get("translate") != null && t == null) {
                if (with instanceof java.util.List<?> w) for (Object o : w) plain(o, b.append(b.isEmpty() ? "" : " "));
                else b.append(m.get("translate"));
            }
            Object extra = m.get("extra");
            if (extra != null) plain(extra, b);
        }
    }

    public static String toLegacy(Component c) {
        return c == null ? null : LEGACY.serialize(c);
    }

    public static String jsonToLegacy(String json) {
        return json == null ? null : LEGACY.serialize(fromJson(json));
    }

    public static String plain(Component c) {
        return c == null ? "" : PlainTextComponentSerializer.plainText().serialize(c);
    }
}
