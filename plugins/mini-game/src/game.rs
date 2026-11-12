//! One arena's game: players, phase, scores (pure state machine, advanced once per second by
//! the plugin's timer and once per tick for KotH captures).

use std::collections::{BTreeMap, BTreeSet};

use rustic::Uuid;

use crate::arena::Pos;

/// Timing and size rules.
#[derive(Debug, Clone, Copy)]
pub struct Rules {
    pub min_players: usize,
    pub max_players: usize,
    pub countdown: u32,
    pub round: u32,
    pub end: u32,
    /// KotH: points (seconds held alone) to win.
    pub koth_points: u32,
}

/// Where a game is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// In the lobby, not enough players.
    Waiting,
    /// Starting in this many seconds.
    Countdown(u32),
    /// Playing, this many seconds left.
    Running(u32),
    /// Finished; players go back in this many seconds.
    Ending(u32),
}

/// What one second changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Nothing to show.
    Idle,
    /// Countdown: seconds left.
    Countdown(u32),
    /// Not enough players any more: back to waiting.
    Aborted,
    /// The round starts now.
    Start,
    /// Round: seconds left.
    Round(u32),
    /// The time is up (the plugin then calls [`Game::finish`]).
    TimeUp,
    /// Ending: seconds left before everyone goes back.
    Ending(u32),
    /// Over: send everyone back, reset the arena.
    Reset,
}

/// Why a player cannot join.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JoinError {
    Already,
    Full,
    InProgress,
}

/// What a KotH tick found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Capture {
    /// The only player in the zone.
    pub holder: Option<Uuid>,
    /// Several players in the zone.
    pub contested: bool,
    /// Whether the holder scored a point this tick.
    pub scored: bool,
    /// The holder reached the target.
    pub won: Option<Uuid>,
}

/// The game of one arena.
#[derive(Debug, Clone)]
pub struct Game {
    pub phase: Phase,
    /// Everyone in the game, in join order (eliminated players included).
    pub players: Vec<Uuid>,
    /// Spleef: still standing. KotH: everyone.
    pub alive: Vec<Uuid>,
    /// Spleef: blocks broken. KotH: points.
    pub scores: BTreeMap<Uuid, u32>,
    /// KotH: ticks held toward the next point.
    hold: BTreeMap<Uuid, u32>,
    /// Spleef: floor blocks broken this round.
    pub broken: BTreeSet<Pos>,
    /// Started with `/mg start`: no minimum.
    pub forced: bool,
    /// Players when the round started.
    pub started_with: usize,
    pub winner: Option<Uuid>,
}

impl Default for Game {
    fn default() -> Self {
        Game {
            phase: Phase::Waiting,
            players: Vec::new(),
            alive: Vec::new(),
            scores: BTreeMap::new(),
            hold: BTreeMap::new(),
            broken: BTreeSet::new(),
            forced: false,
            started_with: 0,
            winner: None,
        }
    }
}

impl Game {
    /// Adds a player; reaching the minimum starts the countdown.
    pub fn join(&mut self, p: Uuid, rules: &Rules) -> Result<(), JoinError> {
        if self.players.contains(&p) {
            return Err(JoinError::Already);
        }
        if matches!(self.phase, Phase::Running(_) | Phase::Ending(_)) {
            return Err(JoinError::InProgress);
        }
        if self.players.len() >= rules.max_players.max(1) {
            return Err(JoinError::Full);
        }
        self.players.push(p);
        if self.phase == Phase::Waiting && self.players.len() >= rules.min_players.max(1) {
            self.phase = Phase::Countdown(rules.countdown.max(1));
        }
        Ok(())
    }

    /// Removes a player (left or disconnected); returns whether they were in.
    pub fn leave(&mut self, p: Uuid) -> bool {
        let was = self.players.contains(&p);
        self.players.retain(|u| *u != p);
        self.alive.retain(|u| *u != p);
        self.hold.remove(&p);
        was
    }

    /// Starts the countdown now, whatever the number of players (at least one).
    pub fn force_start(&mut self, rules: &Rules) -> bool {
        if self.players.is_empty() || matches!(self.phase, Phase::Running(_) | Phase::Ending(_)) {
            return false;
        }
        self.forced = true;
        let n = match self.phase {
            Phase::Countdown(n) => n.min(5),
            _ => rules.countdown.clamp(1, 5),
        };
        self.phase = Phase::Countdown(n);
        true
    }

    /// One second passes.
    pub fn second(&mut self, rules: &Rules) -> Step {
        match self.phase {
            Phase::Waiting => Step::Idle,
            Phase::Countdown(n) => {
                if self.players.is_empty()
                    || (!self.forced && self.players.len() < rules.min_players.max(1))
                {
                    self.phase = Phase::Waiting;
                    self.forced = false;
                    return Step::Aborted;
                }
                if n <= 1 {
                    self.phase = Phase::Running(rules.round.max(1));
                    self.alive = self.players.clone();
                    self.scores = self.players.iter().map(|p| (*p, 0)).collect();
                    self.hold.clear();
                    self.broken.clear();
                    self.started_with = self.players.len();
                    self.winner = None;
                    Step::Start
                } else {
                    self.phase = Phase::Countdown(n - 1);
                    Step::Countdown(n - 1)
                }
            }
            Phase::Running(n) => {
                if n <= 1 {
                    self.phase = Phase::Running(0);
                    Step::TimeUp
                } else {
                    self.phase = Phase::Running(n - 1);
                    Step::Round(n - 1)
                }
            }
            Phase::Ending(n) => {
                if n <= 1 {
                    Step::Reset
                } else {
                    self.phase = Phase::Ending(n - 1);
                    Step::Ending(n - 1)
                }
            }
        }
    }

    /// Ends the round with `winner` (or a draw).
    pub fn finish(&mut self, winner: Option<Uuid>, rules: &Rules) {
        self.winner = winner;
        self.phase = Phase::Ending(rules.end.max(1));
    }

    /// Whether the round is being played.
    pub fn running(&self) -> bool {
        matches!(self.phase, Phase::Running(_))
    }

    /// The unique best score among `among` (a tie or no point at all: none).
    pub fn leader_among(&self, among: &[Uuid]) -> Option<Uuid> {
        let mut best: Option<(Uuid, u32)> = None;
        let mut tie = false;
        for p in among {
            let s = self.scores.get(p).copied().unwrap_or(0);
            match best {
                Some((_, b)) if s == b => tie = true,
                Some((_, b)) if s < b => {}
                _ => {
                    best = Some((*p, s));
                    tie = false;
                }
            }
        }
        match best {
            Some((p, s)) if !tie && (s > 0 || among.len() == 1) => Some(p),
            _ => None,
        }
    }

    /// Spleef: a player fell. Returns whether they were still standing.
    pub fn eliminate(&mut self, p: Uuid) -> bool {
        let was = self.alive.contains(&p);
        self.alive.retain(|u| *u != p);
        was
    }

    /// Spleef: whether the round is decided by eliminations; `Some(winner)` (a draw when
    /// nobody is left).
    pub fn last_standing(&self) -> Option<Option<Uuid>> {
        if !self.running() {
            return None;
        }
        match self.alive.len() {
            0 => Some(None),
            1 if self.started_with > 1 => Some(Some(self.alive[0])),
            _ => None,
        }
    }

    /// Spleef: a floor block broken by `p`; returns whether it counted (first time).
    pub fn block_broken(&mut self, p: Uuid, pos: Pos) -> bool {
        if !self.running() || !self.alive.contains(&p) || !self.broken.insert(pos) {
            return false;
        }
        *self.scores.entry(p).or_default() += 1;
        true
    }

    /// KotH: one tick with these players in the zone.
    pub fn capture_tick(&mut self, in_zone: &[Uuid], rules: &Rules) -> Capture {
        let inside: Vec<Uuid> = in_zone
            .iter()
            .copied()
            .filter(|p| self.alive.contains(p))
            .collect();
        let mut c = Capture {
            contested: inside.len() > 1,
            ..Capture::default()
        };
        if !self.running() || inside.len() != 1 {
            return c;
        }
        let p = inside[0];
        c.holder = Some(p);
        let ticks = self.hold.entry(p).or_default();
        *ticks += 1;
        if *ticks >= 20 {
            *ticks = 0;
            let score = self.scores.entry(p).or_default();
            *score += 1;
            c.scored = true;
            if *score >= rules.koth_points.max(1) {
                c.won = Some(p);
            }
        }
        c
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RULES: Rules = Rules {
        min_players: 2,
        max_players: 3,
        countdown: 3,
        round: 4,
        end: 2,
        koth_points: 2,
    };

    fn u(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    #[test]
    fn joining_and_countdown() {
        let mut g = Game::default();
        assert_eq!(g.join(u(1), &RULES), Ok(()));
        assert_eq!(g.join(u(1), &RULES), Err(JoinError::Already));
        assert_eq!(g.phase, Phase::Waiting);
        assert_eq!(g.second(&RULES), Step::Idle);
        g.join(u(2), &RULES).unwrap();
        assert_eq!(g.phase, Phase::Countdown(3));
        g.join(u(3), &RULES).unwrap();
        assert_eq!(g.join(u(4), &RULES), Err(JoinError::Full));
        assert_eq!(g.second(&RULES), Step::Countdown(2));
        // Dropping below the minimum aborts.
        g.leave(u(2));
        g.leave(u(3));
        assert_eq!(g.second(&RULES), Step::Aborted);
        assert_eq!(g.phase, Phase::Waiting);
    }

    #[test]
    fn a_full_round() {
        let mut g = Game::default();
        g.join(u(1), &RULES).unwrap();
        g.join(u(2), &RULES).unwrap();
        assert_eq!(g.second(&RULES), Step::Countdown(2));
        assert_eq!(g.second(&RULES), Step::Countdown(1));
        assert_eq!(g.second(&RULES), Step::Start);
        assert_eq!(g.phase, Phase::Running(4));
        assert_eq!(g.alive.len(), 2);
        assert_eq!(g.join(u(3), &RULES), Err(JoinError::InProgress));
        assert_eq!(g.second(&RULES), Step::Round(3));
        assert_eq!(g.second(&RULES), Step::Round(2));
        assert_eq!(g.second(&RULES), Step::Round(1));
        assert_eq!(g.second(&RULES), Step::TimeUp);
        let leader = g.leader_among(&g.alive.clone());
        assert_eq!(leader, None); // 0-0: a draw
        g.finish(leader, &RULES);
        assert_eq!(g.second(&RULES), Step::Ending(1));
        assert_eq!(g.second(&RULES), Step::Reset);
    }

    #[test]
    fn spleef_scores_and_last_standing() {
        let mut g = Game::default();
        g.join(u(1), &RULES).unwrap();
        g.join(u(2), &RULES).unwrap();
        g.join(u(3), &RULES).unwrap();
        assert!(!g.block_broken(u(1), Pos::new(0, 0, 0))); // not running yet
        for _ in 0..3 {
            g.second(&RULES);
        }
        assert!(g.running());
        assert!(g.block_broken(u(1), Pos::new(0, 0, 0)));
        assert!(!g.block_broken(u(2), Pos::new(0, 0, 0))); // already broken
        assert!(g.block_broken(u(2), Pos::new(1, 0, 0)));
        assert!(g.block_broken(u(2), Pos::new(2, 0, 0)));
        assert_eq!(g.last_standing(), None);
        assert!(g.eliminate(u(3)));
        assert!(!g.eliminate(u(3)));
        assert_eq!(g.last_standing(), None);
        g.eliminate(u(2));
        assert_eq!(g.last_standing(), Some(Some(u(1))));
        assert!(!g.block_broken(u(2), Pos::new(5, 0, 0))); // eliminated
        assert_eq!(g.leader_among(&[u(1), u(2)]), Some(u(2)));
        g.eliminate(u(1));
        assert_eq!(g.last_standing(), Some(None));
    }

    #[test]
    fn koth_capture() {
        let mut g = Game::default();
        g.join(u(1), &RULES).unwrap();
        g.join(u(2), &RULES).unwrap();
        for _ in 0..3 {
            g.second(&RULES);
        }
        let c = g.capture_tick(&[u(1), u(2)], &RULES);
        assert!(c.contested && c.holder.is_none());
        let mut won = None;
        let mut scored = 0;
        for _ in 0..40 {
            let c = g.capture_tick(&[u(1)], &RULES);
            assert_eq!(c.holder, Some(u(1)));
            scored += usize::from(c.scored);
            won = won.or(c.won);
        }
        assert_eq!(scored, 2);
        assert_eq!(won, Some(u(1)));
        assert_eq!(g.scores[&u(1)], 2);
        assert_eq!(g.capture_tick(&[u(9)], &RULES), Capture::default());
    }

    #[test]
    fn forced_start_and_solo() {
        let mut g = Game::default();
        assert!(!g.force_start(&RULES));
        g.join(u(1), &RULES).unwrap();
        assert!(g.force_start(&RULES));
        assert_eq!(g.phase, Phase::Countdown(3));
        assert_eq!(g.second(&RULES), Step::Countdown(2));
        g.second(&RULES);
        assert_eq!(g.second(&RULES), Step::Start);
        assert_eq!(g.last_standing(), None); // solo: only the clock ends it
        assert_eq!(g.leader_among(&[u(1)]), Some(u(1)));
        // Everyone leaving aborts a countdown.
        let mut h = Game::default();
        h.join(u(1), &RULES).unwrap();
        h.force_start(&RULES);
        h.leave(u(1));
        assert_eq!(h.second(&RULES), Step::Aborted);
    }
}
