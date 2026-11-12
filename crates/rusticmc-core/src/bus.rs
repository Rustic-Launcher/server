//! The event bus: subscriptions per event, sorted by priority then registration order.
//!
//! Listener lists are immutable snapshots (`Rc<[Listener]>`): a dispatch clones the `Rc` and
//! iterates it, so subscribing or unsubscribing during a dispatch only affects the next one.
//! Built-in events are indexed by [`EventKind`] in a dense array, so
//! [`EventBus::has_listeners`] is one array load and a length check.

use crate::id::PluginId;
use crate::lifecycle::PluginSlot;
use crate::model::{EventKey, EventKind, HandlerId, Priority};
use std::collections::HashMap;
use std::rc::Rc;

/// One subscription.
#[derive(Clone)]
pub(crate) struct Listener {
    pub id: HandlerId,
    pub slot: Rc<PluginSlot>,
    pub priority: Priority,
    pub ignore_cancelled: bool,
    pub seq: u64,
}

impl std::fmt::Debug for Listener {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Listener")
            .field("id", &self.id)
            .field("plugin", &self.slot.id)
            .field("priority", &self.priority)
            .field("ignore_cancelled", &self.ignore_cancelled)
            .finish()
    }
}

/// A read-only view of a subscription (for tooling and tests).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListenerInfo {
    /// Handler id.
    pub id: HandlerId,
    /// Plugin.
    pub plugin: PluginId,
    /// Priority.
    pub priority: Priority,
    /// Skips cancelled events.
    pub ignore_cancelled: bool,
}

pub(crate) type ListenerList = Rc<[Listener]>;

/// The bus.
pub(crate) struct EventBus {
    builtin: Vec<ListenerList>,
    custom: HashMap<String, ListenerList>,
    index: HashMap<HandlerId, EventKey>,
    seq: u64,
}

impl Default for EventBus {
    fn default() -> Self {
        Self {
            builtin: (0..EventKind::COUNT)
                .map(|_| Rc::from(Vec::new()))
                .collect(),
            custom: HashMap::new(),
            index: HashMap::new(),
            seq: 0,
        }
    }
}

fn sorted(mut v: Vec<Listener>) -> ListenerList {
    v.sort_by_key(|l| (l.priority, l.seq));
    Rc::from(v)
}

impl EventBus {
    /// Whether any handler listens to a built-in event.
    #[inline]
    pub fn has_listeners(&self, kind: EventKind) -> bool {
        !self.builtin[kind.index()].is_empty()
    }

    /// Whether any handler listens to a custom event.
    pub fn has_custom_listeners(&self, name: &str) -> bool {
        self.custom.get(name).is_some_and(|l| !l.is_empty())
    }

    /// The current snapshot for a built-in event (empty slice if none).
    #[inline]
    pub fn builtin(&self, kind: EventKind) -> &ListenerList {
        &self.builtin[kind.index()]
    }

    /// The current snapshot for a custom event.
    pub fn custom(&self, name: &str) -> Option<&ListenerList> {
        self.custom.get(name)
    }

    fn list_mut(&mut self, key: &EventKey) -> Option<&mut ListenerList> {
        match key {
            EventKey::Builtin(k) => self.builtin.get_mut(k.index()),
            EventKey::Custom(n) => self.custom.get_mut(n),
        }
    }

    /// Add a subscription.
    pub fn subscribe(
        &mut self,
        key: EventKey,
        id: HandlerId,
        slot: Rc<PluginSlot>,
        priority: Priority,
        ignore_cancelled: bool,
    ) {
        self.seq += 1;
        let l = Listener {
            id,
            slot,
            priority,
            ignore_cancelled,
            seq: self.seq,
        };
        let current: Vec<Listener> = match &key {
            EventKey::Builtin(k) => self.builtin[k.index()].to_vec(),
            EventKey::Custom(n) => self.custom.get(n).map(|l| l.to_vec()).unwrap_or_default(),
        };
        let mut v = current;
        v.push(l);
        let list = sorted(v);
        match &key {
            EventKey::Builtin(k) => self.builtin[k.index()] = list,
            EventKey::Custom(n) => {
                self.custom.insert(n.clone(), list);
            }
        }
        self.index.insert(id, key);
    }

    /// Remove one subscription, only if it belongs to `plugin`. Returns whether it existed.
    pub fn unsubscribe(&mut self, plugin: &str, id: HandlerId) -> bool {
        let Some(key) = self.index.get(&id).cloned() else {
            return false;
        };
        let Some(list) = self.list_mut(&key) else {
            return false;
        };
        if !list.iter().any(|l| l.id == id && &*l.slot.id == plugin) {
            return false;
        }
        let v: Vec<Listener> = list.iter().filter(|l| l.id != id).cloned().collect();
        let empty = v.is_empty();
        *list = Rc::from(v);
        if empty && let EventKey::Custom(n) = &key {
            self.custom.remove(n);
        }
        self.index.remove(&id);
        true
    }

    /// Remove every subscription of a plugin; returns how many.
    pub fn remove_plugin(&mut self, plugin: &str) -> usize {
        let mut removed = 0;
        let mut filter = |list: &mut ListenerList| {
            if list.iter().any(|l| &*l.slot.id == plugin) {
                let v: Vec<Listener> = list
                    .iter()
                    .filter(|l| &*l.slot.id != plugin)
                    .cloned()
                    .collect();
                removed += list.len() - v.len();
                *list = Rc::from(v);
            }
        };
        for list in &mut self.builtin {
            filter(list);
        }
        for list in self.custom.values_mut() {
            filter(list);
        }
        self.custom.retain(|_, l| !l.is_empty());
        let builtin = &self.builtin;
        let custom = &self.custom;
        self.index.retain(|id, key| match key {
            EventKey::Builtin(k) => builtin[k.index()].iter().any(|l| l.id == *id),
            EventKey::Custom(n) => custom.get(n).is_some_and(|l| l.iter().any(|x| x.id == *id)),
        });
        removed
    }

    /// Read-only view of the subscriptions of an event.
    pub fn listeners(&self, key: &EventKey) -> Vec<ListenerInfo> {
        let list = match key {
            EventKey::Builtin(k) => Some(&self.builtin[k.index()]),
            EventKey::Custom(n) => self.custom.get(n),
        };
        list.map(|l| {
            l.iter()
                .map(|x| ListenerInfo {
                    id: x.id,
                    plugin: x.slot.id.clone(),
                    priority: x.priority,
                    ignore_cancelled: x.ignore_cancelled,
                })
                .collect()
        })
        .unwrap_or_default()
    }

    /// Number of subscriptions of a plugin.
    pub fn count_for(&self, plugin: &str) -> usize {
        self.builtin
            .iter()
            .chain(self.custom.values())
            .flat_map(|l| l.iter())
            .filter(|l| &*l.slot.id == plugin)
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot(id: &str) -> Rc<PluginSlot> {
        Rc::new(PluginSlot::new(PluginId::new_unchecked(id), 0))
    }

    #[test]
    fn ordering_snapshot_and_removal() {
        let mut bus = EventBus::default();
        let a = slot("a");
        let b = slot("b");
        let join = EventKey::Builtin(EventKind::PlayerJoin);
        assert!(!bus.has_listeners(EventKind::PlayerJoin));
        bus.subscribe(join.clone(), 1, a.clone(), Priority::Monitor, false);
        bus.subscribe(join.clone(), 2, b.clone(), Priority::Normal, false);
        bus.subscribe(join.clone(), 3, a.clone(), Priority::Lowest, true);
        bus.subscribe(join.clone(), 4, b.clone(), Priority::Normal, false);
        assert!(bus.has_listeners(EventKind::PlayerJoin));
        let snap = bus.builtin(EventKind::PlayerJoin).clone();
        let order: Vec<_> = snap.iter().map(|l| l.id).collect();
        assert_eq!(order, [3, 2, 4, 1]);
        // Unsubscribing does not change the snapshot.
        assert!(!bus.unsubscribe("a", 2), "not a's handler");
        assert!(bus.unsubscribe("b", 2));
        assert!(!bus.unsubscribe("b", 2));
        assert_eq!(snap.len(), 4);
        assert_eq!(bus.builtin(EventKind::PlayerJoin).len(), 3);
        bus.subscribe(
            EventKey::Custom("x:y".into()),
            5,
            a.clone(),
            Priority::High,
            false,
        );
        assert!(bus.has_custom_listeners("x:y"));
        assert_eq!(bus.count_for("a"), 3);
        assert_eq!(bus.remove_plugin("a"), 3);
        assert!(!bus.has_custom_listeners("x:y"));
        assert_eq!(bus.listeners(&join).len(), 1);
        assert_eq!(bus.listeners(&join)[0].plugin, "b");
        assert!(!bus.unsubscribe("a", 5));
        assert!(bus.unsubscribe("b", 4));
        assert!(!bus.has_listeners(EventKind::PlayerJoin));
    }
}
