//! Capture of `log` records (plugin logs, runtime warnings) into the console of the
//! [`crate::MockServer`] living on the current thread.
//!
//! A global logger is installed once per process (if no other logger was installed first).
//! Records are routed to the innermost live server of the logging thread: tests run one per
//! thread and a server never leaves its thread, so parallel tests never see each other's lines.
//! Warnings and errors are also written to stderr (shown by `cargo test` for failing tests);
//! set `RUSTIC_TEST_LOG=1` (or `debug`/`trace`) to echo everything.

use std::cell::RefCell;
use std::rc::{Rc, Weak};
use std::sync::Once;

/// Lines captured for one server.
pub(crate) type Sink = Rc<RefCell<Vec<String>>>;

thread_local! {
    static SINKS: RefCell<Vec<Weak<RefCell<Vec<String>>>>> = const { RefCell::new(Vec::new()) };
}

static INSTALL: Once = Once::new();

struct CaptureLogger {
    echo_all: bool,
}

impl log::Log for CaptureLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::max_level()
    }

    fn log(&self, record: &log::Record<'_>) {
        if !self.enabled(record.metadata()) {
            return;
        }
        let line = format!("[{}] {}", record.level(), record.args());
        if self.echo_all || record.level() <= log::Level::Warn {
            eprintln!("{line}");
        }
        let _ = SINKS.try_with(|s| {
            let mut sinks = s.borrow_mut();
            sinks.retain(|w| w.strong_count() > 0);
            if let Some(sink) = sinks.last().and_then(Weak::upgrade)
                && let Ok(mut v) = sink.try_borrow_mut()
            {
                v.push(line);
            }
        });
    }

    fn flush(&self) {}
}

/// Install the capturing logger (once) and register a sink for the current thread.
pub(crate) fn register() -> Sink {
    INSTALL.call_once(|| {
        let env = std::env::var("RUSTIC_TEST_LOG").unwrap_or_default();
        let (level, echo_all) = match env.to_ascii_lowercase().as_str() {
            "" | "0" | "off" => (log::LevelFilter::Info, false),
            "trace" => (log::LevelFilter::Trace, true),
            "debug" => (log::LevelFilter::Debug, true),
            _ => (log::LevelFilter::Info, true),
        };
        if log::set_boxed_logger(Box::new(CaptureLogger { echo_all })).is_ok() {
            log::set_max_level(level);
        }
    });
    let sink: Sink = Rc::new(RefCell::new(Vec::new()));
    SINKS.with(|s| s.borrow_mut().push(Rc::downgrade(&sink)));
    sink
}
