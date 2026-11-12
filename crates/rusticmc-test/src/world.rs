//! [`MockWorld`]: a handle on one dimension of the mock server.

use crate::harness::Harness;
use crate::state::{EntityState, ParticleRecord, SoundRecord, normalize_block, parse_block};
use rusticmc_core::model::{
    BlockPos, BlockState, EntityRef, ItemStack, Location, Uuid, Vec3, Weather,
};
use std::rc::Rc;

/// A world (dimension) of a [`crate::MockServer`]. Cheap to clone.
#[derive(Clone)]
pub struct MockWorld {
    pub(crate) h: Rc<Harness>,
    pub(crate) id: String,
}

impl std::fmt::Debug for MockWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MockWorld").field("id", &self.id).finish()
    }
}

/// Block positions accepted by [`MockWorld`] methods.
pub trait IntoBlockPos {
    /// Convert.
    fn into_block_pos(self) -> BlockPos;
}

impl IntoBlockPos for BlockPos {
    fn into_block_pos(self) -> BlockPos {
        self
    }
}

impl IntoBlockPos for (i32, i32, i32) {
    fn into_block_pos(self) -> BlockPos {
        BlockPos {
            x: self.0,
            y: self.1,
            z: self.2,
        }
    }
}

impl MockWorld {
    fn with<T>(&self, f: impl FnOnce(&crate::state::WorldState) -> T) -> T {
        let st = self.h.state.borrow();
        f(st.worlds.get(&self.id).expect("world exists"))
    }

    fn with_mut<T>(&self, f: impl FnOnce(&mut crate::state::WorldState) -> T) -> T {
        let mut st = self.h.state.borrow_mut();
        f(st.worlds.get_mut(&self.id).expect("world exists"))
    }

    /// The dimension id (`minecraft:overworld`).
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The block at a position.
    pub fn block(&self, pos: impl IntoBlockPos) -> BlockState {
        let pos = pos.into_block_pos();
        self.with(|w| w.block(pos))
    }

    /// The block id at a position (`minecraft:stone`).
    pub fn block_id(&self, pos: impl IntoBlockPos) -> String {
        self.block(pos).id
    }

    /// Set a block (no event), in command syntax: `"oak_log[axis=y]"`.
    pub fn set_block(&self, pos: impl IntoBlockPos, block: &str) -> bool {
        let pos = pos.into_block_pos();
        let state = parse_block(block);
        self.with_mut(|w| w.set(pos, &state))
    }

    /// Set a block state (no event).
    pub fn set_block_state(&self, pos: impl IntoBlockPos, state: &BlockState) -> bool {
        let pos = pos.into_block_pos();
        let state = normalize_block(state);
        self.with_mut(|w| w.set(pos, &state))
    }

    /// Highest non-air block y of a column.
    pub fn highest_block_y(&self, x: i32, z: i32) -> i32 {
        self.with(|w| w.highest_y(x, z))
    }

    /// The ground level of the flat terrain (`64` in the overworld).
    pub fn surface_y(&self) -> i32 {
        self.with(|w| w.surface_y)
    }

    /// Time of day in ticks.
    pub fn time(&self) -> u64 {
        self.with(|w| w.time)
    }

    /// Set the time.
    pub fn set_time(&self, time: u64) {
        self.with_mut(|w| w.time = time);
    }

    /// The weather.
    pub fn weather(&self) -> Weather {
        self.with(|w| w.weather)
    }

    /// Set the weather for `ticks` ticks (0: until changed).
    pub fn set_weather(&self, weather: Weather, ticks: u32) {
        self.with_mut(|w| {
            w.weather = weather;
            w.weather_ticks = ticks;
        });
    }

    /// The spawn point.
    pub fn spawn(&self) -> Location {
        self.with(|w| w.spawn.clone())
    }

    /// Move the spawn point.
    pub fn set_spawn(&self, at: Location) {
        let id = self.id.clone();
        self.with_mut(|w| w.spawn = Location { world: id, ..at });
    }

    /// A location in this world.
    pub fn location(&self, x: f64, y: f64, z: f64) -> Location {
        Location {
            world: self.id.clone(),
            x,
            y,
            z,
            yaw: 0.0,
            pitch: 0.0,
        }
    }

    /// Spawn an entity (no event), e.g. `"zombie"`.
    pub fn spawn_entity(&self, kind: &str, at: Vec3) -> EntityRef {
        use rusticmc_core::Backend;
        self.h
            .backend()
            .worlds_spawn_entity(&self.id, kind, at, "")
            .expect("world exists")
    }

    /// Every non-player entity, in spawn order.
    pub fn entities(&self) -> Vec<EntityState> {
        self.with(|w| w.entities.values().cloned().collect())
    }

    /// One entity.
    pub fn entity(&self, id: u32) -> Option<EntityState> {
        self.with(|w| w.entities.get(&id).cloned())
    }

    /// Entities of a kind (`"minecraft:zombie"` or `"zombie"`).
    pub fn entities_of(&self, kind: &str) -> Vec<EntityState> {
        let kind = crate::state::namespaced(kind);
        self.with(|w| {
            w.entities
                .values()
                .filter(|e| e.entity.kind == kind)
                .cloned()
                .collect()
        })
    }

    /// Items lying on the ground (drops of broken blocks, deaths, thrown items).
    pub fn dropped_items(&self) -> Vec<ItemStack> {
        self.with(|w| w.entities.values().filter_map(|e| e.item.clone()).collect())
    }

    /// Kill an entity (fires `entity-death`; `drops`/`exp` are its default loot).
    pub fn kill_entity(
        &self,
        entity: u32,
        killer: Option<Uuid>,
        drops: Vec<ItemStack>,
        exp: u32,
    ) -> Option<rusticmc_core::model::EntityDeath> {
        self.h.kill_entity(&self.id, entity, killer, drops, exp)
    }

    /// Damage an entity (fires `entity-damage`; it dies at 0 health): the damage dealt.
    pub fn damage_entity(
        &self,
        entity: u32,
        amount: f32,
        cause: &str,
        attacker: Option<EntityRef>,
    ) -> Option<f32> {
        self.h
            .damage_entity(&self.id, entity, amount, cause, attacker)
    }

    /// Particles spawned so far.
    pub fn particles(&self) -> Vec<ParticleRecord> {
        self.with(|w| w.particles.clone())
    }

    /// Sounds played in the world so far.
    pub fn sounds(&self) -> Vec<SoundRecord> {
        self.with(|w| w.sounds.clone())
    }

    /// Whether a chunk is loaded.
    pub fn is_chunk_loaded(&self, cx: i32, cz: i32) -> bool {
        self.with(|w| w.loaded_chunks.contains(&(cx, cz)))
    }

    /// Whether a plugin keeps a chunk loaded.
    pub fn is_chunk_forced(&self, cx: i32, cz: i32) -> bool {
        self.with(|w| w.forced_chunks.contains(&(cx, cz)))
    }

    /// Load a chunk (fires `chunk-load`).
    pub fn load_chunk(&self, cx: i32, cz: i32, new_chunk: bool) {
        let id = self.id.clone();
        self.h.op(|| {
            self.with_mut(|w| w.loaded_chunks.insert((cx, cz)));
            self.h.fire(rusticmc_core::model::ChunkLoad {
                world: id,
                x: cx,
                z: cz,
                new_chunk,
            });
        });
    }

    /// Unload a chunk (fires `chunk-unload`): false when it is not loaded or a plugin keeps it.
    pub fn unload_chunk(&self, cx: i32, cz: i32) -> bool {
        let id = self.id.clone();
        self.h.op(|| {
            let ok = self.with_mut(|w| {
                !w.forced_chunks.contains(&(cx, cz)) && w.loaded_chunks.remove(&(cx, cz))
            });
            if ok {
                self.h.fire(rusticmc_core::model::ChunkUnload {
                    world: id,
                    x: cx,
                    z: cz,
                });
            }
            ok
        })
    }

    /// Block changes plugins made with `physics = true` (counted, not simulated).
    pub fn physics_updates(&self) -> u64 {
        self.with(|w| w.physics_updates)
    }
}
