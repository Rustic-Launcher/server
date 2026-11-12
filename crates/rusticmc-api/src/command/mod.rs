//! Commands: a builder over the WIT command tree, typed arguments and senders.
//!
//! ```ignore
//! Command::new("give-diamonds")
//!     .description("Gives diamonds")
//!     .permission("example.give")
//!     .usage("<target:player> [amount:integer(1..64)]")
//!     .executor(|cmd| {
//!         let target: Player = cmd.arg("target")?;
//!         let amount: u32 = cmd.opt_arg("amount")?.unwrap_or(1);
//!         target.inventory().give(ItemStack::new("diamond").count(amount))?;
//!         cmd.reply(Text::mini("<green>Done"));
//!         Ok(())
//!     })
//!     .register()?;
//! ```
//!
//! See [`usage`](self#usage-strings) for the usage syntax: `<required>`, `[optional]`,
//! `name:kind`, ranges `(1..64)`, fixed suggestions `{a|b}`, dynamic suggestions `*`.
//!
//! # Usage strings
//!
//! ```text
//! node  := literal | '<' arg '>' | '[' arg ']'
//! arg   := name [':' kind] ['(' min '..' max ')'] ['{' a '|' b '}'] ['*']
//! kind  := word | string | greedy | integer | number | boolean | player | players
//!        | position | block-pos | item | world | duration
//! ```

pub(crate) mod usage;

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use crate::__rt;
use crate::bindings::rustic::api::commands::{
    self as wit, ArgValue, CommandContext, CommandSpec, CompleteContext, Usage,
};
use crate::bindings::rustic::api::types::CommandSender;
use crate::error::Error;
use crate::host::call;
use crate::item::ItemStack;
use crate::player::Player;
use crate::text::{Color, NamedColor, Text};
use crate::types::{BlockPos, HostError, Location, Uuid};
use crate::world::World;

/// What a command executor returns.
pub type CommandResult = Result<(), CommandError>;

pub(crate) type Executor = Rc<RefCell<dyn FnMut(&mut CommandCtx) -> CommandResult>>;
pub(crate) type Completer = Rc<RefCell<dyn FnMut(&CompleteCtx) -> Vec<String>>>;

/// Who ran a command (or is asked for a permission).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Sender {
    /// The server console (or RCON).
    Console,
    /// A player.
    Player(Player),
}

impl Sender {
    pub(crate) fn from_raw(raw: CommandSender) -> Sender {
        match raw {
            CommandSender::Console => Sender::Console,
            CommandSender::Player(u) => Sender::Player(Player::from_uuid(u)),
        }
    }

    /// The WIT value.
    pub fn to_raw(self) -> CommandSender {
        match self {
            Sender::Console => CommandSender::Console,
            Sender::Player(p) => CommandSender::Player(p.uuid()),
        }
    }

    /// Sends a message (to the console log or the player's chat).
    pub fn send_message(&self, message: impl Into<Text>) {
        let json = message.into().to_json();
        call!("commands.reply", wit::reply(self.to_raw(), &json));
    }

    /// Whether the sender has a permission node (the console has all).
    pub fn has_permission(&self, node: &str) -> bool {
        crate::permissions::Permissions::has(*self, node)
    }

    /// The player, if a player ran it.
    pub fn as_player(&self) -> Option<Player> {
        match self {
            Sender::Player(p) => Some(*p),
            Sender::Console => None,
        }
    }

    /// Whether the console ran it.
    pub fn is_console(&self) -> bool {
        matches!(self, Sender::Console)
    }

    /// `CONSOLE` or the player's name.
    pub fn name(&self) -> String {
        match self {
            Sender::Console => "CONSOLE".to_owned(),
            Sender::Player(p) => p.name().unwrap_or_else(|| p.uuid().to_string()),
        }
    }
}

impl From<Player> for Sender {
    fn from(p: Player) -> Self {
        Sender::Player(p)
    }
}

/// An error shown to the command's sender (in red unless the text sets a color).
#[derive(Debug, Clone, PartialEq)]
pub struct CommandError {
    // Boxed: keeps `CommandResult` small (clippy::result_large_err).
    message: Box<Text>,
}

impl CommandError {
    /// An error with this message.
    pub fn new(message: impl Into<Text>) -> CommandError {
        CommandError {
            message: Box::new(message.into()),
        }
    }

    /// The message.
    pub fn message(&self) -> &Text {
        &self.message
    }

    /// The JSON text sent to the sender: red unless the message chose a color.
    pub fn to_json(&self) -> String {
        if self.message.style().color.is_some() {
            self.message.to_json()
        } else {
            (*self.message)
                .clone()
                .color(Color::Named(NamedColor::Red))
                .to_json()
        }
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message.plain())
    }
}

impl std::error::Error for CommandError {}

impl From<&str> for CommandError {
    fn from(s: &str) -> Self {
        CommandError::new(s)
    }
}

impl From<String> for CommandError {
    fn from(s: String) -> Self {
        CommandError::new(s)
    }
}

impl From<Text> for CommandError {
    fn from(t: Text) -> Self {
        CommandError {
            message: Box::new(t),
        }
    }
}

impl From<Error> for CommandError {
    fn from(e: Error) -> Self {
        CommandError::new(e.to_string())
    }
}

impl From<crate::services::ServiceError> for CommandError {
    fn from(e: crate::services::ServiceError) -> Self {
        CommandError::new(crate::services::describe_service_error(&e))
    }
}

impl From<HostError> for CommandError {
    fn from(e: HostError) -> Self {
        CommandError::new(crate::error::describe_host_error(&e))
    }
}

/// Converts a parsed argument value to a Rust type, for [`CommandCtx::arg`].
pub trait FromArg: Sized {
    /// What the conversion expects, for error messages.
    const EXPECTED: &'static str;
    /// `None` if the value has another kind.
    fn from_arg(value: &ArgValue) -> Option<Self>;
}

impl FromArg for String {
    const EXPECTED: &'static str = "text";
    fn from_arg(v: &ArgValue) -> Option<Self> {
        match v {
            ArgValue::Word(s) | ArgValue::String(s) | ArgValue::Greedy(s) | ArgValue::World(s) => {
                Some(s.clone())
            }
            ArgValue::Integer(i) => Some(i.to_string()),
            ArgValue::Number(n) => Some(n.to_string()),
            ArgValue::Boolean(b) => Some(b.to_string()),
            _ => None,
        }
    }
}

macro_rules! int_from_arg {
    ($($t:ty),*) => {$(
        impl FromArg for $t {
            const EXPECTED: &'static str = "an integer";
            fn from_arg(v: &ArgValue) -> Option<Self> {
                match v {
                    ArgValue::Integer(i) => <$t>::try_from(*i).ok(),
                    ArgValue::Duration(d) => <$t>::try_from(*d).ok(),
                    _ => None,
                }
            }
        }
    )*};
}
int_from_arg!(i64, i32, u32, u64, usize, u8, i16, u16);

impl FromArg for f64 {
    const EXPECTED: &'static str = "a number";
    fn from_arg(v: &ArgValue) -> Option<Self> {
        match v {
            ArgValue::Number(n) => Some(*n),
            ArgValue::Integer(i) => Some(*i as f64),
            _ => None,
        }
    }
}

impl FromArg for f32 {
    const EXPECTED: &'static str = "a number";
    fn from_arg(v: &ArgValue) -> Option<Self> {
        f64::from_arg(v).map(|n| n as f32)
    }
}

impl FromArg for bool {
    const EXPECTED: &'static str = "true or false";
    fn from_arg(v: &ArgValue) -> Option<Self> {
        match v {
            ArgValue::Boolean(b) => Some(*b),
            _ => None,
        }
    }
}

impl FromArg for Player {
    const EXPECTED: &'static str = "a player";
    fn from_arg(v: &ArgValue) -> Option<Self> {
        match v {
            ArgValue::Player(u) => Some(Player::from_uuid(*u)),
            ArgValue::Players(us) if us.len() == 1 => Some(Player::from_uuid(us[0])),
            _ => None,
        }
    }
}

impl FromArg for Uuid {
    const EXPECTED: &'static str = "a player";
    fn from_arg(v: &ArgValue) -> Option<Self> {
        Player::from_arg(v).map(|p| p.uuid())
    }
}

impl FromArg for Vec<Player> {
    const EXPECTED: &'static str = "players";
    fn from_arg(v: &ArgValue) -> Option<Self> {
        match v {
            ArgValue::Players(us) => Some(us.iter().copied().map(Player::from_uuid).collect()),
            ArgValue::Player(u) => Some(vec![Player::from_uuid(*u)]),
            _ => None,
        }
    }
}

impl FromArg for Location {
    const EXPECTED: &'static str = "a position";
    fn from_arg(v: &ArgValue) -> Option<Self> {
        match v {
            ArgValue::Position(l) => Some(l.clone()),
            _ => None,
        }
    }
}

impl FromArg for BlockPos {
    const EXPECTED: &'static str = "a block position";
    fn from_arg(v: &ArgValue) -> Option<Self> {
        match v {
            ArgValue::BlockPos(p) => Some(*p),
            ArgValue::Position(l) => Some(l.block()),
            _ => None,
        }
    }
}

impl FromArg for ItemStack {
    const EXPECTED: &'static str = "an item";
    fn from_arg(v: &ArgValue) -> Option<Self> {
        match v {
            ArgValue::Item(i) => Some(ItemStack::from_raw(i.clone())),
            _ => None,
        }
    }
}

impl FromArg for World {
    const EXPECTED: &'static str = "a world";
    fn from_arg(v: &ArgValue) -> Option<Self> {
        match v {
            ArgValue::World(w) => Some(World::new(w.clone())),
            _ => None,
        }
    }
}

/// A duration argument, in ticks (`10s` = 200).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ticks(pub u64);

impl FromArg for Ticks {
    const EXPECTED: &'static str = "a duration";
    fn from_arg(v: &ArgValue) -> Option<Self> {
        match v {
            ArgValue::Duration(d) => Some(Ticks(*d)),
            ArgValue::Integer(i) => u64::try_from(*i).ok().map(Ticks),
            _ => None,
        }
    }
}

/// A command invocation: sender, typed arguments, label.
#[derive(Debug, Clone)]
pub struct CommandCtx {
    sender: Sender,
    label: String,
    usage: usize,
    args: Vec<(String, ArgValue)>,
    raw: String,
}

impl CommandCtx {
    /// Builds a context (the runtime does this; public for tests of executors).
    pub fn new(
        sender: Sender,
        label: impl Into<String>,
        usage: usize,
        args: Vec<(String, ArgValue)>,
        raw: impl Into<String>,
    ) -> CommandCtx {
        CommandCtx {
            sender,
            label: label.into(),
            usage,
            args,
            raw: raw.into(),
        }
    }

    pub(crate) fn from_raw(raw: CommandContext, usage: usize) -> CommandCtx {
        CommandCtx {
            sender: Sender::from_raw(raw.sender),
            label: raw.label,
            usage,
            args: raw.args,
            raw: raw.raw,
        }
    }

    /// Who ran the command.
    pub fn sender(&self) -> Sender {
        self.sender
    }

    /// The player who ran it, or an error ("only players can do that") for the console.
    pub fn player(&self) -> Result<Player, CommandError> {
        self.sender
            .as_player()
            .ok_or_else(|| CommandError::new("Only players can use this command."))
    }

    /// A required argument, converted: `cmd.arg::<Player>("target")?`.
    pub fn arg<T: FromArg>(&self, name: &str) -> Result<T, CommandError> {
        self.opt_arg(name)?
            .ok_or_else(|| CommandError::new(format!("Missing argument <{name}>.")))
    }

    /// An optional argument: `Ok(None)` when absent, an error when present with another kind.
    pub fn opt_arg<T: FromArg>(&self, name: &str) -> Result<Option<T>, CommandError> {
        match self.raw_arg(name) {
            None => Ok(None),
            Some(v) => T::from_arg(v).map(Some).ok_or_else(|| {
                CommandError::new(format!("Argument <{name}> must be {}.", T::EXPECTED))
            }),
        }
    }

    /// Whether an argument was given.
    pub fn has_arg(&self, name: &str) -> bool {
        self.raw_arg(name).is_some()
    }

    /// The raw WIT value of an argument.
    pub fn raw_arg(&self, name: &str) -> Option<&ArgValue> {
        self.args.iter().find(|(n, _)| n == name).map(|(_, v)| v)
    }

    /// The name or alias typed.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Index of the usage string (in declaration order on the builder) that matched.
    pub fn usage_index(&self) -> usize {
        self.usage
    }

    /// The whole line, without slash.
    pub fn raw(&self) -> &str {
        &self.raw
    }

    /// Replies to the sender.
    pub fn reply(&self, message: impl Into<Text>) {
        self.sender.send_message(message);
    }
}

/// A tab-completion request for an argument declared with `*` (dynamic suggestions).
#[derive(Debug, Clone)]
pub struct CompleteCtx {
    sender: Sender,
    raw: String,
    argument: String,
    partial: String,
}

impl CompleteCtx {
    /// Builds a context (the runtime does this; public for tests of completers).
    pub fn new(
        sender: Sender,
        raw: impl Into<String>,
        argument: impl Into<String>,
        partial: impl Into<String>,
    ) -> CompleteCtx {
        CompleteCtx {
            sender,
            raw: raw.into(),
            argument: argument.into(),
            partial: partial.into(),
        }
    }

    pub(crate) fn from_raw(raw: CompleteContext) -> CompleteCtx {
        CompleteCtx {
            sender: Sender::from_raw(raw.sender),
            raw: raw.raw,
            argument: raw.argument,
            partial: raw.partial,
        }
    }

    /// Who is typing.
    pub fn sender(&self) -> Sender {
        self.sender
    }

    /// The line typed so far, without slash.
    pub fn raw(&self) -> &str {
        &self.raw
    }

    /// The name of the argument being completed.
    pub fn argument(&self) -> &str {
        &self.argument
    }

    /// What is typed of it so far.
    pub fn partial(&self) -> &str {
        &self.partial
    }

    /// Keeps the options starting with what is typed (case-insensitive).
    pub fn suggest<I, S>(&self, options: I) -> Vec<String>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let p = self.partial.to_lowercase();
        options
            .into_iter()
            .map(Into::into)
            .filter(|o| o.to_lowercase().starts_with(&p))
            .collect()
    }
}

struct Route {
    usage: String,
    permission: Option<String>,
    description: Option<String>,
    executor: Option<Executor>,
}

/// A command under construction. Register it with [`Command::register`] or `ctx.command(..)`.
pub struct Command {
    name: String,
    aliases: Vec<String>,
    description: String,
    permission: Option<String>,
    players_only: bool,
    routes: Vec<Route>,
    executor: Option<Executor>,
    completer: Option<Completer>,
}

impl fmt::Debug for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Command")
            .field("name", &self.name)
            .field("aliases", &self.aliases)
            .field("permission", &self.permission)
            .field(
                "usages",
                &self.routes.iter().map(|r| &r.usage).collect::<Vec<_>>(),
            )
            .finish_non_exhaustive()
    }
}

/// The parts of a registered command the runtime needs to dispatch it.
pub(crate) struct CommandEntry {
    /// WIT usage index -> route index.
    pub(crate) usage_routes: Vec<usize>,
    /// Route index -> executor.
    pub(crate) executors: Vec<Executor>,
    pub(crate) completer: Option<Completer>,
}

impl Command {
    /// A command named `name` (no slash).
    pub fn new(name: impl Into<String>) -> Command {
        Command {
            name: name.into(),
            aliases: Vec::new(),
            description: String::new(),
            permission: None,
            players_only: false,
            routes: Vec::new(),
            executor: None,
            completer: None,
        }
    }

    /// Shown in help and on hover in the client.
    pub fn description(mut self, d: impl Into<String>) -> Command {
        self.description = d.into();
        self
    }

    /// Adds an alias.
    pub fn alias(mut self, a: impl Into<String>) -> Command {
        self.aliases.push(a.into());
        self
    }

    /// Adds aliases.
    pub fn aliases<I, S>(mut self, a: I) -> Command
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.aliases.extend(a.into_iter().map(Into::into));
        self
    }

    /// Permission needed to see and run the command.
    pub fn permission(mut self, p: impl Into<String>) -> Command {
        self.permission = Some(p.into());
        self
    }

    /// Refuses the console.
    pub fn players_only(mut self) -> Command {
        self.players_only = true;
        self
    }

    /// Adds a usage handled by the default [`Command::executor`]. Without any usage, the command
    /// takes no arguments.
    pub fn usage(mut self, usage: impl Into<String>) -> Command {
        self.routes.push(Route {
            usage: usage.into(),
            permission: None,
            description: None,
            executor: None,
        });
        self
    }

    /// Adds a usage with its own executor (sub-commands: `.route("set <name:word>", ...)`).
    pub fn route<F>(mut self, usage: impl Into<String>, executor: F) -> Command
    where
        F: FnMut(&mut CommandCtx) -> CommandResult + 'static,
    {
        self.routes.push(Route {
            usage: usage.into(),
            permission: None,
            description: None,
            executor: Some(Rc::new(RefCell::new(executor))),
        });
        self
    }

    /// Sets the permission of the last added usage (overrides the command's).
    pub fn usage_permission(mut self, p: impl Into<String>) -> Command {
        if let Some(r) = self.routes.last_mut() {
            r.permission = Some(p.into());
        }
        self
    }

    /// Sets the description of the last added usage.
    pub fn usage_description(mut self, d: impl Into<String>) -> Command {
        if let Some(r) = self.routes.last_mut() {
            r.description = Some(d.into());
        }
        self
    }

    /// The executor for usages without their own.
    pub fn executor<F>(mut self, executor: F) -> Command
    where
        F: FnMut(&mut CommandCtx) -> CommandResult + 'static,
    {
        self.executor = Some(Rc::new(RefCell::new(executor)));
        self
    }

    /// Suggestions for arguments declared with `*`.
    pub fn completer<F>(mut self, completer: F) -> Command
    where
        F: FnMut(&CompleteCtx) -> Vec<String> + 'static,
    {
        self.completer = Some(Rc::new(RefCell::new(completer)));
        self
    }

    /// The name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Builds the WIT spec and the dispatch entry (pure; no host call).
    pub(crate) fn build(self) -> Result<(CommandSpec, CommandEntry), Error> {
        let mut routes = self.routes;
        if routes.is_empty() {
            routes.push(Route {
                usage: String::new(),
                permission: None,
                description: None,
                executor: None,
            });
        }
        let mut usages = Vec::new();
        let mut usage_routes = Vec::new();
        let mut executors = Vec::new();
        for (i, r) in routes.into_iter().enumerate() {
            let expansions = usage::parse_usage(&r.usage).map_err(|e| {
                Error::Other(format!("command `{}`, usage `{}`: {e}", self.name, r.usage))
            })?;
            for nodes in expansions {
                usages.push(Usage {
                    nodes,
                    permission: r.permission.clone(),
                    description: r
                        .description
                        .clone()
                        .unwrap_or_else(|| self.description.clone()),
                });
                usage_routes.push(i);
            }
            let exec = r.executor.or_else(|| self.executor.clone()).ok_or_else(|| {
                Error::Other(format!(
                    "command `{}`: usage `{}` has no executor (call .executor(..) or use .route(..))",
                    self.name, r.usage
                ))
            })?;
            executors.push(exec);
        }
        let spec = CommandSpec {
            name: self.name,
            aliases: self.aliases,
            description: self.description,
            permission: self.permission,
            players_only: self.players_only,
            usages,
        };
        Ok((
            spec,
            CommandEntry {
                usage_routes,
                executors,
                completer: self.completer,
            },
        ))
    }

    /// Registers the command with the server. In the async instance this does nothing and
    /// returns an inert handle (commands run on the main instance).
    pub fn register(self) -> Result<CommandHandle, Error> {
        let (spec, entry) = self.build()?;
        if !__rt::is_main() {
            return Ok(CommandHandle { id: None });
        }
        let id = call!("commands.register", wit::register(&spec)).map_err(Error::Other)?;
        __rt::add_command(id, entry);
        Ok(CommandHandle { id: Some(id) })
    }
}

/// A registered command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandHandle {
    id: Option<u32>,
}

impl CommandHandle {
    /// The host's id (`None` in the async instance).
    pub fn id(&self) -> Option<u32> {
        self.id
    }

    /// Removes the command.
    pub fn unregister(self) {
        if let Some(id) = self.id {
            call!("commands.unregister", wit::unregister(id));
            __rt::remove_command(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bindings::rustic::api::commands::{ArgKind, NodeSpec};

    #[test]
    fn build_spec_and_routes() {
        let (spec, entry) = Command::new("home")
            .alias("h")
            .description("Homes")
            .permission("homes.use")
            .players_only()
            .usage("[name:word*]")
            .route("set <name:word>", |_| Ok(()))
            .usage_permission("homes.set")
            .executor(|_| Ok(()))
            .completer(|c| c.suggest(["base", "farm"]))
            .build()
            .unwrap();
        assert_eq!(spec.name, "home");
        assert_eq!(spec.aliases, vec!["h"]);
        assert!(spec.players_only);
        assert_eq!(spec.usages.len(), 3);
        assert_eq!(entry.usage_routes, vec![0, 0, 1]);
        assert_eq!(entry.executors.len(), 2);
        assert_eq!(spec.usages[2].permission.as_deref(), Some("homes.set"));
        assert_eq!(spec.usages[0].description, "Homes");
        assert_eq!(spec.usages[2].nodes[0], NodeSpec::Literal("set".into()));
        match &spec.usages[1].nodes[0] {
            NodeSpec::Argument(a) => {
                assert_eq!(a.kind, ArgKind::Word);
                assert!(a.dynamic_suggestions);
            }
            other => panic!("{other:?}"),
        }
        let c = CompleteCtx::new(Sender::Console, "home f", "name", "F");
        let out = (entry.completer.unwrap().borrow_mut())(&c);
        assert_eq!(out, vec!["farm"]);
    }

    #[test]
    fn build_errors() {
        assert!(
            Command::new("x")
                .usage("<a")
                .executor(|_| Ok(()))
                .build()
                .is_err()
        );
        assert!(Command::new("x").usage("<a>").build().is_err());
        assert!(Command::new("x").executor(|_| Ok(())).build().is_ok());
    }

    #[test]
    fn typed_args() {
        let u = Uuid::from_u128(7);
        let ctx = CommandCtx::new(
            Sender::Console,
            "give",
            0,
            vec![
                ("target".into(), ArgValue::Player(u)),
                ("amount".into(), ArgValue::Integer(5)),
                ("ratio".into(), ArgValue::Number(0.5)),
                ("flag".into(), ArgValue::Boolean(true)),
                ("name".into(), ArgValue::Word("base".into())),
                ("time".into(), ArgValue::Duration(200)),
                ("neg".into(), ArgValue::Integer(-1)),
            ],
            "give x 5",
        );
        assert_eq!(ctx.arg::<Player>("target").unwrap().uuid(), u);
        assert_eq!(ctx.arg::<i64>("amount").unwrap(), 5);
        assert_eq!(ctx.arg::<u32>("amount").unwrap(), 5);
        assert_eq!(ctx.arg::<f64>("amount").unwrap(), 5.0);
        assert_eq!(ctx.arg::<f64>("ratio").unwrap(), 0.5);
        assert!(ctx.arg::<bool>("flag").unwrap());
        assert_eq!(ctx.arg::<String>("name").unwrap(), "base");
        assert_eq!(ctx.arg::<Ticks>("time").unwrap(), Ticks(200));
        assert!(ctx.arg::<u32>("neg").is_err());
        assert!(ctx.arg::<i64>("missing").is_err());
        assert_eq!(ctx.opt_arg::<i64>("missing").unwrap(), None);
        let err = ctx.arg::<Player>("amount").unwrap_err();
        assert_eq!(err.to_string(), "Argument <amount> must be a player.");
        assert!(ctx.player().is_err());
        assert_eq!(ctx.label(), "give");
        assert_eq!(ctx.raw(), "give x 5");
    }

    #[test]
    fn error_is_red_unless_colored() {
        let e = CommandError::from("nope");
        assert_eq!(e.to_json(), r#"{"text":"nope","color":"red"}"#);
        let e = CommandError::from(Text::new("ok").color(NamedColor::Green));
        assert_eq!(e.to_json(), r#"{"text":"ok","color":"green"}"#);
        let e = CommandError::from(HostError::NoSuchPlayer);
        assert_eq!(e.to_string(), "that player is not online");
    }
}
