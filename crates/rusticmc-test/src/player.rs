//! [`MockPlayer`]: a simulated client. Its actions fire the same events the real server fires
//! and apply the outcome the same way; everything it received is recorded for assertions.

use crate::harness::{CommandRun, Harness};
use crate::state::{
    BarState, PlayerState, Received, SoundRecord, TitleRecord, namespaced, parse_block,
};
use crate::world::{IntoBlockPos, MockWorld};
use rusticmc_core::model::{
    BlockFace, ClickKind, CommandSender, EntityRef, GameMode, Hand, InteractAction, ItemStack,
    Location, MenuId, Text, Uuid,
};
use serde::de::DeserializeOwned;
use std::rc::Rc;

/// A player of a [`crate::MockServer`]. Cheap to clone; stays valid after the player quits
/// (its recordings remain readable).
#[derive(Clone)]
pub struct MockPlayer {
    pub(crate) h: Rc<Harness>,
    pub(crate) uuid: Uuid,
    pub(crate) name: String,
}

impl std::fmt::Debug for MockPlayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MockPlayer")
            .field("name", &self.name)
            .field("uuid", &self.uuid.to_string())
            .finish()
    }
}

impl PartialEq for MockPlayer {
    fn eq(&self, other: &Self) -> bool {
        self.uuid == other.uuid && Rc::ptr_eq(&self.h, &other.h)
    }
}

/// A plugin menu as the player sees it.
#[derive(Clone, Debug, PartialEq)]
pub struct MenuView {
    /// Menu id.
    pub id: MenuId,
    /// Rows of 9.
    pub rows: u8,
    /// Title.
    pub title: Text,
    /// Items by slot.
    pub items: std::collections::BTreeMap<u32, ItemStack>,
}

impl MenuView {
    /// The item in a slot (empty stack if none).
    pub fn item(&self, slot: u32) -> ItemStack {
        self.items
            .get(&slot)
            .cloned()
            .unwrap_or_else(ItemStack::empty)
    }

    /// The plain title.
    pub fn title_plain(&self) -> String {
        self.title.to_plain()
    }
}

impl MockPlayer {
    fn state(&self) -> PlayerState {
        self.h
            .player_state(self.uuid)
            .expect("the player exists in the mock server")
    }

    fn with_mut<T>(&self, f: impl FnOnce(&mut PlayerState) -> T) -> T {
        let mut st = self.h.state.borrow_mut();
        f(st.players.get_mut(&self.uuid).expect("player exists"))
    }

    // ------------------------------------------------------------------ identity

    /// UUID (offline-mode UUID of the name).
    pub fn uuid(&self) -> Uuid {
        self.uuid
    }

    /// Name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// As a command sender.
    pub fn sender(&self) -> CommandSender {
        CommandSender::Player(self.uuid)
    }

    /// The player's entity.
    pub fn entity(&self) -> EntityRef {
        self.state().entity_ref()
    }

    /// Whether connected.
    pub fn is_online(&self) -> bool {
        self.state().online
    }

    // ------------------------------------------------------------------ actions (events)

    /// Send a chat message: returns whether it was broadcast (false: a handler cancelled it).
    pub fn chat(&self, message: &str) -> bool {
        self.h.chat(self.uuid, message)
    }

    /// Type a command (`"/hello Steve"` or without slash): fires `player-command`, then runs it.
    pub fn command(&self, line: &str) -> CommandRun {
        self.h.command(self.sender(), line)
    }

    /// Alias of [`MockPlayer::command`].
    pub fn perform_command(&self, line: &str) -> CommandRun {
        self.command(line)
    }

    /// Tab-complete a line (`"home "` completes the first argument).
    pub fn complete(&self, line: &str) -> Vec<String> {
        self.h.op(|| {
            self.h
                .rt
                .complete(self.sender(), line, &mut self.h.backend())
        })
    }

    /// Walk to a location (fires `player-move` when beyond the move threshold): false when a
    /// handler cancelled it and the player was put back.
    pub fn move_to(&self, to: Location) -> bool {
        self.h.move_to(self.uuid, to)
    }

    /// Walk by an offset.
    pub fn move_by(&self, dx: f64, dy: f64, dz: f64) -> bool {
        let mut to = self.location();
        to.x += dx;
        to.y += dy;
        to.z += dz;
        self.move_to(to)
    }

    /// Teleport (as a command or another plugin would): fires `player-teleport` and
    /// `player-change-world` across worlds.
    pub fn teleport(&self, to: Location) -> bool {
        self.h.teleport(self.uuid, to)
    }

    /// Change game mode (fires the cancellable `player-game-mode-change`).
    pub fn set_game_mode(&self, mode: GameMode) -> bool {
        self.h.set_game_mode(self.uuid, mode)
    }

    /// Break a block of the player's world (fires `block-break`): false when cancelled or
    /// impossible (air, adventure/spectator, bedrock in survival). Survival drops the item.
    pub fn break_block(&self, pos: impl IntoBlockPos) -> bool {
        self.h.break_block(self.uuid, pos.into_block_pos())
    }

    /// Place a block (`"stone"`, `"oak_log[axis=x]"`, fires `block-place`): survival needs the
    /// item in a hand (one is used up), creative does not.
    pub fn place_block(&self, pos: impl IntoBlockPos, block: &str) -> bool {
        self.h
            .place_block(self.uuid, pos.into_block_pos(), &parse_block(block))
    }

    /// Right-click a block face with the main hand (fires `player-interact`).
    pub fn right_click_block(&self, pos: impl IntoBlockPos, face: BlockFace) -> bool {
        self.h.interact(
            self.uuid,
            InteractAction::RightClickBlock,
            Some(pos.into_block_pos()),
            Some(face),
            Hand::MainHand,
        )
    }

    /// Left-click a block face (fires `player-interact`).
    pub fn left_click_block(&self, pos: impl IntoBlockPos, face: BlockFace) -> bool {
        self.h.interact(
            self.uuid,
            InteractAction::LeftClickBlock,
            Some(pos.into_block_pos()),
            Some(face),
            Hand::MainHand,
        )
    }

    /// Right-click the air with a hand (fires `player-interact`).
    pub fn right_click_air(&self, hand: Hand) -> bool {
        self.h
            .interact(self.uuid, InteractAction::RightClickAir, None, None, hand)
    }

    /// Left-click the air (fires `player-interact`).
    pub fn left_click_air(&self) -> bool {
        self.h.interact(
            self.uuid,
            InteractAction::LeftClickAir,
            None,
            None,
            Hand::MainHand,
        )
    }

    /// Right-click an entity (fires `player-interact-entity`).
    pub fn interact_entity(&self, target: &EntityRef) -> bool {
        self.h
            .interact_entity(self.uuid, target.clone(), Hand::MainHand)
    }

    /// Take damage (fires `entity-damage`; the player dies at 0 health): the damage dealt.
    pub fn damage(&self, amount: f32) -> Option<f32> {
        self.h
            .damage_player(self.uuid, amount, "minecraft:generic", None)
    }

    /// Take damage of a type, optionally from an attacker.
    pub fn damage_by(&self, amount: f32, cause: &str, attacker: Option<EntityRef>) -> Option<f32> {
        self.h.damage_player(self.uuid, amount, cause, attacker)
    }

    /// Hit another player (cause `minecraft:player_attack`).
    pub fn attack(&self, target: &MockPlayer, amount: f32) -> Option<f32> {
        self.h.damage_player(
            target.uuid,
            amount,
            "minecraft:player_attack",
            Some(self.entity()),
        )
    }

    /// Die (fires `player-death` and applies it).
    pub fn kill(&self) {
        self.h.die(self.uuid, "minecraft:generic_kill");
    }

    /// Respawn after death (fires `player-respawn`): where the player respawned.
    pub fn respawn(&self) -> Option<Location> {
        self.h.respawn(self.uuid)
    }

    /// Disconnect (fires `player-quit`).
    pub fn disconnect(&self) {
        self.h.quit(self.uuid);
    }

    /// The plugin menu the player has open.
    pub fn open_menu(&self) -> Option<MenuView> {
        let st = self.h.state.borrow();
        let id = st.players.get(&self.uuid)?.open_menu?;
        st.menus.get(&id).map(|m| MenuView {
            id,
            rows: m.rows,
            title: m.title.clone(),
            items: m.items.clone(),
        })
    }

    /// Click a slot of the open menu (fires `inventory-click`, cancelled by default for plugin
    /// menus): true when the click went through (a handler allowed it). Slots past the menu
    /// are the player's inventory as shown below it.
    pub fn click_menu_slot(&self, slot: u32, click: ClickKind) -> bool {
        self.h.click_menu(self.uuid, slot as i32, click, -1)
    }

    /// Press a hotbar number key over a menu slot.
    pub fn number_key_menu_slot(&self, slot: u32, hotbar: u32) -> bool {
        self.h
            .click_menu(self.uuid, slot as i32, ClickKind::NumberKey, hotbar as i32)
    }

    /// Close the open menu (fires `inventory-close`): false if none was open.
    pub fn close_menu(&self) -> bool {
        self.h.close_menu(self.uuid)
    }

    /// Send a custom payload from the client (fires `plugin-message` if a plugin registered the
    /// channel): whether anyone listens.
    pub fn send_plugin_message(&self, channel: &str, data: &[u8]) -> bool {
        self.h.client_plugin_message(self.uuid, channel, data)
    }

    // ------------------------------------------------------------------ state

    /// Location.
    pub fn location(&self) -> Location {
        self.state().location
    }

    /// The world the player is in.
    pub fn world(&self) -> MockWorld {
        MockWorld {
            h: self.h.clone(),
            id: self.location().world,
        }
    }

    /// Put the player somewhere without any event (test setup).
    pub fn set_location(&self, to: Location) {
        self.with_mut(|p| p.location = to);
    }

    /// Game mode.
    pub fn game_mode(&self) -> GameMode {
        self.state().game_mode
    }

    /// Health.
    pub fn health(&self) -> f32 {
        self.state().health
    }

    /// Set health without any event (test setup); 0 does not kill (use [`MockPlayer::kill`]).
    pub fn set_health(&self, health: f32) {
        self.with_mut(|p| p.health = health.clamp(0.0, p.max_health));
    }

    /// Max health.
    pub fn max_health(&self) -> f32 {
        self.state().max_health
    }

    /// Food level.
    pub fn food(&self) -> u32 {
        self.state().food
    }

    /// Set the food level (test setup).
    pub fn set_food(&self, food: u32) {
        self.with_mut(|p| p.food = food.min(20));
    }

    /// Saturation.
    pub fn saturation(&self) -> f32 {
        self.state().saturation
    }

    /// Experience level.
    pub fn level(&self) -> u32 {
        self.state().level
    }

    /// Set the experience level (test setup).
    pub fn set_level(&self, level: u32) {
        self.with_mut(|p| p.level = level);
    }

    /// Whether flight is allowed.
    pub fn allow_flight(&self) -> bool {
        self.state().allow_flight
    }

    /// Whether flying.
    pub fn is_flying(&self) -> bool {
        self.state().flying
    }

    /// Whether dead (waiting to respawn).
    pub fn is_dead(&self) -> bool {
        self.state().dead
    }

    /// Whether an operator.
    pub fn is_op(&self) -> bool {
        self.h.state.borrow().ops.contains(&self.uuid)
    }

    /// Make (or unmake) the player an operator.
    pub fn set_op(&self, op: bool) {
        let mut st = self.h.state.borrow_mut();
        if op {
            st.ops.insert(self.uuid);
        } else {
            st.ops.remove(&self.uuid);
        }
    }

    /// Grant (`true`) or deny (`false`) a permission node in the server's permission engine.
    pub fn set_permission(&self, node: &str, value: bool) {
        self.set_permission_value(node, Some(value));
    }

    /// Remove an explicit permission node (back to defaults).
    pub fn unset_permission(&self, node: &str) {
        self.set_permission_value(node, None);
    }

    fn set_permission_value(&self, node: &str, value: Option<bool>) {
        let r = self.h.rt.with_permissions(|p| {
            let e = p.engine_mut();
            e.set_user_name(self.uuid.as_u128(), &self.name);
            e.set_user_node(self.uuid.as_u128(), node, value)
        });
        match r {
            Some(Ok(())) => {}
            Some(Err(e)) => panic!("cannot set permission `{node}`: {e}"),
            None => panic!("the permission service is busy"),
        }
    }

    /// Whether the player has a permission (same resolution as the server: provider plugin,
    /// permission engine, declared defaults, op).
    pub fn has_permission(&self, node: &str) -> bool {
        self.h.op(|| {
            self.h
                .rt
                .has_permission(self.sender(), node, &mut self.h.backend())
        })
    }

    /// Client locale (`en_us`).
    pub fn locale(&self) -> String {
        self.state().locale
    }

    /// Set the client locale.
    pub fn set_locale(&self, locale: &str) {
        self.with_mut(|p| p.locale = locale.to_owned());
    }

    /// Ping in ms.
    pub fn ping(&self) -> u32 {
        self.state().ping
    }

    /// Set the ping.
    pub fn set_ping(&self, ms: u32) {
        self.with_mut(|p| p.ping = ms);
    }

    /// IP address.
    pub fn address(&self) -> String {
        self.state().address
    }

    // ------------------------------------------------------------------ inventory

    /// All 41 slots (0-8 hotbar, 9-35 main, 36-39 armor feet..head, 40 off hand).
    pub fn inventory(&self) -> Vec<ItemStack> {
        self.state().inventory.slots
    }

    /// One slot.
    pub fn item(&self, slot: u32) -> ItemStack {
        self.state().inventory.slots[slot as usize].clone()
    }

    /// Set a slot (test setup).
    pub fn set_item(&self, slot: u32, item: ItemStack) {
        self.with_mut(|p| p.inventory.slots[slot as usize] = crate::state::normalize_item(&item));
    }

    /// Give items like the server does: returns what did not fit.
    pub fn give(&self, id: &str, count: u32) -> ItemStack {
        self.give_stack(&ItemStack::new(namespaced(id), count))
    }

    /// Give a stack (with components): returns what did not fit.
    pub fn give_stack(&self, item: &ItemStack) -> ItemStack {
        self.with_mut(|p| p.inventory.give(item))
    }

    /// How many of an item the player has (`"diamond"` or `"minecraft:diamond"`).
    pub fn count(&self, id: &str) -> u32 {
        self.state().inventory.count(id)
    }

    /// The selected hotbar slot.
    pub fn held_slot(&self) -> u32 {
        self.state().held_slot
    }

    /// Select a hotbar slot (0-8).
    pub fn set_held_slot(&self, slot: u32) {
        assert!(slot < 9, "hotbar slots are 0-8");
        self.with_mut(|p| p.held_slot = slot);
    }

    /// The item in the main hand.
    pub fn held_item(&self) -> ItemStack {
        let p = self.state();
        p.inventory.slots[p.held_slot as usize].clone()
    }

    /// The item on the cursor (while a menu is open).
    pub fn cursor(&self) -> ItemStack {
        self.state().cursor
    }

    // ------------------------------------------------------------------ what the player received

    /// Every chat line received, as plain text, oldest first.
    pub fn messages(&self) -> Vec<String> {
        self.state().messages().map(Text::to_plain).collect()
    }

    /// Every chat line received, as JSON components.
    pub fn messages_json(&self) -> Vec<Text> {
        self.state().messages().cloned().collect()
    }

    /// The next chat line not yet read with `next_message` (plain text).
    pub fn next_message(&self) -> Option<String> {
        self.with_mut(|p| {
            let all: Vec<String> = p.messages().map(Text::to_plain).collect();
            let m = all.get(p.read_messages).cloned();
            if m.is_some() {
                p.read_messages += 1;
            }
            m
        })
    }

    /// Forget received chat lines, action bars, titles and sounds.
    pub fn clear_messages(&self) {
        self.with_mut(|p| {
            p.received.clear();
            p.read_messages = 0;
        });
    }

    /// Panics unless some received chat line contains `needle`.
    #[track_caller]
    pub fn assert_message_contains(&self, needle: &str) {
        let all = self.messages();
        assert!(
            all.iter().any(|m| m.contains(needle)),
            "{} received no message containing {needle:?}; messages: {all:#?}",
            self.name
        );
    }

    /// Panics if some received chat line contains `needle`.
    #[track_caller]
    pub fn assert_no_message_contains(&self, needle: &str) {
        let all = self.messages();
        assert!(
            !all.iter().any(|m| m.contains(needle)),
            "{} received a message containing {needle:?}; messages: {all:#?}",
            self.name
        );
    }

    /// Reads the next message (see [`MockPlayer::next_message`]) and panics unless it equals
    /// `expected`.
    #[track_caller]
    pub fn assert_next_message(&self, expected: &str) {
        match self.next_message() {
            Some(m) => assert_eq!(m, expected, "next message of {}", self.name),
            None => panic!(
                "{} has no unread message (expected {expected:?})",
                self.name
            ),
        }
    }

    /// Panics if there are unread messages.
    #[track_caller]
    pub fn assert_no_more_messages(&self) {
        if let Some(m) = self.next_message() {
            panic!("{} has an unread message: {m:?}", self.name);
        }
    }

    /// Action bar texts received (plain), oldest first.
    pub fn action_bars(&self) -> Vec<String> {
        self.state()
            .received
            .iter()
            .filter_map(|r| match r {
                Received::ActionBar(t) => Some(t.to_plain()),
                _ => None,
            })
            .collect()
    }

    /// The last action bar text (plain).
    pub fn last_action_bar(&self) -> Option<String> {
        self.action_bars().pop()
    }

    /// Titles received.
    pub fn titles(&self) -> Vec<TitleRecord> {
        self.state()
            .received
            .iter()
            .filter_map(|r| match r {
                Received::Title(t) => Some(t.clone()),
                _ => None,
            })
            .collect()
    }

    /// The last title.
    pub fn last_title(&self) -> Option<TitleRecord> {
        self.titles().pop()
    }

    /// Sounds heard.
    pub fn sounds(&self) -> Vec<SoundRecord> {
        self.state()
            .received
            .iter()
            .filter_map(|r| match r {
                Received::Sound(s) => Some(s.clone()),
                _ => None,
            })
            .collect()
    }

    /// Panics unless the player heard this sound (`"minecraft:..."` or without namespace).
    #[track_caller]
    pub fn assert_heard(&self, sound: &str) {
        let sound = namespaced(sound);
        let heard = self.sounds();
        assert!(
            heard.iter().any(|s| s.sound == sound),
            "{} did not hear {sound}; heard: {:?}",
            self.name,
            heard.iter().map(|s| &s.sound).collect::<Vec<_>>()
        );
    }

    /// Everything received, in order.
    pub fn received(&self) -> Vec<Received> {
        self.state().received
    }

    /// The kick reason, if the player was kicked.
    pub fn kick_reason(&self) -> Option<String> {
        self.state().kicked.map(|t| t.to_plain())
    }

    /// Panics unless the player was kicked.
    #[track_caller]
    pub fn assert_kicked(&self) {
        assert!(
            self.state().kicked.is_some(),
            "{} was not kicked",
            self.name
        );
    }

    /// Plugin messages the server sent to this client: (channel, data).
    pub fn plugin_messages(&self) -> Vec<(String, Vec<u8>)> {
        self.state().plugin_messages
    }

    /// Tab list header and footer (plain).
    pub fn tab_header_footer(&self) -> Option<(String, String)> {
        let p = self.state();
        Some((p.tab_header?.to_plain(), p.tab_footer?.to_plain()))
    }

    /// The name shown in the tab list, if a plugin set one (plain).
    pub fn tab_display_name(&self) -> Option<String> {
        self.state().display_name.map(|t| t.to_plain())
    }

    /// Boss bars the player sees.
    pub fn boss_bars(&self) -> Vec<BarState> {
        self.h
            .state
            .borrow()
            .bars
            .values()
            .filter(|b| b.viewers.contains(&self.uuid))
            .cloned()
            .collect()
    }

    // ------------------------------------------------------------------ plugin data

    /// Raw per-player data a plugin stored (`players.set-data`).
    pub fn data(&self, plugin: &str, key: &str) -> Option<Vec<u8>> {
        let id = rusticmc_core::PluginId::new(plugin).ok()?;
        self.h
            .rt
            .shared()
            .async_api(id)
            .player_data_get(self.uuid, key)
    }

    /// Per-player data as JSON (what the SDK's `player.data().set(key, &value)` writes).
    pub fn data_json<T: DeserializeOwned>(&self, plugin: &str, key: &str) -> Option<T> {
        serde_json::from_slice(&self.data(plugin, key)?).ok()
    }
}
