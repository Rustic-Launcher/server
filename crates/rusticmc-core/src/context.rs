//! The per-call context: how a host function (a wasmtime import, which only gets
//! `&mut StoreData`) reaches the core state and the current [`Backend`]. See ADR 0004.
//!
//! Three thread-local, scoped slots, all set by the runtime around each entry point and each
//! call into a plugin, and restored on exit (also when unwinding):
//!
//! * the **backend**: [`with_backend`] installs a `&mut dyn Backend` for the duration of a
//!   closure; [`backend_mut`] lends it out. While lent, the slot is empty, so a nested
//!   `backend_mut` returns an error instead of aliasing the `&mut` (no double borrow, no
//!   panic). Without a backend (async threads, outside a tick) it returns
//!   [`HostError::MainThreadOnly`]. A nested [`with_backend`] (the engine calling back into
//!   the runtime from inside a backend call) shadows the slot and restores it afterwards.
//! * the **call stack**: [`CallContext`] `{ plugin_id, is_main }` of the plugin currently
//!   executing (the innermost one); [`current_call`] reads it.
//! * the **runtime**: [`runtime`] returns a handle to the [`Rustic`] whose entry point is
//!   running, and [`api`] the [`PluginApi`] of the plugin currently executing, which is what
//!   main-instance imports use (they cannot spoof another plugin's identity).
//!
//! The backend slot is implemented with `scoped-tls-hkt` (a higher-kinded scoped thread
//! local): no `unsafe` in this crate.

use crate::backend::Backend;
use crate::id::PluginId;
use crate::model::HostError;
use crate::runtime::{Inner, PluginApi, Rustic};
use scoped_tls_hkt::{ReborrowMut, scoped_thread_local};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

/// A `&mut dyn Backend` with its lifetime as a parameter, so it can live in a scoped TLS slot.
pub struct BackendSlot<'a>(&'a mut (dyn Backend + 'a));

impl<'a, 'b: 'a> ReborrowMut<'a> for BackendSlot<'b> {
    type Result = BackendSlot<'a>;
    fn reborrow_mut(&'a mut self) -> Self::Result {
        BackendSlot(&mut *self.0)
    }
}

scoped_thread_local!(static mut BACKEND: for<'a> BackendSlot<'a>);

thread_local! {
    static LENT: Cell<u32> = const { Cell::new(0) };
    static CALLS: RefCell<Vec<CallContext>> = const { RefCell::new(Vec::new()) };
    static RUNTIMES: RefCell<Vec<Weak<Inner>>> = const { RefCell::new(Vec::new()) };
}

/// Run `f` with `backend` installed as the current backend of this thread.
pub fn with_backend<R>(backend: &mut dyn Backend, f: impl FnOnce() -> R) -> R {
    BACKEND.set(BackendSlot(backend), f)
}

/// Whether a backend is installed and not currently lent out.
pub fn has_backend() -> bool {
    BACKEND.is_set()
}

/// Borrow the current backend for the duration of `f`.
///
/// Errors: [`HostError::MainThreadOnly`] when no backend is installed (async instance, outside
/// a runtime entry point); [`HostError::Other`] when it is already lent out further up the
/// stack (a re-entrant call that would alias it).
pub fn backend_mut<R>(f: impl FnOnce(&mut dyn Backend) -> R) -> Result<R, HostError> {
    if !BACKEND.is_set() {
        return Err(if LENT.with(Cell::get) > 0 {
            HostError::Other(
                "the backend is already in use by an enclosing call (re-entrant access)".into(),
            )
        } else {
            HostError::MainThreadOnly
        });
    }
    struct Lent;
    impl Drop for Lent {
        fn drop(&mut self) {
            LENT.with(|c| c.set(c.get().saturating_sub(1)));
        }
    }
    LENT.with(|c| c.set(c.get() + 1));
    let _lent = Lent;
    Ok(BACKEND.with(|slot| f(slot.0)))
}

/// Like [`backend_mut`] for calls that return a `Result<T, HostError>` themselves.
pub fn backend_call<T>(
    f: impl FnOnce(&mut dyn Backend) -> Result<T, HostError>,
) -> Result<T, HostError> {
    backend_mut(f).and_then(|r| r)
}

/// Who is executing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallContext {
    /// The plugin.
    pub plugin_id: PluginId,
    /// Whether this is the main (tick) instance; false in the async instance.
    pub is_main: bool,
}

/// The innermost plugin call on this thread.
pub fn current_call() -> Option<CallContext> {
    CALLS.with(|c| c.borrow().last().cloned())
}

/// Depth of nested plugin calls on this thread.
pub fn call_depth() -> usize {
    CALLS.with(|c| c.borrow().len())
}

/// Whether `plugin` is anywhere on this thread's call stack (it is executing).
pub fn is_executing(plugin: &str) -> bool {
    CALLS.with(|c| c.borrow().iter().any(|x| &*x.plugin_id == plugin))
}

/// Run `f` with `ctx` pushed on the call stack. The runtime does this around every call into a
/// plugin; the host does it around calls into its async instances (`is_main: false`) and the
/// test harness may use it to simulate a plugin's imports.
pub fn with_call<R>(ctx: CallContext, f: impl FnOnce() -> R) -> R {
    struct Pop;
    impl Drop for Pop {
        fn drop(&mut self) {
            CALLS.with(|c| {
                c.borrow_mut().pop();
            });
        }
    }
    CALLS.with(|c| c.borrow_mut().push(ctx));
    let _pop = Pop;
    f()
}

/// Run `f` with `inner` as the current runtime of this thread.
pub(crate) fn with_runtime<R>(inner: &Rc<Inner>, f: impl FnOnce() -> R) -> R {
    struct Pop;
    impl Drop for Pop {
        fn drop(&mut self) {
            RUNTIMES.with(|c| {
                c.borrow_mut().pop();
            });
        }
    }
    RUNTIMES.with(|c| c.borrow_mut().push(Rc::downgrade(inner)));
    let _pop = Pop;
    f()
}

/// The runtime whose entry point is running on this thread.
pub fn runtime() -> Result<Rustic, HostError> {
    RUNTIMES
        .with(|c| c.borrow().last().and_then(Weak::upgrade))
        .map(Rustic::from_inner)
        .ok_or(HostError::MainThreadOnly)
}

/// The API handle of the plugin currently executing in the main instance: what the host's
/// main-instance import functions use.
pub fn api() -> Result<PluginApi, HostError> {
    let call = current_call().ok_or(HostError::MainThreadOnly)?;
    if !call.is_main {
        return Err(HostError::MainThreadOnly);
    }
    Ok(runtime()?.api(call.plugin_id))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::NullBackend;
    use crate::model::{Text, Uuid};

    struct Counter(u32);
    impl Backend for Counter {
        fn players_online(&mut self) -> Result<Vec<Uuid>, HostError> {
            self.0 += 1;
            Ok(vec![Uuid::new(0, u64::from(self.0))])
        }
    }

    #[test]
    fn absent_backend_is_main_thread_only() {
        assert!(!has_backend());
        assert_eq!(backend_mut(|_| ()), Err(HostError::MainThreadOnly));
    }

    #[test]
    fn lend_and_nested_access() {
        let mut c = Counter(0);
        with_backend(&mut c, || {
            assert!(has_backend());
            let r = backend_call(|b| b.players_online()).unwrap();
            assert_eq!(r[0].lo, 1);
            // Nested access while lent is an error, not a panic or an alias.
            let nested = backend_mut(|_| backend_mut(|_| ()));
            assert!(matches!(nested, Ok(Err(HostError::Other(_)))));
            // A nested with_backend (engine calling back) shadows and restores.
            backend_mut(|_outer| {
                let mut inner = NullBackend;
                with_backend(&mut inner, || {
                    assert!(backend_call(|b| b.players_online()).is_err());
                });
                assert!(!has_backend(), "still lent to the outer closure");
            })
            .unwrap();
            assert_eq!(backend_call(|b| b.players_online()).unwrap()[0].lo, 2);
        });
        assert_eq!(c.0, 2);
        assert!(!has_backend());
    }

    #[test]
    fn restores_after_panic() {
        let r = std::panic::catch_unwind(|| {
            let mut b = NullBackend;
            with_backend(&mut b, || {
                with_call(
                    CallContext {
                        plugin_id: PluginId::new_unchecked("p"),
                        is_main: true,
                    },
                    || {
                        let _ = backend_mut(|_| panic!("boom"));
                    },
                )
            })
        });
        assert!(r.is_err());
        assert!(!has_backend());
        assert!(current_call().is_none());
        assert_eq!(LENT.with(Cell::get), 0);
        // And the slot works again.
        let mut b = NullBackend;
        with_backend(&mut b, || {
            assert!(backend_mut(|b| b.console_message(&Text::plain("x"))).is_ok())
        });
    }

    #[test]
    fn call_stack() {
        assert!(current_call().is_none());
        with_call(
            CallContext {
                plugin_id: PluginId::new_unchecked("a"),
                is_main: true,
            },
            || {
                with_call(
                    CallContext {
                        plugin_id: PluginId::new_unchecked("b"),
                        is_main: false,
                    },
                    || {
                        assert_eq!(current_call().unwrap().plugin_id, "b");
                        assert!(is_executing("a") && is_executing("b"));
                        assert_eq!(call_depth(), 2);
                        assert!(api().is_err(), "async context has no main API");
                    },
                );
                assert_eq!(current_call().unwrap().plugin_id, "a");
                assert!(runtime().is_err(), "no runtime entered");
            },
        );
        assert_eq!(call_depth(), 0);
    }
}
