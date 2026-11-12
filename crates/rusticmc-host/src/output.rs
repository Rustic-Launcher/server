//! Guest stdout/stderr: forwarded to the server log line by line with the plugin id, through a
//! token bucket (a plugin printing in a loop cannot flood the console), and the last lines kept
//! so a Rust panic message (printed to stderr right before the abort trap) can be attached to
//! the [`rusticmc_core::PluginFault::Panic`].

use crate::config::OutputConfig;
use bytes::Bytes;
use std::collections::VecDeque;
use std::pin::Pin;
use std::sync::{Arc, Mutex, PoisonError};
use std::task::{Context, Poll};
use std::time::Instant;
use wasmtime_wasi::cli::{IsTerminal, StdoutStream};
use wasmtime_wasi::p2::{OutputStream, Pollable, StreamError};

/// Lines of stderr remembered for panic messages.
const RECENT_LINES: usize = 32;

struct State {
    prefix: String,
    level: log::Level,
    cfg: OutputConfig,
    partial: Vec<u8>,
    tokens: f64,
    refilled: Instant,
    suppressed: u64,
    recent: VecDeque<(u64, String)>,
    seq: u64,
}

impl State {
    fn emit(&mut self, line: &[u8]) {
        let mut text = String::from_utf8_lossy(line).into_owned();
        if text.ends_with('\r') {
            text.pop();
        }
        if text.len() > self.cfg.max_line_bytes {
            let mut cut = self.cfg.max_line_bytes;
            while !text.is_char_boundary(cut) {
                cut -= 1;
            }
            text.truncate(cut);
            text.push_str("...");
        }
        self.seq += 1;
        if self.recent.len() == RECENT_LINES {
            self.recent.pop_front();
        }
        self.recent.push_back((self.seq, text.clone()));
        // Token bucket.
        let now = Instant::now();
        let elapsed = now.saturating_duration_since(self.refilled).as_secs_f64();
        self.refilled = now;
        let burst = f64::from(self.cfg.burst.max(1));
        self.tokens = (self.tokens + elapsed * f64::from(self.cfg.lines_per_second)).min(burst);
        if self.tokens < 1.0 {
            self.suppressed += 1;
            return;
        }
        self.tokens -= 1.0;
        if self.suppressed > 0 {
            log::warn!(
                target: "rustic",
                "{} {} output line(s) suppressed (rate limit)",
                self.prefix,
                self.suppressed
            );
            self.suppressed = 0;
        }
        log::log!(target: "rustic", self.level, "{} {text}", self.prefix);
    }

    fn write(&mut self, bytes: &[u8]) {
        let mut rest = bytes;
        while let Some(i) = rest.iter().position(|b| *b == b'\n') {
            if self.partial.is_empty() {
                self.emit(&rest[..i]);
            } else {
                let mut line = std::mem::take(&mut self.partial);
                line.extend_from_slice(&rest[..i]);
                self.emit(&line);
            }
            rest = &rest[i + 1..];
        }
        if !rest.is_empty() {
            // Bound the partial line too.
            let cap = self.cfg.max_line_bytes.saturating_add(4);
            let room = cap.saturating_sub(self.partial.len());
            self.partial
                .extend_from_slice(&rest[..rest.len().min(room)]);
        }
    }
}

/// One guest output stream (stdout or stderr of one instance). Cheap to clone.
#[derive(Clone)]
pub(crate) struct LogSink(Arc<Mutex<State>>);

impl LogSink {
    /// A sink logging at `level` with `prefix` (e.g. `[hello]`).
    pub(crate) fn new(prefix: String, level: log::Level, cfg: OutputConfig) -> Self {
        let tokens = f64::from(cfg.burst.max(1));
        Self(Arc::new(Mutex::new(State {
            prefix,
            level,
            cfg,
            partial: Vec::new(),
            tokens,
            refilled: Instant::now(),
            suppressed: 0,
            recent: VecDeque::new(),
            seq: 0,
        })))
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Sequence number of the last line written (a mark for [`LogSink::lines_since`]).
    pub(crate) fn mark(&self) -> u64 {
        self.lock().seq
    }

    /// Lines written after `mark` (at most the last 32), flushing any unterminated line first.
    pub(crate) fn lines_since(&self, mark: u64) -> Vec<String> {
        let mut s = self.lock();
        if !s.partial.is_empty() {
            let line = std::mem::take(&mut s.partial);
            s.emit(&line);
        }
        s.recent
            .iter()
            .filter(|(seq, _)| *seq > mark)
            .map(|(_, l)| l.clone())
            .collect()
    }

    fn write(&self, bytes: &[u8]) {
        self.lock().write(bytes);
    }
}

/// Extract a Rust panic message from stderr lines:
/// `thread '<unnamed>' panicked at src/lib.rs:5:9:` then the message, then `note: ...`.
pub(crate) fn panic_message(lines: &[String]) -> Option<String> {
    let start = lines.iter().rposition(|l| l.contains(" panicked at "))?;
    let header = &lines[start];
    let location = header
        .split_once(" panicked at ")
        .map(|(_, loc)| loc.trim().trim_end_matches(':').to_owned())
        .unwrap_or_default();
    // Older formats put the message on the header line: `panicked at 'msg', src/lib.rs:1:2`.
    let mut message: Vec<&str> = Vec::new();
    for l in &lines[start + 1..] {
        if l.starts_with("note: ") || l.starts_with("stack backtrace:") {
            break;
        }
        message.push(l);
    }
    let message = message.join("\n").trim().to_owned();
    Some(match (message.is_empty(), location.is_empty()) {
        (true, _) => location,
        (false, true) => message,
        (false, false) => format!("{message} (at {location})"),
    })
}

impl IsTerminal for LogSink {
    fn is_terminal(&self) -> bool {
        false
    }
}

impl StdoutStream for LogSink {
    fn p2_stream(&self) -> Box<dyn OutputStream> {
        Box::new(self.clone())
    }

    fn async_stream(&self) -> Box<dyn tokio::io::AsyncWrite + Send + Sync> {
        Box::new(self.clone())
    }
}

#[wasmtime_wasi::async_trait]
impl OutputStream for LogSink {
    fn write(&mut self, bytes: Bytes) -> Result<(), StreamError> {
        LogSink::write(self, &bytes);
        Ok(())
    }

    fn flush(&mut self) -> Result<(), StreamError> {
        Ok(())
    }

    fn check_write(&mut self) -> Result<usize, StreamError> {
        Ok(64 * 1024)
    }
}

#[wasmtime_wasi::async_trait]
impl Pollable for LogSink {
    async fn ready(&mut self) {}
}

impl tokio::io::AsyncWrite for LogSink {
    fn poll_write(
        self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        LogSink::write(&self, buf);
        Poll::Ready(Ok(buf.len()))
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<std::io::Result<()>> {
        Poll::Ready(Ok(()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sink(lines_per_second: u32, burst: u32) -> LogSink {
        LogSink::new(
            "[t]".into(),
            log::Level::Info,
            OutputConfig {
                lines_per_second,
                burst,
                max_line_bytes: 16,
            },
        )
    }

    #[test]
    fn splits_lines_and_keeps_recent() {
        let s = sink(1000, 1000);
        let m = s.mark();
        s.write(b"hello\nwor");
        s.write(b"ld\r\npartial");
        let lines = s.lines_since(m);
        assert_eq!(lines, ["hello", "world", "partial"]);
        assert!(s.lines_since(s.mark()).is_empty());
    }

    #[test]
    fn long_lines_are_cut() {
        let s = sink(1000, 1000);
        s.write(&[b'x'; 100]);
        s.write(b"\n");
        let l = s.lines_since(0);
        assert_eq!(l.len(), 1);
        assert!(l[0].len() <= 20 && l[0].ends_with("..."), "{l:?}");
    }

    #[test]
    fn rate_limit_still_records_lines() {
        let s = sink(1, 2);
        for i in 0..10 {
            s.write(format!("line {i}\n").as_bytes());
        }
        assert_eq!(s.lock().suppressed, 8);
        assert_eq!(
            s.lines_since(0).len(),
            10,
            "recent lines are kept even when suppressed"
        );
    }

    #[test]
    fn panic_messages() {
        let lines: Vec<String> = [
            "some output",
            "",
            "thread '<unnamed>' panicked at src/lib.rs:12:5:",
            "boom: the handler exploded",
            "note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        assert_eq!(
            panic_message(&lines).unwrap(),
            "boom: the handler exploded (at src/lib.rs:12:5)"
        );
        assert_eq!(panic_message(&lines[..2]), None);
        let only_header = vec!["thread 'main' panicked at x.rs:1:1:".to_owned()];
        assert_eq!(panic_message(&only_header).unwrap(), "x.rs:1:1");
    }
}
