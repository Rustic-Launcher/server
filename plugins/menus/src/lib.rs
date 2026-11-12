//! menus: a chest-menu demo built on the SDK's `Menu` builder, and a recipe book of reusable
//! patterns (see README.md).
//!
//! `/menu` opens a 3-row main menu:
//! * **Server info** (live: refreshed every second while open, the title too);
//! * **Warps**: a selector running console commands, with an optional confirmation dialog;
//! * **Settings**: toggles saved per player; lore and enchantment glint follow the state;
//!   left-click toggles, right-click explains, shift-click restores the default;
//! * **Players**: a paginated list of player heads (`minecraft:player_head` + profile
//!   component) updated in place (items and title) when paging or when players come and go.
//!
//! Every menu has back/close buttons; clicks never move items (cancelled by the SDK, and again
//! by a guard handler); a player's menu is closed and forgotten when they disconnect.

mod config;
mod confirm;
mod lang;
mod ui;

use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;

use rustic::permissions::Permissions;
use rustic::prelude::*;
use rustic::text::escape_mini;

use crate::config::{DEFAULT_CONFIG, MenusConfig, SETTINGS_KEY, Settings};
use crate::confirm::Confirm;
use crate::lang::{Lang, Msg, tr, tr_mini};

/// Permission nodes.
pub mod perms {
    /// `/menu`.
    pub const USE: &str = "menus.use";
    /// Shift-clicking a head teleports to that player.
    pub const TELEPORT: &str = "menus.teleport";
}

/// Main menu slots.
mod main_slots {
    use crate::ui::slot;
    pub const INFO: u32 = slot(1, 1);
    pub const WARPS: u32 = slot(1, 3);
    pub const SETTINGS: u32 = slot(1, 5);
    pub const PLAYERS: u32 = slot(1, 7);
    pub const CLOSE: u32 = slot(2, 4);
}

/// Player list layout: 5 rows of heads, then the navigation row.
mod list_slots {
    use crate::ui::slot;
    pub const PER_PAGE: usize = 45;
    pub const BACK: u32 = slot(5, 0);
    pub const PREV: u32 = slot(5, 3);
    pub const PAGE: u32 = slot(5, 4);
    pub const NEXT: u32 = slot(5, 5);
    pub const CLOSE: u32 = slot(5, 8);
}

/// What a player is looking at.
#[derive(Debug, Clone, PartialEq, Eq)]
enum View {
    /// What the live items show (title JSON, info item SNBT), to refresh only on change: on
    /// some servers a title change reopens the window.
    Main {
        title: String,
        info: String,
    },
    Warps,
    Settings,
    /// The page (0-based) and the players shown, to refresh only on change.
    Players {
        page: usize,
        shown: Vec<Player>,
    },
    Confirm,
}

#[derive(Debug, Clone)]
struct Session {
    handle: MenuHandle,
    view: View,
}

#[rustic::plugin]
#[derive(Default)]
struct Menus {
    cfg: MenusConfig,
    sessions: HashMap<Player, Session>,
}

/// A click handler that reaches the plugin state.
fn on_click(
    mut f: impl FnMut(&mut Menus, &mut MenuClick) -> Result<()> + 'static,
) -> impl FnMut(&mut MenuClick) + 'static {
    move |click: &mut MenuClick| {
        if !is_button_click(click.kind()) {
            return;
        }
        let player = click.player();
        if let Err(e) = with_plugin(|m: &mut Menus| f(m, click)) {
            player.send_message(Text::new(e.to_string()).color(NamedColor::Red));
        }
    }
}

/// Buttons react to left/right clicks (with or without shift) only: drops, number keys, middle
/// and double clicks do nothing (they stay cancelled like every click).
fn is_button_click(kind: ClickKind) -> bool {
    matches!(
        kind,
        ClickKind::Left | ClickKind::Right | ClickKind::ShiftLeft | ClickKind::ShiftRight
    )
}

/// The close button's handler.
fn close_button(click: &mut MenuClick) {
    if is_button_click(click.kind()) {
        click.close();
    }
}

/// Runs `f` with the plugin state, reporting errors to the player (for callbacks).
fn with_menus(player: Player, f: impl FnOnce(&mut Menus) -> Result<()>) {
    if let Err(e) = with_plugin(f) {
        player.send_message(Text::new(e.to_string()).color(NamedColor::Red));
    }
}

impl Plugin for Menus {
    fn load(&mut self, _ctx: &mut Ctx) -> Result<()> {
        Permissions::declare(perms::USE)
            .default(PermissionDefault::Allow)
            .description("Open /menu")
            .register();
        Permissions::declare(perms::TELEPORT)
            .default(PermissionDefault::Op)
            .description("Teleport to players from the player list")
            .register();
        Ok(())
    }

    fn enable(&mut self, ctx: &mut Ctx) -> Result<()> {
        if !ctx.is_main() {
            return Ok(());
        }
        self.cfg = rustic::config::Config::new()
            .defaults(DEFAULT_CONFIG)
            .load()?;
        let open = |f: fn(&mut Menus, Player) -> Result<()>| {
            move |cmd: &mut CommandCtx| -> CommandResult {
                let p = cmd.player()?;
                with_plugin(|m: &mut Menus| f(m, p))?;
                Ok(())
            }
        };
        ctx.command(
            Command::new("menu")
                .description("Open the server menu")
                .alias("gui")
                .permission(perms::USE)
                .players_only()
                .route("", open(Menus::open_main))
                .route("warps", open(Menus::open_warps))
                .route("settings", open(Menus::open_settings))
                .route("players [page:integer(1..1000)]", |cmd| {
                    let p = cmd.player()?;
                    let page = cmd.opt_arg::<u32>("page")?.unwrap_or(1) as usize;
                    with_plugin(|m: &mut Menus| m.open_players(p, page.saturating_sub(1)))?;
                    Ok(())
                }),
        )?;
        Scheduler::run_timer(
            self.cfg.refresh_ticks.max(1),
            self.cfg.refresh_ticks.max(1),
            |_| with_plugin(Menus::refresh),
        );
        Ok(())
    }

    fn disable(&mut self, _ctx: &mut Ctx) {
        for (_, s) in self.sessions.drain() {
            s.handle.close();
        }
    }
}

impl Menus {
    fn lang(&self, player: Player) -> Lang {
        Lang::from_code(&self.cfg.language).unwrap_or_else(|| {
            player
                .locale()
                .ok()
                .and_then(|l| Lang::from_code(&l))
                .unwrap_or(Lang::En)
        })
    }

    /// Opens `menu` for `player` and remembers it; the session ends when that menu closes.
    fn show(&mut self, player: Player, menu: Menu, view: View) -> Result<MenuHandle> {
        // The close handler is built before the id exists: share it through a cell.
        let id = Rc::new(Cell::new(None));
        let id_in_close = id.clone();
        let handle = menu
            .on_close(move |p| {
                if let Some(closed) = id_in_close.get() {
                    with_plugin(|m: &mut Menus| m.closed(p, closed));
                }
            })
            .open(&player)?;
        id.set(Some(handle.id()));
        self.sessions.insert(player, Session { handle, view });
        Ok(handle)
    }

    /// A menu closed: forget the session unless another menu replaced it meanwhile.
    fn closed(&mut self, player: Player, menu: u32) {
        if self
            .sessions
            .get(&player)
            .is_some_and(|s| s.handle.id() == menu)
        {
            self.sessions.remove(&player);
        }
    }

    // ---- main menu ----------------------------------------------------------------------

    fn info_item(lang: Lang) -> ItemStack {
        let stats = Server::tick_stats();
        let online = Player::online().len().to_string();
        let max = Server::max_players().to_string();
        let lines = [
            tr_mini(lang, Msg::InfoVersion, &[("version", &Server::version())]),
            tr_mini(
                lang,
                Msg::InfoPlayers,
                &[("online", &online), ("max", &max)],
            ),
            tr_mini(
                lang,
                Msg::InfoTps,
                &[
                    ("tps", &format!("{:.1}", stats.tps)),
                    ("mspt", &format!("{:.1}", stats.mspt)),
                ],
            ),
            tr_mini(
                lang,
                Msg::InfoPlugins,
                &[("count", &Server::plugins().len().to_string())],
            ),
            String::new(),
            tr_mini(lang, Msg::InfoHint, &[]),
        ];
        ui::button(
            "minecraft:nether_star",
            &tr_mini(lang, Msg::InfoName, &[]),
            &lines,
        )
    }

    /// The server info button: left prints a summary, right refreshes the item. The item is
    /// replaced every second with this same handler (`set_display` would drop it).
    fn info_handler() -> impl FnMut(&mut MenuClick) + 'static {
        on_click(|m, c| {
            let lang = m.lang(c.player());
            match c.kind() {
                // Right-click refreshes the item in place.
                ClickKind::Right | ClickKind::ShiftRight => c.menu().set_item(
                    main_slots::INFO,
                    Self::info_item(lang),
                    Self::info_handler(),
                )?,
                _ => {
                    let stats = Server::tick_stats();
                    c.player().send_message(tr(
                        lang,
                        Msg::ServerInfoChat,
                        &[
                            ("version", &Server::version()),
                            ("online", &Player::online().len().to_string()),
                            ("max", &Server::max_players().to_string()),
                            ("tps", &format!("{:.1}", stats.tps)),
                            ("plugins", &Server::plugins().len().to_string()),
                        ],
                    ));
                }
            }
            Ok(())
        })
    }

    fn main_title(lang: Lang) -> Text {
        tr(
            lang,
            Msg::MainTitle,
            &[("online", &Player::online().len().to_string())],
        )
    }

    fn open_main(&mut self, player: Player) -> Result<()> {
        let lang = self.lang(player);
        let online = Player::online().len().to_string();
        let name = player.name().unwrap_or_default();
        let hint = tr_mini(lang, Msg::OpenHint, &[]);
        let menu = Menu::new(3, Self::main_title(lang))
            .item(
                main_slots::INFO,
                Self::info_item(lang),
                Self::info_handler(),
            )
            .item(
                main_slots::WARPS,
                ui::button(
                    "minecraft:ender_pearl",
                    &tr_mini(lang, Msg::WarpsName, &[]),
                    &[tr_mini(lang, Msg::WarpsLore, &[]), hint.clone()],
                ),
                on_click(|m, c| m.open_warps(c.player())),
            )
            .item(
                main_slots::SETTINGS,
                ui::button(
                    "minecraft:comparator",
                    &tr_mini(lang, Msg::SettingsName, &[]),
                    &[tr_mini(lang, Msg::SettingsLore, &[]), hint.clone()],
                ),
                on_click(|m, c| m.open_settings(c.player())),
            )
            .item(
                main_slots::PLAYERS,
                ui::player_head(&name, Some(player.uuid()))
                    .name(tr(lang, Msg::PlayersName, &[]))
                    .lore([
                        tr(lang, Msg::PlayersLore, &[("online", &online)]),
                        Text::mini(&hint),
                    ]),
                on_click(|m, c| m.open_players(c.player(), 0)),
            )
            .item(main_slots::CLOSE, ui::close_button(lang), close_button)
            .fill(ui::filler());
        let view = View::Main {
            title: Self::main_title(lang).to_json(),
            info: Self::info_item(lang).components_snbt(),
        };
        self.show(player, menu, view)?;
        Ok(())
    }

    /// The back/close row of a submenu with `rows` rows.
    fn nav(menu: Menu, rows: u8, lang: Lang) -> Menu {
        let last = u32::from(rows) - 1;
        menu.item(
            ui::slot(last, 0),
            ui::back_button(lang),
            on_click(|m, c| m.open_main(c.player())),
        )
        .item(ui::slot(last, 4), ui::close_button(lang), close_button)
    }

    // ---- warps --------------------------------------------------------------------------

    fn open_warps(&mut self, player: Player) -> Result<()> {
        let lang = self.lang(player);
        let rows = ui::rows_for_bordered(self.cfg.warps.len());
        let mut menu = Menu::new(rows, tr(lang, Msg::WarpsTitle, &[]));
        if self.cfg.warps.is_empty() {
            menu = menu.display(
                ui::slot(1, 4),
                ItemStack::new("minecraft:structure_void").name(tr(lang, Msg::NoWarps, &[])),
            );
        }
        let max = (usize::from(rows) - 2) * 7;
        for (i, warp) in self.cfg.warps.iter().enumerate().take(max) {
            let mut lore = vec![format!("<gray>{}", escape_mini(&warp.description))];
            lore.push(String::new());
            lore.push(tr_mini(lang, Msg::WarpHint, &[]));
            let icon = if warp.icon.is_empty() {
                "minecraft:ender_pearl"
            } else {
                &warp.icon
            };
            menu = menu.item(
                ui::bordered_slot(i),
                ui::button(icon, &warp.name, &lore),
                on_click(move |m, c| m.warp_clicked(c, i)),
            );
        }
        let menu = Self::nav(menu, rows, lang).fill(ui::filler());
        self.show(player, menu, View::Warps)?;
        Ok(())
    }

    fn warp_clicked(&mut self, click: &mut MenuClick, index: usize) -> Result<()> {
        let player = click.player();
        let lang = self.lang(player);
        let Some(warp) = self.cfg.warps.get(index).cloned() else {
            return Ok(());
        };
        let name = Text::mini(&warp.name).plain();
        match click.kind() {
            ClickKind::Right | ClickKind::ShiftRight => {
                player.send_message(tr(
                    lang,
                    Msg::WarpDetails,
                    &[("name", &name), ("description", &warp.description)],
                ));
            }
            _ if warp.confirm => {
                let question = ui::button(
                    if warp.icon.is_empty() {
                        "minecraft:ender_pearl"
                    } else {
                        &warp.icon
                    },
                    &tr_mini(lang, Msg::WarpQuestion, &[("name", &name)]),
                    &[format!("<gray>{}", escape_mini(&warp.description))],
                );
                let handle = Confirm::new(lang)
                    .title(tr(lang, Msg::WarpsTitle, &[]))
                    .question(question)
                    .on_yes(move |p| with_menus(p, |m| m.run_warp(p, index)))
                    .on_no(|p| with_menus(p, |m| m.open_warps(p)))
                    .open(&player)?;
                self.sessions.insert(
                    player,
                    Session {
                        handle,
                        view: View::Confirm,
                    },
                );
            }
            _ => {
                click.close();
                self.run_warp(player, index)?;
            }
        }
        Ok(())
    }

    fn run_warp(&mut self, player: Player, index: usize) -> Result<()> {
        let Some(warp) = self.cfg.warps.get(index) else {
            return Ok(());
        };
        let name = player.name().unwrap_or_default();
        for command in &warp.commands {
            let line = command.replace("{player}", &name);
            if !Server::dispatch_command(&line) {
                warn!("warp command failed: {line}");
            }
        }
        let lang = self.lang(player);
        player.send_message(tr(
            lang,
            Msg::WarpDone,
            &[("name", &Text::mini(&warp.name).plain())],
        ));
        Ok(())
    }

    // ---- settings -----------------------------------------------------------------------

    fn settings(player: Player) -> Settings {
        player
            .data()
            .get(SETTINGS_KEY)
            .ok()
            .flatten()
            .unwrap_or_default()
    }

    fn toggle_item(&self, player: Player, index: usize) -> Option<ItemStack> {
        let t = self.cfg.toggles.get(index)?;
        let on = self.cfg.value(&Self::settings(player), &t.id);
        let icon = if t.icon.is_empty() {
            "minecraft:lever"
        } else {
            &t.icon
        };
        Some(ui::toggle_item(
            icon,
            &t.name,
            &t.description,
            on,
            self.lang(player),
        ))
    }

    fn open_settings(&mut self, player: Player) -> Result<()> {
        let lang = self.lang(player);
        let rows = ui::rows_for_bordered(self.cfg.toggles.len());
        let mut menu = Menu::new(rows, tr(lang, Msg::SettingsTitle, &[]));
        let max = (usize::from(rows) - 2) * 7;
        for i in 0..self.cfg.toggles.len().min(max) {
            if let Some(item) = self.toggle_item(player, i) {
                menu = menu.item(ui::bordered_slot(i), item, Self::toggle_handler(i));
            }
        }
        let last = u32::from(rows) - 1;
        let reset = ui::button(
            "minecraft:tnt",
            &tr_mini(lang, Msg::ResetAllName, &[]),
            &[tr_mini(lang, Msg::ResetAllLore, &[])],
        );
        let menu = Self::nav(menu, rows, lang)
            .item(
                ui::slot(last, 8),
                reset,
                on_click(|m, c| m.confirm_reset(c.player())),
            )
            .fill(ui::filler());
        self.show(player, menu, View::Settings)?;
        Ok(())
    }

    fn toggle_handler(index: usize) -> impl FnMut(&mut MenuClick) + 'static {
        on_click(move |m, c| m.toggle_clicked(c, index))
    }

    fn toggle_clicked(&mut self, click: &mut MenuClick, index: usize) -> Result<()> {
        let player = click.player();
        let lang = self.lang(player);
        let Some(t) = self.cfg.toggles.get(index).cloned() else {
            return Ok(());
        };
        let mut settings = Self::settings(player);
        match click.kind() {
            ClickKind::Right => {
                for line in &t.description {
                    player.send_message(Text::mini(line));
                }
                return Ok(());
            }
            ClickKind::ShiftLeft | ClickKind::ShiftRight => {
                settings.remove(&t.id);
            }
            _ => {
                let now = !self.cfg.value(&settings, &t.id);
                settings.insert(t.id.clone(), now);
            }
        }
        player.data().set(SETTINGS_KEY, &settings)?;
        let on = self.cfg.value(&settings, &t.id);
        let state = tr_mini(lang, if on { Msg::On } else { Msg::Off }, &[]);
        player.send_message(Text::mini(
            &crate::lang::template(lang, Msg::Toggled)
                .replace("{name}", &escape_mini(&t.name))
                .replace("{state}", &state),
        ));
        player.sound("minecraft:ui.button.click", 0.6, if on { 1.4 } else { 0.8 });
        if let Some(item) = self.toggle_item(player, index) {
            click
                .menu()
                .set_item(click.slot(), item, Self::toggle_handler(index))?;
        }
        Ok(())
    }

    fn confirm_reset(&mut self, player: Player) -> Result<()> {
        let lang = self.lang(player);
        let question = ui::button(
            "minecraft:tnt",
            &tr_mini(lang, Msg::ResetAllName, &[]),
            &[tr_mini(lang, Msg::ResetAllLore, &[])],
        );
        let handle = Confirm::new(lang)
            .question(question)
            .on_yes(|p| {
                p.data().delete(SETTINGS_KEY);
                with_menus(p, |m| {
                    p.send_message(tr(m.lang(p), Msg::ResetDone, &[]));
                    m.open_settings(p)
                });
            })
            .on_no(|p| {
                with_menus(p, |m| {
                    p.send_message(tr(m.lang(p), Msg::Cancelled, &[]));
                    m.open_settings(p)
                })
            })
            .open(&player)?;
        self.sessions.insert(
            player,
            Session {
                handle,
                view: View::Confirm,
            },
        );
        Ok(())
    }

    // ---- players ------------------------------------------------------------------------

    fn sorted_online() -> Vec<Player> {
        let mut list: Vec<(String, Player)> = Player::online()
            .into_iter()
            .map(|p| (p.name().unwrap_or_default().to_lowercase(), p))
            .collect();
        list.sort();
        list.into_iter().map(|(_, p)| p).collect()
    }

    fn open_players(&mut self, player: Player, page: usize) -> Result<()> {
        let lang = self.lang(player);
        let menu = Menu::new(6, tr(lang, Msg::PlayersName, &[]))
            .item(
                list_slots::BACK,
                ui::back_button(lang),
                on_click(|m, c| m.open_main(c.player())),
            )
            .item(list_slots::CLOSE, ui::close_button(lang), close_button);
        let handle = self.show(
            player,
            menu,
            View::Players {
                page,
                shown: Vec::new(),
            },
        )?;
        self.render_players(player, handle, page, true)
    }

    /// Fills the player list page in place (heads, navigation, title). Unless `force`, does
    /// nothing when the page shows the same players already.
    fn render_players(
        &mut self,
        viewer: Player,
        handle: MenuHandle,
        page: usize,
        force: bool,
    ) -> Result<()> {
        let lang = self.lang(viewer);
        let list = Self::sorted_online();
        let per = list_slots::PER_PAGE;
        let pages = ui::page_count(list.len(), per);
        let page = ui::clamp_page(page, list.len(), per);
        let shown: Vec<Player> = list[ui::page_range(list.len(), per, page)].to_vec();
        if let Some(Session {
            view: View::Players { page: p, shown: s },
            ..
        }) = self.sessions.get(&viewer)
            && !force
            && *p == page
            && *s == shown
        {
            return Ok(());
        }
        for slot in 0..per {
            let slot32 = slot as u32;
            match shown.get(slot) {
                Some(&target) => {
                    let name = target.name().unwrap_or_default();
                    let world = target
                        .world()
                        .map(|w| w.id().trim_start_matches("minecraft:").to_owned())
                        .unwrap_or_default();
                    let ping = target.ping().map(|p| p.to_string()).unwrap_or_default();
                    let head = ui::player_head(&name, Some(target.uuid()))
                        .name(Text::mini(&format!("<gold>{}", escape_mini(&name))))
                        .lore([
                            tr(lang, Msg::HeadLore1, &[("world", &world)]),
                            tr(lang, Msg::HeadLore2, &[("ping", &ping)]),
                            Text::new(""),
                            tr(lang, Msg::HeadHints, &[]),
                        ]);
                    handle.set_item(
                        slot32,
                        head,
                        on_click(move |m, c| m.head_clicked(c, target)),
                    )?;
                }
                None => handle.set_display(slot32, ItemStack::empty())?,
            }
        }
        let (page_s, pages_s) = ((page + 1).to_string(), pages.to_string());
        if page > 0 {
            let item =
                ItemStack::new("minecraft:spectral_arrow").name(tr(lang, Msg::PrevPage, &[]));
            handle.set_item(
                list_slots::PREV,
                item,
                on_click(move |m, c| m.render_players(c.player(), c.menu(), page - 1, true)),
            )?;
        } else {
            handle.set_display(list_slots::PREV, ui::filler())?;
        }
        if page + 1 < pages {
            let item =
                ItemStack::new("minecraft:spectral_arrow").name(tr(lang, Msg::NextPage, &[]));
            handle.set_item(
                list_slots::NEXT,
                item,
                on_click(move |m, c| m.render_players(c.player(), c.menu(), page + 1, true)),
            )?;
        } else {
            handle.set_display(list_slots::NEXT, ui::filler())?;
        }
        handle.set_display(
            list_slots::PAGE,
            ItemStack::new("minecraft:paper")
                .name(tr(
                    lang,
                    Msg::PageInfo,
                    &[("page", &page_s), ("pages", &pages_s)],
                ))
                .count((page + 1).min(64) as u32),
        )?;
        for s in [
            list_slots::BACK + 1,
            list_slots::BACK + 2,
            list_slots::NEXT + 1,
            list_slots::NEXT + 2,
        ] {
            handle.set_display(s, ui::filler())?;
        }
        handle.set_title(tr(
            lang,
            Msg::PlayersTitle,
            &[
                ("online", &list.len().to_string()),
                ("page", &page_s),
                ("pages", &pages_s),
            ],
        ))?;
        if let Some(s) = self.sessions.get_mut(&viewer)
            && s.handle == handle
        {
            s.view = View::Players { page, shown };
        }
        Ok(())
    }

    fn head_clicked(&mut self, click: &mut MenuClick, target: Player) -> Result<()> {
        let viewer = click.player();
        let lang = self.lang(viewer);
        let name = target.name().unwrap_or_default();
        if !target.is_online() {
            viewer.send_message(tr(lang, Msg::PlayerGone, &[("name", &name)]));
            return Ok(());
        }
        match click.kind() {
            ClickKind::Right => {
                viewer.send_message(
                    tr(lang, Msg::MessageLink, &[("name", &name)])
                        .suggest_command(format!("/msg {name} ")),
                );
            }
            ClickKind::ShiftLeft | ClickKind::ShiftRight => {
                if target == viewer {
                    viewer.send_message(tr(lang, Msg::ThatsYou, &[]));
                } else if !viewer.has_permission(perms::TELEPORT) {
                    viewer.send_message(tr(lang, Msg::NoTeleport, &[]));
                } else {
                    let page = match self.sessions.get(&viewer) {
                        Some(Session {
                            view: View::Players { page, .. },
                            ..
                        }) => *page,
                        _ => 0,
                    };
                    let handle = Confirm::new(lang)
                        .question(ui::player_head(&name, Some(target.uuid())).name(tr(
                            lang,
                            Msg::TeleportQuestion,
                            &[("name", &name)],
                        )))
                        .on_yes(move |p| {
                            with_menus(p, |m| {
                                let lang = m.lang(p);
                                let n = target.name().unwrap_or_default();
                                match target.location() {
                                    Ok(to) => {
                                        p.teleport(&to)?;
                                        p.send_message(tr(lang, Msg::Teleported, &[("name", &n)]));
                                    }
                                    Err(_) => {
                                        p.send_message(tr(lang, Msg::PlayerGone, &[("name", &n)]))
                                    }
                                }
                                Ok(())
                            })
                        })
                        .on_no(move |p| with_menus(p, |m| m.open_players(p, page)))
                        .open(&viewer)?;
                    self.sessions.insert(
                        viewer,
                        Session {
                            handle,
                            view: View::Confirm,
                        },
                    );
                }
            }
            _ => {
                let at = target.location()?;
                viewer.send_message(tr(
                    lang,
                    Msg::PlayerInfo,
                    &[
                        ("name", &name),
                        ("world", at.world.trim_start_matches("minecraft:")),
                        ("x", &format!("{:.0}", at.x)),
                        ("y", &format!("{:.0}", at.y)),
                        ("z", &format!("{:.0}", at.z)),
                        (
                            "ping",
                            &target.ping().map(|p| p.to_string()).unwrap_or_default(),
                        ),
                    ],
                ));
            }
        }
        Ok(())
    }

    // ---- live refresh -------------------------------------------------------------------

    fn refresh(&mut self) {
        let sessions: Vec<(Player, Session)> =
            self.sessions.iter().map(|(p, s)| (*p, s.clone())).collect();
        for (player, s) in sessions {
            let lang = self.lang(player);
            let result = match s.view {
                View::Main { title, info } => {
                    let (new_title, new_info) = (Self::main_title(lang), Self::info_item(lang));
                    let (title_json, info_snbt) = (new_title.to_json(), new_info.components_snbt());
                    let mut result = Ok(());
                    if info_snbt != info {
                        result =
                            s.handle
                                .set_item(main_slots::INFO, new_info, Self::info_handler());
                    }
                    if result.is_ok() && title_json != title {
                        result = s.handle.set_title(new_title);
                    }
                    if let Some(cur) = self.sessions.get_mut(&player)
                        && cur.handle == s.handle
                    {
                        cur.view = View::Main {
                            title: title_json,
                            info: info_snbt,
                        };
                    }
                    result
                }
                View::Players { page, .. } => self.render_players(player, s.handle, page, false),
                _ => Ok(()),
            };
            if result.is_err() {
                // The menu is gone (closed without an event reaching us): forget it.
                self.sessions.remove(&player);
            }
        }
    }
}

#[rustic::handlers]
impl Menus {
    /// Closing on disconnect: the menu and its session go away with the player.
    #[event_handler(priority = "monitor")]
    fn on_quit(&mut self, event: &mut PlayerQuitEvent) {
        if let Some(s) = self.sessions.remove(&event.player()) {
            s.handle.close();
        }
    }

    /// Belt and braces: whatever another plugin did, a click in one of our menus never moves
    /// an item (the SDK already cancels by default).
    #[event_handler(priority = "highest")]
    fn guard_clicks(&mut self, event: &mut InventoryClickEvent) {
        if let Some(menu) = event.menu()
            && self.sessions.values().any(|s| s.handle.id() == menu)
        {
            event.cancel();
        }
    }
}
