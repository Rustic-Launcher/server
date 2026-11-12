//! `server-list-ping` from the connection threads (Horizon's `status_response`, patch U1).
//!
//! A ping is answered on its connection's own thread, where the runtime cannot run. When a plugin
//! listens (a flag the server thread refreshes each tick, so a ping nobody listens to costs one
//! atomic load), the ping is handed to the server thread, which fires the event at the end of its
//! tick, and the connection waits for the answer up to [`WAIT`]; a late answer leaves the ping as
//! the server would have answered it.
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, SyncSender, sync_channel};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use rusticmc_core::model::{EventKind, ServerListPing, Text};

/// The longest a ping waits for the plugins (a tick is 50 ms).
const WAIT: Duration = Duration::from_millis(250);
/// Pings waiting at once past which they are answered unchanged (a flood of pings).
const MAX_WAITING: usize = 64;

static LISTENED: AtomicBool = AtomicBool::new(false);

type Request = (ServerListPing, SyncSender<ServerListPing>);

fn queue() -> &'static Mutex<Vec<Request>> {
    static QUEUE: OnceLock<Mutex<Vec<Request>>> = OnceLock::new();
    QUEUE.get_or_init(|| Mutex::new(Vec::new()))
}

/// On a connection thread: the ping as the plugins leave it, or None (nobody listens, or no
/// answer in time).
pub fn ask(ping: ServerListPing) -> Option<ServerListPing> {
    if !LISTENED.load(Ordering::Relaxed) {
        return None;
    }
    let (tx, rx): (SyncSender<ServerListPing>, Receiver<ServerListPing>) = sync_channel(1);
    {
        let mut q = queue().lock().unwrap_or_else(|e| e.into_inner());
        if q.len() >= MAX_WAITING {
            return None;
        }
        q.push((ping, tx));
    }
    rx.recv_timeout(WAIT).ok()
}

/// On the server thread, at the end of a tick: answer the pings waiting and note whether any
/// plugin listens.
pub fn serve(rustic: &rusticmc_core::Rustic, backend: &mut dyn rusticmc_core::Backend) {
    LISTENED.store(
        rustic.has_listeners_for(EventKind::ServerListPing),
        Ordering::Relaxed,
    );
    let waiting: Vec<Request> = {
        let mut q = queue().lock().unwrap_or_else(|e| e.into_inner());
        std::mem::take(&mut *q)
    };
    for (ping, answer) in waiting {
        let ping = rustic.fire_server_list_ping(ping, backend);
        let _ = answer.try_send(ping);
    }
}

/// The ping's text for the plugins: the motd as `server.properties` has it (legacy `§` codes are
/// shown as they are).
pub fn motd_text(motd: &str) -> Text {
    Text::plain(motd)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ping() -> ServerListPing {
        ServerListPing {
            address: "127.0.0.1".into(),
            motd: Text::plain("m"),
            max_players: 20,
            online_players: 0,
            show_players: true,
        }
    }

    #[test]
    fn a_ping_is_handed_over_only_when_listened_to() {
        LISTENED.store(false, Ordering::Relaxed);
        assert!(ask(ping()).is_none());
        assert!(queue().lock().unwrap().is_empty());
        // Listened to: it waits for the server thread and gets its answer.
        LISTENED.store(true, Ordering::Relaxed);
        let server = std::thread::spawn(|| {
            for _ in 0..200 {
                let waiting: Vec<Request> = std::mem::take(&mut *queue().lock().unwrap());
                if let Some((mut p, answer)) = waiting.into_iter().next() {
                    p.max_players = 99;
                    let _ = answer.try_send(p);
                    return;
                }
                std::thread::sleep(Duration::from_millis(1));
            }
        });
        let answer = ask(ping()).expect("answered in time");
        assert_eq!(answer.max_players, 99);
        server.join().unwrap();
        LISTENED.store(false, Ordering::Relaxed);
    }
}
