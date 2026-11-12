//! essentials-lite: the everyday commands of a survival server.
//!
//! * `/spawn`, `/setspawn`; `/home`, `/sethome`, `/delhome`, `/homes` (limit per permission
//!   `essentials.homes.<n>`, optional economy cost); `/warp`, `/setwarp`, `/delwarp`, `/warps`;
//! * `/tpa`, `/tpahere`, `/tpaccept`, `/tpdeny` (requests expire, clickable buttons);
//! * `/back` (last location before a teleport or a death);
//! * `/msg` + `/r` (with the chat-meta prefix when a provider is installed), `/broadcast`;
//! * teleport warmup (cancelled when the player moves) and cooldown.
//!
//! Homes and the `/back` location live in per-player data; spawn and warps in the plugin's
//! storage. Messages in English or French.

pub mod lang;
pub mod logic;

use std::collections::{BTreeMap, HashMap};

use rustic::config::Config;
use rustic::permissions::Permissions;
use rustic::prelude::*;
use rustic::text::escape_mini;
use serde::{Deserialize, Serialize};

use crate::lang::{Lang, Msg, meta_to_mini, msg, msg_with, part};
use crate::logic::{Loc, NoRequest, Request, Requests, max_homes, normalize_name, seconds_left};

/// The packaged default configuration.
pub const DEFAULT_CONFIG: &str = include_str!("../config.toml");

/// Storage key of the spawn.
pub const SPAWN_KEY: &str = "spawn";
/// Storage key of the warps.
pub const WARPS_KEY: &str = "warps";
/// Per-player data key of the homes.
pub const HOMES_KEY: &str = "homes";
/// Per-player data key of the `/back` location.
pub const BACK_KEY: &str = "back";

/// Permission nodes and their defaults (`(node, default, description)`).
pub const PERMISSIONS: &[(&str, PermissionDefault, &str)] = &[
    ("essentials.spawn", PermissionDefault::Allow, "Use /spawn"),
    (
        "essentials.setspawn",
        PermissionDefault::Op,
        "Use /setspawn",
    ),
    (
        "essentials.home",
        PermissionDefault::Allow,
        "Use /home and /homes",
    ),
    (
        "essentials.sethome",
        PermissionDefault::Allow,
        "Use /sethome and /delhome",
    ),
    (
        "essentials.homes.unlimited",
        PermissionDefault::Op,
        "No limit on homes",
    ),
    (
        "essentials.sethome.free",
        PermissionDefault::Op,
        "New homes cost nothing",
    ),
    ("essentials.tpa", PermissionDefault::Allow, "Use /tpa"),
    (
        "essentials.tpahere",
        PermissionDefault::Allow,
        "Use /tpahere",
    ),
    (
        "essentials.tpaccept",
        PermissionDefault::Allow,
        "Use /tpaccept and /tpdeny",
    ),
    ("essentials.back", PermissionDefault::Allow, "Use /back"),
    (
        "essentials.warp",
        PermissionDefault::Allow,
        "Use /warp and /warps",
    ),
    ("essentials.setwarp", PermissionDefault::Op, "Use /setwarp"),
    ("essentials.delwarp", PermissionDefault::Op, "Use /delwarp"),
    (
        "essentials.msg",
        PermissionDefault::Allow,
        "Use /msg and /r",
    ),
    (
        "essentials.broadcast",
        PermissionDefault::Op,
        "Use /broadcast",
    ),
    (
        "essentials.warmup.bypass",
        PermissionDefault::Op,
        "Teleport without warmup",
    ),
    (
        "essentials.cooldown.bypass",
        PermissionDefault::Op,
        "Teleport without cooldown",
    ),
];

/// `config.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct EssConfig {
    /// Message language.
    pub lang: Lang,
    /// Seconds to stand still before a teleport (0 = instant).
    pub teleport_warmup_seconds: u64,
    /// Moving farther than this (blocks) during the warmup cancels the teleport.
    pub warmup_cancel_distance: f64,
    /// Seconds between two teleports of the same player (0 = none).
    pub teleport_cooldown_seconds: u64,
    /// Seconds before a `/tpa` request expires.
    pub tpa_timeout_seconds: u64,
    /// Homes everyone may set.
    pub default_max_homes: u32,
    /// Tiers granted by `essentials.homes.<n>`.
    pub home_tiers: Vec<u32>,
    /// Price of a new home in the economy's minor units (0 = free; ignored without economy).
    pub sethome_cost: i64,
    /// Require `essentials.warps.<name>` for each warp.
    pub per_warp_permissions: bool,
    /// `/broadcast` line; `{message}` is the text (MiniMessage allowed).
    pub broadcast_format: String,
    /// Tell players how to `/back` after they die.
    pub back_on_death_hint: bool,
}

impl Default for EssConfig {
    fn default() -> Self {
        EssConfig {
            lang: Lang::En,
            teleport_warmup_seconds: 3,
            warmup_cancel_distance: 0.5,
            teleport_cooldown_seconds: 5,
            tpa_timeout_seconds: 60,
            default_max_homes: 1,
            home_tiers: vec![3, 5, 10],
            sethome_cost: 0,
            per_warp_permissions: false,
            broadcast_format: "<dark_gray>[<gold>Broadcast</gold>]</dark_gray> <white>{message}"
                .into(),
            back_on_death_hint: true,
        }
    }
}

/// A teleport waiting for its warmup.
#[derive(Debug, Clone)]
struct Warmup {
    id: u64,
    start: Loc,
    target: Loc,
    place: String,
    task: Task,
}

/// The plugin.
#[rustic::plugin]
#[derive(Default)]
pub struct Essentials {
    cfg: EssConfig,
    spawn: Option<Loc>,
    warps: BTreeMap<String, Loc>,
    requests: Requests,
    warmups: HashMap<Uuid, Warmup>,
    next_warmup: u64,
    cooldowns: HashMap<Uuid, u64>,
    back: HashMap<Uuid, Loc>,
    reply: HashMap<Sender, Sender>,
}

impl Plugin for Essentials {
    fn load(&mut self, _ctx: &mut Ctx) -> Result<()> {
        for (node, default, description) in PERMISSIONS {
            Permissions::declare(*node)
                .default(*default)
                .description(*description)
                .register();
        }
        Ok(())
    }

    fn enable(&mut self, ctx: &mut Ctx) -> Result<()> {
        self.cfg = Config::new().defaults(DEFAULT_CONFIG).load()?;
        for n in &self.cfg.home_tiers {
            Permissions::declare(format!("essentials.homes.{n}"))
                .default(PermissionDefault::Deny)
                .description(format!("Up to {n} homes"))
                .register();
        }
        self.spawn = Storage::get(SPAWN_KEY)?;
        self.warps = Storage::get_or_default(WARPS_KEY)?;
        if ctx.is_main() {
            info!(
                "essentials-lite enabled: {} warps, spawn {}, warmup {}s, cooldown {}s, lang {:?}",
                self.warps.len(),
                if self.spawn.is_some() {
                    "set"
                } else {
                    "from the world"
                },
                self.cfg.teleport_warmup_seconds,
                self.cfg.teleport_cooldown_seconds,
                self.cfg.lang
            );
        }
        Ok(())
    }
}

impl Essentials {
    // ------------------------------------------------------------------ messages

    fn lang(&self) -> Lang {
        self.cfg.lang
    }

    fn err(&self, key: Msg, args: &[(&str, &str)]) -> CommandError {
        CommandError::from(msg(self.lang(), key, args))
    }

    fn player(&self, cmd: &CommandCtx) -> std::result::Result<Player, CommandError> {
        cmd.sender()
            .as_player()
            .ok_or_else(|| self.err(Msg::PlayersOnly, &[]))
    }

    fn name_of(p: Player) -> String {
        p.name().unwrap_or_else(|| p.uuid().to_string())
    }

    fn place(&self, key: Msg, name: &str) -> String {
        part(self.lang(), key, &[("name", name)])
    }

    fn name_arg(&self, raw: &str) -> std::result::Result<String, CommandError> {
        normalize_name(raw).ok_or_else(|| self.err(Msg::InvalidName, &[("name", raw)]))
    }

    // ------------------------------------------------------------------ teleports

    /// Teleports `p` to `target` after the warmup (unless bypassed), honouring the cooldown.
    fn teleport(&mut self, p: Player, target: Loc, place: String) -> CommandResult {
        let now = Scheduler::current_tick();
        let id = p.uuid();
        if let Some(until) = self.cooldowns.get(&id)
            && *until > now
            && !p.has_permission("essentials.cooldown.bypass")
        {
            return Err(self.err(
                Msg::Cooldown,
                &[("seconds", &seconds_left(*until, now).to_string())],
            ));
        }
        let warmup = self.cfg.teleport_warmup_seconds;
        if warmup == 0 || p.has_permission("essentials.warmup.bypass") {
            return self.do_teleport(p, &target, &place);
        }
        if let Some(old) = self.warmups.remove(&id) {
            old.task.cancel();
            p.send_message(msg(self.lang(), Msg::TeleportReplaced, &[]));
        }
        let start = Loc::from(&p.location()?);
        self.next_warmup += 1;
        let warmup_id = self.next_warmup;
        let task = Scheduler::run_later(warmup * 20, move || {
            with_plugin(|e: &mut Essentials| e.finish_warmup(id, warmup_id));
        });
        p.send_message(msg_with(
            self.lang(),
            Msg::TeleportingIn,
            &[("seconds", &warmup.to_string())],
            &[("place", &place)],
        ));
        self.warmups.insert(
            id,
            Warmup {
                id: warmup_id,
                start,
                target,
                place,
                task,
            },
        );
        Ok(())
    }

    fn finish_warmup(&mut self, id: Uuid, warmup_id: u64) {
        if !self.warmups.get(&id).is_some_and(|w| w.id == warmup_id) {
            return;
        }
        let Some(w) = self.warmups.remove(&id) else {
            return;
        };
        let p = Player::from_uuid(id);
        if let Err(e) = self.do_teleport(p, &w.target, &w.place) {
            p.send_message(e.message().clone());
        }
    }

    fn do_teleport(&mut self, p: Player, target: &Loc, place: &str) -> CommandResult {
        if let Ok(here) = p.location() {
            self.set_back(p, Loc::from(&here));
        }
        p.teleport(&target.to_location()).map_err(|e| {
            self.err(
                Msg::TeleportFailed,
                &[("error", &CommandError::from(e).to_string())],
            )
        })?;
        if self.cfg.teleport_cooldown_seconds > 0 {
            self.cooldowns.insert(
                p.uuid(),
                Scheduler::current_tick() + self.cfg.teleport_cooldown_seconds * 20,
            );
        }
        p.send_message(msg_with(
            self.lang(),
            Msg::Teleported,
            &[],
            &[("place", place)],
        ));
        Ok(())
    }

    fn set_back(&mut self, p: Player, loc: Loc) {
        if let Err(e) = p.data().set(BACK_KEY, &loc) {
            warn!("cannot save the /back location: {e}");
        }
        self.back.insert(p.uuid(), loc);
    }

    fn back_of(&self, p: Player) -> Option<Loc> {
        self.back
            .get(&p.uuid())
            .cloned()
            .or_else(|| p.data().get(BACK_KEY).ok().flatten())
    }

    fn cancel_warmup(&mut self, id: Uuid) -> bool {
        match self.warmups.remove(&id) {
            Some(w) => {
                w.task.cancel();
                true
            }
            None => false,
        }
    }

    // ------------------------------------------------------------------ homes

    fn homes(p: Player) -> BTreeMap<String, Loc> {
        p.data().get(HOMES_KEY).ok().flatten().unwrap_or_default()
    }

    fn max_homes(&self, p: Player) -> Option<u32> {
        max_homes(self.cfg.default_max_homes, &self.cfg.home_tiers, |n| {
            p.has_permission(n)
        })
    }

    fn loc_list_entry(&self, cmd: &str, name: &str, loc: &Loc) -> Text {
        let (x, y, z) = loc.block_coords();
        let hover = lang::fill(
            Msg::ClickToTeleport.template(self.lang()),
            &[
                ("name", name),
                ("world", loc.world_short()),
                ("x", &x.to_string()),
                ("y", &y.to_string()),
                ("z", &z.to_string()),
            ],
        );
        Text::mini(&format!("<yellow>[{}]</yellow>", escape_mini(name)))
            .hover(Text::mini(&hover))
            .run_command(format!("/{cmd} {name}"))
    }

    fn clickable_list(&self, cmd: &str, entries: &BTreeMap<String, Loc>) -> Text {
        let mut line = Text::empty();
        for (i, (name, loc)) in entries.iter().enumerate() {
            if i > 0 {
                line = line.append(" ");
            }
            line = line.append(self.loc_list_entry(cmd, name, loc));
        }
        line
    }

    fn charge_for_home(&self, p: Player) -> CommandResult {
        let cost = self.cfg.sethome_cost;
        if cost <= 0 || p.has_permission("essentials.sethome.free") || !Economy::is_available() {
            return Ok(());
        }
        let shown = Economy::format(cost).unwrap_or_else(|_| cost.to_string());
        match Economy::withdraw(p, cost) {
            Ok(_) => {
                p.send_message(msg(self.lang(), Msg::HomeCharged, &[("cost", &shown)]));
                Ok(())
            }
            Err(e) => Err(self.err(
                Msg::HomeTooExpensive,
                &[
                    ("cost", &shown),
                    ("reason", &CommandError::from(e).to_string()),
                ],
            )),
        }
    }

    // ------------------------------------------------------------------ requests

    fn send_request(&mut self, cmd: &CommandCtx, here: bool) -> CommandResult {
        let p = self.player(cmd)?;
        let target: Player = cmd.arg("target")?;
        if target == p {
            return Err(self.err(Msg::TpaSelf, &[]));
        }
        let timeout = self.cfg.tpa_timeout_seconds.max(1);
        let expires = Scheduler::current_tick() + timeout * 20;
        let id = self.requests.add(p.uuid(), target.uuid(), here, expires);
        Scheduler::run_later(timeout * 20, move || {
            with_plugin(|e: &mut Essentials| e.expire_request(id));
        });
        let (from_name, to_name) = (Self::name_of(p), Self::name_of(target));
        let secs = timeout.to_string();
        let lang = self.lang();
        target.send_message(msg(
            lang,
            if here {
                Msg::TpaHereReceived
            } else {
                Msg::TpaReceived
            },
            &[("name", &from_name)],
        ));
        target.send_message(msg(
            lang,
            Msg::TpaButtons,
            &[("name", &from_name), ("seconds", &secs)],
        ));
        target.sound("minecraft:block.note_block.pling", 1.0, 1.0);
        cmd.reply(msg(
            lang,
            Msg::TpaSent,
            &[("name", &to_name), ("seconds", &secs)],
        ));
        Ok(())
    }

    fn expire_request(&mut self, id: u64) {
        if let Some(r) = self.requests.remove(id) {
            self.tell_expired(&r);
        }
    }

    fn tell_expired(&self, r: &Request) {
        let (from, to) = (Player::from_uuid(r.from), Player::from_uuid(r.to));
        from.send_message(msg(
            self.lang(),
            Msg::TpaExpired,
            &[("name", &Self::name_of(to))],
        ));
        to.send_message(msg(
            self.lang(),
            Msg::TpaExpiredTarget,
            &[("name", &Self::name_of(from))],
        ));
    }

    /// The request `/tpaccept [player]` / `/tpdeny [player]` refers to.
    fn take_request(&mut self, cmd: &CommandCtx) -> std::result::Result<Request, CommandError> {
        let p = self.player(cmd)?;
        let now = Scheduler::current_tick();
        for r in self.requests.expire(now) {
            self.tell_expired(&r);
        }
        let name: Option<String> = cmd.opt_arg("player")?;
        let from = match &name {
            Some(n) => match Player::by_name(n) {
                Some(f) => Some(f.uuid()),
                None => return Err(self.err(Msg::TpaNoneFrom, &[("name", n)])),
            },
            None => None,
        };
        self.requests
            .take(p.uuid(), from, now)
            .map_err(|e| match e {
                NoRequest::None => self.err(Msg::TpaNone, &[]),
                NoRequest::NoneFrom => {
                    self.err(Msg::TpaNoneFrom, &[("name", name.as_deref().unwrap_or(""))])
                }
            })
    }

    // ------------------------------------------------------------------ private messages

    fn display_name(&self, s: Sender) -> String {
        match s {
            Sender::Console => "<white>Console</white>".to_owned(),
            Sender::Player(p) => {
                let name = format!("<white>{}</white>", escape_mini(&Self::name_of(p)));
                // Degrades to the bare name without a chat-meta provider.
                match ChatMeta::prefix(p) {
                    Ok(Some(prefix)) if !prefix.trim().is_empty() => {
                        // Command arguments lose trailing spaces: separate prefix and name.
                        let gap = if prefix.ends_with(' ') { "" } else { " " };
                        format!("{}<reset>{gap}{name}", meta_to_mini(&prefix))
                    }
                    _ => name,
                }
            }
        }
    }

    fn send_private(&mut self, from: Sender, to: Sender, message: &str) {
        let lang = self.lang();
        let me = format!("<white>{}</white>", Msg::Me.template(lang));
        let text = escape_mini(message);
        let (from_shown, to_shown) = (self.display_name(from), self.display_name(to));
        let reply_to = match from {
            Sender::Console => "console".to_owned(),
            Sender::Player(p) => Self::name_of(p),
        };
        let line = |a: &str, b: &str| {
            lang::fill_raw(
                Msg::MsgFormat.template(lang),
                &[("from", a), ("to", b), ("message", &text)],
            )
        };
        from.send_message(Text::mini(&line(&me, &to_shown)));
        let received = Text::mini(&line(&from_shown, &me));
        let received = if from.is_console() {
            received
        } else {
            received
                .hover(Text::mini(Msg::MsgHover.template(lang)))
                .suggest_command(format!("/m {reply_to} "))
        };
        to.send_message(received);
        if let Sender::Player(p) = to {
            p.sound("minecraft:entity.experience_orb.pickup", 0.5, 1.5);
        }
        self.reply.insert(to, from);
        self.reply.insert(from, to);
    }
}

#[rustic::handlers]
impl Essentials {
    // ------------------------------------------------------------------ events

    /// Cancels a pending warmup when the player moves too far.
    #[event_handler(priority = "monitor", ignore_cancelled = true)]
    fn on_move(&mut self, event: &mut PlayerMoveEvent) {
        let id = event.player().uuid();
        let moved = self.warmups.get(&id).is_some_and(|w| {
            w.start
                .moved_beyond(event.to(), self.cfg.warmup_cancel_distance)
        });
        if moved && self.cancel_warmup(id) {
            event
                .player()
                .send_message(msg(self.lang(), Msg::TeleportCancelled, &[]));
        }
    }

    /// Any teleport (other plugins, vanilla `/tp`) sets the `/back` location.
    #[event_handler(priority = "monitor")]
    fn on_teleport(&mut self, event: &mut PlayerTeleportEvent) {
        let from = Loc::from(event.from());
        self.set_back(event.player(), from);
    }

    /// Dying sets the `/back` location.
    #[event_handler(priority = "monitor")]
    fn on_death(&mut self, event: &mut PlayerDeathEvent) {
        let p = event.player();
        if let Ok(here) = p.location() {
            self.set_back(p, Loc::from(&here));
            if self.cfg.back_on_death_hint && p.has_permission("essentials.back") {
                p.send_message(msg(self.lang(), Msg::BackOnDeath, &[]));
            }
        }
    }

    /// Forgets what belongs to a leaving player.
    #[event_handler]
    fn on_quit(&mut self, event: &mut PlayerQuitEvent) {
        let id = event.player().uuid();
        self.cancel_warmup(id);
        self.requests.drop_player(id);
        self.back.remove(&id);
    }

    // ------------------------------------------------------------------ spawn

    #[command(
        name = "spawn",
        permission = "essentials.spawn",
        players_only,
        description = "Teleport to the spawn"
    )]
    fn cmd_spawn(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        let p = self.player(cmd)?;
        let target = match &self.spawn {
            Some(l) => l.clone(),
            None => {
                let l = World::overworld()
                    .spawn_location()
                    .map_err(|_| self.err(Msg::NoSpawn, &[]))?;
                Loc::from(&l)
            }
        };
        let place = Msg::PlaceSpawn.template(self.lang()).to_owned();
        self.teleport(p, target, place)
    }

    #[command(
        name = "setspawn",
        permission = "essentials.setspawn",
        players_only,
        description = "Set the spawn to where you stand"
    )]
    fn cmd_setspawn(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        let p = self.player(cmd)?;
        let here = p.location()?;
        let loc = Loc::from(&here);
        Storage::set(SPAWN_KEY, &loc)?;
        // Also the world's own spawn (compasses, new players); not fatal when refused.
        if let Err(e) = here.world().set_spawn_location(&here) {
            warn!("world spawn not updated: {e}");
        }
        let (x, y, z) = loc.block_coords();
        cmd.reply(msg(
            self.lang(),
            Msg::SpawnSet,
            &[
                ("x", &x.to_string()),
                ("y", &y.to_string()),
                ("z", &z.to_string()),
                ("world", loc.world_short()),
            ],
        ));
        self.spawn = Some(loc);
        Ok(())
    }

    // ------------------------------------------------------------------ homes

    #[command(
        name = "home",
        aliases = ["h"],
        permission = "essentials.home",
        usage = "[name:word*]",
        players_only,
        description = "Teleport to one of your homes"
    )]
    fn cmd_home(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        let p = self.player(cmd)?;
        let homes = Self::homes(p);
        let name = match cmd.opt_arg::<String>("name")? {
            Some(raw) => self.name_arg(&raw)?,
            None if homes.len() == 1 => homes.keys().next().cloned().unwrap_or_default(),
            None if homes.is_empty() => {
                return Err(self.err(Msg::NoHomes, &[]));
            }
            None if homes.contains_key("home") => "home".to_owned(),
            None => {
                // Several homes and none called "home": show the list.
                return self.cmd_homes(cmd);
            }
        };
        let Some(loc) = homes.get(&name).cloned() else {
            return Err(self.err(Msg::UnknownHome, &[("name", &name)]));
        };
        let place = self.place(Msg::PlaceHome, &name);
        self.teleport(p, loc, place)
    }

    #[completer("home")]
    fn complete_home(&mut self, ctx: &CompleteCtx) -> Vec<String> {
        match ctx.sender().as_player() {
            Some(p) => ctx.suggest(Self::homes(p).into_keys()),
            None => Vec::new(),
        }
    }

    #[command(
        name = "sethome",
        permission = "essentials.sethome",
        usage = "[name:word]",
        players_only,
        description = "Set a home where you stand"
    )]
    fn cmd_sethome(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        let p = self.player(cmd)?;
        let name = match cmd.opt_arg::<String>("name")? {
            Some(raw) => self.name_arg(&raw)?,
            None => "home".to_owned(),
        };
        let mut homes = Self::homes(p);
        let existing = homes.contains_key(&name);
        if !existing {
            if let Some(max) = self.max_homes(p)
                && homes.len() >= max as usize
            {
                return Err(self.err(Msg::HomeLimit, &[("max", &max.to_string())]));
            }
            self.charge_for_home(p)?;
        }
        homes.insert(name.clone(), Loc::from(&p.location()?));
        p.data().set(HOMES_KEY, &homes)?;
        cmd.reply(msg(
            self.lang(),
            if existing {
                Msg::HomeMoved
            } else {
                Msg::HomeSet
            },
            &[("name", &name)],
        ));
        Ok(())
    }

    #[command(
        name = "delhome",
        permission = "essentials.sethome",
        usage = "<name:word*>",
        players_only,
        description = "Delete one of your homes"
    )]
    fn cmd_delhome(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        let p = self.player(cmd)?;
        let raw: String = cmd.arg("name")?;
        let name = self.name_arg(&raw)?;
        let mut homes = Self::homes(p);
        if homes.remove(&name).is_none() {
            return Err(self.err(Msg::UnknownHome, &[("name", &name)]));
        }
        p.data().set(HOMES_KEY, &homes)?;
        cmd.reply(msg(self.lang(), Msg::HomeDeleted, &[("name", &name)]));
        Ok(())
    }

    #[completer("delhome")]
    fn complete_delhome(&mut self, ctx: &CompleteCtx) -> Vec<String> {
        self.complete_home(ctx)
    }

    #[command(
        name = "homes",
        permission = "essentials.home",
        players_only,
        description = "List your homes (click one to go)"
    )]
    fn cmd_homes(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        let p = self.player(cmd)?;
        let homes = Self::homes(p);
        if homes.is_empty() {
            cmd.reply(msg(self.lang(), Msg::NoHomes, &[]));
            return Ok(());
        }
        let max = match self.max_homes(p) {
            Some(m) => m.to_string(),
            None => Msg::Unlimited.template(self.lang()).to_owned(),
        };
        cmd.reply(msg(
            self.lang(),
            Msg::HomesList,
            &[("count", &homes.len().to_string()), ("max", &max)],
        ));
        cmd.reply(self.clickable_list("home", &homes));
        Ok(())
    }

    // ------------------------------------------------------------------ warps

    #[command(
        name = "warp",
        permission = "essentials.warp",
        usage = "<name:word*>",
        players_only,
        description = "Teleport to a warp"
    )]
    fn cmd_warp(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        let p = self.player(cmd)?;
        let raw: String = cmd.arg("name")?;
        let name = self.name_arg(&raw)?;
        let Some(loc) = self.warps.get(&name).cloned() else {
            return Err(self.err(Msg::UnknownWarp, &[("name", &name)]));
        };
        if self.cfg.per_warp_permissions && !p.has_permission(&format!("essentials.warps.{name}")) {
            return Err(self.err(Msg::NoWarpPermission, &[("name", &name)]));
        }
        let place = self.place(Msg::PlaceWarp, &name);
        self.teleport(p, loc, place)
    }

    #[completer("warp")]
    fn complete_warp(&mut self, ctx: &CompleteCtx) -> Vec<String> {
        ctx.suggest(self.warps.keys().cloned())
    }

    #[command(
        name = "setwarp",
        permission = "essentials.setwarp",
        usage = "<name:word>",
        players_only,
        description = "Create or move a warp where you stand"
    )]
    fn cmd_setwarp(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        let p = self.player(cmd)?;
        let raw: String = cmd.arg("name")?;
        let name = self.name_arg(&raw)?;
        self.warps.insert(name.clone(), Loc::from(&p.location()?));
        Storage::set(WARPS_KEY, &self.warps)?;
        cmd.reply(msg(self.lang(), Msg::WarpSet, &[("name", &name)]));
        Ok(())
    }

    #[command(
        name = "delwarp",
        permission = "essentials.delwarp",
        usage = "<name:word*>",
        description = "Delete a warp"
    )]
    fn cmd_delwarp(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        let raw: String = cmd.arg("name")?;
        let name = self.name_arg(&raw)?;
        if self.warps.remove(&name).is_none() {
            return Err(self.err(Msg::UnknownWarp, &[("name", &name)]));
        }
        Storage::set(WARPS_KEY, &self.warps)?;
        cmd.reply(msg(self.lang(), Msg::WarpDeleted, &[("name", &name)]));
        Ok(())
    }

    #[completer("delwarp")]
    fn complete_delwarp(&mut self, ctx: &CompleteCtx) -> Vec<String> {
        self.complete_warp(ctx)
    }

    #[command(
        name = "warps",
        permission = "essentials.warp",
        description = "List the warps (click one to go)"
    )]
    fn cmd_warps(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        let sender = cmd.sender();
        let visible: BTreeMap<String, Loc> = self
            .warps
            .iter()
            .filter(|(name, _)| {
                !self.cfg.per_warp_permissions
                    || sender.has_permission(&format!("essentials.warps.{name}"))
            })
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        if visible.is_empty() {
            cmd.reply(msg(self.lang(), Msg::NoWarps, &[]));
            return Ok(());
        }
        cmd.reply(msg(
            self.lang(),
            Msg::WarpsList,
            &[("count", &visible.len().to_string())],
        ));
        cmd.reply(self.clickable_list("warp", &visible));
        Ok(())
    }

    // ------------------------------------------------------------------ teleport requests

    #[command(
        name = "tpa",
        permission = "essentials.tpa",
        usage = "<target:player>",
        players_only,
        description = "Ask to teleport to a player"
    )]
    fn cmd_tpa(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        self.send_request(cmd, false)
    }

    #[command(
        name = "tpahere",
        permission = "essentials.tpahere",
        usage = "<target:player>",
        players_only,
        description = "Ask a player to teleport to you"
    )]
    fn cmd_tpahere(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        self.send_request(cmd, true)
    }

    #[command(
        name = "tpaccept",
        aliases = ["tpyes"],
        permission = "essentials.tpaccept",
        usage = "[player:word*]",
        players_only,
        description = "Accept a teleport request"
    )]
    fn cmd_tpaccept(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        let r = self.take_request(cmd)?;
        let (traveller, dest) = (
            Player::from_uuid(r.traveller()),
            Player::from_uuid(r.destination()),
        );
        let requester = Player::from_uuid(r.from);
        let me = Player::from_uuid(r.to);
        if !requester.is_online() {
            return Err(self.err(Msg::PlayerOffline, &[("name", &Self::name_of(requester))]));
        }
        let lang = self.lang();
        cmd.reply(msg(
            lang,
            Msg::TpaAccepted,
            &[("name", &Self::name_of(requester))],
        ));
        requester.send_message(msg(
            lang,
            Msg::TpaAcceptedOther,
            &[("name", &Self::name_of(me))],
        ));
        let target = Loc::from(&dest.location()?);
        let place = self.place(Msg::PlacePlayer, &Self::name_of(dest));
        if let Err(e) = self.teleport(traveller, target, place) {
            traveller.send_message(e.message().clone());
        }
        Ok(())
    }

    #[completer("tpaccept")]
    fn complete_tpaccept(&mut self, ctx: &CompleteCtx) -> Vec<String> {
        let Some(p) = ctx.sender().as_player() else {
            return Vec::new();
        };
        let now = Scheduler::current_tick();
        ctx.suggest(
            self.requests
                .pending_for(p.uuid(), now)
                .into_iter()
                .filter_map(|r| Player::from_uuid(r.from).name()),
        )
    }

    #[command(
        name = "tpdeny",
        aliases = ["tpno"],
        permission = "essentials.tpaccept",
        usage = "[player:word*]",
        players_only,
        description = "Deny a teleport request"
    )]
    fn cmd_tpdeny(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        let r = self.take_request(cmd)?;
        let (requester, me) = (Player::from_uuid(r.from), Player::from_uuid(r.to));
        cmd.reply(msg(
            self.lang(),
            Msg::TpaDenied,
            &[("name", &Self::name_of(requester))],
        ));
        requester.send_message(msg(
            self.lang(),
            Msg::TpaDeniedOther,
            &[("name", &Self::name_of(me))],
        ));
        Ok(())
    }

    #[completer("tpdeny")]
    fn complete_tpdeny(&mut self, ctx: &CompleteCtx) -> Vec<String> {
        self.complete_tpaccept(ctx)
    }

    // ------------------------------------------------------------------ back

    #[command(
        name = "back",
        permission = "essentials.back",
        players_only,
        description = "Return to your previous location"
    )]
    fn cmd_back(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        let p = self.player(cmd)?;
        let Some(loc) = self.back_of(p) else {
            return Err(self.err(Msg::BackNone, &[]));
        };
        let place = Msg::PlaceBack.template(self.lang()).to_owned();
        self.teleport(p, loc, place)
    }

    // ------------------------------------------------------------------ messages

    #[command(
        name = "msg",
        aliases = ["m", "pm", "whisper"],
        permission = "essentials.msg",
        usage = "<target:player> <message:greedy>",
        description = "Send a private message"
    )]
    fn cmd_msg(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        let target: Player = cmd.arg("target")?;
        let message: String = cmd.arg("message")?;
        let from = cmd.sender();
        if from == Sender::Player(target) {
            return Err(self.err(Msg::MsgSelf, &[]));
        }
        self.send_private(from, Sender::Player(target), &message);
        Ok(())
    }

    #[command(
        name = "r",
        aliases = ["reply"],
        permission = "essentials.msg",
        usage = "<message:greedy>",
        description = "Reply to the last private message"
    )]
    fn cmd_reply(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        let message: String = cmd.arg("message")?;
        let from = cmd.sender();
        let Some(to) = self.reply.get(&from).copied() else {
            return Err(self.err(Msg::MsgNoReply, &[]));
        };
        if let Sender::Player(p) = to
            && !p.is_online()
        {
            return Err(self.err(Msg::PlayerOffline, &[("name", &Self::name_of(p))]));
        }
        self.send_private(from, to, &message);
        Ok(())
    }

    #[command(
        name = "broadcast",
        aliases = ["bc"],
        permission = "essentials.broadcast",
        usage = "<message:greedy>",
        description = "Announce a message to everyone (MiniMessage allowed)"
    )]
    fn cmd_broadcast(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        let message: String = cmd.arg("message")?;
        // The sender holds essentials.broadcast (op by default): MiniMessage tags are allowed.
        let line = lang::fill_raw(&self.cfg.broadcast_format, &[("message", &message)]);
        Server::broadcast(Text::mini(&line));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipped_config_matches_defaults() {
        let (cfg, rewrite) = Config::new()
            .defaults(DEFAULT_CONFIG)
            .load_from_str::<EssConfig>(Some(DEFAULT_CONFIG))
            .unwrap();
        assert_eq!(rewrite, None);
        assert_eq!(cfg, EssConfig::default());
    }

    #[test]
    fn partial_admin_config_keeps_defaults() {
        let (cfg, _) = Config::new()
            .defaults(DEFAULT_CONFIG)
            .load_from_str::<EssConfig>(Some(
                "config_version = 1\nlang = \"fr\"\nteleport_warmup_seconds = 0\n",
            ))
            .unwrap();
        assert_eq!(cfg.lang, Lang::Fr);
        assert_eq!(cfg.teleport_warmup_seconds, 0);
        assert_eq!(cfg.tpa_timeout_seconds, 60);
    }

    #[test]
    fn permission_table_is_consistent() {
        let mut seen = std::collections::BTreeSet::new();
        for (node, _, desc) in PERMISSIONS {
            assert!(node.starts_with("essentials."), "{node}");
            assert!(!desc.is_empty());
            assert!(seen.insert(*node), "duplicate {node}");
        }
    }
}
