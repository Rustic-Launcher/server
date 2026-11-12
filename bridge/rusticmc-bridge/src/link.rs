//! The wire protocol with the JVM: length-prefixed frames over a loopback TCP socket.
//!
//! Frame: `u32 length` (big-endian, bytes after this field) `| u8 kind | u32 id | payload`, kind
//! 1 = request, 2 = reply, 3 = error reply; payload = UTF-8 JSON (a request is an object with an
//! `op`). Request ids are per direction. Both sides are single-threaded on the connection, so
//! calls nest: while the host waits for its reply, the JVM may send requests (a plugin calling the
//! API), which are served on the waiting thread, and replies arrive in LIFO order.
//!
//! The socket has a read timeout: a JVM that does not answer within it (a plugin stuck in a loop)
//! is a [`LinkError::TimedOut`].

use serde_json::Value;
use std::cell::Cell;
use std::io::{self, Read, Write};
use std::net::TcpStream;
use std::time::Duration;

/// Request frame.
pub const REQ: u8 = 1;
/// Reply frame.
pub const OK: u8 = 2;
/// Error reply frame (the payload is a JSON string).
pub const ERR: u8 = 3;
/// Largest frame accepted.
pub const MAX_FRAME: usize = 64 << 20;

/// Why a call over the link did not complete.
#[derive(Debug)]
pub enum LinkError {
    /// No reply within the timeout.
    TimedOut(Duration),
    /// The connection broke (the JVM exited or crashed).
    Io(io::Error),
    /// The JVM sent something that is not the protocol.
    Protocol(String),
}

impl std::fmt::Display for LinkError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LinkError::TimedOut(d) => {
                write!(f, "the JVM did not answer within {} ms", d.as_millis())
            }
            LinkError::Io(e) => write!(f, "connection to the JVM lost: {e}"),
            LinkError::Protocol(m) => write!(f, "protocol error: {m}"),
        }
    }
}

/// What served a JVM request answers: a value, or an error message (an exception in Java).
pub type Served = Result<Value, String>;

/// One connection to the JVM.
#[derive(Debug)]
pub struct Link {
    stream: TcpStream,
    next_id: Cell<u32>,
    timeout: Duration,
    /// Round trips made (statistics).
    pub calls: Cell<u64>,
}

impl Link {
    /// Wrap a connected stream; `timeout` bounds every wait for the JVM.
    pub fn new(stream: TcpStream, timeout: Duration) -> io::Result<Self> {
        stream.set_nodelay(true)?;
        stream.set_read_timeout(Some(timeout))?;
        Ok(Self {
            stream,
            next_id: Cell::new(1),
            timeout,
            calls: Cell::new(0),
        })
    }

    fn write_frame(&self, kind: u8, id: u32, payload: &[u8]) -> io::Result<()> {
        let mut buf = Vec::with_capacity(payload.len() + 9);
        buf.extend_from_slice(&((payload.len() + 5) as u32).to_be_bytes());
        buf.push(kind);
        buf.extend_from_slice(&id.to_be_bytes());
        buf.extend_from_slice(payload);
        (&self.stream).write_all(&buf)
    }

    fn read_frame(&self) -> Result<(u8, u32, Vec<u8>), LinkError> {
        let mut head = [0u8; 9];
        self.read_exact(&mut head)?;
        let len = u32::from_be_bytes([head[0], head[1], head[2], head[3]]) as usize;
        if !(5..=MAX_FRAME).contains(&len) {
            return Err(LinkError::Protocol(format!("bad frame length {len}")));
        }
        let kind = head[4];
        let id = u32::from_be_bytes([head[5], head[6], head[7], head[8]]);
        let mut payload = vec![0u8; len - 5];
        self.read_exact(&mut payload)?;
        Ok((kind, id, payload))
    }

    fn read_exact(&self, buf: &mut [u8]) -> Result<(), LinkError> {
        match (&self.stream).read_exact(buf) {
            Ok(()) => Ok(()),
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) =>
            {
                Err(LinkError::TimedOut(self.timeout))
            }
            Err(e) => Err(LinkError::Io(e)),
        }
    }

    /// Send a request and wait for its reply, serving the JVM's nested requests with `serve`.
    /// `Ok(Err(message))`: the JVM answered with an error (an exception on its side).
    pub fn request(
        &self,
        body: &Value,
        serve: &dyn Fn(&Value) -> Served,
    ) -> Result<Served, LinkError> {
        let id = self.next_id.get();
        self.next_id.set(id.wrapping_add(1).max(1));
        let payload = serde_json::to_vec(body).map_err(|e| LinkError::Protocol(e.to_string()))?;
        self.write_frame(REQ, id, &payload).map_err(LinkError::Io)?;
        self.calls.set(self.calls.get() + 1);
        loop {
            let (kind, rid, payload) = self.read_frame()?;
            match kind {
                REQ => {
                    let req: Value = serde_json::from_slice(&payload)
                        .map_err(|e| LinkError::Protocol(format!("bad request JSON: {e}")))?;
                    let (k, out) = match serve(&req) {
                        Ok(v) => (OK, v),
                        Err(msg) => (ERR, Value::String(msg)),
                    };
                    let bytes =
                        serde_json::to_vec(&out).map_err(|e| LinkError::Protocol(e.to_string()))?;
                    self.write_frame(k, rid, &bytes).map_err(LinkError::Io)?;
                }
                OK | ERR if rid == id => {
                    let v: Value = if payload.is_empty() {
                        Value::Null
                    } else {
                        serde_json::from_slice(&payload)
                            .map_err(|e| LinkError::Protocol(format!("bad reply JSON: {e}")))?
                    };
                    return Ok(if kind == OK {
                        Ok(v)
                    } else {
                        Err(v
                            .as_str()
                            .map(str::to_owned)
                            .unwrap_or_else(|| v.to_string()))
                    });
                }
                _ => {
                    return Err(LinkError::Protocol(format!(
                        "frame kind {kind} id {rid} while waiting for reply {id}"
                    )));
                }
            }
        }
    }

    /// Close the socket (the JVM sees end of stream and stops).
    pub fn close(&self) {
        let _ = self.stream.shutdown(std::net::Shutdown::Both);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    /// A fake JVM: answers `echo`, and on `nested` first calls back into the host.
    fn fake_jvm(listener: TcpListener) {
        let (stream, _) = listener.accept().unwrap();
        let jvm = Link::new(stream, Duration::from_secs(5)).unwrap();
        loop {
            let Ok((kind, id, payload)) = jvm.read_frame() else {
                return;
            };
            assert_eq!(kind, REQ);
            let req: Value = serde_json::from_slice(&payload).unwrap();
            let reply = match req["op"].as_str().unwrap() {
                "echo" => (OK, req["x"].clone()),
                "nested" => {
                    let got = jvm
                        .request(&serde_json::json!({"op": "host"}), &|_| Ok(Value::Null))
                        .unwrap()
                        .unwrap();
                    (OK, serde_json::json!({"host said": got}))
                }
                "fail" => (ERR, Value::String("boom".into())),
                "hang" => {
                    std::thread::sleep(Duration::from_millis(400));
                    (OK, Value::Null)
                }
                _ => return,
            };
            let _ = jvm.write_frame(reply.0, id, &serde_json::to_vec(&reply.1).unwrap());
        }
    }

    #[test]
    fn requests_nest_and_time_out() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let t = std::thread::spawn(move || fake_jvm(listener));
        let link = Link::new(
            TcpStream::connect(addr).unwrap(),
            Duration::from_millis(200),
        )
        .unwrap();
        let none = |_: &Value| -> Served { Err("unexpected".into()) };
        let r = link
            .request(&serde_json::json!({"op": "echo", "x": [1, "a"]}), &none)
            .unwrap();
        assert_eq!(r.unwrap(), serde_json::json!([1, "a"]));
        let served = std::cell::Cell::new(0);
        let r = link
            .request(&serde_json::json!({"op": "nested"}), &|req| {
                assert_eq!(req["op"], "host");
                served.set(served.get() + 1);
                Ok(serde_json::json!(42))
            })
            .unwrap();
        assert_eq!(r.unwrap(), serde_json::json!({"host said": 42}));
        assert_eq!(served.get(), 1);
        let r = link
            .request(&serde_json::json!({"op": "fail"}), &none)
            .unwrap();
        assert_eq!(r.unwrap_err(), "boom");
        let r = link.request(&serde_json::json!({"op": "hang"}), &none);
        assert!(matches!(r, Err(LinkError::TimedOut(_))), "{r:?}");
        link.close();
        t.join().unwrap();
    }
}
