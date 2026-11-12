//! A player's inventory.
//!
//! Slots: 0-8 hotbar, 9-35 main, 36-39 armor (feet, legs, chest, head), 40 off hand.

use crate::bindings::rustic::api::inventories as wit;
use crate::error::Result;
use crate::host::call;
use crate::item::ItemStack;
use crate::types::Uuid;

/// Slot numbers.
pub mod slots {
    /// First hotbar slot.
    pub const HOTBAR_START: u32 = 0;
    /// First slot of the main inventory.
    pub const MAIN_START: u32 = 9;
    /// Boots.
    pub const FEET: u32 = 36;
    /// Leggings.
    pub const LEGS: u32 = 37;
    /// Chestplate.
    pub const CHEST: u32 = 38;
    /// Helmet.
    pub const HEAD: u32 = 39;
    /// Off hand.
    pub const OFF_HAND: u32 = 40;
    /// Number of slots.
    pub const COUNT: u32 = 41;
}

/// A player's inventory (a handle; every call asks the host).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlayerInventory {
    uuid: Uuid,
}

impl PlayerInventory {
    pub(crate) fn new(uuid: Uuid) -> PlayerInventory {
        PlayerInventory { uuid }
    }

    /// The stack in a slot.
    pub fn get(&self, slot: u32) -> Result<ItemStack> {
        Ok(
            call!("inventories.get-slot", wit::get_slot(self.uuid, slot))
                .map(ItemStack::from_raw)?,
        )
    }

    /// Puts a stack in a slot.
    pub fn set(&self, slot: u32, item: &ItemStack) -> Result<()> {
        let raw = item.to_raw();
        Ok(call!(
            "inventories.set-slot",
            wit::set_slot(self.uuid, slot, &raw)
        )?)
    }

    /// All 41 slots.
    pub fn contents(&self) -> Result<Vec<ItemStack>> {
        Ok(call!("inventories.contents", wit::contents(self.uuid))?
            .into_iter()
            .map(ItemStack::from_raw)
            .collect())
    }

    /// Adds a stack where it fits; returns what did not fit (empty if everything did).
    pub fn give(&self, item: &ItemStack) -> Result<ItemStack> {
        let raw = item.to_raw();
        Ok(call!("inventories.give", wit::give(self.uuid, &raw)).map(ItemStack::from_raw)?)
    }

    /// Removes up to `count` items of `id`; returns how many were removed.
    pub fn take(&self, id: &str, count: u32) -> Result<u32> {
        let id = crate::types::namespaced(id.to_owned());
        Ok(call!("inventories.take", wit::take(self.uuid, &id, count))?)
    }

    /// Empties the inventory.
    pub fn clear(&self) -> Result<()> {
        Ok(call!("inventories.clear", wit::clear(self.uuid))?)
    }

    /// The selected hotbar slot (0-8).
    pub fn held_slot(&self) -> Result<u32> {
        Ok(call!("inventories.held-slot", wit::held_slot(self.uuid))?)
    }

    /// The stack in the main hand.
    pub fn main_hand(&self) -> Result<ItemStack> {
        self.get(self.held_slot()?)
    }

    /// Closes whatever container the player has open.
    pub fn close(&self) -> Result<()> {
        Ok(call!(
            "inventories.close-container",
            wit::close_container(self.uuid)
        )?)
    }
}
