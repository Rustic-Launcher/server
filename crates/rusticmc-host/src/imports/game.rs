//! Game-facing interfaces: `server`, `players`, `inventories`, `menus`, `worlds`,
//! `scoreboards`, `bossbars`, `tablist`, `messaging`. Main instance only (except the few
//! functions the core or a snapshot can answer off-thread).

use super::wit;
use crate::bindings::rustic::api::{
    bossbars as wb, inventories as wi, menus as wm, messaging as wmsg, players as wpl,
    scoreboards as wsb, server as wsv, tablist as wtl, types as wt, worlds as ww,
};
use crate::convert::Cv;
use crate::state::HostState;
use rusticmc_core::model as m;

type R<T> = Result<T, wt::HostError>;

// ---------------------------------------------------------------------------- server

impl wsv::Host for HostState {
    fn broadcast(&mut self, message: wt::Text) {
        let t: m::Text = message.cv();
        self.game_or("server.broadcast", (), |b| b.server_broadcast(&t));
    }

    fn dispatch_command(&mut self, command: String) -> bool {
        self.game_or("server.dispatch-command", false, |b| {
            b.server_dispatch_command(&command)
        })
    }

    fn get_tick_stats(&mut self) -> wsv::TickStats {
        self.game_or("server.get-tick-stats", m::TickStats::default(), |b| {
            b.server_get_tick_stats()
        })
        .cv()
    }

    fn max_players(&mut self) -> u32 {
        self.game_or("server.max-players", 0, |b| b.server_max_players())
    }

    fn motd(&mut self) -> wt::Text {
        self.game_or("server.motd", m::Text::default(), |b| b.server_motd())
            .cv()
    }

    fn version(&mut self) -> String {
        let fallback = format!("Rustic {}", env!("CARGO_PKG_VERSION"));
        if !self.is_main {
            return fallback;
        }
        rusticmc_core::context::backend_call(|b| b.server_version()).unwrap_or(fallback)
    }

    fn plugins(&mut self) -> Vec<wsv::PluginMeta> {
        match self.main_api() {
            Ok(api) => api.plugins().cv(),
            Err(_) => {
                self.warn_once(
                    "server.plugins",
                    "main instance only; returning an empty list in the async instance",
                );
                Vec::new()
            }
        }
    }

    fn self_meta(&mut self) -> wsv::PluginMeta {
        match self.main_api() {
            Ok(api) => api.self_meta().cv(),
            // The SDK asks once per instance (its plugin id): answer from the manifest.
            Err(_) => wsv::PluginMeta {
                id: self.meta.id.to_string(),
                name: self.meta.name.clone(),
                version: self.meta.version.clone(),
                enabled: self.shared.is_enabled(&self.plugin),
            },
        }
    }

    fn offline_player(&mut self, name: String) -> Option<wt::Uuid> {
        self.game_or("server.offline-player", None, |b| {
            b.server_offline_player(&name)
        })
        .cv()
    }

    fn operators(&mut self) -> Vec<wt::Uuid> {
        self.game_or("server.operators", Vec::new(), |b| b.server_operators())
            .cv()
    }
}

// ---------------------------------------------------------------------------- players

impl wpl::Host for HostState {
    fn online(&mut self) -> Vec<wt::Uuid> {
        self.game_or("players.online", Vec::new(), |b| b.players_online())
            .cv()
    }

    fn name(&mut self, player: wt::Uuid) -> Option<String> {
        self.game_or("players.name", None, |b| b.players_name(player.cv()))
    }

    fn by_name(&mut self, name: String) -> Option<wt::Uuid> {
        self.game_or("players.by-name", None, |b| b.players_by_name(&name))
            .cv()
    }

    fn is_online(&mut self, player: wt::Uuid) -> bool {
        self.game_or("players.is-online", false, |b| {
            b.players_is_online(player.cv())
        })
    }

    fn get_location(&mut self, player: wt::Uuid) -> R<wt::Location> {
        wit(self.game(|b| b.players_get_location(player.cv())))
    }

    fn address(&mut self, player: wt::Uuid) -> R<String> {
        wit(self.game(|b| b.players_address(player.cv())))
    }

    fn ping(&mut self, player: wt::Uuid) -> R<u32> {
        wit(self.game(|b| b.players_ping(player.cv())))
    }

    fn locale(&mut self, player: wt::Uuid) -> R<String> {
        wit(self.game(|b| b.players_locale(player.cv())))
    }

    fn send_message(&mut self, player: wt::Uuid, message: wt::Text) -> R<()> {
        let t: m::Text = message.cv();
        wit(self.game(|b| b.players_send_message(player.cv(), &t)))
    }

    fn send_action_bar(&mut self, player: wt::Uuid, message: wt::Text) -> R<()> {
        let t: m::Text = message.cv();
        wit(self.game(|b| b.players_send_action_bar(player.cv(), &t)))
    }

    fn send_title(
        &mut self,
        player: wt::Uuid,
        title: wt::Text,
        subtitle: wt::Text,
        fade_in: u32,
        stay: u32,
        fade_out: u32,
    ) -> R<()> {
        let (t, s): (m::Text, m::Text) = (title.cv(), subtitle.cv());
        wit(self.game(|b| b.players_send_title(player.cv(), &t, &s, fade_in, stay, fade_out)))
    }

    fn play_sound(&mut self, player: wt::Uuid, sound: String, volume: f32, pitch: f32) -> R<()> {
        wit(self.game(|b| b.players_play_sound(player.cv(), &sound, volume, pitch)))
    }

    fn teleport(&mut self, player: wt::Uuid, to: wt::Location) -> R<()> {
        let to: m::Location = to.cv();
        if !to.is_finite() {
            return Err(wt::HostError::Invalid("non-finite coordinates".into()));
        }
        wit(self.game(|b| b.players_teleport(player.cv(), &to)))
    }

    fn get_game_mode(&mut self, player: wt::Uuid) -> R<wt::GameMode> {
        wit(self.game(|b| b.players_get_game_mode(player.cv())))
    }

    fn set_game_mode(&mut self, player: wt::Uuid, mode: wt::GameMode) -> R<()> {
        wit(self.game(|b| b.players_set_game_mode(player.cv(), mode.cv())))
    }

    fn health(&mut self, player: wt::Uuid) -> R<f32> {
        wit(self.game(|b| b.players_health(player.cv())))
    }

    fn set_health(&mut self, player: wt::Uuid, health: f32) -> R<()> {
        if !health.is_finite() {
            return Err(wt::HostError::Invalid("health must be finite".into()));
        }
        wit(self.game(|b| b.players_set_health(player.cv(), health)))
    }

    fn max_health(&mut self, player: wt::Uuid) -> R<f32> {
        wit(self.game(|b| b.players_max_health(player.cv())))
    }

    fn food_level(&mut self, player: wt::Uuid) -> R<u32> {
        wit(self.game(|b| b.players_food_level(player.cv())))
    }

    fn set_food_level(&mut self, player: wt::Uuid, food: u32) -> R<()> {
        wit(self.game(|b| b.players_set_food_level(player.cv(), food)))
    }

    fn saturation(&mut self, player: wt::Uuid) -> R<f32> {
        wit(self.game(|b| b.players_saturation(player.cv())))
    }

    fn set_saturation(&mut self, player: wt::Uuid, saturation: f32) -> R<()> {
        if !saturation.is_finite() {
            return Err(wt::HostError::Invalid("saturation must be finite".into()));
        }
        wit(self.game(|b| b.players_set_saturation(player.cv(), saturation)))
    }

    fn experience_level(&mut self, player: wt::Uuid) -> R<u32> {
        wit(self.game(|b| b.players_experience_level(player.cv())))
    }

    fn set_experience_level(&mut self, player: wt::Uuid, level: u32) -> R<()> {
        wit(self.game(|b| b.players_set_experience_level(player.cv(), level)))
    }

    fn set_flying(&mut self, player: wt::Uuid, allow: bool, flying: bool) -> R<()> {
        wit(self.game(|b| b.players_set_flying(player.cv(), allow, flying)))
    }

    fn kick(&mut self, player: wt::Uuid, reason: wt::Text) -> R<()> {
        let t: m::Text = reason.cv();
        wit(self.game(|b| b.players_kick(player.cv(), &t)))
    }

    fn is_op(&mut self, player: wt::Uuid) -> bool {
        self.game_or("players.is-op", false, |b| b.players_is_op(player.cv()))
    }

    fn set_op(&mut self, player: wt::Uuid, op: bool) -> R<()> {
        wit(self.game(|b| b.players_set_op(player.cv(), op)))
    }

    fn get_data(&mut self, player: wt::Uuid, key: String) -> Option<Vec<u8>> {
        self.async_api.player_data_get(player.cv(), &key)
    }

    fn set_data(&mut self, player: wt::Uuid, key: String, value: Vec<u8>) {
        self.async_api.player_data_set(player.cv(), &key, value);
    }

    fn delete_data(&mut self, player: wt::Uuid, key: String) -> bool {
        self.async_api.player_data_delete(player.cv(), &key)
    }
}

// ---------------------------------------------------------------------------- inventories

impl wi::Host for HostState {
    fn get_slot(&mut self, player: wt::Uuid, slot: u32) -> R<wt::ItemStack> {
        wit(self.game(|b| b.inventories_get_slot(player.cv(), slot)))
    }

    fn set_slot(&mut self, player: wt::Uuid, slot: u32, item: wt::ItemStack) -> R<()> {
        let item: m::ItemStack = item.cv();
        wit(self.game(|b| b.inventories_set_slot(player.cv(), slot, &item)))
    }

    fn contents(&mut self, player: wt::Uuid) -> R<Vec<wt::ItemStack>> {
        wit(self.game(|b| b.inventories_contents(player.cv())))
    }

    fn give(&mut self, player: wt::Uuid, item: wt::ItemStack) -> R<wt::ItemStack> {
        let item: m::ItemStack = item.cv();
        wit(self.game(|b| b.inventories_give(player.cv(), &item)))
    }

    fn take(&mut self, player: wt::Uuid, id: String, count: u32) -> R<u32> {
        wit(self.game(|b| b.inventories_take(player.cv(), &id, count)))
    }

    fn clear(&mut self, player: wt::Uuid) -> R<()> {
        wit(self.game(|b| b.inventories_clear(player.cv())))
    }

    fn held_slot(&mut self, player: wt::Uuid) -> R<u32> {
        wit(self.game(|b| b.inventories_held_slot(player.cv())))
    }

    fn close_container(&mut self, player: wt::Uuid) -> R<()> {
        wit(self.game(|b| b.inventories_close_container(player.cv())))
    }
}

// ---------------------------------------------------------------------------- menus

impl wm::Host for HostState {
    fn open(&mut self, player: wt::Uuid, spec: wm::MenuSpec) -> R<wm::MenuId> {
        let spec: m::MenuSpec = spec.cv();
        wit(self
            .main_api()
            .and_then(|api| api.menus_open(player.cv(), &spec)))
    }

    fn set_item(&mut self, menu: wm::MenuId, slot: u32, item: wt::ItemStack) -> R<()> {
        let item: m::ItemStack = item.cv();
        wit(self
            .main_api()
            .and_then(|api| api.menus_set_item(menu, slot, &item)))
    }

    fn set_title(&mut self, menu: wm::MenuId, title: wt::Text) -> R<()> {
        let t: m::Text = title.cv();
        wit(self
            .main_api()
            .and_then(|api| api.menus_set_title(menu, &t)))
    }

    fn close(&mut self, menu: wm::MenuId) {
        match self.main_api() {
            Ok(api) => api.menus_close(menu),
            Err(_) => self.main_only_ignored("menus.close"),
        }
    }

    fn open_menu(&mut self, player: wt::Uuid) -> Option<wm::MenuId> {
        match self.main_api() {
            Ok(api) => api.menus_open_menu(player.cv()),
            Err(_) => {
                self.main_only_ignored("menus.open-menu");
                None
            }
        }
    }
}

// ---------------------------------------------------------------------------- worlds

impl ww::Host for HostState {
    fn all(&mut self) -> Vec<String> {
        if !self.is_main {
            // Allowed off-thread by the WIT: served from the snapshot the main thread keeps.
            return self.host.snapshot().worlds;
        }
        match rusticmc_core::context::backend_call(|b| b.worlds_all()) {
            Ok(w) => {
                self.host.update_snapshot(|s| s.worlds.clone_from(&w));
                w
            }
            Err(e) => {
                log::debug!(target: "rustic", "[{}] worlds.all: {e}", self.plugin);
                Vec::new()
            }
        }
    }

    fn get_block(&mut self, world: String, pos: wt::BlockPos) -> R<wt::BlockState> {
        wit(self.game(|b| b.worlds_get_block(&world, pos.cv())))
    }

    fn set_block(
        &mut self,
        world: String,
        pos: wt::BlockPos,
        state: wt::BlockState,
        physics: bool,
    ) -> R<bool> {
        let state: m::BlockState = state.cv();
        wit(self.game(|b| b.worlds_set_block(&world, pos.cv(), &state, physics)))
    }

    fn set_blocks(
        &mut self,
        world: String,
        blocks: Vec<(wt::BlockPos, wt::BlockState)>,
        physics: bool,
    ) -> R<u32> {
        let blocks: Vec<(m::BlockPos, m::BlockState)> = blocks.cv();
        wit(self.game(|b| b.worlds_set_blocks(&world, &blocks, physics)))
    }

    fn fill(
        &mut self,
        world: String,
        from: wt::BlockPos,
        to: wt::BlockPos,
        state: wt::BlockState,
    ) -> R<u32> {
        let volume = (i64::from(from.x.abs_diff(to.x)) + 1)
            * (i64::from(from.y.abs_diff(to.y)) + 1)
            * (i64::from(from.z.abs_diff(to.z)) + 1);
        if volume > 32_768 {
            return Err(wt::HostError::Invalid(format!(
                "fill covers {volume} blocks, at most 32768"
            )));
        }
        let state: m::BlockState = state.cv();
        wit(self.game(|b| b.worlds_fill(&world, from.cv(), to.cv(), &state)))
    }

    fn highest_block_y(&mut self, world: String, x: i32, z: i32) -> R<i32> {
        wit(self.game(|b| b.worlds_highest_block_y(&world, x, z)))
    }

    fn is_chunk_loaded(&mut self, world: String, cx: i32, cz: i32) -> bool {
        self.game_or("worlds.is-chunk-loaded", false, |b| {
            b.worlds_is_chunk_loaded(&world, cx, cz)
        })
    }

    fn keep_chunk_loaded(&mut self, world: String, cx: i32, cz: i32, keep: bool) -> R<()> {
        wit(self
            .main_api()
            .and_then(|api| api.worlds_keep_chunk_loaded(&world, cx, cz, keep)))
    }

    fn time(&mut self, world: String) -> R<u64> {
        wit(self.game(|b| b.worlds_time(&world)))
    }

    fn set_time(&mut self, world: String, time: u64) -> R<()> {
        wit(self.game(|b| b.worlds_set_time(&world, time)))
    }

    fn get_weather(&mut self, world: String) -> R<wt::Weather> {
        wit(self.game(|b| b.worlds_get_weather(&world)))
    }

    fn set_weather(&mut self, world: String, weather: wt::Weather, duration_ticks: u32) -> R<()> {
        wit(self.game(|b| b.worlds_set_weather(&world, weather.cv(), duration_ticks)))
    }

    fn spawn(&mut self, world: String) -> R<wt::Location> {
        wit(self.game(|b| b.worlds_spawn(&world)))
    }

    fn set_spawn(&mut self, world: String, at: wt::Location) -> R<()> {
        let at: m::Location = at.cv();
        if !at.is_finite() {
            return Err(wt::HostError::Invalid("non-finite coordinates".into()));
        }
        wit(self.game(|b| b.worlds_set_spawn(&world, &at)))
    }

    fn spawn_entity(
        &mut self,
        world: String,
        kind: String,
        at: wt::Vec3,
        nbt: String,
    ) -> R<wt::EntityRef> {
        wit(self.game(|b| b.worlds_spawn_entity(&world, &kind, at.cv(), &nbt)))
    }

    fn remove_entity(&mut self, world: String, entity: u32) -> R<()> {
        wit(self.game(|b| b.worlds_remove_entity(&world, entity)))
    }

    fn entities_near(&mut self, world: String, at: wt::Vec3, radius: f64) -> R<Vec<wt::EntityRef>> {
        if !radius.is_finite() || radius < 0.0 {
            return Err(wt::HostError::Invalid(
                "radius must be finite and >= 0".into(),
            ));
        }
        wit(self.game(|b| b.worlds_entities_near(&world, at.cv(), radius)))
    }

    fn entity_location(&mut self, world: String, entity: u32) -> R<wt::Location> {
        wit(self.game(|b| b.worlds_entity_location(&world, entity)))
    }

    #[allow(clippy::too_many_arguments)]
    fn spawn_particle(
        &mut self,
        world: String,
        particle: String,
        options: String,
        at: wt::Vec3,
        count: u32,
        offset: wt::Vec3,
        speed: f64,
    ) -> R<()> {
        wit(self.game(|b| {
            b.worlds_spawn_particle(
                &world,
                &particle,
                &options,
                at.cv(),
                count,
                offset.cv(),
                speed,
            )
        }))
    }

    fn play_sound(
        &mut self,
        world: String,
        sound: String,
        at: wt::Vec3,
        volume: f32,
        pitch: f32,
    ) -> R<()> {
        wit(self.game(|b| b.worlds_play_sound(&world, &sound, at.cv(), volume, pitch)))
    }

    fn players(&mut self, world: String) -> Vec<wt::Uuid> {
        self.game_or("worlds.players", Vec::new(), |b| b.worlds_players(&world))
            .cv()
    }
}

// ---------------------------------------------------------------------------- scoreboards

impl wsb::Host for HostState {
    fn create_objective(&mut self, name: String, display: wt::Text, criteria: String) -> R<()> {
        let t: m::Text = display.cv();
        wit(self.game(|b| b.scoreboards_create_objective(&name, &t, &criteria)))
    }

    fn remove_objective(&mut self, name: String) -> R<()> {
        wit(self.game(|b| b.scoreboards_remove_objective(&name)))
    }

    fn set_display(&mut self, slot: wsb::DisplaySlot, objective: Option<String>) -> R<()> {
        wit(self.game(|b| b.scoreboards_set_display(slot.cv(), objective.as_deref())))
    }

    fn set_objective_display(&mut self, name: String, display: wt::Text) -> R<()> {
        let t: m::Text = display.cv();
        wit(self.game(|b| b.scoreboards_set_objective_display(&name, &t)))
    }

    fn set_score(&mut self, objective: String, entry: String, score: i32) -> R<()> {
        wit(self.game(|b| b.scoreboards_set_score(&objective, &entry, score)))
    }

    fn reset_score(&mut self, objective: String, entry: String) -> R<()> {
        wit(self.game(|b| b.scoreboards_reset_score(&objective, &entry)))
    }

    fn get_score(&mut self, objective: String, entry: String) -> Option<i32> {
        self.game_or("scoreboards.get-score", None, |b| {
            b.scoreboards_get_score(&objective, &entry)
        })
    }

    fn create_team(&mut self, name: String) -> R<()> {
        wit(self.game(|b| b.scoreboards_create_team(&name)))
    }

    fn remove_team(&mut self, name: String) -> R<()> {
        wit(self.game(|b| b.scoreboards_remove_team(&name)))
    }

    fn set_team_prefix(&mut self, name: String, prefix: wt::Text) -> R<()> {
        let t: m::Text = prefix.cv();
        wit(self.game(|b| b.scoreboards_set_team_prefix(&name, &t)))
    }

    fn set_team_suffix(&mut self, name: String, suffix: wt::Text) -> R<()> {
        let t: m::Text = suffix.cv();
        wit(self.game(|b| b.scoreboards_set_team_suffix(&name, &t)))
    }

    fn set_team_color(&mut self, name: String, color: wsb::TeamColor) -> R<()> {
        wit(self.game(|b| b.scoreboards_set_team_color(&name, color.cv())))
    }

    fn team_add(&mut self, name: String, entry: String) -> R<()> {
        wit(self.game(|b| b.scoreboards_team_add(&name, &entry)))
    }

    fn team_remove(&mut self, name: String, entry: String) -> R<()> {
        wit(self.game(|b| b.scoreboards_team_remove(&name, &entry)))
    }
}

// ---------------------------------------------------------------------------- bossbars

impl wb::Host for HostState {
    fn create(
        &mut self,
        title: wt::Text,
        color: wb::BarColor,
        style: wb::BarStyle,
        progress: f32,
    ) -> wb::BarId {
        let t: m::Text = title.cv();
        match self
            .main_api()
            .and_then(|api| api.bossbars_create(&t, color.cv(), style.cv(), progress))
        {
            Ok(id) => id,
            Err(e) => {
                // The WIT gives no error channel: 0 is never a valid bar id.
                log::warn!(target: "rustic", "[{}] bossbars.create: {e}", self.plugin);
                0
            }
        }
    }

    fn set_title(&mut self, bar: wb::BarId, title: wt::Text) {
        let t: m::Text = title.cv();
        if let Err(e) = self
            .main_api()
            .and_then(|api| api.bossbars_set_title(bar, &t))
        {
            log::debug!(target: "rustic", "[{}] bossbars.set-title: {e}", self.plugin);
        }
    }

    fn set_progress(&mut self, bar: wb::BarId, progress: f32) {
        if let Err(e) = self
            .main_api()
            .and_then(|api| api.bossbars_set_progress(bar, progress))
        {
            log::debug!(target: "rustic", "[{}] bossbars.set-progress: {e}", self.plugin);
        }
    }

    fn set_color(&mut self, bar: wb::BarId, color: wb::BarColor) {
        if let Err(e) = self
            .main_api()
            .and_then(|api| api.bossbars_set_color(bar, color.cv()))
        {
            log::debug!(target: "rustic", "[{}] bossbars.set-color: {e}", self.plugin);
        }
    }

    fn show(&mut self, bar: wb::BarId, player: wt::Uuid) -> R<()> {
        wit(self
            .main_api()
            .and_then(|api| api.bossbars_show(bar, player.cv())))
    }

    fn hide(&mut self, bar: wb::BarId, player: wt::Uuid) {
        if let Err(e) = self
            .main_api()
            .and_then(|api| api.bossbars_hide(bar, player.cv()))
        {
            log::debug!(target: "rustic", "[{}] bossbars.hide: {e}", self.plugin);
        }
    }

    fn remove(&mut self, bar: wb::BarId) {
        match self.main_api() {
            Ok(api) => api.bossbars_remove(bar),
            Err(_) => self.main_only_ignored("bossbars.remove"),
        }
    }
}

// ---------------------------------------------------------------------------- tablist

impl wtl::Host for HostState {
    fn set_header_footer(&mut self, player: wt::Uuid, header: wt::Text, footer: wt::Text) -> R<()> {
        let (h, f): (m::Text, m::Text) = (header.cv(), footer.cv());
        wit(self.game(|b| b.tablist_set_header_footer(player.cv(), &h, &f)))
    }

    fn set_display_name(&mut self, player: wt::Uuid, name: Option<wt::Text>) -> R<()> {
        let n: Option<m::Text> = name.cv();
        wit(self.game(|b| b.tablist_set_display_name(player.cv(), n.as_ref())))
    }
}

// ---------------------------------------------------------------------------- messaging

impl wmsg::Host for HostState {
    fn register_incoming(&mut self, channel: String) -> Result<(), String> {
        let api = self
            .main_api()
            .map_err(|e| format!("messaging.register-incoming: {e}"))?;
        api.messaging_register_incoming(&channel)
    }

    fn unregister_incoming(&mut self, channel: String) {
        match self.main_api() {
            Ok(api) => api.messaging_unregister_incoming(&channel),
            Err(_) => self.main_only_ignored("messaging.unregister-incoming"),
        }
    }

    fn send(&mut self, player: wt::Uuid, channel: String, data: Vec<u8>) -> R<()> {
        wit(self.game(|b| b.messaging_send(player.cv(), &channel, &data)))
    }

    fn behind_proxy(&mut self) -> bool {
        if !self.is_main {
            return self.host.snapshot().behind_proxy;
        }
        match rusticmc_core::context::backend_call(|b| b.messaging_behind_proxy()) {
            Ok(v) => {
                self.host.update_snapshot(|s| s.behind_proxy = v);
                v
            }
            Err(_) => false,
        }
    }
}
