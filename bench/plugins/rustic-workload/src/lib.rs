//! The benchmark workload as a Rustic WASM plugin (docs/PERFORMANCE.md, "Plugin overhead").
//! The same workload exists as a Bukkit plugin (`bench/bukkit/workload`); keep the two
//! equivalent:
//!
//! * player move: per-player move count and distance travelled (from -> to);
//! * chat: lower-case the message, look for five banned words, cancel and warn on a match;
//! * `/workload`: replies with the counters;
//! * a task every tick: every online player's location, the farthest from (0, 0) kept.

use rustic::prelude::*;
use std::collections::HashMap;

const BANNED: [&str; 5] = ["badword", "spoiler", "griefer", "lagmachine", "dupe"];

/// Per player: moves seen and blocks travelled.
#[derive(Debug, Default, Clone, Copy)]
struct Track {
    moves: u64,
    distance: f64,
}

#[rustic::plugin]
#[derive(Default)]
struct Workload {
    tracks: HashMap<(u64, u64), Track>,
    chats: u64,
    blocked: u64,
    ticks: u64,
    farthest: f64,
}

impl Plugin for Workload {
    fn enable(&mut self, ctx: &mut Ctx) -> Result<()> {
        Scheduler::run_timer(1, 1, |_task| with_plugin(Workload::tick));
        if ctx.is_main() {
            info!("workload enabled");
        }
        Ok(())
    }
}

/// The word of [`BANNED`] that `message` contains, if any (case-insensitive).
fn banned_word(message: &str) -> Option<&'static str> {
    let message = message.to_lowercase();
    BANNED.into_iter().find(|w| message.contains(w))
}

/// Straight-line distance between two locations.
fn distance(from: &Location, to: &Location) -> f64 {
    let (dx, dy, dz) = (to.x - from.x, to.y - from.y, to.z - from.z);
    (dx * dx + dy * dy + dz * dz).sqrt()
}

impl Workload {
    fn tick(&mut self) {
        self.ticks += 1;
        for player in Player::online() {
            if let Ok(l) = player.location() {
                self.farthest = self.farthest.max(l.x.hypot(l.z));
            }
        }
    }

    fn report(&self) -> String {
        let (moves, dist) = self
            .tracks
            .values()
            .fold((0, 0.0), |(m, d), t| (m + t.moves, d + t.distance));
        format!(
            "workload: {} ticks, {} moves ({:.1} blocks), {} chats ({} blocked), farthest {:.1}",
            self.ticks, moves, dist, self.chats, self.blocked, self.farthest
        )
    }
}

#[rustic::handlers]
impl Workload {
    #[event_handler]
    fn on_move(&mut self, event: &mut PlayerMoveEvent) {
        let uuid = event.player().uuid();
        let track = self.tracks.entry((uuid.hi, uuid.lo)).or_default();
        track.moves += 1;
        track.distance += distance(event.from(), event.to());
    }

    #[event_handler(ignore_cancelled = true)]
    fn on_chat(&mut self, event: &mut PlayerChatEvent) {
        self.chats += 1;
        if let Some(word) = banned_word(event.message()) {
            event.cancel();
            self.blocked += 1;
            event
                .player()
                .send_message(Text::new(format!("Please do not say '{word}'.")));
        }
    }

    #[command(name = "workload", description = "Show the workload counters")]
    fn workload(&mut self, cmd: &mut CommandCtx) -> CommandResult {
        cmd.reply(Text::new(self.report()));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn banned_words_match_case_insensitively() {
        assert_eq!(banned_word("no DUPE here"), Some("dupe"));
        assert_eq!(banned_word("hello there"), None);
    }

    #[test]
    fn distance_is_euclidean() {
        let at = |x, y, z| Location {
            world: "minecraft:overworld".into(),
            x,
            y,
            z,
            yaw: 0.0,
            pitch: 0.0,
        };
        assert!((distance(&at(0.0, 0.0, 0.0), &at(3.0, 4.0, 0.0)) - 5.0).abs() < 1e-9);
    }

    #[test]
    fn report_sums_every_player() {
        let mut w = Workload::default();
        w.tracks.insert(
            (0, 1),
            Track {
                moves: 2,
                distance: 1.5,
            },
        );
        w.tracks.insert(
            (0, 2),
            Track {
                moves: 3,
                distance: 2.0,
            },
        );
        assert!(w.report().contains("5 moves (3.5 blocks)"));
    }
}
