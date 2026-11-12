//! The in-memory game: worlds, players, menus, scoreboards, boss bars, channels. Pure data and
//! the game rules that need no runtime (inventory stacking, block storage, world generation).
//!
//! Exposed for assertions the [`crate::MockServer`] helpers do not cover
//! ([`crate::MockServer::with_state`]); field names follow the WIT vocabulary.

#![allow(missing_docs)]

use rusticmc_core::model::{
    BarColor, BarId, BarStyle, BlockPos, BlockState, DisplaySlot, EntityRef, GameMode, ItemStack,
    Location, MenuId, TeamColor, Text, Uuid, Vec3, Weather,
};
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};

/// Number of slots of a player inventory (WIT `inventories`).
pub const INVENTORY_SLOTS: usize = 41;
/// Hotbar + main inventory: the slots `give` fills.
pub const STORAGE_SLOTS: usize = 36;

/// The overworld id.
pub const OVERWORLD: &str = "minecraft:overworld";
/// The nether id.
pub const NETHER: &str = "minecraft:the_nether";
/// The end id.
pub const THE_END: &str = "minecraft:the_end";

/// `"stone"` → `"minecraft:stone"`; ids that already have a namespace are kept.
pub fn namespaced(id: &str) -> String {
    let id = id.trim();
    if id.contains(':') {
        id.to_owned()
    } else {
        format!("minecraft:{id}")
    }
}

/// A block state with a namespaced id and properties sorted by key (so equal states compare
/// equal however they were written).
pub fn normalize_block(state: &BlockState) -> BlockState {
    let mut properties = state.properties.clone();
    properties.sort();
    BlockState {
        id: namespaced(&state.id),
        properties,
    }
}

/// Parse the command syntax `oak_log[axis=y]` into a block state.
pub fn parse_block(text: &str) -> BlockState {
    let text = text.trim();
    let (id, props) = match text.split_once('[') {
        Some((id, rest)) => (id, rest.trim_end_matches(']')),
        None => (text, ""),
    };
    let properties = props
        .split(',')
        .filter_map(|kv| {
            let (k, v) = kv.split_once('=')?;
            Some((k.trim().to_owned(), v.trim().to_owned()))
        })
        .collect();
    normalize_block(&BlockState {
        id: id.to_owned(),
        properties,
    })
}

/// Air.
pub fn air() -> BlockState {
    BlockState {
        id: "minecraft:air".into(),
        properties: Vec::new(),
    }
}

/// Whether a block id is one of the air blocks.
pub fn is_air(state: &BlockState) -> bool {
    matches!(
        state.id.as_str(),
        "minecraft:air" | "minecraft:cave_air" | "minecraft:void_air"
    )
}

/// An item stack with a namespaced id (empty stacks become the canonical empty stack).
pub fn normalize_item(item: &ItemStack) -> ItemStack {
    if item.count == 0 || item.id.is_empty() || namespaced(&item.id) == "minecraft:air" {
        return ItemStack::empty();
    }
    ItemStack {
        id: namespaced(&item.id),
        count: item.count,
        components: item.components.clone(),
    }
}

/// Max stack size of an item id (vanilla defaults; components are not interpreted).
pub fn max_stack(id: &str) -> u32 {
    let path = id.rsplit(':').next().unwrap_or(id);
    const SINGLE_SUFFIXES: [&str; 12] = [
        "_sword",
        "_pickaxe",
        "_axe",
        "_shovel",
        "_hoe",
        "_helmet",
        "_chestplate",
        "_leggings",
        "_boots",
        "_shulker_box",
        "_bucket",
        "_spear",
    ];
    const SINGLE: [&str; 22] = [
        "bow",
        "crossbow",
        "trident",
        "shield",
        "elytra",
        "potion",
        "splash_potion",
        "lingering_potion",
        "totem_of_undying",
        "shulker_box",
        "fishing_rod",
        "flint_and_steel",
        "shears",
        "saddle",
        "mace",
        "carrot_on_a_stick",
        "warped_fungus_on_a_stick",
        "enchanted_book",
        "written_book",
        "writable_book",
        "music_disc_cat",
        "brush",
    ];
    const SIXTEEN: [&str; 9] = [
        "ender_pearl",
        "snowball",
        "egg",
        "bucket",
        "armor_stand",
        "honey_bottle",
        "oak_sign",
        "white_banner",
        "written_book_copy",
    ];
    if path == "bucket" || SIXTEEN.contains(&path) || path.ends_with("_sign") {
        16
    } else if SINGLE.contains(&path)
        || SINGLE_SUFFIXES.iter().any(|s| path.ends_with(s))
        || path.starts_with("music_disc_")
    {
        1
    } else if path.ends_with("_banner") {
        16
    } else {
        64
    }
}

/// Whether two stacks can merge (same item and components).
pub fn stackable(a: &ItemStack, b: &ItemStack) -> bool {
    !a.is_empty() && !b.is_empty() && a.id == b.id && a.components == b.components
}

/// A 41-slot player inventory with vanilla-like give/take semantics.
#[derive(Clone, Debug, PartialEq)]
pub struct Inventory {
    /// Slots: 0-8 hotbar, 9-35 main, 36-39 armor (feet..head), 40 off hand.
    pub slots: Vec<ItemStack>,
}

impl Default for Inventory {
    fn default() -> Self {
        Self {
            slots: vec![ItemStack::empty(); INVENTORY_SLOTS],
        }
    }
}

impl Inventory {
    /// Add an item where it fits: first onto existing stacks of the same item (hotbar, then main
    /// inventory), then into empty slots in the same order. Returns what did not fit.
    pub fn give(&mut self, item: &ItemStack) -> ItemStack {
        let item = normalize_item(item);
        if item.is_empty() {
            return ItemStack::empty();
        }
        let max = max_stack(&item.id);
        let mut left = item.count;
        for slot in self.slots.iter_mut().take(STORAGE_SLOTS) {
            if left == 0 {
                break;
            }
            if stackable(slot, &item) && slot.count < max {
                let n = (max - slot.count).min(left);
                slot.count += n;
                left -= n;
            }
        }
        for slot in self.slots.iter_mut().take(STORAGE_SLOTS) {
            if left == 0 {
                break;
            }
            if slot.is_empty() {
                let n = max.min(left);
                *slot = ItemStack {
                    id: item.id.clone(),
                    count: n,
                    components: item.components.clone(),
                };
                left -= n;
            }
        }
        if left == 0 {
            ItemStack::empty()
        } else {
            ItemStack {
                count: left,
                ..item
            }
        }
    }

    /// Remove up to `count` items with this id (any components), from every slot in order.
    /// Returns how many were removed.
    pub fn take(&mut self, id: &str, count: u32) -> u32 {
        let id = namespaced(id);
        let mut removed = 0;
        for slot in &mut self.slots {
            if removed == count {
                break;
            }
            if !slot.is_empty() && slot.id == id {
                let n = slot.count.min(count - removed);
                slot.count -= n;
                removed += n;
                if slot.count == 0 {
                    *slot = ItemStack::empty();
                }
            }
        }
        removed
    }

    /// How many items with this id the inventory holds.
    pub fn count(&self, id: &str) -> u32 {
        let id = namespaced(id);
        self.slots
            .iter()
            .filter(|s| !s.is_empty() && s.id == id)
            .map(|s| s.count)
            .sum()
    }

    /// Empty every slot.
    pub fn clear(&mut self) {
        self.slots = vec![ItemStack::empty(); INVENTORY_SLOTS];
    }

    /// The non-empty stacks with their slots.
    pub fn stacks(&self) -> Vec<(u32, ItemStack)> {
        self.slots
            .iter()
            .enumerate()
            .filter(|(_, s)| !s.is_empty())
            .map(|(i, s)| (i as u32, s.clone()))
            .collect()
    }
}

/// Something a player received: chat, action bar, title, sound.
#[derive(Clone, Debug, PartialEq)]
pub enum Received {
    /// A chat line.
    Message(Text),
    /// An action bar text.
    ActionBar(Text),
    /// A title.
    Title(TitleRecord),
    /// A sound played for this player (by a plugin or nearby in the world).
    Sound(SoundRecord),
}

/// A title shown to a player.
#[derive(Clone, Debug, PartialEq)]
pub struct TitleRecord {
    /// Title.
    pub title: Text,
    /// Subtitle.
    pub subtitle: Text,
    /// Fade-in ticks.
    pub fade_in: u32,
    /// Stay ticks.
    pub stay: u32,
    /// Fade-out ticks.
    pub fade_out: u32,
}

/// A sound.
#[derive(Clone, Debug, PartialEq)]
pub struct SoundRecord {
    /// Sound event id.
    pub sound: String,
    /// Where (the player's position for player sounds).
    pub at: Vec3,
    /// Volume.
    pub volume: f32,
    /// Pitch.
    pub pitch: f32,
}

/// A particle spawn.
#[derive(Clone, Debug, PartialEq)]
pub struct ParticleRecord {
    /// Particle type id.
    pub particle: String,
    /// Options SNBT.
    pub options: String,
    /// Where.
    pub at: Vec3,
    /// Count.
    pub count: u32,
    /// Offset.
    pub offset: Vec3,
    /// Speed.
    pub speed: f64,
}

/// A player (online or not).
#[derive(Clone, Debug)]
pub struct PlayerState {
    pub uuid: Uuid,
    pub name: String,
    pub online: bool,
    pub entity_id: u32,
    pub location: Location,
    pub game_mode: GameMode,
    pub health: f32,
    pub max_health: f32,
    pub food: u32,
    pub saturation: f32,
    pub level: u32,
    pub allow_flight: bool,
    pub flying: bool,
    pub address: String,
    pub ping: u32,
    pub locale: String,
    pub inventory: Inventory,
    pub held_slot: u32,
    pub cursor: ItemStack,
    pub dead: bool,
    pub open_menu: Option<MenuId>,
    pub received: Vec<Received>,
    pub read_messages: usize,
    pub kicked: Option<Text>,
    pub plugin_messages: Vec<(String, Vec<u8>)>,
    pub tab_header: Option<Text>,
    pub tab_footer: Option<Text>,
    pub display_name: Option<Text>,
}

impl PlayerState {
    pub fn entity_ref(&self) -> EntityRef {
        EntityRef {
            id: self.entity_id,
            uuid: self.uuid,
            kind: "minecraft:player".into(),
        }
    }

    pub fn messages(&self) -> impl Iterator<Item = &Text> {
        self.received.iter().filter_map(|r| match r {
            Received::Message(t) => Some(t),
            _ => None,
        })
    }
}

/// A non-player entity.
#[derive(Clone, Debug)]
pub struct EntityState {
    pub entity: EntityRef,
    pub location: Location,
    pub nbt: String,
    /// For item entities (drops): the stack.
    pub item: Option<ItemStack>,
    /// Health (living entities; 20 by default).
    pub health: f32,
}

/// One world (dimension).
#[derive(Clone, Debug)]
pub struct WorldState {
    pub id: String,
    pub min_y: i32,
    pub max_y: i32,
    /// Flat generator: `ground` at `surface_y`, `filler` below down to `min_y + 1`,
    /// bedrock at `min_y`, air above.
    pub surface_y: i32,
    pub ground: BlockState,
    pub filler: BlockState,
    pub blocks: HashMap<BlockPos, BlockState>,
    pub time: u64,
    pub weather: Weather,
    pub weather_ticks: u32,
    pub spawn: Location,
    pub entities: BTreeMap<u32, EntityState>,
    pub particles: Vec<ParticleRecord>,
    pub sounds: Vec<SoundRecord>,
    pub loaded_chunks: BTreeSet<(i32, i32)>,
    pub forced_chunks: BTreeSet<(i32, i32)>,
    /// Block changes made through `set-block`/`fill` with `physics = true` (counted, not simulated).
    pub physics_updates: u64,
}

impl WorldState {
    pub fn flat(
        id: &str,
        min_y: i32,
        max_y: i32,
        surface_y: i32,
        ground: &str,
        filler: &str,
    ) -> Self {
        let spawn = Location {
            world: id.to_owned(),
            x: 0.5,
            y: f64::from(surface_y + 1),
            z: 0.5,
            yaw: 0.0,
            pitch: 0.0,
        };
        let mut loaded_chunks = BTreeSet::new();
        for cx in -2..=2 {
            for cz in -2..=2 {
                loaded_chunks.insert((cx, cz));
            }
        }
        Self {
            id: id.to_owned(),
            min_y,
            max_y,
            surface_y,
            ground: parse_block(ground),
            filler: parse_block(filler),
            blocks: HashMap::new(),
            time: 0,
            weather: Weather::Clear,
            weather_ticks: 0,
            spawn,
            entities: BTreeMap::new(),
            particles: Vec::new(),
            sounds: Vec::new(),
            loaded_chunks,
            forced_chunks: BTreeSet::new(),
            physics_updates: 0,
        }
    }

    pub fn in_height(&self, y: i32) -> bool {
        y >= self.min_y && y <= self.max_y
    }

    /// The block at `pos` (generated terrain unless something was set there).
    pub fn block(&self, pos: BlockPos) -> BlockState {
        if let Some(b) = self.blocks.get(&pos) {
            return b.clone();
        }
        self.generated(pos)
    }

    fn generated(&self, pos: BlockPos) -> BlockState {
        if !self.in_height(pos.y) || pos.y > self.surface_y {
            air()
        } else if pos.y == self.min_y {
            parse_block("bedrock")
        } else if pos.y == self.surface_y {
            self.ground.clone()
        } else {
            self.filler.clone()
        }
    }

    /// Set a block; returns whether it changed. Out-of-height positions are refused.
    pub fn set(&mut self, pos: BlockPos, state: &BlockState) -> bool {
        if !self.in_height(pos.y) {
            return false;
        }
        let state = normalize_block(state);
        if self.block(pos) == state {
            return false;
        }
        if self.generated(pos) == state {
            self.blocks.remove(&pos);
        } else {
            self.blocks.insert(pos, state);
        }
        self.loaded_chunks.insert((pos.x >> 4, pos.z >> 4));
        true
    }

    pub fn highest_y(&self, x: i32, z: i32) -> i32 {
        let explicit = self
            .blocks
            .iter()
            .filter(|(p, b)| p.x == x && p.z == z && !is_air(b))
            .map(|(p, _)| p.y)
            .max();
        let mut y = self.surface_y;
        // A generated column may have been dug out from the top.
        while y >= self.min_y && is_air(&self.block(BlockPos { x, y, z })) {
            y -= 1;
        }
        match explicit {
            Some(e) => e.max(y),
            None => y,
        }
    }
}

/// A plugin menu.
#[derive(Clone, Debug)]
pub struct MenuState {
    pub id: MenuId,
    pub player: Uuid,
    pub rows: u8,
    pub title: Text,
    pub items: BTreeMap<u32, ItemStack>,
}

/// A scoreboard objective.
#[derive(Clone, Debug, PartialEq)]
pub struct ObjectiveState {
    pub display: Text,
    pub criteria: String,
    pub scores: BTreeMap<String, i32>,
}

/// A scoreboard team.
#[derive(Clone, Debug, PartialEq)]
pub struct TeamState {
    pub prefix: Text,
    pub suffix: Text,
    pub color: TeamColor,
    pub members: BTreeSet<String>,
}

/// A boss bar.
#[derive(Clone, Debug, PartialEq)]
pub struct BarState {
    pub title: Text,
    pub color: BarColor,
    pub style: BarStyle,
    pub progress: f32,
    pub viewers: BTreeSet<Uuid>,
}

/// Something to do once the current harness operation returns (events caused by a plugin's own
/// calls are fired after its call, never re-entrantly).
#[derive(Clone, Debug)]
pub enum Pending {
    /// The player died (health set to 0 by a plugin, `/kill`).
    Death { player: Uuid, cause: String },
    /// The player was kicked: fire the quit event and remove them.
    Kicked { player: Uuid },
    /// A menu or container was closed by a plugin or by opening another one.
    MenuClosed { player: Uuid, menu: Option<MenuId> },
}

/// The whole game.
#[derive(Debug)]
pub struct GameState {
    pub players: BTreeMap<Uuid, PlayerState>,
    /// Online players in join order.
    pub online: Vec<Uuid>,
    pub ops: BTreeSet<Uuid>,
    pub worlds: BTreeMap<String, WorldState>,
    pub next_entity: u32,
    pub next_menu: MenuId,
    pub next_bar: BarId,
    pub menus: BTreeMap<MenuId, MenuState>,
    pub objectives: BTreeMap<String, ObjectiveState>,
    pub display_slots: BTreeMap<String, String>,
    pub teams: BTreeMap<String, TeamState>,
    pub bars: BTreeMap<BarId, BarState>,
    pub incoming_channels: BTreeSet<String>,
    pub console: Vec<String>,
    pub broadcasts: Vec<Text>,
    pub console_commands: Vec<String>,
    pub max_players: u32,
    pub motd: Text,
    pub behind_proxy: bool,
    pub keep_inventory: bool,
    pub tick_ms: f64,
    pub pending: VecDeque<Pending>,
    pub commands_changed: u32,
    pub disabled_plugins: Vec<String>,
}

impl Default for GameState {
    fn default() -> Self {
        let mut worlds = BTreeMap::new();
        worlds.insert(
            OVERWORLD.to_owned(),
            WorldState::flat(OVERWORLD, -64, 319, 64, "grass_block[snowy=false]", "stone"),
        );
        worlds.insert(
            NETHER.to_owned(),
            WorldState::flat(NETHER, 0, 255, 64, "netherrack", "netherrack"),
        );
        worlds.insert(
            THE_END.to_owned(),
            WorldState::flat(THE_END, 0, 255, 48, "end_stone", "end_stone"),
        );
        Self {
            players: BTreeMap::new(),
            online: Vec::new(),
            ops: BTreeSet::new(),
            worlds,
            next_entity: 1,
            next_menu: 1,
            next_bar: 1,
            menus: BTreeMap::new(),
            objectives: BTreeMap::new(),
            display_slots: BTreeMap::new(),
            teams: BTreeMap::new(),
            bars: BTreeMap::new(),
            incoming_channels: BTreeSet::new(),
            console: Vec::new(),
            broadcasts: Vec::new(),
            console_commands: Vec::new(),
            max_players: 20,
            motd: Text::plain("A Rustic test server"),
            behind_proxy: false,
            keep_inventory: false,
            tick_ms: 0.0,
            pending: VecDeque::new(),
            commands_changed: 0,
            disabled_plugins: Vec::new(),
        }
    }
}

/// The scoreboard slot key.
pub fn slot_key(slot: DisplaySlot) -> &'static str {
    match slot {
        DisplaySlot::Sidebar => "sidebar",
        DisplaySlot::PlayerList => "list",
        DisplaySlot::BelowName => "below_name",
    }
}

impl GameState {
    pub fn next_entity_id(&mut self) -> u32 {
        let id = self.next_entity;
        self.next_entity += 1;
        id
    }

    pub fn online_player(&self, uuid: Uuid) -> Option<&PlayerState> {
        self.players.get(&uuid).filter(|p| p.online)
    }

    pub fn online_player_mut(&mut self, uuid: Uuid) -> Option<&mut PlayerState> {
        self.players.get_mut(&uuid).filter(|p| p.online)
    }

    pub fn by_name(&self, name: &str) -> Option<Uuid> {
        self.online
            .iter()
            .copied()
            .find(|u| self.players[u].name.eq_ignore_ascii_case(name))
    }

    /// Show `message` to every online player and the console.
    pub fn broadcast(&mut self, message: &Text) {
        for u in self.online.clone() {
            if let Some(p) = self.players.get_mut(&u) {
                p.received.push(Received::Message(message.clone()));
            }
        }
        self.console.push(message.to_plain());
        self.broadcasts.push(message.clone());
    }

    /// Play a sound in a world: recorded on the world and for every player in range.
    pub fn world_sound(&mut self, world: &str, record: SoundRecord) {
        let range = 16.0 * f64::from(record.volume.max(1.0));
        let at = record.at;
        if let Some(w) = self.worlds.get_mut(world) {
            w.sounds.push(record.clone());
        }
        for u in self.online.clone() {
            if let Some(p) = self.players.get_mut(&u)
                && p.location.world == world
            {
                let (dx, dy, dz) = (
                    p.location.x - at.x,
                    p.location.y - at.y,
                    p.location.z - at.z,
                );
                if (dx * dx + dy * dy + dz * dz).sqrt() <= range {
                    p.received.push(Received::Sound(record.clone()));
                }
            }
        }
    }

    /// Spawn an item entity (a drop) and return its id.
    pub fn drop_item(&mut self, world: &str, at: Vec3, item: ItemStack) -> Option<u32> {
        let item = normalize_item(&item);
        if item.is_empty() || !self.worlds.contains_key(world) {
            return None;
        }
        let id = self.next_entity_id();
        let uuid = Uuid::new(0x00d7_0000_0000_0000 | u64::from(id), u64::from(id));
        let w = self.worlds.get_mut(world)?;
        w.entities.insert(
            id,
            EntityState {
                entity: EntityRef {
                    id,
                    uuid,
                    kind: "minecraft:item".into(),
                },
                location: Location {
                    world: world.to_owned(),
                    x: at.x,
                    y: at.y,
                    z: at.z,
                    yaw: 0.0,
                    pitch: 0.0,
                },
                nbt: String::new(),
                item: Some(item),
                health: 5.0,
            },
        );
        Some(id)
    }

    /// Close the menu a player has open (state only; events are the caller's business).
    pub fn close_menu_of(&mut self, player: Uuid) -> Option<MenuId> {
        let p = self.players.get_mut(&player)?;
        let m = p.open_menu.take()?;
        self.menus.remove(&m);
        Some(m)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn give_stacks_then_fills_empty_slots() {
        let mut inv = Inventory::default();
        assert!(inv.give(&ItemStack::new("diamond", 10)).is_empty());
        assert!(
            inv.give(&ItemStack::new("minecraft:diamond", 60))
                .is_empty()
        );
        assert_eq!(inv.slots[0].count, 64);
        assert_eq!(inv.slots[1].count, 6);
        assert_eq!(inv.count("diamond"), 70);
        // Swords do not stack.
        inv.give(&ItemStack::new("diamond_sword", 2));
        assert_eq!(inv.slots[2].count, 1);
        assert_eq!(inv.slots[3].count, 1);
        // Components make stacks distinct.
        let named = ItemStack {
            id: "minecraft:diamond".into(),
            count: 1,
            components: "{custom_name:'x'}".into(),
        };
        inv.give(&named);
        assert_eq!(inv.slots[4], named);
    }

    #[test]
    fn give_returns_what_does_not_fit() {
        let mut inv = Inventory::default();
        let left = inv.give(&ItemStack::new("stone", 64 * 36 + 5));
        assert_eq!(left.count, 5);
        assert_eq!(left.id, "minecraft:stone");
        assert!(
            inv.slots[36..].iter().all(ItemStack::is_empty),
            "armor untouched"
        );
    }

    #[test]
    fn take_removes_across_slots() {
        let mut inv = Inventory::default();
        inv.give(&ItemStack::new("dirt", 100));
        inv.slots[40] = ItemStack::new("minecraft:dirt", 3);
        assert_eq!(inv.take("dirt", 102), 102);
        assert_eq!(inv.count("dirt"), 1);
        assert_eq!(inv.take("dirt", 10), 1);
        assert_eq!(inv.take("dirt", 10), 0);
        assert!(inv.stacks().is_empty());
    }

    #[test]
    fn flat_world_blocks() {
        let mut w = WorldState::flat(OVERWORLD, -64, 319, 64, "grass_block", "stone");
        let p = |y| BlockPos { x: 3, y, z: -7 };
        assert_eq!(w.block(p(64)).id, "minecraft:grass_block");
        assert_eq!(w.block(p(10)).id, "minecraft:stone");
        assert_eq!(w.block(p(-64)).id, "minecraft:bedrock");
        assert!(is_air(&w.block(p(65))));
        assert_eq!(w.highest_y(3, -7), 64);
        assert!(w.set(p(70), &parse_block("oak_log[axis=y]")));
        assert!(!w.set(p(70), &parse_block("minecraft:oak_log[axis=y]")));
        assert_eq!(w.highest_y(3, -7), 70);
        assert!(w.set(p(64), &air()));
        assert!(!w.set(p(400), &air()));
        // Setting back to the generated block forgets the override.
        assert!(w.set(p(64), &parse_block("grass_block")));
        assert!(!w.blocks.contains_key(&p(64)));
    }

    #[test]
    fn block_parsing_sorts_properties() {
        let b = parse_block("chest[waterlogged=false,facing=north]");
        assert_eq!(b.id, "minecraft:chest");
        assert_eq!(
            b.properties,
            vec![
                ("facing".to_owned(), "north".to_owned()),
                ("waterlogged".to_owned(), "false".to_owned())
            ]
        );
    }
}
