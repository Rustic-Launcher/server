// SPDX-License-Identifier: GPL-3.0-only
package rustic.bridge;

import java.io.ByteArrayOutputStream;
import java.io.FileDescriptor;
import java.io.FileOutputStream;
import java.io.OutputStream;
import java.io.PrintStream;
import java.io.PrintWriter;
import java.io.StringWriter;
import java.nio.charset.StandardCharsets;
import java.util.logging.Handler;
import java.util.logging.Level;
import java.util.logging.LogManager;
import java.util.logging.LogRecord;
import java.util.logging.Logger;

/**
 * Log output of the JVM. Every line goes to the process's real stdout as
 * {@code R|<LEVEL>|<source>|<text>}; the Rust side reads the child's stdout and forwards each
 * line to the `log` crate (target {@code bukkit}). Plugin loggers (java.util.logging), and
 * {@code System.out}/{@code System.err} writes, end up here.
 */
public final class BridgeLog {
    private BridgeLog() {}

    private static final PrintStream RAW =
            new PrintStream(new FileOutputStream(FileDescriptor.out), true, StandardCharsets.UTF_8);

    public static synchronized void line(String level, String source, String text) {
        for (String l : text.split("\r?\n", -1)) {
            RAW.print("R|" + level + "|" + source.replace('|', '/') + "|" + l + "\n");
        }
        RAW.flush();
    }

    public static void info(String source, String text) {
        line("INFO", source, text);
    }

    public static void warn(String source, String text) {
        line("WARN", source, text);
    }

    public static void error(String source, String text, Throwable t) {
        line("ERROR", source, t == null ? text : text + "\n" + stack(t));
    }

    public static String stack(Throwable t) {
        StringWriter w = new StringWriter();
        t.printStackTrace(new PrintWriter(w));
        return w.toString().stripTrailing();
    }

    static String level(Level l) {
        int v = l.intValue();
        if (v >= Level.SEVERE.intValue()) return "ERROR";
        if (v >= Level.WARNING.intValue()) return "WARN";
        if (v >= Level.INFO.intValue()) return "INFO";
        if (v >= Level.FINE.intValue()) return "DEBUG";
        return "TRACE";
    }

    /** A java.util.logging handler writing through {@link #line}. */
    public static final class JulHandler extends Handler {
        @Override
        public void publish(LogRecord r) {
            if (!isLoggable(r)) return;
            String msg = r.getMessage();
            if (msg == null) msg = "";
            Object[] params = r.getParameters();
            if (params != null && params.length > 0 && msg.contains("{")) {
                try {
                    msg = java.text.MessageFormat.format(msg, params);
                } catch (IllegalArgumentException ignored) {
                    // keep the raw message
                }
            }
            String src = r.getLoggerName() == null || r.getLoggerName().isEmpty() ? "jvm" : r.getLoggerName();
            String text = msg;
            if (r.getThrown() != null) text = msg + "\n" + stack(r.getThrown());
            line(level(r.getLevel()), src, text);
        }

        @Override
        public void flush() {}

        @Override
        public void close() {}
    }

    /** Route java.util.logging and System.out/err through the bridge log. */
    static void install() {
        LogManager.getLogManager().reset();
        Logger root = Logger.getLogger("");
        root.addHandler(new JulHandler());
        root.setLevel(Level.INFO);
        System.setOut(new PrintStream(new LineSink("INFO", "stdout"), true, StandardCharsets.UTF_8));
        System.setErr(new PrintStream(new LineSink("WARN", "stderr"), true, StandardCharsets.UTF_8));
    }

    /** A plugin logger, named like on Paper (the plugin name, or its prefix). */
    public static Logger pluginLogger(String name) {
        Logger l = Logger.getLogger(name);
        l.setUseParentHandlers(true);
        return l;
    }

    private static final class LineSink extends OutputStream {
        private final String level;
        private final String source;
        private final ByteArrayOutputStream buf = new ByteArrayOutputStream();

        LineSink(String level, String source) {
            this.level = level;
            this.source = source;
        }

        @Override
        public synchronized void write(int b) {
            if (b == '\n') {
                flushLine();
            } else if (b != '\r') {
                buf.write(b);
            }
        }

        private void flushLine() {
            line(level, source, buf.toString(StandardCharsets.UTF_8));
            buf.reset();
        }

        @Override
        public synchronized void flush() {
            if (buf.size() > 0) flushLine();
        }
    }
}
