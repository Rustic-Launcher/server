//! mini-game: arena games validating the scheduler and world APIs.
//!
//! Two modes per arena:
//! * **Spleef**: a snow floor (rebuilt with one `set_blocks` batch before and after each round,
//!   broken blocks tracked); players dig under each other; falling through eliminates; the last
//!   one standing wins (or the best digger when the time is up).
//! * **King of the Hill**: a capture zone checked every tick from player locations, particles
//!   on its border; standing alone in it scores a point per second; first to the target (or the
//!   best at the end) wins.
//!
//! Shared flow: `/mg join` puts players in the arena lobby with a boss bar; reaching the minimum
//! starts a countdown (boss bar progress, titles, sounds); the round teleports everyone to the
//! start, shows a live sidebar (scores and timer, updated every second by `run_timer`), ends
//! with titles, stats in storage (`/mg stats`, `/mg top`) and rewards through the economy
//! service when one is installed, then sends everyone back to the lobby. Disconnects and
//! disabling the plugin mid-game clean everything up (floor, bars, sidebar, items, positions).

mod arena;
mod config;
mod game;
mod lang;
mod stats;

use std::collections::{BTreeMap, HashMap};

use rustic::permissions::Permissions;
use rustic::prelude::*;
use serde::{Deserialize, Serialize};

use crate::arena::{Arena, Loc, Mode, Pos};
use crate::config::{Cfg, DEFAULT_CONFIG};
use crate::game::{Game, JoinError, Phase, Step};
use crate::lang::{Lang, Msg, tr, tr_mini};
use crate::stats::{Stat, Stats};

/// Permission nodes.
pub mod perms {
    /// Join, leave, list, stats, top.
    pub const PLAY: &str = "minigame.play";
    /// Create, edit, delete, start and stop arenas.
    pub const ADMIN: &str = "minigame.admin";
}

/// Marks the items the game hands out, so only those are taken back.
const ITEM_MARKER: &str = "rustic_minigame";
/// Per-player data: where to put a player who disconnected mid-game when they come back.
const RETURN_KEY: &str = "return";

/// Saved for a player who left during a game while offline.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PendingReturn {
    to: Loc,
    mode: Option<String>,
}

fn to_location(l: &Loc) -> Location {
    Location::new(l.world.clone(), l.x, l.y, l.z).facing(l.yaw, l.pitch)
}

fn from_location(l: &Location) -> Loc {
    Loc {
        world: l.world.clone(),
        x: l.x,
        y: l.y,
        z: l.z,
        yaw: l.yaw,
        pitch: l.pitch,
    }
}

fn mode_name(m: GameMode) -> &'static str {
    match m {
        GameMode::Survival => "survival",
        GameMode::Creative => "creative",
        GameMode::Adventure => "adventure",
        GameMode::Spectator => "spectator",
    }
}

fn parse_mode(s: &str) -> Option<GameMode> {
    match s {
        "survival" => Some(GameMode::Survival),
        "creative" => Some(GameMode::Creative),
        "adventure" => Some(GameMode::Adventure),
        "spectator" => Some(GameMode::Spectator),
        _ => None,
    }
}

#[rustic::plugin]
#[derive(Default)]
struct MiniGame {
    cfg: Cfg,
    arenas: BTreeMap<String, Arena>,
    games: BTreeMap<String, Game>,
    /// One boss bar per player in a game (each in the player's language).
    bars: HashMap<Uuid, BossBar>,
    /// Game modes before a round, restored afterwards.
    modes: HashMap<Uuid, GameMode>,
    /// The per-tick task, running while a round is played.
    ticker: Option<Task>,
    ticks: u64,
    /// KotH: who held each zone at the last tick (to tell players only on change).
    holders: HashMap<String, Option<Uuid>>,
}

/// Runs a command body with the plugin state.
fn exec(
    f: fn(&mut MiniGame, &mut CommandCtx) -> CommandResult,
) -> impl FnMut(&mut CommandCtx) -> CommandResult {
    move |cmd| with_plugin(|m: &mut MiniGame| f(m, cmd))
}

impl Plugin for MiniGame {
    fn load(&mut self, _ctx: &mut Ctx) -> Result<()> {
        Permissions::declare(perms::PLAY)
            .default(PermissionDefault::Allow)
            .description("Play the mini-games: /mg join, leave, list, stats, top")
            .register();
        Permissions::declare(perms::ADMIN)
            .default(PermissionDefault::Op)
            .description("Create, edit, start and stop mini-game arenas")
            .register();
        Ok(())
    }

    fn enable(&mut self, ctx: &mut Ctx) -> Result<()> {
        if !ctx.is_main() {
            return Ok(());
        }
        self.cfg = rustic::config::Config::new()
            .defaults(DEFAULT_CONFIG)
            .load()?;
        for key in Storage::keys(arena::KEY_PREFIX) {
            match Storage::get::<Arena>(&key) {
                Ok(Some(a)) => {
                    self.arenas.insert(a.name.clone(), a);
                }
                Ok(None) => {}
                Err(e) => warn!("ignoring unreadable arena {key}: {e}"),
            }
        }
        ctx.command(
            Command::new("mg")
                .alias("minigame")
                .description("Mini-game arenas: join, leave, stats, top")
                .permission(perms::PLAY)
                .route("", exec(MiniGame::cmd_help))
                .route("help", exec(MiniGame::cmd_help))
                .route("join [arena:word*]", exec(MiniGame::cmd_join))
                .route("leave", exec(MiniGame::cmd_leave))
                .route("list", exec(MiniGame::cmd_list))
                .route("stats [player:word*]", exec(MiniGame::cmd_stats))
                .route(
                    "top [stat:word{wins|games|points}]",
                    exec(MiniGame::cmd_top),
                )
                .route(
                    "create <arena:word> [mode:word{spleef|koth}]",
                    exec(MiniGame::cmd_create),
                )
                .usage_permission(perms::ADMIN)
                .route("setspawn <arena:word*>", exec(MiniGame::cmd_setspawn))
                .usage_permission(perms::ADMIN)
                .route("setlobby <arena:word*>", exec(MiniGame::cmd_setlobby))
                .usage_permission(perms::ADMIN)
                .route(
                    "setarea <arena:word*> [radius:integer(0..30)]",
                    exec(MiniGame::cmd_setarea),
                )
                .usage_permission(perms::ADMIN)
                .route("delete <arena:word*>", exec(MiniGame::cmd_delete))
                .usage_permission(perms::ADMIN)
                .route("start <arena:word*>", exec(MiniGame::cmd_start))
                .usage_permission(perms::ADMIN)
                .route("stop <arena:word*>", exec(MiniGame::cmd_stop))
                .usage_permission(perms::ADMIN)
                .completer(|c| {
                    with_plugin(|m: &mut MiniGame| match c.argument() {
                        "arena" => c.suggest(m.arenas.keys().cloned()),
                        _ => c.suggest(Player::online().iter().filter_map(Player::name)),
                    })
                }),
        )?;
        // Countdowns, round timers, sidebars and boss bars: once per second.
        Scheduler::run_timer(20, 20, |_| with_plugin(MiniGame::second));
        info!(
            "mini-game enabled: {} arenas, economy rewards: {}",
            self.arenas.len(),
            Economy::is_available()
        );
        Ok(())
    }

    fn disable(&mut self, _ctx: &mut Ctx) {
        let names: Vec<String> = self.games.keys().cloned().collect();
        for name in names {
            let players = self
                .games
                .get(&name)
                .map(|g| g.players.clone())
                .unwrap_or_default();
            for p in players {
                let player = Player::from_uuid(p);
                if player.is_online() {
                    player.send_message(tr(self.lang(player), Msg::ShutDown, &[]));
                }
            }
            self.reset_arena(&name);
        }
    }
}

// ---- helpers ------------------------------------------------------------------------------

impl MiniGame {
    fn lang(&self, player: Player) -> Lang {
        Lang::from_code(&self.cfg.language).unwrap_or_else(|| {
            player
                .locale()
                .ok()
                .and_then(|l| Lang::from_code(&l))
                .unwrap_or(Lang::En)
        })
    }

    fn sender_lang(&self, cmd: &CommandCtx) -> Lang {
        match cmd.sender().as_player() {
            Some(p) => self.lang(p),
            None => Lang::from_code(&self.cfg.language).unwrap_or(Lang::En),
        }
    }

    /// The language of shared displays (the sidebar).
    fn shared_lang(&self) -> Lang {
        Lang::from_code(&self.cfg.language).unwrap_or(Lang::En)
    }

    fn mode_label(lang: Lang, mode: Mode) -> String {
        tr_mini(
            lang,
            match mode {
                Mode::Spleef => Msg::ModeSpleef,
                Mode::Koth => Msg::ModeKoth,
            },
            &[],
        )
    }

    fn game_of(&self, player: Uuid) -> Option<String> {
        self.games
            .iter()
            .find(|(_, g)| g.players.contains(&player))
            .map(|(n, _)| n.clone())
    }

    fn arena_arg(&self, cmd: &CommandCtx) -> Result<Arena, CommandError> {
        let name: String = cmd.arg("arena")?;
        let name = name.to_ascii_lowercase();
        self.arenas
            .get(&name)
            .cloned()
            .ok_or_else(|| tr(self.sender_lang(cmd), Msg::NoArena, &[("arena", &name)]).into())
    }

    fn save_arena(&mut self, arena: Arena) -> Result<()> {
        Storage::set(&arena::key(&arena.name), &arena)?;
        self.arenas.insert(arena.name.clone(), arena);
        Ok(())
    }

    /// Tells every player of a game something (in their language).
    fn tell_game(&self, name: &str, msg: Msg, vars: &[(&str, &str)]) {
        if let Some(g) = self.games.get(name) {
            for p in &g.players {
                let player = Player::from_uuid(*p);
                player.send_message(tr(self.lang(player), msg, vars));
            }
        }
    }

    fn player_name(p: Uuid) -> String {
        Player::from_uuid(p).name().unwrap_or_else(|| p.to_string())
    }

    /// Gives the spleef tool, marked so that only it is taken back.
    fn give_tool(&self, player: Player) {
        let tool = ItemStack::new(self.cfg.spleef.tool.clone())
            .name(tr(self.lang(player), Msg::ToolName, &[]))
            .enchant("efficiency", 5)
            .unbreakable()
            .component("minecraft:custom_data", format!("{{{ITEM_MARKER}:1b}}"));
        if let Err(e) = player.inventory().give(&tool) {
            warn!("could not give the spleef tool: {e}");
        }
    }

    /// Takes back every item the game gave.
    fn strip_items(player: Player) {
        let inv = player.inventory();
        let Ok(contents) = inv.contents() else { return };
        for (slot, item) in contents.iter().enumerate() {
            if item.components_snbt().contains(ITEM_MARKER) {
                let _ = inv.set(slot as u32, &ItemStack::empty());
            }
        }
    }

    /// Back to the lobby, previous game mode, game items taken, bar removed.
    fn send_back(&mut self, player: Player, lobby: &Loc) {
        if let Some(bar) = self.bars.remove(&player.uuid()) {
            bar.remove();
        }
        let mode = self.modes.remove(&player.uuid());
        if !player.is_online() {
            return;
        }
        Self::strip_items(player);
        if let Some(m) = mode {
            let _ = player.set_game_mode(m);
        }
        if let Err(e) = player.teleport(&to_location(lobby)) {
            warn!("could not send a player back to the lobby: {e}");
        }
    }

    fn build_floor(&self, arena: &Arena) {
        let world = World::new(arena.spawn.world.clone());
        let mut batch = world.batch();
        let block = BlockState::from(self.cfg.spleef.floor_block.as_str());
        for p in arena.floor() {
            batch.push(BlockPos::new(p.x, p.y, p.z), block.clone());
        }
        match batch.apply() {
            Ok(n) => debug!("spleef floor of {}: {n} blocks changed", arena.name),
            Err(e) => warn!("could not build the spleef floor of {}: {e}", arena.name),
        }
    }

    fn objective_name(arena: &str) -> String {
        format!("g_{arena}")
    }

    // ---- boss bars, sidebar ---------------------------------------------------------------

    fn update_bars(&mut self, name: &str) {
        let (Some(g), Some(a)) = (self.games.get(name), self.arenas.get(name)) else {
            return;
        };
        let rules = self.cfg.rules();
        let players = g.players.clone();
        let phase = g.phase;
        let winner = g.winner;
        let count = players.len().to_string();
        let mode = a.mode;
        for p in players {
            let player = Player::from_uuid(p);
            let lang = self.lang(player);
            let (title, progress, color) = match phase {
                Phase::Waiting => (
                    tr(
                        lang,
                        Msg::BarWaiting,
                        &[("count", &count), ("min", &rules.min_players.to_string())],
                    ),
                    players_ratio(&count, rules.min_players),
                    BarColor::Yellow,
                ),
                Phase::Countdown(n) => (
                    tr(lang, Msg::BarCountdown, &[("seconds", &n.to_string())]),
                    n as f32 / rules.countdown as f32,
                    BarColor::Green,
                ),
                Phase::Running(n) => (
                    tr(
                        lang,
                        Msg::BarRunning,
                        &[
                            ("mode", &Text::mini(&Self::mode_label(lang, mode)).plain()),
                            ("time", &arena::clock(n)),
                        ],
                    ),
                    n as f32 / rules.round as f32,
                    BarColor::Blue,
                ),
                Phase::Ending(_) => (
                    match winner {
                        Some(w) => tr(lang, Msg::BarEnding, &[("winner", &Self::player_name(w))]),
                        None => tr(lang, Msg::BarDraw, &[]),
                    },
                    1.0,
                    BarColor::Purple,
                ),
            };
            match self.bars.get(&p) {
                Some(bar) => {
                    bar.set_title(title);
                    bar.set_progress(progress);
                    bar.set_color(color);
                }
                None => {
                    let bar = BossBar::create(title, color, BarStyle::Progress, progress);
                    let _ = bar.show(&player);
                    self.bars.insert(p, bar);
                }
            }
        }
    }

    fn update_sidebar(&self, name: &str) {
        let Some(g) = self.games.get(name) else {
            return;
        };
        let objective = Scoreboard::objective(&Self::objective_name(name));
        let time_line = tr_mini(self.shared_lang(), Msg::SidebarTime, &[]);
        if let Phase::Running(n) = g.phase {
            let _ = objective.set_score(&time_line, n as i32);
        }
        for (p, score) in &g.scores {
            if g.players.contains(p) {
                let _ = objective.set_score(&Self::player_name(*p), *score as i32);
            }
        }
    }

    // ---- the round ------------------------------------------------------------------------

    /// Once per second, for every game.
    fn second(&mut self) {
        let rules = self.cfg.rules();
        let names: Vec<String> = self.games.keys().cloned().collect();
        for name in names {
            let Some(step) = self.games.get_mut(&name).map(|g| g.second(&rules)) else {
                continue;
            };
            match step {
                Step::Idle | Step::Ending(_) => {}
                Step::Countdown(n) => {
                    if n <= 5 {
                        let players = self.games[&name].players.clone();
                        for p in players {
                            let player = Player::from_uuid(p);
                            let lang = self.lang(player);
                            let _ = player.title_with(
                                Text::mini(&format!("<yellow>{n}")),
                                tr(lang, Msg::CountdownSub, &[]),
                                0,
                                25,
                                5,
                            );
                            player.sound("minecraft:block.note_block.hat", 1.0, 1.0);
                        }
                    }
                }
                Step::Aborted => self.tell_game(&name, Msg::Aborted, &[]),
                Step::Start => self.start_round(&name),
                Step::Round(_) => self.update_sidebar(&name),
                Step::TimeUp => {
                    let winner = self.games.get(&name).and_then(|g| {
                        match self.arenas.get(&name).map(|a| a.mode) {
                            Some(Mode::Spleef) => g.leader_among(&g.alive),
                            _ => g.leader_among(&g.players),
                        }
                    });
                    self.end_round(&name, winner);
                }
                Step::Reset => {
                    self.reset_arena(&name);
                    continue;
                }
            }
            self.update_bars(&name);
        }
    }

    fn start_round(&mut self, name: &str) {
        let Some(arena) = self.arenas.get(name).cloned() else {
            return;
        };
        let players = self.games[name].players.clone();
        if arena.mode == Mode::Spleef {
            self.build_floor(&arena);
        }
        let n = players.len();
        for (i, p) in players.iter().enumerate() {
            let player = Player::from_uuid(*p);
            let lang = self.lang(player);
            if let Ok(m) = player.game_mode() {
                self.modes.insert(*p, m);
            }
            let _ = player.set_game_mode(if arena.mode == Mode::Spleef {
                GameMode::Survival
            } else {
                GameMode::Adventure
            });
            if let Err(e) = player.teleport(&to_location(&arena.start_position(i, n))) {
                warn!("could not teleport a player to the start of {name}: {e}");
            }
            if arena.mode == Mode::Spleef {
                self.give_tool(player);
            }
            let sub = if arena.mode == Mode::Spleef {
                Msg::GoSpleef
            } else {
                Msg::GoKoth
            };
            let _ = player.title_with(tr(lang, Msg::Go, &[]), tr(lang, sub, &[]), 0, 30, 10);
            player.sound("minecraft:entity.player.levelup", 1.0, 1.0);
        }
        // The sidebar: scores and the timer, updated every second.
        let lang = self.shared_lang();
        let title = tr(
            lang,
            Msg::SidebarTitle,
            &[
                (
                    "mode",
                    &Text::mini(&Self::mode_label(lang, arena.mode)).plain(),
                ),
                ("arena", name),
            ],
        );
        let objective_name = Self::objective_name(name);
        let _ = Scoreboard::objective(&objective_name).remove();
        match Scoreboard::create_objective(&objective_name, title, "dummy") {
            Ok(o) => {
                let _ = o.display_in(DisplaySlot::Sidebar);
            }
            Err(e) => warn!("could not create the sidebar of {name}: {e}"),
        }
        self.update_sidebar(name);
        self.holders.insert(name.to_owned(), None);
        if self.ticker.is_none() {
            self.ticker = Some(Scheduler::run_timer(1, 1, |_| with_plugin(MiniGame::tick)));
        }
    }

    /// Every tick while a round is played: falls (spleef), the zone (KotH).
    fn tick(&mut self) {
        self.ticks += 1;
        let running: Vec<String> = self
            .games
            .iter()
            .filter(|(_, g)| g.running())
            .map(|(n, _)| n.clone())
            .collect();
        if running.is_empty() {
            if let Some(t) = self.ticker.take() {
                t.cancel();
            }
            return;
        }
        for name in running {
            let Some(arena) = self.arenas.get(&name).cloned() else {
                continue;
            };
            match arena.mode {
                Mode::Spleef => self.spleef_tick(&name, &arena),
                Mode::Koth => self.koth_tick(&name, &arena),
            }
        }
    }

    fn spleef_tick(&mut self, name: &str, arena: &Arena) {
        let alive = self.games[name].alive.clone();
        for p in alive {
            let player = Player::from_uuid(p);
            let Ok(at) = player.location() else { continue };
            if at.world != arena.spawn.world || !arena.fell(at.y) {
                continue;
            }
            let Some(g) = self.games.get_mut(name) else {
                return;
            };
            g.eliminate(p);
            let left = g.alive.len().to_string();
            let who = Self::player_name(p);
            self.tell_game(name, Msg::Fell, &[("player", &who), ("alive", &left)]);
            let lang = self.lang(player);
            let _ = player.title_with(
                tr(lang, Msg::YouFell, &[]),
                tr(lang, Msg::YouFellSub, &[]),
                5,
                40,
                10,
            );
            player.sound("minecraft:entity.player.hurt", 1.0, 0.8);
            Self::strip_items(player);
            let _ = player.teleport(&to_location(&arena.lobby));
        }
        if let Some(result) = self.games.get(name).and_then(Game::last_standing) {
            self.end_round(name, result);
        }
    }

    fn koth_tick(&mut self, name: &str, arena: &Arena) {
        let rules = self.cfg.rules();
        let players = self.games[name].alive.clone();
        let in_zone: Vec<Uuid> = players
            .iter()
            .copied()
            .filter(|p| {
                Player::from_uuid(*p)
                    .location()
                    .is_ok_and(|l| l.world == arena.spawn.world && arena.in_zone(l.x, l.y, l.z))
            })
            .collect();
        let Some(capture) = self
            .games
            .get_mut(name)
            .map(|g| g.capture_tick(&in_zone, &rules))
        else {
            return;
        };
        if capture.scored {
            self.update_sidebar(name);
        }
        // Tell the players when the hill changes hands, and every second anyway.
        let changed = self.holders.get(name).copied().flatten() != capture.holder;
        self.holders.insert(name.to_owned(), capture.holder);
        if changed || capture.scored || self.ticks.is_multiple_of(20) {
            let target = rules.koth_points.to_string();
            for p in &players {
                let player = Player::from_uuid(*p);
                let lang = self.lang(player);
                let line = match capture.holder {
                    Some(h) => {
                        let points = self.games[name].scores.get(&h).copied().unwrap_or(0);
                        tr(
                            lang,
                            Msg::King,
                            &[
                                ("player", &Self::player_name(h)),
                                ("points", &points.to_string()),
                                ("target", &target),
                            ],
                        )
                    }
                    None if capture.contested => tr(lang, Msg::Contested, &[]),
                    None => tr(lang, Msg::HillFree, &[]),
                };
                player.action_bar(line);
            }
        }
        if self.ticks.is_multiple_of(10) {
            let color = match (capture.holder, capture.contested) {
                (Some(_), _) => "[1.0,0.67,0.0]",
                (None, true) => "[1.0,0.2,0.2]",
                _ => "[1.0,1.0,1.0]",
            };
            let world = World::new(arena.spawn.world.clone());
            let y = f64::from(arena.center.y) + 1.2;
            let n = (arena.radius.max(1) * 6).min(64) as usize;
            for (x, z) in arena.border(n) {
                let mut particle = world
                    .particle(&self.cfg.koth.particle, Vec3::new(x, y, z))
                    .count(1);
                if self.cfg.koth.particle.ends_with("dust") {
                    particle = particle.options(format!("{{color:{color},scale:1.0}}"));
                }
                let _ = particle.spawn();
            }
        }
        if let Some(w) = capture.won {
            self.end_round(name, Some(w));
        }
    }

    /// The round is over: titles, stats, rewards; everyone goes back after `end_seconds`.
    fn end_round(&mut self, name: &str, winner: Option<Uuid>) {
        let rules = self.cfg.rules();
        let Some(g) = self.games.get_mut(name) else {
            return;
        };
        if !g.running() {
            return;
        }
        g.finish(winner, &rules);
        let players = g.players.clone();
        let scores = g.scores.clone();
        let winner_name = winner.map(Self::player_name);
        for p in &players {
            let player = Player::from_uuid(*p);
            let lang = self.lang(player);
            let (title, sub) = match (&winner_name, winner == Some(*p)) {
                (Some(w), true) => (
                    tr(lang, Msg::Victory, &[]),
                    tr(lang, Msg::WinnerSub, &[("winner", w)]),
                ),
                (Some(w), false) => (
                    tr(lang, Msg::GameOver, &[]),
                    tr(lang, Msg::WinnerSub, &[("winner", w)]),
                ),
                (None, _) => (tr(lang, Msg::GameOver, &[]), tr(lang, Msg::DrawSub, &[])),
            };
            let _ = player.title_with(title, sub, 5, 60, 20);
            player.sound("minecraft:ui.toast.challenge_complete", 1.0, 1.0);
            // Stats.
            let key = stats::key(*p);
            let result = Storage::update(&key, |s: &mut Stats| {
                s.name = Self::player_name(*p);
                s.games += 1;
                s.points += u64::from(scores.get(p).copied().unwrap_or(0));
                if winner == Some(*p) {
                    s.wins += 1;
                }
            });
            if let Err(e) = result {
                warn!("could not save the stats of {}: {e}", Self::player_name(*p));
            }
            // Rewards, when an economy plugin is installed.
            let amount = self.cfg.rewards.play
                + if winner == Some(*p) {
                    self.cfg.rewards.win
                } else {
                    0
                };
            if amount > 0 {
                match Economy::deposit(player, amount) {
                    Ok(_) => {
                        let shown = Economy::format(amount).unwrap_or_else(|_| amount.to_string());
                        player.send_message(tr(lang, Msg::Reward, &[("amount", &shown)]));
                    }
                    Err(ServiceError::NoProvider) => {}
                    Err(e) => warn!("reward not paid: {}", Error::from(e)),
                }
            }
        }
        match &winner_name {
            Some(w) => self.tell_game(name, Msg::Won, &[("winner", w), ("arena", name)]),
            None => self.tell_game(name, Msg::Draw, &[("arena", name)]),
        }
        self.update_sidebar(name);
        self.update_bars(name);
    }

    /// Everyone back to the lobby, the floor restored, the sidebar removed, the game forgotten.
    fn reset_arena(&mut self, name: &str) {
        let Some(g) = self.games.remove(name) else {
            return;
        };
        let Some(arena) = self.arenas.get(name).cloned() else {
            return;
        };
        for p in &g.players {
            self.send_back(Player::from_uuid(*p), &arena.lobby);
        }
        let played = g.started_with > 0;
        if played {
            let _ = Scoreboard::objective(&Self::objective_name(name)).remove();
            if arena.mode == Mode::Spleef {
                self.build_floor(&arena);
            }
        }
        self.holders.remove(name);
    }

    /// A player leaves their game (command, disconnect).
    fn leave_game(&mut self, player: Player, online: bool) -> Option<String> {
        let name = self.game_of(player.uuid())?;
        let arena = self.arenas.get(&name).cloned()?;
        let rules = self.cfg.rules();
        let g = self.games.get_mut(&name)?;
        let was_playing = matches!(g.phase, Phase::Running(_) | Phase::Ending(_));
        g.leave(player.uuid());
        let count = g.players.len().to_string();
        let empty = g.players.is_empty();
        let max = rules.max_players.to_string();
        if online {
            self.send_back(player, &arena.lobby);
        } else {
            // Offline: put them back when they return.
            if let Some(bar) = self.bars.remove(&player.uuid()) {
                bar.remove();
            }
            let mode = self
                .modes
                .remove(&player.uuid())
                .map(|m| mode_name(m).to_owned());
            if was_playing {
                let pending = PendingReturn {
                    to: arena.lobby.clone(),
                    mode,
                };
                if let Err(e) = player.data().set(RETURN_KEY, &pending) {
                    warn!("could not remember where to put a player back: {e}");
                }
            }
        }
        let who = player.name().unwrap_or_default();
        self.tell_game(
            &name,
            Msg::LeftOther,
            &[("player", &who), ("count", &count), ("max", &max)],
        );
        if empty {
            self.reset_arena(&name);
        } else {
            if let Some(result) = self.games.get(&name).and_then(Game::last_standing) {
                self.end_round(&name, result);
            }
            self.update_bars(&name);
        }
        Some(name)
    }

    fn arena_state(&self, lang: Lang, name: &str) -> (String, usize) {
        let (msg, count) = match self.games.get(name) {
            None => (Msg::StateWaiting, 0),
            Some(g) => (
                match g.phase {
                    Phase::Waiting => Msg::StateWaiting,
                    Phase::Countdown(_) => Msg::StateCountdown,
                    Phase::Running(_) => Msg::StateRunning,
                    Phase::Ending(_) => Msg::StateEnding,
                },
                g.players.len(),
            ),
        };
        (tr_mini(lang, msg, &[]), count)
    }

    fn here(cmd: &CommandCtx) -> Result<(Player, Location), CommandError> {
        let p = cmd.player()?;
        let at = p.location()?;
        Ok((p, at))
    }
}

fn players_ratio(count: &str, min: usize) -> f32 {
    let c: f32 = count.parse().unwrap_or(0.0);
    (c / min.max(1) as f32).clamp(0.0, 1.0)
}

// ---- commands -----------------------------------------------------------------------------

impl MiniGame {
    fn cmd_help(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        let lang = self.sender_lang(cmd);
        cmd.reply(tr(lang, Msg::Help, &[]));
        if cmd.sender().has_permission(perms::ADMIN) {
            cmd.reply(tr(lang, Msg::HelpAdmin, &[]));
        }
        Ok(())
    }

    fn cmd_join(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        let player = cmd.player()?;
        let lang = self.lang(player);
        if let Some(current) = self.game_of(player.uuid()) {
            return Err(tr(lang, Msg::AlreadyIn, &[("arena", &current)]).into());
        }
        let rules = self.cfg.rules();
        let name = match cmd.opt_arg::<String>("arena")? {
            Some(n) => {
                let n = n.to_ascii_lowercase();
                if !self.arenas.contains_key(&n) {
                    return Err(tr(lang, Msg::NoArena, &[("arena", &n)]).into());
                }
                n
            }
            // The fullest arena still open.
            None => self
                .arenas
                .keys()
                .filter(|n| {
                    self.games.get(*n).is_none_or(|g| {
                        matches!(g.phase, Phase::Waiting | Phase::Countdown(_))
                            && g.players.len() < rules.max_players
                    })
                })
                .max_by_key(|n| self.games.get(*n).map_or(0, |g| g.players.len()))
                .cloned()
                .ok_or_else(|| tr(lang, Msg::NoOpenArena, &[]))?,
        };
        let arena = self.arenas[&name].clone();
        let game = self.games.entry(name.clone()).or_default();
        match game.join(player.uuid(), &rules) {
            Ok(()) => {}
            Err(JoinError::Full) => return Err(tr(lang, Msg::Full, &[("arena", &name)]).into()),
            Err(JoinError::InProgress) => {
                return Err(tr(lang, Msg::InProgress, &[("arena", &name)]).into());
            }
            Err(JoinError::Already) => {
                return Err(tr(lang, Msg::AlreadyIn, &[("arena", &name)]).into());
            }
        }
        let count = game.players.len().to_string();
        let max = rules.max_players.to_string();
        player.teleport(&to_location(&arena.lobby))?;
        cmd.reply(tr(
            lang,
            Msg::Joined,
            &[("arena", &name), ("count", &count), ("max", &max)],
        ));
        let who = player.name().unwrap_or_default();
        let others: Vec<Uuid> = self.games[&name]
            .players
            .iter()
            .copied()
            .filter(|p| *p != player.uuid())
            .collect();
        for p in others {
            let other = Player::from_uuid(p);
            other.send_message(tr(
                self.lang(other),
                Msg::JoinedOther,
                &[("player", &who), ("count", &count), ("max", &max)],
            ));
        }
        self.update_bars(&name);
        Ok(())
    }

    fn cmd_leave(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        let player = cmd.player()?;
        let lang = self.lang(player);
        match self.leave_game(player, true) {
            Some(name) => {
                cmd.reply(tr(lang, Msg::Left, &[("arena", &name)]));
                Ok(())
            }
            None => Err(tr(lang, Msg::NotInGame, &[]).into()),
        }
    }

    fn cmd_list(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        let lang = self.sender_lang(cmd);
        if self.arenas.is_empty() {
            cmd.reply(tr(lang, Msg::ListEmpty, &[]));
            return Ok(());
        }
        cmd.reply(tr(lang, Msg::ListHeader, &[]));
        let max = self.cfg.rules().max_players.to_string();
        for (name, a) in &self.arenas {
            let (state, count) = self.arena_state(lang, name);
            let mode = Text::mini(&Self::mode_label(lang, a.mode)).plain();
            cmd.reply(
                tr(
                    lang,
                    Msg::ListEntry,
                    &[
                        ("arena", name),
                        ("mode", &mode),
                        ("state", &state),
                        ("count", &count.to_string()),
                        ("max", &max),
                    ],
                )
                .hover(tr(lang, Msg::ClickToJoin, &[]))
                .run_command(format!("/mg join {name}")),
            );
        }
        Ok(())
    }

    fn cmd_stats(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        let lang = self.sender_lang(cmd);
        let (uuid, name) = match cmd.opt_arg::<String>("player")? {
            Some(n) => {
                let p = Player::by_name(&n)
                    .or_else(|| Server::offline_player(&n))
                    .ok_or_else(|| tr(lang, Msg::UnknownPlayer, &[("player", &n)]))?;
                (p.uuid(), p.name().unwrap_or(n))
            }
            None => {
                let p = cmd.player()?;
                (p.uuid(), p.name().unwrap_or_default())
            }
        };
        match Storage::get::<Stats>(&stats::key(uuid))? {
            Some(s) if s.games > 0 => {
                cmd.reply(tr(lang, Msg::StatsHeader, &[("player", &name)]));
                cmd.reply(tr(
                    lang,
                    Msg::StatsLine,
                    &[
                        ("wins", &s.wins.to_string()),
                        ("games", &s.games.to_string()),
                        ("rate", &s.win_rate().to_string()),
                        ("points", &s.points.to_string()),
                    ],
                ));
            }
            _ => cmd.reply(tr(lang, Msg::NoStats, &[("player", &name)])),
        }
        Ok(())
    }

    fn cmd_top(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        let lang = self.sender_lang(cmd);
        let typed: Option<String> = cmd.opt_arg("stat")?;
        let stat = Stat::parse(typed.as_deref()).unwrap_or(Stat::Wins);
        let all: Vec<Stats> = Storage::keys(stats::KEY_PREFIX)
            .iter()
            .filter_map(|k| Storage::get::<Stats>(k).ok().flatten())
            .collect();
        let best = stats::top(all, stat, 10);
        if best.is_empty() {
            cmd.reply(tr(lang, Msg::TopEmpty, &[]));
            return Ok(());
        }
        cmd.reply(tr(lang, Msg::TopHeader, &[("stat", stat.name())]));
        for (i, s) in best.iter().enumerate() {
            cmd.reply(
                tr(
                    lang,
                    Msg::TopLine,
                    &[
                        ("rank", &(i + 1).to_string()),
                        ("player", &s.name),
                        ("wins", &s.wins.to_string()),
                        ("games", &s.games.to_string()),
                        ("points", &s.points.to_string()),
                    ],
                )
                .hover(tr(lang, Msg::TopHover, &[("player", &s.name)]))
                .run_command(format!("/mg stats {}", s.name)),
            );
        }
        Ok(())
    }

    fn cmd_create(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        let lang = self.sender_lang(cmd);
        let (_, at) = Self::here(cmd)?;
        let name: String = cmd.arg("arena")?;
        let name = name.to_ascii_lowercase();
        if !arena::valid_name(&name) {
            return Err(tr(lang, Msg::BadName, &[]).into());
        }
        if self.arenas.contains_key(&name) {
            return Err(tr(lang, Msg::Exists, &[("arena", &name)]).into());
        }
        let mode = match cmd.opt_arg::<String>("mode")? {
            None => Mode::Spleef,
            Some(m) => Mode::parse(&m).ok_or_else(|| tr(lang, Msg::BadMode, &[]))?,
        };
        let feet = at.block();
        let here = from_location(&at);
        let radius = self.cfg.default_radius.clamp(0, 30);
        let arena = Arena {
            name: name.clone(),
            mode,
            spawn: here.clone(),
            lobby: here,
            center: Pos::new(feet.x, feet.y - 1, feet.z),
            radius,
        };
        if mode == Mode::Spleef {
            self.build_floor(&arena);
        }
        self.save_arena(arena)?;
        let mode_shown = Text::mini(&Self::mode_label(lang, mode)).plain();
        cmd.reply(tr(
            lang,
            Msg::Created,
            &[
                ("arena", &name),
                ("mode", &mode_shown),
                ("radius", &radius.to_string()),
            ],
        ));
        Ok(())
    }

    fn cmd_setspawn(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        let lang = self.sender_lang(cmd);
        let (_, at) = Self::here(cmd)?;
        let mut arena = self.arena_arg(cmd)?;
        arena.spawn = from_location(&at);
        let name = arena.name.clone();
        self.save_arena(arena)?;
        cmd.reply(tr(lang, Msg::SpawnSet, &[("arena", &name)]));
        Ok(())
    }

    fn cmd_setlobby(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        let lang = self.sender_lang(cmd);
        let (_, at) = Self::here(cmd)?;
        let mut arena = self.arena_arg(cmd)?;
        arena.lobby = from_location(&at);
        let name = arena.name.clone();
        self.save_arena(arena)?;
        cmd.reply(tr(lang, Msg::LobbySet, &[("arena", &name)]));
        Ok(())
    }

    fn cmd_setarea(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        let lang = self.sender_lang(cmd);
        let (_, at) = Self::here(cmd)?;
        let mut arena = self.arena_arg(cmd)?;
        if self
            .games
            .get(&arena.name)
            .is_some_and(|g| g.started_with > 0 || g.running())
        {
            return Err(tr(lang, Msg::Busy, &[("arena", &arena.name)]).into());
        }
        let radius = cmd
            .opt_arg::<i64>("radius")?
            .map_or(arena.radius, |r| r.clamp(0, 30) as i32);
        let feet = at.block();
        arena.center = Pos::new(feet.x, feet.y - 1, feet.z);
        arena.radius = radius;
        if arena.mode == Mode::Spleef {
            self.build_floor(&arena);
        }
        let name = arena.name.clone();
        self.save_arena(arena)?;
        cmd.reply(tr(
            lang,
            Msg::AreaSet,
            &[("arena", &name), ("radius", &radius.to_string())],
        ));
        Ok(())
    }

    fn cmd_delete(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        let lang = self.sender_lang(cmd);
        let arena = self.arena_arg(cmd)?;
        if self.games.contains_key(&arena.name) {
            return Err(tr(lang, Msg::Busy, &[("arena", &arena.name)]).into());
        }
        Storage::delete(&arena::key(&arena.name));
        self.arenas.remove(&arena.name);
        cmd.reply(tr(lang, Msg::Deleted, &[("arena", &arena.name)]));
        Ok(())
    }

    fn cmd_start(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        let lang = self.sender_lang(cmd);
        let arena = self.arena_arg(cmd)?;
        let rules = self.cfg.rules();
        let started = self
            .games
            .get_mut(&arena.name)
            .is_some_and(|g| g.force_start(&rules));
        if !started {
            return Err(tr(lang, Msg::CannotStart, &[("arena", &arena.name)]).into());
        }
        cmd.reply(tr(lang, Msg::Starting, &[("arena", &arena.name)]));
        self.update_bars(&arena.name);
        Ok(())
    }

    fn cmd_stop(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        let lang = self.sender_lang(cmd);
        let arena = self.arena_arg(cmd)?;
        if !self.games.contains_key(&arena.name) {
            return Err(tr(lang, Msg::NothingToStop, &[("arena", &arena.name)]).into());
        }
        self.tell_game(&arena.name, Msg::Stopped, &[("arena", &arena.name)]);
        cmd.reply(tr(lang, Msg::Stopped, &[("arena", &arena.name)]));
        self.reset_arena(&arena.name);
        Ok(())
    }
}

// ---- events -------------------------------------------------------------------------------

#[rustic::handlers]
impl MiniGame {
    /// Spleef digging: only the floor block, only by standing players, only while playing.
    /// Arenas with a game are protected from everyone else.
    #[event_handler(priority = "high")]
    fn on_break(&mut self, event: &mut BlockBreakEvent) {
        let p = event.player().uuid();
        let pos = event.pos();
        let pos = Pos::new(pos.x, pos.y, pos.z);
        let floor_block = BlockState::from(self.cfg.spleef.floor_block.as_str()).id;
        if let Some(name) = self.game_of(p) {
            let is_floor = self.arenas.get(&name).is_some_and(|a| {
                a.mode == Mode::Spleef
                    && a.spawn.world == event.world().id()
                    && a.in_floor(pos)
                    && event.block().id == floor_block
            });
            let counted = is_floor
                && self
                    .games
                    .get_mut(&name)
                    .is_some_and(|g| g.block_broken(p, pos));
            if counted {
                event.set_drop_items(false);
                self.update_sidebar(&name);
            } else {
                event.cancel();
            }
            return;
        }
        let protected = self.games.keys().any(|n| {
            self.arenas.get(n).is_some_and(|a| {
                a.mode == Mode::Spleef && a.spawn.world == event.world().id() && a.in_floor(pos)
            })
        });
        if protected {
            event.cancel();
        }
    }

    #[event_handler(priority = "high")]
    fn on_place(&mut self, event: &mut BlockPlaceEvent) {
        if self.game_of(event.player().uuid()).is_some() {
            event.cancel();
        }
    }

    /// No damage inside a game (falls, other players).
    #[event_handler(priority = "high")]
    fn on_damage(&mut self, event: &mut EntityDamageEvent) {
        if let Some(p) = event.target().as_player()
            && self.game_of(p.uuid()).is_some()
        {
            event.cancel();
        }
    }

    #[event_handler(priority = "monitor")]
    fn on_quit(&mut self, event: &mut PlayerQuitEvent) {
        self.leave_game(event.player(), false);
    }

    /// A player who disconnected mid-game comes back to the lobby, without the game's items.
    #[event_handler]
    fn on_join(&mut self, event: &mut PlayerJoinEvent) -> Result<()> {
        let player = event.player();
        let Some(pending) = player.data().get::<PendingReturn>(RETURN_KEY)? else {
            return Ok(());
        };
        player.data().delete(RETURN_KEY);
        Self::strip_items(player);
        if let Some(m) = pending.mode.as_deref().and_then(parse_mode) {
            let _ = player.set_game_mode(m);
        }
        player.teleport(&to_location(&pending.to))?;
        Ok(())
    }
}
