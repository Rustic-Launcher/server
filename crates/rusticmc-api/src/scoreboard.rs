//! Scoreboard objectives and teams (on the server's main scoreboard), boss bars and the tab list.
//!
//! Objective and team names are prefixed with the plugin id (`<id>.<name>`) so plugins never
//! collide; the methods take the short name.

use crate::__rt;
use crate::bindings::rustic::api::{bossbars, scoreboards as wit, tablist};
use crate::error::Result;
use crate::host::call;
use crate::player::Player;
use crate::text::Text;
use crate::types::{BarColor, BarStyle, DisplaySlot, TeamColor};

fn full_name(short: &str) -> String {
    format!("{}.{short}", __rt::plugin_id())
}

/// The main scoreboard.
#[derive(Debug, Clone, Copy)]
pub struct Scoreboard;

impl Scoreboard {
    /// Creates an objective (`criteria` is usually `"dummy"`).
    pub fn create_objective(
        name: &str,
        display: impl Into<Text>,
        criteria: &str,
    ) -> Result<Objective> {
        let full = full_name(name);
        let json = display.into().to_json();
        call!(
            "scoreboards.create-objective",
            wit::create_objective(&full, &json, criteria)
        )?;
        Ok(Objective { name: full })
    }

    /// A handle for an objective created earlier (no host call).
    pub fn objective(name: &str) -> Objective {
        Objective {
            name: full_name(name),
        }
    }

    /// Clears a display slot.
    pub fn clear_display(slot: DisplaySlot) -> Result<()> {
        Ok(call!(
            "scoreboards.set-display",
            wit::set_display(slot, None)
        )?)
    }

    /// Creates a team.
    pub fn create_team(name: &str) -> Result<Team> {
        let full = full_name(name);
        call!("scoreboards.create-team", wit::create_team(&full))?;
        Ok(Team { name: full })
    }

    /// A handle for a team created earlier (no host call).
    pub fn team(name: &str) -> Team {
        Team {
            name: full_name(name),
        }
    }
}

/// A scoreboard objective of this plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Objective {
    name: String,
}

impl Objective {
    /// The full (prefixed) name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Shows it in a display slot (sidebar, player list, below name).
    pub fn display_in(&self, slot: DisplaySlot) -> Result<()> {
        Ok(call!(
            "scoreboards.set-display",
            wit::set_display(slot, Some(&self.name))
        )?)
    }

    /// Changes the title.
    pub fn set_display_name(&self, display: impl Into<Text>) -> Result<()> {
        let json = display.into().to_json();
        Ok(call!(
            "scoreboards.set-objective-display",
            wit::set_objective_display(&self.name, &json)
        )?)
    }

    /// Sets an entry's score (an entry is a player name or any text line).
    pub fn set_score(&self, entry: &str, score: i32) -> Result<()> {
        Ok(call!(
            "scoreboards.set-score",
            wit::set_score(&self.name, entry, score)
        )?)
    }

    /// An entry's score.
    pub fn score(&self, entry: &str) -> Option<i32> {
        call!("scoreboards.get-score", wit::get_score(&self.name, entry))
    }

    /// Removes an entry.
    pub fn reset_score(&self, entry: &str) -> Result<()> {
        Ok(call!(
            "scoreboards.reset-score",
            wit::reset_score(&self.name, entry)
        )?)
    }

    /// Deletes the objective.
    pub fn remove(self) -> Result<()> {
        Ok(call!(
            "scoreboards.remove-objective",
            wit::remove_objective(&self.name)
        )?)
    }
}

/// A team of this plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Team {
    name: String,
}

impl Team {
    /// The full (prefixed) name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Text before members' names.
    pub fn set_prefix(&self, prefix: impl Into<Text>) -> Result<()> {
        let json = prefix.into().to_json();
        Ok(call!(
            "scoreboards.set-team-prefix",
            wit::set_team_prefix(&self.name, &json)
        )?)
    }

    /// Text after members' names.
    pub fn set_suffix(&self, suffix: impl Into<Text>) -> Result<()> {
        let json = suffix.into().to_json();
        Ok(call!(
            "scoreboards.set-team-suffix",
            wit::set_team_suffix(&self.name, &json)
        )?)
    }

    /// Name color.
    pub fn set_color(&self, color: TeamColor) -> Result<()> {
        Ok(call!(
            "scoreboards.set-team-color",
            wit::set_team_color(&self.name, color)
        )?)
    }

    /// Adds an entry (player name).
    pub fn add(&self, entry: &str) -> Result<()> {
        Ok(call!(
            "scoreboards.team-add",
            wit::team_add(&self.name, entry)
        )?)
    }

    /// Removes an entry.
    pub fn remove_entry(&self, entry: &str) -> Result<()> {
        Ok(call!(
            "scoreboards.team-remove",
            wit::team_remove(&self.name, entry)
        )?)
    }

    /// Deletes the team.
    pub fn remove(self) -> Result<()> {
        Ok(call!(
            "scoreboards.remove-team",
            wit::remove_team(&self.name)
        )?)
    }
}

/// A boss bar. Removed by the host when the plugin is disabled.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BossBar {
    id: u32,
}

impl BossBar {
    /// Creates a bar (`progress` 0.0..=1.0); nobody sees it until [`BossBar::show`].
    pub fn create(
        title: impl Into<Text>,
        color: BarColor,
        style: BarStyle,
        progress: f32,
    ) -> BossBar {
        let json = title.into().to_json();
        let id = call!(
            "bossbars.create",
            bossbars::create(&json, color, style, progress.clamp(0.0, 1.0))
        );
        BossBar { id }
    }

    /// The host's id.
    pub fn id(&self) -> u32 {
        self.id
    }

    /// Changes the title.
    pub fn set_title(&self, title: impl Into<Text>) {
        let json = title.into().to_json();
        call!("bossbars.set-title", bossbars::set_title(self.id, &json));
    }

    /// Changes the progress (clamped to 0.0..=1.0).
    pub fn set_progress(&self, progress: f32) {
        call!(
            "bossbars.set-progress",
            bossbars::set_progress(self.id, progress.clamp(0.0, 1.0))
        );
    }

    /// Changes the color.
    pub fn set_color(&self, color: BarColor) {
        call!("bossbars.set-color", bossbars::set_color(self.id, color));
    }

    /// Shows it to a player.
    pub fn show(&self, player: &Player) -> Result<()> {
        Ok(call!(
            "bossbars.show",
            bossbars::show(self.id, player.uuid())
        )?)
    }

    /// Hides it from a player.
    pub fn hide(&self, player: &Player) {
        call!("bossbars.hide", bossbars::hide(self.id, player.uuid()));
    }

    /// Deletes it.
    pub fn remove(self) {
        call!("bossbars.remove", bossbars::remove(self.id));
    }
}

/// The tab (player) list.
#[derive(Debug, Clone, Copy)]
pub struct TabList;

impl TabList {
    /// Sets the header and footer a player sees.
    pub fn set_header_footer(
        player: &Player,
        header: impl Into<Text>,
        footer: impl Into<Text>,
    ) -> Result<()> {
        player.set_tab_header_footer(header, footer)
    }

    /// The name shown for `player` in everyone's list (`None` restores it).
    pub fn set_display_name(player: &Player, name: Option<Text>) -> Result<()> {
        let json = name.map(|t| t.to_json());
        Ok(call!(
            "tablist.set-display-name",
            tablist::set_display_name(player.uuid(), json.as_deref())
        )?)
    }
}
