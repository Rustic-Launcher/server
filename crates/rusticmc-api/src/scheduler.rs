//! The scheduler: main-thread tasks (closures) and async jobs (named closures run in the
//! plugin's async instance).
//!
//! # Two instances
//!
//! A plugin is instantiated twice: the **main instance** runs on the server tick (events,
//! commands, `run_later`/`run_timer` closures, world access); the **async instance** is a
//! separate instance of the same component that runs off the tick and may block (HTTP,
//! storage), but cannot touch the world. They share no memory, so a closure created in the main
//! instance cannot run in the async one. Instead:
//!
//! 1. register async jobs **by name** in `Plugin::enable` (which runs in both instances):
//!    `ctx.async_task("fetch", |url: String| { ... })`;
//! 2. start one from the main instance with a serde payload:
//!    `Scheduler::run_async("fetch", &url)?`;
//! 3. hand results back with [`Scheduler::post_to_main`], received in the main instance by a
//!    handler registered with `ctx.on_message("topic", |value: T| ...)`.
//!
//! Under the hood an async job is `scheduler.run-async-job(job, payload, delay-ms)`; the host
//! calls the `plugin.run-job(task, job, payload)` export in the async instance, and the SDK runs
//! the closure registered under that name.

use std::cell::RefCell;
use std::rc::Rc;

use serde::Serialize;

use crate::__rt;
use crate::bindings::rustic::api::scheduler as wit;
use crate::error::{Error, Result};
use crate::host::call;

/// A scheduled task. Copy it into closures to cancel later.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Task {
    id: Option<u32>,
    is_async: bool,
}

impl Task {
    pub(crate) fn main(id: u32) -> Task {
        Task {
            id: Some(id),
            is_async: false,
        }
    }

    pub(crate) fn inert() -> Task {
        Task {
            id: None,
            is_async: false,
        }
    }

    /// The host's task id (`None` for a task that was not scheduled).
    pub fn id(&self) -> Option<u32> {
        self.id
    }

    /// Whether it runs in the async instance.
    pub fn is_async(&self) -> bool {
        self.is_async
    }

    /// Cancels it (a running timer stops after the current run).
    pub fn cancel(&self) {
        let Some(id) = self.id else { return };
        call!("scheduler.cancel", wit::cancel(id));
        if !self.is_async {
            __rt::remove_task(id);
        }
    }
}

/// Main-thread scheduling from the async instance does nothing: silently during `enable` (which
/// runs in both instances), with a warning anywhere else.
fn skipped_in_async(what: &str) {
    if !__rt::in_lifecycle() {
        crate::warn!(
            "Scheduler::{what} called in the async instance; ignored (use Scheduler::post_to_main)"
        );
    }
}

/// Scheduling (all associated functions).
#[derive(Debug, Clone, Copy)]
pub struct Scheduler;

impl Scheduler {
    /// Runs `f` once on the main thread after `delay_ticks` (0 = next tick).
    pub fn run_later(delay_ticks: u64, f: impl FnOnce() + 'static) -> Task {
        if !__rt::is_main() {
            skipped_in_async("run_later");
            return Task::inert();
        }
        let id = call!("scheduler.run-later", wit::run_later(delay_ticks));
        __rt::add_task(id, __rt::MainTask::Once(Box::new(f)));
        Task::main(id)
    }

    /// Runs `f` on the main thread every `period_ticks` (>= 1) after `delay_ticks`. `f` gets the
    /// task, to cancel itself.
    pub fn run_timer(delay_ticks: u64, period_ticks: u64, f: impl FnMut(&Task) + 'static) -> Task {
        if !__rt::is_main() {
            skipped_in_async("run_timer");
            return Task::inert();
        }
        let id = call!(
            "scheduler.run-timer",
            wit::run_timer(delay_ticks, period_ticks.max(1))
        );
        __rt::add_task(id, __rt::MainTask::Repeat(Rc::new(RefCell::new(f))));
        Task::main(id)
    }

    /// Runs the async job registered as `job` (see `Ctx::async_task`) with `payload` (sent as
    /// JSON), as soon as possible.
    pub fn run_async<T: Serialize + ?Sized>(job: &str, payload: &T) -> Result<Task> {
        Self::run_async_later(job, payload, 0)
    }

    /// Runs an async job after `delay_ms`.
    pub fn run_async_later<T: Serialize + ?Sized>(
        job: &str,
        payload: &T,
        delay_ms: u64,
    ) -> Result<Task> {
        check_job(job)?;
        let bytes = serde_json::to_vec(payload)?;
        let id = call!(
            "scheduler.run-async-job",
            wit::run_async_job(job, &bytes, delay_ms)
        );
        Ok(Task {
            id: Some(id),
            is_async: true,
        })
    }

    /// Runs an async job every `period_ticks` (>= 1) after `delay_ticks`, with the same payload
    /// each time. Driven by a main-instance timer (so it is main-instance only and paced by the
    /// tick): cancelling the returned task stops it; runs already started finish.
    pub fn run_async_timer<T: Serialize + ?Sized>(
        job: &str,
        payload: &T,
        delay_ticks: u64,
        period_ticks: u64,
    ) -> Result<Task> {
        check_job(job)?;
        if !__rt::is_main() {
            return Err(Error::MainThreadOnly("Scheduler::run_async_timer"));
        }
        let bytes = serde_json::to_vec(payload)?;
        let job = job.to_owned();
        Ok(Self::run_timer(delay_ticks, period_ticks, move |_| {
            call!(
                "scheduler.run-async-job",
                wit::run_async_job(&job, &bytes, 0)
            );
        }))
    }

    /// From the async instance: delivers `value` (JSON) to the main instance's handler for
    /// `topic` (`ctx.on_message`) on the next tick.
    pub fn post_to_main<T: Serialize + ?Sized>(topic: &str, value: &T) -> Result<()> {
        let bytes = serde_json::to_vec(value)?;
        Self::post_to_main_raw(topic, &bytes);
        Ok(())
    }

    /// Like [`Scheduler::post_to_main`] with raw bytes.
    pub fn post_to_main_raw(topic: &str, payload: &[u8]) {
        call!("scheduler.post-to-main", wit::post_to_main(topic, payload));
    }

    /// Server ticks since start.
    pub fn current_tick() -> u64 {
        call!("scheduler.current-tick", wit::current_tick())
    }

    /// Whether this code runs in the main instance.
    pub fn is_main() -> bool {
        __rt::is_main()
    }
}

fn check_job(job: &str) -> Result<()> {
    if __rt::has_async_job(job) {
        Ok(())
    } else {
        Err(Error::Other(format!(
            "no async job named `{job}`: register it with ctx.async_task(\"{job}\", ..) in enable"
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_jobs_are_refused_before_any_host_call() {
        let err = Scheduler::run_async("nope", &1).unwrap_err();
        assert!(err.to_string().contains("no async job named `nope`"));
        assert!(Task::inert().id().is_none());
    }
}
