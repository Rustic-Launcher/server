//! Perms: groups, inheritance, weights, prefixes/suffixes, wildcard and negated nodes,
//! temporary nodes and groups, promotion tracks. Provides the standard
//! `rustic:services/permission-provider` and `rustic:services/chat-meta` services, so every
//! permission check of the server (and every chat formatter) goes through it.
//!
//! * Data: the plugin's KV storage (`groups`, `tracks`, `users/<uuid>`, JSON values), with
//!   `/perms export` / `/perms import` to a human-editable `perms.toml` in the data folder.
//! * Commands: `/perms ...` (permission `perms.admin`), a typed command tree with completion of
//!   players, groups, tracks and nodes.
//! * Messages: English or French (`lang` in `config.toml`).
//!
//! The resolution rules are documented in [`model`].

pub mod lang;
pub mod model;

use std::collections::BTreeSet;

use rustic::command::Ticks;
use rustic::config::Config;
use rustic::permissions::Permissions;
use rustic::prelude::*;
use rustic::text::escape_mini;
use serde::{Deserialize, Serialize};

use crate::lang::{Lang, Msg, line, msg, msg_with, render_meta};
use crate::model::{
    Expired, Group, ModelError, PermData, TempNode, Track, TrackMove, User, format_duration,
    normalize_name, now_secs, parse_node_input,
};

/// The packaged default configuration.
pub const DEFAULT_CONFIG: &str = include_str!("../config.toml");
/// Storage key of the groups map.
pub const GROUPS_KEY: &str = "groups";
/// Storage key of the tracks map.
pub const TRACKS_KEY: &str = "tracks";
/// Storage key prefix of users (`users/<uuid>`).
pub const USER_PREFIX: &str = "users/";
/// The permission guarding `/perms`.
pub const ADMIN_PERMISSION: &str = "perms.admin";

/// `config.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PermsConfig {
    /// Message language.
    pub lang: Lang,
    /// The group of players without any other group (created when missing).
    pub default_group: String,
    /// Period of the expiry check, in ticks.
    pub expiry_check_ticks: u64,
    /// Tell online players when a temporary group or node of theirs expires.
    pub notify_expiry: bool,
    /// The export/import file, relative to the data folder.
    pub export_file: String,
}

impl Default for PermsConfig {
    fn default() -> Self {
        PermsConfig {
            lang: Lang::En,
            default_group: "default".into(),
            expiry_check_ticks: 20,
            notify_expiry: true,
            export_file: "perms.toml".into(),
        }
    }
}

/// The plugin.
#[rustic::plugin]
#[derive(Default)]
pub struct Perms {
    cfg: PermsConfig,
    data: PermData,
    loaded: bool,
    expiry_task: Option<Task>,
}

/// Reads everything from storage. Unreadable user entries are skipped with a warning.
fn load_data() -> rustic::Result<PermData> {
    let mut data = PermData {
        groups: Storage::get_or_default(GROUPS_KEY)?,
        tracks: Storage::get_or_default(TRACKS_KEY)?,
        ..PermData::default()
    };
    for key in Storage::keys(USER_PREFIX) {
        let Some(id) = key
            .strip_prefix(USER_PREFIX)
            .and_then(|s| s.parse::<Uuid>().ok())
        else {
            continue;
        };
        match Storage::get::<User>(&key) {
            Ok(Some(user)) => {
                data.users.insert(id, user);
            }
            Ok(None) => {}
            Err(e) => warn!("skipping unreadable entry {key}: {e}"),
        }
    }
    Ok(data)
}

/// `/perms ...` executors: `on!(method)` reaches the plugin state.
macro_rules! on {
    ($method:ident) => {
        |c: &mut CommandCtx| with_plugin(|p: &mut Perms| p.$method(c))
    };
}

/// The usages shown by `/perms help` (also the order of the command tree).
pub const HELP: &[&str] = &[
    "/perms user <player> info",
    "/perms user <player> group add|remove|set <group>",
    "/perms user <player> group addtemp <group> <duration>",
    "/perms user <player> permission set <node> [true|false]",
    "/perms user <player> permission settemp <node> <true|false> <duration>",
    "/perms user <player> permission unset <node>",
    "/perms user <player> promote|demote <track>",
    "/perms user <player> setprefix|setsuffix [text]",
    "/perms group <group> create [weight]",
    "/perms group <group> delete|info",
    "/perms group <group> permission set|settemp|unset <node> ...",
    "/perms group <group> parent add|remove <group>",
    "/perms group <group> setprefix|setsuffix [text]",
    "/perms group <group> setweight <weight>",
    "/perms groups",
    "/perms track <track> create|delete|info",
    "/perms track <track> append|remove <group>",
    "/perms track <track> insert <group> <position>",
    "/perms tracks",
    "/perms check <player> <node>",
    "/perms reload | export | import",
];

impl Plugin for Perms {
    fn load(&mut self, _ctx: &mut Ctx) -> Result<()> {
        Permissions::declare(ADMIN_PERMISSION)
            .default(PermissionDefault::Op)
            .description("Use /perms: manage groups, permissions and tracks")
            .register();
        Ok(())
    }

    fn enable(&mut self, ctx: &mut Ctx) -> Result<()> {
        self.cfg = Config::new().defaults(DEFAULT_CONFIG).load()?;
        self.cfg.default_group =
            normalize_name(&self.cfg.default_group).unwrap_or_else(|_| "default".into());
        if self.reload_data()? && ctx.is_main() {
            self.save_groups();
        }

        ctx.command(perms_command())?;
        let period = self.cfg.expiry_check_ticks.max(1);
        self.expiry_task = Some(Scheduler::run_timer(period, period, |_| {
            with_plugin(Perms::purge_expired)
        }));

        if ctx.is_main() {
            info!(
                "Perms enabled: {} groups, {} players, {} tracks (default group `{}`, lang {:?})",
                self.data.groups.len(),
                self.data.users.len(),
                self.data.tracks.len(),
                self.cfg.default_group,
                self.cfg.lang
            );
        }
        Ok(())
    }
}

/// The `/perms` command tree.
fn perms_command() -> Command {
    Command::new("perms")
        .alias("perm")
        .description("Manage groups, permissions and tracks")
        .permission(ADMIN_PERMISSION)
        .route("", on!(help))
        .route("help", on!(help))
        // Users
        .route("user <user:word*> info", on!(user_info))
        .route("user <user:word*> group add <group:word*>", on!(user_group_add))
        .route(
            "user <user:word*> group addtemp <group:word*> <duration:duration>",
            on!(user_group_add_temp),
        )
        .route(
            "user <user:word*> group remove <group:word*>",
            on!(user_group_remove),
        )
        .route("user <user:word*> group set <group:word*>", on!(user_group_set))
        .route(
            "user <user:word*> permission set <node:string*> [value:boolean]",
            on!(user_perm_set),
        )
        .route(
            "user <user:word*> permission settemp <node:string*> <value:boolean> <duration:duration>",
            on!(user_perm_set_temp),
        )
        .route(
            "user <user:word*> permission unset <node:string*>",
            on!(user_perm_unset),
        )
        .route("user <user:word*> promote <track:word*>", on!(user_promote))
        .route("user <user:word*> demote <track:word*>", on!(user_demote))
        .route("user <user:word*> setprefix [text:greedy]", on!(user_set_prefix))
        .route("user <user:word*> setsuffix [text:greedy]", on!(user_set_suffix))
        // Groups
        .route("group <group:word*> create [weight:integer]", on!(group_create))
        .route("group <group:word*> delete", on!(group_delete))
        .route("group <group:word*> info", on!(group_info))
        .route(
            "group <group:word*> permission set <node:string*> [value:boolean]",
            on!(group_perm_set),
        )
        .route(
            "group <group:word*> permission settemp <node:string*> <value:boolean> <duration:duration>",
            on!(group_perm_set_temp),
        )
        .route(
            "group <group:word*> permission unset <node:string*>",
            on!(group_perm_unset),
        )
        .route(
            "group <group:word*> parent add <parent:word*>",
            on!(group_parent_add),
        )
        .route(
            "group <group:word*> parent remove <parent:word*>",
            on!(group_parent_remove),
        )
        .route("group <group:word*> setprefix [text:greedy]", on!(group_set_prefix))
        .route("group <group:word*> setsuffix [text:greedy]", on!(group_set_suffix))
        .route(
            "group <group:word*> setweight <weight:integer>",
            on!(group_set_weight),
        )
        .route("groups", on!(group_list))
        // Tracks
        .route("track <track:word*> create", on!(track_create))
        .route("track <track:word*> delete", on!(track_delete))
        .route("track <track:word*> info", on!(track_info))
        .route("track <track:word*> append <group:word*>", on!(track_append))
        .route(
            "track <track:word*> insert <group:word*> <position:integer(1..1000)>",
            on!(track_insert),
        )
        .route("track <track:word*> remove <group:word*>", on!(track_remove))
        .route("tracks", on!(track_list))
        // Misc
        .route("check <user:word*> <node:string*>", on!(cmd_check))
        .route("reload", on!(cmd_reload))
        .route("export", on!(export))
        .route("import", on!(import))
        .completer(|c| with_plugin(|p: &mut Perms| p.complete(c)))
}

/// Which side of the chat meta.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Side {
    Prefix,
    Suffix,
}

impl Perms {
    // ------------------------------------------------------------------ state and storage

    fn ensure_loaded(&mut self) {
        if !self.loaded {
            self.data = load_data().unwrap_or_else(|e| {
                error!("cannot read the permission data: {e}");
                PermData::default()
            });
            self.data.ensure_default(&self.cfg.default_group);
            self.loaded = true;
        }
    }

    /// Reads the data from storage; `true` when the default group had to be created (the
    /// caller saves the groups).
    fn reload_data(&mut self) -> rustic::Result<bool> {
        self.data = load_data()?;
        self.loaded = true;
        Ok(self.data.ensure_default(&self.cfg.default_group))
    }

    fn save_groups(&self) {
        if let Err(e) = Storage::set(GROUPS_KEY, &self.data.groups) {
            error!("cannot save groups: {e}");
        }
    }

    fn save_tracks(&self) {
        if let Err(e) = Storage::set(TRACKS_KEY, &self.data.tracks) {
            error!("cannot save tracks: {e}");
        }
    }

    fn save_user(&self, id: Uuid) {
        let key = format!("{USER_PREFIX}{id}");
        match self.data.users.get(&id) {
            Some(u) => {
                if let Err(e) = Storage::set(&key, u) {
                    error!("cannot save player {id}: {e}");
                }
            }
            None => {
                Storage::delete(&key);
            }
        }
    }

    fn save_all(&self) {
        for key in Storage::keys(USER_PREFIX) {
            let keep = key
                .strip_prefix(USER_PREFIX)
                .and_then(|s| s.parse::<Uuid>().ok())
                .is_some_and(|id| self.data.users.contains_key(&id));
            if !keep {
                Storage::delete(&key);
            }
        }
        self.save_groups();
        self.save_tracks();
        for id in self.data.users.keys() {
            self.save_user(*id);
        }
    }

    fn default_group(&self) -> &str {
        &self.cfg.default_group
    }

    fn user_mut(&mut self, id: Uuid, name: &str) -> &mut User {
        let default = self.cfg.default_group.clone();
        let user = self
            .data
            .users
            .entry(id)
            .or_insert_with(|| User::new(name, &default));
        if !name.is_empty() && user.name != name {
            user.name = name.to_owned();
        }
        user
    }

    /// Removes expired temporary entries (tick timer).
    fn purge_expired(&mut self) {
        let purge = self.data.purge_expired(now_secs());
        if purge.removed.is_empty() {
            return;
        }
        for id in &purge.users {
            self.save_user(*id);
        }
        if purge.groups_changed {
            self.save_groups();
        }
        let lang = self.cfg.lang;
        for (who, what) in &purge.removed {
            match (who, what) {
                (Some(id), Expired::Group(g)) => {
                    info!("temporary group {g} of {id} expired");
                    if self.cfg.notify_expiry {
                        Player::from_uuid(*id).send_message(msg(
                            lang,
                            Msg::GroupExpired,
                            &[("group", g)],
                        ));
                    }
                }
                (Some(id), Expired::Node(n)) => {
                    info!("temporary node {n} of {id} expired");
                    if self.cfg.notify_expiry {
                        Player::from_uuid(*id).send_message(msg(
                            lang,
                            Msg::NodeExpired,
                            &[("node", n)],
                        ));
                    }
                }
                (_, Expired::GroupNode(g, n)) => info!("temporary node {n} of group {g} expired"),
                (None, _) => {}
            }
        }
    }

    // ------------------------------------------------------------------ messages

    fn lang(&self) -> Lang {
        self.cfg.lang
    }

    fn say(&self, c: &CommandCtx, key: Msg, args: &[(&str, &str)]) {
        c.reply(msg(self.lang(), key, args));
    }

    fn err(&self, key: Msg, args: &[(&str, &str)]) -> CommandError {
        CommandError::from(msg(self.lang(), key, args))
    }

    fn model_err(&self, e: ModelError) -> CommandError {
        match e {
            ModelError::UnknownGroup(g) => self.err(Msg::UnknownGroup, &[("group", &g)]),
            ModelError::UnknownTrack(t) => self.err(Msg::UnknownTrack, &[("track", &t)]),
            ModelError::InvalidNode(n) => self.err(Msg::InvalidNode, &[("node", &n)]),
            ModelError::InvalidName(n) => self.err(Msg::InvalidName, &[("name", &n)]),
            ModelError::Cycle(g, p) => self.err(Msg::Cycle, &[("group", &g), ("parent", &p)]),
            ModelError::DefaultGroup(g) => self.err(Msg::DefaultGroup, &[("group", &g)]),
            ModelError::AmbiguousTrack(t) => self.err(Msg::AmbiguousTrack, &[("track", &t)]),
            ModelError::EmptyTrack(t) => self.err(Msg::EmptyTrack, &[("track", &t)]),
        }
    }

    fn target_user_text(&self, name: &str) -> String {
        lang::fill(Msg::TargetUser.template(self.lang()), &[("name", name)])
    }

    fn target_group_text(&self, group: &str) -> String {
        lang::fill(Msg::TargetGroup.template(self.lang()), &[("group", group)])
    }

    fn none_word(&self) -> &'static str {
        Msg::None.template(self.lang())
    }

    fn bool_text(value: bool) -> &'static str {
        if value {
            "<green>true</green>"
        } else {
            "<red>false</red>"
        }
    }

    fn expiry_text(&self, expires: u64, now: u64) -> String {
        lang::fill(
            Msg::ExpiresIn.template(self.lang()),
            &[("time", &format_duration(expires.saturating_sub(now)))],
        )
    }

    // ------------------------------------------------------------------ argument helpers

    /// Online player by name, then known users, then the server's user cache, then a UUID.
    fn find_user(&self, input: &str) -> Option<(Uuid, String)> {
        if let Ok(id) = input.parse::<Uuid>() {
            let name = self
                .data
                .users
                .get(&id)
                .map(|u| u.name.clone())
                .or_else(|| Player::from_uuid(id).name())
                .unwrap_or_else(|| id.to_string());
            return Some((id, name));
        }
        if let Some(p) = Player::by_name(input) {
            return Some((p.uuid(), p.name().unwrap_or_else(|| input.to_owned())));
        }
        if let Some(id) = self.data.user_by_name(input) {
            return Some((id, self.data.users[&id].name.clone()));
        }
        Server::offline_player(input)
            .map(|p| (p.uuid(), p.name().unwrap_or_else(|| input.to_owned())))
    }

    fn user_arg(&self, c: &CommandCtx) -> Result<(Uuid, String), CommandError> {
        let input: String = c.arg("user")?;
        self.find_user(&input)
            .ok_or_else(|| self.err(Msg::UnknownUser, &[("name", &input)]))
    }

    /// A normalized name argument (group, parent, track).
    fn name_arg(&self, c: &CommandCtx, arg: &str) -> Result<String, CommandError> {
        let raw: String = c.arg(arg)?;
        normalize_name(&raw).map_err(|e| self.model_err(e))
    }

    /// An existing group.
    fn group_arg(&self, c: &CommandCtx, arg: &str) -> Result<String, CommandError> {
        let g = self.name_arg(c, arg)?;
        if self.data.groups.contains_key(&g) {
            Ok(g)
        } else {
            Err(self.err(Msg::UnknownGroup, &[("group", &g)]))
        }
    }

    /// An existing track.
    fn track_arg(&self, c: &CommandCtx) -> Result<String, CommandError> {
        let t = self.name_arg(c, "track")?;
        if self.data.tracks.contains_key(&t) {
            Ok(t)
        } else {
            Err(self.err(Msg::UnknownTrack, &[("track", &t)]))
        }
    }

    /// `(node, value)` from `<node>` and the optional/required `value` (a leading `-` negates).
    fn node_args(&self, c: &CommandCtx) -> Result<(String, bool), CommandError> {
        let raw: String = c.arg("node")?;
        let (node, positive) = parse_node_input(&raw).map_err(|e| self.model_err(e))?;
        let value = c.opt_arg::<bool>("value")?.unwrap_or(true);
        Ok((node, positive && value))
    }

    fn duration_secs(c: &CommandCtx) -> Result<u64, CommandError> {
        let Ticks(ticks) = c.arg("duration")?;
        Ok((ticks / 20).max(1))
    }

    // ------------------------------------------------------------------ help

    fn help(&mut self, c: &mut CommandCtx) -> CommandResult {
        self.say(c, Msg::HelpHeader, &[]);
        for usage in HELP {
            let fill_in = usage.split(" <").next().unwrap_or(usage);
            c.reply(
                Text::mini(&format!(
                    "<dark_gray> » </dark_gray><gray>{}",
                    escape_mini(usage)
                ))
                .suggest_command(format!("{} ", fill_in.split('|').next().unwrap_or(fill_in))),
            );
        }
        Ok(())
    }

    // ------------------------------------------------------------------ users

    fn user_info(&mut self, c: &mut CommandCtx) -> CommandResult {
        let (id, name) = self.user_arg(c)?;
        let now = now_secs();
        let lang = self.lang();
        let default = self.default_group().to_owned();
        let groups = self.data.user_groups(id, &default, now);
        c.reply(msg(
            lang,
            Msg::UserHeader,
            &[("name", &name), ("uuid", &id.to_string())],
        ));
        let primary = groups.first().cloned().unwrap_or_else(|| default.clone());
        c.reply(line(lang, Msg::PrimaryGroup, &[("group", &primary)]));

        let user = self.data.users.get(&id).cloned().unwrap_or_default();
        let mut parts: Vec<String> = Vec::new();
        for g in &groups {
            let mut part = format!(
                "<yellow><click:run_command:'/perms group {g} info'>{g}</click></yellow>",
                g = escape_mini(g)
            );
            if !user.groups.contains(g)
                && let Some(exp) = user.temp_groups.get(g)
            {
                part.push_str(&format!(
                    " <dark_gray>({})</dark_gray>",
                    escape_mini(&self.expiry_text(*exp, now))
                ));
            }
            parts.push(part);
        }
        c.reply(Text::mini(&lang::fill_raw(
            Msg::GroupsLine.template(lang),
            &[("groups", &parts.join("<gray>, </gray>"))],
        )));
        self.meta_lines(c, self.data.meta(id, false, &default, now), Side::Prefix);
        self.meta_lines(c, self.data.meta(id, true, &default, now), Side::Suffix);
        self.node_lines(c, &user.nodes, &user.temp_nodes, now);
        Ok(())
    }

    fn meta_lines(&self, c: &CommandCtx, value: Option<String>, side: Side) {
        let key = match side {
            Side::Prefix => Msg::PrefixLine,
            Side::Suffix => Msg::SuffixLine,
        };
        let shown = match value {
            Some(raw) => render_meta(&raw).append(Text::mini(&format!(
                " <dark_gray>({})</dark_gray>",
                escape_mini(&raw)
            ))),
            None => Text::mini(&format!("<dark_gray>{}", self.none_word())),
        };
        c.reply(line(self.lang(), key, &[]).append(shown));
    }

    fn node_lines(
        &self,
        c: &CommandCtx,
        nodes: &std::collections::BTreeMap<String, bool>,
        temp: &std::collections::BTreeMap<String, TempNode>,
        now: u64,
    ) {
        let active_temp: Vec<(&String, &TempNode)> =
            temp.iter().filter(|(_, t)| t.expires > now).collect();
        let count = nodes.len() + active_temp.len();
        c.reply(line(
            self.lang(),
            Msg::NodesLine,
            &[("count", &count.to_string())],
        ));
        for (node, value) in nodes {
            c.reply(Text::mini(&format!(
                "<dark_gray> - </dark_gray><yellow>{}</yellow> <gray>=</gray> {}",
                escape_mini(node),
                Self::bool_text(*value)
            )));
        }
        for (node, t) in active_temp {
            c.reply(Text::mini(&format!(
                "<dark_gray> - </dark_gray><yellow>{}</yellow> <gray>=</gray> {} <dark_gray>({})</dark_gray>",
                escape_mini(node),
                Self::bool_text(t.value),
                escape_mini(&self.expiry_text(t.expires, now))
            )));
        }
    }

    fn notify_rank(&self, id: Uuid) {
        let groups = self.data.user_groups(id, self.default_group(), now_secs());
        if let Some(primary) = groups.first() {
            Player::from_uuid(id).send_message(msg(
                self.lang(),
                Msg::YourGroupsChanged,
                &[("group", primary)],
            ));
        }
    }

    fn user_group_add(&mut self, c: &mut CommandCtx) -> CommandResult {
        let (id, name) = self.user_arg(c)?;
        let group = self.group_arg(c, "group")?;
        let user = self.user_mut(id, &name);
        user.temp_groups.remove(&group);
        if !user.groups.insert(group.clone()) {
            self.save_user(id);
            self.say(
                c,
                Msg::UserGroupAlready,
                &[("name", &name), ("group", &group)],
            );
            return Ok(());
        }
        self.save_user(id);
        self.say(
            c,
            Msg::UserGroupAdded,
            &[("name", &name), ("group", &group)],
        );
        self.notify_rank(id);
        Ok(())
    }

    fn user_group_add_temp(&mut self, c: &mut CommandCtx) -> CommandResult {
        let (id, name) = self.user_arg(c)?;
        let group = self.group_arg(c, "group")?;
        let secs = Self::duration_secs(c)?;
        let user = self.user_mut(id, &name);
        if user.groups.contains(&group) {
            self.say(
                c,
                Msg::UserGroupAlready,
                &[("name", &name), ("group", &group)],
            );
            return Ok(());
        }
        user.temp_groups.insert(group.clone(), now_secs() + secs);
        self.save_user(id);
        self.say(
            c,
            Msg::UserGroupAddedTemp,
            &[
                ("name", &name),
                ("group", &group),
                ("time", &format_duration(secs)),
            ],
        );
        self.notify_rank(id);
        Ok(())
    }

    fn user_group_remove(&mut self, c: &mut CommandCtx) -> CommandResult {
        let (id, name) = self.user_arg(c)?;
        let group = self.name_arg(c, "group")?;
        let user = self.user_mut(id, &name);
        let removed = user.groups.remove(&group) | user.temp_groups.remove(&group).is_some();
        self.save_user(id);
        if removed {
            self.say(
                c,
                Msg::UserGroupRemoved,
                &[("name", &name), ("group", &group)],
            );
            self.notify_rank(id);
        } else {
            self.say(
                c,
                Msg::UserGroupNotMember,
                &[("name", &name), ("group", &group)],
            );
        }
        Ok(())
    }

    fn user_group_set(&mut self, c: &mut CommandCtx) -> CommandResult {
        let (id, name) = self.user_arg(c)?;
        let group = self.group_arg(c, "group")?;
        let user = self.user_mut(id, &name);
        user.groups = BTreeSet::from([group.clone()]);
        user.temp_groups.clear();
        self.save_user(id);
        self.say(c, Msg::UserGroupSet, &[("name", &name), ("group", &group)]);
        self.notify_rank(id);
        Ok(())
    }

    fn user_perm_set(&mut self, c: &mut CommandCtx) -> CommandResult {
        let (id, name) = self.user_arg(c)?;
        let (node, value) = self.node_args(c)?;
        let user = self.user_mut(id, &name);
        user.temp_nodes.remove(&node);
        user.nodes.insert(node.clone(), value);
        self.save_user(id);
        let target = self.target_user_text(&name);
        c.reply(msg_with(
            self.lang(),
            Msg::PermSet,
            &[("node", &node)],
            &[("value", Self::bool_text(value)), ("target", &target)],
        ));
        Ok(())
    }

    fn user_perm_set_temp(&mut self, c: &mut CommandCtx) -> CommandResult {
        let (id, name) = self.user_arg(c)?;
        let (node, value) = self.node_args(c)?;
        let secs = Self::duration_secs(c)?;
        let expires = now_secs() + secs;
        let user = self.user_mut(id, &name);
        user.temp_nodes
            .insert(node.clone(), TempNode { value, expires });
        self.save_user(id);
        let target = self.target_user_text(&name);
        c.reply(msg_with(
            self.lang(),
            Msg::PermSetTemp,
            &[("node", &node), ("time", &format_duration(secs))],
            &[("value", Self::bool_text(value)), ("target", &target)],
        ));
        Ok(())
    }

    fn user_perm_unset(&mut self, c: &mut CommandCtx) -> CommandResult {
        let (id, name) = self.user_arg(c)?;
        let (node, _) = self.node_args(c)?;
        let target = self.target_user_text(&name);
        let removed = match self.data.users.get_mut(&id) {
            Some(u) => u.nodes.remove(&node).is_some() | u.temp_nodes.remove(&node).is_some(),
            None => false,
        };
        let key = if removed {
            self.save_user(id);
            Msg::PermUnset
        } else {
            Msg::PermNotSet
        };
        c.reply(msg_with(
            self.lang(),
            key,
            &[("node", &node)],
            &[("target", &target)],
        ));
        Ok(())
    }

    fn user_track(&mut self, c: &mut CommandCtx, up: bool) -> CommandResult {
        let (id, name) = self.user_arg(c)?;
        let track = self.track_arg(c)?;
        let default = self.default_group().to_owned();
        let outcome = self
            .data
            .move_on_track(id, &name, &track, up, &default, now_secs())
            .map_err(|e| self.model_err(e))?;
        let n = name.as_str();
        let t = track.as_str();
        match &outcome {
            TrackMove::Joined(g) => self.say(
                c,
                Msg::PromotedJoined,
                &[("name", n), ("track", t), ("group", g)],
            ),
            TrackMove::Moved { from, to } => self.say(
                c,
                if up { Msg::Promoted } else { Msg::Demoted },
                &[("name", n), ("track", t), ("from", from), ("to", to)],
            ),
            TrackMove::Left(g) => {
                self.say(
                    c,
                    Msg::DemotedLeft,
                    &[("name", n), ("track", t), ("group", g)],
                );
            }
            TrackMove::Nothing => self.say(
                c,
                if up {
                    Msg::PromoteTop
                } else {
                    Msg::DemoteNotOn
                },
                &[("name", n), ("track", t)],
            ),
        }
        if outcome != TrackMove::Nothing {
            self.save_user(id);
            self.notify_rank(id);
        }
        Ok(())
    }

    fn user_promote(&mut self, c: &mut CommandCtx) -> CommandResult {
        self.user_track(c, true)
    }

    fn user_demote(&mut self, c: &mut CommandCtx) -> CommandResult {
        self.user_track(c, false)
    }

    fn user_meta(&mut self, c: &mut CommandCtx, side: Side) -> CommandResult {
        let (id, name) = self.user_arg(c)?;
        let text: Option<String> = c.opt_arg("text")?;
        let user = self.user_mut(id, &name);
        let slot = match side {
            Side::Prefix => &mut user.prefix,
            Side::Suffix => &mut user.suffix,
        };
        slot.clone_from(&text);
        self.save_user(id);
        let target = self.target_user_text(&name);
        self.reply_meta(c, side, &target, text.as_deref());
        Ok(())
    }

    fn reply_meta(&self, c: &CommandCtx, side: Side, target: &str, value: Option<&str>) {
        let (set, cleared) = match side {
            Side::Prefix => (Msg::PrefixSet, Msg::PrefixCleared),
            Side::Suffix => (Msg::SuffixSet, Msg::SuffixCleared),
        };
        match value {
            Some(v) => c.reply(
                msg_with(self.lang(), set, &[], &[("target", target)]).append(render_meta(v)),
            ),
            None => c.reply(msg_with(self.lang(), cleared, &[], &[("target", target)])),
        }
    }

    fn user_set_prefix(&mut self, c: &mut CommandCtx) -> CommandResult {
        self.user_meta(c, Side::Prefix)
    }

    fn user_set_suffix(&mut self, c: &mut CommandCtx) -> CommandResult {
        self.user_meta(c, Side::Suffix)
    }

    // ------------------------------------------------------------------ groups

    fn group_create(&mut self, c: &mut CommandCtx) -> CommandResult {
        let group = self.name_arg(c, "group")?;
        if self.data.groups.contains_key(&group) {
            return Err(self.err(Msg::GroupExists, &[("group", &group)]));
        }
        let weight: i32 = c
            .opt_arg::<i64>("weight")?
            .unwrap_or(0)
            .clamp(-1_000_000, 1_000_000) as i32;
        self.data.groups.insert(
            group.clone(),
            Group {
                weight,
                ..Group::default()
            },
        );
        self.save_groups();
        self.say(
            c,
            Msg::GroupCreated,
            &[("group", &group), ("weight", &weight.to_string())],
        );
        Ok(())
    }

    fn group_delete(&mut self, c: &mut CommandCtx) -> CommandResult {
        let group = self.name_arg(c, "group")?;
        let default = self.default_group().to_owned();
        let members: Vec<Uuid> = self
            .data
            .users
            .iter()
            .filter(|(_, u)| u.groups.contains(&group) || u.temp_groups.contains_key(&group))
            .map(|(id, _)| *id)
            .collect();
        self.data
            .delete_group(&group, &default)
            .map_err(|e| self.model_err(e))?;
        self.save_groups();
        self.save_tracks();
        for id in members {
            self.save_user(id);
        }
        self.say(c, Msg::GroupDeleted, &[("group", &group)]);
        Ok(())
    }

    fn group_info(&mut self, c: &mut CommandCtx) -> CommandResult {
        let name = self.group_arg(c, "group")?;
        let g = self.data.groups[&name].clone();
        let lang = self.lang();
        self.say(c, Msg::GroupHeader, &[("group", &name)]);
        c.reply(line(
            lang,
            Msg::WeightLine,
            &[("weight", &g.weight.to_string())],
        ));
        let parents = if g.parents.is_empty() {
            format!("<dark_gray>{}", self.none_word())
        } else {
            g.parents
                .iter()
                .map(|p| {
                    format!(
                        "<yellow><click:run_command:'/perms group {p} info'>{p}</click></yellow>",
                        p = escape_mini(p)
                    )
                })
                .collect::<Vec<_>>()
                .join("<gray>, </gray>")
        };
        c.reply(Text::mini(&lang::fill_raw(
            Msg::ParentsLine.template(lang),
            &[("parents", &parents)],
        )));
        let members = self
            .data
            .users
            .values()
            .filter(|u| u.groups.contains(&name) || u.temp_groups.contains_key(&name))
            .count();
        c.reply(line(
            lang,
            Msg::MembersLine,
            &[("count", &members.to_string())],
        ));
        self.meta_lines(c, g.prefix.clone(), Side::Prefix);
        self.meta_lines(c, g.suffix.clone(), Side::Suffix);
        self.node_lines(c, &g.nodes, &g.temp_nodes, now_secs());
        Ok(())
    }

    fn group_perm_set(&mut self, c: &mut CommandCtx) -> CommandResult {
        let group = self.group_arg(c, "group")?;
        let (node, value) = self.node_args(c)?;
        if let Some(g) = self.data.groups.get_mut(&group) {
            g.temp_nodes.remove(&node);
            g.nodes.insert(node.clone(), value);
        }
        self.save_groups();
        let target = self.target_group_text(&group);
        c.reply(msg_with(
            self.lang(),
            Msg::PermSet,
            &[("node", &node)],
            &[("value", Self::bool_text(value)), ("target", &target)],
        ));
        Ok(())
    }

    fn group_perm_set_temp(&mut self, c: &mut CommandCtx) -> CommandResult {
        let group = self.group_arg(c, "group")?;
        let (node, value) = self.node_args(c)?;
        let secs = Self::duration_secs(c)?;
        let expires = now_secs() + secs;
        if let Some(g) = self.data.groups.get_mut(&group) {
            g.temp_nodes
                .insert(node.clone(), TempNode { value, expires });
        }
        self.save_groups();
        let target = self.target_group_text(&group);
        c.reply(msg_with(
            self.lang(),
            Msg::PermSetTemp,
            &[("node", &node), ("time", &format_duration(secs))],
            &[("value", Self::bool_text(value)), ("target", &target)],
        ));
        Ok(())
    }

    fn group_perm_unset(&mut self, c: &mut CommandCtx) -> CommandResult {
        let group = self.group_arg(c, "group")?;
        let (node, _) = self.node_args(c)?;
        let removed = self.data.groups.get_mut(&group).is_some_and(|g| {
            g.nodes.remove(&node).is_some() | g.temp_nodes.remove(&node).is_some()
        });
        if removed {
            self.save_groups();
        }
        let target = self.target_group_text(&group);
        c.reply(msg_with(
            self.lang(),
            if removed {
                Msg::PermUnset
            } else {
                Msg::PermNotSet
            },
            &[("node", &node)],
            &[("target", &target)],
        ));
        Ok(())
    }

    fn group_parent_add(&mut self, c: &mut CommandCtx) -> CommandResult {
        let group = self.group_arg(c, "group")?;
        let parent = self.group_arg(c, "parent")?;
        let added = self
            .data
            .add_parent(&group, &parent)
            .map_err(|e| self.model_err(e))?;
        if added {
            self.save_groups();
            self.say(
                c,
                Msg::ParentAdded,
                &[("group", &group), ("parent", &parent)],
            );
        } else {
            self.say(
                c,
                Msg::ParentAlready,
                &[("group", &group), ("parent", &parent)],
            );
        }
        Ok(())
    }

    fn group_parent_remove(&mut self, c: &mut CommandCtx) -> CommandResult {
        let group = self.group_arg(c, "group")?;
        let parent = self.name_arg(c, "parent")?;
        let removed = self
            .data
            .groups
            .get_mut(&group)
            .is_some_and(|g| g.parents.remove(&parent));
        if removed {
            self.save_groups();
            self.say(
                c,
                Msg::ParentRemoved,
                &[("group", &group), ("parent", &parent)],
            );
        } else {
            self.say(
                c,
                Msg::ParentNotSet,
                &[("group", &group), ("parent", &parent)],
            );
        }
        Ok(())
    }

    fn group_meta(&mut self, c: &mut CommandCtx, side: Side) -> CommandResult {
        let group = self.group_arg(c, "group")?;
        let text: Option<String> = c.opt_arg("text")?;
        if let Some(g) = self.data.groups.get_mut(&group) {
            match side {
                Side::Prefix => g.prefix.clone_from(&text),
                Side::Suffix => g.suffix.clone_from(&text),
            }
        }
        self.save_groups();
        let target = self.target_group_text(&group);
        self.reply_meta(c, side, &target, text.as_deref());
        Ok(())
    }

    fn group_set_prefix(&mut self, c: &mut CommandCtx) -> CommandResult {
        self.group_meta(c, Side::Prefix)
    }

    fn group_set_suffix(&mut self, c: &mut CommandCtx) -> CommandResult {
        self.group_meta(c, Side::Suffix)
    }

    fn group_set_weight(&mut self, c: &mut CommandCtx) -> CommandResult {
        let group = self.group_arg(c, "group")?;
        let weight = c.arg::<i64>("weight")?.clamp(-1_000_000, 1_000_000) as i32;
        if let Some(g) = self.data.groups.get_mut(&group) {
            g.weight = weight;
        }
        self.save_groups();
        self.say(
            c,
            Msg::WeightSet,
            &[("group", &group), ("weight", &weight.to_string())],
        );
        Ok(())
    }

    fn group_list(&mut self, c: &mut CommandCtx) -> CommandResult {
        let mut groups: Vec<(&String, &Group)> = self.data.groups.iter().collect();
        groups.sort_by(|a, b| b.1.weight.cmp(&a.1.weight).then(a.0.cmp(b.0)));
        self.say(c, Msg::GroupList, &[("count", &groups.len().to_string())]);
        for (name, g) in groups {
            let mut entry = Text::mini(&format!(
                "<dark_gray> - </dark_gray><yellow>{}</yellow> <dark_gray>(w{})</dark_gray> ",
                escape_mini(name),
                g.weight
            ));
            if let Some(p) = &g.prefix {
                entry = entry.append(render_meta(p));
            }
            c.reply(
                entry
                    .hover(Text::mini(&format!(
                        "<gray>/perms group {} info",
                        escape_mini(name)
                    )))
                    .run_command(format!("/perms group {name} info")),
            );
        }
        Ok(())
    }

    // ------------------------------------------------------------------ tracks

    fn track_create(&mut self, c: &mut CommandCtx) -> CommandResult {
        let track = self.name_arg(c, "track")?;
        if self.data.tracks.contains_key(&track) {
            return Err(self.err(Msg::TrackExists, &[("track", &track)]));
        }
        self.data.tracks.insert(track.clone(), Track::default());
        self.save_tracks();
        self.say(c, Msg::TrackCreated, &[("track", &track)]);
        Ok(())
    }

    fn track_delete(&mut self, c: &mut CommandCtx) -> CommandResult {
        let track = self.track_arg(c)?;
        self.data.tracks.remove(&track);
        self.save_tracks();
        self.say(c, Msg::TrackDeleted, &[("track", &track)]);
        Ok(())
    }

    fn track_line(&self, track: &str) -> Text {
        let groups = &self.data.tracks[track].groups;
        let shown = if groups.is_empty() {
            format!("<dark_gray>{}", self.none_word())
        } else {
            groups
                .iter()
                .map(|g| format!("<yellow>{}</yellow>", escape_mini(g)))
                .collect::<Vec<_>>()
                .join("<dark_gray> → </dark_gray>")
        };
        Text::mini(&lang::fill_raw(
            &lang::fill(Msg::TrackInfo.template(self.lang()), &[("track", track)]),
            &[("groups", &shown)],
        ))
    }

    fn track_info(&mut self, c: &mut CommandCtx) -> CommandResult {
        let track = self.track_arg(c)?;
        c.reply(Text::mini(lang::HEADER).append(self.track_line(&track)));
        Ok(())
    }

    fn track_append(&mut self, c: &mut CommandCtx) -> CommandResult {
        self.track_add(c, None)
    }

    fn track_insert(&mut self, c: &mut CommandCtx) -> CommandResult {
        let position: usize = c.arg("position")?;
        self.track_add(c, Some(position))
    }

    fn track_add(&mut self, c: &mut CommandCtx, position: Option<usize>) -> CommandResult {
        let track = self.track_arg(c)?;
        let group = self.group_arg(c, "group")?;
        let Some(Track { groups: ladder }) = self.data.tracks.get_mut(&track) else {
            return Err(self.err(Msg::UnknownTrack, &[("track", &track)]));
        };
        if ladder.contains(&group) {
            return Err(self.err(Msg::AlreadyOnTrack, &[("group", &group), ("track", &track)]));
        }
        match position {
            None => ladder.push(group.clone()),
            Some(p) => {
                let idx = p.saturating_sub(1).min(ladder.len());
                ladder.insert(idx, group.clone());
            }
        }
        let at = ladder.iter().position(|g| *g == group).unwrap_or(0) + 1;
        self.save_tracks();
        match position {
            None => self.say(
                c,
                Msg::TrackAppended,
                &[("group", &group), ("track", &track)],
            ),
            Some(_) => self.say(
                c,
                Msg::TrackInserted,
                &[
                    ("group", &group),
                    ("track", &track),
                    ("position", &at.to_string()),
                ],
            ),
        }
        Ok(())
    }

    fn track_remove(&mut self, c: &mut CommandCtx) -> CommandResult {
        let track = self.track_arg(c)?;
        let group = self.name_arg(c, "group")?;
        let Some(Track { groups: ladder }) = self.data.tracks.get_mut(&track) else {
            return Err(self.err(Msg::UnknownTrack, &[("track", &track)]));
        };
        let before = ladder.len();
        ladder.retain(|g| *g != group);
        if ladder.len() == before {
            return Err(self.err(Msg::NotOnTrack, &[("group", &group), ("track", &track)]));
        }
        self.save_tracks();
        self.say(
            c,
            Msg::TrackRemoved,
            &[("group", &group), ("track", &track)],
        );
        Ok(())
    }

    fn track_list(&mut self, c: &mut CommandCtx) -> CommandResult {
        self.say(
            c,
            Msg::TrackList,
            &[("count", &self.data.tracks.len().to_string())],
        );
        for name in self.data.tracks.keys() {
            c.reply(
                Text::mini("<dark_gray> - </dark_gray>")
                    .append(self.track_line(name))
                    .run_command(format!("/perms track {name} info")),
            );
        }
        Ok(())
    }

    // ------------------------------------------------------------------ misc

    fn cmd_check(&mut self, c: &mut CommandCtx) -> CommandResult {
        let (id, name) = self.user_arg(c)?;
        let raw: String = c.arg("node")?;
        let node = raw.trim().to_ascii_lowercase();
        model::validate_node(&node).map_err(|e| self.model_err(e))?;
        let v = self
            .data
            .resolve(id, &node, self.default_group(), now_secs());
        match v.value {
            Some(value) => c.reply(msg_with(
                self.lang(),
                Msg::CheckResult,
                &[
                    ("name", &name),
                    ("node", &node),
                    ("source", v.source.as_deref().unwrap_or("?")),
                    ("key", v.key.as_deref().unwrap_or("?")),
                ],
                &[("value", Self::bool_text(value))],
            )),
            None => self.say(c, Msg::CheckUndefined, &[("name", &name), ("node", &node)]),
        }
        Ok(())
    }

    fn cmd_reload(&mut self, c: &mut CommandCtx) -> CommandResult {
        let cfg: PermsConfig = Config::new().defaults(DEFAULT_CONFIG).load()?;
        let period_changed = cfg.expiry_check_ticks != self.cfg.expiry_check_ticks;
        self.cfg = cfg;
        self.cfg.default_group =
            normalize_name(&self.cfg.default_group).unwrap_or_else(|_| "default".into());
        if self.reload_data()? {
            self.save_groups();
        }
        if period_changed {
            if let Some(t) = self.expiry_task.take() {
                t.cancel();
            }
            let period = self.cfg.expiry_check_ticks.max(1);
            self.expiry_task = Some(Scheduler::run_timer(period, period, |_| {
                with_plugin(Perms::purge_expired)
            }));
        }
        self.counts_message(c, Msg::Reloaded, None);
        Ok(())
    }

    fn counts_message(&self, c: &CommandCtx, key: Msg, file: Option<&str>) {
        self.say(
            c,
            key,
            &[
                ("groups", &self.data.groups.len().to_string()),
                ("users", &self.data.users.len().to_string()),
                ("tracks", &self.data.tracks.len().to_string()),
                ("file", file.unwrap_or("")),
            ],
        );
    }

    fn export_path(&self) -> String {
        format!(
            "{}{}",
            rustic::config::DATA_DIR,
            self.cfg.export_file.trim_start_matches('/')
        )
    }

    fn export(&mut self, c: &mut CommandCtx) -> CommandResult {
        let file = self.cfg.export_file.clone();
        let file_err = |e: String| self.err(Msg::FileError, &[("file", &file), ("error", &e)]);
        let body =
            rustic::toml::to_string_pretty(&self.data).map_err(|e| file_err(e.to_string()))?;
        let text = format!(
            "# Exported by the Perms plugin. Edit, then run /perms import to load it.\n\
             # Nodes: true grants, false negates; temp_* entries expire at a unix time (seconds).\n\n{body}"
        );
        std::fs::write(self.export_path(), text).map_err(|e| file_err(e.to_string()))?;
        self.counts_message(c, Msg::Exported, Some(&file));
        Ok(())
    }

    fn import(&mut self, c: &mut CommandCtx) -> CommandResult {
        let file = self.cfg.export_file.clone();
        let file_err = |e: String| self.err(Msg::FileError, &[("file", &file), ("error", &e)]);
        let text =
            std::fs::read_to_string(self.export_path()).map_err(|e| file_err(e.to_string()))?;
        let mut data: PermData =
            rustic::toml::from_str(&text).map_err(|e| file_err(e.to_string()))?;
        data.validate().map_err(file_err)?;
        data.ensure_default(&self.cfg.default_group);
        self.data = data;
        self.save_all();
        self.counts_message(c, Msg::Imported, Some(&file));
        Ok(())
    }

    // ------------------------------------------------------------------ completion

    fn complete(&mut self, c: &CompleteCtx) -> Vec<String> {
        match c.argument() {
            "user" => {
                let mut names: BTreeSet<String> =
                    Player::online().iter().filter_map(Player::name).collect();
                names.extend(self.data.users.values().map(|u| u.name.clone()));
                c.suggest(names)
            }
            "group" | "parent" => c.suggest(self.data.groups.keys().cloned()),
            "track" => c.suggest(self.data.tracks.keys().cloned()),
            "node" => {
                let mut nodes: BTreeSet<String> = BTreeSet::from(["*".to_owned()]);
                for g in self.data.groups.values() {
                    nodes.extend(g.nodes.keys().cloned());
                    nodes.extend(g.temp_nodes.keys().cloned());
                }
                for u in self.data.users.values() {
                    nodes.extend(u.nodes.keys().cloned());
                    nodes.extend(u.temp_nodes.keys().cloned());
                }
                nodes.extend(self.data.groups.keys().map(|g| format!("group.{g}")));
                c.suggest(nodes)
            }
            _ => Vec::new(),
        }
    }
}

#[rustic::handlers]
impl Perms {
    /// Remembers names (for offline lookups) and puts new players in the default group.
    #[event_handler(priority = "lowest")]
    fn on_join(&mut self, event: &mut PlayerJoinEvent) {
        self.ensure_loaded();
        let id = event.player().uuid();
        let name = event.name().to_owned();
        let changed = match self.data.users.get(&id) {
            Some(u) => u.name != name,
            None => true,
        };
        if changed {
            self.user_mut(id, &name);
            self.save_user(id);
        }
    }
}

fn tristate(v: Option<bool>) -> Tristate {
    match v {
        Some(true) => Tristate::Allow,
        Some(false) => Tristate::Deny,
        None => Tristate::Undefined,
    }
}

impl ProvidePermissions for Perms {
    const PRIORITY: Priority = Priority::High;

    fn check(&mut self, player: Uuid, node: &str) -> ServiceResult<Tristate> {
        self.ensure_loaded();
        let v = self
            .data
            .resolve(player, node, &self.cfg.default_group, now_secs());
        Ok(tristate(v.value))
    }

    fn groups(&mut self, player: Uuid) -> ServiceResult<Vec<String>> {
        self.ensure_loaded();
        Ok(self
            .data
            .user_groups(player, &self.cfg.default_group, now_secs()))
    }

    fn add_group(&mut self, player: Uuid, group: &str) -> ServiceResult<()> {
        self.ensure_loaded();
        let group = normalize_name(group)
            .ok()
            .filter(|g| self.data.groups.contains_key(g))
            .ok_or_else(|| ServiceError::Refused(format!("unknown group `{group}`")))?;
        let name = Player::from_uuid(player).name().unwrap_or_default();
        let user = self.user_mut(player, &name);
        user.temp_groups.remove(&group);
        user.groups.insert(group);
        self.save_user(player);
        Ok(())
    }

    fn remove_group(&mut self, player: Uuid, group: &str) -> ServiceResult<()> {
        self.ensure_loaded();
        let group = group.to_ascii_lowercase();
        if let Some(u) = self.data.users.get_mut(&player) {
            let removed = u.groups.remove(&group) | u.temp_groups.remove(&group).is_some();
            if removed {
                self.save_user(player);
            }
        }
        Ok(())
    }

    fn set_node(&mut self, player: Uuid, node: &str, value: Tristate) -> ServiceResult<()> {
        self.ensure_loaded();
        let (node, positive) = parse_node_input(node)
            .map_err(|_| ServiceError::Refused(format!("invalid node `{node}`")))?;
        let name = Player::from_uuid(player).name().unwrap_or_default();
        let user = self.user_mut(player, &name);
        user.temp_nodes.remove(&node);
        match value {
            Tristate::Allow => {
                user.nodes.insert(node, positive);
            }
            Tristate::Deny => {
                user.nodes.insert(node, false);
            }
            Tristate::Undefined => {
                user.nodes.remove(&node);
            }
        }
        self.save_user(player);
        Ok(())
    }
}

impl ProvideChatMeta for Perms {
    const PRIORITY: Priority = Priority::High;

    fn prefix(&mut self, player: Uuid) -> ServiceResult<Option<String>> {
        self.ensure_loaded();
        Ok(self
            .data
            .meta(player, false, &self.cfg.default_group, now_secs()))
    }

    fn suffix(&mut self, player: Uuid) -> ServiceResult<Option<String>> {
        self.ensure_loaded();
        Ok(self
            .data
            .meta(player, true, &self.cfg.default_group, now_secs()))
    }

    fn group_prefix(&mut self, group: &str) -> ServiceResult<Option<String>> {
        self.ensure_loaded();
        Ok(self
            .data
            .groups
            .get(&group.to_ascii_lowercase())
            .and_then(|g| g.prefix.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_config_matches_defaults() {
        let (cfg, rewrite) = Config::new()
            .defaults(DEFAULT_CONFIG)
            .load_from_str::<PermsConfig>(Some(DEFAULT_CONFIG))
            .unwrap();
        assert_eq!(rewrite, None);
        assert_eq!(cfg, PermsConfig::default());
    }

    #[test]
    fn french_config() {
        let (cfg, _) = Config::new()
            .defaults(DEFAULT_CONFIG)
            .load_from_str::<PermsConfig>(Some("config_version = 1\nlang = \"fr\"\n"))
            .unwrap();
        assert_eq!(cfg.lang, Lang::Fr);
        assert_eq!(cfg.default_group, "default");
    }

    #[test]
    fn command_tree_builds() {
        // Every usage parses and has an executor (pure; no host call).
        let cmd = perms_command();
        assert_eq!(cmd.name(), "perms");
        assert!(format!("{cmd:?}").contains("user <user:word*> info"));
    }

    #[test]
    fn tristate_mapping() {
        assert_eq!(tristate(Some(true)), Tristate::Allow);
        assert_eq!(tristate(Some(false)), Tristate::Deny);
        assert_eq!(tristate(None), Tristate::Undefined);
    }

    #[test]
    fn provider_answers_from_data() {
        let mut p = Perms {
            loaded: true,
            ..Perms::default()
        };
        p.data.ensure_default("default");
        p.data
            .groups
            .get_mut("default")
            .unwrap()
            .nodes
            .insert("essentials.*".into(), true);
        p.data.groups.get_mut("default").unwrap().prefix = Some("&7".into());
        let id = Uuid::from_u128(1);
        assert_eq!(
            ProvidePermissions::check(&mut p, id, "essentials.home").unwrap(),
            Tristate::Allow
        );
        assert_eq!(
            ProvidePermissions::check(&mut p, id, "other").unwrap(),
            Tristate::Undefined
        );
        assert_eq!(p.groups(id).unwrap(), vec!["default"]);
        assert_eq!(p.primary_group(id).unwrap().as_deref(), Some("default"));
        assert_eq!(p.prefix(id).unwrap().as_deref(), Some("&7"));
        assert_eq!(p.suffix(id).unwrap(), None);
        assert_eq!(p.group_prefix("DEFAULT").unwrap().as_deref(), Some("&7"));
    }
}
