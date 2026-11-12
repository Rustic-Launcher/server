//! The shared core of a [`crate::MockServer`]: the runtime, the in-memory game, the
//! [`Backend`] over it, and the game rules that turn simulated player actions into events and
//! apply their outcome the way the real server does.

use crate::logcap::Sink;
use crate::state::*;
use rusticmc_core::clock::FakeClock;
use rusticmc_core::lifecycle::{LoadRequest, PluginInstance, PluginLoader};
use rusticmc_core::model::*;
use rusticmc_core::native::NativeLoader;
use rusticmc_core::{Backend, CommandOutcome, PluginId, Rustic};
use std::cell::{Cell, RefCell, RefMut};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

/// How async tasks (`run-async*`) run in the harness.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AsyncMode {
    /// Deterministic (the default): due async tasks run inline, on the test thread, right after
    /// each [`crate::MockServer::tick`], in task order.
    #[default]
    Inline,
    /// Async tasks only run when the test calls [`crate::MockServer::run_async`].
    Manual,
}

/// What happened to a command line.
#[derive(Clone, Debug, PartialEq)]
pub enum CommandRun {
    /// A `player-command` / `server-command` handler cancelled it: nothing ran.
    Cancelled,
    /// A Rustic command (a plugin's or `/rustic`) handled it.
    Plugin(CommandOutcome),
    /// One of the mock's vanilla commands ran (`say`, `give`, `tp`...). `false`: it refused
    /// (no permission, bad arguments); the sender was told why.
    Vanilla(bool),
    /// Nobody knows this command; the sender got "Unknown or incomplete command".
    Unknown,
}

impl CommandRun {
    /// Whether the command ran successfully.
    pub fn executed(&self) -> bool {
        matches!(
            self,
            CommandRun::Plugin(CommandOutcome::Executed) | CommandRun::Vanilla(true)
        )
    }
}

/// Picks the native or the WebAssembly loader by plugin id.
pub(crate) struct HarnessLoader {
    pub native: NativeLoader,
    pub native_ids: Rc<RefCell<BTreeSet<String>>>,
    pub wasm: Option<Box<dyn PluginLoader>>,
}

impl PluginLoader for HarnessLoader {
    fn load(&mut self, req: &LoadRequest<'_>) -> Result<Box<dyn PluginInstance>, String> {
        let id = req.package.manifest.id.to_string();
        if self.native_ids.borrow().contains(&id) {
            return self.native.load(req);
        }
        match &mut self.wasm {
            Some(w) => w.load(req),
            None => Err(format!(
                "`{id}` is not a native plugin and WebAssembly support is off \
                 (enable the `wasm` feature of rusticmc-test)"
            )),
        }
    }

    fn unloaded(&mut self, plugin: &PluginId) {
        self.native.unloaded(plugin);
        if let Some(w) = &mut self.wasm {
            w.unloaded(plugin);
        }
    }
}

/// The WebAssembly side of the harness (the lazily created host and its async pool).
pub(crate) trait WasmSide {
    /// Whether a WebAssembly plugin was loaded (the host and its async pool exist).
    fn active(&self) -> bool;
    /// Wait until the host's async pool is idle; `became_due`: an async task became due during
    /// the tick. Returns how many async tasks ran meanwhile.
    fn wait_idle(&self, h: &Harness, became_due: bool) -> usize;
}

/// Shared between the server and its player handles.
pub(crate) struct Harness {
    pub rt: Rustic,
    pub state: RefCell<GameState>,
    pub clock: Arc<FakeClock>,
    pub native: NativeLoader,
    pub native_ids: Rc<RefCell<BTreeSet<String>>>,
    pub wasm: Option<Rc<dyn WasmSide>>,
    pub async_mode: Cell<AsyncMode>,
    pub depth: Cell<u32>,
    pub dir: PathBuf,
    pub log: Sink,
    pub _tmp: Option<tempfile::TempDir>,
}

/// Offline-mode UUID of a name (as the vanilla server computes it: MD5 of
/// `"OfflinePlayer:" + name`, version 3).
pub fn offline_uuid(name: &str) -> Uuid {
    use md5::{Digest, Md5};
    let mut h: [u8; 16] = Md5::digest(format!("OfflinePlayer:{name}").as_bytes()).into();
    h[6] = (h[6] & 0x0f) | 0x30;
    h[8] = (h[8] & 0x3f) | 0x80;
    Uuid::from_u128(u128::from_be_bytes(h))
}

fn vec3(l: &Location) -> Vec3 {
    l.vec3()
}

fn center(pos: BlockPos) -> Vec3 {
    Vec3 {
        x: f64::from(pos.x) + 0.5,
        y: f64::from(pos.y) + 0.5,
        z: f64::from(pos.z) + 0.5,
    }
}

impl Harness {
    pub fn st(&self) -> RefMut<'_, GameState> {
        self.state.borrow_mut()
    }

    pub fn backend(&self) -> MockBackend<'_> {
        MockBackend { h: self }
    }

    /// Run a harness operation; events queued by plugins' own calls are fired when the
    /// outermost operation returns.
    pub fn op<R>(&self, f: impl FnOnce() -> R) -> R {
        self.depth.set(self.depth.get() + 1);
        struct Down<'a>(&'a Cell<u32>);
        impl Drop for Down<'_> {
            fn drop(&mut self) {
                self.0.set(self.0.get().saturating_sub(1));
            }
        }
        let r = {
            let _down = Down(&self.depth);
            f()
        };
        if self.depth.get() == 0 {
            self.flush_pending();
        }
        r
    }

    fn flush_pending(&self) {
        self.depth.set(1);
        for _ in 0..10_000 {
            let next = self.st().pending.pop_front();
            let Some(p) = next else { break };
            match p {
                Pending::Death { player, cause } => self.die(player, &cause),
                Pending::Kicked { player } => self.quit(player),
                Pending::MenuClosed { player, menu } => {
                    self.fire(InventoryClose { player, menu });
                }
            }
        }
        self.depth.set(0);
    }

    pub fn fire<E: EventRecord>(&self, event: E) -> E {
        self.rt.fire_record(event, &mut self.backend())
    }

    pub fn player_state(&self, uuid: Uuid) -> Option<PlayerState> {
        self.state.borrow().players.get(&uuid).cloned()
    }

    fn online(&self, uuid: Uuid) -> Option<PlayerState> {
        self.state
            .borrow()
            .players
            .get(&uuid)
            .filter(|p| p.online)
            .cloned()
    }

    pub fn tell(&self, uuid: Uuid, message: Text) {
        if let Some(p) = self.st().online_player_mut(uuid) {
            p.received.push(Received::Message(message));
        }
    }

    pub fn tell_sender(&self, sender: CommandSender, message: Text) {
        match sender {
            CommandSender::Console => self.st().console.push(message.to_plain()),
            CommandSender::Player(u) => self.tell(u, message),
        }
    }

    // ------------------------------------------------------------------ join / quit

    /// A player connects. `Err(reason)` when refused (pre-login cancelled, server full).
    pub fn join(&self, name: &str, address: &str) -> Result<Uuid, Text> {
        self.op(|| {
            let existing = {
                let st = self.state.borrow();
                st.players
                    .values()
                    .find(|p| p.name.eq_ignore_ascii_case(name))
                    .map(|p| (p.uuid, p.online))
            };
            if let Some((_, true)) = existing {
                return Err(Text::plain(format!("{name} is already online")));
            }
            let uuid = existing.map(|e| e.0).unwrap_or_else(|| offline_uuid(name));
            let pre = self.fire(PlayerPreLogin {
                player: uuid,
                name: name.to_owned(),
                address: address.to_owned(),
                cancelled: false,
                kick_message: None,
            });
            if pre.cancelled {
                return Err(pre
                    .kick_message
                    .unwrap_or_else(|| Text::plain("You are not allowed to join this server.")));
            }
            {
                let mut st = self.st();
                if st.online.len() as u32 >= st.max_players && !st.ops.contains(&uuid) {
                    return Err(Text::plain("The server is full!"));
                }
                let entity_id = st.next_entity_id();
                let spawn = st.worlds[OVERWORLD].spawn.clone();
                let p = st.players.entry(uuid).or_insert_with(|| PlayerState {
                    uuid,
                    name: name.to_owned(),
                    online: false,
                    entity_id,
                    location: spawn,
                    game_mode: GameMode::Survival,
                    health: 20.0,
                    max_health: 20.0,
                    food: 20,
                    saturation: 5.0,
                    level: 0,
                    allow_flight: false,
                    flying: false,
                    address: address.to_owned(),
                    ping: 0,
                    locale: "en_us".into(),
                    inventory: Inventory::default(),
                    held_slot: 0,
                    cursor: ItemStack::empty(),
                    dead: false,
                    open_menu: None,
                    received: Vec::new(),
                    read_messages: 0,
                    kicked: None,
                    plugin_messages: Vec::new(),
                    tab_header: None,
                    tab_footer: None,
                    display_name: None,
                });
                p.online = true;
                p.entity_id = entity_id;
                p.address = address.to_owned();
                p.kicked = None;
                st.online.push(uuid);
            }
            let join = self.fire(PlayerJoin {
                player: uuid,
                name: name.to_owned(),
                join_message: Some(Text::colored(format!("{name} joined the game"), "yellow")),
            });
            if let Some(m) = join.join_message {
                self.st().broadcast(&m);
            }
            Ok(uuid)
        })
    }

    /// A player disconnects (or was kicked): close their menu, fire `player-quit`, broadcast the
    /// quit line.
    pub fn quit(&self, uuid: Uuid) {
        self.op(|| {
            let Some(p) = self.online(uuid) else { return };
            if let Some(m) = self.st().close_menu_of(uuid) {
                self.fire(InventoryClose {
                    player: uuid,
                    menu: Some(m),
                });
            }
            let quit = self.fire(PlayerQuit {
                player: uuid,
                name: p.name.clone(),
                quit_message: Some(Text::colored(format!("{} left the game", p.name), "yellow")),
            });
            let mut st = self.st();
            st.online.retain(|u| *u != uuid);
            if let Some(ps) = st.players.get_mut(&uuid) {
                ps.online = false;
                ps.open_menu = None;
            }
            for bar in st.bars.values_mut() {
                bar.viewers.remove(&uuid);
            }
            if let Some(m) = quit.quit_message {
                st.broadcast(&m);
            }
        })
    }

    // ------------------------------------------------------------------ chat and commands

    /// Chat: returns whether the message was broadcast.
    pub fn chat(&self, uuid: Uuid, message: &str) -> bool {
        self.op(|| {
            let Some(p) = self.online(uuid) else {
                return false;
            };
            let e = self.fire(PlayerChat {
                player: uuid,
                name: p.name.clone(),
                message: message.to_owned(),
                cancelled: false,
                display: None,
            });
            if e.cancelled {
                return false;
            }
            let line = e
                .display
                .unwrap_or_else(|| Text::plain(format!("<{}> {message}", p.name)));
            self.st().broadcast(&line);
            true
        })
    }

    /// Run a command line as `sender`, as typed (fires `player-command` / `server-command`).
    pub fn command(&self, sender: CommandSender, line: &str) -> CommandRun {
        self.op(|| {
            let stripped = line.trim().trim_start_matches('/').to_owned();
            match sender {
                CommandSender::Player(u) => {
                    let Some(p) = self.online(u) else {
                        return CommandRun::Unknown;
                    };
                    let e = self.fire(PlayerCommand {
                        player: u,
                        name: p.name,
                        command: stripped.clone(),
                        cancelled: false,
                    });
                    if e.cancelled {
                        return CommandRun::Cancelled;
                    }
                }
                CommandSender::Console => {
                    self.st().console_commands.push(stripped.clone());
                    let e = self.fire(ServerCommand {
                        command: stripped.clone(),
                        cancelled: false,
                    });
                    if e.cancelled {
                        return CommandRun::Cancelled;
                    }
                }
            }
            self.run_command(sender, &stripped)
        })
    }

    /// Run a command without the typed-command events (what `server.dispatch-command` does).
    pub fn run_command(&self, sender: CommandSender, line: &str) -> CommandRun {
        match self.rt.dispatch_command(sender, line, &mut self.backend()) {
            CommandOutcome::UnknownCommand => match crate::vanilla::run(self, sender, line) {
                Some(ok) => CommandRun::Vanilla(ok),
                None => {
                    self.tell_sender(
                        sender,
                        Text::error(format!(
                            "Unknown or incomplete command, see below for error\n{line}<--[HERE]"
                        )),
                    );
                    CommandRun::Unknown
                }
            },
            other => CommandRun::Plugin(other),
        }
    }

    // ------------------------------------------------------------------ movement

    /// A move packet: returns whether the player moved (false: a handler put them back).
    pub fn move_to(&self, uuid: Uuid, to: Location) -> bool {
        self.op(|| {
            let Some(p) = self.online(uuid) else {
                return false;
            };
            if p.location.world != to.world {
                self.teleport(uuid, to);
                return true;
            }
            let from = p.location.clone();
            let (dx, dy, dz) = (to.x - from.x, to.y - from.y, to.z - from.z);
            let dist = (dx * dx + dy * dy + dz * dz).sqrt();
            if dist <= self.rt.config().events.move_threshold {
                // Below the threshold (or a pure rotation): no event, as on the real server.
                if let Some(ps) = self.st().online_player_mut(uuid) {
                    ps.location = to;
                }
                return true;
            }
            let e = self.fire(PlayerMove {
                player: uuid,
                from: from.clone(),
                to: to.clone(),
                cancelled: false,
            });
            let moved = !e.cancelled;
            if let Some(ps) = self.st().online_player_mut(uuid) {
                ps.location = if moved { to } else { from };
            }
            moved
        })
    }

    /// Teleport (a plugin, a command): applies, then fires `player-teleport` and, across
    /// worlds, `player-change-world`.
    pub fn teleport(&self, uuid: Uuid, to: Location) -> bool {
        self.op(|| {
            let from = {
                let mut st = self.st();
                if !st.worlds.contains_key(&to.world) {
                    return false;
                }
                let Some(p) = st.online_player_mut(uuid) else {
                    return false;
                };
                std::mem::replace(&mut p.location, to.clone())
            };
            self.fire(PlayerTeleport {
                player: uuid,
                from: from.clone(),
                to: to.clone(),
            });
            if from.world != to.world {
                self.fire(PlayerChangeWorld {
                    player: uuid,
                    from_world: from.world,
                    to_world: to.world,
                });
            }
            true
        })
    }

    /// Change game mode (fires the cancellable event): returns whether it changed.
    pub fn set_game_mode(&self, uuid: Uuid, mode: GameMode) -> bool {
        self.op(|| {
            let Some(p) = self.online(uuid) else {
                return false;
            };
            if p.game_mode == mode {
                return false;
            }
            let e = self.fire(PlayerGameModeChange {
                player: uuid,
                old_mode: p.game_mode,
                new_mode: mode,
                cancelled: false,
            });
            if e.cancelled {
                return false;
            }
            if let Some(ps) = self.st().online_player_mut(uuid) {
                ps.game_mode = mode;
                match mode {
                    GameMode::Creative => ps.allow_flight = true,
                    GameMode::Spectator => {
                        ps.allow_flight = true;
                        ps.flying = true;
                    }
                    GameMode::Survival | GameMode::Adventure => {
                        ps.allow_flight = false;
                        ps.flying = false;
                    }
                }
            }
            true
        })
    }

    // ------------------------------------------------------------------ blocks

    /// Break a block in the player's world: returns whether it was broken.
    pub fn break_block(&self, uuid: Uuid, pos: BlockPos) -> bool {
        self.op(|| {
            let Some(p) = self.online(uuid) else {
                return false;
            };
            if p.dead || matches!(p.game_mode, GameMode::Adventure | GameMode::Spectator) {
                return false;
            }
            let world = p.location.world.clone();
            let block = match self.state.borrow().worlds.get(&world) {
                Some(w) => w.block(pos),
                None => return false,
            };
            if is_air(&block)
                || (p.game_mode == GameMode::Survival && block.id == "minecraft:bedrock")
            {
                return false;
            }
            let e = self.fire(BlockBreak {
                player: uuid,
                world: world.clone(),
                pos,
                block: block.clone(),
                cancelled: false,
                drop_items: p.game_mode == GameMode::Survival,
            });
            if e.cancelled {
                return false;
            }
            let mut st = self.st();
            if let Some(w) = st.worlds.get_mut(&world) {
                w.set(pos, &air());
            }
            if e.drop_items {
                st.drop_item(&world, center(pos), ItemStack::new(block.id, 1));
            }
            true
        })
    }

    /// Place a block from the player's hand: returns whether it was placed. In survival and
    /// adventure the player must hold the block's item (one is used up); creative places
    /// anything.
    pub fn place_block(&self, uuid: Uuid, pos: BlockPos, block: &BlockState) -> bool {
        self.op(|| {
            let block = normalize_block(block);
            let Some(p) = self.online(uuid) else {
                return false;
            };
            if p.dead || p.game_mode == GameMode::Spectator {
                return false;
            }
            let world = p.location.world.clone();
            match self.state.borrow().worlds.get(&world) {
                Some(w) if w.in_height(pos.y) && is_air(&w.block(pos)) => {}
                _ => return false,
            }
            let main = p.held_slot as usize;
            let holds = |slot: usize| {
                let s = &p.inventory.slots[slot];
                !s.is_empty() && s.id == block.id
            };
            let (hand, slot) = if holds(main) {
                (Hand::MainHand, Some(main))
            } else if holds(40) {
                (Hand::OffHand, Some(40))
            } else {
                (Hand::MainHand, None)
            };
            if slot.is_none() && p.game_mode != GameMode::Creative {
                return false;
            }
            let e = self.fire(BlockPlace {
                player: uuid,
                world: world.clone(),
                pos,
                block: block.clone(),
                hand,
                cancelled: false,
            });
            if e.cancelled {
                return false;
            }
            let mut st = self.st();
            if let Some(w) = st.worlds.get_mut(&world) {
                w.set(pos, &block);
            }
            if p.game_mode != GameMode::Creative
                && let Some(slot) = slot
                && let Some(ps) = st.online_player_mut(uuid)
            {
                let s = &mut ps.inventory.slots[slot];
                s.count -= 1;
                if s.count == 0 {
                    *s = ItemStack::empty();
                }
            }
            true
        })
    }

    /// Click a block or the air: returns whether the interaction went ahead (not cancelled).
    pub fn interact(
        &self,
        uuid: Uuid,
        action: InteractAction,
        block: Option<BlockPos>,
        face: Option<BlockFace>,
        hand: Hand,
    ) -> bool {
        self.op(|| {
            let Some(p) = self.online(uuid) else {
                return false;
            };
            let slot = match hand {
                Hand::MainHand => p.held_slot as usize,
                Hand::OffHand => 40,
            };
            let e = self.fire(PlayerInteract {
                player: uuid,
                action,
                hand,
                item: p.inventory.slots[slot].clone(),
                world: p.location.world.clone(),
                block,
                face,
                cancelled: false,
            });
            !e.cancelled
        })
    }

    /// Right-click an entity: returns whether the interaction went ahead.
    pub fn interact_entity(&self, uuid: Uuid, target: EntityRef, hand: Hand) -> bool {
        self.op(|| {
            if self.online(uuid).is_none() {
                return false;
            }
            !self
                .fire(PlayerInteractEntity {
                    player: uuid,
                    target,
                    hand,
                    cancelled: false,
                })
                .cancelled
        })
    }

    // ------------------------------------------------------------------ damage, death, respawn

    /// Damage a player: returns the damage actually dealt (`None`: cancelled or invulnerable).
    pub fn damage_player(
        &self,
        uuid: Uuid,
        amount: f32,
        cause: &str,
        attacker: Option<EntityRef>,
    ) -> Option<f32> {
        self.op(|| {
            let p = self.online(uuid)?;
            let bypass = matches!(cause, "minecraft:out_of_world" | "minecraft:generic_kill");
            if p.dead
                || (!bypass && matches!(p.game_mode, GameMode::Creative | GameMode::Spectator))
            {
                return None;
            }
            let e = self.fire(EntityDamage {
                target: p.entity_ref(),
                cause: cause.to_owned(),
                attacker,
                amount,
                cancelled: false,
            });
            if e.cancelled {
                return None;
            }
            let dead = {
                let mut st = self.st();
                let ps = st.online_player_mut(uuid)?;
                ps.health = (ps.health - e.amount).max(0.0);
                ps.health <= 0.0
            };
            if dead {
                self.die(uuid, cause);
            }
            Some(e.amount)
        })
    }

    /// Damage a non-player entity: returns the damage dealt.
    pub fn damage_entity(
        &self,
        world: &str,
        entity: u32,
        amount: f32,
        cause: &str,
        attacker: Option<EntityRef>,
    ) -> Option<f32> {
        self.op(|| {
            let target = self
                .state
                .borrow()
                .worlds
                .get(world)?
                .entities
                .get(&entity)?
                .entity
                .clone();
            let killer = attacker
                .as_ref()
                .filter(|a| a.kind == "minecraft:player")
                .map(|a| a.uuid);
            let e = self.fire(EntityDamage {
                target,
                cause: cause.to_owned(),
                attacker,
                amount,
                cancelled: false,
            });
            if e.cancelled {
                return None;
            }
            let dead = {
                let mut st = self.st();
                let ent = st.worlds.get_mut(world)?.entities.get_mut(&entity)?;
                ent.health = (ent.health - e.amount).max(0.0);
                ent.health <= 0.0
            };
            if dead {
                self.kill_entity(world, entity, killer, Vec::new(), 5);
            }
            Some(e.amount)
        })
    }

    /// A player dies: fire `player-death`, apply keep-inventory / keep-level, drop items,
    /// broadcast the death line.
    pub fn die(&self, uuid: Uuid, cause: &str) {
        self.op(|| {
            let Some(p) = self.online(uuid) else { return };
            if p.dead {
                return;
            }
            {
                let mut st = self.st();
                if let Some(ps) = st.online_player_mut(uuid) {
                    ps.dead = true;
                    ps.health = 0.0;
                }
            }
            if let Some(m) = self.st().close_menu_of(uuid) {
                self.fire(InventoryClose {
                    player: uuid,
                    menu: Some(m),
                });
            }
            let keep = self.state.borrow().keep_inventory;
            let e = self.fire(PlayerDeath {
                player: uuid,
                cause: cause.to_owned(),
                death_message: Some(Text::plain(format!("{} died", p.name))),
                keep_inventory: keep,
                keep_level: keep,
            });
            let mut st = self.st();
            if !e.keep_inventory {
                let stacks = st
                    .players
                    .get(&uuid)
                    .map(|p| p.inventory.stacks())
                    .unwrap_or_default();
                for (_, s) in stacks {
                    st.drop_item(&p.location.world, vec3(&p.location), s);
                }
                if let Some(ps) = st.players.get_mut(&uuid) {
                    ps.inventory.clear();
                }
            }
            if !e.keep_level
                && let Some(ps) = st.players.get_mut(&uuid)
            {
                ps.level = 0;
            }
            if let Some(m) = e.death_message {
                st.broadcast(&m);
            }
        })
    }

    /// A dead player respawns: returns where (`None` if they were not dead).
    pub fn respawn(&self, uuid: Uuid) -> Option<Location> {
        self.op(|| {
            let p = self.online(uuid)?;
            if !p.dead {
                return None;
            }
            let spawn = self.state.borrow().worlds[OVERWORLD].spawn.clone();
            let e = self.fire(PlayerRespawn {
                player: uuid,
                location: spawn,
            });
            {
                let mut st = self.st();
                let ps = st.online_player_mut(uuid)?;
                ps.dead = false;
                ps.health = ps.max_health;
                ps.food = 20;
                ps.saturation = 5.0;
                ps.location = e.location.clone();
            }
            if e.location.world != p.location.world {
                self.fire(PlayerChangeWorld {
                    player: uuid,
                    from_world: p.location.world,
                    to_world: e.location.world.clone(),
                });
            }
            Some(e.location)
        })
    }

    /// A non-player entity dies: fire `entity-death`, remove it, spawn its drops.
    pub fn kill_entity(
        &self,
        world: &str,
        entity: u32,
        killer: Option<Uuid>,
        drops: Vec<ItemStack>,
        exp: u32,
    ) -> Option<EntityDeath> {
        self.op(|| {
            let ent = self
                .state
                .borrow()
                .worlds
                .get(world)?
                .entities
                .get(&entity)?
                .clone();
            let e = self.fire(EntityDeath {
                entity: ent.entity.clone(),
                killer,
                drops,
                dropped_exp: exp,
            });
            let mut st = self.st();
            st.worlds.get_mut(world)?.entities.remove(&entity);
            for d in &e.drops {
                st.drop_item(world, vec3(&ent.location), d.clone());
            }
            Some(e)
        })
    }

    // ------------------------------------------------------------------ menus

    /// Click a slot of the player's open menu: returns whether the click went through
    /// (plugin menus cancel clicks unless a handler allows them).
    pub fn click_menu(&self, uuid: Uuid, slot: i32, click: ClickKind, hotbar_button: i32) -> bool {
        self.op(|| {
            let Some(p) = self.online(uuid) else {
                return false;
            };
            let Some(menu_id) = p.open_menu else {
                return false;
            };
            let (rows, item) = {
                let st = self.state.borrow();
                let Some(m) = st.menus.get(&menu_id) else {
                    return false;
                };
                let size = i32::from(m.rows) * 9;
                let item = if slot < 0 {
                    ItemStack::empty()
                } else if slot < size {
                    m.items
                        .get(&(slot as u32))
                        .cloned()
                        .unwrap_or_else(ItemStack::empty)
                } else {
                    view_to_inventory(slot - size)
                        .map(|i| p.inventory.slots[i].clone())
                        .unwrap_or_else(ItemStack::empty)
                };
                (m.rows, item)
            };
            let e = self.fire(InventoryClick {
                player: uuid,
                menu: Some(menu_id),
                slot,
                click,
                hotbar_button,
                item,
                cursor: p.cursor.clone(),
                cancelled: false,
            });
            if e.cancelled {
                return false;
            }
            let mut st = self.st();
            if !st.menus.contains_key(&menu_id) {
                return true; // closed by the handler
            }
            apply_click(&mut st, uuid, menu_id, rows, slot, click);
            true
        })
    }

    /// The player closes their open menu (Escape).
    pub fn close_menu(&self, uuid: Uuid) -> bool {
        self.op(|| {
            let m = self.st().close_menu_of(uuid);
            match m {
                Some(m) => {
                    self.fire(InventoryClose {
                        player: uuid,
                        menu: Some(m),
                    });
                    true
                }
                None => false,
            }
        })
    }

    // ------------------------------------------------------------------ misc

    /// A custom payload from a client: returns whether some plugin listens on the channel.
    pub fn client_plugin_message(&self, uuid: Uuid, channel: &str, data: &[u8]) -> bool {
        self.op(|| {
            let channel = channel.to_ascii_lowercase();
            if self.online(uuid).is_none()
                || !self.state.borrow().incoming_channels.contains(&channel)
            {
                return false;
            }
            self.fire(PluginMessage {
                player: uuid,
                channel,
                data: data.to_vec(),
            });
            true
        })
    }

    /// Run async tasks that are due now (inline, deterministic): returns how many ran.
    ///
    /// Native plugins' tasks run here on the test thread. Once a WebAssembly plugin is loaded,
    /// the host's async pool owns the async queue: this waits until it is idle instead.
    pub fn run_async_due(&self, became_due: bool) -> usize {
        if let Some(w) = self.wasm.as_ref().filter(|w| w.active()) {
            return w.wait_idle(self, became_due);
        }
        let mut ran = 0;
        for _round in 0..64 {
            let due = self
                .rt
                .due_async_tasks(rusticmc_core::clock::Clock::now(&*self.clock));
            if due.is_empty() {
                break;
            }
            for d in due {
                ran += 1;
                let task = d.task;
                let result = self
                    .rt
                    .with_plugin_instance(&d.plugin, |inst| match &d.job {
                        Some(j) => inst.run_job(task, &j.name, &j.payload),
                        None => inst.run_task(task),
                    })
                    .map_err(|e| e.to_string());
                if let Err(e) = result {
                    log::warn!(target: "rustic", "[{}] async task {} failed: {e}", d.plugin, d.task);
                }
                self.rt.async_task_finished(d.task);
            }
        }
        ran
    }

    /// One server tick.
    pub fn tick(&self) -> rusticmc_core::TickReport {
        self.op(|| {
            let next_async = self.rt.shared().scheduler().next_async_deadline();
            self.clock.advance(Duration::from_millis(50));
            let became_due =
                next_async.is_some_and(|d| d <= rusticmc_core::clock::Clock::now(&*self.clock));
            {
                let mut st = self.st();
                for w in st.worlds.values_mut() {
                    w.time = w.time.wrapping_add(1);
                    if w.weather_ticks > 0 {
                        w.weather_ticks -= 1;
                        if w.weather_ticks == 0 {
                            w.weather = Weather::Clear;
                        }
                    }
                }
            }
            let t0 = std::time::Instant::now();
            let report = self.rt.tick(&mut self.backend());
            if self.async_mode.get() == AsyncMode::Inline {
                self.run_async_due(became_due);
            }
            let took = t0.elapsed();
            self.rt.record_server_tick(took);
            self.st().tick_ms = took.as_secs_f64() * 1000.0;
            report
        })
    }
}

/// Container view slot (below the menu) → player inventory slot: 0-26 are the main inventory
/// (9-35), 27-35 the hotbar (0-8).
fn view_to_inventory(view: i32) -> Option<usize> {
    match view {
        0..=26 => Some(view as usize + 9),
        27..=35 => Some(view as usize - 27),
        _ => None,
    }
}

/// Apply an allowed click: left/right swap or merge the cursor with the slot, shift-click moves
/// the stack to the other side, drop throws one item (control-drop the stack).
fn apply_click(
    st: &mut GameState,
    uuid: Uuid,
    menu: MenuId,
    rows: u8,
    slot: i32,
    click: ClickKind,
) {
    let size = i32::from(rows) * 9;
    if slot < 0 {
        return;
    }
    let world = match st.players.get(&uuid) {
        Some(p) => p.location.clone(),
        None => return,
    };
    // Read the clicked stack.
    let current = if slot < size {
        st.menus[&menu]
            .items
            .get(&(slot as u32))
            .cloned()
            .unwrap_or_else(ItemStack::empty)
    } else {
        match view_to_inventory(slot - size) {
            Some(i) => st.players[&uuid].inventory.slots[i].clone(),
            None => return,
        }
    };
    let cursor = st.players[&uuid].cursor.clone();
    let (new_slot, new_cursor, dropped, to_player) = match click {
        ClickKind::Left | ClickKind::Right => {
            if stackable(&current, &cursor) {
                let max = max_stack(&current.id);
                let n = (max - current.count.min(max)).min(cursor.count);
                let mut s = current.clone();
                s.count += n;
                let mut c = cursor.clone();
                c.count -= n;
                (s, normalize_item(&c), None, None)
            } else {
                (cursor, current, None, None)
            }
        }
        ClickKind::ShiftLeft | ClickKind::ShiftRight if slot < size => {
            (ItemStack::empty(), cursor, None, Some(current))
        }
        ClickKind::Drop | ClickKind::ControlDrop if !current.is_empty() => {
            let n = if click == ClickKind::Drop {
                1
            } else {
                current.count
            };
            let mut rest = current.clone();
            rest.count -= n;
            let mut thrown = current.clone();
            thrown.count = n;
            (normalize_item(&rest), cursor, Some(thrown), None)
        }
        _ => return,
    };
    let mut new_slot = new_slot;
    if let Some(stack) = to_player {
        let left = st
            .players
            .get_mut(&uuid)
            .map(|p| p.inventory.give(&stack))
            .unwrap_or(stack);
        new_slot = left;
    }
    if slot < size {
        let items = &mut st.menus.get_mut(&menu).expect("menu exists").items;
        if new_slot.is_empty() {
            items.remove(&(slot as u32));
        } else {
            items.insert(slot as u32, new_slot);
        }
    } else if let Some(i) = view_to_inventory(slot - size)
        && let Some(p) = st.players.get_mut(&uuid)
    {
        p.inventory.slots[i] = normalize_item(&new_slot);
    }
    if let Some(p) = st.players.get_mut(&uuid) {
        p.cursor = normalize_item(&new_cursor);
    }
    if let Some(thrown) = dropped {
        st.drop_item(&world.world, world.vec3(), thrown);
    }
}

/// The [`Backend`] of the harness: every call reads or changes [`GameState`].
pub(crate) struct MockBackend<'a> {
    h: &'a Harness,
}

macro_rules! player_mut {
    ($self:ident, $player:expr) => {
        RefMut::filter_map($self.h.st(), |s| s.online_player_mut($player))
            .map_err(|_| HostError::NoSuchPlayer)
    };
}

impl MockBackend<'_> {
    fn st(&self) -> RefMut<'_, GameState> {
        self.h.st()
    }

    fn get<T>(&self, player: Uuid, f: impl FnOnce(&PlayerState) -> T) -> Result<T, HostError> {
        self.h
            .state
            .borrow()
            .online_player(player)
            .map(f)
            .ok_or(HostError::NoSuchPlayer)
    }

    fn world<T>(&self, world: &str, f: impl FnOnce(&mut WorldState) -> T) -> Result<T, HostError> {
        self.st()
            .worlds
            .get_mut(world)
            .map(f)
            .ok_or(HostError::NoSuchWorld)
    }
}

fn check_slot(slot: u32) -> Result<usize, HostError> {
    if (slot as usize) < INVENTORY_SLOTS {
        Ok(slot as usize)
    } else {
        Err(HostError::Invalid(format!(
            "inventory slot {slot} is out of range (0-40)"
        )))
    }
}

impl Backend for MockBackend<'_> {
    fn console_message(&mut self, message: &Text) -> Result<(), HostError> {
        self.st().console.push(message.to_plain());
        Ok(())
    }

    fn plugin_disabled(&mut self, plugin: &str) {
        self.st().disabled_plugins.push(plugin.to_owned());
    }

    fn commands_changed(&mut self) {
        self.st().commands_changed += 1;
    }

    // ---- server
    fn server_broadcast(&mut self, message: &Text) -> Result<(), HostError> {
        self.st().broadcast(message);
        Ok(())
    }
    fn server_dispatch_command(&mut self, command: &str) -> Result<bool, HostError> {
        let line = command.trim().trim_start_matches('/').to_owned();
        self.st().console_commands.push(line.clone());
        Ok(self.h.run_command(CommandSender::Console, &line).executed())
    }
    fn server_get_tick_stats(&mut self) -> Result<TickStats, HostError> {
        Ok(TickStats {
            mspt: self.st().tick_ms,
            tps: 20.0,
        })
    }
    fn server_max_players(&mut self) -> Result<u32, HostError> {
        Ok(self.st().max_players)
    }
    fn server_motd(&mut self) -> Result<Text, HostError> {
        Ok(self.st().motd.clone())
    }
    fn server_version(&mut self) -> Result<String, HostError> {
        Ok(format!(
            "Rustic {} on MockServer (rusticmc-test {})",
            rusticmc_core::runtime::CORE_VERSION,
            env!("CARGO_PKG_VERSION")
        ))
    }
    fn server_offline_player(&mut self, name: &str) -> Result<Option<Uuid>, HostError> {
        Ok(self
            .h
            .state
            .borrow()
            .players
            .values()
            .find(|p| p.name.eq_ignore_ascii_case(name))
            .map(|p| p.uuid))
    }
    fn server_operators(&mut self) -> Result<Vec<Uuid>, HostError> {
        Ok(self.st().ops.iter().copied().collect())
    }

    // ---- players
    fn players_online(&mut self) -> Result<Vec<Uuid>, HostError> {
        Ok(self.st().online.clone())
    }
    fn players_name(&mut self, player: Uuid) -> Result<Option<String>, HostError> {
        Ok(self.st().players.get(&player).map(|p| p.name.clone()))
    }
    fn players_by_name(&mut self, name: &str) -> Result<Option<Uuid>, HostError> {
        Ok(self.st().by_name(name))
    }
    fn players_is_online(&mut self, player: Uuid) -> Result<bool, HostError> {
        Ok(self.st().online_player(player).is_some())
    }
    fn players_get_location(&mut self, player: Uuid) -> Result<Location, HostError> {
        self.get(player, |p| p.location.clone())
    }
    fn players_address(&mut self, player: Uuid) -> Result<String, HostError> {
        self.get(player, |p| p.address.clone())
    }
    fn players_ping(&mut self, player: Uuid) -> Result<u32, HostError> {
        self.get(player, |p| p.ping)
    }
    fn players_locale(&mut self, player: Uuid) -> Result<String, HostError> {
        self.get(player, |p| p.locale.clone())
    }
    fn players_send_message(&mut self, player: Uuid, message: &Text) -> Result<(), HostError> {
        player_mut!(self, player)?
            .received
            .push(Received::Message(message.clone()));
        Ok(())
    }
    fn players_send_action_bar(&mut self, player: Uuid, message: &Text) -> Result<(), HostError> {
        player_mut!(self, player)?
            .received
            .push(Received::ActionBar(message.clone()));
        Ok(())
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
        player_mut!(self, player)?
            .received
            .push(Received::Title(TitleRecord {
                title: title.clone(),
                subtitle: subtitle.clone(),
                fade_in,
                stay,
                fade_out,
            }));
        Ok(())
    }
    fn players_play_sound(
        &mut self,
        player: Uuid,
        sound: &str,
        volume: f32,
        pitch: f32,
    ) -> Result<(), HostError> {
        let mut p = player_mut!(self, player)?;
        let at = p.location.vec3();
        p.received.push(Received::Sound(SoundRecord {
            sound: namespaced(sound),
            at,
            volume,
            pitch,
        }));
        Ok(())
    }
    fn players_teleport(&mut self, player: Uuid, to: &Location) -> Result<(), HostError> {
        if !to.is_finite() {
            return Err(HostError::Invalid("non-finite location".into()));
        }
        if !self.st().worlds.contains_key(&to.world) {
            return Err(HostError::NoSuchWorld);
        }
        if self.st().online_player(player).is_none() {
            return Err(HostError::NoSuchPlayer);
        }
        self.h.teleport(player, to.clone());
        Ok(())
    }
    fn players_get_game_mode(&mut self, player: Uuid) -> Result<GameMode, HostError> {
        self.get(player, |p| p.game_mode)
    }
    fn players_set_game_mode(&mut self, player: Uuid, mode: GameMode) -> Result<(), HostError> {
        self.get(player, |_| ())?;
        self.h.set_game_mode(player, mode);
        Ok(())
    }
    fn players_health(&mut self, player: Uuid) -> Result<f32, HostError> {
        self.get(player, |p| p.health)
    }
    fn players_set_health(&mut self, player: Uuid, health: f32) -> Result<(), HostError> {
        if !health.is_finite() || health < 0.0 {
            return Err(HostError::Invalid(format!("health {health}")));
        }
        let mut st = self.st();
        let p = st
            .online_player_mut(player)
            .ok_or(HostError::NoSuchPlayer)?;
        if health > p.max_health {
            return Err(HostError::Invalid(format!(
                "health {health} is above the maximum {}",
                p.max_health
            )));
        }
        p.health = health;
        if health <= 0.0 && !p.dead {
            st.pending.push_back(Pending::Death {
                player,
                cause: "minecraft:generic".into(),
            });
        }
        Ok(())
    }
    fn players_max_health(&mut self, player: Uuid) -> Result<f32, HostError> {
        self.get(player, |p| p.max_health)
    }
    fn players_food_level(&mut self, player: Uuid) -> Result<u32, HostError> {
        self.get(player, |p| p.food)
    }
    fn players_set_food_level(&mut self, player: Uuid, food: u32) -> Result<(), HostError> {
        player_mut!(self, player)?.food = food.min(20);
        Ok(())
    }
    fn players_saturation(&mut self, player: Uuid) -> Result<f32, HostError> {
        self.get(player, |p| p.saturation)
    }
    fn players_set_saturation(&mut self, player: Uuid, saturation: f32) -> Result<(), HostError> {
        let mut p = player_mut!(self, player)?;
        if !saturation.is_finite() {
            return Err(HostError::Invalid("non-finite saturation".into()));
        }
        p.saturation = saturation.clamp(0.0, p.food as f32);
        Ok(())
    }
    fn players_experience_level(&mut self, player: Uuid) -> Result<u32, HostError> {
        self.get(player, |p| p.level)
    }
    fn players_set_experience_level(&mut self, player: Uuid, level: u32) -> Result<(), HostError> {
        player_mut!(self, player)?.level = level;
        Ok(())
    }
    fn players_set_flying(
        &mut self,
        player: Uuid,
        allow: bool,
        flying: bool,
    ) -> Result<(), HostError> {
        let mut p = player_mut!(self, player)?;
        p.allow_flight = allow;
        p.flying = allow && flying;
        Ok(())
    }
    fn players_kick(&mut self, player: Uuid, reason: &Text) -> Result<(), HostError> {
        let mut st = self.st();
        let p = st
            .online_player_mut(player)
            .ok_or(HostError::NoSuchPlayer)?;
        if p.kicked.is_none() {
            p.kicked = Some(reason.clone());
            st.pending.push_back(Pending::Kicked { player });
        }
        Ok(())
    }
    fn players_is_op(&mut self, player: Uuid) -> Result<bool, HostError> {
        Ok(self.st().ops.contains(&player))
    }
    fn players_set_op(&mut self, player: Uuid, op: bool) -> Result<(), HostError> {
        let mut st = self.st();
        if !st.players.contains_key(&player) {
            return Err(HostError::NoSuchPlayer);
        }
        if op {
            st.ops.insert(player);
        } else {
            st.ops.remove(&player);
        }
        Ok(())
    }

    // ---- inventories
    fn inventories_get_slot(&mut self, player: Uuid, slot: u32) -> Result<ItemStack, HostError> {
        let i = check_slot(slot)?;
        self.get(player, |p| p.inventory.slots[i].clone())
    }
    fn inventories_set_slot(
        &mut self,
        player: Uuid,
        slot: u32,
        item: &ItemStack,
    ) -> Result<(), HostError> {
        let i = check_slot(slot)?;
        player_mut!(self, player)?.inventory.slots[i] = normalize_item(item);
        Ok(())
    }
    fn inventories_contents(&mut self, player: Uuid) -> Result<Vec<ItemStack>, HostError> {
        self.get(player, |p| p.inventory.slots.clone())
    }
    fn inventories_give(&mut self, player: Uuid, item: &ItemStack) -> Result<ItemStack, HostError> {
        Ok(player_mut!(self, player)?.inventory.give(item))
    }
    fn inventories_take(&mut self, player: Uuid, id: &str, count: u32) -> Result<u32, HostError> {
        Ok(player_mut!(self, player)?.inventory.take(id, count))
    }
    fn inventories_clear(&mut self, player: Uuid) -> Result<(), HostError> {
        player_mut!(self, player)?.inventory.clear();
        Ok(())
    }
    fn inventories_held_slot(&mut self, player: Uuid) -> Result<u32, HostError> {
        self.get(player, |p| p.held_slot)
    }
    fn inventories_close_container(&mut self, player: Uuid) -> Result<(), HostError> {
        let mut st = self.st();
        if st.online_player(player).is_none() {
            return Err(HostError::NoSuchPlayer);
        }
        if let Some(m) = st.close_menu_of(player) {
            st.pending.push_back(Pending::MenuClosed {
                player,
                menu: Some(m),
            });
        }
        Ok(())
    }

    // ---- menus
    fn menus_open(&mut self, player: Uuid, spec: &MenuSpec) -> Result<MenuId, HostError> {
        if spec.rows == 0 || spec.rows > 6 {
            return Err(HostError::Invalid("menus have 1 to 6 rows".into()));
        }
        let size = u32::from(spec.rows) * 9;
        if let Some((slot, _)) = spec.items.iter().find(|(s, _)| *s >= size) {
            return Err(HostError::Invalid(format!(
                "slot {slot} is outside a {}-row menu",
                spec.rows
            )));
        }
        let mut st = self.st();
        if st.online_player(player).is_none() {
            return Err(HostError::NoSuchPlayer);
        }
        if let Some(old) = st.close_menu_of(player) {
            st.pending.push_back(Pending::MenuClosed {
                player,
                menu: Some(old),
            });
        }
        let id = st.next_menu;
        st.next_menu += 1;
        st.menus.insert(
            id,
            MenuState {
                id,
                player,
                rows: spec.rows,
                title: spec.title.clone(),
                items: spec
                    .items
                    .iter()
                    .map(|(s, i)| (*s, normalize_item(i)))
                    .filter(|(_, i)| !i.is_empty())
                    .collect(),
            },
        );
        if let Some(p) = st.players.get_mut(&player) {
            p.open_menu = Some(id);
        }
        Ok(id)
    }
    fn menus_set_item(
        &mut self,
        menu: MenuId,
        slot: u32,
        item: &ItemStack,
    ) -> Result<(), HostError> {
        let mut st = self.st();
        let m = st
            .menus
            .get_mut(&menu)
            .ok_or_else(|| HostError::Invalid(format!("menu {menu} is not open")))?;
        if slot >= u32::from(m.rows) * 9 {
            return Err(HostError::Invalid(format!(
                "slot {slot} is outside the menu"
            )));
        }
        let item = normalize_item(item);
        if item.is_empty() {
            m.items.remove(&slot);
        } else {
            m.items.insert(slot, item);
        }
        Ok(())
    }
    fn menus_set_title(&mut self, menu: MenuId, title: &Text) -> Result<(), HostError> {
        let mut st = self.st();
        let m = st
            .menus
            .get_mut(&menu)
            .ok_or_else(|| HostError::Invalid(format!("menu {menu} is not open")))?;
        m.title = title.clone();
        Ok(())
    }
    fn menus_close(&mut self, menu: MenuId) -> Result<(), HostError> {
        let mut st = self.st();
        if let Some(m) = st.menus.remove(&menu) {
            if let Some(p) = st.players.get_mut(&m.player)
                && p.open_menu == Some(menu)
            {
                p.open_menu = None;
            }
            st.pending.push_back(Pending::MenuClosed {
                player: m.player,
                menu: Some(menu),
            });
        }
        Ok(())
    }
    fn menus_open_menu(&mut self, player: Uuid) -> Result<Option<MenuId>, HostError> {
        Ok(self.st().players.get(&player).and_then(|p| p.open_menu))
    }

    // ---- worlds
    fn worlds_all(&mut self) -> Result<Vec<String>, HostError> {
        Ok(self.st().worlds.keys().cloned().collect())
    }
    fn worlds_get_block(&mut self, world: &str, pos: BlockPos) -> Result<BlockState, HostError> {
        self.world(world, |w| w.block(pos))
    }
    fn worlds_set_block(
        &mut self,
        world: &str,
        pos: BlockPos,
        state: &BlockState,
        physics: bool,
    ) -> Result<bool, HostError> {
        self.world(world, |w| {
            let changed = w.set(pos, state);
            if changed && physics {
                w.physics_updates += 1;
            }
            changed
        })
    }
    fn worlds_set_blocks(
        &mut self,
        world: &str,
        blocks: &[(BlockPos, BlockState)],
        physics: bool,
    ) -> Result<u32, HostError> {
        self.world(world, |w| {
            let mut n = 0;
            for (pos, state) in blocks {
                if w.set(*pos, state) {
                    n += 1;
                }
            }
            if physics {
                w.physics_updates += u64::from(n);
            }
            n
        })
    }
    fn worlds_fill(
        &mut self,
        world: &str,
        from: BlockPos,
        to: BlockPos,
        state: &BlockState,
    ) -> Result<u32, HostError> {
        let (x0, x1) = (from.x.min(to.x), from.x.max(to.x));
        let (y0, y1) = (from.y.min(to.y), from.y.max(to.y));
        let (z0, z1) = (from.z.min(to.z), from.z.max(to.z));
        let volume = (i64::from(x1 - x0) + 1) * (i64::from(y1 - y0) + 1) * (i64::from(z1 - z0) + 1);
        if volume > 32_768 {
            return Err(HostError::Invalid(format!(
                "fill of {volume} blocks exceeds the 32768 limit"
            )));
        }
        self.world(world, |w| {
            let mut n = 0;
            for x in x0..=x1 {
                for y in y0..=y1 {
                    for z in z0..=z1 {
                        if w.set(BlockPos { x, y, z }, state) {
                            n += 1;
                        }
                    }
                }
            }
            n
        })
    }
    fn worlds_highest_block_y(&mut self, world: &str, x: i32, z: i32) -> Result<i32, HostError> {
        self.world(world, |w| w.highest_y(x, z))
    }
    fn worlds_is_chunk_loaded(&mut self, world: &str, cx: i32, cz: i32) -> Result<bool, HostError> {
        Ok(self
            .st()
            .worlds
            .get(world)
            .is_some_and(|w| w.loaded_chunks.contains(&(cx, cz))))
    }
    fn worlds_keep_chunk_loaded(
        &mut self,
        world: &str,
        cx: i32,
        cz: i32,
        keep: bool,
    ) -> Result<(), HostError> {
        self.world(world, |w| {
            if keep {
                w.forced_chunks.insert((cx, cz));
                w.loaded_chunks.insert((cx, cz));
            } else {
                w.forced_chunks.remove(&(cx, cz));
            }
        })
    }
    fn worlds_time(&mut self, world: &str) -> Result<u64, HostError> {
        self.world(world, |w| w.time)
    }
    fn worlds_set_time(&mut self, world: &str, time: u64) -> Result<(), HostError> {
        self.world(world, |w| w.time = time)
    }
    fn worlds_get_weather(&mut self, world: &str) -> Result<Weather, HostError> {
        self.world(world, |w| w.weather)
    }
    fn worlds_set_weather(
        &mut self,
        world: &str,
        weather: Weather,
        duration_ticks: u32,
    ) -> Result<(), HostError> {
        self.world(world, |w| {
            w.weather = weather;
            w.weather_ticks = duration_ticks;
        })
    }
    fn worlds_spawn(&mut self, world: &str) -> Result<Location, HostError> {
        self.world(world, |w| w.spawn.clone())
    }
    fn worlds_set_spawn(&mut self, world: &str, at: &Location) -> Result<(), HostError> {
        if !at.is_finite() {
            return Err(HostError::Invalid("non-finite location".into()));
        }
        self.world(world, |w| {
            w.spawn = Location {
                world: world.to_owned(),
                ..at.clone()
            }
        })
    }
    fn worlds_spawn_entity(
        &mut self,
        world: &str,
        kind: &str,
        at: Vec3,
        nbt: &str,
    ) -> Result<EntityRef, HostError> {
        let mut st = self.st();
        if !st.worlds.contains_key(world) {
            return Err(HostError::NoSuchWorld);
        }
        if kind.trim().is_empty() {
            return Err(HostError::Invalid("empty entity type".into()));
        }
        let id = st.next_entity_id();
        let entity = EntityRef {
            id,
            uuid: Uuid::new(0x0e17_0000_0000_0000 | u64::from(id), u64::from(id)),
            kind: namespaced(kind),
        };
        let w = st.worlds.get_mut(world).ok_or(HostError::NoSuchWorld)?;
        w.entities.insert(
            id,
            EntityState {
                entity: entity.clone(),
                location: Location {
                    world: world.to_owned(),
                    x: at.x,
                    y: at.y,
                    z: at.z,
                    yaw: 0.0,
                    pitch: 0.0,
                },
                nbt: nbt.to_owned(),
                item: None,
                health: 20.0,
            },
        );
        Ok(entity)
    }
    fn worlds_remove_entity(&mut self, world: &str, entity: u32) -> Result<(), HostError> {
        self.world(world, |w| w.entities.remove(&entity))?
            .map(|_| ())
            .ok_or_else(|| HostError::Invalid(format!("no entity {entity} in {world}")))
    }
    fn worlds_entities_near(
        &mut self,
        world: &str,
        at: Vec3,
        radius: f64,
    ) -> Result<Vec<EntityRef>, HostError> {
        let st = self.h.state.borrow();
        let w = st.worlds.get(world).ok_or(HostError::NoSuchWorld)?;
        let near = |l: &Location| {
            let (dx, dy, dz) = (l.x - at.x, l.y - at.y, l.z - at.z);
            (dx * dx + dy * dy + dz * dz).sqrt() <= radius
        };
        let mut out: Vec<EntityRef> = w
            .entities
            .values()
            .filter(|e| near(&e.location))
            .map(|e| e.entity.clone())
            .collect();
        out.extend(
            st.online
                .iter()
                .map(|u| &st.players[u])
                .filter(|p| p.location.world == world && !p.dead && near(&p.location))
                .map(PlayerState::entity_ref),
        );
        Ok(out)
    }
    fn worlds_entity_location(&mut self, world: &str, entity: u32) -> Result<Location, HostError> {
        let st = self.h.state.borrow();
        let w = st.worlds.get(world).ok_or(HostError::NoSuchWorld)?;
        if let Some(e) = w.entities.get(&entity) {
            return Ok(e.location.clone());
        }
        st.online
            .iter()
            .map(|u| &st.players[u])
            .find(|p| p.entity_id == entity && p.location.world == world)
            .map(|p| p.location.clone())
            .ok_or_else(|| HostError::Invalid(format!("no entity {entity} in {world}")))
    }
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
        self.world(world, |w| {
            w.particles.push(ParticleRecord {
                particle: namespaced(particle),
                options: options.to_owned(),
                at,
                count,
                offset,
                speed,
            })
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
        let mut st = self.st();
        if !st.worlds.contains_key(world) {
            return Err(HostError::NoSuchWorld);
        }
        st.world_sound(
            world,
            SoundRecord {
                sound: namespaced(sound),
                at,
                volume,
                pitch,
            },
        );
        Ok(())
    }
    fn worlds_players(&mut self, world: &str) -> Result<Vec<Uuid>, HostError> {
        let st = self.h.state.borrow();
        Ok(st
            .online
            .iter()
            .copied()
            .filter(|u| st.players[u].location.world == world)
            .collect())
    }

    // ---- scoreboards
    fn scoreboards_create_objective(
        &mut self,
        name: &str,
        display: &Text,
        criteria: &str,
    ) -> Result<(), HostError> {
        let mut st = self.st();
        if st.objectives.contains_key(name) {
            return Err(HostError::Invalid(format!(
                "objective `{name}` already exists"
            )));
        }
        st.objectives.insert(
            name.to_owned(),
            ObjectiveState {
                display: display.clone(),
                criteria: criteria.to_owned(),
                scores: Default::default(),
            },
        );
        Ok(())
    }
    fn scoreboards_remove_objective(&mut self, name: &str) -> Result<(), HostError> {
        let mut st = self.st();
        st.objectives
            .remove(name)
            .ok_or_else(|| HostError::Invalid(format!("no objective `{name}`")))?;
        st.display_slots.retain(|_, o| o != name);
        Ok(())
    }
    fn scoreboards_set_display(
        &mut self,
        slot: DisplaySlot,
        objective: Option<&str>,
    ) -> Result<(), HostError> {
        let mut st = self.st();
        match objective {
            Some(o) => {
                if !st.objectives.contains_key(o) {
                    return Err(HostError::Invalid(format!("no objective `{o}`")));
                }
                st.display_slots
                    .insert(slot_key(slot).to_owned(), o.to_owned());
            }
            None => {
                st.display_slots.remove(slot_key(slot));
            }
        }
        Ok(())
    }
    fn scoreboards_set_objective_display(
        &mut self,
        name: &str,
        display: &Text,
    ) -> Result<(), HostError> {
        let mut st = self.st();
        let o = st
            .objectives
            .get_mut(name)
            .ok_or_else(|| HostError::Invalid(format!("no objective `{name}`")))?;
        o.display = display.clone();
        Ok(())
    }
    fn scoreboards_set_score(
        &mut self,
        objective: &str,
        entry: &str,
        score: i32,
    ) -> Result<(), HostError> {
        let mut st = self.st();
        let o = st
            .objectives
            .get_mut(objective)
            .ok_or_else(|| HostError::Invalid(format!("no objective `{objective}`")))?;
        o.scores.insert(entry.to_owned(), score);
        Ok(())
    }
    fn scoreboards_reset_score(&mut self, objective: &str, entry: &str) -> Result<(), HostError> {
        let mut st = self.st();
        let o = st
            .objectives
            .get_mut(objective)
            .ok_or_else(|| HostError::Invalid(format!("no objective `{objective}`")))?;
        o.scores.remove(entry);
        Ok(())
    }
    fn scoreboards_get_score(
        &mut self,
        objective: &str,
        entry: &str,
    ) -> Result<Option<i32>, HostError> {
        Ok(self
            .st()
            .objectives
            .get(objective)
            .and_then(|o| o.scores.get(entry).copied()))
    }
    fn scoreboards_create_team(&mut self, name: &str) -> Result<(), HostError> {
        let mut st = self.st();
        if st.teams.contains_key(name) {
            return Err(HostError::Invalid(format!("team `{name}` already exists")));
        }
        st.teams.insert(
            name.to_owned(),
            TeamState {
                prefix: Text::plain(""),
                suffix: Text::plain(""),
                color: TeamColor::Reset,
                members: Default::default(),
            },
        );
        Ok(())
    }
    fn scoreboards_remove_team(&mut self, name: &str) -> Result<(), HostError> {
        self.st()
            .teams
            .remove(name)
            .map(|_| ())
            .ok_or_else(|| HostError::Invalid(format!("no team `{name}`")))
    }
    fn scoreboards_set_team_prefix(&mut self, name: &str, prefix: &Text) -> Result<(), HostError> {
        team(self, name, |t| t.prefix = prefix.clone())
    }
    fn scoreboards_set_team_suffix(&mut self, name: &str, suffix: &Text) -> Result<(), HostError> {
        team(self, name, |t| t.suffix = suffix.clone())
    }
    fn scoreboards_set_team_color(
        &mut self,
        name: &str,
        color: TeamColor,
    ) -> Result<(), HostError> {
        team(self, name, |t| t.color = color)
    }
    fn scoreboards_team_add(&mut self, name: &str, entry: &str) -> Result<(), HostError> {
        let mut st = self.st();
        if !st.teams.contains_key(name) {
            return Err(HostError::Invalid(format!("no team `{name}`")));
        }
        // An entry is in at most one team, as in vanilla.
        for t in st.teams.values_mut() {
            t.members.remove(entry);
        }
        st.teams
            .get_mut(name)
            .expect("checked")
            .members
            .insert(entry.to_owned());
        Ok(())
    }
    fn scoreboards_team_remove(&mut self, name: &str, entry: &str) -> Result<(), HostError> {
        team(self, name, |t| {
            t.members.remove(entry);
        })
    }

    // ---- bossbars
    fn bossbars_create(
        &mut self,
        title: &Text,
        color: BarColor,
        style: BarStyle,
        progress: f32,
    ) -> Result<BarId, HostError> {
        let mut st = self.st();
        let id = st.next_bar;
        st.next_bar += 1;
        st.bars.insert(
            id,
            BarState {
                title: title.clone(),
                color,
                style,
                progress,
                viewers: Default::default(),
            },
        );
        Ok(id)
    }
    fn bossbars_set_title(&mut self, bar: BarId, title: &Text) -> Result<(), HostError> {
        with_bar(self, bar, |b| b.title = title.clone())
    }
    fn bossbars_set_progress(&mut self, bar: BarId, progress: f32) -> Result<(), HostError> {
        with_bar(self, bar, |b| b.progress = progress)
    }
    fn bossbars_set_color(&mut self, bar: BarId, color: BarColor) -> Result<(), HostError> {
        with_bar(self, bar, |b| b.color = color)
    }
    fn bossbars_show(&mut self, bar: BarId, player: Uuid) -> Result<(), HostError> {
        if self.st().online_player(player).is_none() {
            return Err(HostError::NoSuchPlayer);
        }
        with_bar(self, bar, |b| {
            b.viewers.insert(player);
        })
    }
    fn bossbars_hide(&mut self, bar: BarId, player: Uuid) -> Result<(), HostError> {
        with_bar(self, bar, |b| {
            b.viewers.remove(&player);
        })
    }
    fn bossbars_remove(&mut self, bar: BarId) -> Result<(), HostError> {
        self.st().bars.remove(&bar);
        Ok(())
    }

    // ---- tablist
    fn tablist_set_header_footer(
        &mut self,
        player: Uuid,
        header: &Text,
        footer: &Text,
    ) -> Result<(), HostError> {
        let mut p = player_mut!(self, player)?;
        p.tab_header = Some(header.clone());
        p.tab_footer = Some(footer.clone());
        Ok(())
    }
    fn tablist_set_display_name(
        &mut self,
        player: Uuid,
        name: Option<&Text>,
    ) -> Result<(), HostError> {
        player_mut!(self, player)?.display_name = name.cloned();
        Ok(())
    }

    // ---- messaging
    fn messaging_register_incoming(&mut self, channel: &str) -> Result<(), HostError> {
        self.st()
            .incoming_channels
            .insert(channel.to_ascii_lowercase());
        Ok(())
    }
    fn messaging_unregister_incoming(&mut self, channel: &str) -> Result<(), HostError> {
        self.st()
            .incoming_channels
            .remove(&channel.to_ascii_lowercase());
        Ok(())
    }
    fn messaging_send(
        &mut self,
        player: Uuid,
        channel: &str,
        data: &[u8],
    ) -> Result<(), HostError> {
        if data.len() > 1024 * 1024 {
            return Err(HostError::Invalid(
                "plugin messages are at most 1 MiB".into(),
            ));
        }
        player_mut!(self, player)?
            .plugin_messages
            .push((channel.to_ascii_lowercase(), data.to_vec()));
        Ok(())
    }
    fn messaging_behind_proxy(&mut self) -> Result<bool, HostError> {
        Ok(self.st().behind_proxy)
    }
}

fn team(
    b: &mut MockBackend<'_>,
    name: &str,
    f: impl FnOnce(&mut TeamState),
) -> Result<(), HostError> {
    let mut st = b.st();
    let t = st
        .teams
        .get_mut(name)
        .ok_or_else(|| HostError::Invalid(format!("no team `{name}`")))?;
    f(t);
    Ok(())
}

fn with_bar(
    b: &mut MockBackend<'_>,
    bar: BarId,
    f: impl FnOnce(&mut BarState),
) -> Result<(), HostError> {
    let mut st = b.st();
    let bar = st
        .bars
        .get_mut(&bar)
        .ok_or_else(|| HostError::Invalid(format!("no boss bar {bar}")))?;
    f(bar);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offline_uuid_matches_vanilla() {
        // Known value: offline UUID of "Notch".
        assert_eq!(
            offline_uuid("Notch").to_string(),
            "b50ad385-829d-3141-a216-7e7d7539ba7f"
        );
    }

    #[test]
    fn view_slots_map_to_inventory() {
        assert_eq!(view_to_inventory(0), Some(9));
        assert_eq!(view_to_inventory(26), Some(35));
        assert_eq!(view_to_inventory(27), Some(0));
        assert_eq!(view_to_inventory(35), Some(8));
        assert_eq!(view_to_inventory(36), None);
    }
}
