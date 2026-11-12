//! What the adapter keeps of the things plugins show players, which Horizon keeps per player only:
//! menus (`open_menu` windows), boss bars (`boss_bar` per viewer), and the per-tick notes that join
//! two hooks (a block break that must drop nothing, the loot of that break).
//!
//! Thread locals of the server thread, borrowed briefly and never across a `Host` call, so a
//! nested runtime entry (re-entrancy) sees the same state.
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use horizon_client::items::ItemStack as HStack;
use horizon_client::mod_api::{NativeBossBar, Text as HText};
use rusticmc_core::model::{BarId, MenuId};

/// A menu a plugin opened: who sees it, Horizon's window, and its contents (the model plugins
/// change slot by slot; Horizon takes the whole list).
#[derive(Clone)]
pub struct Menu {
    pub player: u128,
    pub window: i32,
    pub rows: u8,
    pub title: HText,
    pub items: Vec<Option<HStack>>,
}

#[derive(Default)]
struct Menus {
    next: MenuId,
    open: HashMap<MenuId, Menu>,
}

/// A boss bar: what it shows and who sees it.
#[derive(Clone)]
pub struct Bar {
    pub id: u128,
    pub bar: NativeBossBar,
    pub viewers: HashSet<u128>,
}

#[derive(Default)]
struct Bars {
    next: BarId,
    bars: HashMap<BarId, Bar>,
}

thread_local! {
    static MENUS: RefCell<Menus> = RefCell::new(Menus::default());
    static BARS: RefCell<Bars> = RefCell::new(Bars::default());
    /// Block breaks of this tick whose drops a plugin refused (`block-break.drop-items = false`),
    /// by dimension and position: their loot and experience are cleared.
    static NO_DROPS: RefCell<HashSet<(String, [i32; 3])>> = RefCell::new(HashSet::new());
    /// When each player's plugin menu last closed: Horizon also reports that window as one of
    /// its own containers closing (`player_container_close`), which must not reach plugins as a
    /// second, menu-less `inventory-close`.
    static MENU_CLOSED_AT: RefCell<HashMap<u128, std::time::Instant>> = RefCell::new(HashMap::new());
}

thread_local! {
    /// Plugin menus Rustic closed itself whose close Horizon did not report: their
    /// `inventory-close` fires at the end of the tick.
    static CLOSED: RefCell<Vec<(MenuId, u128)>> = const { RefCell::new(Vec::new()) };
}

/// Rustic closed `id` (Horizon's `close_menu`): when Horizon's `menu_closed` did not already
/// report it, it is forgotten and its close is queued for [`take_closed`].
pub fn closed_by_rustic(id: MenuId, player: u128) {
    if menu(id).is_some() {
        forget_menu(id);
        note_menu_closed(player);
        CLOSED.with(|c| c.borrow_mut().push((id, player)));
    }
}

/// The menu closes to report.
pub fn take_closed() -> Vec<(MenuId, u128)> {
    CLOSED.with(|c| std::mem::take(&mut *c.borrow_mut()))
}

/// A plugin menu of `player` closed (or is closing) now.
pub fn note_menu_closed(player: u128) {
    MENU_CLOSED_AT.with(|m| m.borrow_mut().insert(player, std::time::Instant::now()));
}

/// Whether a container close Horizon reports for `player` is their plugin menu's: one is open, or
/// one closed within the last two seconds.
pub fn is_plugin_menu_close(player: u128) -> bool {
    menu_of(player).is_some()
        || MENU_CLOSED_AT.with(|m| {
            m.borrow()
                .get(&player)
                .is_some_and(|at| at.elapsed() < std::time::Duration::from_secs(2))
        })
}

// ---- menus ------------------------------------------------------------------------------

/// Records a menu Horizon opened; its id.
pub fn menu_opened(menu: Menu) -> MenuId {
    MENUS.with(|m| {
        let mut m = m.borrow_mut();
        m.next = m.next.wrapping_add(1).max(1);
        let id = m.next;
        m.open.insert(id, menu);
        id
    })
}

pub fn menu(id: MenuId) -> Option<Menu> {
    MENUS.with(|m| m.borrow().open.get(&id).cloned())
}

/// Changes a recorded menu.
pub fn update_menu(id: MenuId, f: impl FnOnce(&mut Menu)) -> Option<Menu> {
    MENUS.with(|m| {
        let mut m = m.borrow_mut();
        let menu = m.open.get_mut(&id)?;
        f(menu);
        Some(menu.clone())
    })
}

/// The plugin menu `player` sees in `window`.
pub fn menu_at(player: u128, window: i32) -> Option<(MenuId, Menu)> {
    MENUS.with(|m| {
        m.borrow()
            .open
            .iter()
            .find(|(_, menu)| menu.player == player && menu.window == window)
            .map(|(id, menu)| (*id, menu.clone()))
    })
}

/// The plugin menu `player` has open, the latest opened.
pub fn menu_of(player: u128) -> Option<MenuId> {
    MENUS.with(|m| {
        m.borrow()
            .open
            .iter()
            .filter(|(_, menu)| menu.player == player)
            .map(|(id, _)| *id)
            .max()
    })
}

pub fn forget_menu(id: MenuId) {
    MENUS.with(|m| m.borrow_mut().open.remove(&id));
}

/// A player left: their menus are gone.
pub fn player_left(player: u128) -> Vec<MenuId> {
    let gone = MENUS.with(|m| {
        let mut m = m.borrow_mut();
        let ids: Vec<MenuId> = m
            .open
            .iter()
            .filter(|(_, menu)| menu.player == player)
            .map(|(id, _)| *id)
            .collect();
        for id in &ids {
            m.open.remove(id);
        }
        ids
    });
    BARS.with(|b| {
        for bar in b.borrow_mut().bars.values_mut() {
            bar.viewers.remove(&player);
        }
    });
    MENU_CLOSED_AT.with(|m| m.borrow_mut().remove(&player));
    gone
}

// ---- boss bars --------------------------------------------------------------------------

/// The id Horizon's boss bar packets carry for Rustic's bar `id` (a UUID of its own, so a
/// vanilla `/bossbar` never meets it).
fn bar_uuid(id: BarId) -> u128 {
    0x5275_7374_6963_4000_8000_0000_0000_0000 | u128::from(id)
}

pub fn bar_created(bar: NativeBossBar) -> BarId {
    BARS.with(|b| {
        let mut b = b.borrow_mut();
        b.next = b.next.wrapping_add(1).max(1);
        let id = b.next;
        b.bars.insert(
            id,
            Bar {
                id: bar_uuid(id),
                bar,
                viewers: HashSet::new(),
            },
        );
        id
    })
}

pub fn update_bar(id: BarId, f: impl FnOnce(&mut Bar)) -> Option<Bar> {
    BARS.with(|b| {
        let mut b = b.borrow_mut();
        let bar = b.bars.get_mut(&id)?;
        f(bar);
        Some(bar.clone())
    })
}

pub fn remove_bar(id: BarId) -> Option<Bar> {
    BARS.with(|b| b.borrow_mut().bars.remove(&id))
}

// ---- entities ---------------------------------------------------------------------------

thread_local! {
    /// Entity id -> (UUID, type), for the hooks that name entities by id and have no host.
    static ENTITIES: RefCell<HashMap<u32, (u128, String)>> = RefCell::new(HashMap::new());
}

pub fn entity_joined(id: u32, uuid: u128, kind: &str) {
    ENTITIES.with(|e| e.borrow_mut().insert(id, (uuid, kind.to_owned())));
}

pub fn entity_left(id: u32) {
    ENTITIES.with(|e| e.borrow_mut().remove(&id));
}

pub fn entity(id: u32) -> Option<(u128, String)> {
    ENTITIES.with(|e| e.borrow().get(&id).cloned())
}

// ---- drops ------------------------------------------------------------------------------

pub fn refuse_drops(dimension: &str, pos: [i32; 3]) {
    NO_DROPS.with(|d| d.borrow_mut().insert((dimension.to_owned(), pos)));
}

pub fn drops_refused(dimension: &str, pos: [i32; 3]) -> bool {
    NO_DROPS.with(|d| d.borrow().contains(&(dimension.to_owned(), pos)))
}

/// The tick ended: the breaks it heard are done.
pub fn end_tick() {
    NO_DROPS.with(|d| d.borrow_mut().clear());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn menu(player: u128, window: i32) -> Menu {
        Menu {
            player,
            window,
            rows: 1,
            title: HText::literal("t"),
            items: vec![None; 9],
        }
    }

    #[test]
    fn menus_are_found_by_window_and_go_with_their_player() {
        let a = menu_opened(menu(1, 5));
        let b = menu_opened(menu(2, 7));
        assert_eq!(menu_at(1, 5).map(|(id, _)| id), Some(a));
        assert!(menu_at(1, 7).is_none());
        assert_eq!(menu_of(2), Some(b));
        update_menu(a, |m| m.window = 6);
        assert!(menu_at(1, 5).is_none(), "a re-opened menu has a new window");
        assert_eq!(player_left(1), vec![a]);
        assert!(super::menu(a).is_none() && super::menu(b).is_some());
    }

    #[test]
    fn bars_keep_their_viewers_and_a_uuid_of_their_own() {
        let id = bar_created(NativeBossBar::default());
        let bar = update_bar(id, |b| {
            b.viewers.insert(9);
        })
        .unwrap();
        assert_eq!(bar.id & 0xFFFF_FFFF, u128::from(id));
        player_left(9);
        assert!(remove_bar(id).unwrap().viewers.is_empty());
    }

    #[test]
    fn refused_drops_last_one_tick() {
        refuse_drops("minecraft:overworld", [1, 2, 3]);
        assert!(drops_refused("minecraft:overworld", [1, 2, 3]));
        assert!(!drops_refused("minecraft:the_nether", [1, 2, 3]));
        end_tick();
        assert!(!drops_refused("minecraft:overworld", [1, 2, 3]));
    }
}
