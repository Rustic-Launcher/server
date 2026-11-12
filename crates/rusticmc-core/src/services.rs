//! The service registry: which plugin provides which WIT interface.
//!
//! A provider declares `services.provide(iface, priority)` (usually in `on-load`); consumers
//! (through the host's linker) ask [`ServiceRegistry::provider`]. The highest priority wins,
//! ties go to the first registration. Every change of the effective provider of an interface
//! is reported to the listeners registered with [`ServiceRegistry::on_change`] so the host can
//! re-link consumers. Thread-safe (consumers in async instances look providers up too).

use crate::id::PluginId;
use crate::model::Priority;
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, PoisonError, RwLock};

/// The standard permission provider interface.
pub const PERMISSION_PROVIDER_IFACE: &str = "rustic:services/permission-provider@0.1.0";
/// The standard economy interface.
pub const ECONOMY_IFACE: &str = "rustic:services/economy@0.1.0";
/// The standard chat metadata interface.
pub const CHAT_META_IFACE: &str = "rustic:services/chat-meta@0.1.0";

/// A change of the effective provider of an interface.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ServiceChange {
    /// The interface.
    pub iface: String,
    /// Previous provider.
    pub old: Option<PluginId>,
    /// New provider.
    pub new: Option<PluginId>,
}

#[derive(Clone, Debug)]
struct Entry {
    plugin: PluginId,
    priority: Priority,
    seq: u64,
}

type Listener = Arc<dyn Fn(&ServiceChange) + Send + Sync>;

/// The registry.
#[derive(Default)]
pub struct ServiceRegistry {
    entries: RwLock<BTreeMap<String, Vec<Entry>>>,
    listeners: RwLock<Vec<Listener>>,
    seq: AtomicU64,
}

impl std::fmt::Debug for ServiceRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServiceRegistry").finish_non_exhaustive()
    }
}

/// Validate a fully qualified interface name: `namespace:package/interface@major.minor.patch`.
pub fn validate_iface(iface: &str) -> Result<(), String> {
    let err = || {
        format!(
            "`{iface}` is not a fully qualified interface like \"rustic:services/economy@0.1.0\""
        )
    };
    let (path, version) = iface.split_once('@').ok_or_else(err)?;
    semver::Version::parse(version).map_err(|_| err())?;
    let (pkg, name) = path.split_once('/').ok_or_else(err)?;
    let (ns, pkgname) = pkg.split_once(':').ok_or_else(err)?;
    let ident = |s: &str| {
        !s.is_empty()
            && s.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            && !s.starts_with('-')
    };
    if ident(ns) && ident(pkgname) && ident(name) {
        Ok(())
    } else {
        Err(err())
    }
}

fn best(list: &[Entry]) -> Option<&Entry> {
    list.iter()
        .max_by(|a, b| a.priority.cmp(&b.priority).then_with(|| b.seq.cmp(&a.seq)))
}

impl ServiceRegistry {
    /// Empty.
    pub fn new() -> Self {
        Self::default()
    }

    fn notify(&self, changes: &[ServiceChange]) {
        if changes.is_empty() {
            return;
        }
        let listeners: Vec<Listener> = self
            .listeners
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .clone();
        for c in changes {
            log::info!(
                target: "rustic",
                "service {}: provider {} -> {}",
                c.iface,
                c.old.as_deref().unwrap_or("none"),
                c.new.as_deref().unwrap_or("none")
            );
            for l in &listeners {
                l(c);
            }
        }
    }

    /// Register a change listener.
    pub fn on_change(&self, f: impl Fn(&ServiceChange) + Send + Sync + 'static) {
        self.listeners
            .write()
            .unwrap_or_else(PoisonError::into_inner)
            .push(Arc::new(f));
    }

    /// `services.provide`. Providing again updates the priority.
    pub fn provide(
        &self,
        plugin: &PluginId,
        iface: &str,
        priority: Priority,
    ) -> Result<(), String> {
        validate_iface(iface)?;
        let change = {
            let mut g = self.entries.write().unwrap_or_else(PoisonError::into_inner);
            let list = g.entry(iface.to_owned()).or_default();
            let old = best(list).map(|e| e.plugin.clone());
            if let Some(e) = list.iter_mut().find(|e| e.plugin == *plugin) {
                e.priority = priority;
            } else {
                list.push(Entry {
                    plugin: plugin.clone(),
                    priority,
                    seq: self.seq.fetch_add(1, Ordering::Relaxed),
                });
            }
            let new = best(list).map(|e| e.plugin.clone());
            (old != new).then(|| ServiceChange {
                iface: iface.to_owned(),
                old,
                new,
            })
        };
        self.notify(change.as_slice());
        Ok(())
    }

    /// `services.provider`: the plugin currently providing `iface`.
    pub fn provider(&self, iface: &str) -> Option<PluginId> {
        let g = self.entries.read().unwrap_or_else(PoisonError::into_inner);
        g.get(iface).and_then(|l| best(l)).map(|e| e.plugin.clone())
    }

    /// Every provider of `iface`, best first.
    pub fn providers(&self, iface: &str) -> Vec<(PluginId, Priority)> {
        let g = self.entries.read().unwrap_or_else(PoisonError::into_inner);
        let mut list = g.get(iface).cloned().unwrap_or_default();
        list.sort_by(|a, b| b.priority.cmp(&a.priority).then_with(|| a.seq.cmp(&b.seq)));
        list.into_iter().map(|e| (e.plugin, e.priority)).collect()
    }

    /// Interfaces a plugin provides.
    pub fn provided_by(&self, plugin: &str) -> Vec<String> {
        let g = self.entries.read().unwrap_or_else(PoisonError::into_inner);
        g.iter()
            .filter(|(_, l)| l.iter().any(|e| &*e.plugin == plugin))
            .map(|(k, _)| k.clone())
            .collect()
    }

    /// Withdraw one interface of a plugin.
    pub fn withdraw(&self, plugin: &str, iface: &str) -> bool {
        let (found, change) = {
            let mut g = self.entries.write().unwrap_or_else(PoisonError::into_inner);
            let Some(list) = g.get_mut(iface) else {
                return false;
            };
            let old = best(list).map(|e| e.plugin.clone());
            let before = list.len();
            list.retain(|e| &*e.plugin != plugin);
            let found = list.len() != before;
            let new = best(list).map(|e| e.plugin.clone());
            if list.is_empty() {
                g.remove(iface);
            }
            (
                found,
                (old != new).then(|| ServiceChange {
                    iface: iface.to_owned(),
                    old,
                    new,
                }),
            )
        };
        self.notify(change.as_slice());
        found
    }

    /// Withdraw everything a plugin provides (on disable); returns the changes notified.
    pub fn remove_plugin(&self, plugin: &str) -> Vec<ServiceChange> {
        let changes: Vec<ServiceChange> = {
            let mut g = self.entries.write().unwrap_or_else(PoisonError::into_inner);
            let mut changes = Vec::new();
            g.retain(|iface, list| {
                let old = best(list).map(|e| e.plugin.clone());
                list.retain(|e| &*e.plugin != plugin);
                let new = best(list).map(|e| e.plugin.clone());
                if old != new {
                    changes.push(ServiceChange {
                        iface: iface.clone(),
                        old,
                        new,
                    });
                }
                !list.is_empty()
            });
            changes
        };
        self.notify(&changes);
        changes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn p(s: &str) -> PluginId {
        PluginId::new_unchecked(s)
    }

    #[test]
    fn iface_names() {
        assert!(validate_iface(ECONOMY_IFACE).is_ok());
        assert!(validate_iface("acme:shop/prices@1.2.3").is_ok());
        assert!(validate_iface("economy").is_err());
        assert!(validate_iface("rustic:services/economy").is_err());
        assert!(validate_iface("rustic:services/Economy@0.1.0").is_err());
        assert!(validate_iface("rustic/economy@0.1.0").is_err());
    }

    #[test]
    fn priorities_ties_and_notifications() {
        let r = ServiceRegistry::new();
        let log: Arc<Mutex<Vec<ServiceChange>>> = Arc::default();
        let l2 = log.clone();
        r.on_change(move |c| l2.lock().unwrap().push(c.clone()));
        r.provide(&p("a"), ECONOMY_IFACE, Priority::Normal).unwrap();
        r.provide(&p("b"), ECONOMY_IFACE, Priority::Normal).unwrap(); // tie: a stays
        assert_eq!(r.provider(ECONOMY_IFACE), Some(p("a")));
        r.provide(&p("c"), ECONOMY_IFACE, Priority::High).unwrap();
        assert_eq!(r.provider(ECONOMY_IFACE), Some(p("c")));
        assert_eq!(
            r.providers(ECONOMY_IFACE)
                .into_iter()
                .map(|x| x.0)
                .collect::<Vec<_>>(),
            vec![p("c"), p("a"), p("b")]
        );
        assert!(r.provide(&p("a"), "bad", Priority::Low).is_err());
        assert_eq!(r.provided_by("a"), vec![ECONOMY_IFACE.to_owned()]);
        let changes = r.remove_plugin("c");
        assert_eq!(changes.len(), 1);
        assert_eq!(r.provider(ECONOMY_IFACE), Some(p("a")));
        assert!(r.withdraw("a", ECONOMY_IFACE));
        assert!(!r.withdraw("a", ECONOMY_IFACE));
        assert_eq!(r.provider(ECONOMY_IFACE), Some(p("b")));
        r.remove_plugin("b");
        assert_eq!(r.provider(ECONOMY_IFACE), None);
        let log = log.lock().unwrap();
        let seq: Vec<_> = log
            .iter()
            .map(|c| {
                (
                    c.old.as_deref().map(str::to_owned),
                    c.new.as_deref().map(str::to_owned),
                )
            })
            .collect();
        assert_eq!(
            seq,
            vec![
                (None, Some("a".into())),
                (Some("a".into()), Some("c".into())),
                (Some("c".into()), Some("a".into())),
                (Some("a".into()), Some("b".into())),
                (Some("b".into()), None),
            ]
        );
    }
}
