//! The scheduler: main-thread tasks by tick, async tasks by wall-clock time, and the
//! `post-to-main` inbox. Thread-safe (the async instance schedules, cancels and posts).
//!
//! * **Sync tasks** run in [`crate::Rustic::tick`], in task-id order. A task scheduled with
//!   delay `d` during tick `t` first runs at tick `t + max(d, 1)`; a timer then every `period`
//!   ticks (min 1) after the tick it actually ran. Tasks that do not fit in the per-tick budget
//!   roll over to the next tick, which resumes from the first task that did not run.
//! * **Async tasks** are only tracked here: the host polls [`Scheduler::due_async`], runs them
//!   on its async instance pool and reports completion with [`Scheduler::async_finished`].
//!   A task is never handed out twice concurrently.
//! * **Messages** posted from the async instance are delivered at the start of the next tick
//!   to the main instance's `on-message`.
//!
//! Everything of a plugin is dropped by [`Scheduler::cancel_plugin`] (on disable).

use crate::id::PluginId;
use crate::model::TaskId;
use std::collections::{BTreeMap, VecDeque};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

#[derive(Clone, Debug)]
struct SyncTask {
    plugin: PluginId,
    next_tick: u64,
    period: Option<u64>,
}

#[derive(Clone, Debug)]
struct AsyncTask {
    plugin: PluginId,
    next_run: Instant,
    period: Option<Duration>,
    running: bool,
    job: Option<AsyncJob>,
}

/// A named job with its payload (`scheduler.run-async-job`), run with `plugin.run-job`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AsyncJob {
    /// Job name.
    pub name: String,
    /// Payload handed over from the main instance.
    pub payload: Vec<u8>,
}

/// A message from the async instance.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    /// Plugin.
    pub plugin: PluginId,
    /// Topic.
    pub topic: String,
    /// Payload.
    pub payload: Vec<u8>,
}

/// An async task the host should run now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AsyncDue {
    /// Task id (pass to `plugin.run-task` in the async instance).
    pub task: TaskId,
    /// Plugin.
    pub plugin: PluginId,
    /// Whether it repeats (timing key).
    pub repeating: bool,
    /// `Some` for `run-async-job`: call `plugin.run-job(task, job.name, job.payload)` instead
    /// of `plugin.run-task(task)`.
    pub job: Option<AsyncJob>,
}

/// A sync task due this tick.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SyncDue {
    pub task: TaskId,
    pub plugin: PluginId,
    pub repeating: bool,
}

#[derive(Default)]
struct Inner {
    sync: BTreeMap<TaskId, SyncTask>,
    asyncs: BTreeMap<TaskId, AsyncTask>,
    inbox: VecDeque<Message>,
    resume_from: Option<TaskId>,
}

/// Max messages waiting in the inbox (per server); more are dropped with a warning.
pub const MAX_INBOX: usize = 100_000;

/// The scheduler.
pub struct Scheduler {
    next_id: AtomicU32,
    inner: Mutex<Inner>,
}

impl std::fmt::Debug for Scheduler {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Scheduler").finish_non_exhaustive()
    }
}

impl Default for Scheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl Scheduler {
    /// Empty.
    pub fn new() -> Self {
        Self {
            next_id: AtomicU32::new(1),
            inner: Mutex::new(Inner::default()),
        }
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn next(&self) -> TaskId {
        // Ids are never reused within a run (4 billion tasks).
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        if id == u32::MAX {
            log::error!(target: "rustic", "task ids exhausted; wrapping");
            self.next_id.store(1, Ordering::Relaxed);
        }
        id
    }

    /// `scheduler.run-later`: once, `delay` ticks after `current_tick` (0 = next tick).
    pub fn run_later(&self, plugin: &PluginId, current_tick: u64, delay: u64) -> TaskId {
        let id = self.next();
        self.lock().sync.insert(
            id,
            SyncTask {
                plugin: plugin.clone(),
                next_tick: current_tick.saturating_add(delay.max(1)),
                period: None,
            },
        );
        id
    }

    /// `scheduler.run-timer`: every `period` ticks (min 1) after `delay`.
    pub fn run_timer(
        &self,
        plugin: &PluginId,
        current_tick: u64,
        delay: u64,
        period: u64,
    ) -> TaskId {
        let id = self.next();
        self.lock().sync.insert(
            id,
            SyncTask {
                plugin: plugin.clone(),
                next_tick: current_tick.saturating_add(delay.max(1)),
                period: Some(period.max(1)),
            },
        );
        id
    }

    /// `scheduler.run-async`: once in the async instance after `delay`.
    pub fn run_async(&self, plugin: &PluginId, now: Instant, delay: Duration) -> TaskId {
        let id = self.next();
        self.lock().asyncs.insert(
            id,
            AsyncTask {
                plugin: plugin.clone(),
                next_run: now + delay,
                period: None,
                running: false,
                job: None,
            },
        );
        id
    }

    /// `scheduler.run-async-job`: run job `name` once in the async instance after `delay`,
    /// with `payload` (the main -> async data channel).
    pub fn run_async_job(
        &self,
        plugin: &PluginId,
        now: Instant,
        name: String,
        payload: Vec<u8>,
        delay: Duration,
    ) -> TaskId {
        let id = self.next();
        self.lock().asyncs.insert(
            id,
            AsyncTask {
                plugin: plugin.clone(),
                next_run: now + delay,
                period: None,
                running: false,
                job: Some(AsyncJob { name, payload }),
            },
        );
        id
    }

    /// `scheduler.run-async-timer`: every `period` (min 1 ms) after `delay`.
    pub fn run_async_timer(
        &self,
        plugin: &PluginId,
        now: Instant,
        delay: Duration,
        period: Duration,
    ) -> TaskId {
        let id = self.next();
        self.lock().asyncs.insert(
            id,
            AsyncTask {
                plugin: plugin.clone(),
                next_run: now + delay,
                period: Some(period.max(Duration::from_millis(1))),
                running: false,
                job: None,
            },
        );
        id
    }

    /// `scheduler.cancel`: only the plugin's own tasks. Returns whether something was
    /// cancelled. A running async task finishes, but does not repeat.
    pub fn cancel(&self, plugin: &str, task: TaskId) -> bool {
        let mut g = self.lock();
        if g.sync.get(&task).is_some_and(|t| &*t.plugin == plugin) {
            g.sync.remove(&task);
            return true;
        }
        if g.asyncs.get(&task).is_some_and(|t| &*t.plugin == plugin) {
            g.asyncs.remove(&task);
            return true;
        }
        false
    }

    /// Drop every task and pending message of a plugin; returns how many tasks.
    pub fn cancel_plugin(&self, plugin: &str) -> usize {
        let mut g = self.lock();
        let before = g.sync.len() + g.asyncs.len();
        g.sync.retain(|_, t| &*t.plugin != plugin);
        g.asyncs.retain(|_, t| &*t.plugin != plugin);
        g.inbox.retain(|m| &*m.plugin != plugin);
        before - g.sync.len() - g.asyncs.len()
    }

    /// `scheduler.post-to-main`. Returns false (message dropped) when the inbox is full.
    pub fn post_to_main(&self, plugin: &PluginId, topic: String, payload: Vec<u8>) -> bool {
        let mut g = self.lock();
        if g.inbox.len() >= MAX_INBOX {
            log::warn!(target: "rustic", "[{plugin}] post-to-main dropped: inbox full ({MAX_INBOX})");
            return false;
        }
        g.inbox.push_back(Message {
            plugin: plugin.clone(),
            topic,
            payload,
        });
        true
    }

    /// Take every pending message.
    pub(crate) fn take_inbox(&self) -> Vec<Message> {
        self.lock().inbox.drain(..).collect()
    }

    /// Sync tasks due at `tick`, in id order (resuming after a rollover).
    pub(crate) fn due_sync(&self, tick: u64) -> Vec<SyncDue> {
        let mut g = self.lock();
        let resume = g.resume_from.take().unwrap_or(0);
        let mut due: Vec<SyncDue> = g
            .sync
            .iter()
            .filter(|(_, t)| t.next_tick <= tick)
            .map(|(id, t)| SyncDue {
                task: *id,
                plugin: t.plugin.clone(),
                repeating: t.period.is_some(),
            })
            .collect();
        if resume > 0 {
            let split = due.partition_point(|d| d.task < resume);
            due.rotate_left(split);
        }
        due
    }

    /// Remember where to resume next tick after running out of budget.
    pub(crate) fn set_resume(&self, task: TaskId) {
        self.lock().resume_from = Some(task);
    }

    /// Just before running a due task: re-check it still exists, then remove it (one-shot) or
    /// reschedule it (timer). Returns its plugin.
    pub(crate) fn begin_run(&self, task: TaskId, tick: u64) -> Option<PluginId> {
        let mut g = self.lock();
        let t = g.sync.get_mut(&task)?;
        if t.next_tick > tick {
            return None;
        }
        let plugin = t.plugin.clone();
        match t.period {
            Some(p) => t.next_tick = tick.saturating_add(p),
            None => {
                g.sync.remove(&task);
            }
        }
        Some(plugin)
    }

    /// Async tasks due at `now`, marked running. The host runs each and then calls
    /// [`Scheduler::async_finished`].
    pub fn due_async(&self, now: Instant) -> Vec<AsyncDue> {
        let mut g = self.lock();
        let mut out = Vec::new();
        for (id, t) in g.asyncs.iter_mut() {
            if !t.running && t.next_run <= now {
                t.running = true;
                if let Some(p) = t.period {
                    let next = t.next_run + p;
                    t.next_run = if next <= now { now + p } else { next };
                }
                out.push(AsyncDue {
                    task: *id,
                    plugin: t.plugin.clone(),
                    repeating: t.period.is_some(),
                    // One-shot: the payload is handed out once.
                    job: t.job.take(),
                });
            }
        }
        out
    }

    /// An async task handed out by [`Scheduler::due_async`] finished (successfully or not).
    pub fn async_finished(&self, task: TaskId) {
        let mut g = self.lock();
        let remove = match g.asyncs.get_mut(&task) {
            Some(t) => {
                t.running = false;
                t.period.is_none()
            }
            None => false,
        };
        if remove {
            g.asyncs.remove(&task);
        }
    }

    /// When the next async task is due (for the host's pool to sleep until then).
    pub fn next_async_deadline(&self) -> Option<Instant> {
        self.lock()
            .asyncs
            .values()
            .filter(|t| !t.running)
            .map(|t| t.next_run)
            .min()
    }

    /// Whether a task (sync or async) is scheduled.
    pub fn is_scheduled(&self, task: TaskId) -> bool {
        let g = self.lock();
        g.sync.contains_key(&task) || g.asyncs.contains_key(&task)
    }

    /// Number of scheduled (sync, async) tasks of a plugin.
    pub fn count(&self, plugin: &str) -> (usize, usize) {
        let g = self.lock();
        (
            g.sync.values().filter(|t| &*t.plugin == plugin).count(),
            g.asyncs.values().filter(|t| &*t.plugin == plugin).count(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::{Clock, FakeClock};

    fn p(s: &str) -> PluginId {
        PluginId::new_unchecked(s)
    }

    fn run_tick(s: &Scheduler, tick: u64) -> Vec<TaskId> {
        let mut ran = Vec::new();
        for d in s.due_sync(tick) {
            if s.begin_run(d.task, tick).is_some() {
                ran.push(d.task);
            }
        }
        ran
    }

    #[test]
    fn later_and_timer() {
        let s = Scheduler::new();
        let a = s.run_later(&p("a"), 10, 0); // tick 11
        let b = s.run_later(&p("a"), 10, 2); // tick 12
        let t = s.run_timer(&p("b"), 10, 1, 3); // 11, 14, 17
        assert_eq!(run_tick(&s, 10), Vec::<TaskId>::new());
        assert_eq!(run_tick(&s, 11), vec![a, t]);
        assert_eq!(run_tick(&s, 12), vec![b]);
        assert_eq!(run_tick(&s, 13), Vec::<TaskId>::new());
        assert_eq!(run_tick(&s, 14), vec![t]);
        assert!(!s.cancel("a", t), "not a's task");
        assert!(s.cancel("b", t));
        assert_eq!(run_tick(&s, 17), Vec::<TaskId>::new());
        assert!(!s.is_scheduled(a));
    }

    #[test]
    fn period_zero_is_one_and_cancel_during_tick() {
        let s = Scheduler::new();
        let t = s.run_timer(&p("a"), 0, 0, 0);
        let u = s.run_later(&p("a"), 0, 1);
        let due = s.due_sync(1);
        assert_eq!(due.len(), 2);
        // `t` cancels `u` while running: `u` must not run.
        assert!(s.begin_run(t, 1).is_some());
        s.cancel("a", u);
        assert!(s.begin_run(u, 1).is_none());
        assert_eq!(run_tick(&s, 2), vec![t]);
    }

    #[test]
    fn resume_after_rollover() {
        let s = Scheduler::new();
        let ids: Vec<_> = (0..4).map(|_| s.run_later(&p("a"), 0, 1)).collect();
        let due = s.due_sync(1);
        // Pretend only the first two fit in the budget.
        for d in &due[..2] {
            s.begin_run(d.task, 1);
        }
        s.set_resume(due[2].task);
        let t5 = s.run_later(&p("a"), 1, 1); // due at 2, lower priority than the rolled ones
        let next: Vec<_> = s.due_sync(2).into_iter().map(|d| d.task).collect();
        assert_eq!(next, vec![ids[2], ids[3], t5]);
    }

    #[test]
    fn async_tasks_with_fake_clock() {
        let clock = FakeClock::new();
        let s = Scheduler::new();
        let once = s.run_async(&p("a"), clock.now(), Duration::from_millis(100));
        let rep = s.run_async_timer(
            &p("a"),
            clock.now(),
            Duration::ZERO,
            Duration::from_millis(50),
        );
        assert_eq!(s.next_async_deadline(), Some(clock.now()));
        let due: Vec<_> = s
            .due_async(clock.now())
            .into_iter()
            .map(|d| d.task)
            .collect();
        assert_eq!(due, vec![rep]);
        // Still running: not handed out again.
        clock.advance(Duration::from_millis(60));
        assert!(s.due_async(clock.now()).is_empty());
        s.async_finished(rep);
        clock.advance(Duration::from_millis(40)); // t = 100
        let due: Vec<_> = s
            .due_async(clock.now())
            .into_iter()
            .map(|d| d.task)
            .collect();
        assert_eq!(due, vec![once, rep]);
        s.async_finished(once);
        s.async_finished(rep);
        assert!(!s.is_scheduled(once));
        assert!(s.is_scheduled(rep));
        // Missed periods are skipped, not replayed.
        clock.advance(Duration::from_secs(10));
        assert_eq!(s.due_async(clock.now()).len(), 1);
        s.async_finished(rep);
        assert!(s.due_async(clock.now()).is_empty());
        // Cancelled while running: finishes, never repeats.
        clock.advance(Duration::from_millis(50));
        assert_eq!(s.due_async(clock.now()).len(), 1);
        assert!(s.cancel("a", rep));
        s.async_finished(rep);
        clock.advance(Duration::from_secs(1));
        assert!(s.due_async(clock.now()).is_empty());
    }

    #[test]
    fn async_jobs_carry_their_payload_once() {
        let clock = FakeClock::new();
        let s = Scheduler::new();
        let j = s.run_async_job(
            &p("a"),
            clock.now(),
            "save".into(),
            vec![1, 2],
            Duration::from_millis(5),
        );
        assert!(s.due_async(clock.now()).is_empty());
        clock.advance(Duration::from_millis(5));
        let due = s.due_async(clock.now());
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].task, j);
        assert_eq!(
            due[0].job,
            Some(AsyncJob {
                name: "save".into(),
                payload: vec![1, 2]
            })
        );
        assert!(!due[0].repeating);
        s.async_finished(j);
        assert!(!s.is_scheduled(j));
    }

    #[test]
    fn inbox_and_cancel_plugin() {
        let s = Scheduler::new();
        s.run_later(&p("a"), 0, 1);
        s.run_async(&p("a"), Instant::now(), Duration::ZERO);
        s.run_later(&p("b"), 0, 1);
        assert!(s.post_to_main(&p("a"), "t".into(), vec![1]));
        assert!(s.post_to_main(&p("b"), "u".into(), vec![2]));
        assert_eq!(s.count("a"), (1, 1));
        assert_eq!(s.cancel_plugin("a"), 2);
        assert_eq!(s.count("a"), (0, 0));
        let msgs = s.take_inbox();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].topic, "u");
        assert!(s.take_inbox().is_empty());
    }
}
