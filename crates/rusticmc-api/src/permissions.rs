//! Permission nodes: declare them (optional; undeclared nodes default to `op`) and check them.

use crate::__rt;
use crate::bindings::rustic::api::permissions as wit;
use crate::command::Sender;
use crate::host::call;
use crate::types::PermissionDefault;

/// Permission checks and declarations.
#[derive(Debug, Clone, Copy)]
pub struct Permissions;

impl Permissions {
    /// Whether `subject` has `node` (the console has everything).
    pub fn has(subject: impl Into<Sender>, node: &str) -> bool {
        let raw = subject.into().to_raw();
        call!("permissions.has", wit::has(raw, node))
    }

    /// Starts declaring a node: `Permissions::declare("homes.use").default(PermissionDefault::Allow).register()`.
    pub fn declare(node: impl Into<String>) -> PermissionDecl {
        PermissionDecl {
            node: node.into(),
            default: PermissionDefault::Op,
            description: String::new(),
            children: Vec::new(),
        }
    }
}

/// A permission node declaration (see [`Permissions::declare`]).
#[derive(Debug, Clone)]
#[must_use = "call .register()"]
pub struct PermissionDecl {
    node: String,
    default: PermissionDefault,
    description: String,
    children: Vec<(String, bool)>,
}

impl PermissionDecl {
    /// Who has it without an explicit rule (default `op`).
    pub fn default(mut self, d: PermissionDefault) -> Self {
        self.default = d;
        self
    }

    /// A description for permission tools.
    pub fn description(mut self, d: impl Into<String>) -> Self {
        self.description = d.into();
        self
    }

    /// A node this one grants (`true`) or revokes (`false`).
    pub fn child(mut self, node: impl Into<String>, value: bool) -> Self {
        self.children.push((node.into(), value));
        self
    }

    /// Sends the declaration (no-op in the async instance).
    pub fn register(self) {
        if !__rt::is_main() {
            return;
        }
        call!(
            "permissions.declare",
            wit::declare(&self.node, self.default, &self.description, &self.children)
        );
    }
}
