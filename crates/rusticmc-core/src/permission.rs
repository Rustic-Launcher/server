//! Permission checks.
//!
//! `has(sender, node)`:
//!
//! 1. The console has every permission.
//! 2. **Explicit rules**: from the permission provider plugin if one provides
//!    `rustic:services/permission-provider@0.1.0` (through the host's [`PermissionProvider`]),
//!    otherwise from the default engine (`rusticmc-permissions`, `permissions.toml`). If the
//!    provider fails for a lookup, the default engine answers that lookup instead.
//! 3. **Declared parents**: if a declared node lists `node` among its children and the player
//!    has that parent (recursively, through this same resolution), the child's value applies.
//! 4. **Declared default** of the node (`allow`, `deny`, `op`, `not-op`).
//! 5. Undeclared nodes default to `op`.
//!
//! Wildcards (`homes.*`, `*`) are handled by the explicit rule source.

use crate::id::PluginId;
use crate::model::{PermissionDefault, ServiceError, Tristate, Uuid};
use rusticmc_permissions::{PermissionEngine, validate_node};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// A permission declared by a plugin (`permissions.declare`) or the core.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeclaredPermission {
    /// The node.
    pub node: String,
    /// Default for players without a rule.
    pub default: PermissionDefault,
    /// Description.
    pub description: String,
    /// Implied nodes: granted (`true`) or negated (`false`) when this node is held.
    pub children: Vec<(String, bool)>,
    /// Who declared it.
    pub owner: PluginId,
}

/// Every declared node with the reverse child → parents index.
#[derive(Clone, Debug, Default)]
pub struct DeclaredTable {
    nodes: HashMap<String, DeclaredPermission>,
    parents: HashMap<String, Vec<String>>,
}

impl DeclaredTable {
    fn reindex(&mut self) {
        let mut parents: HashMap<String, Vec<String>> = HashMap::new();
        for (node, d) in &self.nodes {
            for (child, _) in &d.children {
                parents.entry(child.clone()).or_default().push(node.clone());
            }
        }
        for v in parents.values_mut() {
            v.sort();
        }
        self.parents = parents;
    }

    /// A declared node.
    pub fn get(&self, node: &str) -> Option<&DeclaredPermission> {
        self.nodes.get(node)
    }
}

/// Implemented by the host when a plugin provides the permission-provider service: it calls
/// the provider plugin's `check` export (through [`crate::Rustic::with_plugin_instance`]).
pub trait PermissionProvider {
    /// `permission-provider.check`.
    fn check(
        &mut self,
        provider: &PluginId,
        player: Uuid,
        node: &str,
    ) -> Result<Tristate, ServiceError>;
}

/// Declared nodes plus the default engine.
pub struct PermissionService {
    declared: Rc<DeclaredTable>,
    engine: PermissionEngine,
}

impl std::fmt::Debug for PermissionService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PermissionService")
            .field("declared", &self.declared.nodes.len())
            .finish_non_exhaustive()
    }
}

/// Normalise and validate a node (lowercase).
pub fn normalize_node(node: &str) -> Result<String, String> {
    let n = node.trim().to_ascii_lowercase();
    validate_node(&n)?;
    Ok(n)
}

impl PermissionService {
    /// Over a default engine.
    pub fn new(engine: PermissionEngine) -> Self {
        Self {
            declared: Rc::new(DeclaredTable::default()),
            engine,
        }
    }

    /// `permissions.declare`. Re-declaring one's own node replaces it; another plugin's node
    /// is refused.
    pub fn declare(
        &mut self,
        owner: &PluginId,
        node: &str,
        default: PermissionDefault,
        description: &str,
        children: &[(String, bool)],
    ) -> Result<(), String> {
        let node = normalize_node(node)?;
        if node.ends_with('*') {
            return Err(format!("`{node}`: wildcards cannot be declared"));
        }
        let mut kids = Vec::new();
        for (c, v) in children {
            let c = normalize_node(c)?;
            if c == node {
                return Err(format!("`{node}` cannot be its own child"));
            }
            kids.push((c, *v));
        }
        if let Some(existing) = self.declared.nodes.get(&node)
            && existing.owner != *owner
        {
            return Err(format!(
                "`{node}` is already declared by `{}`",
                existing.owner
            ));
        }
        let table = Rc::make_mut(&mut self.declared);
        table.nodes.insert(
            node.clone(),
            DeclaredPermission {
                node,
                default,
                description: description.to_owned(),
                children: kids,
                owner: owner.clone(),
            },
        );
        table.reindex();
        Ok(())
    }

    /// Forget every node a plugin declared; returns how many.
    pub fn remove_plugin(&mut self, owner: &str) -> usize {
        if !self.declared.nodes.values().any(|d| &*d.owner == owner) {
            return 0;
        }
        let table = Rc::make_mut(&mut self.declared);
        let before = table.nodes.len();
        table.nodes.retain(|_, d| &*d.owner != owner);
        table.reindex();
        before - table.nodes.len()
    }

    /// A declared node.
    pub fn declared(&self, node: &str) -> Option<DeclaredPermission> {
        self.declared.nodes.get(node).cloned()
    }

    /// Every declared node, sorted.
    pub fn all_declared(&self) -> Vec<DeclaredPermission> {
        let mut v: Vec<_> = self.declared.nodes.values().cloned().collect();
        v.sort_by(|a, b| a.node.cmp(&b.node));
        v
    }

    /// Snapshot of the declared table (cheap clone).
    pub fn table(&self) -> Rc<DeclaredTable> {
        self.declared.clone()
    }

    /// The default engine.
    pub fn engine(&self) -> &PermissionEngine {
        &self.engine
    }

    /// The default engine, mutable (permission commands, the perms plugin's fallback).
    pub fn engine_mut(&mut self) -> &mut PermissionEngine {
        &mut self.engine
    }

    /// Replace the default engine (reload of `permissions.toml`).
    pub fn set_engine(&mut self, engine: PermissionEngine) {
        self.engine = engine;
    }
}

/// Resolve `node` for a player (steps 2 to 5 of the module docs); `explicit` answers step 2.
pub fn resolve(
    table: &DeclaredTable,
    node: &str,
    is_op: bool,
    explicit: &mut dyn FnMut(&str) -> Tristate,
) -> bool {
    fn go(
        table: &DeclaredTable,
        node: &str,
        is_op: bool,
        explicit: &mut dyn FnMut(&str) -> Tristate,
        visiting: &mut HashSet<String>,
    ) -> bool {
        if let Some(v) = explicit(node).as_bool() {
            return v;
        }
        if visiting.len() < 32 && visiting.insert(node.to_owned()) {
            if let Some(parents) = table.parents.get(node) {
                for p in parents {
                    if visiting.contains(p) {
                        continue;
                    }
                    if go(table, p, is_op, explicit, visiting)
                        && let Some(value) = table
                            .nodes
                            .get(p)
                            .and_then(|d| d.children.iter().find(|(c, _)| c == node))
                            .map(|(_, v)| *v)
                    {
                        visiting.remove(node);
                        return value;
                    }
                }
            }
            visiting.remove(node);
        }
        table
            .nodes
            .get(node)
            .map(|d| d.default)
            .unwrap_or(PermissionDefault::Op)
            .evaluate(is_op)
    }
    let lowered;
    let node = if node.bytes().any(|b| b.is_ascii_uppercase()) {
        lowered = node.to_ascii_lowercase();
        lowered.as_str()
    } else {
        node
    };
    go(table, node, is_op, explicit, &mut HashSet::new())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner() -> PluginId {
        PluginId::new_unchecked("homes")
    }

    fn engine(text: &str) -> PermissionEngine {
        PermissionEngine::parse(text).unwrap()
    }

    #[test]
    fn declare_rules() {
        let mut s = PermissionService::new(PermissionEngine::default());
        s.declare(&owner(), "Homes.Home", PermissionDefault::Allow, "d", &[])
            .unwrap();
        assert!(s.declared("homes.home").is_some());
        assert!(
            s.declare(&owner(), "homes.*", PermissionDefault::Op, "", &[])
                .is_err()
        );
        assert!(
            s.declare(&owner(), "a..b", PermissionDefault::Op, "", &[])
                .is_err()
        );
        assert!(
            s.declare(
                &owner(),
                "x",
                PermissionDefault::Op,
                "",
                &[("x".into(), true)]
            )
            .is_err()
        );
        let other = PluginId::new_unchecked("other");
        assert!(
            s.declare(&other, "homes.home", PermissionDefault::Deny, "", &[])
                .is_err()
        );
        s.declare(
            &owner(),
            "homes.home",
            PermissionDefault::Deny,
            "again",
            &[],
        )
        .unwrap();
        assert_eq!(
            s.declared("homes.home").unwrap().default,
            PermissionDefault::Deny
        );
        assert_eq!(s.remove_plugin("homes"), 1);
        assert_eq!(s.remove_plugin("homes"), 0);
        assert!(s.all_declared().is_empty());
    }

    #[test]
    fn resolution_order() {
        let mut s = PermissionService::new(engine(
            r#"
[groups.default]
nodes = { "homes.denied" = false, "homes.admin" = true }
"#,
        ));
        let o = owner();
        s.declare(&o, "homes.home", PermissionDefault::Allow, "", &[])
            .unwrap();
        s.declare(&o, "homes.denied", PermissionDefault::Allow, "", &[])
            .unwrap();
        s.declare(&o, "homes.secret", PermissionDefault::Op, "", &[])
            .unwrap();
        s.declare(&o, "homes.notop", PermissionDefault::NotOp, "", &[])
            .unwrap();
        s.declare(
            &o,
            "homes.admin",
            PermissionDefault::Op,
            "",
            &[("homes.other".into(), true), ("homes.home".into(), false)],
        )
        .unwrap();
        let table = s.table();
        let player = 7u128;
        let mut ex = |n: &str| s.engine().check(player, n);
        // Declared default.
        assert!(resolve(&table, "homes.secret", true, &mut ex));
        assert!(!resolve(&table, "homes.secret", false, &mut ex));
        assert!(resolve(&table, "homes.notop", false, &mut ex));
        // Explicit rule beats declared default.
        assert!(!resolve(&table, "homes.denied", true, &mut ex));
        // Parent held (explicitly) → children apply, even negations.
        assert!(resolve(&table, "homes.other", false, &mut ex));
        assert!(!resolve(&table, "homes.home", false, &mut ex));
        // Undeclared → op.
        assert!(resolve(&table, "x.y", true, &mut ex));
        assert!(!resolve(&table, "X.Y", false, &mut ex));
    }

    #[test]
    fn parent_by_default_and_cycles() {
        let mut s = PermissionService::new(PermissionEngine::default());
        let o = owner();
        s.declare(&o, "a", PermissionDefault::Op, "", &[("b".into(), true)])
            .unwrap();
        s.declare(&o, "b", PermissionDefault::Deny, "", &[("a".into(), true)])
            .unwrap();
        let table = s.table();
        let mut none = |_: &str| Tristate::Undefined;
        // Op holds `a` by default, so `b` (child of a) is granted despite its deny default.
        assert!(resolve(&table, "b", true, &mut none));
        assert!(!resolve(&table, "b", false, &mut none));
        // The a <-> b loop terminates.
        assert!(resolve(&table, "a", true, &mut none));
        assert!(!resolve(&table, "a", false, &mut none));
    }
}
