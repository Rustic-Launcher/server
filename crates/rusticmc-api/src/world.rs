//! Worlds (dimensions): blocks, time, weather, entities, particles, sounds.
//!
//! Everything except [`World::all`] is main-instance only.

use crate::bindings::rustic::api::worlds as wit;
use crate::error::Result;
use crate::host::call;
use crate::player::Player;
use crate::types::{BlockPos, BlockState, EntityRef, Location, Vec3, Weather};

/// A world by dimension id (`minecraft:overworld`).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct World {
    id: String,
}

impl World {
    /// A handle for a dimension id (no host call; an id without namespace gets `minecraft:`).
    pub fn new(id: impl Into<String>) -> World {
        World {
            id: crate::types::namespaced(id.into()),
        }
    }

    /// `minecraft:overworld`.
    pub fn overworld() -> World {
        World::new("minecraft:overworld")
    }

    /// `minecraft:the_nether`.
    pub fn nether() -> World {
        World::new("minecraft:the_nether")
    }

    /// `minecraft:the_end`.
    pub fn the_end() -> World {
        World::new("minecraft:the_end")
    }

    /// Loaded worlds.
    pub fn all() -> Vec<World> {
        call!("worlds.all", wit::all())
            .into_iter()
            .map(World::new)
            .collect()
    }

    /// The dimension id.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The block state at `pos`.
    pub fn get_block(&self, pos: BlockPos) -> Result<BlockState> {
        Ok(call!("worlds.get-block", wit::get_block(&self.id, pos))?)
    }

    /// Sets a block with neighbour updates; returns whether it changed.
    pub fn set_block(&self, pos: BlockPos, state: impl Into<BlockState>) -> Result<bool> {
        self.set_block_with(pos, state, true)
    }

    /// Sets a block; `physics: false` skips neighbour updates.
    pub fn set_block_with(
        &self,
        pos: BlockPos,
        state: impl Into<BlockState>,
        physics: bool,
    ) -> Result<bool> {
        let state = state.into();
        Ok(call!(
            "worlds.set-block",
            wit::set_block(&self.id, pos, &state, physics)
        )?)
    }

    /// Fills the box between two corners (inclusive, at most 32768 blocks); returns the count.
    pub fn fill(&self, from: BlockPos, to: BlockPos, state: impl Into<BlockState>) -> Result<u32> {
        let state = state.into();
        Ok(call!("worlds.fill", wit::fill(&self.id, from, to, &state))?)
    }

    /// Many block writes sent in one call: `world.batch().set(a, s).set(b, s).apply()?`.
    pub fn batch(&self) -> BlockBatch {
        BlockBatch {
            world: self.id.clone(),
            blocks: Vec::new(),
            physics: true,
        }
    }

    /// The Y of the highest non-air block in a column.
    pub fn highest_block_y(&self, x: i32, z: i32) -> Result<i32> {
        Ok(call!(
            "worlds.highest-block-y",
            wit::highest_block_y(&self.id, x, z)
        )?)
    }

    /// Whether chunk `(cx, cz)` is loaded.
    pub fn is_chunk_loaded(&self, cx: i32, cz: i32) -> bool {
        call!(
            "worlds.is-chunk-loaded",
            wit::is_chunk_loaded(&self.id, cx, cz)
        )
    }

    /// Keeps a chunk loaded while the plugin is enabled (or releases it).
    pub fn keep_chunk_loaded(&self, cx: i32, cz: i32, keep: bool) -> Result<()> {
        Ok(call!(
            "worlds.keep-chunk-loaded",
            wit::keep_chunk_loaded(&self.id, cx, cz, keep)
        )?)
    }

    /// Day time in ticks.
    pub fn time(&self) -> Result<u64> {
        Ok(call!("worlds.time", wit::time(&self.id))?)
    }

    /// Sets the day time.
    pub fn set_time(&self, time: u64) -> Result<()> {
        Ok(call!("worlds.set-time", wit::set_time(&self.id, time))?)
    }

    /// The weather.
    pub fn weather(&self) -> Result<Weather> {
        Ok(call!("worlds.get-weather", wit::get_weather(&self.id))?)
    }

    /// Sets the weather for `duration_ticks`.
    pub fn set_weather(&self, weather: Weather, duration_ticks: u32) -> Result<()> {
        Ok(call!(
            "worlds.set-weather",
            wit::set_weather(&self.id, weather, duration_ticks)
        )?)
    }

    /// The spawn location.
    pub fn spawn_location(&self) -> Result<Location> {
        Ok(call!("worlds.spawn", wit::spawn(&self.id))?)
    }

    /// Moves the spawn location.
    pub fn set_spawn_location(&self, at: &Location) -> Result<()> {
        Ok(call!("worlds.set-spawn", wit::set_spawn(&self.id, at))?)
    }

    /// Spawns an entity (`minecraft:zombie`).
    pub fn spawn_entity(&self, kind: &str, at: Vec3) -> Result<EntityRef> {
        self.spawn_entity_nbt(kind, at, "")
    }

    /// Spawns an entity with SNBT data (`{CustomName:"Bob"}`).
    pub fn spawn_entity_nbt(&self, kind: &str, at: Vec3, nbt: &str) -> Result<EntityRef> {
        let kind = crate::types::namespaced(kind.to_owned());
        Ok(call!(
            "worlds.spawn-entity",
            wit::spawn_entity(&self.id, &kind, at, nbt)
        )?)
    }

    /// Removes an entity by network id.
    pub fn remove_entity(&self, entity: &EntityRef) -> Result<()> {
        Ok(call!(
            "worlds.remove-entity",
            wit::remove_entity(&self.id, entity.id)
        )?)
    }

    /// Entities within `radius` of a point.
    pub fn entities_near(&self, at: Vec3, radius: f64) -> Result<Vec<EntityRef>> {
        Ok(call!(
            "worlds.entities-near",
            wit::entities_near(&self.id, at, radius)
        )?)
    }

    /// Where an entity is.
    pub fn entity_location(&self, entity: &EntityRef) -> Result<Location> {
        Ok(call!(
            "worlds.entity-location",
            wit::entity_location(&self.id, entity.id)
        )?)
    }

    /// A particle effect builder: `world.particle("minecraft:flame", at).count(20).spawn()?`.
    pub fn particle(&self, particle: &str, at: Vec3) -> Particle {
        Particle {
            world: self.id.clone(),
            particle: crate::types::namespaced(particle.to_owned()),
            options: String::new(),
            at,
            count: 1,
            offset: Vec3::new(0.0, 0.0, 0.0),
            speed: 0.0,
        }
    }

    /// Plays a sound at a point for everyone nearby.
    pub fn play_sound(&self, sound: &str, at: Vec3, volume: f32, pitch: f32) -> Result<()> {
        Ok(call!(
            "worlds.play-sound",
            wit::play_sound(&self.id, sound, at, volume, pitch)
        )?)
    }

    /// Players in this world.
    pub fn players(&self) -> Vec<Player> {
        call!("worlds.players", wit::players(&self.id))
            .into_iter()
            .map(Player::from_uuid)
            .collect()
    }
}

/// Block writes collected and sent in one host call (see [`World::batch`]).
#[derive(Debug, Clone)]
pub struct BlockBatch {
    world: String,
    blocks: Vec<(BlockPos, BlockState)>,
    physics: bool,
}

impl BlockBatch {
    /// Adds a write (applied in order).
    pub fn set(mut self, pos: BlockPos, state: impl Into<BlockState>) -> BlockBatch {
        self.blocks.push((pos, state.into()));
        self
    }

    /// Adds a write through a mutable reference (for loops).
    pub fn push(&mut self, pos: BlockPos, state: impl Into<BlockState>) {
        self.blocks.push((pos, state.into()));
    }

    /// Skips neighbour updates.
    pub fn without_physics(mut self) -> BlockBatch {
        self.physics = false;
        self
    }

    /// Number of writes queued.
    pub fn len(&self) -> usize {
        self.blocks.len()
    }

    /// Whether nothing is queued.
    pub fn is_empty(&self) -> bool {
        self.blocks.is_empty()
    }

    /// Sends the writes; returns how many blocks changed.
    pub fn apply(self) -> Result<u32> {
        if self.blocks.is_empty() {
            return Ok(0);
        }
        Ok(call!(
            "worlds.set-blocks",
            wit::set_blocks(&self.world, &self.blocks, self.physics)
        )?)
    }
}

/// A particle effect (see [`World::particle`]).
#[derive(Debug, Clone)]
pub struct Particle {
    world: String,
    particle: String,
    options: String,
    at: Vec3,
    count: u32,
    offset: Vec3,
    speed: f64,
}

impl Particle {
    /// How many particles.
    pub fn count(mut self, count: u32) -> Particle {
        self.count = count;
        self
    }

    /// Random spread on each axis.
    pub fn offset(mut self, dx: f64, dy: f64, dz: f64) -> Particle {
        self.offset = Vec3::new(dx, dy, dz);
        self
    }

    /// Particle speed.
    pub fn speed(mut self, speed: f64) -> Particle {
        self.speed = speed;
        self
    }

    /// Particle options as SNBT (`{color:[1.0,0.0,0.0],scale:1.0}` for `minecraft:dust`).
    pub fn options(mut self, snbt: impl Into<String>) -> Particle {
        self.options = snbt.into();
        self
    }

    /// Spawns it.
    pub fn spawn(self) -> Result<()> {
        Ok(call!(
            "worlds.spawn-particle",
            wit::spawn_particle(
                &self.world,
                &self.particle,
                &self.options,
                self.at,
                self.count,
                self.offset,
                self.speed
            )
        )?)
    }
}
