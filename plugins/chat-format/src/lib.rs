//! chat-format: chat line formats, colours, mentions, anti-spam, filters and mutes.
//!
//! * Formats every chat line from a template (`{prefix}{name}{suffix}: {message}`), per group,
//!   with prefixes from the `chat-meta` service when a permissions plugin provides one; the name
//!   shows player info on hover and suggests `/msg <name> ` on click.
//! * Colours and decorations in messages (MiniMessage-like tags and `&` codes) for players with
//!   `chatformat.color`; everyone else's text is shown literally.
//! * `@name` mentions are highlighted; the mentioned player hears a sound.
//! * Anti-spam: rate limit, duplicate detection, caps limit, link filter with a whitelist and a
//!   word filter (mask or block).
//! * `/mute <player> [duration] [reason]`, `/unmute <player>`, `/mutes`: persisted in storage,
//!   expiring on time.
//! * Join, first-join and quit lines.
//!
//! Messages are in English or French (the client's language, or forced in the config).

mod config;
mod filter;
mod format;
mod lang;
mod mute;

use std::time::{SystemTime, UNIX_EPOCH};

use rustic::command::Sender;
use rustic::permissions::Permissions;
use rustic::prelude::*;
use rustic::text::escape_mini;

use crate::config::{ChatConfig, DEFAULT_CONFIG, GroupConfig};
use crate::filter::{Action, SpamGuard, SpamVerdict};
use crate::lang::{Lang, Msg, tr};
use crate::mute::{MuteLength, MuteRecord, Mutes};

/// Permission nodes.
pub mod perms {
    /// Colours and decorations in messages.
    pub const COLOR: &str = "chatformat.color";
    /// May mention players with `@name`.
    pub const MENTION: &str = "chatformat.mention";
    /// `/mute`, `/unmute`, `/mutes`.
    pub const MUTE: &str = "chatformat.mute";
    /// Cannot be muted.
    pub const MUTE_EXEMPT: &str = "chatformat.mute.exempt";
    /// Told when someone is muted.
    pub const MUTE_NOTIFY: &str = "chatformat.mute.notify";
    /// `/chatformat reload`.
    pub const ADMIN: &str = "chatformat.admin";
    /// Skips the rate limit and duplicate check.
    pub const BYPASS_SPAM: &str = "chatformat.bypass.spam";
    /// Skips the caps limit.
    pub const BYPASS_CAPS: &str = "chatformat.bypass.caps";
    /// Skips the link filter.
    pub const BYPASS_LINKS: &str = "chatformat.bypass.links";
    /// Skips the word filter.
    pub const BYPASS_FILTER: &str = "chatformat.bypass.filter";
}

/// Per-player data key: the player joined before.
const SEEN_KEY: &str = "seen";

/// Milliseconds since the Unix epoch (WASI wall clock).
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// A formatted message waiting for the monitor handler (sounds only play if nobody cancelled).
#[derive(Debug)]
struct Pending {
    sender: Player,
    mentioned: Vec<Player>,
}

#[rustic::plugin]
#[derive(Default)]
struct ChatFormat {
    cfg: ChatConfig,
    spam: SpamGuard,
    mutes: Mutes,
    pending: Option<Pending>,
}

impl Plugin for ChatFormat {
    fn load(&mut self, _ctx: &mut Ctx) -> Result<()> {
        use PermissionDefault::{Allow, Op};
        for (node, default, description) in [
            (perms::COLOR, Op, "Use colours and decorations in chat"),
            (perms::MENTION, Allow, "Mention players with @name"),
            (perms::MUTE, Op, "Use /mute, /unmute and /mutes"),
            (perms::MUTE_EXEMPT, Op, "Cannot be muted"),
            (perms::MUTE_NOTIFY, Op, "Be told when someone is muted"),
            (perms::ADMIN, Op, "Reload the chat-format configuration"),
            (
                perms::BYPASS_SPAM,
                Op,
                "Skip the chat rate limit and duplicate check",
            ),
            (perms::BYPASS_CAPS, Op, "Skip the capital letters limit"),
            (perms::BYPASS_LINKS, Op, "Post links in chat"),
            (perms::BYPASS_FILTER, Op, "Skip the forbidden words filter"),
        ] {
            Permissions::declare(node)
                .default(default)
                .description(description)
                .register();
        }
        Ok(())
    }

    fn enable(&mut self, ctx: &mut Ctx) -> Result<()> {
        // Everything here serves the main instance (chat, commands); the async one idles.
        if !ctx.is_main() {
            return Ok(());
        }
        self.cfg = rustic::config::Config::new()
            .defaults(DEFAULT_CONFIG)
            .load()?;
        self.load_mutes();
        // Lift expired mutes and tell the players, every 5 seconds.
        Scheduler::run_timer(100, 100, |_| with_plugin(ChatFormat::sweep_mutes));
        info!(
            "chat-format enabled: {} group formats, {} active mutes, chat-meta provider: {}",
            self.cfg.groups.len(),
            self.mutes.len(),
            ChatMeta::is_available()
        );
        Ok(())
    }
}

impl ChatFormat {
    fn load_mutes(&mut self) {
        self.mutes = Mutes::default();
        for key in Storage::keys(mute::KEY_PREFIX) {
            let Some(id) = key.strip_prefix(mute::KEY_PREFIX) else {
                continue;
            };
            let Ok(uuid) = id.parse::<Uuid>() else {
                continue;
            };
            match Storage::get::<MuteRecord>(&key) {
                Ok(Some(rec)) => self.mutes.insert(uuid, rec),
                Ok(None) => {}
                Err(e) => warn!("ignoring unreadable mute record {key}: {e}"),
            }
        }
    }

    fn sweep_mutes(&mut self) {
        for (uuid, _) in self.mutes.take_expired(now_ms()) {
            Storage::delete(&mute::key(uuid));
            let player = Player::from_uuid(uuid);
            if player.is_online() {
                player.send_message(tr(self.lang_for(player), Msg::YouWereUnmuted, &[]));
            }
        }
    }

    /// The language for a player's messages.
    fn lang_for(&self, player: Player) -> Lang {
        match Lang::from_code(&self.cfg.language) {
            Some(l) => l,
            None => player
                .locale()
                .ok()
                .and_then(|l| Lang::from_code(&l))
                .unwrap_or(Lang::En),
        }
    }

    fn lang_for_sender(&self, sender: Sender) -> Lang {
        match sender {
            Sender::Player(p) => self.lang_for(p),
            Sender::Console => Lang::from_code(&self.cfg.language).unwrap_or(Lang::En),
        }
    }

    /// The configured group a player belongs to.
    fn group_of(&self, player: Player) -> (String, Option<&GroupConfig>) {
        let find = |name: &str| {
            self.cfg
                .groups
                .iter()
                .find(|g| g.name.eq_ignore_ascii_case(name))
        };
        // A permissions plugin knows the groups.
        if let Ok(Some(primary)) = PermissionProvider::primary_group(player) {
            if let Some(g) = find(&primary) {
                return (g.name.clone(), Some(g));
            }
            if let Ok(groups) = PermissionProvider::groups(player)
                && let Some(g) = self
                    .cfg
                    .groups
                    .iter()
                    .find(|g| groups.iter().any(|n| n.eq_ignore_ascii_case(&g.name)))
            {
                return (g.name.clone(), Some(g));
            }
            return (primary, None);
        }
        // Otherwise a permission per group, first match wins.
        for g in &self.cfg.groups {
            if player.has_permission(&format!("chatformat.group.{}", g.name)) {
                return (g.name.clone(), Some(g));
            }
        }
        (self.cfg.format.default_group.clone(), None)
    }

    /// Prefix and suffix as MiniMessage-like text: the chat-meta service first, then the group,
    /// then the defaults. Legacy codes are converted.
    fn affixes(&self, player: Player, group: Option<&GroupConfig>) -> (String, String) {
        let pick =
            |service: ServiceResult<Option<String>>, from_group: Option<&str>, default: &str| {
                let chosen = match service {
                    Ok(Some(s)) if !s.is_empty() => s,
                    _ => match from_group {
                        Some(g) if !g.is_empty() => g.to_owned(),
                        _ => default.to_owned(),
                    },
                };
                format::legacy_to_mini(&chosen)
            };
        let prefix = pick(
            ChatMeta::prefix(player),
            group.map(|g| g.prefix.as_str()),
            &self.cfg.format.default_prefix,
        );
        let suffix = pick(
            ChatMeta::suffix(player),
            group.map(|g| g.suffix.as_str()),
            &self.cfg.format.default_suffix,
        );
        (prefix, suffix)
    }

    /// Builds the displayed line; returns it with the mentioned players (sender excluded).
    fn render_line(&self, player: Player, name: &str, text: &str) -> (Text, Vec<Player>) {
        let (group, group_cfg) = self.group_of(player);
        let template = group_cfg
            .map(|g| g.format.as_str())
            .filter(|f| !f.is_empty())
            .unwrap_or(&self.cfg.format.default);
        let (prefix, suffix) = self.affixes(player, group_cfg);
        let world = player
            .world()
            .map(|w| w.id().trim_start_matches("minecraft:").to_owned())
            .unwrap_or_default();
        let ping = player
            .ping()
            .map(|p| p.to_string())
            .unwrap_or_else(|_| "?".into());
        let uuid = player.uuid().to_string();
        let (e_name, e_group, e_world) =
            (escape_mini(name), escape_mini(&group), escape_mini(&world));
        let info = [
            ("name", e_name.as_str()),
            ("group", e_group.as_str()),
            ("world", e_world.as_str()),
            ("uuid", uuid.as_str()),
            ("ping", ping.as_str()),
        ];
        let hover = format::fill(&self.cfg.format.name_hover, &info);
        let click = format::fill(&self.cfg.format.name_click, &[("name", name)]);
        let name_part = format::interactive_name(name, &hover, &click);

        let body = if player.has_permission(perms::COLOR) {
            format::sanitize_colored(text)
        } else {
            format::sanitize_plain(text)
        };
        let (body, names) = if self.cfg.mentions.enabled && player.has_permission(perms::MENTION) {
            format::apply_mentions(&body, &self.cfg.mentions.format, |typed| {
                Player::by_name(typed).and_then(|p| p.name())
            })
        } else {
            (body, Vec::new())
        };
        let mentioned = names
            .iter()
            .filter_map(|n| Player::by_name(n))
            .filter(|p| *p != player)
            .collect();
        let line = format::fill(
            template,
            &[
                ("prefix", prefix.as_str()),
                ("suffix", suffix.as_str()),
                ("name", name_part.as_str()),
                ("message", body.as_str()),
                ("group", e_group.as_str()),
                ("world", e_world.as_str()),
            ],
        );
        (Text::mini(&line), mentioned)
    }

    /// Refuses a chat message: cancel + tell the sender.
    fn refuse(&self, event: &mut PlayerChatEvent, msg: Msg, vars: &[(&str, &str)]) {
        event.cancel();
        let p = event.player();
        p.send_message(tr(self.lang_for(p), msg, vars));
    }

    /// The active mute of a player; an expired one is lifted on the way.
    fn active_mute(&mut self, player: Player, now: u64) -> Option<MuteRecord> {
        if let Some(m) = self.mutes.active(player.uuid(), now) {
            return Some(m.clone());
        }
        if self.mutes.remove(player.uuid()).is_some() {
            Storage::delete(&mute::key(player.uuid()));
        }
        None
    }

    fn tell_muted(&self, player: Player, m: &MuteRecord, now: u64) {
        let lang = self.lang_for(player);
        let text = match m.remaining(now) {
            None => tr(lang, Msg::MutedPerm, &[("reason", &m.reason)]),
            Some(ms) => tr(
                lang,
                Msg::MutedTemp,
                &[("time", &mute::format_duration(ms)), ("reason", &m.reason)],
            ),
        };
        player.send_message(text);
    }

    /// A join or quit line from a template: `vanilla` keeps the server's, empty hides it.
    fn line(template: &str, name: &str) -> Option<Option<Text>> {
        match template.trim() {
            "vanilla" => None,
            "" => Some(None),
            t => Some(Some(Text::mini(&format::fill(
                t,
                &[("name", &escape_mini(name))],
            )))),
        }
    }
}

/// Resolves a player by name: online first, then anyone who ever joined.
fn resolve_player(name: &str) -> Option<Player> {
    Player::by_name(name).or_else(|| Server::offline_player(name))
}

#[rustic::handlers]
impl ChatFormat {
    #[event_handler(priority = "high", ignore_cancelled = true)]
    fn on_chat(&mut self, event: &mut PlayerChatEvent) {
        let player = event.player();
        let raw = event.message().to_owned();
        let now = now_ms();
        self.pending = None;

        if let Some(m) = self.active_mute(player, now) {
            event.cancel();
            self.tell_muted(player, &m, now);
            return;
        }

        let limits = self.cfg.spam.limits();
        if !player.has_permission(perms::BYPASS_SPAM) {
            match self.spam.check(player.uuid(), &raw, now, limits) {
                SpamVerdict::Ok => {}
                SpamVerdict::TooFast(s) => {
                    return self.refuse(event, Msg::TooFast, &[("seconds", &s.to_string())]);
                }
                SpamVerdict::Duplicate => return self.refuse(event, Msg::Duplicate, &[]),
            }
        }

        let mut text = raw.clone();
        let words = &self.cfg.words;
        if !player.has_permission(perms::BYPASS_FILTER)
            && filter::has_banned_word(&text, &words.list)
        {
            if words.action == Action::Block {
                return self.refuse(event, Msg::Word, &[]);
            }
            text = filter::mask_words(&text, &words.list, words.mask_char);
        }

        let links = &self.cfg.links;
        if links.enabled
            && !player.has_permission(perms::BYPASS_LINKS)
            && !filter::forbidden_links(&text, &links.whitelist).is_empty()
        {
            if links.action == Action::Block {
                return self.refuse(event, Msg::Link, &[]);
            }
            text = filter::mask_links(&text, &links.whitelist, &links.replacement);
        }

        let caps = &self.cfg.caps;
        if caps.enabled
            && !player.has_permission(perms::BYPASS_CAPS)
            && filter::is_shouting(&text, caps.min_letters, caps.max_ratio)
        {
            if caps.action == Action::Block {
                return self.refuse(event, Msg::Caps, &[]);
            }
            text = text.to_lowercase();
        }

        self.spam.record(player.uuid(), &raw, now, limits);
        let name = event.name().to_owned();
        let (line, mentioned) = self.render_line(player, &name, &text);
        event.set_display(line);
        self.pending = Some(Pending {
            sender: player,
            mentioned,
        });
    }

    /// Mention sounds, once the outcome is final (another plugin may cancel after us).
    #[event_handler(priority = "monitor")]
    fn on_chat_done(&mut self, event: &mut PlayerChatEvent) {
        let Some(p) = self.pending.take() else { return };
        if event.is_cancelled() || p.sender != event.player() {
            return;
        }
        let m = &self.cfg.mentions;
        for target in p.mentioned {
            if !m.sound.is_empty() {
                target.sound(&m.sound, m.volume, m.pitch);
            }
            if m.action_bar {
                let lang = self.lang_for(target);
                target.action_bar(tr(lang, Msg::Mentioned, &[("player", event.name())]));
            }
        }
    }

    #[event_handler(priority = "high")]
    fn on_join(&mut self, event: &mut PlayerJoinEvent) -> Result<()> {
        let data = event.player().data();
        let first = !data.get::<bool>(SEEN_KEY)?.unwrap_or(false);
        if first {
            data.set(SEEN_KEY, &true)?;
        }
        let j = &self.cfg.join;
        let template = if first && !j.first_message.trim().is_empty() {
            &j.first_message
        } else {
            &j.message
        };
        match Self::line(template, event.name()) {
            None => {}
            Some(None) => event.hide_join_message(),
            Some(Some(t)) => event.set_join_message(t),
        }
        Ok(())
    }

    #[event_handler(priority = "high")]
    fn on_quit(&mut self, event: &mut PlayerQuitEvent) {
        self.spam.forget(event.player().uuid());
        match Self::line(&self.cfg.join.quit_message, event.name()) {
            None => {}
            Some(None) => event.hide_quit_message(),
            Some(Some(t)) => event.set_quit_message(t),
        }
    }

    /// Muted players cannot whisper around the mute.
    #[event_handler(priority = "low", ignore_cancelled = true)]
    fn on_command(&mut self, event: &mut PlayerCommandEvent) {
        let first = event
            .command()
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();
        let label = first.rsplit(':').next().unwrap_or("").to_owned();
        if !self
            .cfg
            .mute
            .blocked_commands
            .iter()
            .any(|c| c.eq_ignore_ascii_case(&label))
        {
            return;
        }
        let player = event.player();
        if self.active_mute(player, now_ms()).is_some() {
            event.cancel();
            player.send_message(tr(self.lang_for(player), Msg::CommandBlockedMuted, &[]));
        }
    }

    #[command(
        name = "mute",
        aliases = ["cmute"],
        permission = "chatformat.mute",
        usage = "<player:word*> [duration:word] [reason:greedy]",
        description = "Mute a player: /mute <player> [10m|2h|1d|perm] [reason]"
    )]
    fn mute_cmd(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        let lang = self.lang_for_sender(cmd.sender());
        let typed: String = cmd.arg("player")?;
        let target = resolve_player(&typed)
            .ok_or_else(|| tr(lang, Msg::UnknownPlayer, &[("player", &typed)]))?;
        let name = target.name().unwrap_or_else(|| typed.clone());
        if target.is_online() && target.has_permission(perms::MUTE_EXEMPT) {
            return Err(tr(lang, Msg::CannotMute, &[("player", &name)]).into());
        }
        let duration: Option<String> = cmd.opt_arg("duration")?;
        let reason: Option<String> = cmd.opt_arg("reason")?;
        let (length, reason) = mute::split_duration_reason(duration.as_deref(), reason.as_deref());
        let now = now_ms();
        let until = match length {
            MuteLength::Permanent => None,
            MuteLength::For(ms) => Some(now.saturating_add(ms)),
        };
        let by = cmd.sender().name();
        let record = MuteRecord {
            name: name.clone(),
            until,
            reason: reason.clone(),
            by: by.clone(),
            at: now,
        };
        Storage::set(&mute::key(target.uuid()), &record)?;
        self.mutes.insert(target.uuid(), record);

        let time_for = |l: Lang| match length {
            MuteLength::Permanent => template_word(l, Msg::Permanent),
            MuteLength::For(ms) => mute::format_duration(ms),
        };
        cmd.reply(match length {
            MuteLength::Permanent => tr(
                lang,
                Msg::MuteDonePerm,
                &[("player", &name), ("reason", &reason)],
            ),
            MuteLength::For(_) => tr(
                lang,
                Msg::MuteDoneTemp,
                &[
                    ("player", &name),
                    ("time", &time_for(lang)),
                    ("reason", &reason),
                ],
            ),
        });
        if target.is_online() {
            let tl = self.lang_for(target);
            target.send_message(match length {
                MuteLength::Permanent => tr(
                    tl,
                    Msg::YouWereMutedPerm,
                    &[("by", &by), ("reason", &reason)],
                ),
                MuteLength::For(_) => tr(
                    tl,
                    Msg::YouWereMutedTemp,
                    &[("by", &by), ("time", &time_for(tl)), ("reason", &reason)],
                ),
            });
        }
        let sender = cmd.sender().as_player();
        for staff in Player::online() {
            if Some(staff) != sender && staff != target && staff.has_permission(perms::MUTE_NOTIFY)
            {
                let sl = self.lang_for(staff);
                staff.send_message(tr(
                    sl,
                    Msg::MuteNotify,
                    &[
                        ("by", &by),
                        ("player", &name),
                        ("time", &time_for(sl)),
                        ("reason", &reason),
                    ],
                ));
            }
        }
        Ok(())
    }

    #[command(
        name = "unmute",
        aliases = ["cunmute"],
        permission = "chatformat.mute",
        usage = "<player:word*>",
        description = "Let a muted player chat again"
    )]
    fn unmute_cmd(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        let lang = self.lang_for_sender(cmd.sender());
        let typed: String = cmd.arg("player")?;
        // A muted player may be offline and unknown to the user cache: also match stored names.
        let uuid = resolve_player(&typed).map(|p| p.uuid()).or_else(|| {
            self.mutes
                .list(0)
                .into_iter()
                .find(|(_, m)| m.name.eq_ignore_ascii_case(&typed))
                .map(|(u, _)| u)
        });
        let Some(uuid) = uuid else {
            return Err(tr(lang, Msg::UnknownPlayer, &[("player", &typed)]).into());
        };
        let removed = self.mutes.remove(uuid);
        let existed = Storage::delete(&mute::key(uuid));
        let target = Player::from_uuid(uuid);
        let name = target
            .name()
            .or(removed.as_ref().map(|m| m.name.clone()))
            .unwrap_or(typed);
        if removed.is_none() && !existed {
            return Err(tr(lang, Msg::NotMuted, &[("player", &name)]).into());
        }
        cmd.reply(tr(lang, Msg::Unmuted, &[("player", &name)]));
        if target.is_online() {
            target.send_message(tr(self.lang_for(target), Msg::YouWereUnmuted, &[]));
        }
        Ok(())
    }

    #[command(
        name = "mutes",
        aliases = ["cmutes"],
        permission = "chatformat.mute",
        description = "List muted players"
    )]
    fn mutes_cmd(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        let lang = self.lang_for_sender(cmd.sender());
        let now = now_ms();
        let list = self.mutes.list(now);
        if list.is_empty() {
            cmd.reply(tr(lang, Msg::MuteListEmpty, &[]));
            return Ok(());
        }
        cmd.reply(tr(
            lang,
            Msg::MuteListHeader,
            &[("count", &list.len().to_string())],
        ));
        for (_, m) in list {
            let time = match m.remaining(now) {
                None => template_word(lang, Msg::Permanent),
                Some(ms) => mute::format_duration(ms),
            };
            cmd.reply(
                tr(
                    lang,
                    Msg::MuteListEntry,
                    &[("player", &m.name), ("time", &time), ("reason", &m.reason)],
                )
                .hover(Text::new(format!("by {}", m.by)))
                .suggest_command(format!("/unmute {}", m.name)),
            );
        }
        Ok(())
    }

    #[command(
        name = "chatformat",
        permission = "chatformat.admin",
        usage = "reload",
        description = "Reload the chat-format configuration"
    )]
    fn admin_cmd(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        let lang = self.lang_for_sender(cmd.sender());
        match rustic::config::Config::new()
            .defaults(DEFAULT_CONFIG)
            .load::<ChatConfig>()
        {
            Ok(cfg) => {
                self.cfg = cfg;
                cmd.reply(tr(lang, Msg::Reloaded, &[]));
                Ok(())
            }
            Err(e) => Err(tr(lang, Msg::ReloadFailed, &[("error", &e.to_string())]).into()),
        }
    }

    #[completer("mute")]
    fn complete_mute(&mut self, ctx: &CompleteCtx) -> Vec<String> {
        ctx.suggest(Player::online().iter().filter_map(Player::name))
    }

    #[completer("unmute")]
    fn complete_unmute(&mut self, ctx: &CompleteCtx) -> Vec<String> {
        ctx.suggest(self.mutes.list(now_ms()).into_iter().map(|(_, m)| m.name))
    }
}

/// A one-word message (`permanent`) as plain text.
fn template_word(lang: Lang, msg: Msg) -> String {
    lang::template(lang, msg).to_owned()
}
