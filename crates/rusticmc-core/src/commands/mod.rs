//! Commands: a registry of typed command trees built from `command-spec` usages, a parser,
//! tab completion and a neutral export for the engine adapter.
//!
//! * **Names**: the first plugin to register a name keeps the bare name; every command is also
//!   reachable as `<plugin-id>:<name>` (and `<plugin-id>:<alias>`). Names beat aliases. When a
//!   command goes away, bare labels are re-assigned to the next claimant in registration order.
//! * **Tree**: the usages of a command are merged into one tree of literal and argument nodes.
//!   Parsing walks it depth first (with backtracking), so ambiguous usages resolve to the first
//!   usage that consumes the whole line.
//! * **Permissions**: a usage's permission overrides the command's; the runtime checks them on
//!   the parsed matches (see [`crate::Rustic::dispatch_command`]).

pub mod parse;

use crate::id::PluginId;
use crate::model::{
    ArgKind, ArgSpec, ArgValue, CommandId, CommandSender, CommandSpec, NodeSpec, Usage,
};
use parse::{ArgError, Resolver, parse_arg, suggest, token};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

/// Max usages per command, nodes per usage, suggestions per argument.
pub const MAX_USAGES: usize = 64;
/// Max nodes in one usage.
pub const MAX_NODES: usize = 32;
/// Max fixed suggestions of one argument.
pub const MAX_SUGGESTIONS: usize = 256;

/// The kind of a tree node.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NodeKind {
    /// The command itself.
    Root,
    /// A fixed word.
    Literal(String),
    /// A typed argument.
    Argument(ArgSpec),
}

/// A node of a command tree.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CommandNode {
    /// What it is.
    pub kind: NodeKind,
    /// Children, in first-registration order.
    pub children: Vec<CommandNode>,
    /// The usage that ends here, if any.
    pub executes: Option<u32>,
    /// Every usage passing through this node.
    pub usages: Vec<u32>,
}

impl CommandNode {
    fn new(kind: NodeKind) -> Self {
        Self {
            kind,
            children: Vec::new(),
            executes: None,
            usages: Vec::new(),
        }
    }

    fn same_as(&self, spec: &NodeSpec) -> bool {
        match (&self.kind, spec) {
            (NodeKind::Literal(a), NodeSpec::Literal(b)) => a.eq_ignore_ascii_case(b),
            (NodeKind::Argument(a), NodeSpec::Argument(b)) => a == b,
            _ => false,
        }
    }
}

/// A registered command.
#[derive(Clone, Debug)]
pub struct RegisteredCommand {
    /// Id given to the plugin.
    pub id: CommandId,
    /// Owner (`rustic` for built-ins).
    pub owner: PluginId,
    /// The spec as registered (name and aliases normalised).
    pub spec: CommandSpec,
    /// The merged tree.
    pub root: CommandNode,
    /// Labels currently bound to this command (bare and namespaced).
    pub labels: Vec<String>,
}

impl RegisteredCommand {
    /// Effective permission of a usage (its own, else the command's).
    pub fn usage_permission(&self, usage: u32) -> Option<&str> {
        self.spec
            .usages
            .get(usage as usize)
            .and_then(|u| u.permission.as_deref())
            .or(self.spec.permission.as_deref())
    }

    /// `/label a <b> [c]` style line for a usage.
    pub fn usage_line(&self, label: &str, usage: u32) -> String {
        let mut s = format!("/{label}");
        if let Some(u) = self.spec.usages.get(usage as usize) {
            for n in &u.nodes {
                s.push(' ');
                match n {
                    NodeSpec::Literal(l) => s.push_str(l),
                    NodeSpec::Argument(a) => {
                        s.push('<');
                        s.push_str(&a.name);
                        s.push('>');
                    }
                }
            }
        }
        s
    }
}

/// One way the line parses.
#[derive(Clone, Debug, PartialEq)]
pub struct ParsedMatch {
    /// Usage index.
    pub usage: u32,
    /// Arguments in order.
    pub args: Vec<(String, ArgValue)>,
    /// Literal nodes matched: a parse through more literals is more specific and wins
    /// (`/home fail` runs the `fail` usage, not `<name>` = "fail").
    pub literals: u32,
}

/// Result of parsing a command line.
#[derive(Clone, Debug, PartialEq)]
pub enum ParseOutcome {
    /// No command with that label.
    Unknown,
    /// The command exists; full matches in usage-priority order (may be empty: then `error`
    /// says why).
    Command {
        /// Command id.
        id: CommandId,
        /// The label typed (lowercased).
        label: String,
        /// Matches.
        matches: Vec<ParsedMatch>,
        /// The most relevant error when nothing matched.
        error: Option<String>,
    },
}

/// A suggestion candidate with the usages it belongs to (for permission filtering).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    /// Text.
    pub text: String,
    /// Usages through the node it comes from (empty: the command itself).
    pub usages: Vec<u32>,
}

/// An argument whose completions the owning plugin must provide.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DynamicRequest {
    /// Argument name.
    pub argument: String,
    /// Usages through it.
    pub usages: Vec<u32>,
}

/// What completing a line needs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CompletionPlan {
    /// The command, if the label part is complete.
    pub command: Option<CommandId>,
    /// The token being completed.
    pub partial: String,
    /// Static candidates (labels when completing the command name).
    pub candidates: Vec<Candidate>,
    /// Dynamic arguments to ask the plugin about.
    pub dynamic: Vec<DynamicRequest>,
    /// Commands whose label is being completed (label, id).
    pub labels: Vec<(String, CommandId)>,
}

/// Neutral export of a node.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExportNode {
    /// `root`, `literal` or `argument`.
    pub kind: ExportKind,
    /// Literal text, argument name or command name.
    pub name: String,
    /// Children.
    pub children: Vec<ExportNode>,
    /// The permission every usage through this node requires (`None` when there is none or
    /// they differ; check [`ExportNode::usages`] then).
    pub permission: Option<String>,
    /// Whether a usage ends here.
    pub executable: bool,
    /// Usages through this node.
    pub usages: Vec<u32>,
}

/// Kind of an exported node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ExportKind {
    /// The command.
    Root,
    /// A literal.
    Literal,
    /// An argument of this kind.
    Argument(ArgKind),
}

/// Neutral export of a command (for the engine adapter and the test harness).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CommandTreeExport {
    /// Command id.
    pub id: CommandId,
    /// Owner plugin.
    pub owner: String,
    /// Name.
    pub name: String,
    /// Aliases.
    pub aliases: Vec<String>,
    /// Labels bound right now (bare ones the command won, plus namespaced ones).
    pub labels: Vec<String>,
    /// Description.
    pub description: String,
    /// Command permission.
    pub permission: Option<String>,
    /// Refuses the console.
    pub players_only: bool,
    /// The tree.
    pub root: ExportNode,
}

/// One usage line in Horizon's `register_commands` syntax.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HorizonUsage {
    /// The label (first word of `usage`).
    pub label: String,
    /// `"pay <player:player> <amount:integer>"`.
    pub usage: String,
    /// Effective permission of the usage.
    pub permission: Option<String>,
    /// Description.
    pub description: String,
}

/// Horizon argument kind for a Rustic one: what the player's client parses, completes and colours
/// (the line still reaches Rustic raw, which parses it itself). Uses the kinds Rustic's Horizon
/// patch adds (`string`, `players`, `vec3`, `dimension`, bounds on `integer`/`number`); a
/// duration stays a `word`, since vanilla's `time` parser takes no minutes or hours. A trailing
/// `?` makes the client ask the server for completions (`tab_complete_v2`).
pub fn horizon_kind(spec: &ArgSpec, last: bool) -> String {
    let _ = last;
    let ask = spec.dynamic_suggestions || !spec.suggestions.is_empty();
    let asking = |base: &str| {
        if ask {
            format!("{base}?")
        } else {
            base.to_owned()
        }
    };
    match spec.kind {
        ArgKind::Word | ArgKind::Duration => asking("word"),
        ArgKind::Greedy => asking("text"),
        ArgKind::String => asking("string"),
        // Names a plugin suggests itself go through the server; otherwise the client completes
        // names and selectors.
        ArgKind::Players if ask => "word?".to_owned(),
        ArgKind::Players => "players".to_owned(),
        ArgKind::World if ask => "word?".to_owned(),
        ArgKind::World => "dimension".to_owned(),
        ArgKind::Integer => bounded("integer", spec, true),
        ArgKind::Number => bounded("number", spec, false),
        ArgKind::Boolean => "boolean".to_owned(),
        ArgKind::Player => "player".to_owned(),
        ArgKind::Item => "item".to_owned(),
        ArgKind::BlockPos => "block_pos".to_owned(),
        ArgKind::Position => "vec3".to_owned(),
    }
}

/// `integer(min,max)` / `number(min,max)` when the spec has bounds (either may be absent); an
/// integer's bounds are rounded inwards and clamped to `i32`.
fn bounded(kind: &str, spec: &ArgSpec, integer: bool) -> String {
    if spec.min.is_none() && spec.max.is_none() {
        return kind.to_owned();
    }
    let bound = |v: Option<f64>, up: bool| match v.filter(|v| v.is_finite()) {
        None => String::new(),
        Some(v) if integer => {
            let v = if up { v.ceil() } else { v.floor() };
            (v.clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32).to_string()
        }
        Some(v) => v.to_string(),
    };
    format!(
        "{kind}({},{})",
        bound(spec.min, true),
        bound(spec.max, false)
    )
}

fn valid_label(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 64
        && s.bytes()
            .all(|b| matches!(b, b'a'..=b'z' | b'0'..=b'9' | b'_' | b'-' | b'.'))
}

fn valid_arg_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 32
        && s.bytes()
            .all(|b| matches!(b, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'-'))
}

/// Validate and normalise a spec for `owner`.
pub fn normalize_spec(owner: &PluginId, mut spec: CommandSpec) -> Result<CommandSpec, String> {
    let strip = |name: &str| -> Result<String, String> {
        let lower = name.trim().to_ascii_lowercase();
        let bare = match lower.split_once(':') {
            Some((ns, rest)) if ns == owner.as_str() => rest.to_owned(),
            Some((ns, _)) => {
                return Err(format!(
                    "`{name}`: a plugin can only use its own namespace (`{owner}:`), not `{ns}:`"
                ));
            }
            None => lower,
        };
        if valid_label(&bare) {
            Ok(bare)
        } else {
            Err(format!(
                "command name `{name}` must be 1 to 64 characters of [a-z0-9_.-]"
            ))
        }
    };
    spec.name = strip(&spec.name)?;
    let mut aliases = Vec::new();
    for a in &spec.aliases {
        let a = strip(a)?;
        if a != spec.name && !aliases.contains(&a) {
            aliases.push(a);
        }
    }
    spec.aliases = aliases;
    let norm_perm = |p: &Option<String>| -> Result<Option<String>, String> {
        match p {
            None => Ok(None),
            Some(p) if p.trim().is_empty() => Ok(None),
            Some(p) => crate::permission::normalize_node(p).map(Some),
        }
    };
    spec.permission = norm_perm(&spec.permission)?;
    if spec.usages.is_empty() {
        spec.usages.push(Usage {
            nodes: Vec::new(),
            permission: None,
            description: spec.description.clone(),
        });
    }
    if spec.usages.len() > MAX_USAGES {
        return Err(format!("at most {MAX_USAGES} usages per command"));
    }
    let mut seen_paths: Vec<Vec<String>> = Vec::new();
    for (ui, u) in spec.usages.iter_mut().enumerate() {
        u.permission = norm_perm(&u.permission)?;
        if u.nodes.len() > MAX_NODES {
            return Err(format!("usage {ui}: at most {MAX_NODES} nodes"));
        }
        let mut names: Vec<&str> = Vec::new();
        let n = u.nodes.len();
        let mut path = Vec::new();
        for (i, node) in u.nodes.iter_mut().enumerate() {
            match node {
                NodeSpec::Literal(l) => {
                    *l = l.trim().to_ascii_lowercase();
                    if !valid_label(l) {
                        return Err(format!("usage {ui}: literal `{l}` must be [a-z0-9_.-]"));
                    }
                    path.push(format!("lit:{l}"));
                }
                NodeSpec::Argument(a) => {
                    if !valid_arg_name(&a.name) {
                        return Err(format!(
                            "usage {ui}: argument name `{}` must be [A-Za-z0-9_-]",
                            a.name
                        ));
                    }
                    if names.contains(&a.name.as_str()) {
                        return Err(format!("usage {ui}: two arguments are named `{}`", a.name));
                    }
                    names.push(&a.name);
                    if a.kind == ArgKind::Greedy && i + 1 != n {
                        return Err(format!(
                            "usage {ui}: greedy argument `{}` must be last",
                            a.name
                        ));
                    }
                    let numeric = matches!(
                        a.kind,
                        ArgKind::Integer | ArgKind::Number | ArgKind::Duration
                    );
                    if (a.min.is_some() || a.max.is_some()) && !numeric {
                        return Err(format!(
                            "usage {ui}: min/max only apply to integer, number and duration (`{}`)",
                            a.name
                        ));
                    }
                    if a.min.is_some_and(|v| !v.is_finite())
                        || a.max.is_some_and(|v| !v.is_finite())
                    {
                        return Err(format!("usage {ui}: bounds of `{}` must be finite", a.name));
                    }
                    if let (Some(lo), Some(hi)) = (a.min, a.max)
                        && lo > hi
                    {
                        return Err(format!("usage {ui}: min > max for `{}`", a.name));
                    }
                    if a.suggestions.len() > MAX_SUGGESTIONS {
                        return Err(format!(
                            "usage {ui}: at most {MAX_SUGGESTIONS} suggestions for `{}`",
                            a.name
                        ));
                    }
                    path.push(format!("arg:{}:{}", a.name, a.kind.name()));
                }
            }
        }
        if seen_paths.contains(&path) {
            return Err(format!("usage {ui} duplicates an earlier usage"));
        }
        seen_paths.push(path);
    }
    Ok(spec)
}

fn build_tree(spec: &CommandSpec) -> CommandNode {
    let mut root = CommandNode::new(NodeKind::Root);
    for (ui, u) in spec.usages.iter().enumerate() {
        let ui = ui as u32;
        let mut node = &mut root;
        node.usages.push(ui);
        for n in &u.nodes {
            let pos = node.children.iter().position(|c| c.same_as(n));
            let idx = match pos {
                Some(i) => i,
                None => {
                    node.children.push(CommandNode::new(match n {
                        NodeSpec::Literal(l) => NodeKind::Literal(l.clone()),
                        NodeSpec::Argument(a) => NodeKind::Argument(a.clone()),
                    }));
                    node.children.len() - 1
                }
            };
            node = &mut node.children[idx];
            node.usages.push(ui);
        }
        if node.executes.is_none() {
            node.executes = Some(ui);
        }
    }
    root
}

/// The registry.
#[derive(Debug, Default)]
pub struct CommandRegistry {
    commands: BTreeMap<CommandId, RegisteredCommand>,
    labels: HashMap<String, CommandId>,
    generation: u64,
}

/// Max full matches collected for one line.
const MAX_MATCHES: usize = 16;

struct Walk<'r> {
    sender: CommandSender,
    resolver: &'r mut dyn Resolver,
    total: usize,
    matches: Vec<ParsedMatch>,
    best_error: Option<ArgError>,
    budget: usize,
}

impl Walk<'_> {
    fn fail(&mut self, e: ArgError) {
        if self
            .best_error
            .as_ref()
            .is_none_or(|b| e.position >= b.position)
        {
            self.best_error = Some(e);
        }
    }

    /// A literal that did not match: only kept when nothing better is known.
    fn fail_weak(&mut self, e: ArgError) {
        if self
            .best_error
            .as_ref()
            .is_none_or(|b| e.position > b.position)
        {
            self.best_error = Some(e);
        }
    }

    fn walk(
        &mut self,
        node: &CommandNode,
        input: &str,
        args: &mut Vec<(String, ArgValue)>,
        literals: u32,
    ) {
        if self.matches.len() >= MAX_MATCHES || self.budget == 0 {
            return;
        }
        self.budget -= 1;
        let input = input.trim_start();
        let pos = self.total - input.len();
        if input.is_empty() {
            if let Some(u) = node.executes {
                self.matches.push(ParsedMatch {
                    usage: u,
                    args: args.clone(),
                    literals,
                });
                return;
            }
            let expected: Vec<String> = node
                .children
                .iter()
                .map(|c| match &c.kind {
                    NodeKind::Literal(l) => l.clone(),
                    NodeKind::Argument(a) => format!("<{}>", a.name),
                    NodeKind::Root => String::new(),
                })
                .collect();
            self.fail(ArgError {
                position: pos,
                message: format!("incomplete command: expected {}", expected.join(" or ")),
            });
            return;
        }
        if node.children.is_empty() {
            self.fail(ArgError {
                position: pos,
                message: format!("too many arguments: `{}`", input.trim_end()),
            });
            return;
        }
        for child in &node.children {
            match &child.kind {
                NodeKind::Literal(l) => {
                    let (tok, rest) = token(input);
                    if tok.eq_ignore_ascii_case(l) {
                        self.walk(child, rest, args, literals + 1);
                    } else {
                        self.fail_weak(ArgError {
                            position: pos,
                            message: format!("unknown argument `{tok}`"),
                        });
                    }
                }
                NodeKind::Argument(spec) => {
                    match parse_arg(spec, input, pos, &self.sender, self.resolver) {
                        Ok(alts) => {
                            for (value, rest) in alts {
                                if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
                                    continue;
                                }
                                args.push((spec.name.clone(), value));
                                self.walk(child, rest, args, literals);
                                args.pop();
                            }
                        }
                        Err(e) => self.fail(e),
                    }
                }
                NodeKind::Root => {}
            }
        }
    }
}

impl CommandRegistry {
    /// Empty.
    pub fn new() -> Self {
        Self::default()
    }

    /// Changes whenever the set of commands or labels changes.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Register a command (already normalised by [`normalize_spec`] or not: it is normalised
    /// here). Returns the labels bound.
    pub fn register(
        &mut self,
        owner: &PluginId,
        id: CommandId,
        spec: CommandSpec,
    ) -> Result<Vec<String>, String> {
        let spec = normalize_spec(owner, spec)?;
        if self.commands.values().any(|c| {
            c.owner == *owner && (c.spec.name == spec.name || c.spec.aliases.contains(&spec.name))
        }) {
            return Err(format!(
                "`{}` is already registered by this plugin",
                spec.name
            ));
        }
        let root = build_tree(&spec);
        self.commands.insert(
            id,
            RegisteredCommand {
                id,
                owner: owner.clone(),
                spec,
                root,
                labels: Vec::new(),
            },
        );
        self.rebind();
        let cmd = &self.commands[&id];
        if !cmd.labels.contains(&cmd.spec.name) {
            let holder = self
                .labels
                .get(&cmd.spec.name)
                .and_then(|h| self.commands.get(h));
            log::warn!(
                target: "rustic",
                "[{owner}] command `{}` is taken by `{}`; it is available as `/{owner}:{}`",
                cmd.spec.name,
                holder.map(|h| h.owner.as_str()).unwrap_or("?"),
                cmd.spec.name
            );
        }
        Ok(cmd.labels.clone())
    }

    /// Unregister one command (only by its owner). Returns whether it existed.
    pub fn unregister(&mut self, owner: &str, id: CommandId) -> bool {
        if self.commands.get(&id).is_some_and(|c| &*c.owner == owner) {
            self.commands.remove(&id);
            self.rebind();
            true
        } else {
            false
        }
    }

    /// Remove every command of a plugin; returns how many.
    pub fn remove_plugin(&mut self, owner: &str) -> usize {
        let before = self.commands.len();
        self.commands.retain(|_, c| &*c.owner != owner);
        let n = before - self.commands.len();
        if n > 0 {
            self.rebind();
        }
        n
    }

    fn rebind(&mut self) {
        self.generation += 1;
        let mut labels: HashMap<String, CommandId> = HashMap::new();
        for c in self.commands.values() {
            labels.insert(format!("{}:{}", c.owner, c.spec.name), c.id);
            for a in &c.spec.aliases {
                labels.insert(format!("{}:{}", c.owner, a), c.id);
            }
        }
        for c in self.commands.values() {
            labels.entry(c.spec.name.clone()).or_insert(c.id);
        }
        for c in self.commands.values() {
            for a in &c.spec.aliases {
                labels.entry(a.clone()).or_insert(c.id);
            }
        }
        for c in self.commands.values_mut() {
            c.labels.clear();
        }
        let mut sorted: Vec<(&String, &CommandId)> = labels.iter().collect();
        sorted.sort();
        for (label, id) in sorted {
            if let Some(c) = self.commands.get_mut(id) {
                c.labels.push(label.clone());
            }
        }
        self.labels = labels;
    }

    /// A command by id.
    pub fn get(&self, id: CommandId) -> Option<&RegisteredCommand> {
        self.commands.get(&id)
    }

    /// The command a label runs.
    pub fn by_label(&self, label: &str) -> Option<&RegisteredCommand> {
        let lower = label.to_ascii_lowercase();
        self.labels.get(&lower).and_then(|id| self.commands.get(id))
    }

    /// Every command, by id.
    pub fn all(&self) -> impl Iterator<Item = &RegisteredCommand> {
        self.commands.values()
    }

    /// Every bound label.
    pub fn labels(&self) -> impl Iterator<Item = (&String, &CommandId)> {
        self.labels.iter()
    }

    /// Parse a line (without slash).
    pub fn parse(
        &self,
        line: &str,
        sender: CommandSender,
        resolver: &mut dyn Resolver,
    ) -> ParseOutcome {
        let line = line.trim();
        let (label, rest) = token(line);
        let Some(cmd) = self.by_label(label) else {
            return ParseOutcome::Unknown;
        };
        let mut w = Walk {
            sender,
            resolver,
            total: line.len(),
            matches: Vec::new(),
            best_error: None,
            budget: 10_000,
        };
        let mut args = Vec::new();
        w.walk(&cmd.root, rest, &mut args, 0);
        // Ambiguous parses: more literals first (like Brigadier), then declaration order.
        w.matches.sort_by(|a, b| {
            b.literals
                .cmp(&a.literals)
                .then_with(|| a.usage.cmp(&b.usage))
        });
        let error = if w.matches.is_empty() {
            Some(
                w.best_error
                    .map(|e| e.message)
                    .unwrap_or_else(|| "invalid command".to_owned()),
            )
        } else {
            None
        };
        ParseOutcome::Command {
            id: cmd.id,
            label: label.to_ascii_lowercase(),
            matches: w.matches,
            error,
        }
    }

    /// Plan the completion of a line (without slash).
    pub fn complete(
        &self,
        line: &str,
        sender: CommandSender,
        resolver: &mut dyn Resolver,
    ) -> CompletionPlan {
        let line = line.trim_start();
        let mut plan = CompletionPlan::default();
        let Some(space) = line.find(char::is_whitespace) else {
            let partial = line.to_ascii_lowercase();
            let mut labels: Vec<(String, CommandId)> = self
                .labels
                .iter()
                .filter(|(l, _)| l.starts_with(&partial))
                .map(|(l, id)| (l.clone(), *id))
                .collect();
            labels.sort();
            plan.partial = partial;
            plan.labels = labels;
            return plan;
        };
        let label = &line[..space];
        let Some(cmd) = self.by_label(label) else {
            return plan;
        };
        plan.command = Some(cmd.id);
        let after = &line[space..];
        let (prefix, partial) = match after.rfind(char::is_whitespace) {
            Some(i) => (&after[..i], &after[i + 1..]),
            None => ("", after),
        };
        plan.partial = partial.to_owned();
        // Find every node reached exactly at the end of `prefix`.
        let mut reached: Vec<&CommandNode> = Vec::new();
        fn reach<'n>(
            node: &'n CommandNode,
            input: &str,
            sender: &CommandSender,
            r: &mut dyn Resolver,
            out: &mut Vec<&'n CommandNode>,
            budget: &mut usize,
        ) {
            if *budget == 0 {
                return;
            }
            *budget -= 1;
            let input = input.trim_start();
            if input.is_empty() {
                if !out.iter().any(|n| std::ptr::eq(*n, node)) {
                    out.push(node);
                }
                return;
            }
            for child in &node.children {
                match &child.kind {
                    NodeKind::Literal(l) => {
                        let (tok, rest) = token(input);
                        if tok.eq_ignore_ascii_case(l) {
                            reach(child, rest, sender, r, out, budget);
                        }
                    }
                    NodeKind::Argument(spec) => {
                        if let Ok(alts) = parse_arg(spec, input, 0, sender, r) {
                            for (_, rest) in alts {
                                if rest.is_empty() || rest.starts_with(char::is_whitespace) {
                                    reach(child, rest, sender, r, out, budget);
                                }
                            }
                        }
                    }
                    NodeKind::Root => {}
                }
            }
        }
        let mut budget = 10_000;
        reach(
            &cmd.root,
            prefix,
            &sender,
            resolver,
            &mut reached,
            &mut budget,
        );
        for node in reached {
            for child in &node.children {
                match &child.kind {
                    NodeKind::Literal(l) => plan.candidates.push(Candidate {
                        text: l.clone(),
                        usages: child.usages.clone(),
                    }),
                    NodeKind::Argument(spec) => {
                        for s in suggest(spec, partial, resolver) {
                            plan.candidates.push(Candidate {
                                text: s,
                                usages: child.usages.clone(),
                            });
                        }
                        if spec.dynamic_suggestions {
                            plan.dynamic.push(DynamicRequest {
                                argument: spec.name.clone(),
                                usages: child.usages.clone(),
                            });
                        }
                    }
                    NodeKind::Root => {}
                }
            }
        }
        plan
    }

    /// Neutral export of every command.
    pub fn export(&self) -> Vec<CommandTreeExport> {
        self.commands.values().map(export_command).collect()
    }

    /// Horizon usage lines for every bound label of every command.
    pub fn horizon_usages(&self) -> Vec<HorizonUsage> {
        let mut out = Vec::new();
        for c in self.commands.values() {
            out.extend(horizon_usages_of(c));
        }
        out.sort_by(|a, b| a.usage.cmp(&b.usage));
        out
    }
}

fn export_node(cmd: &RegisteredCommand, node: &CommandNode) -> ExportNode {
    let perms: Vec<Option<&str>> = node
        .usages
        .iter()
        .map(|u| cmd.usage_permission(*u))
        .collect();
    let permission = match perms.first() {
        Some(first) if perms.iter().all(|p| p == first) => first.map(str::to_owned),
        _ => None,
    };
    let (kind, name) = match &node.kind {
        NodeKind::Root => (ExportKind::Root, cmd.spec.name.clone()),
        NodeKind::Literal(l) => (ExportKind::Literal, l.clone()),
        NodeKind::Argument(a) => (ExportKind::Argument(a.kind), a.name.clone()),
    };
    ExportNode {
        kind,
        name,
        children: node.children.iter().map(|c| export_node(cmd, c)).collect(),
        permission,
        executable: node.executes.is_some(),
        usages: node.usages.clone(),
    }
}

/// Export one command.
pub fn export_command(cmd: &RegisteredCommand) -> CommandTreeExport {
    CommandTreeExport {
        id: cmd.id,
        owner: cmd.owner.to_string(),
        name: cmd.spec.name.clone(),
        aliases: cmd.spec.aliases.clone(),
        labels: cmd.labels.clone(),
        description: cmd.spec.description.clone(),
        permission: cmd.spec.permission.clone(),
        players_only: cmd.spec.players_only,
        root: export_node(cmd, &cmd.root),
    }
}

/// Horizon usage lines of one command: one per (bound label, usage); an `item` argument also
/// gets a variant followed by `<name_count:integer>` so the optional count reaches the parser.
pub fn horizon_usages_of(cmd: &RegisteredCommand) -> Vec<HorizonUsage> {
    let mut out = Vec::new();
    for label in &cmd.labels {
        for (ui, u) in cmd.spec.usages.iter().enumerate() {
            let mut variants: Vec<Vec<String>> = vec![vec![label.clone()]];
            let n = u.nodes.len();
            for (i, node) in u.nodes.iter().enumerate() {
                let last = i + 1 == n;
                match node {
                    NodeSpec::Literal(l) => variants.iter_mut().for_each(|v| v.push(l.clone())),
                    NodeSpec::Argument(a) => {
                        let word = format!("<{}:{}>", a.name, horizon_kind(a, last));
                        if a.kind == ArgKind::Item && variants.len() < 8 {
                            let mut with_count = variants.clone();
                            for v in &mut variants {
                                v.push(word.clone());
                            }
                            for v in &mut with_count {
                                v.push(word.clone());
                                v.push(format!("<{}_count:integer>", a.name));
                            }
                            variants.extend(with_count);
                        } else {
                            variants.iter_mut().for_each(|v| v.push(word.clone()));
                        }
                    }
                }
            }
            for v in variants {
                out.push(HorizonUsage {
                    label: label.clone(),
                    usage: v.join(" "),
                    permission: cmd.usage_permission(ui as u32).map(str::to_owned),
                    description: if u.description.is_empty() {
                        cmd.spec.description.clone()
                    } else {
                        u.description.clone()
                    },
                });
            }
        }
    }
    out
}

/// Small helpers to build specs (used by the core's built-ins, tests and the test harness).
pub mod build {
    use crate::model::{ArgKind, ArgSpec, CommandSpec, NodeSpec, Usage};

    /// A literal node.
    pub fn lit(s: &str) -> NodeSpec {
        NodeSpec::Literal(s.to_owned())
    }

    /// An argument node.
    pub fn arg(name: &str, kind: ArgKind) -> NodeSpec {
        NodeSpec::Argument(ArgSpec::new(name, kind))
    }

    /// An argument node with dynamic suggestions.
    pub fn dyn_arg(name: &str, kind: ArgKind) -> NodeSpec {
        let mut a = ArgSpec::new(name, kind);
        a.dynamic_suggestions = true;
        NodeSpec::Argument(a)
    }

    /// A usage.
    pub fn usage(nodes: Vec<NodeSpec>) -> Usage {
        Usage {
            nodes,
            permission: None,
            description: String::new(),
        }
    }

    /// A usage with its own permission.
    pub fn usage_perm(nodes: Vec<NodeSpec>, permission: &str) -> Usage {
        Usage {
            nodes,
            permission: Some(permission.to_owned()),
            description: String::new(),
        }
    }

    /// A command spec.
    pub fn command(name: &str, usages: Vec<Usage>) -> CommandSpec {
        CommandSpec {
            name: name.to_owned(),
            aliases: Vec::new(),
            description: String::new(),
            permission: None,
            players_only: false,
            usages,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::build::*;
    use super::parse::tests::FakeResolver;
    use super::*;
    use crate::model::{ItemStack, Uuid};

    fn p(s: &str) -> PluginId {
        PluginId::new_unchecked(s)
    }

    fn pay() -> CommandSpec {
        let mut amount = ArgSpec::new("amount", ArgKind::Integer);
        amount.min = Some(1.0);
        let mut c = command(
            "pay",
            vec![
                usage(vec![
                    arg("player", ArgKind::Player),
                    NodeSpec::Argument(amount),
                ]),
                usage_perm(
                    vec![lit("all"), arg("amount", ArgKind::Integer)],
                    "eco.pay.all",
                ),
                usage(vec![lit("history")]),
                usage(vec![lit("history"), arg("page", ArgKind::Integer)]),
            ],
        );
        c.aliases = vec!["give-money".into(), "PAY".into()];
        c.permission = Some("eco.pay".into());
        c
    }

    fn parse(reg: &CommandRegistry, line: &str) -> ParseOutcome {
        let mut r = FakeResolver::with_players();
        reg.parse(line, CommandSender::Console, &mut r)
    }

    #[test]
    fn labels_and_conflicts() {
        let mut reg = CommandRegistry::new();
        let labels = reg.register(&p("eco"), 1, pay()).unwrap();
        assert_eq!(labels, ["eco:give-money", "eco:pay", "give-money", "pay"]);
        // Second plugin with the same name: only namespaced.
        let labels = reg.register(&p("bank"), 2, command("pay", vec![])).unwrap();
        assert_eq!(labels, ["bank:pay"]);
        assert_eq!(reg.by_label("PAY").unwrap().owner, "eco");
        // Same plugin twice: refused.
        assert!(reg.register(&p("eco"), 3, command("pay", vec![])).is_err());
        assert!(
            reg.register(&p("eco"), 3, command("bank:x", vec![]))
                .is_err()
        );
        assert!(
            reg.register(&p("eco"), 3, command("bad name", vec![]))
                .is_err()
        );
        assert_eq!(
            reg.register(&p("eco"), 3, command("eco:bal", vec![]))
                .unwrap(),
            ["bal", "eco:bal"]
        );
        // Removing the winner hands the bare name over.
        let g = reg.generation();
        assert!(!reg.unregister("bank", 1));
        assert!(reg.unregister("eco", 1));
        assert!(reg.generation() > g);
        assert_eq!(reg.by_label("pay").unwrap().owner, "bank");
        assert_eq!(reg.remove_plugin("eco"), 1);
        assert!(reg.by_label("bal").is_none());
    }

    #[test]
    fn validation() {
        let o = p("x");
        let bad = |c: CommandSpec| normalize_spec(&o, c).unwrap_err();
        assert!(
            bad(command(
                "a",
                vec![usage(vec![arg("t", ArgKind::Greedy), lit("x")])]
            ))
            .contains("last")
        );
        assert!(
            bad(command(
                "a",
                vec![usage(vec![
                    arg("t", ArgKind::Word),
                    arg("t", ArgKind::Word)
                ])]
            ))
            .contains("two arguments")
        );
        let mut w = ArgSpec::new("w", ArgKind::Word);
        w.min = Some(1.0);
        assert!(bad(command("a", vec![usage(vec![NodeSpec::Argument(w)])])).contains("min/max"));
        let mut i = ArgSpec::new("i", ArgKind::Integer);
        i.min = Some(5.0);
        i.max = Some(1.0);
        assert!(bad(command("a", vec![usage(vec![NodeSpec::Argument(i)])])).contains("min > max"));
        assert!(
            bad(command(
                "a",
                vec![usage(vec![lit("x")]), usage(vec![lit("X")])]
            ))
            .contains("duplicates")
        );
        assert!(bad(command("a", vec![usage(vec![lit("x y")])])).contains("literal"));
        let mut c = command("a", vec![]);
        c.permission = Some("Bad..Perm".into());
        assert!(normalize_spec(&o, c).is_err());
        let ok = normalize_spec(&o, command("A", vec![])).unwrap();
        assert_eq!(ok.name, "a");
        assert_eq!(ok.usages.len(), 1);
    }

    #[test]
    fn parsing_usages() {
        let mut reg = CommandRegistry::new();
        reg.register(&p("eco"), 1, pay()).unwrap();
        match parse(&reg, "pay bob 5") {
            ParseOutcome::Command {
                matches,
                label,
                error,
                ..
            } => {
                assert_eq!(label, "pay");
                assert!(error.is_none());
                assert_eq!(matches.len(), 1);
                assert_eq!(matches[0].usage, 0);
                assert_eq!(
                    matches[0].args[0],
                    ("player".into(), ArgValue::Player(Uuid::new(0, 2)))
                );
                assert_eq!(matches[0].args[1], ("amount".into(), ArgValue::Integer(5)));
            }
            other => panic!("{other:?}"),
        }
        match parse(&reg, "eco:give-money ALL   7  ") {
            ParseOutcome::Command { matches, .. } => assert_eq!(matches[0].usage, 1),
            other => panic!("{other:?}"),
        }
        match parse(&reg, "pay history") {
            ParseOutcome::Command { matches, .. } => assert_eq!(matches[0].usage, 2),
            other => panic!("{other:?}"),
        }
        match parse(&reg, "pay history 2") {
            ParseOutcome::Command { matches, .. } => assert_eq!(matches[0].usage, 3),
            other => panic!("{other:?}"),
        }
        let err = |line: &str| match parse(&reg, line) {
            ParseOutcome::Command {
                error: Some(e),
                matches,
                ..
            } if matches.is_empty() => e,
            other => panic!("{line}: {other:?}"),
        };
        assert!(
            err("pay bob 0").contains("at least 1"),
            "{}",
            err("pay bob 0")
        );
        assert!(err("pay bob x").contains("integer"));
        assert!(err("pay dave 5").contains("not online"));
        assert!(err("pay bob").contains("incomplete"));
        assert!(err("pay bob 5 6").contains("too many"));
        assert!(err("pay history 2 3").contains("too many"));
        assert_eq!(parse(&reg, "nope"), ParseOutcome::Unknown);
    }

    #[test]
    fn backtracking_item_count() {
        let mut reg = CommandRegistry::new();
        reg.register(
            &p("kit"),
            1,
            command(
                "give2",
                vec![
                    usage(vec![
                        arg("item", ArgKind::Item),
                        arg("slots", ArgKind::Integer),
                    ]),
                    usage(vec![arg("item", ArgKind::Item)]),
                ],
            ),
        )
        .unwrap();
        match parse(&reg, "give2 diamond 5") {
            ParseOutcome::Command { matches, .. } => {
                // usage 0 needs `slots`: item without count + slots 5; usage 1: item with count 5.
                assert_eq!(matches.len(), 2);
                assert_eq!(matches[0].usage, 0);
                assert_eq!(
                    matches[0].args[0].1,
                    ArgValue::Item(ItemStack::new("minecraft:diamond", 1))
                );
                assert_eq!(matches[1].usage, 1);
                assert_eq!(
                    matches[1].args[0].1,
                    ArgValue::Item(ItemStack::new("minecraft:diamond", 5))
                );
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn completion_plans() {
        let mut reg = CommandRegistry::new();
        reg.register(&p("eco"), 1, pay()).unwrap();
        let mut spec = command("home", vec![usage(vec![dyn_arg("name", ArgKind::Word)])]);
        spec.aliases = vec!["h".into()];
        reg.register(&p("homes"), 2, spec).unwrap();
        let mut r = FakeResolver::with_players();
        let plan = reg.complete("p", CommandSender::Console, &mut r);
        assert_eq!(
            plan.labels.iter().map(|l| l.0.as_str()).collect::<Vec<_>>(),
            ["pay"]
        );
        let plan = reg.complete("pay ", CommandSender::Console, &mut r);
        let texts: Vec<_> = plan.candidates.iter().map(|c| c.text.as_str()).collect();
        assert_eq!(texts, ["Alice", "Bob", "Carol", "all", "history"]);
        let all = plan.candidates.iter().find(|c| c.text == "all").unwrap();
        assert_eq!(all.usages, [1]);
        let plan = reg.complete("pay history ", CommandSender::Console, &mut r);
        assert!(plan.candidates.is_empty());
        let plan = reg.complete("h ki", CommandSender::Console, &mut r);
        assert_eq!(plan.partial, "ki");
        assert_eq!(
            plan.dynamic,
            [DynamicRequest {
                argument: "name".into(),
                usages: vec![0]
            }]
        );
        assert!(
            reg.complete("zzz a", CommandSender::Console, &mut r)
                .command
                .is_none()
        );
    }

    #[test]
    fn exports() {
        let mut reg = CommandRegistry::new();
        reg.register(&p("eco"), 1, pay()).unwrap();
        let e = &reg.export()[0];
        assert_eq!(e.name, "pay");
        assert_eq!(e.root.kind, ExportKind::Root);
        assert_eq!(e.root.children.len(), 3); // <player>, all, history (merged)
        let all = &e.root.children[1];
        assert_eq!(all.permission.as_deref(), Some("eco.pay.all"));
        assert_eq!(e.root.children[0].permission.as_deref(), Some("eco.pay"));
        assert!(e.root.permission.is_none(), "mixed");
        let hist = &e.root.children[2];
        assert!(hist.executable);
        assert_eq!(
            hist.children[0].kind,
            ExportKind::Argument(ArgKind::Integer)
        );
        let usages: Vec<String> = reg.horizon_usages().into_iter().map(|u| u.usage).collect();
        assert!(usages.contains(&"pay <player:player> <amount:integer(1,)>".to_owned()));
        assert!(usages.contains(&"eco:pay all <amount:integer>".to_owned()));
        assert!(usages.contains(&"give-money history".to_owned()));
        assert_eq!(usages.len(), 4 * 4);
        let mut kit = CommandRegistry::new();
        kit.register(
            &p("kit"),
            1,
            command(
                "k",
                vec![usage(vec![
                    arg("item", ArgKind::Item),
                    arg("msg", ArgKind::Greedy),
                ])],
            ),
        )
        .unwrap();
        let u: Vec<String> = kit.horizon_usages().into_iter().map(|u| u.usage).collect();
        assert!(u.contains(&"k <item:item> <msg:text>".to_owned()));
        assert!(u.contains(&"k <item:item> <item_count:integer> <msg:text>".to_owned()));
        let json = serde_json::to_string(&reg.export()).unwrap();
        assert!(json.contains("\"argument\":\"integer\""), "{json}");
    }

    #[test]
    fn horizon_kind_mapping() {
        let k = |kind, last| horizon_kind(&ArgSpec::new("a", kind), last);
        assert_eq!(k(ArgKind::Word, false), "word");
        assert_eq!(k(ArgKind::String, true), "string");
        assert_eq!(k(ArgKind::String, false), "string");
        assert_eq!(k(ArgKind::Greedy, true), "text");
        assert_eq!(k(ArgKind::Players, true), "players");
        assert_eq!(k(ArgKind::World, false), "dimension");
        assert_eq!(k(ArgKind::Position, false), "vec3");
        assert_eq!(k(ArgKind::Position, true), "vec3");
        assert_eq!(k(ArgKind::BlockPos, false), "block_pos");
        assert_eq!(k(ArgKind::Duration, false), "word");
        assert_eq!(k(ArgKind::Integer, false), "integer");
        let mut s = ArgSpec::new("a", ArgKind::Word);
        s.dynamic_suggestions = true;
        assert_eq!(horizon_kind(&s, false), "word?");
        let mut s = ArgSpec::new("a", ArgKind::String);
        s.suggestions = vec!["x".into()];
        assert_eq!(horizon_kind(&s, false), "string?");
        let mut s = ArgSpec::new("a", ArgKind::Players);
        s.dynamic_suggestions = true;
        assert_eq!(horizon_kind(&s, false), "word?");
        let mut n = ArgSpec::new("n", ArgKind::Integer);
        n.min = Some(1.0);
        n.max = Some(64.5);
        assert_eq!(horizon_kind(&n, false), "integer(1,64)");
        n.max = None;
        n.min = Some(0.5);
        assert_eq!(horizon_kind(&n, false), "integer(1,)");
        let mut f = ArgSpec::new("f", ArgKind::Number);
        f.max = Some(2.5);
        assert_eq!(horizon_kind(&f, false), "number(,2.5)");
    }
}
