//! Chest menus with per-slot click handlers.
//!
//! ```ignore
//! Menu::new(3, Text::mini("<dark_gray>Warps"))
//!     .item(11, ItemStack::new("grass_block").name("Spawn"), |click| {
//!         click.close();
//!         click.player().teleport(&spawn).ok();
//!     })
//!     .display(13, ItemStack::new("paper").name("Pick one"))
//!     .open(&player)?;
//! ```
//!
//! Clicks in a plugin menu are cancelled by default (nothing moves); a handler may call
//! [`MenuClick::allow`]. The SDK subscribes to `inventory-click` / `inventory-close` the first
//! time a menu opens and routes clicks by (menu id, slot); handlers are dropped when the menu
//! closes.

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap};
use std::rc::Rc;

use crate::__rt;
use crate::bindings::rustic::api::menus as wit;
use crate::error::Result;
use crate::host::call;
use crate::item::ItemStack;
use crate::player::Player;
use crate::text::Text;
use crate::types::ClickKind;

pub(crate) type ClickHandler = Rc<RefCell<dyn FnMut(&mut MenuClick)>>;
pub(crate) type CloseHandler = Box<dyn FnOnce(Player)>;

/// A menu definition. Each [`Menu::open`] opens a new menu instance for one player.
pub struct Menu {
    rows: u8,
    title: Text,
    items: BTreeMap<u32, ItemStack>,
    handlers: HashMap<u32, ClickHandler>,
    on_close: Option<CloseHandler>,
}

impl std::fmt::Debug for Menu {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Menu")
            .field("rows", &self.rows)
            .field("title", &self.title.plain())
            .field("items", &self.items.len())
            .field("handlers", &self.handlers.len())
            .finish()
    }
}

impl Menu {
    /// A menu of `rows` rows of 9 (clamped to 1..=6).
    pub fn new(rows: u8, title: impl Into<Text>) -> Menu {
        Menu {
            rows: rows.clamp(1, 6),
            title: title.into(),
            items: BTreeMap::new(),
            handlers: HashMap::new(),
            on_close: None,
        }
    }

    /// Number of slots.
    pub fn size(&self) -> u32 {
        u32::from(self.rows) * 9
    }

    /// An item with a click handler. Slots outside the menu are ignored.
    pub fn item<F>(mut self, slot: u32, item: impl Into<ItemStack>, handler: F) -> Menu
    where
        F: FnMut(&mut MenuClick) + 'static,
    {
        if slot < self.size() {
            self.items.insert(slot, item.into());
            self.handlers.insert(slot, Rc::new(RefCell::new(handler)));
        }
        self
    }

    /// An item without handler (clicking it does nothing).
    pub fn display(mut self, slot: u32, item: impl Into<ItemStack>) -> Menu {
        if slot < self.size() {
            self.items.insert(slot, item.into());
            self.handlers.remove(&slot);
        }
        self
    }

    /// Puts `item` in every slot still empty (borders, backgrounds).
    pub fn fill(mut self, item: impl Into<ItemStack>) -> Menu {
        let item = item.into();
        for slot in 0..self.size() {
            self.items.entry(slot).or_insert_with(|| item.clone());
        }
        self
    }

    /// Called with the player when the menu closes.
    pub fn on_close(mut self, f: impl FnOnce(Player) + 'static) -> Menu {
        self.on_close = Some(Box::new(f));
        self
    }

    /// The WIT spec (pure).
    pub(crate) fn spec(&self) -> wit::MenuSpec {
        wit::MenuSpec {
            rows: self.rows,
            title: self.title.to_json(),
            items: self.items.iter().map(|(s, i)| (*s, i.to_raw())).collect(),
        }
    }

    /// Opens it for a player. Main instance only.
    pub fn open(self, player: &Player) -> Result<MenuHandle> {
        let spec = self.spec();
        __rt::ensure_menu_listeners()?;
        let id = call!("menus.open", wit::open(player.uuid(), &spec))?;
        __rt::with_menus(|m| m.insert(id, self.handlers, self.on_close));
        Ok(MenuHandle { id })
    }
}

/// An open menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MenuHandle {
    id: u32,
}

impl MenuHandle {
    pub(crate) fn new(id: u32) -> MenuHandle {
        MenuHandle { id }
    }

    /// The host's menu id.
    pub fn id(&self) -> u32 {
        self.id
    }

    /// Replaces a slot's item and handler.
    pub fn set_item<F>(&self, slot: u32, item: impl Into<ItemStack>, handler: F) -> Result<()>
    where
        F: FnMut(&mut MenuClick) + 'static,
    {
        let raw = item.into().to_raw();
        call!("menus.set-item", wit::set_item(self.id, slot, &raw))?;
        __rt::with_menus(|m| m.set_handler(self.id, slot, Rc::new(RefCell::new(handler))));
        Ok(())
    }

    /// Replaces a slot's item and removes its handler.
    pub fn set_display(&self, slot: u32, item: impl Into<ItemStack>) -> Result<()> {
        let raw = item.into().to_raw();
        call!("menus.set-item", wit::set_item(self.id, slot, &raw))?;
        __rt::with_menus(|m| m.clear_slot(self.id, slot));
        Ok(())
    }

    /// Changes the title.
    pub fn set_title(&self, title: impl Into<Text>) -> Result<()> {
        let json = title.into().to_json();
        Ok(call!("menus.set-title", wit::set_title(self.id, &json))?)
    }

    /// Closes the menu (its handlers are dropped when the close event arrives).
    pub fn close(&self) {
        call!("menus.close", wit::close(self.id));
    }
}

/// A click in a menu slot that has a handler.
#[derive(Debug, Clone)]
pub struct MenuClick {
    pub(crate) player: Player,
    pub(crate) menu: MenuHandle,
    pub(crate) slot: u32,
    pub(crate) kind: ClickKind,
    pub(crate) item: ItemStack,
    pub(crate) cursor: ItemStack,
    pub(crate) cancelled: bool,
}

impl MenuClick {
    /// Who clicked.
    pub fn player(&self) -> Player {
        self.player
    }

    /// The menu.
    pub fn menu(&self) -> MenuHandle {
        self.menu
    }

    /// The slot.
    pub fn slot(&self) -> u32 {
        self.slot
    }

    /// The kind of click.
    pub fn kind(&self) -> ClickKind {
        self.kind
    }

    /// The item in the slot.
    pub fn item(&self) -> &ItemStack {
        &self.item
    }

    /// The item on the cursor.
    pub fn cursor(&self) -> &ItemStack {
        &self.cursor
    }

    /// Lets the click through (items move); clicks are cancelled by default.
    pub fn allow(&mut self) {
        self.cancelled = false;
    }

    /// Whether the click stays cancelled.
    pub fn is_cancelled(&self) -> bool {
        self.cancelled
    }

    /// Closes the menu.
    pub fn close(&self) {
        self.menu.close();
    }
}

/// Click and close handlers of open menus, keyed by menu id and slot.
#[derive(Default)]
pub(crate) struct MenuRegistry {
    slots: HashMap<(u32, u32), ClickHandler>,
    close: HashMap<u32, CloseHandler>,
    open: HashMap<u32, usize>,
}

impl MenuRegistry {
    pub(crate) fn insert(
        &mut self,
        menu: u32,
        handlers: HashMap<u32, ClickHandler>,
        on_close: Option<CloseHandler>,
    ) {
        self.remove(menu);
        self.open.insert(menu, handlers.len());
        for (slot, h) in handlers {
            self.slots.insert((menu, slot), h);
        }
        if let Some(c) = on_close {
            self.close.insert(menu, c);
        }
    }

    pub(crate) fn set_handler(&mut self, menu: u32, slot: u32, h: ClickHandler) {
        self.slots.insert((menu, slot), h);
        self.open.entry(menu).or_default();
    }

    pub(crate) fn clear_slot(&mut self, menu: u32, slot: u32) {
        self.slots.remove(&(menu, slot));
    }

    pub(crate) fn handler(&self, menu: u32, slot: u32) -> Option<ClickHandler> {
        self.slots.get(&(menu, slot)).cloned()
    }

    #[cfg(test)]
    pub(crate) fn is_open(&self, menu: u32) -> bool {
        self.open.contains_key(&menu)
    }

    /// Forgets a menu; returns its close handler.
    pub(crate) fn remove(&mut self, menu: u32) -> Option<CloseHandler> {
        self.open.remove(&menu);
        self.slots.retain(|(m, _), _| *m != menu);
        self.close.remove(&menu)
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.open.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Uuid;

    fn click(menu: u32, slot: u32) -> MenuClick {
        MenuClick {
            player: Player::from_uuid(Uuid::from_u128(9)),
            menu: MenuHandle::new(menu),
            slot,
            kind: ClickKind::Left,
            item: ItemStack::empty(),
            cursor: ItemStack::empty(),
            cancelled: true,
        }
    }

    #[test]
    fn builder_spec() {
        let m = Menu::new(9, "T")
            .item(0, "diamond", |_| {})
            .display(1, ItemStack::new("stone"))
            .item(99, "dirt", |_| {})
            .fill(ItemStack::new("gray_stained_glass_pane"));
        assert_eq!(m.size(), 54);
        let spec = m.spec();
        assert_eq!(spec.rows, 6);
        assert_eq!(spec.items.len(), 54);
        assert_eq!(spec.items[0].1.id, "minecraft:diamond");
        assert_eq!(spec.items[1].1.id, "minecraft:stone");
        assert_eq!(spec.items[2].1.id, "minecraft:gray_stained_glass_pane");
        assert_eq!(m.handlers.len(), 1);
        assert_eq!(spec.title, r#"{"text":"T"}"#);
        // display() over an item removes its handler
        let m = Menu::new(1, "x").item(3, "a", |_| {}).display(3, "b");
        assert!(m.handlers.is_empty());
    }

    #[test]
    fn registry_routes_by_menu_and_slot() {
        let hits = Rc::new(RefCell::new(Vec::new()));
        let mut reg = MenuRegistry::default();
        let h = hits.clone();
        let m = Menu::new(1, "x")
            .item(4, "a", move |c| {
                h.borrow_mut().push((c.menu().id(), c.slot()));
                c.allow();
            })
            .on_close(|_| {});
        reg.insert(7, m.handlers, m.on_close);
        assert!(reg.is_open(7));
        assert!(reg.handler(7, 3).is_none());
        assert!(reg.handler(8, 4).is_none());
        let mut c = click(7, 4);
        (reg.handler(7, 4).unwrap().borrow_mut())(&mut c);
        assert!(!c.is_cancelled());
        assert_eq!(*hits.borrow(), vec![(7, 4)]);

        reg.set_handler(7, 5, Rc::new(RefCell::new(|_: &mut MenuClick| {})));
        assert!(reg.handler(7, 5).is_some());
        reg.clear_slot(7, 5);
        assert!(reg.handler(7, 5).is_none());

        assert!(reg.remove(7).is_some());
        assert!(!reg.is_open(7));
        assert!(reg.handler(7, 4).is_none());
        assert_eq!(reg.len(), 0);
        assert!(reg.remove(7).is_none());
    }
}
