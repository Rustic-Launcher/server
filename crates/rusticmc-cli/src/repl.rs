//! The headless REPL of `rustic run`: a MockServer with the plugin loaded, driven by short
//! commands (`join Steve`, `chat Steve hello`, `cmd Steve /hello`, `tick 20`...). After each
//! command it prints what players received and what the console logged.

use crate::ui;
use rusticmc_test::model::{ClickKind, GameMode};
use rusticmc_test::{MockPlayer, MockServer, Received};
use std::collections::BTreeMap;
use std::io::Write;

const HELP: &str = "\
Players
  join <name> [op]              connect a player (op: as an operator)
  leave <name>                  disconnect
  chat <name> <message>         send a chat message
  cmd <name> <command...>       run a command as the player (/hello Alex)
  complete <name> <line...>     tab-complete (complete Steve hello A)
  op <name> | deop <name>       operator status
  perm <name> <node> <true|false|unset>
  gm <name> <survival|creative|adventure|spectator>
  tp <name> <x> <y> <z>         teleport (fires player-teleport)
  move <name> <dx> <dy> <dz>    walk (fires player-move)
  break <name> <x> <y> <z>      break a block
  place <name> <x> <y> <z> <block>
  give <name> <item> [count]    put items in the inventory
  damage <name> <amount> | kill <name> | respawn <name>
  menu <name>                   show the open menu
  click <name> <slot> [left|right|shift]
  close <name>                  close the open menu
  status <name>                 location, health, game mode, inventory
Server
  /<command...>                 run a command on the console (/rustic plugins)
  tick [n]                      advance n ticks (default 1, 20 ticks = 1 s)
  players | plugins | console   lists
  reload                        rebuild is automatic with --watch; this reloads the plugin
  help | exit";

/// REPL state.
pub struct Repl {
    server: MockServer,
    plugin: String,
    seen: BTreeMap<String, usize>,
    console_seen: usize,
    log_seen: usize,
}

impl Repl {
    /// A REPL over a server (plugins already added).
    pub fn new(server: MockServer, plugin: String) -> Self {
        let mut r = Repl {
            server,
            plugin,
            seen: BTreeMap::new(),
            console_seen: 0,
            log_seen: 0,
        };
        r.server.with_backend(|rt, b| {
            for (id, res) in rt.enable_all(b) {
                if let Err(e) = res {
                    ui::error(format!("{id}: {e}"));
                }
            }
        });
        r.flush();
        r
    }

    /// The welcome text.
    pub fn banner(&self) {
        let state = self
            .server
            .plugin_state(&self.plugin)
            .map(|s| s.to_string())
            .unwrap_or_else(|| "not loaded".into());
        anstream::println!(
            "{}Rustic REPL{} — `{}` is {state}. In-memory server (rusticmc-test), fake clock. Type `help`.",
            ui::BOLD,
            ui::BOLD.render_reset(),
            self.plugin
        );
        if let Some(info) = self.server.plugin(&self.plugin)
            && let Some(f) = info.failure
        {
            ui::error(f);
        }
    }

    /// Print the prompt.
    pub fn prompt(&self) {
        anstream::print!("{}>{} ", ui::CYAN, ui::CYAN.render_reset());
        let _ = std::io::stdout().flush();
    }

    fn player(&self, name: Option<&str>) -> Result<MockPlayer, String> {
        let name = name.ok_or("which player?")?;
        self.server
            .player(name)
            .ok_or_else(|| format!("{name} is not online (join {name})"))
    }

    /// Reload the plugin from its dev directory.
    pub fn reload(&mut self) {
        match self.server.reload(&self.plugin) {
            Ok(()) => ui::status("Reloaded", &self.plugin),
            Err(e) => ui::error(e),
        }
        self.flush();
    }

    /// Run one line: false to stop.
    pub fn execute(&mut self, line: &str) -> bool {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            return true;
        }
        if line == "exit" || line == "quit" {
            return false;
        }
        anstream::println!("{}» {line}{}", ui::DIM, ui::DIM.render_reset());
        if let Err(e) = self.dispatch(line) {
            ui::error(e);
        }
        self.flush();
        true
    }

    fn dispatch(&mut self, line: &str) -> Result<(), String> {
        if let Some(cmd) = line.strip_prefix('/') {
            let r = self.server.console_command(cmd);
            anstream::println!("  {}", describe(&r));
            return Ok(());
        }
        let words: Vec<&str> = line.split_whitespace().collect();
        let rest = |n: usize| -> String {
            line.split_whitespace()
                .skip(n)
                .collect::<Vec<_>>()
                .join(" ")
        };
        let num = |i: usize| -> Result<f64, String> {
            words
                .get(i)
                .ok_or("missing number")?
                .parse::<f64>()
                .map_err(|e| e.to_string())
        };
        match words[0] {
            "help" => anstream::println!("{HELP}"),
            "join" => {
                let name = words.get(1).ok_or("join <name>")?;
                let p = if words.get(2) == Some(&"op") {
                    self.server.add_op(name)
                } else {
                    self.server
                        .try_add_player(name)
                        .map_err(|r| format!("refused: {r}"))?
                };
                self.seen.insert(p.name().to_owned(), 0);
            }
            "leave" => self.player(words.get(1).copied())?.disconnect(),
            "chat" => {
                let p = self.player(words.get(1).copied())?;
                if !p.chat(&rest(2)) {
                    anstream::println!("  (cancelled by a plugin)");
                }
            }
            "cmd" | "run" => {
                let p = self.player(words.get(1).copied())?;
                let r = p.command(&rest(2));
                anstream::println!("  {}", describe(&r));
            }
            "complete" => {
                let p = self.player(words.get(1).copied())?;
                let mut line = rest(2);
                if line.ends_with(' ') || words.len() == 2 {
                    line.push(' ');
                }
                anstream::println!("  {:?}", p.complete(&line));
            }
            "op" | "deop" => self.player(words.get(1).copied())?.set_op(words[0] == "op"),
            "perm" => {
                let p = self.player(words.get(1).copied())?;
                let node = words
                    .get(2)
                    .ok_or("perm <name> <node> <true|false|unset>")?;
                match words.get(3).copied() {
                    Some("true") => p.set_permission(node, true),
                    Some("false") => p.set_permission(node, false),
                    Some("unset") => p.unset_permission(node),
                    _ => return Err("perm <name> <node> <true|false|unset>".into()),
                }
            }
            "gm" => {
                let p = self.player(words.get(1).copied())?;
                let mode = match words.get(2).copied() {
                    Some("survival") => GameMode::Survival,
                    Some("creative") => GameMode::Creative,
                    Some("adventure") => GameMode::Adventure,
                    Some("spectator") => GameMode::Spectator,
                    _ => return Err("gm <name> <survival|creative|adventure|spectator>".into()),
                };
                if !p.set_game_mode(mode) {
                    anstream::println!("  (unchanged or cancelled)");
                }
            }
            "tp" | "move" => {
                let p = self.player(words.get(1).copied())?;
                let (x, y, z) = (num(2)?, num(3)?, num(4)?);
                let ok = if words[0] == "tp" {
                    let mut to = p.location();
                    (to.x, to.y, to.z) = (x, y, z);
                    p.teleport(to)
                } else {
                    p.move_by(x, y, z)
                };
                if !ok {
                    anstream::println!("  (cancelled by a plugin)");
                }
            }
            "break" | "place" => {
                let p = self.player(words.get(1).copied())?;
                let pos = (num(2)? as i32, num(3)? as i32, num(4)? as i32);
                let ok = if words[0] == "break" {
                    p.break_block(pos)
                } else {
                    p.place_block(pos, words.get(5).ok_or("place <name> x y z <block>")?)
                };
                anstream::println!("  {}", if ok { "done" } else { "refused or cancelled" });
            }
            "give" => {
                let p = self.player(words.get(1).copied())?;
                let item = words.get(2).ok_or("give <name> <item> [count]")?;
                let n = words.get(3).and_then(|n| n.parse().ok()).unwrap_or(1);
                p.give(item, n);
            }
            "damage" => {
                let p = self.player(words.get(1).copied())?;
                anstream::println!("  dealt {:?}", p.damage(num(2)? as f32));
            }
            "kill" => self.player(words.get(1).copied())?.kill(),
            "respawn" => {
                let p = self.player(words.get(1).copied())?;
                anstream::println!(
                    "  respawned at {:?}",
                    p.respawn().map(|l| (l.world, l.x, l.y, l.z))
                );
            }
            "menu" => {
                let p = self.player(words.get(1).copied())?;
                match p.open_menu() {
                    None => anstream::println!("  no menu open"),
                    Some(m) => {
                        anstream::println!("  \"{}\" ({} rows)", m.title_plain(), m.rows);
                        for (slot, item) in &m.items {
                            anstream::println!("    [{slot:>2}] {} x{}", item.id, item.count);
                        }
                    }
                }
            }
            "click" => {
                let p = self.player(words.get(1).copied())?;
                let slot: u32 = num(2)? as u32;
                let kind = match words.get(3).copied() {
                    None | Some("left") => ClickKind::Left,
                    Some("right") => ClickKind::Right,
                    Some("shift") => ClickKind::ShiftLeft,
                    Some(k) => return Err(format!("unknown click {k}")),
                };
                let ok = p.click_menu_slot(slot, kind);
                anstream::println!(
                    "  {}",
                    if ok {
                        "click went through"
                    } else {
                        "click cancelled"
                    }
                );
            }
            "close" => {
                self.player(words.get(1).copied())?.close_menu();
            }
            "status" => {
                let p = self.player(words.get(1).copied())?;
                let l = p.location();
                anstream::println!(
                    "  {} at {} {:.1} {:.1} {:.1}, {:?}, health {}/{}, food {}, level {}{}",
                    p.name(),
                    l.world,
                    l.x,
                    l.y,
                    l.z,
                    p.game_mode(),
                    p.health(),
                    p.max_health(),
                    p.food(),
                    p.level(),
                    if p.is_op() { ", op" } else { "" }
                );
                for (slot, item) in p
                    .inventory()
                    .iter()
                    .enumerate()
                    .filter(|(_, i)| !i.is_empty())
                {
                    anstream::println!("    [{slot:>2}] {} x{}", item.id, item.count);
                }
            }
            "tick" => {
                let n = words.get(1).and_then(|n| n.parse().ok()).unwrap_or(1);
                self.server.tick(n);
                anstream::println!("  tick {}", self.server.current_tick());
            }
            "players" => {
                let names: Vec<String> = self
                    .server
                    .online_players()
                    .iter()
                    .map(|p| p.name().to_owned())
                    .collect();
                anstream::println!(
                    "  {}",
                    if names.is_empty() {
                        "(nobody)".into()
                    } else {
                        names.join(", ")
                    }
                );
            }
            "plugins" => {
                for p in self.server.runtime().plugins() {
                    anstream::println!(
                        "  {} {} {}{}",
                        p.id,
                        p.version,
                        p.state,
                        p.failure.map(|f| format!(" ({f})")).unwrap_or_default()
                    );
                }
            }
            "console" => {
                for l in self.server.console() {
                    anstream::println!("  {l}");
                }
            }
            "reload" => self.reload(),
            other => return Err(format!("unknown command `{other}` (type `help`)")),
        }
        Ok(())
    }

    /// Print what is new: console lines, then what each player received.
    fn flush(&mut self) {
        use ui::{CYAN, DIM, YELLOW};
        let logs = self.server.log_lines();
        for l in logs.iter().skip(self.log_seen) {
            anstream::println!("  {DIM}[log]{DIM:#} {l}");
        }
        self.log_seen = logs.len();
        let console = self.server.console_messages();
        for l in console.iter().skip(self.console_seen) {
            anstream::println!("  {DIM}[console]{DIM:#} {l}");
        }
        self.console_seen = console.len();
        for p in self.server.online_players() {
            let received = p.received();
            let seen = self.seen.entry(p.name().to_owned()).or_insert(0);
            for r in received.iter().skip(*seen) {
                let (kind, text) = match r {
                    Received::Message(t) => ("chat", t.to_plain()),
                    Received::ActionBar(t) => ("action bar", t.to_plain()),
                    Received::Title(t) => (
                        "title",
                        format!("{} / {}", t.title.to_plain(), t.subtitle.to_plain()),
                    ),
                    Received::Sound(s) => ("sound", s.sound.clone()),
                };
                anstream::println!(
                    "  {CYAN}{}{CYAN:#} {YELLOW}{kind}{YELLOW:#} {text}",
                    p.name()
                );
            }
            *seen = received.len();
            if let Some(reason) = p.kick_reason() {
                anstream::println!("  {} was kicked: {reason}", p.name());
            }
        }
    }
}

fn describe(r: &rusticmc_test::CommandRun) -> String {
    use rusticmc_test::{CommandOutcome, CommandRun};
    match r {
        CommandRun::Cancelled => "cancelled by a plugin".into(),
        CommandRun::Plugin(CommandOutcome::Executed) | CommandRun::Vanilla(true) => "ok".into(),
        CommandRun::Plugin(o) => format!("{o:?}"),
        CommandRun::Vanilla(false) => "refused".into(),
        CommandRun::Unknown => "unknown command".into(),
    }
}
