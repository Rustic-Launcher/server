// SPDX-License-Identifier: GPL-3.0-only
package rustic.bridge;

import java.io.BufferedInputStream;
import java.io.BufferedOutputStream;
import java.io.DataInputStream;
import java.io.DataOutputStream;
import java.io.EOFException;
import java.io.IOException;
import java.net.Socket;
import java.nio.charset.StandardCharsets;

/**
 * How the Java side talks to the Rust host. Messages are UTF-8 JSON; a reply is one kind byte
 * ({@link #OK} or {@link #ERR}) followed by the JSON. Both directions nest: while one side waits
 * for a reply, the other may send it requests, which are served on the waiting thread.
 */
public interface Transport {
    byte REQ = 1;
    byte OK = 2;
    byte ERR = 3;

    /** Send a request to the host and wait for its reply (served nested requests meanwhile). */
    byte[] call(byte[] request) throws IOException;

    /**
     * Sidecar transport: length-prefixed frames over a loopback TCP socket.
     *
     * <p>Frame: {@code u32 length (big-endian, bytes after this field) | u8 kind | u32 id |
     * payload}. Request ids are per direction; replies come back in LIFO order because both sides
     * are single-threaded on the connection.
     */
    final class Socket_ implements Transport {
        private final Socket socket;
        private final DataInputStream in;
        private final DataOutputStream out;
        private final Host host;
        private int nextId = 1;

        Socket_(Socket socket, Host host) throws IOException {
            this.socket = socket;
            socket.setTcpNoDelay(true);
            this.in = new DataInputStream(new BufferedInputStream(socket.getInputStream(), 1 << 16));
            this.out = new DataOutputStream(new BufferedOutputStream(socket.getOutputStream(), 1 << 16));
            this.host = host;
        }

        void writeFrame(byte kind, int id, byte[] payload, int off, int len) throws IOException {
            out.writeInt(len + 5);
            out.writeByte(kind);
            out.writeInt(id);
            out.write(payload, off, len);
            out.flush();
        }

        /** One frame: kind, id, payload. */
        record Frame(byte kind, int id, byte[] payload) {}

        Frame readFrame() throws IOException {
            int len;
            try {
                len = in.readInt();
            } catch (EOFException e) {
                throw new EOFException("the host closed the connection");
            }
            if (len < 5 || len > (64 << 20)) throw new IOException("bad frame length " + len);
            byte kind = in.readByte();
            int id = in.readInt();
            byte[] p = new byte[len - 5];
            in.readFully(p);
            return new Frame(kind, id, p);
        }

        void serveOne(Frame f) throws IOException {
            byte[] reply = host.handle(f.payload());
            writeFrame(reply[0], f.id(), reply, 1, reply.length - 1);
        }

        @Override
        public byte[] call(byte[] request) throws IOException {
            int id = nextId++;
            writeFrame(REQ, id, request, 0, request.length);
            while (true) {
                Frame f = readFrame();
                if (f.kind() == REQ) {
                    serveOne(f);
                } else if (f.id() == id) {
                    byte[] r = new byte[f.payload().length + 1];
                    r[0] = f.kind();
                    System.arraycopy(f.payload(), 0, r, 1, f.payload().length);
                    return r;
                } else {
                    throw new IOException("protocol error: reply " + f.id() + " while waiting for " + id);
                }
            }
        }

        /** The main loop: serve host requests until the connection closes. */
        void serve() throws IOException {
            while (true) {
                Frame f = readFrame();
                if (f.kind() != REQ) throw new IOException("protocol error: unexpected reply " + f.id());
                serveOne(f);
                if (host.stopRequested()) return;
            }
        }

        void close() {
            try {
                socket.close();
            } catch (IOException ignored) {
                // closing anyway
            }
        }
    }

    static byte[] utf8(String s) {
        return s.getBytes(StandardCharsets.UTF_8);
    }
}
