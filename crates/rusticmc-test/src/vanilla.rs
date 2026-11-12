//! A few vanilla commands, so plugins that call `server.dispatch-command("give ...")` or tests
//! that type `/gamemode creative` see the usual effect. Players need to be operators; the
//! console may run everything.
//!
//! Supported: `say`, `msg`/`tell`/`w`, `give`, `clear`, `gamemode`, `tp`/`teleport`,
//! `time set|add`, `weather`, `kill`, `kick`, `op`, `deop`, `list`, `setworldspawn`.

use crate::harness::Harness;
use crate::state::{OVERWORLD, namespaced};
use rusticmc_core::model::{CommandSender, GameMode, ItemStack, Location, Text, Uuid, Weather};

/// Run `line` (no slash) if it is a known vanilla command: `Some(success)`, `None` if unknown.
pub(crate) fn run(h: &Harness, sender: CommandSender, line: &str) -> Option<bool> {
    let mut words = line.split_whitespace();
    let label = words.next()?.to_ascii_lowercase();
    let label = label
        .strip_prefix("minecraft:")
        .unwrap_or(&label)
        .to_owned();
    let args: Vec<&str> = words.collect();
    let known = [
        "say",
        "msg",
        "tell",
        "w",
        "give",
        "clear",
        "gamemode",
        "tp",
        "teleport",
        "time",
        "weather",
        "kill",
        "kick",
        "op",
        "deop",
        "list",
        "setworldspawn",
    ];
    if !known.contains(&label.as_str()) {
        return None;
    }
    let is_op = match sender {
        CommandSender::Console => true,
        CommandSender::Player(u) => h.state.borrow().ops.contains(&u),
    };
    let public = matches!(label.as_str(), "msg" | "tell" | "w" | "list");
    if !is_op && !public {
        h.tell_sender(
            sender,
            Text::error("Unknown or incomplete command: you do not have permission"),
        );
        return Some(false);
    }
    let fail = |msg: &str| {
        h.tell_sender(sender, Text::error(msg));
        Some(false)
    };
    let sender_name = match sender {
        CommandSender::Console => "Server".to_owned(),
        CommandSender::Player(u) => h
            .state
            .borrow()
            .players
            .get(&u)
            .map(|p| p.name.clone())
            .unwrap_or_default(),
    };
    // Resolve a player argument (name or @s for a player sender).
    let player = |arg: Option<&&str>| -> Option<Uuid> {
        match arg.copied() {
            None | Some("@s") => sender.player(),
            Some(name) => h.state.borrow().by_name(name),
        }
    };
    match label.as_str() {
        "say" => {
            if args.is_empty() {
                return fail("Usage: /say <message>");
            }
            h.st()
                .broadcast(&Text::plain(format!("[{sender_name}] {}", args.join(" "))));
            Some(true)
        }
        "msg" | "tell" | "w" => {
            let Some(target) = args.first().and_then(|n| h.state.borrow().by_name(n)) else {
                return fail("No player was found");
            };
            let text = args[1..].join(" ");
            h.tell(
                target,
                Text::colored(format!("{sender_name} whispers to you: {text}"), "gray"),
            );
            Some(true)
        }
        "give" => {
            let Some(target) = player(args.first()) else {
                return fail("No player was found");
            };
            let Some(item) = args.get(1) else {
                return fail("Usage: /give <player> <item> [count]");
            };
            let count = args.get(2).and_then(|c| c.parse().ok()).unwrap_or(1);
            let left = h
                .st()
                .online_player_mut(target)
                .map(|p| p.inventory.give(&ItemStack::new(namespaced(item), count)));
            Some(left.is_some())
        }
        "clear" => {
            let Some(target) = player(args.first()) else {
                return fail("No player was found");
            };
            if let Some(p) = h.st().online_player_mut(target) {
                p.inventory.clear();
            }
            Some(true)
        }
        "gamemode" => {
            let mode = match args.first().map(|m| m.to_ascii_lowercase()).as_deref() {
                Some("survival" | "s" | "0") => GameMode::Survival,
                Some("creative" | "c" | "1") => GameMode::Creative,
                Some("adventure" | "a" | "2") => GameMode::Adventure,
                Some("spectator" | "sp" | "3") => GameMode::Spectator,
                _ => return fail("Usage: /gamemode <mode> [player]"),
            };
            let Some(target) = player(args.get(1)) else {
                return fail("No player was found");
            };
            h.set_game_mode(target, mode);
            Some(true)
        }
        "tp" | "teleport" => {
            // tp <x y z> | tp <player> <x y z> | tp <player> <other>
            let (target, rest) = match args.len() {
                3 => (sender.player(), &args[..]),
                1 => (sender.player(), &args[..]),
                _ => (player(args.first()), &args[1..]),
            };
            let Some(target) = target else {
                return fail("No player was found");
            };
            let current = h.player_state(target)?.location;
            let dest = if rest.len() == 3 {
                let coord = |s: &str, base: f64| -> Option<f64> {
                    match s.strip_prefix('~') {
                        Some("") => Some(base),
                        Some(d) => d.parse::<f64>().ok().map(|d| base + d),
                        None => s.parse().ok(),
                    }
                };
                match (
                    coord(rest[0], current.x),
                    coord(rest[1], current.y),
                    coord(rest[2], current.z),
                ) {
                    (Some(x), Some(y), Some(z)) => Location { x, y, z, ..current },
                    _ => return fail("Invalid coordinates"),
                }
            } else if let Some(other) = rest.first().and_then(|n| h.state.borrow().by_name(n)) {
                h.player_state(other)?.location
            } else {
                return fail("Usage: /tp [player] <x y z | player>");
            };
            Some(h.teleport(target, dest))
        }
        "time" => {
            let value = |s: &str| -> Option<u64> {
                match s {
                    "day" => Some(1000),
                    "noon" => Some(6000),
                    "night" => Some(13000),
                    "midnight" => Some(18000),
                    n => n.parse().ok(),
                }
            };
            let world = sender
                .player()
                .and_then(|u| h.player_state(u))
                .map(|p| p.location.world)
                .unwrap_or_else(|| OVERWORLD.to_owned());
            let mut st = h.st();
            let Some(w) = st.worlds.get_mut(&world) else {
                return Some(false);
            };
            match (args.first().copied(), args.get(1).and_then(|v| value(v))) {
                (Some("set"), Some(t)) => w.time = t,
                (Some("add"), Some(t)) => w.time = w.time.wrapping_add(t),
                _ => {
                    drop(st);
                    return fail("Usage: /time set|add <value>");
                }
            }
            Some(true)
        }
        "weather" => {
            let weather = match args.first().copied() {
                Some("clear") => Weather::Clear,
                Some("rain") => Weather::Rain,
                Some("thunder") => Weather::Thunder,
                _ => return fail("Usage: /weather clear|rain|thunder [ticks]"),
            };
            let ticks = args.get(1).and_then(|t| t.parse().ok()).unwrap_or(6000);
            let mut st = h.st();
            for w in st.worlds.values_mut() {
                w.weather = weather;
                w.weather_ticks = ticks;
            }
            Some(true)
        }
        "kill" => {
            let Some(target) = player(args.first()) else {
                return fail("No player was found");
            };
            h.die(target, "minecraft:generic_kill");
            Some(true)
        }
        "kick" => {
            let Some(target) = player(args.first()) else {
                return fail("No player was found");
            };
            let reason = if args.len() > 1 {
                args[1..].join(" ")
            } else {
                "Kicked by an operator".to_owned()
            };
            if let Some(p) = h.st().online_player_mut(target) {
                p.kicked = Some(Text::plain(reason));
            }
            h.quit(target);
            Some(true)
        }
        "op" | "deop" => {
            let Some(target) = args.first().and_then(|n| {
                h.state
                    .borrow()
                    .players
                    .values()
                    .find(|p| p.name.eq_ignore_ascii_case(n))
                    .map(|p| p.uuid)
            }) else {
                return fail("That player does not exist");
            };
            let mut st = h.st();
            if label == "op" {
                st.ops.insert(target);
            } else {
                st.ops.remove(&target);
            }
            Some(true)
        }
        "list" => {
            let names: Vec<String> = {
                let st = h.state.borrow();
                st.online
                    .iter()
                    .map(|u| st.players[u].name.clone())
                    .collect()
            };
            let st_max = h.state.borrow().max_players;
            h.tell_sender(
                sender,
                Text::plain(format!(
                    "There are {} of a max of {st_max} players online: {}",
                    names.len(),
                    names.join(", ")
                )),
            );
            Some(true)
        }
        "setworldspawn" => {
            let Some(p) = sender.player().and_then(|u| h.player_state(u)) else {
                return fail("Only players can set the world spawn without coordinates");
            };
            let mut st = h.st();
            if let Some(w) = st.worlds.get_mut(&p.location.world) {
                w.spawn = p.location.clone();
            }
            Some(true)
        }
        _ => None,
    }
}
