//! Watchdog support: the runtime records which plugin is executing and since when (a few
//! relaxed atomic stores per call), and a [`Watchdog`] thread reports calls that run longer
//! than a threshold. The host stops them (wasmtime epochs); the watchdog only observes.

use crate::id::PluginId;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicU8, AtomicU32, Ordering};
use std::sync::{Arc, PoisonError, RwLock};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// What kind of call is running.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum CallKind {
    /// `on-load`.
    Load = 1,
    /// `on-enable`.
    Enable,
    /// `on-disable`.
    Disable,
    /// An event handler.
    Event,
    /// A command.
    Command,
    /// A completion.
    Completion,
    /// A sync task.
    Task,
    /// An `on-message`.
    Message,
    /// A call on behalf of another plugin (service, permission provider).
    Service,
}

impl CallKind {
    fn from_u8(v: u8) -> Option<Self> {
        Some(match v {
            1 => CallKind::Load,
            2 => CallKind::Enable,
            3 => CallKind::Disable,
            4 => CallKind::Event,
            5 => CallKind::Command,
            6 => CallKind::Completion,
            7 => CallKind::Task,
            8 => CallKind::Message,
            9 => CallKind::Service,
            _ => return None,
        })
    }

    /// Lowercase label.
    pub fn label(self) -> &'static str {
        match self {
            CallKind::Load => "on-load",
            CallKind::Enable => "on-enable",
            CallKind::Disable => "on-disable",
            CallKind::Event => "event",
            CallKind::Command => "command",
            CallKind::Completion => "completion",
            CallKind::Task => "task",
            CallKind::Message => "message",
            CallKind::Service => "service",
        }
    }
}

/// A call in progress.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutingCall {
    /// The plugin.
    pub plugin: PluginId,
    /// What it is doing.
    pub kind: CallKind,
    /// Since when.
    pub since: Instant,
    /// For how long (at observation time).
    pub elapsed: Duration,
}

/// Shared record of the innermost call into a plugin on the main thread.
#[derive(Debug)]
pub struct ExecutionTracker {
    epoch: Instant,
    plugin: AtomicU32,   // index + 1, 0 = idle
    since_ns: AtomicI64, // signed offset from `epoch`
    kind: AtomicU8,
    names: RwLock<Vec<PluginId>>,
}

impl Default for ExecutionTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// What to restore when a nested call returns.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Previous {
    plugin: u32,
    since: i64,
    kind: u8,
}

impl ExecutionTracker {
    /// Idle tracker.
    pub fn new() -> Self {
        Self {
            epoch: Instant::now(),
            plugin: AtomicU32::new(0),
            since_ns: AtomicI64::new(0),
            kind: AtomicU8::new(0),
            names: RwLock::new(Vec::new()),
        }
    }

    /// Give a plugin an index (once per slot).
    pub(crate) fn register(&self, id: &PluginId) -> u32 {
        let mut names = self.names.write().unwrap_or_else(PoisonError::into_inner);
        if let Some(i) = names.iter().position(|n| n == id) {
            return i as u32;
        }
        names.push(id.clone());
        (names.len() - 1) as u32
    }

    /// Mark `index` as executing `kind` from now on; returns what to restore.
    #[inline]
    pub(crate) fn enter(&self, index: u32, kind: CallKind, now: Instant) -> Previous {
        let prev = Previous {
            plugin: self.plugin.load(Ordering::Relaxed),
            since: self.since_ns.load(Ordering::Relaxed),
            kind: self.kind.load(Ordering::Relaxed),
        };
        let ns = if now >= self.epoch {
            i64::try_from((now - self.epoch).as_nanos()).unwrap_or(i64::MAX)
        } else {
            -i64::try_from((self.epoch - now).as_nanos()).unwrap_or(i64::MAX)
        };
        self.since_ns.store(ns, Ordering::Relaxed);
        self.kind.store(kind as u8, Ordering::Relaxed);
        self.plugin.store(index + 1, Ordering::Release);
        prev
    }

    /// Restore the state before the matching `enter`.
    #[inline]
    pub(crate) fn exit(&self, prev: Previous) {
        self.since_ns.store(prev.since, Ordering::Relaxed);
        self.kind.store(prev.kind, Ordering::Relaxed);
        self.plugin.store(prev.plugin, Ordering::Release);
    }

    /// The call in progress, if any (observed from any thread; best effort).
    pub fn current(&self) -> Option<ExecutingCall> {
        let p = self.plugin.load(Ordering::Acquire);
        if p == 0 {
            return None;
        }
        let off = self.since_ns.load(Ordering::Relaxed);
        let since = if off >= 0 {
            self.epoch + Duration::from_nanos(off as u64)
        } else {
            self.epoch
                .checked_sub(Duration::from_nanos(off.unsigned_abs()))
                .unwrap_or(self.epoch)
        };
        let kind = CallKind::from_u8(self.kind.load(Ordering::Relaxed))?;
        let names = self.names.read().unwrap_or_else(PoisonError::into_inner);
        let plugin = names.get((p - 1) as usize)?.clone();
        Some(ExecutingCall {
            plugin,
            kind,
            since,
            elapsed: Instant::now().saturating_duration_since(since),
        })
    }
}

/// Reports calls that run longer than a threshold, once per call.
pub struct Watchdog {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl std::fmt::Debug for Watchdog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Watchdog").finish_non_exhaustive()
    }
}

/// Stateful check used by the watchdog thread (exposed for hosts that poll themselves).
#[derive(Debug, Default)]
pub struct StuckDetector {
    last: Option<(PluginId, Instant)>,
}

impl StuckDetector {
    /// The call to report now, if the current one exceeds `threshold` and was not reported.
    pub fn check(
        &mut self,
        tracker: &ExecutionTracker,
        threshold: Duration,
    ) -> Option<ExecutingCall> {
        let call = tracker.current()?;
        if call.elapsed < threshold {
            return None;
        }
        let key = (call.plugin.clone(), call.since);
        if self.last.as_ref() == Some(&key) {
            return None;
        }
        self.last = Some(key);
        Some(call)
    }
}

impl Watchdog {
    /// Start a thread that checks `tracker` every `poll` and calls `on_stuck` once for each
    /// call running longer than `threshold`.
    pub fn spawn(
        tracker: Arc<ExecutionTracker>,
        threshold: Duration,
        poll: Duration,
        on_stuck: impl Fn(&ExecutingCall) + Send + 'static,
    ) -> std::io::Result<Watchdog> {
        let stop = Arc::new(AtomicBool::new(false));
        let stop2 = stop.clone();
        let thread = std::thread::Builder::new()
            .name("rustic-watchdog".into())
            .spawn(move || {
                let mut detector = StuckDetector::default();
                while !stop2.load(Ordering::Relaxed) {
                    if let Some(call) = detector.check(&tracker, threshold) {
                        on_stuck(&call);
                    }
                    std::thread::park_timeout(poll);
                }
            })?;
        Ok(Watchdog {
            stop,
            thread: Some(thread),
        })
    }

    /// The default callback: an error log line naming the plugin.
    pub fn log_stuck(call: &ExecutingCall) {
        log::error!(
            target: "rustic",
            "[{}] {} has been running for {:.1} s (stuck? the host's epoch deadline should stop it)",
            call.plugin,
            call.kind.label(),
            call.elapsed.as_secs_f64()
        );
    }

    /// Stop the thread and wait for it.
    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            t.thread().unpark();
            let _ = t.join();
        }
    }
}

impl Drop for Watchdog {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[test]
    fn tracker_nesting() {
        let t = ExecutionTracker::new();
        assert!(t.current().is_none());
        let a = t.register(&PluginId::new_unchecked("a"));
        let b = t.register(&PluginId::new_unchecked("b"));
        assert_eq!(t.register(&PluginId::new_unchecked("a")), a);
        let t0 = Instant::now();
        let p1 = t.enter(a, CallKind::Event, t0);
        let p2 = t.enter(b, CallKind::Service, t0 + Duration::from_millis(1));
        assert_eq!(t.current().unwrap().plugin, "b");
        assert_eq!(t.current().unwrap().kind, CallKind::Service);
        t.exit(p2);
        let c = t.current().unwrap();
        assert_eq!(c.plugin, "a");
        assert_eq!(c.kind, CallKind::Event);
        t.exit(p1);
        assert!(t.current().is_none());
    }

    #[test]
    fn detector_reports_once() {
        let t = ExecutionTracker::new();
        let a = t.register(&PluginId::new_unchecked("slow"));
        let mut d = StuckDetector::default();
        let prev = t.enter(a, CallKind::Task, Instant::now() - Duration::from_secs(20));
        let call = d.check(&t, Duration::from_secs(10)).unwrap();
        assert_eq!(call.plugin, "slow");
        assert!(call.elapsed >= Duration::from_secs(20));
        assert!(d.check(&t, Duration::from_secs(10)).is_none());
        t.exit(prev);
        assert!(d.check(&t, Duration::from_secs(10)).is_none());
    }

    #[test]
    fn thread_calls_back() {
        let t = Arc::new(ExecutionTracker::new());
        let a = t.register(&PluginId::new_unchecked("loop"));
        let seen: Arc<Mutex<Vec<String>>> = Arc::default();
        let s2 = seen.clone();
        let _p = t.enter(a, CallKind::Event, Instant::now() - Duration::from_secs(5));
        let w = Watchdog::spawn(
            t.clone(),
            Duration::from_secs(1),
            Duration::from_millis(5),
            move |c| {
                s2.lock().unwrap().push(c.plugin.to_string());
            },
        )
        .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while seen.lock().unwrap().is_empty() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(5));
        }
        w.stop();
        assert_eq!(seen.lock().unwrap().as_slice(), ["loop"]);
        Watchdog::log_stuck(&t.current().unwrap());
    }
}
