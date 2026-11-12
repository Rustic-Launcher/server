//! The async instance pool (ADR 0021): a dispatcher thread polls
//! [`SharedHandle::due_async_tasks`] and hands each task to a small set of workers, which call
//! `run-job` / `run-task` in the plugin's **async instance** (its own store, never touched by
//! the server thread outside `on-load`/`on-enable`).
//!
//! * One task at a time per plugin (an instance is single-threaded): a plugin's backlog is
//!   served in order by whichever worker holds it, so a slow plugin cannot occupy every worker.
//! * A task starts only once the scheduling call has returned: workers wait while the plugin's
//!   main instance is executing and until its async instance finished `on-enable`.
//! * Faults are reported with [`SharedHandle::report_fault`]; the core disables the plugin on
//!   the next tick. The async instance is dropped right away (it may be poisoned).

use crate::instance::WasmInstance;
use rusticmc_core::context::{self, CallContext};
use rusticmc_core::model::TaskId;
use rusticmc_core::scheduler::AsyncDue;
use rusticmc_core::timings::SectionKind;
use rusticmc_core::{PluginFault, PluginId, SharedHandle};
use std::collections::{HashMap, VecDeque};
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The async side of one plugin incarnation.
pub(crate) struct AsyncEntry {
    pub plugin: PluginId,
    /// `None` once it faulted.
    pub instance: Mutex<Option<WasmInstance>>,
    /// The main instance is executing (a task scheduled by that call must wait for it).
    pub main_busy: AtomicBool,
    /// `on-enable` completed in the async instance.
    pub ready: AtomicBool,
    /// The plugin was unloaded: drop pending work, interrupt the running call.
    pub gone: AtomicBool,
    /// Shared with the store's call clock: interrupts the running call.
    pub cancel: Arc<AtomicBool>,
    queue: Mutex<EntryQueue>,
}

#[derive(Default)]
struct EntryQueue {
    running: bool,
    pending: VecDeque<AsyncDue>,
}

impl AsyncEntry {
    pub(crate) fn new(plugin: PluginId, instance: WasmInstance, cancel: Arc<AtomicBool>) -> Self {
        Self {
            plugin,
            instance: Mutex::new(Some(instance)),
            main_busy: AtomicBool::new(false),
            ready: AtomicBool::new(false),
            gone: AtomicBool::new(false),
            cancel,
            queue: Mutex::new(EntryQueue::default()),
        }
    }

    /// Mark unloaded and interrupt whatever runs.
    pub(crate) fn retire(&self) {
        self.gone.store(true, Ordering::Release);
        self.cancel.store(true, Ordering::Relaxed);
    }

    /// Run `f` on the async instance from the current thread (lifecycle calls), with an
    /// async call frame pushed.
    pub(crate) fn with_instance<R>(
        &self,
        f: impl FnOnce(&mut WasmInstance) -> Result<R, PluginFault>,
    ) -> Result<R, PluginFault> {
        let mut g = lock(&self.instance);
        let Some(inst) = g.as_mut() else {
            return Err(PluginFault::other(
                "the async instance is gone (it faulted)",
            ));
        };
        let r = context::with_call(
            CallContext {
                plugin_id: self.plugin.clone(),
                is_main: false,
            },
            || f(inst),
        );
        if r.is_err() {
            *g = None;
        }
        r
    }
}

/// Counters (benches, tests, `/rustic` diagnostics).
#[derive(Debug, Default)]
pub(crate) struct PoolStats {
    pub tasks_run: AtomicU64,
    pub faults: AtomicU64,
    /// Async tasks taken from the scheduler and not finished yet, plus one while the dispatcher
    /// is taking them: 0 means the pool holds no work (the test harness waits on it).
    pub in_flight: AtomicU64,
}

pub(crate) struct PoolInner {
    shared: SharedHandle,
    entries: Mutex<HashMap<PluginId, Arc<AsyncEntry>>>,
    work: Mutex<VecDeque<(Arc<AsyncEntry>, AsyncDue)>>,
    work_cv: Condvar,
    stop: AtomicBool,
    poll: Duration,
    pub stats: PoolStats,
}

/// The pool: dispatcher + workers. Dropping it stops and joins every thread.
pub(crate) struct AsyncPool {
    pub inner: Arc<PoolInner>,
    threads: Vec<JoinHandle<()>>,
}

impl AsyncPool {
    pub(crate) fn start(
        shared: SharedHandle,
        workers: usize,
        poll: Duration,
    ) -> std::io::Result<Self> {
        let inner = Arc::new(PoolInner {
            shared,
            entries: Mutex::new(HashMap::new()),
            work: Mutex::new(VecDeque::new()),
            work_cv: Condvar::new(),
            stop: AtomicBool::new(false),
            poll,
            stats: PoolStats::default(),
        });
        let mut threads = Vec::new();
        let i = inner.clone();
        threads.push(
            std::thread::Builder::new()
                .name("rustic-async-dispatch".into())
                .spawn(move || i.dispatch_loop())?,
        );
        for n in 0..workers.max(1) {
            let i = inner.clone();
            threads.push(
                std::thread::Builder::new()
                    .name(format!("rustic-async-{n}"))
                    .spawn(move || i.worker_loop())?,
            );
        }
        Ok(Self { inner, threads })
    }

    /// Register a plugin's async side (replacing a stale one with the same id).
    pub(crate) fn register(&self, entry: Arc<AsyncEntry>) {
        if let Some(old) = lock(&self.inner.entries).insert(entry.plugin.clone(), entry) {
            old.retire();
        }
    }

    /// Remove `entry` if it is still the registered one for its plugin.
    pub(crate) fn unregister(&self, entry: &Arc<AsyncEntry>) {
        let mut map = lock(&self.inner.entries);
        if map
            .get(&entry.plugin)
            .is_some_and(|e| Arc::ptr_eq(e, entry))
        {
            map.remove(&entry.plugin);
        }
    }
}

impl Drop for AsyncPool {
    fn drop(&mut self) {
        self.inner.stop.store(true, Ordering::Release);
        for e in lock(&self.inner.entries).values() {
            e.retire();
        }
        self.inner.work_cv.notify_all();
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
        lock(&self.inner.entries).clear();
        lock(&self.inner.work).clear();
    }
}

impl PoolInner {
    /// A task taken by the dispatcher is done (run, dropped or skipped).
    fn finished(&self, task: TaskId) {
        self.shared.async_task_finished(task);
        self.stats.in_flight.fetch_sub(1, Ordering::SeqCst);
    }

    fn stopping(&self) -> bool {
        self.stop.load(Ordering::Acquire)
    }

    fn dispatch_loop(&self) {
        while !self.stopping() {
            // The token covers the moment the tasks have left the scheduler but are not counted yet.
            self.stats.in_flight.fetch_add(1, Ordering::SeqCst);
            let due = self.shared.due_async_tasks(self.shared.now());
            self.stats
                .in_flight
                .fetch_add(due.len() as u64, Ordering::SeqCst);
            self.stats.in_flight.fetch_sub(1, Ordering::SeqCst);
            for d in due {
                self.submit(d);
            }
            std::thread::sleep(self.poll);
        }
    }

    fn submit(&self, due: AsyncDue) {
        let entry = lock(&self.entries).get(&due.plugin).cloned();
        let Some(entry) = entry else {
            self.finished(due.task);
            return;
        };
        {
            let mut q = lock(&entry.queue);
            if q.running {
                q.pending.push_back(due);
                return;
            }
            q.running = true;
        }
        lock(&self.work).push_back((entry, due));
        self.work_cv.notify_one();
    }

    fn worker_loop(&self) {
        loop {
            let job = {
                let mut w = lock(&self.work);
                loop {
                    if self.stopping() {
                        return;
                    }
                    if let Some(j) = w.pop_front() {
                        break j;
                    }
                    w = self
                        .work_cv
                        .wait_timeout(w, Duration::from_millis(100))
                        .unwrap_or_else(PoisonError::into_inner)
                        .0;
                }
            };
            let (entry, mut due) = job;
            loop {
                self.run(&entry, &due);
                let mut q = lock(&entry.queue);
                match q.pending.pop_front() {
                    Some(next) if !self.stopping() => due = next,
                    _ => {
                        for d in q.pending.drain(..) {
                            self.finished(d.task);
                        }
                        q.running = false;
                        break;
                    }
                }
            }
        }
    }

    /// Wait until the async instance may start (enabled, scheduling call returned).
    fn wait_ready(&self, entry: &AsyncEntry) -> bool {
        let start = Instant::now();
        loop {
            if entry.gone.load(Ordering::Acquire) || self.stopping() {
                return false;
            }
            if entry.ready.load(Ordering::Acquire) && !entry.main_busy.load(Ordering::Acquire) {
                return true;
            }
            if start.elapsed() > Duration::from_secs(30) {
                log::warn!(
                    target: "rustic",
                    "[{}] async task dropped: the plugin's main instance stayed busy or never enabled",
                    entry.plugin
                );
                return false;
            }
            std::thread::sleep(Duration::from_micros(200));
        }
    }

    fn run(&self, entry: &AsyncEntry, due: &AsyncDue) {
        if !self.wait_ready(entry) {
            self.finished(due.task);
            return;
        }
        let timings = self.shared.timings();
        let t0 = timings.start();
        let name = match &due.job {
            Some(j) => j.name.clone(),
            None if due.repeating => format!("timer#{}", due.task),
            None => "run-async".to_owned(),
        };
        let res = std::panic::catch_unwind(AssertUnwindSafe(|| {
            entry.with_instance(|inst| match &due.job {
                Some(job) => inst.run_job(due.task, &job.name, &job.payload),
                None => inst.run_task(due.task),
            })
        }))
        .unwrap_or_else(|p| Err(PluginFault::from_panic(p)));
        timings.finish(t0, &entry.plugin, SectionKind::AsyncTask, &name);
        self.stats.tasks_run.fetch_add(1, Ordering::Relaxed);
        if let Err(fault) = res
            && !entry.gone.load(Ordering::Acquire)
        {
            self.stats.faults.fetch_add(1, Ordering::Relaxed);
            log::error!(
                target: "rustic",
                "[{}] {} in async task `{name}`: {}",
                entry.plugin,
                fault.kind(),
                fault.message()
            );
            self.shared.report_fault(&entry.plugin, fault);
        }
        self.finished(due.task);
    }
}
