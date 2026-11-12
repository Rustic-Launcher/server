//! `RusticHooks`: the one trait mod Horizon calls; each Horizon hook becomes a Rustic event
//! (docs/ARCHITECTURE.md §5.2), fired only when a plugin listens (`has_listeners_for`: an unheard
//! event costs one branch), with the fields plugins may change written back.
//!
//! **No panic reaches Horizon.** A panic in a hook on a dedicated server crash-stops it
//! (`game/dedicated.rs`); every hook body here runs in [`guard`], which catches the panic, logs it
//! and answers as if Rustic had not been there. Plugin panics and traps are already contained by
//! the core (the plugin is disabled); this guard is for the adapter itself.
//! `RUSTIC_TEST_PANIC=<hook>` makes that hook panic on purpose, to check it.
use std::cell::RefCell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::Instant;

use horizon_client::mod_api::{
    BlockExpEvent, BlockPlaceEvent, ChatDecorateEvent, ChunkLoadEvent, ChunkUnloadEvent,
    ConsoleLineEvent, CustomPayloadFromClientEvent, DestroyBlockEvent, EntityJoinLevelEvent,
    EntityLeaveLevelEvent, Flow, Hand, Hooks, Host, InteractEntityV2Event, InventoryClickEvent,
    LeftClickBlockEvent, LevelHost, LivingIncomingDamageV3Event, LogLineEvent, LootDropsEvent,
    MenuClickedEvent, MenuClosedEvent, PerformCommandEvent, PerformConsoleCommandEvent,
    PermissionCheckEvent, PlayerChangedDimensionEvent, PlayerChatV2Event, PlayerCommandEvent,
    PlayerContainerCloseEvent, PlayerDiedV3Event, PlayerGameModeChangedEvent,
    PlayerJoinMessageEvent, PlayerLeaveMessageEvent, PlayerLoggedInEvent, PlayerLoggedOutEvent,
    PlayerMoveEvent, PlayerRespawnPositionEvent, PlayerTeleportedEvent, RegisterCommandsEvent,
    RightClickBlockV2Event, ServerStoppingEvent, ServerTickEvent, ServerTickPhase,
    StatusResponseEvent, Suggestion, TabCompleteV2Event, Text,
};
use rusticmc_core::model::{
    BlockBreak, BlockPlace, ChunkLoad, ChunkUnload, ClickKind, CommandSender, EntityDamage,
    EntityDeath, EntityRef, EventKind, InteractAction, InventoryClick, InventoryClose, Location,
    PlayerChangeWorld, PlayerChat, PlayerCommand, PlayerDeath, PlayerGameModeChange,
    PlayerInteract, PlayerInteractEntity, PlayerJoin, PlayerMove, PlayerQuit, PlayerRespawn,
    PlayerTeleport, PluginMessage, ServerCommand, ServerListPing,
};

use crate::{commands, convert, ping, server, ui};

thread_local! {
    /// When the current server tick started (`ServerTickStart`), for the timings' MSPT.
    static TICK_START: RefCell<Option<Instant>> = const { RefCell::new(None) };
    /// Players whose `player-join` / `player-quit` fired already this session.
    static JOINED: RefCell<std::collections::HashSet<u128>> = RefCell::new(Default::default());
    /// The display a `player-chat` handler set, for `chat_decorate` right after (same thread).
    static CHAT_DISPLAY: RefCell<Option<(u128, String, Text)>> = const { RefCell::new(None) };
}

thread_local! {
    /// Hook bodies running on this thread: a panic there is caught (by [`guard`], or by the core
    /// around a plugin call), so Horizon's crash report is not written for it.
    static GUARDED: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// Puts a panic hook in front of Horizon's: a panic inside a Rustic hook body is caught, so it is
/// logged as one line instead of the crash report Horizon's hook writes for every panic; any
/// other panic goes to Horizon's hook as before. Installed once, when the runtime starts (after
/// Horizon installed its own).
pub fn install_panic_filter() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let horizon = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            if GUARDED.with(|g| g.get()) > 0 {
                let at = info
                    .location()
                    .map(|l| format!(" at {}:{}", l.file(), l.line()))
                    .unwrap_or_default();
                log::warn!(target: "rustic", "caught panic{at}: {}", panic_text(info.payload()));
            } else {
                horizon(info);
            }
        }));
    });
}

fn panic_text(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_owned())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "a panic".into())
}

/// Runs a hook body; a panic is logged and `fallback` answered.
fn guard<R>(hook: &'static str, fallback: R, body: impl FnOnce() -> R) -> R {
    struct Depth;
    impl Drop for Depth {
        fn drop(&mut self) {
            GUARDED.with(|g| g.set(g.get().saturating_sub(1)));
        }
    }
    GUARDED.with(|g| g.set(g.get() + 1));
    let _depth = Depth;
    match catch_unwind(AssertUnwindSafe(|| {
        if std::env::var("RUSTIC_TEST_PANIC").is_ok_and(|h| h == hook) {
            panic!("RUSTIC_TEST_PANIC={hook}: a deliberate panic in the adapter");
        }
        body()
    })) {
        Ok(r) => r,
        Err(payload) => {
            let what = panic_text(payload.as_ref());
            log::error!(target: "rustic", "hook {hook} panicked, ignored (the server goes on): {what}");
            fallback
        }
    }
}

fn flow(cancel: bool) -> Flow {
    if cancel { Flow::Cancel } else { Flow::Continue }
}

/// Vanilla's `ContainerClick` mode and button as Rustic's click kind.
fn click_kind(mode: i32, button: i32) -> ClickKind {
    match (mode, button) {
        (0, 0) => ClickKind::Left,
        (0, 1) => ClickKind::Right,
        (1, 0) => ClickKind::ShiftLeft,
        (1, 1) => ClickKind::ShiftRight,
        (2, _) => ClickKind::NumberKey,
        (3, _) => ClickKind::Middle,
        (4, 0) => ClickKind::Drop,
        (4, 1) => ClickKind::ControlDrop,
        (6, _) => ClickKind::DoubleClick,
        _ => ClickKind::Other,
    }
}

fn face(normal: [i32; 3]) -> Option<rusticmc_core::model::BlockFace> {
    use rusticmc_core::model::BlockFace;
    Some(match normal {
        [0, -1, 0] => BlockFace::Down,
        [0, 1, 0] => BlockFace::Up,
        [0, 0, -1] => BlockFace::North,
        [0, 0, 1] => BlockFace::South,
        [-1, 0, 0] => BlockFace::West,
        [1, 0, 0] => BlockFace::East,
        _ => return None,
    })
}

/// `player-interact` for a click on a block, as the acting player (their state is swapped in).
fn interact(
    host: &mut dyn Host,
    action: InteractAction,
    pos: Option<[i32; 3]>,
    normal: Option<[i32; 3]>,
    hand: Hand,
) -> Flow {
    let Some(rustic) = server::runtime() else {
        return Flow::Continue;
    };
    if !rustic.has_listeners_for(EventKind::PlayerInteract) {
        return Flow::Continue;
    }
    let Some(player) = host.acting_player() else {
        return Flow::Continue;
    };
    let item = match hand {
        Hand::Off => {
            host.player_equipment(player.uuid, horizon_client::mod_api::EquipmentSlot::OffHand)
        }
        _ => host.held_stack(player.uuid),
    };
    let interact = PlayerInteract {
        player: convert::uuid(player.uuid),
        action,
        hand: convert::hand(Some(hand)),
        item: convert::stack_to_core(item.as_ref()),
        world: player.dimension.clone(),
        block: pos.map(convert::block_pos),
        face: normal.and_then(face),
        cancelled: false,
    };
    let fired = server::enter(Some(host), |r, b| r.fire_player_interact(interact, b));
    flow(fired.is_some_and(|e| e.cancelled))
}

type ItemStackCore = rusticmc_core::model::ItemStack;

/// The trait mod.
pub struct RusticHooks;

impl RusticHooks {
    pub fn new() -> Self {
        Self
    }
}

impl Hooks for RusticHooks {
    // ---- lifecycle, scheduler -----------------------------------------------------------
    fn server_tick(&self, host: &mut dyn Host, event: &mut ServerTickEvent<'_>) {
        guard("server_tick", (), || match event.phase {
            ServerTickPhase::ServerTickStart { .. } => {
                server::start(host);
                server::refresh(host);
                TICK_START.with(|t| *t.borrow_mut() = Some(Instant::now()));
            }
            ServerTickPhase::ServerTickEnd { .. } => {
                server::flush(host);
                server::enter(Some(host), |rustic, backend| {
                    for (menu, player) in ui::take_closed() {
                        let close = InventoryClose {
                            player: convert::uuid(player),
                            menu: Some(menu),
                        };
                        rustic.fire_inventory_close(close, backend);
                    }
                    ping::serve(rustic, backend);
                    rustic.tick(backend);
                    if let Some(start) = TICK_START.with(|t| t.borrow_mut().take()) {
                        rustic.record_server_tick(start.elapsed());
                    }
                });
                ui::end_tick();
            }
            _ => {}
        });
    }

    fn server_stopping(&self, host: &mut dyn Host, _event: &mut ServerStoppingEvent) {
        guard("server_stopping", (), || server::stop(host));
    }

    // ---- commands ----------------------------------------------------------------------
    fn register_commands(&self, host: &mut dyn Host, event: &mut RegisterCommandsEvent<'_>) {
        guard("register_commands", (), || {
            server::start(host);
            if let Some(rustic) = server::runtime() {
                let mine = commands::native_commands(&rustic);
                log::info!(target: "rustic", "registering {} command usage(s) with Horizon", mine.len());
                event.commands.extend(mine);
            }
        });
    }

    fn perform_command(&self, host: &mut dyn Host, event: &mut PerformCommandEvent<'_>) -> Flow {
        guard("perform_command", Flow::Continue, || {
            let Some(rustic) = server::runtime() else {
                return Flow::Continue;
            };
            if !commands::owns(&rustic, event.command) {
                return Flow::Continue;
            }
            let sender = CommandSender::Player(convert::uuid(event.player.uuid));
            let outcome = server::enter(Some(host), |rustic, backend| {
                rustic.dispatch_command(sender, event.command, backend)
            });
            flow(outcome.is_some_and(|o| commands::ran(&o)))
        })
    }

    fn perform_console_command(
        &self,
        host: &mut dyn Host,
        event: &mut PerformConsoleCommandEvent<'_>,
    ) -> Flow {
        guard("perform_console_command", Flow::Continue, || {
            let Some(rustic) = server::runtime() else {
                return Flow::Continue;
            };
            if !commands::owns(&rustic, event.command) {
                return Flow::Continue;
            }
            let outcome = server::enter(Some(host), |rustic, backend| {
                rustic.dispatch_command(CommandSender::Console, event.command, backend)
            });
            flow(outcome.is_some_and(|o| commands::ran(&o)))
        })
    }

    fn tab_complete_v2(&self, host: &mut dyn Host, event: &mut TabCompleteV2Event<'_>) {
        guard("tab_complete_v2", (), || {
            let Some(rustic) = server::runtime() else {
                return;
            };
            if !commands::owns(&rustic, event.label) {
                return;
            }
            let mut line = event.label.to_owned();
            for arg in event.arguments {
                line.push(' ');
                line.push_str(arg);
            }
            let sender = CommandSender::Player(convert::uuid(event.player));
            let range = (event.range[0], event.range[1]);
            if let Some(words) = server::enter(Some(host), |rustic, backend| {
                rustic.complete(sender, &line, backend)
            }) {
                event
                    .suggestions
                    .extend(words.into_iter().map(|text| Suggestion {
                        range,
                        text,
                        tooltip: None,
                        int_value: None,
                    }));
            }
        });
    }

    fn permission_check(
        &self,
        _host: &dyn Host,
        event: &mut PermissionCheckEvent<'_>,
    ) -> Option<bool> {
        guard("permission_check", None, || {
            let sender = CommandSender::Player(convert::uuid(event.player));
            // `&dyn Host` here: the no-host backend (the snapshot) answers op status.
            server::enter(None, |rustic, backend| {
                rustic.has_permission(sender, event.node, backend)
            })
        })
    }

    // ---- players -----------------------------------------------------------------------
    fn player_join_message(&self, host: &mut dyn Host, event: &mut PlayerJoinMessageEvent<'_>) {
        guard("player_join_message", (), || {
            JOINED.with(|j| j.borrow_mut().insert(event.player));
            let Some(rustic) = server::runtime() else {
                return;
            };
            if !rustic.has_listeners_for(EventKind::PlayerJoin) {
                return;
            }
            let join = PlayerJoin {
                player: convert::uuid(event.player),
                name: event.name.to_owned(),
                join_message: event.line.as_ref().map(convert::from_horizon),
            };
            let before = join.join_message.clone();
            if let Some(join) = server::enter(Some(host), |r, b| r.fire_player_join(join, b))
                && join.join_message != before
            {
                *event.line = join.join_message.as_ref().map(convert::to_horizon);
            }
        });
    }

    fn player_logged_in(&self, host: &mut dyn Host, event: &mut PlayerLoggedInEvent<'_>) {
        guard("player_logged_in", (), || {
            log::info!(target: "rustic", "player {} joined (uuid {:032x})", event.name, event.player);
            // A player Horizon did not ask `player_join_message` for (a proxy's hand-over): join
            // now, with no line to change.
            if JOINED.with(|j| j.borrow_mut().insert(event.player))
                && let Some(rustic) = server::runtime()
                && rustic.has_listeners_for(EventKind::PlayerJoin)
            {
                let join = PlayerJoin {
                    player: convert::uuid(event.player),
                    name: event.name.to_owned(),
                    join_message: None,
                };
                server::enter(Some(host), |r, b| r.fire_player_join(join, b));
            }
        });
    }

    fn player_leave_message(&self, host: &mut dyn Host, event: &mut PlayerLeaveMessageEvent<'_>) {
        guard("player_leave_message", (), || {
            JOINED.with(|j| j.borrow_mut().remove(&event.player));
            let Some(rustic) = server::runtime() else {
                return;
            };
            if !rustic.has_listeners_for(EventKind::PlayerQuit) {
                return;
            }
            let quit = PlayerQuit {
                player: convert::uuid(event.player),
                name: event.name.to_owned(),
                quit_message: event.line.as_ref().map(convert::from_horizon),
            };
            let before = quit.quit_message.clone();
            if let Some(quit) = server::enter(Some(host), |r, b| r.fire_player_quit(quit, b))
                && quit.quit_message != before
            {
                *event.line = quit.quit_message.as_ref().map(convert::to_horizon);
            }
        });
    }

    fn player_logged_out(&self, host: &mut dyn Host, event: &mut PlayerLoggedOutEvent<'_>) {
        guard("player_logged_out", (), || {
            log::info!(target: "rustic", "player {} left", event.name);
            ui::player_left(event.player);
            // Left without the line (moved to another server unannounced): quit now.
            if JOINED.with(|j| j.borrow_mut().remove(&event.player))
                && let Some(rustic) = server::runtime()
                && rustic.has_listeners_for(EventKind::PlayerQuit)
            {
                let quit = PlayerQuit {
                    player: convert::uuid(event.player),
                    name: event.name.to_owned(),
                    quit_message: None,
                };
                server::enter(Some(host), |r, b| r.fire_player_quit(quit, b));
            }
        });
    }

    fn player_chat_v2(&self, event: &mut PlayerChatV2Event<'_>) -> Flow {
        guard("player_chat_v2", Flow::Continue, || {
            let Some(rustic) = server::runtime() else {
                return Flow::Continue;
            };
            if !rustic.has_listeners_for(EventKind::PlayerChat) {
                return Flow::Continue;
            }
            let chat = PlayerChat {
                player: convert::uuid(event.player),
                name: event.name.to_owned(),
                message: event.message.to_owned(),
                cancelled: false,
                display: None,
            };
            // No host in this hook: reads come from the tick's snapshot, writes wait for the tick.
            let Some(chat) = server::enter(None, |r, b| r.fire_player_chat(chat, b)) else {
                return Flow::Continue;
            };
            if let Some(display) = chat.display.filter(|_| !chat.cancelled) {
                let shown = convert::to_horizon(&display);
                CHAT_DISPLAY.with(|d| *d.borrow_mut() = Some((event.player, chat.message, shown)));
            }
            flow(chat.cancelled)
        })
    }

    fn chat_decorate(&self, event: &mut ChatDecorateEvent<'_>) -> Option<Text> {
        guard("chat_decorate", None, || {
            CHAT_DISPLAY.with(|d| {
                let mut d = d.borrow_mut();
                match d.take() {
                    Some((player, message, shown))
                        if player == event.player && message == event.message =>
                    {
                        Some(shown)
                    }
                    _ => None,
                }
            })
        })
    }

    fn player_command(&self, host: &mut dyn Host, event: &mut PlayerCommandEvent<'_>) -> Flow {
        guard("player_command", Flow::Continue, || {
            let Some(rustic) = server::runtime() else {
                return Flow::Continue;
            };
            if !rustic.has_listeners_for(EventKind::PlayerCommand) {
                return Flow::Continue;
            }
            let command = PlayerCommand {
                player: convert::uuid(event.player),
                name: event.name.to_owned(),
                command: event.command.to_owned(),
                cancelled: false,
            };
            let fired = server::enter(Some(host), |r, b| r.fire_player_command(command, b));
            flow(fired.is_some_and(|c| c.cancelled))
        })
    }

    fn console_line(&self, host: &mut dyn Host, event: &mut ConsoleLineEvent<'_>) -> Flow {
        guard("console_line", Flow::Continue, || {
            let Some(rustic) = server::runtime() else {
                return Flow::Continue;
            };
            if !rustic.has_listeners_for(EventKind::ServerCommand) {
                return Flow::Continue;
            }
            let command = ServerCommand {
                command: event.line.to_owned(),
                cancelled: false,
            };
            let fired = server::enter(Some(host), |r, b| r.fire_server_command(command, b));
            flow(fired.is_some_and(|c| c.cancelled))
        })
    }

    fn player_died_v3(&self, host: &mut dyn Host, event: &mut PlayerDiedV3Event<'_>) {
        guard("player_died_v3", (), || {
            let Some(rustic) = server::runtime() else {
                return;
            };
            if !rustic.has_listeners_for(EventKind::PlayerDeath) {
                return;
            }
            let death = PlayerDeath {
                player: convert::uuid(event.player),
                cause: String::new(),
                death_message: event.message.as_ref().map(convert::from_horizon),
                keep_inventory: event.keep_inventory,
                keep_level: event.keep_level,
            };
            let before = death.death_message.clone();
            if let Some(death) = server::enter(Some(host), |r, b| r.fire_player_death(death, b)) {
                if death.death_message != before {
                    *event.message = death.death_message.as_ref().map(convert::to_horizon);
                }
                event.keep_inventory = death.keep_inventory;
                event.keep_level = death.keep_level;
            }
        });
    }

    // ---- blocks ------------------------------------------------------------------------
    fn destroy_block(&self, host: &mut dyn Host, event: &mut DestroyBlockEvent) -> Flow {
        guard("destroy_block", Flow::Continue, || {
            let Some(rustic) = server::runtime() else {
                return Flow::Continue;
            };
            if !rustic.has_listeners_for(EventKind::BlockBreak) {
                return Flow::Continue;
            }
            let Some(player) = host.acting_player() else {
                log::debug!(target: "rustic", "destroy_block at {:?} with no acting player", event.pos);
                return Flow::Continue;
            };
            let block = convert::block_state(&host.block(event.pos));
            let dimension = player.dimension.clone();
            let fired = server::enter(Some(host), |r, b| {
                r.fire_block_break(
                    BlockBreak {
                        player: convert::uuid(player.uuid),
                        world: player.dimension.clone(),
                        pos: convert::block_pos(event.pos),
                        block,
                        cancelled: false,
                        drop_items: true,
                    },
                    b,
                )
            });
            if let Some(e) = &fired
                && !e.cancelled
                && !e.drop_items
            {
                ui::refuse_drops(&dimension, event.pos);
            }
            flow(fired.is_some_and(|e| e.cancelled))
        })
    }

    fn block_place(&self, host: &mut dyn Host, event: &mut BlockPlaceEvent<'_>) -> Flow {
        guard("block_place", Flow::Continue, || {
            let Some(rustic) = server::runtime() else {
                return Flow::Continue;
            };
            if !rustic.has_listeners_for(EventKind::BlockPlace) {
                return Flow::Continue;
            }
            let world = host
                .acting_player()
                .map(|p| p.dimension)
                .unwrap_or_default();
            let hand = convert::hand(host.acting_hand());
            let place = BlockPlace {
                player: convert::uuid(event.player),
                world,
                pos: convert::block_pos(event.pos),
                // The state about to be set, by name (patch U12); the item's id if unknown.
                block: convert::block_state(
                    &host
                        .block_state_name(event.state)
                        .unwrap_or_else(|| event.stack.item.to_string()),
                ),
                hand,
                cancelled: false,
            };
            let fired = server::enter(Some(host), |r, b| r.fire_block_place(place, b));
            flow(fired.is_some_and(|e| e.cancelled))
        })
    }

    // ---- drops ---------------------------------------------------------------------------
    /// A block break a plugin refused the drops of (`drop-items = false`): its loot goes. An
    /// entity's loot is the `entity-death` event's `drops`, which plugins may change.
    fn loot_drops(&self, host: &mut dyn Host, event: &mut LootDropsEvent<'_>) {
        guard("loot_drops", (), || {
            let context = event.context;
            if let Some((pos, _)) = context.block {
                if ui::drops_refused(context.dimension, pos) {
                    event.drops.clear();
                }
                return;
            }
            let Some((id, kind)) = context.entity else {
                return;
            };
            if kind == "minecraft:player" {
                // A player's death drops are `player-death`'s (keep-inventory).
                return;
            }
            let Some(rustic) = server::runtime() else {
                return;
            };
            if !rustic.has_listeners_for(EventKind::EntityDeath) {
                return;
            }
            let uuid = host
                .entity(id)
                .and_then(|e| host.entity_uuid(e))
                .unwrap_or(0);
            let drops = event
                .drops
                .iter()
                .map(|s| convert::stack_to_core(Some(s)))
                .collect();
            let death = EntityDeath {
                entity: EntityRef {
                    id,
                    uuid: convert::uuid(uuid),
                    kind: kind.to_owned(),
                },
                killer: context.player.map(convert::uuid),
                drops,
                dropped_exp: 0,
            };
            let before = death.drops.clone();
            if let Some(death) = server::enter(Some(host), |r, b| r.fire_entity_death(death, b))
                && death.drops != before
            {
                *event.drops = death
                    .drops
                    .iter()
                    .filter_map(|s| convert::stack_to_horizon(host, s))
                    .collect();
            }
        });
    }

    fn block_exp(&self, _level: &dyn LevelHost, event: &mut BlockExpEvent<'_>) {
        guard("block_exp", (), || {
            let pos = [event.pos.x, event.pos.y, event.pos.z];
            let dimension = match event.dimension {
                horizon_client::mod_api::LevelDimension::Overworld => "minecraft:overworld",
                horizon_client::mod_api::LevelDimension::Nether => "minecraft:the_nether",
                horizon_client::mod_api::LevelDimension::End => "minecraft:the_end",
            };
            if ui::drops_refused(dimension, pos) {
                event.experience = 0;
            }
        });
    }

    // ---- damage --------------------------------------------------------------------------
    /// `entity-damage` for every hit (`living_incoming_damage_v3`, no host): players' UUIDs come
    /// with the hook, mobs' from the id -> UUID table `entity_join_level` keeps while a plugin
    /// listens. `amount` is written back, `cancelled` cancels the hit.
    fn living_incoming_damage_v3(&self, event: &mut LivingIncomingDamageV3Event<'_>) -> Flow {
        guard("living_incoming_damage_v3", Flow::Continue, || {
            let Some(rustic) = server::runtime() else {
                return Flow::Continue;
            };
            if !rustic.has_listeners_for(EventKind::EntityDamage) {
                return Flow::Continue;
            }
            let attacker = event.attacker.map(|id| {
                let (uuid, kind) = match event.attacker_uuid {
                    Some(u) => (u, "minecraft:player".to_owned()),
                    None => ui::entity(id).unwrap_or((0, String::new())),
                };
                EntityRef {
                    id,
                    uuid: convert::uuid(uuid),
                    kind,
                }
            });
            let uuid = event
                .victim_uuid
                .or_else(|| ui::entity(event.victim).map(|(u, _)| u))
                .unwrap_or(0);
            let damage = EntityDamage {
                target: EntityRef {
                    id: event.victim,
                    uuid: convert::uuid(uuid),
                    kind: event.kind.to_owned(),
                },
                cause: event.damage_type.to_owned(),
                attacker,
                amount: event.amount,
                cancelled: false,
            };
            // No host here: reads come from the tick's snapshot, writes wait for the tick.
            match server::enter(None, |r, b| r.fire_entity_damage(damage, b)) {
                Some(d) if d.cancelled => Flow::Cancel,
                Some(d) => {
                    if d.amount.is_finite() && d.amount >= 0.0 {
                        event.amount = d.amount;
                    }
                    Flow::Continue
                }
                None => Flow::Continue,
            }
        })
    }

    /// Keeps the UUID of each entity that joins a level while a plugin listens to damage, for the
    /// no-host damage hook.
    fn entity_join_level(&self, host: &mut dyn Host, event: &mut EntityJoinLevelEvent<'_>) -> Flow {
        guard("entity_join_level", Flow::Continue, || {
            if !server::runtime().is_some_and(|r| r.has_listeners_for(EventKind::EntityDamage))
                || matches!(
                    event.kind,
                    "minecraft:item" | "minecraft:experience_orb" | "minecraft:arrow"
                )
            {
                return Flow::Continue;
            }
            if let Some(uuid) = host.entity(event.entity).and_then(|e| host.entity_uuid(e)) {
                ui::entity_joined(event.entity, uuid, event.kind);
            }
            Flow::Continue
        })
    }

    fn entity_leave_level(&self, _host: &mut dyn Host, event: &mut EntityLeaveLevelEvent<'_>) {
        guard("entity_leave_level", (), || ui::entity_left(event.entity));
    }

    // ---- menus ---------------------------------------------------------------------------
    /// A click in a plugin's menu. The core cancels it by default (`fire`); Horizon's own
    /// menus never move items anyway.
    fn menu_clicked(&self, host: &mut dyn Host, event: &mut MenuClickedEvent) {
        guard("menu_clicked", (), || {
            let Some((id, menu)) = ui::menu_at(event.uuid, event.window) else {
                return;
            };
            if !server::runtime().is_some_and(|r| r.has_listeners_for(EventKind::InventoryClick)) {
                return;
            }
            let item = usize::try_from(event.slot)
                .ok()
                .and_then(|i| menu.items.get(i).cloned().flatten());
            let cursor = host.menu_cursor(event.uuid);
            let click = InventoryClick {
                player: convert::uuid(event.uuid),
                menu: Some(id),
                slot: event.slot,
                click: click_kind(event.mode, event.button),
                hotbar_button: if event.mode == 2 { event.button } else { -1 },
                item: convert::stack_to_core(item.as_ref()),
                cursor: convert::stack_to_core(cursor.as_ref()),
                cancelled: false,
            };
            server::enter(Some(host), |r, b| r.fire_inventory_click(click, b));
        });
    }

    fn menu_closed(&self, host: &mut dyn Host, event: &mut MenuClosedEvent) {
        guard("menu_closed", (), || {
            let Some((id, _)) = ui::menu_at(event.uuid, event.window) else {
                return;
            };
            ui::forget_menu(id);
            ui::note_menu_closed(event.uuid);
            let close = InventoryClose {
                player: convert::uuid(event.uuid),
                menu: Some(id),
            };
            server::enter(Some(host), |r, b| r.fire_inventory_close(close, b));
        });
    }

    /// A click in one of the game's own menus (a chest, the inventory): cancellable.
    fn inventory_click(&self, host: &mut dyn Host, event: &mut InventoryClickEvent<'_>) -> Flow {
        guard("inventory_click", Flow::Continue, || {
            let Some(rustic) = server::runtime() else {
                return Flow::Continue;
            };
            if !rustic.has_listeners_for(EventKind::InventoryClick) {
                return Flow::Continue;
            }
            let item = usize::try_from(event.slot)
                .ok()
                .and_then(|i| host.menu_stacks()?.get(i).cloned().flatten());
            let click = InventoryClick {
                player: convert::uuid(event.player),
                menu: None,
                slot: event.slot,
                click: click_kind(event.mode, event.button),
                hotbar_button: if event.mode == 2 { event.button } else { -1 },
                item: convert::stack_to_core(item.as_ref()),
                cursor: ItemStackCore::empty(),
                cancelled: false,
            };
            let fired = server::enter(Some(host), |r, b| r.fire_inventory_click(click, b));
            flow(fired.is_some_and(|c| c.cancelled))
        })
    }

    fn player_container_close(
        &self,
        host: &mut dyn Host,
        event: &mut PlayerContainerCloseEvent<'_>,
    ) {
        guard("player_container_close", (), || {
            if !server::runtime().is_some_and(|r| r.has_listeners_for(EventKind::InventoryClose))
                || ui::is_plugin_menu_close(event.player)
            {
                return;
            }
            let close = InventoryClose {
                player: convert::uuid(event.player),
                menu: None,
            };
            server::enter(Some(host), |r, b| r.fire_inventory_close(close, b));
        });
    }

    // ---- players (more) ------------------------------------------------------------------
    fn player_move(&self, event: &mut PlayerMoveEvent<'_>) -> Flow {
        guard("player_move", Flow::Continue, || {
            let Some(rustic) = server::runtime() else {
                return Flow::Continue;
            };
            if !rustic.has_listeners_for(EventKind::PlayerMove) {
                return Flow::Continue;
            }
            let [fx, fy, fz] = event.from;
            let [tx, ty, tz] = *event.to;
            let threshold = rustic.config().events.move_threshold;
            let moved = (tx - fx).powi(2) + (ty - fy).powi(2) + (tz - fz).powi(2);
            if moved < threshold * threshold {
                return Flow::Continue;
            }
            let world = server::snapshot()
                .iter()
                .find(|p| p.uuid == event.player)
                .map(|p| p.dimension.clone())
                .unwrap_or_default();
            let at = |[x, y, z]: [f64; 3]| Location {
                world: world.clone(),
                x,
                y,
                z,
                yaw: event.yaw,
                pitch: event.pitch,
            };
            let mv = PlayerMove {
                player: convert::uuid(event.player),
                from: at(event.from),
                to: at(*event.to),
                cancelled: false,
            };
            // No host here: reads come from the tick's snapshot, writes wait for the tick.
            match server::enter(None, |r, b| r.fire_player_move(mv, b)) {
                Some(m) if m.cancelled => Flow::Cancel,
                Some(m) => {
                    if m.to.is_finite() {
                        *event.to = [m.to.x, m.to.y, m.to.z];
                    }
                    Flow::Continue
                }
                None => Flow::Continue,
            }
        })
    }

    fn player_teleported(&self, host: &mut dyn Host, event: &mut PlayerTeleportedEvent<'_>) {
        guard("player_teleported", (), || {
            if !server::runtime().is_some_and(|r| r.has_listeners_for(EventKind::PlayerTeleport)) {
                return;
            }
            let at = |[x, y, z]: [f64; 3]| Location {
                world: event.dimension.to_owned(),
                x,
                y,
                z,
                yaw: 0.0,
                pitch: 0.0,
            };
            let tp = PlayerTeleport {
                player: convert::uuid(event.player),
                from: at(event.from),
                to: at(event.to),
            };
            server::enter(Some(host), |r, b| r.fire_player_teleport(tp, b));
        });
    }

    fn player_changed_dimension(
        &self,
        host: &mut dyn Host,
        event: &mut PlayerChangedDimensionEvent<'_>,
    ) {
        guard("player_changed_dimension", (), || {
            if !server::runtime().is_some_and(|r| r.has_listeners_for(EventKind::PlayerChangeWorld))
            {
                return;
            }
            let change = PlayerChangeWorld {
                player: convert::uuid(event.player),
                from_world: event.from.to_owned(),
                to_world: event.to.to_owned(),
            };
            server::enter(Some(host), |r, b| r.fire_player_change_world(change, b));
        });
    }

    fn player_respawn_position(
        &self,
        host: &mut dyn Host,
        event: &mut PlayerRespawnPositionEvent<'_>,
    ) {
        guard("player_respawn_position", (), || {
            if !server::runtime().is_some_and(|r| r.has_listeners_for(EventKind::PlayerRespawn)) {
                return;
            }
            let [x, y, z] = *event.pos;
            let respawn = PlayerRespawn {
                player: convert::uuid(event.player),
                location: Location {
                    world: event.dimension.clone(),
                    x,
                    y,
                    z,
                    yaw: event.yaw,
                    pitch: event.pitch,
                },
            };
            if let Some(r) = server::enter(Some(host), |r, b| r.fire_player_respawn(respawn, b))
                && r.location.is_finite()
            {
                *event.dimension = r.location.world;
                *event.pos = [r.location.x, r.location.y, r.location.z];
                event.yaw = r.location.yaw;
                event.pitch = r.location.pitch;
            }
        });
    }

    fn player_game_mode_changed(
        &self,
        host: &mut dyn Host,
        event: &mut PlayerGameModeChangedEvent<'_>,
    ) -> Flow {
        guard("player_game_mode_changed", Flow::Continue, || {
            if !server::runtime()
                .is_some_and(|r| r.has_listeners_for(EventKind::PlayerGameModeChange))
            {
                return Flow::Continue;
            }
            let change = PlayerGameModeChange {
                player: convert::uuid(event.player),
                old_mode: convert::game_mode(event.from),
                new_mode: convert::game_mode(event.to),
                cancelled: false,
            };
            let fired = server::enter(Some(host), |r, b| r.fire_player_game_mode_change(change, b));
            flow(fired.is_some_and(|c| c.cancelled))
        })
    }

    fn right_click_block_v2(
        &self,
        host: &mut dyn Host,
        event: &mut RightClickBlockV2Event,
    ) -> Flow {
        guard("right_click_block_v2", Flow::Continue, || {
            interact(
                host,
                InteractAction::RightClickBlock,
                Some(event.pos),
                Some(event.face),
                event.hand,
            )
        })
    }

    fn left_click_block(&self, host: &mut dyn Host, event: &mut LeftClickBlockEvent) -> Flow {
        guard("left_click_block", Flow::Continue, || {
            interact(
                host,
                InteractAction::LeftClickBlock,
                Some(event.pos),
                Some(event.face),
                Hand::Main,
            )
        })
    }

    fn interact_entity_v2(
        &self,
        host: &mut dyn Host,
        event: &mut InteractEntityV2Event<'_>,
    ) -> Flow {
        guard("interact_entity_v2", Flow::Continue, || {
            let Some(rustic) = server::runtime() else {
                return Flow::Continue;
            };
            if !rustic.has_listeners_for(EventKind::PlayerInteractEntity) {
                return Flow::Continue;
            }
            let Some(player) = host.acting_player() else {
                return Flow::Continue;
            };
            let Ok(id) = u32::try_from(event.entity) else {
                return Flow::Continue;
            };
            let uuid = host
                .entity(id)
                .and_then(|e| host.entity_uuid(e))
                .unwrap_or(0);
            let interact = PlayerInteractEntity {
                player: convert::uuid(player.uuid),
                target: EntityRef {
                    id,
                    uuid: convert::uuid(uuid),
                    kind: event.kind.to_owned(),
                },
                hand: convert::hand(Some(event.hand)),
                cancelled: false,
            };
            let fired = server::enter(Some(host), |r, b| {
                r.fire_player_interact_entity(interact, b)
            });
            flow(fired.is_some_and(|e| e.cancelled))
        })
    }

    fn custom_payload_from_client(
        &self,
        host: &mut dyn Host,
        event: &mut CustomPayloadFromClientEvent<'_>,
    ) {
        guard("custom_payload_from_client", (), || {
            if !server::runtime().is_some_and(|r| r.has_listeners_for(EventKind::PluginMessage)) {
                return;
            }
            let message = PluginMessage {
                player: convert::uuid(event.player),
                channel: event.channel.to_owned(),
                data: event.data.to_vec(),
            };
            server::enter(Some(host), |r, b| r.fire_plugin_message(message, b));
        });
    }

    fn chunk_load(&self, host: &mut dyn Host, event: &mut ChunkLoadEvent<'_>) {
        guard("chunk_load", (), || {
            if !server::runtime().is_some_and(|r| r.has_listeners_for(EventKind::ChunkLoad)) {
                return;
            }
            let load = ChunkLoad {
                world: event.dimension.to_owned(),
                x: event.chunk[0],
                z: event.chunk[1],
                new_chunk: event.new,
            };
            server::enter(Some(host), |r, b| r.fire_chunk_load(load, b));
        });
    }

    fn chunk_unload(&self, host: &mut dyn Host, event: &mut ChunkUnloadEvent<'_>) {
        guard("chunk_unload", (), || {
            if !server::runtime().is_some_and(|r| r.has_listeners_for(EventKind::ChunkUnload)) {
                return;
            }
            let unload = ChunkUnload {
                world: event.dimension.to_owned(),
                x: event.chunk[0],
                z: event.chunk[1],
            };
            server::enter(Some(host), |r, b| r.fire_chunk_unload(unload, b));
        });
    }

    // ---- logging -------------------------------------------------------------------------
    /// Horizon's logger writes only its own targets to the console (stdout); Rustic's lines and the
    /// plugins' (target `rustic`, `[rustic] ...` in the file) are echoed here, as the file has them,
    /// so the console, `rustic run` and the supervisor's panel show them. On the log's writer thread.
    fn log_line(&self, event: &mut LogLineEvent<'_>) {
        guard("log_line", (), || {
            if event.message.starts_with("[rustic]") {
                use std::io::Write;
                let mut out = std::io::stdout().lock();
                let _ = writeln!(out, "{}", event.line.trim_end());
                let _ = out.flush();
            }
        });
    }

    // ---- connections -------------------------------------------------------------------
    fn status_response(&self, event: &mut StatusResponseEvent<'_>) {
        guard("status_response", (), || {
            let asked = ServerListPing {
                address: event.address.ip().to_string(),
                motd: ping::motd_text(event.motd),
                max_players: event.max_players.max(0) as u32,
                online_players: event.online.max(0) as u32,
                show_players: !event.sample.is_empty(),
            };
            let before = asked.clone();
            let Some(answer) = ping::ask(asked) else {
                return;
            };
            if answer.motd != before.motd {
                *event.description = Some(convert::to_horizon(&answer.motd));
            }
            event.max_players = i32::try_from(answer.max_players).unwrap_or(i32::MAX);
            if !answer.show_players {
                event.sample.clear();
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_panicking_hook_body_answers_the_fallback() {
        assert_eq!(guard("test_hook", 7, || -> i32 { panic!("inside") }), 7);
        assert_eq!(guard("test_hook", 7, || 3), 3);
        assert!(matches!(
            guard("test_hook", Flow::Continue, || -> Flow { panic!("x") }),
            Flow::Continue
        ));
    }
}
