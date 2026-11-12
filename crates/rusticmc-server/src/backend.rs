//! `rusticmc_core::Backend` over Horizon's `Host` (docs/ARCHITECTURE.md §5.1).
//!
//! Two modes:
//!
//! * **with a host**: inside every Horizon hook that gets `host: &mut dyn Host`, calls go straight
//!   to the game;
//! * **without**: the 44 server hooks that get no host (`player_chat_v2`, `player_move`, ...) and
//!   anything else outside a hook. Reads are served from the [`World`] snapshot taken at each
//!   server tick (online players, motd, limit) and writes are queued, then run in order at the
//!   next `server_tick` (`ServerTickEnd`), where a host is available.
//!
//! Every method maps one WIT import; what Horizon has no helper for stays `Unsupported` (the core's
//! default). Re-entrancy: a `Host` call may fire hooks again (`teleport_player` ->
//! `player_teleport`); the core gives each nested runtime entry its own backend borrow (ADR 0004)
//! and this type holds no lock while it calls the host.
use horizon_client::mod_api::{Host, NativeBossBar, NativeInventoryKind, NativeServerPlayer};
use rusticmc_core::Backend;
use rusticmc_core::model::{
    BarColor, BarId, BarStyle, BlockPos, BlockState, DisplaySlot, EntityRef, GameMode, HostError,
    ItemStack, Location, MenuId, MenuSpec, TeamColor, Text, TickStats, Uuid, Vec3, Weather,
};

use std::rc::Rc;

use crate::{convert, ui};

/// A write that waits for a host.
pub type Deferred = Box<dyn FnOnce(&mut dyn Host)>;

/// What the adapter knows of the game between hooks, besides the players' snapshot
/// ([`crate::server::snapshot`]): the server's motd and limit, and the writes no-host hooks queue.
#[derive(Default)]
pub struct World {
    pub motd: String,
    pub max_players: Option<u32>,
    /// Writes made without a host, run at the next `server_tick`.
    pub deferred: Vec<Deferred>,
    /// The core changed its commands: re-register them (`host.refresh_commands()`).
    pub commands_changed: bool,
}

/// The game as one runtime entry point (`fire`, `tick`, `dispatch_command`...) sees it.
pub struct HorizonBackend<'h, 'w> {
    host: Option<&'h mut dyn Host>,
    world: &'w mut World,
    /// `host.server_players()` as last read, kept until the next host call that may change the
    /// game. The game does not advance while a runtime entry runs except through this backend's own
    /// host calls (which clear it), so a plugin loop over N players costs one `server_players()`,
    /// not N (docs/PERFORMANCE.md: 17 µs per `location()` at 50 players before).
    players: Option<Rc<Vec<NativeServerPlayer>>>,
}

impl<'h, 'w> HorizonBackend<'h, 'w> {
    pub fn new(host: Option<&'h mut dyn Host>, world: &'w mut World) -> Self {
        Self {
            host,
            world,
            players: None,
        }
    }

    /// The host for a call that may change the game (or fire hooks that do): drops the players'
    /// cache first.
    fn host_mut(&mut self) -> Option<&mut dyn Host> {
        self.players = None;
        self.host.as_deref_mut().map(|h| h as &mut dyn Host)
    }

    /// A read that needs the game: the host when there is one, else the snapshot through `snap`.
    fn read<R>(
        &mut self,
        live: impl FnOnce(&mut dyn Host) -> R,
        snap: impl FnOnce(&World) -> Result<R, HostError>,
    ) -> Result<R, HostError> {
        match self.host_mut() {
            Some(host) => Ok(live(host)),
            None => snap(self.world),
        }
    }

    /// A read only the game can answer.
    fn live<R>(&mut self, live: impl FnOnce(&mut dyn Host) -> R) -> Result<R, HostError> {
        match self.host_mut() {
            Some(host) => Ok(live(host)),
            None => Err(HostError::Unsupported(
                "this call needs the game; it is not reachable from this event (try a task)".into(),
            )),
        }
    }

    /// A write: now with a host, else queued for the next tick.
    fn write(&mut self, write: impl FnOnce(&mut dyn Host) + 'static) -> Result<(), HostError> {
        match self.host_mut() {
            Some(host) => write(host),
            None => self.world.deferred.push(Box::new(write)),
        }
        Ok(())
    }

    /// The player's record, live (cached, see `players`) or from the snapshot.
    fn player(&mut self, player: Uuid) -> Result<NativeServerPlayer, HostError> {
        let raw = convert::raw(player);
        self.players()
            .iter()
            .find(|p| p.uuid == raw)
            .cloned()
            .ok_or(HostError::NoSuchPlayer)
    }

    /// A console command, now or at the end of the tick.
    fn command(&mut self, line: String) -> Result<(), HostError> {
        self.write(move |h| {
            h.run_command(&line);
        })
    }

    /// Changes a boss bar and sends it again to who sees it.
    fn bar_changed(&mut self, bar: BarId, f: impl FnOnce(&mut ui::Bar)) -> Result<(), HostError> {
        let changed = ui::update_bar(bar, f)
            .ok_or_else(|| HostError::Invalid(format!("no boss bar {bar}")))?;
        self.write(move |h| {
            for viewer in &changed.viewers {
                h.boss_bar(*viewer, changed.id, Some(&changed.bar));
            }
        })
    }

    /// The online players: live (read once, then cached until a host call that may change the
    /// game) or the tick's snapshot.
    fn players(&mut self) -> Rc<Vec<NativeServerPlayer>> {
        let Some(host) = self.host.as_deref_mut() else {
            return crate::server::snapshot();
        };
        self.players
            .get_or_insert_with(|| Rc::new(host.server_players()))
            .clone()
    }
}

fn item(stack: Option<&horizon_client::items::ItemStack>) -> ItemStack {
    convert::stack_to_core(stack)
}

/// A level by dimension id, or `NoSuchWorld`.
fn level(h: &dyn Host, world: &str) -> Result<horizon_client::mod_api::LevelRef, HostError> {
    h.level(world).ok_or(HostError::NoSuchWorld)
}

/// A component as a command argument (vanilla's commands read SNBT; component JSON is SNBT too).
fn component_arg(text: &Text) -> String {
    text.as_json().to_owned()
}

/// A scoreboard name or entry as one command word: refused when it would split the command.
fn word(name: &str) -> Result<&str, HostError> {
    if name.is_empty() || name.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return Err(HostError::Invalid(format!("`{name}` is not one word")));
    }
    Ok(name)
}

fn bar_color(color: BarColor) -> i32 {
    match color {
        BarColor::Pink => 0,
        BarColor::Blue => 1,
        BarColor::Red => 2,
        BarColor::Green => 3,
        BarColor::Yellow => 4,
        BarColor::Purple => 5,
        BarColor::White => 6,
    }
}

fn bar_style(style: BarStyle) -> i32 {
    match style {
        BarStyle::Progress => 0,
        BarStyle::Notched6 => 1,
        BarStyle::Notched10 => 2,
        BarStyle::Notched12 => 3,
        BarStyle::Notched20 => 4,
    }
}

fn team_color(color: TeamColor) -> &'static str {
    match color {
        TeamColor::Black => "black",
        TeamColor::DarkBlue => "dark_blue",
        TeamColor::DarkGreen => "dark_green",
        TeamColor::DarkAqua => "dark_aqua",
        TeamColor::DarkRed => "dark_red",
        TeamColor::DarkPurple => "dark_purple",
        TeamColor::Gold => "gold",
        TeamColor::Gray => "gray",
        TeamColor::DarkGray => "dark_gray",
        TeamColor::Blue => "blue",
        TeamColor::Green => "green",
        TeamColor::Aqua => "aqua",
        TeamColor::Red => "red",
        TeamColor::LightPurple => "light_purple",
        TeamColor::Yellow => "yellow",
        TeamColor::White => "white",
        TeamColor::Reset => "reset",
    }
}

/// `count` of `id` (plain or not) out of `slots`, last slots first as Bukkit's `removeItem`
/// does not promise an order; the slots to change and what to leave there.
fn take_plan(
    slots: &[Option<horizon_client::items::ItemStack>],
    id: &str,
    mut count: u32,
) -> Vec<(usize, Option<horizon_client::items::ItemStack>)> {
    let mut plan = Vec::new();
    for (i, slot) in slots.iter().enumerate() {
        if count == 0 {
            break;
        }
        let Some(stack) = slot.as_ref().filter(|s| &*s.item == id) else {
            continue;
        };
        let taken = stack.count.min(count);
        count -= taken;
        let left = (stack.count > taken).then(|| {
            let mut rest = stack.clone();
            rest.count -= taken;
            rest
        });
        plan.push((i, left));
    }
    plan
}

/// The text a `run_command_captured` answered, for an error.
fn answer(result: &horizon_client::mod_api::NativeCommandResult) -> String {
    result
        .output
        .iter()
        .map(|t| t.plain())
        .collect::<Vec<_>>()
        .join(" / ")
}

impl Backend for HorizonBackend<'_, '_> {
    // ---- server ----
    fn server_broadcast(&mut self, message: &Text) -> Result<(), HostError> {
        let line = convert::to_horizon(message);
        self.write(move |h| h.broadcast(line))
    }

    fn server_dispatch_command(&mut self, command: &str) -> Result<bool, HostError> {
        let line = command.trim_start_matches('/').to_owned();
        self.live(move |h| h.run_mod_command(&line) || h.run_command(&line))
    }

    fn server_get_tick_stats(&mut self) -> Result<TickStats, HostError> {
        self.live(|h| {
            let (rate, mspt) = h.tick_stats();
            let tps = h.tps()[0];
            TickStats {
                mspt,
                tps: if tps > 0.0 { tps } else { f64::from(rate) },
            }
        })
    }

    fn server_max_players(&mut self) -> Result<u32, HostError> {
        self.read(
            |h| h.max_players().unwrap_or(0),
            |w| Ok(w.max_players.unwrap_or(0)),
        )
    }

    fn server_motd(&mut self) -> Result<Text, HostError> {
        let motd = self.read(|h| h.motd(), |w| Ok(w.motd.clone()))?;
        Ok(Text::plain(motd))
    }

    fn server_version(&mut self) -> Result<String, HostError> {
        Ok(crate::server::version_line())
    }

    fn server_offline_player(&mut self, name: &str) -> Result<Option<Uuid>, HostError> {
        self.live(|h| h.profile_by_name(name).map(|(u, _)| convert::uuid(u)))
    }

    /// A reply to the console: inside a console command Horizon's `feedback` gives it to whoever ran
    /// it (the console, RCON, `horizon-server cmd`, the panel), as vanilla commands' answers; with
    /// no host, the log.
    fn console_message(&mut self, message: &Text) -> Result<(), HostError> {
        match self.host_mut() {
            Some(host) => host.feedback(convert::to_horizon(message)),
            None => log::info!(target: "rustic", "{}", message.to_plain()),
        }
        Ok(())
    }

    // ---- players ----
    fn players_online(&mut self) -> Result<Vec<Uuid>, HostError> {
        Ok(self
            .players()
            .iter()
            .map(|p| convert::uuid(p.uuid))
            .collect())
    }

    fn players_name(&mut self, player: Uuid) -> Result<Option<String>, HostError> {
        Ok(self.player(player).ok().map(|p| p.name))
    }

    fn players_by_name(&mut self, name: &str) -> Result<Option<Uuid>, HostError> {
        Ok(self
            .players()
            .iter()
            .find(|p| p.name.eq_ignore_ascii_case(name))
            .map(|p| convert::uuid(p.uuid)))
    }

    fn players_is_online(&mut self, player: Uuid) -> Result<bool, HostError> {
        Ok(self.player(player).is_ok())
    }

    fn players_get_location(&mut self, player: Uuid) -> Result<Location, HostError> {
        Ok(convert::location(&self.player(player)?))
    }

    fn players_address(&mut self, player: Uuid) -> Result<String, HostError> {
        Ok(self.player(player)?.address)
    }

    fn players_ping(&mut self, player: Uuid) -> Result<u32, HostError> {
        Ok(self.player(player)?.latency.max(0) as u32)
    }

    fn players_locale(&mut self, player: Uuid) -> Result<String, HostError> {
        let raw = convert::raw(player);
        self.live(|h| h.player_client(raw).map(|c| c.language))?
            .ok_or(HostError::NoSuchPlayer)
    }

    fn players_send_message(&mut self, player: Uuid, message: &Text) -> Result<(), HostError> {
        let (raw, line) = (convert::raw(player), convert::to_horizon(message));
        self.write(move |h| {
            h.tell(raw, line);
        })
    }

    fn players_send_action_bar(&mut self, player: Uuid, message: &Text) -> Result<(), HostError> {
        let (raw, line) = (convert::raw(player), convert::to_horizon(message));
        self.write(move |h| {
            h.action_bar(raw, line);
        })
    }

    fn players_send_title(
        &mut self,
        player: Uuid,
        title: &Text,
        subtitle: &Text,
        fade_in: u32,
        stay: u32,
        fade_out: u32,
    ) -> Result<(), HostError> {
        let raw = convert::raw(player);
        let (title, subtitle) = (convert::to_horizon(title), convert::to_horizon(subtitle));
        let ticks = |t: u32| i32::try_from(t).unwrap_or(i32::MAX);
        let (i, s, o) = (ticks(fade_in), ticks(stay), ticks(fade_out));
        self.write(move |h| {
            h.title(raw, &title, &subtitle, i, s, o);
        })
    }

    fn players_play_sound(
        &mut self,
        player: Uuid,
        sound: &str,
        volume: f32,
        pitch: f32,
    ) -> Result<(), HostError> {
        let (raw, sound) = (convert::raw(player), sound.to_owned());
        self.write(move |h| {
            h.play_sound_to(raw, &sound, "master", None, volume, pitch);
        })
    }

    fn players_teleport(&mut self, player: Uuid, to: &Location) -> Result<(), HostError> {
        if !to.is_finite() {
            return Err(HostError::Invalid("a location that is not finite".into()));
        }
        let (raw, to) = (convert::raw(player), to.clone());
        self.write(move |h| {
            h.teleport_player(raw, &to.world, [to.x, to.y, to.z], to.yaw, to.pitch);
        })
    }

    fn players_get_game_mode(&mut self, player: Uuid) -> Result<GameMode, HostError> {
        Ok(convert::game_mode(self.player(player)?.game_mode))
    }

    fn players_set_game_mode(&mut self, player: Uuid, mode: GameMode) -> Result<(), HostError> {
        let raw = convert::raw(player);
        self.write(move |h| {
            h.set_game_mode(raw, convert::game_mode_id(mode));
        })
    }

    fn players_health(&mut self, player: Uuid) -> Result<f32, HostError> {
        Ok(self.player(player)?.health)
    }

    fn players_set_health(&mut self, player: Uuid, health: f32) -> Result<(), HostError> {
        let raw = convert::raw(player);
        self.write(move |h| {
            h.set_player_status(raw, Some(health), None, None);
        })
    }

    fn players_food_level(&mut self, player: Uuid) -> Result<u32, HostError> {
        let raw = convert::raw(player);
        let food = self.live(|h| h.player_food(raw))?;
        food.map(|(f, _, _)| f.max(0) as u32)
            .ok_or(HostError::NoSuchPlayer)
    }

    fn players_saturation(&mut self, player: Uuid) -> Result<f32, HostError> {
        let raw = convert::raw(player);
        let food = self.live(|h| h.player_food(raw))?;
        food.map(|(_, s, _)| s).ok_or(HostError::NoSuchPlayer)
    }

    fn players_set_food_level(&mut self, player: Uuid, food: u32) -> Result<(), HostError> {
        let raw = convert::raw(player);
        self.write(move |h| {
            if let Some((_, saturation, exhaustion)) = h.player_food(raw) {
                let food = i32::try_from(food).unwrap_or(20);
                h.set_player_status(raw, None, Some((food, saturation, exhaustion)), None);
            }
        })
    }

    fn players_experience_level(&mut self, player: Uuid) -> Result<u32, HostError> {
        Ok(self.player(player)?.xp_level.max(0) as u32)
    }

    fn players_max_health(&mut self, player: Uuid) -> Result<f32, HostError> {
        let raw = convert::raw(player);
        self.live(|h| h.attribute(raw, "minecraft:max_health"))?
            .map(|(_, value)| value as f32)
            .ok_or(HostError::NoSuchPlayer)
    }

    fn players_set_saturation(&mut self, player: Uuid, saturation: f32) -> Result<(), HostError> {
        let raw = convert::raw(player);
        self.write(move |h| {
            if let Some((food, _, exhaustion)) = h.player_food(raw) {
                h.set_player_status(
                    raw,
                    None,
                    Some((food, saturation.max(0.0), exhaustion)),
                    None,
                );
            }
        })
    }

    fn players_set_experience_level(&mut self, player: Uuid, level: u32) -> Result<(), HostError> {
        // `/experience set` keeps the progress and the total as vanilla's `setExperienceLevels`.
        let name = self.player(player)?.name;
        self.write(move |h| {
            h.run_command(&format!("experience set {name} {level} levels"));
        })
    }

    fn players_set_flying(
        &mut self,
        player: Uuid,
        allow: bool,
        flying: bool,
    ) -> Result<(), HostError> {
        let raw = convert::raw(player);
        self.write(move |h| {
            h.set_flight(raw, allow, flying);
        })
    }

    fn players_kick(&mut self, player: Uuid, reason: &Text) -> Result<(), HostError> {
        let (raw, reason) = (convert::raw(player), convert::to_horizon(reason));
        self.write(move |h| {
            h.kick(raw, &reason);
        })
    }

    fn players_is_op(&mut self, player: Uuid) -> Result<bool, HostError> {
        Ok(self.player(player)?.permission > 0)
    }

    fn players_set_op(&mut self, player: Uuid, op: bool) -> Result<(), HostError> {
        let name = self.player(player)?.name;
        let line = format!("{} {name}", if op { "op" } else { "deop" });
        self.write(move |h| {
            h.run_command(&line);
        })
    }

    // ---- inventories ----
    fn inventories_contents(&mut self, player: Uuid) -> Result<Vec<ItemStack>, HostError> {
        let raw = convert::raw(player);
        let inventory = self
            .live(|h| h.inventory(raw, NativeInventoryKind::Player))?
            .map_err(HostError::Other)?;
        Ok(inventory.slots.iter().map(|s| item(s.as_ref())).collect())
    }

    fn inventories_get_slot(&mut self, player: Uuid, slot: u32) -> Result<ItemStack, HostError> {
        let contents = self.inventories_contents(player)?;
        contents
            .get(slot as usize)
            .cloned()
            .ok_or_else(|| HostError::Invalid(format!("no slot {slot}")))
    }

    fn inventories_held_slot(&mut self, player: Uuid) -> Result<u32, HostError> {
        let raw = convert::raw(player);
        let inventory = self
            .live(|h| h.inventory(raw, NativeInventoryKind::Player))?
            .map_err(HostError::Other)?;
        Ok(inventory.selected as u32)
    }

    fn inventories_set_slot(
        &mut self,
        player: Uuid,
        slot: u32,
        stack: &ItemStack,
    ) -> Result<(), HostError> {
        let raw = convert::raw(player);
        let stack = stack.clone();
        self.live(move |h| {
            let new = convert::stack_to_horizon(h, &stack);
            if !stack.is_empty() && new.is_none() {
                return Err(HostError::Invalid(format!(
                    "not an item stack: {}",
                    stack.id
                )));
            }
            let current = h
                .inventory(raw, NativeInventoryKind::Player)
                .map_err(HostError::Other)?;
            let expected = current.slots.get(slot as usize).cloned().flatten();
            h.set_inventory_slot(
                raw,
                NativeInventoryKind::Player,
                slot as usize,
                expected.as_ref(),
                new,
            )
            .map(|_| ())
            .map_err(HostError::Other)
        })?
    }

    fn inventories_give(
        &mut self,
        player: Uuid,
        stack: &ItemStack,
    ) -> Result<ItemStack, HostError> {
        let raw = convert::raw(player);
        let stack = stack.clone();
        self.live(move |h| {
            let Some(given) = convert::stack_to_horizon(h, &stack) else {
                return Ok(ItemStack::empty());
            };
            Ok(convert::stack_to_core(h.give_stack(raw, given).as_ref()))
        })?
    }

    fn inventories_take(&mut self, player: Uuid, id: &str, count: u32) -> Result<u32, HostError> {
        let raw = convert::raw(player);
        self.live(move |h| {
            let current = h
                .inventory(raw, NativeInventoryKind::Player)
                .map_err(HostError::Other)?;
            let mut taken = 0;
            for (slot, left) in take_plan(&current.slots, id, count) {
                let before = current.slots[slot].clone();
                let had = before.as_ref().map_or(0, |s| s.count);
                let kept = left.as_ref().map_or(0, |s| s.count);
                if h.set_inventory_slot(
                    raw,
                    NativeInventoryKind::Player,
                    slot,
                    before.as_ref(),
                    left,
                )
                .is_ok()
                {
                    taken += had - kept;
                }
            }
            Ok(taken)
        })?
    }

    fn inventories_clear(&mut self, player: Uuid) -> Result<(), HostError> {
        let raw = convert::raw(player);
        self.live(move |h| {
            h.set_inventory(raw, NativeInventoryKind::Player, vec![None; 41], None)
                .map_err(HostError::Other)
        })?
    }

    fn inventories_close_container(&mut self, player: Uuid) -> Result<(), HostError> {
        let raw = convert::raw(player);
        self.write(move |h| {
            h.close_container(raw);
        })
    }

    // ---- worlds ----
    fn worlds_all(&mut self) -> Result<Vec<String>, HostError> {
        self.live(|h| {
            [
                "minecraft:overworld",
                "minecraft:the_nether",
                "minecraft:the_end",
            ]
            .into_iter()
            .filter(|d| h.level(d).is_some())
            .map(str::to_owned)
            .collect()
        })
    }

    fn worlds_get_block(&mut self, world: &str, pos: BlockPos) -> Result<BlockState, HostError> {
        let state = self.live(|h| {
            let level = h.level(world)?;
            h.level_block_state(level, [pos.x, pos.y, pos.z])
        })?;
        state
            .map(|s| convert::block_state(&s))
            .ok_or(HostError::NoSuchWorld)
    }

    fn worlds_set_block(
        &mut self,
        world: &str,
        pos: BlockPos,
        state: &BlockState,
        _physics: bool,
    ) -> Result<bool, HostError> {
        // `/setblock` through `execute in`, which enters the level: any loaded level, from any hook.
        let line = format!(
            "execute in {world} run setblock {} {} {} {}",
            pos.x,
            pos.y,
            pos.z,
            convert::block_state_string(state)
        );
        self.live(move |h| h.run_command(&line))
    }

    fn worlds_spawn(&mut self, world: &str) -> Result<Location, HostError> {
        let spawn = self.live(|h| h.world_spawn())?;
        let ([x, y, z], [yaw, pitch]) = spawn.ok_or(HostError::NoSuchWorld)?;
        Ok(Location {
            world: world.to_owned(),
            x: f64::from(x) + 0.5,
            y: f64::from(y),
            z: f64::from(z) + 0.5,
            yaw,
            pitch,
        })
    }

    fn worlds_set_spawn(&mut self, _world: &str, at: &Location) -> Result<(), HostError> {
        if !at.is_finite() {
            return Err(HostError::Invalid("a location that is not finite".into()));
        }
        let pos = [
            at.x.floor() as i32,
            at.y.floor() as i32,
            at.z.floor() as i32,
        ];
        let angle = [at.yaw, at.pitch];
        self.write(move |h| {
            h.set_world_spawn(pos, angle);
        })
    }

    fn worlds_players(&mut self, world: &str) -> Result<Vec<Uuid>, HostError> {
        Ok(self
            .players()
            .iter()
            .filter(|p| p.dimension == world)
            .map(|p| convert::uuid(p.uuid))
            .collect())
    }

    // ---- menus ----
    fn menus_open(&mut self, player: Uuid, spec: &MenuSpec) -> Result<MenuId, HostError> {
        let raw = convert::raw(player);
        let rows = spec.rows.clamp(1, 6);
        let title = convert::to_horizon(&spec.title);
        let spec = spec.clone();
        self.live(move |h| {
            let mut items = vec![None; usize::from(rows) * 9];
            for (slot, stack) in &spec.items {
                if let Some(cell) = items.get_mut(*slot as usize) {
                    *cell = convert::stack_to_horizon(h, stack);
                }
            }
            let window = h
                .open_menu(raw, rows, title.clone(), items.clone())
                .ok_or(HostError::NoSuchPlayer)?;
            Ok(ui::menu_opened(ui::Menu {
                player: raw,
                window,
                rows,
                title,
                items,
            }))
        })?
    }

    fn menus_set_item(
        &mut self,
        menu: MenuId,
        slot: u32,
        stack: &ItemStack,
    ) -> Result<(), HostError> {
        let stack = stack.clone();
        self.live(move |h| {
            let new = convert::stack_to_horizon(h, &stack);
            let menu = ui::update_menu(menu, |m| {
                if let Some(cell) = m.items.get_mut(slot as usize) {
                    *cell = new;
                }
            })
            .ok_or_else(|| HostError::Invalid(format!("menu {menu} is not open")))?;
            h.set_menu_items(menu.player, menu.items);
            Ok(())
        })?
    }

    fn menus_set_title(&mut self, menu: MenuId, title: &Text) -> Result<(), HostError> {
        // No packet renames a window: the menu opens again under the new title, with its items.
        let title = convert::to_horizon(title);
        self.live(move |h| {
            let current = ui::menu(menu)
                .ok_or_else(|| HostError::Invalid(format!("menu {menu} is not open")))?;
            let window = h
                .open_menu(current.player, current.rows, title.clone(), current.items)
                .ok_or(HostError::NoSuchPlayer)?;
            ui::update_menu(menu, |m| {
                m.window = window;
                m.title = title;
            });
            Ok(())
        })?
    }

    fn menus_close(&mut self, menu: MenuId) -> Result<(), HostError> {
        let Some(current) = ui::menu(menu) else {
            return Ok(());
        };
        // At the end of the tick, not inside the plugin's call: `menu_closed` follows and fires
        // `inventory-close` to the plugin, which the core could not deliver while it executes.
        self.world.deferred.push(Box::new(move |h: &mut dyn Host| {
            h.close_menu(current.player);
            ui::closed_by_rustic(menu, current.player);
        }));
        Ok(())
    }

    fn menus_open_menu(&mut self, player: Uuid) -> Result<Option<MenuId>, HostError> {
        Ok(ui::menu_of(convert::raw(player)))
    }

    // ---- worlds (more) ----
    fn worlds_set_blocks(
        &mut self,
        world: &str,
        blocks: &[(BlockPos, BlockState)],
        physics: bool,
    ) -> Result<u32, HostError> {
        let mut set = 0;
        for (pos, state) in blocks {
            if self.worlds_set_block(world, *pos, state, physics)? {
                set += 1;
            }
        }
        Ok(set)
    }

    fn worlds_fill(
        &mut self,
        world: &str,
        from: BlockPos,
        to: BlockPos,
        state: &BlockState,
    ) -> Result<u32, HostError> {
        let line = format!(
            "execute in {world} run fill {} {} {} {} {} {} {}",
            from.x,
            from.y,
            from.z,
            to.x,
            to.y,
            to.z,
            convert::block_state_string(state)
        );
        let result = self.live(move |h| h.run_command_captured(&line))?;
        if !result.ran {
            return Err(HostError::Invalid(answer(&result)));
        }
        Ok(result.result.max(0) as u32)
    }

    fn worlds_highest_block_y(&mut self, world: &str, x: i32, z: i32) -> Result<i32, HostError> {
        // From the top down over the loaded column (any level, `level_block_state`).
        self.live(|h| {
            let level = level(h, world)?;
            let mut y = 319;
            while y >= -64 {
                match h.level_block_state(level, [x, y, z]) {
                    None => return Err(HostError::Invalid(format!("chunk of {x} {z} not loaded"))),
                    Some(state)
                        if !state.starts_with("minecraft:air")
                            && !state.starts_with("minecraft:cave_air")
                            && !state.starts_with("minecraft:void_air") =>
                    {
                        return Ok(y);
                    }
                    Some(_) => y -= 1,
                }
            }
            Ok(-64)
        })?
    }

    fn worlds_is_chunk_loaded(&mut self, world: &str, cx: i32, cz: i32) -> Result<bool, HostError> {
        self.live(|h| h.chunk_is_loaded(world, [cx, cz]))
    }

    fn worlds_keep_chunk_loaded(
        &mut self,
        world: &str,
        cx: i32,
        cz: i32,
        keep: bool,
    ) -> Result<(), HostError> {
        let world = world.to_owned();
        self.write(move |h| {
            h.force_chunk_in("rustic", &world, [cx, cz], keep);
        })
    }

    fn worlds_time(&mut self, world: &str) -> Result<u64, HostError> {
        self.live(|h| {
            let level = level(h, world)?;
            h.level_day_time(level)
                .map(|t| t.max(0) as u64)
                .ok_or(HostError::NoSuchWorld)
        })?
    }

    fn worlds_set_time(&mut self, world: &str, time: u64) -> Result<(), HostError> {
        self.live(|h| {
            let level = level(h, world)?;
            let time =
                i64::try_from(time).map_err(|_| HostError::Invalid("time too large".into()))?;
            h.level_set_day_time(level, time);
            Ok(())
        })?
    }

    fn worlds_get_weather(&mut self, world: &str) -> Result<Weather, HostError> {
        self.live(|h| {
            let level = level(h, world)?;
            Ok(
                match (h.level_is_raining(level), h.level_is_thundering(level)) {
                    (_, Some(true)) => Weather::Thunder,
                    (Some(true), _) => Weather::Rain,
                    _ => Weather::Clear,
                },
            )
        })?
    }

    fn worlds_set_weather(
        &mut self,
        world: &str,
        weather: Weather,
        duration_ticks: u32,
    ) -> Result<(), HostError> {
        self.live(|h| {
            let level = level(h, world)?;
            let ticks = i32::try_from(duration_ticks).unwrap_or(i32::MAX);
            let (clear, time, rain, thunder) = match weather {
                Weather::Clear => (ticks, 0, false, false),
                Weather::Rain => (0, ticks, true, false),
                Weather::Thunder => (0, ticks, true, true),
            };
            h.level_set_weather(level, clear, time, rain, thunder);
            Ok(())
        })?
    }

    fn worlds_spawn_entity(
        &mut self,
        world: &str,
        kind: &str,
        at: Vec3,
        nbt: &str,
    ) -> Result<EntityRef, HostError> {
        let compound = if nbt.trim().is_empty() {
            horizon_world::nbt::Compound::default()
        } else {
            match horizon_world::nbt::parse_snbt(nbt) {
                Ok((horizon_world::nbt::Tag::Compound(c), _)) => c,
                _ => return Err(HostError::Invalid("nbt is not an SNBT compound".into())),
            }
        };
        self.live(|h| {
            let id = h
                .summon(world, kind, [at.x, at.y, at.z], compound)
                .ok_or_else(|| HostError::Invalid(format!("cannot summon {kind} in {world}")))?;
            let uuid = h.entity(id).and_then(|e| h.entity_uuid(e)).unwrap_or(0);
            Ok(EntityRef {
                id,
                uuid: convert::uuid(uuid),
                kind: kind.to_owned(),
            })
        })?
    }

    fn worlds_remove_entity(&mut self, world: &str, entity: u32) -> Result<(), HostError> {
        let world = world.to_owned();
        self.write(move |h| {
            h.remove_entity(&world, entity);
        })
    }

    fn worlds_entities_near(
        &mut self,
        world: &str,
        at: Vec3,
        radius: f64,
    ) -> Result<Vec<EntityRef>, HostError> {
        if !(radius.is_finite() && radius >= 0.0) {
            return Err(HostError::Invalid("radius".into()));
        }
        self.live(|h| {
            let min = [at.x - radius, at.y - radius, at.z - radius];
            let max = [at.x + radius, at.y + radius, at.z + radius];
            let ids = h.entities_in(world, min, max);
            ids.into_iter()
                .filter_map(|id| {
                    let e = h.entity(id)?;
                    let [x, y, z] = h.entity_position(e)?;
                    let d2 = (x - at.x).powi(2) + (y - at.y).powi(2) + (z - at.z).powi(2);
                    (d2 <= radius * radius).then_some(())?;
                    let kind = h.entity_kind(e)?;
                    let uuid = h.entity_uuid(e).unwrap_or(0);
                    Some(EntityRef {
                        id,
                        uuid: convert::uuid(uuid),
                        kind,
                    })
                })
                .collect()
        })
    }

    fn worlds_entity_location(&mut self, world: &str, entity: u32) -> Result<Location, HostError> {
        self.live(|h| {
            let e = h
                .entity(entity)
                .ok_or(HostError::Invalid(format!("no entity {entity}")))?;
            let [x, y, z] = h
                .entity_position(e)
                .ok_or(HostError::Invalid(format!("no entity {entity}")))?;
            let [yaw, pitch] = h.entity_rotation(e).unwrap_or([0.0, 0.0]);
            Ok(Location {
                world: world.to_owned(),
                x,
                y,
                z,
                yaw,
                pitch,
            })
        })?
    }

    #[allow(clippy::too_many_arguments)]
    fn worlds_spawn_particle(
        &mut self,
        world: &str,
        particle: &str,
        options: &str,
        at: Vec3,
        count: u32,
        offset: Vec3,
        speed: f64,
    ) -> Result<(), HostError> {
        let line = format!(
            "execute in {world} run particle {particle}{options} {} {} {} {} {} {} {speed} {count} force",
            at.x, at.y, at.z, offset.x, offset.y, offset.z
        );
        self.write(move |h| {
            h.run_command(&line);
        })
    }

    fn worlds_play_sound(
        &mut self,
        world: &str,
        sound: &str,
        at: Vec3,
        volume: f32,
        pitch: f32,
    ) -> Result<(), HostError> {
        let (world, sound) = (world.to_owned(), sound.to_owned());
        self.write(move |h| {
            if let Some(level) = h.level(&world) {
                // SoundSource 0: master.
                h.level_play_sound(level, &sound, [at.x, at.y, at.z], 0, volume, pitch);
            }
        })
    }

    // ---- scoreboards (vanilla's scoreboard, through its commands) ----
    fn scoreboards_create_objective(
        &mut self,
        name: &str,
        display: &Text,
        criteria: &str,
    ) -> Result<(), HostError> {
        let line = format!(
            "scoreboard objectives add {} {} {}",
            word(name)?,
            word(criteria)?,
            component_arg(display)
        );
        self.command(line)
    }

    fn scoreboards_remove_objective(&mut self, name: &str) -> Result<(), HostError> {
        let line = format!("scoreboard objectives remove {}", word(name)?);
        self.command(line)
    }

    fn scoreboards_set_display(
        &mut self,
        slot: DisplaySlot,
        objective: Option<&str>,
    ) -> Result<(), HostError> {
        let slot = match slot {
            DisplaySlot::Sidebar => "sidebar",
            DisplaySlot::PlayerList => "list",
            DisplaySlot::BelowName => "below_name",
        };
        let line = match objective {
            Some(o) => format!("scoreboard objectives setdisplay {slot} {}", word(o)?),
            None => format!("scoreboard objectives setdisplay {slot}"),
        };
        self.command(line)
    }

    fn scoreboards_set_objective_display(
        &mut self,
        name: &str,
        display: &Text,
    ) -> Result<(), HostError> {
        let line = format!(
            "scoreboard objectives modify {} displayname {}",
            word(name)?,
            component_arg(display)
        );
        self.command(line)
    }

    fn scoreboards_set_score(
        &mut self,
        objective: &str,
        entry: &str,
        score: i32,
    ) -> Result<(), HostError> {
        let line = format!(
            "scoreboard players set {} {} {score}",
            word(entry)?,
            word(objective)?
        );
        self.command(line)
    }

    fn scoreboards_reset_score(&mut self, objective: &str, entry: &str) -> Result<(), HostError> {
        let line = format!(
            "scoreboard players reset {} {}",
            word(entry)?,
            word(objective)?
        );
        self.command(line)
    }

    fn scoreboards_get_score(
        &mut self,
        objective: &str,
        entry: &str,
    ) -> Result<Option<i32>, HostError> {
        let line = format!(
            "scoreboard players get {} {}",
            word(entry)?,
            word(objective)?
        );
        let result = self.live(move |h| h.run_command_captured(&line))?;
        Ok(result.ran.then_some(result.result))
    }

    fn scoreboards_create_team(&mut self, name: &str) -> Result<(), HostError> {
        let line = format!("team add {}", word(name)?);
        self.command(line)
    }

    fn scoreboards_remove_team(&mut self, name: &str) -> Result<(), HostError> {
        let line = format!("team remove {}", word(name)?);
        self.command(line)
    }

    fn scoreboards_set_team_prefix(&mut self, name: &str, prefix: &Text) -> Result<(), HostError> {
        let line = format!(
            "team modify {} prefix {}",
            word(name)?,
            component_arg(prefix)
        );
        self.command(line)
    }

    fn scoreboards_set_team_suffix(&mut self, name: &str, suffix: &Text) -> Result<(), HostError> {
        let line = format!(
            "team modify {} suffix {}",
            word(name)?,
            component_arg(suffix)
        );
        self.command(line)
    }

    fn scoreboards_set_team_color(
        &mut self,
        name: &str,
        color: TeamColor,
    ) -> Result<(), HostError> {
        let line = format!("team modify {} color {}", word(name)?, team_color(color));
        self.command(line)
    }

    fn scoreboards_team_add(&mut self, name: &str, entry: &str) -> Result<(), HostError> {
        let line = format!("team join {} {}", word(name)?, word(entry)?);
        self.command(line)
    }

    fn scoreboards_team_remove(&mut self, name: &str, entry: &str) -> Result<(), HostError> {
        let _ = word(name)?;
        let line = format!("team leave {}", word(entry)?);
        self.command(line)
    }

    // ---- boss bars (Horizon's per-viewer `boss_bar`) ----
    fn bossbars_create(
        &mut self,
        title: &Text,
        color: BarColor,
        style: BarStyle,
        progress: f32,
    ) -> Result<BarId, HostError> {
        Ok(ui::bar_created(NativeBossBar {
            name: convert::to_horizon(title),
            progress: progress.clamp(0.0, 1.0),
            color: bar_color(color),
            overlay: bar_style(style),
            ..NativeBossBar::default()
        }))
    }

    fn bossbars_set_title(&mut self, bar: BarId, title: &Text) -> Result<(), HostError> {
        let name = convert::to_horizon(title);
        self.bar_changed(bar, move |b| b.bar.name = name)
    }

    fn bossbars_set_progress(&mut self, bar: BarId, progress: f32) -> Result<(), HostError> {
        self.bar_changed(bar, move |b| b.bar.progress = progress.clamp(0.0, 1.0))
    }

    fn bossbars_set_color(&mut self, bar: BarId, color: BarColor) -> Result<(), HostError> {
        self.bar_changed(bar, move |b| b.bar.color = bar_color(color))
    }

    fn bossbars_show(&mut self, bar: BarId, player: Uuid) -> Result<(), HostError> {
        let raw = convert::raw(player);
        let shown = ui::update_bar(bar, |b| {
            b.viewers.insert(raw);
        })
        .ok_or_else(|| HostError::Invalid(format!("no boss bar {bar}")))?;
        self.write(move |h| {
            h.boss_bar(raw, shown.id, Some(&shown.bar));
        })
    }

    fn bossbars_hide(&mut self, bar: BarId, player: Uuid) -> Result<(), HostError> {
        let raw = convert::raw(player);
        let Some(hidden) = ui::update_bar(bar, |b| {
            b.viewers.remove(&raw);
        }) else {
            return Ok(());
        };
        self.write(move |h| {
            h.boss_bar(raw, hidden.id, None);
        })
    }

    fn bossbars_remove(&mut self, bar: BarId) -> Result<(), HostError> {
        let Some(gone) = ui::remove_bar(bar) else {
            return Ok(());
        };
        self.write(move |h| {
            for viewer in &gone.viewers {
                h.boss_bar(*viewer, gone.id, None);
            }
        })
    }

    // ---- messaging ----
    fn messaging_send(
        &mut self,
        player: Uuid,
        channel: &str,
        data: &[u8],
    ) -> Result<(), HostError> {
        let raw = convert::raw(player);
        let (channel, data) = (channel.to_owned(), data.to_vec());
        self.live(move |h| {
            h.send_custom_payload_to_player(raw, &channel, &data)
                .map_err(HostError::Other)
        })?
    }

    fn messaging_register_incoming(&mut self, _channel: &str) -> Result<(), HostError> {
        // The core routes `plugin-message` by the channels plugins registered; Horizon hands every
        // client payload to `custom_payload_from_client`.
        Ok(())
    }

    fn messaging_unregister_incoming(&mut self, _channel: &str) -> Result<(), HostError> {
        Ok(())
    }

    fn messaging_behind_proxy(&mut self) -> Result<bool, HostError> {
        // Velocity forwarding: `forwarding.secret` in the server folder (docs/velocity-forwarding.md).
        let dir = self.live(|h| h.server_dir())?;
        Ok(dir.is_some_and(|d| d.join("forwarding.secret").is_file()))
    }

    fn server_operators(&mut self) -> Result<Vec<Uuid>, HostError> {
        let dir = self
            .live(|h| h.server_dir())?
            .ok_or(HostError::Unsupported("no server folder".into()))?;
        let text = std::fs::read_to_string(dir.join("ops.json")).unwrap_or_default();
        let list: Vec<serde_json::Value> = serde_json::from_str(&text).unwrap_or_default();
        Ok(list
            .iter()
            .filter_map(|e| e.get("uuid")?.as_str()?.parse().ok())
            .collect())
    }

    // ---- tablist ----
    fn tablist_set_display_name(
        &mut self,
        player: Uuid,
        name: Option<&Text>,
    ) -> Result<(), HostError> {
        let raw = convert::raw(player);
        let name = name.map(convert::to_horizon);
        self.write(move |h| {
            h.set_list_name(raw, name.as_ref());
        })
    }

    fn tablist_set_header_footer(
        &mut self,
        player: Uuid,
        header: &Text,
        footer: &Text,
    ) -> Result<(), HostError> {
        let raw = convert::raw(player);
        let (header, footer) = (convert::to_horizon(header), convert::to_horizon(footer));
        self.write(move |h| {
            h.tab_list(raw, &header, &footer);
        })
    }

    // ---- bookkeeping ----
    fn commands_changed(&mut self) {
        self.world.commands_changed = true;
        if let Some(host) = self.host_mut() {
            host.refresh_commands();
            self.world.commands_changed = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use horizon_client::mod_api::Text as HorizonText;

    use super::*;

    /// A host that only counts `server_players()` reads (every other `Host` method panics).
    #[derive(Default)]
    struct Counting {
        reads: Cell<u32>,
        told: u32,
    }

    impl Host for Counting {
        fn server_players(&self) -> Vec<NativeServerPlayer> {
            self.reads.set(self.reads.get() + 1);
            Vec::new()
        }

        fn tell(&mut self, _uuid: u128, _line: HorizonText) -> bool {
            self.told += 1;
            false
        }
    }

    #[test]
    fn player_reads_share_one_server_players_until_a_host_call() {
        let mut host = Counting::default();
        let mut world = World::default();
        let who = convert::uuid(7);
        {
            let mut backend = HorizonBackend::new(Some(&mut host), &mut world);
            for _ in 0..50 {
                assert!(matches!(
                    backend.players_get_location(who),
                    Err(HostError::NoSuchPlayer)
                ));
            }
            assert!(!backend.players_is_online(who).unwrap());
            assert!(backend.players_online().unwrap().is_empty());
            // A call that may change the game drops the cache.
            backend
                .players_send_message(who, &Text::plain("hi"))
                .unwrap();
            assert!(backend.players_get_location(who).is_err());
        }
        assert_eq!(host.reads.get(), 2);
        assert_eq!(host.told, 1);
    }
}
